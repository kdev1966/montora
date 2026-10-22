// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! The captions sheet: the tray's Captions tool, as a form and then as a
//! progress report.
//!
//! One switch says what it captions: "Use voice from timeline", on, takes
//! the words from a clip with sound, chosen in the sheet from the clips on
//! the timeline that have any; off, from a script typed in it. It opens on
//! when a clip with sound is selected, or when a clip's menu opened it on
//! that clip, and off otherwise. Either lands as one batch of title clips,
//! one undo step.
//! The transcriber runs on a worker and reports as
//! [`CaptionsMsg::Progress`], ending as [`CaptionsMsg::Finished`].
//!
//! The model list is the installed whisper models and, after them, the
//! transcriber of the provider chosen in Settings > AI when it has one
//! (Groq, OpenAI): the clip's sound is read here as for whisper, sent in
//! pieces, and the words that come back, each with its own timing, are cut
//! into caption lines the way a script is.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use concat_project::Command;
use concat_project::model::{TextStyle, Transcript, TranscriptSegment, TranscriptWord};
use concat_speech::JobStage;
use concat_speech::transcribe::Segment;

use crate::host::{on_ui_in_project, spawn_in_project};
use crate::i18n::{t, tf};
use crate::panes::Msg;
use crate::panes::ai_settings::error_text;
use crate::panes::pace::{Pace, Stage, Timings};
use crate::panes::settings::installed;
use crate::studio::Studio;
use crate::ui::CaptionsSheetData;

/// Where a caption sits, by the sheet's row: a frame-height fraction from
/// the centre, positive down. Bottom, centre, top.
const CAPTION_OFFSETS: [f64; 3] = [0.35, 0.0, -0.35];
/// A caption's cap height by the sheet's row, as a fraction of the frame.
const CAPTION_SIZES: [f64; 3] = [0.04, 0.05, 0.065];
/// A rough speaking rate, for a script's timing and the speech sheet's
/// estimate.
pub const CHARS_PER_SECOND: f32 = 14.0;

/// Everything that can happen to the captions sheet.
#[derive(Clone, Debug)]
pub enum CaptionsMsg {
    /// The tray's Captions tool: the selected clip's voice when it has
    /// sound, the script otherwise.
    Open,
    /// A clip's menu: the sheet on that clip's voice.
    OpenOnClip(String),
    Close,
    /// The "Use voice from timeline" switch.
    VoiceToggled(bool),
    /// Row in the list of clips with sound.
    SourceChanged(i32),
    TextEdited(String),
    ModelChanged(i32),
    PlacementChanged(i32),
    SizeChanged(i32),
    /// Run the pass the sheet describes.
    Begin,
    Cancel,
    /// The transcriber's worker reporting the stage it is in.
    Progress(JobStage),
    /// The transcriber's worker is done: what was said, or why not.
    Finished(Result<Heard, String>),
}

/// What a run heard, in seconds from the start of the window it read.
#[derive(Clone, Debug, Default)]
pub struct Heard {
    /// The caption lines to place, already cut to a readable length.
    pub lines: Vec<Segment>,
    /// The phrases as the transcriber gave them, kept in the project.
    pub segments: Vec<Segment>,
    /// The single words with their times, when the transcriber gave them.
    pub words: Vec<TranscriptWord>,
    /// The language heard, when the transcriber said.
    pub language: String,
}

impl Heard {
    /// Lines and phrases are the same: whisper's segments are both.
    fn of_segments(segments: Vec<Segment>) -> Heard {
        Heard {
            lines: segments.clone(),
            segments,
            ..Heard::default()
        }
    }
}

/// The clip a transcription is running over, as the finished segments
/// need it: where it starts on the timeline and how fast it plays.
#[derive(Clone, Debug)]
struct Subject {
    start: f64,
    speed: f64,
    /// The file the clip plays, and the stretch of it that was read:
    /// where the transcript is kept, in the file's own seconds.
    media_id: String,
    audio_stream: Option<u32>,
    source_start: f64,
    window: f64,
    /// What heard it, as the transcript names it ("whisper:base").
    source: String,
    /// The look chosen when the run began.
    look: (f64, f64),
    /// The transcriber model, and the thirty-second windows whisper reads
    /// the sound in: what the run's timing is kept by.
    model: String,
    windows: f32,
}

