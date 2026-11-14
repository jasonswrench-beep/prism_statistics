//! Datasets and variables.
//!
//! A dataset is a set of equally long columns, one per variable. Numeric cells are `Option<f64>`: `None` is system
//! missing (an empty cell). User-missing codes (such as -99 for "refused") stay in the data as numbers and are listed on
//! the variable, so they can be shown, reported and switched off, but never silently used as real values (MIG-03).

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The level of measurement shown in Variable View.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Measure {
    Nominal,
    Ordinal,
    Scale,
}

/// One column of data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "values", rename_all = "snake_case")]
pub enum Column {
    Numeric(Vec<Option<f64>>),
    Text(Vec<Option<String>>),
}

impl Column {
    pub fn len(&self) -> usize {
        match self {
            Column::Numeric(v) => v.len(),
            Column::Text(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A label for one value of a variable, stored by its code ("1" means "Female").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueLabel {
    pub value: f64,
    pub label: String,
}

/// A variable: its stable ID, name, label, measure, value labels and user-missing codes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    /// Stable for the life of the dataset; renaming a variable keeps its ID, so results stay linked to it.
    pub id: u32,
    pub name: String,
    pub label: String,
    pub measure: Measure,
    pub value_labels: Vec<ValueLabel>,
    /// Codes that mean "missing" for this variable (user-missing), kept apart from empty cells (system missing).
    pub missing_values: Vec<f64>,
    /// Ranges of codes that mean "missing", inclusive at both ends (from files that declare them, such as .sav).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_ranges: Vec<(f64, f64)>,
}

impl Variable {
    /// The label if there is one, otherwise the name. This is what output shows.
    pub fn display_name(&self) -> &str {
        if self.label.trim().is_empty() {
            &self.name
        } else {
            &self.label
        }
    }

    pub fn value_label(&self, value: f64) -> Option<&str> {
        self.value_labels.iter().find(|l| l.value == value).map(|l| l.label.as_str())
    }

    /// The value label, or the value itself written without a needless ".0".
    pub fn value_text(&self, value: f64) -> String {
        match self.value_label(value) {
            Some(l) => l.to_string(),
            None => format_code(value),
        }
    }

    pub fn is_user_missing(&self, value: f64) -> bool {
        self.missing_values.contains(&value) || self.missing_ranges.iter().any(|(lo, hi)| (*lo..=*hi).contains(&value))
    }
}

/// Writes a category code the way a person would type it: 1, not 1.0.
pub fn format_code(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// What a numeric cell holds once missing-value codes are taken into account.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumericCell {
    /// An empty cell.
    SystemMissing,
    /// A value listed as a missing code on the variable.
    UserMissing(f64),
    Value(f64),
}

/// Checks a variable name: a letter first, then letters, digits or underscores, at most 64 characters.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() => {}
        _ => return false,
    }
    name.chars().count() <= 64 && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// A rectangular dataset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    pub name: String,
    pub variables: Vec<Variable>,
    pub columns: Vec<Column>,
    next_id: u32,
}

impl Dataset {
    pub fn new(name: impl Into<String>) -> Self {
        Dataset { name: name.into(), variables: Vec::new(), columns: Vec::new(), next_id: 1 }
    }

    pub fn n_rows(&self) -> usize {
        self.columns.first().map_or(0, Column::len)
    }

    /// Whether the dataset holds together, as one read from a project file must: one column per variable, every column
    /// the same length, valid and distinct names, distinct IDs, and finite codes in the value labels and missing values.
    pub fn check(&self) -> Result<()> {
        let bad = |m: String| Err(Error::Invalid(m));
        if self.variables.len() != self.columns.len() {
            return bad(format!("it lists {} variables but holds {} columns", self.variables.len(), self.columns.len()));
        }
        let n = self.n_rows();
        let mut names = std::collections::HashSet::new();
        let mut ids = std::collections::HashSet::new();
        for (v, c) in self.variables.iter().zip(&self.columns) {
            if c.len() != n {
                return bad(format!("{} has {} cases where the others have {n}", v.name, c.len()));
            }
            if !valid_name(&v.name) || !names.insert(v.name.to_ascii_lowercase()) {
                return bad(format!("the variable name {:?} isn't valid or is used twice", v.name));
            }
            if !ids.insert(v.id) {
                return bad(format!("two variables share the number {}", v.id));
            }
            if v.value_labels.iter().any(|l| !l.value.is_finite()) || v.missing_values.iter().any(|m| !m.is_finite()) || v.missing_ranges.iter().any(|(a, b)| !a.is_finite() || !b.is_finite()) {
                return bad(format!("{} has a value label or missing code that isn't a number", v.name));
            }
        }
        Ok(())
    }

    /// Adds a variable with its data and returns its new ID. Numeric columns start as scale, text as nominal.
    pub fn add_variable(&mut self, name: &str, column: Column) -> Result<u32> {
        if !valid_name(name) {
            return Err(Error::InvalidName(name.to_string()));
        }
        if self.find(name).is_some() {
            return Err(Error::DuplicateVariable(name.to_string()));
        }
        if !self.columns.is_empty() && column.len() != self.n_rows() {
            return Err(Error::RowCount { expected: self.n_rows(), got: column.len() });
        }
        if self.next_id == 0 {
            self.next_id = 1;
        }
        let id = self.next_id;
        self.next_id += 1;
        let measure = match column {
            Column::Numeric(_) => Measure::Scale,
            Column::Text(_) => Measure::Nominal,
        };
        self.variables.push(Variable { id, name: name.to_string(), label: String::new(), measure, value_labels: Vec::new(), missing_values: Vec::new(), missing_ranges: Vec::new() });
        self.columns.push(column);
        Ok(id)
    }

    /// Finds a variable by name, ignoring case (names are unique ignoring case).
    pub fn find(&self, name: &str) -> Option<usize> {
        self.variables.iter().position(|v| v.name.eq_ignore_ascii_case(name))
    }

    pub fn variable(&self, name: &str) -> Result<&Variable> {
        self.find(name).map(|i| &self.variables[i]).ok_or_else(|| Error::UnknownVariable(name.to_string()))
    }

    pub fn variable_mut(&mut self, name: &str) -> Result<&mut Variable> {
        match self.find(name) {
            Some(i) => Ok(&mut self.variables[i]),
            None => Err(Error::UnknownVariable(name.to_string())),
        }
    }

    /// The numeric cells of a variable, each classified as a value, user-missing or system missing.
    pub fn numeric_cells(&self, name: &str) -> Result<Vec<NumericCell>> {
        let i = self.find(name).ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
        let var = &self.variables[i];
        match &self.columns[i] {
            Column::Numeric(values) => Ok(values
                .iter()
                .map(|v| match v {
                    None => NumericCell::SystemMissing,
                    Some(x) if var.is_user_missing(*x) => NumericCell::UserMissing(*x),
                    Some(x) => NumericCell::Value(*x),
                })
                .collect()),
            Column::Text(_) => Err(Error::NotNumeric(var.name.clone())),
        }
    }
}
