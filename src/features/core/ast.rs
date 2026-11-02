#[derive(Debug, Clone, PartialEq)]
pub enum MathConstant {
    PI,
    E,
}

impl MathConstant {
    pub const NAMES: &[&str] = &["pi", "e"];

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

impl Expr {
    pub fn to_string_pretty(&self) -> String {
        match self {
            Expr::Number(n) => n.to_string(),
            Expr::Constant(MathConstant::PI) => "pi".to_string(),
            Expr::Constant(MathConstant::E) => "e".to_string(),
            Expr::Variable(name) => name.clone(),
            Expr::Neg(inner) => format!("-({})", inner.to_string_pretty()),
            Expr::Add(l, r) => format!("({} + {})", l.to_string_pretty(), r.to_string_pretty()),
            Expr::Sub(l, r) => format!("({} - {})", l.to_string_pretty(), r.to_string_pretty()),
            Expr::Mul(l, r) => format!("{} * {}", l.to_string_pretty(), r.to_string_pretty()),
            Expr::Div(l, r) => format!("({} / {})", l.to_string_pretty(), r.to_string_pretty()),
            Expr::Call(name, args) => {
                let args_str = args
                    .iter()
                    .map(|a| a.to_string_pretty())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, args_str)
            }
        }
    }
}
