//! Scripts: the Paste button's destination and the batch runner's input.
//!
//! A script is plain text, one command per line: the command's name, a space, and its arguments as JSON, exactly as a
//! project records them. Lines starting with `#` are comments; blank lines are skipped. For example:
//!
//! ```text
//! # Exam study, spring term
//! open_file {"path": "C:\\Data\\exam.sav"}
//! update_variable {"name": "score", "missing_values": [-99]}
//! anova_oneway {"dependent": ["score"], "factor": "cond", "post_hoc": "tukey"}
//! ```
//!
//! Only commands that open data, change variables or cells, run an analysis, or export results may appear; anything
//! else (saving projects, settings, the AI connection) is refused with the line number, so a script can't change more
//! than it says.

use serde_json::{json, Value};

use crate::{Session, ANALYSES};

/// The file extension of a script.
pub const EXTENSION: &str = "pscript";

/// Commands a script may use besides the analyses.
const OTHERS: [&str; 24] = ["sort_cases", "rank_cases", "standardize", "reverse_score", "date_parts", "date_difference", "health_fix", "paste_cells", "open_file", "open_google_sheet", "update_variable", "set_cell", "compute_variable", "recode", "scale_score", "set_data_settings", "clear_results", "export", "open_project", "append_cases", "merge_variables", "aggregate", "restructure_long", "restructure_wide"];

/// One command of a script, with its line number (from 1).
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub number: usize,
    pub command: String,
    pub args: Value,
}

/// The line of a script that runs `command` with `args`.
pub fn line(command: &str, args: &Value) -> String {
    format!("{command} {}", serde_json::to_string(args).unwrap_or_else(|_| "{}".into()))
}

/// Reads a script. Fails with the line number and the problem at the first line that isn't a command.
pub fn parse(text: &str) -> Result<Vec<Line>, (usize, String)> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let number = i + 1;
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let (command, rest) = match t.find(|c: char| c.is_whitespace()) {
            Some(k) => (&t[..k], t[k..].trim()),
            None => (t, ""),
        };
        if !(ANALYSES.contains(&command) || OTHERS.contains(&command)) {
            return Err((number, format!("\"{command}\" isn't a command a script can run. Use an analysis (such as anova_oneway), open_file, update_variable, set_cell, clear_results or export.")));
        }
        let args: Value = if rest.is_empty() { json!({}) } else { serde_json::from_str(rest).map_err(|e| (number, format!("The arguments aren't valid JSON: {e}.")))? };
        if !args.is_object() {
            return Err((number, "The arguments must be a JSON object, in braces.".into()));
        }
        out.push(Line { number, command: command.to_string(), args });
    }
    Ok(out)
}

/// The files a script would write (exports and saved projects), in order, with whether each already exists, so the
/// window can show them and ask before a script replaces any.
pub fn outputs(text: &str) -> Vec<Value> {
    let Ok(lines) = parse(text) else { return Vec::new() };
    lines
        .iter()
        .filter_map(|l| {
            let path = l.args["path"].as_str()?;
            let ext = match l.command.as_str() {
                "export" => l.args["format"].as_str().map(|f| if f == "r" { "R" } else { f })?.to_string(),
                "save_project" => prism_engine::project::EXTENSION.to_string(),
                _ => return None,
            };
            let full = if prism_engine::import::extension(path).is_some_and(|e| e.eq_ignore_ascii_case(&ext)) { path.to_string() } else { format!("{path}.{ext}") };
            let refused = crate::local_path(&full).err();
            Some(json!({ "line": l.number, "command": l.command, "path": full, "exists": refused.is_none() && std::path::Path::new(&full).exists(), "refused": refused }))
        })
        .collect()
}

/// What a script run did: how many lines ran, the results it made, and the first error (with its line), if any. The
/// run stops at the first error so later lines never act on a state the script didn't expect. A script never replaces
/// an existing file (see `Session::may_write`).
pub fn run(session: &mut Session, text: &str) -> Value {
    run_with(session, text, &[])
}

