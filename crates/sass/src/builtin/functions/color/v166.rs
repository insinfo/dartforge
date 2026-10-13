//! The color functions of dart-sass 1.66.0 (`lib/src/functions/color.dart`
//! of that version), for the compatibility mode
//! ([`crate::VersaoDartSass::V1_66_0`]), on top of the model of
//! [`crate::color::v166`].
//!
//! Each function of [`super`] calls the one here when the mode is on. Those
//! that 1.66 did not have (the CSS Color 4 ones: `lab()`, `oklch()`,
//! `color()`, global `hwb()` and the space ones of `sass:color`) are
//! refused: in 1.66 they would be plain CSS functions or "Undefined
//! function", and the output is not guaranteed.
use super::*;
use crate::color::v166 as m;

fn color_166(c: Result<Color, String>, span: Span) -> SassResult<Value> {
    c.map(color_value).map_err(|e| (e, span).into())
}

/// The color of the argument, which the 1.66 model must be able to
/// represent.
fn color_arg(v: &Value, name: &str, span: Span) -> SassResult<Arc<Color>> {
    let c = as_color(v, name, span)?;
    if m::rgb(&c).is_none() {
        return fail(m::not_guaranteed(&c), span);
    }
    Ok(c)
}

fn rgb_of(c: &Color) -> [f64; 3] {
    m::rgb(c).unwrap_or([0.0; 3])
}

fn hsl_of(c: &Color) -> [f64; 3] {
    m::hsl(c).unwrap_or([0.0; 3])
}

