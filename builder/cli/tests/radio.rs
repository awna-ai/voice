use std::{fs, path::PathBuf, process::Command};

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
