use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use super::core::{Answer, NumberFormat};

pub const MAX_SAVED_ENTRIES: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub input: String,
    pub answer: Answer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Input,
    Result,
}

pub fn line_count(history: &[HistoryEntry]) -> usize {
    history.len() * 2
}

pub fn locate(line: usize) -> (usize, LineKind) {
    if line.is_multiple_of(2) {
        (line / 2, LineKind::Input)
    } else {
        (line / 2, LineKind::Result)
    }
}

pub fn insert_text(history: &[HistoryEntry], line: usize) -> Option<String> {
    insert_text_with(history, line, &NumberFormat::default())
}

pub fn insert_text_with(
    history: &[HistoryEntry],
    line: usize,
    numbers: &NumberFormat,
) -> Option<String> {
    let (entry, kind) = locate(line);
    let entry = history.get(entry)?;
    match kind {
        LineKind::Input => {
            let t = entry.input.trim();
            (!t.is_empty()).then(|| t.to_string())
        }
        LineKind::Result => entry.answer.insert_text_with(numbers),
    }
}

pub fn is_selectable(history: &[HistoryEntry], line: usize) -> bool {
    insert_text(history, line).is_some()
}

pub fn remove_lines(history: &mut Vec<HistoryEntry>, lines: RangeInclusive<usize>) {
    let first = lines.start() / 2;
    let last = (lines.end() / 2).min(history.len().saturating_sub(1));
    if first < history.len() {
        history.drain(first..=last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::core::ast::Expr;
    use crate::features::core::number::NumFmt;

    fn entry(input: &str, answer: Answer) -> HistoryEntry {
        HistoryEntry {
            input: input.into(),
            answer,
        }
    }

    fn sample() -> Vec<HistoryEntry> {
        vec![
            entry("1+1", Answer::value(None, Expr::num(2), NumFmt::Decimal)),
            entry("1/0", Answer::Error("Division by zero".into())),
            entry("delvar(x)", Answer::Message("Deleted".into())),
        ]
    }

    #[test]
    fn lines_map_to_entries() {
        assert_eq!(locate(0), (0, LineKind::Input));
        assert_eq!(locate(3), (1, LineKind::Result));
        assert_eq!(line_count(&sample()), 6);
    }

    #[test]
    fn only_meaningful_lines_are_selectable() {
        let h = sample();
        assert_eq!(insert_text(&h, 0).as_deref(), Some("1+1"));
        assert_eq!(insert_text(&h, 1).as_deref(), Some("2"));
        assert_eq!(insert_text(&h, 2).as_deref(), Some("1/0"));
        assert!(!is_selectable(&h, 3));
        assert!(!is_selectable(&h, 5));
        assert!(!is_selectable(&h, 99));
    }

    #[test]
    fn removing_lines_removes_whole_entries() {
        let mut h = sample();
        remove_lines(&mut h, 1..=2);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].input, "delvar(x)");
        remove_lines(&mut h, 0..=1);
        assert!(h.is_empty());
        remove_lines(&mut h, 0..=0);
    }
}
