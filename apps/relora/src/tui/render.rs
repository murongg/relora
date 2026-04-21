use super::*;
use relora_app::view::{SaveSqlDialogView, SavedSqlView};
use relora_core::db::{
    DatabaseKind, DbObjectRef, DriverCapabilities, ExplainFlavor, IdentifierQuoteStyle,
};

pub(super) fn draw(frame: &mut Frame<'_>, app: &AppShell) {
    match app {
        AppShell::Launcher(launcher) => draw_launcher(frame, launcher),
        AppShell::Workspace(workspace) => draw_workspace(frame, workspace),
    }
}

pub(super) fn draw_workspace(frame: &mut Frame<'_>, app: &WorkspaceApp) {
    let view = app.view();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(WORKSPACE_HEADER_HEIGHT),
            Constraint::Min(WORKSPACE_MIN_BODY_HEIGHT),
            Constraint::Length(WORKSPACE_FOOTER_HEIGHT),
        ])
        .split(frame.area());

    draw_header(frame, vertical[0], app, view);
    draw_main(frame, vertical[1], app, view);
    draw_footer(frame, vertical[2], view);

    if let Some(row_inspector) = view.row_inspector {
        draw_row_inspector(frame, frame.area(), row_inspector);
    }

    if view.help_overlay_visible {
        draw_help_overlay(frame, frame.area(), view);
    }

    if let Some(data_filter) = view.data_filter {
        draw_data_filter(frame, frame.area(), data_filter);
    }

    if let Some(cell_edit) = view.cell_edit {
        draw_cell_edit(frame, frame.area(), cell_edit);
    }

    if let Some(save_sql_dialog) = view.save_sql_dialog {
        draw_save_sql_dialog(frame, frame.area(), save_sql_dialog);
    }

    if let Some(sql_history) = view.sql_history {
        draw_sql_history(frame, frame.area(), sql_history);
    }

    if let Some(saved_sql) = view.saved_sql {
        draw_saved_sql(frame, frame.area(), saved_sql);
    }

    if let Some(command_palette) = view.command_palette {
        draw_command_palette(frame, frame.area(), command_palette);
    }

    if let Some(delete_confirmation) = view.delete_confirmation {
        draw_workspace_delete_confirmation(frame, frame.area(), delete_confirmation);
    }
}

pub(super) fn draw_launcher(frame: &mut Frame<'_>, app: &LauncherApp) {
    let view = app.view();
    frame.render_widget(
        Block::default().style(Style::default().bg(BACKGROUND_DEEP).fg(TEXT_DEFAULT)),
        frame.area(),
    );

    let card = centered_rect(
        LAUNCHER_CARD_WIDTH_PERCENT,
        LAUNCHER_CARD_HEIGHT_PERCENT,
        frame.area(),
    );
    frame.render_widget(Clear, card);
    frame.render_widget(
        Block::default().style(Style::default().bg(SURFACE_CARD).fg(TEXT_STRONG)),
        card,
    );
    let accent = Rect::new(card.x, card.y, card.width, LAUNCHER_ACCENT_HEIGHT);
    frame.render_widget(
        Paragraph::new(" ".repeat(card.width as usize)).style(
            Style::default()
                .bg(theme_accent_color())
                .fg(theme_accent_color()),
        ),
        accent,
    );

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(LAUNCHER_LOGO_HEIGHT),
            Constraint::Length(LAUNCHER_BRAND_COPY_HEIGHT),
            Constraint::Length(LAUNCHER_SECTION_HEADER_HEIGHT),
            Constraint::Min(LAUNCHER_LIST_MIN_HEIGHT),
            Constraint::Length(LAUNCHER_FOOTER_HEIGHT),
        ])
        .margin(LAUNCHER_CARD_MARGIN)
        .split(card);

    let launch_count = if view.marked_indexes.is_empty() {
        usize::from(!view.connections.is_empty())
    } else {
        view.marked_indexes.len()
    };

    frame.render_widget(
        Paragraph::new(
            LAUNCHER_PIXEL_WORDMARK
                .iter()
                .enumerate()
                .map(|(index, line)| {
                    Line::from(Span::styled(
                        *line,
                        Style::default()
                            .fg(if index == 0 {
                                TEXT_LOGO
                            } else {
                                theme_accent_color()
                            })
                            .add_modifier(Modifier::BOLD),
                    ))
                })
                .collect::<Vec<_>>(),
        )
        .alignment(Alignment::Center),
        vertical[0],
    );

    let brand_copy = Paragraph::new(vec![
        Line::from(Span::styled(
            PRODUCT_TAGLINE,
            Style::default()
                .fg(theme_accent_color())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            PRODUCT_DESCRIPTION,
            Style::default().fg(TEXT_SECONDARY),
        )),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(brand_copy, vertical[1]);

    let section_header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                TITLE_SAVED_CONNECTIONS,
                Style::default()
                    .fg(TEXT_STRONG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!(" Profiles {} ", view.connections.len()),
                Style::default()
                    .bg(BADGE_BACKGROUND)
                    .fg(theme_accent_color()),
            ),
            Span::raw("  "),
            Span::styled(
                format!(" Ready {} ", launch_count),
                Style::default()
                    .bg(BADGE_READY_BACKGROUND)
                    .fg(theme_accent_color()),
            ),
        ]),
        Line::from(Span::styled(
            PRODUCT_ENGINE_NOTE,
            Style::default().fg(TEXT_MUTED),
        )),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(section_header, vertical[2]);

    if view.connections.is_empty() {
        let body = Paragraph::new(vec![
            Line::from(Span::styled(
                LAUNCHER_EMPTY_TITLE,
                Style::default()
                    .fg(TEXT_STRONG)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                LAUNCHER_EMPTY_PRIMARY,
                Style::default().fg(TEXT_SECONDARY),
            )),
            Line::from(Span::styled(
                LAUNCHER_EMPTY_SECONDARY,
                Style::default().fg(TEXT_MUTED),
            )),
        ])
        .wrap(Wrap { trim: true });
        frame.render_widget(body, vertical[3]);
    } else {
        let items = view
            .connections
            .iter()
            .enumerate()
            .map(|(index, connection)| {
                let status = if view.marked_indexes.contains(&index) {
                    LAUNCHER_STATUS_QUEUED
                } else if index == view.selected_index {
                    LAUNCHER_STATUS_FOCUSED
                } else {
                    LAUNCHER_STATUS_READY
                };
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            launcher_database_badge(connection.url.as_str()),
                            Style::default()
                                .bg(BADGE_DATABASE_BACKGROUND)
                                .fg(theme_accent_color())
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(
                            connection.name.as_str(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        if connection.read_only {
                            Span::styled("  read-only", Style::default().fg(theme_accent_color()))
                        } else {
                            Span::raw("")
                        },
                        Span::raw("  "),
                        Span::styled(status, Style::default().fg(theme_accent_color())),
                    ]),
                    Line::from(Span::styled(
                        connection.url.as_str(),
                        Style::default().fg(TEXT_MUTED),
                    )),
                ])
            })
            .collect::<Vec<_>>();
        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(SELECTION_BACKGROUND)
                    .fg(TEXT_SELECTED)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("› ");
        let mut state = ListState::default();
        state.select(Some(
            view.selected_index
                .min(view.connections.len().saturating_sub(1)),
        ));
        frame.render_stateful_widget(list, vertical[3], &mut state);
    };

    let status = view.status.unwrap_or(LAUNCHER_DEFAULT_STATUS);
    let help = if view.form.is_some() {
        LAUNCHER_HELP_FORM
    } else {
        LAUNCHER_HELP_IDLE
    };
    let footer = Paragraph::new(vec![
        Line::from(Span::styled(
            help,
            Style::default()
                .fg(TEXT_STRONG)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(status, Style::default().fg(TEXT_SECONDARY))),
        Line::from(Span::styled(
            if view.form.is_some() {
                LAUNCHER_FOOTER_FORM
            } else {
                LAUNCHER_FOOTER_IDLE
            },
            Style::default().fg(TEXT_MUTED),
        )),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(footer, vertical[4]);

    if let Some(form) = view.form {
        draw_launcher_form(frame, frame.area(), form);
    }

    if let Some(file_picker) = view.sqlite_file_picker {
        draw_launcher_sqlite_file_picker(frame, frame.area(), file_picker);
    }

    if let Some(delete_confirmation) = view.delete_confirmation {
        draw_launcher_delete_confirmation(frame, frame.area(), delete_confirmation);
    }

    if let Some(missing_driver) = view.missing_driver {
        draw_launcher_missing_driver(frame, frame.area(), missing_driver);
    }
}

fn launcher_database_badge(url: &str) -> &'static str {
    match DatabaseKind::from_url(url) {
        Ok(DatabaseKind::Postgres) => DATABASE_BADGE_POSTGRES,
        Ok(DatabaseKind::MySql) => DATABASE_BADGE_MYSQL,
        Ok(DatabaseKind::Sqlite) => DATABASE_BADGE_SQLITE,
        Err(_) => DATABASE_BADGE_GENERIC,
    }
}

pub(super) fn draw_launcher_missing_driver(
    frame: &mut Frame<'_>,
    area: Rect,
    missing: crate::launcher::LauncherMissingDriverView<'_>,
) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().style(Style::default().bg(BACKGROUND_DEEP).fg(TEXT_DEFAULT)),
        area,
    );
    let popup = centered_rect(
        CONNECTION_FORM_WIDTH_PERCENT,
        DRIVER_MISSING_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_accent_color()))
        .style(Style::default().bg(SURFACE_CARD).fg(TEXT_STRONG))
        .title(TITLE_DRIVER_MISSING);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let body_area = Layout::default()
        .direction(Direction::Vertical)
        .margin(DELETE_CONFIRM_MARGIN)
        .constraints([Constraint::Min(1)])
        .split(inner)[0];
    let body = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Missing ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(
                missing.display_name,
                Style::default()
                    .fg(theme_accent_color())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" driver", Style::default().fg(TEXT_SECONDARY)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            missing.binary,
            Style::default()
                .fg(TEXT_STRONG)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            missing.env_var,
            Style::default()
                .fg(TEXT_STRONG)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            DRIVER_MISSING_WARNING,
            Style::default().fg(TEXT_MUTED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            DRIVER_MISSING_HELP,
            Style::default()
                .fg(theme_accent_color())
                .add_modifier(Modifier::BOLD),
        )),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(body, body_area);
}

