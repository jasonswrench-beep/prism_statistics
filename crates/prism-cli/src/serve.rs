//! `prism serve`: the command layer over HTTP, for trying the interface in a browser and for automated interface checks.
//!
//! `POST /api/<command>` with a JSON body runs one command and answers `{"ok": value}` or `{"error": message}`.
//! Any other GET serves the built interface from `--ui` (default `app/ui/dist`). It listens on 127.0.0.1 only.
//!
//! A web page open in the same browser must not be able to drive it, so a command is run only when the request is JSON
//! (`Content-Type: application/json`), is addressed to this server by name (Host 127.0.0.1 or localhost and this port,
//! which stops DNS rebinding), comes from this server's own page if it names an origin, and carries the token printed at
//! start (`X-Prism-Token`). The interface finds the token in the page it is served, which other sites can't read.
//! `write_script` is left out: nothing here writes a file the user didn't export.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use prism_api::Session;
use tiny_http::{Header, Method, Response, Server};

pub fn serve(args: &[String]) -> Result<(), String> {
    let mut port = 4317u16;
    let mut ui = PathBuf::from("app/ui/dist");
    let mut recovery = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--port" => port = it.next().and_then(|p| p.parse().ok()).ok_or("--port needs a number")?,
            "--ui" => ui = PathBuf::from(it.next().ok_or("--ui needs a folder")?),
            "--recovery" => recovery = Some(PathBuf::from(it.next().ok_or("--recovery needs a folder")?)),
            other => return Err(format!("unknown option {other}")),
        }
    }
    let server = Server::http(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    let token = new_token();
    eprintln!("PrismStatistics test server on http://127.0.0.1:{port} (interface from {})", ui.display());
    eprintln!("API token (send as X-Prism-Token): {token}");
    let token = Arc::new(token);
    let session = Arc::new(Mutex::new(match recovery {
        Some(dir) => Session::with_recovery(dir),
        None => Session::new(),
    }));
    // Several workers, so progress and Cancel are answered while an analysis runs, as in the desktop app.
    let server = Arc::new(server);
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let (server, session, ui, token) = (server.clone(), session.clone(), ui.clone(), token.clone());
            std::thread::spawn(move || {
                let mut last = Instant::now();
                loop {
                    let request = match server.recv_timeout(Duration::from_secs(5)) {
                        Ok(Some(r)) => Some(r),
                        Ok(None) => None,
                        Err(_) => return,
                    };
                    if last.elapsed() >= Duration::from_secs(30) {
                        last = Instant::now();
                        if let Ok(mut s) = session.try_lock() {
                            let _ = s.handle("autosave", serde_json::Value::Null);
                        }
                    }
                    if let Some(request) = request {
                        answer(request, &session, &ui, &token, port);
                    }
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
    Ok(())
}

/// A random token for this run of the server.
fn new_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..2)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u32(i);
            h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Whether a command request comes from this server's own page (see the module note).
fn trusted(request: &tiny_http::Request, token: &str, port: u16) -> Result<(), &'static str> {
    let get = |name: &str| request.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name)).map(|h| h.value.as_str().to_string());
    let hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    if !get("Host").is_some_and(|h| hosts.iter().any(|ok| h.eq_ignore_ascii_case(ok))) {
        return Err("wrong host");
    }
    if let Some(origin) = get("Origin") {
        if !hosts.iter().any(|h| origin.eq_ignore_ascii_case(&format!("http://{h}"))) {
            return Err("wrong origin");
        }
    }
    if !get("Content-Type").is_some_and(|c| c.split(';').next().is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))) {
        return Err("not JSON");
    }
    if get("X-Prism-Token").as_deref() != Some(token) {
        return Err("no token");
    }
    Ok(())
}

