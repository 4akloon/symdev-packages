//! `StaticServer` over real sockets on 127.0.0.1.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use super::StaticServer;

/// A log the server thread writes and the test reads.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<u8>>>);

impl Write for Log {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(data)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A server on a temporary bucket holding `index.toml` and `symdev/0.1.0/a b.tar.gz`, and
/// a secret beside the bucket.
fn start() -> (tempfile::TempDir, u16, Log) {
    let tmp = tempfile::tempdir().unwrap();
    let bucket = tmp.path().join("bucket");
    std::fs::create_dir_all(bucket.join("symdev/0.1.0")).unwrap();
    std::fs::write(bucket.join("index.toml"), "# index\n").unwrap();
    std::fs::write(bucket.join("symdev/0.1.0/a b.tar.gz"), [0x1f, 0x8b, 0, 1]).unwrap();
    std::fs::write(tmp.path().join("secret"), "not served").unwrap();
    let server = StaticServer::bind(&bucket).unwrap();
    let port = server.port();
    let log = Log::default();
    let thread_log = log.clone();
    std::thread::spawn(move || server.serve(thread_log));
    (tmp, port, log)
}

/// The raw response to `request`.
fn ask(port: u16, request: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    response
}

fn get(port: u16, path: &str) -> (String, Vec<u8>) {
    let response = ask(port, &format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n"));
    let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8(response[..split].to_vec()).unwrap();
    (head, response[split + 4..].to_vec())
}

#[test]
fn a_file_is_served_with_its_length() {
    let (_tmp, port, _) = start();
    let (head, body) = get(port, "/index.toml");
    assert!(head.starts_with("HTTP/1.0 200 OK\r\n"), "{head}");
    assert!(head.contains("\r\nContent-Length: 8"), "{head}");
    assert_eq!(body, b"# index\n");
}

#[test]
fn a_query_is_ignored_and_percent_escapes_are_decoded() {
    let (_tmp, port, _) = start();
    let (head, body) = get(port, "/symdev/0.1.0/a%20b.tar.gz?x=1");
    assert!(head.starts_with("HTTP/1.0 200 OK\r\n"), "{head}");
    assert_eq!(body, [0x1f, 0x8b, 0, 1]);
}

#[test]
fn what_the_bucket_does_not_hold_is_not_found() {
    let (_tmp, port, _) = start();
    for path in [
        "/missing.toml",
        "/../secret",
        "/symdev/%2e%2e/../secret",
        "/symdev",
    ] {
        let (head, _) = get(port, path);
        assert!(head.starts_with("HTTP/1.0 404 "), "{path}: {head}");
    }
}

#[test]
fn each_request_is_logged_as_python_http_server_logged_it() {
    let (_tmp, port, log) = start();
    get(port, "/index.toml");
    get(port, "/missing.toml");
    let logged = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
    assert!(
        logged.contains("\"GET /index.toml HTTP/1.1\" 200 8\n"),
        "{logged}"
    );
    assert!(
        logged.contains("\"GET /missing.toml HTTP/1.1\" 404 -\n"),
        "{logged}"
    );
}
