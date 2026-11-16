//! The dataset itself, written out (development list item 8): File › Export › Data as CSV or .sav.
//!
//! - **CSV**: UTF-8 with a header row of variable names, the values as numbers or text, an empty field for an empty
//!   cell. With `labels`, a numeric variable's value labels are written in place of the codes. User-missing codes are
//!   written as they are (they are values in the data), and a sidecar is not written: the CSV says what the data say.
//! - **.sav**: variable names, labels, measures, value labels and user-missing codes (up to three values, or one range
//!   and one value, as the format allows) are kept, with the data uncompressed (`.sav`) or compressed (`.zsav`). Text
//!   variables become string variables as wide as their longest value.
//!
//! Both write only what the dataset holds now. Select Cases and Split File are not applied: the whole dataset is
//! written, so nothing is lost by exporting while a filter is on.

use std::io::{Cursor, Write};
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};

use crate::error::{Error, Result};
use crate::model::{format_code, Column, Dataset, Measure};

/// A CSV field, quoted when it holds a comma, a quote or a line break (and a leading = + - @ is not altered: this is
/// data for statistics programs, not a spreadsheet formula).
fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The dataset as CSV bytes.
pub fn to_csv(ds: &Dataset, labels: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let header: Vec<String> = ds.variables.iter().map(|v| field(&v.name)).collect();
    out.extend_from_slice(header.join(",").as_bytes());
    out.extend_from_slice(b"\r\n");
    for r in 0..ds.n_rows() {
        let cells: Vec<String> = ds
            .variables
            .iter()
            .zip(ds.columns.iter())
            .map(|(v, c)| match c {
                Column::Numeric(x) => match x[r] {
                    None => String::new(),
                    Some(n) if labels => field(&v.value_label(n).map_or_else(|| number(n), str::to_string)),
                    Some(n) => number(n),
                },
                Column::Text(x) => x[r].as_deref().map(field).unwrap_or_default(),
            })
            .collect();
        out.extend_from_slice(cells.join(",").as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out
}

/// A number written so that it reads back exactly: whole numbers without ".0", others in the shortest exact form.
fn number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format_code(n)
    } else {
        format!("{n}")
    }
}

/// The dataset as .sav (or, with `zsav`, .zsav) bytes.
pub fn to_sav(ds: &Dataset, zsav: bool) -> Result<Vec<u8>> {
    use ambers::{Compression, MissingSpec, SpssMetadata, Value};
    let mut fields = Vec::new();
    let mut arrays: Vec<ArrayRef> = Vec::new();
    let mut meta = SpssMetadata::default();
    let mut notes = Vec::new();
    for (v, c) in ds.variables.iter().zip(ds.columns.iter()) {
        if v.name.len() > 64 {
            return Err(Error::Invalid(format!("{} is longer than the 64 characters a .sav file allows.", v.name)));
        }
        match c {
            Column::Numeric(x) => {
                fields.push(Field::new(&v.name, DataType::Float64, true));
                arrays.push(Arc::new(Float64Array::from(x.clone())));
                if !v.value_labels.is_empty() {
                    meta.variable_value_labels.insert(v.name.clone(), v.value_labels.iter().map(|l| (Value::Numeric(l.value), l.label.clone())).collect());
                }
                let mut missing: Vec<MissingSpec> = Vec::new();
                match (v.missing_ranges.as_slice(), v.missing_values.as_slice()) {
                    ([], vals) => missing.extend(vals.iter().take(3).map(|m| MissingSpec::Value(*m))),
                    ([(lo, hi), ..], vals) => {
                        missing.push(MissingSpec::Range { lo: *lo, hi: *hi });
                        missing.extend(vals.iter().take(1).map(|m| MissingSpec::Value(*m)));
                    }
                }
                let kept = missing.len();
                let had = v.missing_values.len() + v.missing_ranges.len();
                if had > kept {
                    notes.push(format!("{} has {had} missing-value codes; a .sav file holds at most three values, or one range and one value, so the first {kept} were kept.", v.name));
                }
                if !missing.is_empty() {
                    meta.variable_missing_values.insert(v.name.clone(), missing);
                }
                let decimals = x.iter().flatten().map(|n| decimals(*n)).max().unwrap_or(0).min(16);
                meta.variable_formats.insert(v.name.clone(), format!("F{}.{decimals}", (decimals + 10).min(40)));
            }
            Column::Text(x) => {
                let width = x.iter().flatten().map(|s| s.len()).max().unwrap_or(1).max(1);
                fields.push(Field::new(&v.name, DataType::Utf8, true));
                arrays.push(Arc::new(StringArray::from(x.iter().map(|s| s.as_deref()).collect::<Vec<_>>())));
                meta.variable_formats.insert(v.name.clone(), format!("A{width}"));
                meta.variable_storage_widths.insert(v.name.clone(), width);
            }
        }
        if !v.label.is_empty() {
            meta.variable_labels.insert(v.name.clone(), v.label.clone());
        }
        meta.variable_measures.insert(
            v.name.clone(),
            match v.measure {
                Measure::Nominal => ambers::Measure::Nominal,
                Measure::Ordinal => ambers::Measure::Ordinal,
                Measure::Scale => ambers::Measure::Scale,
            },
        );
        meta.variable_names.push(v.name.clone());
    }
    meta.number_columns = fields.len();
    meta.number_rows = Some(ds.n_rows() as i64);
    meta.file_label = ds.name.chars().take(64).collect();
    meta.notes = notes;
    let batch = RecordBatch::try_new(Arc::new(Schema::new(fields)), arrays).map_err(|e| Error::Invalid(format!("The data could not be prepared for the .sav file: {e}")))?;
    let mut buf = Cursor::new(Vec::new());
    ambers::write_sav_to_writer(&mut buf, &batch, &meta, if zsav { Compression::Zlib } else { Compression::None }, None).map_err(|e| Error::Invalid(format!("The .sav file could not be written: {e}")))?;
    buf.flush()?;
    Ok(buf.into_inner())
}

/// Decimal places a value needs, up to 16 (for the display format only; the stored value is the full double).
fn decimals(n: f64) -> usize {
    if !n.is_finite() || n.fract() == 0.0 {
        return 0;
    }
    let s = format!("{n}");
    s.split_once('.').map_or(0, |(_, f)| f.trim_end_matches('0').len())
}
