use std::collections::BTreeMap;

use anyhow::{Context, Result};
use mysql::{Opts, Params, Pool, PooledConn, Row, Value, prelude::Queryable};
use relora_core::db::{
    Catalog, CatalogSummary, CommandResult, DatabaseDriver, DatabaseEntry, DatabaseKind,
    DatabaseSummary, DbColumn, DbObjectKind, DbObjectRef, ObjectKindCount, QueryResult,
    SchemaEntry, SchemaSummary, SqlExecutionResult, TablePreview,
};
use url::Url;

const CATALOG_SQL: &str = r#"
    SELECT table_schema, table_name, table_type
    FROM information_schema.tables
    WHERE table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys')
      AND table_type IN ('BASE TABLE', 'VIEW')
    ORDER BY table_schema, table_name
"#;

const CATALOG_SUMMARY_SQL: &str = r#"
    SELECT table_schema, table_type, COUNT(*) AS object_count
    FROM information_schema.tables
    WHERE table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys')
      AND table_type IN ('BASE TABLE', 'VIEW')
    GROUP BY table_schema, table_type
    ORDER BY table_schema, table_type
"#;

const COLUMN_SQL: &str = r#"
    SELECT
        c.column_name,
        c.data_type,
        c.is_nullable,
        CASE WHEN c.column_default IS NULL THEN 0 ELSE 1 END AS has_default,
        CASE WHEN kcu.column_name IS NULL THEN 0 ELSE 1 END AS is_primary_key
    FROM information_schema.columns c
    LEFT JOIN information_schema.table_constraints tc
      ON tc.table_schema = c.table_schema
     AND tc.table_name = c.table_name
     AND tc.constraint_type = 'PRIMARY KEY'
    LEFT JOIN information_schema.key_column_usage kcu
      ON kcu.constraint_schema = tc.constraint_schema
     AND kcu.constraint_name = tc.constraint_name
     AND kcu.table_schema = c.table_schema
     AND kcu.table_name = c.table_name
     AND kcu.column_name = c.column_name
    WHERE c.table_schema = ?
      AND c.table_name = ?
    ORDER BY c.ordinal_position
"#;

pub struct MySqlDriver {
    pool: Pool,
    connection_label: String,
}

impl MySqlDriver {
    pub fn connect(url: &str) -> Result<Self> {
        let opts = Opts::from_url(url).context("failed to parse MySQL/MariaDB url")?;
        let pool = Pool::new(opts).context("failed to create MySQL/MariaDB pool")?;
        Ok(Self {
            pool,
            connection_label: mysql_connection_label(url),
        })
    }

    fn connection(&self) -> Result<PooledConn> {
        self.pool
            .get_conn()
            .context("failed to connect to MySQL/MariaDB")
    }

    fn load_columns(&mut self, table: &DbObjectRef) -> Result<Vec<String>> {
        Ok(self
            .load_object_columns(table)?
            .into_iter()
            .map(|column| column.name)
            .collect())
    }
}

impl DatabaseDriver for MySqlDriver {
    fn kind(&self) -> DatabaseKind {
        DatabaseKind::MySql
    }

    fn connection_label(&self) -> &str {
        &self.connection_label
    }

    fn load_catalog(&mut self) -> Result<Catalog> {
        let mut connection = self.connection()?;
        let rows = connection
            .query::<(String, String, String), _>(CATALOG_SQL)
            .context("failed to query MySQL/MariaDB catalog")?;
        let mut databases: BTreeMap<String, Vec<DbObjectRef>> = BTreeMap::new();

        for (schema, name, table_type) in rows {
            databases
                .entry(schema.clone())
                .or_default()
                .push(DbObjectRef {
                    database: schema.clone(),
                    schema,
                    name,
                    kind: mysql_object_kind(&table_type),
                });
        }

        Ok(Catalog {
            databases: databases
                .into_iter()
                .map(|(database, objects)| DatabaseEntry {
                    name: database.clone(),
                    schemas: vec![SchemaEntry {
                        database: database.clone(),
                        name: database,
                        objects,
                    }],
                })
                .collect(),
        })
    }

    fn load_catalog_summary(&mut self) -> Result<CatalogSummary> {
        let mut connection = self.connection()?;
        let rows = connection
            .query::<(String, String, u64), _>(CATALOG_SUMMARY_SQL)
            .context("failed to query MySQL/MariaDB catalog summary")?;
        let mut databases: BTreeMap<String, Vec<ObjectKindCount>> = BTreeMap::new();

        for (schema, table_type, object_count) in rows {
            databases.entry(schema).or_default().push(ObjectKindCount {
                kind: mysql_object_kind(&table_type),
                count: object_count as usize,
            });
        }

        Ok(CatalogSummary {
            databases: databases
                .into_iter()
                .map(|(database, object_counts)| DatabaseSummary {
                    name: database.clone(),
                    schemas: vec![SchemaSummary {
                        database: database.clone(),
                        name: database,
                        object_counts,
                    }],
                })
                .collect(),
        })
    }