/// `SassString.isSpecialNumber` of 1.66 (without `attr()` and `if()`).
fn is_special(v: &Value) -> bool {
    match v {
        Value::Calculation(..) => true,
        Value::String(s, QuoteKind::None) => {
            let t = s.as_bytes();
            if t.len() < "min(_)".len() {
                return false;
            }
            let l = |i: usize| t[i].to_ascii_lowercase();
            match l(0) {
                b'c' => match l(1) {
                    b'l' => l(2) == b'a' && l(3) == b'm' && l(4) == b'p' && t[5] == b'(',
                    b'a' => l(2) == b'l' && l(3) == b'c' && t[4] == b'(',
                    _ => false,
                },
                b'v' => l(1) == b'a' && l(2) == b'r' && t[3] == b'(',
                b'e' => l(1) == b'n' && l(2) == b'v' && t[3] == b'(',
                b'm' => match l(1) {
                    b'a' => l(2) == b'x' && t[3] == b'(',
                    b'i' => l(2) == b'n' && t[3] == b'(',
                    _ => false,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

/// `SassString.isVar` of 1.66.
fn is_var(v: &Value) -> bool {
    match v {
        Value::String(s, QuoteKind::None) => {
            let t = s.as_bytes();
            t.len() >= "var(--_)".len()
                && t[0].eq_ignore_ascii_case(&b'v')
                && t[1].eq_ignore_ascii_case(&b'a')
                && t[2].eq_ignore_ascii_case(&b'r')
                && t[3] == b'('
        }
        _ => false,
    }
}

/// `_percentageOrUnitless` of 1.66: clamps to `[0, max]`.
fn pct_or_unitless(n: &SassNumber, max: f64, name: &str, span: Span) -> SassResult<f64> {
    let v = if is_unitless(n) {
        n.num.0
    } else if has_unit(n, &Unit::Percent) {
        max * n.num.0 / 100.0
    } else {
        return fail(
            format!(
                "${name}: Expected {} to have no units or \"%\".",
                inspect_str(&Value::Dimension(n.clone()), span)
            ),
            span,
        );
    };
    Ok(v.clamp(0.0, max))
}

/// `assertUnit('%')`.
fn require_percent(n: &SassNumber, name: &str, span: Span) -> SassResult<()> {
    if has_unit(n, &Unit::Percent) {
        return Ok(());
    }
    fail(
        format!(
            "${name}: Expected {} to have unit \"%\".",
            inspect_str(&Value::Dimension(n.clone()), span)
        ),
        span,
    )
}

/// Refuses a function that 1.66 did not have.
pub(crate) fn nonexistent(name: &str, span: Span) -> SassResult<Value> {
    fail(
        format!(
            "dartforge-sass (dart-sass 1.66 mode): {name}() does not exist in dart-sass \
             1.66 (it is from CSS Color 4, dart-sass 1.79+); the 1.66 output is not guaranteed"
        ),
        span,
    )
}

// ### RGB

/// `_rgb`.
fn rgb4(name: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let alpha = a.get(3);
    if a[..3].iter().any(is_special) || alpha.is_some_and(is_special) {
        return function_string(name, a, span);
    }
    let r = as_number(&a[0], "red", span)?;
    let g = as_number(&a[1], "green", span)?;
    let b = as_number(&a[2], "blue", span)?;
    let alpha = match alpha {
        Some(v) => pct_or_unitless(&as_number(v, "alpha", span)?, 1.0, "alpha", span)?,
        None => 1.0,
    };
    color_166(
        m::from_rgb(
            m::fuzzy_round(pct_or_unitless(&r, 255.0, "red", span)?),
            m::fuzzy_round(pct_or_unitless(&g, 255.0, "green", span)?),
            m::fuzzy_round(pct_or_unitless(&b, 255.0, "blue", span)?),
            alpha,
            ColorFormat::RgbFunction,
        ),
        span,
    )
}

/// `_rgbTwoArg`.
fn rgb2_166(name: &str, a: &[Value], span: Span) -> SassResult<Value> {
    if is_var(&a[0]) || (!matches!(a[0], Value::Color(..)) && is_var(&a[1])) {
        return function_string(name, a, span);
    }
    if is_special(&a[1]) {
        let c = color_arg(&a[0], "color", span)?;
        let [r, g, b] = rgb_of(&c);
        return Ok(Value::String(
            format!(
                "{name}({r}, {g}, {b}, {})",
                a[1].to_css_string(span, false)?
            ),
            QuoteKind::None,
        ));
    }
    let c = color_arg(&a[0], "color", span)?;
    let alpha = as_number(&a[1], "alpha", span)?;
    color_166(
        m::with_alpha(&c, pct_or_unitless(&alpha, 1.0, "alpha", span)?),
        span,
    )
}

/// `_parseChannels`: the arguments, or the CSS function as text.
fn parse_channels(
    name: &str,
    arg_names: &[&str],
    channels: &Value,
    span: Span,
) -> SassResult<Result<Vec<Value>, Value>> {
    if is_var(channels) {
        return Ok(Err(function_string(name, &[channels.clone()], span)?));
    }
    let original = channels.clone();
    let mut channels = channels.clone();
    let mut slash_alpha = None;
    if channels.separator() == ListSeparator::Slash {
        let list = channels.clone().as_list();
        if list.len() != 2 {
            return fail(
                format!(
                    "Only 2 slash-separated elements allowed, but {} {} passed.",
                    list.len(),
                    if list.len() == 1 { "was" } else { "were" }
                ),
                span,
            );
        }
        channels = list[0].clone();
        let alpha = list[1].clone();
        if !is_special(&alpha) {
            as_number(&alpha, "alpha", span)?;
        }
        slash_alpha = Some(alpha);
        if is_var(&list[0]) {
            return Ok(Err(function_string(name, &[original], span)?));
        }
    }
    let comma = channels.separator() == ListSeparator::Comma;
    let bracketed = matches!(channels, Value::List(_, _, Brackets::Bracketed));
    if comma || bracketed {
        let mut msg = String::from("$channels must be");
        if bracketed {
            msg.push_str(" an unbracketed");
        }
        if comma {
            msg.push_str(if bracketed { "," } else { " a" });
            msg.push_str(" space-separated");
        }
        msg.push_str(" list.");
        return fail(msg, span);
    }
    let list = channels.clone().as_list();
    if list.len() > 3 {
        return fail(
            format!("Only 3 elements allowed, but {} were passed.", list.len()),
            span,
        );
    } else if list.len() < 3 {
        // `_isVarSlash`: 1.66 requires quotes (a bug of its own, ported
        // as is).
        let var_slash = |v: &Value| {
            matches!(v, Value::String(t, QuoteKind::Quoted)
                if t.len() >= 4 && t[..4].eq_ignore_ascii_case("var(") && t.contains('/'))
        };
        if list.iter().any(is_var) || list.last().is_some_and(var_slash) {
            return Ok(Err(function_string(name, &[original], span)?));
        }
        return fail(
            format!("Missing element {}.", arg_names[list.len()]),
            span,
        );
    }
    if let Some(alpha) = slash_alpha {
        let mut v = list;
        v.push(alpha);
        return Ok(Ok(v));
    }
    match &list[2] {
        Value::Dimension(SassNumber {
            as_slash: Some(slash),
            ..
        }) => Ok(Ok(vec![
            list[0].clone(),
            list[1].clone(),
            Value::Dimension(slash.0.clone()),
            Value::Dimension(slash.1.clone()),
        ])),
        Value::String(t, QuoteKind::None) if t.contains('/') => {
            Ok(Err(function_string(name, &[channels], span)?))
        }
        _ => Ok(Ok(list)),
    }
}

fn rgb_like(name: &'static str, args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let overloads = [
        signature!("red", "green", "blue", "alpha"),
        signature!("red", "green", "blue"),
        signature!("color", "alpha"),
        signature!("channels"),
    ];
    let i = choose_overload(&overloads, &args);
    let a = bind_args(args, &overloads[i])?;
    match i {
        0 | 1 => rgb4(name, &a, span),
        2 => rgb2_166(name, &a, span),
        _ => match parse_channels(name, &["$red", "$green", "$blue"], &a[0], span)? {
            Ok(v) => rgb4(name, &v, span),
            Err(s) => Ok(s),
        },
    }
}

pub(crate) fn rgb(args: ArgumentResult) -> SassResult<Value> {
    rgb_like("rgb", args)
}

pub(crate) fn rgba(args: ArgumentResult) -> SassResult<Value> {
    rgb_like("rgba", args)
}

/// `red`, `green`, `blue`, `hue`, `saturation`, `lightness`, `whiteness`
/// and `blackness` of 1.66.
pub(crate) fn channel(name: &str, args: ArgumentResult) -> SassResult<Value> {
    match name {
        "red" => rgb_channel(args, 0),
        "green" => rgb_channel(args, 1),
        "blue" => rgb_channel(args, 2),
        "hue" => hsl_channel(args, 0, Unit::Deg),
        "saturation" => hsl_channel(args, 1, Unit::Percent),
        "lightness" => hsl_channel(args, 2, Unit::Percent),
        "whiteness" => whiteness_blackness(args, 0),
        _ => whiteness_blackness(args, 1),
    }
}

fn rgb_channel(args: ArgumentResult, i: usize) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    let c = color_arg(&a[0], "color", span)?;
    Ok(num(rgb_of(&c)[i], Unit::None))
}

/// `_mixColors`.
fn mix_colors(c1: &Color, c2: &Color, weight: &SassNumber, span: Span) -> SassResult<Value> {
    let weight_scale = value_in_range(weight, 0.0, 100.0, "weight", span)? / 100.0;
    let normalized = weight_scale * 2.0 - 1.0;
    let (a1, a2) = (m::alpha(c1), m::alpha(c2));
    let alpha_distance = a1 - a2;
    let combined = if normalized * alpha_distance == -1.0 {
        normalized
    } else {
        (normalized + alpha_distance) / (1.0 + normalized * alpha_distance)
    };
    let w1 = (combined + 1.0) / 2.0;
    let w2 = 1.0 - w1;
    let ([r1, g1, b1], [r2, g2, b2]) = (rgb_of(c1), rgb_of(c2));
    color_166(
        m::from_rgb(
            m::fuzzy_round(r1 * w1 + r2 * w2),
            m::fuzzy_round(g1 * w1 + g2 * w2),
            m::fuzzy_round(b1 * w1 + b2 * w2),
            a1 * weight_scale + a2 * (1.0 - weight_scale),
            ColorFormat::Infer,
        ),
        span,
    )
}

pub(crate) fn mix(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(
        args,
        &signature!("color1", "color2", "weight" = ParamDefault::Pct(50.0)),
    )?;
    let c1 = color_arg(&a[0], "color1", span)?;
    let c2 = color_arg(&a[1], "color2", span)?;
    let weight = as_number(&a[2], "weight", span)?;
    mix_colors(&c1, &c2, &weight, span)
}

pub(crate) fn invert(args: ArgumentResult, global: bool) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color", "weight" = ParamDefault::Pct(100.0)))?;
    let weight = as_number(&a[1], "weight", span)?;
    if matches!(a[0], Value::Dimension(..)) || (global && is_special(&a[0])) {
        if weight.num.0 != 100.0 || !has_unit(&weight, &Unit::Percent) {
            return fail(
                "Only one argument may be passed to the plain-CSS invert() function.",
                span,
            );
        }
        return function_string("invert", &a[..1], span);
    }
    let c = color_arg(&a[0], "color", span)?;
    let [r, g, b] = rgb_of(&c);
    let inverse = m::from_rgb(
        255.0 - r,
        255.0 - g,
        255.0 - b,
        m::alpha(&c),
        ColorFormat::Infer,
    )
    .map_err(|e| (e, span))?;
    mix_colors(&inverse, &c, &weight, span)
}

// ### HSL

fn hsl_channel(args: ArgumentResult, i: usize, unit: Unit) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    let c = color_arg(&a[0], "color", span)?;
    Ok(num(hsl_of(&c)[i], unit))
}

/// `_hsl`.
fn hsl4(name: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let alpha = a.get(3);
    if a[..3].iter().any(is_special) || alpha.is_some_and(is_special) {
        return function_string(name, a, span);
    }
    let h = angle_value(&a[0], "hue", span)?;
    let s = as_number(&a[1], "saturation", span)?;
    let l = as_number(&a[2], "lightness", span)?;
    let alpha = match alpha {
        Some(v) => pct_or_unitless(&as_number(v, "alpha", span)?, 1.0, "alpha", span)?,
        None => 1.0,
    };
    color_166(
        m::from_hsl(
            h,
            s.num.0.clamp(0.0, 100.0),
            l.num.0.clamp(0.0, 100.0),
            alpha,
            ColorFormat::HslFunction,
        ),
        span,
    )
}

fn hsl_like(name: &'static str, args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let overloads = [
        signature!("hue", "saturation", "lightness", "alpha"),
        signature!("hue", "saturation", "lightness"),
        signature!("hue", "saturation"),
        signature!("channels"),
    ];
    let i = choose_overload(&overloads, &args);
    let a = bind_args(args, &overloads[i])?;
    match i {
        0 | 1 => hsl4(name, &a, span),
        2 => {
            if is_var(&a[0]) || is_var(&a[1]) {
                function_string(name, &a, span)
            } else {
                fail("Missing argument $lightness.", span)
            }
        }
        _ => match parse_channels(name, &["$hue", "$saturation", "$lightness"], &a[0], span)? {
            Ok(v) => hsl4(name, &v, span),
            Err(s) => Ok(s),
        },
    }
}

