//! The JS interop members of `dart:js_interop` on the WebAssembly target: the
//! `external` members of interop extension types, of extensions on interop
//! types, of `@JS`/`@staticInterop` classes and the `@JS` top-level members.
//! Each becomes a snippet with dart2wasm's trampoline (`pkg/dart2wasm/lib/js/
//! interop_specializer.dart`) and the conversions of dart2wasm 3.6 on each side.
//!
//! As in dart2wasm, a call to a method or a constructor passes only the
//! positional arguments written at the call site (`f(1)` calls `x0.f(x1)`, not
//! `x0.f(x1, null)`), an object literal constructor only the named arguments
//! written (`({a: x0})`), and a tear-off or an indirect call passes every
//! parameter.

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use crate::js_interop::Kind;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElement, FunctionKind, FunctionRef, LibraryId, VariableRef,
};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;
use dartforge_types::table::{Type as Ty, TypeId};

/// What the trampoline of an interop member does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    /// `new C(args)`.
    Constructor,
    /// `({name: arg, …})`: an external constructor with named parameters, or a
    /// factory of an `@anonymous` class.
    ObjectLiteral,
    Getter,
    Setter,
    Method,
    /// `operator []`.
    IndexGet,
    /// `operator []=`.
    IndexSet,
}

/// An interop member: its shape and its JavaScript name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InteropMember {
    pub shape: Shape,
    /// The receiver is the first argument (an instance member); otherwise the
    /// name starts at `globalThis` (a static or top-level member) or names the
    /// class of a constructor.
    pub instance: bool,
    /// The dotted name: of the property on the receiver, or the full path.
    pub selector: String,
}

/// The arguments the call site writes (`match_args`): how many positional
/// ones, and the names of the named ones.
#[derive(Clone, Debug, Default)]
pub(crate) struct CallShape {
    pub positional: usize,
    pub named: Vec<SymbolId>,
}

impl InteropMember {
    /// The trampoline's JavaScript for the parameters `params` (the receiver
    /// first, for an instance member) and, for an object literal, the keys of
    /// the arguments: dart2wasm's `generateJS`.
    pub fn javascript(&self, params: &[String], keys: &[String]) -> String {
        let receiver = if self.shape == Shape::Constructor || self.shape == Shape::ObjectLiteral {
            String::new()
        } else if self.instance {
            params[0].clone()
        } else {
            "globalThis".to_string()
        };
        let arguments = if self.instance { &params[1..] } else { params };
        let body = match self.shape {
            Shape::Constructor => {
                format!(
                    "new {}({})",
                    recombine(&self.selector, ""),
                    arguments.join(",")
                )
            }
            Shape::ObjectLiteral => {
                let pairs: Vec<String> = keys
                    .iter()
                    .zip(arguments)
                    .map(|(k, a)| format!("{k}: {a}"))
                    .collect();
                format!("({{{}}})", pairs.join(","))
            }
            Shape::Getter => recombine(&self.selector, &receiver),
            Shape::Setter => format!(
                "{} = {}",
                recombine(&self.selector, &receiver),
                arguments[0]
            ),
            Shape::Method => {
                format!(
                    "{}({})",
                    recombine(&self.selector, &receiver),
                    arguments.join(",")
                )
            }
            Shape::IndexGet => format!("{receiver}[{}]", arguments[0]),
            Shape::IndexSet => format!("{receiver}[{}] = {}", arguments[0], arguments[1]),
        };
        let list = params.join(",");
        if params.len() == 1 {
            format!("{list} => {body}")
        } else {
            format!("({list}) => {body}")
        }
    }
}

/// dart2wasm's `_splitSelectorsAndRecombine`: the dotted `selector` after
/// `receiver`, with dot notation where the part is an identifier and brackets
/// otherwise (a bracket needs a receiver: `globalThis`).
fn recombine(selector: &str, receiver: &str) -> String {
    let mut out = String::from(receiver);
    for part in selector.split('.') {
        let identifier = part
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
        if identifier {
            if !out.is_empty() {
                out.push('.');
            }
            out.push_str(part);
        } else {
            if out.is_empty() {
                out.push_str("globalThis");
            }
            out.push_str(&format!("['{part}']"));
        }
    }
    out
}

