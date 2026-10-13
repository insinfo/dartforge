//! Membros de extensão (P4): o receptor vira o primeiro parâmetro.
//!
//! A especificação (§13 "Extensions") resolve `r.m(…)` estaticamente para o
//! membro da extensão aplicável; a chamada é direta, sem despacho, com `r`
//! como `this`. Aqui o membro de instância de uma extensão é uma função
//! `df.<library>.ext:<E>.<m>(this, …)` com `this` na representação do
//! tipo `on` (um `int` vai como `i64`).

use super::fn_builder::FnBuilder;
use super::members::Evaluated;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ExtensionId, FunctionKind};
use dartforge_frontend::ast::{self, ParameterKind};
use dartforge_types::table::TypeId;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Representação do receptor (`this`) de uma extensão.
    pub fn extension_receiver_repr(&self, e: ExtensionId) -> Type {
        let on = self.ctx.outline.extensions[e.0 as usize].on;
        self.repr(on)
    }

    /// `r.m(args)` resolvido para o membro `fid` de uma extensão. `receiver`
    /// é o tipo estático de `r` e `call` a expressão com os argumentos
    /// (os dois armam a tupla de uma extensão ou membro genéricos).
    pub fn call_extension(
        &mut self,
        recv: Operand,
        fid: usize,
        evaluated: &[Evaluated],
        receiver: Option<TypeId>,
        call: Option<(ast::ExprId, &ast::Arguments)>,
        span: Span,
    ) -> Operand {
        // A tupla desta chamada não vaza para a de fora (um getter de
        // extensão pode ser argumento de uma chamada genérica já armada).
        let saved = self.armed_tuple.take();
        let is_call = call.is_some();
        self.arm_tuple_with_receiver(fid, receiver, call);
        let r = self.call_armed_extension(recv, fid, evaluated, is_call, span);
        self.armed_tuple = saved;
        r
    }

    /// `is_call`: a expressão é uma chamada (`x.g()`); num getter, o valor
    /// que ele devolve é chamado com os argumentos (mesmo sem nenhum).
    fn call_armed_extension(&mut self, recv: Operand, fid: usize, evaluated: &[Evaluated], is_call: bool, span: Span) -> Operand {
        let f = &self.ctx.program.functions[fid];
        if !super::user_function(self.ctx, fid) || !super::members::has_body(self.ctx, fid) {
            return self.unsupported("membro de extensão do SDK", span);
        }
        if f.static_ {
            let args = self.match_args(fid, evaluated);
            return self.call_direct(fid, None, args);
        }
        // O receptor na representação do `on` (extensão) ou da
        // representação (tipo de extensão, `extension_types.rs`).
        let Some(r) = self.static_this_repr(fid) else {
            return self.unsupported("membro de extensão sem extensão", span);
        };
        let recv = self.coerce(recv, r);
        if f.kind == FunctionKind::Getter && (is_call || !evaluated.is_empty()) {
            let v = self.call_direct(fid, Some(recv), Vec::new());
            return self.call_function_value(v, evaluated);
        }
        let args = self.match_args(fid, evaluated);
        self.call_direct(fid, Some(recv), args)
    }

    /// O operador de instância de extensão que a inferência resolveu para a
    /// expressão (`p + 1`, `p[i]`), se é um.
    pub fn extension_operator(&self, expr: ast::ExprId) -> Option<usize> {
        // O de um tipo de extensão também é chamado assim.
        if let Some(f) = self.function_te(expr) {
            return Some(f);
        }
        match self.ctx.get_resolved(self.unit_id, expr) {
            Some(dartforge_types::resolved::Resolved::ExtensionMember { member, .. })
                if !self.ctx.program.functions[member.0 as usize].static_
                    && super::user_function(self.ctx, member.0 as usize) =>
            {
                Some(member.0 as usize)
            }
            _ => None,
        }
    }

    /// `r.x` resolvido para um getter (ou tear-off de método) de extensão.
    pub fn read_extension(&mut self, recv: Operand, fid: usize, receiver: Option<TypeId>, span: Span) -> Operand {
        let f = &self.ctx.program.functions[fid];
        // Um método ESTÁTICO da extensão citado sem qualificação no corpo
        // dela (`codeUnits.any(_maiuscula)`) é um tear-off estático, como
        // `Ext._maiuscula`; getters e campos estáticos são lidos.
        if f.static_ && f.kind == FunctionKind::Function && f.variable.is_none() {
            let el = dartforge_elements::model::Element::Function(dartforge_elements::model::FunctionElementId(fid as u32));
            return self.read_element(el, span);
        }
        if f.kind == FunctionKind::Getter || f.static_ {
            return self.call_extension(recv, fid, &[], receiver, None, span);
        }
        self.extension_tearoff(recv, fid, receiver, span)
    }

    /// Tear-off de um método de instância de extensão (`21.double_of`,
    /// `completer.completeErrorIfPending`): uma closure com o receptor no
    /// ambiente, cuja entrada chama o membro como `call_extension`. Os
    /// argumentos de tipo de uma extensão genérica vêm do tipo estático do
    /// receptor, então a entrada é uma por (membro, tipo do receptor).
    fn extension_tearoff(&mut self, recv: Operand, fid: usize, receiver: Option<TypeId>, span: Span) -> Operand {
        let alvo = super::symbol_of(self.ctx, fid);
        let symbol_ent = match receiver {
            Some(t) if self.generic_extension(fid) => format!("{alvo}$tearx{}", t.0),
            _ => format!("{alvo}$tearx"),
        };
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), name, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            let env = e.closure_context(clo);
            let receiver_op = e.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
            if let Some(vals) = e.unpack(&infos, args, desc) {
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.call_extension(receiver_op, fid, &evaluated, receiver, None, span);
                let r = if matches!(e.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    e.coerce(r, Type::Ref)
                };
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let recv = self.coerce(recv, Type::Ref);
        let env = self.emit(Instruction::AllocEnv { values: vec![recv.clone()] }, Type::Ref);
        let c = self.emit(Instruction::AllocClosure { code_symbol: symbol_ent, env }, Type::Ref);
        // A assinatura com os argumentos de tipo do dono tirados do tipo
        // estático do receptor (a tupla da chamada que a entrada faria).
        let tuple = if self.generic_extension(fid) {
            let saved = self.armed_tuple.take();
            self.arm_tuple_with_receiver(fid, receiver, None);
            std::mem::replace(&mut self.armed_tuple, saved)
        } else {
            None
        };
        self.define_tearoff_rti_with_tuple(c.clone(), fid, Some(recv), tuple);
        c
    }

    /// O membro é de uma extensão (ou tipo de extensão) com parâmetros de
    /// tipo.
    fn generic_extension(&self, fid: usize) -> bool {
        self.receiver_pattern(fid).is_some_and(|(n, _)| n > 0)
    }
}
