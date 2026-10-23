// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Words from whisper tokens.
//!
//! whisper.cpp decodes in tokens, which are pieces of words, and gives each
//! one its own timestamps and probability. This turns a segment's tokens into
//! the words a person would read: a token that begins with a space starts a
//! new word, the pieces after it join it, and a lone punctuation mark sticks
//! to the word before it. Each word carries the times of its first and last
//! token and the mean probability of its tokens.
//!
//! Pure: no model and no audio, so it is tested with hand-written tokens.

use serde::{Deserialize, Serialize};

/// One token as whisper.cpp reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// The token's text, as decoded (a leading space starts a new word).
    pub text: String,
    /// Start, in centiseconds from the start of the window.
    pub t0: i64,
    /// End, in centiseconds from the start of the window.
    pub t1: i64,
    /// The model's probability for the token, 0 to 1.
    pub p: f32,
    /// A special token (start, end, language, timestamp): never part of a word.
    pub special: bool,
}

/// A word heard, in seconds from the start of the window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Word {
    /// Where it starts.
    pub start: f64,
    /// Where it ends.
    pub end: f64,
    /// The word, trimmed, with any punctuation that followed it.
    pub text: String,
    /// The mean probability of its tokens, 0 to 1.
    pub confidence: f32,
}

/// A word being put together.
struct Building {
    text: String,
    t0: i64,
    t1: i64,
    p_sum: f32,
    count: u32,
}

impl Building {
    fn new(token: &Token, text: &str) -> Self {
        let mut building = Building {
            text: String::new(),
            t0: token.t0,
            t1: token.t1,
            p_sum: 0.0,
            count: 0,
        };
        building.add(token, text);
        building
    }

    fn add(&mut self, token: &Token, text: &str) {
        self.text.push_str(text);
        self.t1 = token.t1;
        self.p_sum += if token.p.is_finite() { token.p } else { 0.0 };
        self.count += 1;
    }

    fn finish(self, segment: (f64, f64)) -> Option<Word> {
        let text = self.text.trim().to_owned();
        if text.is_empty() {
            return None;
        }
        let start = clamp_time(self.t0, segment.0, segment);
        let end = clamp_time(self.t1, segment.1, segment).max(start);
        let confidence = (self.p_sum / self.count.max(1) as f32).clamp(0.0, 1.0);
        Some(Word {
            start,
            end,
            text,
            confidence,
        })
    }
}

/// Centiseconds to seconds inside the segment; a negative time, which
/// whisper does not give but a bad window could, falls back to `fallback`.
fn clamp_time(centiseconds: i64, fallback: f64, segment: (f64, f64)) -> f64 {
    let seconds = if centiseconds < 0 {
        fallback
    } else {
        centiseconds as f64 / 100.0
    };
    let seconds = if seconds.is_finite() {
        seconds
    } else {
        fallback
    };
    seconds.max(segment.0).min(segment.1)
}

