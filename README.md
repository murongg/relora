# Relora

English | [简体中文](README.zh-CN.md)

Relora is a terminal database workspace built with Rust and `ratatui`.

It is designed for keyboard-first database work: connect to multiple databases, browse structure, preview data, run SQL, and stage safe edits without leaving the terminal.

License: MIT.

## What Relora Does

Relora is a terminal alternative to the “open a GUI client just to inspect a table or run a query” workflow.

It is built for people who:

- prefer terminal-native tools
- manage more than one database connection
- want browsing, preview, and SQL editing in one place
- care about performance and low-overhead architecture

## Features

### Multi-connection workspace

- save and manage multiple named connections
- open one or many connections in the same workspace
- browse databases, schemas, and grouped objects from a left-side asset tree

### Data browsing

- preview tables and table-like objects in the `Data` tab
- page through preview rows
- inspect object columns in the `Structure` tab
- open a row inspector for wide rows and long values
- copy the current cell, row, or generated `WHERE` clause

### SQL workflow

- built-in SQL editor
- execute only the statement under the cursor by default
- SQL history with search and rerun
- SQL autocomplete for keywords, objects, and columns
- multiple SQL tabs and multiple result sets
- PostgreSQL `EXPLAIN` and `EXPLAIN ANALYZE`

### Safer editing flow

- quick filtering from the `Data` tab
- CRUD templates: `SELECT`, `INSERT`, `UPDATE`, `DELETE`
- staged CRUD: edit a cell, preview generated SQL, then commit in a transaction

### Runtime model

- one background worker per connection
- async preview refresh, structure loading, and SQL execution
- task deduplication, cancellation, and priority scheduling

## Supported Databases

Relora currently supports:

- PostgreSQL
- MySQL / MariaDB
- SQLite

Support is provided through external sidecar drivers:

- `relora-driver-postgres`
- `relora-driver-mysql`
- `relora-driver-sqlite`

The main Relora binary does not link database client drivers directly.

## Install

### npm

For end users, the easiest path is npm:

```bash
npm install -g relora
relora
```

The npm package downloads a prebuilt Relora bundle for the current platform from GitHub Releases, including:

- `relora`
- `relora-driver-postgres`
- `relora-driver-mysql`
- `relora-driver-sqlite`

You can also launch it without a global install:

```bash
npx relora
```

### curl (macOS / Linux)

If you prefer a single shell installer:

```bash
curl -fsSL https://raw.githubusercontent.com/murongg/relora/main/scripts/install.sh | sh
```

The installer downloads the matching prebuilt release bundle into `~/.local/bin` by default.

Useful overrides:

```bash
curl -fsSL https://raw.githubusercontent.com/murongg/relora/main/scripts/install.sh | RELORA_VERSION=0.1.0 sh
curl -fsSL https://raw.githubusercontent.com/murongg/relora/main/scripts/install.sh | RELORA_INSTALL_DIR=/usr/local/bin sh
```

### From source

For contributors and local development, the source workflow is still:

```bash
cargo run -p relora
```

To build all runtime binaries from source:

```bash
cargo build --release -p relora -p relora-driver-postgres -p relora-driver-mysql -p relora-driver-sqlite
```

## How to Use Relora

### 1. Start Relora

Open the launcher:

```bash
relora
```

Or from source:

```bash
cargo run -p relora
```

Open a single connection directly:

```bash
cargo run -p relora -- --url postgresql://postgres:postgres@localhost:5432/postgres
```

Open multiple named connections:

```bash
cargo run -p relora -- \
  --connection pg=postgresql://postgres:postgres@localhost:5432/postgres \
  --connection analytics=postgresql://postgres:postgres@localhost:5432/analytics
```

Use environment variables:

```bash
export RELORA_DATABASE_URL=postgresql://postgres:postgres@localhost:5432/postgres
cargo run -p relora
```

Or:

```bash
export RELORA_CONNECTIONS='pg=postgresql://postgres:postgres@localhost:5432/postgres;analytics=postgresql://postgres:postgres@localhost:5432/analytics'
cargo run -p relora
```

Saved connections are stored at:

```text
~/.config/relora/connections.json
```

### 2. Add or edit a connection

If you start in launcher mode, press `a` to add a connection.

The form supports:

- Driver
- Host / SQLite path
- Port
- Database
- User
- Password
- URL override

How it works:

- if `URL override` is filled, Relora uses it directly
- otherwise, Relora builds the URL from structured fields
- the `database` field is optional for server-level connections

Connection testing:

- press `t` on the `Driver` field
- or press `Ctrl-T` anywhere in the form

### 3. Browse data and structure

