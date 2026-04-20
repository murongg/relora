use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use relora_core::{
    app::App as ConnectionApp,
    db::{DatabaseKind, DbColumn, DbObjectKind, DbObjectRef, SqlExecutionResult, TablePreview},
};

use crate::{
    background::{SessionEvent, SessionWorker, TemplateKind},
    completion::{CompletionItem, suggest_sql_completions},
    editor::SqlEditorBuffer,
    sql_tools::{
        StagedCrudSql, copy_row_text, explain_sql, primary_key_names, staged_update_sql,
        where_clause_for_row,
    },
    templates::{delete_template, insert_template, select_template, update_template},
    tree::{TreeEntry, TreeNodeKey, TreeRow},
    view::{
        CellEditView, CommandPaletteItemView, CommandPaletteView, DataFilterView,
        DeleteConfirmationView, EditorCompletionView, EditorView, RightPaneTab, RightPaneTabView,
        RowInspectorPane, RowInspectorView, SqlHistoryView, StagedCrudView, StructureView,
        WorkspaceView,
    },
};

const GRID_PAGE_STEP: usize = 10;
const GRID_COLUMN_PAGE_STEP: usize = 4;
const ROW_INSPECTOR_SCROLL_STEP: usize = 3;
const ROW_INSPECTOR_PAGE_STEP: usize = 10;
const PREVIEW_PAGE_STEP_MULTIPLIER: usize = 1;
const GRID_COLUMN_WIDTH_STEP: u16 = 4;
const MIN_GRID_COLUMN_WIDTH_OVERRIDE: u16 = 6;
const MAX_GRID_COLUMN_WIDTH_OVERRIDE: u16 = 32;
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(500);

pub struct ConnectionBootstrap {
    pub name: String,
    pub driver: Box<dyn relora_core::db::DatabaseDriver>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceAction {
    NextItem,
    PreviousItem,
    ToggleNode,
    ToggleBrowserFocus,
    ReverseBrowserFocus,
    FocusAssets,
    FocusSqlEditor,
    FocusDataGrid,
    ScrollDataGridDown,
    ScrollDataGridUp,
    PageDataGridDown,
    PageDataGridUp,
    ScrollDataGridRight,
    ScrollDataGridLeft,
    PageDataGridRight,
    PageDataGridLeft,
    ExpandSelectedGridColumn,
    ShrinkSelectedGridColumn,
    ResetSelectedGridColumnWidth,
    FreezeGridColumnsThroughSelection,
    ClearFrozenGridColumns,
    OpenCommandPalette,
    CloseCommandPalette,
    NextCommandPaletteItem,
    PreviousCommandPaletteItem,
    ExecuteCommandPaletteSelection,
    OpenSqlHistory,
    CloseSqlHistory,
    NextSqlHistoryItem,
    PreviousSqlHistoryItem,
    RunSqlHistorySelection,
    OpenDataFilter,
    CloseDataFilter,
    ApplyDataFilter,
    CopyCurrentCell,
    CopyCurrentRow,
    CopyCurrentWhereClause,
    StartCellEdit,
    CloseCellEdit,
    PreviewStagedCrud,
    CommitStagedCrud,
    ConfirmDeleteOperation,
    CancelDeleteOperation,
    SelectRightDataTab,
    SelectRightSqlTab,
    SelectRightStructureTab,
    NextRightTab,
    PreviousRightTab,
    OpenRowInspector,
    CloseRowInspector,
    NextRowInspectorPane,
    PreviousRowInspectorPane,
    NextRowInspectorField,
    PreviousRowInspectorField,
    ScrollRowInspectorDetailDown,
    ScrollRowInspectorDetailUp,
    PageRowInspectorDetailDown,
    PageRowInspectorDetailUp,
    NextPreviewPage,
    PreviousPreviewPage,
    Refresh,
    OpenSqlEditor,
    OpenSelectTemplate,
    OpenInsertTemplate,
    OpenUpdateTemplate,
    OpenDeleteTemplate,
    ExecuteEditor,
    ExplainCurrentStatement,
    ExplainAnalyzeCurrentStatement,
    AcceptEditorCompletion,
    NextEditorCompletion,
    PreviousEditorCompletion,
    CloseEditorCompletion,
    NewEditorTab,
    CloseEditorTab,
    NextEditorTab,
    PreviousEditorTab,
    NextResultSet,
    PreviousResultSet,
    CancelTasks,
    CloseEditor,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserFocus {
    Assets,
    SqlEditor,
    DataGrid,
}

#[derive(Debug, Clone, Copy)]
struct PaletteCommand {
    item: CommandPaletteItemView,
    action: WorkspaceAction,
}

const PALETTE_COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Inspect Current Row",
            hint: "Open a field-by-field view for the current table row",
        },
        action: WorkspaceAction::OpenRowInspector,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Open SQL Editor",
            hint: "Open an editor for the selected connection or object",
        },
        action: WorkspaceAction::OpenSqlEditor,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Show Table Structure",
            hint: "Inspect columns, nullability, defaults, and primary keys",
        },
        action: WorkspaceAction::SelectRightStructureTab,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Open SQL History",
            hint: "Search and rerun previously executed SQL",
        },
        action: WorkspaceAction::OpenSqlHistory,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Explain Current SQL",
            hint: "Run EXPLAIN for the statement under the cursor",
        },
        action: WorkspaceAction::ExplainCurrentStatement,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Filter Data",
            hint: "Filter the selected table preview with a safe parameterized search",
        },
        action: WorkspaceAction::OpenDataFilter,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Focus Data Grid",
            hint: "Move keyboard focus to the preview or result table",
        },
        action: WorkspaceAction::FocusDataGrid,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Focus SQL Editor",
            hint: "Move keyboard focus to the active SQL editor tab",
        },
        action: WorkspaceAction::FocusSqlEditor,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Focus Assets",
            hint: "Move keyboard focus to the database object tree",
        },
        action: WorkspaceAction::FocusAssets,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Refresh Connection",
            hint: "Reload catalog metadata for the selected connection",
        },
        action: WorkspaceAction::Refresh,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Open SELECT Template",
            hint: "Create a SELECT statement for the selected object",
        },
        action: WorkspaceAction::OpenSelectTemplate,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Open INSERT Template",
            hint: "Create an INSERT statement for the selected table",
        },
        action: WorkspaceAction::OpenInsertTemplate,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Open UPDATE Template",
            hint: "Create an UPDATE statement for the selected table",
        },
        action: WorkspaceAction::OpenUpdateTemplate,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Open DELETE Template",
            hint: "Create a DELETE statement for the selected table",
        },
        action: WorkspaceAction::OpenDeleteTemplate,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Cancel Tasks",
            hint: "Cancel pending work for the active connection",
        },
        action: WorkspaceAction::CancelTasks,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "New SQL Tab",
            hint: "Open another SQL editor tab",
        },
        action: WorkspaceAction::NewEditorTab,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Close SQL Editor",
            hint: "Close the SQL editor pane",
        },
        action: WorkspaceAction::CloseEditor,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Quit",
            hint: "Exit Relora",
        },
        action: WorkspaceAction::Quit,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlEditorSnapshot {
    pub title: String,
    pub sql: String,
}

pub struct WorkspaceApp {
    sessions: Vec<ConnectionSession>,
    entries: Vec<TreeEntry>,
    tree_rows: Vec<TreeRow>,
    selected_row: usize,
    should_quit: bool,
    editor: Option<SqlEditorState>,
    empty_grid: TablePreview,
    browser_focus: BrowserFocus,
    active_right_tab: RightPaneTab,
    structure: StructureState,
    grid_selected_row: usize,
    grid_selected_column: usize,
    grid_scroll_offset: usize,
    grid_column_offset: usize,
    command_palette: Option<CommandPaletteState>,
    editor_completion: EditorCompletionState,
    sql_history: SqlHistoryState,
    data_filter: Option<DataFilterState>,
    active_data_filter: Option<String>,
    preview_page_offset: usize,
    preview_has_next_page: bool,
    last_copied_text: Option<String>,
    copy_sequence: u64,
    grid_column_width_overrides: BTreeMap<String, BTreeMap<usize, u16>>,
    grid_frozen_leading_columns: BTreeMap<String, usize>,
    cell_edit: Option<CellEditState>,
    staged_crud: Option<StagedCrudState>,
    delete_confirmation: Option<DeleteConfirmationState>,
    row_inspector: Option<RowInspectorState>,
    workspace_status: Option<String>,
    last_tree_click: Option<TreeClickState>,
    last_grid_click: Option<GridClickState>,
}

struct ConnectionSession {
    name: String,
    connection_label: String,
    kind: DatabaseKind,
    app: ConnectionApp,
    worker: SessionWorker,
    expanded: bool,
    expanded_databases: BTreeSet<String>,
    expanded_schemas: BTreeSet<(String, String)>,
    expanded_groups: BTreeSet<(String, String, DbObjectKind)>,
    pending: PendingSessionWork,
}

#[derive(Default)]
struct PendingSessionWork {
    preview_request: Option<PendingPreviewRequest>,
    refresh_request_id: Option<u64>,
    template_request: Option<PendingTemplateRequest>,
    structure_request: Option<PendingStructureRequest>,
    execute_requests: BTreeMap<u64, usize>,
}

struct PendingPreviewRequest {
    request_id: u64,
}

struct PendingTemplateRequest {
    request_id: u64,
    kind: TemplateKind,
    object: DbObjectRef,
}

struct PendingStructureRequest {
    request_id: u64,
    object: DbObjectRef,
}

#[derive(Default)]
struct StructureState {
    object: Option<DbObjectRef>,
    columns: Vec<DbColumn>,
    grid: TablePreview,
    loading: bool,
    loaded: bool,
    status: Option<String>,
}

struct CommandPaletteState {
    query: String,
    visible_items: Vec<CommandPaletteItemView>,
    selected: usize,
}

#[derive(Default)]
struct EditorCompletionState {
    items: Vec<CompletionItem>,
    selected: usize,
}

#[derive(Default)]
struct SqlHistoryState {
    entries: Vec<String>,
    query: String,
    visible_items: Vec<String>,
    selected: usize,
    open: bool,
}

struct DataFilterState {
    input: String,
}

struct CellEditState {
    connection_index: usize,
    object: DbObjectRef,
    row_index: usize,
    column_index: usize,
    column_name: String,
    input: String,
}

struct StagedCrudState {
    connection_index: usize,
    sql: StagedCrudSql,
}

struct DeleteConfirmationState {
    title: String,
    message: String,
    sql_preview: String,
    operation: PendingDeleteOperation,
}

enum PendingDeleteOperation {
    ExecuteSql {
        connection_index: usize,
        sql: String,
        status: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeleteOperationKind {
    Delete,
    Drop,
    Truncate,
}

impl DeleteOperationKind {
    fn label(self) -> &'static str {
        match self {
            Self::Delete => "DELETE",
            Self::Drop => "DROP",
            Self::Truncate => "TRUNCATE",
        }
    }
}

struct RowInspectorState {
    selected_field: usize,
    detail_scroll: usize,
    formatted: bool,
    active_pane: RowInspectorPane,
}

struct TreeClickState {
    row_index: usize,
    at: Instant,
}

struct GridClickState {
    row_index: usize,
    column_index: usize,
    at: Instant,
}

struct SqlEditorState {
    tabs: Vec<SqlEditorTab>,
    selected_tab: usize,
    next_tab_number: usize,
    tab_strip: String,
}

struct SqlEditorTab {
    id: usize,
    connection_index: usize,
    database_name: Option<String>,
    title: String,
    buffer: SqlEditorBuffer,
    status: Option<String>,
    result_sets: Vec<EditorResultSet>,
    selected_result: usize,
    pending_execute_request_id: Option<u64>,
    result_strip: String,
}

struct EditorResultSet {
    title: String,
    grid: TablePreview,
}

impl WorkspaceApp {
    pub fn bootstrap(bootstraps: Vec<ConnectionBootstrap>, preview_limit: usize) -> Result<Self> {
        let mut sessions = Vec::new();
        for bootstrap in bootstraps {
            let mut driver = bootstrap.driver;
            let app = ConnectionApp::bootstrap(driver.as_mut(), preview_limit)?;
            let mut session = ConnectionSession {
                name: bootstrap.name,
                connection_label: app.connection_label().to_string(),
                kind: driver.kind(),
                worker: SessionWorker::spawn(driver),
                app,
                expanded: true,
                expanded_databases: BTreeSet::new(),
                expanded_schemas: BTreeSet::new(),
                expanded_groups: BTreeSet::new(),
                pending: PendingSessionWork::default(),
            };

            if let Some(database) = session.app.selected_database_name() {
                session.expanded_databases.insert(database.to_string());
            }
            if let Some(schema) = session.app.selected_schema_name() {
                let database = session.app.selected_database_name().unwrap_or_default();
                session
                    .expanded_schemas
                    .insert((database.to_string(), schema.to_string()));
                if let Some(object) = session.app.selected_object() {
                    session.expanded_groups.insert((
                        database.to_string(),
                        schema.to_string(),
                        object.kind,
                    ));
                }
            }

            sessions.push(session);
        }

        let mut workspace = Self {
            sessions,
            entries: Vec::new(),
            tree_rows: Vec::new(),
            selected_row: 0,
            should_quit: false,
            editor: None,
            empty_grid: TablePreview::default(),
            browser_focus: BrowserFocus::Assets,
            active_right_tab: RightPaneTab::Data,
            structure: StructureState::default(),
            grid_selected_row: 0,
            grid_selected_column: 0,
            grid_scroll_offset: 0,
            grid_column_offset: 0,
            command_palette: None,
            editor_completion: EditorCompletionState::default(),
            sql_history: SqlHistoryState::default(),
            data_filter: None,
            active_data_filter: None,
            preview_page_offset: 0,
            preview_has_next_page: false,
            last_copied_text: None,
            copy_sequence: 0,
            grid_column_width_overrides: BTreeMap::new(),
            grid_frozen_leading_columns: BTreeMap::new(),
            cell_edit: None,
            staged_crud: None,
            delete_confirmation: None,
            row_inspector: None,
            workspace_status: None,
            last_tree_click: None,
            last_grid_click: None,
        };
        workspace.rebuild_rows(None);
        workspace.selected_row = workspace.first_object_row().unwrap_or(0);
        workspace.sync_preview_pagination_from_active_preview();
        Ok(workspace)
    }

