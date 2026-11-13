use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use serde::{Deserialize, Serialize};

use super::number::{self, MAX_EXACT_DIGITS, Rational};

pub const MAX_DECIMAL_PLACES: u8 = 15;

const NOISY_SIGNIFICANT_DIGITS: i32 = 13;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Notation {
    #[default]
    Standard,
    Power,
    E,
    SiPrefix,
}

impl Notation {
    pub const ALL: [Notation; 4] = [
        Notation::Standard,
        Notation::Power,
        Notation::E,
        Notation::SiPrefix,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Notation::Standard => "Standard",
            Notation::Power => "Power of ten  1.5*10^3",
            Notation::E => "E notation  1.5e3",
            Notation::SiPrefix => "SI prefix  1.5k",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Notation::Standard => "standard",
            Notation::Power => "power",
            Notation::E => "e",
            Notation::SiPrefix => "si",
        }
    }

    pub fn from_key(word: &str) -> Option<Self> {
        Some(match word.to_ascii_lowercase().as_str() {
            "standard" | "std" | "normal" | "off" | "none" => Notation::Standard,
            "power" | "pow" | "x10" | "times10" => Notation::Power,
            "e" | "enotation" | "exp" => Notation::E,
            "si" | "prefix" | "prefixes" => Notation::SiPrefix,
            _ => return None,
        })
    }

    pub fn uses_exponent_step(self) -> bool {
        matches!(self, Notation::Power | Notation::E)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExponentStep {
    #[default]
    Scientific,
    Engineering,
}

impl ExponentStep {
    pub const ALL: [ExponentStep; 2] = [ExponentStep::Scientific, ExponentStep::Engineering];

    pub fn label(self) -> &'static str {
        match self {
            ExponentStep::Scientific => "Scientific  1.5e4",
            ExponentStep::Engineering => "Engineering  15e3",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ExponentStep::Scientific => "sci",
            ExponentStep::Engineering => "eng",
        }
    }

    pub fn from_key(word: &str) -> Option<Self> {
        Some(match word.to_ascii_lowercase().as_str() {
            "sci" | "scientific" | "1" => ExponentStep::Scientific,
            "eng" | "engineering" | "3" => ExponentStep::Engineering,
            _ => return None,
        })
    }

    fn step(self) -> i32 {
        match self {
            ExponentStep::Scientific => 1,
            ExponentStep::Engineering => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NumberFormat {
    pub decimal_places: Option<u8>,
    pub notation: Notation,
    pub exponent_step: ExponentStep,
    pub extended_prefixes: bool,
}

impl NumberFormat {
    pub fn summary(&self) -> String {
        let mut s = match self.notation {
            Notation::Standard => "std".to_string(),
            Notation::Power => format!("×10ⁿ·{}", self.exponent_step.key()),
            Notation::E => format!("e·{}", self.exponent_step.key()),
            Notation::SiPrefix if self.extended_prefixes => "SI+".to_string(),
            Notation::SiPrefix => "SI".to_string(),
        };
        if let Some(p) = self.decimal_places {
            s.push_str(&format!(" {p}dp"));
        }
        s
    }

    pub fn format(&self, value: &Rational) -> FormattedNumber {
        let negative = value.is_negative();
        let a = value.abs();
        if a.is_zero() {
            return FormattedNumber::plain(false, "0".into());
        }

        let mut out = match self.notation {
            Notation::Standard => self.standard(&a),
            Notation::Power | Notation::E => {
                let step = self.exponent_step.step();
                let (m, exp) = scale(denoise_split(&a), self.decimal_places, |e| {
                    Some(e.div_euclid(step) * step)
                })
                .expect("every exponent is representable");
                let scale = match (exp, self.notation) {
                    (0, _) => Scale::None,
                    (e, Notation::Power) => Scale::Power(e),
                    (e, _) => Scale::E(e),
                };
                FormattedNumber::new(false, decimal_string(&m), scale)
            }
            Notation::SiPrefix => self.si(&a),
        };
        out.negative = negative && !out.is_zero();
        out
    }

    fn standard(&self, a: &Rational) -> FormattedNumber {
        let auto = number::to_decimal_string(a);
        let auto_exponent = auto.contains('e');
        match self.decimal_places {
            None => match auto.split_once('e') {
                Some((m, e)) => FormattedNumber::new(
                    false,
                    m.to_string(),
                    Scale::Auto(e.parse().unwrap_or_default()),
                ),
                None => FormattedNumber::plain(false, auto),
            },
            Some(_) if auto_exponent => {
                let (m, exp) = scale(denoise_split(a), self.decimal_places, Some)
                    .expect("every exponent is representable");
                let scale = if exp == 0 {
                    Scale::None
                } else {
                    Scale::Auto(exp)
                };
                FormattedNumber::new(false, decimal_string(&m), scale)
            }
            Some(places) => {
                let rounded = round_to(&denoise(a), i32::from(places));
                FormattedNumber::plain(false, decimal_string(&rounded))
            }
        }
    }

    fn si(&self, a: &Rational) -> FormattedNumber {
        let extended = self.extended_prefixes;
        let choose = |e: i32| {
            if !(SI_MIN_EXPONENT..SI_MAX_EXPONENT + 3).contains(&e) {
                return None;
            }
            Some(
                SI_PREFIXES
                    .iter()
                    .filter(|p| extended || !p.extended)
                    .map(|p| p.exponent)
                    .chain([0])
                    .filter(|&x| x <= e)
                    .max()
                    .unwrap_or(SI_MIN_EXPONENT),
            )
        };
        let clean = denoise_split(a);
        match scale(clean.clone(), self.decimal_places, choose) {
            Some((m, exp)) => {
                let scale = match prefix_for(exp) {
                    Some(p) => Scale::Prefix(p),
                    None => Scale::None,
                };
                FormattedNumber::new(false, decimal_string(&m), scale)
            }
            None => {
                let (m, exp) = scale(clean, self.decimal_places, |e| Some(e.div_euclid(3) * 3))
                    .expect("every exponent is representable");
                FormattedNumber::new(false, decimal_string(&m), Scale::E(exp))
            }
        }
    }

    pub fn parseable(self) -> NumberFormat {
        match self.notation {
            Notation::E => NumberFormat {
                notation: Notation::Power,
                ..self
            },
            Notation::SiPrefix => NumberFormat {
                notation: Notation::Power,
                exponent_step: ExponentStep::Engineering,
                ..self
            },
            _ => self,
        }
    }
}

// ---------------------------------------------------------------------------
// SI prefixes
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub struct SiPrefix {
    pub symbol: &'static str,
    pub name: &'static str,
    pub exponent: i32,
    pub extended: bool,
}

const fn prefix(symbol: &'static str, name: &'static str, exponent: i32) -> SiPrefix {
    SiPrefix {
        symbol,
        name,
        exponent,
        extended: false,
    }
}

const fn extended(symbol: &'static str, name: &'static str, exponent: i32) -> SiPrefix {
    SiPrefix {
        symbol,
        name,
        exponent,
        extended: true,
    }
}

pub const SI_PREFIXES: &[SiPrefix] = &[
    prefix("q", "quecto", -30),
    prefix("r", "ronto", -27),
    prefix("y", "yocto", -24),
    prefix("z", "zepto", -21),
    prefix("a", "atto", -18),
    prefix("f", "femto", -15),
    prefix("p", "pico", -12),
    prefix("n", "nano", -9),
    prefix("µ", "micro", -6),
    prefix("m", "milli", -3),
    extended("c", "centi", -2),
    extended("d", "deci", -1),
    extended("da", "deca", 1),
    extended("h", "hecto", 2),
    prefix("k", "kilo", 3),
    prefix("M", "mega", 6),
    prefix("G", "giga", 9),
    prefix("T", "tera", 12),
    prefix("P", "peta", 15),
    prefix("E", "exa", 18),
    prefix("Z", "zetta", 21),
    prefix("Y", "yotta", 24),
    prefix("R", "ronna", 27),
    prefix("Q", "quetta", 30),
];

const SI_MIN_EXPONENT: i32 = -30;
const SI_MAX_EXPONENT: i32 = 30;

fn prefix_for(exponent: i32) -> Option<&'static SiPrefix> {
    SI_PREFIXES.iter().find(|p| p.exponent == exponent)
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    None,
    Auto(i32),
    Power(i32),
    E(i32),
    Prefix(&'static SiPrefix),
}

impl Scale {
    pub fn exponent(self) -> i32 {
        match self {
            Scale::None => 0,
            Scale::Auto(e) | Scale::Power(e) | Scale::E(e) => e,
            Scale::Prefix(p) => p.exponent,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormattedNumber {
    pub negative: bool,
    pub mantissa: String,
    pub scale: Scale,
}

impl FormattedNumber {
    fn new(negative: bool, mantissa: String, scale: Scale) -> Self {
        Self {
            negative,
            mantissa,
            scale,
        }
    }

    fn plain(negative: bool, mantissa: String) -> Self {
        Self::new(negative, mantissa, Scale::None)
    }

    pub fn is_zero(&self) -> bool {
        self.mantissa.bytes().all(|b| b == b'0' || b == b'.')
    }

    pub fn is_product(&self, parseable: bool) -> bool {
        match self.scale {
            Scale::None => false,
            Scale::Power(_) => true,
            Scale::Auto(_) | Scale::E(_) | Scale::Prefix(_) => parseable,
        }
    }

    pub fn render(&self, parseable: bool) -> String {
        let mut s = String::new();
        if self.negative {
            s.push('-');
        }
        s.push_str(&self.mantissa);
        let exp = self.scale.exponent();
        match self.scale {
            Scale::None => {}
            _ if parseable => s.push_str(&format!("*10^{exp}")),
            Scale::Power(e) => s.push_str(&format!("*10^{e}")),
            Scale::Auto(e) | Scale::E(e) => s.push_str(&format!("e{e}")),
            Scale::Prefix(p) => s.push_str(p.symbol),
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Exact decimal arithmetic
// ---------------------------------------------------------------------------

fn pow10(e: i32) -> Rational {
    let p = BigInt::from(10u32).pow(e.unsigned_abs());
    if e >= 0 {
        Rational::from_integer(p)
    } else {
        Rational::new(BigInt::from(1u32), p)
    }
}

fn pow10_int(e: i32) -> BigInt {
    BigInt::from(10u32).pow(e.unsigned_abs())
}

fn floor_log10(a: &Rational) -> i32 {
    let digits = |n: &BigInt| number::digit_count(n) as i32;
    let e = digits(a.numer()) - digits(a.denom());
    let below = if e >= 0 {
        *a.numer() < a.denom() * pow10_int(e)
    } else {
        a.numer() * pow10_int(e) < *a.denom()
    };
    if below { e - 1 } else { e }
}

fn round_to(a: &Rational, places: i32) -> Rational {
    let s = pow10(places);
    let half = Rational::new(1.into(), 2.into());
    (a * &s + half).floor() / s
}

fn denoise_split(a: &Rational) -> (Rational, i32) {
    if a.is_zero() {
        return (a.clone(), 0);
    }
    let e = floor_log10(a);
    let exact =
        a.numer().bits() <= 128 && number::decimal_digits(a).is_some_and(|d| d <= MAX_EXACT_DIGITS);
    if exact {
        return (a / pow10(e), e);
    }
    let shift = e - (NOISY_SIGNIFICANT_DIGITS - 1);
    let (num, den) = if shift >= 0 {
        (a.numer().clone(), a.denom() * pow10_int(shift))
    } else {
        (a.numer() * pow10_int(shift), a.denom().clone())
    };
    let q = (num * 2 + &den) / (den * 2);
    let m = Rational::new(q, pow10_int(NOISY_SIGNIFICANT_DIGITS - 1));
    if m >= Rational::from_integer(10.into()) {
        (m / Rational::from_integer(10.into()), e + 1)
    } else {
        (m, e)
    }
}

fn denoise(a: &Rational) -> Rational {
    let (m, e) = denoise_split(a);
    m * pow10(e)
}

fn scale(
    value: (Rational, i32),
    places: Option<u8>,
    choose: impl Fn(i32) -> Option<i32>,
) -> Option<(Rational, i32)> {
    let (mut m, mut e) = value;
    let mut exp = choose(e)?;
    for _ in 0..3 {
        let mantissa = &m * pow10(e - exp);
        let mantissa = match places {
            Some(p) => round_to(&mantissa, i32::from(p)),
            None => mantissa,
        };
        if mantissa.is_zero() {
            return Some((mantissa, 0));
        }
        let ml = floor_log10(&mantissa);
        let next = choose(ml + exp)?;
        if next == exp {
            return Some((mantissa, exp));
        }
        m = &mantissa / pow10(ml);
        e = ml + exp;
        exp = next;
    }
    Some((&m * pow10(e - exp), exp))
}

fn decimal_string(r: &Rational) -> String {
    let digits = number::decimal_digits(r).unwrap_or(MAX_EXACT_DIGITS);
    let scaled = round_to(r, digits as i32) * pow10(digits as i32);
    let mut s = scaled.to_integer().to_string();
    if digits == 0 {
        return s;
    }
    let d = digits as usize;
    if s.len() <= d {
        s = format!("{}{s}", "0".repeat(d - s.len() + 1));
    }
    let split = s.len() - d;
    format!("{}.{}", &s[..split], &s[split..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::number::{int, parse_decimal};

    fn r(s: &str) -> Rational {
        let (neg, digits) = match s.strip_prefix('-') {
            Some(d) => (true, d),
            None => (false, s),
        };
        let v = parse_decimal(digits).unwrap();
        if neg { -v } else { v }
    }

    fn fmt(nf: NumberFormat, s: &str) -> String {
        nf.format(&r(s)).render(false)
    }

    fn nf(notation: Notation, step: ExponentStep, places: Option<u8>) -> NumberFormat {
        NumberFormat {
            decimal_places: places,
            notation,
            exponent_step: step,
            extended_prefixes: false,
        }
    }

    #[test]
    fn default_matches_the_legacy_decimal_output() {
        let d = NumberFormat::default();
        for v in [
            Rational::new(1.into(), 3.into()),
            r("0.25"),
            r("-0.05"),
            int(7),
            int(0),
            parse_decimal("0.000000000015").unwrap(),
            number::pow_int(&int(2), &int(100)).unwrap(),
            number::pow_int(&int(10), &int(21)).unwrap(),
        ] {
            assert_eq!(d.format(&v).render(false), number::to_decimal_string(&v));
        }
    }

    #[test]
    fn floor_log10_is_exact() {
        assert_eq!(floor_log10(&int(1)), 0);
        assert_eq!(floor_log10(&int(9)), 0);
        assert_eq!(floor_log10(&int(10)), 1);
        assert_eq!(floor_log10(&r("0.1")), -1);
        assert_eq!(floor_log10(&r("0.099")), -2);
        assert_eq!(floor_log10(&Rational::new(1.into(), 3.into())), -1);
    }

    #[test]
    fn decimal_places_round_half_away_from_zero() {
        let two = nf(Notation::Standard, ExponentStep::Scientific, Some(2));
        assert_eq!(fmt(two, "3.14159"), "3.14");
        assert_eq!(fmt(two, "2.675"), "2.68");
        assert_eq!(fmt(two, "-2.675"), "-2.68");
        assert_eq!(fmt(two, "1.5"), "1.5");
        assert_eq!(fmt(two, "-0.001"), "0");
        assert_eq!(fmt(two, "1234"), "1234");
        let zero = nf(Notation::Standard, ExponentStep::Scientific, Some(0));
        assert_eq!(fmt(zero, "2.5"), "3");
        assert_eq!(
            two.format(&Rational::new(2.into(), 3.into())).render(false),
            "0.67"
        );
    }

    #[test]
    fn scientific_and_engineering_exponents() {
        let sci = nf(Notation::E, ExponentStep::Scientific, None);
        assert_eq!(fmt(sci, "12345"), "1.2345e4");
        assert_eq!(fmt(sci, "0.00012"), "1.2e-4");
        assert_eq!(fmt(sci, "5"), "5");
        let eng = nf(Notation::E, ExponentStep::Engineering, None);
        assert_eq!(fmt(eng, "12345"), "12.345e3");
        assert_eq!(fmt(eng, "0.00012"), "120e-6");
        let pow = nf(Notation::Power, ExponentStep::Scientific, Some(2));
        assert_eq!(fmt(pow, "-12345"), "-1.23*10^4");
        assert_eq!(
            fmt(pow, "9.996"),
            "1*10^1",
            "rounds up into the next decade"
        );
        assert_eq!(fmt(pow, "99960"), "1*10^5");
    }

    #[test]
    fn si_prefixes() {
        let si = nf(Notation::SiPrefix, ExponentStep::Scientific, Some(3));
        assert_eq!(fmt(si, "1500"), "1.5k");
        assert_eq!(fmt(si, "15000"), "15k");
        assert_eq!(fmt(si, "0.0047"), "4.7m");
        assert_eq!(fmt(si, "0.0000033"), "3.3µ");
        assert_eq!(fmt(si, "999999.9999"), "1M");
        assert_eq!(fmt(si, "12"), "12");
        assert_eq!(fmt(si, "0.05"), "50m");
        let ext = NumberFormat {
            extended_prefixes: true,
            ..si
        };
        assert_eq!(fmt(ext, "0.05"), "5c");
        assert_eq!(fmt(ext, "0.5"), "5d");
        assert_eq!(fmt(ext, "50"), "5da");
        assert_eq!(fmt(ext, "500"), "5h");
        assert_eq!(fmt(ext, "5000"), "5k");
        let huge = number::pow_int(&int(10), &int(40)).unwrap();
        assert_eq!(si.format(&huge).render(false), "10e39");
    }

    #[test]
    fn inexact_values_keep_thirteen_significant_digits() {
        let sci = nf(Notation::E, ExponentStep::Scientific, None);
        let third = Rational::new(1.into(), 3.into());
        assert_eq!(sci.format(&third).render(false), "3.333333333333e-1");
        let many = nf(Notation::Standard, ExponentStep::Scientific, Some(15));
        assert_eq!(many.format(&third).render(false), "0.3333333333333");
    }

    #[test]
    fn parseable_output_uses_powers_of_ten() {
        let si = nf(Notation::SiPrefix, ExponentStep::Scientific, None);
        let f = si.format(&r("1500"));
        assert_eq!(f.render(false), "1.5k");
        assert_eq!(f.render(true), "1.5*10^3");
        assert!(f.is_product(true));
        assert!(!f.is_product(false));
        let tiny = number::pow_int(&int(10), &int(-20)).unwrap();
        let auto = NumberFormat::default().format(&tiny);
        assert_eq!(auto.render(false), "1e-20");
        assert_eq!(auto.render(true), "1*10^-20");
    }
}
