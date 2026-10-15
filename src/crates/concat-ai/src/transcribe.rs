// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Montora contributors

//! Speech to text through a provider, each word with its own timing.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;

use crate::http::{self, AiError, Multipart};
use crate::provider::Endpoint;

/// The largest file a provider takes in one request. OpenAI and Groq both
/// stop at 25 MB on their free and paid plans alike; a longer recording is
/// sent in pieces by the caller.
pub const MAX_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;

/// What was said in a recording.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transcript {
    /// All of it, as one text.
    pub text: String,
    /// The language the provider heard, as it names it ("english", "fr").
    pub language: Option<String>,
    /// The recording's length, in seconds, when the provider says.
    pub duration: Option<f64>,
    /// The sentences or phrases, in order.
    pub segments: Vec<TranscriptSegment>,
    /// Every word, in order. Empty when the provider gives no word timings.
    pub words: Vec<TranscriptWord>,
}

/// A stretch of speech: a sentence or a phrase.
#[derive(Clone, Debug, PartialEq)]
pub struct TranscriptSegment {
    /// Where it starts, in seconds from the start of the file.
    pub start: f64,
    /// Where it ends.
    pub end: f64,
    /// What was said.
    pub text: String,
}

/// One word and when it was said: what a cut on a word, or the removal of
/// an "um", is made from.
#[derive(Clone, Debug, PartialEq)]
pub struct TranscriptWord {
    /// The word, as written.
    pub word: String,
    /// Where it starts, in seconds from the start of the file.
    pub start: f64,
    /// Where it ends.
    pub end: f64,
}

/// How long a provider may take over one file: it answers only once the
/// whole of it is transcribed.
const TRANSCRIBE_READ: Duration = Duration::from_secs(300);

/// Sends the audio file at `audio` to `endpoint` and returns its words.
/// `language` is a hint (ISO 639-1, "fr"); the provider guesses without
/// one. `cancel` is read before the upload starts.
pub fn transcribe(
    endpoint: &Endpoint,
    audio: &Path,
    language: Option<&str>,
    cancel: &AtomicBool,
) -> Result<Transcript, AiError> {
    let shown = audio.display();
    let size = std::fs::metadata(audio)
        .map_err(|error| AiError::File(format!("{shown} could not be read: {error}")))?
        .len();
    if size > MAX_UPLOAD_BYTES {
        return Err(AiError::File(format!(
            "{shown} is {} MB, more than the {} MB a provider takes at once",
            size.div_ceil(1024 * 1024),
            MAX_UPLOAD_BYTES / (1024 * 1024)
        )));
    }
    let bytes = std::fs::read(audio)
        .map_err(|error| AiError::File(format!("{shown} could not be read: {error}")))?;
    if cancel.load(Ordering::Relaxed) {
        return Err(AiError::Cancelled);
    }
    let name = audio.file_name().map_or_else(
        || "audio".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let answer = http::post_multipart(
        endpoint,
        "audio/transcriptions",
        form(endpoint, &name, &bytes, language),
        TRANSCRIBE_READ,
    )?;
    transcript(&answer)
}

/// The form a transcription is sent as: the file, the model, and a request
/// for the verbose answer with word and segment timings.
fn form(endpoint: &Endpoint, name: &str, bytes: &[u8], language: Option<&str>) -> Multipart {
    let mut form = Multipart::new();
    form.text("model", &endpoint.model);
    form.text("response_format", "verbose_json");
    form.text("timestamp_granularities[]", "word");
    form.text("timestamp_granularities[]", "segment");
    form.text("temperature", "0");
    if let Some(language) = language.filter(|language| !language.is_empty()) {
        form.text("language", language);
    }
    form.file("file", name, mime(name), bytes);
    form
}

/// The media type a provider expects for a file named `name`. They check
/// the extension as much as the header, so both say the same.
fn mime(name: &str) -> &'static str {
    let extension = Path::new(name)
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("mp3" | "mpga" | "mpeg") => "audio/mpeg",
        Some("m4a" | "mp4") => "audio/mp4",
        Some("ogg" | "opus") => "audio/ogg",
        Some("webm") => "audio/webm",
        _ => "application/octet-stream",
    }
}

