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

/// Whether `fid` is the setter of a field or variable (its implicit accessor
/// takes the value).
fn implicit_setter(ctx: &Context, fid: usize) -> bool {
    ctx.program.functions[fid].kind == FunctionKind::ImplicitAccessor
        && ctx
            .outline
            .functions
            .get(fid)
            .is_some_and(|d| d.parameters.len() == 1)
}

fn accessor_shape(ctx: &Context, fid: usize) -> Option<Shape> {
    let f = &ctx.program.functions[fid];
    let name = ctx.symbol_name(f.name);
    Some(match f.kind {
        FunctionKind::Getter => Shape::Getter,
        FunctionKind::Setter => Shape::Setter,
        FunctionKind::ImplicitAccessor if implicit_setter(ctx, fid) => Shape::Setter,
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
    // The accessors of an `external` field or top-level variable are external.
    let external = f.external
        || f.variable
            .is_some_and(|v| ctx.program.variables[v.0 as usize].external);
    if !external || f.patched_by.is_some() {
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
            let shape = match operator(accessor_shape(ctx, fid)) {
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
            let shape = match operator(accessor_shape(ctx, fid)) {
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
        let shape = match operator(accessor_shape(ctx, fid)) {
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
    let shape = match operator(accessor_shape(ctx, fid)) {
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
        let mut parameters: Vec<_> = data.parameters.to_vec();
        let mut return_type = data.return_type;
        // The accessors of an `external` field or variable: the declared type
        // of the variable.
        if let Some(v) = self.ctx.program.functions[fid].variable {
            let ty = super::members::variable_type(self.ctx, v);
            match member.shape {
                Shape::Setter => match parameters.first_mut() {
                    Some(p) => p.ty = ty,
                    None => parameters.push(dartforge_types::resolve::ParameterTypeData {
                        name: None,
                        external_name: None,
                        ty,
                        required: true,
                        kind: ast::ParameterKind::Required,
                    }),
                },
                _ => return_type = ty,
            }
        }
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
        // `T`, a type parameter of the member or of its extension type: the
        // result is cast to what the call instantiates it with (dart2wasm casts
        // to the return type).
        let instantiated = match self.ctx.table.get(return_type) {
            Ty::TypeParameter {
                param,
                nullable: false,
            } => {
                let param = *param;
                let types = self.call_types.take().filter(|(f, _)| *f == fid);
                self.type_params_of(fid)
                    .iter()
                    .position(|&p| p == param)
                    .and_then(|i| types.and_then(|(_, t)| t.get(i).copied().flatten()))
            }
            _ => None,
        };
        let result = self.result_kind(return_type);
        let ret = match result {
            Result_::Void => Kind::Void,
            Result_::Value { .. } | Result_::Helper(_) => Kind::Value,
        };
        let params: Vec<String> = (0..values.len()).map(|i| format!("x{i}")).collect();
        let code = member.javascript(&params, &keys);
        let raw = self.emit_snippet(&code, values, ret, js);
        if let Some(t) = instantiated {
            let value = self.call_direct(js.boxer.0 as usize, None, vec![raw]);
            let rti = self.type_rti(t);
            self.cast_rti(value.clone(), rti);
            return value;
        }
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

/// The implicit getter (or setter) of an `external` interop variable (`@JS()
/// external int foo;`), whose reads and writes are the interop member's.
pub(crate) fn interop_variable_accessor(
    ctx: &Context,
    vid: dartforge_elements::model::VariableId,
    setter: bool,
) -> Option<usize> {
    if !ctx.program.variables[vid.0 as usize].external {
        return None;
    }
    ctx.program
        .functions
        .iter()
        .enumerate()
        .find(|(i, f)| f.variable == Some(vid) && implicit_setter(ctx, *i) == setter)
        .map(|(i, _)| i)
        .filter(|&i| interop_member(ctx, i).is_some())
}

/// Whether `fid` is `FunctionToJSExportedDartFunction.toJS` (`Some(false)`) or
/// `.toJSCaptureThis` (`Some(true)`) of `dart:js_interop`.
pub(crate) fn function_to_js(ctx: &Context, fid: usize) -> Option<bool> {
    let f = &ctx.program.functions[fid];
    let e = f.extension?;
    if ctx.program.library(f.library).uri != "dart:js_interop"
        || ctx.program.extensions[e.0 as usize]
            .name
            .is_none_or(|n| ctx.symbol_name(n) != "FunctionToJSExportedDartFunction")
    {
        return None;
    }
    match ctx.symbol_name(f.name) {
        "toJS" => Some(false),
        "toJSCaptureThis" => Some(true),
        _ => None,
    }
}

/// Whether `fid` is `createJSInteropWrapper` of `dart:js_interop` (or its
/// patch's `_createJSInteropWrapperOn`, which also takes the prototype).
pub(crate) fn create_js_interop_wrapper(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    f.class.is_none()
        && f.extension.is_none()
        && ctx.program.library(f.library).uri == "dart:js_interop"
        && matches!(
            ctx.symbol_name(f.name),
            "createJSInteropWrapper" | "_createJSInteropWrapperOn"
        )
}

/// `@JSExport(…)` among `metadata`: `Some("")` without a name.
fn js_export_annotation(
    ctx: &Context,
    unit: dartforge_elements::model::UnitId,
    metadata: &[ast::Annotation],
) -> Option<String> {
    metadata.iter().find_map(|a| {
        let Some(Element::Class(c)) = annotation_element(ctx, unit, a) else {
            return None;
        };
        let class = &ctx.program.classes[c.0 as usize];
        if ctx.symbol_name(class.name) != "JSExport" || !interop_library(ctx, class.library) {
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

/// What an export of a Dart object makes a property of the JavaScript object.
#[derive(Clone, Debug)]
pub(crate) enum ExportMember {
    /// The tear-off of the method, as a JavaScript function.
    Method(usize),
    /// Accessors: the name of the Dart getter and of the setter, with the type
    /// of the value, and the library of the member (for private names).
    Accessors {
        getter: Option<(SymbolId, TypeId)>,
        setter: Option<(SymbolId, TypeId)>,
        library: LibraryId,
    },
}

/// The exports of a class (dart2wasm's `ExportChecker`): its exportable
/// members and those of its superclasses (an override replaces the inherited
/// member), each with `@JSExport` or in a class with `@JSExport`, by export
/// name (`@JSExport('name')` or the Dart name), in declaration order.
pub(crate) fn exports_of(ctx: &Context, c: ClassId) -> Vec<(String, ExportMember)> {
    let mut chain = Vec::new();
    let mut current = Some(c);
    while let Some(k) = current {
        if Some(k) == ctx.core.object_class {
            break;
        }
        chain.push(k);
        let class = &ctx.program.classes[k.0 as usize];
        chain.extend(class.mixin_classes.iter().rev());
        current = class.supertype_class;
    }
    chain.reverse();
    // By Dart name (`x` and `x=`): the member that wins and its export name.
    #[derive(Clone)]
    enum Slot {
        Method(usize),
        Getter(SymbolId, TypeId),
        Setter(SymbolId, TypeId),
    }
    // The export name, the member and its library.
    type Entry = Option<(String, Slot, LibraryId)>;
    let mut members: Vec<(String, Entry)> = Vec::new();
    let mut put = |key: String, entry: Entry| match members.iter_mut().find(|(k, _)| *k == key) {
        Some(m) => m.1 = entry,
        None => members.push((key, entry)),
    };
    for k in chain {
        let class = &ctx.program.classes[k.0 as usize];
        let class_exported = class_metadata(ctx, k)
            .is_some_and(|(u, meta)| js_export_annotation(ctx, u, meta).is_some());
        for (i, f) in ctx.program.functions.iter().enumerate() {
            if f.class != Some(k) || f.static_ || f.abstract_ || f.external || f.factory {
                continue;
            }
            let exportable = match f.kind {
                FunctionKind::Function | FunctionKind::Getter | FunctionKind::Setter => {
                    f.variable.is_none()
                        && ctx
                            .outline
                            .functions
                            .get(i)
                            .is_some_and(|d| d.type_params.is_empty())
                }
                _ => false,
            };
            if !exportable {
                continue;
            }
            let own =
                function_metadata(ctx, f).and_then(|(u, meta)| js_export_annotation(ctx, u, meta));
            let name = ctx.symbol_name(f.name).trim_end_matches('=').to_string();
            let entry = (own.is_some() || class_exported).then(|| {
                let export = own
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| name.clone());
                let slot = match f.kind {
                    FunctionKind::Function => Slot::Method(i),
                    FunctionKind::Getter => Slot::Getter(
                        f.name,
                        ctx.outline
                            .functions
                            .get(i)
                            .map_or(ctx.core.dynamic_, |d| d.return_type),
                    ),
                    _ => Slot::Setter(
                        f.name,
                        ctx.outline
                            .functions
                            .get(i)
                            .and_then(|d| d.parameters.first().map(|p| p.ty))
                            .unwrap_or(ctx.core.dynamic_),
                    ),
                };
                (export, slot, f.library)
            });
            let key = if f.kind == FunctionKind::Setter {
                format!("{name}=")
            } else {
                name
            };
            put(key, entry);
        }
        for &v in &class.fields {
            let var = &ctx.program.variables[v.0 as usize];
            if var.static_ || var.external || class.representation == Some(v) {
                continue;
            }
            let own = match var.node {
                VariableRef::Field { unit, member, .. } => js_export_annotation(
                    ctx,
                    unit,
                    &ctx.program.unit(unit).ast.member(member).metadata,
                ),
                _ => None,
            };
            let name = ctx.symbol_name(var.name).to_string();
            let ty = super::members::variable_type(ctx, v);
            let export = (own.is_some() || class_exported).then(|| {
                own.clone()
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| name.clone())
            });
            put(
                name.clone(),
                export
                    .clone()
                    .map(|e| (e, Slot::Getter(var.name, ty), var.library)),
            );
            if !var.final_ && !var.const_ {
                put(
                    format!("{name}="),
                    export.map(|e| (e, Slot::Setter(var.name, ty), var.library)),
                );
            }
        }
    }
    let mut exports: Vec<(String, ExportMember)> = Vec::new();
    for (_, entry) in members {
        let Some((export, slot, library)) = entry else {
            continue;
        };
        match slot {
            Slot::Method(f) => exports.push((export, ExportMember::Method(f))),
            Slot::Getter(n, t) | Slot::Setter(n, t) => {
                let setter = matches!(slot, Slot::Setter(..));
                match exports.iter_mut().find(|(e, _)| *e == export) {
                    Some((
                        _,
                        ExportMember::Accessors {
                            getter, setter: s, ..
                        },
                    )) => {
                        if setter {
                            *s = Some((n, t));
                        } else {
                            *getter = Some((n, t));
                        }
                    }
                    _ => exports.push((
                        export,
                        ExportMember::Accessors {
                            getter: (!setter).then_some((n, t)),
                            setter: setter.then_some((n, t)),
                            library,
                        },
                    )),
                }
            }
        }
    }
    exports
}

/// Whether `fid` is `JSAnyUtilityExtension.isA` of `dart:js_interop`.
pub(crate) fn is_a(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    f.extension.is_some_and(|e| {
        ctx.program.library(f.library).uri == "dart:js_interop"
            && ctx.program.extensions[e.0 as usize]
                .name
                .is_some_and(|n| ctx.symbol_name(n) == "JSAnyUtilityExtension")
            && ctx.symbol_name(f.name) == "isA"
    })
}

/// The JS type of `dart:js_interop` that an interop extension type wraps, at the
/// end of its chain of representations.
fn core_js_type(ctx: &Context, c: ClassId) -> Option<ClassId> {
    let mut current = c;
    for _ in 0..64 {
        let class = &ctx.program.classes[current.0 as usize];
        if class.kind != ClassKind::ExtensionType {
            return None;
        }
        if ctx.program.library(class.library).uri == "dart:js_interop"
            && ctx.symbol_name(class.name).starts_with("JS")
        {
            return Some(current);
        }
        let t = ctx
            .outline
            .variables
            .get(class.representation?.0 as usize)?
            .declared_type?;
        match ctx.table.get(t) {
            Ty::ExtensionType { decl, .. } => current = *decl,
            _ => return None,
        }
    }
    None
}

impl FnBuilder<'_, '_> {
    /// A tear-off of an `external` interop member in program code is a compile
    /// error, as in dart2wasm (`_js_interop_checks`): `Some` with the
    /// operand of the diagnostic.
    pub(super) fn interop_tearoff(&mut self, fid: usize, span: Span) -> Option<Operand> {
        if self
            .ctx
            .program
            .library(self.ctx.program.unit(self.unit_id).library)
            .is_sdk
            || interop_member(self.ctx, fid).is_none()
        {
            return None;
        }
        let f = &self.ctx.program.functions[fid];
        let kind = match f.class {
            _ if f.extension.is_some() => "extension interop member",
            Some(c) if self.ctx.program.classes[c.0 as usize].kind == ClassKind::ExtensionType => {
                "extension type interop member"
            }
            Some(_) => "@staticInterop member",
            None => "top-level member",
        };
        let name = match self.ctx.symbol_name(f.name) {
            "" => "new",
            n => n,
        };
        let message = format!(
            "Tear-offs of external {kind} '{name}' are disallowed.\n\
             Declare a closure that calls this member instead."
        );
        Some(self.language_error(&message, span))
    }

    /// `createJSInteropWrapper<T>(object, [proto])` (dart2wasm's
    /// `_createExport`): a JavaScript object (or one with the prototype `proto`)
    /// with a property per export of `T`'s class — a method is its tear-off as
    /// a JavaScript function, getters and setters an accessor property
    /// (`Object.defineProperty`).
    pub(super) fn lower_js_interop_wrapper(
        &mut self,
        args: &[Operand],
        offset: Option<usize>,
    ) -> Operand {
        let span = Span { start: 0, end: 0 };
        let Some(&ty) = offset
            .and_then(|o| self.ctx.bodies.units[self.unit_id.0 as usize].instantiation(o))
            .and_then(|a| a.first())
        else {
            return self.unsupported("createJSInteropWrapper without its type argument", span);
        };
        let Ty::Interface { class, .. } = self.ctx.table.get(ty) else {
            return self.unsupported("createJSInteropWrapper of a type that is not a class", span);
        };
        let class = *class;
        let (Some(object), Some(set_helper), Some(accessor_helper)) = (
            self.js_helper_function("jsExportObject"),
            self.js_helper_function("jsExportProperty"),
            self.js_helper_function("jsExportAccessor"),
        ) else {
            return self.unsupported("dart:_js_helper without the export helpers", span);
        };
        let instance = self.coerce(
            args.first()
                .cloned()
                .unwrap_or(Operand::Constant(Constant::Null)),
            Type::Ref,
        );
        let proto = self.coerce(
            args.get(1)
                .cloned()
                .unwrap_or(Operand::Constant(Constant::Null)),
            Type::Ref,
        );
        let literal = self.call_direct(object, None, vec![proto]);
        for (name, member) in exports_of(self.ctx, class) {
            let name_op = self.emit(
                Instruction::Const(Constant::StringWtf8(name.into_bytes())),
                Type::Ref,
            );
            match member {
                ExportMember::Method(m) => {
                    let tear_off = self.method_tearoff(instance.clone(), m, span);
                    let signature = self.ctx.outline.functions[m].signature;
                    let f = self.lower_function_to_js(tear_off, Some(signature), false, span);
                    self.call_direct(set_helper, None, vec![literal.clone(), name_op, f]);
                }
                ExportMember::Accessors {
                    getter,
                    setter,
                    library,
                } => {
                    let g = match getter {
                        Some((n, t)) => {
                            let closure =
                                self.export_accessor_closure(instance.clone(), n, library, false);
                            let result = self.callback_result_kind(t);
                            self.wrap_closure(closure, "", result, 0, 0)
                        }
                        None => Operand::Constant(Constant::Null),
                    };
                    let s = match setter {
                        Some((n, t)) => {
                            let closure =
                                self.export_accessor_closure(instance.clone(), n, library, true);
                            let kind = self.callback_kind(t).to_string();
                            self.wrap_closure(closure, &kind, "v", 1, 1)
                        }
                        None => Operand::Constant(Constant::Null),
                    };
                    self.call_direct(accessor_helper, None, vec![literal.clone(), name_op, g, s]);
                }
            }
        }
        literal
    }

    /// `jsFunctionToJS` of a closure with the given description.
    fn wrap_closure(
        &mut self,
        closure: Operand,
        kinds: &str,
        result: &str,
        positional: i64,
        required: i64,
    ) -> Operand {
        let Some(helper) = self.js_helper_function("jsFunctionToJS") else {
            return self.unsupported(
                "dart:_js_helper without jsFunctionToJS",
                Span { start: 0, end: 0 },
            );
        };
        let kinds = self.emit(
            Instruction::Const(Constant::StringWtf8(kinds.as_bytes().to_vec())),
            Type::Ref,
        );
        let result = self.emit(
            Instruction::Const(Constant::StringWtf8(result.as_bytes().to_vec())),
            Type::Ref,
        );
        self.call_direct(
            helper,
            None,
            vec![
                closure,
                kinds,
                result,
                Operand::Constant(Constant::Int(positional)),
                Operand::Constant(Constant::Int(required)),
                Operand::Constant(Constant::Bool(false)),
            ],
        )
    }

    /// The result code of a callback returning `ty` (`jsFunctionToJS`).
    fn callback_result_kind(&self, ty: TypeId) -> &'static str {
        let ty = self.ctx.erase(ty);
        if self.ctx.is_void(ty) {
            "v"
        } else if self.js_value_type(ty).is_some() {
            "r"
        } else {
            "j"
        }
    }

    /// A closure that reads (`() => object.name`) or writes
    /// (`(v) { object.name = v; }`) the member `name` of `object` by its
    /// selector: the getter and setter of an export.
    fn export_accessor_closure(
        &mut self,
        object: Operand,
        name: SymbolId,
        library: LibraryId,
        setter: bool,
    ) -> Operand {
        use super::closures::{DefaultValue, EntryParam};
        use super::sdk_source::{AccessKind, selector_string};
        let name_str = self.ctx.symbol_name(name).trim_end_matches('=').to_string();
        let kind = if setter {
            AccessKind::Write
        } else {
            AccessKind::Read
        };
        let selector = selector_string(self.ctx, kind, &name_str, library);
        let symbol = format!(
            "df.jsexport.{}.{}",
            if setter { "set" } else { "get" },
            crate::context::escape_str(&selector)
        );
        if !self.done_entries.contains(&symbol) {
            self.done_entries.insert(symbol.clone());
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol.clone(), name_str, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Arguments));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            let env = e.closure_context(clo);
            let receiver = e.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
            let params: Vec<EntryParam> = if setter {
                vec![EntryParam {
                    name: None,
                    kind: ast::ParameterKind::Required,
                    required: true,
                    pattern: DefaultValue::Absent,
                }]
            } else {
                Vec::new()
            };
            if let Some(vals) = e.unpack(&params, args, desc) {
                let evaluated: Vec<super::members::Evaluated> =
                    vals.into_iter().map(|v| (None, v)).collect();
                let r = e.call_by_selector(receiver, selector, &evaluated);
                let r = if setter {
                    Operand::Constant(Constant::Null)
                } else {
                    e.coerce(r, Type::Ref)
                };
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let object = self.coerce(object, Type::Ref);
        let env = self.emit(
            Instruction::AllocEnv {
                values: vec![object],
            },
            Type::Ref,
        );
        self.emit(
            Instruction::AllocClosure {
                code_symbol: symbol,
                env,
            },
            Type::Ref,
        )
    }

    /// `v.isA<T>()`, as dart2wasm 3.6 lowers it: `JSAny` is any value but `null`;
    /// `JSNumber`, `JSBoolean`, `JSString`, `JSSymbol` and `JSBigInt` are a
    /// `typeof`; `JSTypedArray`, an `instanceof` of the typed arrays' common
    /// prototype; any other type is an `instanceof` of the constructor its JS
    /// name names (`@JS` or the Dart name, with the library's prefix); a
    /// nullable `T` also accepts `null` (`jsIsA` in `dart:_js_helper`).
    pub(super) fn lower_is_a(
        &mut self,
        value: Operand,
        call: Option<(ast::ExprId, &ast::Arguments)>,
        span: Span,
    ) -> Operand {
        let Some((_, arguments)) = call else {
            return self.unsupported("isA without a call", span);
        };
        let Some(&ty) = self.ctx.bodies.units[self.unit_id.0 as usize]
            .instantiation(arguments.span.start)
            .and_then(|a| a.first())
        else {
            return self.unsupported("isA without its type argument", span);
        };
        let (decl, nullable) = match self.ctx.table.get(ty) {
            Ty::ExtensionType { decl, nullable, .. } => (*decl, *nullable),
            _ => return self.unsupported("isA of a type that is not a JS type", span),
        };
        let Some(core) = core_js_type(self.ctx, decl) else {
            return self.unsupported("isA of a type that is not a JS type", span);
        };
        let core_name = self
            .ctx
            .symbol_name(self.ctx.program.classes[core.0 as usize].name);
        let (mode, name) = match core_name {
            "JSAny" if decl == core => (0, String::new()),
            "JSNumber" if decl == core => (1, "number".to_string()),
            "JSBoolean" if decl == core => (1, "boolean".to_string()),
            "JSString" if decl == core => (1, "string".to_string()),
            "JSSymbol" if decl == core => (1, "symbol".to_string()),
            "JSBigInt" if decl == core => (1, "bigint".to_string()),
            // `TypedArray` is not a global: the prototype the typed arrays share.
            "JSTypedArray" if decl == core => (3, String::new()),
            _ => {
                let class = &self.ctx.program.classes[decl.0 as usize];
                let own = class_metadata(self.ctx, decl)
                    .and_then(|(u, meta)| js_annotation(self.ctx, u, meta));
                (
                    2,
                    top_level_js_string(
                        self.ctx,
                        own,
                        self.ctx.symbol_name(class.name),
                        class.library,
                    ),
                )
            }
        };
        let Some(helper) = self.js_helper_function("jsIsA") else {
            return self.unsupported("dart:_js_helper without jsIsA", span);
        };
        let v = self.coerce(value, Type::Ref);
        let name = self.emit(
            Instruction::Const(Constant::StringWtf8(name.into_bytes())),
            Type::Ref,
        );
        self.call_direct(
            helper,
            None,
            vec![
                v,
                Operand::Constant(Constant::Bool(nullable)),
                Operand::Constant(Constant::Int(mode)),
                name,
            ],
        )
    }

    /// The conversion code of a callback's parameter of type `ty`
    /// (`jsFunctionToJS` in `dart:_js_helper`).
    fn callback_kind(&self, ty: TypeId) -> char {
        let ty = self.ctx.erase(ty);
        if let Some(nullable) = self.js_value_type(ty) {
            return if nullable { 'r' } else { 'R' };
        }
        if let Ty::TypeParameter { param, .. } = self.ctx.table.get(ty) {
            let bound = self.ctx.erase(self.ctx.table.param(*param).bound);
            if self.js_value_type(bound).is_some() {
                return 'r';
            }
        }
        match self.core_class_name(ty) {
            Some(("int", false)) => 'i',
            Some(("int", true)) => 'I',
            Some(("double", false)) => 'd',
            Some(("double", true)) => 'D',
            Some(("num", false)) => 'n',
            Some(("num", true)) => 'N',
            Some(("bool", false)) => 'b',
            Some(("bool", true)) => 'B',
            Some(("String", false)) => 's',
            Some(("String", true)) => 'S',
            _ => 'o',
        }
    }

    /// `f.toJS`: the wrapper of the adapter for `f`, described by its static type
    /// (`jsFunctionToJS`).
    pub(super) fn lower_function_to_js(
        &mut self,
        function: Operand,
        receiver: Option<TypeId>,
        capture_this: bool,
        span: Span,
    ) -> Operand {
        if !crate::target::is_wasm() {
            return self.unsupported("Function.toJS outside the WebAssembly target", span);
        }
        let Some(ty) = receiver.map(|t| self.ctx.erase(t)) else {
            return self.unsupported(
                "Function.toJS without the static type of the function",
                span,
            );
        };
        let Ty::Function {
            ret,
            positional,
            optional,
            named,
            ..
        } = self.ctx.table.get(ty).clone()
        else {
            return self.unsupported("Function.toJS on a value without a function type", span);
        };
        if !named.is_empty() {
            return self.unsupported("Function.toJS of a function with named parameters", span);
        }
        let kinds: String = positional
            .iter()
            .chain(optional.iter())
            .map(|&t| self.callback_kind(t))
            .collect();
        let ret = self.ctx.erase(ret);
        let result = if self.ctx.is_void(ret) {
            "v"
        } else if self.js_value_type(ret).is_some() {
            "r"
        } else {
            "j"
        };
        let Some(helper) = self.js_helper_function("jsFunctionToJS") else {
            return self.unsupported("dart:_js_helper without jsFunctionToJS", span);
        };
        let f = self.coerce(function, Type::Ref);
        let kinds = self.emit(
            Instruction::Const(Constant::StringWtf8(kinds.into_bytes())),
            Type::Ref,
        );
        let result = self.emit(
            Instruction::Const(Constant::StringWtf8(result.as_bytes().to_vec())),
            Type::Ref,
        );
        let total = (positional.len() + optional.len()) as i64;
        self.call_direct(
            helper,
            None,
            vec![
                f,
                kinds,
                result,
                Operand::Constant(Constant::Int(total)),
                Operand::Constant(Constant::Int(positional.len() as i64)),
                Operand::Constant(Constant::Bool(capture_this)),
            ],
        )
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
