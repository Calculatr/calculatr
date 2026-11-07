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

fn decimal_digits(r: &Rational) -> Option<u32> {
    let mut d = r.denom().clone();
    let (two, five) = (BigInt::from(2u32), BigInt::from(5u32));
    let (mut twos, mut fives) = (0u32, 0u32);
    while (&d % &two).is_zero() {
        d /= &two;
        twos += 1;
    }
    while (&d % &five).is_zero() {
        d /= &five;
        fives += 1;
    }
    d.is_one().then_some(twos.max(fives))
}

const MAX_EXACT_DIGITS: u32 = 15;

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
        _ => fmt_f64(to_f64(r)),
    }
}

fn fmt_huge_integer(n: &BigInt) -> String {
    let digits = n.abs().to_string();
    let mut exponent = digits.len() - 1;
    let lead: u64 = digits[..14].parse().unwrap_or(0);
    let mut mantissa = (lead + 5) / 10;
    if mantissa >= 10_000_000_000_000 {
        mantissa /= 10;
        exponent += 1;
    }
    let m = mantissa.to_string();
    let frac = m[1..].trim_end_matches('0');
    let sign = if n.is_negative() { "-" } else { "" };
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
    Some(if e >= 0 {
        Rational::new(n, d)
    } else {
        Rational::new(d, n)
    })
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
