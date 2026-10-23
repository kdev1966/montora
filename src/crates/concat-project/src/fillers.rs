// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Hesitations ("euh", "mmm") and verbal tics ("du coup", "يعني") found in the
//! timed words of a transcript, so the editor can cut them.
//!
//! Pure: words and two lists in, the stretches of time they occupy out. Words
//! are compared in a folded form (case, Latin accents, Arabic diacritics and
//! punctuation ignored), a hesitation may be stretched ("euhhh"), and an entry
//! of several words matches across consecutive words.

use std::cmp::Reverse;

use crate::model::TranscriptWord;

/// Hesitations Montora knows out of the box. Never carry meaning.
pub const DEFAULT_HESITATIONS: &[&str] = &[
    "euh", "heu", "hum", "hm", "mm", "eh", "ah", "um", "uh", "er", "اه", "ام", "مم", "إيه",
];

/// Verbal tics Montora knows out of the box. Usually filler, sometimes meaning.
pub const DEFAULT_TICS: &[&str] = &[
    "du coup",
    "en fait",
    "genre",
    "voilà",
    "bah",
    "ben",
    "like",
    "you know",
    "basically",
    "يعني",
    "زعمة",
    "فاهم",
    "تعرف",
    "بالحق",
    "شنوة",
    "اسمع",
];

/// One filler found: where it is, in the words' seconds, and what it matched.
#[derive(Clone, Debug, PartialEq)]
pub struct FillerHit {
    /// Seconds from the start: the first matched word's start.
    pub start: f64,
    /// Seconds from the start: the last matched word's end.
    pub end: f64,
    /// The list entry, as written in the list.
    pub phrase: String,
    /// True for a tic, false for a hesitation.
    pub tic: bool,
}

/// `text` as fillers are compared: lower case, Latin accents folded
/// (é è ê ë → e, à â ä → a, î ï → i, ô ö → o, ù û ü → u, ç → c), Arabic
/// diacritics and tatweel removed, every character that is neither
/// alphanumeric nor whitespace removed, whitespace collapsed to single spaces
/// and trimmed.
pub fn normalize(text: &str) -> String {
    let folded: String = text
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|c| !matches!(*c, '\u{064B}'..='\u{065F}' | '\u{0670}' | '\u{0640}'))
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'à' | 'â' | 'ä' => 'a',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `token` with each run of the same character reduced to one.
fn collapse_runs(token: &str) -> String {
    let mut out = String::with_capacity(token.len());
    let mut last = None;
    for c in token.chars() {
        if last != Some(c) {
            out.push(c);
        }
        last = Some(c);
    }
    out
}

/// A list entry ready to match: its tokens, as written, and its kind.
struct Candidate<'a> {
    tokens: Vec<String>,
    phrase: &'a str,
    tic: bool,
}

fn candidates<'a>(list: &'a [String], tic: bool, out: &mut Vec<Candidate<'a>>) {
    for entry in list {
        let tokens: Vec<String> = normalize(entry)
            .split(' ')
            .filter(|token| !token.is_empty())
            .map(|token| {
                if tic {
                    token.to_owned()
                } else {
                    collapse_runs(token)
                }
            })
            .collect();
        if !tokens.is_empty() {
            out.push(Candidate {
                tokens,
                phrase: entry,
                tic,
            });
        }
    }
}

/// A word whose times can be trusted.
fn sane(word: &TranscriptWord) -> bool {
    word.start.is_finite() && word.end.is_finite() && word.end >= word.start
}

