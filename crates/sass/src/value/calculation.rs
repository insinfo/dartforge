//! Cálculos do CSS (`calc()`, `min()`, `round()`, `sin()`…) — porte de
//! `lib/src/value/calculation.dart` do dart-sass 1.102.0.
//!
//! O grass 0.13 só conhecia `calc`, `min`, `max` e `clamp`, e os analisava
//! com uma gramática própria; o dart-sass 1.102 os analisa como chamadas de
//! função comuns e decide na avaliação (ver `visit_function_call_expr`), com
//! todas as funções de cálculo do CSS Values 4 e as simplificações abaixo.
use codemap::Span;

use crate::{
    common::BinaryOp,
    error::SassResult,
    serializer::inspect_number,
    unit::Unit,
    value::{conversion_factor, fuzzy_equals, fuzzy_less_than, SassNumber, Value},
    Options,
};

use super::Number;

/// Um argumento de cálculo: número, cálculo, texto sem aspas ou operação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalculationArg {
    Number(SassNumber),
    Calculation(SassCalculation),
    String(String),
    Operation {
        lhs: Box<Self>,
        op: BinaryOp,
        rhs: Box<Self>,
    },
}

impl CalculationArg {
    /// `_parenthesizeCalculationRhs` do serializador.
    pub fn parenthesize_calculation_rhs(outer: BinaryOp, right: BinaryOp) -> bool {
        if outer == BinaryOp::Div {
            true
        } else if outer == BinaryOp::Plus {
            false
        } else {
            right == BinaryOp::Plus || right == BinaryOp::Minus
        }
    }
}

/// Um cálculo (`SassCalculation`): o nome como o dart-sass o guarda e os
/// argumentos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SassCalculation {
    pub name: String,
    pub args: Vec<CalculationArg>,
}

fn err<T>(msg: impl Into<String>, span: Span) -> SassResult<T> {
    Err((msg.into(), span).into())
}

fn num(n: f64, unit: Unit) -> SassNumber {
    SassNumber {
        num: Number(n),
        unit,
        as_slash: None,
    }
}

fn inspecionar(n: &SassNumber, span: Span) -> String {
    inspect_number(n, &Options::default(), span).unwrap_or_default()
}

/// `hasCompatibleUnits` do dart-sass.
pub(crate) fn has_compatible_units(a: &SassNumber, b: &SassNumber) -> bool {
    a.has_compatible_units(&b.unit)
}

/// `convertValueToMatch`: o valor de `n` nas unidades de `alvo` (sem
/// unidade de um lado e com do outro: o valor como está).
fn convert_value_to_match(n: &SassNumber, alvo: &SassNumber) -> f64 {
    if n.unit == alvo.unit || n.unit == Unit::None || alvo.unit == Unit::None {
        return n.num.0;
    }
    n.num.convert(&n.unit, &alvo.unit).0
}

/// `_matchUnits`.
fn match_units(v: f64, n: &SassNumber) -> SassNumber {
    num(v, n.unit.clone())
}

/// `double.sign` do Dart.
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

/// `signIncludingZero`.
fn sign_including_zero(x: f64) -> f64 {
    if x == 0.0 {
        if x.is_sign_negative() {
            -1.0
        } else {
            1.0
        }
    } else {
        sign(x)
    }
}

/// `moduloLikeSass`.
pub(crate) fn modulo_like_sass(num1: f64, num2: f64) -> f64 {
    if num1.is_infinite() {
        return f64::NAN;
    }
    if num2.is_infinite() {
        return if sign_including_zero(num1) == sign(num2) {
            num1
        } else {
            f64::NAN
        };
    }
    if num2 > 0.0 {
        return crate::color::dart_mod(num1, num2);
    }
    if num2 == 0.0 {
        return f64::NAN;
    }
    let result = crate::color::dart_mod(num1, num2);
    if result == 0.0 {
        0.0
    } else {
        result + num2
    }
}

