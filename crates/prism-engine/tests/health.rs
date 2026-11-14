//! Data health and paste preview: every finding on a dataset built to have each problem once, the fixes (preview,
//! then apply), and pastes with a non-number in a numeric variable, values past the last variable and new cases.

use prism_engine::health::{self, Fix};
use prism_engine::import::read_bytes;
use prism_engine::model::{Column, Measure, ValueLabel};
use prism_engine::Dataset;

fn messy() -> Dataset {
    // id: identifier with a repeat; code: unique whole numbers measured as scale; income: -99 and 999 codes;
    // age_text: numbers stored as text with one typo; sex: spellings differing in case and spaces; same: constant;
    // nothing: empty; rating: labeled 1-5 with a 7. The last row repeats row 3 exactly.
    let csv = "id,code,income,age_text,sex,same,nothing,rating\n\
1,101,52000,34,Male,1,,4\n\
2,102,48000,29,female,1,,5\n\
3,103,-99,41,Female,1,,3\n\
4,104,61000,3o,male ,1,,7\n\
4,105,999,55,Male,1,,2\n\
6,106,57000,47,Female,1,,1\n\
7,107,45500,38,Male,1,,4\n\
8,108,50500,62,Female,1,,2\n\
9,109,58000,33,Male,1,,5\n\
10,110,49000,44,Female,1,,3\n\
3,103,-99,41,Female,1,,3\n";
    let (mut ds, _) = read_bytes("messy.csv", "messy", csv.as_bytes(), None).unwrap();
    let r = ds.variable_mut("rating").unwrap();
    r.measure = Measure::Ordinal;
    r.value_labels = (1..=5).map(|v| ValueLabel { value: v as f64, label: format!("level {v}") }).collect();
    ds
}

fn find<'a>(h: &'a health::Health, kind: &str, var: &str) -> &'a health::Finding {
    h.findings.iter().find(|f| f.kind == kind && (var.is_empty() || f.variables.iter().any(|v| v == var))).unwrap_or_else(|| panic!("no {kind} finding for {var}: {:#?}", h.findings))
}

#[test]
fn every_finding_is_reported() {
    let ds = messy();
    let h = health::check(&ds);
    assert_eq!(h.rows, 11);
    let inc = h.variables.iter().find(|v| v.name == "income").unwrap();
    assert_eq!((inc.valid, inc.missing, inc.unique), (11, 0, 10));
    let f = find(&h, "missing_codes", "income");
    assert_eq!(f.cases, [3, 5, 11]);
    assert_eq!(f.fix, Some(Fix::MarkMissing { name: "income".into(), codes: vec![-99.0, 999.0] }));
    let f = find(&h, "numbers_as_text", "age_text");
    assert_eq!(f.cases, [4]);
    assert!(find(&h, "spellings", "sex").message.contains("\"Male\""));
    find(&h, "constant", "same");
    find(&h, "empty", "nothing");
    assert_eq!(find(&h, "unlabeled", "rating").cases, [4]);
    assert_eq!(find(&h, "duplicate_ids", "id").cases, [5, 11]);
    assert_eq!(find(&h, "identifier", "code").fix, Some(Fix::SetMeasure { name: "code".into(), measure: Measure::Nominal }));
    assert_eq!(find(&h, "duplicate_cases", "").cases, [11]);
    assert!(!h.findings.iter().any(|f| f.kind == "identifier" && f.variables == ["income"]), "incomes are not identifiers");
    // The check changes nothing.
    assert_eq!(ds, messy());
}

#[test]
fn fixes_preview_then_apply() {
    let mut ds = messy();
    let to_num = Fix::ToNumeric { name: "age_text".into() };
    let p = health::fix(&mut ds, &to_num, false).unwrap();
    assert_eq!(p.changes, [(4, "3o".to_string(), "(empty)".to_string())]);
    assert!(matches!(ds.columns[ds.find("age_text").unwrap()], Column::Text(_)), "a preview changes nothing");
    health::fix(&mut ds, &to_num, true).unwrap();
    let Column::Numeric(v) = &ds.columns[ds.find("age_text").unwrap()] else { panic!() };
    assert_eq!((v[0], v[3]), (Some(34.0), None));
    let mark = Fix::MarkMissing { name: "income".into(), codes: vec![-99.0, 999.0] };
    assert_eq!(health::fix(&mut ds, &mark, false).unwrap().count, 3);
    health::fix(&mut ds, &mark, true).unwrap();
    assert!(ds.variable("income").unwrap().is_user_missing(999.0));
    let h = health::check(&ds);
    assert!(!h.findings.iter().any(|f| f.kind == "missing_codes" || f.kind == "numbers_as_text"), "{:#?}", h.findings);
    health::fix(&mut ds, &Fix::SetMeasure { name: "code".into(), measure: Measure::Nominal }, true).unwrap();
    assert_eq!(ds.variable("code").unwrap().measure, Measure::Nominal);
}

#[test]
fn paste_preview() {
    let mut ds = messy();
    // At case 10 (row 9), income: a number, a non-number, and a value past the last variable; then two new cases.
    let text = "50000\t40\tMale\n12x\t41\tFemale\t1\t\t5\textra\n60000\t42\tMale\n";
    let plan = health::paste(&mut ds, 9, "income", text, false).unwrap();
    assert_eq!((plan.new_cases, plan.left_out, plan.problems), (1, 1, 1));
    let bad = plan.changes.iter().find(|c| c.problem.is_some()).unwrap();
    assert_eq!((bad.row, bad.name.as_str(), bad.new.as_deref()), (10, "income", Some("12x")));
    assert_eq!(ds.n_rows(), 11, "a preview changes nothing");
    health::paste(&mut ds, 9, "income", text, true).unwrap();
    assert_eq!(ds.n_rows(), 12);
    let Column::Numeric(inc) = &ds.columns[ds.find("income").unwrap()] else { panic!() };
    assert_eq!((inc[9], inc[10], inc[11]), (Some(50000.0), Some(-99.0), Some(60000.0)), "the non-number left the cell as it was");
    let Column::Text(sex) = &ds.columns[ds.find("sex").unwrap()] else { panic!() };
    assert_eq!(sex[11].as_deref(), Some("Male"));
    // An empty paste is refused.
    assert!(health::paste(&mut ds, 0, "id", "", false).is_err());
}
