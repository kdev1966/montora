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
    let name = audio.file_name().map_or_else(
        || "audio".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    transcribe_bytes(endpoint, &name, &bytes, language, cancel)
}

/// Sends `bytes`, an audio file named `name`, to `endpoint` and returns its
/// words. The name's extension is what the provider reads the format from.
pub fn transcribe_bytes(
    endpoint: &Endpoint,
    name: &str,
    bytes: &[u8],
    language: Option<&str>,
    cancel: &AtomicBool,
) -> Result<Transcript, AiError> {
    if bytes.len() as u64 > MAX_UPLOAD_BYTES {
        return Err(AiError::File(format!(
            "{name} is {} MB, more than the {} MB a provider takes at once",
            (bytes.len() as u64).div_ceil(1024 * 1024),
            MAX_UPLOAD_BYTES / (1024 * 1024)
        )));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AiError::Cancelled);
    }
    let answer = http::post_multipart(
        endpoint,
        "audio/transcriptions",
        form(endpoint, name, bytes, language),
        TRANSCRIBE_READ,
    )?;
    transcript(&answer)
}

/// The longest piece of a recording sent in one request: ten minutes, which
/// as 16 kHz 16-bit mono is 19 MB, inside every provider's 25 MB.
const PIECE_SECONDS: f64 = 600.0;

/// How far before a piece's end the cut may move to land in a pause rather
/// than in a word.
const CUT_SEARCH_SECONDS: f64 = 20.0;

/// Transcribes `samples`, mono at `rate` Hz, however long: in pieces of at
/// most [`PIECE_SECONDS`], each cut in the quietest moment near its end so
/// no word is split, each sent as WAV made in memory. The words and
/// segments come back on one clock, seconds from the first sample.
/// `progress` is told before each piece is sent, as (pieces done, pieces);
/// `cancel` is read between pieces.
pub fn transcribe_samples(
    endpoint: &Endpoint,
    samples: &[f32],
    rate: u32,
    language: Option<&str>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize),
) -> Result<Transcript, AiError> {
    if samples.is_empty() || rate == 0 {
        return Err(AiError::File("there is no sound to transcribe".into()));
    }
    let pieces = pieces(samples, rate);
    let mut whole = Transcript {
        duration: Some(samples.len() as f64 / f64::from(rate)),
        ..Transcript::default()
    };
    for (index, range) in pieces.iter().enumerate() {
        progress(index, pieces.len());
        let offset = range.start as f64 / f64::from(rate);
        let part = transcribe_bytes(
            endpoint,
            "voice.wav",
            &wav(&samples[range.clone()], rate),
            language,
            cancel,
        )?;
        if whole.language.is_none() {
            whole.language = part.language;
        }
        if !part.text.is_empty() {
            if !whole.text.is_empty() {
                whole.text.push(' ');
            }
            whole.text.push_str(&part.text);
        }
        whole
            .segments
            .extend(part.segments.into_iter().map(|segment| TranscriptSegment {
                start: segment.start + offset,
                end: segment.end + offset,
                ..segment
            }));
        whole
            .words
            .extend(part.words.into_iter().map(|word| TranscriptWord {
                start: word.start + offset,
                end: word.end + offset,
                ..word
            }));
    }
    progress(pieces.len(), pieces.len());
    Ok(whole)
}

/// Where `samples` is cut into pieces: none longer than [`PIECE_SECONDS`],
/// each ending at the quietest tenth of a second in the last
/// [`CUT_SEARCH_SECONDS`] before that limit.
fn pieces(samples: &[f32], rate: u32) -> Vec<std::ops::Range<usize>> {
    let rate = rate as usize;
    let longest = (PIECE_SECONDS as usize) * rate;
    let search = (CUT_SEARCH_SECONDS as usize) * rate;
    let frame = (rate / 10).max(1);
    let mut pieces = Vec::new();
    let mut start = 0;
    while samples.len() - start > longest {
        let limit = start + longest;
        let from = limit - search;
        let quietest = (from..limit - frame)
            .step_by(frame)
            .min_by(|&a, &b| {
                energy(&samples[a..a + frame]).total_cmp(&energy(&samples[b..b + frame]))
            })
            .map_or(limit, |at| at + frame / 2);
        pieces.push(start..quietest);
        start = quietest;
    }
    pieces.push(start..samples.len());
    pieces
}

/// The mean square of `samples`: how loud they are.
fn energy(samples: &[f32]) -> f32 {
    samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32
}

/// `samples` as a 16-bit mono WAV file.
fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

