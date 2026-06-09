# Contributing to Orbit

Orbit is early software built around file formats controlled by other tools.
The most useful contributions are focused, testable, and based on session data
you have verified locally.

## Before you start

Open an issue before taking on a broad feature, a new platform, or a large UI
change. Small bug fixes and adapter improvements can go straight to a pull
request when the problem and expected behavior are clear.

Never include private transcripts, API keys, access tokens, user names, or
local paths in issues, fixtures, screenshots, or commits. Reduce session
fixtures to the smallest sanitized example that still reproduces the behavior.

## Local setup

Orbit v0.1 release builds are tested on macOS. Local Linux development is
supported for Claude Code, Codex, Cursor, and OpenCode adapters.

You need:

- Node.js 18 or newer
- Rust and Cargo
- The [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/)

On Ubuntu/Debian, install Tauri's Linux development dependencies:

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
npm run tauri dev
```

`npm run dev` starts only the Vite frontend. Most useful behavior requires the
full Tauri app because data access happens through Rust commands.

## Project shape

- `src/` contains the React and TypeScript frontend.
- `src-tauri/src/` contains Tauri commands, adapters, indexing, and SQLite
  access.
- `src-tauri/src/adapters/` contains one adapter per supported agent.
- `src/types/index.ts` mirrors shared Rust models and agent metadata.

## Changing an adapter

An adapter owns five things: detecting an installed agent, finding sessions,
parsing them, building a resume command, and detecting active sessions.

When fixing an adapter:

1. Confirm the current upstream storage format before changing the parser.
2. Add a small sanitized fixture or inline test input.
3. Test malformed and partially written sessions where practical.
4. Keep parsing tolerant of unknown event types.
5. Bump the adapter parser version in `src-tauri/src/indexer/mod.rs` when the
   normalized output changes for existing files.

When adding an agent, register it in both backend and frontend agent lists. A
backend-only adapter can index data but will still be missing labels, colors,
and filters in the UI.

## Verification

Run the checks that cover your change:

```bash
npm run build
cd src-tauri && cargo test
```

Before opening a pull request, also run:

```bash
git diff --check
```

There is no frontend test runner or linter configured yet. For frontend changes,
run the full Tauri app and describe what you checked manually.

## Pull requests

Keep pull requests narrow. Explain:

- What was wrong or missing
- What changed
- How you verified it
- Any session formats, agent versions, or platform assumptions involved

Do not mix formatting changes or unrelated refactors into a functional fix.
