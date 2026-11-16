//! The project file (`.pstat`): format 1 as specified in PSTAT-1, revision 1.1 (openstage-core `specs/pstat`).
//!
//! A project is one file holding the dataset (values, variables, labels and missing codes), where the data came from,
//! the operation record (every analysis and variable change, in order, with its settings), and the results with the
//! settings that made them. It is a zip archive with one `project.json` inside, so it stays small and can grow to hold
//! more parts later without changing the format's name.
//!
//! Saving is transactional: the new file is written next to the old one under a temporary name and then moved over
//! it, so a crash or a full disk can never leave a half-written project behind. A file from a newer version of
//! PrismStatistics is refused with a plain message rather than opened and damaged.

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::import::ImportReport;
use crate::model::Dataset;
use crate::result::ResultBundle;

pub const EXTENSION: &str = "pstat";
pub const FORMAT: &str = "PrismStatistics project";
pub const VERSION: u32 = 1;
/// The layout OpenAssess writes for a class's scored answer sheets (PSTAT-1 section 7.1).
pub const PROFILE_ITEM_RESULTS: &str = "OpenAssess item results 1";
/// The same with the letters marked and the part `assessment.json` (key and question details; section 7.2).
pub const PROFILE_ITEM_RESULTS_2: &str = "OpenAssess item results 2";
/// PrismQDA's cases with their attributes and codes, and the part `case-table.json` (section 7.3).
pub const PROFILE_CASE_TABLE: &str = "PrismQDA case table 1";

/// The part (an entry beside project.json) each profile carries.
pub fn part_for(profile: &str) -> Option<&'static str> {
    match profile {
        PROFILE_ITEM_RESULTS_2 => Some("assessment.json"),
        PROFILE_CASE_TABLE => Some("case-table.json"),
        _ => None,
    }
}
const ENTRY: &str = "project.json";

/// Where the data came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub file_name: String,
    pub report: ImportReport,
}

/// One step in the operation record: a command and its arguments, exactly as they were run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub seq: u32,
    pub command: String,
    pub args: Value,
}

/// A result kept in the project, linked to the operation that made it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredResult {
    pub id: u32,
    pub operation: u32,
    pub bundle: ResultBundle,
    pub paragraph: Option<String>,
    /// An AI draft for this result (prism-ai's Draft), with its checks, model and review status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<Value>,
    /// When the project was saved: a fingerprint of the data and settings the result reads (prism-api's recheck), and
    /// whether it was out of date then. An opened project needn't run every analysis again to show which results are
    /// out of date. Only a hint: a fingerprint that doesn't match the data means the result is checked again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<(u64, bool)>,
}

/// One block of the Report Builder's layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReportBlock {
    Heading { text: String },
    /// The writer's own text; *asterisks* mark italics, as in the template paragraphs.
    Text { text: String },
    /// A result, with the parts to show.
    Result {
        id: u32,
        #[serde(default = "yes")]
        charts: bool,
        #[serde(default = "yes")]
        tables: bool,
        #[serde(default = "yes")]
        checks: bool,
        #[serde(default = "yes")]
        paragraph: bool,
        #[serde(default = "yes")]
        method: bool,
    },
}

fn yes() -> bool {
    true
}

/// The Report Builder's layout: a title and the blocks in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub title: String,
    pub blocks: Vec<ReportBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub format: String,
    pub version: u32,
    /// The PrismStatistics version that saved the file.
    pub app_version: String,
    pub dataset: Dataset,
    pub source: Option<Source>,
    pub operations: Vec<Operation>,
    pub results: Vec<StoredResult>,
    /// The Report Builder's layout, once the user has arranged one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<Report>,
    /// Select cases, Split file and Weight cases as they are now; each analysis records the settings it ran with.
    #[serde(default, skip_serializing_if = "crate::subset::DataSettings::is_empty")]
    pub data_settings: crate::subset::DataSettings,
    /// The profile the file says it follows (PSTAT-1 section 7), kept when the project is saved again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// The profile's part as it was read (entry name and bytes), written back unchanged beside project.json when the
    /// project is saved (rule Z4). Not part of project.json.
    #[serde(skip)]
    pub part: Option<(String, Vec<u8>)>,
}

