// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Montora contributors

//! The HTTP underneath every call, and the errors a person can act on.

use std::time::Duration;

use serde_json::Value;

use crate::provider::Endpoint;

/// Why a call did not give an answer, sorted by what the person can do
/// about it: fix the key, wait, check the connection, pick another model.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AiError {
    /// The provider needs a key and none is set.
    #[error("no API key is set for this provider")]
    MissingKey,
    /// The key was refused (401 or 403).
    #[error("the provider refused the API key: {0}")]
    Unauthorized(String),
    /// Too many requests, or the free quota is used up (429).
    #[error("the provider's rate limit was reached: {message}")]
    RateLimited {
        /// How long the provider asked to wait, in seconds, when it said.
        retry_after: Option<u64>,
        /// What the provider said.
        message: String,
    },
    /// Any other answer that was not a success.
    #[error("the provider answered {code}: {message}")]
    Status {
        /// The HTTP status.
        code: u16,
        /// What the provider said.
        message: String,
    },
    /// The provider could not be reached, or stopped answering.
    #[error("the provider could not be reached: {0}")]
    Network(String),
    /// The answer came, but not in a shape this client reads.
    #[error("the provider's answer could not be read: {0}")]
    Invalid(String),
    /// A local file the request needed could not be read.
    #[error("{0}")]
    File(String),
    /// The task was cancelled before this call.
    #[error("cancelled")]
    Cancelled,
}

/// How long a call may take to connect.
const CONNECT: Duration = Duration::from_secs(20);

/// A client whose reads give up after `read`. A model writing a long reply
/// sends nothing until it is done, so a chat's limit is generous.
fn agent(read: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(CONNECT)
        .timeout_read(read)
        .user_agent(&format!("Montora/{}", env!("CARGO_PKG_VERSION")))
        .build()
}

/// The request with the key on it, if the endpoint has one.
fn authorised(request: ureq::Request, endpoint: &Endpoint) -> ureq::Request {
    match &endpoint.api_key {
        Some(key) => request.set("Authorization", &format!("Bearer {key}")),
        None => request,
    }
}

/// GET `path`, read as JSON.
pub(crate) fn get_json(endpoint: &Endpoint, path: &str, read: Duration) -> Result<Value, AiError> {
    let request = authorised(agent(read).get(&endpoint.url(path)), endpoint);
    read_json(request.call())
}

/// POST `body` as JSON to `path`, read the answer as JSON.
pub(crate) fn post_json(
    endpoint: &Endpoint,
    path: &str,
    body: &Value,
    read: Duration,
) -> Result<Value, AiError> {
    let request = authorised(agent(read).post(&endpoint.url(path)), endpoint)
        .set("Content-Type", "application/json");
    read_json(request.send_string(&body.to_string()))
}

/// POST `form` to `path`, read the answer as JSON.
pub(crate) fn post_multipart(
    endpoint: &Endpoint,
    path: &str,
    form: Multipart,
    read: Duration,
) -> Result<Value, AiError> {
    let request = authorised(agent(read).post(&endpoint.url(path)), endpoint).set(
        "Content-Type",
        &format!("multipart/form-data; boundary={}", form.boundary),
    );
    read_json(request.send_bytes(&form.finish()))
}

/// The answer as JSON, or the error it amounts to.
fn read_json(result: Result<ureq::Response, ureq::Error>) -> Result<Value, AiError> {
    match result {
        Ok(response) => {
            let text = response
                .into_string()
                .map_err(|error| AiError::Network(format!("the answer was cut short: {error}")))?;
            serde_json::from_str(&text).map_err(|error| AiError::Invalid(error.to_string()))
        }
        Err(ureq::Error::Status(code, response)) => {
            let retry_after = response
                .header("Retry-After")
                .and_then(|value| value.trim().parse::<u64>().ok());
            let body = response.into_string().unwrap_or_default();
            Err(status_error(code, &body, retry_after))
        }
        Err(ureq::Error::Transport(transport)) => Err(AiError::Network(transport.to_string())),
    }
}

/// A failed status as the error a person can act on, with the provider's
/// own words when its body carries them.
pub(crate) fn status_error(code: u16, body: &str, retry_after: Option<u64>) -> AiError {
    let message = provider_message(body);
    match code {
        401 | 403 => AiError::Unauthorized(message),
        429 => AiError::RateLimited {
            retry_after,
            message,
        },
        _ => AiError::Status { code, message },
    }
}

