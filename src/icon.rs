//! The mark as pixels, for the window icon and the `.ico` beside the
//! executable that the Start menu shortcut points at.
//!
//! egui cannot draw into an image, so this rasterises the still mark itself
//! from the numbers in `logo.rs`, on a graphite rounded square. Each shape is
//! a distance function, which gives smooth edges at every size without a
//! drawing crate.

use std::f32::consts::TAU;
#[cfg(windows)]
use std::path::PathBuf;

use crate::logo;

const BACKGROUND: [u8; 3] = [0x16, 0x18, 0x1c];
/// The ring's radius as a share of the icon's width.
const RING_SHARE: f32 = 0.36;
#[cfg(windows)]
const ICO_SIZES: [usize; 6] = [16, 24, 32, 48, 64, 128];

/// One pixel being built up: straight colour and coverage, 0 to 1.
#[derive(Clone, Copy)]
struct Pixel {
    rgb: [f32; 3],
    alpha: f32,
}

impl Pixel {
    const CLEAR: Self = Self {
        rgb: [0.0; 3],
        alpha: 0.0,
    };

    /// Lay `rgb` over this pixel at `alpha`.
    fn over(&mut self, rgb: [u8; 3], alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        let out = alpha + self.alpha * (1.0 - alpha);
        if out <= 0.0 {
            return;
        }
        for (channel, new) in self.rgb.iter_mut().zip(rgb) {
            let new = f32::from(new) / 255.0;
            *channel = (new * alpha + *channel * self.alpha * (1.0 - alpha)) / out;
        }
        self.alpha = out;
    }

    fn rgba(self) -> [u8; 4] {
        let byte = |value: f32| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let byte = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            byte
        };
        [
            byte(self.rgb[0]),
            byte(self.rgb[1]),
            byte(self.rgb[2]),
            byte(self.alpha),
        ]
    }
}

/// Coverage of a pixel whose centre is `distance` pixels outside a shape.
fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

fn rounded_box(point: (f32, f32), center: (f32, f32), half: (f32, f32), radius: f32) -> f32 {
    let qx = (point.0 - center.0).abs() - half.0 + radius;
    let qy = (point.1 - center.1).abs() - half.1 + radius;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

fn segment(point: (f32, f32), from: (f32, f32), to: (f32, f32)) -> f32 {
    let (px, py) = (point.0 - from.0, point.1 - from.1);
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let along = ((px * dx + py * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((px - dx * along).powi(2) + (py - dy * along).powi(2)).sqrt()
}

/// The still mark as `size` by `size` RGBA pixels, rows top to bottom.
#[must_use]
pub fn rgba(size: usize) -> Vec<u8> {
    #[allow(clippy::cast_precision_loss)]
    let width = size as f32;
    let center = (width / 2.0, width / 2.0);
    let radius = width * RING_SHARE;
    let angle = logo::REST_ANGLE;
    let tip = (
        center.0 + angle.cos() * radius,
        center.1 + angle.sin() * radius,
    );

    let mut bytes = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            #[allow(clippy::cast_precision_loss)]
            let point = (x as f32 + 0.5, y as f32 + 0.5);
            let mut pixel = Pixel::CLEAR;

            let half = width / 2.0 - 0.5;
            let plate = rounded_box(point, center, (half, half), width * 0.22);
            pixel.over(BACKGROUND, coverage(plate));

            let (dx, dy) = (point.0 - center.0, point.1 - center.1);
            let from_center = (dx * dx + dy * dy).sqrt();

            let behind_arm = (angle - dy.atan2(dx)).rem_euclid(TAU);
            if behind_arm < logo::FAN_SPAN {
                let strength = logo::FAN_ALPHA * (1.0 - behind_arm / logo::FAN_SPAN);
                pixel.over(logo::COPPER, strength * coverage(from_center - radius));
            }

            let ring_half = (radius * logo::RING_WIDTH).max(1.0) / 2.0;
            let ring = (from_center - radius).abs() - ring_half;
            pixel.over(logo::COPPER, logo::RING_ALPHA * coverage(ring));

            for index in 0..3 {
                let pill_half = (
                    logo::PILL_WIDTHS[index] * radius / 2.0,
                    (logo::PILL_HEIGHT * radius / 2.0).max(0.6),
                );
                let pill_center = (
                    center.0 + logo::PILL_LEFT * radius + pill_half.0,
                    center.1 + logo::PILL_YS[index] * radius,
                );
                let pill = rounded_box(point, pill_center, pill_half, pill_half.1);
                pixel.over(logo::PILL, logo::REST_PILLS[index] * coverage(pill));
            }

            let arm_half = (radius * logo::ARM_WIDTH).max(1.0) / 2.0;
            pixel.over(
                logo::COPPER,
                coverage(segment(point, center, tip) - arm_half),
            );

            let dot = ((point.0 - tip.0).powi(2) + (point.1 - tip.1).powi(2)).sqrt()
                - (radius * logo::DOT_RADIUS).max(1.0);
            pixel.over(logo::DOT, coverage(dot));

            bytes.extend_from_slice(&pixel.rgba());
        }
    }
    bytes
}

/// The mark as a Windows `.ico` holding several sizes.
///
/// Each image is an uncompressed 32-bit bitmap, the form every Windows
/// version reads: a header, rows bottom to top as BGRA, then a one-bit mask
/// that is all zero because the alpha channel already says what is clear.
#[cfg(windows)]
#[must_use]
pub fn ico() -> Vec<u8> {
    let images: Vec<(usize, Vec<u8>)> = ICO_SIZES
        .iter()
        .map(|size| (*size, ico_bitmap(*size)))
        .collect();

    let mut file = Vec::new();
    file.extend_from_slice(&0u16.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&le16(images.len()));

    let mut offset = 6 + 16 * images.len();
    for (size, bitmap) in &images {
        // The directory's size bytes hold 0 for 256, and nothing here is larger.
        #[allow(clippy::cast_possible_truncation)]
        let edge = *size as u8;
        file.extend_from_slice(&[edge, edge, 0, 0]);
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&32u16.to_le_bytes());
        file.extend_from_slice(&le32(bitmap.len()));
        file.extend_from_slice(&le32(offset));
        offset += bitmap.len();
    }
    for (_, bitmap) in images {
        file.extend_from_slice(&bitmap);
    }
    file
}

