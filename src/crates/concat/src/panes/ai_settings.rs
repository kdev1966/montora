// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Montora contributors

//! Settings > AI: which provider the AI features run on, the key it takes,
//! and the model to ask for.
//!
//! The key is the one thing on the sheet that is not a preference. It goes
//! to the system's credential store through montora-ai, on a worker - a
//! Linux Secret Service may stop to ask for its password, and the window
//! must not stop with it - and it never comes back to the sheet: the page
//! knows only whether one is saved. What is typed waits in [`Secret`],
//! which prints as dots, until Save sends it on.
//!
//! The model list is the provider's own, asked for whenever there is a key
//! to ask with: which models a provider offers, and which are free, changes
//! too often to be written into the app. Asking is also the test that the
//! key and the address work, so the answer is what the page says.

use montora_ai::secrets::Secrets;
use montora_ai::{AiError, Provider};

use crate::host::spawn;
use crate::i18n::{t, tf};
use crate::panes::Msg;
use crate::panes::settings::SettingsMsg;
use crate::platform;
use crate::studio::Studio;

/// Text that must never reach a log line: what is typed into the key
/// field. Its Debug is dots, so a message carrying it can be logged whole.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<secret>")
    }
}

/// The AI page's state beyond the preferences.
#[derive(Default)]
pub struct AiSettings {
    /// What is in the key field, not yet saved.
    pub draft: Secret,
    /// Whether the chosen provider has a key in the store; `None` until
    /// the store has answered.
    pub key_saved: Option<bool>,
    /// A save or a forget is on its way to the store.
    pub key_busy: bool,
    /// The models the chosen provider listed, sorted.
    pub models: Vec<String>,
    /// The provider is being asked for its models.
    pub fetching: bool,
    /// What the page says under the model list: how many, or why none.
    pub status: String,
    /// `status` is a failure.
    pub failed: bool,
}

impl AiSettings {
    /// The chosen provider, from the preferences.
    pub fn provider(studio: &Studio) -> Option<Provider> {
        studio.prefs.ai.provider()
    }

    /// The page opened, or the provider changed: what is shown for the
    /// last one is dropped, and the store is asked about this one's key.
    pub fn reload(&mut self, studio: &Studio) {
        self.draft = Secret::default();
        self.models.clear();
        self.status.clear();
        self.failed = false;
        self.key_saved = None;
        let Some(provider) = Self::provider(studio) else {
            return;
        };
        let config = studio.host.dirs.config.clone();
        spawn(
            move || {
                Secrets::new(&config)
                    .api_key(provider)
                    .map(|key| key.is_some())
            },
            move |studio, _, _, result| {
                studio.handle(Msg::Settings(SettingsMsg::AiKeyState { provider, result }));
            },
        );
    }

    /// The store's answer about `provider`'s key. With a key, or for a
    /// server that takes none, the models are asked for straight away.
    pub fn key_state(&mut self, provider: Provider, result: Result<bool, String>, studio: &Studio) {
        if Self::provider(studio) != Some(provider) {
            return;
        }
        match result {
            Ok(saved) => {
                self.key_saved = Some(saved);
                if saved || !provider.needs_key() {
                    self.fetch_models(studio);
                }
            }
            Err(error) => {
                self.key_saved = Some(false);
                self.say(&error, true);
            }
        }
    }

    /// The provider picked from the page's list, by its place in
    /// [`Provider::ALL`].
    pub fn choose_provider(&mut self, index: i32, studio: &mut Studio) {
        let Some(provider) = Provider::ALL.get(index.max(0) as usize).copied() else {
            return;
        };
        studio.prefs.ai.provider = Some(provider.id().to_owned());
        studio.prefs.save(&studio.host.dirs);
        self.reload(studio);
    }

    /// The custom server's address, as typed. The list is dropped: it was
    /// the old address's.
    pub fn custom_url_edited(&mut self, text: &str, studio: &mut Studio) {
        studio.prefs.ai.custom_url = text.trim().to_owned();
        studio.prefs.save(&studio.host.dirs);
        self.models.clear();
        self.status.clear();
        self.failed = false;
    }

    /// Sends the typed key to the store, on a worker.
    pub fn save_key(&mut self, studio: &Studio) {
        let Some(provider) = Self::provider(studio) else {
            return;
        };
        if self.key_busy || self.draft.0.trim().is_empty() {
            return;
        }
        self.key_busy = true;
        let key = std::mem::take(&mut self.draft);
        let config = studio.host.dirs.config.clone();
        spawn(
            move || Secrets::new(&config).set_api_key(provider, &key.0),
            move |studio, _, _, result| {
                studio.handle(Msg::Settings(SettingsMsg::AiKeyStored {
                    provider,
                    saved: true,
                    result,
                }));
            },
        );
    }

    /// Removes the provider's key from the store, on a worker.
    pub fn forget_key(&mut self, studio: &Studio) {
        let Some(provider) = Self::provider(studio) else {
            return;
        };
        if self.key_busy {
            return;
        }
        self.key_busy = true;
        self.draft = Secret::default();
        let config = studio.host.dirs.config.clone();
        spawn(
            move || Secrets::new(&config).forget_api_key(provider),
            move |studio, _, _, result| {
                studio.handle(Msg::Settings(SettingsMsg::AiKeyStored {
                    provider,
                    saved: false,
                    result,
                }));
            },
        );
    }

