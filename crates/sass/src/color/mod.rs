//! As cores do Sass como o dart-sass 1.102.0 as modela: um espaço de cor
//! (`ColorSpace`), três canais que podem faltar (`none`) e o alfa — porte de
//! `lib/src/value/color.dart` e `lib/src/value/color/**` do dart-sass.
//!
//! O grass guardava só RGB/HSL e arredondava os canais a cada operação
//! (`darken(#36c, 10%)` virava `#2952a3`); o dart-sass 1.79+ não arredonda
//! (`rgb(16%, 32%, 64%)`) e tem os espaços do CSS Color 4 (`lab`, `oklch`,
//! `color(display-p3 …)`…). As contas seguem as do dart-sass passo a passo,
//! com as mesmas funções da libm (`pow`, `atan2`…), para dar os mesmos bits.
//!
//! Semântica do Dart que importa aqui: `a % b` com `b > 0` é sempre não
//! negativo (`rem_euclid`), `double.sign` de zero é zero e `round()`
//! arredonda metades para longe do zero (como o `f64::round`).

use std::fmt;

pub(crate) use name::NAMED_COLORS;

use crate::value::{fuzzy_equals, fuzzy_less_than, fuzzy_less_than_or_equals, Number};

mod matrizes;
mod name;
pub(crate) mod v166;

use matrizes::*;

/// `% ` do Dart para `double`: o resto euclidiano.
pub(crate) fn dart_mod(a: f64, b: f64) -> f64 {
    let r = a % b;
    if r < 0.0 {
        r + b.abs()
    } else if r == 0.0 {
        // O Dart devolve `0.0` (sem sinal) para resto zero.
        0.0
    } else {
        r
    }
}

/// `math.pow` do Dart para `double` (`_doublePow` da VM): os casos
/// especiais dele e expoente `2`/`3` como multiplicação (medido contra o
/// Dart 3.6.2: `pow(x, 3)` dá `x*x*x`, não o `pow` da libm); o resto é o
/// `pow` da libm, como na VM.
pub(crate) fn dart_pow(x: f64, y: f64) -> f64 {
    if y == 0.0 || x == 1.0 {
        return 1.0;
    }
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if x != f64::NEG_INFINITY && y == 0.5 {
        return if x == 0.0 { 0.0 } else { x.sqrt() };
    }
    if y == 2.0 {
        return x * x;
    }
    if y == 3.0 {
        return x * x * x;
    }
    x.powf(y)
}

/// `math.max` do Dart: `NaN` se algum for `NaN` (o `f64::max` o ignora).
pub(crate) fn dart_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// `math.min` do Dart: `NaN` se algum for `NaN`.
pub(crate) fn dart_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// `double.sign` do Dart: `-1`, `0` ou `1` (e `NaN` para `NaN`).
fn dart_sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

pub(crate) fn fuzzy_greater_than_or_equals(a: f64, b: f64) -> bool {
    a > b || fuzzy_equals(a, b)
}

/// `fuzzyInRange`.
pub(crate) fn fuzzy_in_range(n: f64, min: f64, max: f64) -> bool {
    fuzzy_greater_than_or_equals(n, min) && fuzzy_less_than_or_equals(n, max)
}

/// `fuzzyEqualsNullable`.
pub(crate) fn fuzzy_equals_nullable(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => fuzzy_equals(a, b),
        _ => false,
    }
}

/// `clampLikeCss`: `NaN` vira o limite inferior.
pub(crate) fn clamp_like_css(n: f64, lower: f64, upper: f64) -> f64 {
    if n.is_nan() {
        lower
    } else {
        n.clamp(lower, upper)
    }
}

/// Um espaço de cor (`ColorSpace` do dart-sass). `Lms` é interno.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ColorSpace {
    Rgb,
    Hsl,
    Hwb,
    Srgb,
    SrgbLinear,
    DisplayP3,
    DisplayP3Linear,
    A98Rgb,
    ProphotoRgb,
    Rec2020,
    XyzD65,
    XyzD50,
    Lab,
    Lch,
    Lms,
    Oklab,
    Oklch,
}

/// Um canal de um espaço (`ColorChannel`/`LinearChannel`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChannelInfo {
    pub name: &'static str,
    pub is_polar_angle: bool,
    pub associated_unit: Option<&'static str>,
    /// `LinearChannel`: faixa e regras de corte; `false` no canal de matiz.
    pub linear: bool,
    pub min: f64,
    pub max: f64,
    pub requires_percent: bool,
    pub lower_clamped: bool,
    pub upper_clamped: bool,
}

impl ChannelInfo {
    const fn linear(name: &'static str, min: f64, max: f64) -> Self {
        ChannelInfo {
            name,
            is_polar_angle: false,
            // `conventionallyPercent ?? (min == 0 && max == 100)`
            associated_unit: if min == 0.0 && max == 100.0 {
                Some("%")
            } else {
                None
            },
            linear: true,
            min,
            max,
            requires_percent: false,
            lower_clamped: false,
            upper_clamped: false,
        }
    }

    const fn percent(mut self) -> Self {
        self.associated_unit = Some("%");
        self
    }

    const fn requires_percent(mut self) -> Self {
        self.requires_percent = true;
        self
    }

    const fn clamped(mut self, lower: bool, upper: bool) -> Self {
        self.lower_clamped = lower;
        self.upper_clamped = upper;
        self
    }

    const HUE: ChannelInfo = ChannelInfo {
        name: "hue",
        is_polar_angle: true,
        associated_unit: Some("deg"),
        linear: false,
        min: 0.0,
        max: 0.0,
        requires_percent: false,
        lower_clamped: false,
        upper_clamped: false,
    };

    pub const ALPHA: ChannelInfo = ChannelInfo::linear("alpha", 0.0, 1.0);

    /// `isAnalogous`.
    pub fn is_analogous(&self, other: &ChannelInfo) -> bool {
        let grupo = |n: &str| match n {
            "red" | "x" => 1,
            "green" | "y" => 2,
            "blue" | "z" => 3,
            "chroma" | "saturation" => 4,
            "lightness" => 5,
            "hue" => 6,
            _ => 0,
        };
        let a = grupo(self.name);
        a != 0 && a == grupo(other.name)
    }
}

const RGB_CHANNELS: [ChannelInfo; 3] = [
    ChannelInfo::linear("red", 0.0, 1.0),
    ChannelInfo::linear("green", 0.0, 1.0),
    ChannelInfo::linear("blue", 0.0, 1.0),
];

const XYZ_CHANNELS: [ChannelInfo; 3] = [
    ChannelInfo::linear("x", 0.0, 1.0),
    ChannelInfo::linear("y", 0.0, 1.0),
    ChannelInfo::linear("z", 0.0, 1.0),
];

const LAB_KAPPA: f64 = 24389.0 / 27.0;
const LAB_EPSILON: f64 = 216.0 / 24389.0;

