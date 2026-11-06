//! The polished sidebar (`ui.style = "app"`): section chips, headings with
//! rules, space and agent cards, and pill buttons, all on the chrome color.
//!
//! It reuses the classic sidebar's data (workspace grouping, configured row
//! tokens, agent ordering) and fills the same hit map, so clicks, drags,
//! scrolling, and keyboard navigation behave as they do in classic.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use super::render::sidebar::{
    displayed_workspace_status, workspace_entries, workspace_rows, workspace_selection_background,
};
use super::render::{display_width, put_segment, put_text, ShellRenderState};
use super::shape::{self, Corners, Fill};
use super::*;
use crate::api::schema::AgentStatus;
use crate::app::app_tokens::AppTokens;
use crate::config::StatusIndicatorStyle;

/// Spaces' share of the section rows until the user drags the split.
const DEFAULT_SPACES_SHARE: f32 = 0.54;
/// Rows a card adds around its content: one cap above, one below.
const CARD_CAP_ROWS: u16 = 2;
/// Smallest tab name worth showing in an agent group header.
const MIN_TAB_LABEL_WIDTH: u16 = 8;

/// Where each part of the app-style sidebar goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct AppSidebarLayout {
    /// Columns cards span: one column in from the left, stopping before the gutter.
    pub content: Rect,
    pub chips_y: u16,
    /// The `«` collapse button, right-aligned on the chip row.
    pub collapse: Rect,
    /// All section rows, headings included.
    pub sections: Rect,
    pub spaces: Option<SidebarSectionLayout>,
    pub agents: Option<SidebarSectionLayout>,
    pub footer_y: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SidebarSectionLayout {
    pub heading_y: u16,
    pub list: Rect,
}

/// Lay out the sidebar `area` (the gutter is its last column). None when the
/// sidebar is too short for chips, two section rows, and the footer.
pub(super) fn app_sidebar_layout(
    area: Rect,
    split: Option<f32>,
    spaces_visible: bool,
    agents_visible: bool,
) -> Option<AppSidebarLayout> {
    let content = Rect::new(area.x + 1, area.y, area.width.checked_sub(2)?, area.height);
    let sections_height = area.height.checked_sub(6)?;
    if content.width < 12 || sections_height < 4 {
        return None;
    }
    let sections = Rect::new(content.x, area.y + 3, content.width, sections_height);
    let section = |y: u16, height: u16| SidebarSectionLayout {
        heading_y: y,
        list: Rect::new(content.x, y + 1, content.width, height.saturating_sub(1)),
    };
    let (spaces, agents) = match (spaces_visible, agents_visible) {
        (true, true) => {
            let share = split.unwrap_or(DEFAULT_SPACES_SHARE).clamp(0.1, 0.9);
            let spaces_rows =
                ((f32::from(sections_height) * share).floor() as u16).clamp(2, sections_height - 2);
            (
                Some(section(sections.y, spaces_rows)),
                Some(section(
                    sections.y + spaces_rows,
                    sections_height - spaces_rows,
                )),
            )
        }
        (true, false) => (Some(section(sections.y, sections_height)), None),
        (false, true) => (None, Some(section(sections.y, sections_height))),
        (false, false) => (None, None),
    };
    Some(AppSidebarLayout {
        content,
        chips_y: area.y + 1,
        collapse: Rect::new(content.right() - 3, area.y + 1, 3, 1),
        sections,
        spaces,
        agents,
        footer_y: area.bottom() - 2,
    })
}

