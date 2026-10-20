//! The linter's `resolveNameInScope` (`util/scope.dart`) over the DartForge
//! tree: the scope of the first ancestor with a scope of the
//! `ScopeResolverVisitor` and its chain, by source regions.
//!
//! Each local declaration comes in with the region where the name holds, from
//! the innermost to the outermost: a function's type parameters hold in the
//! whole function; the parameters, in the body; a block's locals (variables,
//! pattern declarations and local functions), in the whole block (the
//! `LocalScope` kept in the block receives all of them); those of a `case`,
//! in the `case`; those of a `for`, in the statement; those of a `catch`, in
//! the body; a class's declared members (the `InstanceScope`: accessors and
//! methods, instance and static), in the methods and constructor bodies; a
//! class's type parameters, in the declaration. For equal regions, the order
//! breaks the tie (a method's `TypeParameterScope` is inside the class's
//! `InstanceScope`). The scope is the first one with the name, as a getter or
//! as a setter (`EnclosedScope.lookup`); a local is only a getter.
//!
//! Known deviation: the variables of collection-element `for` and `if case`
//! and those of switch expression cases do not enter the regions.
//! Written without compiling or running (2026-10-05).

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, DeclId, DeclKind, ForInTarget, ForInit, FunctionBody, MemberKind, PatternId,
    PatternKind, StmtId, StmtKind,
};
use dartforge_intern::SymbolId;

/// What the name is in a region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    /// Variable, parameter or local function (getter only).
    Local,
    /// Type parameter (of a function or a class).
    TypeParameter,
    /// Declared member of class `owner` (the getter or the setter).
    Member { owner: DeclId, setter: bool },
}

/// A declaration with the region where the name holds.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub name: SymbolId,
    pub region: Span,
    /// Tie-break between equal regions (smaller is more inner).
    pub order: u8,
    pub kind: EntryKind,
    /// The start of the declared name.
    pub decl_start: usize,
}

/// The unit's regions.
pub struct Scopes {
    entries: Vec<Entry>,
}