impl ColorSpace {
    pub fn name(self) -> &'static str {
        match self {
            ColorSpace::Rgb => "rgb",
            ColorSpace::Hsl => "hsl",
            ColorSpace::Hwb => "hwb",
            ColorSpace::Srgb => "srgb",
            ColorSpace::SrgbLinear => "srgb-linear",
            ColorSpace::DisplayP3 => "display-p3",
            ColorSpace::DisplayP3Linear => "display-p3-linear",
            ColorSpace::A98Rgb => "a98-rgb",
            ColorSpace::ProphotoRgb => "prophoto-rgb",
            ColorSpace::Rec2020 => "rec2020",
            ColorSpace::XyzD65 => "xyz",
            ColorSpace::XyzD50 => "xyz-d50",
            ColorSpace::Lab => "lab",
            ColorSpace::Lch => "lch",
            ColorSpace::Lms => "lms",
            ColorSpace::Oklab => "oklab",
            ColorSpace::Oklch => "oklch",
        }
    }

    /// `ColorSpace.fromName` (sem distinguir maiúsculas).
    pub fn from_name(name: &str) -> Option<ColorSpace> {
        Some(match name.to_ascii_lowercase().as_str() {
            "rgb" => ColorSpace::Rgb,
            "hwb" => ColorSpace::Hwb,
            "hsl" => ColorSpace::Hsl,
            "srgb" => ColorSpace::Srgb,
            "srgb-linear" => ColorSpace::SrgbLinear,
            "display-p3" => ColorSpace::DisplayP3,
            "display-p3-linear" => ColorSpace::DisplayP3Linear,
            "a98-rgb" => ColorSpace::A98Rgb,
            "prophoto-rgb" => ColorSpace::ProphotoRgb,
            "rec2020" => ColorSpace::Rec2020,
            "xyz" | "xyz-d65" => ColorSpace::XyzD65,
            "xyz-d50" => ColorSpace::XyzD50,
            "lab" => ColorSpace::Lab,
            "lch" => ColorSpace::Lch,
            "oklab" => ColorSpace::Oklab,
            "oklch" => ColorSpace::Oklch,
            _ => return None,
        })
    }

    pub fn channels(self) -> [ChannelInfo; 3] {
        match self {
            ColorSpace::Rgb => [
                ChannelInfo::linear("red", 0.0, 255.0).clamped(true, true),
                ChannelInfo::linear("green", 0.0, 255.0).clamped(true, true),
                ChannelInfo::linear("blue", 0.0, 255.0).clamped(true, true),
            ],
            ColorSpace::Hsl => [
                ChannelInfo::HUE,
                ChannelInfo::linear("saturation", 0.0, 100.0)
                    .requires_percent()
                    .clamped(true, false),
                ChannelInfo::linear("lightness", 0.0, 100.0).requires_percent(),
            ],
            ColorSpace::Hwb => [
                ChannelInfo::HUE,
                ChannelInfo::linear("whiteness", 0.0, 100.0).requires_percent(),
                ChannelInfo::linear("blackness", 0.0, 100.0).requires_percent(),
            ],
            ColorSpace::Srgb
            | ColorSpace::SrgbLinear
            | ColorSpace::DisplayP3
            | ColorSpace::DisplayP3Linear
            | ColorSpace::A98Rgb
            | ColorSpace::ProphotoRgb
            | ColorSpace::Rec2020 => RGB_CHANNELS,
            ColorSpace::XyzD65 | ColorSpace::XyzD50 => XYZ_CHANNELS,
            ColorSpace::Lab => [
                ChannelInfo::linear("lightness", 0.0, 100.0).clamped(true, true),
                ChannelInfo::linear("a", -125.0, 125.0),
                ChannelInfo::linear("b", -125.0, 125.0),
            ],
            ColorSpace::Lch => [
                ChannelInfo::linear("lightness", 0.0, 100.0).clamped(true, true),
                ChannelInfo::linear("chroma", 0.0, 150.0).clamped(true, false),
                ChannelInfo::HUE,
            ],
            ColorSpace::Lms => [
                ChannelInfo::linear("long", 0.0, 1.0),
                ChannelInfo::linear("medium", 0.0, 1.0),
                ChannelInfo::linear("short", 0.0, 1.0),
            ],
            ColorSpace::Oklab => [
                ChannelInfo::linear("lightness", 0.0, 1.0)
                    .percent()
                    .clamped(true, true),
                ChannelInfo::linear("a", -0.4, 0.4),
                ChannelInfo::linear("b", -0.4, 0.4),
            ],
            ColorSpace::Oklch => [
                ChannelInfo::linear("lightness", 0.0, 1.0)
                    .percent()
                    .clamped(true, true),
                ChannelInfo::linear("chroma", 0.0, 0.4).clamped(true, false),
                ChannelInfo::HUE,
            ],
        }
    }

    pub fn is_bounded(self) -> bool {
        !matches!(
            self,
            ColorSpace::XyzD65
                | ColorSpace::XyzD50
                | ColorSpace::Lab
                | ColorSpace::Lch
                | ColorSpace::Lms
                | ColorSpace::Oklab
                | ColorSpace::Oklch
        )
    }

    pub fn is_legacy(self) -> bool {
        matches!(self, ColorSpace::Rgb | ColorSpace::Hsl | ColorSpace::Hwb)
    }

    pub fn is_polar(self) -> bool {
        matches!(
            self,
            ColorSpace::Hsl | ColorSpace::Hwb | ColorSpace::Lch | ColorSpace::Oklch
        )
    }

    fn to_linear(self, c: f64) -> f64 {
        match self {
            ColorSpace::Rgb => srgb_to_linear(c / 255.0),
            ColorSpace::Srgb | ColorSpace::DisplayP3 => srgb_to_linear(c),
            ColorSpace::A98Rgb => dart_sign(c) * dart_pow(c.abs(), 563.0 / 256.0),
            ColorSpace::ProphotoRgb => {
                let abs = c.abs();
                if abs <= 16.0 / 512.0 {
                    c / 16.0
                } else {
                    dart_sign(c) * dart_pow(abs, 1.8)
                }
            }
            ColorSpace::Rec2020 => dart_sign(c) * dart_pow(c.abs(), 2.40),
            _ => c,
        }
    }

    fn from_linear(self, c: f64) -> f64 {
        match self {
            ColorSpace::Rgb => srgb_from_linear(c) * 255.0,
            ColorSpace::Srgb | ColorSpace::DisplayP3 => srgb_from_linear(c),
            ColorSpace::A98Rgb => dart_sign(c) * dart_pow(c.abs(), 256.0 / 563.0),
            ColorSpace::ProphotoRgb => {
                let abs = c.abs();
                if abs >= 1.0 / 512.0 {
                    dart_sign(c) * dart_pow(abs, 1.0 / 1.8)
                } else {
                    16.0 * c
                }
            }
            ColorSpace::Rec2020 => dart_sign(c) * dart_pow(c.abs(), 1.0 / 2.40),
            _ => c,
        }
    }

    /// `transformationMatrix(dest)` de cada espaço linear.
    fn transformation_matrix(self, dest: ColorSpace) -> &'static [f64; 9] {
        use ColorSpace::*;
        let srgb_like = matches!(dest, SrgbLinear | Srgb | Rgb);
        let p3_like = matches!(dest, DisplayP3 | DisplayP3Linear);
        match self {
            Srgb | SrgbLinear => match dest {
                DisplayP3 | DisplayP3Linear => &LINEAR_SRGB_TO_LINEAR_DISPLAY_P3,
                A98Rgb => &LINEAR_SRGB_TO_LINEAR_A98_RGB,
                ProphotoRgb => &LINEAR_SRGB_TO_LINEAR_PROPHOTO_RGB,
                Rec2020 => &LINEAR_SRGB_TO_LINEAR_REC2020,
                XyzD65 => &LINEAR_SRGB_TO_XYZ_D65,
                XyzD50 => &LINEAR_SRGB_TO_XYZ_D50,
                Lms => &LINEAR_SRGB_TO_LMS,
                _ => unreachable!("conversão de {self:?} para {dest:?}"),
            },
            DisplayP3 | DisplayP3Linear => match dest {
                _ if srgb_like => &LINEAR_DISPLAY_P3_TO_LINEAR_SRGB,
                A98Rgb => &LINEAR_DISPLAY_P3_TO_LINEAR_A98_RGB,
                ProphotoRgb => &LINEAR_DISPLAY_P3_TO_LINEAR_PROPHOTO_RGB,
                Rec2020 => &LINEAR_DISPLAY_P3_TO_LINEAR_REC2020,
                XyzD65 => &LINEAR_DISPLAY_P3_TO_XYZ_D65,
                XyzD50 => &LINEAR_DISPLAY_P3_TO_XYZ_D50,
                Lms => &LINEAR_DISPLAY_P3_TO_LMS,
                _ => unreachable!("conversão de {self:?} para {dest:?}"),
            },
            A98Rgb => match dest {
                _ if srgb_like => &LINEAR_A98_RGB_TO_LINEAR_SRGB,
                _ if p3_like => &LINEAR_A98_RGB_TO_LINEAR_DISPLAY_P3,
                ProphotoRgb => &LINEAR_A98_RGB_TO_LINEAR_PROPHOTO_RGB,
                Rec2020 => &LINEAR_A98_RGB_TO_LINEAR_REC2020,
                XyzD65 => &LINEAR_A98_RGB_TO_XYZ_D65,
                XyzD50 => &LINEAR_A98_RGB_TO_XYZ_D50,
                Lms => &LINEAR_A98_RGB_TO_LMS,
                _ => unreachable!("conversão de {self:?} para {dest:?}"),
            },
            ProphotoRgb => match dest {
                _ if srgb_like => &LINEAR_PROPHOTO_RGB_TO_LINEAR_SRGB,
                A98Rgb => &LINEAR_PROPHOTO_RGB_TO_LINEAR_A98_RGB,
                _ if p3_like => &LINEAR_PROPHOTO_RGB_TO_LINEAR_DISPLAY_P3,
                Rec2020 => &LINEAR_PROPHOTO_RGB_TO_LINEAR_REC2020,
                XyzD65 => &LINEAR_PROPHOTO_RGB_TO_XYZ_D65,
                XyzD50 => &LINEAR_PROPHOTO_RGB_TO_XYZ_D50,
                Lms => &LINEAR_PROPHOTO_RGB_TO_LMS,
                _ => unreachable!("conversão de {self:?} para {dest:?}"),
            },
            Rec2020 => match dest {
                _ if srgb_like => &LINEAR_REC2020_TO_LINEAR_SRGB,
                A98Rgb => &LINEAR_REC2020_TO_LINEAR_A98_RGB,
                _ if p3_like => &LINEAR_REC2020_TO_LINEAR_DISPLAY_P3,
                ProphotoRgb => &LINEAR_REC2020_TO_LINEAR_PROPHOTO_RGB,
                XyzD65 => &LINEAR_REC2020_TO_XYZ_D65,
                XyzD50 => &LINEAR_REC2020_TO_XYZ_D50,
                Lms => &LINEAR_REC2020_TO_LMS,
                _ => unreachable!("conversão de {self:?} para {dest:?}"),
            },
            XyzD65 => match dest {
                _ if srgb_like => &XYZ_D65_TO_LINEAR_SRGB,
                A98Rgb => &XYZ_D65_TO_LINEAR_A98_RGB,
                ProphotoRgb => &XYZ_D65_TO_LINEAR_PROPHOTO_RGB,
                _ if p3_like => &XYZ_D65_TO_LINEAR_DISPLAY_P3,
                Rec2020 => &XYZ_D65_TO_LINEAR_REC2020,
                XyzD50 => &XYZ_D65_TO_XYZ_D50,
                Lms => &XYZ_D65_TO_LMS,
                _ => unreachable!("conversão de {self:?} para {dest:?}"),
            },
            XyzD50 => match dest {
                _ if srgb_like => &XYZ_D50_TO_LINEAR_SRGB,
                A98Rgb => &XYZ_D50_TO_LINEAR_A98_RGB,
                ProphotoRgb => &XYZ_D50_TO_LINEAR_PROPHOTO_RGB,
                _ if p3_like => &XYZ_D50_TO_LINEAR_DISPLAY_P3,
                Rec2020 => &XYZ_D50_TO_LINEAR_REC2020,
                XyzD65 => &XYZ_D50_TO_XYZ_D65,
                Lms => &XYZ_D50_TO_LMS,
                _ => unreachable!("conversão de {self:?} para {dest:?}"),
            },
            Lms => match dest {
                _ if srgb_like => &LMS_TO_LINEAR_SRGB,
                A98Rgb => &LMS_TO_LINEAR_A98_RGB,
                ProphotoRgb => &LMS_TO_LINEAR_PROPHOTO_RGB,
                _ if p3_like => &LMS_TO_LINEAR_DISPLAY_P3,
                Rec2020 => &LMS_TO_LINEAR_REC2020,
                XyzD65 => &LMS_TO_XYZ_D65,
                XyzD50 => &LMS_TO_XYZ_D50,
                _ => unreachable!("conversão de {self:?} para {dest:?}"),
            },
            _ => unreachable!("conversão linear de {self:?}"),
        }
    }

    /// `ColorSpace.convert` (com o despacho de cada subclasse).
    pub fn convert(
        self,
        dest: ColorSpace,
        c0: Option<f64>,
        c1: Option<f64>,
        c2: Option<f64>,
        alpha: Option<f64>,
    ) -> Color {
        match self {
            ColorSpace::Rgb => ColorSpace::Srgb.convert(
                dest,
                c0.map(|c| c / 255.0),
                c1.map(|c| c / 255.0),
                c2.map(|c| c / 255.0),
                alpha,
            ),
            ColorSpace::Hsl => {
                let (hue, saturation, lightness) = (c0, c1, c2);
                let scaled_hue = dart_mod(hue.unwrap_or(0.0) / 360.0, 1.0);
                let scaled_saturation = saturation.unwrap_or(0.0) / 100.0;
                let scaled_lightness = lightness.unwrap_or(0.0) / 100.0;
                let m2 = if scaled_lightness <= 0.5 {
                    scaled_lightness * (scaled_saturation + 1.0)
                } else {
                    scaled_lightness + scaled_saturation - scaled_lightness * scaled_saturation
                };
                let m1 = scaled_lightness * 2.0 - m2;
                srgb_convert(
                    dest,
                    Some(hue_to_rgb(m1, m2, scaled_hue + 1.0 / 3.0)),
                    Some(hue_to_rgb(m1, m2, scaled_hue)),
                    Some(hue_to_rgb(m1, m2, scaled_hue - 1.0 / 3.0)),
                    alpha,
                    lightness.is_none(),
                    saturation.is_none(),
                    hue.is_none(),
                )
            }
            ColorSpace::Hwb => {
                let hue = c0;
                let scaled_hue = dart_mod(hue.unwrap_or(0.0), 360.0) / 360.0;
                let mut scaled_whiteness = c1.unwrap_or(0.0) / 100.0;
                let mut scaled_blackness = c2.unwrap_or(0.0) / 100.0;
                let sum = scaled_whiteness + scaled_blackness;
                if sum > 1.0 {
                    scaled_whiteness /= sum;
                    scaled_blackness /= sum;
                }
                let factor = 1.0 - scaled_whiteness - scaled_blackness;
                let to_rgb = |h: f64| hue_to_rgb(0.0, 1.0, h) * factor + scaled_whiteness;
                srgb_convert(
                    dest,
                    Some(to_rgb(scaled_hue + 1.0 / 3.0)),
                    Some(to_rgb(scaled_hue)),
                    Some(to_rgb(scaled_hue - 1.0 / 3.0)),
                    alpha,
                    false,
                    false,
                    hue.is_none(),
                )
            }
            ColorSpace::Srgb => srgb_convert(dest, c0, c1, c2, alpha, false, false, false),
            ColorSpace::SrgbLinear => match dest {
                ColorSpace::Rgb | ColorSpace::Hsl | ColorSpace::Hwb | ColorSpace::Srgb => {
                    ColorSpace::Srgb.convert(
                        dest,
                        c0.map(srgb_from_linear),
                        c1.map(srgb_from_linear),
                        c2.map(srgb_from_linear),
                        alpha,
                    )
                }
                _ => convert_linear(self, dest, c0, c1, c2, alpha, Faltas::default()),
            },
            ColorSpace::DisplayP3 => {
                if dest == ColorSpace::DisplayP3Linear {
                    Color::for_space_internal(
                        dest,
                        c0.map(srgb_to_linear),
                        c1.map(srgb_to_linear),
                        c2.map(srgb_to_linear),
                        alpha,
                    )
                } else {
                    convert_linear(self, dest, c0, c1, c2, alpha, Faltas::default())
                }
            }
            ColorSpace::DisplayP3Linear => {
                if dest == ColorSpace::DisplayP3 {
                    Color::for_space_internal(
                        dest,
                        c0.map(srgb_from_linear),
                        c1.map(srgb_from_linear),
                        c2.map(srgb_from_linear),
                        alpha,
                    )
                } else {
                    convert_linear(self, dest, c0, c1, c2, alpha, Faltas::default())
                }
            }
            ColorSpace::A98Rgb
            | ColorSpace::ProphotoRgb
            | ColorSpace::Rec2020
            | ColorSpace::XyzD65 => {
                convert_linear(self, dest, c0, c1, c2, alpha, Faltas::default())
            }
            ColorSpace::XyzD50 => xyz_d50_convert(dest, c0, c1, c2, alpha, Faltas::default()),
            ColorSpace::Lab => lab_convert(dest, c0, c1, c2, alpha, false, false),
            ColorSpace::Lch => {
                let (lightness, chroma, hue) = (c0, c1, c2);
                let hue_radians = hue.unwrap_or(0.0) * std::f64::consts::PI / 180.0;
                lab_convert(
                    dest,
                    lightness,
                    Some(chroma.unwrap_or(0.0) * hue_radians.cos()),
                    Some(chroma.unwrap_or(0.0) * hue_radians.sin()),
                    alpha,
                    chroma.is_none(),
                    hue.is_none(),
                )
            }
            ColorSpace::Lms => lms_convert(dest, c0, c1, c2, alpha, Faltas::default()),
            ColorSpace::Oklab => oklab_convert(dest, c0, c1, c2, alpha, false, false),
            ColorSpace::Oklch => {
                let (lightness, chroma, hue) = (c0, c1, c2);
                let hue_radians = hue.unwrap_or(0.0) * std::f64::consts::PI / 180.0;
                oklab_convert(
                    dest,
                    lightness,
                    Some(chroma.unwrap_or(0.0) * hue_radians.cos()),
                    Some(chroma.unwrap_or(0.0) * hue_radians.sin()),
                    alpha,
                    chroma.is_none(),
                    hue.is_none(),
                )
            }
        }
    }
}

