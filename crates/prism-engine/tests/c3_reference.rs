//! The shared item-analysis test set (C3, openstage-core `testsets/item-analysis`, set 2.0.0, copied whole into
//! reference/c3-item-analysis). Every figure in its reference.json was computed in base R from the case files alone
//! (make_reference.R there). Each case's .pstat file (profile `OpenAssess item results 1`, or 2 for `letters-60` and
//! `credited-40`) is opened as a project and run through Reliability (the listwise figures), Item Analysis on the
//! scored points (the figures OpenAssess also reports, and `item_analysis_listwise`), and for the profile-2 cases Item
//! Analysis on the letters with the key from the file's `assessment.json` (`answers`), where a question credited to
//! everyone is never counted blank (set 2.0.0, PSTAT-1 revision 1.3). Tolerances are the set's: 1e-9, and 1e-8 for
//! Feldt's limits.

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
    assert_eq!(r["_set_version"], "2.0.0", "the copy is at set 2.0.0");
    let cases = r["cases"].as_object().unwrap();
    assert_eq!(cases.len(), 10);
    let mut c = Checker { failures: Vec::new(), figures: 0 };
    for (case, want) in cases {
        let p = open(case);
        assert!(matches!(p.profile.as_deref(), Some(project::PROFILE_ITEM_RESULTS) | Some(project::PROFILE_ITEM_RESULTS_2)), "{case} names a profile");
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
        let key: Vec<item_analysis::KeyEntry> = qs
            .iter()
            .map(|q| info["items"].as_array().unwrap().iter().find(|it| it["id"] == q.as_str()).unwrap_or_else(|| panic!("{case}: {q} in case.json")))
            .map(|it| item_analysis::KeyEntry { correct: vec![], points: it["possible"].as_f64().unwrap(), adjustment: item_analysis::Adjustment::None })
            .collect();
        let b = item_analysis::run(ds, &item_analysis::Spec { items: qs.clone(), key, scored: true, group_fraction: 0.27, groups: vec![], group_name: String::new() }).unwrap();
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
                c.check(&format!("{case} {q} upper 27"), number(&b, &format!("{k}.upper")), &w["upper_27"], 1e-9);
                c.check(&format!("{case} {q} lower 27"), number(&b, &format!("{k}.lower")), &w["lower_27"], 1e-9);
                c.check(&format!("{case} {q} difficulty"), number(&b, &format!("{k}.p")), &w["difficulty"], 1e-9);
                c.check(&format!("{case} {q} point-biserial"), number(&b, &format!("{k}.rpb")), &w["point_biserial_item_included"], 1e-9);
                c.check(&format!("{case} {q} upper - lower"), number(&b, &format!("{k}.D")), &w["upper_lower_27"], 1e-9);
            }
        }
    }
    assert!(c.failures.is_empty(), "{} of {} figures differ from R:\n{}", c.failures.len(), c.figures, c.failures.join("\n"));
    assert!(c.figures > 400, "only {} figures checked", c.figures);
}

/// `item_analysis_listwise` for one result, against the set's figures for it.
fn listwise(c: &mut Checker, case: &str, b: &ResultBundle, want: &Value, qs: &[String]) {
    let w = &want["item_analysis_listwise"];
    for (ours, theirs) in [("n", "n"), ("k", "k"), ("possible", "possible"), ("mean", "mean"), ("sd", "sd"), ("sd.n", "sd_n"), ("mean.p", "mean_p"), ("reliability", "reliability"), ("kr21", "kr21"), ("sem", "sem"), ("group.size", "group_size")] {
        let theirs = if theirs == "group_size" && w["split"] == false { &Value::Null } else { &w[theirs] };
        c.check(&format!("{case} listwise {ours}"), number(b, ours), theirs, 1e-9);
    }
    assert_eq!(b.text("reliability.kind") == Some("KR-20"), w["right_or_wrong"].as_bool().unwrap(), "{case}: KR-20 or alpha");
    for (i, q) in qs.iter().enumerate() {
        let wi = &w["item"][q];
        let k = format!("item.{}", i + 1);
        for (ours, theirs) in [("p", "p"), ("mean", "mean"), ("rpb", "point_biserial_item_included"), ("rpb_corrected", "corrected_item_total_r"), ("alpha_deleted", "alpha_if_deleted"), ("upper", "upper"), ("lower", "lower"), ("D", "d")] {
            c.check(&format!("{case} listwise {q} {theirs}"), number(b, &format!("{k}.{ours}")), &wi[theirs], 1e-9);
        }
    }
}

