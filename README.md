<div align="center">

<img src="assets/montora_logo_512.png" alt="Montora" width="120" />

# Montora

**A free, open-source video editor for Windows, macOS, Linux and Android, with AI that runs on your machine or with the provider you choose.**

**🇬🇧 English** · [🇫🇷 Français](docs/README.fr.md)

</div>

> [!NOTE]
> Montora is a fork of [Concat](https://github.com/jub0t/concat) by Jareer and the Concat contributors. It keeps Concat's engine and adds new editing features and AI tools. It is in early development, and there is no Montora release yet.

## What it is

Montora is a video editor for the things people usually open CapCut for: short videos, YouTube cuts, tutorials, podcast clips. Its engine, inherited from Concat, is native Rust with a GPU compositor.

From Concat it already has:

- **Auto-captions** with a local Whisper model.
- **Text-to-speech and voice cloning**, all local.
- **Background removal**: people, objects, or a mask you paint yourself.
- **Keyframes** with a curve editor: position, scale, rotation, opacity, volume, effect parameters.
- **170+ effects, filters, transitions and text animations**, rendered on the GPU.
- **A multi-track timeline**: split, trim, ripple, freeze frame, speed, blend modes.
- **Export** to H.264, HEVC and AV1, up to 4K 60 with 10-bit colour.
- **An API** over JSON-RPC, gRPC and MCP, so scripts and AI agents can edit too.
- **No watermark, no account, no subscription.** Your footage never leaves your disk unless you choose a cloud AI provider.

## Where it is going

1. **AI providers, your choice.** Local models first. Then optionally Ollama or LM Studio, any OpenAI-compatible API (OpenAI, Mistral, Groq, OpenRouter…) or Anthropic. You bring your own key and choose your own budget.
2. **Edit by text.** Delete a sentence from the transcript and the video is cut to match. Remove silences and filler words in one step.
3. **Subtitles in and out**: SRT import and export.
4. **Highlights**: turn a long recording into short clips.
5. **Later**: an editing assistant, automatic 9:16 reframing, dubbing, and search in your footage.

## Platforms

Windows, macOS (Apple silicon and Intel), Linux and Android. Releases will be published on [GitHub Releases](https://github.com/kdev1966/montora/releases).

Montora is distributed outside the app stores. On macOS, open it the first time with right-click › **Open**. On an Intel Mac, background removal, image enhancement and the Chatterbox voice need macOS 13.3 or later.

## Building from source

You need Rust (rustup installs the pinned version), the FFmpeg 7+ development libraries, cmake and a C++ compiler. On macOS:

```sh
brew install ffmpeg cmake
cd src
FFMPEG_DIR="$(brew --prefix ffmpeg)" cargo dev
```

[CONTRIBUTING.md](docs/CONTRIBUTING.md) covers Windows, Linux and the checks to run. [ARCHITECTURE.md](docs/ARCHITECTURE.md) is a map of the engine. It was written for Concat and is still accurate for Montora.

## License and credits

Montora is free software under the [GNU AGPL v3 or later](LICENSE), like Concat, with the same [plugin exception](LICENSE-EXCEPTIONS.md). Third-party components are listed in [THIRD_PARTY_NOTICES.md](docs/THIRD_PARTY_NOTICES.md).

"Concat" and its logo belong to Jareer (see [TRADEMARK.md](docs/TRADEMARK.md)). Montora is not affiliated with or endorsed by the Concat project.