/// Draw the app-style sidebar. Returns false (drawing nothing) when the area
/// is too small, so the caller can fall back to the classic sidebar.
pub(super) fn render_app_sidebar(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    state: &mut ShellRenderState<'_>,
    hits: &mut ShellHitMap,
    app: &AppTokens,
) -> bool {
    let split = state
        .sidebar_section_split_manual
        .then_some(state.sidebar_section_split);
    let Some(layout) = app_sidebar_layout(
        area,
        split,
        state.sidebar_spaces_visible,
        state.sidebar_agents_visible,
    ) else {
        return false;
    };
    let palette = &config.palette;
    shape::clear(buffer, area, app.chrome_bg);

    // Width drag on the gutter. Its rows match the section rows so dragging a
    // section split maps rows to the same ratio the layout uses.
    hits.sidebar_divider = Rect::new(
        area.right() - 1,
        layout.sections.y,
        1,
        layout.sections.height,
    );
    render_chips(buffer, &layout, state, palette.accent, app, hits);
    hits.sidebar_toggle = layout.collapse;
    put_text(
        buffer,
        layout.collapse.x + 1,
        layout.collapse.y,
        1,
        "«",
        Style::default().fg(app.text_secondary),
    );

    if layout.spaces.is_none() && layout.agents.is_none() {
        put_text(
            buffer,
            layout.content.x,
            layout.sections.y,
            layout.content.width,
            "all panes hidden",
            Style::default().fg(app.label),
        );
    }
    if let Some(section) = layout.spaces {
        heading(buffer, &layout, section.heading_y, "spaces", None, app);
        hits.workspace_body = section.list;
        render_space_cards(buffer, section.list, snapshot, config, state, hits, app);
    }
    if let Some(section) = layout.agents {
        let sort_label =
            snapshot
                .agent_view_label
                .as_deref()
                .unwrap_or(match config.agent_panel_sort {
                    crate::config::AgentPanelSortConfig::Spaces => "grouped",
                    crate::config::AgentPanelSortConfig::Priority => "priority",
                });
        let pill = heading(
            buffer,
            &layout,
            section.heading_y,
            "agents",
            Some(sort_label),
            app,
        );
        hits.agent_sort_toggle = if config.mouse_capture && snapshot.agent_view_label.is_none() {
            pill
        } else {
            Rect::default()
        };
        if layout.spaces.is_some() {
            // Dragging the agents heading moves the split between the sections.
            hits.sidebar_section_divider = Rect::new(
                layout.content.x,
                section.heading_y,
                pill.x.saturating_sub(layout.content.x + 1),
                1,
            );
        }
        hits.agent_body = section.list;
        render_agent_cards(
            buffer,
            section.list,
            snapshot,
            config,
            state.agent_scroll,
            hits,
            app,
        );
    }

    render_footer(buffer, &layout, snapshot, config, hits, app);
    true
}

fn render_chips(
    buffer: &mut Buffer,
    layout: &AppSidebarLayout,
    state: &ShellRenderState<'_>,
    accent: Color,
    app: &AppTokens,
    hits: &mut ShellHitMap,
) {
    let chip_style = |on: bool| {
        if on {
            Style::default()
                .fg(app.accent_fg)
                .bg(accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.text_secondary).bg(app.card_bg)
        }
    };
    let right = layout.collapse.x.saturating_sub(1);
    hits.sidebar_spaces_chip = shape::pill(
        buffer,
        layout.content.x,
        layout.chips_y,
        right,
        "spaces",
        chip_style(state.sidebar_spaces_visible),
        Corners::Square,
    );
    hits.sidebar_agents_chip = shape::pill(
        buffer,
        hits.sidebar_spaces_chip.right() + 1,
        layout.chips_y,
        right,
        "agents",
        chip_style(state.sidebar_agents_visible),
        Corners::Square,
    );
}

/// A bold section heading in `label`, then a rule out to the right edge, and
/// an optional pill at the end. Returns the pill's rect.
fn heading(
    buffer: &mut Buffer,
    layout: &AppSidebarLayout,
    y: u16,
    title: &str,
    pill: Option<&str>,
    app: &AppTokens,
) -> Rect {
    let content = layout.content;
    let after_title = put_segment(
        buffer,
        content.x,
        y,
        content.right(),
        title,
        Style::default().fg(app.label).add_modifier(Modifier::BOLD),
    );
    let pill_rect = pill.map_or(Rect::default(), |label| {
        let width = display_width(label) + 2;
        shape::pill(
            buffer,
            content.right().saturating_sub(width),
            y,
            content.right(),
            label,
            Style::default().fg(app.text_secondary).bg(app.card_bg),
            Corners::Square,
        )
    });
    let rule_end = if pill_rect.is_empty() {
        content.right()
    } else {
        pill_rect.x.saturating_sub(1)
    };
    let rule_start = after_title + 1;
    if rule_end > rule_start {
        let rule = "─".repeat(usize::from(rule_end - rule_start));
        put_text(
            buffer,
            rule_start,
            y,
            rule_end - rule_start,
            &rule,
            Style::default().fg(app.rule),
        );
    }
    pill_rect
}

