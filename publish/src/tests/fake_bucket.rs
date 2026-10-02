//! An S3-like bucket on 127.0.0.1 for the publisher's tests: GET answers a stored object
//! or 404, PUT stores the body; every request is logged in arrival order.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// One request the bucket received; header names lower-cased.
#[derive(Clone, Debug)]
pub(super) struct Logged {
    pub(super) method: String,
    pub(super) key: String,
    pub(super) headers: BTreeMap<String, String>,
}

#[derive(Default)]
struct State {
    objects: BTreeMap<String, Vec<u8>>,
    log: Vec<Logged>,
    broken: bool,
}

pub(super) struct FakeBucket {
    /// `http://127.0.0.1:<port>/bucket/`, path-style like an S3 endpoint.
    pub(super) url: String,
    state: Arc<Mutex<State>>,
}

impl FakeBucket {
    pub(super) fn start() -> FakeBucket {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/bucket/", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let shared = Arc::clone(&state);
        thread::spawn(move || {
            for stream in listener.incoming() {
                serve(stream.unwrap(), &shared);
            }
        });
        FakeBucket { url, state }
    }

    /// Answers every request with HTTP 500.
    pub(super) fn broken() -> FakeBucket {
        let bucket = Self::start();
        bucket.state.lock().unwrap().broken = true;
        bucket
    }

    pub(super) fn with(self, key: &str, bytes: &[u8]) -> FakeBucket {
        let mut state = self.state.lock().unwrap();
        state.objects.insert(key.to_string(), bytes.to_vec());
        drop(state);
        self
    }

    pub(super) fn object(&self, key: &str) -> Option<Vec<u8>> {
        self.state.lock().unwrap().objects.get(key).cloned()
    }

    pub(super) fn log(&self) -> Vec<Logged> {
        self.state.lock().unwrap().log.clone()
    }

    /// The log as `METHOD key` lines.
    pub(super) fn calls(&self) -> Vec<String> {
        let log = self.log();
        log.iter()
            .map(|l| format!("{} {}", l.method, l.key))
            .collect()
    }
}

fn serve(mut stream: TcpStream, state: &Mutex<State>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let mut start = line.split_whitespace();
    let method = start.next().unwrap().to_string();
    let path = start.next().unwrap().to_string();
    let mut headers = BTreeMap::new();
    loop {
        line.clear();
        reader.read_line(&mut line).unwrap();
        let Some((name, value)) = line.trim_end().split_once(':') else {
            break;
        };
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }
    let length = headers
        .get("content-length")
        .map_or(0, |v| v.parse().unwrap());
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    let key = path.strip_prefix("/bucket/").unwrap_or(&path).to_string();
    let mut state = state.lock().unwrap();
    state.log.push(Logged {
        method: method.clone(),
        key: key.clone(),
        headers,
    });
    let (status, answer) = if state.broken {
        ("500 Internal Server Error", Vec::new())
    } else if method == "PUT" {
        state.objects.insert(key, body);
        ("200 OK", Vec::new())
    } else {
        match state.objects.get(&key) {
            Some(bytes) => ("200 OK", bytes.clone()),
            None => (
                "404 Not Found",
                b"<Error><Code>NoSuchKey</Code></Error>".to_vec(),
            ),
        }
    };
    drop(state);
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        answer.len()
    );
    stream
        .write_all(&[head.as_bytes(), &answer].concat())
        .unwrap();
}