pub(crate) fn hsl(args: ArgumentResult) -> SassResult<Value> {
    hsl_like("hsl", args)
}

pub(crate) fn hsla(args: ArgumentResult) -> SassResult<Value> {
    hsl_like("hsla", args)
}

/// `changeHsl`: the given channels change, the others come from the
/// color.
fn change_hsl(
    c: &Color,
    h: Option<f64>,
    s: Option<f64>,
    l: Option<f64>,
    alpha: Option<f64>,
    span: Span,
) -> SassResult<Value> {
    let [h0, s0, l0] = hsl_of(c);
    color_166(
        m::from_hsl(
            h.unwrap_or(h0),
            s.unwrap_or(s0),
            l.unwrap_or(l0),
            alpha.unwrap_or(m::alpha(c)),
            ColorFormat::Infer,
        ),
        span,
    )
}

pub(crate) fn grayscale(args: ArgumentResult, global: bool) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    if matches!(a[0], Value::Dimension(..)) || (global && is_special(&a[0])) {
        return function_string("grayscale", &a[..1], span);
    }
    let c = color_arg(&a[0], "color", span)?;
    change_hsl(&c, None, Some(0.0), None, None, span)
}

pub(crate) fn complement(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    let c = color_arg(&a[0], "color", span)?;
    change_hsl(&c, Some(hsl_of(&c)[0] + 180.0), None, None, None, span)
}

