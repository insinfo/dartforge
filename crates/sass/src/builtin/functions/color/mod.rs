//! The Sass color functions — port of `lib/src/functions/color.dart` of
//! dart-sass 1.102.0 (the global ones and those of the `sass:color`
//! module), on top of the color model of [`crate::color`].
//!
//! Arguments are bound as in dart-sass's `ParameterList.verify`
//! (positional, named, defaults and `$rest...`), and the overloaded
//! functions choose the signature like `BuiltInCallable.callbackFor`. The
//! dart-sass deprecation warnings are not emitted (the compilation runs
//! without a logger).
use std::{cell::Cell, rc::Rc};

use crate::builtin::builtin_imports::*;
use crate::color::{
    clamp_like_css, dart_mod, ChannelInfo, ColorFormat, ColorSpace, GamutMapMethod,
    HueInterpolation, InterpolationMethod,
};
use crate::value::{conversion_factor, fuzzy_equals, ArgList};

use super::GlobalFunctionMap;

/// A parameter: name and default value.
#[derive(Clone, Copy)]
enum ParamDefault {
    Required,
    Null,
    Pct(f64),
    Num(f64),
}

type Param = (&'static str, ParamDefault);

/// A signature: the parameters and, if any, the `$rest...`.
struct Signature {
    params: &'static [Param],
    rest: bool,
}

fn default_value(p: ParamDefault) -> Value {
    match p {
        ParamDefault::Required | ParamDefault::Null => Value::Null,
        ParamDefault::Pct(n) => Value::Dimension(SassNumber {
            num: Number(n),
            unit: Unit::Percent,
            as_slash: None,
        }),
        ParamDefault::Num(n) => Value::Dimension(SassNumber::new_unitless(n)),
    }
}

/// `ParameterList.matches`.
fn matches_signature(a: &Signature, positional_count: usize, names: &BTreeSet<Identifier>) -> bool {
    let mut used = 0;
    for (i, (name, p)) in a.params.iter().enumerate() {
        let ident = Identifier::from(*name);
        if i < positional_count {
            if names.contains(&ident) {
                return false;
            }
        } else if names.contains(&ident) {
            used += 1;
        } else if matches!(p, ParamDefault::Required) {
            return false;
        }
    }
    if a.rest {
        return true;
    }
    if positional_count > a.params.len() {
        return false;
    }
    used >= names.len()
}

/// `BuiltInCallable.callbackFor`: the overload that matches, or the closest
/// one.
fn choose_overload(overloads: &[Signature], args: &ArgumentResult) -> usize {
    let positional_count = args.positional.len();
    let names: BTreeSet<Identifier> = args.named.keys().copied().collect();
    let mut fuzzy: Option<usize> = None;
    let mut min_dist: Option<isize> = None;
    for (i, a) in overloads.iter().enumerate() {
        if matches_signature(a, positional_count, &names) {
            return i;
        }
        let dist = a.params.len() as isize - positional_count as isize;
        if let Some(m) = min_dist {
            if dist.abs() > m.abs() {
                continue;
            }
            if dist.abs() == m.abs() && dist < 0 {
                continue;
            }
        }
        min_dist = Some(dist);
        fuzzy = Some(i);
    }
    fuzzy.unwrap_or(0)
}

/// Binds the arguments to the signature (`verify` + the evaluator's
/// binding).
fn bind_args(args: ArgumentResult, a: &Signature) -> SassResult<Vec<Value>> {
    let span = args.span;
    let ArgumentResult {
        mut positional,
        mut named,
        separator,
        ..
    } = args;
    let positional_count = positional.len();
    let mut used = 0;
    for (i, (name, p)) in a.params.iter().enumerate() {
        let ident = Identifier::from(*name);
        if i < positional_count {
            if named.contains_key(&ident) {
                return Err((
                    format!("Argument ${name} was passed both by position and by name."),
                    span,
                )
                    .into());
            }
        } else if named.contains_key(&ident) {
            used += 1;
        } else if matches!(p, ParamDefault::Required) {
            return Err((format!("Missing argument ${name}."), span).into());
        }
    }
    if !a.rest {
        if positional_count > a.params.len() {
            let n = a.params.len();
            return Err((
                format!(
                    "Only {n} {}argument{} allowed, but {positional_count} {} passed.",
                    if named.is_empty() { "" } else { "positional " },
                    if n == 1 { "" } else { "s" },
                    if positional_count == 1 { "was" } else { "were" }
                ),
                span,
            )
                .into());
        }
        if used < named.len() {
            let unknown: Vec<String> = named
                .keys()
                .filter(|k| !a.params.iter().any(|(n, _)| Identifier::from(*n) == **k))
                .map(|k| format!("${k}"))
                .collect();
            return Err((
                format!(
                    "No argument{} named {}.",
                    if unknown.len() == 1 { "" } else { "s" },
                    unknown.join(" or ")
                ),
                span,
            )
                .into());
        }
    }
    let mut out = Vec::with_capacity(a.params.len() + 1);
    let rest_positional = if positional_count > a.params.len() {
        positional.split_off(a.params.len())
    } else {
        Vec::new()
    };
    let mut pos = positional.into_iter();
    for (name, p) in a.params {
        let v = match pos.next() {
            Some(v) => v,
            None => named
                .remove(&Identifier::from(*name))
                .unwrap_or_else(|| default_value(*p)),
        };
        out.push(v);
    }
    if a.rest {
        let sep = if separator == ListSeparator::Undecided {
            ListSeparator::Comma
        } else {
            separator
        };
        out.push(Value::ArgList(ArgList::new(
            rest_positional,
            Rc::new(Cell::new(false)),
            named,
            sep,
        )));
    }
    Ok(out)
}

macro_rules! signature {
    ($($name:literal $(= $p:expr)?),* $(; $rest:ident)?) => {
        Signature {
            params: &[$(($name, signature!(@p $($p)?))),*],
            rest: signature!(@r $($rest)?),
        }
    };
    (@p) => { ParamDefault::Required };
    (@p $p:expr) => { $p };
    (@r) => { false };
    (@r $r:ident) => { true };
}

// The 1.66 mode uses the utilities and the `signature!` from here.
mod v166;

// ### Value utilities

fn fail<T>(msg: impl Into<String>, span: Span) -> SassResult<T> {
    Err((msg.into(), span).into())
}

fn with_name(msg: String, name: Option<&str>) -> String {
    match name {
        Some(n) => format!("${n}: {msg}"),
        None => msg,
    }
}

fn as_number(v: &Value, name: &str, span: Span) -> SassResult<SassNumber> {
    v.clone().assert_number_with_name(name, span)
}

fn as_color(v: &Value, name: &str, span: Span) -> SassResult<Arc<Color>> {
    v.clone().assert_color_with_name(name, span)
}

fn inspect_str(v: &Value, span: Span) -> String {
    v.inspect(span).unwrap_or_default()
}

fn num(n: f64, unit: Unit) -> Value {
    Value::Dimension(SassNumber {
        num: Number(n),
        unit,
        as_slash: None,
    })
}

fn has_unit(n: &SassNumber, u: &Unit) -> bool {
    n.unit == *u
}

fn is_unitless(n: &SassNumber) -> bool {
    n.unit == Unit::None
}

/// `fuzzyCheckRange`.
fn fuzzy_check_range(n: f64, min: f64, max: f64) -> Option<f64> {
    if fuzzy_equals(n, min) {
        return Some(min);
    }
    if fuzzy_equals(n, max) {
        return Some(max);
    }
    (n > min && n < max).then_some(n)
}

/// `valueInRange`.
fn value_in_range(n: &SassNumber, min: f64, max: f64, name: &str, span: Span) -> SassResult<f64> {
    match fuzzy_check_range(n.num.0, min, max) {
        Some(v) => Ok(v),
        None => fail(
            format!(
                "${name}: Expected {} to be within {min} and {max}.",
                inspect_str(&Value::Dimension(n.clone()), span)
            ),
            span,
        ),
    }
}

/// `valueInRangeWithUnit`.
fn value_in_range_with_unit(
    n: &SassNumber,
    min: f64,
    max: f64,
    name: &str,
    unit: &str,
    span: Span,
) -> SassResult<f64> {
    match fuzzy_check_range(n.num.0, min, max) {
        Some(v) => Ok(v),
        None => fail(
            format!(
                "${name}: Expected {} to be within {min}{unit} and {max}{unit}.",
                inspect_str(&Value::Dimension(n.clone()), span)
            ),
            span,
        ),
    }
}

/// `coerceValueToUnit('deg')`, if compatible.
fn in_degrees(n: &SassNumber) -> Option<f64> {
    if n.unit == Unit::None {
        return None;
    }
    conversion_factor(&n.unit, &Unit::Deg).map(|f| n.num.0 * f)
}

/// `_angleValue`: degrees, or the value as is (with a warning, in the
/// official compiler).
fn angle_value(v: &Value, name: &str, span: Span) -> SassResult<f64> {
    let n = as_number(v, name, span)?;
    Ok(in_degrees(&n).unwrap_or(n.num.0))
}

/// `_percentageOrUnitless`.
fn percentage_or_unitless(
    n: &SassNumber,
    max: f64,
    name: Option<&str>,
    span: Span,
) -> SassResult<f64> {
    if is_unitless(n) {
        Ok(n.num.0)
    } else if has_unit(n, &Unit::Percent) {
        Ok(max * n.num.0 / 100.0)
    } else {
        fail(
            with_name(
                format!(
                    "Expected {} to have unit \"%\" or no units.",
                    inspect_str(&Value::Dimension(n.clone()), span)
                ),
                name,
            ),
            span,
        )
    }
}

/// dart-sass's `isSpecialNumber`.
fn is_special_number(v: &Value) -> bool {
    match v {
        Value::Calculation(..) => true,
        Value::String(s, QuoteKind::None) => {
            let t = s.as_bytes();
            if t.len() < "min(_)".len() {
                return false;
            }
            let l = |i: usize| t[i].to_ascii_lowercase();
            match l(0) {
                b'a' => l(1) == b't' && l(2) == b't' && l(3) == b'r' && t[4] == b'(',
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
                b'i' => l(1) == b'f' && t[2] == b'(',
                _ => false,
            }
        }
        _ => false,
    }
}

/// dart-sass's `isSpecialVariable`.
fn is_special_variable(v: &Value) -> bool {
    match v {
        Value::String(s, QuoteKind::None) => {
            let t = s.as_bytes();
            if t.len() < "var(_)".len() {
                return false;
            }
            let l = |i: usize| t[i].to_ascii_lowercase();
            match l(0) {
                b'a' => l(1) == b't' && l(2) == b't' && l(3) == b'r' && t[4] == b'(',
                b'i' => l(1) == b'f' && t[2] == b'(',
                b'v' => l(1) == b'a' && l(2) == b'r' && t[3] == b'(',
                _ => false,
            }
        }
        _ => false,
    }
}

/// `_isNone`.
fn is_none(v: &Value) -> bool {
    matches!(v, Value::String(s, QuoteKind::None) if s.eq_ignore_ascii_case("none"))
}

/// `_functionString`.
fn function_string(name: &str, args: &[Value], span: Span) -> SassResult<Value> {
    let parts = args
        .iter()
        .map(|a| a.to_css_string(span, false))
        .collect::<SassResult<Vec<_>>>()?;
    Ok(Value::String(
        format!("{name}({})", parts.join(", ")),
        QuoteKind::None,
    ))
}

fn color_value(c: Color) -> Value {
    Value::Color(Arc::new(c))
}

/// `_checkPercent` (only warns in the official compiler).
fn check_percent(_n: &SassNumber, _name: &str) {}

/// Text `$x` of an unquoted string (`assertString` + `assertUnquoted`).
fn unquoted_string(v: &Value, name: &str, span: Span) -> SassResult<String> {
    match v {
        Value::String(s, QuoteKind::None) => Ok(s.clone()),
        Value::String(s, QuoteKind::Quoted) => fail(
            format!("${name}: Expected {s:?} to be an unquoted string."),
            span,
        ),
        v => fail(
            format!("${name}: {} is not a string.", inspect_str(v, span)),
            span,
        ),
    }
}

fn space_by_name(name: &str, arg: &str, span: Span) -> SassResult<ColorSpace> {
    ColorSpace::from_name(name)
        .ok_or_else(|| (format!("${arg}: Unknown color space \"{name}\"."), span).into())
}

// ### Construction from channels

/// `_channelFromValue`.
fn channel_from_value(
    ch: &ChannelInfo,
    v: Option<&SassNumber>,
    clamp: bool,
    span: Span,
) -> SassResult<Option<f64>> {
    let Some(v) = v else { return Ok(None) };
    if ch.linear {
        if ch.requires_percent && !has_unit(v, &Unit::Percent) {
            return fail(
                format!(
                    "${}: Expected {} to have unit \"%\".",
                    ch.name,
                    inspect_str(&Value::Dimension(v.clone()), span)
                ),
                span,
            );
        }
        let base = percentage_or_unitless(v, ch.max, Some(ch.name), span)?;
        if (!ch.lower_clamped && !ch.upper_clamped) || !clamp {
            return Ok(Some(base));
        }
        return Ok(Some(clamp_like_css(
            base,
            if ch.lower_clamped {
                ch.min
            } else {
                f64::NEG_INFINITY
            },
            if ch.upper_clamped {
                ch.max
            } else {
                f64::INFINITY
            },
        )));
    }
    // Hue: `coerceValueToUnit('deg') % 360`.
    let degrees = if v.unit == Unit::None {
        v.num.0
    } else {
        match conversion_factor(&v.unit, &Unit::Deg) {
            Some(f) => v.num.0 * f,
            None => {
                return fail(
                    format!(
                        "${}: Expected {} to have angle units (deg, grad, rad, turn).",
                        ch.name,
                        inspect_str(&Value::Dimension(v.clone()), span)
                    ),
                    span,
                )
            }
        }
    };
    Ok(Some(dart_mod(degrees, 360.0)))
}

/// `_forcePercent`.
fn force_percent(n: Option<&SassNumber>) -> Option<SassNumber> {
    n.map(|n| SassNumber {
        num: n.num,
        unit: Unit::Percent,
        as_slash: None,
    })
}

/// `_colorFromChannels`.
fn color_from_channels(
    space: ColorSpace,
    c0: Option<&SassNumber>,
    c1: Option<&SassNumber>,
    c2: Option<&SassNumber>,
    alpha: Option<f64>,
    clamp: bool,
    from_rgb_function: bool,
    span: Span,
) -> SassResult<Color> {
    let ch = space.channels();
    match space {
        ColorSpace::Hsl => {
            let hue = match c0 {
                Some(h) => Some(angle_value(&Value::Dimension(h.clone()), "hue", span)?),
                None => None,
            };
            let s = force_percent(c1);
            let l = force_percent(c2);
            Ok(Color::for_space_internal(
                space,
                hue,
                channel_from_value(&ch[1], s.as_ref(), clamp, span)?,
                channel_from_value(&ch[2], l.as_ref(), clamp, span)?,
                alpha,
            ))
        }
        ColorSpace::Hwb => {
            if let Some(w) = c1 {
                w.assert_unit(&Unit::Percent, "whiteness", span)?;
            }
            if let Some(b) = c2 {
                b.assert_unit(&Unit::Percent, "blackness", span)?;
            }
            let mut whiteness = c1.map(|n| n.num.0);
            let mut blackness = c2.map(|n| n.num.0);
            if let (Some(w), Some(b)) = (whiteness, blackness) {
                if w + b > 100.0 {
                    whiteness = Some(w / (w + b) * 100.0);
                    blackness = Some(b / (w + b) * 100.0);
                }
            }
            let hue = match c0 {
                Some(h) => Some(angle_value(&Value::Dimension(h.clone()), "hue", span)?),
                None => None,
            };
            Ok(Color::for_space_internal(
                space, hue, whiteness, blackness, alpha,
            ))
        }
        ColorSpace::Rgb => Ok(Color::rgb_internal(
            channel_from_value(&ch[0], c0, clamp, span)?,
            channel_from_value(&ch[1], c1, clamp, span)?,
            channel_from_value(&ch[2], c2, clamp, span)?,
            alpha,
            if from_rgb_function {
                ColorFormat::RgbFunction
            } else {
                ColorFormat::Infer
            },
        )),
        _ => Ok(Color::for_space_internal(
            space,
            channel_from_value(&ch[0], c0, clamp, span)?,
            channel_from_value(&ch[1], c1, clamp, span)?,
            channel_from_value(&ch[2], c2, clamp, span)?,
            alpha,
        )),
    }
}

fn alpha_clamped(v: &Value, span: Span) -> SassResult<f64> {
    let n = as_number(v, "alpha", span)?;
    Ok(clamp_like_css(
        percentage_or_unitless(&n, 1.0, Some("alpha"), span)?,
        0.0,
        1.0,
    ))
}

/// `_rgb` (three or four arguments).
fn rgb3(name: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let alpha = a.get(3);
    if a[..3].iter().any(is_special_number) || alpha.is_some_and(is_special_number) {
        return function_string(name, a, span);
    }
    let r = as_number(&a[0], "red", span)?;
    let g = as_number(&a[1], "green", span)?;
    let b = as_number(&a[2], "blue", span)?;
    let alpha = match alpha {
        Some(v) => alpha_clamped(v, span)?,
        None => 1.0,
    };
    Ok(color_value(color_from_channels(
        ColorSpace::Rgb,
        Some(&r),
        Some(&g),
        Some(&b),
        Some(alpha),
        true,
        true,
        span,
    )?))
}

/// `_rgbTwoArg`.
fn rgb2(name: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let (first, second) = (&a[0], &a[1]);
    if is_special_variable(first)
        || (!matches!(first, Value::Color(..)) && is_special_variable(second))
    {
        return function_string(name, a, span);
    }
    let c = as_color(first, "color", span)?;
    if !c.is_legacy() {
        return fail(
            format!(
                "${name}: Expected {} to be in the legacy RGB, HSL, or HWB color space.",
                inspect_str(first, span)
            ),
            span,
        );
    }
    let c = c.to_space(ColorSpace::Rgb, true);
    if is_special_number(second) {
        return function_string(
            name,
            &[
                num(c.channel0(), Unit::None),
                num(c.channel1(), Unit::None),
                num(c.channel2(), Unit::None),
                second.clone(),
            ],
            span,
        );
    }
    let alpha = as_number(second, "alpha", span)?;
    Ok(color_value(c.change_alpha(clamp_like_css(
        percentage_or_unitless(&alpha, 1.0, Some("alpha"), span)?,
        0.0,
        1.0,
    ))))
}

/// `_hsl` (three or four arguments).
fn hsl3(name: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let alpha = a.get(3);
    if a[..3].iter().any(is_special_number) || alpha.is_some_and(is_special_number) {
        return function_string(name, a, span);
    }
    let h = as_number(&a[0], "hue", span)?;
    let s = as_number(&a[1], "saturation", span)?;
    let l = as_number(&a[2], "lightness", span)?;
    let alpha = match alpha {
        Some(v) => alpha_clamped(v, span)?,
        None => 1.0,
    };
    Ok(color_value(color_from_channels(
        ColorSpace::Hsl,
        Some(&h),
        Some(&s),
        Some(&l),
        Some(alpha),
        true,
        false,
        span,
    )?))
}

/// `_parseNumberOrString`: a Sass number or the text as is.
fn number_or_string(text: &str) -> Value {
    let t = text.trim();
    // Number: sign, digits, point, exponent and unit (identifier).
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let digits_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i == digits_start
        || (i == digits_start + 1 && b[digits_start] == b'.')
        || t.len() != text.len()
    {
        return Value::String(text.to_owned(), QuoteKind::None);
    }
    let Ok(v) = t[..i].parse::<f64>() else {
        return Value::String(text.to_owned(), QuoteKind::None);
    };
    let unit_text = &t[i..];
    if unit_text.is_empty() {
        return num(v, Unit::None);
    }
    if unit_text == "%" {
        return num(v, Unit::Percent);
    }
    if unit_text
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c == '-' || c == '_')
    {
        return num(v, Unit::from(unit_text.to_owned()));
    }
    Value::String(text.to_owned(), QuoteKind::None)
}

