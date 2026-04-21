use clap::Parser;
use relora::config::{
    Cli, CliCommand, LaunchMode, default_connection_store_path, load_saved_connections_from_path,
    save_saved_connections_to_path,
};
use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_store_path(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("relora-{test_name}-{nanos}.json"))
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_env_vars<F>(pairs: &[(&str, Option<&str>)], f: F)
where
    F: FnOnce(),
{
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let original = pairs
        .iter()
        .map(|(key, _)| (key.to_string(), std::env::var_os(key)))
        .collect::<Vec<_>>();
    struct EnvRestore(Vec<(String, Option<std::ffi::OsString>)>);

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in self.0.drain(..) {
                match value {
                    Some(value) => unsafe { std::env::set_var(key, value) },
                    None => unsafe { std::env::remove_var(key) },
                }
            }
        }
    }

    let _restore = EnvRestore(original);

    for (key, value) in pairs {
        match value {
            Some(value) => unsafe { std::env::set_var(key, value) },
            None => unsafe { std::env::remove_var(key) },
        }
    }

    f();
}

fn without_connection_envs<F>(f: F)
where
    F: FnOnce(),
{
    with_env_vars(
        &[
            ("RELORA_CONNECTIONS", None),
            ("RELORA_DATABASE_URL", None),
            ("RELORA_CONNECTION_STORE", None),
            ("DATABASE_URL", None),
            ("XDG_CONFIG_HOME", None),
        ],
        f,
    );
}

#[test]
fn cli_without_runtime_connections_enters_launcher_mode() {
    without_connection_envs(|| {
        let cli = Cli::parse_from(["relora"]);
        let store_path = temp_store_path("launcher-mode");
        let config = cli
            .into_config_with_store_path(store_path.clone())
            .expect("config should fall back to launcher mode");

        assert_eq!(config.launch_mode, LaunchMode::Launcher);
        assert!(config.connections.is_empty());
        assert!(config.saved_connections.is_empty());
    });
}

#[test]
fn cli_supports_multiple_named_connections() {
    without_connection_envs(|| {
        let cli = Cli::parse_from([
            "relora",
            "--connection",
            "pg=postgresql://postgres:postgres@localhost/postgres",
            "--connection",
            "analytics=postgresql://postgres:postgres@localhost/analytics",
            "--preview-limit",
            "25",
        ]);

        let config = cli
            .into_config()
            .expect("multiple named connections should parse");

        assert_eq!(config.launch_mode, LaunchMode::Workspace);
        assert_eq!(config.connections.len(), 2);
        assert_eq!(config.connections[0].name, "pg");
        assert_eq!(config.connections[1].name, "analytics");
        assert_eq!(config.preview_limit, 25);
    });
}

#[test]
fn cli_rejects_invalid_named_connection_format() {
    without_connection_envs(|| {
        let cli = Cli::parse_from(["relora", "--connection", "postgresql://localhost/postgres"]);
        let error = cli
            .into_config()
            .expect_err("named connections should require name=url format");

        assert!(format!("{error}").contains("invalid connection spec"));
    });
}

#[test]
fn cli_loads_saved_connections_for_launcher_mode() {
    without_connection_envs(|| {
        let store_path = temp_store_path("saved-connections");
        save_saved_connections_to_path(
            &store_path,
            &[relora::config::ConnectionConfig {
                name: "pg".to_string(),
                url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
                read_only: false,
            }],
        )
        .expect("saved connections should be written");

        let cli = Cli::parse_from(["relora"]);
        let config = cli
            .into_config_with_store_path(store_path.clone())
            .expect("launcher mode should load saved connections");

        assert_eq!(config.launch_mode, LaunchMode::Launcher);
        assert_eq!(config.saved_connections.len(), 1);
        assert_eq!(config.saved_connections[0].name, "pg");
        assert_eq!(
            load_saved_connections_from_path(&store_path)
                .expect("saved connections should round-trip")
                .len(),
            1
        );
    });
}

#[test]
fn cli_uses_relora_runtime_env_vars() {
    let store_path = temp_store_path("relora-runtime-env");
    with_env_vars(
        &[
            (
                "RELORA_CONNECTIONS",
                Some("pg=postgresql://postgres:postgres@localhost/postgres"),
            ),
            (
                "RELORA_DATABASE_URL",
                Some("postgresql://postgres:postgres@localhost/postgres"),
            ),
            ("DATABASE_URL", None),
        ],
        || {
            let cli = Cli::parse_from(["relora"]);
            let config = cli
                .into_config_with_store_path(store_path.clone())
                .expect("relora env vars should supply runtime connections");

            assert_eq!(config.launch_mode, LaunchMode::Workspace);
            assert_eq!(config.connections.len(), 1);
            assert_eq!(config.connections[0].name, "pg");
        },
    );
}

#[test]
fn default_connection_store_path_prefers_relora_store_env_var() {
    let store_path = std::env::temp_dir().join("relora-explicit-store.json");
    with_env_vars(
        &[
            (
                "RELORA_CONNECTION_STORE",
                Some(store_path.to_string_lossy().as_ref()),
            ),
            ("XDG_CONFIG_HOME", None),
            ("HOME", None),
        ],
        || {
            assert_eq!(default_connection_store_path(), store_path);
        },
    );
}

#[test]
fn cli_does_not_expose_cargo_driver_install_commands() {
    assert!(Cli::try_parse_from(["relora", "driver", "install", "postgres"]).is_err());
    assert!(Cli::try_parse_from(["relora", "driver", "install", "mysql"]).is_err());
    assert!(Cli::try_parse_from(["relora", "driver", "install", "sqlite"]).is_err());
}

#[test]
fn cli_supports_paths_command_for_non_interactive_diagnostics() {
    let cli = Cli::parse_from(["relora", "paths", "--json"]);

    assert_eq!(cli.command, Some(CliCommand::Paths { json: true }));
}

#[test]
fn paths_report_lists_store_path_and_known_driver_binaries() {
    let store_path = temp_store_path("paths-report");
    let report = relora::commands::build_paths_report_with_store_path(store_path.clone());

    assert_eq!(report.connection_store_path, store_path);
    assert_eq!(report.app_name, "relora");
    assert_eq!(report.drivers.len(), 3);
    assert_eq!(report.drivers[0].binary, "relora-driver-postgres");
    assert_eq!(report.drivers[1].binary, "relora-driver-mysql");
    assert_eq!(report.drivers[2].binary, "relora-driver-sqlite");

    let json = serde_json::to_value(&report).expect("paths report should serialize");
    assert_eq!(
        json.get("app_name").and_then(|value| value.as_str()),
        Some("relora")
    );
    assert_eq!(
        json.get("connection_store_path")
            .and_then(|value| value.as_str()),
        Some(store_path.to_string_lossy().as_ref())
    );
}
