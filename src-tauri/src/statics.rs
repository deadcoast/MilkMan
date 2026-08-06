//! Built-in static file server — the killer path for single-file HTML
//! artifacts dragged straight out of claude.ai. Zero dependencies: a
//! hand-rolled HTTP/1.1 GET server on a loopback port.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct Icebox {
    pub port: u16,
    stop: Arc<AtomicBool>,
}

impl Icebox {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        // poke the listener so accept() unblocks
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn decode_percent(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn handle(mut stream: TcpStream, root: &Path) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/");
    let path = decode_percent(path.split(['?', '#']).next().unwrap_or("/"));

    let rel = path.trim_start_matches('/');
    let mut target: PathBuf = if rel.is_empty() {
        root.join("index.html")
    } else {
        root.join(rel)
    };
    if target.is_dir() {
        target = target.join("index.html");
    }

    // traversal guard: canonical target must stay inside canonical root
    let ok = match (target.canonicalize(), root.canonicalize()) {
        (Ok(t), Ok(r)) => t.starts_with(&r),
        _ => false,
    };

    let (status, ctype, body): (&str, &str, Vec<u8>) = if ok {
        match std::fs::read(&target) {
            Ok(bytes) => ("200 OK", mime(&target), bytes),
            Err(_) => ("404 NOT FOUND", "text/plain", b"SPOILED: not found".to_vec()),
        }
    } else {
        ("404 NOT FOUND", "text/plain", b"SPOILED: not found".to_vec())
    };

    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

/// Serve `root` on a random loopback port until stopped.
pub fn chill(root: PathBuf) -> std::io::Result<Icebox> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if flag.load(Ordering::SeqCst) {
                break;
            }
            if let Ok(stream) = stream {
                let root = root.clone();
                std::thread::spawn(move || handle(stream, &root));
            }
        }
    });
    Ok(Icebox { port, stop })
}