/// The metadata on the declaration of `f` (the member, the top-level function,
/// the constructor, or the variable of an implicit accessor), with its unit.
fn function_metadata<'p>(
    ctx: &'p Context,
    f: &FunctionElement,
) -> Option<(dartforge_elements::model::UnitId, &'p [ast::Annotation])> {
    if let Some(v) = f.variable {
        return match ctx.program.variables[v.0 as usize].node {
            VariableRef::TopLevel { unit, decl, .. } => {
                Some((unit, &*ctx.program.unit(unit).ast.decl(decl).metadata))
            }
            VariableRef::Field { unit, member, .. } => {
                Some((unit, &*ctx.program.unit(unit).ast.member(member).metadata))
            }
            _ => None,
        };
    }
    match f.node {
        FunctionRef::Function { unit, function } => {
            let a = &ctx.program.unit(unit).ast;
            let meta = a
                .members
                .iter()
                .find(|m| matches!(m.kind, ast::MemberKind::Method(id) if id == function))
                .map(|m| &*m.metadata)
                .or_else(|| {
                    a.decls
                        .iter()
                        .find(|d| matches!(d.kind, ast::DeclKind::Function(id) if id == function))
                        .map(|d| &*d.metadata)
                })?;
            Some((unit, meta))
        }
        FunctionRef::Constructor { unit, member } => {
            Some((unit, &*ctx.program.unit(unit).ast.member(member).metadata))
        }
        FunctionRef::None => None,
    }
}

fn class_metadata<'p>(
    ctx: &'p Context,
    c: ClassId,
) -> Option<(dartforge_elements::model::UnitId, &'p [ast::Annotation])> {
    let d = ctx.program.classes[c.0 as usize].decl?;
    Some((d.unit, &*ctx.program.unit(d.unit).ast.decl(d.decl).metadata))
}

/// The element an annotation names, resolved in its unit.
fn annotation_element(
    ctx: &Context,
    unit: dartforge_elements::model::UnitId,
    a: &ast::Annotation,
) -> Option<Element> {
    let binding = match &a.name[..] {
        [name] => ctx.program.lookup_in_unit(unit, name.sym),
        [prefix, name] => {
            // `p.JS(...)` with an import prefix, or `JS.named(...)`.
            ctx.program
                .lookup_prefixed_in_unit(unit, prefix.sym, name.sym)
                .or_else(|| ctx.program.lookup_in_unit(unit, prefix.sym))
        }
        [prefix, name, _] => ctx
            .program
            .lookup_prefixed_in_unit(unit, prefix.sym, name.sym),
        _ => None,
    }?;
    binding.getter
}

fn interop_library(ctx: &Context, lib: LibraryId) -> bool {
    matches!(
        ctx.program.library(lib).uri.as_str(),
        "dart:js_interop" | "dart:_js_annotations"
    )
}

/// `@JS(…)` among `metadata`: `Some("")` for `@JS()`, `Some(name)` for `@JS('name')`.
fn js_annotation(
    ctx: &Context,
    unit: dartforge_elements::model::UnitId,
    metadata: &[ast::Annotation],
) -> Option<String> {
    metadata.iter().find_map(|a| {
        let Some(Element::Class(c)) = annotation_element(ctx, unit, a) else {
            return None;
        };
        let class = &ctx.program.classes[c.0 as usize];
        if ctx.symbol_name(class.name) != "JS" || !interop_library(ctx, class.library) {
            return None;
        }
        let name = a
            .arguments
            .as_ref()
            .and_then(|args| args.args.first())
            .and_then(
                |arg| match &ctx.program.unit(unit).ast.expr(arg.value).kind {
                    ast::ExprKind::String(s) => s.constant_value().map(|v| v.to_string_lossy()),
                    _ => None,
                },
            )
            .unwrap_or_default();
        Some(name)
    })
}

