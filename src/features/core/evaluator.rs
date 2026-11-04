use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::algebra;
use super::answer::{Answer, Evaluation};
use super::ast::{ANS, Expr, MathConstant, Stmt};
use super::functions;
use super::number::NumFmt;
use super::parser::parse_stmt;

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

    pub fn sorted_names(&self, filter: &str) -> Vec<String> {
        let mut names: Vec<String> = self
            .vars
            .keys()
            .filter(|n| n.contains(filter))
            .cloned()
            .collect();
        names.sort();
        names
    }

    pub fn check_assign(&self, name: &str, expr: &Expr) -> Result<(), String> {
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
}

fn resolve(
    env: &Environment,
    expr: &Expr,
    mode: RenderMode,
    allow_free_variables: bool,
) -> Result<Resolved, String> {
    let substituted = algebra::resolve_infinity(&env.substitute(expr)?)?;
    let mut exact = algebra::simplify(&substituted)?;
    if env.complex {
        exact = algebra::complex_exact(&exact);
    }
    match mode {
        RenderMode::Symbolic => Ok(Resolved {
            shown: exact.clone(),
            exact,
        }),
        RenderMode::Decimal => {
            let folded = algebra::fold_numeric(&exact, env.complex)?;
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
            })
        }
    }
}

pub fn evaluate(env: &mut Environment, input: &str, mode: RenderMode) -> Evaluation {
    let text = input.trim();
    let fmt = mode.num_fmt();

    if text.is_empty() {
        return Evaluation::Answer(Answer::Value {
            label: None,
            expr: Expr::num(0),
            fmt,
        });
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
            }
        }
        Err(e) => Answer::Error(e),
    }
}

fn run_command(env: &mut Environment, name: &str, args: &[Expr]) -> Evaluation {
    let answer = match (name, args) {
        ("clear", []) => return Evaluation::ClearHistory,
        ("clear", _) => Answer::Error("clear() expects no arguments".into()),
        ("delvar", [Expr::Variable(var)]) => {
            if env.remove_var(var) {
                Answer::Message(format!("Deleted variable '{var}'"))
            } else {
                Answer::Error(format!("Unknown variable '{var}'"))
            }
        }
        ("delvar", _) => Answer::Error("delvar() expects a variable name".into()),
        _ => Answer::Error(format!("Unknown command: {name}")),
    };
    Evaluation::Answer(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(env: &mut Environment, input: &str, mode: RenderMode) -> String {
        match evaluate(env, input, mode) {
            Evaluation::Answer(a) => a.text(),
            Evaluation::ClearHistory => "<clear>".into(),
        }
    }

    use RenderMode::{Decimal, Symbolic};

    #[test]
    fn decimal_and_fraction_are_interchangeable() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "x := 0.25", Decimal), "x := 0.25");
        assert_eq!(run(&mut env, "x := 0.25", Symbolic), "x := 1/4");
        assert_eq!(run(&mut env, "1/4", Decimal), "= 0.25");
        assert_eq!(run(&mut env, "1/4", Symbolic), "= 1/4");
        assert_eq!(run(&mut env, "x + 0.5", Symbolic), "= 3/4");
        assert_eq!(run(&mut env, "x + 0.5", Decimal), "= 0.75");
    }

    #[test]
    fn decimal_mode_has_no_binary_noise() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "0.1 + 0.2", Decimal), "= 0.3");
        assert_eq!(run(&mut env, "1/3", Decimal), "= 0.3333333333333");
        assert_eq!(run(&mut env, "pi/2", Decimal), "= 1.570796326795");
        assert_eq!(run(&mut env, "sqrt(2)", Decimal), "= 1.414213562373");
        assert_eq!(
            run(&mut env, "2^100", Decimal),
            "= 1267650600228229401496703205376"
        );
    }

    #[test]
    fn symbolic_mode_keeps_symbols() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "2x + 3x", Symbolic), "= 5*x");
        assert_eq!(
            run(&mut env, "sqrt(16) + sqrt(2)", Symbolic),
            "= sqrt(2) + 4"
        );
        assert_eq!(run(&mut env, "2 pi", Symbolic), "= 2*pi");
    }

    #[test]
    fn equations_can_be_stored_in_variables() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "x := 3y=2", Symbolic), "x := 3*y = 2");
        assert_eq!(run(&mut env, "x", Symbolic), "= 3*y = 2");
        assert_eq!(
            run(&mut env, "x := 0.5y = 1/4", Decimal),
            "x := 0.5*y = 0.25"
        );
        assert!(run(&mut env, "x + 1", Symbolic).contains("holds an equation"));
        assert_eq!(run(&mut env, "z = 0.25", Symbolic), "= z = 1/4");
    }

    #[test]
    fn unknown_variables() {
        let mut env = Environment::new();
        assert!(run(&mut env, "y + 1", Decimal).contains("Unknown variable 'y'"));
        assert_eq!(run(&mut env, "y + 1", Symbolic), "= y + 1");
        assert_eq!(run(&mut env, "b := a + 1", Decimal), "b := a + 1");
        run(&mut env, "a := 2", Decimal);
        assert_eq!(run(&mut env, "b", Decimal), "= 3");
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
        assert_eq!(
            run(&mut env, "delvar(a)", Decimal),
            "= Deleted variable 'a'"
        );
        assert_eq!(run(&mut env, "b", Decimal), "= 3");
        assert!(run(&mut env, "delvar(a)", Decimal).contains("Unknown variable"));
    }

    #[test]
    fn commands_and_errors() {
        let mut env = Environment::new();
        assert_eq!(run(&mut env, "clear()", Decimal), "<clear>");
        assert!(run(&mut env, "1/0", Decimal).contains("Division by zero"));
        assert!(run(&mut env, "sqrt(-1)", Decimal).contains("undefined"));
        assert!(run(&mut env, "1 +", Decimal).contains("Error"));
        assert_eq!(run(&mut env, "", Decimal), "= 0");
    }
}
