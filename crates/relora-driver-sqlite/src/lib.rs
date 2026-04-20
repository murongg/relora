use anyhow::{Context, Result, bail};
use relora_core::db::{
    Catalog, CommandResult, DatabaseDriver, DatabaseEntry, DatabaseKind, DbColumn, DbObjectKind,
    DbObjectRef, QueryResult, SchemaEntry, SqlExecutionResult, TablePreview,
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
        let sql = format!(
            "PRAGMA {}.table_info({})",
            quoted_identifier(&table.schema),
            quoted_string(&table.name)
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .with_context(|| format!("failed to describe {}", table.database_qualified_name()))?;
        let rows = statement.query_map([], |row| {
            Ok(DbColumn {
                name: row.get(1)?,
                data_type: row.get::<_, String>(2)?,
                nullable: row.get::<_, i64>(3)? == 0,
                has_default: row.get::<_, Option<String>>(4)?.is_some(),
                is_primary_key: row.get::<_, i64>(5)? > 0,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read SQLite columns")
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
}
