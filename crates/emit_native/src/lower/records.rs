//! Records (P3; unified space, P2).
//!
//! A positional-only record is the runtime's `_Record` (fixed cid 12, `REFS` body:
//! word 0 is the number of fields and the following ones are the fields, all `Ref`;
//! docs/NATIVO-ESPACO-UNIFICADO.md §2.5). The
//! runtime does not have the shape (the names) of a record with a named field, so
//! each **shape** of the program — number of positionals and sorted names,
//! collected from every literal, pattern and type before the lowering
//! (`Context::record_shapes`) — is a "class" of the program: an object with
//! the positional fields and then the named ones (in name order), all
//! kept as references, with id `SHAPE_BASE_ID + k`. `toString()` and
//! `==` are generated (specification §20.6 "Records": structural equality
//! field by field, same shape), and the access `r.x`/`r.$1` without a static type
//! tests the shapes that have the field.

use super::fn_builder::FnBuilder;
use crate::context::SHAPE_BASE_ID;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, ExprId};

/// Symbol of the structural equality of records with a shape.
pub const EQUAL_SYMBOL: &str = "df.$registro.igual";

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `(e1, …, name: e)` with some named field.
    pub fn lower_named_record(
        &mut self,
        ast: &ast::Ast,
        positional: &[ExprId],
        named: &[(ast::Name, ExprId)],
        span: Span,
    ) -> Operand {
        // The evaluation order is that of the text.
        let mut items: Vec<(u32, Option<String>, ExprId)> = positional
            .iter()
            .map(|&e| (ast.expr(e).span.start as u32, None, e))
            .collect();
        for (n, e) in named {
            items.push((
                ast.expr(*e).span.start as u32,
                Some(self.ctx.symbol_name(n.sym).to_string()),
                *e,
            ));
        }
        items.sort_by_key(|(s, _, _)| *s);
        let mut names: Vec<String> = named.iter().map(|(n, _)| self.ctx.symbol_name(n.sym).to_string()).collect();
        names.sort();
        let Some(id) = self.ctx.shape_id(positional.len(), &names) else {
            return self.unsupported("record com forma desconhecida", span);
        };
        let mut values = Vec::with_capacity(items.len());
        let mut k_pos = 0usize;
        for (_, name, e) in items {
            let v = self.lower_expr(ast, e);
            let v = self.coerce(v, Type::Ref);
            let idx = match name {
                None => {
                    let i = k_pos;
                    k_pos += 1;
                    i
                }
                Some(n) => positional.len() + names.iter().position(|x| *x == n).expect("nome da forma"),
            };
            values.push((idx, v));
        }
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(i64::from(id))), Type::I64),
                    (Operand::Constant(Constant::Int((positional.len() + names.len()) as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        for (idx, v) in values {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_object_set".to_string(),
                    args: vec![
                        (obj.clone(), Type::Ref),
                        (Operand::Constant(Constant::Int(idx as i64)), Type::I64),
                        (v, Type::I64),
                        (Operand::Constant(Constant::Int(1)), Type::I8),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
        let names = self.emit(Instruction::Const(Constant::String(names.join(","))), Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_registro_nomeado".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(positional.len() as i64)), Type::I64),
                    (names, Type::Ref),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        obj
    }

    /// `(e1, …, ek)` positional only: a `_Record` (`AllocRecord`) with the
    /// fields boxed here, in HIR `Box` instructions (the emitter writes
    /// the words without allocating in between).
    pub fn lower_positional_record(&mut self, ast: &ast::Ast, positional: &[ExprId]) -> Operand {
        let mut ops = Vec::with_capacity(positional.len());
        for &p in positional {
            let v = self.lower_expr(ast, p);
            let v = self.coerce(v, Type::Ref);
            ops.push((v, 3));
        }
        self.emit(Instruction::AllocRecord { elements: ops }, Type::Ref)
    }

    /// The shapes with field `name` (`$k` or named): (id, index).
    pub fn shapes_with_field(&self, name: &str) -> Vec<(i64, usize)> {
        let positional = name.strip_prefix('$').and_then(|d| d.parse::<usize>().ok()).filter(|k| *k >= 1);
        let mut exit = Vec::new();
        for (k, (npos, names)) in self.ctx.record_shapes.iter().enumerate() {
            let id = i64::from(SHAPE_BASE_ID) + k as i64;
            match positional {
                Some(p) if p <= *npos => exit.push((id, p - 1)),
                Some(_) => {}
                None => {
                    if let Some(j) = names.iter().position(|n| n == name) {
                        exit.push((id, npos + j));
                    }
                }
            }
        }
        exit
    }

    /// Reads field `i` (a reference) of a record with a shape.
    pub fn shape_field(&mut self, obj: Operand, i: usize) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (Operand::Constant(Constant::Int(i as i64)), Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// `r.x`/`r.$k` by the dynamic class of the receiver (what is not a record goes
    /// to `pattern`); `None` when no shape has the field and it is not
    /// positional.
    pub fn read_record_field(
        &mut self,
        recv: Operand,
        name: &str,
        pattern: &mut dyn FnMut(&mut Self) -> Operand,
    ) -> Option<Operand> {
        let shapes = self.shapes_with_field(name);
        let positional = name.strip_prefix('$').and_then(|d| d.parse::<i64>().ok()).filter(|k| *k >= 1);
        if shapes.is_empty() && positional.is_none() {
            return None;
        }
        let recv = self.coerce(recv, Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let join = self.new_block();
        let b_default = self.new_block();
        let mut cases = Vec::new();
        let mut blocks = Vec::new();
        for &(id, i) in &shapes {
            let b = self.new_block();
            cases.push((id, b));
            blocks.push((b, i));
        }
        // The runtime's positional record is the fixed class `_Record`.
        let record_id = i64::from(dartforge_runtime::layout::cid::RECORD);
        let b_runtime = self.new_block();
        cases.push((record_id, b_runtime));
        self.terminate(Terminator::Switch {
            val: cls,
            default: b_default,
            cases: cases,
        });
        let mut entries = Vec::new();
        for (b, i) in blocks {
            self.set_block(b);
            let v = self.shape_field(recv.clone(), i);
            entries.push((self.current_block, v));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(b_runtime);
        match positional {
            Some(k) => {
                // Inline: the number of fields in word 0 of the body and
                // field `k` (1-based) in word `k`; outside the shape,
                // `NoSuchMethodError`.
                let n = self.object_word(recv.clone(), 0, Type::I64);
                let fits = self.emit(Instruction::ICmp(ICmpOp::Sge, n, Operand::Constant(Constant::Int(k))), Type::I1);
                let b_read = self.new_block();
                let b_outside = self.new_block();
                self.terminate(Terminator::CondBranch { cond: fits, then_block: b_read, else_block: b_outside });
                self.set_block(b_read);
                let v = self.object_word(recv.clone(), k, Type::Ref);
                entries.push((self.current_block, v));
                self.terminate(Terminator::Branch(join));
                self.set_block(b_outside);
                self.throw_nsm(name);
            }
            None => {
                // A runtime positional record has no named field.
                self.throw_nsm(name);
            }
        }
        self.set_block(b_default);
        // What is not a record: the usual path; if it does not know the name
        // (the member only exists in records), `NoSuchMethodError`.
        let n_errors = self.errors.len();
        let mut v = pattern(self);
        if self.errors.len() > n_errors {
            self.errors.truncate(n_errors);
            v = self.throw_nsm(name);
        }
        if !matches!(self.operand_type(&v), Type::Void | Type::Ptr) {
            let v = self.coerce(v, Type::Ref);
            if !self.is_terminated() {
                entries.push((self.current_block, v));
                self.terminate(Terminator::Branch(join));
            }
        } else if !self.is_terminated() {
            entries.push((self.current_block, Operand::Constant(Constant::Null)));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        if entries.is_empty() {
            return Some(Operand::Constant(Constant::Null));
        }
        Some(self.emit(
            Instruction::Phi {
                incoming: entries,
                ty: Type::Ref,
            },
            Type::Ref,
        ))
    }

    /// `toString()` of shape `k`: `(p0, p1, a: x, b: y)`.
    pub fn lower_shape_to_string(&mut self, k: usize) {
        // Body called as a Dart function (by the `toString` dispatch): the
        // Ref return is Owned.
        if self.ctx.arc_memory {
            self.dart_ref_returns.insert(self.func.symbol.clone());
        }
        let this = Operand::Val(self.add_param("this".to_string(), Type::Ref));
        let (npos, names) = self.ctx.record_shapes[k].clone();
        let mut text = self.emit(Instruction::Const(Constant::String("(".to_string())), Type::Ref);
        for i in 0..npos + names.len() {
            let mut prefix = String::new();
            if i > 0 {
                prefix.push_str(", ");
            }
            if i >= npos {
                prefix.push_str(&names[i - npos]);
                prefix.push_str(": ");
            }
            if !prefix.is_empty() {
                let p = self.emit(Instruction::Const(Constant::String(prefix)), Type::Ref);
                text = self.concatenate(text, p);
            }
            let v = self.shape_field(this.clone(), i);
            let s = self.text_of(v);
            text = self.concatenate(text, s);
        }
        let end = self.emit(Instruction::Const(Constant::String(")".to_string())), Type::Ref);
        let r = self.concatenate(text, end);
        self.terminate(Terminator::Return(Some(r)));
    }

    fn concatenate(&mut self, a: Operand, b: Operand) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_string_concat".to_string(),
                args: vec![(a, Type::Ref), (b, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// `df.$registro.igual(a, b)`: same shape and equal fields (`==` of each
    /// field); what is not a record with a shape goes to the runtime `==`.
    pub fn lower_record_equality(&mut self) {
        let a = Operand::Val(self.add_param("a".to_string(), Type::Ref));
        let b = Operand::Val(self.add_param("b".to_string(), Type::Ref));
        let ca = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(a.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let cb = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(b.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let shapes = self.ctx.record_shapes.clone();
        let b_runtime = self.new_block();
        let b_false = self.new_block();
        let mut cases = Vec::new();
        let mut blocks = Vec::new();
        for (k, (npos, names)) in shapes.iter().enumerate() {
            let bl = self.new_block();
            cases.push((i64::from(SHAPE_BASE_ID) + k as i64, bl));
            blocks.push((bl, k, npos + names.len()));
        }
        self.terminate(Terminator::Switch {
            val: ca.clone(),
            default: b_runtime,
            cases: cases,
        });
        for (bl, k, n) in blocks {
            self.set_block(bl);
            let same = self.emit(
                Instruction::ICmp(
                    ICmpOp::Eq,
                    cb.clone(),
                    Operand::Constant(Constant::Int(i64::from(SHAPE_BASE_ID) + k as i64)),
                ),
                Type::I1,
            );
            let b_fields = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: same,
                then_block: b_fields,
                else_block: b_false,
            });
            self.set_block(b_fields);
            for i in 0..n {
                let x = self.shape_field(a.clone(), i);
                let y = self.shape_field(b.clone(), i);
                let ok = self.operate(ast::BinaryOp::Eq, x, y, false, Span { start: 0, end: 0 });
                let ok = self.to_bool(ok);
                let following = self.new_block();
                self.terminate(Terminator::CondBranch {
                    cond: ok,
                    then_block: following,
                    else_block: b_false,
                });
                self.set_block(following);
            }
            self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Bool(true)))));
        }
        self.set_block(b_false);
        self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Bool(false)))));
        self.set_block(b_runtime);
        let r = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_equal".to_string(),
                args: vec![(a, Type::Ref), (b, Type::Ref)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let r = self.emit(
            Instruction::Trunc {
                op: r,
                from: Type::I8,
                to: Type::I1,
            },
            Type::I1,
        );
        self.terminate(Terminator::Return(Some(r)));
    }

    /// Hash of a record shape: equal values must have the same hash
    /// so that `_CompactLinkedHashSet` and `_CompactLinkedHashMap` find the
    /// second record. The numeric value itself is not part of the Dart API.
    pub fn lower_shape_hash(&mut self, k: usize) {
        let this = Operand::Val(self.add_param("this".to_string(), Type::Ref));
        let (npos, names) = &self.ctx.record_shapes[k];
        let mut hash = Operand::Constant(Constant::Int(i64::from(SHAPE_BASE_ID) + k as i64));
        for i in 0..npos + names.len() {
            let field = self.shape_field(this.clone(), i);
            let code = self.call_by_name(field, super::sdk_source::AccessKind::Read, "hashCode", &[]);
            let code = self.coerce(code, Type::I64);
            let multiplied = self.emit(
                Instruction::Mul(hash, Operand::Constant(Constant::Int(31))),
                Type::I64,
            );
            hash = self.emit(Instruction::Add(multiplied, code), Type::I64);
        }
        self.terminate(Terminator::Return(Some(hash)));
    }

    /// `r.f(args)` where `f` is a field of some record shape: in the records
    /// with the field, reads and calls the value; on other receivers, `pattern`.
    pub fn call_record_field(
        &mut self,
        recv: Operand,
        name: &str,
        args: &mut dyn FnMut(&mut Self) -> Vec<super::members::Evaluated>,
        pattern: &mut dyn FnMut(&mut Self) -> Operand,
    ) -> Operand {
        let shapes = self.shapes_with_field(name);
        let recv = self.coerce(recv, Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let join = self.new_block();
        let b_default = self.new_block();
        let blocks: Vec<(i64, usize, BlockId)> = shapes.iter().map(|&(id, i)| (id, i, self.new_block())).collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: b_default,
            cases: blocks.iter().map(|&(id, _, b)| (id, b)).collect(),
        });
        let mut entries = Vec::new();
        for (_, i, b) in blocks {
            self.set_block(b);
            let f = self.shape_field(recv.clone(), i);
            let ev = args(self);
            let r = self.call_function_value(f, &ev);
            if !self.is_terminated() {
                entries.push((self.current_block, r));
                self.terminate(Terminator::Branch(join));
            }
        }
        self.set_block(b_default);
        let n_errors = self.errors.len();
        let mut v = pattern(self);
        if self.errors.len() > n_errors {
            self.errors.truncate(n_errors);
            v = self.throw_nsm(name);
        }
        if !self.is_terminated() {
            let v = if matches!(self.operand_type(&v), Type::Void | Type::Ptr) {
                Operand::Constant(Constant::Null)
            } else {
                self.coerce(v, Type::Ref)
            };
            if !self.is_terminated() {
                entries.push((self.current_block, v));
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
}
