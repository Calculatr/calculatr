use num_traits::ToPrimitive;

use super::algebra;
use super::ast::Expr;
use super::number::{self, Rational};
use super::units::{self, Dimension, dimension_text};

pub fn split(e: &Expr) -> Result<Option<(Expr, Dimension)>, String> {
    let bx = Box::new;
    Ok(Some(match e {
        Expr::Unit(symbol) => {
            let u = units::resolve(symbol).ok_or_else(|| format!("Unknown unit '{symbol}'"))?;
            (Expr::Num(u.factor), u.dimension)
        }
        Expr::Num(_) | Expr::Constant(_) | Expr::Variable(_) => (e.clone(), Dimension::NONE),
        Expr::Neg(a) => {
            let Some((s, d)) = split(a)? else {
                return Ok(None);
            };
            (Expr::Neg(bx(s)), d)
        }
        Expr::Fact(a) => {
            let Some((s, d)) = split(a)? else {
                return Ok(None);
            };
            if !d.is_dimensionless() {
                return Err(format!(
                    "Factorial needs a dimensionless argument (got {})",
                    dimension_text(d)
                ));
            }
            (Expr::Fact(bx(s)), Dimension::NONE)
        }
        Expr::Add(a, b) | Expr::Sub(a, b) => {
            let (Some((sa, da)), Some((sb, db))) = (split(a)?, split(b)?) else {
                return Ok(None);
            };
            if da != db {
                if algebra::first_free_variable(e).is_some() {
                    return Ok(None);
                }
                return Err(format!(
                    "Incompatible units: {} and {}",
                    dimension_text(da),
                    dimension_text(db)
                ));
            }
            let s = match e {
                Expr::Add(..) => Expr::Add(bx(sa), bx(sb)),
                _ => Expr::Sub(bx(sa), bx(sb)),
            };
            (s, da)
        }
        Expr::Mul(a, b) => {
            let (Some((sa, da)), Some((sb, db))) = (split(a)?, split(b)?) else {
                return Ok(None);
            };
            (Expr::Mul(bx(sa), bx(sb)), da * db)
        }
        Expr::Div(a, b) => {
            let (Some((sa, da)), Some((sb, db))) = (split(a)?, split(b)?) else {
                return Ok(None);
            };
            (Expr::Div(bx(sa), bx(sb)), da / db)
        }
        Expr::Pow(a, b) => {
            let (Some((sa, da)), Some((sb, db))) = (split(a)?, split(b)?) else {
                return Ok(None);
            };
            if !db.is_dimensionless() {
                return Err("Exponents can't have units".to_string());
            }
            let d = if da.is_dimensionless() {
                da
            } else {
                let exp = match algebra::simplify(&sb)? {
                    Expr::Num(r) => r,
                    _ => return Err("A quantity with a unit needs a numeric exponent".into()),
                };
                power_dimension(da, &exp)?
            };
            (Expr::Pow(bx(sa), bx(sb)), d)
        }
        Expr::Call(name, args) => {
            let mut scalars = Vec::with_capacity(args.len());
            let mut dims = Vec::with_capacity(args.len());
            for arg in args {
                let Some((s, d)) = split(arg)? else {
                    return Ok(None);
                };
                scalars.push(s);
                dims.push(d);
            }
            let d = match (name.as_str(), dims.as_slice()) {
                ("abs", [d]) => *d,
                ("sign", [_]) => Dimension::NONE,
                ("sqrt", [d]) => power_dimension(*d, &Rational::new(1.into(), 2.into()))?,
                ("cbrt", [d]) => power_dimension(*d, &Rational::new(1.into(), 3.into()))?,
                ("mult", [a, b]) => *a * *b,
                ("frac", [a, b]) => *a / *b,
                ("add" | "sub", [a, b]) if a == b => *a,
                _ if dims.iter().all(|d| d.is_dimensionless()) => Dimension::NONE,
                _ => {
                    return Err(format!(
                        "{name}() needs a dimensionless argument (got {})",
                        dims.iter()
                            .find(|d| !d.is_dimensionless())
                            .map_or_else(String::new, |d| dimension_text(*d))
                    ));
                }
            };
            (Expr::Call(name.clone(), scalars), d)
        }
        Expr::Eq(..) => return Ok(None),
        Expr::Convert(..) => return Err("'->' can only be used once, at the end".to_string()),
    }))
}

