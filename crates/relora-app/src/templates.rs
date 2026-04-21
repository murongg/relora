use relora_core::db::{DbColumn, DbObjectRef, DriverCapabilities, IdentifierQuoteStyle};

pub fn select_template(
    capabilities: DriverCapabilities,
    object: &DbObjectRef,
    limit: usize,
) -> String {
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
                is_primary_key: true,
            },
            DbColumn {
                name: "email".to_string(),
                data_type: "text".to_string(),
                nullable: false,
                has_default: false,
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
}
