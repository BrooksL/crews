//! Shared shapes for the polished shell (`ui.style = "app"`): cards, pills,
//! panels, and floating surfaces drawn from terminal cells.
//!
//! Softness comes from half-row caps: the row above a card is `▄` and the row
//! below is `▀`, colored with the card fill over the surface beneath.
//!
//! In rounded mode the corner cells of caps, and the end cells of pills, are
//! left blank on the surface beneath and recorded as [`CornerMark`]s. The
//! client turns each mark into a small kitty image under the text that paints
//! the fill with a rounded edge. Square mode draws the same half blocks all
//! the way across.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

const UPPER_CAP: &str = "▄";
const LOWER_CAP: &str = "▀";

/// Which edge of a shape a corner image rounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CornerKind {
    /// Cap cells: the fill covers the lower (top caps) or upper (bottom caps)
    /// half of the cell, rounded on one side.
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    /// Pill ends: a half-ellipse the full height of the cell.
    PillLeft,
    PillRight,
}

/// How round a corner is. Radii scale with the cell height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Radius {
    Card,
    Panel,
    Pill,
}

/// One cell that rounded mode paints with an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CornerMark {
    pub x: u16,
    pub y: u16,
    pub kind: CornerKind,
    pub radius: Radius,
    pub fill: Color,
    pub beneath: Color,
}

/// A soft shadow under a floating surface. `area` is the cells the shadow
/// image covers; `card` is the surface's own footprint inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ShadowMark {
    pub area: Rect,
    pub card: Rect,
}

/// How corner cells are drawn, and (in rounded mode) the marks collected
/// while drawing a frame.
#[derive(Debug, Default)]
pub(crate) struct Corners {
    rounded: bool,
    marks: Vec<CornerMark>,
    shadows: Vec<ShadowMark>,
}

impl Corners {
    /// Corner cells use the same half blocks as the rest of the edge.
    pub(crate) fn square() -> Self {
        Self::default()
    }

    /// Corner cells are left for rounded images.
    pub(crate) fn rounded() -> Self {
        Self {
            rounded: true,
            ..Self::default()
        }
    }

    pub(crate) fn is_rounded(&self) -> bool {
        self.rounded
    }

    pub(crate) fn marks(&self) -> &[CornerMark] {
        &self.marks
    }

    pub(crate) fn shadows(&self) -> &[ShadowMark] {
        &self.shadows
    }

    /// Recolor the marks drawn so far, so their images match cells that a
    /// later pass recolored (the scrim behind an open menu).
    pub(super) fn recolor_marks(&mut self, recolor: impl Fn(Color) -> Color) {
        for mark in &mut self.marks {
            mark.fill = recolor(mark.fill);
            mark.beneath = recolor(mark.beneath);
        }
    }

