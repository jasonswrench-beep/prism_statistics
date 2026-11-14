//! Importing data files from other statistics programs: `.sav` / `.zsav`, `.dta`, `.sas7bdat` and JASP's `.jasp`
//! (data only), task T2-05 subset.
//!
//! Each reader is a pure-Rust library (MIT licensed), so nothing needs to be installed alongside PrismStatistics. Every
//! format is turned into the same intermediate columns and then into a dataset the same way, so the rules are shared:
//!
//! - Variable names are kept when they are valid and unique; otherwise they are repaired and listed in the report.
//! - Variable labels, value labels, user-missing codes and ranges, and measurement levels come across when the file has
//!   them. Value labels on text variables are dropped, with a note.
//! - Numeric cells the file marks as missing become empty. A format's special missing codes (such as `.a` to `.z`) are
//!   counted in the report, since they all become plain empty cells.
//! - Empty text cells are empty, as in a CSV file.
//! - Dates and times become ISO text ("2026-01-05", "2026-01-05T14:30:00") until date variables exist, with a note.

use std::io::Cursor;

use crate::error::{Error, Result};
use crate::import::{unique_name, ImportReport};
use crate::model::{Column, Dataset, Measure, ValueLabel};

/// One column as read from a file, before it becomes a variable.
struct Incoming {
    name: String,
    label: String,
    data: Data,
    value_labels: Vec<ValueLabel>,
    missing_values: Vec<f64>,
    missing_ranges: Vec<(f64, f64)>,
    measure: Option<Measure>,
}

enum Data {
    Numeric(Vec<Option<f64>>),
    Text(Vec<Option<String>>),
}

impl Incoming {
    fn new(name: impl Into<String>, data: Data) -> Self {
        Incoming { name: name.into(), label: String::new(), data, value_labels: Vec::new(), missing_values: Vec::new(), missing_ranges: Vec::new(), measure: None }
    }
}

/// Collects what the user should be told about the conversion.
#[derive(Default)]
struct Notes {
    dates: Vec<String>,
    text_labels: Vec<String>,
    special_missing: usize,
    other: Vec<String>,
}

impl Notes {
    fn into_report(self, report: &mut ImportReport) {
        if !self.dates.is_empty() {
            report.notes.push(format!("Dates and times were read as text (for example 2026-01-05) in: {}. Date variables are coming in a later release.", self.dates.join(", ")));
        }
        if !self.text_labels.is_empty() {
            report.notes.push(format!("Value labels on text variables were not kept: {}.", self.text_labels.join(", ")));
        }
        if self.special_missing > 0 {
            report.notes.push(format!("{} cell{} held a special missing code (such as .a to .z) and became empty.", self.special_missing, if self.special_missing == 1 { "" } else { "s" }));
        }
        report.notes.extend(self.other);
    }
}

/// Builds the dataset from the columns, repairing names as the CSV reader does.
fn assemble(name: &str, cols: Vec<Incoming>, notes: Notes) -> Result<(Dataset, ImportReport)> {
    let mut report = ImportReport::default();
    let mut ds = Dataset::new(name);
    for (i, c) in cols.into_iter().enumerate() {
        let used = unique_name(&ds, &c.name, i);
        if used != c.name {
            report.renamed.push((c.name.clone(), used.clone()));
        }
        let column = match c.data {
            Data::Numeric(v) => Column::Numeric(v),
            Data::Text(v) => Column::Text(v.into_iter().map(|s| s.filter(|t| !t.trim().is_empty())).collect()),
        };
        let numeric = matches!(column, Column::Numeric(_));
        ds.add_variable(&used, column)?;
        let v = ds.variable_mut(&used)?;
        v.label = c.label.trim().to_string();
        if numeric {
            v.value_labels = c.value_labels;
            v.missing_values = c.missing_values;
            v.missing_ranges = c.missing_ranges;
        }
        if let Some(m) = c.measure {
            v.measure = m;
        }
    }
    report.rows = ds.n_rows();
    report.variables = ds.variables.len();
    notes.into_report(&mut report);
    Ok((ds, report))
}

/// The error for a file a reader couldn't make sense of, with the reader's own explanation.
fn unreadable<E: std::fmt::Display>(ext: &'static str) -> impl Fn(E) -> Error + Copy {
    move |e| Error::Invalid(format!("The .{ext} file could not be read: {e}"))
}

