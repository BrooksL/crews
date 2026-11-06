//! App-style sidebar (`ui.style = "app"`): layout, cards, click targets,
//! scrolling, and the section chips.

use super::*;
use crate::config::{AgentPanelSortConfig, UiStyleConfig};

const COLS: u16 = 177;
const ROWS: u16 = 48;

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

fn workspace(id: &str, label: &str, focused: bool, status: AgentStatus) -> ClientShellWorkspace {
    let mut workspace = snapshot().workspaces.remove(0);
    workspace.workspace_id = id.into();
    workspace.label = label.into();
    workspace.branch = None;
    workspace.focused = focused;
    workspace.agent_status = status;
    workspace
}

fn tab(id: &str, workspace_id: &str, label: &str) -> ClientShellTab {
    let mut tab = snapshot().tabs.remove(0);
    tab.tab_id = id.into();
    tab.workspace_id = workspace_id.into();
    tab.label = label.into();
    tab.focused = false;
    tab
}

fn agent(
    pane_id: &str,
    workspace_id: &str,
    tab_id: &str,
    name: &str,
    status: AgentStatus,
) -> ClientShellAgent {
    ClientShellAgent {
        pane_id: pane_id.into(),
        workspace_id: workspace_id.into(),
        tab_id: tab_id.into(),
        name: Some(name.into()),
        display_agent: None,
        agent: Some("claude".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: status,
        state_change_seq: 0,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: false,
    }
}

/// Two spaces (the first focused) and three agents: two share a space.
fn sidebar_snapshot() -> ClientShellSnapshot {
    let mut snapshot = snapshot();
    snapshot.workspaces = vec![
        workspace("ws_1", "check-ins", true, AgentStatus::Blocked),
        workspace("ws_2", "churchcenter", false, AgentStatus::Idle),
    ];
    snapshot.tabs = vec![tab("tab_1", "ws_1", "1"), tab("tab_2", "ws_2", "1")];
    snapshot.agents = vec![
        agent("pane_1", "ws_1", "tab_1", "lead", AgentStatus::Blocked),
        agent("pane_2", "ws_1", "tab_1", "helper", AgentStatus::Working),
        agent("pane_3", "ws_2", "tab_2", "solo", AgentStatus::Idle),
    ];
    snapshot
}

fn composed(state: &mut ClientShellState) -> FrameData {
    state.set_pane_surface(surface());
    state.compose(COLS, ROWS).expect("compose").frame
}

fn app_state(snapshot: ClientShellSnapshot) -> ClientShellState {
    let mut state = ClientShellState::new(app_config());
    state.set_snapshot(Box::new(snapshot));
    state
}

fn cell(frame: &FrameData, x: u16, y: u16) -> &crate::protocol::CellData {
    &frame.cells[usize::from(y) * usize::from(frame.width) + usize::from(x)]
}

fn row_text(frame: &FrameData, y: u16) -> String {
    frame_rows(frame)[usize::from(y)].clone()
}

#[test]
fn layout_follows_the_spec_rows_for_every_visibility_combination() {
    let area = Rect::new(0, 0, 36, ROWS - 1);
    let layout = |spaces, agents| {
        crate::client::shell::app_sidebar::app_sidebar_layout(area, None, spaces, agents).unwrap()
    };

    let both = layout(true, true);
    assert_eq!(both.content, Rect::new(1, 0, 34, ROWS - 1));
    assert_eq!(both.chips_y, 1);
    assert_eq!(both.collapse, Rect::new(32, 1, 3, 1));
    assert_eq!(both.footer_y, 45);
    let spaces = both.spaces.unwrap();
    let agents = both.agents.unwrap();
    assert_eq!(
        (spaces.heading_y, spaces.list),
        (3, Rect::new(1, 4, 34, 21))
    );
    assert_eq!(
        (agents.heading_y, agents.list),
        (25, Rect::new(1, 26, 34, 18))
    );

    let spaces_only = layout(true, false);
    assert_eq!(spaces_only.spaces.unwrap().list, Rect::new(1, 4, 34, 40));
    assert!(spaces_only.agents.is_none());

    let agents_only = layout(false, true);
    assert_eq!(agents_only.agents.unwrap().heading_y, 3);
    assert_eq!(agents_only.agents.unwrap().list, Rect::new(1, 4, 34, 40));
    assert!(agents_only.spaces.is_none());

    let neither = layout(false, false);
    assert!(neither.spaces.is_none() && neither.agents.is_none());
}

#[test]
fn a_dragged_split_replaces_the_default_share() {
    let area = Rect::new(0, 0, 36, ROWS - 1);
    let layout =
        crate::client::shell::app_sidebar::app_sidebar_layout(area, Some(0.25), true, true)
            .unwrap();
    assert_eq!(layout.agents.unwrap().heading_y, 3 + 10);
}

#[test]
fn both_sections_hidden_shows_a_label() {
    let mut state = app_state(sidebar_snapshot());
    state.sidebar_spaces_visible = false;
    state.sidebar_agents_visible = false;
    let frame = composed(&mut state);
    assert!(row_text(&frame, 3).contains("all panes hidden"));
    assert!(state.hits.workspaces.is_empty());
    assert!(state.hits.agents.is_empty());
}

#[test]
fn space_cards_have_caps_fills_and_columns_from_the_spec() {
    let mut state = app_state(sidebar_snapshot());
    let frame = composed(&mut state);
    let app = app();

    // Focused card: content row 5 with caps on rows 4 and 6.
    assert_eq!(cell(&frame, 5, 4).symbol, "▄");
    assert_eq!(cell(&frame, 5, 4).fg, color(app.card_focus_bg));
    assert_eq!(cell(&frame, 5, 4).bg, color(app.chrome_bg));
    assert_eq!(cell(&frame, 3, 5).symbol, "×", "status symbol at column 3");
    assert_eq!(cell(&frame, 5, 5).symbol, "c", "name at column 5");
    assert_eq!(cell(&frame, 5, 5).bg, color(app.card_focus_bg));
    assert_eq!(cell(&frame, 5, 5).fg, color(app.text_strong));
    assert_eq!(cell(&frame, 5, 6).symbol, "▀");

    // The next card starts right after: caps on rows 7 and 9.
    assert_eq!(cell(&frame, 5, 7).symbol, "▄");
    assert_eq!(cell(&frame, 5, 8).bg, color(app.card_bg));
    assert_eq!(cell(&frame, 3, 8).symbol, "○");
}

#[test]
fn space_click_targets_include_the_cap_rows() {
    let mut state = app_state(sidebar_snapshot());
    composed(&mut state);
    let rects = state
        .hits
        .workspaces
        .iter()
        .map(|hit| (hit.workspace_id.as_str(), hit.rect))
        .collect::<Vec<_>>();
    assert_eq!(
        rects,
        vec![
            ("ws_1", Rect::new(1, 4, 34, 3)),
            ("ws_2", Rect::new(1, 7, 34, 3)),
        ]
    );
}

#[test]
fn space_list_scrolls_by_whole_cards_without_a_scrollbar() {
    let mut snapshot = sidebar_snapshot();
    snapshot.workspaces = (0..12)
        .map(|index| {
            workspace(
                &format!("ws_{index}"),
                &format!("space {index}"),
                index == 0,
                AgentStatus::Idle,
            )
        })
        .collect();
    let mut state = app_state(snapshot);
    composed(&mut state);
    let metrics = state.hits.workspace_scroll_metrics.unwrap();
    // 12 cards of 3 rows in a 21-row list: 7 fit, so the list can scroll 5 cards.
    assert_eq!(state.hits.workspace_max_scroll, 5);
    assert_eq!(metrics.viewport_rows, 7, "list metrics count whole cards");
    assert_eq!(state.hits.workspaces.len(), 7);
    assert!(state.hits.workspace_scrollbar.is_empty());

    state.workspace_scroll = 2;
    composed(&mut state);
    assert_eq!(state.hits.workspaces[0].workspace_id, "ws_2");
    assert_eq!(state.hits.workspaces[0].rect.y, 4);
}

#[test]
fn grouped_agent_cards_have_a_header_and_one_row_per_agent() {
    let mut state = app_state(sidebar_snapshot());
    let frame = composed(&mut state);
    let app = app();

    // First card: caps on 26 and 29, header on 27, agents on 28 and 29... the
    // two-agent card spans rows 26 (cap) to 30 (cap).
    assert_eq!(cell(&frame, 5, 26).symbol, "▄");
    assert!(row_text(&frame, 27).contains("check-ins"));
    let lead = row_text(&frame, 28);
    assert!(
        lead.contains("× lead") && lead.contains("needs you"),
        "{lead:?}"
    );
    assert!(row_text(&frame, 29).contains("working"));
    assert_eq!(cell(&frame, 5, 30).symbol, "▀");
    // Agent rows are indented two columns past the header.
    assert_eq!(cell(&frame, 5, 28).symbol, "×");
    assert_eq!(cell(&frame, 5, 28).bg, color(app.card_bg));

    let solo = row_text(&frame, 33);
    assert!(solo.contains("○ solo") && solo.contains("idle"), "{solo:?}");

    let hits = state
        .hits
        .agents
        .iter()
        .map(|(rect, pane)| (pane.as_str(), *rect))
        .collect::<Vec<_>>();
    assert_eq!(
        hits[..3],
        [
            ("pane_1", Rect::new(1, 26, 34, 2)),
            ("pane_1", Rect::new(1, 28, 34, 1)),
            ("pane_2", Rect::new(1, 29, 34, 2)),
        ]
    );
}

#[test]
fn ungrouped_agent_cards_show_the_space_name() {
    let mut config = app_config();
    config.agent_panel_sort = AgentPanelSortConfig::Priority;
    let mut state = ClientShellState::new(config);
    state.set_snapshot(Box::new(sidebar_snapshot()));
    let frame = composed(&mut state);
    let first = row_text(&frame, 27);
    assert!(first.contains("× lead check-ins"), "{first:?}");
    assert!(row_text(&frame, 25).contains("priority"));
    assert_eq!(
        state.hits.agents[0],
        (Rect::new(1, 26, 34, 3), "pane_1".to_string())
    );
}

#[test]
fn chips_toggle_sections_and_survive_a_restart() {
    let dir = std::env::temp_dir().join(format!(
        "herdr-app-sidebar-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = dir.join("prefs.json");
    let mut state = ClientShellState::new(app_config().with_preferences_path(path.clone()));
    state.set_snapshot(Box::new(sidebar_snapshot()));
    composed(&mut state);
    let chip = state.hits.sidebar_agents_chip;
    assert_eq!(chip, Rect::new(10, 1, 8, 1));

    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: chip.x + 2,
        row: chip.y,
        modifiers: KeyModifiers::NONE,
    };
    state.handle_raw_events(vec![RawInputEvent::Mouse(click)]);
    assert!(!state.sidebar_agents_visible);
    let frame = composed(&mut state);
    assert!(state.hits.agents.is_empty());
    assert!(state.hits.agent_body.is_empty() || !row_text(&frame, 25).contains("agents ─"));

    let restored = ClientShellState::new(app_config().with_preferences_path(path));
    assert!(
        !restored.sidebar_agents_visible,
        "hidden agents section is remembered"
    );
    assert!(restored.sidebar_spaces_visible);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn toggle_actions_flip_each_section() {
    let mut state = app_state(sidebar_snapshot());
    let mut outcome = ClientShellInput::default();
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::ToggleSidebarSpaces),
        &mut outcome,
    );
    assert!(!state.sidebar_spaces_visible);
    assert!(outcome.repaint);
    state.record_binding(
        crate::input::KeybindMatch::Action(crate::input::KeybindAction::ToggleSidebarAgents),
        &mut outcome,
    );
    assert!(!state.sidebar_agents_visible);
}

#[test]
fn chips_headings_and_pills_use_the_spec_colors() {
    let mut state = app_state(sidebar_snapshot());
    state.sidebar_agents_visible = false;
    let frame = composed(&mut state);
    let app = app();
    let palette = crate::app::state::Palette::crews();
    let on = cell(&frame, 2, 1);
    assert_eq!(
        (on.symbol.as_str(), on.bg, on.fg),
        ("s", color(palette.accent), color(app.accent_fg))
    );
    let off = cell(&frame, 11, 1);
    assert_eq!(
        (off.symbol.as_str(), off.bg, off.fg),
        ("a", color(app.card_bg), color(app.text_secondary))
    );
    assert_eq!(cell(&frame, 1, 3).fg, color(app.label));
    assert_eq!(cell(&frame, 20, 3).symbol, "─");
    assert_eq!(cell(&frame, 20, 3).fg, color(app.rule));
    assert!(row_text(&frame, 45).contains("+ new space"));
    assert!(row_text(&frame, 45).contains("≡ menu"));
    assert_eq!(state.hits.new_workspace, Rect::new(1, 45, 13, 1));
    assert_eq!(state.hits.global_launcher.right(), 35);
}

#[test]
fn classic_style_keeps_the_classic_sidebar() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(sidebar_snapshot()));
    let frame = composed(&mut state);
    assert!(row_text(&frame, 0).starts_with(" spaces"));
    assert!(state.hits.sidebar_spaces_chip.is_empty());
}

