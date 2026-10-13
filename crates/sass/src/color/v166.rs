//! The color model of dart-sass 1.66.0 (`lib/src/value/color.dart` of that
//! version), for the compatibility mode
//! ([`crate::DartSassVersion::V1_66_0`]).
//!
//! The 1.66 `SassColor` stores the **integer** RGB (0–255) and the HSL: a
//! color created from RGB derives the HSL from the integers (`_rgbToHsl`);
//! one created from HSL keeps the channels as they came and derives the
//! rounded RGB (`_hslToRgb`, with `fuzzyRound`). Here it lives in the 1.102
//! [`Color`] as an `rgb` space with integer channels or an `hsl` space with
//! the 1.66 channels, marked with [`Color::model_166`]; [`rgb`] and [`hsl`]
//! give the 1.66 readings and return `None` for colors that 1.66 would not
//! have (other spaces, missing channels, fractional RGB or outside 0–255),
//! which the mode refuses.
use super::{dart_max, dart_min, dart_mod, Color, ColorFormat, ColorSpace};
use crate::value::{fuzzy_equals, fuzzy_less_than, fuzzy_less_than_or_equals};

/// `fuzzyRound` of 1.66 (`util/number.dart`), with Dart's `%` (Euclidean
/// remainder: `-0.3 % 1 == 0.7`).
pub(crate) fn fuzzy_round(number: f64) -> f64 {
    if number > 0.0 {
        if fuzzy_less_than(dart_mod(number, 1.0), 0.5) {
            number.floor()
        } else {
            number.ceil()
        }
    } else if fuzzy_less_than_or_equals(dart_mod(number, 1.0), 0.5) {
        number.floor()
    } else {
        number.ceil()
    }
}

/// `fuzzyCheckRange`.
fn fuzzy_check_range(n: f64, min: f64, max: f64) -> Option<f64> {
    if fuzzy_equals(n, min) {
        Some(min)
    } else if fuzzy_equals(n, max) {
        Some(max)
    } else {
        (n > min && n < max).then_some(n)
    }
}

/// `fuzzyAssertRange`: Dart's `RangeError` becomes a text error.
pub(crate) fn fuzzy_assert_range(n: f64, min: f64, max: f64, name: &str) -> Result<f64, String> {
    fuzzy_check_range(n, min, max)
        .ok_or_else(|| format!("RangeError ({name}): must be between {min} and {max}: {n}"))
}

fn new_color(space: ColorSpace, c: [f64; 3], alpha: f64, format: ColorFormat) -> Color {
    let mut color = Color::new_raw(
        space,
        Some(c[0]),
        Some(c[1]),
        Some(c[2]),
        Some(alpha),
        format,
    );
    // `new_raw` clamps the alpha to [0, 1] with tolerance; 1.66 keeps the
    // value that passed `fuzzyAssertRange`.
    color.alpha = Some(alpha);
    color.model_166 = true;
    color
}

/// `SassColor.rgbInternal`: integer channels in 0–255 and alpha in [0, 1].
///
/// # Errors
///
/// The 1.66 `RangeError` when a channel or the alpha is out of range.
pub(crate) fn from_rgb(
    r: f64,
    g: f64,
    b: f64,
    alpha: f64,
    format: ColorFormat,
) -> Result<Color, String> {
    let alpha = fuzzy_assert_range(alpha, 0.0, 1.0, "alpha")?;
    for (v, name) in [(r, "red"), (g, "green"), (b, "blue")] {
        if !(0.0..=255.0).contains(&v) || v.fract() != 0.0 {
            return Err(format!(
                "RangeError ({name}): Not in inclusive range 0..255: {v}"
            ));
        }
    }
    Ok(new_color(ColorSpace::Rgb, [r, g, b], alpha, format))
}

