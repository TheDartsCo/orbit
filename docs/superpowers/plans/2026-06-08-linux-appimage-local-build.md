# Linux AppImage Local Build Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a local Ubuntu-only AppImage packaging command that builds Orbit and fails if no AppImage artifact is produced.

**Architecture:** Keep packaging logic in small repository scripts under `scripts/`, expose the user-facing command through `package.json`, and document the local-only support boundary in README. The build script delegates artifact validation to a verifier script so the verifier can be tested without running a full Tauri build.

**Tech Stack:** npm scripts, Bash, Tauri v2 CLI, Rust/Cargo, Vite/TypeScript, Ubuntu Linux.

---

## File Structure

- Create `scripts/verify-linux-appimage.sh`
  - Responsibility: validate that an AppImage artifact directory contains at least one `.AppImage` file and print the artifact path(s).
  - Interface: `bash scripts/verify-linux-appimage.sh [artifact_dir]`
  - Default artifact directory: `src-tauri/target/release/bundle/appimage/`

- Create `scripts/build-linux-appimage.sh`
  - Responsibility: remove stale AppImage artifacts, run Tauri AppImage-only bundling, and call the verifier.
  - Interface: `bash scripts/build-linux-appimage.sh`

- Modify `package.json`
  - Responsibility: expose `npm run build:linux:appimage` as the supported local packaging command.

- Modify `README.md`
  - Responsibility: document the local Ubuntu AppImage build command, artifact path, run command, and support boundary.

---

### Task 1: AppImage Artifact Verifier

**Files:**
- Create: `scripts/verify-linux-appimage.sh`

- [ ] **Step 1: Run the missing verifier command to confirm the red state**

Run from the repo root:

```bash
empty_dir="$(mktemp -d)"
bash scripts/verify-linux-appimage.sh "$empty_dir"
```

Expected: command fails because `scripts/verify-linux-appimage.sh` does not exist yet. The output should include:

```text
scripts/verify-linux-appimage.sh: No such file or directory
```

- [ ] **Step 2: Create the verifier script**

Create `scripts/verify-linux-appimage.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="${1:-$ROOT_DIR/src-tauri/target/release/bundle/appimage}"

if [[ ! -d "$ARTIFACT_DIR" ]]; then
  echo "No AppImage artifact directory found: $ARTIFACT_DIR" >&2
  exit 1
fi

mapfile -t artifacts < <(find "$ARTIFACT_DIR" -maxdepth 1 -type f -name '*.AppImage' | sort)

if [[ "${#artifacts[@]}" -eq 0 ]]; then
  echo "No AppImage artifact found in: $ARTIFACT_DIR" >&2
  exit 1
fi

echo "Created AppImage artifact(s):"
for artifact in "${artifacts[@]}"; do
  echo "  $artifact"
done
```

Make it executable:

```bash
chmod +x scripts/verify-linux-appimage.sh
```

- [ ] **Step 3: Verify the empty-directory failure branch**

Run:

```bash
empty_dir="$(mktemp -d)"
bash scripts/verify-linux-appimage.sh "$empty_dir"
```

Expected: exit code `1`, with output containing:

```text
No AppImage artifact found in:
```

- [ ] **Step 4: Verify the success branch with a fake AppImage**

Run:

```bash
artifact_dir="$(mktemp -d)"
touch "$artifact_dir/Orbit_test.AppImage"
bash scripts/verify-linux-appimage.sh "$artifact_dir"
```

Expected: exit code `0`, with output containing:

```text
Created AppImage artifact(s):
```

and:

```text
Orbit_test.AppImage
```

- [ ] **Step 5: Commit the verifier**

Run:

```bash
git add scripts/verify-linux-appimage.sh
git commit -m "chore: add linux appimage artifact verifier"
```

Expected: commit succeeds and includes only `scripts/verify-linux-appimage.sh`.

---

### Task 2: Local AppImage Build Command

**Files:**
- Create: `scripts/build-linux-appimage.sh`
- Modify: `package.json`

- [ ] **Step 1: Run the missing npm script to confirm the red state**

Run:

```bash
npm run build:linux:appimage
```

Expected: npm fails because the script does not exist yet. The output should include:

```text
Missing script: "build:linux:appimage"
```

- [ ] **Step 2: Create the build script**

Create `scripts/build-linux-appimage.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPIMAGE_DIR="$ROOT_DIR/src-tauri/target/release/bundle/appimage"

if [[ -d "$APPIMAGE_DIR" ]]; then
  find "$APPIMAGE_DIR" -maxdepth 1 -type f -name '*.AppImage' -delete
fi

cd "$ROOT_DIR"
npm run tauri -- build --bundles appimage

bash "$ROOT_DIR/scripts/verify-linux-appimage.sh" "$APPIMAGE_DIR"
```

Make it executable:

```bash
chmod +x scripts/build-linux-appimage.sh
```

- [ ] **Step 3: Add the npm script**

Modify the `"scripts"` section in `package.json` so it is:

