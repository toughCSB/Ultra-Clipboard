use std::path::Path;

use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};

const DEFAULT_ICON_PIXEL_SIZE: u32 = 256;

pub const DIR_CACHE_KEY: &str = "<dir>";

pub fn get_icon_cache_key(path: &Path) -> String {
    if path.is_dir() {
        return DIR_CACHE_KEY.to_string();
    }

    let extname = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    if (cfg!(target_os = "macos") && extname == "app")
        || (cfg!(target_os = "windows") && extname == "exe")
    {
        return path.to_string_lossy().to_string();
    }

    if extname.is_empty() {
        return path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
    }

    format!(".{}", extname.to_lowercase())
}

pub fn icon_png(path: &Path, size: Option<u32>) -> Option<Vec<u8>> {
    let size = size.unwrap_or(DEFAULT_ICON_PIXEL_SIZE);
    let icon = match file_icon_provider::get_file_icon(path, size as u16) {
        Ok(i) => i,
        Err(err) => {
            log::warn!(
                "icon_png: get_file_icon failed for {}: {err:?}",
                path.display()
            );
            return None;
        }
    };
    let mut out = Vec::with_capacity((size * size * 4) as usize / 2);
    let encoder = PngEncoder::new(&mut out);
    if let Err(err) = encoder.write_image(
        &icon.pixels,
        icon.width,
        icon.height,
        ColorType::Rgba8.into(),
    ) {
        log::warn!("icon_png: encode PNG failed for {}: {err}", path.display());
        return None;
    }
    Some(out)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn icon_png_for_finder_returns_bytes() {
        let path = PathBuf::from("/System/Library/CoreServices/Finder.app");
        let png = icon_png(&path, None).expect("expected PNG bytes");
        assert!(png.len() > 100, "PNG too small: {}", png.len());

        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        println!("Finder.app icon PNG size: {} bytes", png.len());
    }
}