    fn load_schema_objects(&mut self, database: &str, schema: &str) -> Result<Vec<DbObjectRef>> {
        let mut connection = self.connection()?;
        let rows = connection
            .exec::<(String, String, String), _, _>(
                "SELECT table_schema, table_name, table_type
                 FROM information_schema.tables
                 WHERE table_schema = ?
                   AND table_type IN ('BASE TABLE', 'VIEW')
                 ORDER BY table_type, table_name",
                (schema,),
            )
            .with_context(|| {
                format!("failed to query MySQL/MariaDB schema objects for {database}.{schema}")
            })?;

        Ok(rows
            .into_iter()
            .map(|(schema, name, table_type)| DbObjectRef {
                database: database.to_string(),
                schema,
                name,
                kind: mysql_object_kind(&table_type),
            })
            .collect())
    }

    fn load_schema_objects_of_kind(
        &mut self,
        database: &str,
        schema: &str,
        kind: DbObjectKind,
    ) -> Result<Vec<DbObjectRef>> {
        let Some(table_type) = mysql_table_type(kind) else {
            return Ok(Vec::new());
        };
        let mut connection = self.connection()?;
        let rows = connection
            .exec::<(String, String, String), _, _>(
                "SELECT table_schema, table_name, table_type
                 FROM information_schema.tables
                 WHERE table_schema = ?
                   AND table_type = ?
                 ORDER BY table_name",
                (schema, table_type),
            )
            .with_context(|| {
                format!(
                    "failed to query MySQL/MariaDB {} for {database}.{schema}",
                    kind.group_label()
                )
            })?;

        Ok(rows
            .into_iter()
            .map(|(schema, name, table_type)| DbObjectRef {
                database: database.to_string(),
                schema,
                name,
                kind: mysql_object_kind(&table_type),
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

        let sql = format!(
            "SELECT * FROM {} LIMIT {} OFFSET {}",
            mysql_qualified_name(&table.schema, &table.name),
            limit.max(1),
            offset
        );
        let mut connection = self.connection()?;
        let result = query_result(&mut connection, &sql, Params::Empty).with_context(|| {
            format!(
                "failed to preview MySQL/MariaDB object {}",
                table.database_qualified_name()
            )
        })?;

        Ok(result.into())
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
            .map(|column| format!("CAST({} AS CHAR) LIKE ?", quoted_identifier(column)))
            .collect::<Vec<_>>()
            .join(" OR ");
        let sql = format!(
            "SELECT * FROM {} WHERE {} LIMIT {} OFFSET {}",
            mysql_qualified_name(&table.schema, &table.name),
            predicate,
            limit.max(1),
            offset
        );
        let pattern = format!("%{}%", filter);
        let params = Params::Positional(
            columns
                .iter()
                .map(|_| Value::from(pattern.clone()))
                .collect(),
        );
        let mut connection = self.connection()?;
        let result = query_result(&mut connection, &sql, params).with_context(|| {
            format!(
                "failed to filter MySQL/MariaDB object {}",
                table.database_qualified_name()
            )
        })?;

        Ok(result.into())
    }

    fn load_object_columns(&mut self, table: &DbObjectRef) -> Result<Vec<DbColumn>> {
        let mut connection = self.connection()?;
        let rows = connection
            .exec::<(String, String, String, u8, u8), _, _>(
                COLUMN_SQL,
                (&table.schema, &table.name),
            )
            .with_context(|| format!("failed to describe {}", table.database_qualified_name()))?;

        Ok(rows
            .into_iter()
            .map(
                |(name, data_type, is_nullable, has_default, is_primary_key)| DbColumn {
                    name,
                    data_type,
                    nullable: is_nullable == "YES",
                    has_default: has_default > 0,
                    is_primary_key: is_primary_key > 0,
                },
            )
            .collect())
    }

    fn execute_sql(
        &mut self,
        database: Option<&str>,
        sql: &str,
    ) -> Result<Vec<SqlExecutionResult>> {
        let mut connection = self.connection()?;
        if let Some(database) = database {
            connection
                .query_drop(format!("USE {}", quoted_identifier(database)))
                .with_context(|| {
                    format!("failed to switch MySQL/MariaDB database to {database}")
                })?;
        }

        split_sql_statements(sql)
            .into_iter()
            .map(|statement| execute_statement(&mut connection, statement))
            .collect()
    }
}

fn query_result(connection: &mut PooledConn, sql: &str, params: Params) -> Result<QueryResult> {
    let mut result = connection
        .exec_iter(sql, params)
        .with_context(|| format!("failed to execute MySQL/MariaDB query `{sql}`"))?;
    let columns = result
        .columns()
        .as_ref()
        .iter()
        .map(|column| column.name_str().to_string())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();

    for row in result.by_ref() {
        let row = row.context("failed to read MySQL/MariaDB row")?;
        rows.push(row_to_strings(&row, columns.len()));
    }

    Ok(QueryResult { columns, rows })
}

fn execute_statement(connection: &mut PooledConn, sql: &str) -> Result<SqlExecutionResult> {
    let mut result = connection
        .query_iter(sql)
        .with_context(|| format!("failed to execute MySQL/MariaDB statement `{sql}`"))?;
    let columns = result
        .columns()
        .as_ref()
        .iter()
        .map(|column| column.name_str().to_string())
        .collect::<Vec<_>>();

    if columns.is_empty() {
        return Ok(SqlExecutionResult::Command(CommandResult {
            tag: command_tag(sql),
            rows_affected: result.affected_rows(),
        }));
    }

    let mut rows = Vec::new();
    for row in result.by_ref() {
        let row = row.context("failed to read MySQL/MariaDB row")?;
        rows.push(row_to_strings(&row, columns.len()));
    }

    Ok(SqlExecutionResult::Query(QueryResult { columns, rows }))
}

fn row_to_strings(row: &Row, column_count: usize) -> Vec<String> {
    (0..column_count)
        .map(|index| row.as_ref(index).map(value_to_string).unwrap_or_default())
        .collect()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::NULL => "NULL".to_string(),
        Value::Bytes(value) => String::from_utf8_lossy(value).to_string(),
        Value::Int(value) => value.to_string(),
        Value::UInt(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Double(value) => value.to_string(),
        Value::Date(year, month, day, hour, minute, second, micros) => {
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{micros:06}")
        }
        Value::Time(negative, days, hours, minutes, seconds, micros) => {
            let sign = if *negative { "-" } else { "" };
            format!("{sign}{days} {hours:02}:{minutes:02}:{seconds:02}.{micros:06}")
        }
    }
}

fn mysql_connection_label(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return "mysql".to_string();
    };

    let host = parsed.host_str().unwrap_or("localhost");
    let port = parsed
        .port()
        .map(|value| format!(":{value}"))
        .unwrap_or_default();
    let database = parsed.path().trim_start_matches('/');
    if database.is_empty() {
        format!("{}://{host}{port}", parsed.scheme())
    } else {
        format!("{}://{host}{port}/{database}", parsed.scheme())
    }
}

fn mysql_object_kind(table_type: &str) -> DbObjectKind {
    match table_type {
        "VIEW" => DbObjectKind::View,
        _ => DbObjectKind::Table,
    }
}

fn mysql_table_type(kind: DbObjectKind) -> Option<&'static str> {
    match kind {
        DbObjectKind::Table => Some("BASE TABLE"),
        DbObjectKind::View => Some("VIEW"),
        DbObjectKind::MaterializedView => None,
        DbObjectKind::ForeignTable => None,
        DbObjectKind::Function => None,
    }
}

fn mysql_qualified_name(schema: &str, table: &str) -> String {
    format!("{}.{}", quoted_identifier(schema), quoted_identifier(table))
}

fn quoted_identifier(identifier: &str) -> String {
    format!("`{}`", identifier.replace('`', "``"))
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

    #[test]
    fn mysql_connection_label_hides_credentials() {
        assert_eq!(
            mysql_connection_label("mysql://root:secret@localhost:3306/app"),
            "mysql://localhost:3306/app"
        );
        assert_eq!(
            mysql_connection_label("mariadb://root:secret@db.internal/app"),
            "mariadb://db.internal/app"
        );
    }

    #[test]
    fn mysql_identifier_quoting_escapes_backticks() {
        assert_eq!(quoted_identifier("weird`name"), "`weird``name`");
        assert_eq!(
            mysql_qualified_name("app", "users"),
            "`app`.`users`".to_string()
        );
    }

    #[test]
    fn mysql_statement_splitter_ignores_empty_statements() {
        assert_eq!(
            split_sql_statements("SELECT 1; ; UPDATE users SET name = 'a';"),
            vec!["SELECT 1", "UPDATE users SET name = 'a'"]
        );
    }
}
