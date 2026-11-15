use num_complex::Complex64;
use num_traits::{One, Signed, ToPrimitive, Zero};

use super::ast::{Expr, MathConstant};
use super::functions;
use super::number::{self, Rational, from_f64, pow_int, sqrt_exact};

// ---------------------------------------------------------------------------
// Exact simplification
// ---------------------------------------------------------------------------

pub fn simplify(e: &Expr) -> Result<Expr, String> {
    Ok(match e {
        Expr::Num(_) | Expr::Constant(_) | Expr::Variable(_) | Expr::Unit(_) => e.clone(),
        Expr::Convert(a, u) => Expr::Convert(Box::new(simplify(a)?), u.clone()),
        Expr::Neg(a) => scale(simplify(a)?, -Rational::one()),
        Expr::Fact(a) => {
            let inner = simplify(a)?;
            match &inner {
                Expr::Num(r) => match number::factorial(r) {
                    Some(res) => Expr::Num(res),
                    None => {
                        if !number::is_integer(r) || r.is_negative() {
                            return Err(
                                "Factorial is only defined for non-negative integers".into()
                            );
                        }
                        return Err("Factorial input too large".into());
                    }
                },
                _ => Expr::Fact(Box::new(inner)),
            }
        }
        Expr::Prc(a) => quotient(simplify(a)?, Expr::num(100))?,
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
        Expr::Num(r) => Expr::Num(number::mul(&r, &coefficient)),
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
            Expr::Num(r) => *constant = number::add(constant, &number::mul(&sign, &r)),
            Expr::Mul(a, b) if matches!(*a, Expr::Num(_)) => {
                let Expr::Num(c) = *a else { unreachable!() };
                push_term(terms, number::mul(&sign, &c), *b);
            }
            other => push_term(terms, sign, other),
        }
    }

    fn push_term(terms: &mut Vec<(Rational, Expr)>, coefficient: Rational, base: Expr) {
        match terms.iter_mut().find(|(_, b)| *b == base) {
            Some((c, _)) => *c = number::add(c, &coefficient),
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
            Expr::Num(r) => *coefficient = number::mul(coefficient, &r),
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
    let check = |v: f64, operands: [f64; 2], what: &str| {
        if v.is_nan() {
            Err(format!("{what} is undefined"))
        } else if v.is_infinite() && operands.iter().all(|x| x.is_finite()) {
            Err(format!("{what} overflows"))
        } else {
            Ok(v)
        }
    };
    let both =
        |a: &Expr, b: &Expr| -> Result<(f64, f64), String> { Ok((eval_f64(a)?, eval_f64(b)?)) };
    match e {
        Expr::Num(r) => Ok(number::to_f64(r)),
        Expr::Constant(c) => c
            .value()
            .ok_or_else(|| "Result is complex (needs complex evaluation)".to_string()),
        Expr::Variable(name) => Err(format!("Unknown variable '{name}'")),
        Expr::Unit(u) => Err(format!("'{u}' is a unit, not a number")),
        Expr::Convert(a, _) => eval_f64(a),
        Expr::Neg(a) => Ok(-eval_f64(a)?),
        Expr::Fact(a) => {
            let v = eval_f64(a)?;
            let n = number::to_int_exact(v)
                .ok_or_else(|| "Factorial requires a non-negative integer".to_string())?;
            if n < 0 {
                return Err("Factorial is only defined for non-negative integers".into());
            }
            if n > 170 {
                return Err("Factorial overflows f64".into());
            }
            let mut acc = 1.0f64;
            for i in 2..=n {
                acc *= i as f64;
            }
            check(acc, [v, 0.0], "Factorial")
        }
        Expr::Prc(a) => Ok(eval_f64(a)? / 100.0),
        Expr::Add(a, b) => {
            let (l, r) = both(a, b)?;
            check(l + r, [l, r], "Sum")
        }
        Expr::Sub(a, b) => {
            let (l, r) = both(a, b)?;
            check(l - r, [l, r], "Difference")
        }
        Expr::Mul(a, b) => {
            let (l, r) = both(a, b)?;
            check(l * r, [l, r], "Product")
        }
        Expr::Div(a, b) => {
            let (l, r) = both(a, b)?;
            if r == 0.0 {
                Err("Division by zero".to_string())
            } else {
                check(l / r, [l, r], "Quotient")
            }
        }
        Expr::Pow(a, b) => {
            let (l, r) = both(a, b)?;
            if (l.abs() == 1.0 && r.is_infinite()) || (l.is_infinite() && r == 0.0) {
                return Err("Power is undefined".to_string());
            }
            check(l.powf(r), [l, r], "Power")
        }
        Expr::Call(name, args) => {
            let values = args.iter().map(eval_f64).collect::<Result<Vec<_>, _>>()?;
            functions::call(name, &values)
        }
        Expr::Eq(..) => Err("An equation has no numeric value".to_string()),
    }
}

fn infinity(sign_of: f64) -> Expr {
    let inf = Expr::Constant(MathConstant::Inf);
    if sign_of < 0.0 {
        Expr::Neg(Box::new(inf))
    } else {
        inf
    }
}

pub fn resolve_infinity(e: &Expr) -> Result<Expr, String> {
    if e.is_closed()
        && e.contains_constant(MathConstant::Inf)
        && !e.contains_constant(MathConstant::I)
        && !matches!(e, Expr::Eq(..))
    {
        let v = eval_f64(e)?;
        return Ok(if v.is_infinite() {
            infinity(v)
        } else if v == 0.0 {
            Expr::Num(Rational::zero())
        } else {
            e.clone()
        });
    }
    e.try_map(&mut resolve_infinity)
}

pub fn fold_numeric(e: &Expr, complex: bool) -> Result<Expr, String> {
    if matches!(e, Expr::Num(_)) {
        return Ok(e.clone());
    }
    if e.is_closed() && !matches!(e, Expr::Eq(..)) {
        return fold_closed(e, complex);
    }
    e.try_map(&mut |c| fold_numeric(c, complex))
}

fn fold_closed(e: &Expr, complex: bool) -> Result<Expr, String> {
    let real = if e.contains_constant(MathConstant::I) {
        Err("Result is complex".to_string())
    } else {
        eval_f64(e)
    };
    match real {
        Ok(v) if v.is_infinite() => Ok(infinity(v)),
        Ok(v) => from_f64(v)
            .map(Expr::Num)
            .ok_or_else(|| "Result is not finite".to_string()),
        Err(_) if complex => {
            let z = clean(eval_c64(e)?);
            match (from_f64(z.re), from_f64(z.im)) {
                (Some(re), Some(im)) => Ok(complex_expr(re, im)),
                _ => Err("Result is not finite".to_string()),
            }
        }
        Err(err) => Err(err),
    }
}

// ---------------------------------------------------------------------------
// Complex numbers
// ---------------------------------------------------------------------------

pub fn eval_c64(e: &Expr) -> Result<Complex64, String> {
    let check = |v: Complex64, operands: [Complex64; 2], what: &str| {
        if v.re.is_nan() || v.im.is_nan() {
            Err(format!("{what} is undefined"))
        } else if !v.is_finite() && operands.iter().all(|x| x.is_finite()) {
            Err(format!("{what} overflows"))
        } else {
            Ok(v)
        }
    };
    let both = |a: &Expr, b: &Expr| -> Result<(Complex64, Complex64), String> {
        Ok((eval_c64(a)?, eval_c64(b)?))
    };
    match e {
        Expr::Num(r) => Ok(Complex64::new(number::to_f64(r), 0.0)),
        Expr::Constant(c) => Ok(match c.value() {
            Some(v) => Complex64::new(v, 0.0),
            None => Complex64::new(0.0, 1.0),
        }),
        Expr::Variable(name) => Err(format!("Unknown variable '{name}'")),
        Expr::Unit(u) => Err(format!("'{u}' is a unit, not a number")),
        Expr::Convert(a, _) => eval_c64(a),
        Expr::Neg(a) => Ok(-eval_c64(a)?),
        Expr::Fact(a) => {
            let z = eval_c64(a)?;
            if z.im != 0.0 {
                return Err("Factorial not supported for complex numbers".into());
            }
            let n = number::to_int_exact(z.re)
                .ok_or_else(|| "Factorial requires a non-negative integer".to_string())?;
            if n < 0 {
                return Err("Factorial is only defined for non-negative integers".into());
            }
            if n > 170 {
                return Err("Factorial overflows".into());
            }
            let mut acc = 1.0f64;
            for i in 2..=n {
                acc *= i as f64;
            }
            check(
                Complex64::new(acc, 0.0),
                [z, Complex64::zero()],
                "Factorial",
            )
        }
        Expr::Prc(a) => Ok(eval_c64(a)? / 100.0),
        Expr::Add(a, b) => {
            let (l, r) = both(a, b)?;
            check(l + r, [l, r], "Sum")
        }
        Expr::Sub(a, b) => {
            let (l, r) = both(a, b)?;
            check(l - r, [l, r], "Difference")
        }
        Expr::Mul(a, b) => {
            let (l, r) = both(a, b)?;
            check(l * r, [l, r], "Product")
        }
        Expr::Div(a, b) => {
            let (l, r) = both(a, b)?;
            if r.re == 0.0 && r.im == 0.0 {
                Err("Division by zero".to_string())
            } else {
                check(l / r, [l, r], "Quotient")
            }
        }
        Expr::Pow(a, b) => {
            let (l, r) = both(a, b)?;
            if l.re == 0.0 && l.im == 0.0 {
                return if r.im == 0.0 && r.re > 0.0 {
                    Ok(Complex64::new(0.0, 0.0))
                } else {
                    Err("Power is undefined".to_string())
                };
            }
            let power = if r.im == 0.0 && r.re.fract() == 0.0 && r.re.abs() <= 64.0 {
                l.powi(r.re as i32)
            } else {
                l.powc(r)
            };
            check(power, [l, r], "Power")
        }
        Expr::Call(name, args) => {
            let values = args.iter().map(eval_c64).collect::<Result<Vec<_>, _>>()?;
            functions::call_complex(name, &values)
        }
        Expr::Eq(..) => Err("An equation has no numeric value".to_string()),
    }
}

fn clean(z: Complex64) -> Complex64 {
    let tolerance = 1e-12 * z.re.abs().max(z.im.abs());
    Complex64::new(
        if z.re.abs() <= tolerance { 0.0 } else { z.re },
        if z.im.abs() <= tolerance { 0.0 } else { z.im },
    )
}

pub fn complex_expr(re: Rational, im: Rational) -> Expr {
    let unit = || Expr::Constant(MathConstant::I);
    let imaginary = |b: &Rational| {
        if b.is_one() {
            unit()
        } else {
            Expr::Mul(Box::new(Expr::Num(b.clone())), Box::new(unit()))
        }
    };
    if im.is_zero() {
        return Expr::Num(re);
    }
    let magnitude = imaginary(&im.abs());
    if re.is_zero() {
        return if im.is_negative() {
            Expr::Neg(Box::new(magnitude))
        } else {
            magnitude
        };
    }
    let (l, r) = (Box::new(Expr::Num(re)), Box::new(magnitude));
    if im.is_negative() {
        Expr::Sub(l, r)
    } else {
        Expr::Add(l, r)
    }
}

type Cx = (Rational, Rational);

fn cx_mul(a: &Cx, b: &Cx) -> Cx {
    (&a.0 * &b.0 - &a.1 * &b.1, &a.0 * &b.1 + &a.1 * &b.0)
}

fn cx_div(a: &Cx, b: &Cx) -> Option<Cx> {
    let norm = &b.0 * &b.0 + &b.1 * &b.1;
    if norm.is_zero() {
        return None;
    }
    let (re, im) = cx_mul(a, &(b.0.clone(), -b.1.clone()));
    Some((re / &norm, im / &norm))
}

fn cx_pow(z: &Cx, n: i64) -> Option<Cx> {
    if n.abs() > 1024 {
        return None;
    }
    let (mut result, mut base): (Cx, Cx) = ((Rational::one(), Rational::zero()), z.clone());
    let mut k = n.unsigned_abs();
    while k > 0 {
        if k & 1 == 1 {
            result = cx_mul(&result, &base);
        }
        base = cx_mul(&base, &base);
        k >>= 1;
    }
    if n < 0 {
        cx_div(&(Rational::one(), Rational::zero()), &result)
    } else {
        Some(result)
    }
}

fn exact_complex(e: &Expr) -> Option<Cx> {
    let zero = Rational::zero;
    match e {
        Expr::Num(r) => Some((r.clone(), zero())),
        Expr::Constant(MathConstant::I) => Some((zero(), Rational::one())),
        Expr::Neg(a) => exact_complex(a).map(|(re, im)| (-re, -im)),
        Expr::Add(a, b) => {
            let (x, y) = (exact_complex(a)?, exact_complex(b)?);
            Some((x.0 + y.0, x.1 + y.1))
        }
        Expr::Sub(a, b) => {
            let (x, y) = (exact_complex(a)?, exact_complex(b)?);
            Some((x.0 - y.0, x.1 - y.1))
        }
        Expr::Mul(a, b) => Some(cx_mul(&exact_complex(a)?, &exact_complex(b)?)),
        Expr::Div(a, b) => cx_div(&exact_complex(a)?, &exact_complex(b)?),
        Expr::Pow(a, b) => {
            let (exp_re, exp_im) = exact_complex(b)?;
            if !exp_im.is_zero() || !number::is_integer(&exp_re) {
                return None;
            }
            cx_pow(&exact_complex(a)?, exp_re.numer().to_i64()?)
        }
        Expr::Call(name, args) => match (name.as_str(), args.as_slice()) {
            ("sqrt", [x]) => {
                let (re, im) = exact_complex(x)?;
                if !im.is_zero() {
                    return None;
                }
                if re.is_negative() {
                    sqrt_exact(&-re).map(|root| (zero(), root))
                } else {
                    sqrt_exact(&re).map(|root| (root, zero()))
                }
            }
            ("abs", [x]) => {
                let (re, im) = exact_complex(x)?;
                sqrt_exact(&(&re * &re + &im * &im)).map(|norm| (norm, zero()))
            }
            ("frac", [a, b]) => cx_div(&exact_complex(a)?, &exact_complex(b)?),
            _ => None,
        },
        _ => None,
    }
}

fn mentions_call(e: &Expr, name: &str) -> bool {
    matches!(e, Expr::Call(n, _) if n == name)
        || e.children().into_iter().any(|c| mentions_call(c, name))
}

pub fn complex_exact(e: &Expr) -> Expr {
    if e.is_closed()
        && !matches!(e, Expr::Eq(..))
        && (e.contains_constant(MathConstant::I) || mentions_call(e, "sqrt"))
        && let Some((re, im)) = exact_complex(e)
    {
        return complex_expr(re, im);
    }
    e.try_map::<()>(&mut |c| Ok(complex_exact(c)))
        .expect("infallible")
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
