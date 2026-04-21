use std::collections::BTreeMap;

use anyhow::{Context, Result};
use postgres::{Client, NoTls, SimpleQueryMessage};
use relora_core::db::{
    Catalog, CatalogSummary, CommandResult, DatabaseDriver, DatabaseEntry, DatabaseKind,
    DatabaseSummary, DbColumn, DbObjectKind, DbObjectRef, ObjectKindCount, QueryResult,
    SchemaEntry, SchemaSummary, SqlExecutionResult, TablePreview,
};
use serde_json::Value;
use url::Url;

const CATALOG_SQL: &str = r#"
    SELECT table_schema, table_name, table_type
    FROM information_schema.tables
    WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
      AND table_type IN ('BASE TABLE', 'VIEW', 'FOREIGN TABLE')
    ORDER BY table_schema, table_name
"#;

const CATALOG_SUMMARY_SQL: &str = r#"
    SELECT
        table_schema,
        COUNT(*) FILTER (WHERE table_type = 'BASE TABLE') AS table_count,
        COUNT(*) FILTER (WHERE table_type = 'VIEW') AS view_count,
        COUNT(*) FILTER (WHERE table_type = 'FOREIGN TABLE') AS foreign_table_count
    FROM information_schema.tables
    WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
      AND table_type IN ('BASE TABLE', 'VIEW', 'FOREIGN TABLE')
    GROUP BY table_schema
    ORDER BY table_schema
"#;

const COLUMN_SQL: &str = r#"
    WITH primary_keys AS (
        SELECT kcu.table_schema, kcu.table_name, kcu.column_name
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
          ON tc.constraint_name = kcu.constraint_name
         AND tc.table_schema = kcu.table_schema
        WHERE tc.constraint_type = 'PRIMARY KEY'
    )
    SELECT
        c.column_name,
        c.data_type,
        c.is_nullable = 'YES' AS is_nullable,
        c.column_default IS NOT NULL AS has_default,
        pk.column_name IS NOT NULL AS is_primary_key
    FROM information_schema.columns c
    LEFT JOIN primary_keys pk
      ON c.table_schema = pk.table_schema
     AND c.table_name = pk.table_name
     AND c.column_name = pk.column_name
    WHERE c.table_schema = $1
      AND c.table_name = $2
    ORDER BY c.ordinal_position
"#;

const DATABASES_SQL: &str = r#"
    SELECT datname
    FROM pg_database
    WHERE datallowconn
      AND NOT datistemplate
    ORDER BY (datname = current_database()) DESC, datname
"#;

pub struct PostgresDriver {
    clients: BTreeMap<String, Client>,
    base_url: Url,
    default_database: String,
    connection_label: String,
}

impl PostgresDriver {
    pub fn connect(url: &str) -> Result<Self> {
        let parsed = Url::parse(url).context("failed to parse PostgreSQL url")?;
        let default_database = database_name_from_url(&parsed);
        let client = Client::connect(url, NoTls).context("failed to connect to PostgreSQL")?;
        let mut clients = BTreeMap::new();
        clients.insert(default_database.clone(), client);
        Ok(Self {
            clients,
            base_url: parsed,
            default_database,
            connection_label: build_connection_label(url),
        })
    }

    fn client_for_database(&mut self, database: &str) -> Result<&mut Client> {
        if !self.clients.contains_key(database) {
            let url = connection_url_for_database(&self.base_url, database)?;
            let client = Client::connect(&url, NoTls)
                .with_context(|| format!("failed to connect to PostgreSQL database {database}"))?;
            self.clients.insert(database.to_string(), client);
        }

        self.clients
            .get_mut(database)
            .with_context(|| format!("database client is unavailable for {database}"))
    }

    fn database_names(&mut self) -> Result<Vec<String>> {
        let database = self.default_database.clone();
        let client = self.client_for_database(&database)?;
        let rows = client
            .query(DATABASES_SQL, &[])
            .context("failed to query PostgreSQL databases")?;
        Ok(rows.into_iter().map(|row| row.get(0)).collect())
    }