#[test]
fn item_analysis_as_a_scale_analysis_matches_the_set() {
    let r = reference();
    let mut c = Checker { failures: Vec::new(), figures: 0 };
    for (case, want) in r["cases"].as_object().unwrap() {
        let p = open(case);
        let qs = questions(&p.dataset);
        let info: Value = serde_json::from_str(&std::fs::read_to_string(set_dir().join("cases").join(case).join("case.json")).unwrap()).unwrap();
        let key: Vec<item_analysis::KeyEntry> = qs
            .iter()
            .map(|q| info["items"].as_array().unwrap().iter().find(|it| it["id"] == q.as_str()).unwrap())
            .map(|it| item_analysis::KeyEntry { correct: vec![], points: it["possible"].as_f64().unwrap(), adjustment: item_analysis::Adjustment::None })
            .collect();
        let b = item_analysis::run(&p.dataset, &item_analysis::Spec { items: qs.clone(), key, scored: true, group_fraction: 0.27, groups: vec![], group_name: String::new() }).unwrap();
        listwise(&mut c, case, &b, want, &qs);
    }
    assert!(c.failures.is_empty(), "{} of {} figures differ from R:\n{}", c.failures.len(), c.figures, c.failures.join("\n"));
    assert!(c.figures > 500, "only {} figures checked", c.figures);
}

#[test]
fn letters_scored_with_the_files_key_match_the_set() {
    let r = reference();
    let mut c = Checker { failures: Vec::new(), figures: 0 };
    let mut cases = Vec::new();
    for (case, want) in r["cases"].as_object().unwrap().iter().filter(|(_, w)| w.get("answers").is_some()) {
        cases.push(case.clone());
        let p = open(case);
        assert_eq!(p.profile.as_deref(), Some(project::PROFILE_ITEM_RESULTS_2), "{case}");
        let part = p.part_json().expect("the file's assessment.json is kept");
        let (keyed, points_only) = item_analysis::key_from_assessment(&p.dataset, &part);
        assert!(points_only.is_empty(), "every question in {case} has a plain key: {points_only:?}");
        let items: Vec<String> = keyed.iter().map(|(v, _)| v.clone()).collect();
        let qs: Vec<String> = items.iter().map(|r| format!("q{}", &r[1..])).collect();
        if case == "letters-60" {
            assert_eq!(items.len(), 11, "q12 is dropped on every version, so the file leaves it out");
        }
        let key = keyed.into_iter().map(|(_, k)| k).collect();
        let b = item_analysis::run(&p.dataset, &item_analysis::Spec { items, key, scored: false, group_fraction: 0.27, groups: vec![], group_name: String::new() }).unwrap();
        assert!(b.is_ok(), "{case}: {:?}", b.status);
        // Scoring the letters gives the file's points, so every scale figure is the same.
        listwise(&mut c, &format!("{case} (letters)"), &b, want, &qs);
        for (i, q) in qs.iter().enumerate() {
            let w = &want["answers"][q];
            let k = format!("item.{}", i + 1);
            c.check(&format!("{case} {q} blank"), number(&b, &format!("{k}.blank")), &w["blank"], 0.0);
            for (answer, row) in w["by_answer"].as_object().unwrap() {
                for g in ["n", "upper", "middle", "lower"] {
                    let got = number(&b, &format!("{k}.opt.{answer}.{g}"));
                    // A row with no sheets may be left out by a tool.
                    let got = if got.is_none() && row["n"] == 0 { Some(0.0) } else { got };
                    c.check(&format!("{case} {q} answer {answer} {g}"), got, &row[g], 0.0);
                }
            }
        }
    }
    assert_eq!(cases, ["credited-40", "letters-60"]);
    assert!(c.failures.is_empty(), "{} of {} figures differ from R:\n{}", c.failures.len(), c.figures, c.failures.join("\n"));
    assert!(c.figures > 600, "only {} figures checked", c.figures);
}

