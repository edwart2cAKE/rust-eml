#!/usr/bin/env python3
"""Render the binary tree of a quaternary-encoded EML function string as SVG.

Pure Python stdlib - no pip packages, no graphviz, no sympy.

Alphabet:
  0 = x                  (leaf)
  1 = y                  (leaf)
  3 = const 1            (leaf)
  2 = exp(a) - ln(b)     (binary operator: left child feeds exp, right feeds ln)

Usage:
  python3 scripts/draw_eml.py --string 203 --out 203.svg --caption
  python3 scripts/draw_eml.py --string 22322232232303133 --stdout
  python3 scripts/draw_eml.py --selftest
"""

import argparse
import sys
import xml.etree.ElementTree as ET


class DrawError(Exception):
    pass


# ------------------------------------------------------------------ tree build


class Node:
    __slots__ = ("ch", "left", "right", "x", "y")

    def __init__(self, ch, left=None, right=None):
        self.ch = ch
        self.left = left
        self.right = right
        self.x = 0.0
        self.y = 0.0


def split_pair(s):
    """Split a balanced expression into (first_segment, second_segment)."""
    n = len(s)
    i = 0
    count = 0
    while i < n:
        count += 1 if s[i] in "013" else -1
        i += 1
        if count == 1:
            break
    part1 = s[:i]
    count = 0
    start2 = i
    while i < n:
        count += 1 if s[i] in "013" else -1
        i += 1
        if count == 1:
            break
    if i != n:
        return None
    return part1, s[start2:i]


def parse_tree(qs):
    """Parse a quaternary string into a Node tree. Raises DrawError if invalid."""
    if not qs:
        raise DrawError("empty string")
    if len(qs) == 1:
        if qs not in "013":
            raise DrawError(f"single-character string {qs!r} must be a leaf (0/1/3)")
        return Node(qs)
    if not qs.startswith("2"):
        raise DrawError(f"multi-char string must start with '2' (got {qs[0]!r})")
    parts = split_pair(qs[1:])
    if parts is None:
        raise DrawError("unbalanced or structurally invalid string")
    a, b = parts
    return Node("2", parse_tree(a), parse_tree(b))


def to_expr(node):
    """Reconstruct a readable expression from a tree (no sympy needed)."""
    if node.ch == "0":
        return "x"
    if node.ch == "1":
        return "y"
    if node.ch == "3":
        return "1"
    return f"exp({to_expr(node.left)}) - ln({to_expr(node.right)})"


# ----------------------------------------------------------------------- layout

LINE_H = 15.0
PAD_X = 12.0
PAD_Y = 8.0
NODE_H = 2 * LINE_H + 2 * PAD_Y
Y_SPACING = 90.0
MARGIN = 30.0
GAP = 6.8


def label_lines(node):
    if node.ch == "2":
        return ["2", "exp(a) - ln(b)"]
    if node.ch == "0":
        return ["0", "x"]
    if node.ch == "1":
        return ["1", "y"]
    return ["3", "1"]


def node_width(lines):
    w = max(len(line) for line in lines) * 7.4 + 2 * PAD_X
    return max(w, 90.0 if lines[0] == "2" else 52.0)


def compute_layout(root):
    """In-order ranks become x, tree depth becomes y. Returns (nodes, max_depth, max_w)."""
    nodes = []
    max_depth = 0

    def walk(node, depth):
        nonlocal max_depth
        if node.left:
            walk(node.left, depth + 1)
        node.y = depth
        nodes.append(node)
        if node.right:
            walk(node.right, depth + 1)
        if depth > max_depth:
            max_depth = depth

    walk(root, 0)
    max_w = max(node_width(label_lines(n)) for n in nodes)
    x_spacing = max_w + GAP
    for rank, node in enumerate(nodes):
        node.x = x_spacing * rank
    return nodes, max_depth, max_w


# ----------------------------------------------------------------------- svg

def escape(text):
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def fill_for(node):
    if node.ch == "2":
        return "#fdf3d7"
    if node.ch == "0":
        return "#fbe4e4"
    if node.ch == "1":
        return "#e3edfc"
    return "#e8f3e2"


def stroke_for(node):
    if node.ch == "2":
        return "#c8a24a"
    if node.ch == "0":
        return "#c96262"
    if node.ch == "1":
        return "#5b7fc9"
    return "#6a9c5c"


def edge_path(px, py, cx, cy):
    top = py + NODE_H / 2
    bot = cy - NODE_H / 2
    mid = (top + bot) / 2
    return f"M {px:.1f} {top:.1f} L {px:.1f} {mid:.1f} L {cx:.1f} {mid:.1f} L {cx:.1f} {bot:.1f}"