    fn mark(&mut self, buffer: &mut Buffer, mark: CornerMark) {
        if let Some(cell) = buffer.cell_mut((mark.x, mark.y)) {
            let symbol_is_blank = cell.symbol().trim().is_empty()
                || cell.symbol() == UPPER_CAP
                || cell.symbol() == LOWER_CAP;
            if symbol_is_blank {
                cell.reset();
                cell.set_symbol(" ");
            }
            cell.set_bg(mark.beneath);
            self.marks.push(mark);
        }
    }
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

/// Round the two end cells of a cap row.
fn round_cap(
    buffer: &mut Buffer,
    row: Rect,
    top: bool,
    fill: Fill,
    radius: Radius,
    corners: &mut Corners,
) {
    if !corners.rounded || row.width < 2 || row.y >= buffer.area.bottom() {
        return;
    }
    let (left, right) = if top {
        (CornerKind::TopLeft, CornerKind::TopRight)
    } else {
        (CornerKind::BottomLeft, CornerKind::BottomRight)
    };
    for (x, kind) in [(row.x, left), (row.right() - 1, right)] {
        corners.mark(
            buffer,
            CornerMark {
                x,
                y: row.y,
                kind,
                radius,
                fill: fill.fill,
                beneath: fill.beneath,
            },
        );
    }
}

fn rounded_card(
    buffer: &mut Buffer,
    content: Rect,
    fill: Fill,
    radius: Radius,
    corners: &mut Corners,
) -> Rect {
    if content.is_empty() {
        return Rect::default();
    }
    if let Some(top) = content.y.checked_sub(1) {
        top_cap(buffer, content.x, top, content.width, fill);
        round_cap(
            buffer,
            Rect::new(content.x, top, content.width, 1),
            true,
            fill,
            radius,
            corners,
        );
    }
    clear(buffer, content, fill.fill);
    bottom_cap(buffer, content.x, content.bottom(), content.width, fill);
    round_cap(
        buffer,
        Rect::new(content.x, content.bottom(), content.width, 1),
        false,
        fill,
        radius,
        corners,
    );
    let top = content.y.saturating_sub(1);
    Rect::new(
        content.x,
        top,
        content.width,
        content.bottom().saturating_add(1) - top,
    )
    .intersection(buffer.area)
}

/// A card: `content` rows filled, with caps on the row above and the row below.
/// Returns the full footprint including caps (the click target).
pub(super) fn card(buffer: &mut Buffer, content: Rect, fill: Fill, corners: &mut Corners) -> Rect {
    rounded_card(buffer, content, fill, Radius::Card, corners)
}

/// A panel spanning `outer`: caps on its first and last rows. The interior is
/// left to the caller, since panel content covers it every frame anyway.
/// Returns the interior (the rows between the caps).
pub(super) fn panel(buffer: &mut Buffer, outer: Rect, fill: Fill, corners: &mut Corners) -> Rect {
    if outer.height < 3 || outer.width == 0 {
        return Rect::default();
    }
    top_cap(buffer, outer.x, outer.y, outer.width, fill);
    round_cap(
        buffer,
        Rect::new(outer.x, outer.y, outer.width, 1),
        true,
        fill,
        Radius::Panel,
        corners,
    );
    bottom_cap(buffer, outer.x, outer.bottom() - 1, outer.width, fill);
    round_cap(
        buffer,
        Rect::new(outer.x, outer.bottom() - 1, outer.width, 1),
        false,
        fill,
        Radius::Panel,
        corners,
    );
    Rect::new(outer.x, outer.y + 1, outer.width, outer.height - 2)
}

/// Columns and rows a floating surface's shadow reaches past its footprint.
const SHADOW_MARGIN_COLUMNS: u16 = 3;
const SHADOW_MARGIN_ROWS: u16 = 2;

/// A floating surface (toast, popover): a card plus, in rounded mode, a soft
/// shadow around it.
// Toasts use this; they land on a separate branch.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn floating(
    buffer: &mut Buffer,
    content: Rect,
    fill: Fill,
    corners: &mut Corners,
) -> Rect {
    let footprint = card(buffer, content, fill, corners);
    push_shadow(buffer, footprint, corners);
    footprint
}

/// A floating surface over mixed content (menus, overlays). Like
/// [`floating`], but `content` is left as drawn, and each cap cell keeps its
/// own background as the surface beneath. `default_beneath` stands in for
/// cells with no background set.
pub(super) fn floating_over(
    buffer: &mut Buffer,
    content: Rect,
    fill: Color,
    default_beneath: Color,
    corners: &mut Corners,
) -> Rect {
    let content = content.intersection(buffer.area);
    if content.is_empty() {
        return Rect::default();
    }
    let top = content.y.checked_sub(1);
    let bottom = (content.bottom() < buffer.area.bottom()).then_some(content.bottom());
    for (y, symbol, ends) in [
        (top, UPPER_CAP, (CornerKind::TopLeft, CornerKind::TopRight)),
        (
            bottom,
            LOWER_CAP,
            (CornerKind::BottomLeft, CornerKind::BottomRight),
        ),
    ] {
        let Some(y) = y else { continue };
        for x in content.left()..content.right() {
            let cell = &mut buffer[(x, y)];
            let beneath = match cell.bg {
                Color::Reset => default_beneath,
                color => color,
            };
            cell.reset();
            cell.set_symbol(symbol)
                .set_style(Style::default().fg(fill).bg(beneath));
            let kind = if x == content.x {
                Some(ends.0)
            } else if x + 1 == content.right() {
                Some(ends.1)
            } else {
                None
            };
            if let Some(kind) = kind.filter(|_| corners.rounded && content.width >= 2) {
                corners.mark(
                    buffer,
                    CornerMark {
                        x,
                        y,
                        kind,
                        radius: Radius::Card,
                        fill,
                        beneath,
                    },
                );
            }
        }
    }
    let first = top.unwrap_or(content.y);
    let last = bottom.map_or(content.bottom(), |row| row + 1);
    let footprint = Rect::new(content.x, first, content.width, last - first);
    push_shadow(buffer, footprint, corners);
    footprint
}

fn push_shadow(buffer: &Buffer, footprint: Rect, corners: &mut Corners) {
    if corners.rounded && !footprint.is_empty() {
        let left = footprint.x.saturating_sub(SHADOW_MARGIN_COLUMNS);
        let top = footprint.y.saturating_sub(SHADOW_MARGIN_ROWS);
        let area = Rect::new(
            left,
            top,
            footprint.right() + SHADOW_MARGIN_COLUMNS - left,
            footprint.bottom() + SHADOW_MARGIN_ROWS - top,
        )
        .intersection(buffer.area);
        corners.shadows.push(ShadowMark {
            area,
            card: footprint,
        });
    }
}

