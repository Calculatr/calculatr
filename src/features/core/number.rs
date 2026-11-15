use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

pub type Rational = BigRational;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NumFmt {
    Decimal,
    Fraction,
}

const MAX_EXPONENT: i64 = 4096;
const MAX_BITS: u64 = 1 << 17;

pub fn int(n: i64) -> Rational {
    Rational::from_integer(BigInt::from(n))
}

fn gcd(a: &BigInt, b: &BigInt) -> BigInt {
    let (mut a, mut b) = (a.abs(), b.abs());
    if a < b {
        std::mem::swap(&mut a, &mut b);
    }
    while !b.is_zero() {
        let r = &a % &b;
        a = b;
        b = r;
    }
    a
}

fn div_exact(n: &BigInt, d: &BigInt) -> BigInt {
    if d.is_one() { n.clone() } else { n / d }
}

pub fn mul(a: &Rational, b: &Rational) -> Rational {
    if a.is_zero() || b.is_zero() {
        return Rational::zero();
    }
    if a.denom().is_one() && b.denom().is_one() {
        return Rational::from_integer(a.numer() * b.numer());
    }
    let g1 = gcd(a.numer(), b.denom());
    let g2 = gcd(b.numer(), a.denom());
    let numer = div_exact(a.numer(), &g1) * div_exact(b.numer(), &g2);
    let denom = div_exact(a.denom(), &g2) * div_exact(b.denom(), &g1);
    Rational::new_raw(numer, denom)
}

pub fn add(a: &Rational, b: &Rational) -> Rational {
    if a.denom().is_one() && b.denom().is_one() {
        return Rational::from_integer(a.numer() + b.numer());
    }
    let g = gcd(a.denom(), b.denom());
    if g.is_one() {
        let numer = a.numer() * b.denom() + b.numer() * a.denom();
        return Rational::new_raw(numer, a.denom() * b.denom());
    }
    let (ad, bd) = (a.denom() / &g, b.denom() / &g);
    let t = a.numer() * &bd + b.numer() * &ad;
    if t.is_zero() {
        return Rational::zero();
    }
    let g2 = gcd(&t, &g);
    Rational::new_raw(div_exact(&t, &g2), ad * div_exact(b.denom(), &g2))
}

