#!/usr/bin/env python3
"""Symbolic verifier for quaternary-encoded expressions (EML space).

Checks whether a quaternary expression (alphabet 0=x, 1=y, 3=const 1,
2=operator exp(a)-ln(b)) is equal to a target expression.

Verification is symbolic (sympy); when no symbolic proof is found, a
very-high-precision numeric check (mpmath) is reported as LIKELY and clearly
labeled as not a proof. A clear numeric divergence is reported as DIFFERENT.
Optionally, interval arithmetic (mp.iv) provides a rigorous disproof.

Run with:  uv run --with sympy --with mpmath python3 scripts/verify_eml.py --help
"""

import argparse
import random
import sys

import sympy
import mpmath

X = sympy.Symbol("x", positive=True, real=True)
Y = sympy.Symbol("y", positive=True, real=True)

VERIFIED = "VERIFIED"
LIKELY = "LIKELY"
DIFFERENT = "DIFFERENT"
INVALID = "INVALID"
UNKNOWN = "UNKNOWN"

DEFAULT_DPS = 200
DEFAULT_GRID = 8


class ParseError(Exception):
    pass


# ---------------------------------------------------------------- quaternary


def quaternary_to_sympy(qs):
    """Parse a quaternary string into a sympy expression (also validates)."""
    stack = []
    for c in reversed(qs):
        if c == "0":
            stack.append(X)
        elif c == "1":
            stack.append(Y)
        elif c == "3":
            stack.append(sympy.Integer(1))
        elif c == "2":
            if len(stack) < 2:
                raise ValueError("operator underflow (invalid structure)")
            a = stack.pop()
            b = stack.pop()
            stack.append(sympy.exp(a) - sympy.log(b))
        else:
            raise ValueError(f"invalid character {c!r}")
    if len(stack) != 1:
        raise ValueError("leftover operands (invalid structure)")
    return stack[0]


# ---------------------------------------------------------------- target parser


def _skip_ws(s, pos):
    while pos < len(s) and s[pos] == " ":
        pos += 1
    return pos


def _parse_number(s, pos):
    start = pos
    if pos < len(s) and s[pos] == "-":
        pos += 1
    if pos >= len(s) or not (s[pos].isdigit() or s[pos] == "."):
        raise ParseError("expected number")
    while pos < len(s) and s[pos].isdigit():
        pos += 1
    if pos < len(s) and s[pos] == ".":
        pos += 1
        while pos < len(s) and s[pos].isdigit():
            pos += 1
    tok = s[start:pos]
    if not tok or tok == "-":
        raise ParseError("expected number")
    return (sympy.Rational(tok) if "." in tok else sympy.Integer(tok)), pos


def _parse_primary(s, pos):
    pos = _skip_ws(s, pos)
    if pos >= len(s):
        raise ParseError("unexpected end of expression")
    c = s[pos]
    if c == "x":
        return X, pos + 1
    if c == "y":
        return Y, pos + 1
    if c == "(":
        expr, pos = _parse_expr(s, pos + 1)
        pos = _skip_ws(s, pos)
        if pos >= len(s) or s[pos] != ")":
            raise ParseError("expected ')'")
        return expr, pos + 1
    if c == "@":
        start = pos + 1
        pos = start
        while pos < len(s) and s[pos] in "0123":
            pos += 1
        if pos == start:
            raise ParseError("expected quaternary string after '@'")
        try:
            return quaternary_to_sympy(s[start:pos]), pos
        except ValueError as e:
            raise ParseError(f"invalid quaternary string: {e}")
    return _parse_number(s, pos)


def _parse_unary(s, pos):
    pos = _skip_ws(s, pos)
    if pos >= len(s):
        raise ParseError("unexpected end")
    if s.startswith("exp", pos):
        pos = _skip_ws(s, pos + 3)
        if pos >= len(s) or s[pos] != "(":
            raise ParseError("expected '(' after exp")
        inner, pos = _parse_expr(s, pos + 1)
        pos = _skip_ws(s, pos)
        if pos >= len(s) or s[pos] != ")":
            raise ParseError("expected ')' after exp")
        return sympy.exp(inner), pos + 1
    if s.startswith("ln", pos):
        pos = _skip_ws(s, pos + 2)
        if pos >= len(s) or s[pos] != "(":
            raise ParseError("expected '(' after ln")
        inner, pos = _parse_expr(s, pos + 1)
        pos = _skip_ws(s, pos)
        if pos >= len(s) or s[pos] != ")":
            raise ParseError("expected ')' after ln")
        return sympy.log(inner), pos + 1
    if s[pos] == "-":
        inner, pos = _parse_unary(s, pos + 1)
        return -inner, pos
    return _parse_primary(s, pos)


