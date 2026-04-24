use super::*;

pub(super) fn handle_shell_key(app: &mut AppShell, key: KeyEvent) -> Result<bool> {
    match app {
        AppShell::Workspace(workspace) => {
            if key.code == KeyCode::Esc
                && workspace.launcher_available()
                && workspace_can_return_to_launcher(workspace)
            {
                if let Some(launcher) = workspace.take_launcher() {
                    *app = AppShell::Launcher(launcher);
                    return Ok(false);
                }
            }
            handle_key(workspace, key)?;
            Ok(workspace.should_quit())
        }
        AppShell::Launcher(_) => handle_launcher_key(app, key),
    }
}

fn workspace_can_return_to_launcher(workspace: &WorkspaceApp) -> bool {
    !workspace.delete_confirmation_open()
        && !workspace.help_overlay_open()
        && !workspace.command_palette_open()
        && !workspace.saved_sql_open()
        && !workspace.save_sql_dialog_open()
        && !workspace.create_table_form_open()
        && !workspace.structure_editor_form_open()
        && !workspace.alter_column_form_open()
        && !workspace.add_column_form_open()
        && !workspace.rename_table_form_open()
        && !workspace.create_index_form_open()
        && !workspace.drop_index_form_open()
        && !workspace.insert_row_form_open()
        && !workspace.sql_history_open()
        && !workspace.data_filter_open()
        && !workspace.cell_edit_open()
        && !workspace.row_inspector_open()
        && !(workspace.active_right_tab() == RightPaneTab::Sql
            && workspace.is_editor_open()
            && (workspace.sql_editor_focused() || workspace.editor_completion_open()))
}

pub(super) fn handle_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(KEY_INTERRUPT) {
        return app.apply_action(WorkspaceAction::Quit);
    }

    if app.delete_confirmation_open() {
        return handle_delete_confirmation_key(app, key);
    }

    if app.help_overlay_open() {
        return handle_help_overlay_key(app, key);
    }

    if app.command_palette_open() {
        return handle_command_palette_key(app, key);
    }

    if app.saved_sql_open() {
        return handle_saved_sql_key(app, key);
    }

    if app.save_sql_dialog_open() {
        return handle_save_sql_dialog_key(app, key);
    }

    if app.create_table_form_open() {
        return handle_create_table_form_key(app, key);
    }

    if app.structure_editor_form_open() {
        return handle_structure_editor_form_key(app, key);
    }

    if app.alter_column_form_open() {
        return handle_alter_column_form_key(app, key);
    }

    if app.add_column_form_open() {
        return handle_add_column_form_key(app, key);
    }

    if app.rename_table_form_open() {
        return handle_rename_table_form_key(app, key);
    }

    if app.create_index_form_open() {
        return handle_create_index_form_key(app, key);
    }

    if app.drop_index_form_open() {
        return handle_drop_index_form_key(app, key);
    }

    if app.insert_row_form_open() {
        return handle_insert_row_form_key(app, key);
    }

    if app.sql_history_open() {
        return handle_sql_history_key(app, key);
    }

    if app.data_filter_open() {
        return handle_data_filter_key(app, key);
    }

    if app.cell_edit_open() {
        return handle_cell_edit_key(app, key);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char(KEY_COMMAND_PALETTE)
    {
        return app.apply_action(WorkspaceAction::OpenCommandPalette);
    }

    if key.code == KeyCode::F(FKEY_SQL_HISTORY)
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char(KEY_SQL_HISTORY))
    {
        return app.apply_action(WorkspaceAction::OpenSqlHistory);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(KEY_SAVED_SQL) {
        return app.apply_action(WorkspaceAction::OpenSavedSql);
    }

    if key.code == KeyCode::F(FKEY_HELP) {
        return app.apply_action(WorkspaceAction::OpenHelpOverlay);
    }

    if is_help_key(key.code) && should_open_help_overlay(app, key) {
        return app.apply_action(WorkspaceAction::OpenHelpOverlay);
    }

    if let Some(action) = map_right_tab_key_to_action(key) {
        return app.apply_action(action);
    }

    if app.row_inspector_open() {
        return handle_row_inspector_key(app, key);
    }

    if key.code == KeyCode::Tab {
        if app.active_right_tab() == RightPaneTab::Sql
            && app.is_editor_open()
            && app.sql_editor_focused()
            && app.editor_completion_open()
        {
            return handle_editor_key(app, key);
        }
        return app.apply_action(WorkspaceAction::ToggleBrowserFocus);
    }

    if key.code == KeyCode::BackTab {
        if app.active_right_tab() == RightPaneTab::Sql
            && app.is_editor_open()
            && app.sql_editor_focused()
            && app.editor_completion_open()
        {
            app.apply_action(WorkspaceAction::CloseEditorCompletion)?;
        }
        return app.apply_action(WorkspaceAction::ReverseBrowserFocus);
    }

    if app.active_right_tab() == RightPaneTab::Structure
        && key.code == KeyCode::Char(KEY_STRUCTURE_EDIT_COLUMN)
    {
        app.apply_action(WorkspaceAction::OpenStructureEditor)?;
        return Ok(());
    }

    if app.active_right_tab() == RightPaneTab::Structure
        && key.code == KeyCode::Char(KEY_STRUCTURE_ADD_COLUMN)
    {
        app.apply_action(WorkspaceAction::OpenAddColumnForm)?;
        return Ok(());
    }

    if app.active_right_tab() == RightPaneTab::Structure
        && key.code == KeyCode::Char(KEY_STRUCTURE_DROP_COLUMN)
    {
        app.apply_action(WorkspaceAction::PromptDropStructureColumn)?;
        return Ok(());
    }

    if app.active_right_tab() == RightPaneTab::Structure
        && key.code == KeyCode::Char(KEY_STRUCTURE_RENAME_TABLE)
    {
        app.apply_action(WorkspaceAction::OpenRenameTableForm)?;
        return Ok(());
    }

    if app.active_right_tab() == RightPaneTab::Structure
        && key.code == KeyCode::Char(KEY_STRUCTURE_CREATE_INDEX)
    {
        app.apply_action(WorkspaceAction::OpenCreateIndexForm)?;
        return Ok(());
    }

    if app.active_right_tab() == RightPaneTab::Structure
        && key.code == KeyCode::Char(KEY_STRUCTURE_DROP_INDEX)
    {
        app.apply_action(WorkspaceAction::OpenDropIndexForm)?;
        return Ok(());
    }

    if app.data_grid_focused() {
        if let Some(action) = map_data_grid_key_to_action(key) {
            app.apply_action(action)?;
            return Ok(());
        }
    }

    if app.is_editor_open()
        && app.active_right_tab() == RightPaneTab::Sql
        && app.sql_editor_focused()
    {
        return handle_editor_key(app, key);
    }

    if let Some(action) = map_browser_key_to_action(key) {
        app.apply_action(action)?;
    }

    Ok(())
}

