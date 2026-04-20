use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use anyhow::Result;
use relora_app::view::RightPaneTab;
use relora_app::workspace::{ConnectionBootstrap, WorkspaceAction, WorkspaceApp};
use relora_core::db::{
    Catalog, DatabaseDriver, DatabaseEntry, DatabaseKind, DbColumn, DbObjectKind, DbObjectRef,
    QueryResult, SchemaEntry, SqlExecutionResult, TablePreview,
};

#[derive(Debug)]
struct MockDriver {
    catalogs: VecDeque<Catalog>,
    previews: VecDeque<TablePreview>,
    filtered_previews: VecDeque<TablePreview>,
    columns: VecDeque<Vec<DbColumn>>,
    executions: VecDeque<Vec<SqlExecutionResult>>,
    executed_sql: Arc<Mutex<Vec<String>>>,
}

impl MockDriver {
    fn new(
        catalogs: Vec<Catalog>,
        previews: Vec<TablePreview>,
        columns: Vec<Vec<DbColumn>>,
        executions: Vec<Vec<SqlExecutionResult>>,
    ) -> Self {
        Self {
            catalogs: VecDeque::from(catalogs),
            previews: VecDeque::from(previews),
            filtered_previews: VecDeque::new(),
            columns: VecDeque::from(columns),
            executions: VecDeque::from(executions),
            executed_sql: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_filtered_previews(mut self, previews: Vec<TablePreview>) -> Self {
        self.filtered_previews = VecDeque::from(previews);
        self
    }

    fn with_sql_recorder(mut self, executed_sql: Arc<Mutex<Vec<String>>>) -> Self {
        self.executed_sql = executed_sql;
        self
    }
}

#[derive(Debug)]
struct BlockingPreviewDriver {
    catalogs: VecDeque<Catalog>,
    previews: VecDeque<TablePreview>,
    unblock_preview: Option<mpsc::Receiver<()>>,
    preview_calls: usize,
}

impl BlockingPreviewDriver {
    fn new(
        catalogs: Vec<Catalog>,
        previews: Vec<TablePreview>,
        unblock_preview: mpsc::Receiver<()>,
    ) -> Self {
        Self {
            catalogs: VecDeque::from(catalogs),
            previews: VecDeque::from(previews),
            unblock_preview: Some(unblock_preview),
            preview_calls: 0,
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
        _table: &DbObjectRef,
        _filter: &str,
        _limit: usize,
        _offset: usize,
    ) -> Result<TablePreview> {
        self.filtered_previews
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("missing mocked filtered preview"))
    }

    fn load_object_columns(&mut self, _table: &DbObjectRef) -> Result<Vec<DbColumn>> {
        self.columns
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("missing mocked columns"))
    }

    fn execute_sql(
        &mut self,
        _database: Option<&str>,
        _sql: &str,
    ) -> Result<Vec<SqlExecutionResult>> {
        self.executed_sql
            .lock()
            .expect("sql recorder lock should be available")
            .push(_sql.to_string());
        self.executions
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("missing mocked execution"))
    }
}

