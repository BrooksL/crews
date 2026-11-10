//! App-style floating layers (`ui.style = "app"`): the global menu popover,
//! context menus, the scrim behind them, and larger overlays floating in the
//! panel.

use super::*;
use crate::app::app_tokens::{mix, rgb, AppTokens};
use crate::config::{CornersConfig, UiStyleConfig};

const COLS: u16 = 177;
const ROWS: u16 = 48;

fn config(style: UiStyleConfig) -> ClientShellConfig {
    let mut config = Config::default();
    config.ui.style = style;
    config.ui.corners = CornersConfig::Square;
    config.theme.name = Some("crews".to_string());
    ClientShellConfig::from_config(&config)
}

fn state(style: UiStyleConfig) -> ClientShellState {
    let mut state = ClientShellState::new(config(style));
    state.set_snapshot(Box::new(snapshot()));
    state
}

fn app() -> AppTokens {
    crate::app::state::Palette::crews().app.unwrap()
}

fn color(color: ratatui::style::Color) -> u32 {
    crate::protocol::color_to_u32(color)
}

fn scrimmed(base: ratatui::style::Color) -> u32 {
    let (r, g, b) = mix(rgb(base).unwrap(), rgb(app().scrim).unwrap(), 0.5);
    color(ratatui::style::Color::Rgb(r, g, b))
}

fn compose(state: &mut ClientShellState) -> FrameData {
    state.set_pane_surface(surface());
    state.compose(COLS, ROWS).expect("compose").frame
}

fn cell(frame: &FrameData, x: u16, y: u16) -> &crate::protocol::CellData {
    &frame.cells[usize::from(y) * usize::from(frame.width) + usize::from(x)]
}

fn row_text(frame: &FrameData, rect: Rect) -> String {
    (rect.x..rect.right())
        .map(|x| cell(frame, x, rect.y).symbol.as_str())
        .collect()
}

fn mouse(state: &mut ClientShellState, kind: MouseEventKind, column: u16, row: u16) {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })]);
}

fn open_global_menu(state: &mut ClientShellState) -> FrameData {
    compose(state);
    state.toggle_global_menu();
    compose(state)
}

#[test]
fn global_menu_opens_above_the_menu_button() {
    let mut state = state(UiStyleConfig::App);
    let frame = open_global_menu(&mut state);
    let app = app();
    let launcher = state.hits.global_launcher;
    let rows = state.hits.global_menu_rows.clone();
    // settings, keybinds, reload config, detach.
    assert_eq!(rows.len(), 4);

    // 25 columns wide, right edge on the button's right edge, one column of
    // padding around each row pill.
    let left = launcher.right() - 25;
    for (rect, _) in &rows {
        assert_eq!((rect.x, rect.width), (left + 1, 23));
    }
    // The bottom cap is on the row above the button; the divider sits above
    // the last row.
    let detach = rows[3].0;
    assert_eq!(detach.y, launcher.y - 2);
    assert_eq!(rows[2].0.y, detach.y - 2);
    assert_eq!(cell(&frame, left, launcher.y - 1).symbol, "▀");
    assert_eq!(cell(&frame, left + 1, detach.y - 1).symbol, "─");
    assert_eq!(
        cell(&frame, left + 1, detach.y - 1).fg,
        color(app.float_divider)
    );
    assert_eq!(cell(&frame, left, rows[0].0.y - 1).symbol, "▄");

    // The first row is selected on open.
    let selected = cell(&frame, rows[0].0.x + 1, rows[0].0.y);
    assert_eq!(selected.symbol, "s");
    assert_eq!(selected.bg, color(app.float_hover_bg));
    assert_eq!(selected.fg, color(app.text_strong));
    assert_eq!(
        cell(&frame, rows[1].0.x + 1, rows[1].0.y).bg,
        color(app.float_bg)
    );

    // Detach shows its keys, right-aligned, read from the keybinds.
    let text = row_text(&frame, detach);
    assert!(text.trim_end().ends_with("⌃B q"), "{text:?}");
    let hint = cell(&frame, detach.right() - 2, detach.y);
    assert_eq!(hint.fg, color(app.toast_body));
}

