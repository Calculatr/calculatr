pub mod algebra;
pub mod answer;
pub mod ast;
pub mod evaluator;
pub mod format;
pub mod functions;
pub mod names;
pub mod notation;
pub mod number;
pub mod parser;
pub mod quantity;
pub mod units;

pub use answer::{Answer, ConfigRequest, Evaluation};
pub use evaluator::{Environment, RenderMode};
pub use notation::{ExponentStep, Notation, NumberFormat};

pub fn evaluate(env: &mut Environment, input: &str, mode: RenderMode) -> Evaluation {
    evaluator::evaluate(env, input, mode)
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub kind: CandidateKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateKind {
    Function {
        params: Vec<String>,
        doc: &'static str,
    },
    Constant,
    Variable,
}

impl Candidate {
    pub fn is_function(&self) -> bool {
        matches!(self.kind, CandidateKind::Function { .. })
    }

    pub fn signature(&self) -> String {
        match &self.kind {
            CandidateKind::Function { params, .. } => {
                format!("{}({})", self.name, params.join(", "))
            }
            _ => self.name.clone(),
        }
    }
}

pub fn completions(env: &Environment) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = functions::FUNCTIONS
        .iter()
        .map(|f| Candidate {
            name: f.name.to_string(),
            kind: CandidateKind::Function {
                params: f.params.iter().map(|p| p.to_string()).collect(),
                doc: f.doc,
            },
        })
        .collect();
    out.extend(ast::MathConstant::ALL.iter().map(|c| Candidate {
        name: c.name().to_string(),
        kind: CandidateKind::Constant,
    }));
    if env.complex() {
        out.push(Candidate {
            name: ast::MathConstant::I.name().to_string(),
            kind: CandidateKind::Constant,
        });
    }
    out.push(Candidate {
        name: ast::ANS.to_string(),
        kind: CandidateKind::Constant,
    });
    out.extend(env.var_names().map(|n| Candidate {
        name: n.clone(),
        kind: CandidateKind::Variable,
    }));
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);
    out
}
