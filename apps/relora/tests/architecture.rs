use relora_app::{
    editor::SqlEditorBuffer,
    syntax::{SqlTokenKind, highlight_sql_line},
    templates::{delete_template, insert_template, select_template, update_template},
};
use relora_core::db::{DatabaseKind, DbColumn, DbObjectKind, DbObjectRef, DriverCapabilities};
use std::fs;
use std::path::Path;

fn object(kind: DbObjectKind, schema: &str, name: &str) -> DbObjectRef {
    DbObjectRef {
        database: "postgres".to_string(),
        schema: schema.to_string(),
        name: name.to_string(),
        kind,
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

#[test]
fn sql_editor_buffer_handles_insert_newline_and_backspace_without_ui() {
    let mut buffer = SqlEditorBuffer::from_sql("select 1");
    buffer.insert_char(';');
    buffer.new_line();
    buffer.insert_str("select 2;");
    buffer.move_up();
    buffer.move_left();
    buffer.backspace();

    assert_eq!(buffer.sql(), "select ;\nselect 2;");
    assert_eq!(buffer.cursor(), (0, 7));
}

#[test]
fn sql_highlighter_classifies_common_postgres_tokens() {
    let tokens = highlight_sql_line("SELECT 'bob' FROM users WHERE id = 1 -- active");
    let significant = tokens
        .iter()
        .filter(|token| token.kind != SqlTokenKind::Whitespace)
        .map(|token| (token.kind, token.text.as_str()))
        .collect::<Vec<_>>();

    assert_eq!(
        significant,
        vec![
            (SqlTokenKind::Keyword, "SELECT"),
            (SqlTokenKind::String, "'bob'"),
            (SqlTokenKind::Keyword, "FROM"),
            (SqlTokenKind::Identifier, "users"),
            (SqlTokenKind::Keyword, "WHERE"),
            (SqlTokenKind::Identifier, "id"),
            (SqlTokenKind::Symbol, "="),
            (SqlTokenKind::Number, "1"),
            (SqlTokenKind::Comment, "-- active"),
        ]
    );
}

#[test]
fn crud_templates_prefer_primary_keys_and_quote_identifiers() {
    let target = object(DbObjectKind::Table, "public", "users");
    let columns = columns(&[
        ("id", "integer", false, true, true),
        ("email", "text", false, false, false),
        ("display_name", "text", true, false, false),
    ]);
    let capabilities = DriverCapabilities::for_kind(DatabaseKind::Postgres);

    let select_sql = select_template(capabilities, &target, 50);
    let insert_sql = insert_template(capabilities, &target, &columns);
    let update_sql = update_template(capabilities, &target, &columns);
    let delete_sql = delete_template(capabilities, &target, &columns);

    assert!(select_sql.contains("FROM \"public\".\"users\""));
    assert!(insert_sql.contains("INSERT INTO \"public\".\"users\""));
    assert!(insert_sql.contains("\"email\""));
    assert!(update_sql.contains("WHERE \"id\" ="));
    assert!(delete_sql.contains("DELETE FROM \"public\".\"users\""));
    assert!(delete_sql.contains("WHERE \"id\" ="));
}

#[test]
fn tui_runtime_is_split_into_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui");
    for path in [
        root.join("mod.rs"),
        root.join("colors.rs"),
        root.join("metrics.rs"),
        root.join("shortcuts.rs"),
        root.join("strings.rs"),
        root.join("input.rs"),
        root.join("layout.rs"),
        root.join("grid.rs"),
        root.join("render.rs"),
        root.join("tests.rs"),
        root.join("snapshot_tests.rs"),
    ] {
        assert!(path.exists(), "expected {:?} to exist", path);
    }
}

#[test]
fn tui_golden_snapshots_exist_for_core_surfaces() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui");
    let snapshot_tests = root.join("snapshot_tests.rs");
    assert!(
        snapshot_tests.exists(),
        "expected {:?} to define the golden snapshot suite",
        snapshot_tests
    );

    let snapshot_test_source =
        fs::read_to_string(&snapshot_tests).expect("snapshot test module should be readable");
    for scenario in [
        "launcher_golden_snapshot",
        "data_tab_golden_snapshot",
        "sql_tab_golden_snapshot",
        "structure_tab_golden_snapshot",
        "row_inspector_golden_snapshot",
        "help_overlay_golden_snapshot",
    ] {
        assert!(
            snapshot_test_source.contains(scenario),
            "snapshot test module should define {scenario}"
        );
    }

    let snapshots = root.join("snapshots");
    for path in [
        snapshots.join("launcher.snap"),
        snapshots.join("workspace_data_tab.snap"),
        snapshots.join("workspace_sql_tab.snap"),
        snapshots.join("workspace_structure_tab.snap"),
        snapshots.join("workspace_row_inspector.snap"),
        snapshots.join("workspace_help_overlay.snap"),
    ] {
        assert!(path.exists(), "expected {:?} to exist", path);
    }
}

#[test]
fn workspace_hot_path_benchmarks_exist() {
    let bench = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/relora-app/benches/workspace_hot_paths.rs");
    assert!(bench.exists(), "expected {:?} to exist", bench);

    let bench_source = fs::read_to_string(&bench).expect("benchmark should be readable");
    for scenario in [
        "workspace_bootstrap_large_catalog",
        "workspace_cancel_inflight_preview",
        "workspace_scroll_wide_preview_columns",
        "workspace_switch_sql_result_sets",
    ] {
        assert!(
            bench_source.contains(scenario),
            "benchmark should define scenario {scenario}"
        );
    }
}

