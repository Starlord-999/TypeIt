# TypeIt

Local-first dictation app: press a hotkey, speak, and the transcript is auto-pasted wherever your cursor is. Transcription runs entirely on-device via [whisper.cpp](https://github.com/ggml-org/whisper.cpp) (or optionally Groq's cloud API).

## Download (no build required)

Grab the latest installer for your OS from [Releases](https://github.com/Starlord-999/TypeIt/releases):

- **macOS**: download the `.dmg`, open it, drag TypeIt to Applications. First launch will warn "unidentified developer" — right-click the app and choose **Open** once to bypass it (Gatekeeper, since this isn't signed with a paid Apple cert).
- **Windows**: download the `.msi` and run it. SmartScreen may warn "Windows protected your PC" — click **More info → Run anyway** (same reason: no paid code-signing cert).

On first use, the app will ask for microphone access. On macOS, auto-paste also needs **Accessibility** permission — grant it in System Settings → Privacy & Security → Accessibility, then reopen the app if it doesn't paste automatically.

## Building from source

Requires [Rust](https://rustup.rs), [Node.js](https://nodejs.org), and a C++ toolchain (Xcode Command Line Tools on macOS, Visual Studio Build Tools on Windows) since the local transcription engine is compiled from source.

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

# Windows (PowerShell):
# Copy-Item "C:\temp\whisper.cpp\build\bin\Release\whisper-cli.exe" src-tauri\binaries\whisper-cpp-x86_64-pc-windows-msvc.exe

npm run tauri dev    # run in dev mode
npm run tauri build  # produce an installer in src-tauri/target/release/bundle/
```

## How it works

- **Hotkey**: configurable in Settings → Recording ("Record Shortcut" button). Requires an app restart to take effect.
- **Engine**: Local (whisper.cpp, on-device, private) or Cloud (Groq API, needs your own API key).
- **Language**: auto-detect, or pin a specific language if auto-detect misfires on short/mixed-language audio.
- **Recording mode**: Toggle (tap to start, tap again to stop) or Push-to-Talk (hold to record, release to stop and transcribe).
