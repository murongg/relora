use std::collections::VecDeque;

use anyhow::Result;
use relora_core::app::{App, AppAction, Focus};
use relora_core::db::{
    Catalog, DatabaseDriver, DatabaseEntry, DatabaseKind, DbColumn, DbObjectKind, DbObjectRef,
    SchemaEntry, SqlExecutionResult, TablePreview,
};

#[derive(Debug)]
struct MockDriver {
    catalogs: VecDeque<Catalog>,
    previews: VecDeque<TablePreview>,
}

impl MockDriver {
    fn new(catalogs: Vec<Catalog>, previews: Vec<TablePreview>) -> Self {
        Self {
            catalogs: VecDeque::from(catalogs),
            previews: VecDeque::from(previews),
        }
    }
}

impl DatabaseDriver for MockDriver {
    fn kind(&self) -> DatabaseKind {
        DatabaseKind::Postgres
    }

    fn connection_label(&self) -> &str {
        "mock://postgres"
    }

    fn load_catalog(&mut self) -> Result<Catalog> {
        self.catalogs
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("missing mocked catalog"))
    }

    fn load_preview_page(
        &mut self,
        _table: &DbObjectRef,
        _limit: usize,
        _offset: usize,
    ) -> Result<TablePreview> {
        self.previews
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("missing mocked preview"))
    }

    fn load_filtered_preview_page(
        &mut self,
        table: &DbObjectRef,
        _filter: &str,
        limit: usize,
        offset: usize,
    ) -> Result<TablePreview> {
        self.load_preview_page(table, limit, offset)
    }

    fn load_object_columns(&mut self, _table: &DbObjectRef) -> Result<Vec<DbColumn>> {
        Ok(Vec::new())
    }

    fn execute_sql(
        &mut self,
        _database: Option<&str>,
        _sql: &str,
    ) -> Result<Vec<SqlExecutionResult>> {
        Err(anyhow::anyhow!("sql execution is not used in this test"))
    }
}

fn catalog(names: &[(&str, &[(DbObjectKind, &str)])]) -> Catalog {
    Catalog {
        databases: vec![DatabaseEntry {
            name: "postgres".to_string(),
            schemas: names
                .iter()
                .map(|(schema, objects)| SchemaEntry {
                    database: "postgres".to_string(),
                    name: (*schema).to_string(),
                    objects: objects
                        .iter()
                        .map(|(kind, name)| DbObjectRef {
                            database: "postgres".to_string(),
                            schema: (*schema).to_string(),
                            name: (*name).to_string(),
                            kind: *kind,
                        })
                        .collect(),
                })
                .collect(),
        }],
    }
}

fn preview(columns: &[&str], rows: &[&[&str]]) -> TablePreview {
    TablePreview {
        columns: columns.iter().map(|value| (*value).to_string()).collect(),
        rows: rows
            .iter()
            .map(|row| row.iter().map(|value| (*value).to_string()).collect())
            .collect(),
    }
}

#[test]
fn bootstrap_selects_the_first_schema_and_table_preview() -> Result<()> {
    let mut driver = MockDriver::new(
        vec![catalog(&[
            (
                "public",
                &[
                    (DbObjectKind::Table, "users"),
                    (DbObjectKind::Table, "orders"),
                ],
            ),
            ("analytics", &[(DbObjectKind::View, "events")]),
        ])],
        vec![preview(&["id", "email"], &[&["1", "alice@example.com"]])],
    );

    let app = App::bootstrap(&mut driver, 20)?;

    assert_eq!(app.focus(), Focus::Schemas);
    assert_eq!(app.selected_schema_name(), Some("public"));
    assert_eq!(
        app.selected_object().map(|table| table.name.as_str()),
        Some("users")
    );
    assert_eq!(app.preview().columns, vec!["id", "email"]);
    Ok(())
}

#[test]
fn moving_between_panes_and_items_keeps_preview_in_sync() -> Result<()> {
    let mut driver = MockDriver::new(
        vec![catalog(&[
            (
                "public",
                &[
                    (DbObjectKind::Table, "users"),
                    (DbObjectKind::Table, "orders"),
                ],
            ),
            ("analytics", &[(DbObjectKind::View, "events")]),
        ])],
        vec![
            preview(&["id"], &[&["1"]]),
            preview(&["id"], &[&["101"]]),
            preview(&["ts"], &[&["2026-04-18T00:00:00Z"]]),
        ],
    );

    let mut app = App::bootstrap(&mut driver, 20)?;
    app.apply_action(AppAction::NextPane, &mut driver);
    app.apply_action(AppAction::NextItem, &mut driver);

    assert_eq!(app.focus(), Focus::Objects);
    assert_eq!(
        app.selected_object().map(|table| table.name.as_str()),
        Some("orders")
    );
    assert_eq!(app.preview().rows[0][0], "101");

    app.apply_action(AppAction::PreviousPane, &mut driver);
    app.apply_action(AppAction::NextItem, &mut driver);

    assert_eq!(app.selected_schema_name(), Some("analytics"));
    assert_eq!(
        app.selected_object().map(|table| table.name.as_str()),
        Some("events")
    );
    assert_eq!(app.preview().columns, vec!["ts"]);
    Ok(())
}