pub(super) fn handle_launcher_key(app: &mut AppShell, key: KeyEvent) -> Result<bool> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(KEY_INTERRUPT) {
        return Ok(true);
    }

    let preview_limit;
    let outcome = {
        let AppShell::Launcher(launcher) = app else {
            return Ok(false);
        };

        if launcher.form_snapshot().is_some() {
            return handle_launcher_form_key(launcher, key);
        }

        if launcher.pending_delete_connection_name().is_some() {
            let action = match key.code {
                KeyCode::Char(KEY_CONFIRM_YES_LOWER) | KeyCode::Char(KEY_CONFIRM_YES_UPPER) => {
                    Some(LauncherAction::ConfirmDeleteConnection)
                }
                KeyCode::Char(KEY_CONFIRM_NO_LOWER)
                | KeyCode::Char(KEY_CONFIRM_NO_UPPER)
                | KeyCode::Esc => Some(LauncherAction::CancelDeleteConnection),
                _ => None,
            };

            let Some(action) = action else {
                return Ok(false);
            };

            match launcher.apply_action(action) {
                Ok(_) => {}
                Err(error) => launcher.set_status(format!("Delete failed: {error}")),
            }
            return Ok(false);
        }

        let action = match key.code {
            KeyCode::Down | KeyCode::Char(KEY_LAUNCHER_DOWN) => {
                Some(LauncherAction::NextConnection)
            }
            KeyCode::Up | KeyCode::Char(KEY_LAUNCHER_UP) => {
                Some(LauncherAction::PreviousConnection)
            }
            KeyCode::Char(KEY_LAUNCHER_MULTI_SELECT) => {
                Some(LauncherAction::ToggleMarkedConnection)
            }
            KeyCode::Char(KEY_LAUNCHER_NEW_CONNECTION) => {
                Some(LauncherAction::OpenCreateConnectionForm)
            }
            KeyCode::Char(KEY_LAUNCHER_EDIT_CONNECTION) => {
                Some(LauncherAction::OpenEditConnectionForm)
            }
            KeyCode::Char(KEY_LAUNCHER_DELETE_CONNECTION) => {
                Some(LauncherAction::DeleteSelectedConnection)
            }
            KeyCode::Enter => Some(LauncherAction::LaunchSelectedConnections),
            KeyCode::Char(KEY_LAUNCHER_QUIT) | KeyCode::Esc => Some(LauncherAction::Quit),
            _ => None,
        };

        let Some(action) = action else {
            return Ok(false);
        };

        preview_limit = launcher.preview_limit();
        match launcher.apply_action(action) {
            Ok(outcome) => outcome,
            Err(error) => {
                launcher.set_status(format!("Launcher action failed: {error}"));
                return Ok(false);
            }
        }
    };

    match outcome {
        crate::launcher::LauncherOutcome::Stay => Ok(false),
        crate::launcher::LauncherOutcome::Quit => Ok(true),
        crate::launcher::LauncherOutcome::Launch(connections) => {
            let saved_sql = if let AppShell::Launcher(launcher) = app {
                load_saved_sql_from_path(launcher.saved_sql_store_path()).unwrap_or_default()
            } else {
                Vec::new()
            };
            match bootstrap_workspace(&connections, preview_limit, &saved_sql) {
                Ok(workspace) => {
                    let AppShell::Launcher(launcher) = app else {
                        unreachable!("launcher outcome should only be handled from the launcher");
                    };
                    let launcher = launcher.clone_for_workspace_return();
                    *app = AppShell::Workspace(Box::new(WorkspaceShell::with_launcher(
                        workspace, launcher,
                    )));
                }
                Err(error) => {
                    if let AppShell::Launcher(launcher) = app {
                        launcher.set_status(format!("Connection failed: {error}"));
                    }
                }
            }
            Ok(false)
        }
    }
}

pub(super) fn handle_launcher_form_key(launcher: &mut LauncherApp, key: KeyEvent) -> Result<bool> {
    if launcher.sqlite_file_picker_is_open() {
        match key.code {
            KeyCode::Esc => {
                launcher.cancel_sqlite_file_picker();
                return Ok(false);
            }
            KeyCode::Enter => {
                launcher.submit_sqlite_file_picker()?;
                return Ok(false);
            }
            KeyCode::Down | KeyCode::Char(KEY_LAUNCHER_DOWN) => {
                launcher.next_sqlite_file_picker_entry()?;
                return Ok(false);
            }
            KeyCode::Up | KeyCode::Char(KEY_LAUNCHER_UP) => {
                launcher.previous_sqlite_file_picker_entry()?;
                return Ok(false);
            }
            KeyCode::Left | KeyCode::Backspace => {
                launcher.ascend_sqlite_file_picker()?;
                return Ok(false);
            }
            _ => return Ok(false),
        }
    }

    if launcher.pending_missing_driver().is_some() {
        match key.code {
            KeyCode::Enter
            | KeyCode::Char(KEY_CONFIRM_NO_LOWER)
            | KeyCode::Char(KEY_CONFIRM_NO_UPPER)
            | KeyCode::Esc => {
                launcher.cancel_missing_driver_prompt();
                return Ok(false);
            }
            _ => return Ok(false),
        }
    }

    match key.code {
        KeyCode::Tab | KeyCode::Down => {
            launcher.apply_action(LauncherAction::SwitchFormField)?;
        }
        KeyCode::BackTab | KeyCode::Up => {
            launcher.apply_action(LauncherAction::PreviousFormField)?;
        }
        KeyCode::Right => {
            launcher.apply_action(LauncherAction::NextFormDriver)?;
        }
        KeyCode::Left => {
            launcher.apply_action(LauncherAction::PreviousFormDriver)?;
        }
        KeyCode::Enter => match launcher.apply_action(LauncherAction::SubmitConnectionForm) {
            Ok(_) => {}
            Err(error) => launcher.set_status(format!("Save failed: {error}")),
        },
        KeyCode::Esc => {
            launcher.apply_action(LauncherAction::CancelConnectionForm)?;
        }
        KeyCode::Backspace => {
            launcher.backspace_form()?;
        }
        KeyCode::Char(KEY_FORM_OPEN_FILE) | KeyCode::Char('O')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            match launcher.open_sqlite_file_picker() {
                Ok(()) => {}
                Err(error) => launcher.set_status(format!("SQLite browse failed: {error}")),
            }
        }
        KeyCode::Char(KEY_LAUNCHER_TEST_CONNECTION) | KeyCode::Char('T')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            test_launcher_form_connection(launcher);
        }
        KeyCode::Char(KEY_LAUNCHER_TEST_CONNECTION) | KeyCode::Char('T')
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                && launcher
                    .form_snapshot()
                    .is_some_and(|form| form.field == LauncherFormField::Driver) =>
        {
            test_launcher_form_connection(launcher);
        }
        KeyCode::Char(ch)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            launcher.insert_form_char(ch)?;
        }
        _ => {}
    }
    Ok(false)
}

fn test_launcher_form_connection(launcher: &mut LauncherApp) {
    let connection = match launcher.connection_form_config() {
        Ok(connection) => connection,
        Err(error) => {
            launcher.set_status(format!("Connection test failed: {error}"));
            return;
        }
    };

    match drivers::driver_availability_for_url(&connection.url) {
        Ok(drivers::DriverAvailability::Available) => {}
        Ok(drivers::DriverAvailability::Missing(plan)) => {
            launcher.prompt_missing_driver(plan.kind, plan.display_name, plan.binary);
            return;
        }
        Err(error) => {
            launcher.set_status(format!("Connection test failed: {error}"));
            return;
        }
    }

    launcher.set_status(format!("Testing `{}`...", connection.name));
    match drivers::test_connection(&connection) {
        Ok(()) => launcher.set_status(format!("Connection `{}` succeeded.", connection.name)),
        Err(error) => launcher.set_status(format!("Connection test failed: {error}")),
    }
}

