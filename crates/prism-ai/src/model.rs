//! The model: where it lives, how it is downloaded and verified, and constrained generation.
//!
//! The model is not part of the installer. Turning AI writing on offers a one-time download of about 2.5 GB from the
//! model's publisher on Hugging Face, pinned to a fixed revision and checked against SHA-256 hashes recorded here
//! (SEC-05), so the file used is exactly the file that was evaluated. It runs on the CPU through llama.cpp
//! ([`crate::llama`]); an earlier candle engine was about eight times slower on the same paragraph.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub use crate::llama::generate;

pub struct ModelFile {
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

pub struct ModelInfo {
    pub name: &'static str,
    pub license: &'static str,
    pub folder: &'static str,
    pub files: [ModelFile; 1],
}

/// Qwen3-4B, 4-bit (Q4_K_M), weights under Apache-2.0 (licence rule of 2026-11-10). Chosen over Qwen3-1.7B in the
/// first spike: the smaller model left out required statistics; this one wrote complete APA paragraphs.
pub const MODEL: ModelInfo = ModelInfo {
    name: "Qwen3-4B (Q4_K_M)",
    license: "Apache-2.0",
    folder: "qwen3-4b-q4km",
    files: [
        ModelFile {
            name: "Qwen3-4B-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/bc640142c66e1fdd12af0bd68f40445458f3869b/Qwen3-4B-Q4_K_M.gguf",
            sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
            size: 2_497_280_256,
        },
    ],
};

/// The app's identifier (tauri.conf.json). Its folder under %LOCALAPPDATA% is the one the uninstaller's "delete app data"
/// box removes, so the model and settings kept there go with an uninstall when the user asks for that.
pub const APP_ID: &str = "io.github.jasonswrench-beep.prismstatistics";

/// Where PrismStatistics keeps its own files: %LOCALAPPDATA%\<APP_ID> on Windows (PRISM_DATA_DIR overrides it).
/// Versions before 0.1.0 kept them in %LOCALAPPDATA%\PrismStatistics, the install folder; they are moved once.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("PRISM_DATA_DIR") {
        return PathBuf::from(d);
    }
    let base = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("XDG_DATA_HOME")).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))).unwrap_or_else(std::env::temp_dir);
    let dir = base.join(APP_ID);
    static MOVED: std::sync::Once = std::sync::Once::new();
    MOVED.call_once(|| move_old_data(&base.join("PrismStatistics"), &dir));
    dir
}

/// Moves the model folder and settings from the old place, when they are there and not yet in the new one. Nothing
/// else in the old folder (the program itself) is touched, and a failed move leaves both where they were.
fn move_old_data(old: &Path, new: &Path) {
    for name in ["models", "settings.json"] {
        let (from, to) = (old.join(name), new.join(name));
        if from.exists() && !to.exists() && std::fs::create_dir_all(new).is_ok() {
            let _ = std::fs::rename(&from, &to);
        }
    }
}

pub fn model_dir() -> PathBuf {
    data_dir().join("models").join(MODEL.folder)
}

/// True when every model file is present at its full size. (The hashes were checked when it was downloaded.)
pub fn installed(dir: &Path) -> bool {
    MODEL.files.iter().all(|f| std::fs::metadata(dir.join(f.name)).is_ok_and(|m| m.len() == f.size))
}

/// Downloads the model files, verifying each one's hash before it is put in place. `progress(done, total)` is called
/// as bytes arrive. A file that is already present and complete is not downloaded again.
pub fn download(dir: &Path, progress: impl Fn(u64, u64)) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("The model folder could not be created: {e}"))?;
    let total: u64 = MODEL.files.iter().map(|f| f.size).sum();
    let mut done = 0u64;
    let agent = ureq::Agent::config_builder().user_agent(concat!("PrismStatistics/", env!("CARGO_PKG_VERSION"))).build().new_agent();
    for f in &MODEL.files {
        let target = dir.join(f.name);
        if std::fs::metadata(&target).is_ok_and(|m| m.len() == f.size) {
            done += f.size;
            progress(done, total);
            continue;
        }
        let partial = dir.join(format!("{}.part", f.name));
        let mut response = agent.get(f.url).call().map_err(|e| format!("The model could not be downloaded. Check the internet connection and try again. ({e})"))?;
        let mut reader = response.body_mut().with_config().limit(f.size + 1).reader();
        let mut out = std::fs::File::create(&partial).map_err(|e| format!("The model file could not be written: {e}"))?;
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("The download was interrupted: {e}"))?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            out.write_all(&buf[..n]).map_err(|e| format!("The model file could not be written: {e}"))?;
            done += n as u64;
            progress(done, total);
        }
        out.sync_all().ok();
        drop(out);
        let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if got != f.sha256 {
            let _ = std::fs::remove_file(&partial);
            return Err(format!("The downloaded {} didn't match its published checksum, so it was deleted. Try again later.", f.name));
        }
        std::fs::rename(&partial, &target).map_err(|e| format!("The model file could not be put in place: {e}"))?;
    }
    Ok(())
}

