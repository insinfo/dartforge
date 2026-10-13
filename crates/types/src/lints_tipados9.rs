//! `avoid_redundant_argument_values` (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8),
//! written directly from the 3.6.2 emitter, with the constants engine.
//!
//! The argument lists of `MethodInvocation`, of
//! `FunctionExpressionInvocation` whose target is the tear-off of a declared
//! function (the parameters of its type are the element's; a written
//! function type has synthetic parameters, without values), of instance
//! creations (a redirecting factory follows the chain to the last
//! constructor and matches the arguments against its parameters) and of
//! enum constants. From the last argument to the first: an optional
//! parameter, neither `required` nor `@required`, with a known default value
//! (the written one; without it, `null`, or that of the superclass
//! constructor's parameter for a `super.x` whose type fits the parameter's)
//! equal to the argument's constant value (through `ConstantVisitor`, in the
//! node's constant context); the first optional positional ends the scan.
//!
//! Written without compiling or running (2026-10-05).

use crate::constantes::avaliador::{Constante, Ctx, Motor};
use crate::constantes::valor::Valor;
use crate::lints_tipados::Finding;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, TypeTable};
use dartforge_elements::model::{Element, FunctionElementId, FunctionRef, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind, Initializer, MemberKind, ParameterKind, StmtKind};
use dartforge_intern::Interner;
use std::collections::HashSet;

/// The written parameters of a model function, with the unit.
fn parameters_of(program: &Program, f: FunctionElementId) -> Option<(UnitId, &[ast::Parameter])> {
    match program.function(f).node {
        FunctionRef::Function { unit, function } => Some((unit, program.unit(unit).ast.function(function).parameters.as_deref().unwrap_or(&[]))),
        FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
            MemberKind::Constructor(k) => Some((unit, &k.parameters[..])),
            _ => None,
        },
        FunctionRef::None => None,
    }
}

/// The constructor that factory `f` redirects to (`= C.name`).
fn redirected(program: &Program, interner: &Interner, f: FunctionElementId) -> Option<FunctionElementId> {
    let FunctionRef::Constructor { unit, member } = program.function(f).node else { return None };
    let a = &program.unit(unit).ast;
    let MemberKind::Constructor(k) = &a.member(member).kind else { return None };
    if !k.factory {
        return None;
    }
    let r = k.redirect.as_ref()?;
    let (tgt, ctor) = crate::redirection::class_and_constructor(program, unit, r)?;
    let key = match ctor {
        Some(n) => n.sym,
        None => interner.lookup("")?,
    };
    program.class(tgt).constructors.get(&key).copied()
}

/// A written parameter with its owner function (for `super.x`).
#[derive(Clone, Copy)]
struct Param<'p> {
    owner: Option<FunctionElementId>,
    unit: UnitId,
    p: &'p ast::Parameter,
    /// The index in the written list.
    index: usize,
}

struct Cx<'m, 'p> {
    program: &'p Program,
    interner: &'p Interner,
    core: &'p CoreTypes,
    outline: &'p OutlineTypes,
    engine: Motor<'m>,
}

