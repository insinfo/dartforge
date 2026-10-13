//! `BestPracticesVerifier` checks on declarations and written types
//! (docs/ANALYZER-ESPECIFICACAO.md §6, family B):
//!
//! * `non_nullable_equals_parameter` (`_checkForNullableEqualsParameterType`,
//!   `an611:src/error/best_practices_verifier.dart:1150-1184`);
//! * `unnecessary_no_such_method` (`_checkForUnnecessaryNoSuchMethod`,
//!   `:1234-1280`);
//! * `unnecessary_question_mark` (`visitNamedType`, `:658-676`).
//!
//! Written without compiling or running (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::BodyTypes;
use crate::table::{CoreTypes, Exibicao, Type, TypeId, TypeTable};
use dartforge_diagnostics::{codigos::warning as w, Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, ExprKind, FunctionBody, StmtKind};
use dartforge_intern::{Interner, SymbolId};

/// `TypeSystemImpl.isNullable`.
fn anulavel(table: &TypeTable, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Never => false,
        Type::Interface { nullable, .. }
        | Type::Function { nullable, .. }
        | Type::Record { nullable, .. }
        | Type::TypeParameter { nullable, .. }
        | Type::ExtensionType { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || anulavel(table, *arg),
        Type::Intersection { bound, .. } => anulavel(table, *bound),
    }
}

/// `NON_NULLABLE_EQUALS_PARAMETER`: an `operator ==` with exactly one
/// parameter (of any kind) whose type, written or inherited, is `Object?` or
/// `dynamic` — at the operator name.
pub fn parametro_de_igualdade_anulavel(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    for (i, f) in program.functions.iter().enumerate() {
        if f.library != lib || (f.class.is_none() && f.extension.is_none()) || interner.resolve(f.name) != "==" {
            continue;
        }
        let FunctionRef::Function { unit, function } = f.node else { continue };
        let af = program.unit(unit).ast.function(function);
        let Some(ps) = &af.parameters else { continue };
        if ps.len() != 1 {
            continue;
        }
        let Some(p) = outline.functions.get(i).and_then(|d| d.parameters.first()) else { continue };
        if matches!(table.exibicao(p.ty), Some(Exibicao::Invalido)) {
            continue;
        }
        let object_or_dynamic = match table.get(p.ty) {
            Type::Dynamic => true,
            Type::Interface { class, .. } => Some(*class) == core.object_class,
            _ => false,
        };
        if !object_or_dynamic || !anulavel(table, p.ty) {
            continue;
        }
        let Some(the_name) = af.name else { continue };
        output.push((unit, Diagnostic::com_codigo(w::NON_NULLABLE_EQUALS_PARAMETER, the_name.span, Vec::<&str>::new())));
    }
    output
}

/// The implementation of `the_name` that `super.the_name` reaches from `c`
/// (`getMember(…, forSuper: true)`): the mixins of `c` from last to first,
/// then the superclass, its mixins and so on; only concrete members.
fn super_member(program: &Program, c: ClassId, the_name: SymbolId) -> Option<FunctionElementId> {
    let concrete = |f: FunctionElementId| !program.function(f).abstract_;
    let mut current = c;
    let mut first = true;
    for _ in 0..256 {
        let cls = program.class(current);
        if !first
            && let Some(&f) = cls.instance_members.get(&the_name)
            && concrete(f)
        {
            return Some(f);
        }
        for &m in cls.mixin_classes.iter().rev() {
            if let Some(&f) = program.class(m).instance_members.get(&the_name)
                && concrete(f)
            {
                return Some(f);
            }
        }
        first = false;
        current = cls.supertype_class?;
    }
    None
}

/// `UNNECESSARY_NO_SUCH_METHOD`: a `noSuchMethod` method whose body is only
/// `super.noSuchMethod(x)` (with `=>` or `{ return …; }`, one argument) and
/// whose called `noSuchMethod` is a method of a class that is not `Object` —
/// at the method name.
pub fn no_such_method_desnecessario(program: &Program, interner: &Interner, core: &CoreTypes, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    for f in program.functions.iter() {
        if f.library != lib || interner.resolve(f.name) != "noSuchMethod" || f.kind != FunctionKind::Function {
            continue;
        }
        let (Some(c), FunctionRef::Function { unit, function }) = (f.class, f.node) else { continue };
        let a = &program.unit(unit).ast;
        let af = a.function(function);
        let invocation = match af.body {
            FunctionBody::Expression(e) => Some(e),
            FunctionBody::Block(s) => match &a.stmt(s).kind {
                StmtKind::Block(body) if body.len() == 1 => match &a.stmt(body[0]).kind {
                    StmtKind::Return(Some(e)) => Some(*e),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        let Some(e) = invocation else { continue };
        let ExprKind::Call { target, arguments } = &a.expr(e).kind else { continue };
        if arguments.args.len() != 1 {
            continue;
        }
        let ExprKind::Property { target: receiver, name, null_aware: false } = &a.expr(*target).kind else { continue };
        if !matches!(a.expr(*receiver).kind, ExprKind::Super) || interner.resolve(name.sym) != "noSuchMethod" {
            continue;
        }
        let Some(callee) = super_member(program, c, f.name) else { continue };
        let fc = program.function(callee);
        let Some(owner) = fc.class else { continue };
        // `methodElement is MethodElement && classElement is ClassElement`.
        if fc.kind != FunctionKind::Function
            || !matches!(program.class(owner).kind, ClassKind::Class | ClassKind::MixinApplication)
            || Some(owner) == core.object_class
        {
            continue;
        }
        let Some(the_name) = af.name else { continue };
        output.push((unit, Diagnostic::com_codigo(w::UNNECESSARY_NO_SUCH_METHOD, the_name.span, Vec::<&str>::new())));
    }
    output
}

/// `UNNECESSARY_QUESTION_MARK`: a named type with `?` that resolves to the
/// `Null` of `dart:core`, or `dynamic` written that way, without an alias — at
/// the `?`, with the qualified name as written.
pub fn interrogacoes_desnecessarias(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    outline: &OutlineTypes,
    bodies: Option<&BodyTypes>,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    for &u in &program.library(lib).units {
        let a = &program.unit(u).ast;
        for (i, ty) in a.types.iter().enumerate() {
            if !ty.nullable {
                continue;
            }
            let ast::TypeKind::Named { name, .. } = &ty.kind else { continue };
            let id = ast::TypeId(i as u32);
            let resolved = outline
                .tipos_escritos
                .get(&(u, id))
                .copied()
                .or_else(|| bodies.and_then(|c| c.units.get(u.0 as usize)).and_then(|b| b.tipos_de_anotacoes.get(&id).copied()));
            let Some(t) = resolved else { continue };
            if matches!(table.exibicao(t), Some(Exibicao::Alias { .. } | Exibicao::Invalido | Exibicao::NeverAnulavel)) {
                continue;
            }
            let last = name.last().map(|n| interner.resolve(n.sym)).unwrap_or("");
            let reports = match table.get(t) {
                Type::Null => true,
                Type::Dynamic => last == "dynamic",
                _ => false,
            };
            if !reports || ty.span.end == ty.span.start {
                continue;
            }
            let text: Vec<&str> = name.iter().map(|n| interner.resolve(n.sym)).collect();
            let qualified = text.join(".");
            let sp = Span { start: ty.span.end - 1, end: ty.span.end };
            output.push((u, Diagnostic::com_codigo(w::UNNECESSARY_QUESTION_MARK, sp, [qualified.as_str()])));
        }
    }
    output
}
