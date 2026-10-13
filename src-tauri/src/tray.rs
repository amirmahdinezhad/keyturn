//! Transparent monochrome Keyturn mark for the menu bar.
//! macOS template rendering uses its alpha mask, so the app icon's solid
//! square background must never be included in this asset.

use tauri::image::Image;

static ICON: &[u8] = include_bytes!("../icons/tray-template.png");

/// The one and only tray icon. Set once at startup; never redrawn.
pub fn render() -> Image<'static> {
    Image::from_bytes(ICON).expect("bundled tray icon must decode")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_is_a_transparent_glyph_instead_of_an_opaque_tile() {
        let icon = render();
        assert_eq!((icon.width(), icon.height()), (46, 36));
        let pixels: Vec<_> = icon.rgba().chunks_exact(4).collect();
        assert_eq!(pixels[0][3], 0);
        assert_eq!(pixels[pixels.len() - 1][3], 0);
        let visible = pixels.iter().filter(|pixel| pixel[3] > 128).count();
        assert!(visible > pixels.len() / 10, "logo must remain visible");
        assert!(
            visible < pixels.len() / 2,
            "background must stay transparent"
        );
    }
}
