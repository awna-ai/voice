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
    assert!(!output.status.success());
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
