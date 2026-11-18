//! The Tab Sweep mark, and how it moves.
//!
//! The mark is a copper ring holding three tab-shaped bars, with an arm
//! sweeping round it and a fading fan behind the arm, like a radar. The bars
//! dim top to bottom: tabs on their way out.
//!
//! It is drawn with egui shapes rather than loaded from an image, as ironterm
//! draws its logo, so the same drawing serves the title, the loading screen
//! and the closing screen, where the arm turns. The numbers here are in units
//! of the ring's radius; `icon.rs` rasterises the same numbers for the window
//! icon.

use std::f32::consts::TAU;

use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

use crate::theme;

pub const PILL_LEFT: f32 = -0.46;
pub const PILL_WIDTHS: [f32; 3] = [0.92, 0.70, 0.44];
pub const PILL_YS: [f32; 3] = [-0.30, 0.0, 0.30];
pub const PILL_HEIGHT: f32 = 0.17;
pub const RING_WIDTH: f32 = 0.07;
pub const RING_ALPHA: f32 = 0.6;
pub const ARM_WIDTH: f32 = 0.06;
pub const DOT_RADIUS: f32 = 0.10;
/// How far the fan trails behind the arm, in radians, and how strong it is
/// at the arm.
pub const FAN_SPAN: f32 = 1.4;
pub const FAN_ALPHA: f32 = 0.38;
/// Where the arm sits in the still mark: up and to the right.
pub const REST_ANGLE: f32 = -0.87;
pub const REST_PILLS: [f32; 3] = [1.0, 0.62, 0.30];

pub const COPPER: [u8; 3] = [0xe0, 0x8a, 0x45];
pub const DOT: [u8; 3] = [0xec, 0xa5, 0x6e];
pub const PILL: [u8; 3] = [0xc9, 0xc8, 0xc4];

/// One full turn of the arm.
const TURN_SECONDS: f64 = 1.8;
const FAN_STEPS: usize = 28;

/// The moving parts of the mark at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    /// The arm's angle in radians, clockwise from pointing right.
    pub angle: f32,
    /// Opacity of each bar, top to bottom, 0 to 1.
    pub pills: [f32; 3],
}

impl Sweep {
    /// The still mark.
    #[must_use]
    pub fn resting() -> Self {
        Self {
            angle: REST_ANGLE,
            pills: REST_PILLS,
        }
    }

    /// The mark `seconds` into its turning. Each bar dims as the arm passes
    /// and comes back over the rest of the turn, one after another.
    #[must_use]
    pub fn at(seconds: f64) -> Self {
        let turn = (seconds / TURN_SECONDS).rem_euclid(1.0);
        #[allow(clippy::cast_possible_truncation)]
        let turn = turn as f32;
        let mut pills = [0.0; 3];
        for (index, pill) in pills.iter_mut().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let hit = 0.08 + index as f32 * 0.2;
            let since_hit = (turn - hit).rem_euclid(1.0);
            *pill = 0.2 + 0.8 * since_hit;
        }
        Self {
            angle: turn * TAU - TAU / 4.0,
            pills,
        }
    }
}

fn with_alpha(rgb: [u8; 3], alpha: f32) -> Color32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], alpha)
}

fn on_circle(center: Pos2, radius: f32, angle: f32) -> Pos2 {
    center + vec2(angle.cos(), angle.sin()) * radius
}

/// Draw the mark centred on `center` with a ring of `radius`.
pub fn paint(painter: &Painter, center: Pos2, radius: f32, sweep: Sweep) {
    paint_fan(painter, center, radius, sweep.angle);
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(
            (radius * RING_WIDTH).max(1.0),
            with_alpha(COPPER, RING_ALPHA),
        ),
    );
    for index in 0..3 {
        let left = center.x + PILL_LEFT * radius;
        let middle = center.y + PILL_YS[index] * radius;
        let half_height = PILL_HEIGHT * radius * 0.5;
        let rect = Rect::from_min_max(
            pos2(left, middle - half_height),
            pos2(left + PILL_WIDTHS[index] * radius, middle + half_height),
        );
        painter.rect_filled(rect, half_height, with_alpha(PILL, sweep.pills[index]));
    }
    let tip = on_circle(center, radius, sweep.angle);
    painter.line_segment(
        [center, tip],
        Stroke::new((radius * ARM_WIDTH).max(1.0), theme::ACCENT),
    );
    painter.circle_filled(tip, (radius * DOT_RADIUS).max(1.5), theme::ACCENT_TEXT);
}

