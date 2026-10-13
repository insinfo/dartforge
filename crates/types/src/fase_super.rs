//! The analyzer's `MustCallSuperVerifier`
//! (`analyzer/lib/src/error/must_call_super_verifier.dart`;
//! docs/ANALYZER-ESPECIFICACAO-INFRA.md, batch II.8): `must_call_super`, at
//! the name of a non-abstract instance method, getter or setter that
//! overrides a member annotated with `@mustCallSuper`, has an inherited
//! concrete implementation, and whose body does not call
//! `super.<the same member>`.
//!
//! The annotation is recognized by name (`mustCallSuper`, with or without an
//! import prefix), without checking that it comes from `package:meta`. The
//! `super` call is searched for in the body tree, nested functions included.
//! Written without compiling or running (2026-10-04).

use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, FunctionElementId, FunctionKind as ElementKind, FunctionRef, LibraryId, Program, UnitId, VariableRef,
};
use dartforge_frontend::ast::{self, AssignOp, DeclKind, ExprKind, FunctionKind, MemberKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashSet, VecDeque};

/// The annotation list has `@mustCallSuper` (or `@prefixo.mustCallSuper`).
fn annotated(metadata: &[ast::Annotation], sym: SymbolId) -> bool {
    metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| n.sym == sym))
}

/// The annotations of a member's declaration: the method's, or the field's
/// for an implicit accessor.
fn annotations_of(program: &Program, f: FunctionElementId) -> &[ast::Annotation] {
    let element = program.function(f);
    if let Some(v) = element.variable {
        return match program.variable(v).node {
            VariableRef::Field { unit, member, .. } => &program.unit(unit).ast.member(member).metadata,
            _ => &[],
        };
    }
    let (FunctionRef::Function { unit, function }, Some(class)) = (element.node, element.class) else { return &[] };
    let Some(decl) = program.class(class).decl else { return &[] };
    let a = &program.unit(unit).ast;
    if decl.unit != unit {
        return &[];
    }
    let members: &[ast::MemberId] = match &a.decl(decl.decl).kind {
        DeclKind::Class(x) => &x.members,
        DeclKind::Enum(x) => &x.members,
        DeclKind::Mixin(x) => &x.members,
        _ => return &[],
    };
    members
        .iter()
        .map(|m| a.member(*m))
        .find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function))
        .map_or(&[][..], |m| &m.metadata[..])
}

/// `_findOverriddenMemberWithMustCallSuper`: breadth-first through the
/// mixins, the superclass and (in a mixin) the `on` constraints, without the
/// interfaces; the first ancestor with a member named `the_name` decides.
fn annotated_above(program: &Program, interner: &Interner, class: ClassId, lib: LibraryId, keys: &[SymbolId], sym: SymbolId) -> Option<ClassId> {
    let private = keys.first().is_some_and(|c| interner.resolve(*c).starts_with('_'));
    let mut queue: VecDeque<ClassId> = VecDeque::new();
    let enqueue = |queue: &mut VecDeque<ClassId>, c: ClassId| {
        let e = program.class(c);
        queue.extend(e.mixin_classes.iter().copied());
        queue.extend(e.supertype_class);
        if e.kind == ClassKind::Mixin {
            queue.extend(e.on_classes.iter().copied());
        }
    };
    let mut seen: HashSet<ClassId> = HashSet::new();
    enqueue(&mut queue, class);
    while let Some(ancestral) = queue.pop_front() {
        if !seen.insert(ancestral) {
            continue;
        }
        let e = program.class(ancestral);
        if !private || e.library == lib {
            // `getMethod(name) ?? getGetter(name) ?? getSetter(name)`.
            let mem = keys.iter().find_map(|c| e.instance_members.get(c).copied());
            if let Some(m) = mem
                && annotated(annotations_of(program, m), sym)
            {
                return Some(ancestral);
            }
        }
        enqueue(&mut queue, ancestral);
    }
    None
}

/// `lookUpConcrete…` starting from `start`: the class itself (with
/// `with_itself`), its mixins from last to first, and so on along the
/// superclass chain; the first non-abstract member of the requested kind.
pub(crate) fn the_concrete(program: &Program, start: ClassId, with_itself: bool, key: SymbolId, lib: LibraryId, private: bool, method: bool) -> bool {
    let serve = |c: ClassId| {
        let e = program.class(c);
        if private && e.library != lib {
            return false;
        }
        e.instance_members.get(&key).is_some_and(|f| {
            let m = program.function(*f);
            let is_method = matches!(m.kind, ElementKind::Function | ElementKind::Operator);
            !m.abstract_ && is_method == method
        })
    };
    let mut seen: HashSet<ClassId> = HashSet::new();
    let mut current = Some(start);
    let mut first = true;
    while let Some(c) = current {
        if !seen.insert(c) {
            break;
        }
        if (!first || with_itself) && serve(c) {
            return true;
        }
        first = false;
        let e = program.class(c);
        if e.mixin_classes.iter().rev().any(|m| serve(*m)) {
            return true;
        }
        current = e.supertype_class;
    }
    false
}

