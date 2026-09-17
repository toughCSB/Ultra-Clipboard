//! Pixel rectangles and raw RGBA buffer helpers shared by capture and output.

use serde::{Deserialize, Serialize};

/// Rectangle in physical pixels. The origin depends on the caller: virtual desktop
/// coordinates for monitors and windows, or monitor-local coordinates for selections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    pub fn bottom(&self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && y >= f64::from(self.y)
            && x < self.right() as f64
            && y < self.bottom() as f64
    }

    pub fn intersect(&self, other: &PixelRect) -> Option<PixelRect> {
        let left = i64::from(self.x.max(other.x));
        let top = i64::from(self.y.max(other.y));
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right <= left || bottom <= top {
            return None;
        }

        Some(PixelRect {
            x: left as i32,
            y: top as i32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }

    /// Re-expresses this rectangle relative to another origin in the same coordinate space.
    pub fn relative_to(&self, origin_x: i32, origin_y: i32) -> PixelRect {
        PixelRect {
            x: self.x - origin_x,
            y: self.y - origin_y,
            width: self.width,
            height: self.height,
        }
    }

    pub fn byte_len(&self) -> Option<usize> {
        (self.width as usize)
            .checked_mul(self.height as usize)?
            .checked_mul(4)
    }
}

/// Copies `rect` out of a top-down RGBA buffer. Returns `None` when the rectangle
/// is empty, outside the source, or the buffer length does not match its size.
pub fn crop_rgba(source: &[u8], width: u32, height: u32, rect: PixelRect) -> Option<Vec<u8>> {
    let bounds = PixelRect::new(0, 0, width, height);
    if bounds.byte_len()? != source.len() || bounds.intersect(&rect)? != rect {
        return None;
    }

    let row_bytes = width as usize * 4;
    let start_x = rect.x as usize * 4;
    let copy_bytes = rect.width as usize * 4;
    let mut output = Vec::with_capacity(rect.byte_len()?);

    for row in rect.y as usize..rect.y as usize + rect.height as usize {
        let offset = row * row_bytes + start_x;
        output.extend_from_slice(&source[offset..offset + copy_bytes]);
    }

    Some(output)
}

/// Converts GDI BGRA pixels to RGBA and forces opaque alpha, because screen DIBs
/// leave the reserved byte undefined.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn bgra_to_rgba_opaque(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = u8::MAX;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered_rgba(width: u32, height: u32) -> Vec<u8> {
        (0..width * height)
            .flat_map(|index| {
                let value = index as u8;
                [value, value, value, 255]
            })
            .collect()
    }

    #[test]
    fn intersect_clips_to_overlap() {
        let a = PixelRect::new(0, 0, 100, 80);
        let b = PixelRect::new(60, -10, 100, 40);

        assert_eq!(a.intersect(&b), Some(PixelRect::new(60, 0, 40, 30)));
    }

    #[test]
    fn intersect_rejects_touching_edges() {
        let a = PixelRect::new(0, 0, 10, 10);
        let b = PixelRect::new(10, 0, 10, 10);

        assert_eq!(a.intersect(&b), None);
    }

    #[test]
    fn contains_point_excludes_right_and_bottom_edges() {
        let rect = PixelRect::new(-1920, 0, 1920, 1080);

        assert!(rect.contains_point(-1920.0, 0.0));
        assert!(rect.contains_point(-0.5, 1079.5));
        assert!(!rect.contains_point(0.0, 10.0));
        assert!(!rect.contains_point(-10.0, 1080.0));
    }

    #[test]
    fn relative_to_makes_virtual_rect_monitor_local() {
        let window = PixelRect::new(2500, 300, 800, 600);

        assert_eq!(
            window.relative_to(2160, 0),
            PixelRect::new(340, 300, 800, 600)
        );
    }

    #[test]
    fn crop_copies_exact_pixels() {
        let source = numbered_rgba(4, 3);
        let cropped = crop_rgba(&source, 4, 3, PixelRect::new(1, 1, 2, 2)).unwrap();

        assert_eq!(
            cropped,
            vec![5, 5, 5, 255, 6, 6, 6, 255, 9, 9, 9, 255, 10, 10, 10, 255]
        );
    }

    #[test]
    fn crop_rejects_out_of_bounds_and_bad_buffers() {
        let source = numbered_rgba(4, 3);

        assert!(crop_rgba(&source, 4, 3, PixelRect::new(3, 0, 2, 1)).is_none());
        assert!(crop_rgba(&source, 4, 3, PixelRect::new(0, 0, 0, 1)).is_none());
        assert!(crop_rgba(&source[..8], 4, 3, PixelRect::new(0, 0, 1, 1)).is_none());
    }

    #[test]
    fn bgra_conversion_swaps_channels_and_sets_alpha() {
        let mut pixels = vec![10, 20, 30, 0, 1, 2, 3, 99];

        bgra_to_rgba_opaque(&mut pixels);

        assert_eq!(pixels, vec![30, 20, 10, 255, 3, 2, 1, 255]);
    }
}