fn answer(mut request: tiny_http::Request, session: &Mutex<Session>, ui: &Path, token: &str, port: u16) {
    let url = request.url().split('?').next().unwrap_or("/").to_string();
    let refused = (request.method() == &Method::Post && url.starts_with("/api/")).then(|| trusted(&request, token, port)).and_then(Result::err);
    let response = if let Some(why) = refused {
        Response::from_string(format!("Refused: {why}.")).with_status_code(403)
    } else if request.method() == &Method::Post && url.starts_with("/api/") && &url[5..] == "write_script" {
        Response::from_string(serde_json::json!({ "error": "The test server doesn't write scripts; use the desktop app." }).to_string()).with_header(header("Content-Type", "application/json"))
    } else if request.method() == &Method::Post && url.starts_with("/api/") {
        let mut body = String::new();
        let _ = request.as_reader().read_to_string(&mut body);
        let args = if body.trim().is_empty() { serde_json::json!({}) } else { serde_json::from_str(&body).unwrap_or(serde_json::Value::Null) };
        let cmd = &url[5..];
        let result = match prism_api::recovery::job_command(cmd, &args) {
            Some(r) => r,
            None => session.lock().unwrap_or_else(|e| e.into_inner()).handle(cmd, args),
        };
        let answer = match result {
            Ok(v) => serde_json::json!({ "ok": v }),
            Err(e) => serde_json::json!({ "error": e }),
        };
        Response::from_string(answer.to_string()).with_header(header("Content-Type", "application/json"))
    } else {
        static_file(ui, &url, token)
    };
    let _ = request.respond(response);
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("static header")
}

fn static_file(root: &Path, url: &str, token: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let rel = url.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    // Never serve anything outside the interface folder.
    if rel.split('/').any(|part| part == ".." || part.contains(':') || part.contains('\\')) {
        return Response::from_string("Not found").with_status_code(404);
    }
    let path = root.join(rel);
    let path = if path.is_file() { path } else { root.join("index.html") };
    match std::fs::read(&path) {
        Ok(bytes) => {
            // The page carries the token, for the interface's own requests.
            let bytes = if path.extension().is_some_and(|e| e == "html") {
                String::from_utf8_lossy(&bytes).replacen("</head>", &format!("<meta name=\"prism-token\" content=\"{token}\"></head>"), 1).into_bytes()
            } else {
                bytes
            };
            let kind = match path.extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html; charset=utf-8",
                Some("js") => "text/javascript",
                Some("css") => "text/css",
                Some("svg") => "image/svg+xml",
                Some("woff2") => "font/woff2",
                Some("png") => "image/png",
                Some("ico") => "image/x-icon",
                _ => "application/octet-stream",
            };
            Response::from_data(bytes).with_header(header("Content-Type", kind))
        }
        Err(_) => Response::from_string("The interface isn't built yet. Run npm run build in app/ui.").with_status_code(404),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    /// Sends one raw HTTP request and returns the status code and body.
    fn send(port: u16, request: &str) -> (u16, String) {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        let code = out.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        (code, out.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
    }

    fn post(port: u16, cmd: &str, headers: &str) -> (u16, String) {
        send(port, &format!("POST /api/{cmd} HTTP/1.1\r\nConnection: close\r\n{headers}Content-Length: 2\r\n\r\n{{}}"))
    }

    #[test]
    fn only_the_servers_own_page_can_run_commands() {
        let port = 47_000 + (std::process::id() % 2000) as u16;
        let ui = std::env::temp_dir().join(format!("prism-serve-ui-{}", std::process::id()));
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(ui.join("index.html"), "<html><head><title>t</title></head><body></body></html>").unwrap();
        let args = vec!["--port".to_string(), port.to_string(), "--ui".to_string(), ui.to_string_lossy().into_owned()];
        std::thread::spawn(move || super::serve(&args));
        std::thread::sleep(std::time::Duration::from_millis(300));
        // The page carries the token.
        let (code, page) = send(port, &format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"));
        assert_eq!(code, 200);
        let token = page.split("name=\"prism-token\" content=\"").nth(1).and_then(|r| r.split('"').next()).unwrap().to_string();
        assert_eq!(token.len(), 32);
        let ok = format!("Host: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nX-Prism-Token: {token}\r\n");
        assert_eq!(post(port, "project_state", &ok).0, 200);
        assert_eq!(post(port, "project_state", &format!("{ok}Origin: http://localhost:{port}\r\n")).0, 200);
        // Anything else is refused before the command runs.
        for bad in [
            format!("Host: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n"),
            format!("Host: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nX-Prism-Token: wrong\r\n"),
            format!("Host: 127.0.0.1:{port}\r\nContent-Type: text/plain\r\nX-Prism-Token: {token}\r\n"),
            format!("Host: evil.example:{port}\r\nContent-Type: application/json\r\nX-Prism-Token: {token}\r\n"),
            format!("{ok}Origin: http://evil.example\r\n"),
        ] {
            assert_eq!(post(port, "project_state", &bad).0, 403, "{bad}");
        }
        let (code, body) = post(port, "write_script", &ok);
        assert!(code == 200 && body.contains("doesn't write scripts"), "{body}");
        std::fs::remove_dir_all(&ui).ok();
    }
}
