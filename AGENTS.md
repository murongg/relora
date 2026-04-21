# AGENTS.md

This file is the quick-start guide for coding agents working in the Relora repository.

## Project Summary

Relora is a keyboard-first terminal database workspace built with Rust and `ratatui`.

Core product goals:

- browse multiple database connections in one workspace
- inspect schemas, objects, and preview rows without leaving the terminal
- edit and execute SQL in-place
- keep database drivers as external sidecar binaries instead of linking them into the main app

Supported databases today:

- PostgreSQL
- MySQL / MariaDB
- SQLite

## Repository Layout

Top-level layout:

- `apps/relora`: main TUI application and launcher
- `crates/relora-app`: shared app state, workspace behavior, background task logic
- `crates/relora-core`: database abstractions, domain types, shared utilities
- `crates/relora-driver-postgres`: PostgreSQL sidecar driver
- `crates/relora-driver-mysql`: MySQL / MariaDB sidecar driver
- `crates/relora-driver-sqlite`: SQLite sidecar driver
- `packages/relora-npm`: npm wrapper that downloads release bundles
- `scripts`: installer, release bundle, source smoke-test, and version sync scripts
- `.github/workflows`: CI and release automation
- `docs/packaging.md`: packaging and release expectations

## Architecture Rules

### Sidecar driver boundary

The main app must not link database client drivers directly.

- `apps/relora` depends on `relora-core` and `relora-app`
- database-specific protocol/client crates belong in sidecar driver crates
- driver discovery and launch logic belongs in `apps/relora/src/drivers.rs`

If you add database support, prefer adding a new sidecar crate instead of pushing DB client code into the app binary.

### TUI module boundaries

The TUI is intentionally split into focused modules under `apps/relora/src/tui`.

Keep responsibilities separated:

- `mod.rs`: runtime/bootstrap wiring
- `input.rs`: keyboard and mouse handling
- `render.rs`: drawing logic
- `layout.rs`: layout composition
- `grid.rs`: table/grid layout helpers
- `colors.rs`: theme colors only
- `metrics.rs`: layout constants only
- `shortcuts.rs`: key constants and help strings only
- `strings.rs`: copy/text constants only

Do not reintroduce scattered UI literals when there is already a central module for them.

Examples:

- color constants belong in `colors.rs`
- sizing/layout constants belong in `metrics.rs`
- user-facing strings belong in `strings.rs`
- key definitions and shortcut help belong in `shortcuts.rs`

### Workspace behavior

Most interactive behavior lives in `crates/relora-app/src/workspace.rs`.

Use that crate for:

- selection changes
- background task orchestration
- SQL execution behavior
- row preview / structure loading
- staged CRUD flow

Try to keep `apps/relora` focused on shell/TUI concerns and `relora-app` focused on application behavior.

## Build and Validation

The workspace MSRV is Rust `1.85`.

Prefer validating with the pinned MSRV whenever you touch dependencies, CI, release automation, or syntax that may be version-sensitive.

Common commands:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

MSRV-sensitive verification:

```bash
cargo +1.85.0 test --workspace
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
```

Source-build smoke test:

```bash
scripts/smoke-test-source-build.sh
```

Non-interactive runtime diagnostic:

```bash
cargo run -p relora -- paths --json
```

## Dependency Policy

Do not casually widen dependency versions.

The workspace intentionally pins several crates to stay compatible with Rust 1.85 and with the current release pipeline.

Pay special attention to:

- `ratatui`
- `arboard`
- `mysql`
- `url`

If you change dependency versions:

1. update `Cargo.toml`
2. refresh `Cargo.lock`
3. run MSRV validation with `cargo +1.85.0`

Avoid reintroducing versions that require newer Rust than `1.85`.

## Release and Packaging

Important files:

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `package.json`
- `scripts/sync-version.cjs`
- `scripts/package-release-bundle.cjs`
- `scripts/install.sh`
- `packages/relora-npm/package.json`

Release flow:

```bash
npm install
npm run release
```

This uses `bumpp`, syncs versions across the workspace, tags with `v*`, and triggers the release workflow.

Packaging rules:

- GitHub Releases bundles are the source of truth for npm and curl installers
- npm package name is `relora`
- release bundles must include `relora` plus all sidecar drivers

GitHub Actions note:

- do not reference `secrets.*` directly in a job-level `if:` expression in `release.yml`
- prefer a detection step that writes to `$GITHUB_OUTPUT`, then gate later steps off that output

## Testing Strategy

There are strong architecture tests in `apps/relora/tests/architecture.rs`.

Unit and integration tests in this repository should target code under `apps/` and `crates/`.

Do not add unit-test-style assertions for non-code assets such as:

- `README` files
- `LICENSE`
- `.github/workflows`
- `scripts/`
- `packages/`
- release or packaging metadata outside Rust application code

If you change:

- TUI module structure
- shortcut centralization

expect architecture tests to fail if you drift from established patterns.

Behavioral coverage is split across:

- `apps/relora/src/tui/tests.rs`
- `apps/relora/src/tui/snapshot_tests.rs`
- `apps/relora/tests/workspace.rs`
- `apps/relora/tests/cli.rs`

Use the layers intentionally:

- `apps/relora/src/tui/tests.rs`: focused rendering/input regressions and layout edge cases
- `apps/relora/src/tui/snapshot_tests.rs`: golden snapshots for stable core surfaces such as launcher, data/sql/structure tabs, row inspector, and help overlay
- `apps/relora/tests/workspace.rs`: higher-level end-to-end-ish workspace flows that span background work, tab switching, filtering, SQL execution, and row inspection

When you change a major TUI surface:

- update or extend the relevant golden snapshot
- add or update a workspace flow test if the change spans multiple panes, background tasks, or user steps

Snapshot update command:

```bash
RELORA_UPDATE_TUI_SNAPSHOTS=1 cargo test -p relora golden_snapshot -- --nocapture
```

When fixing bugs, prefer adding or updating a focused regression test before or alongside the production change.

## Contributor Guidance for Agents

Before making larger changes:

- inspect existing tests near the area you are editing
- preserve the sidecar-driver architecture
- preserve centralized TUI constants/modules
- preserve Rust 1.85 compatibility

When editing docs:

- English README is the default entrypoint
- Simplified Chinese lives in `README.zh-CN.md`
- keep install and usage docs aligned across both files when relevant

When done:

- summarize the user-visible effect
- mention any validation you actually ran
- call out anything you could not verify
