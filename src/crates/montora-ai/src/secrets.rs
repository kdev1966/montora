// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Montora contributors

//! The user's API keys, kept by the system rather than by Montora.
//!
//! On macOS, Windows and Linux a key goes to the system's credential store
//! (the Keychain, the Credential Manager, the Secret Service) under
//! Montora's app id, and never into settings.json, which a person may copy,
//! back up or attach to a bug report. On Android, where Montora reaches no
//! such store, a key is a file of its own in the app's private folder,
//! readable by the app alone, which is what Android's sandbox offers every
//! app.
//!
//! A Linux desktop without a Secret Service (a bare window manager, a
//! server) is told so, rather than having the key quietly written to a
//! file: the person chose the system store.

use std::path::{Path, PathBuf};

use crate::provider::Provider;

/// Where the keys are kept.
pub struct Secrets {
    store: Store,
}

enum Store {
    /// The system's credential store; see [`system`].
    System,
    /// One file per key in this folder, private to the app.
    Files(PathBuf),
}

impl Secrets {
    /// The keys of this machine's user: the system store where there is
    /// one, otherwise files under `config`, the app's configuration folder.
    pub fn new(config: &Path) -> Secrets {
        if system::AVAILABLE {
            Secrets {
                store: Store::System,
            }
        } else {
            Secrets::in_files(config)
        }
    }

    /// Keys kept as files under `config`: the store on a phone, and the one
    /// tests use, so they never touch the machine's real keychain.
    pub fn in_files(config: &Path) -> Secrets {
        Secrets {
            store: Store::Files(config.join("secrets")),
        }
    }

    /// The key set for `provider`, or `None` when there is none.
    pub fn api_key(&self, provider: Provider) -> Result<Option<String>, String> {
        match &self.store {
            Store::System => system::get(provider),
            Store::Files(folder) => match std::fs::read_to_string(key_file(folder, provider)) {
                Ok(key) => Ok(Some(key.trim().to_owned()).filter(|key| !key.is_empty())),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(format!("the API key could not be read: {error}")),
            },
        }
    }

    /// Keeps `key` for `provider`, in place of any it had. A blank key
    /// removes it.
    pub fn set_api_key(&self, provider: Provider, key: &str) -> Result<(), String> {
        let key = key.trim();
        if key.is_empty() {
            return self.forget_api_key(provider);
        }
        match &self.store {
            Store::System => system::set(provider, key),
            Store::Files(folder) => write_private(&key_file(folder, provider), key),
        }
    }

    /// Removes the key for `provider`. Removing one that is not there is
    /// not an error.
    pub fn forget_api_key(&self, provider: Provider) -> Result<(), String> {
        match &self.store {
            Store::System => system::delete(provider),
            Store::Files(folder) => match std::fs::remove_file(key_file(folder, provider)) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("the API key could not be removed: {error}")),
            },
        }
    }
}

/// The system's credential store, through keyring.
#[cfg(any(
    target_os = "macos",
    windows,
    all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )
))]
mod system {
    use crate::provider::Provider;

    /// This platform has a store keyring reaches.
    pub const AVAILABLE: bool = true;

    /// The service keys are filed under: the app id.
    const SERVICE: &str = "io.github.kdev1966.montora";

    fn entry(provider: Provider) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, &format!("api-key.{}", provider.id()))
            .map_err(|error| refused(&error))
    }

    pub fn get(provider: Provider) -> Result<Option<String>, String> {
        match entry(provider)?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(refused(&error)),
        }
    }

    pub fn set(provider: Provider, key: &str) -> Result<(), String> {
        entry(provider)?
            .set_password(key)
            .map_err(|error| refused(&error))
    }

    pub fn delete(provider: Provider) -> Result<(), String> {
        match entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(refused(&error)),
        }
    }

    /// A store error, said so a person knows where to look.
    fn refused(error: &keyring::Error) -> String {
        match error {
            keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => format!(
                "the system's credential store is not available ({error}); on Linux, a \
                 Secret Service such as GNOME Keyring or KWallet has to be running"
            ),
            other => format!("the system's credential store refused: {other}"),
        }
    }
}

/// No system store here: [`Secrets::new`] keeps keys in files instead and
/// never builds a [`Store::System`], so these only say why if it did.
#[cfg(not(any(
    target_os = "macos",
    windows,
    all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )
)))]
mod system {
    use crate::provider::Provider;

    /// This platform has no store keyring reaches.
    pub const AVAILABLE: bool = false;

    const NONE: &str = "this platform has no system credential store";

    pub fn get(_: Provider) -> Result<Option<String>, String> {
        Err(NONE.to_owned())
    }

    pub fn set(_: Provider, _: &str) -> Result<(), String> {
        Err(NONE.to_owned())
    }

    pub fn delete(_: Provider) -> Result<(), String> {
        Err(NONE.to_owned())
    }
}

/// The file `provider`'s key is kept in.
fn key_file(folder: &Path, provider: Provider) -> PathBuf {
    folder.join(format!("{}.key", provider.id()))
}

/// Writes `text` to `path`, readable by this user alone, replacing the file
/// whole so a crash never leaves half a key.
fn write_private(path: &Path, text: &str) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("the API key could not be saved: {error}");
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(failed)?;
    }
    let partial = path.with_extension("key.partial");
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&partial).map_err(failed)?;
        file.write_all(text.as_bytes()).map_err(failed)?;
        file.sync_all().map_err(failed)?;
    }
    std::fs::rename(&partial, path).map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("montora-secrets-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn a_key_is_kept_read_back_replaced_and_forgotten() {
        let secrets = Secrets::in_files(&folder("cycle"));
        assert_eq!(secrets.api_key(Provider::Groq).unwrap(), None);
        secrets.set_api_key(Provider::Groq, "  gsk-one \n").unwrap();
        assert_eq!(
            secrets.api_key(Provider::Groq).unwrap().as_deref(),
            Some("gsk-one")
        );
        secrets.set_api_key(Provider::Groq, "gsk-two").unwrap();
        assert_eq!(
            secrets.api_key(Provider::Groq).unwrap().as_deref(),
            Some("gsk-two")
        );
        secrets.forget_api_key(Provider::Groq).unwrap();
        assert_eq!(secrets.api_key(Provider::Groq).unwrap(), None);
        secrets.forget_api_key(Provider::Groq).unwrap();
    }

    #[test]
    fn each_provider_has_its_own_key() {
        let secrets = Secrets::in_files(&folder("apart"));
        secrets.set_api_key(Provider::Groq, "g").unwrap();
        secrets.set_api_key(Provider::OpencodeZen, "o").unwrap();
        assert_eq!(
            secrets.api_key(Provider::Groq).unwrap().as_deref(),
            Some("g")
        );
        assert_eq!(
            secrets.api_key(Provider::OpencodeZen).unwrap().as_deref(),
            Some("o")
        );
        assert_eq!(secrets.api_key(Provider::Xai).unwrap(), None);
    }

    #[test]
    fn setting_a_blank_key_forgets_it() {
        let secrets = Secrets::in_files(&folder("blank"));
        secrets.set_api_key(Provider::Openai, "sk").unwrap();
        secrets.set_api_key(Provider::Openai, "   ").unwrap();
        assert_eq!(secrets.api_key(Provider::Openai).unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_key_file_is_readable_by_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt;
        let root = folder("mode");
        Secrets::in_files(&root)
            .set_api_key(Provider::Xai, "xai-key")
            .unwrap();
        let mode = std::fs::metadata(root.join("secrets").join("xai.key"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(!root.join("secrets").join("xai.key.partial").exists());
    }
}