pub(super) fn draw_launcher_delete_confirmation(
    frame: &mut Frame<'_>,
    area: Rect,
    confirmation: crate::launcher::LauncherDeleteConfirmationView<'_>,
) {
    let popup = centered_rect(
        DELETE_CONFIRM_WIDTH_PERCENT,
        DELETE_CONFIRM_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_accent_color()))
        .style(Style::default().bg(SURFACE_CARD).fg(TEXT_STRONG))
        .title(TITLE_DELETE_CONNECTION);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let body_area = Layout::default()
        .direction(Direction::Vertical)
        .margin(DELETE_CONFIRM_MARGIN)
        .constraints([Constraint::Min(1)])
        .split(inner)[0];
    let body = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Delete ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(
                confirmation.name,
                Style::default()
                    .fg(theme_accent_color())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("?", Style::default().fg(TEXT_SECONDARY)),
        ]),
        Line::from(Span::styled(
            confirmation.url,
            Style::default().fg(TEXT_MUTED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            DELETE_CONNECTION_WARNING_PROFILE,
            Style::default().fg(TEXT_SECONDARY),
        )),
        Line::from(Span::styled(
            DELETE_CONNECTION_WARNING_DATABASE,
            Style::default().fg(TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(Span::styled(
            DELETE_CONNECTION_HELP,
            Style::default()
                .fg(TEXT_STRONG)
                .add_modifier(Modifier::BOLD),
        )),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(body, body_area);
}

pub(super) fn draw_workspace_delete_confirmation(
    frame: &mut Frame<'_>,
    area: Rect,
    confirmation: DeleteConfirmationView<'_>,
) {
    let popup = centered_rect(
        DELETE_CONFIRM_WIDTH_PERCENT,
        DELETE_CONFIRM_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_accent_color()))
        .style(Style::default().bg(SURFACE_CARD).fg(TEXT_STRONG))
        .title(confirmation.title);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let body_area = Layout::default()
        .direction(Direction::Vertical)
        .margin(DELETE_CONFIRM_MARGIN)
        .constraints([Constraint::Min(1)])
        .split(inner)[0];
    let body = Paragraph::new(vec![
        Line::from(Span::styled(
            confirmation.message,
            Style::default().fg(TEXT_SECONDARY),
        )),
        Line::from(""),
        Line::from(Span::styled(
            confirmation.sql_preview,
            Style::default()
                .fg(TEXT_STRONG)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            confirmation.warning,
            Style::default().fg(TEXT_MUTED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            confirmation.help,
            Style::default()
                .fg(theme_accent_color())
                .add_modifier(Modifier::BOLD),
        )),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(body, body_area);
}

pub(super) fn draw_launcher_form(
    frame: &mut Frame<'_>,
    area: Rect,
    form: crate::launcher::LauncherFormView<'_>,
) {
    let popup = centered_rect(
        CONNECTION_FORM_WIDTH_PERCENT,
        CONNECTION_FORM_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_accent_color()))
        .style(Style::default().bg(SURFACE_CARD).fg(TEXT_STRONG))
        .title(if form.editing_existing {
            TITLE_EDIT_CONNECTION
        } else {
            TITLE_NEW_CONNECTION
        });
    frame.render_widget(block.clone(), popup);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .margin(CONNECTION_FORM_MARGIN)
        .constraints(connection_form_constraints(form.driver))
        .split(popup);
    let password = if form.password.is_empty() {
        String::new()
    } else {
        "*".repeat(form.password.chars().count())
    };
    let rows = launcher_form_rows(&form, &password);
    for (index, (field, label, value)) in rows.iter().enumerate() {
        draw_launcher_form_row(frame, sections[index], form.field == *field, label, value);
    }
    let help = if let Some(status) = form.status {
        vec![
            Line::from(Span::styled(status, Style::default().fg(TEXT_SECONDARY))),
            Line::from(Span::styled(
                form_help_label(form.driver),
                Style::default().fg(TEXT_MUTED),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            form_help_label(form.driver),
            Style::default().fg(TEXT_MUTED),
        ))]
    };
    frame.render_widget(
        Paragraph::new(help).wrap(Wrap { trim: true }),
        sections[rows.len()],
    );
}

fn connection_form_constraints(driver: crate::launcher::LauncherDatabaseKind) -> Vec<Constraint> {
    let field_count = match driver {
        crate::launcher::LauncherDatabaseKind::Sqlite => 5,
        crate::launcher::LauncherDatabaseKind::Postgres
        | crate::launcher::LauncherDatabaseKind::MySql => CONNECTION_FORM_FIELD_COUNT,
    };
    let mut constraints = vec![Constraint::Length(CONNECTION_FORM_FIELD_HEIGHT); field_count];
    constraints.push(Constraint::Min(CONNECTION_FORM_HELP_MIN_HEIGHT));
    constraints
}

fn launcher_form_rows(
    form: &crate::launcher::LauncherFormView<'_>,
    password: &str,
) -> Vec<(LauncherFormField, &'static str, String)> {
    let mut rows = vec![
        (
            LauncherFormField::Name,
            FORM_NAME_LABEL,
            form.name.to_string(),
        ),
        (
            LauncherFormField::Driver,
            FORM_DRIVER_LABEL,
            form.driver.label().to_string(),
        ),
        (
            LauncherFormField::Access,
            FORM_ACCESS_LABEL,
            if form.read_only {
                "Read-only".to_string()
            } else {
                "Read-write".to_string()
            },
        ),
    ];

    match form.driver {
        crate::launcher::LauncherDatabaseKind::Sqlite => {
            rows.push((
                LauncherFormField::Database,
                FORM_SQLITE_FILE_LABEL,
                form.database.to_string(),
            ));
        }
        crate::launcher::LauncherDatabaseKind::Postgres
        | crate::launcher::LauncherDatabaseKind::MySql => {
            rows.extend([
                (
                    LauncherFormField::Host,
                    FORM_HOST_LABEL,
                    form.host.to_string(),
                ),
                (
                    LauncherFormField::Port,
                    FORM_PORT_LABEL,
                    form.port.to_string(),
                ),
                (
                    LauncherFormField::Database,
                    FORM_DATABASE_LABEL,
                    form.database.to_string(),
                ),
                (
                    LauncherFormField::Username,
                    FORM_USERNAME_LABEL,
                    form.username.to_string(),
                ),
                (
                    LauncherFormField::Password,
                    FORM_PASSWORD_LABEL,
                    password.to_string(),
                ),
            ]);
        }
    }

    rows.push((LauncherFormField::Url, FORM_URL_LABEL, form.url.to_string()));
    rows
}

fn form_help_label(driver: crate::launcher::LauncherDatabaseKind) -> &'static str {
    match driver {
        crate::launcher::LauncherDatabaseKind::Sqlite => FORM_SAVE_HELP_SQLITE,
        crate::launcher::LauncherDatabaseKind::Postgres
        | crate::launcher::LauncherDatabaseKind::MySql => FORM_SAVE_HELP,
    }
}

fn draw_launcher_form_row(
    frame: &mut Frame<'_>,
    area: Rect,
    active: bool,
    label: &str,
    value: &str,
) {
    let prefix = if active { "> " } else { "  " };
    let style = if active {
        Style::default()
            .fg(TEXT_STRONG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(TEXT_SECONDARY)
    };
    frame.render_widget(
        Paragraph::new(format!("{prefix}{label}: {value}")).style(style),
        area,
    );
}

pub(super) fn draw_launcher_sqlite_file_picker(
    frame: &mut Frame<'_>,
    area: Rect,
    picker: crate::launcher::LauncherSqliteFilePickerView,
) {
    let popup = centered_rect(
        SQLITE_FILE_PICKER_WIDTH_PERCENT,
        SQLITE_FILE_PICKER_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme_accent_color()))
        .style(Style::default().bg(SURFACE_CARD).fg(TEXT_STRONG))
        .title(TITLE_SQLITE_FILE_PICKER);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .margin(CONNECTION_FORM_MARGIN)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                picker.current_dir,
                Style::default()
                    .fg(theme_accent_color())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Choose an existing SQLite file. Directories open in place.",
                Style::default().fg(TEXT_MUTED),
            )),
        ]),
        sections[0],
    );

    let items = if picker.entries.is_empty() {
        vec![ListItem::new(Span::styled(
            "No files found in this folder.",
            Style::default().fg(TEXT_MUTED),
        ))]
    } else {
        picker
            .entries
            .iter()
            .map(|entry| {
                let prefix = if entry.is_directory { "› " } else { "  " };
                ListItem::new(format!("{prefix}{}", entry.label))
            })
            .collect()
    };
    let mut state = ListState::default();
    if !picker.entries.is_empty() {
        state.select(Some(picker.selected_index));
    }
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(
                Style::default()
                    .bg(SELECTION_BACKGROUND)
                    .fg(TEXT_SELECTED)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> "),
        sections[1],
        &mut state,
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            SQLITE_FILE_PICKER_HELP,
            Style::default().fg(TEXT_MUTED),
        )),
        sections[2],
    );
}

