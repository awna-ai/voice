//! The person's pasted Console key (VF5), kept in Voice's own Keychain item because the
//! sandbox keeps the Keychain closed to Claude Code.
use std::path::Path;
use voice_core::error::{Error, Result};
#[cfg(not(feature = "test-stubs"))]
use voice_save::keychain::Keychain;
use voice_save::{keychain::TokenStore, secret::Secret};

use crate::text;

pub const SERVICE: &str = "voice.claude-console";

/// Check only the shape of a Console key, without contacting Anthropic.
pub fn valid(key: &str) -> bool {
    key.starts_with("sk-ant-")
        && !key.starts_with("sk-ant-admin")
        && (20..=256).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

pub(crate) fn load(home: &Path) -> Result<Option<Secret>> {
    let key = store(home)
        .load()
        .map_err(|_| invalid(text::KEY_UNREADABLE))?;
    if key.as_ref().is_some_and(|key| !valid(key.expose())) {
        return Err(invalid(text::KEY_UNREADABLE));
    }
    Ok(key)
}

pub(crate) fn save(home: &Path, key: &Secret) -> Result<()> {
    store(home)
        .save(key)
        .map_err(|_| invalid(text::KEY_NOT_KEPT))
}

pub(crate) fn forget(home: &Path) -> Result<()> {
    store(home)
        .forget()
        .map_err(|_| invalid(text::KEY_NOT_FORGOTTEN))
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

#[cfg(not(feature = "test-stubs"))]
fn store(_home: &Path) -> impl TokenStore {
    Keychain::with_service(SERVICE).expect("the Console service name is valid")
}

#[cfg(feature = "test-stubs")]
fn store(home: &Path) -> impl TokenStore {
    TestStore(home.join(".voice-test-claude-console-key"))
}

#[cfg(feature = "test-stubs")]
struct TestStore(std::path::PathBuf);

#[cfg(feature = "test-stubs")]
impl TokenStore for TestStore {
    fn load(&self) -> Result<Option<Secret>> {
        use std::{fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt};
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.0)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(invalid(text::KEY_UNREADABLE)),
        };
        if !file
            .metadata()
            .map_err(|_| invalid(text::KEY_UNREADABLE))?
            .is_file()
        {
            return Err(invalid(text::KEY_UNREADABLE));
        }
        let mut value = String::new();
        let read = file.take(257).read_to_string(&mut value);
        let key = Secret::new(value);
        read.map_err(|_| invalid(text::KEY_UNREADABLE))?;
        Ok(Some(key))
    }

    fn save(&self, key: &Secret) -> Result<()> {
        use std::{
            fs::{self, OpenOptions},
            io::Write,
            os::unix::fs::{OpenOptionsExt, PermissionsExt},
        };
        let save = || -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(&self.0)?;
            if !file.metadata()?.is_file() {
                return Err(std::io::Error::other("not a regular file"));
            }
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
            file.set_len(0)?;
            file.write_all(key.expose().as_bytes())?;
            file.sync_all()
        };
        save().map_err(|_| invalid(text::KEY_NOT_KEPT))
    }

    fn forget(&self) -> Result<()> {
        match std::fs::remove_file(&self.0) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(invalid(text::KEY_NOT_FORGOTTEN)),
        }
    }
}