    fn load_catalog_for_database(&mut self, database: &str) -> Result<DatabaseEntry> {
        let client = self.client_for_database(database)?;
        let rows = client
            .query(CATALOG_SQL, &[])
            .with_context(|| format!("failed to query PostgreSQL catalog for {database}"))?;

        let mut schemas: Vec<SchemaEntry> = Vec::new();
        for row in rows {
            let schema_name: String = row.get(0);
            let table_name: String = row.get(1);
            let table_type: String = row.get(2);
            let kind = parse_object_kind(&table_type);

            if let Some(schema) = schemas
                .last_mut()
                .filter(|schema| schema.name == schema_name)
            {
                schema.objects.push(DbObjectRef {
                    database: database.to_string(),
                    schema: schema_name,
                    name: table_name,
                    kind,
                });
                continue;
            }

            schemas.push(SchemaEntry {
                database: database.to_string(),
                name: schema_name.clone(),
                objects: vec![DbObjectRef {
                    database: database.to_string(),
                    schema: schema_name,
                    name: table_name,
                    kind,
                }],
            });
        }

        Ok(DatabaseEntry {
            name: database.to_string(),
            schemas,
        })
    }
}

impl DatabaseDriver for PostgresDriver {
    fn kind(&self) -> DatabaseKind {
        DatabaseKind::Postgres
    }

    fn connection_label(&self) -> &str {
        &self.connection_label
    }

    fn load_catalog(&mut self) -> Result<Catalog> {
        let databases = self
            .database_names()?
            .into_iter()
            .map(|database| self.load_catalog_for_database(&database))
            .collect::<Result<Vec<_>>>()?;

        Ok(Catalog { databases })
    }

