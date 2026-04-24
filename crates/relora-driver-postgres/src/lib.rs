use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use postgres::{Client, NoTls, SimpleQueryMessage};
use relora_core::db::{
    Catalog, CatalogSummary, CommandResult, DatabaseDriver, DatabaseEntry, DatabaseKind,
    DatabaseSummary, DbColumn, DbObjectKind, DbObjectRef, ObjectKindCount, QueryResult,
    SchemaEntry, SchemaSummary, SqlExecutionResult, TablePreview,
};
use serde_json::Value;
use url::Url;

const CATALOG_SQL: &str = r#"
    SELECT schema_name, object_name, object_kind
    FROM (
        SELECT
            n.nspname AS schema_name,
            c.relname AS object_name,
            CASE c.relkind
                WHEN 'r' THEN 'BASE TABLE'
                WHEN 'v' THEN 'VIEW'
                WHEN 'm' THEN 'MATERIALIZED VIEW'
                WHEN 'f' THEN 'FOREIGN TABLE'
            END AS object_kind,
            CASE c.relkind
                WHEN 'r' THEN 1
                WHEN 'v' THEN 2
                WHEN 'm' THEN 3
                WHEN 'f' THEN 4
            END AS kind_order
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND c.relkind IN ('r', 'v', 'm', 'f')
        UNION ALL
        SELECT
            n.nspname AS schema_name,
            format('%s(%s)', p.proname, COALESCE(pg_get_function_identity_arguments(p.oid), ''))
                AS object_name,
            'FUNCTION' AS object_kind,
            5 AS kind_order
        FROM pg_proc p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND p.prokind = 'f'
    ) catalog_objects
    ORDER BY schema_name, kind_order, object_name
"#;

const CATALOG_SUMMARY_SQL: &str = r#"
    WITH catalog_objects AS (
        SELECT
            n.nspname AS schema_name,
            CASE c.relkind
                WHEN 'r' THEN 'BASE TABLE'
                WHEN 'v' THEN 'VIEW'
                WHEN 'm' THEN 'MATERIALIZED VIEW'
                WHEN 'f' THEN 'FOREIGN TABLE'
            END AS object_kind
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND c.relkind IN ('r', 'v', 'm', 'f')
        UNION ALL
        SELECT
            n.nspname AS schema_name,
            'FUNCTION' AS object_kind
        FROM pg_proc p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND p.prokind = 'f'
    )
    SELECT
        schema_name,
        COUNT(*) FILTER (WHERE object_kind = 'BASE TABLE') AS table_count,
        COUNT(*) FILTER (WHERE object_kind = 'VIEW') AS view_count,
        COUNT(*) FILTER (WHERE object_kind = 'MATERIALIZED VIEW') AS materialized_view_count,
        COUNT(*) FILTER (WHERE object_kind = 'FOREIGN TABLE') AS foreign_table_count,
        COUNT(*) FILTER (WHERE object_kind = 'FUNCTION') AS function_count
    FROM catalog_objects
    GROUP BY schema_name
    ORDER BY schema_name
"#;

const SCHEMA_OBJECTS_SQL: &str = r#"
    SELECT schema_name, object_name, object_kind
    FROM (
        SELECT
            n.nspname AS schema_name,
            c.relname AS object_name,
            CASE c.relkind
                WHEN 'r' THEN 'BASE TABLE'
                WHEN 'v' THEN 'VIEW'
                WHEN 'm' THEN 'MATERIALIZED VIEW'
                WHEN 'f' THEN 'FOREIGN TABLE'
            END AS object_kind,
            CASE c.relkind
                WHEN 'r' THEN 1
                WHEN 'v' THEN 2
                WHEN 'm' THEN 3
                WHEN 'f' THEN 4
            END AS kind_order
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = $1
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND c.relkind IN ('r', 'v', 'm', 'f')
        UNION ALL
        SELECT
            n.nspname AS schema_name,
            format('%s(%s)', p.proname, COALESCE(pg_get_function_identity_arguments(p.oid), ''))
                AS object_name,
            'FUNCTION' AS object_kind,
            5 AS kind_order
        FROM pg_proc p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname = $1
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND p.prokind = 'f'
    ) schema_objects
    ORDER BY kind_order, object_name
"#;