/// Card fill for a space, following today's precedence: navigate selection,
/// then drag, then focus.
fn space_card_fill(
    palette: &Palette,
    app: &AppTokens,
    selected: bool,
    dragged: bool,
    focused: bool,
) -> Color {
    if selected {
        workspace_selection_background(palette)
    } else if dragged {
        palette.surface1
    } else if focused {
        app.card_focus_bg
    } else {
        app.card_bg
    }
}

/// Card heights (content rows plus caps) for every space entry.
fn space_card_heights(
    snapshot: &ClientShellSnapshot,
    entries: &[WorkspaceEntry],
    collapsed_groups: &HashSet<String>,
    config: &ClientShellConfig,
) -> Vec<u16> {
    entries
        .iter()
        .map(|entry| {
            let rows = snapshot.workspaces.get(entry.index).map_or(1, |workspace| {
                workspace_rows(
                    workspace,
                    displayed_workspace_status(snapshot, workspace, collapsed_groups),
                    entry.indented,
                    &config.spaces,
                )
                .len()
                .max(1)
            });
            (rows.min(usize::from(u16::MAX - CARD_CAP_ROWS)) as u16) + CARD_CAP_ROWS
        })
        .collect()
}

fn render_space_cards(
    buffer: &mut Buffer,
    list: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    state: &mut ShellRenderState<'_>,
    hits: &mut ShellHitMap,
    app: &AppTokens,
) {
    let palette = &config.palette;
    let entries = workspace_entries(snapshot, state.collapsed_groups);
    let heights = space_card_heights(snapshot, &entries, state.collapsed_groups, config);
    let gaps = vec![0; heights.len()];
    if !list.is_empty() && std::mem::take(state.reveal_focused_workspace) {
        if let Some(target) = entries
            .iter()
            .position(|entry| snapshot.workspaces[entry.index].focused)
        {
            *state.workspace_scroll = super::scroll::list_scroll_start_to_reveal(
                &heights,
                &gaps,
                list.height,
                *state.workspace_scroll,
                target,
            );
        }
    }
    let metrics =
        super::scroll::list_scroll_metrics(&heights, &gaps, list.height, *state.workspace_scroll);
    hits.workspace_max_scroll = metrics.max_offset_from_bottom;
    hits.workspace_scroll_metrics = Some(metrics);
    *state.workspace_scroll = metrics
        .max_offset_from_bottom
        .saturating_sub(metrics.offset_from_bottom);

    let mut y = list.y;
    for (position, entry) in entries.iter().enumerate().skip(*state.workspace_scroll) {
        let Some(workspace) = snapshot.workspaces.get(entry.index) else {
            continue;
        };
        let height = heights[position];
        if y.saturating_add(height) > list.bottom() {
            break;
        }
        let indent = if entry.indented { 2 } else { 0 };
        let content = Rect::new(
            list.x + indent,
            y + 1,
            list.width.saturating_sub(indent),
            height - CARD_CAP_ROWS,
        );
        let selected = state.selected_workspace_id.is_some_and(|target| {
            target.matches(state.active_endpoint_id, &workspace.workspace_id)
        });
        let dragged = state.dragged_workspace_id == Some(workspace.workspace_id.as_str());
        let fill = space_card_fill(palette, app, selected, dragged, workspace.focused);
        let footprint = shape::card(
            buffer,
            content,
            Fill::new(fill, app.chrome_bg),
            Corners::Square,
        );
        let status = displayed_workspace_status(snapshot, workspace, state.collapsed_groups);
        let rows = workspace_rows(workspace, status, entry.indented, &config.spaces);
        let group_toggle = render_group_toggle(
            buffer,
            content,
            snapshot,
            entry.index,
            state.collapsed_groups,
            palette.accent,
            fill,
        );
        let text_right = content
            .right()
            .saturating_sub(if group_toggle.is_some() { 4 } else { 2 });
        render_space_rows(
            buffer,
            content,
            text_right,
            &rows,
            status,
            workspace.focused,
            fill,
            palette,
            app,
        );
        hits.workspaces.push(WorkspaceHit {
            rect: footprint,
            endpoint_id: ClientEndpointId::Local,
            workspace_id: workspace.workspace_id.clone(),
            indented: entry.indented,
            group_toggle,
        });
        y = y.saturating_add(height);
    }

    if let Some(row) = state
        .workspace_drop_indicator_row
        .filter(|row| *row >= list.y && *row < list.bottom())
    {
        put_text(
            buffer,
            list.x,
            row,
            list.width,
            &"─".repeat(usize::from(list.width)),
            Style::default().fg(palette.accent),
        );
    }
}