impl fmt::Display for ColorSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Os canais que faltavam na origem (`missingLightness` etc. do dart-sass).
#[derive(Debug, Clone, Copy, Default)]
struct Faltas {
    lightness: bool,
    chroma: bool,
    hue: bool,
    a: bool,
    b: bool,
}

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

fn srgb_to_linear(channel: f64) -> f64 {
    let abs = channel.abs();
    if abs <= 0.04045 {
        channel / 12.92
    } else {
        dart_sign(channel) * dart_pow((abs + 0.055) / 1.055, 2.4)
    }
}

fn srgb_from_linear(channel: f64) -> f64 {
    let abs = channel.abs();
    if abs <= 0.0031308 {
        channel * 12.92
    } else {
        dart_sign(channel) * (1.055 * dart_pow(abs, 1.0 / 2.4) - 0.055)
    }
}

/// `convertLinear`.
fn convert_linear(
    from: ColorSpace,
    dest: ColorSpace,
    red: Option<f64>,
    green: Option<f64>,
    blue: Option<f64>,
    alpha: Option<f64>,
    faltas: Faltas,
) -> Color {
    let linear_dest = match dest {
        ColorSpace::Hsl | ColorSpace::Hwb => ColorSpace::Srgb,
        ColorSpace::Lab | ColorSpace::Lch => ColorSpace::XyzD50,
        ColorSpace::Oklab | ColorSpace::Oklch => ColorSpace::Lms,
        _ => dest,
    };
    let (tr, tg, tb) = if linear_dest == from {
        (red, green, blue)
    } else {
        let lr = from.to_linear(red.unwrap_or(0.0));
        let lg = from.to_linear(green.unwrap_or(0.0));
        let lb = from.to_linear(blue.unwrap_or(0.0));
        let m = from.transformation_matrix(linear_dest);
        (
            Some(linear_dest.from_linear(m[0] * lr + m[1] * lg + m[2] * lb)),
            Some(linear_dest.from_linear(m[3] * lr + m[4] * lg + m[5] * lb)),
            Some(linear_dest.from_linear(m[6] * lr + m[7] * lg + m[8] * lb)),
        )
    };
    match dest {
        ColorSpace::Hsl | ColorSpace::Hwb => srgb_convert(
            dest,
            tr,
            tg,
            tb,
            alpha,
            faltas.lightness,
            faltas.chroma,
            faltas.hue,
        ),
        ColorSpace::Lab | ColorSpace::Lch => xyz_d50_convert(dest, tr, tg, tb, alpha, faltas),
        ColorSpace::Oklab | ColorSpace::Oklch => lms_convert(dest, tr, tg, tb, alpha, faltas),
        _ => Color::for_space_internal(dest, red.and(tr), green.and(tg), blue.and(tb), alpha),
    }
}

