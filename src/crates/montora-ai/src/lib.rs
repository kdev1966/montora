// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Montora contributors

//! The AI services Montora's text and transcription tasks run on.
//!
//! Every provider here speaks the OpenAI-compatible HTTP protocol: Groq,
//! OpenCode Zen, xAI, OpenAI itself, and a server on the user's own machine
//! (Ollama, LM Studio, llama.cpp's server). One client covers them all; a
//! [`Provider`] is a preset that knows where each one lives and what it
//! offers, and an [`Endpoint`] is a provider with the user's key and choice
//! of model, ready to call.
//!
//! - [`chat`] sends a conversation and returns the reply: the summaries,
//!   highlight picks and clean-ups the editing features ask for.
//! - [`models`] lists what an endpoint serves, so the settings offer real
//!   model names rather than a list that goes stale.
//! - [`transcribe`] sends an audio file and returns its words, each with
//!   its own start and end.
//!
//! Everything blocks: a task runs on a worker thread like every other long
//! job in the engine, and checks its cancel flag between the calls it makes.
//!
//! Keys are the user's and stay on the machine, in the system's credential
//! store; see [`secrets`].

mod chat;
mod http;
mod provider;
pub mod secrets;
mod transcribe;

pub use chat::{ChatReply, ChatRequest, Message, Role, chat, is_chat_model, models};
pub use http::AiError;
pub use provider::{Endpoint, Provider};
pub use transcribe::{MAX_UPLOAD_BYTES, Transcript, TranscriptSegment, TranscriptWord, transcribe};