/// What a provider said in an error body. OpenAI and those that copy it
/// write `{"error": {"message": ...}}`; some write `{"error": "..."}` or
/// `{"message": ...}`; anything else is shown as it came, shortened.
fn provider_message(body: &str) -> String {
    let said = serde_json::from_str::<Value>(body).ok().and_then(|value| {
        value
            .pointer("/error/message")
            .or_else(|| value.get("error").filter(|error| error.is_string()))
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let text = said.unwrap_or_else(|| body.trim().to_owned());
    if text.is_empty() {
        "no reason given".to_owned()
    } else {
        text.chars().take(300).collect()
    }
}

/// A multipart/form-data body, built by hand: ureq 2 has no form encoder,
/// and a transcription is one file and a few fields.
pub(crate) struct Multipart {
    boundary: String,
    body: Vec<u8>,
}

impl Multipart {
    /// An empty form, with a boundary no audio file or field will contain.
    pub(crate) fn new() -> Multipart {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        Multipart {
            boundary: format!("montora-form-{nanos:x}-{:x}", std::process::id()),
            body: Vec::new(),
        }
    }

    /// A text field.
    pub(crate) fn text(&mut self, name: &str, value: &str) {
        self.head(&format!(
            "Content-Disposition: form-data; name=\"{name}\"\r\n"
        ));
        self.body.extend_from_slice(value.as_bytes());
        self.body.extend_from_slice(b"\r\n");
    }

    /// A file field: `bytes`, sent as `filename` of type `mime`.
    pub(crate) fn file(&mut self, name: &str, filename: &str, mime: &str, bytes: &[u8]) {
        let filename = filename.replace(['"', '\r', '\n'], "_");
        self.head(&format!(
            "Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n\
             Content-Type: {mime}\r\n"
        ));
        self.body.extend_from_slice(bytes);
        self.body.extend_from_slice(b"\r\n");
    }

    fn head(&mut self, headers: &str) {
        self.body
            .extend_from_slice(format!("--{}\r\n{headers}\r\n", self.boundary).as_bytes());
    }

    /// The encoded body, closed.
    fn finish(mut self) -> Vec<u8> {
        self.body
            .extend_from_slice(format!("--{}--\r\n", self.boundary).as_bytes());
        self.body
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// One request as a test server saw it.
    pub(crate) struct Seen {
        /// The request line: `POST /v1/chat/completions HTTP/1.1`.
        pub line: String,
        /// The headers, names lowercased.
        pub headers: Vec<(String, String)>,
        /// The body, raw.
        pub body: Vec<u8>,
    }

    impl Seen {
        pub(crate) fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        }
    }

    /// A server on a free local port that answers one request with
    /// `status`, `headers` and `body`, and hands back what it was sent.
    pub(crate) fn serve_once(
        status: u16,
        headers: &[(&str, &str)],
        body: &str,
    ) -> (String, mpsc::Receiver<Seen>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let extra: String = headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect();
        let answer = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
            body.len()
        );
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut headers = Vec::new();
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                let header = header.trim_end();
                if header.is_empty() {
                    break;
                }
                if let Some((name, value)) = header.split_once(':') {
                    headers.push((name.trim().to_lowercase(), value.trim().to_owned()));
                }
            }
            let length = headers
                .iter()
                .find(|(name, _)| name == "content-length")
                .map_or(0, |(_, value)| value.parse::<usize>().unwrap());
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let mut stream = stream;
            stream.write_all(answer.as_bytes()).unwrap();
            let _ = sender.send(Seen {
                line: line.trim_end().to_owned(),
                headers,
                body,
            });
        });
        (base, receiver)
    }

    #[test]
    fn a_refused_key_is_unauthorised_with_the_providers_words() {
        let error = status_error(401, r#"{"error":{"message":"Invalid API Key"}}"#, None);
        assert_eq!(error, AiError::Unauthorized("Invalid API Key".into()));
    }

    #[test]
    fn a_rate_limit_carries_how_long_to_wait() {
        let error = status_error(429, r#"{"error":"slow down"}"#, Some(12));
        assert_eq!(
            error,
            AiError::RateLimited {
                retry_after: Some(12),
                message: "slow down".into()
            }
        );
    }

    #[test]
    fn a_body_that_is_not_json_is_shown_shortened() {
        let body = "x".repeat(1000);
        let AiError::Status { code, message } = status_error(502, &body, None) else {
            panic!("not a status error");
        };
        assert_eq!(code, 502);
        assert_eq!(message.len(), 300);
        let AiError::Status { message, .. } = status_error(500, "", None) else {
            panic!("not a status error");
        };
        assert_eq!(message, "no reason given");
    }

    #[test]
    fn the_key_goes_as_a_bearer_token_and_a_429_reads_its_retry_after() {
        let (base, seen) = serve_once(429, &[("Retry-After", "7")], r#"{"message":"quota"}"#);
        let endpoint = Endpoint::new(&base, Some("sk-test".into()), "m");
        let error = get_json(&endpoint, "models", Duration::from_secs(5)).unwrap_err();
        assert_eq!(
            error,
            AiError::RateLimited {
                retry_after: Some(7),
                message: "quota".into()
            }
        );
        let seen = seen.recv().unwrap();
        assert_eq!(seen.line, "GET /v1/models HTTP/1.1");
        assert_eq!(seen.header("authorization"), Some("Bearer sk-test"));
        assert!(seen.header("user-agent").unwrap().starts_with("Montora/"));
    }

    #[test]
    fn no_key_sends_no_authorisation() {
        let (base, seen) = serve_once(200, &[], r#"{"data":[]}"#);
        let endpoint = Endpoint::new(&base, None, "m");
        get_json(&endpoint, "models", Duration::from_secs(5)).unwrap();
        assert_eq!(seen.recv().unwrap().header("authorization"), None);
    }

    #[test]
    fn an_unreachable_server_is_a_network_error() {
        // A port that was free a moment ago: nothing listens on it now.
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let endpoint = Endpoint::new(&format!("http://127.0.0.1:{port}/v1"), None, "m");
        let error = get_json(&endpoint, "models", Duration::from_secs(5)).unwrap_err();
        assert!(matches!(error, AiError::Network(_)), "{error:?}");
    }

    #[test]
    fn a_multipart_form_has_its_fields_and_closes() {
        let mut form = Multipart::new();
        let boundary = form.boundary.clone();
        form.text("model", "whisper-1");
        form.file("file", "a\"b.wav", "audio/wav", b"RIFF");
        let body = String::from_utf8(form.finish()).unwrap();
        assert!(body.starts_with(&format!("--{boundary}\r\n")));
        assert!(body.contains("name=\"model\"\r\n\r\nwhisper-1\r\n"));
        assert!(body.contains("filename=\"a_b.wav\"\r\nContent-Type: audio/wav\r\n\r\nRIFF\r\n"));
        assert!(body.ends_with(&format!("--{boundary}--\r\n")));
    }
}
