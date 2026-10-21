/// `Number.prototype.toString` for a finite number: the shortest round-trip decimal, with
/// exponent notation outside 1e-6..1e21. Node and Rust must write identical text for thresholds.
pub(crate) fn js_number_to_string(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let sign = if v < 0.0 { "-" } else { "" };
    let sci = format!("{:e}", v.abs());
    let (mantissa, exp) = sci
        .split_once('e')
        .expect("LowerExp always has an exponent");
    let exp: i32 = exp.parse().expect("LowerExp exponent is an integer");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1;

    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        let (int, frac) = digits.split_at(n as usize);
        format!("{int}.{frac}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let e_sign = if e < 0 { '-' } else { '+' };
        let (first, rest) = digits.split_at(1);
        let mant = if rest.is_empty() {
            first.to_string()
        } else {
            format!("{first}.{rest}")
        };
        format!("{mant}e{e_sign}{}", e.abs())
    };
    format!("{sign}{body}")
}

/// Matches `^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$`.
pub(crate) fn is_numeric_literal(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut i = 0;
    if matches!(b.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let int_digits = digits(i);
    i += int_digits;
    let mut frac_digits = 0;
    let has_dot = b.get(i) == Some(&b'.');
    if has_dot {
        i += 1;
        frac_digits = digits(i);
        i += frac_digits;
    }
    if int_digits == 0 && !(has_dot && frac_digits > 0) {
        return false;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let exp_digits = digits(i);
        if exp_digits == 0 {
            return false;
        }
        i += exp_digits;
    }
    i == b.len()
}