/// Questions' details in `assessment.json` (letters-60 leaves them empty, so this gives some) are offered as subscores.
#[test]
fn question_details_are_offered_as_subscores() {
    let mut p = open("letters-60");
    let mut part = p.part_json().unwrap();
    let bloom = ["remember", "remember", "apply", "apply", "understand", "understand", "remember", "analyze", "analyze", "apply", "understand", "apply"];
    for (i, q) in part["questions"].as_array_mut().unwrap().iter_mut().enumerate() {
        q["details"]["bloom"] = serde_json::json!(bloom[i % bloom.len()]);
        q["details"]["chapter"] = serde_json::json!("4");
    }
    p.part = Some(("assessment.json".into(), serde_json::to_vec(&part).unwrap()));
    let (_, suggestions) = project::profile_suggestions(&p).unwrap();
    let labels: Vec<&str> = suggestions.iter().map(|s| s.label.as_str()).collect();
    let by_bloom = suggestions.iter().find(|s| s.label.ends_with("by bloom level")).unwrap_or_else(|| panic!("{labels:?}"));
    assert!(!labels.iter().any(|l| l.ends_with("by chapter")), "one chapter splits nothing: {labels:?}");
    let spec: item_analysis::Spec = serde_json::from_value(by_bloom.args.clone()).unwrap();
    assert_eq!(spec.group_name, "Bloom level");
    let b = item_analysis::run(&p.dataset, &spec).unwrap();
    assert!(b.is_ok(), "{:?}", b.status);
    let t = b.tables.iter().find(|t| t.id == "subscores").unwrap();
    let names: Vec<String> = t.rows.iter().map(|r| match &r[0] { prism_engine::result::Cell::Text { text } => text.clone(), _ => String::new() }).collect();
    assert_eq!(names, ["Remember", "Understand", "Apply", "Analyze"], "in Bloom's order, with a capital");
    // The subscores add up to the whole test.
    let total: f64 = (1..=4).map(|j| b.number(&format!("subscore.{j}.mean")).unwrap()).sum();
    assert!((total - b.number("mean").unwrap()).abs() < 1e-9);
}

/// PSTAT-1 1.3's example of a short answer in a box (question 3, type `other`, no `r` variable) beside a credited
/// question: the letters are analysed without the box, which is analysed from its points, and the banner says why.
#[test]
fn a_box_scored_by_a_person_is_analysed_from_its_points() {
    let path = set_dir().parent().unwrap().join("pstat-examples").join("item-results-2-boxes.pstat");
    let p = project::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(p.profile.as_deref(), Some(project::PROFILE_ITEM_RESULTS_2));
    let (keyed, points_only) = item_analysis::key_from_assessment(&p.dataset, &p.part_json().unwrap());
    assert_eq!(keyed.iter().map(|(v, _)| v.as_str()).collect::<Vec<_>>(), ["r01", "r02"]);
    assert_eq!(points_only, [3]);
    let (about, suggestions) = project::profile_suggestions(&p).unwrap();
    assert!(about.contains("question 3 is analysed from its points only: it is scored by a person"), "{about}");
    assert_eq!(suggestions[0].args["items"], serde_json::json!(["r01", "r02"]));
    assert!(suggestions.iter().any(|s| s.args["scored"] == true && s.args["items"] == serde_json::json!(["q01", "q02", "q03"])));
}

#[test]
fn a_profile_part_survives_a_save() {
    let src = std::fs::read(set_dir().join("cases/letters-60/letters-60.pstat")).unwrap();
    let p = project::from_bytes(&src).unwrap();
    let (name, bytes) = p.part.clone().expect("the part is read");
    assert_eq!(name, "assessment.json");
    let again = project::from_bytes(&project::to_bytes(&p).unwrap()).unwrap();
    assert_eq!(again.part, Some((name, bytes)), "written back byte for byte (rule Z4)");
    // A project that no longer names the profile drops the part.
    let mut plain = p.clone();
    plain.profile = None;
    assert!(project::from_bytes(&project::to_bytes(&plain).unwrap()).unwrap().part.is_none());
}

#[test]
fn the_two_correlations_are_labelled_as_the_set_asks() {
    let p = open("small-ties");
    let qs = questions(&p.dataset);
    let b = reliability::run(&p.dataset, &reliability::Spec { items: qs.clone(), reverse: vec![], scale_min: None, scale_max: None, level: 0.95 }).unwrap();
    assert!(b.tables[1].columns.iter().any(|c| c == "Corrected item-total r (total leaves the item out)"));
    let b = item_analysis::run(&p.dataset, &item_analysis::Spec { items: qs, key: vec![], scored: true, group_fraction: 0.27, groups: vec![], group_name: String::new() }).unwrap();
    let cols = &b.tables[1].columns;
    assert!(cols.iter().any(|c| c == "Point-biserial (total includes the question)"));
    assert!(cols.iter().any(|c| c == "Corrected item-total r (total leaves the item out)"));
    // q06 runs the wrong way: only the corrected figure shows it (the set's README).
    assert!(b.number("item.6.rpb").unwrap() > 0.0 && b.number("item.6.rpb_corrected").unwrap() < 0.0);
}
