//! A picture a message carried, as it is filed under its message (docs/pictures.md, 「Pictures received」): a JPEG
//! under the threshold as it came, any other picture under it with no transparency as it came, and every other one
//! laid on white, brought inside the largest side and written as JPEG. A picture that cannot be redrawn is kept as
//! it came. A picture that moves, an animated WebP or a GIF of more than one frame, is told apart so a sticker or an
//! animation is kept as it came, and a still sticker keeps its transparency as PNG or as the PNG or WebP it came as.
//! The pictures read are JPEG, PNG, GIF, WebP and TIFF.

use std::io::Cursor;

use image::AnimationDecoder;
use image::codecs::gif::GifDecoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPDecoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageEncoder, ImageFormat, ImageReader, RgbImage, Rgba32FImage, RgbaImage};

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

/// The extension a picture that moves is filed under, `webp` for an animated WebP and `gif` for a GIF of more than
/// one frame; none for a still picture or bytes no reader here takes.
pub fn moving(bytes: &[u8]) -> Option<&'static str> {
  match image::guess_format(bytes).ok()? {
    ImageFormat::WebP => WebPDecoder::new(Cursor::new(bytes)).ok()?.has_animation().then_some("webp"),
    ImageFormat::Gif => {
      let frames = GifDecoder::new(Cursor::new(bytes))
        .ok()?
        .into_frames()
        .take(2)
        .filter(Result::is_ok)
        .count();

      (frames > 1).then_some("gif")
    }
    _ => None,
  }
}

/// Whether any pixel of the picture is less than opaque; a picture with no alpha channel is opaque.
pub fn is_transparent(bytes: &[u8]) -> Result<bool, NotAPicture> {
  Ok(has_transparency(&decode(bytes)?))
}

/// Whether any pixel of a drawn picture is less than opaque.
pub fn has_transparency(picture: &DynamicImage) -> bool {
  picture.color().has_alpha() && picture.to_rgba16().pixels().any(|pixel| pixel.0[3] < u16::MAX)
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

/// The picture written as PNG, its transparency with it.
pub fn png(picture: &RgbaImage) -> Result<Vec<u8>, NotAPicture> {
  let mut out = Vec::new();

  PngEncoder::new(&mut out)
    .write_image(picture.as_raw(), picture.width(), picture.height(), image::ExtendedColorType::Rgba8)
    .map_err(failed)?;

  Ok(out)
}

// The picture brought inside a square of `limit` pixels with its transparency, each pixel's colour weighed by its
// own alpha while it is brought down, so a transparent pixel's colour never bleeds into the edge beside it.
fn brought_inside_with_alpha(picture: &DynamicImage, limit: u32) -> RgbaImage {
  let (width, height) = inside(picture.width(), picture.height(), limit);

  if (width, height) == (picture.width(), picture.height()) {
    return picture.to_rgba8();
  }

  let mut weighed = picture.to_rgba32f();

  for pixel in weighed.pixels_mut() {
    let [red, green, blue, alpha] = pixel.0;
    pixel.0 = [red * alpha, green * alpha, blue * alpha, alpha];
  }

  let mut brought: Rgba32FImage = image::imageops::resize(&weighed, width, height, FilterType::Lanczos3);

  for pixel in brought.pixels_mut() {
    let alpha = pixel.0[3].clamp(0.0, 1.0);
    let unweighed = |channel: f32| if alpha > 0.0 { (channel / alpha).clamp(0.0, 1.0) } else { 0.0 };
    pixel.0 = [unweighed(pixel.0[0]), unweighed(pixel.0[1]), unweighed(pixel.0[2]), alpha];
  }

  DynamicImage::ImageRgba32F(brought).to_rgba8()
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

/// A still sticker as it is filed under its message, with the extension it is filed under (docs/pictures.md,
/// 「Pictures received」): one with transparency keeps it, a PNG or a WebP no larger than the threshold as it came and
/// any other one brought inside the largest side and written as PNG; one with none is filed as any picture is.
pub fn sticker_kept(bytes: Vec<u8>, keeping: Keeping) -> (Vec<u8>, &'static str) {
  let Some(picture) = decode(&bytes).ok().filter(has_transparency) else {
    return (kept(bytes, keeping), "jpg");
  };

  if bytes.len() <= keeping.threshold_bytes {
    match image::guess_format(&bytes) {
      Ok(ImageFormat::Png) => return (bytes, "png"),
      Ok(ImageFormat::WebP) => return (bytes, "webp"),
      _ => {}
    }
  }

  match png(&brought_inside_with_alpha(&picture, keeping.max_dimension)) {
    Ok(png) => (png, "png"),
    Err(why) => {
      eprintln!("bmo-receiver: a sticker could not be written as PNG and is kept as any picture: {why}");
      (kept(bytes, keeping), "jpg")
    }
  }
}
