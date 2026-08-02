use rand::rngs::StdRng;
use rand::SeedableRng;
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use cbfunc::{NumExpr, is_valid, parse_num};

const DEFAULT_TOLERANCE: f64 = 1e-5;
const NUM_EVAL_POINTS: usize = 10;
const MAX_DEPTH: u32 = 20;
const DEFAULT_DEPTH: u32 = 12;

#[derive(Clone)]
enum TargetExpr {
    X,
    Y,
    Const(f64),
    Add(Box<TargetExpr>, Box<TargetExpr>),
    Sub(Box<TargetExpr>, Box<TargetExpr>),
    Mul(Box<TargetExpr>, Box<TargetExpr>),
    Div(Box<TargetExpr>, Box<TargetExpr>),
    Pow(Box<TargetExpr>, Box<TargetExpr>),
    Exp(Box<TargetExpr>),
    Ln(Box<TargetExpr>),
    Neg(Box<TargetExpr>),
    Quaternary(NumExpr),
}

impl TargetExpr {
    fn eval(&self, x: f64, y: f64) -> f64 {
        match self {
            TargetExpr::X => x,
            TargetExpr::Y => y,
            TargetExpr::Const(c) => *c,
            TargetExpr::Add(a, b) => a.eval(x, y) + b.eval(x, y),
            TargetExpr::Sub(a, b) => a.eval(x, y) - b.eval(x, y),
            TargetExpr::Mul(a, b) => a.eval(x, y) * b.eval(x, y),
            TargetExpr::Div(a, b) => a.eval(x, y) / b.eval(x, y),
            TargetExpr::Pow(a, b) => a.eval(x, y).powf(b.eval(x, y)),
            TargetExpr::Exp(e) => e.eval(x, y).exp(),
            TargetExpr::Ln(e) => e.eval(x, y).ln(),
            TargetExpr::Neg(e) => -e.eval(x, y),
            TargetExpr::Quaternary(n) => n.eval(x, y),
        }
    }
}

fn used_variables_in_target(t: &TargetExpr) -> (bool, bool) {
    match t {
        TargetExpr::X => (true, false),
        TargetExpr::Y => (false, true),
        TargetExpr::Const(_) => (false, false),
        TargetExpr::Add(a, b) | TargetExpr::Sub(a, b) | TargetExpr::Mul(a, b) | TargetExpr::Div(a, b) | TargetExpr::Pow(a, b) => {
            let (ax, ay) = used_variables_in_target(a);
            let (bx, by) = used_variables_in_target(b);
            (ax || bx, ay || by)
        }
        TargetExpr::Exp(e) | TargetExpr::Ln(e) | TargetExpr::Neg(e) => used_variables_in_target(e),
        TargetExpr::Quaternary(n) => used_variables_in_num(n),
    }
}

fn used_variables_in_num(n: &NumExpr) -> (bool, bool) {
    match n {
        NumExpr::X => (true, false),
        NumExpr::Y => (false, true),
        NumExpr::Const(_) => (false, false),
        NumExpr::Exp(e) | NumExpr::Log(e) => used_variables_in_num(e),
        NumExpr::Add(a, b) | NumExpr::Sub(a, b) => {
            let (ax, ay) = used_variables_in_num(a);
            let (bx, by) = used_variables_in_num(b);
            (ax || bx, ay || by)
        }
    }
}

fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && bytes[*pos] == b' ' {
        *pos += 1;
    }
}

fn parse_number(s: &str, bytes: &[u8], pos: &mut usize) -> Result<f64, String> {
    let start = *pos;
    if *pos < bytes.len() && bytes[*pos] == b'-' {
        *pos += 1;
    }
    if *pos >= bytes.len() || (!bytes[*pos].is_ascii_digit() && bytes[*pos] != b'.') {
        return Err("expected number".to_string());
    }
    while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
        *pos += 1;
    }
    if *pos < bytes.len() && bytes[*pos] == b'.' {
        *pos += 1;
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() {
            *pos += 1;
        }
    }
    if *pos == start || (*pos == start + 1 && bytes[start] == b'-') {
        return Err("expected number".to_string());
    }
    s[start..*pos].parse::<f64>().map_err(|_| "invalid number".to_string())
}

