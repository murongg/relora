pub(super) const KEY_INTERRUPT: char = 'c';
pub(super) const KEY_COMMAND_PALETTE: char = 'p';
pub(super) const KEY_SQL_HISTORY: char = 'r';
pub(super) const KEY_SAVED_SQL: char = 'o';
pub(super) const KEY_HELP: char = '?';
pub(super) const KEY_HELP_FULLWIDTH: char = '？';
pub(super) const KEY_CONFIRM_YES_LOWER: char = 'y';
pub(super) const KEY_CONFIRM_YES_UPPER: char = 'Y';
pub(super) const KEY_CONFIRM_NO_LOWER: char = 'n';
pub(super) const KEY_CONFIRM_NO_UPPER: char = 'N';

pub(super) const KEY_LAUNCHER_DOWN: char = 'j';
pub(super) const KEY_LAUNCHER_UP: char = 'k';
pub(super) const KEY_LAUNCHER_MULTI_SELECT: char = ' ';
pub(super) const KEY_LAUNCHER_NEW_CONNECTION: char = 'a';
pub(super) const KEY_LAUNCHER_EDIT_CONNECTION: char = 'e';
pub(super) const KEY_LAUNCHER_DELETE_CONNECTION: char = 'd';
pub(super) const KEY_LAUNCHER_QUIT: char = 'q';
pub(super) const KEY_LAUNCHER_TEST_CONNECTION: char = 't';
pub(super) const KEY_FORM_OPEN_FILE: char = 'o';

pub(super) const KEY_DATA_GRID_COPY_ROW: char = 'y';
pub(super) const KEY_DATA_GRID_COPY_CELL: char = 'Y';
pub(super) const KEY_DATA_GRID_COPY_WHERE: char = 'w';
pub(super) const KEY_DATA_GRID_EDIT_CELL: char = 'i';
pub(super) const KEY_DATA_GRID_INSERT_ROW: char = 'a';
pub(super) const KEY_DATA_GRID_DELETE_ROW: char = 'D';
pub(super) const KEY_DATA_GRID_NEXT_PAGE: char = 'n';
pub(super) const KEY_DATA_GRID_PREVIOUS_PAGE: char = 'p';
pub(super) const KEY_DATA_GRID_SHRINK_COLUMN: char = '[';
pub(super) const KEY_DATA_GRID_EXPAND_COLUMN: char = ']';
pub(super) const KEY_DATA_GRID_RESET_COLUMN: char = '=';
pub(super) const KEY_DATA_GRID_FREEZE_COLUMNS: char = 'f';
pub(super) const KEY_DATA_GRID_CLEAR_FROZEN: char = 'F';

pub(super) const KEY_BROWSER_QUIT: char = 'q';
pub(super) const KEY_BROWSER_COMMAND_PALETTE: char = ':';
pub(super) const KEY_BROWSER_FILTER: char = '/';
pub(super) const KEY_BROWSER_REFRESH: char = 'r';
pub(super) const KEY_BROWSER_CANCEL_TASKS: char = 'c';
pub(super) const KEY_BROWSER_OPEN_SQL: char = 'e';
pub(super) const KEY_BROWSER_TEMPLATE_SELECT: char = 's';
pub(super) const KEY_BROWSER_TEMPLATE_INSERT: char = 'i';
pub(super) const KEY_BROWSER_TEMPLATE_UPDATE: char = 'u';
pub(super) const KEY_BROWSER_TEMPLATE_DELETE: char = 'd';
pub(super) const KEY_BROWSER_DOWN: char = 'j';
pub(super) const KEY_BROWSER_UP: char = 'k';
pub(super) const KEY_BROWSER_EXPAND_RIGHT: char = 'l';
pub(super) const KEY_BROWSER_EXPAND_LEFT: char = 'h';
pub(super) const KEY_BROWSER_TOGGLE_NODE: char = ' ';

pub(super) const KEY_ROW_INSPECTOR_SCROLL_DOWN: char = 'd';
pub(super) const KEY_ROW_INSPECTOR_SCROLL_UP: char = 'u';
pub(super) const KEY_ROW_INSPECTOR_QUIT: char = 'q';
pub(super) const KEY_ROW_INSPECTOR_DOWN: char = 'j';
pub(super) const KEY_ROW_INSPECTOR_UP: char = 'k';
pub(super) const KEY_ROW_INSPECTOR_COPY: char = 'y';
pub(super) const KEY_ROW_INSPECTOR_COPY_UPPER: char = 'Y';
pub(super) const KEY_ROW_INSPECTOR_EDIT: char = 'i';
pub(super) const KEY_ROW_INSPECTOR_FORMAT: char = 'f';