    fn load_catalog_summary(&mut self) -> Result<CatalogSummary> {
        let databases = self
            .database_names()?
            .into_iter()
            .map(|database| {
                let client = self.client_for_database(&database)?;
                let rows = client.query(CATALOG_SUMMARY_SQL, &[]).with_context(|| {
                    format!("failed to query PostgreSQL catalog summary for {database}")
                })?;

                Ok(DatabaseSummary {
                    name: database.clone(),
                    schemas: rows
                        .into_iter()
                        .map(|row| SchemaSummary {
                            database: database.clone(),
                            name: row.get::<_, String>(0),
                            object_counts: [
                                (DbObjectKind::Table, row.get::<_, i64>(1)),
                                (DbObjectKind::View, row.get::<_, i64>(2)),
                                (DbObjectKind::ForeignTable, row.get::<_, i64>(3)),
                            ]
                            .into_iter()
                            .filter(|(_, count)| *count > 0)
                            .map(|(kind, count)| ObjectKindCount {
                                kind,
                                count: count as usize,
                            })
                            .collect(),
                        })
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(CatalogSummary { databases })
    }

    fn load_schema_objects(&mut self, database: &str, schema: &str) -> Result<Vec<DbObjectRef>> {
        let client = self.client_for_database(database)?;
        let rows = client
            .query(
                "SELECT table_schema, table_name, table_type
                 FROM information_schema.tables
                 WHERE table_schema = $1
                   AND table_schema NOT IN ('pg_catalog', 'information_schema')
                   AND table_type IN ('BASE TABLE', 'VIEW', 'FOREIGN TABLE')
                 ORDER BY table_type, table_name",
                &[&schema],
            )
            .with_context(|| {
                format!("failed to query PostgreSQL schema objects for {database}.{schema}")
            })?;

        Ok(rows
            .into_iter()
            .map(|row| DbObjectRef {
                database: database.to_string(),
                schema: row.get::<_, String>(0),
                name: row.get::<_, String>(1),
                kind: parse_object_kind(&row.get::<_, String>(2)),
            })
            .collect())
    }

    fn load_schema_objects_of_kind(
        &mut self,
        database: &str,
        schema: &str,
        kind: DbObjectKind,
    ) -> Result<Vec<DbObjectRef>> {
        let table_type = postgres_table_type(kind);
        let client = self.client_for_database(database)?;
        let rows = client
            .query(
                "SELECT table_schema, table_name, table_type
                 FROM information_schema.tables
                 WHERE table_schema = $1
                   AND table_schema NOT IN ('pg_catalog', 'information_schema')
                   AND table_type = $2
                 ORDER BY table_name",
                &[&schema, &table_type],
            )
            .with_context(|| {
                format!(
                    "failed to query PostgreSQL {} for {database}.{schema}",
                    kind.group_label()
                )
            })?;

        Ok(rows
            .into_iter()
            .map(|row| DbObjectRef {
                database: database.to_string(),
                schema: row.get::<_, String>(0),
                name: row.get::<_, String>(1),
                kind: parse_object_kind(&row.get::<_, String>(2)),
            })
            .collect())
    }

    fn load_preview_page(
        &mut self,
        table: &DbObjectRef,
        limit: usize,
        offset: usize,
    ) -> Result<TablePreview> {
        let columns = self.load_columns(table)?;
        if columns.is_empty() {
            return Ok(TablePreview::default());
        }

        let query = format!(
            "SELECT row_to_json(t)::text FROM (SELECT * FROM {} LIMIT {} OFFSET {}) AS t",
            qualified_name(table),
            limit.max(1),
            offset
        );
        let rows = self
            .client_for_database(&table.database)?
            .query(query.as_str(), &[])
            .with_context(|| format!("failed to preview {}", table.database_qualified_name()))?;

        let parsed_rows = rows
            .into_iter()
            .map(|row| {
                let payload: String = row.get(0);
                let json: Value =
                    serde_json::from_str(&payload).context("failed to decode preview row")?;

                Ok(columns
                    .iter()
                    .map(|column| {
                        json.get(column)
                            .map(json_value_to_string)
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>())
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(TablePreview {
            columns,
            rows: parsed_rows,
        })
    }

    fn load_filtered_preview_page(
        &mut self,
        table: &DbObjectRef,
        filter: &str,
        limit: usize,
        offset: usize,
    ) -> Result<TablePreview> {
        let columns = self.load_columns(table)?;
        if columns.is_empty() {
            return Ok(TablePreview::default());
        }

        let predicate = columns
            .iter()
            .map(|column| format!("{}::text ILIKE $1", quoted_identifier(column)))
            .collect::<Vec<_>>()
            .join(" OR ");
        let query = format!(
            "SELECT row_to_json(t)::text FROM (SELECT * FROM {} WHERE {} LIMIT {} OFFSET {}) AS t",
            qualified_name(table),
            predicate,
            limit.max(1),
            offset
        );
        let pattern = format!("%{filter}%");
        let rows = self
            .client_for_database(&table.database)?
            .query(query.as_str(), &[&pattern])
            .with_context(|| {
                format!(
                    "failed to filter preview {}",
                    table.database_qualified_name()
                )
            })?;

        let parsed_rows = rows
            .into_iter()
            .map(|row| {
                let payload: String = row.get(0);
                let json: Value =
                    serde_json::from_str(&payload).context("failed to decode preview row")?;

                Ok(columns
                    .iter()
                    .map(|column| {
                        json.get(column)
                            .map(json_value_to_string)
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>())
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(TablePreview {
            columns,
            rows: parsed_rows,
        })
    }

    fn load_object_columns(&mut self, table: &DbObjectRef) -> Result<Vec<DbColumn>> {
        let rows = self
            .client_for_database(&table.database)?
            .query(COLUMN_SQL, &[&table.schema, &table.name])
            .with_context(|| format!("failed to describe {}", table.database_qualified_name()))?;

        Ok(rows
            .into_iter()
            .map(|row| DbColumn {
                name: row.get(0),
                data_type: row.get(1),
                nullable: row.get(2),
                has_default: row.get(3),
                is_primary_key: row.get(4),
            })
            .collect())
    }

    fn execute_sql(
        &mut self,
        database: Option<&str>,
        sql: &str,
    ) -> Result<Vec<SqlExecutionResult>> {
        let database = database
            .unwrap_or(self.default_database.as_str())
            .to_string();
        let messages = self
            .client_for_database(&database)?
            .simple_query(sql)
            .context("failed to execute SQL batch")?;
        let statements = split_sql_statements(sql);

        let mut results = Vec::new();
        let mut statement_index = 0usize;
        let mut current_columns: Option<Vec<String>> = None;
        let mut current_rows: Vec<Vec<String>> = Vec::new();

        for message in messages {
            match message {
                SimpleQueryMessage::RowDescription(columns) => {
                    current_columns = Some(
                        columns
                            .iter()
                            .map(|column| column.name().to_string())
                            .collect(),
                    );
                    current_rows.clear();
                }
                SimpleQueryMessage::Row(row) => {
                    let columns = current_columns.get_or_insert_with(|| {
                        row.columns()
                            .iter()
                            .map(|column| column.name().to_string())
                            .collect()
                    });
                    current_rows.push(
                        (0..columns.len())
                            .map(|index| row.get(index).unwrap_or("NULL").to_string())
                            .collect(),
                    );
                }
                SimpleQueryMessage::CommandComplete(rows_affected) => {
                    if let Some(columns) = current_columns.take() {
                        results.push(SqlExecutionResult::Query(QueryResult {
                            columns,
                            rows: std::mem::take(&mut current_rows),
                        }));
                    } else {
                        let tag = statements
                            .get(statement_index)
                            .map(|statement| command_tag(statement))
                            .unwrap_or_else(|| "COMMAND".to_string());
                        results.push(SqlExecutionResult::Command(CommandResult {
                            tag,
                            rows_affected,
                        }));
                    }
                    statement_index += 1;
                }
                _ => {}
            }
        }

        Ok(results)
    }
}

impl PostgresDriver {
    fn load_columns(&mut self, table: &DbObjectRef) -> Result<Vec<String>> {
        let columns = self.load_object_columns(table)?;
        Ok(columns.into_iter().map(|column| column.name).collect())
    }
}

fn build_connection_label(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return "postgres".to_string();
    };

    let host = parsed.host_str().unwrap_or("localhost");
    let port = parsed
        .port()
        .map(|value| format!(":{value}"))
        .unwrap_or_default();
    let database = database_name_from_url(&parsed);

    format!("postgres://{host}{port}/{database}")
}

fn qualified_name(table: &DbObjectRef) -> String {
    format!(
        "\"{}\".\"{}\"",
        escape_identifier(&table.schema),
        escape_identifier(&table.name)
    )
}

fn parse_object_kind(table_type: &str) -> DbObjectKind {
    match table_type {
        "VIEW" => DbObjectKind::View,
        "FOREIGN TABLE" => DbObjectKind::ForeignTable,
        _ => DbObjectKind::Table,
    }
}

fn postgres_table_type(kind: DbObjectKind) -> &'static str {
    match kind {
        DbObjectKind::Table => "BASE TABLE",
        DbObjectKind::View => "VIEW",
        DbObjectKind::ForeignTable => "FOREIGN TABLE",
    }
}

fn quoted_identifier(identifier: &str) -> String {
    format!("\"{}\"", escape_identifier(identifier))
}

fn escape_identifier(identifier: &str) -> String {
    identifier.replace('"', "\"\"")
}

fn database_name_from_url(url: &Url) -> String {
    let database = url.path().trim_start_matches('/');
    if database.is_empty() {
        "postgres".to_string()
    } else {
        database.to_string()
    }
}

fn connection_url_for_database(base_url: &Url, database: &str) -> Result<String> {
    let mut url = base_url.clone();
    url.set_path(&format!("/{}", escape_path_segment(database)));
    Ok(url.to_string())
}

fn escape_path_segment(segment: &str) -> String {
    segment.replace('%', "%25").replace('/', "%2F")
}

fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn command_tag(sql: &str) -> String {
    sql.split_whitespace()
        .next()
        .map(|token| token.to_uppercase())
        .filter(|token| !token.is_empty())
        .unwrap_or_else(|| "COMMAND".to_string())
}

fn split_sql_statements(sql: &str) -> Vec<&str> {
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::COLUMN_SQL;

    #[test]
    fn column_query_qualifies_schema_and_table_filters() {
        assert!(COLUMN_SQL.contains("WHERE c.table_schema = $1"));
        assert!(COLUMN_SQL.contains("AND c.table_name = $2"));
    }
}