pub(super) fn draw_header(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &WorkspaceApp,
    view: WorkspaceView<'_>,
) {
    let connection_label = match (
        view.selected_connection_name,
        view.selected_connection_kind.map(database_kind_label),
    ) {
        (Some(name), Some(kind)) => format!("{name} ({kind})"),
        (Some(name), None) => name.to_string(),
        _ => format!("{} 0", TITLE_SAVED_CONNECTIONS),
    };

    let scope_label = if let Some(object) = view.selected_object {
        object_scope_label(view.selected_connection_kind, object)
    } else if let (Some(database), Some(schema)) =
        (view.selected_database_name, view.selected_schema_name)
    {
        format!("{database}/{schema}")
    } else if let Some(database) = view.selected_database_name {
        format!("database/{database}")
    } else {
        format!("connections {}", view.connection_count)
    };

    let activity_label = match view.active_right_tab {
        RightPaneTab::Data => app
            .preview_page_summary()
            .unwrap_or_else(|| format!("rows {}", view.preview_grid.rows.len())),
        RightPaneTab::Sql => view
            .editor
            .map(|editor| {
                format!(
                    "result {}/{}",
                    editor.selected_result_index + 1,
                    editor.result_set_count.max(1)
                )
            })
            .unwrap_or_else(|| "editor idle".to_string()),
        RightPaneTab::Structure => format!(
            "columns {}",
            view.structure
                .map(|structure| structure.columns.len())
                .unwrap_or_default()
        ),
    };

    let status_label = if view.preview_loading {
        " Loading ".to_string()
    } else if view.selected_connection_busy {
        format!(" Busy {} ", view.pending_task_count.max(1))
    } else if view.pending_task_count > 0 {
        format!(" Tasks {} ", view.pending_task_count)
    } else {
        " Ready ".to_string()
    };
    let status_style =
        if view.preview_loading || view.selected_connection_busy || view.pending_task_count > 0 {
            Style::default()
                .bg(BADGE_BACKGROUND)
                .fg(theme_accent_color())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .bg(BADGE_READY_BACKGROUND)
                .fg(theme_accent_color())
                .add_modifier(Modifier::BOLD)
        };

    let active_tab = match view.active_right_tab {
        RightPaneTab::Data => TAB_DATA,
        RightPaneTab::Sql => TAB_SQL,
        RightPaneTab::Structure => TAB_STRUCTURE,
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", APP_NAME),
            Style::default()
                .fg(TEXT_LOGO)
                .bg(BADGE_BACKGROUND)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(connection_label, Style::default().fg(TEXT_STRONG)),
        Span::styled(" | ", Style::default().fg(TEXT_MUTED)),
        Span::styled(scope_label, Style::default().fg(TEXT_SECONDARY)),
        Span::styled(" | ", Style::default().fg(TEXT_MUTED)),
        Span::styled(format!(" {} ", active_tab), active_tab_style()),
        Span::raw(" "),
        Span::styled(activity_label, Style::default().fg(TEXT_SECONDARY)),
        Span::styled(" | ", Style::default().fg(TEXT_MUTED)),
        Span::styled(status_label, status_style),
    ]);

    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(SURFACE_CARD).fg(TEXT_DEFAULT)),
        area,
    );
}

