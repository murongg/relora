use std::{env, ffi::OsString, path::PathBuf, process::Command};

use anyhow::{Context, Result, bail};
use relora_core::db::{
    Catalog, CatalogSummary, DatabaseDriver, DatabaseKind, DbColumn, DbObjectKind, DbObjectRef,
    DriverCapabilities, SqlExecutionResult, TablePreview,
};
use serde::de::DeserializeOwned;
use url::Url;

use crate::config::ConnectionConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverSidecarPlan {
    pub kind: DatabaseKind,
    pub display_name: &'static str,
    pub binary: &'static str,
    pub workspace_path: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverPathStatus {
    pub kind: &'static str,
    pub display_name: &'static str,
    pub binary: &'static str,
    pub override_env: String,
    pub resolved_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverAvailability {
    Available,
    Missing(DriverSidecarPlan),
}

pub fn connect(connection: &ConnectionConfig) -> Result<Box<dyn DatabaseDriver>> {
    match DatabaseKind::from_url(&connection.url)? {
        kind @ (DatabaseKind::Postgres | DatabaseKind::MySql | DatabaseKind::Sqlite) => {
            connect_external(kind, &connection.url)
        }
    }
}

pub fn sidecar_plan(kind: DatabaseKind) -> Option<DriverSidecarPlan> {
    match kind {
        DatabaseKind::Postgres => Some(DriverSidecarPlan {
            kind,
            display_name: "PostgreSQL",
            binary: "relora-driver-postgres",
            workspace_path: Some("crates/relora-driver-postgres"),
        }),
        DatabaseKind::MySql => Some(DriverSidecarPlan {
            kind,
            display_name: "MySQL/MariaDB",
            binary: "relora-driver-mysql",
            workspace_path: Some("crates/relora-driver-mysql"),
        }),
        DatabaseKind::Sqlite => Some(DriverSidecarPlan {
            kind,
            display_name: "SQLite",
            binary: "relora-driver-sqlite",
            workspace_path: Some("crates/relora-driver-sqlite"),
        }),
    }
}

pub fn driver_availability_for_url(url: &str) -> Result<DriverAvailability> {
    let kind = DatabaseKind::from_url(url)?;
    let Some(plan) = sidecar_plan(kind) else {
        return Ok(DriverAvailability::Available);
    };
    Ok(driver_availability_from_plan(plan, |binary| {
        find_driver_binary(binary)
    }))
}

pub fn test_connection(connection: &ConnectionConfig) -> Result<()> {
    let mut driver = connect(connection)?;
    driver.load_catalog()?;
    Ok(())
}

pub fn driver_path_statuses() -> Vec<DriverPathStatus> {
    [
        DatabaseKind::Postgres,
        DatabaseKind::MySql,
        DatabaseKind::Sqlite,
    ]
    .into_iter()
    .filter_map(sidecar_plan)
    .map(|plan| DriverPathStatus {
        kind: kind_slug(plan.kind),
        display_name: plan.display_name,
        binary: plan.binary,
        override_env: driver_override_env_var(plan.binary),
        resolved_path: find_driver_binary(plan.binary),
    })
    .collect()
}

fn driver_availability_from_plan<F>(plan: DriverSidecarPlan, lookup: F) -> DriverAvailability
where
    F: FnOnce(&str) -> Option<PathBuf>,
{
    if lookup(plan.binary).is_some() {
        DriverAvailability::Available
    } else {
        DriverAvailability::Missing(plan)
    }
}

fn connect_external(kind: DatabaseKind, url: &str) -> Result<Box<dyn DatabaseDriver>> {
    connect_external_with_lookup(kind, url, find_driver_binary)
}

fn connect_external_with_lookup<F>(
    kind: DatabaseKind,
    url: &str,
    lookup: F,
) -> Result<Box<dyn DatabaseDriver>>
where
    F: FnOnce(&str) -> Option<PathBuf>,
{
    let Some(plan) = sidecar_plan(kind) else {
        bail!("{} does not use an external driver", kind_label(kind));
    };

    let Some(binary) = lookup(plan.binary) else {
        bail!(
            "{} driver is missing. Install the full Relora bundle, put `{}` on PATH, or set `{}`.",
            plan.display_name,
            plan.binary,
            driver_override_env_var(plan.binary)
        );
    };

    Ok(Box::new(ExternalCommandDriver::new(kind, url, binary)))
}

fn find_driver_binary(binary: &str) -> Option<PathBuf> {
    let override_env = driver_override_env_var(binary);
    if let Some(path) = env::var_os(override_env) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    preferred_driver_candidate(
        bundled_driver(binary),
        workspace_target_driver(binary),
        path_driver(binary),
        cargo_bin_driver(binary),
    )
}

fn preferred_driver_candidate(
    bundled: Option<PathBuf>,
    workspace: Option<PathBuf>,
    path: Option<PathBuf>,
    cargo_bin: Option<PathBuf>,
) -> Option<PathBuf> {
    bundled.or(workspace).or(path).or(cargo_bin)
}

fn path_driver(binary: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|dir| dir.join(binary))
            .find(|candidate| candidate.is_file())
    })
}