impl Project {
    /// The profile's part, parsed, when the file carries one.
    pub fn part_json(&self) -> Option<Value> {
        self.part.as_ref().and_then(|(_, b)| serde_json::from_slice(b).ok())
    }
}

impl Project {
    pub fn new(dataset: Dataset, source: Option<Source>) -> Self {
        Project { format: FORMAT.into(), version: VERSION, app_version: env!("CARGO_PKG_VERSION").into(), dataset, source, operations: Vec::new(), results: Vec::new(), report: None, data_settings: Default::default(), profile: None, part: None }
    }
}

/// An analysis worth offering when a file is opened, with its dialog's starting arguments.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Suggestion {
    pub dialog: String,
    pub label: String,
    pub args: Value,
}

/// What to offer for a file that follows a known profile, and a sentence saying what the file is.
///
/// - OpenAssess's results (profiles 1 and 2, PSTAT-1 sections 7.1 and 7.2): item analysis and reliability on the
///   points (`q01`, …) and descriptives of `total`; with profile 2, also item analysis of the letters marked (`r01`, …)
///   scored with the file's own key, which gives the answers chosen by score group.
/// - PrismQDA's case table (section 7.3): how often each code was applied (frequencies of the `_present` variables,
///   descriptives of the `_count` ones), and a pointer to Add Variables to match it with survey data on `case_id`.
pub fn profile_suggestions(p: &Project) -> Option<(String, Vec<Suggestion>)> {
    let by = |fallback: &str| p.app_version.split_whitespace().next().filter(|w| !w.starts_with(|c: char| c.is_ascii_digit())).unwrap_or(fallback).to_string();
    let names = |keep: &dyn Fn(&str) -> bool| -> Vec<String> { p.dataset.variables.iter().map(|v| v.name.clone()).filter(|n| keep(n)).collect() };
    match p.profile.as_deref() {
        Some(PROFILE_ITEM_RESULTS) | Some(PROFILE_ITEM_RESULTS_2) => {
            let numbered = |prefix: char| move |n: &str| n.len() >= 3 && n.starts_with(prefix) && n[1..].chars().all(|c| c.is_ascii_digit());
            let questions = names(&numbered('q'));
            if questions.len() < 2 {
                return None;
            }
            let mut out = Vec::new();
            let mut about = format!("This file is {}'s scored results: {} questions, with the points each sheet earned", by("OpenAssess"), questions.len());
            if let Some(part) = p.part_json() {
                let (keyed, points_only) = crate::analysis::item_analysis::key_from_assessment(&p.dataset, &part);
                if keyed.len() >= 2 {
                    let items: Vec<&String> = keyed.iter().map(|(v, _)| v).collect();
                    let key: Vec<&crate::analysis::item_analysis::KeyEntry> = keyed.iter().map(|(_, k)| k).collect();
                    out.push(Suggestion {
                        dialog: "item_analysis".into(),
                        label: "Item analysis of the answers marked, with the file's key".into(),
                        args: serde_json::json!({ "items": items, "key": key, "scored": false, "group_fraction": 0.27 }),
                    });
                    about += ", the letters marked and the key";
                    if !points_only.is_empty() {
                        let list = points_only.iter().map(u64::to_string).collect::<Vec<_>>().join(", ");
                        about += &if points_only.len() == 1 { format!(" (question {list} is analysed from its points only: its key differs by version or gives partial credit)") } else { format!(" (questions {list} are analysed from their points only: their keys differ by version or give partial credit)") };
                    }
                }
            }
            out.push(Suggestion { dialog: "item_analysis".into(), label: "Item analysis of the points".into(), args: serde_json::json!({ "items": questions, "key": [], "scored": true, "group_fraction": 0.27 }) });
            out.push(Suggestion { dialog: "reliability".into(), label: "Reliability of the questions".into(), args: serde_json::json!({ "items": questions }) });
            if p.dataset.find("total").is_some() {
                out.push(Suggestion { dialog: "descriptives".into(), label: "Descriptives of the total".into(), args: serde_json::json!({ "variables": ["total"] }) });
            }
            Some((about + ".", out))
        }
        Some(PROFILE_CASE_TABLE) => {
            let code = |suffix: &'static str| move |n: &str| n.starts_with("code_") && n.ends_with(suffix);
            let present = names(&code("_present"));
            let counts = names(&code("_count"));
            let mut out = Vec::new();
            if !present.is_empty() {
                out.push(Suggestion { dialog: "frequencies".into(), label: "How many cases each code appears in".into(), args: serde_json::json!({ "variables": present }) });
            }
            if !counts.is_empty() {
                out.push(Suggestion { dialog: "descriptives".into(), label: "Excerpts per case for each code".into(), args: serde_json::json!({ "variables": counts }) });
            }
            let about = format!(
                "This file is {}'s case table: {} cases, with {} code{} applied in each. To set it beside survey data, open the survey file and use Data › Add Variables with this file, matching on case_id.",
                by("PrismQDA"),
                p.dataset.n_rows(),
                present.len(),
                if present.len() == 1 { "" } else { "s" }
            );
            Some((about, out))
        }
        _ => None,
    }
}

