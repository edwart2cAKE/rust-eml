use rand::Rng;
use std::collections::HashMap;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

fn is_leaf(ch: u8) -> bool {
    ch == b'0' || ch == b'1' || ch == b'3'
}

pub fn split_pair(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    let mut count = 0i32;
    loop {
        if i >= n {
            return None;
        }
        if is_leaf(bytes[i]) {
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
        if is_leaf(bytes[i]) {
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

#[derive(Clone)]
pub enum NumExpr {
    X,
    Y,
    Const(f64),
    Exp(Box<NumExpr>),
    Log(Box<NumExpr>),
    Add(Box<NumExpr>, Box<NumExpr>),
    Sub(Box<NumExpr>, Box<NumExpr>),
}

impl NumExpr {
    pub fn eval(&self, x: f64, y: f64) -> f64 {
        match self {
            NumExpr::X => x,
            NumExpr::Y => y,
            NumExpr::Const(c) => *c,
            NumExpr::Exp(e) => e.eval(x, y).exp(),
            NumExpr::Log(e) => e.eval(x, y).ln(),
            NumExpr::Add(a, b) => a.eval(x, y) + b.eval(x, y),
            NumExpr::Sub(a, b) => a.eval(x, y) - b.eval(x, y),
        }
    }
}

pub fn f_num(a: &NumExpr, b: &NumExpr) -> NumExpr {
    NumExpr::Sub(
        Box::new(NumExpr::Exp(Box::new(a.clone()))),
        Box::new(NumExpr::Log(Box::new(b.clone()))),
    )
}

pub fn is_valid(fs: &str, cache: &mut HashMap<String, bool>) -> bool {
    if let Some(&valid) = cache.get(fs) {
        return valid;
    }
    let valid = if fs == "0" || fs == "1" || fs == "3" {
        true
    } else if fs.starts_with('2') {
        let rest = &fs[1..];
        split_pair(rest).map_or(false, |(a, b)| is_valid(a, cache) && is_valid(b, cache))
    } else {
        false
    };
    if valid {
        cache.insert(fs.to_string(), true);
    }
    valid
}

pub fn parse_num(fs: &str) -> Option<NumExpr> {
    if fs == "0" {
        Some(NumExpr::X)
    } else if fs == "1" {
        Some(NumExpr::Y)
    } else if fs == "3" {
        Some(NumExpr::Const(1.0))
    } else if fs.starts_with('2') {
        let (part1, part2) = split_pair(&fs[1..])?;
        let p1 = parse_num(part1)?;
        let p2 = parse_num(part2)?;
        Some(f_num(&p1, &p2))
    } else {
        None
    }
}

pub fn parse_num_cached(
    fs: &str,
    cache: &mut HashMap<String, Option<NumExpr>>,
) -> Option<NumExpr> {
    if let Some(cached) = cache.get(fs) {
        return cached.clone();
    }
    let result = parse_num(fs);
    if result.is_some() {
        cache.insert(fs.to_string(), result.clone());
    }
    result
}

pub fn equals_num(
    parsed: &NumExpr,
    target_num: &NumExpr,
    rng: &mut impl Rng,
) -> bool {
    for _ in 0..10 {
        let x_val: f64 = rng.gen_range(0.1..=10.0);
        let y_val: f64 = rng.gen_range(0.1..=10.0);
        let p_val = parsed.eval(x_val, y_val);
        let t_val = target_num.eval(x_val, y_val);
        if p_val.is_nan() || t_val.is_nan() || (p_val - t_val).abs() > 1e-10 {
            return false;
        }
    }
    true
}

pub fn open_progress_log(name: &str) -> std::io::Result<std::fs::File> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("logs");
    std::fs::create_dir_all(&dir)?;
    std::fs::File::create(dir.join(name))
}

pub const NUM_EVAL_POINTS: usize = 10;

#[derive(Clone)]
pub enum TargetExpr {
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
    pub fn eval(&self, x: f64, y: f64) -> f64 {
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

pub fn used_variables_in_target(t: &TargetExpr) -> (bool, bool) {
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

pub fn parse_target(s: &str) -> Result<TargetExpr, String> {
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

pub fn fuzzy_equal(parsed: &NumExpr, target: &TargetExpr, rng: &mut impl Rng, tolerance: f64) -> bool {
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

pub fn catalan(k: u64) -> u64 {
    let mut c: u128 = 1;
    for i in 1..=k {
        c = c * (k + i) as u128 / i as u128;
    }
    (c / (k + 1) as u128) as u64
}

pub fn valid_count(leaf_types: u64, k: u64) -> u64 {
    (catalan(k) as u128 * (leaf_types as u128).pow(k as u32 + 1)) as u64
}

pub fn total_valid(leaf_types: u64, max_length: u32) -> u64 {
    let mut total: u64 = 0;
    let mut len = 1u32;
    while len <= max_length {
        total += valid_count(leaf_types, (len - 1) as u64 / 2);
        len += 2;
    }
    total
}

pub fn gen_complete(
    s: &mut Vec<u8>,
    slots: u64,
    budget: u64,
    leaves: &[char],
    emit: &mut impl FnMut(&str) -> bool,
) -> bool {
    if slots == 0 {
        if budget == 0 {
            return emit(std::str::from_utf8(s).unwrap());
        }
        return true;
    }
    if budget == 0 || slots > budget {
        return true;
    }
    s.push(b'2');
    let cont = gen_complete(s, slots + 1, budget - 1, leaves, emit);
    s.pop();
    if !cont {
        return false;
    }
    for &leaf in leaves {
        s.push(leaf as u8);
        let cont = gen_complete(s, slots - 1, budget - 1, leaves, emit);
        s.pop();
        if !cont {
            return false;
        }
    }
    true
}

pub fn build_frontier(
    length: u32,
    leaves: &[char],
    target_frontier: usize,
) -> Vec<(Vec<u8>, u64, u64)> {
    if length == 1 {
        return leaves.iter().map(|&c| (vec![c as u8], 0, 0)).collect();
    }
    let mut work: Vec<(Vec<u8>, u64, u64)> =
        vec![(vec![b'2'], 2u64, (length - 1) as u64)];
    let mut out: Vec<(Vec<u8>, u64, u64)> = Vec::new();
    while out.len() + work.len() < target_frontier {
        let Some(state) = work.pop() else { break };
        if state.1 == 0 {
            out.push(state);
            continue;
        }
        if state.2 == 0 || state.1 > state.2 {
            continue;
        }
        let (prefix, slots, budget) = state;
        {
            let mut p = prefix.clone();
            p.push(b'2');
            let (ns, nb) = (slots + 1, budget - 1);
            if ns == 0 {
                if nb == 0 {
                    out.push((p, 0, 0));
                }
            } else if nb > 0 && ns <= nb {
                work.push((p, ns, nb));
            }
        }
        for &leaf in leaves {
            let mut p = prefix.clone();
            p.push(leaf as u8);
            let (ns, nb) = (slots - 1, budget - 1);
            if ns == 0 {
                if nb == 0 {
                    out.push((p, 0, 0));
                }
            } else if nb > 0 && ns <= nb {
                work.push((p, ns, nb));
            }
        }
    }
    out.extend(work);
    out
}

pub fn search_length(
    length: u32,
    leaves: &[char],
    target: &TargetExpr,
    tol: f64,
    stop: &AtomicBool,
    matches_store: &Mutex<Vec<(String, u32)>>,
    matched: &AtomicU64,
    iterated: &AtomicU64,
    num_threads: usize,
) {
    let frontier = build_frontier(length, leaves, (num_threads * 16).max(1024));
    let mstore = matches_store;
    let mmatched = matched;
    let mit = iterated;
    let sflag = stop;
    let leaves_ref = leaves.to_vec();

    let _: Result<(), ()> = frontier.par_iter().try_for_each(|(prefix, slots, budget)| {
        if sflag.load(Ordering::Relaxed) {
            return Err(());
        }
        let mut rng = StdRng::from_entropy();
        let mut s = prefix.clone();
        gen_complete(&mut s, *slots, *budget, &leaves_ref, &mut |fs| {
            if let Some(parsed) = parse_num(fs) {
                if fuzzy_equal(&parsed, target, &mut rng, tol) {
                    mmatched.fetch_add(1, Ordering::Relaxed);
                    mstore.lock().unwrap().push((fs.to_string(), length));
                }
            }
            mit.fetch_add(1, Ordering::Relaxed);
            !sflag.load(Ordering::Relaxed)
        });
        Ok(())
    });
}
