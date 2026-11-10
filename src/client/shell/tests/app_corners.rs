//! App-style rounded corners through the full compose path.

use super::*;
use crate::config::{CornersConfig, UiStyleConfig};

fn state(corners: CornersConfig) -> ClientShellState {
    let mut config = Config::default();
    config.ui.style = UiStyleConfig::App;
    config.ui.corners = corners;
    config.theme.name = Some("crews".to_string());
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.set_graphics_cell_size(10, 21);
    state.set_host_shows_kitty_images(true);
    state
}

fn compose(state: &mut ClientShellState) -> (FrameData, String) {
    state.set_pane_surface(surface());
    let composed = state.compose(177, 48).expect("compose");
    let graphics = String::from_utf8_lossy(&composed.graphics.into_inline_bytes()).into_owned();
    (composed.frame, graphics)
}

fn cell(frame: &FrameData, x: u16, y: u16) -> &crate::protocol::CellData {
    &frame.cells[usize::from(y) * usize::from(frame.width) + usize::from(x)]
}

#[test]
fn auto_corners_place_images_on_a_kitty_terminal() {
    let mut state = state(CornersConfig::Auto);
    let (frame, graphics) = compose(&mut state);
    let app = crate::app::state::Palette::crews().app.unwrap();
    // The panel's top-left corner (column 36, row 0) is left on the chrome.
    let corner = cell(&frame, 36, 0);
    assert_eq!(corner.symbol, " ");
    assert_eq!(corner.bg, crate::protocol::color_to_u32(app.chrome_bg));
    assert!(
        graphics.contains("\x1b[1;37H\x1b_Ga=p,"),
        "panel corner placed"
    );
    assert!(
        graphics.contains("a=t,t=d,f=32,s=10,v=21,"),
        "one-cell corner images"
    );
    assert!(graphics.contains("z=-1,C=1"));
}

#[test]
fn square_corners_and_unknown_cells_use_no_images() {
    let mut square = state(CornersConfig::Square);
    let (frame, graphics) = compose(&mut square);
    assert_eq!(cell(&frame, 36, 0).symbol, "▄");
    assert!(!graphics.contains("a=p,"));

    let mut unknown = state(CornersConfig::Rounded);
    unknown.set_graphics_cell_size(0, 0);
    let (frame, graphics) = compose(&mut unknown);
    assert_eq!(cell(&frame, 36, 0).symbol, "▄");
    assert!(!graphics.contains("a=p,"));
}

#[test]
fn auto_stays_square_on_other_terminals_but_rounded_forces_images() {
    let mut auto = state(CornersConfig::Auto);
    auto.set_host_shows_kitty_images(false);
    assert!(!compose(&mut auto).1.contains("a=p,"));

    let mut forced = state(CornersConfig::Rounded);
    forced.set_host_shows_kitty_images(false);
    assert!(compose(&mut forced).1.contains("a=p,"));
}

#[test]
fn images_upload_once_and_are_removed_when_switching_to_square() {
    let mut state = state(CornersConfig::Auto);
    let (_, first) = compose(&mut state);
    let uploads = first.matches("a=t,").count();
    assert!(uploads > 0);
    let (_, second) = compose(&mut state);
    assert_eq!(second.matches("a=t,").count(), 0, "nothing re-uploaded");

    state.config.corners = CornersConfig::Square;
    let (_, cleared) = compose(&mut state);
    assert_eq!(cleared.matches("a=d,d=I,").count(), uploads);
}

#[test]
fn a_theme_reload_uploads_new_corner_colors() {
    let mut state = state(CornersConfig::Auto);
    let (_, first) = compose(&mut state);
    let uploads = first.matches("a=t,").count();
    state.config.palette = crate::app::state::Palette::catppuccin();
    let (_, reloaded) = compose(&mut state);
    assert_eq!(reloaded.matches("a=t,").count(), uploads);
    assert_eq!(reloaded.matches("a=d,d=I,").count(), uploads);
}
