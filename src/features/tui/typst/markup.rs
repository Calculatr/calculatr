use num_traits::Signed;

use crate::features::core::Answer;
use crate::features::core::ast::{ANS, Expr, MathConstant, Stmt};
use crate::features::core::format::{Style, as_quantity, fractional_coefficient, prec};
use crate::features::core::functions::{self, TypstForm};
use crate::features::core::notation::{FormattedNumber, NumberFormat, Scale};
use crate::features::core::number::{self, NumFmt, Rational};
use crate::features::core::units::{self, is_unit_expr};
use crate::features::tui::theme::{LineRole, Theme, TokenKind, hex};

pub const FONT_SIZE_PT: f32 = 12.0;

struct Palette {
    plain: String,
    number: String,
    variable: String,
    function: String,
    constant: String,
    operator: String,
    bracket: String,
    punct: String,
}

impl Palette {
    fn new(theme: &Theme, role: LineRole) -> Self {
        let c = |kind| hex(theme.token_color(role, kind));
        Self {
            plain: c(TokenKind::Text),
            number: c(TokenKind::Number),
            variable: c(TokenKind::Variable),
            function: c(TokenKind::Function),
            constant: c(TokenKind::Constant),
            operator: c(TokenKind::Operator),
            bracket: c(TokenKind::Bracket),
            punct: c(TokenKind::Punct),
        }
    }
}

fn document(body: &str, p: &Palette) -> String {
    let font_size = FONT_SIZE_PT;
    format!(
        r##"#set page(width: auto, height: auto, margin: (x: 1pt, y: 1pt), fill: none)
#set text(size: {font_size}pt, fill: rgb("{plain}"))
#let cnum(x) = text(fill: rgb("{number}"), x)
#let cvar(x) = text(fill: rgb("{variable}"), x)
#let cfun(x) = text(fill: rgb("{function}"), x)
#let ccon(x) = text(fill: rgb("{constant}"), x)
#let cpun(x) = text(fill: rgb("{punct}"), x)
#let cbr(x) = text(fill: rgb("{bracket}"), x)
#let cstr(x) = text(fill: rgb("{operator}"), x)
#let cop(x) = math.class("binary", text(fill: rgb("{operator}"), x))
#let cun(x) = math.class("unary", text(fill: rgb("{operator}"), x))
#let crel(x) = math.class("relation", text(fill: rgb("{operator}"), x))
$ {body} $
"##,
        plain = p.plain,
        number = p.number,
        variable = p.variable,
        function = p.function,
        constant = p.constant,
        punct = p.punct,
        bracket = p.bracket,
        operator = p.operator,
    )
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

pub fn input_source(stmt: &Stmt, theme: &Theme) -> String {
    let body = match stmt {
        Stmt::Expr(e) => expr_math(e, NumFmt::Decimal),
        Stmt::Assign(name, e) => format!(
            "{} crel(colon.eq) {}",
            variable(name),
            expr_math(e, NumFmt::Decimal)
        ),
    };
    document(&body, &Palette::new(theme, LineRole::HistoryInput))
}

pub fn answer_source(answer: &Answer, theme: &Theme, numbers: &NumberFormat) -> Option<String> {
    let Answer::Value { label, fmt, .. } = answer else {
        return None;
    };
    let head = match label {
        Some(name) => format!("{} crel(colon.eq)", variable(name)),
        None => String::new(),
    };
    let value = match answer.merged_prefix(numbers) {
        Some((number, unit)) => {
            let mut out = String::new();
            emit_formatted(&number, &mut out);
            format!("{out} thin {}", unit_atom(&unit, 1))
        }
        None => {
            let style = Style::new(*fmt).with_numbers(*numbers);
            expr_math(&answer.quantity()?, style)
        }
    };
    let body = format!("{head} {value}");
    Some(document(&body, &Palette::new(theme, LineRole::Result)))
}

// ---------------------------------------------------------------------------
// Expression -> math
// ---------------------------------------------------------------------------

pub fn expr_math(e: &Expr, style: impl Into<Style>) -> String {
    let mut out = String::new();
    emit(e, style.into(), 0, &mut out);
    out
}

fn typst_symbol(c: MathConstant) -> &'static str {
    match c {
        MathConstant::Inf => "infinity",
        other => other.name(),
    }
}

