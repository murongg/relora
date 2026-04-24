use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use relora_core::db::{
    Catalog, CatalogSummary, CommandResult, DatabaseDriver, DatabaseEntry, DatabaseKind,
    DatabaseSummary, DbColumn, DbObjectKind, DbObjectRef, ObjectKindCount, QueryResult,
    SchemaEntry, SchemaSummary, SqlExecutionResult, TablePreview,
};
use rusqlite::{Connection, Row, types::ValueRef};
use url::Url;

pub struct SqliteDriver {
    connection: Connection,
    connection_label: String,
}

impl SqliteDriver {
    pub fn connect(url: &str) -> Result<Self> {
        let database_path = database_path_from_url(url)?;
        let connection = if database_path == ":memory:" {
            Connection::open_in_memory().context("failed to open in-memory SQLite database")?
        } else {
            Connection::open(&database_path)
                .with_context(|| format!("failed to open SQLite database {database_path}"))?
        };

        Ok(Self {
            connection,
            connection_label: database_path,
        })
    }

    fn database_names(&self) -> Result<Vec<String>> {
        let mut statement = self
            .connection
            .prepare("PRAGMA database_list")
            .context("failed to inspect SQLite databases")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(1))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read SQLite database list")
    }

    fn load_columns(&mut self, table: &DbObjectRef) -> Result<Vec<String>> {
        Ok(self
            .load_object_columns(table)?
            .into_iter()
            .map(|column| column.name)
            .collect())
    }

    fn load_object_columns_with_xinfo(&mut self, table: &DbObjectRef) -> Result<Vec<DbColumn>> {
        let sql = format!(
            "PRAGMA {}.table_xinfo({})",
            quoted_identifier(&table.schema),
            quoted_string(&table.name)
        );
        let mut statement = self.connection.prepare(&sql).with_context(|| {
            format!(
                "failed to describe {} using SQLite table_xinfo",
                table.database_qualified_name()
            )
        })?;
        let rows = statement.query_map([], |row| {
            let hidden_flag = row.get::<_, i64>(6)?;
            Ok((hidden_flag, parse_column_row(row)?))
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read SQLite xinfo columns")
            .map(|columns| {
                let unique_columns = self
                    .load_single_column_unique_columns(table)
                    .unwrap_or_default();
                columns
                    .into_iter()
                    // SQLite marks internal virtual-table columns as hidden=1.
                    // Generated columns are surfaced as 2/3 and should stay visible.
                    .filter_map(|(hidden_flag, mut column)| {
                        if hidden_flag == 1 {
                            return None;
                        }
                        column.is_unique = unique_columns.contains(&column.name);
                        Some(column)
                    })
                    .collect()
            })
    }

    fn load_object_columns_with_table_info(
        &mut self,
        table: &DbObjectRef,
    ) -> Result<Vec<DbColumn>> {
        let sql = format!(
            "PRAGMA {}.table_info({})",
            quoted_identifier(&table.schema),
            quoted_string(&table.name)
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .with_context(|| format!("failed to describe {}", table.database_qualified_name()))?;
        let rows = statement.query_map([], parse_column_row)?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read SQLite columns")
            .map(|mut columns| {
                let unique_columns = self
                    .load_single_column_unique_columns(table)
                    .unwrap_or_default();
                for column in &mut columns {
                    column.is_unique = unique_columns.contains(&column.name);
                }
                columns
            })
    }

    fn load_single_column_unique_columns(&self, table: &DbObjectRef) -> Result<BTreeSet<String>> {
        let sql = format!(
            "PRAGMA {}.index_list({})",
            quoted_identifier(&table.schema),
            quoted_string(&table.name)
        );
        let mut statement = self.connection.prepare(&sql).with_context(|| {
            format!(
                "failed to inspect SQLite indexes for {}",
                table.database_qualified_name()
            )
        })?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? > 0,
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut unique_columns = BTreeSet::new();
        for row in rows {
            let (index_name, is_unique, origin) = row?;
            if !is_unique || origin == "pk" {
                continue;
            }
            let info_sql = format!(
                "PRAGMA {}.index_info({})",
                quoted_identifier(&table.schema),
                quoted_string(&index_name)
            );
            let mut info_statement = self.connection.prepare(&info_sql).with_context(|| {
                format!(
                    "failed to inspect SQLite index {} for {}",
                    index_name,
                    table.database_qualified_name()
                )
            })?;
            let columns = info_statement
                .query_map([], |row| row.get::<_, String>(2))?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("failed to read SQLite index columns")?;
            if columns.len() == 1 {
                unique_columns.insert(columns[0].clone());
            }
        }
        Ok(unique_columns)
    }
}

impl DatabaseDriver for SqliteDriver {
    fn kind(&self) -> DatabaseKind {
        DatabaseKind::Sqlite
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
                let sql = format!(
                    "SELECT type, COUNT(*) FROM {}.sqlite_master \
                     WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
                     GROUP BY type ORDER BY type",
                    quoted_identifier(&database)
                );
                let mut statement = self.connection.prepare(&sql).with_context(|| {
                    format!("failed to inspect SQLite catalog summary for {database}")
                })?;
                let rows = statement.query_map([], |row| {
                    Ok(ObjectKindCount {
                        kind: sqlite_object_kind(&row.get::<_, String>(0)?),
                        count: row.get::<_, i64>(1)? as usize,
                    })
                })?;
                let object_counts = rows
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("failed to read SQLite catalog summary rows")?;

                Ok(DatabaseSummary {
                    name: database.clone(),
                    schemas: vec![SchemaSummary {
                        database: database.clone(),
                        name: database,
                        object_counts,
                    }],
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(CatalogSummary { databases })
    }

    fn load_schema_objects(&mut self, database: &str, schema: &str) -> Result<Vec<DbObjectRef>> {
        self.load_catalog_for_database(schema)
            .map(|entry| {
                entry
                    .schemas
                    .into_iter()
                    .next()
                    .map(|schema| schema.objects)
                    .unwrap_or_default()
            })
            .with_context(|| {
                format!("failed to inspect SQLite schema objects for {database}.{schema}")
            })
    }

    fn load_schema_objects_of_kind(
        &mut self,
        database: &str,
        schema: &str,
        kind: DbObjectKind,
    ) -> Result<Vec<DbObjectRef>> {
        let Some(object_type) = sqlite_object_type(kind) else {
            return Ok(Vec::new());
        };
        let sql = format!(
            "SELECT name, type
             FROM {}.sqlite_master
             WHERE type = ?1
               AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
            quoted_identifier(schema)
        );
        let mut statement = self.connection.prepare(&sql).with_context(|| {
            format!(
                "failed to inspect SQLite {} for {database}.{schema}",
                kind.group_label()
            )
        })?;
        let rows = statement.query_map([object_type], |row| {
            Ok(DbObjectRef {
                database: database.to_string(),
                schema: schema.to_string(),
                name: row.get(0)?,
                kind: sqlite_object_kind(&row.get::<_, String>(1)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| {
                format!(
                    "failed to read SQLite {} for {database}.{schema}",
                    kind.group_label()
                )
            })
    }

    fn load_preview_page(
        &mut self,
        table: &DbObjectRef,
        limit: usize,
        offset: usize,
    ) -> Result<TablePreview> {
        let sql = format!(
            "SELECT * FROM {} LIMIT ?1 OFFSET ?2",
            sqlite_qualified_name(&table.schema, &table.name)
        );
        let mut statement = self.connection.prepare(&sql).with_context(|| {
            format!(
                "failed to prepare SQLite preview for {}",
                table.database_qualified_name()
            )
        })?;
        let columns = statement
            .column_names()
            .into_iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let column_count = statement.column_count();
        let rows = statement.query_map([limit.max(1) as i64, offset as i64], |row| {
            row_to_strings(row, column_count)
        })?;

        Ok(TablePreview {
            columns,
            rows: rows
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("failed to read SQLite preview rows")?,
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
            .map(|column| {
                format!(
                    "CAST({} AS TEXT) LIKE ?1 ESCAPE '\\'",
                    quoted_identifier(column)
                )
            })
            .collect::<Vec<_>>()
            .join(" OR ");
        let sql = format!(
            "SELECT * FROM {} WHERE {} LIMIT ?2 OFFSET ?3",
            sqlite_qualified_name(&table.schema, &table.name),
            predicate
        );
        let pattern = format!("%{}%", escape_like_pattern(filter));
        let mut statement = self.connection.prepare(&sql).with_context(|| {
            format!(
                "failed to prepare SQLite filtered preview for {}",
                table.database_qualified_name()
            )
        })?;
        let column_count = statement.column_count();
        let rows = statement.query_map((&pattern, limit.max(1) as i64, offset as i64), |row| {
            row_to_strings(row, column_count)
        })?;

        Ok(TablePreview {
            columns,
            rows: rows
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("failed to read SQLite filtered preview rows")?,
        })
    }

    fn load_object_columns(&mut self, table: &DbObjectRef) -> Result<Vec<DbColumn>> {
        self.load_object_columns_with_xinfo(table)
            .or_else(|_| self.load_object_columns_with_table_info(table))
    }

    fn execute_sql(
        &mut self,
        _database: Option<&str>,
        sql: &str,
    ) -> Result<Vec<SqlExecutionResult>> {
        split_sql_statements(sql)
            .into_iter()
            .map(|statement| execute_statement(&self.connection, statement))
            .collect()
    }
}

impl SqliteDriver {
    fn load_catalog_for_database(&self, database: &str) -> Result<DatabaseEntry> {
        let sql = format!(
            "SELECT name, type FROM {}.sqlite_master \
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
             ORDER BY type, name",
            quoted_identifier(database)
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .with_context(|| format!("failed to inspect SQLite catalog for {database}"))?;
        let rows = statement.query_map([], |row| {
            let name: String = row.get(0)?;
            let object_type: String = row.get(1)?;
            Ok(DbObjectRef {
                database: database.to_string(),
                schema: database.to_string(),
                name,
                kind: sqlite_object_kind(&object_type),
            })
        })?;
        let objects = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read SQLite catalog rows")?;

        Ok(DatabaseEntry {
            name: database.to_string(),
            schemas: vec![SchemaEntry {
                database: database.to_string(),
                name: database.to_string(),
                objects,
            }],
        })
    }
}

pub fn database_path_from_url(url: &str) -> Result<String> {
    if matches!(
        url,
        "sqlite::memory:" | "sqlite3::memory:" | "sqlite://:memory:"
    ) {
        return Ok(":memory:".to_string());
    }

    let parsed = Url::parse(url).context("failed to parse SQLite url")?;
    if !matches!(parsed.scheme(), "sqlite" | "sqlite3") {
        bail!("unsupported SQLite url scheme: {}", parsed.scheme());
    }

    let path = parsed.path();
    if matches!(path, "/:memory:" | ":memory:") {
        return Ok(":memory:".to_string());
    }
    if !path.is_empty() && path != "/" {
        return Ok(path.to_string());
    }
    if let Some(host) = parsed.host_str().filter(|host| !host.is_empty()) {
        return Ok(host.to_string());
    }

    bail!("SQLite url must include a database path")
}

fn execute_statement(connection: &Connection, sql: &str) -> Result<SqlExecutionResult> {
    let mut statement = connection
        .prepare(sql)
        .with_context(|| format!("failed to prepare SQLite statement `{sql}`"))?;
    let column_count = statement.column_count();
    if column_count > 0 {
        let columns = statement
            .column_names()
            .into_iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let rows = statement.query_map([], |row| row_to_strings(row, column_count))?;
        return Ok(SqlExecutionResult::Query(QueryResult {
            columns,
            rows: rows
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("failed to read SQLite query rows")?,
        }));
    }

    let rows_affected = statement
        .execute([])
        .with_context(|| format!("failed to execute SQLite statement `{sql}`"))?;
    Ok(SqlExecutionResult::Command(CommandResult {
        tag: command_tag(sql),
        rows_affected: rows_affected as u64,
    }))
}

fn row_to_strings(row: &Row<'_>, column_count: usize) -> rusqlite::Result<Vec<String>> {
    (0..column_count)
        .map(|index| value_ref_to_string(row.get_ref(index)?))
        .collect()
}

fn value_ref_to_string(value: ValueRef<'_>) -> rusqlite::Result<String> {
    Ok(match value {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(value) => value.to_string(),
        ValueRef::Real(value) => value.to_string(),
        ValueRef::Text(value) => String::from_utf8_lossy(value).to_string(),
        ValueRef::Blob(value) => format!("<{} bytes blob>", value.len()),
    })
}

fn sqlite_object_kind(object_type: &str) -> DbObjectKind {
    match object_type {
        "view" => DbObjectKind::View,
        _ => DbObjectKind::Table,
    }
}

fn sqlite_object_type(kind: DbObjectKind) -> Option<&'static str> {
    match kind {
        DbObjectKind::Table => Some("table"),
        DbObjectKind::View => Some("view"),
        DbObjectKind::MaterializedView => None,
        DbObjectKind::ForeignTable => None,
        DbObjectKind::Function => None,
    }
}

fn parse_column_row(row: &Row<'_>) -> rusqlite::Result<DbColumn> {
    Ok(DbColumn {
        name: row.get(1)?,
        data_type: row.get::<_, String>(2)?,
        nullable: row.get::<_, i64>(3)? == 0,
        has_default: row.get::<_, Option<String>>(4)?.is_some(),
        is_unique: false,
        is_primary_key: row.get::<_, i64>(5)? > 0,
    })
}

fn sqlite_qualified_name(schema: &str, table: &str) -> String {
    format!("{}.{}", quoted_identifier(schema), quoted_identifier(table))
}

fn quoted_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn quoted_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn escape_like_pattern(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
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
    use super::*;

    fn user_table() -> DbObjectRef {
        DbObjectRef {
            database: "main".to_string(),
            schema: "main".to_string(),
            name: "users".to_string(),
            kind: DbObjectKind::Table,
        }
    }

    #[test]
    fn sqlite_url_parser_supports_files_and_memory() {
        assert_eq!(
            database_path_from_url("sqlite::memory:").unwrap(),
            ":memory:"
        );
        assert_eq!(
            database_path_from_url("sqlite:///tmp/relora.db").unwrap(),
            "/tmp/relora.db"
        );
    }

    #[test]
    fn sqlite_driver_loads_catalog_columns_preview_and_sql_results() -> Result<()> {
        let mut driver = SqliteDriver::connect("sqlite::memory:")?;
        driver.execute_sql(
            None,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT NOT NULL);
             INSERT INTO users (email) VALUES ('ada@example.com'), ('grace@example.com');",
        )?;

        let catalog = driver.load_catalog()?;
        assert_eq!(catalog.databases[0].name, "main");
        assert_eq!(catalog.databases[0].schemas[0].objects[0], user_table());

        let columns = driver.load_object_columns(&user_table())?;
        assert_eq!(columns[0].name, "id");
        assert!(columns[0].is_primary_key);
        assert!(!columns[1].nullable);

        let preview = driver.load_preview(&user_table(), 10)?;
        assert_eq!(preview.columns, vec!["id", "email"]);
        assert_eq!(preview.rows.len(), 2);

        let filtered = driver.load_filtered_preview(&user_table(), "ada", 10)?;
        assert_eq!(filtered.rows, vec![vec!["1", "ada@example.com"]]);

        let results = driver.execute_sql(None, "SELECT email FROM users WHERE id = 2;")?;
        assert_eq!(
            results[0].clone().into_preview().rows,
            vec![vec!["grace@example.com"]]
        );
        Ok(())
    }

    #[test]
    fn sqlite_preview_uses_query_columns_when_table_info_is_incomplete() -> Result<()> {
        let mut driver = SqliteDriver::connect("sqlite::memory:")?;
        driver.execute_sql(
            None,
            "CREATE TABLE metrics (
                base INTEGER NOT NULL,
                doubled INTEGER GENERATED ALWAYS AS (base * 2) STORED
             );
             INSERT INTO metrics (base) VALUES (3), (5);",
        )?;

        let table = DbObjectRef {
            database: "main".to_string(),
            schema: "main".to_string(),
            name: "metrics".to_string(),
            kind: DbObjectKind::Table,
        };

        let columns = driver.load_object_columns(&table)?;
        assert_eq!(
            columns
                .into_iter()
                .map(|column| column.name)
                .collect::<Vec<_>>(),
            vec!["base", "doubled"]
        );

        let preview = driver.load_preview(&table, 10)?;
        assert_eq!(preview.columns, vec!["base", "doubled"]);
        assert_eq!(preview.rows, vec![vec!["3", "6"], vec!["5", "10"]]);

        let filtered = driver.load_filtered_preview(&table, "6", 10)?;
        assert_eq!(filtered.columns, vec!["base", "doubled"]);
        assert_eq!(filtered.rows, vec![vec!["3", "6"]]);
        Ok(())
    }
}
