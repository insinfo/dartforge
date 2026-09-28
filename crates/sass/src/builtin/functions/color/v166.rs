//! As funções de cor do dart-sass 1.66.0 (`lib/src/functions/color.dart`
//! dessa versão), para o modo de compatibilidade
//! ([`crate::VersaoDartSass::V1_66_0`]), sobre o modelo de
//! [`crate::color::v166`].
//!
//! Cada função do [`super`] chama a daqui quando o modo está ligado. As que
//! o 1.66 não tinha (as do CSS Color 4: `lab()`, `oklch()`, `color()`,
//! `hwb()` global e as de espaço do `sass:color`) são recusadas: no 1.66
//! seriam funções CSS puras ou "Undefined function", e a saída não é
//! garantida.
use super::*;
use crate::color::v166 as m;

fn cor166(c: Result<Color, String>, span: Span) -> SassResult<Value> {
    c.map(cor_valor).map_err(|e| (e, span).into())
}

/// A cor do argumento, que o modelo do 1.66 precisa representar.
fn cor_arg(v: &Value, nome: &str, span: Span) -> SassResult<Arc<Color>> {
    let c = cor(v, nome, span)?;
    if m::rgb(&c).is_none() {
        return erro(m::nao_garantida(&c), span);
    }
    Ok(c)
}

fn rgb_de(c: &Color) -> [f64; 3] {
    m::rgb(c).unwrap_or([0.0; 3])
}

fn hsl_de(c: &Color) -> [f64; 3] {
    m::hsl(c).unwrap_or([0.0; 3])
}