/// Seconds whisper takes over one thirty-second window, per megabyte of
/// model, before a run on the model has been timed: about a second for
/// base and ten for medium on eight cores of a recent CPU.
const WINDOW_SECONDS_PER_MB: f32 = 1.0 / 160.0;

/// The captions sheet's state.
#[derive(Default)]
pub struct CaptionsPane {
    pub open: bool,
    /// "Use voice from timeline": the words come from [`Self::clip`]'s sound
    /// when on, from [`Self::text`] when off.
    pub from_voice: bool,
    /// The clip whose voice is transcribed: one with sound, chosen from the
    /// timeline's. Kept while the switch is off, so turning it back on
    /// returns to the same clip.
    pub clip: Option<String>,
    /// The script's words.
    pub text: String,
    /// Row in the installed transcriber list.
    pub model: usize,
    /// 0 bottom, 1 centre, 2 top.
    pub placement: usize,
    /// 0 small, 1 medium, 2 large.
    pub size: usize,
    pub running: bool,
    /// How far the running transcription looks.
    pub pace: Pace,
    /// Why the last run failed, when it did.
    pub message: String,
    /// The clip under the running transcription.
    subject: Option<Subject>,
    /// How long each model took this session, for the next run's bar.
    timings: Timings,
    /// The running online transcription's stop flag, read between pieces.
    online_cancel: Arc<AtomicBool>,
}

/// The transcriber the provider chosen in Settings > AI offers, when it
/// offers one: the provider, and the model it transcribes with.
pub fn online_transcriber(studio: &Studio) -> Option<(montora_ai::Provider, &'static str)> {
    let provider = studio.prefs.ai.provider()?;
    Some((provider, provider.transcription_model()?))
}

/// The sheet's model list: the installed whisper models, then the
/// provider's transcriber, named as online.
pub fn model_names(studio: &Studio) -> Vec<slint::SharedString> {
    installed(&studio.settings.transcribers)
        .iter()
        .map(|model| model.name.as_str().into())
        .chain(
            online_transcriber(studio)
                .map(|(provider, _)| tf("captions.onlineModel", &[&provider.label()]).into()),
        )
        .collect()
}

/// How long a caption line from the online transcriber runs at most, in
/// characters: the script's limit.
const ONLINE_LINE_CHARS: usize = CAPTION_CHARS;
/// A pause longer than this, in seconds, starts a new caption line.
const ONLINE_LINE_PAUSE: f64 = 0.8;

impl CaptionsPane {
    /// Applies one message. The studio is the rest of the window; while
    /// this runs the studio's copy of the pane is a blank it must not read.
    pub fn update(&mut self, msg: CaptionsMsg, studio: &mut Studio) {
        match msg {
            CaptionsMsg::Open => {
                let selected = studio.sole_selection();
                self.open_on(studio, selected, false);
            }
            CaptionsMsg::OpenOnClip(id) => self.open_on(studio, Some(id), true),
            CaptionsMsg::Close => self.open = false,
            CaptionsMsg::VoiceToggled(on) => {
                self.from_voice = on;
                self.message.clear();
                let sources = sources(studio);
                if on
                    && !self
                        .clip
                        .as_ref()
                        .is_some_and(|id| sources.iter().any(|s| &s.id == id))
                {
                    self.clip = sources.first().map(|source| source.id.clone());
                }
            }
            CaptionsMsg::SourceChanged(index) => {
                if let Some(source) = usize::try_from(index)
                    .ok()
                    .and_then(|index| sources(studio).into_iter().nth(index))
                {
                    self.clip = Some(source.id);
                    self.message.clear();
                }
            }
            CaptionsMsg::TextEdited(text) => self.text = text,
            CaptionsMsg::ModelChanged(index) => self.model = index.max(0) as usize,
            CaptionsMsg::PlacementChanged(index) => {
                self.placement = (index.max(0) as usize).min(2);
            }
            CaptionsMsg::SizeChanged(index) => self.size = (index.max(0) as usize).min(2),
            CaptionsMsg::Begin => {
                if self.from_voice {
                    self.run_sound(studio);
                } else {
                    self.run_script(studio);
                }
            }
            CaptionsMsg::Cancel => {
                studio.host.transcriber.cancel();
                self.online_cancel.store(true, Ordering::Relaxed);
                self.running = false;
                self.open = false;
            }
            CaptionsMsg::Progress(stage) => self.pace.report(stage),
            CaptionsMsg::Finished(result) => {
                self.running = false;
                let subject = self.subject.take();
                if let (Ok(_), Some(subject)) = (&result, &subject) {
                    self.timings
                        .keep(&subject.model, self.pace.learned(subject.windows));
                }
                match result {
                    Ok(heard) => {
                        let Some(subject) = subject else {
                            return;
                        };
                        let look = subject.look;
                        let base = studio.prefs.title_style.clone();
                        let kept = kept_transcript(&subject, &heard);
                        let commands: Vec<Command> = heard
                            .lines
                            .into_iter()
                            .filter_map(|segment| {
                                let text = segment.text.trim().to_owned();
                                (!text.is_empty()).then(|| {
                                    caption_clip(
                                        text,
                                        subject.start + segment.start / subject.speed,
                                        ((segment.end - segment.start) / subject.speed).max(0.2),
                                        look,
                                        base.as_ref(),
                                    )
                                })
                            })
                            .collect();
                        let count = commands.len();
                        self.open = false;
                        // What was heard is kept with the file, in the same
                        // undo step as the captions made from it, so a
                        // second pass over the clip need not listen again.
                        let mut batch = vec![kept];
                        batch.extend(commands);
                        studio.apply(Command::Batch { commands: batch });
                        if count == 0 {
                            studio.notify(&t("captions.nothingSaidClip"), true);
                        } else {
                            studio.notify(&tf("captions.addedCaptions", &[&count]), false);
                        }
                    }
                    // Asked for: the sheet is already on its way down.
                    Err(error) if error.contains("cancel") => self.open = false,
                    Err(error) => self.message = error,
                }
            }
        }
    }