pub(super) const KEY_EDITOR_NEW_TAB: char = 't';
pub(super) const KEY_EDITOR_CLOSE_TAB: char = 'w';
pub(super) const KEY_EDITOR_CANCEL_TASKS: char = 'k';
pub(super) const KEY_EDITOR_COMMIT_STAGED: char = 'g';
pub(super) const KEY_EDITOR_SAVE_SQL: char = 's';
pub(super) const KEY_EDITOR_DELETE_SAVED_SQL: char = 'd';

pub(super) const KEY_ALT_TAB_DATA: char = '1';
pub(super) const KEY_ALT_TAB_SQL: char = '2';
pub(super) const KEY_ALT_TAB_STRUCTURE: char = '3';

pub(super) const FKEY_TAB_DATA: u8 = 2;
pub(super) const FKEY_TAB_SQL: u8 = 3;
pub(super) const FKEY_TAB_STRUCTURE: u8 = 4;
pub(super) const FKEY_HELP: u8 = 1;
pub(super) const FKEY_EDITOR_EXECUTE: u8 = 5;
pub(super) const FKEY_EDITOR_PREVIOUS_TAB: u8 = 6;
pub(super) const FKEY_EDITOR_NEXT_TAB: u8 = 7;
pub(super) const FKEY_EDITOR_PREVIOUS_RESULT: u8 = 8;
pub(super) const FKEY_EDITOR_NEXT_RESULT: u8 = 9;
pub(super) const FKEY_SQL_HISTORY: u8 = 10;
pub(super) const FKEY_EDITOR_EXPLAIN: u8 = 11;
pub(super) const FKEY_EDITOR_EXPLAIN_ANALYZE: u8 = 12;

pub(super) const RIGHT_TAB_SHORTCUT_HELP: &str = "F2/F3/F4 or Alt-1/2/3";
pub(super) const LAUNCHER_HELP_FORM: &str = "Connection Form";
pub(super) const LAUNCHER_HELP_IDLE: &str =
    "Launch selected: Enter   New connection: a   Edit: e   Delete: d   Multi-select: Space";
pub(super) const LAUNCHER_FOOTER_FORM: &str =
    "Type details, Tab switches field, Enter saves, Esc cancels.";
pub(super) const LAUNCHER_FOOTER_IDLE: &str =
    "j/k select profiles, Space queues multiple launches, q exits Relora.";
pub(super) const FORM_SAVE_HELP: &str = "Driver/Mode: t tests, p/m/s or Left/Right select driver, r/o or Left/Right toggle mode. Ctrl-T tests anywhere.";
pub(super) const FORM_SAVE_HELP_SQLITE: &str = "SQLite: Ctrl-O browses files, :memory: stays in RAM. Driver/Mode still use Left/Right; Ctrl-T tests anywhere.";
pub(super) const DRIVER_MISSING_HELP: &str = "Press Esc or Enter to close.";
pub(super) const DELETE_CONNECTION_HELP: &str = "Press y to delete, n or Esc to cancel.";
pub(super) const SQLITE_FILE_PICKER_HELP: &str =
    "SQLite files: Enter open/select, Up/Down move, Left go up, Esc close";

pub(super) const FOOTER_COMMAND_HELP: &str =
    "Command: type to filter, Up/Down select, Enter run, Esc close";
pub(super) const FOOTER_KEYBOARD_HELP: &str =
    "Keyboard help: Esc/Enter/?/F1 close, then continue working in the same pane";
pub(super) const FOOTER_SQL_HISTORY_HELP: &str =
    "SQL history: type search, Up/Down select, Enter rerun, Esc close";
pub(super) const FOOTER_SAVED_SQL_HELP: &str =
    "Saved SQL: type search, Up/Down select, Enter open, Esc close";
pub(super) const FOOTER_DATA_FILTER_HELP: &str =
    "Data filter: type quick filter, Enter apply, Esc close";
