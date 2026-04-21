use std::{
    collections::HashSet,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use anyhow::{Result, anyhow};
use relora_core::db::{
    CatalogSummary, DatabaseDriver, DbColumn, DbObjectKind, DbObjectRef, SqlExecutionResult,
    TablePreview,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemplateKind {
    Insert,
    Update,
    Delete,
}

pub(crate) struct SessionWorker {
    command_tx: Sender<SessionCommand>,
    event_rx: Receiver<SessionEvent>,
    next_request_id: u64,
}

impl SessionWorker {
    pub fn spawn(driver: Box<dyn DatabaseDriver>) -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        thread::spawn(move || {
            worker_loop(driver, command_rx, event_tx);
        });

        Self {
            command_tx,
            event_rx,
            next_request_id: 1,
        }
    }

    pub fn request_preview(
        &mut self,
        object: DbObjectRef,
        limit: usize,
        offset: usize,
    ) -> Result<u64> {
        let request_id = self.next_request_id();
        self.command_tx
            .send(SessionCommand::LoadPreview {
                request_id,
                object,
                limit,
                offset,
            })
            .map_err(|_| anyhow!("background worker is unavailable"))?;
        Ok(request_id)
    }

    pub fn request_filtered_preview(
        &mut self,
        object: DbObjectRef,
        filter: String,
        limit: usize,
        offset: usize,
    ) -> Result<u64> {
        let request_id = self.next_request_id();
        self.command_tx
            .send(SessionCommand::LoadFilteredPreview {
                request_id,
                object,
                filter,
                limit,
                offset,
            })
            .map_err(|_| anyhow!("background worker is unavailable"))?;
        Ok(request_id)
    }

    pub fn request_refresh(
        &mut self,
        selected_object: Option<DbObjectRef>,
        preview_limit: usize,
        preview_offset: usize,
        preview_filter: Option<String>,
    ) -> Result<u64> {
        let request_id = self.next_request_id();
        self.command_tx
            .send(SessionCommand::RefreshCatalog {
                request_id,
                selected_object,
                preview_limit,
                preview_offset,
                preview_filter,
            })
            .map_err(|_| anyhow!("background worker is unavailable"))?;
        Ok(request_id)
    }

    pub fn request_schema_objects(
        &mut self,
        database: String,
        schema: String,
        kind: DbObjectKind,
    ) -> Result<u64> {
        let request_id = self.next_request_id();
        self.command_tx
            .send(SessionCommand::LoadSchemaObjects {
                request_id,
                database,
                schema,
                kind,
            })
            .map_err(|_| anyhow!("background worker is unavailable"))?;
        Ok(request_id)
    }

    pub fn request_template(&mut self, object: DbObjectRef, kind: TemplateKind) -> Result<u64> {
        let request_id = self.next_request_id();
        self.command_tx
            .send(SessionCommand::LoadColumns {
                request_id,
                object,
                kind,
            })
            .map_err(|_| anyhow!("background worker is unavailable"))?;
        Ok(request_id)
    }

    pub fn request_structure(&mut self, object: DbObjectRef) -> Result<u64> {
        let request_id = self.next_request_id();
        self.command_tx
            .send(SessionCommand::LoadStructureColumns { request_id, object })
            .map_err(|_| anyhow!("background worker is unavailable"))?;
        Ok(request_id)
    }

    pub fn request_sql_execution(&mut self, database: Option<String>, sql: String) -> Result<u64> {
        let request_id = self.next_request_id();
        self.command_tx
            .send(SessionCommand::ExecuteSql {
                request_id,
                database,
                sql,
            })
            .map_err(|_| anyhow!("background worker is unavailable"))?;
        Ok(request_id)
    }

    pub fn cancel_requests(&self, request_ids: Vec<u64>) -> Result<()> {
        if request_ids.is_empty() {
            return Ok(());
        }

        self.command_tx
            .send(SessionCommand::CancelRequests { request_ids })
            .map_err(|_| anyhow!("background worker is unavailable"))
    }

    pub fn try_recv(&self) -> Option<SessionEvent> {
        self.event_rx.try_recv().ok()
    }

    fn next_request_id(&mut self) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        request_id
    }
}

