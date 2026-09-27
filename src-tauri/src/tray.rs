//! Static menu-bar icon.
//!
//! Quota rendering (battery/bar/percent styles, pace markers, multi-profile
//! cells) lived here in v2. With CLI-owned auth there are no quotas to draw,
//! so the tray is a fixed icon: the bundled app mark.

use tauri::image::Image;

static ICON: &[u8] = include_bytes!("../icons/32x32.png");

/// The one and only tray icon. Set once at startup; never redrawn.
pub fn render() -> Image<'static> {
    Image::from_bytes(ICON).expect("bundled tray icon must decode")
}