/// `SassString.isSpecialNumber` do 1.66 (sem `attr()` e `if()`).
fn especial(v: &Value) -> bool {
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

/// `SassString.isVar` do 1.66.
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

/// `_percentageOrUnitless` do 1.66: corta em `[0, max]`.
fn pct_ou_sem(n: &SassNumber, max: f64, nome: &str, span: Span) -> SassResult<f64> {
    let v = if sem_unidade(n) {
        n.num.0
    } else if tem_unidade(n, &Unit::Percent) {
        max * n.num.0 / 100.0
    } else {
        return erro(
            format!(
                "${nome}: Expected {} to have no units or \"%\".",
                inspecionar(&Value::Dimension(n.clone()), span)
            ),
            span,
        );
    };
    Ok(v.clamp(0.0, max))
}

/// `assertUnit('%')`.
fn exige_pct(n: &SassNumber, nome: &str, span: Span) -> SassResult<()> {
    if tem_unidade(n, &Unit::Percent) {
        return Ok(());
    }
    erro(
        format!(
            "${nome}: Expected {} to have unit \"%\".",
            inspecionar(&Value::Dimension(n.clone()), span)
        ),
        span,
    )
}

/// Recusa uma função que o 1.66 não tinha.
pub(crate) fn inexistente(nome: &str, span: Span) -> SassResult<Value> {
    erro(
        format!(
            "dartforge-sass (modo dart-sass 1.66): {nome}() não existe no dart-sass \
             1.66 (é do CSS Color 4, dart-sass 1.79+); a saída do 1.66 não é garantida"
        ),
        span,
    )
}

// ### RGB

/// `_rgb`.
fn rgb4(nome: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let alpha = a.get(3);
    if a[..3].iter().any(especial) || alpha.is_some_and(especial) {
        return function_string(nome, a, span);
    }
    let r = numero(&a[0], "red", span)?;
    let g = numero(&a[1], "green", span)?;
    let b = numero(&a[2], "blue", span)?;
    let alpha = match alpha {
        Some(v) => pct_ou_sem(&numero(v, "alpha", span)?, 1.0, "alpha", span)?,
        None => 1.0,
    };
    cor166(
        m::de_rgb(
            m::fuzzy_round(pct_ou_sem(&r, 255.0, "red", span)?),
            m::fuzzy_round(pct_ou_sem(&g, 255.0, "green", span)?),
            m::fuzzy_round(pct_ou_sem(&b, 255.0, "blue", span)?),
            alpha,
            ColorFormat::RgbFunction,
        ),
        span,
    )
}

/// `_rgbTwoArg`.
fn rgb2_166(nome: &str, a: &[Value], span: Span) -> SassResult<Value> {
    if is_var(&a[0]) || (!matches!(a[0], Value::Color(..)) && is_var(&a[1])) {
        return function_string(nome, a, span);
    }
    if especial(&a[1]) {
        let c = cor_arg(&a[0], "color", span)?;
        let [r, g, b] = rgb_de(&c);
        return Ok(Value::String(
            format!(
                "{nome}({r}, {g}, {b}, {})",
                a[1].to_css_string(span, false)?
            ),
            QuoteKind::None,
        ));
    }
    let c = cor_arg(&a[0], "color", span)?;
    let alpha = numero(&a[1], "alpha", span)?;
    cor166(
        m::com_alpha(&c, pct_ou_sem(&alpha, 1.0, "alpha", span)?),
        span,
    )
}

/// `_parseChannels`: os argumentos, ou a função CSS como texto.
fn parse_channels(
    nome: &str,
    argumentos: &[&str],
    channels: &Value,
    span: Span,
) -> SassResult<Result<Vec<Value>, Value>> {
    if is_var(channels) {
        return Ok(Err(function_string(nome, &[channels.clone()], span)?));
    }
    let original = channels.clone();
    let mut channels = channels.clone();
    let mut alpha_da_barra = None;
    if channels.separator() == ListSeparator::Slash {
        let lista = channels.clone().as_list();
        if lista.len() != 2 {
            return erro(
                format!(
                    "Only 2 slash-separated elements allowed, but {} {} passed.",
                    lista.len(),
                    if lista.len() == 1 { "was" } else { "were" }
                ),
                span,
            );
        }
        channels = lista[0].clone();
        let alpha = lista[1].clone();
        if !especial(&alpha) {
            numero(&alpha, "alpha", span)?;
        }
        alpha_da_barra = Some(alpha);
        if is_var(&lista[0]) {
            return Ok(Err(function_string(nome, &[original], span)?));
        }
    }
    let virgula = channels.separator() == ListSeparator::Comma;
    let colchetes = matches!(channels, Value::List(_, _, Brackets::Bracketed));
    if virgula || colchetes {
        let mut msg = String::from("$channels must be");
        if colchetes {
            msg.push_str(" an unbracketed");
        }
        if virgula {
            msg.push_str(if colchetes { "," } else { " a" });
            msg.push_str(" space-separated");
        }
        msg.push_str(" list.");
        return erro(msg, span);
    }
    let lista = channels.clone().as_list();
    if lista.len() > 3 {
        return erro(
            format!("Only 3 elements allowed, but {} were passed.", lista.len()),
            span,
        );
    } else if lista.len() < 3 {
        // `_isVarSlash`: o 1.66 exige aspas (um erro dele, portado igual).
        let var_barra = |v: &Value| {
            matches!(v, Value::String(t, QuoteKind::Quoted)
                if t.len() >= 4 && t[..4].eq_ignore_ascii_case("var(") && t.contains('/'))
        };
        if lista.iter().any(is_var) || lista.last().is_some_and(var_barra) {
            return Ok(Err(function_string(nome, &[original], span)?));
        }
        return erro(
            format!("Missing element {}.", argumentos[lista.len()]),
            span,
        );
    }
    if let Some(alpha) = alpha_da_barra {
        let mut v = lista;
        v.push(alpha);
        return Ok(Ok(v));
    }
    match &lista[2] {
        Value::Dimension(SassNumber {
            as_slash: Some(barra),
            ..
        }) => Ok(Ok(vec![
            lista[0].clone(),
            lista[1].clone(),
            Value::Dimension(barra.0.clone()),
            Value::Dimension(barra.1.clone()),
        ])),
        Value::String(t, QuoteKind::None) if t.contains('/') => {
            Ok(Err(function_string(nome, &[channels], span)?))
        }
        _ => Ok(Ok(lista)),
    }
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
        0 | 1 => rgb4(nome, &a, span),
        2 => rgb2_166(nome, &a, span),
        _ => match parse_channels(nome, &["$red", "$green", "$blue"], &a[0], span)? {
            Ok(v) => rgb4(nome, &v, span),
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
/// e `blackness` do 1.66.
pub(crate) fn canal(nome: &str, args: ArgumentResult) -> SassResult<Value> {
    match nome {
        "red" => canal_rgb(args, 0),
        "green" => canal_rgb(args, 1),
        "blue" => canal_rgb(args, 2),
        "hue" => canal_hsl(args, 0, Unit::Deg),
        "saturation" => canal_hsl(args, 1, Unit::Percent),
        "lightness" => canal_hsl(args, 2, Unit::Percent),
        "whiteness" => whiteness_blackness(args, 0),
        _ => whiteness_blackness(args, 1),
    }
}

fn canal_rgb(args: ArgumentResult, i: usize) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    let c = cor_arg(&a[0], "color", span)?;
    Ok(num(rgb_de(&c)[i], Unit::None))
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
    let ([r1, g1, b1], [r2, g2, b2]) = (rgb_de(c1), rgb_de(c2));
    cor166(
        m::de_rgb(
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
    let a = ligar(
        args,
        &assinatura!("color1", "color2", "weight" = Padrao::Pct(50.0)),
    )?;
    let c1 = cor_arg(&a[0], "color1", span)?;
    let c2 = cor_arg(&a[1], "color2", span)?;
    let weight = numero(&a[2], "weight", span)?;
    mix_colors(&c1, &c2, &weight, span)
}

pub(crate) fn invert(args: ArgumentResult, global: bool) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "weight" = Padrao::Pct(100.0)))?;
    let weight = numero(&a[1], "weight", span)?;
    if matches!(a[0], Value::Dimension(..)) || (global && especial(&a[0])) {
        if weight.num.0 != 100.0 || !tem_unidade(&weight, &Unit::Percent) {
            return erro(
                "Only one argument may be passed to the plain-CSS invert() function.",
                span,
            );
        }
        return function_string("invert", &a[..1], span);
    }
    let c = cor_arg(&a[0], "color", span)?;
    let [r, g, b] = rgb_de(&c);
    let inverso = m::de_rgb(
        255.0 - r,
        255.0 - g,
        255.0 - b,
        m::alpha(&c),
        ColorFormat::Infer,
    )
    .map_err(|e| (e, span))?;
    mix_colors(&inverso, &c, &weight, span)
}

// ### HSL

fn canal_hsl(args: ArgumentResult, i: usize, unit: Unit) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    let c = cor_arg(&a[0], "color", span)?;
    Ok(num(hsl_de(&c)[i], unit))
}

