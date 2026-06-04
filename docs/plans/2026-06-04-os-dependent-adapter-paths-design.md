# OS-Dependent Adapter Paths Design

## Goal

Restrict every adapter's existing session-data discovery paths to macOS until
Linux and Windows paths are explicitly implemented.

## Design

Each adapter's first path-resolution function will use explicit
`cfg!(target_os = "...")` branches. The macOS branch preserves the existing
lookup behavior. Linux and Windows branches return `None` or an empty vector
and include `// To be implemented.` comments. An unknown-target fallback also
returns no path.

The branches remain local to each adapter because future platform-specific
storage paths differ by agent. Claude detection will use its gated project
directory resolution so detection and scanning agree.

## Verification

Run the adapter-focused Rust tests, the complete Rust suite, formatting checks,
and `git diff --check`. Compile-check Linux and Windows targets when installed.