pub(super) fn map_right_tab_key_to_action(key: KeyEvent) -> Option<WorkspaceAction> {
    match key.code {
        KeyCode::F(FKEY_TAB_DATA) => Some(WorkspaceAction::SelectRightDataTab),
        KeyCode::F(FKEY_TAB_SQL) => Some(WorkspaceAction::SelectRightSqlTab),
        KeyCode::F(FKEY_TAB_STRUCTURE) => Some(WorkspaceAction::SelectRightStructureTab),
        KeyCode::Char(KEY_ALT_TAB_DATA) if is_tab_digit_modifier(key.modifiers) => {
            Some(WorkspaceAction::SelectRightDataTab)
        }
        KeyCode::Char(KEY_ALT_TAB_SQL) if is_tab_digit_modifier(key.modifiers) => {
            Some(WorkspaceAction::SelectRightSqlTab)
        }
        KeyCode::Char(KEY_ALT_TAB_STRUCTURE) if is_tab_digit_modifier(key.modifiers) => {
            Some(WorkspaceAction::SelectRightStructureTab)
        }
        _ => None,
    }
}

fn is_tab_digit_modifier(modifiers: KeyModifiers) -> bool {
    modifiers == KeyModifiers::ALT
}

pub(super) fn handle_delete_confirmation_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char(KEY_CONFIRM_YES_LOWER) | KeyCode::Char(KEY_CONFIRM_YES_UPPER) => {
            app.apply_action(WorkspaceAction::ConfirmDeleteOperation)
        }
        KeyCode::Char(KEY_CONFIRM_NO_LOWER)
        | KeyCode::Char(KEY_CONFIRM_NO_UPPER)
        | KeyCode::Esc => app.apply_action(WorkspaceAction::CancelDeleteOperation),
        _ => Ok(()),
    }
}

pub(super) fn handle_command_palette_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char(KEY_COMMAND_PALETTE)
    {
        return app.apply_action(WorkspaceAction::CloseCommandPalette);
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseCommandPalette),
        KeyCode::Enter => app.apply_action(WorkspaceAction::ExecuteCommandPaletteSelection),
        KeyCode::Down => app.apply_action(WorkspaceAction::NextCommandPaletteItem),
        KeyCode::Up => app.apply_action(WorkspaceAction::PreviousCommandPaletteItem),
        KeyCode::Backspace => app.backspace_command_palette(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_command_palette_char(ch)
        }
        _ => Ok(()),
    }
}

fn should_open_help_overlay(app: &WorkspaceApp, key: KeyEvent) -> bool {
    !(key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        || (app.active_right_tab() == RightPaneTab::Sql
            && app.is_editor_open()
            && app.sql_editor_focused()))
}

pub(super) fn handle_help_overlay_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::F(FKEY_HELP) => {
            app.apply_action(WorkspaceAction::CloseHelpOverlay)
        }
        code if is_help_key(code) => app.apply_action(WorkspaceAction::CloseHelpOverlay),
        _ => Ok(()),
    }
}

fn is_help_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Char(KEY_HELP) | KeyCode::Char(KEY_HELP_FULLWIDTH)
    )
}

