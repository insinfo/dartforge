//! `external` das bibliotecas compiladas da fonte (P6): o corpo é a chamada
//! ao native do runtime.
//!
//! Na VM, um `external` com `@pragma("vm:external-name", "Nome")` é uma
//! função C++ do runtime (`runtime/lib/*.cc`). Aqui é a função Rust
//! `dartforge_nativo_<Nome>` (a tabela `crate::natives`, de δ), com a
//! assinatura na representação da HIR dos tipos declarados (`int` → `i64`,
//! `double` → `double`, `bool` → `i8` — o `bool` da ABI C —, o resto,
//! receptor inclusive, → `Ref`). O erro sai pela exceção pendente.
//!
//! O native que a tabela ainda marca `Pendente` é diagnóstico (N1) — só
//! vale se a poda mantiver a função (`source.rs`).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O corpo de um `external` com native: os parâmetros (e `this`) vão ao
    /// `dartforge_nativo_<Nome>` e o resultado volta na representação do
    /// retorno. Devolve `false` se a função não tem native.
    pub fn lower_external(&mut self, fid: usize, span: Span) -> bool {
        let (native, recognized) = crate::source::native_pragma(self.ctx.program, self.ctx.interner, fid);
        if (recognized || native.is_some()) && self.lower_intrinsic(fid) {
            return true;
        }
        let Some(name) = native else {
            return false;
        };
        match crate::natives::native(&name) {
            Some(n) if n.state == crate::natives::State::Runtime => {}
            _ => {
                self.unsupported(&format!("native `{name}` sem implementação no runtime"), span);
                self.terminate(Terminator::Return(None));
                return true;
            }
        }
        let mut args: Vec<(Operand, Type)> = Vec::new();
        for (vid, _, ty) in self.func.params.clone() {
            let abi = if ty == Type::I1 { Type::I8 } else { ty };
            let op = self.coerce(Operand::Val(vid), abi);
            args.push((op, abi));
        }
        let ret = self.func.return_ty;
        // O native que o SDK declara com mais de um tipo de retorno devolve
        // sempre a forma mais larga (`Ref`); o tipo declarado vem da coerção.
        let ret_abi = if crate::natives::RETURN_REF.contains(&name.as_str()) && ret != Type::Void {
            Type::Ref
        } else if ret == Type::I1 {
            Type::I8
        } else {
            ret
        };
        let r = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::natives::symbol(&name),
                args,
                ret_ty: ret_abi,
            },
            ret_abi,
        );
        if ret == Type::Void {
            self.terminate(Terminator::Return(None));
        } else {
            let r = self.coerce(r, ret);
            self.terminate(Terminator::Return(Some(r)));
        }
        true
    }

    /// Intrínsecos (`@pragma("vm:recognized")` sem native) que o código da
    /// fonte usa: na VM o compilador os gera no lugar da chamada; aqui são
    /// um corpo curto. `unsafeCast<T>(v)` é `v` (o chamador garante o tipo).
    fn lower_intrinsic(&mut self, fid: usize) -> bool {
        let f = &self.ctx.program.functions[fid];
        let lib = &self.ctx.program.library(f.library).uri;
        let name = self.ctx.symbol_name(f.name);
        match (lib.as_str(), name) {
            ("dart:_internal", "unsafeCast") => {
                let Some(&(v, _, _)) = self.func.params.first() else {
                    return false;
                };
                let ret = self.func.return_ty;
                let r = self.coerce(Operand::Val(v), ret);
                self.terminate(Terminator::Return(Some(r)));
                true
            }
            _ => false,
        }
    }
}
