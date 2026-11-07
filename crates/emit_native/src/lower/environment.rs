//! Native compilation environment. This is independent of the process environment
//! and of analyzer constant evaluation, which intentionally keeps queries unknown.

use super::fn_builder::FnBuilder;
use crate::hir::{Constant, Instruction, Operand, Terminator, Type};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionElementId, UnitId, VariableRef};
use dartforge_frontend::ast::{self, ExprKind};
use dartforge_types::resolved::{MemberRef, Resolved};

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The public core environment factory, including its SDK patch implementation.
    pub(super) fn environment_factory(
        &self,
        fid: FunctionElementId,
    ) -> Option<(&'static str, bool)> {
        let f = self.ctx.program.function(self.ctx.program.public(fid));
        if !f.const_ || Some(f.library) != self.ctx.core.core_library {
            return None;
        }
        let class = f.class?;
        let name = self.ctx.symbol_name(f.name);
        let kind = if Some(class) == self.ctx.core.bool_class {
            "bool"
        } else if Some(class) == self.ctx.core.int_class {
            "int"
        } else if Some(class) == self.ctx.core.string_class {
            "String"
        } else {
            return None;
        };
        match name {
            "fromEnvironment" => Some((kind, false)),
            "hasEnvironment" if kind == "bool" => Some((kind, true)),
            _ => None,
        }
    }

    /// The definitions: `dartforge.compiler` (true); `dart.library.<name>` of
    /// the destination's SDK layout (declared as `true` only when supported;
    /// `dart:mirrors` only outside the product profile, as the VM's AOT does
    /// not have it); and the VM's profile
    /// keys — `dart.vm.product` (`Context::product`: true in
    /// `DartFlags::PRODUCTION`, as in `dart compile exe`; false in development,
    /// as in `dart run`) and, outside WebAssembly, `dart.isVM` (true).
    /// `dart.vm.profile` and unavailable libraries are absent; their queries
    /// retain defaults and `hasEnvironment` is false. An integer query
    /// cannot parse `true`/`false` and therefore uses its default; other keys
    /// remain undefined.
    pub(super) fn environment_value(
        &self,
        unit: UnitId,
        key: ast::ExprId,
        kind: &str,
        has: bool,
    ) -> Option<Constant> {
        let key = self.environment_key(unit, key, 0)?;
        if let Some(environment) = &self.ctx.program.compilation_environment {
            return if has {
                Some(Constant::Bool(environment.get(&key).is_some()))
            } else {
                match kind {
                    "bool" => environment.boolean(&key).map(Constant::Bool),
                    "int" => environment
                        .integer(&key)
                        .and_then(|value| value.exact_integer())
                        .map(Constant::Int),
                    _ => environment
                        .get(&key)
                        .map(|value| Constant::String(value.into())),
                }
            };
        }

        let value = match key.as_str() {
            "dartforge.compiler" => true,
            library if library.starts_with("dart.library.") => {
                let name = &library["dart.library.".len()..];
                match self.ctx.library_support.get(name) {
                    Some(true) if !(name == "mirrors" && self.ctx.product) => true,
                    _ if has => return Some(Constant::Bool(false)),
                    _ => return None,
                }
            }
            "dart.vm.product" => self.ctx.product,
            // The VM defines it in JIT and AOT; dart2wasm does not.
            "dart.isVM" if !crate::target::is_wasm() => true,
            _ => {
                return if has {
                    Some(Constant::Bool(false))
                } else {
                    None
                };
            }
        };
        if has {
            return Some(Constant::Bool(true));
        }
        match kind {
            "bool" => Some(Constant::Bool(value)),
            "String" => Some(Constant::String(
                if value { "true" } else { "false" }.into(),
            )),
            _ => None,
        }
    }

    /// Lowers native constant queries and ordinary lookups from one immutable
    /// context. Lookup tables live in generated code, not process-global state.
    pub(super) fn native_environment_constructor(
        &mut self,
        ast: &ast::Ast,
        fid: FunctionElementId,
        args: &[ast::Argument],
        span: Span,
    ) -> Option<Operand> {
        let (kind, has) = self.environment_factory(fid)?;
        let environment = self.ctx.program.compilation_environment.as_ref()?.clone();
        let key_expression = args.iter().find(|argument| argument.name.is_none())?.value;
        let known = self.known_environment_name(self.unit_id, key_expression);
        if self.in_const_context && known.is_none() {
            return Some(self.unsupported("constant compilation environment name", span));
        }
        // CFE hoists a named value preceding the positional name, but applies
        // its parameter cast at the call, after evaluating/checking that name.
        let prior_types = self.arm_constructor_types(fid.0 as usize, None, args);
        let types = self.arg_types.take().map(|(_, types)| types);
        let positional_index = args.iter().position(|argument| argument.name.is_none())?;
        let mut evaluated = Vec::new();
        let mut pending = Vec::new();
        for (index, argument) in args.iter().enumerate() {
            let value = self.lower_expr(ast, argument.value);
            let name = argument.name.map(|name| name.sym);
            if name.is_some() && index < positional_index {
                pending.push((argument.value, name, value.clone()));
            } else {
                self.implicit_argument_cast(types.as_ref(), argument.value, name, 0, &value);
                if name.is_none() {
                    for (expression, name, value) in pending.drain(..) {
                        self.implicit_argument_cast(types.as_ref(), expression, name, 0, &value);
                    }
                }
            }
            evaluated.push((name, value));
        }
        self.arg_types = prior_types;
        let ty = match kind {
            "bool" => Type::I1,
            "int" => Type::I64,
            _ => Type::Ref,
        };
        let default = if has {
            self.emit(Instruction::Const(Constant::Bool(false)), Type::I1)
        } else if let Some((_, value)) = evaluated.iter().find(|(name, _)| name.is_some()) {
            self.coerce(value.clone(), ty)
        } else {
            self.emit(
                Instruction::Const(match kind {
                    "bool" => Constant::Bool(false),
                    "int" => Constant::Int(0),
                    _ => Constant::String(String::new()),
                }),
                ty,
            )
        };
        let lookup = |name: &str| -> Option<Constant> {
            let value = if self.in_const_context {
                environment.get(name)
            } else {
                environment.runtime_get(name)
            };
            if has {
                return Some(Constant::Bool(value.is_some()));
            }
            match kind {
                "bool" => match value? {
                    "true" => Some(Constant::Bool(true)),
                    "false" => Some(Constant::Bool(false)),
                    _ => None,
                },
                "int" => (if self.in_const_context {
                    environment.integer(name)
                } else {
                    environment.runtime_integer(name)
                })
                .and_then(|value| value.exact_integer())
                .map(Constant::Int),
                _ => value.map(|value| Constant::String(value.into())),
            }
        };
        if let Some(known) = known {
            let constant = known.as_deref().and_then(lookup);
            return Some(match constant {
                Some(constant) => self.emit(Instruction::Const(constant), ty),
                None => default,
            });
        }
        let entries: Vec<_> = environment
            .runtime_entries()
            .filter_map(|(name, _)| lookup(name).map(|value| (name.to_string(), value)))
            .collect();
        let key = self.coerce(
            evaluated.iter().find(|(name, _)| name.is_none())?.1.clone(),
            Type::Ref,
        );
        Some(self.emit_environment_lookup(key, default, entries, ty))
    }

    /// Constructor tear-offs carry already checked values rather than an AST.
    pub(super) fn evaluated_environment_constructor(
        &mut self,
        fid: FunctionElementId,
        evaluated: &[super::members::Evaluated],
    ) -> Option<Operand> {
        let (kind, has) = self.environment_factory(fid)?;
        let environment = self.ctx.program.compilation_environment.as_ref()?.clone();
        let ty = match kind {
            "bool" => Type::I1,
            "int" => Type::I64,
            _ => Type::Ref,
        };
        let key = self.coerce(
            evaluated.iter().find(|(name, _)| name.is_none())?.1.clone(),
            Type::Ref,
        );
        let default =
            if !has && let Some((_, value)) = evaluated.iter().find(|(name, _)| name.is_some()) {
                self.coerce(value.clone(), ty)
            } else {
                self.emit(
                    Instruction::Const(match kind {
                        "bool" => Constant::Bool(false),
                        "int" => Constant::Int(0),
                        _ => Constant::String(String::new()),
                    }),
                    ty,
                )
            };
        let declarations: Vec<_> = if self.in_const_context {
            environment.entries().collect()
        } else {
            environment.runtime_entries().collect()
        };
        let entries = declarations
            .into_iter()
            .filter_map(|(name, raw)| {
                let value = if has {
                    Some(Constant::Bool(true))
                } else {
                    match kind {
                        "bool" => match raw {
                            "true" => Some(Constant::Bool(true)),
                            "false" => Some(Constant::Bool(false)),
                            _ => None,
                        },
                        "int" => (if self.in_const_context {
                            environment.integer(name)
                        } else {
                            environment.runtime_integer(name)
                        })
                        .and_then(|value| value.exact_integer())
                        .map(Constant::Int),
                        _ => Some(Constant::String(raw.into())),
                    }
                };
                value.map(|value| (name.to_string(), value))
            })
            .collect();
        Some(self.emit_environment_lookup(key, default, entries, ty))
    }

    fn emit_environment_lookup(
        &mut self,
        key: Operand,
        default: Operand,
        entries: Vec<(String, Constant)>,
        ty: Type,
    ) -> Operand {
        let end = self.new_block();
        let mut incoming = Vec::new();
        for (name, value) in entries {
            let found = self.new_block();
            let next = self.new_block();
            let name = self.emit(Instruction::Const(Constant::String(name)), Type::Ref);
            let condition = self.string_equality(key.clone(), name);
            self.terminate(Terminator::CondBranch {
                cond: condition,
                then_block: found,
                else_block: next,
            });
            self.set_block(found);
            let value = self.emit(Instruction::Const(value), ty);
            incoming.push((self.current_block, value));
            self.terminate(Terminator::Branch(end));
            self.set_block(next);
        }
        incoming.push((self.current_block, default));
        self.terminate(Terminator::Branch(end));
        self.set_block(end);
        self.emit(Instruction::Phi { incoming, ty }, ty)
    }

    fn known_environment_name(
        &self,
        unit: UnitId,
        expression: ast::ExprId,
    ) -> Option<Option<String>> {
        if let Some(name) = self.environment_key(unit, expression, 0) {
            return Some(Some(name));
        }
        use dartforge_types::const_eval::evaluation::{Ctx, Engine};
        let mut table = self.ctx.table.clone();
        let inferred = self
            .ctx
            .compiled
            .iter()
            .enumerate()
            .filter_map(|(index, compiled)| {
                compiled.then_some(dartforge_elements::model::LibraryId(index as u32))
            })
            .collect();
        let mut engine = Engine::new(
            self.ctx.program,
            self.ctx.interner,
            &mut table,
            self.ctx.core,
            self.ctx.outline,
            self.ctx.bodies,
            &inferred,
        );
        engine.conservative_string_interpolation = true;
        let value = engine.evaluate(
            &Ctx::simple(unit, self.ctx.program.unit(unit).library),
            expression,
            true,
        );
        Some(String::from_utf16(value.value()?.as_text()?).ok())
    }

    pub(super) fn environment_key(
        &self,
        unit: UnitId,
        e: ast::ExprId,
        depth: u32,
    ) -> Option<String> {
        if depth > 32 {
            return None;
        }
        let ast = &self.ctx.program.unit(unit).ast;
        match &ast.expr(e).kind {
            ExprKind::String(s) => {
                if let Some(value) = s.constant_value() {
                    return Some(value.as_str()?.to_string());
                }
                let mut text = String::new();
                for part in &s.parts {
                    match part {
                        ast::StringPart::Text(value) => text.push_str(value.as_str()?),
                        ast::StringPart::Interpolation(value) => {
                            if let Some(value) = self.environment_key(unit, *value, depth + 1) {
                                text.push_str(&value);
                            } else {
                                let value = self.environment_boolean(unit, *value, depth + 1)?;
                                text.push_str(if value { "true" } else { "false" });
                            }
                        }
                    }
                }
                Some(text)
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                let branch = if self.environment_boolean(unit, *condition, depth + 1)? {
                    then
                } else {
                    else_
                };
                self.environment_key(unit, *branch, depth + 1)
            }
            ExprKind::Parenthesized(x) => self.environment_key(unit, *x, depth + 1),
            ExprKind::Binary {
                op: ast::BinaryOp::Add,
                left,
                right,
            } => Some(
                self.environment_key(unit, *left, depth + 1)?
                    + &self.environment_key(unit, *right, depth + 1)?,
            ),
            ExprKind::Identifier(n)
                if unit == self.unit_id && self.local_const_keys.contains_key(&n.sym) =>
            {
                let (_, init) = self.local_const_keys.get(&n.sym)?;
                self.environment_key(unit, *init, depth + 1)
            }
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let vid = match self.ctx.get_resolved(unit, e)? {
                    Resolved::Element(Element::Variable(v))
                    | Resolved::Member {
                        member: MemberRef::Variable(v),
                        ..
                    } => *v,
                    Resolved::Element(Element::Function(f))
                    | Resolved::Member {
                        member: MemberRef::Function(f),
                        ..
                    } => self.ctx.program.function(*f).variable?,
                    _ => return None,
                };
                let var = self.ctx.program.variable(vid);
                if !var.const_ {
                    return None;
                }
                let other = match var.node {
                    VariableRef::Field { unit, .. } | VariableRef::TopLevel { unit, .. } => unit,
                    _ => return None,
                };
                self.environment_key(other, self.variable_initializer_in(vid)?, depth + 1)
            }
            _ => None,
        }
    }

    fn environment_boolean(&self, unit: UnitId, e: ast::ExprId, depth: u32) -> Option<bool> {
        if depth > 32 {
            return None;
        }
        let ast = &self.ctx.program.unit(unit).ast;
        match &ast.expr(e).kind {
            ExprKind::Bool(value) => Some(*value),
            ExprKind::Parenthesized(inner) => self.environment_boolean(unit, *inner, depth + 1),
            ExprKind::Unary {
                op: ast::UnaryOp::Not,
                operand,
            } => self
                .environment_boolean(unit, *operand, depth + 1)
                .map(|value| !value),
            ExprKind::Identifier(n)
                if unit == self.unit_id && self.local_const_keys.contains_key(&n.sym) =>
            {
                let (_, init) = self.local_const_keys.get(&n.sym)?;
                self.environment_boolean(unit, *init, depth + 1)
            }
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => {
                let Resolved::Constructor(fid) = self.ctx.get_resolved(unit, e)? else {
                    return None;
                };
                let (kind, has) = self.environment_factory(*fid)?;
                if kind != "bool" {
                    return None;
                }
                let key = arguments.args.iter().find(|a| a.name.is_none())?.value;
                self.environment_key(unit, key, depth + 1)?;
                match self.environment_value(unit, key, kind, has) {
                    Some(Constant::Bool(value)) => Some(value),
                    _ => arguments
                        .args
                        .iter()
                        .find(|a| {
                            a.name
                                .is_some_and(|n| self.ctx.symbol_name(n.sym) == "defaultValue")
                        })
                        .map_or(Some(false), |a| {
                            self.environment_boolean(unit, a.value, depth + 1)
                        }),
                }
            }
            ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                let vid = match self.ctx.get_resolved(unit, e)? {
                    Resolved::Element(Element::Variable(v))
                    | Resolved::Member {
                        member: MemberRef::Variable(v),
                        ..
                    } => *v,
                    Resolved::Element(Element::Function(f))
                    | Resolved::Member {
                        member: MemberRef::Function(f),
                        ..
                    } => self.ctx.program.function(*f).variable?,
                    _ => return None,
                };
                let var = self.ctx.program.variable(vid);
                if !var.const_ {
                    return None;
                }
                let other = match var.node {
                    VariableRef::Field { unit, .. } | VariableRef::TopLevel { unit, .. } => unit,
                    _ => return None,
                };
                self.environment_boolean(other, self.variable_initializer_in(vid)?, depth + 1)
            }
            _ => None,
        }
    }
}
