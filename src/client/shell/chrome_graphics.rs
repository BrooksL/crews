//! Rounded-mode images for the polished shell (`ui.style = "app"`).
//!
//! Each [`CornerMark`] becomes a one-cell RGBA image placed under the text
//! (z = -1, above cell backgrounds) that paints the shape's fill with a rounded
//! edge; the cell's own background is the surface beneath. [`ShadowMark`]s
//! become a soft shadow image under a floating surface.
//!
//! Images are generated once per (shape, colors, radius, cell size) and reused.
//! Each frame re-places what is visible (cheap, and it survives the screen
//! clears of full redraws), removes placements that went away, and deletes
//! images no shape uses any more.

use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use std::sync::Arc;

use ratatui::layout::Rect;
use ratatui::style::Color;

use super::shape::{CornerKind, Corners, Radius};
use crate::kitty_graphics::surface::Occlusion;
use crate::kitty_graphics::{GraphicsOperation, GraphicsOutput, HostCellSize};

/// Chrome images live in their own id range, clear of pane images
/// (10,000 to 910,000).
const CHROME_IMAGE_ID_BASE: u32 = 2_000_000;
const CHROME_IMAGE_ID_SPAN: u32 = 1_000_000;
/// The spec's radii and shadow are in pixels for a 21 px tall cell; they scale
/// with the real cell height.
const DESIGN_CELL_HEIGHT: f64 = 21.0;
const CARD_RADIUS: f64 = 8.0;
const PANEL_RADIUS: f64 = 10.0;
const SHADOW_OFFSET_Y: f64 = 10.0;
const SHADOW_BLUR: f64 = 28.0;
const SHADOW_ALPHA: f64 = 0.45;
/// Corner images sit under text, above cell backgrounds. Shadows sit one
/// layer lower so corner images draw over them.
const CORNER_Z: i32 = -1;
const SHADOW_Z: i32 = -2;
/// Samples per pixel axis for anti-aliasing.
const SUPERSAMPLE: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ImageKey {
    Corner {
        kind: CornerKind,
        radius: Radius,
        fill: Color,
        beneath: Color,
        cell: (u32, u32),
    },
    Shadow {
        cells: (u16, u16),
        card: (u16, u16, u16, u16),
        cell: (u32, u32),
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Placement {
    image: u32,
    x: u16,
    y: u16,
    cols: u16,
    rows: u16,
    z: i32,
}

impl Placement {
    /// Unique per image: one placement per cell position.
    fn id(self) -> u32 {
        (u32::from(self.y) << 16 | u32::from(self.x)) + 1
    }
}

#[derive(Debug, Default)]
pub(super) struct ChromeGraphics {
    images: HashMap<ImageKey, u32>,
    placements: HashSet<Placement>,
    next_image: u32,
}

impl ChromeGraphics {
    /// Graphics commands for this frame's marks: uploads for new images, a
    /// placement for every visible mark, and deletes for what went away.
    pub(super) fn encode(
        &mut self,
        corners: &Corners,
        occlusion: &Occlusion,
        cell: HostCellSize,
    ) -> GraphicsOutput {
        let mut output = GraphicsOutput::default();
        let cell_px = (cell.width_px, cell.height_px);
        let mut wanted = Vec::with_capacity(corners.marks().len() + corners.shadows().len());
        let mut used_images = HashSet::new();

        for shadow in corners.shadows() {
            if shadow.area.is_empty() || occlusion.covers_rect(shadow.card) {
                continue;
            }
            let key = ImageKey::Shadow {
                cells: (shadow.area.width, shadow.area.height),
                card: (
                    shadow.card.x - shadow.area.x,
                    shadow.card.y - shadow.area.y,
                    shadow.card.width,
                    shadow.card.height,
                ),
                cell: cell_px,
            };
            let image = self.image(key, cell, &mut output);
            used_images.insert(image);
            wanted.push(Placement {
                image,
                x: shadow.area.x,
                y: shadow.area.y,
                cols: shadow.area.width,
                rows: shadow.area.height,
                z: SHADOW_Z,
            });
        }
        for mark in corners.marks() {
            if occlusion.covers_rect(Rect::new(mark.x, mark.y, 1, 1)) {
                continue;
            }
            let key = ImageKey::Corner {
                kind: mark.kind,
                radius: mark.radius,
                fill: mark.fill,
                beneath: mark.beneath,
                cell: cell_px,
            };
            let image = self.image(key, cell, &mut output);
            used_images.insert(image);
            wanted.push(Placement {
                image,
                x: mark.x,
                y: mark.y,
                cols: 1,
                rows: 1,
                z: CORNER_Z,
            });
        }

        let mut bytes = Vec::new();
        let wanted_set = wanted.iter().copied().collect::<HashSet<_>>();
        for gone in self.placements.difference(&wanted_set) {
            if used_images.contains(&gone.image) {
                let _ = write!(
                    bytes,
                    "\x1b_Ga=d,d=i,i={},p={},q=2;\x1b\\",
                    gone.image,
                    gone.id()
                );
            }
        }
        // Images nothing uses this frame go away with their placements.
        let unused = self
            .images
            .iter()
            .filter(|(_, id)| !used_images.contains(*id))
            .map(|(key, id)| (*key, *id))
            .collect::<Vec<_>>();
        for (key, id) in unused {
            self.images.remove(&key);
            crate::kitty_graphics::encode_delete_image(&mut bytes, id);
        }
        for placement in &wanted {
            let _ = write!(
                bytes,
                "\x1b[{};{}H\x1b_Ga=p,i={},p={},c={},r={},z={},C=1,q=2;\x1b\\",
                placement.y + 1,
                placement.x + 1,
                placement.image,
                placement.id(),
                placement.cols,
                placement.rows,
                placement.z,
            );
        }
        output.push_bytes(bytes);
        self.placements = wanted_set;
        output
    }

    /// Delete every chrome image (switching to square mode, or detaching).
    pub(super) fn clear(&mut self) -> GraphicsOutput {
        let mut bytes = Vec::new();
        for id in self.images.values() {
            crate::kitty_graphics::encode_delete_image(&mut bytes, *id);
        }
        self.images.clear();
        self.placements.clear();
        GraphicsOutput::from_bytes(bytes)
    }

    fn image(&mut self, key: ImageKey, cell: HostCellSize, output: &mut GraphicsOutput) -> u32 {
        if let Some(id) = self.images.get(&key) {
            return *id;
        }
        let id = CHROME_IMAGE_ID_BASE + self.next_image % CHROME_IMAGE_ID_SPAN;
        self.next_image = self.next_image.wrapping_add(1);
        let (width, height, pixels) = match key {
            ImageKey::Corner {
                kind, radius, fill, ..
            } => (
                cell.width_px,
                cell.height_px,
                corner_rgba(kind, radius, rgb(fill), cell.width_px, cell.height_px),
            ),
            ImageKey::Shadow { cells, card, .. } => {
                let width = u32::from(cells.0) * cell.width_px;
                let height = u32::from(cells.1) * cell.height_px;
                (width, height, shadow_rgba(cells, card, cell))
            }
        };
        output.operations.push(GraphicsOperation::Upload {
            control: format!("a=t,t=d,f=32,s={width},v={height},i={id},q=2"),
            data: Arc::from(pixels),
        });
        self.images.insert(key, id);
        id
    }
}

fn rgb(color: Color) -> (u8, u8, u8) {
    crate::app::app_tokens::rgb(color).unwrap_or((0, 0, 0))
}

fn scale(cell_height: u32) -> f64 {
    f64::from(cell_height) / DESIGN_CELL_HEIGHT
}

/// Radius in pixels, clamped to the cell width and half the cell height.
fn radius_px(radius: Radius, width: u32, height: u32) -> f64 {
    let design = match radius {
        Radius::Card => CARD_RADIUS,
        Radius::Panel => PANEL_RADIUS,
        Radius::Pill => f64::INFINITY,
    };
    (design * scale(height))
        .min(f64::from(width))
        .min(f64::from(height) / 2.0)
}

/// Fraction of the pixel at (`px`, `py`) the shape covers, by supersampling.
fn coverage(px: u32, py: u32, inside: &impl Fn(f64, f64) -> bool) -> f64 {
    let mut hits = 0;
    for sy in 0..SUPERSAMPLE {
        for sx in 0..SUPERSAMPLE {
            let x = f64::from(px) + (f64::from(sx) + 0.5) / f64::from(SUPERSAMPLE);
            let y = f64::from(py) + (f64::from(sy) + 0.5) / f64::from(SUPERSAMPLE);
            if inside(x, y) {
                hits += 1;
            }
        }
    }
    f64::from(hits) / f64::from(SUPERSAMPLE * SUPERSAMPLE)
}

/// One cell of a rounded edge: `fill` where the shape is, transparent elsewhere.
pub(super) fn corner_rgba(
    kind: CornerKind,
    radius: Radius,
    fill: (u8, u8, u8),
    width: u32,
    height: u32,
) -> Vec<u8> {
    let (w, h) = (f64::from(width), f64::from(height));
    let half = h / 2.0;
    let r = radius_px(radius, width, height);
    // Work in a frame where the rounded corner is always at the left and the
    // fill hangs below the cell's middle (a top-left cap corner), then mirror.
    let (mirror_x, mirror_y) = match kind {
        CornerKind::TopLeft | CornerKind::PillLeft => (false, false),
        CornerKind::TopRight | CornerKind::PillRight => (true, false),
        CornerKind::BottomLeft => (false, true),
        CornerKind::BottomRight => (true, true),
    };
    let pill = matches!(kind, CornerKind::PillLeft | CornerKind::PillRight);
    let inside = |x: f64, y: f64| {
        let x = if mirror_x { w - x } else { x };
        let y = if mirror_y { h - y } else { y };
        if pill {
            // Half-ellipse spanning the full cell height, flat side on the right.
            let dx = (x - w) / w;
            let dy = (y - half) / half;
            return dx * dx + dy * dy <= 1.0;
        }
        if y < half {
            return false;
        }
        if x >= r || y >= half + r {
            return true;
        }
        let (cx, cy) = (r, half + r);
        (x - cx).powi(2) + (y - cy).powi(2) <= r * r
    };
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for py in 0..height {
        for px in 0..width {
            let alpha = coverage(px, py, &inside);
            pixels.extend_from_slice(&[fill.0, fill.1, fill.2, (alpha * 255.0).round() as u8]);
        }
    }
    pixels
}

/// A soft black shadow under a floating surface, offset downward and blurred,
/// transparent over the surface itself.
pub(super) fn shadow_rgba(
    cells: (u16, u16),
    card: (u16, u16, u16, u16),
    cell: HostCellSize,
) -> Vec<u8> {
    let (cw, ch) = (f64::from(cell.width_px), f64::from(cell.height_px));
    let width = u32::from(cells.0) * cell.width_px;
    let height = u32::from(cells.1) * cell.height_px;
    let scale = scale(cell.height_px);
    let blur = (SHADOW_BLUR * scale).max(1.0);
    let offset = SHADOW_OFFSET_Y * scale;
    // The card's footprint in pixels; its caps are half rows, so the visible
    // card starts half a row in from the top and bottom of the footprint.
    let left = f64::from(card.0) * cw;
    let right = f64::from(card.0 + card.2) * cw;
    let top = f64::from(card.1) * ch + ch / 2.0;
    let bottom = f64::from(card.1 + card.3) * ch - ch / 2.0;
    let radius = radius_px(Radius::Card, cell.width_px, cell.height_px);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for py in 0..height {
        for px in 0..width {
            let (x, y) = (f64::from(px) + 0.5, f64::from(py) + 0.5);
            let over_card = inside_rounded_rect(x, y, (left, top, right, bottom), radius);
            let alpha = if over_card {
                0.0
            } else {
                // Distance outside the offset card rectangle.
                let dx = (left - x).max(x - right).max(0.0);
                let dy = (top + offset - y).max(y - (bottom + offset)).max(0.0);
                let distance = (dx * dx + dy * dy).sqrt();
                let t = (1.0 - distance / blur).clamp(0.0, 1.0);
                SHADOW_ALPHA * t * t * (3.0 - 2.0 * t)
            };
            pixels.extend_from_slice(&[0, 0, 0, (alpha * 255.0).round() as u8]);
        }
    }
    pixels
}

/// Whether (`x`, `y`) is inside a rectangle with corners rounded by `radius`.
fn inside_rounded_rect(
    x: f64,
    y: f64,
    (left, top, right, bottom): (f64, f64, f64, f64),
    radius: f64,
) -> bool {
    if x < left || x >= right || y < top || y >= bottom {
        return false;
    }
    let cx = x.clamp(left + radius, right - radius);
    let cy = y.clamp(top + radius, bottom - radius);
    (x - cx).powi(2) + (y - cy).powi(2) <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;

    const FILL: Color = Color::Rgb(41, 44, 51);
    const BENEATH: Color = Color::Rgb(31, 34, 40);
    const CELL: HostCellSize = HostCellSize {
        width_px: 10,
        height_px: 21,
    };

    fn alpha(pixels: &[u8], width: u32, x: u32, y: u32) -> u8 {
        pixels[((y * width + x) * 4 + 3) as usize]
    }

    fn rounded_card() -> Corners {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
        let mut corners = Corners::rounded();
        super::super::shape::card(
            &mut buffer,
            Rect::new(2, 1, 4, 2),
            super::super::shape::Fill::new(FILL, BENEATH),
            &mut corners,
        );
        corners
    }

    fn text(output: &GraphicsOutput) -> String {
        String::from_utf8_lossy(&output.clone().into_inline_bytes()).into_owned()
    }

    #[test]
    fn top_left_corner_is_filled_below_the_middle_and_rounded() {
        let pixels = corner_rgba(CornerKind::TopLeft, Radius::Card, (41, 44, 51), 10, 21);
        assert_eq!(pixels.len(), 10 * 21 * 4);
        assert_eq!(
            alpha(&pixels, 10, 9, 2),
            0,
            "upper half is the surface beneath"
        );
        assert_eq!(alpha(&pixels, 10, 9, 18), 255, "lower half is the fill");
        assert_eq!(alpha(&pixels, 10, 0, 11), 0, "the very corner is cut away");
        assert_eq!(alpha(&pixels, 10, 5, 20), 255);
        assert_eq!(&pixels[..3], &[41, 44, 51]);
    }

    #[test]
    fn bottom_and_right_corners_mirror_the_top_left() {
        let top_left = corner_rgba(CornerKind::TopLeft, Radius::Card, (1, 2, 3), 10, 21);
        let bottom_right = corner_rgba(CornerKind::BottomRight, Radius::Card, (1, 2, 3), 10, 21);
        for (x, y) in [(0, 11), (3, 13), (9, 18), (5, 20)] {
            assert_eq!(
                alpha(&top_left, 10, x, y),
                alpha(&bottom_right, 10, 9 - x, 20 - y),
                "pixel {x},{y}"
            );
        }
    }

    #[test]
    fn pill_ends_are_full_height_half_ellipses() {
        let left = corner_rgba(CornerKind::PillLeft, Radius::Pill, (1, 2, 3), 10, 21);
        assert_eq!(alpha(&left, 10, 9, 10), 255, "flat side touches the pill");
        assert_eq!(alpha(&left, 10, 0, 0), 0);
        assert_eq!(alpha(&left, 10, 0, 20), 0);
        assert!(
            alpha(&left, 10, 1, 10) > 200,
            "the curve reaches the left edge mid-height"
        );
    }

    #[test]
    fn radii_are_clamped_and_scale_with_the_cell() {
        assert!((radius_px(Radius::Card, 10, 21) - 8.0).abs() < 1e-9);
        assert!((radius_px(Radius::Panel, 10, 21) - 10.0).abs() < 1e-9);
        assert!(
            (radius_px(Radius::Card, 20, 42) - 16.0).abs() < 1e-9,
            "retina cells"
        );
        assert!(
            (radius_px(Radius::Panel, 6, 21) - 6.0).abs() < 1e-9,
            "never wider than a cell"
        );
        assert!(
            (radius_px(Radius::Pill, 10, 21) - 10.0).abs() < 1e-9,
            "half the cell height"
        );
    }

    #[test]
    fn shadow_is_clear_over_the_card_and_darkest_just_below_it() {
        let cell = CELL;
        let pixels = shadow_rgba((10, 6), (3, 2, 4, 2), cell);
        let width = 10 * cell.width_px;
        let card_center = (35, 2 * 21 + 21);
        assert_eq!(alpha(&pixels, width, card_center.0, card_center.1), 0);
        let below = alpha(&pixels, width, 50, 4 * 21 - 10 + 8);
        let far = alpha(&pixels, width, 2, 2);
        assert!(below > 90, "{below}");
        assert_eq!(far, 0);
    }

    #[test]
    fn encode_uploads_once_and_replaces_every_frame() {
        let corners = rounded_card();
        let mut graphics = ChromeGraphics::default();
        let first = text(&graphics.encode(&corners, &Occlusion::default(), CELL));
        assert_eq!(first.matches("a=t,").count(), 4, "four corner images");
        assert_eq!(first.matches("a=p,").count(), 4);
        assert!(first.contains("z=-1,C=1"));
        assert!(
            first.contains("\x1b[1;3H"),
            "top-left placed at row 0, column 2"
        );

        let second = text(&graphics.encode(&corners, &Occlusion::default(), CELL));
        assert_eq!(second.matches("a=t,").count(), 0, "images are reused");
        assert_eq!(second.matches("a=p,").count(), 4, "placements are replayed");
        assert!(!second.contains("a=d,"));
    }

    #[test]
    fn theme_or_cell_size_changes_resend_images_and_delete_old_ones() {
        let corners = rounded_card();
        let mut graphics = ChromeGraphics::default();
        graphics.encode(&corners, &Occlusion::default(), CELL);
        let resized = text(&graphics.encode(
            &corners,
            &Occlusion::default(),
            HostCellSize {
                width_px: 20,
                height_px: 42,
            },
        ));
        assert_eq!(resized.matches("a=t,").count(), 4);
        assert!(resized.contains("s=20,v=42"));
        assert_eq!(resized.matches("a=d,d=I,").count(), 4, "old images deleted");

        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 5));
        let mut recolored = Corners::rounded();
        super::super::shape::card(
            &mut buffer,
            Rect::new(2, 1, 4, 2),
            super::super::shape::Fill::new(Color::Rgb(1, 2, 3), BENEATH),
            &mut recolored,
        );
        let reloaded = text(&graphics.encode(
            &recolored,
            &Occlusion::default(),
            HostCellSize {
                width_px: 20,
                height_px: 42,
            },
        ));
        assert_eq!(
            reloaded.matches("a=t,").count(),
            4,
            "a theme reload uploads new colors"
        );
        assert_eq!(reloaded.matches("a=d,d=I,").count(), 4);
    }

    #[test]
    fn shapes_that_go_away_are_deleted() {
        let mut graphics = ChromeGraphics::default();
        graphics.encode(&rounded_card(), &Occlusion::default(), CELL);
        let gone = text(&graphics.encode(&Corners::rounded(), &Occlusion::default(), CELL));
        assert_eq!(gone.matches("a=d,d=I,").count(), 4);
        assert!(!gone.contains("a=p,"));
        assert!(
            text(&graphics.encode(&Corners::rounded(), &Occlusion::default(), CELL)).is_empty()
        );
    }

    #[test]
    fn covered_corners_are_not_placed() {
        let mut graphics = ChromeGraphics::default();
        let mut occlusion = Occlusion::default();
        occlusion.cover(Rect::new(0, 0, 4, 1));
        let output = text(&graphics.encode(&rounded_card(), &occlusion, CELL));
        assert_eq!(
            output.matches("a=p,").count(),
            3,
            "the covered top-left is skipped"
        );
    }

    #[test]
    fn clear_deletes_every_image() {
        let mut graphics = ChromeGraphics::default();
        graphics.encode(&rounded_card(), &Occlusion::default(), CELL);
        let cleared = text(&graphics.clear());
        assert_eq!(cleared.matches("a=d,d=I,").count(), 4);
        assert!(graphics.images.is_empty());
    }
}