/// Days since 1970-01-01 as an ISO date.
fn iso_date(days: i64) -> String {
    let (y, m, d) = crate::xlsx::civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Seconds since 1970-01-01 as an ISO date, with the time when it isn't midnight.
fn iso_datetime(secs: f64) -> String {
    let whole = secs.floor() as i64;
    let (days, rest) = (whole.div_euclid(86_400), whole.rem_euclid(86_400));
    if rest == 0 {
        iso_date(days)
    } else {
        format!("{}T{:02}:{:02}:{:02}", iso_date(days), rest / 3600, (rest % 3600) / 60, rest % 60)
    }
}

fn hms(secs: i64) -> String {
    let s = secs.rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// Days between 1960-01-01 (the epoch of the .dta and .sas7bdat formats) and 1970-01-01.
const DAYS_1960_TO_1970: i64 = 3653;

// ---------------------------------------------------------------- .sav and .zsav

pub fn read_sav(bytes: &[u8], name: &str) -> Result<(Dataset, ImportReport)> {
    use arrow_array::cast::AsArray;
    use arrow_array::types::{Date32Type, DurationMicrosecondType, Float64Type, TimestampMicrosecondType};
    use arrow_array::Array;
    use arrow_schema::{DataType, TimeUnit};

    let damaged = unreadable("sav");
    crate::limits::check_zsav(bytes)?;
    let (batch, meta) = ambers::read_sav_from_reader(Cursor::new(bytes)).map_err(damaged)?;
    let mut notes = Notes::default();
    notes.other.extend(meta.warnings.iter().map(|w| format!("The file may be damaged: {w}")));
    let mut cols = Vec::new();
    for (i, field) in batch.schema().fields().iter().enumerate() {
        let a = batch.column(i);
        let var = field.name().clone();
        let n = a.len();
        let data = match a.data_type() {
            DataType::Float64 => {
                let p = a.as_primitive::<Float64Type>();
                Data::Numeric((0..n).map(|r| p.is_valid(r).then(|| p.value(r)).filter(|v| v.is_finite())).collect())
            }
            DataType::Date32 => {
                notes.dates.push(var.clone());
                let p = a.as_primitive::<Date32Type>();
                Data::Text((0..n).map(|r| p.is_valid(r).then(|| iso_date(p.value(r) as i64))).collect())
            }
            DataType::Timestamp(TimeUnit::Microsecond, _) => {
                notes.dates.push(var.clone());
                let p = a.as_primitive::<TimestampMicrosecondType>();
                Data::Text((0..n).map(|r| p.is_valid(r).then(|| iso_datetime(p.value(r) as f64 / 1e6))).collect())
            }
            DataType::Duration(TimeUnit::Microsecond) => {
                notes.dates.push(var.clone());
                let p = a.as_primitive::<DurationMicrosecondType>();
                Data::Text((0..n).map(|r| p.is_valid(r).then(|| hms(p.value(r) / 1_000_000))).collect())
            }
            DataType::Utf8View => {
                let s = a.as_string_view();
                Data::Text((0..n).map(|r| s.is_valid(r).then(|| s.value(r).trim_end().to_string())).collect())
            }
            DataType::Utf8 => {
                let s = a.as_string::<i32>();
                Data::Text((0..n).map(|r| s.is_valid(r).then(|| s.value(r).trim_end().to_string())).collect())
            }
            other => return Err(Error::Invalid(format!("Variable {var} in the .sav file has a type PrismStatistics can't read yet ({other})."))),
        };
        let mut c = Incoming::new(var.clone(), data);
        c.label = meta.variable_labels.get(&var).cloned().unwrap_or_default();
        if let Some(labels) = meta.variable_value_labels.get(&var) {
            let numeric: Vec<ValueLabel> = labels
                .iter()
                .filter_map(|(k, l)| match k {
                    ambers::Value::Numeric(v) => Some(ValueLabel { value: *v, label: l.clone() }),
                    ambers::Value::String(_) => None,
                })
                .collect();
            if matches!(c.data, Data::Text(_)) && !labels.is_empty() {
                notes.text_labels.push(var.clone());
            }
            c.value_labels = numeric;
            c.value_labels.sort_by(|a, b| a.value.total_cmp(&b.value));
        }
        for spec in meta.variable_missing_values.get(&var).into_iter().flatten() {
            match spec {
                ambers::MissingSpec::Value(v) => c.missing_values.push(*v),
                ambers::MissingSpec::Range { lo, hi } => c.missing_ranges.push((*lo, *hi)),
                ambers::MissingSpec::StringValue(_) => {}
            }
        }
        c.measure = match meta.variable_measures.get(&var) {
            Some(ambers::Measure::Nominal) => Some(Measure::Nominal),
            Some(ambers::Measure::Ordinal) => Some(Measure::Ordinal),
            Some(ambers::Measure::Scale) => Some(Measure::Scale),
            _ => None,
        };
        cols.push(c);
    }
    assemble(name, cols, notes)
}

// ---------------------------------------------------------------- .dta

pub fn read_dta(bytes: &[u8], name: &str) -> Result<(Dataset, ImportReport)> {
    // The reader's module paths carry the format's original name; these two lines are its only mention.
    use dta::stata::dta::{dta_reader::DtaReader, long_string_ref::LongStringRef, long_string_table::LongStringTable, value::Value, variable_type::VariableType};
    use dta::stata::{missing_value::MissingValue, stata_byte::StataByte as B, stata_double::StataDouble as D, stata_float::StataFloat as F, stata_int::StataInt as I, stata_long::StataLong as L};

    let damaged = unreadable("dta");
    let schema_reader = DtaReader::new().from_reader(Cursor::new(bytes)).read_header().map_err(damaged)?;
    let mut characteristics = schema_reader.read_schema().map_err(damaged)?;
    characteristics.skip_to_end().map_err(damaged)?;
    let mut records = characteristics.into_record_reader().map_err(damaged)?;
    let schema = records.schema().clone();
    let vars = schema.variables();

    enum Cell {
        Num(Option<f64>),
        Text(Option<String>),
        Long(LongStringRef),
    }
    let mut special = 0usize;
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    while let Some(record) = records.read_record().map_err(damaged)? {
        let mut row = Vec::with_capacity(vars.len());
        for v in record.values() {
            let (present, missing) = match v {
                Value::Byte(x) => (x.present().map(f64::from), if let B::Missing(m) = x { Some(*m) } else { None }),
                Value::Int(x) => (x.present().map(f64::from), if let I::Missing(m) = x { Some(*m) } else { None }),
                Value::Long(x) => (x.present().map(f64::from), if let L::Missing(m) = x { Some(*m) } else { None }),
                Value::Float(x) => (x.present().map(f64::from), if let F::Missing(m) = x { Some(*m) } else { None }),
                Value::Double(x) => (x.present(), if let D::Missing(m) = x { Some(*m) } else { None }),
                Value::String(s) => {
                    row.push(Cell::Text(Some(s.to_string())));
                    continue;
                }
                Value::LongStringRef(r) => {
                    row.push(Cell::Long(*r));
                    continue;
                }
            };
            special += usize::from(missing.is_some_and(|m| m != MissingValue::System));
            row.push(Cell::Num(present));
        }
        rows.push(row);
    }
    let mut long_reader = records.into_long_string_reader().map_err(damaged)?;
    let encoding = long_reader.encoding();
    let mut table = LongStringTable::for_reading();
    long_reader.read_remaining_into(&mut table).map_err(damaged)?;
    let mut label_reader = long_reader.into_value_label_reader().map_err(damaged)?;
    let mut sets = Vec::new();
    while let Some(set) = label_reader.read_value_label_set().map_err(damaged)? {
        sets.push(set);
    }

    let mut notes = Notes { special_missing: special, ..Notes::default() };
    let mut cols = Vec::new();
    for (j, v) in vars.iter().enumerate() {
        let text = matches!(v.variable_type(), VariableType::FixedString(_) | VariableType::LongString);
        let fmt = v.format().trim_start_matches('%').trim_start_matches('-');
        let data = if text {
            Data::Text(
                rows.iter()
                    .map(|r| match &r[j] {
                        Cell::Text(s) => s.clone(),
                        Cell::Long(rf) => table.get(rf).and_then(|l| l.data_str(encoding).map(|s| s.trim_end_matches('\0').to_string())),
                        Cell::Num(_) => None,
                    })
                    .collect(),
            )
        } else {
            let nums = rows.iter().map(|r| if let Cell::Num(x) = &r[j] { *x } else { None });
            if fmt.starts_with("td") || fmt.starts_with('d') {
                notes.dates.push(v.name().to_string());
                Data::Text(nums.map(|x| x.map(|d| iso_date(d.round() as i64 - DAYS_1960_TO_1970))).collect())
            } else if fmt.starts_with("tc") || fmt.starts_with("tC") {
                notes.dates.push(v.name().to_string());
                Data::Text(nums.map(|x| x.map(|ms| iso_datetime(ms / 1000.0 - (DAYS_1960_TO_1970 * 86_400) as f64))).collect())
            } else {
                Data::Numeric(nums.collect())
            }
        };
        let mut c = Incoming::new(v.name(), data);
        c.label = v.label().to_string();
        let set_name = v.value_label_name();
        if !set_name.is_empty() {
            if let Some(set) = sets.iter().find(|s| s.name() == set_name) {
                if matches!(c.data, Data::Numeric(_)) {
                    c.value_labels = set.entries().iter().map(|e| ValueLabel { value: f64::from(e.value()), label: e.label().to_string() }).collect();
                    c.value_labels.sort_by(|a, b| a.value.total_cmp(&b.value));
                } else {
                    notes.text_labels.push(v.name().to_string());
                }
            }
        }
        cols.push(c);
    }
    assemble(name, cols, notes)
}

// ---------------------------------------------------------------- .sas7bdat

pub fn read_sas7bdat(bytes: &[u8], name: &str) -> Result<(Dataset, ImportReport)> {
    use sas7bdat::{CellValue, Dataset as SasFile, LogicalType};
    use std::ops::ControlFlow;

    let damaged = unreadable("sas7bdat");
    let file = SasFile::from_bytes(bytes.to_vec()).map_err(damaged)?;
    let metas = file.columns().to_vec();
    let mut notes = Notes::default();
    let mut data: Vec<Data> = metas
        .iter()
        .map(|m| match m.logical_type {
            LogicalType::String => Data::Text(Vec::new()),
            LogicalType::Integer | LogicalType::Float => Data::Numeric(Vec::new()),
            _ => {
                notes.dates.push(m.name.clone());
                Data::Text(Vec::new())
            }
        })
        .collect();
    file.visit_rows(|row| {
        for (j, cell) in row.iter().enumerate() {
            match (&mut data[j], cell) {
                (Data::Numeric(v), CellValue::Int32(x)) => v.push(Some(f64::from(*x))),
                (Data::Numeric(v), CellValue::Int64(x)) => v.push(Some(*x as f64)),
                (Data::Numeric(v), CellValue::Float64(x)) => v.push(Some(*x).filter(|x| x.is_finite())),
                (Data::Numeric(v), _) => v.push(None),
                (Data::Text(v), CellValue::Str(s)) => v.push(Some(s.trim_end().to_string())),
                (Data::Text(v), CellValue::Bytes(b)) => v.push(Some(String::from_utf8_lossy(b).trim_end().to_string())),
                (Data::Text(v), CellValue::Date(d)) => v.push(Some(iso_date(i64::from(d.days_since_sas_epoch) - DAYS_1960_TO_1970))),
                (Data::Text(v), CellValue::DateTime(t)) => v.push(Some(iso_datetime((t.seconds_since_sas_epoch - DAYS_1960_TO_1970 * 86_400) as f64))),
                (Data::Text(v), CellValue::Time(t)) => v.push(Some(hms(i64::from(t.seconds_since_midnight)))),
                (Data::Text(v), CellValue::Int32(x)) => v.push(Some(x.to_string())),
                (Data::Text(v), CellValue::Int64(x)) => v.push(Some(x.to_string())),
                (Data::Text(v), CellValue::Float64(x)) => v.push(Some(crate::model::format_code(*x))),
                (Data::Text(v), CellValue::Null) => v.push(None),
            }
        }
        Ok(ControlFlow::Continue(()))
    })
    .map_err(damaged)?;
    let cols = metas
        .iter()
        .zip(data)
        .map(|(m, d)| {
            let mut c = Incoming::new(m.name.clone(), d);
            c.label = m.label.clone().unwrap_or_default();
            c
        })
        .collect();
    let (ds, mut report) = assemble(name, cols, notes)?;
    // Keep each variable's format name (under its name in PrismStatistics) for the catalog's value labels.
    for (m, v) in metas.iter().zip(&ds.variables) {
        if let Some(f) = m.format.as_deref().map(sas7bdat::catalog::normalize_format_name).filter(|f| !f.is_empty()) {
            report.sas_formats.push((v.name.clone(), f));
        }
    }
    Ok((ds, report))
}

/// The value-label sets of a SAS format catalog (.sas7bcat).
pub fn read_sas_catalog(bytes: &[u8]) -> Result<Vec<sas7bdat::LabelSet>> {
    let layout = sas7bdat::catalog::parse_catalog(&mut Cursor::new(bytes)).map_err(|e| Error::Invalid(format!("This file isn't a SAS format catalog (.sas7bcat), or it is damaged ({e}).")))?;
    Ok(layout.label_sets)
}

/// What attaching a catalog did, in plain words for the import report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogOutcome {
    /// Variables that got value labels.
    pub labeled: Vec<String>,
    /// Text variables whose format has labels; PrismStatistics keeps value labels for numeric codes only.
    pub text_skipped: Vec<String>,
    /// Format names the data use that the catalog doesn't define (built-in SAS formats such as BEST12 are not listed).
    pub missing: Vec<String>,
}

/// Attaches value labels from catalog label sets to the variables whose SAS format names them. Existing labels for the
/// same code are replaced; others are kept. Special missing codes (.A to .Z) have no numeric code here and are skipped.
pub fn apply_sas_labels(ds: &mut Dataset, formats: &[(String, String)], sets: &[sas7bdat::LabelSet]) -> CatalogOutcome {
    let mut out = CatalogOutcome::default();
    for (var, format) in formats {
        let Some(set) = sets.iter().find(|s| sas7bdat::catalog::normalize_format_name(&s.name) == *format) else {
            // Built-in formats (BEST, DATE, $CHAR, ...) aren't in a user catalog; only name the ones that look custom.
            if !is_builtin_format(format) && !out.missing.contains(format) {
                out.missing.push(format.clone());
            }
            continue;
        };
        let Ok(v) = ds.variable(var) else { continue };
        let i = ds.find(var).expect("found");
        if matches!(ds.columns[i], Column::Text(_)) || set.value_type == sas7bdat::ValueType::String {
            out.text_skipped.push(v.display_name().to_string());
            continue;
        }
        let v = ds.variable_mut(var).expect("found");
        let mut added = false;
        for l in &set.labels {
            let code = match l.key {
                sas7bdat::ValueKey::Numeric(x) => x,
                sas7bdat::ValueKey::Integer(x) => f64::from(x),
                _ => continue,
            };
            v.value_labels.retain(|x| x.value != code);
            v.value_labels.push(ValueLabel { value: code, label: l.label.trim_end().to_string() });
            added = true;
        }
        v.value_labels.sort_by(|a, b| a.value.total_cmp(&b.value));
        if added {
            out.labeled.push(v.display_name().to_string());
        }
    }
    out
}

/// SAS's own formats, which never come from a catalog.
fn is_builtin_format(f: &str) -> bool {
    let base = f.trim_start_matches('$').trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    ["BEST", "F", "W", "COMMA", "COMMAX", "DOLLAR", "PERCENT", "Z", "E", "CHAR", "DATE", "DATETIME", "TIME", "MMDDYY", "DDMMYY", "YYMMDD", "WEEKDATE", "WORDDATE", "MONYY", "YEAR", "HHMM", "NLNUM", "BESTD", "UPCASE", "HEX", "ROMAN", "WORDS", "", "IS8601DA", "IS8601DT", "E8601DA", "E8601DT", "B8601DA"]
        .contains(&base)
}

/// The outcome as sentences for the import report.
pub fn catalog_notes(o: &CatalogOutcome, file: &str) -> Vec<String> {
    let mut n = Vec::new();
    if !o.labeled.is_empty() {
        n.push(format!("Value labels for {} came from {file}.", crate::apa::series(&o.labeled)));
    } else {
        n.push(format!("{file} had no value labels for these variables' formats."));
    }
    if !o.text_skipped.is_empty() {
        n.push(format!("{} {} text, and PrismStatistics keeps value labels for numeric codes only, so {} labels were not attached.", crate::apa::series(&o.text_skipped), if o.text_skipped.len() == 1 { "is" } else { "are" }, if o.text_skipped.len() == 1 { "its" } else { "their" }));
    }
    if !o.missing.is_empty() {
        n.push(format!("{file} doesn't define the format{} {}.", if o.missing.len() == 1 { "" } else { "s" }, o.missing.join(", ")));
    }
    n
}

// ---------------------------------------------------------------- .jasp (data only)

/// The data of a JASP file (version 0.17 and later): a zip archive whose `internal.sqlite` database holds the columns.
/// Each cell is stored twice, as a number and as a key into the column's labels, whose `originalValueJson` is the value
/// as it was entered (a number or text). The analyses in the file are not read.
pub fn read_jasp(bytes: &[u8], name: &str) -> Result<(Dataset, ImportReport)> {

    let not_jasp = || Error::Invalid("This file isn't a .jasp file, or it is damaged.".into());
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| not_jasp())?;
    let db = match zip.by_name("internal.sqlite") {
        Ok(f) => crate::limits::read_capped(f, "This .jasp file")?,
        Err(_) => {
            return Err(Error::Invalid(
                "This .jasp file was saved by a version older than 0.17, which keeps its data differently. Open it in a current version of the program that made it and save it again, or export the data as CSV.".into(),
            ))
        }
    };
    // SQLite reads from a file, so the database is copied to a temporary one that is removed afterwards. Its name is
    // unpredictable and it must be new, so nothing else can be put in its place.
    let (tmp, mut file) = new_temp_file()?;
    std::io::Write::write_all(&mut file, &db)?;
    drop(file);
    let result = read_jasp_db(&tmp, name);
    let _ = std::fs::remove_file(&tmp);
    result
}

