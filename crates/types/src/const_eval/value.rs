//! Constant values: analyzer 6.11's `DartObjectImpl` and `InstanceState`s
//! (`src/dart/constant/value.dart`), with the same operations and the same
//! evaluation exceptions (`EvaluationException`).
//!
//! A value has its runtime type (with extension types erased) and its state.
//! States with an unknown value (`UNKNOWN_VALUE`) are valid: evaluation goes
//! on without proving any error about them.

use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::{codes::compile_time_error as c, Code};
use dartforge_elements::model::{ClassId, FunctionElementId, VariableId};
use dartforge_intern::SymbolId;
use std::rc::Rc;

/// Text of a Dart `String`: UTF-16 code units.
pub type Text = Rc<[u16]>;

/// Key of a field of a generic object: the field name, or the pseudo-field
/// `(super)` with the superclass's object (`GenericState.SUPERCLASS_FIELD`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    MemberName(SymbolId),
    Super,
    /// `index` and `_name` of an enum constant (`updateEnumConstant`).
    Index,
    EnumName,
}

/// The element of a function value (`FunctionState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Function {
    Element(FunctionElementId),
    /// The primary constructor of an extension type (without an element).
    Primary(ClassId),
}

/// `InstanceState`.
#[derive(Clone, Debug)]
pub enum InstanceState {
    Bool(Option<bool>),
    Int(Option<i64>),
    Double(Option<f64>),
    Str(Option<Text>),
    /// `NullState`; `invalid` is the fictitious object of an unresolved
    /// expression (`_unresolvedObject`).
    Null { invalid: bool },
    Symbol(Option<Rc<str>>),
    Type(Option<TypeId>),
    Function { element: Function, args: Option<Rc<[TypeId]>> },
    List { element: TypeId, elements: Rc<Vec<Value>>, unknown: bool },
    Set { elements: Rc<Vec<Value>>, unknown: bool },
    Map { entries: Rc<Vec<(Value, Value)>>, unknown: bool },
    Record { positional: Rc<Vec<Value>>, named: Rc<Vec<(SymbolId, Value)>> },
    Generic { fields: Rc<Vec<(Field, Value)>>, unknown: bool },
}

/// `DartObjectImpl`.
#[derive(Clone, Debug)]
pub struct Value {
    pub ty: TypeId,
    pub state: InstanceState,
    /// The constant variable the value was associated with (`forVariable`).
    pub variable: Option<VariableId>,
}

/// `EvaluationException`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exception {
    pub code: Code,
    pub runtime: bool,
}

impl Exception {
    pub fn new(code: Code) -> Self {
        Exception { code, runtime: false }
    }
}

type R<T> = Result<T, Exception>;

fn throws<T>(code: Code) -> R<T> {
    Err(Exception::new(code))
}

