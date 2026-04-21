# Feature Parity Matrix

This document tracks the current PostgreSQL / MySQL / SQLite support surface in Relora and turns the remaining gaps into an execution checklist.

It is intentionally grounded in the current codebase, not aspirational product copy.

Status meanings:

- `Done`: implemented and already exercised by existing driver logic or workspace tests
- `Partial`: available today, but still dialect-generic, runtime-dependent, or not fully normalized across drivers
- `Gap`: not yet productized and should be treated as follow-up work

Last reviewed: `2026-04-21`

## Current Matrix

| Capability | PostgreSQL | MySQL / MariaDB | SQLite | Notes |
| --- | --- | --- | --- | --- |
| Catalog browsing | `Done` | `Done` | `Done` | All three drivers implement `load_catalog`. PostgreSQL exposes real `database -> schema -> object`; MySQL and SQLite collapse `database` and `schema` to the same label. |
| Object kinds | `Done` | `Partial` | `Partial` | PostgreSQL exposes `Table`, `View`, and `Foreign Table`. MySQL and SQLite currently expose only `Table` and `View`. |
| Data preview | `Done` | `Done` | `Done` | All drivers implement paged `load_preview_page(limit, offset)`. |
| Data tab filter | `Done` | `Done` | `Done` | All drivers implement filtered preview, but matching semantics differ: PostgreSQL uses `ILIKE`, MySQL depends on collation, SQLite uses `LIKE`. |
| Structure / column view | `Done` | `Done` | `Done` | All drivers implement `load_object_columns`, including PK/default/nullability metadata. |
| SQL editor | `Done` | `Done` | `Done` | Editor, current-statement execution, result tabs, and history are app-level and available for every driver. |
| SQL history | `Done` | `Done` | `Done` | Implemented in `WorkspaceApp`; not dialect-specific. |
| SQL completion | `Done` | `Done` | `Done` | Current completion model is generic: keywords + loaded objects + loaded columns. It is scoped by active database, but not yet dialect-aware. |
| Result switching | `Done` | `Done` | `Done` | Multiple query results can be browsed in the SQL tab for every driver. |
| CRUD SQL templates | `Done` | `Partial` | `Partial` | Template generation is shared app logic. `INSERT / UPDATE / DELETE` currently append `RETURNING *`, which is a clean fit for PostgreSQL but still needs dialect review for MySQL and runtime-version validation for SQLite. |
| Copy current cell / row | `Done` | `Done` | `Done` | Plain-text copy flows are shared app logic and already stable. |
| Copy current `WHERE` clause | `Done` | `Partial` | `Done` | Shared SQL builder uses ANSI double-quoted identifiers. That is fine for PostgreSQL and SQLite, but not a native MySQL / MariaDB snippet format. |
| `EXPLAIN` | `Done` | `Partial` | `Partial` | Relora prepends `EXPLAIN` to the current statement. PostgreSQL is the productized path today. MySQL and SQLite need driver-aware UX and validation. |
| `EXPLAIN ANALYZE` | `Done` | `Partial` | `Gap` | PostgreSQL is first-class. MySQL / MariaDB behavior varies by server family/version. SQLite should likely map to `EXPLAIN QUERY PLAN` instead of reusing the PostgreSQL wording. |
| Staged row edit preview | `Done` | `Partial` | `Partial` | Shared staged CRUD SQL currently emits PostgreSQL-friendly `BEGIN ... UPDATE ... RETURNING * ... ROLLBACK/COMMIT`. MySQL and SQLite need dialect/runtime review. |
| Background cancellation | `Done` | `Done` | `Done` | Cancellation, late-result suppression, and preview task replacement live in `relora-app`, not in a single driver. |

## What The Matrix Says

Relora already has strong parity in the read path:

- catalog browsing
- row preview
- filtering
- structure inspection
- SQL editing
- SQL history
- generic completion

The main parity gaps are on the write and dialect-sensitive path:

- CRUD templates are still PostgreSQL-shaped
- staged CRUD preview is still PostgreSQL-shaped
- copied `WHERE` clauses are not MySQL-native
- `EXPLAIN` behavior is only truly productized for PostgreSQL
- catalog presentation still differs between PostgreSQL and the collapsed MySQL / SQLite model

## Execution Checklist

### P0: close correctness gaps in cross-database workflows

- [ ] Add driver capability flags for `returning`, `identifier quoting`, and `explain strategy`
- [ ] Make CRUD template generation dialect-aware instead of emitting one shared SQL shape
- [ ] Make staged CRUD preview/commit SQL dialect-aware, including non-`RETURNING` fallbacks
- [ ] Split `EXPLAIN` behavior into driver-specific flows:
  - PostgreSQL: `EXPLAIN` / `EXPLAIN ANALYZE`
  - MySQL / MariaDB: validate `EXPLAIN` and family/version support before surfacing `ANALYZE`
  - SQLite: map to `EXPLAIN QUERY PLAN`
- [ ] Make copied `WHERE` clauses render with driver-appropriate identifier quoting

### P0: add driver-specific regression coverage

- [ ] Add PostgreSQL / MySQL / SQLite integration tests for filtered preview semantics
- [ ] Add PostgreSQL / MySQL / SQLite integration tests for CRUD template generation
- [ ] Add PostgreSQL / MySQL / SQLite integration tests for staged row edit SQL generation
- [ ] Add PostgreSQL / MySQL / SQLite integration tests for `EXPLAIN` / `EXPLAIN ANALYZE` behavior
- [ ] Add golden workspace scenarios that snapshot the same flows across all three drivers

### P1: normalize UX, not just backend support

- [ ] Decide whether MySQL and SQLite should continue to show a collapsed `database/schema` model or receive a dedicated tree presentation
- [ ] Surface unsupported or partial actions in the UI instead of letting users discover them by execution failure
- [ ] Upgrade completion from generic keyword/object/column matching to driver-aware context
- [ ] Add a small in-product support summary so users can see which features are first-class on the current connection

## Evidence In Code

This matrix is based on the current implementation in:

- `crates/relora-driver-postgres/src/lib.rs`
- `crates/relora-driver-mysql/src/lib.rs`
- `crates/relora-driver-sqlite/src/lib.rs`
- `crates/relora-app/src/templates.rs`
- `crates/relora-app/src/sql_tools.rs`
- `crates/relora-app/src/completion.rs`
- `crates/relora-app/src/workspace.rs`
- `apps/relora/tests/workspace.rs`

If the implementation changes, update this file in the same pull request.
