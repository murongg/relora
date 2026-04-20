use relora_core::db::{DbColumn, DbObjectRef};

pub fn select_template(object: &DbObjectRef, limit: usize) -> String {
    format!(
        "SELECT *\nFROM {}\nLIMIT {};",
        qualified_name(object),
        limit.max(1)
    )
}

pub fn insert_template(object: &DbObjectRef, columns: &[DbColumn]) -> String {
    let editable = editable_columns(columns);
    let column_list = editable
        .iter()
        .map(|column| format!("    {}", quote_identifier(&column.name)))
        .collect::<Vec<_>>()
        .join(",\n");
    let values = editable
        .iter()
        .map(|column| format!("    /* {} */", column.data_type))
        .collect::<Vec<_>>()
        .join(",\n");

    format!(
        "INSERT INTO {} (\n{}\n)\nVALUES (\n{}\n)\nRETURNING *;",
        qualified_name(object),
        column_list,
        values
    )
}

pub fn update_template(object: &DbObjectRef, columns: &[DbColumn]) -> String {
    let editable = editable_columns(columns);
    let assignments = editable
        .iter()
        .map(|column| {
            format!(
                "    {} = /* {} */",
                quote_identifier(&column.name),
                column.data_type
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let predicate = predicate_template(columns);

    format!(
        "UPDATE {}\nSET\n{}\nWHERE {}\nRETURNING *;",
        qualified_name(object),
        assignments,
        predicate
    )
}

pub fn delete_template(object: &DbObjectRef, columns: &[DbColumn]) -> String {
    let predicate = predicate_template(columns);
    format!(
        "DELETE FROM {}\nWHERE {}\nRETURNING *;",
        qualified_name(object),
        predicate
    )
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

fn predicate_template(columns: &[DbColumn]) -> String {
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
                quote_identifier(&column.name),
                column.data_type
            )
        })
        .collect::<Vec<_>>()
        .join("\n  AND ")
}

fn qualified_name(object: &DbObjectRef) -> String {
    format!(
        "{}.{}",
        quote_identifier(&object.schema),
        quote_identifier(&object.name)
    )
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
