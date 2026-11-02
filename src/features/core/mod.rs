pub mod ast;
pub mod evaluator;
mod functions;
pub mod parser;

use ast::MathConstant;
pub use evaluator::{Environment, RenderMode};

pub fn evaluate(env: &mut Environment, input: &str, mode: RenderMode) -> String {
    evaluator::evaluate(env, input, mode)
}

pub struct Candidate {
    pub name: String,
    pub is_function: bool,
}

pub fn completions(env: &Environment) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = functions::NAMES
        .iter()
        .map(|n| Candidate {
            name: n.to_string(),
            is_function: true,
        })
        .collect();
    out.extend(MathConstant::NAMES.iter().map(|n| Candidate {
        name: n.to_string(),
        is_function: false,
    }));
    out.extend(env.var_names().map(|n| Candidate {
        name: n.clone(),
        is_function: false,
    }));
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);
    out
}
