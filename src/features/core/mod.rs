mod ast;
pub mod evaluator;
mod functions;
pub mod parser;

pub fn evaluate(input: &str) -> String {
    evaluator::evaluate(input)
}
