use crate::features::core::Candidate;

/// Zustand beim Durchschalten mit mehrfachem Tab
pub struct CompState {
    pub prefix: String,
    pub idx: usize,
}

/// Wort am Ende des Inputs: (Startindex, Wort).
/// Führende Ziffern/Unterstriche werden übersprungen ("2si" -> "si").
pub fn current_word(input: &str) -> (usize, &str) {
    let b = input.as_bytes();
    let mut start = b.len();
    while start > 0 && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_') {
        start -= 1;
    }
    while start < b.len() && (b[start].is_ascii_digit() || b[start] == b'_') {
        start += 1;
    }
    (start, &input[start..])
}

pub fn matching<'a>(cands: &'a [Candidate], prefix: &str) -> Vec<&'a Candidate> {
    if prefix.is_empty() {
        return Vec::new();
    }
    cands.iter().filter(|c| c.name.starts_with(prefix)).collect()
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
