#![allow(dead_code)]

use std::fmt;
use std::ops::{Div, Mul};

pub const UNIT_SEPARATOR: char = '_';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dimension(pub [i8; 7]);

impl Dimension {
    pub const NONE: Dimension = Dimension([0; 7]);
    pub const LENGTH: Dimension = Dimension([1, 0, 0, 0, 0, 0, 0]);
    pub const MASS: Dimension = Dimension([0, 1, 0, 0, 0, 0, 0]);
    pub const TIME: Dimension = Dimension([0, 0, 1, 0, 0, 0, 0]);
    pub const CURRENT: Dimension = Dimension([0, 0, 0, 1, 0, 0, 0]);
    pub const TEMPERATURE: Dimension = Dimension([0, 0, 0, 0, 1, 0, 0]);
    pub const AMOUNT: Dimension = Dimension([0, 0, 0, 0, 0, 1, 0]);
    pub const LUMINOSITY: Dimension = Dimension([0, 0, 0, 0, 0, 0, 1]);

    pub fn is_dimensionless(self) -> bool {
        self == Self::NONE
    }

    pub fn powi(self, n: i8) -> Dimension {
        Dimension(self.0.map(|e| e * n))
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
    fn div(self, rhs: Dimension) -> Dimension {
        self * rhs.powi(-1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    pub symbol: &'static str,
    pub name: &'static str,
    pub dimension: Dimension,
    pub factor: f64,
}

impl Unit {
    const fn si(symbol: &'static str, name: &'static str, dimension: Dimension) -> Self {
        Self {
            symbol,
            name,
            dimension,
            factor: 1.0,
        }
    }

    pub fn suffix(&self) -> String {
        format!("{UNIT_SEPARATOR}{}", self.symbol)
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol)
    }
}

const M: Dimension = Dimension::LENGTH;
const KG: Dimension = Dimension::MASS;
const S: Dimension = Dimension::TIME;
const A: Dimension = Dimension::CURRENT;

pub const UNITS: &[Unit] = &[
    Unit::si("m", "metre", Dimension::LENGTH),
    Unit {
        symbol: "g",
        name: "gram",
        dimension: Dimension::MASS,
        factor: 1e-3,
    },
    Unit::si("s", "second", Dimension::TIME),
    Unit::si("A", "ampere", Dimension::CURRENT),
    Unit::si("K", "kelvin", Dimension::TEMPERATURE),
    Unit::si("mol", "mole", Dimension::AMOUNT),
    Unit::si("cd", "candela", Dimension::LUMINOSITY),
    // N = kg·m/s²
    Unit::si("N", "newton", Dimension([1, 1, -2, 0, 0, 0, 0])),
    // J = N·m
    Unit::si("J", "joule", Dimension([2, 1, -2, 0, 0, 0, 0])),
    // W = J/s
    Unit::si("W", "watt", Dimension([2, 1, -3, 0, 0, 0, 0])),
    // Pa = N/m²
    Unit::si("Pa", "pascal", Dimension([-1, 1, -2, 0, 0, 0, 0])),
    // Hz = 1/s
    Unit::si("Hz", "hertz", Dimension([0, 0, -1, 0, 0, 0, 0])),
    // C = A·s
    Unit::si("C", "coulomb", Dimension([0, 0, 1, 1, 0, 0, 0])),
    // V = W/A
    Unit::si("V", "volt", Dimension([2, 1, -3, -1, 0, 0, 0])),
    // Ω = V/A
    Unit::si("Ohm", "ohm", Dimension([2, 1, -3, -2, 0, 0, 0])),
];

pub fn lookup(symbol: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|u| u.symbol == symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_are_unique() {
        let mut symbols: Vec<_> = UNITS.iter().map(|u| u.symbol).collect();
        symbols.sort();
        let len = symbols.len();
        symbols.dedup();
        assert_eq!(len, symbols.len());
    }

    #[test]
    fn derived_dimensions_are_consistent() {
        let newton = lookup("N").unwrap().dimension;
        assert_eq!(newton, KG * M / S.powi(2));
        assert_eq!(lookup("J").unwrap().dimension, newton * M);
        assert_eq!(
            lookup("V").unwrap().dimension,
            lookup("W").unwrap().dimension / A
        );
        assert!((newton / newton).is_dimensionless());
    }

    #[test]
    fn suffix_uses_the_separator() {
        assert_eq!(lookup("N").unwrap().suffix(), "_N");
    }
}