/// The fillers in `words` (in time order): hesitations and tics from both
/// lists.
pub fn find_fillers(
    words: &[TranscriptWord],
    hesitations: &[String],
    tics: &[String],
) -> Vec<FillerHit> {
    let mut all = Vec::new();
    candidates(hesitations, false, &mut all);
    candidates(tics, true, &mut all);
    // Longest first; at the same length a hesitation beats a tic. The sort is
    // stable, so list order settles the rest.
    all.sort_by_key(|c| (Reverse(c.tokens.len()), c.tic));

    // (word, plain form, stretched-collapsed form), skipping what cannot match.
    let usable: Vec<(&TranscriptWord, String, String)> = words
        .iter()
        .filter(|word| sane(word))
        .filter_map(|word| {
            let plain = normalize(&word.text);
            (!plain.is_empty()).then(|| {
                let collapsed = collapse_runs(&plain);
                (word, plain, collapsed)
            })
        })
        .collect();

    let mut hits = Vec::new();
    let mut at = 0;
    while at < usable.len() {
        let found = all.iter().find(|candidate| {
            let end = at + candidate.tokens.len();
            end <= usable.len()
                && usable[at..end].iter().zip(&candidate.tokens).all(
                    |((_, plain, collapsed), token)| {
                        if candidate.tic {
                            plain == token
                        } else {
                            collapsed == token
                        }
                    },
                )
        });
        match found {
            Some(candidate) => {
                let last = at + candidate.tokens.len() - 1;
                hits.push(FillerHit {
                    start: usable[at].0.start,
                    end: usable[last].0.end,
                    phrase: candidate.phrase.to_owned(),
                    tic: candidate.tic,
                });
                at = last + 1;
            }
            None => at += 1,
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(start: f64, end: f64, text: &str) -> TranscriptWord {
        TranscriptWord {
            start,
            end,
            text: text.to_owned(),
            confidence: None,
        }
    }

    /// Words one second long, back to back, from these texts.
    fn speech(texts: &[&str]) -> Vec<TranscriptWord> {
        texts
            .iter()
            .enumerate()
            .map(|(i, text)| word(i as f64, i as f64 + 1.0, text))
            .collect()
    }

    fn defaults() -> (Vec<String>, Vec<String>) {
        let own = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect();
        (own(DEFAULT_HESITATIONS), own(DEFAULT_TICS))
    }

    fn find(texts: &[&str]) -> Vec<FillerHit> {
        let (hesitations, tics) = defaults();
        find_fillers(&speech(texts), &hesitations, &tics)
    }

    #[test]
    fn normalize_folds_accents_and_strips_punctuation_and_arabic_diacritics() {
        assert_eq!(normalize("Euh,"), "euh");
        assert_eq!(normalize("  Voilà !  "), "voila");
        assert_eq!(normalize("ÉLÈVE çà ôtez"), "eleve ca otez");
        assert_eq!(normalize("يَعْنِي"), "يعني");
        assert_eq!(normalize("يعـــني"), "يعني");
        assert_eq!(normalize("du   coup..."), "du coup");
        assert_eq!(normalize("?!"), "");
    }

    #[test]
    fn stretched_hesitations_match_the_hesitation_list() {
        let hits = find(&["bonjour", "euhhh", "Mmmm", "ممم"]);
        let phrases: Vec<_> = hits.iter().map(|h| (h.phrase.as_str(), h.tic)).collect();
        assert_eq!(phrases, [("euh", false), ("mm", false), ("مم", false)]);
        assert_eq!((hits[0].start, hits[0].end), (1.0, 2.0));
    }

    #[test]
    fn a_tic_of_two_words_matches_across_them_and_its_second_word_alone_does_not() {
        let hits = find(&["bon", "du", "coup", "voila"]);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].phrase, "du coup");
        assert!(hits[0].tic);
        assert_eq!((hits[0].start, hits[0].end), (1.0, 3.0));
        assert_eq!(hits[1].phrase, "voilà");
        assert!(find(&["un", "coup", "de", "main"]).is_empty());
    }

    #[test]
    fn an_arabic_tic_with_punctuation_attached_matches() {
        let hits = find(&["«يَعْنِي»،", "هذا"]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].phrase, "يعني");
        assert!(hits[0].tic);
    }

    #[test]
    fn an_entry_the_user_added_matches_like_the_defaults() {
        let hits = find_fillers(&speech(&["قلت", "خاطر,", "جاء"]), &[], &["خاطر".to_owned()]);
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].phrase.as_str(), hits[0].tic), ("خاطر", true));
        assert_eq!((hits[0].start, hits[0].end), (1.0, 2.0));
    }

    #[test]
    fn overlapping_candidates_give_one_hit_with_the_longest_entry() {
        let tics = vec!["you".to_owned(), "you know".to_owned(), "know".to_owned()];
        let hits = find_fillers(&speech(&["you", "know"]), &[], &tics);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].phrase, "you know");
        assert_eq!((hits[0].start, hits[0].end), (0.0, 2.0));
    }

    #[test]
    fn a_hesitation_beats_a_tic_that_matches_the_same_span() {
        let hesitations = vec!["euh".to_owned()];
        let tics = vec!["euh".to_owned()];
        let hits = find_fillers(&speech(&["euh"]), &hesitations, &tics);
        assert_eq!(hits.len(), 1);
        assert!(!hits[0].tic);
    }

    #[test]
    fn nothing_to_look_in_or_look_for_gives_no_hits() {
        let (hesitations, tics) = defaults();
        assert!(find_fillers(&[], &hesitations, &tics).is_empty());
        assert!(find_fillers(&speech(&["euh", "du", "coup"]), &[], &[]).is_empty());
        let blank = vec![String::new(), " ?! ".to_owned()];
        assert!(find_fillers(&speech(&["euh"]), &blank, &blank).is_empty());
    }

    #[test]
    fn a_word_of_only_punctuation_does_not_break_a_phrase() {
        let hits = find(&["du", "...", "coup"]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].phrase, "du coup");
        assert_eq!((hits[0].start, hits[0].end), (0.0, 3.0));
    }

    #[test]
    fn stretching_applies_to_hesitations_only() {
        assert!(find(&["genree"]).is_empty());
        assert!(find(&["coupp"]).is_empty());
    }

    #[test]
    fn words_with_odd_times_are_skipped_and_nothing_panics() {
        let (hesitations, tics) = defaults();
        let words = vec![
            word(f64::NAN, 1.0, "euh"),
            word(0.0, f64::INFINITY, "euh"),
            word(5.0, 4.0, "euh"),
            word(6.0, 7.0, "euh"),
        ];
        let hits = find_fillers(&words, &hesitations, &tics);
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].start, hits[0].end), (6.0, 7.0));
    }
}
