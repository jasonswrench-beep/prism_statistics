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
    /// Rows that had more fields than the header; the extra fields were left out.
    #[serde(default)]
    pub long_rows: usize,
    /// Fully empty rows that were left out.
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
        // A line of nothing but spaces (or delimiters) is a blank row, left out like the empty lines the reader skips.
        if record.iter().all(|f| f.trim().is_empty()) {
            report.skipped_blank_rows += 1;
            continue;
        }
        if record.len() < headers.len() {
            report.short_rows += 1;
        } else if record.iter().skip(headers.len()).any(|f| !f.trim().is_empty()) {
            report.long_rows += 1;
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
    let rows = |n: usize| if n == 1 { "1 row".to_string() } else { format!("{n} rows") };
    if report.short_rows > 0 {
        report.notes.push(format!("{} had fewer fields than the header; the missing cells are empty.", rows(report.short_rows)));
    }
    if report.long_rows > 0 {
        report.notes.push(format!("{} had more fields than the header; the extra fields were left out. Check the delimiter and any quotes in those rows.", rows(report.long_rows)));
    }
    if report.skipped_blank_rows > 0 {
        let were = if report.skipped_blank_rows == 1 { "was" } else { "were" };
        report.notes.push(format!("{} that {were} blank {were} left out.", rows(report.skipped_blank_rows)));
    }
    Ok((ds, report))
}

/// The delimiter of a text file whose extension doesn't say: tab, semicolon or comma, whichever the first line has
/// most of (comma when it has none).
fn sniff_delimiter(bytes: &[u8]) -> u8 {
    let first = bytes.split(|b| *b == b'\n').next().unwrap_or(&[]);
    let count = |d: u8| first.iter().filter(|b| **b == d).count();
    [b'\t', b';', b','].into_iter().max_by_key(|d| (count(*d), *d == b',')).filter(|d| count(*d) > 0).unwrap_or(b',')
}

/// Whether the bytes look like a binary file (a document, an archive, an image) rather than text.
fn looks_binary(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8192)];
    head.starts_with(b"PK\x03\x04") || head.starts_with(b"%PDF") || head.starts_with(b"\xD0\xCF\x11\xE0") || head.contains(&0)
}

/// The file types the reader opens, for messages.
pub const OPENS: &str = ".csv, .tsv, .txt, .xlsx, .sav, .zsav, .dta, .sas7bdat and .jasp, and xAPI statements as .json,";

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
        // Numbers are measured as scale, except an identifier (a different whole number for nearly every case).
        let last = ds.variables.len() - 1;
        if let Column::Numeric(v) = &ds.columns[last] {
            let vals: Vec<f64> = v.iter().flatten().copied().collect();
            if crate::health::looks_like_identifier(&used, &vals) {
                ds.variables[last].measure = crate::model::Measure::Nominal;
            }
        }
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
        // A project's dataset, so Add Cases and Add Variables can take one: a PrismQDA case table or an OpenAssess
        // results file, which are .pstat files (PSTAT-1). Its results and record stay in that project.
        Some("pstat") => {
            let p = crate::project::from_bytes(bytes)?;
            let mut notes = vec!["Only the project's data was read; its results stay in that project.".to_string()];
            if let Some(profile) = &p.profile {
                notes.push(format!("The file says it follows the profile \"{profile}\"."));
            }
            let report = ImportReport { rows: p.dataset.n_rows(), variables: p.dataset.variables.len(), notes, ..Default::default() };
            Ok((p.dataset, report))
        }
        Some("tsv") | Some("tab") => text_data(bytes, b'\t', name),
        Some("csv") => text_data(bytes, b',', name),
        None | Some("txt") | Some("dat") | Some("data") => text_data(bytes, sniff_delimiter(bytes), name),
        // xAPI statements from a learning record store; any other JSON is refused.
        Some("json") | Some("jsonl") => match crate::xapi::parse(bytes).as_ref().and_then(crate::xapi::statements) {
            Some(list) => crate::xapi::read(&list, name),
            None => Err(Error::Invalid(format!("Only xAPI statements (a learning record store's export) can be opened from a JSON file. Save other tables as .csv and open that. PrismStatistics opens {OPENS} files."))),
        },
        Some(other) => Err(Error::Invalid(format!("PrismStatistics can't open .{other} files as data. It opens {OPENS} files."))),
    }
}

/// A delimited text file, refused when it isn't text at all.
fn text_data(bytes: &[u8], delimiter: u8, name: &str) -> Result<(Dataset, ImportReport)> {
    if looks_binary(bytes) {
        return Err(Error::Invalid(format!("This file isn't a text data file (it looks like a document, an archive or an image). PrismStatistics opens {OPENS} files.")));
    }
    read_delimited(bytes, delimiter, name)
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

    #[test]
    fn says_what_happened_to_short_long_and_blank_rows() {
        let data = "a,b,c,d\n1,2,3,4\n5,6\n\n   \n7,8,9,10,11\n12,13,14,15\n";
        let (ds, report) = read_bytes("t.csv", "t", data.as_bytes(), None).unwrap();
        assert_eq!((report.rows, report.short_rows, report.long_rows, report.skipped_blank_rows), (4, 1, 1, 1));
        assert_eq!(ds.n_rows(), 4);
        let notes = report.notes.join(" ");
        assert!(notes.contains("1 row had fewer fields") && notes.contains("1 row had more fields") && notes.contains("blank was left out"), "{notes}");
    }

    #[test]
    fn refuses_files_that_are_not_data() {
        for (name, bytes) in [("notes.docx", &b"PK\x03\x04rest"[..]), ("a.json", b"{\"x\": 1}"), ("a.pdf", b"%PDF-1.7")] {
            let e = read_bytes(name, "t", bytes, None).unwrap_err().to_string();
            assert!(e.contains("opens .csv"), "{name}: {e}");
        }
        // A document renamed to .csv is refused too.
        let e = read_bytes("renamed.csv", "t", b"PK\x03\x04\0\0", None).unwrap_err().to_string();
        assert!(e.contains("isn't a text data file"), "{e}");
    }

    #[test]
    fn a_text_file_gets_the_delimiter_its_first_line_uses() {
        let (ds, _) = read_bytes("t.txt", "t", b"a\tb\tc\n1\t2\t3\n", None).unwrap();
        assert_eq!(ds.variables.len(), 3);
        let (ds, _) = read_bytes("t.txt", "t", b"a;b\n1,5;2\n", None).unwrap();
        assert_eq!(ds.variables.len(), 2);
    }
}