pub fn driver_override_env_var(binary: &str) -> String {
    format!(
        "RELORA_{}_DRIVER",
        binary
            .strip_prefix("relora-driver-")
            .unwrap_or(binary)
            .replace('-', "_")
            .to_uppercase()
    )
}

struct ExternalCommandDriver {
    kind: DatabaseKind,
    url: String,
    binary: PathBuf,
    connection_label: String,
    capabilities: DriverCapabilities,
}

impl ExternalCommandDriver {
    fn new(kind: DatabaseKind, url: &str, binary: PathBuf) -> Self {
        let capabilities =
            run_json_command::<DriverCapabilities>(&binary, url, kind, "capabilities", Vec::new())
                .unwrap_or_else(|_| DriverCapabilities::for_kind(kind));
        Self {
            kind,
            url: url.to_string(),
            binary,
            connection_label: external_connection_label(kind, url),
            capabilities,
        }
    }

    fn run_json<T>(&self, command: &str, args: Vec<OsString>) -> Result<T>
    where
        T: DeserializeOwned,
    {
        run_json_command(&self.binary, &self.url, self.kind, command, args)
    }
}

impl DatabaseDriver for ExternalCommandDriver {
    fn kind(&self) -> DatabaseKind {
        self.kind
    }

    fn capabilities(&self) -> DriverCapabilities {
        self.capabilities
    }

    fn connection_label(&self) -> &str {
        &self.connection_label
    }

    fn load_catalog(&mut self) -> Result<Catalog> {
        self.run_json("catalog", Vec::new())
    }

    fn load_catalog_summary(&mut self) -> Result<CatalogSummary> {
        self.run_json("catalog-summary", Vec::new())
    }

    fn load_schema_objects(&mut self, database: &str, schema: &str) -> Result<Vec<DbObjectRef>> {
        self.run_json(
            "schema-objects",
            vec![
                "--database".into(),
                database.into(),
                "--schema".into(),
                schema.into(),
            ],
        )
    }

    fn load_schema_objects_of_kind(
        &mut self,
        database: &str,
        schema: &str,
        kind: DbObjectKind,
    ) -> Result<Vec<DbObjectRef>> {
        self.run_json(
            "schema-objects",
            vec![
                "--database".into(),
                database.into(),
                "--schema".into(),
                schema.into(),
                "--kind".into(),
                kind.wire_name().into(),
            ],
        )
    }

    fn load_preview_page(
        &mut self,
        table: &DbObjectRef,
        limit: usize,
        offset: usize,
    ) -> Result<TablePreview> {
        self.run_json(
            "preview",
            vec![
                "--object".into(),
                serde_json::to_string(table)?.into(),
                "--limit".into(),
                limit.to_string().into(),
                "--offset".into(),
                offset.to_string().into(),
            ],
        )
    }

