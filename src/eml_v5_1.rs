use std::collections::HashMap;
use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use cbfunc::{parse_target, search_length, total_valid, used_variables_in_target};

const DEFAULT_TOLERANCE: f64 = 1e-5;
const DEFAULT_DEPTH: u32 = 12;
const WARN_ITERATIONS: u64 = 1_000_000_000;

fn describe_alphabet(alphabet: &[char]) -> String {
    alphabet.iter().map(|c| {
        match c {
            '0' => "x",
            '1' => "y",
            '2' => "f",
            '3' => "1",
            _ => "?",
        }
    }).collect::<Vec<_>>().join(",")
}

fn print_help() {
    eprintln!("Usage: eml_v5_1 --target <EXPR> [OPTIONS]");
    eprintln!();
    eprintln!("Search for quaternary-encoded expressions equivalent to a target.");
    eprintln!("The alphabet adapts to variables in the target:");
    eprintln!("  x+y  → 0=x, 1=y, 2=f, 3=1    (base 4)");
    eprintln!("  x+1  → 0=x,      2=f, 3=1    (base 3, no y leaf)");
    eprintln!("  42   →      2=f, 3=1          (base 2, no leaves)");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  --target <EXPR>  Target expression. Examples:");
    eprintln!("                     x*y        product of variables");
    eprintln!("                     x+1        x plus 1");
    eprintln!("                     @200       quaternary string (exp(x)-ln(y))");
    eprintln!("  --depth <N>      Max tree depth (default: 12). Mutually exclusive with --time.");
    eprintln!("  --time <SECS>    Max search time in seconds. Mutually exclusive with --depth.");
    eprintln!("  --tol <FLOAT>    Numeric tolerance (default: 1e-5).");
    eprintln!("  --threads <N>    Number of threads (default: all available).");
    eprintln!("  --no-verify       Skip symbolic verification of matches (default: verify via sympy).");
    eprintln!("  --help           Show this help message.");
}

