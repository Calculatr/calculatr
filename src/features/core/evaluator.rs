use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::ast::{Expr, MathConstant, Stmt};
use super::functions;
use super::parser::parse_stmt;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Environment {
    vars: HashMap<String, Expr>,
}

impl Environment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_vars(&self) -> &std::collections::HashMap<String, Expr> {
        &self.vars
    }

    pub fn remove_var(&mut self, name: &str) -> bool {
        // 1. Prüfen, ob die Variable überhaupt existiert
        if !self.vars.contains_key(name) {
            return false;
        }

        // 2. Zuerst alle abhängigen Variablen berechnen, solange 'name' noch existiert!
        let mut updates = Vec::new();
        for (var_name, expr) in &self.vars {
            if var_name != name
                && self.depends_on(expr, name, &mut HashSet::new())
                && let Ok(val) = self.eval(expr)
            {
                updates.push((var_name.clone(), Expr::Number(val)));
            }
        }

        // 3. Jetzt die Variable sicher entfernen
        self.vars.remove(name);

        // 4. Die abhängigen Variablen mit ihren neuen Festwerten aktualisieren
        for (var_name, new_expr) in updates {
            self.vars.insert(var_name, new_expr);
        }

        true
    }

    pub fn eval(&self, expr: &Expr) -> Result<f64, String> {
        self.eval_inner(expr, &mut Vec::new())
    }

    fn eval_inner(&self, expr: &Expr, stack: &mut Vec<String>) -> Result<f64, String> {
        match expr {
            Expr::Number(n) => Ok(*n),
            Expr::Constant(c) => Ok(c.value()),
            Expr::Variable(name) => {
                if stack.contains(name) {
                    return Err(format!("Circular dependency involving '{}'", name));
                }
                let def = self
                    .vars
                    .get(name)
                    .ok_or_else(|| format!("Unknown variable: {}", name))?;

                stack.push(name.clone());
                let result = self.eval_inner(def, stack);
                stack.pop();
                result
            }
            Expr::Neg(inner) => Ok(-self.eval_inner(inner, stack)?),
            Expr::Add(l, r) => Ok(self.eval_inner(l, stack)? + self.eval_inner(r, stack)?),
            Expr::Sub(l, r) => Ok(self.eval_inner(l, stack)? - self.eval_inner(r, stack)?),
            Expr::Mul(l, r) => Ok(self.eval_inner(l, stack)? * self.eval_inner(r, stack)?),
            Expr::Div(l, r) => {
                let left = self.eval_inner(l, stack)?;
                let right = self.eval_inner(r, stack)?;
                if right == 0.0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(left / right)
                }
            }
            Expr::Call(name, args) => {
                let values = args
                    .iter()
                    .map(|a| self.eval_inner(a, stack))
                    .collect::<Result<Vec<f64>, String>>()?;
                functions::call(name, &values)
            }
        }
    }

    pub fn assign(&mut self, name: &str, expr: Expr) -> Result<(), String> {
        if MathConstant::from_name(name).is_some() {
            return Err(format!("'{}' is a constant and read-only", name));
        }
        if self.depends_on(&expr, name, &mut HashSet::new()) {
            return Err(format!(
                "Circular dependency: '{}' would depend on itself",
                name
            ));
        }
        self.vars.insert(name.to_string(), expr);
        Ok(())
    }

    fn depends_on(&self, expr: &Expr, target: &str, visited: &mut HashSet<String>) -> bool {
        match expr {
            Expr::Number(_) | Expr::Constant(_) => false,
            Expr::Variable(n) => {
                n == target
                    || (visited.insert(n.clone())
                        && self
                            .vars
                            .get(n)
                            .is_some_and(|e| self.depends_on(e, target, visited)))
            }
            Expr::Neg(e) => self.depends_on(e, target, visited),
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.depends_on(l, target, visited) || self.depends_on(r, target, visited)
            }
            Expr::Call(_, args) => args.iter().any(|a| self.depends_on(a, target, visited)),
        }
    }

    pub fn get_expr(&self, name: &str) -> Option<&Expr> {
        self.vars.get(name)
    }

    pub fn var_names(&self) -> impl Iterator<Item = &String> {
        self.vars.keys()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RenderMode {
    Decimal,
    Symbolic,
}

pub fn evaluate(env: &mut Environment, input: &str, mode: RenderMode) -> String {
    let cleaned = input.trim();

    if cleaned.is_empty() {
        return "= 0".to_string();
    }

    if let Ok(Stmt::Expr(Expr::Call(name, args))) = parse_stmt(cleaned)
        && name == "delvar"
    {
        return match args.first() {
            Some(Expr::Variable(var_name)) => {
                if env.remove_var(var_name) {
                    format!("= Deleted variable '{}'", var_name)
                } else {
                    format!("= Error: Unknown variable '{}'", var_name)
                }
            }
            _ => "= Error: delvar() expects a variable name".to_string(),
        };
    }

    match parse_stmt(cleaned) {
        Ok(Stmt::Expr(expr)) => match mode {
            RenderMode::Decimal => match env.eval(&expr) {
                Ok(v) => format!("= {}", v),
                Err(e) => format!("= Error: {}", e),
            },
            RenderMode::Symbolic => {
                format!("= {}", expr.to_string_pretty())
            }
        },
        Ok(Stmt::Assign(name, expr)) => match env.assign(&name, expr.clone()) {
            Err(e) => format!("= Error: {}", e),
            Ok(()) => match mode {
                RenderMode::Decimal => match env.eval(&Expr::Variable(name.clone())) {
                    Ok(v) => format!("{} = {}", name, v),
                    Err(e) => format!("{} saved ({})", name, e),
                },
                RenderMode::Symbolic => {
                    format!("{} = {}", name, expr.to_string_pretty())
                }
            },
        },
        Err(e) => format!("= Error: {}", e),
    }
}