/// A one-row pill: `label` with one column of padding on each side, filled
/// with `style`'s background, sitting on `beneath`. Clipped at `right`.
/// Returns the pill's rect.
pub(super) fn pill(
    buffer: &mut Buffer,
    (x, y): (u16, u16),
    right: u16,
    label: &str,
    style: Style,
    beneath: Color,
    corners: &mut Corners,
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
    round_pill_ends(buffer, rect, fill, beneath, corners);
    rect
}

/// Round both ends of an already-filled one-row pill.
pub(super) fn round_pill_ends(
    buffer: &mut Buffer,
    rect: Rect,
    fill: Color,
    beneath: Color,
    corners: &mut Corners,
) {
    if !corners.rounded || rect.width < 2 {
        return;
    }
    for (x, kind) in [
        (rect.x, CornerKind::PillLeft),
        (rect.right() - 1, CornerKind::PillRight),
    ] {
        corners.mark(
            buffer,
            CornerMark {
                x,
                y: rect.y,
                kind,
                radius: Radius::Pill,
                fill,
                beneath,
            },
        );
    }
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
            &mut Corners::square(),
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
            &mut Corners::square(),
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
            &mut Corners::square(),
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
            &mut Corners::square(),
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
            &mut Corners::square(),
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
            (1, 0),
            12,
            "spaces",
            Style::default().fg(Color::White).bg(FILL),
            BENEATH,
            &mut Corners::square(),
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
            (2, 0),
            6,
            "agents",
            Style::default().bg(FILL),
            BENEATH,
            &mut Corners::square(),
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

    #[test]
    fn rounded_card_leaves_corner_cells_on_the_surface_beneath() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
        let mut corners = Corners::rounded();
        card(
            &mut buffer,
            Rect::new(2, 1, 4, 2),
            Fill::new(FILL, BENEATH),
            &mut corners,
        );
        let kinds = corners
            .marks()
            .iter()
            .map(|mark| (mark.x, mark.y, mark.kind))
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                (2, 0, CornerKind::TopLeft),
                (5, 0, CornerKind::TopRight),
                (2, 3, CornerKind::BottomLeft),
                (5, 3, CornerKind::BottomRight),
            ]
        );
        for (x, y) in [(2, 0), (5, 0), (2, 3), (5, 3)] {
            assert_eq!(symbol(&buffer, x, y), " ");
            assert_eq!(buffer[(x, y)].bg, BENEATH);
        }
        assert_eq!(
            symbol(&buffer, 3, 0),
            "▄",
            "inner cap cells keep their half blocks"
        );
        assert!(corners
            .marks()
            .iter()
            .all(|mark| mark.radius == Radius::Card
                && mark.fill == FILL
                && mark.beneath == BENEATH));
    }

    #[test]
    fn square_mode_records_no_marks() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
        let mut corners = Corners::square();
        card(
            &mut buffer,
            Rect::new(2, 1, 4, 2),
            Fill::new(FILL, BENEATH),
            &mut corners,
        );
        floating(
            &mut buffer,
            Rect::new(2, 1, 4, 2),
            Fill::new(FILL, BENEATH),
            &mut corners,
        );
        assert!(corners.marks().is_empty() && corners.shadows().is_empty());
        assert_eq!(symbol(&buffer, 2, 0), "▄");
    }

    #[test]
    fn rounded_pill_ends_keep_their_label() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 1));
        let mut corners = Corners::rounded();
        let rect = pill(
            &mut buffer,
            (1, 0),
            12,
            "ab",
            Style::default().bg(FILL),
            BENEATH,
            &mut corners,
        );
        assert_eq!(rect, Rect::new(1, 0, 4, 1));
        assert_eq!(buffer[(1, 0)].bg, BENEATH);
        assert_eq!(buffer[(4, 0)].bg, BENEATH);
        assert_eq!(buffer[(2, 0)].bg, FILL);
        assert_eq!(symbol(&buffer, 2, 0), "a");
        let kinds = corners
            .marks()
            .iter()
            .map(|mark| mark.kind)
            .collect::<Vec<_>>();
        assert_eq!(kinds, vec![CornerKind::PillLeft, CornerKind::PillRight]);
    }

    #[test]
    fn rounded_panel_uses_the_panel_radius_and_floating_adds_a_shadow() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 12));
        let mut corners = Corners::rounded();
        panel(
            &mut buffer,
            Rect::new(0, 0, 20, 12),
            Fill::new(FILL, BENEATH),
            &mut corners,
        );
        assert!(corners
            .marks()
            .iter()
            .all(|mark| mark.radius == Radius::Panel));
        let footprint = floating(
            &mut buffer,
            Rect::new(6, 4, 6, 2),
            Fill::new(FILL, BENEATH),
            &mut corners,
        );
        assert_eq!(
            corners.shadows(),
            &[ShadowMark {
                area: Rect::new(3, 1, 12, 8),
                card: footprint,
            }]
        );
    }
}