/// `SrgbColorSpace.convert`.
#[allow(clippy::too_many_arguments)]
fn srgb_convert(
    dest: ColorSpace,
    red: Option<f64>,
    green: Option<f64>,
    blue: Option<f64>,
    alpha: Option<f64>,
    missing_lightness: bool,
    missing_chroma: bool,
    missing_hue: bool,
) -> Color {
    match dest {
        ColorSpace::Hsl | ColorSpace::Hwb => {
            let red = red.unwrap_or(0.0);
            let green = green.unwrap_or(0.0);
            let blue = blue.unwrap_or(0.0);
            let max = dart_max(dart_max(red, green), blue);
            let min = dart_min(dart_min(red, green), blue);
            let delta = max - min;
            let mut hue = if max == min {
                0.0
            } else if max == red {
                60.0 * (green - blue) / delta + 360.0
            } else if max == green {
                60.0 * (blue - red) / delta + 120.0
            } else {
                60.0 * (red - green) / delta + 240.0
            };
            if dest == ColorSpace::Hsl {
                let lightness = (min + max) / 2.0;
                let mut saturation = if lightness == 0.0 || lightness == 1.0 {
                    0.0
                } else {
                    100.0 * (max - lightness) / dart_min(lightness, 1.0 - lightness)
                };
                if saturation < 0.0 {
                    hue += 180.0;
                    saturation = saturation.abs();
                }
                Color::for_space_internal(
                    dest,
                    if missing_hue || fuzzy_equals(saturation, 0.0) {
                        None
                    } else {
                        Some(dart_mod(hue, 360.0))
                    },
                    if missing_chroma {
                        None
                    } else {
                        Some(saturation)
                    },
                    if missing_lightness {
                        None
                    } else {
                        Some(lightness * 100.0)
                    },
                    alpha,
                )
            } else {
                let whiteness = min * 100.0;
                let blackness = 100.0 - max * 100.0;
                Color::for_space_internal(
                    dest,
                    if missing_hue || fuzzy_greater_than_or_equals(whiteness + blackness, 100.0) {
                        None
                    } else {
                        Some(dart_mod(hue, 360.0))
                    },
                    Some(whiteness),
                    Some(blackness),
                    alpha,
                )
            }
        }
        ColorSpace::Rgb => Color::new_raw(
            ColorSpace::Rgb,
            red.map(|r| r * 255.0),
            green.map(|g| g * 255.0),
            blue.map(|b| b * 255.0),
            alpha,
            ColorFormat::Infer,
        ),
        ColorSpace::SrgbLinear => Color::for_space_internal(
            dest,
            red.map(srgb_to_linear),
            green.map(srgb_to_linear),
            blue.map(srgb_to_linear),
            alpha,
        ),
        _ => convert_linear(
            ColorSpace::Srgb,
            dest,
            red,
            green,
            blue,
            alpha,
            Faltas {
                lightness: missing_lightness,
                chroma: missing_chroma,
                hue: missing_hue,
                a: false,
                b: false,
            },
        ),
    }
}