After launching a connection:

1. use the left asset tree to pick a database, schema, and object
2. use the `Data` tab to preview rows
3. use the `Structure` tab to inspect columns and metadata
4. press `Enter` on a row to open the row inspector

### 4. Run SQL

Open the SQL editor with:

- `F3`
- `Ctrl-2`
- or `e` from the browser

From there you can:

- write SQL
- execute the current statement with `F5` or `Ctrl-Enter`
- switch SQL tabs and result sets
- rerun previous SQL from history with `F10` or `Ctrl-R`
- use `F11` / `F12` for `EXPLAIN` workflows

### 5. Stage and commit edits

From the data grid:

1. move to a cell
2. press `e`
3. enter the new value
4. press `Enter` to preview the generated SQL
5. press `Ctrl-G` in the SQL tab to commit the staged transaction

## Keybindings at a Glance

### Global

- `Tab` / `Shift-Tab`: switch focus between panes
- `F2` / `Ctrl-1`: open `Data`
- `F3` / `Ctrl-2`: open `SQL`
- `F4` / `Ctrl-3`: open `Structure`
- `Ctrl-P`: command palette
- `F10` / `Ctrl-R`: SQL history

### Browser

- `j` / `k` or `Up` / `Down`: move selection
- `Enter`, `Space`, `h`, `l`, `Left`, `Right`: expand or collapse
- `e`: open SQL editor
- `s` / `i` / `u` / `x`: generate CRUD templates
- `r`: refresh
- `c`: cancel tasks

### Data grid

- `j` / `k`: move rows
- `h` / `l`: move columns
- `PageUp` / `PageDown`: scroll by page
- `N` / `P`: next / previous preview page
- `y`: copy row
- `Y`: copy cell
- `w`: copy `WHERE` clause
- `e`: stage cell edit

### SQL editor

- `F5` or `Ctrl-Enter`: execute current statement
- `F11`: `EXPLAIN`
- `F12`: `EXPLAIN ANALYZE`
- `Ctrl-T`: new SQL tab
- `Ctrl-W`: close SQL tab
- `F6` / `F7`: switch SQL tabs
- `F8` / `F9`: switch result sets
- `Ctrl-G`: commit staged CRUD

## Driver Sidecars

Relora does not run `cargo install` inside the TUI. End users should not need a Rust toolchain.

For npm installs, sidecars are bundled into the downloaded runtime package automatically.

For packagers and non-interactive smoke tests, use:

```bash
relora paths --json
```

Driver lookup order:

- `RELORA_POSTGRES_DRIVER` / `RELORA_MYSQL_DRIVER` / `RELORA_SQLITE_DRIVER`
- matching binary in `PATH`
- matching binary next to the Relora executable
- `~/.cargo/bin`
- workspace `target/debug` or `target/release`

## CLI Options

```bash
cargo run -p relora -- --help
```

- `--url`: single database URL
- `--connection`: named connection in `name=url` form, repeatable
- `--preview-limit`: preview row limit, default `100`

## For Contributors

### Monorepo layout

```text
.
├── apps/
│   └── relora/
├── packages/
│   └── relora-npm/
├── scripts/
│   └── package-release-bundle.cjs
└── crates/
    ├── relora-app/
    ├── relora-core/
    ├── relora-driver-mysql/
    ├── relora-driver-postgres/
    └── relora-driver-sqlite/
```

### Package responsibilities

- `apps/relora`: executable app, CLI config, sidecar registry, TUI shell, and `ratatui` rendering
- `packages/relora-npm`: npm installer package that downloads prebuilt Relora bundles
- `scripts/package-release-bundle.cjs`: helper for creating versioned release bundles for npm installs
- `crates/relora-app`: application state, workspace projection, SQL editor state, CRUD helpers, read-only UI views
- `crates/relora-core`: shared database traits and domain models
- `crates/relora-driver-postgres`: PostgreSQL sidecar
- `crates/relora-driver-mysql`: MySQL / MariaDB sidecar
- `crates/relora-driver-sqlite`: SQLite sidecar

### Validation

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Build an npm release bundle

After building release binaries, package them into the versioned archive expected by the npm installer:

```bash
cargo build --release -p relora -p relora-driver-postgres -p relora-driver-mysql -p relora-driver-sqlite
node scripts/package-release-bundle.cjs --platform darwin --arch arm64
```

### Packaging notes

For package-manager integration work, see [docs/packaging.md](docs/packaging.md). A source-build smoke test is also available:

```bash
scripts/smoke-test-source-build.sh
```

Maintainers can cut releases with `bumpp`:

```bash
npm install
npm run release
```
