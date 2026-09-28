//! As funções de cor do Sass — porte de `lib/src/functions/color.dart` do
//! dart-sass 1.102.0 (globais e as do módulo `sass:color`), sobre o modelo
//! de cor de [`crate::color`].
//!
//! Os argumentos são ligados como no `ParameterList.verify` do dart-sass
//! (posicionais, nomeados, padrões e `$resto...`), e as funções
//! sobrecarregadas escolhem a assinatura como o
//! `BuiltInCallable.callbackFor`. Os avisos de depreciação do dart-sass não
//! são emitidos (a compilação roda sem logger).
use std::{cell::Cell, rc::Rc};

use crate::builtin::builtin_imports::*;
use crate::color::{
    clamp_like_css, dart_mod, ChannelInfo, ColorFormat, ColorSpace, GamutMapMethod,
    HueInterpolation, InterpolationMethod,
};
use crate::value::{conversion_factor, fuzzy_equals, ArgList};

use super::GlobalFunctionMap;

/// Um parâmetro: nome e valor padrão.
#[derive(Clone, Copy)]
enum Padrao {
    Nenhum,
    Nulo,
    Pct(f64),
    Num(f64),
}

type Param = (&'static str, Padrao);

/// Uma assinatura: os parâmetros e, se houver, o `$resto...`.
struct Assinatura {
    params: &'static [Param],
    resto: bool,
}

fn padrao(p: Padrao) -> Value {
    match p {
        Padrao::Nenhum | Padrao::Nulo => Value::Null,
        Padrao::Pct(n) => Value::Dimension(SassNumber {
            num: Number(n),
            unit: Unit::Percent,
            as_slash: None,
        }),
        Padrao::Num(n) => Value::Dimension(SassNumber::new_unitless(n)),
    }
}

/// `ParameterList.matches`.
fn casa(a: &Assinatura, posicionais: usize, nomes: &BTreeSet<Identifier>) -> bool {
    let mut usados = 0;
    for (i, (nome, p)) in a.params.iter().enumerate() {
        let ident = Identifier::from(*nome);
        if i < posicionais {
            if nomes.contains(&ident) {
                return false;
            }
        } else if nomes.contains(&ident) {
            usados += 1;
        } else if matches!(p, Padrao::Nenhum) {
            return false;
        }
    }
    if a.resto {
        return true;
    }
    if posicionais > a.params.len() {
        return false;
    }
    usados >= nomes.len()
}

/// `BuiltInCallable.callbackFor`: a sobrecarga que casa, ou a mais próxima.
fn escolher(sobrecargas: &[Assinatura], args: &ArgumentResult) -> usize {
    let posicionais = args.positional.len();
    let nomes: BTreeSet<Identifier> = args.named.keys().copied().collect();
    let mut fuzzy: Option<usize> = None;
    let mut min_dist: Option<isize> = None;
    for (i, a) in sobrecargas.iter().enumerate() {
        if casa(a, posicionais, &nomes) {
            return i;
        }
        let dist = a.params.len() as isize - posicionais as isize;
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

/// Liga os argumentos à assinatura (`verify` + a ligação do avaliador).
fn ligar(args: ArgumentResult, a: &Assinatura) -> SassResult<Vec<Value>> {
    let span = args.span;
    let ArgumentResult {
        mut positional,
        mut named,
        separator,
        ..
    } = args;
    let posicionais = positional.len();
    let mut usados = 0;
    for (i, (nome, p)) in a.params.iter().enumerate() {
        let ident = Identifier::from(*nome);
        if i < posicionais {
            if named.contains_key(&ident) {
                return Err((
                    format!("Argument ${nome} was passed both by position and by name."),
                    span,
                )
                    .into());
            }
        } else if named.contains_key(&ident) {
            usados += 1;
        } else if matches!(p, Padrao::Nenhum) {
            return Err((format!("Missing argument ${nome}."), span).into());
        }
    }
    if !a.resto {
        if posicionais > a.params.len() {
            let n = a.params.len();
            return Err((
                format!(
                    "Only {n} {}argument{} allowed, but {posicionais} {} passed.",
                    if named.is_empty() { "" } else { "positional " },
                    if n == 1 { "" } else { "s" },
                    if posicionais == 1 { "was" } else { "were" }
                ),
                span,
            )
                .into());
        }
        if usados < named.len() {
            let desconhecidos: Vec<String> = named
                .keys()
                .filter(|k| !a.params.iter().any(|(n, _)| Identifier::from(*n) == **k))
                .map(|k| format!("${k}"))
                .collect();
            return Err((
                format!(
                    "No argument{} named {}.",
                    if desconhecidos.len() == 1 { "" } else { "s" },
                    desconhecidos.join(" or ")
                ),
                span,
            )
                .into());
        }
    }
    let mut out = Vec::with_capacity(a.params.len() + 1);
    let resto_pos = if posicionais > a.params.len() {
        positional.split_off(a.params.len())
    } else {
        Vec::new()
    };
    let mut pos = positional.into_iter();
    for (nome, p) in a.params {
        let v = match pos.next() {
            Some(v) => v,
            None => named
                .remove(&Identifier::from(*nome))
                .unwrap_or_else(|| padrao(*p)),
        };
        out.push(v);
    }
    if a.resto {
        let sep = if separator == ListSeparator::Undecided {
            ListSeparator::Comma
        } else {
            separator
        };
        out.push(Value::ArgList(ArgList::new(
            resto_pos,
            Rc::new(Cell::new(false)),
            named,
            sep,
        )));
    }
    Ok(out)
}

macro_rules! assinatura {
    ($($nome:literal $(= $p:expr)?),* $(; $resto:ident)?) => {
        Assinatura {
            params: &[$(($nome, assinatura!(@p $($p)?))),*],
            resto: assinatura!(@r $($resto)?),
        }
    };
    (@p) => { Padrao::Nenhum };
    (@p $p:expr) => { $p };
    (@r) => { false };
    (@r $r:ident) => { true };
}

// ### Utilidades de valores

fn erro<T>(msg: impl Into<String>, span: Span) -> SassResult<T> {
    Err((msg.into(), span).into())
}

fn com_nome(msg: String, nome: Option<&str>) -> String {
    match nome {
        Some(n) => format!("${n}: {msg}"),
        None => msg,
    }
}

fn numero(v: &Value, nome: &str, span: Span) -> SassResult<SassNumber> {
    v.clone().assert_number_with_name(nome, span)
}

fn cor(v: &Value, nome: &str, span: Span) -> SassResult<Arc<Color>> {
    v.clone().assert_color_with_name(nome, span)
}

fn inspecionar(v: &Value, span: Span) -> String {
    v.inspect(span).unwrap_or_default()
}

fn num(n: f64, unit: Unit) -> Value {
    Value::Dimension(SassNumber {
        num: Number(n),
        unit,
        as_slash: None,
    })
}

fn tem_unidade(n: &SassNumber, u: &Unit) -> bool {
    n.unit == *u
}

fn sem_unidade(n: &SassNumber) -> bool {
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
fn value_in_range(n: &SassNumber, min: f64, max: f64, nome: &str, span: Span) -> SassResult<f64> {
    match fuzzy_check_range(n.num.0, min, max) {
        Some(v) => Ok(v),
        None => erro(
            format!(
                "${nome}: Expected {} to be within {min} and {max}.",
                inspecionar(&Value::Dimension(n.clone()), span)
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
    nome: &str,
    unit: &str,
    span: Span,
) -> SassResult<f64> {
    match fuzzy_check_range(n.num.0, min, max) {
        Some(v) => Ok(v),
        None => erro(
            format!(
                "${nome}: Expected {} to be within {min}{unit} and {max}{unit}.",
                inspecionar(&Value::Dimension(n.clone()), span)
            ),
            span,
        ),
    }
}

/// `coerceValueToUnit('deg')`, se compatível.
fn em_graus(n: &SassNumber) -> Option<f64> {
    if n.unit == Unit::None {
        return None;
    }
    conversion_factor(&n.unit, &Unit::Deg).map(|f| n.num.0 * f)
}

/// `_angleValue`: graus, ou o valor como está (com aviso, no oficial).
fn angle_value(v: &Value, nome: &str, span: Span) -> SassResult<f64> {
    let n = numero(v, nome, span)?;
    Ok(em_graus(&n).unwrap_or(n.num.0))
}

/// `_percentageOrUnitless`.
fn percentage_or_unitless(
    n: &SassNumber,
    max: f64,
    nome: Option<&str>,
    span: Span,
) -> SassResult<f64> {
    if sem_unidade(n) {
        Ok(n.num.0)
    } else if tem_unidade(n, &Unit::Percent) {
        Ok(max * n.num.0 / 100.0)
    } else {
        erro(
            com_nome(
                format!(
                    "Expected {} to have unit \"%\" or no units.",
                    inspecionar(&Value::Dimension(n.clone()), span)
                ),
                nome,
            ),
            span,
        )
    }
}

/// `isSpecialNumber` do dart-sass.
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

/// `isSpecialVariable` do dart-sass.
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
fn function_string(nome: &str, args: &[Value], span: Span) -> SassResult<Value> {
    let partes = args
        .iter()
        .map(|a| a.to_css_string(span, false))
        .collect::<SassResult<Vec<_>>>()?;
    Ok(Value::String(
        format!("{nome}({})", partes.join(", ")),
        QuoteKind::None,
    ))
}

fn cor_valor(c: Color) -> Value {
    Value::Color(Arc::new(c))
}

/// `_checkPercent` (só avisa no oficial).
fn check_percent(_n: &SassNumber, _nome: &str) {}

/// Texto `$x` de um string não entre aspas (`assertString` +
/// `assertUnquoted`).
fn texto_sem_aspas(v: &Value, nome: &str, span: Span) -> SassResult<String> {
    match v {
        Value::String(s, QuoteKind::None) => Ok(s.clone()),
        Value::String(s, QuoteKind::Quoted) => erro(
            format!("${nome}: Expected {s:?} to be an unquoted string."),
            span,
        ),
        v => erro(
            format!("${nome}: {} is not a string.", inspecionar(v, span)),
            span,
        ),
    }
}

fn espaco_por_nome(nome: &str, arg: &str, span: Span) -> SassResult<ColorSpace> {
    ColorSpace::from_name(nome)
        .ok_or_else(|| (format!("${arg}: Unknown color space \"{nome}\"."), span).into())
}

// ### Construção a partir de canais

/// `_channelFromValue`.
fn channel_from_value(
    ch: &ChannelInfo,
    v: Option<&SassNumber>,
    clamp: bool,
    span: Span,
) -> SassResult<Option<f64>> {
    let Some(v) = v else { return Ok(None) };
    if ch.linear {
        if ch.requires_percent && !tem_unidade(v, &Unit::Percent) {
            return erro(
                format!(
                    "${}: Expected {} to have unit \"%\".",
                    ch.name,
                    inspecionar(&Value::Dimension(v.clone()), span)
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
    // Matiz: `coerceValueToUnit('deg') % 360`.
    let graus = if v.unit == Unit::None {
        v.num.0
    } else {
        match conversion_factor(&v.unit, &Unit::Deg) {
            Some(f) => v.num.0 * f,
            None => {
                return erro(
                    format!(
                        "${}: Expected {} to have angle units (deg, grad, rad, turn).",
                        ch.name,
                        inspecionar(&Value::Dimension(v.clone()), span)
                    ),
                    span,
                )
            }
        }
    };
    Ok(Some(dart_mod(graus, 360.0)))
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
    let n = numero(v, "alpha", span)?;
    Ok(clamp_like_css(
        percentage_or_unitless(&n, 1.0, Some("alpha"), span)?,
        0.0,
        1.0,
    ))
}

/// `_rgb` (três ou quatro argumentos).
fn rgb3(nome: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let alpha = a.get(3);
    if a[..3].iter().any(is_special_number) || alpha.is_some_and(is_special_number) {
        return function_string(nome, a, span);
    }
    let r = numero(&a[0], "red", span)?;
    let g = numero(&a[1], "green", span)?;
    let b = numero(&a[2], "blue", span)?;
    let alpha = match alpha {
        Some(v) => alpha_clamped(v, span)?,
        None => 1.0,
    };
    Ok(cor_valor(color_from_channels(
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
fn rgb2(nome: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let (first, second) = (&a[0], &a[1]);
    if is_special_variable(first)
        || (!matches!(first, Value::Color(..)) && is_special_variable(second))
    {
        return function_string(nome, a, span);
    }
    let c = cor(first, "color", span)?;
    if !c.is_legacy() {
        return erro(
            format!(
                "${nome}: Expected {} to be in the legacy RGB, HSL, or HWB color space.",
                inspecionar(first, span)
            ),
            span,
        );
    }
    let c = c.to_space(ColorSpace::Rgb, true);
    if is_special_number(second) {
        return function_string(
            nome,
            &[
                num(c.channel0(), Unit::None),
                num(c.channel1(), Unit::None),
                num(c.channel2(), Unit::None),
                second.clone(),
            ],
            span,
        );
    }
    let alpha = numero(second, "alpha", span)?;
    Ok(cor_valor(c.change_alpha(clamp_like_css(
        percentage_or_unitless(&alpha, 1.0, Some("alpha"), span)?,
        0.0,
        1.0,
    ))))
}

/// `_hsl` (três ou quatro argumentos).
fn hsl3(nome: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let alpha = a.get(3);
    if a[..3].iter().any(is_special_number) || alpha.is_some_and(is_special_number) {
        return function_string(nome, a, span);
    }
    let h = numero(&a[0], "hue", span)?;
    let s = numero(&a[1], "saturation", span)?;
    let l = numero(&a[2], "lightness", span)?;
    let alpha = match alpha {
        Some(v) => alpha_clamped(v, span)?,
        None => 1.0,
    };
    Ok(cor_valor(color_from_channels(
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

/// `_parseNumberOrString`: um número do Sass ou o texto como está.
fn number_or_string(text: &str) -> Value {
    let t = text.trim();
    // Número: sinal, dígitos, ponto, expoente e unidade (identificador).
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let ini_dig = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i == ini_dig || (i == ini_dig + 1 && b[ini_dig] == b'.') || t.len() != text.len() {
        return Value::String(text.to_owned(), QuoteKind::None);
    }
    let Ok(v) = t[..i].parse::<f64>() else {
        return Value::String(text.to_owned(), QuoteKind::None);
    };
    let unidade = &t[i..];
    if unidade.is_empty() {
        return num(v, Unit::None);
    }
    if unidade == "%" {
        return num(v, Unit::Percent);
    }
    if unidade
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c == '-' || c == '_')
    {
        return num(v, Unit::from(unidade.to_owned()));
    }
    Value::String(text.to_owned(), QuoteKind::None)
}

/// `assertCommonListStyle`: a lista (ou o valor como lista de um).
fn common_list(
    v: &Value,
    nome: Option<&str>,
    allow_slash: bool,
    span: Span,
) -> SassResult<Vec<Value>> {
    let (elems, sep, brackets) = match v {
        Value::List(e, s, b) => (e.clone(), *s, *b),
        Value::ArgList(a) => (a.elems.clone(), a.separator, Brackets::None),
        Value::Map(m) if m.is_empty() => (Vec::new(), ListSeparator::Undecided, Brackets::None),
        v => (vec![v.clone()], ListSeparator::Undecided, Brackets::None),
    };
    let invalido = sep == ListSeparator::Comma || (!allow_slash && sep == ListSeparator::Slash);
    if invalido || brackets == Brackets::Bracketed {
        let quais = if allow_slash {
            "space- or slash-separated"
        } else {
            "space-separated"
        };
        return erro(
            com_nome(
                format!("Expected {quais} list, was {}", inspecionar(v, span)),
                nome,
            ),
            span,
        );
    }
    Ok(elems)
}

/// `_parseSlashChannels`: os componentes e o alfa, ou `None` se a entrada
/// deve ir como está.
fn parse_slash_channels(
    v: &Value,
    nome: Option<&str>,
    span: Span,
) -> SassResult<Option<(Value, Option<Value>)>> {
    let lista = common_list(v, nome, true, span)?;
    if v.separator() == ListSeparator::Slash {
        if lista.len() == 2 {
            return Ok(Some((lista[0].clone(), Some(lista[1].clone()))));
        }
        return erro(
            com_nome(
                format!(
                    "Only 2 slash-separated elements allowed, but {} {} passed.",
                    lista.len(),
                    if lista.len() == 1 { "was" } else { "were" }
                ),
                nome,
            ),
            span,
        );
    }
    match lista.last() {
        Some(Value::String(text, QuoteKind::None)) => {
            let partes: Vec<&str> = text.split('/').collect();
            match partes.as_slice() {
                [_] => Ok(Some((v.clone(), None))),
                [c3, a] => {
                    let mut ini = lista[..lista.len() - 1].to_vec();
                    ini.push(number_or_string(c3));
                    Ok(Some((
                        Value::List(ini, ListSeparator::Space, Brackets::None),
                        Some(number_or_string(a)),
                    )))
                }
                _ => Ok(None),
            }
        }
        Some(Value::Dimension(n)) if n.as_slash.is_some() => {
            let par = n.as_slash.clone().unwrap();
            let mut ini = lista[..lista.len() - 1].to_vec();
            ini.push(Value::Dimension(par.0.clone()));
            Ok(Some((
                Value::List(ini, ListSeparator::Space, Brackets::None),
                Some(Value::Dimension(par.1.clone())),
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
    nome: Option<&str>,
    span: Span,
) -> SassResult<Value> {
    if is_special_variable(input) {
        return function_string(function_name, &[input.clone()], span);
    }
    let Some((components, alpha_value)) = parse_slash_channels(input, nome, span)? else {
        return function_string(function_name, &[input.clone()], span);
    };
    let mut space = space;
    let lista = common_list(&components, nome, false, span)?;
    let channels: Vec<Value> = match lista.as_slice() {
        [] => {
            return erro(
                com_nome("Color component list may not be empty.".into(), nome),
                span,
            )
        }
        [Value::String(t, QuoteKind::None), ..] if t.eq_ignore_ascii_case("from") => {
            return function_string(function_name, &[input.clone()], span);
        }
        _ if is_special_variable(&components) => vec![components.clone()],
        [first, rest @ ..] => {
            let chs: Vec<Value> = if space.is_none() {
                let nome_espaco = match first {
                    Value::String(s, QuoteKind::None) => s.clone(),
                    Value::String(s, QuoteKind::Quoted) => {
                        return erro(
                            com_nome(format!("Expected {s:?} to be an unquoted string."), nome),
                            span,
                        )
                    }
                    v => {
                        return erro(
                            com_nome(format!("{} is not a string.", inspecionar(v, span)), nome),
                            span,
                        )
                    }
                };
                space = if is_special_variable(first) {
                    None
                } else {
                    Some(ColorSpace::from_name(&nome_espaco).ok_or_else(|| {
                        (
                            com_nome(format!("Unknown color space \"{nome_espaco}\"."), nome),
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
                    return erro(
                        com_nome(
                            format!(
                                "The color() function doesn't support the color space {s}. Use the {s}() function instead."
                            ),
                            nome,
                        ),
                        span,
                    );
                }
                rest.to_vec()
            } else {
                lista.clone()
            };
            for (i, c) in chs.iter().enumerate() {
                if !is_special_number(c) && !matches!(c, Value::Dimension(..)) && !is_none(c) {
                    let nome_canal = space
                        .and_then(|s| s.channels().get(i).map(|ch| format!("{} channel", ch.name)))
                        .unwrap_or_else(|| format!("channel {}", i + 1));
                    return erro(
                        com_nome(
                            format!(
                                "Expected {nome_canal} to be a number, was {}.",
                                inspecionar(c, span)
                            ),
                            nome,
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
                let mut todos = channels.clone();
                todos.push(a.clone());
                function_string(function_name, &todos, span)
            } else {
                function_string(function_name, &[input.clone()], span)
            };
        }
    }
    let alpha = match &alpha_value {
        None => Some(1.0),
        Some(Value::String(t, QuoteKind::None)) if t == "none" => None,
        Some(a) => {
            let n = numero(a, nome.unwrap_or("alpha"), span)?;
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
            let mut todos = channels.clone();
            if let Some(a) = alpha_value {
                todos.push(a);
            }
            function_string(function_name, &todos, span)
        } else {
            function_string(function_name, &[input.clone()], span)
        };
    }
    if channels.len() != 3 {
        return erro(
            com_nome(
                format!(
                    "The {space} color space has 3 channels but {} has {}.",
                    inspecionar(input, span),
                    channels.len()
                ),
                nome,
            ),
            span,
        );
    }
    let n = |v: &Value| match v {
        Value::Dimension(n) => Some(n.clone()),
        _ => None,
    };
    let (a0, a1, a2) = (n(&channels[0]), n(&channels[1]), n(&channels[2]));
    Ok(cor_valor(color_from_channels(
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

// ### Funções

fn channel_fn(
    a: &[Value],
    space: ColorSpace,
    canal: &str,
    unit: Unit,
    arredonda: bool,
    span: Span,
) -> SassResult<Value> {
    let c = cor(&a[0], "color", span)?;
    if !c.is_legacy() {
        return erro(
            format!(
                "color.{canal}() is only supported for legacy colors. Please use color.channel() instead with an explicit $space argument."
            ),
            span,
        );
    }
    let v = c.legacy_channel(space, canal);
    Ok(num(if arredonda { v.round() } else { v }, unit))
}

macro_rules! canal {
    ($f:ident, $space:expr, $canal:literal, $unit:expr, $arred:expr) => {
        pub(crate) fn $f(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
            let span = args.span;
            let a = ligar(args, &assinatura!("color"))?;
            channel_fn(&a, $space, $canal, $unit, $arred, span)
        }
    };
}

canal!(red, ColorSpace::Rgb, "red", Unit::None, true);
canal!(green, ColorSpace::Rgb, "green", Unit::None, true);
canal!(blue, ColorSpace::Rgb, "blue", Unit::None, true);
canal!(hue, ColorSpace::Hsl, "hue", Unit::Deg, false);
canal!(
    saturation,
    ColorSpace::Hsl,
    "saturation",
    Unit::Percent,
    false
);
canal!(
    lightness,
    ColorSpace::Hsl,
    "lightness",
    Unit::Percent,
    false
);
canal!(
    whiteness,
    ColorSpace::Hwb,
    "whiteness",
    Unit::Percent,
    false
);
canal!(
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
    let lista = common_list(v, Some("method"), false, span)?;
    if lista.is_empty() {
        return erro(
            "$method: Expected a color interpolation method, got an empty list.",
            span,
        );
    }
    let space = espaco_por_nome(&texto_sem_aspas(&lista[0], "method", span)?, "method", span)?;
    if lista.len() == 1 {
        return Ok(InterpolationMethod::new(space, None));
    }
    let hue = match texto_sem_aspas(&lista[1], "method", span)?
        .to_ascii_lowercase()
        .as_str()
    {
        "shorter" => HueInterpolation::Shorter,
        "longer" => HueInterpolation::Longer,
        "increasing" => HueInterpolation::Increasing,
        "decreasing" => HueInterpolation::Decreasing,
        _ => {
            return erro(
                format!(
                    "$method: Unknown hue interpolation method {}.",
                    inspecionar(&lista[1], span)
                ),
                span,
            )
        }
    };
    if lista.len() == 2 {
        return erro(
            format!(
                "$method: Expected unquoted string \"hue\" after {}.",
                inspecionar(v, span)
            ),
            span,
        );
    }
    if !texto_sem_aspas(&lista[2], "method", span)?.eq_ignore_ascii_case("hue") {
        return erro(
            format!(
                "$method: Expected unquoted string \"hue\" at the end of {}, was {}.",
                inspecionar(v, span),
                inspecionar(&lista[2], span)
            ),
            span,
        );
    }
    if lista.len() > 3 {
        return erro(
            format!(
                "$method: Expected nothing after \"hue\" in {}.",
                inspecionar(v, span)
            ),
            span,
        );
    }
    if !space.is_polar() {
        return erro(
            format!(
                "$method: Hue interpolation method \"{} hue\" may not be set for rectangular color space {space}.",
                hue.name()
            ),
            span,
        );
    }
    Ok(InterpolationMethod::new(space, Some(hue)))
}

pub(crate) fn mix(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(
        args,
        &assinatura!(
            "color1",
            "color2",
            "weight" = Padrao::Pct(50.0),
            "method" = Padrao::Nulo
        ),
    )?;
    let c1 = cor(&a[0], "color1", span)?;
    let c2 = cor(&a[1], "color2", span)?;
    let weight = numero(&a[2], "weight", span)?;
    if a[3] != Value::Null {
        let method = interpolation_method(&a[3], span)?;
        let w = value_in_range_with_unit(&weight, 0.0, 100.0, "weight", "%", span)? / 100.0;
        return Ok(cor_valor(c1.interpolate(&c2, method, w, false)));
    }
    check_percent(&weight, "weight");
    if !c1.is_legacy() {
        return erro(
            format!(
                "$color1: To use color.mix() with non-legacy color {}, you must provide a $method.",
                inspecionar(&a[0], span)
            ),
            span,
        );
    }
    if !c2.is_legacy() {
        return erro(
            format!(
                "$color2: To use color.mix() with non-legacy color {}, you must provide a $method.",
                inspecionar(&a[1], span)
            ),
            span,
        );
    }
    Ok(cor_valor(mix_legacy(&c1, &c2, &weight, span)?))
}

fn rgb_like(nome: &'static str, args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let sobrecargas = [
        assinatura!("red", "green", "blue", "alpha"),
        assinatura!("red", "green", "blue"),
        assinatura!("color", "alpha"),
        assinatura!("channels"),
    ];
    let i = escolher(&sobrecargas, &args);
    let a = ligar(args, &sobrecargas[i])?;
    match i {
        0 | 1 => rgb3(nome, &a, span),
        2 => rgb2(nome, &a, span),
        _ => parse_channels(nome, &a[0], Some(ColorSpace::Rgb), Some("channels"), span),
    }
}

pub(crate) fn rgb(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    rgb_like("rgb", args)
}

pub(crate) fn rgba(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    rgb_like("rgba", args)
}

fn hsl_like(nome: &'static str, args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let sobrecargas = [
        assinatura!("hue", "saturation", "lightness", "alpha"),
        assinatura!("hue", "saturation", "lightness"),
        assinatura!("hue", "saturation"),
        assinatura!("channels"),
    ];
    let i = escolher(&sobrecargas, &args);
    let a = ligar(args, &sobrecargas[i])?;
    match i {
        0 | 1 => hsl3(nome, &a, span),
        2 => {
            if is_special_variable(&a[0]) || is_special_variable(&a[1]) {
                function_string(nome, &a, span)
            } else {
                erro("Missing argument $lightness.", span)
            }
        }
        _ => parse_channels(nome, &a[0], Some(ColorSpace::Hsl), Some("channels"), span),
    }
}

pub(crate) fn hsl(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    hsl_like("hsl", args)
}

pub(crate) fn hsla(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
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

fn missing_channel_error<T>(c: &Color, canal: &str, span: Span) -> SassResult<T> {
    let texto = Value::Color(Arc::new(c.clone()))
        .to_css_string(span, false)
        .unwrap_or_default();
    erro(
        format!(
            "${canal}: Because the CSS working group is still deciding on the best behavior, Sass doesn't currently support modifying missing channels (color: {texto})."
        ),
        span,
    )
}

/// `_invert`.
fn invert_impl(a: &[Value], global: bool, span: Span) -> SassResult<Value> {
    let weight = numero(&a[1], "weight", span)?;
    if matches!(a[0], Value::Dimension(..)) || (global && is_special_number(&a[0])) {
        if weight.num.0 != 100.0 || !tem_unidade(&weight, &Unit::Percent) {
            return erro(
                "Only one argument may be passed to the plain-CSS invert() function.",
                span,
            );
        }
        return function_string("invert", &a[..1], span);
    }
    let c = cor(&a[0], "color", span)?;
    if a[2] == Value::Null {
        if !c.is_legacy() {
            return erro(
                format!(
                    "$color: To use color.invert() with non-legacy color {}, you must provide a $space.",
                    inspecionar(&a[0], span)
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
        return Ok(cor_valor(
            mix_legacy(&inv, &c, &weight, span)?.to_space(c.space(), true),
        ));
    }
    let space = espaco_por_nome(&texto_sem_aspas(&a[2], "space", span)?, "space", span)?;
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
    Ok(cor_valor(if fuzzy_equals(w, 1.0) {
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

pub(crate) fn invert_global(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(
        args,
        &assinatura!(
            "color",
            "weight" = Padrao::Pct(100.0),
            "space" = Padrao::Nulo
        ),
    )?;
    invert_impl(&a, true, span)
}

pub(crate) fn invert_module(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(
        args,
        &assinatura!(
            "color",
            "weight" = Padrao::Pct(100.0),
            "space" = Padrao::Nulo
        ),
    )?;
    invert_impl(&a, false, span)
}

/// `_grayscale`.
fn grayscale_impl(v: &Value, span: Span) -> SassResult<Value> {
    let c = cor(v, "color", span)?;
    Ok(cor_valor(if c.is_legacy() {
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

pub(crate) fn grayscale_global(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    if matches!(a[0], Value::Dimension(..)) || is_special_number(&a[0]) {
        return function_string("grayscale", &a, span);
    }
    grayscale_impl(&a[0], span)
}

pub(crate) fn grayscale_module(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    if matches!(a[0], Value::Dimension(..)) {
        return function_string("grayscale", &a[..1], span);
    }
    grayscale_impl(&a[0], span)
}

fn legado(c: &Color, nome: &str, span: Span) -> SassResult<()> {
    if c.is_legacy() {
        Ok(())
    } else {
        erro(
            format!(
                "{nome}() is only supported for legacy colors. Please use color.adjust() instead with an explicit $space argument."
            ),
            span,
        )
    }
}

pub(crate) fn adjust_hue(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "degrees"))?;
    let c = cor(&a[0], "color", span)?;
    let degrees = angle_value(&a[1], "degrees", span)?;
    legado(&c, "adjust-hue", span)?;
    let h = c.legacy_channel(ColorSpace::Hsl, "hue");
    Ok(cor_valor(c.change_hsl(Some(h + degrees), None, None)))
}

fn mudar_hsl(args: ArgumentResult, nome: &str, canal: &str, sinal: f64) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "amount"))?;
    let c = cor(&a[0], "color", span)?;
    let amount = numero(&a[1], "amount", span)?;
    legado(&c, nome, span)?;
    let atual = c.legacy_channel(ColorSpace::Hsl, canal);
    let novo = clamp_like_css(
        atual + sinal * value_in_range(&amount, 0.0, 100.0, "amount", span)?,
        0.0,
        100.0,
    );
    Ok(cor_valor(if canal == "lightness" {
        c.change_hsl(None, None, Some(novo))
    } else {
        c.change_hsl(None, Some(novo), None)
    }))
}

pub(crate) fn lighten(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    mudar_hsl(args, "lighten", "lightness", 1.0)
}

pub(crate) fn darken(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    mudar_hsl(args, "darken", "lightness", -1.0)
}

pub(crate) fn desaturate(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    mudar_hsl(args, "desaturate", "saturation", -1.0)
}

pub(crate) fn saturate(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let sobrecargas = [assinatura!("amount"), assinatura!("color", "amount")];
    let i = escolher(&sobrecargas, &args);
    if i == 0 {
        let a = ligar(args, &sobrecargas[0])?;
        if matches!(a[0], Value::Dimension(..)) || is_special_number(&a[0]) {
            return function_string("saturate", &a, span);
        }
        let n = numero(&a[0], "amount", span)?;
        let t = Value::Dimension(n).to_css_string(span, false)?;
        return Ok(Value::String(format!("saturate({t})"), QuoteKind::None));
    }
    mudar_hsl(args, "saturate", "saturation", 1.0)
}

fn mudar_alpha(args: ArgumentResult, nome: &str, sinal: f64) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "amount"))?;
    let c = cor(&a[0], "color", span)?;
    let amount = numero(&a[1], "amount", span)?;
    legado(&c, nome, span)?;
    let v = value_in_range_with_unit(&amount, 0.0, 1.0, "amount", "", span)?;
    Ok(cor_valor(c.change_alpha(clamp_like_css(
        c.alpha_f64() + sinal * v,
        0.0,
        1.0,
    ))))
}

pub(crate) fn opacify(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    mudar_alpha(args, "opacify", 1.0)
}

pub(crate) fn fade_in(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    mudar_alpha(args, "fade-in", 1.0)
}

pub(crate) fn transparentize(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    mudar_alpha(args, "transparentize", -1.0)
}

pub(crate) fn fade_out(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    mudar_alpha(args, "fade-out", -1.0)
}

/// `^[a-zA-Z]+\s*=` em algum ponto do texto (`contains` de `RegExp`).
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
        // O `^` do RegExp só casa no início (sem `multiLine`).
        if i != 0 {
            return false;
        }
        while j < b.len() && matches!(b[j], b' ' | b'\t' | b'\n' | b'\r' | b'\x0c') {
            j += 1;
        }
        j < b.len() && b[j] == b'='
    })
}

fn alpha_impl(args: ArgumentResult, modulo: bool) -> SassResult<Value> {
    let span = args.span;
    let sobrecargas = [assinatura!("color"), assinatura!(; args)];
    let i = escolher(&sobrecargas, &args);
    let a = ligar(args, &sobrecargas[i])?;
    if i == 0 {
        return match &a[0] {
            Value::String(t, QuoteKind::None) if microsoft_filter(t) => function_string("alpha", &a, span),
            Value::Color(c) if !c.is_legacy() => erro(
                format!(
                    "{}alpha() is only supported for legacy colors. Please use color.channel() instead.",
                    if modulo { "color." } else { "" }
                ),
                span,
            ),
            v => Ok(num(cor(v, "color", span)?.alpha_f64(), Unit::None)),
        };
    }
    let lista = match &a[0] {
        Value::ArgList(l) => l.elems.clone(),
        v => v.clone().as_list(),
    };
    let todos_filtro = lista
        .iter()
        .all(|v| matches!(v, Value::String(t, QuoteKind::None) if microsoft_filter(t)));
    if (modulo || !lista.is_empty()) && todos_filtro {
        return function_string("alpha", &lista, span);
    }
    if lista.is_empty() {
        return erro("Missing argument $color.", span);
    }
    erro(
        format!("Only 1 argument allowed, but {} were passed.", lista.len()),
        span,
    )
}

pub(crate) fn alpha_global(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    alpha_impl(args, false)
}

pub(crate) fn alpha_module(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    alpha_impl(args, true)
}

pub(crate) fn opacity_global(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    if matches!(a[0], Value::Dimension(..)) || is_special_number(&a[0]) {
        return function_string("opacity", &a, span);
    }
    Ok(num(cor(&a[0], "color", span)?.alpha_f64(), Unit::None))
}

pub(crate) fn opacity_module(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    if matches!(a[0], Value::Dimension(..)) {
        return function_string("opacity", &a, span);
    }
    Ok(num(cor(&a[0], "color", span)?.alpha_f64(), Unit::None))
}

macro_rules! espaco_fn {
    ($f:ident, $nome:literal, $space:expr) => {
        pub(crate) fn $f(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
            let span = args.span;
            let a = ligar(args, &assinatura!("channels"))?;
            parse_channels($nome, &a[0], $space, Some("channels"), span)
        }
    };
}

espaco_fn!(hwb_global, "hwb", Some(ColorSpace::Hwb));
espaco_fn!(lab, "lab", Some(ColorSpace::Lab));
espaco_fn!(lch, "lch", Some(ColorSpace::Lch));
espaco_fn!(oklab, "oklab", Some(ColorSpace::Oklab));
espaco_fn!(oklch, "oklch", Some(ColorSpace::Oklch));

pub(crate) fn color_fn(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("description"))?;
    parse_channels("color", &a[0], None, Some("description"), span)
}

pub(crate) fn hwb_module(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let sobrecargas = [
        assinatura!("hue", "whiteness", "blackness", "alpha" = Padrao::Num(1.0)),
        assinatura!("channels"),
    ];
    let i = escolher(&sobrecargas, &args);
    let a = ligar(args, &sobrecargas[i])?;
    if i == 0 {
        let lista = Value::List(
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
        return parse_channels("hwb", &lista, Some(ColorSpace::Hwb), None, span);
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
        return erro(format!("${}: Channel isn't scalable.", ch.name), span);
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
enum Modo {
    Change,
    Adjust,
    Scale,
}

/// `_updateComponents`.
fn update_components(a: &[Value], modo: Modo, span: Span) -> SassResult<Value> {
    let Value::ArgList(lista) = &a[1] else {
        unreachable!()
    };
    if !lista.elems.is_empty() {
        return erro(
            "Only one positional argument is allowed. All other arguments must be passed by name.",
            span,
        );
    }
    let mut keywords: Vec<(String, Value)> = lista
        .keywords()
        .iter()
        .map(|(k, v)| (k.as_str().to_owned(), v.clone()))
        .collect();
    let original = cor(&a[0], "color", span)?;
    let mut tirar = |nome: &str| -> Option<Value> {
        let i = keywords.iter().position(|(k, _)| k == nome)?;
        Some(keywords.remove(i).1)
    };
    let space_kw = match tirar("space") {
        Some(v) => Some(texto_sem_aspas(&v, "space", span)?),
        None => None,
    };
    let alpha_arg = tirar("alpha");
    let chaves: Vec<String> = keywords.iter().map(|(k, _)| k.clone()).collect();
    let color = if space_kw.is_none() && original.is_legacy() && !keywords.is_empty() {
        match sniff_legacy(&chaves) {
            Some(s) => original.to_space(s, false),
            None => (*original).clone(),
        }
    } else {
        match &space_kw {
            Some(s) => original.to_space(espaco_por_nome(s, "space", span)?, true),
            None => (*original).clone(),
        }
    };
    let info = color.space().channels();
    let mut channel_args: [Option<Value>; 3] = [None, None, None];
    for (nome, valor) in keywords {
        let Some(i) = info.iter().position(|c| c.name == nome) else {
            return erro(
                format!(
                    "${nome}: Color space {} doesn't have a channel with this name.",
                    color.space()
                ),
                span,
            );
        };
        channel_args[i] = Some(valor);
    }
    let result = match modo {
        Modo::Change => change_color(&color, &channel_args, alpha_arg.as_ref(), span)?,
        _ => {
            let mut nums: [Option<SassNumber>; 3] = [None, None, None];
            for i in 0..3 {
                if let Some(v) = &channel_args[i] {
                    nums[i] = Some(numero(v, info[i].name, span)?);
                }
            }
            let alpha_num = match &alpha_arg {
                Some(v) => Some(numero(v, "alpha", span)?),
                None => None,
            };
            let old = [
                color.channel0_or_null(),
                color.channel1_or_null(),
                color.channel2_or_null(),
            ];
            if modo == Modo::Scale {
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
    Ok(cor_valor(result.to_space(original.space(), false)))
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
    erro(
        format!(
            "${}: {} is not a number or unquoted \"none\".",
            c.space().channels()[i].name,
            inspecionar(arg, span)
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
        Some(Value::Dimension(n)) if sem_unidade(n) => {
            Some(value_in_range(n, 0.0, 1.0, "alpha", span)?)
        }
        Some(Value::Dimension(n)) if tem_unidade(n, &Unit::Percent) => {
            Some(value_in_range_with_unit(n, 0.0, 100.0, "alpha", "%", span)? / 100.0)
        }
        Some(Value::Dimension(n)) => Some(value_in_range(n, 0.0, 1.0, "alpha", span)?),
        Some(v) => {
            return erro(
                format!(
                    "$alpha: {} is not a number or unquoted \"none\".",
                    inspecionar(v, span)
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

macro_rules! atualiza {
    ($f:ident, $modo:expr) => {
        pub(crate) fn $f(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
            let span = args.span;
            let a = ligar(args, &assinatura!("color"; kwargs))?;
            update_components(&a, $modo, span)
        }
    };
}

atualiza!(adjust, Modo::Adjust);
atualiza!(scale, Modo::Scale);
atualiza!(change, Modo::Change);

pub(crate) fn ie_hex_str(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    let c = cor(&a[0], "color", span)?
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

pub(crate) fn complement(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "space" = Padrao::Nulo))?;
    let c = cor(&a[0], "color", span)?;
    let space = if c.is_legacy() && a[1] == Value::Null {
        ColorSpace::Hsl
    } else {
        espaco_por_nome(&texto_sem_aspas(&a[1], "space", span)?, "space", span)?
    };
    if !space.is_polar() {
        return erro(
            format!("$space: Color space {space} doesn't have a hue channel."),
            span,
        );
    }
    let s = c.to_space(space, a[1] != Value::Null);
    let ch = space.channels();
    let meia = SassNumber::new_unitless(180.0);
    let r = if space.is_legacy() {
        Color::for_space_internal(
            space,
            adjust_channel(&s, &ch[0], s.channel0_or_null(), Some(&meia), span)?,
            s.channel1_or_null(),
            s.channel2_or_null(),
            s.alpha_or_null(),
        )
    } else {
        Color::for_space_internal(
            space,
            s.channel0_or_null(),
            s.channel1_or_null(),
            adjust_channel(&s, &ch[2], s.channel2_or_null(), Some(&meia), span)?,
            s.alpha_or_null(),
        )
    };
    Ok(cor_valor(r.to_space(c.space(), false)))
}

// ### Só no módulo

/// `_colorInSpace`.
fn color_in_space(v: &Value, space: &Value, legacy_missing: bool, span: Span) -> SassResult<Color> {
    let c = cor(v, "color", span)?;
    if *space == Value::Null {
        return Ok((*c).clone());
    }
    Ok(c.to_space(
        espaco_por_nome(&texto_sem_aspas(space, "space", span)?, "space", span)?,
        legacy_missing,
    ))
}

pub(crate) fn space(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    Ok(Value::String(
        cor(&a[0], "color", span)?.space().name().to_owned(),
        QuoteKind::None,
    ))
}

pub(crate) fn to_space(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "space"))?;
    Ok(cor_valor(color_in_space(&a[0], &a[1], false, span)?))
}

pub(crate) fn is_legacy(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    Ok(Value::bool(cor(&a[0], "color", span)?.is_legacy()))
}

/// `_channelName`: string **entre aspas**.
fn channel_name(v: &Value, span: Span) -> SassResult<String> {
    match v {
        Value::String(s, QuoteKind::Quoted) => Ok(s.clone()),
        Value::String(s, QuoteKind::None) => erro(
            format!("$channel: Expected {s} to be a quoted string."),
            span,
        ),
        v => erro(
            format!("$channel: {} is not a string.", inspecionar(v, span)),
            span,
        ),
    }
}

fn sem_canal<T>(v: &Value, canal: &str, span: Span) -> SassResult<T> {
    erro(
        format!(
            "$channel: Color {} doesn't have a channel named \"{canal}\".",
            inspecionar(v, span)
        ),
        span,
    )
}

pub(crate) fn is_missing(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "channel"))?;
    let c = cor(&a[0], "color", span)?;
    let canal = channel_name(&a[1], span)?;
    match c.is_channel_missing(&canal) {
        Some(b) => Ok(Value::bool(b)),
        None => sem_canal(&a[0], &canal, span),
    }
}

pub(crate) fn is_in_gamut(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "space" = Padrao::Nulo))?;
    Ok(Value::bool(
        color_in_space(&a[0], &a[1], true, span)?.is_in_gamut(),
    ))
}

pub(crate) fn to_gamut(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(
        args,
        &assinatura!("color", "space" = Padrao::Nulo, "method" = Padrao::Nulo),
    )?;
    let c = cor(&a[0], "color", span)?;
    let space = if a[1] == Value::Null {
        c.space()
    } else {
        espaco_por_nome(&texto_sem_aspas(&a[1], "space", span)?, "space", span)?
    };
    if a[2] == Value::Null {
        return erro(
            "$method: color.to-gamut() requires a $method argument for forwards-compatibility with changes in the CSS spec. Suggestion:\n\n$method: local-minde",
            span,
        );
    }
    let method = match texto_sem_aspas(&a[2], "method", span)?.as_str() {
        "clip" => GamutMapMethod::Clip,
        "local-minde" => GamutMapMethod::LocalMinde,
        m => return erro(format!("Unknown gamut map method \"{m}\"."), span),
    };
    if !space.is_bounded() {
        return Ok(Value::Color(c));
    }
    Ok(cor_valor(
        c.to_space(space, true)
            .to_gamut(method)
            .to_space(c.space(), false),
    ))
}

pub(crate) fn channel(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(
        args,
        &assinatura!("color", "channel", "space" = Padrao::Nulo),
    )?;
    let c = color_in_space(&a[0], &a[2], true, span)?;
    let canal = channel_name(&a[1], span)?;
    if canal == "alpha" {
        return Ok(num(c.alpha_f64(), Unit::None));
    }
    let info = c.space().channels();
    let Some(i) = info.iter().position(|ch| ch.name == canal) else {
        return erro(
            format!(
                "$channel: Color {} has no channel named {canal}.",
                inspecionar(&cor_valor(c.clone()), span)
            ),
            span,
        );
    };
    let ch = info[i];
    let mut valor = [c.channel0(), c.channel1(), c.channel2()][i];
    let unit = match ch.associated_unit {
        Some("%") => {
            valor = valor * 100.0 / ch.max;
            Unit::Percent
        }
        Some("deg") => Unit::Deg,
        _ => Unit::None,
    };
    Ok(num(valor, unit))
}

pub(crate) fn same(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color1", "color2"))?;
    let c1 = cor(&a[0], "color1", span)?;
    let c2 = cor(&a[1], "color2", span)?;
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

pub(crate) fn is_powerless(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(
        args,
        &assinatura!("color", "channel", "space" = Padrao::Nulo),
    )?;
    let c = color_in_space(&a[0], &a[2], true, span)?;
    let canal = channel_name(&a[1], span)?;
    match c.is_channel_powerless(&canal) {
        Some(b) => Ok(Value::bool(b)),
        None => sem_canal(&cor_valor(c.clone()), &canal, span),
    }
}

/// `_removedColorFunction`.
fn removida(args: ArgumentResult, nome: &str, arg: &str, negativo: bool) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "amount"))?;
    erro(
        format!(
            "The function {nome}() isn't in the sass:color module.\n\nRecommendation: color.adjust({}, ${arg}: {}{})\n\nMore info: https://sass-lang.com/documentation/functions/color#{nome}",
            inspecionar(&a[0], span),
            if negativo { "-" } else { "" },
            inspecionar(&a[1], span)
        ),
        span,
    )
}

macro_rules! removida {
    ($f:ident, $nome:literal, $arg:literal, $neg:expr) => {
        pub(crate) fn $f(args: ArgumentResult, _v: &mut Visitor) -> SassResult<Value> {
            removida(args, $nome, $arg, $neg)
        }
    };
}

removida!(removida_adjust_hue, "adjust-hue", "hue", false);
removida!(removida_lighten, "lighten", "lightness", false);
removida!(removida_darken, "darken", "lightness", true);
removida!(removida_saturate, "saturate", "saturation", false);
removida!(removida_desaturate, "desaturate", "saturation", true);
removida!(removida_opacify, "opacify", "alpha", false);
removida!(removida_fade_in, "fade-in", "alpha", false);
removida!(removida_transparentize, "transparentize", "alpha", true);
removida!(removida_fade_out, "fade-out", "alpha", true);

/// As funções globais (`global` do `functions/color.dart`).
pub(crate) fn declare(f: &mut GlobalFunctionMap) {
    let lista: [(
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
    for (nome, func) in lista {
        // `whiteness`/`blackness` não são globais no dart-sass.
        if nome == "whiteness" || nome == "blackness" {
            continue;
        }
        f.insert(nome, Builtin::new(func));
    }
}
