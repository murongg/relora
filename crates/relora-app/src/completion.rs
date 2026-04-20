use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Object,
    Column,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
}

const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "GROUP BY",
    "ORDER BY",
    "LIMIT",
    "INSERT",
    "UPDATE",
    "DELETE",
    "RETURNING",
    "EXPLAIN",
    "ANALYZE",
    "JOIN",
    "LEFT JOIN",
    "RIGHT JOIN",
    "INNER JOIN",
    "ON",
    "VALUES",
    "SET",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
];

pub fn suggest_sql_completions(
    prefix: &str,
    objects: &[String],
    columns: &[String],
) -> Vec<CompletionItem> {
    let normalized_prefix = prefix.trim().to_ascii_lowercase();
    if normalized_prefix.is_empty() {
        return Vec::new();
    }

    let mut seen = BTreeSet::new();
    let mut items = Vec::new();

    for keyword in SQL_KEYWORDS {
        push_completion(
            &mut items,
            &mut seen,
            keyword,
            CompletionKind::Keyword,
            &normalized_prefix,
        );
    }
    for object in objects {
        push_completion(
            &mut items,
            &mut seen,
            object,
            CompletionKind::Object,
            &normalized_prefix,
        );
    }
    for column in columns {
        push_completion(
            &mut items,
            &mut seen,
            column,
            CompletionKind::Column,
            &normalized_prefix,
        );
    }

    items.sort_by_key(|item| (completion_rank(item.kind), item.label.to_ascii_lowercase()));
    items
}

fn push_completion(
    items: &mut Vec<CompletionItem>,
    seen: &mut BTreeSet<String>,
    label: &str,
    kind: CompletionKind,
    prefix: &str,
) {
    if !label.to_ascii_lowercase().starts_with(prefix) {
        return;
    }
    if !seen.insert(label.to_ascii_lowercase()) {
        return;
    }
    items.push(CompletionItem {
        label: label.to_string(),
        kind,
    });
}

fn completion_rank(kind: CompletionKind) -> usize {
    match kind {
        CompletionKind::Column => 0,
        CompletionKind::Object => 1,
        CompletionKind::Keyword => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::{CompletionKind, suggest_sql_completions};

    #[test]
    fn suggest_sql_completions_prioritizes_columns_then_objects_then_keywords() {
        let items =
            suggest_sql_completions("se", &["sessions".to_string()], &["session_id".to_string()]);

        assert_eq!(items[0].kind, CompletionKind::Column);
        assert_eq!(items[0].label, "session_id");
        assert_eq!(items[1].kind, CompletionKind::Object);
        assert_eq!(items[2].kind, CompletionKind::Keyword);
        assert_eq!(items[2].label, "SELECT");
    }
}
