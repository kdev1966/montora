// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Silence finding: the quiet stretches of a recording worth cutting out.
//!
//! The samples are measured in short windows, a window is silent when its
//! loudness sits under a threshold, and a run of silent windows is a silence.
//! Only the runs long enough to matter are kept, and each is pulled in from
//! both ends so a little room is left around the speech on either side. The
//! work is pure: samples in, seconds out, nothing read from or written to disk.

/// What counts as silence, and how much of it to leave around speech.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SilenceSettings {
    /// Loudness under which a stretch is silent, in dBFS (e.g. -35.0).
    pub threshold_db: f32,
    /// The shortest silence worth cutting, in seconds (e.g. 0.6).
    pub min_silence: f64,
    /// Sound kept on each side of a cut, in seconds (e.g. 0.15).
    pub padding: f64,
    /// Length of the windows loudness is measured over, in seconds (e.g. 0.02).
    pub window: f64,
}

impl Default for SilenceSettings {
    fn default() -> Self {
        Self {
            threshold_db: -35.0,
            min_silence: 0.6,
            padding: 0.15,
            window: 0.02,
        }
    }
}

/// The stretches of `samples` (mono, `rate` per second) to cut, as
/// (start, end) seconds from the first sample, in order, not overlapping.
///
/// A silence is a run of windows all quieter than the threshold; runs shorter
/// than the minimum are left alone. Each kept run is shrunk by the padding on
/// both sides, except at the very start or end of the samples, where there is
/// no speech to leave room for. A run that shrinks to nothing is dropped.
/// Samples that are not finite count as zero. Input that cannot be measured -
/// no samples, a rate of zero, a window that is not positive, or settings that
/// are not finite - gives no silences.
pub fn find_silences(samples: &[f32], rate: u32, settings: &SilenceSettings) -> Vec<(f64, f64)> {
    let usable = settings.threshold_db.is_finite()
        && settings.min_silence.is_finite()
        && settings.padding.is_finite()
        && settings.window.is_finite()
        && settings.window > 0.0;
    if samples.is_empty() || rate == 0 || !usable {
        return Vec::new();
    }

    let rate_hz = f64::from(rate);
    let window_len = ((settings.window * rate_hz).round() as usize).clamp(1, samples.len());
    let window_count = samples.len().div_ceil(window_len);
    let threshold = f64::from(settings.threshold_db);
    let padding = settings.padding.max(0.0);

    // The seconds at which window `index` starts; the one past the last
    // window is the end of the samples, not a whole window further.
    let seconds_at = |index: usize| (index * window_len).min(samples.len()) as f64 / rate_hz;

    let mut silences = Vec::new();
    let mut run_start: Option<usize> = None;
    for index in 0..=window_count {
        let silent = index < window_count && {
            let from = index * window_len;
            let to = (from + window_len).min(samples.len());
            loudness_db(&samples[from..to]) < threshold
        };
        match (silent, run_start) {
            (true, None) => run_start = Some(index),
            (false, Some(first)) => {
                run_start = None;
                if let Some(cut) = trim(
                    seconds_at(first),
                    seconds_at(index),
                    first == 0,
                    index == window_count,
                    settings.min_silence,
                    padding,
                ) {
                    silences.push(cut);
                }
            }
            _ => {}
        }
    }
    silences
}

/// The loudness of a window in dBFS: 20 * log10 of its RMS, with samples that
/// are not finite counted as zero. A window of zeroes is minus infinity.
fn loudness_db(window: &[f32]) -> f64 {
    let sum: f64 = window
        .iter()
        .map(|sample| {
            if sample.is_finite() {
                f64::from(*sample) * f64::from(*sample)
            } else {
                0.0
            }
        })
        .sum();
    let rms = (sum / window.len() as f64).sqrt();
    20.0 * rms.log10()
}