    fn load_filtered_preview_page(
        &mut self,
        table: &DbObjectRef,
        filter: &str,
        limit: usize,
        offset: usize,
    ) -> Result<TablePreview> {
        self.run_json(
            "preview",
            vec![
                "--object".into(),
                serde_json::to_string(table)?.into(),
                "--filter".into(),
                filter.into(),
                "--limit".into(),
                limit.to_string().into(),
                "--offset".into(),
                offset.to_string().into(),
            ],
        )
    }

    fn load_object_columns(&mut self, table: &DbObjectRef) -> Result<Vec<DbColumn>> {
        self.run_json(
            "columns",
            vec!["--object".into(), serde_json::to_string(table)?.into()],
        )
    }

    fn execute_sql(
        &mut self,
        database: Option<&str>,
        sql: &str,
    ) -> Result<Vec<SqlExecutionResult>> {
        let mut args = Vec::new();
        if let Some(database) = database {
            args.push("--database".into());
            args.push(database.into());
        }
        args.push("--sql".into());
        args.push(sql.into());
        self.run_json("execute", args)
    }
}

fn run_json_command<T>(
    binary: &PathBuf,
    url: &str,
    kind: DatabaseKind,
    command: &str,
    args: Vec<OsString>,
) -> Result<T>
where
    T: DeserializeOwned,
{
    let output = Command::new(binary)
        .arg("--url")
        .arg(url)
        .arg(command)
        .args(args)
        .output()
        .with_context(|| {
            format!(
                "failed to run external {} driver at {}",
                kind_label(kind),
                binary.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "external {} driver command `{}` failed: {}",
            kind_label(kind),
            command,
            stderr.trim()
        );
    }

    serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "external {} driver returned invalid JSON for `{}`",
            kind_label(kind),
            command
        )
    })
}

fn external_connection_label(kind: DatabaseKind, url: &str) -> String {
    if kind == DatabaseKind::Sqlite {
        return Url::parse(url)
            .ok()
            .map(|parsed| parsed.path().to_string())
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| "sqlite".to_string());
    }

    let Ok(parsed) = Url::parse(url) else {
        return kind_label(kind).to_string();
    };
    let host = parsed.host_str().unwrap_or("localhost");
    let port = parsed
        .port()
        .map(|value| format!(":{value}"))
        .unwrap_or_default();
    let database = parsed.path().trim_start_matches('/');
    if database.is_empty() {
        format!("{}://{host}{port}", parsed.scheme())
    } else {
        format!("{}://{host}{port}/{database}", parsed.scheme())
    }
}

fn kind_label(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::Postgres => "PostgreSQL",
        DatabaseKind::MySql => "MySQL/MariaDB",
        DatabaseKind::Sqlite => "SQLite",
    }
}

fn kind_slug(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::Postgres => "postgresql",
        DatabaseKind::MySql => "mysql",
        DatabaseKind::Sqlite => "sqlite",
    }
}

fn bundled_driver(binary: &str) -> Option<PathBuf> {
    let executable = env::current_exe().ok()?;
    let directory = executable.parent()?;
    let candidate = directory.join(binary);
    candidate.is_file().then_some(candidate)
}

fn cargo_bin_driver(binary: &str) -> Option<PathBuf> {
    if let Some(cargo_home) = env::var_os("CARGO_HOME") {
        let path = PathBuf::from(cargo_home).join("bin").join(binary);
        if path.is_file() {
            return Some(path);
        }
    }

    let home = env::var_os("HOME")?;
    let path = PathBuf::from(home).join(".cargo").join("bin").join(binary);
    path.is_file().then_some(path)
}

