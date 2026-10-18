//! A picture a message carried, as it is filed under its message (docs/pictures.md, 「Pictures received」): a JPEG
//! under the threshold as it came, any other picture under it with no transparency as it came, and every other one
//! laid on white, brought inside the largest side and written as JPEG. A picture that cannot be redrawn is kept as
//! it came. The pictures read are JPEG, PNG, GIF, WebP and TIFF.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader, RgbImage};

/// Why bytes are no picture this reads.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct NotAPicture(pub String);

fn failed(why: impl std::fmt::Display) -> NotAPicture {
  NotAPicture(why.to_string())
}

/// The white a picture is laid on wherever its transparency would be lost.
const GROUND: u8 = 255;

/// How a picture is filed: past how many bytes it is brought down, inside how many pixels, at what quality.
#[derive(Clone, Copy, Debug)]
pub struct Keeping {
  pub threshold_bytes: usize,
  pub max_dimension: u32,
  pub quality: u8,
}

/// Whether bytes are a JPEG, read off their first bytes: the one picture never decoded to ask about.
pub fn is_jpeg(bytes: &[u8]) -> bool {
  bytes.starts_with(&[0xff, 0xd8, 0xff])
}

/// The picture the bytes draw, of the formats this reads.
pub fn decode(bytes: &[u8]) -> Result<DynamicImage, NotAPicture> {
  match image::guess_format(bytes) {
    Ok(
      format @ (ImageFormat::Jpeg
      | ImageFormat::Png
      | ImageFormat::Gif
      | ImageFormat::WebP
      | ImageFormat::Tiff),
    ) => ImageReader::with_format(Cursor::new(bytes), format).decode().map_err(failed),
    _ => Err(failed("unsupported image format")),
  }
}

/// Whether any pixel of the picture is less than opaque; a picture with no alpha channel is opaque.
pub fn is_transparent(bytes: &[u8]) -> Result<bool, NotAPicture> {
  let picture = decode(bytes)?;

  Ok(picture.color().has_alpha() && picture.to_rgba16().pixels().any(|pixel| pixel.0[3] < u16::MAX))
}

/// The picture laid on white: each pixel's colour weighed by its own alpha against the ground, so an anti-aliased
/// edge keeps its softness against the white.
pub fn on_white(picture: &DynamicImage) -> RgbImage {
  let rgba = picture.to_rgba8();

  RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
    let [red, green, blue, alpha] = rgba.get_pixel(x, y).0;
    let laid = |channel: u8| {
      ((u32::from(channel) * u32::from(alpha) + u32::from(GROUND) * (255 - u32::from(alpha)) + 127) / 255)
        as u8
    };

    image::Rgb([laid(red), laid(green), laid(blue)])
  })
}

/// The picture written as JPEG at this quality.
pub fn jpeg(picture: &RgbImage, quality: u8) -> Result<Vec<u8>, NotAPicture> {
  let mut out = Vec::new();

  JpegEncoder::new_with_quality(&mut out, quality).encode_image(picture).map_err(failed)?;

  Ok(out)
}

// The size a picture is brought inside a square of `limit` pixels at, never larger than it stands.
fn inside(width: u32, height: u32, limit: u32) -> (u32, u32) {
  if width <= limit && height <= limit {
    return (width, height);
  }

  let scale = f64::from(limit) / f64::from(width.max(height));
  let scaled = |side: u32| ((f64::from(side) * scale).round() as u32).clamp(1, limit);

  (scaled(width), scaled(height))
}

/// A picture as it is filed under its message.
pub fn kept(bytes: Vec<u8>, keeping: Keeping) -> Vec<u8> {
  let large = bytes.len() > keeping.threshold_bytes;

  if !large && is_jpeg(&bytes) {
    return bytes;
  }

  let redrawn = (|| -> Result<Option<Vec<u8>>, NotAPicture> {
    if !large && !is_transparent(&bytes)? {
      return Ok(None);
    }

    // Laid on white before it is brought down, which is the same picture as bringing its premultiplied pixels
    // down and laying them on white after, and keeps a transparent pixel's colour out of the edge.
    let laid = on_white(&decode(&bytes)?);
    let (width, height) = inside(laid.width(), laid.height(), keeping.max_dimension);
    let brought = if (width, height) == laid.dimensions() {
      laid
    } else {
      image::imageops::resize(&laid, width, height, FilterType::Lanczos3)
    };

    jpeg(&brought, keeping.quality).map(Some)
  })();

  match redrawn {
    Ok(Some(redrawn)) => redrawn,
    Ok(None) => bytes,
    Err(why) => {
      eprintln!("bmo-receiver: a picture could not be redrawn and is kept as it came: {why}");
      bytes
    }
  }
}