fn parse_primary(s: &str, bytes: &[u8], pos: &mut usize) -> Result<TargetExpr, String> {
    skip_ws(bytes, pos);
    if *pos >= bytes.len() {
        return Err("unexpected end of expression".to_string());
    }
    match bytes[*pos] {
        b'x' => { *pos += 1; Ok(TargetExpr::X) }
        b'y' => { *pos += 1; Ok(TargetExpr::Y) }
        b'(' => {
            *pos += 1;
            let expr = parse_expr(s, bytes, pos)?;
            skip_ws(bytes, pos);
            if *pos >= bytes.len() || bytes[*pos] != b')' {
                return Err("expected ')'".to_string());
            }
            *pos += 1;
            Ok(expr)
        }
        b'@' => {
            *pos += 1;
            let start = *pos;
            while *pos < bytes.len() && bytes[*pos] >= b'0' && bytes[*pos] <= b'3' {
                *pos += 1;
            }
            if *pos == start {
                return Err("expected quaternary string after '@'".to_string());
            }
            let qs = &s[start..*pos];
            let mut cache = HashMap::new();
            if !is_valid(qs, &mut cache) {
                return Err(format!("invalid quaternary string: '{}'", qs));
            }
            match parse_num(qs) {
                Some(n) => Ok(TargetExpr::Quaternary(n)),
                None => Err(format!("failed to parse quaternary string: '{}'", qs)),
            }
        }
        _ => {
            let n = parse_number(s, bytes, pos)?;
            Ok(TargetExpr::Const(n))
        }
    }
}

fn parse_unary(s: &str, bytes: &[u8], pos: &mut usize) -> Result<TargetExpr, String> {
    skip_ws(bytes, pos);
    if *pos >= bytes.len() {
        return Err("unexpected end".to_string());
    }
    if *pos + 3 <= bytes.len() && &s[*pos..*pos + 3] == "exp" {
        *pos += 3;
        skip_ws(bytes, pos);
        if *pos >= bytes.len() || bytes[*pos] != b'(' {
            return Err("expected '(' after exp".to_string());
        }
        *pos += 1;
        let e = parse_expr(s, bytes, pos)?;
        skip_ws(bytes, pos);
        if *pos >= bytes.len() || bytes[*pos] != b')' {
            return Err("expected ')' after exp".to_string());
        }
        *pos += 1;
        Ok(TargetExpr::Exp(Box::new(e)))
    } else if *pos + 2 <= bytes.len() && &s[*pos..*pos + 2] == "ln" {
        *pos += 2;
        skip_ws(bytes, pos);
        if *pos >= bytes.len() || bytes[*pos] != b'(' {
            return Err("expected '(' after ln".to_string());
        }
        *pos += 1;
        let e = parse_expr(s, bytes, pos)?;
        skip_ws(bytes, pos);
        if *pos >= bytes.len() || bytes[*pos] != b')' {
            return Err("expected ')' after ln".to_string());
        }
        *pos += 1;
        Ok(TargetExpr::Ln(Box::new(e)))
    } else if bytes[*pos] == b'-' {
        *pos += 1;
        let e = parse_unary(s, bytes, pos)?;
        Ok(TargetExpr::Neg(Box::new(e)))
    } else {
        parse_primary(s, bytes, pos)
    }
}

fn parse_power(s: &str, bytes: &[u8], pos: &mut usize) -> Result<TargetExpr, String> {
    let mut left = parse_unary(s, bytes, pos)?;
    loop {
        skip_ws(bytes, pos);
        if *pos < bytes.len() && bytes[*pos] == b'^' {
            *pos += 1;
            let right = parse_power(s, bytes, pos)?;
            left = TargetExpr::Pow(Box::new(left), Box::new(right));
        } else {
            break;
        }
    }
    Ok(left)
}

