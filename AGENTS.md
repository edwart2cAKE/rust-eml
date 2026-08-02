# AGENTS.md

Rust bruteforcer searching "EML space" (quaternary-encoded expressions) for strings matching a target. See README.md and https://arxiv.org/html/2603.21852v2.

## Build & run
- `cargo build --release` — searches are slow; debug builds are impractical for real runs
- `cargo run --release --bin eml_v5_1 -- --target "x*y" --depth 17` — general search tool (`--depth`/`--time` are mutually exclusive); prefer v5_1 over v5_0 (valid-only enumeration, ~2-3 orders of magnitude faster). At the end of a search, found matches are auto-verified symbolically (see Verifier below); `--no-verify` skips this
- `uv run --with sympy --with mpmath python3 scripts/verify_eml.py --target <EXPR> --string <quaternary_string>` — symbolic verifier: proves equality via sympy when it can (VERIFIED), else does a very-high-precision numeric check (LIKELY, dps=200 default), and always flags clear differences (DIFFERENT). Batch: `--strings-file` (tolerates `"<qs> (depth N)"` log lines); `--proof-only` and `--interval` available; `--selftest` runs built-in cases
- `cargo run --release --bin check_xy -- <quaternary_string>` — validate a string and check if it equals x*y
- `python3 scripts/draw_eml.py --string <quaternary_string> --caption` — render the string's binary tree as a standalone SVG (pure Python stdlib; `--out`, `--stdout`, `--selftest`). Same split semantics as `split_pair`; mirrors verify_eml.py's grammar
- `python3 scripts/fs_to_wolfram.py --string <quaternary_string> [--fullform]` — convert a string to a Wolfram Language expression (`Exp[a] - Log[b]`; `--fullform` gives `Subtract[Exp[a], Log[b]]`); pure stdlib, reuses draw_eml.py's parser; `--selftest`
- `cargo test --release --test eml_v5_1` — integration tests for the valid-string generator (in tests/eml_v5_1.rs); no other bins have tests
- Bin names ≠ source names (see Cargo.toml `[[bin]]`): `cbfunc_v2.rs`→`cbfunc2`, `cbfunc_v3.rs`→`cbfunc3`. Bins: eml_v5_1, eml_v5_0, check_xy, eml_xy_from_exy1, eml_v1, cbfunc, cbfunc2, cbfunc3
- Older bins (cbfunc*, eml_v1, eml_xy_from_exy1) are one-off scripts with hardcoded target + DEPTH, all marked `#![deprecated]`; prefer eml_v5_1 for new work. eml_v5_0 is also `#![deprecated]` (index enumeration too slow)

## Encoding
- Alphabet: `0`=x, `1`=y, `3`=const 1, `2`=operator `exp(a)-ln(b)` (binary)
- Valid: single leaf, or starts with `2` and splits into two balanced sub-expressions (`split_pair` in src/lib.rs). Multi-char strings not starting with `2` are invalid
- Older binaries use reduced alphabets (ternary 0/2/3, binary 0/2), hardcoded per file

## Architecture
- src/lib.rs is the shared crate `cbfunc`; all binaries `use cbfunc::...`. lib.rs holds everything reusable: validity/parse (`is_valid`, `parse_num`, `split_pair`), `NumExpr` eval + `equals_num`, the target-expression parser + `TargetExpr`/`fuzzy_equal` (probabilistic match), valid-string generation (`catalan`, `valid_count`, `total_valid`, `gen_complete`, `build_frontier`), `search_length`, and `open_progress_log`
- Bins (eml_v5_0/eml_v5_1) are thin CLI wrappers: arg parsing, alphabet selection from target variables, progress reporter, `main`
- eml_v5_1 generates only structurally valid strings (full binary trees: leaves = operators + 1; all valid strings have odd length). Per length L=2k+1 with T leaf types, count = Catalan(k)·T^(k+1); even lengths are empty. `build_frontier` builds a parallel work set of disjoint prefix states (an antichain — every string emitted exactly once; do NOT keep expanded parents in the frontier or work is duplicated ~100x)
- Matching is probabilistic: 10 random eval points (x,y ∈ [0.1,10]) vs target; tolerance 1e-10 in cbfunc*/check_xy, 1e-5 default in eml_v5_0/eml_v5_1. RNG is non-deterministic (thread_rng / StdRng::from_entropy) → possible false positives

## Verifier (scripts/verify_eml.py)
- Re-implements the target grammar (x, y, numbers, + - * / ^, exp()/ln(), `@quat`) and the quaternary→expression mapping in Python/sympy. Emits one tab-separated line per string: `<VERDICT>\t<string>\t<detail>`
- Verdicts: `VERIFIED` (sympy proof via a strategy cascade: simplify/cancel/factor/powsimp/expand_log/logcombine/radsimp/ratsimp), `LIKELY` (very-high-precision mpmath, labeled not-a-proof), `DIFFERENT`, `INVALID`, `UNKNOWN` (proof-only mode)
- `eml_v5_1` shells out to it via `uv run --with sympy --with mpmath` at the end of a search when matches exist; degrades gracefully (prints unverified + warning) if `uv`/sympy are unavailable. First run downloads sympy (needs network)
- Caveat: full symbolic equality of exp/ln expressions is undecidable in general (Richardson); the deep x*y matches at depth 17 are provable by `simplify`, but not every true equality may be automatically provable — those fall back to LIKELY

## Gotchas
- Full searches iterate up to ~Catalan(k)·T^(k+1) valid strings (depth 17 x*y ≈ 31M, ~90s); v5_0's old index-based enumeration iterates 4^depth strings and is impractically slow
- All binaries write progress + matches to `logs/` (gitignored, created at runtime); matches also print to stdout
- No lint, fmt, or CI. `Cargo.lock` is gitignored. Deps: rand 0.8, rayon 1