impl DatabaseDriver for BlockingPreviewDriver {
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
        self.preview_calls += 1;
        if self.preview_calls > 1
            && let Some(receiver) = self.unblock_preview.take()
        {
            receiver
                .recv()
                .map_err(|_| anyhow::anyhow!("preview unblock signal was dropped"))?;
        }

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

fn catalog(schema: &str, objects: &[(DbObjectKind, &str)]) -> Catalog {
    Catalog {
        databases: vec![DatabaseEntry {
            name: "postgres".to_string(),
            schemas: vec![SchemaEntry {
                database: "postgres".to_string(),
                name: schema.to_string(),
                objects: objects
                    .iter()
                    .map(|(kind, name)| DbObjectRef {
                        database: "postgres".to_string(),
                        schema: schema.to_string(),
                        name: (*name).to_string(),
                        kind: *kind,
                    })
                    .collect(),
            }],
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

fn columns(values: &[(&str, &str, bool, bool, bool)]) -> Vec<DbColumn> {
    values
        .iter()
        .map(
            |(name, data_type, nullable, has_default, is_primary_key)| DbColumn {
                name: (*name).to_string(),
                data_type: (*data_type).to_string(),
                nullable: *nullable,
                has_default: *has_default,
                is_primary_key: *is_primary_key,
            },
        )
        .collect()
}

fn query(columns: &[&str], rows: &[&[&str]]) -> SqlExecutionResult {
    SqlExecutionResult::Query(QueryResult {
        columns: columns.iter().map(|value| (*value).to_string()).collect(),
        rows: rows
            .iter()
            .map(|row| row.iter().map(|value| (*value).to_string()).collect())
            .collect(),
    })
}

fn query_batch(items: Vec<SqlExecutionResult>) -> Vec<SqlExecutionResult> {
    items
}

fn drain_until_idle(workspace: &mut WorkspaceApp) -> Result<()> {
    for _ in 0..50 {
        workspace.drain_background()?;
        if !workspace.has_pending_tasks() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(anyhow::anyhow!("workspace did not become idle in time"))
}

#[test]
fn workspace_bootstrap_builds_a_multi_connection_asset_tree() -> Result<()> {
    let bootstraps = vec![
        ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog(
                    "public",
                    &[
                        (DbObjectKind::Table, "users"),
                        (DbObjectKind::View, "active_users"),
                    ],
                )],
                vec![preview(&["id"], &[&["1"]])],
                vec![],
                vec![],
            )),
        },
        ConnectionBootstrap {
            name: "analytics".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog("mart", &[(DbObjectKind::Table, "events")])],
                vec![
                    preview(&["event_id"], &[&["evt_0"]]),
                    preview(&["event_id"], &[&["evt_1"]]),
                ],
                vec![],
                vec![],
            )),
        },
    ];

    let workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let rows = workspace.tree_rows();
    let labels = rows.iter().map(|row| row.label.clone()).collect::<Vec<_>>();

    assert!(labels.contains(&"pg".to_string()));
    assert!(labels.contains(&"analytics".to_string()));
    assert!(labels.contains(&"Tables".to_string()));
    assert_eq!(workspace.selected_row().label, "users");
    Ok(())
}

#[test]
fn workspace_can_navigate_to_a_table_in_another_connection() -> Result<()> {
    let bootstraps = vec![
        ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
                vec![],
                vec![],
            )),
        },
        ConnectionBootstrap {
            name: "analytics".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog("mart", &[(DbObjectKind::Table, "events")])],
                vec![
                    preview(&["event_id"], &[&["evt_0"]]),
                    preview(&["event_id"], &[&["evt_1"]]),
                ],
                vec![],
                vec![],
            )),
        },
    ];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    for _ in 0..4 {
        workspace.apply_action(WorkspaceAction::NextItem)?;
    }
    drain_until_idle(&mut workspace)?;

    assert_eq!(workspace.selected_row().label, "events");
    assert_eq!(workspace.active_preview().columns, vec!["event_id"]);
    Ok(())
}

#[test]
fn workspace_tree_navigation_stops_at_the_last_row() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog(
                "public",
                &[
                    (DbObjectKind::Table, "users"),
                    (DbObjectKind::Table, "events"),
                ],
            )],
            vec![
                preview(&["id"], &[&["1"]]),
                preview(&["event_id"], &[&["evt_1"]]),
            ],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;

    workspace.apply_action(WorkspaceAction::NextItem)?;
    drain_until_idle(&mut workspace)?;
    assert_eq!(workspace.selected_row().label, "events");

    workspace.apply_action(WorkspaceAction::NextItem)?;

    assert_eq!(workspace.selected_row().label, "events");
    Ok(())
}

#[test]
fn workspace_tree_shows_database_nodes_for_multi_database_connections() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
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
                                name: "user_sessions".to_string(),
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
                                name: "user_events".to_string(),
                                kind: DbObjectKind::View,
                            }],
                        }],
                    },
                ],
            }],
            vec![preview(&["id"], &[&["1"]])],
            vec![],
            vec![],
        )),
    }];

    let workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let labels = workspace
        .tree_rows()
        .iter()
        .map(|row| row.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"app"));
    assert!(labels.contains(&"analytics"));
    assert!(labels.contains(&"user_sessions"));
    Ok(())
}

#[test]
fn workspace_builds_insert_update_and_delete_templates_for_selected_table() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
            vec![
                columns(&[
                    ("id", "integer", false, true, true),
                    ("email", "text", false, false, false),
                    ("display_name", "text", true, false, false),
                ]),
                columns(&[
                    ("id", "integer", false, true, true),
                    ("email", "text", false, false, false),
                    ("display_name", "text", true, false, false),
                ]),
                columns(&[
                    ("id", "integer", false, true, true),
                    ("email", "text", false, false, false),
                    ("display_name", "text", true, false, false),
                ]),
            ],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::OpenInsertTemplate)?;
    drain_until_idle(&mut workspace)?;
    let insert = workspace.editor_snapshot().expect("editor should be open");
    assert!(insert.sql.contains("INSERT INTO \"public\".\"users\""));
    assert!(insert.sql.contains("\"email\""));

    workspace.apply_action(WorkspaceAction::OpenUpdateTemplate)?;
    drain_until_idle(&mut workspace)?;
    let update = workspace
        .editor_snapshot()
        .expect("editor should remain open");
    assert!(update.sql.contains("UPDATE \"public\".\"users\""));
    assert!(update.sql.contains("WHERE \"id\" ="));

    workspace.apply_action(WorkspaceAction::OpenDeleteTemplate)?;
    drain_until_idle(&mut workspace)?;
    let delete = workspace
        .editor_snapshot()
        .expect("editor should remain open");
    assert!(delete.sql.contains("DELETE FROM \"public\".\"users\""));
    assert!(delete.sql.contains("WHERE \"id\" ="));
    Ok(())
}

