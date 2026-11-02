mod ast;
pub mod evaluator;
mod functions;
pub mod parser;

pub use evaluator::{Environment, RenderMode};

pub fn evaluate(env: &mut Environment, input: &str, mode: RenderMode) -> String {
    evaluator::evaluate(env, input, mode)
}
