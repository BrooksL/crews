//! App-style panel header (`ui.style = "app"`): title, branch, tabs as a
//! segmented control, the new tab button, and the icon buttons.

use super::*;
use crate::config::UiStyleConfig;

const COLS: u16 = 177;
const ROWS: u16 = 48;
const HEADER_Y: u16 = 1;

fn app_config() -> ClientShellConfig {
    let mut config = Config::default();
    config.ui.style = UiStyleConfig::App;
    config.theme.name = Some("crews".to_string());
    ClientShellConfig::from_config(&config)
}

fn app() -> crate::app::app_tokens::AppTokens {
    crate::app::state::Palette::crews().app.unwrap()
}

fn color(color: ratatui::style::Color) -> u32 {
    crate::protocol::color_to_u32(color)
}

fn tab(number: u16, label: Option<&str>, focused: bool) -> ClientShellTab {
    ClientShellTab {
        tab_id: format!("tab_{number}"),
        workspace_id: "ws_1".into(),
        number: number.into(),
        label: label.map_or_else(|| number.to_string(), str::to_string),
        custom_label: label.is_some(),
        zoomed: false,
        focused,
        agent_status: AgentStatus::Idle,
    }
}

fn state_with_tabs(tabs: Vec<ClientShellTab>) -> ClientShellState {
    let mut snapshot = snapshot();
    snapshot.workspaces[0].label = "check-ins".into();
    snapshot.workspaces[0].branch = Some("lb/polish".into());
    snapshot.tabs = tabs;
    let mut state = ClientShellState::new(app_config());
    state.set_snapshot(Box::new(snapshot));
    state
}

fn compose(state: &mut ClientShellState) -> FrameData {
    state.set_pane_surface(surface());
    state.compose(COLS, ROWS).expect("compose").frame
}

fn cell(frame: &FrameData, x: u16, y: u16) -> &crate::protocol::CellData {
    &frame.cells[usize::from(y) * usize::from(frame.width) + usize::from(x)]
}

fn header_text(frame: &FrameData) -> String {
    frame_rows(frame)[usize::from(HEADER_Y)].clone()
}

fn click(state: &mut ClientShellState, rect: Rect) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x + rect.width / 2,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    })])
}

#[test]
fn header_shows_title_branch_tabs_and_buttons() {
    let mut state = state_with_tabs(vec![tab(1, None, true), tab(2, Some("server"), false)]);
    let frame = compose(&mut state);
    let app = app();
    let palette = crate::app::state::Palette::crews();
    let text = header_text(&frame);

    // Title two columns in from the panel edge (column 36).
    assert_eq!(cell(&frame, 38, HEADER_Y).symbol, "c");
    assert_eq!(cell(&frame, 38, HEADER_Y).fg, color(app.text_strong));
    let branch_x = 38 + "check-ins ".len() as u16;
    assert_eq!(cell(&frame, branch_x, HEADER_Y).fg, color(palette.accent));
    assert!(text.contains("check-ins lb/polish"), "{text:?}");

    // Tabs: two columns after the branch, inside the segment track.
    let tabs = &state.hits.tabs;
    assert_eq!(tabs.len(), 2);
    let track_x = branch_x + "lb/polish".len() as u16 + 2;
    assert_eq!(tabs[0].0, Rect::new(track_x + 1, HEADER_Y, 3, 1));
    assert_eq!(cell(&frame, track_x, HEADER_Y).bg, color(app.segment_bg));
    let active = cell(&frame, track_x + 2, HEADER_Y);
    assert_eq!(
        (active.symbol.as_str(), active.bg, active.fg),
        ("1", color(app.card_focus_bg), color(palette.accent))
    );
    assert!(text.contains(" 2 server "), "{text:?}");
    let inactive_number = cell(&frame, tabs[1].0.x + 1, HEADER_Y);
    assert_eq!(
        (inactive_number.fg, inactive_number.bg),
        (color(app.label), color(app.segment_bg))
    );
    assert_eq!(
        cell(&frame, tabs[1].0.x + 3, HEADER_Y).fg,
        color(app.text_secondary)
    );
    assert_eq!(
        state.hits.new_tab,
        Rect::new(tabs[1].0.right() + 2, HEADER_Y, 3, 1)
    );

    // Icon buttons end two columns in from the panel's right edge (column 176).
    assert_eq!(state.hits.header_more, Rect::new(171, HEADER_Y, 3, 1));
    assert_eq!(state.hits.header_zoom, Rect::new(167, HEADER_Y, 3, 1));
    assert_eq!(state.hits.header_split, Rect::new(163, HEADER_Y, 3, 1));
    assert_eq!(cell(&frame, 172, HEADER_Y).symbol, "⋯");
    assert_eq!(cell(&frame, 168, HEADER_Y).symbol, "⤢");
    assert_eq!(cell(&frame, 164, HEADER_Y).symbol, "◫");
}

#[test]
fn header_shows_even_for_a_single_tab() {
    let mut config = Config::default();
    config.ui.style = UiStyleConfig::App;
    config.ui.hide_tab_bar_when_single_tab = true;
    config.theme.name = Some("crews".to_string());
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    let frame = compose(&mut state);
    assert!(header_text(&frame).contains("client-shell"));
    assert_eq!(state.hits.tabs.len(), 1);
}

#[test]
fn overflowing_tabs_scroll_with_edge_arrows() {
    let mut tabs = (1..=40)
        .map(|number| tab(number, Some("a-long-tab-name"), false))
        .collect::<Vec<_>>();
    tabs[30].focused = true;
    let mut state = state_with_tabs(tabs);
    state.reveal_focused_tab = true;
    let frame = compose(&mut state);
    assert!(!state.hits.tab_scroll_left.is_empty());
    assert!(!state.hits.tab_scroll_right.is_empty());
    assert!(
        state.tab_scroll > 0,
        "revealing the focused tab scrolls the strip"
    );
    assert!(state.hits.tabs.iter().any(|(_, id)| id == "tab_31"));
    assert_eq!(
        cell(&frame, state.hits.tab_scroll_left.x, HEADER_Y).symbol,
        "‹"
    );
    assert_eq!(
        cell(&frame, state.hits.tab_scroll_right.x, HEADER_Y).symbol,
        "›"
    );
    let last_tab = state.hits.tabs.last().unwrap().0;
    assert!(
        last_tab.right() < state.hits.header_split.x,
        "tabs never reach the buttons"
    );

    let before = state.tab_scroll;
    let left = state.hits.tab_scroll_left;
    click(&mut state, left);
    assert_eq!(state.tab_scroll, before - 1, "‹ scrolls one tab left");
}

#[test]
fn header_buttons_run_the_existing_actions() {
    let mut state = state_with_tabs(vec![tab(1, None, true)]);
    state.set_endpoint_methods(None);
    compose(&mut state);

    let split = state.hits.header_split;
    let outcome = click(&mut state, split);
    assert!(
        !outcome.actions.is_empty() || !outcome.requests.is_empty(),
        "split sends a request"
    );

    compose(&mut state);
    let more = state.hits.header_more;
    click(&mut state, more);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::ContextMenu(_))
    ));
}

#[test]
fn reported_pane_size_leaves_two_columns_of_padding() {
    let state = state_with_tabs(vec![tab(1, None, true)]);
    let size = state.surface_size(COLS, ROWS);
    assert_eq!((size.cols, size.rows), (COLS - 36 - 1 - 4, ROWS - 6));
}