/// `_hsl`.
fn hsl4(nome: &str, a: &[Value], span: Span) -> SassResult<Value> {
    let alpha = a.get(3);
    if a[..3].iter().any(especial) || alpha.is_some_and(especial) {
        return function_string(nome, a, span);
    }
    let h = angle_value(&a[0], "hue", span)?;
    let s = numero(&a[1], "saturation", span)?;
    let l = numero(&a[2], "lightness", span)?;
    let alpha = match alpha {
        Some(v) => pct_ou_sem(&numero(v, "alpha", span)?, 1.0, "alpha", span)?,
        None => 1.0,
    };
    cor166(
        m::de_hsl(
            h,
            s.num.0.clamp(0.0, 100.0),
            l.num.0.clamp(0.0, 100.0),
            alpha,
            ColorFormat::HslFunction,
        ),
        span,
    )
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
        0 | 1 => hsl4(nome, &a, span),
        2 => {
            if is_var(&a[0]) || is_var(&a[1]) {
                function_string(nome, &a, span)
            } else {
                erro("Missing argument $lightness.", span)
            }
        }
        _ => match parse_channels(nome, &["$hue", "$saturation", "$lightness"], &a[0], span)? {
            Ok(v) => hsl4(nome, &v, span),
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

/// `changeHsl`: os canais dados trocam, os outros vêm da cor.
fn change_hsl(
    c: &Color,
    h: Option<f64>,
    s: Option<f64>,
    l: Option<f64>,
    alpha: Option<f64>,
    span: Span,
) -> SassResult<Value> {
    let [h0, s0, l0] = hsl_de(c);
    cor166(
        m::de_hsl(
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
    let a = ligar(args, &assinatura!("color"))?;
    if matches!(a[0], Value::Dimension(..)) || (global && especial(&a[0])) {
        return function_string("grayscale", &a[..1], span);
    }
    let c = cor_arg(&a[0], "color", span)?;
    change_hsl(&c, None, Some(0.0), None, None, span)
}

pub(crate) fn complement(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    let c = cor_arg(&a[0], "color", span)?;
    change_hsl(&c, Some(hsl_de(&c)[0] + 180.0), None, None, None, span)
}

pub(crate) fn adjust_hue(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "degrees"))?;
    let c = cor_arg(&a[0], "color", span)?;
    let graus = angle_value(&a[1], "degrees", span)?;
    change_hsl(&c, Some(hsl_de(&c)[0] + graus), None, None, None, span)
}

/// `lighten`/`darken` (`canal` 2) e `saturate`/`desaturate` (`canal` 1).
pub(crate) fn mudar(args: ArgumentResult, canal: usize, sinal: f64) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "amount"))?;
    let c = cor_arg(&a[0], "color", span)?;
    let amount = value_in_range(&numero(&a[1], "amount", span)?, 0.0, 100.0, "amount", span)?;
    let v = (hsl_de(&c)[canal] + sinal * amount).clamp(0.0, 100.0);
    if canal == 1 {
        change_hsl(&c, None, Some(v), None, None, span)
    } else {
        change_hsl(&c, None, None, Some(v), None, span)
    }
}

