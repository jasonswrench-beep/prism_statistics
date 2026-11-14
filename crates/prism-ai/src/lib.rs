//! The AI writer (decision D14, design 10-ai-apa-writeup.md).
//!
//! Optional and off by default. A small open model (Qwen3-4B, Apache-2.0 weights) runs on the user's own computer
//! through llama.cpp (MIT); nothing is sent anywhere. The model never writes a number: decoding only lets it produce
//! English prose and `{slot}` references to fields of the verified result, and PrismStatistics fills each slot with the
//! same APA formatting the template writer uses. Every draft then goes through the checks in [`checks`], and nothing
//! reaches an export until the user has reviewed or edited it.

pub mod checks;
pub mod llama;
pub mod model;
pub mod slots;

pub use checks::{Check, CheckState};
pub use model::{download, generate, installed, model_dir, ModelInfo, MODEL};
pub use slots::{fill, prepare, Input, Slot};

use serde::{Deserialize, Serialize};

/// An AI draft kept with its result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    /// The paragraph with slots filled, marked like template text (`*t*` for italics).
    pub text: String,
    /// What the model wrote, with its `{slots}`, kept for the record (REP-05).
    pub raw: String,
    pub model: String,
    pub model_sha256: String,
    pub checks: Vec<Check>,
    /// draft, reviewed, edited.
    pub status: String,
    /// Seconds the model took.
    pub seconds: f64,
}

impl Draft {
    /// True when a check failed outright, so the draft must not be offered as text to use.
    pub fn rejected(&self) -> bool {
        self.checks.iter().any(|c| c.state == CheckState::Fail)
    }
}

/// Turns what the model wrote into a draft: fills the slots and runs every check.
pub fn finish(bundle: &prism_engine::ResultBundle, input: &Input, raw: &str, seconds: f64) -> Draft {
    let (text, unknown) = fill(input, raw);
    let checks = checks::run(bundle, input, raw, &text, &unknown);
    Draft { text, raw: raw.to_string(), model: MODEL.name.to_string(), model_sha256: MODEL.files[0].sha256.to_string(), checks, status: "draft".into(), seconds }
}
