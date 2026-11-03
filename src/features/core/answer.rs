use serde::{Deserialize, Serialize};

use super::ast::Expr;
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
    pub fn text(&self) -> String {
        match self {
            Answer::Value {
                label: Some(name),
                expr,
                fmt,
            } => {
                format!("{name} := {}", expr.to_text(*fmt))
            }
            Answer::Value {
                label: None,
                expr,
                fmt,
            } => expr.to_text(*fmt),
            Answer::Message(m) => m.clone(),
            Answer::Error(e) => format!("Error: {e}"),
        }
    }

    pub fn insert_text(&self) -> Option<String> {
        match self {
            Answer::Value { expr, fmt, .. } => Some(expr.to_text(*fmt)),
            Answer::Message(_) | Answer::Error(_) => None,
        }
    }
}

pub enum Evaluation {
    Answer(Answer),
    ClearHistory,
}
