use std::collections::HashMap;
use std::io::Write;
use std::time::Instant;
use cbfunc::{NumExpr, parse_num_cached, equals_num};

const DEPTH: u32 = 20;

fn get_ternary(num: u64) -> String {
    let mut s = String::new();
    let mut n = num;
    s.push(char::from_digit((n % 3) as u32, 10).unwrap());
    n /= 3;
    while n > 0 {
        s.push(char::from_digit((n % 3) as u32, 10).unwrap());
        n /= 3;
    }
    s.chars().rev().collect()
}

fn main() {
    let target_num = NumExpr::Add(
        Box::new(NumExpr::X),
        Box::new(NumExpr::Y),
    );

    let mut cache: HashMap<String, Option<NumExpr>> = HashMap::new();
    let mut rng = rand::thread_rng();

    let total = 3_u64.pow(DEPTH);

    let mut progress = std::fs::File::create("progress.log").unwrap();
    writeln!(progress, "DEPTH={} | {} total iterations | searching for x + y", DEPTH, total).unwrap();
    progress.flush().unwrap();

    let overall_start = Instant::now();
    let mut matched = 0u64;

    for i in 0..total {
        let fs = get_ternary(i);
        if fs.len() > 1 && !fs.starts_with('2') {
            continue;
        }
        if let Some(parsed) = parse_num_cached(&fs, &mut cache) {
            if equals_num(&parsed, &target_num, &mut rng) {
                matched += 1;
                writeln!(progress, "MATCH: {}", fs).unwrap();
                progress.flush().unwrap();
                println!("MATCH: {}", fs);
                std::io::stdout().flush().unwrap();
            }
        }
        if i % 10_000_000 == 0 && i > 0 {
            let elapsed = overall_start.elapsed().as_secs_f64();
            let rate = i as f64 / elapsed;
            let pct = i as f64 / total as f64 * 100.0;
            let remaining = (total - i) as f64 / rate;
            writeln!(
                progress,
                "  {}/{} ({:.1}%) | {:.0} it/s | ~{:.0}s remaining | {} matches | cache: {}",
                i, total, pct, rate, remaining, matched, cache.len()
            ).unwrap();
            progress.flush().unwrap();
        }
    }

    let elapsed = overall_start.elapsed();
    writeln!(
        progress,
        "Done in {:.1?} | {} matches found | cache size: {}",
        elapsed, matched, cache.len()
    ).unwrap();
    progress.flush().unwrap();
    println!("Done in {:.1?} | {} matches found", elapsed, matched);
}
