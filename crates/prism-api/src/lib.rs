//! The command layer the interface talks to.
//!
//! The desktop window (Tauri) and the local test server (`prism serve`) both pass commands here, so the interface
//! behaves the same in both. A command is a name plus JSON arguments, and the answer is JSON or an error message
//! written for the user.
//!
//! Everything that changes the project is recorded as an operation (the command and its exact arguments), so a saved
//! project says how each result was made. When the data changes after a result was run (for example a new missing-value
//! code), the result is marked out of date until it is run again (GUIDE-07, QA07).

mod ai;
mod examples;
pub use ai::write_value as ai_write;
mod google;
pub mod mcp;
mod recheck;
pub mod recovery;
mod rexpr;
pub mod rscript;
pub mod script;
mod undo;

use std::path::{Path, PathBuf};

use base64::Engine as _;
use prism_engine::analysis::{agreement, anova_factorial, anova_oneway, bootstrap, canonical, factor, glm_more, manova, missing, mixed, permutation, pls, process, sem, survey, lca, lpa, forest, boost, irt, meta, gee, panel, dpanel, predict, tree, spc, doe, survival, timeseries, rm_anova, trimmed, classic_np, correlation, crosstabs, curve, descriptives, explore, frequencies, graphs, logistic, partial_correlation, power, rank_tests, regression, related, reliability, item_analysis, kirkpatrick, ttest_independent, ttest_one_sample, ttest_paired};
use prism_engine::model::{NumericCell, ValueLabel};
use prism_engine::project::{self, Operation, Project, Report, ReportBlock, Source, StoredResult};
use prism_engine::result::Field;
use prism_engine::{apa, import, templates, Column, Dataset, Measure, ResultBundle};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The commands that run an analysis; each is recorded and can be run again.
const ANALYSES: [&str; 79] = [
    "ttest_independent",
    "frequencies",
    "descriptives",
    "ttest_one_sample",
    "ttest_paired",
    "anova_oneway",
    "crosstabs",
    "correlation",
    "mann_whitney",
    "wilcoxon",
    "kruskal",
    "regression",
    "mcnemar",
    "friedman",
    "partial_correlation",
    "anova_factorial",
    "logistic",
    "reliability",
    "item_analysis",
    "kirkpatrick",
    "canonical",
    "power",
    "chart",
    "chisq_gof",
    "binomial",
    "runs",
    "sign_test",
    "median_test",
    "ks2",
    "cochran",
    "kappa",
    "fleiss",
    "kripp",
    "icc",
    "explore",
    "curve",
    "bootstrap",
    "rm_anova",
    "manova",
    "factor",
    "mediation",
    "moderation",
    "ordinal_regression",
    "multinomial_regression",
    "count_regression",
    "yuen",
    "yuen_paired",
    "trimmed_anova",
    "mixed_model",
    "missing_patterns",
    "mi_regression",
    "permutation",
    "kaplan_meier",
    "cox_regression",
    "roc",
    "ts_acf",
    "ts_smooth",
    "arima",
    "survey_estimates",
    "survey_regression",
    "sem",
    "sem_invariance",
    "pls",
    "lca",
    "lpa",
    "forest",
    "boost",
    "irt",
    "meta",
    "gee",
    "panel",
    "pgmm",
    "penalized",
    "tree",
    "spc",
    "doe_design",
    "doe_effects",
    "rsm",
    "doe_mixture",
];

#[derive(Default)]
pub struct Session {
    pub project: Option<Project>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    /// A Google Sheets download waiting for a tab to be chosen: (link, file name, bytes).
    google: Option<(String, String, Vec<u8>)>,
    /// Where unsaved work is copied for crash recovery (the desktop app sets it; see recovery.rs).
    pub recovery: Option<recovery::Recovery>,
    /// Counts changes, so the autosave writes only when there is something new.
    edits: u64,
    autosaved: Option<u64>,
    autosaved_at: u64,
    /// Edit › Undo and Redo (undo.rs).
    undo: Vec<undo::Step>,
    redo: Vec<undo::Step>,
    /// Commands running inside another (a script's lines), which don't get undo steps of their own.
    nesting: u32,
    /// While a script runs: the existing files the user agreed to let it replace (see `may_write`).
    pub(crate) script_replace: Option<Vec<String>>,
    /// Whether each result is out of date, kept until the data it reads change (recheck.rs).
    recheck: std::sync::Mutex<recheck::Recheck>,
}

/// A result as the Output tab shows it.
#[derive(Debug, Clone, Serialize)]
pub struct OutputItem {
    pub id: u32,
    pub operation: u32,
    pub bundle: ResultBundle,
    pub paragraph: Option<String>,
    /// The data changed after this result was run, so its numbers may no longer match. Run it again to update.
    pub stale: bool,
    /// The AI draft, when one was written.
    pub ai: Option<Value>,
    /// The command and arguments that made the result, so its dialog can be opened again with the same settings.
    pub command: String,
    pub args: Value,
}

#[derive(Deserialize)]
struct PathArgs {
    path: String,
    #[serde(default)]
    sheet: Option<String>,
}

#[derive(Deserialize)]
struct BytesArgs {
    file_name: String,
    base64: String,
    #[serde(default)]
    sheet: Option<String>,
}

/// A path given by the user, a script or an assistant, for a file on this computer. Network shares and device paths
/// (`\\server\share`, `\\?\`, `\\.\`) are refused: reaching one makes Windows contact another computer and offer the
/// user's sign-in. So are drive-relative paths (`C:file`), which depend on a hidden current folder.
pub(crate) fn local_path(p: &str) -> Result<PathBuf, String> {
    let t = p.trim();
    if t.is_empty() {
        return Err("Choose a file.".into());
    }
    let b = t.as_bytes();
    let slash = |c: u8| c == b'\\' || c == b'/';
    if b.len() >= 2 && slash(b[0]) && slash(b[1]) || t.starts_with("\\??") {
        return Err(format!("{t} is a network or device path. Copy the file to this computer and use that copy."));
    }
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && !(b.len() >= 3 && slash(b[2])) {
        return Err(format!("{t} has no folder after the drive letter. Give the full path, starting with the drive and a backslash, such as D:\\Data\\file."));
    }
    Ok(PathBuf::from(t))
}

impl Session {
    /// Writing `path` is allowed: always from the window and the command line, but a script may replace an existing
    /// file only when the user agreed to replace that file before the script ran.
    pub(crate) fn may_write(&self, path: &Path) -> Result<(), String> {
        if let Some(allowed) = &self.script_replace {
            if path.exists() && !allowed.iter().any(|a| Path::new(a) == path) {
                return Err(format!("{} already exists. A script replaces a file only when you agree to it before the script runs; give the line a new file name, or run the script from the Script screen and agree to replace it.", path.display()));
            }
        }
        Ok(())
    }
}

/// The other file of an append or merge: from a path, or from the bytes the app read (file_name + base64).
fn other_file(args: &Value) -> Result<(String, prism_engine::Dataset), String> {
    #[derive(Deserialize)]
    struct A {
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        file_name: Option<String>,
        #[serde(default)]
        base64: Option<String>,
        #[serde(default)]
        sheet: Option<String>,
    }
    let a: A = parse(args.clone())?;
    let (name, bytes) = match (&a.path, &a.base64) {
        (Some(p), _) => (Path::new(p).file_name().and_then(|n| n.to_str()).unwrap_or("data").to_string(), std::fs::read(local_path(p)?).map_err(|e| format!("Could not read the file: {e}"))?),
        (None, Some(b)) => (a.file_name.clone().unwrap_or_else(|| "data".into()), base64::engine::general_purpose::STANDARD.decode(b.as_bytes()).map_err(|_| "The file could not be read.".to_string())?),
        _ => return Err("Choose the other data file.".into()),
    };
    let stem = Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or("Other").to_string();
    let (ds, _) = import::read_bytes(&name, &stem, &bytes, a.sheet.as_deref()).map_err(|e| e.to_string())?;
    Ok((name, ds))
}

/// What the history keeps of an append or merge: the options and the other file's name, not its bytes.
fn recorded_file_args(args: &Value, name: &str) -> Value {
    let mut a = args.clone();
    if let Some(o) = a.as_object_mut() {
        o.remove("base64");
        o.remove("path");
        o.insert("file_name".into(), json!(name));
    }
    a
}

#[derive(Deserialize)]
struct OpenTextArgs {
    name: String,
    text: String,
}

#[derive(Deserialize)]
struct RowsArgs {
    start: usize,
    count: usize,
}

