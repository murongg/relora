use super::*;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::{thread, time::Duration};

use crate::launcher::{LauncherAction, LauncherApp, LauncherFormField};
use anyhow::Result;
use ratatui::backend::TestBackend;
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

fn query(columns: &[&str], rows: &[&[&str]]) -> Vec<SqlExecutionResult> {
    vec![SqlExecutionResult::Query(QueryResult {
        columns: columns.iter().map(|value| (*value).to_string()).collect(),
        rows: rows
            .iter()
            .map(|row| row.iter().map(|value| (*value).to_string()).collect())
            .collect(),
    })]
}

fn query_batch(items: Vec<Vec<SqlExecutionResult>>) -> Vec<SqlExecutionResult> {
    items.into_iter().flatten().collect()
}

#[test]
fn mysql_object_scope_label_collapses_duplicate_database_schema() {
    let object = DbObjectRef {
        database: "relora_demo".to_string(),
        schema: "relora_demo".to_string(),
        name: "release_runs".to_string(),
        kind: DbObjectKind::Table,
    };

    assert_eq!(
        render::object_scope_label(Some(DatabaseKind::MySql), &object),
        "relora_demo.release_runs"
    );
}

fn drain_until_result_visible(app: &mut WorkspaceApp) -> Result<()> {
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

fn drain_until_preview_columns(app: &mut WorkspaceApp, columns: &[&str]) -> Result<()> {
    let expected = columns
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    for _ in 0..20 {
        app.drain_background()?;
        if app.active_preview().columns == expected {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(anyhow::anyhow!(
        "preview columns did not become visible in time"
    ))
}

fn click_at(app: &mut WorkspaceApp, area: Rect, column: u16, row: u16) -> Result<()> {
    handle_mouse(
        app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )
}

fn tab_click_point(area: Rect, tab_title: &str) -> (u16, u16) {
    let main = workspace_main_sections(workspace_body_area(area));
    let details = workspace_detail_sections(main[1]);
    let tab_area = details[0];
    let mut x = tab_area.x + 1;
    let y = tab_area.y + 1;
    for title in ["Data", "SQL", "Structure"] {
        let width = title.len() as u16 + 2;
        if title == tab_title {
            return (x + width / 2, y);
        }
        x += width + 1;
    }
    panic!("missing tab title: {tab_title}");
}

fn asset_row_click_point(area: Rect, row_index: usize) -> (u16, u16) {
    let main = workspace_main_sections(workspace_body_area(area));
    let assets = main[0];
    (assets.x + 2, assets.y + 1 + row_index as u16)
}

fn grid_cell_click_point(
    app: &WorkspaceApp,
    area: Rect,
    row_delta: u16,
    visible_column_index: usize,
) -> (u16, u16) {
    let grid_area = active_grid_area(area, app).expect("grid area should be available");
    let columns = grid_column_layouts(
        grid_area,
        app.active_grid(),
        &GridViewport {
            selected_row_index: app.grid_selected_row_index(),
            selected_column_index: app.grid_selected_column_index(),
            row_offset: app.grid_scroll_offset(),
            column_offset: app.grid_column_offset(),
            focused: app.data_grid_focused(),
            width_overrides: app
                .current_grid_column_width_overrides()
                .map(|overrides| {
                    overrides
                        .iter()
                        .map(|(index, width)| (*index, *width))
                        .collect()
                })
                .unwrap_or_default(),
            frozen_leading_columns: app.frozen_grid_column_count(),
        },
    );
    let target_index = columns
        .get(visible_column_index)
        .map(|column| column.index)
        .expect("visible grid column should exist");
    let mut x = grid_area.x + 1;
    for column in columns {
        if column.index == target_index {
            return (x + (column.width / 2).max(1), grid_area.y + 2 + row_delta);
        }
        x = x
            .saturating_add(column.width)
            .saturating_add(GRID_COLUMN_SPACING);
    }
    (grid_area.x + 2, grid_area.y + 2 + row_delta)
}

fn editor_tab_click_point(app: &WorkspaceApp, area: Rect, tab_index: usize) -> (u16, u16) {
    let main = workspace_main_sections(workspace_body_area(area));
    let details = workspace_detail_sections(main[1]);
    let editor_area = sql_tab_sections(details[2])[0];
    let strip = app
        .editor_tab_strip()
        .expect("editor tab strip should be available");
    let start_x = editor_area.x + 1 + "Tabs: ".chars().count() as u16;
    (
        bracket_segment_center(strip, start_x, tab_index).expect("editor tab should be clickable"),
        editor_area.y + 1,
    )
}

fn result_set_click_point(app: &WorkspaceApp, area: Rect, result_index: usize) -> (u16, u16) {
    let main = workspace_main_sections(workspace_body_area(area));
    let details = workspace_detail_sections(main[1]);
    let editor_area = sql_tab_sections(details[2])[0];
    let editor = app.view().editor.expect("editor should be visible");
    let strip = editor
        .result_strip
        .expect("result strip should be available for clickable result sets");
    let start_x = editor_area.x + 1 + "Results: ".chars().count() as u16;
    let row = editor_area.y + 1 + usize::from(!editor.tab_strip.is_empty()) as u16;
    (
        bracket_segment_center(strip, start_x, result_index)
            .expect("result set should be clickable"),
        row,
    )
}

fn bracket_segment_center(strip: &str, start_x: u16, target_index: usize) -> Option<u16> {
    let mut x = start_x;
    let mut chars = strip.chars().peekable();
    let mut index = 0;
    while let Some(ch) = chars.next() {
        if ch != '[' {
            x = x.saturating_add(1);
            continue;
        }

        let segment_start = x;
        x = x.saturating_add(1);
        for next in chars.by_ref() {
            x = x.saturating_add(1);
            if next == ']' {
                break;
            }
        }
        if index == target_index {
            return Some(segment_start + (x.saturating_sub(segment_start) / 2));
        }
        index += 1;
    }
    None
}

#[test]
fn editor_control_enter_executes_sql_without_legacy_aliases() {
    assert_eq!(
        map_editor_control_key_to_action(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL)),
        Some(WorkspaceAction::ExecuteEditor)
    );
    for code in [KeyCode::Char('m'), KeyCode::Char('j')] {
        let key = KeyEvent::new(code, KeyModifiers::CONTROL);
        assert_eq!(map_editor_control_key_to_action(key), None);
    }
}

#[test]
fn right_tab_keys_include_structure_tab() {
    assert_eq!(
        map_right_tab_key_to_action(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)),
        Some(WorkspaceAction::SelectRightDataTab)
    );
    assert_eq!(
        map_right_tab_key_to_action(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::ALT)),
        Some(WorkspaceAction::SelectRightSqlTab)
    );
    assert_eq!(
        map_right_tab_key_to_action(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE)),
        Some(WorkspaceAction::SelectRightStructureTab)
    );
    assert_eq!(
        map_right_tab_key_to_action(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::ALT)),
        Some(WorkspaceAction::SelectRightStructureTab)
    );
    assert_eq!(
        map_right_tab_key_to_action(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::CONTROL)),
        None
    );
}

