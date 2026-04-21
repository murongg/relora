use std::collections::VecDeque;
use std::hint::black_box;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use relora::tui::render_workspace_for_benchmark;
use relora_app::workspace::{ConnectionBootstrap, WorkspaceAction, WorkspaceApp};
use relora_core::db::{
    Catalog, DatabaseDriver, DatabaseEntry, DatabaseKind, DbColumn, DbObjectKind, DbObjectRef,
    QueryResult, SchemaEntry, SqlExecutionResult, TablePreview,
};

#[derive(Debug)]
struct MockDriver {
    catalogs: VecDeque<Catalog>,
    previews: VecDeque<TablePreview>,
    executions: VecDeque<Vec<SqlExecutionResult>>,
}

impl MockDriver {
    fn new(catalogs: Vec<Catalog>, previews: Vec<TablePreview>) -> Self {
        Self {
            catalogs: VecDeque::from(catalogs),
            previews: VecDeque::from(previews),
            executions: VecDeque::new(),
        }
    }

    fn with_executions(mut self, executions: Vec<Vec<SqlExecutionResult>>) -> Self {
        self.executions = VecDeque::from(executions);
        self
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
        self.executions
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("missing mocked execution"))
    }
}

fn render_workspace_data_tab_dense_grid(c: &mut Criterion) {
    c.bench_function("render_workspace_data_tab_dense_grid", |b| {
        b.iter_batched(
            build_dense_grid_workspace,
            |workspace| {
                let mut terminal =
                    Terminal::new(TestBackend::new(220, 64)).expect("terminal should build");
                terminal
                    .draw(|frame| render_workspace_for_benchmark(frame, &workspace))
                    .expect("workspace render should succeed");
                black_box(terminal.backend().buffer().area.width);
            },
            BatchSize::SmallInput,
        );
    });
}

fn render_workspace_assets_expanded_tree(c: &mut Criterion) {
    c.bench_function("render_workspace_assets_expanded_tree", |b| {
        b.iter_batched(
            build_assets_tree_workspace,
            |workspace| {
                let mut terminal =
                    Terminal::new(TestBackend::new(220, 64)).expect("terminal should build");
                terminal
                    .draw(|frame| render_workspace_for_benchmark(frame, &workspace))
                    .expect("workspace render should succeed");
                black_box(terminal.backend().buffer().area.height);
            },
            BatchSize::SmallInput,
        );
    });
}

fn render_workspace_sql_tab_result_grid(c: &mut Criterion) {
    c.bench_function("render_workspace_sql_tab_result_grid", |b| {
        b.iter_batched(
            build_sql_results_workspace,
            |workspace| {
                let mut terminal =
                    Terminal::new(TestBackend::new(220, 64)).expect("terminal should build");
                terminal
                    .draw(|frame| render_workspace_for_benchmark(frame, &workspace))
                    .expect("workspace render should succeed");
                black_box(terminal.backend().buffer().area.height);
            },
            BatchSize::SmallInput,
        );
    });
}

fn render_workspace_row_inspector_long_text(c: &mut Criterion) {
    c.bench_function("render_workspace_row_inspector_long_text", |b| {
        b.iter_batched(
            build_row_inspector_workspace,
            |workspace| {
                let mut terminal =
                    Terminal::new(TestBackend::new(220, 64)).expect("terminal should build");
                terminal
                    .draw(|frame| render_workspace_for_benchmark(frame, &workspace))
                    .expect("workspace render should succeed");
                black_box(terminal.backend().buffer().area.width);
            },
            BatchSize::SmallInput,
        );
    });
}

fn build_dense_grid_workspace() -> WorkspaceApp {
    let mut workspace = WorkspaceApp::bootstrap(
        vec![ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog_with_table("postgres", "public", "activities")],
                vec![build_preview(72, 180, 20)],
            )),
        }],
        100,
    )
    .expect("workspace should bootstrap for dense grid render benchmark");
    workspace
        .apply_action(WorkspaceAction::FocusDataGrid)
        .expect("grid focus should succeed");
    workspace.select_grid_cell(24, 12);
    workspace
}

fn build_assets_tree_workspace() -> WorkspaceApp {
    let mut workspace = WorkspaceApp::bootstrap(
        vec![ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog_with_tables("postgres", 96, 48)],
                vec![build_preview(12, 32, 16)],
            )),
        }],
        100,
    )
    .expect("workspace should bootstrap for assets tree render benchmark");
    expand_all_schema_table_groups(&mut workspace, 96)
        .expect("expanded tree should build for assets benchmark");
    workspace
}

fn build_sql_results_workspace() -> WorkspaceApp {
    let mut workspace = WorkspaceApp::bootstrap(
        vec![ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(
                MockDriver::new(
                    vec![catalog_with_table("postgres", "public", "activities")],
                    vec![build_preview(12, 48, 12)],
                )
                .with_executions(vec![build_query_result_sets(6, 34, 120, 24)]),
            ),
        }],
        100,
    )
    .expect("workspace should bootstrap for sql render benchmark");
    workspace
        .apply_action(WorkspaceAction::OpenSqlEditor)
        .expect("sql editor should open");
    workspace
        .set_editor_sql("select * from activities; select * from release_runs;")
        .expect("sql should be set");
    workspace
        .apply_action(WorkspaceAction::ExecuteEditor)
        .expect("sql execution should schedule");
    drain_until_sql_results_ready(&mut workspace).expect("sql results should become visible");
    workspace
        .apply_action(WorkspaceAction::FocusDataGrid)
        .expect("result grid focus should succeed");
    workspace
}

