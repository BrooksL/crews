//! Shared shapes for the polished shell (`ui.style = "app"`): cards, pills,
//! panels, and floating surfaces drawn from terminal cells.
//!
//! Softness comes from half-row caps: the row above a card is `▄` and the row
//! below is `▀`, colored with the card fill over the surface beneath. Rounded
//! corners (kitty graphics) arrive later; callers already pass [`Corners`] so
//! they won't change when they do.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

const UPPER_CAP: &str = "▄";
const LOWER_CAP: &str = "▀";

/// How corner cells are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Corners {
    /// Corner cells use the same half blocks as the rest of the edge.
    #[default]
    Square,
}

/// A filled shape and the color of the surface it sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Fill {
    pub fill: Color,
    pub beneath: Color,
}

impl Fill {
    pub(super) fn new(fill: Color, beneath: Color) -> Self {
        Self { fill, beneath }
    }
}

/// Set the background of every cell in `rect`, keeping symbols and foregrounds.
pub(super) fn fill_bg(buffer: &mut Buffer, rect: Rect, color: Color) {
    let rect = rect.intersection(buffer.area);
    for y in rect.top()..rect.bottom() {
        for x in rect.left()..rect.right() {
            buffer[(x, y)].set_bg(color);
        }
    }
}

/// Clear `rect` to blank cells on `color`.
pub(super) fn clear(buffer: &mut Buffer, rect: Rect, color: Color) {
    let rect = rect.intersection(buffer.area);
    let style = Style::default().bg(color);
    for y in rect.top()..rect.bottom() {
        for x in rect.left()..rect.right() {
            buffer[(x, y)].reset();
            buffer[(x, y)].set_symbol(" ").set_style(style);
        }
    }
}

/// Replace any background in `from` with `to` inside `rect`.
pub(super) fn recolor_bg(buffer: &mut Buffer, rect: Rect, from: &[Color], to: Color) {
    let rect = rect.intersection(buffer.area);
    for y in rect.top()..rect.bottom() {
        for x in rect.left()..rect.right() {
            let cell = &mut buffer[(x, y)];
            if from.contains(&cell.bg) {
                cell.set_bg(to);
            }
        }
    }
}

fn cap_row(buffer: &mut Buffer, x: u16, y: u16, width: u16, symbol: &str, fill: Fill) {
    let row = Rect::new(x, y, width, 1).intersection(buffer.area);
    let style = Style::default().fg(fill.fill).bg(fill.beneath);
    for cx in row.left()..row.right() {
        buffer[(cx, row.y)].reset();
        buffer[(cx, row.y)].set_symbol(symbol).set_style(style);
    }
}

/// Half-row cap above content: `▄` in the fill color over the surface beneath.
pub(super) fn top_cap(buffer: &mut Buffer, x: u16, y: u16, width: u16, fill: Fill) {
    cap_row(buffer, x, y, width, UPPER_CAP, fill);
}

/// Half-row cap below content: `▀` in the fill color over the surface beneath.
pub(super) fn bottom_cap(buffer: &mut Buffer, x: u16, y: u16, width: u16, fill: Fill) {
    cap_row(buffer, x, y, width, LOWER_CAP, fill);
}

/// A card: `content` rows filled, with caps on the row above and the row below.
/// Returns the full footprint including caps (the click target).
pub(super) fn card(buffer: &mut Buffer, content: Rect, fill: Fill, _corners: Corners) -> Rect {
    if content.is_empty() {
        return Rect::default();
    }
    if let Some(top) = content.y.checked_sub(1) {
        top_cap(buffer, content.x, top, content.width, fill);
    }
    clear(buffer, content, fill.fill);
    bottom_cap(buffer, content.x, content.bottom(), content.width, fill);
    let top = content.y.saturating_sub(1);
    Rect::new(
        content.x,
        top,
        content.width,
        content.bottom().saturating_add(1) - top,
    )
    .intersection(buffer.area)
}

/// A panel spanning `outer`: caps on its first and last rows. The interior is
/// left to the caller, since panel content covers it every frame anyway.
/// Returns the interior (the rows between the caps).
pub(super) fn panel(buffer: &mut Buffer, outer: Rect, fill: Fill, _corners: Corners) -> Rect {
    if outer.height < 3 || outer.width == 0 {
        return Rect::default();
    }
    top_cap(buffer, outer.x, outer.y, outer.width, fill);
    bottom_cap(buffer, outer.x, outer.bottom() - 1, outer.width, fill);
    Rect::new(outer.x, outer.y + 1, outer.width, outer.height - 2)
}

// Toasts and popovers use this from phase 5.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn floating(buffer: &mut Buffer, content: Rect, fill: Fill, corners: Corners) -> Rect {
    card(buffer, content, fill, corners)
}

