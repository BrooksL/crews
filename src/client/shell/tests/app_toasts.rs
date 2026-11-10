//! App-style toasts (`ui.style = "app"`): stacked cards, open / dismiss
//! buttons, and `ui.toast.herdr.keep_needs_attention`.

use super::*;
use crate::client::shell::notification_policy::COMPLETION_RECHECK_INTERVAL;
use crate::config::UiStyleConfig;
use std::time::Duration;

const COLS: u16 = 177;
const ROWS: u16 = 48;
/// The panel is 36..176; a 44-wide card ends 2 columns in from its right edge.
const CARD_X: u16 = 176 - 2 - 44;

fn config(style: UiStyleConfig, keep_needs_attention: bool) -> ClientShellConfig {
    let mut config = Config::default();
    config.ui.style = style;
    config.theme.name = Some("crews".to_string());
    config.ui.toast.delivery = crate::config::ToastDelivery::Herdr;
    config.ui.toast.herdr.keep_needs_attention = keep_needs_attention;
    let mut config = ClientShellConfig::from_config(&config);
    // Deliver immediately so tests can tick from a single `now`.
    config.toast_delay_seconds = 0;
    config
}

fn state(style: UiStyleConfig, keep_needs_attention: bool) -> ClientShellState {
    let mut state = ClientShellState::new(config(style, keep_needs_attention));
    let mut snapshot = snapshot();
    // Finished toasts are validated against the client-projected agent state,
    // which reads Done after a Working agent goes Idle (see `finish`).
    snapshot.agents.push(ClientShellAgent {
        pane_id: "done".into(),
        workspace_id: "ws_2".into(),
        tab_id: "tab_2".into(),
        name: None,
        display_agent: Some("claude".into()),
        agent: Some("claude".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Working,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: false,
    });
    state.set_snapshot(Box::new(snapshot));
    state.set_pane_surface(surface());
    state
}

fn app() -> crate::app::app_tokens::AppTokens {
    crate::app::state::Palette::crews().app.unwrap()
}

fn color(color: ratatui::style::Color) -> u32 {
    crate::protocol::color_to_u32(color)
}

fn event(kind: SemanticNotificationKind, title: &str, pane: Option<&str>) -> SemanticNotification {
    SemanticNotification {
        kind,
        title: title.into(),
        body: Some(format!("{title} body")),
        sound: None,
        agent: None,
        workspace_id: Some("ws_2".into()),
        // Not the focused tab, so the toast is shown.
        tab_id: Some("tab_2".into()),
        pane_id: pane.map(Into::into),
        position: None,
    }
}

fn notify(state: &mut ClientShellState, title: &str, now: std::time::Instant) {
    state.receive_notification(
        &ClientEndpointId::Local,
        event(SemanticNotificationKind::Custom, title, Some(title)),
        now,
    );
}

/// Deliver a Finished toast for the `done` pane: the agent stops working, and
/// the toast is shown once the projected state reads Done.
fn finish(state: &mut ClientShellState, now: std::time::Instant) {
    state.receive_notification(
        &ClientEndpointId::Local,
        event(SemanticNotificationKind::Finished, "finished", Some("done")),
        now,
    );
    let mut snapshot = state.snapshot.as_deref().expect("snapshot").clone();
    snapshot.revision += 1;
    snapshot.agents[0].agent_status = AgentStatus::Idle;
    snapshot.agents[0].state_change_seq = 2;
    let revision = snapshot.revision;
    state.set_snapshot(Box::new(snapshot));
    let mut frame = surface();
    frame.projection_revision = revision;
    state.set_pane_surface(frame);
    state.tick_notifications(now + COMPLETION_RECHECK_INTERVAL);
}

fn visible_titles(state: &ClientShellState) -> Vec<&str> {
    state
        .visible_notifications
        .iter()
        .map(|notification| notification.event.title.as_str())
        .collect()
}

fn click(state: &mut ClientShellState, (column, row): (u16, u16)) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })])
}

fn focuses_pane(outcome: &ClientShellInput, pane: &str) -> bool {
    outcome.actions.iter().any(|action| {
        matches!(
            action,
            ClientShellAction::Endpoint { request, .. }
                if matches!(
                    &request.method,
                    crate::api::schema::Method::PaneFocus(params) if params.pane_id == pane
                )
        )
    })
}

fn cell(frame: &FrameData, x: u16, y: u16) -> &crate::protocol::CellData {
    &frame.cells[usize::from(y) * usize::from(frame.width) + usize::from(x)]
}

