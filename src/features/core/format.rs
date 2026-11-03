use num_traits::Signed;

use super::ast::Expr;
use super::number::{self, NumFmt, Rational};

impl Expr {
    pub fn to_text(&self, fmt: NumFmt) -> String {
        fmt_expr(self, fmt)
    }
}

pub fn fmt_expr(e: &Expr, fmt: NumFmt) -> String {
    let mut out = String::new();
    write(e, fmt, 0, &mut out);
    out
}

pub fn fmt_rational(r: &Rational, fmt: NumFmt) -> String {
    match fmt {
        NumFmt::Decimal => number::to_decimal_string(r),
        NumFmt::Fraction if number::is_integer(r) => r.numer().to_string(),
        NumFmt::Fraction => format!("{}/{}", r.numer(), r.denom()),
    }
}

pub(crate) fn prec(e: &Expr, fmt: NumFmt) -> u8 {
    match e {
        Expr::Eq(..) => 0,
        Expr::Add(..) | Expr::Sub(..) => 1,
        Expr::Mul(..) | Expr::Div(..) => 2,
        Expr::Neg(..) => 3,
        Expr::Pow(..) => 4,
        Expr::Num(r) => {
            if fmt == NumFmt::Fraction && !number::is_integer(r) {
                2
            } else if r.is_negative() {
                3
            } else {
                5
            }
        }
        Expr::Constant(_) | Expr::Variable(_) | Expr::Call(..) => 5,
    }
}

pub(crate) fn fractional_coefficient(e: &Expr, fmt: NumFmt) -> Option<(&Rational, &Expr)> {
    match e {
        Expr::Mul(a, b) if fmt == NumFmt::Fraction => match &**a {
            Expr::Num(r) if !number::is_integer(r) => Some((r, b)),
            _ => None,
        },
        _ => None,
    }
}

fn write(e: &Expr, fmt: NumFmt, min_prec: u8, out: &mut String) {
    let needs_parens = prec(e, fmt) < min_prec;
    if needs_parens {
        out.push('(');
    }
    write_inner(e, fmt, out);
    if needs_parens {
        out.push(')');
    }
}

fn write_inner(e: &Expr, fmt: NumFmt, out: &mut String) {
    if let Some((coef, rest)) = fractional_coefficient(e, fmt) {
        let (p, q) = (coef.numer(), coef.denom());
        let magnitude_one = p.abs() == 1.into();
        if p.is_negative() {
            out.push('-');
        }
        if !magnitude_one {
            out.push_str(&p.abs().to_string());
            out.push('*');
        }
        write(rest, fmt, 2, out);
        out.push('/');
        out.push_str(&q.to_string());
        return;
    }

    match e {
        Expr::Num(r) => out.push_str(&fmt_rational(r, fmt)),
        Expr::Constant(c) => out.push_str(c.name()),
        Expr::Variable(n) => out.push_str(n),
        Expr::Neg(a) => {
            out.push('-');
            let force =
                matches!(&**a, Expr::Neg(_)) || matches!(&**a, Expr::Num(r) if r.is_negative());
            write(a, fmt, if force { 4 } else { 2 }, out);
        }
        Expr::Add(a, b) => {
            write(a, fmt, 1, out);
            out.push_str(" + ");
            write(b, fmt, 1, out);
        }
        Expr::Sub(a, b) => {
            write(a, fmt, 1, out);
            out.push_str(" - ");
            write(b, fmt, 2, out);
        }
        Expr::Mul(a, b) => {
            write(a, fmt, 2, out);
            out.push('*');
            write(b, fmt, 2, out);
        }
        Expr::Div(a, b) => {
            write(a, fmt, 2, out);
            out.push('/');
            write(b, fmt, 3, out);
        }
        Expr::Pow(a, b) => {
            write(a, fmt, 5, out);
            out.push('^');
            write(b, fmt, 3, out);
        }
        Expr::Call(name, args) => {
            out.push_str(name);
            out.push('(');
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write(arg, fmt, 0, out);
            }
            out.push(')');
        }
        Expr::Eq(a, b) => {
            write(a, fmt, 0, out);
            out.push_str(" = ");
            write(b, fmt, 0, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::ast::Stmt;
    use crate::features::core::parser::parse_stmt;

    fn text(s: &str, fmt: NumFmt) -> String {
        let Stmt::Expr(e) = parse_stmt(s).unwrap() else {
            panic!()
        };
        fmt_expr(&e, fmt)
    }

    #[test]
    fn minimal_parentheses_roundtrip() {
        for src in [
            "1 + 2*3",
            "(1 + 2)*3",
            "a - (b + c)",
            "a/(b*c)",
            "2^3^2",
            "(2^3)^2",
            "-(a + b)",
            "x^-1",
            "f(1, 2)",
        ] {
            let once = text(src, NumFmt::Decimal);
            assert_eq!(text(&once, NumFmt::Decimal), once, "{src}");
        }
        assert_eq!(text("(1+2)*3", NumFmt::Decimal), "(1 + 2)*3");
        assert_eq!(text("a-(b+c)", NumFmt::Decimal), "a - (b + c)");
        assert_eq!(text("a/(b*c)", NumFmt::Decimal), "a/(b*c)");
    }

    #[test]
    fn number_styles() {
        assert_eq!(text("0.25", NumFmt::Decimal), "0.25");
        assert_eq!(text("0.25", NumFmt::Fraction), "1/4");
        assert_eq!(text("0.25^2", NumFmt::Fraction), "(1/4)^2");
        assert_eq!(text("3y = 2", NumFmt::Decimal), "3*y = 2");
    }
}
