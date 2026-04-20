use relora_core::db::{DbColumn, DbObjectRef, TablePreview};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedCrudSql {
    pub preview_sql: String,
    pub commit_sql: String,
}

pub fn explain_sql(statement: &str, analyze: bool) -> String {
    let statement = statement.trim();
    if analyze {
        format!("EXPLAIN ANALYZE {statement}")
    } else {
        format!("EXPLAIN {statement}")
    }
}

pub fn copy_row_text(row: &[String]) -> String {
    row.join("\t")
}

pub fn where_clause_for_row(columns: &[String], row: &[String], key_columns: &[String]) -> String {
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
                quote_identifier(column),
                quote_literal(value)
            ))
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

pub fn staged_update_sql(
    object: &DbObjectRef,
    grid: &TablePreview,
    row_index: usize,
    column_index: usize,
    new_value: &str,
    key_columns: &[String],
) -> Option<StagedCrudSql> {
    let column = grid.columns.get(column_index)?;
    let row = grid.rows.get(row_index)?;
    let predicate = where_clause_for_row(&grid.columns, row, key_columns);
    if predicate.is_empty() {
        return None;
    }

    let update_sql = format!(
        "UPDATE {}\nSET {} = {}\nWHERE {}\nRETURNING *;",
        qualified_name(object),
        quote_identifier(column),
        quote_literal(new_value),
        predicate
    );
    let preview_sql = format!("BEGIN;\n{update_sql}\nROLLBACK;");
    let commit_sql = format!("BEGIN;\n{update_sql}\nCOMMIT;");
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

pub fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub fn quote_literal(value: &str) -> String {
    if value == "NULL" {
        "NULL".to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

pub fn qualified_name(object: &DbObjectRef) -> String {
    format!(
        "{}.{}",
        quote_identifier(&object.schema),
        quote_identifier(&object.name)
    )
}
