# Siri-like TUI Voice Assistant

A highly functional, completely local, Siri-like TUI (Terminal User Interface) application built with Rust. It features real-time voice interaction, AI-powered conversations, news monitoring, and system integrations.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/language-Rust-orange.svg)

## ✨ Features

- **Circular "Uni" Visualizer**: A dynamic, frame-based visualizer that changes color and motion based on the assistant's state (Thinking, Speaking, Listening).
- **Multi-Model AI**: Seamlessly switch between local LLMs (e.g., `qwen2.5:3b`, `phi3:mini`) using Ollama.
- **Natural Voice**: High-quality Japanese voice synthesis via VOICEVOX (defaulting to Meimei Himari).
- **Voice & Text Input**: Talk using Space-to-Record (via Vosk) or type messages directly.
- **Smart News Monitor**: Aggregates RSS feeds from NHK and Yahoo News. Proactively alerts you to emergency news and summarizes headlines on request.
- **System Integrations**: 
  - **Thunderbird**: Real-time monitoring for new email arrivals.
  - **Music Integration**: Voice-command support to launch local music TUI players.
- **Session Management**: Automatically saves chat history and allows switching between past conversations.
- **Fully Local & Private**: No cloud required. All processing happens on your machine.

## 🛠️ Prerequisites

- **Ollama**: For running LLMs. `ollama serve` should be accessible.
- **VOICEVOX Engine**: The headless engine version should be placed in `~/.voicevox_extracted/vv-engine/`.
- **Vosk Japanese Model**: Place the model folder as `vosk-model-jp` in the project root.
- **libvosk.so**: Ensure the Vosk shared library is installed in your system path (e.g., `/usr/local/lib`).

## 🚀 Getting Started

1. **Clone the repository**
   ```bash
   git clone https://github.com/yourusername/voiceassistant.git
   cd voiceassistant
   ```

2. **Setup Dependencies**
   Install system libraries (Arch Linux example):
   ```bash
   sudo pacman -S alsa-lib openssl
   ```

3. **Run the application**
   ```bash
   cargo run --release
   ```

## ⌨️ Controls

| Key | Action |
|-----|--------|
| **Space** | Start voice recording (3 seconds) |
| **Enter** | Send text input |
| **Tab** | Toggle AI Models |
| **S** | Switch Voice (Himari / Zundamon / Metan) |
| **N** | Start a New session |
| **L** | List and Load past sessions |
| **Q** | Quit |

## 📁 Project Structure

- `src/main.rs`: Core TUI logic and state management.
- `src/voicevox.rs`: API client for voice synthesis.
- `src/vosk_engine.rs`: Offline speech-to-text engine.
- `src/audio.rs`: Low-level audio recording and playback.

## 📜 License

MIT License. See `LICENSE` for details.
