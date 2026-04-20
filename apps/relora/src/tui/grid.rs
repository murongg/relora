use super::*;

pub(super) fn draw_grid(
    frame: &mut Frame<'_>,
    area: Rect,
    grid: &TablePreview,
    title: &str,
    viewport: GridViewport,
) {
    let row_offset = viewport.row_offset.min(grid.rows.len().saturating_sub(1));
    let title = grid_title(
        title,
        row_offset,
        grid_first_scroll_column_index(&viewport, grid.columns.len()),
    );
    let title = if viewport.focused {
        format!("* {title}")
    } else {
        title
    };
    let block = focusable_block(title, viewport.focused);

    if grid.columns.is_empty() {
        let placeholder = Paragraph::new(EMPTY_GRID_MESSAGE)
            .block(block)
            .wrap(Wrap { trim: true });
        frame.render_widget(placeholder, area);
        return;
    }

    let columns = grid_column_layouts(area, grid, &viewport);
    let widths = columns
        .iter()
        .map(|column| Constraint::Length(column.width))
        .collect::<Vec<_>>();
    let header = Row::new(columns.iter().map(|column| {
        let label = grid
            .columns
            .get(column.index)
            .map(String::as_str)
            .unwrap_or_default();
        Cell::from(compact_value_preview(label, column.width as usize)).style(
            grid_header_cell_style(
                viewport.focused,
                column.index == viewport.selected_column_index,
            ),
        )
    }));

    let selected_row_index = viewport
        .selected_row_index
        .min(grid.rows.len().saturating_sub(1));
    let rows = grid
        .rows
        .iter()
        .enumerate()
        .skip(row_offset)
        .map(|(row_index, row)| {
            let row_selected = row_index == selected_row_index;
            Row::new(columns.iter().map(|column| {
                let column_selected = column.index == viewport.selected_column_index;
                let style = grid_body_cell_style(viewport.focused, row_selected, column_selected);
                let value = row
                    .get(column.index)
                    .map(String::as_str)
                    .unwrap_or_default();
                Cell::from(compact_value_preview(value, column.width as usize)).style(style)
            }))
            .height(1)
        });

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(GRID_COLUMN_SPACING)
        .highlight_symbol(">> ");
    let mut state = TableState::default();
    state.select(Some(
        viewport
            .selected_row_index
            .min(grid.rows.len().saturating_sub(1))
            .saturating_sub(row_offset),
    ));
    frame.render_stateful_widget(table, area, &mut state);
}

pub(super) fn grid_title(
    title: &str,
    row_offset: usize,
    first_scrolled_column_index: Option<usize>,
) -> String {
    let mut parts = Vec::new();
    if row_offset > 0 {
        parts.push(format!("row {}", row_offset + 1));
    }
    if let Some(column_index) = first_scrolled_column_index {
        parts.push(format!("col {}", column_index + 1));
    }

    if parts.is_empty() {
        title.to_string()
    } else {
        format!("{title} | {}", parts.join(", "))
    }
}

pub(super) fn grid_row_highlight_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(TEXT_INVERSE)
            .bg(theme_accent_color())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(TEXT_DIM)
    }
}

pub(super) fn grid_body_cell_style(
    focused: bool,
    row_selected: bool,
    column_selected: bool,
) -> Style {
    match (focused, row_selected, column_selected) {
        (true, true, true) => Style::default()
            .fg(TEXT_INVERSE)
            .bg(GRID_CURRENT_CELL_BACKGROUND)
            .add_modifier(Modifier::BOLD),
        (true, true, false) => grid_row_highlight_style(true),
        (true, false, true) => Style::default().fg(TEXT_STRONG).bg(GRID_COLUMN_BACKGROUND),
        (false, true, true) => Style::default()
            .fg(TEXT_STRONG)
            .bg(GRID_INACTIVE_CELL_BACKGROUND)
            .add_modifier(Modifier::BOLD),
        (false, true, false) => grid_row_highlight_style(false),
        (false, false, true) => Style::default()
            .fg(TEXT_GRID_COLUMN)
            .bg(GRID_INACTIVE_COLUMN_BACKGROUND),
        (_, false, false) => Style::default(),
    }
}

pub(super) fn grid_header_cell_style(focused: bool, selected: bool) -> Style {
    let mut style = Style::default()
        .fg(theme_accent_color())
        .add_modifier(Modifier::BOLD);
    if selected {
        if focused {
            style = style.fg(TEXT_INVERSE).bg(theme_accent_color());
        } else {
            style = style.bg(TEXT_DIM);
        }
    }
    style
}

pub(super) fn compact_value_preview(value: &str, limit: usize) -> String {
    let preview = value.replace('\n', " ");
    let char_count = preview.chars().count();
    if char_count <= limit {
        preview
    } else {
        let truncated = preview
            .chars()
            .take(limit.saturating_sub(1))
            .collect::<String>();
        format!("{truncated}…")
    }
}