/// `assertCommonListStyle`: the list (or the value as a one-element list).
fn common_list(
    v: &Value,
    name: Option<&str>,
    allow_slash: bool,
    span: Span,
) -> SassResult<Vec<Value>> {
    let (elems, sep, brackets) = match v {
        Value::List(e, s, b) => (e.clone(), *s, *b),
        Value::ArgList(a) => (a.elems.clone(), a.separator, Brackets::None),
        Value::Map(m) if m.is_empty() => (Vec::new(), ListSeparator::Undecided, Brackets::None),
        v => (vec![v.clone()], ListSeparator::Undecided, Brackets::None),
    };
    let invalid = sep == ListSeparator::Comma || (!allow_slash && sep == ListSeparator::Slash);
    if invalid || brackets == Brackets::Bracketed {
        let kind = if allow_slash {
            "space- or slash-separated"
        } else {
            "space-separated"
        };
        return fail(
            with_name(
                format!("Expected {kind} list, was {}", inspect_str(v, span)),
                name,
            ),
            span,
        );
    }
    Ok(elems)
}

/// `_parseSlashChannels`: the components and the alpha, or `None` if the
/// input must go as is.
fn parse_slash_channels(
    v: &Value,
    name: Option<&str>,
    span: Span,
) -> SassResult<Option<(Value, Option<Value>)>> {
    let list = common_list(v, name, true, span)?;
    if v.separator() == ListSeparator::Slash {
        if list.len() == 2 {
            return Ok(Some((list[0].clone(), Some(list[1].clone()))));
        }
        return fail(
            with_name(
                format!(
                    "Only 2 slash-separated elements allowed, but {} {} passed.",
                    list.len(),
                    if list.len() == 1 { "was" } else { "were" }
                ),
                name,
            ),
            span,
        );
    }
    match list.last() {
        Some(Value::String(text, QuoteKind::None)) => {
            let parts: Vec<&str> = text.split('/').collect();
            match parts.as_slice() {
                [_] => Ok(Some((v.clone(), None))),
                [c3, a] => {
                    let mut leading = list[..list.len() - 1].to_vec();
                    leading.push(number_or_string(c3));
                    Ok(Some((
                        Value::List(leading, ListSeparator::Space, Brackets::None),
                        Some(number_or_string(a)),
                    )))
                }
                _ => Ok(None),
            }
        }
        Some(Value::Dimension(n)) if n.as_slash.is_some() => {
            let pair = n.as_slash.clone().unwrap();
            let mut leading = list[..list.len() - 1].to_vec();
            leading.push(Value::Dimension(pair.0.clone()));
            Ok(Some((
                Value::List(leading, ListSeparator::Space, Brackets::None),
                Some(Value::Dimension(pair.1.clone())),
            )))
        }
        _ => Ok(Some((v.clone(), None))),
    }
}