    /// The sheet opened afresh. `clip` is the clip it was opened on, if
    /// any: the switch starts on when that clip has sound, or when a clip's
    /// menu asked for its voice (`voice`), in which case a clip without
    /// sound falls to the first clip that has some.
    fn open_on(&mut self, studio: &Studio, clip: Option<String>, voice: bool) {
        let installed = installed(&studio.settings.transcribers);
        // The active whisper model; the online transcriber, listed after
        // the installed ones, when no whisper model is installed.
        let model = installed.iter().position(|model| model.active).unwrap_or(0);
        let sources = sources(studio);
        let sounding = clip.filter(|id| sources.iter().any(|source| &source.id == id));
        let from_voice = voice || sounding.is_some();
        let clip = sounding.or_else(|| {
            from_voice
                .then(|| sources.first().map(|source| source.id.clone()))
                .flatten()
        });
        *self = CaptionsPane {
            open: true,
            from_voice,
            clip,
            model,
            placement: 0,
            size: 1,
            timings: std::mem::take(&mut self.timings),
            ..CaptionsPane::default()
        };
    }

    /// A caption's look, by the sheet's rows: where it sits and its size.
    fn look(&self) -> (f64, f64) {
        (
            CAPTION_OFFSETS[self.placement.min(2)],
            CAPTION_SIZES[self.size.min(2)],
        )
    }

    /// The script as titles, one after another from the playhead.
    fn run_script(&mut self, studio: &mut Studio) {
        let lines = script_captions(&self.text);
        if lines.is_empty() {
            self.message = t("captions.nothingToCaptionYet");
            return;
        }
        let look = self.look();
        let mut at = f64::from(studio.playhead);
        let base = studio.prefs.title_style.clone();
        let commands: Vec<Command> = lines
            .into_iter()
            .map(|(text, seconds)| {
                let command = caption_clip(text, at, seconds, look, base.as_ref());
                at += seconds;
                command
            })
            .collect();
        let count = commands.len();
        self.open = false;
        studio.apply(Command::Batch { commands });
        studio.notify(&tf("captions.addedCaptions", &[&count]), false);
    }