#[test]
fn editor_shortcuts_include_explain_and_staged_commit() {
    assert_eq!(
        map_editor_control_key_to_action(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL)),
        Some(WorkspaceAction::CommitStagedCrud)
    );
}

#[test]
fn data_grid_shortcuts_include_copy_filter_and_edit_actions() {
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
        Some(WorkspaceAction::CopyCurrentRow)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT)),
        Some(WorkspaceAction::CopyCurrentCell)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE)),
        Some(WorkspaceAction::CopyCurrentWhereClause)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
        Some(WorkspaceAction::StartCellEdit)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)),
        None
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
        Some(WorkspaceAction::NextPreviewPage)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT)),
        None
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE)),
        Some(WorkspaceAction::PreviousPreviewPage)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::SHIFT)),
        None
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE)),
        Some(WorkspaceAction::ShrinkSelectedGridColumn)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE)),
        Some(WorkspaceAction::ExpandSelectedGridColumn)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE)),
        Some(WorkspaceAction::ResetSelectedGridColumnWidth)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)),
        Some(WorkspaceAction::FreezeGridColumnsThroughSelection)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE)),
        None
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT)),
        Some(WorkspaceAction::ClearFrozenGridColumns)
    );
    assert_eq!(
        map_data_grid_key_to_action(KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::SHIFT)),
        None
    );
}

#[test]
fn browser_shortcuts_use_crud_mnemonics_for_templates() {
    assert_eq!(
        map_browser_key_to_action(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
        Some(WorkspaceAction::OpenSelectTemplate)
    );
    assert_eq!(
        map_browser_key_to_action(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
        Some(WorkspaceAction::OpenInsertTemplate)
    );
    assert_eq!(
        map_browser_key_to_action(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE)),
        Some(WorkspaceAction::OpenUpdateTemplate)
    );
    assert_eq!(
        map_browser_key_to_action(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)),
        Some(WorkspaceAction::OpenDeleteTemplate)
    );
    assert_eq!(
        map_browser_key_to_action(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
        None
    );
}

#[test]
fn grid_column_layout_freezes_the_first_column_when_horizontally_scrolled() {
    let grid = preview(
        &["id", "email", "status", "city"],
        &[&["1", "alice@example.com", "active", "Shanghai"]],
    );
    let viewport = GridViewport {
        selected_row_index: 0,
        selected_column_index: 2,
        row_offset: 0,
        column_offset: 1,
        focused: true,
        width_overrides: Vec::new(),
        frozen_leading_columns: 0,
    };

    let columns = grid_column_layouts(Rect::new(0, 0, 80, 10), &grid, &viewport);
    let indexes = columns
        .into_iter()
        .map(|column| column.index)
        .collect::<Vec<_>>();

    assert_eq!(indexes.first().copied(), Some(0));
    assert!(indexes.contains(&1));
    assert!(indexes.contains(&2));
}

#[test]
fn grid_column_layout_gives_more_width_to_longer_columns() {
    let grid = preview(&["id", "email"], &[&["1", "alice.long.name@example.com"]]);
    let viewport = GridViewport {
        selected_row_index: 0,
        selected_column_index: 1,
        row_offset: 0,
        column_offset: 0,
        focused: true,
        width_overrides: Vec::new(),
        frozen_leading_columns: 0,
    };

    let columns = grid_column_layouts(Rect::new(0, 0, 60, 10), &grid, &viewport);

    assert_eq!(columns.len(), 2);
    assert!(columns[1].width > columns[0].width);
}

#[test]
fn grid_column_layout_respects_manual_width_overrides() {
    let grid = preview(&["id", "email"], &[&["1", "alice@example.com"]]);
    let viewport = GridViewport {
        selected_row_index: 0,
        selected_column_index: 1,
        row_offset: 0,
        column_offset: 0,
        focused: true,
        width_overrides: vec![(1, 28)],
        frozen_leading_columns: 0,
    };

    let columns = grid_column_layouts(Rect::new(0, 0, 80, 10), &grid, &viewport);

    assert_eq!(columns[1].width, 28);
}

#[test]
fn grid_column_layout_respects_frozen_leading_columns() {
    let grid = preview(
        &["id", "email", "status", "city", "country"],
        &[&["1", "alice@example.com", "active", "Shanghai", "CN"]],
    );
    let viewport = GridViewport {
        selected_row_index: 0,
        selected_column_index: 3,
        row_offset: 0,
        column_offset: 2,
        focused: true,
        width_overrides: Vec::new(),
        frozen_leading_columns: 2,
    };

    let columns = grid_column_layouts(Rect::new(0, 0, 80, 10), &grid, &viewport);
    let indexes = columns
        .into_iter()
        .map(|column| column.index)
        .collect::<Vec<_>>();

    assert_eq!(&indexes[..2], &[0, 1]);
    assert!(!indexes.contains(&2));
    assert!(indexes.contains(&3));
}

#[test]
fn selected_grid_header_uses_contrasting_foreground_when_focused() {
    let style = grid_header_cell_style(true, true);

    assert_eq!(style.fg, Some(TEXT_INVERSE));
    assert_eq!(style.bg, Some(theme_accent_color()));
    assert_ne!(style.fg, style.bg);
}

#[test]
fn focused_grid_uses_distinct_styles_for_selected_row_column_and_cell() {
    let selected_row = grid_body_cell_style(true, true, false);
    let selected_column = grid_body_cell_style(true, false, true);
    let current_cell = grid_body_cell_style(true, true, true);

    assert_ne!(selected_row.bg, selected_column.bg);
    assert_ne!(selected_row.bg, current_cell.bg);
    assert_ne!(selected_column.bg, current_cell.bg);
}

#[test]
fn focused_selected_column_cells_keep_text_visible() {
    let style = grid_body_cell_style(true, false, true);

    assert!(style.bg.is_some());
    assert!(style.fg.is_some());
    assert_ne!(style.fg, style.bg);
}

#[test]
fn footer_renders_status_text_inside_the_status_box() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
        )),
    }];
    let app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Workspace(app.into())))?;

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Browsing Table public.users"));
    Ok(())
}

