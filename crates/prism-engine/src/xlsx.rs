//! Reading `.xlsx` workbooks (decision D06). One sheet becomes one dataset: the first non-empty row names the
//! variables, fully empty rows are skipped, and formulas come in as the values they last calculated. A column of
//! numbers becomes numeric; a column with any text becomes text. Dates come in as ISO text (`2026-01-02`) for now,
//! because the date type of the data model is still to come, and the import report says so.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx};

use crate::error::{Error, Result};
use crate::import::{build_dataset, ImportReport, RawCell};
use crate::model::Dataset;

fn open(bytes: &[u8]) -> Result<Xlsx<Cursor<&[u8]>>> {
    crate::limits::check_zip(bytes, "This workbook")?;
    calamine::open_workbook_from_rs(Cursor::new(bytes)).map_err(|e| Error::Invalid(format!("The workbook could not be opened. It may be damaged or password protected. ({e})")))
}

/// The sheets of a workbook in order, with whether each is visible.
pub fn sheet_names(bytes: &[u8]) -> Result<Vec<(String, bool)>> {
    let wb = open(bytes)?;
    Ok(wb.sheets_metadata().iter().map(|m| (m.name.clone(), m.visible == calamine::SheetVisible::Visible)).collect())
}

/// Reads one sheet, or the first visible sheet when `sheet` is None.
pub fn read_xlsx(bytes: &[u8], name: &str, sheet: Option<&str>) -> Result<(Dataset, ImportReport)> {
    let mut wb = open(bytes)?;
    let meta: Vec<(String, bool)> = wb.sheets_metadata().iter().map(|m| (m.name.clone(), m.visible == calamine::SheetVisible::Visible)).collect();
    if meta.is_empty() {
        return Err(Error::Invalid("The workbook has no sheets.".into()));
    }
    let chosen = match sheet {
        Some(s) => meta.iter().find(|(n, _)| n == s).map(|(n, _)| n.clone()).ok_or_else(|| Error::Invalid(format!("The workbook has no sheet named \"{s}\".")))?,
        None => meta.iter().find(|(_, v)| *v).unwrap_or(&meta[0]).0.clone(),
    };
    let range = wb.worksheet_range(&chosen).map_err(|e| Error::Invalid(format!("The sheet \"{chosen}\" could not be read. ({e})")))?;

    let mut report = ImportReport::default();
    let mut errors = 0usize;
    let mut dates = 0usize;
    let rows: Vec<Vec<RawCell>> = range
        .rows()
        .map(|row| {
            row.iter()
                .map(|d| match d {
                    Data::Empty => RawCell::Empty,
                    Data::String(s) if s.trim().is_empty() => RawCell::Empty,
                    Data::String(s) => RawCell::Text(s.trim().to_string()),
                    Data::Float(f) => RawCell::Number(*f),
                    Data::Int(i) => RawCell::Number(*i as f64),
                    Data::Bool(b) => RawCell::Text(if *b { "TRUE".into() } else { "FALSE".into() }),
                    Data::DateTime(dt) => {
                        dates += 1;
                        // calamine's as_f64 already moves the old 1904 date system onto the standard one.
                        RawCell::Text(serial_to_iso(dt.as_f64()))
                    }
                    Data::DateTimeIso(s) => {
                        dates += 1;
                        RawCell::Text(s.clone())
                    }
                    Data::DurationIso(s) => RawCell::Text(s.clone()),
                    Data::Error(_) => {
                        errors += 1;
                        RawCell::Empty
                    }
                })
                .collect()
        })
        .collect();
    let first = rows.iter().position(|r| r.iter().any(|c| !matches!(c, RawCell::Empty))).ok_or_else(|| Error::Invalid(format!("The sheet \"{chosen}\" is empty.")))?;
    let headers: Vec<String> = rows[first]
        .iter()
        .map(|c| match c {
            RawCell::Empty => String::new(),
            RawCell::Text(s) => s.clone(),
            RawCell::Number(n) => crate::model::format_code(*n),
        })
        .collect();
    let body: Vec<Vec<RawCell>> = rows[first + 1..].iter().filter(|r| r.iter().any(|c| !matches!(c, RawCell::Empty))).cloned().collect();
    report.skipped_blank_rows = rows.len() - first - 1 - body.len();
    let ds = build_dataset(name, &headers, body, &mut report)?;
    report.sheet = Some(chosen.clone());
    report.other_sheets = meta.iter().map(|(n, _)| n.clone()).filter(|n| *n != chosen).collect();
    if errors > 0 {
        report.notes.push(format!("{errors} cell(s) held a formula error and were left empty."));
    }
    if dates > 0 {
        report.notes.push(format!("{dates} date cell(s) were read as text (YYYY-MM-DD). Date variables are coming in a later release."));
    }
    Ok((ds, report))
}