impl InstanceState {
    pub fn unknown(&self) -> bool {
        match self {
            InstanceState::Bool(v) => v.is_none(),
            InstanceState::Int(v) => v.is_none(),
            InstanceState::Double(v) => v.is_none(),
            InstanceState::Str(v) => v.is_none(),
            InstanceState::List { unknown, .. } => *unknown,
            InstanceState::Set { unknown: the_unknown, .. } | InstanceState::Map { unknown: the_unknown, .. } => *the_unknown,
            InstanceState::Generic { unknown: the_unknown, .. } => *the_unknown,
            _ => false,
        }
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, InstanceState::Bool(_))
    }

    pub fn is_int(&self) -> bool {
        matches!(self, InstanceState::Int(_))
    }

    fn is_num(&self) -> bool {
        matches!(self, InstanceState::Int(_) | InstanceState::Double(_))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, InstanceState::Null { .. })
    }

    pub fn is_bool_num_string_or_null(&self) -> bool {
        matches!(self, InstanceState::Bool(_) | InstanceState::Int(_) | InstanceState::Double(_) | InstanceState::Str(_) | InstanceState::Null { .. })
    }

    fn assert_bool(s: &InstanceState) -> R<()> {
        if s.is_bool() { Ok(()) } else { throws(c::CONST_EVAL_TYPE_BOOL) }
    }

    fn assert_int_or_null(s: &InstanceState) -> R<()> {
        if matches!(s, InstanceState::Int(_) | InstanceState::Null { .. }) { Ok(()) } else { throws(c::CONST_EVAL_TYPE_INT) }
    }

    fn assert_num_or_null(s: &InstanceState) -> R<()> {
        if s.is_num() || s.is_null() { Ok(()) } else { throws(c::CONST_EVAL_TYPE_NUM) }
    }

    fn assert_num_string_or_null(s: &InstanceState) -> R<()> {
        if s.is_num() || matches!(s, InstanceState::Str(_)) || s.is_null() {
            Ok(())
        } else {
            throws(c::CONST_EVAL_TYPE_NUM_STRING)
        }
    }

    fn assert_string(s: &InstanceState) -> R<()> {
        if matches!(s, InstanceState::Str(_)) { Ok(()) } else { throws(c::CONST_EVAL_TYPE_STRING) }
    }

    /// `convertToBool`.
    pub fn to_bool(&self) -> R<InstanceState> {
        match self {
            InstanceState::Bool(v) => Ok(InstanceState::Bool(*v)),
            InstanceState::Null { .. } => throws(c::CONST_EVAL_THROWS_EXCEPTION),
            _ => Ok(InstanceState::Bool(Some(false))),
        }
    }

    /// `convertToString`.
    pub fn to_text(&self, format_type: &dyn Fn(TypeId) -> String, function_name: &dyn Fn(Function) -> String) -> R<InstanceState> {
        Ok(match self {
            InstanceState::Bool(v) => InstanceState::Str(v.map(|b| the_text(if b { "true" } else { "false" }))),
            InstanceState::Int(v) => InstanceState::Str(v.map(|i| the_text(&i.to_string()))),
            InstanceState::Double(v) => InstanceState::Str(v.map(|d| the_text(&double_as_dart(d)))),
            InstanceState::Str(v) => InstanceState::Str(v.clone()),
            InstanceState::Null { .. } => InstanceState::Str(Some(the_text("null"))),
            InstanceState::Symbol(v) => InstanceState::Str(v.as_ref().map(|s| the_text(s))),
            InstanceState::Type(t) => InstanceState::Str(t.map(|t| the_text(&format_type(t)))),
            InstanceState::Function { element, .. } => InstanceState::Str(Some(the_text(&function_name(*element)))),
            InstanceState::List { .. } | InstanceState::Set { .. } | InstanceState::Map { .. } | InstanceState::Record { .. } | InstanceState::Generic { .. } => {
                InstanceState::Str(None)
            }
        })
    }

    /// `add`.
    pub fn add(&self, d: &InstanceState) -> R<InstanceState> {
        match self {
            InstanceState::Int(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = v else {
                    return Ok(if matches!(d, InstanceState::Double(_)) { InstanceState::Double(None) } else { InstanceState::Int(None) });
                };
                match d {
                    InstanceState::Int(b) => Ok(InstanceState::Int(b.map(|b| a.wrapping_add(b)))),
                    InstanceState::Double(b) => Ok(InstanceState::Double(b.map(|b| *a as f64 + b))),
                    _ => throws(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            InstanceState::Double(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = v else { return Ok(InstanceState::Double(None)) };
                match d {
                    InstanceState::Int(b) => Ok(InstanceState::Double(b.map(|b| a + b as f64))),
                    InstanceState::Double(b) => Ok(InstanceState::Double(b.map(|b| a + b))),
                    _ => throws(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            InstanceState::Str(_) if matches!(d, InstanceState::Str(_)) => self.concatenate(d),
            _ => {
                Self::assert_num_string_or_null(self)?;
                Self::assert_num_string_or_null(d)?;
                throws(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `concatenate`.
    pub fn concatenate(&self, d: &InstanceState) -> R<InstanceState> {
        if let InstanceState::Str(v) = self {
            let Some(a) = v else { return Ok(InstanceState::Str(None)) };
            if let InstanceState::Str(b) = d {
                let Some(b) = b else { return Ok(InstanceState::Str(None)) };
                let mut u: Vec<u16> = a.to_vec();
                u.extend_from_slice(b);
                return Ok(InstanceState::Str(Some(u.into())));
            }
        }
        Self::assert_string(d)?;
        throws(c::CONST_EVAL_THROWS_EXCEPTION)
    }

    fn arithmetic(
        &self,
        d: &InstanceState,
        int: impl Fn(i64, i64) -> Option<R<InstanceState>>,
        double: impl Fn(f64, f64) -> R<InstanceState>,
        unknown_int_with_double: InstanceState,
    ) -> R<InstanceState> {
        match self {
            InstanceState::Int(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = *v else {
                    return Ok(if matches!(d, InstanceState::Double(_)) { unknown_int_with_double } else { InstanceState::Int(None) });
                };
                match d {
                    InstanceState::Int(b) => match b {
                        None => Ok(InstanceState::Int(None)),
                        Some(b) => int(a, *b).unwrap_or_else(|| throws(c::CONST_EVAL_THROWS_EXCEPTION)),
                    },
                    InstanceState::Double(b) => match b {
                        None => Ok(InstanceState::Double(None)),
                        Some(b) => double(a as f64, *b),
                    },
                    _ => throws(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            InstanceState::Double(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = *v else { return Ok(InstanceState::Double(None)) };
                match d {
                    InstanceState::Int(b) => match b {
                        None => Ok(InstanceState::Double(None)),
                        Some(b) => double(a, *b as f64),
                    },
                    InstanceState::Double(b) => match b {
                        None => Ok(InstanceState::Double(None)),
                        Some(b) => double(a, *b),
                    },
                    _ => throws(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            _ => {
                Self::assert_num_or_null(self)?;
                Self::assert_num_or_null(d)?;
                throws(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `minus`.
    pub fn subtract(&self, d: &InstanceState) -> R<InstanceState> {
        self.arithmetic(d, |a, b| Some(Ok(InstanceState::Int(Some(a.wrapping_sub(b))))), |a, b| Ok(InstanceState::Double(Some(a - b))), InstanceState::Double(None))
    }

    /// `times`.
    pub fn multiply(&self, d: &InstanceState) -> R<InstanceState> {
        self.arithmetic(d, |a, b| Some(Ok(InstanceState::Int(Some(a.wrapping_mul(b))))), |a, b| Ok(InstanceState::Double(Some(a * b))), InstanceState::Double(None))
    }

    /// `divide`: always `double` (the unknown `int` too).
    pub fn divide(&self, d: &InstanceState) -> R<InstanceState> {
        if let InstanceState::Int(None) = self {
            Self::assert_num_or_null(d)?;
            return Ok(InstanceState::Double(None));
        }
        if let (InstanceState::Int(Some(_)), InstanceState::Int(None)) = (self, d) {
            return Ok(InstanceState::Double(None));
        }
        self.arithmetic(
            d,
            |a, b| Some(Ok(InstanceState::Double(Some(a as f64 / b as f64)))),
            |a, b| Ok(InstanceState::Double(Some(a / b))),
            InstanceState::Double(None),
        )
    }

    /// `remainder` (Dart's `%`: Euclidean remainder, never negative).
    pub fn rest(&self, d: &InstanceState) -> R<InstanceState> {
        self.arithmetic(
            d,
            |a, b| (b != 0).then(|| Ok(InstanceState::Int(Some(a.wrapping_rem_euclid(b))))),
            |a, b| Ok(InstanceState::Double(Some(double_remainder(a, b)))),
            InstanceState::Double(None),
        )
    }

    /// `integerDivide` (`~/`).
    pub fn integer_divide(&self, d: &InstanceState) -> R<InstanceState> {
        match self {
            InstanceState::Int(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = *v else { return Ok(InstanceState::Int(None)) };
                match d {
                    InstanceState::Int(None) | InstanceState::Double(None) => Ok(InstanceState::Int(None)),
                    InstanceState::Int(Some(0)) => Err(Exception { code: c::CONST_EVAL_THROWS_IDBZE, runtime: true }),
                    InstanceState::Int(Some(b)) => Ok(InstanceState::Int(Some(a.wrapping_div(*b)))),
                    InstanceState::Double(Some(b)) => {
                        let r = a as f64 / b;
                        if r.is_finite() { Ok(InstanceState::Int(Some(r as i64))) } else { throws(c::CONST_EVAL_THROWS_EXCEPTION) }
                    }
                    _ => throws(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            InstanceState::Double(v) => {
                Self::assert_num_or_null(d)?;
                let Some(a) = *v else { return Ok(InstanceState::Int(None)) };
                let b = match d {
                    InstanceState::Int(None) | InstanceState::Double(None) => return Ok(InstanceState::Int(None)),
                    InstanceState::Int(Some(b)) => *b as f64,
                    InstanceState::Double(Some(b)) => *b,
                    _ => return throws(c::CONST_EVAL_THROWS_EXCEPTION),
                };
                let r = a / b;
                if r.is_finite() { Ok(InstanceState::Int(Some(r as i64))) } else { throws(c::CONST_EVAL_THROWS_EXCEPTION) }
            }
            _ => {
                Self::assert_num_or_null(self)?;
                Self::assert_num_or_null(d)?;
                throws(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    fn compare(&self, d: &InstanceState, f: impl Fn(std::cmp::Ordering) -> bool) -> R<InstanceState> {
        let value = |s: &InstanceState| -> Option<Option<f64>> {
            match s {
                InstanceState::Int(v) => Some(v.map(|i| i as f64)),
                InstanceState::Double(v) => Some(*v),
                _ => None,
            }
        };
        match (self, d) {
            (InstanceState::Int(Some(a)), InstanceState::Int(Some(b))) => Ok(InstanceState::Bool(Some(f(a.cmp(b))))),
            _ => {
                let Some(a) = value(self) else {
                    Self::assert_num_or_null(self)?;
                    Self::assert_num_or_null(d)?;
                    return throws(c::CONST_EVAL_THROWS_EXCEPTION);
                };
                Self::assert_num_or_null(d)?;
                let Some(a) = a else { return Ok(InstanceState::Bool(None)) };
                let Some(b) = value(d) else { return throws(c::CONST_EVAL_THROWS_EXCEPTION) };
                let Some(b) = b else { return Ok(InstanceState::Bool(None)) };
                Ok(InstanceState::Bool(Some(match a.partial_cmp(&b) {
                    Some(o) => f(o),
                    None => false,
                })))
            }
        }
    }

    pub fn greater(&self, d: &InstanceState) -> R<InstanceState> {
        self.compare(d, |o| o == std::cmp::Ordering::Greater)
    }
    pub fn greater_or_equal(&self, d: &InstanceState) -> R<InstanceState> {
        self.compare(d, |o| o != std::cmp::Ordering::Less)
    }
    pub fn less(&self, d: &InstanceState) -> R<InstanceState> {
        self.compare(d, |o| o == std::cmp::Ordering::Less)
    }
    pub fn less_or_equal(&self, d: &InstanceState) -> R<InstanceState> {
        self.compare(d, |o| o != std::cmp::Ordering::Greater)
    }

    fn bits(&self, d: &InstanceState, f: impl Fn(i64, i64) -> Option<InstanceState>) -> R<InstanceState> {
        match self {
            InstanceState::Int(v) => {
                Self::assert_int_or_null(d)?;
                let Some(a) = *v else { return Ok(InstanceState::Int(None)) };
                match d {
                    InstanceState::Int(None) => Ok(InstanceState::Int(None)),
                    InstanceState::Int(Some(b)) => f(a, *b).map_or_else(|| throws(c::CONST_EVAL_THROWS_EXCEPTION), Ok),
                    _ => throws(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            _ => {
                Self::assert_int_or_null(self)?;
                Self::assert_int_or_null(d)?;
                throws(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    pub fn bit_and(&self, d: &InstanceState) -> R<InstanceState> {
        self.bits(d, |a, b| Some(InstanceState::Int(Some(a & b))))
    }
    pub fn bit_or(&self, d: &InstanceState) -> R<InstanceState> {
        self.bits(d, |a, b| Some(InstanceState::Int(Some(a | b))))
    }
    pub fn bit_xor(&self, d: &InstanceState) -> R<InstanceState> {
        self.bits(d, |a, b| Some(InstanceState::Int(Some(a ^ b))))
    }
    /// `<<`: a shift wider than 31 bits is unknown.
    pub fn shift_left(&self, d: &InstanceState) -> R<InstanceState> {
        self.bits(d, |a, b| {
            if bit_length(b) > 31 {
                Some(InstanceState::Int(None))
            } else if b >= 0 {
                Some(InstanceState::Int(Some(if b >= 64 { 0 } else { a.wrapping_shl(b as u32) })))
            } else {
                None
            }
        })
    }
    pub fn shift_right(&self, d: &InstanceState) -> R<InstanceState> {
        self.bits(d, |a, b| {
            if bit_length(b) > 31 {
                Some(InstanceState::Int(None))
            } else if b >= 0 {
                Some(InstanceState::Int(Some(if b >= 64 { if a < 0 { -1 } else { 0 } } else { a >> b })))
            } else {
                None
            }
        })
    }
    pub fn logical_shift_right(&self, d: &InstanceState) -> R<InstanceState> {
        self.bits(d, |a, b| {
            if b >= 64 {
                Some(InstanceState::Int(Some(0)))
            } else if b >= 0 {
                Some(InstanceState::Int(Some(((a as u64) >> b) as i64)))
            } else {
                None
            }
        })
    }

    /// `bitNot`.
    pub fn negate_bits(&self) -> R<InstanceState> {
        match self {
            InstanceState::Int(v) => Ok(InstanceState::Int(v.map(|i| !i))),
            _ => {
                Self::assert_int_or_null(self)?;
                throws(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `negated`.
    pub fn negate(&self) -> R<InstanceState> {
        match self {
            InstanceState::Int(v) => Ok(InstanceState::Int(v.map(|i| i.wrapping_neg()))),
            InstanceState::Double(v) => Ok(InstanceState::Double(v.map(|d| -d))),
            _ => {
                Self::assert_num_or_null(self)?;
                throws(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `logicalNot`.
    pub fn logical_not(&self) -> R<InstanceState> {
        match self {
            InstanceState::Bool(v) => Ok(InstanceState::Bool(v.map(|b| !b))),
            InstanceState::Null { .. } => throws(c::CONST_EVAL_THROWS_EXCEPTION),
            _ => {
                Self::assert_bool(self)?;
                Ok(InstanceState::Bool(Some(true)))
            }
        }
    }

    fn logical(&self, d: &InstanceState, f: impl Fn(bool, bool) -> bool) -> R<InstanceState> {
        Self::assert_bool(self)?;
        Self::assert_bool(d)?;
        let (InstanceState::Bool(a), InstanceState::Bool(b)) = (self.to_bool()?, d.to_bool()?) else { unreachable!() };
        Ok(InstanceState::Bool(match (a, b) {
            (Some(a), Some(b)) => Some(f(a, b)),
            _ => None,
        }))
    }
    pub fn logical_and(&self, d: &InstanceState) -> R<InstanceState> {
        self.logical(d, |a, b| a & b)
    }
    pub fn logical_or(&self, d: &InstanceState) -> R<InstanceState> {
        self.logical(d, |a, b| a | b)
    }
    pub fn logical_xor(&self, d: &InstanceState) -> R<InstanceState> {
        self.logical(d, |a, b| a ^ b)
    }

    /// `stringLength` (in UTF-16 code units).
    pub fn length(&self) -> R<InstanceState> {
        match self {
            InstanceState::Str(v) => Ok(InstanceState::Int(v.as_ref().map(|s| s.len() as i64))),
            _ => {
                Self::assert_string(self)?;
                throws(c::CONST_EVAL_THROWS_EXCEPTION)
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
pub fn the_text(s: &str) -> Text {
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

impl Value {
    /// The type comes already erased (`extensionTypeErasure`, done by the
    /// engine).
    pub fn new(ty: TypeId, state: InstanceState) -> Value {
        Value { ty, state, variable: None }
    }

    pub fn null(core: &CoreTypes) -> Value {
        Value { ty: core.null, state: InstanceState::Null { invalid: false }, variable: None }
    }

    pub fn bool_(core: &CoreTypes, v: Option<bool>) -> Value {
        Value { ty: core.bool_, state: InstanceState::Bool(v), variable: None }
    }

    /// `validWithUnknownValue` (the type already erased).
    pub fn unknown(table: &TypeTable, core: &CoreTypes, ty: TypeId, list_element: Option<TypeId>) -> Value {
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
            InstanceState::Bool(None)
        } else if ty == core.double || of(core.double_class) {
            InstanceState::Double(None)
        } else if ty == core.int || of(core.int_class) {
            InstanceState::Int(None)
        } else if ty == core.string || of(core.string_class) {
            InstanceState::Str(None)
        } else if cls.is_some() && cls == core.list_class {
            InstanceState::List { element: list_element.unwrap_or(core.dynamic_), elements: Rc::new(Vec::new()), unknown: true }
        } else if cls.is_some() && cls == core.map_class {
            InstanceState::Map { entries: Rc::new(Vec::new()), unknown: true }
        } else if cls.is_some() && cls == core.set_class {
            InstanceState::Set { elements: Rc::new(Vec::new()), unknown: true }
        } else {
            InstanceState::Generic { fields: Rc::new(Vec::new()), unknown: true }
        };
        Value { ty, state, variable: None }
    }

    pub fn actually_unknown(&self) -> bool {
        self.state.unknown()
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.state {
            InstanceState::Bool(v) => v,
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&Text> {
        match &self.state {
            InstanceState::Str(Some(s)) => Some(s),
            _ => None,
        }
    }

    pub fn as_type(&self) -> Option<TypeId> {
        match self.state {
            InstanceState::Type(t) => t,
            _ => None,
        }
    }

    /// `toListValue`.
    pub fn as_list(&self) -> Option<&Rc<Vec<Value>>> {
        match &self.state {
            InstanceState::List { elements, .. } => Some(elements),
            _ => None,
        }
    }

    /// `toSetValue`.
    pub fn as_set(&self) -> Option<&Rc<Vec<Value>>> {
        match &self.state {
            InstanceState::Set { elements, .. } => Some(elements),
            _ => None,
        }
    }

    /// `toMapValue`.
    pub fn as_map(&self) -> Option<&Rc<Vec<(Value, Value)>>> {
        match &self.state {
            InstanceState::Map { entries, .. } => Some(entries),
            _ => None,
        }
    }

    pub fn as_function(&self) -> Option<Function> {
        match self.state {
            InstanceState::Function { element, .. } => Some(element),
            _ => None,
        }
    }
}