#[test]
fn workspace_header_renders_compact_context_bar() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "localhost".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"], &["2"], &["3"]])],
        )),
    }];
    let app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let backend = TestBackend::new(140, 30);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Workspace(app.into())))?;

    let buffer = terminal.backend().buffer();
    let header_row = (0..buffer.area.width)
        .map(|x| buffer[(x, 0)].symbol())
        .collect::<String>();

    assert!(header_row.contains("Relora"));
    assert!(header_row.contains("localhost (PostgreSQL)"));
    assert!(header_row.contains("postgres.public.users"));
    assert!(header_row.contains("Data"));
    assert!(header_row.contains("page 1 | rows 1-3 | limit 50"));
    assert!(header_row.contains("Ready"));
    assert!(!header_row.contains("Workspace"));
    Ok(())
}

#[test]
fn summary_renders_selected_database_context_for_multi_database_connections() -> Result<()> {
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
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let analytics_index = app
        .tree_rows()
        .iter()
        .position(|row| row.label == "analytics")
        .expect("analytics row should exist");
    app.select_tree_row_index(analytics_index)?;
    app.open_selected_tree_item_default()?;
    let mart_index = app
        .tree_rows()
        .iter()
        .position(|row| row.label == "mart")
        .expect("mart row should exist");
    app.select_tree_row_index(mart_index)?;
    app.open_selected_tree_item_default()?;
    let views_index = app
        .tree_rows()
        .iter()
        .position(|row| row.label == "Views")
        .expect("views row should exist");
    app.select_tree_row_index(views_index)?;
    app.open_selected_tree_item_default()?;
    let events_index = app
        .tree_rows()
        .iter()
        .position(|row| row.label == "events")
        .expect("events row should exist after expanding analytics");
    app.select_tree_row_index(events_index)?;
    for _ in 0..8 {
        app.drain_background()?;
        if app.active_preview().columns == vec!["event_id"] {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Workspace(app.into())))?;

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Database: analytics"));
    assert!(rendered.contains("Databases: 2"));
    Ok(())
}

#[test]
fn launcher_screen_renders_saved_connections() -> Result<()> {
    let launcher = LauncherApp::new(
        vec![crate::config::ConnectionConfig {
            name: "pg".to_string(),
            url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
        }],
        std::env::temp_dir().join("relora-launcher-render-test.json"),
    );
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Launcher(Box::new(launcher))))?;

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Relora"));
    assert!(rendered.contains("Saved Connections"));
    assert!(rendered.contains("Terminal Database Workspace"));
    assert!(rendered.contains("Launch selected"));
    assert!(rendered.contains("pg"));
    Ok(())
}

#[test]
fn launcher_screen_renders_database_badges_per_connection_kind() -> Result<()> {
    let launcher = LauncherApp::new(
        vec![
            crate::config::ConnectionConfig {
                name: "postgres-main".to_string(),
                url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
            },
            crate::config::ConnectionConfig {
                name: "mysql-main".to_string(),
                url: "mysql://root:secret@localhost/mysql".to_string(),
            },
            crate::config::ConnectionConfig {
                name: "sqlite-main".to_string(),
                url: "sqlite:///tmp/relora.db".to_string(),
            },
        ],
        std::env::temp_dir().join("relora-launcher-badges-test.json"),
    );
    let backend = TestBackend::new(100, 32);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Launcher(Box::new(launcher))))?;

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    let postgres_line = rendered
        .lines()
        .find(|line| line.contains("postgres-main"))
        .expect("postgres row should be rendered");
    let mysql_line = rendered
        .lines()
        .find(|line| line.contains("mysql-main"))
        .expect("mysql row should be rendered");
    let sqlite_line = rendered
        .lines()
        .find(|line| line.contains("sqlite-main"))
        .expect("sqlite row should be rendered");

    assert!(postgres_line.contains("PG"));
    assert!(mysql_line.contains("MY"));
    assert!(sqlite_line.contains("SQ"));
    Ok(())
}

