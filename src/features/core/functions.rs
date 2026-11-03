pub const NAMES: &[&str] = &["sin", "cos", "sqrt", "frac", "clear", "delvar"];

pub fn call(name: &str, args: &[f64]) -> Result<f64, String> {
    match name {
        "sin" => match args {
            [x] => Ok(f64::sin(*x)),
            _ => Err("sin() expects exactly 1 argument".to_string()),
        },
        "cos" => match args {
            [x] => Ok(f64::cos(*x)),
            _ => Err("cos() expects exactly 1 argument".to_string()),
        },
        "sqrt" => match args {
            [x] => Ok(f64::sqrt(*x)),
            _ => Err("sqrt() expects exactly 1 argument".to_string()),
        },
        "frac" => match args {
            [num, den] => {
                if *den == 0.0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(num / den)
                }
            }
            _ => Err("frac() expects exactly 2 arguments".to_string()),
        },
        "clear" => match args {
            [] => Ok(0.0),
            _ => Err("clear() expects no arguments".to_string()),
        },
        "delvar" => Err("delvar expects a variable name string".to_string()),
        _ => Err(format!("Unknown function: {}", name)),
    }
}
