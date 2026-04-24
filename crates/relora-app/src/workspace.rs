use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, anyhow};
use relora_core::{
    app::App as ConnectionApp,
    db::{
        CatalogSummary, DatabaseKind, DbColumn, DbObjectKind, DbObjectRef, DriverCapabilities,
        SqlExecutionResult, TablePreview,
    },
};

use crate::{
    background::{SessionEvent, SessionWorker, TemplateKind},
    completion::{CompletionItem, suggest_sql_completions},
    editor::SqlEditorBuffer,
    sql_tools::{
        StagedCrudSql, copy_row_text, explain_sql, primary_key_names, staged_delete_sql,
        staged_insert_sql, staged_update_sql, where_clause_for_row,
    },
    templates::{
        AddColumnTemplate, AlterColumnTemplate, CreateIndexTemplate, CreateTableColumnTemplate,
        RenameTableTemplate, add_column_template, add_primary_key_template,
        add_unique_constraint_template, alter_column_template, create_index_template,
        create_table_template, delete_template, drop_column_template, drop_index_template,
        drop_primary_key_template, drop_unique_constraint_template, insert_template,
        rename_table_template, select_template, update_template,
    },
    tree::{TreeEntry, TreeNodeKey, TreeRow},
    view::{
        AddColumnFieldFocusView, AddColumnFormSnapshot, AlterColumnFieldFocusView,
        AlterColumnFormSnapshot, CellEditView, CommandPaletteItemView, CommandPaletteView,
        CreateIndexFormSnapshot, CreateTableColumnSnapshot, CreateTableFieldFocusView,
        CreateTableFormSnapshot, DataFilterView, DeleteConfirmationView, DropIndexFormSnapshot,
        EditorCompletionView, EditorView, InsertRowDatePickerSnapshot,
        InsertRowDateTimeSegmentView, InsertRowFieldKindView, InsertRowFieldSnapshot,
        InsertRowFormSnapshot, RenameTableFormSnapshot, RightPaneTab, RightPaneTabView,
        RowInspectorPane, RowInspectorView, SaveSqlDialogView, SavedSqlView, SqlHistoryView,
        StagedCrudView, StructureEditorColumnSnapshot, StructureEditorFieldFocusView,
        StructureEditorFormSnapshot, StructureView, WorkspaceView,
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
    OpenSelectedTreeItemDefault,
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
    OpenHelpOverlay,
    CloseHelpOverlay,
    NextCommandPaletteItem,
    PreviousCommandPaletteItem,
    ExecuteCommandPaletteSelection,
    OpenSqlHistory,
    CloseSqlHistory,
    NextSqlHistoryItem,
    PreviousSqlHistoryItem,
    RunSqlHistorySelection,
    OpenSavedSql,
    CloseSavedSql,
    NextSavedSqlItem,
    PreviousSavedSqlItem,
    OpenSavedSqlSelection,
    OpenSaveSqlDialog,
    CloseSaveSqlDialog,
    ConfirmSaveSql,
    OpenCreateTableForm,
    CloseCreateTableForm,
    NextCreateTableField,
    PreviousCreateTableField,
    MoveCreateTableFieldLeft,
    MoveCreateTableFieldRight,
    CycleCreateTableColumnTypeNext,
    CycleCreateTableColumnTypePrevious,
    ToggleCreateTableColumnNullable,
    ToggleCreateTableColumnUnique,
    ToggleCreateTableColumnAutoIncrement,
    ToggleCreateTableColumnPrimaryKey,
    AddCreateTableColumn,
    RemoveCreateTableColumn,
    PreviewCreateTableForm,
    OpenStructureEditor,
    CloseStructureEditorForm,
    NextStructureEditorField,
    PreviousStructureEditorField,
    MoveStructureEditorFieldLeft,
    MoveStructureEditorFieldRight,
    CycleStructureEditorColumnTypeNext,
    CycleStructureEditorColumnTypePrevious,
    ToggleStructureEditorNullable,
    ToggleStructureEditorUnique,
    ToggleStructureEditorPrimaryKey,
    AddStructureEditorColumn,
    RemoveStructureEditorColumn,
    PreviewStructureEditorForm,
    OpenAlterColumnForm,
    CloseAlterColumnForm,
    NextAlterColumnField,
    PreviousAlterColumnField,
    CycleAlterColumnTypeNext,
    CycleAlterColumnTypePrevious,
    ToggleAlterColumnNullable,
    PreviewAlterColumnForm,
    OpenAddColumnForm,
    CloseAddColumnForm,
    NextAddColumnField,
    PreviousAddColumnField,
    CycleAddColumnTypeNext,
    CycleAddColumnTypePrevious,
    ToggleAddColumnNullable,
    PreviewAddColumnForm,
    OpenRenameTableForm,
    CloseRenameTableForm,
    PreviewRenameTableForm,
    PromptDropStructureColumn,
    OpenCreateIndexForm,
    CloseCreateIndexForm,
    ToggleCreateIndexUnique,
    PreviewCreateIndexForm,
    OpenDropIndexForm,
    CloseDropIndexForm,
    PreviewDropIndexForm,
    OpenInsertRowForm,
    CloseInsertRowForm,
    NextInsertRowField,
    PreviousInsertRowField,
    PreviewInsertRowForm,
    DeleteSavedSqlFromEditor,
    OpenDataFilter,
    CloseDataFilter,
    ApplyDataFilter,
    CopyCurrentCell,
    CopyCurrentRow,
    CopyCurrentWhereClause,
    StartCellEdit,
    PreviewDeleteCurrentRow,
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
            title: "Open Saved SQL",
            hint: "Search saved SQL snippets and open them in the editor",
        },
        action: WorkspaceAction::OpenSavedSql,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Save Current SQL",
            hint: "Save the active SQL editor buffer for later reuse",
        },
        action: WorkspaceAction::OpenSaveSqlDialog,
    },
    PaletteCommand {
        item: CommandPaletteItemView {
            title: "Create Table",
            hint: "Open a form that generates CREATE TABLE SQL in the current schema",
        },
        action: WorkspaceAction::OpenCreateTableForm,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSqlEntry {
    pub name: String,
    pub sql: String,
    pub connection_name: Option<String>,
    pub database_name: Option<String>,
    pub schema_name: Option<String>,
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
    help_overlay_visible: bool,
    editor_completion: EditorCompletionState,
    sql_history: SqlHistoryState,
    saved_sql: SavedSqlState,
    save_sql_dialog: Option<SaveSqlDialogState>,
    create_table_form: Option<CreateTableFormState>,
    structure_editor_form: Option<StructureEditorFormState>,
    alter_column_form: Option<AlterColumnFormState>,
    add_column_form: Option<AddColumnFormState>,
    rename_table_form: Option<RenameTableFormState>,
    create_index_form: Option<CreateIndexFormState>,
    drop_index_form: Option<DropIndexFormState>,
    insert_row_form: Option<InsertRowFormState>,
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
    capabilities: DriverCapabilities,
    read_only: bool,
    catalog_summary: CatalogSummary,
    app: ConnectionApp,
    worker: SessionWorker,
    expanded: bool,
    expanded_databases: BTreeSet<String>,
    expanded_schemas: BTreeSet<(String, String)>,
    expanded_groups: BTreeSet<(String, String, DbObjectKind)>,
    expanded_saved_query_groups: BTreeSet<(String, String)>,
    loaded_groups: BTreeSet<(String, String, DbObjectKind)>,
    pending: PendingSessionWork,
}

#[derive(Default)]
struct PendingSessionWork {
    preview_request: Option<PendingPreviewRequest>,
    refresh_request: Option<PendingRefreshRequest>,
    group_request_ids: BTreeMap<(String, String, DbObjectKind), u64>,
    template_request: Option<PendingTemplateRequest>,
    structure_request: Option<PendingStructureRequest>,
    execute_requests: BTreeMap<u64, PendingExecuteRequest>,
}

struct PendingPreviewRequest {
    request_id: u64,
}

struct PendingRefreshRequest {
    request_id: u64,
    selection_target: Option<DbObjectRef>,
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

struct PendingExecuteRequest {
    tab_id: usize,
    refresh_target: Option<DbObjectRef>,
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

struct SaveSqlDialogState {
    name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateTableFieldFocus {
    TableName,
    ColumnName,
    ColumnType,
    DefaultValue,
    Nullable,
    Unique,
    AutoIncrement,
    PrimaryKey,
}

impl CreateTableFieldFocus {
    fn cycle(self, delta: isize) -> Self {
        const ORDER: [CreateTableFieldFocus; 7] = [
            CreateTableFieldFocus::ColumnName,
            CreateTableFieldFocus::ColumnType,
            CreateTableFieldFocus::DefaultValue,
            CreateTableFieldFocus::Nullable,
            CreateTableFieldFocus::Unique,
            CreateTableFieldFocus::AutoIncrement,
            CreateTableFieldFocus::PrimaryKey,
        ];
        let current_index = ORDER.iter().position(|focus| *focus == self).unwrap_or(0);
        let len = ORDER.len();
        let offset = delta.unsigned_abs() % len;
        let next_index = if delta.is_negative() {
            (current_index + len - offset) % len
        } else {
            (current_index + offset) % len
        };
        ORDER[next_index]
    }

    fn into_view(self) -> CreateTableFieldFocusView {
        match self {
            Self::TableName => CreateTableFieldFocusView::TableName,
            Self::ColumnName => CreateTableFieldFocusView::ColumnName,
            Self::ColumnType => CreateTableFieldFocusView::ColumnType,
            Self::DefaultValue => CreateTableFieldFocusView::DefaultValue,
            Self::Nullable => CreateTableFieldFocusView::Nullable,
            Self::Unique => CreateTableFieldFocusView::Unique,
            Self::AutoIncrement => CreateTableFieldFocusView::AutoIncrement,
            Self::PrimaryKey => CreateTableFieldFocusView::PrimaryKey,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CreateTableTypeOption {
    label: &'static str,
    sql: &'static str,
}

#[derive(Clone)]
struct CreateTableColumnState {
    name: String,
    type_index: usize,
    default_value: String,
    nullable: bool,
    unique: bool,
    auto_increment: bool,
    primary_key: bool,
}

struct CreateTableFormState {
    connection_index: usize,
    database_name: String,
    schema_name: String,
    table_name: String,
    selected_row: usize,
    selected_focus: CreateTableFieldFocus,
    columns: Vec<CreateTableColumnState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlterColumnFieldFocus {
    ColumnName,
    ColumnType,
    DefaultValue,
    Nullable,
}

impl AlterColumnFieldFocus {
    fn cycle(self, delta: isize) -> Self {
        const ORDER: [AlterColumnFieldFocus; 4] = [
            AlterColumnFieldFocus::ColumnName,
            AlterColumnFieldFocus::ColumnType,
            AlterColumnFieldFocus::DefaultValue,
            AlterColumnFieldFocus::Nullable,
        ];
        let current_index = ORDER.iter().position(|focus| *focus == self).unwrap_or(0);
        let len = ORDER.len();
        let offset = delta.unsigned_abs() % len;
        let next_index = if delta.is_negative() {
            (current_index + len - offset) % len
        } else {
            (current_index + offset) % len
        };
        ORDER[next_index]
    }

    fn into_view(self) -> AlterColumnFieldFocusView {
        match self {
            Self::ColumnName => AlterColumnFieldFocusView::ColumnName,
            Self::ColumnType => AlterColumnFieldFocusView::ColumnType,
            Self::DefaultValue => AlterColumnFieldFocusView::DefaultValue,
            Self::Nullable => AlterColumnFieldFocusView::Nullable,
        }
    }
}

struct AlterColumnFormState {
    connection_index: usize,
    database_name: String,
    schema_name: String,
    table_name: String,
    old_name: String,
    new_name: String,
    old_data_type: String,
    type_index: usize,
    old_nullable: bool,
    nullable: bool,
    default_value: String,
    selected_focus: AlterColumnFieldFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddColumnFieldFocus {
    ColumnName,
    ColumnType,
    Nullable,
    DefaultValue,
}

impl AddColumnFieldFocus {
    fn cycle(self, delta: isize) -> Self {
        const ORDER: [AddColumnFieldFocus; 4] = [
            AddColumnFieldFocus::ColumnName,
            AddColumnFieldFocus::ColumnType,
            AddColumnFieldFocus::DefaultValue,
            AddColumnFieldFocus::Nullable,
        ];
        let current_index = ORDER.iter().position(|focus| *focus == self).unwrap_or(0);
        let len = ORDER.len();
        let offset = delta.unsigned_abs() % len;
        let next_index = if delta.is_negative() {
            (current_index + len - offset) % len
        } else {
            (current_index + offset) % len
        };
        ORDER[next_index]
    }

    fn into_view(self) -> AddColumnFieldFocusView {
        match self {
            Self::ColumnName => AddColumnFieldFocusView::ColumnName,
            Self::ColumnType => AddColumnFieldFocusView::ColumnType,
            Self::Nullable => AddColumnFieldFocusView::Nullable,
            Self::DefaultValue => AddColumnFieldFocusView::DefaultValue,
        }
    }
}

struct AddColumnFormState {
    connection_index: usize,
    database_name: String,
    schema_name: String,
    table_name: String,
    name: String,
    type_index: usize,
    nullable: bool,
    default_value: String,
    selected_focus: AddColumnFieldFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructureEditorFieldFocus {
    TableName,
    ColumnName,
    ColumnType,
    DefaultValue,
    Nullable,
    Unique,
    PrimaryKey,
}

impl StructureEditorFieldFocus {
    fn cycle(self, delta: isize) -> Self {
        const ORDER: [StructureEditorFieldFocus; 6] = [
            StructureEditorFieldFocus::ColumnName,
            StructureEditorFieldFocus::ColumnType,
            StructureEditorFieldFocus::DefaultValue,
            StructureEditorFieldFocus::Nullable,
            StructureEditorFieldFocus::Unique,
            StructureEditorFieldFocus::PrimaryKey,
        ];
        let current_index = ORDER.iter().position(|focus| *focus == self).unwrap_or(0);
        let len = ORDER.len();
        let offset = delta.unsigned_abs() % len;
        let next_index = if delta.is_negative() {
            (current_index + len - offset) % len
        } else {
            (current_index + offset) % len
        };
        ORDER[next_index]
    }

    fn into_view(self) -> StructureEditorFieldFocusView {
        match self {
            Self::TableName => StructureEditorFieldFocusView::TableName,
            Self::ColumnName => StructureEditorFieldFocusView::ColumnName,
            Self::ColumnType => StructureEditorFieldFocusView::ColumnType,
            Self::DefaultValue => StructureEditorFieldFocusView::DefaultValue,
            Self::Nullable => StructureEditorFieldFocusView::Nullable,
            Self::Unique => StructureEditorFieldFocusView::Unique,
            Self::PrimaryKey => StructureEditorFieldFocusView::PrimaryKey,
        }
    }
}

#[derive(Clone)]
struct StructureEditorColumnState {
    original_name: Option<String>,
    original_data_type: Option<String>,
    original_nullable: Option<bool>,
    original_default: Option<String>,
    original_unique: Option<bool>,
    original_primary_key: Option<bool>,
    name: String,
    data_type: String,
    default_value: String,
    nullable: bool,
    unique: bool,
    primary_key: bool,
}

struct StructureEditorFormState {
    connection_index: usize,
    database_name: String,
    schema_name: String,
    old_table_name: String,
    object_kind: DbObjectKind,
    table_name: String,
    selected_row: usize,
    selected_focus: StructureEditorFieldFocus,
    anchor_row: usize,
    columns: Vec<StructureEditorColumnState>,
}

struct RenameTableFormState {
    connection_index: usize,
    database_name: String,
    schema_name: String,
    old_name: String,
    new_name: String,
}

struct CreateIndexFormState {
    connection_index: usize,
    database_name: String,
    schema_name: String,
    table_name: String,
    column_name: String,
    index_name: String,
    unique: bool,
}

struct DropIndexFormState {
    connection_index: usize,
    database_name: String,
    schema_name: String,
    table_name: String,
    index_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertRowFieldKind {
    Text,
    Number,
    Boolean,
    Date,
    DateTime,
    Json,
}

impl InsertRowFieldKind {
    fn supports_date_picker(self) -> bool {
        matches!(self, Self::Date | Self::DateTime)
    }

    fn supports_time_picker(self) -> bool {
        matches!(self, Self::DateTime)
    }

    fn into_view(self) -> InsertRowFieldKindView {
        match self {
            Self::Text => InsertRowFieldKindView::Text,
            Self::Number => InsertRowFieldKindView::Number,
            Self::Boolean => InsertRowFieldKindView::Boolean,
            Self::Date => InsertRowFieldKindView::Date,
            Self::DateTime => InsertRowFieldKindView::DateTime,
            Self::Json => InsertRowFieldKindView::Json,
        }
    }
}

impl InsertRowDateTimeSegment {
    fn default_for_kind(kind: InsertRowFieldKind) -> Option<Self> {
        kind.supports_time_picker().then_some(Self::Day)
    }

    fn cycle(self, delta: isize) -> Self {
        const ORDER: [InsertRowDateTimeSegment; 4] = [
            InsertRowDateTimeSegment::Day,
            InsertRowDateTimeSegment::Hour,
            InsertRowDateTimeSegment::Minute,
            InsertRowDateTimeSegment::Second,
        ];
        let current_index = ORDER
            .iter()
            .position(|segment| *segment == self)
            .unwrap_or(0);
        let len = ORDER.len();
        let offset = delta.unsigned_abs() % len;
        let next_index = if delta.is_negative() {
            (current_index + len - offset) % len
        } else {
            (current_index + offset) % len
        };
        ORDER[next_index]
    }

    fn into_view(self) -> InsertRowDateTimeSegmentView {
        match self {
            Self::Day => InsertRowDateTimeSegmentView::Day,
            Self::Hour => InsertRowDateTimeSegmentView::Hour,
            Self::Minute => InsertRowDateTimeSegmentView::Minute,
            Self::Second => InsertRowDateTimeSegmentView::Second,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InsertRowDateValue {
    year: i32,
    month: u8,
    day: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InsertRowTimeValue {
    hour: u8,
    minute: u8,
    second: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertRowDateTimeSegment {
    Day,
    Hour,
    Minute,
    Second,
}

#[derive(Clone)]
struct InsertRowFormFieldState {
    name: String,
    data_type: String,
    nullable: bool,
    has_default: bool,
    is_primary_key: bool,
    kind: InsertRowFieldKind,
    value: String,
    date_value: Option<InsertRowDateValue>,
    time_value: Option<InsertRowTimeValue>,
    time_segment: Option<InsertRowDateTimeSegment>,
}

struct InsertRowFormState {
    connection_index: usize,
    object: DbObjectRef,
    selected_field: usize,
    fields: Vec<InsertRowFormFieldState>,
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
    warning: String,
    help: String,
    operation: PendingDeleteOperation,
}

enum PendingDeleteOperation {
    ExecuteStatement {
        connection_index: usize,
        sql: String,
        status: Option<String>,
        refresh_target: Option<DbObjectRef>,
    },
    PreviewInEditor {
        connection_index: usize,
        database_name: String,
        title: String,
        sql: String,
        status: Option<String>,
        refresh_target: Option<DbObjectRef>,
    },
    DeleteSavedQuery {
        name: String,
        tab_id: usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteOperationKind {
    Insert,
    Update,
    Delete,
    Truncate,
    Drop,
    Alter,
    Create,
    Replace,
    Merge,
}

impl WriteOperationKind {
    fn label(self) -> &'static str {
        match self {
            Self::Insert => "INSERT",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
            Self::Truncate => "TRUNCATE",
            Self::Drop => "DROP",
            Self::Alter => "ALTER",
            Self::Create => "CREATE",
            Self::Replace => "REPLACE",
            Self::Merge => "MERGE",
        }
    }

    fn delete_confirmation_kind(self) -> Option<DeleteOperationKind> {
        match self {
            Self::Delete => Some(DeleteOperationKind::Delete),
            Self::Drop => Some(DeleteOperationKind::Drop),
            Self::Truncate => Some(DeleteOperationKind::Truncate),
            _ => None,
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
    saved_query_name: Option<String>,
    buffer: SqlEditorBuffer,
    status: Option<String>,
    result_sets: Vec<EditorResultSet>,
    selected_result: usize,
    pending_execute_request_id: Option<u64>,
    post_execute_refresh_target: Option<DbObjectRef>,
    result_strip: String,
}

struct EditorResultSet {
    title: String,
    grid: TablePreview,
}

#[derive(Default)]
struct SavedSqlState {
    entries: Vec<SavedSqlEntry>,
    visible_items: Vec<SavedSqlEntry>,
    query: String,
    selected: usize,
    open: bool,
    sync_sequence: u64,
}

impl WorkspaceApp {
    pub fn bootstrap(bootstraps: Vec<ConnectionBootstrap>, preview_limit: usize) -> Result<Self> {
        let mut sessions = Vec::new();
        for bootstrap in bootstraps {
            let mut driver = bootstrap.driver;
            let capabilities = driver.capabilities();
            let catalog_summary = driver.load_catalog_summary()?;
            let mut app = ConnectionApp::from_catalog(
                catalog_summary.as_catalog_with_unloaded_objects(),
                driver.connection_label(),
                preview_limit,
            );
            let mut loaded_groups = BTreeSet::new();
            if let Some((database, schema, kind)) =
                first_object_group_with_objects(&catalog_summary)
            {
                let objects = driver.load_schema_objects_of_kind(&database, &schema, kind)?;
                app.merge_schema_objects_of_kind(&database, &schema, kind, objects)?;
                app.select_schema_locally(&database, &schema)?;
                loaded_groups.insert((database, schema, kind));
                let preview_result = app
                    .selected_object()
                    .cloned()
                    .map(|object| {
                        if object.kind.supports_data_preview() {
                            driver
                                .load_preview(&object, preview_limit)
                                .map_err(|error| error.to_string())
                        } else {
                            Err(preview_unavailable_message(&object))
                        }
                    })
                    .unwrap_or_else(|| Ok(TablePreview::default()));
                app.apply_preview_result(preview_result);
            } else {
                app.set_status(format!(
                    "Connected to {}. No database objects were found.",
                    driver.connection_label()
                ));
            }
            let mut session = ConnectionSession {
                name: bootstrap.name,
                connection_label: app.connection_label().to_string(),
                kind: driver.kind(),
                capabilities,
                read_only: false,
                catalog_summary,
                worker: SessionWorker::spawn(driver),
                app,
                expanded: true,
                expanded_databases: BTreeSet::new(),
                expanded_schemas: BTreeSet::new(),
                expanded_groups: BTreeSet::new(),
                expanded_saved_query_groups: BTreeSet::new(),
                loaded_groups,
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
            help_overlay_visible: false,
            editor_completion: EditorCompletionState::default(),
            sql_history: SqlHistoryState::default(),
            saved_sql: SavedSqlState::default(),
            save_sql_dialog: None,
            create_table_form: None,
            structure_editor_form: None,
            alter_column_form: None,
            add_column_form: None,
            rename_table_form: None,
            create_index_form: None,
            drop_index_form: None,
            insert_row_form: None,
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
                | WorkspaceAction::OpenHelpOverlay
                | WorkspaceAction::CloseHelpOverlay
                | WorkspaceAction::NextCommandPaletteItem
                | WorkspaceAction::PreviousCommandPaletteItem
                | WorkspaceAction::OpenSqlHistory
                | WorkspaceAction::CloseSqlHistory
                | WorkspaceAction::NextSqlHistoryItem
                | WorkspaceAction::PreviousSqlHistoryItem
                | WorkspaceAction::OpenSavedSql
                | WorkspaceAction::CloseSavedSql
                | WorkspaceAction::NextSavedSqlItem
                | WorkspaceAction::PreviousSavedSqlItem
                | WorkspaceAction::OpenSaveSqlDialog
                | WorkspaceAction::CloseSaveSqlDialog
                | WorkspaceAction::OpenCreateTableForm
                | WorkspaceAction::CloseCreateTableForm
                | WorkspaceAction::NextCreateTableField
                | WorkspaceAction::PreviousCreateTableField
                | WorkspaceAction::MoveCreateTableFieldLeft
                | WorkspaceAction::MoveCreateTableFieldRight
                | WorkspaceAction::CycleCreateTableColumnTypeNext
                | WorkspaceAction::CycleCreateTableColumnTypePrevious
                | WorkspaceAction::ToggleCreateTableColumnNullable
                | WorkspaceAction::ToggleCreateTableColumnUnique
                | WorkspaceAction::ToggleCreateTableColumnAutoIncrement
                | WorkspaceAction::ToggleCreateTableColumnPrimaryKey
                | WorkspaceAction::AddCreateTableColumn
                | WorkspaceAction::RemoveCreateTableColumn
                | WorkspaceAction::PreviewCreateTableForm
                | WorkspaceAction::OpenStructureEditor
                | WorkspaceAction::CloseStructureEditorForm
                | WorkspaceAction::NextStructureEditorField
                | WorkspaceAction::PreviousStructureEditorField
                | WorkspaceAction::MoveStructureEditorFieldLeft
                | WorkspaceAction::MoveStructureEditorFieldRight
                | WorkspaceAction::CycleStructureEditorColumnTypeNext
                | WorkspaceAction::CycleStructureEditorColumnTypePrevious
                | WorkspaceAction::ToggleStructureEditorNullable
                | WorkspaceAction::ToggleStructureEditorUnique
                | WorkspaceAction::ToggleStructureEditorPrimaryKey
                | WorkspaceAction::AddStructureEditorColumn
                | WorkspaceAction::RemoveStructureEditorColumn
                | WorkspaceAction::PreviewStructureEditorForm
                | WorkspaceAction::OpenAlterColumnForm
                | WorkspaceAction::CloseAlterColumnForm
                | WorkspaceAction::NextAlterColumnField
                | WorkspaceAction::PreviousAlterColumnField
                | WorkspaceAction::CycleAlterColumnTypeNext
                | WorkspaceAction::CycleAlterColumnTypePrevious
                | WorkspaceAction::ToggleAlterColumnNullable
                | WorkspaceAction::PreviewAlterColumnForm
                | WorkspaceAction::OpenAddColumnForm
                | WorkspaceAction::CloseAddColumnForm
                | WorkspaceAction::NextAddColumnField
                | WorkspaceAction::PreviousAddColumnField
                | WorkspaceAction::CycleAddColumnTypeNext
                | WorkspaceAction::CycleAddColumnTypePrevious
                | WorkspaceAction::ToggleAddColumnNullable
                | WorkspaceAction::PreviewAddColumnForm
                | WorkspaceAction::OpenRenameTableForm
                | WorkspaceAction::CloseRenameTableForm
                | WorkspaceAction::PreviewRenameTableForm
                | WorkspaceAction::PromptDropStructureColumn
                | WorkspaceAction::OpenCreateIndexForm
                | WorkspaceAction::CloseCreateIndexForm
                | WorkspaceAction::ToggleCreateIndexUnique
                | WorkspaceAction::PreviewCreateIndexForm
                | WorkspaceAction::OpenDropIndexForm
                | WorkspaceAction::CloseDropIndexForm
                | WorkspaceAction::PreviewDropIndexForm
                | WorkspaceAction::OpenInsertRowForm
                | WorkspaceAction::CloseInsertRowForm
                | WorkspaceAction::NextInsertRowField
                | WorkspaceAction::PreviousInsertRowField
                | WorkspaceAction::PreviewInsertRowForm
                | WorkspaceAction::DeleteSavedSqlFromEditor
                | WorkspaceAction::OpenDataFilter
                | WorkspaceAction::CloseDataFilter
                | WorkspaceAction::StartCellEdit
                | WorkspaceAction::PreviewDeleteCurrentRow
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
            WorkspaceAction::OpenSelectedTreeItemDefault => {
                let result = self.open_selected_tree_item_default();
                self.handle_error(result);
            }
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
            WorkspaceAction::OpenHelpOverlay => self.open_help_overlay(),
            WorkspaceAction::CloseHelpOverlay => self.close_help_overlay(),
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
            WorkspaceAction::OpenSavedSql => self.open_saved_sql(),
            WorkspaceAction::CloseSavedSql => self.close_saved_sql(),
            WorkspaceAction::NextSavedSqlItem => self.move_saved_sql_selection(1),
            WorkspaceAction::PreviousSavedSqlItem => self.move_saved_sql_selection(-1),
            WorkspaceAction::OpenSavedSqlSelection => {
                let result = self.open_saved_sql_selection();
                self.handle_error(result);
            }
            WorkspaceAction::OpenSaveSqlDialog => {
                let result = self.open_save_sql_dialog();
                self.handle_error(result);
            }
            WorkspaceAction::CloseSaveSqlDialog => self.close_save_sql_dialog(),
            WorkspaceAction::ConfirmSaveSql => {
                let result = self.confirm_save_sql();
                self.handle_error(result);
            }
            WorkspaceAction::OpenCreateTableForm => {
                let result = self.open_create_table_form();
                self.handle_error(result);
            }
            WorkspaceAction::CloseCreateTableForm => self.close_create_table_form(),
            WorkspaceAction::NextCreateTableField => self.move_create_table_form_selection(1),
            WorkspaceAction::PreviousCreateTableField => self.move_create_table_form_selection(-1),
            WorkspaceAction::MoveCreateTableFieldLeft => self.move_create_table_form_focus(-1),
            WorkspaceAction::MoveCreateTableFieldRight => self.move_create_table_form_focus(1),
            WorkspaceAction::CycleCreateTableColumnTypeNext => {
                let result = self.cycle_create_table_form_column_type(1);
                self.handle_error(result);
            }
            WorkspaceAction::CycleCreateTableColumnTypePrevious => {
                let result = self.cycle_create_table_form_column_type(-1);
                self.handle_error(result);
            }
            WorkspaceAction::ToggleCreateTableColumnNullable => {
                let result = self.toggle_create_table_form_nullable();
                self.handle_error(result);
            }
            WorkspaceAction::ToggleCreateTableColumnUnique => {
                let result = self.toggle_create_table_form_unique();
                self.handle_error(result);
            }
            WorkspaceAction::ToggleCreateTableColumnAutoIncrement => {
                let result = self.toggle_create_table_form_auto_increment();
                self.handle_error(result);
            }
            WorkspaceAction::ToggleCreateTableColumnPrimaryKey => {
                let result = self.toggle_create_table_form_primary_key();
                self.handle_error(result);
            }
            WorkspaceAction::AddCreateTableColumn => self.add_create_table_form_column(),
            WorkspaceAction::RemoveCreateTableColumn => {
                let result = self.remove_create_table_form_column();
                self.handle_error(result);
            }
            WorkspaceAction::PreviewCreateTableForm => {
                let result = self.preview_create_table_form();
                self.handle_error(result);
            }
            WorkspaceAction::OpenStructureEditor => {
                let result = self.open_structure_editor();
                self.handle_error(result);
            }
            WorkspaceAction::CloseStructureEditorForm => self.close_structure_editor_form(),
            WorkspaceAction::NextStructureEditorField => {
                self.move_structure_editor_form_selection(1)
            }
            WorkspaceAction::PreviousStructureEditorField => {
                self.move_structure_editor_form_selection(-1)
            }
            WorkspaceAction::MoveStructureEditorFieldLeft => {
                self.move_structure_editor_form_focus(-1)
            }
            WorkspaceAction::MoveStructureEditorFieldRight => {
                self.move_structure_editor_form_focus(1)
            }
            WorkspaceAction::CycleStructureEditorColumnTypeNext => {
                let result = self.cycle_structure_editor_form_column_type(1);
                self.handle_error(result);
            }
            WorkspaceAction::CycleStructureEditorColumnTypePrevious => {
                let result = self.cycle_structure_editor_form_column_type(-1);
                self.handle_error(result);
            }
            WorkspaceAction::ToggleStructureEditorNullable => {
                let result = self.toggle_structure_editor_form_nullable();
                self.handle_error(result);
            }
            WorkspaceAction::ToggleStructureEditorUnique => {
                let result = self.toggle_structure_editor_form_unique();
                self.handle_error(result);
            }
            WorkspaceAction::ToggleStructureEditorPrimaryKey => {
                let result = self.toggle_structure_editor_form_primary_key();
                self.handle_error(result);
            }
            WorkspaceAction::AddStructureEditorColumn => self.add_structure_editor_form_column(),
            WorkspaceAction::RemoveStructureEditorColumn => {
                let result = self.remove_structure_editor_form_column();
                self.handle_error(result);
            }
            WorkspaceAction::PreviewStructureEditorForm => {
                let result = self.preview_structure_editor_form();
                self.handle_error(result);
            }
            WorkspaceAction::OpenAlterColumnForm => {
                let result = self.open_alter_column_form();
                self.handle_error(result);
            }
            WorkspaceAction::CloseAlterColumnForm => self.close_alter_column_form(),
            WorkspaceAction::NextAlterColumnField => self.move_alter_column_form_focus(1),
            WorkspaceAction::PreviousAlterColumnField => self.move_alter_column_form_focus(-1),
            WorkspaceAction::CycleAlterColumnTypeNext => {
                let result = self.cycle_alter_column_form_type(1);
                self.handle_error(result);
            }
            WorkspaceAction::CycleAlterColumnTypePrevious => {
                let result = self.cycle_alter_column_form_type(-1);
                self.handle_error(result);
            }
            WorkspaceAction::ToggleAlterColumnNullable => {
                let result = self.toggle_alter_column_form_nullable();
                self.handle_error(result);
            }
            WorkspaceAction::PreviewAlterColumnForm => {
                let result = self.preview_alter_column_form();
                self.handle_error(result);
            }
            WorkspaceAction::OpenAddColumnForm => {
                let result = self.open_add_column_form();
                self.handle_error(result);
            }
            WorkspaceAction::CloseAddColumnForm => self.close_add_column_form(),
            WorkspaceAction::NextAddColumnField => self.move_add_column_form_focus(1),
            WorkspaceAction::PreviousAddColumnField => self.move_add_column_form_focus(-1),
            WorkspaceAction::CycleAddColumnTypeNext => {
                let result = self.cycle_add_column_form_type(1);
                self.handle_error(result);
            }
            WorkspaceAction::CycleAddColumnTypePrevious => {
                let result = self.cycle_add_column_form_type(-1);
                self.handle_error(result);
            }
            WorkspaceAction::ToggleAddColumnNullable => {
                let result = self.toggle_add_column_form_nullable();
                self.handle_error(result);
            }
            WorkspaceAction::PreviewAddColumnForm => {
                let result = self.preview_add_column_form();
                self.handle_error(result);
            }
            WorkspaceAction::OpenRenameTableForm => {
                let result = self.open_rename_table_form();
                self.handle_error(result);
            }
            WorkspaceAction::CloseRenameTableForm => self.close_rename_table_form(),
            WorkspaceAction::PreviewRenameTableForm => {
                let result = self.preview_rename_table_form();
                self.handle_error(result);
            }
            WorkspaceAction::PromptDropStructureColumn => {
                let result = self.prompt_drop_structure_column();
                self.handle_error(result);
            }
            WorkspaceAction::OpenCreateIndexForm => {
                let result = self.open_create_index_form();
                self.handle_error(result);
            }
            WorkspaceAction::CloseCreateIndexForm => self.close_create_index_form(),
            WorkspaceAction::ToggleCreateIndexUnique => {
                let result = self.toggle_create_index_form_unique();
                self.handle_error(result);
            }
            WorkspaceAction::PreviewCreateIndexForm => {
                let result = self.preview_create_index_form();
                self.handle_error(result);
            }
            WorkspaceAction::OpenDropIndexForm => {
                let result = self.open_drop_index_form();
                self.handle_error(result);
            }
            WorkspaceAction::CloseDropIndexForm => self.close_drop_index_form(),
            WorkspaceAction::PreviewDropIndexForm => {
                let result = self.preview_drop_index_form();
                self.handle_error(result);
            }
            WorkspaceAction::OpenInsertRowForm => {
                let result = self.open_insert_row_form();
                self.handle_error(result);
            }
            WorkspaceAction::CloseInsertRowForm => self.close_insert_row_form(),
            WorkspaceAction::NextInsertRowField => self.move_insert_row_form_selection(1),
            WorkspaceAction::PreviousInsertRowField => self.move_insert_row_form_selection(-1),
            WorkspaceAction::PreviewInsertRowForm => {
                let result = self.preview_insert_row_form();
                self.handle_error(result);
            }
            WorkspaceAction::DeleteSavedSqlFromEditor => {
                let result = self.delete_saved_sql_from_editor();
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
            WorkspaceAction::PreviewDeleteCurrentRow => {
                let result = self.preview_delete_current_row();
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

    pub fn help_overlay_open(&self) -> bool {
        self.help_overlay_visible
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
            selected_connection_capabilities: selected_session.map(|session| session.capabilities),
            selected_connection_read_only: selected_session
                .map(|session| session.read_only)
                .unwrap_or(false),
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
            saved_sql: self.saved_sql.view(),
            save_sql_dialog: self.save_sql_dialog_view(),
            create_table_form_open: self.create_table_form_open(),
            structure_editor_form_open: self.structure_editor_form.is_some(),
            alter_column_form_open: self.alter_column_form.is_some(),
            add_column_form_open: self.add_column_form.is_some(),
            rename_table_form_open: self.rename_table_form.is_some(),
            create_index_form_open: self.create_index_form.is_some(),
            drop_index_form_open: self.drop_index_form.is_some(),
            insert_row_form_open: self.insert_row_form_open(),
            data_filter: self.data_filter_view(),
            cell_edit: self.cell_edit_view(),
            row_inspector: self.row_inspector_view(),
            help_overlay_visible: self.help_overlay_visible,
            editor: self.editor.as_ref().map(SqlEditorState::view),
            editor_completion: self.editor_completion.view(),
            structure: self.structure_view(),
            staged_crud: self.staged_crud_view(),
            delete_confirmation: self.delete_confirmation_view(),
            status: self.selected_session_status(),
            selected_connection_database_count: selected_session
                .map(|session| session.catalog_summary.databases.len())
                .unwrap_or_default(),
            selected_connection_schema_count: selected_session
                .map(|session| session.catalog_summary.schema_count())
                .unwrap_or_default(),
            selected_connection_object_count: selected_session
                .map(|session| session.catalog_summary.object_count())
                .unwrap_or_default(),
            selected_schema_table_count: self
                .object_count_for_selected_schema(DbObjectKind::Table)
                .unwrap_or_default(),
            selected_schema_view_count: self
                .object_count_for_selected_schema(DbObjectKind::View)
                .unwrap_or_default(),
            selected_schema_materialized_view_count: self
                .object_count_for_selected_schema(DbObjectKind::MaterializedView)
                .unwrap_or_default(),
            selected_schema_foreign_table_count: self
                .object_count_for_selected_schema(DbObjectKind::ForeignTable)
                .unwrap_or_default(),
            selected_schema_function_count: self
                .object_count_for_selected_schema(DbObjectKind::Function)
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

    fn open_help_overlay(&mut self) {
        self.help_overlay_visible = true;
    }

    fn close_help_overlay(&mut self) {
        self.help_overlay_visible = false;
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

    pub fn saved_sql_open(&self) -> bool {
        self.saved_sql.open
    }

    pub fn save_sql_dialog_open(&self) -> bool {
        self.save_sql_dialog.is_some()
    }

    pub fn create_table_form_open(&self) -> bool {
        self.create_table_form.is_some()
    }

    pub fn structure_editor_form_open(&self) -> bool {
        self.structure_editor_form.is_some()
    }

    pub fn alter_column_form_open(&self) -> bool {
        self.alter_column_form.is_some()
    }

    pub fn add_column_form_open(&self) -> bool {
        self.add_column_form.is_some()
    }

    pub fn rename_table_form_open(&self) -> bool {
        self.rename_table_form.is_some()
    }

    pub fn create_index_form_open(&self) -> bool {
        self.create_index_form.is_some()
    }

    pub fn drop_index_form_open(&self) -> bool {
        self.drop_index_form.is_some()
    }

    pub fn insert_row_form_open(&self) -> bool {
        self.insert_row_form.is_some()
    }

    pub fn insert_sql_history_search_char(&mut self, ch: char) -> Result<()> {
        self.sql_history.insert_char(ch);
        Ok(())
    }

    pub fn backspace_sql_history_search(&mut self) -> Result<()> {
        self.sql_history.backspace();
        Ok(())
    }

    pub fn insert_saved_sql_search_char(&mut self, ch: char) -> Result<()> {
        self.saved_sql.insert_char(ch);
        Ok(())
    }

    pub fn backspace_saved_sql_search(&mut self) -> Result<()> {
        self.saved_sql.backspace();
        Ok(())
    }

    pub fn insert_save_sql_name_char(&mut self, ch: char) -> Result<()> {
        let dialog = self
            .save_sql_dialog
            .as_mut()
            .ok_or_else(|| anyhow!("save SQL dialog is not open"))?;
        dialog.name.push(ch);
        Ok(())
    }

    pub fn backspace_save_sql_name(&mut self) -> Result<()> {
        let dialog = self
            .save_sql_dialog
            .as_mut()
            .ok_or_else(|| anyhow!("save SQL dialog is not open"))?;
        dialog.name.pop();
        Ok(())
    }

    pub fn clear_save_sql_name(&mut self) -> Result<()> {
        let dialog = self
            .save_sql_dialog
            .as_mut()
            .ok_or_else(|| anyhow!("save SQL dialog is not open"))?;
        dialog.name.clear();
        Ok(())
    }

    pub fn insert_create_table_form_char(&mut self, ch: char) -> Result<()> {
        let form = self
            .create_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("create table form is not open"))?;

        match create_table_active_text_target(form) {
            Some(target) => {
                target.push(ch);
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub fn backspace_create_table_form(&mut self) -> Result<()> {
        let form = self
            .create_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("create table form is not open"))?;

        match create_table_active_text_target(form) {
            Some(target) => {
                target.pop();
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub fn clear_create_table_form_field(&mut self) -> Result<()> {
        let form = self
            .create_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("create table form is not open"))?;

        match create_table_active_text_target(form) {
            Some(target) => {
                target.clear();
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub fn create_table_form_selected_field_is_type(&self) -> bool {
        self.create_table_form.as_ref().is_some_and(|form| {
            form.selected_row > 0 && form.selected_focus == CreateTableFieldFocus::ColumnType
        })
    }

    pub fn create_table_form_selected_field_is_toggle(&self) -> bool {
        self.create_table_form.as_ref().is_some_and(|form| {
            form.selected_row > 0
                && matches!(
                    form.selected_focus,
                    CreateTableFieldFocus::Nullable
                        | CreateTableFieldFocus::Unique
                        | CreateTableFieldFocus::AutoIncrement
                        | CreateTableFieldFocus::PrimaryKey
                )
        })
    }

    pub fn preview_and_execute_create_table_form(&mut self) -> Result<()> {
        self.preview_create_table_form()?;
        self.execute_editor()
    }

    pub fn preview_and_execute_structure_editor_form(&mut self) -> Result<()> {
        self.preview_structure_editor_form()?;
        self.execute_editor()?;
        self.select_right_structure_tab()
    }

    pub fn structure_editor_form_selected_field_is_type(&self) -> bool {
        self.structure_editor_form.as_ref().is_some_and(|form| {
            form.selected_row > 0 && form.selected_focus == StructureEditorFieldFocus::ColumnType
        })
    }

    pub fn structure_editor_form_selected_field_is_nullable(&self) -> bool {
        self.structure_editor_form.as_ref().is_some_and(|form| {
            form.selected_row > 0 && form.selected_focus == StructureEditorFieldFocus::Nullable
        })
    }

    pub fn structure_editor_form_selected_field_is_unique(&self) -> bool {
        self.structure_editor_form.as_ref().is_some_and(|form| {
            form.selected_row > 0 && form.selected_focus == StructureEditorFieldFocus::Unique
        })
    }

    pub fn structure_editor_form_selected_field_is_primary_key(&self) -> bool {
        self.structure_editor_form.as_ref().is_some_and(|form| {
            form.selected_row > 0 && form.selected_focus == StructureEditorFieldFocus::PrimaryKey
        })
    }

    pub fn structure_editor_form_selected_field_is_toggle(&self) -> bool {
        self.structure_editor_form.as_ref().is_some_and(|form| {
            form.selected_row > 0
                && matches!(
                    form.selected_focus,
                    StructureEditorFieldFocus::Nullable
                        | StructureEditorFieldFocus::Unique
                        | StructureEditorFieldFocus::PrimaryKey
                )
        })
    }

    pub fn insert_structure_editor_form_char(&mut self, ch: char) -> Result<()> {
        let form = self
            .structure_editor_form
            .as_mut()
            .ok_or_else(|| anyhow!("structure editor is not open"))?;

        match structure_editor_active_text_target(form) {
            Some(target) => {
                if *target == STRUCTURE_EDITOR_EXISTING_DEFAULT_SENTINEL {
                    target.clear();
                }
                target.push(ch);
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub fn backspace_structure_editor_form(&mut self) -> Result<()> {
        let form = self
            .structure_editor_form
            .as_mut()
            .ok_or_else(|| anyhow!("structure editor is not open"))?;

        match structure_editor_active_text_target(form) {
            Some(target) => {
                if *target == STRUCTURE_EDITOR_EXISTING_DEFAULT_SENTINEL {
                    target.clear();
                } else {
                    target.pop();
                }
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub fn clear_structure_editor_form_field(&mut self) -> Result<()> {
        let form = self
            .structure_editor_form
            .as_mut()
            .ok_or_else(|| anyhow!("structure editor is not open"))?;

        match structure_editor_active_text_target(form) {
            Some(target) => {
                target.clear();
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub fn alter_column_form_selected_field_is_type(&self) -> bool {
        self.alter_column_form
            .as_ref()
            .is_some_and(|form| form.selected_focus == AlterColumnFieldFocus::ColumnType)
    }

    pub fn alter_column_form_selected_field_is_nullable(&self) -> bool {
        self.alter_column_form
            .as_ref()
            .is_some_and(|form| form.selected_focus == AlterColumnFieldFocus::Nullable)
    }

    pub fn insert_alter_column_form_char(&mut self, ch: char) -> Result<()> {
        let form = self
            .alter_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("alter column form is not open"))?;
        match form.selected_focus {
            AlterColumnFieldFocus::ColumnName => form.new_name.push(ch),
            AlterColumnFieldFocus::DefaultValue => form.default_value.push(ch),
            AlterColumnFieldFocus::ColumnType | AlterColumnFieldFocus::Nullable => {}
        }
        Ok(())
    }

    pub fn backspace_alter_column_form(&mut self) -> Result<()> {
        let form = self
            .alter_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("alter column form is not open"))?;
        match form.selected_focus {
            AlterColumnFieldFocus::ColumnName => {
                form.new_name.pop();
            }
            AlterColumnFieldFocus::DefaultValue => {
                form.default_value.pop();
            }
            AlterColumnFieldFocus::ColumnType | AlterColumnFieldFocus::Nullable => {}
        }
        Ok(())
    }

    pub fn clear_alter_column_form_field(&mut self) -> Result<()> {
        let form = self
            .alter_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("alter column form is not open"))?;
        match form.selected_focus {
            AlterColumnFieldFocus::ColumnName => form.new_name.clear(),
            AlterColumnFieldFocus::DefaultValue => form.default_value.clear(),
            AlterColumnFieldFocus::ColumnType | AlterColumnFieldFocus::Nullable => {}
        }
        Ok(())
    }

    pub fn add_column_form_selected_field_is_type(&self) -> bool {
        self.add_column_form
            .as_ref()
            .is_some_and(|form| form.selected_focus == AddColumnFieldFocus::ColumnType)
    }

    pub fn add_column_form_selected_field_is_nullable(&self) -> bool {
        self.add_column_form
            .as_ref()
            .is_some_and(|form| form.selected_focus == AddColumnFieldFocus::Nullable)
    }

    pub fn insert_add_column_form_char(&mut self, ch: char) -> Result<()> {
        let form = self
            .add_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("add column form is not open"))?;
        match form.selected_focus {
            AddColumnFieldFocus::ColumnName => form.name.push(ch),
            AddColumnFieldFocus::DefaultValue => form.default_value.push(ch),
            AddColumnFieldFocus::ColumnType | AddColumnFieldFocus::Nullable => {}
        }
        Ok(())
    }

    pub fn backspace_add_column_form(&mut self) -> Result<()> {
        let form = self
            .add_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("add column form is not open"))?;
        match form.selected_focus {
            AddColumnFieldFocus::ColumnName => {
                form.name.pop();
            }
            AddColumnFieldFocus::DefaultValue => {
                form.default_value.pop();
            }
            AddColumnFieldFocus::ColumnType | AddColumnFieldFocus::Nullable => {}
        }
        Ok(())
    }

    pub fn clear_add_column_form_field(&mut self) -> Result<()> {
        let form = self
            .add_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("add column form is not open"))?;
        match form.selected_focus {
            AddColumnFieldFocus::ColumnName => form.name.clear(),
            AddColumnFieldFocus::DefaultValue => form.default_value.clear(),
            AddColumnFieldFocus::ColumnType | AddColumnFieldFocus::Nullable => {}
        }
        Ok(())
    }

    pub fn insert_rename_table_form_char(&mut self, ch: char) -> Result<()> {
        let form = self
            .rename_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("rename table form is not open"))?;
        form.new_name.push(ch);
        Ok(())
    }

    pub fn backspace_rename_table_form(&mut self) -> Result<()> {
        let form = self
            .rename_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("rename table form is not open"))?;
        form.new_name.pop();
        Ok(())
    }

    pub fn clear_rename_table_form(&mut self) -> Result<()> {
        let form = self
            .rename_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("rename table form is not open"))?;
        form.new_name.clear();
        Ok(())
    }

    pub fn create_index_form_unique_selected(&self) -> bool {
        self.create_index_form.is_some()
    }

    pub fn insert_create_index_form_char(&mut self, ch: char) -> Result<()> {
        let form = self
            .create_index_form
            .as_mut()
            .ok_or_else(|| anyhow!("create index form is not open"))?;
        form.index_name.push(ch);
        Ok(())
    }

    pub fn backspace_create_index_form(&mut self) -> Result<()> {
        let form = self
            .create_index_form
            .as_mut()
            .ok_or_else(|| anyhow!("create index form is not open"))?;
        form.index_name.pop();
        Ok(())
    }

    pub fn clear_create_index_form(&mut self) -> Result<()> {
        let form = self
            .create_index_form
            .as_mut()
            .ok_or_else(|| anyhow!("create index form is not open"))?;
        form.index_name.clear();
        Ok(())
    }

    pub fn insert_drop_index_form_char(&mut self, ch: char) -> Result<()> {
        let form = self
            .drop_index_form
            .as_mut()
            .ok_or_else(|| anyhow!("drop index form is not open"))?;
        form.index_name.push(ch);
        Ok(())
    }

    pub fn backspace_drop_index_form(&mut self) -> Result<()> {
        let form = self
            .drop_index_form
            .as_mut()
            .ok_or_else(|| anyhow!("drop index form is not open"))?;
        form.index_name.pop();
        Ok(())
    }

    pub fn clear_drop_index_form(&mut self) -> Result<()> {
        let form = self
            .drop_index_form
            .as_mut()
            .ok_or_else(|| anyhow!("drop index form is not open"))?;
        form.index_name.clear();
        Ok(())
    }

    pub fn insert_insert_row_form_char(&mut self, ch: char) -> Result<()> {
        let form = self
            .insert_row_form
            .as_mut()
            .ok_or_else(|| anyhow!("insert row form is not open"))?;
        let field = form
            .fields
            .get_mut(form.selected_field)
            .ok_or_else(|| anyhow!("selected insert row field is no longer available"))?;
        field.value.push(ch);
        sync_insert_row_form_field(field);
        Ok(())
    }

    pub fn backspace_insert_row_form(&mut self) -> Result<()> {
        let form = self
            .insert_row_form
            .as_mut()
            .ok_or_else(|| anyhow!("insert row form is not open"))?;
        let field = form
            .fields
            .get_mut(form.selected_field)
            .ok_or_else(|| anyhow!("selected insert row field is no longer available"))?;
        field.value.pop();
        sync_insert_row_form_field(field);
        Ok(())
    }

    pub fn clear_insert_row_form_field(&mut self) -> Result<()> {
        let form = self
            .insert_row_form
            .as_mut()
            .ok_or_else(|| anyhow!("insert row form is not open"))?;
        let field = form
            .fields
            .get_mut(form.selected_field)
            .ok_or_else(|| anyhow!("selected insert row field is no longer available"))?;
        field.value.clear();
        sync_insert_row_form_field(field);
        Ok(())
    }

    pub fn insert_row_form_selected_field_supports_date_picker(&self) -> bool {
        self.insert_row_form
            .as_ref()
            .and_then(|form| form.fields.get(form.selected_field))
            .is_some_and(|field| field.kind.supports_date_picker())
    }

    pub fn insert_row_form_selected_field_supports_time_picker(&self) -> bool {
        self.insert_row_form
            .as_ref()
            .and_then(|form| form.fields.get(form.selected_field))
            .is_some_and(|field| field.kind.supports_time_picker())
    }

    pub fn adjust_insert_row_form_date_days(&mut self, delta: i32) -> Result<()> {
        self.mutate_selected_insert_row_form_date(|date| date.add_days(delta))
    }

    pub fn adjust_insert_row_form_date_months(&mut self, delta: i32) -> Result<()> {
        self.mutate_selected_insert_row_form_date(|date| date.add_months(delta))
    }

    pub fn adjust_insert_row_form_date_years(&mut self, delta: i32) -> Result<()> {
        self.mutate_selected_insert_row_form_date(|date| date.add_years(delta))
    }

    pub fn set_insert_row_form_date_today(&mut self) -> Result<()> {
        self.mutate_selected_insert_row_form_date(|_| InsertRowDateValue::today())
    }

    pub fn adjust_insert_row_form_time_hours(&mut self, delta: i32) -> Result<()> {
        self.mutate_selected_insert_row_form_time(|time| time.add_hours(delta))
    }

    pub fn adjust_insert_row_form_time_minutes(&mut self, delta: i32) -> Result<()> {
        self.mutate_selected_insert_row_form_time(|time| time.add_minutes(delta))
    }

    pub fn adjust_insert_row_form_time_seconds(&mut self, delta: i32) -> Result<()> {
        self.mutate_selected_insert_row_form_time(|time| time.add_seconds(delta))
    }

    pub fn move_insert_row_form_time_segment(&mut self, delta: isize) -> Result<()> {
        let form = self
            .insert_row_form
            .as_mut()
            .ok_or_else(|| anyhow!("insert row form is not open"))?;
        let field = form
            .fields
            .get_mut(form.selected_field)
            .ok_or_else(|| anyhow!("selected insert row field is no longer available"))?;
        let segment = field
            .time_segment
            .ok_or_else(|| anyhow!("selected field does not support segmented datetime focus"))?;
        field.time_segment = Some(segment.cycle(delta));
        Ok(())
    }

    pub fn adjust_insert_row_form_active_time_segment(&mut self, delta: i32) -> Result<()> {
        let form = self
            .insert_row_form
            .as_mut()
            .ok_or_else(|| anyhow!("insert row form is not open"))?;
        let field = form
            .fields
            .get_mut(form.selected_field)
            .ok_or_else(|| anyhow!("selected insert row field is no longer available"))?;
        let segment = field
            .time_segment
            .ok_or_else(|| anyhow!("selected field does not support segmented datetime focus"))?;

        match segment {
            InsertRowDateTimeSegment::Day => {
                let base = field.date_value.unwrap_or_else(InsertRowDateValue::today);
                let updated = base.add_days(delta);
                field.date_value = Some(updated);
                field.value = render_insert_row_form_field_temporal_value(
                    field.kind,
                    field.value.trim(),
                    updated,
                    field.time_value,
                );
            }
            InsertRowDateTimeSegment::Hour => {
                let base = field
                    .time_value
                    .unwrap_or_else(InsertRowTimeValue::midnight);
                let updated = base.add_hours(delta);
                field.time_value = Some(updated);
                let date = field.date_value.unwrap_or_else(InsertRowDateValue::today);
                field.value = render_insert_row_form_field_temporal_value(
                    field.kind,
                    field.value.trim(),
                    date,
                    Some(updated),
                );
            }
            InsertRowDateTimeSegment::Minute => {
                let base = field
                    .time_value
                    .unwrap_or_else(InsertRowTimeValue::midnight);
                let updated = base.add_minutes(delta);
                field.time_value = Some(updated);
                let date = field.date_value.unwrap_or_else(InsertRowDateValue::today);
                field.value = render_insert_row_form_field_temporal_value(
                    field.kind,
                    field.value.trim(),
                    date,
                    Some(updated),
                );
            }
            InsertRowDateTimeSegment::Second => {
                let base = field
                    .time_value
                    .unwrap_or_else(InsertRowTimeValue::midnight);
                let updated = base.add_seconds(delta);
                field.time_value = Some(updated);
                let date = field.date_value.unwrap_or_else(InsertRowDateValue::today);
                field.value = render_insert_row_form_field_temporal_value(
                    field.kind,
                    field.value.trim(),
                    date,
                    Some(updated),
                );
            }
        }

        Ok(())
    }

    pub fn set_insert_row_form_datetime_now(&mut self) -> Result<()> {
        let form = self
            .insert_row_form
            .as_mut()
            .ok_or_else(|| anyhow!("insert row form is not open"))?;
        let field = form
            .fields
            .get_mut(form.selected_field)
            .ok_or_else(|| anyhow!("selected insert row field is no longer available"))?;
        if !field.kind.supports_time_picker() {
            return Err(anyhow!(
                "selected field does not support the datetime picker"
            ));
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let date = InsertRowDateValue::from_unix_days((now / 86_400) as i64);
        let time = InsertRowTimeValue::from_seconds_of_day((now % 86_400) as u32);
        field.date_value = Some(date);
        field.time_value = Some(time);
        field.value = render_insert_row_form_field_temporal_value(
            field.kind,
            field.value.trim(),
            date,
            Some(time),
        );
        Ok(())
    }

    fn mutate_selected_insert_row_form_date<F>(&mut self, mutator: F) -> Result<()>
    where
        F: FnOnce(InsertRowDateValue) -> InsertRowDateValue,
    {
        let form = self
            .insert_row_form
            .as_mut()
            .ok_or_else(|| anyhow!("insert row form is not open"))?;
        let field = form
            .fields
            .get_mut(form.selected_field)
            .ok_or_else(|| anyhow!("selected insert row field is no longer available"))?;
        if !field.kind.supports_date_picker() {
            return Err(anyhow!("selected field does not support the date picker"));
        }
        let base = field.date_value.unwrap_or_else(InsertRowDateValue::today);
        let updated = mutator(base);
        field.date_value = Some(updated);
        field.value = render_insert_row_form_field_temporal_value(
            field.kind,
            field.value.trim(),
            updated,
            field.time_value,
        );
        Ok(())
    }

    fn mutate_selected_insert_row_form_time<F>(&mut self, mutator: F) -> Result<()>
    where
        F: FnOnce(InsertRowTimeValue) -> InsertRowTimeValue,
    {
        let form = self
            .insert_row_form
            .as_mut()
            .ok_or_else(|| anyhow!("insert row form is not open"))?;
        let field = form
            .fields
            .get_mut(form.selected_field)
            .ok_or_else(|| anyhow!("selected insert row field is no longer available"))?;
        if !field.kind.supports_time_picker() {
            return Err(anyhow!("selected field does not support the time picker"));
        }
        let base = field
            .time_value
            .unwrap_or_else(InsertRowTimeValue::midnight);
        let updated = mutator(base);
        field.time_value = Some(updated);
        let date = field.date_value.unwrap_or_else(InsertRowDateValue::today);
        field.value = render_insert_row_form_field_temporal_value(
            field.kind,
            field.value.trim(),
            date,
            Some(updated),
        );
        Ok(())
    }

    pub fn replace_saved_queries(&mut self, entries: Vec<SavedSqlEntry>) {
        let selected_key = self
            .entries
            .get(self.selected_row)
            .map(|entry| entry.key.clone());
        self.saved_sql.replace_entries(entries);
        self.rebuild_rows(selected_key);
    }

    pub fn saved_queries_snapshot(&self) -> Vec<SavedSqlEntry> {
        self.saved_sql.entries.clone()
    }

    pub fn saved_queries_sync_sequence(&self) -> u64 {
        self.saved_sql.sync_sequence
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
            self.insert_row_form = None;
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
            TreeNodeKey::SavedQuery {
                connection, name, ..
            } => self.open_saved_sql_named(connection, &name),
            TreeNodeKey::Object { object, .. } if object.kind.supports_data_preview() => {
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
            TreeNodeKey::Object { .. } => self.open_sql_editor(),
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
            TreeNodeKey::SavedQueryGroup { database, .. } => Some(database.as_str()),
            TreeNodeKey::Object { object, .. } => Some(object.database.as_str()),
            TreeNodeKey::SavedQuery { database, .. } => Some(database.as_str()),
            TreeNodeKey::Connection { .. } => self.selected_session()?.app.selected_database_name(),
        }
    }

    pub fn selected_schema_name(&self) -> Option<&str> {
        match &self.entries.get(self.selected_row)?.key {
            TreeNodeKey::Schema { schema, .. } => Some(schema.as_str()),
            TreeNodeKey::Group { schema, .. } => Some(schema.as_str()),
            TreeNodeKey::SavedQueryGroup { schema, .. } => Some(schema.as_str()),
            TreeNodeKey::Object { object, .. } => Some(object.schema.as_str()),
            TreeNodeKey::SavedQuery { schema, .. } => Some(schema.as_str()),
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

    pub fn set_connection_read_only(
        &mut self,
        connection_index: usize,
        read_only: bool,
    ) -> Result<()> {
        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        session.read_only = read_only;
        Ok(())
    }

    pub fn connection_read_only(&self, connection_index: usize) -> Option<bool> {
        self.sessions
            .get(connection_index)
            .map(|session| session.read_only)
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
        Some(self.selected_session()?.catalog_summary.schema_count())
    }

    pub fn selected_connection_object_count(&self) -> Option<usize> {
        Some(self.selected_session()?.catalog_summary.object_count())
    }

    pub fn object_count_for_selected_schema(&self, kind: DbObjectKind) -> Option<usize> {
        let session = self.selected_session()?;
        let database_name = self.selected_database_name()?;
        let schema_name = self.selected_schema_name()?;
        let schema = session
            .catalog_summary
            .databases
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

        let (connection_index, object) = self.selected_preview_target()?;
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

        let (connection_index, object) = self.selected_preview_target()?;
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
        let Some((connection_index, database_name, prefix)) =
            self.active_editor_tab().and_then(|tab| {
                tab.buffer
                    .completion_prefix()
                    .map(|prefix| (tab.connection_index, tab.database_name.clone(), prefix))
            })
        else {
            self.editor_completion.clear();
            return;
        };
        let Some(session) = self.sessions.get(connection_index) else {
            self.editor_completion.clear();
            return;
        };
        if !session.capabilities.supports_sql_completion {
            self.editor_completion.clear();
            return;
        }
        let active_database = database_name
            .as_deref()
            .or_else(|| session.app.selected_database_name());
        let objects = session
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
            .collect::<Vec<_>>();
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

    fn open_saved_sql(&mut self) {
        self.saved_sql.open();
    }

    fn close_saved_sql(&mut self) {
        self.saved_sql.open = false;
    }

    fn move_saved_sql_selection(&mut self, delta: isize) {
        self.saved_sql.move_selection(delta);
    }

    fn open_saved_sql_selection(&mut self) -> Result<()> {
        let entry = self
            .saved_sql
            .selected_entry()
            .cloned()
            .ok_or_else(|| anyhow!("no saved SQL item is selected"))?;
        self.saved_sql.open = false;
        self.open_saved_sql_entry(None, entry)
    }

    fn open_saved_sql_named(&mut self, connection_index: usize, name: &str) -> Result<()> {
        let entry = self
            .saved_sql
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .cloned()
            .ok_or_else(|| anyhow!("saved SQL `{name}` no longer exists"))?;
        self.open_saved_sql_entry(Some(connection_index), entry)
    }

    fn open_saved_sql_entry(
        &mut self,
        preferred_connection_index: Option<usize>,
        entry: SavedSqlEntry,
    ) -> Result<()> {
        let connection_index = preferred_connection_index
            .or_else(|| {
                entry.connection_name.as_deref().and_then(|name| {
                    self.sessions
                        .iter()
                        .position(|session| session.name == name)
                })
            })
            .or_else(|| self.active_editor_connection_index())
            .or_else(|| self.selected_connection_index())
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let title = entry.name.clone();
        let database_name = entry.database_name.clone();
        let saved_query_name = entry.name.clone();
        self.open_editor_tab(connection_index, database_name, title, entry.sql);
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.saved_query_name = Some(saved_query_name);
        }
        self.active_right_tab = RightPaneTab::Sql;
        self.focus_sql_editor();
        self.reset_grid_scroll();
        self.refresh_editor_completion();
        Ok(())
    }

    fn open_save_sql_dialog(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        let sql = tab.buffer.sql();
        if sql.trim().is_empty() {
            return Err(anyhow!("enter SQL before saving it"));
        }

        self.save_sql_dialog = Some(SaveSqlDialogState {
            name: tab.saved_query_name.clone().unwrap_or_default(),
        });
        Ok(())
    }

    fn close_save_sql_dialog(&mut self) {
        self.save_sql_dialog = None;
    }

    fn open_create_table_form(&mut self) -> Result<()> {
        let (connection_index, database_name, schema_name) = self.selected_schema_target()?;
        let connection_kind = self
            .sessions
            .get(connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;

        self.create_table_form = Some(CreateTableFormState {
            connection_index,
            database_name,
            schema_name,
            table_name: String::new(),
            selected_row: 0,
            selected_focus: CreateTableFieldFocus::TableName,
            columns: vec![default_create_table_primary_column(connection_kind)],
        });
        Ok(())
    }

    fn close_create_table_form(&mut self) {
        self.create_table_form = None;
    }

    fn move_create_table_form_selection(&mut self, delta: isize) {
        let Some(form) = self.create_table_form.as_mut() else {
            return;
        };
        let row_count = form.columns.len() + 1;
        if row_count == 0 {
            form.selected_row = 0;
            return;
        }

        let offset = delta.unsigned_abs() % row_count;
        form.selected_row = if delta.is_negative() {
            (form.selected_row + row_count - offset) % row_count
        } else {
            (form.selected_row + offset) % row_count
        };

        if form.selected_row == 0 {
            form.selected_focus = CreateTableFieldFocus::TableName;
        } else if form.selected_focus == CreateTableFieldFocus::TableName {
            form.selected_focus = CreateTableFieldFocus::ColumnName;
        }
    }

    fn move_create_table_form_focus(&mut self, delta: isize) {
        let Some(form) = self.create_table_form.as_mut() else {
            return;
        };
        if form.selected_row == 0 {
            if delta.is_positive() && !form.columns.is_empty() {
                form.selected_row = 1;
                form.selected_focus = CreateTableFieldFocus::ColumnName;
            } else {
                form.selected_focus = CreateTableFieldFocus::TableName;
            }
            return;
        }
        let current = if form.selected_focus == CreateTableFieldFocus::TableName {
            CreateTableFieldFocus::ColumnName
        } else {
            form.selected_focus
        };
        let next = current.cycle(delta);

        if delta.is_positive()
            && current == CreateTableFieldFocus::PrimaryKey
            && form.selected_row < form.columns.len()
        {
            form.selected_row += 1;
            form.selected_focus = CreateTableFieldFocus::ColumnName;
            return;
        }

        if delta.is_negative() && current == CreateTableFieldFocus::ColumnName {
            if form.selected_row > 1 {
                form.selected_row -= 1;
                form.selected_focus = CreateTableFieldFocus::PrimaryKey;
            } else {
                form.selected_row = 0;
                form.selected_focus = CreateTableFieldFocus::TableName;
            }
            return;
        }

        form.selected_focus = next;
    }

    fn cycle_create_table_form_column_type(&mut self, delta: isize) -> Result<()> {
        let connection_index = self
            .create_table_form
            .as_ref()
            .map(|form| form.connection_index)
            .ok_or_else(|| anyhow!("create table form is not open"))?;
        let kind = self
            .sessions
            .get(connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let options = create_table_type_options(kind);
        let form = self
            .create_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("create table form is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        let column = form
            .columns
            .get_mut(column_index)
            .ok_or_else(|| anyhow!("selected create table column is no longer available"))?;
        let len = options.len();
        let offset = delta.unsigned_abs() % len;
        column.type_index = if delta.is_negative() {
            (column.type_index + len - offset) % len
        } else {
            (column.type_index + offset) % len
        };
        Ok(())
    }

    fn toggle_create_table_form_nullable(&mut self) -> Result<()> {
        let form = self
            .create_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("create table form is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        let column = form
            .columns
            .get_mut(column_index)
            .ok_or_else(|| anyhow!("selected create table column is no longer available"))?;
        if column.primary_key {
            column.primary_key = false;
            column.auto_increment = false;
            column.nullable = true;
        } else {
            column.nullable = !column.nullable;
            if column.nullable {
                column.auto_increment = false;
            }
        }
        Ok(())
    }

    fn toggle_create_table_form_unique(&mut self) -> Result<()> {
        let form = self
            .create_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("create table form is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        let column = form
            .columns
            .get_mut(column_index)
            .ok_or_else(|| anyhow!("selected create table column is no longer available"))?;
        column.unique = !column.unique;
        if column.unique {
            column.auto_increment = false;
        }
        Ok(())
    }

    fn toggle_create_table_form_auto_increment(&mut self) -> Result<()> {
        let kind = self
            .create_table_form
            .as_ref()
            .and_then(|form| self.sessions.get(form.connection_index))
            .map(|session| session.kind)
            .unwrap_or(DatabaseKind::Postgres);
        let form = self
            .create_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("create table form is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        if column_index >= form.columns.len() {
            return Err(anyhow!(
                "selected create table column is no longer available"
            ));
        }

        let enable = !form.columns[column_index].auto_increment;
        for (index, column) in form.columns.iter_mut().enumerate() {
            if index == column_index {
                column.auto_increment = enable;
                if enable {
                    column.type_index = create_table_auto_increment_type_index(kind);
                    column.nullable = false;
                    column.unique = false;
                    column.primary_key = true;
                }
            } else if enable {
                column.primary_key = false;
                column.auto_increment = false;
            }
        }
        Ok(())
    }

    fn toggle_create_table_form_primary_key(&mut self) -> Result<()> {
        let form = self
            .create_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("create table form is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        if column_index >= form.columns.len() {
            return Err(anyhow!(
                "selected create table column is no longer available"
            ));
        }

        let enable = !form.columns[column_index].primary_key;
        for (index, column) in form.columns.iter_mut().enumerate() {
            column.primary_key = enable && index == column_index;
            if column.primary_key {
                column.nullable = false;
                column.unique = false;
            } else {
                column.auto_increment = false;
            }
        }
        Ok(())
    }

    fn add_create_table_form_column(&mut self) {
        let kind = self
            .create_table_form
            .as_ref()
            .and_then(|form| self.sessions.get(form.connection_index))
            .map(|session| session.kind)
            .unwrap_or(DatabaseKind::Postgres);
        let Some(form) = self.create_table_form.as_mut() else {
            return;
        };
        let next_index = form.columns.len() + 1;
        form.columns
            .push(default_create_table_regular_column(kind, next_index));
        form.selected_row = form.columns.len();
        form.selected_focus = CreateTableFieldFocus::ColumnName;
    }

    fn remove_create_table_form_column(&mut self) -> Result<()> {
        let form = self
            .create_table_form
            .as_mut()
            .ok_or_else(|| anyhow!("create table form is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        if form.columns.len() <= 1 {
            return Err(anyhow!("a table needs at least one column"));
        }
        if column_index >= form.columns.len() {
            return Err(anyhow!(
                "selected create table column is no longer available"
            ));
        }
        form.columns.remove(column_index);
        form.selected_row = form.selected_row.min(form.columns.len());
        if form.selected_row == 0 {
            form.selected_focus = CreateTableFieldFocus::TableName;
        } else {
            form.selected_focus = CreateTableFieldFocus::ColumnName;
        }
        Ok(())
    }

    fn preview_create_table_form(&mut self) -> Result<()> {
        let form = self
            .create_table_form
            .as_ref()
            .ok_or_else(|| anyhow!("create table form is not open"))?;

        let table_name = form.table_name.trim().to_string();
        if table_name.is_empty() {
            return Err(anyhow!("table name cannot be empty"));
        }
        if form.columns.is_empty() {
            return Err(anyhow!(
                "add at least one column before previewing CREATE TABLE"
            ));
        }

        let connection_kind = self
            .sessions
            .get(form.connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let options = create_table_type_options(connection_kind);
        let mut seen_names = BTreeSet::new();
        let mut columns = Vec::with_capacity(form.columns.len());
        for column in &form.columns {
            let name = column.name.trim();
            if name.is_empty() {
                return Err(anyhow!("column names cannot be empty"));
            }
            if !seen_names.insert(name.to_string()) {
                return Err(anyhow!("column names must be unique"));
            }
            let option = options
                .get(column.type_index)
                .copied()
                .unwrap_or_else(|| default_create_table_primary_type(connection_kind));
            columns.push(CreateTableColumnTemplate {
                name,
                data_type: option.sql,
                default_value: (!column.default_value.trim().is_empty())
                    .then_some(column.default_value.trim()),
                nullable: column.nullable,
                unique: column.unique,
                auto_increment: column.auto_increment,
                primary_key: column.primary_key,
            });
        }

        let sql = create_table_template(
            connection_kind,
            self.connection_capabilities(form.connection_index)?
                .identifier_quote_style,
            &form.schema_name,
            &table_name,
            &columns,
        );
        let connection_index = form.connection_index;
        let database_name = form.database_name.clone();
        let schema_name = form.schema_name.clone();
        self.create_table_form = None;
        let title = format!(
            "Create Table {}.{}.{}",
            database_name, schema_name, table_name
        );
        self.open_editor_tab(connection_index, Some(database_name.clone()), title, sql);
        self.set_active_editor_post_execute_refresh_target(DbObjectRef {
            database: database_name.clone(),
            schema: schema_name,
            name: table_name,
            kind: DbObjectKind::Table,
        });
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.status = Some(
                "Generated CREATE TABLE SQL. Review it, then run with Ctrl-Enter.".to_string(),
            );
        }
        self.workspace_status =
            Some("Generated CREATE TABLE SQL in the editor; run it with Ctrl-Enter.".to_string());
        Ok(())
    }

    fn open_structure_editor(&mut self) -> Result<()> {
        if self.active_right_tab != RightPaneTab::Structure {
            return Err(anyhow!(
                "open the Structure tab before editing table structure"
            ));
        }
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let object = self
            .structure
            .object
            .clone()
            .or_else(|| self.selected_object().cloned())
            .ok_or_else(|| anyhow!("select a table-like object before editing structure"))?;
        if !object.kind.supports_staged_crud() {
            return Err(anyhow!("structure editing is only available for tables"));
        }
        if self.structure.columns.is_empty() {
            return Err(anyhow!("load the table structure before editing it"));
        }

        let columns = self
            .structure
            .columns
            .iter()
            .map(|column| StructureEditorColumnState {
                original_name: Some(column.name.clone()),
                original_data_type: Some(column.data_type.clone()),
                original_nullable: Some(column.nullable),
                original_default: column
                    .has_default
                    .then_some(STRUCTURE_EDITOR_EXISTING_DEFAULT_SENTINEL.to_string()),
                original_unique: Some(column.is_unique),
                original_primary_key: Some(column.is_primary_key),
                name: column.name.clone(),
                data_type: column.data_type.clone(),
                default_value: if column.has_default {
                    STRUCTURE_EDITOR_EXISTING_DEFAULT_SENTINEL.to_string()
                } else {
                    String::new()
                },
                nullable: column.nullable,
                unique: column.is_unique,
                primary_key: column.is_primary_key,
            })
            .collect::<Vec<_>>();
        let anchor_row = self
            .grid_selected_row_index()
            .saturating_add(1)
            .clamp(1, columns.len());

        self.structure_editor_form = Some(StructureEditorFormState {
            connection_index,
            database_name: object.database.clone(),
            schema_name: object.schema.clone(),
            old_table_name: object.name.clone(),
            object_kind: object.kind,
            table_name: object.name,
            selected_row: 0,
            selected_focus: StructureEditorFieldFocus::TableName,
            anchor_row,
            columns,
        });
        Ok(())
    }

    fn close_structure_editor_form(&mut self) {
        self.structure_editor_form = None;
    }

    fn move_structure_editor_form_selection(&mut self, delta: isize) {
        let Some(form) = self.structure_editor_form.as_mut() else {
            return;
        };
        let row_count = form.columns.len() + 1;
        if row_count == 0 {
            form.selected_row = 0;
            return;
        }

        if form.selected_row == 0 {
            if delta.is_positive() && !form.columns.is_empty() {
                form.selected_row = form.anchor_row.min(form.columns.len());
                form.selected_focus = StructureEditorFieldFocus::ColumnName;
            } else {
                form.selected_focus = StructureEditorFieldFocus::TableName;
            }
            return;
        }

        let offset = delta.unsigned_abs() % row_count;
        form.selected_row = if delta.is_negative() {
            (form.selected_row + row_count - offset) % row_count
        } else {
            (form.selected_row + offset) % row_count
        };

        if form.selected_row == 0 {
            form.selected_focus = StructureEditorFieldFocus::TableName;
        } else if form.selected_focus == StructureEditorFieldFocus::TableName {
            form.selected_focus = StructureEditorFieldFocus::ColumnName;
        }
    }

    fn move_structure_editor_form_focus(&mut self, delta: isize) {
        let Some(form) = self.structure_editor_form.as_mut() else {
            return;
        };
        if form.selected_row == 0 {
            if delta.is_positive() && !form.columns.is_empty() {
                form.selected_row = form.anchor_row.min(form.columns.len());
                form.selected_focus = StructureEditorFieldFocus::ColumnName;
            } else {
                form.selected_focus = StructureEditorFieldFocus::TableName;
            }
            return;
        }

        let current = if form.selected_focus == StructureEditorFieldFocus::TableName {
            StructureEditorFieldFocus::ColumnName
        } else {
            form.selected_focus
        };
        let next = current.cycle(delta);

        if delta.is_positive()
            && current == StructureEditorFieldFocus::PrimaryKey
            && form.selected_row < form.columns.len()
        {
            form.selected_row += 1;
            form.selected_focus = StructureEditorFieldFocus::ColumnName;
            return;
        }

        if delta.is_negative() && current == StructureEditorFieldFocus::ColumnName {
            if form.selected_row > 1 {
                form.selected_row -= 1;
                form.selected_focus = StructureEditorFieldFocus::PrimaryKey;
            } else {
                form.selected_row = 0;
                form.selected_focus = StructureEditorFieldFocus::TableName;
            }
            return;
        }

        form.selected_focus = next;
    }

    fn cycle_structure_editor_form_column_type(&mut self, delta: isize) -> Result<()> {
        let connection_index = self
            .structure_editor_form
            .as_ref()
            .map(|form| form.connection_index)
            .ok_or_else(|| anyhow!("structure editor is not open"))?;
        let kind = self
            .sessions
            .get(connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let options = create_table_type_options(kind);
        let form = self
            .structure_editor_form
            .as_mut()
            .ok_or_else(|| anyhow!("structure editor is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        let column = form
            .columns
            .get_mut(column_index)
            .ok_or_else(|| anyhow!("selected structure column is no longer available"))?;
        let len = options.len();
        let current_index = options
            .iter()
            .position(|option| option.label.eq_ignore_ascii_case(column.data_type.trim()))
            .unwrap_or(0);
        let offset = delta.unsigned_abs() % len;
        let next_index = if delta.is_negative() {
            (current_index + len - offset) % len
        } else {
            (current_index + offset) % len
        };
        column.data_type = options[next_index].label.to_string();
        Ok(())
    }

    fn toggle_structure_editor_form_nullable(&mut self) -> Result<()> {
        let form = self
            .structure_editor_form
            .as_mut()
            .ok_or_else(|| anyhow!("structure editor is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        let column = form
            .columns
            .get_mut(column_index)
            .ok_or_else(|| anyhow!("selected structure column is no longer available"))?;
        column.nullable = !column.nullable;
        if column.nullable {
            column.primary_key = false;
        }
        Ok(())
    }

    fn toggle_structure_editor_form_unique(&mut self) -> Result<()> {
        let form = self
            .structure_editor_form
            .as_mut()
            .ok_or_else(|| anyhow!("structure editor is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        let column = form
            .columns
            .get_mut(column_index)
            .ok_or_else(|| anyhow!("selected structure column is no longer available"))?;
        if column.primary_key {
            column.unique = false;
        } else {
            column.unique = !column.unique;
        }
        Ok(())
    }

    fn toggle_structure_editor_form_primary_key(&mut self) -> Result<()> {
        let form = self
            .structure_editor_form
            .as_mut()
            .ok_or_else(|| anyhow!("structure editor is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        if column_index >= form.columns.len() {
            return Err(anyhow!("selected structure column is no longer available"));
        }

        let enable = !form.columns[column_index].primary_key;
        for (index, column) in form.columns.iter_mut().enumerate() {
            column.primary_key = enable && index == column_index;
            if column.primary_key {
                column.nullable = false;
                column.unique = false;
            }
        }
        Ok(())
    }

    fn add_structure_editor_form_column(&mut self) {
        let kind = self
            .structure_editor_form
            .as_ref()
            .and_then(|form| self.sessions.get(form.connection_index))
            .map(|session| session.kind)
            .unwrap_or(DatabaseKind::Postgres);
        let Some(form) = self.structure_editor_form.as_mut() else {
            return;
        };
        let next_index = form.columns.len() + 1;
        form.columns
            .push(default_structure_editor_regular_column(kind, next_index));
        form.anchor_row = form.columns.len();
        form.selected_row = form.columns.len();
        form.selected_focus = StructureEditorFieldFocus::ColumnName;
    }

    fn remove_structure_editor_form_column(&mut self) -> Result<()> {
        let form = self
            .structure_editor_form
            .as_mut()
            .ok_or_else(|| anyhow!("structure editor is not open"))?;
        let column_index = form
            .selected_row
            .checked_sub(1)
            .ok_or_else(|| anyhow!("select a column row first"))?;
        if form.columns.len() <= 1 {
            return Err(anyhow!("a table needs at least one column"));
        }
        if column_index >= form.columns.len() {
            return Err(anyhow!("selected structure column is no longer available"));
        }
        form.columns.remove(column_index);
        form.selected_row = form.selected_row.min(form.columns.len());
        form.anchor_row = form.anchor_row.min(form.columns.len()).max(1);
        if form.selected_row == 0 {
            form.selected_focus = StructureEditorFieldFocus::TableName;
        } else {
            form.selected_focus = StructureEditorFieldFocus::ColumnName;
        }
        Ok(())
    }

    fn preview_structure_editor_form(&mut self) -> Result<()> {
        let form = self
            .structure_editor_form
            .as_ref()
            .ok_or_else(|| anyhow!("structure editor is not open"))?;
        let table_name = form.table_name.trim().to_string();
        if table_name.is_empty() {
            return Err(anyhow!("table name cannot be empty"));
        }
        if form.columns.is_empty() {
            return Err(anyhow!("a table needs at least one column"));
        }

        let kind = self
            .sessions
            .get(form.connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let quote_style = self
            .connection_capabilities(form.connection_index)?
            .identifier_quote_style;

        let mut seen_names = BTreeSet::new();
        for column in &form.columns {
            let name = column.name.trim();
            if name.is_empty() {
                return Err(anyhow!("column names cannot be empty"));
            }
            if !seen_names.insert(name.to_string()) {
                return Err(anyhow!("column names must be unique"));
            }
        }

        let mut statements = Vec::new();
        let target_table_name = table_name.as_str();
        let original_primary_key = form
            .columns
            .iter()
            .find(|column| column.original_primary_key.unwrap_or(false));
        let current_primary_key = form.columns.iter().find(|column| column.primary_key);
        let original_primary_identity =
            original_primary_key.and_then(structure_editor_column_identity);
        let current_primary_identity =
            current_primary_key.and_then(structure_editor_column_identity);

        let original_unique_identities = form
            .columns
            .iter()
            .filter(|column| column.original_unique.unwrap_or(false))
            .filter_map(structure_editor_column_identity)
            .collect::<BTreeSet<_>>();
        let current_unique_identities = form
            .columns
            .iter()
            .filter(|column| column.unique && !column.primary_key)
            .filter_map(structure_editor_column_identity)
            .collect::<BTreeSet<_>>();

        if original_primary_identity != current_primary_identity && original_primary_key.is_some() {
            statements.push(drop_primary_key_template(
                kind,
                quote_style,
                &form.schema_name,
                &form.old_table_name,
            ));
        }

        for column in form.columns.iter().filter(|column| {
            column.original_unique.unwrap_or(false)
                && structure_editor_column_identity(column)
                    .is_some_and(|identity| !current_unique_identities.contains(&identity))
        }) {
            let old_name = column
                .original_name
                .as_deref()
                .unwrap_or(column.name.trim());
            statements.push(drop_unique_constraint_template(
                kind,
                quote_style,
                &form.schema_name,
                &form.old_table_name,
                old_name,
            ));
        }

        let retained_original_names = form
            .columns
            .iter()
            .filter_map(|column| column.original_name.as_deref())
            .collect::<BTreeSet<_>>();

        for column in &form.columns {
            let Some(old_name) = column.original_name.as_deref() else {
                continue;
            };
            let name = column.name.trim();
            let data_type = column.data_type.trim();
            let new_default = structure_editor_default_as_option(&column.default_value);
            let sql = alter_column_template(
                kind,
                quote_style,
                &form.schema_name,
                target_table_name,
                AlterColumnTemplate {
                    old_name,
                    new_name: name,
                    old_data_type: column.original_data_type.as_deref().unwrap_or(data_type),
                    new_data_type: data_type,
                    old_nullable: column.original_nullable.unwrap_or(column.nullable),
                    new_nullable: column.nullable,
                    old_default: column.original_default.as_deref(),
                    new_default,
                },
            );
            if !sql.trim_start().starts_with("-- No structural changes") {
                statements.push(sql);
            }
        }

        for original_name in self
            .structure
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .filter(|name| !retained_original_names.contains(name))
        {
            statements.push(drop_column_template(
                kind,
                quote_style,
                &form.schema_name,
                target_table_name,
                original_name,
            ));
        }

        for column in form
            .columns
            .iter()
            .filter(|column| column.original_name.is_none())
        {
            let name = column.name.trim();
            let data_type = column.data_type.trim();
            let new_default = structure_editor_default_as_option(&column.default_value);
            statements.push(add_column_template(
                kind,
                quote_style,
                &form.schema_name,
                target_table_name,
                AddColumnTemplate {
                    name,
                    data_type,
                    nullable: column.nullable,
                    default_value: new_default,
                },
            ));
        }

        if table_name != form.old_table_name {
            statements.push(rename_table_template(
                kind,
                quote_style,
                &form.schema_name,
                RenameTableTemplate {
                    old_name: &form.old_table_name,
                    new_name: &table_name,
                },
            ));
        }

        for column in form.columns.iter().filter(|column| {
            column.unique
                && !column.primary_key
                && structure_editor_column_identity(column)
                    .is_some_and(|identity| !original_unique_identities.contains(&identity))
        }) {
            statements.push(add_unique_constraint_template(
                kind,
                quote_style,
                &form.schema_name,
                target_table_name,
                column.name.trim(),
            ));
        }

        if original_primary_identity != current_primary_identity {
            if let Some(column) = current_primary_key {
                statements.push(add_primary_key_template(
                    kind,
                    quote_style,
                    &form.schema_name,
                    target_table_name,
                    column.name.trim(),
                ));
            }
        }

        let sql = if statements.is_empty() {
            format!(
                "-- No structural changes for {}.{}.",
                form.schema_name, table_name
            )
        } else {
            statements.join("\n")
        };
        let connection_index = form.connection_index;
        let database_name = form.database_name.clone();
        let schema_name = form.schema_name.clone();
        let old_table_name = form.old_table_name.clone();
        let object_kind = form.object_kind;
        self.structure_editor_form = None;
        let title = format!("Edit Table {database_name}.{schema_name}.{old_table_name}");
        self.open_editor_tab(connection_index, Some(database_name.clone()), title, sql);
        self.set_active_editor_post_execute_refresh_target(DbObjectRef {
            database: database_name,
            schema: schema_name,
            name: table_name,
            kind: object_kind,
        });
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.status =
                Some("Generated ALTER TABLE SQL. Review it, then run with Ctrl-Enter.".to_string());
        }
        self.workspace_status =
            Some("Generated ALTER TABLE SQL in the editor; run it with Ctrl-Enter.".to_string());
        Ok(())
    }

    fn open_alter_column_form(&mut self) -> Result<()> {
        if self.active_right_tab != RightPaneTab::Structure {
            return Err(anyhow!(
                "open the Structure tab before editing table structure"
            ));
        }
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let object = self
            .structure
            .object
            .clone()
            .or_else(|| self.selected_object().cloned())
            .ok_or_else(|| anyhow!("select a table-like object before editing structure"))?;
        if !object.kind.supports_staged_crud() {
            return Err(anyhow!("structure editing is only available for tables"));
        }
        let column_index = self.grid_selected_row_index();
        let column = self
            .structure
            .columns
            .get(column_index)
            .cloned()
            .ok_or_else(|| anyhow!("selected structure column is no longer available"))?;
        let kind = self
            .sessions
            .get(connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;

        self.alter_column_form = Some(AlterColumnFormState {
            connection_index,
            database_name: object.database,
            schema_name: object.schema,
            table_name: object.name,
            old_name: column.name.clone(),
            new_name: column.name,
            old_data_type: column.data_type.clone(),
            type_index: create_table_type_index(kind, &column.data_type),
            old_nullable: column.nullable,
            nullable: column.nullable,
            default_value: String::new(),
            selected_focus: AlterColumnFieldFocus::ColumnName,
        });
        Ok(())
    }

    fn close_alter_column_form(&mut self) {
        self.alter_column_form = None;
    }

    fn move_alter_column_form_focus(&mut self, delta: isize) {
        let Some(form) = self.alter_column_form.as_mut() else {
            return;
        };
        form.selected_focus = form.selected_focus.cycle(delta);
    }

    fn cycle_alter_column_form_type(&mut self, delta: isize) -> Result<()> {
        let connection_index = self
            .alter_column_form
            .as_ref()
            .map(|form| form.connection_index)
            .ok_or_else(|| anyhow!("alter column form is not open"))?;
        let kind = self
            .sessions
            .get(connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let options = create_table_type_options(kind);
        let form = self
            .alter_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("alter column form is not open"))?;
        let len = options.len();
        let offset = delta.unsigned_abs() % len;
        form.type_index = if delta.is_negative() {
            (form.type_index + len - offset) % len
        } else {
            (form.type_index + offset) % len
        };
        Ok(())
    }

    fn toggle_alter_column_form_nullable(&mut self) -> Result<()> {
        let form = self
            .alter_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("alter column form is not open"))?;
        form.nullable = !form.nullable;
        Ok(())
    }

    fn preview_alter_column_form(&mut self) -> Result<()> {
        let form = self
            .alter_column_form
            .as_ref()
            .ok_or_else(|| anyhow!("alter column form is not open"))?;
        let new_name = form.new_name.trim().to_string();
        if new_name.is_empty() {
            return Err(anyhow!("column name cannot be empty"));
        }
        let kind = self
            .sessions
            .get(form.connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let options = create_table_type_options(kind);
        let new_type = options
            .get(form.type_index)
            .map(|option| option.sql)
            .unwrap_or_else(|| default_create_table_primary_type(kind).sql);
        let sql = alter_column_template(
            kind,
            self.connection_capabilities(form.connection_index)?
                .identifier_quote_style,
            &form.schema_name,
            &form.table_name,
            AlterColumnTemplate {
                old_name: &form.old_name,
                new_name: &new_name,
                old_data_type: &form.old_data_type,
                new_data_type: new_type,
                old_nullable: form.old_nullable,
                new_nullable: form.nullable,
                old_default: None,
                new_default: (!form.default_value.trim().is_empty())
                    .then_some(form.default_value.trim()),
            },
        );
        let connection_index = form.connection_index;
        let database_name = form.database_name.clone();
        let schema_name = form.schema_name.clone();
        let table_name = form.table_name.clone();
        self.alter_column_form = None;
        let title = format!("Alter Column {database_name}.{schema_name}.{table_name}.{new_name}");
        self.open_editor_tab(connection_index, Some(database_name.clone()), title, sql);
        self.set_active_editor_post_execute_refresh_target(DbObjectRef {
            database: database_name,
            schema: schema_name,
            name: table_name,
            kind: DbObjectKind::Table,
        });
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.status =
                Some("Generated ALTER TABLE SQL. Review it, then run with Ctrl-Enter.".to_string());
        }
        self.workspace_status =
            Some("Generated ALTER TABLE SQL in the editor; run it with Ctrl-Enter.".to_string());
        Ok(())
    }

    fn open_add_column_form(&mut self) -> Result<()> {
        if self.active_right_tab != RightPaneTab::Structure {
            return Err(anyhow!(
                "open the Structure tab before editing table structure"
            ));
        }
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let object = self
            .structure
            .object
            .clone()
            .or_else(|| self.selected_object().cloned())
            .ok_or_else(|| anyhow!("select a table-like object before editing structure"))?;
        if !object.kind.supports_staged_crud() {
            return Err(anyhow!("structure editing is only available for tables"));
        }
        let kind = self
            .sessions
            .get(connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        self.add_column_form = Some(AddColumnFormState {
            connection_index,
            database_name: object.database,
            schema_name: object.schema,
            table_name: object.name,
            name: String::new(),
            type_index: create_table_type_index(kind, "integer"),
            nullable: true,
            default_value: String::new(),
            selected_focus: AddColumnFieldFocus::ColumnName,
        });
        Ok(())
    }

    fn close_add_column_form(&mut self) {
        self.add_column_form = None;
    }

    fn move_add_column_form_focus(&mut self, delta: isize) {
        let Some(form) = self.add_column_form.as_mut() else {
            return;
        };
        form.selected_focus = form.selected_focus.cycle(delta);
    }

    fn cycle_add_column_form_type(&mut self, delta: isize) -> Result<()> {
        let connection_index = self
            .add_column_form
            .as_ref()
            .map(|form| form.connection_index)
            .ok_or_else(|| anyhow!("add column form is not open"))?;
        let kind = self
            .sessions
            .get(connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let options = create_table_type_options(kind);
        let form = self
            .add_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("add column form is not open"))?;
        let len = options.len();
        let offset = delta.unsigned_abs() % len;
        form.type_index = if delta.is_negative() {
            (form.type_index + len - offset) % len
        } else {
            (form.type_index + offset) % len
        };
        Ok(())
    }

    fn toggle_add_column_form_nullable(&mut self) -> Result<()> {
        let form = self
            .add_column_form
            .as_mut()
            .ok_or_else(|| anyhow!("add column form is not open"))?;
        form.nullable = !form.nullable;
        Ok(())
    }

    fn preview_add_column_form(&mut self) -> Result<()> {
        let form = self
            .add_column_form
            .as_ref()
            .ok_or_else(|| anyhow!("add column form is not open"))?;
        let name = form.name.trim().to_string();
        if name.is_empty() {
            return Err(anyhow!("column name cannot be empty"));
        }
        let kind = self
            .sessions
            .get(form.connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let options = create_table_type_options(kind);
        let data_type = options
            .get(form.type_index)
            .map(|option| option.sql)
            .unwrap_or_else(|| default_create_table_primary_type(kind).sql);
        let sql = add_column_template(
            kind,
            self.connection_capabilities(form.connection_index)?
                .identifier_quote_style,
            &form.schema_name,
            &form.table_name,
            AddColumnTemplate {
                name: &name,
                data_type,
                nullable: form.nullable,
                default_value: (!form.default_value.trim().is_empty())
                    .then_some(form.default_value.trim()),
            },
        );
        let connection_index = form.connection_index;
        let database_name = form.database_name.clone();
        let schema_name = form.schema_name.clone();
        let table_name = form.table_name.clone();
        self.add_column_form = None;
        let title = format!("Add Column {database_name}.{schema_name}.{table_name}.{name}");
        self.open_editor_tab(connection_index, Some(database_name.clone()), title, sql);
        self.set_active_editor_post_execute_refresh_target(DbObjectRef {
            database: database_name,
            schema: schema_name,
            name: table_name,
            kind: DbObjectKind::Table,
        });
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.status =
                Some("Generated ALTER TABLE SQL. Review it, then run with Ctrl-Enter.".to_string());
        }
        self.workspace_status =
            Some("Generated ALTER TABLE SQL in the editor; run it with Ctrl-Enter.".to_string());
        Ok(())
    }

    fn open_rename_table_form(&mut self) -> Result<()> {
        if self.active_right_tab != RightPaneTab::Structure {
            return Err(anyhow!(
                "open the Structure tab before editing table structure"
            ));
        }
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let object = self
            .structure
            .object
            .clone()
            .or_else(|| self.selected_object().cloned())
            .ok_or_else(|| anyhow!("select a table-like object before editing structure"))?;
        if !object.kind.supports_staged_crud() {
            return Err(anyhow!("structure editing is only available for tables"));
        }

        self.rename_table_form = Some(RenameTableFormState {
            connection_index,
            database_name: object.database,
            schema_name: object.schema,
            old_name: object.name,
            new_name: String::new(),
        });
        Ok(())
    }

    fn close_rename_table_form(&mut self) {
        self.rename_table_form = None;
    }

    fn preview_rename_table_form(&mut self) -> Result<()> {
        let form = self
            .rename_table_form
            .as_ref()
            .ok_or_else(|| anyhow!("rename table form is not open"))?;
        let new_name = form.new_name.trim().to_string();
        if new_name.is_empty() {
            return Err(anyhow!("table name cannot be empty"));
        }
        let kind = self
            .sessions
            .get(form.connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let sql = rename_table_template(
            kind,
            self.connection_capabilities(form.connection_index)?
                .identifier_quote_style,
            &form.schema_name,
            RenameTableTemplate {
                old_name: &form.old_name,
                new_name: &new_name,
            },
        );
        let connection_index = form.connection_index;
        let database_name = form.database_name.clone();
        let schema_name = form.schema_name.clone();
        let old_name = form.old_name.clone();
        self.rename_table_form = None;
        let title = format!("Rename Table {database_name}.{schema_name}.{old_name}");
        self.open_editor_tab(connection_index, Some(database_name.clone()), title, sql);
        self.set_active_editor_post_execute_refresh_target(DbObjectRef {
            database: database_name,
            schema: schema_name,
            name: new_name,
            kind: DbObjectKind::Table,
        });
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.status =
                Some("Generated ALTER TABLE SQL. Review it, then run with Ctrl-Enter.".to_string());
        }
        self.workspace_status =
            Some("Generated ALTER TABLE SQL in the editor; run it with Ctrl-Enter.".to_string());
        Ok(())
    }

    fn prompt_drop_structure_column(&mut self) -> Result<()> {
        if self.active_right_tab != RightPaneTab::Structure {
            return Err(anyhow!(
                "open the Structure tab before editing table structure"
            ));
        }
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let object = self
            .structure
            .object
            .clone()
            .or_else(|| self.selected_object().cloned())
            .ok_or_else(|| anyhow!("select a table-like object before editing structure"))?;
        if !object.kind.supports_staged_crud() {
            return Err(anyhow!("structure editing is only available for tables"));
        }
        let column = self
            .structure
            .columns
            .get(self.grid_selected_row_index())
            .cloned()
            .ok_or_else(|| anyhow!("selected structure column is no longer available"))?;
        let kind = self
            .sessions
            .get(connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let sql = drop_column_template(
            kind,
            self.connection_capabilities(connection_index)?
                .identifier_quote_style,
            &object.schema,
            &object.name,
            &column.name,
        );
        self.delete_confirmation = Some(DeleteConfirmationState {
            title: format!("Drop Column {}", column.name),
            message: format!(
                "Preview dropping {} from {} before you run it.",
                column.name,
                object.database_qualified_name()
            ),
            sql_preview: sql.clone(),
            warning: "This is a structural change. Review the generated SQL carefully.".to_string(),
            help: "Press y to open the DROP COLUMN SQL in the editor, n or Esc to cancel."
                .to_string(),
            operation: PendingDeleteOperation::PreviewInEditor {
                connection_index,
                database_name: object.database.clone(),
                title: format!(
                    "Drop Column {}.{}.{}",
                    object.database, object.schema, column.name
                ),
                sql,
                status: Some(
                    "Generated DROP COLUMN SQL in the editor; run it with Ctrl-Enter.".to_string(),
                ),
                refresh_target: Some(object),
            },
        });
        Ok(())
    }

    fn open_create_index_form(&mut self) -> Result<()> {
        if self.active_right_tab != RightPaneTab::Structure {
            return Err(anyhow!(
                "open the Structure tab before editing table structure"
            ));
        }
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let object = self
            .structure
            .object
            .clone()
            .or_else(|| self.selected_object().cloned())
            .ok_or_else(|| anyhow!("select a table-like object before editing structure"))?;
        if !object.kind.supports_staged_crud() {
            return Err(anyhow!("index editing is only available for tables"));
        }
        let column = self
            .structure
            .columns
            .get(self.grid_selected_row_index())
            .cloned()
            .ok_or_else(|| anyhow!("selected structure column is no longer available"))?;
        self.create_index_form = Some(CreateIndexFormState {
            connection_index,
            database_name: object.database,
            schema_name: object.schema,
            table_name: object.name.clone(),
            column_name: column.name.clone(),
            index_name: default_index_name(&object.name, &column.name),
            unique: false,
        });
        Ok(())
    }

    fn close_create_index_form(&mut self) {
        self.create_index_form = None;
    }

    fn toggle_create_index_form_unique(&mut self) -> Result<()> {
        let form = self
            .create_index_form
            .as_mut()
            .ok_or_else(|| anyhow!("create index form is not open"))?;
        form.unique = !form.unique;
        Ok(())
    }

    fn preview_create_index_form(&mut self) -> Result<()> {
        let form = self
            .create_index_form
            .as_ref()
            .ok_or_else(|| anyhow!("create index form is not open"))?;
        let index_name = form.index_name.trim().to_string();
        if index_name.is_empty() {
            return Err(anyhow!("index name cannot be empty"));
        }
        let kind = self
            .sessions
            .get(form.connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let sql = create_index_template(
            kind,
            self.connection_capabilities(form.connection_index)?
                .identifier_quote_style,
            &form.schema_name,
            &form.table_name,
            CreateIndexTemplate {
                index_name: &index_name,
                column_name: &form.column_name,
                unique: form.unique,
            },
        );
        let connection_index = form.connection_index;
        let database_name = form.database_name.clone();
        let schema_name = form.schema_name.clone();
        let table_name = form.table_name.clone();
        self.create_index_form = None;
        let title = format!("Create Index {database_name}.{schema_name}.{table_name}");
        self.open_editor_tab(connection_index, Some(database_name.clone()), title, sql);
        self.set_active_editor_post_execute_refresh_target(DbObjectRef {
            database: database_name,
            schema: schema_name,
            name: table_name,
            kind: DbObjectKind::Table,
        });
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.status = Some(
                "Generated CREATE INDEX SQL. Review it, then run with Ctrl-Enter.".to_string(),
            );
        }
        self.workspace_status =
            Some("Generated CREATE INDEX SQL in the editor; run it with Ctrl-Enter.".to_string());
        Ok(())
    }

    fn open_drop_index_form(&mut self) -> Result<()> {
        if self.active_right_tab != RightPaneTab::Structure {
            return Err(anyhow!(
                "open the Structure tab before editing table structure"
            ));
        }
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let object = self
            .structure
            .object
            .clone()
            .or_else(|| self.selected_object().cloned())
            .ok_or_else(|| anyhow!("select a table-like object before editing structure"))?;
        if !object.kind.supports_staged_crud() {
            return Err(anyhow!("index editing is only available for tables"));
        }
        let column = self
            .structure
            .columns
            .get(self.grid_selected_row_index())
            .cloned()
            .ok_or_else(|| anyhow!("selected structure column is no longer available"))?;
        self.drop_index_form = Some(DropIndexFormState {
            connection_index,
            database_name: object.database,
            schema_name: object.schema,
            table_name: object.name.clone(),
            index_name: default_index_name(&object.name, &column.name),
        });
        Ok(())
    }

    fn close_drop_index_form(&mut self) {
        self.drop_index_form = None;
    }

    fn preview_drop_index_form(&mut self) -> Result<()> {
        let form = self
            .drop_index_form
            .as_ref()
            .ok_or_else(|| anyhow!("drop index form is not open"))?;
        let index_name = form.index_name.trim().to_string();
        if index_name.is_empty() {
            return Err(anyhow!("index name cannot be empty"));
        }
        let kind = self
            .sessions
            .get(form.connection_index)
            .map(|session| session.kind)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let sql = drop_index_template(
            kind,
            self.connection_capabilities(form.connection_index)?
                .identifier_quote_style,
            &form.schema_name,
            &form.table_name,
            &index_name,
        );
        let connection_index = form.connection_index;
        let database_name = form.database_name.clone();
        let schema_name = form.schema_name.clone();
        let table_name = form.table_name.clone();
        self.drop_index_form = None;
        let title = format!("Drop Index {database_name}.{schema_name}.{table_name}");
        self.open_editor_tab(connection_index, Some(database_name.clone()), title, sql);
        self.set_active_editor_post_execute_refresh_target(DbObjectRef {
            database: database_name,
            schema: schema_name,
            name: table_name,
            kind: DbObjectKind::Table,
        });
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.status =
                Some("Generated DROP INDEX SQL. Review it, then run with Ctrl-Enter.".to_string());
        }
        self.workspace_status =
            Some("Generated DROP INDEX SQL in the editor; run it with Ctrl-Enter.".to_string());
        Ok(())
    }

    fn open_insert_row_form(&mut self) -> Result<()> {
        let (connection_index, object) = self.selected_table_target()?;
        let using_loaded_structure = self
            .structure
            .object
            .as_ref()
            .is_some_and(|current| current == &object)
            && !self.structure.columns.is_empty();
        let fields = self.insert_row_form_fields_for_object(&object)?;
        if fields.is_empty() {
            return Err(anyhow!("no columns are available for insert"));
        }
        let selected_field = fields
            .iter()
            .position(|field| {
                !field.is_primary_key
                    && !field.has_default
                    && !field.name.eq_ignore_ascii_case("id")
            })
            .or_else(|| {
                fields
                    .iter()
                    .position(|field| !field.is_primary_key && !field.has_default)
            })
            .unwrap_or(0);
        self.insert_row_form = Some(InsertRowFormState {
            connection_index,
            object,
            selected_field,
            fields,
        });
        if !using_loaded_structure {
            let object = self
                .insert_row_form
                .as_ref()
                .map(|form| form.object.clone())
                .ok_or_else(|| anyhow!("insert row form is no longer available"))?;
            self.schedule_structure_for_connection_object(connection_index, object.clone())?;
            self.workspace_status = Some(format!(
                "Loading column types for {}...",
                object.database_qualified_name()
            ));
        }
        Ok(())
    }

    fn close_insert_row_form(&mut self) {
        self.insert_row_form = None;
    }

    fn move_insert_row_form_selection(&mut self, delta: isize) {
        let Some(form) = self.insert_row_form.as_mut() else {
            return;
        };
        if form.fields.is_empty() {
            form.selected_field = 0;
            return;
        }
        let field_count = form.fields.len();
        let offset = delta.unsigned_abs() % field_count;
        form.selected_field = if delta.is_negative() {
            (form.selected_field + field_count - offset) % field_count
        } else {
            (form.selected_field + offset) % field_count
        };
    }

    fn preview_insert_row_form(&mut self) -> Result<()> {
        let form = self
            .insert_row_form
            .as_ref()
            .ok_or_else(|| anyhow!("insert row form is not open"))?;
        let capabilities = self.connection_capabilities(form.connection_index)?;
        if !capabilities.supports_staged_crud {
            return Err(anyhow!(
                "the current connection does not support staged CRUD"
            ));
        }

        let values = form
            .fields
            .iter()
            .filter_map(|field| {
                let value = field.value.trim();
                (!value.is_empty()).then_some((field.name.clone(), value.to_string()))
            })
            .collect::<Vec<_>>();
        if values.is_empty() {
            return Err(anyhow!(
                "enter at least one value before previewing the INSERT"
            ));
        }

        let object = form.object.clone();
        let connection_index = form.connection_index;
        let sql = staged_insert_sql(capabilities, &object, &values)
            .ok_or_else(|| anyhow!("could not build staged INSERT SQL"))?;
        self.insert_row_form = None;
        self.open_editor_tab(
            connection_index,
            Some(object.database.clone()),
            format!("Stage INSERT {}", object.database_qualified_name()),
            sql.preview_sql.clone(),
        );
        self.staged_crud = Some(StagedCrudState {
            connection_index,
            sql,
        });
        self.workspace_status =
            Some("Preview staged INSERT; commit with Ctrl-G or command palette.".to_string());
        Ok(())
    }

    fn confirm_save_sql(&mut self) -> Result<()> {
        let dialog = self
            .save_sql_dialog
            .take()
            .ok_or_else(|| anyhow!("save SQL dialog is not open"))?;
        let name = dialog.name.trim();
        if name.is_empty() {
            return Err(anyhow!("saved SQL name cannot be empty"));
        }

        let tab = self
            .active_editor_tab()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        let sql = tab.buffer.sql();
        if sql.trim().is_empty() {
            return Err(anyhow!("enter SQL before saving it"));
        }

        let existing_saved_query_name = tab.saved_query_name.clone();
        let connection_name = self
            .sessions
            .get(tab.connection_index)
            .map(|session| session.name.clone());
        let database_name = tab.database_name.clone();
        let schema_name = existing_saved_query_name
            .as_deref()
            .and_then(|existing_name| {
                self.saved_sql
                    .entries
                    .iter()
                    .find(|entry| entry.name == existing_name)
                    .and_then(|entry| entry.schema_name.clone())
            })
            .or_else(|| self.selected_schema_name().map(str::to_owned));
        let title = name.to_string();
        let entry = SavedSqlEntry {
            name: title.clone(),
            sql,
            connection_name,
            database_name,
            schema_name,
        };
        if let Some(existing_name) = existing_saved_query_name.as_deref() {
            if existing_name != name {
                self.saved_sql.remove_by_name(existing_name);
            }
        }
        self.saved_sql.upsert(entry);
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.saved_query_name = Some(title.clone());
            tab.title = title.clone();
            tab.status = Some(format!("Saved SQL `{title}`."));
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.rebuild_tab_strip();
        }
        let selected_key = self
            .entries
            .get(self.selected_row)
            .map(|entry| entry.key.clone());
        self.rebuild_rows(selected_key);
        self.workspace_status = Some(format!("Saved SQL `{title}`."));
        Ok(())
    }

    fn delete_saved_sql_from_editor(&mut self) -> Result<()> {
        let tab = self
            .active_editor_tab()
            .ok_or_else(|| anyhow!("sql editor is not open"))?;
        let name = tab
            .saved_query_name
            .clone()
            .ok_or_else(|| anyhow!("the active SQL tab is not linked to a saved query"))?;
        self.delete_confirmation = Some(DeleteConfirmationState {
            title: "Delete Saved SQL".to_string(),
            message: format!("Remove saved query `{name}` from this workspace?"),
            sql_preview: name.clone(),
            warning: "Only the saved query entry is removed. The current editor tab stays open."
                .to_string(),
            help: "Press y to delete, n or Esc to cancel.".to_string(),
            operation: PendingDeleteOperation::DeleteSavedQuery {
                name: name.clone(),
                tab_id: tab.id,
            },
        });
        self.workspace_status = Some(format!("Saved SQL `{name}` is waiting for confirmation."));
        Ok(())
    }

    fn open_data_filter(&mut self) -> Result<()> {
        self.selected_preview_target()?;
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

        let (connection_index, object) = self.selected_preview_target()?;

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
        let quote_style = self
            .active_connection_capabilities()
            .ok_or_else(|| anyhow!("no connection is selected"))?
            .identifier_quote_style;
        let clause = where_clause_for_row(quote_style, &grid.columns, row, &key_columns);
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
        if !object.kind.supports_staged_crud() {
            return Err(anyhow!(
                "staged row editing is only available for tables and foreign tables"
            ));
        }
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
            .as_ref()
            .ok_or_else(|| anyhow!("cell edit is not open"))?;
        let capabilities = self.connection_capabilities(edit.connection_index)?;
        if !capabilities.supports_staged_crud {
            return Err(anyhow!(
                "the current connection does not support staged CRUD"
            ));
        }
        let key_columns = self.selected_key_columns();
        let sql = staged_update_sql(
            capabilities,
            &edit.object,
            self.active_grid(),
            edit.row_index,
            edit.column_index,
            &edit.input,
            &key_columns,
        )
        .ok_or_else(|| anyhow!("could not build staged CRUD SQL"))?;
        let edit = self
            .cell_edit
            .take()
            .ok_or_else(|| anyhow!("cell edit is not open"))?;

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

    fn preview_delete_current_row(&mut self) -> Result<()> {
        let (connection_index, object) = self.selected_table_target()?;
        let capabilities = self.connection_capabilities(connection_index)?;
        if !capabilities.supports_staged_crud {
            return Err(anyhow!(
                "the current connection does not support staged CRUD"
            ));
        }

        let row_index = self.grid_selected_row_index();
        let key_columns = self.selected_key_columns();
        let sql = staged_delete_sql(
            capabilities,
            &object,
            self.active_grid(),
            row_index,
            &key_columns,
        )
        .ok_or_else(|| anyhow!("could not build staged DELETE SQL"))?;

        self.open_editor_tab(
            connection_index,
            Some(object.database.clone()),
            format!("Stage DELETE {}", object.database_qualified_name()),
            sql.preview_sql.clone(),
        );
        self.staged_crud = Some(StagedCrudState {
            connection_index,
            sql,
        });
        self.workspace_status =
            Some("Preview staged DELETE; commit with Ctrl-G or command palette.".to_string());
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
            None,
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

    fn save_sql_dialog_view(&self) -> Option<SaveSqlDialogView<'_>> {
        let dialog = self.save_sql_dialog.as_ref()?;
        Some(SaveSqlDialogView { name: &dialog.name })
    }

    pub fn create_table_form_snapshot(&self) -> Option<CreateTableFormSnapshot> {
        let form = self.create_table_form.as_ref()?;
        let options = create_table_type_options(
            self.sessions
                .get(form.connection_index)
                .map(|session| session.kind)
                .unwrap_or(DatabaseKind::Postgres),
        );
        Some(CreateTableFormSnapshot {
            database_name: form.database_name.clone(),
            schema_name: form.schema_name.clone(),
            table_name: form.table_name.clone(),
            selected_row: form.selected_row,
            selected_focus: form.selected_focus.into_view(),
            columns: form
                .columns
                .iter()
                .map(|column| CreateTableColumnSnapshot {
                    name: column.name.clone(),
                    type_label: options
                        .get(column.type_index)
                        .map(|option| option.label.to_string())
                        .unwrap_or_else(|| options[0].label.to_string()),
                    default_value: (!column.default_value.trim().is_empty())
                        .then_some(column.default_value.clone()),
                    nullable: column.nullable,
                    unique: column.unique,
                    auto_increment: column.auto_increment,
                    primary_key: column.primary_key,
                })
                .collect(),
        })
    }

    pub fn structure_editor_form_snapshot(&self) -> Option<StructureEditorFormSnapshot> {
        let form = self.structure_editor_form.as_ref()?;
        Some(StructureEditorFormSnapshot {
            database_name: form.database_name.clone(),
            schema_name: form.schema_name.clone(),
            old_table_name: form.old_table_name.clone(),
            table_name: form.table_name.clone(),
            selected_row: form.selected_row,
            selected_focus: form.selected_focus.into_view(),
            columns: form
                .columns
                .iter()
                .map(|column| StructureEditorColumnSnapshot {
                    name: column.name.clone(),
                    type_label: column.data_type.clone(),
                    default_value: structure_editor_default_display_value(&column.default_value),
                    nullable: column.nullable,
                    unique: column.unique,
                    primary_key: column.primary_key,
                    existing: column.original_name.is_some(),
                })
                .collect(),
        })
    }

    pub fn alter_column_form_snapshot(&self) -> Option<AlterColumnFormSnapshot> {
        let form = self.alter_column_form.as_ref()?;
        let options = create_table_type_options(
            self.sessions
                .get(form.connection_index)
                .map(|session| session.kind)
                .unwrap_or(DatabaseKind::Postgres),
        );
        Some(AlterColumnFormSnapshot {
            database_name: form.database_name.clone(),
            schema_name: form.schema_name.clone(),
            table_name: form.table_name.clone(),
            old_name: form.old_name.clone(),
            new_name: form.new_name.clone(),
            type_label: options
                .get(form.type_index)
                .map(|option| option.label.to_string())
                .unwrap_or_else(|| options[0].label.to_string()),
            default_value: (!form.default_value.trim().is_empty())
                .then_some(form.default_value.clone()),
            nullable: form.nullable,
            selected_focus: form.selected_focus.into_view(),
        })
    }

    pub fn add_column_form_snapshot(&self) -> Option<AddColumnFormSnapshot> {
        let form = self.add_column_form.as_ref()?;
        let options = create_table_type_options(
            self.sessions
                .get(form.connection_index)
                .map(|session| session.kind)
                .unwrap_or(DatabaseKind::Postgres),
        );
        Some(AddColumnFormSnapshot {
            database_name: form.database_name.clone(),
            schema_name: form.schema_name.clone(),
            table_name: form.table_name.clone(),
            name: form.name.clone(),
            type_label: options
                .get(form.type_index)
                .map(|option| option.label.to_string())
                .unwrap_or_else(|| options[0].label.to_string()),
            nullable: form.nullable,
            default_value: (!form.default_value.trim().is_empty())
                .then_some(form.default_value.clone()),
            selected_focus: form.selected_focus.into_view(),
        })
    }

    pub fn rename_table_form_snapshot(&self) -> Option<RenameTableFormSnapshot> {
        let form = self.rename_table_form.as_ref()?;
        Some(RenameTableFormSnapshot {
            database_name: form.database_name.clone(),
            schema_name: form.schema_name.clone(),
            old_name: form.old_name.clone(),
            new_name: form.new_name.clone(),
        })
    }

    pub fn create_index_form_snapshot(&self) -> Option<CreateIndexFormSnapshot> {
        let form = self.create_index_form.as_ref()?;
        Some(CreateIndexFormSnapshot {
            database_name: form.database_name.clone(),
            schema_name: form.schema_name.clone(),
            table_name: form.table_name.clone(),
            column_name: form.column_name.clone(),
            index_name: form.index_name.clone(),
            unique: form.unique,
        })
    }

    pub fn drop_index_form_snapshot(&self) -> Option<DropIndexFormSnapshot> {
        let form = self.drop_index_form.as_ref()?;
        Some(DropIndexFormSnapshot {
            database_name: form.database_name.clone(),
            schema_name: form.schema_name.clone(),
            table_name: form.table_name.clone(),
            index_name: form.index_name.clone(),
        })
    }

    pub fn insert_row_form_snapshot(&self) -> Option<InsertRowFormSnapshot> {
        let form = self.insert_row_form.as_ref()?;
        let date_picker = form.fields.get(form.selected_field).and_then(|field| {
            field.kind.supports_date_picker().then(|| {
                let date = field.date_value.unwrap_or_else(InsertRowDateValue::today);
                let time_value = field.kind.supports_time_picker().then(|| {
                    field
                        .time_value
                        .unwrap_or_else(InsertRowTimeValue::midnight)
                });
                let active_segment = field.time_segment.map(InsertRowDateTimeSegment::into_view);
                field
                    .date_value
                    .unwrap_or_else(InsertRowDateValue::today)
                    .snapshot(
                        if field.value.trim().is_empty() {
                            render_insert_row_form_field_temporal_value(
                                field.kind, "", date, time_value,
                            )
                        } else {
                            field.value.trim().to_string()
                        },
                        time_value.map(InsertRowTimeValue::to_hms_string),
                        active_segment,
                    )
            })
        });
        Some(InsertRowFormSnapshot {
            object_label: form.object.database_qualified_name(),
            selected_index: form.selected_field,
            fields: form
                .fields
                .iter()
                .map(|field| InsertRowFieldSnapshot {
                    name: field.name.clone(),
                    data_type: field.data_type.clone(),
                    value: field.value.clone(),
                    required: !field.nullable && !field.has_default && !field.is_primary_key,
                    kind: field.kind.into_view(),
                })
                .collect(),
            date_picker,
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
            warning: &confirmation.warning,
            help: &confirmation.help,
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
            self.insert_row_form = None;
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
                let group_key = (database.clone(), schema.clone(), kind);
                let should_load_group =
                    !self.sessions[connection].loaded_groups.contains(&group_key);
                toggle_set(
                    &mut self.sessions[connection].expanded_groups,
                    group_key.clone(),
                );
                if should_load_group
                    && self.sessions[connection]
                        .expanded_groups
                        .contains(&group_key)
                {
                    self.schedule_schema_objects_for_connection_group(
                        connection,
                        database.clone(),
                        schema.clone(),
                        kind,
                    )?;
                }
                self.rebuild_rows(Some(TreeNodeKey::Group {
                    connection,
                    database,
                    schema,
                    kind,
                }));
            }
            TreeNodeKey::SavedQueryGroup {
                connection,
                database,
                schema,
            } => {
                toggle_set(
                    &mut self.sessions[connection].expanded_saved_query_groups,
                    (database.clone(), schema.clone()),
                );
                self.rebuild_rows(Some(TreeNodeKey::SavedQueryGroup {
                    connection,
                    database,
                    schema,
                }));
            }
            TreeNodeKey::Object { .. } => {
                self.ensure_selected_object_preview()?;
            }
            TreeNodeKey::SavedQuery { .. } => {}
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

        let structure_active = self.active_right_tab == RightPaneTab::Structure;
        {
            let session = self
                .sessions
                .get_mut(connection)
                .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
            session
                .app
                .select_object_locally(&object.database, &object.schema, &object.name)?;

            if !object.kind.supports_data_preview() {
                session.app.clear_preview();
                session.app.set_status(preview_unavailable_message(&object));
            }
        }

        if !object.kind.supports_data_preview() {
            self.preview_has_next_page = false;
            if structure_active {
                self.schedule_structure_for_connection_object(connection, object)?;
            }
            return Ok(());
        }

        self.schedule_preview_for_connection_object(connection, object.clone())?;
        if structure_active {
            self.schedule_structure_for_connection_object(connection, object)?;
        }
        Ok(())
    }

    fn selected_preview_target(&self) -> Result<(usize, DbObjectRef)> {
        let (connection_index, object) = self
            .selected_object_target()
            .ok_or_else(|| anyhow!("select a previewable object first"))?;
        if !object.kind.supports_data_preview() {
            return Err(anyhow!(preview_unavailable_message(&object)));
        }
        Ok((connection_index, object))
    }

    fn open_sql_editor(&mut self) -> Result<()> {
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let capabilities = self.connection_capabilities(connection_index)?;
        let database_name = self.selected_database_name().map(str::to_owned);
        let sql = self
            .selected_object()
            .map(|object| {
                select_template(
                    capabilities,
                    object,
                    self.selected_preview_limit().unwrap_or(100),
                )
            })
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
        let sql = select_template(
            self.connection_capabilities(connection_index)?,
            &object,
            self.selected_preview_limit().unwrap_or(100),
        );
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
        let (connection_index, sql, refresh_target) = {
            let tab = self
                .active_editor_tab()
                .ok_or_else(|| anyhow!("sql editor is not open"))?;
            (
                tab.connection_index,
                tab.buffer.current_statement(),
                tab.post_execute_refresh_target.clone(),
            )
        };
        if sql.trim().is_empty() {
            return Err(anyhow!("current SQL statement is empty"));
        }
        self.execute_sql_with_delete_confirmation(
            connection_index,
            sql,
            Some("Executing current SQL statement..."),
            refresh_target,
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
        let sql = explain_sql(
            self.connection_capabilities(connection_index)?,
            &statement,
            analyze,
        )?;
        let status = if analyze {
            "Running EXPLAIN ANALYZE..."
        } else {
            "Running EXPLAIN..."
        };
        self.execute_sql_with_delete_confirmation(connection_index, sql, Some(status), None)
    }

    fn execute_sql_with_delete_confirmation(
        &mut self,
        connection_index: usize,
        sql: String,
        status: Option<&str>,
        refresh_target: Option<DbObjectRef>,
    ) -> Result<()> {
        if let Some(kind) = self.read_only_write_operation(connection_index, &sql)? {
            self.report_blocked_read_only_operation(connection_index, kind);
            return Ok(());
        }

        if let Some(kind) = delete_operation_kind(&sql) {
            self.prompt_delete_operation(
                connection_index,
                sql,
                status.map(str::to_string),
                kind,
                refresh_target,
            )?;
            return Ok(());
        }

        self.execute_sql_on_connection(connection_index, sql, status, refresh_target)
    }

    fn read_only_write_operation(
        &self,
        connection_index: usize,
        sql: &str,
    ) -> Result<Option<WriteOperationKind>> {
        let session = self
            .sessions
            .get(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        if !session.read_only {
            return Ok(None);
        }
        Ok(write_operation_kind(sql))
    }

    fn report_blocked_read_only_operation(
        &mut self,
        connection_index: usize,
        kind: WriteOperationKind,
    ) {
        let connection_name = self
            .sessions
            .get(connection_index)
            .map(|session| session.name.clone())
            .unwrap_or_else(|| "selected connection".to_string());
        let status = format!(
            "Blocked {} on read-only connection `{connection_name}`.",
            kind.label()
        );
        self.workspace_status = Some(status.clone());

        if let Some(editor) = self.editor.as_mut() {
            for tab in &mut editor.tabs {
                if tab.connection_index == connection_index {
                    tab.status = Some(status.clone());
                }
            }
        }
    }

    fn prompt_delete_operation(
        &mut self,
        connection_index: usize,
        sql: String,
        status: Option<String>,
        kind: DeleteOperationKind,
        refresh_target: Option<DbObjectRef>,
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
            warning: "Relora will send this statement to the database only after confirmation."
                .to_string(),
            help: "Press y to execute, n or Esc to cancel.".to_string(),
            operation: PendingDeleteOperation::ExecuteStatement {
                connection_index,
                sql,
                status,
                refresh_target,
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
            PendingDeleteOperation::ExecuteStatement {
                connection_index,
                sql,
                status,
                refresh_target,
            } => self.execute_sql_on_connection(
                connection_index,
                sql,
                status.as_deref(),
                refresh_target,
            ),
            PendingDeleteOperation::PreviewInEditor {
                connection_index,
                database_name,
                title,
                sql,
                status,
                refresh_target,
            } => {
                self.open_editor_tab(connection_index, Some(database_name), title, sql);
                if let Some(refresh_target) = refresh_target {
                    self.set_active_editor_post_execute_refresh_target(refresh_target);
                }
                if let Some(tab) = self.active_editor_tab_mut() {
                    tab.status = status;
                }
                self.workspace_status = Some(
                    "Generated ALTER TABLE SQL in the editor; run it with Ctrl-Enter.".to_string(),
                );
                Ok(())
            }
            PendingDeleteOperation::DeleteSavedQuery { name, tab_id } => {
                self.saved_sql.remove_by_name(&name);
                if let Some(editor) = self.editor.as_mut() {
                    if let Some(tab) = editor.find_tab_mut_by_id(tab_id) {
                        tab.saved_query_name = None;
                        tab.status = Some(format!("Deleted saved SQL `{name}`."));
                    }
                    editor.rebuild_tab_strip();
                }
                let selected_key = self
                    .entries
                    .get(self.selected_row)
                    .map(|entry| entry.key.clone());
                self.rebuild_rows(selected_key);
                self.workspace_status = Some(format!("Deleted saved SQL `{name}`."));
                Ok(())
            }
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
        refresh_target: Option<DbObjectRef>,
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
        session.pending.execute_requests.insert(
            request_id,
            PendingExecuteRequest {
                tab_id,
                refresh_target,
            },
        );

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
                build_rows_for_session(connection_index, session, &self.saved_sql.entries)
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
            | TreeNodeKey::SavedQueryGroup { connection, .. }
            | TreeNodeKey::Object { connection, .. }
            | TreeNodeKey::SavedQuery { connection, .. } => Some(*connection),
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

    fn connection_capabilities(&self, connection_index: usize) -> Result<DriverCapabilities> {
        self.sessions
            .get(connection_index)
            .map(|session| session.capabilities)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))
    }

    fn active_connection_capabilities(&self) -> Option<DriverCapabilities> {
        self.active_editor_connection_index()
            .or_else(|| self.selected_connection_index())
            .and_then(|index| self.sessions.get(index).map(|session| session.capabilities))
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

    fn set_active_editor_post_execute_refresh_target(&mut self, target: DbObjectRef) {
        if let Some(tab) = self.active_editor_tab_mut() {
            tab.post_execute_refresh_target = Some(target);
        }
    }

    fn selected_schema_target(&self) -> Result<(usize, String, String)> {
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let database_name = self
            .selected_database_name()
            .map(str::to_owned)
            .or_else(|| {
                self.sessions
                    .get(connection_index)
                    .and_then(|session| session.app.selected_database_name())
                    .map(str::to_owned)
            })
            .ok_or_else(|| anyhow!("select a database, schema, or object first"))?;
        let schema_name = self.selected_schema_name().map(str::to_owned).or_else(|| {
            self.sessions.get(connection_index).and_then(|session| {
                session
                    .kind
                    .collapses_duplicate_schema(&database_name, &database_name)
                    .then_some(database_name.clone())
            })
        });

        let schema_name = schema_name.ok_or_else(|| anyhow!("select a schema or object first"))?;
        Ok((connection_index, database_name, schema_name))
    }

    fn selected_table_target(&self) -> Result<(usize, DbObjectRef)> {
        let connection_index = self
            .selected_connection_index()
            .ok_or_else(|| anyhow!("no connection is selected"))?;
        let object = self
            .selected_object()
            .cloned()
            .ok_or_else(|| anyhow!("select a table object first"))?;

        if !object.kind.supports_crud_templates() {
            return Err(anyhow!(
                "CRUD templates are only available for tables and foreign tables"
            ));
        }

        Ok((connection_index, object))
    }

    fn insert_row_form_fields_for_object(
        &self,
        object: &DbObjectRef,
    ) -> Result<Vec<InsertRowFormFieldState>> {
        let columns = if self.structure.object.as_ref() == Some(object)
            && !self.structure.columns.is_empty()
        {
            self.structure.columns.clone()
        } else if !self.active_grid().columns.is_empty() {
            self.active_grid()
                .columns
                .iter()
                .map(|name| DbColumn {
                    name: name.clone(),
                    data_type: String::new(),
                    nullable: true,
                    has_default: false,
                    is_unique: false,
                    is_primary_key: false,
                })
                .collect()
        } else {
            return Err(anyhow!(
                "load a preview or structure first so Relora knows which columns to insert"
            ));
        };

        Ok(columns
            .into_iter()
            .map(|column| {
                let kind = classify_insert_row_field_kind(&column.data_type);
                let date_value = kind.supports_date_picker().then(InsertRowDateValue::today);
                let time_value = kind
                    .supports_time_picker()
                    .then(InsertRowTimeValue::midnight);
                InsertRowFormFieldState {
                    name: column.name,
                    data_type: column.data_type,
                    nullable: column.nullable,
                    has_default: column.has_default,
                    is_primary_key: column.is_primary_key,
                    kind,
                    value: String::new(),
                    date_value,
                    time_value,
                    time_segment: InsertRowDateTimeSegment::default_for_kind(kind),
                }
            })
            .collect())
    }

    fn refresh_insert_row_form_field_types(&mut self, object: &DbObjectRef, columns: &[DbColumn]) {
        let Some(form) = self.insert_row_form.as_mut() else {
            return;
        };
        if &form.object != object {
            return;
        }

        let selected_name = form
            .fields
            .get(form.selected_field)
            .map(|field| field.name.clone());
        let existing_fields = form
            .fields
            .iter()
            .cloned()
            .map(|field| (field.name.clone(), field))
            .collect::<BTreeMap<_, _>>();

        let mut fields = Vec::with_capacity(columns.len());
        let mut existing_fields = existing_fields;
        for column in columns {
            let kind = classify_insert_row_field_kind(&column.data_type);
            let mut field =
                existing_fields
                    .remove(&column.name)
                    .unwrap_or(InsertRowFormFieldState {
                        name: column.name.clone(),
                        data_type: column.data_type.clone(),
                        nullable: column.nullable,
                        has_default: column.has_default,
                        is_primary_key: column.is_primary_key,
                        kind,
                        value: String::new(),
                        date_value: kind.supports_date_picker().then(InsertRowDateValue::today),
                        time_value: kind
                            .supports_time_picker()
                            .then(InsertRowTimeValue::midnight),
                        time_segment: InsertRowDateTimeSegment::default_for_kind(kind),
                    });
            field.data_type = column.data_type.clone();
            field.nullable = column.nullable;
            field.has_default = column.has_default;
            field.is_primary_key = column.is_primary_key;
            field.kind = kind;
            if !field.kind.supports_date_picker() {
                field.date_value = None;
            } else if field.date_value.is_none() {
                field.date_value = Some(InsertRowDateValue::today());
            }
            if !field.kind.supports_time_picker() {
                field.time_value = None;
                field.time_segment = None;
            } else if field.time_value.is_none() {
                field.time_value = Some(InsertRowTimeValue::midnight());
                if field.time_segment.is_none() {
                    field.time_segment = InsertRowDateTimeSegment::default_for_kind(field.kind);
                }
            } else if field.time_segment.is_none() {
                field.time_segment = InsertRowDateTimeSegment::default_for_kind(field.kind);
            }
            sync_insert_row_form_field(&mut field);
            fields.push(field);
        }

        if fields.is_empty() {
            return;
        }
        form.selected_field = selected_name
            .as_deref()
            .and_then(|name| fields.iter().position(|field| field.name == name))
            .unwrap_or_else(|| form.selected_field.min(fields.len().saturating_sub(1)));
        form.fields = fields;
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
        if !object.kind.supports_data_preview() {
            let session = self
                .sessions
                .get_mut(connection_index)
                .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
            session.app.clear_preview();
            session.app.set_status(preview_unavailable_message(&object));
            self.preview_has_next_page = false;
            return Ok(());
        }

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
        if !object.kind.supports_data_preview() {
            let session = self
                .sessions
                .get_mut(connection_index)
                .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
            session.app.clear_preview();
            session.app.set_status(preview_unavailable_message(&object));
            self.preview_has_next_page = false;
            return Ok(());
        }

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
        self.schedule_refresh_for_connection_target(connection_index, None)
    }

    fn schedule_refresh_for_connection_target(
        &mut self,
        connection_index: usize,
        refresh_target: Option<DbObjectRef>,
    ) -> Result<()> {
        self.reset_grid_scroll();
        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;

        let mut cancel_ids = Vec::new();
        if let Some(refresh_request) = &session.pending.refresh_request {
            cancel_ids.push(refresh_request.request_id);
        }
        if let Some(preview_request) = &session.pending.preview_request {
            cancel_ids.push(preview_request.request_id);
        }
        if let Some(structure_request) = &session.pending.structure_request {
            cancel_ids.push(structure_request.request_id);
        }
        cancel_ids.extend(session.pending.group_request_ids.values().copied());
        session.worker.cancel_requests(cancel_ids)?;
        session.pending.refresh_request = None;
        session.pending.preview_request = None;
        session.pending.structure_request = None;
        session.pending.group_request_ids.clear();
        if self.active_right_tab == RightPaneTab::Structure {
            self.structure.clear();
        }

        if let Some(target) = refresh_target.as_ref() {
            session.expanded = true;
            session.expanded_databases.insert(target.database.clone());
            session
                .expanded_schemas
                .insert((target.database.clone(), target.schema.clone()));
            session.expanded_groups.insert((
                target.database.clone(),
                target.schema.clone(),
                target.kind,
            ));
        }

        let request_refresh_target = refresh_target
            .clone()
            .or_else(|| session.app.selected_object().cloned());
        let request_id = session.worker.request_refresh(
            request_refresh_target,
            session.app.preview_limit(),
            self.preview_page_offset,
            self.active_data_filter.clone(),
        )?;
        session.pending.refresh_request = Some(PendingRefreshRequest {
            request_id,
            selection_target: refresh_target,
        });
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
        if !session.capabilities.supports_crud_templates {
            return Err(anyhow!(
                "the current connection does not support CRUD templates"
            ));
        }

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

    fn schedule_schema_objects_for_connection_group(
        &mut self,
        connection_index: usize,
        database: String,
        schema: String,
        kind: DbObjectKind,
    ) -> Result<()> {
        let session = self
            .sessions
            .get_mut(connection_index)
            .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
        let key = (database.clone(), schema.clone(), kind);
        if session.loaded_groups.contains(&key)
            || session.pending.group_request_ids.contains_key(&key)
        {
            return Ok(());
        }

        let request_id =
            session
                .worker
                .request_schema_objects(database.clone(), schema.clone(), kind)?;
        session
            .pending
            .group_request_ids
            .insert(key.clone(), request_id);
        session.app.set_status(format!(
            "Loading {} for schema {}.{}...",
            kind.group_label(),
            database,
            schema
        ));
        Ok(())
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
                catalog_summary,
                loaded_group,
                preview_target,
                preview_offset,
                preview,
            } => {
                let mut selected_key = self
                    .entries
                    .get(self.selected_row)
                    .map(|entry| entry.key.clone());
                let mut schedule_follow_up_preview = None;
                {
                    let session = self
                        .sessions
                        .get_mut(session_index)
                        .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
                    let Some(pending_refresh) = session.pending.refresh_request.as_ref() else {
                        return Ok(());
                    };
                    if pending_refresh.request_id != request_id {
                        return Ok(());
                    }

                    let selection_target = pending_refresh.selection_target.clone();
                    if let Some(target) = selection_target.as_ref() {
                        selected_key = Some(TreeNodeKey::Object {
                            connection: session_index,
                            object: target.clone(),
                        });
                    }

                    let current_selected_object = session.app.selected_object().cloned();
                    session.pending.refresh_request = None;
                    match catalog_summary {
                        Ok(catalog_summary) => {
                            session.catalog_summary = catalog_summary.clone();
                            session.loaded_groups.clear();
                            session.app.replace_catalog(
                                catalog_summary.as_catalog_with_unloaded_objects(),
                            );

                            if let Some(loaded_group) = loaded_group {
                                match loaded_group.result {
                                    Ok(objects) => {
                                        session.app.merge_schema_objects_of_kind(
                                            &loaded_group.database,
                                            &loaded_group.schema,
                                            loaded_group.kind,
                                            objects,
                                        )?;
                                        let target_for_selection = selection_target
                                            .as_ref()
                                            .or(current_selected_object.as_ref());
                                        if let Some(target) =
                                            target_for_selection.filter(|target| {
                                                target.database == loaded_group.database
                                                    && target.schema == loaded_group.schema
                                                    && target.kind == loaded_group.kind
                                            })
                                        {
                                            let _ = session.app.select_object_locally(
                                                &target.database,
                                                &target.schema,
                                                &target.name,
                                            );
                                        }
                                        session.loaded_groups.insert((
                                            loaded_group.database,
                                            loaded_group.schema,
                                            loaded_group.kind,
                                        ));
                                    }
                                    Err(error) => {
                                        session.app.set_status(format!(
                                            "Refresh failed to load schema {}: {error}",
                                            loaded_group.kind.group_label()
                                        ));
                                    }
                                }
                            }

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
            SessionEvent::SchemaObjectsLoaded {
                request_id,
                database,
                schema,
                kind,
                result,
            } => {
                let selected_key = self
                    .entries
                    .get(self.selected_row)
                    .map(|entry| entry.key.clone());

                let Some(session) = self.sessions.get_mut(session_index) else {
                    return Ok(());
                };
                let key = (database.clone(), schema.clone(), kind);
                if session.pending.group_request_ids.get(&key) != Some(&request_id) {
                    return Ok(());
                }
                session.pending.group_request_ids.remove(&key);

                match result {
                    Ok(objects) => {
                        session
                            .app
                            .merge_schema_objects_of_kind(&database, &schema, kind, objects)?;
                        session.loaded_groups.insert(key);
                        session.app.set_status(format!(
                            "Loaded {} for schema {}.{}.",
                            kind.group_label(),
                            database,
                            schema
                        ));
                    }
                    Err(error) => {
                        session.app.set_status(format!(
                            "Failed to load {} for schema {}.{}: {error}",
                            kind.group_label(),
                            database,
                            schema
                        ));
                    }
                }

                self.rebuild_rows(selected_key);
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
                        let sql = build_template_sql(
                            self.connection_capabilities(session_index)?,
                            kind,
                            &object,
                            &columns,
                        );
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
                    Ok(columns) => {
                        self.refresh_insert_row_form_field_types(&object, &columns);
                        self.structure.finish_loaded(object, columns);
                    }
                    Err(error) => self.structure.finish_error(object, error),
                }
            }
            SessionEvent::SqlExecuted { request_id, result } => {
                let pending_execute = {
                    let session = self
                        .sessions
                        .get_mut(session_index)
                        .ok_or_else(|| anyhow!("selected connection no longer exists"))?;
                    let Some(pending_execute) =
                        session.pending.execute_requests.remove(&request_id)
                    else {
                        return Ok(());
                    };
                    pending_execute
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
                            if let Some(tab) = editor.find_tab_mut_by_id(pending_execute.tab_id) {
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
                            self.schedule_refresh_for_connection_target(
                                session_index,
                                pending_execute.refresh_target,
                            )?;
                        }
                    }
                    Err(error) => {
                        if let Some(editor) = self.editor.as_mut() {
                            if let Some(tab) = editor.find_tab_mut_by_id(pending_execute.tab_id) {
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
            + usize::from(self.refresh_request.is_some())
            + self.group_request_ids.len()
            + usize::from(self.template_request.is_some())
            + usize::from(self.structure_request.is_some())
            + self.execute_requests.len()
    }

    fn is_busy(&self) -> bool {
        self.count() > 0
    }

    fn clear(&mut self) {
        self.preview_request = None;
        self.refresh_request = None;
        self.group_request_ids.clear();
        self.template_request = None;
        self.structure_request = None;
        self.execute_requests.clear();
    }

    fn request_ids(&self) -> Vec<u64> {
        let mut request_ids = Vec::new();
        if let Some(preview_request) = &self.preview_request {
            request_ids.push(preview_request.request_id);
        }
        if let Some(refresh_request) = &self.refresh_request {
            request_ids.push(refresh_request.request_id);
        }
        request_ids.extend(self.group_request_ids.values().copied());
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

impl SavedSqlState {
    fn view(&self) -> Option<SavedSqlView<'_>> {
        self.open.then_some(SavedSqlView {
            query: &self.query,
            items: &self.visible_items,
            selected_index: self.selected,
        })
    }

    fn replace_entries(&mut self, entries: Vec<SavedSqlEntry>) {
        self.entries = entries;
        self.query.clear();
        self.selected = 0;
        self.open = false;
        self.refresh_matches();
    }

    fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.refresh_matches();
    }

    fn upsert(&mut self, entry: SavedSqlEntry) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|saved| saved.name == entry.name)
        {
            self.entries.remove(index);
        }
        self.entries.push(entry);
        self.sync_sequence = self.sync_sequence.saturating_add(1);
        self.refresh_matches();
    }

    fn remove_by_name(&mut self, name: &str) {
        if let Some(index) = self.entries.iter().position(|entry| entry.name == name) {
            self.entries.remove(index);
            self.sync_sequence = self.sync_sequence.saturating_add(1);
            self.refresh_matches();
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

    fn selected_entry(&self) -> Option<&SavedSqlEntry> {
        self.visible_items.get(self.selected)
    }

    fn refresh_matches(&mut self) {
        let query = self.query.to_ascii_lowercase();
        self.visible_items = self
            .entries
            .iter()
            .rev()
            .filter(|entry| saved_sql_matches_query(entry, query.as_str()))
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
            saved_query_name: None,
            buffer: SqlEditorBuffer::from_sql(&sql),
            status: None,
            result_sets: Vec::new(),
            selected_result: 0,
            pending_execute_request_id: None,
            post_execute_refresh_target: None,
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

fn saved_sql_matches_query(entry: &SavedSqlEntry, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    entry.name.to_ascii_lowercase().contains(query)
        || entry.sql.to_ascii_lowercase().contains(query)
        || entry
            .connection_name
            .as_deref()
            .map(|value| value.to_ascii_lowercase().contains(query))
            .unwrap_or(false)
        || entry
            .database_name
            .as_deref()
            .map(|value| value.to_ascii_lowercase().contains(query))
            .unwrap_or(false)
        || entry
            .schema_name
            .as_deref()
            .map(|value| value.to_ascii_lowercase().contains(query))
            .unwrap_or(false)
}

fn build_rows_for_session(
    connection_index: usize,
    session: &ConnectionSession,
    saved_sql_entries: &[SavedSqlEntry],
) -> Vec<TreeEntry> {
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

    let show_database_nodes = session.catalog_summary.databases.len() > 1;

    for database in &session.catalog_summary.databases {
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
            let collapse_schema = session
                .kind
                .collapses_duplicate_schema(&database.name, &schema.name);
            let group_depth = if collapse_schema {
                database_depth + 1
            } else {
                database_depth + 2
            };
            let object_depth = group_depth + 1;

            if !collapse_schema {
                let schema_expanded = session
                    .expanded_schemas
                    .contains(&(database.name.clone(), schema.name.clone()));
                rows.push(TreeEntry {
                    row: TreeRow::new(
                        schema.name.clone(),
                        database_depth + 1,
                        true,
                        schema_expanded,
                        Some(schema.total_object_count().to_string()),
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
            }

            for kind in DbObjectKind::ordered() {
                let object_count = schema.object_count(kind);
                if !should_show_object_group_nav(session.kind, kind, object_count) {
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
                        group_depth,
                        true,
                        group_expanded,
                        Some(object_count.to_string()),
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

                let group_loaded = session.loaded_groups.contains(&(
                    database.name.clone(),
                    schema.name.clone(),
                    kind,
                ));
                if !group_loaded {
                    continue;
                }

                let objects = session
                    .app
                    .objects_for_schema(&database.name, &schema.name)
                    .unwrap_or(&[])
                    .iter()
                    .filter(|object| object.kind == kind)
                    .cloned()
                    .collect::<Vec<_>>();
                for object in objects {
                    rows.push(TreeEntry {
                        row: TreeRow::new(
                            object.name.clone(),
                            object_depth,
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

            let saved_queries = saved_queries_for_schema(
                session,
                saved_sql_entries,
                &database.name,
                &schema.name,
                database.schemas.len(),
            );
            if should_show_saved_queries_group_nav(saved_queries.len()) {
                let group_key = (database.name.clone(), schema.name.clone());
                let group_expanded = session.expanded_saved_query_groups.contains(&group_key);
                rows.push(TreeEntry {
                    row: TreeRow::new(
                        "Queries",
                        group_depth,
                        true,
                        group_expanded,
                        Some(saved_queries.len().to_string()),
                    ),
                    key: TreeNodeKey::SavedQueryGroup {
                        connection: connection_index,
                        database: database.name.clone(),
                        schema: schema.name.clone(),
                    },
                });

                if group_expanded {
                    for entry in saved_queries {
                        rows.push(TreeEntry {
                            row: TreeRow::new(
                                entry.name.clone(),
                                object_depth,
                                false,
                                false,
                                Some("SQL".to_string()),
                            ),
                            key: TreeNodeKey::SavedQuery {
                                connection: connection_index,
                                database: database.name.clone(),
                                schema: schema.name.clone(),
                                name: entry.name.clone(),
                            },
                        });
                    }
                }
            }
        }
    }

    rows
}

fn should_show_object_group_nav(
    database_kind: DatabaseKind,
    kind: DbObjectKind,
    object_count: usize,
) -> bool {
    if object_count > 0 {
        return true;
    }

    match kind {
        DbObjectKind::View => true,
        DbObjectKind::MaterializedView | DbObjectKind::Function => {
            database_kind == DatabaseKind::Postgres
        }
        DbObjectKind::Table | DbObjectKind::ForeignTable => false,
    }
}

fn should_show_saved_queries_group_nav(_saved_query_count: usize) -> bool {
    true
}

fn saved_queries_for_schema<'a>(
    session: &ConnectionSession,
    entries: &'a [SavedSqlEntry],
    database_name: &str,
    schema_name: &str,
    schema_count: usize,
) -> Vec<&'a SavedSqlEntry> {
    let mut visible = entries
        .iter()
        .filter(|entry| {
            entry
                .connection_name
                .as_deref()
                .is_none_or(|value| value == session.name)
                && entry
                    .database_name
                    .as_deref()
                    .is_none_or(|value| value == database_name)
                && match entry.schema_name.as_deref() {
                    Some(value) => value == schema_name,
                    None => schema_count <= 1,
                }
        })
        .collect::<Vec<_>>();
    visible.sort_by_cached_key(|entry| entry.name.to_ascii_lowercase());
    visible
}

fn first_object_group_with_objects(
    summary: &CatalogSummary,
) -> Option<(String, String, DbObjectKind)> {
    summary.databases.iter().find_map(|database| {
        database.schemas.iter().find_map(|schema| {
            DbObjectKind::ordered().into_iter().find_map(|kind| {
                (schema.object_count(kind) > 0)
                    .then(|| (database.name.clone(), schema.name.clone(), kind))
            })
        })
    })
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

fn preview_unavailable_message(object: &DbObjectRef) -> String {
    format!(
        "Data preview is not available for {} {}. Open SQL to work with it.",
        object.kind.label(),
        object.qualified_name()
    )
}

fn delete_operation_kind(sql: &str) -> Option<DeleteOperationKind> {
    write_operation_kind(sql).and_then(WriteOperationKind::delete_confirmation_kind)
}

fn write_operation_kind(sql: &str) -> Option<WriteOperationKind> {
    let tokens = sql_keyword_tokens(sql);
    if tokens.is_empty() {
        return None;
    }

    if tokens.first().is_some_and(|token| token == "EXPLAIN") {
        if tokens.iter().any(|token| token == "ANALYZE") {
            return write_keyword_in(&tokens[1..]);
        }
        return None;
    }

    write_keyword_in(&tokens)
}

fn write_keyword_in(tokens: &[String]) -> Option<WriteOperationKind> {
    if tokens.iter().any(|token| token == "INSERT") {
        return Some(WriteOperationKind::Insert);
    }
    if tokens.iter().any(|token| token == "UPDATE") {
        return Some(WriteOperationKind::Update);
    }
    if tokens.iter().any(|token| token == "DELETE") {
        return Some(WriteOperationKind::Delete);
    }
    if tokens.iter().any(|token| token == "DROP") {
        return Some(WriteOperationKind::Drop);
    }
    if tokens.iter().any(|token| token == "TRUNCATE") {
        return Some(WriteOperationKind::Truncate);
    }
    if tokens.iter().any(|token| token == "ALTER") {
        return Some(WriteOperationKind::Alter);
    }
    if tokens.iter().any(|token| token == "CREATE") {
        return Some(WriteOperationKind::Create);
    }
    if tokens.iter().any(|token| token == "REPLACE") {
        return Some(WriteOperationKind::Replace);
    }
    if tokens.iter().any(|token| token == "MERGE") {
        return Some(WriteOperationKind::Merge);
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

const STRUCTURE_EDITOR_EXISTING_DEFAULT_SENTINEL: &str = "__relora_existing_default__";

fn create_table_active_text_target(form: &mut CreateTableFormState) -> Option<&mut String> {
    if form.selected_row == 0 || form.selected_focus == CreateTableFieldFocus::TableName {
        return Some(&mut form.table_name);
    }

    match form.selected_focus {
        CreateTableFieldFocus::ColumnName => form
            .columns
            .get_mut(form.selected_row.checked_sub(1)?)
            .map(|column| &mut column.name),
        CreateTableFieldFocus::DefaultValue => form
            .columns
            .get_mut(form.selected_row.checked_sub(1)?)
            .map(|column| &mut column.default_value),
        CreateTableFieldFocus::ColumnType
        | CreateTableFieldFocus::Nullable
        | CreateTableFieldFocus::Unique
        | CreateTableFieldFocus::AutoIncrement
        | CreateTableFieldFocus::PrimaryKey => None,
        CreateTableFieldFocus::TableName => Some(&mut form.table_name),
    }
}

fn structure_editor_active_text_target(form: &mut StructureEditorFormState) -> Option<&mut String> {
    if form.selected_row == 0 || form.selected_focus == StructureEditorFieldFocus::TableName {
        return Some(&mut form.table_name);
    }

    match form.selected_focus {
        StructureEditorFieldFocus::ColumnName => form
            .columns
            .get_mut(form.selected_row.checked_sub(1)?)
            .map(|column| &mut column.name),
        StructureEditorFieldFocus::DefaultValue => form
            .columns
            .get_mut(form.selected_row.checked_sub(1)?)
            .map(|column| &mut column.default_value),
        StructureEditorFieldFocus::ColumnType
        | StructureEditorFieldFocus::Nullable
        | StructureEditorFieldFocus::Unique
        | StructureEditorFieldFocus::PrimaryKey => None,
        StructureEditorFieldFocus::TableName => Some(&mut form.table_name),
    }
}

fn structure_editor_default_as_option(default_value: &str) -> Option<&str> {
    let value = default_value.trim();
    if value.is_empty() {
        None
    } else if value == STRUCTURE_EDITOR_EXISTING_DEFAULT_SENTINEL {
        Some(STRUCTURE_EDITOR_EXISTING_DEFAULT_SENTINEL)
    } else {
        Some(value)
    }
}

fn structure_editor_default_display_value(default_value: &str) -> Option<String> {
    let value = default_value.trim();
    if value.is_empty() {
        None
    } else if value == STRUCTURE_EDITOR_EXISTING_DEFAULT_SENTINEL {
        Some("<existing default>".to_string())
    } else {
        Some(default_value.to_string())
    }
}

fn create_table_type_options(kind: DatabaseKind) -> &'static [CreateTableTypeOption] {
    match kind {
        DatabaseKind::Postgres => &[
            CreateTableTypeOption {
                label: "integer",
                sql: "integer",
            },
            CreateTableTypeOption {
                label: "bigint",
                sql: "bigint",
            },
            CreateTableTypeOption {
                label: "text",
                sql: "text",
            },
            CreateTableTypeOption {
                label: "boolean",
                sql: "boolean",
            },
            CreateTableTypeOption {
                label: "numeric",
                sql: "numeric",
            },
            CreateTableTypeOption {
                label: "date",
                sql: "date",
            },
            CreateTableTypeOption {
                label: "timestamp",
                sql: "timestamp",
            },
            CreateTableTypeOption {
                label: "timestamptz",
                sql: "timestamptz",
            },
            CreateTableTypeOption {
                label: "jsonb",
                sql: "jsonb",
            },
            CreateTableTypeOption {
                label: "uuid",
                sql: "uuid",
            },
        ],
        DatabaseKind::MySql => &[
            CreateTableTypeOption {
                label: "int",
                sql: "int",
            },
            CreateTableTypeOption {
                label: "bigint",
                sql: "bigint",
            },
            CreateTableTypeOption {
                label: "varchar(255)",
                sql: "varchar(255)",
            },
            CreateTableTypeOption {
                label: "text",
                sql: "text",
            },
            CreateTableTypeOption {
                label: "boolean",
                sql: "boolean",
            },
            CreateTableTypeOption {
                label: "decimal(10,2)",
                sql: "decimal(10,2)",
            },
            CreateTableTypeOption {
                label: "date",
                sql: "date",
            },
            CreateTableTypeOption {
                label: "datetime",
                sql: "datetime",
            },
            CreateTableTypeOption {
                label: "timestamp",
                sql: "timestamp",
            },
            CreateTableTypeOption {
                label: "json",
                sql: "json",
            },
        ],
        DatabaseKind::Sqlite => &[
            CreateTableTypeOption {
                label: "INTEGER",
                sql: "INTEGER",
            },
            CreateTableTypeOption {
                label: "TEXT",
                sql: "TEXT",
            },
            CreateTableTypeOption {
                label: "REAL",
                sql: "REAL",
            },
            CreateTableTypeOption {
                label: "NUMERIC",
                sql: "NUMERIC",
            },
            CreateTableTypeOption {
                label: "BLOB",
                sql: "BLOB",
            },
            CreateTableTypeOption {
                label: "DATE",
                sql: "DATE",
            },
            CreateTableTypeOption {
                label: "DATETIME",
                sql: "DATETIME",
            },
        ],
    }
}

fn create_table_type_index(kind: DatabaseKind, label: &str) -> usize {
    create_table_type_options(kind)
        .iter()
        .position(|option| option.label.eq_ignore_ascii_case(label))
        .unwrap_or(0)
}

fn default_create_table_primary_type(kind: DatabaseKind) -> CreateTableTypeOption {
    let label = match kind {
        DatabaseKind::Postgres => "integer",
        DatabaseKind::MySql => "int",
        DatabaseKind::Sqlite => "INTEGER",
    };

    create_table_type_options(kind)
        .get(create_table_type_index(kind, label))
        .copied()
        .unwrap_or(create_table_type_options(kind)[0])
}

fn create_table_auto_increment_type_index(kind: DatabaseKind) -> usize {
    let label = match kind {
        DatabaseKind::Postgres => "integer",
        DatabaseKind::MySql => "int",
        DatabaseKind::Sqlite => "INTEGER",
    };
    create_table_type_index(kind, label)
}

fn default_index_name(table_name: &str, column_name: &str) -> String {
    format!("{table_name}_{column_name}_idx")
}

fn default_create_table_primary_column(kind: DatabaseKind) -> CreateTableColumnState {
    CreateTableColumnState {
        name: "id".to_string(),
        type_index: create_table_type_index(kind, default_create_table_primary_type(kind).label),
        default_value: String::new(),
        nullable: false,
        unique: false,
        auto_increment: false,
        primary_key: true,
    }
}

fn default_create_table_regular_column(
    kind: DatabaseKind,
    ordinal: usize,
) -> CreateTableColumnState {
    let label = match kind {
        DatabaseKind::Postgres => "text",
        DatabaseKind::MySql => "varchar(255)",
        DatabaseKind::Sqlite => "TEXT",
    };

    CreateTableColumnState {
        name: format!("column_{ordinal}"),
        type_index: create_table_type_index(kind, label),
        default_value: String::new(),
        nullable: true,
        unique: false,
        auto_increment: false,
        primary_key: false,
    }
}

fn default_structure_editor_regular_column(
    kind: DatabaseKind,
    ordinal: usize,
) -> StructureEditorColumnState {
    let default_column = default_create_table_regular_column(kind, ordinal);
    let data_type = create_table_type_options(kind)
        .get(default_column.type_index)
        .map(|option| option.label.to_string())
        .unwrap_or_else(|| create_table_type_options(kind)[0].label.to_string());

    StructureEditorColumnState {
        original_name: None,
        original_data_type: None,
        original_nullable: None,
        original_default: None,
        original_unique: None,
        original_primary_key: None,
        name: String::new(),
        data_type,
        default_value: default_column.default_value,
        nullable: default_column.nullable,
        unique: false,
        primary_key: false,
    }
}

fn structure_editor_column_identity(column: &StructureEditorColumnState) -> Option<String> {
    if let Some(original_name) = column.original_name.as_deref() {
        Some(format!("existing:{original_name}"))
    } else {
        let name = column.name.trim();
        (!name.is_empty()).then_some(format!("new:{name}"))
    }
}

fn classify_insert_row_field_kind(data_type: &str) -> InsertRowFieldKind {
    let normalized = data_type.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return InsertRowFieldKind::Text;
    }
    if normalized.contains("bool") || normalized == "tinyint(1)" {
        return InsertRowFieldKind::Boolean;
    }
    if normalized.contains("date") && !normalized.contains("time") {
        return InsertRowFieldKind::Date;
    }
    if normalized.contains("timestamp")
        || normalized.contains("datetime")
        || normalized.contains("timestamptz")
    {
        return InsertRowFieldKind::DateTime;
    }
    if normalized.contains("json") {
        return InsertRowFieldKind::Json;
    }
    if [
        "int", "serial", "numeric", "decimal", "real", "double", "float",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        return InsertRowFieldKind::Number;
    }
    InsertRowFieldKind::Text
}

fn sync_insert_row_form_field(field: &mut InsertRowFormFieldState) {
    if !field.kind.supports_date_picker() {
        return;
    }

    let trimmed = field.value.trim();
    if trimmed.is_empty() {
        field
            .date_value
            .get_or_insert_with(InsertRowDateValue::today);
        if field.kind.supports_time_picker() {
            field
                .time_value
                .get_or_insert_with(InsertRowTimeValue::midnight);
        }
        return;
    }

    if let Some(date) = parse_insert_row_form_date_value(field.kind, trimmed) {
        field.date_value = Some(date);
    }
    if field.kind.supports_time_picker() {
        if let Some(time) = parse_insert_row_form_time_value(field.kind, trimmed) {
            field.time_value = Some(time);
        }
    }
}

fn parse_insert_row_form_date_value(
    kind: InsertRowFieldKind,
    value: &str,
) -> Option<InsertRowDateValue> {
    match kind {
        InsertRowFieldKind::Date => InsertRowDateValue::parse(value),
        InsertRowFieldKind::DateTime => value.get(..10).and_then(InsertRowDateValue::parse),
        _ => None,
    }
}

fn parse_insert_row_form_time_value(
    kind: InsertRowFieldKind,
    value: &str,
) -> Option<InsertRowTimeValue> {
    match kind {
        InsertRowFieldKind::DateTime => split_insert_row_form_datetime_time(value)
            .and_then(|(_, rest)| parse_insert_row_form_time_prefix(rest).map(|(time, _)| time)),
        _ => None,
    }
}

fn render_insert_row_form_field_temporal_value(
    kind: InsertRowFieldKind,
    current_value: &str,
    date: InsertRowDateValue,
    time: Option<InsertRowTimeValue>,
) -> String {
    match kind {
        InsertRowFieldKind::Date => date.to_iso_string(),
        InsertRowFieldKind::DateTime => {
            let (separator, trailing) = insert_row_form_datetime_layout(current_value);
            let time = time
                .or_else(|| parse_insert_row_form_time_value(kind, current_value))
                .unwrap_or_else(InsertRowTimeValue::midnight);
            format!(
                "{}{}{}{}",
                date.to_iso_string(),
                separator,
                time.to_hms_string(),
                trailing
            )
        }
        _ => current_value.to_string(),
    }
}

fn split_insert_row_form_datetime_time(value: &str) -> Option<(char, &str)> {
    let rest = value.get(10..)?;
    if let Some(rest) = rest.strip_prefix('T') {
        Some(('T', rest))
    } else if let Some(rest) = rest.strip_prefix(' ') {
        Some((' ', rest))
    } else {
        Some((' ', rest))
    }
}

fn insert_row_form_datetime_layout(current_value: &str) -> (char, String) {
    let Some((separator, rest)) = split_insert_row_form_datetime_time(current_value) else {
        return (' ', String::new());
    };
    let trailing = parse_insert_row_form_time_prefix(rest)
        .map(|(_, consumed)| rest[consumed..].to_string())
        .unwrap_or_default();
    (separator, trailing)
}

fn parse_insert_row_form_time_prefix(value: &str) -> Option<(InsertRowTimeValue, usize)> {
    let bytes = value.as_bytes();
    if bytes.len() < 5 {
        return None;
    }
    if !bytes[0].is_ascii_digit()
        || !bytes[1].is_ascii_digit()
        || bytes[2] != b':'
        || !bytes[3].is_ascii_digit()
        || !bytes[4].is_ascii_digit()
    {
        return None;
    }

    let hour = value.get(0..2)?.parse::<u8>().ok()?;
    let minute = value.get(3..5)?.parse::<u8>().ok()?;
    let mut second = 0;
    let mut consumed = 5;
    if bytes.get(5) == Some(&b':') {
        if bytes.len() < 8 || !bytes[6].is_ascii_digit() || !bytes[7].is_ascii_digit() {
            return None;
        }
        second = value.get(6..8)?.parse::<u8>().ok()?;
        consumed = 8;
    }

    InsertRowTimeValue::new(hour, minute, second).map(|time| (time, consumed))
}

impl InsertRowDateValue {
    fn today() -> Self {
        let unix_days = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() / 86_400)
            .unwrap_or_default();
        Self::from_unix_days(unix_days as i64)
    }

    fn parse(value: &str) -> Option<Self> {
        let mut parts = value.split('-');
        let year = parts.next()?.parse::<i32>().ok()?;
        let month = parts.next()?.parse::<u8>().ok()?;
        let day = parts.next()?.parse::<u8>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Self::new(year, month, day)
    }

    fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if !(1..=12).contains(&month) {
            return None;
        }
        let max_day = days_in_month(year, month);
        if !(1..=max_day).contains(&day) {
            return None;
        }
        Some(Self { year, month, day })
    }

    fn to_iso_string(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    fn month_label(self) -> String {
        const MONTHS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let month_name = MONTHS[usize::from(self.month.saturating_sub(1))];
        format!("{month_name} {}", self.year)
    }

    fn first_weekday(self) -> usize {
        let first = Self {
            year: self.year,
            month: self.month,
            day: 1,
        };
        let unix_days = first.to_unix_days();
        ((unix_days + 3).rem_euclid(7)) as usize
    }

    fn add_days(self, delta: i32) -> Self {
        Self::from_unix_days(self.to_unix_days() + i64::from(delta))
    }

    fn add_months(self, delta: i32) -> Self {
        let month_index = self.year * 12 + i32::from(self.month) - 1 + delta;
        let year = month_index.div_euclid(12);
        let month = (month_index.rem_euclid(12) + 1) as u8;
        let day = self.day.min(days_in_month(year, month));
        Self { year, month, day }
    }

    fn add_years(self, delta: i32) -> Self {
        let year = self.year + delta;
        let day = self.day.min(days_in_month(year, self.month));
        Self {
            year,
            month: self.month,
            day,
        }
    }

    fn to_unix_days(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    fn from_unix_days(days: i64) -> Self {
        let (year, month, day) = civil_from_days(days);
        Self { year, month, day }
    }

    fn snapshot(
        self,
        selected_value: String,
        time_value: Option<String>,
        active_segment: Option<InsertRowDateTimeSegmentView>,
    ) -> InsertRowDatePickerSnapshot {
        InsertRowDatePickerSnapshot {
            month_label: self.month_label(),
            selected_value,
            time_value,
            active_segment,
            first_weekday: self.first_weekday(),
            day_count: days_in_month(self.year, self.month),
            selected_day: self.day,
        }
    }
}

impl InsertRowTimeValue {
    fn midnight() -> Self {
        Self {
            hour: 0,
            minute: 0,
            second: 0,
        }
    }

    fn new(hour: u8, minute: u8, second: u8) -> Option<Self> {
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        Some(Self {
            hour,
            minute,
            second,
        })
    }

    fn to_hms_string(self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }

    fn from_seconds_of_day(seconds: u32) -> Self {
        let hour = (seconds / 3_600) as u8;
        let minute = ((seconds % 3_600) / 60) as u8;
        let second = (seconds % 60) as u8;
        Self {
            hour,
            minute,
            second,
        }
    }

    fn add_hours(self, delta: i32) -> Self {
        self.add_seconds(delta.saturating_mul(3_600))
    }

    fn add_minutes(self, delta: i32) -> Self {
        self.add_seconds(delta.saturating_mul(60))
    }

    fn add_seconds(self, delta: i32) -> Self {
        let total_seconds =
            i32::from(self.hour) * 3_600 + i32::from(self.minute) * 60 + i32::from(self.second);
        let updated = (total_seconds + delta).rem_euclid(86_400);
        let hour = (updated / 3_600) as u8;
        let minute = ((updated % 3_600) / 60) as u8;
        let second = (updated % 60) as u8;
        Self {
            hour,
            minute,
            second,
        }
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    }
}

fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = i64::from(year) - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u8, day as u8)
}

fn build_template_sql(
    capabilities: DriverCapabilities,
    kind: TemplateKind,
    object: &DbObjectRef,
    columns: &[DbColumn],
) -> String {
    match kind {
        TemplateKind::Insert => insert_template(capabilities, object, columns),
        TemplateKind::Update => update_template(capabilities, object, columns),
        TemplateKind::Delete => delete_template(capabilities, object, columns),
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
                    match (column.is_primary_key, column.is_unique) {
                        (true, true) => "PK UNIQUE".to_string(),
                        (true, false) => "PK".to_string(),
                        (false, true) => "UNIQUE".to_string(),
                        (false, false) => String::new(),
                    },
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

    #[test]
    fn write_operation_detector_covers_mutating_sql() {
        assert_eq!(
            write_operation_kind("insert into users(id) values (1)"),
            Some(WriteOperationKind::Insert)
        );
        assert_eq!(
            write_operation_kind("update users set email = 'alice@example.com'"),
            Some(WriteOperationKind::Update)
        );
        assert_eq!(
            write_operation_kind("create table audit_log(id integer)"),
            Some(WriteOperationKind::Create)
        );
        assert_eq!(
            write_operation_kind("alter table users add column last_seen timestamptz"),
            Some(WriteOperationKind::Alter)
        );
        assert_eq!(
            write_operation_kind("replace into users(id) values (1)"),
            Some(WriteOperationKind::Replace)
        );
        assert_eq!(
            write_operation_kind(
                "merge into users using staging_users on users.id = staging_users.id"
            ),
            Some(WriteOperationKind::Merge)
        );
        assert_eq!(
            write_operation_kind("explain analyze update users set email = 'alice@example.com'"),
            Some(WriteOperationKind::Update)
        );
    }

    #[test]
    fn write_operation_detector_ignores_read_only_safe_mentions() {
        assert_eq!(
            write_operation_kind("select 'update users set email = 1'"),
            None
        );
        assert_eq!(
            write_operation_kind("/* create table users */\nselect 1"),
            None
        );
        assert_eq!(
            write_operation_kind("explain update users set email = 'alice@example.com'"),
            None
        );
    }
}
