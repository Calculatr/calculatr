use serde::{Deserialize, Serialize};

use super::ast::Expr;
use super::format::Style;
use super::notation::NumberFormat;
use super::number::NumFmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Answer {
    Value {
        label: Option<String>,
        expr: Expr,
        fmt: NumFmt,
    },
    Message(String),
    Error(String),
}

impl Answer {
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.text_with(&NumberFormat::default())
    }

    pub fn text_with(&self, numbers: &NumberFormat) -> String {
        match self {
            Answer::Value { label, expr, fmt } => {
                let value = expr.to_text(Style::new(*fmt).with_numbers(*numbers));
                match label {
                    Some(name) => format!("{name} := {value}"),
                    None => value,
                }
            }
            Answer::Message(m) => m.clone(),
            Answer::Error(e) => format!("Error: {e}"),
        }
    }

    pub fn insert_text_with(&self, numbers: &NumberFormat) -> Option<String> {
        match self {
            Answer::Value { expr, fmt, .. } => {
                Some(expr.to_text(Style::new(*fmt).with_numbers(*numbers).parseable()))
            }
            Answer::Message(_) | Answer::Error(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigRequest {
    Show,
    Cycle(String),
    Set(String, String),
}

pub enum Evaluation {
    Answer(Answer),
    ClearHistory,
    Configure(ConfigRequest),
}