/// `SassNumber.modulo`: unidades como na soma.
fn modulo(a: &SassNumber, b: &SassNumber) -> SassNumber {
    let v = convert_value_to_match(b, a);
    let unit = if a.unit == Unit::None {
        b.unit.clone()
    } else {
        a.unit.clone()
    };
    num(modulo_like_sass(a.num.0, v), unit)
}

impl SassCalculation {
    pub fn unsimplified(name: impl Into<String>, args: Vec<CalculationArg>) -> Self {
        Self {
            name: name.into(),
            args,
        }
    }

    fn new(name: &str, args: Vec<CalculationArg>) -> Value {
        Value::Calculation(Self {
            name: name.to_owned(),
            args,
        })
    }

    pub fn calc(arg: CalculationArg) -> Value {
        match Self::simplify(arg) {
            CalculationArg::Number(n) => Value::Dimension(n),
            CalculationArg::Calculation(c) => Value::Calculation(c),
            other => Self::new("calc", vec![other]),
        }
    }

    pub fn min(args: Vec<CalculationArg>, span: Span) -> SassResult<Value> {
        Self::min_max(args, span, true)
    }

    pub fn max(args: Vec<CalculationArg>, span: Span) -> SassResult<Value> {
        Self::min_max(args, span, false)
    }

    fn min_max(args: Vec<CalculationArg>, span: Span, is_min: bool) -> SassResult<Value> {
        let args = Self::simplify_arguments(args);
        let mut melhor: Option<SassNumber> = None;
        for arg in &args {
            match arg {
                CalculationArg::Number(n) => {
                    if let Some(m) = &melhor {
                        if !m.is_comparable_to(n) {
                            melhor = None;
                            break;
                        }
                        let v = convert_value_to_match(n, m);
                        let troca = if is_min {
                            m.num.0 > v && !fuzzy_equals(m.num.0, v)
                        } else {
                            m.num.0 < v && !fuzzy_equals(m.num.0, v)
                        };
                        if troca {
                            melhor = Some(n.clone());
                        }
                    } else {
                        melhor = Some(n.clone());
                    }
                }
                _ => {
                    melhor = None;
                    break;
                }
            }
        }
        if let Some(m) = melhor {
            return Ok(Value::Dimension(m));
        }
        Self::verify_compatible_numbers(&args, span)?;
        Ok(Self::new(if is_min { "min" } else { "max" }, args))
    }

    pub fn hypot(args: Vec<CalculationArg>, span: Span) -> SassResult<Value> {
        let args = Self::simplify_arguments(args);
        Self::verify_compatible_numbers(&args, span)?;
        let first = match args.first() {
            Some(CalculationArg::Number(n)) if n.unit != Unit::Percent => n.clone(),
            _ => return Ok(Self::new("hypot", args)),
        };
        let mut subtotal = 0.0;
        for arg in &args {
            match arg {
                CalculationArg::Number(n) if has_compatible_units(n, &first) => {
                    let v = convert_value_to_match(n, &first);
                    subtotal += v * v;
                }
                _ => return Ok(Self::new("hypot", args)),
            }
        }
        Ok(Value::Dimension(num(subtotal.sqrt(), first.unit.clone())))
    }

    fn single(
        name: &str,
        arg: CalculationArg,
        forbid_units: bool,
        f: impl Fn(&SassNumber) -> SassResult<SassNumber>,
        span: Span,
    ) -> SassResult<Value> {
        match Self::simplify(arg) {
            CalculationArg::Number(n) => {
                if forbid_units {
                    n.assert_no_units("number", span)?;
                }
                Ok(Value::Dimension(f(&n)?))
            }
            other => Ok(Self::new(name, vec![other])),
        }
    }