/// `_parseChannels`.
fn parse_channels(
    function_name: &str,
    input: &Value,
    space: Option<ColorSpace>,
    name: Option<&str>,
    span: Span,
) -> SassResult<Value> {
    if is_special_variable(input) {
        return function_string(function_name, &[input.clone()], span);
    }
    let Some((components, alpha_value)) = parse_slash_channels(input, name, span)? else {
        return function_string(function_name, &[input.clone()], span);
    };
    let mut space = space;
    let list = common_list(&components, name, false, span)?;
    let channels: Vec<Value> = match list.as_slice() {
        [] => {
            return fail(
                with_name("Color component list may not be empty.".into(), name),
                span,
            )
        }
        [Value::String(t, QuoteKind::None), ..] if t.eq_ignore_ascii_case("from") => {
            return function_string(function_name, &[input.clone()], span);
        }
        _ if is_special_variable(&components) => vec![components.clone()],
        [first, rest @ ..] => {
            let chs: Vec<Value> = if space.is_none() {
                let space_name = match first {
                    Value::String(s, QuoteKind::None) => s.clone(),
                    Value::String(s, QuoteKind::Quoted) => {
                        return fail(
                            with_name(format!("Expected {s:?} to be an unquoted string."), name),
                            span,
                        )
                    }
                    v => {
                        return fail(
                            with_name(format!("{} is not a string.", inspect_str(v, span)), name),
                            span,
                        )
                    }
                };
                space = if is_special_variable(first) {
                    None
                } else {
                    Some(ColorSpace::from_name(&space_name).ok_or_else(|| {
                        (
                            with_name(format!("Unknown color space \"{space_name}\"."), name),
                            span,
                        )
                    })?)
                };
                if let Some(
                    s @ (ColorSpace::Rgb
                    | ColorSpace::Hsl
                    | ColorSpace::Hwb
                    | ColorSpace::Lab
                    | ColorSpace::Lch
                    | ColorSpace::Oklab
                    | ColorSpace::Oklch),
                ) = space
                {
                    return fail(
                        with_name(
                            format!(
                                "The color() function doesn't support the color space {s}. Use the {s}() function instead."
                            ),
                            name,
                        ),
                        span,
                    );
                }
                rest.to_vec()
            } else {
                list.clone()
            };
            for (i, c) in chs.iter().enumerate() {
                if !is_special_number(c) && !matches!(c, Value::Dimension(..)) && !is_none(c) {
                    let channel_label = space
                        .and_then(|s| s.channels().get(i).map(|ch| format!("{} channel", ch.name)))
                        .unwrap_or_else(|| format!("channel {}", i + 1));
                    return fail(
                        with_name(
                            format!(
                                "Expected {channel_label} to be a number, was {}.",
                                inspect_str(c, span)
                            ),
                            name,
                        ),
                        span,
                    );
                }
            }
            chs
        }
    };
    let comma_space = matches!(space, Some(ColorSpace::Rgb | ColorSpace::Hsl));
    if let Some(a) = &alpha_value {
        if is_special_number(a) {
            return if channels.len() == 3 && comma_space {
                let mut all_args = channels.clone();
                all_args.push(a.clone());
                function_string(function_name, &all_args, span)
            } else {
                function_string(function_name, &[input.clone()], span)
            };
        }
    }
    let alpha = match &alpha_value {
        None => Some(1.0),
        Some(Value::String(t, QuoteKind::None)) if t == "none" => None,
        Some(a) => {
            let n = as_number(a, name.unwrap_or("alpha"), span)?;
            Some(clamp_like_css(
                percentage_or_unitless(&n, 1.0, Some("alpha"), span)?,
                0.0,
                1.0,
            ))
        }
    };
    let Some(space) = space else {
        return function_string(function_name, &[input.clone()], span);
    };
    if channels.iter().any(is_special_number) {
        return if channels.len() == 3 && comma_space {
            let mut all_args = channels.clone();
            if let Some(a) = alpha_value {
                all_args.push(a);
            }
            function_string(function_name, &all_args, span)
        } else {
            function_string(function_name, &[input.clone()], span)
        };
    }
    if channels.len() != 3 {
        return fail(
            with_name(
                format!(
                    "The {space} color space has 3 channels but {} has {}.",
                    inspect_str(input, span),
                    channels.len()
                ),
                name,
            ),
            span,
        );
    }
    let n = |v: &Value| match v {
        Value::Dimension(n) => Some(n.clone()),
        _ => None,
    };
    let (a0, a1, a2) = (n(&channels[0]), n(&channels[1]), n(&channels[2]));
    Ok(color_value(color_from_channels(
        space,
        a0.as_ref(),
        a1.as_ref(),
        a2.as_ref(),
        alpha,
        true,
        space == ColorSpace::Rgb,
        span,
    )?))
}

