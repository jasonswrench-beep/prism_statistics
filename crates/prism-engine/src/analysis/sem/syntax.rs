//! The text model (checklist C1a), in lavaan's syntax:
//!
//! * `f =~ x1 + x2 + x3`: a latent variable measured by indicators (which may themselves be latent).
//! * `f <~ x1 + x2 + x3`: a composite formed by its indicators (PLS-SEM's formative, mode B, blocks).
//! * `y ~ x1 + x2`: regressions; `y ~ 1` an intercept.
//! * `x1 ~~ x2`: a variance or covariance.
//! * Modifiers before a variable: a number fixes the parameter (`1*x1`), a name labels it (`a*x2`; parameters with the
//!   same label are held equal), `NA*x1` frees one that would be fixed by default, `start(0.5)*x1` gives a starting
//!   value, and `c(a, b)*x1` gives one modifier per group in a multigroup model.
//! * `ab := a*b`: a defined parameter, any arithmetic of labels.
//! * `a == b`: an equality constraint between labels, or a label and a number.
//!
//! * `level: 1` and `level: 2`: the within-cluster and between-cluster parts of a two-level model.
//!
//! Comments start with `#` or `!`. Statements end at a line break or `;`, except that a line ending in an operator or
//! `+` (or a next line starting with `+`) continues the statement.

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Measured,
    /// `<~`: a composite formed by its indicators (formative, mode B in PLS-SEM).
    Composite,
    Regression,
    Covariance,
}

