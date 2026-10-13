//! Constant values: analyzer 6.11's `DartObjectImpl` and `InstanceState`s
//! (`src/dart/constant/value.dart`), with the same operations and the same
//! evaluation exceptions (`EvaluationException`).
//!
//! A value has its runtime type (with extension types erased) and its state.
//! States with an unknown value (`UNKNOWN_VALUE`) are valid: evaluation goes
//! on without proving any error about them.

use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::{codigos::compile_time_error as c, Codigo};
use dartforge_elements::model::{ClassId, FunctionElementId, VariableId};
use dartforge_intern::SymbolId;
use std::rc::Rc;

/// Text of a Dart `String`: UTF-16 code units.
pub type Texto = Rc<[u16]>;

/// Key of a field of a generic object: the field name, or the pseudo-field
/// `(super)` with the superclass's object (`GenericState.SUPERCLASS_FIELD`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Campo {
    Nome(SymbolId),
    Super,
    /// `index` and `_name` of an enum constant (`updateEnumConstant`).
    Indice,
    EnumName,
}

/// The element of a function value (`FunctionState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Funcao {
    Elemento(FunctionElementId),
    /// The primary constructor of an extension type (without an element).
    Primary(ClassId),
}

/// `InstanceState`.
#[derive(Clone, Debug)]
pub enum Estado {
    Bool(Option<bool>),
    Int(Option<i64>),
    Double(Option<f64>),
    Str(Option<Texto>),
    /// `NullState`; `invalid` is the fictitious object of an unresolved
    /// expression (`_unresolvedObject`).
    Null { invalid: bool },
    Simbolo(Option<Rc<str>>),
    Tipo(Option<TypeId>),
    Funcao { elemento: Funcao, args: Option<Rc<[TypeId]>> },
    Lista { elemento: TypeId, elementos: Rc<Vec<Valor>>, unknown: bool },
    Conjunto { elementos: Rc<Vec<Valor>>, desconhecido: bool },
    Mapa { entradas: Rc<Vec<(Valor, Valor)>>, desconhecido: bool },
    Registro { posicionais: Rc<Vec<Valor>>, nomeados: Rc<Vec<(SymbolId, Valor)>> },
    Generico { campos: Rc<Vec<(Campo, Valor)>>, desconhecido: bool },
}

/// `DartObjectImpl`.
#[derive(Clone, Debug)]
pub struct Valor {
    pub tipo: TypeId,
    pub estado: Estado,
    /// The constant variable the value was associated with (`forVariable`).
    pub variavel: Option<VariableId>,
}

/// `EvaluationException`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Excecao {
    pub codigo: Codigo,
    pub runtime: bool,
}

impl Excecao {
    pub fn nova(code: Codigo) -> Self {
        Excecao { codigo: code, runtime: false }
    }
}

type R<T> = Result<T, Excecao>;

fn lanca<T>(code: Codigo) -> R<T> {
    Err(Excecao::nova(code))
}

impl Estado {
    pub fn desconhecido(&self) -> bool {
        match self {
            Estado::Bool(v) => v.is_none(),
            Estado::Int(v) => v.is_none(),
            Estado::Double(v) => v.is_none(),
            Estado::Str(v) => v.is_none(),
            Estado::Lista { unknown, .. } => *unknown,
            Estado::Conjunto { desconhecido: the_unknown, .. } | Estado::Mapa { desconhecido: the_unknown, .. } => *the_unknown,
            Estado::Generico { desconhecido: the_unknown, .. } => *the_unknown,
            _ => false,
        }
    }

    pub fn e_bool(&self) -> bool {
        matches!(self, Estado::Bool(_))
    }

    pub fn e_int(&self) -> bool {
        matches!(self, Estado::Int(_))
    }

