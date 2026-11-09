use std::ops::{Div, Mul};

use num_bigint::BigInt;

use super::ast::Expr;
use super::notation::{SI_PREFIXES, SiPrefix};
use super::number::{self, Rational};

pub const UNIT_SEPARATOR: char = '_';
pub const CONVERSION_ARROW: &str = "->";

// ---------------------------------------------------------------------------
// Dimensions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dimension(pub [i32; 7]);

const BASE_SYMBOLS: [&str; 7] = ["m", "kg", "s", "A", "K", "mol", "cd"];
const BASE_DISPLAY_ORDER: [usize; 7] = [1, 0, 2, 3, 4, 5, 6];

impl Dimension {
    pub const NONE: Dimension = Dimension([0; 7]);

    const fn base(i: usize) -> Dimension {
        let mut d = [0; 7];
        d[i] = 1;
        Dimension(d)
    }

    pub fn is_dimensionless(self) -> bool {
        self == Self::NONE
    }

    pub fn checked_pow(self, numer: i64, denom: i64) -> Option<Dimension> {
        let mut out = [0; 7];
        for (o, e) in out.iter_mut().zip(self.0) {
            let scaled = i64::from(e).checked_mul(numer)?;
            if scaled % denom != 0 {
                return None;
            }
            *o = i32::try_from(scaled / denom).ok()?;
        }
        Some(Dimension(out))
    }

    pub fn power_of(self, unit: Dimension) -> Option<i32> {
        let mut k = None;
        for (a, b) in self.0.into_iter().zip(unit.0) {
            match (a, b) {
                (0, 0) => {}
                (_, 0) => return None,
                (a, b) if a % b != 0 => return None,
                (a, b) => match k {
                    None => k = Some(a / b),
                    Some(k) if k == a / b => {}
                    Some(_) => return None,
                },
            }
        }
        k.filter(|&k| k != 0)
    }

    pub fn base_units(self) -> Expr {
        let factors: Vec<(String, i32)> = BASE_DISPLAY_ORDER
            .iter()
            .filter(|&&i| self.0[i] != 0)
            .map(|&i| (BASE_SYMBOLS[i].to_string(), self.0[i]))
            .collect();
        unit_product(&factors)
    }
}

impl Mul for Dimension {
    type Output = Dimension;
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn mul(self, rhs: Dimension) -> Dimension {
        let mut out = self.0;
        for (o, r) in out.iter_mut().zip(rhs.0) {
            *o += r;
        }
        Dimension(out)
    }
}

impl Div for Dimension {
    type Output = Dimension;
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Dimension) -> Dimension {
        let mut out = self.0;
        for (o, r) in out.iter_mut().zip(rhs.0) {
            *o -= r;
        }
        Dimension(out)
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct UnitDef {
    pub symbol: &'static str,
    pub aliases: &'static [&'static str],
    pub dimension: Dimension,
    pub factor: &'static str,
    pub prefixable: bool,
}

const fn dim(m: i32, kg: i32, s: i32, a: i32, k: i32) -> Dimension {
    Dimension([m, kg, s, a, k, 0, 0])
}

const fn si(symbol: &'static str, dimension: Dimension) -> UnitDef {
    UnitDef {
        symbol,
        aliases: &[],
        dimension,
        factor: "1",
        prefixable: true,
    }
}

const fn other(
    symbol: &'static str,
    dimension: Dimension,
    factor: &'static str,
    prefixable: bool,
) -> UnitDef {
    UnitDef {
        symbol,
        aliases: &[],
        dimension,
        factor,
        prefixable,
    }
}

