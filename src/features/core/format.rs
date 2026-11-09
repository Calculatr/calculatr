use num_traits::Signed;

use super::ast::Expr;
use super::notation::{FormattedNumber, NumberFormat};
use super::number::{self, NumFmt, Rational};
use super::units::{self, UNIT_SEPARATOR, is_unit_expr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fmt: NumFmt,
    pub numbers: NumberFormat,
    pub parseable: bool,
}

impl Style {
    pub fn new(fmt: NumFmt) -> Self {
        Self {
            fmt,
            numbers: NumberFormat::default(),
            parseable: false,
        }
    }

    pub fn with_numbers(mut self, numbers: NumberFormat) -> Self {
        self.numbers = numbers;
        self
    }

    pub fn parseable(mut self) -> Self {
        self.parseable = true;
        self.numbers = self.numbers.parseable();
        self
    }

    pub fn decimal(&self, r: &Rational) -> Option<FormattedNumber> {
        (self.fmt == NumFmt::Decimal).then(|| self.numbers.format(r))
    }
}

impl From<NumFmt> for Style {
    fn from(fmt: NumFmt) -> Self {
        Style::new(fmt)
    }
}

impl Expr {
    pub fn to_text(&self, style: impl Into<Style>) -> String {
        fmt_expr(self, style)
    }
}

pub fn fmt_expr(e: &Expr, style: impl Into<Style>) -> String {
    let mut out = String::new();
    write(e, style.into(), 0, &mut out);
    out
}

pub fn fmt_rational(r: &Rational, style: Style) -> String {
    match style.fmt {
        NumFmt::Decimal => style.numbers.format(r).render(style.parseable),
        NumFmt::Fraction if number::is_integer(r) => r.numer().to_string(),
        NumFmt::Fraction => format!("{}/{}", r.numer(), r.denom()),
    }
}

pub(crate) fn prec(e: &Expr, style: Style) -> u8 {
    if is_unit_expr(e) {
        return 4;
    }
    if let Some((value, _)) = as_quantity(e) {
        return if matches!(value, Expr::Num(r) if r.is_negative()) {
            3
        } else {
            4
        };
    }
    match e {
        Expr::Eq(..) | Expr::Convert(..) => 0,
        Expr::Add(..) | Expr::Sub(..) => 1,
        Expr::Mul(..) | Expr::Div(..) => 2,
        Expr::Neg(..) => 3,
        Expr::Pow(..) => 4,
        Expr::Num(r) => {
            let fraction = style.fmt == NumFmt::Fraction && !number::is_integer(r);
            let product = style
                .decimal(r)
                .is_some_and(|f| f.is_product(style.parseable));
            if fraction || product {
                2
            } else if r.is_negative() {
                3
            } else {
                5
            }
        }
        Expr::Constant(_) | Expr::Variable(_) | Expr::Call(..) | Expr::Unit(_) => 5,
    }
}

pub(crate) fn as_quantity(e: &Expr) -> Option<(&Expr, &Expr)> {
    match e {
        Expr::Mul(a, b) if is_unit_expr(b) && !is_unit_expr(a) => Some((a, b)),
        _ => None,
    }
}

pub(crate) fn is_plain_number(r: &Rational, style: Style) -> bool {
    match style.fmt {
        NumFmt::Fraction => number::is_integer(r),
        NumFmt::Decimal => !style.numbers.format(r).is_product(style.parseable),
    }
}

pub(crate) fn fractional_coefficient(e: &Expr, style: Style) -> Option<(&Rational, &Expr)> {
    match e {
        Expr::Mul(a, b) if style.fmt == NumFmt::Fraction && !is_unit_expr(b) => match &**a {
            Expr::Num(r) if !number::is_integer(r) => Some((r, b)),
            _ => None,
        },
        _ => None,
    }
}

fn write(e: &Expr, style: Style, min_prec: u8, out: &mut String) {
    let needs_parens = prec(e, style) < min_prec;
    if needs_parens {
        out.push('(');
    }
    write_inner(e, style, out);
    if needs_parens {
        out.push(')');
    }
}

