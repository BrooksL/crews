//! Window chrome for the polished shell (`ui.style = "app"`): the darker
//! window around the pane panel, the gutters, the panel's caps, and the status
//! bar row. Layout lives in `ClientShellConfig::layout`; this only draws.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::protocol::FrameData;

use super::shape::{self, Corners, Fill};
use super::ClientShellLayout;
use crate::app::app_tokens::AppTokens;
use crate::app::state::Palette;

/// Paint the chrome and the panel's caps before the sidebar and tab bar draw.
///
/// Only cells nothing else covers are painted, because this runs every frame:
/// the sidebar is recolored in [`finish`] and pane content fills the panel
/// below the tab row.
pub(super) fn render_backdrop(
    buffer: &mut Buffer,
    layout: &ClientShellLayout,
    app: &AppTokens,
    corners: &mut Corners,
) {
    let area = buffer.area;
    let panel = layout.panel;
    if layout.sidebar.is_empty() {
        shape::fill_bg(
            buffer,
            Rect::new(0, 0, panel.x, panel.height),
            app.chrome_bg,
        );
    }
    shape::fill_bg(
        buffer,
        Rect::new(
            panel.right(),
            0,
            area.right().saturating_sub(panel.right()),
            panel.height,
        ),
        app.chrome_bg,
    );
    shape::fill_bg(
        buffer,
        Rect::new(
            0,
            panel.bottom(),
            area.width,
            area.bottom().saturating_sub(panel.bottom()),
        ),
        app.chrome_bg,
    );
    shape::panel(
        buffer,
        panel,
        Fill::new(app.pane_bg, app.chrome_bg),
        corners,
    );
    shape::fill_bg(buffer, layout.tab_bar, app.pane_bg);
    for y in layout.panel_padding_rows {
        shape::fill_bg(buffer, Rect::new(panel.x, y, panel.width, 1), app.pane_bg);
    }
    let content = layout.pane_surface;
    shape::fill_bg(
        buffer,
        Rect::new(panel.x, content.y, content.x - panel.x, content.height),
        app.pane_bg,
    );
    shape::fill_bg(
        buffer,
        Rect::new(
            content.right(),
            content.y,
            panel.right().saturating_sub(content.right()),
            content.height,
        ),
        app.pane_bg,
    );
    // The hairline under the header: a thin line along the top of the padding row.
    let hairline = Style::default().fg(app.hairline).bg(app.pane_bg);
    let hairline_y = layout.panel_padding_rows[0];
    for x in panel.left()..panel.right() {
        if let Some(cell) = buffer.cell_mut((x, hairline_y)) {
            cell.set_symbol(HAIRLINE).set_style(hairline);
        }
    }
}

/// Upper one-eighth block: reads as a one-pixel rule at the top of its cell.
const HAIRLINE: &str = "▔";

/// Paint the gaps between split panes as chrome gutters, so each pane reads
/// as its own panel. Gutters at the content's edges reach through the panel
/// padding (and the bottom cap) to the panel's border.
pub(super) fn paint_pane_gutters(
    frame: &mut FrameData,
    layout: &ClientShellLayout,
    panes: &[crate::protocol::PaneSurfacePane],
    app: &AppTokens,
) {
    if panes.len() < 2 {
        return;
    }
    let surface = layout.pane_surface;
    let panel = layout.panel;
    let chrome = crate::protocol::color_to_u32(app.chrome_bg);
    let mut covered: Vec<(u16, u16)> = Vec::with_capacity(panes.len());
    for row in 0..surface.height {
        covered.clear();
        covered.extend(
            panes
                .iter()
                .filter(|pane| row >= pane.rect.y && row < pane.rect.y + pane.rect.height)
                .map(|pane| (pane.rect.x, pane.rect.x + pane.rect.width)),
        );
        covered.sort_unstable();
        let y = surface.y + row;
        if covered.is_empty() {
            // A horizontal gutter: run it out through the side padding.
            paint_chrome(frame, panel.x, panel.right(), y, chrome);
            continue;
        }
        let mut x = 0;
        for &(start, end) in covered
            .iter()
            .chain(std::iter::once(&(surface.width, surface.width)))
        {
            if start > x {
                let (left, right) = (surface.x + x, surface.x + start.min(surface.width));
                paint_chrome(frame, left, right, y, chrome);
                // Vertical gutters at the content's top and bottom run through
                // the padding rows, and through the bottom cap.
                if row == 0 {
                    paint_chrome(frame, left, right, layout.panel_padding_rows[0], chrome);
                }
                if row + 1 == surface.height {
                    paint_chrome(frame, left, right, layout.panel_padding_rows[1], chrome);
                    paint_chrome(frame, left, right, panel.bottom() - 1, chrome);
                }
            }
            x = x.max(end);
        }
    }
}

