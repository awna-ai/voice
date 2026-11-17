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
