use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ContentFormat};

use super::payload::{ClipboardPayload, ImagePayload, TextPayload};
use crate::core::{AppError, Result};
use crate::settings::{Capture, CaptureKind};

pub struct ClipboardReader {
    ctx: ClipboardContext,
}

impl ClipboardReader {
    pub fn new() -> Result<Self> {
        let ctx = ClipboardContext::new().map_err(clip_err)?;
        Ok(Self { ctx })
    }

    pub fn read_with_capture(&self, capture: &Capture) -> Result<Option<ClipboardPayload>> {
        let mut text_payload: Option<Option<TextPayload>> = None;

        for kind in capture.ordered_kinds() {
            if !capture.is_enabled(kind) {
                continue;
            }

            match kind {
                CaptureKind::Files => {
                    if let Some(files) = self.read_files()? {
                        return Ok(Some(ClipboardPayload::Files(files)));
                    }
                }
                CaptureKind::Image => {
                    if self.ctx.has(ContentFormat::Image) {
                        if let Some(image) = self.read_image()? {
                            return Ok(Some(ClipboardPayload::Image(image)));
                        }
                    }
                }
                CaptureKind::Html | CaptureKind::Rtf | CaptureKind::Text => {
                    if text_payload.is_none() {
                        text_payload = Some(self.read_text_payload()?);
                    }

                    let Some(text) = text_payload.as_ref().and_then(|payload| payload.as_ref())
                    else {
                        continue;
                    };

                    if text_contains_kind(text, kind) {
                        return Ok(Some(ClipboardPayload::Text(text.clone())));
                    }
                }
            }
        }

        Ok(None)
    }

    fn read_files(&self) -> Result<Option<Vec<String>>> {
        if !self.ctx.has(ContentFormat::Files) {
            return Ok(None);
        }

        let files: Vec<String> = self
            .ctx
            .get_files()
            .map_err(clip_err)?
            .into_iter()
            .filter(|path| !path.is_empty())
            .collect();
        if files.is_empty() {
            return Ok(None);
        }

        Ok(Some(files))
    }

    fn read_text_payload(&self) -> Result<Option<TextPayload>> {
        let has_text = self.ctx.has(ContentFormat::Text);
        let has_html = self.ctx.has(ContentFormat::Html);
        let has_rtf = self.ctx.has(ContentFormat::Rtf);
        if !has_text && !has_html && !has_rtf {
            return Ok(None);
        }

        let text = if has_text {
            self.ctx.get_text().map_err(clip_err)?
        } else {
            String::new()
        };
        let html = read_optional(has_html, || self.ctx.get_html());
        let rtf = read_optional(has_rtf, || self.ctx.get_rich_text());

        if text.is_empty() && html.is_none() && rtf.is_none() {
            return Ok(None);
        }

        Ok(Some(TextPayload { text, html, rtf }))
    }

    fn read_image(&self) -> Result<Option<ImagePayload>> {
        if let Ok(bytes) = self.ctx.get_buffer(PNG_FORMAT) {
            if let Some((width, height)) = png_dimensions(&bytes) {
                return Ok(Some(ImagePayload {
                    bytes,
                    width,
                    height,
                }));
            }
        }

        let image = self.ctx.get_image().map_err(clip_err)?;
        let (width, height) = image.get_size();
        if width == 0 || height == 0 {
            return Ok(None);
        }

        let bytes = image.to_png().map_err(clip_err)?.get_bytes().to_vec();
        if bytes.is_empty() {
            return Ok(None);
        }

        Ok(Some(ImagePayload {
            bytes,
            width,
            height,
        }))
    }
}

#[cfg(target_os = "macos")]
const PNG_FORMAT: &str = "public.png";
#[cfg(target_os = "windows")]
const PNG_FORMAT: &str = "PNG";

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 24 || bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

fn read_optional(
    available: bool,
    read: impl FnOnce() -> clipboard_rs::common::Result<String>,
) -> Option<String> {
    if !available {
        return None;
    }
    read().ok().filter(|value| !value.is_empty())
}

fn text_contains_kind(text: &TextPayload, kind: CaptureKind) -> bool {
    let has_plain = !text.text.trim().is_empty();

    match kind {
        CaptureKind::Text => has_plain,
        CaptureKind::Html => {
            has_plain
                && text
                    .html
                    .as_ref()
                    .is_some_and(|html| !html.trim().is_empty())
        }
        CaptureKind::Rtf => {
            has_plain && text.rtf.as_ref().is_some_and(|rtf| !rtf.trim().is_empty())
        }
        CaptureKind::Files | CaptureKind::Image => false,
    }
}

fn clip_err<E: std::fmt::Display>(err: E) -> AppError {
    AppError::Clipboard(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use clipboard_rs::{Clipboard, ClipboardContext};

    fn sample_png(w: u32, h: u32) -> Vec<u8> {
        use std::io::Cursor;
        let buf = image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]));
        let mut out = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(buf)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn png_dimensions_reads_real_png_header() {
        assert_eq!(png_dimensions(&sample_png(123, 45)), Some((123, 45)));
    }

    #[test]
    fn png_dimensions_rejects_non_png_and_truncated() {
        assert_eq!(png_dimensions(b"not a png at all really"), None);

        assert_eq!(png_dimensions(&sample_png(8, 8)[..20]), None);

        assert_eq!(png_dimensions(&[]), None);
    }

    #[test]
    fn rich_text_without_plain_is_not_captureable_text() {
        let text = TextPayload {
            text: String::new(),
            html: Some("<b>only html</b>".to_owned()),
            rtf: Some(r"{\rtf1 only rtf}".to_owned()),
        };

        assert!(!text_contains_kind(&text, CaptureKind::Html));
        assert!(!text_contains_kind(&text, CaptureKind::Rtf));
        assert!(!text_contains_kind(&text, CaptureKind::Text));
    }

    #[test]
    #[ignore = "touches the real system clipboard; run with --ignored on a desktop session"]
    fn round_trip_text() {
        let _guard = crate::clipboard::test_lock::serial();
        let ctx = ClipboardContext::new().unwrap();
        ctx.set_text("hello ultra clipboard".to_string()).unwrap();

        let payload = ClipboardReader::new()
            .unwrap()
            .read_with_capture(&Capture::default())
            .unwrap()
            .expect("clipboard should contain text");

        match payload {
            ClipboardPayload::Text(text) => assert_eq!(text.text, "hello ultra clipboard"),
            other => panic!("expected text payload, got {other:?}"),
        }
    }
}