/// `invokesSuperSelf`: the body of `the_function` has `super.<the_name>` in the right
/// role (read or call for a method or getter, write for a setter), or the
/// operator applied to `super`.
fn calls_super(a: &ast::Ast, source: &str, interner: &Interner, the_function: &ast::Function, the_name: &str, kind: FunctionKind) -> bool {
    let inside = |s: Span| the_function.span.start <= s.start && s.end <= the_function.span.end;
    let e_super = |e: ast::ExprId| matches!(a.expr(e).kind, ExprKind::Super);
    // Simple assignment targets (`=`): they are not reads.
    let mut only_written: HashSet<(usize, usize)> = HashSet::new();
    let mut written_ones: Vec<ast::ExprId> = Vec::new();
    for e in a.exprs.iter().filter(|e| inside(e.span)) {
        if let ExprKind::Assign { op, target, .. } = &e.kind {
            written_ones.push(*target);
            if matches!(op, AssignOp::Assign) {
                let s = a.expr(*target).span;
                only_written.insert((s.start, s.end));
            }
        }
    }
    match kind {
        FunctionKind::Setter => written_ones.iter().any(|t| match &a.expr(*t).kind {
            ExprKind::Property { target, name, .. } => e_super(*target) && interner.resolve(name.sym) == the_name,
            _ => false,
        }),
        FunctionKind::Function | FunctionKind::Getter => a.exprs.iter().filter(|e| inside(e.span)).any(|e| match &e.kind {
            ExprKind::Property { target, name, .. } => {
                e_super(*target) && interner.resolve(name.sym) == the_name && !only_written.contains(&(e.span.start, e.span.end))
            }
            _ => false,
        }),
        FunctionKind::Operator => {
            if the_name == "[]=" {
                return written_ones.iter().any(|t| matches!(&a.expr(*t).kind, ExprKind::Index { target, .. } if e_super(*target)));
            }
            a.exprs.iter().filter(|e| inside(e.span)).any(|e| match &e.kind {
                ExprKind::Index { target, .. } => the_name == "[]" && e_super(*target) && !only_written.contains(&(e.span.start, e.span.end)),
                ExprKind::Unary { op: UnaryOp::Neg, operand } => the_name == "unary-" && e_super(*operand),
                ExprKind::Unary { op: UnaryOp::BitNot, operand } => the_name == "~" && e_super(*operand),
                ExprKind::Binary { left, right, .. } if e_super(*left) => {
                    // The operator text, between the operands; `!=` calls `==`.
                    let (of, until) = (a.expr(*left).span.end, a.expr(*right).span.start);
                    let text = source.get(of..until).map_or("", str::trim);
                    text == the_name || (text == "!=" && the_name == "==")
                }
                _ => false,
            })
        }
    }
}

/// The `must_call_super` findings of library `lib`.
pub fn sem_chamada_ao_super(program: &Program, interner: &Interner, lib: LibraryId) -> Vec<(UnitId, Diagnostic)> {
    let mut output = Vec::new();
    let Some(sym) = interner.lookup("mustCallSuper") else { return output };
    for (i, class) in program.classes.iter().enumerate() {
        if class.library != lib || !matches!(class.kind, ClassKind::Class | ClassKind::Enum | ClassKind::Mixin) {
            continue;
        }
        let Some(decl) = class.decl else { continue };
        let cid = ClassId(i as u32);
        let unit_id = program.unit(decl.unit);
        let a = &unit_id.ast;
        let members: &[ast::MemberId] = match &a.decl(decl.decl).kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            _ => continue,
        };
        for &mid in members {
            let MemberKind::Method(f) = &a.member(mid).kind else { continue };
            let func = a.function(*f);
            // Static or abstract (no body and no `external`): skipped.
            if func.static_ || (matches!(func.body, ast::FunctionBody::Empty) && !func.external) {
                continue;
            }
            let Some(the_name) = func.name else { continue };
            let text = interner.resolve(the_name.sym);
            let private = text.starts_with('_');
            let setter_key = interner.lookup(&format!("{text}_="));
            let keys: Vec<SymbolId> = match func.kind {
                FunctionKind::Setter => setter_key.into_iter().collect(),
                _ => std::iter::once(the_name.sym).chain(setter_key).collect(),
            };
            let Some(owner) = annotated_above(program, interner, cid, lib, &keys, sym) else { continue };
            // The inherited concrete implementation.
            let has_concrete = match func.kind {
                // `_hasConcreteSuperMethod`: the superclass, the mixins or the
                // constraints, each with its own chain.
                FunctionKind::Function | FunctionKind::Operator => class
                    .supertype_class
                    .iter()
                    .chain(class.mixin_classes.iter())
                    .chain(class.on_classes.iter().filter(|_| class.kind == ClassKind::Mixin))
                    .any(|c| the_concrete(program, *c, true, the_name.sym, lib, private, true)),
                // `lookUpInheritedConcreteGetter` / `…Setter`.
                FunctionKind::Getter => {
                    the_concrete(program, cid, false, the_name.sym, lib, private, false)
                        || (class.kind == ClassKind::Mixin
                            && class.on_classes.iter().any(|c| the_concrete(program, *c, true, the_name.sym, lib, private, false)))
                }
                FunctionKind::Setter => setter_key.is_some_and(|k| {
                    the_concrete(program, cid, false, k, lib, private, false)
                        || (class.kind == ClassKind::Mixin
                            && class.on_classes.iter().any(|c| the_concrete(program, *c, true, k, lib, private, false)))
                }),
            };
            if !has_concrete {
                continue;
            }
            if !calls_super(a, &unit_id.source, interner, func, text, func.kind) {
                let owner_name = interner.resolve(program.class(owner).name);
                output.push((decl.unit, Diagnostic::com_codigo(w::MUST_CALL_SUPER, the_name.span, [owner_name])));
            }
        }
    }
    output
}
