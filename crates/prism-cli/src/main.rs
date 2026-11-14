//! `prism`: a command-line harness for the engine, for testing and scripting.
//!
//! ```text
//! prism info data.csv
//! prism ttest data.csv --test stress --group site --groups 1,2 [--student] [--level 0.90]
//!       [--missing site=9 --missing stress=-99] [--labels site=1:Urban,2:Rural]
//! ```
//! `ttest` prints the result bundle as JSON, followed by the template APA paragraph when there is one.
//!
//! `prism serve [--port 4317] [--ui app/ui/dist]` runs the command layer over HTTP on 127.0.0.1 so the interface can be
//! tried and tested in an ordinary browser. It answers only on the loopback address.
//!
//! `prism mcp` runs the connection for AI assistants (Model Context Protocol) over standard input and output.
//!
//! `prism run <script.pscript> [--data <file>] [--out <report.docx|.html|.pdf>] [--project <file.pstat>]` is the batch
//! runner: it opens the data (when given), runs the script's commands in order, stops at the first error with its line
//! number, and writes the results as a report and/or a project. Each result's paragraph is printed as it is made.

mod serve;

use std::path::Path;
use std::process::ExitCode;

use prism_engine::analysis::ttest_independent::{self, Estimator, Spec};
use prism_engine::model::ValueLabel;
use prism_engine::{apa, import, Column, Dataset};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("prism: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let (cmd, rest) = args.split_first().ok_or_else(usage)?;
    match cmd.as_str() {
        "info" => info(rest),
        "ttest" => ttest(rest),
        "serve" => serve::serve(rest),
        "mcp" | "--mcp" => prism_api::mcp::serve_stdio(),
        "run" => prism_api::script::batch(rest),
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command \"{other}\"\n{}", usage())),
    }
}

fn usage() -> String {
    "usage:\n  prism info <file.csv>\n  prism serve [--port 4317] [--ui <dir>]
  prism mcp
  prism run <script.pscript> [--data <file>] [--out <report.docx|.html|.pdf>] [--project <file.pstat>]
  prism ttest <file.csv> --test <var>[,<var>...] --group <var> --groups <a>,<b> [--student] [--level 0.95] [--missing var=code[,code]] [--labels var=code:label[,code:label]]".into()
}

fn load(path: &str) -> Result<Dataset, String> {
    import::read_file(Path::new(path)).map(|(ds, _)| ds).map_err(|e| e.to_string())
}

fn info(args: &[String]) -> Result<(), String> {
    let path = args.first().ok_or_else(usage)?;
    let (ds, report) = import::read_file(Path::new(path)).map_err(|e| e.to_string())?;
    println!("{} rows, {} variables", report.rows, report.variables);
    for (from, to) in &report.renamed {
        println!("  renamed \"{from}\" to {to}");
    }
    for (v, c) in ds.variables.iter().zip(&ds.columns) {
        let (kind, empty) = match c {
            Column::Numeric(x) => ("numeric", x.iter().filter(|v| v.is_none()).count()),
            Column::Text(x) => ("text", x.iter().filter(|v| v.is_none()).count()),
        };
        println!("  {:<24} {:<8} {} empty", v.name, kind, empty);
    }
    Ok(())
}

fn ttest(args: &[String]) -> Result<(), String> {
    let (path, flags) = args.split_first().ok_or_else(usage)?;
    let mut ds = load(path)?;
    let mut spec = Spec { test_variables: Vec::new(), grouping_variable: String::new(), groups: (1.0, 2.0), estimator: Estimator::Welch, level: 0.95, alternative: Default::default(), bayes: Default::default(), prior_scale: Default::default() };
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--test" => spec.test_variables = value()?.split(',').map(str::to_string).collect(),
            "--group" => spec.grouping_variable = value()?,
            "--groups" => {
                let v = value()?;
                let (a, b) = v.split_once(',').ok_or("--groups takes two codes, like 1,2")?;
                spec.groups = (number(a)?, number(b)?);
            }
            "--student" => spec.estimator = Estimator::Student,
            "--level" => spec.level = number(&value()?)?,
            "--missing" => {
                let v = value()?;
                let (var, codes) = v.split_once('=').ok_or("--missing takes var=code")?;
                let codes = codes.split(',').map(number).collect::<Result<Vec<_>, _>>()?;
                ds.variable_mut(var).map_err(|e| e.to_string())?.missing_values = codes;
            }
            "--labels" => {
                let v = value()?;
                let (var, pairs) = v.split_once('=').ok_or("--labels takes var=code:label")?;
                let mut labels = Vec::new();
                for pair in pairs.split(',') {
                    let (code, label) = pair.split_once(':').ok_or("each label is code:label")?;
                    labels.push(ValueLabel { value: number(code)?, label: label.to_string() });
                }
                ds.variable_mut(var).map_err(|e| e.to_string())?.value_labels = labels;
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    let results = ttest_independent::run(&ds, &spec).map_err(|e| e.to_string())?;
    for r in &results {
        println!("{}", serde_json::to_string_pretty(r).map_err(|e| e.to_string())?);
        if let Some(p) = apa::independent_t_paragraph(r) {
            println!("\n{p}\n");
        }
    }
    Ok(())
}

fn number(s: &str) -> Result<f64, String> {
    s.trim().parse::<f64>().map_err(|_| format!("\"{s}\" is not a number"))
}
