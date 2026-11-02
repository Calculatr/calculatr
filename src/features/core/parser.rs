use super::ast::{Expr, MathConstant, Stmt};

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
        if self.pos < self.chars.len() {
            return Err("Unexpected characters at the end of the expression".to_string());
        }
        Ok(stmt)
    }

    // 0. Layer: Assignment `name = expression` or normal expression
    fn parse_statement(&mut self) -> Result<Stmt, String> {
        self.skip_whitespace();
        let start = self.pos;

        if let Some(name) = self.parse_identifier() {
            self.skip_whitespace();
            if self.match_char(b'=') {
                let expr = self.parse_expression()?;
                return Ok(Stmt::Assign(name, expr));
            }
        }

        // No assignment -> reset and parse as expression
        self.pos = start;
        Ok(Stmt::Expr(self.parse_expression()?))
    }

    // 1. Layer: Add and Sub (lowest precedence)
    fn parse_expression(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_term()?;

        loop {
            self.skip_whitespace();
            if self.match_char(b'+') {
                let right = self.parse_term()?;
                left = Expr::Add(Box::new(left), Box::new(right));
            } else if self.match_char(b'-') {
                let right = self.parse_term()?;
                left = Expr::Sub(Box::new(left), Box::new(right));
            } else {
                break;
            }
        }

        Ok(left)
    }

    // 2. Layer: Mul and Div (higher precedence), incl. implicit multiplication (3b, 2(x+1))
    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_factor()?;

        loop {
            self.skip_whitespace();
            if self.match_char(b'*') {
                let right = self.parse_factor()?;
                left = Expr::Mul(Box::new(left), Box::new(right));
            } else if self.match_char(b'/') {
                let right = self.parse_factor()?;
                left = Expr::Div(Box::new(left), Box::new(right));
            } else if self.starts_implicit_factor() {
                let right = self.parse_factor()?;
                left = Expr::Mul(Box::new(left), Box::new(right));
            } else {
                break;
            }
        }

        Ok(left)
    }

    // 3. Layer: Unary minus, numbers, constants, variables, functions, brackets
    fn parse_factor(&mut self) -> Result<Expr, String> {
        self.skip_whitespace();

        if self.pos >= self.chars.len() {
            return Err("Unexpected end of expression".to_string());
        }

        let c = self.chars[self.pos];

        if c == b'-' {
            self.pos += 1;
            let inner = self.parse_factor()?;
            return Ok(Expr::Neg(Box::new(inner)));
        }

        if c.is_ascii_digit() || c == b'.' {
            let start = self.pos;
            while self.pos < self.chars.len()
                && (self.chars[self.pos].is_ascii_digit() || self.chars[self.pos] == b'.')
            {
                self.pos += 1;
            }
            let num_str = std::str::from_utf8(&self.chars[start..self.pos])
                .map_err(|_| "Invalid UTF-8 sequence in number")?;
            let num = num_str
                .parse::<f64>()
                .map_err(|_| format!("Could not parse number: {}", num_str))?;
            return Ok(Expr::Number(num));
        }

        if let Some(ident) = self.parse_identifier() {
            self.skip_whitespace();
            if self.pos < self.chars.len() && self.chars[self.pos] == b'(' {
                self.pos += 1;
                let arg = self.parse_expression()?;
                self.skip_whitespace();
                if !self.match_char(b')') {
                    return Err("Expected closing parenthesis ')'".to_string());
                }
                return Ok(Expr::Call(ident, vec![arg]));
            }

            if let Some(constant) = MathConstant::from_name(&ident) {
                return Ok(Expr::Constant(constant));
            }
            return Ok(Expr::Variable(ident));
        }

        if c == b'(' {
            self.pos += 1;
            let expr = self.parse_expression()?;
            self.skip_whitespace();
            if !self.match_char(b')') {
                return Err("Expected closing parenthesis ')'".to_string());
            }
            return Ok(expr);
        }

        Err(format!("Unexpected character: {}", c as char))
    }

    /// Identifier: letter, followed by letters/digits/underscore.
    fn parse_identifier(&mut self) -> Option<String> {
        if self.pos >= self.chars.len() || !self.chars[self.pos].is_ascii_alphabetic() {
            return None;
        }
        let start = self.pos;
        while self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_alphanumeric() || self.chars[self.pos] == b'_')
        {
            self.pos += 1;
        }
        Some(String::from_utf8_lossy(&self.chars[start..self.pos]).into_owned())
    }

    fn starts_implicit_factor(&self) -> bool {
        self.pos < self.chars.len()
            && (self.chars[self.pos].is_ascii_alphabetic() || self.chars[self.pos] == b'(')
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn match_char(&mut self, expected: u8) -> bool {
        if self.pos < self.chars.len() && self.chars[self.pos] == expected {
            self.pos += 1;
            true
        } else {
            false
        }
    }
}

pub fn parse_stmt(input: &str) -> Result<Stmt, String> {
    let mut parser = Parser::new(input);
    parser.parse()
}
