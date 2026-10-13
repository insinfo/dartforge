//! Atribuição a identificador, propriedade e índice.
//!
//! Antes só o identificador gravava; `a.b = v` e `a[i] = v` avaliavam o valor
//! e o descartavam — a leitura seguinte via o campo zerado (H4 do plano,
//! `docs/NATIVO-PLANO.md` §6).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind};
use dartforge_frontend::ast::{self, ExprId};
use dartforge_types::resolved::{MemberRef, Resolved};

/// Lado direito de uma atribuição: uma expressão, ou o `1` de `++`/`--`.
#[derive(Clone, Copy)]
pub enum Rhs {
    Expr(ExprId),
    One,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    fn lower_rhs(&mut self, ast: &ast::Ast, rhs: Rhs, as_type: Type) -> Operand {
        match rhs {
            Rhs::Expr(e) => self.lower_expr(ast, e),
            Rhs::One if as_type == Type::F64 => Operand::Constant(Constant::Double(1.0)),
            Rhs::One => Operand::Constant(Constant::Int(1)),
        }
    }

    /// Valor a gravar: `v`, `cur op v`, ou `cur ?? v` (`??=`). O valor
    /// corrente fica em `old_value` para o `x++` pós-fixo.
    fn combine(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        cur: Option<Operand>,
        value: Rhs,
    ) -> Operand {
        self.old_value = cur.clone();
        match (op, cur) {
            (ast::AssignOp::Compound(ast::BinaryOp::IfNull), Some(cur)) => {
                let is_null = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, cur.clone(), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_new = self.new_block();
                let b_end = self.new_block();
                let origin = self.current_block;
                self.terminate(Terminator::CondBranch {
                    cond: is_null,
                    then_block: b_new,
                    else_block: b_end,
                });
                self.set_block(b_new);
                let ty = self.operand_type(&cur);
                let v = self.lower_rhs(ast, value, ty);
                let v = self.coerce(v, ty);
                let new_end = self.current_block;
                self.terminate(Terminator::Branch(b_end));
                self.set_block(b_end);
                self.emit(
                    Instruction::Phi {
                        incoming: vec![(origin, cur), (new_end, v)],
                        ty,
                    },
                    ty,
                )
            }
            (ast::AssignOp::Compound(b), Some(cur)) => {
                let ty = self.operand_type(&cur);
                // Basta um lado excluir int para a junção Smi ser impossível.
                // O destino pode ser getter/índice sem tipo conhecido aqui.
                let admits_smi = match value {
                    Rhs::Expr(e) => self.type_admits_smi(self.ctx.get_type(self.unit_id, e)),
                    Rhs::One => true,
                };
                let rhs = self.lower_rhs(ast, value, ty);
                self.lower_binary_op_helper(b, cur, rhs, admits_smi)
            }
            _ => self.lower_rhs(ast, value, Type::Ref),
        }
    }

    /// Atribuição a um par getter/setter de topo explícito. `f` é o elemento
    /// resolvido (getter ou setter); o setter sai do escopo da biblioteca
    /// que o declara, e a leitura corrente (composta) do getter.
    fn assign_top_level_accessor(
        &mut self,
        f: dartforge_elements::model::FunctionElementId,
        sym: dartforge_intern::SymbolId,
        ast: &ast::Ast,
        op: ast::AssignOp,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let func = &self.ctx.program.functions[f.0 as usize];
        let binding = self.ctx.program.lookup(func.library, sym);
        let setter = if func.kind == FunctionKind::Setter {
            Some(f)
        } else {
            binding.and_then(|b| b.setter).and_then(|el| match el {
                Element::Function(s) => Some(s),
                _ => None,
            })
        };
        let getter = if func.kind == FunctionKind::Setter {
            binding.and_then(|b| b.getter).and_then(|el| match el {
                Element::Function(g) => Some(g),
                _ => None,
            })
        } else {
            Some(f)
        };
        let Some(setter) = setter.filter(|s| super::user_function(self.ctx, s.0 as usize)) else {
            let n = self.ctx.symbol_name(sym).to_string();
            return self.unsupported(&format!("atribuição a `{n}` sem setter"), span);
        };
        let cur = match (matches!(op, ast::AssignOp::Compound(_)), getter) {
            (true, Some(g)) => Some(self.call_direct(g.0 as usize, None, Vec::new())),
            (true, None) => {
                let n = self.ctx.symbol_name(sym).to_string();
                return self.unsupported(&format!("leitura de `{n}` sem getter"), span);
            }
            _ => None,
        };
        let v = self.combine(ast, op, cur, value);
        self.call_direct(setter.0 as usize, None, vec![v.clone()]);
        v
    }

