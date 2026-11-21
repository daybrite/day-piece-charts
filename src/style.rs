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
//! The sequential ramp is **Viridis**, sampled at eight stops. Viridis is perceptually uniform
//! (equal steps in the data are equal perceived steps in color) and monotone in lightness, so it
//! survives being printed in grayscale. A rainbow ramp does neither, which is why it misleads.

use day_spec::{Color, Ink, Palette, SystemColor};

fn rgb8(r: u8, g: u8, b: u8) -> Color {
    Color::rgb(
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    )
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
/// Interpolating a perceptual ramp in sRGB is a compromise: the ramp was designed in CAM02-UCS.
/// But with stops this close the error is below a just-noticeable difference, and it keeps this
/// crate free of a color-science dependency.
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
    let mix = |a: u8, b: u8| f64::from(a) + (f64::from(b) - f64::from(a)) * f;
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
    let mix = |x: u8, y: u8| f64::from(x) + (f64::from(y) - f64::from(x)) * f;
    Color::rgb(
        mix(a.0, b.0) / 255.0,
        mix(a.1, b.1) / 255.0,
        mix(a.2, b.2) / 255.0,
    )
}

/// The chart's own chrome, so a chart reads on both grounds without the app restating it.
/// Named fields per the API style rule.
///
/// Every field is an [`Ink`], so the default chrome follows the app's light and dark appearance
/// with no rebuild: a live chart re-records when the appearance changes. Text uses the platform's
/// secondary-text role; the hairlines, the raised surface and its shadow are light/dark pairs
/// tuned for a plot. A chart given its own `Chrome` of fixed colors keeps them in both.
#[derive(Clone, Debug, PartialEq)]
pub struct Chrome {
    pub axis_line: Ink,
    pub grid_line: Ink,
    pub tick: Ink,
    pub label: Ink,
    pub title: Ink,
    pub plot_background: Option<Ink>,
    /// The fill of what the chart raises over its marks: the selection's label box, an
    /// annotation pill. Opaque, so the marks under it do not show through the text.
    pub surface: Ink,
    /// The color of the shadow those cast.
    pub shadow: Ink,
}

/// Chart text: the platform's secondary text, which on Apple platforms is the single grey Swift
/// Charts draws every piece of chart text in.
const TEXT: Ink = Ink::system(SystemColor::SecondaryText);

impl Chrome {
    /// The chrome for a ground of the given darkness, in fixed colors: the default chrome
    /// resolved against Day's own palette for that appearance. For a chart drawn on a ground
    /// whose darkness is known ahead, such as an export onto white.
    ///
    /// Grid lines are faint: a grid is a reading aid, and one that competes with the data for
    /// contrast is worse than none. Labels and titles share one weight, the secondary-text
    /// grey: two separate weights are bright enough to compete with the marks, and the data is
    /// what should carry the contrast.
    pub fn for_dark(dark: bool) -> Self {
        Chrome::default().resolve(&Palette::fallback(dark))
    }

    /// This chrome with every ink resolved against `palette`: fixed colors for that appearance.
    #[must_use]
    pub fn resolve(&self, palette: &Palette) -> Self {
        let fixed = |ink: Ink| Ink::Fixed(ink.resolve(palette));
        Chrome {
            axis_line: fixed(self.axis_line),
            grid_line: fixed(self.grid_line),
            tick: fixed(self.tick),
            label: fixed(self.label),
            title: fixed(self.title),
            plot_background: self.plot_background.map(fixed),
            surface: fixed(self.surface),
            shadow: fixed(self.shadow),
        }
    }
}

impl Default for Chrome {
    /// Chrome that follows the appearance.
    fn default() -> Self {
        Chrome {
            axis_line: Ink::adaptive(
                Color::rgba(0.0, 0.0, 0.0, 0.35),
                Color::rgba(1.0, 1.0, 1.0, 0.45),
            ),
            grid_line: Ink::adaptive(
                Color::rgba(0.0, 0.0, 0.0, 0.10),
                Color::rgba(1.0, 1.0, 1.0, 0.12),
            ),
            tick: Ink::adaptive(
                Color::rgba(0.0, 0.0, 0.0, 0.35),
                Color::rgba(1.0, 1.0, 1.0, 0.45),
            ),
            label: TEXT,
            title: TEXT,
            plot_background: None,
            surface: Ink::adaptive(Color::WHITE, Color::rgb(0.17, 0.17, 0.19)),
            shadow: Ink::adaptive(
                Color::rgba(0.0, 0.0, 0.0, 0.22),
                Color::rgba(0.0, 0.0, 0.0, 0.55),
            ),
        }
    }
}
