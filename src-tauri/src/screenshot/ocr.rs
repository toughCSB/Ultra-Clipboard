//! Text recognition for screenshot areas with the operating system's OCR engine.

use crate::core::{AppError, Result};

/// Recognizes text in straight RGBA pixels and returns it line by line.
pub fn recognize(width: u32, height: u32, rgba: &[u8]) -> Result<String> {
    platform::recognize(width, height, rgba)
}

fn failure(message: impl std::fmt::Display) -> AppError {
    AppError::Other(anyhow::anyhow!("text recognition failed: {message}"))
}

/// Small captures are enlarged before recognition because the Windows engine
/// misses UI-sized glyphs. Returns the scale for an image of this size.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn upscale_factor(width: u32, height: u32, max_dimension: u32) -> u32 {
    const SMALL_SIDE: u32 = 1200;

    let side = width.max(height);
    if side < SMALL_SIDE && side.saturating_mul(2) <= max_dimension {
        2
    } else {
        1
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::sync::OnceLock;

    use image::imageops::{self, FilterType};
    use image::{ImageBuffer, Rgba};
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    use windows::Win32::System::Com::CoIncrementMTAUsage;

    use super::{failure, upscale_factor};
    use crate::core::Result;

    /// WinRT calls from worker threads need a multithreaded apartment to join.
    fn ensure_apartment() {
        static APARTMENT: OnceLock<bool> = OnceLock::new();

        APARTMENT.get_or_init(|| {
            // SAFETY: CoIncrementMTAUsage has no preconditions; the cookie is kept
            // for the whole process so the apartment never shuts down.
            match unsafe { CoIncrementMTAUsage() } {
                Ok(_) => true,
                Err(err) => {
                    log::warn!("keep multithreaded apartment for OCR failed: {err}");
                    false
                }
            }
        });
    }

    pub fn recognize(width: u32, height: u32, rgba: &[u8]) -> Result<String> {
        ensure_apartment();

        let engine = OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| failure("no OCR language is installed in Windows"))?;
        let max_dimension = OcrEngine::MaxImageDimension().map_err(failure)?;
        let source = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(width, height, rgba)
            .ok_or_else(|| failure("image size does not match its pixels"))?;

        let factor = upscale_factor(width, height, max_dimension);
        let fit = (f64::from(max_dimension) / f64::from(width.max(height))).min(1.0);
        let target_width = ((f64::from(width) * fit).floor() as u32).max(1) * factor;
        let target_height = ((f64::from(height) * fit).floor() as u32).max(1) * factor;
        let mut pixels = if (target_width, target_height) == (width, height) {
            source.into_raw().to_vec()
        } else {
            imageops::resize(&source, target_width, target_height, FilterType::CatmullRom)
                .into_raw()
        };

        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        let writer = DataWriter::new().map_err(failure)?;
        writer.WriteBytes(&pixels).map_err(failure)?;
        let buffer = writer.DetachBuffer().map_err(failure)?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            target_width as i32,
            target_height as i32,
        )
        .map_err(failure)?;
        let result = engine
            .RecognizeAsync(&bitmap)
            .and_then(|operation| operation.get())
            .map_err(failure)?;

        let mut lines = Vec::new();
        for line in result.Lines().map_err(failure)? {
            let text = line.Text().map_err(failure)?.to_string_lossy();
            if !text.trim().is_empty() {
                lines.push(text);
            }
        }

        Ok(lines.join("\n"))
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::failure;
    use crate::core::Result;

    pub fn recognize(_width: u32, _height: u32, _rgba: &[u8]) -> Result<String> {
        Err(failure("not available on this platform yet"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_images_are_enlarged_within_the_engine_limit() {
        assert_eq!(upscale_factor(640, 200, 10_000), 2);
        assert_eq!(upscale_factor(2160, 3840, 10_000), 1);
        assert_eq!(upscale_factor(1000, 10, 1_500), 1);
    }
}
