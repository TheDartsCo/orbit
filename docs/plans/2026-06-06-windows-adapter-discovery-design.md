# Windows Adapter Discovery Design

## Goal

Discover sessions for all eight Orbit adapters on Windows while postponing
automatic resume launching. Windows users must still be able to inspect and
copy the session ID and generated resume command.

## Discovery

Each adapter keeps ownership of its storage format and validation rules.
Shared path helpers provide Windows home, roaming application-data, and local
application-data roots. Adapters build a short ordered list of known candidate
locations and select only candidates whose expected directory structure,
files, or SQLite database exist.

Home-relative stores:

- Claude: `%USERPROFILE%\.claude\projects`
- Codex: `%USERPROFILE%\.codex`
- Copilot: `%USERPROFILE%\.copilot\session-state`
- Cursor: `%USERPROFILE%\.cursor`
- OpenCode: `%USERPROFILE%\.local\share\opencode`, then local/roaming
  application-data candidates

Application-data stores:

- JetBrains AI: `%APPDATA%\JetBrains` and `%LOCALAPPDATA%\JetBrains`
- Qoder: roaming/local Qoder `SharedClientCache\cache\db\local.db` candidates
- Warp: local/roaming Warp application-data `warp.sqlite` candidates

Candidate ordering is deterministic. Discovery does not recursively search
arbitrary user directories. Database-backed adapters validate the expected
database file before detection succeeds, and their existing queries remain the
schema-level validation during indexing.

## Resume UX

The backend reports the current operating system to the frontend. On Windows,
clicking Resume opens an Orbit modal instead of invoking `launch_resume`. The
modal explains that automatic resume support is coming soon and displays the
session ID and generated command in separate copyable fields. For adapters
without resume support, the modal still exposes the session ID and clearly
states that no resume command is available.

Copy Resume continues to copy the command directly for resumable adapters.
macOS launch behavior remains unchanged. The Windows branch in
`launch_resume` returns an explicit unsupported error as a backend safety net.

## Verification

Rust unit tests cover Windows candidate construction and candidate selection
using temporary directories. Existing adapter tests cover parsing and database
queries. Run `cargo test`, `cargo fmt --check`, `npm run build`, and
`git diff --check`. A native Windows compile cannot be claimed unless the
Windows Rust target is installed.
