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
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
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

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;

    use image::imageops::{self, FilterType};
    use image::{ImageBuffer, Rgba};
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary};
    use objc2_vision::{
        VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
        VNRequestTextRecognitionLevel,
    };

    use super::{failure, upscale_factor};
    use crate::core::Result;
    use crate::screenshot::output;

    struct RecognizedLine {
        x: f64,
        y: f64,
        text: String,
    }

    pub fn recognize(width: u32, height: u32, rgba: &[u8]) -> Result<String> {
        let source = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(width, height, rgba)
            .ok_or_else(|| failure("image size does not match its pixels"))?;
        let factor = upscale_factor(width, height, u32::MAX);
        let (target_width, target_height, pixels) = if factor == 1 {
            (width, height, source.into_raw().to_vec())
        } else {
            let resized = imageops::resize(
                &source,
                width.saturating_mul(factor),
                height.saturating_mul(factor),
                FilterType::CatmullRom,
            );
            (resized.width(), resized.height(), resized.into_raw())
        };
        let png = output::encode_png(target_width, target_height, &pixels)?;
        // SAFETY: NSData copies the complete PNG buffer before this call returns.
        let image_data =
            unsafe { NSData::dataWithBytes_length(png.as_ptr().cast::<c_void>(), png.len()) };
        let options: Retained<NSDictionary<VNImageOption, AnyObject>> =
            NSDictionary::from_slices::<VNImageOption>(&[], &[]);
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &image_data,
            &options,
        );
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setAutomaticallyDetectsLanguage(true);
        request.setUsesLanguageCorrection(true);

        let base_request: Retained<VNRequest> = request.clone().into_super().into_super();
        let requests = NSArray::from_retained_slice(&[base_request]);
        handler.performRequests_error(&requests).map_err(failure)?;

        let mut lines = request
            .results()
            .map(|observations| {
                observations
                    .iter()
                    .filter_map(|observation| {
                        let candidate = observation.topCandidates(1).to_vec().into_iter().next()?;
                        let text = candidate.string().to_string();
                        if text.trim().is_empty() {
                            return None;
                        }
                        // SAFETY: Vision returns a finite normalized rectangle for each result.
                        let bounds = unsafe { observation.boundingBox() };

                        Some(RecognizedLine {
                            x: bounds.origin.x,
                            y: bounds.origin.y,
                            text,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        lines.sort_by(|left, right| {
            right
                .y
                .total_cmp(&left.y)
                .then_with(|| left.x.total_cmp(&right.x))
        });

        Ok(lines
            .into_iter()
            .map(|line| line.text)
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
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

    #[cfg(target_os = "macos")]
    #[test]
    fn vision_recognizes_ui_text_fixture() {
        let image = image::load_from_memory(include_bytes!("../../tests/fixtures/vision-ocr.png"))
            .unwrap()
            .to_rgba8();
        let text = recognize(image.width(), image.height(), image.as_raw()).unwrap();

        assert!(text.contains("ULTRA"), "recognized: {text:?}");
        assert!(text.contains("CLIPBOARD"), "recognized: {text:?}");
        assert!(text.contains("123"), "recognized: {text:?}");
    }
}
