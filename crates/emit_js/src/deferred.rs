//! `import … deferred as p` on the web, as DDC compiles it: every module is
//! loaded with the program, `p.loadLibrary()` is the runtime's `loadLibrary`
//! (which marks the prefix loaded), and every access through `p` checks it
//! first (`checkDeferredIsLoaded`, `DeferredNotLoadedError` otherwise).

use crate::body::FnEmitter;
use crate::expr::IdentTarget;
use crate::js::{self, Js};
use crate::ty::Ty;
use dartforge_frontend::ast::{ExprId, ExprKind, TypeKind};
use dartforge_intern::SymbolId;

impl FnEmitter<'_, '_> {
    /// `sym` is the prefix of an `import … deferred as` of the current library.
    pub(crate) fn deferred_import(&self, sym: SymbolId) -> bool {
        self.ctx
            .program
            .library(self.lib)
            .imports
            .iter()
            .any(|i| i.deferred && i.prefix == Some(sym))
    }

    /// The deferred prefix `e` is, when it is one.
    fn deferred_prefix_of(&self, e: ExprId) -> Option<SymbolId> {
        let ExprKind::Identifier(id) = &self.expr(e).kind else {
            return None;
        };
        match self.ident_target(id.sym, e) {
            IdentTarget::Prefix(p) if self.deferred_import(p) => Some(p),
            _ => None,
        }
    }

    /// The deferred prefix the expression `e` goes through (`p.x`, `p.f()`,
    /// `p.x = v`, `p.x++`, `p.C.x`, `new p.C()`), with the inner expression the
    /// outer check covers; `p.loadLibrary()` is not an access.
    fn deferred_access(&self, e: ExprId) -> Option<(SymbolId, Option<ExprId>)> {
        match &self.expr(e).kind {
            ExprKind::Property { target, name, .. } => {
                if let Some(p) = self.deferred_prefix_of(*target) {
                    return (self.name(name.sym) != "loadLibrary").then_some((p, None));
                }
                self.deferred_access(*target)
                    .map(|(p, _)| (p, Some(*target)))
            }
            ExprKind::Call { target, .. } | ExprKind::Index { target, .. } => self
                .deferred_access(*target)
                .map(|(p, _)| (p, Some(*target))),
            ExprKind::Assign { target, .. } => self
                .deferred_access(*target)
                .map(|(p, _)| (p, Some(*target))),
            ExprKind::Unary { operand, .. } => self
                .deferred_access(*operand)
                .map(|(p, _)| (p, Some(*operand))),
            ExprKind::InstanceCreation { ty, .. } => match &self.ast().ty(*ty).kind {
                TypeKind::Named { name, .. } if name.len() == 2 => self
                    .deferred_import(name[0].sym)
                    .then_some((name[0].sym, None)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The check that comes before the expression `e` when it goes through a
    /// deferred prefix: `dart.checkDeferredIsLoaded(library, prefix)`.
    pub(crate) fn deferred_check(&mut self, e: ExprId) -> Option<String> {
        if self.deferred_checked.remove(&e)
            || !self
                .ctx
                .program
                .library(self.lib)
                .imports
                .iter()
                .any(|i| i.deferred)
        {
            return None;
        }
        let (p, inner) = self.deferred_access(e)?;
        if let Some(inner) = inner {
            self.deferred_checked.insert(inner);
        }
        Some(format!(
            "dart.checkDeferredIsLoaded({}, {})",
            js::string_literal(&self.ctx.program.library(self.lib).uri),
            js::string_literal(self.name(p))
        ))
    }

    /// `p.loadLibrary()`: the runtime's `loadLibrary`, which marks the prefix
    /// loaded and returns a `Future<void>`.
    pub(crate) fn load_library(&mut self, p: SymbolId) -> (Js, Ty) {
        let t = self.ctx.t_future(Ty::Void);
        (
            Js::prim(format!(
                "dart.loadLibrary({}, {}, \"\")",
                js::string_literal(&self.ctx.program.library(self.lib).uri),
                js::string_literal(self.name(p))
            )),
            t,
        )
    }
}