#[test]
fn pending_preview_renders_loading_instead_of_empty_state() -> Result<()> {
    let (unblock_preview_tx, unblock_preview_rx) = mpsc::channel();
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(BlockingPreviewDriver::new(
            vec![catalog(
                "public",
                &[
                    (DbObjectKind::Table, "users"),
                    (DbObjectKind::Table, "orders"),
                ],
            )],
            vec![
                preview(&["id"], &[&["1"]]),
                preview(&["order_id"], &[&["ord_1"]]),
            ],
            unblock_preview_rx,
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let orders_index = app
        .tree_rows()
        .iter()
        .enumerate()
        .find(|(_, row)| row.label == "orders")
        .map(|(index, _)| index)
        .expect("orders row should be visible");
    app.select_tree_row_index(orders_index)?;

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| draw_workspace(frame, &app))?;

    let rendered = (0..terminal.backend().buffer().area.height)
        .map(|y| {
            (0..terminal.backend().buffer().area.width)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Loading preview"));
    assert!(!rendered.contains("No rows available."));

    unblock_preview_tx
        .send(())
        .expect("preview worker should still be waiting");
    Ok(())
}

#[test]
fn launcher_delete_confirmation_renders_as_modal() -> Result<()> {
    let mut launcher = LauncherApp::new(
        vec![crate::config::ConnectionConfig {
            name: "pg".to_string(),
            url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
        }],
        std::env::temp_dir().join("relora-launcher-delete-modal-test.json"),
    );
    launcher.apply_action(LauncherAction::DeleteSelectedConnection)?;
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Launcher(Box::new(launcher))))?;

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Delete Connection"));
    assert!(rendered.contains("Delete pg?"));
    assert!(rendered.contains("Press y to delete"));
    assert!(rendered.contains("saved Relora profile"));
    assert!(rendered.contains("database is not modified"));
    Ok(())
}

#[test]
fn launcher_delete_confirmation_blocks_navigation_until_answered() -> Result<()> {
    let mut app = AppShell::Launcher(Box::new(LauncherApp::new(
        vec![
            crate::config::ConnectionConfig {
                name: "pg".to_string(),
                url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
            },
            crate::config::ConnectionConfig {
                name: "analytics".to_string(),
                url: "postgresql://postgres:postgres@localhost/analytics".to_string(),
            },
        ],
        std::env::temp_dir().join("relora-launcher-delete-input-test.json"),
    )));

    handle_launcher_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty()),
    )?;
    let AppShell::Launcher(launcher) = &app else {
        panic!("launcher shell should remain active");
    };
    assert_eq!(launcher.pending_delete_connection_name(), Some("pg"));
    assert_eq!(launcher.connections().len(), 2);

    handle_launcher_key(
        &mut app,
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
    )?;
    let AppShell::Launcher(launcher) = &app else {
        panic!("launcher shell should remain active");
    };
    assert_eq!(launcher.selected_index(), 0);
    assert_eq!(launcher.pending_delete_connection_name(), Some("pg"));

    handle_launcher_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()),
    )?;
    let AppShell::Launcher(launcher) = &app else {
        panic!("launcher shell should remain active");
    };
    assert_eq!(launcher.pending_delete_connection_name(), None);
    assert_eq!(launcher.connections().len(), 2);

    handle_launcher_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty()),
    )?;
    handle_launcher_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty()),
    )?;
    let AppShell::Launcher(launcher) = &app else {
        panic!("launcher shell should remain active");
    };
    assert_eq!(launcher.connections().len(), 1);
    assert_eq!(launcher.connections()[0].name, "analytics");
    Ok(())
}

#[test]
fn workspace_escape_returns_to_launcher_when_launcher_is_available() -> Result<()> {
    let workspace = WorkspaceApp::bootstrap(
        vec![ConnectionBootstrap {
            name: "pg".to_string(),
            driver: Box::new(MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
            )),
        }],
        50,
    )?;
    let launcher = LauncherApp::new(
        vec![crate::config::ConnectionConfig {
            name: "pg".to_string(),
            url: "postgresql://postgres@localhost/postgres".to_string(),
        }],
        std::env::temp_dir().join("relora-launcher-return-test.json"),
    );
    let mut shell = AppShell::Workspace(WorkspaceShell::with_launcher(workspace, launcher).into());

    assert!(!handle_shell_key(
        &mut shell,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
    )?);
    assert!(matches!(shell, AppShell::Launcher(_)));
    Ok(())
}

#[test]
fn launcher_missing_driver_prompt_renders_as_modal() -> Result<()> {
    let mut launcher = LauncherApp::new(
        Vec::new(),
        std::env::temp_dir().join("relora-launcher-missing-driver-modal-test.json"),
    );
    launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;
    launcher.prompt_missing_driver(DatabaseKind::MySql, "MySQL/MariaDB", "relora-driver-mysql");
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Launcher(Box::new(launcher))))?;

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Driver Missing"));
    assert!(rendered.contains("MySQL/MariaDB"));
    assert!(rendered.contains("relora-driver-mysql"));
    assert!(rendered.contains("RELORA_MYSQL_DRIVER"));
    assert!(rendered.contains("Press Esc"));
    assert!(!rendered.contains("cargo install"));
    assert!(!rendered.contains("Press y to install"));
    assert!(!rendered.contains("driver is not installed"));
    assert!(!rendered.contains("Driver field:"));
    Ok(())
}

#[test]
fn launcher_missing_driver_prompt_blocks_form_input_until_closed() -> Result<()> {
    let mut launcher = LauncherApp::new(
        Vec::new(),
        std::env::temp_dir().join("relora-launcher-missing-driver-input-test.json"),
    );
    launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;
    launcher.prompt_missing_driver(DatabaseKind::MySql, "MySQL/MariaDB", "relora-driver-mysql");

    handle_launcher_form_key(
        &mut launcher,
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
    )?;

    assert_eq!(launcher.pending_missing_driver(), Some(DatabaseKind::MySql));
    assert_eq!(
        launcher
            .form_snapshot()
            .expect("form should remain open")
            .field,
        LauncherFormField::Name
    );

    handle_launcher_form_key(
        &mut launcher,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
    )?;
    assert_eq!(launcher.pending_missing_driver(), None);
    Ok(())
}

#[test]
fn launcher_screen_renders_wordmark_logo_above_brand_copy() -> Result<()> {
    let launcher = LauncherApp::new(
        vec![crate::config::ConnectionConfig {
            name: "pg".to_string(),
            url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
        }],
        std::env::temp_dir().join("relora-launcher-logo-test.json"),
    );
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Launcher(Box::new(launcher))))?;

    let buffer = terminal.backend().buffer();
    let mut logo_y = None;
    let mut copy_y = None;
    for y in 0..buffer.area.height {
        let row = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>();
        if logo_y.is_none() && row.contains("█▀█ █▀▀ █   █▀█ █▀█ ▄▀█")
        {
            logo_y = Some(y as usize);
        }
        if copy_y.is_none() && row.contains("Terminal Database Workspace") {
            copy_y = Some(y as usize);
        }
    }

    let logo_y = logo_y.expect("launcher wordmark logo should render");
    let copy_y = copy_y.expect("launcher brand copy should render");
    assert!(
        logo_y < copy_y,
        "logo should render above brand copy, got logo_y={logo_y}, copy_y={copy_y}"
    );
    Ok(())
}

#[test]
fn launcher_empty_state_feels_like_a_product_home() -> Result<()> {
    let launcher = LauncherApp::new(
        Vec::new(),
        std::env::temp_dir().join("relora-launcher-empty-state-test.json"),
    );
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Launcher(Box::new(launcher))))?;

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Open a saved workspace"));
    assert!(rendered.contains("Create your first PostgreSQL profile"));
    Ok(())
}

