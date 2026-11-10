//! Popover menus and floating overlays for the polished shell
//! (`ui.style = "app"`): the global menu, pane and sidebar context menus, and
//! the floating frame given to larger overlays (settings, keybinds, ...).
//!
//! Menus are `float_bg` cards over the scrim. The hovered or keyboard-selected
//! row is a `float_hover_bg` pill across the inner width.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use super::global_menu::{global_menu_item_has_badge, global_menu_items, ClientGlobalMenuAction};
use super::key_hints::action_hint;
use super::render::{display_width, put_text};
use super::shape;
use super::*;
use crate::app::app_tokens::AppTokens;

/// Width of the global menu, and the narrowest a context menu gets.
pub(super) const POPOVER_WIDTH: u16 = 25;
/// Columns between a popover's edge and its row pills.
const PADDING: u16 = 1;

struct PopoverItem<'a> {
    label: &'a str,
    hint: Option<String>,
    badge: bool,
}

/// A drawn popover: its footprint (caps included) and one rect per item row.
pub(super) struct Popover {
    pub area: Rect,
    pub rows: Vec<(Rect, usize)>,
}

/// The global menu, opened from the `≡ menu` button. Its right edge lines up
/// with the button's, and its bottom cap sits on the row above the button.
/// When there's no room above, it opens below instead.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_global_menu(
    buffer: &mut Buffer,
    launcher: Rect,
    menu: &ClientGlobalMenuOverlay,
    snapshot: &ClientShellSnapshot,
    keybinds: &LiveKeybindConfig,
    palette: &Palette,
    app: &AppTokens,
    corners: &mut shape::Corners,
) -> Popover {
    let entries = global_menu_items(snapshot);
    let items = entries
        .iter()
        .map(|(label, action)| PopoverItem {
            label,
            hint: (*action == ClientGlobalMenuAction::Binding(crate::input::KeybindAction::Detach))
                .then(|| action_hint(keybinds, &keybinds.keybinds.detach))
                .flatten(),
            badge: global_menu_item_has_badge(snapshot, *action),
        })
        .collect::<Vec<_>>();
    // A divider sets `detach` apart from the rest.
    let divider_before = entries.iter().position(|(_, action)| {
        *action == ClientGlobalMenuAction::Binding(crate::input::KeybindAction::Detach)
    });
    let height = items.len() as u16 + u16::from(divider_before.is_some());
    let screen = buffer.area;
    let width = POPOVER_WIDTH.min(screen.width);
    let right = launcher.right().clamp(screen.x + width, screen.right());
    let y = if launcher.y >= screen.y + height + 2 {
        launcher.y - 1 - height
    } else {
        launcher.bottom() + 1
    };
    let content = Rect::new(right - width, y, width, height).intersection(screen);
    draw(
        buffer,
        content,
        &items,
        divider_before,
        menu.highlighted,
        palette,
        app,
        corners,
    )
}

/// A context menu, with its top cap on the clicked row. It stays inside
/// `bounds`, moving left or up when it would spill past them.
pub(super) fn render_context_menu(
    buffer: &mut Buffer,
    menu: &ClientContextMenuOverlay,
    bounds: Rect,
    palette: &Palette,
    app: &AppTokens,
    corners: &mut shape::Corners,
) -> Popover {
    let entries = menu.items();
    let items = entries
        .iter()
        .map(|item| PopoverItem {
            label: item.label,
            hint: None,
            badge: false,
        })
        .collect::<Vec<_>>();
    let bounds = bounds.intersection(buffer.area);
    let width = context_menu_width(menu).min(bounds.width);
    let height = (items.len() as u16).min(bounds.height.saturating_sub(2));
    let x = menu
        .x
        .min(bounds.right().saturating_sub(width))
        .max(bounds.x);
    let y = (menu.y + 1)
        .min(bounds.bottom().saturating_sub(height + 1))
        .max(bounds.y + 1);
    draw(
        buffer,
        Rect::new(x, y, width, height),
        &items,
        None,
        menu.highlighted,
        palette,
        app,
        corners,
    )
}

