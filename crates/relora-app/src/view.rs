use crate::completion::CompletionItem;
use relora_core::db::{DatabaseKind, DbColumn, DbObjectKind, DbObjectRef, TablePreview};

use crate::tree::TreeRow;

#[derive(Debug, Clone, Copy)]
pub struct EditorView<'a> {
    pub title: &'a str,
    pub tab_strip: &'a str,
    pub tab_count: usize,
    pub selected_tab_index: usize,
    pub lines: &'a [String],
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub result_strip: Option<&'a str>,
    pub result_set_count: usize,
    pub selected_result_index: usize,
    pub status: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct EditorCompletionView<'a> {
    pub items: &'a [CompletionItem],
    pub selected_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPaletteItemView {
    pub title: &'static str,
    pub hint: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandPaletteView<'a> {
    pub query: &'a str,
    pub items: &'a [CommandPaletteItemView],
    pub selected_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SqlHistoryView<'a> {
    pub query: &'a str,
    pub items: &'a [String],
    pub selected_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct DataFilterView<'a> {
    pub input: &'a str,
    pub active_filter: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct CellEditView<'a> {
    pub column: &'a str,
    pub input: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowInspectorPane {
    Fields,
    Preview,
}

#[derive(Debug, Clone, Copy)]
pub struct RowInspectorView<'a> {
    pub row_index: usize,
    pub selected_field: usize,
    pub detail_scroll: usize,
    pub formatted: bool,
    pub active_pane: RowInspectorPane,
    pub columns: &'a [String],
    pub values: &'a [String],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPaneTab {
    Data,
    Sql,
    Structure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RightPaneTabView {
    pub kind: RightPaneTab,
    pub title: &'static str,
    pub active: bool,
    pub available: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct StructureView<'a> {
    pub object: Option<&'a DbObjectRef>,
    pub columns: &'a [DbColumn],
    pub loading: bool,
    pub status: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct StagedCrudView<'a> {
    pub preview_sql: &'a str,
    pub commit_sql: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct DeleteConfirmationView<'a> {
    pub title: &'a str,
    pub message: &'a str,
    pub sql_preview: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct WorkspaceView<'a> {
    pub tree_rows: &'a [TreeRow],
    pub selected_row_index: usize,
    pub connection_count: usize,
    pub pending_task_count: usize,
    pub selected_connection_name: Option<&'a str>,
    pub selected_connection_label: Option<&'a str>,
    pub selected_database_name: Option<&'a str>,
    pub selected_connection_kind: Option<DatabaseKind>,
    pub selected_connection_busy: bool,
    pub selected_schema_name: Option<&'a str>,
    pub selected_group_kind: Option<DbObjectKind>,
    pub selected_object: Option<&'a DbObjectRef>,
    pub active_grid: &'a TablePreview,
    pub preview_grid: &'a TablePreview,
    pub active_right_tab: RightPaneTab,
    pub right_tabs: [RightPaneTabView; 3],
    pub grid_selected_row_index: usize,
    pub grid_selected_column_index: usize,
    pub grid_scroll_offset: usize,
    pub grid_column_offset: usize,
    pub preview_loading: bool,
    pub assets_focused: bool,
    pub sql_editor_focused: bool,
    pub data_grid_focused: bool,
    pub command_palette: Option<CommandPaletteView<'a>>,
    pub sql_history: Option<SqlHistoryView<'a>>,
    pub data_filter: Option<DataFilterView<'a>>,
    pub cell_edit: Option<CellEditView<'a>>,
    pub row_inspector: Option<RowInspectorView<'a>>,
    pub help_overlay_visible: bool,
    pub editor: Option<EditorView<'a>>,
    pub editor_completion: Option<EditorCompletionView<'a>>,
    pub structure: Option<StructureView<'a>>,
    pub staged_crud: Option<StagedCrudView<'a>>,
    pub delete_confirmation: Option<DeleteConfirmationView<'a>>,
    pub status: Option<&'a str>,
    pub selected_connection_database_count: usize,
    pub selected_connection_schema_count: usize,
    pub selected_connection_object_count: usize,
    pub selected_schema_table_count: usize,
    pub selected_schema_view_count: usize,
    pub selected_schema_foreign_table_count: usize,
}