#[test]
fn launcher_connection_form_renders_structured_database_fields() -> Result<()> {
    let mut launcher = LauncherApp::new(
        Vec::new(),
        std::env::temp_dir().join("relora-launcher-structured-form-test.json"),
    );
    launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;
    launcher.apply_action(LauncherAction::SwitchFormField)?;
    launcher.insert_form_char('m')?;
    for _ in 0..4 {
        launcher.apply_action(LauncherAction::SwitchFormField)?;
    }
    for ch in "alice".chars() {
        launcher.insert_form_char(ch)?;
    }
    launcher.apply_action(LauncherAction::SwitchFormField)?;
    for ch in "secret".chars() {
        launcher.insert_form_char(ch)?;
    }

    let form = launcher
        .form_snapshot()
        .expect("launcher form should be open");
    assert_eq!(form.field, LauncherFormField::Password);

    let backend = TestBackend::new(100, 32);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Launcher(Box::new(launcher))))?;

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Driver: MySQL/MariaDB"));
    assert!(rendered.contains("Host / SQLite path"));
    assert!(rendered.contains("User: alice"));
    assert!(rendered.contains("Password: ******"));
    assert!(!rendered.contains("secret"));
    Ok(())
}

#[test]
fn launcher_form_arrow_keys_move_up_and_cycle_driver() -> Result<()> {
    let mut launcher = LauncherApp::new(
        Vec::new(),
        std::env::temp_dir().join("relora-launcher-form-input-test.json"),
    );
    launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;

    handle_launcher_form_key(
        &mut launcher,
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
    )?;
    assert_eq!(
        launcher.form_snapshot().expect("form should be open").field,
        LauncherFormField::Driver
    );

    handle_launcher_form_key(
        &mut launcher,
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
    )?;
    assert_eq!(
        launcher
            .form_snapshot()
            .expect("form should be open")
            .driver,
        crate::launcher::LauncherDatabaseKind::MySql
    );

    handle_launcher_form_key(
        &mut launcher,
        KeyEvent::new(KeyCode::Left, KeyModifiers::empty()),
    )?;
    assert_eq!(
        launcher
            .form_snapshot()
            .expect("form should be open")
            .driver,
        crate::launcher::LauncherDatabaseKind::Postgres
    );

    handle_launcher_form_key(
        &mut launcher,
        KeyEvent::new(KeyCode::Up, KeyModifiers::empty()),
    )?;
    assert_eq!(
        launcher.form_snapshot().expect("form should be open").field,
        LauncherFormField::Name
    );
    Ok(())
}

#[test]
fn launcher_form_t_tests_connection_instead_of_typing() -> Result<()> {
    let mut launcher = LauncherApp::new(
        Vec::new(),
        std::env::temp_dir().join("relora-launcher-form-test-shortcut.json"),
    );
    launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;

    handle_launcher_form_key(
        &mut launcher,
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()),
    )?;

    let form = launcher
        .form_snapshot()
        .expect("launcher form should remain open");
    assert_eq!(form.name, "t");

    handle_launcher_form_key(
        &mut launcher,
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
    )?;
    handle_launcher_form_key(
        &mut launcher,
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()),
    )?;

    let form = launcher
        .form_snapshot()
        .expect("launcher form should remain open");
    assert_eq!(form.name, "t");
    let status = launcher
        .status()
        .expect("test shortcut should report status");
    assert!(
        status.contains("Connection test failed")
            || status.contains("driver is missing")
            || status.contains("succeeded")
    );
    Ok(())
}

#[test]
fn launcher_screen_is_centered_in_the_canvas() -> Result<()> {
    let launcher = LauncherApp::new(
        vec![crate::config::ConnectionConfig {
            name: "pg".to_string(),
            url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
        }],
        std::env::temp_dir().join("relora-launcher-center-test.json"),
    );
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| draw(frame, &AppShell::Launcher(Box::new(launcher))))?;

    let buffer = terminal.backend().buffer();
    let mut title_position = None;
    for y in 0..buffer.area.height {
        let row = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>();
        if let Some(x) = row.find("Relora") {
            title_position = Some((x, y as usize));
            break;
        }
    }

    let (x, y) = title_position.expect("launcher title should render");
    assert!(
        x >= 20,
        "launcher card should be horizontally centered, got x={x}"
    );
    assert!(
        y >= 5,
        "launcher card should be vertically centered, got y={y}"
    );
    Ok(())
}

#[test]
fn ui_accent_styles_match_the_launcher_logo_blue() {
    let accent = theme_accent_color();

    assert_eq!(highlight_style().fg, Some(accent));
    assert_eq!(active_tab_style().bg, Some(accent));
    assert_eq!(grid_row_highlight_style(true).bg, Some(accent));
    assert_eq!(grid_header_cell_style(true, true).bg, Some(accent));
}

#[test]
fn editor_completion_popup_prefers_showing_below_the_cursor() {
    let lines = vec!["select".to_string()];
    let editor = EditorView {
        title: "SQL",
        tab_strip: "",
        tab_count: 1,
        selected_tab_index: 0,
        lines: &lines,
        cursor_row: 0,
        cursor_col: 3,
        result_strip: None,
        result_set_count: 0,
        selected_result_index: 0,
        status: None,
    };

    let rect = editor_completion_popup_rect(Rect::new(10, 5, 60, 18), editor, 4);

    assert_eq!(rect.y, 7);
}

#[test]
fn editor_completion_popup_moves_above_when_bottom_space_is_tight() {
    let lines = vec!["select".to_string(); 11];
    let editor = EditorView {
        title: "SQL",
        tab_strip: "",
        tab_count: 1,
        selected_tab_index: 0,
        lines: &lines,
        cursor_row: 10,
        cursor_col: 3,
        result_strip: None,
        result_set_count: 0,
        selected_result_index: 0,
        status: None,
    };

    let rect = editor_completion_popup_rect(Rect::new(10, 5, 60, 14), editor, 4);

    assert_eq!(rect.y, 10);
}