fn row_text(frame: &FrameData, y: u16) -> String {
    (0..frame.width)
        .map(|x| cell(frame, x, y).symbol.as_str())
        .collect()
}

#[test]
fn cards_stack_down_from_the_panel_top_right_with_gaps() {
    let mut state = state(UiStyleConfig::App, false);
    let now = std::time::Instant::now();
    for title in ["a", "b", "c"] {
        notify(&mut state, title, now);
    }
    state.compose(COLS, ROWS).expect("compose");
    let panel = state.layout(COLS, ROWS).panel;
    assert_eq!(CARD_X, panel.right() - 2 - 44);
    let cards = state
        .hits
        .notification_cards
        .iter()
        .map(|card| (card.index, card.card))
        .collect::<Vec<_>>();
    // Top cap on row 2, five rows each (cap, three content rows, cap), one-row gaps.
    assert_eq!(
        cards,
        vec![
            (0, Rect::new(CARD_X, 2, 44, 5)),
            (1, Rect::new(CARD_X, 8, 44, 5)),
            (2, Rect::new(CARD_X, 14, 44, 5)),
        ]
    );
    // The union covers the stack, for the copy feedback offset.
    assert_eq!(state.hits.notification_toast, Rect::new(CARD_X, 2, 44, 17));
}

#[test]
fn footprints_clamp_to_the_panel_and_drop_cards_that_dont_fit() {
    use crate::client::shell::app_toasts::toast_footprints;
    // Narrow panel: the card shrinks to keep two columns each side.
    assert_eq!(
        toast_footprints(Rect::new(10, 0, 30, 40), 0, 1),
        vec![Rect::new(12, 2, 26, 5)]
    );
    // Room for one card above the panel's bottom cap (row 11), not two.
    assert_eq!(
        toast_footprints(Rect::new(0, 0, 60, 12), 0, 3),
        vec![Rect::new(14, 2, 44, 5)]
    );
    // A config diagnostic or lifecycle banner pushes the stack down.
    assert_eq!(toast_footprints(Rect::new(0, 0, 60, 40), 1, 1)[0].y, 3);
    assert!(toast_footprints(Rect::new(0, 0, 10, 40), 0, 1).is_empty());
}

#[test]
fn at_most_three_are_visible_and_the_fourth_is_promoted_on_dismiss() {
    let mut state = state(UiStyleConfig::App, false);
    let now = std::time::Instant::now();
    for title in ["a", "b", "c", "d"] {
        notify(&mut state, title, now);
    }
    assert_eq!(visible_titles(&state), ["a", "b", "c"]);
    assert_eq!(state.queued_notifications.len(), 1);

    state.compose(COLS, ROWS).expect("compose");
    let dismiss = state.hits.notification_cards[1].dismiss;
    let outcome = click(&mut state, (dismiss.x + 1, dismiss.y));
    assert!(outcome.repaint);
    assert!(outcome.actions.is_empty());
    assert_eq!(visible_titles(&state), ["a", "c", "d"]);
    assert!(state.queued_notifications.is_empty());
}

#[test]
fn clicking_open_focuses_that_toast_and_the_card_body_does_nothing() {
    let mut state = state(UiStyleConfig::App, false);
    let now = std::time::Instant::now();
    for title in ["a", "b"] {
        notify(&mut state, title, now);
    }
    state.compose(COLS, ROWS).expect("compose");

    // The body row is inert.
    let card = state.hits.notification_cards[1].card;
    let outcome = click(&mut state, (card.x + 10, card.y + 2));
    assert!(outcome.actions.is_empty());
    assert_eq!(visible_titles(&state), ["a", "b"]);

    let open = state.hits.notification_cards[1].open;
    assert!(!open.is_empty());
    let outcome = click(&mut state, (open.x, open.y));
    assert!(focuses_pane(&outcome, "b"));
    assert_eq!(visible_titles(&state), ["a"]);
}

#[test]
fn clicking_dismiss_removes_the_toast() {
    let mut state = state(UiStyleConfig::App, false);
    state.receive_notification(
        &ClientEndpointId::Local,
        event(SemanticNotificationKind::Custom, "no pane", None),
        std::time::Instant::now(),
    );
    state.compose(COLS, ROWS).expect("compose");
    let hit = state.hits.notification_cards[0];
    // Without a pane there is no open pill, and dismiss lines up with the body.
    assert!(hit.open.is_empty());
    assert_eq!(hit.dismiss.x + 1, hit.card.x + 4);
    let outcome = click(&mut state, (hit.dismiss.x + 1, hit.dismiss.y));
    assert!(outcome.actions.is_empty());
    assert!(state.visible_notifications.is_empty());
}

