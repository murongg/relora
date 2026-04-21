use std::collections::VecDeque;
use std::hint::black_box;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
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

#[derive(Debug)]
struct BlockingPreviewDriver {
    catalogs: VecDeque<Catalog>,
    previews: VecDeque<TablePreview>,
    unblock_preview: Option<mpsc::Receiver<()>>,
    preview_calls: usize,
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
        if self.preview_calls > 1 {
            if let Some(receiver) = self.unblock_preview.take() {
                receiver
                    .recv()
                    .map_err(|_| anyhow::anyhow!("preview unblock signal was dropped"))?;
            }
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
        Err(anyhow::anyhow!(
            "sql execution is not used in this benchmark"
        ))
    }
}

fn workspace_bootstrap_large_catalog(c: &mut Criterion) {
    let catalog = build_catalog(12, 16, 18);
    let preview = build_preview(12, 80, 24);

    c.bench_function("workspace_bootstrap_large_catalog", |b| {
        b.iter_batched(
            || {
                (0..3)
                    .map(|index| ConnectionBootstrap {
                        name: format!("pg-{index}"),
                        driver: Box::new(MockDriver::new(
                            vec![catalog.clone()],
                            vec![preview.clone()],
                        )) as Box<dyn DatabaseDriver>,
                    })
                    .collect::<Vec<_>>()
            },
            |bootstraps| {
                let workspace = WorkspaceApp::bootstrap(bootstraps, 100)
                    .expect("workspace should bootstrap for large catalog benchmark");
                black_box(workspace.tree_rows().len());
                black_box(workspace.selected_connection_object_count());
            },
            BatchSize::SmallInput,
        );
    });
}

fn workspace_scroll_wide_preview_columns(c: &mut Criterion) {
    c.bench_function("workspace_scroll_wide_preview_columns", |b| {
        b.iter_batched(
            build_wide_preview_workspace,
            |mut workspace| {
                for _ in 0..64 {
                    workspace
                        .apply_action(WorkspaceAction::ScrollDataGridRight)
                        .expect("scrolling right should succeed");
                }
                for _ in 0..16 {
                    workspace
                        .apply_action(WorkspaceAction::ScrollDataGridLeft)
                        .expect("scrolling left should succeed");
                }
                black_box(workspace.grid_column_offset());
                black_box(workspace.grid_selected_column_index());
            },
            BatchSize::SmallInput,
        );
    });
}

fn workspace_cancel_inflight_preview(c: &mut Criterion) {
    c.bench_function("workspace_cancel_inflight_preview", |b| {
        b.iter_batched(
            build_cancelable_preview_workspace,
            |(mut workspace, unblock_preview_tx)| {
                workspace
                    .apply_action(WorkspaceAction::CancelTasks)
                    .expect("cancel should succeed");
                black_box(workspace.has_pending_tasks());

                unblock_preview_tx
                    .send(())
                    .expect("preview worker should still be waiting");

                for _ in 0..256 {
                    workspace
                        .drain_background()
                        .expect("draining background should succeed");
                    if !workspace.has_pending_tasks() {
                        break;
                    }
                    thread::yield_now();
                }

                black_box(workspace.active_preview().columns.len());
                black_box(workspace.selected_session_status());
            },
            BatchSize::SmallInput,
        );
    });
}

fn workspace_switch_sql_result_sets(c: &mut Criterion) {
    c.bench_function("workspace_switch_sql_result_sets", |b| {
        b.iter_batched(
            build_sql_results_workspace,
            |mut workspace| {
                for _ in 0..40 {
                    workspace
                        .apply_action(WorkspaceAction::NextResultSet)
                        .expect("result set switch should succeed");
                }
                black_box(workspace.active_grid().rows.len());
                black_box(workspace.editor_result_set_count());
            },
            BatchSize::SmallInput,
        );
    });
}

fn build_wide_preview_workspace() -> WorkspaceApp {
    WorkspaceApp::bootstrap(
        vec![ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![build_catalog(1, 1, 1)],
                vec![build_preview(140, 120, 48)],
            )),
        }],
        100,
    )
    .expect("workspace should bootstrap for wide preview benchmark")
}