    pub fn apply_action(&mut self, action: WorkspaceAction) -> Result<()> {
        if !matches!(
            action,
            WorkspaceAction::Refresh
                | WorkspaceAction::Quit
                | WorkspaceAction::CancelTasks
                | WorkspaceAction::ToggleBrowserFocus
                | WorkspaceAction::ReverseBrowserFocus
                | WorkspaceAction::FocusAssets
                | WorkspaceAction::FocusSqlEditor
                | WorkspaceAction::FocusDataGrid
                | WorkspaceAction::ScrollDataGridDown
                | WorkspaceAction::ScrollDataGridUp
                | WorkspaceAction::PageDataGridDown
                | WorkspaceAction::PageDataGridUp
                | WorkspaceAction::ScrollDataGridRight
                | WorkspaceAction::ScrollDataGridLeft
                | WorkspaceAction::PageDataGridRight
                | WorkspaceAction::PageDataGridLeft
                | WorkspaceAction::ExpandSelectedGridColumn
                | WorkspaceAction::ShrinkSelectedGridColumn
                | WorkspaceAction::ResetSelectedGridColumnWidth
                | WorkspaceAction::FreezeGridColumnsThroughSelection
                | WorkspaceAction::ClearFrozenGridColumns
                | WorkspaceAction::OpenCommandPalette
                | WorkspaceAction::CloseCommandPalette
                | WorkspaceAction::NextCommandPaletteItem
                | WorkspaceAction::PreviousCommandPaletteItem
                | WorkspaceAction::OpenSqlHistory
                | WorkspaceAction::CloseSqlHistory
                | WorkspaceAction::NextSqlHistoryItem
                | WorkspaceAction::PreviousSqlHistoryItem
                | WorkspaceAction::OpenDataFilter
                | WorkspaceAction::CloseDataFilter
                | WorkspaceAction::StartCellEdit
                | WorkspaceAction::CloseCellEdit
                | WorkspaceAction::ConfirmDeleteOperation
                | WorkspaceAction::CancelDeleteOperation
                | WorkspaceAction::NextEditorCompletion
                | WorkspaceAction::PreviousEditorCompletion
                | WorkspaceAction::CloseEditorCompletion
                | WorkspaceAction::SelectRightDataTab
                | WorkspaceAction::SelectRightSqlTab
                | WorkspaceAction::SelectRightStructureTab
                | WorkspaceAction::NextRightTab
                | WorkspaceAction::PreviousRightTab
                | WorkspaceAction::CloseRowInspector
                | WorkspaceAction::NextRowInspectorPane
                | WorkspaceAction::PreviousRowInspectorPane
                | WorkspaceAction::NextRowInspectorField
                | WorkspaceAction::PreviousRowInspectorField
                | WorkspaceAction::ScrollRowInspectorDetailDown
                | WorkspaceAction::ScrollRowInspectorDetailUp
                | WorkspaceAction::PageRowInspectorDetailDown
                | WorkspaceAction::PageRowInspectorDetailUp
                | WorkspaceAction::NextPreviewPage
                | WorkspaceAction::PreviousPreviewPage
        ) {
            self.workspace_status = None;
        }

        match action {
            WorkspaceAction::NextItem => self.move_selection(1)?,
            WorkspaceAction::PreviousItem => self.move_selection(-1)?,
            WorkspaceAction::ToggleNode => self.toggle_selected()?,
            WorkspaceAction::ToggleBrowserFocus => self.toggle_browser_focus(),
            WorkspaceAction::ReverseBrowserFocus => self.reverse_browser_focus(),
            WorkspaceAction::FocusAssets => self.focus_assets(),
            WorkspaceAction::FocusSqlEditor => self.focus_sql_editor(),
            WorkspaceAction::FocusDataGrid => self.focus_data_grid(),
            WorkspaceAction::ScrollDataGridDown => self.scroll_data_grid_by(1),
            WorkspaceAction::ScrollDataGridUp => self.scroll_data_grid_by(-1),
            WorkspaceAction::PageDataGridDown => self.scroll_data_grid_by(GRID_PAGE_STEP as isize),
            WorkspaceAction::PageDataGridUp => self.scroll_data_grid_by(-(GRID_PAGE_STEP as isize)),
            WorkspaceAction::ScrollDataGridRight => self.scroll_data_grid_columns_by(1),
            WorkspaceAction::ScrollDataGridLeft => self.scroll_data_grid_columns_by(-1),
            WorkspaceAction::PageDataGridRight => {
                self.scroll_data_grid_columns_by(GRID_COLUMN_PAGE_STEP as isize);
            }
            WorkspaceAction::PageDataGridLeft => {
                self.scroll_data_grid_columns_by(-(GRID_COLUMN_PAGE_STEP as isize));
            }
            WorkspaceAction::ExpandSelectedGridColumn => {
                let result =
                    self.adjust_selected_grid_column_width(GRID_COLUMN_WIDTH_STEP as isize);
                self.handle_error(result);
            }
            WorkspaceAction::ShrinkSelectedGridColumn => {
                let result =
                    self.adjust_selected_grid_column_width(-(GRID_COLUMN_WIDTH_STEP as isize));
                self.handle_error(result);
            }
            WorkspaceAction::ResetSelectedGridColumnWidth => {
                let result = self.reset_selected_grid_column_width();
                self.handle_error(result);
            }
            WorkspaceAction::FreezeGridColumnsThroughSelection => {
                let result = self.freeze_grid_columns_through_selection();
                self.handle_error(result);
            }
            WorkspaceAction::ClearFrozenGridColumns => {
                let result = self.clear_frozen_grid_columns();
                self.handle_error(result);
            }
            WorkspaceAction::OpenCommandPalette => self.open_command_palette(),
            WorkspaceAction::CloseCommandPalette => self.close_command_palette(),
            WorkspaceAction::NextCommandPaletteItem => self.move_command_palette_selection(1),
            WorkspaceAction::PreviousCommandPaletteItem => self.move_command_palette_selection(-1),
            WorkspaceAction::ExecuteCommandPaletteSelection => {
                let result = self.execute_command_palette_selection();
                self.handle_error(result);
            }
            WorkspaceAction::OpenSqlHistory => self.open_sql_history(),
            WorkspaceAction::CloseSqlHistory => self.close_sql_history(),
            WorkspaceAction::NextSqlHistoryItem => self.move_sql_history_selection(1),
            WorkspaceAction::PreviousSqlHistoryItem => self.move_sql_history_selection(-1),
            WorkspaceAction::RunSqlHistorySelection => {
                let result = self.run_sql_history_selection();
                self.handle_error(result);
            }
            WorkspaceAction::OpenDataFilter => {
                let result = self.open_data_filter();
                self.handle_error(result);
            }
            WorkspaceAction::CloseDataFilter => self.close_data_filter(),
            WorkspaceAction::ApplyDataFilter => {
                let result = self.apply_data_filter();
                self.handle_error(result);
            }
            WorkspaceAction::CopyCurrentCell => {
                let result = self.copy_current_cell();
                self.handle_error(result);
            }
            WorkspaceAction::CopyCurrentRow => {
                let result = self.copy_current_row();
                self.handle_error(result);
            }
            WorkspaceAction::CopyCurrentWhereClause => {
                let result = self.copy_current_where_clause();
                self.handle_error(result);
            }
            WorkspaceAction::AcceptEditorCompletion => {
                let result = self.accept_editor_completion();
                self.handle_error(result);
            }
            WorkspaceAction::NextEditorCompletion => self.move_editor_completion_selection(1),
            WorkspaceAction::PreviousEditorCompletion => self.move_editor_completion_selection(-1),
            WorkspaceAction::CloseEditorCompletion => self.close_editor_completion(),
            WorkspaceAction::StartCellEdit => {
                let result = self.start_cell_edit();
                self.handle_error(result);
            }
            WorkspaceAction::CloseCellEdit => self.close_cell_edit(),
            WorkspaceAction::PreviewStagedCrud => {
                let result = self.preview_staged_crud();
                self.handle_error(result);
            }
            WorkspaceAction::CommitStagedCrud => {
                let result = self.commit_staged_crud();
                self.handle_error(result);
            }
            WorkspaceAction::ConfirmDeleteOperation => {
                let result = self.confirm_delete_operation();
                self.handle_error(result);
            }
            WorkspaceAction::CancelDeleteOperation => self.cancel_delete_operation(),
            WorkspaceAction::SelectRightDataTab => self.select_right_tab(RightPaneTab::Data),
            WorkspaceAction::SelectRightSqlTab => {
                let result = self.select_right_sql_tab();
                self.handle_error(result);
            }
            WorkspaceAction::SelectRightStructureTab => {
                let result = self.select_right_structure_tab();
                self.handle_error(result);
            }
            WorkspaceAction::NextRightTab => {
                let result = self.next_right_tab();
                self.handle_error(result);
            }
            WorkspaceAction::PreviousRightTab => {
                let result = self.previous_right_tab();
                self.handle_error(result);
            }
            WorkspaceAction::OpenRowInspector => {
                let result = self.open_row_inspector();
                self.handle_error(result);
            }
            WorkspaceAction::CloseRowInspector => self.close_row_inspector(),
            WorkspaceAction::NextRowInspectorPane => self.move_row_inspector_pane(1),
            WorkspaceAction::PreviousRowInspectorPane => self.move_row_inspector_pane(-1),
            WorkspaceAction::NextRowInspectorField => self.move_row_inspector_field(1),
            WorkspaceAction::PreviousRowInspectorField => self.move_row_inspector_field(-1),
            WorkspaceAction::ScrollRowInspectorDetailDown => {
                self.scroll_row_inspector_detail_by(ROW_INSPECTOR_SCROLL_STEP as isize);
            }
            WorkspaceAction::ScrollRowInspectorDetailUp => {
                self.scroll_row_inspector_detail_by(-(ROW_INSPECTOR_SCROLL_STEP as isize));
            }
            WorkspaceAction::PageRowInspectorDetailDown => {
                self.scroll_row_inspector_detail_by(ROW_INSPECTOR_PAGE_STEP as isize);
            }
            WorkspaceAction::PageRowInspectorDetailUp => {
                self.scroll_row_inspector_detail_by(-(ROW_INSPECTOR_PAGE_STEP as isize));
            }
            WorkspaceAction::NextPreviewPage => {
                let result = self.load_next_preview_page();
                self.handle_error(result);
            }
            WorkspaceAction::PreviousPreviewPage => {
                let result = self.load_previous_preview_page();
                self.handle_error(result);
            }
            WorkspaceAction::Refresh => {
                let result = self.refresh_selected_connection();
                self.handle_error(result);
            }
            WorkspaceAction::OpenSqlEditor => {
                let result = self.open_sql_editor();
                self.handle_error(result);
            }
            WorkspaceAction::OpenSelectTemplate => {
                let result = self.open_select_template();
                self.handle_error(result);
            }
            WorkspaceAction::OpenInsertTemplate => {
                let result = self.open_insert_template();
                self.handle_error(result);
            }
            WorkspaceAction::OpenUpdateTemplate => {
                let result = self.open_update_template();
                self.handle_error(result);
            }
            WorkspaceAction::OpenDeleteTemplate => {
                let result = self.open_delete_template();
                self.handle_error(result);
            }
            WorkspaceAction::ExecuteEditor => {
                let result = self.execute_editor();
                self.handle_error(result);
            }
            WorkspaceAction::ExplainCurrentStatement => {
                let result = self.explain_current_statement(false);
                self.handle_error(result);
            }
            WorkspaceAction::ExplainAnalyzeCurrentStatement => {
                let result = self.explain_current_statement(true);
                self.handle_error(result);
            }
            WorkspaceAction::NewEditorTab => {
                let result = self.new_editor_tab();
                self.handle_error(result);
            }
            WorkspaceAction::CloseEditorTab => {
                let result = self.close_editor_tab();
                self.handle_error(result);
            }
            WorkspaceAction::NextEditorTab => {
                let result = self.next_editor_tab();
                self.handle_error(result);
            }
            WorkspaceAction::PreviousEditorTab => {
                let result = self.previous_editor_tab();
                self.handle_error(result);
            }
            WorkspaceAction::NextResultSet => {
                let result = self.next_result_set();
                self.handle_error(result);
            }
            WorkspaceAction::PreviousResultSet => {
                let result = self.previous_result_set();
                self.handle_error(result);
            }
            WorkspaceAction::CancelTasks => {
                let result = self.cancel_selected_connection_tasks();
                self.handle_error(result);
            }
            WorkspaceAction::CloseEditor => {
                self.editor = None;
                self.active_right_tab = RightPaneTab::Data;
                if self.browser_focus == BrowserFocus::SqlEditor {
                    self.browser_focus = BrowserFocus::Assets;
                }
                self.reset_grid_scroll();
            }
            WorkspaceAction::Quit => {
                self.should_quit = true;
            }
        }

        Ok(())
    }

    pub fn drain_background(&mut self) -> Result<usize> {
        let mut processed = 0;
        for session_index in 0..self.sessions.len() {
            loop {
                let event = { self.sessions[session_index].worker.try_recv() };
                let Some(event) = event else {
                    break;
                };
                self.handle_session_event(session_index, event)?;
                processed += 1;
            }
        }

        Ok(processed)
    }

    pub fn has_pending_tasks(&self) -> bool {
        self.pending_task_count() > 0
    }

    pub fn delete_confirmation_open(&self) -> bool {
        self.delete_confirmation.is_some()
    }

    pub fn view(&self) -> WorkspaceView<'_> {
        let selected_connection_index = self.selected_connection_index();
        let selected_session = selected_connection_index.and_then(|index| self.sessions.get(index));

        WorkspaceView {
            tree_rows: self.tree_rows(),
            selected_row_index: self.selected_row_index(),
            connection_count: self.connection_count(),
            pending_task_count: self.pending_task_count(),
            selected_connection_name: self.selected_connection_name(),
            selected_connection_label: self.selected_connection_label(),
            selected_database_name: self.selected_database_name(),
            selected_connection_kind: self.selected_connection_kind(),
            selected_connection_busy: selected_session
                .map(|session| session.pending.is_busy())
                .unwrap_or(false),
            selected_schema_name: self.selected_schema_name(),
            selected_group_kind: self.selected_group_kind(),
            selected_object: self.selected_object(),
            active_grid: self.active_grid(),
            preview_grid: self.active_preview(),
            active_right_tab: self.active_right_tab,
            right_tabs: self.right_tabs(),
            grid_selected_row_index: self.grid_selected_row_index(),
            grid_selected_column_index: self.grid_selected_column_index(),
            grid_scroll_offset: self.grid_scroll_offset(),
            grid_column_offset: self.grid_column_offset(),
            preview_loading: selected_session
                .map(|session| session.pending.preview_request.is_some())
                .unwrap_or(false),
            assets_focused: self.assets_focused(),
            sql_editor_focused: self.sql_editor_focused(),
            data_grid_focused: self.data_grid_focused(),
            command_palette: self.command_palette.as_ref().map(CommandPaletteState::view),
            sql_history: self.sql_history.view(),
            data_filter: self.data_filter_view(),
            cell_edit: self.cell_edit_view(),
            row_inspector: self.row_inspector_view(),
            editor: self.editor.as_ref().map(SqlEditorState::view),
            editor_completion: self.editor_completion.view(),
            structure: self.structure_view(),
            staged_crud: self.staged_crud_view(),
            delete_confirmation: self.delete_confirmation_view(),
            status: self.selected_session_status(),
            selected_connection_database_count: selected_session
                .map(|session| session.app.databases().len())
                .unwrap_or_default(),
            selected_connection_schema_count: selected_session
                .map(|session| {
                    session
                        .app
                        .databases()
                        .iter()
                        .map(|database| database.schemas.len())
                        .sum()
                })
                .unwrap_or_default(),
            selected_connection_object_count: selected_session
                .map(|session| {
                    session
                        .app
                        .databases()
                        .iter()
                        .map(|database| database.object_count())
                        .sum()
                })
                .unwrap_or_default(),
            selected_schema_table_count: self
                .object_count_for_selected_schema(DbObjectKind::Table)
                .unwrap_or_default(),
            selected_schema_view_count: self
                .object_count_for_selected_schema(DbObjectKind::View)
                .unwrap_or_default(),
            selected_schema_foreign_table_count: self
                .object_count_for_selected_schema(DbObjectKind::ForeignTable)
                .unwrap_or_default(),
        }
    }