// ### Functions

fn channel_fn(
    a: &[Value],
    space: ColorSpace,
    channel: &str,
    unit: Unit,
    rounded: bool,
    span: Span,
) -> SassResult<Value> {
    let c = as_color(&a[0], "color", span)?;
    if !c.is_legacy() {
        return fail(
            format!(
                "color.{channel}() is only supported for legacy colors. Please use color.channel() instead with an explicit $space argument."
            ),
            span,
        );
    }
    let v = c.legacy_channel(space, channel);
    Ok(num(if rounded { v.round() } else { v }, unit))
}

macro_rules! channel {
    ($f:ident, $space:expr, $channel:literal, $unit:expr, $rounded:expr) => {
        pub(crate) fn $f(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
            if v.options.v166() {
                return v166::channel($channel, args);
            }
            let span = args.span;
            let a = bind_args(args, &signature!("color"))?;
            channel_fn(&a, $space, $channel, $unit, $rounded, span)
        }
    };
}

channel!(red, ColorSpace::Rgb, "red", Unit::None, true);
channel!(green, ColorSpace::Rgb, "green", Unit::None, true);
channel!(blue, ColorSpace::Rgb, "blue", Unit::None, true);
channel!(hue, ColorSpace::Hsl, "hue", Unit::Deg, false);
channel!(
    saturation,
    ColorSpace::Hsl,
    "saturation",
    Unit::Percent,
    false
);
channel!(
    lightness,
    ColorSpace::Hsl,
    "lightness",
    Unit::Percent,
    false
);
channel!(
    whiteness,
    ColorSpace::Hwb,
    "whiteness",
    Unit::Percent,
    false
);
channel!(
    blackness,
    ColorSpace::Hwb,
    "blackness",
    Unit::Percent,
    false
);

/// `_mixLegacy`.
fn mix_legacy(c1: &Color, c2: &Color, weight: &SassNumber, span: Span) -> SassResult<Color> {
    let rgb1 = c1.to_space(ColorSpace::Rgb, true);
    let rgb2 = c2.to_space(ColorSpace::Rgb, true);
    let weight_scale = value_in_range(weight, 0.0, 100.0, "weight", span)? / 100.0;
    let normalized = weight_scale * 2.0 - 1.0;
    let alpha_distance = c1.alpha_f64() - c2.alpha_f64();
    let combined = if normalized * alpha_distance == -1.0 {
        normalized
    } else {
        (normalized + alpha_distance) / (1.0 + normalized * alpha_distance)
    };
    let w1 = (combined + 1.0) / 2.0;
    let w2 = 1.0 - w1;
    Ok(Color::rgb_internal(
        Some(rgb1.channel0() * w1 + rgb2.channel0() * w2),
        Some(rgb1.channel1() * w1 + rgb2.channel1() * w2),
        Some(rgb1.channel2() * w1 + rgb2.channel2() * w2),
        Some(rgb1.alpha_f64() * weight_scale + rgb2.alpha_f64() * (1.0 - weight_scale)),
        ColorFormat::Infer,
    ))
}

/// `InterpolationMethod.fromValue`.
fn interpolation_method(v: &Value, span: Span) -> SassResult<InterpolationMethod> {
    let list = common_list(v, Some("method"), false, span)?;
    if list.is_empty() {
        return fail(
            "$method: Expected a color interpolation method, got an empty list.",
            span,
        );
    }
    let space = space_by_name(&unquoted_string(&list[0], "method", span)?, "method", span)?;
    if list.len() == 1 {
        return Ok(InterpolationMethod::new(space, None));
    }
    let hue = match unquoted_string(&list[1], "method", span)?
        .to_ascii_lowercase()
        .as_str()
    {
        "shorter" => HueInterpolation::Shorter,
        "longer" => HueInterpolation::Longer,
        "increasing" => HueInterpolation::Increasing,
        "decreasing" => HueInterpolation::Decreasing,
        _ => {
            return fail(
                format!(
                    "$method: Unknown hue interpolation method {}.",
                    inspect_str(&list[1], span)
                ),
                span,
            )
        }
    };
    if list.len() == 2 {
        return fail(
            format!(
                "$method: Expected unquoted string \"hue\" after {}.",
                inspect_str(v, span)
            ),
            span,
        );
    }
    if !unquoted_string(&list[2], "method", span)?.eq_ignore_ascii_case("hue") {
        return fail(
            format!(
                "$method: Expected unquoted string \"hue\" at the end of {}, was {}.",
                inspect_str(v, span),
                inspect_str(&list[2], span)
            ),
            span,
        );
    }
    if list.len() > 3 {
        return fail(
            format!(
                "$method: Expected nothing after \"hue\" in {}.",
                inspect_str(v, span)
            ),
            span,
        );
    }
    if !space.is_polar() {
        return fail(
            format!(
                "$method: Hue interpolation method \"{} hue\" may not be set for rectangular color space {space}.",
                hue.name()
            ),
            span,
        );
    }
    Ok(InterpolationMethod::new(space, Some(hue)))
}

pub(crate) fn mix(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::mix(args);
    }
    let span = args.span;
    let a = bind_args(
        args,
        &signature!(
            "color1",
            "color2",
            "weight" = ParamDefault::Pct(50.0),
            "method" = ParamDefault::Null
        ),
    )?;
    let c1 = as_color(&a[0], "color1", span)?;
    let c2 = as_color(&a[1], "color2", span)?;
    let weight = as_number(&a[2], "weight", span)?;
    if a[3] != Value::Null {
        let method = interpolation_method(&a[3], span)?;
        let w = value_in_range_with_unit(&weight, 0.0, 100.0, "weight", "%", span)? / 100.0;
        return Ok(color_value(c1.interpolate(&c2, method, w, false)));
    }
    check_percent(&weight, "weight");
    if !c1.is_legacy() {
        return fail(
            format!(
                "$color1: To use color.mix() with non-legacy color {}, you must provide a $method.",
                inspect_str(&a[0], span)
            ),
            span,
        );
    }
    if !c2.is_legacy() {
        return fail(
            format!(
                "$color2: To use color.mix() with non-legacy color {}, you must provide a $method.",
                inspect_str(&a[1], span)
            ),
            span,
        );
    }
    Ok(color_value(mix_legacy(&c1, &c2, &weight, span)?))
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
        0 | 1 => rgb3(name, &a, span),
        2 => rgb2(name, &a, span),
        _ => parse_channels(name, &a[0], Some(ColorSpace::Rgb), Some("channels"), span),
    }
}

pub(crate) fn rgb(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::rgb(args);
    }
    rgb_like("rgb", args)
}