impl<'p> Cx<'_, 'p> {
    fn lib(&self, u: UnitId) -> LibraryId {
        self.program.unit(u).library
    }

    /// `computeConstantValue` of the optional parameter.
    fn pattern(&mut self, q: Param<'p>, prof: u32) -> Option<Valor> {
        if prof > 32 {
            return None;
        }
        if let Some(d) = q.p.default_value {
            let lib = self.lib(q.unit);
            return match self.engine.default_result(q.unit, lib, d) {
                Constante::Valor(v) => Some(v),
                Constante::Invalid(_) => None,
            };
        }
        if !q.p.super_ {
            return Some(Valor::null(self.core));
        }
        // `_superConstructorParameterDefaultValue`.
        let f = q.owner?;
        let sup = self.super_parameter(f, q)?;
        let v = self.pattern(sup, prof + 1)?;
        let own = self.outline.functions.get(f.0 as usize)?.parameters.get(q.index)?.ty;
        let mut env = SubtypeEnv::new(self.engine.table, &self.outline.hierarchy, self.core);
        is_subtype(v.tipo, own, &mut env).then_some(v)
    }

    /// `superConstructorParameter`: the parameter with the same name (named) or
    /// at the same position among the positional `super.x`.
    fn super_parameter(&self, f: FunctionElementId, q: Param<'p>) -> Option<Param<'p>> {
        let program = self.program;
        let FunctionRef::Constructor { unit, member } = program.function(f).node else { return None };
        let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else { return None };
        let class = program.function(f).class?;
        let sup = program.class(class).supertype_class?;
        let name = k.initializers.iter().find_map(|i| match i {
            Initializer::Super { constructor, .. } => Some(*constructor),
            _ => None,
        });
        let key = match name.flatten() {
            Some(n) => n.sym,
            None => self.interner.lookup("")?,
        };
        let g = *program.class(sup).constructors.get(&key)?;
        let (gu, gps) = parameters_of(program, g)?;
        let tgt = if q.p.kind == ParameterKind::Named {
            let n = q.p.nome_externo()?.sym;
            gps.iter().position(|x| x.kind == ParameterKind::Named && x.nome_externo().is_some_and(|m| m.sym == n))?
        } else {
            let i = k.parameters.iter().filter(|x| x.super_ && x.kind != ParameterKind::Named).position(|x| std::ptr::eq(x, q.p))?;
            let positional: Vec<usize> = gps.iter().enumerate().filter(|(_, x)| x.kind != ParameterKind::Named).map(|(j, _)| j).collect();
            *positional.get(i)?
        };
        Some(Param { owner: Some(g), unit: gu, p: &gps[tgt], index: tgt })
    }

    /// `checkArgument`. `report`: the reported node, when it is not the value
    /// (the whole `NamedExpression` on the redirecting factory path, which does
    /// not unwrap it).
    #[allow(clippy::too_many_arguments)]
    fn check(&mut self, u: UnitId, pais: &dartforge_frontend::pais::Pais, value: ExprId, param: Option<Param<'p>>, report: Option<dartforge_diagnostics::Span>, out: &mut Vec<Finding>) {
        let Some(q) = param else { return };
        if q.p.kind != ParameterKind::Optional && q.p.kind != ParameterKind::Named {
            return;
        }
        if q.p.required || q.p.metadata.iter().any(|m| crate::fase_resultado::meta_annotation(self.program, self.interner, q.unit, m, "required")) {
            return;
        }
        let Some(pattern) = self.pattern(q, 0) else { return };
        if pattern.estado.unknown() {
            return;
        }
        let a = &self.program.unit(u).ast;
        let cx = Ctx::simples(u, self.lib(u));
        let in_const = pais.em_contexto_constante(a, value);
        let Constante::Valor(v) = self.engine.avaliar(&cx, value, in_const) else { return };
        if v.estado.unknown() {
            return;
        }
        if self.engine.iguais(&v, &pattern) {
            out.push((report.unwrap_or(a.expr(value).span), "avoid_redundant_argument_values", Vec::new()));
        }
    }

    /// `check`: the `staticParameterElement` of each argument.
    fn list(&mut self, u: UnitId, pais: &dartforge_frontend::pais::Pais, args: &ast::Arguments, tgt: Option<(Option<FunctionElementId>, UnitId, &'p [ast::Parameter])>, out: &mut Vec<Finding>) {
        for i in (0..args.args.len()).rev() {
            let x = args.args[i];
            let param = tgt.and_then(|(owner, pu, ps)| {
                let index = match x.name {
                    Some(n) => ps.iter().position(|p| p.kind == ParameterKind::Named && p.nome_externo().is_some_and(|m| m.sym == n.sym)),
                    None => {
                        let k = args.args[..i].iter().filter(|y| y.name.is_none()).count();
                        ps.iter().enumerate().filter(|(_, p)| p.kind != ParameterKind::Named).nth(k).map(|(j, _)| j)
                    }
                };
                index.map(|j| Param { owner, unit: pu, p: &ps[j], index: j })
            });
            self.check(u, pais, x.value, param, None, out);
            if param.is_some_and(|q| q.p.kind == ParameterKind::Optional) {
                break;
            }
        }
    }

    /// `visitInstanceCreationExpression`.
    fn creation(&mut self, u: UnitId, pais: &dartforge_frontend::pais::Pais, args: &ast::Arguments, ctor: Option<FunctionElementId>, out: &mut Vec<Finding>) {
        let Some(k) = ctor else {
            self.list(u, pais, args, None, out);
            return;
        };
        let target_of = |f: FunctionElementId| parameters_of(self.program, f).map(|(pu, ps)| (Some(f), pu, ps));
        let factory = matches!(self.program.function(k).node, FunctionRef::Constructor { unit, member } if matches!(&self.program.unit(unit).ast.member(member).kind, MemberKind::Constructor(c) if c.factory));
        let mut redirected_to = if factory { redirected(self.program, self.interner, k) } else { None };
        if !factory || redirected_to.is_none() {
            let tgt = target_of(k);
            self.list(u, pais, args, tgt, out);
            return;
        }
        let mut seen: HashSet<FunctionElementId> = HashSet::from([k]);
        let mut current = k;
        while let Some(r) = redirected_to {
            if !seen.insert(r) {
                // Cycle: compile error.
                return;
            }
            current = r;
            redirected_to = redirected(self.program, self.interner, r);
        }
        let Some((owner, pu, ps)) = target_of(current) else { return };
        if args.args.is_empty() {
            return;
        }
        for i in (0..args.args.len()).rev() {
            let x = args.args[i];
            let index = match x.name {
                Some(n) => ps.iter().position(|p| p.kind == ParameterKind::Named && p.name.is_some_and(|m| m.sym == n.sym)),
                None => {
                    let count = args.args[..=i].iter().filter(|y| y.name.is_none()).count();
                    let j = count - 1;
                    (j < ps.len() && ps[j].kind != ParameterKind::Named).then_some(j)
                }
            };
            let param = index.map(|j| Param { owner, unit: pu, p: &ps[j], index: j });
            let report = x.name.map(|n| dartforge_diagnostics::Span { start: n.span.start, end: self.program.unit(u).ast.expr(x.value).span.end });
            self.check(u, pais, x.value, param, report, out);
            if param.is_some_and(|q| q.p.kind == ParameterKind::Optional) {
                break;
            }
        }
    }
}

/// The findings in unit `u`.
#[allow(clippy::too_many_arguments)]
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    u: UnitId,
) -> Vec<Finding> {
    let mut out = Vec::new();
    let unit_id = program.unit(u);
    let a = &unit_id.ast;
    let body = &bodies.units[u.0 as usize];
    let before_3 = program.library(unit_id.library).features.versao().major < 3;
    let pais = dartforge_frontend::pais::Pais::novo(a, &unit_id.unit, unit_id.source.as_str(), before_3);
    let engine = Motor::novo(program, interner, table, core, outline, bodies, the_inferred);
    let mut cx = Cx { program, interner, core, outline, engine };
    // The local function declared at `d` (the name's offset).
    let local = |d: usize| -> Option<&[ast::Parameter]> {
        a.stmts.iter().find_map(|s| match &s.kind {
            StmtKind::Function(g) if a.function(*g).name.is_some_and(|n| n.span.start == d) => Some(a.function(*g).parameters.as_deref().unwrap_or(&[])),
            _ => None,
        })
    };
    // The parameters of the function invoked by name (`MethodInvocation`) or
    // through the tear-off (`FunctionExpressionInvocation`).
    let of_function = |e: ExprId| -> Option<(Option<FunctionElementId>, UnitId, &[ast::Parameter])> {
        let f = match body.get_resolved(e)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => *f,
            Resolved::ExtensionMember { member, .. } => *member,
            Resolved::Local(_) => return local(body.declaracao_local(e)?).map(|ps| (None, u, ps)),
            _ => return None,
        };
        if program.function(f).kind == dartforge_elements::model::FunctionKind::Getter {
            return None;
        }
        parameters_of(program, f).map(|(pu, ps)| (Some(f), pu, ps))
    };
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        match &e.kind {
            ExprKind::InstanceCreation { arguments, .. } => {
                let ctor = match body.get_resolved(id) {
                    Some(Resolved::Constructor(f)) => Some(*f),
                    _ => None,
                };
                cx.creation(u, &pais, arguments, ctor, &mut out);
            }
            ExprKind::Call { target, arguments } => {
                if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                    cx.creation(u, &pais, arguments, Some(*f), &mut out);
                    continue;
                }
                if arguments.args.is_empty() {
                    continue;
                }
                if crate::lints_tipados4::is_method_invocation(program, a, body, *target) {
                    let tgt = of_function(*target);
                    cx.list(u, &pais, arguments, tgt, &mut out);
                    continue;
                }
                // `FunctionExpressionInvocation`: the tear-off of a declared
                // function.
                let mut f = *target;
                while let ExprKind::Parenthesized(x) = &a.expr(f).kind {
                    f = *x;
                }
                let tgt = if matches!(a.expr(f).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) { of_function(f) } else { None };
                cx.list(u, &pais, arguments, tgt, &mut out);
            }
            _ => {}
        }
    }
    // `visitEnumConstantArguments`.
    for (di, d) in a.decls.iter().enumerate() {
        let DeclKind::Enum(x) = &d.kind else { continue };
        let Some(class) = (0..program.classes.len())
            .map(|i| dartforge_elements::model::ClassId(i as u32))
            .find(|c| program.class(*c).decl.is_some_and(|r| r.unit == u && r.decl == ast::DeclId(di as u32)))
        else {
            continue;
        };
        for k in x.constants.iter() {
            let Some(args) = &k.arguments else { continue };
            let key = match k.constructor {
                Some(n) => Some(n.sym),
                None => interner.lookup(""),
            };
            let tgt = key
                .and_then(|c| program.class(class).constructors.get(&c).copied())
                .and_then(|f| parameters_of(program, f).map(|(pu, ps)| (Some(f), pu, ps)));
            cx.list(u, &pais, args, tgt, &mut out);
        }
    }
    out
}
