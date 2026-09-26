//! Membros de extensão (P4): o receptor vira o primeiro parâmetro.
//!
//! A especificação (§13 "Extensions") resolve `r.m(…)` estaticamente para o
//! membro da extensão aplicável; a chamada é direta, sem despacho, com `r`
//! como `this`. Aqui o membro de instância de uma extensão é uma função
//! `df.<biblioteca>.ext:<E>.<m>(this, …)` com `this` na representação do
//! tipo `on` (um `int` vai como `i64`).

use super::fn_builder::FnBuilder;
use super::membros::Avaliado;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ExtensionId, FunctionKind};
use dartforge_frontend::ast::{self, ParameterKind};
use dartforge_types::table::TypeId;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Representação do receptor (`this`) de uma extensão.
    pub fn repr_do_receptor_de_extensao(&self, e: ExtensionId) -> Type {
        let on = self.ctx.outline.extensions[e.0 as usize].on;
        self.repr(on)
    }

    /// `r.m(args)` resolvido para o membro `fid` de uma extensão. `receptor`
    /// é o tipo estático de `r` e `chamada` a expressão com os argumentos
    /// (os dois armam a tupla de uma extensão ou membro genéricos).
    pub fn chamar_extensao(
        &mut self,
        recv: Operand,
        fid: usize,
        avaliados: &[Avaliado],
        receptor: Option<TypeId>,
        chamada: Option<(ast::ExprId, &ast::Arguments)>,
        span: Span,
    ) -> Operand {
        // A tupla desta chamada não vaza para a de fora (um getter de
        // extensão pode ser argumento de uma chamada genérica já armada).
        let salvo = self.tupla_armada.take();
        self.armar_tupla_com_receptor(fid, receptor, chamada);
        let r = self.chamar_extensao_armada(recv, fid, avaliados, span);
        self.tupla_armada = salvo;
        r
    }

    fn chamar_extensao_armada(&mut self, recv: Operand, fid: usize, avaliados: &[Avaliado], span: Span) -> Operand {
        let f = &self.ctx.program.functions[fid];
        let Some(e) = f.extension else {
            return self.nao_suportado("membro de extensão sem extensão", span);
        };
        if !super::funcao_do_usuario(self.ctx, fid) || !super::membros::tem_corpo(self.ctx, fid) {
            return self.nao_suportado("membro de extensão do SDK", span);
        }
        if f.static_ {
            let args = self.casar_args(fid, avaliados);
            return self.chamar_direto(fid, None, args);
        }
        let r = self.repr_do_receptor_de_extensao(e);
        let recv = self.coagir(recv, r);
        if f.kind == FunctionKind::Getter && !avaliados.is_empty() {
            let v = self.chamar_direto(fid, Some(recv), Vec::new());
            return self.chamar_valor_funcao(v, avaliados);
        }
        let args = self.casar_args(fid, avaliados);
        self.chamar_direto(fid, Some(recv), args)
    }

    /// O operador de instância de extensão que a inferência resolveu para a
    /// expressão (`p + 1`, `p[i]`), se é um.
    pub fn operador_de_extensao(&self, expr: ast::ExprId) -> Option<usize> {
        match self.ctx.get_resolved(self.unit_id, expr) {
            Some(dartforge_types::resolved::Resolved::ExtensionMember { member, .. })
                if !self.ctx.program.functions[member.0 as usize].static_
                    && super::funcao_do_usuario(self.ctx, member.0 as usize) =>
            {
                Some(member.0 as usize)
            }
            _ => None,
        }
    }

    /// `r.x` resolvido para um getter (ou tear-off de método) de extensão.
    pub fn ler_extensao(&mut self, recv: Operand, fid: usize, receptor: Option<TypeId>, span: Span) -> Operand {
        let f = &self.ctx.program.functions[fid];
        if f.kind == FunctionKind::Getter || f.static_ {
            return self.chamar_extensao(recv, fid, &[], receptor, None, span);
        }
        self.tearoff_de_extensao(recv, fid, receptor, span)
    }

    /// Tear-off de um método de instância de extensão (`21.dobro`,
    /// `completer.completeErrorIfPending`): uma closure com o receptor no
    /// ambiente, cuja entrada chama o membro como `chamar_extensao`. Os
    /// argumentos de tipo de uma extensão genérica vêm do tipo estático do
    /// receptor, então a entrada é uma por (membro, tipo do receptor).
    fn tearoff_de_extensao(&mut self, recv: Operand, fid: usize, receptor: Option<TypeId>, span: Span) -> Operand {
        let alvo = super::simbolo_de(self.ctx, fid);
        let simbolo_ent = match receptor {
            Some(t) if self.extensao_generica(fid) => format!("{alvo}$tearx{}", t.0),
            _ => format!("{alvo}$tearx"),
        };
        if !self.entradas_feitas.contains(&simbolo_ent) {
            self.entradas_feitas.insert(simbolo_ent.clone());
            let infos = self.params_da_funcao(fid);
            let nome = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, simbolo_ent.clone(), nome, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param("args".to_string(), Type::Ptr));
            let desc = Operand::Val(e.add_param("desc".to_string(), Type::Ptr));
            let env = e.emit(
                Instruction::CallRuntime {
                    name: "dartforge_closure_env".to_string(),
                    args: vec![(clo, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            let receptor_op = e.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
            if let Some(vals) = e.desempacotar(&infos, args, desc) {
                let avaliados: Vec<Avaliado> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.chamar_extensao(receptor_op, fid, &avaliados, receptor, None, span);
                let r = if matches!(e.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    e.coagir(r, Type::Ref)
                };
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorver(e);
        }
        let recv = self.coagir(recv, Type::Ref);
        let env = self.emit(Instruction::AllocEnv { values: vec![recv.clone()] }, Type::Ref);
        let c = self.emit(Instruction::AllocClosure { code_symbol: simbolo_ent, env }, Type::Ref);
        self.definir_rti_de_tearoff(c.clone(), fid, Some(recv));
        c
    }

    /// O membro é de uma extensão com parâmetros de tipo.
    fn extensao_generica(&self, fid: usize) -> bool {
        self.ctx.program.functions[fid]
            .extension
            .is_some_and(|e| !self.ctx.outline.extensions[e.0 as usize].type_params.is_empty())
    }
}