    /// Valor em radianos (`coerceValueToUnit("rad")`).
    fn radianos(n: &SassNumber, span: Span) -> SassResult<f64> {
        if n.unit == Unit::None {
            return Ok(n.num.0);
        }
        match conversion_factor(&n.unit, &Unit::Rad) {
            Some(f) => Ok(n.num.0 * f),
            None => err(
                format!(
                    "$number: Expected {} to have an angle unit (deg, grad, rad, turn).",
                    inspecionar(n, span)
                ),
                span,
            ),
        }
    }

    fn graus(r: f64) -> SassNumber {
        num(r * (180.0 / std::f64::consts::PI), Unit::Deg)
    }

    pub fn single_function(name: &str, arg: CalculationArg, span: Span) -> SassResult<Value> {
        match name {
            "sqrt" => Self::single(
                name,
                arg,
                true,
                |n| Ok(num(n.num.0.sqrt(), Unit::None)),
                span,
            ),
            "sin" => Self::single(
                name,
                arg,
                false,
                |n| Ok(num(Self::radianos(n, span)?.sin(), Unit::None)),
                span,
            ),
            "cos" => Self::single(
                name,
                arg,
                false,
                |n| Ok(num(Self::radianos(n, span)?.cos(), Unit::None)),
                span,
            ),
            "tan" => Self::single(
                name,
                arg,
                false,
                |n| Ok(num(Self::radianos(n, span)?.tan(), Unit::None)),
                span,
            ),
            "atan" => Self::single(name, arg, true, |n| Ok(Self::graus(n.num.0.atan())), span),
            "asin" => Self::single(name, arg, true, |n| Ok(Self::graus(n.num.0.asin())), span),
            "acos" => Self::single(name, arg, true, |n| Ok(Self::graus(n.num.0.acos())), span),
            "abs" => match Self::simplify(arg) {
                CalculationArg::Number(n) => {
                    Ok(Value::Dimension(num(n.num.0.abs(), n.unit.clone())))
                }
                other => Ok(Self::new("abs", vec![other])),
            },
            "exp" => match Self::simplify(arg) {
                CalculationArg::Number(n) => {
                    n.assert_no_units("number", span)?;
                    Ok(Value::Dimension(num(
                        crate::color::dart_pow(std::f64::consts::E, n.num.0),
                        Unit::None,
                    )))
                }
                other => Ok(Self::new("exp", vec![other])),
            },
            "sign" => match Self::simplify(arg) {
                CalculationArg::Number(n) if n.num.0.is_nan() || n.num.0 == 0.0 => {
                    Ok(Value::Dimension(n))
                }
                CalculationArg::Number(n) if n.unit != Unit::Percent => {
                    Ok(Value::Dimension(num(sign(n.num.0), n.unit.clone())))
                }
                other => Ok(Self::new("sign", vec![other])),
            },
            _ => unreachable!("cálculo de um argumento {name}"),
        }
    }

    pub fn clamp(
        min: CalculationArg,
        value: Option<CalculationArg>,
        max: Option<CalculationArg>,
        span: Span,
    ) -> SassResult<Value> {
        let min = Self::simplify(min);
        let value = value.map(Self::simplify);
        let max = max.map(Self::simplify);
        if let (
            CalculationArg::Number(mi),
            Some(CalculationArg::Number(v)),
            Some(CalculationArg::Number(ma)),
        ) = (&min, &value, &max)
        {
            if has_compatible_units(mi, v) && has_compatible_units(mi, ma) {
                let vv = v.num.0;
                let vmin = convert_value_to_match(mi, v);
                let vmax = convert_value_to_match(ma, v);
                if vv < vmin || fuzzy_equals(vv, vmin) {
                    return Ok(Value::Dimension(mi.clone()));
                }
                if vv > vmax || fuzzy_equals(vv, vmax) {
                    return Ok(Value::Dimension(ma.clone()));
                }
                return Ok(Value::Dimension(v.clone()));
            }
        }
        let mut args = vec![min];
        args.extend(value);
        args.extend(max);
        Self::verify_compatible_numbers(&args, span)?;
        Self::verify_length(&args, 3, span)?;
        Ok(Self::new("clamp", args))
    }

