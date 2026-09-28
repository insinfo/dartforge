//! O modelo de cor do dart-sass 1.66.0 (`lib/src/value/color.dart` dessa
//! versão), para o modo de compatibilidade
//! ([`crate::VersaoDartSass::V1_66_0`]).
//!
//! O `SassColor` do 1.66 guarda o RGB **inteiro** (0–255) e o HSL: a cor
//! criada por RGB deriva o HSL dos inteiros (`_rgbToHsl`); a criada por HSL
//! guarda os canais como vieram e deriva o RGB arredondado (`_hslToRgb`,
//! com `fuzzyRound`). Aqui ela vive no [`Color`] do 1.102 como espaço `rgb`
//! com canais inteiros ou espaço `hsl` com os canais do 1.66, marcada com
//! [`Color::modelo_166`]; [`rgb`] e [`hsl`] dão as leituras do 1.66 e
//! devolvem `None` para cores que o 1.66 não teria (outros espaços, canais
//! ausentes, RGB fracionário ou fora de 0–255), que o modo recusa.
use super::{dart_max, dart_min, dart_mod, Color, ColorFormat, ColorSpace};
use crate::value::{fuzzy_equals, fuzzy_less_than, fuzzy_less_than_or_equals};

/// `fuzzyRound` do 1.66 (`util/number.dart`), com o `%` do Dart (resto
/// euclidiano: `-0.3 % 1 == 0.7`).
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

/// `fuzzyAssertRange`: o `RangeError` do Dart vira erro de texto.
pub(crate) fn fuzzy_assert_range(n: f64, min: f64, max: f64, nome: &str) -> Result<f64, String> {
    fuzzy_check_range(n, min, max)
        .ok_or_else(|| format!("RangeError ({nome}): must be between {min} and {max}: {n}"))
}

fn nova(space: ColorSpace, c: [f64; 3], alpha: f64, format: ColorFormat) -> Color {
    let mut cor = Color::new_raw(
        space,
        Some(c[0]),
        Some(c[1]),
        Some(c[2]),
        Some(alpha),
        format,
    );
    // O `new_raw` corta o alfa para [0, 1] com tolerância; o 1.66 guarda o
    // valor que passou no `fuzzyAssertRange`.
    cor.alpha = Some(alpha);
    cor.modelo_166 = true;
    cor
}

/// `SassColor.rgbInternal`: canais inteiros em 0–255 e alfa em [0, 1].
///
/// # Erros
///
/// O `RangeError` do 1.66 quando um canal ou o alfa está fora da faixa.
pub(crate) fn de_rgb(
    r: f64,
    g: f64,
    b: f64,
    alpha: f64,
    format: ColorFormat,
) -> Result<Color, String> {
    let alpha = fuzzy_assert_range(alpha, 0.0, 1.0, "alpha")?;
    for (v, nome) in [(r, "red"), (g, "green"), (b, "blue")] {
        if !(0.0..=255.0).contains(&v) || v.fract() != 0.0 {
            return Err(format!(
                "RangeError ({nome}): Not in inclusive range 0..255: {v}"
            ));
        }
    }
    Ok(nova(ColorSpace::Rgb, [r, g, b], alpha, format))
}

/// `SassColor.hslInternal`: a matiz módulo 360, saturação e luminosidade em
/// 0–100.
///
/// # Erros
///
/// O `RangeError` do 1.66 fora das faixas.
pub(crate) fn de_hsl(
    h: f64,
    s: f64,
    l: f64,
    alpha: f64,
    format: ColorFormat,
) -> Result<Color, String> {
    let s = fuzzy_assert_range(s, 0.0, 100.0, "saturation")?;
    let l = fuzzy_assert_range(l, 0.0, 100.0, "lightness")?;
    let alpha = fuzzy_assert_range(alpha, 0.0, 1.0, "alpha")?;
    Ok(nova(
        ColorSpace::Hsl,
        [dart_mod(h, 360.0), s, l],
        alpha,
        format,
    ))
}

