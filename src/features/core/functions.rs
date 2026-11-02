pub fn call(name: &str, args: &[f64]) -> f64 {
    match name {
        "sin" => args.first().copied().unwrap_or(0.0).sin(),
        "cos" => args.first().copied().unwrap_or(0.0).cos(),
        "sqrt" => args.first().copied().unwrap_or(0.0).sqrt(),
        _ => {
            eprintln!("Unknown function: {}", name);
            0.0
        }
    }
}
