use super::ast::{Expr, MathConstant, Stmt};
use super::names::SEPARATOR;
use super::number::parse_decimal;
use super::units::{self, CONVERSION_ARROW, UNIT_SEPARATOR};

pub struct Parser<'a> {
    chars: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.as_bytes(),
            pos: 0,
        }
    }

    pub fn parse(&mut self) -> Result<Stmt, String> {
        let stmt = self.parse_statement()?;
        self.skip_whitespace();
        match self.peek() {
            None => Ok(stmt),
            Some(b'=') => Err("Only one '=' is allowed in an equation".to_string()),
            Some(b':') => Err("Unexpected ':' (assignments use ':=')".to_string()),
            Some(_) => Err("Unexpected characters at the end of the expression".to_string()),
        }
    }

    // --- statements ---------------------------------------------------------

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        self.skip_whitespace();
        let start = self.pos;

        if let Some(name) = self.parse_identifier() {
            self.skip_whitespace();
            if self.match_str(":=") {
                if MathConstant::from_name(&name).is_some() {
                    return Err(format!("'{name}' is a constant and read-only"));
                }
                let value = self.parse_converted()?;
                return Ok(Stmt::Assign(name, value));
            }
        }

        self.pos = start;
        Ok(Stmt::Expr(self.parse_converted()?))
    }

    fn parse_converted(&mut self) -> Result<Expr, String> {
        let value = self.parse_equation()?;
        self.skip_whitespace();
        if !self.match_str(CONVERSION_ARROW) {
            return Ok(value);
        }
        self.skip_whitespace();
        if self.peek().is_none() {
            return Err(format!("Expected a unit after '{CONVERSION_ARROW}'"));
        }
        let unit = self.parse_unit_chain()?;
        Ok(Expr::Convert(Box::new(value), Box::new(unit)))
    }

    fn parse_unit_chain(&mut self) -> Result<Expr, String> {
        let rest = std::str::from_utf8(&self.chars[self.pos..]).map_err(|e| e.to_string())?;
        let (unit, used) = units::parse_chain(rest)?;
        self.pos += used;
        Ok(unit)
    }

    fn with_unit(&mut self, value: Expr) -> Result<Expr, String> {
        if self.peek() != Some(UNIT_SEPARATOR as u8) {
            return Ok(value);
        }
        self.pos += 1;
        let unit = self.parse_unit_chain()?;
        Ok(Expr::Mul(Box::new(value), Box::new(unit)))
    }

    fn parse_equation(&mut self) -> Result<Expr, String> {
        let lhs = self.parse_expression()?;
        self.skip_whitespace();
        if self.match_char(b'=') {
            let rhs = self.parse_expression()?;
            return Ok(Expr::Eq(Box::new(lhs), Box::new(rhs)));
        }
        Ok(lhs)
    }

    // --- expressions --------------------------------------------------------

    fn parse_expression(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_term()?;
        loop {
            self.skip_whitespace();
            if self.match_char(b'+') {
                let right = self.parse_term()?;
                left = left.bin(Expr::Add, right);
            } else if !self.at_arrow() && self.match_char(b'-') {
                let right = self.parse_term()?;
                left = left.bin(Expr::Sub, right);
            } else {
                return Ok(left);
            }
        }
    }

    fn at_arrow(&self) -> bool {
        self.chars[self.pos..].starts_with(CONVERSION_ARROW.as_bytes())
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            if self.match_char(b'*') {
                let right = self.parse_unary()?;
                left = left.bin(Expr::Mul, right);
            } else if self.match_char(b'/') {
                let right = self.parse_unary()?;
                left = left.bin(Expr::Div, right);
            } else if self.starts_implicit_factor() {
                let right = self.parse_unary()?;
                left = left.bin(Expr::Mul, right);
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        self.skip_whitespace();
        if self.match_char(b'-') {
            return Ok(Expr::Neg(Box::new(self.parse_unary()?)));
        }
        if self.match_char(b'+') {
            return self.parse_unary();
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Result<Expr, String> {
        let base = self.parse_primary()?;
        self.skip_whitespace();
        if self.match_char(b'^') {
            let exp = self.parse_unary()?;
            return Ok(base.bin(Expr::Pow, exp));
        }
        Ok(base)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        self.skip_whitespace();
        let Some(c) = self.peek() else {
            return Err("Unexpected end of expression".to_string());
        };

        if c.is_ascii_digit() || c == b'.' {
            return self.parse_number();
        }

        if let Some(ident) = self.parse_identifier() {
            self.skip_whitespace();
            if self.peek() == Some(b'(') {
                self.pos += 1;
                return self.parse_call(ident);
            }
            return Ok(match MathConstant::from_name(&ident) {
                Some(constant) => Expr::Constant(constant),
                None => Expr::Variable(ident),
            });
        }

        if c == b'(' {
            self.pos += 1;
            let expr = self.parse_expression()?;
            self.skip_whitespace();
            if !self.match_char(b')') {
                return Err("Expected closing parenthesis ')'".to_string());
            }
            return self.with_unit(expr);
        }

        if c == b'<' && self.chars.get(self.pos + 1) == Some(&b'>') {
            return Err("Unfilled placeholder '<>'".to_string());
        }

        Err(format!("Unexpected character: {}", c as char))
    }

    fn parse_number(&mut self) -> Result<Expr, String> {
        let start = self.pos;
        while self.peek().is_some_and(|b| b.is_ascii_digit() || b == b'.') {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.chars[start..self.pos]).unwrap_or_default();
        let value = parse_decimal(text)
            .map(Expr::Num)
            .ok_or_else(|| format!("Could not parse number: {text}"))?;
        self.with_unit(value)
    }

    fn parse_call(&mut self, name: String) -> Result<Expr, String> {
        let mut args = Vec::new();
        self.skip_whitespace();
        if !self.match_char(b')') {
            loop {
                args.push(self.parse_expression()?);
                self.skip_whitespace();
                if !self.match_char(b',') {
                    break;
                }
            }
            if !self.match_char(b')') {
                return Err("Expected closing parenthesis ')'".to_string());
            }
        }
        Ok(Expr::Call(name, args))
    }

    // --- lexical helpers ----------------------------------------------------

    fn parse_identifier(&mut self) -> Option<String> {
        if !self.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
            return None;
        }
        let start = self.pos;
        loop {
            while self
                .peek()
                .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                self.pos += 1;
            }
            let continues = self.peek() == Some(SEPARATOR as u8)
                && self
                    .chars
                    .get(self.pos + 1)
                    .is_some_and(|b| b.is_ascii_alphabetic());
            if !continues {
                break;
            }
            self.pos += 1;
        }
        Some(String::from_utf8_lossy(&self.chars[start..self.pos]).into_owned())
    }

    fn starts_implicit_factor(&self) -> bool {
        self.peek()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'(')
    }

    fn peek(&self) -> Option<u8> {
        self.chars.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn match_char(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn match_str(&mut self, expected: &str) -> bool {
        if self.chars[self.pos..].starts_with(expected.as_bytes()) {
            self.pos += expected.len();
            true
        } else {
            false
        }
    }
}

pub fn parse_stmt(input: &str) -> Result<Stmt, String> {
    Parser::new(input).parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::number::{int, parse_decimal};

    fn expr(s: &str) -> Expr {
        match parse_stmt(s).unwrap() {
            Stmt::Expr(e) => e,
            other => panic!("expected expression, got {other:?}"),
        }
    }

    fn var(s: &str) -> Expr {
        Expr::Variable(s.into())
    }

    #[test]
    fn precedence_and_implicit_mul() {
        assert_eq!(
            expr("1+2*3"),
            Expr::num(1).bin(Expr::Add, Expr::num(2).bin(Expr::Mul, Expr::num(3)))
        );
        assert_eq!(expr("3y"), Expr::num(3).bin(Expr::Mul, var("y")));
        assert_eq!(
            expr("2(x+1)"),
            Expr::num(2).bin(Expr::Mul, var("x").bin(Expr::Add, Expr::num(1)))
        );
    }

    #[test]
    fn power_is_right_associative_and_binds_tighter_than_neg() {
        assert_eq!(
            expr("2^3^2"),
            Expr::num(2).bin(Expr::Pow, Expr::num(3).bin(Expr::Pow, Expr::num(2)))
        );
        assert_eq!(
            expr("-2^2"),
            Expr::Neg(Box::new(Expr::num(2).bin(Expr::Pow, Expr::num(2))))
        );
        assert_eq!(
            expr("2^-1"),
            Expr::num(2).bin(Expr::Pow, Expr::Neg(Box::new(Expr::num(1))))
        );
    }

    #[test]
    fn decimal_literal_is_exact() {
        assert_eq!(expr("0.25"), Expr::Num(parse_decimal("0.25").unwrap()));
        assert_eq!(expr("1/4").children().len(), 2);
    }

    #[test]
    fn assignment_uses_colon_equals() {
        assert_eq!(
            parse_stmt("x := 3").unwrap(),
            Stmt::Assign("x".into(), Expr::num(3))
        );
        assert!(matches!(
            parse_stmt("x = 3").unwrap(),
            Stmt::Expr(Expr::Eq(..))
        ));
    }

    #[test]
    fn variables_can_hold_equations() {
        let Stmt::Assign(name, Expr::Eq(l, r)) = parse_stmt("x := 3y=2").unwrap() else {
            panic!("expected equation assignment");
        };
        assert_eq!(name, "x");
        assert_eq!(*l, Expr::num(3).bin(Expr::Mul, var("y")));
        assert_eq!(*r, Expr::Num(int(2)));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_stmt("1 +").is_err());
        assert!(parse_stmt("(1+2").is_err());
        assert!(parse_stmt("a = b = c").is_err());
        assert!(parse_stmt("pi := 3").is_err());
        assert!(parse_stmt("sin(<>)").unwrap_err().contains("placeholder"));
        assert!(parse_stmt("1.2.3").is_err());
    }

    #[test]
    fn dotted_names_address_sections() {
        assert_eq!(
            parse_stmt("physics.a1.g := 9.81").unwrap(),
            Stmt::Assign(
                "physics.a1.g".into(),
                Expr::Num(parse_decimal("9.81").unwrap())
            )
        );
        assert_eq!(
            expr("2physics.g"),
            Expr::num(2).bin(Expr::Mul, var("physics.g"))
        );
        assert!(parse_stmt("physics.").is_err());
        assert!(parse_stmt("physics.1").is_err());
        assert_eq!(expr("x.y+1").children()[0], &var("x.y"));
    }

    #[test]
    fn quantities_and_conversions() {
        let unit = |u: &str| Expr::Unit(u.into());
        assert_eq!(expr("5_N"), Expr::num(5).bin(Expr::Mul, unit("N")));
        assert_eq!(
            expr("9.81_m/s^2"),
            Expr::Num(parse_decimal("9.81").unwrap()).bin(
                Expr::Mul,
                unit("m").bin(Expr::Div, unit("s").bin(Expr::Pow, Expr::num(2)))
            )
        );
        assert_eq!(
            expr("(1+2)_km"),
            Expr::num(1)
                .bin(Expr::Add, Expr::num(2))
                .bin(Expr::Mul, unit("km"))
        );
        assert_eq!(
            expr("2_m*x"),
            Expr::num(2)
                .bin(Expr::Mul, unit("m"))
                .bin(Expr::Mul, var("x"))
        );
        assert_eq!(
            expr("6_m / s"),
            Expr::num(6)
                .bin(Expr::Mul, unit("m"))
                .bin(Expr::Div, var("s"))
        );
        assert_eq!(
            expr("72_km/h -> m/s"),
            Expr::Convert(
                Box::new(Expr::num(72).bin(Expr::Mul, unit("km").bin(Expr::Div, unit("h")))),
                Box::new(unit("m").bin(Expr::Div, unit("s")))
            )
        );
        assert_eq!(expr("5_us"), Expr::num(5).bin(Expr::Mul, unit("µs")));
        assert_eq!(expr("x-1"), var("x").bin(Expr::Sub, Expr::num(1)));
        assert!(parse_stmt("5_xyz").unwrap_err().contains("Unknown unit"));
        assert!(parse_stmt("5 ->").is_err());
        assert!(matches!(
            parse_stmt("v := 3_km -> m").unwrap(),
            Stmt::Assign(_, Expr::Convert(..))
        ));
        assert_eq!(expr("v_0"), var("v_0"));
    }

    #[test]
    fn function_calls() {
        assert_eq!(
            expr("frac(1, 2)"),
            Expr::Call("frac".into(), vec![Expr::num(1), Expr::num(2)])
        );
        assert_eq!(expr("clear()"), Expr::Call("clear".into(), vec![]));
    }
}