pub const UNITS: &[UnitDef] = &[
    si("m", Dimension::base(0)),
    other("g", Dimension::base(1), "0.001", true),
    si("s", Dimension::base(2)),
    si("A", Dimension::base(3)),
    si("K", Dimension::base(4)),
    si("mol", Dimension::base(5)),
    si("cd", Dimension::base(6)),
    si("N", dim(1, 1, -2, 0, 0)),
    si("J", dim(2, 1, -2, 0, 0)),
    si("W", dim(2, 1, -3, 0, 0)),
    si("Pa", dim(-1, 1, -2, 0, 0)),
    si("Hz", dim(0, 0, -1, 0, 0)),
    si("C", dim(0, 0, 1, 1, 0)),
    si("V", dim(2, 1, -3, -1, 0)),
    UnitDef {
        aliases: &["\u{3a9}"],
        ..si("Ohm", dim(2, 1, -3, -2, 0))
    },
    si("S", dim(-2, -1, 3, 2, 0)),
    si("F", dim(-2, -1, 4, 2, 0)),
    si("H", dim(2, 1, -2, -2, 0)),
    si("T", dim(0, 1, -2, -1, 0)),
    si("Wb", dim(2, 1, -2, -1, 0)),
    other("min", Dimension::base(2), "60", false),
    other("h", Dimension::base(2), "3600", false),
    other("d", Dimension::base(2), "86400", false),
    UnitDef {
        aliases: &["l"],
        ..other("L", dim(3, 0, 0, 0, 0), "0.001", true)
    },
    other("t", Dimension::base(1), "1000", false),
    other("bar", dim(-1, 1, -2, 0, 0), "100000", true),
    other("atm", dim(-1, 1, -2, 0, 0), "101325", false),
    other("eV", dim(2, 1, -2, 0, 0), "1.602176634e-19", true),
    other("Wh", dim(2, 1, -2, 0, 0), "3600", true),
    other("Ah", dim(0, 0, 1, 1, 0), "3600", true),
    other("cal", dim(2, 1, -2, 0, 0), "4.184", true),
    other("in", Dimension::base(0), "0.0254", false),
    other("ft", Dimension::base(0), "0.3048", false),
    other("yd", Dimension::base(0), "0.9144", false),
    other("mi", Dimension::base(0), "1609.344", false),
    other("lb", Dimension::base(1), "0.45359237", false),
];

const DERIVED_FOR_DISPLAY: &[&str] = &[
    "N", "J", "W", "Pa", "Hz", "C", "V", "Ohm", "S", "F", "H", "T", "Wb",
];

const MICRO_ALIASES: &[&str] = &["u", "\u{3bc}"];

#[derive(Debug, Clone)]
pub struct ResolvedUnit {
    pub symbol: String,
    pub def: &'static UnitDef,
    pub prefix: Option<&'static SiPrefix>,
    pub factor: Rational,
    pub dimension: Dimension,
}

fn parse_factor(text: &str) -> Rational {
    let (mantissa, exp) = match text.split_once('e') {
        Some((m, e)) => (m, e.parse::<i32>().unwrap_or(0)),
        None => (text, 0),
    };
    let m = number::parse_decimal(mantissa).expect("unit factors are valid decimals");
    m * pow10(exp)
}

fn pow10(e: i32) -> Rational {
    let p = BigInt::from(10u32).pow(e.unsigned_abs());
    if e >= 0 {
        Rational::from_integer(p)
    } else {
        Rational::new(BigInt::from(1u32), p)
    }
}

fn find_def(symbol: &str) -> Option<&'static UnitDef> {
    UNITS
        .iter()
        .find(|u| u.symbol == symbol || u.aliases.contains(&symbol))
}

fn find_prefix(text: &str) -> Option<&'static SiPrefix> {
    SI_PREFIXES.iter().find(|p| p.symbol == text).or_else(|| {
        MICRO_ALIASES
            .contains(&text)
            .then(|| SI_PREFIXES.iter().find(|p| p.symbol == "µ"))
            .flatten()
    })
}

