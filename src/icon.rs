//! Window icon: rasterizes the bundled SVG into pixels the windowing system can display.
//!
//! The SVG at `assets/icon.svg` is the single source of truth. It is compiled
//! into the binary with [`include_str!`] and rendered at startup, so editing the
//! SVG also updates the window / taskbar / dock icon with no extra build step.

use eframe::egui;
use resvg::{tiny_skia, usvg};

/// Side length, in pixels, of the square bitmap the icon is rasterized to.
const ICON_SIZE: u32 = 256;

/// Builds the application icon as raw RGBA pixels for the windowing system.
///
/// The bundled `assets/icon.svg` is parsed and rendered to an `ICON_SIZE`-square
/// bitmap. Rendering is best-effort: if the SVG fails to parse or the pixel
/// buffer cannot be allocated, this returns `None` and the platform falls back
/// to its default window icon rather than failing to launch.
///
/// # Returns
///
/// `Some(IconData)` holding un-premultiplied RGBA pixels (row-major, top to
/// bottom) on success, or `None` if the icon could not be produced.
pub(crate) fn icon() -> Option<egui::IconData> {
    let svg = include_str!("../assets/icon.svg");

    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &options).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(ICON_SIZE, ICON_SIZE)?;

    // The SVG is authored at ICON_SIZE square, so an identity transform maps its
    // user units one-to-one onto the pixmap and fills it exactly.
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );

    // tiny-skia stores colors with premultiplied alpha; egui expects straight
    // (un-premultiplied) RGBA, so demultiply each pixel as it is copied out.
    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for pixel in pixmap.pixels() {
        let color = pixel.demultiply();
        rgba.push(color.red());
        rgba.push(color.green());
        rgba.push(color.blue());
        rgba.push(color.alpha());
    }

    Some(egui::IconData {
        rgba,
        width: ICON_SIZE,
        height: ICON_SIZE,
    })
}
