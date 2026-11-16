//! The shared item-analysis test set (C3, openstage-core `testsets/item-analysis`, set 1.0.0, copied whole into
//! reference/c3-item-analysis). Every figure in its reference.json was computed in base R from the case files alone
//! (make_reference.R there). Each case's .pstat file, written to the `OpenAssess item results 1` profile, is opened as
//! a project and run through Reliability (the listwise figures) and Item Analysis on the scored points (the figures
//! OpenAssess also reports, here unrounded). Tolerances are the set's: 1e-9, and 1e-8 for Feldt's limits.

use std::path::PathBuf;

use prism_engine::analysis::{item_analysis, reliability};
use prism_engine::project;
use prism_engine::{Dataset, ResultBundle};
use serde_json::Value;

fn set_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference").join("c3-item-analysis")
}

fn reference() -> Value {
    serde_json::from_str(&std::fs::read_to_string(set_dir().join("reference.json")).unwrap()).unwrap()
}

fn open(case: &str) -> project::Project {
    project::from_bytes(&std::fs::read(set_dir().join("cases").join(case).join(format!("{case}.pstat"))).unwrap()).unwrap()
}

fn questions(ds: &Dataset) -> Vec<String> {
    ds.variables.iter().map(|v| v.name.clone()).filter(|n| n.starts_with('q')).collect()
}

struct Checker {
    failures: Vec<String>,
    figures: usize,
}

impl Checker {
    /// One figure: a number within `tol`, or, when R has null (undefined), no figure at all.
    fn check(&mut self, what: &str, got: Option<f64>, want: &Value, tol: f64) {
        self.figures += 1;
        match (got.filter(|g| g.is_finite()), want.as_f64()) {
            (None, None) => {}
            (Some(g), Some(w)) if (g - w).abs() <= tol => {}
            (g, w) => self.failures.push(format!("{what}: got {g:?}, R gives {w:?}")),
        }
    }
}

fn number(b: &ResultBundle, key: &str) -> Option<f64> {
    b.number(key)
}