#[test]
fn workspace_executes_sql_from_editor_and_shows_query_results() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
            vec![],
            vec![query_batch(vec![query(
                &["id", "email"],
                &[&["2", "bob@example.com"]],
            )])],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::OpenSqlEditor)?;
    workspace.set_editor_sql("select 2 as id, 'bob@example.com' as email")?;
    workspace.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_idle(&mut workspace)?;

    assert_eq!(workspace.active_grid().columns, vec!["id", "email"]);
    assert_eq!(workspace.active_grid().rows[0][1], "bob@example.com");
    assert!(
        workspace
            .editor_status()
            .expect("editor status should exist")
            .contains("1 row")
    );
    Ok(())
}

#[test]
fn workspace_requires_confirmation_before_executing_delete_sql() -> Result<()> {
    let executed_sql = Arc::new(Mutex::new(Vec::new()));
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
                vec![],
                vec![query_batch(vec![query(&["rows_affected"], &[&["1"]])])],
            )
            .with_sql_recorder(executed_sql.clone()),
        ),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::OpenSqlEditor)?;
    workspace.set_editor_sql("DELETE FROM users WHERE id = 1;")?;

    workspace.apply_action(WorkspaceAction::ExecuteEditor)?;
    assert!(
        workspace.view().delete_confirmation.is_some(),
        "DELETE should require confirmation before execution"
    );
    assert!(
        executed_sql
            .lock()
            .expect("sql recorder lock should be available")
            .is_empty(),
        "DELETE must not execute until the user confirms"
    );

    workspace.apply_action(WorkspaceAction::CancelDeleteOperation)?;
    assert!(workspace.view().delete_confirmation.is_none());
    assert!(
        executed_sql
            .lock()
            .expect("sql recorder lock should be available")
            .is_empty()
    );

    workspace.apply_action(WorkspaceAction::ExecuteEditor)?;
    workspace.apply_action(WorkspaceAction::ConfirmDeleteOperation)?;
    drain_until_idle(&mut workspace)?;

    assert_eq!(
        executed_sql
            .lock()
            .expect("sql recorder lock should be available")
            .as_slice(),
        &["DELETE FROM users WHERE id = 1;"]
    );
    Ok(())
}

#[test]
fn workspace_executes_only_the_statement_under_the_editor_cursor() -> Result<()> {
    let executed_sql = Arc::new(Mutex::new(Vec::new()));
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
                vec![],
                vec![query_batch(vec![query(&["answer"], &[&["2"]])])],
            )
            .with_sql_recorder(executed_sql.clone()),
        ),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::OpenSqlEditor)?;
    workspace.set_editor_sql("select 1;\nselect 2;\nselect 3;")?;
    workspace.move_editor_cursor_up()?;
    workspace.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_idle(&mut workspace)?;

    assert_eq!(
        executed_sql
            .lock()
            .expect("sql recorder lock should be available")
            .as_slice(),
        &["select 2;"]
    );
    assert_eq!(workspace.active_grid().rows[0][0], "2");
    Ok(())
}

#[test]
fn workspace_searches_and_reruns_sql_history() -> Result<()> {
    let executed_sql = Arc::new(Mutex::new(Vec::new()));
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
                vec![],
                vec![
                    query_batch(vec![query(&["answer"], &[&["1"]])]),
                    query_batch(vec![query(&["answer"], &[&["2"]])]),
                    query_batch(vec![query(&["answer"], &[&["2"]])]),
                ],
            )
            .with_sql_recorder(executed_sql.clone()),
        ),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::OpenSqlEditor)?;
    workspace.set_editor_sql("select 1;")?;
    workspace.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_idle(&mut workspace)?;
    workspace.set_editor_sql("select 2;")?;
    workspace.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_idle(&mut workspace)?;

    workspace.apply_action(WorkspaceAction::OpenSqlHistory)?;
    workspace.insert_sql_history_search_char('2')?;
    let history = workspace
        .view()
        .sql_history
        .expect("history overlay should be open");
    assert_eq!(history.items[0], "select 2;");

    workspace.apply_action(WorkspaceAction::RunSqlHistorySelection)?;
    drain_until_idle(&mut workspace)?;

    let recorded = executed_sql
        .lock()
        .expect("sql recorder lock should be available");
    assert_eq!(recorded.last().map(String::as_str), Some("select 2;"));
    Ok(())
}

