use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::core::{Answer, NumberFormat};

pub const MAX_SAVED_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub input: String,
    pub answer: Answer,
    #[serde(skip, default = "next_id")]
    pub id: u64,
}

impl HistoryEntry {
    pub fn new(input: impl Into<String>, answer: Answer) -> Self {
        Self {
            input: input.into(),
            answer,
            id: next_id(),
        }
    }
}

impl PartialEq for HistoryEntry {
    fn eq(&self, other: &Self) -> bool {
        self.input == other.input && self.answer == other.answer
    }
}

fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
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
    let (entry, kind) = locate(line);
    let Some(entry) = history.get(entry) else {
        return false;
    };
    match kind {
        LineKind::Input => !entry.input.trim().is_empty(),
        LineKind::Result => matches!(entry.answer, Answer::Value { .. }),
    }
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
        HistoryEntry::new(input, answer)
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
