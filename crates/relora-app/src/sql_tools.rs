use anyhow::{Result, bail};
use relora_core::db::{
    DbColumn, DbObjectRef, DriverCapabilities, ExplainFlavor, IdentifierQuoteStyle, TablePreview,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedCrudSql {
    pub preview_sql: String,
    pub commit_sql: String,
}

pub fn explain_sql(
    capabilities: DriverCapabilities,
    statement: &str,
    analyze: bool,
) -> Result<String> {
    let statement = statement.trim();
    if !capabilities.supports_explain {
        bail!("the current connection does not support EXPLAIN");
    }

    if analyze {
        if !capabilities.supports_explain_analyze {
            bail!("the current connection does not support EXPLAIN ANALYZE");
        }
        return Ok(format!("EXPLAIN ANALYZE {statement}"));
    }

    Ok(match capabilities.explain_flavor {
        ExplainFlavor::Explain => format!("EXPLAIN {statement}"),
        ExplainFlavor::ExplainQueryPlan => format!("EXPLAIN QUERY PLAN {statement}"),
    })
}

pub fn copy_row_text(row: &[String]) -> String {
    row.join("\t")
}

pub fn where_clause_for_row(
    quote_style: IdentifierQuoteStyle,
    columns: &[String],
    row: &[String],
    key_columns: &[String],
) -> String {
    let predicate_columns = if key_columns.is_empty() {
        columns.iter().collect::<Vec<_>>()
    } else {
        key_columns
            .iter()
            .filter_map(|key| columns.iter().find(|column| *column == key))
            .collect::<Vec<_>>()
    };

    predicate_columns
        .into_iter()
        .filter_map(|column| {
            let index = columns.iter().position(|candidate| candidate == column)?;
            let value = row.get(index)?;
            Some(format!(
                "{} = {}",
                quote_identifier(quote_style, column),
                quote_literal(value)
            ))
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub fn staged_update_sql(
    capabilities: DriverCapabilities,
    object: &DbObjectRef,
    grid: &TablePreview,
    row_index: usize,
    column_index: usize,
    new_value: &str,
    key_columns: &[String],
) -> Option<StagedCrudSql> {
    let column = grid.columns.get(column_index)?;
    let row = grid.rows.get(row_index)?;
    let predicate = where_clause_for_row(
        capabilities.identifier_quote_style,
        &grid.columns,
        row,
        key_columns,
    );
    if predicate.is_empty() {
        return None;
    }

    let update_statement = format!(
        "UPDATE {}\nSET {} = {}\nWHERE {}",
        qualified_name(capabilities.identifier_quote_style, object),
        quote_identifier(capabilities.identifier_quote_style, column),
        quote_literal(new_value),
        predicate
    );
    let result_statement = if capabilities.supports_returning {
        format!("{update_statement}\nRETURNING *;")
    } else {
        format!(
            "{update_statement};\nSELECT *\nFROM {}\nWHERE {};",
            qualified_name(capabilities.identifier_quote_style, object),
            predicate
        )
    };
    let preview_sql = format!("BEGIN;\n{result_statement}\nROLLBACK;");
    let commit_sql = format!("BEGIN;\n{result_statement}\nCOMMIT;");
    Some(StagedCrudSql {
        preview_sql,
        commit_sql,
    })
}

pub fn primary_key_names(columns: &[DbColumn]) -> Vec<String> {
    columns
        .iter()
        .filter(|column| column.is_primary_key)
        .map(|column| column.name.clone())
        .collect()
}

pub fn quote_identifier(quote_style: IdentifierQuoteStyle, value: &str) -> String {
    quote_style.quote_identifier(value)
}

pub fn quote_literal(value: &str) -> String {
    if value == "NULL" {
        "NULL".to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

pub fn qualified_name(quote_style: IdentifierQuoteStyle, object: &DbObjectRef) -> String {
    format!(
        "{}.{}",
        quote_identifier(quote_style, &object.schema),
        quote_identifier(quote_style, &object.name)
    )
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

    fn grid() -> TablePreview {
        TablePreview {
            columns: vec!["id".to_string(), "email".to_string()],
            rows: vec![vec!["1".to_string(), "alice@example.com".to_string()]],
        }
    }

    #[test]
    fn sqlite_explain_uses_query_plan() {
        let sql = explain_sql(
            DriverCapabilities::for_kind(DatabaseKind::Sqlite),
            "select * from users;",
            false,
        )
        .expect("sqlite explain should succeed");

        assert_eq!(sql, "EXPLAIN QUERY PLAN select * from users;");
    }

    #[test]
    fn mysql_explain_analyze_is_rejected() {
        let error = explain_sql(
            DriverCapabilities::for_kind(DatabaseKind::MySql),
            "select * from users;",
            true,
        )
        .expect_err("mysql explain analyze should be rejected");

        assert!(error.to_string().contains("EXPLAIN ANALYZE"));
    }

    #[test]
    fn mysql_staged_update_uses_backticks_and_select_fallback() {
        let sql = staged_update_sql(
            DriverCapabilities::for_kind(DatabaseKind::MySql),
            &object(),
            &grid(),
            0,
            1,
            "new@example.com",
            &["id".to_string()],
        )
        .expect("mysql staged update should be generated");

        assert!(sql.preview_sql.contains("UPDATE `public`.`users`"));
        assert!(sql.preview_sql.contains("SET `email` = 'new@example.com'"));
        assert!(sql.preview_sql.contains("SELECT *"));
        assert!(sql.preview_sql.contains("WHERE `id` = '1';"));
        assert!(!sql.preview_sql.contains("RETURNING *;"));
    }
}