#[test]
fn workspace_surfaces_sql_editor_completions_and_accepts_the_selected_item() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id", "email"], &[&["1", "alice@example.com"]])],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::OpenSqlEditor)?;
    workspace.set_editor_sql("sel")?;

    let completion = workspace
        .view()
        .editor_completion
        .expect("completion popup should be visible for SQL keywords");
    assert_eq!(completion.items[0].label, "SELECT");

    workspace.apply_action(WorkspaceAction::AcceptEditorCompletion)?;
    assert_eq!(
        workspace
            .editor_snapshot()
            .expect("editor should remain open")
            .sql,
        "SELECT"
    );
    assert!(
        workspace.view().editor_completion.is_none(),
        "accepting a completion should dismiss the popup"
    );

    workspace.set_editor_sql("ema")?;
    let completion = workspace
        .view()
        .editor_completion
        .expect("completion popup should be visible for preview columns");
    assert!(completion.items.iter().any(|item| item.label == "email"));
    Ok(())
}

#[test]
fn workspace_scopes_object_completions_to_the_active_database() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
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
                                name: "user_sessions".to_string(),
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
                                name: "user_events".to_string(),
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
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let analytics_index = workspace
        .tree_rows()
        .iter()
        .position(|row| row.label == "analytics")
        .expect("analytics row should exist");
    workspace.select_tree_row_index(analytics_index)?;
    workspace.open_selected_tree_item_default()?;
    let mart_index = workspace
        .tree_rows()
        .iter()
        .position(|row| row.label == "mart")
        .expect("mart row should exist");
    workspace.select_tree_row_index(mart_index)?;
    workspace.open_selected_tree_item_default()?;
    let views_index = workspace
        .tree_rows()
        .iter()
        .position(|row| row.label == "Views")
        .expect("views row should exist");
    workspace.select_tree_row_index(views_index)?;
    workspace.open_selected_tree_item_default()?;
    let events_index = workspace
        .tree_rows()
        .iter()
        .position(|row| row.label == "user_events")
        .expect("user_events row should exist after expanding analytics");
    workspace.select_tree_row_index(events_index)?;
    drain_until_idle(&mut workspace)?;

    workspace.apply_action(WorkspaceAction::OpenSqlEditor)?;
    workspace.set_editor_sql("use")?;

    let completion = workspace
        .view()
        .editor_completion
        .expect("completion popup should be visible for database-scoped objects");
    assert!(
        completion
            .items
            .iter()
            .any(|item| item.label == "user_events")
    );
    assert!(
        !completion
            .items
            .iter()
            .any(|item| item.label == "user_sessions")
    );
    Ok(())
}

#[test]
fn workspace_explains_the_current_sql_statement() -> Result<()> {
    let executed_sql = Arc::new(Mutex::new(Vec::new()));
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
                vec![],
                vec![
                    query_batch(vec![query(&["QUERY PLAN"], &[&["Seq Scan on users"]])]),
                    query_batch(vec![query(&["QUERY PLAN"], &[&["Actual Total Time"]])]),
                ],
            )
            .with_sql_recorder(executed_sql.clone()),
        ),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::OpenSqlEditor)?;
    workspace.set_editor_sql("select 1;\nselect * from users;")?;
    workspace.apply_action(WorkspaceAction::ExplainCurrentStatement)?;
    drain_until_idle(&mut workspace)?;
    workspace.apply_action(WorkspaceAction::ExplainAnalyzeCurrentStatement)?;
    drain_until_idle(&mut workspace)?;

    let recorded = executed_sql
        .lock()
        .expect("sql recorder lock should be available");
    assert_eq!(recorded[0], "EXPLAIN select * from users;");
    assert_eq!(recorded[1], "EXPLAIN ANALYZE select * from users;");
    Ok(())
}