pub(super) fn draw_main(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &WorkspaceApp,
    view: WorkspaceView<'_>,
) {
    let sections = workspace_main_sections(area);

    draw_assets(frame, sections[0], view);
    draw_details(frame, sections[1], app, view);
}

pub(super) fn draw_assets(frame: &mut Frame<'_>, area: Rect, view: WorkspaceView<'_>) {
    let items = view.tree_rows.iter().map(tree_item).collect::<Vec<_>>();
    let title = if view.assets_focused {
        format!("* {TITLE_ASSETS}")
    } else {
        TITLE_ASSETS.to_string()
    };

    let list = List::new(items)
        .block(focusable_block(title, view.assets_focused))
        .highlight_style(highlight_style())
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(Some(view.selected_row_index));
    frame.render_stateful_widget(list, area, &mut state);
}

pub(super) fn draw_details(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &WorkspaceApp,
    view: WorkspaceView<'_>,
) {
    let sections = workspace_detail_sections(area);

    draw_right_tabs(frame, sections[0], view);
    draw_summary(frame, sections[1], view);

    match view.active_right_tab {
        RightPaneTab::Data => draw_data_tab(frame, sections[2], app, view),
        RightPaneTab::Sql => draw_sql_tab(frame, sections[2], app, view),
        RightPaneTab::Structure => draw_structure_tab(frame, sections[2], app, view),
    }
}

pub(super) fn draw_right_tabs(frame: &mut Frame<'_>, area: Rect, view: WorkspaceView<'_>) {
    let mut spans = Vec::new();
    for tab in view.right_tabs {
        let style = if tab.active {
            active_tab_style()
        } else if tab.available {
            Style::default().fg(TEXT_AVAILABLE)
        } else {
            Style::default().fg(TEXT_DIM)
        };
        spans.push(Span::styled(format!(" {} ", tab.title), style));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(
        RIGHT_TAB_SHORTCUT_HELP,
        Style::default().fg(TEXT_DIM),
    ));

    let tabs = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title(TITLE_TABS))
        .wrap(Wrap { trim: true });
    frame.render_widget(tabs, area);
}

pub(super) fn draw_data_tab(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &WorkspaceApp,
    view: WorkspaceView<'_>,
) {
    if view.selected_object.is_some() {
        draw_preview(frame, area, app, view);
    } else {
        draw_placeholder(frame, area, view);
    }
}

pub(super) fn draw_sql_tab(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &WorkspaceApp,
    view: WorkspaceView<'_>,
) {
    let Some(editor) = view.editor else {
        let paragraph = Paragraph::new(OPEN_SQL_EDITOR_MESSAGE)
            .block(Block::default().borders(Borders::ALL).title(TITLE_SQL))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
        return;
    };

    let sections = sql_tab_sections(area);
    draw_editor(frame, sections[0], editor, view.sql_editor_focused);
    if let Some(completion) = view.editor_completion {
        draw_editor_completion(frame, sections[0], editor, completion);
    }
    draw_result_or_preview(frame, sections[1], app, view);
}

pub(super) fn draw_structure_tab(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &WorkspaceApp,
    view: WorkspaceView<'_>,
) {
    let Some(structure) = view.structure else {
        draw_structure_message(frame, area, TITLE_STRUCTURE, SELECT_TABLE_OBJECT_MESSAGE);
        return;
    };

    let title = structure
        .object
        .map(|object| format!("Structure {}", object.qualified_name()))
        .unwrap_or_else(|| TITLE_STRUCTURE.to_string());

    if structure.object.is_none() {
        draw_structure_message(frame, area, &title, SELECT_TABLE_OBJECT_MESSAGE);
        return;
    }

    if structure.loading && structure.columns.is_empty() {
        let message = structure.status.unwrap_or(LOADING_STRUCTURE_MESSAGE);
        draw_structure_message(frame, area, &title, message);
        return;
    }

    if structure.columns.is_empty() {
        let message = structure.status.unwrap_or(NO_COLUMNS_MESSAGE);
        draw_structure_message(frame, area, &title, message);
        return;
    }

    draw_grid(
        frame,
        area,
        view.active_grid,
        &title,
        GridViewport::from_app_view(app, view),
    );
}

pub(super) fn draw_structure_message(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    message: &str,
) {
    let paragraph = Paragraph::new(message)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title.to_string()),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

