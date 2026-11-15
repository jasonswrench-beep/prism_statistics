//! Edit › Undo and Redo for the changes a user makes to the data and the output: editing cells and variables, pasting,
//! the Data and Transform commands, Data Health fixes, case selection, saving regression values, scripts, and clearing
//! the output.
//!
//! Each undoable command keeps a copy of the project from before it ran. Undoing it puts back the data, the data
//! settings and the report layout from that copy, and takes the command's own operations out of the project's record,
//! so the history (and a script made from it) never repeats an undone change. Results the user ran after the change
//! stay: they are checked against the restored data like any other result and marked out of date if their numbers no
//! longer match. Undoing a script also removes the results the script produced. Opening other data or another project
//! clears both lists.

use prism_engine::project::Project;

use crate::Session;

/// How many changes can be undone.
const DEPTH: usize = 30;

pub(crate) struct Step {
    /// What the command did, as Edit › Undo names it ("Recode", "Edit cell").
    label: String,
    /// The project as the step leaves it to go back (for an undo, the project before the change).
    snapshot: Project,
    /// Operations to take out of the record, and to put back from the snapshot.
    remove: Vec<u32>,
    restore: Vec<u32>,
    /// Clearing the output: the results come back from the snapshot as they were.
    results: bool,
}

/// The name Edit › Undo shows for a command, or None when the command can't be undone (it changes nothing, or it opens
/// other data).
pub(crate) fn label(command: &str) -> Option<&'static str> {
    Some(match command {
        "set_cell" => "Edit cell",
        "update_variable" => "Variable change",
        "paste_cells" => "Paste",
        "health_fix" => "Data Health fix",
        "append_cases" => "Add Cases",
        "merge_variables" => "Add Variables",
        "aggregate" => "Aggregate",
        "restructure_long" | "restructure_wide" => "Restructure",
        "compute_variable" => "Compute Variable",
        "recode" => "Recode",
        "scale_score" => "Scale Score",
        "sort_cases" => "Sort Cases",
        "rank_cases" => "Rank Cases",
        "standardize" => "Standardize",
        "reverse_score" => "Reverse Score",
        "date_parts" | "date_difference" => "Date and Time",
        "save_regression" => "Save regression values",
        "set_data_settings" => "Select, Split or Weight Cases",
        "load_sas_catalog" => "Value labels from catalog",
        "run_script" => "Run script",
        "clear_results" => "Clear output",
        _ => return None,
    })
}

/// Commands that replace the project, so the undo lists start again.
pub(crate) fn opens(command: &str) -> bool {
    matches!(command, "open_file" | "open_bytes" | "open_text" | "open_google_sheet" | "open_project" | "open_example" | "create_design" | "recover")
}

fn seqs(p: &Project) -> Vec<u32> {
    p.operations.iter().map(|o| o.seq).collect()
}

/// Moves the session's project to the step's snapshot; returns the step that moves it back.
fn apply(current: &mut Project, step: Step) -> Step {
    let back = Step { label: step.label.clone(), snapshot: current.clone(), remove: step.restore.clone(), restore: step.remove.clone(), results: step.results };
    let snap = step.snapshot;
    current.dataset = snap.dataset;
    current.data_settings = snap.data_settings;
    current.source = snap.source;
    current.report = snap.report;
    current.operations.retain(|o| !step.remove.contains(&o.seq));
    current.operations.extend(snap.operations.iter().filter(|o| step.restore.contains(&o.seq)).cloned());
    current.operations.sort_by_key(|o| o.seq);
    if step.results {
        current.results = snap.results;
    } else {
        current.results.retain(|r| !step.remove.contains(&r.operation));
        current.results.extend(snap.results.iter().filter(|r| step.restore.contains(&r.operation)).cloned());
        current.results.sort_by_key(|r| r.id);
    }
    back
}

impl Session {
    /// Runs a command, keeping what's needed to undo it.
    pub(crate) fn with_undo(&mut self, command: &str, run: impl FnOnce(&mut Self) -> Result<serde_json::Value, String>) -> Result<serde_json::Value, String> {
        // A script's own commands are part of the script's one step.
        if self.nesting > 0 {
            return run(self);
        }
        // Other data or another project: its results are checked afresh, never against answers kept for these.
        if opens(command) {
            self.forget_checks();
        }
        let before = label(command).and_then(|_| self.project.clone());
        let edits = self.edits;
        self.nesting += 1;
        let out = run(self);
        self.nesting -= 1;
        if out.is_ok() && opens(command) {
            self.undo.clear();
            self.redo.clear();
        }
        if let (Ok(_), Some(before), Some(now)) = (&out, before, self.project.as_ref()) {
            if self.edits != edits {
                let had = seqs(&before);
                let added: Vec<u32> = seqs(now).into_iter().filter(|s| !had.contains(s)).collect();
                self.undo.push(Step { label: label(command).unwrap_or_default().to_string(), snapshot: before, remove: added, restore: Vec::new(), results: command == "clear_results" });
                if self.undo.len() > DEPTH {
                    self.undo.remove(0);
                }
                self.redo.clear();
            }
        }
        out
    }

    /// Edit › Undo (`back`) or Redo.
    pub(crate) fn step_back(&mut self, back: bool) -> Result<String, String> {
        let list = if back { &mut self.undo } else { &mut self.redo };
        let step = list.pop().ok_or(if back { "There is nothing to undo." } else { "There is nothing to redo." })?;
        let label = step.label.clone();
        let p = self.project.as_mut().ok_or("Open a data file first.")?;
        let other = apply(p, step);
        if back { self.redo.push(other) } else { self.undo.push(other) }
        self.changed();
        Ok(label)
    }