    /// The sheet's clip through the transcriber on a worker, reporting into
    /// the sheet as it goes.
    fn run_sound(&mut self, studio: &mut Studio) {
        let Some(clip) = self.clip.as_ref().and_then(|id| studio.clip(id)).cloned() else {
            self.message = t("captions.clipNoLongerTimeline");
            return;
        };
        let Some(media) = studio.project().media_by_id(&clip.media_id).cloned() else {
            self.message = t("captions.clipHasNoFile");
            return;
        };
        let local = installed(&studio.settings.transcribers).len();
        if self.model == local
            && let Some(online) = online_transcriber(studio)
        {
            self.run_online(studio, &clip, &media.path, online);
            return;
        }
        let Some((model, megabytes)) = installed(&studio.settings.transcribers)
            .get(self.model)
            .map(|model| (model.id.clone(), model.megabytes))
        else {
            self.message = t("captions.noTranscriber");
            return;
        };
        let request = concat_speech::transcribe::TranscribeRequest {
            path: media.path.clone(),
            audio_stream: clip.audio_stream,
            source_start: clip.source_start,
            window: clip.duration * clip.speed,
            model_id: model.clone(),
        };
        let dirs = studio.host.dirs.clone();
        let transcriber = Arc::clone(&studio.host.transcriber);
        // The run as the bar lays it out: the sound read in at a few
        // hundred times its length, the model loaded unless it still is,
        // and whisper a window at a time - at the pace the last run on
        // this model measured, or the estimate by its size.
        let windows = (request.window / 30.0).ceil().max(1.0) as f32;
        let learned = self.timings.get(&model);
        let loaded = studio.host.transcriber.loaded_model().as_deref() == Some(model.as_str());
        let load = if loaded {
            0.0
        } else {
            learned.load.unwrap_or(0.5 + megabytes / 400.0)
        };
        let per_window = learned
            .per_unit
            .unwrap_or(0.3 + megabytes * WINDOW_SECONDS_PER_MB);
        self.pace = Pace::new(vec![
            (Stage::Reading, 0.2 + request.window as f32 / 400.0),
            (Stage::Loading, load),
            (Stage::Working, windows * per_window),
        ]);
        self.subject = Some(Subject {
            start: clip.start,
            speed: clip.speed,
            media_id: clip.media_id.clone(),
            audio_stream: clip.audio_stream,
            source_start: clip.source_start,
            window: request.window,
            source: format!("whisper:{model}"),
            look: self.look(),
            model,
            windows,
        });
        self.running = true;
        self.message.clear();
        let epoch = crate::host::project_epoch();
        spawn_in_project(
            move || {
                transcriber
                    .transcribe(&dirs, &request, move |stage| {
                        on_ui_in_project(epoch, move |studio, _, _| {
                            studio.handle(Msg::Captions(CaptionsMsg::Progress(stage)));
                        });
                    })
                    .map(Heard::of_segments)
            },
            |studio, _, _, result| studio.handle(Msg::Captions(CaptionsMsg::Finished(result))),
        );
    }

