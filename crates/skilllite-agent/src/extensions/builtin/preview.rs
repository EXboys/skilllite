//! preview_server: local HTTP file server for previewing output.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;

use crate::types::{ToolDefinition, FunctionDef};

use super::{get_path_arg, normalize_path, resolve_within_workspace_or_output};

// ─── Tool definition ────────────────────────────────────────────────────────

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDef {
            name: "preview_server".to_string(),
            description: "Start a local HTTP server to preview HTML files in the browser. Specify the directory to serve.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "directory_path": {
                        "type": "string",
                        "description": "Directory to serve (relative to workspace). Also accepts 'path'."
                    },
                    "path": {
                        "type": "string",
                        "description": "Alias for directory_path"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Port number (default: 8765)"
                    },
                    "open_browser": {
                        "type": "boolean",
                        "description": "Whether to open browser automatically (default: true)",
                        "default": true
                    }
                },
                "required": []
            }),
        },
    }]
}

// ─── Server state ───────────────────────────────────────────────────────────

static ACTIVE_PREVIEW: Mutex<Option<PreviewServerState>> = Mutex::new(None);

struct PreviewServerState {
    serve_dir: String,
    port: u16,
}

// ─── Execution ──────────────────────────────────────────────────────────────

pub(super) fn execute_preview_server(args: &Value, workspace: &Path) -> Result<String> {
    let dir_path = get_path_arg(args, true)
        .ok_or_else(|| anyhow::anyhow!("'directory_path' or 'path' is required"))?;
    let requested_port = args
        .get("port")
        .and_then(|v| v.as_u64())
        .unwrap_or(8765) as u16;
    let should_open_browser = args
        .get("open_browser")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let resolved = resolve_within_workspace_or_output(&dir_path, workspace)?;

    let (serve_dir, target_file) = if resolved.is_file() {
        let fname = resolved.file_name().map(|f| f.to_string_lossy().to_string());
        (resolved.parent().unwrap_or(&resolved).to_path_buf(), fname)
    } else {
        (resolved.clone(), None)
    };

    if !serve_dir.exists() {
        anyhow::bail!("Path not found: {}", dir_path);
    }

    let serve_dir_str = serve_dir.to_string_lossy().to_string();

    {
        let guard = ACTIVE_PREVIEW.lock().map_err(|e| anyhow::anyhow!("Preview lock poisoned: {}", e))?;
        if let Some(ref state) = *guard {
            if state.serve_dir == serve_dir_str {
                let url = build_preview_url(state.port, target_file.as_deref());
                if should_open_browser {
                    open_browser(&url);
                }
                return Ok(format!(
                    "Preview server already running at {}\n\n\
                     Open in browser: {}\n\
                     Serving directory: {}\n\
                     (Browser opened with fresh page.)",
                    url, url, serve_dir_str
                ));
            }
        }
    }

    let listener = {
        let mut bound = None;
        for p in requested_port..requested_port.saturating_add(20).min(65535) {
            match std::net::TcpListener::bind(("127.0.0.1", p)) {
                Ok(l) => {
                    bound = Some((l, p));
                    break;
                }
                Err(_) => continue,
            }
        }
        bound
    };

    let (listener, used_port) = match listener {
        Some((l, p)) => (l, p),
        None => anyhow::bail!(
            "Could not bind to port {} (tried {}-{})",
            requested_port,
            requested_port,
            requested_port + 19
        ),
    };

    {
        let mut guard = ACTIVE_PREVIEW.lock().map_err(|e| anyhow::anyhow!("Preview lock poisoned: {}", e))?;
        *guard = Some(PreviewServerState {
            serve_dir: serve_dir_str.clone(),
            port: used_port,
        });
    }

    let serve_dir_clone = serve_dir.clone();
    std::thread::Builder::new()
        .name("preview-server".to_string())
        .spawn(move || {
            run_file_server(listener, &serve_dir_clone);
        })
        .context("Failed to spawn preview server thread")?;

    let url = build_preview_url(used_port, target_file.as_deref());
    if should_open_browser {
        open_browser(&url);
    }

    Ok(format!(
        "Preview server started at {}\n\n\
         Open in browser: {}\n\
         Serving directory: {}\n\
         (Server runs in background. Stops when you exit.)",
        url, url, serve_dir_str
    ))
}

// ─── HTTP server internals ──────────────────────────────────────────────────

fn build_preview_url(port: u16, filename: Option<&str>) -> String {
    match filename {
        Some(f) => format!("http://127.0.0.1:{}/{}", port, f),
        None => format!("http://127.0.0.1:{}", port),
    }
}

