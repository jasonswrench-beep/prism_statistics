//! What the analyses share: reading a variable's cells with missing values told apart, categories of a grouping
//! variable (numeric codes or text), counting why cases were left out, table cells, and the diagnostics most tests carry.

use crate::error::{Error, Result};
use crate::model::{format_code, Column, Dataset, NumericCell};
use crate::result::{
    CaseAccounting, Cell, Diagnostic, DiagnosticState, Exclusion, Field, ResultBundle, Status,
};

/// One case's value for one variable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Slot<T> {
    Valid(T),
    Empty,
    UserMissing,
}

impl<T: Copy> Slot<T> {
    pub fn value(&self) -> Option<T> {
        match self {
            Slot::Valid(v) => Some(*v),
            _ => None,
        }
    }
}

/// The cells of a numeric variable. Text variables are refused with a plain message.
pub fn numeric(ds: &Dataset, name: &str) -> Result<Vec<Slot<f64>>> {
    Ok(ds
        .numeric_cells(name)?
        .into_iter()
        .map(|c| match c {
            NumericCell::Value(v) => Slot::Valid(v),
            NumericCell::SystemMissing => Slot::Empty,
            NumericCell::UserMissing(_) => Slot::UserMissing,
        })
        .collect())
}

/// One category of a grouping variable: its code (numeric variables) or its text, and the label output shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Level {
    pub code: Option<f64>,
    pub label: String,
}

impl Level {
    /// How the category is written in settings and fields: the code, or the text itself.
    pub fn key(&self) -> String {
        self.code.map_or_else(|| self.label.clone(), format_code)
    }
}

/// More categories than this and the variable is not a grouping variable.
pub const MAX_LEVELS: usize = 50;

/// The categories of a variable and each case's category. Numeric variables are grouped by code (in code order, labeled
/// with their value labels); text variables by their text (in alphabetical order).
pub fn categories(ds: &Dataset, name: &str) -> Result<(Vec<Slot<usize>>, Vec<Level>)> {
    let i = ds
        .find(name)
        .ok_or_else(|| Error::UnknownVariable(name.to_string()))?;
    let var = &ds.variables[i];
    let (slots, levels) = match &ds.columns[i] {
        Column::Numeric(_) => {
            let cells = numeric(ds, name)?;
            let mut codes: Vec<f64> = cells.iter().filter_map(Slot::value).collect();
            codes.sort_by(f64::total_cmp);
            codes.dedup();
            let slots = cells
                .iter()
                .map(|c| match c {
                    Slot::Valid(v) => Slot::Valid(
                        codes
                            .iter()
                            .position(|k| k == v)
                            .expect("every valid code is listed"),
                    ),
                    Slot::Empty => Slot::Empty,
                    Slot::UserMissing => Slot::UserMissing,
                })
                .collect();
            (
                slots,
                codes
                    .iter()
                    .map(|&c| Level {
                        code: Some(c),
                        label: var.value_text(c),
                    })
                    .collect::<Vec<_>>(),
            )
        }
        Column::Text(values) => {
            let mut texts: Vec<&str> = values
                .iter()
                .flatten()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            texts.sort_unstable();
            texts.dedup();
            let slots = values
                .iter()
                .map(|v| match v.as_deref().map(str::trim) {
                    Some(s) if !s.is_empty() => Slot::Valid(
                        texts
                            .iter()
                            .position(|t| *t == s)
                            .expect("every text is listed"),
                    ),
                    _ => Slot::Empty,
                })
                .collect();
            (
                slots,
                texts
                    .iter()
                    .map(|t| Level {
                        code: None,
                        label: t.to_string(),
                    })
                    .collect(),
            )
        }
    };
    if levels.len() > MAX_LEVELS {
        return Err(Error::Invalid(format!(
            "{} has {} different values, which is too many to be a grouping variable.",
            var.display_name(),
            levels.len()
        )));
    }
    Ok((slots, levels))
}

/// Counts the reasons cases were left out, keeping the order in which they first appear.
#[derive(Debug, Default, Clone)]
pub struct Tally(Vec<(String, usize)>);

impl Tally {
    pub fn add(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        match self.0.iter_mut().find(|(r, _)| *r == reason) {
            Some((_, n)) => *n += 1,
            None => self.0.push((reason, 1)),
        }
    }

