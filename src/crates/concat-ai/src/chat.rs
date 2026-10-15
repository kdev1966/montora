// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Montora contributors

//! A conversation with a text model, and the models an endpoint serves.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::http::{self, AiError};
use crate::provider::Endpoint;

/// Who says a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The instructions the model follows.
    System,
    /// What the person (or the feature, on their behalf) asks.
    User,
    /// What the model answered before, for a conversation that goes on.
    Assistant,
}

/// One message of a conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Who says it.
    pub role: Role,
    /// What is said.
    pub content: String,
}

impl Message {
    /// Instructions for the model.
    pub fn system(content: impl Into<String>) -> Message {
        Message {
            role: Role::System,
            content: content.into(),
        }
    }

    /// A request to the model.
    pub fn user(content: impl Into<String>) -> Message {
        Message {
            role: Role::User,
            content: content.into(),
        }
    }
}

/// What to send.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatRequest {
    /// The conversation, oldest first.
    pub messages: Vec<Message>,
    /// How freely the model words its answer, 0 to 2; the provider's own
    /// default when `None`.
    pub temperature: Option<f32>,
    /// The longest answer, in tokens; the provider's default when `None`.
    pub max_tokens: Option<u32>,
    /// Ask for the answer as a JSON object. The messages must say what the
    /// object holds: providers refuse JSON mode otherwise.
    pub json: bool,
}

/// What came back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatReply {
    /// The answer.
    pub content: String,
    /// The model that answered, as the provider names it.
    pub model: String,
    /// Why the answer stopped: `stop` when it was finished, `length` when
    /// it ran out of tokens.
    pub finish_reason: Option<String>,
}

/// How long a model may take over one answer. It sends nothing until it
/// has finished, and a long summary on a busy free tier takes a while.
const CHAT_READ: Duration = Duration::from_secs(180);

/// Sends `request` to `endpoint` and returns the model's answer. `cancel`
/// is read before the call: a request in flight runs to its end.
pub fn chat(
    endpoint: &Endpoint,
    request: &ChatRequest,
    cancel: &AtomicBool,
) -> Result<ChatReply, AiError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(AiError::Cancelled);
    }
    let answer = http::post_json(
        endpoint,
        "chat/completions",
        &body(endpoint, request),
        CHAT_READ,
    )?;
    reply(&answer)
}

/// The JSON a chat request is sent as.
fn body(endpoint: &Endpoint, request: &ChatRequest) -> Value {
    let mut body = json!({
        "model": endpoint.model,
        "messages": request.messages,
        "stream": false,
    });
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    if let Some(max_tokens) = request.max_tokens {
        body["max_tokens"] = json!(max_tokens);
    }
    if request.json {
        body["response_format"] = json!({"type": "json_object"});
    }
    body
}

/// The first choice's message, from a chat completion.
fn reply(answer: &Value) -> Result<ChatReply, AiError> {
    let choice = answer
        .pointer("/choices/0")
        .ok_or_else(|| AiError::Invalid("the answer has no choices".into()))?;
    let content = choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| AiError::Invalid("the answer has no message text".into()))?;
    Ok(ChatReply {
        content: content.to_owned(),
        model: answer
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        finish_reason: choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// The models `endpoint` serves, by id, sorted. Which models a provider
/// offers - and which are free - changes too often to be written down
/// here, so the settings ask.
pub fn models(endpoint: &Endpoint) -> Result<Vec<String>, AiError> {
    let answer = http::get_json(endpoint, "models", Duration::from_secs(30))?;
    let data = answer
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| AiError::Invalid("the model list has no data".into()))?;
    let mut ids: Vec<String> = data
        .iter()
        .filter_map(|model| model.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::tests::serve_once;

    fn request() -> ChatRequest {
        ChatRequest {
            messages: vec![Message::system("Be brief."), Message::user("Hello")],
            temperature: Some(0.2),
            max_tokens: Some(64),
            json: true,
        }
    }

    #[test]
    fn the_request_names_the_model_and_carries_every_option() {
        let endpoint = Endpoint::new("https://x.test/v1", None, "llama-3.3-70b");
        let body = body(&endpoint, &request());
        assert_eq!(body["model"], "llama-3.3-70b");
        assert_eq!(body["stream"], false);
        assert_eq!(
            body["messages"][0],
            json!({"role": "system", "content": "Be brief."})
        );
        assert_eq!(body["messages"][1]["role"], "user");
        assert!((body["temperature"].as_f64().unwrap() - 0.2).abs() < 1e-6);
        assert_eq!(body["max_tokens"], 64);
        assert_eq!(body["response_format"], json!({"type": "json_object"}));
    }

    #[test]
    fn options_left_unset_are_not_sent() {
        let endpoint = Endpoint::new("https://x.test/v1", None, "m");
        let body = body(&endpoint, &ChatRequest::default());
        let object = body.as_object().unwrap();
        for key in ["temperature", "max_tokens", "response_format"] {
            assert!(!object.contains_key(key), "{key} was sent");
        }
    }

    #[test]
    fn a_chat_goes_to_chat_completions_and_reads_the_first_choice() {
        let (base, seen) = serve_once(
            200,
            &[],
            r#"{"model":"m-2","choices":[{"message":{"role":"assistant","content":"Hi."},"finish_reason":"stop"}]}"#,
        );
        let endpoint = Endpoint::new(&base, Some("k".into()), "m");
        let reply = chat(&endpoint, &request(), &AtomicBool::new(false)).unwrap();
        assert_eq!(
            reply,
            ChatReply {
                content: "Hi.".into(),
                model: "m-2".into(),
                finish_reason: Some("stop".into())
            }
        );
        let seen = seen.recv().unwrap();
        assert_eq!(seen.line, "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(seen.header("content-type"), Some("application/json"));
        let sent: Value = serde_json::from_slice(&seen.body).unwrap();
        assert_eq!(sent["messages"][1]["content"], "Hello");
    }

    #[test]
    fn a_cancelled_task_sends_nothing() {
        let endpoint = Endpoint::new("http://127.0.0.1:9/v1", None, "m");
        let error = chat(&endpoint, &request(), &AtomicBool::new(true)).unwrap_err();
        assert_eq!(error, AiError::Cancelled);
    }

    #[test]
    fn an_answer_without_text_is_invalid() {
        let error = reply(&json!({"choices": [{"message": {"content": null}}]})).unwrap_err();
        assert!(matches!(error, AiError::Invalid(_)), "{error:?}");
        let error = reply(&json!({"choices": []})).unwrap_err();
        assert!(matches!(error, AiError::Invalid(_)), "{error:?}");
    }

    #[test]
    fn the_model_list_is_sorted_and_deduplicated() {
        let (base, seen) = serve_once(
            200,
            &[],
            r#"{"object":"list","data":[{"id":"b"},{"id":"a"},{"id":"b"},{"object":"model"}]}"#,
        );
        let endpoint = Endpoint::new(&base, None, "");
        assert_eq!(models(&endpoint).unwrap(), vec!["a", "b"]);
        assert_eq!(seen.recv().unwrap().line, "GET /v1/models HTTP/1.1");
    }
}
