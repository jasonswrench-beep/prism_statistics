//! Commands for the optional AI writer (D14): its switch, the model download, drafting, and review.
//!
//! The switch is off until the user turns it on, and the setting lives in PrismStatistics' own folder, not in projects.
//! Drafting is split so the desktop app can run the slow part without holding the engine: `ai_input` (fast, needs the
//! project), [`write`] (slow, needs only the model), `ai_store` (fast). `ai_draft` does all three, for the test server.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use prism_ai::{Draft, Input, MODEL};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{parse, Session};

static DOWNLOADING: AtomicBool = AtomicBool::new(false);
static DONE: AtomicU64 = AtomicU64::new(0);
static TOTAL: AtomicU64 = AtomicU64::new(0);
static DOWNLOAD_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn settings_path() -> std::path::PathBuf {
    prism_ai::model::data_dir().join("settings.json")
}

fn settings() -> Value {
    std::fs::read(settings_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}))
}

/// A switch in PrismStatistics' settings file; every switch is off until the user turns it on.
pub(crate) fn setting(key: &str) -> bool {
    settings().get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub fn enabled() -> bool {
    setting("ai_writing")
}

fn set_enabled(on: bool) -> Result<(), String> {
    set_setting("ai_writing", on)
}

pub(crate) fn set_setting(key: &str, on: bool) -> Result<(), String> {
    let mut s = settings();
    s[key] = json!(on);
    let path = settings_path();
    std::fs::create_dir_all(path.parent().expect("a settings folder")).map_err(|e| format!("Settings could not be saved: {e}"))?;
    std::fs::write(&path, serde_json::to_vec_pretty(&s).expect("settings serialize")).map_err(|e| format!("Settings could not be saved: {e}"))
}

fn status() -> Value {
    let dir = prism_ai::model_dir();
    json!({
        "enabled": enabled(),
        "installed": prism_ai::installed(&dir),
        "model": MODEL.name,
        "license": MODEL.license,
        "size": MODEL.files.iter().map(|f| f.size).sum::<u64>(),
        "folder": dir.to_string_lossy(),
        "downloading": DOWNLOADING.load(Ordering::SeqCst),
        "done": DONE.load(Ordering::SeqCst),
        "total": TOTAL.load(Ordering::SeqCst),
        "error": DOWNLOAD_ERROR.lock().ok().and_then(|e| e.clone()),
    })
}

/// The slow step: the model writes. Needs no project, so the desktop app runs it without holding the engine.
pub fn write(input: &Input) -> Result<(String, f64), String> {
    if !enabled() {
        return Err("AI writing is turned off. Turn it on in Tools › AI writing.".into());
    }
    prism_ai::generate(&prism_ai::model_dir(), input)
}

/// The slow step for callers that hold the engine elsewhere (the desktop app): takes `ai_input`'s answer and returns
/// what `ai_store` needs, `{ "raw": ..., "seconds": ... }`.
pub fn write_value(input: &Value) -> Result<Value, String> {
    let input: Input = serde_json::from_value(input.clone()).map_err(|e| format!("Bad AI input: {e}"))?;
    let (raw, seconds) = write(&input)?;
    Ok(json!({ "raw": raw, "seconds": seconds }))
}

/// The paragraph and model of a draft the user reviewed or edited; None for anything else.
pub fn reviewed_text(ai: Option<&Value>) -> Option<(String, String)> {
    let d: Draft = serde_json::from_value(ai?.clone()).ok()?;
    matches!(d.status.as_str(), "reviewed" | "edited").then_some((d.text, d.model))
}

impl Session {
    pub(crate) fn ai_command(&mut self, command: &str, args: Value) -> Result<Value, String> {
        #[derive(Deserialize)]
        struct Id {
            id: u32,
        }
        match command {
            "ai_status" => Ok(status()),
            "ai_enable" => {
                #[derive(Deserialize)]
                struct A {
                    on: bool,
                }
                set_enabled(parse::<A>(args)?.on)?;
                Ok(status())
            }
            "ai_download" => {
                if !DOWNLOADING.swap(true, Ordering::SeqCst) {
                    *DOWNLOAD_ERROR.lock().map_err(|_| "busy")? = None;
                    std::thread::spawn(|| {
                        let result = prism_ai::download(&prism_ai::model_dir(), |done, total| {
                            DONE.store(done, Ordering::SeqCst);
                            TOTAL.store(total, Ordering::SeqCst);
                        });
                        if let Err(e) = result {
                            if let Ok(mut slot) = DOWNLOAD_ERROR.lock() {
                                *slot = Some(e);
                            }
                        }
                        DOWNLOADING.store(false, Ordering::SeqCst);
                    });
                }
                Ok(status())
            }
            "ai_remove" => {
                if DOWNLOADING.load(Ordering::SeqCst) {
                    return Err("Wait for the download to finish first.".into());
                }
                let dir = prism_ai::model_dir();
                for f in &MODEL.files {
                    let _ = std::fs::remove_file(dir.join(f.name));
                    let _ = std::fs::remove_file(dir.join(format!("{}.part", f.name)));
                }
                Ok(status())
            }
            // The fast first step: what the model will be given for this result.
            "ai_input" => {
                let a: Id = parse(args)?;
                self.ai_input(a.id).map(|i| json!(i))
            }
            // The fast last step: fill, check and keep the draft.
            "ai_store" => {
                #[derive(Deserialize)]
                struct A {
                    id: u32,
                    input: Input,
                    raw: String,
                    seconds: f64,
                }
                let a: A = parse(args)?;
                self.ai_store(a.id, &a.input, &a.raw, a.seconds)
            }
            "ai_draft" => {
                let a: Id = parse(args)?;
                let input = self.ai_input(a.id)?;
                let (raw, seconds) = write(&input)?;
                self.ai_store(a.id, &input, &raw, seconds)
            }
            "ai_review" => {
                #[derive(Deserialize)]
                struct A {
                    id: u32,
                    action: String,
                    #[serde(default)]
                    text: Option<String>,
                }
                let a: A = parse(args)?;
                let p = self.project_mut()?;
                let r = p.results.iter_mut().find(|r| r.id == a.id).ok_or("That result is no longer in the output.")?;
                match a.action.as_str() {
                    "discard" => r.ai = None,
                    "accept" | "edit" => {
                        let mut d: Draft = serde_json::from_value(r.ai.clone().ok_or("There is no AI draft for this result.")?).map_err(|e| e.to_string())?;
                        if a.action == "accept" && d.rejected() {
                            return Err("This draft failed a check, so it can't be accepted. Edit it or use the template paragraph.".into());
                        }
                        if let Some(t) = a.text.filter(|t| a.action == "edit" && t.trim() != d.text.trim()) {
                            d.text = t.trim().to_string();
                            d.status = "edited".into();
                        } else {
                            d.status = "reviewed".into();
                        }
                        r.ai = Some(json!(d));
                    }
                    other => return Err(format!("\"{other}\" isn't a review action.")),
                }
                self.changed();
                Ok(json!(self.outputs().into_iter().find(|o| o.id == a.id)))
            }
            other => Err(format!("Unknown command \"{other}\".")),
        }
    }

    fn ai_input(&self, id: u32) -> Result<Input, String> {
        if !enabled() {
            return Err("AI writing is turned off. Turn it on in Tools › AI writing.".into());
        }
        let o = self.outputs().into_iter().find(|o| o.id == id).ok_or("That result is no longer in the output.")?;
        if o.stale {
            return Err("The data changed after this result was run. Run it again before writing it up.".into());
        }
        prism_ai::prepare(&o.bundle)
    }

    fn ai_store(&mut self, id: u32, input: &Input, raw: &str, seconds: f64) -> Result<Value, String> {
        let p = self.project_mut()?;
        let r = p.results.iter_mut().find(|r| r.id == id).ok_or("That result is no longer in the output.")?;
        let draft = prism_ai::finish(&r.bundle, input, raw, seconds);
        r.ai = Some(json!(draft));
        self.changed();
        Ok(json!(self.outputs().into_iter().find(|o| o.id == id)))
    }
}