#[test]
fn tui_render_hot_path_benchmarks_exist() {
    let bench = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/tui_render_hot_paths.rs");
    assert!(bench.exists(), "expected {:?} to exist", bench);

    let bench_source = fs::read_to_string(&bench).expect("benchmark should be readable");
    for scenario in [
        "render_workspace_data_tab_dense_grid",
        "render_workspace_sql_tab_result_grid",
        "render_workspace_row_inspector_long_text",
    ] {
        assert!(
            bench_source.contains(scenario),
            "benchmark should define scenario {scenario}"
        );
    }
}

#[test]
fn tui_layout_metrics_are_defined_in_the_metrics_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui");
    let metrics = root.join("metrics.rs");
    assert!(metrics.exists(), "expected {:?} to exist", metrics);

    let metrics_source = fs::read_to_string(&metrics).expect("metrics module should be readable");
    for name in [
        "EVENT_POLL_INTERVAL",
        "WORKSPACE_HEADER_HEIGHT",
        "WORKSPACE_ASSETS_WIDTH_PERCENT",
        "LAUNCHER_CARD_WIDTH_PERCENT",
        "ROW_INSPECTOR_POPUP_WIDTH_PERCENT",
    ] {
        assert!(
            metrics_source.contains(name),
            "metrics module should define {name}"
        );
    }

    let layout_source =
        fs::read_to_string(root.join("layout.rs")).expect("layout module should be readable");
    for literal in [
        "Constraint::Length(3)",
        "Constraint::Min(10)",
        "Constraint::Percentage(30)",
        "Constraint::Percentage(70)",
        "Constraint::Percentage(55)",
        "Constraint::Percentage(45)",
    ] {
        assert!(
            !layout_source.contains(literal),
            "replace {literal} with a named layout metric"
        );
    }
}

#[test]
fn tui_copy_is_defined_in_the_strings_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui");
    let strings = root.join("strings.rs");
    assert!(strings.exists(), "expected {:?} to exist", strings);

    let strings_source = fs::read_to_string(&strings).expect("strings module should be readable");
    for name in ["APP_NAME", "PRODUCT_TAGLINE", "RIGHT_TAB_TITLES"] {
        assert!(
            strings_source.contains(name),
            "strings module should define {name}"
        );
    }

    let render_source =
        fs::read_to_string(root.join("render.rs")).expect("render module should be readable");
    for literal in [
        "\"Relora\"",
        "\"Terminal Database Workspace\"",
        "\"Saved Connections\"",
        "\"Status\"",
    ] {
        assert!(
            !render_source.contains(literal),
            "move {literal} from render.rs into strings.rs"
        );
    }
}

#[test]
fn tui_shortcuts_are_defined_in_the_shortcuts_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui");
    let shortcuts = root.join("shortcuts.rs");
    assert!(shortcuts.exists(), "expected {:?} to exist", shortcuts);

    let shortcuts_source =
        fs::read_to_string(&shortcuts).expect("shortcuts module should be readable");
    for name in [
        "KEY_BROWSER_OPEN_SQL",
        "FKEY_EDITOR_EXECUTE",
        "RIGHT_TAB_SHORTCUT_HELP",
        "FOOTER_SQL_EDITOR_HELP",
    ] {
        assert!(
            shortcuts_source.contains(name),
            "shortcuts module should define {name}"
        );
    }

    let input_source =
        fs::read_to_string(root.join("input.rs")).expect("input module should be readable");
    for literal in [
        "KeyCode::Char('e') => Some(WorkspaceAction::OpenSqlEditor)",
        "KeyCode::F(5) => app.apply_action(WorkspaceAction::ExecuteEditor)",
        "KeyCode::Char('N') => Some(WorkspaceAction::NextPreviewPage)",
    ] {
        assert!(
            !input_source.contains(literal),
            "move {literal} from input.rs into shortcuts.rs-backed constants"
        );
    }
}

#[test]
fn tui_colors_are_defined_only_in_the_colors_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui");
    let colors = root.join("colors.rs");
    assert!(colors.exists(), "expected {:?} to exist", colors);

    let colors_source = fs::read_to_string(&colors).expect("colors module should be readable");
    assert!(
        colors_source.contains("pub(super) const ACCENT"),
        "colors module should expose named theme constants"
    );

    for module in ["mod.rs", "input.rs", "layout.rs", "grid.rs", "render.rs"] {
        let path = root.join(module);
        let source = fs::read_to_string(&path).expect("tui module should be readable");
        assert!(
            !source.contains("Color::"),
            "move direct color literals from {:?} into colors.rs",
            path
        );
    }
}

#[test]
fn tui_runtime_enables_keyboard_enhancement_for_modified_enter_keys() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui");
    let runtime = root.join("mod.rs");
    let source = fs::read_to_string(&runtime).expect("tui runtime module should be readable");

    for expected in [
        "supports_keyboard_enhancement",
        "PushKeyboardEnhancementFlags",
        "PopKeyboardEnhancementFlags",
        "KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES",
        "KeyboardEnhancementFlags::REPORT_EVENT_TYPES",
    ] {
        assert!(
            source.contains(expected),
            "tui runtime should contain {expected} so modified enter keys are reported distinctly"
        );
    }
}
