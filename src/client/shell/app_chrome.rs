//! Window chrome for the polished shell (`ui.style = "app"`): the darker
//! window around the pane panel, the gutters, the panel's caps, and the status
//! bar row. Layout lives in `ClientShellConfig::layout`; this only draws.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::shape::{self, Corners, Fill};
use super::ClientShellLayout;
use crate::app::app_tokens::AppTokens;
use crate::app::state::Palette;

/// Paint the chrome and the panel's caps before the sidebar and tab bar draw.
///
/// Only cells nothing else covers are painted, because this runs every frame:
/// the sidebar is recolored in [`finish`] and pane content fills the panel
/// below the tab row.
pub(super) fn render_backdrop(buffer: &mut Buffer, layout: &ClientShellLayout, app: &AppTokens) {
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
        Corners::Square,
    );
    shape::fill_bg(buffer, layout.tab_bar, app.pane_bg);
}

/// Fit what the classic sidebar and tab bar drew into the app chrome: their
/// unset or classic backgrounds take the chrome and panel colors, and the
/// sidebar's divider line becomes a plain gutter.
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
    shape::recolor_bg(
        buffer,
        layout.tab_bar,
        &[Color::Reset, palette.panel_bg],
        app.pane_bg,
    );
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
                Rect::new(36, 2, cols - 37, rows - 4),
                "content fills rows 2 to H-3 at {cols}x{rows}"
            );
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
        assert_eq!((surface.cols, surface.rows), (177 - 37, 48 - 4));
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
            render_backdrop(&mut buffer, &layout, &app);
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
                render_backdrop(&mut buffer, &layout, &app);
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
    }
}