/// The ▾/▸ toggle on a parent space with linked worktrees, two columns in
/// from the card's right edge.
fn render_group_toggle(
    buffer: &mut Buffer,
    content: Rect,
    snapshot: &ClientShellSnapshot,
    index: usize,
    collapsed_groups: &HashSet<String>,
    accent: Color,
    fill: Color,
) -> Option<(Rect, String)> {
    let key = super::render::sidebar::parent_group_key(snapshot, index)?;
    let toggle = Rect::new(content.right().checked_sub(3)?, content.y, 1, 1);
    let symbol = if collapsed_groups.contains(&key) {
        "▸"
    } else {
        "▾"
    };
    put_text(
        buffer,
        toggle.x,
        toggle.y,
        1,
        symbol,
        Style::default().fg(accent).bg(fill),
    );
    Some((toggle, key))
}

#[allow(clippy::too_many_arguments)]
fn render_space_rows(
    buffer: &mut Buffer,
    content: Rect,
    text_right: u16,
    rows: &[Vec<crate::ui::ResolvedToken>],
    status: AgentStatus,
    focused: bool,
    fill: Color,
    palette: &Palette,
    app: &AppTokens,
) {
    let name_style = if focused {
        Style::default()
            .fg(app.text_strong)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.text)
    };
    let secondary_style = Style::default().fg(if focused {
        palette.accent
    } else {
        app.text_muted
    });
    let (symbol, symbol_color) = app_status_symbol(status, palette, app);
    let status_style = Style::default().fg(symbol_color);
    let icon = (symbol, status_style);
    let icon_row;
    let rows = if rows.is_empty() {
        icon_row = vec![vec![crate::ui::ResolvedToken {
            kind: crate::ui::ResolvedTokenKind::StateIcon,
            style: Default::default(),
        }]];
        &icon_row[..]
    } else {
        rows
    };
    for (index, tokens) in rows.iter().take(usize::from(content.height)).enumerate() {
        let y = content.y + index as u16;
        // Row 0 starts with the status symbol at column +2, the name at +4.
        // Later rows line up with the name.
        let x = content.x + if index == 0 { 2 } else { 4 };
        let mut spans = Vec::new();
        let needs_icon = index == 0
            && !tokens
                .iter()
                .any(|token| matches!(token.kind, crate::ui::ResolvedTokenKind::StateIcon));
        if needs_icon {
            spans.push(Span::styled(icon.0, icon.1));
            spans.push(Span::raw(" "));
        }
        let width = text_right.saturating_sub(x);
        let used = spans.iter().map(|span| span.width()).sum::<usize>();
        spans.extend(crate::ui::resolved_token_spans(
            tokens,
            icon,
            status_style,
            name_style,
            secondary_style,
            secondary_style,
            palette,
            usize::from(width).saturating_sub(used),
        ));
        Paragraph::new(Line::from(spans))
            .style(Style::default().bg(fill))
            .render(Rect::new(x, y, width, 1), buffer);
    }
}

/// App style always shows one of the four status symbols. Panes with no
/// agent state read as idle, drawn in the dimmer label color.
pub(super) fn app_status_symbol(
    status: AgentStatus,
    palette: &Palette,
    app: &AppTokens,
) -> (&'static str, Color) {
    match status {
        AgentStatus::Unknown => (
            status_icon(AgentStatus::Idle, StatusIndicatorStyle::Symbols),
            app.label,
        ),
        status => (
            status_icon(status, StatusIndicatorStyle::Symbols),
            status_color(status, palette),
        ),
    }
}

/// One agent card's worth of rows.
struct AgentCard<'a> {
    workspace: &'a str,
    tab: Option<&'a str>,
    agents: Vec<&'a crate::protocol::ClientShellAgent>,
    grouped: bool,
}

impl AgentCard<'_> {
    fn content_rows(&self) -> u16 {
        let agents = self.agents.len().min(usize::from(u16::MAX - 4)) as u16;
        if self.grouped {
            1 + agents
        } else {
            agents.max(1)
        }
    }
}