pub(crate) fn rgba(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::rgba(args);
    }
    rgb_like("rgba", args)
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
        0 | 1 => hsl3(name, &a, span),
        2 => {
            if is_special_variable(&a[0]) || is_special_variable(&a[1]) {
                function_string(name, &a, span)
            } else {
                fail("Missing argument $lightness.", span)
            }
        }
        _ => parse_channels(name, &a[0], Some(ColorSpace::Hsl), Some("channels"), span),
    }
}

pub(crate) fn hsl(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::hsl(args);
    }
    hsl_like("hsl", args)
}

pub(crate) fn hsla(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::hsla(args);
    }
    hsl_like("hsla", args)
}

/// `_invertChannel`.
fn invert_channel(c: &Color, ch: &ChannelInfo, v: Option<f64>, span: Span) -> SassResult<f64> {
    let Some(v) = v else {
        return missing_channel_error(c, ch.name, span);
    };
    Ok(if ch.linear && ch.min < 0.0 {
        -v
    } else if ch.linear && ch.min == 0.0 {
        ch.max - v
    } else {
        dart_mod(v + 180.0, 360.0)
    })
}

fn missing_channel_error<T>(c: &Color, channel: &str, span: Span) -> SassResult<T> {
    let text = Value::Color(Arc::new(c.clone()))
        .to_css_string(span, false)
        .unwrap_or_default();
    fail(
        format!(
            "${channel}: Because the CSS working group is still deciding on the best behavior, Sass doesn't currently support modifying missing channels (color: {text})."
        ),
        span,
    )
}

/// `_invert`.
fn invert_impl(a: &[Value], global: bool, span: Span) -> SassResult<Value> {
    let weight = as_number(&a[1], "weight", span)?;
    if matches!(a[0], Value::Dimension(..)) || (global && is_special_number(&a[0])) {
        if weight.num.0 != 100.0 || !has_unit(&weight, &Unit::Percent) {
            return fail(
                "Only one argument may be passed to the plain-CSS invert() function.",
                span,
            );
        }
        return function_string("invert", &a[..1], span);
    }
    let c = as_color(&a[0], "color", span)?;
    if a[2] == Value::Null {
        if !c.is_legacy() {
            return fail(
                format!(
                    "$color: To use color.invert() with non-legacy color {}, you must provide a $space.",
                    inspect_str(&a[0], span)
                ),
                span,
            );
        }
        check_percent(&weight, "weight");
        let rgb = c.to_space(ColorSpace::Rgb, true);
        let ch = ColorSpace::Rgb.channels();
        let inv = Color::rgb_internal(
            Some(invert_channel(&rgb, &ch[0], rgb.channel0_or_null(), span)?),
            Some(invert_channel(&rgb, &ch[1], rgb.channel1_or_null(), span)?),
            Some(invert_channel(&rgb, &ch[2], rgb.channel2_or_null(), span)?),
            c.alpha_or_null(),
            ColorFormat::Infer,
        );
        return Ok(color_value(
            mix_legacy(&inv, &c, &weight, span)?.to_space(c.space(), true),
        ));
    }
    let space = space_by_name(&unquoted_string(&a[2], "space", span)?, "space", span)?;
    let w = value_in_range_with_unit(&weight, 0.0, 100.0, "weight", "%", span)? / 100.0;
    if fuzzy_equals(w, 0.0) {
        return Ok(Value::Color(c));
    }
    let s = c.to_space(space, true);
    let ch = space.channels();
    let inverted = match space {
        ColorSpace::Hwb => Color::for_space_internal(
            ColorSpace::Hwb,
            Some(invert_channel(&s, &ch[0], s.channel0_or_null(), span)?),
            s.channel2_or_null(),
            s.channel1_or_null(),
            Some(s.alpha_f64()),
        ),
        ColorSpace::Hsl | ColorSpace::Lch | ColorSpace::Oklch => Color::for_space_internal(
            space,
            Some(invert_channel(&s, &ch[0], s.channel0_or_null(), span)?),
            s.channel1_or_null(),
            Some(invert_channel(&s, &ch[2], s.channel2_or_null(), span)?),
            Some(s.alpha_f64()),
        ),
        _ => Color::for_space_internal(
            space,
            Some(invert_channel(&s, &ch[0], s.channel0_or_null(), span)?),
            Some(invert_channel(&s, &ch[1], s.channel1_or_null(), span)?),
            Some(invert_channel(&s, &ch[2], s.channel2_or_null(), span)?),
            Some(s.alpha_f64()),
        ),
    };
    Ok(color_value(if fuzzy_equals(w, 1.0) {
        inverted.to_space(c.space(), false)
    } else {
        c.interpolate(
            &inverted,
            InterpolationMethod::new(space, None),
            1.0 - w,
            false,
        )
    }))
}

pub(crate) fn invert_global(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::invert(args, true);
    }
    let span = args.span;
    let a = bind_args(
        args,
        &signature!(
            "color",
            "weight" = ParamDefault::Pct(100.0),
            "space" = ParamDefault::Null
        ),
    )?;
    invert_impl(&a, true, span)
}

pub(crate) fn invert_module(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::invert(args, false);
    }
    let span = args.span;
    let a = bind_args(
        args,
        &signature!(
            "color",
            "weight" = ParamDefault::Pct(100.0),
            "space" = ParamDefault::Null
        ),
    )?;
    invert_impl(&a, false, span)
}

/// `_grayscale`.
fn grayscale_impl(v: &Value, span: Span) -> SassResult<Value> {
    let c = as_color(v, "color", span)?;
    Ok(color_value(if c.is_legacy() {
        let hsl = c.to_space(ColorSpace::Hsl, true);
        Color::for_space_internal(
            ColorSpace::Hsl,
            hsl.channel0_or_null(),
            Some(0.0),
            hsl.channel2_or_null(),
            Some(hsl.alpha_f64()),
        )
        .to_space(c.space(), false)
    } else {
        let ok = c.to_space(ColorSpace::Oklch, true);
        Color::for_space_internal(
            ColorSpace::Oklch,
            ok.channel0_or_null(),
            Some(0.0),
            ok.channel2_or_null(),
            Some(ok.alpha_f64()),
        )
        .to_space(c.space(), true)
    }))
}

pub(crate) fn grayscale_global(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::grayscale(args, true);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    if matches!(a[0], Value::Dimension(..)) || is_special_number(&a[0]) {
        return function_string("grayscale", &a, span);
    }
    grayscale_impl(&a[0], span)
}

pub(crate) fn grayscale_module(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::grayscale(args, false);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    if matches!(a[0], Value::Dimension(..)) {
        return function_string("grayscale", &a[..1], span);
    }
    grayscale_impl(&a[0], span)
}

fn require_legacy(c: &Color, name: &str, span: Span) -> SassResult<()> {
    if c.is_legacy() {
        Ok(())
    } else {
        fail(
            format!(
                "{name}() is only supported for legacy colors. Please use color.adjust() instead with an explicit $space argument."
            ),
            span,
        )
    }
}

pub(crate) fn adjust_hue(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::adjust_hue(args);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color", "degrees"))?;
    let c = as_color(&a[0], "color", span)?;
    let degrees = angle_value(&a[1], "degrees", span)?;
    require_legacy(&c, "adjust-hue", span)?;
    let h = c.legacy_channel(ColorSpace::Hsl, "hue");
    Ok(color_value(c.change_hsl(Some(h + degrees), None, None)))
}

fn shift_hsl(args: ArgumentResult, name: &str, channel: &str, sign: f64) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color", "amount"))?;
    let c = as_color(&a[0], "color", span)?;
    let amount = as_number(&a[1], "amount", span)?;
    require_legacy(&c, name, span)?;
    let current = c.legacy_channel(ColorSpace::Hsl, channel);
    let new_value = clamp_like_css(
        current + sign * value_in_range(&amount, 0.0, 100.0, "amount", span)?,
        0.0,
        100.0,
    );
    Ok(color_value(if channel == "lightness" {
        c.change_hsl(None, None, Some(new_value))
    } else {
        c.change_hsl(None, Some(new_value), None)
    }))
}

pub(crate) fn lighten(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::shift_channel(args, 2, 1.0);
    }
    shift_hsl(args, "lighten", "lightness", 1.0)
}

pub(crate) fn darken(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::shift_channel(args, 2, -1.0);
    }
    shift_hsl(args, "darken", "lightness", -1.0)
}

pub(crate) fn desaturate(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::shift_channel(args, 1, -1.0);
    }
    shift_hsl(args, "desaturate", "saturation", -1.0)
}

pub(crate) fn saturate(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::saturate(args);
    }
    let span = args.span;
    let overloads = [signature!("amount"), signature!("color", "amount")];
    let i = choose_overload(&overloads, &args);
    if i == 0 {
        let a = bind_args(args, &overloads[0])?;
        if matches!(a[0], Value::Dimension(..)) || is_special_number(&a[0]) {
            return function_string("saturate", &a, span);
        }
        let n = as_number(&a[0], "amount", span)?;
        let t = Value::Dimension(n).to_css_string(span, false)?;
        return Ok(Value::String(format!("saturate({t})"), QuoteKind::None));
    }
    shift_hsl(args, "saturate", "saturation", 1.0)
}