pub(super) fn handle_sql_history_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(KEY_SQL_HISTORY) {
        return app.apply_action(WorkspaceAction::CloseSqlHistory);
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseSqlHistory),
        KeyCode::Enter => app.apply_action(WorkspaceAction::RunSqlHistorySelection),
        KeyCode::Down => app.apply_action(WorkspaceAction::NextSqlHistoryItem),
        KeyCode::Up => app.apply_action(WorkspaceAction::PreviousSqlHistoryItem),
        KeyCode::Backspace => app.backspace_sql_history_search(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_sql_history_search_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_saved_sql_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(KEY_SAVED_SQL) {
        return app.apply_action(WorkspaceAction::CloseSavedSql);
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseSavedSql),
        KeyCode::Enter => app.apply_action(WorkspaceAction::OpenSavedSqlSelection),
        KeyCode::Down => app.apply_action(WorkspaceAction::NextSavedSqlItem),
        KeyCode::Up => app.apply_action(WorkspaceAction::PreviousSavedSqlItem),
        KeyCode::Backspace => app.backspace_saved_sql_search(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_saved_sql_search_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_save_sql_dialog_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseSaveSqlDialog),
        KeyCode::Enter => app.apply_action(WorkspaceAction::ConfirmSaveSql),
        KeyCode::Backspace => app.backspace_save_sql_name(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_save_sql_name_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_insert_row_form_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') => app.clear_insert_row_form_field(),
            _ => Ok(()),
        };
    }

    if app.insert_row_form_selected_field_supports_time_picker() {
        match key.code {
            KeyCode::Left => return app.move_insert_row_form_time_segment(-1),
            KeyCode::Right => return app.move_insert_row_form_time_segment(1),
            KeyCode::Up => return app.adjust_insert_row_form_active_time_segment(1),
            KeyCode::Down => return app.adjust_insert_row_form_active_time_segment(-1),
            KeyCode::PageUp => return app.adjust_insert_row_form_date_months(-1),
            KeyCode::PageDown => return app.adjust_insert_row_form_date_months(1),
            KeyCode::Home => return app.adjust_insert_row_form_date_years(-1),
            KeyCode::End => return app.adjust_insert_row_form_date_years(1),
            KeyCode::Char('t') => return app.set_insert_row_form_date_today(),
            KeyCode::Char('n') => return app.set_insert_row_form_datetime_now(),
            KeyCode::Char('h') => return app.adjust_insert_row_form_time_hours(-1),
            KeyCode::Char('l') => return app.adjust_insert_row_form_time_hours(1),
            KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                return app.adjust_insert_row_form_time_minutes(1);
            }
            KeyCode::Char('m') => return app.adjust_insert_row_form_time_minutes(-1),
            KeyCode::Char('M') => return app.adjust_insert_row_form_time_minutes(1),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                return app.adjust_insert_row_form_time_seconds(1);
            }
            KeyCode::Char('s') => return app.adjust_insert_row_form_time_seconds(-1),
            KeyCode::Char('S') => return app.adjust_insert_row_form_time_seconds(1),
            _ => {}
        }
    }

    if app.insert_row_form_selected_field_supports_date_picker() {
        match key.code {
            KeyCode::Left => return app.adjust_insert_row_form_date_days(-1),
            KeyCode::Right => return app.adjust_insert_row_form_date_days(1),
            KeyCode::Up => return app.adjust_insert_row_form_date_days(1),
            KeyCode::Down => return app.adjust_insert_row_form_date_days(-1),
            KeyCode::PageUp => return app.adjust_insert_row_form_date_months(-1),
            KeyCode::PageDown => return app.adjust_insert_row_form_date_months(1),
            KeyCode::Home => return app.adjust_insert_row_form_date_years(-1),
            KeyCode::End => return app.adjust_insert_row_form_date_years(1),
            KeyCode::Char('t') => return app.set_insert_row_form_date_today(),
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseInsertRowForm),
        KeyCode::Enter => app.apply_action(WorkspaceAction::PreviewInsertRowForm),
        KeyCode::Tab | KeyCode::Char(KEY_BROWSER_DOWN) => {
            app.apply_action(WorkspaceAction::NextInsertRowField)
        }
        KeyCode::BackTab | KeyCode::Char(KEY_BROWSER_UP) => {
            app.apply_action(WorkspaceAction::PreviousInsertRowField)
        }
        KeyCode::Backspace => app.backspace_insert_row_form(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_insert_row_form_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_create_table_form_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Enter => app.preview_and_execute_create_table_form(),
            KeyCode::Char('u') | KeyCode::Char('U') => app.clear_create_table_form_field(),
            _ => Ok(()),
        };
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseCreateTableForm),
        KeyCode::Enter => app.apply_action(WorkspaceAction::PreviewCreateTableForm),
        KeyCode::Tab => app.apply_action(WorkspaceAction::MoveCreateTableFieldRight),
        KeyCode::BackTab => app.apply_action(WorkspaceAction::MoveCreateTableFieldLeft),
        KeyCode::Down | KeyCode::Char(KEY_BROWSER_DOWN) => {
            app.apply_action(WorkspaceAction::NextCreateTableField)
        }
        KeyCode::Up | KeyCode::Char(KEY_BROWSER_UP) => {
            app.apply_action(WorkspaceAction::PreviousCreateTableField)
        }
        KeyCode::Left => {
            if app.create_table_form_selected_field_is_type() {
                app.apply_action(WorkspaceAction::CycleCreateTableColumnTypePrevious)
            } else {
                app.apply_action(WorkspaceAction::MoveCreateTableFieldLeft)
            }
        }
        KeyCode::Right => {
            if app.create_table_form_selected_field_is_type() {
                app.apply_action(WorkspaceAction::CycleCreateTableColumnTypeNext)
            } else {
                app.apply_action(WorkspaceAction::MoveCreateTableFieldRight)
            }
        }
        KeyCode::Backspace => app.backspace_create_table_form(),
        KeyCode::Char(KEY_CREATE_TABLE_ADD_COLUMN) => {
            app.apply_action(WorkspaceAction::AddCreateTableColumn)
        }
        KeyCode::Char(KEY_CREATE_TABLE_REMOVE_COLUMN) => {
            app.apply_action(WorkspaceAction::RemoveCreateTableColumn)
        }
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE)
            if app.create_table_form_selected_field_is_type() =>
        {
            app.apply_action(WorkspaceAction::CycleCreateTableColumnTypeNext)
        }
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE)
            if app.create_table_form_selected_field_is_toggle() =>
        {
            match app
                .create_table_form_snapshot()
                .map(|form| form.selected_focus)
            {
                Some(CreateTableFieldFocusView::Nullable) => {
                    app.apply_action(WorkspaceAction::ToggleCreateTableColumnNullable)
                }
                Some(CreateTableFieldFocusView::Unique) => {
                    app.apply_action(WorkspaceAction::ToggleCreateTableColumnUnique)
                }
                Some(CreateTableFieldFocusView::AutoIncrement) => {
                    app.apply_action(WorkspaceAction::ToggleCreateTableColumnAutoIncrement)
                }
                Some(CreateTableFieldFocusView::PrimaryKey) => {
                    app.apply_action(WorkspaceAction::ToggleCreateTableColumnPrimaryKey)
                }
                _ => Ok(()),
            }
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_create_table_form_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_alter_column_form_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') => app.clear_alter_column_form_field(),
            _ => Ok(()),
        };
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseAlterColumnForm),
        KeyCode::Enter => app.apply_action(WorkspaceAction::PreviewAlterColumnForm),
        KeyCode::Tab => app.apply_action(WorkspaceAction::NextAlterColumnField),
        KeyCode::BackTab => app.apply_action(WorkspaceAction::PreviousAlterColumnField),
        KeyCode::Left if app.alter_column_form_selected_field_is_type() => {
            app.apply_action(WorkspaceAction::CycleAlterColumnTypePrevious)
        }
        KeyCode::Right if app.alter_column_form_selected_field_is_type() => {
            app.apply_action(WorkspaceAction::CycleAlterColumnTypeNext)
        }
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE)
            if app.alter_column_form_selected_field_is_type() =>
        {
            app.apply_action(WorkspaceAction::CycleAlterColumnTypeNext)
        }
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE)
            if app.alter_column_form_selected_field_is_nullable() =>
        {
            app.apply_action(WorkspaceAction::ToggleAlterColumnNullable)
        }
        KeyCode::Backspace => app.backspace_alter_column_form(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_alter_column_form_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_structure_editor_form_key(
    app: &mut WorkspaceApp,
    key: KeyEvent,
) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Enter => app.preview_and_execute_structure_editor_form(),
            KeyCode::Char('u') | KeyCode::Char('U') => app.clear_structure_editor_form_field(),
            _ => Ok(()),
        };
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseStructureEditorForm),
        KeyCode::Enter => app.apply_action(WorkspaceAction::PreviewStructureEditorForm),
        KeyCode::Tab => app.apply_action(WorkspaceAction::MoveStructureEditorFieldRight),
        KeyCode::BackTab => app.apply_action(WorkspaceAction::MoveStructureEditorFieldLeft),
        KeyCode::Down | KeyCode::Char(KEY_BROWSER_DOWN) => {
            app.apply_action(WorkspaceAction::NextStructureEditorField)
        }
        KeyCode::Up | KeyCode::Char(KEY_BROWSER_UP) => {
            app.apply_action(WorkspaceAction::PreviousStructureEditorField)
        }
        KeyCode::Left => {
            if app.structure_editor_form_selected_field_is_type() {
                app.apply_action(WorkspaceAction::CycleStructureEditorColumnTypePrevious)
            } else {
                app.apply_action(WorkspaceAction::MoveStructureEditorFieldLeft)
            }
        }
        KeyCode::Right => {
            if app.structure_editor_form_selected_field_is_type() {
                app.apply_action(WorkspaceAction::CycleStructureEditorColumnTypeNext)
            } else {
                app.apply_action(WorkspaceAction::MoveStructureEditorFieldRight)
            }
        }
        KeyCode::Backspace => app.backspace_structure_editor_form(),
        KeyCode::Char(KEY_CREATE_TABLE_ADD_COLUMN) => {
            app.apply_action(WorkspaceAction::AddStructureEditorColumn)
        }
        KeyCode::Char(KEY_CREATE_TABLE_REMOVE_COLUMN) => {
            app.apply_action(WorkspaceAction::RemoveStructureEditorColumn)
        }
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE)
            if app.structure_editor_form_selected_field_is_type() =>
        {
            app.apply_action(WorkspaceAction::CycleStructureEditorColumnTypeNext)
        }
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE)
            if app.structure_editor_form_selected_field_is_nullable() =>
        {
            app.apply_action(WorkspaceAction::ToggleStructureEditorNullable)
        }
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE)
            if app.structure_editor_form_selected_field_is_toggle() =>
        {
            if app.structure_editor_form_selected_field_is_nullable() {
                app.apply_action(WorkspaceAction::ToggleStructureEditorNullable)
            } else if app.structure_editor_form_selected_field_is_unique() {
                app.apply_action(WorkspaceAction::ToggleStructureEditorUnique)
            } else {
                app.apply_action(WorkspaceAction::ToggleStructureEditorPrimaryKey)
            }
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_structure_editor_form_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_add_column_form_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') => app.clear_add_column_form_field(),
            _ => Ok(()),
        };
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseAddColumnForm),
        KeyCode::Enter => app.apply_action(WorkspaceAction::PreviewAddColumnForm),
        KeyCode::Tab => app.apply_action(WorkspaceAction::NextAddColumnField),
        KeyCode::BackTab => app.apply_action(WorkspaceAction::PreviousAddColumnField),
        KeyCode::Left if app.add_column_form_selected_field_is_type() => {
            app.apply_action(WorkspaceAction::CycleAddColumnTypePrevious)
        }
        KeyCode::Right if app.add_column_form_selected_field_is_type() => {
            app.apply_action(WorkspaceAction::CycleAddColumnTypeNext)
        }
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE) if app.add_column_form_selected_field_is_type() => {
            app.apply_action(WorkspaceAction::CycleAddColumnTypeNext)
        }
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE)
            if app.add_column_form_selected_field_is_nullable() =>
        {
            app.apply_action(WorkspaceAction::ToggleAddColumnNullable)
        }
        KeyCode::Backspace => app.backspace_add_column_form(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_add_column_form_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_rename_table_form_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') => app.clear_rename_table_form(),
            _ => Ok(()),
        };
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseRenameTableForm),
        KeyCode::Enter => app.apply_action(WorkspaceAction::PreviewRenameTableForm),
        KeyCode::Backspace => app.backspace_rename_table_form(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_rename_table_form_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_create_index_form_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') => app.clear_create_index_form(),
            _ => Ok(()),
        };
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseCreateIndexForm),
        KeyCode::Enter => app.apply_action(WorkspaceAction::PreviewCreateIndexForm),
        KeyCode::Char(KEY_CREATE_TABLE_TOGGLE) if app.create_index_form_unique_selected() => {
            app.apply_action(WorkspaceAction::ToggleCreateIndexUnique)
        }
        KeyCode::Backspace => app.backspace_create_index_form(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_create_index_form_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_drop_index_form_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('u') | KeyCode::Char('U') => app.clear_drop_index_form(),
            _ => Ok(()),
        };
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseDropIndexForm),
        KeyCode::Enter => app.apply_action(WorkspaceAction::PreviewDropIndexForm),
        KeyCode::Backspace => app.backspace_drop_index_form(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_drop_index_form_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_data_filter_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseDataFilter),
        KeyCode::Enter => app.apply_action(WorkspaceAction::ApplyDataFilter),
        KeyCode::Backspace => app.backspace_data_filter(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_data_filter_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_cell_edit_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseCellEdit),
        KeyCode::Enter => app.apply_action(WorkspaceAction::PreviewStagedCrud),
        KeyCode::Backspace => app.backspace_cell_edit(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_cell_edit_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_row_inspector_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char(KEY_ROW_INSPECTOR_SCROLL_DOWN) | KeyCode::Char('D') => {
                app.apply_action(WorkspaceAction::PageRowInspectorDetailDown)
            }
            KeyCode::Char(KEY_ROW_INSPECTOR_SCROLL_UP) | KeyCode::Char('U') => {
                app.apply_action(WorkspaceAction::PageRowInspectorDetailUp)
            }
            _ => Ok(()),
        };
    }

    let active_pane = app
        .view()
        .row_inspector
        .map(|inspector| inspector.active_pane)
        .unwrap_or(RowInspectorPane::Fields);

    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char(KEY_ROW_INSPECTOR_QUIT) => {
            app.apply_action(WorkspaceAction::CloseRowInspector)
        }
        KeyCode::Tab => app.apply_action(WorkspaceAction::NextRowInspectorPane),
        KeyCode::BackTab => app.apply_action(WorkspaceAction::PreviousRowInspectorPane),
        KeyCode::Right | KeyCode::Char(KEY_BROWSER_EXPAND_RIGHT) => match active_pane {
            RowInspectorPane::Fields => app.apply_action(WorkspaceAction::NextRowInspectorPane),
            RowInspectorPane::Preview => Ok(()),
        },
        KeyCode::Left | KeyCode::Char(KEY_BROWSER_EXPAND_LEFT) => match active_pane {
            RowInspectorPane::Fields => Ok(()),
            RowInspectorPane::Preview => {
                app.apply_action(WorkspaceAction::PreviousRowInspectorPane)
            }
        },
        KeyCode::Down | KeyCode::Char(KEY_ROW_INSPECTOR_DOWN) => match active_pane {
            RowInspectorPane::Fields => app.apply_action(WorkspaceAction::NextRowInspectorField),
            RowInspectorPane::Preview => {
                app.apply_action(WorkspaceAction::ScrollRowInspectorDetailDown)
            }
        },
        KeyCode::Up | KeyCode::Char(KEY_ROW_INSPECTOR_UP) => match active_pane {
            RowInspectorPane::Fields => {
                app.apply_action(WorkspaceAction::PreviousRowInspectorField)
            }
            RowInspectorPane::Preview => {
                app.apply_action(WorkspaceAction::ScrollRowInspectorDetailUp)
            }
        },
        KeyCode::PageDown => app.apply_action(WorkspaceAction::PageRowInspectorDetailDown),
        KeyCode::PageUp => app.apply_action(WorkspaceAction::PageRowInspectorDetailUp),
        KeyCode::Char(KEY_ROW_INSPECTOR_COPY) | KeyCode::Char(KEY_ROW_INSPECTOR_COPY_UPPER) => {
            app.apply_action(WorkspaceAction::CopyCurrentCell)
        }
        KeyCode::Char(KEY_ROW_INSPECTOR_EDIT) => app.apply_action(WorkspaceAction::StartCellEdit),
        KeyCode::Char(KEY_ROW_INSPECTOR_FORMAT) => app.toggle_row_inspector_format(),
        _ => Ok(()),
    }
}

