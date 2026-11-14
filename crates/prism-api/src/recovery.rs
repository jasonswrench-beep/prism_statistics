//! Crash recovery and long-analysis control.
//!
//! While a project has unsaved changes, the window calls `autosave` every half minute. It writes the project to a
//! recovery folder (outside the user's own folders) as `recovery-<instance>.pstat`, beside `recovery-<instance>.json`
//! saying which file it came from and when. The note's heartbeat is renewed on each call, so the copy of a window still
//! running is never offered to another. Saving, or closing with nothing unsaved, removes both files. When PrismStatistics
//! starts, `recovery_list` names the copies a closed or crashed window left behind, and `recover` opens one (as unsaved
//! work, so the user decides where it goes) or `discard_recovery` deletes it.
//!
//! `job_status` and `cancel_job` answer without the session: the window calls them while an analysis holds it.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{parse, Session};
use prism_engine::project;

/// Where a session keeps its recovery copy.
#[derive(Debug, Clone)]
pub struct Recovery {
    pub dir: PathBuf,
    /// This window's name among the copies (its process id).
    pub instance: String,
    /// A copy whose heartbeat is older than this many seconds belongs to a window that is no longer running.
    pub stale_after: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Note {
    instance: String,
    name: String,
    path: Option<String>,
    saved_at: u64,
    heartbeat: u64,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The commands that run without the session (and so while an analysis runs), or None for any other command.
pub fn job_command(command: &str, args: &Value) -> Option<Result<Value, String>> {
    match command {
        "job_status" => Some(Ok(json!(prism_engine::job::running()))),
        "cancel_job" => {
            let id = args.get("id").and_then(Value::as_u64);
            Some(Ok(json!({ "cancelled": prism_engine::job::cancel(id) })))
        }
        _ => None,
    }
}

impl Recovery {
    fn files(&self, instance: &str) -> (PathBuf, PathBuf) {
        let safe: String = instance.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        (self.dir.join(format!("recovery-{safe}.{}", project::EXTENSION)), self.dir.join(format!("recovery-{safe}.json")))
    }
}

impl Session {
    /// A session that keeps a recovery copy of unsaved work in `dir`.
    pub fn with_recovery(dir: PathBuf) -> Self {
        Session { recovery: Some(Recovery { dir, instance: std::process::id().to_string(), stale_after: 120 }), ..Self::default() }
    }

    pub(crate) fn recovery_command(&mut self, command: &str, args: Value) -> Result<Value, String> {
        match command {
            "autosave" => self.autosave(),
            "clear_recovery" => {
                self.clear_own_recovery();
                Ok(json!({}))
            }
            "recovery_list" => Ok(json!(self.recovery_list())),
            "recover" | "discard_recovery" => {
                #[derive(Deserialize)]
                struct A {
                    id: String,
                }
                let a: A = parse(args)?;
                let r = self.recovery.clone().ok_or("Recovery isn't available here.")?;
                if a.id == r.instance {
                    return Err("That copy belongs to this window.".into());
                }
                let (data, note) = r.files(&a.id);
                if command == "discard_recovery" {
                    let _ = std::fs::remove_file(&data);
                    let _ = std::fs::remove_file(&note);
                    return Ok(json!({ "recovery": self.recovery_list() }));
                }
                let p = project::load(&data).map_err(|e| format!("The recovered copy could not be opened: {e}"))?;
                let n: Option<Note> = std::fs::read(&note).ok().and_then(|b| serde_json::from_slice(&b).ok());
                self.project = Some(p);
                self.path = n.and_then(|n| n.path).map(PathBuf::from);
                self.changed();
                // The copy now lives in this window (and in its own recovery file from the next autosave).
                let _ = std::fs::remove_file(&data);
                let _ = std::fs::remove_file(&note);
                self.autosave()?;
                Ok(json!({ "state": self.state(), "dataset": self.summary()?, "results": self.outputs() }))
            }
            other => Err(format!("Unknown command \"{other}\".")),
        }
    }

    /// Writes the recovery copy when there is unsaved work it doesn't have yet, and renews the heartbeat; with nothing
    /// unsaved, removes the copy.
    fn autosave(&mut self) -> Result<Value, String> {
        let Some(r) = self.recovery.clone() else { return Ok(json!({ "saved": false })) };
        let (data, note) = r.files(&r.instance);
        let Some(p) = self.project.as_ref().filter(|_| self.dirty) else {
            self.clear_own_recovery();
            return Ok(json!({ "saved": false }));
        };
        std::fs::create_dir_all(&r.dir).map_err(|e| format!("The recovery folder could not be made: {e}"))?;
        let mut saved = false;
        if self.autosaved != Some(self.edits) || !data.is_file() {
            let bytes = project::to_bytes(p).map_err(|e| e.to_string())?;
            project::write_atomic(&data, &bytes).map_err(|e| e.to_string())?;
            self.autosaved = Some(self.edits);
            self.autosaved_at = now();
            saved = true;
        }
        let name = self.path.as_ref().and_then(|p| p.file_stem()).and_then(|s| s.to_str()).map(str::to_string).unwrap_or_else(|| p.dataset.name.clone());
        let n = Note { instance: r.instance.clone(), name, path: self.path.as_ref().map(|p| p.to_string_lossy().into_owned()), saved_at: self.autosaved_at, heartbeat: now() };
        project::write_atomic(&note, &serde_json::to_vec(&n).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        Ok(json!({ "saved": saved }))
    }

    /// Removes this window's recovery copy (after a save, or when nothing is unsaved).
    pub(crate) fn clear_own_recovery(&mut self) {
        if let Some(r) = &self.recovery {
            let (data, note) = r.files(&r.instance);
            let _ = std::fs::remove_file(data);
            let _ = std::fs::remove_file(note);
        }
        self.autosaved = None;
    }

    /// The copies left by windows that are no longer running, newest first.
    fn recovery_list(&self) -> Vec<Value> {
        let Some(r) = &self.recovery else { return Vec::new() };
        let Ok(entries) = std::fs::read_dir(&r.dir) else { return Vec::new() };
        let mut out: Vec<Note> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
            .filter_map(|e| serde_json::from_slice::<Note>(&std::fs::read(e.path()).ok()?).ok())
            .filter(|n| n.instance != r.instance && now().saturating_sub(n.heartbeat) >= r.stale_after && r.files(&n.instance).0.is_file())
            .collect();
        out.sort_by_key(|n| std::cmp::Reverse(n.saved_at));
        out.into_iter().map(|n| json!({ "id": n.instance, "name": n.name, "path": n.path, "saved_at": n.saved_at })).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(dir: &std::path::Path, instance: &str) -> Session {
        Session { recovery: Some(Recovery { dir: dir.to_path_buf(), instance: instance.into(), stale_after: 0 }), ..Session::default() }
    }

    #[test]
    fn unsaved_work_survives_a_crash() {
        let dir = std::env::temp_dir().join(format!("prism-recovery-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = session(&dir, "1");
        // Nothing open, nothing unsaved: no copy.
        assert_eq!(a.handle("autosave", json!({})).unwrap()["saved"], false);
        a.handle("open_text", json!({ "name": "study.csv", "text": "x,y\n1,2\n3,4\n" })).unwrap();
        a.handle("descriptives", json!({ "variables": ["x", "y"] })).unwrap();
        assert_eq!(a.handle("autosave", json!({})).unwrap()["saved"], true);
        // Nothing changed since: the heartbeat is renewed but the copy isn't written again.
        assert_eq!(a.handle("autosave", json!({})).unwrap()["saved"], false);
        // Window 1 "crashes" (is dropped without saving). Window 2 starts and is offered the copy; window 1's own list
        // never shows its own copy.
        assert!(a.recovery_list().is_empty());
        drop(a);
        let mut b = session(&dir, "2");
        let list = b.handle("recovery_list", json!({})).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);
        assert_eq!(list[0]["name"], "study");
        let back = b.handle("recover", json!({ "id": "1" })).unwrap();
        assert_eq!(back["state"]["dirty"], true, "recovered work is unsaved until the user saves it");
        assert_eq!(back["results"].as_array().unwrap().len(), 1);
        assert_eq!(back["dataset"]["variables"].as_array().unwrap().len(), 2);
        // The copy moved to window 2.
        assert!(b.handle("recovery_list", json!({})).unwrap().as_array().unwrap().is_empty());
        assert!(dir.join("recovery-2.pstat").is_file() && !dir.join("recovery-1.pstat").exists());
        // Saving removes the copy.
        let path = dir.join("saved.pstat");
        b.handle("save_project", json!({ "path": path.to_string_lossy() })).unwrap();
        assert!(!dir.join("recovery-2.pstat").exists() && !dir.join("recovery-2.json").exists());
        // A copy can be discarded instead.
        let mut c = session(&dir, "3");
        c.handle("open_text", json!({ "name": "other.csv", "text": "z\n1\n" })).unwrap();
        c.handle("autosave", json!({})).unwrap();
        drop(c);
        let mut d = session(&dir, "4");
        let left = d.handle("discard_recovery", json!({ "id": "3" })).unwrap();
        assert!(left["recovery"].as_array().unwrap().is_empty());
        assert!(!dir.join("recovery-3.pstat").exists());
        // A running window's copy (fresh heartbeat) isn't offered.
        let mut e = session(&dir, "5");
        e.recovery.as_mut().unwrap().stale_after = 600;
        let mut f = session(&dir, "6");
        f.handle("open_text", json!({ "name": "live.csv", "text": "z\n1\n" })).unwrap();
        f.handle("autosave", json!({})).unwrap();
        assert!(e.recovery_list().is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_cancelled_analysis_adds_nothing() {
        let mut s = Session::new();
        let rows: String = (0..200).map(|i| format!("{},{}\n", i % 7, (i * 37 % 101) as f64 / 10.0)).collect();
        s.handle("open_text", json!({ "name": "d.csv", "text": format!("x,y\n{rows}") })).unwrap();
        // Cancel from another thread as soon as the forest starts.
        let stop = std::thread::spawn(|| {
            for _ in 0..2000 {
                if let Some(j) = prism_engine::job::running().into_iter().find(|j| j.label == "forest") {
                    return job_command("cancel_job", &json!({ "id": j.id })).unwrap().unwrap()["cancelled"].as_u64().unwrap();
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            0
        });
        let before = s.project.as_ref().unwrap().operations.len();
        let r = s.handle("forest", json!({ "dependent": "y", "predictors": ["x"], "n_trees": 10000 }));
        assert!(stop.join().unwrap() >= 1, "the job was seen running");
        assert!(r.unwrap_err().contains("cancelled"));
        assert!(s.handle("results", json!({})).unwrap().as_array().unwrap().is_empty());
        assert_eq!(s.project.as_ref().unwrap().operations.len(), before, "nothing was recorded");
    }
}