#[test]
fn keyboard_selection_moves_the_hover_pill() {
    let mut state = state(UiStyleConfig::App);
    open_global_menu(&mut state);
    state.move_global_menu_selection(3);
    let frame = compose(&mut state);
    let app = app();
    let palette = crate::app::state::Palette::crews();
    let rows = state.hits.global_menu_rows.clone();
    assert_eq!(
        cell(&frame, rows[0].0.x + 1, rows[0].0.y).bg,
        color(app.float_bg)
    );
    let detach = rows[3].0;
    assert_eq!(
        cell(&frame, detach.x + 1, detach.y).bg,
        color(app.float_hover_bg)
    );
    // On the hover fill the hint switches to `text` for contrast.
    assert_eq!(
        cell(&frame, detach.right() - 2, detach.y).fg,
        color(palette.text)
    );

    // Selection stops at the last row.
    state.move_global_menu_selection(5);
    let frame = compose(&mut state);
    assert_eq!(
        cell(&frame, detach.x + 1, detach.y).bg,
        color(app.float_hover_bg)
    );
}

#[test]
fn scrim_fades_everything_outside_the_menu() {
    let mut state = state(UiStyleConfig::App);
    let closed = compose(&mut state);
    let open = open_global_menu(&mut state);
    let app = app();
    let palette = crate::app::state::Palette::crews();

    // Pane content: default colors blend from the theme text and pane fill.
    let pane = state.hits.panes[0].inner_rect;
    let content = cell(&open, pane.x, pane.y);
    assert_eq!(content.symbol, "L");
    assert_eq!(content.fg, scrimmed(palette.text));
    assert_eq!(content.bg, scrimmed(app.pane_bg));

    // Chrome: the status bar row is blended from its own color.
    let status = cell(&open, 2, ROWS - 1);
    assert_eq!(cell(&closed, 2, ROWS - 1).bg, color(app.chrome_bg));
    assert_eq!(status.bg, scrimmed(app.chrome_bg));

    // The menu itself is untouched, and its caps sit on the faded chrome.
    let first = state.hits.global_menu_rows[1].0;
    assert_eq!(cell(&open, first.x, first.y).bg, color(app.float_bg));
    let cap = cell(&open, first.x, state.hits.global_launcher.y - 1);
    assert_eq!(cap.fg, color(app.float_bg));
    assert_ne!(cap.bg, color(app.float_bg));
}

#[test]
fn clicking_the_scrim_closes_the_menu() {
    let mut state = state(UiStyleConfig::App);
    let before = compose(&mut state);
    open_global_menu(&mut state);
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), 100, 20);
    assert!(state.overlay.is_none());
    let after = compose(&mut state);
    let pane = state.hits.panes[0].inner_rect;
    assert_eq!(
        cell(&after, pane.x, pane.y).bg,
        cell(&before, pane.x, pane.y).bg
    );
}

#[test]
fn clicking_a_row_runs_it() {
    let mut state = state(UiStyleConfig::App);
    open_global_menu(&mut state);
    let keybinds = state.hits.global_menu_rows[1].0;
    mouse(
        &mut state,
        MouseEventKind::Down(MouseButton::Left),
        keybinds.x + 2,
        keybinds.y,
    );
    assert!(matches!(state.overlay, Some(ClientShellOverlay::Help(_))));
}

#[test]
fn context_menu_opens_at_the_pointer_and_stays_on_screen() {
    let mut state = state(UiStyleConfig::App);
    compose(&mut state);
    state.open_pane_context_menu("pane_1".into(), 80, 20);
    let frame = compose(&mut state);
    let rows = state.hits.context_menu_rows.clone();
    assert!(!rows.is_empty());
    // Top cap on the clicked row, content from the next one.
    assert_eq!(rows[0].0.y, 21);
    assert_eq!(rows[0].0.x, 81);
    assert_eq!(cell(&frame, 80, 20).symbol, "▄");
    assert!(rows[0].0.width >= 23);
    assert_eq!(
        cell(&frame, rows[0].0.x + 1, rows[0].0.y).bg,
        color(app().float_hover_bg)
    );

    // Near the bottom-right corner it moves back inside the window.
    state.overlay = None;
    state.open_pane_context_menu("pane_1".into(), COLS - 2, ROWS - 2);
    compose(&mut state);
    let rows = state.hits.context_menu_rows.clone();
    let last = rows.last().unwrap().0;
    assert!(last.right() < COLS);
    assert!(last.bottom() < ROWS);
}

