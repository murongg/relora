use super::*;
use std::{collections::VecDeque, fs, path::PathBuf, thread, time::Duration};

use anyhow::Result;
use ratatui::backend::TestBackend;
use relora_app::workspace::SavedSqlEntry;
use relora_core::db::{
    Catalog, CatalogSummary, DatabaseDriver, DatabaseEntry, DatabaseKind, DbColumn, DbObjectKind,
    DbObjectRef, QueryResult, SchemaEntry, SqlExecutionResult, TablePreview,
};

#[derive(Debug)]
struct MockDriver {
    catalogs: VecDeque<Catalog>,
    previews: VecDeque<TablePreview>,
    columns: VecDeque<Vec<DbColumn>>,
    executions: VecDeque<Vec<SqlExecutionResult>>,
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
            columns: VecDeque::from(columns),
            executions: VecDeque::from(executions),
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

    fn load_catalog_summary(&mut self) -> Result<CatalogSummary> {
        self.catalogs
            .front()
            .cloned()
            .map(CatalogSummary::from)
            .ok_or_else(|| anyhow::anyhow!("missing mocked catalog"))
    }

    fn load_schema_objects(&mut self, database: &str, schema: &str) -> Result<Vec<DbObjectRef>> {
        self.catalogs
            .front()
            .and_then(|catalog| {
                catalog
                    .databases
                    .iter()
                    .find(|entry| entry.name == database)
                    .and_then(|entry| entry.schemas.iter().find(|entry| entry.name == schema))
                    .map(|entry| entry.objects.clone())
            })
            .ok_or_else(|| anyhow::anyhow!("missing mocked schema objects for {database}.{schema}"))
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
        self.columns
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("missing mocked columns"))
    }

    fn execute_sql(
        &mut self,
        _database: Option<&str>,
        _sql: &str,
    ) -> Result<Vec<SqlExecutionResult>> {
        self.executions
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("missing mocked execution"))
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

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("tui")
        .join("snapshots")
}

fn snapshot_path(name: &str) -> PathBuf {
    snapshot_dir().join(format!("{name}.snap"))
}

fn render_app_shell(shell: &AppShell, width: u16, height: u16) -> Result<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| draw(frame, shell))?;

    Ok(rendered_buffer(terminal.backend().buffer()))
}

fn render_row_inspector_view(
    view: RowInspectorView<'_>,
    width: u16,
    height: u16,
) -> Result<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| draw_row_inspector(frame, frame.area(), view))?;

    Ok(rendered_buffer(terminal.backend().buffer()))
}

fn render_help_overlay_view(width: u16, height: u16) -> Result<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| draw_help_overlay_static(frame, frame.area()))?;

    Ok(rendered_buffer(terminal.backend().buffer()))
}

fn rendered_buffer(buffer: &ratatui::buffer::Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_snapshot_text(input: &str) -> String {
    input.replace("\r\n", "\n")
}

fn assert_matches_snapshot(name: &str, actual: &str) -> Result<()> {
    let path = snapshot_path(name);
    let actual = normalize_snapshot_text(actual);
    if std::env::var_os("RELORA_UPDATE_TUI_SNAPSHOTS").is_some() {
        fs::create_dir_all(snapshot_dir())?;
        fs::write(&path, &actual)?;
    }

    let expected = fs::read_to_string(&path).map_err(|error| {
        anyhow::anyhow!(
            "missing snapshot {} ({error}). Re-run with RELORA_UPDATE_TUI_SNAPSHOTS=1 to create it",
            path.display()
        )
    })?;
    let expected = normalize_snapshot_text(&expected);
    assert_eq!(expected, actual, "snapshot mismatch for {}", path.display());
    Ok(())
}

fn drain_until_sql_results(app: &mut WorkspaceApp) -> Result<()> {
    for _ in 0..20 {
        app.drain_background()?;
        if app.sql_results_available() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(anyhow::anyhow!(
        "sql results did not become visible in time"
    ))
}

fn drain_until_structure_loaded(app: &mut WorkspaceApp) -> Result<()> {
    for _ in 0..20 {
        app.drain_background()?;
        if let Some(structure) = app.view().structure {
            if !structure.loading && !structure.columns.is_empty() {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(anyhow::anyhow!(
        "structure columns did not become visible in time"
    ))
}

fn launcher_snapshot_shell() -> AppShell {
    AppShell::Launcher(Box::new(LauncherApp::new(
        vec![
            crate::config::ConnectionConfig {
                name: "pg".to_string(),
                url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
                read_only: false,
            },
            crate::config::ConnectionConfig {
                name: "mysql".to_string(),
                url: "mysql://root:secret@localhost/mysql".to_string(),
                read_only: false,
            },
        ],
        std::env::temp_dir().join("relora-launcher-golden-snapshot.json"),
    )))
}

fn data_tab_snapshot_shell() -> Result<AppShell> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["id", "email", "status"],
                &[
                    &["1", "alice@example.com", "active"],
                    &["2", "bob@example.com", "pending"],
                    &["3", "carol@example.com", "disabled"],
                ],
            )],
            vec![],
            vec![],
        )),
    }];

    Ok(AppShell::Workspace(
        WorkspaceApp::bootstrap(bootstraps, 50)?.into(),
    ))
}

