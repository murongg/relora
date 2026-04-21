use crate::db::{
    Catalog, DatabaseDriver, DatabaseEntry, DbObjectKind, DbObjectRef, SchemaEntry, TablePreview,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Schemas,
    Objects,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Schemas => Self::Objects,
            Self::Objects => Self::Schemas,
        }
    }

    fn previous(self) -> Self {
        self.next()
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Schemas => "Schemas",
            Self::Objects => "Objects",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    NextItem,
    PreviousItem,
    NextPane,
    PreviousPane,
    Refresh,
    Quit,
}

#[derive(Debug, Clone)]
pub struct App {
    focus: Focus,
    catalog: Catalog,
    selected_database: usize,
    selected_schema: usize,
    selected_object: usize,
    preview: TablePreview,
    status: String,
    connection_label: String,
    preview_limit: usize,
    should_quit: bool,
}

impl App {
    pub fn from_catalog(
        catalog: Catalog,
        connection_label: impl Into<String>,
        preview_limit: usize,
    ) -> Self {
        Self {
            focus: Focus::Schemas,
            catalog,
            selected_database: 0,
            selected_schema: 0,
            selected_object: 0,
            preview: TablePreview::default(),
            status: String::new(),
            connection_label: connection_label.into(),
            preview_limit: preview_limit.max(1),
            should_quit: false,
        }
    }

    pub fn bootstrap(
        driver: &mut dyn DatabaseDriver,
        preview_limit: usize,
    ) -> anyhow::Result<Self> {
        let catalog = driver.load_catalog()?;
        let mut app = Self::from_catalog(catalog, driver.connection_label(), preview_limit);
        app.sync_preview(driver);
        Ok(app)
    }

    pub fn apply_action(&mut self, action: AppAction, driver: &mut dyn DatabaseDriver) {
        match action {
            AppAction::NextItem => self.move_selection(1, driver),
            AppAction::PreviousItem => self.move_selection(-1, driver),
            AppAction::NextPane => {
                self.focus = self.focus.next();
            }
            AppAction::PreviousPane => {
                self.focus = self.focus.previous();
            }
            AppAction::Refresh => self.refresh(driver),
            AppAction::Quit => {
                self.should_quit = true;
            }
        }
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    pub fn focus_title(&self) -> &'static str {
        self.focus.title()
    }

    pub fn schemas(&self) -> &[SchemaEntry] {
        self.catalog
            .databases
            .get(self.selected_database)
            .map(|database| database.schemas.as_slice())
            .unwrap_or(&[])
    }

    pub fn databases(&self) -> &[DatabaseEntry] {
        &self.catalog.databases
    }

    pub fn selected_database_index(&self) -> Option<usize> {
        (!self.catalog.databases.is_empty()).then_some(self.selected_database)
    }

    pub fn selected_database_name(&self) -> Option<&str> {
        self.catalog
            .databases
            .get(self.selected_database)
            .map(|database| database.name.as_str())
    }

    pub fn current_objects(&self) -> &[DbObjectRef] {
        self.schemas()
            .get(self.selected_schema)
            .map(|schema| schema.objects.as_slice())
            .unwrap_or(&[])
    }

    pub fn selected_schema_index(&self) -> Option<usize> {
        (!self.schemas().is_empty()).then_some(self.selected_schema)
    }

    pub fn selected_object_index(&self) -> Option<usize> {
        (!self.current_objects().is_empty()).then_some(self.selected_object)
    }

    pub fn selected_schema_name(&self) -> Option<&str> {
        self.schemas()
            .get(self.selected_schema)
            .map(|schema| schema.name.as_str())
    }

    pub fn selected_object(&self) -> Option<&DbObjectRef> {
        self.current_objects().get(self.selected_object)
    }

    pub fn preview(&self) -> &TablePreview {
        &self.preview
    }