#[test]
fn header_more_button_right_aligns_the_context_menu() {
    let mut state = state(UiStyleConfig::App);
    compose(&mut state);
    let button = state.hits.header_more;
    mouse(
        &mut state,
        MouseEventKind::Down(MouseButton::Left),
        button.x + 1,
        button.y,
    );
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::ContextMenu(_))
    ));
    compose(&mut state);
    let row = state.hits.context_menu_rows[0].0;
    // Row pills sit one column inside the popover's edges.
    assert_eq!(row.right() + 1, button.right());
    assert_eq!(row.y, button.bottom() + 1);
}

#[test]
fn large_overlays_float_centered_in_the_panel() {
    let mut state = state(UiStyleConfig::App);
    compose(&mut state);
    state.overlay = Some(ClientShellOverlay::Help(ClientHelpOverlay {
        query: TextEditor::default(),
        search_focused: false,
        scroll: 0,
    }));
    let frame = compose(&mut state);
    let app = app();
    let popup = state.hits.help_popup;
    let panel = state.layout(COLS, ROWS).panel;
    // Centered in the panel, not the window.
    let popup_center = popup.x + popup.width / 2;
    let panel_center = panel.x + panel.width / 2;
    assert!(
        popup_center.abs_diff(panel_center) <= 1,
        "{popup:?} in {panel:?}"
    );

    // No border: the classic corner is now float fill, with caps outside.
    let corner = cell(&frame, popup.x, popup.y);
    assert_eq!(corner.symbol, " ");
    assert_eq!(corner.bg, color(app.float_bg));
    assert_eq!(cell(&frame, popup.x, popup.y - 1).symbol, "▄");
    assert_eq!(cell(&frame, popup.x, popup.bottom()).symbol, "▀");
    assert!(frame.cells.iter().all(|cell| cell.symbol != "┌"));

    // The scrim is behind it.
    let pane = state.hits.panes.first().map(|hit| hit.inner_rect);
    let status = cell(&frame, 2, ROWS - 1);
    assert_eq!(status.bg, scrimmed(app.chrome_bg), "{pane:?}");
}

#[test]
fn rounded_mode_recolors_corners_under_the_scrim_and_shadows_the_menu() {
    let mut config = Config::default();
    config.ui.style = UiStyleConfig::App;
    config.ui.corners = CornersConfig::Rounded;
    config.theme.name = Some("crews".to_string());
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.set_graphics_cell_size(10, 21);
    state.set_pane_surface(surface());
    let closed = state.compose(COLS, ROWS).expect("compose").graphics;
    let closed = String::from_utf8_lossy(&closed.into_inline_bytes()).into_owned();
    state.toggle_global_menu();
    let open = state.compose(COLS, ROWS).expect("compose").graphics;
    let open = String::from_utf8_lossy(&open.into_inline_bytes()).into_owned();
    // Nothing changed between frames except the menu, so any upload is a
    // faded corner color, the popover's own corners, or its shadow.
    assert!(closed.contains("a=t,"));
    assert!(open.contains("a=t,"));
    let reopened = state.compose(COLS, ROWS).expect("compose").graphics;
    let reopened = String::from_utf8_lossy(&reopened.into_inline_bytes()).into_owned();
    assert!(
        !reopened.contains("a=t,"),
        "cached while the menu stays open"
    );
}

#[test]
fn classic_menus_keep_their_border_and_no_scrim() {
    let mut state = state(UiStyleConfig::Classic);
    let closed = compose(&mut state);
    let open = open_global_menu(&mut state);
    assert!(open.cells.iter().any(|cell| cell.symbol == "┌"));
    let pane = state.hits.panes[0].inner_rect;
    assert_eq!(
        cell(&open, pane.x, pane.y).bg,
        cell(&closed, pane.x, pane.y).bg
    );
}
