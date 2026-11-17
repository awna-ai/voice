use std::{fs, path::PathBuf, process::Command};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from("/tmp").join(format!("vrc-{}", std::process::id()));
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