fn parse_term(s: &str, bytes: &[u8], pos: &mut usize) -> Result<TargetExpr, String> {
    let mut left = parse_power(s, bytes, pos)?;
    loop {
        skip_ws(bytes, pos);
        if *pos >= bytes.len() { break; }
        match bytes[*pos] {
            b'*' => {
                *pos += 1;
                let right = parse_power(s, bytes, pos)?;
                left = TargetExpr::Mul(Box::new(left), Box::new(right));
            }
            b'/' => {
                *pos += 1;
                let right = parse_power(s, bytes, pos)?;
                left = TargetExpr::Div(Box::new(left), Box::new(right));
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_expr(s: &str, bytes: &[u8], pos: &mut usize) -> Result<TargetExpr, String> {
    let mut left = parse_term(s, bytes, pos)?;
    loop {
        skip_ws(bytes, pos);
        if *pos >= bytes.len() { break; }
        match bytes[*pos] {
            b'+' => {
                *pos += 1;
                let right = parse_term(s, bytes, pos)?;
                left = TargetExpr::Add(Box::new(left), Box::new(right));
            }
            b'-' => {
                *pos += 1;
                let right = parse_term(s, bytes, pos)?;
                left = TargetExpr::Sub(Box::new(left), Box::new(right));
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_target(s: &str) -> Result<TargetExpr, String> {
    let bytes = s.as_bytes();
    let mut pos = 0;
    let result = parse_expr(s, bytes, &mut pos)?;
    skip_ws(bytes, &mut pos);
    if pos != bytes.len() {
        let ch = s[pos..].chars().next().unwrap_or('?');
        return Err(format!("unexpected character '{}' at position {}", ch, pos));
    }
    Ok(result)
}

fn fuzzy_equal(parsed: &NumExpr, target: &TargetExpr, rng: &mut impl rand::Rng, tolerance: f64) -> bool {
    for _ in 0..NUM_EVAL_POINTS {
        let x_val: f64 = rng.gen_range(0.1..=10.0);
        let y_val: f64 = rng.gen_range(0.1..=10.0);
        let p_val = parsed.eval(x_val, y_val);
        let t_val = target.eval(x_val, y_val);
        if p_val.is_nan() || t_val.is_nan() || (p_val - t_val).abs() > tolerance {
            return false;
        }
    }
    true
}

fn get_alphabet_string(mut i: u64, alphabet: &[char]) -> String {
    let base = alphabet.len() as u64;
    let mut fs = String::new();
    fs.push(alphabet[(i % base) as usize]);
    i /= base;
    while i > 0 {
        fs.push(alphabet[(i % base) as usize]);
        i /= base;
    }
    fs.chars().rev().collect()
}

fn depth_first_index(length: u32, base: u64) -> u64 {
    if length == 1 { 0 } else { base.pow(length - 1) }
}

fn depth_count(length: u32, base: u64) -> u64 {
    base.pow(length) - depth_first_index(length, base)
}

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
    eprintln!("Usage: eml_v5_0 --target <EXPR> [OPTIONS]");
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
    eprintln!("  --help           Show this help message.");
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

    let max_compute_depth = if use_time_mode { MAX_DEPTH } else { depth_val };
    let total_iters: u64 = (1..=max_compute_depth).map(|l| depth_count(l, base)).sum();
    let num_threads = rayon::current_num_threads();
    let progress = Arc::new(Mutex::new(cbfunc::open_progress_log("progress_v5.log").unwrap()));
    {
        let mut pf = progress.lock().unwrap();
        let depth_desc = if use_time_mode { "∞".to_string() } else { depth_val.to_string() };
        let time_desc = if use_time_mode { format!("{}s", time_val) } else { "none".to_string() };
        writeln!(*pf, "target: {} | alphabet: [{}] | base: {} | tol: {:.0e} | depth: {} | time: {} | threads: {} | iter: {}", target_str, alphabet_desc, base, tolerance, depth_desc, time_desc, num_threads, total_iters).unwrap();
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
                loop {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        _ => {}
                    }
                    if sflag.load(Ordering::Relaxed) {
                        break;
                    }
                    let elapsed = overall_start.elapsed().as_secs_f64();
                    if elapsed >= time_val {
                        sflag.store(true, Ordering::Relaxed);
                        let done = it.load(Ordering::Relaxed);
                        let rate = done as f64 / elapsed.max(0.001);
                        let mut pf = p.lock().unwrap();
                        writeln!(*pf, "  {:.0}s | {}/{} iter | {:.0} it/s | {} matches", elapsed, done, total_iters, rate, mt.load(Ordering::Relaxed)).unwrap();
                        pf.flush().unwrap();
                        break;
                    }
                    if last_report.elapsed() >= Duration::from_secs(1) {
                        last_report = Instant::now();
                        let done = it.load(Ordering::Relaxed);
                        let rate = done as f64 / elapsed.max(0.001);
                        let mut pf = p.lock().unwrap();
                        writeln!(*pf, "  {:.0}s | {}/{} iter | {:.0} it/s | {} matches", elapsed, done, total_iters, rate, mt.load(Ordering::Relaxed)).unwrap();
                        pf.flush().unwrap();
                    }
                }
            })
        };

        for length in 1u32..=max_compute_depth {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }
            let first = depth_first_index(length, base);
            let count = depth_count(length, base);
            let num_chunks = (num_threads as u64 * 4).max(1);
            let chunk_size = count / num_chunks;
            let ranges: Vec<(u64, u64)> = (0..num_chunks)
                .map(|c| {
                    let start = first + c * chunk_size;
                    let end = if c == num_chunks - 1 { first + count } else { start + chunk_size };
                    (start, end)
                })
                .collect();

            let mstore = matches_store.clone();
            let mmatched_par = matched.clone();
            let mit = iterated.clone();
            let sflag = stop_flag.clone();
            let alphabet_ref: Arc<[char]> = alphabet.clone().into();
            let tol = tolerance;

            let _: Result<(), ()> = ranges.par_iter().try_for_each(|&(start, end)| {
                let mut cache: HashMap<String, bool> = HashMap::new();
                let mut rng = StdRng::from_entropy();

                for i in start..end {
                    if sflag.load(Ordering::Relaxed) {
                        return Err(());
                    }
                    let fs = get_alphabet_string(i, &alphabet_ref);
                    if fs.len() > 1 && !fs.starts_with('2') {
                        mit.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    if is_valid(&fs, &mut cache) {
                        if let Some(parsed) = parse_num(&fs) {
                            if fuzzy_equal(&parsed, &target, &mut rng, tol) {
                                mmatched_par.fetch_add(1, Ordering::Relaxed);
                                mstore.lock().unwrap().push((fs, length));
                            }
                        }
                    }
                    mit.fetch_add(1, Ordering::Relaxed);
                }
                Ok(())
            });

            {
                let mut pf = progress.lock().unwrap();
                writeln!(*pf, "  depth {} done | elapsed {:.1}s | {} matches", length, overall_start.elapsed().as_secs_f64(), matched.load(Ordering::Relaxed)).unwrap();
                pf.flush().unwrap();
            }
        }

        stop_flag.store(true, Ordering::Relaxed);
        drop(tx);
        reporter_handle.join().unwrap();
    } else {
        let mut total: u64 = 0;
        for l in 1..=max_compute_depth {
            total += depth_count(l, base);
        }

        {
            let mut pf = progress.lock().unwrap();
            writeln!(*pf, "{} total iterations", total).unwrap();
            pf.flush().unwrap();
        }

        let iterated = Arc::new(AtomicU64::new(0));

        let (tx, rx) = mpsc::channel::<()>();
        let reporter_handle = {
            let p = progress.clone();
            let it = iterated.clone();
            let mt = matched.clone();
            std::thread::spawn(move || loop {
                match rx.recv_timeout(Duration::from_secs(1)) {
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    _ => {}
                }
                let done = it.load(Ordering::Relaxed);
                if done >= total {
                    break;
                }
                let elapsed = overall_start.elapsed().as_secs_f64();
                let rate = done as f64 / elapsed;
                let pct = done as f64 / total as f64 * 100.0;
                let remaining = (total - done) as f64 / rate;
                let mut pf = p.lock().unwrap();
                writeln!(*pf, "  {}/{} ({:.1}%) | {:.0} it/s | ~{:.0}s remaining | {} matches", done, total, pct, rate, remaining, mt.load(Ordering::Relaxed)).unwrap();
                pf.flush().unwrap();
            })
        };

        for length in 1u32..=max_compute_depth {
            let first = depth_first_index(length, base);
            let count = depth_count(length, base);
            let num_chunks = (num_threads as u64 * 4).max(1);
            let chunk_size = count / num_chunks;
            let ranges: Vec<(u64, u64)> = (0..num_chunks)
                .map(|c| {
                    let start = first + c * chunk_size;
                    let end = if c == num_chunks - 1 { first + count } else { start + chunk_size };
                    (start, end)
                })
                .collect();

            let mstore = matches_store.clone();
            let mmatched_par = matched.clone();
            let mit = iterated.clone();
            let alphabet_ref: Arc<[char]> = alphabet.clone().into();
            let tol = tolerance;

            ranges.par_iter().for_each(|&(start, end)| {
                let mut cache: HashMap<String, bool> = HashMap::new();
                let mut rng = StdRng::from_entropy();

                for i in start..end {
                    let fs = get_alphabet_string(i, &alphabet_ref);
                    if fs.len() > 1 && !fs.starts_with('2') {
                        mit.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    if is_valid(&fs, &mut cache) {
                        if let Some(parsed) = parse_num(&fs) {
                            if fuzzy_equal(&parsed, &target, &mut rng, tol) {
                                mmatched_par.fetch_add(1, Ordering::Relaxed);
                                mstore.lock().unwrap().push((fs, length));
                            }
                        }
                    }
                    mit.fetch_add(1, Ordering::Relaxed);
                }
            });
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
        writeln!(*pf, "Done in {:.1?} | {} matches found", elapsed, final_matched).unwrap();
        pf.flush().unwrap();
    }
    println!("Done in {:.1?} | {} matches found", elapsed, final_matched);

    let matches = matches_store.lock().unwrap();
    if !matches.is_empty() {
        println!("Matches:");
        for (s, depth) in matches.iter() {
            println!("  {} (depth {})", s, depth);
        }
    }
}