#[test]
fn enter_accepts_completion_instead_of_inserting_a_newline() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id", "email"], &[&["1", "alice@example.com"]])],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("sel")?;

    handle_editor_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;

    assert_eq!(
        app.editor_snapshot()
            .expect("editor should remain open after accepting completion")
            .sql,
        "SELECT"
    );
    assert!(
        app.view().editor_completion.is_none(),
        "accepting a completion should dismiss the popup"
    );
    Ok(())
}

#[test]
fn workspace_delete_confirmation_renders_as_modal() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog(
                    "public",
                    &[
                        (DbObjectKind::Table, "users"),
                        (DbObjectKind::Table, "orders"),
                    ],
                )],
                vec![preview(&["id"], &[&["1"]])],
            )
            .with_executions(vec![query(&["rows_affected"], &[&["1"]])]),
        ),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("DELETE FROM users WHERE id = 1;")?;
    app.apply_action(WorkspaceAction::ExecuteEditor)?;

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| draw(frame, &AppShell::Workspace(app.into())))?;

    let buffer = terminal.backend().buffer();
    let rendered = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Confirm DELETE"));
    assert!(rendered.contains("DELETE FROM users"));
    assert!(rendered.contains("Press y to execute"));
    Ok(())
}

#[test]
fn workspace_delete_confirmation_blocks_editor_input_until_answered() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("DELETE FROM users WHERE id = 1;")?;
    app.apply_action(WorkspaceAction::ExecuteEditor)?;

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
    )?;
    assert_eq!(
        app.editor_snapshot()
            .expect("editor should remain open")
            .sql,
        "DELETE FROM users WHERE id = 1;"
    );

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
    )?;
    assert!(app.view().delete_confirmation.is_none());
    assert!(
        app.selected_session_status()
            .expect("cancel should set status")
            .contains("canceled")
    );
    Ok(())
}

#[test]
fn tab_switches_focus_to_sql_results_when_results_exist() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
            )
            .with_executions(vec![query(&["id"], &[&["1"], &["2"]])]),
        ),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("SELECT id FROM users;")?;
    app.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_result_visible(&mut app)?;

    assert!(app.view().sql_editor_focused);

    handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
    assert!(app.data_grid_focused());

    handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))?;
    assert_eq!(app.grid_scroll_offset(), 1);

    handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
    let view = app.view();
    assert!(view.assets_focused);
    assert!(!view.data_grid_focused);
    Ok(())
}

#[test]
fn sql_tab_tab_cycle_allows_navigating_assets_after_results() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog(
                    "public",
                    &[
                        (DbObjectKind::Table, "users"),
                        (DbObjectKind::Table, "orders"),
                    ],
                )],
                vec![preview(&["id"], &[&["1"]])],
            )
            .with_executions(vec![query(&["id"], &[&["1"], &["2"]])]),
        ),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("SELECT id FROM users;")?;
    app.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_result_visible(&mut app)?;

    handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
    handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;

    assert!(app.view().assets_focused);

    let selected_before = app.selected_row_index();
    handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))?;

    assert_ne!(
        app.selected_row_index(),
        selected_before,
        "after tabbing from results to assets, arrow keys should navigate the asset tree"
    );
    Ok(())
}

#[test]
fn tab_cycles_between_editor_and_assets_when_no_results_are_visible() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("SELECT ")?;

    assert!(app.view().sql_editor_focused);
    handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
    assert!(app.view().assets_focused);
    handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
    assert!(app.view().sql_editor_focused);

    assert_eq!(
        app.editor_snapshot()
            .expect("editor should remain open")
            .sql,
        "SELECT "
    );
    Ok(())
}

#[test]
fn backtab_reverses_sql_focus_cycle() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
            )
            .with_executions(vec![query(&["id"], &[&["1"], &["2"]])]),
        ),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("SELECT id FROM users;")?;
    app.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_result_visible(&mut app)?;

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
    )?;
    assert!(app.view().assets_focused);

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
    )?;
    assert!(app.view().data_grid_focused);

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
    )?;
    assert!(app.view().sql_editor_focused);
    Ok(())
}

#[test]
fn mouse_click_focuses_sql_editor_and_results_by_region() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
            )
            .with_executions(vec![query(&["id"], &[&["1"], &["2"]])]),
        ),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let area = Rect::new(0, 0, 120, 40);
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("SELECT id FROM users;")?;
    app.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_result_visible(&mut app)?;

    handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
    assert!(app.view().data_grid_focused);

    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 80,
            row: 16,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )?;
    assert!(app.view().sql_editor_focused);

    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 80,
            row: 30,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )?;
    assert!(app.view().data_grid_focused);
    Ok(())
}

#[test]
fn mouse_click_switches_right_tabs_and_focuses_the_clicked_panel() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let area = Rect::new(0, 0, 120, 40);

    let (sql_x, sql_y) = tab_click_point(area, "SQL");
    click_at(&mut app, area, sql_x, sql_y)?;
    assert_eq!(app.active_right_tab(), RightPaneTab::Sql);
    assert!(app.view().sql_editor_focused);

    let (structure_x, structure_y) = tab_click_point(area, "Structure");
    click_at(&mut app, area, structure_x, structure_y)?;
    assert_eq!(app.active_right_tab(), RightPaneTab::Structure);
    assert!(app.view().data_grid_focused);
    Ok(())
}

#[test]
fn double_clicking_an_asset_row_opens_data_from_structure_tab() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog(
                "public",
                &[
                    (DbObjectKind::Table, "users"),
                    (DbObjectKind::Table, "orders"),
                ],
            )],
            vec![
                preview(&["id"], &[&["1"]]),
                preview(&["order_id"], &[&["ord_1"]]),
            ],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let area = Rect::new(0, 0, 120, 40);
    app.apply_action(WorkspaceAction::SelectRightStructureTab)?;

    let orders_index = app
        .tree_rows()
        .iter()
        .position(|row| row.label == "orders")
        .expect("orders row should exist");
    let (x, y) = asset_row_click_point(area, orders_index);

    click_at(&mut app, area, x, y)?;
    click_at(&mut app, area, x, y)?;
    drain_until_preview_columns(&mut app, &["order_id"])?;

    assert_eq!(app.active_right_tab(), RightPaneTab::Data);
    assert_eq!(app.active_preview().columns, vec!["order_id"]);
    Ok(())
}

