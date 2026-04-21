pub(super) const APP_NAME: &str = "Relora";
pub(super) const LAUNCHER_PIXEL_WORDMARK: [&str; 2] =
    ["█▀█ █▀▀ █   █▀█ █▀█ ▄▀█", "█▀▄ ██▄ █▄▄ █▄█ █▀▄ █▀█"];
pub(super) const PRODUCT_TAGLINE: &str = "Terminal Database Workspace";
pub(super) const PRODUCT_DESCRIPTION: &str = "Open a saved workspace. Relora keeps multi-connection browsing, SQL editing and structure insight in one terminal canvas.";
pub(super) const PRODUCT_ENGINE_NOTE: &str =
    "PostgreSQL, MySQL/MariaDB and SQLite use external sidecar drivers.";

pub(super) const TITLE_ASSETS: &str = "Assets";
pub(super) const TITLE_TABS: &str = "Tabs";
pub(super) const TITLE_SELECTION: &str = "Selection";
pub(super) const TITLE_OVERVIEW: &str = "Overview";
pub(super) const TITLE_COMPLETION: &str = "Completion";
pub(super) const TITLE_RESULTS: &str = "Results";
pub(super) const TITLE_STATUS: &str = "Status";
pub(super) const TITLE_SEARCH: &str = "Search";
pub(super) const TITLE_STRUCTURE: &str = "Structure";
pub(super) const TITLE_SQL: &str = "SQL";
pub(super) const TITLE_KEYBOARD_HELP: &str = "Keyboard Help";
pub(super) const TITLE_SAVED_CONNECTIONS: &str = "Saved Connections";
pub(super) const TITLE_NEW_CONNECTION: &str = "New Connection";
pub(super) const TITLE_EDIT_CONNECTION: &str = "Edit Connection";
pub(super) const TITLE_DELETE_CONNECTION: &str = "Delete Connection";
pub(super) const TITLE_DRIVER_MISSING: &str = "Driver Missing";

pub(super) const TAB_DATA: &str = "Data";
pub(super) const TAB_SQL: &str = "SQL";
pub(super) const TAB_STRUCTURE: &str = "Structure";
pub(super) const RIGHT_TAB_TITLES: [&str; 3] = [TAB_DATA, TAB_SQL, TAB_STRUCTURE];

pub(super) const LAUNCHER_EMPTY_TITLE: &str = "No saved connections yet.";
pub(super) const LAUNCHER_EMPTY_PRIMARY: &str = "Create your first PostgreSQL profile with `a`.";
pub(super) const LAUNCHER_EMPTY_SECONDARY: &str =
    "Once a profile is saved, Enter launches it into the main workspace.";
pub(super) const LAUNCHER_DEFAULT_STATUS: &str =
    "Choose a saved workspace or create a new connection profile.";
pub(super) const LAUNCHER_STATUS_QUEUED: &str = "Queued";
pub(super) const LAUNCHER_STATUS_FOCUSED: &str = "Focused";
pub(super) const LAUNCHER_STATUS_READY: &str = "Ready";
pub(super) const DATABASE_BADGE_POSTGRES: &str = " PG ";
pub(super) const DATABASE_BADGE_MYSQL: &str = " MY ";
pub(super) const DATABASE_BADGE_SQLITE: &str = " SQ ";
pub(super) const DATABASE_BADGE_GENERIC: &str = " DB ";

pub(super) const FORM_NAME_LABEL: &str = "Name";
pub(super) const FORM_DRIVER_LABEL: &str = "Driver";
pub(super) const FORM_ACCESS_LABEL: &str = "Mode";
pub(super) const FORM_HOST_LABEL: &str = "Host / SQLite path";
pub(super) const FORM_PORT_LABEL: &str = "Port";
pub(super) const FORM_DATABASE_LABEL: &str = "Database";
pub(super) const FORM_USERNAME_LABEL: &str = "User";
pub(super) const FORM_PASSWORD_LABEL: &str = "Password";
pub(super) const FORM_URL_LABEL: &str = "URL override";
pub(super) const DRIVER_MISSING_WARNING: &str =
    "Use a full Relora bundle, put the sidecar on PATH, or point Relora at it with the env var.";
pub(super) const DELETE_CONNECTION_WARNING_PROFILE: &str =
    "Only the saved Relora profile is removed.";
pub(super) const DELETE_CONNECTION_WARNING_DATABASE: &str = "The database is not modified.";
pub(super) const DELETE_OPERATION_WARNING: &str =
    "Relora will send this statement to the database only after confirmation.";

pub(super) const HELP_SECTION_GLOBAL: &str = "Global";
pub(super) const HELP_SECTION_DATA: &str = "Data";
pub(super) const HELP_SECTION_SQL: &str = "SQL";
pub(super) const HELP_SECTION_STRUCTURE: &str = "Structure";
pub(super) const HELP_SECTION_DRIVER_SUPPORT: &str = "Driver Support";

pub(super) const EMPTY_GRID_MESSAGE: &str = "No rows available.";
pub(super) const LOADING_PREVIEW_MESSAGE: &str = "Loading preview...";
pub(super) const OPEN_SQL_EDITOR_MESSAGE: &str =
    "Open a SQL editor with e, Ctrl-P, or the command palette.";
pub(super) const SELECT_TABLE_OBJECT_MESSAGE: &str = "Select a table-like object.";
pub(super) const LOADING_STRUCTURE_MESSAGE: &str = "Loading structure...";
pub(super) const NO_COLUMNS_MESSAGE: &str = "No columns available.";
pub(super) const RUN_SQL_RESULTS_MESSAGE: &str = "Run SQL to see results here.";
pub(super) const NO_MATCHING_COMMANDS_MESSAGE: &str = "No matching commands";
pub(super) const NO_MATCHING_SQL_HISTORY_MESSAGE: &str = "No matching SQL history";
pub(super) const READY_STATUS: &str = "Ready.";
pub(super) const UNKNOWN_LABEL: &str = "Unknown";
pub(super) const NOT_AVAILABLE_LABEL: &str = "n/a";
pub(super) const NULL_LABEL: &str = "NULL";

pub(super) const DATABASE_KIND_POSTGRES: &str = "PostgreSQL";
pub(super) const DATABASE_KIND_MYSQL: &str = "MySQL/MariaDB";
pub(super) const DATABASE_KIND_SQLITE: &str = "SQLite";
