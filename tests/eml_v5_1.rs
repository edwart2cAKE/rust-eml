use std::collections::{BTreeSet, HashMap};
use std::process::Command;
use cbfunc::{build_frontier, catalan, eval_quaternary, gen_complete, is_valid, parse_num, total_valid, valid_count};

fn alphabet_chars(base: u64) -> Vec<char> {
    let base_chars = ['0', '1', '3'];
    let mut out: Vec<char> = base_chars[..(base as usize - 1)].to_vec();
    out.push('2');
    out
}

fn alphabet_string(mut i: u64, base: u64, length: u32) -> String {
    let chars = alphabet_chars(base);
    let mut s = Vec::new();
    for _ in 0..length {
        s.push(chars[(i % base) as usize]);
        i /= base;
    }
    s.into_iter().collect()
}

fn brute_valid(base: u64, length: u32) -> Vec<String> {
    let mut out = Vec::new();
    let total = base.pow(length);
    for i in 0..total {
        let s = alphabet_string(i, base, length);
        let mut cache = HashMap::new();
        if is_valid(&s, &mut cache) {
            out.push(s);
        }
    }
    out
}

fn gen_bytes(s: &[u8]) -> String {
    s.iter().map(|&b| b as char).collect()
}

fn generated(base: u64, length: u32) -> BTreeSet<String> {
    let chars = alphabet_chars(base);
    let leaves: Vec<char> = chars[..chars.len() - 1].to_vec();
    let frontier = build_frontier(length, &leaves, usize::MAX);
    let mut out = BTreeSet::new();
    for (prefix, slots, budget) in frontier {
        let mut s = prefix;
        gen_complete(&mut s, slots, budget, &leaves, &mut |fs| {
            out.insert(gen_bytes(fs));
            true
        });
    }
    out
}

#[test]
fn catalan_values() {
    let expected = [1u64, 1, 2, 5, 14, 42, 132, 429, 1430, 4862];
    for (k, &e) in expected.iter().enumerate() {
        assert_eq!(catalan(k as u64), e);
    }
}

#[test]
fn count_formula() {
    assert_eq!(valid_count(3, 9), 287_096_238);
    assert_eq!(valid_count(2, 9), 4_978_688);
    assert_eq!(total_valid(3, 19), 318_380_772);
    assert_eq!(total_valid(2, 19), 5_840_806);
    assert_eq!(total_valid(3, 1), 3);
}

#[test]
fn generation_matches_brute_force() {
    for base in 2..=4u64 {
        let mut length = 1u32;
        while length <= 9 {
            let brute: BTreeSet<String> = brute_valid(base, length).into_iter().collect();
            let gen = generated(base, length);
            assert_eq!(gen.len(), brute.len(), "base {}, length {}", base, length);
            assert_eq!(gen, brute, "base {}, length {}", base, length);
            length += 2;
        }
    }
}

#[test]
fn gen_complete_matches_frontier() {
    for length in (3u32..=9).step_by(2) {
        let chars = alphabet_chars(4);
        let leaves: Vec<char> = chars[..chars.len() - 1].to_vec();
        let frontier = build_frontier(length, &leaves, (1 << 20).min(usize::MAX));
        let mut whole = BTreeSet::new();
        for (prefix, slots, budget) in frontier {
            let mut s = prefix;
            gen_complete(&mut s, slots, budget, &leaves, &mut |fs| {
                whole.insert(gen_bytes(fs));
                true
            });
        }
        assert_eq!(whole.len() as u64, valid_count(3, (length as u64 - 1) / 2), "length {}", length);
    }
}

#[test]
fn no_duplicate_emission() {
    for length in (1u32..=13).step_by(2) {
        let chars = alphabet_chars(4);
        let leaves: Vec<char> = chars[..chars.len() - 1].to_vec();
        for target in [8usize, 1024, usize::MAX] {
            let frontier = build_frontier(length, &leaves, target);
            let mut count = 0usize;
            for (prefix, slots, budget) in frontier {
                let mut s = prefix;
                gen_complete(&mut s, slots, budget, &leaves, &mut |_| {
                    count += 1;
                    true
                });
            }
            assert_eq!(
                count as u64,
                valid_count(3, (length as u64 - 1) / 2),
                "length {}, frontier target {}",
                length,
                target
            );
        }
    }
}

#[test]
fn eval_quaternary_matches_parse_num() {
    let chars = alphabet_chars(4);
    let leaves: Vec<char> = chars[..chars.len() - 1].to_vec();
    let points = [(0.1, 0.1), (0.1, 10.0), (10.0, 0.1), (10.0, 10.0), (1.0, 1.0), (3.7, 2.2)];
    for length in (1u32..=13).step_by(2) {
        let frontier = build_frontier(length, &leaves, usize::MAX);
        for (prefix, slots, budget) in frontier {
            let mut s = prefix;
            gen_complete(&mut s, slots, budget, &leaves, &mut |fs| {
                let parsed = parse_num(std::str::from_utf8(fs).expect("generated string must be utf8"))
                    .expect("generated string must parse");
                for &(x, y) in &points {
                    let direct = eval_quaternary(fs, x, y).expect("generated string must eval");
                    let tree = parsed.eval(x, y);
                    assert!(
                        (direct.is_nan() && tree.is_nan()) || direct == tree,
                        "mismatch for {} at ({}, {}): direct={} tree={}",
                        gen_bytes(fs),
                        x,
                        y,
                        direct,
                        tree
                    );
                }
                true
            });
        }
    }
}

#[test]
fn eval_quaternary_known_matches() {
    for s in ["22322232232303133", "22322232232313033"] {
        let parsed = parse_num(s).expect("known match must parse");
        for (x, y) in [(0.5, 0.5), (2.0, 3.0), (10.0, 0.1)] {
            let direct = eval_quaternary(s.as_bytes(), x, y).expect("known match must eval");
            let tree = parsed.eval(x, y);
            assert!(
                (direct.is_nan() && tree.is_nan()) || direct == tree,
                "mismatch for {} at ({}, {}): direct={} tree={}",
                s,
                x,
                y,
                direct,
                tree
            );
        }
    }
}

#[test]
fn eval_quaternary_rejects_invalid() {
    for bad in ["23", "223", "02", "222", "3333", "20", "2x3", ""] {
        assert!(
            eval_quaternary(bad.as_bytes(), 1.0, 2.0).is_none(),
            "{:?} should evaluate to None",
            bad
        );
    }
    assert_eq!(eval_quaternary(b"0", 1.0, 2.0), Some(1.0));
    assert_eq!(eval_quaternary(b"1", 1.0, 2.0), Some(2.0));
    assert_eq!(eval_quaternary(b"3", 1.0, 2.0), Some(1.0));
}

#[test]
fn eml_v5_1_bin_finds_and_verifies() {
    let bin = env!("CARGO_BIN_EXE_eml_v5_1");
    let out = Command::new(bin)
        .args(["--target", "exp(x)", "--depth", "3"])
        .output()
        .expect("failed to run eml_v5_1");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("203 (depth 3)"), "stdout: {}", stdout);
    assert!(
        stdout.contains("[VERIFIED]") || stdout.contains("could not run uv/sympy verifier"),
        "stdout: {}",
        stdout
    );
}
