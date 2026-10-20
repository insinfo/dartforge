//! `super` (P4): `super.m(…)`, `super.x`, `super.x = v` and `super op e`.
//!
//! The specification (§17.21.3 "Super Invocation") looks the member up starting from
//! the superclass of the **lexically** enclosing class, without dispatch on the
//! dynamic class: the call is direct. When the lookup reaches the SDK
//! (`Object`), the members of `Object` are the runtime's: `toString()` is
//! `Instance of 'C'` and `==` is identity.

use super::fn_builder::FnBuilder;
use super::members::{Evaluated, has_body};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionKind, VariableId};
use dartforge_intern::SymbolId;

/// What `super.name` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperMember {
    Function(usize),
    Field(VariableId),
    /// The lookup reached the SDK.
    Sdk,
    /// `super` inside a `mixin`: the target depends on the application.
    OfMixin,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Looks `name` (or the setter `name=`) up starting from the superclass of the
    /// enclosing class.
    pub fn super_member(&self, name: SymbolId, setter: bool) -> SuperMember {
        let Some(cid) = self.enclosing_class else {
            return SuperMember::Sdk;
        };
        let text = self.ctx.symbol_name(name).to_string();
        let key = if setter {
            self.ctx.interner.lookup(&format!("{text}_="))
        } else {
            Some(name)
        };
        // The superclass of `C` in the linearization is what comes after it (the
        // applied mixins and then the superclass); in a `mixin` the `super`
        // depends on the class that applies it (`SuperMember::OfMixin`).
        if super::members::is_mixin(self.ctx, cid) {
            return SuperMember::OfMixin;
        }
        self.search_after(cid, cid, name, key)
    }

    /// Looks `name` up (`key`: the name or the setter's) in the linearization of
    /// `class`, after `after`.
    fn search_after(
        &self,
        class: dartforge_elements::model::ClassId,
        after: dartforge_elements::model::ClassId,
        name: SymbolId,
        key: Option<SymbolId>,
    ) -> SuperMember {
        let lin = super::members::linearization(self.ctx, class);
        let Some(pos) = lin.iter().position(|c| *c == after) else {
            return SuperMember::Sdk;
        };
        for &c in lin.iter().skip(pos + 1) {
            let cl = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = key.and_then(|k| cl.instance_members.get(&k)) {
                let f = f.0 as usize;
                if let Some(v) = self.ctx.program.functions[f].variable {
                    return SuperMember::Field(v);
                }
                if has_body(self.ctx, f) {
                    return SuperMember::Function(f);
                }
            }
            if let Some(&v) = cl.fields.iter().find(|&&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && var.name == name
            }) {
                return SuperMember::Field(v);
            }
        }
        SuperMember::Sdk
    }

    /// `super.name` inside a `mixin` M: the target is what comes after M in the
    /// linearization of the **dynamic** class of `this` (specification §12.3:
    /// each application of M has its own superclass). The mixin code is
    /// compiled once, so the target is chosen by the class (the same
    /// mechanism as mixin fields): one target per concrete class that
    /// applies M.
    fn mixin_super_targets(&self, name: SymbolId, setter: bool) -> Vec<(i64, SuperMember)> {
        let Some(mixin) = self.enclosing_class else {
            return Vec::new();
        };
        let text = self.ctx.symbol_name(name).to_string();
        let key = if setter {
            self.ctx.interner.lookup(&format!("{text}_="))
        } else {
            Some(name)
        };
        let mut exit = Vec::new();
        for (k, class) in self.ctx.program.classes.iter().enumerate() {
            let kid = dartforge_elements::model::ClassId(k as u32);
            // The classes of the source SDK also apply mixins with `super`
            // (`_ConstMap` with `_ImmutableLinkedHashMapMixin`).
            if !self.ctx.compiled_library(class.library)
                || class.modifiers.abstract_
                || super::members::is_mixin(self.ctx, kid)
            {
                continue;
            }
            let Some(id) = self.ctx.class_id_of(kid) else {
                continue;
            };
            if !super::members::linearization(self.ctx, kid).contains(&mixin) {
                continue;
            }
            exit.push((i64::from(id), self.search_after(kid, mixin, name, key)));
        }
        exit
    }

    fn this_for_super(&mut self, span: Span) -> Option<Operand> {
        let t = self.this_param.clone();
        if t.is_none() {
            self.unsupported("`super` outside an instance member", span);
        }
        t
    }

    /// Performs `use_site` with the target of `super.name`: directly, or — inside a
    /// mixin — by the dynamic class of `this` (`switch`), with the `Ref`
    /// result at a join.
    fn with_super_target(
        &mut self,
        this: Operand,
        name: SymbolId,
        setter: bool,
        span: Span,
        use_site: &mut dyn FnMut(&mut Self, SuperMember, Operand) -> Operand,
    ) -> Operand {
        let target = self.super_member(name, setter);
        if target != SuperMember::OfMixin {
            return use_site(self, target, this);
        }
        let targets = self.mixin_super_targets(name, setter);
        if targets.is_empty() {
            return self.unsupported("`super` inside a mixin without a class applying it", span);
        }
        let this = self.coerce(this, Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(this.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let mut distinct: Vec<SuperMember> = Vec::new();
        for (_, a) in &targets {
            if !distinct.contains(a) {
                distinct.push(*a);
            }
        }
        let blocks: Vec<(SuperMember, BlockId)> =
            distinct.iter().map(|a| (*a, self.new_block())).collect();
        let block_of = |a: SuperMember| {
            blocks
                .iter()
                .find(|(x, _)| *x == a)
                .map(|(_, b)| *b)
                .expect("block")
        };
        let join = self.new_block();
        self.terminate(Terminator::Switch {
            val: cls,
            default: blocks[0].1,
            cases: targets.iter().map(|(id, a)| (*id, block_of(*a))).collect(),
        });
        let mut entries = Vec::new();
        for (a, b) in blocks.clone() {
            self.set_block(b);
            let r = use_site(self, a, this.clone());
            let r = if matches!(self.operand_type(&r), Type::Void | Type::Ptr) {
                Operand::Constant(Constant::Null)
            } else {
                self.coerce(r, Type::Ref)
            };
            if !self.is_terminated() {
                entries.push((self.current_block, r));
                self.terminate(Terminator::Branch(join));
            }
        }
        self.set_block(join);
        if entries.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        self.emit(
            Instruction::Phi {
                incoming: entries,
                ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// `super.name(args)`.
    pub fn call_super_method(
        &mut self,
        name: SymbolId,
        evaluated: &[Evaluated],
        span: Span,
    ) -> Operand {
        let Some(this) = self.this_for_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        self.with_super_target(
            this,
            name,
            false,
            span,
            &mut |s, target, this| match target {
                SuperMember::Function(f) => {
                    if s.ctx.program.functions[f].kind == FunctionKind::Getter {
                        let v = s.call_direct(f, Some(this), Vec::new());
                        return s.call_function_value(v, evaluated);
                    }
                    let args = s.match_args(f, evaluated);
                    s.call_direct(f, Some(this), args)
                }
                SuperMember::Field(v) => {
                    let x = s.read_field_with_late_direct(this, v, span);
                    s.call_function_value(x, evaluated)
                }
                SuperMember::OfMixin => s.unsupported("`super` inside a mixin", span),
                SuperMember::Sdk => match s.ctx.symbol_name(name) {
                    "toString" => s.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_to_string_handle".to_string(),
                            args: vec![(this, Type::Ref)],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    "noSuchMethod" => s.throw_nsm("noSuchMethod"),
                    other => {
                        let n = other.to_string();
                        s.unsupported(&format!("SDK `super.{n}`"), span)
                    }
                },
            },
        )
    }

    /// `super.name` lido.
    pub fn read_super(&mut self, name: SymbolId, span: Span) -> Operand {
        let Some(this) = self.this_for_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        self.with_super_target(
            this,
            name,
            false,
            span,
            &mut |s, target, this| match target {
                SuperMember::Function(f) => {
                    if s.ctx.program.functions[f].kind == FunctionKind::Getter {
                        s.call_direct(f, Some(this), Vec::new())
                    } else {
                        // Tear-off of `super.m`: binds to the superclass method.
                        s.method_tearoff(this, f, span)
                    }
                }
                SuperMember::Field(v) => s.read_field_with_late_direct(this, v, span),
                SuperMember::OfMixin => s.unsupported("`super` inside a mixin", span),
                SuperMember::Sdk => {
                    let n = s.ctx.symbol_name(name).to_string();
                    s.unsupported(&format!("SDK `super.{n}`"), span)
                }
            },
        )
    }

    /// `super.name = v`.
    pub fn store_super(&mut self, name: SymbolId, value: Operand, span: Span) -> Operand {
        let Some(this) = self.this_for_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        let v2 = value.clone();
        self.with_super_target(
            this,
            name,
            true,
            span,
            &mut |s, target, this| match target {
                SuperMember::Function(f) => {
                    let args = s.match_args(f, &[(None, v2.clone())]);
                    s.call_direct(f, Some(this), args);
                    v2.clone()
                }
                SuperMember::Field(v) => {
                    s.store_field(this, v, v2.clone(), span);
                    v2.clone()
                }
                SuperMember::OfMixin => s.unsupported("`super` inside a mixin", span),
                SuperMember::Sdk => {
                    let n = s.ctx.symbol_name(name).to_string();
                    s.unsupported(&format!("SDK `super.{n} =`"), span)
                }
            },
        );
        value
    }

    /// `super op e` (`super == o`, `super + x`).
    pub fn super_operator(&mut self, op_name_str: &str, right: Operand, span: Span) -> Operand {
        let Some(this) = self.this_for_super(span) else {
            return Operand::Constant(Constant::Null);
        };
        let Some(sym) = self.ctx.interner.lookup(op_name_str) else {
            return self.unsupported(&format!("`super {op_name_str}`"), span);
        };
        let equality = op_name_str == "==";
        let op_name = op_name_str.to_string();
        self.with_super_target(
            this,
            sym,
            false,
            span,
            &mut |s, target, this| match target {
                SuperMember::Function(f) => {
                    let args = s.match_args(f, &[(None, right.clone())]);
                    s.call_direct(f, Some(this), args)
                }
                _ if equality => s.identical(this, right.clone()),
                _ => s.unsupported(&format!("`super {op_name}`"), span),
            },
        )
    }
}