    fn e_num(&self) -> bool {
        matches!(self, Estado::Int(_) | Estado::Double(_))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Estado::Null { .. })
    }

    pub fn is_bool_num_string_or_null(&self) -> bool {
        matches!(self, Estado::Bool(_) | Estado::Int(_) | Estado::Double(_) | Estado::Str(_) | Estado::Null { .. })
    }

    fn assert_bool(s: &Estado) -> R<()> {
        if s.e_bool() { Ok(()) } else { lanca(c::CONST_EVAL_TYPE_BOOL) }
    }

    fn assert_int_or_null(s: &Estado) -> R<()> {
        if matches!(s, Estado::Int(_) | Estado::Null { .. }) { Ok(()) } else { lanca(c::CONST_EVAL_TYPE_INT) }
    }

    fn assert_num_or_null(s: &Estado) -> R<()> {
        if s.e_num() || s.is_null() { Ok(()) } else { lanca(c::CONST_EVAL_TYPE_NUM) }
    }

    fn assert_num_string_or_null(s: &Estado) -> R<()> {
        if s.e_num() || matches!(s, Estado::Str(_)) || s.is_null() {
            Ok(())
        } else {
            lanca(c::CONST_EVAL_TYPE_NUM_STRING)
        }
    }

    fn assert_string(s: &Estado) -> R<()> {
        if matches!(s, Estado::Str(_)) { Ok(()) } else { lanca(c::CONST_EVAL_TYPE_STRING) }
    }

    /// `convertToBool`.
    pub fn para_bool(&self) -> R<Estado> {
        match self {
            Estado::Bool(v) => Ok(Estado::Bool(*v)),
            Estado::Null { .. } => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
            _ => Ok(Estado::Bool(Some(false))),
        }
    }

    /// `convertToString`.
    pub fn para_texto(&self, format_type: &dyn Fn(TypeId) -> String, function_name: &dyn Fn(Funcao) -> String) -> R<Estado> {
        Ok(match self {
            Estado::Bool(v) => Estado::Str(v.map(|b| texto(if b { "true" } else { "false" }))),
            Estado::Int(v) => Estado::Str(v.map(|i| texto(&i.to_string()))),
            Estado::Double(v) => Estado::Str(v.map(|d| texto(&double_as_dart(d)))),
            Estado::Str(v) => Estado::Str(v.clone()),
            Estado::Null { .. } => Estado::Str(Some(texto("null"))),
            Estado::Simbolo(v) => Estado::Str(v.as_ref().map(|s| texto(s))),
            Estado::Tipo(t) => Estado::Str(t.map(|t| texto(&format_type(t)))),
            Estado::Funcao { elemento: element, .. } => Estado::Str(Some(texto(&function_name(*element)))),
            Estado::Lista { .. } | Estado::Conjunto { .. } | Estado::Mapa { .. } | Estado::Registro { .. } | Estado::Generico { .. } => {
                Estado::Str(None)
            }
        })
    }

    /// `add`.
    pub fn somar(&self, d: &Estado) -> R<Estado> {
        match self {
            Estado::Int(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = v else {
                    return Ok(if matches!(d, Estado::Double(_)) { Estado::Double(None) } else { Estado::Int(None) });
                };
                match d {
                    Estado::Int(b) => Ok(Estado::Int(b.map(|b| a.wrapping_add(b)))),
                    Estado::Double(b) => Ok(Estado::Double(b.map(|b| *a as f64 + b))),
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            Estado::Double(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = v else { return Ok(Estado::Double(None)) };
                match d {
                    Estado::Int(b) => Ok(Estado::Double(b.map(|b| a + b as f64))),
                    Estado::Double(b) => Ok(Estado::Double(b.map(|b| a + b))),
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            Estado::Str(_) if matches!(d, Estado::Str(_)) => self.concatenar(d),
            _ => {
                Self::assert_num_string_or_null(self)?;
                Self::assert_num_string_or_null(d)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `concatenate`.
    pub fn concatenar(&self, d: &Estado) -> R<Estado> {
        if let Estado::Str(v) = self {
            let Some(a) = v else { return Ok(Estado::Str(None)) };
            if let Estado::Str(b) = d {
                let Some(b) = b else { return Ok(Estado::Str(None)) };
                let mut u: Vec<u16> = a.to_vec();
                u.extend_from_slice(b);
                return Ok(Estado::Str(Some(u.into())));
            }
        }
        Self::assert_string(d)?;
        lanca(c::CONST_EVAL_THROWS_EXCEPTION)
    }

    fn arithmetic(
        &self,
        d: &Estado,
        int: impl Fn(i64, i64) -> Option<R<Estado>>,
        double: impl Fn(f64, f64) -> R<Estado>,
        unknown_int_with_double: Estado,
    ) -> R<Estado> {
        match self {
            Estado::Int(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = *v else {
                    return Ok(if matches!(d, Estado::Double(_)) { unknown_int_with_double } else { Estado::Int(None) });
                };
                match d {
                    Estado::Int(b) => match b {
                        None => Ok(Estado::Int(None)),
                        Some(b) => int(a, *b).unwrap_or_else(|| lanca(c::CONST_EVAL_THROWS_EXCEPTION)),
                    },
                    Estado::Double(b) => match b {
                        None => Ok(Estado::Double(None)),
                        Some(b) => double(a as f64, *b),
                    },
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            Estado::Double(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = *v else { return Ok(Estado::Double(None)) };
                match d {
                    Estado::Int(b) => match b {
                        None => Ok(Estado::Double(None)),
                        Some(b) => double(a, *b as f64),
                    },
                    Estado::Double(b) => match b {
                        None => Ok(Estado::Double(None)),
                        Some(b) => double(a, *b),
                    },
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            _ => {
                Self::assert_num_or_null(self)?;
                Self::assert_num_or_null(d)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `minus`.
    pub fn subtract(&self, d: &Estado) -> R<Estado> {
        self.arithmetic(d, |a, b| Some(Ok(Estado::Int(Some(a.wrapping_sub(b))))), |a, b| Ok(Estado::Double(Some(a - b))), Estado::Double(None))
    }

    /// `times`.
    pub fn multiply(&self, d: &Estado) -> R<Estado> {
        self.arithmetic(d, |a, b| Some(Ok(Estado::Int(Some(a.wrapping_mul(b))))), |a, b| Ok(Estado::Double(Some(a * b))), Estado::Double(None))
    }

    /// `divide`: always `double` (the unknown `int` too).
    pub fn divide(&self, d: &Estado) -> R<Estado> {
        if let Estado::Int(None) = self {
            Self::assert_num_or_null(d)?;
            return Ok(Estado::Double(None));
        }
        if let (Estado::Int(Some(_)), Estado::Int(None)) = (self, d) {
            return Ok(Estado::Double(None));
        }
        self.arithmetic(
            d,
            |a, b| Some(Ok(Estado::Double(Some(a as f64 / b as f64)))),
            |a, b| Ok(Estado::Double(Some(a / b))),
            Estado::Double(None),
        )
    }

    /// `remainder` (Dart's `%`: Euclidean remainder, never negative).
    pub fn resto(&self, d: &Estado) -> R<Estado> {
        self.arithmetic(
            d,
            |a, b| (b != 0).then(|| Ok(Estado::Int(Some(a.wrapping_rem_euclid(b))))),
            |a, b| Ok(Estado::Double(Some(double_remainder(a, b)))),
            Estado::Double(None),
        )
    }

    /// `integerDivide` (`~/`).
    pub fn integer_divide(&self, d: &Estado) -> R<Estado> {
        match self {
            Estado::Int(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = *v else { return Ok(Estado::Int(None)) };
                match d {
                    Estado::Int(None) | Estado::Double(None) => Ok(Estado::Int(None)),
                    Estado::Int(Some(0)) => Err(Excecao { codigo: c::CONST_EVAL_THROWS_IDBZE, runtime: true }),
                    Estado::Int(Some(b)) => Ok(Estado::Int(Some(a.wrapping_div(*b)))),
                    Estado::Double(Some(b)) => {
                        let r = a as f64 / b;
                        if r.is_finite() { Ok(Estado::Int(Some(r as i64))) } else { lanca(c::CONST_EVAL_THROWS_EXCEPTION) }
                    }
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            Estado::Double(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = *v else { return Ok(Estado::Int(None)) };
                let b = match d {
                    Estado::Int(None) | Estado::Double(None) => return Ok(Estado::Int(None)),
                    Estado::Int(Some(b)) => *b as f64,
                    Estado::Double(Some(b)) => *b,
                    _ => return lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                };
                let r = a / b;
                if r.is_finite() { Ok(Estado::Int(Some(r as i64))) } else { lanca(c::CONST_EVAL_THROWS_EXCEPTION) }
            }
            _ => {
                Self::assert_num_or_null(self)?;
                Self::assert_num_or_null(d)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    fn compare(&self, d: &Estado, f: impl Fn(std::cmp::Ordering) -> bool) -> R<Estado> {
        let value = |s: &Estado| -> Option<Option<f64>> {
            match s {
                Estado::Int(v) => Some(v.map(|i| i as f64)),
                Estado::Double(v) => Some(*v),
                _ => None,
            }
        };
        match (self, d) {
            (Estado::Int(Some(a)), Estado::Int(Some(b))) => Ok(Estado::Bool(Some(f(a.cmp(b))))),
            _ => {
                let Some(a) = value(self) else {
                    Self::assert_num_or_null(self)?;
                    Self::assert_num_or_null(d)?;
                    return lanca(c::CONST_EVAL_THROWS_EXCEPTION);
                };
                Self::assert_num_or_null(d)?;
                let Some(a) = a else { return Ok(Estado::Bool(None)) };
                let Some(b) = value(d) else { return lanca(c::CONST_EVAL_THROWS_EXCEPTION) };
                let Some(b) = b else { return Ok(Estado::Bool(None)) };
                Ok(Estado::Bool(Some(match a.partial_cmp(&b) {
                    Some(o) => f(o),
                    None => false,
                })))
            }
        }
    }

    pub fn greater(&self, d: &Estado) -> R<Estado> {
        self.compare(d, |o| o == std::cmp::Ordering::Greater)
    }
    pub fn greater_or_equal(&self, d: &Estado) -> R<Estado> {
        self.compare(d, |o| o != std::cmp::Ordering::Less)
    }
    pub fn less(&self, d: &Estado) -> R<Estado> {
        self.compare(d, |o| o == std::cmp::Ordering::Less)
    }
    pub fn less_or_equal(&self, d: &Estado) -> R<Estado> {
        self.compare(d, |o| o != std::cmp::Ordering::Greater)
    }

    fn bits(&self, d: &Estado, f: impl Fn(i64, i64) -> Option<Estado>) -> R<Estado> {
        match self {
            Estado::Int(v) => {
                Self::assert_int_or_null(d)?;
                let Some(a) = *v else { return Ok(Estado::Int(None)) };
                match d {
                    Estado::Int(None) => Ok(Estado::Int(None)),
                    Estado::Int(Some(b)) => f(a, *b).map_or_else(|| lanca(c::CONST_EVAL_THROWS_EXCEPTION), Ok),
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            _ => {
                Self::assert_int_or_null(self)?;
                Self::assert_int_or_null(d)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    pub fn e_bit(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| Some(Estado::Int(Some(a & b))))
    }
    pub fn bit_or(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| Some(Estado::Int(Some(a | b))))
    }
    pub fn bit_xor(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| Some(Estado::Int(Some(a ^ b))))
    }
    /// `<<`: a shift wider than 31 bits is unknown.
    pub fn shift_left(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| {
            if bit_length(b) > 31 {
                Some(Estado::Int(None))
            } else if b >= 0 {
                Some(Estado::Int(Some(if b >= 64 { 0 } else { a.wrapping_shl(b as u32) })))
            } else {
                None
            }
        })
    }
    pub fn shift_right(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| {
            if bit_length(b) > 31 {
                Some(Estado::Int(None))
            } else if b >= 0 {
                Some(Estado::Int(Some(if b >= 64 { if a < 0 { -1 } else { 0 } } else { a >> b })))
            } else {
                None
            }
        })
    }
    pub fn logical_shift_right(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| {
            if b >= 64 {
                Some(Estado::Int(Some(0)))
            } else if b >= 0 {
                Some(Estado::Int(Some(((a as u64) >> b) as i64)))
            } else {
                None
            }
        })
    }

    /// `bitNot`.
    pub fn negate_bits(&self) -> R<Estado> {
        match self {
            Estado::Int(v) => Ok(Estado::Int(v.map(|i| !i))),
            _ => {
                Self::assert_int_or_null(self)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `negated`.
    pub fn negate(&self) -> R<Estado> {
        match self {
            Estado::Int(v) => Ok(Estado::Int(v.map(|i| i.wrapping_neg()))),
            Estado::Double(v) => Ok(Estado::Double(v.map(|d| -d))),
            _ => {
                Self::assert_num_or_null(self)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `logicalNot`.
    pub fn logical_not(&self) -> R<Estado> {
        match self {
            Estado::Bool(v) => Ok(Estado::Bool(v.map(|b| !b))),
            Estado::Null { .. } => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
            _ => {
                Self::assert_bool(self)?;
                Ok(Estado::Bool(Some(true)))
            }
        }
    }

    fn logical(&self, d: &Estado, f: impl Fn(bool, bool) -> bool) -> R<Estado> {
        Self::assert_bool(self)?;
        Self::assert_bool(d)?;
        let (Estado::Bool(a), Estado::Bool(b)) = (self.para_bool()?, d.para_bool()?) else { unreachable!() };
        Ok(Estado::Bool(match (a, b) {
            (Some(a), Some(b)) => Some(f(a, b)),
            _ => None,
        }))
    }
    pub fn logical_and(&self, d: &Estado) -> R<Estado> {
        self.logical(d, |a, b| a & b)
    }
    pub fn logical_or(&self, d: &Estado) -> R<Estado> {
        self.logical(d, |a, b| a | b)
    }
    pub fn logical_xor(&self, d: &Estado) -> R<Estado> {
        self.logical(d, |a, b| a ^ b)
    }

    /// `stringLength` (in UTF-16 code units).
    pub fn comprimento(&self) -> R<Estado> {
        match self {
            Estado::Str(v) => Ok(Estado::Int(v.as_ref().map(|s| s.len() as i64))),
            _ => {
                Self::assert_string(self)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }
}

/// Dart's `bitLength`: bits without the sign (`-1` has 0).
fn bit_length(v: i64) -> u32 {
    let x = if v < 0 { !v } else { v };
    64 - x.leading_zeros()
}

fn double_remainder(a: f64, b: f64) -> f64 {
    let r = a % b;
    if r < 0.0 { r + b.abs() } else { r }
}

/// The text of a `String` from a `&str`.
pub fn texto(s: &str) -> Texto {
    s.encode_utf16().collect::<Vec<u16>>().into()
}

/// Dart's `double.toString()`: `1.0`, `0.1`, `1e+21`, `1e-7`, `NaN`,
/// `Infinity`.
pub fn double_as_dart(d: f64) -> String {
    if d.is_nan() {
        return "NaN".into();
    }
    if d.is_infinite() {
        return if d > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if d == 0.0 {
        return if d.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let abs = d.abs();
    if (1e-6..1e21).contains(&abs) {
        let s = format!("{d}");
        if s.contains('.') { s } else { format!("{s}.0") }
    } else {
        let s = format!("{d:e}");
        match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        }
    }
}

impl Valor {
    /// The type comes already erased (`extensionTypeErasure`, done by the
    /// engine).
    pub fn novo(ty: TypeId, state: Estado) -> Valor {
        Valor { tipo: ty, estado: state, variavel: None }
    }

    pub fn nulo(core: &CoreTypes) -> Valor {
        Valor { tipo: core.null, estado: Estado::Null { invalid: false }, variavel: None }
    }

    pub fn bool_(core: &CoreTypes, v: Option<bool>) -> Valor {
        Valor { tipo: core.bool_, estado: Estado::Bool(v), variavel: None }
    }

    /// `validWithUnknownValue` (the type already erased).
    pub fn desconhecido(table: &TypeTable, core: &CoreTypes, ty: TypeId, list_element: Option<TypeId>) -> Valor {
        let cls = match table.get(ty) {
            Type::Interface { class, nullable: false, .. } => Some(*class),
            _ => None,
        };
        // An unknown of a nullable type (`bool?`, `int?`) has the state of the
        // base type: on the safe side, no type error is proven about it
        // (`c ? 1 : 2` with `c` an unknown of type `bool?` is not
        // `CONST_EVAL_TYPE_BOOL`).
        let base = match table.get(ty) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let of = |k: Option<ClassId>| base.is_some() && base == k;
        let state = if ty == core.bool_ || of(core.bool_class) {
            Estado::Bool(None)
        } else if ty == core.double || of(core.double_class) {
            Estado::Double(None)
        } else if ty == core.int || of(core.int_class) {
            Estado::Int(None)
        } else if ty == core.string || of(core.string_class) {
            Estado::Str(None)
        } else if cls.is_some() && cls == core.list_class {
            Estado::Lista { elemento: list_element.unwrap_or(core.dynamic_), elementos: Rc::new(Vec::new()), unknown: true }
        } else if cls.is_some() && cls == core.map_class {
            Estado::Mapa { entradas: Rc::new(Vec::new()), desconhecido: true }
        } else if cls.is_some() && cls == core.set_class {
            Estado::Conjunto { elementos: Rc::new(Vec::new()), desconhecido: true }
        } else {
            Estado::Generico { campos: Rc::new(Vec::new()), desconhecido: true }
        };
        Valor { tipo: ty, estado: state, variavel: None }
    }

    pub fn actually_unknown(&self) -> bool {
        self.estado.desconhecido()
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.estado {
            Estado::Bool(v) => v,
            _ => None,
        }
    }

    pub fn como_texto(&self) -> Option<&Texto> {
        match &self.estado {
            Estado::Str(Some(s)) => Some(s),
            _ => None,
        }
    }

    pub fn as_type(&self) -> Option<TypeId> {
        match self.estado {
            Estado::Tipo(t) => t,
            _ => None,
        }
    }

    /// `toListValue`.
    pub fn as_list(&self) -> Option<&Rc<Vec<Valor>>> {
        match &self.estado {
            Estado::Lista { elementos: elements, .. } => Some(elements),
            _ => None,
        }
    }

    /// `toSetValue`.
    pub fn as_set(&self) -> Option<&Rc<Vec<Valor>>> {
        match &self.estado {
            Estado::Conjunto { elementos: elements, .. } => Some(elements),
            _ => None,
        }
    }

    /// `toMapValue`.
    pub fn as_map(&self) -> Option<&Rc<Vec<(Valor, Valor)>>> {
        match &self.estado {
            Estado::Mapa { entradas: entries, .. } => Some(entries),
            _ => None,
        }
    }

    pub fn as_function(&self) -> Option<Funcao> {
        match self.estado {
            Estado::Funcao { elemento: element, .. } => Some(element),
            _ => None,
        }
    }
}
