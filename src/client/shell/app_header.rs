//! The panel header for the polished shell (`ui.style = "app"`). It replaces
//! the tab bar: space name, branch, tabs as a segmented control, a new tab
//! button, and split / zoom / more icon buttons on the right.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use super::render::tabs::{centered_tab_scroll, max_tab_scroll, tab_drop_indicator_x};
use super::render::{display_width, put_segment, put_text};
use super::shape;
use super::*;
use crate::app::app_tokens::AppTokens;

/// Columns between the panel edge and header content.
const HEADER_INSET: u16 = 2;
/// Icon buttons and the new tab button are three columns wide.
const BUTTON_WIDTH: u16 = 3;
const ICON_SPLIT: &str = "◫";
const ICON_ZOOM: &str = "⤢";
const ICON_MORE: &str = "⋯";

pub(super) struct HeaderState<'a> {
    pub tab_scroll: &'a mut usize,
    pub reveal_focused_tab: &'a mut bool,
    pub tab_drag_insert_index: Option<usize>,
    pub corners: &'a mut shape::Corners,
}

pub(super) fn render_app_header(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    state: HeaderState<'_>,
    hits: &mut ShellHitMap,
    app: &AppTokens,
) {
    let palette = &config.palette;
    let inner = Rect::new(
        area.x + HEADER_INSET,
        area.y,
        area.width.saturating_sub(2 * HEADER_INSET),
        1,
    );
    if inner.width < 2 * BUTTON_WIDTH {
        return;
    }
    let base = Style::default().bg(app.pane_bg);
    let tabs = snapshot
        .tabs
        .iter()
        .filter(|tab| Some(tab.workspace_id.as_str()) == snapshot.focused_workspace_id.as_deref())
        .collect::<Vec<_>>();
    let zoomed = tabs.iter().any(|tab| tab.focused && tab.zoomed);

    // Right side: split, zoom, more.
    let mut button_x = inner.right();
    let mut icon = |icon: &str, accent: bool| {
        button_x = button_x.saturating_sub(BUTTON_WIDTH);
        let rect = Rect::new(button_x, inner.y, BUTTON_WIDTH, 1);
        let fg = if accent {
            palette.accent
        } else {
            app.text_secondary
        };
        put_text(buffer, rect.x + 1, rect.y, 1, icon, base.fg(fg));
        button_x = button_x.saturating_sub(1);
        rect
    };
    hits.header_more = icon(ICON_MORE, false);
    hits.header_zoom = icon(ICON_ZOOM, zoomed);
    hits.header_split = icon(ICON_SPLIT, false);
    let left_right = hits.header_split.x.saturating_sub(2);

    // Left side: space name, branch, then the tabs.
    let workspace = snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.focused);
    let mut x = inner.x;
    if let Some(workspace) = workspace {
        x = put_segment(
            buffer,
            x,
            inner.y,
            left_right,
            &workspace.label,
            base.fg(app.text_strong).add_modifier(Modifier::BOLD),
        );
        if let Some(branch) = workspace.branch.as_deref() {
            x = put_segment(
                buffer,
                (x + 1).min(left_right),
                inner.y,
                left_right,
                branch,
                base.fg(palette.accent),
            );
        }
        // The config badge slot (agent profiles) goes here.
        x = (x + 2).min(left_right);
    }

    render_tabs(
        buffer,
        Rect::new(x, inner.y, left_right.saturating_sub(x), 1),
        &tabs,
        config,
        state,
        hits,
        app,
    );
}

fn tab_pill_width(tab: &ClientShellTab) -> u16 {
    display_width(&tab_text(tab)) + 2
}

/// `<number> <name>`; the name only when the tab has one of its own.
fn tab_text(tab: &ClientShellTab) -> String {
    let mut text = tab.number.to_string();
    if tab.custom_label {
        text.push(' ');
        text.push_str(&tab.label);
    }
    if tab.zoomed {
        text.push_str(" Z");
    }
    text
}