#[test]
fn every_case_matches_the_shared_test_set() {
    let r = reference();
    assert_eq!(r["_set_version"], "1.0.0", "the copy is at set 1.0.0");
    let cases = r["cases"].as_object().unwrap();
    assert_eq!(cases.len(), 7);
    let mut c = Checker { failures: Vec::new(), figures: 0 };
    for (case, want) in cases {
        let p = open(case);
        assert_eq!(p.profile.as_deref(), Some(project::PROFILE_ITEM_RESULTS), "{case} names the profile");
        let ds = &p.dataset;
        let qs = questions(ds);

        // Reliability, listwise: what PrismStatistics's own Reliability reports.
        let rl = &want["reliability_listwise"];
        let b = reliability::run(ds, &reliability::Spec { items: qs.clone(), reverse: vec![], scale_min: None, scale_max: None, level: 0.95 }).unwrap();
        assert_eq!(b.number("n"), rl["n"].as_f64(), "{case}: listwise n");
        c.check(&format!("{case} alpha"), number(&b, "alpha"), &rl["alpha"], 1e-9);
        c.check(&format!("{case} Feldt lower"), number(&b, "alpha.ci.lower"), &rl["feldt_95_lower"], 1e-8);
        c.check(&format!("{case} Feldt upper"), number(&b, "alpha.ci.upper"), &rl["feldt_95_upper"], 1e-8);
        c.check(&format!("{case} standardized alpha"), number(&b, "alpha.std"), &rl["standardized_alpha"], 1e-9);
        c.check(&format!("{case} mean inter-item r"), number(&b, "r.mean"), &rl["mean_inter_item_r"], 1e-9);
        for (i, q) in qs.iter().enumerate() {
            let w = &rl["item"][q];
            let key = format!("item.{}", i + 1);
            c.check(&format!("{case} {q} mean"), number(&b, &format!("{key}.mean")), &w["mean"], 1e-9);
            c.check(&format!("{case} {q} sd"), number(&b, &format!("{key}.sd")), &w["sd"], 1e-9);
            c.check(&format!("{case} {q} corrected item-total r"), number(&b, &format!("{key}.r_it")), &w["corrected_item_total_r"], 1e-9);
            c.check(&format!("{case} {q} alpha if deleted"), number(&b, &format!("{key}.alpha_deleted")), &w["alpha_if_deleted"], 1e-9);
        }

        // Item analysis on the scored points, with each question's points from case.json.
        let info: Value = serde_json::from_str(&std::fs::read_to_string(set_dir().join("cases").join(case).join("case.json")).unwrap()).unwrap();
        let key: Vec<item_analysis::KeyEntry> = info["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|it| item_analysis::KeyEntry { correct: vec![], points: it["possible"].as_f64().unwrap(), adjustment: item_analysis::Adjustment::None })
            .collect();
        let b = item_analysis::run(ds, &item_analysis::Spec { items: qs.clone(), key, scored: true, group_fraction: 0.27 }).unwrap();
        assert!(b.is_ok(), "{case}: {:?}", b.status);
        // Listwise, as Reliability: the same cases.
        assert_eq!(b.number("n"), rl["n"].as_f64(), "{case}: item analysis uses the listwise cases");
        c.check(&format!("{case} item-analysis alpha"), number(&b, "reliability"), &rl["alpha"], 1e-9);
        for (i, q) in qs.iter().enumerate() {
            let k = format!("item.{}", i + 1);
            c.check(&format!("{case} {q} item-analysis corrected r"), number(&b, &format!("{k}.rpb_corrected")), &rl["item"][q]["corrected_item_total_r"], 1e-9);
        }
        // Where every question counts on every sheet, the per-sheet figures are the same sheets, so OpenAssess's
        // figures apply too (adjusted-30 is the case where they differ on purpose).
        if want["reliability_every_sheet"]["left_out"].as_array().unwrap().is_empty() {
            let every = &want["reliability_every_sheet"];
            c.check(&format!("{case} KR-20 or alpha"), number(&b, "reliability"), &every["value"], 1e-9);
            assert_eq!(b.text("reliability.kind"), Some(if every["kind"] == "KR-20" { "KR-20" } else { "Cronbach's alpha" }), "{case}: reliability kind");
            c.check(&format!("{case} mean"), number(&b, "mean"), &want["test"]["mean"], 1e-9);
            c.check(&format!("{case} SD (n - 1)"), number(&b, "sd"), &want["test"]["sd_sample"], 1e-9);
            c.check(&format!("{case} SD (n)"), number(&b, "sd.n"), &want["test"]["sd_population"], 1e-9);
            let g = &want["groups_27"];
            if g["split"].as_bool().unwrap() {
                assert_eq!(b.number("group.size"), g["size"].as_f64(), "{case}: group size");
            } else {
                assert_eq!(b.number("group.size"), None, "{case}: no groups");
            }
            for (i, q) in qs.iter().enumerate() {
                let w = &want["item"][q];
                let k = format!("item.{}", i + 1);
                c.check(&format!("{case} {q} difficulty"), number(&b, &format!("{k}.p")), &w["difficulty"], 1e-9);
                c.check(&format!("{case} {q} point-biserial"), number(&b, &format!("{k}.rpb")), &w["point_biserial_item_included"], 1e-9);
                c.check(&format!("{case} {q} upper - lower"), number(&b, &format!("{k}.D")), &w["upper_lower_27"], 1e-9);
            }
        }
    }
    assert!(c.failures.is_empty(), "{} of {} figures differ from R:\n{}", c.failures.len(), c.figures, c.failures.join("\n"));
    assert!(c.figures > 400, "only {} figures checked", c.figures);
}

#[test]
fn the_two_correlations_are_labelled_as_the_set_asks() {
    let p = open("small-ties");
    let qs = questions(&p.dataset);
    let b = reliability::run(&p.dataset, &reliability::Spec { items: qs.clone(), reverse: vec![], scale_min: None, scale_max: None, level: 0.95 }).unwrap();
    assert!(b.tables[1].columns.iter().any(|c| c == "Corrected item-total r (total leaves the item out)"));
    let b = item_analysis::run(&p.dataset, &item_analysis::Spec { items: qs, key: vec![], scored: true, group_fraction: 0.27 }).unwrap();
    let cols = &b.tables[1].columns;
    assert!(cols.iter().any(|c| c == "Point-biserial (total includes the question)"));
    assert!(cols.iter().any(|c| c == "Corrected item-total r (total leaves the item out)"));
    // q06 runs the wrong way: only the corrected figure shows it (the set's README).
    assert!(b.number("item.6.rpb").unwrap() > 0.0 && b.number("item.6.rpb_corrected").unwrap() < 0.0);
}