/// Writes the project to bytes (a zip archive holding `project.json`).
pub fn to_bytes(project: &Project) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(project).map_err(|e| Error::Invalid(format!("The project could not be written: {e}")))?;
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(ENTRY, options).map_err(zip_error)?;
    zip.write_all(&json)?;
    // The profile's part, byte for byte, while the project still names that profile.
    if let (Some((name, bytes)), Some(profile)) = (&project.part, &project.profile) {
        if part_for(profile) == Some(name.as_str()) {
            zip.start_file(name.as_str(), options).map_err(zip_error)?;
            zip.write_all(bytes)?;
        }
    }
    Ok(zip.finish().map_err(zip_error)?.into_inner())
}

/// Reads a project from bytes, refusing anything that isn't a project this version can open.
pub fn from_bytes(bytes: &[u8]) -> Result<Project> {
    let not_project = || Error::Invalid("This file isn't a PrismStatistics project, or it is damaged.".into());
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| not_project())?;
    let entry = zip.by_name(ENTRY).map_err(|_| not_project())?;
    let json = crate::limits::read_capped(entry, "This project")?;
    let head: Value = serde_json::from_slice(&json).map_err(|_| not_project())?;
    if head.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(not_project());
    }
    let version = head.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version > VERSION as u64 {
        return Err(Error::Invalid(newer_message(head.get("app_version").and_then(Value::as_str))));
    }
    let mut project: Project = serde_json::from_value(head).map_err(|e| Error::Invalid(format!("The project is damaged and could not be read: {e}")))?;
    // A profile's part is kept as it is, to be written back when the project is saved. One that can't be read is
    // left out (the data are still the file's).
    if let Some(name) = project.profile.as_deref().and_then(part_for) {
        if let Ok(entry) = zip.by_name(name) {
            if let Ok(bytes) = crate::limits::read_capped(entry, "This project's part") {
                project.part = Some((name.to_string(), bytes));
            }
        }
    }
    // A hand-edited project could hold a dataset that doesn't add up; check it before anything uses it.
    project.dataset.check().map_err(|e| Error::Invalid(format!("The project is damaged and could not be read: {e}")))?;
    // Stored chart pictures are rebuilt from the parts the engine's own charts use; one that can't be is left out.
    for r in &mut project.results {
        r.bundle.charts.retain_mut(|c| match crate::svg_clean::clean(&c.svg) {
            Some(svg) => {
                c.svg = svg;
                true
            }
            None => false,
        });
    }
    Ok(project)
}

/// The refusal for a file in a newer format. PrismStatistics writes its bare version ("0.2.0"); another program writes
/// its name first ("OpenAssess 0.1.0"), so the name is added only when the version starts with a digit.
fn newer_message(app_version: Option<&str>) -> String {
    let by = match app_version.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) if v.starts_with(|c: char| c.is_ascii_digit()) => format!("PrismStatistics {v}"),
        Some(v) => v.to_string(),
        None => "a newer program".to_string(),
    };
    format!("This project was saved by {by} in a newer project format than this copy of PrismStatistics reads. Update PrismStatistics to open it.")
}