fn build_row_inspector_workspace() -> WorkspaceApp {
    let long_text = long_text_payload();
    let preview = TablePreview {
        columns: vec![
            "id".to_string(),
            "payload".to_string(),
            "notes".to_string(),
            "metadata".to_string(),
        ],
        rows: vec![vec![
            "evt_001".to_string(),
            long_text.clone(),
            long_text.clone(),
            format!("{{\"payload\": {:?}}}", &long_text[..80]),
        ]],
    };

    let mut workspace = WorkspaceApp::bootstrap(
        vec![ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog_with_table("postgres", "public", "activity_logs")],
                vec![preview],
            )),
        }],
        100,
    )
    .expect("workspace should bootstrap for row inspector render benchmark");
    workspace
        .apply_action(WorkspaceAction::FocusDataGrid)
        .expect("grid focus should succeed");
    workspace.select_grid_cell(0, 1);
    workspace
        .apply_action(WorkspaceAction::OpenRowInspector)
        .expect("row inspector should open");
    workspace
        .apply_action(WorkspaceAction::NextRowInspectorPane)
        .expect("row inspector pane switch should succeed");
    workspace
        .apply_action(WorkspaceAction::PageRowInspectorDetailDown)
        .expect("row inspector scroll should succeed");
    workspace
}

fn drain_until_sql_results_ready(workspace: &mut WorkspaceApp) -> Result<()> {
    for _ in 0..40 {
        workspace.drain_background()?;
        if workspace.editor_result_set_count() > 1 {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(2));
    }

    Err(anyhow::anyhow!(
        "sql result sets did not become visible in time"
    ))
}

fn expand_all_schema_table_groups(workspace: &mut WorkspaceApp, schema_count: usize) -> Result<()> {
    for schema_index in 1..schema_count {
        let schema_label = format!("schema_{schema_index:03}");
        let schema_row = tree_row_index(workspace, &schema_label);
        workspace.select_tree_row_index(schema_row)?;
        workspace.apply_action(WorkspaceAction::ToggleNode)?;

        let group_row = schema_row + 1;
        workspace.select_tree_row_index(group_row)?;
        workspace.apply_action(WorkspaceAction::ToggleNode)?;
    }

    Ok(())
}

fn tree_row_index(workspace: &WorkspaceApp, label: &str) -> usize {
    workspace
        .tree_rows()
        .iter()
        .position(|row| row.label == label)
        .unwrap_or_else(|| panic!("tree row {label} should exist"))
}

fn catalog_with_table(database: &str, schema: &str, table: &str) -> Catalog {
    Catalog {
        databases: vec![DatabaseEntry {
            name: database.to_string(),
            schemas: vec![SchemaEntry {
                database: database.to_string(),
                name: schema.to_string(),
                objects: vec![DbObjectRef {
                    database: database.to_string(),
                    schema: schema.to_string(),
                    name: table.to_string(),
                    kind: DbObjectKind::Table,
                }],
            }],
        }],
    }
}

fn catalog_with_tables(database: &str, schema_count: usize, table_count: usize) -> Catalog {
    Catalog {
        databases: vec![DatabaseEntry {
            name: database.to_string(),
            schemas: (0..schema_count)
                .map(|schema_index| {
                    let schema = format!("schema_{schema_index:03}");
                    SchemaEntry {
                        database: database.to_string(),
                        name: schema.clone(),
                        objects: (0..table_count)
                            .map(|index| DbObjectRef {
                                database: database.to_string(),
                                schema: schema.clone(),
                                name: format!("table_{index:03}"),
                                kind: DbObjectKind::Table,
                            })
                            .collect(),
                    }
                })
                .collect(),
        }],
    }
}

fn build_preview(column_count: usize, row_count: usize, cell_width: usize) -> TablePreview {
    let columns = (0..column_count)
        .map(|column| format!("column_{column:03}"))
        .collect::<Vec<_>>();
    let rows = (0..row_count)
        .map(|row| {
            (0..column_count)
                .map(|column| format!("r{row:03}_c{column:03}_{}", "x".repeat(cell_width)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    TablePreview { columns, rows }
}

fn build_query_result_sets(
    result_set_count: usize,
    column_count: usize,
    row_count: usize,
    cell_width: usize,
) -> Vec<SqlExecutionResult> {
    (0..result_set_count)
        .map(|result_index| {
            SqlExecutionResult::Query(QueryResult {
                columns: (0..column_count)
                    .map(|column| format!("result_{result_index:02}_col_{column:02}"))
                    .collect(),
                rows: (0..row_count)
                    .map(|row| {
                        (0..column_count)
                            .map(|column| {
                                format!(
                                    "set{result_index:02}_r{row:03}_c{column:02}_{}",
                                    "y".repeat(cell_width)
                                )
                            })
                            .collect()
                    })
                    .collect(),
            })
        })
        .collect()
}

fn long_text_payload() -> String {
    (0..96)
        .map(|line| format!("line_{line:03}: {}", "payload".repeat(10)))
        .collect::<Vec<_>>()
        .join("\n")
}

criterion_group!(
    benches,
    render_workspace_assets_expanded_tree,
    render_workspace_data_tab_dense_grid,
    render_workspace_sql_tab_result_grid,
    render_workspace_row_inspector_long_text
);
criterion_main!(benches);