    /// The store has saved or forgotten `provider`'s key, or says why not.
    pub fn key_stored(
        &mut self,
        provider: Provider,
        saved: bool,
        result: Result<(), String>,
        studio: &mut Studio,
    ) {
        self.key_busy = false;
        if let Err(error) = result {
            studio.notify(&error, true);
            return;
        }
        if Self::provider(studio) != Some(provider) {
            return;
        }
        self.key_saved = Some(saved);
        if saved {
            studio.notify(&t("settings.aiKeySavedToast"), false);
            self.fetch_models(studio);
        } else {
            studio.notify(&t("settings.aiKeyForgotten"), false);
            self.models.clear();
            self.status.clear();
            self.failed = false;
        }
    }

    /// Opens the page where the chosen provider hands out keys.
    pub fn open_keys_page(studio: &mut Studio) {
        let Some(url) = Self::provider(studio).and_then(Provider::keys_url) else {
            return;
        };
        if let Err(error) = platform::open_url(url) {
            studio.notify(&tf("settings.couldNotOpenLink", &[&url, &error]), true);
        }
    }

    /// Asks the chosen provider for its models, on a worker. The key is
    /// read from the store there, never kept here.
    pub fn fetch_models(&mut self, studio: &Studio) {
        let Some(provider) = Self::provider(studio) else {
            return;
        };
        if self.fetching {
            return;
        }
        self.fetching = true;
        self.say(&t("settings.aiLookingForModels"), false);
        let config = studio.host.dirs.config.clone();
        let custom_url = studio.prefs.ai.custom_url.clone();
        spawn(
            move || {
                let key = Secrets::new(&config)
                    .api_key(provider)
                    .map_err(AiError::File)?;
                let endpoint = provider.endpoint(&custom_url, key, "")?;
                montora_ai::models(&endpoint)
            },
            move |studio, _, _, result| {
                studio.handle(Msg::Settings(SettingsMsg::AiModelsFetched {
                    provider,
                    result,
                }));
            },
        );
    }

    /// The provider's answer: its models, or why it gave none. A provider
    /// with no model chosen yet gets the first on its list, so the
    /// features have something to ask for.
    pub fn models_fetched(
        &mut self,
        provider: Provider,
        result: Result<Vec<String>, AiError>,
        studio: &mut Studio,
    ) {
        self.fetching = false;
        if Self::provider(studio) != Some(provider) {
            return;
        }
        match result {
            Ok(models) => {
                self.say(&tf("settings.aiModelsFound", &[&models.len()]), false);
                if studio.prefs.ai.model(provider).is_none()
                    && let Some(first) = models.first()
                {
                    studio
                        .prefs
                        .ai
                        .models
                        .insert(provider.id().to_owned(), first.clone());
                    studio.prefs.save(&studio.host.dirs);
                }
                self.models = models;
            }
            Err(error) => {
                self.models.clear();
                self.say(&error_text(&error), true);
            }
        }
    }

    /// A model picked from the page's list.
    pub fn choose_model(&mut self, index: i32, studio: &mut Studio) {
        let Some(provider) = Self::provider(studio) else {
            return;
        };
        let Some(model) = self.models.get(index.max(0) as usize) else {
            return;
        };
        studio
            .prefs
            .ai
            .models
            .insert(provider.id().to_owned(), model.clone());
        studio.prefs.save(&studio.host.dirs);
    }

    /// The chosen model's place in the list, or -1 when it is not on it.
    pub fn model_index(&self, studio: &Studio) -> i32 {
        Self::provider(studio)
            .and_then(|provider| studio.prefs.ai.model(provider))
            .and_then(|chosen| self.models.iter().position(|model| model == chosen))
            .map_or(-1, |index| index as i32)
    }

    fn say(&mut self, text: &str, failed: bool) {
        self.status = text.to_owned();
        self.failed = failed;
    }
}

/// An AI failure in the person's language, saying what they can do.
pub fn error_text(error: &AiError) -> String {
    match error {
        AiError::MissingKey => t("ai.errorMissingKey"),
        AiError::Unauthorized(said) => tf("ai.errorUnauthorized", &[said]),
        AiError::RateLimited {
            retry_after: Some(seconds),
            ..
        } => tf("ai.errorRateLimitedWait", &[seconds]),
        AiError::RateLimited { message, .. } => tf("ai.errorRateLimited", &[message]),
        AiError::Status { code, message } => tf("ai.errorStatus", &[code, message]),
        AiError::Network(said) => tf("ai.errorNetwork", &[said]),
        AiError::Invalid(said) => tf("ai.errorInvalid", &[said]),
        AiError::File(said) => said.clone(),
        AiError::Cancelled => t("ai.cancelled"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_prints() {
        let secret = Secret("gsk-very-secret".into());
        assert_eq!(format!("{secret:?}"), "<secret>");
        let message = SettingsMsg::AiKeyEdited(secret);
        assert!(!format!("{message:?}").contains("gsk-very-secret"));
    }

    #[test]
    fn every_failure_reads_as_words() {
        for error in [
            AiError::MissingKey,
            AiError::Unauthorized("bad key".into()),
            AiError::RateLimited {
                retry_after: Some(30),
                message: "slow".into(),
            },
            AiError::RateLimited {
                retry_after: None,
                message: "slow".into(),
            },
            AiError::Status {
                code: 500,
                message: "oops".into(),
            },
            AiError::Network("refused".into()),
            AiError::Invalid("not json".into()),
            AiError::File("no file".into()),
            AiError::Cancelled,
        ] {
            let text = error_text(&error);
            assert!(!text.is_empty(), "{error:?}");
            assert!(
                !text.starts_with("ai."),
                "{error:?} reads as its key: {text}"
            );
        }
    }
}