#[derive(Deserialize, Serialize)]
struct VariableArgs {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    measure: Option<Measure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value_labels: Option<Vec<ValueLabel>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    missing_values: Option<Vec<f64>>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs one command. Unknown commands and bad arguments come back as errors, never panics: a failure inside the
    /// engine (for example on a damaged project) becomes an error message, and the session keeps working.
    pub fn handle(&mut self, command: &str, args: Value) -> Result<Value, String> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.handle_command(command, args))) {
            Ok(r) => r,
            Err(_) => {
                self.nesting = 0;
                self.script_replace = None;
                Err(format!("\"{command}\" stopped on an internal error, so it did nothing more. Your data and output are still open; please report what you were doing."))
            }
        }
    }

    fn handle_command(&mut self, command: &str, args: Value) -> Result<Value, String> {
        match command {
            "undo" | "redo" => {
                let label = self.step_back(command == "undo")?;
                Ok(json!({ "did": label, "dataset": self.summary()?, "state": self.state() }))
            }
            _ => self.with_undo(command, |s| s.dispatch(command, args)),
        }
    }

    fn dispatch(&mut self, command: &str, args: Value) -> Result<Value, String> {
        match command {
            "open_file" => {
                let a: PathArgs = parse(args)?;
                let bytes = std::fs::read(local_path(&a.path)?).map_err(|e| format!("Could not read the file: {e}"))?;
                let file_name = Path::new(&a.path).file_name().and_then(|n| n.to_str()).unwrap_or("data").to_string();
                let mut opened = self.open_data(&file_name, &bytes, a.sheet.as_deref())?;
                // A .sas7bdat file keeps its value labels in a catalog beside it: the same name, or formats.sas7bcat.
                if import::extension(&file_name).as_deref() == Some("sas7bdat") {
                    let path = Path::new(&a.path);
                    let found = [path.with_extension("sas7bcat"), path.with_file_name("formats.sas7bcat")].into_iter().find(|c| c.is_file());
                    let notes = match found {
                        Some(cat) => {
                            let name = cat.file_name().and_then(|n| n.to_str()).unwrap_or("the catalog").to_string();
                            match std::fs::read(&cat).map_err(|e| e.to_string()).and_then(|b| self.attach_sas_catalog(&name, &b)) {
                                Ok(n) => n,
                                Err(e) => vec![format!("{name} couldn't be read, so no value labels were attached: {e}")],
                            }
                        }
                        None => vec!["Value labels for a .sas7bdat file live in a separate format catalog (.sas7bcat). None was found next to this file; to add them, choose File › Import › SAS value labels.".into()],
                    };
                    if let Some(list) = opened["report"]["notes"].as_array_mut() {
                        list.extend(notes.into_iter().map(Value::String));
                    }
                    opened["dataset"] = self.summary()?;
                }
                Ok(opened)
            }
            "load_sas_catalog" => {
                #[derive(Deserialize)]
                struct A {
                    #[serde(default)]
                    path: Option<String>,
                    #[serde(default)]
                    file_name: Option<String>,
                    #[serde(default)]
                    base64: Option<String>,
                }
                let a: A = parse(args)?;
                let (name, bytes) = match (&a.path, &a.base64) {
                    (Some(p), _) => (Path::new(p).file_name().and_then(|n| n.to_str()).unwrap_or("the catalog").to_string(), std::fs::read(local_path(p)?).map_err(|e| format!("Could not read the catalog: {e}"))?),
                    (None, Some(b)) => (a.file_name.clone().unwrap_or_else(|| "the catalog".into()), base64::engine::general_purpose::STANDARD.decode(b.as_bytes()).map_err(|_| "The catalog could not be read.".to_string())?),
                    _ => return Err("Choose a .sas7bcat file.".into()),
                };
                let notes = self.attach_sas_catalog(&name, &bytes)?;
                Ok(json!({ "notes": notes, "dataset": self.summary()? }))
            }
            "data_health" => Ok(serde_json::to_value(prism_engine::health::check(&self.project()?.dataset)).map_err(|e| e.to_string())?),
            "health_fix" => {
                #[derive(Deserialize)]
                struct A {
                    fix: prism_engine::health::Fix,
                    #[serde(default)]
                    apply: bool,
                }
                let a: A = parse(args)?;
                let ds = &mut self.project_mut()?.dataset;
                let preview = prism_engine::health::fix(ds, &a.fix, a.apply).map_err(|e| e.to_string())?;
                if !a.apply {
                    return Ok(json!({ "preview": preview }));
                }
                self.record("health_fix", json!({ "fix": a.fix, "apply": true }))?;
                Ok(json!({ "preview": preview, "dataset": self.summary()? }))
            }
            "paste_cells" => {
                #[derive(Deserialize)]
                struct A {
                    row: usize,
                    name: String,
                    text: String,
                    #[serde(default)]
                    apply: bool,
                }
                let a: A = parse(args)?;
                let ds = &mut self.project_mut()?.dataset;
                let plan = prism_engine::health::paste(ds, a.row, &a.name, &a.text, a.apply).map_err(|e| e.to_string())?;
                if !a.apply {
                    return Ok(json!({ "plan": plan }));
                }
                // The cells changed are kept too, so the R export can repeat them.
                let cells: Vec<Value> = plan.changes.iter().filter(|c| c.problem.is_none()).map(|c| json!([c.row, c.name, c.new])).collect();
                self.record("paste_cells", json!({ "row": a.row, "name": a.name, "text": a.text, "apply": true, "cells": cells, "new_cases": plan.new_cases }))?;
                Ok(json!({ "plan": plan, "dataset": self.summary()? }))
            }
            "inspect_file" => {
                // Reads another file (for append or merge) without opening it: its variables and size.
                let (name, other) = other_file(&args)?;
                let vars: Vec<Value> = other.variables.iter().zip(&other.columns).map(|(v, c)| json!({ "name": v.name, "label": v.label, "type": if matches!(c, Column::Numeric(_)) { "numeric" } else { "text" } })).collect();
                Ok(json!({ "file_name": name, "rows": other.n_rows(), "variables": vars }))
            }
            "append_cases" => {
                #[derive(Deserialize)]
                struct A {
                    #[serde(default)]
                    source: Option<String>,
                }
                let a: A = parse(args.clone())?;
                let (name, other) = other_file(&args)?;
                let ds = &mut self.project_mut()?.dataset;
                let r = prism_engine::reshape::append(ds, &other, a.source.as_deref().map(str::trim).filter(|s| !s.is_empty())).map_err(|e| e.to_string())?;
                self.record("append_cases", recorded_file_args(&args, &name))?;
                Ok(json!({ "report": r, "dataset": self.summary()? }))
            }
            "merge_variables" => {
                #[derive(Deserialize)]
                struct A {
                    keys: Vec<String>,
                    #[serde(default)]
                    join: prism_engine::reshape::Join,
                }
                let a: A = parse(args.clone())?;
                let (name, other) = other_file(&args)?;
                let ds = &mut self.project_mut()?.dataset;
                let r = prism_engine::reshape::merge(ds, &other, &a.keys, a.join).map_err(|e| e.to_string())?;
                self.record("merge_variables", recorded_file_args(&args, &name))?;
                Ok(json!({ "report": r, "dataset": self.summary()? }))
            }
            "aggregate" => {
                let a: prism_engine::reshape::AggregateSpec = parse(args.clone())?;
                let ds = &mut self.project_mut()?.dataset;
                let r = prism_engine::reshape::aggregate(ds, &a).map_err(|e| e.to_string())?;
                self.record("aggregate", args)?;
                Ok(json!({ "report": r, "dataset": self.summary()? }))
            }
            "restructure_long" => {
                let a: prism_engine::reshape::LongSpec = parse(args.clone())?;
                let ds = &mut self.project_mut()?.dataset;
                let r = prism_engine::reshape::to_long(ds, &a).map_err(|e| e.to_string())?;
                self.record("restructure_long", args)?;
                Ok(json!({ "report": r, "dataset": self.summary()? }))
            }
            "restructure_wide" => {
                let a: prism_engine::reshape::WideSpec = parse(args.clone())?;
                let ds = &mut self.project_mut()?.dataset;
                let r = prism_engine::reshape::to_wide(ds, &a).map_err(|e| e.to_string())?;
                self.record("restructure_wide", args)?;
                Ok(json!({ "report": r, "dataset": self.summary()? }))
            }
            "open_bytes" => {
                let a: BytesArgs = parse(args)?;
                let bytes = base64::engine::general_purpose::STANDARD.decode(a.base64.as_bytes()).map_err(|_| "The file could not be read.".to_string())?;
                self.open_data(&a.file_name, &bytes, a.sheet.as_deref())
            }
            "create_design" => {
                // Builds an experimental design, opens it as the data (replacing what is open) and records the design.
                let spec: doe::Spec = parse(args.clone())?;
                let d = doe::build(&spec).map_err(|e| e.to_string())?;
                let text = doe::csv(&spec, &d);
                let name = match spec.kind {
                    doe::Kind::Full => "Full factorial design.csv",
                    doe::Kind::Fractional => "Fractional factorial design.csv",
                    doe::Kind::Ccd => "Central composite design.csv",
                    doe::Kind::Bbd => "Box-Behnken design.csv",
                    doe::Kind::Pb => "Plackett-Burman design.csv",
                    doe::Kind::Lattice | doe::Kind::Centroid => "Mixture design.csv",
                    doe::Kind::Optimal => "Optimal design.csv",
                    doe::Kind::Vertices => "Mixture design.csv",
                };
                let mut reply = self.open_data(name, text.as_bytes(), None)?;
                let outputs = self.handle("doe_design", args)?;
                reply["outputs"] = outputs;
                reply["state"] = self.state();
                Ok(reply)
            }
            "open_text" => {
                let a: OpenTextArgs = parse(args)?;
                self.open_data(&a.name, a.text.as_bytes(), None)
            }
            "open_google_sheet" => {
                #[derive(Deserialize)]
                struct A {
                    url: String,
                    #[serde(default)]
                    sheet: Option<String>,
                }
                let a: A = parse(args)?;
                // The download is kept until a sheet is chosen, so a workbook with several tabs is fetched once.
                let (name, bytes) = match self.google.take().filter(|(u, _, _)| *u == a.url) {
                    Some((_, n, b)) => (n, b),
                    None => google::download(&a.url)?,
                };
                let reply = self.open_data(&name, &bytes, a.sheet.as_deref())?;
                if reply.get("choose_sheet").is_some() {
                    self.google = Some((a.url, name, bytes));
                    return Ok(reply);
                }
                let p = self.project_mut()?;
                if let Some(op) = p.operations.first_mut() {
                    op.args["google_sheet"] = json!(a.url.trim());
                }
                if let Some(src) = p.source.as_mut() {
                    src.report.notes.push("Downloaded from a shared spreadsheet link. Later changes to the sheet don't appear here; open the link again to get them.".into());
                }
                Ok(json!({ "report": p.source.as_ref().map(|s| &s.report), "dataset": self.summary()?, "state": self.state() }))
            }
            "project_state" => Ok(self.state()),
            "autosave" | "clear_recovery" | "recovery_list" | "recover" | "discard_recovery" => self.recovery_command(command, args),
            c if recovery::job_command(c, &args).is_some() => recovery::job_command(c, &args).unwrap_or(Ok(Value::Null)),
            "save_project" => {
                #[derive(Deserialize)]
                struct A {
                    #[serde(default)]
                    path: Option<String>,
                }
                let a: A = parse(args)?;
                if self.project.is_none() {
                    return Err("There is nothing to save yet. Open a data file first.".into());
                }
                let mut path = match a.path {
                    Some(p) => local_path(&p)?,
                    None => self.path.clone().ok_or("Choose where to save the project.")?,
                };
                if import::extension(&path.to_string_lossy()).as_deref() != Some(project::EXTENSION) {
                    let mut s = path.into_os_string();
                    s.push(format!(".{}", project::EXTENSION));
                    path = PathBuf::from(s);
                }
                if self.path.as_ref() != Some(&path) {
                    self.may_write(&path)?;
                }
                project::save(&path, &self.to_save()?).map_err(|e| e.to_string())?;
                self.path = Some(path);
                self.dirty = false;
                self.clear_own_recovery();
                Ok(self.state())
            }
            "open_project" => {
                let a: PathArgs = parse(args)?;
                let p = project::load(&local_path(&a.path)?).map_err(|e| e.to_string())?;
                self.project = Some(p);
                self.path = Some(PathBuf::from(a.path));
                self.dirty = false;
                self.clear_own_recovery();
                let offer = self.project.as_ref().and_then(project::profile_suggestions);
                let mut reply = json!({ "state": self.state(), "dataset": self.summary()?, "results": self.outputs() });
                if let Some((about, suggestions)) = offer {
                    reply["profile"] = json!({ "about": about, "suggestions": suggestions });
                }
                Ok(reply)
            }
            "dataset" => self.summary(),
            "rows" => {
                let a: RowsArgs = parse(args)?;
                self.rows(a.start, a.count)
            }
            "examples" => Ok(examples::list()),
            "open_example" => {
                #[derive(Deserialize)]
                struct A {
                    id: String,
                    #[serde(default)]
                    blank: bool,
                }
                let a: A = parse(args)?;
                self.open_example(&a.id, a.blank)
            }
            "find" => {
                #[derive(Deserialize)]
                struct A {
                    text: String,
                    #[serde(default)]
                    variable: Option<String>,
                    #[serde(default)]
                    whole: bool,
                    #[serde(default)]
                    labels: bool,
                }
                let a: A = parse(args)?;
                self.find(&a.text, a.variable.as_deref(), a.whole, a.labels)
            }
            "update_variable" => {
                let a: VariableArgs = parse(args)?;
                let record = serde_json::to_value(&a).map_err(|e| e.to_string())?;
                let ds = &mut self.project_mut()?.dataset;
                let v = ds.variable_mut(&a.name).map_err(|e| e.to_string())?;
                if let Some(l) = a.label {
                    v.label = l;
                }
                if let Some(m) = a.measure {
                    v.measure = m;
                }
                if let Some(vl) = a.value_labels {
                    v.value_labels = vl;
                }
                if let Some(mv) = a.missing_values {
                    v.missing_values = mv;
                }
                self.record("update_variable", record)?;
                self.summary()
            }
            "set_cell" => {
                // Data View editing (only with the browse-only switch off). Recorded like every other change, so results
                // that used the old value are marked out of date.
                #[derive(Deserialize)]
                struct A {
                    row: usize,
                    name: String,
                    #[serde(default)]
                    value: Option<String>,
                }
                let a: A = parse(args)?;
                let ds = &mut self.project_mut()?.dataset;
                let i = ds.find(&a.name).ok_or_else(|| format!("There is no variable named {}.", a.name))?;
                let n = ds.n_rows();
                if a.row >= n {
                    return Err(format!("There is no case {} (the data have {n}).", a.row + 1));
                }
                let text = a.value.as_deref().map(str::trim).filter(|t| !t.is_empty());
                let old = match &mut ds.columns[i] {
                    Column::Numeric(v) => {
                        let new = match text {
                            None => None,
                            Some(t) => Some(t.parse::<f64>().ok().filter(|x| x.is_finite()).ok_or_else(|| format!("\"{t}\" is not a number, and {} is a numeric variable. Leave the cell empty for a missing value.", a.name))?),
                        };
                        let old = v[a.row].map_or(Value::Null, |x| json!(x));
                        v[a.row] = new;
                        old
                    }
                    Column::Text(v) => {
                        let old = v[a.row].clone().map_or(Value::Null, Value::String);
                        v[a.row] = text.map(str::to_string);
                        old
                    }
                };
                self.record("set_cell", json!({ "row": a.row, "name": a.name, "value": text, "old": old }))?;
                self.rows(a.row, 1)
            }
            "sem_diagram" => {
                // The diagram of a model text, laid out around any positions given (the diagram editor).
                #[derive(Deserialize)]
                struct A {
                    model: String,
                    #[serde(default)]
                    layout: std::collections::BTreeMap<String, [f64; 2]>,
                }
                let a: A = parse(args)?;
                let mut d = prism_engine::analysis::sem::diagram::structure(&a.model).map_err(|e| e.to_string())?;
                prism_engine::analysis::sem::diagram::layout(&mut d, &a.layout);
                serde_json::to_value(d).map_err(|e| e.to_string())
            }
            "group_codes" => {
                #[derive(Deserialize)]
                struct A {
                    name: String,
                }
                let a: A = parse(args)?;
                self.group_codes(&a.name)
            }
            c if ANALYSES.contains(&c) => {
                if (c == "power" || c == "doe_design") && self.project.is_none() {
                    // Planning needs no data: results go into a project with an empty dataset until data are opened.
                    self.project = Some(Project::new(Dataset::new("Untitled"), None));
                }
                let mut args = args;
                let settings = self.project()?.data_settings.clone();
                if c != "power" && c != "doe_design" && !settings.is_empty() && args.get("data").is_none() {
                    if let Some(o) = args.as_object_mut() {
                        o.insert("data".into(), serde_json::to_value(&settings).map_err(|e| e.to_string())?);
                    }
                }
                let started = std::time::Instant::now();
                let made = run_job(&self.project()?.dataset, c, &args)?;
                let took = started.elapsed().as_secs_f64();
                let seq = self.record(c, args)?;
                let p = self.project_mut()?;
                let mut ids = Vec::new();
                for (bundle, paragraph) in made {
                    let id = p.results.iter().map(|r| r.id).max().unwrap_or(0) + 1;
                    p.results.push(StoredResult { id, operation: seq, bundle, paragraph, ai: None, checked: None });
                    ids.push(id);
                }
                self.just_made(&ids, took);
                Ok(json!(self.outputs().into_iter().filter(|o| ids.contains(&o.id)).collect::<Vec<_>>()))
            }
            "rerun" => {
                #[derive(Deserialize)]
                struct A {
                    id: u32,
                }
                let a: A = parse(args)?;
                self.rerun(a.id)?;
                Ok(json!(self.outputs()))
            }
            "participants" => {
                #[derive(Deserialize)]
                struct A {
                    ids: Vec<u32>,
                }
                let a: A = parse(args)?;
                let p = self.project()?;
                let chosen: Vec<&StoredResult> = p.results.iter().filter(|r| a.ids.contains(&r.id)).collect();
                let freqs: Vec<&ResultBundle> = chosen.iter().filter(|r| r.bundle.method_id == frequencies::METHOD_ID).map(|r| &r.bundle).collect();
                let desc = chosen.iter().rev().find(|r| r.bundle.method_id == descriptives::METHOD_ID).map(|r| &r.bundle);
                let text = apa::participants_paragraph(&freqs, desc).ok_or("Choose at least one Frequencies or Descriptives result.")?;
                Ok(json!({ "paragraph": text }))
            }
            "export" => {
                #[derive(Deserialize)]
                struct A {
                    format: String,
                    #[serde(default)]
                    path: Option<String>,
                    #[serde(default)]
                    ids: Option<Vec<u32>>,
                    /// Use the Report Builder's layout.
                    #[serde(default)]
                    report: bool,
                }
                let a: A = parse(args)?;
                self.export(&a.format, a.path.as_deref(), a.ids.as_deref(), a.report)
            }
            "export_data" => {
                // The dataset itself, as CSV (codes, or value labels) or .sav/.zsav, with its codebook where the format
                // holds one (development list item 8).
                #[derive(Deserialize)]
                struct A {
                    format: String,
                    #[serde(default)]
                    path: Option<String>,
                }
                let a: A = parse(args)?;
                let ds = &self.project()?.dataset;
                let (bytes, ext) = match a.format.as_str() {
                    "csv" => (prism_engine::dataset_export::to_csv(ds, false), "csv"),
                    "csv_labels" => (prism_engine::dataset_export::to_csv(ds, true), "csv"),
                    "sav" => (prism_engine::dataset_export::to_sav(ds, false).map_err(|e| e.to_string())?, "sav"),
                    "zsav" => (prism_engine::dataset_export::to_sav(ds, true).map_err(|e| e.to_string())?, "zsav"),
                    f => return Err(format!("Unknown data format \"{f}\". Use csv, csv_labels, sav or zsav.")),
                };
                let stem: String = ds.name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
                let file_name = format!("{}.{ext}", if stem.is_empty() { "data" } else { &stem });
                let (rows, vars) = (ds.n_rows(), ds.variables.len());
                match a.path {
                    Some(path) => {
                        let target = local_path(&path)?;
                        self.may_write(&target)?;
                        project::write_atomic(&target, &bytes).map_err(|e| format!("Could not save {path}: {e}"))?;
                        Ok(json!({ "path": path, "rows": rows, "variables": vars }))
                    }
                    None => Ok(json!({ "file_name": file_name, "base64": base64::engine::general_purpose::STANDARD.encode(&bytes), "rows": rows, "variables": vars })),
                }
            }
            "export_item" => {
                // One figure as PNG or SVG, or one table as CSV (full-precision numbers), from a result's own menu.
                #[derive(Deserialize)]
                struct A {
                    id: u32,
                    /// A chart's or table's id within the result.
                    item: String,
                    format: String,
                    #[serde(default)]
                    path: Option<String>,
                }
                let a: A = parse(args)?;
                let p = self.project()?;
                let r = p.results.iter().find(|r| r.id == a.id).ok_or("That result is no longer in the output.")?;
                let stem: String = r.bundle.title.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
                let (bytes, ext, what) = match a.format.as_str() {
                    "png" | "svg" => {
                        let c = r.bundle.charts.iter().find(|c| c.id == a.item).ok_or("That figure is no longer in the result.")?;
                        let bytes = if a.format == "svg" { c.svg.clone().into_bytes() } else { prism_pdf::chart_png(&c.svg, 3.0).ok_or("The figure could not be drawn as a PNG.")? };
                        (bytes, a.format.clone(), c.title.clone())
                    }
                    "csv" => {
                        let t = r.bundle.tables.iter().find(|t| t.id == a.item).ok_or("That table is no longer in the result.")?;
                        (table_csv(t).into_bytes(), "csv".to_string(), t.title.clone())
                    }
                    f => return Err(format!("Unknown format \"{f}\".")),
                };
                let file_name = format!("{stem} - {}.{ext}", what.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" "));
                match a.path {
                    Some(path) => {
                        let target = local_path(&path)?;
                        self.may_write(&target)?;
                        project::write_atomic(&target, &bytes).map_err(|e| format!("Could not save {path}: {e}"))?;
                        Ok(json!({ "path": path }))
                    }
                    None => Ok(json!({ "file_name": file_name, "base64": base64::engine::general_purpose::STANDARD.encode(&bytes) })),
                }
            }
            "save_regression" => {
                // Saves a regression result's predicted values, residuals, standardized residuals, Cook's distances
                // and leverages as new variables, filled for the cases the model used.
                #[derive(Deserialize)]
                struct A {
                    id: u32,
                }
                let a: A = parse(args)?;
                let p = self.project()?;
                let r = p.results.iter().find(|r| r.id == a.id).ok_or("That result is no longer in the output.")?;
                let op = p.operations.iter().find(|o| o.seq == r.operation).ok_or("The settings for this result are missing.")?;
                if op.command != "regression" {
                    return Err("Only a linear regression result has values to save.".into());
                }
                let op_args = op.args.clone();
                let spec: regression::Spec = parse(op_args.clone())?;
                let vals = regression::case_values(&p.dataset, &spec).map_err(|e| e.to_string())?;
                let n = p.dataset.n_rows();
                let ds = &mut self.project_mut()?.dataset;
                let mut k = 1;
                while ["PRE", "RES", "ZRE", "COO", "LEV"].iter().any(|s| ds.find(&format!("{s}_{k}")).is_some()) {
                    k += 1;
                }
                let labels = ["Predicted value", "Residual", "Standardized residual", "Cook's distance", "Leverage"];
                let mut made = Vec::new();
                for (j, stem) in ["PRE", "RES", "ZRE", "COO", "LEV"].iter().enumerate() {
                    let mut col = vec![None; n];
                    for (row, v) in &vals {
                        col[*row] = Some(v[j]);
                    }
                    let name = format!("{stem}_{k}");
                    ds.add_variable(&name, Column::Numeric(col)).map_err(|e| e.to_string())?;
                    ds.variable_mut(&name).map_err(|e| e.to_string())?.label = format!("{} from result {}", labels[j], a.id);
                    made.push(name);
                }
                self.record("save_regression", json!({ "id": a.id, "regression": op_args, "names": made }))?;
                Ok(json!({ "names": made, "dataset": self.summary()? }))
            }
            "data_settings" => Ok(serde_json::to_value(&self.project()?.data_settings).map_err(|e| e.to_string())?),
            "set_data_settings" => {
                // Select cases, Split file and Weight cases, for the analyses run from now on. Earlier results keep the
                // settings they ran with, so this change doesn't make them out of date.
                let mut a: prism_engine::subset::DataSettings = parse(args)?;
                let blank = |s: Option<String>| s.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
                a.filter = blank(a.filter);
                a.split = blank(a.split);
                a.weight = blank(a.weight);
                a.check(&self.project()?.dataset).map_err(|e| e.to_string())?;
                self.project_mut()?.data_settings = a.clone();
                self.record("set_data_settings", serde_json::to_value(&a).map_err(|e| e.to_string())?)?;
                Ok(serde_json::to_value(&a).map_err(|e| e.to_string())?)
            }
            "compute_variable" => {
                #[derive(Deserialize)]
                struct A {
                    target: String,
                    formula: String,
                    #[serde(default)]
                    label: Option<String>,
                    #[serde(default)]
                    replace: bool,
                }
                let a: A = parse(args.clone())?;
                let ds = &mut self.project_mut()?.dataset;
                let r = prism_engine::data_ops::compute(ds, &a.target, &a.formula, a.label.as_deref(), a.replace).map_err(|e| e.to_string())?;
                self.record("compute_variable", args)?;
                Ok(json!({ "report": r, "dataset": self.summary()? }))
            }
            "recode" => {
                #[derive(Deserialize)]
                struct A {
                    variables: Vec<String>,
                    rules: Vec<prism_engine::data_ops::Rule>,
                    #[serde(default)]
                    into: Option<Vec<String>>,
                    #[serde(default)]
                    label: Option<String>,
                }
                let a: A = parse(args.clone())?;
                let ds = &mut self.project_mut()?.dataset;
                let r = prism_engine::data_ops::recode(ds, &a.variables, &a.rules, a.into.as_deref().filter(|v| !v.is_empty()), a.label.as_deref()).map_err(|e| e.to_string())?;
                self.record("recode", args)?;
                Ok(json!({ "report": r, "dataset": self.summary()? }))
            }
            "sort_cases" => {
                #[derive(Deserialize)]
                struct A {
                    keys: Vec<prism_engine::transform::SortKey>,
                }
                let a: A = parse(args.clone())?;
                let ds = &mut self.project_mut()?.dataset;
                let n = prism_engine::transform::sort_cases(ds, &a.keys).map_err(|e| e.to_string())?;
                self.record("sort_cases", args)?;
                Ok(json!({ "cases": n, "dataset": self.summary()? }))
            }
            "rank_cases" | "standardize" | "reverse_score" | "date_parts" | "date_difference" => {
                use prism_engine::transform as tx;
                let ds = &mut self.project_mut()?.dataset;
                let r = match command {
                    "rank_cases" => tx::rank_cases(ds, &parse(args.clone())?),
                    "standardize" => tx::standardize(ds, &parse(args.clone())?),
                    "reverse_score" => tx::reverse_score(ds, &parse(args.clone())?),
                    "date_parts" => tx::date_parts(ds, &parse(args.clone())?),
                    _ => tx::date_difference(ds, &parse(args.clone())?),
                }
                .map_err(|e| e.to_string())?;
                self.record(command, args)?;
                Ok(json!({ "report": r, "dataset": self.summary()? }))
            }
            "scale_score" => {
                let a: prism_engine::data_ops::ScaleSpec = parse(args.clone())?;
                let ds = &mut self.project_mut()?.dataset;
                let r = prism_engine::data_ops::scale_score(ds, &a).map_err(|e| e.to_string())?;
                self.record("scale_score", args)?;
                Ok(json!({ "report": r, "dataset": self.summary()? }))
            }
            "script_outputs" => {
                #[derive(Deserialize)]
                struct A {
                    text: String,
                }
                let a: A = parse(args)?;
                Ok(json!(script::outputs(&a.text)))
            }
            "run_script" => {
                #[derive(Deserialize)]
                struct A {
                    text: String,
                    /// Existing files the user agreed the script may replace.
                    #[serde(default)]
                    replace: Vec<String>,
                }
                let a: A = parse(args)?;
                let mut r = script::run_with(self, &a.text, &a.replace);
                r["results"] = json!(self.outputs());
                r["state"] = self.state();
                Ok(r)
            }
            "history_script" => Ok(json!({ "text": script::from_history(self)? })),
            "report_get" => Ok(json!(self.report_layout())),
            "report_set" => {
                let rep: Report = parse(args)?;
                let p = self.project_mut()?;
                p.report = Some(rep);
                self.changed();
                Ok(json!(self.report_layout()))
            }
            "read_script" => {
                let a: PathArgs = parse(args)?;
                let text = std::fs::read_to_string(local_path(&a.path)?).map_err(|e| format!("Could not read the script: {e}"))?;
                Ok(json!({ "text": text, "path": a.path }))
            }
            "write_script" => {
                #[derive(Deserialize)]
                struct A {
                    path: String,
                    text: String,
                }
                let a: A = parse(args)?;
                let path = if Path::new(&a.path).extension().is_none() { format!("{}.{}", a.path, script::EXTENSION) } else { a.path };
                std::fs::write(local_path(&path)?, a.text).map_err(|e| format!("Could not save the script: {e}"))?;
                Ok(json!({ "path": path }))
            }
            c if c.starts_with("ai_") => self.ai_command(c, args),
            c if c.starts_with("mcp_") => mcp::command(c, args),
            "results" => Ok(json!(self.outputs())),
            "clear_results" => {
                self.project_mut()?.results.clear();
                self.changed();
                Ok(json!([]))
            }
            other => Err(format!("Unknown command \"{other}\".")),
        }
    }

    fn open_data(&mut self, file_name: &str, bytes: &[u8], sheet: Option<&str>) -> Result<Value, String> {
        let is_workbook = matches!(import::extension(file_name).as_deref(), Some("xlsx") | Some("xlsm"));
        if is_workbook && sheet.is_none() {
            let sheets = prism_engine::xlsx::sheet_names(bytes).map_err(|e| e.to_string())?;
            let visible: Vec<&String> = sheets.iter().filter(|(_, v)| *v).map(|(n, _)| n).collect();
            if visible.len() > 1 {
                return Ok(json!({ "choose_sheet": visible }));
            }
        }
        let stem = Path::new(file_name).file_stem().and_then(|s| s.to_str()).unwrap_or("Dataset").to_string();
        let (ds, report) = import::read_bytes(file_name, &stem, bytes, sheet).map_err(|e| e.to_string())?;
        let mut p = Project::new(ds, Some(Source { file_name: file_name.to_string(), report: report.clone() }));
        p.operations.push(Operation { seq: 1, command: "import".into(), args: json!({ "file_name": file_name, "sheet": report.sheet }) });
        // Power analyses run before any data was open move into the new project.
        if let Some(old) = self.project.take().filter(|o| o.dataset.variables.is_empty()) {
            for op in old.operations.into_iter().filter(|o| o.command == "power") {
                let seq = p.operations.len() as u32 + 1;
                for r in old.results.iter().filter(|r| r.operation == op.seq) {
                    let mut r = r.clone();
                    r.operation = seq;
                    r.id = p.results.len() as u32 + 1;
                    p.results.push(r);
                }
                p.operations.push(Operation { seq, command: op.command, args: op.args });
            }
        }
        self.project = Some(p);
        self.path = None;
        self.changed();
        Ok(json!({ "report": report, "dataset": self.summary()?, "state": self.state() }))
    }

    fn project(&self) -> Result<&Project, String> {
        self.project.as_ref().ok_or_else(|| "Open a data file first.".to_string())
    }

    fn project_mut(&mut self) -> Result<&mut Project, String> {
        self.project.as_mut().ok_or_else(|| "Open a data file first.".to_string())
    }

    /// Marks the project as changed since it was last saved.
    pub(crate) fn changed(&mut self) {
        self.dirty = true;
        self.edits += 1;
    }

    /// Adds an operation to the record and returns its number.
    fn record(&mut self, command: &str, args: Value) -> Result<u32, String> {
        let p = self.project_mut()?;
        let seq = p.operations.iter().map(|o| o.seq).max().unwrap_or(0) + 1;
        p.operations.push(Operation { seq, command: command.to_string(), args });
        self.changed();
        Ok(seq)
    }

    fn state(&self) -> Value {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).and_then(|s| s.to_str()).map(str::to_string);
        json!({
            "path": self.path.as_ref().map(|p| p.to_string_lossy().to_string()),
            "name": name.or_else(|| self.project.as_ref().map(|p| p.dataset.name.clone())),
            "dirty": self.dirty,
            "has_data": self.project.as_ref().is_some_and(|p| !p.dataset.variables.is_empty()),
            "extension": project::EXTENSION,
            "undo": self.undo_names().0,
            "redo": self.undo_names().1,
        })
    }

    /// The results with their out-of-date flags, worked out by running each operation again on the current data.
    pub(crate) fn outputs(&self) -> Vec<OutputItem> {
        let Some(p) = &self.project else { return Vec::new() };
        let mut hashes = recheck::Hashes::new(&p.dataset);
        let mut known = self.recheck.lock().unwrap_or_else(|e| e.into_inner());
        // The key of each operation that made a result, and the operations whose results need checking by running them.
        let mut keys: Vec<(u32, u64)> = Vec::new();
        let mut todo: Vec<&Operation> = Vec::new();
        for r in &p.results {
            let Some(op) = p.operations.iter().find(|o| o.seq == r.operation) else { continue };
            let key = match keys.iter().find(|(s, _)| *s == op.seq) {
                Some((_, k)) => *k,
                None => {
                    let k = hashes.key(op);
                    keys.push((op.seq, k));
                    k
                }
            };
            // A slow analysis is out of date as soon as what it reads changes (recheck.rs); the others are run to see.
            if known.get(r, key).is_none() && !known.slow(op.seq) && !todo.iter().any(|o| o.seq == op.seq) {
                todo.push(op);
            }
        }
        // Run those again, several at once.
        let runs: Vec<Option<(Option<Vec<ResultBundle>>, f64)>> = parallel(&todo, |o| {
            let started = std::time::Instant::now();
            let made = run_analysis(&p.dataset, &o.command, &o.args).ok().map(|v| v.into_iter().map(|(b, _)| b).collect());
            Some((made, started.elapsed().as_secs_f64()))
        });
        let again: Vec<(u32, Option<Vec<ResultBundle>>)> = runs
            .into_iter()
            .zip(&todo)
            .map(|(run, o)| {
                let (made, took) = run.unwrap_or((None, 0.0));
                known.took(o.seq, took);
                (o.seq, made)
            })
            .collect();
        let mut seen: Vec<(u32, usize)> = Vec::new();
        p.results
            .iter()
            .map(|r| {
                let index = match seen.iter_mut().find(|(op, _)| *op == r.operation) {
                    Some((_, n)) => {
                        *n += 1;
                        *n - 1
                    }
                    None => {
                        seen.push((r.operation, 1));
                        0
                    }
                };
                let key = keys.iter().find(|(s, _)| *s == r.operation).map(|(_, k)| *k);
                let stale = match key.and_then(|k| known.get(r, k)) {
                    Some(s) => s,
                    None => {
                        let fresh = again.iter().find(|(op, _)| *op == r.operation).and_then(|(_, b)| b.as_ref()).and_then(|b| b.get(index));
                        let stale = fresh.is_none_or(|b| !same_numbers(b, &r.bundle));
                        if let Some(k) = key {
                            known.put(r, k, stale);
                        }
                        stale
                    }
                };
                let op = p.operations.iter().find(|o| o.seq == r.operation);
                OutputItem { id: r.id, operation: r.operation, bundle: r.bundle.clone(), paragraph: r.paragraph.clone(), stale, ai: r.ai.clone(), command: op.map(|o| o.command.clone()).unwrap_or_default(), args: op.map(|o| o.args.clone()).unwrap_or(Value::Null) }
            })
            .collect()
    }

    /// Forgets the kept out-of-date answers (other data or another project was opened).
    pub(crate) fn forget_checks(&mut self) {
        self.recheck.get_mut().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Notes that these results were just made from the current data, so they are up to date without running them again.
    fn just_made(&self, ids: &[u32], seconds: f64) {
        let Some(p) = &self.project else { return };
        let mut hashes = recheck::Hashes::new(&p.dataset);
        let mut known = self.recheck.lock().unwrap_or_else(|e| e.into_inner());
        for r in p.results.iter().filter(|r| ids.contains(&r.id)) {
            if let Some(op) = p.operations.iter().find(|o| o.seq == r.operation) {
                known.put(r, hashes.key(op), false);
                known.took(op.seq, seconds);
            }
        }
    }

    /// The project as it is saved: each result with its key and whether it is out of date now, so opening it again
    /// needn't run the analyses to know (recheck.rs).
    fn to_save(&self) -> Result<Project, String> {
        let flags: Vec<(u32, bool)> = self.outputs().into_iter().map(|o| (o.id, o.stale)).collect();
        let mut p = self.project()?.clone();
        let mut hashes = recheck::Hashes::new(&p.dataset);
        let keys: Vec<(u32, u64)> = p.operations.iter().map(|o| (o.seq, hashes.key(o))).collect();
        for r in &mut p.results {
            let key = keys.iter().find(|(s, _)| *s == r.operation).map(|(_, k)| *k);
            let stale = flags.iter().find(|(id, _)| *id == r.id).map(|(_, s)| *s);
            r.checked = key.zip(stale);
        }
        Ok(p)
    }

    /// Runs a result's operation again on the current data and replaces that operation's results.
    fn rerun(&mut self, id: u32) -> Result<(), String> {
        let p = self.project()?;
        let r = p.results.iter().find(|r| r.id == id).ok_or("That result is no longer in the output.")?;
        let op = p.operations.iter().find(|o| o.seq == r.operation).ok_or("The settings for this result are missing.")?.clone();
        let started = std::time::Instant::now();
        let made = run_job(&p.dataset, &op.command, &op.args)?;
        let took = started.elapsed().as_secs_f64();
        let seq = self.record(&op.command, op.args.clone())?;
        let p = self.project_mut()?;
        let at = p.results.iter().position(|r| r.operation == op.seq).unwrap_or(p.results.len());
        let old_ids: Vec<u32> = p.results.iter().filter(|r| r.operation == op.seq).map(|r| r.id).collect();
        p.results.retain(|r| r.operation != op.seq);
        let mut next = p.results.iter().map(|r| r.id).max().unwrap_or(0).max(id);
        let fresh: Vec<StoredResult> = made
            .into_iter()
            .map(|(bundle, paragraph)| {
                next += 1;
                StoredResult { id: next, operation: seq, bundle, paragraph, ai: None, checked: None }
            })
            .collect();
        let new_ids: Vec<u32> = fresh.iter().map(|r| r.id).collect();
        let at = at.min(p.results.len());
        p.results.splice(at..at, fresh);
        // The report keeps pointing at the same results, now run again.
        if let Some(rep) = &mut p.report {
            rep.blocks.retain_mut(|b| match b {
                ReportBlock::Result { id, .. } => match old_ids.iter().position(|o| o == id).and_then(|k| new_ids.get(k)) {
                    Some(n) => {
                        *id = *n;
                        true
                    }
                    None => !old_ids.contains(id),
                },
                _ => true,
            });
        }
        self.just_made(&new_ids, took);
        Ok(())
    }

    /// Attaches the value labels of a SAS format catalog to the open .sas7bdat data, recording it so the project shows
    /// where the labels came from. Returns the notes for the user.
    fn attach_sas_catalog(&mut self, name: &str, bytes: &[u8]) -> Result<Vec<String>, String> {
        let sets = prism_engine::statfile::read_sas_catalog(bytes).map_err(|e| e.to_string())?;
        let p = self.project_mut()?;
        let formats = p.source.as_ref().map(|s| s.report.sas_formats.clone()).unwrap_or_default();
        if formats.is_empty() {
            return Err("The open data didn't come from a .sas7bdat file with format names, so there is nothing to attach the catalog's labels to.".into());
        }
        let outcome = prism_engine::statfile::apply_sas_labels(&mut p.dataset, &formats, &sets);
        let labels: Vec<Value> = p.dataset.variables.iter().filter(|v| outcome.labeled.contains(&v.display_name().to_string())).map(|v| json!({ "name": v.name, "value_labels": v.value_labels })).collect();
        self.record("sas_catalog", json!({ "file_name": name, "labels": labels }))?;
        Ok(prism_engine::statfile::catalog_notes(&outcome, name))
    }

    /// The Report Builder's layout: the saved one (without results that no longer exist), or every result in order.
    fn report_layout(&self) -> Report {
        let Some(p) = &self.project else { return Report { title: String::new(), blocks: Vec::new() } };
        match &p.report {
            Some(r) => Report { title: r.title.clone(), blocks: r.blocks.iter().filter(|b| !matches!(b, ReportBlock::Result { id, .. } if !p.results.iter().any(|x| x.id == *id))).cloned().collect() },
            None => {
                let name = self.path.as_ref().and_then(|p| p.file_stem()).and_then(|s| s.to_str()).unwrap_or(&p.dataset.name).to_string();
                Report { title: format!("{name}: Results"), blocks: p.results.iter().map(|r| ReportBlock::Result { id: r.id, charts: true, tables: true, checks: true, paragraph: true, method: true }).collect() }
            }
        }
    }

    /// Writes the results (all, or the chosen ones) as a Word or HTML document. With a path the file is written there;
    /// without one (the browser test server) the bytes come back for the interface to download.
    fn export(&self, format: &str, path: Option<&str>, ids: Option<&[u32]>, report: bool) -> Result<Value, String> {
        if let Some(p) = path {
            local_path(p)?;
        }
        use prism_engine::export;
        let p = self.project()?;
        if format == "r" {
            // The R script repeats the whole record, whichever results are chosen.
            let text = rscript::script(p);
            let name = self.path.as_ref().and_then(|p| p.file_stem()).and_then(|s| s.to_str()).unwrap_or(&p.dataset.name).to_string();
            let count = p.results.len();
            return match path {
                Some(path) => {
                    let path = local_path(path)?;
                    let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("r")) { path } else { PathBuf::from(format!("{}.R", path.display())) };
                    self.may_write(&path)?;
                    std::fs::write(&path, text).map_err(|e| format!("The file could not be written: {e}"))?;
                    Ok(json!({ "path": path.to_string_lossy(), "count": count }))
                }
                None => Ok(json!({ "file_name": format!("{name} analysis.R"), "base64": base64::engine::general_purpose::STANDARD.encode(text.as_bytes()), "count": count })),
            };
        }
        let all = self.outputs();
        let name = self.path.as_ref().and_then(|p| p.file_stem()).and_then(|s| s.to_str()).unwrap_or(&p.dataset.name).to_string();
        // What goes in, in order: every result (or the chosen ones), or the Report Builder's layout.
        let layout = if report { self.report_layout() } else { Report { title: format!("{name}: Output"), blocks: all.iter().filter(|o| ids.is_none_or(|ids| ids.contains(&o.id))).map(|o| ReportBlock::Result { id: o.id, charts: true, tables: true, checks: true, paragraph: true, method: true }).collect() } };
        let mut chosen: Vec<(&OutputItem, export::Parts, Vec<export::Block>)> = Vec::new();
        let mut pending: Vec<export::Block> = Vec::new();
        for b in &layout.blocks {
            match b {
                ReportBlock::Heading { text } => pending.push(export::Block { heading: true, text: text.clone() }),
                ReportBlock::Text { text } => pending.push(export::Block { heading: false, text: text.clone() }),
                ReportBlock::Result { id, charts, tables, checks, paragraph, method } => {
                    if let Some(o) = all.iter().find(|o| o.id == *id) {
                        chosen.push((o, export::Parts { charts: *charts, tables: *tables, checks: *checks, paragraph: *paragraph, method: *method }, std::mem::take(&mut pending)));
                    }
                }
            }
        }
        if chosen.is_empty() {
            return Err(if report { "The report has no results in it yet. Add some on the Report screen.".into() } else { "There are no results to export yet. Run an analysis first.".into() });
        }
        // A reviewed or edited AI draft takes the place of the template paragraph; an unreviewed one never leaves the app.
        let reviewed: Vec<Option<(String, String)>> = chosen.iter().map(|(o, _, _)| ai::reviewed_text(o.ai.as_ref())).collect();
        let items: Vec<export::Item> = chosen
            .iter()
            .zip(&reviewed)
            .map(|((o, parts, before), r)| {
                let (paragraph, ai_model) = match r {
                    Some((text, model)) => (Some(text.as_str()), Some(model.as_str())),
                    None => (o.paragraph.as_deref(), None),
                };
                export::Item { bundle: &o.bundle, paragraph, stale: o.stale, ai_model, images: if parts.charts { pngs(&o.bundle, format) } else { Vec::new() }, parts: *parts, before: before.clone() }
            })
            .collect();
        let source = p.source.as_ref().map(|s| match &s.report.sheet {
            Some(sheet) => format!("{} (sheet {sheet})", s.file_name),
            None => s.file_name.clone(),
        });
        let header = export::Header { title: layout.title.clone(), source, date: today(), after: pending };
        let (bytes, ext) = match format {
            "docx" => (export::docx(&header, &items).map_err(|e| e.to_string())?, "docx"),
            "html" => (export::html(&header, &items).into_bytes(), "html"),
            "pdf" => (prism_pdf::pdf(&header, &items)?, "pdf"),
            other => return Err(format!("\"{other}\" is not an export format. Choose docx, html, pdf or r.")),
        };
        match path {
            Some(path) => {
                let mut path = local_path(path)?;
                if import::extension(&path.to_string_lossy()).as_deref() != Some(ext) {
                    let mut s = path.into_os_string();
                    s.push(format!(".{ext}"));
                    path = PathBuf::from(s);
                }
                self.may_write(&path)?;
                std::fs::write(&path, &bytes).map_err(|e| format!("The file could not be written: {e}"))?;
                Ok(json!({ "path": path.to_string_lossy(), "count": items.len() }))
            }
            None => Ok(json!({ "file_name": format!("{name} output.{ext}"), "base64": base64::engine::general_purpose::STANDARD.encode(&bytes), "count": items.len() })),
        }
    }

    fn summary(&self) -> Result<Value, String> {
        let ds = &self.project()?.dataset;
        let vars: Vec<Value> = ds
            .variables
            .iter()
            .zip(&ds.columns)
            .map(|(v, c)| {
                let (kind, empty) = match c {
                    Column::Numeric(x) => ("numeric", x.iter().filter(|v| v.is_none()).count()),
                    Column::Text(x) => ("text", x.iter().filter(|v| v.is_none()).count()),
                };
                json!({
                    "id": v.id, "name": v.name, "label": v.label, "measure": v.measure, "type": kind,
                    "value_labels": v.value_labels, "missing_values": v.missing_values, "missing_ranges": v.missing_ranges, "empty": empty,
                })
            })
            .collect();
        Ok(json!({ "name": ds.name, "rows": ds.n_rows(), "variables": vars }))
    }

    /// A window of rows for Data View. Each cell is a number, text, or null, plus a flag for user-missing codes.
    fn rows(&self, start: usize, count: usize) -> Result<Value, String> {
        let ds = &self.project()?.dataset;
        let end = (start + count.min(2000)).min(ds.n_rows());
        let rows: Vec<Value> = (start.min(end)..end)
            .map(|r| {
                let cells: Vec<Value> = ds
                    .variables
                    .iter()
                    .zip(&ds.columns)
                    .map(|(v, c)| match c {
                        Column::Numeric(x) => match x[r] {
                            None => Value::Null,
                            Some(n) if v.is_user_missing(n) => json!({ "missing": n }),
                            Some(n) => json!(n),
                        },
                        Column::Text(x) => x[r].as_ref().map_or(Value::Null, |s| json!(s)),
                    })
                    .collect();
                json!(cells)
            })
            .collect();
        Ok(json!({ "start": start, "rows": rows }))
    }

    /// Edit › Find: the cells whose shown text matches, case by case and left to right, ignoring case. Numbers match as
    /// Data View shows them (3, 2.5), user-missing codes included; with `labels`, a value label matches too. `whole`
    /// needs the whole cell to match, otherwise any part does. At most FIND_LIMIT matches are listed.
    fn find(&self, text: &str, variable: Option<&str>, whole: bool, labels: bool) -> Result<Value, String> {
        const FIND_LIMIT: usize = 1000;
        let ds = &self.project()?.dataset;
        let needle = text.trim().to_lowercase();
        if needle.is_empty() {
            return Err("Type what to find.".into());
        }
        let cols: Vec<usize> = match variable {
            Some(n) => vec![ds.find(n).ok_or_else(|| format!("There is no variable named {n}."))?],
            None => (0..ds.variables.len()).collect(),
        };
        let hit = |s: &str| {
            let s = s.to_lowercase();
            if whole { s == needle } else { s.contains(&needle) }
        };
        let mut matches = Vec::new();
        let mut total = 0usize;
        for r in 0..ds.n_rows() {
            for &c in &cols {
                let v = &ds.variables[c];
                let found = match &ds.columns[c] {
                    Column::Numeric(x) => x[r].is_some_and(|n| hit(&format!("{n}")) || (labels && v.value_labels.iter().any(|l| l.value == n && hit(&l.label)))),
                    Column::Text(x) => x[r].as_deref().is_some_and(hit),
                };
                if found {
                    total += 1;
                    if matches.len() < FIND_LIMIT {
                        matches.push(json!({ "row": r, "name": v.name }));
                    }
                }
            }
        }
        Ok(json!({ "matches": matches, "total": total }))
    }

    /// The distinct valid codes of a numeric variable with their labels and counts, for the group picker.
    fn group_codes(&self, name: &str) -> Result<Value, String> {
        let ds = &self.project()?.dataset;
        let var = ds.variable(name).map_err(|e| e.to_string())?;
        let cells = ds.numeric_cells(name).map_err(|e| e.to_string())?;
        let mut codes: Vec<(f64, usize)> = Vec::new();
        for c in cells {
            if let NumericCell::Value(v) = c {
                match codes.iter_mut().find(|(k, _)| *k == v) {
                    Some((_, n)) => *n += 1,
                    None => codes.push((v, 1)),
                }
            }
        }
        codes.sort_by(|a, b| a.0.total_cmp(&b.0));
        if codes.len() > 50 {
            return Err(format!("{} has {} different values, which is too many to be a grouping variable.", var.display_name(), codes.len()));
        }
        Ok(json!(codes.iter().map(|(v, n)| json!({ "code": v, "label": var.value_label(*v), "count": n })).collect::<Vec<_>>()))
    }
}