/// `@staticInterop` or `@anonymous` (the constants of `dart:_js_annotations`).
fn has_marker(
    ctx: &Context,
    unit: dartforge_elements::model::UnitId,
    metadata: &[ast::Annotation],
    marker: &str,
) -> bool {
    metadata.iter().any(|a| {
        matches!(annotation_element(ctx, unit, a), Some(Element::Variable(v))
            if ctx.symbol_name(ctx.program.variables[v.0 as usize].name) == marker
                && interop_library(ctx, ctx.program.variables[v.0 as usize].library))
    })
}

/// The `@JS` name of a library (`@JS('a.b') library;`), or "".
fn library_js_name(ctx: &Context, lib: LibraryId) -> String {
    let Some(&unit) = ctx.program.library(lib).units.first() else {
        return String::new();
    };
    ctx.program
        .unit(unit)
        .unit
        .directives
        .iter()
        .filter(|d| matches!(d.kind, ast::DirectiveKind::Library { .. }))
        .find_map(|d| js_annotation(ctx, unit, &d.metadata))
        .unwrap_or_default()
}

/// dart2wasm's `_getTopLevelJSString`: the `@JS` name (or `written`),
/// prefixed by the library's.
fn top_level_js_string(
    ctx: &Context,
    own: Option<String>,
    written: &str,
    lib: LibraryId,
) -> String {
    let name = own
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| written.to_string());
    let library = library_js_name(ctx, lib);
    if library.is_empty() {
        name
    } else {
        format!("{library}.{name}")
    }
}

/// Whether `c` is a `@JS` class (`package:js`, `@staticInterop`).
fn js_class(ctx: &Context, c: ClassId) -> bool {
    class_metadata(ctx, c).is_some_and(|(u, meta)| js_annotation(ctx, u, meta).is_some())
}

fn static_interop_class(ctx: &Context, c: ClassId) -> bool {
    class_metadata(ctx, c).is_some_and(|(u, meta)| has_marker(ctx, u, meta, "staticInterop"))
}

/// dart2wasm's "interop extension type": its representation is a JS type of
/// `dart:js_interop`, a `@staticInterop` class or another interop extension type.
pub(crate) fn interop_extension_type(ctx: &Context, c: ClassId) -> bool {
    let mut current = c;
    for _ in 0..64 {
        let class = &ctx.program.classes[current.0 as usize];
        if class.kind != ClassKind::ExtensionType {
            return false;
        }
        if ctx.program.library(class.library).uri == "dart:js_interop"
            && ctx.symbol_name(class.name).starts_with("JS")
        {
            return true;
        }
        let Some(rep) = class.representation else {
            return false;
        };
        let Some(t) = ctx
            .outline
            .variables
            .get(rep.0 as usize)
            .and_then(|v| v.declared_type)
        else {
            return false;
        };
        match ctx.table.get(t) {
            Ty::ExtensionType { decl, .. } => current = *decl,
            Ty::Interface { class, .. } => return static_interop_class(ctx, *class),
            _ => return false,
        }
    }
    false
}

/// An interop type in the sense of an extension's `on`: an interop extension
/// type, a `@JS` class, or a type parameter bounded by one.
fn interop_on_type(ctx: &Context, t: TypeId) -> bool {
    let mut t = t;
    for _ in 0..64 {
        match ctx.table.get(t) {
            Ty::ExtensionType { decl, .. } => return interop_extension_type(ctx, *decl),
            Ty::Interface { class, .. } => return js_class(ctx, *class),
            Ty::TypeParameter { param, .. } => t = ctx.table.param(*param).bound,
            _ => return false,
        }
    }
    false
}

/// The JavaScript name of a member: its `@JS` name, otherwise its Dart name
/// (a setter's without the `=`).
fn member_js_name(ctx: &Context, f: &FunctionElement) -> String {
    let own = function_metadata(ctx, f)
        .and_then(|(u, meta)| js_annotation(ctx, u, meta))
        .filter(|n| !n.is_empty());
    own.unwrap_or_else(|| ctx.symbol_name(f.name).trim_end_matches('=').to_string())
}