fn power_dimension(d: Dimension, exp: &Rational) -> Result<Dimension, String> {
    let (Some(p), Some(q)) = (exp.numer().to_i64(), exp.denom().to_i64()) else {
        return Err("Unit exponent too large".to_string());
    };
    d.checked_pow(p, q).ok_or_else(|| {
        format!(
            "Can't raise {} to the power {}",
            dimension_text(d),
            if q == 1 {
                p.to_string()
            } else {
                format!("{p}/{q}")
            }
        )
    })
}

pub fn convert(scalar: Expr, dimension: Dimension, target: &Expr) -> Result<(Expr, Expr), String> {
    let (factor, target_dim) = units::measure(target)?;
    if target_dim != dimension {
        return Err(format!(
            "Can't convert {} to {}",
            dimension_text(dimension),
            units::unit_text(target)
        ));
    }
    Ok((scale_down(scalar, &factor)?, target.clone()))
}

pub fn present(
    scalar: Expr,
    dimension: Dimension,
    atoms: &[String],
) -> Result<(Expr, Expr), String> {
    let (unit, factor) = units::display_unit(dimension, atoms);
    Ok((scale_down(scalar, &factor)?, unit))
}

fn scale_down(scalar: Expr, factor: &Rational) -> Result<Expr, String> {
    if number::is_integer(factor) && factor.numer() == &1.into() {
        return algebra::simplify(&scalar);
    }
    algebra::simplify(&Expr::Div(
        Box::new(scalar),
        Box::new(Expr::Num(factor.clone())),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::ast::Stmt;
    use crate::features::core::parser::parse_stmt;

    fn split_text(src: &str) -> Result<Option<(String, String)>, String> {
        let Stmt::Expr(e) = parse_stmt(src).unwrap() else {
            panic!()
        };
        let simplified = algebra::simplify(&e)?;
        Ok(split(&simplified)?.map(|(s, d)| {
            let s = algebra::simplify(&s).unwrap();
            (
                crate::features::core::format::fmt_expr(&s, number::NumFmt::Fraction),
                dimension_text(d),
            )
        }))
    }

    #[test]
    fn scalars_are_in_si_units() {
        assert_eq!(
            split_text("3_km + 200_m").unwrap(),
            Some(("3200".into(), "m".into()))
        );
        assert_eq!(
            split_text("2_N * 3_m").unwrap(),
            Some(("6".into(), "kg*m^2/s^2".into()))
        );
        assert_eq!(
            split_text("10_m / 4_m").unwrap(),
            Some(("5/2".into(), "no unit".into()))
        );
        assert_eq!(
            split_text("sqrt(16_m^2)").unwrap(),
            Some(("4".into(), "m".into()))
        );
        assert_eq!(
            split_text("x * 2_m + 1_m").unwrap(),
            Some(("2*x + 1".into(), "m".into()))
        );
    }

    #[test]
    fn mismatches_are_errors() {
        assert!(
            split_text("1_m + 1_s")
                .unwrap_err()
                .contains("Incompatible")
        );
        assert!(split_text("1_m + 1").unwrap_err().contains("Incompatible"));
        assert!(
            split_text("sin(1_m)")
                .unwrap_err()
                .contains("dimensionless")
        );
        assert!(split_text("2^(1_s)").unwrap_err().contains("Exponents"));
        assert!(split_text("sqrt(2_m)").unwrap_err().contains("power 1/2"));
        assert_eq!(split_text("x + 1_m").unwrap(), None);
    }
}