/// Characters a paragraph of English APA prose may contain. Everything else, including digits in any script, is
/// refused, so a number can only enter through a slot.
fn prose_char(c: char) -> bool {
    (c.is_ascii() && !c.is_ascii_digit() && (c >= ' ' || c == '\n'))
        || matches!(c as u32, 0xC0..=0x17F)
        || (matches!(c as u32, 0x391..=0x3C9) && c.is_alphabetic())
        || "\u{2013}\u{2014}\u{2018}\u{2019}\u{201C}\u{201D}\u{2212}\u{00D7}".contains(c)
}

/// Whether `piece` may follow `so_far`: prose outside braces, and inside braces only the start of a real slot name.
pub(crate) fn allowed(so_far: &str, piece: &str, names: &[&str]) -> bool {
    if piece.is_empty() {
        return false;
    }
    let mut open = so_far.rfind('{').filter(|&i| !so_far[i..].contains('}')).map(|i| so_far[i + 1..].to_string());
    for c in piece.chars() {
        match (&mut open, c) {
            (Some(name), '}') => {
                if !names.contains(&name.as_str()) {
                    return false;
                }
                open = None;
            }
            (Some(name), c) => {
                name.push(c);
                if !names.iter().any(|n| n.starts_with(name.as_str())) {
                    return false;
                }
            }
            (None, '{') => {
                open = Some(String::new());
            }
            (None, '}') => return false,
            (None, c) => {
                if !prose_char(c) {
                    return false;
                }
            }
        }
    }
    // Italics are single asterisks; a run of them is never right ("******" appeared where the model wanted "95%").
    if format!("{so_far}{piece}").ends_with("**") || piece.contains("**") {
        return false;
    }
    // Numbers spelled out in words are numbers too ("sixty-six point two"): refuse them as they form.
    open.is_some() || !crate::checks::spells_a_number(&format!("{so_far}{piece}"), so_far.len())
}

#[cfg(test)]
mod tests {

    #[test]
    fn old_model_and_settings_move_once_and_nothing_else_does() {
        let root = std::env::temp_dir().join(format!("prism-move-{}", std::process::id()));
        let (old, new) = (root.join("PrismStatistics"), root.join(super::APP_ID));
        std::fs::create_dir_all(old.join("models").join("m")).unwrap();
        std::fs::write(old.join("models").join("m").join("w.gguf"), b"w").unwrap();
        std::fs::write(old.join("settings.json"), b"{}").unwrap();
        std::fs::write(old.join("PrismStatistics.exe"), b"exe").unwrap();
        super::move_old_data(&old, &new);
        assert!(new.join("models").join("m").join("w.gguf").exists() && new.join("settings.json").exists());
        assert!(!old.join("models").exists() && old.join("PrismStatistics.exe").exists(), "the program stays");
        // A second start finds them already moved; newer settings are never overwritten by old ones.
        std::fs::write(old.join("settings.json"), b"old").unwrap();
        super::move_old_data(&old, &new);
        assert_eq!(std::fs::read(new.join("settings.json")).unwrap(), b"{}");
        std::fs::remove_dir_all(&root).ok();
    }

    use super::*;

    #[test]
    fn only_prose_and_real_slots_can_be_written() {
        let names = ["t", "df", "g1.mean"];
        assert!(allowed("The test", " was significant", &names));
        assert!(!allowed("The test", " in 2026", &names), "digits");
        assert!(!allowed("The test", " 九五", &names), "other scripts");
        assert!(!allowed("x", "\u{FF19}", &names), "full-width digits");
        assert!(allowed("*t*(", "{df", &names));
        assert!(allowed("*t*({df", "}) = {t}", &names));
        assert!(!allowed("*t*({d", "x}", &names), "not a slot");
        assert!(allowed("(*M* = {g1.", "mean})", &names));
        assert!(!allowed("(*M* = {g1.mean", "2}", &names), "digit inside a slot");
        assert!(!allowed("ok", "}", &names), "a stray closing brace");
        assert!(!allowed("difference = {diff}, *", "*", &names), "asterisk runs");
        assert!(!allowed("(*M* = ", "sixty", &names), "number words");
        assert!(!allowed("(*M* = six", "ty", &names), "number words built from pieces");
        assert!(!allowed("*M* = one", " point", &names), "decimals in words");
        assert!(!allowed("about five", " percent", &names), "percentages in words");
        assert!(allowed("across the", " three groups", &names), "small counts are words in APA");
        assert!(!allowed("The -nine", "-five", &names), "digit by digit in words");
        assert!(!allowed("The nine-five", "-per", &names), "per cent");
    }
}