#[test]
fn open_notification_target_acts_on_the_oldest_toast() {
    let mut state = state(UiStyleConfig::App, false);
    let now = std::time::Instant::now();
    for title in ["a", "b"] {
        notify(&mut state, title, now);
    }
    let mut outcome = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::OpenNotificationTarget),
        &mut outcome,
    );
    assert!(focuses_pane(&outcome, "a"));
    assert_eq!(visible_titles(&state), ["b"]);
}

#[test]
fn needs_attention_expires_after_eight_seconds_by_default() {
    let mut state = state(UiStyleConfig::App, false);
    let now = std::time::Instant::now();
    state.receive_notification(
        &ClientEndpointId::Local,
        event(
            SemanticNotificationKind::NeedsAttention,
            "blocked",
            Some("p"),
        ),
        now,
    );
    assert_eq!(visible_titles(&state), ["blocked"]);
    state.tick_notifications(now + Duration::from_millis(7_900));
    assert_eq!(visible_titles(&state), ["blocked"]);
    let (_, repaint) = state.tick_notifications(now + Duration::from_secs(8));
    assert!(repaint);
    assert!(state.visible_notifications.is_empty());
}

#[test]
fn keep_needs_attention_holds_needs_attention_but_not_finished() {
    for style in [UiStyleConfig::App, UiStyleConfig::Classic] {
        let mut state = state(style, true);
        assert!(state.config.toast_keep_needs_attention);
        let now = std::time::Instant::now();
        state.receive_notification(
            &ClientEndpointId::Local,
            event(
                SemanticNotificationKind::NeedsAttention,
                "blocked",
                Some("p"),
            ),
            now,
        );
        state.tick_notifications(now + Duration::from_secs(3_600));
        assert_eq!(visible_titles(&state), ["blocked"], "{style:?}");

        // A newer event for the same pane still replaces it.
        state.receive_notification(
            &ClientEndpointId::Local,
            event(SemanticNotificationKind::Custom, "update", Some("p")),
            now,
        );
        assert_eq!(visible_titles(&state), ["update"], "{style:?}");
        state.tick_notifications(now + Duration::from_secs(5));
        assert!(state.visible_notifications.is_empty(), "{style:?}");
    }

    let mut state = state(UiStyleConfig::App, true);
    let now = std::time::Instant::now();
    state.receive_notification(
        &ClientEndpointId::Local,
        event(
            SemanticNotificationKind::NeedsAttention,
            "blocked",
            Some("p"),
        ),
        now,
    );
    finish(&mut state, now);
    assert_eq!(visible_titles(&state), ["blocked", "finished"]);
    state.tick_notifications(now + COMPLETION_RECHECK_INTERVAL + Duration::from_secs(5));
    assert_eq!(visible_titles(&state), ["blocked"]);
    state.tick_notifications(now + Duration::from_secs(3_600));
    assert_eq!(visible_titles(&state), ["blocked"]);

    // Dismiss still closes it.
    state.compose(COLS, ROWS).expect("compose");
    let dismiss = state.hits.notification_cards[0].dismiss;
    click(&mut state, (dismiss.x + 1, dismiss.y));
    assert!(state.visible_notifications.is_empty());
}

#[test]
fn classic_still_shows_a_single_toast() {
    let mut state = state(UiStyleConfig::Classic, false);
    let now = std::time::Instant::now();
    for title in ["a", "b", "c"] {
        notify(&mut state, title, now);
    }
    assert_eq!(visible_titles(&state), ["a"]);
    assert_eq!(state.queued_notifications.len(), 2);
    state.compose(COLS, ROWS).expect("compose");
    assert!(state.hits.notification_cards.is_empty());
    assert!(!state.hits.notification_toast.is_empty());

    state.tick_notifications(now + Duration::from_secs(5));
    assert_eq!(visible_titles(&state), ["b"]);
}