/// The words of one segment, from its tokens in order. `segment` is the segment's
/// (start, end) in seconds: every word is kept inside it.
pub fn words_from_tokens(tokens: &[Token], segment: (f64, f64)) -> Vec<Word> {
    let mut words = Vec::new();
    let mut current: Option<Building> = None;

    for token in tokens {
        let text = token.text.trim();
        if token.special || text.is_empty() {
            continue;
        }
        let punctuation_only = !text.chars().any(char::is_alphanumeric);
        if punctuation_only {
            // Sticks to the word before it; with none, it is dropped.
            if let Some(word) = current.as_mut() {
                word.add(token, text);
            }
            continue;
        }
        let starts_word = token.text.starts_with(char::is_whitespace);
        match current.as_mut() {
            Some(word) if !starts_word => word.add(token, text),
            _ => {
                if let Some(done) = current.take() {
                    words.extend(done.finish(segment));
                }
                current = Some(Building::new(token, text));
            }
        }
    }
    if let Some(done) = current {
        words.extend(done.finish(segment));
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str, t0: i64, t1: i64, p: f32) -> Token {
        Token {
            text: text.to_owned(),
            t0,
            t1,
            p,
            special: false,
        }
    }

    fn special(text: &str, t0: i64, t1: i64) -> Token {
        Token {
            special: true,
            ..token(text, t0, t1, 1.0)
        }
    }

    const WIDE: (f64, f64) = (0.0, 100.0);

    #[test]
    fn two_spaced_tokens_are_two_words_with_their_times() {
        let words = words_from_tokens(
            &[token(" Hello", 10, 40, 0.9), token(" world", 50, 90, 0.7)],
            WIDE,
        );
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello");
        assert_eq!((words[0].start, words[0].end), (0.1, 0.4));
        assert_eq!(words[1].text, "world");
        assert_eq!((words[1].start, words[1].end), (0.5, 0.9));
    }

    #[test]
    fn pieces_without_a_space_join_into_one_word() {
        let words = words_from_tokens(
            &[
                token(" un", 100, 120, 0.9),
                token("believ", 120, 160, 0.6),
                token("able", 160, 210, 0.9),
            ],
            WIDE,
        );
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "unbelievable");
        assert_eq!((words[0].start, words[0].end), (1.0, 2.1));
        assert!((words[0].confidence - 0.8).abs() < 1e-6);
    }

    #[test]
    fn punctuation_sticks_to_the_word_before() {
        let words = words_from_tokens(
            &[
                token(" Bonjour", 0, 50, 0.9),
                token(",", 50, 55, 0.9),
                token(" ça", 60, 80, 0.9),
                token(" va", 80, 100, 0.9),
                token("?", 100, 110, 0.9),
            ],
            WIDE,
        );
        let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["Bonjour,", "ça", "va?"]);
        assert_eq!(words[0].end, 0.55);
        assert_eq!(words[2].end, 1.1);
    }

    #[test]
    fn spaced_punctuation_does_not_start_a_word() {
        let words = words_from_tokens(
            &[token(" Hi", 0, 10, 0.9), token(" ...", 10, 20, 0.9)],
            WIDE,
        );
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "Hi...");
    }

    #[test]
    fn special_tokens_neither_split_nor_join() {
        let words = words_from_tokens(
            &[
                special("[_BEG_]", 0, 0),
                token(" un", 0, 10, 0.9),
                special("[_TT_50]", 10, 10),
                token("able", 10, 20, 0.9),
                special("[_EOT_]", 20, 20),
            ],
            WIDE,
        );
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "unable");
    }

    #[test]
    fn arabic_words_are_split_on_spaces() {
        let words = words_from_tokens(
            &[token(" مرحبا", 0, 60, 0.8), token(" بكم", 60, 120, 0.8)],
            (0.0, 2.0),
        );
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "مرحبا");
        assert_eq!(words[1].text, "بكم");
        assert_eq!((words[1].start, words[1].end), (0.6, 1.2));
    }

    #[test]
    fn times_outside_the_segment_are_clamped() {
        let words = words_from_tokens(
            &[token(" early", 0, 50, 0.9), token(" late", 150, 400, 0.9)],
            (1.0, 3.0),
        );
        assert_eq!((words[0].start, words[0].end), (1.0, 1.0));
        assert_eq!((words[1].start, words[1].end), (1.5, 3.0));
    }

    #[test]
    fn an_end_before_the_start_gives_an_empty_span() {
        let words = words_from_tokens(&[token(" odd", 80, 30, 0.9)], WIDE);
        assert_eq!(words[0].start, 0.8);
        assert_eq!(words[0].end, words[0].start);
    }

    #[test]
    fn negative_times_fall_back_to_the_segment() {
        let words = words_from_tokens(&[token(" odd", -5, -1, 0.9)], (2.0, 4.0));
        assert_eq!((words[0].start, words[0].end), (2.0, 4.0));
    }

    #[test]
    fn leading_punctuation_with_no_word_is_dropped() {
        let words = words_from_tokens(&[token(" ¿", 0, 5, 0.9), token("Qué", 5, 20, 0.9)], WIDE);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "Qué");
        assert_eq!(words[0].start, 0.05);
    }

    #[test]
    fn no_tokens_give_no_words() {
        assert!(words_from_tokens(&[], WIDE).is_empty());
    }

    #[test]
    fn blank_tokens_are_skipped() {
        let words = words_from_tokens(&[token(" ", 0, 5, 0.9), token("", 5, 6, 0.9)], WIDE);
        assert!(words.is_empty());
    }

    #[test]
    fn a_nan_probability_counts_as_zero_and_does_not_panic() {
        let words = words_from_tokens(
            &[token(" a", 0, 10, f32::NAN), token("b", 10, 20, 0.8)],
            WIDE,
        );
        assert_eq!(words.len(), 1);
        assert!((words[0].confidence - 0.4).abs() < 1e-6);
    }

    #[test]
    fn confidence_is_kept_between_zero_and_one() {
        let words = words_from_tokens(&[token(" hi", 0, 10, 3.0)], WIDE);
        assert_eq!(words[0].confidence, 1.0);
    }
}
