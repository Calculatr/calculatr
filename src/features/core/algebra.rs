use num_traits::{One, Signed, Zero};

use super::ast::Expr;
use super::functions;
use super::number::{self, Rational, from_f64, pow_int, sqrt_exact};

// ---------------------------------------------------------------------------
// Exact simplification
// ---------------------------------------------------------------------------

pub fn simplify(e: &Expr) -> Result<Expr, String> {
    Ok(match e {
        Expr::Num(_) | Expr::Constant(_) | Expr::Variable(_) => e.clone(),
        Expr::Neg(a) => scale(simplify(a)?, -Rational::one()),
        Expr::Add(a, b) => sum(vec![
            (Rational::one(), simplify(a)?),
            (Rational::one(), simplify(b)?),
        ]),
        Expr::Sub(a, b) => sum(vec![
            (Rational::one(), simplify(a)?),
            (-Rational::one(), simplify(b)?),
        ]),
        Expr::Mul(a, b) => product(simplify(a)?, simplify(b)?),
        Expr::Div(a, b) => quotient(simplify(a)?, simplify(b)?)?,
        Expr::Pow(a, b) => power(simplify(a)?, simplify(b)?)?,
        Expr::Eq(a, b) => Expr::Eq(Box::new(simplify(a)?), Box::new(simplify(b)?)),
        Expr::Call(name, args) => {
            let args = args.iter().map(simplify).collect::<Result<Vec<_>, _>>()?;
            if name == "frac" && args.len() == 2 {
                let mut it = args.into_iter();
                let (n, d) = (it.next().unwrap(), it.next().unwrap());
                return quotient(n, d);
            }
            let exact: Option<Vec<Rational>> = args
                .iter()
                .map(|a| match a {
                    Expr::Num(r) => Some(r.clone()),
                    _ => None,
                })
                .collect();
            match exact.and_then(|v| functions::call_exact(name, &v)) {
                Some(r) => Expr::Num(r),
                None => Expr::Call(name.clone(), args),
            }
        }
    })
}

fn scale(e: Expr, coefficient: Rational) -> Expr {
    match e {
        Expr::Num(r) => Expr::Num(r * coefficient),
        Expr::Add(..) | Expr::Sub(..) => sum(vec![(coefficient, e)]),
        other => product(Expr::Num(coefficient), other),
    }
}

fn sum(items: Vec<(Rational, Expr)>) -> Expr {
    fn collect(
        e: Expr,
        sign: Rational,
        terms: &mut Vec<(Rational, Expr)>,
        constant: &mut Rational,
    ) {
        match e {
            Expr::Add(a, b) => {
                collect(*a, sign.clone(), terms, constant);
                collect(*b, sign, terms, constant);
            }
            Expr::Sub(a, b) => {
                collect(*a, sign.clone(), terms, constant);
                collect(*b, -sign, terms, constant);
            }
            Expr::Neg(a) => collect(*a, -sign, terms, constant),
            Expr::Num(r) => *constant += sign * r,
            Expr::Mul(a, b) if matches!(*a, Expr::Num(_)) => {
                let Expr::Num(c) = *a else { unreachable!() };
                push_term(terms, sign * c, *b);
            }
            other => push_term(terms, sign, other),
        }
    }

    fn push_term(terms: &mut Vec<(Rational, Expr)>, coefficient: Rational, base: Expr) {
        match terms.iter_mut().find(|(_, b)| *b == base) {
            Some((c, _)) => *c += coefficient,
            None => terms.push((coefficient, base)),
        }
    }

    let mut terms = Vec::new();
    let mut constant = Rational::zero();
    for (sign, e) in items {
        collect(e, sign, &mut terms, &mut constant);
    }
    terms.retain(|(c, _)| !c.is_zero());

    let mut result: Option<Expr> = None;
    for (c, base) in terms {
        let magnitude = c.abs();
        let term = if magnitude.is_one() {
            base
        } else {
            Expr::Mul(Box::new(Expr::Num(magnitude)), Box::new(base))
        };
        result = Some(match (result, c.is_negative()) {
            (None, false) => term,
            (None, true) => Expr::Neg(Box::new(term)),
            (Some(acc), false) => Expr::Add(Box::new(acc), Box::new(term)),
            (Some(acc), true) => Expr::Sub(Box::new(acc), Box::new(term)),
        });
    }

    match result {
        None => Expr::Num(constant),
        Some(acc) if constant.is_zero() => acc,
        Some(acc) if constant.is_negative() => {
            Expr::Sub(Box::new(acc), Box::new(Expr::Num(-constant)))
        }
        Some(acc) => Expr::Add(Box::new(acc), Box::new(Expr::Num(constant))),
    }
}

