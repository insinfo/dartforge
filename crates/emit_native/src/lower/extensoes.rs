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

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Representação do receptor (`this`) de uma extensão.
    pub fn repr_do_receptor_de_extensao(&self, e: ExtensionId) -> Type {
        let on = self.ctx.outline.extensions[e.0 as usize].on;
        self.repr(on)
    }

    /// `r.m(args)` resolvido para o membro `fid` de uma extensão.
    pub fn chamar_extensao(&mut self, recv: Operand, fid: usize, avaliados: &[Avaliado], span: Span) -> Operand {
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

    /// `r.x` resolvido para um getter (ou tear-off de método) de extensão.
    pub fn ler_extensao(&mut self, recv: Operand, fid: usize, span: Span) -> Operand {
        let f = &self.ctx.program.functions[fid];
        if f.kind == FunctionKind::Getter || f.static_ {
            return self.chamar_extensao(recv, fid, &[], span);
        }
        self.nao_suportado("tear-off de membro de extensão", span)
    }
}
