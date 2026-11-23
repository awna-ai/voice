//! The computer's own password store off a Mac: Windows' Credential Manager, or the Secret
//! Service on Linux (GNOME Keyring, KDE Wallet), through `keyring-core`.
//!
//! Kept apart from the save crate so `cargo check --target` runs for Windows and Linux from a Mac:
//! the save crate's TLS stack (`ring`) builds C code that needs the target's own system headers.
//! The macOS Keychain path stays in the save crate.

pub use keyring_core;
use keyring_core::{CredentialStore, Entry, Result};
use std::{collections::HashMap, sync::Arc};

/// This computer has no password store Voice can use: no Secret Service is running on
/// Linux, or the OS is neither Windows nor Linux.
#[derive(Debug, PartialEq, Eq)]
pub struct NoStore;

/// What the person's password manager shows for an item Voice stores under `service`.
/// New services should add their line here.
pub fn label(service: &str) -> String {
    match service {
        "voice.github" => "Voice: GitHub sign-in".to_owned(),
        "voice.claude-console" => "Voice: Claude Console key".to_owned(),
        _ => format!("Voice: {service}"),
    }
}

pub struct System {
    store: Arc<CredentialStore>,
    modifiers: &'static [(&'static str, &'static str)],
    labelled: bool,
}

impl System {
    /// Windows: `CredWriteW`/`CredReadW` generic credentials (windows-native-keyring-store).
    #[cfg(windows)]
    pub fn open() -> std::result::Result<Self, NoStore> {
        let store = windows_native_keyring_store::Store::new().map_err(|_| NoStore)?;
        // `Enterprise`, the store's default, roams with a domain profile to the person's other
        // computers; `Local` (CRED_PERSIST_LOCAL_MACHINE) keeps the token on this one.
        Ok(Self {
            store,
            modifiers: &[("persistence", "Local")],
            labelled: false,
        })
    }

    /// Linux: the Secret Service over D-Bus, in the default (login) collection. `Store::new`
    /// opens the session, so it fails here when no Secret Service runs (a headless machine).
    #[cfg(target_os = "linux")]
    pub fn open() -> std::result::Result<Self, NoStore> {
        let store = zbus_secret_service_keyring_store::Store::new().map_err(|_| NoStore)?;
        Ok(Self {
            store,
            modifiers: &[],
            labelled: true,
        })
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    pub fn open() -> std::result::Result<Self, NoStore> {
        Err(NoStore)
    }

    /// Any store, built with no modifiers (the mock store refuses them).
    pub fn over(store: Arc<CredentialStore>) -> Self {
        Self {
            store,
            modifiers: &[],
            labelled: false,
        }
    }

    fn modifiers(&self, service: &str) -> HashMap<&'static str, String> {
        let mut modifiers: HashMap<_, _> = self
            .modifiers
            .iter()
            .map(|&(key, value)| (key, value.to_owned()))
            .collect();
        if self.labelled {
            modifiers.insert("label", label(service));
        }
        modifiers
    }

    pub fn entry(&self, service: &str, user: &str) -> Result<Entry> {
        let owned = self.modifiers(service);
        let modifiers: HashMap<&str, &str> = owned
            .iter()
            .map(|(&key, value)| (key, value.as_str()))
            .collect();
        self.store.build(service, user, Some(&modifiers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_match_each_service() {
        assert_eq!(label("voice.github"), "Voice: GitHub sign-in");
        assert_eq!(label("voice.claude-console"), "Voice: Claude Console key");
        assert_eq!(label("voice.one"), "Voice: voice.one");
        assert_eq!(label("other.service"), "Voice: other.service");
    }

    #[test]
    fn labelled_modifiers_match_each_service() {
        let system = System {
            store: keyring_core::mock::Store::new().unwrap(),
            modifiers: &[],
            labelled: true,
        };
        assert_eq!(
            system.modifiers("voice.github"),
            HashMap::from([("label", "Voice: GitHub sign-in".to_owned())])
        );
        assert_eq!(
            system.modifiers("voice.claude-console"),
            HashMap::from([("label", "Voice: Claude Console key".to_owned())])
        );
    }

    #[test]
    fn unlabelled_modifiers_are_empty_for_both_services() {
        let system = System::over(keyring_core::mock::Store::new().unwrap());
        assert!(system.modifiers("voice.github").is_empty());
        assert!(system.modifiers("voice.claude-console").is_empty());
    }

    #[test]
    fn labels_preserve_fixed_modifiers() {
        let system = System {
            store: keyring_core::mock::Store::new().unwrap(),
            modifiers: &[("persistence", "Local")],
            labelled: true,
        };
        assert_eq!(
            system.modifiers("voice.claude-console"),
            HashMap::from([
                ("persistence", "Local".to_owned()),
                ("label", "Voice: Claude Console key".to_owned()),
            ])
        );
    }
}
