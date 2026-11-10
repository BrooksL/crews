//! App-style toasts (`ui.style = "app"`): `float_bg` cards stacked down from
//! the panel's top-right corner, each with a status title, a body row, and
//! `open` / `dismiss` buttons.
//!
//! Classic keeps `notifications::render_visible_notification`.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};

use super::app_sidebar::app_status_symbol;
use super::render::{display_width, put_segment};
use super::shape::{self, Corners, Fill};
use super::*;
use crate::api::schema::AgentStatus;
use crate::app::app_tokens::AppTokens;

/// Card width, before clamping to the panel.
pub(super) const TOAST_WIDTH: u16 = 44;
/// Columns between the card's right edge and the panel's right edge.
pub(super) const TOAST_RIGHT_INSET: u16 = 2;
/// Row of the first card's top cap, counted from the panel's top.
pub(super) const TOAST_TOP_ROW: u16 = 2;
/// Content rows: title, body, actions.
const TOAST_CONTENT_ROWS: u16 = 3;
/// A card's footprint: the content rows plus a cap above and below.
pub(super) const TOAST_FOOTPRINT_ROWS: u16 = TOAST_CONTENT_ROWS + 2;
/// Blank rows between stacked cards.
pub(super) const TOAST_GAP_ROWS: u16 = 1;
/// Narrower than this and the card isn't drawn.
const TOAST_MIN_WIDTH: u16 = 16;
/// Columns from the card edge to the status symbol.
const TOAST_PADDING: u16 = 2;
/// Body and actions start this far past the status symbol.
const TOAST_INDENT: u16 = 2;
const OPEN_LABEL: &str = "open";
const DISMISS_LABEL: &str = "dismiss";

/// Footprints for up to `count` stacked cards in `panel`, top to bottom.
/// Cards that don't fit above the panel's bottom cap are left out.
pub(super) fn toast_footprints(panel: Rect, top_offset: u16, count: usize) -> Vec<Rect> {
    let width = TOAST_WIDTH.min(panel.width.saturating_sub(2 * TOAST_RIGHT_INSET));
    if width < TOAST_MIN_WIDTH || count == 0 {
        return Vec::new();
    }
    let x = panel.right() - TOAST_RIGHT_INSET - width;
    // Keep clear of the panel's own bottom cap row.
    let bottom = panel.bottom().saturating_sub(1);
    let mut top = panel
        .y
        .saturating_add(TOAST_TOP_ROW)
        .saturating_add(top_offset);
    let mut footprints = Vec::with_capacity(count);
    for _ in 0..count {
        if top.saturating_add(TOAST_FOOTPRINT_ROWS) > bottom {
            break;
        }
        footprints.push(Rect::new(x, top, width, TOAST_FOOTPRINT_ROWS));
        top = top.saturating_add(TOAST_FOOTPRINT_ROWS + TOAST_GAP_ROWS);
    }
    footprints
}

/// Symbol and color for a toast's title row: the sidebar's agent symbols, with
/// needs-attention drawn like blocked and finished like done. Toasts that
/// aren't about an agent get an accent dot.
fn toast_symbol(
    kind: SemanticNotificationKind,
    palette: &Palette,
    app: &AppTokens,
) -> (&'static str, Color) {
    match kind {
        SemanticNotificationKind::NeedsAttention => {
            app_status_symbol(AgentStatus::Blocked, palette, app)
        }
        SemanticNotificationKind::Finished => app_status_symbol(AgentStatus::Done, palette, app),
        SemanticNotificationKind::UpdateInstalled | SemanticNotificationKind::Custom => {
            ("●", palette.accent)
        }
    }
}

/// Draw the visible toasts as a stack of floating cards. Returns a hit for
/// each card drawn; cards that don't fit are skipped.
pub(super) fn render_app_toasts(
    buffer: &mut Buffer,
    panel: Rect,
    top_offset: u16,
    notifications: &[ClientVisibleNotification],
    palette: &Palette,
    app: &AppTokens,
    corners: &mut Corners,
) -> Vec<AppToastHit> {
    toast_footprints(panel, top_offset, notifications.len())
        .into_iter()
        .zip(notifications)
        .enumerate()
        .map(|(index, (footprint, notification))| {
            render_app_toast(
                buffer,
                footprint,
                index,
                notification,
                palette,
                app,
                corners,
            )
        })
        .collect()
}

fn render_app_toast(
    buffer: &mut Buffer,
    footprint: Rect,
    index: usize,
    notification: &ClientVisibleNotification,
    palette: &Palette,
    app: &AppTokens,
    corners: &mut Corners,
) -> AppToastHit {
    let fill = app.float_bg;
    let content = Rect::new(
        footprint.x,
        footprint.y + 1,
        footprint.width,
        TOAST_CONTENT_ROWS,
    );
    let card = shape::floating(buffer, content, Fill::new(fill, app.pane_bg), corners);
    let event = &notification.event;
    // The right edge of text; Phase 8 puts the config badge just inside it.
    let text_right = content.right().saturating_sub(TOAST_PADDING);
    let symbol_x = content.x + TOAST_PADDING;
    let indent_x = symbol_x + TOAST_INDENT;

    let (symbol, symbol_color) = toast_symbol(event.kind, palette, app);
    put_segment(
        buffer,
        symbol_x,
        content.y,
        text_right,
        symbol,
        Style::default().fg(symbol_color).bg(fill),
    );
    put_segment(
        buffer,
        indent_x,
        content.y,
        text_right,
        &event.title,
        Style::default()
            .fg(app.text_strong)
            .bg(fill)
            .add_modifier(Modifier::BOLD),
    );

    // An empty body keeps its row, so every card is the same height.
    if let Some(body) = event.body.as_deref() {
        put_segment(
            buffer,
            indent_x,
            content.y + 1,
            text_right,
            body,
            Style::default().fg(app.toast_body).bg(fill),
        );
    }

    let actions_y = content.y + 2;
    let open = if event.pane_id.is_some() {
        shape::pill(
            buffer,
            (indent_x, actions_y),
            text_right,
            OPEN_LABEL,
            Style::default()
                .fg(app.accent_fg)
                .bg(palette.accent)
                .add_modifier(Modifier::BOLD),
            fill,
            corners,
        )
    } else {
        Rect::default()
    };
    // A text button: the label with a column of click slack on each side, so
    // with no `open` pill its text lines up with the body.
    let dismiss_x = if open.is_empty() {
        indent_x - 1
    } else {
        open.right() + 1
    };
    let dismiss_width =
        (display_width(DISMISS_LABEL) + 2).min(content.right().saturating_sub(dismiss_x));
    let dismiss = Rect::new(dismiss_x, actions_y, dismiss_width, 1);
    put_segment(
        buffer,
        dismiss_x + 1,
        actions_y,
        text_right,
        DISMISS_LABEL,
        Style::default().fg(app.text_secondary).bg(fill),
    );

    AppToastHit {
        index,
        card,
        open,
        dismiss,
    }
}
