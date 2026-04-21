# Feature Parity Matrix

This document tracks the real support surface for PostgreSQL, MySQL / MariaDB, and SQLite in Relora.

It should stay aligned with:

- the sidecar driver behavior
- `relora-app` SQL generation and workspace logic
- what the user sees in the TUI help overlay and `Selection` panel

Last reviewed: `2026-04-21`

## Current Matrix

| Capability | PostgreSQL | MySQL / MariaDB | SQLite | Notes |
| --- | --- | --- | --- | --- |
| Catalog browsing | `Done` | `Done` | `Done` | All three drivers expose catalog metadata to the workspace tree. |
| Tree presentation | `Done` | `Partial` | `Partial` | PostgreSQL shows a native `database -> schema -> object` hierarchy. MySQL and SQLite still collapse `database` and `schema` into the same logical level. |
| Object kinds | `Done` | `Partial` | `Partial` | PostgreSQL exposes `Table`, `View`, and `Foreign Table`. MySQL and SQLite currently focus on `Table` and `View`. |
| Data preview | `Done` | `Done` | `Done` | All drivers support paged preview loading. |
| Data tab filter | `Done` | `Done` | `Done` | All drivers support filtered preview, but matching semantics still differ by dialect and collation. |
| Structure / column view | `Done` | `Done` | `Done` | All drivers expose type, nullability, default, and primary key metadata. |
| SQL editor | `Done` | `Done` | `Done` | Editor tabs, current-statement execution, result tabs, and SQL history are app-level. |
| SQL history | `Done` | `Done` | `Done` | Shared workspace behavior. |
| SQL completion | `Done` | `Done` | `Done` | Shared completion model: keywords + loaded objects + loaded columns. It is driver-capable, but not yet dialect-context aware. |
| Result switching | `Done` | `Done` | `Done` | Multi-result browsing is shared app logic. |
| Identifier quoting | `"` | `` ` `` | `"` | Relora now uses driver-aware identifier quoting for generated SQL and copied snippets. |
| CRUD SQL templates | `Done` | `Done` | `Done` | PostgreSQL emits `RETURNING *`; MySQL and SQLite emit dialect-safe templates without `RETURNING`. |
| Copy current cell / row | `Done` | `Done` | `Done` | Shared plain-text copy flow. |
| Copy current `WHERE` clause | `Done` | `Done` | `Done` | Uses driver-aware identifier quoting. |
| `EXPLAIN` | `EXPLAIN` | `EXPLAIN` | `EXPLAIN QUERY PLAN` | Driver capabilities now control the exact statement shape. |
| `EXPLAIN ANALYZE` | `Done` | `Blocked` | `Blocked` | PostgreSQL supports it. MySQL and SQLite are intentionally blocked in-product instead of pretending parity. |
| Staged row edit preview | `Done` | `Done` | `Done` | PostgreSQL uses `RETURNING *`; MySQL and SQLite fall back to `UPDATE ...; SELECT ... WHERE ...;`. |
| Background cancellation | `Done` | `Done` | `Done` | Task cancellation, replacement, and stale-result suppression are shared workspace behavior. |

## In-Product Summary

Relora now surfaces this matrix in-product in two places:

- `?` / `F1` opens keyboard help with a compact driver support section
- the right-hand `Selection` panel shows the active connection's current capability profile:
  - identifier quoting
  - `EXPLAIN` mode
  - `RETURNING` support
  - completion / CRUD template / staged CRUD availability

This should be kept in sync with the real capability flags returned by sidecar drivers.

## What Is Still Not Fully Uniform

The biggest remaining parity gaps are not correctness bugs anymore. They are normalization and UX gaps:

- MySQL and SQLite still use a collapsed tree presentation compared to PostgreSQL
- completion is available everywhere, but it is still generic rather than dialect-context aware
- MySQL `EXPLAIN ANALYZE` is intentionally blocked today instead of being version/family-detected
- object-kind coverage is still richer on PostgreSQL than on MySQL / SQLite

## Next Useful Follow-Ups

- [ ] Add driver-family / version probing for MySQL / MariaDB capability refinement
- [ ] Upgrade completion from generic matching to dialect-aware context
- [ ] Decide whether MySQL and SQLite should keep the collapsed tree model or gain a more tailored object hierarchy
- [ ] Expand parity coverage to result export, saved snippets, and future SQL workflow tooling

## Evidence In Code

This matrix reflects the current implementation in:

- `crates/relora-core/src/db.rs`
- `crates/relora-driver-postgres/src/lib.rs`
- `crates/relora-driver-mysql/src/lib.rs`
- `crates/relora-driver-sqlite/src/lib.rs`
- `crates/relora-app/src/templates.rs`
- `crates/relora-app/src/sql_tools.rs`
- `crates/relora-app/src/workspace.rs`
- `apps/relora/src/tui/render.rs`
- `apps/relora/tests/workspace.rs`

If the implementation changes, update this file in the same pull request.
