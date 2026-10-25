//! Bundled digital window and tray SVGs rasterized at their authored sizes.

use eframe::egui;
use resvg::{tiny_skia, usvg};

/// Side length, in pixels, of the square bitmap the icon is rasterized to.
const ICON_SIZE: u32 = 256;

/// Rasterizes the bundled window SVG.
///
/// # Returns
/// Straight-alpha RGBA pixels, or `None` if SVG parsing or pixel allocation fails.
pub(crate) fn icon() -> Option<egui::IconData> {
    rasterize(include_str!("../assets/icon.svg"), ICON_SIZE)
}

/// Renders the compact digital display used by native tray and menu-bar icons.
///
/// # Returns
/// A 32-by-32 RGBA icon, or `None` if SVG parsing or pixel allocation fails.
pub(crate) fn tray_icon() -> Option<egui::IconData> {
    rasterize(include_str!("../assets/tray.svg"), 32)
}

/// Converts a bundled square SVG to straight-alpha RGBA pixels.
fn rasterize(svg: &str, size: u32) -> Option<egui::IconData> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &options).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;

    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );

    // tiny-skia stores colors with premultiplied alpha; egui expects straight
    // (un-premultiplied) RGBA, so demultiply each pixel as it is copied out.
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for pixel in pixmap.pixels() {
        let color = pixel.demultiply();
        rgba.push(color.red());
        rgba.push(color.green());
        rgba.push(color.blue());
        rgba.push(color.alpha());
    }

    Some(egui::IconData {
        rgba,
        width: size,
        height: size,
    })
}