    /// `r.x op= v` com `x` membro de instância de uma extensão (`member`
    /// é o setter, ou o getter quando a resolução guardou a leitura): o
    /// setter `x=` e, na composta, o getter `x` da mesma extensão, os dois
    /// com o receptor avaliado uma vez.
    #[allow(clippy::too_many_arguments)]
    fn assign_extension(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        recv: Option<ExprId>,
        member: dartforge_elements::model::FunctionElementId,
        sym: dartforge_intern::SymbolId,
        value: Rhs,
        span: Span,
    ) -> Operand {
        // O setter e o getter do mesmo dono: a extensão, ou o tipo de
        // extensão (`extension_types.rs`).
        let name = self.ctx.symbol_name(sym).to_string();
        let m = member.0 as usize;
        let setter_key = self.ctx.interner.lookup(&format!("{name}_="));
        let setter = setter_key.and_then(|k| self.accessor_of_same_owner(m, k));
        let getter = self
            .accessor_of_same_owner(m, sym)
            .filter(|&g| self.ctx.program.functions[g.0 as usize].kind == FunctionKind::Getter);
        let Some(setter) = setter else {
            return self.unsupported(&format!("atribuição a `{name}` sem setter"), span);
        };
        let (recv_op, receiver) = match recv {
            Some(r) => (self.lower_target(ast, r), self.ctx.get_type_raw(self.unit_id, r)),
            None => (
                self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null)),
                self.implicit_receiver(),
            ),
        };
        let cur = if matches!(op, ast::AssignOp::Compound(_)) {
            let Some(g) = getter else {
                return self.unsupported(&format!("leitura de `{name}` sem getter"), span);
            };
            Some(self.call_extension(recv_op.clone(), g.0 as usize, &[], receiver, None, span))
        } else {
            None
        };
        let v = self.combine(ast, op, cur, value);
        self.call_extension(recv_op, setter.0 as usize, &[(None, v.clone())], receiver, None, span);
        v
    }

    /// `a?.b op= v` / `a?[i] op= v` (§17.23): `a` é avaliado uma vez; se é
    /// null, nada mais é avaliado e o valor é null; senão, a atribuição
    /// comum sobre o valor de `a`. A cadeia `?.` de `a` (`x?.y?.b = v`)
    /// desvia para o mesmo null. O `?.` antes do último elo também encurta
    /// a atribuição inteira (`a?.b.c = v`, `a?.b[i] = v`: com `a` null, nem
    /// `v` é avaliado); aí só a cadeia do receptor desvia (`last_null`
    /// falso: o último elo é comum).
    #[allow(clippy::too_many_arguments)]
    fn assign_with_null_aware(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        target: ExprId,
        recv: ExprId,
        last_null: bool,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let exit = self.new_block();
        let saved_chain = self.null_chain.replace((exit, Vec::new()));
        let recv_op = self.lower_target(ast, recv);
        if last_null {
            self.branch_if_null(&recv_op);
        }
        self.receiver_ready = Some((recv, recv_op));
        self.handled_null_aware = Some(target);
        let mut v = self.lower_assignment(ast, op, target, value, span);
        self.receiver_ready = None;
        // O valor antigo (`a?.x++`) também passa pela junção: null quando a
        // cadeia desviou.
        let mut old = self.old_value.take();
        let (exit, mut entries) = std::mem::replace(&mut self.null_chain, saved_chain).expect("cadeia aberta");
        let mut old_entries: Vec<(BlockId, Operand)> =
            entries.iter().map(|(b, _)| (*b, Operand::Constant(Constant::Null))).collect();
        if !self.is_terminated() {
            v = self.coerce(v, Type::Ref);
            if let Some(a) = old.take() {
                let a = self.coerce(a, Type::Ref);
                old_entries.push((self.current_block, a));
            }
            entries.push((self.current_block, v));
            self.terminate(Terminator::Branch(exit));
        }
        self.set_block(exit);
        if entries.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        if old_entries.len() == entries.len() {
            self.old_value = Some(self.emit(Instruction::Phi { incoming: old_entries, ty: Type::Ref }, Type::Ref));
        }
        self.emit(Instruction::Phi { incoming: entries, ty: Type::Ref }, Type::Ref)
    }

    /// `v = e` e `v op= e` num local SIMD sem caixa: o valor e a operação
    /// como vetores; um operador sem forma vetorial passa pela caixa.
    fn assign_simd_local(&mut self, ast: &ast::Ast, sym: dartforge_intern::SymbolId, op: ast::AssignOp, value: Rhs, span: Span) -> Operand {
        let k = self.lookup_local(sym).map(|l| l.ty).expect("local SIMD");
        let v = match (op, value) {
            (ast::AssignOp::Assign, Rhs::Expr(e)) => self.lower_simd(ast, e, k),
            (ast::AssignOp::Compound(b), Rhs::Expr(e)) => {
                let cur = self.read_local_by_name(sym).expect("local SIMD");
                let simd_op = match (b, k) {
                    (ast::BinaryOp::Add, _) => Some(OpSimd::Add),
                    (ast::BinaryOp::Sub, _) => Some(OpSimd::Sub),
                    (ast::BinaryOp::Mul, Type::V4F32 | Type::V2F64) => Some(OpSimd::Mul),
                    (ast::BinaryOp::Div, Type::V4F32 | Type::V2F64) => Some(OpSimd::Div),
                    (ast::BinaryOp::BitAnd, Type::V4I32) => Some(OpSimd::And),
                    (ast::BinaryOp::BitOr, Type::V4I32) => Some(OpSimd::Or),
                    (ast::BinaryOp::BitXor, Type::V4I32) => Some(OpSimd::Xor),
                    _ => None,
                };
                match simd_op {
                    Some(o) => {
                        let rhs = self.lower_simd(ast, e, k);
                        self.emit(Instruction::Simd { op: o, args: vec![cur, rhs] }, k)
                    }
                    None => {
                        let boxed = self.coerce(cur, Type::Ref);
                        let v = self.combine(ast, op, Some(boxed), Rhs::Expr(e));
                        self.coerce(v, k)
                    }
                }
            }
            (op, value) => {
                let cur = matches!(op, ast::AssignOp::Compound(_))
                    .then(|| self.read_local_by_name(sym).map(|c| self.coerce(c, Type::Ref)))
                    .flatten();
                let v = self.combine(ast, op, cur, value);
                self.coerce(v, k)
            }
        };
        match self.store_local(sym, v) {
            Some(v) => v,
            None => self.unsupported("atribuição a local desconhecido", span),
        }
    }

    /// Atribuição. Ordem de avaliação do Dart: receptor, índice, leitura
    /// corrente (se composta), valor, gravação.
    pub fn lower_assignment(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        target: ExprId,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let composite = matches!(op, ast::AssignOp::Compound(_));
        match &ast.expr(target).kind {
            ast::ExprKind::Identifier(name) => {
                let sym = name.sym;
                let resolved = match self.ctx.get_resolved(self.unit_id, target).cloned() {
                    // Sem resolução (corpo de closure) e sem local: pelo
                    // escopo léxico (membro da classe, topo da biblioteca).
                    None if self.lookup_local(sym).is_none() => self.resolve_by_name(sym),
                    r => r,
                };
                match resolved {
                    // Setter de tipo de extensão com o `this` implícito.
                    r @ Some(Resolved::Member { .. }) if self.resolved_member_te(r.as_ref()).is_some() => {
                        match self.resolved_member_te(r.as_ref()) {
                            Some(MemberRef::Function(f)) if !self.is_representation(MemberRef::Function(f)) => {
                                self.assign_extension(ast, op, None, f, sym, value, span)
                            }
                            _ => self.unsupported("atribuição à representação de tipo de extensão", span),
                        }
                    }
                    Some(Resolved::Member { member, .. }) => {
                        self.assign_member(None, member, ast, op, value, span)
                    }
                    Some(Resolved::ExtensionMember { member, .. })
                        if !self.ctx.program.functions[member.0 as usize].static_ =>
                    {
                        self.assign_extension(ast, op, None, member, sym, value, span)
                    }
                    Some(Resolved::Element(Element::Variable(vid))) => {
                        let cur = if composite {
                            Some(self.read_global(vid, span))
                        } else {
                            None
                        };
                        let v = self.combine(ast, op, cur, value);
                        self.store_global(vid, v, span)
                    }
                    Some(Resolved::Element(Element::Function(f)))
                        if self.ctx.program.functions[f.0 as usize].variable.is_some() =>
                    {
                        let vid = self.ctx.program.functions[f.0 as usize]
                            .variable
                            .expect("acessor");
                        let cur = if composite {
                            Some(self.read_global(vid, span))
                        } else {
                            None
                        };
                        let v = self.combine(ast, op, cur, value);
                        self.store_global(vid, v, span)
                    }
                    // Getter/setter de topo explícitos (`set exitCode(int)`):
                    // o elemento resolvido pode ser o getter; o setter é o
                    // outro lado do mesmo nome no escopo.
                    Some(Resolved::Element(Element::Function(f))) => {
                        self.assign_top_level_accessor(f, sym, ast, op, value, span)
                    }
                    _ if self.lookup_local(sym).is_some_and(|l| l.ty.is_vector()) => {
                        // Local SIMD sem caixa (`simd.rs`): o valor vem como vetor.
                        self.assign_simd_local(ast, sym, op, value, span)
                    }
                    _ => {
                        let cur = if composite {
                            self.read_local_by_name(sym)
                        } else {
                            None
                        };
                        if composite && cur.is_none() {
                            return self
                                .unsupported("atribuição composta a local desconhecido", span);
                        }
                        let v = self.combine(ast, op, cur, value);
                        match self.store_local(sym, v) {
                            Some(v) => v,
                            None => self.unsupported("atribuição a local desconhecido", span),
                        }
                    }
                }
            }
            ast::ExprKind::Property {
                target: recv,
                name,
                null_aware,
            } => {
                if (*null_aware || Self::chain_has_null_aware(ast, *recv))
                    && self.handled_null_aware != Some(target)
                {
                    return self.assign_with_null_aware(ast, op, target, *recv, *null_aware, value, span);
                }
                self.handled_null_aware = None;
                // `prefix.x = v`: variável de topo importada com prefixo.
                if let Some(el) = self.prefixed_element(ast, *recv, name.sym) {
                    let vid = match el {
                        Element::Variable(v) => Some(v),
                        Element::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
                        _ => None,
                    };
                    if let Some(vid) = vid {
                        let cur = if composite {
                            Some(self.read_global(vid, span))
                        } else {
                            None
                        };
                        let v = self.combine(ast, op, cur, value);
                        return self.store_global(vid, v, span);
                    }
                    // Getter/setter de topo explícitos da biblioteca do
                    // prefixo (`intl.defaultLocale = l`).
                    if let Element::Function(f) = el {
                        return self.assign_top_level_accessor(f, name.sym, ast, op, value, span);
                    }
                }
                // `super.x = v` (P4).
                if matches!(ast.expr(*recv).kind, ast::ExprKind::Super) {
                    let cur = if composite {
                        Some(self.read_super(name.sym, span))
                    } else {
                        None
                    };
                    let v = self.combine(ast, op, cur, value);
                    return self.store_super(name.sym, v, span);
                }
                // `E.x = v`: campo estático de extensão (ou setter estático).
                if matches!(
                    self.ctx.get_resolved(self.unit_id, *recv),
                    Some(Resolved::Element(Element::Extension(_)))
                ) {
                    match self.ctx.get_resolved(self.unit_id, target).cloned() {
                        Some(Resolved::Element(Element::Variable(vid))) => {
                            let cur = composite.then(|| self.read_global(vid, span));
                            let v = self.combine(ast, op, cur, value);
                            return self.store_global(vid, v, span);
                        }
                        Some(Resolved::ExtensionMember { member, .. })
                            if self.ctx.program.functions[member.0 as usize].kind == FunctionKind::Setter =>
                        {
                            let e = self.ctx.program.functions[member.0 as usize].extension.expect("extensão");
                            let getter = self.ctx.program.extensions[e.0 as usize].static_members.get(&name.sym).copied();
                            let cur = match (composite, getter) {
                                (true, Some(g)) => Some(self.call_direct(g.0 as usize, None, Vec::new())),
                                (true, None) => return self.unsupported("leitura sem getter estático", span),
                                _ => None,
                            };
                            let v = self.combine(ast, op, cur, value);
                            self.call_direct(member.0 as usize, None, vec![v.clone()]);
                            return v;
                        }
                        _ => {
                            let n = self.ctx.symbol_name(name.sym).to_string();
                            return self.unsupported(&format!("atribuição a `{n}` de extensão"), span);
                        }
                    }
                }
                // Setter de tipo de extensão: como o de extensão.
                if let Some(m) = self.member_te(target) {
                    return match m {
                        MemberRef::Function(f) if !self.is_representation(m) => {
                            self.assign_extension(ast, op, Some(*recv), f, name.sym, value, span)
                        }
                        _ => self.unsupported("atribuição à representação de tipo de extensão", span),
                    };
                }
                // Setter de extensão (P4): chamada direta com o receptor.
                if let Some(Resolved::ExtensionMember { member, .. }) = self.ctx.get_resolved(self.unit_id, target).cloned()
                    && !self.ctx.program.functions[member.0 as usize].static_
                    && super::user_function(self.ctx, member.0 as usize)
                {
                    return self.assign_extension(ast, op, Some(*recv), member, name.sym, value, span);
                }
                // `C.x = v` (ou `A.x = v` com `typedef A = C<…>`).
                let target_is_class = self.static_target_class(*recv).is_some();
                if target_is_class {
                    let Some(Resolved::Member { class, member, .. }) =
                        self.ctx.get_resolved(self.unit_id, target).cloned()
                    else {
                        let n = self.ctx.symbol_name(name.sym).to_string();
                        return self.unsupported(&format!("atribuição a `{n}`"), span);
                    };
                    let member = if matches!(member, MemberRef::Function(f)
                        if self.ctx.program.functions[f.0 as usize].kind != FunctionKind::Setter
                            && self.ctx.program.functions[f.0 as usize].variable.is_none()) {
                        let key = format!("{}_=", self.ctx.symbol_name(name.sym));
                        self.ctx.interner.lookup(&key)
                            .and_then(|s| self.ctx.program.classes[class.0 as usize].static_members.get(&s).copied())
                            .map(MemberRef::Function)
                            .unwrap_or(member)
                    } else { member };
                    return self.assign_member(None, member, ast, op, value, span);
                }
                let Some((_, member)) = self.user_member(target, *recv, name.sym, true)
                else {
                    let n = self.ctx.symbol_name(name.sym).to_string();
                    if self.dynamic_receiver(*recv) {
                        // Até um setter inexistente passa pelo seletor: o
                        // runtime produz o `NoSuchMethodError`. O RHS é
                        // avaliado depois do receptor, como em Dart.
                        let recv_op = self.lower_expr(ast, *recv);
                        let cur = composite.then(|| self.call_by_name(
                            recv_op.clone(), super::sdk_source::Tipo::Read, &n, &[],
                        ));
                        let v = self.combine(ast, op, cur, value);
                        self.call_by_name(
                            recv_op, super::sdk_source::Tipo::Write, &n,
                            &[(None, v.clone())],
                        );
                        return v;
                    }
                    return self.unsupported(&format!("atribuição a `{n}`"), span);
                };
                let recv_op = self.lower_expr(ast, *recv);
                self.assign_member(Some(recv_op), member, ast, op, value, span)
            }
            ast::ExprKind::Index {
                target: t,
                index,
                null_aware,
            } => {
                if (*null_aware || Self::chain_has_null_aware(ast, *t))
                    && self.handled_null_aware != Some(target)
                {
                    return self.assign_with_null_aware(ast, op, target, *t, *null_aware, value, span);
                }
                self.handled_null_aware = None;
                // `super[i] op= v`: os operadores `[]`/`[]=` da superclasse.
                if matches!(ast.expr(*t).kind, ast::ExprKind::Super) {
                    let i_op = self.lower_expr(ast, *index);
                    let (Some(get), Some(set)) = (self.ctx.interner.lookup("[]"), self.ctx.interner.lookup("[]=")) else {
                        return self.unsupported("`super[]=` sem operador", span);
                    };
                    let cur = composite.then(|| self.call_super_method(get, &[(None, i_op.clone())], span));
                    let v = self.combine(ast, op, cur, value);
                    self.call_super_method(set, &[(None, i_op), (None, v.clone())], span);
                    return v;
                }
                let t_op = self.lower_expr(ast, *t);
                let i_op = self.lower_expr(ast, *index);
                self.implicit_operator_cast(*index, &i_op, *t, "[]=", 0);
                // `[]=` de extensão (a inferência resolve o índice para o
                // `[]` da extensão; o `[]=` é o da mesma extensão).
                if let Some(get) = self.extension_operator(target) {
                    let set = self.ctx.interner.lookup("[]=").and_then(|k| self.accessor_of_same_owner(get, k));
                    let Some(set) = set else {
                        return self.unsupported("operador []= de extensão ausente", span);
                    };
                    let receiver = self.ctx.get_type_raw(self.unit_id, *t);
                    let cur = composite.then(|| self.call_extension(t_op.clone(), get, &[(None, i_op.clone())], receiver, None, span));
                    let v = self.combine(ast, op, cur, value);
                    self.call_extension(t_op, set.0 as usize, &[(None, i_op), (None, v.clone())], receiver, None, span);
                    return v;
                }
                if let Some(l) = self.indexable(self.ctx.get_type(self.unit_id, *t))
                    && self.operand_type(&i_op) == Type::I64
                {
                    // Lista tipada numérica ou `List<E>`: leitura e gravação
                    // diretas (`typed_items.rs`).
                    let repr = self.expression_repr(target).unwrap_or(Type::Ref);
                    let fixed = self.fixed_list_of(ast, *t);
                    self.access_fixed = fixed.clone();
                    let proven = self.proven_of(ast, *t, Some(*index));
                    self.access_proven = proven.clone();
                    let cur = if composite { self.read_indexed(t_op.clone(), i_op.clone(), l, repr) } else { None };
                    self.access_proven = None;
                    self.access_fixed = None;
                    // Lista SIMD com valor SIMD do mesmo tipo: o valor vai
                    // como vetor, e só o caminho lento (`[]=` do SDK) o põe
                    // na caixa — antes, `v4[i] = a + b` alocava uma caixa
                    // por gravação (docs/SIMD-NATIVO.md).
                    let simd = match (l, value) {
                        (super::typed_items::Indexable::Simd { k, .. }, Rhs::Expr(e))
                            if !composite && self.expr_simd(e) == Some(k) =>
                        {
                            Some((e, k))
                        }
                        _ => None,
                    };
                    let v = match simd {
                        Some((e, k)) => self.lower_simd(ast, e, k),
                        None => self.combine(ast, op, cur, value),
                    };
                    if !composite && simd.is_none() && let Rhs::Expr(e) = value {
                        self.implicit_operator_cast(e, &v, *t, "[]=", 1);
                    }
                    self.access_fixed = fixed;
                    self.access_proven = proven;
                    let stored = self.store_indexed(t_op.clone(), i_op.clone(), v.clone(), l);
                    self.access_proven = None;
                    self.access_fixed = None;
                    if stored {
                        return v;
                    }
                    let v = if simd.is_some() { self.coerce(v, Type::Ref) } else { v };
                    self.call_by_name_typed(t_op, *t, super::sdk_source::Tipo::Call, "[]=", &[(None, i_op), (None, v.clone())]);
                    return v;
                }
                // `[]=` (e o `[]` do composto) de classe do programa: pelos
                // membros.
                if let Some(set) = self.program_operator(*t, "[]=") {
                    let cur = if composite {
                        let Some(get) = self.program_operator(*t, "[]") else {
                            return self.unsupported("operador [] ausente", span);
                        };
                        Some(self.call_member(t_op.clone(), get, &[(None, i_op.clone())], span))
                    } else {
                        None
                    };
                    let v = self.combine(ast, op, cur, value);
                    self.call_member(t_op, set, &[(None, i_op), (None, v.clone())], span);
                    return v;
                }
                // `[]`/`[]=` do SDK pela classe dinâmica.
                use super::sdk_source::Tipo;
                let cur = composite.then(|| self.call_by_name_typed(t_op.clone(), *t, Tipo::Call, "[]", &[(None, i_op.clone())]));
                let v = self.combine(ast, op, cur, value);
                if !composite && let Rhs::Expr(e) = value {
                    self.implicit_operator_cast(e, &v, *t, "[]=", 1);
                }
                self.call_by_name_typed(t_op, *t, Tipo::Call, "[]=", &[(None, i_op), (None, v.clone())]);
                v
            }
            _ => self.unsupported("alvo de atribuição", span),
        }
    }

    /// Grava num membro: campo (de `recv`, ou de `this` quando `recv` é
    /// None), global estático, ou setter.
    fn assign_member(
        &mut self,
        recv: Option<Operand>,
        member: MemberRef,
        ast: &ast::Ast,
        op: ast::AssignOp,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let composite = matches!(op, ast::AssignOp::Compound(_));
        let vid = match member {
            MemberRef::Variable(v) => Some(v),
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
        };
        if let Some(vid) = vid {
            if super::is_global(self.ctx, vid) {
                let cur = if composite {
                    Some(self.read_global(vid, span))
                } else {
                    None
                };
                let v = self.combine(ast, op, cur, value);
                return self.store_global(vid, v, span);
            }
            let Some(obj) = recv.or_else(|| self.this_param.clone()) else {
                return self.unsupported("campo de instância fora de membro de instância", span);
            };
            let cur = if composite {
                Some(self.read_field_with_late(obj.clone(), vid, span))
            } else {
                None
            };
            let v = self.combine(ast, op, cur, value);
            if !self.store_source_field(obj.clone(), vid, v.clone()) {
                self.store_field(obj, vid, v.clone(), span);
            }
            return v;
        }
        let MemberRef::Function(f) = member else {
            unreachable!()
        };
        let fid = f.0 as usize;
        let func = &self.ctx.program.functions[fid];
        if func.kind != FunctionKind::Setter || !super::user_function(self.ctx, fid) {
            return self.unsupported("atribuição a membro que não é campo nem setter", span);
        }
        let static_ty = func.static_;
        let name = self.ctx.symbol_name(func.name).to_string();
        let cid = func.class;
        let obj = if static_ty { None } else { recv.or_else(|| self.this_param.clone()) };
        let cur = if composite {
            let getter = cid.and_then(|cid| {
                if static_ty {
                    let sym = self.ctx.interner.lookup(&name)?;
                    self.ctx.program.classes[cid.0 as usize].static_members.get(&sym).copied()
                } else {
                    // O getter pode vir de uma superinterface (`List.last=`
                    // com o `last` de `Iterable`): a busca é a da interface
                    // do tipo, não só a da cadeia de superclasses.
                    self.member_in_class(cid, &name)
                        .map(|fid| dartforge_elements::model::FunctionElementId(fid as u32))
                        .or_else(|| interface_getter(self.ctx, cid, &name))
                }
            });
            let Some(getter) = getter else {
                return self.unsupported("getter ausente para atribuição composta", span);
            };
            let getter_member = MemberRef::Function(getter);
            if static_ty {
                Some(self.read_static_member(getter_member, span))
            } else {
                let Some(receiver) = obj.clone() else {
                    return self.unsupported("setter fora de membro de instância", span);
                };
                let getter_fid = getter.0 as usize;
                if let Some(vid) = self.ctx.program.functions[getter_fid].variable {
                    Some(self.read_field_with_late(receiver, vid, span))
                } else {
                    Some(self.call_member(receiver, getter_fid, &[], span))
                }
            }
        } else { None };
        let v = self.combine(ast, op, cur, value);
        if static_ty {
            let args = self.match_args(fid, &[(None, v.clone())]);
            self.call_direct(fid, None, args);
            return v;
        }
        let Some(obj) = obj else {
            return self.unsupported("setter fora de membro de instância", span);
        };
        self.call_member(obj, fid, &[(None, v.clone())], span);
        v
    }

}

/// O getter `name` da interface da classe `cid`: a própria classe, as
/// superclasses, os mixins e as superinterfaces, em largura (a primeira
/// declaração achada; numa interface bem formada as outras têm a mesma
/// assinatura).
fn interface_getter(
    ctx: &crate::context::Context,
    cid: dartforge_elements::model::ClassId,
    name: &str,
) -> Option<dartforge_elements::model::FunctionElementId> {
    let sym = ctx.interner.lookup(name)?;
    let mut queue = std::collections::VecDeque::from([cid]);
    let mut seen_set = std::collections::HashSet::new();
    while let Some(c) = queue.pop_front() {
        if !seen_set.insert(c) {
            continue;
        }
        let class = &ctx.program.classes[c.0 as usize];
        if let Some(&f) = class.instance_members.get(&sym)
            && ctx.program.functions[f.0 as usize].kind != FunctionKind::Setter
        {
            return Some(f);
        }
        queue.extend(class.supertype_class);
        queue.extend(class.mixin_classes.iter().copied());
        queue.extend(class.interface_classes.iter().copied());
        queue.extend(class.on_classes.iter().copied());
    }
    None
}