impl Drop for SessionWorker {
    fn drop(&mut self) {
        let _ = self.command_tx.send(SessionCommand::Shutdown);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionCommand {
    LoadPreview {
        request_id: u64,
        object: DbObjectRef,
        limit: usize,
        offset: usize,
    },
    LoadFilteredPreview {
        request_id: u64,
        object: DbObjectRef,
        filter: String,
        limit: usize,
        offset: usize,
    },
    RefreshCatalog {
        request_id: u64,
        selected_object: Option<DbObjectRef>,
        preview_limit: usize,
        preview_offset: usize,
        preview_filter: Option<String>,
    },
    LoadSchemaObjects {
        request_id: u64,
        database: String,
        schema: String,
        kind: DbObjectKind,
    },
    LoadColumns {
        request_id: u64,
        object: DbObjectRef,
        kind: TemplateKind,
    },
    LoadStructureColumns {
        request_id: u64,
        object: DbObjectRef,
    },
    ExecuteSql {
        request_id: u64,
        database: Option<String>,
        sql: String,
    },
    CancelRequests {
        request_ids: Vec<u64>,
    },
    Shutdown,
}

#[derive(Debug)]
pub(crate) struct LoadedObjectGroup {
    pub(crate) database: String,
    pub(crate) schema: String,
    pub(crate) kind: DbObjectKind,
    pub(crate) result: std::result::Result<Vec<DbObjectRef>, String>,
}

#[derive(Debug)]
pub(crate) enum SessionEvent {
    PreviewLoaded {
        request_id: u64,
        object: DbObjectRef,
        offset: usize,
        result: std::result::Result<TablePreview, String>,
    },
    CatalogRefreshed {
        request_id: u64,
        catalog_summary: std::result::Result<CatalogSummary, String>,
        loaded_group: Option<LoadedObjectGroup>,
        preview_target: Option<DbObjectRef>,
        preview_offset: usize,
        preview: Option<std::result::Result<TablePreview, String>>,
    },
    SchemaObjectsLoaded {
        request_id: u64,
        database: String,
        schema: String,
        kind: DbObjectKind,
        result: std::result::Result<Vec<DbObjectRef>, String>,
    },
    ColumnsLoaded {
        request_id: u64,
        object: DbObjectRef,
        kind: TemplateKind,
        result: std::result::Result<Vec<DbColumn>, String>,
    },
    StructureColumnsLoaded {
        request_id: u64,
        object: DbObjectRef,
        result: std::result::Result<Vec<DbColumn>, String>,
    },
    SqlExecuted {
        request_id: u64,
        result: std::result::Result<Vec<SqlExecutionResult>, String>,
    },
}

fn worker_loop(
    mut driver: Box<dyn DatabaseDriver>,
    command_rx: Receiver<SessionCommand>,
    event_tx: Sender<SessionEvent>,
) {
    let mut canceled_request_ids = HashSet::new();

    'outer: while let Ok(first_command) = command_rx.recv() {
        let (commands, should_shutdown) =
            collect_commands(first_command, &command_rx, &mut canceled_request_ids);

        for command in commands {
            let event = match command {
                SessionCommand::LoadPreview {
                    request_id,
                    object,
                    limit,
                    offset,
                } => SessionEvent::PreviewLoaded {
                    request_id,
                    object: object.clone(),
                    offset,
                    result: driver
                        .load_preview_page(&object, limit, offset)
                        .map_err(|error| error.to_string()),
                },
                SessionCommand::LoadFilteredPreview {
                    request_id,
                    object,
                    filter,
                    limit,
                    offset,
                } => SessionEvent::PreviewLoaded {
                    request_id,
                    object: object.clone(),
                    offset,
                    result: driver
                        .load_filtered_preview_page(&object, &filter, limit, offset)
                        .map_err(|error| error.to_string()),
                },
                SessionCommand::RefreshCatalog {
                    request_id,
                    selected_object,
                    preview_limit,
                    preview_offset,
                    preview_filter,
                } => {
                    let catalog_summary = driver
                        .load_catalog_summary()
                        .map_err(|error| error.to_string());
                    let (loaded_group, preview_target, preview) =
                        match (&catalog_summary, selected_object) {
                            (Ok(summary), Some(object))
                                if schema_has_kind(
                                    summary,
                                    &object.database,
                                    &object.schema,
                                    object.kind,
                                ) =>
                            {
                                let objects = driver
                                    .load_schema_objects_of_kind(
                                        &object.database,
                                        &object.schema,
                                        object.kind,
                                    )
                                    .map_err(|error| error.to_string());
                                let preview = match &objects {
                                    Ok(objects)
                                        if objects.iter().any(|candidate| {
                                            candidate.name == object.name
                                                && candidate.kind == object.kind
                                        }) =>
                                    {
                                        let preview = match &preview_filter {
                                            Some(filter) => driver
                                                .load_filtered_preview_page(
                                                    &object,
                                                    filter,
                                                    preview_limit,
                                                    preview_offset,
                                                )
                                                .map_err(|error| error.to_string()),
                                            None => driver
                                                .load_preview_page(
                                                    &object,
                                                    preview_limit,
                                                    preview_offset,
                                                )
                                                .map_err(|error| error.to_string()),
                                        };
                                        (Some(object.clone()), Some(preview))
                                    }
                                    _ => (None, None),
                                };
                                (
                                    Some(LoadedObjectGroup {
                                        database: object.database.clone(),
                                        schema: object.schema.clone(),
                                        kind: object.kind,
                                        result: objects,
                                    }),
                                    preview.0,
                                    preview.1,
                                )
                            }
                            _ => (None, None, None),
                        };

                    SessionEvent::CatalogRefreshed {
                        request_id,
                        catalog_summary,
                        loaded_group,
                        preview_target,
                        preview_offset,
                        preview,
                    }
                }
                SessionCommand::LoadSchemaObjects {
                    request_id,
                    database,
                    schema,
                    kind,
                } => SessionEvent::SchemaObjectsLoaded {
                    request_id,
                    database: database.clone(),
                    schema: schema.clone(),
                    kind,
                    result: driver
                        .load_schema_objects_of_kind(&database, &schema, kind)
                        .map_err(|error| error.to_string()),
                },
                SessionCommand::LoadColumns {
                    request_id,
                    object,
                    kind,
                } => SessionEvent::ColumnsLoaded {
                    request_id,
                    object: object.clone(),
                    kind,
                    result: driver
                        .load_object_columns(&object)
                        .map_err(|error| error.to_string()),
                },
                SessionCommand::LoadStructureColumns { request_id, object } => {
                    SessionEvent::StructureColumnsLoaded {
                        request_id,
                        object: object.clone(),
                        result: driver
                            .load_object_columns(&object)
                            .map_err(|error| error.to_string()),
                    }
                }
                SessionCommand::ExecuteSql {
                    request_id,
                    database,
                    sql,
                } => SessionEvent::SqlExecuted {
                    request_id,
                    result: driver
                        .execute_sql(database.as_deref(), &sql)
                        .map_err(|error| error.to_string()),
                },
                SessionCommand::CancelRequests { .. } | SessionCommand::Shutdown => continue,
            };

            if event_tx.send(event).is_err() {
                break 'outer;
            }
        }

        if should_shutdown {
            break;
        }
    }
}

fn collect_commands(
    first_command: SessionCommand,
    command_rx: &Receiver<SessionCommand>,
    canceled_request_ids: &mut HashSet<u64>,
) -> (Vec<SessionCommand>, bool) {
    let mut raw_commands = vec![first_command];
    while let Ok(command) = command_rx.try_recv() {
        raw_commands.push(command);
    }

    normalize_commands(raw_commands, canceled_request_ids)
}

fn normalize_commands(
    commands: Vec<SessionCommand>,
    canceled_request_ids: &mut HashSet<u64>,
) -> (Vec<SessionCommand>, bool) {
    let mut should_shutdown = false;
    let mut preview = None;
    let mut refresh = None;
    let mut schema_objects = std::collections::BTreeMap::new();
    let mut columns = None;
    let mut structure_columns = None;
    let mut executes = Vec::new();

    let mut pending_commands = Vec::new();
    for command in commands {
        match command {
            SessionCommand::CancelRequests { request_ids } => {
                canceled_request_ids.extend(request_ids)
            }
            SessionCommand::Shutdown => should_shutdown = true,
            other => pending_commands.push(other),
        }
    }

    for command in pending_commands {
        match command {
            SessionCommand::LoadPreview { request_id, .. }
            | SessionCommand::LoadFilteredPreview { request_id, .. } => {
                if !canceled_request_ids.contains(&request_id) {
                    preview = Some(command);
                }
            }
            SessionCommand::RefreshCatalog { request_id, .. } => {
                if !canceled_request_ids.contains(&request_id) {
                    refresh = Some(command);
                }
            }
            SessionCommand::LoadSchemaObjects {
                request_id,
                ref database,
                ref schema,
                kind,
            } => {
                if !canceled_request_ids.contains(&request_id) {
                    schema_objects.insert((database.clone(), schema.clone(), kind), command);
                }
            }
            SessionCommand::LoadColumns { request_id, .. } => {
                if !canceled_request_ids.contains(&request_id) {
                    columns = Some(command);
                }
            }
            SessionCommand::LoadStructureColumns { request_id, .. } => {
                if !canceled_request_ids.contains(&request_id) {
                    structure_columns = Some(command);
                }
            }
            SessionCommand::ExecuteSql { request_id, .. } => {
                if !canceled_request_ids.contains(&request_id) {
                    executes.push(command);
                }
            }
            SessionCommand::CancelRequests { .. } | SessionCommand::Shutdown => {}
        }
    }

    let mut normalized = executes;
    if let Some(command) = refresh {
        normalized.push(command);
    }
    normalized.extend(schema_objects.into_values());
    if let Some(command) = columns {
        normalized.push(command);
    }
    if let Some(command) = structure_columns {
        normalized.push(command);
    }
    if let Some(command) = preview {
        normalized.push(command);
    }

    (normalized, should_shutdown)
}

fn schema_has_kind(
    catalog: &CatalogSummary,
    database_name: &str,
    schema_name: &str,
    kind: DbObjectKind,
) -> bool {
    catalog
        .find_schema(database_name, schema_name)
        .is_some_and(|schema| schema.object_count(kind) > 0)
}

#[cfg(test)]
mod tests {
    use super::{SessionCommand, normalize_commands};
    use crate::background::TemplateKind;
    use relora_core::db::DbObjectKind;