pub(super) fn draw_summary(frame: &mut Frame<'_>, area: Rect, view: WorkspaceView<'_>) {
    let connection_name = view.selected_connection_name.unwrap_or(NOT_AVAILABLE_LABEL);
    let connection_label = view
        .selected_connection_label
        .unwrap_or(NOT_AVAILABLE_LABEL);
    let backend = view
        .selected_connection_kind
        .map(database_kind_label)
        .unwrap_or(UNKNOWN_LABEL);

    let mut lines = vec![
        Line::from(format!("Connection: {connection_name} ({backend})")),
        Line::from(format!("Target: {connection_label}")),
    ];

    if let Some(database_name) = view.selected_database_name {
        lines.push(Line::from(format!("Database: {database_name}")));
    }

    if view.selected_connection_read_only {
        lines.push(Line::from("Mode: read-only"));
    }

    lines.push(Line::from(format!(
        "Databases: {} | Schemas: {} | Objects: {}",
        view.selected_connection_database_count,
        view.selected_connection_schema_count,
        view.selected_connection_object_count
    )));

    if view.selected_connection_busy {
        lines.push(Line::from("Worker: busy"));
    }

    if let Some(capabilities) = view.selected_connection_capabilities {
        lines.push(Line::from(format!(
            "Dialect: {} | {} | {}",
            identifier_quote_style_label(capabilities),
            explain_capability_label(capabilities),
            returning_capability_label(capabilities)
        )));
        lines.push(Line::from(format!(
            "Features: {} | {} | {}",
            support_label("completion", capabilities.supports_sql_completion),
            support_label("templates", capabilities.supports_crud_templates),
            support_label("staged CRUD", capabilities.supports_staged_crud)
        )));
    }

    if let Some(object) = view.selected_object {
        lines.push(Line::from(format!(
            "Object: {} ({})",
            object_scope_label(view.selected_connection_kind, object),
            object.kind.label()
        )));
    } else if let Some(schema_name) = view.selected_schema_name {
        lines.push(Line::from(format!("Scope: schema {schema_name}")));
    } else if let Some(database_name) = view.selected_database_name {
        lines.push(Line::from(format!("Scope: database {database_name}")));
    } else {
        lines.push(Line::from("Scope: connection"));
    }

    let summary = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(TITLE_SELECTION),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(summary, area);
}

pub(super) fn draw_placeholder(frame: &mut Frame<'_>, area: Rect, view: WorkspaceView<'_>) {
    let mut lines = vec![
        format!("Databases: {}", view.selected_connection_database_count),
        format!("Schemas: {}", view.selected_connection_schema_count),
        format!("Objects: {}", view.selected_connection_object_count),
    ];

    if let Some(database_name) = view.selected_database_name {
        lines.push(format!("Selected database: {database_name}"));
    }

    if let Some(schema_name) = view.selected_schema_name {
        lines.push(format!("Selected schema: {schema_name}"));
        lines.push(format!("Tables: {}", view.selected_schema_table_count));
        lines.push(format!("Views: {}", view.selected_schema_view_count));
        lines.push(format!(
            "Foreign tables: {}",
            view.selected_schema_foreign_table_count
        ));
    }

    if let Some(kind) = view.selected_group_kind {
        lines.push(format!("Selected group: {}", kind.group_label()));
    }

    lines.push(SELECT_TABLE_OBJECT_MESSAGE.to_string());
    let paragraph = Paragraph::new(lines.join("\n"))
        .block(Block::default().borders(Borders::ALL).title(TITLE_OVERVIEW))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

pub(super) fn draw_editor(
    frame: &mut Frame<'_>,
    area: Rect,
    editor: EditorView<'_>,
    focused: bool,
) {
    let mut lines = Vec::new();
    if !editor.tab_strip.is_empty() {
        lines.push(Line::from(format!("Tabs: {}", editor.tab_strip)));
    }
    if let Some(result_strip) = editor.result_strip {
        lines.push(Line::from(format!("Results: {}", result_strip)));
    }
    let header_line_count = lines.len();

    lines.extend(editor.lines.iter().map(|line| highlighted_sql_line(line)));
    if lines.is_empty() {
        lines.push(Line::from(String::new()));
    }

    let title = format!(
        "{} | Tab {}/{} | Result {}/{}",
        editor.title,
        editor.selected_tab_index + 1,
        editor.tab_count.max(1),
        editor.selected_result_index + 1,
        editor.result_set_count.max(1),
    );
    let title = if focused { format!("* {title}") } else { title };
    let block = focusable_block(title, focused);
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);

    if focused {
        let max_x = area.width.saturating_sub(2) as usize;
        let content_top = area.y + 1 + header_line_count as u16;
        let max_y = area.height.saturating_sub(2 + header_line_count as u16) as usize;
        let x = area.x + 1 + editor.cursor_col.min(max_x) as u16;
        let y = content_top + editor.cursor_row.min(max_y) as u16;
        frame.set_cursor_position(Position::new(x, y));
    }
}

