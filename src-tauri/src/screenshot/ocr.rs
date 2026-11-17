//! Text recognition for screenshot areas with the operating system's OCR engine.

use crate::core::{AppError, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRegion {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub line_index: usize,
}

pub struct Recognition {
    pub text: String,
    pub regions: Vec<TextRegion>,
}

/// Recognizes text in straight RGBA pixels and returns it line by line.
pub fn recognize(width: u32, height: u32, rgba: &[u8]) -> Result<String> {
    platform::recognize(width, height, rgba)
}

pub fn recognize_regions(width: u32, height: u32, rgba: &[u8]) -> Result<Recognition> {
    platform::recognize_regions(width, height, rgba)
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

/// Converts an engine rectangle to the exported RGBA image's top-left pixels.
fn image_rect(
    (x, y, width, height): (f64, f64, f64, f64),
    (source_width, source_height): (u32, u32),
    (engine_width, engine_height): (f64, f64),
    bottom_left: bool,
) -> Option<(f64, f64, f64, f64)> {
    if ![x, y, width, height, engine_width, engine_height]
        .iter()
        .all(|value| value.is_finite())
        || width <= 0.0
        || height <= 0.0
        || engine_width <= 0.0
        || engine_height <= 0.0
    {
        return None;
    }
    let scale_x = f64::from(source_width) / engine_width;
    let scale_y = f64::from(source_height) / engine_height;
    let left = (x * scale_x).clamp(0.0, f64::from(source_width));
    let right = ((x + width) * scale_x).clamp(0.0, f64::from(source_width));
    let top_engine = if bottom_left {
        engine_height - y - height
    } else {
        y
    };
    let top = (top_engine * scale_y).clamp(0.0, f64::from(source_height));
    let bottom = ((top_engine + height) * scale_y).clamp(0.0, f64::from(source_height));
    (right > left && bottom > top).then_some((left, top, right - left, bottom - top))
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

    use super::{failure, image_rect, upscale_factor, Recognition, TextRegion};
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
        Ok(recognize_inner(width, height, rgba, false)?.text)
    }

    pub fn recognize_regions(width: u32, height: u32, rgba: &[u8]) -> Result<Recognition> {
        recognize_inner(width, height, rgba, true)
    }

    fn recognize_inner(
        width: u32,
        height: u32,
        rgba: &[u8],
        include_regions: bool,
    ) -> Result<Recognition> {
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
        let mut regions = Vec::new();
        for line in result.Lines().map_err(failure)? {
            let text = line.Text().map_err(failure)?.to_string_lossy();
            if !text.trim().is_empty() {
                let line_index = lines.len();
                let mut words = Vec::<TextRegion>::new();
                let mut cursor = 0;
                if include_regions {
                    if let Ok(ocr_words) = line.Words() {
                        for word in ocr_words {
                            let Ok(word_text) = word.Text() else {
                                continue;
                            };
                            let word_text = word_text.to_string_lossy();
                            if word_text.is_empty() {
                                continue;
                            }
                            let Ok(bounds) = word.BoundingRect() else {
                                continue;
                            };
                            let Some((x, y, width, height)) = image_rect(
                                (
                                    f64::from(bounds.X),
                                    f64::from(bounds.Y),
                                    f64::from(bounds.Width),
                                    f64::from(bounds.Height),
                                ),
                                (width, height),
                                (f64::from(target_width), f64::from(target_height)),
                                false,
                            ) else {
                                continue;
                            };
                            let mut region_text = word_text;
                            let word_len = region_text.len();
                            if let Some(relative) = text[cursor..].find(&region_text) {
                                let start = cursor + relative;
                                let separator = &text[cursor..start];
                                if let Some(previous) = words.last_mut() {
                                    previous.text.push_str(separator);
                                } else {
                                    region_text.insert_str(0, separator);
                                }
                                cursor = start + word_len;
                            } else {
                                if let Some(previous) = words.last_mut() {
                                    previous.text.push(' ');
                                }
                                cursor = text.len();
                            }
                            words.push(TextRegion {
                                text: region_text,
                                x,
                                y,
                                width,
                                height,
                                line_index,
                            });
                        }
                    }
                }
                if let Some(last) = words.last_mut() {
                    last.text.push_str(&text[cursor..]);
                }
                regions.extend(words);
                lines.push(text);
            }
        }

        Ok(Recognition {
            text: lines.join("\n"),
            regions,
        })
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;

    use image::imageops::{self, FilterType};
    use image::{ImageBuffer, Rgba};
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, AnyThread};
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSError, NSRange};
    use objc2_vision::{
        VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRectangleObservation,
        VNRequest, VNRequestTextRecognitionLevel,
    };

    use super::{failure, image_rect, upscale_factor, Recognition, TextRegion};
    use crate::core::Result;
    use crate::screenshot::output;

    struct RecognizedLine {
        x: f64,
        y: f64,
        text: String,
        regions: Vec<TextRegion>,
    }

    pub fn recognize(width: u32, height: u32, rgba: &[u8]) -> Result<String> {
        Ok(recognize_inner(width, height, rgba, false)?.text)
    }

    pub fn recognize_regions(width: u32, height: u32, rgba: &[u8]) -> Result<Recognition> {
        recognize_inner(width, height, rgba, true)
    }

    fn recognize_inner(
        width: u32,
        height: u32,
        rgba: &[u8],
        include_regions: bool,
    ) -> Result<Recognition> {
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
                        let mut regions = Vec::<TextRegion>::new();
                        if include_regions {
                            let mut utf16_offset = 0;
                            let mut prefix = String::new();
                            for character in text.chars() {
                                let char_len = character.len_utf16();
                                let range = NSRange::new(utf16_offset, char_len);
                                utf16_offset += char_len;
                                if character.is_whitespace() {
                                    if let Some(last) = regions.last_mut() {
                                        last.text.push(character);
                                    } else {
                                        prefix.push(character);
                                    }
                                    continue;
                                }
                                // SAFETY: Vision's documented selector takes NSRange and NSError**;
                                // objc2-vision 0.3.2 skips this binding, so msg_send retains the result.
                                let word_box: std::result::Result<
                                    Retained<VNRectangleObservation>,
                                    Retained<NSError>,
                                > = unsafe {
                                    msg_send![&*candidate, boundingBoxForRange: range, error: _]
                                };
                                let region = word_box.ok().and_then(|word_box| {
                                    // SAFETY: Vision returns normalized bounds for this observation.
                                    let box_rect = unsafe { word_box.boundingBox() };
                                    image_rect(
                                        (
                                            box_rect.origin.x,
                                            box_rect.origin.y,
                                            box_rect.size.width,
                                            box_rect.size.height,
                                        ),
                                        (width, height),
                                        (1.0, 1.0),
                                        true,
                                    )
                                });
                                let Some((x, y, word_width, word_height)) = region else {
                                    prefix.push(character);
                                    continue;
                                };
                                if let Some(last) = regions.last_mut() {
                                    let same_word = (last.x - x).abs() < 0.25
                                        && (last.y - y).abs() < 0.25
                                        && (last.width - word_width).abs() < 0.25
                                        && (last.height - word_height).abs() < 0.25;
                                    if same_word {
                                        last.text.push_str(&prefix);
                                        last.text.push(character);
                                        prefix.clear();
                                        continue;
                                    }
                                }
                                let mut region_text = std::mem::take(&mut prefix);
                                region_text.push(character);
                                regions.push(TextRegion {
                                    text: region_text,
                                    x,
                                    y,
                                    width: word_width,
                                    height: word_height,
                                    line_index: 0,
                                });
                            }
                            if let Some(last) = regions.last_mut() {
                                last.text.push_str(&prefix);
                            } else if let Some((x, y, word_width, word_height)) = image_rect(
                                (
                                    bounds.origin.x,
                                    bounds.origin.y,
                                    bounds.size.width,
                                    bounds.size.height,
                                ),
                                (width, height),
                                (1.0, 1.0),
                                true,
                            ) {
                                regions.push(TextRegion {
                                    text: text.clone(),
                                    x,
                                    y,
                                    width: word_width,
                                    height: word_height,
                                    line_index: 0,
                                });
                            }
                        }

                        Some(RecognizedLine {
                            x: bounds.origin.x,
                            y: bounds.origin.y,
                            text,
                            regions,
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

        let mut text_lines = Vec::with_capacity(lines.len());
        let mut regions = Vec::new();
        for (line_index, mut line) in lines.into_iter().enumerate() {
            for region in &mut line.regions {
                region.line_index = line_index;
            }
            regions.extend(line.regions);
            text_lines.push(line.text);
        }
        Ok(Recognition {
            text: text_lines.join("\n"),
            regions,
        })
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use super::{failure, Recognition};
    use crate::core::Result;

    pub fn recognize(_width: u32, _height: u32, _rgba: &[u8]) -> Result<String> {
        Err(failure("not available on this platform yet"))
    }

    pub fn recognize_regions(_width: u32, _height: u32, _rgba: &[u8]) -> Result<Recognition> {
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

    #[test]
    fn engine_rectangles_map_to_exported_top_left_pixels() {
        assert_eq!(
            image_rect(
                (400.0, 100.0, 600.0, 120.0),
                (1200, 300),
                (2400.0, 600.0),
                false
            ),
            Some((200.0, 50.0, 300.0, 60.0))
        );
        let vision = image_rect((0.25, 0.1, 0.4, 0.2), (1200, 300), (1.0, 1.0), true).unwrap();
        for (actual, expected) in [vision.0, vision.1, vision.2, vision.3]
            .into_iter()
            .zip([300.0, 210.0, 480.0, 60.0])
        {
            assert!((actual - expected).abs() < 1e-8);
        }
        let clipped = image_rect((-0.1, 0.8, 0.3, 0.3), (1200, 300), (1.0, 1.0), true).unwrap();
        for (actual, expected) in [clipped.0, clipped.1, clipped.2, clipped.3]
            .into_iter()
            .zip([0.0, 0.0, 240.0, 60.0])
        {
            assert!((actual - expected).abs() < 1e-8);
        }
        assert_eq!(
            image_rect((f64::NAN, 0.0, 0.2, 0.2), (1200, 300), (1.0, 1.0), true),
            None
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_ocr_locates_words_in_fixture() {
        let image = image::load_from_memory(include_bytes!("../../tests/fixtures/vision-ocr.png"))
            .unwrap()
            .to_rgba8();
        let result = recognize_regions(image.width(), image.height(), image.as_raw()).unwrap();

        assert!(
            result.text.contains("ULTRA"),
            "recognized: {:?}",
            result.text
        );
        assert!(
            result.text.contains("CLIPBOARD"),
            "recognized: {:?}",
            result.text
        );
        assert!(!result.regions.is_empty());
        assert!(result.regions.iter().all(|region| {
            region.x >= 0.0
                && region.y >= 0.0
                && region.width > 0.0
                && region.height > 0.0
                && region.x + region.width <= f64::from(image.width())
                && region.y + region.height <= f64::from(image.height())
        }));
        assert!(result.regions.iter().any(|region| {
            region.text.contains("ULTRA")
                && region.width < f64::from(image.width()) / 2.0
                && region.height < f64::from(image.height()) / 2.0
        }));
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
        let recognition = recognize_regions(image.width(), image.height(), image.as_raw()).unwrap();
        assert!(!recognition.regions.is_empty());
        assert!(recognition.regions.iter().all(|region| {
            region.x >= 0.0
                && region.y >= 0.0
                && region.x + region.width <= f64::from(image.width())
                && region.y + region.height <= f64::from(image.height())
        }));
    }
}