impl Op {
    pub fn symbol(self) -> &'static str {
        match self {
            Op::Measured => "=~",
            Op::Composite => "<~",
            Op::Regression => "~",
            Op::Covariance => "~~",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modifier {
    Fixed(f64),
    Label(String),
    Free,
    Start(f64),
    /// One modifier per group.
    Groups(Vec<Modifier>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    /// A variable name, or "1" for an intercept.
    pub var: String,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Relation { lhs: String, op: Op, rhs: Term },
    Defined { name: String, expr: Expr },
    Equal { lhs: Expr, rhs: Expr },
    /// `level: 1` (within clusters) or `level: 2` (between clusters) starts a level's part of a two-level model.
    Level(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Name(String),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Call(String, Box<Expr>),
}

impl Expr {
    pub fn eval(&self, value: &dyn Fn(&str) -> Option<f64>) -> Result<f64> {
        Ok(match self {
            Expr::Num(v) => *v,
            Expr::Name(n) => value(n).ok_or_else(|| Error::Invalid(format!("\"{n}\" is not the label of a parameter.")))?,
            Expr::Neg(e) => -e.eval(value)?,
            Expr::Bin(op, a, b) => {
                let (a, b) = (a.eval(value)?, b.eval(value)?);
                match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' => a / b,
                    _ => a.powf(b),
                }
            }
            Expr::Call(f, e) => {
                let v = e.eval(value)?;
                match f.as_str() {
                    "sqrt" => v.sqrt(),
                    "exp" => v.exp(),
                    "log" => v.ln(),
                    "abs" => v.abs(),
                    _ => return Err(Error::Invalid(format!("The function {f}() can't be used in a defined parameter; use sqrt, exp, log or abs."))),
                }
            }
        })
    }

    /// The labels the expression uses.
    pub fn names(&self, out: &mut Vec<String>) {
        match self {
            Expr::Name(n) => {
                if !out.contains(n) {
                    out.push(n.clone());
                }
            }
            Expr::Neg(e) | Expr::Call(_, e) => e.names(out),
            Expr::Bin(_, a, b) => {
                a.names(out);
                b.names(out);
            }
            Expr::Num(_) => {}
        }
    }
}

fn bad(line: &str, why: &str) -> Error {
    Error::Invalid(format!("The model line \"{}\" can't be read: {why}", line.trim()))
}

/// Splits the text into statements (comments removed, continued lines joined).
fn statements(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let continues = |s: &str| {
        let t = s.trim_end();
        t.ends_with('+') || t.ends_with('*') || t.ends_with('~') || t.ends_with("=~") || t.ends_with(":=") || t.ends_with('(') || t.ends_with(',') || t.ends_with("==")
    };
    for raw in text.lines() {
        let line = raw.split(['#', '!']).next().unwrap_or("");
        for (k, part) in line.split(';').enumerate() {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let joins = k == 0 && !cur.is_empty() && (continues(&cur) || part.starts_with('+') || part.starts_with('*'));
            if joins {
                cur.push(' ');
                cur.push_str(part);
            } else {
                if !cur.trim().is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                cur = part.to_string();
            }
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// Splits on `sep` outside parentheses.
fn split_top(s: &str, sep: char) -> Vec<String> {
    let (mut depth, mut cur, mut out) = (0i32, String::new(), Vec::new());
    for ch in s.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if ch == sep && depth == 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    out.push(cur);
    out
}

fn is_name(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(ch) if ch.is_alphabetic() || ch == '_' || ch == '.') && s.chars().all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '.')
}

fn modifier(s: &str, line: &str) -> Result<Modifier> {
    let s = s.trim();
    if s == "NA" {
        return Ok(Modifier::Free);
    }
    if let Ok(v) = s.parse::<f64>() {
        return Ok(Modifier::Fixed(v));
    }
    if let Some(inner) = s.strip_prefix("start(").and_then(|r| r.strip_suffix(')')) {
        return inner.trim().parse::<f64>().map(Modifier::Start).map_err(|_| bad(line, "start() needs a number."));
    }
    if let Some(inner) = s.strip_prefix("c(").and_then(|r| r.strip_suffix(')')) {
        return Ok(Modifier::Groups(split_top(inner, ',').iter().map(|p| modifier(p, line)).collect::<Result<_>>()?));
    }
    if is_name(s) {
        return Ok(Modifier::Label(s.to_string()));
    }
    Err(bad(line, &format!("\"{s}\" is not a number, a label, NA, start() or c().")))
}

fn term(s: &str, line: &str) -> Result<Term> {
    let parts = split_top(s, '*');
    let var = parts.last().map(|p| p.trim().to_string()).unwrap_or_default();
    if var != "1" && !is_name(&var) {
        return Err(bad(line, &format!("\"{var}\" is not a variable name.")));
    }
    let modifiers = parts[..parts.len() - 1].iter().map(|m| modifier(m, line)).collect::<Result<_>>()?;
    Ok(Term { var, modifiers })
}

/// Reads arithmetic: + - * / ^, parentheses, numbers, labels and sqrt/exp/log/abs.
struct ExprParser<'a> {
    s: &'a [u8],
    i: usize,
    line: &'a str,
}

impl ExprParser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn sum(&mut self) -> Result<Expr> {
        let mut e = self.product()?;
        while let Some(c @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            e = Expr::Bin(c as char, Box::new(e), Box::new(self.product()?));
        }
        Ok(e)
    }
    fn product(&mut self) -> Result<Expr> {
        let mut e = self.unary()?;
        while let Some(c @ (b'*' | b'/')) = self.peek() {
            self.i += 1;
            e = Expr::Bin(c as char, Box::new(e), Box::new(self.unary()?));
        }
        Ok(e)
    }
    fn unary(&mut self) -> Result<Expr> {
        if self.peek() == Some(b'-') {
            self.i += 1;
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        let base = self.atom()?;
        if self.peek() == Some(b'^') {
            self.i += 1;
            return Ok(Expr::Bin('^', Box::new(base), Box::new(self.unary()?)));
        }
        Ok(base)
    }
    fn atom(&mut self) -> Result<Expr> {
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                let e = self.sum()?;
                if self.peek() != Some(b')') {
                    return Err(bad(self.line, "a parenthesis is not closed."));
                }
                self.i += 1;
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => {
                let start = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.' || self.s[self.i] == b'e' || self.s[self.i] == b'E') {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
                t.parse().map(Expr::Num).map_err(|_| bad(self.line, &format!("\"{t}\" is not a number.")))
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' || c == b'.' => {
                let start = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_' || self.s[self.i] == b'.') {
                    self.i += 1;
                }
                let name = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("").to_string();
                if self.peek() == Some(b'(') {
                    self.i += 1;
                    let e = self.sum()?;
                    if self.peek() != Some(b')') {
                        return Err(bad(self.line, "a parenthesis is not closed."));
                    }
                    self.i += 1;
                    return Ok(Expr::Call(name, Box::new(e)));
                }
                Ok(Expr::Name(name))
            }
            _ => Err(bad(self.line, "the expression is incomplete.")),
        }
    }
}

pub fn expr(s: &str, line: &str) -> Result<Expr> {
    let mut p = ExprParser { s: s.as_bytes(), i: 0, line };
    let e = p.sum()?;
    if p.peek().is_some() {
        return Err(bad(line, "there is something extra at the end."));
    }
    Ok(e)
}

/// The operator in a statement: the leftmost, preferring the longer at the same place.
fn find_op(s: &str) -> Option<(usize, &'static str)> {
    let ops = ["=~", ":=", "~~", "==", "<~", "~", "<", ">"];
    let mut best: Option<(usize, &'static str)> = None;
    for op in ops {
        if let Some(i) = s.find(op) {
            if best.is_none_or(|(b, bop)| i < b || (i == b && op.len() > bop.len())) {
                best = Some((i, op));
            }
        }
    }
    best
}

pub fn parse(text: &str) -> Result<Vec<Statement>> {
    let mut out = Vec::new();
    for st in statements(text) {
        if let Some(rest) = st.trim().strip_prefix("level") {
            if let Some(v) = rest.trim().strip_prefix(':') {
                let level = match v.trim() {
                    "1" | "within" => 1,
                    "2" | "between" => 2,
                    _ => return Err(bad(&st, "a two-level model has level: 1 (within) and level: 2 (between).")),
                };
                out.push(Statement::Level(level));
                continue;
            }
        }
        let Some((i, op)) = find_op(&st) else {
            return Err(bad(&st, "it has no operator (=~, ~, ~~, := or ==)."));
        };
        let (lhs, rhs) = (st[..i].trim(), st[i + op.len()..].trim());
        if lhs.is_empty() || rhs.is_empty() {
            return Err(bad(&st, "one side of the operator is empty."));
        }
        match op {
            ":=" => {
                if !is_name(lhs) {
                    return Err(bad(&st, "a defined parameter needs a name on the left."));
                }
                out.push(Statement::Defined { name: lhs.to_string(), expr: expr(rhs, &st)? });
            }
            "==" => out.push(Statement::Equal { lhs: expr(lhs, &st)?, rhs: expr(rhs, &st)? }),
            "<" | ">" => return Err(bad(&st, "inequality constraints are not supported.")),
            _ => {
                let op = match op {
                    "=~" => Op::Measured,
                    "<~" => Op::Composite,
                    "~~" => Op::Covariance,
                    _ => Op::Regression,
                };
                let lhss: Vec<String> = split_top(lhs, '+').iter().map(|s| s.trim().to_string()).collect();
                for l in &lhss {
                    if !is_name(l) {
                        return Err(bad(&st, &format!("\"{l}\" is not a variable name.")));
                    }
                }
                for t in split_top(rhs, '+') {
                    let t = t.trim();
                    if t.is_empty() {
                        return Err(bad(&st, "there is an empty term."));
                    }
                    let term = term(t, &st)?;
                    if term.var == "1" && op != Op::Regression {
                        return Err(bad(&st, "an intercept (1) only goes after ~."));
                    }
                    for l in &lhss {
                        out.push(Statement::Relation { lhs: l.clone(), op, rhs: term.clone() });
                    }
                }
            }
        }
    }
    if out.is_empty() {
        return Err(Error::Invalid("The model is empty.".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_lavaan_syntax() {
        let s = parse("# a model\nvisual =~ x1 + a*x2 +\n  NA*x3 ; textual =~ x4\n+ x5\ny ~ start(0.3)*x1 + 1\nx1 ~~ 0.5*x2\nab := a*(b - 1)^2\na == b").unwrap();
        assert_eq!(s.len(), 10);
        assert_eq!(s[1], Statement::Relation { lhs: "visual".into(), op: Op::Measured, rhs: Term { var: "x2".into(), modifiers: vec![Modifier::Label("a".into())] } });
        assert!(matches!(&s[2], Statement::Relation { rhs: Term { modifiers, .. }, .. } if modifiers == &vec![Modifier::Free]));
        assert!(matches!(&s[4], Statement::Relation { lhs, rhs: Term { var, .. }, .. } if lhs == "textual" && var == "x5"));
        assert!(matches!(&s[6], Statement::Relation { rhs: Term { var, .. }, .. } if var == "1"));
        let Statement::Defined { expr, .. } = &s[8] else { panic!() };
        let v = expr.eval(&|n| Some(if n == "a" { 2.0 } else { 4.0 })).unwrap();
        assert_eq!(v, 18.0);
        assert!(parse("y ~ ").is_err());
        assert!(parse("a > 0").is_err());
    }
}