    pub fn tree_rows(&self) -> &[TreeRow] {
        &self.tree_rows
    }

    pub fn selected_row(&self) -> &TreeRow {
        &self.tree_rows[self.selected_row]
    }

    pub fn active_preview(&self) -> &TablePreview {
        let connection_index = self
            .selected_connection_index()
            .unwrap_or(0)
            .min(self.sessions.len().saturating_sub(1));
        self.sessions
            .get(connection_index)
            .map(|session| session.app.preview())
            .unwrap_or(&self.empty_grid)
    }

    pub fn active_grid(&self) -> &TablePreview {
        match self.active_right_tab {
            RightPaneTab::Data => self.active_preview(),
            RightPaneTab::Sql => self
                .editor
                .as_ref()
                .and_then(|editor| editor.active_result_grid())
                .unwrap_or(&self.empty_grid),
            RightPaneTab::Structure => &self.structure.grid,
        }
    }

    pub fn grid_scroll_offset(&self) -> usize {
        self.grid_scroll_offset
            .min(self.active_grid_max_scroll_offset())
    }

    pub fn grid_selected_row_index(&self) -> usize {
        self.grid_selected_row
            .min(self.active_grid().rows.len().saturating_sub(1))
    }

    pub fn grid_selected_column_index(&self) -> usize {
        self.grid_selected_column
            .min(self.active_grid().columns.len().saturating_sub(1))
    }

    pub fn grid_column_offset(&self) -> usize {
        self.grid_column_offset
            .min(self.active_grid_max_column_offset())
    }

    pub fn select_grid_cell(&mut self, row_index: usize, column_index: usize) {
        self.grid_selected_row = row_index.min(self.active_grid_max_scroll_offset());
        self.grid_selected_column = column_index.min(self.active_grid_max_column_offset());
    }

    pub fn register_grid_cell_click(&mut self, row_index: usize, column_index: usize) -> bool {
        let now = Instant::now();
        let is_double_click = self.last_grid_click.as_ref().is_some_and(|click| {
            click.row_index == row_index
                && click.column_index == column_index
                && now.duration_since(click.at) <= DOUBLE_CLICK_WINDOW
        });

        self.select_grid_cell(row_index, column_index);
        if is_double_click {
            self.last_grid_click = None;
        } else {
            self.last_grid_click = Some(GridClickState {
                row_index,
                column_index,
                at: now,
            });
        }

        is_double_click
    }

    pub fn data_grid_focused(&self) -> bool {
        self.browser_focus == BrowserFocus::DataGrid
    }

    pub fn assets_focused(&self) -> bool {
        self.browser_focus == BrowserFocus::Assets
    }

    pub fn sql_editor_focused(&self) -> bool {
        self.browser_focus == BrowserFocus::SqlEditor
    }

    pub fn command_palette_open(&self) -> bool {
        self.command_palette.is_some()
    }

    pub fn command_palette_items(&self) -> Option<&[CommandPaletteItemView]> {
        Some(&self.command_palette.as_ref()?.visible_items)
    }

    pub fn command_palette_query(&self) -> Option<&str> {
        Some(self.command_palette.as_ref()?.query.as_str())
    }

    pub fn sql_history_open(&self) -> bool {
        self.sql_history.open
    }

    pub fn insert_sql_history_search_char(&mut self, ch: char) -> Result<()> {
        self.sql_history.insert_char(ch);
        Ok(())
    }

    pub fn backspace_sql_history_search(&mut self) -> Result<()> {
        self.sql_history.backspace();
        Ok(())
    }

    pub fn data_filter_open(&self) -> bool {
        self.data_filter.is_some()
    }

    pub fn active_data_filter(&self) -> Option<&str> {
        self.active_data_filter.as_deref()
    }

    pub fn insert_data_filter_char(&mut self, ch: char) -> Result<()> {
        let filter = self
            .data_filter
            .as_mut()
            .ok_or_else(|| anyhow!("data filter is not open"))?;
        filter.input.push(ch);
        Ok(())
    }

    pub fn backspace_data_filter(&mut self) -> Result<()> {
        let filter = self
            .data_filter
            .as_mut()
            .ok_or_else(|| anyhow!("data filter is not open"))?;
        filter.input.pop();
        Ok(())
    }

    pub fn last_copied_text(&self) -> Option<&str> {
        self.last_copied_text.as_deref()
    }

    pub fn copy_sequence(&self) -> u64 {
        self.copy_sequence
    }

    pub fn selected_grid_column_width_override(&self) -> Option<u16> {
        self.grid_column_width_override(self.grid_selected_column_index())
    }

    pub fn current_grid_column_width_overrides(&self) -> Option<&BTreeMap<usize, u16>> {
        let key = self.current_grid_layout_key()?;
        self.grid_column_width_overrides.get(&key)
    }

    pub fn grid_column_width_override(&self, column_index: usize) -> Option<u16> {
        self.current_grid_column_width_overrides()
            .and_then(|overrides| overrides.get(&column_index))
            .copied()
    }

    pub fn frozen_grid_column_count(&self) -> usize {
        let Some(key) = self.current_grid_layout_key() else {
            return 0;
        };
        self.grid_frozen_leading_columns
            .get(&key)
            .copied()
            .unwrap_or_default()
            .min(self.active_grid().columns.len())
    }

    pub fn cell_edit_open(&self) -> bool {
        self.cell_edit.is_some()
    }

    pub fn editor_completion_open(&self) -> bool {
        !self.editor_completion.items.is_empty()
    }

    pub fn insert_cell_edit_char(&mut self, ch: char) -> Result<()> {
        let edit = self
            .cell_edit
            .as_mut()
            .ok_or_else(|| anyhow!("cell edit is not open"))?;
        edit.input.push(ch);
        Ok(())
    }

    pub fn backspace_cell_edit(&mut self) -> Result<()> {
        let edit = self
            .cell_edit
            .as_mut()
            .ok_or_else(|| anyhow!("cell edit is not open"))?;
        edit.input.pop();
        Ok(())
    }

    pub fn clear_cell_edit_input(&mut self) -> Result<()> {
        let edit = self
            .cell_edit
            .as_mut()
            .ok_or_else(|| anyhow!("cell edit is not open"))?;
        edit.input.clear();
        Ok(())
    }

    pub fn row_inspector_open(&self) -> bool {
        self.row_inspector.is_some()
    }

    pub fn row_inspector_formatted(&self) -> bool {
        self.row_inspector
            .as_ref()
            .map(|inspector| inspector.formatted)
            .unwrap_or(false)
    }

    pub fn toggle_row_inspector_format(&mut self) -> Result<()> {
        let inspector = self
            .row_inspector
            .as_mut()
            .ok_or_else(|| anyhow!("row inspector is not open"))?;
        inspector.formatted = !inspector.formatted;
        inspector.detail_scroll = 0;
        Ok(())
    }

    pub fn active_right_tab(&self) -> RightPaneTab {
        self.active_right_tab
    }

    pub fn sql_results_available(&self) -> bool {
        self.active_right_tab == RightPaneTab::Sql && !self.active_grid().columns.is_empty()
    }

    pub fn selected_row_index(&self) -> usize {
        self.selected_row
    }

    pub fn select_tree_row_index(&mut self, index: usize) -> Result<()> {
        if self.entries.is_empty() {
            self.selected_row = 0;
            self.last_tree_click = None;
            return Ok(());
        }

        let index = index.min(self.entries.len() - 1);
        let previous_row = self.selected_row;
        self.selected_row = index;
        if self.selected_row != previous_row {
            self.reset_grid_scroll();
            self.active_data_filter = None;
            self.reset_preview_pagination();
            self.staged_crud = None;
        }
        self.ensure_selected_object_preview()
    }

    pub fn register_tree_row_click(&mut self, row_index: usize) -> Result<bool> {
        let now = Instant::now();
        let is_double_click = self.last_tree_click.as_ref().is_some_and(|click| {
            click.row_index == row_index && now.duration_since(click.at) <= DOUBLE_CLICK_WINDOW
        });
        if self.selected_row != row_index {
            self.select_tree_row_index(row_index)?;
        }
        if is_double_click {
            self.last_tree_click = None;
        } else {
            self.last_tree_click = Some(TreeClickState { row_index, at: now });
        }
        Ok(is_double_click)
    }

    pub fn open_selected_tree_item_default(&mut self) -> Result<()> {
        let Some(entry) = self.entries.get(self.selected_row).cloned() else {
            return Ok(());
        };

        match entry.key {
            TreeNodeKey::Object { .. } => {
                if self.active_right_tab == RightPaneTab::Sql {
                    self.open_sql_editor()
                } else {
                    let should_schedule_preview = self
                        .selected_connection_index()
                        .and_then(|index| self.sessions.get(index))
                        .map(|session| session.pending.preview_request.is_none())
                        .unwrap_or(true);
                    self.select_right_tab(RightPaneTab::Data);
                    self.focus_data_grid();
                    if should_schedule_preview {
                        self.ensure_selected_object_preview()
                    } else {
                        Ok(())
                    }
                }
            }
            _ => self.toggle_selected(),
        }
    }

    pub fn connection_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn selected_object(&self) -> Option<&DbObjectRef> {
        match &self.entries.get(self.selected_row)?.key {
            TreeNodeKey::Object { object, .. } => Some(object),
            _ => None,
        }
    }

    pub fn selected_connection_name(&self) -> Option<&str> {
        let index = self.selected_connection_index()?;
        Some(self.sessions.get(index)?.name.as_str())
    }

    pub fn selected_connection_label(&self) -> Option<&str> {
        let index = self.selected_connection_index()?;
        Some(self.sessions.get(index)?.connection_label.as_str())
    }

    pub fn selected_database_name(&self) -> Option<&str> {
        match &self.entries.get(self.selected_row)?.key {
            TreeNodeKey::Database { database, .. } => Some(database.as_str()),
            TreeNodeKey::Schema { database, .. } => Some(database.as_str()),
            TreeNodeKey::Group { database, .. } => Some(database.as_str()),
            TreeNodeKey::Object { object, .. } => Some(object.database.as_str()),
            TreeNodeKey::Connection { .. } => self.selected_session()?.app.selected_database_name(),
        }
    }

    pub fn selected_schema_name(&self) -> Option<&str> {
        match &self.entries.get(self.selected_row)?.key {
            TreeNodeKey::Schema { schema, .. } => Some(schema.as_str()),
            TreeNodeKey::Group { schema, .. } => Some(schema.as_str()),
            TreeNodeKey::Object { object, .. } => Some(object.schema.as_str()),
            TreeNodeKey::Connection { .. } | TreeNodeKey::Database { .. } => None,
        }
    }

    pub fn selected_group_kind(&self) -> Option<DbObjectKind> {
        match &self.entries.get(self.selected_row)?.key {
            TreeNodeKey::Group { kind, .. } => Some(*kind),
            TreeNodeKey::Object { object, .. } => Some(object.kind),
            _ => None,
        }
    }

    pub fn selected_connection_kind(&self) -> Option<DatabaseKind> {
        let index = self.selected_connection_index()?;
        Some(self.sessions.get(index)?.kind)
    }

    pub fn selected_session_status(&self) -> Option<&str> {
        (self.active_right_tab == RightPaneTab::Sql)
            .then(|| self.editor_status())
            .flatten()
            .or_else(|| {
                (self.active_right_tab == RightPaneTab::Structure)
                    .then_some(self.structure.status.as_deref())
                    .flatten()
                    .or_else(|| {
                        self.workspace_status.as_deref().or_else(|| {
                            let index = self.selected_connection_index()?;
                            Some(self.sessions.get(index)?.app.status())
                        })
                    })
            })
    }

    pub fn selected_connection_schema_count(&self) -> Option<usize> {
        let session = self.selected_session()?;
        Some(
            session
                .app
                .databases()
                .iter()
                .map(|database| database.schemas.len())
                .sum(),
        )
    }

    pub fn selected_connection_object_count(&self) -> Option<usize> {
        let session = self.selected_session()?;
        Some(
            session
                .app
                .databases()
                .iter()
                .map(|database| database.object_count())
                .sum(),
        )
    }