/// A result table as CSV: a header row, then each row; numbers at full precision so they can be worked with further.
fn table_csv(t: &prism_engine::result::Table) -> String {
    use prism_engine::result::Cell;
    // Text that a spreadsheet would read as a formula (=, +, -, @, tab, CR first) is kept as text with an apostrophe.
    let field = |s: &str| {
        let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{s}") } else { s.to_string() };
        if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
    };
    let mut out = vec![t.columns.iter().map(|c| field(c)).collect::<Vec<_>>().join(",")];
    for row in &t.rows {
        out.push(
            row.iter()
                .map(|c| match c {
                    Cell::Text { text } => field(text),
                    Cell::Number { value, .. } | Cell::P { value } if value.is_finite() => format!("{value}"),
                    Cell::Count { value } => value.to_string(),
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out.join("\r\n") + "\r\n"
}

/// PNGs of a result's charts, drawn only for Word (HTML embeds the SVG, the PDF its outlines).
fn pngs(b: &ResultBundle, format: &str) -> Vec<Vec<u8>> {
    if format != "docx" {
        return Vec::new();
    }
    b.charts.iter().map(|c| prism_pdf::chart_png(&c.svg, 2.0).unwrap_or_default()).collect()
}

/// Runs an analysis the user asked for, as a job the window can follow and cancel. A cancelled run, or one that failed
/// inside the engine, comes back as an error and leaves the project as it was.
fn run_job(ds: &Dataset, command: &str, args: &Value) -> Result<Vec<(ResultBundle, Option<String>)>, String> {
    let job = prism_engine::job::begin(command);
    let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_analysis(ds, command, args)));
    if job.cancelled() {
        return Err("The analysis was cancelled; nothing was added to the output.".into());
    }
    made.unwrap_or_else(|_| Err("The analysis stopped on an internal error, so it gave no result. The data and the output are unchanged; please report the analysis and its settings.".into()))
}

/// Runs `f` on each item, several at a time (one thread per processor), and returns the answers in order. An item
/// whose `f` panics gets None.
fn parallel<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> Option<R> + Sync) -> Vec<Option<R>> {
    let f = |t: &T| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(t))).ok().flatten();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(items.len().max(1));
    if threads <= 1 {
        return items.iter().map(f).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<(usize, Option<R>)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        mine.push((i, f(item)));
                    }
                    mine
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    // Every item has its answer (a panic is caught inside `f`), but go by position all the same.
    out.sort_by_key(|(i, _)| *i);
    let mut answers: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    for (i, r) in out {
        answers[i] = r;
    }
    answers
}