pub(super) fn handle_row_inspector_mouse(
    app: &mut WorkspaceApp,
    mouse: MouseEvent,
    area: Rect,
) -> Result<()> {
    let popup = centered_rect(
        ROW_INSPECTOR_POPUP_WIDTH_PERCENT,
        ROW_INSPECTOR_POPUP_HEIGHT_PERCENT,
        area,
    );
    if !rect_contains(popup, mouse.column, mouse.row) {
        return Ok(());
    }

    let inner = Block::default().borders(Borders::ALL).inner(popup);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(ROW_INSPECTOR_FIELD_LIST_HEIGHT_PERCENT),
            Constraint::Percentage(ROW_INSPECTOR_DETAIL_HEIGHT_PERCENT),
        ])
        .split(inner);
    let active_pane = app
        .view()
        .row_inspector
        .map(|inspector| inspector.active_pane)
        .unwrap_or(RowInspectorPane::Fields);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left)
            if rect_contains(sections[0], mouse.column, mouse.row) =>
        {
            if matches!(active_pane, RowInspectorPane::Preview) {
                app.apply_action(WorkspaceAction::PreviousRowInspectorPane)
            } else {
                Ok(())
            }
        }
        MouseEventKind::Down(MouseButton::Left)
            if rect_contains(sections[1], mouse.column, mouse.row) =>
        {
            if matches!(active_pane, RowInspectorPane::Fields) {
                app.apply_action(WorkspaceAction::NextRowInspectorPane)
            } else {
                Ok(())
            }
        }
        MouseEventKind::ScrollDown if rect_contains(sections[1], mouse.column, mouse.row) => {
            if matches!(active_pane, RowInspectorPane::Fields) {
                app.apply_action(WorkspaceAction::NextRowInspectorPane)?;
            }
            app.apply_action(WorkspaceAction::ScrollRowInspectorDetailDown)
        }
        MouseEventKind::ScrollUp if rect_contains(sections[1], mouse.column, mouse.row) => {
            if matches!(active_pane, RowInspectorPane::Fields) {
                app.apply_action(WorkspaceAction::NextRowInspectorPane)?;
            }
            app.apply_action(WorkspaceAction::ScrollRowInspectorDetailUp)
        }
        _ => Ok(()),
    }
}

