//! Optional rasterization of this crate's SVG, with no system fonts/resources.

use crate::{RenderError, RenderOptions, style::StyleSheet};
use locus_core::ResolvedScene;
use locus_layout::LayoutScene;

/// Renders our own SVG to PNG bytes at 96 DPI, including the optional background.
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
    render_with_dpi(scene, layout, styles, options, 96)
}

/// Renders PNG at the requested DPI, scaling from 96 CSS pixels per inch.
/// Records the density in PNG metadata so the physical size stays the same.
///
/// # Errors
/// Returns SVG validation failures or E417 for zero/unrepresentable DPI,
/// unsupported raster dimensions, allocation failure, or PNG encoding failure.
/// Images are limited to 64M pixels after scaling.
pub fn render_with_dpi(
    scene: &ResolvedScene,
    layout: &LayoutScene,
    styles: &StyleSheet,
    options: RenderOptions,
    dpi: u32,
) -> Result<Vec<u8>, RenderError> {
    let pixels_per_meter = density(dpi)?;
    let svg = crate::svg::render(scene, layout, styles, options)?;
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
        .map_err(|e| raster(e.to_string()))?;
    let scale = f64::from(dpi) / 96.0;
    let width = dimension(f64::from(tree.size().width()) * scale)?;
    let height = dimension(f64::from(tree.size().height()) * scale)?;
    if u64::from(width) * u64::from(height) > 64 * 1024 * 1024 {
        return Err(raster("image exceeds 64M pixels"));
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| raster("cannot allocate raster image"))?;
    // Raster dimensions round fractional CSS sizes; fill even the partial edge pixels.
    if let Some(background) = options.background.filter(|color| color.3 == 255) {
        pixmap.fill(resvg::tiny_skia::Color::from_rgba8(
            background.0,
            background.1,
            background.2,
            255,
        ));
    }
    // The rasterizer uses f32 transforms; DPI and dimensions were checked above.
    #[allow(clippy::cast_possible_truncation)]
    let scale = scale as f32;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    encode(&pixmap, pixels_per_meter).map_err(|e| raster(e.to_string()))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn density(dpi: u32) -> Result<u32, RenderError> {
    let pixels_per_meter = libm::round(f64::from(dpi) / 0.0254);
    if dpi == 0 || pixels_per_meter > f64::from(u32::MAX) {
        return Err(raster(
            "DPI must be positive and representable in PNG metadata",
        ));
    }
    Ok(pixels_per_meter as u32)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn dimension(value: f64) -> Result<u32, RenderError> {
    let rounded = libm::round(value).max(1.0);
    if !value.is_finite() || rounded > f64::from(u32::MAX) {
        return Err(raster("unsupported raster dimensions"));
    }
    Ok(rounded as u32)
}

fn encode(
    pixmap: &resvg::tiny_skia::Pixmap,
    pixels_per_meter: u32,
) -> Result<Vec<u8>, ::png::EncodingError> {
    // PNG stores straight alpha; tiny-skia stores premultiplied alpha.
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    let mut bytes = Vec::new();
    {
        let mut encoder = ::png::Encoder::new(&mut bytes, pixmap.width(), pixmap.height());
        encoder.set_color(::png::ColorType::Rgba);
        encoder.set_depth(::png::BitDepth::Eight);
        encoder.set_pixel_dims(Some(::png::PixelDimensions {
            xppu: pixels_per_meter,
            yppu: pixels_per_meter,
            unit: ::png::Unit::Meter,
        }));
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&rgba)?;
    }
    Ok(bytes)
}

fn raster(message: impl Into<String>) -> RenderError {
    RenderError::Raster {
        message: message.into(),
    }
}