/// As `run`, letting the script replace the existing files in `replace`, which the user agreed to.
pub fn run_with(session: &mut Session, text: &str, replace: &[String]) -> Value {
    let before = session.script_replace.replace(replace.to_vec());
    let r = run_lines(session, text);
    session.script_replace = before;
    r
}

fn run_lines(session: &mut Session, text: &str) -> Value {
    let lines = match parse(text) {
        Ok(l) => l,
        Err((line, message)) => return json!({ "ran": 0, "ids": [], "error": { "line": line, "message": message } }),
    };
    let mut ids: Vec<u64> = Vec::new();
    for (k, l) in lines.iter().enumerate() {
        match session.handle(&l.command, l.args.clone()) {
            Ok(v) => {
                if ANALYSES.contains(&l.command.as_str()) {
                    ids.extend(v.as_array().into_iter().flatten().filter_map(|o| o["id"].as_u64()));
                }
            }
            Err(message) => return json!({ "ran": k, "ids": ids, "error": { "line": l.number, "message": message } }),
        }
    }
    json!({ "ran": lines.len(), "ids": ids, "error": null })
}

/// The project's record as a script: what was done, in order, ready to save and run again on new data.
pub fn from_history(session: &Session) -> Result<String, String> {
    let p = session.project.as_ref().ok_or("Open a data file first.")?;
    let mut out = vec![format!("# PrismStatistics script ({} operations recorded)", p.operations.len())];
    for op in &p.operations {
        match op.command.as_str() {
            "import" => {
                let name = op.args["file_name"].as_str().unwrap_or("the data file");
                out.push(format!("# The data came from {name}. To run this script on it, give its full path:"));
                out.push(format!("# open_file {}", serde_json::to_string(&json!({ "path": name })).unwrap_or_default()));
            }
            c if ANALYSES.contains(&c) || OTHERS.contains(&c) => {
                let mut args = op.args.clone();
                // A cell edit records the old value too; the script needs only the new one.
                if c == "set_cell" {
                    if let Some(o) = args.as_object_mut() {
                        o.remove("old");
                    }
                }
                if c == "paste_cells" {
                    if let Some(o) = args.as_object_mut() {
                        o.remove("cells");
                        o.remove("new_cases");
                    }
                }
                // An append or merge read another file; a script names it by path.
                if c == "append_cases" || c == "merge_variables" {
                    if let Some(o) = args.as_object_mut() {
                        if let Some(name) = o.remove("file_name") {
                            out.push(format!("# The other file was {}; give its full path below.", name.as_str().unwrap_or("")));
                            o.insert("path".into(), name);
                        }
                    }
                }
                out.push(line(c, &args));
            }
            other => out.push(format!("# ({other} is not repeated by scripts)")),
        }
    }
    Ok(out.join("\n") + "\n")
}

/// How to call the batch runner.
pub const USAGE: &str = "run <script.pscript> [--data <file>] [--out <report.docx|.html|.pdf>] [--project <file.pstat>]";

