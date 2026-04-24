use relora_core::db::{
    DatabaseKind, DbColumn, DbObjectRef, DriverCapabilities, IdentifierQuoteStyle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateTableColumnTemplate<'a> {
    pub name: &'a str,
    pub data_type: &'a str,
    pub default_value: Option<&'a str>,
    pub nullable: bool,
    pub unique: bool,
    pub auto_increment: bool,
    pub primary_key: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlterColumnTemplate<'a> {
    pub old_name: &'a str,
    pub new_name: &'a str,
    pub old_data_type: &'a str,
    pub new_data_type: &'a str,
    pub old_nullable: bool,
    pub new_nullable: bool,
    pub old_default: Option<&'a str>,
    pub new_default: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddColumnTemplate<'a> {
    pub name: &'a str,
    pub data_type: &'a str,
    pub nullable: bool,
    pub default_value: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenameTableTemplate<'a> {
    pub old_name: &'a str,
    pub new_name: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateIndexTemplate<'a> {
    pub index_name: &'a str,
    pub column_name: &'a str,
    pub unique: bool,
}

pub fn select_template(
    capabilities: DriverCapabilities,
    object: &DbObjectRef,
    limit: usize,
) -> String {
    if object.kind == relora_core::db::DbObjectKind::Function {
        return format!(
            "SELECT {};",
            callable_name(capabilities.identifier_quote_style, object)
        );
    }

    format!(
        "SELECT *\nFROM {}\nLIMIT {};",
        qualified_name(capabilities.identifier_quote_style, object),
        limit.max(1)
    )
}

pub fn insert_template(
    capabilities: DriverCapabilities,
    object: &DbObjectRef,
    columns: &[DbColumn],
) -> String {
    let editable = editable_columns(columns);
    let column_list = editable
        .iter()
        .map(|column| {
            format!(
                "    {}",
                quote_identifier(capabilities.identifier_quote_style, &column.name)
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let values = editable
        .iter()
        .map(|column| format!("    /* {} */", column.data_type))
        .collect::<Vec<_>>()
        .join(",\n");

    let mut sql = format!(
        "INSERT INTO {} (\n{}\n)\nVALUES (\n{}\n)",
        qualified_name(capabilities.identifier_quote_style, object),
        column_list,
        values
    );
    if capabilities.supports_returning {
        sql.push_str("\nRETURNING *");
    }
    sql.push(';');
    sql
}

pub fn update_template(
    capabilities: DriverCapabilities,
    object: &DbObjectRef,
    columns: &[DbColumn],
) -> String {
    let editable = editable_columns(columns);
    let assignments = editable
        .iter()
        .map(|column| {
            format!(
                "    {} = /* {} */",
                quote_identifier(capabilities.identifier_quote_style, &column.name),
                column.data_type
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let predicate = predicate_template(capabilities.identifier_quote_style, columns);

    let mut sql = format!(
        "UPDATE {}\nSET\n{}\nWHERE {}",
        qualified_name(capabilities.identifier_quote_style, object),
        assignments,
        predicate
    );
    if capabilities.supports_returning {
        sql.push_str("\nRETURNING *");
    }
    sql.push(';');
    sql
}

pub fn delete_template(
    capabilities: DriverCapabilities,
    object: &DbObjectRef,
    columns: &[DbColumn],
) -> String {
    let predicate = predicate_template(capabilities.identifier_quote_style, columns);
    let mut sql = format!(
        "DELETE FROM {}\nWHERE {}",
        qualified_name(capabilities.identifier_quote_style, object),
        predicate
    );
    if capabilities.supports_returning {
        sql.push_str("\nRETURNING *");
    }
    sql.push(';');
    sql
}

pub fn create_table_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    columns: &[CreateTableColumnTemplate<'_>],
) -> String {
    let column_list = columns
        .iter()
        .map(|column| {
            if kind == DatabaseKind::Sqlite && column.auto_increment {
                return format!(
                    "    {} INTEGER PRIMARY KEY AUTOINCREMENT",
                    quote_identifier(quote_style, column.name)
                );
            }

            let mut line = format!(
                "    {} {}",
                quote_identifier(quote_style, column.name),
                create_table_column_type(kind, column)
            );
            if let Some(default_value) = column
                .default_value
                .filter(|value| !value.trim().is_empty())
            {
                line.push_str(" DEFAULT ");
                line.push_str(default_value.trim());
            }
            if !column.nullable {
                line.push_str(" NOT NULL");
            }
            if column.unique {
                line.push_str(" UNIQUE");
            }
            if kind == DatabaseKind::MySql && column.auto_increment {
                line.push_str(" AUTO_INCREMENT");
            }
            if column.primary_key {
                line.push_str(" PRIMARY KEY");
            }
            line
        })
        .collect::<Vec<_>>()
        .join(",\n");

    format!(
        "CREATE TABLE {}.{} (\n{}\n);",
        quote_identifier(quote_style, schema),
        quote_identifier(quote_style, table_name),
        column_list
    )
}

pub fn alter_column_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    column: AlterColumnTemplate<'_>,
) -> String {
    match kind {
        DatabaseKind::Postgres => {
            alter_column_template_postgres(quote_style, schema, table_name, column)
        }
        DatabaseKind::MySql => alter_column_template_mysql(quote_style, schema, table_name, column),
        DatabaseKind::Sqlite => {
            alter_column_template_sqlite(quote_style, schema, table_name, column)
        }
    }
}

pub fn add_column_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    column: AddColumnTemplate<'_>,
) -> String {
    match kind {
        DatabaseKind::Postgres | DatabaseKind::MySql | DatabaseKind::Sqlite => {
            let mut sql = format!(
                "ALTER TABLE {}.{}\n    ADD COLUMN {} {}",
                quote_identifier(quote_style, schema),
                quote_identifier(quote_style, table_name),
                quote_identifier(quote_style, column.name),
                column.data_type.trim()
            );
            if let Some(default_value) = column
                .default_value
                .filter(|value| !value.trim().is_empty())
            {
                sql.push_str(" DEFAULT ");
                sql.push_str(default_value.trim());
            }
            if !column.nullable {
                sql.push_str(" NOT NULL");
            }
            sql.push(';');
            sql
        }
    }
}

pub fn rename_table_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table: RenameTableTemplate<'_>,
) -> String {
    match kind {
        DatabaseKind::MySql => format!(
            "RENAME TABLE {}.{} TO {}.{};",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table.old_name),
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table.new_name)
        ),
        DatabaseKind::Postgres | DatabaseKind::Sqlite => format!(
            "ALTER TABLE {}.{}\n    RENAME TO {};",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table.old_name),
            quote_identifier(quote_style, table.new_name)
        ),
    }
}

