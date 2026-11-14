//! The data tools against base R (reference/make_reshape_reference.R writes the inputs and the expected datasets):
//! append with a source variable, merges (left, inner, all, one-to-many), aggregate (eleven summaries, and added to
//! cases), and restructuring wide to long (with and without dropping missing rows) and back to wide.

use std::path::PathBuf;

use prism_engine::import::read_file;
use prism_engine::model::Column;
use prism_engine::reshape::{self, AggregateSpec, Fun, Join, LongSpec, Stack, Summary, WideSpec};
use prism_engine::Dataset;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn data(file: &str) -> Dataset {
    read_file(&dir().join(file)).unwrap().0
}

/// Every variable name, row count and cell of `got` equals R's expected file.
fn same(got: &Dataset, file: &str) {
    let want = data(file);
    let gn: Vec<&str> = got.variables.iter().map(|v| v.name.as_str()).collect();
    let wn: Vec<&str> = want.variables.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(gn, wn, "{file}: variables");
    assert_eq!(got.n_rows(), want.n_rows(), "{file}: rows");
    for (j, name) in wn.iter().enumerate() {
        for r in 0..want.n_rows() {
            let ok = match (&got.columns[j], &want.columns[j]) {
                (Column::Numeric(a), Column::Numeric(b)) => match (a[r], b[r]) {
                    (Some(x), Some(y)) => (x - y).abs() <= 1e-9 * y.abs().max(1.0),
                    (None, None) => true,
                    _ => false,
                },
                (Column::Text(a), Column::Text(b)) => a[r] == b[r],
                // An all-missing column reads as numeric in R's file.
                (Column::Text(a), Column::Numeric(b)) => a[r].is_none() && b[r].is_none(),
                (Column::Numeric(a), Column::Text(b)) => a[r].is_none() && b[r].is_none(),
            };
            assert!(ok, "{file}: {name}, row {}: got {:?}, R gives {:?}", r + 1, cellf(&got.columns[j], r), cellf(&want.columns[j], r));
        }
    }
}

fn cellf(c: &Column, r: usize) -> String {
    match c {
        Column::Numeric(v) => format!("{:?}", v[r]),
        Column::Text(v) => format!("{:?}", v[r]),
    }
}

#[test]
fn append_cases() {
    let mut d = data("people.csv");
    let r = reshape::append(&mut d, &data("extra.csv"), Some("source")).unwrap();
    assert_eq!((r.added_cases, r.rows), (4, 16));
    assert_eq!(r.new_variables, ["newvar"]);
    same(&d, "exp_append.csv");
    assert_eq!(d.variable("source").unwrap().value_labels.len(), 2);
    // A variable that is text in one file and numeric in the other is refused, and nothing changes.
    let mut d = data("people.csv");
    let mut bad = data("extra.csv");
    let i = bad.find("age").unwrap();
    bad.columns[i] = Column::Text(vec![Some("x".into()); 4]);
    assert!(reshape::append(&mut d, &bad, None).unwrap_err().to_string().contains("age"));
    assert_eq!(d.n_rows(), 12);
}

#[test]
fn merges() {
    for (join, file) in [(Join::Left, "exp_merge_left.csv"), (Join::Inner, "exp_merge_inner.csv"), (Join::All, "exp_merge_all.csv")] {
        let mut d = data("people.csv");
        let r = reshape::merge(&mut d, &data("lookup.csv"), &["group".to_string()], join).unwrap();
        assert_eq!((r.matched, r.unmatched_here, r.unmatched_other), (10, 2, 1), "{file}");
        same(&d, file);
    }
    let mut d = data("people.csv");
    let r = reshape::merge(&mut d, &data("visits.csv"), &["id".to_string()], Join::Left).unwrap();
    assert_eq!(r.unmatched_other, 1);
    same(&d, "exp_merge_many.csv");
    // Keys repeated in both files are refused.
    let mut v = data("visits.csv");
    assert!(reshape::merge(&mut v, &data("visits.csv"), &["id".to_string()], Join::Left).unwrap_err().to_string().contains("many-to-many"));
    // A clashing name gets _2.
    let mut d = data("people.csv");
    let r = reshape::merge(&mut d, &data("people.csv"), &["id".to_string()], Join::Left).unwrap();
    assert!(r.renamed.contains(&("age".to_string(), "age_2".to_string())));
}

#[test]
fn aggregate() {
    let funs = [Fun::Mean, Fun::Sum, Fun::Median, Fun::Min, Fun::Max, Fun::Sd, Fun::N, Fun::Nmiss, Fun::Cases, Fun::First, Fun::Last];
    let summaries: Vec<Summary> = funs.iter().map(|f| Summary { source: Some("score2".into()), function: *f, target: format!("s2_{}", format!("{f:?}").to_lowercase()), label: None }).collect();
    let mut d = data("people.csv");
    let r = reshape::aggregate(&mut d, &AggregateSpec { breaks: vec!["group".into()], summaries, add_to_cases: false, missing_groups: true }).unwrap();
    assert_eq!(r.groups, 5);
    same(&d, "exp_aggregate.csv");
    let mut d = data("people.csv");
    let add = vec![Summary { source: Some("score1".into()), function: Fun::Mean, target: "m1".into(), label: None }, Summary { source: None, function: Fun::Cases, target: "c".into(), label: None }];
    reshape::aggregate(&mut d, &AggregateSpec { breaks: vec!["group".into()], summaries: add, add_to_cases: true, missing_groups: true }).unwrap();
    same(&d, "exp_aggregate_add.csv");
}

#[test]
fn restructure() {
    let spec = |drop: bool| LongSpec { stacks: vec![Stack { target: "score".into(), variables: vec!["score1".into(), "score2".into(), "score3".into()] }], index: "index".into(), id: Some("id".into()), drop_missing: drop };
    let mut d = data("people.csv");
    let r = reshape::to_long(&mut d, &spec(false)).unwrap();
    assert_eq!(r.rows, 36);
    same(&d, "exp_long.csv");
    let mut back = d.clone();
    reshape::to_wide(&mut back, &WideSpec { id: vec!["id".into()], index: "index".into(), variables: vec!["score".into()] }).unwrap();
    same(&back, "exp_wide.csv");
    let mut d = data("people.csv");
    reshape::to_long(&mut d, &spec(true)).unwrap();
    same(&d, "exp_long_drop.csv");
    // A new identifier is numbered when none is given.
    let mut d = data("people.csv");
    let s = LongSpec { id: None, ..spec(false) };
    assert!(reshape::to_long(&mut d, &s).is_err(), "id is taken");
    let mut d = data("people.csv");
    let s = LongSpec { id: Some("case".into()), ..spec(false) };
    reshape::to_long(&mut d, &s).unwrap();
    assert_eq!(d.variables[0].name, "case");
    // Two cases with the same identifier and index are refused.
    let mut l = data("exp_long.csv");
    let ix = l.find("index").unwrap();
    l.columns[ix] = Column::Numeric(vec![Some(1.0); 36]);
    assert!(reshape::to_wide(&mut l, &WideSpec { id: vec!["id".into()], index: "index".into(), variables: vec![] }).is_err());
}