fn paint_chrome(frame: &mut FrameData, left: u16, right: u16, y: u16, chrome: u32) {
    if y >= frame.height {
        return;
    }
    let row = usize::from(y) * usize::from(frame.width);
    for x in left..right.min(frame.width) {
        let cell = &mut frame.cells[row + usize::from(x)];
        cell.symbol.clear();
        cell.symbol.push(' ');
        cell.fg = 0;
        cell.bg = chrome;
        cell.modifier = 0;
        cell.hyperlink = None;
    }
}

/// Fade unfocused split panes by blending each cell's foreground toward its
/// background. `amount` is 0.0 (no change) to 1.0. Only true-color cells and the
/// default foreground can be blended; indexed colors are left alone.
pub(super) fn dim_unfocused_panes(
    frame: &mut FrameData,
    surface: Rect,
    panes: &[crate::protocol::PaneSurfacePane],
    amount: f32,
    palette: &Palette,
    app: &AppTokens,
) {
    if amount <= 0.0 || panes.len() < 2 {
        return;
    }
    let default_fg = crate::app::app_tokens::rgb(palette.text);
    let default_bg = crate::app::app_tokens::rgb(app.pane_bg);
    for pane in panes.iter().filter(|pane| !pane.focused) {
        for row in pane.rect.y..pane.rect.y + pane.rect.height {
            let y = surface.y + row;
            if y >= frame.height {
                break;
            }
            let start = usize::from(y) * usize::from(frame.width);
            for col in pane.rect.x..pane.rect.x + pane.rect.width {
                let x = surface.x + col;
                if x >= frame.width {
                    break;
                }
                let cell = &mut frame.cells[start + usize::from(x)];
                let fg = match cell.fg {
                    0 => default_fg,
                    value => packed_rgb(value),
                };
                let bg = match cell.bg {
                    0 => default_bg,
                    value => packed_rgb(value),
                };
                if let (Some(fg), Some(bg)) = (fg, bg) {
                    let (r, g, b) = crate::app::app_tokens::mix(fg, bg, f64::from(amount));
                    cell.fg = crate::protocol::color_to_u32(Color::Rgb(r, g, b));
                }
            }
        }
    }
}

/// How far the scrim pulls colors toward `app.scrim`.
pub(super) const SCRIM_AMOUNT: f64 = 0.5;

/// Blend `color` toward the scrim. Colors without an RGB value are returned
/// unchanged.
pub(super) fn scrim_color(color: Color, app: &AppTokens) -> Color {
    match (
        crate::app::app_tokens::rgb(color),
        crate::app::app_tokens::rgb(app.scrim),
    ) {
        (Some(color), Some(scrim)) => {
            let (r, g, b) = crate::app::app_tokens::mix(color, scrim, SCRIM_AMOUNT);
            Color::Rgb(r, g, b)
        }
        _ => color,
    }
}

/// Fade everything already drawn behind an open menu: blend each cell's
/// foreground and background halfway toward `app.scrim`. Default colors use
/// the theme text and pane background. Text in a color that can't be blended
/// (indexed or named) is dimmed instead.
pub(super) fn apply_scrim(frame: &mut FrameData, palette: &Palette, app: &AppTokens) {
    let Some(scrim) = crate::app::app_tokens::rgb(app.scrim) else {
        return;
    };
    let blend = |color: (u8, u8, u8)| {
        let (r, g, b) = crate::app::app_tokens::mix(color, scrim, SCRIM_AMOUNT);
        crate::protocol::color_to_u32(Color::Rgb(r, g, b))
    };
    let default_fg = crate::app::app_tokens::rgb(palette.text).map(blend);
    let default_bg = crate::app::app_tokens::rgb(app.pane_bg).map(blend);
    let dim = ratatui::style::Modifier::DIM.bits();
    // Most cells share a handful of colors; remember the last blend.
    let mut last = (u32::MAX, 0u32);
    let mut blend_packed = |value: u32| -> Option<u32> {
        if value == last.0 {
            return Some(last.1);
        }
        let blended = packed_rgb(value).map(blend)?;
        last = (value, blended);
        Some(blended)
    };
    for cell in &mut frame.cells {
        match cell.fg {
            0 => {
                if let Some(fg) = default_fg {
                    cell.fg = fg;
                }
            }
            value => match blend_packed(value) {
                Some(fg) => cell.fg = fg,
                None => cell.modifier |= dim,
            },
        }
        match cell.bg {
            0 => {
                if let Some(bg) = default_bg {
                    cell.bg = bg;
                }
            }
            value => {
                if let Some(bg) = blend_packed(value) {
                    cell.bg = bg;
                }
            }
        }
    }
}