fn product(a: Expr, b: Expr) -> Expr {
    fn collect(e: Expr, coefficient: &mut Rational, factors: &mut Vec<(Expr, Rational)>) {
        match e {
            Expr::Mul(a, b) => {
                collect(*a, coefficient, factors);
                collect(*b, coefficient, factors);
            }
            Expr::Neg(a) => {
                *coefficient = -coefficient.clone();
                collect(*a, coefficient, factors);
            }
            Expr::Num(r) => *coefficient *= r,
            Expr::Pow(base, exp) if matches!(&*exp, Expr::Num(n) if number::is_integer(n)) => {
                let Expr::Num(n) = *exp else { unreachable!() };
                push_factor(factors, *base, n);
            }
            other => push_factor(factors, other, Rational::one()),
        }
    }

    fn push_factor(factors: &mut Vec<(Expr, Rational)>, base: Expr, exp: Rational) {
        match factors.iter_mut().find(|(b, _)| *b == base) {
            Some((_, e)) => *e += exp,
            None => factors.push((base, exp)),
        }
    }

    let mut coefficient = Rational::one();
    let mut factors = Vec::new();
    collect(a, &mut coefficient, &mut factors);
    collect(b, &mut coefficient, &mut factors);

    if coefficient.is_zero() {
        return Expr::Num(Rational::zero());
    }

    let mut rest: Option<Expr> = None;
    for (base, exp) in factors {
        if exp.is_zero() {
            continue;
        }
        let factor = if exp.is_one() {
            base
        } else {
            Expr::Pow(Box::new(base), Box::new(Expr::Num(exp)))
        };
        rest = Some(match rest {
            None => factor,
            Some(acc) => Expr::Mul(Box::new(acc), Box::new(factor)),
        });
    }

    match rest {
        None => Expr::Num(coefficient),
        Some(r) if coefficient.is_one() => r,
        Some(r) if (-coefficient.clone()).is_one() => Expr::Neg(Box::new(r)),
        Some(r) => Expr::Mul(Box::new(Expr::Num(coefficient)), Box::new(r)),
    }
}

fn quotient(a: Expr, b: Expr) -> Result<Expr, String> {
    match &b {
        Expr::Num(d) if d.is_zero() => Err("Division by zero".to_string()),
        Expr::Num(d) => Ok(product(a, Expr::Num(d.recip()))),
        _ if matches!(&a, Expr::Num(n) if n.is_zero()) => Ok(a),
        _ if a == b => Ok(Expr::num(1)),
        _ => Ok(Expr::Div(Box::new(a), Box::new(b))),
    }
}

fn power(base: Expr, exp: Expr) -> Result<Expr, String> {
    if let Expr::Num(e) = &exp {
        if e.is_zero() {
            return Ok(Expr::num(1));
        }
        if e.is_one() {
            return Ok(base);
        }
        if let Expr::Num(b) = &base {
            if b.is_zero() && e.is_negative() {
                return Err("Division by zero".to_string());
            }
            if let Some(r) = pow_int(b, e) {
                return Ok(Expr::Num(r));
            }
            if number::is_integer(e) {
                return Err("Number too large".to_string());
            }
            if e.denom() == &2.into()
                && let Some(root) = sqrt_exact(b)
                && let Some(r) = pow_int(&root, &Rational::from_integer(e.numer().clone()))
            {
                return Ok(Expr::Num(r));
            }
        }
        if let Expr::Pow(inner_base, inner_exp) = &base
            && let Expr::Num(m) = &**inner_exp
            && number::is_integer(m)
            && number::is_integer(e)
        {
            return Ok(Expr::Pow(inner_base.clone(), Box::new(Expr::Num(m * e))));
        }
    }
    if matches!(&base, Expr::Num(b) if b.is_one()) {
        return Ok(base);
    }
    Ok(Expr::Pow(Box::new(base), Box::new(exp)))
}

// ---------------------------------------------------------------------------
// Floating point evaluation
// ---------------------------------------------------------------------------