pub(crate) fn adjust_hue(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color", "degrees"))?;
    let c = color_arg(&a[0], "color", span)?;
    let degrees = angle_value(&a[1], "degrees", span)?;
    change_hsl(&c, Some(hsl_of(&c)[0] + degrees), None, None, None, span)
}

/// `lighten`/`darken` (`channel` 2) and `saturate`/`desaturate` (`channel` 1).
pub(crate) fn shift_channel(args: ArgumentResult, channel: usize, sign: f64) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color", "amount"))?;
    let c = color_arg(&a[0], "color", span)?;
    let amount = value_in_range(&as_number(&a[1], "amount", span)?, 0.0, 100.0, "amount", span)?;
    let v = (hsl_of(&c)[channel] + sign * amount).clamp(0.0, 100.0);
    if channel == 1 {
        change_hsl(&c, None, Some(v), None, None, span)
    } else {
        change_hsl(&c, None, None, Some(v), None, span)
    }
}

/// `saturate`: with one argument, the CSS filter.
pub(crate) fn saturate(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let overloads = [signature!("amount"), signature!("color", "amount")];
    if choose_overload(&overloads, &args) == 0 {
        let a = bind_args(args, &overloads[0])?;
        if matches!(a[0], Value::Dimension(..)) || is_special(&a[0]) {
            return function_string("saturate", &a, span);
        }
        let n = as_number(&a[0], "amount", span)?;
        return Ok(Value::String(
            format!(
                "saturate({})",
                Value::Dimension(n).to_css_string(span, false)?
            ),
            QuoteKind::None,
        ));
    }
    shift_channel(args, 1, 1.0)
}

