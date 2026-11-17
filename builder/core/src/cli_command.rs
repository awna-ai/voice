use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

pub const CLI_FOLDER: &str = "Voice can't be used from a folder whose name has \" $ ` \\ ! { or } in it. Move Voice to the Applications folder in your home folder, then try again.";

pub const CLI_PLACE: &str = "Voice needs to be installed before it can make an assistant. Install it with install.sh, as \"How to install.txt\" shows (both are in the Voice folder you were sent), then open it from the Applications folder in your home folder.";

/// The `voice` command that ships with this program: this program itself when it is
/// that command, else the `voice` beside it (the app's `Contents/MacOS`).
pub fn shipped_cli() -> Result<PathBuf> {
    let executable = std::env::current_exe().map_err(|source| Error::Io {
        path: PathBuf::from("voice"),
        source,
    })?;
    let path = executable.canonicalize().map_err(|source| Error::Io {
        path: executable,
        source,
    })?;
    if path.file_name() == Some(std::ffi::OsStr::new("voice")) {
        Ok(path)
    } else {
        Ok(path.parent().expect("executable has parent").join("voice"))
    }
}

/// `path` as one shell word for a SKILL.md: `"$HOME/<rest>"` when under `home`, else `"<path>"`.
pub fn cli_word(path: &Path, home: &Path) -> Result<String> {
    let home = home.canonicalize().map_err(|source| Error::Io {
        path: home.to_path_buf(),
        source,
    })?;
    let text = path
        .to_str()
        .ok_or_else(|| Error::Invalid(CLI_FOLDER.into()))?;
    if text.chars().any(|c| {
        c.is_control()
            || matches!(
                c,
                '"' | '$' | '`' | '\\' | '!' | '{' | '}' | '\u{2028}' | '\u{2029}'
            )
    }) {
        return Err(Error::Invalid(CLI_FOLDER.into()));
    }
    match path.strip_prefix(home) {
        Ok(rest) => Ok(format!(
            "\"$HOME/{}\"",
            rest.to_str().expect("UTF-8 path checked")
        )),
        Err(_) => Ok(format!("\"{text}\"")),
    }
}

/// `shipped_cli()` as one shell word for a SKILL.md (see `cli_word`). A release build refuses
/// with `CLI_PLACE` unless the command is in a place that lasts (see `lasting`); a development
/// build runs from its own build folder, wherever that is; in a release build the word uses the
/// canonical path the rule checked.
pub fn shipped_cli_word(home: &Path) -> Result<String> {
    word_from(&shipped_cli()?, home, !cfg!(debug_assertions))
}

/// `shipped_cli()`, refused with `CLI_PLACE` in a release build unless it is in a place that
/// lasts (see `lasting`): for a path another program keeps, such as the handler Radio records;
/// in a release build this is the canonical path the rule checked.
pub fn shipped_lasting_cli(home: &Path) -> Result<PathBuf> {
    let path = shipped_cli()?;
    if !cfg!(debug_assertions) {
        return lasting_at_home(&path, home);
    }
    Ok(path)
}

fn word_from(path: &Path, home: &Path, lasting_only: bool) -> Result<String> {
    if lasting_only {
        cli_word(&lasting_at_home(path, home)?, home)
    } else {
        cli_word(path, home)
    }
}

fn lasting_at_home(path: &Path, home: &Path) -> Result<PathBuf> {
    lasting(
        path,
        &[&home.join("Applications"), Path::new("/Applications")],
    )
}

