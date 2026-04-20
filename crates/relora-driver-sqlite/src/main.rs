use std::io;

use anyhow::Result;
use clap::{Parser, Subcommand};
use relora_core::db::{DatabaseDriver, DbObjectRef};
use relora_driver_sqlite::SqliteDriver;

#[derive(Debug, Parser)]
#[command(
    name = "relora-driver-sqlite",
    version,
    about = "Relora external SQLite driver sidecar."
)]
struct Cli {
    #[arg(long)]
    url: String,

    #[command(subcommand)]
    command: DriverCommand,
}

#[derive(Debug, Subcommand)]
enum DriverCommand {
    Catalog,
    Preview {
        #[arg(long = "object", value_parser = parse_object)]
        object: DbObjectRef,

        #[arg(long, default_value_t = 100)]
        limit: usize,

        #[arg(long, default_value_t = 0)]
        offset: usize,

        #[arg(long)]
        filter: Option<String>,
    },
    Columns {
        #[arg(long = "object", value_parser = parse_object)]
        object: DbObjectRef,
    },
    Execute {
        #[arg(long)]
        database: Option<String>,

        #[arg(long)]
        sql: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut driver = SqliteDriver::connect(&cli.url)?;
    let stdout = io::stdout();
    let mut output = stdout.lock();

    match cli.command {
        DriverCommand::Catalog => serde_json::to_writer(&mut output, &driver.load_catalog()?)?,
        DriverCommand::Preview {
            object,
            limit,
            offset,
            filter,
        } => {
            let preview = if let Some(filter) = filter {
                driver.load_filtered_preview_page(&object, &filter, limit, offset)?
            } else {
                driver.load_preview_page(&object, limit, offset)?
            };
            serde_json::to_writer(&mut output, &preview)?;
        }
        DriverCommand::Columns { object } => {
            serde_json::to_writer(&mut output, &driver.load_object_columns(&object)?)?;
        }
        DriverCommand::Execute { database, sql } => {
            serde_json::to_writer(&mut output, &driver.execute_sql(database.as_deref(), &sql)?)?;
        }
    }

    Ok(())
}

fn parse_object(value: &str) -> Result<DbObjectRef, serde_json::Error> {
    serde_json::from_str(value)
}
