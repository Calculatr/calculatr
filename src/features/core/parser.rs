use super::ast::Expr;

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

    pub fn parse(&mut self) -> Result<Expr, String> {
        let expr = self.parse_expression()?;
        self.skip_whitespace();
        if self.pos < self.chars.len() {
            return Err("Unerwartete Zeichen am Ende des Ausdrucks".to_string());
        }
        Ok(expr)
    }

    // 1. Layer: Add and Dub (lowest Prio)
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

    // 2. Layer: Mult and Div (higher Prio)
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
            } else {
                break;
            }
        }

        Ok(left)
    }

    // 3. Layer: Numbers, Variables or Brackets (highest Prio)
    fn parse_factor(&mut self) -> Result<Expr, String> {
        self.skip_whitespace();

        if self.pos >= self.chars.len() {
            return Err("Unerwartetes Ende des Ausdrucks".to_string());
        }

        let c = self.chars[self.pos];

        if c.is_ascii_digit() || c == b'.' {
            let start = self.pos;
            while self.pos < self.chars.len()
                && (self.chars[self.pos].is_ascii_digit() || self.chars[self.pos] == b'.')
            {
                self.pos += 1;
            }
            let num_str = std::str::from_utf8(&self.chars[start..self.pos])
                .map_err(|_| "Ungültige UTF-8 Sequenz in Zahl")?;
            let num = num_str
                .parse::<f64>()
                .map_err(|_| format!("Konnte Zahl nicht parsen: {}", num_str))?;
            return Ok(Expr::Number(num));
        }

        if c.is_ascii_alphabetic() {
            let start = self.pos;
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_alphabetic() {
                self.pos += 1;
            }
            let ident = std::str::from_utf8(&self.chars[start..self.pos])
                .map_err(|_| "Ungültiger Bezeichner")?;

            self.skip_whitespace();
            if self.pos < self.chars.len() && self.chars[self.pos] == b'(' {
                self.pos += 1;
                let arg = self.parse_expression()?;
                self.skip_whitespace();
                if !self.match_char(b')') {
                    return Err("Erwartete schliessende Klammer ')'".to_string());
                }
                return Ok(Expr::Call(ident.to_string(), vec![arg]));
            }

            return Ok(Expr::Variable(ident.to_string()));
        }

        if c == b'(' {
            self.pos += 1;
            let expr = self.parse_expression()?;
            self.skip_whitespace();
            if !self.match_char(b')') {
                return Err("Erwartete schliessende Klammer ')'".to_string());
            }
            return Ok(expr);
        }

        Err(format!("Unerwartetes Zeichen: {}", c as char))
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

pub fn parse_expr(input: &str) -> Result<Expr, String> {
    let mut parser = Parser::new(input);
    parser.parse()
}