    pub fn pow(
        base: CalculationArg,
        exponent: Option<CalculationArg>,
        span: Span,
    ) -> SassResult<Value> {
        let mut args = vec![base.clone()];
        args.extend(exponent.clone());
        Self::verify_length(&args, 2, span)?;
        let base = Self::simplify(base);
        let exponent = exponent.map(Self::simplify);
        match (base, exponent) {
            (CalculationArg::Number(b), Some(CalculationArg::Number(e))) => {
                b.assert_no_units("base", span)?;
                e.assert_no_units("exponent", span)?;
                Ok(Value::Dimension(num(
                    crate::color::dart_pow(b.num.0, e.num.0),
                    Unit::None,
                )))
            }
            _ => Ok(Self::new("pow", args)),
        }
    }

    pub fn log(
        number: CalculationArg,
        base: Option<CalculationArg>,
        span: Span,
    ) -> SassResult<Value> {
        let number = Self::simplify(number);
        let base = base.map(Self::simplify);
        let mut args = vec![number.clone()];
        args.extend(base.clone());
        let CalculationArg::Number(n) = &number else {
            return Ok(Self::new("log", args));
        };
        match &base {
            Some(CalculationArg::Number(b)) => {
                n.assert_no_units("number", span)?;
                b.assert_no_units("base", span)?;
                Ok(Value::Dimension(num(
                    n.num.0.ln() / b.num.0.ln(),
                    Unit::None,
                )))
            }
            Some(_) => Ok(Self::new("log", args)),
            None => {
                n.assert_no_units("number", span)?;
                Ok(Value::Dimension(num(n.num.0.ln(), Unit::None)))
            }
        }
    }

    pub fn atan2(y: CalculationArg, x: Option<CalculationArg>, span: Span) -> SassResult<Value> {
        let y = Self::simplify(y);
        let x = x.map(Self::simplify);
        let mut args = vec![y.clone()];
        args.extend(x.clone());
        Self::verify_length(&args, 2, span)?;
        Self::verify_compatible_numbers(&args, span)?;
        match (&y, &x) {
            (CalculationArg::Number(y), Some(CalculationArg::Number(x)))
                if y.unit != Unit::Percent
                    && x.unit != Unit::Percent
                    && has_compatible_units(y, x) =>
            {
                Ok(Value::Dimension(Self::graus(
                    y.num.0.atan2(convert_value_to_match(x, y)),
                )))
            }
            _ => Ok(Self::new("atan2", args)),
        }
    }

    pub fn rem(
        dividend: CalculationArg,
        modulus: Option<CalculationArg>,
        span: Span,
    ) -> SassResult<Value> {
        let dividend = Self::simplify(dividend);
        let modulus = modulus.map(Self::simplify);
        let mut args = vec![dividend.clone()];
        args.extend(modulus.clone());
        Self::verify_length(&args, 2, span)?;
        Self::verify_compatible_numbers(&args, span)?;
        match (&dividend, &modulus) {
            (CalculationArg::Number(d), Some(CalculationArg::Number(m)))
                if has_compatible_units(d, m) =>
            {
                let result = modulo(d, m);
                if sign_including_zero(m.num.0) != sign_including_zero(d.num.0) {
                    if m.num.0.is_infinite() {
                        return Ok(Value::Dimension(d.clone()));
                    }
                    if result.num.0 == 0.0 {
                        return Ok(Value::Dimension(num(-result.num.0, result.unit.clone())));
                    }
                    let mv = convert_value_to_match(m, &result);
                    return Ok(Value::Dimension(num(
                        result.num.0 - mv,
                        result.unit.clone(),
                    )));
                }
                Ok(Value::Dimension(result))
            }
            _ => Ok(Self::new("rem", args)),
        }
    }

