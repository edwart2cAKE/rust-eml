use rand::Rng;
use std::collections::HashMap;

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
