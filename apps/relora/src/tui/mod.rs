use std::{
    io,
    ops::{Deref, DerefMut},
};

use anyhow::Result;
use arboard::Clipboard;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, KeyboardEnhancementFlags, MouseButton, MouseEvent, MouseEventKind,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
        Wrap,
    },
};
use relora_app::{
    completion::CompletionKind,
    syntax::{SqlTokenKind, highlight_sql_line},
    view::{
        CellEditView, CommandPaletteView, DataFilterView, DeleteConfirmationView, EditorView,
        RightPaneTab, RowInspectorPane, RowInspectorView, SqlHistoryView, WorkspaceView,
    },
};
use relora_core::db::{DatabaseKind, TablePreview};
use serde_json::Value as JsonValue;

use crate::{
    config::{AppConfig, ConnectionConfig, LaunchMode},
    drivers,
    launcher::{LauncherAction, LauncherApp, LauncherFormField},
    workspace::{ConnectionBootstrap, WorkspaceAction, WorkspaceApp},
};

// Keep the runtime/bootstrap entrypoints here and fan out the bulky interaction
// and rendering logic into focused submodules.
mod colors;
mod grid;
mod input;
mod layout;
mod metrics;
mod render;
mod shortcuts;
mod strings;
#[cfg(test)]
mod tests;

use self::{
    colors::*, grid::*, input::*, layout::*, metrics::*, render::*, shortcuts::*, strings::*,
};

#[derive(Debug, Clone)]
struct GridViewport {
    selected_row_index: usize,
    selected_column_index: usize,
    row_offset: usize,
    column_offset: usize,
    focused: bool,
    width_overrides: Vec<(usize, u16)>,
    frozen_leading_columns: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridColumnLayout {
    index: usize,
    width: u16,
}

impl GridViewport {
    fn from_app_view(app: &WorkspaceApp, view: WorkspaceView<'_>) -> Self {
        Self {
            selected_row_index: view.grid_selected_row_index,
            selected_column_index: view.grid_selected_column_index,
            row_offset: view.grid_scroll_offset,
            column_offset: view.grid_column_offset,
            focused: view.data_grid_focused,
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
        }
    }

    fn width_override(&self, column_index: usize) -> Option<u16> {
        self.width_overrides
            .iter()
            .find(|(index, _)| *index == column_index)
            .map(|(_, width)| *width)
    }
}

trait ClipboardSink {
    fn set_text(&mut self, text: &str) -> Result<()>;
}

struct SystemClipboard {
    clipboard: Clipboard,
}

enum AppShell {
    Launcher(Box<LauncherApp>),
    Workspace(Box<WorkspaceShell>),
}

struct WorkspaceShell {
    workspace: WorkspaceApp,
    launcher: Option<Box<LauncherApp>>,
}

impl WorkspaceShell {
    fn with_launcher(workspace: WorkspaceApp, launcher: LauncherApp) -> Self {
        Self {
            workspace,
            launcher: Some(Box::new(launcher)),
        }
    }

    fn take_launcher(&mut self) -> Option<Box<LauncherApp>> {
        self.launcher.take()
    }

    fn launcher_available(&self) -> bool {
        self.launcher.is_some()
    }
}

impl From<WorkspaceApp> for Box<WorkspaceShell> {
    fn from(workspace: WorkspaceApp) -> Self {
        Box::new(WorkspaceShell {
            workspace,
            launcher: None,
        })
    }
}

impl Deref for WorkspaceShell {
    type Target = WorkspaceApp;

    fn deref(&self) -> &Self::Target {
        &self.workspace
    }
}

impl DerefMut for WorkspaceShell {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.workspace
    }
}

impl SystemClipboard {
    fn new() -> Result<Self> {
        Ok(Self {
            clipboard: Clipboard::new()?,
        })
    }
}

impl ClipboardSink for SystemClipboard {
    fn set_text(&mut self, text: &str) -> Result<()> {
        self.clipboard.set_text(text.to_string())?;
        Ok(())
    }
}