/// `SassColor.hslInternal`: the hue modulo 360, saturation and lightness in
/// 0–100.
///
/// # Errors
///
/// The 1.66 `RangeError` outside the ranges.
pub(crate) fn from_hsl(
    h: f64,
    s: f64,
    l: f64,
    alpha: f64,
    format: ColorFormat,
) -> Result<Color, String> {
    let s = fuzzy_assert_range(s, 0.0, 100.0, "saturation")?;
    let l = fuzzy_assert_range(l, 0.0, 100.0, "lightness")?;
    let alpha = fuzzy_assert_range(alpha, 0.0, 1.0, "alpha")?;
    Ok(new_color(
        ColorSpace::Hsl,
        [dart_mod(h, 360.0), s, l],
        alpha,
        format,
    ))
}

/// `SassColor.hwb` of 1.66: converted right away to integer RGB.
///
/// # Errors
///
/// The 1.66 `RangeError` outside the ranges.
pub(crate) fn from_hwb(h: f64, w: f64, b: f64, alpha: f64) -> Result<Color, String> {
    let scaled_hue = dart_mod(h, 360.0) / 360.0;
    let mut sw = fuzzy_assert_range(w, 0.0, 100.0, "whiteness")? / 100.0;
    let mut sb = fuzzy_assert_range(b, 0.0, 100.0, "blackness")? / 100.0;
    let sum = sw + sb;
    if sum > 1.0 {
        sw /= sum;
        sb /= sum;
    }
    let factor = 1.0 - sw - sb;
    let channel = |hue: f64| fuzzy_round((hue_to_rgb(0.0, 1.0, hue) * factor + sw) * 255.0);
    from_rgb(
        channel(scaled_hue + 1.0 / 3.0),
        channel(scaled_hue),
        channel(scaled_hue - 1.0 / 3.0),
        alpha,
        ColorFormat::Infer,
    )
}

/// `SassColor._hueToRgb`.
fn hue_to_rgb(m1: f64, m2: f64, mut hue: f64) -> f64 {
    if hue < 0.0 {
        hue += 1.0;
    }
    if hue > 1.0 {
        hue -= 1.0;
    }
    if hue < 1.0 / 6.0 {
        m1 + (m2 - m1) * hue * 6.0
    } else if hue < 1.0 / 2.0 {
        m2
    } else if hue < 2.0 / 3.0 {
        m1 + (m2 - m1) * (2.0 / 3.0 - hue) * 6.0
    } else {
        m1
    }
}

/// The present channels, or `None` if one is missing.
fn channels(c: &Color) -> Option<[f64; 3]> {
    Some([c.c0?, c.c1?, c.c2?])
}

/// `red`/`green`/`blue` of 1.66: the integers of an RGB color, or the
/// `_hslToRgb` of an HSL one. `None` when 1.66 would not have the color.
pub(crate) fn rgb(c: &Color) -> Option<[f64; 3]> {
    c.alpha?;
    let [a, b, d] = channels(c)?;
    match c.space {
        ColorSpace::Rgb => {
            let mut out = [0.0; 3];
            for (i, v) in [a, b, d].into_iter().enumerate() {
                let r = crate::value::fuzzy_as_int(v)?;
                if !(0..=255).contains(&r) {
                    return None;
                }
                out[i] = r as f64;
            }
            Some(out)
        }
        ColorSpace::Hsl => {
            let (scaled_hue, s, l) = (a / 360.0, b / 100.0, d / 100.0);
            let m2 = if l <= 0.5 {
                l * (s + 1.0)
            } else {
                l + s - l * s
            };
            let m1 = l * 2.0 - m2;
            Some([
                fuzzy_round(hue_to_rgb(m1, m2, scaled_hue + 1.0 / 3.0) * 255.0),
                fuzzy_round(hue_to_rgb(m1, m2, scaled_hue) * 255.0),
                fuzzy_round(hue_to_rgb(m1, m2, scaled_hue - 1.0 / 3.0) * 255.0),
            ])
        }
        _ => None,
    }
}

