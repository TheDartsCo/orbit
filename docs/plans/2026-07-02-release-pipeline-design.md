# Release Pipeline Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Set up an automated GitHub Actions release pipeline for macOS and Linux.

**Architecture:** A single `.github/workflows/publish.yml` triggered on semver tags (`v*`) and `workflow_dispatch`. macOS builds are signed and notarized using an Apple Developer certificate stored in GitHub Secrets. Linux builds produce .deb and .AppImage. A separate checksums + provenance job attests and hashes all artifacts.

**Tech Stack:** GitHub Actions, Tauri v2 (`tauri-apps/tauri-action@v0`), `actions/attest@v4` for provenance, SHA256 checksums.

## Workflow: `.github/workflows/publish.yml`

**Trigger:** `push tags: v*` + `workflow_dispatch`

**Jobs:**

### publish-tauri (matrix)
- **macos-latest** → aarch64 .dmg signed & notarized
- **macos-13** → x86_64 .dmg signed & notarized
- **ubuntu-22.04** → x86_64 .deb + .AppImage
- Imports Apple Developer cert into keychain, signs, notarizes via env
- Creates/updates GitHub Release (draft), uploads artifacts per platform

### checksums
- Downloads all release assets, generates `sha256sum` checksums.txt, uploads to release

### attest
- Attests every asset with `actions/attest@v4` for SLSA build provenance

**Required secrets:** `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`

**No Windows signing yet** — pipeline can be extended later to add a Windows runner + signing.

---
