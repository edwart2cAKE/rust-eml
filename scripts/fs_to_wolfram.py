#!/usr/bin/env python3
"""Convert a quaternary function string (fs) into a Wolfram Language expression.

Pure Python stdlib - reuses draw_eml.py's tree parser.

Alphabet:
  0 = x                  (leaf)
  1 = y                  (leaf)
  3 = const 1            (leaf)
  2 = exp(a) - ln(b)     (binary operator: left child feeds exp, right feeds ln)

Wolfram mapping:
  0 -> x, 1 -> y, 3 -> 1
  2 -> Exp[LEFT] - Log[RIGHT]        (default)
  2 -> Subtract[Exp[LEFT], Log[RIGHT]]  (--fullform)

Usage:
  python3 scripts/fs_to_wolfram.py --string 203
  python3 scripts/fs_to_wolfram.py --string 22322232232303133 --fullform
  python3 scripts/fs_to_wolfram.py --selftest
"""

import argparse
import sys

from draw_eml import DrawError, parse_tree


def to_wolfram(node, fullform=False):
    if node.ch == "0":
        return "x"
    if node.ch == "1":
        return "y"
    if node.ch == "3":
        return "1"
    a = to_wolfram(node.left, fullform)
    b = to_wolfram(node.right, fullform)
    if fullform:
        return f"Subtract[Exp[{a}], Log[{b}]]"
    return f"Exp[{a}] - Log[{b}]"


# ------------------------------------------------------------------- selftest


def selftest():
    ok = True

    for qs, expected, expected_full in [
        ("203", "Exp[x] - Log[1]", "Subtract[Exp[x], Log[1]]"),
        ("0", "x", "x"),
        ("1", "y", "y"),
        ("3", "1", "1"),
    ]:
        root = parse_tree(qs)
        for flag, expected in ((False, expected), (True, expected_full)):
            got = to_wolfram(root, fullform=flag)
            good = got == expected
            ok = ok and good
            label = "fullform" if flag else "standard"
            print(f"{'PASS' if good else 'FAIL'}  {qs} ({label}): {got}")

    for qs in ["", "4", "23", "02", "222", "2x3", "223", "3333"]:
        try:
            parse_tree(qs)
            print(f"FAIL  {qs!r} should be invalid but parsed")
            ok = False
        except DrawError:
            print(f"PASS  {qs!r} correctly rejected")

    for qs in ["22322232232303133", "22322232232313033"]:
        root = parse_tree(qs)
        for fullform in (False, True):
            expr = to_wolfram(root, fullform=fullform)
            good = expr.count("[") == expr.count("]") and expr.startswith(("Exp", "Subtract"))
            ok = ok and good
            print(f"{'PASS' if good else 'FAIL'}  {qs} brackets balanced ({len(expr)} chars)")

    return 0 if ok else 1


def main(argv):
    ap = argparse.ArgumentParser(
        description="Convert a quaternary function string into a Wolfram Language expression."
    )
    ap.add_argument("--string", help="quaternary string (0=x, 1=y, 3=1, 2=exp(a)-ln(b))")
    ap.add_argument("--fullform", action="store_true", help="emit Subtract[Exp[..], Log[..]] instead of Exp[..] - Log[..]")
    ap.add_argument("--selftest", action="store_true", help="run built-in checks and exit")
    args = ap.parse_args(argv)

    if args.selftest:
        return selftest()

    if not args.string:
        ap.error("--string is required")

    qs = args.string
    for ch in qs:
        if ch not in "0123":
            print(f"error: invalid character {ch!r} (allowed: 0, 1, 2, 3)", file=sys.stderr)
            return 1
    try:
        root = parse_tree(qs)
    except DrawError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1

    print(to_wolfram(root, fullform=args.fullform))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