fn sql_tab_snapshot_shell() -> Result<AppShell> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "release_runs")])],
            vec![preview(
                &["id", "state"],
                &[&["1", "running"], &["2", "done"], &["3", "waiting"]],
            )],
            vec![],
            vec![vec![query(
                &["id", "state", "updated_at"],
                &[
                    &["1", "running", "2026-04-21 10:00"],
                    &["2", "done", "2026-04-21 09:58"],
                ],
            )]],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("select id, state, updated_at from release_runs;")?;
    app.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_sql_results(&mut app)?;
    Ok(AppShell::Workspace(app.into()))
}

fn saved_sql_snapshot_shell() -> Result<AppShell> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "release_runs")])],
            vec![preview(
                &["id", "state", "updated_at"],
                &[
                    &["1", "running", "2026-04-21 10:00"],
                    &["2", "done", "2026-04-21 09:58"],
                ],
            )],
            vec![],
            vec![],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.replace_saved_queries(vec![
        SavedSqlEntry {
            name: "Recent failures".to_string(),
            sql: "select * from release_runs where state = 'failed' order by updated_at desc;"
                .to_string(),
            connection_name: Some("pg".to_string()),
            database_name: Some("postgres".to_string()),
            schema_name: Some("public".to_string()),
        },
        SavedSqlEntry {
            name: "Running jobs".to_string(),
            sql: "select id, state from release_runs where state = 'running';".to_string(),
            connection_name: Some("pg".to_string()),
            database_name: Some("postgres".to_string()),
            schema_name: Some("public".to_string()),
        },
    ]);
    app.apply_action(WorkspaceAction::OpenSavedSql)?;

    Ok(AppShell::Workspace(app.into()))
}

fn structure_tab_snapshot_shell() -> Result<AppShell> {
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
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::SelectRightStructureTab)?;
    drain_until_structure_loaded(&mut app)?;
    Ok(AppShell::Workspace(app.into()))
}

fn row_inspector_snapshot_render() -> Result<String> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["id", "email", "profile_json"],
                &[&[
                    "1",
                    "alice@example.com",
                    "{\"role\":\"admin\",\"team\":\"infra\"}",
                ]],
            )],
            vec![],
            vec![],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::FocusDataGrid)?;
    app.apply_action(WorkspaceAction::ScrollDataGridRight)?;
    app.apply_action(WorkspaceAction::OpenRowInspector)?;

    let inspector = app
        .view()
        .row_inspector
        .ok_or_else(|| anyhow::anyhow!("row inspector should be open"))?;
    render_row_inspector_view(inspector, 100, 24)
}

fn help_overlay_snapshot_render() -> Result<String> {
    render_help_overlay_view(110, 28)
}

#[test]
fn launcher_golden_snapshot() -> Result<()> {
    let shell = launcher_snapshot_shell();
    let rendered = render_app_shell(&shell, 120, 32)?;
    assert_matches_snapshot("launcher", &rendered)
}

#[test]
fn data_tab_golden_snapshot() -> Result<()> {
    let shell = data_tab_snapshot_shell()?;
    let rendered = render_app_shell(&shell, 140, 32)?;
    assert_matches_snapshot("workspace_data_tab", &rendered)
}

#[test]
fn sql_tab_golden_snapshot() -> Result<()> {
    let shell = sql_tab_snapshot_shell()?;
    let rendered = render_app_shell(&shell, 140, 32)?;
    assert_matches_snapshot("workspace_sql_tab", &rendered)
}

#[test]
fn saved_sql_golden_snapshot() -> Result<()> {
    let shell = saved_sql_snapshot_shell()?;
    let rendered = render_app_shell(&shell, 120, 30)?;
    assert_matches_snapshot("workspace_saved_sql", &rendered)
}

#[test]
fn structure_tab_golden_snapshot() -> Result<()> {
    let shell = structure_tab_snapshot_shell()?;
    let rendered = render_app_shell(&shell, 140, 32)?;
    assert_matches_snapshot("workspace_structure_tab", &rendered)
}

#[test]
fn row_inspector_golden_snapshot() -> Result<()> {
    let rendered = row_inspector_snapshot_render()?;
    assert_matches_snapshot("workspace_row_inspector", &rendered)
}

#[test]
fn help_overlay_golden_snapshot() -> Result<()> {
    let rendered = help_overlay_snapshot_render()?;
    assert_matches_snapshot("workspace_help_overlay", &rendered)
}

#[test]
fn snapshot_text_normalization_handles_windows_line_endings() {
    assert_eq!(
        normalize_snapshot_text("first\r\nsecond\r\nthird"),
        "first\nsecond\nthird"
    );
}