    /// What Edit › Undo and Redo would do now, by name.
    pub(crate) fn undo_names(&self) -> (Option<String>, Option<String>) {
        (self.undo.last().map(|s| s.label.clone()), self.redo.last().map(|s| s.label.clone()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    fn session() -> Session {
        let mut s = Session::new();
        s.handle("open_text", json!({ "name": "demo.csv", "text": "g,y\n1,3\n1,4\n1,6\n2,5\n2,7\n2,9\n" })).unwrap();
        s
    }

    fn cell(s: &mut Session, row: usize) -> f64 {
        s.handle("rows", json!({ "start": row, "count": 1 })).unwrap()["rows"][0][1].as_f64().unwrap_or(f64::NAN)
    }

    fn commands(s: &Session) -> Vec<String> {
        s.project.as_ref().unwrap().operations.iter().map(|o| o.command.clone()).collect()
    }

    #[test]
    fn an_edit_is_undone_and_redone_and_later_results_stay() {
        let mut s = session();
        assert_eq!(s.state()["undo"], json!(null));
        s.handle("set_cell", json!({ "row": 0, "name": "y", "value": "30" })).unwrap();
        assert_eq!(cell(&mut s, 0), 30.0);
        s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        assert_eq!(s.state()["undo"], "Edit cell");
        assert_eq!(s.handle("undo", json!({})).unwrap()["did"], "Edit cell");
        assert_eq!(cell(&mut s, 0), 3.0);
        assert!(!commands(&s).contains(&"set_cell".to_string()), "the undone edit leaves the record");
        let out = s.handle("results", json!({})).unwrap();
        assert_eq!(out.as_array().unwrap().len(), 1, "the later result stays");
        assert_eq!(out[0]["stale"], true, "and is out of date: it used the edited value");
        assert_eq!((s.state()["undo"].clone(), s.state()["redo"].clone()), (json!(null), json!("Edit cell")));
        s.handle("redo", json!({})).unwrap();
        assert_eq!(cell(&mut s, 0), 30.0);
        assert!(commands(&s).contains(&"set_cell".to_string()));
        assert_eq!(s.handle("results", json!({})).unwrap()[0]["stale"], false);
        assert!(s.handle("redo", json!({})).unwrap_err().contains("nothing to redo"));
    }

    #[test]
    fn transforms_scripts_and_clearing_are_one_step_each() {
        let mut s = session();
        s.handle("compute_variable", json!({ "target": "y2", "formula": "y * 2" })).unwrap();
        s.handle("recode", json!({ "variables": ["y"], "rules": [{ "from": { "kind": "value", "value": 3 }, "to": { "kind": "value", "value": 30 } }] })).unwrap();
        s.handle("undo", json!({})).unwrap();
        assert_eq!(cell(&mut s, 0), 3.0, "the recode is undone");
        assert!(s.handle("dataset", json!({})).unwrap()["variables"].as_array().unwrap().iter().any(|v| v["name"] == "y2"), "the compute is not");
        // A new change clears Redo.
        s.handle("update_variable", json!({ "name": "y", "label": "Score" })).unwrap();
        assert_eq!(s.state()["redo"], json!(null));
        assert_eq!(s.state()["undo"], "Variable change");

        // A script that computes a variable and runs an analysis is undone as a whole.
        let ran = crate::script::run(&mut s, "compute_variable {\"target\": \"y3\", \"formula\": \"y + 1\"}\nfrequencies {\"variables\": [\"g\"]}");
        assert!(ran["error"].is_null(), "{ran}");
        let ran = s.handle("run_script", json!({ "text": "compute_variable {\"target\": \"y4\", \"formula\": \"y + 1\"}\nfrequencies {\"variables\": [\"g\"]}" })).unwrap();
        assert!(ran["error"].is_null(), "{ran}");
        let results = s.handle("results", json!({})).unwrap().as_array().unwrap().len();
        assert_eq!(s.state()["undo"], "Run script");
        s.handle("undo", json!({})).unwrap();
        let names: Vec<String> = s.handle("dataset", json!({})).unwrap()["variables"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap().to_string()).collect();
        assert!(!names.contains(&"y4".to_string()) && names.contains(&"y3".to_string()), "{names:?}");
        assert_eq!(s.handle("results", json!({})).unwrap().as_array().unwrap().len(), results - 1, "the script's result goes too");

        // Clearing the output.
        let before = s.handle("results", json!({})).unwrap().as_array().unwrap().len();
        s.handle("clear_results", json!({})).unwrap();
        s.handle("undo", json!({})).unwrap();
        assert_eq!(s.handle("results", json!({})).unwrap().as_array().unwrap().len(), before);

        // Opening other data starts again.
        s.handle("open_text", json!({ "name": "other.csv", "text": "a\n1\n2\n" })).unwrap();
        assert_eq!((s.state()["undo"].clone(), s.state()["redo"].clone()), (json!(null), json!(null)));
        assert!(s.handle("undo", json!({})).unwrap_err().contains("nothing to undo"));
    }

    #[test]
    fn commands_that_change_nothing_leave_no_step() {
        let mut s = session();
        s.handle("descriptives", json!({ "variables": ["y"] })).unwrap();
        assert!(s.handle("compute_variable", json!({ "target": "y", "formula": "1" })).is_err());
        assert_eq!(s.state()["undo"], json!(null));
    }
}