fn accessor_shape(ctx: &Context, f: &FunctionElement) -> Option<Shape> {
    let name = ctx.symbol_name(f.name);
    Some(match f.kind {
        FunctionKind::Getter => Shape::Getter,
        FunctionKind::Setter => Shape::Setter,
        FunctionKind::ImplicitAccessor if name.ends_with('=') => Shape::Setter,
        FunctionKind::ImplicitAccessor => Shape::Getter,
        FunctionKind::Operator if name == "[]" => Shape::IndexGet,
        FunctionKind::Operator if name == "[]=" => Shape::IndexSet,
        FunctionKind::Operator => return None,
        FunctionKind::Function => Shape::Method,
        FunctionKind::Constructor | FunctionKind::SyntheticConstructor => return None,
    })
}

fn has_named_parameters(ctx: &Context, fid: usize) -> bool {
    ctx.outline.functions.get(fid).is_some_and(|d| {
        d.parameters
            .iter()
            .any(|p| p.kind == ast::ParameterKind::Named)
    })
}

/// The interop member `fid` is, if it is one (dart2wasm's `_getSpecializer`).
/// `Err` names an interop operator dart2wasm does not support either.
pub(crate) fn interop_member(ctx: &Context, fid: usize) -> Option<Result<InteropMember, String>> {
    let f = &ctx.program.functions[fid];
    if !f.external || f.patched_by.is_some() {
        return None;
    }
    let constructor = matches!(f.node, FunctionRef::Constructor { .. })
        || f.factory
        || matches!(f.kind, FunctionKind::Constructor);
    let operator = |shape: Option<Shape>| {
        shape.ok_or_else(|| {
            format!(
                "External operator {} is unsupported for static interop. Please file a \
                 request in the SDK if you want it to be supported.",
                ctx.symbol_name(f.name)
            )
        })
    };
    if let Some(c) = f.class {
        let class = &ctx.program.classes[c.0 as usize];
        if class.kind == ClassKind::ExtensionType {
            if !interop_extension_type(ctx, c) {
                return None;
            }
            let own = class_metadata(ctx, c).and_then(|(u, meta)| js_annotation(ctx, u, meta));
            let class_string =
                top_level_js_string(ctx, own, ctx.symbol_name(class.name), f.library);
            if constructor {
                let shape = if has_named_parameters(ctx, fid) {
                    Shape::ObjectLiteral
                } else {
                    Shape::Constructor
                };
                return Some(Ok(InteropMember {
                    shape,
                    instance: false,
                    selector: class_string,
                }));
            }
            let shape = match operator(accessor_shape(ctx, f)) {
                Ok(s) => s,
                Err(e) => return Some(Err(e)),
            };
            let name = member_js_name(ctx, f);
            return Some(Ok(if f.static_ {
                InteropMember {
                    shape,
                    instance: false,
                    selector: format!("{class_string}.{name}"),
                }
            } else {
                InteropMember {
                    shape,
                    instance: true,
                    selector: name,
                }
            }));
        }
        if js_class(ctx, c) {
            let own = class_metadata(ctx, c).and_then(|(u, meta)| js_annotation(ctx, u, meta));
            let class_string =
                top_level_js_string(ctx, own, ctx.symbol_name(class.name), class.library);
            if f.factory {
                let anonymous = class_metadata(ctx, c)
                    .is_some_and(|(u, meta)| has_marker(ctx, u, meta, "anonymous"));
                return Some(Ok(InteropMember {
                    shape: if anonymous {
                        Shape::ObjectLiteral
                    } else {
                        Shape::Constructor
                    },
                    instance: false,
                    selector: class_string,
                }));
            }
            if !f.static_ {
                return None;
            }
            let shape = match operator(accessor_shape(ctx, f)) {
                Ok(s) => s,
                Err(e) => return Some(Err(e)),
            };
            return Some(Ok(InteropMember {
                shape,
                instance: false,
                selector: format!("{class_string}.{}", member_js_name(ctx, f)),
            }));
        }
        return None;
    }
    if let Some(e) = f.extension {
        if f.static_ {
            return None;
        }
        let on = ctx.outline.extensions.get(e.0 as usize)?.on;
        if !interop_on_type(ctx, on) {
            return None;
        }
        let shape = match operator(accessor_shape(ctx, f)) {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };
        return Some(Ok(InteropMember {
            shape,
            instance: true,
            selector: member_js_name(ctx, f),
        }));
    }
    let (unit, meta) = function_metadata(ctx, f)?;
    let own = js_annotation(ctx, unit, meta)?;
    let shape = match operator(accessor_shape(ctx, f)) {
        Ok(s) => s,
        Err(e) => return Some(Err(e)),
    };
    let written = ctx.symbol_name(f.name).trim_end_matches('=').to_string();
    Some(Ok(InteropMember {
        shape,
        instance: false,
        selector: top_level_js_string(ctx, Some(own), &written, f.library),
    }))
}