fn open_browser(url: &str) {
    let _ = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else if cfg!(target_os = "linux") {
        std::process::Command::new("xdg-open").arg(url).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
    } else {
        Ok(std::process::Command::new("true").spawn().unwrap())
    };
}

fn run_file_server(listener: std::net::TcpListener, serve_dir: &Path) {
    use std::io::{BufRead, BufReader, Write};

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let reader = BufReader::new(&stream);
        let request_line = match reader.lines().next() {
            Some(Ok(line)) => line,
            _ => continue,
        };

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 || parts[0] != "GET" {
            let _ = stream.write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n");
            continue;
        }

        let request_path = parts[1];
        let clean_path = request_path.split('?').next().unwrap_or("/");
        let decoded = url_decode(clean_path);
        let rel = decoded.trim_start_matches('/');
        let is_root_request = rel.is_empty();

        if is_root_request {
            serve_directory_fallback(&mut stream, serve_dir);
            continue;
        }

        let file_path = serve_dir.join(rel);
        let normalized = normalize_path(&file_path);

        if !normalized.starts_with(serve_dir) {
            let body = "403 Forbidden";
            let resp = format!(
                "HTTP/1.1 403 Forbidden\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            continue;
        }

        if normalized.is_file() {
            match std::fs::read(&normalized) {
                Ok(content) => {
                    let mime = guess_mime(&normalized);
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\n\
                         Content-Type: {}\r\n\
                         Content-Length: {}\r\n\
                         Cache-Control: no-store, no-cache, must-revalidate, max-age=0\r\n\
                         Pragma: no-cache\r\n\
                         Connection: close\r\n\r\n",
                        mime,
                        content.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.write_all(&content);
                }
                Err(_) => {
                    let body = "500 Internal Server Error";
                    let resp = format!(
                        "HTTP/1.1 500 Internal Server Error\r\n\
                         Content-Length: {}\r\n\
                         Connection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes());
                }
            }
        } else {
            let body = "404 Not Found";
            let resp = format!(
                "HTTP/1.1 404 Not Found\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    }
}

fn serve_directory_fallback(stream: &mut std::net::TcpStream, serve_dir: &Path) {
    use std::io::Write;

    let mut html_with_mtime: Vec<(String, std::time::SystemTime)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(serve_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "html" || ext == "htm" {
                        if let (Some(name), Ok(meta)) = (
                            path.file_name().and_then(|n| n.to_str()),
                            path.metadata(),
                        ) {
                            if let Ok(mtime) = meta.modified() {
                                html_with_mtime.push((name.to_string(), mtime));
                            }
                        }
                    }
                }
            }
        }
    }

    if !html_with_mtime.is_empty() {
        html_with_mtime.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let newest = &html_with_mtime[0].0;
        let redirect_url = format!("/{}", newest);
        let resp = format!(
            "HTTP/1.1 302 Found\r\n\
             Location: {}\r\n\
             Content-Length: 0\r\n\
             Connection: close\r\n\r\n",
            redirect_url
        );
        let _ = stream.write_all(resp.as_bytes());
    } else {
        let mut all_files: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(serve_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    if let Some(name) = entry.file_name().to_str() {
                        if !name.starts_with('.') {
                            all_files.push(name.to_string());
                        }
                    }
                }
            }
        }
        all_files.sort();

        let body = generate_listing_html("Files", &all_files);
        let resp = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\n\
             Cache-Control: no-store\r\n\
             Connection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
    }
}

fn generate_listing_html(title: &str, files: &[String]) -> String {
    let items: Vec<String> = files
        .iter()
        .map(|f| format!("<li><a href=\"/{}\">{}</a></li>", f, f))
        .collect();
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\">\
         <title>SkillLite Preview - {}</title>\
         <style>body{{font-family:system-ui,-apple-system,sans-serif;max-width:600px;margin:40px auto;padding:0 20px}}\
         a{{color:#2563eb;text-decoration:none;font-size:18px}}a:hover{{text-decoration:underline}}\
         li{{margin:8px 0}}h1{{color:#1e293b}}</style></head>\
         <body><h1>{}</h1><ul>{}</ul></body></html>",
        title, title, items.join("")
    )
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().and_then(|c| hex_val(c));
            let lo = chars.next().and_then(|c| hex_val(c));
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push((h << 4 | l) as char);
            } else {
                result.push('%');
            }
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn guess_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("txt") | Some("md") => "text/plain; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("xml") => "application/xml; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}