#[test]
fn child_spaces_are_indented_cards_without_tree_lines() {
    let mut snapshot = sidebar_snapshot();
    let worktree = |linked| ClientShellWorktree {
        key: "repo".into(),
        label: "repo".into(),
        is_linked_worktree: linked,
    };
    snapshot.workspaces[0].worktree = Some(worktree(false));
    snapshot.workspaces[1].worktree = Some(worktree(true));
    snapshot.workspaces[1].branch = Some("worktree/fix-login".into());
    let mut state = app_state(snapshot);
    let frame = composed(&mut state);

    let child = &state.hits.workspaces[1];
    assert!(child.indented);
    assert_eq!(
        child.rect,
        Rect::new(3, 7, 32, 3),
        "child card is indented two columns"
    );
    let text = row_text(&frame, 8);
    assert!(text.contains("fix-login"), "{text:?}");
    assert!(!text.contains('├') && !text.contains('└'), "{text:?}");
    let (toggle, key) = state.hits.workspaces[0].group_toggle.clone().unwrap();
    assert_eq!(key, "repo");
    assert_eq!((toggle.x, toggle.y), (32, 5));
    assert_eq!(cell(&frame, 32, 5).symbol, "▾");
}

#[test]
fn a_space_without_an_agent_still_shows_a_status_symbol() {
    let mut snapshot = sidebar_snapshot();
    snapshot.workspaces[1].agent_status = AgentStatus::Unknown;
    snapshot.agents.retain(|agent| agent.workspace_id != "ws_2");
    let mut state = app_state(snapshot);
    let frame = composed(&mut state);
    assert_eq!(cell(&frame, 3, 8).symbol, "○");
    assert_eq!(cell(&frame, 3, 8).fg, color(app().label));
}

