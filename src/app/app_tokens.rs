//! Extra colors for the polished shell (`ui.style = "app"`).
//!
//! Classic rendering never reads these. Dark built-in themes derive them from
//! the theme's terminal background, text, and accent; light themes and the
//! ANSI-only `terminal` theme have none, so app style falls back to classic.

use ratatui::style::Color;

/// Minimum WCAG contrast for any text token on the surface it is drawn on.
pub const MIN_TEXT_CONTRAST: f64 = 4.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppTokens {
    /// Window behind the panel: sidebar, gutters, status bar.
    pub chrome_bg: Color,
    /// Panel fill. Must equal the host terminal background so app content blends in.
    pub pane_bg: Color,
    /// Space and agent cards, off chips, pills.
    pub card_bg: Color,
    /// Any card or button under the mouse.
    pub card_hover_bg: Color,
    /// Focused space card, active tab.
    pub card_focus_bg: Color,
    /// Track behind the tabs.
    pub segment_bg: Color,
    /// Line under the panel header.
    pub hairline: Color,
    /// Rules beside section headings.
    pub rule: Color,
    /// Toasts and popovers.
    pub float_bg: Color,
    /// Hovered popover row.
    pub float_hover_bg: Color,
    /// Divider inside popovers.
    pub float_divider: Color,
    /// Dims everything behind an open menu (blended at 50%).
    pub scrim: Color,
    /// Focused names, header title.
    pub text_strong: Color,
    /// Inactive tabs, off chips, icon buttons.
    pub text_secondary: Color,
    /// Secondary names in cards, the word "idle".
    pub text_muted: Color,
    /// Section headings, key hints, session name.
    pub label: Color,
    /// Toast body text and key hints inside popovers.
    pub toast_body: Color,
    /// Text drawn on an accent fill.
    pub accent_fg: Color,
}

type Rgb = (u8, u8, u8);

const BLACK: Rgb = (0, 0, 0);
const WHITE: Rgb = (255, 255, 255);

impl AppTokens {
    /// Derive tokens for a dark theme from its terminal background, text, and accent.
    ///
    /// Surfaces step away from the background toward the text color, so they keep
    /// the theme's hue. Text tokens are then lifted until they reach
    /// [`MIN_TEXT_CONTRAST`] on every surface they are drawn on.
    pub fn derive_dark(pane_bg: Rgb, text: Color, accent: Color) -> Self {
        let text = rgb(text).unwrap_or((205, 214, 244));
        let accent = rgb(accent).unwrap_or(text);
        let chrome = mix(pane_bg, BLACK, 0.25);
        // Surfaces that carry `text` step back toward the background if a dim
        // theme text color would otherwise lose contrast on them.
        let surface = |t: f64| readable_surface(mix(pane_bg, text, t), pane_bg, text);
        let card = surface(0.015);
        let card_hover = surface(0.06);
        let card_focus = surface(0.11);
        let segment = mix(chrome, pane_bg, 0.4);
        let float = surface(0.09);
        let float_hover = surface(0.17);

        let lift = |fg: Rgb, surfaces: &[Rgb]| ensure_contrast(fg, surfaces, WHITE);
        let text_strong = lift(mix(text, WHITE, 0.6), &[card_focus, float, pane_bg]);
        let text_secondary = lift(
            mix(text, pane_bg, 0.22),
            &[chrome, card, card_hover, segment],
        );
        let text_muted = lift(mix(text, pane_bg, 0.25), &[card, card_hover]);
        let label = lift(mix(text, pane_bg, 0.3), &[chrome, segment, pane_bg]);
        let toast_body = lift(mix(text, pane_bg, 0.08), &[float, float_hover]);
        let accent_fg = ensure_contrast(mix(pane_bg, BLACK, 0.3), &[accent], BLACK);

        Self {
            chrome_bg: color(chrome),
            pane_bg: color(pane_bg),
            card_bg: color(card),
            card_hover_bg: color(card_hover),
            card_focus_bg: color(card_focus),
            segment_bg: color(segment),
            hairline: color(mix(pane_bg, text, 0.08)),
            rule: color(mix(pane_bg, text, 0.065)),
            float_bg: color(float),
            float_hover_bg: color(float_hover),
            float_divider: color(mix(pane_bg, text, 0.2)),
            scrim: color(mix(pane_bg, BLACK, 0.75)),
            text_strong: color(text_strong),
            text_secondary: color(text_secondary),
            text_muted: color(text_muted),
            label: color(label),
            toast_body: color(toast_body),
            accent_fg: color(accent_fg),
        }
    }
}

pub(crate) fn rgb(color: Color) -> Option<Rgb> {
    match color {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

fn color((r, g, b): Rgb) -> Color {
    Color::Rgb(r, g, b)
}

/// Linear blend from `a` (t = 0) to `b` (t = 1).
pub(crate) fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let channel = |x: u8, y: u8| -> u8 {
        let value = f64::from(x) + (f64::from(y) - f64::from(x)) * t;
        value.round().clamp(0.0, 255.0) as u8
    };
    (channel(a.0, b.0), channel(a.1, b.1), channel(a.2, b.2))
}

fn relative_luminance((r, g, b): Rgb) -> f64 {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// WCAG contrast ratio between two colors (1.0 to 21.0).
pub(crate) fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Step `surface` back toward `base` until `text` reaches [`MIN_TEXT_CONTRAST`] on it.
fn readable_surface(surface: Rgb, base: Rgb, text: Rgb) -> Rgb {
    (0..=20)
        .map(|step| mix(surface, base, f64::from(step) * 0.05))
        .find(|candidate| contrast(text, *candidate) >= MIN_TEXT_CONTRAST)
        .unwrap_or(base)
}

/// Step `fg` toward `target` until it reaches [`MIN_TEXT_CONTRAST`] on every surface.
fn ensure_contrast(fg: Rgb, surfaces: &[Rgb], target: Rgb) -> Rgb {
    let passes = |c: Rgb| {
        surfaces
            .iter()
            .all(|surface| contrast(c, *surface) >= MIN_TEXT_CONTRAST)
    };
    (0..=20)
        .map(|step| mix(fg, target, f64::from(step) * 0.05))
        .find(|candidate| passes(*candidate))
        .unwrap_or(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_matches_wcag_reference_values() {
        assert!((contrast(BLACK, WHITE) - 21.0).abs() < 0.01);
        assert!((contrast((0x77, 0x77, 0x77), WHITE) - 4.48).abs() < 0.01);
    }

    #[test]
    fn mix_hits_both_endpoints() {
        assert_eq!(mix((10, 20, 30), (110, 120, 130), 0.0), (10, 20, 30));
        assert_eq!(mix((10, 20, 30), (110, 120, 130), 1.0), (110, 120, 130));
        assert_eq!(mix((0, 0, 0), (100, 100, 100), 0.5), (50, 50, 50));
    }

    #[test]
    fn ensure_contrast_lifts_dim_text() {
        let surface = (0x29, 0x2c, 0x33);
        let lifted = ensure_contrast((0x50, 0x50, 0x50), &[surface], WHITE);
        assert!(contrast(lifted, surface) >= MIN_TEXT_CONTRAST);
    }
}