    fn object(name: &str) -> relora_core::db::DbObjectRef {
        relora_core::db::DbObjectRef {
            database: "postgres".to_string(),
            schema: "public".to_string(),
            name: name.to_string(),
            kind: DbObjectKind::Table,
        }
    }

    #[test]
    fn normalize_commands_keeps_priority_and_coalesces_background_work() {
        let mut canceled = std::collections::HashSet::new();
        let commands = vec![
            SessionCommand::LoadPreview {
                request_id: 1,
                object: object("users"),
                limit: 50,
                offset: 0,
            },
            SessionCommand::ExecuteSql {
                request_id: 2,
                database: None,
                sql: "select 1".to_string(),
            },
            SessionCommand::LoadPreview {
                request_id: 3,
                object: object("orders"),
                limit: 50,
                offset: 100,
            },
            SessionCommand::LoadColumns {
                request_id: 4,
                object: object("users"),
                kind: TemplateKind::Insert,
            },
        ];

        let (normalized, should_shutdown) = normalize_commands(commands, &mut canceled);

        assert!(!should_shutdown);
        assert_eq!(
            normalized,
            vec![
                SessionCommand::ExecuteSql {
                    request_id: 2,
                    database: None,
                    sql: "select 1".to_string(),
                },
                SessionCommand::LoadColumns {
                    request_id: 4,
                    object: object("users"),
                    kind: TemplateKind::Insert,
                },
                SessionCommand::LoadPreview {
                    request_id: 3,
                    object: object("orders"),
                    limit: 50,
                    offset: 100,
                },
            ]
        );
    }