fn build_cancelable_preview_workspace() -> (WorkspaceApp, mpsc::Sender<()>) {
    let (unblock_preview_tx, unblock_preview_rx) = mpsc::channel();
    let bootstraps = vec![
        ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![build_catalog(1, 1, 1)],
                vec![build_preview(8, 8, 12)],
            )),
        },
        ConnectionBootstrap {
            name: "analytics".to_string(),
            driver: Box::new(BlockingPreviewDriver::new(
                vec![catalog_with_tables("analytics", "mart", 1)],
                vec![
                    preview_from_values(&["event_id"], &[&["evt_0"]]),
                    preview_from_values(&["event_id"], &[&["evt_1"]]),
                ],
                unblock_preview_rx,
            )),
        },
    ];

    let mut workspace = WorkspaceApp::bootstrap(bootstraps, 50)
        .expect("workspace should bootstrap for cancel benchmark");
    for _ in 0..4 {
        workspace
            .apply_action(WorkspaceAction::NextItem)
            .expect("tree navigation should succeed");
    }

    (workspace, unblock_preview_tx)
}

fn build_sql_results_workspace() -> WorkspaceApp {
    let executions = vec![build_query_result_sets(8, 36, 120, 28)];
    let mut workspace = WorkspaceApp::bootstrap(
        vec![ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(
                MockDriver::new(
                    vec![build_catalog(1, 1, 1)],
                    vec![build_preview(12, 40, 20)],
                )
                .with_executions(executions),
            ),
        }],
        100,
    )
    .expect("workspace should bootstrap for sql result benchmark");

    workspace
        .apply_action(WorkspaceAction::OpenSqlEditor)
        .expect("sql editor should open");
    workspace
        .set_editor_sql("select 1; select 2; select 3;")
        .expect("sql should be set");
    workspace
        .apply_action(WorkspaceAction::ExecuteEditor)
        .expect("sql execution should schedule");
    drain_until_result_sets_ready(&mut workspace)
        .expect("sql results should become visible for benchmark");
    workspace
}

fn drain_until_result_sets_ready(workspace: &mut WorkspaceApp) -> Result<()> {
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

fn build_catalog(database_count: usize, schema_count: usize, objects_per_schema: usize) -> Catalog {
    Catalog {
        databases: (0..database_count)
            .map(|database_index| {
                let database_name = format!("db_{database_index:02}");
                DatabaseEntry {
                    name: database_name.clone(),
                    schemas: (0..schema_count)
                        .map(|schema_index| {
                            let schema_name = format!("schema_{schema_index:02}");
                            SchemaEntry {
                                database: database_name.clone(),
                                name: schema_name.clone(),
                                objects: (0..objects_per_schema)
                                    .map(|object_index| DbObjectRef {
                                        database: database_name.clone(),
                                        schema: schema_name.clone(),
                                        name: format!("table_{object_index:03}"),
                                        kind: DbObjectKind::Table,
                                    })
                                    .collect(),
                            }
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

fn catalog_with_tables(database: &str, schema: &str, table_count: usize) -> Catalog {
    Catalog {
        databases: vec![DatabaseEntry {
            name: database.to_string(),
            schemas: vec![SchemaEntry {
                database: database.to_string(),
                name: schema.to_string(),
                objects: (0..table_count)
                    .map(|index| DbObjectRef {
                        database: database.to_string(),
                        schema: schema.to_string(),
                        name: format!("events_{index:02}"),
                        kind: DbObjectKind::Table,
                    })
                    .collect(),
            }],
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

fn preview_from_values(columns: &[&str], rows: &[&[&str]]) -> TablePreview {
    TablePreview {
        columns: columns.iter().map(|value| (*value).to_string()).collect(),
        rows: rows
            .iter()
            .map(|row| row.iter().map(|value| (*value).to_string()).collect())
            .collect(),
    }
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

criterion_group!(
    benches,
    workspace_bootstrap_large_catalog,
    workspace_cancel_inflight_preview,
    workspace_scroll_wide_preview_columns,
    workspace_switch_sql_result_sets
);
criterion_main!(benches);
