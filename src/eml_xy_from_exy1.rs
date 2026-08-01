use rand::rngs::StdRng;
use rand::SeedableRng;
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use cbfunc::{NumExpr, is_valid, parse_num};

const DEPTH: u32 = 14;
const NUM_EVAL_POINTS: usize = 10;
const TOLERANCE: f64 = 1e-10;

fn get_quaternary(num: u64) -> String {
    let mut s = String::new();
    let mut n = num;
    s.push(char::from_digit((n % 4) as u32, 10).unwrap());
    n /= 4;
    while n > 0 {
        s.push(char::from_digit((n % 4) as u32, 10).unwrap());
        n /= 4;
    }
    s.chars().rev().collect()
}

fn equals_target(parsed: &NumExpr, rng: &mut impl rand::Rng) -> bool {
    for _ in 0..NUM_EVAL_POINTS {
        let x_val: f64 = rng.gen_range(0.1..=10.0);
        let y_val: f64 = rng.gen_range(0.1..=10.0);
        let p_val = parsed.eval(x_val, y_val);
        let t_val = x_val * y_val;
        if p_val.is_nan() || (p_val - t_val).abs() > TOLERANCE {
            return false;
        }
    }
    true
}

fn main() {
    let target_desc = "x * y";
    let total = 4_u64.pow(DEPTH);

    let matches_store: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let iterated = Arc::new(AtomicU64::new(0));
    let matched = Arc::new(AtomicU64::new(0));

    let num_threads = rayon::current_num_threads() as u64;
    let num_chunks = (num_threads * 4).max(1);
    let chunk_size = total / num_chunks;
    let ranges: Vec<(u64, u64)> = (0..num_chunks)
        .map(|c| {
            let start = c * chunk_size;
            let end = if c == num_chunks - 1 { total } else { start + chunk_size };
            (start, end)
        })
        .collect();

    let mut progress = cbfunc::open_progress_log("progress_eml_xy.log").unwrap();
    writeln!(
        progress,
        "DEPTH={} | TARGET={} | {} total iterations | {} threads",
        DEPTH, target_desc, total, rayon::current_num_threads()
    )
    .unwrap();
    progress.flush().unwrap();

    let reporter_iterated = iterated.clone();
    let reporter_matched = matched.clone();
    let overall_start = Instant::now();
    let progress_file = Arc::new(Mutex::new(progress));

    {
        let progress_file = progress_file.clone();
        let reporter = std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            let done = reporter_iterated.load(Ordering::Relaxed);
            if done >= total {
                break;
            }
            let elapsed = overall_start.elapsed().as_secs_f64();
            let rate = done as f64 / elapsed;
            let pct = done as f64 / total as f64 * 100.0;
            let remaining = (total - done) as f64 / rate;
            let mut pf = progress_file.lock().unwrap();
            writeln!(
                *pf,
                "  {}/{} ({:.1}%) | {:.0} it/s | ~{:.0}s remaining | {} matches",
                done, total, pct, rate, remaining, reporter_matched.load(Ordering::Relaxed)
            )
            .unwrap();
            pf.flush().unwrap();
        });

        let mstore = matches_store.clone();
        let mit = iterated.clone();
        let mmatched = matched.clone();

        ranges.par_iter().for_each(|&(start, end)| {
            let mut cache: HashMap<String, bool> = HashMap::new();
            let mut rng = StdRng::from_entropy();
            let mut local_matched = 0u64;

            for i in start..end {
                let fs = get_quaternary(i);
                if fs.len() > 1 && !fs.starts_with('2') {
                    mit.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                if is_valid(&fs, &mut cache) {
                    if let Some(parsed) = parse_num(&fs) {
                        if equals_target(&parsed, &mut rng) {
                            local_matched += 1;
                            mstore.lock().unwrap().push(fs);
                        }
                    }
                }
                mit.fetch_add(1, Ordering::Relaxed);
            }

            mmatched.fetch_add(local_matched, Ordering::Relaxed);
        });

        reporter.join().unwrap();
    }

    let elapsed = overall_start.elapsed();
    let final_matched = matched.load(Ordering::Relaxed);
    let final_iterated = iterated.load(Ordering::Relaxed);

    {
        let mut pf = progress_file.lock().unwrap();
        writeln!(
            *pf,
            "Done in {:.1?} | {} matches found | {} iterated",
            elapsed, final_matched, final_iterated
        )
        .unwrap();
        pf.flush().unwrap();
    }

    println!("Done in {:.1?} | {} matches found", elapsed, final_matched);

    let matches = matches_store.lock().unwrap();
    if !matches.is_empty() {
        println!("Matches:");
        for m in matches.iter() {
            println!("  {}", m);
        }
    }
}