/// The verbose answer, read.
fn transcript(answer: &Value) -> Result<Transcript, AiError> {
    let text = answer
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| AiError::Invalid("the transcription has no text".into()))?;
    let timed = |key: &str, field: &str| -> Vec<(String, f64, f64)> {
        answer
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                Some((
                    item.get(field)?.as_str()?.to_owned(),
                    item.get("start")?.as_f64()?,
                    item.get("end")?.as_f64()?,
                ))
            })
            .collect()
    };
    Ok(Transcript {
        text: text.trim().to_owned(),
        language: answer
            .get("language")
            .and_then(Value::as_str)
            .map(str::to_owned),
        duration: answer.get("duration").and_then(Value::as_f64),
        segments: timed("segments", "text")
            .into_iter()
            .map(|(text, start, end)| TranscriptSegment {
                start,
                end,
                text: text.trim().to_owned(),
            })
            .collect(),
        words: timed("words", "word")
            .into_iter()
            .map(|(word, start, end)| TranscriptWord {
                word: word.trim().to_owned(),
                start,
                end,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::tests::serve_once;
    use serde_json::json;

    fn scratch(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("montora-ai-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    const VERBOSE: &str = r#"{
        "task": "transcribe", "language": "french", "duration": 2.5,
        "text": " Bonjour, euh, tout le monde. ",
        "segments": [{"id": 0, "start": 0.0, "end": 2.4, "text": " Bonjour, euh, tout le monde."}],
        "words": [
            {"word": "Bonjour", "start": 0.0, "end": 0.6},
            {"word": " euh", "start": 0.8, "end": 1.1},
            {"word": "tout", "start": 1.3, "end": 1.5},
            {"word": "le", "start": 1.5, "end": 1.6},
            {"word": "monde", "start": 1.6, "end": 2.4}
        ]
    }"#;

    #[test]
    fn the_verbose_answer_gives_segments_and_timed_words() {
        let transcript = transcript(&serde_json::from_str(VERBOSE).unwrap()).unwrap();
        assert_eq!(transcript.text, "Bonjour, euh, tout le monde.");
        assert_eq!(transcript.language.as_deref(), Some("french"));
        assert_eq!(transcript.duration, Some(2.5));
        assert_eq!(
            transcript.segments,
            vec![TranscriptSegment {
                start: 0.0,
                end: 2.4,
                text: "Bonjour, euh, tout le monde.".into()
            }]
        );
        assert_eq!(transcript.words.len(), 5);
        assert_eq!(
            transcript.words[1],
            TranscriptWord {
                word: "euh".into(),
                start: 0.8,
                end: 1.1
            }
        );
    }

    #[test]
    fn an_answer_without_word_timings_has_no_words() {
        let transcript = transcript(&json!({"text": "Hi"})).unwrap();
        assert_eq!(transcript.text, "Hi");
        assert!(transcript.words.is_empty() && transcript.segments.is_empty());
        assert!(matches!(transcript_err(json!({})), AiError::Invalid(_)));
    }

    fn transcript_err(answer: Value) -> AiError {
        transcript(&answer).unwrap_err()
    }

    #[test]
    fn the_file_goes_up_with_the_model_and_the_timing_request() {
        let audio = scratch("voice.wav", b"RIFF-not-really");
        let (base, seen) = serve_once(200, &[], VERBOSE);
        let endpoint = Endpoint::new(&base, Some("k".into()), "whisper-large-v3-turbo");
        let transcript =
            transcribe(&endpoint, &audio, Some("fr"), &AtomicBool::new(false)).unwrap();
        assert_eq!(transcript.words.len(), 5);

        let seen = seen.recv().unwrap();
        assert_eq!(seen.line, "POST /v1/audio/transcriptions HTTP/1.1");
        assert!(
            seen.header("content-type")
                .unwrap()
                .starts_with("multipart/form-data; boundary=")
        );
        let body = String::from_utf8_lossy(&seen.body);
        for part in [
            "name=\"model\"\r\n\r\nwhisper-large-v3-turbo\r\n",
            "name=\"response_format\"\r\n\r\nverbose_json\r\n",
            "name=\"timestamp_granularities[]\"\r\n\r\nword\r\n",
            "name=\"timestamp_granularities[]\"\r\n\r\nsegment\r\n",
            "name=\"language\"\r\n\r\nfr\r\n",
            "filename=\"voice.wav\"\r\nContent-Type: audio/wav\r\n\r\nRIFF-not-really\r\n",
        ] {
            assert!(body.contains(part), "missing {part:?} in {body}");
        }
    }

    #[test]
    fn a_missing_file_is_a_file_error_and_sends_nothing() {
        let endpoint = Endpoint::new("http://127.0.0.1:9/v1", None, "m");
        let missing = std::env::temp_dir().join("montora-ai-no-such-file.wav");
        let error = transcribe(&endpoint, &missing, None, &AtomicBool::new(false)).unwrap_err();
        assert!(matches!(error, AiError::File(_)), "{error:?}");
    }

    #[test]
    fn a_cancelled_task_uploads_nothing() {
        let audio = scratch("cancelled.wav", b"RIFF");
        let endpoint = Endpoint::new("http://127.0.0.1:9/v1", None, "m");
        let error = transcribe(&endpoint, &audio, None, &AtomicBool::new(true)).unwrap_err();
        assert_eq!(error, AiError::Cancelled);
    }

    #[test]
    fn the_media_type_follows_the_extension() {
        assert_eq!(mime("a.WAV"), "audio/wav");
        assert_eq!(mime("a.flac"), "audio/flac");
        assert_eq!(mime("a.mp3"), "audio/mpeg");
        assert_eq!(mime("a.m4a"), "audio/mp4");
        assert_eq!(mime("a.opus"), "audio/ogg");
        assert_eq!(mime("a"), "application/octet-stream");
    }
}