```json
"scripts": {
  "dev": "vite",
  "build": "tsc && vite build",
  "build:linux:appimage": "bash scripts/build-linux-appimage.sh",
  "preview": "vite preview",
  "tauri": "tauri"
}
```

Do not change dependencies or devDependencies.

- [ ] **Step 4: Run the local AppImage build**

Run:

```bash
npm run build:linux:appimage
```

Expected: Tauri runs with AppImage-only bundling. The command exits `0` and ends with output containing:

```text
Created AppImage artifact(s):
```

The printed artifact path should be under:

```text
src-tauri/target/release/bundle/appimage/
```

- [ ] **Step 5: Confirm the artifact exists with an independent command**

Run:

```bash
find src-tauri/target/release/bundle/appimage -maxdepth 1 -type f -name '*.AppImage' -print
```

Expected: at least one `.AppImage` path is printed.

- [ ] **Step 6: Commit the build command**

Run:

```bash
git add package.json scripts/build-linux-appimage.sh
git commit -m "chore: add local linux appimage build command"
```

Expected: commit succeeds and includes only `package.json` and `scripts/build-linux-appimage.sh`.

---

### Task 3: README Documentation

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update the intro support sentence**

Replace this text near the top of `README.md`:

```markdown
Orbit is **macOS-first** for release builds, with experimental Windows session
discovery and local Linux development support for Claude Code, Codex, Cursor,
and OpenCode. Linux release packages are not published or fully tested yet.
```

with:

```markdown
Orbit is **macOS-first** for release builds, with experimental Windows session
discovery and local Linux development support for Claude Code, Codex, Cursor,
and OpenCode. AppImage generation is locally verified on the current Ubuntu
development machine, but broader Linux release support is not claimed yet.
```

- [ ] **Step 2: Add AppImage build instructions**

In `README.md`, after the generic source build command block:

````markdown
```bash
git clone https://github.com/TheDartsCo/orbit.git
cd orbit
npm install
npm run tauri build
```
````

add:

````markdown
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
````

- [ ] **Step 3: Update the Linux support paragraph**

Replace this paragraph in `README.md`:

```markdown
Linux local dev support means the app can be built and run from source on a
Linux desktop with Tauri prerequisites installed. Published Linux release
packages are still outside the v0.1 support boundary.
```

with:

```markdown
Linux local dev support means the app can be built and run from source on a
Linux desktop with Tauri prerequisites installed. Local AppImage generation is
available on the current Ubuntu development machine, but published Linux release
packages are still outside the v0.1 support boundary.
```

- [ ] **Step 4: Update the platform-status Linux bullet**

Replace this bullet in `README.md`:

```markdown
- Linux is supported for local development with Claude Code, Codex, Cursor, and
  OpenCode discovery.
```

with:

```markdown
- Linux is supported for local development with Claude Code, Codex, Cursor, and
  OpenCode discovery. Local AppImage generation is verified on the current
  Ubuntu development machine only.
```

- [ ] **Step 5: Run Markdown and whitespace checks**

Run:

```bash
git diff --check README.md
```

Expected: exit code `0`.

- [ ] **Step 6: Commit the README update**

Run:

```bash
git add README.md
git commit -m "docs: document local linux appimage build"
```

Expected: commit succeeds and includes only `README.md`.

---

### Task 4: Full Verification And Optional Local Smoke

**Files:**
- No file edits expected.

- [ ] **Step 1: Run the AppImage build command**

Run:

```bash
npm run build:linux:appimage
```

Expected: exit code `0`, with output containing:

```text
Created AppImage artifact(s):
```

- [ ] **Step 2: Confirm the AppImage artifact independently**

Run:

```bash
find src-tauri/target/release/bundle/appimage -maxdepth 1 -type f -name '*.AppImage' -print
```

Expected: at least one `.AppImage` path is printed.

- [ ] **Step 3: Run the frontend production build**

Run:

```bash
npm run build
```

Expected: exit code `0`. A Vite chunk-size warning is acceptable if the build completes successfully.

- [ ] **Step 4: Run the Rust test suite**

Run:

```bash
cd src-tauri && cargo test
```

Expected: exit code `0` and all tests pass.

- [ ] **Step 5: Run final formatting and whitespace checks**

Run:

```bash
git diff --check
```

Expected: exit code `0`.

- [ ] **Step 6: Optional manual AppImage launch**

Run from the repo root if the local desktop session allows launching GUI apps:

```bash
chmod +x src-tauri/target/release/bundle/appimage/*.AppImage
./src-tauri/target/release/bundle/appimage/*.AppImage
```

Expected: Orbit opens from the AppImage. Close the app after launch.

If the desktop session cannot launch GUI apps from the terminal, do not change code. Record that the manual smoke launch was not run and keep the command output from Steps 1-5 as the required verification evidence.

- [ ] **Step 7: Report final state**

Run:

```bash
git status --short --branch
```

Expected: branch is ahead of its remote by the new implementation commits and there are no unstaged or uncommitted changes except generated ignored build artifacts.