#[test]
fn double_clicking_an_asset_row_opens_sql_when_sql_tab_is_active() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog(
                "public",
                &[
                    (DbObjectKind::Table, "users"),
                    (DbObjectKind::Table, "orders"),
                ],
            )],
            vec![
                preview(&["id"], &[&["1"]]),
                preview(&["order_id"], &[&["ord_1"]]),
            ],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let area = Rect::new(0, 0, 120, 40);
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;

    let orders_index = app
        .tree_rows()
        .iter()
        .position(|row| row.label == "orders")
        .expect("orders row should exist");
    let (x, y) = asset_row_click_point(area, orders_index);

    click_at(&mut app, area, x, y)?;
    click_at(&mut app, area, x, y)?;

    assert_eq!(app.active_right_tab(), RightPaneTab::Sql);
    assert!(
        app.editor_snapshot()
            .expect("sql editor should remain open")
            .sql
            .contains("\"orders\""),
        "double-clicking an asset while the SQL tab is active should open SQL for that object"
    );
    Ok(())
}

#[test]
fn mouse_click_selects_grid_cell_for_copy_actions() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["id", "email"],
                &[&["1", "alice@example.com"], &["2", "bob@example.com"]],
            )],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let area = Rect::new(0, 0, 120, 40);
    let (x, y) = grid_cell_click_point(&app, area, 1, 1);

    click_at(&mut app, area, x, y)?;
    app.apply_action(WorkspaceAction::CopyCurrentCell)?;

    assert_eq!(app.grid_selected_row_index(), 1);
    assert_eq!(app.grid_selected_column_index(), 1);
    assert_eq!(app.last_copied_text(), Some("bob@example.com"));
    Ok(())
}

#[test]
fn mouse_click_selects_editor_tabs_and_middle_click_closes_them() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"]])],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let area = Rect::new(0, 0, 120, 40);
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    let first_title = app
        .active_editor_tab_title()
        .expect("first editor tab should exist")
        .to_string();
    app.apply_action(WorkspaceAction::NewEditorTab)?;
    let second_title = app
        .active_editor_tab_title()
        .expect("second editor tab should exist")
        .to_string();

    let (first_x, first_y) = editor_tab_click_point(&app, area, 0);
    click_at(&mut app, area, first_x, first_y)?;
    assert_eq!(app.active_editor_tab_title(), Some(first_title.as_str()));

    let (second_x, second_y) = editor_tab_click_point(&app, area, 1);
    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Middle),
            column: second_x,
            row: second_y,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )?;

    assert_eq!(app.editor_tab_count(), 1);
    assert_eq!(app.active_editor_tab_title(), Some(first_title.as_str()));
    assert_ne!(app.active_editor_tab_title(), Some(second_title.as_str()));
    Ok(())
}

#[test]
fn mouse_click_switches_sql_result_sets_from_the_result_strip() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(
            MockDriver::new(
                vec![catalog("public", &[(DbObjectKind::Table, "users")])],
                vec![preview(&["id"], &[&["1"]])],
            )
            .with_executions(vec![query_batch(vec![
                query(&["id"], &[&["1"]]),
                query(&["email"], &[&["alice@example.com"]]),
            ])]),
        ),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let area = Rect::new(0, 0, 120, 40);
    app.apply_action(WorkspaceAction::OpenSqlEditor)?;
    app.set_editor_sql("select 1; select 'alice@example.com';")?;
    app.apply_action(WorkspaceAction::ExecuteEditor)?;
    drain_until_result_visible(&mut app)?;

    let (x, y) = result_set_click_point(&app, area, 1);
    click_at(&mut app, area, x, y)?;

    assert_eq!(app.active_grid().columns, vec!["email"]);
    Ok(())
}

#[test]
fn double_clicking_a_grid_cell_opens_the_row_inspector_at_that_field() -> Result<()> {
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
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let area = Rect::new(0, 0, 120, 40);
    let (x, y) = grid_cell_click_point(&app, area, 1, 1);

    click_at(&mut app, area, x, y)?;
    assert!(!app.row_inspector_open());

    click_at(&mut app, area, x, y)?;

    let inspector = app
        .view()
        .row_inspector
        .expect("double-clicking a data cell should open the row inspector");
    assert_eq!(inspector.row_index, 1);
    assert_eq!(inspector.selected_field, 1);
    assert_eq!(inspector.columns[1], "email");
    assert_eq!(inspector.values[1], "bob@example.com");
    Ok(())
}

#[derive(Default)]
struct TestClipboard {
    writes: Vec<String>,
}

impl ClipboardSink for TestClipboard {
    fn set_text(&mut self, text: &str) -> Result<()> {
        self.writes.push(text.to_string());
        Ok(())
    }
}

#[test]
fn row_inspector_copy_and_edit_follow_the_selected_field() -> Result<()> {
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
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::FocusDataGrid)?;
    app.apply_action(WorkspaceAction::ScrollDataGridDown)?;
    app.apply_action(WorkspaceAction::OpenRowInspector)?;
    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
    )?;

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
    )?;
    assert_eq!(app.last_copied_text(), Some("bob@example.com"));

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
    )?;
    let edit = app
        .view()
        .cell_edit
        .expect("editing from row inspector should open the cell edit modal");
    assert_eq!(edit.column, "email");
    assert_eq!(edit.input, "bob@example.com");
    Ok(())
}

#[test]
fn clipboard_sync_writes_every_new_copy_event_even_if_text_repeats() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["id"], &[&["1"], &["1"]])],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let mut clipboard = TestClipboard::default();
    let mut last_synced_copy_sequence = 0;

    app.apply_action(WorkspaceAction::CopyCurrentCell)?;
    assert!(sync_clipboard_if_needed(
        &app,
        &mut clipboard,
        &mut last_synced_copy_sequence
    )?);
    assert_eq!(clipboard.writes, vec!["1"]);

    assert!(!sync_clipboard_if_needed(
        &app,
        &mut clipboard,
        &mut last_synced_copy_sequence
    )?);

    app.apply_action(WorkspaceAction::ScrollDataGridDown)?;
    app.apply_action(WorkspaceAction::CopyCurrentCell)?;
    assert!(sync_clipboard_if_needed(
        &app,
        &mut clipboard,
        &mut last_synced_copy_sequence
    )?);
    assert_eq!(clipboard.writes, vec!["1", "1"]);
    Ok(())
}