/// A spreadsheet serial day number (day 0 = 1899-12-30) as an ISO date, with the time when there is one.
fn serial_to_iso(serial: f64) -> String {
    if !serial.is_finite() {
        return String::new();
    }
    let whole = serial.floor();
    let secs = ((serial - whole) * 86_400.0).round() as i64;
    let (shift, secs) = if secs >= 86_400 { (1, secs - 86_400) } else { (0, secs) };
    let (y, m, d) = civil_from_days(days_from_civil(1899, 12, 30) + whole as i64 + shift);
    if secs == 0 {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date (H. Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub(crate) fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;

    use super::*;
    use crate::model::Column;

    pub enum X {
        S(&'static str),
        N(f64),
        D(f64),
        E,
        B,
    }

    /// A minimal workbook written by hand: sheets of (name, rows).
    pub fn workbook(sheets: &[(&str, Vec<Vec<X>>)], hidden_first: bool) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        let mut put = |name: &str, body: &str| {
            zip.start_file(name, o).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        };
        let ws_ct = "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
        let overrides: String = (1..=sheets.len()).map(|i| format!(r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="{ws_ct}"/>"#)).collect();
        put(
            "[Content_Types].xml",
            &format!(
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>{overrides}</Types>"#
            ),
        );
        put("_rels/.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#);
        let tags: String = sheets
            .iter()
            .enumerate()
            .map(|(i, (n, _))| {
                let state = if hidden_first && i == 0 { r#" state="hidden""# } else { "" };
                format!(r#"<sheet name="{n}" sheetId="{}"{state} r:id="rId{}"/>"#, i + 1, i + 1)
            })
            .collect();
        put(
            "xl/workbook.xml",
            &format!(r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{tags}</sheets></workbook>"#),
        );
        let rels: String = (1..=sheets.len())
            .map(|i| format!(r#"<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>"#))
            .collect();
        put("xl/_rels/workbook.xml.rels", &format!(r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#));
        put(
            "xl/styles.xml",
            r#"<?xml version="1.0"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/></font></fonts><fills count="1"><fill><patternFill patternType="none"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14" applyNumberFormat="1"/></cellXfs></styleSheet>"#,
        );
        for (i, (_, rows)) in sheets.iter().enumerate() {
            let mut body = String::new();
            for (r, row) in rows.iter().enumerate() {
                body.push_str(&format!(r#"<row r="{}">"#, r + 1));
                for (c, cell) in row.iter().enumerate() {
                    let at = format!("{}{}", (b'A' + c as u8) as char, r + 1);
                    match cell {
                        X::S(s) => body.push_str(&format!(r#"<c r="{at}" t="inlineStr"><is><t>{s}</t></is></c>"#)),
                        X::N(n) => body.push_str(&format!(r#"<c r="{at}"><v>{n}</v></c>"#)),
                        X::D(n) => body.push_str(&format!(r#"<c r="{at}" s="1"><v>{n}</v></c>"#)),
                        X::E => body.push_str(&format!(r#"<c r="{at}" t="e"><v>#DIV/0!</v></c>"#)),
                        X::B => {}
                    }
                }
                body.push_str("</row>");
            }
            put(&format!("xl/worksheets/sheet{}.xml", i + 1), &format!(r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{body}</sheetData></worksheet>"#));
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn cells_keep_their_types() {
        let wb = workbook(
            &[(
                "Responses",
                vec![
                    vec![X::B, X::B],
                    vec![X::S("id"), X::S("score"), X::S("when"), X::S("group"), X::S("note")],
                    vec![X::N(1.0), X::N(4.0), X::D(46_023.0), X::N(1.0), X::S("ok &amp; fine")],
                    vec![X::B],
                    vec![X::N(2.0), X::E, X::D(46_024.5), X::S("two"), X::B],
                ],
            )],
            false,
        );
        let (ds, report) = read_xlsx(&wb, "Survey", None).unwrap();
        assert_eq!(ds.n_rows(), 2);
        assert_eq!(report.skipped_blank_rows, 1);
        assert_eq!(ds.columns[1], Column::Numeric(vec![Some(4.0), None]));
        assert_eq!(ds.columns[2], Column::Text(vec![Some("2026-01-01".into()), Some("2026-01-02T12:00:00".into())]));
        assert_eq!(ds.columns[3], Column::Text(vec![Some("1".into()), Some("two".into())]));
        assert_eq!(ds.columns[4], Column::Text(vec![Some("ok & fine".into()), None]));
        assert!(report.notes.iter().any(|n| n.contains("formula error")));
        assert!(report.notes.iter().any(|n| n.contains("date cell")));
    }

    #[test]
    fn picks_the_first_visible_sheet_and_names_the_others() {
        let wb = workbook(
            &[("Hidden", vec![vec![X::S("a")], vec![X::N(1.0)]]), ("Data", vec![vec![X::S("b")], vec![X::N(2.0)]]), ("More", vec![vec![X::S("c")], vec![X::N(3.0)]])],
            true,
        );
        let (ds, report) = read_xlsx(&wb, "W", None).unwrap();
        assert_eq!(ds.variables[0].name, "b");
        assert_eq!(report.other_sheets, ["Hidden", "More"]);
        assert_eq!(sheet_names(&wb).unwrap(), [("Hidden".to_string(), false), ("Data".to_string(), true), ("More".to_string(), true)]);
        assert_eq!(read_xlsx(&wb, "W", Some("More")).unwrap().0.variables[0].name, "c");
        assert!(read_xlsx(&wb, "W", Some("Nope")).unwrap_err().to_string().contains("no sheet named"));
    }

    #[test]
    fn damaged_workbooks_are_refused() {
        assert!(read_xlsx(b"PK\x03\x04 not really", "W", None).is_err());
        assert!(read_xlsx(b"", "W", None).is_err());
        let good = workbook(&[("S", vec![vec![X::S("a")], vec![X::N(1.0)]])], false);
        assert!(read_xlsx(&good[..good.len() / 2], "W", None).is_err());
        let empty = workbook(&[("S", vec![])], false);
        assert!(read_xlsx(&empty, "W", None).unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn dates_convert() {
        assert_eq!(serial_to_iso(1.0), "1899-12-31");
        assert_eq!(serial_to_iso(45_658.25), "2025-01-01T06:00:00");
    }
}