pub fn drop_column_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    column_name: &str,
) -> String {
    match kind {
        DatabaseKind::Postgres | DatabaseKind::MySql => format!(
            "ALTER TABLE {}.{}\n    DROP COLUMN {};",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name),
            quote_identifier(quote_style, column_name)
        ),
        DatabaseKind::Sqlite => format!(
            "-- SQLite cannot directly drop columns in every supported environment.\n-- Rebuild the table manually to remove {} from {}.{}.",
            quote_identifier(quote_style, column_name),
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name)
        ),
    }
}

pub fn create_index_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    index: CreateIndexTemplate<'_>,
) -> String {
    let unique = if index.unique { "UNIQUE " } else { "" };
    match kind {
        DatabaseKind::Postgres | DatabaseKind::MySql | DatabaseKind::Sqlite => format!(
            "CREATE {unique}INDEX {}\n    ON {}.{} ({});",
            quote_identifier(quote_style, index.index_name),
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name),
            quote_identifier(quote_style, index.column_name)
        ),
    }
}

pub fn drop_index_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    index_name: &str,
) -> String {
    match kind {
        DatabaseKind::MySql => format!(
            "DROP INDEX {}\n    ON {}.{};",
            quote_identifier(quote_style, index_name),
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name)
        ),
        DatabaseKind::Postgres | DatabaseKind::Sqlite => format!(
            "DROP INDEX {}.{};",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, index_name)
        ),
    }
}

pub fn add_primary_key_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    column_name: &str,
) -> String {
    match kind {
        DatabaseKind::Postgres => format!(
            "ALTER TABLE {}.{}\n    ADD CONSTRAINT {} PRIMARY KEY ({});",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name),
            quote_identifier(quote_style, &primary_key_constraint_name(table_name)),
            quote_identifier(quote_style, column_name)
        ),
        DatabaseKind::MySql => format!(
            "ALTER TABLE {}.{}\n    ADD PRIMARY KEY ({});",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name),
            quote_identifier(quote_style, column_name)
        ),
        DatabaseKind::Sqlite => format!(
            "-- SQLite cannot directly add a primary key constraint.\n-- Rebuild {}.{} with {} as the PRIMARY KEY.",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name),
            quote_identifier(quote_style, column_name)
        ),
    }
}