#[test]
fn workspace_exposes_right_pane_tabs_and_opens_sql_editor_as_a_tab() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;

    let view = workspace.view();
    assert_eq!(view.active_right_tab, RightPaneTab::Data);
    assert_eq!(view.right_tabs[0].title, "Data");
    assert!(view.right_tabs[0].active);
    assert_eq!(view.right_tabs[1].title, "SQL");

    workspace.apply_action(WorkspaceAction::OpenSqlEditor)?;
    let view = workspace.view();
    assert_eq!(view.active_right_tab, RightPaneTab::Sql);
    assert!(view.right_tabs[1].active);
    assert!(view.editor.is_some());

    workspace.apply_action(WorkspaceAction::SelectRightDataTab)?;
    let view = workspace.view();
    assert_eq!(view.active_right_tab, RightPaneTab::Data);
    assert!(view.right_tabs[0].active);
    assert_eq!(view.active_grid.columns, vec!["id"]);

    workspace.apply_action(WorkspaceAction::SelectRightSqlTab)?;
    assert_eq!(workspace.view().active_right_tab, RightPaneTab::Sql);
    Ok(())
}

#[test]
fn workspace_selecting_sql_tab_opens_an_editable_sql_editor_when_needed() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    assert!(!workspace.is_editor_open());

    workspace.apply_action(WorkspaceAction::SelectRightSqlTab)?;

    assert_eq!(workspace.view().active_right_tab, RightPaneTab::Sql);
    assert!(workspace.is_editor_open());
    assert!(
        workspace
            .editor_snapshot()
            .expect("SQL editor should be editable")
            .sql
            .contains("SELECT")
    );

    workspace.insert_editor_char(' ')?;
    assert!(
        workspace
            .editor_snapshot()
            .expect("SQL editor should still be editable")
            .sql
            .ends_with(' ')
    );
    Ok(())
}

#[test]
fn workspace_loads_structure_columns_for_selected_table() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
            vec![columns(&[
                ("id", "integer", false, true, true),
                ("email", "text", false, false, false),
                ("display_name", "text", true, false, false),
            ])],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;

    workspace.apply_action(WorkspaceAction::SelectRightStructureTab)?;
    assert!(workspace.has_pending_tasks());
    drain_until_idle(&mut workspace)?;

    let view = workspace.view();
    assert_eq!(view.active_right_tab, RightPaneTab::Structure);
    assert_eq!(view.right_tabs[2].title, "Structure");

    let structure = view.structure.expect("structure view should be available");
    assert_eq!(structure.object.unwrap().qualified_name(), "public.users");
    assert!(!structure.loading);
    assert_eq!(structure.columns.len(), 3);
    assert_eq!(structure.columns[0].name, "id");
    assert_eq!(structure.columns[0].data_type, "integer");
    assert!(structure.columns[0].is_primary_key);
    assert_eq!(structure.columns[2].name, "display_name");
    assert!(structure.columns[2].nullable);
    Ok(())
}

#[test]
fn workspace_scrolls_structure_columns_without_moving_asset_selection() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
            vec![columns(&[
                ("id", "integer", false, true, true),
                ("email", "text", false, false, false),
                ("display_name", "text", true, false, false),
                ("created_at", "timestamp", false, true, false),
            ])],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::SelectRightStructureTab)?;
    drain_until_idle(&mut workspace)?;
    let selected_row = workspace.selected_row_index();

    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridDown)?;

    assert_eq!(workspace.selected_row_index(), selected_row);
    assert_eq!(workspace.grid_scroll_offset(), 1);
    assert_eq!(workspace.active_grid().rows[1][1], "email");
    Ok(())
}

#[test]
fn workspace_filters_data_tab_with_a_safe_background_preview_request() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(
                    &["id", "email"],
                    &[&["1", "alice@example.com"], &["2", "bob@example.com"]],
                )],
                vec![],
                vec![],
            )
            .with_filtered_previews(vec![preview(
                &["id", "email"],
                &[&["2", "bob@example.com"]],
            )]),
        ),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::OpenDataFilter)?;
    workspace.insert_data_filter_char('b')?;
    workspace.insert_data_filter_char('o')?;
    workspace.insert_data_filter_char('b')?;
    workspace.apply_action(WorkspaceAction::ApplyDataFilter)?;
    assert!(workspace.has_pending_tasks());
    drain_until_idle(&mut workspace)?;

    assert_eq!(
        workspace.active_preview().rows,
        vec![vec!["2", "bob@example.com"]]
    );
    assert_eq!(workspace.active_data_filter(), Some("bob"));
    Ok(())
}