fn shift_alpha(args: ArgumentResult, name: &str, sign: f64) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color", "amount"))?;
    let c = as_color(&a[0], "color", span)?;
    let amount = as_number(&a[1], "amount", span)?;
    require_legacy(&c, name, span)?;
    let v = value_in_range_with_unit(&amount, 0.0, 1.0, "amount", "", span)?;
    Ok(color_value(c.change_alpha(clamp_like_css(
        c.alpha_f64() + sign * v,
        0.0,
        1.0,
    ))))
}

pub(crate) fn opacify(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::shift_opacity(args, 1.0);
    }
    shift_alpha(args, "opacify", 1.0)
}

pub(crate) fn fade_in(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::shift_opacity(args, 1.0);
    }
    shift_alpha(args, "fade-in", 1.0)
}

pub(crate) fn transparentize(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::shift_opacity(args, -1.0);
    }
    shift_alpha(args, "transparentize", -1.0)
}

pub(crate) fn fade_out(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::shift_opacity(args, -1.0);
    }
    shift_alpha(args, "fade-out", -1.0)
}

/// `^[a-zA-Z]+\s*=` somewhere in the text (`RegExp`'s `contains`).
fn microsoft_filter(s: &str) -> bool {
    let b = s.as_bytes();
    (0..b.len()).any(|i| {
        let mut j = i;
        while j < b.len() && b[j].is_ascii_alphabetic() {
            j += 1;
        }
        if j == i {
            return false;
        }
        // The RegExp `^` only matches at the start (without `multiLine`).
        if i != 0 {
            return false;
        }
        while j < b.len() && matches!(b[j], b' ' | b'\t' | b'\n' | b'\r' | b'\x0c') {
            j += 1;
        }
        j < b.len() && b[j] == b'='
    })
}

fn alpha_impl(args: ArgumentResult, in_module: bool) -> SassResult<Value> {
    let span = args.span;
    let overloads = [signature!("color"), signature!(; args)];
    let i = choose_overload(&overloads, &args);
    let a = bind_args(args, &overloads[i])?;
    if i == 0 {
        return match &a[0] {
            Value::String(t, QuoteKind::None) if microsoft_filter(t) => function_string("alpha", &a, span),
            Value::Color(c) if !c.is_legacy() => fail(
                format!(
                    "{}alpha() is only supported for legacy colors. Please use color.channel() instead.",
                    if in_module { "color." } else { "" }
                ),
                span,
            ),
            v => Ok(num(as_color(v, "color", span)?.alpha_f64(), Unit::None)),
        };
    }
    let list = match &a[0] {
        Value::ArgList(l) => l.elems.clone(),
        v => v.clone().as_list(),
    };
    let all_filters = list
        .iter()
        .all(|v| matches!(v, Value::String(t, QuoteKind::None) if microsoft_filter(t)));
    if (in_module || !list.is_empty()) && all_filters {
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

pub(crate) fn alpha_global(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::alpha(args);
    }
    alpha_impl(args, false)
}

pub(crate) fn alpha_module(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::alpha(args);
    }
    alpha_impl(args, true)
}

pub(crate) fn opacity_global(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::opacity(args, true);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    if matches!(a[0], Value::Dimension(..)) || is_special_number(&a[0]) {
        return function_string("opacity", &a, span);
    }
    Ok(num(as_color(&a[0], "color", span)?.alpha_f64(), Unit::None))
}

pub(crate) fn opacity_module(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::opacity(args, false);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    if matches!(a[0], Value::Dimension(..)) {
        return function_string("opacity", &a, span);
    }
    Ok(num(as_color(&a[0], "color", span)?.alpha_f64(), Unit::None))
}

macro_rules! space_fn {
    ($f:ident, $name:literal, $space:expr) => {
        pub(crate) fn $f(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
            if v.options.v166() {
                return v166::nonexistent($name, args.span);
            }
            let span = args.span;
            let a = bind_args(args, &signature!("channels"))?;
            parse_channels($name, &a[0], $space, Some("channels"), span)
        }
    };
}

space_fn!(hwb_global, "hwb", Some(ColorSpace::Hwb));
space_fn!(lab, "lab", Some(ColorSpace::Lab));
space_fn!(lch, "lch", Some(ColorSpace::Lch));
space_fn!(oklab, "oklab", Some(ColorSpace::Oklab));
space_fn!(oklch, "oklch", Some(ColorSpace::Oklch));

pub(crate) fn color_fn(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color", args.span);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("description"))?;
    parse_channels("color", &a[0], None, Some("description"), span)
}

pub(crate) fn hwb_module(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::hwb(args);
    }
    let span = args.span;
    let overloads = [
        signature!(
            "hue",
            "whiteness",
            "blackness",
            "alpha" = ParamDefault::Num(1.0)
        ),
        signature!("channels"),
    ];
    let i = choose_overload(&overloads, &args);
    let a = bind_args(args, &overloads[i])?;
    if i == 0 {
        let list = Value::List(
            vec![
                Value::List(
                    vec![a[0].clone(), a[1].clone(), a[2].clone()],
                    ListSeparator::Space,
                    Brackets::None,
                ),
                a[3].clone(),
            ],
            ListSeparator::Slash,
            Brackets::None,
        );
        return parse_channels("hwb", &list, Some(ColorSpace::Hwb), None, span);
    }
    parse_channels("hwb", &a[0], Some(ColorSpace::Hwb), Some("channels"), span)
}

/// `_adjustChannel`.
fn adjust_channel(
    c: &Color,
    ch: &ChannelInfo,
    old: Option<f64>,
    adj: Option<&SassNumber>,
    span: Span,
) -> SassResult<Option<f64>> {
    let Some(adj) = adj else { return Ok(old) };
    let Some(old) = old else {
        return missing_channel_error(c, ch.name, span);
    };
    let mut adj = adj.clone();
    if matches!(c.space(), ColorSpace::Hsl | ColorSpace::Hwb) && ch.is_polar_angle {
        adj = SassNumber::new_unitless(angle_value(&Value::Dimension(adj), "hue", span)?);
    } else if c.space() == ColorSpace::Hsl && (ch.name == "saturation" || ch.name == "lightness") {
        adj = SassNumber {
            num: adj.num,
            unit: Unit::Percent,
            as_slash: None,
        };
    } else if ch.name == "alpha" && ch.linear && adj.unit != Unit::None {
        adj = SassNumber::new_unitless(adj.num.0);
    }
    let delta = channel_from_value(ch, Some(&adj), false, span)?.unwrap_or(0.0);
    let result = old + delta;
    if ch.linear && ch.lower_clamped && result < ch.min {
        return Ok(Some(if old < ch.min {
            crate::color::dart_max(old, result)
        } else {
            ch.min
        }));
    }
    if ch.linear && ch.upper_clamped && result > ch.max {
        return Ok(Some(if old > ch.max {
            crate::color::dart_min(old, result)
        } else {
            ch.max
        }));
    }
    Ok(Some(result))
}

/// `_scaleChannel`.
fn scale_channel(
    c: &Color,
    ch: &ChannelInfo,
    old: Option<f64>,
    factor: Option<&SassNumber>,
    span: Span,
) -> SassResult<Option<f64>> {
    let Some(factor) = factor else { return Ok(old) };
    if !ch.linear {
        return fail(format!("${}: Channel isn't scalable.", ch.name), span);
    }
    let Some(old) = old else {
        return missing_channel_error(c, ch.name, span);
    };
    factor.assert_unit(&Unit::Percent, ch.name, span)?;
    let f = value_in_range_with_unit(factor, -100.0, 100.0, ch.name, "%", span)? / 100.0;
    Ok(Some(if f == 0.0 {
        old
    } else if f > 0.0 {
        if old >= ch.max {
            old
        } else {
            old + (ch.max - old) * f
        }
    } else if old <= ch.min {
        old
    } else {
        old + (old - ch.min) * f
    }))
}

/// `_sniffLegacyColorSpace`.
fn sniff_legacy(keys: &[String]) -> Option<ColorSpace> {
    for k in keys {
        match k.as_str() {
            "red" | "green" | "blue" => return Some(ColorSpace::Rgb),
            "saturation" | "lightness" => return Some(ColorSpace::Hsl),
            "whiteness" | "blackness" => return Some(ColorSpace::Hwb),
            _ => {}
        }
    }
    keys.iter().any(|k| k == "hue").then_some(ColorSpace::Hsl)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Change,
    Adjust,
    Scale,
}

