use serde::{Deserialize, Serialize};

use super::number::Rational;

pub const ANS: &str = "ans";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MathConstant {
    Pi,
    E,
    Tau,
    Phi,
    Inf,
    I,
}

impl MathConstant {
    pub const ALL: &[MathConstant] = &[Self::Pi, Self::E, Self::Tau, Self::Phi, Self::Inf];

    pub fn name(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::E => "e",
            Self::Tau => "tau",
            Self::Phi => "phi",
            Self::Inf => "inf",
            Self::I => "i",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.name() == name)
    }

    pub fn value(self) -> Option<f64> {
        match self {
            Self::Pi => Some(std::f64::consts::PI),
            Self::E => Some(std::f64::consts::E),
            Self::Tau => Some(std::f64::consts::TAU),
            Self::Phi => Some(std::f64::consts::GOLDEN_RATIO),
            Self::Inf => Some(f64::INFINITY),
            Self::I => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Num(Rational),
    Constant(MathConstant),
    Variable(String),
    Neg(Box<Expr>),
    Fact(Box<Expr>),
    // TODO: Percent %
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Unit(String),
    Convert(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Assign(String, Expr),
    Expr(Expr),
}

impl Expr {
    pub fn num(n: i64) -> Expr {
        Expr::Num(super::number::int(n))
    }

    pub fn num_bits(&self) -> u64 {
        match self {
            Expr::Num(r) => r.numer().bits() + r.denom().bits(),
            Expr::Constant(_) | Expr::Variable(_) | Expr::Unit(_) => 0,
            Expr::Neg(a) | Expr::Fact(a) => a.num_bits(),
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Pow(a, b)
            | Expr::Eq(a, b)
            | Expr::Convert(a, b) => a.num_bits().saturating_add(b.num_bits()),
            Expr::Call(_, args) => args.iter().map(Expr::num_bits).fold(0, u64::saturating_add),
        }
    }

    pub fn bin(self, op: fn(Box<Expr>, Box<Expr>) -> Expr, rhs: Expr) -> Expr {
        op(Box::new(self), Box::new(rhs))
    }

    pub fn children(&self) -> Vec<&Expr> {
        match self {
            Expr::Num(_) | Expr::Constant(_) | Expr::Variable(_) | Expr::Unit(_) => vec![],
            Expr::Neg(a) | Expr::Fact(a) => vec![a],
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Pow(a, b)
            | Expr::Eq(a, b)
            | Expr::Convert(a, b) => vec![a, b],
            Expr::Call(_, args) => args.iter().collect(),
        }
    }

    pub fn try_map<E>(&self, f: &mut impl FnMut(&Expr) -> Result<Expr, E>) -> Result<Expr, E> {
        let bx = |e: Expr| Box::new(e);
        Ok(match self {
            Expr::Num(_) | Expr::Constant(_) | Expr::Variable(_) | Expr::Unit(_) => self.clone(),
            Expr::Neg(a) => Expr::Neg(bx(f(a)?)),
            Expr::Fact(a) => Expr::Fact(bx(f(a)?)),
            Expr::Add(a, b) => Expr::Add(bx(f(a)?), bx(f(b)?)),
            Expr::Sub(a, b) => Expr::Sub(bx(f(a)?), bx(f(b)?)),
            Expr::Mul(a, b) => Expr::Mul(bx(f(a)?), bx(f(b)?)),
            Expr::Div(a, b) => Expr::Div(bx(f(a)?), bx(f(b)?)),
            Expr::Pow(a, b) => Expr::Pow(bx(f(a)?), bx(f(b)?)),
            Expr::Eq(a, b) => Expr::Eq(bx(f(a)?), bx(f(b)?)),
            Expr::Convert(a, u) => Expr::Convert(bx(f(a)?), u.clone()),
            Expr::Call(n, args) => Expr::Call(
                n.clone(),
                args.iter().map(&mut *f).collect::<Result<Vec<_>, E>>()?,
            ),
        })
    }

    pub fn is_closed(&self) -> bool {
        match self {
            Expr::Variable(_) | Expr::Unit(_) => false,
            _ => self.children().into_iter().all(Expr::is_closed),
        }
    }

    pub fn mentions(&self, name: &str) -> bool {
        match self {
            Expr::Variable(n) => n == name,
            _ => self.children().into_iter().any(|c| c.mentions(name)),
        }
    }

    pub fn has_units(&self) -> bool {
        match self {
            Expr::Unit(_) => true,
            _ => self.children().into_iter().any(Expr::has_units),
        }
    }

    pub fn unit_atoms(&self, out: &mut Vec<String>) {
        match self {
            Expr::Unit(s) => out.push(s.clone()),
            _ => self.children().into_iter().for_each(|c| c.unit_atoms(out)),
        }
    }

    pub fn contains_constant(&self, c: MathConstant) -> bool {
        match self {
            Expr::Constant(k) => *k == c,
            _ => self.children().into_iter().any(|e| e.contains_constant(c)),
        }
    }

    pub fn replace_var(&self, name: &str, with: &Expr) -> Expr {
        match self {
            Expr::Variable(n) if n == name => with.clone(),
            _ => self
                .try_map::<()>(&mut |c| Ok(c.replace_var(name, with)))
                .expect("infallible"),
        }
    }
}
