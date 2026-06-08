<p align="center">
  <img src="public/orbit-mark.svg" width="112" alt="Orbit logo">
</p>

<h1 align="center">Orbit</h1>

<p align="center">
  Your coding-agent history is scattered across tools. Orbit puts it in one place.
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#supported-agents">Supported agents</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

<p align="center">
  <img src="public/orbit-screenshot.png" alt="Orbit session browser">
</p>

Orbit is a native session browser for AI coding agents. It finds the session
history already stored on your computer, normalizes it into one local index, and
gives you a fast way to search, filter, read, and resume past work.

Orbit is **macOS-first** for release builds, with experimental Windows session
discovery and local Linux development support for Claude Code, Codex, Cursor,
and OpenCode. AppImage generation is locally verified on the current Ubuntu
development machine, but broader Linux release support is not claimed yet.

## Why Orbit

Coding agents are useful, but their history is fragmented. A useful debugging
session may be in Claude Code, yesterday's implementation may be in Codex, and
the command you need may be buried in Warp.

Orbit makes that history usable:

- Search session titles, messages, tool calls, inputs, and outputs
- Filter by agent, project, model, branch, or date
- Read long transcripts without loading the entire conversation into the UI
- See active sessions and indexing status
- Resume supported sessions in your preferred terminal
- Keep everything local

Orbit reads local session files and stores its index in a local SQLite
database. It does not upload your transcripts.

## Install

### Download

Download the latest macOS build from
[GitHub Releases](https://github.com/TheDartsCo/orbit/releases).

Orbit is not notarized yet. On first launch, macOS may require you to
right-click the app and choose **Open**.

### Build from source

You need Node.js 18 or newer, Rust, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform.

On Ubuntu/Debian, install Tauri's Linux development dependencies first:

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

```bash
git clone https://github.com/TheDartsCo/orbit.git
cd orbit
npm install
npm run tauri build
```

The packaged app is written to `src-tauri/target/release/bundle/`.

To build only a local Linux AppImage on Ubuntu:

```bash
npm run build:linux:appimage
```

The AppImage is written to:

```text
src-tauri/target/release/bundle/appimage/
```

To run it locally:

```bash
chmod +x src-tauri/target/release/bundle/appimage/*.AppImage
./src-tauri/target/release/bundle/appimage/*.AppImage
```

This AppImage is verified only on the current Ubuntu development machine. A
portable Linux release should be built on an older supported Linux baseline.

## Use Orbit

1. Open Orbit.
2. Click the refresh button in the lower-left corner to index local sessions.
3. Search or filter the session list.
4. Select a session to read its transcript.
5. Use **Resume** when the source agent supports it.

Orbit detects installed agents automatically. It never creates or modifies
their source session files.

## Supported agents

| Agent | Transcript | macOS discovery | macOS resume | Windows discovery | Windows resume | Linux local dev |
| --- | :---: | :---: | --- | :---: | --- | :---: |
| Antigravity | ✅ | ✅ | Not available | 🧪 | 📋 Copy command | Planned |
| Claude Code | ✅ | ✅ | ✅ Launch | 🧪 | 📋 Copy command | ✅ Launch |
| Codex | ✅ | ✅ | ✅ Launch | 🧪 | 📋 Copy command | ✅ Launch |
| Cursor | ✅ | ✅ | Opens project | 🧪 | 📋 Copy command | ✅ Opens project |
| GitHub Copilot CLI | ✅ | ✅ | ✅ Launch | 🧪 | 📋 Copy command | Planned |
| JetBrains AI | ✅ | ✅ | Not available | 🧪 | 📋 Session ID | Planned |
| OpenCode | ✅ | ✅ | ✅ Launch | 🧪 | 📋 Copy command | ✅ Launch |
| Qoder | ✅ | ✅ | Opens Qoder | 🧪 | 📋 Copy command | Planned |
| Warp | ✅ | ✅ | Opens Warp | 🧪 | 📋 Copy command | Planned |

**Legend:** ✅ supported · 🧪 implemented and unit-tested, native Windows
verification pending · 📋 shown in a copyable Windows dialog

On Windows, Orbit discovers and parses local sessions but does not launch
resume commands automatically yet. Clicking **Resume** shows the session ID
and available command so you can copy them.

Linux local dev support means the app can be built and run from source on a
Linux desktop with Tauri prerequisites installed. Local AppImage generation is
available on the current Ubuntu development machine, but published Linux release
packages are still outside the v0.1 support boundary.

Agent storage formats are private implementation details and can change without
notice. If an update breaks an adapter, please open an issue with the agent
version and a sanitized example of the affected session structure.

## How it works

Orbit is a Tauri v2 desktop app with a React frontend and Rust backend.

Each agent has a small Rust adapter responsible for detection, session
discovery, parsing, and resume commands. The indexer normalizes those sessions
into SQLite, skips unchanged files using parser-version and file metadata
hashes, and removes stale entries after every complete scan.

The frontend talks to the backend through Tauri commands. Session and transcript
lists are virtualized so large histories remain responsive.

The local database lives in the platform data directory:

```text
macOS:   ~/Library/Application Support/orbit/orbit.db
Windows: %APPDATA%\orbit\orbit.db
Linux:   ~/.local/share/orbit/orbit.db
```

Deleting that database only removes Orbit's index. Your original agent sessions
remain untouched and can be indexed again.

## Platform status

- macOS is the primary development and release platform.
- Windows adapter discovery is implemented and unit-tested, but still needs
  native Windows build and runtime verification.
- Linux is supported for local development with Claude Code, Codex, Cursor, and
  OpenCode discovery. Local AppImage generation is verified on the current
  Ubuntu development machine only.
- App bundles are not signed or notarized yet.
- Session formats can change when agent vendors update their tools.
- Orbit refreshes sessions on manual reindex; live file watching is not enabled
  yet.
- Automatic resume launching is supported on macOS and Linux where the adapter
  supports resume. Windows uses copyable session details for now.

## Development

```bash
npm install
npm run tauri dev
```

Useful checks:

```bash
npm run build
cd src-tauri && cargo test
```

See [CONTRIBUTING.md](CONTRIBUTING.md) before changing an adapter or submitting
a pull request.

## License

Orbit is available under the [MIT License](LICENSE).
