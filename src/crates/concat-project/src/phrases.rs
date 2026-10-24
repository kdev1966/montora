// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Editing by the text: the phrases of a transcript a person struck out,
//! turned into the stretches of the file to cut.
//!
//! Pure: phrases and which of them go in, `[start, end]` in seconds of the
//! file out, ready for [`crate::commands::Command::RemoveRanges`]. Phrases
//! struck one after another go as one stretch, the pause between them with
//! them, so cutting three sentences in a row leaves no breath behind.

use crate::model::TranscriptSegment;

/// The phrases of `segments` that play within `window` (seconds of the
/// file, what a clip shows), in order: the ones a sheet lists for that clip.
pub fn phrases_in(segments: &[TranscriptSegment], window: (f64, f64)) -> Vec<TranscriptSegment> {
    let mut phrases: Vec<TranscriptSegment> = segments
        .iter()
        .filter(|segment| {
            segment.start.is_finite()
                && segment.end.is_finite()
                && segment.start < segment.end
                && segment.start < window.1
                && window.0 < segment.end
                && !segment.text.trim().is_empty()
        })
        .cloned()
        .collect();
    phrases.sort_by(|a, b| a.start.total_cmp(&b.start));
    phrases
}

/// The stretches to cut for the phrases whose `struck` is true, in order.
/// A run of struck phrases is one stretch, from the first's start to the
/// last's end. `struck` is read beside `phrases`; a phrase without an
/// answer stays.
pub fn struck_ranges(phrases: &[TranscriptSegment], struck: &[bool]) -> Vec<[f64; 2]> {
    let mut ranges: Vec<[f64; 2]> = Vec::new();
    let mut run: Option<[f64; 2]> = None;
    for (i, phrase) in phrases.iter().enumerate() {
        if struck.get(i).copied().unwrap_or(false) {
            run = Some(match run {
                Some([start, end]) => [start, end.max(phrase.end)],
                None => [phrase.start, phrase.end],
            });
        } else if let Some(range) = run.take() {
            ranges.push(range);
        }
    }
    ranges.extend(run);
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phrase(start: f64, end: f64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            start,
            end,
            text: text.to_owned(),
        }
    }

    /// Three phrases with half a second of pause between them.
    fn talk() -> Vec<TranscriptSegment> {
        vec![
            phrase(0.0, 2.0, "Bonjour."),
            phrase(2.5, 4.0, "Euh, attendez."),
            phrase(4.5, 7.0, "On commence."),
        ]
    }

    #[test]
    fn nothing_struck_cuts_nothing() {
        assert!(struck_ranges(&talk(), &[false, false, false]).is_empty());
        assert!(struck_ranges(&talk(), &[]).is_empty());
    }

    #[test]
    fn one_phrase_is_its_own_times() {
        assert_eq!(
            struck_ranges(&talk(), &[false, true, false]),
            vec![[2.5, 4.0]]
        );
    }

    #[test]
    fn struck_neighbours_take_the_pause_between_them() {
        assert_eq!(
            struck_ranges(&talk(), &[true, true, false]),
            vec![[0.0, 4.0]]
        );
        assert_eq!(
            struck_ranges(&talk(), &[true, true, true]),
            vec![[0.0, 7.0]]
        );
    }

    #[test]
    fn a_kept_phrase_splits_the_runs() {
        assert_eq!(
            struck_ranges(&talk(), &[true, false, true]),
            vec![[0.0, 2.0], [4.5, 7.0]]
        );
    }

    #[test]
    fn phrases_in_keeps_what_the_window_shows() {
        let listed = phrases_in(&talk(), (3.0, 5.0));
        let texts: Vec<&str> = listed.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, ["Euh, attendez.", "On commence."]);
    }

    #[test]
    fn phrases_in_drops_empty_and_broken_phrases() {
        let segments = vec![
            phrase(0.0, 1.0, "  "),
            phrase(2.0, 1.0, "backwards"),
            phrase(f64::NAN, 3.0, "nan"),
            phrase(3.0, 4.0, "kept"),
        ];
        let listed = phrases_in(&segments, (0.0, 10.0));
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].text, "kept");
    }
}