pub(super) fn draw_editor_completion(
    frame: &mut Frame<'_>,
    area: Rect,
    editor: EditorView<'_>,
    completion: relora_app::view::EditorCompletionView<'_>,
) {
    let popup = editor_completion_popup_rect(area, editor, completion.items.len());
    frame.render_widget(Clear, popup);

    let items = completion
        .items
        .iter()
        .map(|item| {
            ListItem::new(Line::from(vec![
                Span::styled(item.label.as_str(), completion_kind_style(item.kind)),
                Span::styled(
                    format!("  {}", completion_kind_label(item.kind)),
                    Style::default().fg(TEXT_DIM),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(TITLE_COMPLETION),
        )
        .highlight_style(highlight_style())
        .highlight_symbol(">> ");
    let mut state = ListState::default();
    state.select(Some(completion.selected_index));
    frame.render_stateful_widget(list, popup, &mut state);
}

pub(super) fn editor_completion_popup_rect(
    area: Rect,
    editor: EditorView<'_>,
    item_count: usize,
) -> Rect {
    let header_line_count =
        usize::from(!editor.tab_strip.is_empty()) + usize::from(editor.result_strip.is_some());
    let content_top = area.y + 1 + header_line_count as u16;
    let content_left = area.x + 1;
    let max_content_width = area.width.saturating_sub(2);
    let popup_width = if max_content_width < COMPLETION_MIN_WIDTH {
        max_content_width.max(1)
    } else {
        max_content_width.min(COMPLETION_MAX_WIDTH)
    };
    let desired_height = area
        .height
        .saturating_sub(POPUP_CHROME_HEIGHT)
        .min((item_count.max(1) as u16).saturating_add(POPUP_CHROME_HEIGHT));
    let popup_height = if desired_height < COMPLETION_MIN_HEIGHT {
        desired_height.max(1)
    } else {
        desired_height.min(COMPLETION_MAX_HEIGHT)
    };

    let cursor_x = content_left
        + editor
            .cursor_col
            .min(max_content_width.saturating_sub(1) as usize) as u16;
    let cursor_y = content_top
        + editor
            .cursor_row
            .min(area.height.saturating_sub(2 + header_line_count as u16) as usize)
            as u16;

    let max_x = area.x + area.width.saturating_sub(popup_width + 1);
    let popup_x = cursor_x.clamp(content_left, max_x.max(content_left));

    let preferred_y = cursor_y.saturating_add(1);
    let max_y = area.y + area.height.saturating_sub(popup_height + 1);
    let popup_y = if preferred_y <= max_y {
        preferred_y
    } else {
        cursor_y
            .saturating_sub(popup_height)
            .max(content_top)
            .min(max_y.max(content_top))
    };

    Rect::new(popup_x, popup_y, popup_width, popup_height)
}

pub(super) fn highlighted_sql_line(line: &str) -> Line<'static> {
    Line::from(
        highlight_sql_line(line)
            .into_iter()
            .map(|token| Span::styled(token.text, sql_token_style(token.kind)))
            .collect::<Vec<_>>(),
    )
}

pub(super) fn sql_token_style(kind: SqlTokenKind) -> Style {
    match kind {
        SqlTokenKind::Keyword => Style::default()
            .fg(SQL_KEYWORD)
            .add_modifier(Modifier::BOLD),
        SqlTokenKind::String => Style::default().fg(SQL_STRING),
        SqlTokenKind::Number => Style::default().fg(SQL_NUMBER),
        SqlTokenKind::Comment => Style::default().fg(TEXT_DIM).add_modifier(Modifier::ITALIC),
        SqlTokenKind::Symbol => Style::default().fg(SQL_SYMBOL),
        SqlTokenKind::Identifier | SqlTokenKind::Whitespace => Style::default(),
    }
}

pub(super) fn completion_kind_style(kind: CompletionKind) -> Style {
    match kind {
        CompletionKind::Keyword => Style::default()
            .fg(COMPLETION_KEYWORD)
            .add_modifier(Modifier::BOLD),
        CompletionKind::Object => Style::default().fg(COMPLETION_OBJECT),
        CompletionKind::Column => Style::default().fg(COMPLETION_COLUMN),
    }
}

pub(super) fn completion_kind_label(kind: CompletionKind) -> &'static str {
    match kind {
        CompletionKind::Keyword => "keyword",
        CompletionKind::Object => "object",
        CompletionKind::Column => "column",
    }
}

pub(super) fn draw_result_or_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &WorkspaceApp,
    view: WorkspaceView<'_>,
) {
    let sql_result_is_empty = view.active_right_tab == RightPaneTab::Sql
        && view
            .editor
            .map(|editor| editor.result_set_count == 0)
            .unwrap_or(true);
    if sql_result_is_empty || view.active_grid.columns.is_empty() {
        let paragraph = Paragraph::new(RUN_SQL_RESULTS_MESSAGE)
            .block(Block::default().borders(Borders::ALL).title(TITLE_RESULTS))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
        return;
    }

    draw_grid(
        frame,
        area,
        view.active_grid,
        TITLE_RESULTS,
        GridViewport::from_app_view(app, view),
    );
}

pub(super) fn draw_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &WorkspaceApp,
    view: WorkspaceView<'_>,
) {
    let Some(object) = view.selected_object else {
        return;
    };

    let title = if let Some(page) = app.preview_page_summary() {
        format!(
            "Preview {} {} | {}",
            object.kind.label(),
            object.qualified_name(),
            page
        )
    } else {
        format!(
            "Preview {} {}",
            object.kind.label(),
            object.qualified_name()
        )
    };

    if view.preview_loading && view.preview_grid.columns.is_empty() {
        let title = if view.data_grid_focused {
            format!("* {title}")
        } else {
            title
        };
        let placeholder = Paragraph::new(LOADING_PREVIEW_MESSAGE)
            .block(focusable_block(title, view.data_grid_focused))
            .wrap(Wrap { trim: true });
        frame.render_widget(placeholder, area);
        return;
    }

    draw_grid(
        frame,
        area,
        view.preview_grid,
        &title,
        GridViewport::from_app_view(app, view),
    );
}

pub(super) fn draw_row_inspector(
    frame: &mut Frame<'_>,
    area: Rect,
    inspector: RowInspectorView<'_>,
) {
    let popup = centered_rect(
        ROW_INSPECTOR_POPUP_WIDTH_PERCENT,
        ROW_INSPECTOR_POPUP_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);

    let block = Block::default().borders(Borders::ALL).title(format!(
        "Cell Details | row {} | Tab switch box | Esc close",
        inspector.row_index + 1,
    ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(ROW_INSPECTOR_FIELD_LIST_HEIGHT_PERCENT),
            Constraint::Percentage(ROW_INSPECTOR_DETAIL_HEIGHT_PERCENT),
        ])
        .split(inner);

    let items = inspector
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let value = inspector
                .values
                .get(index)
                .map(String::as_str)
                .unwrap_or(NULL_LABEL);
            ListItem::new(Line::from(vec![
                Span::styled(
                    column.as_str(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    compact_value_preview(value, ROW_INSPECTOR_VALUE_PREVIEW_LIMIT),
                    Style::default().fg(TEXT_DIM),
                ),
            ]))
        })
        .collect::<Vec<_>>();

    let fields_focused = matches!(inspector.active_pane, RowInspectorPane::Fields);
    let fields_block = focusable_block("Fields | j/k move | Tab preview", fields_focused);
    let fields_inner = fields_block.inner(sections[0]);
    frame.render_widget(fields_block, sections[0]);

    let mut state = ListState::default();
    if !inspector.columns.is_empty() {
        state.select(Some(inspector.selected_field));
    }
    let list = List::new(items)
        .highlight_style(highlight_style())
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, fields_inner, &mut state);

    let selected_column = inspector
        .columns
        .get(inspector.selected_field)
        .map(String::as_str)
        .unwrap_or("n/a");
    let selected_value = inspector
        .values
        .get(inspector.selected_field)
        .map(String::as_str)
        .unwrap_or(NULL_LABEL);
    let detail_value = format_detail_value(selected_value, inspector.formatted);
    let preview_focused = matches!(inspector.active_pane, RowInspectorPane::Preview);
    let detail_block = focusable_block(
        format!(
            "Preview: {selected_column} | {} char(s) | scroll {} | j/k or PgUp/PgDn scroll | Tab fields | f {} | y copy | e edit",
            selected_value.chars().count(),
            inspector.detail_scroll,
            if inspector.formatted { "raw" } else { "format" }
        ),
        preview_focused,
    );
    let detail = Paragraph::new(detail_value)
        .block(detail_block)
        .scroll((inspector.detail_scroll.min(u16::MAX as usize) as u16, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(detail, sections[1]);
}

pub(super) fn draw_command_palette(
    frame: &mut Frame<'_>,
    area: Rect,
    palette: CommandPaletteView<'_>,
) {
    let popup = centered_rect(
        COMMAND_PALETTE_WIDTH_PERCENT,
        COMMAND_PALETTE_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);

    let block = focusable_block("Command Palette | Esc close | Enter run", true);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(MODAL_SEARCH_HEIGHT),
            Constraint::Min(MODAL_BODY_MIN_HEIGHT),
        ])
        .split(inner);

    let query = Paragraph::new(format!("> {}", palette.query))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .title(TITLE_SEARCH),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(query, sections[0]);

    let items = if palette.items.is_empty() {
        vec![ListItem::new(Line::from(NO_MATCHING_COMMANDS_MESSAGE))]
    } else {
        palette
            .items
            .iter()
            .map(|item| ListItem::new(Line::from(format!("{} - {}", item.title, item.hint))))
            .collect::<Vec<_>>()
    };

    let list = List::new(items)
        .highlight_style(highlight_style())
        .highlight_symbol(">> ");
    let mut state = ListState::default();
    if !palette.items.is_empty() {
        state.select(Some(palette.selected_index));
    }
    frame.render_stateful_widget(list, sections[1], &mut state);
}

