use relora_app::{
    editor::SqlEditorBuffer,
    syntax::{SqlTokenKind, highlight_sql_line},
    templates::{delete_template, insert_template, select_template, update_template},
};
use relora_core::db::{DbColumn, DbObjectKind, DbObjectRef};
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

    let select_sql = select_template(&target, 50);
    let insert_sql = insert_template(&target, &columns);
    let update_sql = update_template(&target, &columns);
    let delete_sql = delete_template(&target, &columns);

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
    ] {
        assert!(path.exists(), "expected {:?} to exist", path);
    }
}

#[test]
fn npm_installer_package_exists_for_binary_distribution() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let npm_package = repo_root.join("packages/relora-npm/package.json");
    let npm_wrapper = repo_root.join("packages/relora-npm/bin/relora.js");
    let npm_install = repo_root.join("packages/relora-npm/scripts/postinstall.cjs");
    let release_bundle = repo_root.join("scripts/package-release-bundle.cjs");
    let curl_install = repo_root.join("scripts/install.sh");

    for path in [
        &npm_package,
        &npm_wrapper,
        &npm_install,
        &release_bundle,
        &curl_install,
    ] {
        assert!(path.exists(), "expected {:?} to exist", path);
    }

    let package_source =
        fs::read_to_string(&npm_package).expect("npm package manifest should be readable");
    assert!(
        package_source.contains("\"name\": \"relora\""),
        "npm package should publish under the expected name"
    );
    assert!(
        package_source.contains("\"relora\": \"./bin/relora.js\""),
        "npm package should expose the relora binary entrypoint"
    );

    let install_source =
        fs::read_to_string(&npm_install).expect("npm install script should be readable");
    assert!(
        install_source.contains("releases/download"),
        "postinstall should download release bundles from GitHub releases"
    );
    assert!(
        install_source.contains("relora-v"),
        "postinstall should resolve versioned Relora bundle asset names"
    );

    let curl_source =
        fs::read_to_string(&curl_install).expect("curl install script should be readable");
    assert!(
        curl_source.contains("releases/download"),
        "curl install should download release bundles from GitHub releases"
    );
    assert!(
        curl_source.contains("RELORA_INSTALL_DIR"),
        "curl install should support configurable install directories"
    );
}

#[test]
fn workspace_manifest_declares_release_metadata() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cargo_toml = repo_root.join("Cargo.toml");
    let source = fs::read_to_string(&cargo_toml).expect("workspace manifest should be readable");

    for expected in [
        "homepage = \"https://github.com/murongg/relora\"",
        "repository = \"https://github.com/murongg/relora.git\"",
        "license = \"MIT\"",
        "rust-version = \"1.85\"",
    ] {
        assert!(
            source.contains(expected),
            "workspace manifest should declare {expected}"
        );
    }
}

#[test]
fn repository_declares_mit_license_file() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let license = repo_root.join("LICENSE");
    assert!(license.exists(), "expected {:?} to exist", license);

    let source = fs::read_to_string(&license).expect("license file should be readable");
    assert!(
        source.contains("MIT License"),
        "license file should contain the MIT license text"
    );
    assert!(
        source.contains("Permission is hereby granted, free of charge"),
        "license file should contain the MIT grant clause"
    );
}

#[test]
fn repository_includes_packaging_docs_and_source_smoke_test_script() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let packaging_doc = repo_root.join("docs/packaging.md");
    let smoke_test = repo_root.join("scripts/smoke-test-source-build.sh");

    assert!(
        packaging_doc.exists(),
        "expected {:?} to exist",
        packaging_doc
    );
    assert!(smoke_test.exists(), "expected {:?} to exist", smoke_test);

    let packaging_source =
        fs::read_to_string(&packaging_doc).expect("packaging doc should be readable");
    assert!(
        packaging_source.contains("Homebrew"),
        "packaging doc should mention Homebrew guidance"
    );
    assert!(
        packaging_source.contains("relora paths --json"),
        "packaging doc should describe the non-interactive smoke test entrypoint"
    );

    let smoke_test_source =
        fs::read_to_string(&smoke_test).expect("source smoke test script should be readable");
    assert!(
        smoke_test_source.contains("cargo build --release"),
        "source smoke test should build release binaries from source"
    );
    assert!(
        smoke_test_source.contains("relora paths --json"),
        "source smoke test should verify the relora diagnostic command"
    );
}

#[test]
fn repository_includes_ci_and_release_workflows() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ci_workflow = repo_root.join(".github/workflows/ci.yml");
    let release_workflow = repo_root.join(".github/workflows/release.yml");

    assert!(ci_workflow.exists(), "expected {:?} to exist", ci_workflow);
    assert!(
        release_workflow.exists(),
        "expected {:?} to exist",
        release_workflow
    );

    let ci_source = fs::read_to_string(&ci_workflow).expect("ci workflow should be readable");
    for expected in [
        "cargo fmt --all -- --check",
        "cargo test --workspace",
        "cargo clippy",
    ] {
        assert!(
            ci_source.contains(expected),
            "ci workflow should run {expected}"
        );
    }

    let release_source =
        fs::read_to_string(&release_workflow).expect("release workflow should be readable");
    for expected in [
        "scripts/package-release-bundle.cjs",
        "actions/upload-artifact",
        "softprops/action-gh-release",
        "actions/setup-node",
        "npm publish --access public --provenance",
        "NODE_AUTH_TOKEN",
    ] {
        assert!(
            release_source.contains(expected),
            "release workflow should contain {expected}"
        );
    }
}

#[test]
fn repository_uses_bumpp_for_release_versioning() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let package_json = repo_root.join("package.json");
    let sync_script = repo_root.join("scripts/sync-version.cjs");

    assert!(
        package_json.exists(),
        "expected {:?} to exist",
        package_json
    );
    assert!(sync_script.exists(), "expected {:?} to exist", sync_script);

    let package_source =
        fs::read_to_string(&package_json).expect("workspace package.json should be readable");
    for expected in [
        "\"release\": \"bumpp",
        "\"bumpp\":",
        "\"version\": \"0.1.0\"",
    ] {
        assert!(
            package_source.contains(expected),
            "workspace package.json should contain {expected}"
        );
    }

    let sync_source =
        fs::read_to_string(&sync_script).expect("version sync script should be readable");
    for expected in [
        "packages/relora-npm/package.json",
        "Cargo.toml",
        "Cargo.lock",
        "workspace.package",
    ] {
        assert!(
            sync_source.contains(expected),
            "version sync script should contain {expected}"
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