pub(super) fn handle_editor_key(app: &mut WorkspaceApp, key: KeyEvent) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Some(action) = map_editor_control_key_to_action(key) {
            return app.apply_action(action);
        }
        return Ok(());
    }

    if app.editor_completion_open() {
        match key.code {
            KeyCode::Esc => return app.apply_action(WorkspaceAction::CloseEditorCompletion),
            KeyCode::Enter => return app.apply_action(WorkspaceAction::AcceptEditorCompletion),
            KeyCode::Tab => return app.apply_action(WorkspaceAction::AcceptEditorCompletion),
            KeyCode::Down => return app.apply_action(WorkspaceAction::NextEditorCompletion),
            KeyCode::Up => return app.apply_action(WorkspaceAction::PreviousEditorCompletion),
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => app.apply_action(WorkspaceAction::CloseEditor),
        KeyCode::F(FKEY_EDITOR_EXECUTE) => app.apply_action(WorkspaceAction::ExecuteEditor),
        KeyCode::F(FKEY_EDITOR_PREVIOUS_TAB) => {
            app.apply_action(WorkspaceAction::PreviousEditorTab)
        }
        KeyCode::F(FKEY_EDITOR_NEXT_TAB) => app.apply_action(WorkspaceAction::NextEditorTab),
        KeyCode::F(FKEY_EDITOR_PREVIOUS_RESULT) => {
            app.apply_action(WorkspaceAction::PreviousResultSet)
        }
        KeyCode::F(FKEY_EDITOR_NEXT_RESULT) => app.apply_action(WorkspaceAction::NextResultSet),
        KeyCode::F(FKEY_EDITOR_EXPLAIN) => {
            app.apply_action(WorkspaceAction::ExplainCurrentStatement)
        }
        KeyCode::F(FKEY_EDITOR_EXPLAIN_ANALYZE) => {
            app.apply_action(WorkspaceAction::ExplainAnalyzeCurrentStatement)
        }
        KeyCode::Left => app.move_editor_cursor_left(),
        KeyCode::Right => app.move_editor_cursor_right(),
        KeyCode::Up => app.move_editor_cursor_up(),
        KeyCode::Down => app.move_editor_cursor_down(),
        KeyCode::Backspace => app.backspace_editor(),
        KeyCode::Enter => app.newline_editor(),
        KeyCode::Tab => app.insert_editor_tab(),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.insert_editor_char(ch)
        }
        _ => Ok(()),
    }
}

pub(super) fn map_editor_control_key_to_action(key: KeyEvent) -> Option<WorkspaceAction> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }

    match key.code {
        KeyCode::Enter => Some(WorkspaceAction::ExecuteEditor),
        KeyCode::Char(KEY_SAVED_SQL) => Some(WorkspaceAction::OpenSavedSql),
        KeyCode::Char(KEY_EDITOR_SAVE_SQL) => Some(WorkspaceAction::OpenSaveSqlDialog),
        KeyCode::Char(KEY_EDITOR_DELETE_SAVED_SQL) => {
            Some(WorkspaceAction::DeleteSavedSqlFromEditor)
        }
        KeyCode::Char(KEY_EDITOR_NEW_TAB) => Some(WorkspaceAction::NewEditorTab),
        KeyCode::Char(KEY_EDITOR_CLOSE_TAB) => Some(WorkspaceAction::CloseEditorTab),
        KeyCode::Char(KEY_EDITOR_CANCEL_TASKS) => Some(WorkspaceAction::CancelTasks),
        KeyCode::Char(KEY_EDITOR_COMMIT_STAGED) => Some(WorkspaceAction::CommitStagedCrud),
        _ => None,
    }
}

pub(super) fn map_data_grid_key_to_action(key: KeyEvent) -> Option<WorkspaceAction> {
    match key.code {
        KeyCode::Down | KeyCode::Char(KEY_BROWSER_DOWN) => {
            Some(WorkspaceAction::ScrollDataGridDown)
        }
        KeyCode::Up | KeyCode::Char(KEY_BROWSER_UP) => Some(WorkspaceAction::ScrollDataGridUp),
        KeyCode::PageDown => Some(WorkspaceAction::PageDataGridDown),
        KeyCode::PageUp => Some(WorkspaceAction::PageDataGridUp),
        KeyCode::Right | KeyCode::Char(KEY_BROWSER_EXPAND_RIGHT) => {
            Some(WorkspaceAction::ScrollDataGridRight)
        }
        KeyCode::Left | KeyCode::Char(KEY_BROWSER_EXPAND_LEFT) => {
            Some(WorkspaceAction::ScrollDataGridLeft)
        }
        KeyCode::Enter => Some(WorkspaceAction::OpenRowInspector),
        KeyCode::Char(KEY_DATA_GRID_COPY_ROW) => Some(WorkspaceAction::CopyCurrentRow),
        KeyCode::Char(KEY_DATA_GRID_COPY_CELL) => Some(WorkspaceAction::CopyCurrentCell),
        KeyCode::Char(KEY_DATA_GRID_COPY_WHERE) => Some(WorkspaceAction::CopyCurrentWhereClause),
        KeyCode::Char(KEY_DATA_GRID_EDIT_CELL) => Some(WorkspaceAction::StartCellEdit),
        KeyCode::Char(KEY_DATA_GRID_INSERT_ROW) => Some(WorkspaceAction::OpenInsertRowForm),
        KeyCode::Char(KEY_DATA_GRID_DELETE_ROW) => Some(WorkspaceAction::PreviewDeleteCurrentRow),
        KeyCode::Char(KEY_DATA_GRID_NEXT_PAGE) => Some(WorkspaceAction::NextPreviewPage),
        KeyCode::Char(KEY_DATA_GRID_PREVIOUS_PAGE) => Some(WorkspaceAction::PreviousPreviewPage),
        KeyCode::Char(KEY_DATA_GRID_SHRINK_COLUMN) => {
            Some(WorkspaceAction::ShrinkSelectedGridColumn)
        }
        KeyCode::Char(KEY_DATA_GRID_EXPAND_COLUMN) => {
            Some(WorkspaceAction::ExpandSelectedGridColumn)
        }
        KeyCode::Char(KEY_DATA_GRID_RESET_COLUMN) => {
            Some(WorkspaceAction::ResetSelectedGridColumnWidth)
        }
        KeyCode::Char(KEY_DATA_GRID_FREEZE_COLUMNS) => {
            Some(WorkspaceAction::FreezeGridColumnsThroughSelection)
        }
        KeyCode::Char(KEY_DATA_GRID_CLEAR_FROZEN) => Some(WorkspaceAction::ClearFrozenGridColumns),
        KeyCode::Esc => Some(WorkspaceAction::FocusAssets),
        _ => None,
    }
}