/// `XyzD50ColorSpace.convert`.
fn xyz_d50_convert(
    dest: ColorSpace,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    alpha: Option<f64>,
    faltas: Faltas,
) -> Color {
    match dest {
        ColorSpace::Lab | ColorSpace::Lch => {
            let f = |c: f64| {
                if c > LAB_EPSILON {
                    dart_pow(c, 1.0 / 3.0) + 0.0
                } else {
                    (LAB_KAPPA * c + 16.0) / 116.0
                }
            };
            let f0 = f(x.unwrap_or(0.0) / D50[0]);
            let f1 = f(y.unwrap_or(0.0) / D50[1]);
            let f2 = f(z.unwrap_or(0.0) / D50[2]);
            let lightness = if faltas.lightness {
                None
            } else {
                Some((116.0 * f1) - 16.0)
            };
            let a = 500.0 * (f0 - f1);
            let b = 200.0 * (f1 - f2);
            if dest == ColorSpace::Lab {
                Color::new_raw(
                    ColorSpace::Lab,
                    lightness,
                    if faltas.a { None } else { Some(a) },
                    if faltas.b { None } else { Some(b) },
                    alpha,
                    ColorFormat::Infer,
                )
            } else {
                lab_to_lch(
                    ColorSpace::Lch,
                    lightness,
                    Some(a),
                    Some(b),
                    alpha,
                    faltas.chroma,
                    faltas.hue,
                )
            }
        }
        _ => convert_linear(ColorSpace::XyzD50, dest, x, y, z, alpha, faltas),
    }
}

/// `labToLch`.
fn lab_to_lch(
    dest: ColorSpace,
    lightness: Option<f64>,
    a: Option<f64>,
    b: Option<f64>,
    alpha: Option<f64>,
    missing_chroma: bool,
    missing_hue: bool,
) -> Color {
    let chroma = (dart_pow(a.unwrap_or(0.0), 2.0) + dart_pow(b.unwrap_or(0.0), 2.0)).sqrt();
    let hue = if missing_hue || fuzzy_equals(chroma, 0.0) {
        None
    } else {
        Some(b.unwrap_or(0.0).atan2(a.unwrap_or(0.0)) * 180.0 / std::f64::consts::PI)
    };
    Color::for_space_internal(
        dest,
        lightness,
        if missing_chroma { None } else { Some(chroma) },
        match hue {
            Some(h) if h < 0.0 => Some(h + 360.0),
            h => h,
        },
        alpha,
    )
}

/// `LabColorSpace.convert`.
fn lab_convert(
    dest: ColorSpace,
    lightness: Option<f64>,
    a: Option<f64>,
    b: Option<f64>,
    alpha: Option<f64>,
    missing_chroma: bool,
    missing_hue: bool,
) -> Color {
    match dest {
        ColorSpace::Lab => {
            let powerless = lightness.is_none_or(|l| fuzzy_equals(l, 0.0));
            Color::new_raw(
                ColorSpace::Lab,
                lightness,
                if powerless { None } else { a },
                if powerless { None } else { b },
                alpha,
                ColorFormat::Infer,
            )
        }
        ColorSpace::Lch => lab_to_lch(dest, lightness, a, b, alpha, false, false),
        _ => {
            let missing_lightness = lightness.is_none();
            let lightness = lightness.unwrap_or(0.0);
            let f1 = (lightness + 16.0) / 116.0;
            let f_to_xz = |c: f64| {
                let cubed = dart_pow(c, 3.0) + 0.0;
                if cubed > LAB_EPSILON {
                    cubed
                } else {
                    (116.0 * c - 16.0) / LAB_KAPPA
                }
            };
            let y = if lightness > LAB_KAPPA * LAB_EPSILON {
                dart_pow((lightness + 16.0) / 116.0, 3.0) * 1.0
            } else {
                lightness / LAB_KAPPA
            };
            xyz_d50_convert(
                dest,
                Some(f_to_xz(a.unwrap_or(0.0) / 500.0 + f1) * D50[0]),
                Some(y * D50[1]),
                Some(f_to_xz(f1 - b.unwrap_or(0.0) / 200.0) * D50[2]),
                alpha,
                Faltas {
                    lightness: missing_lightness,
                    chroma: missing_chroma,
                    hue: missing_hue,
                    a: a.is_none(),
                    b: b.is_none(),
                },
            )
        }
    }
}

fn cube_root_preserving_sign(n: f64) -> f64 {
    dart_pow(n.abs(), 1.0 / 3.0) * dart_sign(n)
}

/// `LmsColorSpace.convert`.
fn lms_convert(
    dest: ColorSpace,
    long: Option<f64>,
    medium: Option<f64>,
    short: Option<f64>,
    alpha: Option<f64>,
    faltas: Faltas,
) -> Color {
    match dest {
        ColorSpace::Oklab | ColorSpace::Oklch => {
            let l = cube_root_preserving_sign(long.unwrap_or(0.0));
            let m = cube_root_preserving_sign(medium.unwrap_or(0.0));
            let s = cube_root_preserving_sign(short.unwrap_or(0.0));
            let lightness = LMS_TO_OKLAB[0] * l + LMS_TO_OKLAB[1] * m + LMS_TO_OKLAB[2] * s;
            let a = LMS_TO_OKLAB[3] * l + LMS_TO_OKLAB[4] * m + LMS_TO_OKLAB[5] * s;
            let b = LMS_TO_OKLAB[6] * l + LMS_TO_OKLAB[7] * m + LMS_TO_OKLAB[8] * s;
            if dest == ColorSpace::Oklab {
                Color::new_raw(
                    ColorSpace::Oklab,
                    if faltas.lightness {
                        None
                    } else {
                        Some(lightness)
                    },
                    if faltas.a { None } else { Some(a) },
                    if faltas.b { None } else { Some(b) },
                    alpha,
                    ColorFormat::Infer,
                )
            } else {
                lab_to_lch(
                    dest,
                    if faltas.lightness {
                        None
                    } else {
                        Some(lightness)
                    },
                    Some(a),
                    Some(b),
                    alpha,
                    faltas.chroma,
                    faltas.hue,
                )
            }
        }
        _ => convert_linear(ColorSpace::Lms, dest, long, medium, short, alpha, faltas),
    }
}

