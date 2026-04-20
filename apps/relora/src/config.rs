use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use serde_json::{Value, json};

const APP_NAME: &str = "relora";
const CONNECTIONS_ENV: &str = "RELORA_CONNECTIONS";
const DATABASE_URL_ENV: &str = "RELORA_DATABASE_URL";
const CONNECTION_STORE_ENV: &str = "RELORA_CONNECTION_STORE";

#[derive(Debug, Clone, Parser)]
#[command(
    name = "relora",
    version,
    about = "Relora: a terminal database workspace for managing multiple connections and browsing objects."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<CliCommand>,

    #[arg(
        long,
        help = "Single database connection URL. Falls back to RELORA_DATABASE_URL or DATABASE_URL."
    )]
    pub url: Option<String>,

    #[arg(
        long = "connection",
        value_name = "NAME=URL",
        help = "Named database connection. Can be provided multiple times."
    )]
    pub connections: Vec<String>,

    #[arg(
        long,
        default_value_t = 100,
        help = "Maximum rows to fetch for the table preview."
    )]
    pub preview_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum CliCommand {
    Paths {
        #[arg(long, help = "Emit machine-readable JSON output.")]
        json: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub connections: Vec<ConnectionConfig>,
    pub saved_connections: Vec<ConnectionConfig>,
    pub launch_mode: LaunchMode,
    pub connection_store_path: PathBuf,
    pub preview_limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    Launcher,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionConfig {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    InvalidConnection(String),
    InvalidConnectionStore(String),
    ConnectionStoreIo(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConnection(value) => write!(
                f,
                "invalid connection spec `{value}`: expected NAME=DATABASE_URL"
            ),
            Self::InvalidConnectionStore(message) => {
                write!(f, "invalid saved connection store: {message}")
            }
            Self::ConnectionStoreIo(message) => {
                write!(f, "failed to read or write the connection store: {message}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl Cli {
    pub fn into_config(self) -> Result<AppConfig, ConfigError> {
        self.into_config_with_store_path(default_connection_store_path())
    }

    pub fn into_config_with_store_path(
        self,
        connection_store_path: PathBuf,
    ) -> Result<AppConfig, ConfigError> {
        let mut connections = parse_named_connections(self.connections)?;

        if let Ok(value) = std::env::var(CONNECTIONS_ENV) {
            connections.extend(parse_named_connections(split_connection_specs(value))?);
        }

        if connections.is_empty() {
            if let Some(url) = self
                .url
                .filter(|value| !value.trim().is_empty())
                .or_else(|| std::env::var(DATABASE_URL_ENV).ok())
                .or_else(|| std::env::var("DATABASE_URL").ok())
                .filter(|value| !value.trim().is_empty())
            {
                connections.push(ConnectionConfig {
                    name: "default".to_string(),
                    url,
                });
            }
        }

        let saved_connections = load_saved_connections_from_path(&connection_store_path)?;
        let launch_mode = if connections.is_empty() {
            LaunchMode::Launcher
        } else {
            LaunchMode::Workspace
        };

        Ok(AppConfig {
            connections,
            saved_connections,
            launch_mode,
            connection_store_path,
            preview_limit: self.preview_limit.max(1),
        })
    }
}

pub fn load() -> Result<AppConfig, ConfigError> {
    Cli::parse().into_config()
}

fn parse_named_connections<I>(specs: I) -> Result<Vec<ConnectionConfig>, ConfigError>
where
    I: IntoIterator<Item = String>,
{
    specs.into_iter().map(parse_named_connection).collect()
}

fn parse_named_connection(value: String) -> Result<ConnectionConfig, ConfigError> {
    let (name, url) = value
        .split_once('=')
        .ok_or_else(|| ConfigError::InvalidConnection(value.clone()))?;
    let name = name.trim();
    let url = url.trim();

    if name.is_empty() || url.is_empty() {
        return Err(ConfigError::InvalidConnection(value));
    }

    Ok(ConnectionConfig {
        name: name.to_string(),
        url: url.to_string(),
    })
}

pub fn default_connection_store_path() -> PathBuf {
    if let Some(path) = std::env::var_os(CONNECTION_STORE_ENV) {
        return PathBuf::from(path);
    }

    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(path).join(APP_NAME).join("connections.json");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join(APP_NAME)
            .join("connections.json");
    }

    PathBuf::from(".relora-connections.json")
}

pub fn load_saved_connections_from_path(path: &Path) -> Result<Vec<ConnectionConfig>, ConfigError> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| ConfigError::ConnectionStoreIo(error.to_string()))?;
    let json: Value = serde_json::from_str(&content)
        .map_err(|error| ConfigError::InvalidConnectionStore(error.to_string()))?;
    let Some(items) = json.get("connections").and_then(Value::as_array) else {
        return Err(ConfigError::InvalidConnectionStore(
            "expected a top-level `connections` array".to_string(),
        ));
    };

    items
        .iter()
        .map(|item| {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ConfigError::InvalidConnectionStore(
                        "each saved connection must include a non-empty `name`".to_string(),
                    )
                })?;
            let url = item
                .get("url")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ConfigError::InvalidConnectionStore(
                        "each saved connection must include a non-empty `url`".to_string(),
                    )
                })?;

            Ok(ConnectionConfig {
                name: name.to_string(),
                url: url.to_string(),
            })
        })
        .collect()
}

pub fn save_saved_connections_to_path(
    path: &Path,
    connections: &[ConnectionConfig],
) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|error| ConfigError::ConnectionStoreIo(error.to_string()))?;
        }
    }

    let content = serde_json::to_string_pretty(&json!({
        "connections": connections
            .iter()
            .map(|connection| json!({
                "name": connection.name,
                "url": connection.url,
            }))
            .collect::<Vec<_>>()
    }))
    .map_err(|error| ConfigError::InvalidConnectionStore(error.to_string()))?;

    fs::write(path, content).map_err(|error| ConfigError::ConnectionStoreIo(error.to_string()))
}

fn split_connection_specs(value: String) -> impl Iterator<Item = String> {
    value
        .split(['\n', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>()
        .into_iter()
}
