# Packaging Relora

This document collects the packaging expectations for Relora as we prepare for future Homebrew and other package-manager integrations.

## Current distribution paths

- `npm install -g relora`
- `curl -fsSL .../scripts/install.sh | sh`
- direct GitHub Releases bundles
- source builds from the Rust workspace

## Source build expectations

Relora should always be buildable from source without requiring the npm or curl installers.

Build all runtime binaries:

```bash
cargo build --release -p relora -p relora-driver-postgres -p relora-driver-mysql -p relora-driver-sqlite
```

The resulting release binaries are expected at:

- `target/release/relora`
- `target/release/relora-driver-postgres`
- `target/release/relora-driver-mysql`
- `target/release/relora-driver-sqlite`

## Non-interactive smoke test

Package managers need a stable command that does not launch the TUI. Relora exposes that through:

```bash
relora paths --json
```

This reports:

- the connection store path
- the current app version
- driver override environment variables
- resolved sidecar driver paths when they are discoverable

For source-build validation, use:

```bash
scripts/smoke-test-source-build.sh
```

## Homebrew notes

For future Homebrew integration, prefer a source-build-oriented workflow:

- build Relora and the sidecar drivers from source
- avoid installer-specific behavior in formula logic
- keep non-interactive diagnostics available for `brew test`
- keep project metadata explicit: homepage, repository, license, and rust-version

Relora is not wired into `homebrew/core` yet, but the source build and smoke-test path should stay maintained so that adding a formula later is straightforward.

## GitHub Actions release automation

The repository ships with:

- `.github/workflows/ci.yml` for format, test, and clippy checks
- `.github/workflows/release.yml` for release bundles and npm publishing
- `npm run release` powered by `bumpp` for local version bumps, commit/tag/push, and release preparation

The release workflow publishes GitHub Release assets on `v*` tags. If the repository secret `NPM_TOKEN` is configured, it also publishes the `packages/relora-npm` package to npm and aligns the package version to the pushed release tag before publishing.

## Maintainer release flow

Use `bumpp` from the repository root:

```bash
npm install
npm run release
```

This flow:

- bumps the root workspace version with `bumpp`
- runs `scripts/sync-version.cjs` to sync `Cargo.toml` and `packages/relora-npm/package.json`
- runs workspace tests and clippy
- commits, tags, and pushes the release
- triggers `.github/workflows/release.yml`
