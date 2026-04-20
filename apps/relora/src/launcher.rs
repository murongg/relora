use std::{collections::BTreeSet, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use relora_core::db::DatabaseKind;

use crate::{
    config::{ConnectionConfig, save_saved_connections_to_path},
    drivers,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherAction {
    NextConnection,
    PreviousConnection,
    ToggleMarkedConnection,
    OpenCreateConnectionForm,
    OpenEditConnectionForm,
    DeleteSelectedConnection,
    ConfirmDeleteConnection,
    CancelDeleteConnection,
    SwitchFormField,
    PreviousFormField,
    NextFormDriver,
    PreviousFormDriver,
    SubmitConnectionForm,
    CancelConnectionForm,
    LaunchSelectedConnections,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherOutcome {
    Stay,
    Launch(Vec<ConnectionConfig>),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherFormField {
    Name,
    Driver,
    Host,
    Port,
    Database,
    Username,
    Password,
    Url,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherFormSnapshot {
    pub name: String,
    pub driver: LauncherDatabaseKind,
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub field: LauncherFormField,
    pub editing_existing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherDatabaseKind {
    Postgres,
    MySql,
    Sqlite,
}

pub struct LauncherApp {
    connections: Vec<ConnectionConfig>,
    marked_indexes: BTreeSet<usize>,
    selected_index: usize,
    form: Option<ConnectionFormState>,
    pending_missing_driver: Option<MissingDriverPrompt>,
    pending_delete_index: Option<usize>,
    status: Option<String>,
    store_path: PathBuf,
    preview_limit: usize,
}

struct ConnectionFormState {
    editing_index: Option<usize>,
    name: String,
    driver: LauncherDatabaseKind,
    host: String,
    port: String,
    database: String,
    username: String,
    password: String,
    url: String,
    field: LauncherFormField,
}

struct MissingDriverPrompt {
    kind: DatabaseKind,
    display_name: String,
    binary: String,
    env_var: String,
}

pub struct LauncherView<'a> {
    pub connections: &'a [ConnectionConfig],
    pub selected_index: usize,
    pub marked_indexes: &'a BTreeSet<usize>,
    pub status: Option<&'a str>,
    pub form: Option<LauncherFormView<'a>>,
    pub delete_confirmation: Option<LauncherDeleteConfirmationView<'a>>,
    pub missing_driver: Option<LauncherMissingDriverView<'a>>,
}

pub struct LauncherDeleteConfirmationView<'a> {
    pub name: &'a str,
    pub url: &'a str,
}

pub struct LauncherMissingDriverView<'a> {
    pub display_name: &'a str,
    pub binary: &'a str,
    pub env_var: &'a str,
}

pub struct LauncherFormView<'a> {
    pub name: &'a str,
    pub driver: LauncherDatabaseKind,
    pub host: &'a str,
    pub port: &'a str,
    pub database: &'a str,
    pub username: &'a str,
    pub password: &'a str,
    pub url: &'a str,
    pub field: LauncherFormField,
    pub editing_existing: bool,
    pub status: Option<&'a str>,
}

impl LauncherApp {
    pub fn new(connections: Vec<ConnectionConfig>, store_path: PathBuf) -> Self {
        Self::with_preview_limit(connections, store_path, 100)
    }

    pub fn with_preview_limit(
        connections: Vec<ConnectionConfig>,
        store_path: PathBuf,
        preview_limit: usize,
    ) -> Self {
        Self {
            connections,
            marked_indexes: BTreeSet::new(),
            selected_index: 0,
            form: None,
            pending_missing_driver: None,
            pending_delete_index: None,
            status: None,
            store_path,
            preview_limit: preview_limit.max(1),
        }
    }

    pub fn apply_action(&mut self, action: LauncherAction) -> Result<LauncherOutcome> {
        match action {
            LauncherAction::NextConnection => {
                self.pending_delete_index = None;
                self.move_selection(1);
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::PreviousConnection => {
                self.pending_delete_index = None;
                self.move_selection(-1);
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::ToggleMarkedConnection => {
                self.pending_delete_index = None;
                self.toggle_marked_connection()?;
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::OpenCreateConnectionForm => {
                self.pending_missing_driver = None;
                self.pending_delete_index = None;
                self.form = Some(ConnectionFormState::new(None, None));
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::OpenEditConnectionForm => {
                self.pending_delete_index = None;
                let connection = self
                    .selected_connection()
                    .cloned()
                    .ok_or_else(|| anyhow!("select a connection before editing"))?;
                self.form = Some(ConnectionFormState::new(
                    Some(self.selected_index),
                    Some(connection),
                ));
                self.pending_missing_driver = None;
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::DeleteSelectedConnection => {
                self.prompt_delete_selected_connection()?;
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::ConfirmDeleteConnection => {
                self.confirm_delete_connection()?;
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::CancelDeleteConnection => {
                self.cancel_delete_connection();
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::SwitchFormField => {
                let form = self
                    .form
                    .as_mut()
                    .ok_or_else(|| anyhow!("connection form is not open"))?;
                form.switch_field();
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::PreviousFormField => {
                let form = self
                    .form
                    .as_mut()
                    .ok_or_else(|| anyhow!("connection form is not open"))?;
                form.previous_field();
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::NextFormDriver => {
                let form = self
                    .form
                    .as_mut()
                    .ok_or_else(|| anyhow!("connection form is not open"))?;
                form.cycle_driver_next();
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::PreviousFormDriver => {
                let form = self
                    .form
                    .as_mut()
                    .ok_or_else(|| anyhow!("connection form is not open"))?;
                form.cycle_driver_previous();
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::SubmitConnectionForm => {
                self.pending_delete_index = None;
                self.submit_connection_form()?;
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::CancelConnectionForm => {
                self.form = None;
                self.pending_missing_driver = None;
                self.pending_delete_index = None;
                Ok(LauncherOutcome::Stay)
            }
            LauncherAction::LaunchSelectedConnections => {
                self.pending_delete_index = None;
                let targets = self.launch_targets()?;
                self.status = Some(format!("Launching {} connection(s)...", targets.len()));
                Ok(LauncherOutcome::Launch(targets))
            }
            LauncherAction::Quit => Ok(LauncherOutcome::Quit),
        }
    }

    pub fn insert_form_char(&mut self, ch: char) -> Result<()> {
        let form = self
            .form
            .as_mut()
            .ok_or_else(|| anyhow!("connection form is not open"))?;
        self.pending_missing_driver = None;
        form.insert_char(ch);
        Ok(())
    }

    pub fn backspace_form(&mut self) -> Result<()> {
        let form = self
            .form
            .as_mut()
            .ok_or_else(|| anyhow!("connection form is not open"))?;
        self.pending_missing_driver = None;
        form.backspace();
        Ok(())
    }

    pub fn connections(&self) -> &[ConnectionConfig] {
        &self.connections
    }

    pub fn clone_for_workspace_return(&self) -> Self {
        Self::with_preview_limit(
            self.connections.clone(),
            self.store_path.clone(),
            self.preview_limit,
        )
    }

    pub fn marked_connection_count(&self) -> usize {
        self.marked_indexes.len()
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
            .min(self.connections.len().saturating_sub(1))
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = Some(status.into());
    }

    pub fn connection_form_config(&self) -> Result<ConnectionConfig> {
        self.form
            .as_ref()
            .ok_or_else(|| anyhow!("connection form is not open"))?
            .connection()
    }

    pub fn prompt_missing_driver(&mut self, kind: DatabaseKind, display_name: &str, binary: &str) {
        let env_var = drivers::driver_override_env_var(binary);
        self.pending_missing_driver = Some(MissingDriverPrompt {
            kind,
            display_name: display_name.to_string(),
            binary: binary.to_string(),
            env_var: env_var.clone(),
        });
        self.status = Some(format!(
            "{display_name} driver is missing. Install the full Relora bundle, put `{binary}` on PATH, or set `{env_var}`."
        ));
    }

    pub fn pending_missing_driver(&self) -> Option<DatabaseKind> {
        self.pending_missing_driver
            .as_ref()
            .map(|prompt| prompt.kind)
    }

    pub fn pending_delete_connection_name(&self) -> Option<&str> {
        self.pending_delete_index
            .and_then(|index| self.connections.get(index))
            .map(|connection| connection.name.as_str())
    }

    pub fn cancel_missing_driver_prompt(&mut self) {
        self.pending_missing_driver = None;
        self.status = Some("Driver prompt closed.".to_string());
    }

    pub fn form_snapshot(&self) -> Option<LauncherFormSnapshot> {
        let form = self.form.as_ref()?;
        Some(LauncherFormSnapshot {
            name: form.name.clone(),
            driver: form.driver,
            host: form.host.clone(),
            port: form.port.clone(),
            database: form.database.clone(),
            username: form.username.clone(),
            password: form.password.clone(),
            url: form.url.clone(),
            field: form.field,
            editing_existing: form.editing_index.is_some(),
        })
    }

    pub fn view(&self) -> LauncherView<'_> {
        LauncherView {
            connections: &self.connections,
            selected_index: self.selected_index(),
            marked_indexes: &self.marked_indexes,
            status: self.status(),
            form: self.form.as_ref().map(|form| LauncherFormView {
                name: form.name.as_str(),
                driver: form.driver,
                host: form.host.as_str(),
                port: form.port.as_str(),
                database: form.database.as_str(),
                username: form.username.as_str(),
                password: form.password.as_str(),
                url: form.url.as_str(),
                field: form.field,
                editing_existing: form.editing_index.is_some(),
                status: self.status(),
            }),
            delete_confirmation: self.pending_delete_index.and_then(|index| {
                self.connections
                    .get(index)
                    .map(|connection| LauncherDeleteConfirmationView {
                        name: connection.name.as_str(),
                        url: connection.url.as_str(),
                    })
            }),
            missing_driver: self.pending_missing_driver.as_ref().map(|prompt| {
                LauncherMissingDriverView {
                    display_name: prompt.display_name.as_str(),
                    binary: prompt.binary.as_str(),
                    env_var: prompt.env_var.as_str(),
                }
            }),
        }
    }

    pub fn preview_limit(&self) -> usize {
        self.preview_limit
    }

    fn move_selection(&mut self, delta: isize) {
        if self.connections.is_empty() {
            self.selected_index = 0;
            return;
        }

        if delta.is_negative() {
            self.selected_index = self.selected_index.saturating_sub(delta.unsigned_abs());
        } else {
            self.selected_index = self.selected_index.saturating_add(delta as usize);
        }
        self.selected_index = self.selected_index.min(self.connections.len() - 1);
    }

    fn toggle_marked_connection(&mut self) -> Result<()> {
        if self.connections.is_empty() {
            return Err(anyhow!("no saved connections are available"));
        }

        let index = self.selected_index();
        if !self.marked_indexes.insert(index) {
            self.marked_indexes.remove(&index);
        }
        self.status = Some(format!(
            "{} connection(s) selected for launch.",
            self.marked_indexes.len()
        ));
        Ok(())
    }

    fn prompt_delete_selected_connection(&mut self) -> Result<()> {
        if self.connections.is_empty() {
            return Err(anyhow!("no saved connections are available"));
        }

        let index = self.selected_index();
        let name = self.connections[index].name.clone();
        self.pending_delete_index = Some(index);
        self.status = Some(format!(
            "Delete connection `{name}`? Press y to delete, n or Esc to cancel."
        ));
        Ok(())
    }

    fn confirm_delete_connection(&mut self) -> Result<()> {
        let index = self
            .pending_delete_index
            .take()
            .ok_or_else(|| anyhow!("no connection delete confirmation is pending"))?;
        self.delete_connection_at(index)
    }

    fn cancel_delete_connection(&mut self) {
        let name = self
            .pending_delete_index
            .take()
            .and_then(|index| self.connections.get(index))
            .map(|connection| connection.name.clone());
        self.status = Some(match name {
            Some(name) => format!("Delete canceled for `{name}`."),
            None => "Delete canceled.".to_string(),
        });
    }

    fn delete_connection_at(&mut self, index: usize) -> Result<()> {
        if index >= self.connections.len() {
            return Err(anyhow!("pending connection is no longer available"));
        }

        let removed = self.connections.remove(index);
        self.marked_indexes = self
            .marked_indexes
            .iter()
            .filter_map(|marked| match marked.cmp(&index) {
                std::cmp::Ordering::Less => Some(*marked),
                std::cmp::Ordering::Equal => None,
                std::cmp::Ordering::Greater => Some(marked - 1),
            })
            .collect();
        self.persist_connections()?;
        if self.connections.is_empty() {
            self.selected_index = 0;
        } else {
            self.selected_index = self.selected_index.min(self.connections.len() - 1);
        }
        self.status = Some(format!("Deleted connection `{}`.", removed.name));
        Ok(())
    }

    fn submit_connection_form(&mut self) -> Result<()> {
        let form = self
            .form
            .as_ref()
            .ok_or_else(|| anyhow!("connection form is not open"))?;
        let connection = form.connection()?;
        let editing_index = form.editing_index;

        if self
            .connections
            .iter()
            .enumerate()
            .any(|(index, existing)| {
                existing.name == connection.name && Some(index) != editing_index
            })
        {
            return Err(anyhow!(
                "a saved connection named `{}` already exists",
                connection.name
            ));
        }

        self.form = None;
        let saved_message = if let Some(index) = editing_index {
            self.connections[index] = connection.clone();
            self.selected_index = index;
            format!("Updated connection `{}`.", connection.name)
        } else {
            self.connections.push(connection.clone());
            self.selected_index = self.connections.len() - 1;
            format!("Saved connection `{}`.", connection.name)
        };
        self.persist_connections()?;
        self.status = Some(saved_message);
        Ok(())
    }

    fn launch_targets(&self) -> Result<Vec<ConnectionConfig>> {
        if !self.marked_indexes.is_empty() {
            return Ok(self
                .marked_indexes
                .iter()
                .filter_map(|index| self.connections.get(*index).cloned())
                .collect());
        }

        self.selected_connection()
            .cloned()
            .map(|connection| vec![connection])
            .ok_or_else(|| anyhow!("no saved connections are available"))
    }

    fn persist_connections(&self) -> Result<()> {
        save_saved_connections_to_path(&self.store_path, &self.connections)?;
        Ok(())
    }

    fn selected_connection(&self) -> Option<&ConnectionConfig> {
        self.connections.get(self.selected_index())
    }
}

impl ConnectionFormState {
    fn new(editing_index: Option<usize>, connection: Option<ConnectionConfig>) -> Self {
        let (name, url) = connection
            .map(|connection| (connection.name, connection.url))
            .unwrap_or_else(|| (String::new(), String::new()));
        let parsed = StructuredConnectionFields::from_url(&url)
            .unwrap_or_else(StructuredConnectionFields::postgres_defaults);
        Self {
            editing_index,
            name,
            driver: parsed.driver,
            host: parsed.host,
            port: parsed.port,
            database: parsed.database,
            username: parsed.username,
            password: parsed.password,
            url,
            field: LauncherFormField::Name,
        }
    }

    fn switch_field(&mut self) {
        self.field = match self.field {
            LauncherFormField::Name => LauncherFormField::Driver,
            LauncherFormField::Driver => LauncherFormField::Host,
            LauncherFormField::Host => LauncherFormField::Port,
            LauncherFormField::Port => LauncherFormField::Database,
            LauncherFormField::Database => LauncherFormField::Username,
            LauncherFormField::Username => LauncherFormField::Password,
            LauncherFormField::Password => LauncherFormField::Url,
            LauncherFormField::Url => LauncherFormField::Name,
        };
    }

    fn previous_field(&mut self) {
        self.field = match self.field {
            LauncherFormField::Name => LauncherFormField::Url,
            LauncherFormField::Driver => LauncherFormField::Name,
            LauncherFormField::Host => LauncherFormField::Driver,
            LauncherFormField::Port => LauncherFormField::Host,
            LauncherFormField::Database => LauncherFormField::Port,
            LauncherFormField::Username => LauncherFormField::Database,
            LauncherFormField::Password => LauncherFormField::Username,
            LauncherFormField::Url => LauncherFormField::Password,
        };
    }

    fn cycle_driver_next(&mut self) {
        if self.field == LauncherFormField::Driver {
            self.clear_url_override_for_structured_edit();
            self.set_driver(self.driver.next());
        }
    }

    fn cycle_driver_previous(&mut self) {
        if self.field == LauncherFormField::Driver {
            self.clear_url_override_for_structured_edit();
            self.set_driver(self.driver.previous());
        }
    }

    fn insert_char(&mut self, ch: char) {
        if self.field == LauncherFormField::Driver {
            self.update_driver_from_char(ch);
            return;
        }
        self.clear_url_override_for_structured_edit();
        self.current_field_mut().push(ch);
    }

    fn backspace(&mut self) {
        if self.field == LauncherFormField::Driver {
            return;
        }
        self.clear_url_override_for_structured_edit();
        self.current_field_mut().pop();
    }

    fn update_driver_from_char(&mut self, ch: char) {
        let next = match ch.to_ascii_lowercase() {
            'p' => Some(LauncherDatabaseKind::Postgres),
            'm' => Some(LauncherDatabaseKind::MySql),
            's' => Some(LauncherDatabaseKind::Sqlite),
            ' ' => Some(self.driver.next()),
            _ => None,
        };
        if let Some(next) = next {
            self.clear_url_override_for_structured_edit();
            self.set_driver(next);
        }
    }

    fn set_driver(&mut self, next: LauncherDatabaseKind) {
        self.driver = next;
        if self.port.trim().is_empty() || ["5432", "3306"].contains(&self.port.trim()) {
            self.port = next.default_port().to_string();
        }
        if next == LauncherDatabaseKind::Sqlite {
            self.host.clear();
            self.username.clear();
            self.password.clear();
        } else if self.host.trim().is_empty() {
            self.host = "localhost".to_string();
        }
    }

    fn current_field_mut(&mut self) -> &mut String {
        match self.field {
            LauncherFormField::Name => &mut self.name,
            LauncherFormField::Driver => unreachable!("driver field is not a text field"),
            LauncherFormField::Host => &mut self.host,
            LauncherFormField::Port => &mut self.port,
            LauncherFormField::Database => &mut self.database,
            LauncherFormField::Username => &mut self.username,
            LauncherFormField::Password => &mut self.password,
            LauncherFormField::Url => &mut self.url,
        }
    }

    fn connection(&self) -> Result<ConnectionConfig> {
        let name = self.name.trim();
        let url = self.connection_url()?;
        if name.is_empty() {
            return Err(anyhow!("connection name cannot be empty"));
        }
        DatabaseKind::from_url(&url)?;
        Ok(ConnectionConfig {
            name: name.to_string(),
            url,
        })
    }

    fn connection_url(&self) -> Result<String> {
        let url = self.url.trim();
        if !url.is_empty() {
            return Ok(url.to_string());
        }

        self.structured_url()
    }

    fn clear_url_override_for_structured_edit(&mut self) {
        if matches!(
            self.field,
            LauncherFormField::Driver
                | LauncherFormField::Host
                | LauncherFormField::Port
                | LauncherFormField::Database
                | LauncherFormField::Username
                | LauncherFormField::Password
        ) {
            self.url.clear();
        }
    }

    fn structured_url(&self) -> Result<String> {
        match self.driver {
            LauncherDatabaseKind::Postgres | LauncherDatabaseKind::MySql => {
                let host = self.host.trim();
                if host.is_empty() {
                    return Err(anyhow!("connection host cannot be empty"));
                }
                let scheme = self.driver.scheme();
                let port = self.port.trim();
                let base = if port.is_empty() {
                    format!("{scheme}://{host}")
                } else {
                    format!("{scheme}://{host}:{port}")
                };
                let mut url = url::Url::parse(&base)
                    .with_context(|| format!("invalid connection host or port: {base}"))?;
                if !self.username.trim().is_empty() {
                    url.set_username(self.username.trim())
                        .map_err(|_| anyhow!("invalid connection username"))?;
                    if !self.password.is_empty() {
                        url.set_password(Some(&self.password))
                            .map_err(|_| anyhow!("invalid connection password"))?;
                    }
                }
                let database = self.database.trim();
                if database.is_empty() {
                    url.set_path("/");
                } else {
                    url.set_path(&format!("/{database}"));
                }
                Ok(url.to_string())
            }
            LauncherDatabaseKind::Sqlite => {
                let path = self.database.trim();
                if path.is_empty() {
                    return Err(anyhow!("SQLite database path cannot be empty"));
                }
                if path == ":memory:" {
                    Ok("sqlite::memory:".to_string())
                } else {
                    Ok(format!("sqlite://{path}"))
                }
            }
        }
    }
}

impl LauncherDatabaseKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Postgres => "PostgreSQL",
            Self::MySql => "MySQL/MariaDB",
            Self::Sqlite => "SQLite",
        }
    }

    fn scheme(self) -> &'static str {
        match self {
            Self::Postgres => "postgresql",
            Self::MySql => "mysql",
            Self::Sqlite => "sqlite",
        }
    }

    fn default_port(self) -> &'static str {
        match self {
            Self::Postgres => "5432",
            Self::MySql => "3306",
            Self::Sqlite => "",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Postgres => Self::MySql,
            Self::MySql => Self::Sqlite,
            Self::Sqlite => Self::Postgres,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Postgres => Self::Sqlite,
            Self::MySql => Self::Postgres,
            Self::Sqlite => Self::MySql,
        }
    }
}

struct StructuredConnectionFields {
    driver: LauncherDatabaseKind,
    host: String,
    port: String,
    database: String,
    username: String,
    password: String,
}

impl StructuredConnectionFields {
    fn postgres_defaults() -> Self {
        Self {
            driver: LauncherDatabaseKind::Postgres,
            host: "localhost".to_string(),
            port: LauncherDatabaseKind::Postgres.default_port().to_string(),
            database: "postgres".to_string(),
            username: String::new(),
            password: String::new(),
        }
    }

    fn from_url(url: &str) -> Option<Self> {
        let parsed = url::Url::parse(url).ok()?;
        let driver = match parsed.scheme() {
            "postgres" | "postgresql" => LauncherDatabaseKind::Postgres,
            "mysql" | "mariadb" => LauncherDatabaseKind::MySql,
            "sqlite" | "sqlite3" => LauncherDatabaseKind::Sqlite,
            _ => return None,
        };
        if driver == LauncherDatabaseKind::Sqlite {
            return Some(Self {
                driver,
                host: String::new(),
                port: String::new(),
                database: parsed.path().to_string(),
                username: String::new(),
                password: String::new(),
            });
        }

        Some(Self {
            driver,
            host: parsed.host_str().unwrap_or("localhost").to_string(),
            port: parsed
                .port()
                .map(|port| port.to_string())
                .unwrap_or_else(|| driver.default_port().to_string()),
            database: parsed.path().trim_start_matches('/').to_string(),
            username: parsed.username().to_string(),
            password: parsed.password().unwrap_or_default().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConnectionConfig, load_saved_connections_from_path};
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_store_path(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("relora-launcher-{test_name}-{nanos}.json"))
    }

    fn move_form_to(launcher: &mut LauncherApp, field: LauncherFormField) -> Result<()> {
        for _ in 0..8 {
            if launcher.form_snapshot().expect("form should be open").field == field {
                return Ok(());
            }
            launcher.apply_action(LauncherAction::SwitchFormField)?;
        }
        Err(anyhow!("failed to move launcher form to {field:?}"))
    }

    #[test]
    fn launcher_marks_connections_and_launches_marked_set() -> Result<()> {
        let store_path = temp_store_path("marks");
        let mut launcher = LauncherApp::new(
            vec![
                ConnectionConfig {
                    name: "pg".to_string(),
                    url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
                },
                ConnectionConfig {
                    name: "analytics".to_string(),
                    url: "postgresql://postgres:postgres@localhost/analytics".to_string(),
                },
            ],
            store_path,
        );

        launcher.apply_action(LauncherAction::ToggleMarkedConnection)?;
        launcher.apply_action(LauncherAction::NextConnection)?;
        launcher.apply_action(LauncherAction::ToggleMarkedConnection)?;

        assert_eq!(launcher.marked_connection_count(), 2);
        assert_eq!(
            launcher.apply_action(LauncherAction::LaunchSelectedConnections)?,
            LauncherOutcome::Launch(vec![
                ConnectionConfig {
                    name: "pg".to_string(),
                    url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
                },
                ConnectionConfig {
                    name: "analytics".to_string(),
                    url: "postgresql://postgres:postgres@localhost/analytics".to_string(),
                },
            ])
        );
        Ok(())
    }

    #[test]
    fn launcher_can_create_edit_and_delete_connections_persisted_to_store() -> Result<()> {
        let store_path = temp_store_path("crud");
        let mut launcher = LauncherApp::new(Vec::new(), store_path.clone());

        launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;
        for ch in "pg".chars() {
            launcher.insert_form_char(ch)?;
        }
        move_form_to(&mut launcher, LauncherFormField::Url)?;
        for ch in "postgresql://postgres:postgres@localhost/postgres".chars() {
            launcher.insert_form_char(ch)?;
        }
        launcher.apply_action(LauncherAction::SubmitConnectionForm)?;

        assert_eq!(launcher.connections().len(), 1);
        assert_eq!(launcher.connections()[0].name, "pg");
        assert_eq!(
            load_saved_connections_from_path(&store_path)?,
            vec![ConnectionConfig {
                name: "pg".to_string(),
                url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
            }]
        );

        launcher.apply_action(LauncherAction::OpenEditConnectionForm)?;
        for _ in 0.."pg".chars().count() {
            launcher.backspace_form()?;
        }
        for ch in "primary".chars() {
            launcher.insert_form_char(ch)?;
        }
        launcher.apply_action(LauncherAction::SubmitConnectionForm)?;

        assert_eq!(launcher.connections()[0].name, "primary");

        launcher.apply_action(LauncherAction::DeleteSelectedConnection)?;
        assert_eq!(launcher.connections().len(), 1);
        assert!(
            launcher
                .status()
                .expect("delete should ask for confirmation")
                .contains("Press y to delete")
        );
        assert_eq!(
            load_saved_connections_from_path(&store_path)?,
            vec![ConnectionConfig {
                name: "primary".to_string(),
                url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
            }]
        );

        launcher.apply_action(LauncherAction::ConfirmDeleteConnection)?;
        assert!(launcher.connections().is_empty());
        assert!(load_saved_connections_from_path(&store_path)?.is_empty());
        Ok(())
    }

    #[test]
    fn launcher_can_cancel_delete_confirmation() -> Result<()> {
        let mut launcher = LauncherApp::new(
            vec![ConnectionConfig {
                name: "pg".to_string(),
                url: "postgresql://postgres:postgres@localhost/postgres".to_string(),
            }],
            temp_store_path("delete-cancel"),
        );

        launcher.apply_action(LauncherAction::DeleteSelectedConnection)?;
        assert_eq!(launcher.pending_delete_connection_name(), Some("pg"));

        launcher.apply_action(LauncherAction::CancelDeleteConnection)?;
        assert_eq!(launcher.pending_delete_connection_name(), None);
        assert_eq!(launcher.connections().len(), 1);
        assert!(
            launcher
                .status()
                .expect("cancel should report status")
                .contains("canceled")
        );
        Ok(())
    }

    #[test]
    fn launcher_can_build_connection_url_from_structured_fields() -> Result<()> {
        let store_path = temp_store_path("structured");
        let mut launcher = LauncherApp::new(Vec::new(), store_path.clone());

        launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;
        for ch in "analytics".chars() {
            launcher.insert_form_char(ch)?;
        }
        move_form_to(&mut launcher, LauncherFormField::Driver)?;
        launcher.insert_form_char('p')?;
        move_form_to(&mut launcher, LauncherFormField::Database)?;
        for _ in 0.."postgres".len() {
            launcher.backspace_form()?;
        }
        for ch in "warehouse".chars() {
            launcher.insert_form_char(ch)?;
        }
        move_form_to(&mut launcher, LauncherFormField::Username)?;
        for ch in "alice".chars() {
            launcher.insert_form_char(ch)?;
        }
        move_form_to(&mut launcher, LauncherFormField::Password)?;
        for ch in "secret".chars() {
            launcher.insert_form_char(ch)?;
        }
        launcher.apply_action(LauncherAction::SubmitConnectionForm)?;

        assert_eq!(
            load_saved_connections_from_path(&store_path)?,
            vec![ConnectionConfig {
                name: "analytics".to_string(),
                url: "postgresql://alice:secret@localhost:5432/warehouse".to_string(),
            }]
        );
        Ok(())
    }

    #[test]
    fn launcher_allows_empty_database_in_structured_server_connections() -> Result<()> {
        let store_path = temp_store_path("empty-database");
        let mut launcher = LauncherApp::new(Vec::new(), store_path.clone());

        launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;
        for ch in "server".chars() {
            launcher.insert_form_char(ch)?;
        }
        move_form_to(&mut launcher, LauncherFormField::Database)?;
        for _ in 0.."postgres".len() {
            launcher.backspace_form()?;
        }
        launcher.apply_action(LauncherAction::SubmitConnectionForm)?;

        assert_eq!(
            load_saved_connections_from_path(&store_path)?,
            vec![ConnectionConfig {
                name: "server".to_string(),
                url: "postgresql://localhost:5432/".to_string(),
            }]
        );
        Ok(())
    }

    #[test]
    fn launcher_form_navigation_supports_previous_field_and_driver_cycle() -> Result<()> {
        let mut launcher = LauncherApp::new(Vec::new(), temp_store_path("form-navigation"));

        launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;
        launcher.apply_action(LauncherAction::SwitchFormField)?;
        assert_eq!(
            launcher.form_snapshot().expect("form should be open").field,
            LauncherFormField::Driver
        );

        launcher.apply_action(LauncherAction::PreviousFormField)?;
        assert_eq!(
            launcher.form_snapshot().expect("form should be open").field,
            LauncherFormField::Name
        );

        launcher.apply_action(LauncherAction::PreviousFormField)?;
        assert_eq!(
            launcher.form_snapshot().expect("form should be open").field,
            LauncherFormField::Url
        );

        move_form_to(&mut launcher, LauncherFormField::Driver)?;
        launcher.apply_action(LauncherAction::NextFormDriver)?;
        assert_eq!(
            launcher
                .form_snapshot()
                .expect("form should be open")
                .driver,
            LauncherDatabaseKind::MySql
        );
        launcher.apply_action(LauncherAction::PreviousFormDriver)?;
        assert_eq!(
            launcher
                .form_snapshot()
                .expect("form should be open")
                .driver,
            LauncherDatabaseKind::Postgres
        );
        Ok(())
    }

    #[test]
    fn launcher_tracks_missing_driver_prompt() {
        let mut launcher = LauncherApp::new(Vec::new(), temp_store_path("missing-driver"));

        launcher.prompt_missing_driver(
            DatabaseKind::Postgres,
            "PostgreSQL",
            "relora-driver-postgres",
        );
        assert_eq!(
            launcher.pending_missing_driver(),
            Some(DatabaseKind::Postgres)
        );
        assert!(
            launcher
                .status()
                .expect("prompt should set status")
                .contains("RELORA_POSTGRES_DRIVER")
        );

        launcher.cancel_missing_driver_prompt();
        assert_eq!(launcher.pending_missing_driver(), None);
    }

    #[test]
    fn launcher_edit_form_parses_existing_url_into_structured_fields() -> Result<()> {
        let mut launcher = LauncherApp::new(
            vec![ConnectionConfig {
                name: "mysql".to_string(),
                url: "mysql://root:secret@db.local:3307/app".to_string(),
            }],
            temp_store_path("parse-existing"),
        );

        launcher.apply_action(LauncherAction::OpenEditConnectionForm)?;
        let form = launcher
            .form_snapshot()
            .expect("edit form should be available");

        assert_eq!(form.driver, LauncherDatabaseKind::MySql);
        assert_eq!(form.host, "db.local");
        assert_eq!(form.port, "3307");
        assert_eq!(form.database, "app");
        assert_eq!(form.username, "root");
        assert_eq!(form.password, "secret");
        Ok(())
    }

    #[test]
    fn launcher_edit_form_prefers_structured_fields_over_stale_url_override() -> Result<()> {
        let mut launcher = LauncherApp::new(
            vec![ConnectionConfig {
                name: "mysql".to_string(),
                url: "mysql://root:secret@db.local:3307/app".to_string(),
            }],
            temp_store_path("edit-structured"),
        );

        launcher.apply_action(LauncherAction::OpenEditConnectionForm)?;
        let form = launcher
            .form_snapshot()
            .expect("edit form should be available");
        assert_eq!(form.host, "db.local");

        move_form_to(&mut launcher, LauncherFormField::Host)?;
        for _ in 0.."db.local".chars().count() {
            launcher.backspace_form()?;
        }
        for ch in "db.internal".chars() {
            launcher.insert_form_char(ch)?;
        }

        launcher.apply_action(LauncherAction::SubmitConnectionForm)?;
        assert_eq!(
            launcher.connections()[0].url,
            "mysql://root:secret@db.internal:3307/app"
        );
        Ok(())
    }

    #[test]
    fn launcher_keeps_the_form_open_when_validation_fails() -> Result<()> {
        let mut launcher = LauncherApp::new(Vec::new(), temp_store_path("validation"));

        launcher.apply_action(LauncherAction::OpenCreateConnectionForm)?;
        for ch in "broken".chars() {
            launcher.insert_form_char(ch)?;
        }
        move_form_to(&mut launcher, LauncherFormField::Url)?;
        for ch in "not-a-url".chars() {
            launcher.insert_form_char(ch)?;
        }

        assert!(
            launcher
                .apply_action(LauncherAction::SubmitConnectionForm)
                .is_err()
        );
        assert!(launcher.form_snapshot().is_some());
        Ok(())
    }
}