#[test]
fn row_inspector_can_toggle_between_formatted_and_raw_detail_modes() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["payload"],
                &[&[r#"{"user":{"id":1},"tags":["news","ops"]}"#]],
            )],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::FocusDataGrid)?;
    app.apply_action(WorkspaceAction::OpenRowInspector)?;
    assert!(app.row_inspector_formatted());

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
    )?;
    assert!(!app.row_inspector_formatted());

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
    )?;
    assert!(app.row_inspector_formatted());
    Ok(())
}

#[test]
fn row_inspector_tab_switches_between_fields_and_preview_boxes() -> Result<()> {
    let detail_value = (1..=12)
        .map(|index| format!("marker-{index:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(
                &["payload", "note"],
                &[&[detail_value.as_str(), "short note"]],
            )],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::FocusDataGrid)?;
    app.apply_action(WorkspaceAction::OpenRowInspector)?;

    let inspector = app
        .view()
        .row_inspector
        .expect("row inspector should be open");
    assert_eq!(
        inspector.active_pane,
        relora_app::view::RowInspectorPane::Fields
    );

    handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
    let inspector = app
        .view()
        .row_inspector
        .expect("row inspector should remain open");
    assert_eq!(
        inspector.active_pane,
        relora_app::view::RowInspectorPane::Preview
    );

    handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))?;
    let inspector = app
        .view()
        .row_inspector
        .expect("row inspector should remain open");
    assert_eq!(inspector.selected_field, 0);
    assert_eq!(inspector.detail_scroll, 3);

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
    )?;
    let inspector = app
        .view()
        .row_inspector
        .expect("row inspector should remain open");
    assert_eq!(
        inspector.active_pane,
        relora_app::view::RowInspectorPane::Fields
    );

    handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))?;
    let inspector = app
        .view()
        .row_inspector
        .expect("row inspector should remain open");
    assert_eq!(inspector.selected_field, 1);
    assert_eq!(inspector.detail_scroll, 0);
    Ok(())
}

#[test]
fn row_inspector_can_scroll_long_detail_content() -> Result<()> {
    let detail_value = (1..=24)
        .map(|index| format!("marker-{index:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["payload"], &[&[detail_value.as_str()]])],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::FocusDataGrid)?;
    app.apply_action(WorkspaceAction::OpenRowInspector)?;

    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|frame| {
        draw_row_inspector(
            frame,
            frame.area(),
            app.view()
                .row_inspector
                .expect("row inspector should remain open"),
        )
    })?;
    let initial = (0..terminal.backend().buffer().area.height)
        .map(|y| {
            (0..terminal.backend().buffer().area.width)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!initial.contains("marker-15"));

    handle_key(
        &mut app,
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
    )?;

    terminal.draw(|frame| {
        draw_row_inspector(
            frame,
            frame.area(),
            app.view()
                .row_inspector
                .expect("row inspector should remain open"),
        )
    })?;
    let scrolled = (0..terminal.backend().buffer().area.height)
        .map(|y| {
            (0..terminal.backend().buffer().area.width)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(scrolled.contains("marker-15"));
    Ok(())
}

#[test]
fn row_inspector_renders_the_selected_value_by_default() -> Result<()> {
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["email"], &[&["alice@example.com"]])],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    app.apply_action(WorkspaceAction::FocusDataGrid)?;
    app.apply_action(WorkspaceAction::OpenRowInspector)?;

    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| {
        draw_row_inspector(
            frame,
            frame.area(),
            app.view()
                .row_inspector
                .expect("row inspector should remain open"),
        )
    })?;

    let rendered = (0..terminal.backend().buffer().area.height)
        .map(|y| {
            (0..terminal.backend().buffer().area.width)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("alice@example.com"));
    Ok(())
}

#[test]
fn mouse_wheel_scrolls_row_inspector_detail_content() -> Result<()> {
    let detail_value = (1..=24)
        .map(|index| format!("marker-{index:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let bootstraps = vec![ConnectionBootstrap {
        name: "pg".to_string(),
        driver: Box::new(MockDriver::new(
            vec![catalog("public", &[(DbObjectKind::Table, "users")])],
            vec![preview(&["payload"], &[&[detail_value.as_str()]])],
        )),
    }];
    let mut app = WorkspaceApp::bootstrap(bootstraps, 50)?;
    let area = Rect::new(0, 0, 120, 40);
    app.apply_action(WorkspaceAction::FocusDataGrid)?;
    app.apply_action(WorkspaceAction::OpenRowInspector)?;

    let popup = centered_rect(
        ROW_INSPECTOR_POPUP_WIDTH_PERCENT,
        ROW_INSPECTOR_POPUP_HEIGHT_PERCENT,
        area,
    );
    let inner = Block::default().borders(Borders::ALL).inner(popup);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(ROW_INSPECTOR_FIELD_LIST_HEIGHT_PERCENT),
            Constraint::Percentage(ROW_INSPECTOR_DETAIL_HEIGHT_PERCENT),
        ])
        .split(inner);
    let detail_area = sections[1];

    handle_mouse(
        &mut app,
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: detail_area.x + 2,
            row: detail_area.y + 2,
            modifiers: KeyModifiers::NONE,
        },
        area,
    )?;

    assert_eq!(
        app.view()
            .row_inspector
            .expect("row inspector should remain open")
            .detail_scroll,
        3
    );
    Ok(())
}

#[test]
fn detail_value_formatting_prettifies_json_and_postgres_arrays() {
    let json = r#"{"user":{"id":1},"tags":["news","ops"]}"#;
    let formatted_json = format_detail_value(json, true);
    assert!(formatted_json.contains("\n  \"user\""));
    assert!(formatted_json.contains("\n  \"tags\""));
    assert_eq!(format_detail_value(json, false), json);

    let array = r#"{"alpha","beta","gamma"}"#;
    assert_eq!(
        format_detail_value(array, true),
        "[0] alpha\n[1] beta\n[2] gamma"
    );
    assert_eq!(format_detail_value(array, false), array);
}