/// How an argument crosses, by its declared type (dart2wasm's `jsifyValue`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Argument {
    /// A JS type: its handle.
    Value { nullable: bool },
    /// `double`.
    Number,
    /// `int`, as a number.
    Integer,
    /// `num`: `toDouble()`, then a number.
    Num,
    /// `bool`: a JavaScript boolean.
    Boolean,
    /// `String`, `String?`.
    Text,
    /// `void` or `Null`: JavaScript `null`.
    Null,
    /// Anything else, nullable primitives included: `jsifyRaw`.
    Jsify,
}

/// How the result comes back, by the declared return type (dart2wasm's
/// `castInvocationForReturn`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Result_ {
    Void,
    /// A JS type: boxed; a non-nullable one with a null check.
    Value {
        nullable: bool,
    },
    /// Through a conversion helper of `dart:_js_helper`.
    Helper(&'static str),
}

impl FnBuilder<'_, '_> {
    fn core_class_name(&self, ty: TypeId) -> Option<(&str, bool)> {
        let Ty::Interface {
            class, nullable, ..
        } = self.ctx.table.get(ty)
        else {
            return None;
        };
        let c = &self.ctx.program.classes[class.0 as usize];
        (self.ctx.program.library(c.library).uri == "dart:core")
            .then(|| (self.ctx.symbol_name(c.name), *nullable))
    }

    fn js_value_type(&self, ty: TypeId) -> Option<bool> {
        let js = self.js_helpers()?;
        match self.ctx.table.get(ty) {
            Ty::Interface {
                class, nullable, ..
            } if *class == js.class => Some(*nullable),
            _ => None,
        }
    }

    fn argument_kind(&self, declared: TypeId) -> Argument {
        let ty = self.ctx.erase(declared);
        if self.ctx.is_void(ty) {
            return Argument::Null;
        }
        if let Some(nullable) = self.js_value_type(ty) {
            return Argument::Value { nullable };
        }
        // A type parameter bounded by a JS type (`T extends JSAny?`).
        if let Ty::TypeParameter { param, .. } = self.ctx.table.get(ty) {
            let bound = self.ctx.erase(self.ctx.table.param(*param).bound);
            if self.js_value_type(bound).is_some() {
                return Argument::Value { nullable: true };
            }
        }
        match self.core_class_name(ty) {
            Some(("Null", _)) => Argument::Null,
            Some(("String", _)) => Argument::Text,
            Some(("double", false)) => Argument::Number,
            Some(("int", false)) => Argument::Integer,
            Some(("num", false)) => Argument::Num,
            Some(("bool", false)) => Argument::Boolean,
            _ => Argument::Jsify,
        }
    }

    fn result_kind(&self, declared: TypeId) -> Result_ {
        let ty = self.ctx.erase(declared);
        if self.ctx.is_void(ty) {
            return Result_::Void;
        }
        if let Some(nullable) = self.js_value_type(ty) {
            return Result_::Value { nullable };
        }
        if let Ty::TypeParameter { param, .. } = self.ctx.table.get(ty) {
            let bound = self.ctx.erase(self.ctx.table.param(*param).bound);
            if self.js_value_type(bound).is_some() {
                return Result_::Value { nullable: true };
            }
        }
        Result_::Helper(match self.core_class_name(ty) {
            Some(("int", false)) => "jsResultInt",
            Some(("int", true)) => "jsResultNullableInt",
            Some(("double", false)) => "jsResultDouble",
            Some(("double", true)) => "jsResultNullableDouble",
            Some(("num", false)) => "jsResultNum",
            Some(("num", true)) => "jsResultNullableNum",
            Some(("bool", false)) => "jsResultBool",
            Some(("bool", true)) => "jsResultNullableBool",
            Some(("String", false)) => "jsResultString",
            Some(("String", true)) => "jsResultNullableString",
            _ => "jsResultAny",
        })
    }

