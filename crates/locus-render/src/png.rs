//! Optional rasterization of this crate's SVG, with no system fonts/resources.

use crate::{RenderError, RenderOptions, style::StyleSheet};
use locus_core::ResolvedScene;
use locus_layout::LayoutScene;

/// Renders our own SVG to transparent-background PNG bytes at its CSS size.
/// The default build does not include resvg or any raster dependencies.
///
/// # Errors
/// Returns SVG validation failures or E417 for unsupported raster dimensions,
/// allocation failure, or PNG encoding failure. Images are limited to 64M pixels.
pub fn render(
    scene: &ResolvedScene,
    layout: &LayoutScene,
    styles: &StyleSheet,
    options: RenderOptions,
) -> Result<Vec<u8>, RenderError> {
    let svg = crate::svg::render(scene, layout, styles, options)?;
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
        .map_err(|e| raster(e.to_string()))?;
    let size = tree.size().to_int_size();
    if u64::from(size.width()) * u64::from(size.height()) > 64 * 1024 * 1024 {
        return Err(raster("image exceeds 64M pixels"));
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| raster("cannot allocate raster image"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| raster(e.to_string()))
}

fn raster(message: impl Into<String>) -> RenderError {
    RenderError::Raster {
        message: message.into(),
    }
}