/// A silent run from `start` to `end` seconds as the cut to make: `None` when
/// it is shorter than `min_silence` or shrinks away, otherwise pulled in by
/// `padding` on each side that does not touch the start or end of the samples.
fn trim(
    start: f64,
    end: f64,
    at_head: bool,
    at_tail: bool,
    min_silence: f64,
    padding: f64,
) -> Option<(f64, f64)> {
    if end - start < min_silence {
        return None;
    }
    let from = if at_head { start } else { start + padding };
    let to = if at_tail { end } else { end - padding };
    (to - from > 0.0).then_some((from, to))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 16_000;

    fn tone(seconds: f64) -> Vec<f32> {
        let count = (seconds * f64::from(RATE)) as usize;
        (0..count)
            .map(|i| {
                let t = i as f64 / f64::from(RATE);
                (0.5 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()) as f32
            })
            .collect()
    }

    fn quiet(seconds: f64) -> Vec<f32> {
        vec![0.0; (seconds * f64::from(RATE)) as usize]
    }

    fn join(parts: &[Vec<f32>]) -> Vec<f32> {
        parts.concat()
    }

    fn near(found: (f64, f64), expected: (f64, f64)) {
        assert!(
            (found.0 - expected.0).abs() < 0.03 && (found.1 - expected.1).abs() < 0.03,
            "{found:?} is not near {expected:?}"
        );
    }

    #[test]
    fn nothing_to_measure_gives_nothing() {
        let settings = SilenceSettings::default();
        assert!(find_silences(&[], RATE, &settings).is_empty());
        assert!(find_silences(&quiet(2.0), 0, &settings).is_empty());
    }

    #[test]
    fn settings_that_cannot_be_measured_give_nothing() {
        let samples = quiet(2.0);
        let base = SilenceSettings::default();
        for settings in [
            SilenceSettings {
                window: 0.0,
                ..base
            },
            SilenceSettings {
                window: -0.02,
                ..base
            },
            SilenceSettings {
                window: f64::NAN,
                ..base
            },
            SilenceSettings {
                threshold_db: f32::NAN,
                ..base
            },
            SilenceSettings {
                min_silence: f64::INFINITY,
                ..base
            },
            SilenceSettings {
                padding: f64::NAN,
                ..base
            },
        ] {
            assert!(find_silences(&samples, RATE, &settings).is_empty());
        }
    }

    #[test]
    fn two_seconds_of_silence_is_cut_whole() {
        let found = find_silences(&quiet(2.0), RATE, &SilenceSettings::default());
        assert_eq!(found, vec![(0.0, 2.0)]);
    }

    #[test]
    fn a_silence_between_tones_is_cut_short_of_the_padding() {
        let samples = join(&[tone(1.0), quiet(1.0), tone(1.0)]);
        let found = find_silences(&samples, RATE, &SilenceSettings::default());
        assert_eq!(found.len(), 1, "{found:?}");
        near(found[0], (1.15, 1.85));
    }

    #[test]
    fn a_short_silence_between_tones_is_left_alone() {
        let samples = join(&[tone(1.0), quiet(0.3), tone(1.0)]);
        assert!(find_silences(&samples, RATE, &SilenceSettings::default()).is_empty());
    }

    #[test]
    fn a_silence_at_the_head_keeps_its_start() {
        let samples = join(&[quiet(1.0), tone(1.0)]);
        let found = find_silences(&samples, RATE, &SilenceSettings::default());
        assert_eq!(found.len(), 1, "{found:?}");
        near(found[0], (0.0, 0.85));
    }

    #[test]
    fn a_silence_at_the_tail_keeps_its_end() {
        let samples = join(&[tone(1.0), quiet(1.0)]);
        let found = find_silences(&samples, RATE, &SilenceSettings::default());
        assert_eq!(found.len(), 1, "{found:?}");
        near(found[0], (1.15, 2.0));
    }

    #[test]
    fn a_silence_that_padding_eats_entirely_is_dropped() {
        let samples = join(&[tone(1.0), quiet(0.6), tone(1.0)]);
        let settings = SilenceSettings {
            min_silence: 0.5,
            padding: 0.4,
            ..SilenceSettings::default()
        };
        assert!(find_silences(&samples, RATE, &settings).is_empty());
    }

    #[test]
    fn samples_that_are_not_finite_count_as_silent() {
        let samples = vec![f32::NAN; 2 * RATE as usize];
        let found = find_silences(&samples, RATE, &SilenceSettings::default());
        assert_eq!(found, vec![(0.0, 2.0)]);

        // One second at 4 samples per second: a window per sample.
        let mixed = vec![f32::INFINITY, f32::NEG_INFINITY, f32::NAN, 0.0];
        let found = find_silences(&mixed, 4, &SilenceSettings::default());
        assert_eq!(found, vec![(0.0, 1.0)]);
    }

    #[test]
    fn a_partial_last_window_still_counts() {
        // 1.5 windows of silence at 10 samples per window.
        let settings = SilenceSettings {
            min_silence: 0.0,
            padding: 0.0,
            window: 1.0,
            ..SilenceSettings::default()
        };
        let found = find_silences(&[0.0; 15], 10, &settings);
        assert_eq!(found, vec![(0.0, 1.5)]);
    }

    #[test]
    fn several_silences_come_back_in_order_and_apart() {
        let samples = join(&[
            quiet(0.8),
            tone(0.5),
            quiet(1.0),
            tone(0.5),
            quiet(0.7),
            tone(0.5),
            quiet(1.2),
        ]);
        let found = find_silences(&samples, RATE, &SilenceSettings::default());
        assert_eq!(found.len(), 4, "{found:?}");
        for cut in &found {
            assert!(cut.0 < cut.1, "{cut:?}");
        }
        for pair in found.windows(2) {
            assert!(pair[0].1 <= pair[1].0, "{pair:?}");
        }
        assert_eq!(found[0].0, 0.0);
        assert!((found[3].1 - 5.2).abs() < 1e-9, "{:?}", found[3]);
    }
}