/// Runs one analysis command on a dataset. The same function serves a first run, a rerun and the out-of-date check.
fn run_analysis(ds: &Dataset, command: &str, args: &Value) -> Result<Vec<(ResultBundle, Option<String>)>, String> {
    let settings: prism_engine::subset::DataSettings = args.get("data").map(|d| parse(d.clone())).transpose()?.unwrap_or_default();
    if settings.is_empty() || command == "power" || command == "doe_design" {
        return run_on(ds, command, args);
    }
    let mut out = Vec::new();
    for part in prism_engine::subset::parts(ds, &settings).map_err(|e| e.to_string())? {
        for (mut b, paragraph) in run_on(&part.data, command, args)? {
            if let Some(g) = &part.group {
                b.title = format!("{} ({g})", b.title);
                b.fields.insert("split.group".into(), Field::Text(g.clone()));
            }
            if !part.note.is_empty() {
                b.method_note = format!("{} {}", b.method_note, part.note);
            }
            let paragraph = paragraph.map(|p| match &part.group {
                Some(g) => format!("For the cases with {g}: {p}"),
                None => p,
            });
            out.push((b, paragraph));
        }
    }
    Ok(out)
}

fn run_on(ds: &Dataset, command: &str, args: &Value) -> Result<Vec<(ResultBundle, Option<String>)>, String> {
    fn spec<T: for<'de> Deserialize<'de>>(args: &Value) -> Result<T, String> {
        parse(args.clone())
    }
    let e = |e: prism_engine::Error| e.to_string();
    let bundles: Vec<ResultBundle> = match command {
        "ttest_independent" => ttest_independent::run(ds, &spec(args)?).map_err(e)?,
        "frequencies" => frequencies::run(ds, &spec(args)?).map_err(e)?,
        "descriptives" => vec![descriptives::run(ds, &spec(args)?).map_err(e)?],
        "ttest_one_sample" => ttest_one_sample::run(ds, &spec(args)?).map_err(e)?,
        "ttest_paired" => ttest_paired::run(ds, &spec(args)?).map_err(e)?,
        "anova_oneway" => anova_oneway::run(ds, &spec(args)?).map_err(e)?,
        "crosstabs" => vec![crosstabs::run(ds, &spec(args)?).map_err(e)?],
        "correlation" => correlation::run(ds, &spec(args)?).map_err(e)?,
        "mann_whitney" => rank_tests::mann_whitney(ds, &spec(args)?).map_err(e)?,
        "wilcoxon" => rank_tests::wilcoxon(ds, &spec(args)?).map_err(e)?,
        "kruskal" => rank_tests::kruskal(ds, &spec(args)?).map_err(e)?,
        "regression" => vec![regression::run(ds, &spec(args)?).map_err(e)?],
        "mcnemar" => related::mcnemar(ds, &spec(args)?).map_err(e)?,
        "friedman" => vec![related::friedman(ds, &spec(args)?).map_err(e)?],
        "partial_correlation" => vec![partial_correlation::run(ds, &spec(args)?).map_err(e)?],
        "anova_factorial" => vec![anova_factorial::run(ds, &spec(args)?).map_err(e)?],
        "logistic" => vec![logistic::run(ds, &spec(args)?).map_err(e)?],
        "reliability" => vec![reliability::run(ds, &spec(args)?).map_err(e)?],
        "item_analysis" => vec![item_analysis::run(ds, &spec(args)?).map_err(e)?],
        "kirkpatrick" => vec![kirkpatrick::run(ds, &spec(args)?).map_err(e)?],
        "canonical" => vec![canonical::run(ds, &spec(args)?).map_err(e)?],
        // Power analysis plans a study, so it doesn't read the data.
        "power" => vec![power::run(&spec(args)?).map_err(e)?],
        "chart" => graphs::run(ds, &spec(args)?).map_err(e)?,
        "chisq_gof" => vec![classic_np::goodness_of_fit(ds, &spec(args)?).map_err(e)?],
        "binomial" => classic_np::binomial(ds, &spec(args)?).map_err(e)?,
        "runs" => classic_np::runs(ds, &spec(args)?).map_err(e)?,
        "sign_test" => classic_np::sign(ds, &spec(args)?).map_err(e)?,
        "median_test" => classic_np::median_test(ds, &spec(args)?).map_err(e)?,
        "ks2" => classic_np::ks2(ds, &spec(args)?).map_err(e)?,
        "cochran" => vec![related::cochran(ds, &spec(args)?).map_err(e)?],
        "kappa" => vec![agreement::kappa(ds, &spec(args)?).map_err(e)?],
        "fleiss" => vec![agreement::fleiss(ds, &spec(args)?).map_err(e)?],
        "kripp" => vec![agreement::kripp(ds, &spec(args)?).map_err(e)?],
        "icc" => vec![agreement::icc(ds, &spec(args)?).map_err(e)?],
        "explore" => explore::run(ds, &spec(args)?).map_err(e)?,
        "curve" => curve::run(ds, &spec(args)?).map_err(e)?,
        "bootstrap" => vec![bootstrap::run(ds, &spec(args)?).map_err(e)?],
        "rm_anova" => vec![rm_anova::run(ds, &spec(args)?).map_err(e)?],
        "manova" => vec![manova::run(ds, &spec(args)?).map_err(e)?],
        "factor" => vec![factor::run(ds, &spec(args)?).map_err(e)?],
        "mediation" => vec![process::mediation(ds, &spec(args)?).map_err(e)?],
        "moderation" => vec![process::moderation(ds, &spec(args)?).map_err(e)?],
        "ordinal_regression" => vec![glm_more::ordinal(ds, &spec(args)?).map_err(e)?],
        "multinomial_regression" => vec![glm_more::multinomial(ds, &spec(args)?).map_err(e)?],
        "count_regression" => vec![glm_more::count(ds, &spec(args)?).map_err(e)?],
        "yuen" => trimmed::yuen(ds, &spec(args)?).map_err(e)?,
        "yuen_paired" => trimmed::yuen_paired(ds, &spec(args)?).map_err(e)?,
        "trimmed_anova" => trimmed::t1way(ds, &spec(args)?).map_err(e)?,
        "mixed_model" => vec![mixed::run(ds, &spec(args)?).map_err(e)?],
        "missing_patterns" => vec![missing::patterns(ds, &spec(args)?).map_err(e)?],
        "mi_regression" => vec![missing::mi_regression(ds, &spec(args)?).map_err(e)?],
        "permutation" => vec![permutation::run(ds, &spec(args)?).map_err(e)?],
        "kaplan_meier" => vec![survival::km(ds, &spec(args)?).map_err(e)?],
        "cox_regression" => vec![survival::cox(ds, &spec(args)?).map_err(e)?],
        "roc" => vec![survival::roc(ds, &spec(args)?).map_err(e)?],
        "ts_acf" => vec![timeseries::run_acf(ds, &spec(args)?).map_err(e)?],
        "ts_smooth" => vec![timeseries::run_smooth(ds, &spec(args)?).map_err(e)?],
        "arima" => vec![timeseries::run_arima(ds, &spec(args)?).map_err(e)?],
        "survey_estimates" => vec![survey::estimates(ds, &spec(args)?).map_err(e)?],
        "survey_regression" => vec![survey::regression(ds, &spec(args)?).map_err(e)?],
        "sem" => vec![sem::run(ds, &spec(args)?).map_err(e)?],
        "sem_invariance" => vec![sem::invariance::run(ds, &spec(args)?).map_err(e)?],
        "pls" => vec![pls::run(ds, &spec(args)?).map_err(e)?],
        "lca" => vec![lca::run(ds, &spec(args)?).map_err(e)?],
        "lpa" => vec![lpa::run(ds, &spec(args)?).map_err(e)?],
        "forest" => vec![forest::run(ds, &spec(args)?).map_err(e)?],
        "boost" => vec![boost::run(ds, &spec(args)?).map_err(e)?],
        "meta" => vec![meta::run(ds, &spec(args)?).map_err(e)?],
        "gee" => vec![gee::run(ds, &spec(args)?).map_err(e)?],
        "panel" => vec![panel::run(ds, &spec(args)?).map_err(e)?],
        "pgmm" => vec![dpanel::run(ds, &spec(args)?).map_err(e)?],
        "penalized" => vec![predict::run(ds, &spec(args)?).map_err(e)?],
        "tree" => vec![tree::run(ds, &spec(args)?).map_err(e)?],
        "spc" => vec![spc::run(ds, &spec(args)?).map_err(e)?],
        "doe_design" => vec![doe::run(ds, &spec(args)?).map_err(e)?],
        "doe_effects" => vec![doe::effects(ds, &spec(args)?).map_err(e)?],
        "rsm" => vec![doe::response_surface(ds, &spec(args)?).map_err(e)?],
        "doe_mixture" => vec![doe::mixture(ds, &spec(args)?).map_err(e)?],
        "irt" => vec![irt::run(ds, &spec(args)?).map_err(e)?],
        other => return Err(format!("\"{other}\" is not an analysis.")),
    };
    Ok(bundles
        .into_iter()
        .map(|b| {
            let p = templates::paragraph(&b);
            (b, p)
        })
        .collect())
}

