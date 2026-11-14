//! Imports of .sav, .dta and .sas7bdat files against R's haven package (reference/statfiles_reference.json, made with
//! the fixtures by make_statfiles.R). Every format must give the same values; the .sav file also carries a user-missing
//! code and a missing range, which must arrive as missing-value settings rather than as empty cells.

use std::path::PathBuf;

use prism_engine::import::read_file;
use prism_engine::model::{Column, Measure, NumericCell};
use prism_engine::Dataset;
use serde_json::Value;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn reference() -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("statfiles_reference.json")).unwrap()).unwrap()
}

/// Checks every column of an imported dataset against haven's reading of the same file.
fn matches_haven(ds: &Dataset, want: &Value) {
    for name in ["id", "sex", "age", "score", "name", "visit"] {
        let i = ds.find(name).unwrap_or_else(|| panic!("no variable {name}"));
        let col = &ds.columns[i];
        let expected = want[name].as_array().unwrap();
        assert_eq!(col.len(), expected.len(), "{name}");
        for (r, e) in expected.iter().enumerate() {
            match col {
                Column::Numeric(v) => assert_eq!(v[r], e.as_f64(), "{name} row {r}"),
                // An empty string in the file is an empty cell here, as in a CSV file.
                Column::Text(v) => assert_eq!(v[r].as_deref(), e.as_str().filter(|s| !s.is_empty()), "{name} row {r}"),
            }
        }
        let label = want["_labels"][name].as_str().unwrap_or("");
        assert_eq!(ds.variables[i].label, label, "label of {name}");
    }
}

#[test]
fn sav_matches_haven_and_keeps_missing_settings() {
    let (ds, report) = read_file(&dir().join("statfile.sav")).unwrap();
    matches_haven(&ds, &reference()["sav"]);
    let sex = ds.variable("sex").unwrap();
    assert_eq!(sex.value_labels.iter().map(|l| (l.value, l.label.as_str())).collect::<Vec<_>>(), [(1.0, "Female"), (2.0, "Male"), (9.0, "No answer")]);
    assert_eq!(sex.missing_values, [9.0]);
    assert_eq!(ds.variable("age").unwrap().missing_ranges, [(90.0, 99.0)]);
    // The codes stay in the data and are treated as missing by every analysis.
    assert_eq!(ds.numeric_cells("sex").unwrap()[4], NumericCell::UserMissing(9.0));
    assert_eq!(ds.numeric_cells("age").unwrap()[2], NumericCell::UserMissing(99.0));
    assert_eq!(ds.variable("name").unwrap().measure, Measure::Nominal);
    assert!(report.notes.iter().any(|n| n.contains("Dates") && n.contains("visit")), "{:?}", report.notes);
    assert_eq!((report.rows, report.variables), (8, 6));
}

#[test]
fn dta_matches_haven() {
    let (ds, report) = read_file(&dir().join("statfile.dta")).unwrap();
    matches_haven(&ds, &reference()["dta"]);
    let sex = ds.variable("sex").unwrap();
    assert_eq!(sex.value_labels.len(), 3);
    assert!(sex.missing_values.is_empty(), "a .dta file has no user-missing codes");
    assert!(report.notes.iter().any(|n| n.contains("visit")), "{:?}", report.notes);
}

#[test]
fn sas7bdat_matches_haven() {
    let (ds, report) = read_file(&dir().join("statfile.sas7bdat")).unwrap();
    matches_haven(&ds, &reference()["sas7bdat"]);
    assert_eq!(report.sas_formats.len(), report.sas_formats.iter().filter(|(_, f)| !f.is_empty()).count());
}

/// A catalog's label sets attach to the variables whose SAS format names them. (The catalog file format itself is
/// parsed by the sas7bdat crate, which tests it against ReadStat's catalogs; these label sets stand in for one.)
#[test]
fn sas_catalog_labels_attach_by_format_name() {
    use prism_engine::statfile::{apply_sas_labels, catalog_notes};
    use sas7bdat::{LabelSet, ValueKey, ValueLabel, ValueType};
    let (mut ds, _) = read_file(&dir().join("statfile.sas7bdat")).unwrap();
    let numeric: Vec<String> = ds.variables.iter().zip(&ds.columns).filter(|(_, c)| matches!(c, prism_engine::Column::Numeric(_))).map(|(v, _)| v.name.clone()).collect();
    let text: Vec<String> = ds.variables.iter().zip(&ds.columns).filter(|(_, c)| matches!(c, prism_engine::Column::Text(_))).map(|(v, _)| v.name.clone()).collect();
    let mut sex = LabelSet::new("SEXF".into(), ValueType::Numeric);
    sex.labels = vec![ValueLabel { key: ValueKey::Integer(1), label: "Female  ".into() }, ValueLabel { key: ValueKey::Numeric(2.0), label: "Male".into() }, ValueLabel { key: ValueKey::Tagged('A'), label: "Refused".into() }];
    let mut words = LabelSet::new("$WORDS".into(), ValueType::String);
    words.labels = vec![ValueLabel { key: ValueKey::String("a".into()), label: "Apple".into() }];
    let mut formats = vec![(numeric[0].clone(), "SEXF".to_string()), (numeric[1].clone(), "BEST12".to_string()), (numeric[1].clone(), "MYFMT".to_string())];
    if let Some(t) = text.first() {
        formats.push((t.clone(), "$WORDS".to_string()));
    }
    let out = apply_sas_labels(&mut ds, &formats, &[sex, words]);
    let v = ds.variable(&numeric[0]).unwrap();
    assert_eq!(v.value_labels.iter().map(|l| (l.value, l.label.as_str())).collect::<Vec<_>>(), [(1.0, "Female"), (2.0, "Male")], "trailing blanks trimmed; .A skipped");
    assert_eq!(out.missing, ["MYFMT"], "a built-in format (BEST12) isn't reported as missing");
    assert_eq!(out.text_skipped.len(), usize::from(!text.is_empty()));
    let notes = catalog_notes(&out, "formats.sas7bcat");
    assert!(notes[0].starts_with("Value labels for"), "{notes:?}");
    assert!(prism_engine::statfile::read_sas_catalog(b"not a catalog").unwrap_err().to_string().contains(".sas7bcat"));
}

#[test]
fn damaged_files_are_refused_plainly() {
    for ext in ["sav", "dta", "sas7bdat"] {
        let err = prism_engine::import::read_bytes(&format!("x.{ext}"), "x", b"not a data file at all", None).unwrap_err().to_string();
        assert!(err.contains(&format!(".{ext} file could not be read")), "{ext}: {err}");
    }
}