fn agent_cards<'a>(
    snapshot: &'a ClientShellSnapshot,
    config: &ClientShellConfig,
) -> Vec<AgentCard<'a>> {
    let grouped = config.agent_panel_sort == crate::config::AgentPanelSortConfig::Spaces
        && snapshot.agent_view_label.is_none();
    let mut cards: Vec<AgentCard<'a>> = Vec::new();
    for pane_id in super::render::ordered_agent_pane_ids(snapshot, config.agent_panel_sort) {
        let Some(agent) = snapshot
            .agents
            .iter()
            .find(|agent| agent.pane_id == pane_id)
        else {
            continue;
        };
        let Some(workspace) = snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.workspace_id == agent.workspace_id)
        else {
            continue;
        };
        let tab_count = snapshot
            .tabs
            .iter()
            .filter(|tab| tab.workspace_id == agent.workspace_id)
            .count();
        let tab = snapshot
            .tabs
            .iter()
            .find(|tab| tab.tab_id == agent.tab_id)
            .filter(|tab| tab_count > 1 || tab.custom_label)
            .map(|tab| tab.label.as_str());
        if grouped {
            if let Some(card) = cards.iter_mut().find(|card| {
                card.agents.first().is_some_and(|first| {
                    first.workspace_id == agent.workspace_id
                        && (tab.is_none() || first.tab_id == agent.tab_id)
                })
            }) {
                card.agents.push(agent);
                continue;
            }
        }
        cards.push(AgentCard {
            workspace: &workspace.label,
            tab,
            agents: vec![agent],
            grouped,
        });
    }
    cards
}

fn agent_label(agent: &crate::protocol::ClientShellAgent) -> &str {
    agent
        .display_agent
        .as_deref()
        .or(agent.name.as_deref())
        .or(agent.agent.as_deref())
        .or(agent.title.as_deref())
        .unwrap_or("agent")
}

/// The state word on the right of an agent row, in its status color.
fn agent_state_word(
    status: AgentStatus,
    palette: &Palette,
    app: &AppTokens,
) -> (&'static str, Color) {
    match status {
        AgentStatus::Blocked => ("needs you", palette.red),
        AgentStatus::Working => ("working", palette.yellow),
        AgentStatus::Done => ("done", palette.teal),
        AgentStatus::Idle | AgentStatus::Unknown => ("idle", app.text_muted),
    }
}

fn render_agent_cards(
    buffer: &mut Buffer,
    list: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    agent_scroll: &mut usize,
    hits: &mut ShellHitMap,
    app: &AppTokens,
) {
    let palette = &config.palette;
    let cards = agent_cards(snapshot, config);
    if list.is_empty() || cards.is_empty() {
        *agent_scroll = 0;
        if snapshot.agent_view_label.is_some() && !list.is_empty() {
            put_text(
                buffer,
                list.x,
                list.y,
                list.width,
                "no matching agents",
                Style::default().fg(app.label),
            );
        }
        return;
    }
    let heights = cards
        .iter()
        .map(|card| card.content_rows() + CARD_CAP_ROWS)
        .collect::<Vec<_>>();
    let gaps = vec![0; heights.len()];
    let metrics = super::scroll::list_scroll_metrics(&heights, &gaps, list.height, *agent_scroll);
    hits.agent_max_scroll = metrics.max_offset_from_bottom;
    hits.agent_scroll_metrics = Some(metrics);
    *agent_scroll = metrics
        .max_offset_from_bottom
        .saturating_sub(metrics.offset_from_bottom);

    let mut y = list.y;
    for (index, card) in cards.iter().enumerate().skip(*agent_scroll) {
        let height = heights[index];
        if y.saturating_add(height) > list.bottom() {
            break;
        }
        let content = Rect::new(list.x, y + 1, list.width, height - CARD_CAP_ROWS);
        let fill = app.card_bg;
        shape::card(
            buffer,
            content,
            Fill::new(fill, app.chrome_bg),
            Corners::Square,
        );
        let text_right = content.right().saturating_sub(2);
        let mut row_y = content.y;
        if card.grouped {
            let x = put_segment(
                buffer,
                content.x + 2,
                row_y,
                text_right,
                card.workspace,
                Style::default()
                    .fg(palette.text)
                    .bg(fill)
                    .add_modifier(Modifier::BOLD),
            );
            if let Some(tab) = card.tab {
                let tab_x = x + 1;
                if text_right.saturating_sub(tab_x) >= MIN_TAB_LABEL_WIDTH {
                    put_segment(
                        buffer,
                        tab_x,
                        row_y,
                        text_right,
                        tab,
                        Style::default().fg(app.text_muted).bg(fill),
                    );
                }
            }
            // The header row (and the cap above it) focuses the group's first agent.
            if let Some(first) = card.agents.first() {
                hits.agents.push((
                    Rect::new(content.x, y, content.width, 2),
                    first.pane_id.clone(),
                ));
            }
            row_y += 1;
        }
        let last = card.agents.len().saturating_sub(1);
        for (agent_index, agent) in card.agents.iter().enumerate() {
            let indent = if card.grouped { 4 } else { 2 };
            render_agent_line(
                buffer,
                Rect::new(
                    content.x + indent,
                    row_y,
                    text_right.saturating_sub(content.x + indent),
                    1,
                ),
                agent,
                (!card.grouped).then_some(card.workspace),
                fill,
                palette,
                app,
            );
            let top = if !card.grouped && agent_index == 0 {
                y
            } else {
                row_y
            };
            let bottom = if agent_index == last {
                row_y + 2
            } else {
                row_y + 1
            };
            hits.agents.push((
                Rect::new(content.x, top, content.width, bottom - top),
                agent.pane_id.clone(),
            ));
            row_y += 1;
        }
        y = y.saturating_add(height);
    }
}