#[test]
fn switching_to_classic_returns_extra_toasts_to_the_queue() {
    let mut state = state(UiStyleConfig::App, false);
    let now = std::time::Instant::now();
    for title in ["a", "b", "c", "d"] {
        notify(&mut state, title, now);
    }
    state.config.style = UiStyleConfig::Classic;
    state.tick_notifications(now);
    assert_eq!(visible_titles(&state), ["a"]);
    let queued = state
        .queued_notifications
        .iter()
        .map(|notification| notification.event.title.as_str())
        .collect::<Vec<_>>();
    assert_eq!(queued, ["b", "c", "d"]);
}

#[test]
fn card_colors_come_from_tokens() {
    let mut state = state(UiStyleConfig::App, false);
    state.receive_notification(
        &ClientEndpointId::Local,
        event(
            SemanticNotificationKind::NeedsAttention,
            "lead needs you",
            Some("p"),
        ),
        std::time::Instant::now(),
    );
    let frame = state.compose(COLS, ROWS).expect("compose").frame;
    let app = app();
    let palette = crate::app::state::Palette::crews();
    let hit = state.hits.notification_cards[0];
    let (x, top) = (hit.card.x, hit.card.y);

    // Square mode: caps in float_bg over the pane background.
    let cap = cell(&frame, x, top);
    assert_eq!(cap.symbol, "▄");
    assert_eq!(cap.fg, color(app.float_bg));
    assert_eq!(cap.bg, color(app.pane_bg));
    assert_eq!(cell(&frame, x, top + 4).symbol, "▀");

    // Title row: blocked symbol, then a bold text_strong title.
    let title_y = top + 1;
    assert_eq!(cell(&frame, x, title_y).bg, color(app.float_bg));
    let symbol = cell(&frame, x + 2, title_y);
    assert_eq!(symbol.symbol, "×");
    assert_eq!(symbol.fg, color(palette.red));
    let title = cell(&frame, x + 4, title_y);
    assert_eq!(title.symbol, "l");
    assert_eq!(title.fg, color(app.text_strong));
    assert_ne!(title.modifier & Modifier::BOLD.bits(), 0);
    assert!(row_text(&frame, title_y).contains("× lead needs you"));

    // Body, indented past the symbol.
    let body = cell(&frame, x + 4, top + 2);
    assert_eq!(body.symbol, "l");
    assert_eq!(body.fg, color(app.toast_body));
    assert!(row_text(&frame, top + 2).contains("lead needs you body"));

    // Actions: an accent `open` pill and a text-only `dismiss`.
    let open = hit.open;
    assert_eq!(open, Rect::new(x + 4, top + 3, 6, 1));
    let label = cell(&frame, open.x + 1, open.y);
    assert_eq!(label.symbol, "o");
    assert_eq!(label.bg, color(palette.accent));
    assert_eq!(label.fg, color(app.accent_fg));
    assert_ne!(label.modifier & Modifier::BOLD.bits(), 0);
    let dismiss = cell(&frame, hit.dismiss.x + 1, hit.dismiss.y);
    assert_eq!(dismiss.symbol, "d");
    assert_eq!(dismiss.fg, color(app.text_secondary));
    assert_eq!(dismiss.bg, color(app.float_bg));
    assert!(row_text(&frame, top + 3).contains(" open   dismiss"));
}

#[test]
fn rounded_toasts_keep_their_corners_and_shadow_over_their_own_occlusion() {
    let mut state = state(UiStyleConfig::App, false);
    state.set_graphics_cell_size(10, 21);
    state.set_host_shows_kitty_images(true);
    notify(&mut state, "a", std::time::Instant::now());
    state.set_pane_surface(surface());
    let composed = state.compose(COLS, ROWS).expect("compose");
    let graphics = String::from_utf8_lossy(&composed.graphics.into_inline_bytes()).into_owned();
    let card = state.hits.notification_cards[0].card;
    // Top-left corner cell of the card (1-based cursor position).
    assert!(
        graphics.contains(&format!("\x1b[{};{}H\x1b_Ga=p,", card.y + 1, card.x + 1)),
        "toast corner placed"
    );
    // The shadow starts three columns left and two rows above the card.
    assert!(
        graphics.contains(&format!("\x1b[{};{}H\x1b_Ga=p,", card.y - 1, card.x - 2)),
        "toast shadow placed"
    );
}

#[test]
fn keep_needs_attention_threads_into_the_shell_config() {
    assert!(!ClientShellConfig::from_config(&Config::default()).toast_keep_needs_attention);
    let parsed: Config =
        toml::from_str("[ui.toast.herdr]\nkeep_needs_attention = true").expect("parse");
    assert!(ClientShellConfig::from_config(&parsed).toast_keep_needs_attention);
}