fn variable(name: &str) -> String {
    let mut it = name.chars();
    match (it.next(), it.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => format!("cvar({c})"),
        _ => format!("cvar(\"{name}\")"),
    }
}

fn leads_with_number(e: &Expr, style: Style, min_prec: u8) -> bool {
    if prec(e, style) < min_prec {
        return false;
    }
    match e {
        Expr::Num(_) | Expr::Neg(_) => true,
        Expr::Add(a, _)
        | Expr::Sub(a, _)
        | Expr::Mul(a, _)
        | Expr::Div(a, _)
        | Expr::Pow(a, _)
        | Expr::Eq(a, _) => leads_with_number(a, style, 0),
        _ => false,
    }
}

fn emit(e: &Expr, style: Style, min_prec: u8, out: &mut String) {
    let parens = prec(e, style) < min_prec;
    if parens {
        out.push_str("cbr(lr(( ");
    }
    emit_inner(e, style, out);
    if parens {
        out.push_str(" )))");
    }
}

fn unit_atom(symbol: &str, exp: i32) -> String {
    let base = format!("ccon(upright(\"{}\"))", symbol.replace("Ohm", "Ω"));
    match exp {
        1 => base,
        e if e < 0 => format!("attach({base}, t: cun(-) cnum({}))", e.unsigned_abs()),
        e => format!("attach({base}, t: cnum({e}))"),
    }
}

fn unit_math(unit: &Expr) -> String {
    let factors = units::flatten(unit);
    let numerator: Vec<String> = factors
        .iter()
        .filter(|(_, e)| *e > 0)
        .map(|(s, e)| unit_atom(s, *e))
        .collect();
    if numerator.is_empty() {
        return factors
            .iter()
            .map(|(s, e)| unit_atom(s, *e))
            .collect::<Vec<_>>()
            .join(" dot.op ");
    }
    let mut out = numerator.join(" dot.op ");
    for (s, e) in factors.iter().filter(|(_, e)| *e < 0) {
        out.push_str(&format!(" cpun(slash) {}", unit_atom(s, -e)));
    }
    out
}

fn emit_inner(e: &Expr, style: Style, out: &mut String) {
    if is_unit_expr(e) {
        out.push_str(&format!("cnum(1) thin {}", unit_math(e)));
        return;
    }
    if let Some((value, unit)) = as_quantity(e) {
        match value {
            Expr::Num(r) => emit_number(r, style, out),
            _ => emit(value, style, 5, out),
        }
        out.push_str(&format!(" thin {}", unit_math(unit)));
        return;
    }
    if let Some((coef, rest)) = fractional_coefficient(e, style) {
        if coef.is_negative() {
            out.push_str("cun(-) ");
        }
        out.push_str("cstr(frac(");
        if coef.numer().abs() != 1.into() {
            out.push_str(&format!("cnum({})", coef.numer().abs()));
            out.push_str(if leads_with_number(rest, style, 2) {
                " cop(times) "
            } else {
                " "
            });
        }
        emit(rest, style, 2, out);
        out.push_str(&format!(", cnum({})))", coef.denom()));
        return;
    }

    match e {
        Expr::Num(r) => emit_number(r, style, out),
        Expr::Constant(c) => out.push_str(&format!("ccon({})", typst_symbol(*c))),
        Expr::Variable(name) if name == ANS => out.push_str("ccon(\"ans\")"),
        Expr::Variable(name) => out.push_str(&variable(name)),
        Expr::Neg(a) => {
            out.push_str("cun(-) ");
            let force =
                matches!(&**a, Expr::Neg(_)) || matches!(&**a, Expr::Num(r) if r.is_negative());
            emit(a, style, if force { 4 } else { 2 }, out);
        }
        Expr::Add(a, b) => {
            emit(a, style, 1, out);
            out.push_str(" cop(+) ");
            emit(b, style, 1, out);
        }
        Expr::Sub(a, b) => {
            emit(a, style, 1, out);
            out.push_str(" cop(-) ");
            emit(b, style, 2, out);
        }
        Expr::Mul(a, b) => {
            emit(a, style, 2, out);
            out.push_str(if leads_with_number(b, style, 2) {
                " cop(times) "
            } else {
                " "
            });
            emit(b, style, 2, out);
        }
        Expr::Div(a, b) => {
            out.push_str("cstr(frac(");
            emit(a, style, 0, out);
            out.push_str(", ");
            emit(b, style, 0, out);
            out.push_str("))");
        }
        Expr::Pow(a, b) => {
            out.push_str("attach(");
            emit(a, style, 5, out);
            out.push_str(", t: ");
            emit(b, style, 0, out);
            out.push(')');
        }
        Expr::Call(name, args) => emit_call(name, args, style, out),
        Expr::Eq(a, b) => {
            emit(a, style, 0, out);
            out.push_str(" crel(=) ");
            emit(b, style, 0, out);
        }
        Expr::Convert(a, unit) => {
            emit(a, style, 0, out);
            out.push_str(&format!(" crel(arrow.r) {}", unit_math(unit)));
        }
        Expr::Unit(symbol) => out.push_str(&unit_atom(symbol, 1)),
    }
}