/// A one-row pill: `label` with one column of padding on each side, filled
/// with `style`'s background. Clipped at `right`. Returns the pill's rect.
// Sidebar chips and buttons use this from phase 2.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn pill(
    buffer: &mut Buffer,
    x: u16,
    y: u16,
    right: u16,
    label: &str,
    style: Style,
    _corners: Corners,
) -> Rect {
    let fill = style.bg.unwrap_or(Color::Reset);
    let label_width = super::render::display_width(label);
    let width = label_width.saturating_add(2).min(right.saturating_sub(x));
    let rect = Rect::new(x, y, width, 1).intersection(buffer.area);
    if rect.is_empty() {
        return Rect::default();
    }
    clear(buffer, rect, fill);
    if rect.width > 2 {
        buffer.set_stringn(rect.x + 1, y, label, usize::from(rect.width - 2), style);
    }
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILL: Color = Color::Rgb(41, 44, 51);
    const BENEATH: Color = Color::Rgb(31, 34, 40);

    fn symbol(buffer: &Buffer, x: u16, y: u16) -> &str {
        buffer[(x, y)].symbol()
    }

    #[test]
    fn card_draws_half_row_caps_around_content() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
        let footprint = card(
            &mut buffer,
            Rect::new(2, 1, 4, 2),
            Fill::new(FILL, BENEATH),
            Corners::Square,
        );

        assert_eq!(footprint, Rect::new(2, 0, 4, 4));
        for x in 2..6 {
            assert_eq!(symbol(&buffer, x, 0), "▄");
            assert_eq!(buffer[(x, 0)].fg, FILL);
            assert_eq!(buffer[(x, 0)].bg, BENEATH);
            assert_eq!(buffer[(x, 1)].bg, FILL);
            assert_eq!(buffer[(x, 2)].bg, FILL);
            assert_eq!(symbol(&buffer, x, 3), "▀");
            assert_eq!(buffer[(x, 3)].fg, FILL);
            assert_eq!(buffer[(x, 3)].bg, BENEATH);
        }
        assert_eq!(symbol(&buffer, 1, 0), " ", "nothing drawn outside the card");
        assert_eq!(symbol(&buffer, 6, 3), " ");
    }

    #[test]
    fn square_mode_never_uses_quadrant_characters() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 4));
        card(
            &mut buffer,
            Rect::new(0, 1, 8, 2),
            Fill::new(FILL, BENEATH),
            Corners::Square,
        );
        for cell in buffer.content() {
            assert!(!["▗", "▖", "▝", "▘"].contains(&cell.symbol()));
        }
    }

    #[test]
    fn card_at_the_top_edge_skips_the_missing_cap_row() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 3));
        let footprint = card(
            &mut buffer,
            Rect::new(0, 0, 4, 1),
            Fill::new(FILL, BENEATH),
            Corners::Square,
        );
        assert_eq!(footprint, Rect::new(0, 0, 4, 2));
        assert_eq!(symbol(&buffer, 0, 1), "▀");
    }

    #[test]
    fn panel_returns_the_rows_between_its_caps() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 6));
        let interior = panel(
            &mut buffer,
            Rect::new(1, 0, 4, 6),
            Fill::new(FILL, BENEATH),
            Corners::Square,
        );
        assert_eq!(interior, Rect::new(1, 1, 4, 4));
        assert_eq!(symbol(&buffer, 1, 0), "▄");
        assert_eq!(symbol(&buffer, 4, 5), "▀");
    }

    #[test]
    fn floating_surface_is_a_card_for_now() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 5));
        let content = Rect::new(1, 1, 5, 3);
        let footprint = floating(
            &mut buffer,
            content,
            Fill::new(FILL, BENEATH),
            Corners::Square,
        );
        assert_eq!(footprint, Rect::new(1, 0, 5, 5));
        assert_eq!(symbol(&buffer, 1, 0), "▄");
        assert_eq!(symbol(&buffer, 1, 4), "▀");
    }

    #[test]
    fn pill_pads_the_label_by_one_column_each_side() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 1));
        let rect = pill(
            &mut buffer,
            1,
            0,
            12,
            "spaces",
            Style::default().fg(Color::White).bg(FILL),
            Corners::Square,
        );
        assert_eq!(rect, Rect::new(1, 0, 8, 1));
        assert_eq!(symbol(&buffer, 1, 0), " ");
        assert_eq!(symbol(&buffer, 2, 0), "s");
        assert_eq!(symbol(&buffer, 8, 0), " ");
        assert!((1..9).all(|x| buffer[(x, 0)].bg == FILL));
        assert_eq!(buffer[(9, 0)].bg, Color::Reset);
    }

    #[test]
    fn pill_clips_at_the_right_edge() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 1));
        let rect = pill(
            &mut buffer,
            2,
            0,
            6,
            "agents",
            Style::default().bg(FILL),
            Corners::Square,
        );
        assert_eq!(rect, Rect::new(2, 0, 4, 1));
    }

    #[test]
    fn recolor_bg_only_touches_matching_cells() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 1));
        buffer[(1, 0)].set_bg(Color::Blue);
        let area = buffer.area;
        recolor_bg(&mut buffer, area, &[Color::Reset], FILL);
        assert_eq!(buffer[(0, 0)].bg, FILL);
        assert_eq!(buffer[(1, 0)].bg, Color::Blue);
        assert_eq!(buffer[(2, 0)].bg, FILL);
    }
}