    pub fn preview_limit(&self) -> usize {
        self.preview_limit
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn connection_label(&self) -> &str {
        &self.connection_label
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn refresh(&mut self, driver: &mut dyn DatabaseDriver) {
        self.refresh_catalog(driver);
    }

    pub fn select_object(
        &mut self,
        database_name: &str,
        schema_name: &str,
        object_name: &str,
        driver: &mut dyn DatabaseDriver,
    ) -> anyhow::Result<()> {
        self.select_object_locally(database_name, schema_name, object_name)?;
        self.sync_preview(driver);
        Ok(())
    }

    pub fn select_object_locally(
        &mut self,
        database_name: &str,
        schema_name: &str,
        object_name: &str,
    ) -> anyhow::Result<()> {
        let database_index = self
            .catalog
            .databases
            .iter()
            .position(|database| database.name == database_name)
            .ok_or_else(|| anyhow::anyhow!("database not found: {database_name}"))?;

        let schema_index = self.catalog.databases[database_index]
            .schemas
            .iter()
            .position(|schema| schema.name == schema_name)
            .ok_or_else(|| anyhow::anyhow!("schema not found: {database_name}.{schema_name}"))?;

        let object_index = self.catalog.databases[database_index].schemas[schema_index]
            .objects
            .iter()
            .position(|object| object.name == object_name)
            .ok_or_else(|| {
                anyhow::anyhow!("object not found: {database_name}.{schema_name}.{object_name}")
            })?;

        self.selected_database = database_index;
        self.selected_schema = schema_index;
        self.selected_object = object_index;
        Ok(())
    }

    pub fn select_schema_locally(
        &mut self,
        database_name: &str,
        schema_name: &str,
    ) -> anyhow::Result<()> {
        let database_index = self
            .catalog
            .databases
            .iter()
            .position(|database| database.name == database_name)
            .ok_or_else(|| anyhow::anyhow!("database not found: {database_name}"))?;

        let schema_index = self.catalog.databases[database_index]
            .schemas
            .iter()
            .position(|schema| schema.name == schema_name)
            .ok_or_else(|| anyhow::anyhow!("schema not found: {database_name}.{schema_name}"))?;

        self.selected_database = database_index;
        self.selected_schema = schema_index;
        self.selected_object = 0;
        Ok(())
    }

    pub fn replace_catalog(&mut self, catalog: Catalog) {
        let previous_database = self.selected_database_name().map(str::to_owned);
        let previous_schema = self.selected_schema_name().map(str::to_owned);
        let previous_object = self.selected_object().cloned();

        self.catalog = catalog;
        self.selected_database = previous_database
            .as_deref()
            .and_then(|name| {
                self.catalog
                    .databases
                    .iter()
                    .position(|database| database.name == name)
            })
            .unwrap_or(0);
        self.selected_schema = previous_schema
            .as_deref()
            .and_then(|name| self.schemas().iter().position(|schema| schema.name == name))
            .unwrap_or(0);

        self.selected_object = previous_object
            .as_ref()
            .and_then(|object| {
                self.current_objects().iter().position(|candidate| {
                    candidate.database == object.database
                        && candidate.name == object.name
                        && candidate.kind == object.kind
                })
            })
            .unwrap_or(0);
    }

    pub fn merge_schema_objects(
        &mut self,
        database_name: &str,
        schema_name: &str,
        objects: Vec<DbObjectRef>,
    ) -> anyhow::Result<()> {
        let previous_object = self.selected_object().cloned();
        let database_index = self
            .catalog
            .databases
            .iter()
            .position(|database| database.name == database_name)
            .ok_or_else(|| anyhow::anyhow!("database not found: {database_name}"))?;
        let schema_index = self.catalog.databases[database_index]
            .schemas
            .iter()
            .position(|schema| schema.name == schema_name)
            .ok_or_else(|| anyhow::anyhow!("schema not found: {database_name}.{schema_name}"))?;

        self.catalog.databases[database_index].schemas[schema_index].objects = objects;
        if self.selected_database == database_index && self.selected_schema == schema_index {
            self.selected_object = previous_object
                .as_ref()
                .and_then(|object| {
                    self.catalog.databases[database_index].schemas[schema_index]
                        .objects
                        .iter()
                        .position(|candidate| {
                            candidate.database == object.database
                                && candidate.schema == object.schema
                                && candidate.name == object.name
                                && candidate.kind == object.kind
                        })
                })
                .unwrap_or(0);
        }
        Ok(())
    }

    pub fn merge_schema_objects_of_kind(
        &mut self,
        database_name: &str,
        schema_name: &str,
        kind: DbObjectKind,
        objects: Vec<DbObjectRef>,
    ) -> anyhow::Result<()> {
        let previous_object = self.selected_object().cloned();
        let database_index = self
            .catalog
            .databases
            .iter()
            .position(|database| database.name == database_name)
            .ok_or_else(|| anyhow::anyhow!("database not found: {database_name}"))?;
        let schema_index = self.catalog.databases[database_index]
            .schemas
            .iter()
            .position(|schema| schema.name == schema_name)
            .ok_or_else(|| anyhow::anyhow!("schema not found: {database_name}.{schema_name}"))?;

        let schema_objects =
            &mut self.catalog.databases[database_index].schemas[schema_index].objects;
        schema_objects.retain(|object| object.kind != kind);
        schema_objects.extend(objects.into_iter().filter(|object| object.kind == kind));

        if self.selected_database == database_index && self.selected_schema == schema_index {
            self.selected_object = previous_object
                .as_ref()
                .and_then(|object| {
                    schema_objects.iter().position(|candidate| {
                        candidate.database == object.database
                            && candidate.schema == object.schema
                            && candidate.name == object.name
                            && candidate.kind == object.kind
                    })
                })
                .unwrap_or(0);
        }
        Ok(())
    }

    pub fn objects_for_schema(
        &self,
        database_name: &str,
        schema_name: &str,
    ) -> Option<&[DbObjectRef]> {
        self.catalog
            .databases
            .iter()
            .find(|database| database.name == database_name)?
            .schemas
            .iter()
            .find(|schema| schema.name == schema_name)
            .map(|schema| schema.objects.as_slice())
    }

    pub fn clear_preview(&mut self) {
        self.preview = TablePreview::default();
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    pub fn apply_preview_result(&mut self, result: std::result::Result<TablePreview, String>) {
        let Some(object) = self.selected_object().cloned() else {
            self.preview = TablePreview::default();
            self.status = format!(
                "Connected to {}. No database objects were found.",
                self.connection_label
            );
            return;
        };

        match result {
            Ok(preview) => {
                let row_count = preview.rows.len();
                self.preview = preview;
                self.status = format!(
                    "Browsing {} {} on {} (loaded {} row(s), limit {}).",
                    object.kind.label(),
                    object.qualified_name(),
                    self.connection_label,
                    row_count,
                    self.preview_limit
                );
            }
            Err(error) => {
                self.preview = TablePreview::default();
                self.status = format!(
                    "Connected to {}. Failed to preview {} {}: {error}",
                    self.connection_label,
                    object.kind.label(),
                    object.qualified_name()
                );
            }
        }
    }

    fn move_selection(&mut self, delta: isize, driver: &mut dyn DatabaseDriver) {
        match self.focus {
            Focus::Schemas => {
                let len = self.schemas().len();
                if len == 0 {
                    return;
                }

                self.selected_schema = wrapped_index(self.selected_schema, len, delta);
                self.selected_object = 0;
                self.sync_preview(driver);
            }
            Focus::Objects => {
                let len = self.current_objects().len();
                if len == 0 {
                    return;
                }

                self.selected_object = wrapped_index(self.selected_object, len, delta);
                self.sync_preview(driver);
            }
        }
    }

    fn refresh_catalog(&mut self, driver: &mut dyn DatabaseDriver) {
        match driver.load_catalog() {
            Ok(catalog) => {
                self.replace_catalog(catalog);

                self.sync_preview(driver);
            }
            Err(error) => {
                self.status = format!("Refresh failed: {error}");
            }
        }
    }

    fn sync_preview(&mut self, driver: &mut dyn DatabaseDriver) {
        let result = self
            .selected_object()
            .cloned()
            .map(|object| {
                driver
                    .load_preview(&object, self.preview_limit)
                    .map_err(|error| error.to_string())
            })
            .unwrap_or_else(|| Ok(TablePreview::default()));
        self.apply_preview_result(result);
    }
}

fn wrapped_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let offset = delta.unsigned_abs() % len;
    if delta.is_negative() {
        (current + len - offset) % len
    } else {
        (current + offset) % len
    }
}