pub fn drop_primary_key_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
) -> String {
    match kind {
        DatabaseKind::Postgres => format!(
            "ALTER TABLE {}.{}\n    DROP CONSTRAINT IF EXISTS {};",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name),
            quote_identifier(quote_style, &primary_key_constraint_name(table_name))
        ),
        DatabaseKind::MySql => format!(
            "ALTER TABLE {}.{}\n    DROP PRIMARY KEY;",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name)
        ),
        DatabaseKind::Sqlite => format!(
            "-- SQLite cannot directly drop a primary key constraint.\n-- Rebuild {}.{} without the PRIMARY KEY definition.",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name)
        ),
    }
}

pub fn add_unique_constraint_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    column_name: &str,
) -> String {
    match kind {
        DatabaseKind::Postgres => format!(
            "ALTER TABLE {}.{}\n    ADD CONSTRAINT {} UNIQUE ({});",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name),
            quote_identifier(
                quote_style,
                &unique_constraint_name(table_name, column_name)
            ),
            quote_identifier(quote_style, column_name)
        ),
        DatabaseKind::MySql | DatabaseKind::Sqlite => create_index_template(
            kind,
            quote_style,
            schema,
            table_name,
            CreateIndexTemplate {
                index_name: &unique_constraint_name(table_name, column_name),
                column_name,
                unique: true,
            },
        ),
    }
}

pub fn drop_unique_constraint_template(
    kind: DatabaseKind,
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    column_name: &str,
) -> String {
    let index_name = unique_constraint_name(table_name, column_name);
    match kind {
        DatabaseKind::Postgres => format!(
            "ALTER TABLE {}.{}\n    DROP CONSTRAINT IF EXISTS {};",
            quote_identifier(quote_style, schema),
            quote_identifier(quote_style, table_name),
            quote_identifier(quote_style, &index_name)
        ),
        DatabaseKind::MySql | DatabaseKind::Sqlite => {
            drop_index_template(kind, quote_style, schema, table_name, &index_name)
        }
    }
}

fn alter_column_template_postgres(
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    column: AlterColumnTemplate<'_>,
) -> String {
    let table = format!(
        "{}.{}",
        quote_identifier(quote_style, schema),
        quote_identifier(quote_style, table_name)
    );
    let mut statements = Vec::new();
    let active_name = if column.old_name != column.new_name {
        statements.push(format!(
            "ALTER TABLE {table}\n    RENAME COLUMN {} TO {};",
            quote_identifier(quote_style, column.old_name),
            quote_identifier(quote_style, column.new_name)
        ));
        column.new_name
    } else {
        column.old_name
    };

    if !column
        .old_data_type
        .eq_ignore_ascii_case(column.new_data_type)
    {
        statements.push(format!(
            "ALTER TABLE {table}\n    ALTER COLUMN {} TYPE {};",
            quote_identifier(quote_style, active_name),
            column.new_data_type.trim()
        ));
    }

    if column.old_nullable != column.new_nullable {
        let nullability = if column.new_nullable {
            "DROP NOT NULL"
        } else {
            "SET NOT NULL"
        };
        statements.push(format!(
            "ALTER TABLE {table}\n    ALTER COLUMN {} {nullability};",
            quote_identifier(quote_style, active_name)
        ));
    }

    if normalize_optional_sql(column.old_default) != normalize_optional_sql(column.new_default) {
        if let Some(default_value) = column.new_default.filter(|value| !value.trim().is_empty()) {
            statements.push(format!(
                "ALTER TABLE {table}\n    ALTER COLUMN {} SET DEFAULT {};",
                quote_identifier(quote_style, active_name),
                default_value.trim()
            ));
        } else {
            statements.push(format!(
                "ALTER TABLE {table}\n    ALTER COLUMN {} DROP DEFAULT;",
                quote_identifier(quote_style, active_name)
            ));
        }
    }

    if statements.is_empty() {
        format!("-- No structural changes for {table}.")
    } else {
        statements.join("\n")
    }
}

fn primary_key_constraint_name(table_name: &str) -> String {
    format!("{table_name}_pkey")
}

fn unique_constraint_name(table_name: &str, column_name: &str) -> String {
    format!("{table_name}_{column_name}_key")
}

