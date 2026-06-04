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
history already stored on your machine, normalizes it into one local index, and
gives you a fast way to search, filter, read, and resume past work.

Orbit v0.1 is **macOS-first for release builds**. Local Linux development
support is available for Claude Code, Codex, Cursor, and OpenCode. Linux
release packages are not published or fully tested yet.

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

## Use Orbit

1. Open Orbit.
2. Click the refresh button in the lower-left corner to index local sessions.
3. Search or filter the session list.
4. Select a session to read its transcript.
5. Use **Resume** when the source agent supports it.

Orbit detects installed agents automatically. It never creates or modifies
their source session files.

## Supported agents

| Agent | macOS discovery | Linux local dev discovery | Transcript parsing | Resume |
| --- | --- | --- | --- | --- |
| Claude Code | Yes | Yes | Yes | Yes |
| Codex | Yes | Yes | Yes | Yes |
| GitHub Copilot CLI | Yes | Not yet | Yes | Yes on macOS |
| Cursor | Yes | Yes | Yes | Opens project |
| OpenCode | Yes | Yes | Yes | Yes |
| Warp | Yes | Not yet | Yes | Not yet |
| Qoder | Yes | Not yet | Yes | Not yet |

Transcript parsing and resume apply on platforms where discovery is supported.

Linux local dev support means the app can be built and run from source on a
Linux desktop with Tauri prerequisites installed. Published Linux release
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

The local database uses the platform data directory. On macOS it usually lives
at:

```text
~/Library/Application Support/orbit/orbit.db
```

On Linux it usually lives under:

```text
~/.local/share/orbit/orbit.db
```

Deleting that database only removes Orbit's index. Your original agent sessions
remain untouched and can be indexed again.

## v0.1 limitations

- macOS is the only release-tested platform.
- Linux is supported for local development with Claude Code, Codex, Cursor, and
  OpenCode discovery.
- App bundles are not signed or notarized yet.
- Session formats can change when agent vendors update their tools.
- Orbit refreshes sessions on manual reindex; live file watching is not enabled
  yet.
- Resume behavior depends on the source agent and your installed terminal.

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
