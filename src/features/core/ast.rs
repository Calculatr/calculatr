#[derive(Debug, Clone, PartialEq)]
pub enum MathConstant {
    PI,
    E,
}

impl MathConstant {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "pi" => Some(Self::PI),
            "e" => Some(Self::E),
            _ => None,
        }
    }

    pub fn value(&self) -> f64 {
        match self {
            Self::PI => std::f64::consts::PI,
            Self::E => std::f64::consts::E,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Constant(MathConstant),
    Variable(String),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Assign(String, Expr),
    Expr(Expr),
}
