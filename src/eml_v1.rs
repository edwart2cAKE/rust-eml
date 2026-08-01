use std::collections::HashMap;
use std::io::Write;
use std::time::Instant;

const DEPTH: u32 = 20;
const TARGET: f64 = 42.0;
const TOLERANCE: f64 = 1e-10;

fn get_binary_string(num: u64, length: u32) -> String {
    let mut s = String::with_capacity(length as usize);
    for bit in (0..length).rev() {
        if (num >> bit) & 1 == 0 {
            s.push('0');
        } else {
            s.push('2');
        }
    }
    s
}

fn split_pair(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    let mut count = 0i32;
    loop {
        if i >= n {
            return None;
        }
        if bytes[i] == b'1' || bytes[i] == b'0' {
            count += 1;
        } else {
            count -= 1;
        }
        i += 1;
        if count == 1 {
            break;
        }
    }
    let part1 = &s[..i];
    let start2 = i;
    let mut count = 0i32;
    loop {
        if i >= n {
            return None;
        }
        if bytes[i] == b'1' || bytes[i] == b'0' {
            count += 1;
        } else {
            count -= 1;
        }
        i += 1;
        if count == 1 {
            break;
        }
    }
    if i != n {
        return None;
    }
    Some((part1, &s[start2..i]))
}

fn get_value(s: &str, cache: &mut HashMap<String, f64>) -> Option<f64> {
    if let Some(&v) = cache.get(s) {
        return Some(v);
    }
    let val = if s == "0" {
        1.0
    } else if s.starts_with('2') {
        let (a, b) = split_pair(&s[1..])?;
        let av = get_value(a, cache)?;
        let bv = get_value(b, cache)?;
        av.exp() - bv.ln()
    } else {
        return None;
    };
    if val.is_finite() {
        cache.insert(s.to_owned(), val);
        Some(val)
    } else {
        None
    }
}

fn main() {
    let mut cache: HashMap<String, f64> = HashMap::new();

    let mut total: u64 = 0;
    for l in 1..=DEPTH {
        total += 1u64 << l;
    }

    let mut progress = cbfunc::open_progress_log("progress_eml_v1.log").unwrap();
    writeln!(
        progress,
        "DEPTH={} | TARGET={} | {} total iterations",
        DEPTH, TARGET, total
    )
    .unwrap();
    progress.flush().unwrap();

    let overall_start = Instant::now();
    let mut matched = 0u64;
    let mut iterated: u64 = 0;

    for length in 1..=DEPTH {
        let count = 1u64 << length;
        for i in 0..count {
            let fs = get_binary_string(i, length);
            if let Some(val) = get_value(&fs, &mut cache) {
                if (val - TARGET).abs() < TOLERANCE {
                    matched += 1;
                    writeln!(progress, "MATCH: {} = {}", fs, val).unwrap();
                    progress.flush().unwrap();
                    println!("MATCH: {} = {}", fs, val);
                    std::io::stdout().flush().unwrap();
                }
            }
            iterated += 1;
            if iterated % 1_000_000 == 0 {
                let elapsed = overall_start.elapsed().as_secs_f64();
                let rate = iterated as f64 / elapsed;
                let pct = iterated as f64 / total as f64 * 100.0;
                let remaining = (total - iterated) as f64 / rate;
                writeln!(
                    progress,
                    "  {}/{} ({:.1}%) | {:.0} it/s | ~{:.0}s remaining | {} matches | cache: {}",
                    iterated, total, pct, rate, remaining, matched, cache.len()
                )
                .unwrap();
                progress.flush().unwrap();
            }
        }
    }

    let elapsed = overall_start.elapsed();
    writeln!(
        progress,
        "Done in {:.1?} | {} matches found | cache size: {}",
        elapsed, matched, cache.len()
    )
    .unwrap();
    progress.flush().unwrap();
    println!("Done in {:.1?} | {} matches found", elapsed, matched);
}
