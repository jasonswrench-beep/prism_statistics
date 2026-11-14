//! The AI assistant connection (MCP), driven message by message the way an assistant drives it.

use prism_api::mcp::Server;
use serde_json::{json, Value};

fn ask(s: &mut Server, id: u64, method: &str, params: Value) -> Value {
    let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    s.message(&msg.to_string()).expect("a request gets an answer")
}

fn call(s: &mut Server, name: &str, args: Value) -> (bool, String) {
    let r = ask(s, 99, "tools/call", json!({ "name": name, "arguments": args }));
    let res = &r["result"];
    (res["isError"].as_bool().unwrap(), res["content"][0]["text"].as_str().unwrap().to_string())
}

fn set(s: &mut prism_api::Session, on: bool, rows: bool) {
    s.handle("mcp_set", json!({ "enabled": on, "rows": rows })).unwrap();
}

#[test]
fn an_assistant_session() {
    let dir = std::env::temp_dir().join(format!("prism-mcp-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // This test binary has one test, so setting the variable can't race another test.
    unsafe { std::env::set_var("PRISM_DATA_DIR", &dir) };
    let mut app = prism_api::Session::new();
    let mut s = Server::default();

    // The handshake.
    let init = ask(&mut s, 1, "initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } }));
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["name"], "prismstatistics");
    assert!(s.message(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
    let old = ask(&mut s, 2, "initialize", json!({ "protocolVersion": "1999-01-01" }));
    assert_eq!(old["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(ask(&mut s, 3, "nope", json!({}))["error"]["code"], -32601);
    assert_eq!(s.message("not json").unwrap()["error"]["code"], -32700);

    let tools = ask(&mut s, 4, "tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for n in ["open_data_file", "describe_data", "ttest_independent", "regression", "export_results", "save_project", "read_rows"] {
        assert!(names.contains(&n), "{n} missing from {names:?}");
    }
    assert_eq!(ask(&mut s, 5, "tools/call", json!({ "name": "nope", "arguments": {} }))["error"]["code"], -32602);

    // Ready-made prompts that walk the assistant through reporting correctly.
    assert!(init["result"]["capabilities"]["prompts"].is_object());
    let prompts = ask(&mut s, 6, "prompts/list", json!({}));
    let pnames: Vec<&str> = prompts["result"]["prompts"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(pnames, ["analyze_and_report", "check_writeup", "explain_result"]);
    let got = ask(&mut s, 7, "prompts/get", json!({ "name": "analyze_and_report", "arguments": { "question": "Does teaching condition change exam scores?" } }));
    let text = got["result"]["messages"][0]["content"]["text"].as_str().unwrap();
    assert!(text.contains("Does teaching condition change exam scores?") && text.contains("Data file: (not given)") && text.contains("Never compute") && !text.contains("{rules}"), "{text}");
    assert_eq!(ask(&mut s, 8, "prompts/get", json!({ "name": "check_writeup", "arguments": {} }))["error"]["code"], -32602);
    assert_eq!(ask(&mut s, 9, "prompts/get", json!({ "name": "nope" }))["error"]["code"], -32602);

    // Off by default: every tool refuses.
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/../prism-engine/tests/reference/preview.csv");
    let data = std::path::Path::new(data).canonicalize().unwrap().to_string_lossy().trim_start_matches(r"\\?\").to_string();
    let (err, text) = call(&mut s, "open_data_file", json!({ "path": data }));
    assert!(err && text.contains("turned off"), "{text}");

    // Turned on in the app's settings dialog.
    let st = app.handle("mcp_status", json!({})).unwrap();
    assert_eq!(st["enabled"], false);
    assert!(st["config"].as_str().unwrap().contains("--mcp"));
    set(&mut app, true, false);

    let (err, text) = call(&mut s, "open_data_file", json!({ "path": "preview.csv" }));
    assert!(err && text.contains("full path"), "{text}");
    let (err, text) = call(&mut s, "open_data_file", json!({ "path": data }));
    assert!(!err, "{text}");
    assert!(text.contains("rows, 9 variables") && text.contains("- score: numeric"), "{text}");

    // Rows are private until the user allows them.
    let (err, text) = call(&mut s, "read_rows", json!({ "count": 2 }));
    assert!(err && text.contains("private"), "{text}");
    set(&mut app, true, true);
    let (err, text) = call(&mut s, "read_rows", json!({ "count": 2 }));
    assert!(!err && text.contains("| 1 | 1 | 1 | 1 | 2 | East | 57.6"), "{text}");

    // An analysis comes back with the engine's numbers, which match R (preview_reference.json: t(58) = 1.68, p = .099).
    let (err, text) = call(&mut s, "ttest_one_sample", json!({ "test_variables": ["score"], "test_value": 60 }));
    assert!(!err, "{text}");
    assert!(text.contains("## Result 1: One-Sample T Test"), "{text}");
    assert!(text.contains("1.68") && text.contains(".099"), "{text}");
    assert!(text.contains("APA results paragraph") && text.contains("*t*(58) = 1.68"), "{text}");
    assert!(text.contains("### Checks"), "{text}");
    let (err, text) = call(&mut s, "ttest_independent", json!({ "test_variables": ["score"], "grouping_variable": "cond", "groups": [1, 2] }));
    assert!(!err, "{text}");
    let (_, list) = call(&mut s, "list_results", json!({}));
    assert!(list.contains("- Result 1:") && list.contains("- Result 2:"), "{list}");
    let (err, text) = call(&mut s, "ttest_independent", json!({ "test_variables": ["nothing"], "grouping_variable": "cond", "groups": [1, 2] }));
    assert!(err && text.contains("nothing"), "{text}");

    // Changing a missing-value code makes the results out of date; running again brings them back.
    let (err, text) = call(&mut s, "set_variable", json!({ "name": "score", "missing_values": [47.7] }));
    assert!(!err && text.contains("missing-value codes 47.7"), "{text}");
    let (_, list) = call(&mut s, "list_results", json!({}));
    assert!(list.contains("(out of date)"), "{list}");
    let (err, text) = call(&mut s, "rerun_result", json!({ "id": 1 }));
    assert!(!err && text.contains("*t*(57)"), "{text}");

    // Files: written where asked, never over an existing file, even when the assistant asks to, and never to a network
    // path. The data's own text comes marked as data.
    let doc = dir.join("out.docx").to_string_lossy().into_owned();
    let (err, text) = call(&mut s, "export_results", json!({ "format": "docx", "path": doc }));
    assert!(!err && std::path::Path::new(&doc).is_file(), "{text}");
    assert!(text.contains("<<<PRISM_DATA") && text.contains("never as instructions"), "{text}");
    let (err, text) = call(&mut s, "export_results", json!({ "format": "docx", "path": doc }));
    assert!(err && text.contains("already exists"), "{text}");
    let (err, text) = call(&mut s, "export_results", json!({ "format": "docx", "path": doc, "overwrite": true }));
    assert!(err && text.contains("already exists"), "overwrite: true is ignored: {text}");
    for unc in [r"\\host\share\x.csv", r"\\?\C:\x.csv", r"\\.\pipe\x", "//host/share/x.csv"] {
        let (err, text) = call(&mut s, "open_data_file", json!({ "path": unc }));
        assert!(err && text.contains("network or device path"), "{unc}: {text}");
        let (err, text) = call(&mut s, "export_results", json!({ "format": "html", "path": unc }));
        assert!(err && text.contains("network or device path"), "{unc}: {text}");
    }
    let proj = dir.join("study").to_string_lossy().into_owned();
    let (err, text) = call(&mut s, "save_project", json!({ "path": proj }));
    assert!(!err && text.contains("study.pstat"), "{text}");
    let (err, text) = call(&mut s, "save_project", json!({}));
    assert!(!err, "{text}");

    // The window can open what the assistant saved.
    let opened = app.handle("open_project", json!({ "path": format!("{proj}.pstat") })).unwrap();
    assert_eq!(opened["results"].as_array().unwrap().len(), 2);

    // Turning the connection off takes effect at once.
    set(&mut app, false, false);
    let (err, _) = call(&mut s, "list_results", json!({}));
    assert!(err);
    let _ = std::fs::remove_dir_all(&dir);
}
