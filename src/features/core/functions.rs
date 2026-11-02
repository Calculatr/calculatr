pub const NAMES: &[&str] = &["sin", "cos", "sqrt"];

pub fn call(name: &str, args: &[f64]) -> Result<f64, String> {
    let f: fn(f64) -> f64 = match name {
        "sin" => f64::sin,
        "cos" => f64::cos,
        "sqrt" => f64::sqrt,
        _ => return Err(format!("Unknown function: {}", name)),
    };

    match args {
        [x] => Ok(f(*x)),
        _ => Err(format!("{}() expects exactly 1 argument", name)),
    }
}