fn alter_column_template_mysql(
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    column: AlterColumnTemplate<'_>,
) -> String {
    let nullability = if column.new_nullable {
        "NULL"
    } else {
        "NOT NULL"
    };
    format!(
        "ALTER TABLE {}.{}\n    CHANGE COLUMN {} {} {} {};",
        quote_identifier(quote_style, schema),
        quote_identifier(quote_style, table_name),
        quote_identifier(quote_style, column.old_name),
        quote_identifier(quote_style, column.new_name),
        column.new_data_type.trim(),
        nullability
    )
}

fn alter_column_template_sqlite(
    quote_style: IdentifierQuoteStyle,
    schema: &str,
    table_name: &str,
    column: AlterColumnTemplate<'_>,
) -> String {
    let table = format!(
        "{}.{}",
        quote_identifier(quote_style, schema),
        quote_identifier(quote_style, table_name)
    );
    let mut statements = Vec::new();
    if column.old_name != column.new_name {
        statements.push(format!(
            "ALTER TABLE {table}\n    RENAME COLUMN {} TO {};",
            quote_identifier(quote_style, column.old_name),
            quote_identifier(quote_style, column.new_name)
        ));
    }
    if !column
        .old_data_type
        .eq_ignore_ascii_case(column.new_data_type)
        || column.old_nullable != column.new_nullable
    {
        statements.push(
            "-- SQLite cannot directly alter column type or nullability; rebuild the table manually."
                .to_string(),
        );
    }

    if statements.is_empty() {
        format!("-- No structural changes for {table}.")
    } else {
        statements.join("\n")
    }
}

fn create_table_column_type<'a>(
    kind: DatabaseKind,
    column: &'a CreateTableColumnTemplate<'a>,
) -> &'a str {
    if !column.auto_increment || kind != DatabaseKind::Postgres {
        return column.data_type;
    }

    if column.data_type.eq_ignore_ascii_case("bigint") {
        "bigserial"
    } else {
        "serial"
    }
}

