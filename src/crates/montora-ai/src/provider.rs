// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Montora contributors

//! Where each provider lives, and what it offers.

use serde::{Deserialize, Serialize};

use crate::http::AiError;

/// A service an AI task can run on. All of them speak the OpenAI-compatible
/// protocol; what differs is the address, whether a key is needed, and
/// whether they transcribe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    /// Groq: a free tier with rate limits, and Whisper transcription with
    /// word timestamps.
    Groq,
    /// OpenCode Zen: a gateway with some models free of charge. Which ones
    /// changes often, so the list is read from the service.
    OpencodeZen,
    /// xAI, the Grok models.
    Xai,
    /// OpenAI.
    Openai,
    /// Any other OpenAI-compatible server, at an address the user gives:
    /// Ollama, LM Studio, llama.cpp's server, a company gateway.
    Custom,
}

impl Provider {
    /// Every provider, in the order the settings list them.
    pub const ALL: [Provider; 5] = [
        Provider::Groq,
        Provider::OpencodeZen,
        Provider::Xai,
        Provider::Openai,
        Provider::Custom,
    ];

    /// The stable name settings and the credential store file it under.
    pub fn id(self) -> &'static str {
        match self {
            Provider::Groq => "groq",
            Provider::OpencodeZen => "opencode-zen",
            Provider::Xai => "xai",
            Provider::Openai => "openai",
            Provider::Custom => "custom",
        }
    }

    /// The provider an id names, if any.
    pub fn from_id(id: &str) -> Option<Provider> {
        Provider::ALL
            .into_iter()
            .find(|provider| provider.id() == id)
    }

    /// The name a person reads.
    pub fn label(self) -> &'static str {
        match self {
            Provider::Groq => "Groq",
            Provider::OpencodeZen => "OpenCode Zen",
            Provider::Xai => "xAI (Grok)",
            Provider::Openai => "OpenAI",
            Provider::Custom => "Custom server",
        }
    }

    /// The API's base address, which every path is joined to; `None` for
    /// [`Provider::Custom`], whose address the user gives.
    pub fn base_url(self) -> Option<&'static str> {
        match self {
            Provider::Groq => Some("https://api.groq.com/openai/v1"),
            Provider::OpencodeZen => Some("https://opencode.ai/zen/v1"),
            Provider::Xai => Some("https://api.x.ai/v1"),
            Provider::Openai => Some("https://api.openai.com/v1"),
            Provider::Custom => None,
        }
    }

    /// Whether a call needs a key. A server on one's own machine usually
    /// takes none.
    pub fn needs_key(self) -> bool {
        self != Provider::Custom
    }

    /// Where a person gets a key, for the settings to link to.
    pub fn keys_url(self) -> Option<&'static str> {
        match self {
            Provider::Groq => Some("https://console.groq.com/keys"),
            Provider::OpencodeZen => Some("https://opencode.ai/zen"),
            Provider::Xai => Some("https://console.x.ai"),
            Provider::Openai => Some("https://platform.openai.com/api-keys"),
            Provider::Custom => None,
        }
    }

    /// The model it transcribes with, word timestamps included, if it
    /// transcribes at all. A custom server's is not known in advance.
    pub fn transcription_model(self) -> Option<&'static str> {
        match self {
            Provider::Groq => Some("whisper-large-v3-turbo"),
            // whisper-1 is OpenAI's one transcription model that returns
            // word timestamps; the newer ones return text only.
            Provider::Openai => Some("whisper-1"),
            Provider::OpencodeZen | Provider::Xai | Provider::Custom => None,
        }
    }

    /// The endpoint for `model` on this provider, with `api_key`.
    /// `custom_url` is the address of a [`Provider::Custom`] server and is
    /// ignored for the others. Refuses a provider that needs a key when
    /// none is given, and a custom server without an address, before
    /// anything is sent.
    pub fn endpoint(
        self,
        custom_url: &str,
        api_key: Option<String>,
        model: &str,
    ) -> Result<Endpoint, AiError> {
        let base_url = match self.base_url() {
            Some(url) => url,
            None if custom_url.trim().is_empty() => {
                return Err(AiError::Invalid("the custom server has no address".into()));
            }
            None => custom_url.trim(),
        };
        let endpoint = Endpoint::new(base_url, api_key, model);
        if self.needs_key() && endpoint.api_key.is_none() {
            return Err(AiError::MissingKey);
        }
        Ok(endpoint)
    }
}