pub(super) const FOOTER_SAVE_SQL_HELP: &str = "Save SQL: type a name, Enter save, Esc close";
pub(super) const FOOTER_INSERT_ROW_FORM_HELP: &str = "Insert row: Tab/j/k switch field, type value, date arrows/PgUp/Home adjust, datetime Left/Right segment Up/Down adjust, Ctrl-U clear, Enter preview SQL";
pub(super) const FOOTER_CELL_EDIT_HELP: &str =
    "Cell edit: type new value, Enter preview staged SQL, Esc close";
pub(super) const FOOTER_ROW_INSPECTOR_HELP: &str = "Cell details: Tab switch box, j/k move or scroll, PgUp/PgDn or Ctrl-U/Ctrl-D scroll, y copy, i edit, f raw/formatted";
pub(super) const FOOTER_COMPLETION_HELP: &str =
    "Completion: Enter/Tab accept, Up/Down select, Esc close";
pub(super) const FOOTER_SQL_RESULTS_HELP: &str =
    "SQL results: j/k rows, h/l columns, [/] resize, = auto, f freeze, F clear, Tab cycle";
pub(super) const FOOTER_SQL_ASSETS_HELP: &str =
    "SQL assets: j/k browse, Enter open/toggle, Tab/Shift-Tab cycle, F2 Data, F4 Structure";
pub(super) const FOOTER_SQL_EDITOR_HELP: &str = "SQL editor: Tab/Shift-Tab cycle, Ctrl-Enter run, Ctrl-S save, Ctrl-D delete saved, Ctrl-O saved";
pub(super) const FOOTER_SQL_TAB_HELP: &str =
    "SQL tab: e open editor, Ctrl-O saved, Ctrl-D delete saved, F10 history, F2 Data, F4 Structure";
pub(super) const FOOTER_STRUCTURE_GRID_HELP: &str =
    "Structure tab: j/k fields, [/] resize, = auto, f freeze, F clear, Enter inspect";
pub(super) const FOOTER_STRUCTURE_HELP: &str =
    "Structure tab: Tab fields, F2 Data, F3 SQL, r refresh, Ctrl-P command";
pub(super) const FOOTER_DATA_GRID_HELP: &str =
    "Data tab: / filter, a insert, D delete, i stage edit, y row, Y cell, w WHERE, n/p page";
pub(super) const FOOTER_DATA_HELP: &str =
    "Data tab: / filter, n/p page, F3 SQL, F4 Structure, j/k assets, Tab grid, e SQL, Ctrl-O/F10";

pub(super) const HELP_GLOBAL_SHORTCUTS: [(&str, &str); 5] = [
    ("F1 / ?", "Open or close help"),
    ("Ctrl-P", "Open command palette"),
    ("F2 / Alt-1", "Focus Data tab"),
    ("F3 / Alt-2", "Focus SQL tab"),
    ("F4 / Alt-3", "Focus Structure tab"),
];

pub(super) const HELP_DATA_SHORTCUTS: [(&str, &str); 5] = [
    ("Tab", "Cycle assets, grid, and editor"),
    ("/", "Open data filter"),
    ("n / p", "Load next or previous page"),
    ("y / Y / w", "Copy row, cell, or WHERE"),
    (
        "a / D / i",
        "Open INSERT, stage DELETE row, or stage cell edit",
    ),
];

pub(super) const HELP_SQL_SHORTCUTS: [(&str, &str); 8] = [
    ("e", "Open SQL editor from browser"),
    ("Ctrl-Enter", "Run statement under cursor"),
    ("Ctrl-S", "Save current SQL"),
    ("Ctrl-D", "Delete the active saved SQL"),
    ("Ctrl-O", "Open saved SQL"),
    ("F10 / Ctrl-R", "Open SQL history"),
    ("F11 / F12", "Explain / analyze when supported"),
    ("Tab", "Cycle editor, results, and assets"),
];

pub(super) const HELP_STRUCTURE_SHORTCUTS: [(&str, &str); 4] = [
    ("F4 / Alt-3", "Switch to structure tab"),
    ("Tab", "Focus structure grid"),
    ("Enter", "Inspect selected field or row"),
    ("[ / ] / =", "Resize or reset column width"),
];

pub(super) const HELP_DRIVER_SUPPORT_ROWS: [(&str, &str); 3] = [
    ("PostgreSQL", "RETURNING | staged CRUD | ANALYZE"),
    ("MySQL/MariaDB", "backticks | EXPLAIN | no RETURNING"),
    ("SQLite", "QUERY PLAN | no ANALYZE | no RETURNING"),
];
