use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::algebra;
use super::answer::{Answer, ConfigRequest, Evaluation};
use super::ast::{ANS, Expr, MathConstant, Stmt};
use super::functions;
use super::names;
use super::number::{self, NumFmt};
use super::parser::parse_stmt;
use super::{quantity, units};

pub(crate) const MAX_RESULT_BITS: u64 = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderMode {
    Decimal,
    Symbolic,
}

impl RenderMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Decimal => Self::Symbolic,
            Self::Symbolic => Self::Decimal,
        }
    }

    pub fn num_fmt(self) -> NumFmt {
        match self {
            Self::Decimal => NumFmt::Decimal,
            Self::Symbolic => NumFmt::Fraction,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Environment {
    vars: HashMap<String, Expr>,
    #[serde(default)]
    last: Option<Expr>,
    #[serde(skip)]
    complex: bool,
}

impl Environment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_complex(&mut self, on: bool) {
        self.complex = on;
    }

    pub fn complex(&self) -> bool {
        self.complex
    }

    pub fn get_expr(&self, name: &str) -> Option<&Expr> {
        self.vars.get(name)
    }

    pub fn var_names(&self) -> impl Iterator<Item = &String> {
        self.vars.keys()
    }

    pub fn sorted_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.vars.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn is_section(&self, path: &str) -> bool {
        self.vars.keys().any(|n| names::is_within(n, path))
    }

    pub fn section_members(&self, section: &str) -> Vec<String> {
        let mut members: Vec<String> = self
            .vars
            .keys()
            .filter(|n| names::is_within(n, section))
            .cloned()
            .collect();
        members.sort();
        members
    }

    pub fn check_assign(&self, name: &str, expr: &Expr) -> Result<(), String> {
        if self.is_section(name) {
            return Err(format!(
                "'{name}' is a section; delete it with delvar({name}, -r) or choose another name"
            ));
        }
        if let Some(var) = names::ancestors(name).find(|a| self.vars.contains_key(*a)) {
            return Err(format!("'{var}' is a variable and can't contain '{name}'"));
        }
        if MathConstant::from_name(name).is_some() || name == ANS {
            return Err(format!("'{name}' is a constant and read-only"));
        }
        if self.complex && name == MathConstant::I.name() {
            return Err("'i' is the imaginary unit while complex numbers are enabled".to_string());
        }
        if functions::lookup(name).is_some() {
            return Err(format!("'{name}' is a function name"));
        }
        if self.depends_on(expr, name, &mut HashSet::new()) {
            return Err(format!(
                "Circular dependency: '{name}' would depend on itself"
            ));
        }
        Ok(())
    }

    pub fn assign(&mut self, name: &str, expr: Expr) -> Result<(), String> {
        self.check_assign(name, &expr)?;
        self.vars.insert(name.to_string(), expr);
        Ok(())
    }

    fn remember(&mut self, exact: &Expr) {
        if !matches!(exact, Expr::Eq(..)) {
            self.last = Some(exact.clone());
        }
    }

    pub fn remove_var(&mut self, name: &str) -> bool {
        let Some(removed) = self.vars.remove(name) else {
            return false;
        };
        for def in self.vars.values_mut() {
            if def.mentions(name) {
                *def = def.replace_var(name, &removed);
            }
        }
        true
    }

    pub fn remove_section(&mut self, section: &str) -> usize {
        let members = self.section_members(section);
        for name in &members {
            self.remove_var(name);
        }
        members.len()
    }

    fn depends_on(&self, expr: &Expr, target: &str, visited: &mut HashSet<String>) -> bool {
        match expr {
            Expr::Variable(n) => {
                n == target
                    || (visited.insert(n.clone())
                        && self
                            .vars
                            .get(n)
                            .is_some_and(|d| self.depends_on(d, target, visited)))
            }
            _ => expr
                .children()
                .into_iter()
                .any(|c| self.depends_on(c, target, visited)),
        }
    }

    fn bind_ans(&self, expr: &Expr) -> Result<Expr, String> {
        if !expr.mentions(ANS) {
            return Ok(expr.clone());
        }
        let last = self.last.as_ref().ok_or("No previous result for 'ans'")?;
        Ok(expr.replace_var(ANS, last))
    }

    pub fn substitute(&self, expr: &Expr) -> Result<Expr, String> {
        self.substitute_inner(expr, &mut Vec::new(), true)
    }

    fn substitute_inner(
        &self,
        e: &Expr,
        stack: &mut Vec<String>,
        top: bool,
    ) -> Result<Expr, String> {
        match e {
            Expr::Variable(name) if self.complex && name == MathConstant::I.name() => {
                Ok(Expr::Constant(MathConstant::I))
            }
            Expr::Variable(name) if name == ANS => self
                .last
                .clone()
                .ok_or_else(|| "No previous result for 'ans'".to_string()),
            Expr::Variable(name) => {
                let Some(def) = self.vars.get(name) else {
                    return Ok(e.clone());
                };
                if stack.contains(name) {
                    return Err(format!("Circular dependency involving '{name}'"));
                }
                if matches!(def, Expr::Eq(..)) && !top {
                    return Err(format!(
                        "'{name}' holds an equation and can't be used inside an expression"
                    ));
                }
                let def = match def {
                    Expr::Convert(value, _) if !top => value,
                    _ => def,
                };
                stack.push(name.clone());
                let result = self.substitute_inner(def, stack, top);
                stack.pop();
                result
            }
            Expr::Eq(a, b) if top => Ok(Expr::Eq(
                Box::new(self.substitute_inner(a, stack, false)?),
                Box::new(self.substitute_inner(b, stack, false)?),
            )),
            Expr::Eq(..) => Err("Equations can't be nested".to_string()),
            _ => e.try_map(&mut |c| self.substitute_inner(c, stack, false)),
        }
    }
}

struct Resolved {
    exact: Expr,
    shown: Expr,
    unit: Option<Expr>,
}

fn resolve(
    env: &Environment,
    expr: &Expr,
    mode: RenderMode,
    allow_free_variables: bool,
) -> Result<Resolved, String> {
    let (body, target) = match env.substitute(expr)? {
        Expr::Convert(value, unit) => (*value, Some(*unit)),
        other => (other, None),
    };
    let body = algebra::resolve_infinity(&body)?;
    let mut exact = algebra::simplify(&body)?;
    if env.complex {
        exact = algebra::complex_exact(&exact);
    }
    if exact.num_bits() > MAX_RESULT_BITS {
        return Err("Result is too large (limit is about 315,000 digits)".into());
    }
    let (value, unit) = if exact.has_units() || target.is_some() {
        separate_units(env, &body, &exact, target.as_ref())?
    } else {
        (exact.clone(), None)
    };
    match mode {
        RenderMode::Symbolic => Ok(Resolved {
            exact,
            shown: value,
            unit,
        }),
        RenderMode::Decimal => {
            let folded = algebra::fold_numeric(&value, env.complex)?;
            if !allow_free_variables
                && !matches!(folded, Expr::Eq(..))
                && let Some(name) = algebra::first_free_variable(&folded)
            {
                return Err(format!(
                    "Unknown variable '{name}' (define it with {name} := …)"
                ));
            }
            Ok(Resolved {
                exact,
                shown: folded,
                unit,
            })
        }
    }
}

fn separate_units(
    env: &Environment,
    body: &Expr,
    exact: &Expr,
    target: Option<&Expr>,
) -> Result<(Expr, Option<Expr>), String> {
    let Some((scalar, dimension)) = quantity::split(exact)? else {
        if target.is_some() {
            return Err("Can't convert: the units can't be separated from the unknowns".into());
        }
        return Ok((exact.clone(), None));
    };
    let (value, unit) = match target {
        Some(_) if dimension.is_dimensionless() => {
            return Err("Nothing to convert: the result has no unit".into());
        }
        Some(target) => {
            let (v, u) = quantity::convert(scalar, dimension, target)?;
            (v, Some(u))
        }
        None if dimension.is_dimensionless() => (algebra::simplify(&scalar)?, None),
        None => {
            let mut atoms = Vec::new();
            body.unit_atoms(&mut atoms);
            let (v, u) = quantity::present(scalar, dimension, &atoms)?;
            (v, Some(u))
        }
    };
    let value = if env.complex {
        algebra::complex_exact(&value)
    } else {
        value
    };
    Ok((value, unit))
}

pub fn evaluate(env: &mut Environment, input: &str, mode: RenderMode) -> Evaluation {
    let text = input.trim();
    let fmt = mode.num_fmt();

    if text.is_empty() {
        return Evaluation::Answer(Answer::value(None, Expr::num(0), fmt));
    }

    let answer = match parse_stmt(text) {
        Err(e) => Answer::Error(e),
        Ok(Stmt::Expr(Expr::Call(name, args)))
            if functions::lookup(&name).is_some_and(|f| f.is_command()) =>
        {
            return run_command(env, &name, &args);
        }
        Ok(Stmt::Expr(expr)) => match resolve(env, &expr, mode, false) {
            Ok(resolved) => {
                env.remember(&resolved.exact);
                Answer::Value {
                    label: None,
                    expr: resolved.shown,
                    fmt,
                    unit: resolved.unit,
                }
            }
            Err(e) => Answer::Error(e),
        },
        Ok(Stmt::Assign(name, expr)) => assign(env, name, expr, mode),
    };
    Evaluation::Answer(answer)
}

fn assign(env: &mut Environment, name: String, expr: Expr, mode: RenderMode) -> Answer {
    let result = env.bind_ans(&expr).and_then(|expr| {
        env.check_assign(&name, &expr)?;
        let resolved = resolve(env, &expr, mode, true)?;
        Ok((expr, resolved))
    });
    match result {
        Ok((expr, resolved)) => {
            let _ = env.assign(&name, expr);
            env.remember(&resolved.exact);
            Answer::Value {
                label: Some(name),
                expr: resolved.shown,
                fmt: mode.num_fmt(),
                unit: resolved.unit,
            }
        }
        Err(e) => Answer::Error(e),
    }
}

fn run_command(env: &mut Environment, name: &str, args: &[Expr]) -> Evaluation {
    let answer = match (name, args) {
        ("clear", []) => return Evaluation::ClearHistory,
        ("clear", _) => Answer::Error("clear() expects no arguments".into()),
        ("delvar", _) => delete_variable(env, args),
        ("units", []) => Answer::Message(units::catalogue()),
        ("units", _) => Answer::Error("units() expects no arguments".into()),
        ("settings", []) => return Evaluation::Configure(ConfigRequest::Show),
        ("settings", _) => Answer::Error("settings() expects no arguments".into()),
        ("set", [key]) => match command_word(key) {
            Some(key) => return Evaluation::Configure(ConfigRequest::Cycle(key)),
            None => Answer::Error("set() expects a setting name, e.g. set(typst)".into()),
        },
        ("set", [key, value]) => match (command_word(key), command_word(value)) {
            (Some(k), Some(v)) => return Evaluation::Configure(ConfigRequest::Set(k, v)),
            _ => Answer::Error(
                "set() expects a setting name and a value, e.g. set(notation, si)".into(),
            ),
        },
        ("set", _) => {
            Answer::Error("Usage: set(setting) to toggle, set(setting, value) to set".into())
        }
        (shortcut, []) if SETTING_SHORTCUTS.contains(&shortcut) => {
            return Evaluation::Configure(ConfigRequest::Cycle(shortcut.to_string()));
        }
        (shortcut, [value]) if SETTING_SHORTCUTS.contains(&shortcut) => match command_word(value) {
            Some(v) => {
                return Evaluation::Configure(ConfigRequest::Set(shortcut.to_string(), v));
            }
            None => Answer::Error(format!("{shortcut}() expects a single word or number")),
        },
        (shortcut, _) if SETTING_SHORTCUTS.contains(&shortcut) => {
            Answer::Error(format!("{shortcut}() expects at most one argument"))
        }
        _ => Answer::Error(format!("Unknown command: {name}")),
    };
    Evaluation::Answer(answer)
}

const SETTING_SHORTCUTS: &[&str] = &["notation", "decimals", "exponent", "extprefixes"];

fn command_word(e: &Expr) -> Option<String> {
    match e {
        Expr::Variable(n) => Some(n.clone()),
        Expr::Constant(c) => Some(c.name().to_string()),
        Expr::Num(r) if number::is_integer(r) => Some(r.numer().to_string()),
        _ => None,
    }
}

fn is_recursive_flag(e: &Expr) -> bool {
    matches!(e, Expr::Neg(inner)
        if matches!(&**inner, Expr::Variable(f) if matches!(f.as_str(), "r" | "R" | "rf" | "fr")))
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn delete_variable(env: &mut Environment, args: &[Expr]) -> Answer {
    const USAGE: &str =
        "Usage: delvar(name) deletes a variable, delvar(section, -r) a whole section";
    let (target, recursive) = match args {
        [Expr::Variable(t)] => (t, false),
        [Expr::Variable(t), flag] if is_recursive_flag(flag) => (t, true),
        [Expr::Variable(_), _] => {
            return Answer::Error("Unknown flag; use -r to delete a section recursively".into());
        }
        _ => return Answer::Error(USAGE.into()),
    };

    if env.remove_var(target) {
        return Answer::Message(format!("Deleted variable '{target}'"));
    }
    let members = env.section_members(target).len();
    if members == 0 {
        return Answer::Error(format!("Unknown variable or section '{target}'"));
    }
    if !recursive {
        return Answer::Error(format!(
            "'{target}' is a section with {}; use delvar({target}, -r) to delete it recursively",
            plural(members, "variable")
        ));
    }
    let deleted = env.remove_section(target);
    Answer::Message(format!(
        "Deleted section '{target}' ({})",
        plural(deleted, "variable")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(env: &mut Environment, input: &str, mode: RenderMode) -> String {
        match evaluate(env, input, mode) {
            Evaluation::Answer(a) => a.text(),
            Evaluation::ClearHistory => "<clear>".into(),
            Evaluation::Configure(req) => format!("<config {req:?}>"),
        }
    }

    use RenderMode::{Decimal, Symbolic};

    #[test]
    fn decimal_and_fraction_are_interchangeable() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "x := 0.25", Decimal), "x := 0.25");
        assert_eq!(run(&mut env, "x := 0.25", Symbolic), "x := 1/4");
        assert_eq!(run(&mut env, "1/4", Decimal), "0.25");
        assert_eq!(run(&mut env, "1/4", Symbolic), "1/4");
        assert_eq!(run(&mut env, "x + 0.5", Symbolic), "3/4");
        assert_eq!(run(&mut env, "x + 0.5", Decimal), "0.75");
    }

    #[test]
    fn decimal_mode_has_no_binary_noise() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "0.1 + 0.2", Decimal), "0.3");
        assert_eq!(run(&mut env, "1/3", Decimal), "0.3333333333333");
        assert_eq!(run(&mut env, "pi/2", Decimal), "1.570796326795");
        assert_eq!(run(&mut env, "sqrt(2)", Decimal), "1.414213562373");
        assert_eq!(
            run(&mut env, "2^100", Decimal),
            "1267650600228229401496703205376"
        );
    }

    #[test]
    fn symbolic_mode_keeps_symbols() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "2x + 3x", Symbolic), "5*x");
        assert_eq!(run(&mut env, "sqrt(16) + sqrt(2)", Symbolic), "sqrt(2) + 4");
        assert_eq!(run(&mut env, "2 pi", Symbolic), "2*pi");
    }

    #[test]
    fn equations_can_be_stored_in_variables() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "x := 3y=2", Symbolic), "x := 3*y = 2");
        assert_eq!(run(&mut env, "x", Symbolic), "3*y = 2");
        assert_eq!(
            run(&mut env, "x := 0.5y = 1/4", Decimal),
            "x := 0.5*y = 0.25"
        );
        assert!(run(&mut env, "x + 1", Symbolic).contains("holds an equation"));
        assert_eq!(run(&mut env, "z = 0.25", Symbolic), "z = 1/4");
    }

    #[test]
    fn unknown_variables() {
        let mut env = Environment::new();
        assert!(run(&mut env, "y + 1", Decimal).contains("Unknown variable 'y'"));
        assert_eq!(run(&mut env, "y + 1", Symbolic), "y + 1");
        assert_eq!(run(&mut env, "b := a + 1", Decimal), "b := a + 1");
        run(&mut env, "a := 2", Decimal);
        assert_eq!(run(&mut env, "b", Decimal), "3");
    }

    #[test]
    fn circular_dependencies_are_rejected() {
        let mut env = Environment::new();
        run(&mut env, "a := b + 1", Decimal);
        assert!(run(&mut env, "b := a", Decimal).contains("Circular"));
        assert!(run(&mut env, "a := a + 1", Decimal).contains("Circular"));
        assert!(run(&mut env, "pi := 3", Decimal).contains("constant"));
        assert!(run(&mut env, "sin := 3", Decimal).contains("function"));
    }

    #[test]
    fn delvar_freezes_dependents() {
        let mut env = Environment::new();
        run(&mut env, "a := 2", Decimal);
        run(&mut env, "b := a + 1", Decimal);
        assert_eq!(run(&mut env, "delvar(a)", Decimal), "Deleted variable 'a'");
        assert_eq!(run(&mut env, "b", Decimal), "3");
        assert!(run(&mut env, "delvar(a)", Decimal).contains("Unknown variable"));
    }

    #[test]
    fn sections_behave_like_directories() {
        let mut env = Environment::new();
        assert_eq!(
            run(&mut env, "physics.g := 9.81", Decimal),
            "physics.g := 9.81"
        );
        run(&mut env, "physics.a1.a.v := 2", Decimal);
        run(&mut env, "math.x := 3", Decimal);
        assert_eq!(run(&mut env, "physics.g * math.x", Decimal), "29.43");
        assert!(env.is_section("physics"));
        assert!(env.is_section("physics.a1"));
        assert!(!env.is_section("physics.g"));
        assert!(run(&mut env, "physics := 1", Decimal).contains("is a section"));
        assert!(run(&mut env, "math.x.y := 1", Decimal).contains("is a variable"));
    }

    #[test]
    fn delvar_needs_a_flag_for_sections() {
        let mut env = Environment::new();
        run(&mut env, "physics.g := 9.81", Decimal);
        run(&mut env, "physics.a1.v := 2", Decimal);
        run(&mut env, "w := physics.g * 2", Decimal);
        let refused = run(&mut env, "delvar(physics)", Decimal);
        assert!(refused.contains("delvar(physics, -r)"), "{refused}");
        assert!(run(&mut env, "delvar(physics, -x)", Decimal).contains("Unknown flag"));
        assert_eq!(
            run(&mut env, "delvar(physics.a1, -r)", Decimal),
            "Deleted section 'physics.a1' (1 variable)"
        );
        assert_eq!(
            run(&mut env, "delvar(physics, -rf)", Decimal),
            "Deleted section 'physics' (1 variable)"
        );
        assert!(!env.is_section("physics"));
        assert_eq!(
            run(&mut env, "w", Decimal),
            "19.62",
            "dependents keep their value"
        );
        assert!(run(&mut env, "delvar(physics)", Decimal).contains("Unknown"));
    }

    #[test]
    fn settings_commands_are_forwarded() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "settings()", Decimal), "<config Show>");
        assert_eq!(
            run(&mut env, "set(notation, e)", Decimal),
            "<config Set(\"notation\", \"e\")>"
        );
        assert_eq!(
            run(&mut env, "decimals(4)", Decimal),
            "<config Set(\"decimals\", \"4\")>"
        );
        assert_eq!(
            run(&mut env, "set(typst)", Decimal),
            "<config Cycle(\"typst\")>"
        );
        assert_eq!(
            run(&mut env, "notation()", Decimal),
            "<config Cycle(\"notation\")>"
        );
        assert!(run(&mut env, "set(1/2, on)", Decimal).contains("Error"));
        assert!(run(&mut env, "notation(1, 2)", Decimal).contains("at most one"));
    }

    #[test]
    fn unit_arithmetic() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "5_N", Decimal), "5_N");
        assert_eq!(run(&mut env, "3_km + 200_m", Decimal), "3200_m");
        assert_eq!(run(&mut env, "2_h + 3_h", Decimal), "5_h");
        assert_eq!(run(&mut env, "2_N * 3_m", Decimal), "6_J");
        assert_eq!(run(&mut env, "(3_m)^2", Decimal), "9_m^2");
        assert_eq!(run(&mut env, "5_m^2", Decimal), "5_m^2");
        assert_eq!(run(&mut env, "10_m / 4_m", Decimal), "2.5");
        assert_eq!(
            run(&mut env, "100_m / 9.58_s", Decimal),
            "10.43841336117_m/s"
        );
        assert_eq!(run(&mut env, "sqrt(16_m^2)", Decimal), "4_m");
        assert_eq!(run(&mut env, "1_kWh", Decimal), "1_kWh");
        assert_eq!(run(&mut env, "12_V * 2_A", Decimal), "24_W");
        assert_eq!(run(&mut env, "1/(2_s)", Decimal), "0.5_s^-1");
        assert_eq!(run(&mut env, "1/(2_s) -> Hz", Decimal), "0.5_Hz");
        assert_eq!(run(&mut env, "-5_m", Decimal), "-5_m");
    }

    #[test]
    fn unit_conversions() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "72_km/h -> m/s", Decimal), "20_m/s");
        assert_eq!(run(&mut env, "1_kWh -> J", Decimal), "3600000_J");
        assert_eq!(run(&mut env, "1_h + 30_min -> min", Decimal), "90_min");
        assert_eq!(run(&mut env, "1_in -> mm", Decimal), "25.4_mm");
        assert_eq!(run(&mut env, "3_km -> m", Symbolic), "3000_m");
        assert_eq!(run(&mut env, "1_m -> ft", Symbolic), "(1250/381)_ft");
        assert!(run(&mut env, "1_m -> s", Decimal).contains("Can't convert m to s"));
        assert!(run(&mut env, "5 -> m", Decimal).contains("Nothing to convert"));
    }

    #[test]
    fn unit_errors() {
        let mut env = Environment::new();
        assert!(run(&mut env, "1_m + 1_s", Decimal).contains("Incompatible units: m and s"));
        assert!(run(&mut env, "1_m + 1", Decimal).contains("Incompatible"));
        assert!(run(&mut env, "sin(1_m)", Decimal).contains("dimensionless"));
        assert!(run(&mut env, "2^(1_s)", Decimal).contains("Exponents"));
        assert!(run(&mut env, "5_xyz", Decimal).contains("Unknown unit"));
    }

    #[test]
    fn variables_and_ans_keep_units() {
        let mut env = Environment::new();
        assert_eq!(
            run(&mut env, "physics.g := 9.81_m/s^2", Decimal),
            "physics.g := 9.81_m/s^2"
        );
        run(&mut env, "mass := 2_kg", Decimal);
        assert_eq!(run(&mut env, "mass * physics.g", Decimal), "19.62_N");
        assert_eq!(run(&mut env, "ans * 2_m", Decimal), "39.24_J");
        assert_eq!(run(&mut env, "v := 72_km/h -> m/s", Decimal), "v := 20_m/s");
        assert_eq!(run(&mut env, "v", Decimal), "20_m/s");
        assert_eq!(run(&mut env, "v * 10_s", Decimal), "200_m");
        assert_eq!(run(&mut env, "x * 2_m", Symbolic), "(2*x)_m");
        assert_eq!(run(&mut env, "x + 1_m", Symbolic), "x + 1_m");
    }

    #[test]
    fn commands_and_errors() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "clear()", Decimal), "<clear>");
        assert!(run(&mut env, "1/0", Decimal).contains("Division by zero"));
        assert!(run(&mut env, "sqrt(-1)", Decimal).contains("undefined"));
        assert!(run(&mut env, "1 +", Decimal).contains("Error"));
        assert_eq!(run(&mut env, "", Decimal), "0");
    }

    #[test]
    fn logb_and_root_evaluation() {
        let mut env = Environment::new();
        // Exact rational & symbolic
        assert_eq!(run(&mut env, "logb(2, 8)", Decimal), "3");
        assert_eq!(run(&mut env, "logb(8, 4)", Symbolic), "2/3");
        assert_eq!(run(&mut env, "logb(2, 1/8)", Decimal), "-3");
        assert_eq!(run(&mut env, "logb(1/2, 8)", Decimal), "-3");
        assert_eq!(run(&mut env, "logb(2, 1)", Decimal), "0");
        assert_eq!(run(&mut env, "logb(2, 3)", Symbolic), "logb(2, 3)");
        assert_eq!(run(&mut env, "logb(2, 3)", Decimal), "1.584962500721");
        assert_eq!(run(&mut env, "logb(e, e^3)", Decimal), "3");

        assert_eq!(run(&mut env, "root(3, 8)", Decimal), "2");
        assert_eq!(run(&mut env, "root(3, -8)", Decimal), "-2");
        assert_eq!(run(&mut env, "root(4, 16)", Symbolic), "2");
        assert_eq!(run(&mut env, "root(4, 16/81)", Symbolic), "2/3");
        assert_eq!(run(&mut env, "root(-2, 4)", Decimal), "0.5");
        assert_eq!(run(&mut env, "root(-2, 4)", Symbolic), "1/2");
        assert_eq!(run(&mut env, "root(1/2, 3)", Decimal), "9");
        assert_eq!(run(&mut env, "root(3, 2)", Symbolic), "root(3, 2)");
        assert_eq!(run(&mut env, "root(3, 2)", Decimal), "1.259921049895");
        assert_eq!(run(&mut env, "root(3, 8_m^3)", Decimal), "2_m");

        // Domain & arity errors in real mode
        assert!(run(&mut env, "logb(1, 2)", Decimal).contains("undefined"));
        assert!(run(&mut env, "logb(-2, 4)", Decimal).contains("undefined"));
        assert!(run(&mut env, "logb(2, 0)", Decimal).contains("undefined"));
        assert!(run(&mut env, "logb(2, -4)", Decimal).contains("undefined"));
        assert!(run(&mut env, "logb(2)", Decimal).contains("2 arguments"));
        assert!(run(&mut env, "root(0, 4)", Decimal).contains("zero"));
        assert!(run(&mut env, "root(2, -4)", Decimal).contains("undefined"));
        assert!(run(&mut env, "root(2)", Decimal).contains("2 arguments"));

        // Complex mode
        env.set_complex(true);
        assert_eq!(run(&mut env, "root(2, -4)", Symbolic), "2*i");
        assert_eq!(run(&mut env, "root(2, -4)", Decimal), "2*i");
        assert_eq!(run(&mut env, "root(-2, -4)", Symbolic), "-i/2");
        assert_eq!(run(&mut env, "root(3, -8)", Decimal), "-2");
        assert_eq!(run(&mut env, "logb(i, -1)", Decimal), "2");
        assert_eq!(run(&mut env, "logb(2, -1)", Decimal), "4.532360141827*i");
        assert!(run(&mut env, "logb(1, 2)", Decimal).contains("undefined"));
        assert!(run(&mut env, "root(0, 4)", Decimal).contains("undefined"));
    }
}
