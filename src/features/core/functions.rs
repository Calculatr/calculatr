use super::number::{self, Rational};
use num_complex::Complex64;
use num_traits::{Float, Inv, Signed, Zero};

pub type ExactFn = fn(&[Rational]) -> Option<Rational>;

pub type ComplexFn = fn(&[Complex64]) -> Complex64;

pub enum Eval {
    F1(fn(f64) -> f64),                         // INFO: Simple 1 var in -> 1 out
    F2(fn(f64, f64) -> f64),                    // INFO: Simple 2 var in -> 1 out
    Checked(fn(&[f64]) -> Result<f64, String>), // INFO: Complex mult var in -> 1 out (x/0 returns error)
    Command,                                    // INFO: Command such as clear()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TypstForm {
    Named,
    Sqrt,
    Add,
    Sub,
    Mult,
    Frac,
    Abs,
    Floor,
    Ceil,
}

pub struct FunctionDef {
    pub name: &'static str,
    pub params: &'static [&'static str],
    pub doc: &'static str,
    pub eval: Eval,
    pub exact: Option<ExactFn>,
    pub complex: Option<ComplexFn>,
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
            complex: None,
            typst: TypstForm::Named,
        }
    }

    const fn exact(mut self, f: fn(&[Rational]) -> Option<Rational>) -> Self {
        self.exact = Some(f);
        self
    }

    const fn complex(mut self, f: ComplexFn) -> Self {
        self.complex = Some(f);
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

fn exact_add(a: &[Rational]) -> Option<Rational> {
    match a {
        [x, y] => Some(x + y),
        _ => None,
    }
}

fn exact_sub(a: &[Rational]) -> Option<Rational> {
    match a {
        [x, y] => Some(x - y),
        _ => None,
    }
}

fn exact_mult(a: &[Rational]) -> Option<Rational> {
    match a {
        [x, y] => Some(x * y),
        _ => None,
    }
}

fn exact_frac(a: &[Rational]) -> Option<Rational> {
    match a {
        [n, d] if !d.is_zero() => Some(n / d),
        _ => None,
    }
}

fn exact_floor(a: &[Rational]) -> Option<Rational> {
    match a {
        [x] => Some(x.floor()),
        _ => None,
    }
}

fn exact_ceil(a: &[Rational]) -> Option<Rational> {
    match a {
        [x] => Some(x.ceil()),
        _ => None,
    }
}

fn exact_round(a: &[Rational]) -> Option<Rational> {
    match a {
        [x] => Some(x.round()),
        _ => None,
    }
}

fn exact_trunc(a: &[Rational]) -> Option<Rational> {
    match a {
        [x] => Some(x.trunc()),
        _ => None,
    }
}

fn exact_fract(a: &[Rational]) -> Option<Rational> {
    match a {
        [x] => Some(x.fract()),
        _ => None,
    }
}

fn exact_signum(a: &[Rational]) -> Option<Rational> {
    match a {
        [x] => Some(x.signum()),
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
    FunctionDef::new("sin", &["x"], "Sine, x in radians", Eval::F1(f64::sin))
        .exact(zero_at_zero)
        .complex(|a| a[0].sin()),
    FunctionDef::new("cos", &["x"], "Cosine, x in radians", Eval::F1(f64::cos))
        .exact(one_at_zero)
        .complex(|a| a[0].cos()),
    FunctionDef::new("tan", &["x"], "Tangent, x in radians", Eval::F1(f64::tan))
        .exact(zero_at_zero)
        .complex(|a| a[0].tan()),
    FunctionDef::new("asin", &["x"], "Inverse sine", Eval::F1(f64::asin))
        .exact(zero_at_zero)
        .complex(|a| a[0].asin()),
    FunctionDef::new("acos", &["x"], "Inverse cosine", Eval::F1(f64::acos))
        .exact(zero_at_one)
        .complex(|a| a[0].acos()),
    FunctionDef::new("atan", &["x"], "Inverse tangent", Eval::F1(f64::atan))
        .exact(zero_at_zero)
        .complex(|a| a[0].atan()),
    FunctionDef::new("sec", &["x"], "Secant", Eval::F1(|x| 1.0 / x.cos()))
        .exact(one_at_zero)
        .complex(|a| a[0].cos().inv()),
    FunctionDef::new("csc", &["x"], "Cosecant", Eval::F1(|x| 1.0 / x.sin()))
        .complex(|a| a[0].sin().inv()),
    FunctionDef::new("cot", &["x"], "Cotangent", Eval::F1(|x| 1.0 / x.tan())).complex(|a| {
        let s = a[0].sin();
        let c = a[0].cos();
        c / s
    }),
    FunctionDef::new(
        "atan2",
        &["y", "x"],
        "Arc tangent of y/x in radians (all quadrants)",
        Eval::F2(|y, x| y.atan2(x)),
    ),
    // Hyperbolic geometry
    FunctionDef::new("sinh", &["x"], "Sinh", Eval::F1(f64::sinh))
        .exact(zero_at_zero)
        .complex(|a| a[0].sinh()),
    FunctionDef::new("cosh", &["x"], "Cosh", Eval::F1(f64::cosh))
        .exact(one_at_zero)
        .complex(|a| a[0].cosh()),
    FunctionDef::new("tanh", &["x"], "Tanh", Eval::F1(f64::tanh))
        .exact(zero_at_zero)
        .complex(|a| a[0].tanh()),
    FunctionDef::new("asinh", &["x"], "Inverse sinh", Eval::F1(f64::asinh))
        .exact(zero_at_zero)
        .complex(|a| a[0].asinh()),
    FunctionDef::new("acosh", &["x"], "Inverse cosh", Eval::F1(f64::acosh))
        .exact(zero_at_one)
        .complex(|a| a[0].acosh()),
    FunctionDef::new("atanh", &["x"], "Inverse tanh", Eval::F1(f64::atanh))
        .exact(zero_at_zero)
        .complex(|a| a[0].atanh()),
    // Powers and Logarithms
    FunctionDef::new("exp", &["x"], "e to the power x", Eval::F1(f64::exp))
        .exact(one_at_zero)
        .complex(|a| a[0].exp()),
    FunctionDef::new("ln", &["x"], "Natural logarithm", Eval::F1(f64::ln))
        .exact(zero_at_one)
        .complex(|a| a[0].ln()),
    FunctionDef::new("log", &["x"], "Base-10 logarithm", Eval::F1(f64::log10))
        .exact(zero_at_one)
        .complex(|a| a[0].log(10.0)),
    // Algebra
    FunctionDef::new("sqrt", &["x"], "Square root", Eval::F1(f64::sqrt))
        .exact(exact_sqrt)
        .complex(|a| a[0].sqrt())
        .typst(TypstForm::Sqrt),
    FunctionDef::new("abs", &["x"], "Absolute value", Eval::F1(f64::abs))
        .exact(exact_abs)
        .complex(|a| Complex64::new(a[0].norm(), 0.0))
        .typst(TypstForm::Abs),
    FunctionDef::new("add", &["x", "y"], "Addition", Eval::F2(|x, y| x + y))
        .exact(exact_add)
        .complex(|a| a[0] + a[1])
        .typst(TypstForm::Add),
    FunctionDef::new("sub", &["x", "y"], "Subtraction", Eval::F2(|x, y| x - y))
        .exact(exact_sub)
        .complex(|a| a[0] - a[1])
        .typst(TypstForm::Sub),
    FunctionDef::new(
        "mult",
        &["x", "y"],
        "Multiplication",
        Eval::F2(|x, y| x * y),
    )
    .exact(exact_mult)
    .complex(|a| a[0] * a[1])
    .typst(TypstForm::Mult),
    FunctionDef::new("frac", &["num", "den"], "Fraction", Eval::Checked(num_frac))
        .exact(exact_frac)
        .complex(|a| a[0] / a[1])
        .typst(TypstForm::Frac),
    FunctionDef::new(
        "floor",
        &["x"],
        "Floor function, greatest integer <= x",
        Eval::F1(f64::floor),
    )
    .exact(exact_floor)
    .complex(|a| Complex64::new(a[0].re.floor(), a[0].im.floor()))
    .typst(TypstForm::Floor),
    FunctionDef::new(
        "ceil",
        &["x"],
        "Ceiling function, smallest integer >= x",
        Eval::F1(f64::ceil),
    )
    .exact(exact_ceil)
    .complex(|a| Complex64::new(a[0].re.ceil(), a[0].im.ceil()))
    .typst(TypstForm::Ceil),
    FunctionDef::new(
        "round",
        &["x"],
        "Round to nearest integer",
        Eval::F1(f64::round),
    )
    .exact(exact_round)
    .complex(|a| Complex64::new(a[0].re.round(), a[0].im.round())),
    FunctionDef::new(
        "trunc",
        &["x"],
        "Truncate fractional part (round toward zero)",
        Eval::F1(f64::trunc),
    )
    .exact(exact_trunc)
    .complex(|a| Complex64::new(a[0].re.trunc(), a[0].im.trunc())),
    FunctionDef::new(
        "fract",
        &["x"],
        "Fractional part, x - trunc(x)",
        Eval::F1(f64::fract),
    )
    .exact(exact_fract)
    .complex(|a| Complex64::new(a[0].re.fract(), a[0].im.fract())),
    FunctionDef::new(
        "sign",
        &["x"],
        "Signum function (-1, 0, or 1)",
        Eval::F1(f64::signum),
    )
    .exact(exact_signum)
    .complex(|a| {
        let norm = a[0].norm();
        if norm == 0.0 {
            Complex64::zero()
        } else {
            a[0] / norm
        }
    }),
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

pub fn call(name: &str, args: &[f64]) -> Result<f64, String> {
    let def = lookup(name).ok_or_else(|| format!("Unknown function: {name}"))?;
    if args.len() != def.params.len() {
        return Err(arity_error(def));
    }
    let value = match (&def.eval, args) {
        (Eval::F1(f), [x]) => f(*x),
        (Eval::F2(f), [x, y]) => f(*x, *y),
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
    } else if value.is_infinite() && args.iter().all(|a| a.is_finite()) {
        Err(format!("{name}() overflows or is infinite for this input"))
    } else {
        Ok(value)
    }
}

pub fn call_complex(name: &str, args: &[Complex64]) -> Result<Complex64, String> {
    let def = lookup(name).ok_or_else(|| format!("Unknown function: {name}"))?;
    if args.len() != def.params.len() {
        return Err(arity_error(def));
    }
    let f = def
        .complex
        .ok_or_else(|| format!("{name}() doesn't support complex numbers"))?;
    let value = f(args);
    if value.re.is_nan() || value.im.is_nan() {
        Err(format!("{name}() is undefined for this input"))
    } else if !value.is_finite() && args.iter().all(|a| a.is_finite()) {
        Err(format!("{name}() overflows or is infinite for this input"))
    } else {
        Ok(value)
    }
}

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
