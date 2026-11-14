//! JASP data import.
//!
//! The committed test builds a small .jasp file the way JASP 0.19 lays one out (a zip holding `internal.sqlite`), with
//! a labelled numeric category, a text category with a missing-value string, a scale variable with a title and an empty
//! cell, and a filtered row. The second test runs on the product owner's JASP data library when it is present in
//! `samples-private/` (never committed) and compares every file with the CSV copy beside it.

use std::io::Write;
use std::path::{Path, PathBuf};

use prism_engine::import::read_bytes;
use prism_engine::model::{Column, Measure};

fn build_jasp() -> Vec<u8> {
    let dir = std::env::temp_dir().join(format!("prism-jasp-fixture-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("internal.sqlite");
    let _ = std::fs::remove_file(&db);
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute_batch(
        r#"
        CREATE TABLE DataSets (id INTEGER PRIMARY KEY, dataFilePath TEXT, databaseJson TEXT, emptyValuesJson TEXT, revision INT DEFAULT 0, dataFileSynch INT, description TEXT, dataFileTimestamp INT);
        CREATE TABLE Columns (id INTEGER PRIMARY KEY, dataSet INT, name TEXT, title TEXT, description TEXT, columnType TEXT, colIdx INT, invalidated INT NULL, codeType TEXT NULL, rCode TEXT NULL, error TEXT NULL, constructorJson TEXT NULL, analysisId INT NULL, revision INT DEFAULT 0, emptyValuesJson TEXT, autoSortByValue INT);
        CREATE TABLE Labels (id INTEGER PRIMARY KEY, columnId INT, value INT, ordering INT, filterAllows INT, label TEXT, originalValueJson TEXT, description TEXT);
        INSERT INTO DataSets (id, dataFilePath, databaseJson, emptyValuesJson) VALUES (1, '', 'null', '{"custom": true, "strings": [".", "NA"]}');
        INSERT INTO Columns (id, dataSet, name, title, columnType, colIdx) VALUES
            (1, 1, 'score', 'Quiz score', 'scale', 0),
            (2, 1, 'group', 'group', 'nominal', 1),
            (3, 1, 'sex', 'sex', 'nominal', 2);
        INSERT INTO Labels (columnId, value, ordering, filterAllows, label, originalValueJson) VALUES
            (2, 1, 0, 1, 'Control', '1'),
            (2, 2, 1, 1, 'Treatment', '2'),
            (3, 0, 0, 1, 'Female', '"F"'),
            (3, 1, 1, 1, 'Male', '"M"'),
            (3, 2, 2, 1, 'NA', '"NA"');
        CREATE TABLE DataSet_1 (rowNumber INTEGER PRIMARY KEY, Filter_1 INT NOT NULL DEFAULT 1, Column_1_DBL REAL NULL, Column_1_INT INT NULL, Column_2_DBL REAL NULL, Column_2_INT INT NULL, Column_3_DBL REAL NULL, Column_3_INT INT NULL);
        INSERT INTO DataSet_1 VALUES
            (1, 1, 4.5, -1, 1.0, 1, 'nan', 0),
            (2, 1, 'nan', -1, 2.0, 2, 'nan', 1),
            (3, 0, 3.25, -1, 'nan', -1, 'nan', 2),
            (4, 1, 5.0, -1, 2.0, 2, 'nan', 1);
        "#,
    )
    .unwrap();
    drop(c);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default();
    zip.start_file("manifest.json", o).unwrap();
    zip.write_all(br#"{"jaspArchiveVersion": "5", "jaspVersion": "0.19.2"}"#).unwrap();
    zip.start_file("internal.sqlite", o).unwrap();
    zip.write_all(&std::fs::read(&db).unwrap()).unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    std::fs::remove_dir_all(&dir).unwrap();
    bytes
}

#[test]
fn reads_values_labels_and_missing_cells() {
    let (ds, report) = read_bytes("study.jasp", "study", &build_jasp(), None).unwrap();
    assert_eq!(ds.n_rows(), 4);
    let score = &ds.columns[ds.find("score").unwrap()];
    assert_eq!(score, &Column::Numeric(vec![Some(4.5), None, Some(3.25), Some(5.0)]));
    assert_eq!(ds.variable("score").unwrap().label, "Quiz score");
    let group = ds.variable("group").unwrap();
    assert_eq!(group.label, "", "a title equal to the name is not a label");
    assert_eq!(group.value_labels.iter().map(|l| (l.value, l.label.as_str())).collect::<Vec<_>>(), [(1.0, "Control"), (2.0, "Treatment")]);
    assert_eq!(group.measure, Measure::Nominal);
    assert_eq!(ds.columns[ds.find("group").unwrap()], Column::Numeric(vec![Some(1.0), Some(2.0), None, Some(2.0)]));
    // Text categories show their labels; "NA" is on the file's missing-value list.
    assert_eq!(ds.columns[ds.find("sex").unwrap()], Column::Text(vec![Some("Female".into()), Some("Male".into()), None, Some("Male".into())]));
    assert!(report.notes.iter().any(|n| n.contains("filter that hid 1 row")), "{:?}", report.notes);
}

#[test]
fn old_and_damaged_files_are_refused_plainly() {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"{}").unwrap();
    let old = zip.finish().unwrap().into_inner();
    assert!(read_bytes("old.jasp", "old", &old, None).unwrap_err().to_string().contains("older than 0.17"));
    assert!(read_bytes("x.jasp", "x", b"nope", None).unwrap_err().to_string().contains("isn't a .jasp file"));
}

/// Every .jasp file in the local data library imports, and those whose CSV copy has the same rows and columns match it.
/// Skipped when the library isn't on this computer.
#[test]
fn local_data_library_matches_its_csv_copies() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples-private");
    if !root.exists() {
        return;
    }
    let mut files = Vec::new();
    collect(&root, &mut files);
    let (mut imported, mut compared, mut matched) = (0, 0, 0);
    let mut mismatches = Vec::new();
    for path in &files {
        let (ds, _) = read_bytes("f.jasp", "f", &std::fs::read(path).unwrap(), None).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        imported += 1;
        let csv = path.with_extension("csv");
        let Ok((want, _)) = prism_engine::import::read_file(&csv) else { continue };
        if want.n_rows() != ds.n_rows() || want.variables.iter().any(|v| ds.find(&v.name).is_none()) {
            continue;
        }
        compared += 1;
        let same = want.variables.iter().zip(&want.columns).all(|(v, wc)| same_column(wc, &ds.columns[ds.find(&v.name).unwrap()]));
        if same {
            matched += 1;
        } else {
            mismatches.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
    }
    eprintln!("JASP library: {imported} imported, {compared} comparable with their CSV copy, {matched} matched; differ: {mismatches:?}");
    assert_eq!(imported, files.len());
    assert!(matched * 10 >= compared * 9, "too many differ from their CSV copies: {mismatches:?}");
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "jasp") {
            out.push(p);
        }
    }
}

/// CSV copies often round to six significant digits, so numbers are compared to that precision.
fn same_column(csv: &Column, jasp: &Column) -> bool {
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-5 * a.abs().max(b.abs()).max(1.0);
    match (csv, jasp) {
        (Column::Numeric(a), Column::Numeric(b)) => a.iter().zip(b).all(|(x, y)| match (x, y) {
            (Some(x), Some(y)) => close(*x, *y),
            (None, None) => true,
            _ => false,
        }),
        (Column::Text(a), Column::Text(b)) => a.iter().zip(b).all(|(x, y)| x.as_deref().map(str::trim) == y.as_deref().map(str::trim) || x.as_deref() == Some("NA") && y.is_none()),
        // A column of codes may be numeric in one and text in the other; compare as text.
        (Column::Numeric(a), Column::Text(b)) | (Column::Text(b), Column::Numeric(a)) => a.iter().zip(b).all(|(x, y)| x.map(prism_engine::model::format_code) == y.clone().filter(|t| t != "NA")),
    }
}