    pub fn object_count_for_selected_schema(&self, kind: DbObjectKind) -> Option<usize> {
        let session = self.selected_session()?;
        let database_name = self.selected_database_name()?;
        let schema_name = self.selected_schema_name()?;
        let schema = session
            .app
            .databases()
            .iter()
            .find(|database| database.name == database_name)?
            .schemas
            .iter()
            .find(|schema| schema.name == schema_name)?;
        Some(schema.object_count(kind))
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn is_editor_open(&self) -> bool {
        self.editor.is_some()
    }

    pub fn editor_snapshot(&self) -> Option<SqlEditorSnapshot> {
        let editor = self.editor.as_ref()?;
        let tab = editor.active_tab()?;
        Some(SqlEditorSnapshot {
            title: tab.title.clone(),
            sql: tab.buffer.sql(),
        })
    }

    pub fn editor_status(&self) -> Option<&str> {
        self.editor.as_ref()?.active_tab()?.status.as_deref()
    }

    pub fn editor_tab_count(&self) -> usize {
        self.editor
            .as_ref()
            .map(|editor| editor.tabs.len())
            .unwrap_or(0)
    }

    pub fn editor_result_set_count(&self) -> usize {
        self.editor
            .as_ref()
            .and_then(|editor| editor.active_tab())
            .map(|tab| tab.result_sets.len())
            .unwrap_or(0)
    }

    pub fn active_editor_tab_title(&self) -> Option<&str> {
        self.editor
            .as_ref()?
            .active_tab()
            .map(|tab| tab.title.as_str())
    }

    pub fn editor_tab_strip(&self) -> Option<&str> {
        Some(self.editor.as_ref()?.tab_strip.as_str())
    }

    pub fn select_editor_tab_index(&mut self, index: usize) -> Result<()> {
        let editor = self
            .editor
            .as_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        if index >= editor.tabs.len() {
            return Err(anyhow!("sql editor tab index is out of range"));
        }
        editor.selected_tab = index;
        editor.rebuild_tab_strip();
        self.active_right_tab = RightPaneTab::Sql;
        self.focus_sql_editor();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    pub fn select_result_set_index(&mut self, index: usize) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.select_result_set(index)?;
        self.active_right_tab = RightPaneTab::Sql;
        self.reset_grid_scroll();
        Ok(())
    }

    pub fn close_editor_tab_index(&mut self, index: usize) -> Result<()> {
        let editor = self
            .editor
            .as_ref()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        if index >= editor.tabs.len() {
            return Err(anyhow!("sql editor tab index is out of range"));
        }
        self.select_editor_tab_index(index)?;
        self.close_editor_tab()
    }

    pub fn insert_command_palette_char(&mut self, ch: char) -> Result<()> {
        let palette = self
            .command_palette
            .as_mut()
            .ok_or_else(|| anyhow!("command palette is not open"))?;
        palette.insert_char(ch);
        Ok(())
    }

    pub fn backspace_command_palette(&mut self) -> Result<()> {
        let palette = self
            .command_palette
            .as_mut()
            .ok_or_else(|| anyhow!("command palette is not open"))?;
        palette.backspace();
        Ok(())
    }

    pub fn set_editor_sql(&mut self, sql: &str) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.buffer.replace_sql(sql);
        tab.result_sets.clear();
        tab.selected_result = 0;
        tab.rebuild_result_strip();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    pub fn editor_cursor(&self) -> Option<(usize, usize)> {
        let tab = self.editor.as_ref()?.active_tab()?;
        Some(tab.buffer.cursor())
    }

    pub fn insert_editor_char(&mut self, ch: char) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.buffer.insert_char(ch);
        tab.result_sets.clear();
        tab.selected_result = 0;
        tab.rebuild_result_strip();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    pub fn insert_editor_tab(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.buffer.insert_str("    ");
        tab.result_sets.clear();
        tab.selected_result = 0;
        tab.rebuild_result_strip();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    pub fn backspace_editor(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.buffer.backspace();
        tab.result_sets.clear();
        tab.selected_result = 0;
        tab.rebuild_result_strip();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    pub fn newline_editor(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.buffer.new_line();
        tab.result_sets.clear();
        tab.selected_result = 0;
        tab.rebuild_result_strip();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    pub fn move_editor_cursor_left(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.buffer.move_left();
        self.refresh_editor_completion();
        Ok(())
    }

    pub fn move_editor_cursor_right(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.buffer.move_right();
        self.refresh_editor_completion();
        Ok(())
    }

    pub fn move_editor_cursor_up(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.buffer.move_up();
        self.refresh_editor_completion();
        Ok(())
    }

    pub fn move_editor_cursor_down(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.buffer.move_down();
        self.refresh_editor_completion();
        Ok(())
    }

    fn toggle_browser_focus(&mut self) {
        self.browser_focus = match self.active_right_tab {
            RightPaneTab::Sql if self.editor.is_some() => match self.browser_focus {
                BrowserFocus::Assets => BrowserFocus::SqlEditor,
                BrowserFocus::SqlEditor => {
                    if self.sql_results_available() {
                        BrowserFocus::DataGrid
                    } else {
                        BrowserFocus::Assets
                    }
                }
                BrowserFocus::DataGrid => BrowserFocus::Assets,
            },
            _ => match self.browser_focus {
                BrowserFocus::DataGrid => BrowserFocus::Assets,
                BrowserFocus::Assets | BrowserFocus::SqlEditor => BrowserFocus::DataGrid,
            },
        };
    }

    fn reverse_browser_focus(&mut self) {
        self.browser_focus = match self.active_right_tab {
            RightPaneTab::Sql if self.editor.is_some() => match self.browser_focus {
                BrowserFocus::Assets => {
                    if self.sql_results_available() {
                        BrowserFocus::DataGrid
                    } else {
                        BrowserFocus::SqlEditor
                    }
                }
                BrowserFocus::SqlEditor => BrowserFocus::Assets,
                BrowserFocus::DataGrid => BrowserFocus::SqlEditor,
            },
            _ => match self.browser_focus {
                BrowserFocus::DataGrid => BrowserFocus::Assets,
                BrowserFocus::Assets | BrowserFocus::SqlEditor => BrowserFocus::DataGrid,
            },
        };
    }

    fn focus_assets(&mut self) {
        self.browser_focus = BrowserFocus::Assets;
    }

    fn focus_sql_editor(&mut self) {
        if self.editor.is_some() {
            self.active_right_tab = RightPaneTab::Sql;
            self.browser_focus = BrowserFocus::SqlEditor;
        } else {
            self.browser_focus = BrowserFocus::Assets;
        }
    }

    fn focus_data_grid(&mut self) {
        self.browser_focus = BrowserFocus::DataGrid;
    }

    fn select_right_tab(&mut self, tab: RightPaneTab) {
        self.active_right_tab = tab;
        if tab != RightPaneTab::Sql && self.browser_focus == BrowserFocus::SqlEditor {
            self.browser_focus = BrowserFocus::Assets;
        }
        self.reset_grid_scroll();
    }

    fn select_right_sql_tab(&mut self) -> Result<()> {
        if self.editor.is_none() {
            self.open_sql_editor()
        } else {
            self.select_right_tab(RightPaneTab::Sql);
            self.focus_sql_editor();
            Ok(())
        }
    }

    fn select_right_structure_tab(&mut self) -> Result<()> {
        self.select_right_tab(RightPaneTab::Structure);
        self.ensure_selected_object_structure()
    }

    fn next_right_tab(&mut self) -> Result<()> {
        match self.active_right_tab {
            RightPaneTab::Data => self.select_right_sql_tab(),
            RightPaneTab::Sql => self.select_right_structure_tab(),
            RightPaneTab::Structure => {
                self.select_right_tab(RightPaneTab::Data);
                Ok(())
            }
        }
    }

    fn previous_right_tab(&mut self) -> Result<()> {
        match self.active_right_tab {
            RightPaneTab::Data => self.select_right_structure_tab(),
            RightPaneTab::Sql => {
                self.select_right_tab(RightPaneTab::Data);
                Ok(())
            }
            RightPaneTab::Structure => self.select_right_sql_tab(),
        }
    }

    fn right_tabs(&self) -> [RightPaneTabView; 3] {
        [
            RightPaneTabView {
                kind: RightPaneTab::Data,
                title: "Data",
                active: self.active_right_tab == RightPaneTab::Data,
                available: true,
            },
            RightPaneTabView {
                kind: RightPaneTab::Sql,
                title: "SQL",
                active: self.active_right_tab == RightPaneTab::Sql,
                available: true,
            },
            RightPaneTabView {
                kind: RightPaneTab::Structure,
                title: "Structure",
                active: self.active_right_tab == RightPaneTab::Structure,
                available: true,
            },
        ]
    }

    fn scroll_data_grid_by(&mut self, delta: isize) {
        if delta.is_negative() {
            self.grid_scroll_offset = self.grid_scroll_offset.saturating_sub(delta.unsigned_abs());
        } else {
            self.grid_scroll_offset = self.grid_scroll_offset.saturating_add(delta.unsigned_abs());
        }
        self.grid_selected_row = self.grid_scroll_offset;
        self.clamp_grid_scroll();
    }

    fn scroll_data_grid_columns_by(&mut self, delta: isize) {
        if delta.is_negative() {
            self.grid_column_offset = self.grid_column_offset.saturating_sub(delta.unsigned_abs());
        } else {
            self.grid_column_offset = self.grid_column_offset.saturating_add(delta.unsigned_abs());
        }
        self.grid_selected_column = self.grid_column_offset;
        self.clamp_grid_scroll();
    }

    fn current_grid_layout_key(&self) -> Option<String> {
        match self.active_right_tab {
            RightPaneTab::Data => {
                let connection_index = self.selected_connection_index()?;
                let object = self.selected_object()?;
                Some(format!(
                    "data/{connection_index}/{}",
                    object.database_qualified_name()
                ))
            }
            RightPaneTab::Structure => {
                let connection_index = self.selected_connection_index()?;
                let object = self
                    .structure
                    .object
                    .as_ref()
                    .or_else(|| self.selected_object())?;
                Some(format!(
                    "structure/{connection_index}/{}",
                    object.database_qualified_name()
                ))
            }
            RightPaneTab::Sql => {
                let editor = self.editor.as_ref()?;
                let tab = editor.active_tab()?;
                Some(format!("sql/{}/{}", tab.id, tab.selected_result))
            }
        }
    }

    fn adjust_selected_grid_column_width(&mut self, delta: isize) -> Result<()> {
        let key = self
            .current_grid_layout_key()
            .ok_or_else(|| anyhow!("no grid is available to resize"))?;
        let column_index = self.grid_selected_column_index();
        let current_width = self
            .grid_column_width_overrides
            .get(&key)
            .and_then(|overrides| overrides.get(&column_index))
            .copied()
            .unwrap_or_else(|| self.preferred_active_grid_column_width(column_index));
        let next_width = if delta.is_negative() {
            current_width.saturating_sub(delta.unsigned_abs() as u16)
        } else {
            current_width.saturating_add(delta as u16)
        }
        .clamp(
            MIN_GRID_COLUMN_WIDTH_OVERRIDE,
            MAX_GRID_COLUMN_WIDTH_OVERRIDE,
        );
        self.grid_column_width_overrides
            .entry(key)
            .or_default()
            .insert(column_index, next_width);
        self.workspace_status = Some(format!(
            "Resized column {} to {} chars.",
            column_index + 1,
            next_width
        ));
        Ok(())
    }

    fn reset_selected_grid_column_width(&mut self) -> Result<()> {
        let key = self
            .current_grid_layout_key()
            .ok_or_else(|| anyhow!("no grid is available to resize"))?;
        let column_index = self.grid_selected_column_index();
        let mut remove_key = false;
        if let Some(overrides) = self.grid_column_width_overrides.get_mut(&key) {
            overrides.remove(&column_index);
            remove_key = overrides.is_empty();
        }
        if remove_key {
            self.grid_column_width_overrides.remove(&key);
        }
        self.workspace_status = Some(format!(
            "Restored automatic width for column {}.",
            column_index + 1
        ));
        Ok(())
    }

    fn freeze_grid_columns_through_selection(&mut self) -> Result<()> {
        let key = self
            .current_grid_layout_key()
            .ok_or_else(|| anyhow!("no grid is available to freeze"))?;
        if self.active_grid().columns.is_empty() {
            return Err(anyhow!("no grid is available to freeze"));
        }

        let frozen_count = self
            .grid_selected_column_index()
            .saturating_add(1)
            .min(self.active_grid().columns.len());
        self.grid_frozen_leading_columns.insert(key, frozen_count);
        self.workspace_status = Some(format!(
            "Pinned the first {} column{}.",
            frozen_count,
            if frozen_count == 1 { "" } else { "s" }
        ));
        Ok(())
    }

    fn clear_frozen_grid_columns(&mut self) -> Result<()> {
        let key = self
            .current_grid_layout_key()
            .ok_or_else(|| anyhow!("no grid is available to update"))?;
        self.grid_frozen_leading_columns.remove(&key);
        self.workspace_status = Some("Cleared frozen grid columns.".to_string());
        Ok(())
    }

    fn preferred_active_grid_column_width(&self, column_index: usize) -> u16 {
        let grid = self.active_grid();
        let header_width = grid
            .columns
            .get(column_index)
            .map(|value| value.chars().count())
            .unwrap_or_default();
        let sample_width = grid
            .rows
            .iter()
            .skip(self.grid_scroll_offset())
            .take(12)
            .filter_map(|row| row.get(column_index))
            .map(|value| value.replace('\n', " ").chars().count())
            .max()
            .unwrap_or_default();
        header_width.max(sample_width).clamp(
            MIN_GRID_COLUMN_WIDTH_OVERRIDE as usize,
            MAX_GRID_COLUMN_WIDTH_OVERRIDE as usize,
        ) as u16
    }

    fn reset_grid_scroll(&mut self) {
        self.grid_selected_row = 0;
        self.grid_selected_column = 0;
        self.grid_scroll_offset = 0;
        self.grid_column_offset = 0;
        self.row_inspector = None;
        self.last_grid_click = None;
    }

    fn clamp_grid_scroll(&mut self) {
        self.grid_selected_row = self.grid_selected_row_index();
        self.grid_selected_column = self.grid_selected_column_index();
        self.grid_scroll_offset = self.grid_scroll_offset();
        self.grid_column_offset = self.grid_column_offset();
    }

    fn reset_preview_pagination(&mut self) {
        self.preview_page_offset = 0;
        self.preview_has_next_page = false;
    }

    fn sync_preview_pagination_from_active_preview(&mut self) {
        let row_count = self.active_preview().rows.len();
        let limit = self.selected_preview_limit().unwrap_or(100);
        self.preview_has_next_page = row_count >= limit;
    }

    fn load_next_preview_page(&mut self) -> Result<()> {
        if self.active_right_tab != RightPaneTab::Data {
            return Ok(());
        }
        if !self.preview_has_next_page && !self.active_preview().rows.is_empty() {
            self.workspace_status = Some("Already on the last loaded preview page.".to_string());
            return Ok(());
        }

        let (connection_index, object) = self
            .selected_object_target()
            .ok_or_else(|| anyhow!("select a table-like object first"))?;
        let page_size = self.selected_preview_limit().unwrap_or(100) * PREVIEW_PAGE_STEP_MULTIPLIER;
        let next_offset = self.preview_page_offset.saturating_add(page_size);
        if let Some(filter) = self.active_data_filter.clone() {
            self.schedule_filtered_preview_page_for_connection_object(
                connection_index,
                object,
                filter,
                next_offset,
                false,
            )
        } else {
            self.schedule_preview_page_for_connection_object(
                connection_index,
                object,
                next_offset,
                false,
            )
        }
    }

    fn load_previous_preview_page(&mut self) -> Result<()> {
        if self.active_right_tab != RightPaneTab::Data {
            return Ok(());
        }
        if self.preview_page_offset == 0 {
            self.workspace_status = Some("Already on the first preview page.".to_string());
            return Ok(());
        }

        let (connection_index, object) = self
            .selected_object_target()
            .ok_or_else(|| anyhow!("select a table-like object first"))?;
        let page_size = self.selected_preview_limit().unwrap_or(100) * PREVIEW_PAGE_STEP_MULTIPLIER;
        let previous_offset = self.preview_page_offset.saturating_sub(page_size);
        if let Some(filter) = self.active_data_filter.clone() {
            self.schedule_filtered_preview_page_for_connection_object(
                connection_index,
                object,
                filter,
                previous_offset,
                false,
            )
        } else {
            self.schedule_preview_page_for_connection_object(
                connection_index,
                object,
                previous_offset,
                false,
            )
        }
    }

    fn active_grid_max_scroll_offset(&self) -> usize {
        self.active_grid().rows.len().saturating_sub(1)
    }

    fn active_grid_max_column_offset(&self) -> usize {
        self.active_grid().columns.len().saturating_sub(1)
    }

    fn open_command_palette(&mut self) {
        self.command_palette = Some(CommandPaletteState::new());
    }

    fn close_command_palette(&mut self) {
        self.command_palette = None;
    }

    fn move_command_palette_selection(&mut self, delta: isize) {
        if let Some(palette) = self.command_palette.as_mut() {
            palette.move_selection(delta);
        }
    }

    fn execute_command_palette_selection(&mut self) -> Result<()> {
        let Some(action) = self
            .command_palette
            .as_ref()
            .and_then(CommandPaletteState::selected_action)
        else {
            self.close_command_palette();
            return Ok(());
        };

        self.close_command_palette();
        self.apply_action(action)
    }

    fn accept_editor_completion(&mut self) -> Result<()> {
        let label = self
            .editor_completion
            .selected_label()
            .ok_or_else(|| anyhow!("no completion item is selected"))?
            .to_string();
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        if !tab.buffer.apply_completion(&label) {
            return Err(anyhow!("could not apply the selected completion"));
        }
        tab.result_sets.clear();
        tab.selected_result = 0;
        tab.rebuild_result_strip();
        self.close_editor_completion();
        Ok(())
    }

    fn move_editor_completion_selection(&mut self, delta: isize) {
        self.editor_completion.move_selection(delta);
    }

    fn close_editor_completion(&mut self) {
        self.editor_completion.clear();
    }

    fn refresh_editor_completion(&mut self) {
        let Some(tab) = self.active_editor_tab() else {
            self.editor_completion.clear();
            return;
        };
        let Some(prefix) = tab.buffer.completion_prefix() else {
            self.editor_completion.clear();
            return;
        };
        let connection_index = tab.connection_index;
        let active_database = tab.database_name.as_deref().or_else(|| {
            self.sessions
                .get(connection_index)?
                .app
                .selected_database_name()
        });
        let objects = self
            .sessions
            .get(connection_index)
            .map(|session| {
                session
                    .app
                    .databases()
                    .iter()
                    .filter(|database| {
                        active_database
                            .map(|active_database| database.name == active_database)
                            .unwrap_or(true)
                    })
                    .flat_map(|database| database.schemas.iter())
                    .flat_map(|schema| schema.objects.iter().map(|object| object.name.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let columns = self.active_preview().columns.clone();
        let items = suggest_sql_completions(&prefix, &objects, &columns);
        self.editor_completion.set_items(items);
    }

    fn open_sql_history(&mut self) {
        self.sql_history.open();
    }

    fn close_sql_history(&mut self) {
        self.sql_history.open = false;
    }

    fn move_sql_history_selection(&mut self, delta: isize) {
        self.sql_history.move_selection(delta);
    }

    fn run_sql_history_selection(&mut self) -> Result<()> {
        let sql = self
            .sql_history
            .selected_sql()
            .ok_or_else(|| anyhow!("no SQL history item is selected"))?
            .to_string();
        self.sql_history.open = false;

        let connection_index = self
            .active_editor_connection_index()
            .or_else(|| self.selected_connection_index())
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let database_name = self
            .active_editor_tab()
            .and_then(|tab| tab.database_name.clone())
            .or_else(|| self.selected_database_name().map(str::to_owned));
        self.open_editor_tab(
            connection_index,
            database_name,
            "SQL History".to_string(),
            sql,
        );
        self.execute_editor()
    }

    fn open_data_filter(&mut self) -> Result<()> {
        if self.selected_object().is_none() {
            return Err(anyhow!("select a table-like object before filtering data"));
        }
        self.select_right_tab(RightPaneTab::Data);
        self.data_filter = Some(DataFilterState {
            input: self.active_data_filter.clone().unwrap_or_default(),
        });
        Ok(())
    }

    fn close_data_filter(&mut self) {
        self.data_filter = None;
    }

    fn apply_data_filter(&mut self) -> Result<()> {
        let filter = self
            .data_filter
            .as_ref()
            .ok_or_else(|| anyhow!("data filter is not open"))?
            .input
            .trim()
            .to_string();
        self.data_filter = None;
        self.active_data_filter = (!filter.is_empty()).then_some(filter.clone());
        self.reset_preview_pagination();

        let (connection_index, object) = self
            .selected_object_target()
            .ok_or_else(|| anyhow!("select a table-like object before filtering data"))?;

        if filter.is_empty() {
            self.schedule_preview_for_connection_object(connection_index, object)
        } else {
            self.schedule_filtered_preview_for_connection_object(connection_index, object, filter)
        }
    }

    fn copy_current_cell(&mut self) -> Result<()> {
        let (row, column_index) = self.current_grid_row_and_column()?;
        let value = row
            .get(column_index)
            .ok_or_else(|| anyhow!("selected cell is no longer available"))?
            .clone();
        self.set_copied_text(value, "Copied current cell.");
        Ok(())
    }

    fn copy_current_row(&mut self) -> Result<()> {
        let (row, _) = self.current_grid_row_and_column()?;
        self.set_copied_text(copy_row_text(row), "Copied current row.");
        Ok(())
    }

    fn copy_current_where_clause(&mut self) -> Result<()> {
        let grid = self.active_grid();
        let row_index = self.grid_selected_row_index();
        let row = grid
            .rows
            .get(row_index)
            .ok_or_else(|| anyhow!("selected row is no longer available"))?;
        let key_columns = self.selected_key_columns();
        let clause = where_clause_for_row(&grid.columns, row, &key_columns);
        if clause.is_empty() {
            return Err(anyhow!(
                "could not build a WHERE clause for the selected row"
            ));
        }
        self.set_copied_text(clause, "Copied WHERE clause.");
        Ok(())
    }

    fn set_copied_text(&mut self, text: String, status: &str) {
        self.last_copied_text = Some(text);
        self.copy_sequence = self.copy_sequence.saturating_add(1);
        self.workspace_status = Some(status.to_string());
    }

    fn start_cell_edit(&mut self) -> Result<()> {
        let (connection_index, object) = self
            .selected_object_target()
            .ok_or_else(|| anyhow!("select a table-like object before editing a row"))?;
        let grid = self.active_grid();
        let row_index = self.grid_selected_row_index();
        let column_index = self.grid_selected_column_index();
        let column_name = grid
            .columns
            .get(column_index)
            .ok_or_else(|| anyhow!("selected column is no longer available"))?
            .clone();
        let input = grid
            .rows
            .get(row_index)
            .and_then(|row| row.get(column_index))
            .ok_or_else(|| anyhow!("selected cell is no longer available"))?
            .clone();

        self.cell_edit = Some(CellEditState {
            connection_index,
            object,
            row_index,
            column_index,
            column_name,
            input,
        });
        Ok(())
    }

    fn close_cell_edit(&mut self) {
        self.cell_edit = None;
    }

    fn preview_staged_crud(&mut self) -> Result<()> {
        let edit = self
            .cell_edit
            .take()
            .ok_or_else(|| anyhow!("cell edit is not open"))?;
        let key_columns = self.selected_key_columns();
        let sql = staged_update_sql(
            &edit.object,
            self.active_grid(),
            edit.row_index,
            edit.column_index,
            &edit.input,
            &key_columns,
        )
        .ok_or_else(|| anyhow!("could not build staged CRUD SQL"))?;

        self.open_editor_tab(
            edit.connection_index,
            Some(edit.object.database.clone()),
            format!("Stage UPDATE {}", edit.object.database_qualified_name()),
            sql.preview_sql.clone(),
        );
        self.staged_crud = Some(StagedCrudState {
            connection_index: edit.connection_index,
            sql,
        });
        self.workspace_status =
            Some("Preview staged UPDATE; commit with Ctrl-G or command palette.".to_string());
        Ok(())
    }

    fn commit_staged_crud(&mut self) -> Result<()> {
        let (connection_index, sql) = self
            .staged_crud
            .as_ref()
            .map(|staged| (staged.connection_index, staged.sql.commit_sql.clone()))
            .ok_or_else(|| anyhow!("no staged CRUD change is available to commit"))?;
        self.execute_sql_with_delete_confirmation(
            connection_index,
            sql,
            Some("Committing staged CRUD..."),
        )
    }

    fn open_row_inspector(&mut self) -> Result<()> {
        let grid = self.active_grid();
        if grid.columns.is_empty() || grid.rows.is_empty() {
            return Err(anyhow!("no row is available to inspect"));
        }

        let row_index = self.grid_selected_row_index();
        if row_index >= grid.rows.len() {
            return Err(anyhow!("selected row is no longer available"));
        }

        self.row_inspector = Some(RowInspectorState {
            selected_field: self.grid_selected_column_index(),
            detail_scroll: 0,
            formatted: true,
            active_pane: RowInspectorPane::Fields,
        });
        Ok(())
    }

    fn close_row_inspector(&mut self) {
        self.row_inspector = None;
    }

    fn move_row_inspector_pane(&mut self, delta: isize) {
        let Some(inspector) = self.row_inspector.as_mut() else {
            return;
        };

        inspector.active_pane = match (inspector.active_pane, delta.is_negative()) {
            (RowInspectorPane::Fields, false) | (RowInspectorPane::Fields, true) => {
                RowInspectorPane::Preview
            }
            (RowInspectorPane::Preview, false) | (RowInspectorPane::Preview, true) => {
                RowInspectorPane::Fields
            }
        };
    }

    fn move_row_inspector_field(&mut self, delta: isize) {
        let field_count = self.active_grid().columns.len();
        let Some(inspector) = self.row_inspector.as_mut() else {
            return;
        };

        if field_count == 0 {
            inspector.selected_field = 0;
            return;
        }

        let offset = delta.unsigned_abs() % field_count;
        inspector.selected_field = if delta.is_negative() {
            (inspector.selected_field + field_count - offset) % field_count
        } else {
            (inspector.selected_field + offset) % field_count
        };
        inspector.detail_scroll = 0;
        self.grid_selected_column = inspector.selected_field;
    }

    fn scroll_row_inspector_detail_by(&mut self, delta: isize) {
        let Some(inspector) = self.row_inspector.as_mut() else {
            return;
        };

        if delta.is_negative() {
            inspector.detail_scroll = inspector.detail_scroll.saturating_sub(delta.unsigned_abs());
        } else {
            inspector.detail_scroll = inspector.detail_scroll.saturating_add(delta as usize);
        }
    }

    fn row_inspector_view(&self) -> Option<RowInspectorView<'_>> {
        let inspector = self.row_inspector.as_ref()?;
        let grid = self.active_grid();
        let row_index = self.grid_selected_row_index();
        let values = grid.rows.get(row_index)?;

        Some(RowInspectorView {
            row_index,
            selected_field: inspector
                .selected_field
                .min(grid.columns.len().saturating_sub(1)),
            detail_scroll: inspector.detail_scroll,
            formatted: inspector.formatted,
            active_pane: inspector.active_pane,
            columns: &grid.columns,
            values,
        })
    }

    fn data_filter_view(&self) -> Option<DataFilterView<'_>> {
        let filter = self.data_filter.as_ref()?;
        Some(DataFilterView {
            input: &filter.input,
            active_filter: self.active_data_filter.as_deref(),
        })
    }

    fn cell_edit_view(&self) -> Option<CellEditView<'_>> {
        let edit = self.cell_edit.as_ref()?;
        Some(CellEditView {
            column: &edit.column_name,
            input: &edit.input,
        })
    }

    fn structure_view(&self) -> Option<StructureView<'_>> {
        if self.active_right_tab != RightPaneTab::Structure {
            return None;
        }

        Some(StructureView {
            object: self
                .structure
                .object
                .as_ref()
                .or_else(|| self.selected_object()),
            columns: &self.structure.columns,
            loading: self.structure.loading,
            status: self.structure.status.as_deref(),
        })
    }

    fn staged_crud_view(&self) -> Option<StagedCrudView<'_>> {
        let staged = self.staged_crud.as_ref()?;
        Some(StagedCrudView {
            preview_sql: &staged.sql.preview_sql,
            commit_sql: &staged.sql.commit_sql,
        })
    }

    fn delete_confirmation_view(&self) -> Option<DeleteConfirmationView<'_>> {
        let confirmation = self.delete_confirmation.as_ref()?;
        Some(DeleteConfirmationView {
            title: &confirmation.title,
            message: &confirmation.message,
            sql_preview: &confirmation.sql_preview,
        })
    }

    fn move_selection(&mut self, delta: isize) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }

        let previous_row = self.selected_row;
        if delta.is_negative() {
            self.selected_row = self.selected_row.saturating_sub(delta.unsigned_abs());
        } else {
            self.selected_row = self
                .selected_row
                .saturating_add(delta as usize)
                .min(self.entries.len() - 1);
        }
        if self.selected_row != previous_row {
            self.reset_grid_scroll();
            self.active_data_filter = None;
            self.reset_preview_pagination();
            self.staged_crud = None;
        }
        self.ensure_selected_object_preview()
    }

