mod ast;
pub mod evaluator;
mod functions;
pub mod parser;

pub use evaluator::Environment;

pub fn evaluate(env: &mut Environment, input: &str) -> String {
    evaluator::evaluate(env, input)
}