pub fn eval_f64(e: &Expr) -> Result<f64, String> {
    let finite = |v: f64, what: &str| {
        if v.is_nan() {
            Err(format!("{what} is undefined"))
        } else if v.is_infinite() {
            Err(format!("{what} overflows"))
        } else {
            Ok(v)
        }
    };
    match e {
        Expr::Num(r) => Ok(number::to_f64(r)),
        Expr::Constant(c) => Ok(c.value()),
        Expr::Variable(name) => Err(format!("Unknown variable '{name}'")),
        Expr::Neg(a) => Ok(-eval_f64(a)?),
        Expr::Add(a, b) => finite(eval_f64(a)? + eval_f64(b)?, "Sum"),
        Expr::Sub(a, b) => finite(eval_f64(a)? - eval_f64(b)?, "Difference"),
        Expr::Mul(a, b) => finite(eval_f64(a)? * eval_f64(b)?, "Product"),
        Expr::Div(a, b) => {
            let (l, r) = (eval_f64(a)?, eval_f64(b)?);
            if r == 0.0 {
                Err("Division by zero".to_string())
            } else {
                finite(l / r, "Quotient")
            }
        }
        Expr::Pow(a, b) => finite(eval_f64(a)?.powf(eval_f64(b)?), "Power"),
        Expr::Call(name, args) => {
            let values = args.iter().map(eval_f64).collect::<Result<Vec<_>, _>>()?;
            functions::call(name, &values)
        }
        Expr::Eq(..) => Err("An equation has no numeric value".to_string()),
    }
}

pub fn fold_numeric(e: &Expr) -> Result<Expr, String> {
    if matches!(e, Expr::Num(_)) {
        return Ok(e.clone());
    }
    if e.is_closed() && !matches!(e, Expr::Eq(..)) {
        let v = eval_f64(e)?;
        return from_f64(v)
            .map(Expr::Num)
            .ok_or_else(|| "Result is not finite".to_string());
    }
    e.try_map(&mut fold_numeric)
}

pub fn first_free_variable(e: &Expr) -> Option<&str> {
    match e {
        Expr::Variable(n) => Some(n),
        _ => e.children().into_iter().find_map(first_free_variable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::ast::Stmt;
    use crate::features::core::format::fmt_expr;
    use crate::features::core::number::NumFmt;
    use crate::features::core::parser::parse_stmt;

    fn simp(s: &str) -> String {
        let Stmt::Expr(e) = parse_stmt(s).unwrap() else {
            panic!()
        };
        fmt_expr(&simplify(&e).unwrap(), NumFmt::Fraction)
    }

    #[test]
    fn exact_rational_folding() {
        assert_eq!(simp("0.25"), "1/4");
        assert_eq!(simp("1/4 + 0.25"), "1/2");
        assert_eq!(simp("0.1 + 0.2"), "3/10");
        assert_eq!(simp("2^10"), "1024");
        assert_eq!(simp("2^-2"), "1/4");
        assert_eq!(simp("4^(3/2)"), "8");
    }

    #[test]
    fn identities_and_like_terms() {
        assert_eq!(simp("x + 0"), "x");
        assert_eq!(simp("x * 1"), "x");
        assert_eq!(simp("x * 0"), "0");
        assert_eq!(simp("x^0"), "1");
        assert_eq!(simp("2x + 3x"), "5*x");
        assert_eq!(simp("x - x"), "0");
        assert_eq!(simp("-(x + 1)"), "-x - 1");
        assert_eq!(simp("x*x"), "x^2");
        assert_eq!(simp("(x^2)^3"), "x^6");
        assert_eq!(simp("2*3*x"), "6*x");
        assert_eq!(simp("x/2"), "x/2");
        assert_eq!(simp("(2x)/4"), "x/2");
    }

    #[test]
    fn functions_stay_symbolic_unless_exact() {
        assert_eq!(simp("sqrt(16)"), "4");
        assert_eq!(simp("sqrt(2)"), "sqrt(2)");
        assert_eq!(simp("frac(1, 4)"), "1/4");
        assert_eq!(simp("sin(0)"), "0");
        assert_eq!(simp("sin(1)"), "sin(1)");
        assert_eq!(simp("pi - pi"), "0");
    }

    #[test]
    fn division_by_zero_is_an_error() {
        let Stmt::Expr(e) = parse_stmt("1/0").unwrap() else {
            panic!()
        };
        assert!(simplify(&e).is_err());
        let Stmt::Expr(e) = parse_stmt("0^-1").unwrap() else {
            panic!()
        };
        assert!(simplify(&e).is_err());
    }

    #[test]
    fn float_evaluation() {
        let Stmt::Expr(e) = parse_stmt("pi/2").unwrap() else {
            panic!()
        };
        assert!((eval_f64(&e).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let Stmt::Expr(e) = parse_stmt("sqrt(-1)").unwrap() else {
            panic!()
        };
        assert!(eval_f64(&e).is_err());
        let Stmt::Expr(e) = parse_stmt("x + 1").unwrap() else {
            panic!()
        };
        assert!(eval_f64(&e).is_err());
    }
}