/// True when two results report the same numbers (labels may differ: relabeling never makes a result out of date).
fn same_numbers(a: &ResultBundle, b: &ResultBundle) -> bool {
    if a.status != b.status || a.cases != b.cases {
        return false;
    }
    let numbers = |r: &ResultBundle| r.fields.iter().filter_map(|(k, v)| if let Field::Number(n) = v { Some((k.clone(), *n)) } else { None }).collect::<Vec<_>>();
    let (x, y) = (numbers(a), numbers(b));
    x.len() == y.len() && x.iter().zip(&y).all(|((ka, va), (kb, vb))| ka == kb && (va == vb || (va.is_nan() && vb.is_nan()) || (va - vb).abs() <= 1e-12 * va.abs().max(1.0)))
}

/// Today's date (UTC) as "November 11, 2026", for the document header.
fn today() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    format!("{} {day}, {year}", MONTHS[(month - 1) as usize])
}

fn parse<T: for<'de> Deserialize<'de>>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("Bad arguments: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        let text = "g,y\n1,3\n1,4\n1,6\n2,5\n2,7\n2,9\n9,1\n";
        s.handle("open_text", json!({ "name": "demo.csv", "text": text })).unwrap();
        s.handle("update_variable", json!({ "name": "g", "value_labels": [{ "value": 1.0, "label": "A" }, { "value": 2.0, "label": "B" }], "missing_values": [9.0] })).unwrap();
        s
    }

    #[test]
    fn data_settings_travel_with_each_analysis() {
        let mut s = session();
        s.handle("open_text", json!({ "name": "w.csv", "text": "g,y,w
1,3,1
1,4,2
1,6,1
2,5,1
2,7,3
2,9,1
1,2,0
" })).unwrap();
        let before = s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        assert_eq!(before[0]["bundle"]["cases"]["analyzed"], 7);
        s.handle("set_data_settings", json!({ "filter": "y > 2", "weight": "w", "split": "g" })).unwrap();
        let split = s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        assert_eq!(split.as_array().unwrap().len(), 2);
        assert!(split[0]["bundle"]["title"].as_str().unwrap().ends_with("(g = 1)"));
        // g = 1 keeps y 3, 4 (twice), 6: four weighted cases; y = 2 is filtered out and has weight 0 anyway.
        assert_eq!(split[0]["bundle"]["cases"]["analyzed"], 4);
        assert!(split[0]["paragraph"].as_str().unwrap_or("For the cases with g = 1").starts_with("For the cases with g = 1"));
        // The earlier result keeps its own settings, so it isn't out of date; the new ones aren't either.
        let all = s.handle("results", json!({})).unwrap();
        assert!(all.as_array().unwrap().iter().all(|r| r["stale"] == false), "{all}");
        let script = rscript::script(s.project().unwrap());
        assert!(script.contains("x <- x[with(x, `y` > 2.0) %in% TRUE, ]"), "{script}");
        assert!(script.contains("for (grp in sort(unique(na.omit(x[[\"g\"]]))))"), "{script}");
        // Turning everything off.
        s.handle("set_data_settings", json!({})).unwrap();
        assert_eq!(s.handle("data_settings", json!({})).unwrap(), json!({}));
        assert!(s.handle("set_data_settings", json!({ "weight": "nosuch" })).is_err());
    }

    #[test]
    fn compute_recode_and_scale_are_recorded() {
        let mut s = session();
        let c = s.handle("compute_variable", json!({ "target": "y2", "formula": "y * 2", "label": "Doubled" })).unwrap();
        assert_eq!(c["report"]["valid"], 7);
        // g's code 9 is user-missing, so the formula sees NA there.
        let c = s.handle("compute_variable", json!({ "target": "g10", "formula": "g * 10" })).unwrap();
        assert_eq!(c["report"]["missing"], 1);
        let rc = s.handle("recode", json!({ "variables": ["y"], "rules": [{ "from": { "kind": "range", "hi": 4 }, "to": { "kind": "value", "value": 0 } }, { "from": { "kind": "else" }, "to": { "kind": "value", "value": 1 } }], "into": ["high"] })).unwrap();
        assert_eq!(rc["report"]["names"][0], "high");
        let sc = s.handle("scale_score", json!({ "target": "both", "items": ["y", "y2"], "reverse": ["y"], "scale_min": 0, "scale_max": 10 })).unwrap();
        assert_eq!(sc["report"]["valid"], 7);
        assert!(s.handle("compute_variable", json!({ "target": "y2", "formula": "1" })).unwrap_err().contains("already a variable"));
        // In place: unmatched values stay.
        s.handle("recode", json!({ "variables": ["y"], "rules": [{ "from": { "kind": "value", "value": 3 }, "to": { "kind": "value", "value": 30 } }] })).unwrap();
        let script = rscript::script(s.project().unwrap());
        assert!(script.contains("d[[\"y2\"]] <- with(prep(d)"), "{script}");
        assert!(script.contains("rowSums(!is.na(it)) >= 2"), "{script}");
        assert!(script.contains("hit <- !done & (!miss & old <= 4)"), "{script}");
        // Scripts may use them.
        let ran = script::run(&mut s, "compute_variable {\"target\": \"y3\", \"formula\": \"y + 1\"}");
        assert!(ran["error"].is_null(), "{ran}");
    }

    #[test]
    fn network_paths_are_refused_and_scripts_never_replace_files_unasked() {
        let mut s = session();
        for unc in [r"\\host\share\x.csv", r"\\?\C:\x.csv", r"\\.\C:\x.csv", "//host/share/x.csv", r"\??\C:\x.csv"] {
            for (command, args) in [
                ("open_file", json!({ "path": unc })),
                ("append_cases", json!({ "path": unc })),
                ("merge_variables", json!({ "path": unc, "keys": ["g"] })),
                ("inspect_file", json!({ "path": unc })),
                ("open_project", json!({ "path": unc })),
                ("export", json!({ "format": "html", "path": unc })),
                ("save_project", json!({ "path": unc })),
            ] {
                let e = s.handle(command, args).unwrap_err();
                assert!(e.contains("network or device path"), "{command} {unc}: {e}");
            }
        }
        assert!(s.handle("open_file", json!({ "path": "C:data.csv" })).unwrap_err().contains("no folder after the drive letter"));

        // A script lists what it would write; it replaces an existing file only when the user agreed to that file.
        let dir = std::env::temp_dir().join(format!("prism-script-out-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("report.html");
        std::fs::write(&out, "mine").unwrap();
        let text = format!("descriptives {{\"variables\": [\"y\"]}}\nexport {}", json!({ "format": "html", "path": dir.join("report").to_string_lossy() }));
        let listed = s.handle("script_outputs", json!({ "text": text })).unwrap();
        assert_eq!(listed[0]["path"], out.to_string_lossy().as_ref());
        assert_eq!((listed[0]["line"].as_u64(), listed[0]["exists"].as_bool()), (Some(2), Some(true)));
        let r = s.handle("run_script", json!({ "text": text })).unwrap();
        assert!(r["error"]["message"].as_str().unwrap().contains("already exists"), "{r}");
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "mine", "not replaced");
        let r = s.handle("run_script", json!({ "text": text, "replace": [out.to_string_lossy()] })).unwrap();
        assert!(r["error"].is_null(), "{r}");
        assert!(std::fs::read_to_string(&out).unwrap().contains("<!doctype html>"));
        // From the window itself, an export replaces a file as asked.
        s.handle("export", json!({ "format": "html", "path": out.to_string_lossy() })).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_project_whose_data_dont_add_up_is_refused() {
        let s = session();
        let dir = std::env::temp_dir().join(format!("prism-uneven-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (k, damage) in [
            |p: &mut prism_engine::project::Project| p.dataset.columns[0] = prism_engine::Column::Numeric(vec![Some(1.0)]),
            |p: &mut prism_engine::project::Project| p.dataset.variables[1].name = "g".into(),
            |p: &mut prism_engine::project::Project| p.dataset.variables[1].name = "x\"); system('calc')".into(),
            |p: &mut prism_engine::project::Project| p.dataset.columns.pop().map(|_| ()).unwrap_or(()),
        ]
        .into_iter()
        .enumerate()
        {
            let mut p = s.project.clone().unwrap();
            damage(&mut p);
            let path = dir.join(format!("bad{k}.pstat"));
            std::fs::write(&path, prism_engine::project::to_bytes(&p).unwrap()).unwrap();
            let e = Session::new().handle("open_project", json!({ "path": path.to_string_lossy() })).unwrap_err();
            assert!(e.contains("damaged"), "{k}: {e}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn chart_pictures_from_a_project_file_are_not_trusted() {
        use base64::Engine as _;
        let dir = std::env::temp_dir().join(format!("prism-evil-chart-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let secret = dir.join("secret.png");
        std::fs::write(&secret, b"not really a png").unwrap();
        let mut s = session();
        s.handle("chart", json!({ "kind": "histogram", "variables": ["y"] })).unwrap();
        s.handle("chart", json!({ "kind": "boxplot", "variables": ["y"] })).unwrap();
        {
            let p = s.project.as_mut().unwrap();
            p.results[0].bundle.charts[0].svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><script>alert(1)</script><foreignObject><div xmlns="http://www.w3.org/1999/xhtml">x</div></foreignObject><image href="{}" width="10" height="10"/><image href="\\host\x.png"/><rect width="10" height="10"/></svg>"##,
                secret.display()
            );
            p.results[1].bundle.charts[0].svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="100000"><rect width="10" height="10"/></svg>"#.into();
        }
        let path = dir.join("evil.pstat");
        std::fs::write(&path, prism_engine::project::to_bytes(s.project.as_ref().unwrap()).unwrap()).unwrap();
        let mut t = Session::new();
        t.handle("open_project", json!({ "path": path.to_string_lossy() })).unwrap();
        let results = t.handle("results", json!({})).unwrap();
        let svg = results[0]["bundle"]["charts"][0]["svg"].as_str().unwrap().to_string();
        assert!(svg.contains("<rect") && !svg.contains("script") && !svg.contains("foreignObject") && !svg.contains("image"), "{svg}");
        assert_eq!(results[1]["bundle"]["charts"].as_array().map_or(0, Vec::len), 0, "a chart declaring a huge size is left out");
        for format in ["html", "docx", "pdf"] {
            let out = t.handle("export", json!({ "format": format })).unwrap();
            let bytes = base64::engine::general_purpose::STANDARD.decode(out["base64"].as_str().unwrap()).unwrap();
            let text = if format == "docx" {
                let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
                (0..z.len()).map(|i| { let mut e = z.by_index(i).unwrap(); let mut s = Vec::new(); std::io::Read::read_to_end(&mut e, &mut s).unwrap(); String::from_utf8_lossy(&s).into_owned() }).collect::<String>()
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            };
            for bad in ["<script", "foreignObject", "secret.png", "not really a png", "\\\\host"] {
                assert!(!text.contains(bad), "{format} export holds {bad}");
            }
            if format == "html" {
                assert!(text.contains("Content-Security-Policy"), "html export has a CSP");
            }
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The exported report, as Word and as a web page, against the core's accessibility rules (A11Y-1, tag
    /// a11y-v0.1.0): no rule fails and nothing is left to review. (PDFs are not read by those rules yet; the PDF/UA check is the owner's.)
    #[test]
    fn exported_reports_pass_the_shared_accessibility_rules() {
        use base64::Engine as _;
        let mut s = session();
        s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        s.handle("ttest_independent", json!({ "test_variables": ["y"], "grouping_variable": "g", "groups": [1.0, 2.0] })).unwrap();
        s.handle("chart", json!({ "kind": "histogram", "variables": ["y"] })).unwrap();
        for format in ["html", "docx"] {
            let out = s.handle("export", json!({ "format": format })).unwrap();
            let bytes = base64::engine::general_purpose::STANDARD.decode(out["base64"].as_str().unwrap()).unwrap();
            if let Ok(dir) = std::env::var("PRISM_A11Y_OUT") {
                std::fs::write(std::path::Path::new(&dir).join(format!("report.{format}")), &bytes).unwrap();
            }
            let report = openstage_a11y::check_bytes(&format!("report.{format}"), &bytes, &Default::default()).unwrap();
            let failed: Vec<String> = report.findings.iter().filter(|f| matches!(f.status, openstage_a11y::Status::Fail | openstage_a11y::Status::NeedsReview)).map(|f| format!("{} {:?}: {}", f.rule, f.at, f.message)).collect();
            assert!(failed.is_empty(), "{format} export fails or needs review under A11Y-1:
{}", failed.join("
"));
        }
    }

    #[test]
    fn one_figure_or_table_is_saved_on_its_own() {
        use base64::Engine as _;
        let mut s = session();
        let t = s.handle("ttest_independent", json!({ "test_variables": ["y"], "grouping_variable": "g", "groups": [1.0, 2.0] })).unwrap();
        let id = t[0]["id"].as_u64().unwrap();
        let table = t[0]["bundle"]["tables"][1]["id"].as_str().unwrap().to_string();
        let csv = s.handle("export_item", json!({ "id": id, "item": table, "format": "csv" })).unwrap();
        let text = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(csv["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert!(text.starts_with("Test,t,df,p (two-sided)"), "{text}");
        let t = prism_engine::result::Table {
            id: "x".into(),
            title: "x".into(),
            columns: vec!["=HYPERLINK(\"x\")".into(), "a".into()],
            rows: vec![vec![prism_engine::result::Cell::Text { text: "+cmd|' /C calc'!A0".into() }, prism_engine::result::Cell::Number { value: -2.5, decimals: 2 }], vec![prism_engine::result::Cell::Text { text: "@SUM(1)".into() }, prism_engine::result::Cell::Text { text: "-x".into() }]],
            footnotes: vec![],
        };
        assert_eq!(table_csv(&t), "\"'=HYPERLINK(\"\"x\"\")\",a\r\n'+cmd|' /C calc'!A0,-2.5\r\n'@SUM(1),'-x\r\n", "formulas stay text; numbers stay numbers");
        assert!(text.lines().nth(1).unwrap().split(',').nth(1).unwrap().len() > 6, "numbers at full precision: {text}");
        assert!(csv["file_name"].as_str().unwrap().ends_with(".csv"));
        let h = s.handle("chart", json!({ "kind": "histogram", "variables": ["y"] })).unwrap();
        let (hid, chart) = (h[0]["id"].as_u64().unwrap(), h[0]["bundle"]["charts"][0]["id"].as_str().unwrap().to_string());
        let svg = s.handle("export_item", json!({ "id": hid, "item": chart, "format": "svg" })).unwrap();
        assert!(String::from_utf8(base64::engine::general_purpose::STANDARD.decode(svg["base64"].as_str().unwrap()).unwrap()).unwrap().contains("<svg"));
        let dir = std::env::temp_dir().join(format!("prism-item-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hist.png");
        s.handle("export_item", json!({ "id": hid, "item": chart, "format": "png", "path": path.to_string_lossy() })).unwrap();
        assert_eq!(&std::fs::read(&path).unwrap()[1..4], b"PNG");
        std::fs::remove_dir_all(&dir).ok();
        assert!(s.handle("export_item", json!({ "id": 999, "item": chart, "format": "png" })).is_err());
    }

    #[test]
    fn find_cells_by_value_and_label() {
        let mut s = session();
        let f = s.handle("find", json!({ "text": "1", "whole": true })).unwrap();
        // g is 1 in three cases; y is 1 once (the case whose g is the missing code 9).
        assert_eq!(f["total"], 4);
        assert_eq!(f["matches"][0], json!({ "row": 0, "name": "g" }));
        let f = s.handle("find", json!({ "text": "9", "variable": "g" })).unwrap();
        assert_eq!(f["matches"], json!([{ "row": 6, "name": "g" }]), "user-missing codes are found");
        assert_eq!(s.handle("find", json!({ "text": "b" })).unwrap()["total"], 0, "labels only when asked");
        assert_eq!(s.handle("find", json!({ "text": "b", "labels": true, "whole": true })).unwrap()["total"], 3);
        assert!(s.handle("find", json!({ "text": " " })).is_err());
        assert!(s.handle("find", json!({ "text": "1", "variable": "nosuch" })).is_err());
    }

    #[test]
    fn open_label_and_run() {
        let mut s = session();
        let codes = s.handle("group_codes", json!({ "name": "g" })).unwrap();
        assert_eq!(codes.as_array().unwrap().len(), 2);
        let out = s.handle("ttest_independent", json!({ "test_variables": ["y"], "grouping_variable": "g", "groups": [1.0, 2.0] })).unwrap();
        assert_eq!(out[0]["bundle"]["cases"]["analyzed"], 6);
        assert!(out[0]["paragraph"].as_str().unwrap().contains("A and B groups"));
        let rows = s.handle("rows", json!({ "start": 6, "count": 10 })).unwrap();
        assert_eq!(rows["rows"][0][0]["missing"], 9.0);
        assert!(s.handle("nope", json!({})).is_err());
        let f = s.handle("frequencies", json!({ "variables": ["g"] })).unwrap();
        let d = s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        let ids = json!([f[0]["id"], d[0]["id"]]);
        let p = s.handle("participants", json!({ "ids": ids })).unwrap();
        assert!(p["paragraph"].as_str().unwrap().starts_with("The sample included 7 participants. For g, 42.9% A (*n* = 3)"), "{p}");
    }

    #[test]
    fn every_preview_analysis_runs_and_reruns() {
        let mut s = session();
        let calls = [
            ("ttest_one_sample", json!({ "test_variables": ["y"], "test_value": 5 })),
            ("ttest_paired", json!({ "pairs": [["g", "y"]] })),
            ("anova_oneway", json!({ "dependent": ["y"], "factor": "g", "post_hoc": "tukey" })),
            ("crosstabs", json!({ "row": "g", "column": "g2" })),
            ("correlation", json!({ "variables": ["g", "y"] })),
            ("mann_whitney", json!({ "test_variables": ["y"], "grouping_variable": "g", "groups": [1.0, 2.0] })),
            ("wilcoxon", json!({ "pairs": [["g", "y"]] })),
            ("kruskal", json!({ "dependent": ["y"], "factor": "g", "post_hoc": "dunn" })),
            ("regression", json!({ "dependent": "y", "factors": [{ "name": "g" }] })),
            ("crosstabs", json!({ "row": "g", "column": "g2", "fisher": true })),
            ("mcnemar", json!({ "pairs": [["g", "g2"]] })),
            ("friedman", json!({ "variables": ["g", "g2", "y"] })),
            ("partial_correlation", json!({ "variables": ["g", "y"], "controls": ["g2"] })),
            ("anova_factorial", json!({ "dependent": "y", "factors": ["g", "g2"] })),
            ("logistic", json!({ "dependent": "g", "covariates": ["y"] })),
            ("reliability", json!({ "items": ["g", "g2", "y"], "reverse": ["g2"] })),
            ("item_analysis", json!({ "items": ["g", "g2", "y"], "scored": true })),
            ("kirkpatrick", json!({ "reaction": ["g", "g2"], "learning": [{ "pre": "g", "post": "y" }] })),
            ("canonical", json!({ "set1": ["g", "g2"], "set2": ["y"] })),
            ("chisq_gof", json!({ "variable": "g2" })),
            ("binomial", json!({ "variables": ["g2"] })),
            ("runs", json!({ "variables": ["y"] })),
            ("sign_test", json!({ "pairs": [["g", "y"]] })),
            ("median_test", json!({ "dependent": ["y"], "factor": "g2" })),
            ("ks2", json!({ "test_variables": ["y"], "grouping_variable": "g", "groups": [1.0, 2.0] })),
            ("cochran", json!({ "variables": ["g", "g2"] })),
            ("kappa", json!({ "rater1": "g", "rater2": "g2" })),
            ("fleiss", json!({ "raters": ["g", "g2"] })),
            ("kripp", json!({ "raters": ["g", "g2", "y"], "metric": "interval" })),
            ("icc", json!({ "raters": ["g", "g2", "y"] })),
            ("explore", json!({ "dependent": ["y"], "factor": "g" })),
            ("kaplan_meier", json!({ "time": "y", "status": "g", "events": [2], "group": "g2" })),
            ("sem", json!({ "model": "y ~ g + g2" })),
            ("survey_estimates", json!({ "variables": ["y"], "weight": "g" })),
            ("ts_acf", json!({ "variable": "y" })),
            ("arima", json!({ "variable": "y", "p": 1 })),
            ("roc", json!({ "variables": ["y"], "state": "g", "positive": 2 })),
            ("permutation", json!({ "test": "anova", "variables": ["y"], "group": "g", "resamples": 500 })),
            ("missing_patterns", json!({ "variables": ["g", "y"] })),
            ("yuen", json!({ "test_variables": ["y"], "grouping_variable": "g", "groups": [1.0, 2.0] })),
            ("trimmed_anova", json!({ "dependent": ["y"], "factor": "g" })),
            ("count_regression", json!({ "dependent": "g", "covariates": ["y"] })),
            ("gee", json!({ "dependent": "y", "covariates": ["g2"], "cluster": "g" })),
            ("penalized", json!({ "dependent": "y", "covariates": ["g", "g2"], "folds": 3 })),
            ("tree", json!({ "dependent": "y", "predictors": ["g", "g2"], "minsplit": 4, "folds": 3 })),
            ("spc", json!({ "chart": "imr", "measures": ["y"] })),
            ("mediation", json!({ "x": "g", "mediators": ["g2"], "y": "y", "resamples": 200 })),
            ("moderation", json!({ "x": "g", "w": "g2", "y": "y" })),
            ("factor", json!({ "variables": ["y", "g", "g2"], "extraction": "pca", "factors": 1 })),
            ("manova", json!({ "dependent": ["y", "g2"], "factor": "g" })),
            ("rm_anova", json!({ "measures": ["g", "g2", "y"], "post_hoc": "holm" })),
            ("bootstrap", json!({ "statistic": "median", "variables": ["y"], "resamples": 200 })),
            ("curve", json!({ "dependent": ["y"], "independent": "g", "models": ["linear", "quadratic"] })),
            ("crosstabs", json!({ "row": "g", "column": "g2", "risk": true, "ordinal": true })),
            ("chart", json!({ "kind": "histogram", "variables": ["y"] })),
            ("chart", json!({ "kind": "boxplot", "variables": ["y"], "group": "g" })),
            ("chart", json!({ "kind": "bar", "variables": ["g"] })),
            ("chart", json!({ "kind": "scatter", "x": "g2", "y": "y" })),
            ("chart", json!({ "kind": "qq", "variables": ["y"] })),
            ("regression", json!({ "dependent": "y", "covariates": ["g2"], "factors": [], "plots": true })),
        ];
        s.handle("open_text", json!({ "name": "demo.csv", "text": "g,g2,y
1,1,3
1,2,4
1,1,6
2,2,5
2,1,7
2,2,9
2,2,8
" })).unwrap();
        for (command, args) in calls {
            let out = s.handle(command, args).unwrap_or_else(|e| panic!("{command}: {e}"));
            assert_eq!(out[0]["bundle"]["status"]["state"], "ok", "{command}: {out}");
            assert!(command == "chart" || out[0]["paragraph"].is_string(), "{command} has no paragraph");
        }
        let all = s.handle("results", json!({})).unwrap();
        assert!(all.as_array().unwrap().iter().all(|r| r["stale"] == false));
        // Every picture the engine draws comes through the cleaning of pictures read from a project, element for element.
        let mut pictures = 0;
        for r in all.as_array().unwrap() {
            for c in r["bundle"]["charts"].as_array().into_iter().flatten() {
                let svg = c["svg"].as_str().unwrap();
                let clean = prism_engine::svg_clean::clean(svg).unwrap_or_else(|| panic!("{} was refused", r["bundle"]["title"]));
                for tag in ["<rect", "<line", "<text", "<path", "<polyline", "<polygon", "<circle", "<ellipse", "<g", "<marker", "data-param", "marker-end"] {
                    assert_eq!(svg.matches(tag).count(), clean.matches(tag).count(), "{tag} in {}", r["bundle"]["title"]);
                }
                pictures += 1;
            }
        }
        assert!(pictures >= 8, "{pictures} pictures");
        let again = s.handle("rerun", json!({ "id": all[2]["id"] })).unwrap();
        assert_eq!(again.as_array().unwrap().len(), all.as_array().unwrap().len());
    }

    #[test]
    fn a_design_opens_as_data_with_its_summary() {
        let mut s = Session::default();
        let reply = s
            .handle("create_design", json!({ "kind": "fractional", "runs": 8, "factors": [{ "name": "temp", "low": 150.0, "high": 180.0 }, { "name": "time" }, { "name": "conc" }, { "name": "speed" }, { "name": "rate" }], "randomize": true, "seed": 3, "response": "yield" }))
            .unwrap();
        assert_eq!(reply["dataset"]["rows"], 8);
        let names: Vec<&str> = reply["dataset"]["variables"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["run", "std_order", "temp", "time", "conc", "speed", "rate", "yield"]);
        assert_eq!(reply["outputs"][0]["bundle"]["fields"]["resolution"], 3.0);
        assert!(reply["outputs"][0]["bundle"]["fields"]["alias.main"].as_str().unwrap().starts_with("temp="));
        // The temperature column holds the actual settings.
        let rows = s.handle("rows", json!({ "start": 0, "count": 8 })).unwrap();
        assert!(rows.to_string().contains("150") && rows.to_string().contains("180"), "{rows}");
    }

    #[test]
    fn power_runs_without_data_and_moves_into_the_opened_data() {
        let mut s = Session::default();
        let out = s.handle("power", json!({ "test": "independent", "effect_size": 0.5 })).unwrap();
        assert_eq!(out[0]["bundle"]["fields"]["n"], 64.0);
        assert_eq!(s.state()["has_data"], false);
        s.handle("open_text", json!({ "name": "demo.csv", "text": "g,y
1,3
2,4
1,5
" })).unwrap();
        assert_eq!(s.state()["has_data"], true);
        let all = s.handle("results", json!({})).unwrap();
        assert_eq!(all.as_array().unwrap().len(), 1);
        assert_eq!(all[0]["stale"], false);
        let again = s.handle("rerun", json!({ "id": all[0]["id"] })).unwrap();
        assert_eq!(again[0]["bundle"]["method_id"], "power");
    }

    #[test]
    fn editing_a_cell_is_recorded_and_makes_results_out_of_date() {
        let mut s = session();
        s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        let got = s.handle("set_cell", json!({ "row": 0, "name": "y", "value": "100" })).unwrap();
        assert_eq!(got["rows"][0][s.handle("dataset", json!({})).unwrap()["variables"].as_array().unwrap().iter().position(|v| v["name"] == "y").unwrap()], 100.0);
        assert_eq!(s.handle("results", json!({})).unwrap()[0]["stale"], true);
        assert!(s.handle("set_cell", json!({ "row": 0, "name": "y", "value": "abc" })).unwrap_err().contains("not a number"));
        assert!(s.handle("set_cell", json!({ "row": 999, "name": "y", "value": "1" })).unwrap_err().contains("no case 1000"));
        s.handle("set_cell", json!({ "row": 0, "name": "y", "value": "" })).unwrap();
        let ops = &s.project.as_ref().unwrap().operations;
        assert_eq!(ops.iter().filter(|o| o.command == "set_cell").count(), 2);
    }

    #[test]
    fn every_chart_draws_and_exports() {
        let mut s = session();
        let calls = [
            json!({ "kind": "histogram", "variables": ["y"], "normal": true }),
            json!({ "kind": "boxplot", "variables": ["y"], "group": "g" }),
            json!({ "kind": "boxplot", "variables": ["y", "g"] }),
            json!({ "kind": "bar", "variables": ["g"] }),
            json!({ "kind": "bar", "variables": ["g"], "bar_stat": "mean", "mean_of": "y" }),
            json!({ "kind": "scatter", "x": "g", "y": "y", "line": true }),
            json!({ "kind": "qq", "variables": ["y"] }),
        ];
        for args in calls {
            let out = s.handle("chart", args.clone()).unwrap_or_else(|e| panic!("{args}: {e}"));
            let b = &out[0]["bundle"];
            assert_eq!(b["status"]["state"], "ok", "{args}: {b}");
            let svg = b["charts"][0]["svg"].as_str().unwrap();
            assert!(svg.starts_with("<svg") && svg.contains("role=\"img\""), "{args}");
            assert!(!b["charts"][0]["alt"].as_str().unwrap().is_empty());
        }
        let h = s.handle("export", json!({ "format": "html" })).unwrap();
        let html = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(h["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert!(html.contains("<b>Figure 7</b>") && html.contains("<svg"), "html figures");
        let d = s.handle("export", json!({ "format": "docx" })).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(d["base64"].as_str().unwrap()).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert!(zip.by_name("word/media/figure7.png").is_ok(), "Word has the seventh chart as a PNG");
        let mut doc = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut doc).unwrap();
        assert!(doc.contains("descr=\"Histogram of y"), "Word pictures carry the alt text");
        let p = s.handle("export", json!({ "format": "pdf" })).unwrap();
        assert!(p["base64"].as_str().unwrap().len() > 1000);
    }

    #[test]
    fn the_report_builder_layout_shapes_the_export() {
        let mut s = session();
        let t = s.handle("ttest_independent", json!({ "test_variables": ["y"], "grouping_variable": "g", "groups": [1.0, 2.0] })).unwrap();
        let d = s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        let (tid, did) = (t[0]["id"].as_u64().unwrap(), d[0]["id"].as_u64().unwrap());
        // With nothing arranged, the report is every result in order.
        let r = s.handle("report_get", json!({})).unwrap();
        assert_eq!(r["blocks"].as_array().unwrap().len(), 2);
        s.handle("report_set", json!({ "title": "Exam study", "blocks": [
            { "kind": "heading", "text": "Sample" },
            { "kind": "result", "id": did, "checks": false, "method": false },
            { "kind": "text", "text": "Then the *main* test." },
            { "kind": "result", "id": tid, "tables": false },
            { "kind": "text", "text": "Closing words." },
        ] })).unwrap();
        let h = s.handle("export", json!({ "format": "html", "report": true })).unwrap();
        let html = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(h["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert!(html.contains("<title>Exam study</title>"));
        let (sample, desc, main, closing) = (html.find(">Sample<").unwrap(), html.find("Descriptives").unwrap(), html.find("<i>main</i>").unwrap(), html.find("Closing words.").unwrap());
        assert!(sample < desc && desc < main && main < closing, "blocks in order");
        assert_eq!(html.matches("<b>Method.</b>").count(), 1, "the descriptives method note is left out");
        assert_eq!(html.matches("<table").count(), 1, "the t test's tables are left out");
        // Running the t test again keeps it in the report.
        s.handle("rerun", json!({ "id": tid })).unwrap();
        let r = s.handle("report_get", json!({})).unwrap();
        assert_eq!(r["blocks"].as_array().unwrap().iter().filter(|b| b["kind"] == "result").count(), 2);
        assert!(s.handle("export", json!({ "format": "pdf", "report": true })).is_ok());
        assert!(s.handle("export", json!({ "format": "docx", "report": true })).is_ok());
    }

    #[test]
    fn a_sas_file_says_where_its_value_labels_are() {
        let mut s = Session::new();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../prism-engine/tests/reference/statfile.sas7bdat");
        let opened = s.handle("open_file", json!({ "path": path })).unwrap();
        let notes: Vec<&str> = opened["report"]["notes"].as_array().unwrap().iter().filter_map(|n| n.as_str()).collect();
        assert!(notes.iter().any(|n| n.contains("SAS value labels")), "{notes:?}");
        let err = s.handle("load_sas_catalog", json!({ "file_name": "x.sas7bcat", "base64": "bm90IGEgY2F0YWxvZw==" })).unwrap_err();
        assert!(err.contains(".sas7bcat"), "{err}");
    }

    #[test]
    fn regression_values_are_saved_as_variables() {
        let mut s = session();
        let out = s.handle("regression", json!({ "dependent": "y", "factors": [{ "name": "g" }], "plots": true })).unwrap();
        let id = out[0]["id"].clone();
        assert_eq!(out[0]["bundle"]["charts"].as_array().unwrap().len(), 3);
        let r = s.handle("save_regression", json!({ "id": id })).unwrap();
        assert_eq!(r["names"], json!(["PRE_1", "RES_1", "ZRE_1", "COO_1", "LEV_1"]));
        let again = s.handle("save_regression", json!({ "id": id })).unwrap();
        assert_eq!(again["names"][0], "PRE_2");
        // The user-missing case (g = 9) was not in the model, so its saved values are empty.
        let rows = s.handle("rows", json!({ "start": 6, "count": 1 })).unwrap();
        let names: Vec<String> = s.handle("dataset", json!({})).unwrap()["variables"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap().to_string()).collect();
        assert!(rows["rows"][0][names.iter().position(|n| n == "PRE_1").unwrap()].is_null());
    }

    #[test]
    fn exports_word_and_html() {
        let mut s = session();
        assert!(s.handle("export", json!({ "format": "html" })).unwrap_err().contains("no results"));
        s.handle("ttest_independent", json!({ "test_variables": ["y"], "grouping_variable": "g", "groups": [1.0, 2.0] })).unwrap();
        s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        let h = s.handle("export", json!({ "format": "html" })).unwrap();
        let html = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(h["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert!(html.contains("<b>Table 1</b>") && html.contains("<i>t</i>("), "{html}");
        assert_eq!(h["file_name"], "demo output.html");
        let p = s.handle("export", json!({ "format": "pdf" })).unwrap();
        let pdf = base64::engine::general_purpose::STANDARD.decode(p["base64"].as_str().unwrap()).unwrap();
        assert!(pdf.starts_with(b"%PDF-") && p["file_name"] == "demo output.pdf");
        let dir = std::env::temp_dir().join(format!("prism-export-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let w = s.handle("export", json!({ "format": "docx", "path": dir.join("report").to_string_lossy(), "ids": [1] })).unwrap();
        assert_eq!(w["count"], 1);
        let bytes = std::fs::read(w["path"].as_str().unwrap()).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut doc = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut doc).unwrap();
        assert!(doc.contains("Independent-Samples T Test") && !doc.contains("Descriptives"));
        assert!(today().contains(", 20"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_openassess_results_file_offers_item_analysis_and_keeps_its_profile() {
        let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prism-engine/tests/reference/c3-item-analysis/cases/small-ties/small-ties.pstat");
        let dir = std::env::temp_dir().join(format!("prism-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("small-ties.pstat");
        std::fs::copy(&src, &path).unwrap();
        let mut s = Session::new();
        let r = s.handle("open_project", json!({ "path": path.to_string_lossy() })).unwrap();
        let offer = &r["profile"];
        assert!(offer["about"].as_str().unwrap().contains("6 questions"), "{offer}");
        let first = &offer["suggestions"][0];
        assert_eq!(first["dialog"], "item_analysis");
        // The offered settings run as they stand.
        let out = s.handle("item_analysis", first["args"].clone()).unwrap();
        assert!(out.to_string().contains("Corrected item-total r"), "{out}");
        s.handle("save_project", json!({ "path": path.to_string_lossy() })).unwrap();
        let again = s.handle("open_project", json!({ "path": path.to_string_lossy() })).unwrap();
        assert!(again["profile"].is_object(), "the profile is kept when the project is saved again");
        // A project of PrismStatistics's own offers nothing.
        let mut t = Session::new();
        t.handle("open_text", json!({ "name": "a.csv", "text": "q01,q02\n1,0\n0,1\n1,1\n" })).unwrap();
        let own = dir.join("own.pstat");
        t.handle("save_project", json!({ "path": own.to_string_lossy() })).unwrap();
        assert!(t.handle("open_project", json!({ "path": own.to_string_lossy() })).unwrap().get("profile").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_case_table_project_merges_onto_survey_data_by_case() {
        // A case table as a qualitative program would send it: one row per case, its id, and each code's presence and
        // count. Saved as a project, then added to survey data by the case id.
        let dir = std::env::temp_dir().join(format!("prism-casetable-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("cases.pstat");
        let mut q = Session::new();
        q.handle("open_text", json!({ "name": "cases.csv", "text": "case_id,code_barriers_present,code_barriers_count\n2,1,3\n1,0,0\n3,1,1\n" })).unwrap();
        q.handle("save_project", json!({ "path": table.to_string_lossy() })).unwrap();
        let mut s = Session::new();
        s.handle("open_text", json!({ "name": "survey.csv", "text": "case_id,satisfaction\n1,4\n2,2\n3,5\n4,3\n" })).unwrap();
        let r = s.handle("merge_variables", json!({ "path": table.to_string_lossy(), "keys": ["case_id"] })).unwrap();
        assert!(r.to_string().contains("code_barriers_count"), "{r}");
        let rows = s.handle("rows", json!({ "start": 0, "count": 4 })).unwrap().to_string();
        assert!(rows.contains("3"), "{rows}");
        let ds = &s.project.as_ref().unwrap().dataset;
        assert_eq!(ds.variables.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["case_id", "satisfaction", "code_barriers_present", "code_barriers_count"]);
        assert_eq!(ds.columns[3], prism_engine::model::Column::Numeric(vec![Some(0.0), Some(3.0), Some(1.0), None]), "matched by case, not by row");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_dataset_exports_as_csv_and_sav_and_reads_back() {
        let mut s = Session::new();
        s.handle("open_text", json!({ "name": "d.csv", "text": "g,score,note\n1,3.25,\"a, b\"\n2,,x\n9,4,\n" })).unwrap();
        s.handle("update_variable", json!({ "name": "g", "label": "Group", "measure": "nominal", "value_labels": [{ "value": 1, "label": "Control" }, { "value": 2, "label": "Treatment" }], "missing_values": [9] })).unwrap();
        let dir = std::env::temp_dir().join(format!("prism-export-data-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("d.csv");
        s.handle("export_data", json!({ "format": "csv", "path": csv.to_string_lossy() })).unwrap();
        assert_eq!(std::fs::read_to_string(&csv).unwrap(), "g,score,note\r\n1,3.25,\"a, b\"\r\n2,,x\r\n9,4,\r\n");
        let labelled = s.handle("export_data", json!({ "format": "csv_labels" })).unwrap();
        let text = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(labelled["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert!(text.contains("Control,3.25") && text.contains("Treatment,,x"), "{text}");
        for (fmt, ext) in [("sav", "sav"), ("zsav", "zsav")] {
            let path = dir.join(format!("d.{ext}"));
            s.handle("export_data", json!({ "format": fmt, "path": path.to_string_lossy() })).unwrap();
            let (back, _) = prism_engine::import::read_file(&path).unwrap();
            let orig = &s.project.as_ref().unwrap().dataset;
            assert_eq!(back.columns[0], orig.columns[0], "{fmt}: codes kept");
            assert_eq!(back.columns[1], orig.columns[1], "{fmt}: values kept exactly");
            let g = back.variable("g").unwrap();
            assert_eq!((g.label.as_str(), g.value_labels.len(), g.missing_values.clone()), ("Group", 2, vec![9.0]), "{fmt}: codebook kept");
            assert_eq!(back.columns[2], prism_engine::model::Column::Text(vec![Some("a, b".into()), Some("x".into()), None]), "{fmt}: text kept");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn profile_2_and_case_table_files_offer_their_analyses_and_keep_their_parts() {
        let dir = std::env::temp_dir().join(format!("prism-profiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reference = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prism-engine/tests/reference");
        // OpenAssess results with the letters and the key: the first offer scores the letters with the file's key.
        let path = dir.join("letters-60.pstat");
        std::fs::copy(reference.join("c3-item-analysis/cases/letters-60/letters-60.pstat"), &path).unwrap();
        let mut s = Session::new();
        let r = s.handle("open_project", json!({ "path": path.to_string_lossy() })).unwrap();
        let offer = &r["profile"];
        assert!(offer["about"].as_str().unwrap().contains("the letters marked and the key"), "{offer}");
        let first = &offer["suggestions"][0];
        assert_eq!(first["args"]["scored"], false);
        assert_eq!(first["args"]["items"][0], "r01");
        assert_eq!(first["args"]["key"][7]["correct"], json!(["B", "E"]), "q08 accepts B or E");
        let out = s.handle("item_analysis", first["args"].clone()).unwrap();
        assert!(out.to_string().contains("Options by score group"), "{out}");
        // Saving keeps assessment.json (rule Z4), so the offer is still there after reopening.
        s.handle("save_project", json!({ "path": path.to_string_lossy() })).unwrap();
        let again = Session::new().handle("open_project", json!({ "path": path.to_string_lossy() })).unwrap();
        assert_eq!(again["profile"]["suggestions"][0]["args"]["scored"], false);
        // A PrismQDA case table: how often each code appears, and where to match it with survey data.
        let table = dir.join("case-table.pstat");
        std::fs::copy(reference.join("pstat-examples/case-table.pstat"), &table).unwrap();
        let r = Session::new().handle("open_project", json!({ "path": table.to_string_lossy() })).unwrap();
        let offer = &r["profile"];
        assert!(offer["about"].as_str().unwrap().contains("case_id"), "{offer}");
        assert_eq!(offer["suggestions"][0]["dialog"], "frequencies");
        assert_eq!(offer["suggestions"][0]["args"]["variables"], json!(["code_12_present", "code_13_present", "code_20_present"]));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn students_ids_and_names_open_but_stay_out_of_the_results() {
        // PSTAT-1 revision 1.2: profile 2 may carry student_id and student_name after the r variables. A file with
        // both still gets its offers, its analyses never mention a student, and the ids can match two administrations.
        let reference = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prism-engine/tests/reference");
        let mut p = prism_engine::project::from_bytes(&std::fs::read(reference.join("c3-item-analysis/cases/letters-60/letters-60.pstat")).unwrap()).unwrap();
        let n = p.dataset.n_rows();
        let ids: Vec<Option<String>> = (0..n).map(|i| Some(format!("S{:04}", 1000 + i))).collect();
        let names: Vec<Option<String>> = (0..n).map(|i| Some(format!("Student Name{i}"))).collect();
        p.dataset.add_variable("student_id", prism_engine::model::Column::Text(ids)).unwrap();
        p.dataset.add_variable("student_name", prism_engine::model::Column::Text(names)).unwrap();
        let dir = std::env::temp_dir().join(format!("prism-ids-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("with-ids.pstat");
        std::fs::write(&path, prism_engine::project::to_bytes(&p).unwrap()).unwrap();
        let mut s = Session::new();
        let r = s.handle("open_project", json!({ "path": path.to_string_lossy() })).unwrap();
        let offers = r["profile"]["suggestions"].as_array().unwrap().clone();
        assert!(offers.len() >= 4, "{offers:?}");
        for o in &offers {
            assert!(!o["args"].to_string().contains("student"), "no offer reads the students: {o}");
            let out = s.handle(o["dialog"].as_str().unwrap(), o["args"].clone()).unwrap();
            let text = out.to_string();
            assert!(!text.contains("Student Name") && !text.contains("S1000"), "{} shows no student", o["label"]);
        }
        // Two administrations matched on student_id: Add Variables brings the second file's points beside the first.
        let second = dir.join("second.pstat");
        std::fs::copy(&path, &second).unwrap();
        s.handle("merge_variables", json!({ "path": second.to_string_lossy(), "keys": ["student_id"] })).unwrap();
        let ds = &s.project.as_ref().unwrap().dataset;
        assert!(ds.find("total_2").is_some(), "the second file's total arrives as total_2");
        let k = s.handle("kirkpatrick", json!({ "learning": [{ "pre": "total", "post": "total_2" }], "learning_max": 11 })).unwrap();
        assert!(k.to_string().contains("learning.1.n"), "{k}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_reopen_and_mark_out_of_date() {
        let dir = std::env::temp_dir().join(format!("prism-api-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = session();
        s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        assert_eq!(s.handle("project_state", json!({})).unwrap()["dirty"], true);
        let saved = s.handle("save_project", json!({ "path": dir.join("demo").to_string_lossy() })).unwrap();
        assert!(saved["path"].as_str().unwrap().ends_with("demo.pstat"));
        assert_eq!(saved["dirty"], false);

        let mut t = Session::new();
        let opened = t.handle("open_project", json!({ "path": saved["path"] })).unwrap();
        assert_eq!(opened["dataset"]["rows"], 7);
        assert_eq!(opened["results"][0]["stale"], false);
        let ops = &t.project.as_ref().unwrap().operations;
        assert_eq!(ops.iter().map(|o| o.command.as_str()).collect::<Vec<_>>(), ["import", "update_variable", "descriptives"]);

        // Relabeling doesn't change any numbers; a new missing code does.
        t.handle("update_variable", json!({ "name": "y", "label": "Outcome" })).unwrap();
        assert_eq!(t.handle("results", json!({})).unwrap()[0]["stale"], false);
        t.handle("update_variable", json!({ "name": "y", "missing_values": [9.0] })).unwrap();
        let r = t.handle("results", json!({})).unwrap();
        assert_eq!(r[0]["stale"], true);
        let again = t.handle("rerun", json!({ "id": r[0]["id"] })).unwrap();
        assert_eq!(again.as_array().unwrap().len(), 1);
        assert_eq!(again[0]["stale"], false);
        assert_eq!(again[0]["bundle"]["fields"]["y.n"], 6.0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn workbooks_with_several_sheets_ask_which_one() {
        let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
        let wb = include_bytes!("../../prism-engine/tests/reference/two_sheets.xlsx");
        let mut s = Session::new();
        let first = s.handle("open_bytes", json!({ "file_name": "study.xlsx", "base64": b64(wb) })).unwrap();
        assert_eq!(first["choose_sheet"], json!(["Survey", "Codebook"]));
        assert!(s.project.is_none(), "nothing opens until a sheet is chosen");
        let r = s.handle("open_bytes", json!({ "file_name": "study.xlsx", "base64": b64(wb), "sheet": "Survey" })).unwrap();
        assert_eq!(r["dataset"]["rows"], 4);
        assert_eq!(r["report"]["other_sheets"], json!(["Codebook"]));
        let rows = s.handle("rows", json!({ "start": 0, "count": 4 })).unwrap();
        assert_eq!(rows["rows"][0], json!([1.0, 1.0, 4.5, "2026-01-05"]));
        assert_eq!(rows["rows"][2][2], Value::Null);
        assert!(s.handle("open_bytes", json!({ "file_name": "x.xlsx", "base64": b64(b"not a workbook") })).is_err());
        let csv = s.handle("open_bytes", json!({ "file_name": "x.csv", "base64": b64(b"a,b
1,2
") })).unwrap();
        assert_eq!(csv["dataset"]["rows"], 1);
    }

    #[test]
    fn out_of_date_checks_are_kept_until_what_an_analysis_reads_changes() {
        let mut s = session();
        s.handle("compute_variable", json!({ "target": "z", "formula": "y * 2" })).unwrap();
        let made = s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        let id = made[0]["id"].clone();
        let stale = |s: &mut Session| s.handle("results", json!({})).unwrap()[0]["stale"].as_bool().unwrap();
        // Another variable's values: still up to date.
        s.handle("set_cell", json!({ "row": 0, "name": "z", "value": "50" })).unwrap();
        assert!(!stale(&mut s));
        // Its own values: out of date; the old value back: up to date again.
        s.handle("set_cell", json!({ "row": 0, "name": "y", "value": "100" })).unwrap();
        assert!(stale(&mut s));
        s.handle("set_cell", json!({ "row": 0, "name": "y", "value": "3" })).unwrap();
        assert!(!stale(&mut s));

        // A slow analysis is marked out of date without being run again, and Undo puts it back.
        let seq = s.project.as_ref().unwrap().results[0].operation;
        s.recheck.lock().unwrap().took(seq, 2.0 * recheck::SLOW);
        s.handle("set_cell", json!({ "row": 1, "name": "y", "value": "4" })).unwrap();
        assert!(!stale(&mut s), "the same value is no change");
        s.handle("set_cell", json!({ "row": 1, "name": "y", "value": "40" })).unwrap();
        assert!(stale(&mut s));
        s.handle("undo", json!({})).unwrap();
        assert!(!stale(&mut s));

        // Saved with its check: opened again, it is up to date without running, and still checked against changes.
        let dir = std::env::temp_dir().join(format!("prism-api-recheck-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let saved = s.handle("save_project", json!({ "path": dir.join("kept").to_string_lossy() })).unwrap();
        let p = project::load(Path::new(saved["path"].as_str().unwrap())).unwrap();
        assert!(p.results[0].checked.is_some(), "the result keeps its check");
        let mut t = Session::new();
        let opened = t.handle("open_project", json!({ "path": saved["path"] })).unwrap();
        assert_eq!(opened["results"][0]["stale"], false);
        assert_eq!(opened["results"][0]["id"], id);
        t.handle("set_cell", json!({ "row": 2, "name": "y", "value": "60" })).unwrap();
        assert!(stale(&mut t));
        // Saved while out of date: opened again, still out of date.
        let saved = t.handle("save_project", json!({ "path": dir.join("kept").to_string_lossy() })).unwrap();
        let mut u = Session::new();
        assert_eq!(u.handle("open_project", json!({ "path": saved["path"] })).unwrap()["results"][0]["stale"], true);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