/// Saves transactionally: write a temporary file beside the target, flush it to disk, check it, then move it into place.
/// The version it replaces is kept beside it as `<name>.backup.pstat` (see [`backup_path`]), so a save made by mistake
/// can be undone by opening the backup. A crash at any moment leaves either the old file or the new one, never half of one.
pub fn save(path: &Path, project: &Project) -> Result<()> {
    let bytes = to_bytes(project)?;
    // Check what will be written before replacing anything.
    from_bytes(&bytes)?;
    if let Ok(old) = std::fs::read(path) {
        // Only a readable project is worth keeping: never let a damaged file replace a good backup.
        if from_bytes(&old).is_ok() {
            write_atomic(&backup_path(path), &old)?;
        }
    }
    write_atomic(path, &bytes)
}

/// Where [`save`] keeps the version a save replaced: study.pstat becomes study.backup.pstat.
pub fn backup_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("{stem}.backup.{EXTENSION}"))
}

/// Writes bytes to a temporary file beside `path`, flushes them to disk, reads them back, then renames the file into place.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = temp_path(path);
    let written = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        if std::fs::read(&tmp)? != bytes {
            return Err(std::io::Error::other("the file read back differs from what was written"));
        }
        // A virus scanner or file indexer can hold the target open for a moment; try again before giving up.
        let mut wait = 50;
        loop {
            match std::fs::rename(&tmp, path) {
                Ok(()) => return Ok(()),
                Err(e) if wait <= 800 && matches!(e.kind(), std::io::ErrorKind::PermissionDenied) => {
                    std::thread::sleep(std::time::Duration::from_millis(wait));
                    wait *= 2;
                }
                Err(e) => return Err(e),
            }
        }
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(written?)
}

pub fn load(path: &Path) -> Result<Project> {
    from_bytes(&std::fs::read(path)?)
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".saving");
    path.with_file_name(name)
}

fn zip_error(e: zip::result::ZipError) -> Error {
    Error::Invalid(format!("The project file could not be written: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Column, ValueLabel};

    fn sample() -> Project {
        let mut ds = Dataset::new("study");
        ds.add_variable("g", Column::Numeric(vec![Some(1.0), Some(2.0), None])).unwrap();
        ds.add_variable("note", Column::Text(vec![Some("a".into()), None, Some("ü".into())])).unwrap();
        let g = ds.variable_mut("g").unwrap();
        g.value_labels = vec![ValueLabel { value: 1.0, label: "One".into() }];
        g.missing_values = vec![9.0];
        let mut p = Project::new(ds, Some(Source { file_name: "study.csv".into(), report: ImportReport::default() }));
        p.operations.push(Operation { seq: 1, command: "frequencies".into(), args: serde_json::json!({ "variables": ["g"] }) });
        p
    }

    #[test]
    fn round_trips_through_a_file() {
        let dir = std::env::temp_dir().join(format!("prism-project-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("study.pstat");
        let p = sample();
        save(&path, &p).unwrap();
        assert_eq!(load(&path).unwrap(), p);
        // Saving again replaces the file and leaves no temporary file behind.
        save(&path, &p).unwrap();
        assert!(!temp_path(&path).exists());
        // The second save kept the first as the backup.
        assert_eq!(backup_path(&path), dir.join("study.backup.pstat"));
        assert_eq!(load(&backup_path(&path)).unwrap(), p);
        let mut changed = p.clone();
        changed.operations.clear();
        save(&path, &changed).unwrap();
        assert_eq!(load(&backup_path(&path)).unwrap(), p, "the backup is the version just replaced");
        assert_eq!(load(&path).unwrap(), changed);
        // A damaged file is never kept as the backup.
        std::fs::write(&path, b"damaged").unwrap();
        save(&path, &changed).unwrap();
        assert_eq!(load(&backup_path(&path)).unwrap(), p);
        // A dataset added to after reopening keeps giving new IDs.
        let mut back = load(&path).unwrap().dataset;
        let id = back.add_variable("z", Column::Numeric(vec![None, None, None])).unwrap();
        assert_eq!(id, 3);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_other_files_and_newer_versions() {
        assert!(from_bytes(b"not a zip").unwrap_err().to_string().contains("isn't a PrismStatistics project"));
        let mut p = serde_json::to_value(sample()).unwrap();
        p["version"] = serde_json::json!(VERSION + 1);
        p["app_version"] = serde_json::json!("9.0.0");
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(ENTRY, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(p.to_string().as_bytes()).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(from_bytes(&bytes).unwrap_err().to_string().contains("PrismStatistics 9.0.0"));
    }
}