/// A provider made callable: its address, the user's key, and the model to
/// ask for.
#[derive(Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// The base address, without a trailing slash.
    pub base_url: String,
    /// The key, sent as a bearer token; `None` for a server that takes
    /// none.
    pub api_key: Option<String>,
    /// The model every request names.
    pub model: String,
}

impl Endpoint {
    /// An endpoint at `base_url`, which may end in a slash or not.
    pub fn new(base_url: &str, api_key: Option<String>, model: &str) -> Endpoint {
        Endpoint {
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key: api_key.filter(|key| !key.trim().is_empty()),
            model: model.to_owned(),
        }
    }

    /// `path` under the base address: `url("chat/completions")`.
    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }
}

// By hand, so a key never reaches a log line or a panic message.
impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Endpoint")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<set>"))
            .field("model", &self.model)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_comes_back_from_its_id() {
        for provider in Provider::ALL {
            assert_eq!(Provider::from_id(provider.id()), Some(provider));
        }
        assert_eq!(Provider::from_id("nobody"), None);
    }

    #[test]
    fn the_serialised_name_is_the_id() {
        for provider in Provider::ALL {
            let json = serde_json::to_string(&provider).unwrap();
            assert_eq!(json, format!("\"{}\"", provider.id()));
        }
    }

    #[test]
    fn only_the_custom_server_has_no_address_and_needs_no_key() {
        for provider in Provider::ALL {
            let custom = provider == Provider::Custom;
            assert_eq!(provider.base_url().is_none(), custom);
            assert_eq!(provider.needs_key(), !custom);
        }
    }

    #[test]
    fn a_provider_that_needs_a_key_refuses_to_go_without_one() {
        assert_eq!(
            Provider::Groq.endpoint("", None, "m").unwrap_err(),
            AiError::MissingKey
        );
        let endpoint = Provider::Groq
            .endpoint("", Some("gsk".into()), "m")
            .unwrap();
        assert_eq!(endpoint.base_url, "https://api.groq.com/openai/v1");
    }

    #[test]
    fn a_custom_server_needs_an_address_but_no_key() {
        let error = Provider::Custom.endpoint("  ", None, "m").unwrap_err();
        assert!(matches!(error, AiError::Invalid(_)), "{error:?}");
        let endpoint = Provider::Custom
            .endpoint(" http://localhost:11434/v1/ ", None, "llama3.2")
            .unwrap();
        assert_eq!(endpoint.base_url, "http://localhost:11434/v1");
        assert_eq!(endpoint.api_key, None);
    }

    #[test]
    fn a_preset_ignores_a_custom_address() {
        let endpoint = Provider::Openai
            .endpoint("http://elsewhere.test", Some("sk".into()), "m")
            .unwrap();
        assert_eq!(endpoint.base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn an_endpoint_joins_paths_with_one_slash() {
        let endpoint = Endpoint::new("http://localhost:11434/v1/", None, "llama3");
        assert_eq!(endpoint.url("models"), "http://localhost:11434/v1/models");
        assert_eq!(endpoint.url("/models"), "http://localhost:11434/v1/models");
    }

    #[test]
    fn a_blank_key_is_no_key() {
        let endpoint = Endpoint::new("https://x.test/v1", Some("  ".into()), "m");
        assert_eq!(endpoint.api_key, None);
    }

    #[test]
    fn debug_never_shows_the_key() {
        let endpoint = Endpoint::new("https://x.test/v1", Some("sk-secret".into()), "m");
        let shown = format!("{endpoint:?}");
        assert!(!shown.contains("sk-secret"), "{shown}");
        assert!(shown.contains("<set>"), "{shown}");
    }
}
