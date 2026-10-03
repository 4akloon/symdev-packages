//! `StaticServer`: GET of the files under one directory, for the install test's fake
//! bucket (it replaced `python3 -m http.server`).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::tool_error::{Result, ToolError};

/// Serves `root` on 127.0.0.1 at a free port, one connection at a time, HTTP/1.0 with
/// `Connection: close`. Only GET and HEAD; a directory, a missing file or a path with a
/// `..` segment is 404. Each request is logged as http.server logged it:
/// `"GET /index.toml HTTP/1.1" 200 <bytes>`.
pub struct StaticServer {
    root: PathBuf,
    listener: TcpListener,
}

impl StaticServer {
    pub fn bind(root: &Path) -> Result<Self> {
        if !root.is_dir() {
            return Err(ToolError::new(format!(
                "{} is not a directory",
                root.display()
            )));
        }
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|e| ToolError::io("cannot listen on 127.0.0.1", &e))?;
        Ok(Self {
            root: root.to_path_buf(),
            listener,
        })
    }

    pub fn port(&self) -> u16 {
        self.listener.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    /// Serves until the process is killed; a connection that fails is logged and dropped.
    /// A request is logged before its answer is sent, as http.server did, so a client that
    /// has the answer finds the request in the log.
    pub fn serve(self, mut log: impl Write) {
        for stream in self.listener.incoming() {
            let failed = match stream {
                Ok(stream) => self.answer(stream, &mut log).err(),
                Err(e) => Some(e.to_string()),
            };
            if let Some(e) = failed {
                let _ = writeln!(log, "connection failed: {e}");
                let _ = log.flush();
            }
        }
    }

    /// Answers one request, logging it first.
    fn answer(
        &self,
        mut stream: TcpStream,
        log: &mut impl Write,
    ) -> std::result::Result<(), String> {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let head = Self::read_head(&mut stream)?;
        let request = head.lines().next().unwrap_or_default().to_string();
        let mut words = request.split_whitespace();
        let (method, target) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
        let file = Self::relative(target)
            .map(|rel| self.root.join(rel))
            .filter(|p| p.is_file());
        let (status, body) = match (method, file) {
            ("GET" | "HEAD", Some(file)) => match std::fs::read(&file) {
                Ok(body) => ("200 OK", Some(body)),
                Err(_) => ("403 Forbidden", None),
            },
            ("GET" | "HEAD", None) => ("404 Not Found", None),
            _ => ("501 Not Implemented", None),
        };
        let length = body.as_ref().map_or(0, Vec::len);
        let response = format!(
            "HTTP/1.0 {status}\r\nServer: pkgtools-serve\r\nContent-Type: application/octet-stream\r\n\
             Content-Length: {length}\r\nConnection: close\r\n\r\n"
        );
        let mut out = response.into_bytes();
        if method == "GET" {
            out.extend(body.as_deref().unwrap_or_default());
        }
        let size = if body.is_some() {
            length.to_string()
        } else {
            "-".into()
        };
        let code = status.split(' ').next().unwrap_or_default();
        let _ = writeln!(log, "\"{request}\" {code} {size}");
        let _ = log.flush();
        stream.write_all(&out).map_err(|e| e.to_string())
    }

    fn read_head(stream: &mut TcpStream) -> std::result::Result<String, String> {
        let mut head = Vec::new();
        let mut buf = [0u8; 1024];
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 || head.len() > 16 * 1024 {
                break;
            }
            head.extend_from_slice(&buf[..n]);
        }
        Ok(String::from_utf8_lossy(&head).into_owned())
    }

    /// The file a request target names, below the root; `None` for one that leaves it.
    fn relative(target: &str) -> Option<PathBuf> {
        let path = target.split(['?', '#']).next()?.strip_prefix('/')?;
        let decoded = Self::percent_decoded(path)?;
        let mut rel = PathBuf::new();
        for segment in decoded.split('/').filter(|s| !s.is_empty() && *s != ".") {
            if segment == ".." {
                return None;
            }
            rel.push(segment);
        }
        Some(rel)
    }

    fn percent_decoded(text: &str) -> Option<String> {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at] == b'%' {
                let hex = std::str::from_utf8(bytes.get(at + 1..at + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                at += 3;
            } else {
                out.push(bytes[at]);
                at += 1;
            }
        }
        String::from_utf8(out).ok()
    }
}