// ### Opacity

/// `_opacify`/`_transparentize`.
pub(crate) fn shift_opacity(args: ArgumentResult, sign: f64) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color", "amount"))?;
    let c = color_arg(&a[0], "color", span)?;
    let amount = value_in_range_with_unit(
        &as_number(&a[1], "amount", span)?,
        0.0,
        1.0,
        "amount",
        "",
        span,
    )?;
    color_166(
        m::with_alpha(&c, (m::alpha(&c) + sign * amount).clamp(0.0, 1.0)),
        span,
    )
}

/// Global and module `alpha()` (the Microsoft filter is text).
pub(crate) fn alpha(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let overloads = [signature!("color"), signature!(; args)];
    let i = choose_overload(&overloads, &args);
    let a = bind_args(args, &overloads[i])?;
    if i == 0 {
        if let Value::String(t, QuoteKind::None) = &a[0] {
            if microsoft_filter(t) {
                return function_string("alpha", &a, span);
            }
        }
        let c = color_arg(&a[0], "color", span)?;
        return Ok(num(m::alpha(&c), Unit::None));
    }
    let list = match &a[0] {
        Value::ArgList(l) => l.elems.clone(),
        v => v.clone().as_list(),
    };
    if !list.is_empty()
        && list
            .iter()
            .all(|v| matches!(v, Value::String(t, QuoteKind::None) if microsoft_filter(t)))
    {
        return function_string("alpha", &list, span);
    }
    if list.is_empty() {
        return fail("Missing argument $color.", span);
    }
    fail(
        format!("Only 1 argument allowed, but {} were passed.", list.len()),
        span,
    )
}