    /// The call to the interop member `fid`: the arguments of the parameters
    /// the call site writes (`shape`; every one without it), converted, the
    /// trampoline, and the result converted to the declared return type.
    pub(super) fn call_interop(
        &mut self,
        fid: usize,
        member: &InteropMember,
        this: Option<Operand>,
        args: &[Operand],
        shape: Option<CallShape>,
    ) -> Operand {
        let span = Span { start: 0, end: 0 };
        if !crate::target::is_wasm() {
            return self.unsupported("JS interop member outside the WebAssembly target", span);
        }
        let Some(js) = self.js_helpers() else {
            return self.unsupported("dart:_js_helper without JSValue", span);
        };
        let Some(data) = self.ctx.outline.functions.get(fid) else {
            return self.unsupported("JS interop member without a signature", span);
        };
        let parameters: Vec<_> = data.parameters.to_vec();
        let return_type = data.return_type;
        use ast::ParameterKind;
        // The parameters that cross: by the shape of the member and of the call.
        let mut used: Vec<usize> = Vec::new();
        let mut keys: Vec<String> = Vec::new();
        match member.shape {
            Shape::Getter => {}
            Shape::Setter | Shape::IndexGet => used.push(0),
            Shape::IndexSet => used.extend([0, 1]),
            Shape::Method | Shape::Constructor => {
                let positional = parameters
                    .iter()
                    .filter(|p| p.kind != ParameterKind::Named)
                    .count();
                let n = shape
                    .as_ref()
                    .map_or(positional, |s| s.positional.min(positional));
                used.extend(0..n);
            }
            Shape::ObjectLiteral => {
                for (i, p) in parameters.iter().enumerate() {
                    let written = shape
                        .as_ref()
                        .is_none_or(|s| p.external_name.is_some_and(|n| s.named.contains(&n)));
                    if p.kind == ParameterKind::Named && written {
                        used.push(i);
                        keys.push(
                            p.external_name
                                .map(|n| self.ctx.symbol_name(n).to_string())
                                .unwrap_or_default(),
                        );
                    }
                }
            }
        }
        if used.iter().any(|&i| i >= args.len()) {
            return self.unsupported("JS interop member with missing arguments", span);
        }
        // The receiver and the arguments, converted on the Dart side first (what
        // allocates: `jsifyRaw`, `toDouble`), then crossing.
        let mut values: Vec<(Operand, Kind, bool)> = Vec::new();
        if member.instance {
            let Some(this) = this else {
                return self.unsupported("JS interop instance member without a receiver", span);
            };
            values.push((self.coerce(this, Type::Ref), Kind::Value, true));
        }
        for &i in &used {
            let arg = args[i].clone();
            let value = match self.argument_kind(parameters[i].ty) {
                Argument::Value { nullable } => {
                    (self.coerce(arg, Type::Ref), Kind::Value, nullable)
                }
                Argument::Number => (self.coerce(arg, Type::F64), Kind::Number, false),
                Argument::Integer => (self.coerce(arg, Type::I64), Kind::Integer, false),
                Argument::Num => {
                    let n = self.coerce(arg, Type::Ref);
                    let Some(f) = self.js_helper_function("jsArgumentNum") else {
                        return self.unsupported("dart:_js_helper without jsArgumentNum", span);
                    };
                    let d = self.call_direct(f, None, vec![n]);
                    (self.coerce(d, Type::F64), Kind::Number, false)
                }
                Argument::Boolean => (self.coerce(arg, Type::I1), Kind::JsBoolean, false),
                Argument::Text => (self.coerce(arg, Type::Ref), Kind::Text, true),
                Argument::Null => (Operand::Constant(Constant::Null), Kind::Value, true),
                Argument::Jsify => {
                    let Some(f) = self.js_helper_function("jsifyRaw") else {
                        return self.unsupported("dart:_js_helper without jsifyRaw", span);
                    };
                    let v = self.coerce(arg, Type::Ref);
                    (self.call_direct(f, None, vec![v]), Kind::Value, true)
                }
            };
            values.push(value);
        }
        let result = self.result_kind(return_type);
        let ret = match result {
            Result_::Void => Kind::Void,
            Result_::Value { .. } | Result_::Helper(_) => Kind::Value,
        };
        let params: Vec<String> = (0..values.len()).map(|i| format!("x{i}")).collect();
        let code = member.javascript(&params, &keys);
        let raw = self.emit_snippet(&code, values, ret, js);
        match result {
            Result_::Void => Operand::Constant(Constant::Null),
            Result_::Value { nullable: true } => {
                self.call_direct(js.boxer.0 as usize, None, vec![raw])
            }
            Result_::Value { nullable: false } => match self.js_helper_function("jsResultValue") {
                Some(f) => self.call_direct(f, None, vec![raw]),
                None => self.unsupported("dart:_js_helper without jsResultValue", span),
            },
            Result_::Helper(name) => match self.js_helper_function(name) {
                Some(f) => self.call_direct(f, None, vec![raw]),
                None => self.unsupported(&format!("dart:_js_helper without {name}"), span),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(shape: Shape, instance: bool, selector: &str) -> InteropMember {
        InteropMember {
            shape,
            instance,
            selector: selector.to_string(),
        }
    }

    fn params(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("x{i}")).collect()
    }

    /// The trampolines dart2wasm 3.6.2 writes for the same members (its `.mjs`).
    #[test]
    fn trampolines_match_dart2wasm() {
        assert_eq!(
            member(Shape::Getter, true, "i").javascript(&params(1), &[]),
            "x0 => x0.i"
        );
        assert_eq!(
            member(Shape::Setter, true, "i").javascript(&params(2), &[]),
            "(x0,x1) => x0.i = x1"
        );
        assert_eq!(
            member(Shape::Method, true, "f").javascript(&params(4), &[]),
            "(x0,x1,x2,x3) => x0.f(x1,x2,x3)"
        );
        assert_eq!(
            member(Shape::Method, true, "g").javascript(&params(1), &[]),
            "x0 => x0.g()"
        );
        assert_eq!(
            member(Shape::Constructor, false, "ns1.Date").javascript(&params(3), &[]),
            "(x0,x1,x2) => new ns1.Date(x0,x1,x2)"
        );
        assert_eq!(
            member(Shape::Method, false, "ns1.Date.now").javascript(&params(0), &[]),
            "() => globalThis.ns1.Date.now()"
        );
        assert_eq!(
            member(Shape::Getter, false, "ns1.a.b.c").javascript(&params(0), &[]),
            "() => globalThis.ns1.a.b.c"
        );
        assert_eq!(
            member(Shape::Method, false, "eval").javascript(&params(1), &[]),
            "x0 => globalThis.eval(x0)"
        );
        assert_eq!(
            member(Shape::ObjectLiteral, false, "")
                .javascript(&params(2), &["a".to_string(), "cc".to_string()]),
            "(x0,x1) => ({a: x0,cc: x1})"
        );
        assert_eq!(
            member(Shape::ObjectLiteral, false, "").javascript(&params(0), &[]),
            "() => ({})"
        );
        assert_eq!(
            member(Shape::IndexGet, true, "[]").javascript(&params(2), &[]),
            "(x0,x1) => x0[x1]"
        );
        assert_eq!(
            member(Shape::IndexSet, true, "[]=").javascript(&params(3), &[]),
            "(x0,x1,x2) => x0[x1] = x2"
        );
        // A part that is not an identifier goes in brackets, after `globalThis`
        // when it starts the name.
        assert_eq!(
            member(Shape::Getter, false, "a-b.c").javascript(&params(0), &[]),
            "() => globalThis['a-b'].c"
        );
        assert_eq!(
            member(Shape::Constructor, false, "my-class").javascript(&params(0), &[]),
            "() => new globalThis['my-class']()"
        );
    }
}