const SCHEMA_OBJECTS_OF_KIND_SQL: &str = r#"
    SELECT schema_name, object_name, object_kind
    FROM (
        SELECT
            n.nspname AS schema_name,
            c.relname AS object_name,
            CASE c.relkind
                WHEN 'r' THEN 'BASE TABLE'
                WHEN 'v' THEN 'VIEW'
                WHEN 'm' THEN 'MATERIALIZED VIEW'
                WHEN 'f' THEN 'FOREIGN TABLE'
            END AS object_kind
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = $1
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND c.relkind IN ('r', 'v', 'm', 'f')
        UNION ALL
        SELECT
            n.nspname AS schema_name,
            format('%s(%s)', p.proname, COALESCE(pg_get_function_identity_arguments(p.oid), ''))
                AS object_name,
            'FUNCTION' AS object_kind
        FROM pg_proc p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname = $1
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND p.prokind = 'f'
    ) schema_objects
    WHERE object_kind = $2
    ORDER BY object_name
"#;

const RELATION_COLUMN_SQL: &str = r#"
    SELECT
        a.attname AS column_name,
        pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type,
        NOT a.attnotnull AS is_nullable,
        ad.adbin IS NOT NULL AS has_default,
        EXISTS (
            SELECT 1
            FROM pg_index ix
            WHERE ix.indrelid = c.oid
              AND ix.indisunique
              AND NOT ix.indisprimary
              AND ix.indnatts = 1
              AND a.attnum = ANY(ix.indkey)
        ) AS is_unique,
        EXISTS (
            SELECT 1
            FROM pg_index ix
            WHERE ix.indrelid = c.oid
              AND ix.indisprimary
              AND a.attnum = ANY(ix.indkey)
        ) AS is_primary_key
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    JOIN pg_attribute a ON a.attrelid = c.oid
    LEFT JOIN pg_attrdef ad
      ON ad.adrelid = c.oid
     AND ad.adnum = a.attnum
    WHERE n.nspname = $1
      AND c.relname = $2
      AND c.relkind IN ('r', 'v', 'm', 'f')
      AND a.attnum > 0
      AND NOT a.attisdropped
    ORDER BY a.attnum
"#;