    fn toggle_selected(&mut self) -> Result<()> {
        let Some(entry) = self.entries.get(self.selected_row).cloned() else {
            return Ok(());
        };

        match entry.key {
            TreeNodeKey::Connection { connection } => {
                self.sessions[connection].expanded = !self.sessions[connection].expanded;
                self.rebuild_rows(Some(entry.key));
            }
            TreeNodeKey::Database {
                connection,
                database,
            } => {
                toggle_set(
                    &mut self.sessions[connection].expanded_databases,
                    database.clone(),
                );
                self.rebuild_rows(Some(TreeNodeKey::Database {
                    connection,
                    database,
                }));
            }
            TreeNodeKey::Schema {
                connection,
                database,
                schema,
            } => {
                toggle_set(
                    &mut self.sessions[connection].expanded_schemas,
                    (database.clone(), schema.clone()),
                );
                self.rebuild_rows(Some(TreeNodeKey::Schema {
                    connection,
                    database,
                    schema,
                }));
            }
            TreeNodeKey::Group {
                connection,
                database,
                schema,
                kind,
            } => {
                toggle_set(
                    &mut self.sessions[connection].expanded_groups,
                    (database.clone(), schema.clone(), kind),
                );
                self.rebuild_rows(Some(TreeNodeKey::Group {
                    connection,
                    database,
                    schema,
                    kind,
                }));
            }
            TreeNodeKey::Object { .. } => {
                self.ensure_selected_object_preview()?;
            }
        }

        Ok(())
    }

    fn refresh_selected_connection(&mut self) -> Result<()> {
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        self.schedule_refresh_for_connection(connection_index)
    }

    fn ensure_selected_object_preview(&mut self) -> Result<()> {
        let Some(TreeNodeKey::Object { connection, object }) = self
            .entries
            .get(self.selected_row)
            .map(|entry| entry.key.clone())
        else {
            if self.active_right_tab == RightPaneTab::Structure {
                self.structure.clear();
            }
            return Ok(());
        };

        let session = self
            .sessions
            .get_mut(connection)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        session
            .app
            .select_object_locally(&object.database, &object.schema, &object.name)?;
        self.schedule_preview_for_connection_object(connection, object.clone())?;
        if self.active_right_tab == RightPaneTab::Structure {
            self.schedule_structure_for_connection_object(connection, object)?;
        }
        Ok(())
    }

    fn open_sql_editor(&mut self) -> Result<()> {
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let database_name = self.selected_database_name().map(str::to_owned);
        let sql = self
            .selected_object()
            .map(|object| select_template(object, self.selected_preview_limit().unwrap_or(100)))
            .unwrap_or_else(|| "SELECT 1;".to_string());
        let title = self
            .selected_object()
            .map(|object| format!("SQL Editor ({})", object.database_qualified_name()))
            .unwrap_or_else(|| "SQL Editor".to_string());
        self.open_editor_tab(connection_index, database_name, title, sql);
        Ok(())
    }

    fn open_select_template(&mut self) -> Result<()> {
        let object = self.selected_object().cloned().ok_or_else(|| {
            anyhow!("select a table-like object before opening a SELECT template")
        })?;
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let sql = select_template(&object, self.selected_preview_limit().unwrap_or(100));
        self.open_editor_tab(
            connection_index,
            Some(object.database.clone()),
            format!("SELECT {}", object.database_qualified_name()),
            sql,
        );
        Ok(())
    }

    fn open_insert_template(&mut self) -> Result<()> {
        self.schedule_template_request(TemplateKind::Insert)
    }

    fn open_update_template(&mut self) -> Result<()> {
        self.schedule_template_request(TemplateKind::Update)
    }

    fn open_delete_template(&mut self) -> Result<()> {
        self.schedule_template_request(TemplateKind::Delete)
    }