    /// Adds the reason a slot is missing, if it is. Returns true when the case is to be left out.
    pub fn check<T>(&mut self, slot: &Slot<T>, role: &str) -> bool {
        match slot {
            Slot::Valid(_) => false,
            Slot::Empty => {
                self.add(format!("{role} empty"));
                true
            }
            Slot::UserMissing => {
                self.add(format!("{role} user-missing"));
                true
            }
        }
    }

    pub fn accounting(self, total: usize, analyzed: usize) -> CaseAccounting {
        CaseAccounting {
            total,
            analyzed,
            excluded: self
                .0
                .into_iter()
                .map(|(reason, count)| Exclusion { reason, count })
                .collect(),
        }
    }
}

/// A result with no tables yet.
pub fn bundle(
    method_id: &str,
    method_version: u32,
    title: String,
    settings: serde_json::Value,
    cases: CaseAccounting,
    method_note: String,
) -> ResultBundle {
    ResultBundle {
        method_id: method_id.into(),
        method_version,
        title,
        settings,
        status: Status::Ok,
        cases,
        tables: Vec::new(),
        diagnostics: Vec::new(),
        method_note,
        fields: Default::default(),
        charts: Vec::new(),
    }
}

pub fn set(b: &mut ResultBundle, key: impl Into<String>, value: f64) {
    b.fields.insert(key.into(), Field::Number(value));
}

pub fn set_text(b: &mut ResultBundle, key: impl Into<String>, value: impl Into<String>) {
    b.fields.insert(key.into(), Field::Text(value.into()));
}

pub fn fail(b: &mut ResultBundle, reason: impl Into<String>) {
    b.status = Status::Failed {
        reason: reason.into(),
    };
}

pub fn text(s: impl Into<String>) -> Cell {
    Cell::Text { text: s.into() }
}

/// A number with `decimals` places, or an empty cell when it couldn't be computed.
pub fn num(value: f64, decimals: u8) -> Cell {
    if value.is_finite() {
        Cell::Number { value, decimals }
    } else {
        Cell::Empty
    }
}

/// An interval bound: a number, or ∞ for the open end of a one-sided interval.
pub fn bound(value: f64, decimals: u8) -> Cell {
    if value.is_infinite() {
        text(if value > 0.0 { "∞" } else { "−∞" })
    } else {
        num(value, decimals)
    }
}

pub fn opt(value: Option<f64>, decimals: u8) -> Cell {
    value.map_or(Cell::Empty, |v| num(v, decimals))
}

pub fn count(value: usize) -> Cell {
    Cell::Count { value }
}

pub fn p(value: f64) -> Cell {
    if value.is_finite() {
        Cell::P { value }
    } else {
        Cell::Empty
    }
}

/// The confidence level as a heading: "95%".
pub fn level_text(level: f64) -> String {
    format!("{}%", crate::apa::num(level * 100.0, 0))
}

pub fn check_level(level: f64) -> Result<()> {
    if level > 0.0 && level < 1.0 {
        Ok(())
    } else {
        Err(Error::Invalid(
            "The confidence level must be between 0 and 100%.".into(),
        ))
    }
}

/// Below this many cases a test depends heavily on the shape of the data.
pub const SMALL: usize = 10;

/// "Sample size": review needed below [`SMALL`] cases (in the smallest group, when there are groups).
pub fn size_diagnostic(smallest: usize, detail: String) -> Diagnostic {
    if smallest < SMALL {
        Diagnostic {
            id: "size".into(),
            title: "Sample size".into(),
            state: DiagnosticState::ReviewNeeded,
            message: format!("{detail}. With fewer than {SMALL} cases the result depends heavily on the shape of the data, so look at the values before relying on the p value."),
        }
    } else {
        Diagnostic {
            id: "size".into(),
            title: "Sample size".into(),
            state: DiagnosticState::NoConcernDetected,
            message: format!("{detail}."),
        }
    }
}

pub fn not_assessed(id: &str, title: &str, message: &str) -> Diagnostic {
    Diagnostic {
        id: id.into(),
        title: title.into(),
        state: DiagnosticState::NotAssessed,
        message: message.into(),
    }
}

/// Size, mean, SD and SE of valid values, as table cells after a label.
pub fn summary_row(label: &str, x: &[f64]) -> Vec<Cell> {
    let d = prismqda_stats::describe::descriptives(x, 0.95);
    vec![
        text(label),
        count(d.n),
        opt(d.mean, 3),
        opt(d.sd, 3),
        opt(d.se, 3),
    ]
}

pub fn median(x: &[f64]) -> Option<f64> {
    prismqda_stats::describe::percentile_sorted(&prismqda_stats::describe::sorted_finite(x), 0.5)
}