#[test]
fn workspace_copies_current_cell_row_and_where_clause() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["id", "email"],
                &[&["1", "alice@example.com"], &["2", "bob's@example.com"]],
            )],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridDown)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridRight)?;

    workspace.apply_action(WorkspaceAction::CopyCurrentCell)?;
    assert_eq!(workspace.last_copied_text(), Some("bob's@example.com"));

    workspace.apply_action(WorkspaceAction::CopyCurrentRow)?;
    assert_eq!(workspace.last_copied_text(), Some("2\tbob's@example.com"));

    workspace.apply_action(WorkspaceAction::CopyCurrentWhereClause)?;
    assert_eq!(
        workspace.last_copied_text(),
        Some("\"id\" = '2' AND \"email\" = 'bob''s@example.com'")
    );
    Ok(())
}

#[test]
fn workspace_stages_cell_update_preview_sql_then_commits_transaction() -> Result<()> {
    let executed_sql = Arc::new(Mutex::new(Vec::new()));
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id", "email"], &[&["1", "alice@example.com"]])],
                vec![],
                vec![query_batch(vec![query(
                    &["id", "email"],
                    &[&["1", "new@example.com"]],
                )])],
            )
            .with_sql_recorder(executed_sql.clone()),
        ),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridRight)?;
    workspace.apply_action(WorkspaceAction::StartCellEdit)?;
    workspace.clear_cell_edit_input()?;
    for ch in "new@example.com".chars() {
        workspace.insert_cell_edit_char(ch)?;
    }
    workspace.apply_action(WorkspaceAction::PreviewStagedCrud)?;

    let staged = workspace
        .view()
        .staged_crud
        .expect("staged CRUD preview should be available");
    assert!(staged.preview_sql.contains("ROLLBACK;"));
    assert!(staged.commit_sql.contains("COMMIT;"));
    assert!(
        workspace
            .editor_snapshot()
            .expect("preview SQL should open in the SQL editor")
            .sql
            .contains("UPDATE \"public\".\"users\"")
    );

    workspace.apply_action(WorkspaceAction::CommitStagedCrud)?;
    drain_until_idle(&mut workspace)?;

    let recorded = executed_sql
        .lock()
        .expect("sql recorder lock should be available");
    assert!(recorded[0].contains("BEGIN;"));
    assert!(recorded[0].contains("COMMIT;"));
    assert!(recorded[0].contains("\"email\" = 'new@example.com'"));
    Ok(())
}

#[test]
fn workspace_scrolls_data_grid_without_moving_asset_selection() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"], &["2"], &["3"], &["4"], &["5"]])],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let selected_row = workspace.selected_row_index();

    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridDown)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridDown)?;

    assert_eq!(workspace.selected_row_index(), selected_row);
    assert_eq!(workspace.grid_scroll_offset(), 2);
    assert_eq!(workspace.view().grid_selected_row_index, 2);

    workspace.apply_action(WorkspaceAction::ScrollDataGridUp)?;

    assert_eq!(workspace.selected_row_index(), selected_row);
    assert_eq!(workspace.grid_scroll_offset(), 1);
    assert_eq!(workspace.view().grid_selected_row_index, 1);
    Ok(())
}

#[test]
fn workspace_scrolls_data_grid_columns_without_moving_asset_selection() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["id", "email", "display_name", "created_at"],
                &[&["1", "alice@example.com", "Alice", "2026-04-19"]],
            )],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let selected_row = workspace.selected_row_index();

    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridRight)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridRight)?;

    assert_eq!(workspace.selected_row_index(), selected_row);
    assert_eq!(workspace.grid_column_offset(), 2);

    workspace.apply_action(WorkspaceAction::ScrollDataGridLeft)?;

    assert_eq!(workspace.selected_row_index(), selected_row);
    assert_eq!(workspace.grid_column_offset(), 1);
    Ok(())
}

#[test]
fn workspace_can_resize_and_reset_the_selected_grid_column_width() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["id", "email", "display_name"],
                &[&["1", "alice@example.com", "Alice"]],
            )],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridRight)?;

    assert_eq!(workspace.grid_selected_column_index(), 1);
    assert_eq!(workspace.selected_grid_column_width_override(), None);

    workspace.apply_action(WorkspaceAction::ExpandSelectedGridColumn)?;
    let expanded_width = workspace
        .selected_grid_column_width_override()
        .expect("selected column should have a width override after expanding");
    assert!(expanded_width > 8);

    workspace.apply_action(WorkspaceAction::ShrinkSelectedGridColumn)?;
    let shrunk_width = workspace
        .selected_grid_column_width_override()
        .expect("selected column should still have an override after shrinking");
    assert!(shrunk_width < expanded_width);

    workspace.apply_action(WorkspaceAction::ResetSelectedGridColumnWidth)?;
    assert_eq!(workspace.selected_grid_column_width_override(), None);
    Ok(())
}

