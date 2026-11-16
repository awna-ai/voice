use super::*;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
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
struct FakeRadio {
    _scratch: Scratch,
    socket: PathBuf,
    seen: Arc<Mutex<Vec<Value>>>,
    thread: Option<JoinHandle<()>>,
}

impl FakeRadio {
    fn answering(answers: &[Value]) -> Self {
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

    fn radio(&self) -> Radio {
        Radio::at(self.socket.clone())
    }

    /// Every request line, once the fake has answered them all.
    fn requests(mut self) -> Vec<Value> {
        self.thread.take().unwrap().join().unwrap();
        self.seen.lock().unwrap().clone()
    }
}

fn link_hello() -> Value {
    json!({"ok": true, "radio": "0.2.0", "protocol": {"major": 1, "minor": 7}, "majors": [1],
        "ops": radio_client::OPS})
}

fn one(answer: Value) -> FakeRadio {
    FakeRadio::answering(&[answer])
}

fn message(error: Error) -> String {
    match error {
        Error::Invalid(message) => message,
        other => panic!("expected a sentence, got {other}"),
    }
}

#[test]
fn hello_names_the_version_and_protocol() {
    let fake = one(link_hello());
    let hello = fake.radio().hello().unwrap();
    assert_eq!(hello.radio, "0.2.0");
    assert_eq!((hello.major, hello.minor), (1, 7));
    assert_eq!(fake.requests(), vec![json!({"op": "hello", "v": 1})]);
}

#[test]
fn hello_refuses_a_radio_that_cannot_link() {
    let mut old = link_hello();
    old["protocol"]["minor"] = 6.into();
    let mut other_major = link_hello();
    other_major["protocol"]["major"] = 2.into();
    let mut missing_op = link_hello();
    missing_op["ops"] = json!(radio_client::OPS[..radio_client::OPS.len() - 1]);
    for answer in [old, other_major, missing_op] {
        let fake = one(answer);
        assert_eq!(
            message(fake.radio().hello().unwrap_err()),
            text::TOO_OLD,
            "{:?}",
            fake.requests()
        );
    }
}

#[test]
fn each_link_op_sends_only_its_own_fields() {
    let fake = FakeRadio::answering(&[
        json!({"ok": true}),
        json!({"ok": true, "fleet": {"id": "f1", "name": "moss", "version": 1, "founder": "k1"},
            "recovery_file": "/tmp/recovery"}),
        json!({"ok": true, "code": "rj1.secret", "code_id": "c1", "expires_at": 1800.0}),
        json!({"ok": true, "fleet": {"id": "f1", "name": "moss", "version": 2, "founder": "k1"}}),
        json!({"ok": true, "port": 7341, "addresses": ["10.0.0.2"]}),
        json!({"ok": true, "port": 7341, "addresses": ["10.0.0.2"]}),
        json!({"ok": true, "removed": true, "port": 7341, "addresses": []}),
        json!({"ok": true, "computers": [{"id": "k2", "name": "pebble", "state": "accepted",
            "trusted": false}]}),
        json!({"ok": true, "size": 12}),
        json!({"ok": true}),
        json!({"ok": true, "deliveries": [{"id": 3, "computer": "pebble", "key": "k2",
            "name": "h.jsonl", "size": 12, "at": 5.0}]}),
    ]);
    let radio = fake.radio();
    radio
        .register(vec![Member {
            kind: "assistant".into(),
            name: "quill".into(),
        }])
        .unwrap();
    let (fleet, recovery) = radio.fleet_init("moss").unwrap();
    assert_eq!(
        (fleet.name.as_str(), recovery.as_str()),
        ("moss", "/tmp/recovery")
    );
    let code = radio.code_create(Some("pebble"), Some(600)).unwrap();
    assert_eq!((code.code(), code.id.as_str()), ("rj1.secret", "c1"));
    assert_eq!(radio.join("rj1.secret").unwrap().version, 2);
    let added = radio.link_address_add("10.0.0.2").unwrap();
    assert_eq!(
        (added.port, added.addresses.clone()),
        (7341, vec!["10.0.0.2".to_owned()])
    );
    assert_eq!(radio.link_address_list().unwrap(), added);
    assert!(
        radio
            .link_address_remove("10.0.0.2")
            .unwrap()
            .addresses
            .is_empty()
    );
    assert_eq!(
        radio.computer_list().unwrap()[0].name.as_deref(),
        Some("pebble")
    );
    assert_eq!(
        radio
            .send("pebble", "h.jsonl", Path::new("/tmp/h.jsonl"))
            .unwrap(),
        12
    );
    radio
        .delivery_folder_set(Path::new("/tmp/deliveries"))
        .unwrap();
    assert_eq!(radio.deliveries(Some(2), Some(10)).unwrap()[0].id, 3);
    assert_eq!(
        fake.requests(),
        vec![
            json!({"op": "register", "app": "voice", "app_version": env!("CARGO_PKG_VERSION"),
                "members": [{"kind": "assistant", "name": "quill"}], "v": 1}),
            json!({"op": "fleet_init", "name": "moss", "v": 1}),
            json!({"op": "code_create", "name": "pebble", "ttl": 600, "v": 1}),
            json!({"op": "join", "code": "rj1.secret", "v": 1}),
            json!({"op": "link_address_add", "address": "10.0.0.2", "v": 1}),
            json!({"op": "link_address_list", "v": 1}),
            json!({"op": "link_address_remove", "address": "10.0.0.2", "v": 1}),
            json!({"op": "computer_list", "v": 1}),
            json!({"op": "send", "app": "voice", "computer": "pebble", "name": "h.jsonl",
                "file": "/tmp/h.jsonl", "v": 1}),
            json!({"op": "delivery_folder_set", "app": "voice", "folder": "/tmp/deliveries",
                "v": 1}),
            json!({"op": "deliveries", "app": "voice", "after": 2, "limit": 10, "v": 1}),
        ]
    );
}

#[test]
fn optional_fields_are_left_out() {
    let fake = FakeRadio::answering(&[
        json!({"ok": true, "code": "rj1.secret", "code_id": "c1", "expires_at": 1800.0}),
        json!({"ok": true, "deliveries": []}),
    ]);
    let radio = fake.radio();
    radio.code_create(None, None).unwrap();
    assert!(radio.deliveries(None, None).unwrap().is_empty());
    assert_eq!(
        fake.requests(),
        vec![
            json!({"op": "code_create", "v": 1}),
            json!({"op": "deliveries", "app": "voice", "v": 1}),
        ]
    );
}

#[test]
fn a_refusal_shows_radios_phrase() {
    let fake = one(json!({"ok": false, "error": "the folder already exists; name a new one"}));
    assert_eq!(
        message(
            fake.radio()
                .delivery_folder_set(Path::new("/tmp/d"))
                .unwrap_err()
        ),
        "Radio said: the folder already exists; name a new one."
    );
}

#[test]
fn an_unsafe_phrase_is_replaced() {
    for phrase in [
        json!("two\nlines"),
        json!("x".repeat(201)),
        json!(""),
        json!("bad\u{202E}gnin"),
        json!("line\u{2028}break"),
        json!("caf\u{e9}"),
        Value::Null,
    ] {
        let fake = one(json!({"ok": false, "error": phrase}));
        assert_eq!(
            message(fake.radio().computer_list().unwrap_err()),
            text::REFUSED
        );
    }
}

#[test]
fn an_error_that_is_not_text_is_no_answer() {
    let fake = one(json!({"ok": false, "error": 7}));
    assert_eq!(
        message(fake.radio().computer_list().unwrap_err()),
        text::NO_ANSWER
    );
}

#[test]
fn a_missing_field_is_no_answer() {
    let fake = FakeRadio::answering(&[
        json!({"ok": true}),
        json!({"ok": true, "code_id": "c1", "expires_at": 1.0}),
        json!({"ok": true, "port": 7341}),
        json!({"ok": true}),
    ]);
    let radio = fake.radio();
    assert_eq!(
        message(radio.send("p", "h", Path::new("/tmp/h")).unwrap_err()),
        text::NO_ANSWER
    );
    assert_eq!(
        message(radio.code_create(None, None).unwrap_err()),
        text::NO_ANSWER
    );
    assert_eq!(
        message(radio.link_address_list().unwrap_err()),
        text::NO_ANSWER
    );
    assert_eq!(message(radio.join("rj1.x").unwrap_err()), text::NO_ANSWER);
}

#[test]
fn no_radio_is_not_running() {
    let scratch = Scratch::new();
    let radio = Radio::at(scratch.0.join("none.sock"));
    assert_eq!(message(radio.hello().unwrap_err()), text::NOT_RUNNING);
}

#[test]
fn a_join_code_never_shows_in_debug() {
    let fake = one(json!({"ok": true, "code": "rj1.secret", "code_id": "c1", "expires_at": 1.0}));
    let code = fake.radio().code_create(None, None).unwrap();
    let shown = format!("{code:?}");
    assert!(!shown.contains("secret"), "{shown}");
    assert!(shown.contains("c1"), "{shown}");
}

#[test]
fn for_home_uses_radios_own_socket() {
    assert_eq!(
        Radio::for_home(Path::new("/h")).socket,
        PathBuf::from("/h/.local/state/radio/radio.sock")
    );
}

fn fake_copy(scratch: &Scratch, exit: &str) -> (PathBuf, PathBuf) {
    let radio = scratch.0.join("radio");
    let args = scratch.0.join("args");
    fs::write(
        &radio,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\n{exit}\n",
            args.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&radio, fs::Permissions::from_mode(0o755)).unwrap();
    (radio, args)
}

#[test]
fn install_says_each_exit_code_plainly() {
    for (exit, expected) in [
        ("exit 0", Ok(text::INSTALLED)),
        ("exit 4", Ok(text::INSTALLED)),
        ("exit 1", Err(text::INSTALL_FAILED)),
        ("exit 2", Err(text::INSTALL_REFUSED)),
        ("exit 3", Err(text::NO_SCREEN)),
        ("exit 9", Err(text::INSTALL_STOPPED)),
        ("kill -9 $$", Err(text::INSTALL_STOPPED)),
    ] {
        let scratch = Scratch::new();
        let (radio, args) = fake_copy(&scratch, exit);
        let handler = scratch.0.join("voice");
        let got = install_within(&radio, &handler, Duration::from_secs(60)).map_err(message);
        assert_eq!(got, expected.map_err(str::to_owned), "{exit}");
        assert_eq!(
            fs::read_to_string(args).unwrap(),
            format!(
                "install\n--app\nvoice\n--app-version\n{}\n--handler\n{}\n",
                env!("CARGO_PKG_VERSION"),
                handler.display()
            )
        );
    }
}

#[test]
fn install_refuses_a_missing_linked_or_odd_copy_before_running() {
    let scratch = Scratch::new();
    let (real, args) = fake_copy(&scratch, "exit 0");
    let link = scratch.0.join("linked");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let folder = scratch.0.join("folder");
    fs::create_dir(&folder).unwrap();
    for copy in [scratch.0.join("missing"), link, folder] {
        assert_eq!(
            message(
                install_within(&copy, Path::new("/tmp/voice"), Duration::from_secs(60))
                    .unwrap_err()
            ),
            text::NO_COPY
        );
    }
    assert!(!args.exists());
}

#[test]
fn install_gives_up_after_its_deadline() {
    let scratch = Scratch::new();
    // The fake starts a child of its own and waits on it: once Voice gives up, both must be gone.
    let pids = scratch.0.join("pids");
    let (radio, _) = fake_copy(
        &scratch,
        &format!("sleep 30 & echo \"$$ $!\" > '{}'; wait", pids.display()),
    );
    let started = std::time::Instant::now();
    let got = install_within(&radio, Path::new("/tmp/voice"), Duration::from_millis(300));
    assert_eq!(message(got.unwrap_err()), text::INSTALL_TOO_LONG);
    for pid in fs::read_to_string(pids).unwrap().split_whitespace() {
        let pid: i32 = pid.parse().unwrap();
        // SAFETY: kill with signal 0 only checks whether the process exists.
        assert_ne!(unsafe { libc::kill(pid, 0) }, 0, "{pid} is still running");
    }
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn status_with_radio_names_its_version_and_computers() {
    let fake = FakeRadio::answering(&[
        link_hello(),
        json!({"ok": true, "computers": [
            {"id": "k2", "name": "pebble", "state": "accepted", "trusted": false},
            {"id": "k3", "state": "pending", "trusted": false}]}),
    ]);
    let (lines, answers) = status_lines(Ok(PathBuf::from("/A/Voice.app/radio")), &fake.radio());
    assert!(answers);
    assert_eq!(
        lines,
        vec![
            "Voice's copy of Radio: /A/Voice.app/radio".to_owned(),
            "Radio is running: version 0.2.0, protocol 1.7.".to_owned(),
            "Linked computers: pebble (accepted), k3 (pending).".to_owned(),
        ]
    );
    fake.requests();
}

#[test]
fn status_without_radio_says_so() {
    let scratch = Scratch::new();
    let radio = Radio::at(scratch.0.join("none.sock"));
    let (lines, answers) = status_lines(Err(Error::Invalid(text::NO_COPY.into())), &radio);
    assert!(!answers);
    assert_eq!(
        lines,
        vec![
            format!("Voice's copy of Radio: {}", text::NO_COPY),
            text::NOT_RUNNING.to_owned(),
        ]
    );
}

#[test]
fn status_never_prints_a_hidden_character() {
    let mut hello = link_hello();
    hello["radio"] = json!("0.2.0\u{1b}[2J");
    let fake = FakeRadio::answering(&[
        hello,
        json!({"ok": true, "computers": [
            {"id": "k\u{1b}2", "state": "accepted\u{7}", "trusted": false},
            {"id": "k3", "name": "peb\u{202E}ble", "state": "pending", "trusted": false}]}),
    ]);
    let (lines, _) = status_lines(Ok(PathBuf::from("/r")), &fake.radio());
    assert_eq!(
        lines[1],
        "Radio is running: version 0.2.0?[2J, protocol 1.7."
    );
    assert_eq!(
        lines[2],
        "Linked computers: k?2 (accepted?), peb?ble (pending)."
    );
    fake.requests();
}

#[test]
fn hello_refuses_a_radio_missing_any_link_op() {
    for op in LINK_OPS {
        let mut answer = link_hello();
        answer["ops"] = json!(
            radio_client::OPS
                .iter()
                .filter(|have| *have != op)
                .collect::<Vec<_>>()
        );
        let fake = one(answer);
        assert_eq!(
            message(fake.radio().hello().unwrap_err()),
            text::TOO_OLD,
            "{op}"
        );
        fake.requests();
    }
}

#[test]
fn another_major_is_named_as_such() {
    let fake = one(json!({"ok": false, "error": "unsupported_version", "supported": [2]}));
    assert_eq!(
        message(fake.radio().hello().unwrap_err()),
        text::OTHER_MAJOR
    );
}

#[test]
fn install_runs_only_an_absolute_copy() {
    let scratch = Scratch::new();
    let (radio, args) = fake_copy(&scratch, "exit 0");
    // A relative path to a real copy: enough `..` to reach `/` from any working folder.
    let relative = PathBuf::from("../".repeat(64)).join(radio.strip_prefix("/").unwrap());
    assert!(relative.is_file());
    let got = install_within(&relative, Path::new("/tmp/voice"), Duration::from_secs(5));
    assert_eq!(message(got.unwrap_err()), text::NO_COPY);
    assert!(!args.exists());
}

#[test]
fn a_path_that_is_not_text_is_refused_before_sending() {
    use std::os::unix::ffi::OsStrExt;
    let scratch = Scratch::new();
    let radio = Radio::at(scratch.0.join("none.sock"));
    let odd = Path::new(std::ffi::OsStr::from_bytes(b"/tmp/\xff"));
    assert_eq!(
        message(radio.send("p", "h", odd).unwrap_err()),
        text::NOT_TEXT
    );
    assert_eq!(
        message(radio.delivery_folder_set(odd).unwrap_err()),
        text::NOT_TEXT
    );
}

#[test]
fn status_with_no_computers_says_none_yet() {
    let fake = FakeRadio::answering(&[link_hello(), json!({"ok": true, "computers": []})]);
    let (lines, _) = status_lines(Ok(PathBuf::from("/r")), &fake.radio());
    assert_eq!(lines[2], text::NO_COMPUTERS);
    fake.requests();
}
