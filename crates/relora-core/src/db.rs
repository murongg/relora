use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseKind {
    Postgres,
    MySql,
    Sqlite,
}

impl DatabaseKind {
    pub fn from_url(url: &str) -> Result<Self> {
        let parsed = Url::parse(url).context("failed to parse database url")?;
        match parsed.scheme() {
            "postgres" | "postgresql" => Ok(Self::Postgres),
            "mysql" | "mariadb" => Ok(Self::MySql),
            "sqlite" | "sqlite3" => Ok(Self::Sqlite),
            other => bail!("unsupported database scheme: {other}"),
        }
    }

    pub fn collapses_duplicate_schema(self, database: &str, schema: &str) -> bool {
        matches!(self, Self::MySql | Self::Sqlite) && database == schema
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentifierQuoteStyle {
    DoubleQuote,
    Backtick,
}

impl IdentifierQuoteStyle {
    pub fn quote_identifier(self, value: &str) -> String {
        match self {
            Self::DoubleQuote => format!("\"{}\"", value.replace('"', "\"\"")),
            Self::Backtick => format!("`{}`", value.replace('`', "``")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExplainFlavor {
    Explain,
    ExplainQueryPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverCapabilities {
    pub identifier_quote_style: IdentifierQuoteStyle,
    pub supports_crud_templates: bool,
    pub supports_staged_crud: bool,
    pub supports_sql_completion: bool,
    pub supports_explain: bool,
    pub supports_explain_analyze: bool,
    pub supports_returning: bool,
    pub explain_flavor: ExplainFlavor,
}

impl DriverCapabilities {
    pub fn for_kind(kind: DatabaseKind) -> Self {
        match kind {
            DatabaseKind::Postgres => Self {
                identifier_quote_style: IdentifierQuoteStyle::DoubleQuote,
                supports_crud_templates: true,
                supports_staged_crud: true,
                supports_sql_completion: true,
                supports_explain: true,
                supports_explain_analyze: true,
                supports_returning: true,
                explain_flavor: ExplainFlavor::Explain,
            },
            DatabaseKind::MySql => Self {
                identifier_quote_style: IdentifierQuoteStyle::Backtick,
                supports_crud_templates: true,
                supports_staged_crud: true,
                supports_sql_completion: true,
                supports_explain: true,
                supports_explain_analyze: false,
                supports_returning: false,
                explain_flavor: ExplainFlavor::Explain,
            },
            DatabaseKind::Sqlite => Self {
                identifier_quote_style: IdentifierQuoteStyle::DoubleQuote,
                supports_crud_templates: true,
                supports_staged_crud: true,
                supports_sql_completion: true,
                supports_explain: true,
                supports_explain_analyze: false,
                supports_returning: false,
                explain_flavor: ExplainFlavor::ExplainQueryPlan,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DbObjectKind {
    Table,
    View,
    ForeignTable,
}

impl DbObjectKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Table => "Table",
            Self::View => "View",
            Self::ForeignTable => "Foreign Table",
        }
    }

    pub fn group_label(self) -> &'static str {
        match self {
            Self::Table => "Tables",
            Self::View => "Views",
            Self::ForeignTable => "Foreign Tables",
        }
    }

    pub fn ordered() -> [Self; 3] {
        [Self::Table, Self::View, Self::ForeignTable]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub databases: Vec<DatabaseEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseEntry {
    pub name: String,
    pub schemas: Vec<SchemaEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaEntry {
    pub database: String,
    pub name: String,
    pub objects: Vec<DbObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbObjectRef {
    pub database: String,
    pub schema: String,
    pub name: String,
    pub kind: DbObjectKind,
}

impl DbObjectRef {
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }

    pub fn database_qualified_name(&self) -> String {
        format!("{}.{}.{}", self.database, self.schema, self.name)
    }
}

impl SchemaEntry {
    pub fn objects_of_kind(&self, kind: DbObjectKind) -> impl Iterator<Item = &DbObjectRef> {
        self.objects
            .iter()
            .filter(move |object| object.kind == kind)
    }

    pub fn object_count(&self, kind: DbObjectKind) -> usize {
        self.objects_of_kind(kind).count()
    }
}

impl DatabaseEntry {
    pub fn schema_count(&self) -> usize {
        self.schemas.len()
    }

    pub fn object_count(&self) -> usize {
        self.schemas.iter().map(|schema| schema.objects.len()).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub has_default: bool,
    pub is_primary_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TablePreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl QueryResult {
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}

impl From<QueryResult> for TablePreview {
    fn from(value: QueryResult) -> Self {
        Self {
            columns: value.columns,
            rows: value.rows,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    pub tag: String,
    pub rows_affected: u64,
}

impl CommandResult {
    pub fn summary(&self) -> String {
        format!("{} affected {} row(s).", self.tag, self.rows_affected)
    }
}

impl From<CommandResult> for TablePreview {
    fn from(value: CommandResult) -> Self {
        Self {
            columns: vec!["tag".to_string(), "rows_affected".to_string()],
            rows: vec![vec![value.tag, value.rows_affected.to_string()]],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlExecutionResult {
    Query(QueryResult),
    Command(CommandResult),
}

impl SqlExecutionResult {
    pub fn into_preview(self) -> TablePreview {
        match self {
            Self::Query(result) => result.into(),
            Self::Command(result) => result.into(),
        }
    }
}

pub trait DatabaseDriver: Send {
    fn kind(&self) -> DatabaseKind;
    fn capabilities(&self) -> DriverCapabilities {
        DriverCapabilities::for_kind(self.kind())
    }
    fn connection_label(&self) -> &str;
    fn load_catalog(&mut self) -> Result<Catalog>;
    fn load_preview_page(
        &mut self,
        table: &DbObjectRef,
        limit: usize,
        offset: usize,
    ) -> Result<TablePreview>;
    fn load_preview(&mut self, table: &DbObjectRef, limit: usize) -> Result<TablePreview> {
        self.load_preview_page(table, limit, 0)
    }
    fn load_filtered_preview_page(
        &mut self,
        table: &DbObjectRef,
        filter: &str,
        limit: usize,
        offset: usize,
    ) -> Result<TablePreview>;
    fn load_filtered_preview(
        &mut self,
        table: &DbObjectRef,
        filter: &str,
        limit: usize,
    ) -> Result<TablePreview> {
        self.load_filtered_preview_page(table, filter, limit, 0)
    }
    fn load_object_columns(&mut self, table: &DbObjectRef) -> Result<Vec<DbColumn>>;
    fn execute_sql(&mut self, database: Option<&str>, sql: &str)
    -> Result<Vec<SqlExecutionResult>>;
}