#[test]
fn workspace_can_freeze_columns_through_the_current_selection_and_clear_them() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["id", "email", "display_name", "created_at"],
                &[&["1", "alice@example.com", "Alice", "2026-04-19"]],
            )],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridRight)?;
    workspace.apply_action(WorkspaceAction::FreezeGridColumnsThroughSelection)?;

    assert_eq!(workspace.frozen_grid_column_count(), 2);

    workspace.apply_action(WorkspaceAction::ClearFrozenGridColumns)?;
    assert_eq!(workspace.frozen_grid_column_count(), 0);
    Ok(())
}

#[test]
fn workspace_treats_blank_status_as_absent_and_surfaces_feedback_messages() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id", "email"], &[&["1", "alice@example.com"]])],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    assert!(
        workspace
            .selected_session_status()
            .expect("bootstrap status should exist")
            .contains("Browsing Table")
    );

    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::CopyCurrentCell)?;

    assert_eq!(
        workspace.selected_session_status(),
        Some("Copied current cell.")
    );
    Ok(())
}

#[test]
fn workspace_command_palette_filters_and_executes_selected_command() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;

    workspace.apply_action(WorkspaceAction::OpenCommandPalette)?;
    workspace.insert_command_palette_char('s')?;
    workspace.insert_command_palette_char('q')?;
    workspace.insert_command_palette_char('l')?;

    let items = workspace
        .command_palette_items()
        .expect("command palette should be open");
    assert_eq!(items[0].title, "Open SQL Editor");

    workspace.apply_action(WorkspaceAction::ExecuteCommandPaletteSelection)?;

    assert!(!workspace.command_palette_open());
    assert!(workspace.is_editor_open());
    Ok(())
}

#[test]
fn workspace_opens_row_inspector_for_current_grid_row() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["id", "email", "bio"],
                &[
                    &["1", "alice@example.com", "short bio"],
                    &["2", "bob@example.com", "longer biography"],
                ],
            )],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::ScrollDataGridDown)?;
    workspace.apply_action(WorkspaceAction::OpenRowInspector)?;

    let view = workspace.view();
    let inspector = view
        .row_inspector
        .expect("row inspector should be open for the current row");

    assert_eq!(inspector.row_index, 1);
    assert_eq!(inspector.columns[1], "email");
    assert_eq!(inspector.values[1], "bob@example.com");
    assert_eq!(inspector.selected_field, 0);

    workspace.apply_action(WorkspaceAction::NextRowInspectorField)?;
    assert_eq!(workspace.view().row_inspector.unwrap().selected_field, 1);

    workspace.apply_action(WorkspaceAction::CloseRowInspector)?;
    assert!(workspace.view().row_inspector.is_none());
    Ok(())
}

#[test]
fn workspace_scrolls_row_inspector_detail_and_resets_on_field_change() -> Result<()> {
    let detail_value = (1..=24)
        .map(|index| format!("marker-{index:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["payload", "note"],
                &[&[detail_value.as_str(), "short"]],
            )],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::OpenRowInspector)?;
    workspace.apply_action(WorkspaceAction::PageRowInspectorDetailDown)?;

    assert_eq!(
        workspace
            .view()
            .row_inspector
            .expect("row inspector should remain open")
            .detail_scroll,
        10
    );

    workspace.apply_action(WorkspaceAction::NextRowInspectorField)?;
    let inspector = workspace
        .view()
        .row_inspector
        .expect("row inspector should remain open after changing field");
    assert_eq!(inspector.selected_field, 1);
    assert_eq!(inspector.detail_scroll, 0);
    Ok(())
}

#[test]
fn workspace_pages_preview_rows_forward_and_back() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![
                preview(&["id"], &[&["1"], &["2"]]),
                preview(&["id"], &[&["3"], &["4"]]),
                preview(&["id"], &[&["1"], &["2"]]),
            ],
            vec![],
            vec![],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 2)?;
    assert_eq!(workspace.active_preview().rows[0][0], "1");
    assert_eq!(workspace.preview_page_offset(), 0);

    workspace.apply_action(WorkspaceAction::FocusDataGrid)?;
    workspace.apply_action(WorkspaceAction::NextPreviewPage)?;
    drain_until_idle(&mut workspace)?;
    assert_eq!(workspace.active_preview().rows[0][0], "3");
    assert_eq!(workspace.preview_page_offset(), 2);

    workspace.apply_action(WorkspaceAction::PreviousPreviewPage)?;
    drain_until_idle(&mut workspace)?;
    assert_eq!(workspace.active_preview().rows[0][0], "1");
    assert_eq!(workspace.preview_page_offset(), 0);
    Ok(())
}

