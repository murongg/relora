use std::time::Duration;

pub(super) const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(75);

pub(super) const WORKSPACE_HEADER_HEIGHT: u16 = 1;
pub(super) const WORKSPACE_MIN_BODY_HEIGHT: u16 = 10;
pub(super) const WORKSPACE_FOOTER_HEIGHT: u16 = 3;
pub(super) const WORKSPACE_ASSETS_WIDTH_PERCENT: u16 = 30;
pub(super) const WORKSPACE_DETAILS_WIDTH_PERCENT: u16 = 70;
pub(super) const WORKSPACE_TABS_HEIGHT: u16 = 3;
pub(super) const WORKSPACE_SUMMARY_HEIGHT: u16 = 7;
pub(super) const WORKSPACE_DETAILS_MIN_HEIGHT: u16 = 8;
pub(super) const SQL_EDITOR_HEIGHT_PERCENT: u16 = 55;
pub(super) const SQL_RESULTS_HEIGHT_PERCENT: u16 = 45;

pub(super) const GRID_COLUMN_SPACING: u16 = 1;
pub(super) const GRID_MAX_COLUMN_WIDTH: u16 = 32;
pub(super) const GRID_SAMPLE_ROW_COUNT: usize = 12;

pub(super) const LAUNCHER_CARD_WIDTH_PERCENT: u16 = 60;
pub(super) const LAUNCHER_CARD_HEIGHT_PERCENT: u16 = 66;
pub(super) const LAUNCHER_ACCENT_HEIGHT: u16 = 1;
pub(super) const LAUNCHER_CARD_MARGIN: u16 = 2;
pub(super) const LAUNCHER_LOGO_HEIGHT: u16 = 3;
pub(super) const LAUNCHER_BRAND_COPY_HEIGHT: u16 = 2;
pub(super) const LAUNCHER_SECTION_HEADER_HEIGHT: u16 = 2;
pub(super) const LAUNCHER_LIST_MIN_HEIGHT: u16 = 3;
pub(super) const LAUNCHER_FOOTER_HEIGHT: u16 = 3;

pub(super) const CONNECTION_FORM_WIDTH_PERCENT: u16 = 70;
pub(super) const CONNECTION_FORM_HEIGHT_PERCENT: u16 = 72;
pub(super) const CONNECTION_FORM_MARGIN: u16 = 1;
pub(super) const CONNECTION_FORM_FIELD_HEIGHT: u16 = 2;
pub(super) const CONNECTION_FORM_HELP_MIN_HEIGHT: u16 = 1;
pub(super) const CONNECTION_FORM_FIELD_COUNT: usize = 8;
pub(super) const DELETE_CONFIRM_WIDTH_PERCENT: u16 = 58;
pub(super) const DELETE_CONFIRM_HEIGHT_PERCENT: u16 = 38;
pub(super) const DRIVER_MISSING_HEIGHT_PERCENT: u16 = 48;
pub(super) const DELETE_CONFIRM_MARGIN: u16 = 1;

pub(super) const COMPLETION_MIN_WIDTH: u16 = 20;
pub(super) const COMPLETION_MAX_WIDTH: u16 = 48;
pub(super) const COMPLETION_MIN_HEIGHT: u16 = 3;
pub(super) const COMPLETION_MAX_HEIGHT: u16 = 8;
pub(super) const POPUP_CHROME_HEIGHT: u16 = 2;

pub(super) const ROW_INSPECTOR_POPUP_WIDTH_PERCENT: u16 = 78;
pub(super) const ROW_INSPECTOR_POPUP_HEIGHT_PERCENT: u16 = 76;
pub(super) const ROW_INSPECTOR_FIELD_LIST_HEIGHT_PERCENT: u16 = 36;
pub(super) const ROW_INSPECTOR_DETAIL_HEIGHT_PERCENT: u16 = 64;
pub(super) const ROW_INSPECTOR_VALUE_PREVIEW_LIMIT: usize = 72;

pub(super) const COMMAND_PALETTE_WIDTH_PERCENT: u16 = 72;
pub(super) const COMMAND_PALETTE_HEIGHT_PERCENT: u16 = 45;
pub(super) const SQL_HISTORY_WIDTH_PERCENT: u16 = 76;
pub(super) const SQL_HISTORY_HEIGHT_PERCENT: u16 = 55;
pub(super) const HELP_OVERLAY_WIDTH_PERCENT: u16 = 74;
pub(super) const HELP_OVERLAY_HEIGHT_PERCENT: u16 = 70;
pub(super) const INLINE_MODAL_WIDTH_PERCENT: u16 = 70;
pub(super) const INLINE_MODAL_HEIGHT_PERCENT: u16 = 18;
pub(super) const MODAL_SEARCH_HEIGHT: u16 = 3;
pub(super) const MODAL_BODY_MIN_HEIGHT: u16 = 3;

pub(super) const FULL_PERCENT: u16 = 100;