fn push_help_section(
    lines: &mut Vec<Line<'static>>,
    title: &'static str,
    shortcuts: &[(&'static str, &'static str)],
) {
    if !lines.is_empty() {
        lines.push(Line::from(String::new()));
    }

    lines.push(Line::from(Span::styled(
        title,
        Style::default()
            .fg(theme_accent_color())
            .add_modifier(Modifier::BOLD),
    )));

    for (keys, description) in shortcuts {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{keys:<12} "),
                Style::default()
                    .fg(TEXT_STRONG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(*description, Style::default().fg(TEXT_SECONDARY)),
        ]));
    }
}

pub(super) fn draw_help_overlay_static(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered_rect(
        HELP_OVERLAY_WIDTH_PERCENT,
        HELP_OVERLAY_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);

    let block = focusable_block(format!("{TITLE_KEYBOARD_HELP} | Esc close"), true);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(inner);
    let shortcut_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(sections[0]);

    let mut left_lines = Vec::new();
    push_help_section(&mut left_lines, HELP_SECTION_GLOBAL, &HELP_GLOBAL_SHORTCUTS);
    push_help_section(&mut left_lines, HELP_SECTION_DATA, &HELP_DATA_SHORTCUTS);
    let left = Paragraph::new(left_lines).wrap(Wrap { trim: false });
    frame.render_widget(left, shortcut_columns[0]);

    let mut right_lines = Vec::new();
    push_help_section(&mut right_lines, HELP_SECTION_SQL, &HELP_SQL_SHORTCUTS);
    push_help_section(
        &mut right_lines,
        HELP_SECTION_STRUCTURE,
        &HELP_STRUCTURE_SHORTCUTS,
    );
    let right = Paragraph::new(right_lines).wrap(Wrap { trim: false });
    frame.render_widget(right, shortcut_columns[1]);

    let mut capability_lines = Vec::new();
    push_help_section(
        &mut capability_lines,
        HELP_SECTION_DRIVER_SUPPORT,
        &HELP_DRIVER_SUPPORT_ROWS,
    );
    let capabilities = Paragraph::new(capability_lines).wrap(Wrap { trim: false });
    frame.render_widget(capabilities, sections[1]);
}

pub(super) fn draw_help_overlay(frame: &mut Frame<'_>, area: Rect, _view: WorkspaceView<'_>) {
    draw_help_overlay_static(frame, area);
}

fn identifier_quote_style_label(capabilities: DriverCapabilities) -> &'static str {
    match capabilities.identifier_quote_style {
        IdentifierQuoteStyle::DoubleQuote => "double quotes",
        IdentifierQuoteStyle::Backtick => "backticks",
    }
}

fn explain_capability_label(capabilities: DriverCapabilities) -> &'static str {
    match (
        capabilities.supports_explain,
        capabilities.explain_flavor,
        capabilities.supports_explain_analyze,
    ) {
        (false, _, _) => "no EXPLAIN",
        (true, ExplainFlavor::ExplainQueryPlan, _) => "QUERY PLAN",
        (true, ExplainFlavor::Explain, true) => "EXPLAIN + ANALYZE",
        (true, ExplainFlavor::Explain, false) => "EXPLAIN only",
    }
}

fn returning_capability_label(capabilities: DriverCapabilities) -> &'static str {
    if capabilities.supports_returning {
        "RETURNING"
    } else {
        "no RETURNING"
    }
}

fn support_label(name: &'static str, supported: bool) -> String {
    if supported {
        name.to_string()
    } else {
        format!("no {name}")
    }
}

pub(super) fn draw_sql_history(frame: &mut Frame<'_>, area: Rect, history: SqlHistoryView<'_>) {
    let popup = centered_rect(SQL_HISTORY_WIDTH_PERCENT, SQL_HISTORY_HEIGHT_PERCENT, area);
    frame.render_widget(Clear, popup);

    let block = focusable_block("SQL History | type search | Enter rerun | Esc close", true);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(MODAL_SEARCH_HEIGHT),
            Constraint::Min(MODAL_BODY_MIN_HEIGHT),
        ])
        .split(inner);

    let query = Paragraph::new(format!("? {}", history.query))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .title(TITLE_SEARCH),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(query, sections[0]);

    let items = if history.items.is_empty() {
        vec![ListItem::new(Line::from(NO_MATCHING_SQL_HISTORY_MESSAGE))]
    } else {
        history
            .items
            .iter()
            .map(|sql| ListItem::new(Line::from(sql.as_str())))
            .collect::<Vec<_>>()
    };

    let list = List::new(items)
        .highlight_style(highlight_style())
        .highlight_symbol(">> ");
    let mut state = ListState::default();
    if !history.items.is_empty() {
        state.select(Some(history.selected_index));
    }
    frame.render_stateful_widget(list, sections[1], &mut state);
}