/// `OklabColorSpace.convert`.
fn oklab_convert(
    dest: ColorSpace,
    lightness: Option<f64>,
    a: Option<f64>,
    b: Option<f64>,
    alpha: Option<f64>,
    missing_chroma: bool,
    missing_hue: bool,
) -> Color {
    if dest == ColorSpace::Oklch {
        return lab_to_lch(dest, lightness, a, b, alpha, missing_chroma, missing_hue);
    }
    let faltas = Faltas {
        lightness: lightness.is_none(),
        chroma: missing_chroma,
        hue: missing_hue,
        a: a.is_none(),
        b: b.is_none(),
    };
    let l = lightness.unwrap_or(0.0);
    let a = a.unwrap_or(0.0);
    let b = b.unwrap_or(0.0);
    let m = &OKLAB_TO_LMS;
    lms_convert(
        dest,
        Some(dart_pow(m[0] * l + m[1] * a + m[2] * b, 3.0) + 0.0),
        Some(dart_pow(m[3] * l + m[4] * a + m[5] * b, 3.0) + 0.0),
        Some(dart_pow(m[6] * l + m[7] * a + m[8] * b, 3.0) + 0.0),
        alpha,
        faltas,
    )
}

/// Em que formato a cor foi escrita, para o `expanded` a reescrever igual
/// (`ColorFormat` do dart-sass; só para cores `rgb`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ColorFormat {
    /// `rgb()`/`rgba()` (`ColorFormat.rgbFunction`).
    RgbFunction,
    /// O texto original (`SpanColorFormat`): nome ou hexadecimal.
    Literal(String),
    /// Nenhum.
    Infer,
    /// `hsl()`/`hsla()` do dart-sass 1.66 (`ColorFormat.hslFunction`), só
    /// no modo 1.66: o 1.102 não tem este formato.
    HslFunction,
}

/// Uma cor (`SassColor`).
#[derive(Debug, Clone)]
pub struct Color {
    space: ColorSpace,
    c0: Option<f64>,
    c1: Option<f64>,
    c2: Option<f64>,
    alpha: Option<f64>,
    pub(crate) format: ColorFormat,
    /// Criada por uma função de cor do modo 1.66 ([`v166`]): compara e
    /// serializa pelo modelo do dart-sass 1.66.
    pub(crate) modelo_166: bool,
}

/// Métodos de interpolação de matiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HueInterpolation {
    Shorter,
    Longer,
    Increasing,
    Decreasing,
}

impl HueInterpolation {
    pub fn name(self) -> &'static str {
        match self {
            HueInterpolation::Shorter => "shorter",
            HueInterpolation::Longer => "longer",
            HueInterpolation::Increasing => "increasing",
            HueInterpolation::Decreasing => "decreasing",
        }
    }
}

/// `InterpolationMethod`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InterpolationMethod {
    pub space: ColorSpace,
    pub hue: Option<HueInterpolation>,
}

impl InterpolationMethod {
    pub fn new(space: ColorSpace, hue: Option<HueInterpolation>) -> Self {
        InterpolationMethod {
            space,
            hue: if space.is_polar() {
                Some(hue.unwrap_or(HueInterpolation::Shorter))
            } else {
                None
            },
        }
    }
}

/// `GamutMapMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GamutMapMethod {
    Clip,
    LocalMinde,
}

impl Color {
    /// `SassColor._forSpace`: sem normalizar nada (o alfa é conferido na
    /// faixa `[0, 1]`, com a tolerância do dart-sass).
    pub(crate) fn new_raw(
        space: ColorSpace,
        c0: Option<f64>,
        c1: Option<f64>,
        c2: Option<f64>,
        alpha: Option<f64>,
        format: ColorFormat,
    ) -> Color {
        let alpha = alpha.map(|a| {
            if fuzzy_equals(a, 0.0) {
                0.0
            } else if fuzzy_equals(a, 1.0) {
                1.0
            } else {
                a.clamp(0.0, 1.0)
            }
        });
        Color {
            space,
            c0,
            c1,
            c2,
            alpha,
            format,
            modelo_166: false,
        }
    }

    /// `SassColor.rgbInternal`.
    pub(crate) fn rgb_internal(
        r: Option<f64>,
        g: Option<f64>,
        b: Option<f64>,
        alpha: Option<f64>,
        format: ColorFormat,
    ) -> Color {
        Color::new_raw(ColorSpace::Rgb, r, g, b, alpha, format)
    }

    /// `SassColor.forSpaceInternal`: normaliza a matiz e tira o sinal da
    /// saturação/croma nos espaços polares.
    pub(crate) fn for_space_internal(
        space: ColorSpace,
        c0: Option<f64>,
        c1: Option<f64>,
        c2: Option<f64>,
        alpha: Option<f64>,
    ) -> Color {
        let inverte = |c: Option<f64>| c.is_some_and(|c| fuzzy_less_than(c, 0.0));
        match space {
            ColorSpace::Hsl => Color::new_raw(
                space,
                normalize_hue(c0, inverte(c1)),
                c1.map(f64::abs),
                c2,
                alpha,
                ColorFormat::Infer,
            ),
            ColorSpace::Hwb => Color::new_raw(
                space,
                normalize_hue(c0, false),
                c1,
                c2,
                alpha,
                ColorFormat::Infer,
            ),
            ColorSpace::Lch | ColorSpace::Oklch => Color::new_raw(
                space,
                c0,
                c1.map(f64::abs),
                normalize_hue(c2, inverte(c1)),
                alpha,
                ColorFormat::Infer,
            ),
            _ => Color::new_raw(space, c0, c1, c2, alpha, ColorFormat::Infer),
        }
    }

    pub(crate) fn space(&self) -> ColorSpace {
        self.space
    }

    pub(crate) fn channels_or_null(&self) -> [Option<f64>; 3] {
        [self.c0, self.c1, self.c2]
    }

    pub(crate) fn channel0(&self) -> f64 {
        self.c0.unwrap_or(0.0)
    }

    pub(crate) fn channel1(&self) -> f64 {
        self.c1.unwrap_or(0.0)
    }

    pub(crate) fn channel2(&self) -> f64 {
        self.c2.unwrap_or(0.0)
    }

    pub(crate) fn channel0_or_null(&self) -> Option<f64> {
        self.c0
    }

    pub(crate) fn channel1_or_null(&self) -> Option<f64> {
        self.c1
    }

    pub(crate) fn channel2_or_null(&self) -> Option<f64> {
        self.c2
    }

    pub(crate) fn alpha_or_null(&self) -> Option<f64> {
        self.alpha
    }

    /// O alfa (`0` quando falta), como `Number` para o resto do grass.
    pub fn alpha(&self) -> Number {
        Number(self.alpha.unwrap_or(0.0))
    }

    pub(crate) fn alpha_f64(&self) -> f64 {
        self.alpha.unwrap_or(0.0)
    }

    pub(crate) fn is_legacy(&self) -> bool {
        self.space.is_legacy()
    }

    pub(crate) fn is_channel0_missing(&self) -> bool {
        self.c0.is_none()
    }

    pub(crate) fn is_channel1_missing(&self) -> bool {
        self.c1.is_none()
    }

    pub(crate) fn is_channel2_missing(&self) -> bool {
        self.c2.is_none()
    }

    pub(crate) fn has_missing_channel(&self) -> bool {
        self.c0.is_none() || self.c1.is_none() || self.c2.is_none() || self.alpha.is_none()
    }

    fn is_channel0_powerless(&self) -> bool {
        match self.space {
            ColorSpace::Hsl => fuzzy_equals(self.channel1(), 0.0),
            ColorSpace::Hwb => {
                fuzzy_greater_than_or_equals(self.channel1() + self.channel2(), 100.0)
            }
            _ => false,
        }
    }

    fn is_channel2_powerless(&self) -> bool {
        match self.space {
            ColorSpace::Lch | ColorSpace::Oklch => fuzzy_equals(self.channel1(), 0.0),
            _ => false,
        }
    }