    fn execute_editor(&mut self) -> Result<()> {
        let (connection_index, sql) = {
            let tab = self
                .active_editor_tab()
                .ok_or_else(|| anyhow!("sql editor is not open"))?;
            (tab.connection_index, tab.buffer.current_statement())
        };
        if sql.trim().is_empty() {
            return Err(anyhow!("current SQL statement is empty"));
        }
        self.execute_sql_with_delete_confirmation(
            connection_index,
            sql,
            Some("Executing current SQL statement..."),
        )
    }

    fn explain_current_statement(&mut self, analyze: bool) -> Result<()> {
        let (connection_index, statement) = {
            let tab = self
                .active_editor_tab()
                .ok_or_else(|| anyhow!("sql editor is not open"))?;
            (tab.connection_index, tab.buffer.current_statement())
        };
        if statement.trim().is_empty() {
            return Err(anyhow!("current SQL statement is empty"));
        }
        let sql = explain_sql(&statement, analyze);
        let status = if analyze {
            "Running EXPLAIN ANALYZE..."
        } else {
            "Running EXPLAIN..."
        };
        self.execute_sql_with_delete_confirmation(connection_index, sql, Some(status))
    }

    fn execute_sql_with_delete_confirmation(
        &mut self,
        connection_index: usize,
        sql: String,
        status: Option<&str>,
    ) -> Result<()> {
        if let Some(kind) = delete_operation_kind(&sql) {
            self.prompt_delete_operation(connection_index, sql, status.map(str::to_string), kind)?;
            return Ok(());
        }

        self.execute_sql_on_connection(connection_index, sql, status)
    }