#[allow(clippy::too_many_arguments)]
fn draw(
    buffer: &mut Buffer,
    content: Rect,
    items: &[PopoverItem<'_>],
    divider_before: Option<usize>,
    highlighted: usize,
    palette: &Palette,
    app: &AppTokens,
    corners: &mut shape::Corners,
) -> Popover {
    let content = content.intersection(buffer.area);
    if content.width <= 2 * PADDING + 2 || content.is_empty() {
        return Popover {
            area: Rect::default(),
            rows: Vec::new(),
        };
    }
    shape::clear(buffer, content, app.float_bg);
    let area = shape::floating_over(buffer, content, app.float_bg, app.pane_bg, corners);
    let inner_x = content.x + PADDING;
    let inner_width = content.width - 2 * PADDING;
    let mut rows = Vec::with_capacity(items.len());
    let mut y = content.y;
    for (index, item) in items.iter().enumerate() {
        if divider_before == Some(index) && y < content.bottom() {
            put_text(
                buffer,
                inner_x,
                y,
                inner_width,
                &"─".repeat(usize::from(inner_width)),
                Style::default().fg(app.float_divider).bg(app.float_bg),
            );
            y += 1;
        }
        if y >= content.bottom() {
            break;
        }
        let row = Rect::new(inner_x, y, inner_width, 1);
        let hovered = index == highlighted;
        let bg = if hovered {
            app.float_hover_bg
        } else {
            app.float_bg
        };
        if hovered {
            shape::clear(buffer, row, bg);
            shape::round_pill_ends(buffer, row, bg, app.float_bg, corners);
        }
        let label = if hovered {
            Style::default()
                .fg(app.text_strong)
                .bg(bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.text).bg(bg)
        };
        // Text sits one column in from the row pill's ends.
        let text_x = row.x + 1;
        let text_right = row.right() - 1;
        let mut label_right = text_right;
        if let Some(hint) = item.hint.as_deref() {
            let hint_width = display_width(hint);
            if hint_width + 2 < text_right - text_x {
                let hint_x = text_right - hint_width;
                put_text(
                    buffer,
                    hint_x,
                    y,
                    hint_width,
                    hint,
                    // `toast_body` doesn't meet contrast on the hover fill.
                    Style::default()
                        .fg(if hovered {
                            palette.text
                        } else {
                            app.toast_body
                        })
                        .bg(bg),
                );
                label_right = hint_x - 1;
            }
        } else if item.badge {
            put_text(
                buffer,
                text_right - 1,
                y,
                1,
                "●",
                Style::default().fg(palette.accent).bg(bg),
            );
            label_right = text_right - 2;
        }
        put_text(
            buffer,
            text_x,
            y,
            label_right.saturating_sub(text_x),
            item.label,
            label,
        );
        rows.push((row, index));
        y += 1;
    }
    Popover { area, rows }
}

/// Whether an overlay puts the scrim behind it. The navigator keeps the
/// workspace in view, as classic leaves it undimmed.
pub(super) fn overlay_scrims(overlay: &ClientShellOverlay) -> bool {
    !matches!(overlay, ClientShellOverlay::Navigator(_))
}

/// Width of a context menu: room for its longest label, and never narrower
/// than the global menu.
pub(super) fn context_menu_width(menu: &ClientContextMenuOverlay) -> u16 {
    let widest = menu
        .items()
        .iter()
        .map(|item| display_width(item.label))
        .max()
        .unwrap_or(0);
    (widest + 2 * PADDING + 2).max(POPOVER_WIDTH)
}

/// Draw a classic overlay centered in the panel instead of the window, then
/// give it the floating frame. `render` draws into a buffer covering the
/// panel's interior.
pub(super) fn render_floating_overlay(
    buffer: &mut Buffer,
    panel: Rect,
    render: impl FnOnce(&mut Buffer) -> Option<super::render::OverlayRender>,
    palette: &Palette,
    app: &AppTokens,
    corners: &mut shape::Corners,
) -> Option<super::render::OverlayRender> {
    // Leave the panel's cap rows for the overlay's own caps.
    let area = Rect::new(
        panel.x,
        panel.y + 1,
        panel.width,
        panel.height.saturating_sub(2),
    )
    .intersection(buffer.area);
    let mut panel_buffer = Buffer::empty(area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            panel_buffer[(x, y)] = buffer[(x, y)].clone();
        }
    }
    let mut rendered = render(&mut panel_buffer)?;
    buffer.merge(&panel_buffer);
    rendered.area = float_overlay_frame(buffer, rendered.area, palette, app, corners);
    Some(rendered)
}

const BORDER_SYMBOLS: [&str; 6] = ["┌", "┐", "└", "┘", "─", "│"];

/// Give a classic overlay, already drawn at `outer`, the app look: no border,
/// `float_bg` instead of the classic panel color, half-row caps above and
/// below, and a shadow in rounded mode. Returns the footprint, caps included.
pub(super) fn float_overlay_frame(
    buffer: &mut Buffer,
    outer: Rect,
    palette: &Palette,
    app: &AppTokens,
    corners: &mut shape::Corners,
) -> Rect {
    let outer = outer.intersection(buffer.area);
    if outer.is_empty() {
        return Rect::default();
    }
    for y in outer.top()..outer.bottom() {
        let edge_row = y == outer.top() || y + 1 == outer.bottom();
        for x in outer.left()..outer.right() {
            let cell = &mut buffer[(x, y)];
            let edge = edge_row || x == outer.left() || x + 1 == outer.right();
            if edge && BORDER_SYMBOLS.contains(&cell.symbol()) {
                cell.set_symbol(" ");
            }
            if cell.bg == palette.panel_bg || cell.bg == ratatui::style::Color::Reset {
                cell.set_bg(app.float_bg);
            }
        }
    }
    shape::floating_over(buffer, outer, app.float_bg, app.pane_bg, corners)
}