#[cfg(test)]
mod preview {
    use super::*;

    /// Writes raw RGBA previews for a human to look at. Not part of the suite.
    #[test]
    #[ignore = "writes preview images; run by hand"]
    fn write_corner_previews() {
        let dir = std::path::PathBuf::from(
            std::env::var("CHROME_PREVIEW_DIR").unwrap_or_else(|_| "/tmp".into()),
        );
        let (w, h) = (19, 42);
        for (name, kind, radius) in [
            ("top_left", CornerKind::TopLeft, Radius::Card),
            ("top_right", CornerKind::TopRight, Radius::Card),
            ("bottom_left", CornerKind::BottomLeft, Radius::Card),
            ("bottom_right", CornerKind::BottomRight, Radius::Card),
            ("pill_left", CornerKind::PillLeft, Radius::Pill),
            ("pill_right", CornerKind::PillRight, Radius::Pill),
        ] {
            let pixels = corner_rgba(kind, radius, (58, 63, 77), w, h);
            std::fs::write(dir.join(format!("{name}.rgba")), pixels).unwrap();
        }
        let cell = HostCellSize {
            width_px: w,
            height_px: h,
        };
        std::fs::write(
            dir.join("shadow.rgba"),
            shadow_rgba((14, 7), (3, 2, 8, 3), cell),
        )
        .unwrap();
    }
}