pub fn run(config: AppConfig) -> Result<()> {
    let mut app = match config.launch_mode {
        LaunchMode::Workspace => {
            let launcher = LauncherApp::with_preview_limit(
                launcher_connections_for_workspace(&config),
                config.connection_store_path.clone(),
                config.preview_limit,
            );
            AppShell::Workspace(Box::new(WorkspaceShell::with_launcher(
                bootstrap_workspace(&config.connections, config.preview_limit)?,
                launcher,
            )))
        }
        LaunchMode::Launcher => AppShell::Launcher(Box::new(LauncherApp::with_preview_limit(
            config.saved_connections,
            config.connection_store_path,
            config.preview_limit,
        ))),
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    let keyboard_enhancement_enabled = matches!(supports_keyboard_enhancement(), Ok(true));
    if keyboard_enhancement_enabled {
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
    } else {
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut clipboard = SystemClipboard::new().ok();

    let run_result = run_loop(
        &mut terminal,
        &mut app,
        clipboard
            .as_mut()
            .map(|clipboard| clipboard as &mut dyn ClipboardSink),
    );

    disable_raw_mode()?;
    if keyboard_enhancement_enabled {
        execute!(
            terminal.backend_mut(),
            PopKeyboardEnhancementFlags,
            DisableMouseCapture,
            LeaveAlternateScreen
        )?;
    } else {
        execute!(
            terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        )?;
    }
    terminal.show_cursor()?;

    run_result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut AppShell,
    mut clipboard: Option<&mut dyn ClipboardSink>,
) -> Result<()> {
    let mut last_synced_copy_sequence = 0;
    loop {
        if let AppShell::Workspace(workspace) = app {
            workspace.drain_background()?;
        }
        terminal.draw(|frame| draw(frame, app))?;

        if !event::poll(EVENT_POLL_INTERVAL)? {
            if matches!(app, AppShell::Workspace(workspace) if workspace.should_quit()) {
                break;
            }
            continue;
        }

        let should_quit = match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => handle_shell_key(app, key)?,
            Event::Mouse(mouse) => {
                let size = terminal.size()?;
                let area = Rect::new(0, 0, size.width, size.height);
                handle_shell_mouse(app, mouse, area)?
            }
            _ => false,
        };

        if should_quit || matches!(app, AppShell::Workspace(workspace) if workspace.should_quit()) {
            break;
        }

        if let AppShell::Workspace(workspace) = app {
            if let Some(clipboard) = clipboard.as_deref_mut() {
                let _ =
                    sync_clipboard_if_needed(workspace, clipboard, &mut last_synced_copy_sequence);
            }
        }
    }

    Ok(())
}

fn bootstrap_workspace(
    connections: &[ConnectionConfig],
    preview_limit: usize,
) -> Result<WorkspaceApp> {
    let bootstraps = connections
        .iter()
        .map(|connection| {
            Ok(ConnectionBootstrap {
                name: connection.name.clone(),
                driver: drivers::connect(connection)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    WorkspaceApp::bootstrap(bootstraps, preview_limit)
}

fn launcher_connections_for_workspace(config: &AppConfig) -> Vec<ConnectionConfig> {
    let mut connections = config.saved_connections.clone();
    for connection in &config.connections {
        if !connections.iter().any(|saved| saved == connection) {
            connections.push(connection.clone());
        }
    }
    connections
}

fn sync_clipboard_if_needed(
    app: &WorkspaceApp,
    clipboard: &mut dyn ClipboardSink,
    last_synced_copy_sequence: &mut u64,
) -> Result<bool> {
    let copy_sequence = app.copy_sequence();
    if copy_sequence == 0 || copy_sequence == *last_synced_copy_sequence {
        return Ok(false);
    }

    *last_synced_copy_sequence = copy_sequence;
    let Some(text) = app.last_copied_text() else {
        return Ok(false);
    };
    clipboard.set_text(text)?;
    Ok(true)
}
