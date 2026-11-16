//! File › Open Example Data: simulated datasets for teaching, built into the program with their codebooks (variable
//! labels, measures, value labels and missing-value codes). `tools/make_examples.py` makes them; the worked examples in
//! the help quote numbers from them, which the tests below check.

use prism_engine::model::{Measure, ValueLabel};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::Session;

const EXAMPLES: &[(&str, &str, &str)] = &[
    ("study_methods", include_str!("../data/examples/study_methods.csv"), include_str!("../data/examples/study_methods.json")),
    ("sleep_reaction", include_str!("../data/examples/sleep_reaction.csv"), include_str!("../data/examples/sleep_reaction.json")),
    ("clinic_followup", include_str!("../data/examples/clinic_followup.csv"), include_str!("../data/examples/clinic_followup.json")),
    ("training_evaluation", include_str!("../data/examples/training_evaluation.csv"), include_str!("../data/examples/training_evaluation.json")),
];

#[derive(Deserialize)]
struct Codebook {
    variables: Vec<VariableMeta>,
}

#[derive(Deserialize)]
struct VariableMeta {
    name: String,
    label: String,
    measure: Measure,
    #[serde(default)]
    value_labels: Vec<ValueLabel>,
    #[serde(default)]
    missing_values: Vec<f64>,
}

/// What File › Open Example Data lists: each codebook without its variables.
#[derive(Serialize)]
struct Listed {
    id: String,
    title: String,
    description: String,
    rows: u64,
    variables: usize,
    #[serde(rename = "try")]
    suggestions: Vec<String>,
    /// Also offered as a blank template: the same variables and codebook with no cases.
    template: bool,
}

