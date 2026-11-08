use serde::{Deserialize, Serialize};

use super::ast::Expr;
use super::format::Style;
use super::notation::{FormattedNumber, Notation, NumberFormat, Scale};
use super::number::NumFmt;
use super::units::{self, UNIT_SEPARATOR};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Answer {
    Value {
        label: Option<String>,
        expr: Expr,
        fmt: NumFmt,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<Expr>,
    },
    Message(String),
    Error(String),
}

impl Answer {
    pub fn value(label: Option<String>, expr: Expr, fmt: NumFmt) -> Answer {
        Answer::Value {
            label,
            expr,
            fmt,
            unit: None,
        }
    }

    #[cfg(test)]
    pub fn text(&self) -> String {
        self.text_with(&NumberFormat::default())
    }

    pub fn quantity(&self) -> Option<Expr> {
        match self {
            Answer::Value {
                expr,
                unit: Some(u),
                ..
            } => Some(Expr::Mul(Box::new(expr.clone()), Box::new(u.clone()))),
            Answer::Value { expr, .. } => Some(expr.clone()),
            _ => None,
        }
    }

    pub fn merged_prefix(&self, numbers: &NumberFormat) -> Option<(FormattedNumber, String)> {
        let Answer::Value {
            expr: Expr::Num(r),
            unit: Some(Expr::Unit(symbol)),
            fmt: NumFmt::Decimal,
            ..
        } = self
        else {
            return None;
        };
        if numbers.notation != Notation::SiPrefix {
            return None;
        }
        let unit = units::resolve(symbol)?;
        if unit.prefix.is_some() || !unit.def.prefixable {
            return None;
        }
        let formatted = numbers.format(r);
        let Scale::Prefix(prefix) = formatted.scale else {
            return None;
        };
        Some((
            FormattedNumber {
                scale: Scale::None,
                ..formatted
            },
            format!("{}{}", prefix.symbol, unit.def.symbol),
        ))
    }

    fn value_text(&self, numbers: &NumberFormat, parseable: bool) -> Option<String> {
        let Answer::Value { fmt, .. } = self else {
            return None;
        };
        if let Some((number, unit)) = self.merged_prefix(numbers) {
            return Some(format!("{}{UNIT_SEPARATOR}{unit}", number.render(false)));
        }
        let mut style = Style::new(*fmt).with_numbers(*numbers);
        if parseable {
            style = style.parseable();
        }
        self.quantity().map(|q| q.to_text(style))
    }

    pub fn text_with(&self, numbers: &NumberFormat) -> String {
        match self {
            Answer::Value { label, .. } => {
                let value = self.value_text(numbers, false).unwrap_or_default();
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
        self.value_text(numbers, true)
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
