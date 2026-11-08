use super::theme::TokenKind;
use crate::features::core::ast::{ANS, MathConstant};
use crate::features::core::functions;
use crate::features::core::names::SEPARATOR;

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

pub fn tokenize(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;

    let next_non_space = |from: usize| chars[from..].iter().find(|c| !c.is_whitespace()).copied();

    while i < chars.len() {
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
                '+' | '-' | '*' | '/' | '^' | '=' => TokenKind::Operator,
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
    fn tokens_reassemble_to_the_input() {
        for s in ["", "3y = 2", "frac(1, 2) ^ -x", "αβ + 1", "a.b.c + d."] {
            let joined: String = tokenize(s).into_iter().map(|t| t.text).collect();
            assert_eq!(joined, s);
        }
    }
}
