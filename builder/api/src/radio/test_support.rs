use super::Radio;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread::JoinHandle,
};

pub(crate) struct Scratch(pub(crate) PathBuf);

impl Scratch {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        // Short: a Unix socket path is at most 104 bytes on macOS.
        let path = PathBuf::from("/tmp").join(format!(
            "vr-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A stand-in Radio: answers each connection with the next canned line and keeps every request line.
pub(crate) struct FakeRadio {
    _scratch: Scratch,
    socket: PathBuf,
    seen: Arc<Mutex<Vec<Value>>>,
    thread: Option<JoinHandle<()>>,
}

impl FakeRadio {
    pub(crate) fn answering(answers: &[Value]) -> Self {
        let scratch = Scratch::new();
        let socket = scratch.0.join("r.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let answers: Vec<String> = answers.iter().map(Value::to_string).collect();
        let kept = Arc::clone(&seen);
        let thread = std::thread::spawn(move || {
            for answer in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                kept.lock()
                    .unwrap()
                    .push(serde_json::from_str(&line).unwrap());
                stream.write_all(answer.as_bytes()).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });
        Self {
            _scratch: scratch,
            socket,
            seen,
            thread: Some(thread),
        }
    }

    pub(crate) fn radio(&self) -> Radio {
        Radio::at(self.socket.clone())
    }

    /// Every request line, once the fake has answered them all.
    pub(crate) fn requests(mut self) -> Vec<Value> {
        self.thread.take().unwrap().join().unwrap();
        self.seen.lock().unwrap().clone()
    }
}

pub(crate) fn link_hello() -> Value {
    json!({"ok": true, "radio": "0.3.0", "protocol": {"major": 1, "minor": 11}, "majors": [1],
        "ops": radio_client::OPS})
}