    /// `isInGamut`.
    pub(crate) fn is_in_gamut(&self) -> bool {
        if !self.space.is_bounded() {
            return true;
        }
        let ch = self.space.channels();
        let dentro = |v: f64, c: &ChannelInfo| {
            !c.linear
                || (fuzzy_less_than_or_equals(v, c.max) && fuzzy_greater_than_or_equals(v, c.min))
        };
        dentro(self.channel0(), &ch[0])
            && dentro(self.channel1(), &ch[1])
            && dentro(self.channel2(), &ch[2])
    }

    /// O índice do canal `name` no espaço (ou 3 para `alpha`).
    fn channel_index(&self, name: &str) -> Option<usize> {
        if name == "alpha" {
            return Some(3);
        }
        self.space.channels().iter().position(|c| c.name == name)
    }

    /// `channel(name)`: `None` se o espaço não tem o canal.
    pub(crate) fn channel(&self, name: &str) -> Option<f64> {
        Some(match self.channel_index(name)? {
            0 => self.channel0(),
            1 => self.channel1(),
            2 => self.channel2(),
            _ => self.alpha_f64(),
        })
    }

    /// `isChannelMissing(name)`.
    pub(crate) fn is_channel_missing(&self, name: &str) -> Option<bool> {
        Some(match self.channel_index(name)? {
            0 => self.c0.is_none(),
            1 => self.c1.is_none(),
            2 => self.c2.is_none(),
            _ => self.alpha.is_none(),
        })
    }

    /// `isChannelPowerless(name)`.
    pub(crate) fn is_channel_powerless(&self, name: &str) -> Option<bool> {
        Some(match self.channel_index(name)? {
            0 => self.is_channel0_powerless(),
            1 => false,
            2 => self.is_channel2_powerless(),
            _ => false,
        })
    }

    /// `_legacyChannel(space, name)` (quem chama garante cor legada).
    pub(crate) fn legacy_channel(&self, space: ColorSpace, name: &str) -> f64 {
        self.to_space(space, true).channel(name).unwrap_or(0.0)
    }

    /// `toSpace(space, legacyMissing:)`.
    pub(crate) fn to_space(&self, space: ColorSpace, legacy_missing: bool) -> Color {
        if self.space == space {
            return self.clone();
        }
        // O `alpha` aqui é o getter do Dart: `0` quando falta.
        let converted =
            self.space
                .convert(space, self.c0, self.c1, self.c2, Some(self.alpha_f64()));
        if !legacy_missing && converted.is_legacy() && converted.has_missing_channel() {
            Color::for_space_internal(
                converted.space,
                Some(converted.channel0()),
                Some(converted.channel1()),
                Some(converted.channel2()),
                Some(converted.alpha_f64()),
            )
        } else {
            converted
        }
    }

    /// `toGamut(method)`.
    pub(crate) fn to_gamut(&self, method: GamutMapMethod) -> Color {
        if self.is_in_gamut() {
            return self.clone();
        }
        match method {
            GamutMapMethod::Clip => self.clip(),
            GamutMapMethod::LocalMinde => self.local_minde(),
        }
    }

    fn clip(&self) -> Color {
        let ch = self.space.channels();
        let clamp = |v: Option<f64>, c: &ChannelInfo| {
            v.map(|v| {
                if c.linear {
                    clamp_like_css(v, c.min, c.max)
                } else {
                    v
                }
            })
        };
        Color::for_space_internal(
            self.space,
            clamp(self.c0, &ch[0]),
            clamp(self.c1, &ch[1]),
            clamp(self.c2, &ch[2]),
            self.alpha,
        )
    }

    fn local_minde(&self) -> Color {
        const JND: f64 = 0.02;
        const EPSILON: f64 = 0.0001;
        let origin = self.to_space(ColorSpace::Oklch, true);
        let lightness = origin.c0;
        let hue = origin.c2;
        let alpha = origin.alpha;
        if fuzzy_greater_than_or_equals(lightness.unwrap_or(0.0), 1.0) {
            return if self.is_legacy() {
                Color::rgb_internal(
                    Some(255.0),
                    Some(255.0),
                    Some(255.0),
                    self.alpha,
                    ColorFormat::Infer,
                )
                .to_space(self.space, true)
            } else {
                Color::for_space_internal(self.space, Some(1.0), Some(1.0), Some(1.0), self.alpha)
            };
        } else if fuzzy_less_than_or_equals(lightness.unwrap_or(0.0), 0.0) {
            return Color::rgb_internal(
                Some(0.0),
                Some(0.0),
                Some(0.0),
                self.alpha,
                ColorFormat::Infer,
            )
            .to_space(self.space, true);
        }
        let mut clipped = self.to_gamut(GamutMapMethod::Clip);
        if delta_eok(&clipped, self) < JND {
            return clipped;
        }
        let mut min = 0.0;
        let mut max = origin.channel1();
        let mut min_in_gamut = true;
        while max - min > EPSILON {
            let chroma = (min + max) / 2.0;
            let current =
                ColorSpace::Oklch.convert(self.space, lightness, Some(chroma), hue, alpha);
            if min_in_gamut && current.is_in_gamut() {
                min = chroma;
                continue;
            }
            clipped = current.to_gamut(GamutMapMethod::Clip);
            let e = delta_eok(&clipped, &current);
            if e < JND {
                if JND - e < EPSILON {
                    return clipped;
                }
                min_in_gamut = false;
                min = chroma;
            } else {
                max = chroma;
            }
        }
        clipped
    }

    /// `changeAlpha`: os canais que faltam viram `0`.
    pub(crate) fn change_alpha(&self, alpha: f64) -> Color {
        Color::for_space_internal(
            self.space,
            Some(self.channel0()),
            Some(self.channel1()),
            Some(self.channel2()),
            Some(alpha),
        )
    }

    /// `changeHsl` com os valores já decididos (legado).
    pub(crate) fn change_hsl(
        &self,
        hue: Option<f64>,
        saturation: Option<f64>,
        lightness: Option<f64>,
    ) -> Color {
        let h = hue.unwrap_or_else(|| self.legacy_channel(ColorSpace::Hsl, "hue"));
        let s = saturation.unwrap_or_else(|| self.legacy_channel(ColorSpace::Hsl, "saturation"));
        let l = lightness.unwrap_or_else(|| self.legacy_channel(ColorSpace::Hsl, "lightness"));
        Color::for_space_internal(
            ColorSpace::Hsl,
            Some(h),
            Some(s),
            Some(l),
            Some(self.alpha_f64()),
        )
        .to_space(self.space, true)
    }