pub(crate) fn opacity(args: ArgumentResult, global: bool) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    if matches!(a[0], Value::Dimension(..)) || (global && is_special(&a[0])) {
        return function_string("opacity", &a, span);
    }
    let c = color_arg(&a[0], "color", span)?;
    Ok(num(m::alpha(&c), Unit::None))
}

// ### HWB (module)

pub(crate) fn hwb(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let overloads = [
        signature!("hue", "whiteness", "blackness", "alpha" = ParamDefault::Num(1.0)),
        signature!("channels"),
    ];
    let i = choose_overload(&overloads, &args);
    let a = bind_args(args, &overloads[i])?;
    let a = if i == 0 {
        a
    } else {
        match parse_channels("hwb", &["$hue", "$whiteness", "$blackness"], &a[0], span)? {
            Ok(v) => v,
            Err(s) => {
                return fail(
                    format!(
                        "Expected numeric channels, got \"{}\".",
                        s.to_css_string(span, false)?
                    ),
                    span,
                )
            }
        }
    };
    let h = angle_value(&a[0], "hue", span)?;
    let w = as_number(&a[1], "whiteness", span)?;
    let b = as_number(&a[2], "blackness", span)?;
    require_percent(&w, "whiteness", span)?;
    require_percent(&b, "blackness", span)?;
    let alpha = match a.get(3) {
        Some(v) => pct_or_unitless(&as_number(v, "alpha", span)?, 1.0, "alpha", span)?,
        None => 1.0,
    };
    color_166(
        m::from_hwb(
            h,
            value_in_range(&w, 0.0, 100.0, "whiteness", span)?,
            value_in_range(&b, 0.0, 100.0, "blackness", span)?,
            alpha,
        ),
        span,
    )
}

pub(crate) fn whiteness_blackness(args: ArgumentResult, i: usize) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    let c = color_arg(&a[0], "color", span)?;
    Ok(num(m::hwb(&c).unwrap_or([0.0; 2])[i], Unit::Percent))
}

// ### Others

pub(crate) fn ie_hex_str(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    let c = color_arg(&a[0], "color", span)?;
    let [r, g, b] = rgb_of(&c);
    let hex = |v: f64| format!("{:02X}", v as i64);
    Ok(Value::String(
        format!(
            "#{}{}{}{}",
            hex(m::fuzzy_round(m::alpha(&c) * 255.0)),
            hex(r),
            hex(g),
            hex(b)
        ),
        QuoteKind::None,
    ))
}

