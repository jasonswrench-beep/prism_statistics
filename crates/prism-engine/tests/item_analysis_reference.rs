//! Test scoring and item analysis from letter answers against R (reference/item_analysis_reference.json, made by
//! make_item_analysis_reference.R from item_letters.csv): every item and test figure from base R, and difficulty and
//! the item-in-total point-biserial of the single-answer questions also from psych's score.multiple.choice.

use std::path::PathBuf;

use prism_engine::analysis::item_analysis::{self, Adjustment, KeyEntry, Spec};
use prism_engine::import::read_file;
use prism_engine::Dataset;

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn r() -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(reference_dir().join("item_analysis_reference.json")).unwrap()).unwrap()
}

fn data() -> Dataset {
    read_file(&reference_dir().join("item_letters.csv")).unwrap().0
}

fn names(k: usize) -> Vec<String> {
    (1..=k).map(|i| format!("q{i:02}")).collect()
}

/// The key the R script scores with, typed as a teacher would.
const KEY: &str = "B, A, D/B, C, B, A, C, D, B, AC, C, D";

#[test]
fn letters_scored_against_the_key_match_r() {
    let ds = data();
    // Subscores by chapter, as the R script groups them: q10 and q12 in no chapter, chapter 3 a single item.
    let chapters = ["2", "2", "2", "10", "10", "10", "10", "1", "1", "", "3", ""].map(String::from).to_vec();
    let spec = Spec { items: names(12), key: item_analysis::parse_key(KEY), scored: false, group_fraction: 0.27, groups: chapters, group_name: "Chapter".into() };
    let b = item_analysis::run(&ds, &spec).unwrap();
    assert!(b.is_ok(), "{:?}", b.status);
    let mut failures = Vec::new();
    let mut n = 0;
    for (key, want) in r().iter().filter(|(k, _)| !k.starts_with('_') && !k.starts_with("psych.")) {
        let want = want.as_f64().unwrap();
        n += 1;
        match b.number(key) {
            Some(got) if (got - want).abs() <= 1e-9 * want.abs().max(1.0) => {}
            got => failures.push(format!("{key}: got {got:?}, R gives {want}")),
        }
    }
    assert!(failures.is_empty(), "{} of {n} figures differ:\n{}", failures.len(), failures.join("\n"));
    assert!(n > 300, "only {n} figures");
    // The sheet R leaves out as entirely blank is an empty line, which the CSV reader already skips.
    assert_eq!(b.number("n"), Some(119.0));
    // Chapters in natural order, 10 last; the one-item chapter has no reliability.
    let t = b.tables.iter().find(|t| t.id == "subscores").unwrap();
    assert_eq!(t.title, "Subscores by chapter");
    let firsts: Vec<String> = t.rows.iter().map(|r| match &r[0] { prism_engine::result::Cell::Text { text } => text.clone(), _ => String::new() }).collect();
    assert_eq!(firsts, ["1", "2", "3", "10"]);
    assert!(b.number("subscore.3.reliability").is_none());
    assert!(t.footnotes.iter().any(|f| f.contains("q10 and q12 have no chapter")), "{:?}", t.footnotes);
    // q12 was simulated to run the wrong way, and the diagnostics say so.
    let d = b.diagnostics.iter().find(|d| d.id == "discrimination").unwrap();
    assert!(d.message.contains("q12"), "{}", d.message);
}

#[test]
fn single_answer_questions_match_psych() {
    let ds = data();
    let single: Vec<usize> = (1..=12).filter(|i| *i != 3 && *i != 10).collect();
    let key: Vec<KeyEntry> = item_analysis::parse_key(KEY).into_iter().enumerate().filter(|(i, _)| single.contains(&(i + 1))).map(|(_, k)| k).collect();
    let items: Vec<String> = single.iter().map(|i| format!("q{i:02}")).collect();
    let b = item_analysis::run(&ds, &Spec { items, key, scored: false, group_fraction: 0.27, groups: vec![], group_name: String::new() }).unwrap();
    let r = r();
    // psych scores every row; the sheet with all twelve blank is also blank on these ten, so both leave it out.
    assert_eq!(b.number("n"), r["psych.n"].as_f64());
    for (pos, i) in single.iter().enumerate() {
        for (ours, theirs) in [("p", "p"), ("rpb", "r")] {
            let got = b.number(&format!("item.{}.{ours}", pos + 1)).unwrap();
            let want = r[&format!("psych.item.{i}.{theirs}")].as_f64().unwrap();
            assert!((got - want).abs() < 1e-9, "q{i:02} {ours}: got {got}, psych gives {want}");
        }
    }
}

#[test]
fn keys_adjustments_and_answer_forms() {
    assert_eq!(item_analysis::normalize(" c, a "), "AC");
    assert_eq!(item_analysis::normalize("b"), "B");
    assert_eq!(item_analysis::normalize("10"), "10");
    let k = item_analysis::parse_key("b; A/c  ac, -, *");
    assert_eq!(k.len(), 5);
    assert_eq!((k[3].adjustment, k[4].adjustment), (Adjustment::Dropped, Adjustment::CreditAll));
    assert_eq!(k[1].correct, vec!["A", "C"]);
    assert_eq!(k[2].correct, vec!["AC"]);

    let ds = data();
    let mut key = item_analysis::parse_key(KEY);
    key[0].adjustment = Adjustment::Dropped;
    key[1].adjustment = Adjustment::CreditAll;
    let b = item_analysis::run(&ds, &Spec { items: names(12), key, scored: false, group_fraction: 0.27, groups: vec![], group_name: String::new() }).unwrap();
    assert_eq!(b.number("k"), Some(11.0), "a dropped item is left out");
    assert_eq!(b.number("item.1.p"), Some(1.0), "q02 is credited to everyone");
    assert!(b.number("item.1.rpb").is_none(), "an item everyone got right has no correlation");
    assert!(b.method_note.contains("dropped") && b.method_note.contains("credited"));

    // A key of the wrong length is refused in words.
    let e = item_analysis::run(&ds, &Spec { items: names(12), key: item_analysis::parse_key("A, B"), scored: false, group_fraction: 0.27, groups: vec![], group_name: String::new() }).unwrap_err();
    assert!(e.to_string().contains("one key entry for each item"), "{e}");
}

#[test]
fn small_classes_are_flagged_and_not_split() {
    let mut ds = Dataset::new("t");
    for (name, col) in [("a", ["A", "B", "A", "A", "C", ""]), ("b", ["B", "B", "A", "B", "B", ""]), ("c", ["C", "C", "C", "D", "C", " "])] {
        ds.add_variable(name, prism_engine::model::Column::Text(col.iter().map(|s| Some(s.to_string())).collect())).unwrap();
    }
    let b = item_analysis::run(&ds, &Spec { items: vec!["a".into(), "b".into(), "c".into()], key: item_analysis::parse_key("A B C"), scored: false, group_fraction: 0.27, groups: vec![], group_name: String::new() }).unwrap();
    assert!(b.number("group.size").is_none(), "27% of 5 rounds to 1, too few to form groups");
    assert_eq!((b.cases.total, b.cases.analyzed), (6, 5));
    assert_eq!(b.cases.excluded[0].reason, "every item blank");
    assert!(b.diagnostics.iter().any(|d| d.id == "size" && d.message.contains("unstable")));
}
