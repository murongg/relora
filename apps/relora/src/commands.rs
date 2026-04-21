use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;

use crate::{
    config::{CliCommand, default_connection_store_path, default_saved_sql_store_path},
    drivers::driver_path_statuses,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathsReport {
    pub app_name: &'static str,
    pub version: &'static str,
    pub connection_store_path: PathBuf,
    pub saved_sql_store_path: PathBuf,
    pub drivers: Vec<DriverPathReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriverPathReport {
    pub kind: &'static str,
    pub display_name: &'static str,
    pub binary: &'static str,
    pub override_env: String,
    pub resolved_path: Option<PathBuf>,
}

pub fn run(command: CliCommand) -> Result<()> {
    match command {
        CliCommand::Paths { json } => run_paths(json),
    }
}

pub fn build_paths_report() -> PathsReport {
    build_paths_report_with_store_paths(
        default_connection_store_path(),
        default_saved_sql_store_path(),
    )
}

pub fn build_paths_report_with_store_path(connection_store_path: PathBuf) -> PathsReport {
    build_paths_report_with_store_paths(
        connection_store_path.clone(),
        crate::config::saved_sql_store_path_for_connection_store(&connection_store_path),
    )
}

pub fn build_paths_report_with_store_paths(
    connection_store_path: PathBuf,
    saved_sql_store_path: PathBuf,
) -> PathsReport {
    let drivers = driver_path_statuses()
        .into_iter()
        .map(|status| DriverPathReport {
            kind: status.kind,
            display_name: status.display_name,
            binary: status.binary,
            override_env: status.override_env,
            resolved_path: status.resolved_path,
        })
        .collect();

    PathsReport {
        app_name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        connection_store_path,
        saved_sql_store_path,
        drivers,
    }
}

fn run_paths(json: bool) -> Result<()> {
    let report = build_paths_report();
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_human_paths_report(&report);
    }
    Ok(())
}

fn print_human_paths_report(report: &PathsReport) {
    println!("{} {}", report.app_name, report.version);
    println!(
        "Connection store: {}",
        report.connection_store_path.display()
    );
    println!("Saved SQL store: {}", report.saved_sql_store_path.display());
    println!("Driver binaries:");
    for driver in &report.drivers {
        let resolved = driver
            .resolved_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not found".to_string());
        println!(
            "- {}: {} ({}) -> {}",
            driver.display_name, driver.binary, driver.override_env, resolved
        );
    }
}