const FUNCTION_COLUMN_SQL: &str = r#"
    WITH function_target AS (
        SELECT
            p.oid,
            p.proname,
            COALESCE(pg_get_function_identity_arguments(p.oid), '') AS identity_arguments,
            pg_get_function_result(p.oid) AS return_type,
            COALESCE(p.proargnames, ARRAY[]::text[]) AS arg_names,
            COALESCE(p.proargmodes, ARRAY[]::"char"[]) AS arg_modes,
            COALESCE(p.proallargtypes, p.proargtypes::oid[]) AS arg_types
        FROM pg_proc p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname = $1
          AND p.prokind = 'f'
          AND p.proname = $2
          AND COALESCE(pg_get_function_identity_arguments(p.oid), '') = $3
    ),
    arguments AS (
        SELECT
            arg_types.ordinality AS sort_order,
            COALESCE(
                NULLIF(function_target.arg_names[arg_types.ordinality], ''),
                format('$%s', arg_types.ordinality)
            ) AS column_name,
            format_type(arg_types.type_oid, NULL) AS data_type,
            TRUE AS is_nullable,
            FALSE AS has_default,
            FALSE AS is_unique,
            FALSE AS is_primary_key
        FROM function_target
        CROSS JOIN LATERAL unnest(function_target.arg_types) WITH ORDINALITY AS arg_types(type_oid, ordinality)
        WHERE COALESCE(function_target.arg_modes[arg_types.ordinality], 'i') IN ('i', 'b', 'v')
    )
    SELECT column_name, data_type, is_nullable, has_default, is_unique, is_primary_key
    FROM (
        SELECT
            sort_order,
            column_name,
            data_type,
            is_nullable,
            has_default,
            is_unique,
            is_primary_key
        FROM arguments
        UNION ALL
        SELECT
            2147483647 AS sort_order,
            'returns' AS column_name,
            return_type AS data_type,
            FALSE AS is_nullable,
            FALSE AS has_default,
            FALSE AS is_unique,
            FALSE AS is_primary_key
        FROM function_target
    ) function_columns
    ORDER BY sort_order
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
            let object_name: String = row.get(1);
            let object_kind: String = row.get(2);
            let kind = parse_object_kind(&object_kind);

            if let Some(schema) = schemas
                .last_mut()
                .filter(|schema| schema.name == schema_name)
            {
                schema.objects.push(DbObjectRef {
                    database: database.to_string(),
                    schema: schema_name,
                    name: object_name,
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
                    name: object_name,
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
                                (DbObjectKind::MaterializedView, row.get::<_, i64>(3)),
                                (DbObjectKind::ForeignTable, row.get::<_, i64>(4)),
                                (DbObjectKind::Function, row.get::<_, i64>(5)),
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
            .query(SCHEMA_OBJECTS_SQL, &[&schema])
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
            .query(SCHEMA_OBJECTS_OF_KIND_SQL, &[&schema, &table_type])
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
        if !table.kind.supports_data_preview() {
            bail!(
                "data preview is not available for {} {}",
                table.kind.label(),
                table.database_qualified_name()
            );
        }

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
        if !table.kind.supports_data_preview() {
            bail!(
                "data preview is not available for {} {}",
                table.kind.label(),
                table.database_qualified_name()
            );
        }

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
        let rows = if table.kind == DbObjectKind::Function {
            let (function_name, identity_arguments) = split_function_identity(&table.name);
            self.client_for_database(&table.database)?
                .query(
                    FUNCTION_COLUMN_SQL,
                    &[&table.schema, &function_name, &identity_arguments],
                )
                .with_context(|| {
                    format!("failed to describe {}", table.database_qualified_name())
                })?
        } else {
            self.client_for_database(&table.database)?
                .query(RELATION_COLUMN_SQL, &[&table.schema, &table.name])
                .with_context(|| {
                    format!("failed to describe {}", table.database_qualified_name())
                })?
        };

        Ok(rows
            .into_iter()
            .map(|row| DbColumn {
                name: row.get(0),
                data_type: row.get(1),
                nullable: row.get(2),
                has_default: row.get(3),
                is_unique: row.get(4),
                is_primary_key: row.get(5),
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
        "MATERIALIZED VIEW" => DbObjectKind::MaterializedView,
        "FOREIGN TABLE" => DbObjectKind::ForeignTable,
        "FUNCTION" => DbObjectKind::Function,
        _ => DbObjectKind::Table,
    }
}

fn postgres_table_type(kind: DbObjectKind) -> &'static str {
    match kind {
        DbObjectKind::Table => "BASE TABLE",
        DbObjectKind::View => "VIEW",
        DbObjectKind::MaterializedView => "MATERIALIZED VIEW",
        DbObjectKind::ForeignTable => "FOREIGN TABLE",
        DbObjectKind::Function => "FUNCTION",
    }
}

fn split_function_identity(name: &str) -> (String, String) {
    let Some(open_paren) = name.find('(') else {
        return (name.to_string(), String::new());
    };
    let Some(close_paren) = name.rfind(')') else {
        return (name.to_string(), String::new());
    };
    if close_paren < open_paren {
        return (name.to_string(), String::new());
    }

    (
        name[..open_paren].to_string(),
        name[open_paren + 1..close_paren].trim().to_string(),
    )
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
    use super::{RELATION_COLUMN_SQL, parse_object_kind, split_function_identity};
    use relora_core::db::DbObjectKind;

    #[test]
    fn column_query_qualifies_schema_and_table_filters() {
        assert!(RELATION_COLUMN_SQL.contains("WHERE n.nspname = $1"));
        assert!(RELATION_COLUMN_SQL.contains("AND c.relname = $2"));
    }

    #[test]
    fn parse_object_kind_supports_materialized_views_and_functions() {
        assert_eq!(
            parse_object_kind("MATERIALIZED VIEW"),
            DbObjectKind::MaterializedView
        );
        assert_eq!(parse_object_kind("FUNCTION"), DbObjectKind::Function);
    }

    #[test]
    fn split_function_identity_extracts_identity_arguments_from_signatures() {
        assert_eq!(
            split_function_identity("refresh_sales()"),
            ("refresh_sales".to_string(), String::new())
        );
        assert_eq!(
            split_function_identity("calculate_tax(integer, numeric)"),
            ("calculate_tax".to_string(), "integer, numeric".to_string())
        );
    }
}
