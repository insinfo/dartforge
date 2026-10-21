//! The lowering of the JavaScript interop of WebAssembly modules
//! (`crate::js_interop`): a call to an SDK `external` with
//! `@pragma('dartforge:js', '<code>')` becomes the call to its snippet, with each
//! value converted by its declared Dart type; the interop members of user code
//! (`lower/js_members.rs`) come through the same call.
//!
//! | declared type | crosses as | conversion |
//! |---|---|---|
//! | `JSValue`, a JS type (erased to it) | [`Kind::Value`] | the handle of `_ref`; `null` is 0 |
//! | `double` | [`Kind::Number`] | none |
//! | `int` | [`Kind::Integer`] | to the JavaScript number; returned, `ToInt32` |
//! | `bool` | [`Kind::Boolean`] | 0/1 |
//! | `String`, `String?` | [`Kind::Text`] | a new JavaScript string; `null` is 0 |
//! | `void` (return) | [`Kind::Void`] | — |
//!
//! A returned value comes back as a new `JSValue` (`JSValue.box` when the
//! declared type is nullable: handle 0 is Dart `null`; `JSValue(h)` otherwise,
//! as dart2wasm's `JSValue(ref)`), a Dart string, a `double` or a `bool`. The
//! conversions of the arguments allocate nothing on the Dart heap between the
//! reading of each handle and the call, so no collection releases a handle the
//! call is about to read.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use crate::js_interop::{Kind, Snippet};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionElementId, VariableId};
use dartforge_types::table::{Type as Ty, TypeId};

