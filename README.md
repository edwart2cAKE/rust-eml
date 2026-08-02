# rust-eml
Search through eml space using a rust-based bruteforcer.

Based on this paper: https://arxiv.org/html/2603.21852v2

## Encoding

Each string is a quaternary encoding of an expression tree:

| char | meaning |
|------|---------|
| `0`  | variable `x` (leaf) |
| `1`  | variable `y` (leaf) |
| `3`  | constant `1` (leaf) |
| `2`  | binary operator `exp(a) - ln(b)` (left subtree feeds `exp`, right feeds `ln`) |

A valid string is either a single leaf (`0`, `1`, or `3`), or starts with `2`
and splits into two balanced sub-expressions. All valid strings have odd length:
for length `L = 2k + 1` with `T` leaf types, the count is `Catalan(k) * T^(k+1)`.
`22322232232303133` and `22322232232313033`, for example, are two 17-character
encodings of `x*y`.

## Tools

### Search: `eml_v5_1` (preferred)
```
cargo run --release --bin eml_v5_1 -- --target "x*y" --depth 17
cargo run --release --bin eml_v5_1 -- --target "x*y" --time 60 --threads 4
```
- `--depth` and `--time` are mutually exclusive (time mode runs unbounded and
  warns past ~1e9 iterations).
- `--tol` sets the fuzzy-match tolerance (default `1e-5`).
- At the end of a search, found matches are automatically verified symbolically
  (see Verifier below). `--no-verify` skips this.
- Progress and matches are also written to `logs/`.

`eml_v5_1` enumerates only structurally valid strings (~2-3 orders of magnitude
faster than the older `eml_v5_0`, which is deprecated). Matching is probabilistic:
random points `(x, y)` are evaluated against the target, so a match is a strong
hint that must be confirmed by the verifier.

### Verify: `scripts/verify_eml.py`
```
uv run --with sympy --with mpmath python3 scripts/verify_eml.py \
  --target "x*y" --string 22322232232303133
```
- Proves equality symbolically with sympy when possible (`VERIFIED`), otherwise
  does a very-high-precision numeric check (`LIKELY`, dps=200 default), and
  flags clear differences (`DIFFERENT`).
- Verdicts: `VERIFIED` / `LIKELY` / `DIFFERENT` / `INVALID` / `UNKNOWN`.
- Batch mode via `--strings-file` (tolerates `"<qs> (depth N)"` log lines);
  `--proof-only` and `--interval` also available; `--selftest` runs built-in cases.
- Requires `uv` (pulls sympy + mpmath on first run; needs network).

### Draw: `scripts/draw_eml.py`
```
python3 scripts/draw_eml.py --string 22322232232303133 --caption
```
- Renders the binary tree of a quaternary string as a standalone SVG, pure
  Python stdlib (no graphviz, no pip packages).
- `--out <file>` (default `<string>.svg`), `--stdout` to print the SVG,
  `--caption` to add a title with the expanded expression,
  `--selftest` for built-in checks.

### Convert to Wolfram: `scripts/fs_to_wolfram.py`
```
python3 scripts/fs_to_wolfram.py --string 203
# Exp[x] - Log[1]
python3 scripts/fs_to_wolfram.py --string 22322232232303133 --fullform
# Subtract[Exp[..], Log[..]] — unambiguous InputForm
```
- Converts a quaternary string into a copy-paste-ready Wolfram Language
  expression: `0`→`x`, `1`→`y`, `3`→`1`, `2`→`Exp[LEFT] - Log[RIGHT]`.
- Pure Python stdlib; reuses the `draw_eml.py` parser.
- `--fullform` emits `Subtract[Exp[..], Log[..]]`; `--selftest` runs built-in checks.

## Results

Strings matching operator `x*y` at depth 17 (both `VERIFIED` symbolically):
- `22322232232303133`
- `22322232232313033`

Operator `x-y` in a 10s search with 4 threads:
- `22322303213`