    pub fn modulo(
        dividend: CalculationArg,
        modulus: Option<CalculationArg>,
        span: Span,
    ) -> SassResult<Value> {
        let dividend = Self::simplify(dividend);
        let modulus = modulus.map(Self::simplify);
        let mut args = vec![dividend.clone()];
        args.extend(modulus.clone());
        Self::verify_length(&args, 2, span)?;
        Self::verify_compatible_numbers(&args, span)?;
        match (&dividend, &modulus) {
            (CalculationArg::Number(d), Some(CalculationArg::Number(m)))
                if has_compatible_units(d, m) =>
            {
                Ok(Value::Dimension(modulo(d, m)))
            }
            _ => Ok(Self::new("mod", args)),
        }
    }

    fn eh_estrategia(a: &CalculationArg) -> Option<&str> {
        match a {
            CalculationArg::String(s)
                if matches!(s.as_str(), "nearest" | "up" | "down" | "to-zero") =>
            {
                Some(s)
            }
            _ => None,
        }
    }

    /// `roundInternal`.
    pub fn round(
        a: CalculationArg,
        b: Option<CalculationArg>,
        c: Option<CalculationArg>,
        in_legacy: bool,
        span: Span,
    ) -> SassResult<Value> {
        let a = Self::simplify(a);
        let b = b.map(Self::simplify);
        let c = c.map(Self::simplify);
        use CalculationArg as A;
        match (&a, &b, &c) {
            (A::Number(n), None, None) if n.unit == Unit::None => {
                Ok(Value::Dimension(num(n.num.0.round(), Unit::None)))
            }
            (A::Number(n), None, None) if in_legacy => {
                Ok(Value::Dimension(match_units(n.num.0.round(), n)))
            }
            (A::Number(n), Some(A::Number(step)), None) if !has_compatible_units(n, step) => {
                Self::verify_compatible_numbers(&[a.clone(), b.clone().unwrap()], span)?;
                Ok(Self::new("round", vec![a.clone(), b.clone().unwrap()]))
            }
            (A::Number(n), Some(A::Number(step)), None) => {
                Self::verify_compatible_numbers(&[a.clone(), b.clone().unwrap()], span)?;
                Ok(Value::Dimension(Self::round_with_step("nearest", n, step)))
            }
            (s, Some(A::Number(n)), Some(A::Number(step))) if Self::eh_estrategia(s).is_some() => {
                Self::verify_compatible_numbers(&[b.clone().unwrap(), c.clone().unwrap()], span)?;
                if !has_compatible_units(n, step) {
                    return Ok(Self::new(
                        "round",
                        vec![a.clone(), b.clone().unwrap(), c.clone().unwrap()],
                    ));
                }
                Ok(Value::Dimension(Self::round_with_step(
                    Self::eh_estrategia(s).unwrap(),
                    n,
                    step,
                )))
            }
            (s, Some(A::String(_)), None) if Self::eh_estrategia(s).is_some() => {
                Ok(Self::new("round", vec![a.clone(), b.clone().unwrap()]))
            }
            (s, Some(_), None) if Self::eh_estrategia(s).is_some() => {
                err("If strategy is not null, step is required.", span)
            }
            (s, None, None) if Self::eh_estrategia(s).is_some() => {
                err("Number to round and step arguments are required.", span)
            }
            (_, None, None) => Ok(Self::new("round", vec![a.clone()])),
            (_, Some(_), None) => Ok(Self::new("round", vec![a.clone(), b.clone().unwrap()])),
            (s, Some(_), Some(_))
                if Self::eh_estrategia(s).is_some()
                    || matches!(s, A::String(t) if is_special_variable_text(t)) =>
            {
                Ok(Self::new(
                    "round",
                    vec![a.clone(), b.clone().unwrap(), c.clone().unwrap()],
                ))
            }
            (_, Some(_), Some(_)) => err(
                format!(
                    "{} must be either nearest, up, down or to-zero.",
                    Self::texto_arg(&a, span)
                ),
                span,
            ),
            _ => err("Invalid parameters.", span),
        }
    }

