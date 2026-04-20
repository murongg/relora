use super::*;

pub(super) fn workspace_body_area(area: Rect) -> Rect {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(WORKSPACE_HEADER_HEIGHT),
            Constraint::Min(WORKSPACE_MIN_BODY_HEIGHT),
            Constraint::Length(WORKSPACE_FOOTER_HEIGHT),
        ])
        .split(area)[1]
}

pub(super) fn workspace_main_sections(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(WORKSPACE_ASSETS_WIDTH_PERCENT),
            Constraint::Percentage(WORKSPACE_DETAILS_WIDTH_PERCENT),
        ])
        .split(area)
        .to_vec()
}

pub(super) fn workspace_detail_sections(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(WORKSPACE_TABS_HEIGHT),
            Constraint::Length(WORKSPACE_SUMMARY_HEIGHT),
            Constraint::Min(WORKSPACE_DETAILS_MIN_HEIGHT),
        ])
        .split(area)
        .to_vec()
}

pub(super) fn sql_tab_sections(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(SQL_EDITOR_HEIGHT_PERCENT),
            Constraint::Percentage(SQL_RESULTS_HEIGHT_PERCENT),
        ])
        .split(area)
        .to_vec()
}

pub(super) fn rect_contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}