fn lasting(path: &Path, roots: &[&Path]) -> Result<PathBuf> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.file_type().is_file() {
        return Err(Error::Invalid(CLI_PLACE.into()));
    }
    let canonical = path.canonicalize().map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if on_a_passing_disk(&canonical) {
        return Err(Error::Invalid(CLI_PLACE.into()));
    }
    if !roots
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| canonical.starts_with(root))
    {
        return Err(Error::Invalid(CLI_PLACE.into()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid takes no arguments and cannot fail.
        let euid = unsafe { libc::geteuid() };
        for (index, folder) in canonical.ancestors().enumerate() {
            let metadata = std::fs::symlink_metadata(folder).map_err(|source| Error::Io {
                path: folder.to_path_buf(),
                source,
            })?;
            let safe = place_safe(
                index == 0,
                metadata.uid(),
                metadata.gid(),
                metadata.mode(),
                euid,
            );
            if !safe {
                return Err(Error::Invalid(CLI_PLACE.into()));
            }
        }
    }
    Ok(canonical)
}

/// The command itself: owned by the person or root, and writable by no one else (Radio's handler rule).
#[cfg(unix)]
fn command_safe(owner: u32, mode: u32, euid: u32) -> bool {
    (owner == euid || owner == 0) && mode & 0o022 == 0
}

/// A folder above the command: owned by the person or root, writable by its group only when the group is
/// wheel (0) or admin (80), as Radio's handler check allows, and never by others.
#[cfg(unix)]
fn folder_safe(owner: u32, group: u32, mode: u32, euid: u32) -> bool {
    (owner == euid || owner == 0)
        && (mode & 0o020 == 0 || matches!(group, 0 | 80))
        && mode & 0o002 == 0
}

/// One path on the way up from the command: the command itself gets the command's rule, every folder above it
/// the folder rule.
#[cfg(unix)]
fn place_safe(is_command: bool, owner: u32, group: u32, mode: u32, euid: u32) -> bool {
    if is_command {
        command_safe(owner, mode, euid)
    } else {
        folder_safe(owner, group, mode, euid)
    }
}

fn on_a_passing_disk(canonical: &Path) -> bool {
    canonical.starts_with("/Volumes")
        || canonical
            .components()
            .any(|c| c.as_os_str() == "AppTranslocation")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn the_command_gets_the_command_rule_and_folders_the_folder_rule() {
        let euid = 501;
        assert!(
            !place_safe(true, 501, 80, 0o775, euid),
            "command (501, 80, 0o775)"
        );
        assert!(
            place_safe(false, 501, 80, 0o775, euid),
            "folder (501, 80, 0o775)"
        );
        assert!(
            place_safe(true, 501, 20, 0o755, euid),
            "command (501, 20, 0o755)"
        );
        assert!(
            !place_safe(false, 501, 20, 0o775, euid),
            "folder (501, 20, 0o775)"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_command_is_writable_by_no_one_else() {
        let euid = 501;
        assert!(command_safe(501, 0o755, euid), "(501, 0o755)");
        assert!(command_safe(0, 0o755, euid), "(0, 0o755)");
        assert!(!command_safe(502, 0o755, euid), "(502, 0o755)");
        assert!(!command_safe(501, 0o775, euid), "(501, 0o775)");
        assert!(!command_safe(501, 0o757, euid), "(501, 0o757)");
        assert!(command_safe(501, 0o644, euid), "(501, 0o644)");
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_is_group_writable_only_for_wheel_or_admin() {
        let euid = 501;
        assert!(folder_safe(501, 20, 0o755, euid), "(501, 20, 0o755)");
        assert!(!folder_safe(501, 20, 0o775, euid), "(501, 20, 0o775)");
        assert!(folder_safe(501, 80, 0o775, euid), "(501, 80, 0o775)");
        assert!(folder_safe(0, 80, 0o775, euid), "(0, 80, 0o775)");
        assert!(folder_safe(0, 0, 0o775, euid), "(0, 0, 0o775)");
        assert!(!folder_safe(501, 80, 0o777, euid), "(501, 80, 0o777)");
        assert!(!folder_safe(0, 0, 0o1777, euid), "(0, 0, 0o1777)");
        assert!(!folder_safe(502, 20, 0o755, euid), "(502, 20, 0o755)");
        assert!(folder_safe(0, 20, 0o755, euid), "(0, 20, 0o755)");
    }

    // macOS has a per-user temp folder; Linux's /tmp is writable by others and refused.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_command_is_a_plain_file_not_a_link_or_a_folder() {
        let root = Scratch::new();
        let real = command_at(root.path(), "real/voice");
        assert!(lasting(&real, &[root.path()]).is_ok());
        let app = root.path().join("app");
        fs::create_dir(&app).unwrap();
        let linked = app.join("voice");
        std::os::unix::fs::symlink(&real, &linked).unwrap();
        assert!(matches!(
            lasting(&linked, &[root.path()]),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
        let folder = root.path().join("folder/voice");
        fs::create_dir_all(&folder).unwrap();
        assert!(matches!(
            lasting(&folder, &[root.path()]),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_lasting_path_is_the_canonical_one_checked() {
        let root = Scratch::new();
        let command = command_at(root.path(), "Applications/Voice.app/Contents/MacOS/voice");
        let expected = command.canonicalize().unwrap();
        assert_ne!(
            command, expected,
            "the scratch path should go through the /var link"
        );
        assert_eq!(lasting(&command, &[root.path()]).unwrap(), expected);
        assert_eq!(
            word_from(&command, root.path(), true).unwrap(),
            cli_word(&expected, root.path()).unwrap()
        );
    }

    #[test]
    fn a_missing_command_is_an_io_error() {
        let root = Scratch::new();
        assert!(matches!(
            lasting(&root.path().join("absent"), &[root.path()]),
            Err(Error::Io { .. })
        ));
    }

    #[test]
    fn outside_every_root_is_refused() {
        let place = Scratch::new();
        let root = Scratch::new();
        let command = command_at(place.path(), "voice");
        assert!(matches!(
            lasting(&command, &[root.path()]),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
    }

    #[test]
    fn a_translocated_copy_under_a_root_is_refused() {
        let root = Scratch::new();
        let command = command_at(
            root.path(),
            "AppTranslocation/1234/d/Voice.app/Contents/MacOS/voice",
        );
        assert!(matches!(
            lasting(&command, &[root.path()]),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_others_can_write_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let root = Scratch::new();
        let command = command_at(root.path(), "open/voice");
        let open = root.path().join("open");
        fs::set_permissions(&open, fs::Permissions::from_mode(0o757)).unwrap();
        let folder_result = lasting(&command, &[root.path()]);
        fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&command, fs::Permissions::from_mode(0o646)).unwrap();
        let file_result = lasting(&command, &[root.path()]);
        fs::set_permissions(&command, fs::Permissions::from_mode(0o644)).unwrap();
        fs::set_permissions(&command, fs::Permissions::from_mode(0o664)).unwrap();
        let group_file_result = lasting(&command, &[root.path()]);
        fs::set_permissions(&command, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(folder_result, Err(Error::Invalid(message)) if message == CLI_PLACE));
        assert!(matches!(file_result, Err(Error::Invalid(message)) if message == CLI_PLACE));
        assert!(matches!(group_file_result, Err(Error::Invalid(message)) if message == CLI_PLACE));
    }

    #[test]
    fn the_place_rule_runs_only_when_asked() {
        let place = Scratch::new();
        let home = Scratch::new();
        let command = command_at(place.path(), "voice");
        assert_eq!(
            word_from(&command, home.path(), false).unwrap(),
            cli_word(&command, home.path()).unwrap()
        );
        assert!(matches!(
            word_from(&command, home.path(), true),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn a_development_build_skips_the_place_rule() {
        let home = Scratch::new();
        assert_eq!(
            shipped_cli_word(home.path()).ok(),
            Some(cli_word(&shipped_cli().unwrap(), home.path()).unwrap())
        );
    }

    fn command_at(root: &Path, relative: &str) -> PathBuf {
        let command = root.join(relative);
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::write(&command, "command").unwrap();
        command
    }

    // macOS has a per-user temp folder; Linux's /tmp is writable by others and refused.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_root_folder_owned_by_the_person_is_lasting() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let root = Scratch::new();
        let command = command_at(root.path(), "Applications/Voice.app/Contents/MacOS/voice");
        assert!(lasting(&command, &[root.path()]).is_ok());
        assert_eq!(
            word_from(&command, root.path(), true).unwrap(),
            cli_word(&command.canonicalize().unwrap(), root.path()).unwrap()
        );
        let applications = root.path().join("Applications");
        let gid = fs::symlink_metadata(&applications).unwrap().gid();
        assert!(!matches!(gid, 0 | 80));
        // Every member of this folder's group (staff, every account) could swap the command;
        // only a wheel or admin group, as /Applications has, may write a folder above it.
        fs::set_permissions(&applications, fs::Permissions::from_mode(0o775)).unwrap();
        assert!(matches!(
            lasting(&command, &[root.path()]),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
        fs::set_permissions(&applications, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_the_applications_folder_in_home_is_lasting() {
        let home = Scratch::new();
        let command = command_at(home.path(), "Applications/Voice.app/Contents/MacOS/voice");
        assert_eq!(
            word_from(&command, home.path(), true).unwrap(),
            cli_word(&command.canonicalize().unwrap(), home.path()).unwrap()
        );
        let download = command_at(
            home.path(),
            "Downloads/Voice/Voice.app/Contents/MacOS/voice",
        );
        assert!(matches!(
            word_from(&download, home.path(), true),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
        let applications_old = command_at(
            home.path(),
            "ApplicationsOld/Voice.app/Contents/MacOS/voice",
        );
        assert!(matches!(
            word_from(&applications_old, home.path(), true),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
    }

    #[test]
    fn a_passing_disk_is_the_volumes_folder_or_a_translocated_copy() {
        for path in [
            "/Volumes/Voice/Voice.app/Contents/MacOS/voice",
            "/Volumes",
            "/private/var/folders/ab/cd/T/AppTranslocation/1234-ABCD/d/Voice.app/Contents/MacOS/voice",
        ] {
            assert!(on_a_passing_disk(Path::new(path)), "accepted {path}");
        }
        for path in [
            "/VolumesX/voice",
            "/Applications/Voice.app/Contents/MacOS/voice",
            "/Users/robin/AppTranslocationNotes/voice",
        ] {
            assert!(!on_a_passing_disk(Path::new(path)), "refused {path}");
        }
    }

    #[test]
    fn a_development_build_ships_the_cli_from_anywhere() {
        let home = Scratch::new();
        assert_eq!(
            shipped_lasting_cli(home.path()).unwrap(),
            shipped_cli().unwrap()
        );
    }

    #[test]
    fn shipped_cli_is_beside_the_canonical_executable() {
        let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
        let expected = executable.parent().unwrap().join("voice");
        assert_eq!(shipped_cli().unwrap(), expected);
    }
    #[test]
    fn under_home_is_one_word_with_home_expansion_and_spaces() {
        let home = Scratch::new();
        let path = home
            .path()
            .canonicalize()
            .unwrap()
            .join("Applications/Voice.app/Contents/MacOS/voice");
        assert_eq!(
            cli_word(&path, home.path()).unwrap(),
            "\"$HOME/Applications/Voice.app/Contents/MacOS/voice\""
        );
    }
    #[test]
    fn outside_home_is_one_quoted_word() {
        let home = Scratch::new();
        let other = Scratch::new();
        let path = other.path().join("Voice.app/voice");
        assert_eq!(
            cli_word(&path, home.path()).unwrap(),
            format!("\"{}\"", path.display())
        );
    }
    #[test]
    fn refuses_shell_characters_and_controls_in_the_whole_path() {
        let home = Scratch::new();
        for c in [
            '"', '$', '`', '\\', '!', '{', '}', '\0', '\n', '\r', '\t', '\u{7f}', '\u{85}',
            '\u{2028}', '\u{2029}',
        ] {
            for path in [
                home.path().join(format!("bad{c}/voice")),
                PathBuf::from(format!("/bad{c}/voice")),
            ] {
                assert!(
                    matches!(cli_word(&path, home.path()), Err(Error::Invalid(message)) if message == CLI_FOLDER),
                    "accepted {path:?}"
                );
            }
            if !c.is_control() && c != '\\' {
                let unsafe_home = home.path().join(format!("home{c}"));
                std::fs::create_dir(&unsafe_home).unwrap();
                let path = unsafe_home.canonicalize().unwrap().join("voice");
                assert!(
                    matches!(cli_word(&path, &unsafe_home), Err(Error::Invalid(message)) if message == CLI_FOLDER),
                    "accepted unsafe home: {path:?}"
                );
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn refuses_non_utf8_paths() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
        let home = Scratch::new();
        assert!(
            matches!(cli_word(Path::new(OsStr::from_bytes(b"/bad\xff/voice")), home.path()), Err(Error::Invalid(message)) if message == CLI_FOLDER)
        );
    }
    #[test]
    fn missing_home_is_an_io_error() {
        let home = Scratch::new();
        assert!(matches!(
            cli_word(
                Path::new("/Applications/voice"),
                &home.path().join("absent")
            ),
            Err(Error::Io { .. })
        ));
    }
}