pub(super) fn map_browser_key_to_action(key: KeyEvent) -> Option<WorkspaceAction> {
    match key.code {
        KeyCode::Char(KEY_BROWSER_QUIT) | KeyCode::Esc => Some(WorkspaceAction::Quit),
        KeyCode::Char(KEY_BROWSER_COMMAND_PALETTE) => Some(WorkspaceAction::OpenCommandPalette),
        KeyCode::Char(KEY_BROWSER_FILTER) => Some(WorkspaceAction::OpenDataFilter),
        KeyCode::Char(KEY_BROWSER_REFRESH) => Some(WorkspaceAction::Refresh),
        KeyCode::Char(KEY_BROWSER_CANCEL_TASKS) => Some(WorkspaceAction::CancelTasks),
        KeyCode::Char(KEY_BROWSER_OPEN_SQL) => Some(WorkspaceAction::OpenSqlEditor),
        KeyCode::Char(KEY_BROWSER_CREATE_TABLE) => Some(WorkspaceAction::OpenCreateTableForm),
        KeyCode::Char(KEY_BROWSER_TEMPLATE_SELECT) => Some(WorkspaceAction::OpenSelectTemplate),
        KeyCode::Char(KEY_BROWSER_TEMPLATE_INSERT) => Some(WorkspaceAction::OpenInsertTemplate),
        KeyCode::Char(KEY_BROWSER_TEMPLATE_UPDATE) => Some(WorkspaceAction::OpenUpdateTemplate),
        KeyCode::Char(KEY_BROWSER_TEMPLATE_DELETE) => Some(WorkspaceAction::OpenDeleteTemplate),
        KeyCode::Down | KeyCode::Char(KEY_BROWSER_DOWN) => Some(WorkspaceAction::NextItem),
        KeyCode::Up | KeyCode::Char(KEY_BROWSER_UP) => Some(WorkspaceAction::PreviousItem),
        KeyCode::Enter => Some(WorkspaceAction::OpenSelectedTreeItemDefault),
        KeyCode::Right
        | KeyCode::Left
        | KeyCode::Char(KEY_BROWSER_EXPAND_RIGHT)
        | KeyCode::Char(KEY_BROWSER_EXPAND_LEFT)
        | KeyCode::Char(KEY_BROWSER_TOGGLE_NODE) => Some(WorkspaceAction::ToggleNode),
        _ => None,
    }
}

pub(super) fn handle_shell_mouse(
    app: &mut AppShell,
    mouse: MouseEvent,
    area: Rect,
) -> Result<bool> {
    let AppShell::Workspace(app) = app else {
        return Ok(false);
    };
    handle_mouse(app, mouse, area)?;
    Ok(false)
}