pub(crate) fn list() -> Value {
    let items: Vec<Listed> = EXAMPLES
        .iter()
        .map(|(id, _, meta)| {
            let m: Value = serde_json::from_str(meta).expect("example codebook");
            Listed {
                id: id.to_string(),
                title: m["title"].as_str().unwrap_or(id).to_string(),
                description: m["description"].as_str().unwrap_or_default().to_string(),
                rows: m["rows"].as_u64().unwrap_or(0),
                variables: m["variables"].as_array().map_or(0, Vec::len),
                suggestions: m["try"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect(),
                template: m["template"].as_bool().unwrap_or(false),
            }
        })
        .collect();
    json!(items)
}

impl Session {
    /// Opens an example dataset as new, unsaved data, with its codebook applied. `blank` opens a template example's
    /// variables and codebook with no cases, for entering one's own data.
    pub(crate) fn open_example(&mut self, id: &str, blank: bool) -> Result<Value, String> {
        let (_, csv, meta) = EXAMPLES.iter().find(|(i, _, _)| *i == id).ok_or_else(|| format!("There is no example dataset named {id}."))?;
        let book: Codebook = serde_json::from_str(meta).map_err(|e| format!("The example's codebook could not be read: {e}"))?;
        let header = csv.lines().next().unwrap_or_default();
        let mut out = self.open_data(&format!("{id}.csv"), if blank { header.as_bytes() } else { csv.as_bytes() }, None)?;
        let ds = &mut self.project.as_mut().ok_or("The example did not open.")?.dataset;
        if let Some(t) = serde_json::from_str::<Value>(meta).ok().and_then(|m| m["title"].as_str().map(str::to_string)) {
            ds.name = t;
        }
        for m in book.variables {
            let v = ds.variable_mut(&m.name).map_err(|e| e.to_string())?;
            v.label = m.label;
            v.measure = m.measure;
            v.value_labels = m.value_labels;
            v.missing_values = m.missing_values;
        }
        out["dataset"] = self.summary()?;
        out["state"] = self.state();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    fn open(id: &str) -> Session {
        let mut s = Session::new();
        s.handle("open_example", json!({ "id": id })).unwrap();
        s
    }

    fn field(out: &serde_json::Value, k: &str) -> f64 {
        out[0]["bundle"]["fields"][k].as_f64().unwrap_or_else(|| panic!("no field {k}: {}", out[0]["bundle"]["fields"]))
    }

    #[test]
    fn every_example_opens_with_its_codebook() {
        let list = super::list();
        assert_eq!(list.as_array().unwrap().len(), 4);
        for e in list.as_array().unwrap() {
            let mut s = open(e["id"].as_str().unwrap());
            let ds = s.handle("dataset", json!({})).unwrap();
            assert_eq!(ds["rows"], e["rows"]);
            assert_eq!(ds["variables"].as_array().unwrap().len() as u64, e["variables"].as_u64().unwrap());
            assert!(ds["variables"].as_array().unwrap().iter().all(|v| !v["label"].as_str().unwrap().is_empty()), "every variable has a label");
            assert_eq!(s.state()["dirty"], true, "an example opens as unsaved data");
            assert_eq!(s.state()["name"], e["title"], "named by its title");
            assert_eq!(s.state()["undo"], json!(null));
        }
        // A template example also opens blank: its variables and codebook, no cases.
        let mut t = crate::Session::new();
        t.handle("open_example", json!({ "id": "training_evaluation", "blank": true })).unwrap();
        let ds = t.handle("dataset", json!({})).unwrap();
        assert_eq!(ds["rows"], 0);
        assert_eq!(ds["variables"].as_array().unwrap().len(), 11);
        assert!(ds["variables"].as_array().unwrap().iter().any(|v| v["name"] == "react_1" && v["value_labels"].as_array().is_some_and(|l| l.len() == 5)));
        assert_eq!(super::list().as_array().unwrap().iter().filter(|e| e["template"] == true).count(), 1);
        let mut s = open("study_methods");
        let ds = s.handle("dataset", json!({})).unwrap();
        let sat = ds["variables"].as_array().unwrap().iter().find(|v| v["name"] == "satisfaction").unwrap();
        assert_eq!(sat["missing_values"], json!([-99.0]));
        assert_eq!(sat["measure"], "ordinal");
        assert!(Session::new().handle("open_example", json!({ "id": "nosuch" })).is_err());
    }

    /// The numbers the worked examples in the help quote (help/examples/*.md). If the data are made again and these move,
    /// update the pages to match.
    #[test]
    fn the_worked_examples_quote_these_numbers() {
        let mut s = open("study_methods");
        let t = s.handle("ttest_independent", json!({ "test_variables": ["exam"], "grouping_variable": "method", "groups": [1.0, 2.0] })).unwrap();
        let r = |x: f64, d: i32| (x * 10f64.powi(d)).round() / 10f64.powi(d);
        // Comparing two groups.
        assert_eq!((r(field(&t, "g1.mean"), 2), r(field(&t, "g2.mean"), 2), r(field(&t, "g1.sd"), 2), r(field(&t, "g2.sd"), 2)), (65.67, 71.03, 13.33, 12.31));
        assert_eq!((r(field(&t, "diff"), 2), r(field(&t, "g.ci.lower"), 2), r(field(&t, "g.ci.upper"), 2)), (-5.37, -0.77, -0.06));
        assert_eq!((r(field(&t, "t"), 2), r(field(&t, "df"), 2), r(field(&t, "p"), 3)), (-2.29, 117.27, 0.024));
        assert_eq!((r(field(&t, "g"), 2), r(field(&t, "ci.lower"), 2), r(field(&t, "ci.upper"), 2)), (-0.42, -10.01, -0.73));
        // Before and after.
        let p = s.handle("ttest_paired", json!({ "pairs": [["pretest", "posttest"]] })).unwrap();
        assert_eq!((r(field(&p, "x1.mean"), 2), r(field(&p, "x2.mean"), 2), r(field(&p, "t"), 2), field(&p, "df")), (55.77, 67.55, -21.63, 119.0));
        assert!(field(&p, "p") < 0.001);
        assert_eq!((r(field(&p, "diff"), 2), r(field(&p, "d"), 2), r(field(&p, "ci.lower"), 2), r(field(&p, "ci.upper"), 2)), (-11.78, -1.97, -12.86, -10.70));
        assert_eq!((r(field(&p, "x1.sd"), 2), r(field(&p, "x2.sd"), 2)), (12.52, 13.73));
        // Relationship and prediction.
        let c = s.handle("correlation", json!({ "variables": ["hours", "exam"] })).unwrap();
        assert_eq!((r(field(&c, "pair.1.2.r"), 2), field(&c, "pair.1.2.n")), (0.36, 120.0));
        assert!(field(&c, "pair.1.2.p") < 0.001);
        assert_eq!((r(field(&c, "pair.1.2.ci.lower"), 2), r(field(&c, "pair.1.2.ci.upper"), 2)), (0.19, 0.50));
        let g = s.handle("regression", json!({ "dependent": "exam", "covariates": ["hours"], "factors": [] })).unwrap();
        assert_eq!((r(field(&g, "r2"), 3), r(field(&g, "term.1.b"), 2), r(field(&g, "b0.b"), 2), r(field(&g, "F"), 2)), (0.128, 1.51, 58.91, 17.34));
        assert_eq!((r(field(&g, "term.1.ci.lower"), 2), r(field(&g, "term.1.ci.upper"), 2)), (0.79, 2.23));
        assert!(field(&g, "p") < 0.001);
    }
}

