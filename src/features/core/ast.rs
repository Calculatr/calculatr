use serde::{Deserialize, Serialize};

use super::number::Rational;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MathConstant {
    Pi,
    E,
}

impl MathConstant {
    pub const ALL: &[MathConstant] = &[Self::Pi, Self::E];

    pub fn name(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::E => "e",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|c| c.name() == name)
    }

    pub fn value(self) -> f64 {
        match self {
            Self::Pi => std::f64::consts::PI,
            Self::E => std::f64::consts::E,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Num(Rational),
    Constant(MathConstant),
    Variable(String),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Eq(Box<Expr>, Box<Expr>),
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

    pub fn bin(self, op: fn(Box<Expr>, Box<Expr>) -> Expr, rhs: Expr) -> Expr {
        op(Box::new(self), Box::new(rhs))
    }

    pub fn children(&self) -> Vec<&Expr> {
        match self {
            Expr::Num(_) | Expr::Constant(_) | Expr::Variable(_) => vec![],
            Expr::Neg(a) => vec![a],
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Pow(a, b)
            | Expr::Eq(a, b) => vec![a, b],
            Expr::Call(_, args) => args.iter().collect(),
        }
    }

    pub fn try_map<E>(&self, f: &mut impl FnMut(&Expr) -> Result<Expr, E>) -> Result<Expr, E> {
        let bx = |e: Expr| Box::new(e);
        Ok(match self {
            Expr::Num(_) | Expr::Constant(_) | Expr::Variable(_) => self.clone(),
            Expr::Neg(a) => Expr::Neg(bx(f(a)?)),
            Expr::Add(a, b) => Expr::Add(bx(f(a)?), bx(f(b)?)),
            Expr::Sub(a, b) => Expr::Sub(bx(f(a)?), bx(f(b)?)),
            Expr::Mul(a, b) => Expr::Mul(bx(f(a)?), bx(f(b)?)),
            Expr::Div(a, b) => Expr::Div(bx(f(a)?), bx(f(b)?)),
            Expr::Pow(a, b) => Expr::Pow(bx(f(a)?), bx(f(b)?)),
            Expr::Eq(a, b) => Expr::Eq(bx(f(a)?), bx(f(b)?)),
            Expr::Call(n, args) => Expr::Call(
                n.clone(),
                args.iter().map(&mut *f).collect::<Result<Vec<_>, E>>()?,
            ),
        })
    }

    pub fn is_closed(&self) -> bool {
        match self {
            Expr::Variable(_) => false,
            _ => self.children().into_iter().all(Expr::is_closed),
        }
    }

    pub fn mentions(&self, name: &str) -> bool {
        match self {
            Expr::Variable(n) => n == name,
            _ => self.children().into_iter().any(|c| c.mentions(name)),
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
