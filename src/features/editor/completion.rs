use crate::features::core::names::SEPARATOR;
use crate::features::core::{Candidate, CandidateKind};

use super::buffer::PLACEHOLDER;

#[derive(Debug, Clone)]
pub struct CompState {
    pub prefix: String,
    pub idx: usize,
    pub start: usize,
    pub len: usize,
}

pub fn current_word(chars: &[char], cursor: usize) -> (usize, String) {
    let is_name_char = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == SEPARATOR;
    let mut start = cursor.min(chars.len());
    while start > 0 && is_name_char(chars[start - 1]) {
        start -= 1;
    }
    while start < cursor && !chars[start].is_ascii_alphabetic() {
        start += 1;
    }
    (start, chars[start..cursor].iter().collect())
}

pub fn matching<'a>(cands: &'a [Candidate], prefix: &str) -> Vec<&'a Candidate> {
    if prefix.is_empty() {
        return Vec::new();
    }
    cands
        .iter()
        .filter(|c| c.name.starts_with(prefix))
        .collect()
}

pub fn has_longer_match(matches: &[&Candidate], word: &str) -> bool {
    matches.iter().any(|m| m.name.len() > word.len())
}

pub fn common_prefix(matches: &[&Candidate]) -> String {
    let mut p = match matches.first() {
        Some(c) => c.name.clone(),
        None => return String::new(),
    };
    for c in &matches[1..] {
        let n = p
            .bytes()
            .zip(c.name.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        p.truncate(n);
    }
    p
}

pub struct Expansion {
    pub text: String,
    pub first_placeholder: Option<usize>,
}

pub fn expand(cand: &Candidate, with_snippet: bool) -> Expansion {
    match &cand.kind {
        CandidateKind::Function { params, .. } if with_snippet => {
            let args = vec![PLACEHOLDER; params.len()].join(", ");
            Expansion {
                text: format!("{}({})", cand.name, args),
                first_placeholder: (!params.is_empty()).then(|| cand.name.chars().count() + 1),
            }
        }
        _ => Expansion {
            text: cand.name.clone(),
            first_placeholder: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn word_detection() {
        assert_eq!(current_word(&chars("2+si"), 4), (2, "si".to_string()));
        assert_eq!(current_word(&chars("2si"), 3), (1, "si".to_string()));
        assert_eq!(current_word(&chars("a_b"), 3), (0, "a_b".to_string()));
        assert_eq!(current_word(&chars("sin(3"), 5), (5, String::new()));
        assert_eq!(current_word(&chars("sqrt"), 2), (0, "sq".to_string()));
        assert_eq!(
            current_word(&chars("1+physics.a"), 11),
            (2, "physics.a".to_string())
        );
        assert_eq!(current_word(&chars("2.5"), 3), (3, String::new()));
        assert_eq!(current_word(&chars("3phy"), 4), (1, "phy".to_string()));
    }

    #[test]
    fn snippets() {
        let frac = Candidate {
            name: "frac".into(),
            kind: CandidateKind::Function {
                params: vec!["num".into(), "den".into()],
                doc: "",
            },
        };
        let e = expand(&frac, true);
        assert_eq!(e.text, "frac(<>, <>)");
        assert_eq!(e.first_placeholder, Some(5));
        assert_eq!(expand(&frac, false).text, "frac");

        let clear = Candidate {
            name: "clear".into(),
            kind: CandidateKind::Function {
                params: vec![],
                doc: "",
            },
        };
        assert_eq!(expand(&clear, true).text, "clear()");
        assert_eq!(expand(&clear, true).first_placeholder, None);
    }
}
