//! Reading files into datasets: CSV, TSV and `.xlsx` (see `xlsx.rs`). `.sav`, `.dta`, `.sas7bdat`, `.jasp` data and
//! Google Sheets follow (decision D06).

use std::io::Read;
use std::path::Path;

use crate::error::{Error, Result};
use crate::model::{valid_name, Column, Dataset};

/// What happened during an import, for the import report shown to the user.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportReport {
    pub rows: usize,
    pub variables: usize,
    /// Header names that had to be changed to be valid, unique variable names: (original, used).
    pub renamed: Vec<(String, String)>,
    /// Rows that were shorter than the header and were padded with empty cells.
    pub short_rows: usize,
    /// Fully empty rows that were left out (workbooks).
    #[serde(default)]
    pub skipped_blank_rows: usize,
    /// The sheet that was read, for a workbook.
    #[serde(default)]
    pub sheet: Option<String>,
    /// The workbook's other sheets, which were not read.
    #[serde(default)]
    pub other_sheets: Vec<String>,
    /// Anything else the user should know, in plain words.
    #[serde(default)]
    pub notes: Vec<String>,
    /// For a .sas7bdat file: each variable's SAS format name (variable, format), so value labels from a catalog
    /// (.sas7bcat) can be attached, at import or later.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sas_formats: Vec<(String, String)>,
}

/// One cell as read from a file, before the column's type is decided.
#[derive(Debug, Clone, PartialEq)]
pub enum RawCell {
    Empty,
    Number(f64),
    Text(String),
}

/// Reads a delimited text file. The first row names the variables. A column whose non-empty cells all read as numbers
/// becomes numeric; any other column is text. Empty cells are system missing.
pub fn read_delimited<R: Read>(reader: R, delimiter: u8, name: &str) -> Result<(Dataset, ImportReport)> {
    let mut rdr = csv::ReaderBuilder::new().delimiter(delimiter).has_headers(true).flexible(true).from_reader(reader);
    let headers: Vec<String> = rdr.headers()?.iter().map(|h| h.trim().trim_start_matches('\u{feff}').to_string()).collect();
    if headers.is_empty() || headers.iter().all(String::is_empty) {
        return Err(Error::NoHeader);
    }
    let mut report = ImportReport::default();
    let mut rows = Vec::new();
    for record in rdr.records() {
        let record = record?;
        if record.len() < headers.len() {
            report.short_rows += 1;
        }
        rows.push(
            (0..headers.len())
                .map(|c| {
                    let s = record.get(c).unwrap_or("").trim();
                    if s.is_empty() {
                        RawCell::Empty
                    } else {
                        RawCell::Text(s.to_string())
                    }
                })
                .collect(),
        );
    }
    let ds = build_dataset(name, &headers, rows, &mut report)?;
    Ok((ds, report))
}

/// Builds a dataset from a header row and rows of raw cells, deciding each column's type and repairing names.
pub(crate) fn build_dataset(name: &str, headers: &[String], rows: Vec<Vec<RawCell>>, report: &mut ImportReport) -> Result<Dataset> {
    let width = headers.len();
    let mut columns: Vec<Vec<RawCell>> = vec![Vec::with_capacity(rows.len()); width];
    for row in rows {
        let mut row = row.into_iter();
        for column in columns.iter_mut() {
            column.push(row.next().unwrap_or(RawCell::Empty));
        }
    }
    let mut ds = Dataset::new(name);
    for (c, header) in headers.iter().enumerate() {
        let header = header.trim();
        let used = unique_name(&ds, header, c);
        if used != header {
            report.renamed.push((header.to_string(), used.clone()));
        }
        ds.add_variable(&used, to_column(&columns[c]))?;
    }
    report.rows = ds.n_rows();
    report.variables = ds.variables.len();
    Ok(ds)
}

/// Reads a `.csv`, `.tsv` or `.xlsx` file, choosing the reader from the extension.
pub fn read_file(path: &Path) -> Result<(Dataset, ImportReport)> {
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Dataset").to_string();
    let bytes = std::fs::read(path)?;
    read_bytes(&path.to_string_lossy(), &name, &bytes, None)
}

/// Reads a file's bytes, choosing the reader from the file name's extension. `sheet` picks a workbook sheet.
pub fn read_bytes(file_name: &str, name: &str, bytes: &[u8], sheet: Option<&str>) -> Result<(Dataset, ImportReport)> {
    match extension(file_name).as_deref() {
        Some("xlsx") | Some("xlsm") => crate::xlsx::read_xlsx(bytes, name, sheet),
        Some("xls") | Some("ods") => Err(Error::Invalid("Only .xlsx workbooks can be opened for now. Save the file as .xlsx or .csv and open that.".into())),
        Some("sav") | Some("zsav") => crate::statfile::read_sav(bytes, name),
        Some("dta") => crate::statfile::read_dta(bytes, name),
        Some("sas7bdat") => crate::statfile::read_sas7bdat(bytes, name),
        Some("jasp") => crate::statfile::read_jasp(bytes, name),
        Some("tsv") | Some("tab") => read_delimited(bytes, b'\t', name),
        _ => read_delimited(bytes, b',', name),
    }
}

pub fn extension(file_name: &str) -> Option<String> {
    Path::new(file_name).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase)
}

fn to_column(cells: &[RawCell]) -> Column {
    let as_number = |c: &RawCell| match c {
        RawCell::Number(n) => Some(*n),
        RawCell::Text(s) => parse_number(s),
        RawCell::Empty => None,
    };
    let numeric = cells.iter().all(|c| matches!(c, RawCell::Empty) || as_number(c).is_some());
    if numeric && cells.iter().any(|c| !matches!(c, RawCell::Empty)) {
        Column::Numeric(cells.iter().map(as_number).collect())
    } else {
        Column::Text(
            cells
                .iter()
                .map(|c| match c {
                    RawCell::Empty => None,
                    RawCell::Text(s) => Some(s.clone()),
                    RawCell::Number(n) => Some(crate::model::format_code(*n)),
                })
                .collect(),
        )
    }
}

fn parse_number(s: &str) -> Option<f64> {
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Turns a header into a valid variable name that isn't taken yet.
pub(crate) fn unique_name(ds: &Dataset, header: &str, index: usize) -> String {
    let mut base: String = header.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    base = base.trim_matches('_').to_string();
    if base.is_empty() {
        base = format!("V{}", index + 1);
    }
    if !base.chars().next().is_some_and(char::is_alphabetic) {
        base = format!("V_{base}");
    }
    base = base.chars().take(60).collect();
    let mut name = base.clone();
    let mut k = 2;
    while ds.find(&name).is_some() || !valid_name(&name) {
        name = format!("{base}_{k}");
        k += 1;
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_numbers_text_and_blanks() {
        let data = "id,group,score,note\n1,1,4.5,ok\n2,2,,\n3,1,-99,late\n";
        let (ds, report) = read_delimited(data.as_bytes(), b',', "t").unwrap();
        assert_eq!(report.rows, 3);
        assert_eq!(ds.columns[2], Column::Numeric(vec![Some(4.5), None, Some(-99.0)]));
        assert_eq!(ds.columns[3], Column::Text(vec![Some("ok".into()), None, Some("late".into())]));
    }

    #[test]
    fn repairs_bad_and_duplicate_headers() {
        let data = "Age (years),age,,2nd\n1,2,3,4\n";
        let (ds, report) = read_delimited(data.as_bytes(), b',', "t").unwrap();
        let names: Vec<_> = ds.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["Age__years", "age", "V3", "V_2nd"]);
        assert_eq!(report.renamed.len(), 3);
    }
}
