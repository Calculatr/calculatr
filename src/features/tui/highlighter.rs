use super::theme::TokenKind;
use crate::features::core::ast::{ANS, MathConstant};
use crate::features::core::functions;
use crate::features::core::names::SEPARATOR;
use crate::features::core::units::{self, CONVERSION_ARROW, UNIT_SEPARATOR};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub kind: TokenKind,
}

fn token(text: impl Into<String>, kind: TokenKind) -> Token {
    Token {
        text: text.into(),
        kind,
    }
}

fn c_is(chars: &[char], i: usize, c: char) -> bool {
    chars.get(i) == Some(&c)
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;

    let next_non_space = |from: usize| chars[from..].iter().find(|c| !c.is_whitespace()).copied();
    let unit_chain = |from: usize| {
        let rest: String = chars[from..].iter().collect();
        units::parse_chain(&rest)
            .ok()
            .map(|(_, bytes)| rest[..bytes].chars().count())
    };

    while i < chars.len() {
        let attaches_unit = matches!(
            out.last(),
            Some(Token {
                kind: TokenKind::Number,
                ..
            })
        ) || out.last().is_some_and(|t: &Token| t.text == ")");
        if c_is(&chars, i, UNIT_SEPARATOR)
            && attaches_unit
            && let Some(len) = unit_chain(i + 1)
        {
            out.push(token(
                chars[i..i + 1 + len].iter().collect::<String>(),
                TokenKind::Constant,
            ));
            i += 1 + len;
            continue;
        }
        if chars[i..].starts_with(&CONVERSION_ARROW.chars().collect::<Vec<_>>()) {
            out.push(token(CONVERSION_ARROW, TokenKind::Operator));
            i += CONVERSION_ARROW.len();
            let spaces = chars[i..].iter().take_while(|c| c.is_whitespace()).count();
            if spaces > 0 {
                out.push(token(
                    chars[i..i + spaces].iter().collect::<String>(),
                    TokenKind::Text,
                ));
                i += spaces;
            }
            if let Some(len) = unit_chain(i) {
                out.push(token(
                    chars[i..i + len].iter().collect::<String>(),
                    TokenKind::Constant,
                ));
                i += len;
            }
            continue;
        }
        let c = chars[i];
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            out.push(token(
                chars[start..i].iter().collect::<String>(),
                TokenKind::Number,
            ));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            loop {
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let continues = chars.get(i) == Some(&SEPARATOR)
                    && chars.get(i + 1).is_some_and(|c| c.is_alphabetic());
                if !continues {
                    break;
                }
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let kind = if MathConstant::from_name(&word).is_some() || word == ANS {
                TokenKind::Constant
            } else if functions::lookup(&word).is_some() || next_non_space(i) == Some('(') {
                TokenKind::Function
            } else {
                TokenKind::Variable
            };
            out.push(token(word, kind));
        } else if c == ':' && chars.get(i + 1) == Some(&'=') {
            out.push(token(":=", TokenKind::Operator));
            i += 2;
        } else if c == '<' && chars.get(i + 1) == Some(&'>') {
            out.push(token("<>", TokenKind::Placeholder));
            i += 2;
        } else {
            let kind = match c {
                '+' | '-' | '*' | '/' | '^' | '=' | '!' => TokenKind::Operator,
                '(' | ')' | '[' | ']' | '{' | '}' => TokenKind::Bracket,
                ',' => TokenKind::Punct,
                _ => TokenKind::Text,
            };
            out.push(token(c, kind));
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn kinds(s: &str) -> Vec<(String, TokenKind)> {
        tokenize(s).into_iter().map(|t| (t.text, t.kind)).collect()
    }

    #[test]
    fn classifies_tokens() {
        assert_eq!(
            kinds("sin(x)+2*pi"),
            vec![
                ("sin".into(), Function),
                ("(".into(), Bracket),
                ("x".into(), Variable),
                (")".into(), Bracket),
                ("+".into(), Operator),
                ("2".into(), Number),
                ("*".into(), Operator),
                ("pi".into(), Constant),
            ]
        );
    }

    #[test]
    fn assignment_placeholders_and_unknown_calls() {
        assert_eq!(
            kinds("x := f(<>, 1.5)"),
            vec![
                ("x".into(), Variable),
                (" ".into(), Text),
                (":=".into(), Operator),
                (" ".into(), Text),
                ("f".into(), Function),
                ("(".into(), Bracket),
                ("<>".into(), Placeholder),
                (",".into(), Punct),
                (" ".into(), Text),
                ("1.5".into(), Number),
                (")".into(), Bracket),
            ]
        );
    }

    #[test]
    fn dotted_names_are_one_token() {
        assert_eq!(
            kinds("physics.g*2.5"),
            vec![
                ("physics.g".into(), Variable),
                ("*".into(), Operator),
                ("2.5".into(), Number),
            ]
        );
    }

    #[test]
    fn units_and_conversions() {
        assert_eq!(
            kinds("9.81_m/s^2*x -> km/h"),
            vec![
                ("9.81".into(), Number),
                ("_m/s^2".into(), Constant),
                ("*".into(), Operator),
                ("x".into(), Variable),
                (" ".into(), Text),
                ("->".into(), Operator),
                (" ".into(), Text),
                ("km/h".into(), Constant),
            ]
        );
        assert_eq!(kinds("v_0")[0], ("v_0".into(), Variable));
        assert_eq!(kinds("(a)_kN")[3], ("_kN".into(), Constant));
    }

    #[test]
    fn tokens_reassemble_to_the_input() {
        for s in [
            "",
            "3y = 2",
            "frac(1, 2) ^ -x",
            "αβ + 1",
            "a.b.c + d.",
            "5_µs -> ms",
            "2_xyz",
            "->",
        ] {
            let joined: String = tokenize(s).into_iter().map(|t| t.text).collect();
            assert_eq!(joined, s);
        }
    }
}