/// `_updateComponents` (`adjust`, `scale` and `change`).
pub(crate) fn update_components(args: ArgumentResult, mode: Mode) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color"; kwargs))?;
    let c = color_arg(&a[0], "color", span)?;
    let Value::ArgList(list) = &a[1] else {
        unreachable!()
    };
    if !list.elems.is_empty() {
        return fail(
            "Only one positional argument is allowed. All other arguments must be passed by name.",
            span,
        );
    }
    let mut keywords: Vec<(String, Value)> = list
        .keywords()
        .iter()
        .map(|(k, v)| (k.as_str().to_owned(), v.clone()))
        .collect();
    let (change, adjust, scale) = (
        matches!(mode, Mode::Change),
        matches!(mode, Mode::Adjust),
        matches!(mode, Mode::Scale),
    );
    // `getParam`.
    let mut param = |name: &str,
                     max: f64,
                     check_percent: bool,
                     assert_percent: bool|
     -> SassResult<Option<f64>> {
        let Some(i) = keywords.iter().position(|(k, _)| k == name) else {
            return Ok(None);
        };
        let n = as_number(&keywords.remove(i).1, name, span)?;
        if scale || assert_percent {
            require_percent(&n, name, span)?;
        }
        let max = if scale { 100.0 } else { max };
        let min = if change { 0.0 } else { -max };
        Ok(Some(if scale || assert_percent {
            value_in_range(&n, min, max, name, span)?
        } else {
            value_in_range_with_unit(
                &n,
                min,
                max,
                name,
                if check_percent { "%" } else { "" },
                span,
            )?
        }))
    };
    let alpha = param("alpha", 1.0, false, false)?;
    let red = param("red", 255.0, false, false)?;
    let green = param("green", 255.0, false, false)?;
    let blue = param("blue", 255.0, false, false)?;
    let hue = if scale {
        None
    } else {
        match keywords.iter().position(|(k, _)| k == "hue") {
            Some(i) => Some(angle_value(&keywords.remove(i).1, "hue", span)?),
            None => None,
        }
    };
    let mut param =
        |name: &str, check_percent: bool, assert_percent: bool| -> SassResult<Option<f64>> {
            let Some(i) = keywords.iter().position(|(k, _)| k == name) else {
                return Ok(None);
            };
            let n = as_number(&keywords.remove(i).1, name, span)?;
            if scale || assert_percent {
                require_percent(&n, name, span)?;
            }
            let min = if change { 0.0 } else { -100.0 };
            Ok(Some(if scale || assert_percent {
                value_in_range(&n, min, 100.0, name, span)?
            } else {
                value_in_range_with_unit(
                    &n,
                    min,
                    100.0,
                    name,
                    if check_percent { "%" } else { "" },
                    span,
                )?
            }))
        };
    let saturation = param("saturation", true, false)?;
    let lightness = param("lightness", true, false)?;
    let whiteness = param("whiteness", false, true)?;
    let blackness = param("blackness", false, true)?;
    if !keywords.is_empty() {
        let names: Vec<String> = keywords.iter().map(|(k, _)| format!("${k}")).collect();
        let sentence = match names.as_slice() {
            [one] => one.clone(),
            [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
            [] => String::new(),
        };
        return fail(
            format!(
                "No argument{} named {sentence}.",
                if names.len() == 1 { "" } else { "s" }
            ),
            span,
        );
    }
    let has_rgb = red.is_some() || green.is_some() || blue.is_some();
    let has_sl = saturation.is_some() || lightness.is_some();
    let has_wb = whiteness.is_some() || blackness.is_some();
    if has_rgb && (has_sl || has_wb || hue.is_some()) {
        return fail(
            format!(
                "RGB parameters may not be passed along with {} parameters.",
                if has_wb { "HWB" } else { "HSL" }
            ),
            span,
        );
    }
    if has_sl && has_wb {
        return fail(
            "HSL parameters may not be passed along with HWB parameters.",
            span,
        );
    }
    let update = |current: f64, param: Option<f64>, max: f64| -> f64 {
        let Some(p) = param else { return current };
        if change {
            p
        } else if adjust {
            (current + p).clamp(0.0, max)
        } else {
            current + (if p > 0.0 { max - current } else { current }) * (p / 100.0)
        }
    };
    let a0 = m::alpha(&c);
    if has_rgb {
        let [r, g, b] = rgb_of(&c);
        return color_166(
            m::from_rgb(
                m::fuzzy_round(update(r, red, 255.0)),
                m::fuzzy_round(update(g, green, 255.0)),
                m::fuzzy_round(update(b, blue, 255.0)),
                update(a0, alpha, 1.0),
                ColorFormat::Infer,
            ),
            span,
        );
    }
    let [h0, s0, l0] = hsl_of(&c);
    let new_hue = if change {
        hue.unwrap_or(h0)
    } else {
        h0 + hue.unwrap_or(0.0)
    };
    if has_wb {
        let [w0, b0] = m::hwb(&c).unwrap_or([0.0; 2]);
        return color_166(
            m::from_hwb(
                new_hue,
                update(w0, whiteness, 100.0),
                update(b0, blackness, 100.0),
                update(a0, alpha, 1.0),
            ),
            span,
        );
    }
    if hue.is_some() || has_sl {
        return color_166(
            m::from_hsl(
                new_hue,
                update(s0, saturation, 100.0),
                update(l0, lightness, 100.0),
                update(a0, alpha, 1.0),
                ColorFormat::Infer,
            ),
            span,
        );
    }
    if alpha.is_some() {
        return color_166(m::with_alpha(&c, update(a0, alpha, 1.0)), span);
    }
    Ok(a[0].clone())
}
