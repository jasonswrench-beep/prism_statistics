//! The PrismStatistics desktop app: a window around the React interface, with one door to the engine.
//!
//! The interface calls `invoke('call', { cmd, args })`; this hands it to the same command layer that `prism serve`
//! and the tests use (crates/prism-api), so what ships is what was tested. Nothing here decides anything.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use prism_api::Session;
use serde_json::Value;
use tauri::{Manager, RunEvent, State};

struct Engine(Mutex<Session>);

/// A project the system asked the program to open (macOS sends a file opened from Finder as an event, not as an
/// argument), held until the window asks for it.
#[derive(Default)]
struct Opened(Mutex<Option<String>>);

/// Runs one engine command. Async so a long analysis never freezes the window.
#[tauri::command]
async fn call(cmd: String, args: Option<Value>, engine: State<'_, Engine>) -> Result<Value, String> {
    let args = args.unwrap_or(Value::Null);
    // Progress and Cancel answer at once, without waiting for the analysis that holds the engine.
    if let Some(answer) = prism_api::recovery::job_command(&cmd, &args) {
        return answer;
    }
    if cmd == "ai_draft" {
        return ai_draft(args, &engine).await;
    }
    // Commands catch their own failures (Session::handle), so a poisoned lock still holds a usable session.
    let mut session = engine.0.lock().unwrap_or_else(|e| e.into_inner());
    session.handle(&cmd, args)
}

/// An AI draft takes from seconds to a minute, so the model writes without holding the engine: the rest of the window keeps
/// working meanwhile. Only the first and last steps (fast) take the lock.
async fn ai_draft(args: Value, engine: &State<'_, Engine>) -> Result<Value, String> {
    let input = engine.0.lock().unwrap_or_else(|e| e.into_inner()).handle("ai_input", args.clone())?;
    let for_model = input.clone();
    let written = tauri::async_runtime::spawn_blocking(move || prism_api::ai_write(&for_model)).await.map_err(|e| e.to_string())??;
    let store = serde_json::json!({ "id": args["id"], "input": input, "raw": written["raw"], "seconds": written["seconds"] });
    engine.0.lock().unwrap_or_else(|e| e.into_inner()).handle("ai_store", store)
}

/// The project file PrismStatistics was started with (double-clicking a .pstat file in File Explorer, or opening one from
/// Finder), if any.
#[tauri::command]
fn opened_with(opened: State<'_, Opened>) -> Option<String> {
    if let Some(p) = opened.0.lock().unwrap_or_else(|e| e.into_inner()).take() {
        return Some(p);
    }
    std::env::args_os().skip(1).map(std::path::PathBuf::from).find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pstat")) && p.is_file()).map(|p| p.to_string_lossy().into_owned())
}

fn main() {
    // An AI assistant starts PrismStatistics.exe --mcp: answer it over standard input and output, with no window.
    if std::env::args().skip(1).any(|a| a == "--mcp") {
        if let Err(e) = prism_api::mcp::serve_stdio() {
            eprintln!("PrismStatistics: {e}");
            std::process::exit(1);
        }
        return;
    }
    // prismstatistics-app.exe run script.pscript ...: the batch runner, printing to the terminal it was started from.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("run") {
        attach_console();
        if let Err(e) = prism_api::script::batch(&args[1..]) {
            eprintln!("PrismStatistics: {e}");
            std::process::exit(1);
        }
        return;
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Unsaved work is copied to the app's own data folder every half minute, for recovery after a crash.
            let session = match app.path().app_local_data_dir() {
                Ok(dir) => Session::with_recovery(dir.join("recovery")),
                Err(_) => Session::new(),
            };
            app.manage(Engine(Mutex::new(session)));
            app.manage(Opened::default());
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(30));
                autosave(&handle);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![call, opened_with])
        .build(tauri::generate_context!())
        .expect("the PrismStatistics window could not start");
    app.run(|handle, event| {
        // On the way out, bring the recovery copy up to date: removed when everything was saved, kept when it wasn't,
        // so work closed without saving can still be recovered at the next start.
        if let RunEvent::Exit = event {
            autosave(handle);
        }
        // macOS: a .pstat file opened from Finder, at start-up or while the program runs.
        #[cfg(target_os = "macos")]
        if let RunEvent::Opened { urls } = &event {
            use tauri::Emitter;
            for path in urls.iter().filter_map(|u| u.to_file_path().ok()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pstat"))) {
                let path = path.to_string_lossy().into_owned();
                if let Some(opened) = handle.try_state::<Opened>() {
                    *opened.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(path.clone());
                }
                let _ = handle.emit("open-project", path);
            }
        }
    });
}

fn autosave(handle: &tauri::AppHandle) {
    if let Some(engine) = handle.try_state::<Engine>() {
        let mut session = engine.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = session.handle("autosave", Value::Null);
    }
}

/// A windowed program has no console of its own; borrow the one it was started from so the batch runner's output shows.
#[cfg(windows)]
fn attach_console() {
    extern "system" {
        fn AttachConsole(process: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    // SAFETY: AttachConsole takes a process id and has no other preconditions; failure (no parent console) is harmless.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}