    fn prompt_delete_operation(
        &mut self,
        connection_index: usize,
        sql: String,
        status: Option<String>,
        kind: DeleteOperationKind,
    ) -> Result<()> {
        let connection_name = self
            .sessions
            .get(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?
            .name
            .clone();
        self.delete_confirmation = Some(DeleteConfirmationState {
            title: format!("Confirm {}", kind.label()),
            message: format!("This statement can delete data or schema on `{connection_name}`."),
            sql_preview: sql_preview(&sql),
            operation: PendingDeleteOperation::ExecuteSql {
                connection_index,
                sql,
                status,
            },
        });
        self.workspace_status = Some(format!(
            "{} statement is waiting for confirmation.",
            kind.label()
        ));
        Ok(())
    }

    fn confirm_delete_operation(&mut self) -> Result<()> {
        let confirmation = self
            .delete_confirmation
            .take()
            .ok_or_else(|| anyhow!("no delete confirmation is pending"))?;

        match confirmation.operation {
            PendingDeleteOperation::ExecuteSql {
                connection_index,
                sql,
                status,
            } => self.execute_sql_on_connection(connection_index, sql, status.as_deref()),
        }
    }

    fn cancel_delete_operation(&mut self) {
        self.delete_confirmation = None;
        self.workspace_status = Some("Delete operation canceled.".to_string());
    }

    fn execute_sql_on_connection(
        &mut self,
        connection_index: usize,
        sql: String,
        status: Option<&str>,
    ) -> Result<()> {
        if self
            .active_editor_tab()
            .map(|tab| tab.connection_index != connection_index)
            .unwrap_or(true)
        {
            self.open_editor_tab(
                connection_index,
                self.selected_database_name().map(str::to_owned),
                "SQL Execution".to_string(),
                sql.clone(),
            );
        }

        let (tab_id, database_name, old_request_id) = {
            let tab = self
                .active_editor_tab()
                .ok_or_else(|| anyhow!("sql editor is not open"))?;
            (
                tab.id,
                tab.database_name.clone(),
                tab.pending_execute_request_id,
            )
        };

        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;

        if let Some(old_request_id) = old_request_id {
            session.worker.cancel_requests(vec![old_request_id])?;
            session.pending.execute_requests.remove(&old_request_id);
        }

        self.sql_history.push(sql.clone());
        let request_id = session.worker.request_sql_execution(database_name, sql)?;
        session.pending.execute_requests.insert(request_id, tab_id);

        if let Some(tab) = self.active_editor_tab_mut() {
            tab.pending_execute_request_id = Some(request_id);
            tab.result_sets.clear();
            tab.selected_result = 0;
            tab.status = Some(
                status
                    .unwrap_or("Executing SQL in background...")
                    .to_string(),
            );
            tab.rebuild_result_strip();
        }
        self.reset_grid_scroll();

        Ok(())
    }

    fn new_editor_tab(&mut self) -> Result<()> {
        let connection_index = self
            .active_editor_connection_index()
            .or_else(|| self.selected_connection_index())
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let database_name = self
            .active_editor_tab()
            .and_then(|tab| tab.database_name.clone())
            .or_else(|| self.selected_database_name().map(str::to_owned));
        let editor = self.editor.get_or_insert_with(SqlEditorState::new);
        editor.push_generated_tab(connection_index, database_name, "SELECT 1;".to_string());
        self.active_right_tab = RightPaneTab::Sql;
        self.focus_sql_editor();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    fn close_editor_tab(&mut self) -> Result<()> {
        let Some(editor) = self.editor.as_mut() else {
            return Err(anyhow!("sql editor is not open"));
        };

        if let Some(tab) = editor.active_tab() {
            let connection_index = tab.connection_index;
            let request_ids = editor
                .active_tab()
                .into_iter()
                .flat_map(|tab| tab.pending_execute_request_id.into_iter())
                .collect::<Vec<_>>();
            if !request_ids.is_empty() {
                if let Some(session) = self.sessions.get_mut(connection_index) {
                    session.worker.cancel_requests(request_ids.clone())?;
                    for request_id in &request_ids {
                        session.pending.execute_requests.remove(request_id);
                    }
                }
            }
        }

        editor.close_active_tab();
        if editor.tabs.is_empty() {
            self.editor = None;
            self.active_right_tab = RightPaneTab::Data;
            if self.browser_focus == BrowserFocus::SqlEditor {
                self.browser_focus = BrowserFocus::Assets;
            }
        }
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    fn next_editor_tab(&mut self) -> Result<()> {
        let editor = self
            .editor
            .as_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        editor.next_tab();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    fn previous_editor_tab(&mut self) -> Result<()> {
        let editor = self
            .editor
            .as_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        editor.previous_tab();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    fn next_result_set(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.next_result_set();
        self.reset_grid_scroll();
        Ok(())
    }

    fn previous_result_set(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab_mut()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        tab.previous_result_set();
        self.reset_grid_scroll();
        Ok(())
    }

    fn cancel_selected_connection_tasks(&mut self) -> Result<()> {
        let connection_index = self
            .active_editor_connection_index()
            .or_else(|| self.selected_connection_index())
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let request_ids = session.pending.request_ids();
        let canceled_structure_object = session
            .pending
            .structure_request
            .as_ref()
            .map(|request| request.object.clone());
        session.worker.cancel_requests(request_ids.clone())?;
        session.pending.clear();
        session
            .app
            .set_status(format!("Canceled {} pending task(s).", request_ids.len()));

        if let Some(editor) = self.editor.as_mut() {
            editor.cancel_requests_for_connection(connection_index, &request_ids);
        }
        if canceled_structure_object
            .as_ref()
            .is_some_and(|object| self.structure.object.as_ref() == Some(object))
        {
            self.structure.cancel_loading();
        }

        Ok(())
    }

    fn rebuild_rows(&mut self, selected_key: Option<TreeNodeKey>) {
        self.entries = self
            .sessions
            .iter()
            .enumerate()
            .flat_map(|(connection_index, session)| {
                build_rows_for_session(connection_index, session)
            })
            .collect();
        self.tree_rows = self.entries.iter().map(|entry| entry.row.clone()).collect();

        if self.entries.is_empty() {
            self.selected_row = 0;
            return;
        }

        if let Some(key) = selected_key {
            if let Some(index) = self.entries.iter().position(|entry| entry.key == key) {
                self.selected_row = index;
                return;
            }
        }

        self.selected_row = self.selected_row.min(self.entries.len() - 1);
    }

    fn first_object_row(&self) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| matches!(entry.key, TreeNodeKey::Object { .. }))
    }

    fn selected_connection_index(&self) -> Option<usize> {
        match &self.entries.get(self.selected_row)?.key {
            TreeNodeKey::Connection { connection }
            | TreeNodeKey::Database { connection, .. }
            | TreeNodeKey::Schema { connection, .. }
            | TreeNodeKey::Group { connection, .. }
            | TreeNodeKey::Object { connection, .. } => Some(*connection),
        }
    }

    fn active_editor_connection_index(&self) -> Option<usize> {
        self.editor
            .as_ref()
            .and_then(|editor| editor.active_tab())
            .map(|tab| tab.connection_index)
    }

    fn selected_session(&self) -> Option<&ConnectionSession> {
        let index = self.selected_connection_index()?;
        self.sessions.get(index)
    }

    fn selected_preview_limit(&self) -> Option<usize> {
        let session = self.selected_session()?;
        Some(session.app.preview_limit())
    }

    pub fn preview_page_offset(&self) -> usize {
        self.preview_page_offset
    }

    pub fn preview_page_number(&self) -> usize {
        self.selected_preview_limit()
            .map(|limit| {
                self.preview_page_offset / (limit * PREVIEW_PAGE_STEP_MULTIPLIER).max(1) + 1
            })
            .unwrap_or(1)
    }

    pub fn preview_has_previous_page(&self) -> bool {
        self.preview_page_offset > 0
    }

    pub fn preview_has_next_page(&self) -> bool {
        self.preview_has_next_page
    }

    pub fn preview_page_summary(&self) -> Option<String> {
        let limit = self.selected_preview_limit()?;
        let rows = self.active_preview().rows.len();
        let start = if rows == 0 {
            0
        } else {
            self.preview_page_offset + 1
        };
        let end = self.preview_page_offset + rows;
        Some(format!(
            "page {} | rows {}-{} | limit {}",
            self.preview_page_number(),
            start,
            end,
            limit
        ))
    }

    fn active_editor_tab(&self) -> Option<&SqlEditorTab> {
        self.editor.as_ref()?.active_tab()
    }

    fn active_editor_tab_mut(&mut self) -> Option<&mut SqlEditorTab> {
        self.editor.as_mut()?.active_tab_mut()
    }

    fn open_editor_tab(
        &mut self,
        connection_index: usize,
        database_name: Option<String>,
        title: String,
        sql: String,
    ) {
        let editor = self.editor.get_or_insert_with(SqlEditorState::new);
        editor.push_tab(connection_index, database_name, title, sql);
        self.active_right_tab = RightPaneTab::Sql;
        self.focus_sql_editor();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
    }

    fn selected_table_target(&self) -> Result<(usize, DbObjectRef)> {
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let object = self
            .selected_object()
            .cloned()
            .ok_or_else(|| anyhow!("select a table object first"))?;

        if object.kind == DbObjectKind::View {
            return Err(anyhow!("CRUD templates are only available for tables"));
        }

        Ok((connection_index, object))
    }

    fn current_grid_row_and_column(&self) -> Result<(&Vec<String>, usize)> {
        let grid = self.active_grid();
        let row_index = self.grid_selected_row_index();
        let column_index = self.grid_selected_column_index();
        let row = grid
            .rows
            .get(row_index)
            .ok_or_else(|| anyhow!("selected row is no longer available"))?;
        if column_index >= grid.columns.len() {
            return Err(anyhow!("selected column is no longer available"));
        }
        Ok((row, column_index))
    }

    fn selected_key_columns(&self) -> Vec<String> {
        let Some(object) = self.selected_object() else {
            return Vec::new();
        };
        if self.structure.object.as_ref() == Some(object) {
            primary_key_names(&self.structure.columns)
        } else {
            Vec::new()
        }
    }

    fn pending_task_count(&self) -> usize {
        self.sessions
            .iter()
            .map(|session| session.pending.count())
            .sum()
    }

    fn schedule_preview_for_connection_object(
        &mut self,
        connection_index: usize,
        object: DbObjectRef,
    ) -> Result<()> {
        self.schedule_preview_page_for_connection_object(connection_index, object, 0, true)
    }

    fn schedule_filtered_preview_for_connection_object(
        &mut self,
        connection_index: usize,
        object: DbObjectRef,
        filter: String,
    ) -> Result<()> {
        self.schedule_filtered_preview_page_for_connection_object(
            connection_index,
            object,
            filter,
            0,
            true,
        )
    }

    fn schedule_preview_page_for_connection_object(
        &mut self,
        connection_index: usize,
        object: DbObjectRef,
        offset: usize,
        clear_existing: bool,
    ) -> Result<()> {
        self.reset_grid_scroll();
        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;

        if let Some(previous) = &session.pending.preview_request {
            session.worker.cancel_requests(vec![previous.request_id])?;
        }

        let request_id =
            session
                .worker
                .request_preview(object.clone(), session.app.preview_limit(), offset)?;
        session.pending.preview_request = Some(PendingPreviewRequest { request_id });
        if clear_existing {
            session.app.clear_preview();
        }
        session.app.set_status(loading_preview_message(&object));
        Ok(())
    }

    fn schedule_filtered_preview_page_for_connection_object(
        &mut self,
        connection_index: usize,
        object: DbObjectRef,
        filter: String,
        offset: usize,
        clear_existing: bool,
    ) -> Result<()> {
        self.reset_grid_scroll();
        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;

        if let Some(previous) = &session.pending.preview_request {
            session.worker.cancel_requests(vec![previous.request_id])?;
        }

        let request_id = session.worker.request_filtered_preview(
            object.clone(),
            filter.clone(),
            session.app.preview_limit(),
            offset,
        )?;
        session.pending.preview_request = Some(PendingPreviewRequest { request_id });
        if clear_existing {
            session.app.clear_preview();
        }
        session.app.set_status(format!(
            "Filtering {} {} with {:?}...",
            object.kind.label(),
            object.qualified_name(),
            filter
        ));
        Ok(())
    }

    fn schedule_refresh_for_connection(&mut self, connection_index: usize) -> Result<()> {
        self.reset_grid_scroll();
        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;

        let mut cancel_ids = Vec::new();
        if let Some(request_id) = session.pending.refresh_request_id {
            cancel_ids.push(request_id);
        }
        if let Some(preview_request) = &session.pending.preview_request {
            cancel_ids.push(preview_request.request_id);
        }
        if let Some(structure_request) = &session.pending.structure_request {
            cancel_ids.push(structure_request.request_id);
        }
        session.worker.cancel_requests(cancel_ids)?;
        session.pending.refresh_request_id = None;
        session.pending.preview_request = None;
        session.pending.structure_request = None;
        if self.active_right_tab == RightPaneTab::Structure {
            self.structure.clear();
        }

        let request_id = session.worker.request_refresh(
            session.app.selected_object().cloned(),
            session.app.preview_limit(),
            self.preview_page_offset,
            self.active_data_filter.clone(),
        )?;
        session.pending.refresh_request_id = Some(request_id);
        session.app.set_status(format!(
            "Refreshing catalog for {}...",
            session.connection_label
        ));
        Ok(())
    }

    fn schedule_template_request(&mut self, kind: TemplateKind) -> Result<()> {
        let (connection_index, object) = self.selected_table_target()?;
        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;

        if let Some(previous) = &session.pending.template_request {
            session.worker.cancel_requests(vec![previous.request_id])?;
        }

        let request_id = session.worker.request_template(object.clone(), kind)?;
        session.pending.template_request = Some(PendingTemplateRequest {
            request_id,
            kind,
            object: object.clone(),
        });
        self.workspace_status = Some(format!(
            "Loading columns for {} {}...",
            kind.action_label(),
            object.qualified_name()
        ));
        Ok(())
    }

    fn ensure_selected_object_structure(&mut self) -> Result<()> {
        let Some((connection_index, object)) = self.selected_object_target() else {
            self.structure.clear();
            return Ok(());
        };

        self.schedule_structure_for_connection_object(connection_index, object)
    }

    fn selected_object_target(&self) -> Option<(usize, DbObjectRef)> {
        match &self.entries.get(self.selected_row)?.key {
            TreeNodeKey::Object { connection, object } => Some((*connection, object.clone())),
            _ => None,
        }
    }

    fn schedule_structure_for_connection_object(
        &mut self,
        connection_index: usize,
        object: DbObjectRef,
    ) -> Result<()> {
        if self.structure.matches_loaded_or_loading(&object) {
            return Ok(());
        }

        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;

        if let Some(previous) = &session.pending.structure_request {
            session.worker.cancel_requests(vec![previous.request_id])?;
        }

        let request_id = session.worker.request_structure(object.clone())?;
        session.pending.structure_request = Some(PendingStructureRequest {
            request_id,
            object: object.clone(),
        });
        self.structure.start_loading(object);
        Ok(())
    }

    fn handle_session_event(&mut self, session_index: usize, event: SessionEvent) -> Result<()> {
        match event {
            SessionEvent::PreviewLoaded {
                request_id,
                object,
                offset,
                result,
            } => {
                let current_offset = self.preview_page_offset;
                let current_row_count = self.active_preview().rows.len();
                let current_limit = self.selected_preview_limit().unwrap_or(100);
                let applied_preview = {
                    let session = self
                        .sessions
                        .get_mut(session_index)
                        .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
                    if session
                        .pending
                        .preview_request
                        .as_ref()
                        .map(|request| request.request_id)
                        != Some(request_id)
                    {
                        return Ok(());
                    }

                    session.pending.preview_request = None;
                    if session_selected_object_matches(session, &object) {
                        match result {
                            Ok(preview) if offset > 0 && preview.rows.is_empty() => {
                                self.preview_has_next_page = false;
                                let current_page = current_offset / current_limit.max(1) + 1;
                                let current_start = if current_row_count == 0 {
                                    0
                                } else {
                                    current_offset + 1
                                };
                                let current_end = current_offset + current_row_count;
                                session.app.set_status(format!(
                                    "Reached the end of {} {}. Staying on page {} (rows {}-{}).",
                                    object.kind.label(),
                                    object.qualified_name(),
                                    current_page,
                                    current_start,
                                    current_end
                                ));
                                false
                            }
                            Ok(preview) => {
                                let row_count = preview.rows.len();
                                session.app.apply_preview_result(Ok(preview));
                                self.preview_page_offset = offset;
                                self.preview_has_next_page =
                                    row_count >= session.app.preview_limit();
                                true
                            }
                            Err(error) => {
                                session.app.apply_preview_result(Err(error));
                                false
                            }
                        }
                    } else {
                        false
                    }
                };

                if applied_preview {
                    self.reset_grid_scroll();
                }
            }
            SessionEvent::CatalogRefreshed {
                request_id,
                catalog,
                preview_target,
                preview_offset,
                preview,
            } => {
                let selected_key = self
                    .entries
                    .get(self.selected_row)
                    .map(|entry| entry.key.clone());

                let mut schedule_follow_up_preview = None;
                {
                    let session = self
                        .sessions
                        .get_mut(session_index)
                        .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
                    if session.pending.refresh_request_id != Some(request_id) {
                        return Ok(());
                    }

                    session.pending.refresh_request_id = None;
                    match catalog {
                        Ok(catalog) => {
                            session.app.replace_catalog(catalog);

                            if let Some(target) = preview_target {
                                if session_selected_object_matches(session, &target) {
                                    if let Some(preview) = preview {
                                        match preview {
                                            Ok(preview) => {
                                                let row_count = preview.rows.len();
                                                session.app.apply_preview_result(Ok(preview));
                                                self.preview_page_offset = preview_offset;
                                                self.preview_has_next_page =
                                                    row_count >= session.app.preview_limit();
                                            }
                                            Err(error) => {
                                                session.app.apply_preview_result(Err(error));
                                            }
                                        }
                                    }
                                } else if session.pending.preview_request.is_none() {
                                    schedule_follow_up_preview =
                                        session.app.selected_object().cloned();
                                }
                            } else if session.pending.preview_request.is_none() {
                                schedule_follow_up_preview = session.app.selected_object().cloned();
                            }
                        }
                        Err(error) => {
                            session.app.set_status(format!("Refresh failed: {error}"));
                        }
                    }
                }

                self.rebuild_rows(selected_key);

                if let Some(object) = schedule_follow_up_preview {
                    self.schedule_preview_for_connection_object(session_index, object)?;
                }
                if self.active_right_tab == RightPaneTab::Structure {
                    self.ensure_selected_object_structure()?;
                }
            }
            SessionEvent::ColumnsLoaded {
                request_id,
                object,
                kind,
                result,
            } => {
                let pending_matches = self
                    .sessions
                    .get(session_index)
                    .and_then(|session| session.pending.template_request.as_ref())
                    .map(|pending| {
                        pending.request_id == request_id
                            && pending.object.database == object.database
                            && pending.kind == kind
                            && pending.object.schema == object.schema
                            && pending.object.name == object.name
                            && pending.object.kind == object.kind
                    })
                    .unwrap_or(false);

                if !pending_matches {
                    return Ok(());
                }

                if let Some(session) = self.sessions.get_mut(session_index) {
                    session.pending.template_request = None;
                }

                match result {
                    Ok(columns) => {
                        let sql = build_template_sql(kind, &object, &columns);
                        self.open_editor_tab(
                            session_index,
                            Some(object.database.clone()),
                            format!("{} {}", kind.title(), object.database_qualified_name()),
                            sql,
                        );
                        self.workspace_status = None;
                    }
                    Err(error) => {
                        self.workspace_status = Some(error);
                    }
                }
            }
            SessionEvent::StructureColumnsLoaded {
                request_id,
                object,
                result,
            } => {
                let pending_matches = self
                    .sessions
                    .get(session_index)
                    .and_then(|session| session.pending.structure_request.as_ref())
                    .map(|pending| pending.request_id == request_id && pending.object == object)
                    .unwrap_or(false);

                if !pending_matches {
                    return Ok(());
                }

                if let Some(session) = self.sessions.get_mut(session_index) {
                    session.pending.structure_request = None;
                }

                match result {
                    Ok(columns) => self.structure.finish_loaded(object, columns),
                    Err(error) => self.structure.finish_error(object, error),
                }
            }
            SessionEvent::SqlExecuted { request_id, result } => {
                let tab_id = {
                    let session = self
                        .sessions
                        .get_mut(session_index)
                        .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
                    let Some(tab_id) = session.pending.execute_requests.remove(&request_id) else {
                        return Ok(());
                    };
                    tab_id
                };

                match result {
                    Ok(results) => {
                        let should_refresh = results
                            .iter()
                            .any(|item| matches!(item, SqlExecutionResult::Command(_)));
                        let tab_status = execution_summary(&results);
                        let result_sets = build_result_sets(results);

                        let mut applied_sql_results = false;
                        if let Some(editor) = self.editor.as_mut() {
                            if let Some(tab) = editor.find_tab_mut_by_id(tab_id) {
                                tab.pending_execute_request_id = None;
                                tab.status = Some(tab_status.clone());
                                tab.result_sets = result_sets;
                                tab.selected_result = 0;
                                tab.rebuild_result_strip();
                                applied_sql_results = true;
                            } else {
                                self.workspace_status = Some(tab_status);
                            }
                        } else {
                            self.workspace_status = Some(tab_status);
                        }

                        if applied_sql_results {
                            self.reset_grid_scroll();
                        }

                        if should_refresh {
                            self.schedule_refresh_for_connection(session_index)?;
                        }
                    }
                    Err(error) => {
                        if let Some(editor) = self.editor.as_mut() {
                            if let Some(tab) = editor.find_tab_mut_by_id(tab_id) {
                                tab.pending_execute_request_id = None;
                                tab.status = Some(error);
                                tab.rebuild_result_strip();
                            } else {
                                self.workspace_status = Some("SQL execution failed.".to_string());
                            }
                        } else {
                            self.workspace_status = Some(error);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_error(&mut self, result: Result<()>) {
        if let Err(error) = result {
            if let Some(tab) = self.active_editor_tab_mut() {
                tab.status = Some(error.to_string());
            } else {
                self.workspace_status = Some(format!("Action failed: {error}"));
            }
        }
    }
}

impl PendingSessionWork {
    fn count(&self) -> usize {
        usize::from(self.preview_request.is_some())
            + usize::from(self.refresh_request_id.is_some())
            + usize::from(self.template_request.is_some())
            + usize::from(self.structure_request.is_some())
            + self.execute_requests.len()
    }

    fn is_busy(&self) -> bool {
        self.count() > 0
    }

    fn clear(&mut self) {
        self.preview_request = None;
        self.refresh_request_id = None;
        self.template_request = None;
        self.structure_request = None;
        self.execute_requests.clear();
    }

    fn request_ids(&self) -> Vec<u64> {
        let mut request_ids = Vec::new();
        if let Some(preview_request) = &self.preview_request {
            request_ids.push(preview_request.request_id);
        }
        if let Some(request_id) = self.refresh_request_id {
            request_ids.push(request_id);
        }
        if let Some(template_request) = &self.template_request {
            request_ids.push(template_request.request_id);
        }
        if let Some(structure_request) = &self.structure_request {
            request_ids.push(structure_request.request_id);
        }
        request_ids.extend(self.execute_requests.keys().copied());
        request_ids
    }
}

impl StructureState {
    fn matches_loaded_or_loading(&self, object: &DbObjectRef) -> bool {
        self.object.as_ref() == Some(object) && (self.loading || self.loaded)
    }

    fn start_loading(&mut self, object: DbObjectRef) {
        let qualified_name = object.qualified_name();
        self.object = Some(object);
        self.columns.clear();
        self.grid = TablePreview::default();
        self.loading = true;
        self.loaded = false;
        self.status = Some(format!("Loading structure for {qualified_name}..."));
    }

    fn finish_loaded(&mut self, object: DbObjectRef, columns: Vec<DbColumn>) {
        let grid = structure_grid_from_columns(&columns);
        self.object = Some(object);
        self.columns = columns;
        self.grid = grid;
        self.loading = false;
        self.loaded = true;
        self.status = None;
    }

    fn finish_error(&mut self, object: DbObjectRef, error: String) {
        self.object = Some(object);
        self.columns.clear();
        self.grid = TablePreview::default();
        self.loading = false;
        self.loaded = false;
        self.status = Some(format!("Failed to load structure: {error}"));
    }

    fn cancel_loading(&mut self) {
        self.loading = false;
        self.loaded = false;
        self.grid = TablePreview::default();
        self.status = Some("Canceled structure load.".to_string());
    }

    fn clear(&mut self) {
        self.object = None;
        self.columns.clear();
        self.grid = TablePreview::default();
        self.loading = false;
        self.loaded = false;
        self.status = None;
    }
}

impl CommandPaletteState {
    fn new() -> Self {
        let mut palette = Self {
            query: String::new(),
            visible_items: Vec::new(),
            selected: 0,
        };
        palette.refresh_matches();
        palette
    }

    fn view(&self) -> CommandPaletteView<'_> {
        CommandPaletteView {
            query: &self.query,
            items: &self.visible_items,
            selected_index: self.selected,
        }
    }

    fn insert_char(&mut self, ch: char) {
        self.query.push(ch);
        self.refresh_matches();
    }

    fn backspace(&mut self) {
        self.query.pop();
        self.refresh_matches();
    }

    fn move_selection(&mut self, delta: isize) {
        if self.visible_items.is_empty() {
            self.selected = 0;
            return;
        }

        let len = self.visible_items.len();
        let offset = delta.unsigned_abs() % len;
        self.selected = if delta.is_negative() {
            (self.selected + len - offset) % len
        } else {
            (self.selected + offset) % len
        };
    }

    fn selected_action(&self) -> Option<WorkspaceAction> {
        let selected_title = self.visible_items.get(self.selected)?.title;
        PALETTE_COMMANDS
            .iter()
            .find(|command| command.item.title == selected_title)
            .map(|command| command.action)
    }

    fn refresh_matches(&mut self) {
        self.visible_items = PALETTE_COMMANDS
            .iter()
            .filter(|command| command_matches_query(command, &self.query))
            .map(|command| command.item)
            .collect();
        self.selected = self
            .selected
            .min(self.visible_items.len().saturating_sub(1));
    }
}

impl SqlHistoryState {
    fn view(&self) -> Option<SqlHistoryView<'_>> {
        self.open.then_some(SqlHistoryView {
            query: &self.query,
            items: &self.visible_items,
            selected_index: self.selected,
        })
    }

    fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.refresh_matches();
    }

    fn push(&mut self, sql: String) {
        let normalized = sql.trim().to_string();
        if normalized.is_empty() || self.entries.last() == Some(&normalized) {
            return;
        }
        self.entries.push(normalized);
        self.refresh_matches();
    }

    fn insert_char(&mut self, ch: char) {
        self.query.push(ch);
        self.refresh_matches();
    }

    fn backspace(&mut self) {
        self.query.pop();
        self.refresh_matches();
    }

    fn move_selection(&mut self, delta: isize) {
        if self.visible_items.is_empty() {
            self.selected = 0;
            return;
        }

        let len = self.visible_items.len();
        let offset = delta.unsigned_abs() % len;
        self.selected = if delta.is_negative() {
            (self.selected + len - offset) % len
        } else {
            (self.selected + offset) % len
        };
    }

    fn selected_sql(&self) -> Option<&str> {
        self.visible_items.get(self.selected).map(String::as_str)
    }

    fn refresh_matches(&mut self) {
        let query = self.query.to_ascii_lowercase();
        self.visible_items = self
            .entries
            .iter()
            .rev()
            .filter(|sql| sql.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect();
        self.selected = self
            .selected
            .min(self.visible_items.len().saturating_sub(1));
    }
}

impl EditorCompletionState {
    fn view(&self) -> Option<EditorCompletionView<'_>> {
        (!self.items.is_empty()).then_some(EditorCompletionView {
            items: &self.items,
            selected_index: self.selected,
        })
    }

    fn set_items(&mut self, items: Vec<CompletionItem>) {
        self.items = items;
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
    }

    fn clear(&mut self) {
        self.items.clear();
        self.selected = 0;
    }

    fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.items.len();
        let offset = delta.unsigned_abs() % len;
        self.selected = if delta.is_negative() {
            (self.selected + len - offset) % len
        } else {
            (self.selected + offset) % len
        };
    }

    fn selected_label(&self) -> Option<&str> {
        Some(self.items.get(self.selected)?.label.as_str())
    }
}

impl SqlEditorState {
    fn new() -> Self {
        Self {
            tabs: Vec::new(),
            selected_tab: 0,
            next_tab_number: 1,
            tab_strip: String::new(),
        }
    }

    fn push_tab(
        &mut self,
        connection_index: usize,
        database_name: Option<String>,
        title: String,
        sql: String,
    ) {
        let id = self.next_tab_number;
        self.next_tab_number += 1;
        self.tabs.push(SqlEditorTab::new(
            id,
            connection_index,
            database_name,
            title,
            sql,
        ));
        self.selected_tab = self.tabs.len().saturating_sub(1);
        self.rebuild_tab_strip();
    }

    fn push_generated_tab(
        &mut self,
        connection_index: usize,
        database_name: Option<String>,
        sql: String,
    ) {
        let title = format!("SQL Tab {}", self.next_tab_number);
        self.push_tab(connection_index, database_name, title, sql);
    }

    fn active_tab(&self) -> Option<&SqlEditorTab> {
        self.tabs.get(self.selected_tab)
    }

    fn active_tab_mut(&mut self) -> Option<&mut SqlEditorTab> {
        self.tabs.get_mut(self.selected_tab)
    }

    fn find_tab_mut_by_id(&mut self, tab_id: usize) -> Option<&mut SqlEditorTab> {
        self.tabs.iter_mut().find(|tab| tab.id == tab_id)
    }

    fn close_active_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }

        self.tabs.remove(self.selected_tab);
        if !self.tabs.is_empty() {
            self.selected_tab = self.selected_tab.min(self.tabs.len() - 1);
        } else {
            self.selected_tab = 0;
        }
        self.rebuild_tab_strip();
    }

    fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.selected_tab = (self.selected_tab + 1) % self.tabs.len();
        self.rebuild_tab_strip();
    }

    fn previous_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.selected_tab = (self.selected_tab + self.tabs.len() - 1) % self.tabs.len();
        self.rebuild_tab_strip();
    }

    fn active_result_grid(&self) -> Option<&TablePreview> {
        self.active_tab()?.active_result_grid()
    }

    fn cancel_requests_for_connection(&mut self, connection_index: usize, request_ids: &[u64]) {
        for tab in &mut self.tabs {
            if tab.connection_index == connection_index
                && tab
                    .pending_execute_request_id
                    .is_some_and(|request_id| request_ids.contains(&request_id))
            {
                tab.pending_execute_request_id = None;
                tab.status = Some("Canceled pending task(s).".to_string());
            }
        }
        self.rebuild_tab_strip();
    }

    fn view(&self) -> EditorView<'_> {
        let tab = self
            .active_tab()
            .expect("editor view requires an active tab");
        let (cursor_row, cursor_col) = tab.buffer.cursor();
        EditorView {
            title: &tab.title,
            tab_strip: &self.tab_strip,
            tab_count: self.tabs.len(),
            selected_tab_index: self.selected_tab,
            lines: tab.buffer.lines(),
            cursor_row,
            cursor_col,
            result_strip: (!tab.result_strip.is_empty()).then_some(tab.result_strip.as_str()),
            result_set_count: tab.result_sets.len(),
            selected_result_index: tab.selected_result,
            status: tab.status.as_deref(),
        }
    }

