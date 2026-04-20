use relora_core::db::{DbObjectKind, DbObjectRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub label: String,
    pub rendered: String,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
    pub badge: Option<String>,
}

impl TreeRow {
    pub fn new(
        label: impl Into<String>,
        depth: usize,
        expandable: bool,
        expanded: bool,
        badge: Option<String>,
    ) -> Self {
        let label = label.into();
        let rendered = render_label(&label, depth, expandable, expanded, badge.as_deref());
        Self {
            label,
            rendered,
            depth,
            expandable,
            expanded,
            badge,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TreeNodeKey {
    Connection {
        connection: usize,
    },
    Database {
        connection: usize,
        database: String,
    },
    Schema {
        connection: usize,
        database: String,
        schema: String,
    },
    Group {
        connection: usize,
        database: String,
        schema: String,
        kind: DbObjectKind,
    },
    Object {
        connection: usize,
        object: DbObjectRef,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct TreeEntry {
    pub row: TreeRow,
    pub key: TreeNodeKey,
}

fn render_label(
    label: &str,
    depth: usize,
    expandable: bool,
    expanded: bool,
    badge: Option<&str>,
) -> String {
    let indent = "  ".repeat(depth);
    let marker = if expandable {
        if expanded { "[-]" } else { "[+]" }
    } else {
        "   "
    };

    let mut rendered = format!("{indent}{marker} {label}");
    if let Some(badge) = badge {
        rendered.push_str(&format!(" [{badge}]"));
    }
    rendered
}