/// One agent: status symbol and name on the left; either the state word
/// (grouped) or the space name (ungrouped) after it.
fn render_agent_line(
    buffer: &mut Buffer,
    area: Rect,
    agent: &crate::protocol::ClientShellAgent,
    workspace: Option<&str>,
    fill: Color,
    palette: &Palette,
    app: &AppTokens,
) {
    if area.is_empty() {
        return;
    }
    let right = area.right();
    let name_style = if agent.focused {
        Style::default()
            .fg(app.text_strong)
            .bg(fill)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette.text).bg(fill)
    };
    let (symbol, symbol_color) = app_status_symbol(agent.agent_status, palette, app);
    let mut x = put_segment(
        buffer,
        area.x,
        area.y,
        right,
        symbol,
        Style::default().fg(symbol_color).bg(fill),
    );
    x = (x + 1).min(right);
    match workspace {
        None => {
            let (word, color) = agent_state_word(agent.agent_status, palette, app);
            let word_width = display_width(word);
            let name_right = right.saturating_sub(word_width + 1).max(x);
            put_segment(
                buffer,
                x,
                area.y,
                name_right,
                agent_label(agent),
                name_style,
            );
            if right.saturating_sub(x) > word_width {
                put_text(
                    buffer,
                    right - word_width,
                    area.y,
                    word_width,
                    word,
                    Style::default().fg(color).bg(fill),
                );
            }
        }
        Some(workspace) => {
            x = put_segment(buffer, x, area.y, right, agent_label(agent), name_style);
            put_segment(
                buffer,
                (x + 1).min(right),
                area.y,
                right,
                workspace,
                Style::default().fg(app.text_muted).bg(fill),
            );
        }
    }
}

fn render_footer(
    buffer: &mut Buffer,
    layout: &AppSidebarLayout,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    hits: &mut ShellHitMap,
    app: &AppTokens,
) {
    let content = layout.content;
    let button = Style::default().fg(app.text_secondary).bg(app.card_bg);
    hits.new_workspace = shape::pill(
        buffer,
        content.x,
        layout.footer_y,
        content.right(),
        "+ new space",
        button,
        Corners::Square,
    );
    let attention = super::global_menu::global_menu_attention(snapshot);
    let menu_style = if attention {
        Style::default()
            .fg(config.palette.accent)
            .bg(app.card_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        button
    };
    let label = if attention { "● menu" } else { "≡ menu" };
    let width = display_width(label) + 2;
    hits.global_launcher = shape::pill(
        buffer,
        content.right().saturating_sub(width),
        layout.footer_y,
        content.right(),
        label,
        menu_style,
        Corners::Square,
    );
}