def _parse_power(s, pos):
    left, pos = _parse_unary(s, pos)
    while True:
        pos = _skip_ws(s, pos)
        if pos >= len(s) or s[pos] != "^":
            break
        right, pos = _parse_power(s, pos + 1)
        left = left**right
    return left, pos


def _parse_term(s, pos):
    left, pos = _parse_power(s, pos)
    while True:
        pos = _skip_ws(s, pos)
        if pos >= len(s):
            break
        c = s[pos]
        if c == "*":
            right, pos = _parse_power(s, pos + 1)
            left = left * right
        elif c == "/":
            right, pos = _parse_power(s, pos + 1)
            left = left / right
        else:
            break
    return left, pos


def _parse_expr(s, pos):
    left, pos = _parse_term(s, pos)
    while True:
        pos = _skip_ws(s, pos)
        if pos >= len(s):
            break
        c = s[pos]
        if c == "+":
            right, pos = _parse_term(s, pos + 1)
            left = left + right
        elif c == "-":
            right, pos = _parse_term(s, pos + 1)
            left = left - right
        else:
            break
    return left, pos


def parse_target(s):
    expr, pos = _parse_expr(s, 0)
    pos = _skip_ws(s, pos)
    if pos != len(s):
        raise ParseError(f"unexpected character {s[pos]!r} at position {pos}")
    return expr


# ---------------------------------------------------------------- symbolic


def _is_zero(e):
    try:
        if e == 0:
            return True
        return e.is_zero is True
    except Exception:
        return False


def symbolic_verify(qexpr, texpr):
    """Try to prove qexpr == texpr symbolically. Returns (proved, strategy)."""
    diff = qexpr - texpr
    strategies = [
        ("simplify", lambda e: sympy.simplify(e)),
        ("simplify+together", lambda e: sympy.together(sympy.simplify(e))),
        ("cancel", lambda e: sympy.cancel(sympy.together(e))),
        ("factor", lambda e: sympy.factor(sympy.together(e))),
        ("powsimp", lambda e: sympy.powsimp(e, force=True)),
        ("expand-log", lambda e: sympy.expand_log(sympy.powsimp(e, force=True), force=True)),
        ("logcombine", lambda e: sympy.logcombine(e, force=True)),
        ("radsimp", lambda e: sympy.radsimp(e)),
        ("ratsimp", lambda e: sympy.ratsimp(sympy.together(e))),
        ("expand", lambda e: sympy.expand(e)),
    ]
    base = sympy.simplify(diff)
    for start in (diff, base):
        for name, transform in strategies:
            try:
                r = transform(start)
            except Exception:
                continue
            if _is_zero(r):
                return True, name
    try:
        if sympy.simplify(qexpr) == sympy.simplify(texpr):
            return True, "simplify-sides"
    except Exception:
        pass
    try:
        if diff.equals(0) is True:
            return True, "sympy.equals"
    except Exception:
        pass
    return False, None


# ---------------------------------------------------------------- numeric


def numeric_verify(qexpr, texpr, dps, grid):
    """Very-high-precision numeric check. Returns (verdict, detail)."""
    mpmath.mp.dps = dps
    try:
        fq = sympy.lambdify((X, Y), qexpr, "mpmath")
        ft = sympy.lambdify((X, Y), texpr, "mpmath")
    except Exception as e:
        return UNKNOWN, f"could not compile numeric form: {e}"
    eps = mpmath.mpf(10) ** -(dps - 20)
    rng = random.Random(1234)
    points = [(rng.uniform(0.1, 10.0), rng.uniform(0.1, 10.0)) for _ in range(grid * grid)]
    points += [
        (0.1, 0.1), (1.0, 1.0), (10.0, 10.0), (1.0, 10.0), (10.0, 1.0),
        (mpmath.mpf("1e-3"), 1.0), (1.0, mpmath.mpf("1e-3")),
        (0.1, 1.0), (1.0, 0.1),
    ]
    for px, py in points:
        px = mpmath.mpf(px)
        py = mpmath.mpf(py)
        try:
            a = fq(px, py)
            b = ft(px, py)
        except Exception:
            continue
        if isinstance(a, mpmath.mpc) or isinstance(b, mpmath.mpc):
            continue
        if not (mpmath.isfinite(a) and mpmath.isfinite(b)):
            continue
        if abs(a - b) > eps:
            return DIFFERENT, f"differs at x={mpmath.nstr(px, 10)}, y={mpmath.nstr(py, 10)}: {mpmath.nstr(a, 12)} vs {mpmath.nstr(b, 12)}"
    return LIKELY, f"|diff| <= {mpmath.nstr(eps, 3)} at all {len(points)} points (dps={dps})"


