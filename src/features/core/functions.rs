use super::number::{self, Rational};
use num_traits::{Signed, Zero};

pub type ExactFn = fn(&[Rational]) -> Option<Rational>;

pub enum Eval {
    F1(fn(f64) -> f64),
    Checked(fn(&[f64]) -> Result<f64, String>),
    Command,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TypstForm {
    Named,
    Sqrt,
    Frac,
    Abs,
}

pub struct FunctionDef {
    pub name: &'static str,
    pub params: &'static [&'static str],
    pub doc: &'static str,
    pub eval: Eval,
    pub exact: Option<ExactFn>,
    pub typst: TypstForm,
}

impl FunctionDef {
    const fn new(
        name: &'static str,
        params: &'static [&'static str],
        doc: &'static str,
        eval: Eval,
    ) -> Self {
        Self {
            name,
            params,
            doc,
            eval,
            exact: None,
            typst: TypstForm::Named,
        }
    }

    const fn exact(mut self, f: fn(&[Rational]) -> Option<Rational>) -> Self {
        self.exact = Some(f);
        self
    }

    const fn typst(mut self, form: TypstForm) -> Self {
        self.typst = form;
        self
    }

    pub fn is_command(&self) -> bool {
        matches!(self.eval, Eval::Command)
    }
}

// ---------------------------------------------------------------------------
// Exact hooks
// ---------------------------------------------------------------------------

macro_rules! at_point {
    ($name:ident, $input:expr => $output:expr) => {
        fn $name(a: &[Rational]) -> Option<Rational> {
            match a {
                [x] if *x == number::int($input) => Some(number::int($output)),
                _ => None,
            }
        }
    };
}

at_point!(zero_at_zero, 0 => 0);
at_point!(one_at_zero, 0 => 1);
at_point!(zero_at_one, 1 => 0);

fn exact_sqrt(a: &[Rational]) -> Option<Rational> {
    match a {
        [x] => number::sqrt_exact(x),
        _ => None,
    }
}

fn exact_abs(a: &[Rational]) -> Option<Rational> {
    match a {
        [x] => Some(x.abs()),
        _ => None,
    }
}

fn exact_frac(a: &[Rational]) -> Option<Rational> {
    match a {
        [n, d] if !d.is_zero() => Some(n / d),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Numeric hooks
// ---------------------------------------------------------------------------

fn num_frac(a: &[f64]) -> Result<f64, String> {
    match a {
        [_, d] if *d == 0.0 => Err("Division by zero".into()),
        [n, d] => Ok(n / d),
        _ => unreachable!("arity is checked by `call`"),
    }
}

// ---------------------------------------------------------------------------
// The registry
// ---------------------------------------------------------------------------

pub static FUNCTIONS: &[FunctionDef] = &[
    // Trigonometry (radians)
    FunctionDef::new("sin", &["x"], "Sine, x in radians", Eval::F1(f64::sin)).exact(zero_at_zero),
    FunctionDef::new("cos", &["x"], "Cosine, x in radians", Eval::F1(f64::cos)).exact(one_at_zero),
    FunctionDef::new("tan", &["x"], "Tangent, x in radians", Eval::F1(f64::tan))
        .exact(zero_at_zero),
    FunctionDef::new("asin", &["x"], "Inverse sine", Eval::F1(f64::asin)).exact(zero_at_zero),
    FunctionDef::new("acos", &["x"], "Inverse cosine", Eval::F1(f64::acos)).exact(zero_at_one),
    FunctionDef::new("atan", &["x"], "Inverse tangent", Eval::F1(f64::atan)).exact(zero_at_zero),
    // Exponentials / logarithms
    FunctionDef::new("exp", &["x"], "e to the power x", Eval::F1(f64::exp)).exact(one_at_zero),
    FunctionDef::new("ln", &["x"], "Natural logarithm", Eval::F1(f64::ln)).exact(zero_at_one),
    FunctionDef::new("log", &["x"], "Base-10 logarithm", Eval::F1(f64::log10)).exact(zero_at_one),
    // Algebra
    FunctionDef::new("sqrt", &["x"], "Square root", Eval::F1(f64::sqrt))
        .exact(exact_sqrt)
        .typst(TypstForm::Sqrt),
    FunctionDef::new("abs", &["x"], "Absolute value", Eval::F1(f64::abs))
        .exact(exact_abs)
        .typst(TypstForm::Abs),
    FunctionDef::new("frac", &["num", "den"], "Fraction", Eval::Checked(num_frac))
        .exact(exact_frac)
        .typst(TypstForm::Frac),
    // Commands
    FunctionDef::new("clear", &[], "Clear the history", Eval::Command),
    FunctionDef::new("delvar", &["name"], "Delete a variable", Eval::Command),
];

pub fn lookup(name: &str) -> Option<&'static FunctionDef> {
    FUNCTIONS.iter().find(|f| f.name == name)
}

fn arity_error(def: &FunctionDef) -> String {
    let n = def.params.len();
    format!(
        "{}() expects {} argument{}",
        def.name,
        n,
        if n == 1 { "" } else { "s" }
    )
}

/// Numerically evaluates `name(args)`.
pub fn call(name: &str, args: &[f64]) -> Result<f64, String> {
    let def = lookup(name).ok_or_else(|| format!("Unknown function: {name}"))?;
    if args.len() != def.params.len() {
        return Err(arity_error(def));
    }
    let value = match (&def.eval, args) {
        (Eval::F1(f), [x]) => f(*x),
        (Eval::Checked(f), _) => f(args)?,
        (Eval::Command, _) => {
            return Err(format!(
                "{name}() is a command and can't be used in an expression"
            ));
        }
        _ => return Err(arity_error(def)),
    };
    if value.is_nan() {
        Err(format!("{name}() is undefined for this input"))
    } else if value.is_infinite() {
        Err(format!("{name}() overflows or is infinite for this input"))
    } else {
        Ok(value)
    }
}

/// Exact evaluation, if the function supports it for these arguments.
pub fn call_exact(name: &str, args: &[Rational]) -> Option<Rational> {
    let def = lookup(name)?;
    if args.len() != def.params.len() {
        return None;
    }
    def.exact.and_then(|f| f(args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique() {
        let mut names: Vec<_> = FUNCTIONS.iter().map(|f| f.name).collect();
        names.sort();
        let len = names.len();
        names.dedup();
        assert_eq!(len, names.len());
    }

    #[test]
    fn numeric_call_checks_arity_and_domain() {
        assert!(call("sin", &[0.0]).unwrap().abs() < 1e-12);
        assert!(call("sin", &[]).is_err());
        assert!(call("sqrt", &[-1.0]).is_err());
        assert!(call("ln", &[0.0]).is_err());
        assert_eq!(call("frac", &[1.0, 4.0]).unwrap(), 0.25);
        assert!(call("frac", &[1.0, 0.0]).is_err());
        assert!(call("clear", &[]).is_err());
    }

    #[test]
    fn exact_hooks() {
        assert_eq!(call_exact("sqrt", &[number::int(9)]), Some(number::int(3)));
        assert_eq!(call_exact("sqrt", &[number::int(2)]), None);
        assert_eq!(call_exact("cos", &[number::int(0)]), Some(number::int(1)));
        assert_eq!(call_exact("sin", &[number::int(1)]), None);
    }
}
