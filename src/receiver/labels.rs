//! A picture with a word burned into it (docs/pictures.md, 「Pictures sent」): an album of unrelated pictures —
//! one product each — is read by its pictures alone once the platform shows the album without their captions, so
//! the name is drawn into the picture itself: a dark band along the lower edge with the label on it, white. What
//! comes back is a new JPEG, laid on white where the picture has transparency; the original is not touched. The
//! text is drawn by the machine's own fonts, so no font is named by the deployment.

use std::sync::{Arc, LazyLock};

use image::imageops::FilterType;
use image::{DynamicImage, RgbImage, RgbaImage};

use resvg::{tiny_skia, usvg};

use super::pictures::{NotAPicture, decode, jpeg, on_white};

// The widest a labelled picture is drawn. Wider sources are brought down to it, narrower ones stay.
const LABELLED_WIDTH: u32 = 1280;

// What a label's font size is at a picture's width, and how many lines the band holds at most.
const FONT_SHARE: f64 = 1.0 / 24.0;
const WIDEST_LINES: usize = 2;

// The quality a labelled picture is written at.
const QUALITY: u8 = 86;

// The machine's fonts, read once for every picture this process draws.
static FONTS: LazyLock<Arc<usvg::fontdb::Database>> = LazyLock::new(|| {
  let mut fonts = usvg::fontdb::Database::new();

  fonts.load_system_fonts();
  Arc::new(fonts)
});

fn failed(why: impl std::fmt::Display) -> NotAPicture {
  NotAPicture(format!("SVG: {why}"))
}

// A CJK character is drawn about a whole em wide and a Latin one about half of that, so the width of a line
// is estimated in ems rather than measured: the font is whatever the machine has.
fn ems_of(character: char) -> f64 {
  let wide = matches!(
    character,
    '\u{1100}'..='\u{11FF}' | '\u{2E80}'..='\u{A4CF}' | '\u{AC00}'..='\u{D7AF}' | '\u{F900}'..='\u{FAFF}' | '\u{FF00}'..='\u{FFEF}'
  );

  if wide { 1.0 } else { 0.55 }
}

fn ems(text: &[char]) -> f64 {
  text.iter().map(|character| ems_of(*character)).sum()
}

/// The label broken into at most two lines that fit `ems_per_line`, the last one ending in an ellipsis where
/// the label did not fit.
pub fn label_lines(label: &str, ems_per_line: f64) -> Vec<String> {
  let characters: Vec<char> = label.split_whitespace().collect::<Vec<_>>().join(" ").chars().collect();
  let mut lines: Vec<String> = vec![];
  let mut line: Vec<char> = vec![];
  let mut width = 0.0;
  let mut index = 0;

  while index < characters.len() {
    let character = characters[index];
    let wide = ems_of(character);

    if width + wide > ems_per_line {
      if lines.len() == WIDEST_LINES - 1 {
        break;
      }

      lines.push(line.iter().collect::<String>().trim_end().to_owned());
      line.clear();
      width = 0.0;
    }

    line.push(character);
    width += wide;
    index += 1;
  }

  if index < characters.len() {
    while !line.is_empty() && ems(&line) + 1.0 > ems_per_line {
      line.pop();
    }

    let cut: String = line.iter().collect();

    line = format!("{}…", cut.trim_end()).chars().collect();
  }

  let last: String = line.iter().collect::<String>().trim_end().to_owned();

  if line.is_empty() {
    lines
  } else {
    lines.push(last);
    lines
  }
}

fn escape_xml(text: &str) -> String {
  text
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
}

// The way a number of pixels is rounded, half away from nothing, as the scripts round it.
fn rounded(value: f64) -> u32 {
  value.round().max(0.0) as u32
}

/// The picture an SVG document draws, its pixels no longer premultiplied by their alpha.
fn drawn(bytes: &[u8]) -> Result<DynamicImage, NotAPicture> {
  let options = usvg::Options {
    fontdb: FONTS.clone(),
    ..usvg::Options::default()
  };
  let tree = usvg::Tree::from_data(bytes, &options).map_err(failed)?;
  let size = tree.size().to_int_size();
  let mut pixmap =
    tiny_skia::Pixmap::new(size.width(), size.height()).ok_or_else(|| failed("an empty picture"))?;

  resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

  let pixels = pixmap.pixels().iter().flat_map(|pixel| {
    let straight = pixel.demultiply();

    [straight.red(), straight.green(), straight.blue(), straight.alpha()]
  });
  let picture = RgbaImage::from_raw(size.width(), size.height(), pixels.collect())
    .ok_or_else(|| failed("the pixels do not fill the picture"))?;

  Ok(DynamicImage::ImageRgba8(picture))
}

// The overlay laid over the picture, each pixel weighed by the overlay's own alpha.
fn composite(ground: &mut RgbImage, overlay: &DynamicImage) {
  let overlay = overlay.to_rgba8();

  for (x, y, pixel) in ground.enumerate_pixels_mut() {
    let Some(over) = overlay.get_pixel_checked(x, y) else { continue };
    let [red, green, blue, alpha] = over.0;
    let laid = |under: u8, over: u8| {
      ((u32::from(over) * u32::from(alpha) + u32::from(under) * (255 - u32::from(alpha)) + 127) / 255) as u8
    };

    pixel.0 = [laid(pixel.0[0], red), laid(pixel.0[1], green), laid(pixel.0[2], blue)];
  }
}

/// The picture with the label on it, as JPEG bytes. None where the label says nothing.
pub fn labelled(bytes: &[u8], label: &str) -> Result<Option<Vec<u8>>, NotAPicture> {
  let wording = label.trim();

  if wording.is_empty() {
    return Ok(None);
  }

  let mut picture = decode(bytes)?;

  if picture.width() > LABELLED_WIDTH {
    let height =
      rounded(f64::from(picture.height()) * f64::from(LABELLED_WIDTH) / f64::from(picture.width()));

    picture = picture.resize_exact(LABELLED_WIDTH, height.max(1), FilterType::Lanczos3);
  }

  let mut ground = on_white(&picture);
  let (width, height) = ground.dimensions();
  let size = rounded(f64::from(width) * FONT_SHARE).max(14);
  let lines = label_lines(wording, f64::from(width) * 0.94 / f64::from(size));
  let line_height = rounded(f64::from(size) * 1.3);
  let band = rounded(f64::from(size) * 0.6) + u32::try_from(lines.len()).unwrap_or(0) * line_height;
  let band_top = i64::from(height) - i64::from(band);
  let x = rounded(f64::from(width) * 0.03);
  let text: String = lines
    .iter()
    .enumerate()
    .map(|(row, line)| {
      let row = i64::try_from(row).unwrap_or(0);
      let y = band_top + i64::from(rounded(f64::from(size) * 0.3)) + (row + 1) * i64::from(line_height)
        - i64::from(rounded(f64::from(size) * 0.25));

      format!(
        r##"<text x="{x}" y="{y}" font-family="sans-serif" font-size="{size}" font-weight="700" fill="#ffffff">{}</text>"##,
        escape_xml(line),
      )
    })
    .collect();
  let overlay = format!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><rect x="0" y="{band_top}" width="{width}" height="{band}" fill="#000000" fill-opacity="0.68"/>{text}</svg>"##
  );
  let drawn = drawn(overlay.as_bytes())?.to_rgba8();

  composite(&mut ground, &DynamicImage::ImageRgba8(drawn));

  jpeg(&ground, QUALITY).map(Some)
}