/// `saturate`: com um argumento, o filtro CSS.
pub(crate) fn saturate(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let sobrecargas = [assinatura!("amount"), assinatura!("color", "amount")];
    if escolher(&sobrecargas, &args) == 0 {
        let a = ligar(args, &sobrecargas[0])?;
        if matches!(a[0], Value::Dimension(..)) || especial(&a[0]) {
            return function_string("saturate", &a, span);
        }
        let n = numero(&a[0], "amount", span)?;
        return Ok(Value::String(
            format!(
                "saturate({})",
                Value::Dimension(n).to_css_string(span, false)?
            ),
            QuoteKind::None,
        ));
    }
    mudar(args, 1, 1.0)
}

// ### Opacidade

/// `_opacify`/`_transparentize`.
pub(crate) fn opacidade(args: ArgumentResult, sinal: f64) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color", "amount"))?;
    let c = cor_arg(&a[0], "color", span)?;
    let amount = value_in_range_with_unit(
        &numero(&a[1], "amount", span)?,
        0.0,
        1.0,
        "amount",
        "",
        span,
    )?;
    cor166(
        m::com_alpha(&c, (m::alpha(&c) + sinal * amount).clamp(0.0, 1.0)),
        span,
    )
}

/// `alpha()` global e do módulo (o filtro da Microsoft é texto).
pub(crate) fn alpha(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let sobrecargas = [assinatura!("color"), assinatura!(; args)];
    let i = escolher(&sobrecargas, &args);
    let a = ligar(args, &sobrecargas[i])?;
    if i == 0 {
        if let Value::String(t, QuoteKind::None) = &a[0] {
            if microsoft_filter(t) {
                return function_string("alpha", &a, span);
            }
        }
        let c = cor_arg(&a[0], "color", span)?;
        return Ok(num(m::alpha(&c), Unit::None));
    }
    let lista = match &a[0] {
        Value::ArgList(l) => l.elems.clone(),
        v => v.clone().as_list(),
    };
    if !lista.is_empty()
        && lista
            .iter()
            .all(|v| matches!(v, Value::String(t, QuoteKind::None) if microsoft_filter(t)))
    {
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

pub(crate) fn opacity(args: ArgumentResult, global: bool) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    if matches!(a[0], Value::Dimension(..)) || (global && especial(&a[0])) {
        return function_string("opacity", &a, span);
    }
    let c = cor_arg(&a[0], "color", span)?;
    Ok(num(m::alpha(&c), Unit::None))
}

// ### HWB (módulo)

pub(crate) fn hwb(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let sobrecargas = [
        assinatura!("hue", "whiteness", "blackness", "alpha" = Padrao::Num(1.0)),
        assinatura!("channels"),
    ];
    let i = escolher(&sobrecargas, &args);
    let a = ligar(args, &sobrecargas[i])?;
    let a = if i == 0 {
        a
    } else {
        match parse_channels("hwb", &["$hue", "$whiteness", "$blackness"], &a[0], span)? {
            Ok(v) => v,
            Err(s) => {
                return erro(
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
    let w = numero(&a[1], "whiteness", span)?;
    let b = numero(&a[2], "blackness", span)?;
    exige_pct(&w, "whiteness", span)?;
    exige_pct(&b, "blackness", span)?;
    let alpha = match a.get(3) {
        Some(v) => pct_ou_sem(&numero(v, "alpha", span)?, 1.0, "alpha", span)?,
        None => 1.0,
    };
    cor166(
        m::de_hwb(
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
    let a = ligar(args, &assinatura!("color"))?;
    let c = cor_arg(&a[0], "color", span)?;
    Ok(num(m::hwb(&c).unwrap_or([0.0; 2])[i], Unit::Percent))
}

// ### Outras

pub(crate) fn ie_hex_str(args: ArgumentResult) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"))?;
    let c = cor_arg(&a[0], "color", span)?;
    let [r, g, b] = rgb_de(&c);
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

/// `_updateComponents` (`adjust`, `scale` e `change`).
pub(crate) fn update_components(args: ArgumentResult, modo: Modo) -> SassResult<Value> {
    let span = args.span;
    let a = ligar(args, &assinatura!("color"; kwargs))?;
    let c = cor_arg(&a[0], "color", span)?;
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
    let (change, adjust, scale) = (
        matches!(modo, Modo::Change),
        matches!(modo, Modo::Adjust),
        matches!(modo, Modo::Scale),
    );
    // `getParam`.
    let mut param = |nome: &str,
                     max: f64,
                     check_percent: bool,
                     assert_percent: bool|
     -> SassResult<Option<f64>> {
        let Some(i) = keywords.iter().position(|(k, _)| k == nome) else {
            return Ok(None);
        };
        let n = numero(&keywords.remove(i).1, nome, span)?;
        if scale || assert_percent {
            exige_pct(&n, nome, span)?;
        }
        let max = if scale { 100.0 } else { max };
        let min = if change { 0.0 } else { -max };
        Ok(Some(if scale || assert_percent {
            value_in_range(&n, min, max, nome, span)?
        } else {
            value_in_range_with_unit(
                &n,
                min,
                max,
                nome,
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
        |nome: &str, check_percent: bool, assert_percent: bool| -> SassResult<Option<f64>> {
            let Some(i) = keywords.iter().position(|(k, _)| k == nome) else {
                return Ok(None);
            };
            let n = numero(&keywords.remove(i).1, nome, span)?;
            if scale || assert_percent {
                exige_pct(&n, nome, span)?;
            }
            let min = if change { 0.0 } else { -100.0 };
            Ok(Some(if scale || assert_percent {
                value_in_range(&n, min, 100.0, nome, span)?
            } else {
                value_in_range_with_unit(
                    &n,
                    min,
                    100.0,
                    nome,
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
        let nomes: Vec<String> = keywords.iter().map(|(k, _)| format!("${k}")).collect();
        let sentenca = match nomes.as_slice() {
            [um] => um.clone(),
            [resto @ .., ultimo] => format!("{} or {ultimo}", resto.join(", ")),
            [] => String::new(),
        };
        return erro(
            format!(
                "No argument{} named {sentenca}.",
                if nomes.len() == 1 { "" } else { "s" }
            ),
            span,
        );
    }
    let has_rgb = red.is_some() || green.is_some() || blue.is_some();
    let has_sl = saturation.is_some() || lightness.is_some();
    let has_wb = whiteness.is_some() || blackness.is_some();
    if has_rgb && (has_sl || has_wb || hue.is_some()) {
        return erro(
            format!(
                "RGB parameters may not be passed along with {} parameters.",
                if has_wb { "HWB" } else { "HSL" }
            ),
            span,
        );
    }
    if has_sl && has_wb {
        return erro(
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
        let [r, g, b] = rgb_de(&c);
        return cor166(
            m::de_rgb(
                m::fuzzy_round(update(r, red, 255.0)),
                m::fuzzy_round(update(g, green, 255.0)),
                m::fuzzy_round(update(b, blue, 255.0)),
                update(a0, alpha, 1.0),
                ColorFormat::Infer,
            ),
            span,
        );
    }
    let [h0, s0, l0] = hsl_de(&c);
    let nova_matiz = if change {
        hue.unwrap_or(h0)
    } else {
        h0 + hue.unwrap_or(0.0)
    };
    if has_wb {
        let [w0, b0] = m::hwb(&c).unwrap_or([0.0; 2]);
        return cor166(
            m::de_hwb(
                nova_matiz,
                update(w0, whiteness, 100.0),
                update(b0, blackness, 100.0),
                update(a0, alpha, 1.0),
            ),
            span,
        );
    }
    if hue.is_some() || has_sl {
        return cor166(
            m::de_hsl(
                nova_matiz,
                update(s0, saturation, 100.0),
                update(l0, lightness, 100.0),
                update(a0, alpha, 1.0),
                ColorFormat::Infer,
            ),
            span,
        );
    }
    if alpha.is_some() {
        return cor166(m::com_alpha(&c, update(a0, alpha, 1.0)), span);
    }
    Ok(a[0].clone())
}