/// Whole-frame compose cost, classic vs app style, with 1 and 15 spaces and agents.
#[test]
#[ignore = "profile; run with just bench-render-scale"]
fn app_sidebar_render_scale_profile() {
    for count in [1usize, 15] {
        let mut snapshot = sidebar_snapshot();
        snapshot.workspaces = (0..count)
            .map(|index| {
                workspace(
                    &format!("ws_{index}"),
                    &format!("space {index}"),
                    index == 0,
                    AgentStatus::Working,
                )
            })
            .collect();
        snapshot.tabs = (0..count)
            .map(|index| tab(&format!("tab_{index}"), &format!("ws_{index}"), "1"))
            .collect();
        snapshot.agents = (0..count)
            .map(|index| {
                agent(
                    &format!("pane_{index}"),
                    &format!("ws_{index}"),
                    &format!("tab_{index}"),
                    "lead",
                    AgentStatus::Working,
                )
            })
            .collect();
        for (style, config) in [
            (
                "classic",
                ClientShellConfig::from_config(&Config::default()),
            ),
            ("app", app_config()),
            ("app-rounded", app_config()),
        ] {
            let mut state = ClientShellState::new(config);
            if style == "app-rounded" {
                state.set_graphics_cell_size(19, 42);
                state.set_host_shows_kitty_images(true);
            }
            state.set_snapshot(Box::new(snapshot.clone()));
            state.set_pane_surface(surface());
            state.compose(COLS, ROWS).unwrap();
            let mut samples = Vec::with_capacity(200);
            for _ in 0..200 {
                let started = std::time::Instant::now();
                std::hint::black_box(state.compose(COLS, ROWS).unwrap());
                samples.push(started.elapsed().as_micros());
            }
            samples.sort_unstable();
            eprintln!(
                "sidebar style={style} spaces={count} median_us={} p95_us={}",
                samples[100], samples[190]
            );
        }
    }
}