fn within(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

fn body_region(a: &Ast, b: &FunctionBody) -> Option<Span> {
    match b {
        FunctionBody::Block(s) => Some(a.stmt(*s).span),
        FunctionBody::Expression(e) => Some(a.expr(*e).span),
        _ => None,
    }
}

/// The variables declared by a pattern.
pub fn pattern_names(a: &Ast, p: PatternId) -> Vec<ast::Name> {
    let mut ps = Vec::new();
    super::rules9::pattern_variables(a, p, &mut ps);
    ps.into_iter()
        .filter_map(|x| match &a.pattern(x).kind {
            PatternKind::Variable { name, .. } => Some(*name),
            _ => None,
        })
        .collect()
}

/// The locals declared directly in a statement list.
fn block_locals(a: &Ast, stmts: &[StmtId]) -> Vec<ast::Name> {
    let mut v = Vec::new();
    for &s in stmts {
        let mut s = s;
        while let StmtKind::Labeled { body, .. } = &a.stmt(s).kind {
            s = *body;
        }
        match &a.stmt(s).kind {
            StmtKind::Variables(l) => v.extend(l.variables.iter().map(|x| x.name)),
            StmtKind::PatternVariables { pattern, .. } => v.extend(pattern_names(a, *pattern)),
            StmtKind::Function(f) => v.extend(a.function(*f).name),
            _ => {}
        }
    }
    v
}

impl Scopes {
    pub fn new(a: &Ast) -> Scopes {
        let mut v: Vec<Entry> = Vec::new();
        let add = |names: &mut dyn Iterator<Item = ast::Name>,
                   region: Span,
                   order: u8,
                   kind: EntryKind,
                   v: &mut Vec<Entry>| {
            for n in names {
                v.push(Entry {
                    name: n.sym,
                    region,
                    order,
                    kind,
                    decl_start: n.span.start,
                });
            }
        };
        for f in a.functions.iter() {
            add(
                &mut f.type_params.iter().map(|t| t.name),
                f.span,
                0,
                EntryKind::TypeParameter,
                &mut v,
            );
            if let (Some(ps), Some(body)) = (&f.parameters, body_region(a, &f.body)) {
                add(
                    &mut ps.iter().filter_map(|p| p.name),
                    body,
                    0,
                    EntryKind::Local,
                    &mut v,
                );
            }
        }
        for (k, d) in a.decls.iter().enumerate() {
            let did = DeclId(k as u32);
            let (tps, members): (&[ast::TypeParameter], &[ast::MemberId]) = match &d.kind {
                DeclKind::Class(x) => (&x.type_params, &x.members),
                DeclKind::Mixin(x) => (&x.type_params, &x.members),
                DeclKind::Enum(x) => (&x.type_params, &x.members),
                DeclKind::Extension(x) => (&x.type_params, &x.members),
                DeclKind::ExtensionType(x) => (&x.type_params, &x.members),
                _ => continue,
            };
            add(
                &mut tps.iter().map(|t| t.name),
                d.span,
                2,
                EntryKind::TypeParameter,
                &mut v,
            );
            // The `InstanceScope`: getters (fields, getters, methods, enum
            // constants, the representation) and setters (fields that are not
            // `final`/`const`, setters).
            let mut getters: Vec<ast::Name> = Vec::new();
            let mut setters: Vec<ast::Name> = Vec::new();
            for &m in members {
                match &a.member(m).kind {
                    MemberKind::Field(l) => {
                        getters.extend(l.variables.iter().map(|x| x.name));
                        if !l.final_ && !l.const_ {
                            setters.extend(l.variables.iter().map(|x| x.name));
                        }
                    }
                    MemberKind::Method(f) => {
                        let f = a.function(*f);
                        if let Some(n) = f.name {
                            if f.kind == ast::FunctionKind::Setter {
                                setters.push(n);
                            } else {
                                getters.push(n);
                            }
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
            if let DeclKind::Enum(x) = &d.kind {
                getters.extend(x.constants.iter().map(|k| k.name));
            }
            if let DeclKind::ExtensionType(x) = &d.kind {
                getters.push(x.representation_name);
            }
            for &m in members {
                let region = match &a.member(m).kind {
                    MemberKind::Method(f) => Some(a.function(*f).span),
                    MemberKind::Constructor(k) => body_region(a, &k.body),
                    MemberKind::Field(_) => None,
                };
                if let Some(r) = region {
                    add(
                        &mut getters.iter().copied(),
                        r,
                        1,
                        EntryKind::Member {
                            owner: did,
                            setter: false,
                        },
                        &mut v,
                    );
                    add(
                        &mut setters.iter().copied(),
                        r,
                        1,
                        EntryKind::Member {
                            owner: did,
                            setter: true,
                        },
                        &mut v,
                    );
                }
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind
                && let Some(body) = body_region(a, &k.body)
            {
                add(
                    &mut k.parameters.iter().filter_map(|p| p.name),
                    body,
                    0,
                    EntryKind::Local,
                    &mut v,
                );
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Block(stmts) => add(
                    &mut block_locals(a, stmts).into_iter(),
                    s.span,
                    0,
                    EntryKind::Local,
                    &mut v,
                ),
                StmtKind::Switch { cases, .. } => {
                    for case in cases.iter() {
                        let mut names = block_locals(a, &case.body);
                        if let Some(p) = case.pattern {
                            names.extend(pattern_names(a, p));
                        }
                        add(
                            &mut names.into_iter(),
                            case.span,
                            0,
                            EntryKind::Local,
                            &mut v,
                        );
                    }
                }
                StmtKind::Try { catches, .. } => {
                    for k in catches.iter() {
                        add(
                            &mut k.exception.into_iter().chain(k.stack_trace),
                            a.stmt(k.body).span,
                            0,
                            EntryKind::Local,
                            &mut v,
                        );
                    }
                }
                StmtKind::For { init, .. } => {
                    let names: Vec<ast::Name> = match init {
                        Some(ForInit::Variables(l)) => l.variables.iter().map(|x| x.name).collect(),
                        Some(ForInit::Pattern { pattern, .. }) => pattern_names(a, *pattern),
                        _ => Vec::new(),
                    };
                    add(&mut names.into_iter(), s.span, 0, EntryKind::Local, &mut v);
                }
                StmtKind::ForIn { target, .. } => {
                    let names: Vec<ast::Name> = match target {
                        ForInTarget::Declared { name, .. } => vec![*name],
                        ForInTarget::Pattern { pattern, .. } => pattern_names(a, *pattern),
                        ForInTarget::Expression(_) => Vec::new(),
                    };
                    add(&mut names.into_iter(), s.span, 0, EntryKind::Local, &mut v);
                }
                StmtKind::If {
                    case_pattern: Some(p),
                    then,
                    ..
                } => {
                    let region = Span {
                        start: a.pattern(*p).span.start,
                        end: a.stmt(*then).span.end,
                    };
                    add(
                        &mut pattern_names(a, *p).into_iter(),
                        region,
                        0,
                        EntryKind::Local,
                        &mut v,
                    );
                }
                _ => {}
            }
        }
        Scopes { entries: v }
    }

    /// The entries of the innermost scope at `pos` that has `name` (as a
    /// getter or setter); empty when no local or class scope has it.
    pub fn lookup(&self, name: SymbolId, pos: usize) -> Vec<&Entry> {
        let point = Span {
            start: pos,
            end: pos,
        };
        let candidates: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|r| r.name == name && within(point, r.region))
            .collect();
        let Some(best) = candidates
            .iter()
            .map(|r| (r.region.end - r.region.start, r.order))
            .min()
        else {
            return Vec::new();
        };
        candidates
            .into_iter()
            .filter(|r| (r.region.end - r.region.start, r.order) == best)
            .collect()
    }
}
