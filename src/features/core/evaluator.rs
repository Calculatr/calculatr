use super::ast::Expr;
use super::functions;
use super::parser::parse_expr;

pub fn eval(expr: &Expr) -> f64 {
    match expr {
        Expr::Number(n) => *n,
        Expr::Variable(name) => 0.0,

        Expr::Add(left, right) => eval(left) + eval(right),
        Expr::Sub(left, right) => eval(left) - eval(right),
        Expr::Mul(left, right) => eval(left) * eval(right),
        Expr::Div(left, right) => eval(left) / eval(right),

        Expr::Call(name, args) => {
            let evaluated_args: Vec<f64> = args.iter().map(|arg| eval(arg)).collect();
            functions::call(name, &evaluated_args)
        }
    }
}

pub fn evaluate(input: &str) -> String {
    let cleaned = input.trim();

    if cleaned.is_empty() {
        return "= 0".to_string();
    }

    let parsed_result: Result<f64, _> = cleaned.parse();

    match parse_expr(cleaned) {
        Ok(ast) => {
            let result = eval(&ast);
            format!("= {}", result)
        }
        Err(err) => {
            format!("= Fehler: {}", err)
        }
    }
}