fn emit_call(name: &str, args: &[Expr], style: Style, out: &mut String) {
    let form = functions::lookup(name).map_or(TypstForm::Named, |f| f.typst);
    match (form, args) {
        (TypstForm::Add, [x, y]) => {
            out.push_str("cstr(");
            emit(x, style, 0, out);
            out.push_str(" + ");
            emit(y, style, 0, out);
            out.push(')');
        }
        (TypstForm::Sub, [x, y]) => {
            out.push_str("cstr(");
            emit(x, style, 0, out);
            out.push_str(" - ");
            emit(y, style, 0, out);
            out.push(')');
        }
        (TypstForm::Mult, [x, y]) => {
            out.push_str("cstr(");
            emit(x, style, 0, out);
            out.push_str(" times ");
            emit(y, style, 0, out);
            out.push(')');
        }
        (TypstForm::Frac, [num, den]) => {
            out.push_str("cstr(frac(");
            emit(num, style, 0, out);
            out.push_str(", ");
            emit(den, style, 0, out);
            out.push_str("))");
        }
        (TypstForm::Sqrt, [x]) => {
            out.push_str("cstr(sqrt(");
            emit(x, style, 0, out);
            out.push_str("))");
        }
        (TypstForm::Cbrt, [x]) => {
            out.push_str("cstr(root(3, ");
            emit(x, style, 0, out);
            out.push_str("))");
        }
        (TypstForm::Abs, [x]) => {
            out.push_str("cbr(abs(");
            emit(x, style, 0, out);
            out.push_str("))");
        }
        (TypstForm::Floor, [x]) => {
            out.push_str("cbr(floor(");
            emit(x, style, 0, out);
            out.push_str("))");
        }
        (TypstForm::Ceil, [x]) => {
            out.push_str("cbr(ceil(");
            emit(x, style, 0, out);
            out.push_str("))");
        }
        _ => {
            out.push_str(&format!("cfun(\"{name}\") "));
            if args.is_empty() {
                out.push_str("cbr(\"()\")");
                return;
            }
            out.push_str("cbr(lr(( ");
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    out.push_str(" cpun(\",\") thin ");
                }
                emit(arg, style, 0, out);
            }
            out.push_str(" )))");
        }
    }
}

fn emit_number(r: &Rational, style: Style, out: &mut String) {
    if style.fmt == NumFmt::Fraction && !number::is_integer(r) {
        if r.is_negative() {
            out.push_str("cun(-) ");
        }
        let magnitude = r.abs();
        out.push_str(&format!(
            "cstr(frac(cnum({}), cnum({})))",
            magnitude.numer(),
            magnitude.denom()
        ));
        return;
    }
    let numbers = match style.fmt {
        NumFmt::Decimal => style.numbers,
        NumFmt::Fraction => NumberFormat::default(),
    };
    emit_formatted(&numbers.format(r), out);
}