    /// The sheet's clip through the provider's transcriber on a worker: its
    /// sound read as whisper reads it, sent in pieces, and the timed words
    /// cut into caption lines. The key is read from the store on the
    /// worker.
    fn run_online(
        &mut self,
        studio: &mut Studio,
        clip: &concat_project::model::Clip,
        path: &str,
        (provider, model): (montora_ai::Provider, &'static str),
    ) {
        let window = clip.duration * clip.speed;
        if window.is_nan() || window <= 0.0 {
            self.message = t("captions.nothingSaidClip");
            return;
        }
        let label = format!("{}:{model}", provider.id());
        // Ten-minute pieces; each one's upload and transcription at the
        // pace the last online run measured, or a guess.
        let pieces = (window / 600.0).ceil().max(1.0) as f32;
        let per_piece = self
            .timings
            .get(&label)
            .per_unit
            .unwrap_or(4.0 + (window as f32 / pieces) / 40.0);
        self.pace = Pace::new(vec![
            (Stage::Reading, 0.2 + window as f32 / 400.0),
            (Stage::Working, pieces * per_piece),
        ]);
        self.subject = Some(Subject {
            start: clip.start,
            speed: clip.speed,
            media_id: clip.media_id.clone(),
            audio_stream: clip.audio_stream,
            source_start: clip.source_start,
            window,
            source: label.clone(),
            look: self.look(),
            model: label,
            windows: pieces,
        });
        self.running = true;
        self.message.clear();
        let cancel = Arc::new(AtomicBool::new(false));
        self.online_cancel = Arc::clone(&cancel);
        let options = concat_media::AudioOptions {
            start: Some(clip.source_start),
            duration: Some(window),
            filters: Vec::new(),
            rate: 16_000,
            channels: 1,
            format: concat_media::SampleFormat::F32,
            stream: clip.audio_stream.map(|index| index as usize),
        };
        let path = path.to_owned();
        let config = studio.host.dirs.config.clone();
        let custom_url = studio.prefs.ai.custom_url.clone();
        let epoch = crate::host::project_epoch();
        spawn_in_project(
            move || {
                let report = move |stage: JobStage| {
                    on_ui_in_project(epoch, move |studio, _, _| {
                        studio.handle(Msg::Captions(CaptionsMsg::Progress(stage)));
                    });
                };
                report(JobStage::Reading);
                let samples = concat_media::AudioDecoder::open(&path, &options)
                    .and_then(|mut decoder| decoder.collect_f32())
                    .map_err(|error| error.to_string())?;
                if samples.is_empty() {
                    return Err(t("captions.nothingSaidClip"));
                }
                let key = montora_ai::secrets::Secrets::new(&config).api_key(provider)?;
                let endpoint = provider
                    .endpoint(&custom_url, key, model)
                    .map_err(|error| error_text(&error))?;
                let transcript = montora_ai::transcribe_samples(
                    &endpoint,
                    &samples,
                    16_000,
                    None,
                    &cancel,
                    |done, all| {
                        let all = all.max(1) as f32;
                        report(JobStage::Working {
                            done: done as f32 / all,
                            next: ((done + 1) as f32 / all).min(1.0),
                        });
                    },
                )
                .map_err(|error| match error {
                    // Said the way the whisper path says it, so the sheet
                    // closes quietly on a cancel.
                    montora_ai::AiError::Cancelled => "transcription cancelled".to_owned(),
                    other => error_text(&other),
                })?;
                let segment = |start: f64, end: f64, text: String| Segment { start, end, text };
                Ok(Heard {
                    lines: transcript
                        .lines(ONLINE_LINE_CHARS, ONLINE_LINE_PAUSE)
                        .into_iter()
                        .map(|line| segment(line.start, line.end, line.text))
                        .collect(),
                    segments: transcript
                        .segments
                        .iter()
                        .map(|phrase| segment(phrase.start, phrase.end, phrase.text.clone()))
                        .collect(),
                    words: transcript
                        .words
                        .iter()
                        .map(|word| TranscriptWord {
                            start: word.start,
                            end: word.end,
                            text: word.word.clone(),
                            confidence: None,
                        })
                        .collect(),
                    language: transcript.language.clone().unwrap_or_default(),
                })
            },
            |studio, _, _, result| studio.handle(Msg::Captions(CaptionsMsg::Finished(result))),
        );
    }

    /// The sheet as Slint shows it.
    pub fn data(&self, studio: &Studio) -> CaptionsSheetData {
        let sources = if self.open {
            sources(studio)
        } else {
            Vec::new()
        };
        let source = self
            .clip
            .as_ref()
            .and_then(|id| sources.iter().position(|source| &source.id == id))
            .map_or(-1, |index| index as i32);
        let labels: Vec<slint::SharedString> = sources
            .into_iter()
            .map(|source| source.label.into())
            .collect();
        CaptionsSheetData {
            open: self.open,
            from_voice: self.from_voice,
            sources: std::rc::Rc::new(slint::VecModel::from(labels)).into(),
            source,
            text: self.text.as_str().into(),
            model: self.model as i32,
            placement: self.placement as i32,
            size: self.size as i32,
            running: self.running,
            progress: self.pace.floor,
            ceiling: self.pace.ceiling,
            step: self.pace.step,
            left: self.pace.left,
            stage: match self.pace.stage() {
                Some(Stage::Loading) => t("captions.loadingTheModel"),
                Some(Stage::Working) => t("captions.transcribing"),
                _ => t("captions.readingTheSound"),
            }
            .into(),
            ready: !installed(&studio.settings.transcribers).is_empty()
                || online_transcriber(studio).is_some(),
            message: self.message.as_str().into(),
        }
    }
}

/// A clip on the timeline whose voice can be captioned, as the sheet's
/// list shows it.
struct Source {
    id: String,
    label: String,
}

/// The clips on the current timeline that have sound, in the order they
/// play: an audio clip, or a video whose file has an audio stream.
/// The command that keeps what a run heard with the clip's file: its
/// phrases and words moved from the window's seconds to the file's, over
/// the stretch of the file the run read.
fn kept_transcript(subject: &Subject, heard: &Heard) -> Command {
    let at = |seconds: f64| subject.source_start + seconds;
    Command::SetTranscript {
        media_id: subject.media_id.clone(),
        transcript: Transcript {
            audio_stream: subject.audio_stream,
            language: heard.language.clone(),
            source: subject.source.clone(),
            spans: vec![[subject.source_start, at(subject.window)]],
            segments: heard
                .segments
                .iter()
                .filter(|segment| !segment.text.trim().is_empty())
                .map(|segment| TranscriptSegment {
                    start: at(segment.start),
                    end: at(segment.end),
                    text: segment.text.trim().to_owned(),
                })
                .collect(),
            words: heard
                .words
                .iter()
                .map(|word| TranscriptWord {
                    start: at(word.start),
                    end: at(word.end),
                    ..word.clone()
                })
                .collect(),
        },
    }
}

fn sources(studio: &Studio) -> Vec<Source> {
    let project = studio.project();
    let mut clips: Vec<_> = studio
        .timeline()
        .clips
        .iter()
        .filter(|clip| studio.clip_has_sound(clip))
        .map(|clip| {
            let name = project
                .media_by_id(&clip.media_id)
                .map_or(clip.name.as_str(), |media| media.name.as_str());
            (clip.start, clip.id.clone(), name.to_owned())
        })
        .collect();
    clips.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    clips
        .into_iter()
        .map(|(start, id, name)| Source {
            id,
            label: source_label(&name, start),
        })
        .collect()
}

/// A clip as the list names it: its file and where it starts on the
/// timeline, so two clips of one file are told apart.
fn source_label(name: &str, start: f64) -> String {
    let seconds = start.max(0.0).floor() as u64;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{name} · {hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{name} · {minutes}:{seconds:02}")
    }
}

/// One caption as a title clip: its words, when and for how long, and
/// its look.
fn caption_clip(
    text: String,
    start: f64,
    duration: f64,
    look: (f64, f64),
    base: Option<&TextStyle>,
) -> Command {
    let (offset_y, font_size) = look;
    // The kept look where there is one - family, weight, colours, stroke,
    // shadow, background - at the caption's own size and place; the
    // bundled face otherwise.
    let style = match base {
        Some(base) => TextStyle {
            content: text,
            font_size,
            max_width: 0.0,
            max_height: 0.0,
            ..base.clone()
        },
        None => TextStyle {
            content: text,
            font_family: "Hanken Grotesk".to_owned(),
            font_size,
            font_weight: 600.0,
            ..TextStyle::default()
        },
    };
    Command::AddTextClip {
        track_id: None,
        above: true,
        start,
        style: Some(style),
        duration: Some(duration),
        offset_y: Some(offset_y),
    }
}

/// Longest a caption line gets before it is wrapped: about what two lines
/// of broadcast subtitle hold, and what a reader takes in at a glance.
const CAPTION_CHARS: usize = 42;

/// A script as caption lines, each with how long it stays up: a line's
/// reading time at [`CHARS_PER_SECOND`], held to one second at least so
/// a short word is not a flicker, and seven at most so a long line does
/// not hang. A line break in the script is a break the author asked for;
/// within a paragraph a sentence is a caption, and a long sentence wraps
/// at its words.
fn script_captions(text: &str) -> Vec<(String, f64)> {
    text.lines()
        .flat_map(sentences)
        .flat_map(|sentence| wrap_caption(&sentence))
        .map(|line| {
            let seconds =
                (line.chars().count() as f64 / f64::from(CHARS_PER_SECOND)).clamp(1.0, 7.0);
            (line, seconds)
        })
        .collect()
}

/// A paragraph's sentences. A full stop, question or exclamation mark ends
/// one when it is followed by space or by the end - so "3.5" and "e.g." hold
/// together - and the CJK marks end one on their own.
fn sentences(paragraph: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = paragraph.chars().peekable();
    while let Some(ch) = chars.next() {
        current.push(ch);
        let ends = match ch {
            '。' | '！' | '？' => true,
            '.' | '!' | '?' => chars.peek().is_none_or(|next| next.is_whitespace()),
            _ => false,
        };
        if ends {
            let sentence = current.trim();
            if !sentence.is_empty() {
                out.push(sentence.to_owned());
            }
            current.clear();
        }
    }
    let rest = current.trim();
    if !rest.is_empty() {
        out.push(rest.to_owned());
    }
    out
}

/// A sentence in lines of at most [`CAPTION_CHARS`], broken between words;
/// a word longer than a line, or a run of CJK with no spaces, is broken
/// where it must be.
fn wrap_caption(sentence: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_chars = 0;
    for word in sentence.split_whitespace() {
        let word_chars = word.chars().count();
        if line_chars > 0 && line_chars + 1 + word_chars > CAPTION_CHARS {
            lines.push(std::mem::take(&mut line));
            line_chars = 0;
        }
        if word_chars > CAPTION_CHARS {
            let mut piece = String::new();
            for ch in word.chars() {
                piece.push(ch);
                if piece.chars().count() == CAPTION_CHARS {
                    lines.push(std::mem::take(&mut piece));
                }
            }
            line = piece;
            line_chars = line.chars().count();
            continue;
        }
        if line_chars > 0 {
            line.push(' ');
            line_chars += 1;
        }
        line.push_str(word);
        line_chars += word_chars;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{
        Command, Heard, Segment, Subject, TranscriptWord, kept_transcript, script_captions,
        source_label,
    };

    /// What a run heard is moved from the window's seconds to the file's,
    /// over the stretch of the file it read; a blank phrase is not kept.
    #[test]
    fn what_a_run_heard_is_kept_in_the_files_own_seconds() {
        let subject = Subject {
            start: 3.0,
            speed: 2.0,
            media_id: "m1".to_owned(),
            audio_stream: Some(1),
            source_start: 10.0,
            window: 5.0,
            source: "whisper:base".to_owned(),
            look: (0.0, 0.0),
            model: "base".to_owned(),
            windows: 1.0,
        };
        let heard = Heard {
            lines: Vec::new(),
            segments: vec![
                Segment {
                    start: 1.0,
                    end: 2.0,
                    text: " la perle ".to_owned(),
                },
                Segment {
                    start: 2.5,
                    end: 3.0,
                    text: "  ".to_owned(),
                },
            ],
            words: vec![TranscriptWord {
                start: 1.0,
                end: 1.4,
                text: "la".to_owned(),
                confidence: None,
            }],
            language: "fr".to_owned(),
        };
        let Command::SetTranscript {
            media_id,
            transcript,
        } = kept_transcript(&subject, &heard)
        else {
            panic!("keeps a transcript");
        };
        assert_eq!(media_id, "m1");
        assert_eq!(transcript.audio_stream, Some(1));
        assert_eq!(
            transcript.spans,
            vec![[10.0, 15.0]],
            "the stretch of the file read"
        );
        assert_eq!(transcript.segments.len(), 1, "a blank phrase is not kept");
        assert_eq!(transcript.segments[0].text, "la perle");
        assert_eq!(
            (transcript.segments[0].start, transcript.segments[0].end),
            (11.0, 12.0)
        );
        assert_eq!(
            (transcript.words[0].start, transcript.words[0].end),
            (11.0, 11.4)
        );
        assert_eq!(transcript.source, "whisper:base");
    }

    /// A clip in the voice list is its file and where it starts, in
    /// minutes and seconds, with hours once there are any.
    #[test]
    fn a_voice_is_named_by_its_file_and_where_it_starts() {
        assert_eq!(source_label("Interview.mp4", 0.0), "Interview.mp4 · 0:00");
        assert_eq!(source_label("Interview.mp4", 72.9), "Interview.mp4 · 1:12");
        assert_eq!(source_label("Talk.wav", 3_725.0), "Talk.wav · 1:02:05");
        assert_eq!(source_label("Talk.wav", -1.0), "Talk.wav · 0:00");
    }

    /// A script becomes one caption per sentence, a hand line break is
    /// kept, a long sentence wraps at its words, and each line is held for
    /// its reading time within one to seven seconds.
    #[test]
    fn a_script_is_cut_into_readable_lines() {
        let lines = script_captions(
            "Hello there. This is version 3.5, mind!\n\nA sentence that runs on for far \
             longer than a caption line has any business running on for. Ok?",
        );
        let text: Vec<&str> = lines.iter().map(|(line, _)| line.as_str()).collect();
        assert_eq!(
            text,
            [
                "Hello there.",
                "This is version 3.5, mind!",
                "A sentence that runs on for far longer",
                "than a caption line has any business",
                "running on for.",
                "Ok?",
            ]
        );
        assert!(
            lines
                .iter()
                .all(|(_, seconds)| (1.0..=7.0).contains(seconds))
        );
        assert_eq!(lines[0].1, 1.0);
        assert!(lines[2].1 > lines[0].1);
        assert!(script_captions("  \n ").is_empty());
        assert_eq!(script_captions("你好。再见！").len(), 2);
    }
}
