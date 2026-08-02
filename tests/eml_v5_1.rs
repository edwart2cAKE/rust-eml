use std::collections::{BTreeSet, HashMap};
use cbfunc::{build_frontier, catalan, gen_complete, is_valid, total_valid, valid_count};

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

fn generated(base: u64, length: u32) -> BTreeSet<String> {
    let chars = alphabet_chars(base);
    let leaves: Vec<char> = chars[..chars.len() - 1].to_vec();
    let frontier = build_frontier(length, &leaves, usize::MAX);
    let mut out = BTreeSet::new();
    for (prefix, slots, budget) in frontier {
        let mut s = prefix;
        gen_complete(&mut s, slots, budget, &leaves, &mut |fs| {
            out.insert(fs.to_string());
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
                whole.insert(fs.to_string());
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