pub fn parse_decimal(s: &str) -> Option<Rational> {
    let (int_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    let digits = format!("{int_part}{frac_part}");
    if !digits.bytes().all(|b| b.is_ascii_digit()) || frac_part.contains('.') {
        return None;
    }
    let numer: BigInt = digits.parse().ok()?;
    let denom = BigInt::from(10u32).pow(frac_part.len() as u32);
    Some(Rational::new(numer, denom))
}

pub fn to_int_exact(x: f64) -> Option<i64> {
    if x.is_finite() && x.fract() == 0.0 && x >= i64::MIN as f64 && x <= i64::MAX as f64 {
        Some(x as i64)
    } else {
        None
    }
}

pub fn to_f64(r: &Rational) -> f64 {
    r.to_f64().unwrap_or(f64::NAN)
}

pub fn from_f64(v: f64) -> Option<Rational> {
    Rational::from_float(v)
}

pub fn is_integer(r: &Rational) -> bool {
    r.denom().is_one()
}

pub(crate) fn decimal_digits(r: &Rational) -> Option<u32> {
    let twos = r.denom().trailing_zeros().unwrap_or(0);
    let mut d = r.denom() >> twos;
    let mut fives = 0u64;
    let chunk = BigInt::from(5u64.pow(27));
    while (&d % &chunk).is_zero() {
        d /= &chunk;
        fives += 27;
    }
    let five = BigInt::from(5u32);
    while (&d % &five).is_zero() {
        d /= &five;
        fives += 1;
    }
    d.is_one().then_some(twos.max(fives) as u32)
}

pub(crate) const MAX_EXACT_DIGITS: u32 = 15;

pub fn to_decimal_string(r: &Rational) -> String {
    if is_integer(r) {
        return if r.numer().bits() <= 128 {
            r.numer().to_string()
        } else if to_f64(r).is_finite() {
            fmt_f64(to_f64(r))
        } else {
            fmt_huge_integer(r.numer())
        };
    }
    match decimal_digits(r) {
        Some(digits) if digits <= MAX_EXACT_DIGITS && r.numer().bits() <= 128 => {
            let scale = BigInt::from(10u32).pow(digits);
            let scaled = (r.abs() * Rational::from_integer(scale)).to_integer();
            let mut s = scaled.to_string();
            if digits as usize >= s.len() {
                s = format!("{}{}", "0".repeat(digits as usize - s.len() + 1), s);
            }
            let split = s.len() - digits as usize;
            let sign = if r.is_negative() { "-" } else { "" };
            format!("{sign}{}.{}", &s[..split], &s[split..])
        }
        _ => match to_f64(r) {
            v if v == 0.0 || !v.is_finite() => fmt_huge_rational(r),
            v => fmt_f64(v),
        },
    }
}

pub(crate) const MAX_DISPLAY_BITS: u64 = 1000;

fn leading_digits(n: &BigInt) -> (u64, usize) {
    let n = n.abs();
    let approx = (n.bits().saturating_sub(1) as f64 * std::f64::consts::LOG10_2) as u32;
    let shift = approx.saturating_sub(16);
    let head = (&n / BigInt::from(10u32).pow(shift)).to_string();
    let lead = head.get(..14).and_then(|s| s.parse().ok()).unwrap_or(0);
    (lead, head.len() + shift as usize)
}

pub(crate) fn digit_count(n: &BigInt) -> usize {
    if n.bits() <= 128 {
        n.abs().to_string().len()
    } else {
        leading_digits(n).1
    }
}

pub fn int_text(n: &BigInt, exact: bool) -> String {
    if exact || n.bits() <= MAX_DISPLAY_BITS {
        n.to_string()
    } else {
        fmt_huge_integer(n)
    }
}

fn leading_digits_ratio(r: &Rational) -> (u64, i64) {
    let (n, d) = (r.numer().abs(), r.denom());
    let e = digit_count(&n) as i64 - digit_count(d) as i64;
    let shift = e - 15;
    let p = BigInt::from(10u32).pow(shift.unsigned_abs() as u32);
    let q = if shift >= 0 { n / (d * p) } else { n * p / d };
    let head = q.to_string();
    let lead = head.get(..14).and_then(|s| s.parse().ok()).unwrap_or(0);
    (lead, head.len() as i64 - 1 + shift)
}

fn fmt_huge_integer(n: &BigInt) -> String {
    let (lead, len) = leading_digits(n);
    fmt_sci(n.is_negative(), lead, len as i64 - 1)
}

fn fmt_huge_rational(r: &Rational) -> String {
    let (lead, exponent) = leading_digits_ratio(r);
    fmt_sci(r.is_negative(), lead, exponent)
}

fn fmt_sci(negative: bool, lead: u64, mut exponent: i64) -> String {
    let mut mantissa = (lead + 5) / 10;
    if mantissa >= 10_000_000_000_000 {
        mantissa /= 10;
        exponent += 1;
    }
    let m = mantissa.to_string();
    let frac = m[1..].trim_end_matches('0');
    let sign = if negative { "-" } else { "" };
    if frac.is_empty() {
        format!("{sign}{}e{exponent}", &m[..1])
    } else {
        format!("{sign}{}.{frac}e{exponent}", &m[..1])
    }
}

pub fn fmt_f64(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v == 0.0 {
        return "0".into();
    }
    let rounded: f64 = format!("{v:.12e}").parse().unwrap_or(v);
    let a = rounded.abs();
    if !(1e-9..1e15).contains(&a) {
        format!("{rounded:e}")
    } else {
        format!("{rounded}")
    }
}

const MAX_FACTORIAL: u64 = 10_000;

pub fn factorial(r: &Rational) -> Option<Rational> {
    if !is_integer(r) || r.is_negative() {
        return None;
    }
    let n = r.numer().to_u64()?;
    if n > MAX_FACTORIAL {
        return None;
    }
    Some(Rational::from_integer(range_product(2, n)))
}

fn range_product(lo: u64, hi: u64) -> BigInt {
    if lo > hi {
        return BigInt::one();
    }
    if hi - lo < 32 {
        return (lo..=hi).fold(BigInt::one(), |acc, i| acc * i);
    }
    let mid = lo + (hi - lo) / 2;
    range_product(lo, mid) * range_product(mid + 1, hi)
}

pub fn pow_int(base: &Rational, exp: &Rational) -> Option<Rational> {
    if !is_integer(exp) {
        return None;
    }
    let e = exp.numer().to_i64()?;
    if e.abs() > MAX_EXPONENT {
        return None;
    }
    if base.is_zero() && e < 0 {
        return None;
    }
    if base.is_zero() || base.is_one() {
        return Some(if e == 0 {
            Rational::one()
        } else {
            base.clone()
        });
    }
    if (base.numer().bits() + base.denom().bits()).saturating_mul(e.unsigned_abs()) > MAX_BITS {
        return None;
    }
    let p = e.unsigned_abs().min(u32::MAX as u64) as u32;
    let n = base.numer().pow(p);
    let d = base.denom().pow(p);
    let r = Rational::new_raw(n, d);
    Some(if e >= 0 { r } else { r.recip() })
}

pub fn sqrt_exact(r: &Rational) -> Option<Rational> {
    if r.is_negative() {
        return None;
    }
    let n = r.numer().sqrt();
    let d = r.denom().sqrt();
    if &(&n * &n) == r.numer() && &(&d * &d) == r.denom() {
        Some(Rational::new(n, d))
    } else {
        None
    }
}

pub fn cbrt_exact(r: &Rational) -> Option<Rational> {
    let is_neg = r.is_negative();
    let num = r.numer().abs();
    let den = r.denom();

    let n = num.cbrt();
    let d = den.cbrt();

    if &(&n * &n * &n) == &num && &(&d * &d * &d) == den {
        let res = Rational::new(n, d.clone());
        Some(if is_neg { -res } else { res })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> Rational {
        parse_decimal(s).unwrap()
    }

    #[test]
    fn decimal_literals_are_exact() {
        assert_eq!(r("0.25"), Rational::new(1.into(), 4.into()));
        assert_eq!(r(".5"), Rational::new(1.into(), 2.into()));
        assert_eq!(r("12"), int(12));
        assert!(parse_decimal("1.2.3").is_none());
        assert!(parse_decimal(".").is_none());
    }

    #[test]
    fn decimal_strings() {
        assert_eq!(to_decimal_string(&r("0.25")), "0.25");
        assert_eq!(to_decimal_string(&-r("0.05")), "-0.05");
        assert_eq!(to_decimal_string(&int(7)), "7");
        assert_eq!(
            to_decimal_string(&Rational::new(1.into(), 3.into())),
            "0.3333333333333"
        );
    }

    #[test]
    fn float_formatting() {
        assert_eq!(fmt_f64(0.1 + 0.2), "0.3");
        assert_eq!(fmt_f64(5.0), "5");
        assert_eq!(fmt_f64(-0.0), "0");
        assert_eq!(fmt_f64(1.5e-10), "1.5e-10");
        assert_eq!(fmt_f64(1e21), "1e21");
    }

    #[test]
    fn powers_and_roots() {
        assert_eq!(pow_int(&int(2), &int(10)), Some(int(1024)));
        assert_eq!(pow_int(&int(2), &int(-2)), Some(r("0.25")));
        assert_eq!(pow_int(&int(0), &int(-1)), None);
        assert_eq!(pow_int(&int(9), &int(100_000)), None);
        assert_eq!(sqrt_exact(&int(16)), Some(int(4)));
        assert_eq!(sqrt_exact(&r("0.25")), Some(r("0.5")));
        assert_eq!(sqrt_exact(&int(2)), None);
    }
}
