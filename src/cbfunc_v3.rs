use std::collections::HashMap;
use std::io::Write;
use std::time::Instant;
use cbfunc::{NumExpr, is_valid, parse_num, equals_num};

const DEPTH: u32 = 30;

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

fn main() {
    let target_num = NumExpr::Add(
        Box::new(NumExpr::X),
        Box::new(NumExpr::Const(1.0)),
    );

    let mut validity_cache: HashMap<String, bool> = HashMap::new();
    let mut rng = rand::thread_rng();

    let mut total: u64 = 0;
    for l in 1..=DEPTH {
        total += 1u64 << l;
    }

    let mut progress = std::fs::File::create("progress_v3.log").unwrap();
    writeln!(
        progress,
        "DEPTH={} | {} total iterations | searching for x + 1",
        DEPTH, total
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
            if is_valid(&fs, &mut validity_cache) {
                if let Some(parsed) = parse_num(&fs) {
                    if equals_num(&parsed, &target_num, &mut rng) {
                        matched += 1;
                        writeln!(progress, "MATCH: {}", fs).unwrap();
                        progress.flush().unwrap();
                        println!("MATCH: {}", fs);
                        std::io::stdout().flush().unwrap();
                    }
                }
            }
            iterated += 1;
            if iterated % 10_000_000 == 0 {
                let elapsed = overall_start.elapsed().as_secs_f64();
                let rate = iterated as f64 / elapsed;
                let pct = iterated as f64 / total as f64 * 100.0;
                let remaining = (total - iterated) as f64 / rate;
                writeln!(
                    progress,
                    "  {}/{} ({:.1}%) | {:.0} it/s | ~{:.0}s remaining | {} matches | cache: {}",
                    iterated,
                    total,
                    pct,
                    rate,
                    remaining,
                    matched,
                    validity_cache.len()
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
        elapsed,
        matched,
        validity_cache.len()
    )
    .unwrap();
    progress.flush().unwrap();
    println!("Done in {:.1?} | {} matches found", elapsed, matched);
}