/// The tab strip: a segment track holding one pill per tab, then the new tab
/// button. When tabs overflow, the track's end cells become ‹ and › buttons.
fn render_tabs(
    buffer: &mut Buffer,
    area: Rect,
    tabs: &[&ClientShellTab],
    config: &ClientShellConfig,
    state: HeaderState<'_>,
    hits: &mut ShellHitMap,
    app: &AppTokens,
) {
    let palette = &config.palette;
    let new_tab_width = if config.mouse_capture {
        BUTTON_WIDTH + 1
    } else {
        0
    };
    // Track end cells (padding, or the scroll buttons).
    let available = area.width.saturating_sub(new_tab_width + 2);
    let widths = tabs
        .iter()
        .map(|tab| tab_pill_width(tab))
        .collect::<Vec<_>>();
    let total = widths.iter().copied().fold(0u16, u16::saturating_add);
    let overflow = total > available;
    let max_scroll = max_tab_scroll(&widths, available);
    if !overflow {
        *state.tab_scroll = 0;
    } else if *state.reveal_focused_tab {
        if let Some(focused) = tabs.iter().position(|tab| tab.focused) {
            *state.tab_scroll = centered_tab_scroll(focused, &widths, available).min(max_scroll);
        }
    } else {
        *state.tab_scroll = (*state.tab_scroll).min(max_scroll);
    }
    *state.reveal_focused_tab = false;

    if !tabs.is_empty() && available > 0 {
        let track_style = Style::default().bg(app.segment_bg);
        let track_x = area.x;
        // Lay out the visible tabs first so the track can be sized to them.
        let mut visible = Vec::new();
        let mut used = 0u16;
        for (index, tab) in tabs.iter().enumerate().skip(*state.tab_scroll) {
            if used + widths[index] > available {
                break;
            }
            visible.push((index, *tab));
            used += widths[index];
        }
        let track = Rect::new(track_x, area.y, used + 2, 1);
        shape::clear(buffer, track, app.segment_bg);
        shape::round_pill_ends(buffer, track, app.segment_bg, app.pane_bg, state.corners);
        if overflow {
            hits.tab_scroll_left = Rect::new(track.x, area.y, 1, 1);
            hits.tab_scroll_right = Rect::new(track.right() - 1, area.y, 1, 1);
            let arrow = |enabled: bool| {
                track_style.fg(if enabled {
                    app.text_secondary
                } else {
                    app.label
                })
            };
            put_text(
                buffer,
                track.x,
                area.y,
                1,
                "‹",
                arrow(*state.tab_scroll > 0),
            );
            put_text(
                buffer,
                track.right() - 1,
                area.y,
                1,
                "›",
                arrow(*state.tab_scroll < max_scroll),
            );
        }
        let mut x = track.x + 1;
        for (index, tab) in visible {
            let width = widths[index];
            let (fill, number, name) = if tab.focused {
                (
                    app.card_focus_bg,
                    Style::default()
                        .fg(palette.accent)
                        .add_modifier(Modifier::BOLD),
                    Style::default()
                        .fg(app.text_strong)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    app.segment_bg,
                    Style::default().fg(app.label),
                    Style::default().fg(app.text_secondary),
                )
            };
            let rect = Rect::new(x, area.y, width, 1);
            shape::clear(buffer, rect, fill);
            let text = tab_text(tab);
            let number_text = tab.number.to_string();
            let after_number = put_segment(
                buffer,
                rect.x + 1,
                area.y,
                rect.right() - 1,
                &number_text,
                number.bg(fill),
            );
            put_segment(
                buffer,
                after_number,
                area.y,
                rect.right() - 1,
                &text[number_text.len()..],
                name.bg(fill),
            );
            if tab.focused {
                shape::round_pill_ends(buffer, rect, fill, app.segment_bg, state.corners);
            }
            hits.tabs.push((rect, tab.tab_id.clone()));
            x += width;
        }
        if let Some(insert_index) = state.tab_drag_insert_index {
            if let Some(indicator_x) = tab_drop_indicator_x(hits, tabs, insert_index) {
                put_text(
                    buffer,
                    indicator_x.clamp(track.x, track.right().saturating_sub(1)),
                    area.y,
                    1,
                    "│",
                    track_style.fg(palette.accent),
                );
            }
        }
        if config.mouse_capture {
            hits.new_tab = Rect::new(track.right() + 1, area.y, BUTTON_WIDTH, 1).intersection(area);
        }
    } else if config.mouse_capture {
        hits.new_tab = Rect::new(area.x, area.y, BUTTON_WIDTH, 1).intersection(area);
    }
    if !hits.new_tab.is_empty() {
        put_text(
            buffer,
            hits.new_tab.x + 1,
            area.y,
            1,
            "+",
            Style::default().fg(app.text_secondary).bg(app.pane_bg),
        );
    }
}
