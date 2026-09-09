# TypeIt

Local-first dictation app: press a hotkey, speak, and the transcript is typed wherever your cursor is — live, in chunks, as you talk. Transcription runs entirely on-device via [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (or optionally Groq's cloud API).

Run from source via terminal — there's no packaged installer for this project.

## Setup (one-time)

Requires [Rust](https://rustup.rs), [Node.js](https://nodejs.org), and `cmake` (a C++ toolchain is also needed to build whisper.cpp — Xcode Command Line Tools on macOS, Visual Studio Build Tools on Windows).

```bash
npm install

# Build the whisper.cpp sidecar binary (one-time, ~1-2 min)
git clone --depth 1 https://github.com/ggml-org/whisper.cpp.git /tmp/whisper.cpp
cmake -B /tmp/whisper.cpp/build -S /tmp/whisper.cpp -DBUILD_SHARED_LIBS=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/whisper.cpp/build --config Release --target whisper-cli -j
mkdir -p src-tauri/binaries

# macOS (Apple Silicon):
cp /tmp/whisper.cpp/build/bin/whisper-cli src-tauri/binaries/whisper-cpp-aarch64-apple-darwin
chmod +x src-tauri/binaries/whisper-cpp-aarch64-apple-darwin

# Windows (PowerShell), build output lands under build/bin/Release instead:
# Copy-Item "C:\temp\whisper.cpp\build\bin\Release\whisper-cli.exe" src-tauri\binaries\whisper-cpp-x86_64-pc-windows-msvc.exe
```

## Running

```bash
npm run tauri dev
```

This builds and launches the app with live logs printed to your terminal. Quit with `Ctrl+C` or by closing the app window.

If you see "port 1420 already in use" (a leftover session didn't shut down), free it first:

```bash
lsof -ti:1420 | xargs kill -9
```

### First-run permissions (macOS)

The app will ask for **microphone** access on first use. Auto-paste also needs **Accessibility** permission — grant it in System Settings → Privacy & Security → Accessibility (find the `typeit` binary or your terminal app in the list), then relaunch if it doesn't paste automatically.

## How it works

- **Hotkey**: configurable in Settings → Recording ("Record Shortcut" button). Requires an app restart to take effect.
- **Recording mode**: Toggle (tap to start, tap again to stop) or Push-to-Talk (hold to record, release to stop).
- **Live typing**: while recording, the app transcribes what's been said roughly every 4 seconds and types it out immediately, instead of waiting for the whole utterance. Each chunk is a lightly-cleaned fragment (no forced capitalization/punctuation mid-sentence); the final chunk on stop gets full cleanup.
- **Engine**: Local (whisper.cpp, on-device, private) or Cloud (Groq API, needs your own API key).
- **Language**: auto-detect, or pin a specific language if auto-detect misfires on short/mixed-language audio.
- **Overlay**: a small mic icon (top-right) shows Ready/Recording/Transcribing, visible even over full-screen apps.