fn write_inner(e: &Expr, style: Style, out: &mut String) {
    if is_unit_expr(e) {
        out.push_str(&format!("1{UNIT_SEPARATOR}{}", units::unit_text(e)));
        return;
    }
    if let Some((value, unit)) = as_quantity(e) {
        match value {
            Expr::Num(r) if is_plain_number(r, style) => out.push_str(&fmt_rational(r, style)),
            _ => {
                out.push('(');
                write(value, style, 0, out);
                out.push(')');
            }
        }
        out.push(UNIT_SEPARATOR);
        out.push_str(&units::unit_text(unit));
        return;
    }
    if let Some((coef, rest)) = fractional_coefficient(e, style) {
        let (p, q) = (coef.numer(), coef.denom());
        let magnitude_one = p.abs() == 1.into();
        if p.is_negative() {
            out.push('-');
        }
        if !magnitude_one {
            out.push_str(&p.abs().to_string());
            out.push('*');
        }
        write(rest, style, 2, out);
        out.push('/');
        out.push_str(&q.to_string());
        return;
    }

    match e {
        Expr::Num(r) => out.push_str(&fmt_rational(r, style)),
        Expr::Constant(c) => out.push_str(c.name()),
        Expr::Variable(n) => out.push_str(n),
        Expr::Neg(a) => {
            out.push('-');
            let force =
                matches!(&**a, Expr::Neg(_)) || matches!(&**a, Expr::Num(r) if r.is_negative());
            write(a, style, if force { 4 } else { 2 }, out);
        }
        Expr::Add(a, b) => {
            write(a, style, 1, out);
            out.push_str(" + ");
            write(b, style, 1, out);
        }
        Expr::Sub(a, b) => {
            write(a, style, 1, out);
            out.push_str(" - ");
            write(b, style, 2, out);
        }
        Expr::Mul(a, b) => {
            write(a, style, 2, out);
            out.push('*');
            write(b, style, 2, out);
        }
        Expr::Div(a, b) => {
            write(a, style, 2, out);
            out.push('/');
            write(b, style, 3, out);
        }
        Expr::Pow(a, b) => {
            write(a, style, 5, out);
            out.push('^');
            write(b, style, 3, out);
        }
        Expr::Call(name, args) => {
            out.push_str(name);
            out.push('(');
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write(arg, style, 0, out);
            }
            out.push(')');
        }
        Expr::Eq(a, b) => {
            write(a, style, 0, out);
            out.push_str(" = ");
            write(b, style, 0, out);
        }
        Expr::Convert(a, unit) => {
            write(a, style, 0, out);
            out.push_str(&format!(
                " {} {}",
                units::CONVERSION_ARROW,
                units::unit_text(unit)
            ));
        }
        Expr::Unit(u) => out.push_str(&format!("1{UNIT_SEPARATOR}{u}")),
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
    fn notation_aware_parentheses() {
        use crate::features::core::notation::{ExponentStep, Notation, NumberFormat};
        let numbers = NumberFormat {
            notation: Notation::Power,
            exponent_step: ExponentStep::Scientific,
            ..NumberFormat::default()
        };
        let style = Style::new(NumFmt::Decimal).with_numbers(numbers);
        let Stmt::Expr(e) = parse_stmt("1500^2 + y/1500").unwrap() else {
            panic!()
        };
        assert_eq!(fmt_expr(&e, style), "(1.5*10^3)^2 + y/(1.5*10^3)");

        let si = Style::new(NumFmt::Decimal).with_numbers(NumberFormat {
            notation: Notation::SiPrefix,
            ..NumberFormat::default()
        });
        assert_eq!(fmt_expr(&e, si), "1.5k^2 + y/1.5k");
        assert_eq!(fmt_expr(&e, si.parseable()), "(1.5*10^3)^2 + y/(1.5*10^3)");
        let frac = Style::new(NumFmt::Fraction).with_numbers(numbers);
        assert_eq!(fmt_expr(&e, frac), "1500^2 + y/1500");
    }

    #[test]
    fn quantities_roundtrip() {
        for src in [
            "5_N",
            "9.81_m/s^2",
            "(a + b)_km",
            "(5_m)^2",
            "-5_m",
            "x/2_s",
            "2*3_kg*m",
            "72_km/h -> m/s",
            "1_s^-1",
        ] {
            let once = text(src, NumFmt::Decimal);
            assert_eq!(once, src);
            assert_eq!(text(&once, NumFmt::Decimal), once, "{src}");
        }
        assert_eq!(text("0.75_m", NumFmt::Fraction), "(3/4)_m");
    }

    #[test]
    fn number_styles() {
        assert_eq!(text("0.25", NumFmt::Decimal), "0.25");
        assert_eq!(text("0.25", NumFmt::Fraction), "1/4");
        assert_eq!(text("0.25^2", NumFmt::Fraction), "(1/4)^2");
        assert_eq!(text("3y = 2", NumFmt::Decimal), "3*y = 2");
    }
}