/// `_updateComponents`.
fn update_components(a: &[Value], mode: Mode, span: Span) -> SassResult<Value> {
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
    let original = as_color(&a[0], "color", span)?;
    let mut take = |name: &str| -> Option<Value> {
        let i = keywords.iter().position(|(k, _)| k == name)?;
        Some(keywords.remove(i).1)
    };
    let space_kw = match take("space") {
        Some(v) => Some(unquoted_string(&v, "space", span)?),
        None => None,
    };
    let alpha_arg = take("alpha");
    let keys: Vec<String> = keywords.iter().map(|(k, _)| k.clone()).collect();
    let color = if space_kw.is_none() && original.is_legacy() && !keywords.is_empty() {
        match sniff_legacy(&keys) {
            Some(s) => original.to_space(s, false),
            None => (*original).clone(),
        }
    } else {
        match &space_kw {
            Some(s) => original.to_space(space_by_name(s, "space", span)?, true),
            None => (*original).clone(),
        }
    };
    let info = color.space().channels();
    let mut channel_args: [Option<Value>; 3] = [None, None, None];
    for (name, value) in keywords {
        let Some(i) = info.iter().position(|c| c.name == name) else {
            return fail(
                format!(
                    "${name}: Color space {} doesn't have a channel with this name.",
                    color.space()
                ),
                span,
            );
        };
        channel_args[i] = Some(value);
    }
    let result = match mode {
        Mode::Change => change_color(&color, &channel_args, alpha_arg.as_ref(), span)?,
        _ => {
            let mut nums: [Option<SassNumber>; 3] = [None, None, None];
            for i in 0..3 {
                if let Some(v) = &channel_args[i] {
                    nums[i] = Some(as_number(v, info[i].name, span)?);
                }
            }
            let alpha_num = match &alpha_arg {
                Some(v) => Some(as_number(v, "alpha", span)?),
                None => None,
            };
            let old = [
                color.channel0_or_null(),
                color.channel1_or_null(),
                color.channel2_or_null(),
            ];
            if mode == Mode::Scale {
                Color::for_space_internal(
                    color.space(),
                    scale_channel(&color, &info[0], old[0], nums[0].as_ref(), span)?,
                    scale_channel(&color, &info[1], old[1], nums[1].as_ref(), span)?,
                    scale_channel(&color, &info[2], old[2], nums[2].as_ref(), span)?,
                    scale_channel(
                        &color,
                        &ChannelInfo::ALPHA,
                        color.alpha_or_null(),
                        alpha_num.as_ref(),
                        span,
                    )?,
                )
            } else {
                Color::for_space_internal(
                    color.space(),
                    adjust_channel(&color, &info[0], old[0], nums[0].as_ref(), span)?,
                    adjust_channel(&color, &info[1], old[1], nums[1].as_ref(), span)?,
                    adjust_channel(&color, &info[2], old[2], nums[2].as_ref(), span)?,
                    adjust_channel(
                        &color,
                        &ChannelInfo::ALPHA,
                        color.alpha_or_null(),
                        alpha_num.as_ref(),
                        span,
                    )?
                    .map(|a| clamp_like_css(a, 0.0, 1.0)),
                )
            }
        }
    };
    Ok(color_value(result.to_space(original.space(), false)))
}

/// `_channelForChange`.
fn channel_for_change(
    arg: Option<&Value>,
    c: &Color,
    i: usize,
    span: Span,
) -> SassResult<Option<SassNumber>> {
    let Some(arg) = arg else {
        return Ok(c.channels_or_null()[i].map(|v| SassNumber {
            num: Number(v),
            unit: if matches!(c.space(), ColorSpace::Hsl | ColorSpace::Hwb) && i > 0 {
                Unit::Percent
            } else {
                Unit::None
            },
            as_slash: None,
        }));
    };
    if is_none(arg) {
        return Ok(None);
    }
    if let Value::Dimension(n) = arg {
        return Ok(Some(n.clone()));
    }
    fail(
        format!(
            "${}: {} is not a number or unquoted \"none\".",
            c.space().channels()[i].name,
            inspect_str(arg, span)
        ),
        span,
    )
}

/// `_changeColor`.
fn change_color(
    c: &Color,
    args: &[Option<Value>; 3],
    alpha_arg: Option<&Value>,
    span: Span,
) -> SassResult<Color> {
    let alpha = match alpha_arg {
        None => Some(c.alpha_f64()),
        Some(v) if is_none(v) => None,
        Some(Value::Dimension(n)) if is_unitless(n) => {
            Some(value_in_range(n, 0.0, 1.0, "alpha", span)?)
        }
        Some(Value::Dimension(n)) if has_unit(n, &Unit::Percent) => {
            Some(value_in_range_with_unit(n, 0.0, 100.0, "alpha", "%", span)? / 100.0)
        }
        Some(Value::Dimension(n)) => Some(value_in_range(n, 0.0, 1.0, "alpha", span)?),
        Some(v) => {
            return fail(
                format!(
                    "$alpha: {} is not a number or unquoted \"none\".",
                    inspect_str(v, span)
                ),
                span,
            )
        }
    };
    let c0 = channel_for_change(args[0].as_ref(), c, 0, span)?;
    let c1 = channel_for_change(args[1].as_ref(), c, 1, span)?;
    let c2 = channel_for_change(args[2].as_ref(), c, 2, span)?;
    color_from_channels(
        c.space(),
        c0.as_ref(),
        c1.as_ref(),
        c2.as_ref(),
        alpha,
        false,
        false,
        span,
    )
}

macro_rules! update_fn {
    ($f:ident, $mode:expr) => {
        pub(crate) fn $f(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
            if v.options.v166() {
                return v166::update_components(args, $mode);
            }
            let span = args.span;
            let a = bind_args(args, &signature!("color"; kwargs))?;
            update_components(&a, $mode, span)
        }
    };
}

update_fn!(adjust, Mode::Adjust);
update_fn!(scale, Mode::Scale);
update_fn!(change, Mode::Change);

pub(crate) fn ie_hex_str(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::ie_hex_str(args);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    let c = as_color(&a[0], "color", span)?
        .to_space(ColorSpace::Rgb, true)
        .to_gamut(GamutMapMethod::LocalMinde);
    let hex = |v: f64| format!("{:02X}", crate::value::fuzzy_round(v) as i64);
    Ok(Value::String(
        format!(
            "#{}{}{}{}",
            hex(c.alpha_f64() * 255.0),
            hex(c.channel0()),
            hex(c.channel1()),
            hex(c.channel2())
        ),
        QuoteKind::None,
    ))
}

pub(crate) fn complement(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::complement(args);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color", "space" = ParamDefault::Null))?;
    let c = as_color(&a[0], "color", span)?;
    let space = if c.is_legacy() && a[1] == Value::Null {
        ColorSpace::Hsl
    } else {
        space_by_name(&unquoted_string(&a[1], "space", span)?, "space", span)?
    };
    if !space.is_polar() {
        return fail(
            format!("$space: Color space {space} doesn't have a hue channel."),
            span,
        );
    }
    let s = c.to_space(space, a[1] != Value::Null);
    let ch = space.channels();
    let half_turn = SassNumber::new_unitless(180.0);
    let r = if space.is_legacy() {
        Color::for_space_internal(
            space,
            adjust_channel(&s, &ch[0], s.channel0_or_null(), Some(&half_turn), span)?,
            s.channel1_or_null(),
            s.channel2_or_null(),
            s.alpha_or_null(),
        )
    } else {
        Color::for_space_internal(
            space,
            s.channel0_or_null(),
            s.channel1_or_null(),
            adjust_channel(&s, &ch[2], s.channel2_or_null(), Some(&half_turn), span)?,
            s.alpha_or_null(),
        )
    };
    Ok(color_value(r.to_space(c.space(), false)))
}

// ### Module only

/// `_colorInSpace`.
fn color_in_space(v: &Value, space: &Value, legacy_missing: bool, span: Span) -> SassResult<Color> {
    let c = as_color(v, "color", span)?;
    if *space == Value::Null {
        return Ok((*c).clone());
    }
    Ok(c.to_space(
        space_by_name(&unquoted_string(space, "space", span)?, "space", span)?,
        legacy_missing,
    ))
}

pub(crate) fn space(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color.space", args.span);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    Ok(Value::String(
        as_color(&a[0], "color", span)?.space().name().to_owned(),
        QuoteKind::None,
    ))
}

pub(crate) fn to_space(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color.to-space", args.span);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color", "space"))?;
    Ok(color_value(color_in_space(&a[0], &a[1], false, span)?))
}