    fn rebuild_tab_strip(&mut self) {
        if self.tabs.is_empty() {
            self.tab_strip.clear();
            return;
        }

        self.tab_strip = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let active = if index == self.selected_tab { "*" } else { " " };
                let running = if tab.pending_execute_request_id.is_some() {
                    "..."
                } else {
                    ""
                };
                format!("[{active}{}{}]", tab.title, running)
            })
            .collect::<Vec<_>>()
            .join(" ");
    }
}

impl SqlEditorTab {
    fn new(
        id: usize,
        connection_index: usize,
        database_name: Option<String>,
        title: String,
        sql: String,
    ) -> Self {
        Self {
            id,
            connection_index,
            database_name,
            title,
            buffer: SqlEditorBuffer::from_sql(&sql),
            status: None,
            result_sets: Vec::new(),
            selected_result: 0,
            pending_execute_request_id: None,
            result_strip: String::new(),
        }
    }

    fn active_result_grid(&self) -> Option<&TablePreview> {
        self.result_sets
            .get(self.selected_result)
            .map(|result| &result.grid)
    }

    fn select_result_set(&mut self, index: usize) -> Result<()> {
        if index >= self.result_sets.len() {
            return Err(anyhow!("sql result set index is out of range"));
        }
        self.selected_result = index;
        self.rebuild_result_strip();
        Ok(())
    }

    fn next_result_set(&mut self) {
        if self.result_sets.is_empty() {
            return;
        }
        self.selected_result = (self.selected_result + 1) % self.result_sets.len();
        self.rebuild_result_strip();
    }

    fn previous_result_set(&mut self) {
        if self.result_sets.is_empty() {
            return;
        }
        self.selected_result =
            (self.selected_result + self.result_sets.len() - 1) % self.result_sets.len();
        self.rebuild_result_strip();
    }

    fn rebuild_result_strip(&mut self) {
        if self.result_sets.is_empty() {
            self.result_strip.clear();
            return;
        }

        self.result_strip = self
            .result_sets
            .iter()
            .enumerate()
            .map(|(index, result)| {
                let active = if index == self.selected_result {
                    "*"
                } else {
                    " "
                };
                format!("[{active}{}]", result.title)
            })
            .collect::<Vec<_>>()
            .join(" ");
    }
}

fn build_rows_for_session(connection_index: usize, session: &ConnectionSession) -> Vec<TreeEntry> {
    let mut rows = vec![TreeEntry {
        row: TreeRow::new(
            session.name.clone(),
            0,
            true,
            session.expanded,
            Some(session.connection_label.clone()),
        ),
        key: TreeNodeKey::Connection {
            connection: connection_index,
        },
    }];

    if !session.expanded {
        return rows;
    }

    let show_database_nodes = session.app.databases().len() > 1;

    for database in session.app.databases() {
        let database_depth = usize::from(show_database_nodes);
        if show_database_nodes {
            let database_expanded = session.expanded_databases.contains(&database.name);
            rows.push(TreeEntry {
                row: TreeRow::new(
                    database.name.clone(),
                    1,
                    true,
                    database_expanded,
                    Some(database.object_count().to_string()),
                ),
                key: TreeNodeKey::Database {
                    connection: connection_index,
                    database: database.name.clone(),
                },
            });

            if !database_expanded {
                continue;
            }
        }

        for schema in &database.schemas {
            let schema_expanded = session
                .expanded_schemas
                .contains(&(database.name.clone(), schema.name.clone()));
            rows.push(TreeEntry {
                row: TreeRow::new(
                    schema.name.clone(),
                    database_depth + 1,
                    true,
                    schema_expanded,
                    Some(schema.objects.len().to_string()),
                ),
                key: TreeNodeKey::Schema {
                    connection: connection_index,
                    database: database.name.clone(),
                    schema: schema.name.clone(),
                },
            });

            if !schema_expanded {
                continue;
            }

            for kind in DbObjectKind::ordered() {
                let objects = schema.objects_of_kind(kind).cloned().collect::<Vec<_>>();
                if objects.is_empty() {
                    continue;
                }

                let group_expanded = session.expanded_groups.contains(&(
                    database.name.clone(),
                    schema.name.clone(),
                    kind,
                ));
                rows.push(TreeEntry {
                    row: TreeRow::new(
                        kind.group_label(),
                        database_depth + 2,
                        true,
                        group_expanded,
                        Some(objects.len().to_string()),
                    ),
                    key: TreeNodeKey::Group {
                        connection: connection_index,
                        database: database.name.clone(),
                        schema: schema.name.clone(),
                        kind,
                    },
                });

                if !group_expanded {
                    continue;
                }

                for object in objects {
                    rows.push(TreeEntry {
                        row: TreeRow::new(
                            object.name.clone(),
                            database_depth + 3,
                            false,
                            false,
                            Some(kind.label().to_string()),
                        ),
                        key: TreeNodeKey::Object {
                            connection: connection_index,
                            object,
                        },
                    });
                }
            }
        }
    }

    rows
}

fn toggle_set<T>(set: &mut BTreeSet<T>, value: T)
where
    T: Ord + Clone,
{
    if !set.insert(value.clone()) {
        set.remove(&value);
    }
}

fn command_matches_query(command: &PaletteCommand, query: &str) -> bool {
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() {
        return true;
    }

    let haystack = format!(
        "{} {}",
        command.item.title.to_ascii_lowercase(),
        command.item.hint.to_ascii_lowercase()
    );
    normalized_query
        .split_whitespace()
        .all(|token| haystack.contains(token))
}

fn session_selected_object_matches(session: &ConnectionSession, object: &DbObjectRef) -> bool {
    session
        .app
        .selected_object()
        .map(|selected| {
            selected.database == object.database
                && selected.schema == object.schema
                && selected.name == object.name
                && selected.kind == object.kind
        })
        .unwrap_or(false)
}

fn loading_preview_message(object: &DbObjectRef) -> String {
    format!(
        "Loading preview for {} {}...",
        object.kind.label(),
        object.qualified_name()
    )
}

fn delete_operation_kind(sql: &str) -> Option<DeleteOperationKind> {
    let tokens = sql_keyword_tokens(sql);
    if tokens.is_empty() {
        return None;
    }

    if tokens.first().is_some_and(|token| token == "EXPLAIN") {
        if tokens.iter().any(|token| token == "ANALYZE") {
            return destructive_keyword_in(&tokens[1..]);
        }
        return None;
    }

    destructive_keyword_in(&tokens)
}

fn destructive_keyword_in(tokens: &[String]) -> Option<DeleteOperationKind> {
    if tokens.iter().any(|token| token == "DELETE") {
        return Some(DeleteOperationKind::Delete);
    }
    if tokens.iter().any(|token| token == "DROP") {
        return Some(DeleteOperationKind::Drop);
    }
    if tokens.iter().any(|token| token == "TRUNCATE") {
        return Some(DeleteOperationKind::Truncate);
    }
    None
}

fn sql_keyword_tokens(sql: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' => skip_quoted(&mut chars, '\''),
            '"' => skip_quoted(&mut chars, '"'),
            '`' => skip_quoted(&mut chars, '`'),
            '$' => skip_dollar_quoted(&mut chars),
            '-' if chars.peek() == Some(&'-') => {
                chars.next();
                for next in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = '\0';
                for next in chars.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
            }
            ch if ch.is_ascii_alphabetic() || ch == '_' => {
                let mut token = String::from(ch.to_ascii_uppercase());
                while let Some(next) = chars.peek().copied() {
                    if next.is_ascii_alphanumeric() || next == '_' {
                        token.push(next.to_ascii_uppercase());
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(token);
            }
            _ => {}
        }
    }

    tokens
}

fn skip_quoted<I>(chars: &mut std::iter::Peekable<I>, quote: char)
where
    I: Iterator<Item = char>,
{
    while let Some(ch) = chars.next() {
        if ch == quote {
            if quote == '\'' && chars.peek() == Some(&'\'') {
                chars.next();
                continue;
            }
            break;
        }
    }
}

fn skip_dollar_quoted<I>(chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    let mut delimiter = String::from("$");
    let mut found_opening_delimiter = false;

    while let Some(next) = chars.peek().copied() {
        chars.next();
        delimiter.push(next);
        if next == '$' {
            found_opening_delimiter = true;
            break;
        }
        if !(next.is_ascii_alphanumeric() || next == '_') {
            break;
        }
    }

    if !found_opening_delimiter {
        return;
    }

    let mut tail = String::new();
    for next in chars.by_ref() {
        tail.push(next);
        if tail.ends_with(&delimiter) {
            break;
        }
    }
}

fn sql_preview(sql: &str) -> String {
    const MAX_SQL_PREVIEW_CHARS: usize = 480;
    let trimmed = sql.trim();
    let mut preview = trimmed
        .chars()
        .take(MAX_SQL_PREVIEW_CHARS)
        .collect::<String>();
    if trimmed.chars().count() > MAX_SQL_PREVIEW_CHARS {
        preview.push('…');
    }
    preview
}

fn build_template_sql(kind: TemplateKind, object: &DbObjectRef, columns: &[DbColumn]) -> String {
    match kind {
        TemplateKind::Insert => insert_template(object, columns),
        TemplateKind::Update => update_template(object, columns),
        TemplateKind::Delete => delete_template(object, columns),
    }
}

fn structure_grid_from_columns(columns: &[DbColumn]) -> TablePreview {
    TablePreview {
        columns: ["#", "column", "type", "nullable", "default", "key"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        rows: columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                vec![
                    (index + 1).to_string(),
                    column.name.clone(),
                    column.data_type.clone(),
                    if column.nullable { "YES" } else { "NO" }.to_string(),
                    if column.has_default { "YES" } else { "" }.to_string(),
                    if column.is_primary_key { "PK" } else { "" }.to_string(),
                ]
            })
            .collect(),
    }
}

fn build_result_sets(results: Vec<SqlExecutionResult>) -> Vec<EditorResultSet> {
    results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            let title = match &result {
                SqlExecutionResult::Query(query) => {
                    format!("Result {} ({} row(s))", index + 1, query.row_count())
                }
                SqlExecutionResult::Command(command) => {
                    format!("{} ({})", command.tag, command.rows_affected)
                }
            };
            EditorResultSet {
                title,
                grid: result.into_preview(),
            }
        })
        .collect()
}

fn execution_summary(results: &[SqlExecutionResult]) -> String {
    if results.is_empty() {
        return "SQL batch completed with no result sets.".to_string();
    }

    let query_count = results
        .iter()
        .filter(|result| matches!(result, SqlExecutionResult::Query(_)))
        .count();
    let command_count = results.len() - query_count;

    match (query_count, command_count) {
        (0, 1) => match &results[0] {
            SqlExecutionResult::Command(command) => command.summary(),
            _ => unreachable!(),
        },
        (1, 0) => match &results[0] {
            SqlExecutionResult::Query(query) => {
                format!("Query returned {} row(s).", query.row_count())
            }
            _ => unreachable!(),
        },
        _ => format!(
            "SQL batch produced {} result set(s): {} query result(s), {} command result(s).",
            results.len(),
            query_count,
            command_count
        ),
    }
}

impl TemplateKind {
    fn action_label(self) -> &'static str {
        match self {
            Self::Insert => "INSERT",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
        }
    }

    fn title(self) -> &'static str {
        self.action_label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_operation_detector_covers_destructive_sql() {
        assert_eq!(
            delete_operation_kind("delete from users where id = 1"),
            Some(DeleteOperationKind::Delete)
        );
        assert_eq!(
            delete_operation_kind("drop table users"),
            Some(DeleteOperationKind::Drop)
        );
        assert_eq!(
            delete_operation_kind("truncate table users"),
            Some(DeleteOperationKind::Truncate)
        );
        assert_eq!(
            delete_operation_kind("alter table users drop column email"),
            Some(DeleteOperationKind::Drop)
        );
        assert_eq!(
            delete_operation_kind("explain analyze delete from users"),
            Some(DeleteOperationKind::Delete)
        );
    }

    #[test]
    fn delete_operation_detector_ignores_safe_mentions() {
        assert_eq!(delete_operation_kind("select 'delete from users'"), None);
        assert_eq!(delete_operation_kind("select $$delete from users$$"), None);
        assert_eq!(
            delete_operation_kind("-- delete from users\nselect 1"),
            None
        );
        assert_eq!(delete_operation_kind("explain delete from users"), None);
    }
}