fn packed_rgb(value: u32) -> Option<(u8, u8, u8)> {
    (value >> 24 == 0x02).then_some(((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

/// Fit what the classic (collapsed or multi-machine) sidebar drew into the app
/// chrome: its unset or classic backgrounds take the chrome color, and its
/// divider line becomes a plain gutter.
pub(super) fn finish(
    buffer: &mut Buffer,
    layout: &ClientShellLayout,
    palette: &Palette,
    app: &AppTokens,
) {
    shape::recolor_bg(
        buffer,
        layout.sidebar,
        &[Color::Reset, palette.sidebar_bg],
        app.chrome_bg,
    );
    if layout.sidebar.width > 0 {
        let gutter_x = layout.sidebar.right() - 1;
        shape::clear(
            buffer,
            Rect::new(gutter_x, 0, 1, layout.sidebar.height),
            app.chrome_bg,
        );
    }
}

/// Move the classic mode bar's background onto the status bar's chrome.
pub(super) fn finish_status_bar(
    buffer: &mut Buffer,
    status_bar: Rect,
    palette: &Palette,
    app: &AppTokens,
) {
    shape::recolor_bg(
        buffer,
        status_bar,
        &[Color::Reset, palette.panel_bg],
        app.chrome_bg,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, UiStyleConfig};

    fn app_config() -> super::super::ClientShellConfig {
        let mut config = Config::default();
        config.ui.style = UiStyleConfig::App;
        config.theme.name = Some("crews".to_string());
        super::super::ClientShellConfig::from_config(&config)
    }

    fn classic_config() -> super::super::ClientShellConfig {
        super::super::ClientShellConfig::from_config(&Config::default())
    }

    const SIZES: [(u16, u16); 3] = [(177, 48), (270, 64), (100, 30)];

    #[test]
    fn app_layout_places_regions_on_the_spec_grid() {
        let config = app_config();
        for (cols, rows) in SIZES {
            let layout = config.layout(cols, rows, false, 1, 36);
            assert_eq!(
                layout.sidebar,
                Rect::new(0, 0, 36, rows - 1),
                "{cols}x{rows}"
            );
            assert_eq!(
                layout.panel,
                Rect::new(36, 0, cols - 37, rows - 1),
                "panel spans column 36 to W-2 with caps on rows 0 and H-2 at {cols}x{rows}"
            );
            assert_eq!(layout.tab_bar, Rect::new(36, 1, cols - 37, 1));
            assert_eq!(
                layout.pane_surface,
                Rect::new(38, 3, cols - 41, rows - 6),
                "content is inset 2 columns and fills rows 3 to H-4 at {cols}x{rows}"
            );
            assert_eq!(layout.panel_padding_rows, [2, rows - 3]);
            assert_eq!(layout.status_bar, Rect::new(1, rows - 1, cols - 2, 1));
            assert!(layout.mobile_header.is_empty());
        }
    }

    #[test]
    fn app_style_defaults_the_sidebar_to_36_columns() {
        assert_eq!(app_config().sidebar_width, 36);
        assert_eq!(classic_config().sidebar_width, 26);

        let mut config = Config::default();
        config.ui.style = UiStyleConfig::App;
        config.ui.sidebar_width = 30;
        assert_eq!(
            super::super::ClientShellConfig::from_config(&config).sidebar_width,
            30,
            "an explicit width wins"
        );
    }

    #[test]
    fn hidden_sidebar_leaves_a_one_column_left_gutter() {
        let mut config = app_config();
        config.sidebar_collapsed_mode = crate::config::SidebarCollapsedModeConfig::Hidden;
        let layout = config.layout(120, 40, true, 1, 36);
        assert!(layout.sidebar.is_empty());
        assert_eq!(layout.panel.x, 1);
        assert_eq!(layout.panel.right(), 119);
    }

    #[test]
    fn reported_pane_size_matches_the_panel_content() {
        let config = app_config();
        let surface = config.initial_surface_size(177, 48);
        assert_eq!((surface.cols, surface.rows), (177 - 41, 48 - 6));
    }

    #[test]
    fn classic_layout_is_unchanged() {
        let config = classic_config();
        for (cols, rows) in SIZES {
            let layout = config.layout(cols, rows, false, 1, 26);
            assert_eq!(layout.sidebar, Rect::new(0, 0, 26, rows));
            assert_eq!(layout.tab_bar, Rect::new(26, 0, cols - 26, 1));
            assert_eq!(layout.pane_surface, Rect::new(26, 1, cols - 26, rows - 1));
            assert!(layout.panel.is_empty());
            assert!(layout.status_bar.is_empty());
        }
    }

    #[test]
    fn app_style_on_a_light_theme_keeps_the_classic_layout() {
        let mut config = Config::default();
        config.ui.style = UiStyleConfig::App;
        config.theme.name = Some("catppuccin-latte".to_string());
        assert!(super::super::app_style_diagnostic(&config)
            .is_some_and(|message| message.contains("catppuccin-latte")));
        let shell = super::super::ClientShellConfig::from_config(&config);
        assert!(shell.layout(177, 48, false, 1, 36).panel.is_empty());
    }

    #[test]
    fn small_windows_and_mobile_widths_keep_the_classic_layout() {
        let config = app_config();
        assert!(config.layout(100, 6, false, 1, 36).panel.is_empty());
        assert!(config.layout(50, 30, false, 1, 36).panel.is_empty());
    }

    #[test]
    fn backdrop_and_finish_draw_chrome_caps_and_gutters() {
        let config = app_config();
        let palette = &config.palette;
        let app = config.app_tokens().copied().unwrap();
        for (cols, rows) in SIZES {
            let layout = config.layout(cols, rows, false, 1, 36);
            let mut buffer = Buffer::empty(Rect::new(0, 0, cols, rows));
            render_backdrop(&mut buffer, &layout, &app, &mut Corners::square());
            // Stand-in for the classic sidebar: unset background and a divider.
            buffer[(35, 5)].set_symbol("│");
            finish(&mut buffer, &layout, palette, &app);

            for x in 36..cols - 1 {
                let top = &buffer[(x, 0)];
                assert_eq!(
                    (top.symbol(), top.fg, top.bg),
                    ("▄", app.pane_bg, app.chrome_bg)
                );
                let bottom = &buffer[(x, rows - 2)];
                assert_eq!(
                    (bottom.symbol(), bottom.fg, bottom.bg),
                    ("▀", app.pane_bg, app.chrome_bg)
                );
                assert_eq!(buffer[(x, 1)].bg, app.pane_bg, "tab row sits on the panel");
            }
            for y in 0..rows {
                assert_eq!(buffer[(35, y)].bg, app.chrome_bg, "left gutter row {y}");
                assert_eq!(
                    buffer[(cols - 1, y)].bg,
                    app.chrome_bg,
                    "right gutter row {y}"
                );
            }
            assert_eq!(
                buffer[(35, 5)].symbol(),
                " ",
                "divider line becomes a gutter"
            );
            for x in 0..cols {
                assert_eq!(
                    buffer[(x, rows - 1)].bg,
                    app.chrome_bg,
                    "status bar column {x}"
                );
            }
            assert_eq!(buffer[(10, 10)].bg, app.chrome_bg, "sidebar sits on chrome");
        }
    }

    fn pane(
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        focused: bool,
    ) -> crate::protocol::PaneSurfacePane {
        let rect = crate::protocol::SurfaceRect {
            x,
            y,
            width,
            height,
        };
        crate::protocol::PaneSurfacePane {
            pane_id: format!("pane_{x}_{y}"),
            content_revision: 0,
            rect,
            inner_rect: rect,
            scrollbar_rect: None,
            scroll: None,
            focused,
            mouse_reporting: false,
            sgr_pixel_mouse: false,
            alternate_screen_active: false,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    fn frame(cols: u16, rows: u16) -> FrameData {
        FrameData::from_ratatui_buffer(&Buffer::empty(Rect::new(0, 0, cols, rows)), None)
    }

    fn frame_cell(frame: &FrameData, x: u16, y: u16) -> &crate::protocol::CellData {
        &frame.cells[usize::from(y) * usize::from(frame.width) + usize::from(x)]
    }

    #[test]
    fn padding_and_hairline_sit_on_the_panel() {
        let config = app_config();
        let app = config.app_tokens().copied().unwrap();
        let layout = config.layout(177, 48, false, 1, 36);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 177, 48));
        render_backdrop(&mut buffer, &layout, &app, &mut Corners::square());
        for x in 36..176 {
            let hairline = &buffer[(x, 2)];
            assert_eq!(
                (hairline.symbol(), hairline.fg, hairline.bg),
                ("▔", app.hairline, app.pane_bg),
                "hairline at column {x}"
            );
            assert_eq!(buffer[(x, 45)].bg, app.pane_bg, "bottom padding row");
        }
        for y in 3..44 {
            for x in [36, 37, 174, 175] {
                assert_eq!(buffer[(x, y)].bg, app.pane_bg, "side padding at {x},{y}");
            }
        }
    }

    #[test]
    fn side_by_side_panes_get_a_full_height_gutter() {
        let config = app_config();
        let app = config.app_tokens().copied().unwrap();
        let layout = config.layout(177, 48, false, 1, 36);
        let surface = layout.pane_surface;
        let mut frame = frame(177, 48);
        // Two panes with the server's one-column gap at surface column 60.
        let panes = [
            pane(0, 0, 60, surface.height, true),
            pane(61, 0, surface.width - 61, surface.height, false),
        ];
        paint_pane_gutters(&mut frame, &layout, &panes, &app);
        let chrome = crate::protocol::color_to_u32(app.chrome_bg);
        let gutter_x = surface.x + 60;
        for y in [2, surface.y, surface.bottom() - 1, 45, 46] {
            assert_eq!(frame_cell(&frame, gutter_x, y).bg, chrome, "gutter row {y}");
        }
        assert_ne!(frame_cell(&frame, gutter_x - 1, surface.y).bg, chrome);
        assert_ne!(
            frame_cell(&frame, gutter_x, 1).bg,
            chrome,
            "the header stays whole"
        );
    }

    #[test]
    fn stacked_panes_get_a_gutter_across_the_panel() {
        let config = app_config();
        let app = config.app_tokens().copied().unwrap();
        let layout = config.layout(177, 48, false, 1, 36);
        let surface = layout.pane_surface;
        let mut frame = frame(177, 48);
        let panes = [
            pane(0, 0, surface.width, 20, true),
            pane(0, 21, surface.width, surface.height - 21, false),
        ];
        paint_pane_gutters(&mut frame, &layout, &panes, &app);
        let chrome = crate::protocol::color_to_u32(app.chrome_bg);
        let y = surface.y + 20;
        for x in [36, 37, surface.x, surface.right() - 1, 174, 175] {
            assert_eq!(frame_cell(&frame, x, y).bg, chrome, "gutter column {x}");
        }
    }

    #[test]
    fn a_single_pane_gets_no_gutter() {
        let config = app_config();
        let app = config.app_tokens().copied().unwrap();
        let layout = config.layout(177, 48, false, 1, 36);
        let mut frame = frame(177, 48);
        let before = frame.cells.clone();
        let surface = layout.pane_surface;
        paint_pane_gutters(
            &mut frame,
            &layout,
            &[pane(0, 0, surface.width, surface.height, true)],
            &app,
        );
        assert_eq!(frame.cells, before);
    }

    #[test]
    fn dimming_fades_only_unfocused_panes() {
        let config = app_config();
        let app = config.app_tokens().copied().unwrap();
        let surface = Rect::new(0, 0, 20, 4);
        let mut frame = frame(20, 4);
        let white = crate::protocol::color_to_u32(Color::Rgb(255, 255, 255));
        for cell in &mut frame.cells {
            cell.fg = white;
        }
        let panes = [pane(0, 0, 10, 4, true), pane(10, 0, 10, 4, false)];
        dim_unfocused_panes(&mut frame, surface, &panes, 0.0, &config.palette, &app);
        assert!(
            frame.cells.iter().all(|cell| cell.fg == white),
            "0.0 is off"
        );

        dim_unfocused_panes(&mut frame, surface, &panes, 0.5, &config.palette, &app);
        assert_eq!(frame_cell(&frame, 2, 1).fg, white, "focused pane untouched");
        let (r, g, b) = crate::app::app_tokens::mix((255, 255, 255), (41, 44, 51), 0.5);
        assert_eq!(
            frame_cell(&frame, 12, 1).fg,
            crate::protocol::color_to_u32(Color::Rgb(r, g, b))
        );
    }

    #[test]
    fn dim_unfocused_defaults_off_and_is_clamped() {
        assert_eq!(app_config().dim_unfocused, 0.0);
        let mut config = Config::default();
        config.ui.dim_unfocused = 3.0;
        assert_eq!(
            super::super::ClientShellConfig::from_config(&config).dim_unfocused,
            1.0
        );
        config.ui.dim_unfocused = f32::NAN;
        assert_eq!(
            super::super::ClientShellConfig::from_config(&config).dim_unfocused,
            0.0
        );
    }

    #[test]
    fn app_style_turns_server_pane_borders_off_and_keeps_gaps() {
        let mut config = Config::default();
        config.ui.pane_borders = crate::config::PaneBordersConfig::Always;
        config.ui.pane_gaps = false;
        assert_eq!(
            config.ui.effective_pane_borders(),
            crate::config::PaneBordersConfig::Always
        );
        assert!(!config.ui.effective_pane_gaps());
        config.ui.style = UiStyleConfig::App;
        assert_eq!(
            config.ui.effective_pane_borders(),
            crate::config::PaneBordersConfig::Off
        );
        assert!(config.ui.effective_pane_gaps());
    }

    /// Per-frame cost of the app chrome. It runs once per frame, not per pane.
    #[test]
    #[ignore = "profile; run with just bench-render-scale"]
    fn app_chrome_render_scale_profile() {
        let config = app_config();
        let app = config.app_tokens().copied().unwrap();
        for (cols, rows) in [(177, 48), (270, 64)] {
            let layout = config.layout(cols, rows, false, 1, 36);
            let mut buffer = Buffer::empty(Rect::new(0, 0, cols, rows));
            let mut samples = Vec::with_capacity(500);
            for _ in 0..500 {
                let started = std::time::Instant::now();
                render_backdrop(&mut buffer, &layout, &app, &mut Corners::square());
                finish(&mut buffer, &layout, &config.palette, &app);
                finish_status_bar(&mut buffer, layout.status_bar, &config.palette, &app);
                std::hint::black_box(&buffer);
                samples.push(started.elapsed().as_nanos());
            }
            samples.sort_unstable();
            eprintln!(
                "app-chrome {cols}x{rows} median_us={:.1} p95_us={:.1}",
                samples[250] as f64 / 1000.0,
                samples[475] as f64 / 1000.0
            );
        }

        // Gutters and dimming over split panes: a grid of 2 or 15 panes.
        let layout = config.layout(270, 64, false, 1, 36);
        let surface = layout.pane_surface;
        for (columns, grid_rows) in [(2u16, 1u16), (5, 3)] {
            let width = (surface.width - (columns - 1)) / columns;
            let height = (surface.height - (grid_rows - 1)) / grid_rows;
            let panes = (0..grid_rows)
                .flat_map(|row| {
                    (0..columns).map(move |column| {
                        pane(
                            column * (width + 1),
                            row * (height + 1),
                            width,
                            height,
                            row == 0 && column == 0,
                        )
                    })
                })
                .collect::<Vec<_>>();
            for (label, dim) in [("gutters", 0.0), ("gutters+dim", 0.2)] {
                let mut frame = frame(270, 64);
                let mut samples = Vec::with_capacity(500);
                for _ in 0..500 {
                    let started = std::time::Instant::now();
                    paint_pane_gutters(&mut frame, &layout, &panes, &app);
                    dim_unfocused_panes(&mut frame, surface, &panes, dim, &config.palette, &app);
                    std::hint::black_box(&frame);
                    samples.push(started.elapsed().as_nanos());
                }
                samples.sort_unstable();
                eprintln!(
                    "app-chrome {label} panes={} median_us={:.1} p95_us={:.1}",
                    panes.len(),
                    samples[250] as f64 / 1000.0,
                    samples[475] as f64 / 1000.0
                );
            }
        }
    }
}
