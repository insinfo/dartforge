//! `import … deferred as p`: the compiled program has the library from the
//! start, but, as in the VM (the front end's `CheckLibraryIsLoaded`), an access
//! through `p` before the future of `p.loadLibrary()` completes throws
//! `_DeferredNotLoadedError` ("Deferred library p was not loaded."). Each prefix
//! of each library has a flag global (per isolate, like the Dart statics) that
//! the completion of `loadLibrary` sets (`_dartforgeLoadLibrary`,
//! `native_sdk/async/async_patch.dart`).

use super::fn_builder::FnBuilder;
use crate::hir::{Constant, DartEntryVector, ICmpOp, Instruction, Operand, Terminator, Type};
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::Resolved;

impl FnBuilder<'_, '_> {
    /// `sym` names the prefix of an `import … deferred as` of the current
    /// library, not shadowed by a local.
    pub(super) fn deferred_import(&self, sym: SymbolId) -> bool {
        if self.lookup_local(sym).is_some() {
            return false;
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        self.ctx
            .program
            .library(lib)
            .imports
            .iter()
            .any(|i| i.deferred && i.prefix == Some(sym))
    }

    /// The deferred prefix `target` is, when it is one.
    fn deferred_prefix_of(&self, ast: &ast::Ast, target: ExprId) -> Option<SymbolId> {
        let ExprKind::Identifier(p) = &ast.expr(target).kind else {
            return None;
        };
        if !matches!(
            self.ctx.get_resolved(self.unit_id, target),
            Some(Resolved::Prefix(_)) | None
        ) {
            return None;
        }
        self.deferred_import(p.sym).then_some(p.sym)
    }

    /// The deferred prefix the expression `e` goes through (`p.x`, `p.f()`,
    /// `p.x = v`, `p.x++`, `p.C.x`, `new p.C()`), with the inner expression that
    /// is lowered on its own and so must not check again; `p.loadLibrary()` is
    /// not an access.
    fn deferred_access(&self, ast: &ast::Ast, e: ExprId) -> Option<(SymbolId, Option<ExprId>)> {
        match &ast.expr(e).kind {
            ExprKind::Property { target, name, .. } => {
                if let Some(p) = self.deferred_prefix_of(ast, *target) {
                    return (self.ctx.symbol_name(name.sym) != "loadLibrary").then_some((p, None));
                }
                self.deferred_access(ast, *target)
                    .map(|(p, _)| (p, Some(*target)))
            }
            ExprKind::Call { target, .. } => self
                .deferred_access(ast, *target)
                .map(|(p, _)| (p, Some(*target))),
            ExprKind::Assign { target, .. } => self
                .deferred_access(ast, *target)
                .map(|(p, _)| (p, Some(*target))),
            ExprKind::Unary { operand, .. } => self
                .deferred_access(ast, *operand)
                .map(|(p, _)| (p, Some(*operand))),
            ExprKind::InstanceCreation { ty, .. } => match &ast.ty(*ty).kind {
                ast::TypeKind::Named { name, .. } if name.len() == 2 => self
                    .deferred_import(name[0].sym)
                    .then_some((name[0].sym, None)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The flag global of the deferred prefix `p` of the current library: 0
    /// until the future of `p.loadLibrary()` completes.
    fn deferred_flag(&mut self, p: SymbolId) -> String {
        let lib = self.ctx.program.unit(self.unit_id).library;
        let symbol = format!(
            "dfd.{}.{}",
            crate::context::escape_str(&self.ctx.library_name(lib)),
            crate::context::escape_str(self.ctx.symbol_name(p))
        );
        if self.done_entries.insert(symbol.clone()) {
            self.extra_globals.push((0, Type::I64, symbol.clone()));
        }
        symbol
    }

    /// Before the expression `e`: when it goes through a deferred prefix that
    /// has not loaded yet, throws `_DeferredNotLoadedError`.
    pub(super) fn check_deferred_access(&mut self, ast: &ast::Ast, e: ExprId) {
        if self.deferred_checked.remove(&e) {
            return;
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        if !self
            .ctx
            .program
            .library(lib)
            .imports
            .iter()
            .any(|i| i.deferred)
        {
            return;
        }
        let Some((p, inner)) = self.deferred_access(ast, e) else {
            return;
        };
        if let Some(inner) = inner {
            self.deferred_checked.insert(inner);
        }
        let flag = self.deferred_flag(p);
        let loaded = self.emit(
            Instruction::LoadGlobal {
                symbol: flag,
                ty: Type::I64,
            },
            Type::I64,
        );
        let ok = self.emit(
            Instruction::ICmp(ICmpOp::Ne, loaded, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_fail = self.new_block();
        let b_ok = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ok,
            then_block: b_ok,
            else_block: b_fail,
        });
        self.set_block(b_fail);
        let name = self.emit(
            Instruction::Const(Constant::String(self.ctx.symbol_name(p).to_string())),
            Type::Ref,
        );
        let span = ast.expr(e).span;
        self.call_support("_dartforgeDeferredNotLoaded", vec![name], span);
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(b_ok));
        }
        self.set_block(b_ok);
    }

    /// `p.loadLibrary()`: a future that completes, on a later turn of the event
    /// loop, after marking the prefix loaded (`_dartforgeLoadLibrary`).
    pub(super) fn lower_load_library(
        &mut self,
        ast: &ast::Ast,
        target: ExprId,
        span: Span,
    ) -> Operand {
        let ExprKind::Identifier(p) = &ast.expr(target).kind else {
            return self.unsupported("loadLibrary without its prefix", span);
        };
        let p = p.sym;
        let flag = self.deferred_flag(p);
        let symbol = format!("{flag}.mark");
        if self.done_entries.insert(symbol.clone()) {
            let mut e = FnBuilder::new(
                self.ctx,
                self.unit_id,
                symbol.clone(),
                "loadLibrary".to_string(),
                Type::Ref,
            );
            e.add_param("closure".to_string(), Type::Ref);
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Arguments));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            if e.unpack(&[], args, desc).is_some() {
                e.emit(
                    Instruction::StoreGlobal {
                        symbol: flag,
                        val: Operand::Constant(Constant::Int(1)),
                        ty: Type::I64,
                        root: None,
                    },
                    Type::Void,
                );
                e.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
            }
            self.extra_globals
                .extend(std::mem::take(&mut e.extra_globals));
            self.absorb(e);
        }
        let env = self.emit(Instruction::AllocEnv { values: Vec::new() }, Type::Ref);
        let mark = self.emit(
            Instruction::AllocClosure {
                code_symbol: symbol,
                env,
            },
            Type::Ref,
        );
        self.call_support("_dartforgeLoadLibrary", vec![mark], span)
    }
}