/// The batch runner: `prism run` and `PrismStatistics.exe run`. Paragraphs go to standard output, progress to standard
/// error; the first error comes back with its line number.
pub fn batch(args: &[String]) -> Result<(), String> {
    let (script_path, flags) = args.split_first().ok_or_else(|| USAGE.to_string())?;
    let (mut data, mut out, mut project) = (None, None, None);
    let mut it = flags.iter();
    while let Some(f) = it.next() {
        let value = || -> Result<String, String> { Err(format!("{f} needs a value")) };
        match f.as_str() {
            "--data" => data = Some(it.next().cloned().map_or_else(value, Ok)?),
            "--out" => out = Some(it.next().cloned().map_or_else(value, Ok)?),
            "--project" => project = Some(it.next().cloned().map_or_else(value, Ok)?),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    let text = std::fs::read_to_string(crate::local_path(script_path)?).map_err(|e| format!("could not read {script_path}: {e}"))?;
    let mut s = Session::new();
    if let Some(d) = &data {
        let full = std::fs::canonicalize(crate::local_path(d)?).map_err(|e| format!("could not find {d}: {e}"))?;
        s.handle("open_file", json!({ "path": full.to_string_lossy().trim_start_matches(r"\\?\") }))?;
        eprintln!("Opened {d}.");
    }
    let r = run(&mut s, &text);
    let made: Vec<u64> = r["ids"].as_array().into_iter().flatten().filter_map(|v| v.as_u64()).collect();
    for o in s.handle("results", json!({}))?.as_array().into_iter().flatten().filter(|o| made.contains(&o["id"].as_u64().unwrap_or(0))) {
        println!("## {}", o["bundle"]["title"].as_str().unwrap_or(""));
        match o["paragraph"].as_str() {
            Some(p) => println!("{p}\n"),
            None => println!("(no paragraph: {})\n", o["bundle"]["status"]["reason"].as_str().unwrap_or("see the tables")),
        }
    }
    eprintln!("Ran {} command{}, {} result{}.", r["ran"], if r["ran"] == 1 { "" } else { "s" }, made.len(), if made.len() == 1 { "" } else { "s" });
    if let Some(o) = &out {
        let format = std::path::Path::new(o).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).filter(|e| ["docx", "html", "pdf"].contains(&e.as_str())).ok_or("--out must end in .docx, .html or .pdf")?;
        let w = s.handle("export", json!({ "format": format, "path": o }))?;
        eprintln!("Wrote {}.", w["path"].as_str().unwrap_or(o));
    }
    if let Some(pp) = &project {
        let w = s.handle("save_project", json!({ "path": pp }))?;
        eprintln!("Saved {}.", w["path"].as_str().unwrap_or(pp));
    }
    match r["error"].as_object() {
        Some(e) => Err(format!("{script_path}, line {}: {}", e["line"], e["message"].as_str().unwrap_or(""))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_read_with_their_numbers() {
        let s = "# a comment\n\nfrequencies {\"variables\": [\"g\"]}\n  descriptives {\"variables\":[\"y\"]}\n";
        let l = parse(s).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].number, l[0].command.as_str()), (3, "frequencies"));
        assert_eq!(l[1].args["variables"][0], "y");
        assert_eq!(parse("save_project {}").unwrap_err().0, 1);
        assert!(parse("frequencies {oops}").unwrap_err().1.contains("JSON"));
        assert!(parse("frequencies [1]").unwrap_err().1.contains("object"));
        assert_eq!(line("frequencies", &json!({ "variables": ["g"] })), "frequencies {\"variables\":[\"g\"]}");
    }

    #[test]
    fn a_script_runs_until_its_first_error_and_history_runs_again() {
        let mut s = Session::new();
        s.handle("open_text", json!({ "name": "demo.csv", "text": "g,y\n1,3\n1,4\n1,6\n2,5\n2,7\n2,9\n" })).unwrap();
        let r = run(&mut s, "frequencies {\"variables\": [\"g\"]}\nupdate_variable {\"name\": \"y\", \"missing_values\": [9]}\ndescriptives {\"variables\": [\"y\"]}\nnope {}\n");
        assert_eq!(r["error"]["line"], 4);
        assert_eq!(r["ran"], 0, "a parse error stops the script before anything runs");
        let r = run(&mut s, "frequencies {\"variables\": [\"g\"]}\nupdate_variable {\"name\": \"y\", \"missing_values\": [9]}\ndescriptives {\"variables\": [\"y\"]}\ndescriptives {\"variables\": [\"zzz\"]}\n");
        assert_eq!(r["ran"], 3);
        assert_eq!(r["ids"].as_array().unwrap().len(), 2);
        assert_eq!(r["error"]["line"], 4);
        let text = from_history(&s).unwrap();
        assert!(text.contains("# open_file {\"path\":\"demo.csv\"}"), "{text}");
        assert!(text.contains("update_variable {"), "{text}");
        // The history (minus its comment lines) runs again on the same data.
        let mut again = Session::new();
        again.handle("open_text", json!({ "name": "demo.csv", "text": "g,y\n1,3\n1,4\n1,6\n2,5\n2,7\n2,9\n" })).unwrap();
        let r = run(&mut again, &text);
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["ids"].as_array().unwrap().len(), 2);
    }
}