    #[test]
    fn normalize_commands_drops_canceled_requests() {
        let mut canceled = std::collections::HashSet::new();
        let commands = vec![
            SessionCommand::LoadPreview {
                request_id: 1,
                object: object("users"),
                limit: 50,
                offset: 0,
            },
            SessionCommand::CancelRequests {
                request_ids: vec![1],
            },
            SessionCommand::ExecuteSql {
                request_id: 2,
                database: None,
                sql: "select 1".to_string(),
            },
        ];

        let (normalized, _) = normalize_commands(commands, &mut canceled);

        assert_eq!(
            normalized,
            vec![SessionCommand::ExecuteSql {
                request_id: 2,
                database: None,
                sql: "select 1".to_string(),
            }]
        );
    }

    #[test]
    fn normalize_commands_keeps_only_the_latest_refresh_request() {
        let mut canceled = std::collections::HashSet::new();
        let commands = vec![
            SessionCommand::RefreshCatalog {
                request_id: 1,
                selected_object: Some(object("users")),
                preview_limit: 100,
                preview_offset: 0,
                preview_filter: None,
            },
            SessionCommand::RefreshCatalog {
                request_id: 2,
                selected_object: Some(object("users")),
                preview_limit: 100,
                preview_offset: 0,
                preview_filter: Some("status = 'active'".to_string()),
            },
        ];

        let (normalized, should_shutdown) = normalize_commands(commands, &mut canceled);

        assert!(!should_shutdown);
        assert_eq!(
            normalized,
            vec![SessionCommand::RefreshCatalog {
                request_id: 2,
                selected_object: Some(object("users")),
                preview_limit: 100,
                preview_offset: 0,
                preview_filter: Some("status = 'active'".to_string()),
            }]
        );
    }

    #[test]
    fn normalize_commands_discards_canceled_refreshes_before_coalescing() {
        let mut canceled = std::collections::HashSet::new();
        let commands = vec![
            SessionCommand::RefreshCatalog {
                request_id: 1,
                selected_object: Some(object("users")),
                preview_limit: 100,
                preview_offset: 0,
                preview_filter: None,
            },
            SessionCommand::CancelRequests {
                request_ids: vec![1],
            },
            SessionCommand::RefreshCatalog {
                request_id: 2,
                selected_object: Some(object("users")),
                preview_limit: 100,
                preview_offset: 100,
                preview_filter: Some("status = 'pending'".to_string()),
            },
        ];

        let (normalized, should_shutdown) = normalize_commands(commands, &mut canceled);

        assert!(!should_shutdown);
        assert_eq!(
            normalized,
            vec![SessionCommand::RefreshCatalog {
                request_id: 2,
                selected_object: Some(object("users")),
                preview_limit: 100,
                preview_offset: 100,
                preview_filter: Some("status = 'pending'".to_string()),
            }]
        );
    }
}