/// `JSValue` and the functions of `dart:_js_helper` the lowering calls.
#[derive(Clone, Debug)]
pub struct JsHelpers {
    pub class: ClassId,
    /// `final int _ref`.
    pub handle: VariableId,
    /// `JSValue(int)`: a value that owns the handle.
    pub constructor: FunctionElementId,
    /// `static JSValue? box(int)`: `null` for handle 0.
    pub boxer: FunctionElementId,
    /// `static int unbox(JSValue?)`: 0 for `null`.
    pub unboxer: FunctionElementId,
    /// The library's top-level functions by name (the conversions of the
    /// interop members, `jsifyRaw`…).
    pub functions: std::collections::HashMap<String, usize>,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The helpers, borrowed from the context (not from the builder).
    pub(super) fn js_helpers(&self) -> Option<&'c JsHelpers> {
        let ctx: &'c crate::context::Context<'a> = self.ctx;
        ctx.js_value
            .get_or_init(|| {
                let class = ctx.sdk_class("_js_helper", "JSValue")?;
                let c = &ctx.program.classes[class.0 as usize];
                let symbol = |name: &str| ctx.interner.lookup(name);
                let handle =
                    c.fields.iter().copied().find(|&v| {
                        Some(ctx.program.variables[v.0 as usize].name) == symbol("_ref")
                    })?;
                let functions = ctx
                    .program
                    .functions
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| {
                        f.library == c.library && f.class.is_none() && f.extension.is_none()
                    })
                    .map(|(i, f)| (ctx.symbol_name(f.name).to_string(), i))
                    .collect();
                Some(JsHelpers {
                    class,
                    handle,
                    constructor: *c.constructors.get(&symbol("")?)?,
                    boxer: *c.static_members.get(&symbol("box")?)?,
                    unboxer: *c.static_members.get(&symbol("unbox")?)?,
                    functions,
                })
            })
            .as_ref()
    }

    /// The top-level function `name` of `dart:_js_helper`.
    pub(super) fn js_helper_function(&self, name: &str) -> Option<usize> {
        self.js_helpers()?.functions.get(name).copied()
    }

    /// The kind of a declared type, and whether it is nullable; `None`: the type
    /// does not cross.
    fn snippet_kind(&self, ty: TypeId, js_value: ClassId) -> Option<(Kind, bool)> {
        let ty = self.ctx.erase(ty);
        if self.ctx.is_void(ty) {
            return Some((Kind::Void, false));
        }
        let Ty::Interface {
            class, nullable, ..
        } = self.ctx.table.get(ty)
        else {
            return None;
        };
        if *class == js_value {
            return Some((Kind::Value, *nullable));
        }
        let c = &self.ctx.program.classes[class.0 as usize];
        if self.ctx.program.library(c.library).uri != "dart:core" {
            return None;
        }
        let kind = match self.ctx.symbol_name(c.name) {
            "String" => return Some((Kind::Text, *nullable)),
            "double" => Kind::Number,
            "int" => Kind::Integer,
            "bool" => Kind::Boolean,
            _ => return None,
        };
        (!nullable).then_some((kind, false))
    }

    /// The call to the snippet `code` of the `external` `fid`, with the
    /// arguments already in the representation of its parameters.
    pub(super) fn call_snippet(&mut self, fid: usize, code: &str, args: &[Operand]) -> Operand {
        let span = Span { start: 0, end: 0 };
        let member = self
            .ctx
            .symbol_name(self.ctx.program.functions[fid].name)
            .to_string();
        if !crate::target::is_wasm() {
            return self.unsupported(
                &format!("JavaScript snippet `{member}` outside the WebAssembly target"),
                span,
            );
        }
        let Some(js) = self.js_helpers() else {
            return self.unsupported("dart:_js_helper without JSValue", span);
        };
        let Some(data) = self.ctx.outline.functions.get(fid) else {
            return self.unsupported(&format!("snippet `{member}` without a signature"), span);
        };
        let declared: Vec<TypeId> = data.parameters.iter().map(|p| p.ty).collect();
        let return_type = data.return_type;
        if self.ctx.program.functions[fid].class.is_some()
            && !self.ctx.program.functions[fid].static_
        {
            return self.unsupported(&format!("snippet `{member}` is an instance member"), span);
        }
        let mut values = Vec::with_capacity(declared.len());
        for (i, &ty) in declared.iter().enumerate() {
            match self.snippet_kind(ty, js.class) {
                Some((Kind::Void, _)) | None => {
                    return self.unsupported(
                        &format!("snippet `{member}`: parameter {i} of a type that does not cross"),
                        span,
                    );
                }
                Some((kind, nullable)) => {
                    let Some(arg) = args.get(i) else {
                        return self
                            .unsupported(&format!("snippet `{member}`: argument {i}"), span);
                    };
                    let repr = match kind {
                        Kind::Number => Type::F64,
                        Kind::Integer => Type::I64,
                        Kind::Boolean => Type::I1,
                        _ => Type::Ref,
                    };
                    values.push((self.coerce(arg.clone(), repr), kind, nullable));
                }
            }
        }
        let Some((ret, ret_nullable)) = self.snippet_kind(return_type, js.class) else {
            return self.unsupported(
                &format!("snippet `{member}`: a return type that does not cross"),
                span,
            );
        };
        let result = self.emit_snippet(code, values, ret, js);
        match ret {
            Kind::Text => self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_js_string_to_dart".to_string(),
                    args: vec![(result, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            Kind::Value if ret_nullable => {
                self.call_direct(js.boxer.0 as usize, None, vec![result])
            }
            Kind::Value => self.instantiate_evaluated(js.constructor, &[(None, result)], span),
            _ => result,
        }
    }

    /// The call to `code` with `values` (operand, kind, nullable), already
    /// converted on the Dart side: each handle read (`_ref`, `unbox` for a
    /// nullable value), each text made a JavaScript string, then the call. The
    /// result is raw: the handle (`i64`) of a value or text, the `double`, the
    /// `int`, the `bool` (`i1`), or null for `void`.
    pub(super) fn emit_snippet(
        &mut self,
        code: &str,
        values: Vec<(Operand, Kind, bool)>,
        ret: Kind,
        js: &JsHelpers,
    ) -> Operand {
        let span = Span { start: 0, end: 0 };
        let mut crossing = Vec::with_capacity(values.len());
        for (value, kind, nullable) in &values {
            let c = match kind {
                Kind::Value => match value {
                    Operand::Constant(Constant::Null) => {
                        (Operand::Constant(Constant::Int(0)), Type::I64)
                    }
                    _ if *nullable => {
                        let h = self.call_direct(js.unboxer.0 as usize, None, vec![value.clone()]);
                        (self.coerce(h, Type::I64), Type::I64)
                    }
                    _ => {
                        let h = self.read_field(value.clone(), js.handle, span);
                        (self.coerce(h, Type::I64), Type::I64)
                    }
                },
                Kind::Boolean | Kind::JsBoolean => {
                    let b = self.coerce(value.clone(), Type::I1);
                    let b = self.emit(
                        Instruction::ZExt {
                            op: b,
                            from: Type::I1,
                            to: Type::I8,
                        },
                        Type::I8,
                    );
                    (b, Type::I8)
                }
                Kind::Number => (self.coerce(value.clone(), Type::F64), Type::F64),
                Kind::Integer => (self.coerce(value.clone(), Type::I64), Type::I64),
                // Converted below, after every handle is read.
                Kind::Text => (value.clone(), Type::Ref),
                Kind::Void => unreachable!("void does not cross as an argument"),
            };
            crossing.push(c);
        }
        for (c, (_, kind, _)) in crossing.iter_mut().zip(&values) {
            if *kind == Kind::Text {
                let text = c.0.clone();
                *c = (
                    self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_js_string_to_js".to_string(),
                            args: vec![(text, Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    Type::I64,
                );
            }
        }
        let snippet = Snippet {
            params: values.iter().map(|(_, k, _)| *k).collect(),
            ret,
            code: code.to_string(),
        };
        let ret_ty = match ret {
            Kind::Value | Kind::Text | Kind::Integer => Type::I64,
            Kind::Number => Type::F64,
            Kind::Boolean | Kind::JsBoolean => Type::I8,
            Kind::Void => Type::Void,
        };
        let result = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: snippet.symbol(),
                args: crossing,
                ret_ty,
            },
            ret_ty,
        );
        match ret {
            Kind::Void => Operand::Constant(Constant::Null),
            Kind::Boolean | Kind::JsBoolean => self.emit(
                Instruction::ICmp(ICmpOp::Ne, result, Operand::Constant(Constant::Int(0))),
                Type::I1,
            ),
            _ => result,
        }
    }
}