/// `SassColor.hwb` do 1.66: convertida na hora para RGB inteiro.
///
/// # Erros
///
/// O `RangeError` do 1.66 fora das faixas.
pub(crate) fn de_hwb(h: f64, w: f64, b: f64, alpha: f64) -> Result<Color, String> {
    let scaled_hue = dart_mod(h, 360.0) / 360.0;
    let mut sw = fuzzy_assert_range(w, 0.0, 100.0, "whiteness")? / 100.0;
    let mut sb = fuzzy_assert_range(b, 0.0, 100.0, "blackness")? / 100.0;
    let soma = sw + sb;
    if soma > 1.0 {
        sw /= soma;
        sb /= soma;
    }
    let fator = 1.0 - sw - sb;
    let canal = |hue: f64| fuzzy_round((hue_to_rgb(0.0, 1.0, hue) * fator + sw) * 255.0);
    de_rgb(
        canal(scaled_hue + 1.0 / 3.0),
        canal(scaled_hue),
        canal(scaled_hue - 1.0 / 3.0),
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

/// Os canais presentes, ou `None` se algum falta.
fn canais(c: &Color) -> Option<[f64; 3]> {
    Some([c.c0?, c.c1?, c.c2?])
}

/// `red`/`green`/`blue` do 1.66: os inteiros de uma cor RGB, ou o
/// `_hslToRgb` de uma HSL. `None` quando o 1.66 não teria a cor.
pub(crate) fn rgb(c: &Color) -> Option<[f64; 3]> {
    c.alpha?;
    let [a, b, d] = canais(c)?;
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

/// `hue`/`saturation`/`lightness` do 1.66: os canais de uma cor HSL, ou o
/// `_rgbToHsl` dos inteiros de uma RGB.
pub(crate) fn hsl(c: &Color) -> Option<[f64; 3]> {
    if c.space == ColorSpace::Hsl {
        c.alpha?;
        return canais(c);
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

/// `whiteness`/`blackness` do 1.66 (dos inteiros).
pub(crate) fn hwb(c: &Color) -> Option<[f64; 2]> {
    let [r, g, b] = rgb(c)?;
    Some([
        dart_min(dart_min(r, g), b) / 255.0 * 100.0,
        100.0 - dart_max(dart_max(r, g), b) / 255.0 * 100.0,
    ])
}

/// `changeAlpha` do 1.66: a mesma representação, sem formato.
///
/// # Erros
///
/// Alfa fora de [0, 1], ou cor que o 1.66 não teria.
pub(crate) fn com_alpha(c: &Color, alpha: f64) -> Result<Color, String> {
    let alpha = fuzzy_assert_range(alpha, 0.0, 1.0, "alpha")?;
    match c.space {
        ColorSpace::Hsl => Ok(nova(
            ColorSpace::Hsl,
            canais(c).ok_or_else(|| nao_garantida(c))?,
            alpha,
            ColorFormat::Infer,
        )),
        _ => Ok(nova(
            ColorSpace::Rgb,
            rgb(c).ok_or_else(|| nao_garantida(c))?,
            alpha,
            ColorFormat::Infer,
        )),
    }
}

/// O alfa (o 1.66 não tem alfa ausente).
pub(crate) fn alpha(c: &Color) -> f64 {
    c.alpha.unwrap_or(0.0)
}

/// A mensagem de recusa de uma cor sem representação no 1.66.
pub(crate) fn nao_garantida(c: &Color) -> String {
    format!(
        "dartforge-sass (modo dart-sass 1.66): a cor no espaço {} com canais \
         {:?}/{:?}/{:?} e alfa {:?} não existe no modelo de cor do 1.66 (RGB \
         inteiro de 0 a 255 ou HSL); a saída do 1.66 não é garantida",
        c.space.name(),
        c.c0,
        c.c1,
        c.c2,
        c.alpha
    )
}

/// `SassColor.==` do 1.66: os inteiros RGB e o alfa exatos.
pub(crate) fn iguais(a: &Color, b: &Color) -> bool {
    match (rgb(a), rgb(b)) {
        (Some(x), Some(y)) => x == y && a.alpha == b.alpha,
        _ => false,
    }
}

/// O hash de uma cor do modelo 1.66, coerente com [`iguais`] e com o hash
/// do 1.102 para cores RGB de canais inteiros; `None` para as outras.
pub(crate) fn hash(c: &Color) -> Option<i64> {
    if !c.modelo_166 {
        return None;
    }
    let fh = |x: f64| (x * 1e11).round() as i64;
    let [r, g, b] = rgb(c).unwrap_or([0.0; 3]);
    Some(fh(r) ^ fh(g) ^ fh(b) ^ fh(alpha(c)))
}