fn workspace_target_driver(binary: &str) -> Option<PathBuf> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    ["debug", "release"]
        .into_iter()
        .map(|profile| workspace_root.join("target").join(profile).join(binary))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_plan_describes_external_common_drivers() {
        let postgres = sidecar_plan(DatabaseKind::Postgres).expect("postgres should use a sidecar");
        assert_eq!(postgres.display_name, "PostgreSQL");
        assert_eq!(postgres.binary, "relora-driver-postgres");
        assert_eq!(
            postgres.workspace_path,
            Some("crates/relora-driver-postgres")
        );

        let mysql = sidecar_plan(DatabaseKind::MySql).expect("mysql should use a sidecar");
        assert_eq!(mysql.display_name, "MySQL/MariaDB");
        assert_eq!(mysql.binary, "relora-driver-mysql");
        assert_eq!(mysql.workspace_path, Some("crates/relora-driver-mysql"));

        let sqlite = sidecar_plan(DatabaseKind::Sqlite).expect("sqlite should use a sidecar");
        assert_eq!(sqlite.display_name, "SQLite");
        assert_eq!(sqlite.binary, "relora-driver-sqlite");
        assert_eq!(sqlite.workspace_path, Some("crates/relora-driver-sqlite"));

        assert!(sidecar_plan(DatabaseKind::Postgres).is_some());
    }

    #[test]
    fn external_connection_labels_hide_credentials() {
        assert_eq!(
            external_connection_label(
                DatabaseKind::Postgres,
                "postgresql://postgres:secret@localhost:5432/app"
            ),
            "postgresql://localhost:5432/app"
        );
        assert_eq!(
            external_connection_label(
                DatabaseKind::MySql,
                "mysql://root:secret@localhost:3306/app"
            ),
            "mysql://localhost:3306/app"
        );
        assert_eq!(
            external_connection_label(DatabaseKind::Sqlite, "sqlite:///tmp/app.db"),
            "/tmp/app.db"
        );
    }

    #[test]
    fn missing_driver_error_explains_sidecar_discovery() {
        let error = match connect_external_with_lookup(
            DatabaseKind::MySql,
            "mysql://root@localhost/app",
            |_| None,
        ) {
            Ok(_) => panic!("missing mysql driver should explain installation"),
            Err(error) => error,
        };
        let message = format!("{error:#}");

        assert!(message.contains("MySQL/MariaDB driver is missing"));
        assert!(message.contains("relora-driver-mysql"));
        assert!(message.contains("RELORA_MYSQL_DRIVER"));
        assert!(!message.contains("cargo install"));
    }

    #[test]
    fn driver_availability_can_report_missing_sidecar_driver() {
        let plan = sidecar_plan(DatabaseKind::Postgres).expect("postgres should use a sidecar");
        let availability = driver_availability_from_plan(plan, |_| None);

        assert_eq!(availability, DriverAvailability::Missing(plan));
    }

    #[test]
    fn driver_override_env_var_is_stable_for_sidecars() {
        assert_eq!(
            driver_override_env_var("relora-driver-mysql"),
            "RELORA_MYSQL_DRIVER"
        );
    }

    #[test]
    fn driver_resolution_prefers_bundled_and_workspace_sidecars_before_global_bins() {
        let bundled = Some(PathBuf::from("/bundle/relora-driver-mysql"));
        let workspace = Some(PathBuf::from("/workspace/target/debug/relora-driver-mysql"));
        let path = Some(PathBuf::from("/usr/local/bin/relora-driver-mysql"));
        let cargo_bin = Some(PathBuf::from(
            "/Users/example/.cargo/bin/relora-driver-mysql",
        ));

        assert_eq!(
            preferred_driver_candidate(
                bundled.clone(),
                workspace.clone(),
                path.clone(),
                cargo_bin.clone()
            ),
            bundled
        );
        assert_eq!(
            preferred_driver_candidate(None, workspace.clone(), path.clone(), cargo_bin.clone()),
            workspace
        );
        assert_eq!(
            preferred_driver_candidate(None, None, path.clone(), cargo_bin.clone()),
            path
        );
        assert_eq!(
            preferred_driver_candidate(None, None, None, cargo_bin.clone()),
            cargo_bin
        );
    }
}
