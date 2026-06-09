# Linux AppImage Local Build Design

## Goal

Prove that Orbit can create a Linux AppImage from the current Ubuntu development
machine with a repeatable local command.

This is a first packaging pass only. It should make AppImage generation easy to
run and verify locally without claiming broad Linux release support.

## Scope

Add a local AppImage-only build path:

- Add an npm script named `build:linux:appimage`.
- Build only the AppImage bundle instead of every Linux bundle target.
- Add a small verification script that fails when no `.AppImage` artifact is
  created.
- Document the Ubuntu prerequisites, command, output path, and manual run steps.

The expected artifact directory is:

```text
src-tauri/target/release/bundle/appimage/
```

## Non-Goals

- No GitHub Actions workflow.
- No GitHub release upload.
- No `.deb`, `.rpm`, Snap, Flatpak, or AUR packaging.
- No artifact signing.
- No claim that the AppImage supports older Linux distributions.
- No adapter behavior changes.

## Design

The npm script should be the user-facing entry point. It should run a repository
script rather than inline shell so the artifact check is testable and easy to
extend later.

Proposed command:

```bash
npm run build:linux:appimage
```

The repository script should:

1. Remove stale AppImage files from the expected output directory if it exists.
2. Run Tauri with AppImage-only bundling.
3. Check that at least one `.AppImage` exists in the expected output directory.
4. Print the generated artifact path.

The Tauri invocation is:

```bash
npm run tauri -- build --bundles appimage
```

The script should avoid CI-specific assumptions and should not upload or move
artifacts. The output should remain under Tauri's standard bundle directory.

## Documentation

README should keep the support language conservative:

- Linux AppImage generation is locally verified on the current Ubuntu machine.
- AppImage portability depends on the build system's Linux baseline.
- Broader Linux release support is still not claimed.

The docs should include:

```bash
npm run build:linux:appimage
chmod +x src-tauri/target/release/bundle/appimage/*.AppImage
./src-tauri/target/release/bundle/appimage/*.AppImage
```

The README should continue to mention the Ubuntu/Debian Tauri prerequisites that
are already documented for local Linux development.

## Verification

Required verification:

```bash
npm run build:linux:appimage
npm run build
cd src-tauri && cargo test
```

The AppImage build command passes only if the verification script finds a fresh
`.AppImage` artifact.

Manual smoke test, if the local desktop session allows it:

```bash
chmod +x src-tauri/target/release/bundle/appimage/*.AppImage
./src-tauri/target/release/bundle/appimage/*.AppImage
```

Expected result: Orbit launches from the AppImage.

## Risks

AppImage portability is limited by the glibc and system-library baseline of the
machine that produced it. This pass intentionally proves only the current Ubuntu
machine. A later release-quality pass should build on an older baseline such as
Ubuntu 22.04 or Debian 12, ideally in CI or a container.

Tauri's AppImage tooling may require additional native packages beyond the
development dependencies already documented. If the build fails because a native
tool is missing, update the local prerequisite docs with the exact package that
fixed the build.

## Acceptance Criteria

- `npm run build:linux:appimage` exists.
- The command creates at least one `.AppImage` under Tauri's appimage bundle
  directory.
- The command fails if no AppImage artifact is produced.
- README documents the local AppImage build command and artifact path.
- Existing frontend build and Rust tests still pass.