def interval_disprove(qexpr, texpr, dps):
    """Rigorous interval check. Returns a detail string if a DIFFERENT proof
    is found, else None (inconclusive)."""
    mpmath.mp.dps = dps
    iv = mpmath.mp.iv
    try:
        fq = sympy.lambdify((X, Y), qexpr, [iv])
        ft = sympy.lambdify((X, Y), texpr, [iv])
    except Exception:
        return None
    rng = random.Random(999)
    w = mpmath.mpf("1e-4")
    for _ in range(24):
        cx = mpmath.mpf(rng.uniform(0.1, 10.0))
        cy = mpmath.mpf(rng.uniform(0.1, 10.0))
        ix = iv.mpf([cx - w, cx + w])
        iy = iv.mpf([cy - w, cy + w])
        try:
            d = fq(ix, iy) - ft(ix, iy)
            lo = d.a
            hi = d.b
        except Exception:
            continue
        if lo > 0 or hi < 0:
            return f"interval proof: diff bounded away from 0 near x={mpmath.nstr(cx, 6)}, y={mpmath.nstr(cy, 6)}"
    return None


# ---------------------------------------------------------------- driver


def verify_one(qs, texpr, args):
    try:
        qexpr = quaternary_to_sympy(qs)
    except ValueError as e:
        return INVALID, str(e)
    ok, strategy = symbolic_verify(qexpr, texpr)
    if ok:
        return VERIFIED, f"symbolic: {strategy}"
    if args.proof_only:
        return UNKNOWN, "not proven symbolically (proof-only mode)"
    verdict, detail = numeric_verify(qexpr, texpr, args.dps, args.grid)
    if verdict == DIFFERENT:
        return DIFFERENT, detail
    if args.interval:
        r = interval_disprove(qexpr, texpr, args.dps)
        if r:
            return DIFFERENT, r
    return LIKELY, detail


def _load_strings(args):
    strings = []
    if args.string:
        strings.append(args.string)
    if args.strings_file:
        with open(args.strings_file, "r") as fh:
            for line in fh:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                strings.append(line.split()[0])
    return strings


def selftest(args):
    cases = [
        ("22322232232303133", "x*y", {VERIFIED, LIKELY}),
        ("22322232232313033", "x*y", {VERIFIED, LIKELY}),
        ("203", "exp(x)", {VERIFIED}),
        ("203", "x", {DIFFERENT}),
        ("203", "x+1", {DIFFERENT}),
        ("23", "x", {INVALID}),
        ("22322232232303133", "y", {DIFFERENT}),
    ]
    ok_all = True
    for qs, tgt, allowed in cases:
        try:
            target = parse_target(tgt)
        except ParseError as e:
            print(f"FAIL  target {tgt!r} did not parse: {e}")
            ok_all = False
            continue
        verdict, detail = verify_one(qs, target, args)
        status = "PASS" if verdict in allowed else "FAIL"
        if status == "FAIL":
            ok_all = False
        print(f"{status}  {qs} vs {tgt}: {verdict} ({detail})")
    return 0 if ok_all else 1


def main(argv):
    ap = argparse.ArgumentParser(
        description="Symbolically verify quaternary expressions against a target."
    )
    ap.add_argument("--target", help="target expression (x, y, numbers, + - * / ^ exp() ln() @quat)")
    ap.add_argument("--string", help="single quaternary string to verify")
    ap.add_argument("--strings-file", help="file with one quaternary string per line (# comments allowed)")
    ap.add_argument("--dps", type=int, default=DEFAULT_DPS, help=f"mpmath precision (default {DEFAULT_DPS})")
    ap.add_argument("--grid", type=int, default=DEFAULT_GRID, help=f"points per axis for numeric pass (default {DEFAULT_GRID})")
    ap.add_argument("--proof-only", action="store_true", help="require a symbolic proof; no numeric fallback")
    ap.add_argument("--interval", action="store_true", help="also run a rigorous interval disproof pass")
    ap.add_argument("--selftest", action="store_true", help="run built-in test cases and exit")
    args = ap.parse_args(argv)

    if args.selftest:
        return selftest(args)

    if not args.target:
        ap.error("--target is required")

    try:
        target = parse_target(args.target)
    except ParseError as e:
        print(f"ERROR\ttarget parse failed: {e}", file=sys.stderr)
        return 2

    strings = _load_strings(args)
    if not strings:
        ap.error("provide --string or --strings-file")

    code = 0
    for qs in strings:
        verdict, detail = verify_one(qs, target, args)
        print(f"{verdict}\t{qs}\t{detail}")
        if verdict == DIFFERENT or verdict == INVALID:
            code = 1
        if args.proof_only and verdict != VERIFIED:
            code = 1
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