pub fn resolve(text: &str) -> Option<ResolvedUnit> {
    let make = |def: &'static UnitDef, prefix: Option<&'static SiPrefix>| {
        let p = prefix.map_or(0, |p| p.exponent);
        ResolvedUnit {
            symbol: format!("{}{}", prefix.map_or("", |p| p.symbol), def.symbol),
            def,
            prefix,
            factor: parse_factor(def.factor) * pow10(p),
            dimension: def.dimension,
        }
    };
    if let Some(def) = find_def(text) {
        return Some(make(def, None));
    }
    let mut best: Option<ResolvedUnit> = None;
    for (i, _) in text.char_indices().skip(1) {
        let (head, rest) = text.split_at(i);
        if let (Some(prefix), Some(def)) = (find_prefix(head), find_def(rest))
            && def.prefixable
        {
            best = Some(make(def, Some(prefix)));
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Unit expressions
// ---------------------------------------------------------------------------

pub fn is_unit_expr(e: &Expr) -> bool {
    match e {
        Expr::Unit(_) => true,
        Expr::Pow(base, exp) => is_unit_expr(base) && matches!(**exp, Expr::Num(_)),
        Expr::Mul(a, b) | Expr::Div(a, b) => is_unit_expr(a) && is_unit_expr(b),
        _ => false,
    }
}

pub fn flatten(e: &Expr) -> Vec<(String, i32)> {
    fn walk(e: &Expr, sign: i32, out: &mut Vec<(String, i32)>) {
        match e {
            Expr::Unit(s) => push(out, s, sign),
            Expr::Pow(base, exp) => {
                let n = match &**exp {
                    Expr::Num(r) if number::is_integer(r) => {
                        num_traits::ToPrimitive::to_i32(r.numer()).unwrap_or(1)
                    }
                    _ => 1,
                };
                walk(base, sign * n, out);
            }
            Expr::Mul(a, b) => {
                walk(a, sign, out);
                walk(b, sign, out);
            }
            Expr::Div(a, b) => {
                walk(a, sign, out);
                walk(b, -sign, out);
            }
            _ => {}
        }
    }
    fn push(out: &mut Vec<(String, i32)>, s: &str, e: i32) {
        match out.iter_mut().find(|(n, _)| n == s) {
            Some((_, x)) => *x += e,
            None => out.push((s.to_string(), e)),
        }
    }
    let mut out = Vec::new();
    walk(e, 1, &mut out);
    out.retain(|(_, e)| *e != 0);
    out
}

pub fn unit_product(factors: &[(String, i32)]) -> Expr {
    let atom = |(s, e): &(String, i32)| {
        let u = Expr::Unit(s.clone());
        if *e == 1 {
            u
        } else {
            Expr::Pow(Box::new(u), Box::new(Expr::num(i64::from(*e))))
        }
    };
    let positive: Vec<_> = factors.iter().filter(|(_, e)| *e > 0).collect();
    let negative: Vec<_> = factors.iter().filter(|(_, e)| *e < 0).collect();
    let mut acc: Option<Expr> = None;
    for f in &positive {
        let a = atom(f);
        acc = Some(match acc {
            None => a,
            Some(x) => Expr::Mul(Box::new(x), Box::new(a)),
        });
    }
    for (s, e) in negative {
        acc = Some(match acc {
            None => atom(&(s.clone(), *e)),
            Some(x) => Expr::Div(Box::new(x), Box::new(atom(&(s.clone(), -e)))),
        });
    }
    acc.unwrap_or_else(|| Expr::num(1))
}

pub fn unit_text(e: &Expr) -> String {
    let factors = flatten(e);
    let atom = |s: &str, e: i32| {
        if e == 1 {
            s.to_string()
        } else {
            format!("{s}^{e}")
        }
    };
    let numerator: Vec<String> = factors
        .iter()
        .filter(|(_, e)| *e > 0)
        .map(|(s, e)| atom(s, *e))
        .collect();
    if numerator.is_empty() {
        return factors
            .iter()
            .map(|(s, e)| atom(s, *e))
            .collect::<Vec<_>>()
            .join("*");
    }
    let mut out = numerator.join("*");
    for (s, e) in factors.iter().filter(|(_, e)| *e < 0) {
        out.push('/');
        out.push_str(&atom(s, -e));
    }
    out
}

pub fn measure(e: &Expr) -> Result<(Rational, Dimension), String> {
    let mut factor = Rational::from_integer(1.into());
    let mut dimension = Dimension::NONE;
    for (symbol, exp) in flatten(e) {
        let u = resolve(&symbol).ok_or_else(|| format!("Unknown unit '{symbol}'"))?;
        let f = number::pow_int(&u.factor, &number::int(i64::from(exp)))
            .ok_or("Unit exponent too large")?;
        factor *= f;
        dimension = dimension
            * u.dimension
                .checked_pow(i64::from(exp), 1)
                .ok_or("Unit exponent too large")?;
    }
    Ok((factor, dimension))
}

pub fn display_unit(dimension: Dimension, atoms: &[String]) -> (Expr, Rational) {
    let mut distinct: Vec<&String> = atoms.iter().collect();
    distinct.sort();
    distinct.dedup();
    if let [only] = distinct.as_slice()
        && let Some(u) = resolve(only)
        && let Some(k) = dimension.power_of(u.dimension)
    {
        let unit = unit_product(&[(u.symbol.clone(), k)]);
        let factor = number::pow_int(&u.factor, &number::int(i64::from(k)))
            .unwrap_or_else(|| Rational::from_integer(1.into()));
        return (unit, factor);
    }
    for symbol in DERIVED_FOR_DISPLAY {
        if let Some(u) = resolve(symbol)
            && u.dimension == dimension
        {
            return (Expr::Unit(u.symbol), Rational::from_integer(1.into()));
        }
    }
    (dimension.base_units(), Rational::from_integer(1.into()))
}

pub fn dimension_text(dimension: Dimension) -> String {
    if dimension.is_dimensionless() {
        "no unit".to_string()
    } else {
        unit_text(&dimension.base_units())
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn is_unit_char(c: char) -> bool {
    c.is_ascii_alphabetic() || matches!(c, '\u{b5}' | '\u{3bc}' | '\u{3a9}' | '\u{2126}')
}

fn normalize(atom: &str) -> String {
    atom.replace('\u{3bc}', "\u{b5}")
        .replace('\u{2126}', "\u{3a9}")
}

pub fn parse_chain(s: &str) -> Result<(Expr, usize), String> {
    let (mut acc, mut pos) = parse_unit_factor(s, 0)?;
    loop {
        let op = s[pos..].chars().next();
        if !matches!(op, Some('*' | '/')) {
            break;
        }
        match parse_unit_factor(s, pos + 1) {
            Ok((f, next)) => {
                acc = if op == Some('*') {
                    Expr::Mul(Box::new(acc), Box::new(f))
                } else {
                    Expr::Div(Box::new(acc), Box::new(f))
                };
                pos = next;
            }
            Err(_) => break,
        }
    }
    Ok((acc, pos))
}

fn parse_unit_factor(s: &str, start: usize) -> Result<(Expr, usize), String> {
    let rest = &s[start..];
    let (base, mut pos) = if let Some(inner) = rest.strip_prefix('(') {
        let (e, used) = parse_chain(inner)?;
        if !inner[used..].starts_with(')') {
            return Err("Expected ')' in unit".to_string());
        }
        (e, start + 1 + used + 1)
    } else {
        let len: usize = rest
            .chars()
            .take_while(|&c| is_unit_char(c))
            .map(char::len_utf8)
            .sum();
        if len == 0 {
            return Err("Expected a unit after '_'".to_string());
        }
        let atom = normalize(&rest[..len]);
        let unit = resolve(&atom).ok_or_else(|| format!("Unknown unit '{atom}'"))?;
        (Expr::Unit(unit.symbol), start + len)
    };

    if let Some(after) = s[pos..].strip_prefix('^') {
        let negative = after.starts_with('-');
        let digits = &after[usize::from(negative)..];
        let n = digits.bytes().take_while(u8::is_ascii_digit).count();
        if n > 0 {
            let value: i64 = digits[..n].parse().map_err(|_| "Unit exponent too large")?;
            let value = if negative { -value } else { value };
            pos += 1 + usize::from(negative) + n;
            return Ok((Expr::Pow(Box::new(base), Box::new(Expr::num(value))), pos));
        }
    }
    Ok((base, pos))
}

pub fn catalogue() -> String {
    let list = UNITS.iter().map(|u| u.symbol).collect::<Vec<_>>().join(" ");
    format!("Units: {list}  ·  SI prefixes q … Q (u = µ) on all but min h d t atm in ft yd mi lb")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> Rational {
        parse_factor(s)
    }

    #[test]
    fn symbols_are_unique() {
        let mut symbols: Vec<_> = UNITS
            .iter()
            .flat_map(|u| std::iter::once(u.symbol).chain(u.aliases.iter().copied()))
            .collect();
        symbols.sort();
        let len = symbols.len();
        symbols.dedup();
        assert_eq!(len, symbols.len());
    }

    #[test]
    fn prefixes_resolve() {
        let km = resolve("km").unwrap();
        assert_eq!(km.symbol, "km");
        assert_eq!(km.factor, r("1000"));
        assert_eq!(resolve("kg").unwrap().factor, r("1"));
        assert_eq!(resolve("us").unwrap().symbol, "µs");
        assert_eq!(resolve("µs").unwrap().factor, r("1e-6"));
        assert_eq!(resolve("dam").unwrap().factor, r("10"));
        assert_eq!(resolve("kΩ").unwrap().symbol, "kOhm");
        assert_eq!(resolve("min").unwrap().factor, r("60"));
        assert_eq!(resolve("cd").unwrap().prefix, None);
        assert_eq!(resolve("Pa").unwrap().prefix, None);
        assert_eq!(resolve("mAh").unwrap().factor, r("3.6"));
        assert_eq!(resolve("ml").unwrap().symbol, "mL");
        assert!(resolve("kmin").is_none(), "minutes take no prefix");
        assert!(resolve("xyz").is_none());
    }

    #[test]
    fn chains_parse_and_print() {
        let (e, used) = parse_chain("m/s^2 + 1").unwrap();
        assert_eq!(used, 5);
        assert_eq!(unit_text(&e), "m/s^2");
        let (e, used) = parse_chain("kg*m*x").unwrap();
        assert_eq!(used, 4, "stops before a non-unit");
        assert_eq!(unit_text(&e), "kg*m");
        let (e, _) = parse_chain("s^-1").unwrap();
        assert_eq!(unit_text(&e), "s^-1");
        let (e, _) = parse_chain("(m/s)^2").unwrap();
        assert_eq!(unit_text(&e), "m^2/s^2");
        assert!(parse_chain("xyz").unwrap_err().contains("Unknown unit"));
    }

    #[test]
    fn measuring_units() {
        let (e, _) = parse_chain("km/h").unwrap();
        let (f, d) = measure(&e).unwrap();
        assert_eq!(f, Rational::new(5.into(), 18.into()));
        assert_eq!(d, dim(1, 0, -1, 0, 0));
        let (e, _) = parse_chain("N*m").unwrap();
        assert_eq!(measure(&e).unwrap().1, resolve("J").unwrap().dimension);
    }

    #[test]
    fn display_units() {
        let n = resolve("N").unwrap().dimension;
        assert_eq!(unit_text(&display_unit(n, &[]).0), "N");
        let speed = dim(1, 0, -1, 0, 0);
        assert_eq!(unit_text(&display_unit(speed, &[]).0), "m/s");
        let atoms = vec!["h".to_string(), "h".to_string()];
        let (u, f) = display_unit(Dimension::base(2), &atoms);
        assert_eq!(unit_text(&u), "h");
        assert_eq!(f, r("3600"));
        let atoms = vec!["ft".to_string()];
        assert_eq!(
            unit_text(&display_unit(dim(2, 0, 0, 0, 0), &atoms).0),
            "ft^2"
        );
        assert_eq!(dimension_text(dim(2, 1, -2, 0, 0)), "kg*m^2/s^2");
    }

    #[test]
    fn dimension_powers() {
        let area = dim(2, 0, 0, 0, 0);
        assert_eq!(area.power_of(Dimension::base(0)), Some(2));
        assert_eq!(area.checked_pow(1, 2), Some(Dimension::base(0)));
        assert_eq!(Dimension::base(0).checked_pow(1, 2), None);
    }
}