    /// `interpolate`.
    pub(crate) fn interpolate(
        &self,
        other: &Color,
        method: InterpolationMethod,
        weight: f64,
        legacy_missing: bool,
    ) -> Color {
        if fuzzy_equals(weight, 0.0) {
            return other.clone();
        }
        if fuzzy_equals(weight, 1.0) {
            return self.clone();
        }
        let color1 = self.to_space(method.space, true);
        let color2 = other.to_space(method.space, true);
        let m1_0 = analogous_missing(self, &color1, 0);
        let m1_1 = analogous_missing(self, &color1, 1);
        let m1_2 = analogous_missing(self, &color1, 2);
        let m2_0 = analogous_missing(other, &color2, 0);
        let m2_1 = analogous_missing(other, &color2, 1);
        let m2_2 = analogous_missing(other, &color2, 2);
        let c1_0 = if m1_0 {
            color2.channel0()
        } else {
            color1.channel0()
        };
        let c1_1 = if m1_1 {
            color2.channel1()
        } else {
            color1.channel1()
        };
        let c1_2 = if m1_2 {
            color2.channel2()
        } else {
            color1.channel2()
        };
        let c2_0 = if m2_0 {
            color1.channel0()
        } else {
            color2.channel0()
        };
        let c2_1 = if m2_1 {
            color1.channel1()
        } else {
            color2.channel1()
        };
        let c2_2 = if m2_2 {
            color1.channel2()
        } else {
            color2.channel2()
        };
        let alpha1 = self.alpha.unwrap_or(other.alpha_f64());
        let alpha2 = other.alpha.unwrap_or(self.alpha_f64());
        let this_mult = self.alpha.unwrap_or(1.0) * weight;
        let other_mult = other.alpha.unwrap_or(1.0) * (1.0 - weight);
        let mixed_alpha = if self.alpha.is_none() && other.alpha.is_none() {
            None
        } else {
            Some(alpha1 * weight + alpha2 * (1.0 - weight))
        };
        let div = mixed_alpha.unwrap_or(1.0);
        let mix = |m1: bool, m2: bool, a: f64, b: f64| {
            if m1 && m2 {
                None
            } else {
                Some((a * this_mult + b * other_mult) / div)
            }
        };
        let mixed0 = mix(m1_0, m2_0, c1_0, c2_0);
        let mixed1 = mix(m1_1, m2_1, c1_1, c2_1);
        let mixed2 = mix(m1_2, m2_2, c1_2, c2_2);
        let hue = method.hue.unwrap_or(HueInterpolation::Shorter);
        let result = match method.space {
            ColorSpace::Hsl | ColorSpace::Hwb => Color::for_space_internal(
                method.space,
                if m1_0 && m2_0 {
                    None
                } else {
                    Some(interpolate_hues(c1_0, c2_0, hue, weight))
                },
                mixed1,
                mixed2,
                mixed_alpha,
            ),
            ColorSpace::Lch | ColorSpace::Oklch => Color::for_space_internal(
                method.space,
                mixed0,
                mixed1,
                if m1_2 && m2_2 {
                    None
                } else {
                    Some(interpolate_hues(c1_2, c2_2, hue, weight))
                },
                mixed_alpha,
            ),
            _ => Color::for_space_internal(method.space, mixed0, mixed1, mixed2, mixed_alpha),
        };
        result.to_space(self.space, legacy_missing)
    }
}

fn normalize_hue(hue: Option<f64>, invert: bool) -> Option<f64> {
    hue.map(|h| {
        dart_mod(
            dart_mod(h, 360.0) + 360.0 + if invert { 180.0 } else { 0.0 },
            360.0,
        )
    })
}

fn analogous_missing(original: &Color, output: &Color, index: usize) -> bool {
    if output.channels_or_null()[index].is_none() {
        return true;
    }
    if std::ptr::eq(original, output) || (original.space == output.space) {
        // `identical(original, output)`: `toSpace` devolve a própria cor
        // quando o espaço é o mesmo.
        return false;
    }
    let out_ch = output.space.channels()[index];
    match original
        .space
        .channels()
        .iter()
        .find(|c| out_ch.is_analogous(c))
    {
        Some(c) => original.is_channel_missing(c.name).unwrap_or(false),
        None => false,
    }
}

fn interpolate_hues(mut hue1: f64, mut hue2: f64, method: HueInterpolation, weight: f64) -> f64 {
    match method {
        HueInterpolation::Shorter => {
            let d = hue2 - hue1;
            if d > 180.0 {
                hue1 += 360.0;
            } else if d < -180.0 {
                hue2 += 360.0;
            }
        }
        HueInterpolation::Longer => {
            let d = hue2 - hue1;
            if d > 0.0 && d < 180.0 {
                hue2 += 360.0;
            } else if d > -180.0 && d <= 0.0 {
                hue1 += 360.0;
            }
        }
        HueInterpolation::Increasing if hue2 < hue1 => hue2 += 360.0,
        HueInterpolation::Decreasing if hue1 < hue2 => hue1 += 360.0,
        _ => {}
    }
    hue1 * weight + hue2 * (1.0 - weight)
}

fn delta_eok(color1: &Color, color2: &Color) -> f64 {
    let lab1 = color1.to_space(ColorSpace::Oklab, true);
    let lab2 = color2.to_space(ColorSpace::Oklab, true);
    (dart_pow(lab1.channel0() - lab2.channel0(), 2.0)
        + dart_pow(lab1.channel1() - lab2.channel1(), 2.0)
        + dart_pow(lab1.channel2() - lab2.channel2(), 2.0))
    .sqrt()
}

impl PartialEq for Color {
    /// `SassColor.==`: legadas comparam em RGB; as outras, espaço e canais.
    fn eq(&self, other: &Self) -> bool {
        if self.modelo_166 || other.modelo_166 {
            return v166::iguais(self, other);
        }
        if self.is_legacy() {
            if !other.is_legacy() {
                return false;
            }
            if !fuzzy_equals_nullable(self.alpha, other.alpha) {
                return false;
            }
            if self.space == other.space {
                return fuzzy_equals_nullable(self.c0, other.c0)
                    && fuzzy_equals_nullable(self.c1, other.c1)
                    && fuzzy_equals_nullable(self.c2, other.c2);
            }
            return self.to_space(ColorSpace::Rgb, true) == other.to_space(ColorSpace::Rgb, true);
        }
        self.space == other.space
            && fuzzy_equals_nullable(self.c0, other.c0)
            && fuzzy_equals_nullable(self.c1, other.c1)
            && fuzzy_equals_nullable(self.c2, other.c2)
            && fuzzy_equals_nullable(self.alpha, other.alpha)
    }
}

impl Eq for Color {}

impl std::hash::Hash for Color {
    /// `SassColor.hashCode` (com o `fuzzyHashCode`).
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if let Some(h) = v166::hash(self) {
            h.hash(state);
            return;
        }
        let fh = |x: f64| -> i64 {
            if x.is_finite() {
                (x * 1e11).round() as i64
            } else {
                x.to_bits() as i64
            }
        };
        if self.is_legacy() {
            let rgb = self.to_space(ColorSpace::Rgb, true);
            (fh(rgb.channel0()) ^ fh(rgb.channel1()) ^ fh(rgb.channel2()) ^ fh(self.alpha_f64()))
                .hash(state);
        } else {
            self.space.hash(state);
            (fh(self.channel0())
                ^ fh(self.channel1())
                ^ fh(self.channel2())
                ^ fh(self.alpha_f64()))
            .hash(state);
        }
    }
}

impl Color {
    /// Cor de um nome (`colorsByName`) ou de um hexadecimal, com o texto
    /// original (`SpanColorFormat`).
    pub fn new(red: u8, green: u8, blue: u8, alpha: u8, format: String) -> Self {
        Color::rgb_internal(
            Some(f64::from(red)),
            Some(f64::from(green)),
            Some(f64::from(blue)),
            Some(f64::from(alpha) / 255.0),
            ColorFormat::Literal(format),
        )
    }
}

#[cfg(test)]
mod testes_conversao {
    use super::*;

    #[test]
    fn hsl_para_lab_como_o_dart() {
        let c = Color::for_space_internal(
            ColorSpace::Hsl,
            Some(20.0),
            Some(999999.0),
            Some(50.0),
            Some(1.0),
        );
        let lab = c.to_space(ColorSpace::Lab, true);
        let srgb = c.to_space(ColorSpace::Srgb, true);
        let xyz50 = c.to_space(ColorSpace::XyzD50, true);
        let x = lab.to_space(ColorSpace::XyzD65, true);
        eprintln!(
            "{:?}\n{:?}\n{:?}\n{:?}",
            lab.channels_or_null(),
            srgb.channels_or_null(),
            xyz50.channels_or_null(),
            x.channels_or_null()
        );
    }
}