#[test]
fn workspace_loads_object_preview_in_background_and_applies_it_on_drain() -> Result<()> {
    let (unblock_preview_tx, unblock_preview_rx) = mpsc::channel();
    let bootstraps = vec![
        ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
                vec![],
                vec![],
            )),
        },
        ConnectionBootstrap {
            name: "analytics".to_string(),
            driver: Box::new(BlockingPreviewDriver::new(
                vec![catalog("mart", &[(DbObjectKind::Table, "events")])],
                vec![
                    preview(&["event_id"], &[&["evt_0"]]),
                    preview(&["event_id"], &[&["evt_1"]]),
                ],
                unblock_preview_rx,
            )),
        },
    ];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    for _ in 0..4 {
        workspace.apply_action(WorkspaceAction::NextItem)?;
    }

    assert_eq!(workspace.selected_row().label, "events");
    assert!(workspace.has_pending_tasks());
    assert!(workspace.active_preview().columns.is_empty());
    assert!(
        workspace
            .selected_session_status()
            .expect("status should be present")
            .contains("Loading preview")
    );
    assert_eq!(workspace.drain_background()?, 0);

    unblock_preview_tx
        .send(())
        .expect("preview worker should still be waiting");

    for _ in 0..50 {
        if workspace.drain_background()? > 0 {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(!workspace.has_pending_tasks());
    assert_eq!(workspace.active_preview().columns, vec!["event_id"]);
    assert_eq!(workspace.active_preview().rows[0][0], "evt_1");
    Ok(())
}

#[test]
fn workspace_can_cancel_selected_connection_tasks_and_ignore_late_preview() -> Result<()> {
    let (unblock_preview_tx, unblock_preview_rx) = mpsc::channel();
    let bootstraps = vec![
        ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
                vec![],
                vec![],
            )),
        },
        ConnectionBootstrap {
            name: "analytics".to_string(),
            driver: Box::new(BlockingPreviewDriver::new(
                vec![catalog("mart", &[(DbObjectKind::Table, "events")])],
                vec![
                    preview(&["event_id"], &[&["evt_0"]]),
                    preview(&["event_id"], &[&["evt_1"]]),
                ],
                unblock_preview_rx,
            )),
        },
    ];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    for _ in 0..4 {
        workspace.apply_action(WorkspaceAction::NextItem)?;
    }

    assert!(workspace.has_pending_tasks());
    workspace.apply_action(WorkspaceAction::CancelTasks)?;
    assert!(!workspace.has_pending_tasks());
    assert!(
        workspace
            .selected_session_status()
            .expect("status should exist")
            .contains("Canceled")
    );

    unblock_preview_tx
        .send(())
        .expect("preview worker should still be waiting");

    for _ in 0..50 {
        workspace.drain_background()?;
        thread::sleep(Duration::from_millis(10));
    }

    assert!(workspace.active_preview().columns.is_empty());
    Ok(())
}

#[test]
fn workspace_supports_multiple_editor_tabs_and_multiple_result_sets() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
            vec![],
            vec![
                query_batch(vec![
                    query(&["id"], &[&["1"]]),
                    query(&["email"], &[&["alice@example.com"]]),
                ]),
                query_batch(vec![query(&["status"], &[&["ok"]])]),
            ],
        )),
    }];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)?;
    workspace.apply_action(WorkspaceAction::OpenSqlEditor)?;
    workspace.set_editor_sql("select 1; select 'alice@example.com';")?;
    workspace.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_idle(&mut workspace)?;

    assert_eq!(workspace.editor_tab_count(), 1);
    assert_eq!(workspace.editor_result_set_count(), 2);
    assert_eq!(workspace.active_grid().columns, vec!["id"]);

    workspace.apply_action(WorkspaceAction::NextResultSet)?;
    assert_eq!(workspace.active_grid().columns, vec!["email"]);

    workspace.apply_action(WorkspaceAction::NewEditorTab)?;
    workspace.set_editor_sql("select 'ok' as status;")?;
    workspace.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_idle(&mut workspace)?;

    assert_eq!(workspace.editor_tab_count(), 2);
    assert_eq!(workspace.active_editor_tab_title(), Some("SQL Tab 2"));
    assert_eq!(workspace.active_grid().columns, vec!["status"]);

    workspace.apply_action(WorkspaceAction::PreviousEditorTab)?;
    assert_eq!(
        workspace.active_editor_tab_title(),
        Some("SQL Editor (postgres.public.users)")
    );
    assert_eq!(workspace.editor_result_set_count(), 2);
    Ok(())
}
