// Copyright © The Daybrite Project
// SPDX-License-Identifier: MPL-2.0

//! Color: the categorical palette a series scale assigns from, and the ramps a continuous color
//! channel interpolates through.
//!
//! The default categorical palette is **Okabe–Ito** (Okabe & Ito, *Color Universal Design*), the
//! eight-color set designed to stay distinguishable under all three common forms of color vision
//! deficiency. It is the palette the accessibility literature recommends and it costs nothing
//! against a prettier one: a chart whose series are told apart by hue has to work for the ~8% of
//! men who would otherwise read two of them as the same series.
//!
//! The sequential ramp is **Viridis**, sampled at eight stops. Viridis is perceptually uniform —
//! equal steps in the data are equal perceived steps in color — and monotone in lightness, so it
//! survives being printed in grayscale. A rainbow ramp does neither, which is why it misleads.

use day_spec::Color;

fn rgb8(r: u8, g: u8, b: u8) -> Color {
    Color::rgb(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0)
}

/// The Okabe–Ito qualitative palette, in its published order.
pub fn categorical(i: usize) -> Color {
    const P: [(u8, u8, u8); 8] = [
        (0, 114, 178),   // blue
        (230, 159, 0),   // orange
        (0, 158, 115),   // bluish green
        (204, 121, 167), // reddish purple
        (86, 180, 233),  // sky blue
        (213, 94, 0),    // vermillion
        (240, 228, 66),  // yellow
        (120, 120, 120), // neutral, for an "other" bucket
    ];
    let (r, g, b) = P[i % P.len()];
    rgb8(r, g, b)
}

/// Viridis, sampled at eight stops and interpolated in sRGB.
///
/// Interpolating a perceptual ramp in sRGB is a compromise — the honest space is CAM02-UCS, which
/// is where the ramp was designed — but with stops this close the error is below a just-noticeable
/// difference, and it keeps this crate free of a color-science dependency.
pub fn sequential(t: f64) -> Color {
    const P: [(u8, u8, u8); 8] = [
        (68, 1, 84),
        (72, 40, 120),
        (62, 74, 137),
        (49, 104, 142),
        (38, 130, 142),
        (31, 158, 137),
        (53, 183, 121),
        (253, 231, 37),
    ];
    let t = t.clamp(0.0, 1.0) * (P.len() - 1) as f64;
    let i = t.floor() as usize;
    let f = t - i as f64;
    let (r0, g0, b0) = P[i.min(P.len() - 1)];
    let (r1, g1, b1) = P[(i + 1).min(P.len() - 1)];
    let mix = |a: u8, b: u8| a as f64 + (b as f64 - a as f64) * f;
    Color::rgb(
        mix(r0, r1) / 255.0,
        mix(g0, g1) / 255.0,
        mix(b0, b1) / 255.0,
    )
}

/// A diverging ramp (blue–white–red), for data with a meaningful midpoint. `t` in `0..=1` with
/// 0.5 the neutral centre.
pub fn diverging(t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (a, b, f) = if t < 0.5 {
        ((5u8, 48u8, 97u8), (247u8, 247u8, 247u8), t * 2.0)
    } else {
        ((247u8, 247u8, 247u8), (103u8, 0u8, 31u8), (t - 0.5) * 2.0)
    };
    let mix = |x: u8, y: u8| x as f64 + (y as f64 - x as f64) * f;
    Color::rgb(
        mix(a.0, b.0) / 255.0,
        mix(a.1, b.1) / 255.0,
        mix(a.2, b.2) / 255.0,
    )
}

/// The chart's own chrome, so a chart reads on both grounds without the app restating it.
/// Named fields per the API style rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Chrome {
    pub axis_line: Color,
    pub grid_line: Color,
    pub tick: Color,
    pub label: Color,
    pub title: Color,
    pub plot_background: Option<Color>,
}

impl Chrome {
    /// The chrome for a ground of the given darkness. Grid lines are deliberately faint: a grid is
    /// a reading aid, and one that competes with the data for contrast is worse than none.
    pub fn for_dark(dark: bool) -> Self {
        if dark {
            Chrome {
                axis_line: Color::rgba(1.0, 1.0, 1.0, 0.45),
                grid_line: Color::rgba(1.0, 1.0, 1.0, 0.12),
                tick: Color::rgba(1.0, 1.0, 1.0, 0.45),
                label: Color::rgba(1.0, 1.0, 1.0, 0.75),
                title: Color::rgba(1.0, 1.0, 1.0, 0.9),
                plot_background: None,
            }
        } else {
            Chrome {
                axis_line: Color::rgba(0.0, 0.0, 0.0, 0.35),
                grid_line: Color::rgba(0.0, 0.0, 0.0, 0.10),
                tick: Color::rgba(0.0, 0.0, 0.0, 0.35),
                label: Color::rgba(0.0, 0.0, 0.0, 0.65),
                title: Color::rgba(0.0, 0.0, 0.0, 0.85),
                plot_background: None,
            }
        }
    }
}

impl Default for Chrome {
    fn default() -> Self {
        Chrome::for_dark(false)
    }
}
