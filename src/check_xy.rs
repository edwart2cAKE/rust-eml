use std::collections::HashMap;
use std::env;
use rand::Rng;
use cbfunc::{NumExpr, is_valid, parse_num};

const NUM_EVAL_POINTS: usize = 10;
const TOLERANCE: f64 = 1e-10;

fn equals_xy(parsed: &NumExpr, rng: &mut impl Rng) -> bool {
    for _ in 0..NUM_EVAL_POINTS {
        let x: f64 = rng.gen_range(0.1..=10.0);
        let y: f64 = rng.gen_range(0.1..=10.0);
        let p = parsed.eval(x, y);
        let t = x * y;
        if p.is_nan() || (p - t).abs() > TOLERANCE {
            return false;
        }
    }
    true
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <quaternary_string>", args[0]);
        eprintln!("  Characters: 0=x, 1=y, 2=f(exp(a)-ln(b)), 3=const(1)");
        std::process::exit(1);
    }

    let fs = &args[1];

    if fs.is_empty() {
        println!("invalid (empty string)");
        return;
    }

    for ch in fs.chars() {
        if ch != '0' && ch != '1' && ch != '2' && ch != '3' {
            println!("invalid (bad character '{}')", ch);
            return;
        }
    }

    if fs.len() > 1 && !fs.starts_with('2') {
        println!("invalid (multi-char expression must start with '2')");
        return;
    }

    let mut cache = HashMap::new();
    if !is_valid(fs, &mut cache) {
        println!("invalid (unbalanced or structurally invalid)");
        return;
    }

    let parsed = match parse_num(fs) {
        Some(p) => p,
        None => {
            println!("invalid (parse failed despite validity check)");
            return;
        }
    };

    let mut rng = rand::thread_rng();
    let sample_x: f64 = rng.gen_range(0.1..=10.0);
    let sample_y: f64 = rng.gen_range(0.1..=10.0);
    let sample_val = parsed.eval(sample_x, sample_y);
    println!("valid");
    println!("  eval({:.4}, {:.4}) = {:.6}", sample_x, sample_y, sample_val);

    if equals_xy(&parsed, &mut rng) {
        println!("  => EQUALS x * y");
    } else {
        println!("  => does NOT equal x * y");
    }
}
