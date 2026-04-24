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
    MaterializedView,
    ForeignTable,
    Function,
}

impl DbObjectKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Table => "Table",
            Self::View => "View",
            Self::MaterializedView => "Materialized View",
            Self::ForeignTable => "Foreign Table",
            Self::Function => "Function",
        }
    }

    pub fn group_label(self) -> &'static str {
        match self {
            Self::Table => "Tables",
            Self::View => "Views",
            Self::MaterializedView => "Materialized Views",
            Self::ForeignTable => "Foreign Tables",
            Self::Function => "Functions",
        }
    }

    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::View => "view",
            Self::MaterializedView => "materialized-view",
            Self::ForeignTable => "foreign-table",
            Self::Function => "function",
        }
    }

    pub fn from_wire_name(value: &str) -> Option<Self> {
        match value {
            "table" | "tables" => Some(Self::Table),
            "view" | "views" => Some(Self::View),
            "materialized-view" | "materialized_view" | "materialized view"
            | "materialized-views" | "materialized_views" | "materialized views" => {
                Some(Self::MaterializedView)
            }
            "foreign-table" | "foreign_table" | "foreign table" | "foreign-tables"
            | "foreign_tables" | "foreign tables" => Some(Self::ForeignTable),
            "function" | "functions" | "routine" | "routines" => Some(Self::Function),
            _ => None,
        }
    }

    pub fn ordered() -> [Self; 5] {
        [
            Self::Table,
            Self::View,
            Self::MaterializedView,
            Self::ForeignTable,
            Self::Function,
        ]
    }

    pub fn supports_data_preview(self) -> bool {
        matches!(
            self,
            Self::Table | Self::View | Self::MaterializedView | Self::ForeignTable
        )
    }

    pub fn supports_structure(self) -> bool {
        true
    }

    pub fn supports_crud_templates(self) -> bool {
        matches!(self, Self::Table | Self::ForeignTable)
    }

    pub fn supports_staged_crud(self) -> bool {
        matches!(self, Self::Table | Self::ForeignTable)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub databases: Vec<DatabaseEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CatalogSummary {
    pub databases: Vec<DatabaseSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseEntry {
    pub name: String,
    pub schemas: Vec<SchemaEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseSummary {
    pub name: String,
    pub schemas: Vec<SchemaSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaEntry {
    pub database: String,
    pub name: String,
    pub objects: Vec<DbObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaSummary {
    pub database: String,
    pub name: String,
    pub object_counts: Vec<ObjectKindCount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectKindCount {
    pub kind: DbObjectKind,
    pub count: usize,
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

impl CatalogSummary {
    pub fn as_catalog_with_unloaded_objects(&self) -> Catalog {
        Catalog {
            databases: self
                .databases
                .iter()
                .map(|database| DatabaseEntry {
                    name: database.name.clone(),
                    schemas: database
                        .schemas
                        .iter()
                        .map(|schema| SchemaEntry {
                            database: schema.database.clone(),
                            name: schema.name.clone(),
                            objects: Vec::new(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn schema_count(&self) -> usize {
        self.databases
            .iter()
            .map(|database| database.schemas.len())
            .sum()
    }

    pub fn object_count(&self) -> usize {
        self.databases
            .iter()
            .map(DatabaseSummary::object_count)
            .sum()
    }

    pub fn find_schema(&self, database_name: &str, schema_name: &str) -> Option<&SchemaSummary> {
        self.databases
            .iter()
            .find(|database| database.name == database_name)?
            .schemas
            .iter()
            .find(|schema| schema.name == schema_name)
    }
}

impl DatabaseSummary {
    pub fn object_count(&self) -> usize {
        self.schemas
            .iter()
            .map(SchemaSummary::total_object_count)
            .sum()
    }
}

impl SchemaSummary {
    pub fn object_count(&self, kind: DbObjectKind) -> usize {
        self.object_counts
            .iter()
            .find(|entry| entry.kind == kind)
            .map(|entry| entry.count)
            .unwrap_or_default()
    }

    pub fn total_object_count(&self) -> usize {
        self.object_counts.iter().map(|entry| entry.count).sum()
    }
}

impl From<&Catalog> for CatalogSummary {
    fn from(value: &Catalog) -> Self {
        Self {
            databases: value
                .databases
                .iter()
                .map(|database| DatabaseSummary {
                    name: database.name.clone(),
                    schemas: database
                        .schemas
                        .iter()
                        .map(|schema| SchemaSummary {
                            database: schema.database.clone(),
                            name: schema.name.clone(),
                            object_counts: DbObjectKind::ordered()
                                .into_iter()
                                .map(|kind| ObjectKindCount {
                                    kind,
                                    count: schema.object_count(kind),
                                })
                                .filter(|entry| entry.count > 0)
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl From<Catalog> for CatalogSummary {
    fn from(value: Catalog) -> Self {
        Self::from(&value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub has_default: bool,
    #[serde(default)]
    pub is_unique: bool,
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
    fn load_catalog_summary(&mut self) -> Result<CatalogSummary> {
        self.load_catalog().map(CatalogSummary::from)
    }
    fn load_schema_objects(&mut self, database: &str, schema: &str) -> Result<Vec<DbObjectRef>> {
        let catalog = self.load_catalog()?;
        catalog
            .databases
            .into_iter()
            .find(|entry| entry.name == database)
            .and_then(|entry| entry.schemas.into_iter().find(|entry| entry.name == schema))
            .map(|schema| schema.objects)
            .ok_or_else(|| anyhow::anyhow!("schema not found: {database}.{schema}"))
    }
    fn load_schema_objects_of_kind(
        &mut self,
        database: &str,
        schema: &str,
        kind: DbObjectKind,
    ) -> Result<Vec<DbObjectRef>> {
        Ok(self
            .load_schema_objects(database, schema)?
            .into_iter()
            .filter(|object| object.kind == kind)
            .collect())
    }
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

#[cfg(test)]
mod tests {
    use super::{DbColumn, DbObjectKind};

    #[test]
    fn object_kind_supports_materialized_views_and_functions_on_the_wire() {
        assert_eq!(
            DbObjectKind::from_wire_name("materialized-view"),
            Some(DbObjectKind::MaterializedView)
        );
        assert_eq!(
            DbObjectKind::from_wire_name("function"),
            Some(DbObjectKind::Function)
        );
        assert_eq!(
            DbObjectKind::MaterializedView.wire_name(),
            "materialized-view"
        );
        assert_eq!(DbObjectKind::Function.wire_name(), "function");
    }

    #[test]
    fn object_kind_order_includes_postgres_materialized_views_and_functions() {
        assert_eq!(
            DbObjectKind::ordered(),
            [
                DbObjectKind::Table,
                DbObjectKind::View,
                DbObjectKind::MaterializedView,
                DbObjectKind::ForeignTable,
                DbObjectKind::Function,
            ]
        );
    }

    #[test]
    fn db_column_deserializes_old_sidecar_payloads_without_unique_field() {
        let json = r#"{
            "name":"id",
            "data_type":"integer",
            "nullable":false,
            "has_default":false,
            "is_primary_key":true
        }"#;

        let column: DbColumn = serde_json::from_str(json).expect("legacy payload should decode");

        assert_eq!(column.name, "id");
        assert!(!column.is_unique);
        assert!(column.is_primary_key);
    }
}