fn emit_formatted(n: &FormattedNumber, out: &mut String) {
    if n.negative {
        out.push_str("cun(-) ");
    }
    let mantissa = format!("cnum({})", n.mantissa);
    let times_ten = |e: i32| {
        let exp = if e < 0 {
            format!("cun(-) cnum({})", e.unsigned_abs())
        } else {
            format!("cnum({e})")
        };
        format!("{mantissa} cop(times) attach(cnum(10), t: {exp})")
    };
    let body = match n.scale {
        Scale::None => mantissa,
        Scale::Auto(e) | Scale::Power(e) => times_ten(e),
        Scale::E(e) => format!("cnum(\"{}e{e}\")", n.mantissa),
        Scale::Prefix(p) => format!("{mantissa} thin cnum(upright(\"{}\"))", p.symbol),
    };
    out.push_str(&body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::parser::parse_stmt;

    fn math(src: &str, fmt: NumFmt) -> String {
        let Stmt::Expr(e) = parse_stmt(src).unwrap() else {
            panic!()
        };
        expr_math(&e, fmt)
    }

    #[test]
    fn leaves_are_coloured_by_kind() {
        assert_eq!(math("x+2", NumFmt::Decimal), "cvar(x) cop(+) cnum(2)");
        assert_eq!(math("pi", NumFmt::Decimal), "ccon(pi)");
        assert_eq!(math("foo", NumFmt::Decimal), "cvar(\"foo\")");
    }

    #[test]
    fn structures() {
        assert_eq!(math("1/4", NumFmt::Decimal), "cstr(frac(cnum(1), cnum(4)))");
        assert_eq!(math("x^2", NumFmt::Decimal), "attach(cvar(x), t: cnum(2))");
        assert_eq!(math("sqrt(2)", NumFmt::Decimal), "cstr(sqrt(cnum(2)))");
        assert_eq!(
            math("sin(x)", NumFmt::Decimal),
            "cfun(\"sin\") cbr(lr(( cvar(x) )))"
        );
        assert_eq!(
            math("2(x+1)", NumFmt::Decimal),
            "cnum(2) cbr(lr(( cvar(x) cop(+) cnum(1) )))"
        );
        assert_eq!(math("2*3", NumFmt::Decimal), "cnum(2) cop(times) cnum(3)");
        assert_eq!(math("3y", NumFmt::Decimal), "cnum(3) cvar(y)");
    }

    #[test]
    fn numbers_follow_the_display_format() {
        assert_eq!(math("0.25", NumFmt::Decimal), "cnum(0.25)");
        assert_eq!(
            math("0.25", NumFmt::Fraction),
            "cstr(frac(cnum(1), cnum(4)))"
        );
        assert!(math("2^100", NumFmt::Decimal).contains("attach"));
    }

    #[test]
    fn number_notations() {
        use crate::features::core::notation::Notation;
        let with = |notation| {
            Style::new(NumFmt::Decimal).with_numbers(NumberFormat {
                notation,
                ..NumberFormat::default()
            })
        };
        let Stmt::Expr(e) = parse_stmt("1500").unwrap() else {
            panic!()
        };
        assert_eq!(
            expr_math(&e, with(Notation::Power)),
            "cnum(1.5) cop(times) attach(cnum(10), t: cnum(3))"
        );
        assert_eq!(expr_math(&e, with(Notation::E)), "cnum(\"1.5e3\")");
        assert_eq!(
            expr_math(&e, with(Notation::SiPrefix)),
            "cnum(1.5) thin cnum(upright(\"k\"))"
        );
    }

    #[test]
    fn colours_come_from_the_theme() {
        let theme = Theme::cyberpunk();
        let stmt = parse_stmt("x := 1").unwrap();
        let src = input_source(&stmt, &theme);
        assert!(src.contains(&format!(
            "#let cnum(x) = text(fill: rgb(\"{}\")",
            hex(theme.dim)
        )));
        assert!(src.contains(&format!(
            "#let cbr(x) = text(fill: rgb(\"{}\")",
            hex(theme.bracket)
        )));
        let ans = Answer::value(None, Expr::num(1), NumFmt::Decimal);
        let src = answer_source(&ans, &theme, &NumberFormat::default()).unwrap();
        assert!(src.contains(&format!(
            "#let cnum(x) = text(fill: rgb(\"{}\")",
            hex(theme.number)
        )));
        assert!(
            answer_source(&Answer::Error("x".into()), &theme, &NumberFormat::default()).is_none()
        );
    }
}