#[cfg(windows)]
fn ico_bitmap(size: usize) -> Vec<u8> {
    let pixels = rgba(size);
    let mask_row = size.div_ceil(32) * 4;
    let mut bitmap = Vec::with_capacity(40 + pixels.len() + mask_row * size);

    bitmap.extend_from_slice(&40u32.to_le_bytes());
    bitmap.extend_from_slice(&le32(size));
    // Doubled: the height counts the colour rows and the mask rows.
    bitmap.extend_from_slice(&le32(size * 2));
    bitmap.extend_from_slice(&1u16.to_le_bytes());
    bitmap.extend_from_slice(&32u16.to_le_bytes());
    bitmap.extend_from_slice(&[0; 24]);

    for row in (0..size).rev() {
        let (row_pixels, _) = pixels[row * size * 4..(row + 1) * size * 4].as_chunks::<4>();
        for [red, green, blue, alpha] in row_pixels {
            bitmap.extend_from_slice(&[*blue, *green, *red, *alpha]);
        }
    }
    bitmap.resize(bitmap.len() + mask_row * size, 0);
    bitmap
}

#[cfg(windows)]
fn le16(value: usize) -> [u8; 2] {
    u16::try_from(value).unwrap_or(u16::MAX).to_le_bytes()
}

#[cfg(windows)]
fn le32(value: usize) -> [u8; 4] {
    u32::try_from(value).unwrap_or(u32::MAX).to_le_bytes()
}

/// Where the `.ico` goes: beside the executable.
#[cfg(windows)]
fn ico_path() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.with_file_name("tabsweep.ico"))
}

/// Write the `.ico` beside the executable unless the one there is already
/// this drawing. A failure is ignored: the icon is decoration.
#[cfg(windows)]
pub fn write_ico_beside_exe() {
    let Some(path) = ico_path() else {
        return;
    };
    let wanted = ico();
    if std::fs::read(&path).is_ok_and(|existing| existing == wanted) {
        return;
    }
    let _ = std::fs::write(path, wanted);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(bytes: &[u8], size: usize, x: usize, y: usize) -> [u8; 4] {
        let at = (y * size + x) * 4;
        [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]
    }

    #[test]
    fn the_plate_is_solid_in_the_middle_and_clear_at_the_corners() {
        let size = 64;
        let bytes = rgba(size);
        assert_eq!(bytes.len(), size * size * 4);
        assert_eq!(pixel(&bytes, size, 32, 32)[3], 255);
        assert_eq!(pixel(&bytes, size, 0, 0)[3], 0);
        assert_eq!(pixel(&bytes, size, 63, 63)[3], 0);
    }

    #[test]
    fn the_ring_is_copper() {
        let size = 128;
        let bytes = rgba(size);
        // Straight left of centre on the ring, away from the arm and its fan.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let x = (64.0 - 128.0 * RING_SHARE) as usize;
        let [red, green, blue, _] = pixel(&bytes, size, x, 64);
        assert!(red > green && green > blue, "{red} {green} {blue}");
        assert!(red > 100);
    }

    #[cfg(windows)]
    #[test]
    fn the_ico_lists_every_size_and_its_offsets_add_up() {
        let file = ico();
        assert_eq!(&file[0..4], &[0, 0, 1, 0]);
        let count = usize::from(u16::from_le_bytes([file[4], file[5]]));
        assert_eq!(count, ICO_SIZES.len());

        let mut expected_offset = 6 + 16 * count;
        for (index, size) in ICO_SIZES.iter().enumerate() {
            let entry = &file[6 + index * 16..6 + (index + 1) * 16];
            assert_eq!(usize::from(entry[0]), *size);
            let bytes = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
            let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
            assert_eq!(offset, expected_offset);
            assert_eq!(bytes, 40 + size * size * 4 + size.div_ceil(32) * 4 * size);
            expected_offset += bytes;
        }
        assert_eq!(file.len(), expected_offset);
    }
}
