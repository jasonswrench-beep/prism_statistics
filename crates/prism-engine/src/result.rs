//! The result of an analysis.
//!
//! A [`ResultBundle`] is everything the Output tab, the exports and the report writers need, and nothing they have to
//! recompute: tables, which cases were used and why others were not, diagnostics with one of the four status words
//! (GUIDE-02), a method note, and named fields with stable IDs that writers fill text from.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

/// JSON has no NaN, so a value that couldn't be computed is written as null; this reads it back as NaN.
fn nan_from_null<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
}

/// Whether the analysis produced estimates that may be interpreted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    Ok,
    /// Nothing may be interpreted; the reason is shown instead (defaults table, row 16).
    Failed { reason: String },
}

/// The four status words for a diagnostic. There is no "assumption met" (GUIDE-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticState {
    NoConcernDetected,
    ReviewNeeded,
    UnsupportedCondition,
    NotAssessed,
}

impl DiagnosticState {
    pub fn words(self) -> &'static str {
        match self {
            DiagnosticState::NoConcernDetected => "No concern detected",
            DiagnosticState::ReviewNeeded => "Review needed",
            DiagnosticState::UnsupportedCondition => "Unsupported condition",
            DiagnosticState::NotAssessed => "Not assessed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub id: String,
    pub title: String,
    pub state: DiagnosticState,
    pub message: String,
}

/// How many cases there were, how many were analyzed, and why the rest were left out.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CaseAccounting {
    pub total: usize,
    pub analyzed: usize,
    pub excluded: Vec<Exclusion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exclusion {
    pub reason: String,
    pub count: usize,
}

/// One cell of an output table. Numbers keep their full value; the decimals are only for display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Cell {
    Text { text: String },
    Number {
        #[serde(deserialize_with = "nan_from_null")]
        value: f64,
        decimals: u8,
    },
    Count { value: usize },
    /// A p value, shown as "< .001" below .001 and never as "0.000" (defaults table, row 10).
    P {
        #[serde(deserialize_with = "nan_from_null")]
        value: f64,
    },
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub id: String,
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    pub footnotes: Vec<String>,
}

/// A named value that a writer can cite. Text fields hold labels; number fields hold unrounded results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Field {
    Number(#[serde(deserialize_with = "nan_from_null")] f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultBundle {
    pub method_id: String,
    /// Goes up whenever the computation or a default changes (STAT-04).
    pub method_version: u32,
    pub title: String,
    pub settings: serde_json::Value,
    pub status: Status,
    pub cases: CaseAccounting,
    pub tables: Vec<Table>,
    pub diagnostics: Vec<Diagnostic>,
    pub method_note: String,
    pub fields: BTreeMap<String, Field>,
    /// Figures (Graphs › Chart Builder), as SVG with alternative text. Absent from older projects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charts: Vec<crate::chart::Chart>,
}

impl ResultBundle {
    pub fn number(&self, key: &str) -> Option<f64> {
        match self.fields.get(key) {
            Some(Field::Number(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn text(&self, key: &str) -> Option<&str> {
        match self.fields.get(key) {
            Some(Field::Text(t)) => Some(t),
            _ => None,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.status == Status::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_that_could_not_be_computed_survive_a_round_trip() {
        let cells = vec![Cell::Number { value: f64::NAN, decimals: 1 }, Cell::P { value: f64::NAN }];
        let back: Vec<Cell> = serde_json::from_str(&serde_json::to_string(&cells).unwrap()).unwrap();
        assert!(matches!(back[0], Cell::Number { value, .. } if value.is_nan()));
        let f: Field = serde_json::from_str("null").unwrap();
        assert!(matches!(f, Field::Number(v) if v.is_nan()));
        let t: Field = serde_json::from_str("\"Urban\"").unwrap();
        assert_eq!(t, Field::Text("Urban".into()));
    }
}