    fn texto_arg(a: &CalculationArg, span: Span) -> String {
        match a {
            CalculationArg::String(s) => s.clone(),
            CalculationArg::Number(n) => inspecionar(n, span),
            other => Value::Calculation(SassCalculation::unsimplified("calc", vec![other.clone()]))
                .inspect(span)
                .unwrap_or_default(),
        }
    }

    fn round_with_step(strategy: &str, n: &SassNumber, step: &SassNumber) -> SassNumber {
        let (nv, sv) = (n.num.0, step.num.0);
        if (nv.is_infinite() && sv.is_infinite()) || sv == 0.0 || nv.is_nan() || sv.is_nan() {
            return match_units(f64::NAN, n);
        }
        if nv.is_infinite() {
            return n.clone();
        }
        if sv.is_infinite() {
            return match (strategy, nv) {
                (_, x) if x == 0.0 => n.clone(),
                ("nearest" | "to-zero", x) if x > 0.0 => match_units(0.0, n),
                ("nearest" | "to-zero", _) => match_units(-0.0, n),
                ("up", x) if x > 0.0 => match_units(f64::INFINITY, n),
                ("up", _) => match_units(-0.0, n),
                ("down", x) if x < 0.0 => match_units(f64::NEG_INFINITY, n),
                _ => match_units(0.0, n),
            };
        }
        let s = convert_value_to_match(step, n);
        let q = nv / s;
        match strategy {
            "nearest" => match_units(q.round() * s, n),
            "up" => match_units(if sv < 0.0 { q.floor() } else { q.ceil() } * s, n),
            "down" => match_units(if sv < 0.0 { q.ceil() } else { q.floor() } * s, n),
            "to-zero" => match_units(if nv < 0.0 { q.ceil() } else { q.floor() } * s, n),
            _ => match_units(f64::NAN, n),
        }
    }

    pub fn calc_size(
        basis: CalculationArg,
        value: Option<CalculationArg>,
        span: Span,
    ) -> SassResult<Value> {
        let mut args = vec![basis.clone()];
        args.extend(value.clone());
        Self::verify_length(&args, 2, span)?;
        let mut simp = vec![Self::simplify(basis)];
        simp.extend(value.map(Self::simplify));
        Ok(Self::new("calc-size", simp))
    }

    /// `operateInternal`.
    pub fn operate_internal(
        mut op: BinaryOp,
        left: CalculationArg,
        right: CalculationArg,
        in_legacy: bool,
        simplify: bool,
        span: Span,
    ) -> SassResult<CalculationArg> {
        if !simplify {
            return Ok(CalculationArg::Operation {
                lhs: Box::new(left),
                op,
                rhs: Box::new(right),
            });
        }
        let left = Self::simplify(left);
        let mut right = Self::simplify(right);
        if op == BinaryOp::Plus || op == BinaryOp::Minus {
            if let (CalculationArg::Number(l), CalculationArg::Number(r)) = (&left, &right) {
                let mut compatible = has_compatible_units(l, r);
                if !compatible && in_legacy && l.is_comparable_to(r) {
                    compatible = true;
                }
                if compatible {
                    return Ok(CalculationArg::Number(if op == BinaryOp::Plus {
                        l.clone() + r.clone()
                    } else {
                        l.clone() - r.clone()
                    }));
                }
            }
            Self::verify_compatible_numbers(&[left.clone(), right.clone()], span)?;
            if let CalculationArg::Number(r) = &right {
                if fuzzy_less_than(r.num.0, 0.0) {
                    right = CalculationArg::Number(num(-r.num.0, r.unit.clone()));
                    op = if op == BinaryOp::Plus {
                        BinaryOp::Minus
                    } else {
                        BinaryOp::Plus
                    };
                }
            }
            return Ok(CalculationArg::Operation {
                lhs: Box::new(left),
                op,
                rhs: Box::new(right),
            });
        }
        match (left, right) {
            (CalculationArg::Number(a), CalculationArg::Number(b)) => {
                Ok(CalculationArg::Number(if op == BinaryOp::Mul {
                    a * b
                } else {
                    a / b
                }))
            }
            (l, r) => Ok(CalculationArg::Operation {
                lhs: Box::new(l),
                op,
                rhs: Box::new(r),
            }),
        }
    }