def render_svg(qs, caption=False):
    root = parse_tree(qs)
    nodes, max_depth, max_w = compute_layout(root)

    caption_h = 44.0 if caption else 0.0
    total_h = MARGIN + max_depth * Y_SPACING + NODE_H / 2 + MARGIN + caption_h
    x_off = MARGIN + max_w / 2
    total_w = x_off + max(n.x for n in nodes) + max_w / 2 + MARGIN

    for node in nodes:
        node.x += x_off
        node.y = caption_h + MARGIN + node.y * Y_SPACING

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{total_w:.0f}" height="{total_h:.0f}" '
        f'viewBox="0 0 {total_w:.0f} {total_h:.0f}">'
    ]
    if caption:
        out.append(
            f'<text x="{MARGIN}" y="28" font-family="sans-serif" font-size="14" '
            f'font-weight="bold" fill="#333">{escape(to_expr(root))}</text>'
        )

    out.append('<g stroke="#9aa5b1" stroke-width="1.5" fill="none">')

    def draw_edges(node):
        if node.left:
            out.append(f'<path d="{edge_path(node.x, node.y, node.left.x, node.left.y)}"/>')
            draw_edges(node.left)
        if node.right:
            out.append(f'<path d="{edge_path(node.x, node.y, node.right.x, node.right.y)}"/>')
            draw_edges(node.right)

    draw_edges(root)
    out.append("</g>")

    def draw_node(node):
        lines = label_lines(node)
        w = node_width(lines)
        x0 = node.x - w / 2
        y0 = node.y - NODE_H / 2
        rx = 0.0 if node.ch == "2" else 8.0
        out.append(
            f'<rect x="{x0:.1f}" y="{y0:.1f}" width="{w:.1f}" height="{NODE_H:.1f}" '
            f'rx="{rx:.1f}" fill="{fill_for(node)}" stroke="{stroke_for(node)}" stroke-width="1.5"/>'
        )
        out.append(
            f'<text x="{node.x:.1f}" y="{node.y - 5:.1f}" text-anchor="middle" '
            f'font-family="sans-serif" font-size="13" font-weight="bold" fill="#222">'
            f'{escape(lines[0])}</text>'
        )
        out.append(
            f'<text x="{node.x:.1f}" y="{node.y + 12:.1f}" text-anchor="middle" '
            f'font-family="sans-serif" font-size="11" fill="#444">'
            f'{escape(lines[1])}</text>'
        )
        if node.left:
            draw_node(node.left)
        if node.right:
            draw_node(node.right)

    draw_node(root)
    out.append("</svg>")
    return "\n".join(out)


# ------------------------------------------------------------------- selftest


def selftest():
    ok = True

    for qs, expect_nodes, expect_leaves in [
        ("203", 3, 2),
        ("22322232232303133", 17, 9),
        ("22322232232313033", 17, 9),
    ]:
        root = parse_tree(qs)
        nodes = []

        def count(node):
            nodes.append(node)
            if node.left:
                count(node.left)
            if node.right:
                count(node.right)

        count(root)
        leaves = sum(1 for n in nodes if n.ch != "2")
        good = len(nodes) == expect_nodes and leaves == expect_leaves
        ok = ok and good
        print(f"{'PASS' if good else 'FAIL'}  {qs}: {len(nodes)} nodes ({expect_nodes}), {leaves} leaves ({expect_leaves})")

    for qs in ["0", "1", "3"]:
        good = parse_tree(qs).ch == qs
        ok = ok and good
        print(f"{'PASS' if good else 'FAIL'}  leaf {qs}")

    for qs in ["", "4", "23", "02", "222", "2x3", "223", "3333"]:
        try:
            parse_tree(qs)
            print(f"FAIL  {qs!r} should be invalid but parsed")
            ok = False
        except DrawError:
            print(f"PASS  {qs!r} correctly rejected")

    for qs in ["203", "22322232232303133"]:
        svg = render_svg(qs, caption=True)
        try:
            ET.fromstring(svg)
            good = "exp(a) - ln(b)" in svg
        except ET.ParseError:
            good = False
        ok = ok and good
        print(f"{'PASS' if good else 'FAIL'}  svg {qs} well-formed ({len(svg)} bytes)")

    return 0 if ok else 1


def main(argv):
    ap = argparse.ArgumentParser(
        description="Render the binary tree of a quaternary EML function string as SVG."
    )
    ap.add_argument("--string", help="quaternary string (0=x, 1=y, 3=1, 2=exp(a)-ln(b))")
    ap.add_argument("--out", help="output SVG path (default: <string>.svg in CWD)")
    ap.add_argument("--caption", action="store_true", help="add a title line with the expanded expression")
    ap.add_argument("--stdout", action="store_true", help="print SVG to stdout instead of writing a file")
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
        svg = render_svg(qs, caption=args.caption)
    except DrawError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1

    if args.stdout:
        print(svg)
        return 0

    out = args.out or f"{qs}.svg"
    with open(out, "w") as fh:
        fh.write(svg)
        fh.write("\n")
    print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