/// The fan behind the arm: slices of the disc that fade away from the arm.
fn paint_fan(painter: &Painter, center: Pos2, radius: f32, angle: f32) {
    let mut mesh = egui::Mesh::default();
    for step in 0..=FAN_STEPS {
        #[allow(clippy::cast_precision_loss)]
        let along = step as f32 / FAN_STEPS as f32;
        let color = with_alpha(COPPER, FAN_ALPHA * (1.0 - along));
        mesh.colored_vertex(center, color);
        mesh.colored_vertex(on_circle(center, radius, angle - FAN_SPAN * along), color);
    }
    for step in 0..FAN_STEPS {
        #[allow(clippy::cast_possible_truncation)]
        let base = (step * 2) as u32;
        mesh.add_triangle(base, base + 1, base + 3);
    }
    painter.add(Shape::mesh(mesh));
}

/// An arc of a circle from the top, clockwise, covering `fraction` of it.
/// Used for the closing screen's progress ring.
pub fn paint_arc(painter: &Painter, center: Pos2, radius: f32, fraction: f32, stroke: Stroke) {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.0 {
        return;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let steps = ((fraction * 96.0).ceil() as usize).max(2);
    let points: Vec<Pos2> = (0..=steps)
        .map(|step| {
            #[allow(clippy::cast_precision_loss)]
            let along = step as f32 / steps as f32;
            on_circle(center, radius, -TAU / 4.0 + TAU * fraction * along)
        })
        .collect();
    painter.add(Shape::line(points, stroke));
}

/// Small bars drifting out from the mark and fading: tabs being swept away.
/// They leave from fixed directions, one after another, so the motion is the
/// same every time and never jumps.
pub fn paint_swept_tabs(painter: &Painter, center: Pos2, radius: f32, seconds: f64) {
    const COUNT: usize = 9;
    const LIFE_SECONDS: f64 = 2.6;
    // The golden angle spreads the directions without a pattern showing.
    const GOLDEN_ANGLE: f32 = 2.399_963;

    for index in 0..COUNT {
        #[allow(clippy::cast_precision_loss)]
        let offset = index as f64 / COUNT as f64;
        #[allow(clippy::cast_possible_truncation)]
        let life = (seconds / LIFE_SECONDS + offset).rem_euclid(1.0) as f32;
        #[allow(clippy::cast_precision_loss)]
        let direction = index as f32 * GOLDEN_ANGLE;
        let middle = on_circle(center, radius * (1.55 + 0.75 * life), direction);
        let fade_in = (life * 6.0).min(1.0);
        let fade_out = (1.0 - life) * (1.0 - life);
        let half = vec2(radius * 0.13, radius * 0.04);
        painter.rect_filled(
            Rect::from_center_size(middle, half * 2.0),
            half.y,
            with_alpha(PILL, 0.45 * fade_in * fade_out),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arm_starts_at_the_top_and_is_back_after_one_turn() {
        let start = Sweep::at(0.0);
        assert!((start.angle + TAU / 4.0).abs() < 1e-4);
        let later = Sweep::at(TURN_SECONDS);
        assert!((later.angle - start.angle).abs() < 1e-3);
    }

    #[test]
    fn bars_stay_visible_through_the_whole_turn() {
        for tenth in 0..40 {
            let sweep = Sweep::at(f64::from(tenth) * 0.1);
            for pill in sweep.pills {
                assert!((0.2..=1.0).contains(&pill), "{pill}");
            }
        }
    }

    #[test]
    fn a_bar_is_dimmest_just_after_the_arm_passes_it() {
        let just_after = Sweep::at(TURN_SECONDS * 0.09).pills[0];
        let much_later = Sweep::at(TURN_SECONDS * 0.9).pills[0];
        assert!(just_after < 0.3);
        assert!(much_later > 0.8);
    }
}
