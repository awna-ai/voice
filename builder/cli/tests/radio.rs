use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = PathBuf::from("/tmp").join(format!(
            "vrc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("home")).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn radio_message_is_refused_printing_and_writing_nothing() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    let mut child = Command::new(env!("CARGO_BIN_EXE_voice"))
        .args(["message"])
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", &home)
        .env("RADIO_FROM_KEY", "a".repeat(64))
        .env("RADIO_FROM_NAME", "pebble")
        .env("RADIO_MESSAGE_ID", "0".repeat(32))
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"kind":"seat_turn","room":"r1"}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(fs::read_dir(&home).unwrap().next().is_none());
}

#[test]
fn radio_status_without_radio_says_so_and_exits_one() {
    let scratch = Scratch::new();
    let output = Command::new(env!("CARGO_BIN_EXE_voice"))
        .env("HOME", scratch.0.join("home"))
        .args(["radio", "status"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "Voice's copy of Radio: {}\n{}\n",
            voice_api::radio::text::NO_COPY,
            voice_api::radio::text::NOT_RUNNING
        )
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn radio_install_without_a_copy_says_so_and_exits_one() {
    let scratch = Scratch::new();
    let output = Command::new(env!("CARGO_BIN_EXE_voice"))
        .env("HOME", scratch.0.join("home"))
        .args(["radio", "install"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!("voice: {}\n", voice_api::radio::text::NO_COPY)
    );
}

#[test]
fn radio_install_runs_the_radio_beside_voice_and_says_so() {
    use std::os::unix::fs::PermissionsExt;
    // A development build skips the lasting-place rule, so a copy of `voice` in a scratch folder
    // runs the stand-in `radio` beside it.
    let scratch = Scratch::new();
    let voice = scratch.0.join("voice");
    fs::copy(env!("CARGO_BIN_EXE_voice"), &voice).unwrap();
    let args = scratch.0.join("args");
    let radio = scratch.0.join("radio");
    fs::write(
        &radio,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done >> '{}'\n",
            args.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&radio, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(&voice)
        .env("HOME", scratch.0.join("home"))
        .args(["radio", "install"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{}\n", voice_api::radio::text::INSTALLED)
    );
    assert_eq!(
        fs::read_to_string(args).unwrap(),
        format!(
            "install\n--app\nvoice\n--app-version\n{}\n--handler\n{}\n",
            env!("CARGO_PKG_VERSION"),
            voice.canonicalize().unwrap().display()
        )
    );
}

fn member_computer(home: &std::path::Path) {
    use voice_core::crew::{Decisions, NewMember, new_member};
    new_member(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
        &home.join(".claude/crew/wren"),
        &NewMember {
            decisions: Decisions::Python,
            name: "Wren",
            pronouns: "she/her",
            principal: "Rowan Vale",
            principal_pronouns: "they/them",
            repo_shown: "~/ai",
            date: "2026-11-17",
        },
    )
    .unwrap();
    let state = home.join(".claude/state/wren");
    fs::create_dir_all(&state).unwrap();
    fs::write(
        state.join("save.json"),
        br#"{"version":1,"mode":"github","owner":"example-owner","name":"wren-assistant","branch":"main","role":"member"}"#,
    )
    .unwrap();
}

#[test]
fn radio_join_reads_only_stdin_and_prints_a_sentence_without_the_code() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    member_computer(&home);
    for (code, sentence) in [
        (
            "not-a-private-join-code\n",
            "That isn't a join code. Copy the whole code from the computer that builds your assistant, then try again.",
        ),
        (
            "rj1.private-join-fixture\n",
            voice_api::radio::text::NO_COPY,
        ),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_voice"))
            .env_clear()
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .args(["radio", "join", "wren", "--name", "Work laptop"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(code.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!("voice: {sentence}\n")
        );
    }
}

#[test]
fn radio_join_has_no_code_argument() {
    let output = Command::new(env!("CARGO_BIN_EXE_voice"))
        .args(["radio", "join", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("--name"));
    assert!(!help.contains("--code"));
    assert!(!help.contains("<CODE>"));
    let output = Command::new(env!("CARGO_BIN_EXE_voice"))
        .args(["radio", "join", "wren", "--name", "Work laptop", "--code"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unexpected argument '--code'")
    );
}

#[test]
fn radio_address_refuses_a_port_with_its_sentence_and_exits_one() {
    let scratch = Scratch::new();
    let output = Command::new(env!("CARGO_BIN_EXE_voice"))
        .env("HOME", scratch.0.join("home"))
        .args(["radio", "address", "10.0.0.5:7341"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "voice: That isn't a VPN address. Type the numbers only, like 10.0.0.5, with no port.\n"
    );
}

#[test]
fn radio_join_restores_terminal_echo_when_interrupted() {
    use std::{
        io::{BufRead, BufReader},
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
        time::{Duration, Instant},
    };
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    member_computer(&home);
    let mut master = -1;
    let mut slave = -1;
    // SAFETY: both descriptors point to live storage; optional settings and name are omitted.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0,
        "{}",
        std::io::Error::last_os_error()
    );
    // SAFETY: openpty succeeded and returned two fresh, owned descriptors.
    let (_master, slave) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    let echo = || {
        let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
        // SAFETY: slave stays open and settings points to writable storage.
        assert_eq!(
            unsafe { libc::tcgetattr(slave.as_raw_fd(), settings.as_mut_ptr()) },
            0
        );
        // SAFETY: tcgetattr initialized settings on success.
        unsafe { settings.assume_init().c_lflag & libc::ECHO != 0 }
    };
    assert!(echo(), "the scratch terminal begins with echo enabled");
    let mut child = Command::new(env!("CARGO_BIN_EXE_voice"))
        .env_clear()
        .env("HOME", &home)
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .args(["radio", "join", "wren", "--name", "Work laptop"])
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while echo() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let hidden = !echo();
    // SAFETY: child.id() identifies the child process this test just started.
    let interrupted = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        hidden,
        "join turns terminal echo off while waiting for the code"
    );
    assert_eq!(interrupted, 0);
    assert!(status.is_some(), "join exits after Ctrl-C");
    let mut prompt = String::new();
    BufReader::new(child.stderr.take().unwrap())
        .read_line(&mut prompt)
        .unwrap();
    assert_eq!(prompt, "Paste the join code, then press Return:\n");
    assert!(
        echo(),
        "Ctrl-C must restore the terminal's original echo setting"
    );
}

fn history_command(home: &std::path::Path, command: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_voice"))
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("TZ", "UTC")
        .args(["radio", command, "wren"])
        .output()
        .unwrap()
}

#[test]
fn radio_send_and_take_refuse_the_wrong_role_without_radio() {
    for (command, member, sentence) in [
        (
            "send",
            false,
            "This computer builds Wren. Your other computers send their history here.",
        ),
        (
            "take",
            true,
            "This computer sends its history to the computer that builds Wren; take deliveries there.",
        ),
    ] {
        let scratch = Scratch::new();
        let home = scratch.0.join("home");
        member_computer(&home);
        if !member {
            fs::remove_file(home.join(".claude/state/wren/save.json")).unwrap();
        }
        let output = history_command(&home, command);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!("voice: {sentence}\n")
        );
    }
}

#[test]
fn radio_history_reports_saved_status_without_radio() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    member_computer(&home);
    let state = home.join(".claude/state/wren");
    fs::write(
        state.join("pending-send.jsonl"),
        "private fixture contents must never be printed",
    )
    .unwrap();
    fs::write(
        state.join("last-sent.json"),
        br#"{"at":1800,"to":"Studio"}"#,
    )
    .unwrap();
    fs::write(state.join("deliveries-taken.json"), br#"{"after":2,"computers":{"abcdef0123456789abcdef0123456789":{"name":"Work laptop","at":1800,"records":3}}}"#).unwrap();
    let output = history_command(&home, "history");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty());
    let lines = String::from_utf8(output.stdout).unwrap();
    assert!(lines.contains("Studio"), "{lines}");
    assert!(lines.contains("Work laptop: 3 records, last "), "{lines}");
    assert!(
        lines.to_lowercase().contains("pending")
            || lines.contains("waiting")
            || lines.contains("owed"),
        "{lines}"
    );
    assert!(!lines.contains("private fixture contents"));
    assert!(!lines.contains("abcdef0123456789"));
    assert!(!lines.contains("pending-send.jsonl"));
}

fn with_history_radio(
    home: &std::path::Path,
    command: &str,
    arrivals: serde_json::Value,
) -> std::process::Output {
    use serde_json::json;
    use std::{
        io::{BufRead, BufReader},
        os::unix::net::UnixListener,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };
    let folder = home.join(".local/state/radio");
    fs::create_dir_all(&folder).unwrap();
    let listener = UnixListener::bind(folder.join("radio.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stopped = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&stopped);
    let thread = std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(10);
        while !done.load(Ordering::Relaxed) && Instant::now() < until {
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(error) => panic!("{error}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            let request: serde_json::Value = serde_json::from_str(&line).unwrap();
            let answer = match request["op"].as_str().unwrap() {
                "hello" => {
                    json!({"ok":true,"radio":"0.3.0","protocol":{"major":1,"minor":11},"majors":[1],"ops":["hello","register","fleet_init","code_create","join","resign","link_address_add","link_address_list","link_address_remove","computer_list","send","delivery_folder_set","deliveries"]})
                }
                "link_address_list" => json!({"ok":true,"port":7341,"addresses":["192.0.2.1"]}),
                "register" => json!({"ok":true}),
                "computer_list" => {
                    json!({"ok":true,"computers":[{"id":"founder","name":"Studio","state":"accepted","trusted":true}]})
                }
                "send" => json!({"ok":true,"size":7}),
                "deliveries" => {
                    if request["after"].as_u64().unwrap_or(0) == 0 {
                        json!({"ok":true,"deliveries":arrivals})
                    } else {
                        json!({"ok":true,"deliveries":[]})
                    }
                }
                op => panic!("unexpected operation {op}"),
            };
            writeln!(stream, "{answer}").unwrap();
        }
    });
    let output = history_command(home, command);
    stopped.store(true, Ordering::Relaxed);
    thread.join().unwrap();
    output
}

#[test]
fn radio_send_prints_only_its_destination() {
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    member_computer(&home);
    fs::write(
        home.join(".claude/state/wren/decisions-claude-code.jsonl"),
        "private invented history",
    )
    .unwrap();
    let output = with_history_radio(&home, "send", serde_json::json!([]));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Sent this computer's history to Studio.\n"
    );
}

#[test]
fn radio_take_prints_totals_and_computers_without_history_or_keys() {
    use serde_json::json;
    let scratch = Scratch::new();
    let home = scratch.0.join("home");
    member_computer(&home);
    let state = home.join(".claude/state/wren");
    fs::remove_file(state.join("save.json")).unwrap();
    let folder = state.join("deliveries");
    fs::create_dir(&folder).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).unwrap();
    let record = json!({"id":"invented-id","kind":"message","timestamp":"2026-11-17T12:00:00Z","text":"Keep the first change small."});
    fs::write(folder.join("history-1-1.jsonl"), format!("{record}\n")).unwrap();
    fs::write(folder.join("other.txt"), "invented unrelated file").unwrap();
    let key = "ab".repeat(32);
    let output = with_history_radio(
        &home,
        "take",
        json!([
            {"id":1,"computer":"Work laptop","key":key,"name":"history-1-1.jsonl","size":100,"at":1800.0},
            {"id":2,"computer":"Work laptop","key":key,"name":"other.txt","size":10,"at":1800.0}
        ]),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty());
    let lines = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = lines.lines().collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0], "Took 1 deliveries.");
    assert!(
        lines[1].starts_with("Work laptop: 1 records, last "),
        "{lines:?}"
    );
    let timestamp = lines[1]
        .strip_prefix("Work laptop: 1 records, last ")
        .unwrap();
    let time_parts: Vec<_> = timestamp.split(':').collect();
    let is_time = time_parts.len() == 2
        && time_parts.iter().all(|part| part.len() == 2)
        && time_parts[0].parse::<u8>().is_ok_and(|hour| hour < 24)
        && time_parts[1].parse::<u8>().is_ok_and(|minute| minute < 60);
    let date_parts: Vec<_> = timestamp.split('-').collect();
    let is_date = date_parts.len() == 3
        && date_parts[0].len() == 4
        && date_parts[1..].iter().all(|part| part.len() == 2)
        && date_parts[0].parse::<u16>().is_ok()
        && date_parts[1]
            .parse::<u8>()
            .is_ok_and(|month| (1..=12).contains(&month))
        && date_parts[2]
            .parse::<u8>()
            .is_ok_and(|day| (1..=31).contains(&day));
    assert!(is_time || is_date, "invalid local timestamp: {timestamp}");
    assert_eq!(lines[2], "Left out 1 files that weren't Voice's history.");
}