pub(crate) fn is_legacy(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color.is-legacy", args.span);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color"))?;
    Ok(Value::bool(as_color(&a[0], "color", span)?.is_legacy()))
}

/// `_channelName`: a **quoted** string.
fn channel_name(v: &Value, span: Span) -> SassResult<String> {
    match v {
        Value::String(s, QuoteKind::Quoted) => Ok(s.clone()),
        Value::String(s, QuoteKind::None) => fail(
            format!("$channel: Expected {s} to be a quoted string."),
            span,
        ),
        v => fail(
            format!("$channel: {} is not a string.", inspect_str(v, span)),
            span,
        ),
    }
}

fn no_such_channel<T>(v: &Value, channel: &str, span: Span) -> SassResult<T> {
    fail(
        format!(
            "$channel: Color {} doesn't have a channel named \"{channel}\".",
            inspect_str(v, span)
        ),
        span,
    )
}

pub(crate) fn is_missing(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color.is-missing", args.span);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color", "channel"))?;
    let c = as_color(&a[0], "color", span)?;
    let channel = channel_name(&a[1], span)?;
    match c.is_channel_missing(&channel) {
        Some(b) => Ok(Value::bool(b)),
        None => no_such_channel(&a[0], &channel, span),
    }
}

pub(crate) fn is_in_gamut(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color.is-in-gamut", args.span);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color", "space" = ParamDefault::Null))?;
    Ok(Value::bool(
        color_in_space(&a[0], &a[1], true, span)?.is_in_gamut(),
    ))
}

pub(crate) fn to_gamut(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color.to-gamut", args.span);
    }
    let span = args.span;
    let a = bind_args(
        args,
        &signature!(
            "color",
            "space" = ParamDefault::Null,
            "method" = ParamDefault::Null
        ),
    )?;
    let c = as_color(&a[0], "color", span)?;
    let space = if a[1] == Value::Null {
        c.space()
    } else {
        space_by_name(&unquoted_string(&a[1], "space", span)?, "space", span)?
    };
    if a[2] == Value::Null {
        return fail(
            "$method: color.to-gamut() requires a $method argument for forwards-compatibility with changes in the CSS spec. Suggestion:\n\n$method: local-minde",
            span,
        );
    }
    let method = match unquoted_string(&a[2], "method", span)?.as_str() {
        "clip" => GamutMapMethod::Clip,
        "local-minde" => GamutMapMethod::LocalMinde,
        m => return fail(format!("Unknown gamut map method \"{m}\"."), span),
    };
    if !space.is_bounded() {
        return Ok(Value::Color(c));
    }
    Ok(color_value(
        c.to_space(space, true)
            .to_gamut(method)
            .to_space(c.space(), false),
    ))
}

pub(crate) fn channel(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color.channel", args.span);
    }
    let span = args.span;
    let a = bind_args(
        args,
        &signature!("color", "channel", "space" = ParamDefault::Null),
    )?;
    let c = color_in_space(&a[0], &a[2], true, span)?;
    let channel = channel_name(&a[1], span)?;
    if channel == "alpha" {
        return Ok(num(c.alpha_f64(), Unit::None));
    }
    let info = c.space().channels();
    let Some(i) = info.iter().position(|ch| ch.name == channel) else {
        return fail(
            format!(
                "$channel: Color {} has no channel named {channel}.",
                inspect_str(&color_value(c.clone()), span)
            ),
            span,
        );
    };
    let ch = info[i];
    let mut value = [c.channel0(), c.channel1(), c.channel2()][i];
    let unit = match ch.associated_unit {
        Some("%") => {
            value = value * 100.0 / ch.max;
            Unit::Percent
        }
        Some("deg") => Unit::Deg,
        _ => Unit::None,
    };
    Ok(num(value, unit))
}

pub(crate) fn same(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color.same", args.span);
    }
    let span = args.span;
    let a = bind_args(args, &signature!("color1", "color2"))?;
    let c1 = as_color(&a[0], "color1", span)?;
    let c2 = as_color(&a[1], "color2", span)?;
    let xyz = |c: &Color| -> Color {
        if c.space() == ColorSpace::XyzD65 {
            if c.has_missing_channel() {
                Color::for_space_internal(
                    ColorSpace::XyzD65,
                    Some(c.channel0()),
                    Some(c.channel1()),
                    Some(c.channel2()),
                    Some(c.alpha_f64()),
                )
            } else {
                c.clone()
            }
        } else {
            c.space().convert(
                ColorSpace::XyzD65,
                Some(c.channel0()),
                Some(c.channel1()),
                Some(c.channel2()),
                Some(c.alpha_f64()),
            )
        }
    };
    Ok(Value::bool(if c1.space() == c2.space() {
        fuzzy_equals(c1.channel0(), c2.channel0())
            && fuzzy_equals(c1.channel1(), c2.channel1())
            && fuzzy_equals(c1.channel2(), c2.channel2())
            && fuzzy_equals(c1.alpha_f64(), c2.alpha_f64())
    } else {
        xyz(&c1) == xyz(&c2)
    }))
}

pub(crate) fn is_powerless(args: ArgumentResult, v: &mut Visitor) -> SassResult<Value> {
    if v.options.v166() {
        return v166::nonexistent("color.is-powerless", args.span);
    }
    let span = args.span;
    let a = bind_args(
        args,
        &signature!("color", "channel", "space" = ParamDefault::Null),
    )?;
    let c = color_in_space(&a[0], &a[2], true, span)?;
    let channel = channel_name(&a[1], span)?;
    match c.is_channel_powerless(&channel) {
        Some(b) => Ok(Value::bool(b)),
        None => no_such_channel(&color_value(c.clone()), &channel, span),
    }
}

/// `_removedColorFunction`.
fn removed_fn(args: ArgumentResult, name: &str, arg: &str, negative: bool) -> SassResult<Value> {
    let span = args.span;
    let a = bind_args(args, &signature!("color", "amount"))?;
    fail(
        format!(
            "The function {name}() isn't in the sass:color module.\n\nRecommendation: color.adjust({}, ${arg}: {}{})\n\nMore info: https://sass-lang.com/documentation/functions/color#{name}",
            inspect_str(&a[0], span),
            if negative { "-" } else { "" },
            inspect_str(&a[1], span)
        ),
        span,
    )
}

macro_rules! removed_fn {
    ($f:ident, $name:literal, $arg:literal, $neg:expr) => {
        pub(crate) fn $f(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
            removed_fn(args, $name, $arg, $neg)
        }
    };
}

removed_fn!(removed_adjust_hue, "adjust-hue", "hue", false);
removed_fn!(removed_lighten, "lighten", "lightness", false);
removed_fn!(removed_darken, "darken", "lightness", true);
removed_fn!(removed_saturate, "saturate", "saturation", false);
removed_fn!(removed_desaturate, "desaturate", "saturation", true);
removed_fn!(removed_opacify, "opacify", "alpha", false);
removed_fn!(removed_fade_in, "fade-in", "alpha", false);
removed_fn!(removed_transparentize, "transparentize", "alpha", true);
removed_fn!(removed_fade_out, "fade-out", "alpha", true);

/// The global functions (`global` of `functions/color.dart`).
pub(crate) fn declare(f: &mut GlobalFunctionMap) {
    let list: [(
        &'static str,
        fn(ArgumentResult, &mut Visitor) -> SassResult<Value>,
    ); 37] = [
        ("red", red),
        ("green", green),
        ("blue", blue),
        ("mix", mix),
        ("rgb", rgb),
        ("rgba", rgba),
        ("invert", invert_global),
        ("hue", hue),
        ("saturation", saturation),
        ("lightness", lightness),
        ("hsl", hsl),
        ("hsla", hsla),
        ("grayscale", grayscale_global),
        ("adjust-hue", adjust_hue),
        ("lighten", lighten),
        ("darken", darken),
        ("saturate", saturate),
        ("desaturate", desaturate),
        ("opacify", opacify),
        ("fade-in", fade_in),
        ("transparentize", transparentize),
        ("fade-out", fade_out),
        ("alpha", alpha_global),
        ("opacity", opacity_global),
        ("color", color_fn),
        ("hwb", hwb_global),
        ("lab", lab),
        ("lch", lch),
        ("oklab", oklab),
        ("oklch", oklch),
        ("complement", complement),
        ("ie-hex-str", ie_hex_str),
        ("adjust-color", adjust),
        ("scale-color", scale),
        ("change-color", change),
        ("whiteness", whiteness),
        ("blackness", blackness),
    ];
    for (name, func) in list {
        // `whiteness`/`blackness` are not global in dart-sass.
        if name == "whiteness" || name == "blackness" {
            continue;
        }
        f.insert(name, Builtin::new(func));
    }
}