fn verify_matches(
    matches: &[(String, u32)],
    target: &str,
) -> Option<HashMap<String, (String, String)>> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let script = std::path::Path::new(manifest).join("scripts").join("verify_eml.py");
    if !script.exists() {
        eprintln!("warning: verifier script not found at {}", script.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("eml_matches_{}.txt", std::process::id()));
    {
        let f = match std::fs::File::create(&tmp) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("warning: could not create match file: {}", e);
                return None;
            }
        };
        let mut w = std::io::BufWriter::new(f);
        for (s, _) in matches {
            if writeln!(w, "{}", s).is_err() {
                let _ = std::fs::remove_file(&tmp);
                return None;
            }
        }
    }
    let output = Command::new("uv")
        .args([
            "run",
            "--with",
            "sympy",
            "--with",
            "mpmath",
            "python3",
            script.to_str().unwrap_or(""),
            "--target",
            target,
            "--strings-file",
            tmp.to_str().unwrap_or(""),
        ])
        .output();
    let _ = std::fs::remove_file(&tmp);
    let output = match output {
        Ok(o) => o,
        Err(e) => {
            eprintln!("warning: could not run uv/sympy verifier ({}); reporting matches unverified", e);
            return None;
        }
    };
    if !output.status.success() {
        eprintln!(
            "warning: verifier failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = HashMap::new();
    for line in stdout.lines() {
        let mut parts = line.split('\t');
        if let (Some(v), Some(qs)) = (parts.next(), parts.next()) {
            let detail: String = parts.collect::<Vec<_>>().join("\t");
            results.insert(qs.to_string(), (v.to_string(), detail));
        }
    }
    if results.is_empty() {
        eprintln!("warning: verifier produced no verdicts; reporting matches unverified");
        return None;
    }
    Some(results)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() == 1 || args.iter().any(|a| a == "--help") {
        print_help();
        return;
    }

    let mut target_str: Option<String> = None;
    let mut depth: Option<u32> = None;
    let mut max_time: Option<f64> = None;
    let mut threads: Option<usize> = None;
    let mut tolerance: f64 = DEFAULT_TOLERANCE;
    let mut verify: bool = true;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                i += 1;
                if i >= args.len() { eprintln!("error: --target requires a value"); std::process::exit(1); }
                target_str = Some(args[i].clone());
            }
            "--depth" => {
                i += 1;
                if i >= args.len() { eprintln!("error: --depth requires a value"); std::process::exit(1); }
                depth = Some(args[i].parse().unwrap_or_else(|_| { eprintln!("error: invalid depth '{}'", args[i]); std::process::exit(1); }));
            }
            "--time" => {
                i += 1;
                if i >= args.len() { eprintln!("error: --time requires a value"); std::process::exit(1); }
                max_time = Some(args[i].parse().unwrap_or_else(|_| { eprintln!("error: invalid time '{}'", args[i]); std::process::exit(1); }));
            }
            "--tol" => {
                i += 1;
                if i >= args.len() { eprintln!("error: --tol requires a value"); std::process::exit(1); }
                tolerance = args[i].parse().unwrap_or_else(|_| { eprintln!("error: invalid tolerance '{}'", args[i]); std::process::exit(1); });
            }
            "--threads" => {
                i += 1;
                if i >= args.len() { eprintln!("error: --threads requires a value"); std::process::exit(1); }
                threads = Some(args[i].parse().unwrap_or_else(|_| { eprintln!("error: invalid thread count '{}'", args[i]); std::process::exit(1); }));
            }
            "--no-verify" => {
                verify = false;
            }
            _ => {
                eprintln!("error: unknown flag '{}'", args[i]);
                eprintln!("Use --help for usage.");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let target_str = target_str.unwrap_or_else(|| {
        eprintln!("error: --target is required");
        std::process::exit(1);
    });

    if depth.is_some() && max_time.is_some() {
        eprintln!("error: --depth and --time are mutually exclusive");
        std::process::exit(1);
    }

    let use_time_mode = max_time.is_some();
    let depth_val = depth.unwrap_or(DEFAULT_DEPTH);
    let time_val = max_time.unwrap_or(f64::MAX);

    let target = match parse_target(&target_str) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: failed to parse target expression: {}", e);
            std::process::exit(1);
        }
    };

    let (has_x, has_y) = used_variables_in_target(&target);
    let alphabet: Vec<char> = {
        let mut a = Vec::new();
        if has_x { a.push('0'); }
        if has_y { a.push('1'); }
        a.push('3');
        a.push('2');
        a
    };
    let base = alphabet.len() as u64;
    let alphabet_desc = describe_alphabet(&alphabet);

    if let Some(n) = threads {
        rayon::ThreadPoolBuilder::new().num_threads(n).build_global().unwrap_or_else(|e| { eprintln!("warning: thread pool already initialized: {}", e); });
    }

    let max_compute_depth = depth_val;
    let leaves: Vec<char> = alphabet[..alphabet.len() - 1].to_vec();
    let leaf_types = leaves.len() as u64;
    let total_iters: Option<u64> = if use_time_mode {
        None
    } else {
        Some(total_valid(leaf_types, max_compute_depth))
    };
    let num_threads = rayon::current_num_threads();
    let progress = Arc::new(Mutex::new(cbfunc::open_progress_log("progress_v5_1.log").unwrap()));
    {
        let mut pf = progress.lock().unwrap();
        let depth_desc = if use_time_mode { "∞".to_string() } else { depth_val.to_string() };
        let time_desc = if use_time_mode { format!("{}s", time_val) } else { "none".to_string() };
        let valid_iter_desc = total_iters.map(|t| format!(" | valid iter: {}", t)).unwrap_or_default();
        writeln!(*pf, "target: {} | alphabet: [{}] | base: {} | tol: {:.0e} | depth: {} | time: {} | threads: {}{}", target_str, alphabet_desc, base, tolerance, depth_desc, time_desc, num_threads, valid_iter_desc).unwrap();
        pf.flush().unwrap();
    }

    let overall_start = Instant::now();
    let matches_store: Arc<Mutex<Vec<(String, u32)>>> = Arc::new(Mutex::new(Vec::new()));
    let matched = Arc::new(AtomicU64::new(0));

    if use_time_mode {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let iterated = Arc::new(AtomicU64::new(0));

        let (tx, rx) = mpsc::channel::<()>();
        let reporter_handle = {
            let p = progress.clone();
            let sflag = stop_flag.clone();
            let mt = matched.clone();
            let it = iterated.clone();
            std::thread::spawn(move || {
                let mut last_report = Instant::now();
                let mut warned = false;
                loop {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        _ => {}
                    }
                    if sflag.load(Ordering::Relaxed) {
                        break;
                    }
                    let done = it.load(Ordering::Relaxed);
                    if !warned && done > WARN_ITERATIONS {
                        warned = true;
                        eprintln!("warning: {} iterations done, exceeding 1 billion", done);
                        let mut pf = p.lock().unwrap();
                        writeln!(*pf, "  WARNING: {} iterations done, exceeding 1 billion", done).unwrap();
                        pf.flush().unwrap();
                    }
                    let elapsed = overall_start.elapsed().as_secs_f64();
                    if elapsed >= time_val {
                        sflag.store(true, Ordering::Relaxed);
                        let rate = done as f64 / elapsed.max(0.001);
                        let mut pf = p.lock().unwrap();
                        writeln!(*pf, "  {:.5}s | {} iter | {:.0} it/s | {} matches", elapsed, done, rate, mt.load(Ordering::Relaxed)).unwrap();
                        pf.flush().unwrap();
                        break;
                    }
                    if last_report.elapsed() >= Duration::from_secs(1) {
                        last_report = Instant::now();
                        let rate = done as f64 / elapsed.max(0.001);
                        let mut pf = p.lock().unwrap();
                        writeln!(*pf, "  {:.5}s | {} iter | {:.0} it/s | {} matches", elapsed, done, rate, mt.load(Ordering::Relaxed)).unwrap();
                        pf.flush().unwrap();
                    }
                }
            })
        };

        for length in (1u32..).step_by(2) {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }
            search_length(
                length,
                &leaves,
                &target,
                tolerance,
                &stop_flag,
                &matches_store,
                &matched,
                &iterated,
                num_threads,
            );
            {
                let mut pf = progress.lock().unwrap();
                writeln!(*pf, "  depth {} done | elapsed {:.5}s | {} matches", length, overall_start.elapsed().as_secs_f64(), matched.load(Ordering::Relaxed)).unwrap();
                pf.flush().unwrap();
            }
        }

        stop_flag.store(true, Ordering::Relaxed);
        drop(tx);
        reporter_handle.join().unwrap();
    } else {
        let total = total_iters.unwrap();

        {
            let mut pf = progress.lock().unwrap();
            writeln!(*pf, "{} total valid iterations", total).unwrap();
            pf.flush().unwrap();
        }

        let iterated = Arc::new(AtomicU64::new(0));
        let dummy_stop = AtomicBool::new(false);

        let (tx, rx) = mpsc::channel::<()>();
        let reporter_handle = {
            let p = progress.clone();
            let it = iterated.clone();
            let mt = matched.clone();
            std::thread::spawn(move || {
                let mut warned = false;
                loop {
                    match rx.recv_timeout(Duration::from_secs(1)) {
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        _ => {}
                    }
                    let done = it.load(Ordering::Relaxed);
                    if done >= total {
                        break;
                    }
                    if !warned && done > WARN_ITERATIONS {
                        warned = true;
                        eprintln!("warning: {} iterations done, exceeding 1 billion", done);
                        let mut pf = p.lock().unwrap();
                        writeln!(*pf, "  WARNING: {} iterations done, exceeding 1 billion", done).unwrap();
                        pf.flush().unwrap();
                    }
                    let elapsed = overall_start.elapsed().as_secs_f64();
                    let rate = done as f64 / elapsed;
                    let pct = done as f64 / total as f64 * 100.0;
                    let remaining = (total - done) as f64 / rate;
                    let mut pf = p.lock().unwrap();
                    writeln!(*pf, "  {}/{} ({:.1}%) | {:.0} it/s | ~{:.0}s remaining | {} matches", done, total, pct, rate, remaining, mt.load(Ordering::Relaxed)).unwrap();
                    pf.flush().unwrap();
                }
            })
        };

        for length in 1u32..=max_compute_depth {
            if length % 2 == 0 {
                continue;
            }
            search_length(
                length,
                &leaves,
                &target,
                tolerance,
                &dummy_stop,
                &matches_store,
                &matched,
                &iterated,
                num_threads,
            );
            {
                let mut pf = progress.lock().unwrap();
                writeln!(*pf, "  depth {} done | elapsed {:.5}s | {} matches", length, overall_start.elapsed().as_secs_f64(), matched.load(Ordering::Relaxed)).unwrap();
                pf.flush().unwrap();
            }
        }

        drop(tx);
        reporter_handle.join().unwrap();
        {
            let mut pf = progress.lock().unwrap();
            writeln!(*pf, "  all depths done").unwrap();
            pf.flush().unwrap();
        }
    }

    let elapsed = overall_start.elapsed();
    let final_matched = matched.load(Ordering::Relaxed);

    {
        let mut pf = progress.lock().unwrap();
        writeln!(*pf, "Done in {:.5}s | {} matches found", elapsed.as_secs_f64(), final_matched).unwrap();
        pf.flush().unwrap();
    }
    println!("Done in {:.5}s | {} matches found", elapsed.as_secs_f64(), final_matched);

    let matches = matches_store.lock().unwrap();
    if !matches.is_empty() {
        println!("Matches:");
        let verifications = if verify { verify_matches(&matches, &target_str) } else { None };
        if let Some(verifications) = &verifications {
            let mut n_verified = 0;
            let mut n_likely = 0;
            let mut n_other = 0;
            for (s, depth) in matches.iter() {
                let (v, detail) = verifications
                    .get(s)
                    .cloned()
                    .unwrap_or_else(|| ("UNKNOWN".to_string(), "no verdict".to_string()));
                println!("  {} (depth {}) [{}] {}", s, depth, v, detail);
                match v.as_str() {
                    "VERIFIED" => n_verified += 1,
                    "LIKELY" => n_likely += 1,
                    _ => n_other += 1,
                }
            }
            let mut pf = progress.lock().unwrap();
            writeln!(*pf, "  verification: {} VERIFIED, {} LIKELY, {} DIFFERENT/UNKNOWN", n_verified, n_likely, n_other).unwrap();
            pf.flush().unwrap();
        } else {
            for (s, depth) in matches.iter() {
                println!("  {} (depth {})", s, depth);
            }
        }
    }
}