pub(super) fn handle_mouse(app: &mut WorkspaceApp, mouse: MouseEvent, area: Rect) -> Result<()> {
    if app.help_overlay_open() {
        return Ok(());
    }

    if app.row_inspector_open() {
        return handle_row_inspector_mouse(app, mouse, area);
    }

    if !mouse_in_main_area(mouse, area) {
        return Ok(());
    }

    let focus_action = mouse_focus_action(app, mouse, area);
    match mouse.kind {
        MouseEventKind::ScrollDown => {
            if focus_action == Some(WorkspaceAction::FocusDataGrid) {
                app.apply_action(WorkspaceAction::FocusDataGrid)?;
                app.apply_action(WorkspaceAction::ScrollDataGridDown)?;
            } else if focus_action == Some(WorkspaceAction::FocusAssets) {
                app.apply_action(WorkspaceAction::FocusAssets)?;
                app.apply_action(WorkspaceAction::NextItem)?;
            } else if focus_action == Some(WorkspaceAction::FocusSqlEditor) {
                app.apply_action(WorkspaceAction::FocusSqlEditor)?;
            }
        }
        MouseEventKind::ScrollUp => {
            if focus_action == Some(WorkspaceAction::FocusDataGrid) {
                app.apply_action(WorkspaceAction::FocusDataGrid)?;
                app.apply_action(WorkspaceAction::ScrollDataGridUp)?;
            } else if focus_action == Some(WorkspaceAction::FocusAssets) {
                app.apply_action(WorkspaceAction::FocusAssets)?;
                app.apply_action(WorkspaceAction::PreviousItem)?;
            } else if focus_action == Some(WorkspaceAction::FocusSqlEditor) {
                app.apply_action(WorkspaceAction::FocusSqlEditor)?;
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(action) = right_tab_click_action(mouse, area) {
                app.apply_action(action)?;
                match action {
                    WorkspaceAction::SelectRightSqlTab => {
                        app.apply_action(WorkspaceAction::FocusSqlEditor)?;
                    }
                    WorkspaceAction::SelectRightDataTab
                    | WorkspaceAction::SelectRightStructureTab => {
                        app.apply_action(WorkspaceAction::FocusDataGrid)?;
                    }
                    _ => {}
                }
            } else if let Some(row_index) = asset_row_index_at(mouse, area, app.tree_rows().len()) {
                app.apply_action(WorkspaceAction::FocusAssets)?;
                let is_double_click = app.register_tree_row_click(row_index)?;
                if is_double_click {
                    if app.active_right_tab() == RightPaneTab::Sql
                        && app.selected_object().is_some()
                    {
                        app.apply_action(WorkspaceAction::OpenSqlEditor)?;
                    } else {
                        app.apply_action(WorkspaceAction::OpenSelectedTreeItemDefault)?;
                    }
                }
            } else if let Some(tab_index) = editor_tab_index_at(mouse, area, app) {
                app.select_editor_tab_index(tab_index)?;
            } else if let Some(result_index) = result_set_index_at(mouse, area, app) {
                app.select_result_set_index(result_index)?;
                app.apply_action(WorkspaceAction::FocusDataGrid)?;
            } else if let Some((row_index, column_index)) = grid_cell_at(mouse, area, app) {
                app.apply_action(WorkspaceAction::FocusDataGrid)?;
                let is_double_click = app.register_grid_cell_click(row_index, column_index);
                if is_double_click {
                    app.apply_action(WorkspaceAction::OpenRowInspector)?;
                }
            } else if let Some(action) = focus_action {
                app.apply_action(action)?;
            }
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            if let Some(tab_index) = editor_tab_index_at(mouse, area, app) {
                app.close_editor_tab_index(tab_index)?;
            }
        }
        _ => {}
    }

    Ok(())
}

pub(super) fn mouse_focus_action(
    app: &WorkspaceApp,
    mouse: MouseEvent,
    area: Rect,
) -> Option<WorkspaceAction> {
    let main = workspace_main_sections(workspace_body_area(area));
    if rect_contains(main[0], mouse.column, mouse.row) {
        return Some(WorkspaceAction::FocusAssets);
    }
    if !rect_contains(main[1], mouse.column, mouse.row) {
        return None;
    }

    match app.active_right_tab() {
        RightPaneTab::Sql if app.is_editor_open() => {
            let details = workspace_detail_sections(main[1]);
            if rect_contains(details[2], mouse.column, mouse.row) {
                let sql = sql_tab_sections(details[2]);
                if rect_contains(sql[0], mouse.column, mouse.row) {
                    Some(WorkspaceAction::FocusSqlEditor)
                } else if app.sql_results_available() {
                    Some(WorkspaceAction::FocusDataGrid)
                } else {
                    Some(WorkspaceAction::FocusSqlEditor)
                }
            } else {
                Some(WorkspaceAction::FocusSqlEditor)
            }
        }
        RightPaneTab::Data | RightPaneTab::Structure => Some(WorkspaceAction::FocusDataGrid),
        RightPaneTab::Sql => None,
    }
}

pub(super) fn right_tab_click_action(mouse: MouseEvent, area: Rect) -> Option<WorkspaceAction> {
    let main = workspace_main_sections(workspace_body_area(area));
    let details = workspace_detail_sections(main[1]);
    let tab_area = details[0];
    if mouse.row != tab_area.y + 1 {
        return None;
    }

    let mut x = tab_area.x + 1;
    for (title, action) in RIGHT_TAB_TITLES.into_iter().zip([
        WorkspaceAction::SelectRightDataTab,
        WorkspaceAction::SelectRightSqlTab,
        WorkspaceAction::SelectRightStructureTab,
    ]) {
        let width = title.len() as u16 + 2;
        if mouse.column >= x && mouse.column < x + width {
            return Some(action);
        }
        x += width + 1;
    }
    None
}

pub(super) fn asset_row_index_at(mouse: MouseEvent, area: Rect, row_count: usize) -> Option<usize> {
    let main = workspace_main_sections(workspace_body_area(area));
    let assets = main[0];
    if !rect_contains(assets, mouse.column, mouse.row) {
        return None;
    }

    let inner_top = assets.y + 1;
    let inner_bottom = assets.y + assets.height.saturating_sub(1);
    if mouse.row < inner_top || mouse.row >= inner_bottom {
        return None;
    }

    let row_index = usize::from(mouse.row - inner_top);
    (row_index < row_count).then_some(row_index)
}

pub(super) fn editor_tab_index_at(
    mouse: MouseEvent,
    area: Rect,
    app: &WorkspaceApp,
) -> Option<usize> {
    if app.active_right_tab() != RightPaneTab::Sql || !app.is_editor_open() {
        return None;
    }

    let main = workspace_main_sections(workspace_body_area(area));
    let details = workspace_detail_sections(main[1]);
    let sql = sql_tab_sections(details[2]);
    let editor_area = sql[0];
    let tab_row = editor_area.y + 1;
    if mouse.row != tab_row {
        return None;
    }

    let strip = app.editor_tab_strip()?;
    let prefix_width = "Tabs: ".chars().count() as u16;
    let x = editor_area.x + 1 + prefix_width;
    bracket_segment_index_at(strip, mouse.column, x)
}

pub(super) fn result_set_index_at(
    mouse: MouseEvent,
    area: Rect,
    app: &WorkspaceApp,
) -> Option<usize> {
    if app.active_right_tab() != RightPaneTab::Sql || !app.is_editor_open() {
        return None;
    }

    let editor = app.view().editor?;
    let strip = editor.result_strip?;
    let main = workspace_main_sections(workspace_body_area(area));
    let details = workspace_detail_sections(main[1]);
    let sql = sql_tab_sections(details[2]);
    let editor_area = sql[0];
    let row = editor_area.y + 1 + usize::from(!editor.tab_strip.is_empty()) as u16;
    if mouse.row != row {
        return None;
    }

    let prefix_width = "Results: ".chars().count() as u16;
    let x = editor_area.x + 1 + prefix_width;
    bracket_segment_index_at(strip, mouse.column, x)
}

pub(super) fn grid_cell_at(
    mouse: MouseEvent,
    area: Rect,
    app: &WorkspaceApp,
) -> Option<(usize, usize)> {
    let grid_area = active_grid_area(area, app)?;
    if !rect_contains(grid_area, mouse.column, mouse.row) {
        return None;
    }

    let grid = app.active_grid();
    if grid.columns.is_empty() || grid.rows.is_empty() {
        return None;
    }

    let data_top = grid_area.y + 2;
    let data_bottom = grid_area.y + grid_area.height.saturating_sub(1);
    if mouse.row < data_top || mouse.row >= data_bottom {
        return None;
    }

    let row_delta = usize::from(mouse.row - data_top);
    let row_index = app.grid_scroll_offset().saturating_add(row_delta);
    if row_index >= grid.rows.len() {
        return None;
    }

    let inner_left = grid_area.x + 1;
    let inner_right = grid_area.x + grid_area.width.saturating_sub(1);
    if mouse.column < inner_left || mouse.column >= inner_right {
        return None;
    }

    let viewport = GridViewport {
        selected_row_index: app.grid_selected_row_index(),
        selected_column_index: app.grid_selected_column_index(),
        row_offset: app.grid_scroll_offset(),
        column_offset: app.grid_column_offset(),
        focused: app.data_grid_focused(),
        width_overrides: app
            .current_grid_column_width_overrides()
            .map(|overrides| {
                overrides
                    .iter()
                    .map(|(index, width)| (*index, *width))
                    .collect()
            })
            .unwrap_or_default(),
        frozen_leading_columns: app.frozen_grid_column_count(),
    };
    let columns = grid_column_layouts(grid_area, grid, &viewport);
    let mut x = inner_left;
    for column in columns {
        let end = x.saturating_add(column.width);
        if mouse.column >= x && mouse.column < end {
            return Some((row_index, column.index));
        }
        x = end.saturating_add(GRID_COLUMN_SPACING);
    }
    None
}

pub(super) fn active_grid_area(area: Rect, app: &WorkspaceApp) -> Option<Rect> {
    let main = workspace_main_sections(workspace_body_area(area));
    let details = workspace_detail_sections(main[1]);
    match app.active_right_tab() {
        RightPaneTab::Data | RightPaneTab::Structure => Some(details[2]),
        RightPaneTab::Sql if app.sql_results_available() => Some(sql_tab_sections(details[2])[1]),
        RightPaneTab::Sql => None,
    }
}

pub(super) fn bracket_segment_index_at(strip: &str, column: u16, start_x: u16) -> Option<usize> {
    let mut x = start_x;
    let mut chars = strip.chars().peekable();
    let mut index = 0;
    while let Some(ch) = chars.next() {
        if ch != '[' {
            x = x.saturating_add(1);
            continue;
        }

        let segment_start = x;
        x = x.saturating_add(1);
        for next in chars.by_ref() {
            x = x.saturating_add(1);
            if next == ']' {
                break;
            }
        }
        if column >= segment_start && column < x {
            return Some(index);
        }
        index += 1;
    }
    None
}

pub(super) fn mouse_in_main_area(mouse: MouseEvent, area: Rect) -> bool {
    rect_contains(workspace_body_area(area), mouse.column, mouse.row)
}