    /// `_simplify`.
    pub(crate) fn simplify(arg: CalculationArg) -> CalculationArg {
        match arg {
            CalculationArg::Calculation(calc) if calc.name == "calc" && calc.args.len() == 1 => {
                match &calc.args[0] {
                    CalculationArg::String(text) if needs_parentheses(text) => {
                        CalculationArg::String(format!("({text})"))
                    }
                    v => v.clone(),
                }
            }
            other => other,
        }
    }

    fn simplify_arguments(args: Vec<CalculationArg>) -> Vec<CalculationArg> {
        args.into_iter().map(Self::simplify).collect()
    }

    fn verify_length(args: &[CalculationArg], len: usize, span: Span) -> SassResult<()> {
        if args.len() == len || args.iter().any(|a| matches!(a, CalculationArg::String(..))) {
            return Ok(());
        }
        err(
            format!(
                "{len} arguments required, but only {} {} passed.",
                args.len(),
                if args.len() == 1 { "was" } else { "were" }
            ),
            span,
        )
    }

    pub(crate) fn verify_compatible_numbers(args: &[CalculationArg], span: Span) -> SassResult<()> {
        for arg in args {
            if let CalculationArg::Number(n) = arg {
                if n.unit.is_complex() {
                    return err(
                        format!(
                            "Number {} isn't compatible with CSS calculations.",
                            inspecionar(n, span)
                        ),
                        span,
                    );
                }
            }
        }
        for i in 0..args.len() {
            let CalculationArg::Number(a) = &args[i] else {
                continue;
            };
            for b in &args[i + 1..] {
                let CalculationArg::Number(b) = b else {
                    continue;
                };
                if a.has_possibly_compatible_units(b) {
                    continue;
                }
                return err(
                    format!(
                        "{} and {} are incompatible.",
                        inspecionar(a, span),
                        inspecionar(b, span)
                    ),
                    span,
                );
            }
        }
        Ok(())
    }
}

/// `isSpecialVariable` de um texto sem aspas.
pub(crate) fn is_special_variable_text(t: &str) -> bool {
    let b = t.as_bytes();
    if b.len() < "var(_)".len() {
        return false;
    }
    let l = |i: usize| b[i].to_ascii_lowercase();
    match l(0) {
        b'a' => l(1) == b't' && l(2) == b't' && l(3) == b'r' && b[4] == b'(',
        b'i' => l(1) == b'f' && b[2] == b'(',
        b'v' => l(1) == b'a' && l(2) == b'r' && b[3] == b'(',
        _ => false,
    }
}

/// `_needsParentheses`.
fn needs_parentheses(text: &str) -> bool {
    let b = text.as_bytes();
    let precisa = |c: u8| matches!(c, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c' | b'/' | b'*');
    if b.is_empty() {
        return false;
    }
    let first = b[0];
    if precisa(first) {
        return true;
    }
    let mut could_be_var = b.len() >= 4 && first.eq_ignore_ascii_case(&b'v');
    if b.len() < 2 {
        return false;
    }
    if precisa(b[1]) {
        return true;
    }
    could_be_var = could_be_var && b[1].eq_ignore_ascii_case(&b'a');
    if b.len() < 3 {
        return false;
    }
    if precisa(b[2]) {
        return true;
    }
    could_be_var = could_be_var && b[2].eq_ignore_ascii_case(&b'r');
    if b.len() < 4 {
        return false;
    }
    if could_be_var && b[3] == b'(' {
        return true;
    }
    b[3..].iter().any(|&c| precisa(c))
}
