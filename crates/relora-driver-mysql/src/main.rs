use std::io;

use anyhow::Result;
use clap::{Parser, Subcommand};
use relora_core::db::{DatabaseDriver, DbObjectKind, DbObjectRef};
use relora_driver_mysql::MySqlDriver;

#[derive(Debug, Parser)]
#[command(
    name = "relora-driver-mysql",
    version,
    about = "Relora external MySQL/MariaDB driver sidecar."
)]
struct Cli {
    #[arg(long)]
    url: String,

    #[command(subcommand)]
    command: DriverCommand,
}

#[derive(Debug, Subcommand)]
enum DriverCommand {
    Capabilities,
    Catalog,
    CatalogSummary,
    SchemaObjects {
        #[arg(long)]
        database: String,

        #[arg(long)]
        schema: String,

        #[arg(long, value_parser = parse_object_kind)]
        kind: Option<DbObjectKind>,
    },
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
    let mut driver = MySqlDriver::connect(&cli.url)?;
    let stdout = io::stdout();
    let mut output = stdout.lock();

    match cli.command {
        DriverCommand::Capabilities => {
            serde_json::to_writer(&mut output, &driver.capabilities())?;
        }
        DriverCommand::Catalog => serde_json::to_writer(&mut output, &driver.load_catalog()?)?,
        DriverCommand::CatalogSummary => {
            serde_json::to_writer(&mut output, &driver.load_catalog_summary()?)?
        }
        DriverCommand::SchemaObjects {
            database,
            schema,
            kind,
        } => {
            let objects = if let Some(kind) = kind {
                driver.load_schema_objects_of_kind(&database, &schema, kind)?
            } else {
                driver.load_schema_objects(&database, &schema)?
            };
            serde_json::to_writer(&mut output, &objects)?;
        }
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

fn parse_object_kind(value: &str) -> Result<DbObjectKind, String> {
    DbObjectKind::from_wire_name(value).ok_or_else(|| format!("unsupported object kind: {value}"))
}