/// `hue`/`saturation`/`lightness` of 1.66: the channels of an HSL color, or
/// the `_rgbToHsl` of the integers of an RGB one.
pub(crate) fn hsl(c: &Color) -> Option<[f64; 3]> {
    if c.space == ColorSpace::Hsl {
        c.alpha?;
        return channels(c);
    }
    let [r, g, b] = rgb(c)?;
    let (r, g, b) = (r / 255.0, g / 255.0, b / 255.0);
    let max = dart_max(dart_max(r, g), b);
    let min = dart_min(dart_min(r, g), b);
    let delta = max - min;
    let hue = if max == min {
        0.0
    } else if max == r {
        dart_mod(60.0 * (g - b) / delta, 360.0)
    } else if max == g {
        dart_mod(120.0 + 60.0 * (b - r) / delta, 360.0)
    } else {
        dart_mod(240.0 + 60.0 * (r - g) / delta, 360.0)
    };
    let lightness = 50.0 * (max + min);
    let saturation = if max == min {
        0.0
    } else if lightness < 50.0 {
        100.0 * delta / (max + min)
    } else {
        100.0 * delta / (2.0 - max - min)
    };
    Some([hue, saturation, lightness])
}

/// `whiteness`/`blackness` of 1.66 (from the integers).
pub(crate) fn hwb(c: &Color) -> Option<[f64; 2]> {
    let [r, g, b] = rgb(c)?;
    Some([
        dart_min(dart_min(r, g), b) / 255.0 * 100.0,
        100.0 - dart_max(dart_max(r, g), b) / 255.0 * 100.0,
    ])
}

/// `changeAlpha` of 1.66: the same representation, without a format.
///
/// # Errors
///
/// Alpha outside [0, 1], or a color that 1.66 would not have.
pub(crate) fn with_alpha(c: &Color, alpha: f64) -> Result<Color, String> {
    let alpha = fuzzy_assert_range(alpha, 0.0, 1.0, "alpha")?;
    match c.space {
        ColorSpace::Hsl => Ok(new_color(
            ColorSpace::Hsl,
            channels(c).ok_or_else(|| not_guaranteed(c))?,
            alpha,
            ColorFormat::Infer,
        )),
        _ => Ok(new_color(
            ColorSpace::Rgb,
            rgb(c).ok_or_else(|| not_guaranteed(c))?,
            alpha,
            ColorFormat::Infer,
        )),
    }
}

/// The alpha (1.66 has no missing alpha).
pub(crate) fn alpha(c: &Color) -> f64 {
    c.alpha.unwrap_or(0.0)
}

/// The refusal message for a color without a representation in 1.66.
pub(crate) fn not_guaranteed(c: &Color) -> String {
    format!(
        "dartforge-sass (dart-sass 1.66 mode): the color in space {} with \
         channels {:?}/{:?}/{:?} and alpha {:?} does not exist in the 1.66 color \
         model (integer RGB from 0 to 255, or HSL); the 1.66 output is not \
         guaranteed",
        c.space.name(),
        c.c0,
        c.c1,
        c.c2,
        c.alpha
    )
}

/// `SassColor.==` of 1.66: the exact RGB integers and alpha.
pub(crate) fn equals(a: &Color, b: &Color) -> bool {
    match (rgb(a), rgb(b)) {
        (Some(x), Some(y)) => x == y && a.alpha == b.alpha,
        _ => false,
    }
}

/// The hash of a 1.66-model color, consistent with [`equals`] and with the
/// 1.102 hash for RGB colors with integer channels; `None` for the others.
pub(crate) fn hash(c: &Color) -> Option<i64> {
    if !c.model_166 {
        return None;
    }
    let fh = |x: f64| (x * 1e11).round() as i64;
    let [r, g, b] = rgb(c).unwrap_or([0.0; 3]);
    Some(fh(r) ^ fh(g) ^ fh(b) ^ fh(alpha(c)))
}
