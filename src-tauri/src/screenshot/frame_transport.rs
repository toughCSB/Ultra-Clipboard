use std::time::Instant;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ColorType, ImageEncoder};

use crate::core::{AppError, Result};

/// Encodes only the overlay transport; the capture session keeps its original RGBA.
pub fn encode_overlay_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("overlay frame dimensions overflow")))?;
    if width == 0 || height == 0 || rgba.len() != expected {
        return Err(AppError::Other(anyhow::anyhow!(
            "overlay frame has invalid RGBA dimensions"
        )));
    }

    let started = Instant::now();
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Sub)
        .write_image(rgba, width, height, ColorType::Rgba8.into())
        .map_err(|err| AppError::Other(anyhow::anyhow!("encode overlay PNG: {err}")))?;
    log::info!(
        "screenshot overlay PNG encoded in {}ms: raw={} bytes, png={} bytes",
        started.elapsed().as_millis(),
        rgba.len(),
        png.len()
    );

    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_transport_preserves_rust_capture_pixels_losslessly() {
        let rgba = [255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 10, 20, 30, 0];
        let png = encode_overlay_png(&rgba, 2, 2).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();

        assert_eq!(decoded.dimensions(), (2, 2));
        assert_eq!(decoded.into_raw(), rgba);
        assert!(encode_overlay_png(&rgba[..15], 2, 2).is_err());
        assert!(encode_overlay_png(&rgba, 0, 2).is_err());
    }
}