impl Transcript {
    /// The words as caption lines: a line ends at the end of a sentence,
    /// at a pause longer than `pause` seconds, or before the word that
    /// would take it past `max_chars`. Without word timings, the provider's
    /// own segments.
    pub fn lines(&self, max_chars: usize, pause: f64) -> Vec<TranscriptSegment> {
        if self.words.is_empty() {
            return self
                .segments
                .iter()
                .filter(|segment| !segment.text.is_empty())
                .cloned()
                .collect();
        }
        let mut lines: Vec<TranscriptSegment> = Vec::new();
        let mut open: Option<TranscriptSegment> = None;
        for word in self.words.iter().filter(|word| !word.word.is_empty()) {
            if let Some(line) = open.take() {
                let longer = line.text.chars().count() + 1 + word.word.chars().count();
                let ended = line.text.ends_with(['.', '?', '!', '。', '？', '！']);
                if ended || longer > max_chars || word.start - line.end > pause {
                    lines.push(line);
                } else {
                    open = Some(TranscriptSegment {
                        end: word.end,
                        text: format!("{} {}", line.text, word.word),
                        ..line
                    });
                    continue;
                }
            }
            open = Some(TranscriptSegment {
                start: word.start,
                end: word.end,
                text: word.word.clone(),
            });
        }
        lines.extend(open);
        lines
    }
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
    fn a_short_recording_is_one_piece_and_a_long_one_is_cut_in_a_pause() {
        let rate = 100;
        assert_eq!(pieces(&vec![0.5; 30 * rate as usize], rate), vec![0..3000]);
        // Twenty-five minutes of sound with one silent tenth of a second
        // eight seconds before the ten-minute mark.
        let mut samples = vec![0.5; 1500 * rate as usize];
        let quiet = (592 * rate) as usize;
        samples[quiet..quiet + 10].fill(0.0);
        let cuts = pieces(&samples, rate);
        assert_eq!(cuts.len(), 3, "{cuts:?}");
        assert_eq!(cuts[0], 0..quiet + 5);
        assert!(cuts.windows(2).all(|pair| pair[0].end == pair[1].start));
        assert_eq!(cuts.last().unwrap().end, samples.len());
        assert!(cuts.iter().all(|piece| piece.len() <= 600 * rate as usize));
    }

    #[test]
    fn a_wav_has_its_header_and_one_sample_in_two_bytes() {
        let bytes = wav(&[0.0, 1.0, -1.0, 2.0], 16_000);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            16_000
        );
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 8);
        assert_eq!(bytes.len(), 44 + 8);
        assert_eq!(i16::from_le_bytes([bytes[46], bytes[47]]), i16::MAX);
        assert_eq!(
            i16::from_le_bytes([bytes[50], bytes[51]]),
            i16::MAX,
            "clamped"
        );
    }

    #[test]
    fn samples_are_sent_as_wav_and_come_back_on_their_own_clock() {
        let (base, seen) = serve_once(200, &[], VERBOSE);
        let endpoint = Endpoint::new(&base, Some("k".into()), "whisper-large-v3-turbo");
        let mut told = Vec::new();
        let transcript = transcribe_samples(
            &endpoint,
            &[0.1; 1600],
            16_000,
            None,
            &AtomicBool::new(false),
            |done, all| told.push((done, all)),
        )
        .unwrap();
        assert_eq!(told, vec![(0, 1), (1, 1)]);
        assert_eq!(transcript.words.len(), 5);
        assert_eq!(transcript.duration, Some(0.1));
        let body = String::from_utf8_lossy(&seen.recv().unwrap().body).into_owned();
        assert!(body.contains("filename=\"voice.wav\"\r\nContent-Type: audio/wav"));
    }

    #[test]
    fn no_sound_is_refused_before_anything_is_sent() {
        let endpoint = Endpoint::new("http://127.0.0.1:9/v1", None, "m");
        let error = transcribe_samples(
            &endpoint,
            &[],
            16_000,
            None,
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(error, AiError::File(_)), "{error:?}");
    }

    fn word(word: &str, start: f64, end: f64) -> TranscriptWord {
        TranscriptWord {
            word: word.into(),
            start,
            end,
        }
    }

    #[test]
    fn words_become_lines_at_sentences_pauses_and_length() {
        let transcript = Transcript {
            words: vec![
                word("Bonjour", 0.0, 0.5),
                word("à", 0.6, 0.7),
                word("tous.", 0.7, 1.0),
                word("Aujourd'hui", 1.2, 1.8),
                word("on", 1.8, 1.9),
                // A long pause: a new line.
                word("monte", 3.5, 3.8),
                word("une", 3.8, 3.9),
                word("vidéo", 3.9, 4.3),
                word("assez", 4.3, 4.6),
                word("longue", 4.6, 5.0),
                word("pour", 5.0, 5.2),
                word("dépasser", 5.2, 5.7),
                word("la", 5.7, 5.8),
                word("ligne", 5.8, 6.1),
            ],
            ..Transcript::default()
        };
        let lines = transcript.lines(32, 0.8);
        let text: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(
            text,
            [
                "Bonjour à tous.",
                "Aujourd'hui on",
                "monte une vidéo assez longue",
                "pour dépasser la ligne",
            ]
        );
        assert_eq!((lines[0].start, lines[0].end), (0.0, 1.0));
        assert_eq!((lines[2].start, lines[2].end), (3.5, 5.0));
        assert!(lines.iter().all(|line| line.text.chars().count() <= 32));
    }

    #[test]
    fn without_word_timings_the_lines_are_the_segments() {
        let transcript = transcript_from(VERBOSE_NO_WORDS);
        let lines = transcript.lines(42, 0.8);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hi there.");
    }

    const VERBOSE_NO_WORDS: &str =
        r#"{"text": "Hi there.", "segments": [{"start": 0.5, "end": 1.5, "text": " Hi there."}]}"#;

    fn transcript_from(json: &str) -> Transcript {
        transcript(&serde_json::from_str(json).unwrap()).unwrap()
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