#[test]
fn refresh_preserves_schema_and_table_selection_by_name() -> Result<()> {
    let mut driver = MockDriver::new(
        vec![
            catalog(&[(
                "public",
                &[
                    (DbObjectKind::Table, "users"),
                    (DbObjectKind::View, "orders"),
                ],
            )]),
            catalog(&[(
                "public",
                &[
                    (DbObjectKind::Table, "users"),
                    (DbObjectKind::View, "orders"),
                    (DbObjectKind::Table, "audit_logs"),
                ],
            )]),
        ],
        vec![
            preview(&["id"], &[&["1"]]),
            preview(&["id"], &[&["99"]]),
            preview(&["id"], &[&["99"]]),
        ],
    );

    let mut app = App::bootstrap(&mut driver, 20)?;
    app.apply_action(AppAction::NextPane, &mut driver);
    app.apply_action(AppAction::NextItem, &mut driver);
    app.apply_action(AppAction::Refresh, &mut driver);

    assert_eq!(app.selected_schema_name(), Some("public"));
    assert_eq!(
        app.selected_object().map(|table| table.name.as_str()),
        Some("orders")
    );
    assert_eq!(app.preview().rows[0][0], "99");
    Ok(())
}

#[test]
fn postgres_urls_are_recognized_for_driver_selection() {
    let kind = DatabaseKind::from_url("postgresql://postgres:postgres@localhost/postgres")
        .expect("postgres url should be supported");

    assert_eq!(kind, DatabaseKind::Postgres);
}

#[test]
fn common_database_urls_are_recognized_for_sidecar_drivers() {
    assert_eq!(
        DatabaseKind::from_url("mysql://root:secret@localhost/app").unwrap(),
        DatabaseKind::MySql
    );
    assert_eq!(
        DatabaseKind::from_url("mariadb://root:secret@localhost/app").unwrap(),
        DatabaseKind::MySql
    );
    assert_eq!(
        DatabaseKind::from_url("sqlite:///tmp/relora.db").unwrap(),
        DatabaseKind::Sqlite
    );
}

#[test]
fn selecting_an_object_by_path_updates_preview() -> Result<()> {
    let mut driver = MockDriver::new(
        vec![catalog(&[(
            "public",
            &[
                (DbObjectKind::Table, "users"),
                (DbObjectKind::View, "active_users"),
            ],
        )])],
        vec![preview(&["id"], &[&["1"]]), preview(&["id"], &[&["2"]])],
    );

    let mut app = App::bootstrap(&mut driver, 20)?;
    app.select_object("postgres", "public", "active_users", &mut driver)?;

    let selected = app.selected_object().expect("selected object should exist");
    assert_eq!(selected.name, "active_users");
    assert_eq!(selected.kind, DbObjectKind::View);
    assert_eq!(app.preview().rows[0][0], "2");
    Ok(())
}

#[test]
fn bootstrap_supports_multiple_databases_and_object_selection_across_them() -> Result<()> {
    let mut driver = MockDriver::new(
        vec![Catalog {
            databases: vec![
                DatabaseEntry {
                    name: "app".to_string(),
                    schemas: vec![SchemaEntry {
                        database: "app".to_string(),
                        name: "public".to_string(),
                        objects: vec![DbObjectRef {
                            database: "app".to_string(),
                            schema: "public".to_string(),
                            name: "users".to_string(),
                            kind: DbObjectKind::Table,
                        }],
                    }],
                },
                DatabaseEntry {
                    name: "analytics".to_string(),
                    schemas: vec![SchemaEntry {
                        database: "analytics".to_string(),
                        name: "mart".to_string(),
                        objects: vec![DbObjectRef {
                            database: "analytics".to_string(),
                            schema: "mart".to_string(),
                            name: "events".to_string(),
                            kind: DbObjectKind::View,
                        }],
                    }],
                },
            ],
        }],
        vec![
            preview(&["id"], &[&["1"]]),
            preview(&["event_id"], &[&["evt_1"]]),
        ],
    );

    let mut app = App::bootstrap(&mut driver, 20)?;
    assert_eq!(app.selected_database_name(), Some("app"));

    app.select_object("analytics", "mart", "events", &mut driver)?;

    assert_eq!(app.selected_database_name(), Some("analytics"));
    assert_eq!(app.selected_schema_name(), Some("mart"));
    assert_eq!(
        app.selected_object().map(|object| object.name.as_str()),
        Some("events")
    );
    assert_eq!(app.preview().rows[0][0], "evt_1");
    Ok(())
}