static JASP_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// A new file in the temporary folder with a random name, created only if nothing has that name.
fn new_temp_file() -> Result<(std::path::PathBuf, std::fs::File)> {
    use std::hash::{BuildHasher, Hasher};
    for _ in 0..16 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u32(JASP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        let path = std::env::temp_dir().join(format!("prism-jasp-{:016x}.sqlite", h.finish()));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => return Ok((path, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(Error::Invalid("A temporary file for the .jasp data could not be made.".into()))
}

/// A value as it was entered in JASP.
#[derive(Clone)]
enum Orig {
    Num(f64),
    Text(String),
}

struct JaspColumn {
    id: i64,
    name: String,
    title: String,
    kind: String,
    empty: Option<Vec<String>>,
    computed: bool,
}

fn json_strings(j: Option<&str>) -> Option<Vec<String>> {
    let v: serde_json::Value = serde_json::from_str(j?).ok()?;
    Some(v.get("strings")?.as_array()?.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
}

fn read_jasp_db(path: &std::path::Path, name: &str) -> Result<(Dataset, ImportReport)> {
    use rusqlite::{types::Value as Sql, Connection, OpenFlags};
    use serde_json::Value as J;

    let sql = |e: rusqlite::Error| Error::Invalid(format!("The data in this .jasp file could not be read: {e}"));
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(sql)?;
    // The database comes from the file, so its schema is not trusted: no functions or views from it run with side
    // effects, and defensive mode refuses writes to its internals.
    c.pragma_update(None, "trusted_schema", "OFF").map_err(sql)?;
    c.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true).map_err(sql)?;
    let (ds_id, empty_json): (i64, Option<String>) = c.query_row("SELECT id, emptyValuesJson FROM DataSets ORDER BY id LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?))).map_err(sql)?;
    let empty_ds = json_strings(empty_json.as_deref()).unwrap_or_default();
    let table = format!("DataSet_{ds_id}");

    let mut stmt = c.prepare("SELECT id, name, title, columnType, emptyValuesJson, codeType FROM Columns WHERE dataSet = ?1 ORDER BY colIdx").map_err(sql)?;
    let cols: Vec<JaspColumn> = stmt
        .query_map([ds_id], |r| {
            let ev: Option<String> = r.get(4)?;
            let custom = ev.as_deref().and_then(|s| serde_json::from_str::<J>(s).ok()).is_some_and(|v| v.get("custom").and_then(J::as_bool) == Some(true));
            Ok(JaspColumn {
                id: r.get(0)?,
                name: r.get(1)?,
                title: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                kind: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                empty: if custom { json_strings(ev.as_deref()) } else { None },
                computed: r.get::<_, Option<String>>(5)?.is_some_and(|t| !t.is_empty()),
            })
        })
        .map_err(sql)?
        .collect::<std::result::Result<_, _>>()
        .map_err(sql)?;

    let mut notes = Notes::default();
    let filtered: i64 = c.query_row(&format!("SELECT COUNT(*) FROM \"{table}\" WHERE Filter_1 = 0"), [], |r| r.get(0)).unwrap_or(0);
    if filtered > 0 {
        notes.other.push(format!("The .jasp file had a filter that hid {filtered} row{}. Every row was imported; the filter was not.", if filtered == 1 { "" } else { "s" }));
    }
    let computed: Vec<&str> = cols.iter().filter(|c| c.computed).map(|c| c.name.as_str()).collect();
    if !computed.is_empty() {
        notes.other.push(format!("Computed columns were imported as plain values: {}.", computed.join(", ")));
    }
    let mut out = Vec::new();
    for col in &cols {
        let empty = col.empty.as_ref().unwrap_or(&empty_ds);
        let mut labels: Vec<(i64, String, Orig)> = Vec::new();
        let mut ls = c.prepare("SELECT value, label, originalValueJson FROM Labels WHERE columnId = ?1 ORDER BY ordering").map_err(sql)?;
        let rows = ls
            .query_map([col.id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get::<_, Option<String>>(2)?.unwrap_or_default())))
            .map_err(sql)?;
        for row in rows {
            let (key, label, orig) = row.map_err(sql)?;
            let orig = match serde_json::from_str::<J>(&orig) {
                Ok(J::Number(n)) => n.as_f64().map_or(Orig::Text(orig.trim().to_string()), Orig::Num),
                Ok(J::String(s)) => Orig::Text(s),
                _ => Orig::Text(orig.trim().to_string()),
            };
            labels.push((key, label, orig));
        }
        let mut cells: Vec<Option<Orig>> = Vec::new();
        let mut rs = c.prepare(&format!("SELECT \"Column_{0}_DBL\", \"Column_{0}_INT\" FROM \"{table}\" ORDER BY rowNumber", col.id)).map_err(sql)?;
        let rows = rs.query_map([], |r| Ok((r.get::<_, Sql>(0)?, r.get::<_, Option<i64>>(1)?))).map_err(sql)?;
        for row in rows {
            let (dbl, key) = row.map_err(sql)?;
            let labelled = key.filter(|k| *k != -1).and_then(|k| labels.iter().find(|l| l.0 == k));
            cells.push(match labelled {
                Some((_, _, Orig::Text(t))) if t.is_empty() || empty.contains(t) => None,
                Some((_, _, o)) => Some(o.clone()),
                None => match dbl {
                    Sql::Real(x) if x.is_finite() => Some(Orig::Num(x)),
                    Sql::Integer(x) => Some(Orig::Num(x as f64)),
                    _ => None,
                },
            });
        }
        let numeric = cells.iter().flatten().all(|o| matches!(o, Orig::Num(_)));
        let data = if numeric {
            Data::Numeric(cells.iter().map(|o| if let Some(Orig::Num(x)) = o { Some(*x) } else { None }).collect())
        } else {
            // Text categories show their label, which is what the JASP user sees.
            Data::Text(
                cells
                    .iter()
                    .map(|o| {
                        o.as_ref().map(|o| match o {
                            Orig::Num(x) => crate::model::format_code(*x),
                            Orig::Text(t) => labels.iter().find(|l| matches!(&l.2, Orig::Text(u) if u == t)).map_or_else(|| t.clone(), |l| l.1.clone()),
                        })
                    })
                    .collect(),
            )
        };
        let mut inc = Incoming::new(col.name.clone(), data);
        if !col.title.trim().is_empty() && col.title != col.name {
            inc.label = col.title.clone();
        }
        if numeric {
            inc.value_labels = labels
                .iter()
                .filter_map(|(_, l, o)| match o {
                    Orig::Num(x) if !l.is_empty() && *l != crate::model::format_code(*x) => Some(ValueLabel { value: *x, label: l.clone() }),
                    _ => None,
                })
                .collect();
            inc.value_labels.sort_by(|a, b| a.value.total_cmp(&b.value));
        }
        inc.measure = match col.kind.as_str() {
            "scale" if numeric => Some(Measure::Scale),
            "ordinal" => Some(Measure::Ordinal),
            "nominal" | "nominalText" => Some(Measure::Nominal),
            _ => None,
        };
        out.push(inc);
    }
    notes.other.push("Only the data were read from the .jasp file; its analyses can be run again from the Analyze menu.".into());
    assemble(name, out, notes)
}
