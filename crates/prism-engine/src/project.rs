//! The project file (`.pstat`, provisional extension).
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
}

impl Project {
    pub fn new(dataset: Dataset, source: Option<Source>) -> Self {
        Project { format: FORMAT.into(), version: VERSION, app_version: env!("CARGO_PKG_VERSION").into(), dataset, source, operations: Vec::new(), results: Vec::new(), report: None, data_settings: Default::default() }
    }
}

/// Writes the project to bytes (a zip archive holding `project.json`).
pub fn to_bytes(project: &Project) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(project).map_err(|e| Error::Invalid(format!("The project could not be written: {e}")))?;
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(ENTRY, options).map_err(zip_error)?;
    zip.write_all(&json)?;
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
        let by = head.get("app_version").and_then(Value::as_str).unwrap_or("a newer version");
        return Err(Error::Invalid(format!("This project was saved by PrismStatistics {by}, which is newer than this copy. Update PrismStatistics to open it.")));
    }
    let mut project: Project = serde_json::from_value(head).map_err(|e| Error::Invalid(format!("The project is damaged and could not be read: {e}")))?;
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
