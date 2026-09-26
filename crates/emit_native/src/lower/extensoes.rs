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
use dartforge_frontend::ast;
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
        self.nao_suportado("tear-off de membro de extensão", span)
    }
}