pub(super) fn grid_column_layouts(
    area: Rect,
    grid: &TablePreview,
    viewport: &GridViewport,
) -> Vec<GridColumnLayout> {
    let inner_width = area.width.saturating_sub(2);
    if grid.columns.is_empty() || inner_width == 0 {
        return Vec::new();
    }

    let mut indexes = Vec::new();
    let frozen_count = viewport
        .frozen_leading_columns
        .min(grid.columns.len())
        .max(usize::from(
            viewport.column_offset > 0 && grid.columns.len() > 1,
        ));
    for index in 0..frozen_count {
        indexes.push(index);
    }

    let scroll_start = if viewport.column_offset > 0 {
        grid_first_scroll_column_index(viewport, grid.columns.len())
            .unwrap_or(frozen_count)
            .min(grid.columns.len().saturating_sub(1))
    } else {
        frozen_count
    };

    for index in scroll_start..grid.columns.len() {
        if indexes.contains(&index) {
            continue;
        }

        let mut required_width = grid_total_min_width(&indexes);
        if !indexes.is_empty() {
            required_width = required_width.saturating_add(GRID_COLUMN_SPACING);
        }
        required_width = required_width.saturating_add(grid_min_column_width(index));

        if required_width > inner_width {
            if indexes.is_empty() {
                indexes.push(index);
            }
            break;
        }

        indexes.push(index);
    }

    if indexes.is_empty() {
        indexes.push(0);
    }

    let mut widths = indexes
        .iter()
        .map(|&index| preferred_grid_column_width(grid, index, viewport.row_offset, viewport))
        .collect::<Vec<_>>();
    shrink_grid_column_widths(&mut widths, &indexes, inner_width);

    indexes
        .into_iter()
        .zip(widths)
        .map(|(index, width)| GridColumnLayout { index, width })
        .collect()
}

pub(super) fn grid_first_scroll_column_index(
    viewport: &GridViewport,
    total_columns: usize,
) -> Option<usize> {
    if viewport.column_offset == 0 || total_columns == 0 {
        return None;
    }

    let explicit_frozen_count = viewport.frozen_leading_columns.min(total_columns);
    let auto_frozen_count =
        usize::from(viewport.column_offset > 0 && explicit_frozen_count == 0 && total_columns > 1);
    let frozen_count = explicit_frozen_count.max(auto_frozen_count);
    let scroll_start = if explicit_frozen_count > 0 {
        viewport.column_offset.saturating_add(1)
    } else {
        viewport.column_offset
    };
    Some(
        scroll_start
            .max(frozen_count)
            .min(total_columns.saturating_sub(1)),
    )
}

pub(super) fn grid_total_min_width(indexes: &[usize]) -> u16 {
    indexes
        .iter()
        .map(|&index| grid_min_column_width(index))
        .sum::<u16>()
        .saturating_add(GRID_COLUMN_SPACING.saturating_mul(indexes.len().saturating_sub(1) as u16))
}

pub(super) fn grid_min_column_width(index: usize) -> u16 {
    if index == 0 { 6 } else { 8 }
}

pub(super) fn preferred_grid_column_width(
    grid: &TablePreview,
    index: usize,
    row_offset: usize,
    viewport: &GridViewport,
) -> u16 {
    if let Some(width) = viewport.width_override(index) {
        return width.clamp(grid_min_column_width(index), GRID_MAX_COLUMN_WIDTH);
    }

    let header_width = grid
        .columns
        .get(index)
        .map(|value| display_text_width(value))
        .unwrap_or_default();
    let sample_width = grid
        .rows
        .iter()
        .skip(row_offset)
        .take(GRID_SAMPLE_ROW_COUNT)
        .filter_map(|row| row.get(index))
        .map(|value| display_text_width(value))
        .max()
        .unwrap_or_default();
    let preferred = header_width.max(sample_width).clamp(
        grid_min_column_width(index) as usize,
        GRID_MAX_COLUMN_WIDTH as usize,
    );
    preferred as u16
}

pub(super) fn display_text_width(value: &str) -> usize {
    value
        .replace('\n', " ")
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_else(|| value.chars().count())
}

pub(super) fn shrink_grid_column_widths(
    widths: &mut [u16],
    indexes: &[usize],
    available_width: u16,
) {
    if widths.is_empty() {
        return;
    }

    let max_width = available_width
        .saturating_sub(GRID_COLUMN_SPACING.saturating_mul(widths.len().saturating_sub(1) as u16));
    let mut current_width = widths.iter().sum::<u16>();
    while current_width > max_width {
        let Some((position, _)) = widths
            .iter()
            .enumerate()
            .filter(|(position, width)| **width > grid_min_column_width(indexes[*position]))
            .max_by_key(|(_, width)| *width)
        else {
            break;
        };
        widths[position] = widths[position].saturating_sub(1);
        current_width = current_width.saturating_sub(1);
    }
}

pub(super) fn format_detail_value(value: &str, formatted: bool) -> String {
    if !formatted {
        return value.to_string();
    }

    let trimmed = value.trim();
    if let Ok(json) = serde_json::from_str::<JsonValue>(trimmed) {
        return serde_json::to_string_pretty(&json).unwrap_or_else(|_| value.to_string());
    }

    if let Some(items) = parse_postgres_array_items(trimmed) {
        if items.is_empty() {
            return "[]".to_string();
        }
        return items
            .into_iter()
            .enumerate()
            .map(|(index, item)| format!("[{index}] {item}"))
            .collect::<Vec<_>>()
            .join("\n");
    }

    value.to_string()
}

pub(super) fn parse_postgres_array_items(value: &str) -> Option<Vec<String>> {
    if !(value.starts_with('{') && value.ends_with('}')) {
        return None;
    }

    let inner = &value[1..value.len().saturating_sub(1)];
    if inner.is_empty() {
        return Some(Vec::new());
    }

    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape = false;

    for ch in inner.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        match ch {
            '\\' if in_quotes => escape = true,
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                items.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if in_quotes || escape {
        return None;
    }

    items.push(current.trim().to_string());
    Some(items)
}