pub(super) fn draw_saved_sql(frame: &mut Frame<'_>, area: Rect, saved_sql: SavedSqlView<'_>) {
    let popup = centered_rect(SQL_HISTORY_WIDTH_PERCENT, SQL_HISTORY_HEIGHT_PERCENT, area);
    frame.render_widget(Clear, popup);

    let block = focusable_block("Saved SQL | type search | Enter open | Esc close", true);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(MODAL_SEARCH_HEIGHT),
            Constraint::Min(MODAL_BODY_MIN_HEIGHT),
        ])
        .split(inner);

    let query = Paragraph::new(format!("? {}", saved_sql.query))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .title(TITLE_SEARCH),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(query, sections[0]);

    let items = if saved_sql.items.is_empty() {
        vec![ListItem::new(Line::from(NO_MATCHING_SAVED_SQL_MESSAGE))]
    } else {
        saved_sql
            .items
            .iter()
            .map(|entry| {
                let location = match (&entry.connection_name, &entry.database_name) {
                    (Some(connection), Some(database)) => format!("{connection} · {database}"),
                    (Some(connection), None) => connection.clone(),
                    (None, Some(database)) => database.clone(),
                    (None, None) => "workspace".to_string(),
                };
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            entry.name.as_str(),
                            Style::default()
                                .fg(TEXT_STRONG)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("  "),
                        Span::styled(location, Style::default().fg(theme_accent_color())),
                    ]),
                    Line::from(Span::styled(
                        saved_sql_preview(entry.sql.as_str()),
                        Style::default().fg(TEXT_MUTED),
                    )),
                ])
            })
            .collect::<Vec<_>>()
    };

    let list = List::new(items)
        .highlight_style(highlight_style())
        .highlight_symbol(">> ");
    let mut state = ListState::default();
    if !saved_sql.items.is_empty() {
        state.select(Some(saved_sql.selected_index));
    }
    frame.render_stateful_widget(list, sections[1], &mut state);
}

pub(super) fn draw_data_filter(frame: &mut Frame<'_>, area: Rect, filter: DataFilterView<'_>) {
    let popup = centered_rect(
        INLINE_MODAL_WIDTH_PERCENT,
        INLINE_MODAL_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);

    let active = filter
        .active_filter
        .map(|value| format!(" | active: {value}"))
        .unwrap_or_default();
    let paragraph = Paragraph::new(format!("/ {}{}", filter.input, active))
        .block(focusable_block(
            "Data Filter | Enter apply | Esc close",
            true,
        ))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, popup);
}

pub(super) fn draw_cell_edit(frame: &mut Frame<'_>, area: Rect, edit: CellEditView<'_>) {
    let popup = centered_rect(
        INLINE_MODAL_WIDTH_PERCENT,
        INLINE_MODAL_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);

    let paragraph = Paragraph::new(edit.input)
        .block(focusable_block(
            format!("Edit {} | Enter preview SQL | Esc close", edit.column),
            true,
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup);
}

pub(super) fn draw_save_sql_dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    save_sql_dialog: SaveSqlDialogView<'_>,
) {
    let popup = centered_rect(
        INLINE_MODAL_WIDTH_PERCENT,
        INLINE_MODAL_HEIGHT_PERCENT,
        area,
    );
    frame.render_widget(Clear, popup);

    let paragraph = Paragraph::new(save_sql_dialog.name)
        .block(focusable_block("Save SQL | Enter save | Esc close", true))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup);
}

fn saved_sql_preview(sql: &str) -> String {
    sql.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(72).collect::<String>())
        .unwrap_or_default()
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((FULL_PERCENT - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((FULL_PERCENT - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((FULL_PERCENT - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((FULL_PERCENT - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

pub(super) fn focusable_block<T>(title: T, focused: bool) -> Block<'static>
where
    T: Into<String>,
{
    let border_style = if focused {
        Style::default()
            .fg(theme_accent_color())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    Block::default()
        .borders(Borders::ALL)
        .title(title.into())
        .border_style(border_style)
}

pub(super) fn draw_footer(frame: &mut Frame<'_>, area: Rect, view: WorkspaceView<'_>) {
    let status = view.status.unwrap_or(READY_STATUS);
    let help = if view.help_overlay_visible {
        FOOTER_KEYBOARD_HELP
    } else if view.command_palette.is_some() {
        FOOTER_COMMAND_HELP
    } else if view.saved_sql.is_some() {
        FOOTER_SAVED_SQL_HELP
    } else if view.save_sql_dialog.is_some() {
        FOOTER_SAVE_SQL_HELP
    } else if view.sql_history.is_some() {
        FOOTER_SQL_HISTORY_HELP
    } else if view.data_filter.is_some() {
        FOOTER_DATA_FILTER_HELP
    } else if view.cell_edit.is_some() {
        FOOTER_CELL_EDIT_HELP
    } else if view.row_inspector.is_some() {
        FOOTER_ROW_INSPECTOR_HELP
    } else if view.editor_completion.is_some() {
        FOOTER_COMPLETION_HELP
    } else if view.active_right_tab == RightPaneTab::Sql && view.data_grid_focused {
        FOOTER_SQL_RESULTS_HELP
    } else if view.active_right_tab == RightPaneTab::Sql && view.assets_focused {
        FOOTER_SQL_ASSETS_HELP
    } else if view.active_right_tab == RightPaneTab::Sql && view.editor.is_some() {
        FOOTER_SQL_EDITOR_HELP
    } else if view.active_right_tab == RightPaneTab::Sql {
        FOOTER_SQL_TAB_HELP
    } else if view.active_right_tab == RightPaneTab::Structure && view.data_grid_focused {
        FOOTER_STRUCTURE_GRID_HELP
    } else if view.active_right_tab == RightPaneTab::Structure {
        FOOTER_STRUCTURE_HELP
    } else if view.data_grid_focused {
        FOOTER_DATA_GRID_HELP
    } else {
        FOOTER_DATA_HELP
    };
    let footer = Paragraph::new(format!("{status} | {help}"))
        .block(Block::default().borders(Borders::ALL).title(TITLE_STATUS))
        .wrap(Wrap { trim: true });

    frame.render_widget(footer, area);
}

pub(super) fn tree_item(row: &relora_app::tree::TreeRow) -> ListItem<'_> {
    ListItem::new(Line::from(row.rendered.as_str()))
}

pub(super) fn highlight_style() -> Style {
    Style::default()
        .fg(theme_accent_color())
        .add_modifier(Modifier::BOLD)
}

pub(super) fn theme_accent_color() -> Color {
    ACCENT
}

pub(super) fn active_tab_style() -> Style {
    Style::default()
        .fg(TEXT_INVERSE)
        .bg(theme_accent_color())
        .add_modifier(Modifier::BOLD)
}

pub(super) fn database_kind_label(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::Postgres => DATABASE_KIND_POSTGRES,
        DatabaseKind::MySql => DATABASE_KIND_MYSQL,
        DatabaseKind::Sqlite => DATABASE_KIND_SQLITE,
    }
}

pub(super) fn object_scope_label(kind: Option<DatabaseKind>, object: &DbObjectRef) -> String {
    if kind.is_some_and(|kind| kind.collapses_duplicate_schema(&object.database, &object.schema)) {
        format!("{}.{}", object.database, object.name)
    } else {
        object.database_qualified_name()
    }
}
