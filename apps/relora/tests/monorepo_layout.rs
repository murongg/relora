use relora::drivers;
use relora_core::db::DatabaseKind;
use std::{fs, path::Path};

#[test]
fn workspace_exposes_core_without_linking_database_drivers_into_the_app() {
    let kind = DatabaseKind::from_url("postgresql://postgres:postgres@localhost/postgres")
        .expect("postgres url should resolve through core crate");

    assert_eq!(kind, DatabaseKind::Postgres);

    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("relora manifest should be readable");
    assert!(
        !manifest.contains("relora-driver-postgres"),
        "Postgres should be a sidecar driver, not a linked app dependency"
    );
}

#[test]
fn common_database_drivers_are_external_sidecars() {
    let postgres =
        drivers::sidecar_plan(DatabaseKind::Postgres).expect("postgres should use a sidecar");
    let mysql = drivers::sidecar_plan(DatabaseKind::MySql).expect("mysql should use a sidecar");
    let sqlite = drivers::sidecar_plan(DatabaseKind::Sqlite).expect("sqlite should use a sidecar");

    assert_eq!(postgres.binary, "relora-driver-postgres");
    assert_eq!(
        drivers::driver_override_env_var(postgres.binary),
        "RELORA_POSTGRES_DRIVER"
    );
    assert_eq!(mysql.binary, "relora-driver-mysql");
    assert_eq!(
        drivers::driver_override_env_var(mysql.binary),
        "RELORA_MYSQL_DRIVER"
    );
    assert_eq!(sqlite.binary, "relora-driver-sqlite");
    assert_eq!(
        drivers::driver_override_env_var(sqlite.binary),
        "RELORA_SQLITE_DRIVER"
    );
}

#[test]
fn postgres_driver_is_a_workspace_sidecar_package() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("relora app should live under apps/relora");
    let postgres_driver = workspace_root.join("crates/relora-driver-postgres");

    assert!(
        postgres_driver.join("Cargo.toml").exists(),
        "Postgres driver should be a standalone sidecar crate"
    );
    assert_eq!(
        drivers::sidecar_plan(DatabaseKind::Postgres)
            .expect("postgres should use a sidecar")
            .workspace_path,
        Some("crates/relora-driver-postgres")
    );
}

#[test]
fn sqlite_driver_is_a_workspace_sidecar_package() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("relora app should live under apps/relora");
    let sqlite_driver = workspace_root.join("crates/relora-driver-sqlite");

    assert!(
        sqlite_driver.join("Cargo.toml").exists(),
        "SQLite driver should be a standalone sidecar crate"
    );
    assert_eq!(
        drivers::sidecar_plan(DatabaseKind::Sqlite)
            .expect("sqlite should use a sidecar")
            .workspace_path,
        Some("crates/relora-driver-sqlite")
    );
}

#[test]
fn mysql_driver_is_a_workspace_sidecar_package() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("relora app should live under apps/relora");
    let mysql_driver = workspace_root.join("crates/relora-driver-mysql");

    assert!(
        mysql_driver.join("Cargo.toml").exists(),
        "MySQL/MariaDB driver should be a standalone sidecar crate"
    );
    assert_eq!(
        drivers::sidecar_plan(DatabaseKind::MySql)
            .expect("mysql should use a sidecar")
            .workspace_path,
        Some("crates/relora-driver-mysql")
    );
}