fn normalize_optional_sql(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn editable_columns(columns: &[DbColumn]) -> Vec<&DbColumn> {
    let editable = columns
        .iter()
        .filter(|column| !column.is_primary_key)
        .collect::<Vec<_>>();
    if editable.is_empty() {
        columns.iter().collect()
    } else {
        editable
    }
}

fn predicate_template(quote_style: IdentifierQuoteStyle, columns: &[DbColumn]) -> String {
    let keys = columns
        .iter()
        .filter(|column| column.is_primary_key)
        .collect::<Vec<_>>();
    let predicate_columns = if keys.is_empty() {
        columns.first().into_iter().collect::<Vec<_>>()
    } else {
        keys
    };

    predicate_columns
        .iter()
        .map(|column| {
            format!(
                "{} = /* {} */",
                quote_identifier(quote_style, &column.name),
                column.data_type
            )
        })
        .collect::<Vec<_>>()
        .join("\n  AND ")
}

fn qualified_name(quote_style: IdentifierQuoteStyle, object: &DbObjectRef) -> String {
    format!(
        "{}.{}",
        quote_identifier(quote_style, &object.schema),
        quote_identifier(quote_style, &object.name)
    )
}

fn callable_name(quote_style: IdentifierQuoteStyle, object: &DbObjectRef) -> String {
    let (function_name, signature) = split_function_signature(&object.name);
    let args = if signature.is_some_and(str::is_empty) {
        "()"
    } else {
        "(/* args */)"
    };

    format!(
        "{}.{}{}",
        quote_identifier(quote_style, &object.schema),
        quote_identifier(quote_style, function_name),
        args
    )
}

fn split_function_signature(name: &str) -> (&str, Option<&str>) {
    let Some(open_paren) = name.find('(') else {
        return (name, None);
    };
    let Some(close_paren) = name.rfind(')') else {
        return (name, None);
    };
    if close_paren < open_paren {
        return (name, None);
    }

    (
        &name[..open_paren],
        Some(name[open_paren + 1..close_paren].trim()),
    )
}

fn quote_identifier(quote_style: IdentifierQuoteStyle, value: &str) -> String {
    quote_style.quote_identifier(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use relora_core::db::{DatabaseKind, DbObjectKind, DriverCapabilities};

    fn object() -> DbObjectRef {
        DbObjectRef {
            database: "app".to_string(),
            schema: "public".to_string(),
            name: "users".to_string(),
            kind: DbObjectKind::Table,
        }
    }

    fn columns() -> Vec<DbColumn> {
        vec![
            DbColumn {
                name: "id".to_string(),
                data_type: "integer".to_string(),
                nullable: false,
                has_default: true,
                is_unique: false,
                is_primary_key: true,
            },
            DbColumn {
                name: "email".to_string(),
                data_type: "text".to_string(),
                nullable: false,
                has_default: false,
                is_unique: false,
                is_primary_key: false,
            },
        ]
    }

    #[test]
    fn postgres_templates_use_double_quotes_and_returning() {
        let capabilities = DriverCapabilities::for_kind(DatabaseKind::Postgres);
        let object = object();
        let columns = columns();

        let insert_sql = insert_template(capabilities, &object, &columns);
        let update_sql = update_template(capabilities, &object, &columns);
        let delete_sql = delete_template(capabilities, &object, &columns);

        assert!(insert_sql.contains("INSERT INTO \"public\".\"users\""));
        assert!(insert_sql.contains("RETURNING *;"));
        assert!(update_sql.contains("WHERE \"id\" ="));
        assert!(update_sql.contains("RETURNING *;"));
        assert!(delete_sql.contains("DELETE FROM \"public\".\"users\""));
        assert!(delete_sql.contains("RETURNING *;"));
    }

    #[test]
    fn mysql_templates_use_backticks_without_returning() {
        let capabilities = DriverCapabilities::for_kind(DatabaseKind::MySql);
        let object = object();
        let columns = columns();

        let select_sql = select_template(capabilities, &object, 100);
        let insert_sql = insert_template(capabilities, &object, &columns);
        let update_sql = update_template(capabilities, &object, &columns);
        let delete_sql = delete_template(capabilities, &object, &columns);

        assert!(select_sql.contains("FROM `public`.`users`"));
        assert!(insert_sql.contains("INSERT INTO `public`.`users`"));
        assert!(insert_sql.contains("`email`"));
        assert!(!insert_sql.contains("RETURNING *;"));
        assert!(update_sql.contains("WHERE `id` ="));
        assert!(!update_sql.contains("RETURNING *;"));
        assert!(delete_sql.contains("DELETE FROM `public`.`users`"));
        assert!(!delete_sql.contains("RETURNING *;"));
    }

    #[test]
    fn function_select_template_uses_a_function_call_instead_of_from_clause() {
        let capabilities = DriverCapabilities::for_kind(DatabaseKind::Postgres);
        let object = DbObjectRef {
            database: "app".to_string(),
            schema: "public".to_string(),
            name: "refresh_sales".to_string(),
            kind: DbObjectKind::Function,
        };

        let sql = select_template(capabilities, &object, 100);

        assert_eq!(sql, "SELECT \"public\".\"refresh_sales\"(/* args */);");
    }

    #[test]
    fn create_table_template_quotes_identifiers_and_renders_constraints() {
        let sql = create_table_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "audit_log",
            &[
                CreateTableColumnTemplate {
                    name: "id",
                    data_type: "bigint",
                    default_value: None,
                    nullable: false,
                    unique: false,
                    auto_increment: false,
                    primary_key: true,
                },
                CreateTableColumnTemplate {
                    name: "message",
                    data_type: "text",
                    default_value: Some("'hello'"),
                    nullable: true,
                    unique: true,
                    auto_increment: false,
                    primary_key: false,
                },
            ],
        );

        assert!(sql.contains("CREATE TABLE \"public\".\"audit_log\""));
        assert!(sql.contains("\"id\" bigint NOT NULL PRIMARY KEY"));
        assert!(sql.contains("\"message\" text DEFAULT 'hello' UNIQUE"));
    }

    #[test]
    fn create_table_template_renders_postgres_serial_column() {
        let sql = create_table_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            &[CreateTableColumnTemplate {
                name: "id",
                data_type: "integer",
                default_value: None,
                nullable: false,
                unique: false,
                auto_increment: true,
                primary_key: true,
            }],
        );

        assert!(sql.contains("\"id\" serial NOT NULL PRIMARY KEY"));
    }

    #[test]
    fn create_table_template_renders_mysql_auto_increment_column() {
        let sql = create_table_template(
            DatabaseKind::MySql,
            IdentifierQuoteStyle::Backtick,
            "relora_demo",
            "users",
            &[CreateTableColumnTemplate {
                name: "id",
                data_type: "int",
                default_value: None,
                nullable: false,
                unique: false,
                auto_increment: true,
                primary_key: true,
            }],
        );

        assert!(sql.contains("`id` int NOT NULL AUTO_INCREMENT PRIMARY KEY"));
    }

    #[test]
    fn create_table_template_renders_sqlite_autoincrement_column() {
        let sql = create_table_template(
            DatabaseKind::Sqlite,
            IdentifierQuoteStyle::DoubleQuote,
            "main",
            "users",
            &[CreateTableColumnTemplate {
                name: "id",
                data_type: "INTEGER",
                default_value: None,
                nullable: false,
                unique: false,
                auto_increment: true,
                primary_key: true,
            }],
        );

        assert!(sql.contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"));
        assert!(!sql.contains("AUTOINCREMENT PRIMARY KEY"));
    }

    #[test]
    fn alter_column_template_renders_postgres_rename_type_and_nullability_changes() {
        let sql = alter_column_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            AlterColumnTemplate {
                old_name: "display_name",
                new_name: "name",
                old_data_type: "text",
                new_data_type: "varchar(120)",
                old_nullable: true,
                new_nullable: false,
                old_default: None,
                new_default: None,
            },
        );

        assert!(sql.contains("ALTER TABLE \"public\".\"users\""));
        assert!(sql.contains("RENAME COLUMN \"display_name\" TO \"name\";"));
        assert!(sql.contains("ALTER COLUMN \"name\" TYPE varchar(120);"));
        assert!(sql.contains("ALTER COLUMN \"name\" SET NOT NULL;"));
    }

    #[test]
    fn alter_column_template_renders_postgres_default_changes() {
        let sql = alter_column_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            AlterColumnTemplate {
                old_name: "status",
                new_name: "status",
                old_data_type: "text",
                new_data_type: "text",
                old_nullable: false,
                new_nullable: false,
                old_default: None,
                new_default: Some("'draft'"),
            },
        );

        assert!(sql.contains("ALTER COLUMN \"status\" SET DEFAULT 'draft';"));
    }

    #[test]
    fn add_column_template_renders_postgres_column_definition() {
        let sql = add_column_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            AddColumnTemplate {
                name: "status",
                data_type: "text",
                nullable: false,
                default_value: Some("'draft'"),
            },
        );

        assert!(sql.contains("ALTER TABLE \"public\".\"users\""));
        assert!(sql.contains("ADD COLUMN \"status\" text DEFAULT 'draft' NOT NULL;"));
    }

    #[test]
    fn primary_key_templates_render_postgres_constraint_statements() {
        let drop_sql = drop_primary_key_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
        );
        let add_sql = add_primary_key_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            "email",
        );

        assert!(drop_sql.contains("DROP CONSTRAINT IF EXISTS \"users_pkey\";"));
        assert!(add_sql.contains("ADD CONSTRAINT \"users_pkey\" PRIMARY KEY (\"email\");"));
    }

    #[test]
    fn unique_constraint_templates_render_postgres_constraint_statements() {
        let drop_sql = drop_unique_constraint_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            "handle",
        );
        let add_sql = add_unique_constraint_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            "handle",
        );

        assert!(drop_sql.contains("DROP CONSTRAINT IF EXISTS \"users_handle_key\";"));
        assert!(add_sql.contains("ADD CONSTRAINT \"users_handle_key\" UNIQUE (\"handle\");"));
    }

    #[test]
    fn rename_table_template_renders_postgres_statement() {
        let sql = rename_table_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            RenameTableTemplate {
                old_name: "users",
                new_name: "members",
            },
        );

        assert_eq!(
            sql,
            "ALTER TABLE \"public\".\"users\"\n    RENAME TO \"members\";"
        );
    }

    #[test]
    fn drop_column_template_renders_postgres_statement() {
        let sql = drop_column_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            "status",
        );

        assert_eq!(
            sql,
            "ALTER TABLE \"public\".\"users\"\n    DROP COLUMN \"status\";"
        );
    }

    #[test]
    fn create_index_template_renders_postgres_statement() {
        let sql = create_index_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            CreateIndexTemplate {
                index_name: "users_status_idx",
                column_name: "status",
                unique: true,
            },
        );

        assert_eq!(
            sql,
            "CREATE UNIQUE INDEX \"users_status_idx\"\n    ON \"public\".\"users\" (\"status\");"
        );
    }

    #[test]
    fn drop_index_template_renders_postgres_statement() {
        let sql = drop_index_template(
            DatabaseKind::Postgres,
            IdentifierQuoteStyle::DoubleQuote,
            "public",
            "users",
            "users_status_idx",
        );

        assert_eq!(sql, "DROP INDEX \"public\".\"users_status_idx\";");
    }
}
