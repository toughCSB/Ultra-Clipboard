use std::io::Cursor;

use image::{GenericImageView, ImageDecoder};

use super::{ImagePayload, ProtocolError, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS};

pub(super) fn validate_image(image: &ImagePayload) -> Result<(), ProtocolError> {
    if image.bytes.is_empty() || image.bytes.len() > MAX_IMAGE_BYTES {
        return Err(ProtocolError::TooLarge);
    }
    validate_dimensions(image.width, image.height)?;
    let decoder = image::codecs::png::PngDecoder::new(Cursor::new(&image.bytes))
        .map_err(|_| ProtocolError::InvalidImage)?;
    let dimensions = decoder.dimensions();
    validate_dimensions(dimensions.0, dimensions.1)?;
    if dimensions != (image.width, image.height) {
        return Err(ProtocolError::InvalidImage);
    }
    image::DynamicImage::from_decoder(decoder)
        .map_err(|_| ProtocolError::InvalidImage)?
        .dimensions();
    Ok(())
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), ProtocolError> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || pixels > MAX_IMAGE_PIXELS
    {
        return Err(ProtocolError::InvalidImage);
    }
    Ok(())
}
