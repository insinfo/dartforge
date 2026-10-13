//! Checking the emitter's inference against the shared inference.
//!
//! The JS emitter deduces the type of each expression with its own system
//! ([`Ty`], `emit_expr` returning `(Js, Ty)`), while `crates/types` already
//! provides the static type and the resolved element of each node in
//! [`dartforge_types::resolved::BodyTypes`] — which the native backend consumes
//! directly. This module is the first step towards a single source: with
//! `DARTFORGE_JS_CONFERIR_TIPOS` set, each emitted expression (outside
//! speculative emission) has its `Ty` compared with the shared type, converted
//! to `Ty`, and each identifier has the target the emitter resolved compared
//! with the shared `Resolved`. Divergences are recorded; the emission does not change.
//!
//! Values of the variable:
//! - `1` (or any non-empty value that is not a path): summary per category on
//!   `stderr` at the end of emission;
//! - a file path ending in `.tsv`: besides the summary, each divergence becomes
//!   a line appended to the file (file:line, node kind, category, expression
//!   text, emitter type, shared type, path, start and length in bytes — these
//!   three to cross-reference with the `package:analyzer` oracle,
//!   `tools/oraculo_tipos`). Each emission first appends a `#totais` line
//!   (checked expressions, checked identifiers, skipped write targets).
//!
//! The comparison normalizes what is only representation (see [`normalize`] and
//! [`equivalent`]): `FutureOr` as a class or as `Ty::FutureOr`, the `Null` class
//! as `Ty::Null`, type parameters with the same name but distinct ids (the
//! emitter creates its own ids for local generic functions) and bound
//! parameters of function types, compared by position.
//! The corpus inventory is in `docs/INFERENCIA-JS-ALINHAMENTO.md`.

use crate::body::FnEmitter;
use crate::ctx::{Ctx, Member, MemberKind};
use crate::expr::IdentTarget;
use crate::ty::{Ty, TyParam};
use dartforge_elements::model::{Element, UnitId};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeId};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write as _;

/// Name of the environment variable that turns the check on.
pub const ENV_VAR: &str = "DARTFORGE_JS_CONFERIR_TIPOS";

/// A divergence between the emitter and the shared inference.
#[derive(Clone, Debug)]
pub struct Divergence {
    /// `file:line` of the expression.
    pub location: String,
    /// Node kind (`Identifier`, `Call`, …).
    pub node: String,
    /// `kind` (type) or `target` (target); kept in Portuguese because it is a
    /// column value of the `.tsv` read by `scripts/conferir-tipos-js.mjs`.
    pub kind: &'static str,
    pub category: String,
    /// Text of the expression in the source (one line, truncated).
    pub text: String,
    pub emitter: String,
    pub common: String,
    /// Path of the unit and byte range of the expression, to cross-reference
    /// with the analyzer oracle (`tools/oraculo_tipos`).
    pub path: String,
    pub start: usize,
    pub length: usize,
}

/// State of the check for one emission.
#[derive(Default)]
pub struct TypeCheck {
    /// Nodes already checked: the emitter emits the same expression more than
    /// once (different contexts, re-emissions); the first real emission counts.
    types_seen: HashSet<(u32, u32)>,
    targets_seen: HashSet<(u32, u32)>,
    /// Targets of compound assignment and of `++`/`--` that were emitted but
    /// not checked ([`Ctx::is_write_target`]).
    pub skipped_write_targets: usize,
    pub checked_types: usize,
    pub checked_targets: usize,
    /// Identifiers whose target came from the shared resolution
    /// ([`FnEmitter::ident_target`]) and those that kept the emitter's
    /// own lookup, counted once per node.
    origin_seen: HashSet<(u32, u32)>,
    pub targets_from_shared: usize,
    pub targets_from_own_lookup: usize,
    /// Member accesses with an explicit receiver (`a.b`, `a.m()`, `a.b = v`)
    /// whose member came from the shared resolution ([`FnEmitter::access_member`])
    /// and those that kept the own lookup, once per node and kind.
    member_seen: HashSet<(u32, u32, bool)>,
    pub members_from_shared: usize,
    pub members_from_own_lookup: usize,
    /// Generic calls (and constructors of a generic class without written type
    /// arguments) instantiated by the shared inference and by the emitter's own
    /// deduction (`emit_args_infer`), once per argument list.
    instantiation_seen: HashSet<(u32, usize)>,
    pub instantiations_from_shared: usize,
    pub own_instantiations: usize,
    pub divergences: Vec<Divergence>,
    /// Path of the `.tsv` requested in the variable, if any.
    output: Option<std::path::PathBuf>,
}

impl TypeCheck {
    /// The check requested by the environment, or `None` when it is off.
    pub fn from_env() -> Option<TypeCheck> {
        let v = std::env::var(ENV_VAR)
            .ok()
            .filter(|v| !v.is_empty() && v != "0")?;
        let output = v.ends_with(".tsv").then(|| std::path::PathBuf::from(&v));
        Some(TypeCheck {
            output,
            ..Default::default()
        })
    }

    /// Summary per category on `stderr` and, if requested, the divergences in the `.tsv`.
    pub fn report(&self) {
        let (mut types, mut targets) = (0usize, 0usize);
        let mut by_category: BTreeMap<(&str, &str), usize> = BTreeMap::new();
        for d in &self.divergences {
            if d.kind == "tipo" {
                types += 1
            } else {
                targets += 1
            }
            *by_category
                .entry((d.kind, d.category.as_str()))
                .or_default() += 1;
        }
        eprintln!(
            "type check: {} expressions, {types} diverge ({} write targets not checked); {} identifiers, {targets} with a divergent target; target from the shared resolution in {}, from the own lookup in {}; member with receiver from the shared resolution in {}, from the own lookup in {}; generic instantiation from the shared inference in {}, own in {}",
            self.checked_types,
            self.skipped_write_targets,
            self.checked_targets,
            self.targets_from_shared,
            self.targets_from_own_lookup,
            self.members_from_shared,
            self.members_from_own_lookup,
            self.instantiations_from_shared,
            self.own_instantiations
        );
        let mut v: Vec<_> = by_category.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for ((kind, cat), n) in v {
            eprintln!("{n:7}  {kind}: {cat}");
        }
        let Some(p) = &self.output else { return };
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p);
        match file {
            Ok(mut f) => {
                let mut text = String::new();
                // Totals line, for whoever sums several programs.
                text.push_str(&format!(
                    "#totais\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    self.checked_types,
                    self.checked_targets,
                    self.skipped_write_targets,
                    self.targets_from_shared,
                    self.targets_from_own_lookup,
                    self.instantiations_from_shared,
                    self.own_instantiations,
                    self.members_from_shared,
                    self.members_from_own_lookup
                ));
                for d in &self.divergences {
                    text.push_str(&format!(
                        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                        d.location,
                        d.node,
                        d.kind,
                        d.category,
                        d.text,
                        d.emitter,
                        d.common,
                        d.path,
                        d.start,
                        d.length
                    ));
                }
                if let Err(e) = f.write_all(text.as_bytes()) {
                    eprintln!("warning: {}: {e}", p.display());
                }
            }
            Err(e) => eprintln!("warning: {}: {e}", p.display()),
        }
    }
}

impl<'m, 'a> FnEmitter<'m, 'a> {
    /// Counts where the target of identifier `e` came from (shared resolution or
    /// own lookup), once per node, outside speculative emission.
    pub(crate) fn count_target_origin(&self, e: ExprId, from_shared: bool) {
        let Some(c) = &self.ctx.type_check else {
            return;
        };
        if self.speculating > 0 || e.0 == u32::MAX {
            return;
        }
        let mut c = c.borrow_mut();
        if !c.origin_seen.insert((self.unit.0, e.0)) {
            return;
        }
        if from_shared {
            c.targets_from_shared += 1;
        } else {
            c.targets_from_own_lookup += 1;
        }
    }

    /// Checks the member of access `e` with an explicit receiver: counts where
    /// it came from (shared resolution or own lookup) and records the divergence
    /// when the two find different declarations. Once per node and kind (read or
    /// write), outside speculative emission.
    pub(crate) fn check_member(&self, e: ExprId, name: &str, shared: Option<&Member>, own: Option<&Member>) {
        let Some(c) = &self.ctx.type_check else {
            return;
        };
        if self.speculating > 0 || e.0 == u32::MAX {
            return;
        }
        let write = matches!(shared.or(own).map(|m| m.kind), Some(MemberKind::Setter(_)));
        if !c.borrow_mut().member_seen.insert((self.unit.0, e.0, write)) {
            return;
        }
        if shared.is_some() {
            c.borrow_mut().members_from_shared += 1;
        } else {
            c.borrow_mut().members_from_own_lookup += 1;
        }
        let (Some(a), Some(b)) = (shared, own) else { return };
        if a.class == b.class && a.kind == b.kind && a.subst == b.subst {
            return;
        }
        let describe = |m: &Member| format!("{}.{name} ({:?}) {:?}", self.ctx.interner.resolve(self.ctx.program.class(m.class).name), m.kind, m.subst);
        let d = self.divergence(e, "membro", "member declaration".to_string(), describe(b), describe(a));
        c.borrow_mut().divergences.push(d);
    }

    /// Counts where the instantiation of the generic call whose argument list
    /// starts at `offset` came from, once per call.
    pub(crate) fn count_instantiation_origin(&self, offset: usize, from_shared: bool) {
        let Some(c) = &self.ctx.type_check else {
            return;
        };
        if self.speculating > 0 {
            return;
        }
        let mut c = c.borrow_mut();
        if !c.instantiation_seen.insert((self.unit.0, offset)) {
            return;
        }
        if from_shared {
            c.instantiations_from_shared += 1;
        } else {
            c.own_instantiations += 1;
        }
    }

    /// Checks the type `ty` that emission follows for `e` (the one deduced by the
    /// emitter, already refined by the shared one, `FnEmitter::refine_by_common`,
    /// before context coercions) against the shared static type.
    pub(crate) fn check_type(&self, e: ExprId, ty: &Ty) {
        let Some(c) = &self.ctx.type_check else {
            return;
        };
        if self.speculating > 0 || e.0 == u32::MAX {
            return;
        }
        if !c.borrow_mut().types_seen.insert((self.unit.0, e.0)) {
            return;
        }
        if self.ctx.is_write_target(self.unit, e) {
            c.borrow_mut().skipped_write_targets += 1;
            return;
        }
        c.borrow_mut().checked_types += 1;
        let Some(shared) = shared_type(self.ctx, self.unit, e) else {
            return;
        };
        let shared = ty_from_table(self.ctx, shared);
        let (a, b) = (normalize(self.ctx, ty), normalize(self.ctx, &shared));
        if equivalent(&a, &b, &mut Vec::new()) {
            return;
        }
        let category = type_category(&a, &b).to_string();
        let d = self.divergence(
            e,
            "tipo",
            category,
            display(self.ctx, &a),
            display(self.ctx, &b),
        );
        c.borrow_mut().divergences.push(d);
    }

    /// Checks the target the emitter resolved for identifier `e` against the
    /// shared `Resolved`. Targets without a stable identity in the emitter
    /// (members looked up by name on the extension's `$this`) are not checked.
    pub(crate) fn check_target(&self, e: ExprId, target: &IdentTarget) {
        let Some(c) = &self.ctx.type_check else {
            return;
        };
        if self.speculating > 0 || e.0 == u32::MAX {
            return;
        }
        if !c.borrow_mut().targets_seen.insert((self.unit.0, e.0)) {
            return;
        }
        let name = match &self.expr(e).kind {
            ExprKind::Identifier(n) => Some(n.sym),
            _ => None,
        };
        let Some(emitter) = emitter_key(self.ctx, target, name) else {
            return;
        };
        c.borrow_mut().checked_targets += 1;
        let shared = self
            .ctx
            .bodies
            .units
            .get(self.unit.0 as usize)
            .and_then(|u| u.get_resolved(e))
            .map(|r| shared_key(self.ctx, r))
            .unwrap_or_else(|| "unresolved".to_string());
        // Name without a declaration (`dynamic`/`Never` as a value, undefined
        // name): neither side has an element.
        if emitter == shared || (emitter == "unknown" && shared == "unresolved") {
            return;
        }
        let category = format!(
            "{} × {}",
            key_kind(&emitter),
            key_kind(&shared)
        );
        let d = self.divergence(e, "alvo", category, emitter, shared);
        c.borrow_mut().divergences.push(d);
    }

    fn divergence(
        &self,
        e: ExprId,
        kind: &'static str,
        category: String,
        emitter: String,
        shared: String,
    ) -> Divergence {
        let unit = self.ctx.program.unit(self.unit);
        let expr = self.expr(e);
        let line = unit.source[..expr.span.start.min(unit.source.len())]
            .matches('\n')
            .count()
            + 1;
        let file = unit
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| unit.uri.clone());
        let mut text: String = self
            .text(expr.span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if text.chars().count() > 60 {
            text = text.chars().take(57).collect::<String>() + "...";
        }
        let node = format!("{:?}", expr.kind);
        let node = node
            .split(|c: char| !c.is_alphanumeric())
            .next()
            .unwrap_or("")
            .to_string();
        let path = unit
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| unit.uri.clone());
        Divergence {
            path,
            start: expr.span.start,
            length: expr.span.end - expr.span.start,
            location: format!("{file}:{line}"),
            node,
            kind,
            category,
            text,
            emitter,
            common: shared,
        }
    }
}

/// Shared static type of `e`, if the unit was inferred.
fn shared_type(ctx: &Ctx, unit: UnitId, e: ExprId) -> Option<TypeId> {
    ctx.bodies
        .units
        .get(unit.0 as usize)
        .and_then(|u| u.get_type(e))
}

/// Converts a `TypeTable` type to `Ty` without side effects on the context
/// (unlike [`Ctx::ty_of`], which records type parameter bounds used later by
/// emission): the check must not change the emitted JS.
pub fn ty_from_table(ctx: &Ctx, id: TypeId) -> Ty {
    match ctx.table.get(id) {
        Type::Dynamic => Ty::Dynamic,
        Type::Void => Ty::Void,
        Type::Never => Ty::Never,
        Type::Null => Ty::Null,
        Type::Interface {
            class,
            args,
            nullable,
        } => {
            let args: Vec<Ty> = args.iter().map(|a| ty_from_table(ctx, *a)).collect();
            let args = ctx.args_at_arity(*class, &args).into_owned();
            Ty::Iface {
                class: *class,
                args,
                nullable: *nullable,
            }
        }
        Type::Function {
            type_params,
            ret,
            positional,
            optional,
            named,
            nullable,
        } => Ty::Fn {
            type_params: type_params
                .iter()
                .map(|p| {
                    let data = ctx.table.param(*p);
                    TyParam {
                        id: p.0,
                        name: ctx.interner.resolve(data.name).to_string(),
                        bound: Box::new(ty_from_table(ctx, data.bound)),
                    }
                })
                .collect(),
            ret: Box::new(ty_from_table(ctx, *ret)),
            pos: positional.iter().map(|a| ty_from_table(ctx, *a)).collect(),
            opt: optional.iter().map(|a| ty_from_table(ctx, *a)).collect(),
            named: {
                let mut v: Vec<(String, Ty, bool)> = named
                    .iter()
                    .map(|(n, t, r)| {
                        (
                            ctx.interner.resolve(*n).to_string(),
                            ty_from_table(ctx, *t),
                            *r,
                        )
                    })
                    .collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                v
            },
            nullable: *nullable,
        },
        Type::Record {
            positional,
            named,
            nullable,
        } => {
            let mut named: Vec<(String, Ty)> = named
                .iter()
                .map(|(n, t)| (ctx.interner.resolve(*n).to_string(), ty_from_table(ctx, *t)))
                .collect();
            named.sort_by(|a, b| a.0.cmp(&b.0));
            Ty::Record {
                pos: positional.iter().map(|a| ty_from_table(ctx, *a)).collect(),
                named,
                nullable: *nullable,
            }
        }
        Type::TypeParameter { param, nullable } => {
            let name = ctx
                .interner
                .resolve(ctx.table.param(*param).name)
                .to_string();
            Ty::Param {
                id: param.0,
                name,
                nullable: *nullable,
            }
        }
        Type::FutureOr { arg, nullable } => Ty::FutureOr {
            arg: Box::new(ty_from_table(ctx, *arg)),
            nullable: *nullable,
        },
        // `X & B` exists only in inference; at run time (and in the emitter) it is `X`.
        Type::Intersection { param, .. } => {
            let name = ctx
                .interner
                .resolve(ctx.table.param(*param).name)
                .to_string();
            Ty::Param {
                id: param.0,
                name,
                nullable: false,
            }
        }
        Type::ExtensionType {
            decl,
            args,
            nullable,
        } => {
            if ctx.interop_ext_types.contains(decl) {
                let args: Vec<Ty> = args.iter().map(|a| ty_from_table(ctx, *a)).collect();
                let args = ctx.args_at_arity(*decl, &args).into_owned();
                return Ty::Iface {
                    class: *decl,
                    args,
                    nullable: *nullable,
                };
            }
            // Erased to the representation type, as `Ctx::ty_of` does.
            let class = ctx.program.class(*decl);
            let Some(rep) = class.representation else {
                return Ty::Dynamic;
            };
            let v = &ctx.outline.variables[rep.0 as usize];
            let Some(t) = v.declared_type.or(v.inferred) else {
                return Ty::Dynamic;
            };
            let mut map = HashMap::new();
            if let Some(params) = ctx.class_params.get(decl.0 as usize) {
                for (p, a) in params.iter().zip(args.iter()) {
                    map.insert(p.id, ty_from_table(ctx, *a));
                }
            }
            let t = ty_from_table(ctx, t).subst_prop(&map);
            if *nullable { t.with_nullable(true) } else { t }
        }
    }
}

/// Canonical form for comparison: `FutureOr` as a class becomes `Ty::FutureOr`,
/// the `Null` class becomes `Ty::Null`, `Never?` becomes `Null`, and
/// `FutureOr<T>?` with a nullable `T` loses the redundant `?`.
pub fn normalize(ctx: &Ctx, t: &Ty) -> Ty {
    match t {
        Ty::Iface {
            class,
            args,
            nullable,
        } if Some(*class) == ctx.future_or => {
            let arg = args
                .first()
                .map(|a| normalize(ctx, a))
                .unwrap_or(Ty::Dynamic);
            Ty::FutureOr {
                arg: Box::new(arg),
                nullable: *nullable,
            }
        }
        Ty::Iface { class, .. } if Some(*class) == ctx.null_ => Ty::Null,
        Ty::Iface {
            class,
            args,
            nullable,
        } => Ty::Iface {
            class: *class,
            args: args.iter().map(|a| normalize(ctx, a)).collect(),
            nullable: *nullable,
        },
        Ty::Fn {
            type_params,
            ret,
            pos,
            opt,
            named,
            nullable,
        } => Ty::Fn {
            type_params: type_params
                .iter()
                .map(|p| TyParam {
                    id: p.id,
                    name: p.name.clone(),
                    bound: Box::new(normalize(ctx, &p.bound)),
                })
                .collect(),
            ret: Box::new(normalize(ctx, ret)),
            pos: pos.iter().map(|a| normalize(ctx, a)).collect(),
            opt: opt.iter().map(|a| normalize(ctx, a)).collect(),
            named: named
                .iter()
                .map(|(n, t, r)| (n.clone(), normalize(ctx, t), *r))
                .collect(),
            nullable: *nullable,
        },
        Ty::Record {
            pos,
            named,
            nullable,
        } => Ty::Record {
            pos: pos.iter().map(|a| normalize(ctx, a)).collect(),
            named: named
                .iter()
                .map(|(n, t)| (n.clone(), normalize(ctx, t)))
                .collect(),
            nullable: *nullable,
        },
        Ty::FutureOr { arg, nullable } => {
            let arg = normalize(ctx, arg);
            let nullable = *nullable && !arg.is_nullable();
            Ty::FutureOr {
                arg: Box::new(arg),
                nullable,
            }
        }
        _ => t.clone(),
    }
}

/// Structural equality modulo representation: bound parameters of function
/// types match by position (`bound`: pairs of ids), free parameters by id or,
/// with different ids, by name.
pub fn equivalent(a: &Ty, b: &Ty, bound: &mut Vec<(u32, u32)>) -> bool {
    match (a, b) {
        (Ty::Dynamic, Ty::Dynamic)
        | (Ty::Void, Ty::Void)
        | (Ty::Never, Ty::Never)
        | (Ty::Null, Ty::Null) => true,
        (
            Ty::Iface {
                class: c1,
                args: a1,
                nullable: n1,
            },
            Ty::Iface {
                class: c2,
                args: a2,
                nullable: n2,
            },
        ) => {
            c1 == c2
                && n1 == n2
                && a1.len() == a2.len()
                && a1.iter().zip(a2).all(|(x, y)| equivalent(x, y, bound))
        }
        (
            Ty::Fn {
                type_params: p1,
                ret: r1,
                pos: s1,
                opt: o1,
                named: m1,
                nullable: n1,
            },
            Ty::Fn {
                type_params: p2,
                ret: r2,
                pos: s2,
                opt: o2,
                named: m2,
                nullable: n2,
            },
        ) => {
            if n1 != n2
                || p1.len() != p2.len()
                || s1.len() != s2.len()
                || o1.len() != o2.len()
                || m1.len() != m2.len()
            {
                return false;
            }
            let mark = bound.len();
            bound.extend(p1.iter().zip(p2).map(|(x, y)| (x.id, y.id)));
            let ok = p1
                .iter()
                .zip(p2)
                .all(|(x, y)| equivalent(&x.bound, &y.bound, bound))
                && equivalent(r1, r2, bound)
                && s1.iter().zip(s2).all(|(x, y)| equivalent(x, y, bound))
                && o1.iter().zip(o2).all(|(x, y)| equivalent(x, y, bound))
                && m1.iter().zip(m2).all(|((k1, t1, q1), (k2, t2, q2))| {
                    k1 == k2 && q1 == q2 && equivalent(t1, t2, bound)
                });
            bound.truncate(mark);
            ok
        }
        (
            Ty::Record {
                pos: s1,
                named: m1,
                nullable: n1,
            },
            Ty::Record {
                pos: s2,
                named: m2,
                nullable: n2,
            },
        ) => {
            n1 == n2
                && s1.len() == s2.len()
                && m1.len() == m2.len()
                && s1.iter().zip(s2).all(|(x, y)| equivalent(x, y, bound))
                && m1
                    .iter()
                    .zip(m2)
                    .all(|((k1, t1), (k2, t2))| k1 == k2 && equivalent(t1, t2, bound))
        }
        (
            Ty::Param {
                id: i1,
                name: x1,
                nullable: n1,
            },
            Ty::Param {
                id: i2,
                name: x2,
                nullable: n2,
            },
        ) => {
            if n1 != n2 {
                return false;
            }
            // The innermost bound one decides; a free one matches by id or name.
            match bound.iter().rev().find(|(x, y)| x == i1 || y == i2) {
                Some((x, y)) => x == i1 && y == i2,
                None => i1 == i2 || x1 == x2,
            }
        }
        (
            Ty::FutureOr {
                arg: a1,
                nullable: n1,
            },
            Ty::FutureOr {
                arg: a2,
                nullable: n2,
            },
        ) => n1 == n2 && equivalent(a1, a2, bound),
        _ => false,
    }
}

/// Category of a type divergence (`a` from the emitter, `b` shared).
fn type_category(a: &Ty, b: &Ty) -> &'static str {
    let kind = |t: &Ty| match t {
        Ty::Dynamic => 0,
        Ty::Void => 1,
        Ty::Never | Ty::Null => 2,
        Ty::Iface { .. } => 3,
        Ty::Fn { .. } => 4,
        Ty::Record { .. } => 5,
        Ty::Param { .. } => 6,
        Ty::FutureOr { .. } => 7,
    };
    if a.is_dynamic() {
        return "emitter dynamic, shared precise";
    }
    if b.is_dynamic() {
        return "shared dynamic, emitter precise";
    }
    if equivalent(
        &a.with_nullable(false),
        &b.with_nullable(false),
        &mut Vec::new(),
    ) {
        return "top-level nullability only";
    }
    match (a, b) {
        (Ty::Iface { class: c1, .. }, Ty::Iface { class: c2, .. }) if c1 == c2 => {
            "same class, different type arguments"
        }
        (Ty::Iface { .. }, Ty::Iface { .. }) => "different classes",
        // The emitter instantiates the generic tear-off at the coercion
        // (`coerce_to`); the shared inference already records the instantiated
        // type on the node, and the analyzer the generic one (the instantiation
        // is a separate node).
        (
            Ty::Fn {
                type_params: p1,
                pos: s1,
                opt: o1,
                named: m1,
                ..
            },
            Ty::Fn {
                type_params: p2,
                pos: s2,
                opt: o2,
                named: m2,
                ..
            },
        ) if !p1.is_empty()
            && p2.is_empty()
            && s1.len() == s2.len()
            && o1.len() == o2.len()
            && m1.len() == m2.len() =>
        {
            "generic tear-off: shared already instantiated"
        }
        (Ty::Fn { .. }, Ty::Fn { .. }) => "different function types",
        (Ty::Record { .. }, Ty::Record { .. }) => "different records",
        (Ty::Param { .. }, Ty::Param { .. }) => "different type parameters",
        (Ty::FutureOr { .. }, Ty::FutureOr { .. }) => "FutureOr with different arguments",
        _ if kind(a) == 1 || kind(b) == 1 => "void × other",
        _ if kind(a) == 2 || kind(b) == 2 => "Never/Null × other",
        _ if kind(a) == 6 || kind(b) == 6 => "type parameter × other",
        _ if kind(a) == 7 || kind(b) == 7 => "FutureOr × other",
        _ => "different kinds",
    }
}

/// Text of a type in Dart style (`List<int>?`, `int Function(String)`).
pub fn display(ctx: &Ctx, t: &Ty) -> String {
    let q = |n: bool| if n { "?" } else { "" };
    let list = |v: &[Ty]| {
        v.iter()
            .map(|a| display(ctx, a))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match t {
        Ty::Dynamic => "dynamic".into(),
        Ty::Void => "void".into(),
        Ty::Never => "Never".into(),
        Ty::Null => "Null".into(),
        Ty::Iface {
            class,
            args,
            nullable,
        } => {
            let n = ctx.class_name(*class);
            if args.is_empty() {
                format!("{n}{}", q(*nullable))
            } else {
                format!("{n}<{}>{}", list(args), q(*nullable))
            }
        }
        Ty::Fn {
            type_params,
            ret,
            pos,
            opt,
            named,
            nullable,
        } => {
            let tp = if type_params.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    type_params
                        .iter()
                        .map(|p| {
                            // Like the analyzer: the bound only shows up when it
                            // is not the implicit one (`Object?`/`dynamic`).
                            let implicit = p.bound.is_dynamic()
                                || (p.bound.is_class(ctx.object) && p.bound.is_nullable());
                            if implicit {
                                p.name.clone()
                            } else {
                                format!("{} extends {}", p.name, display(ctx, &p.bound))
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let mut ps: Vec<String> = pos.iter().map(|a| display(ctx, a)).collect();
            if !opt.is_empty() {
                ps.push(format!("[{}]", list(opt)));
            }
            if !named.is_empty() {
                let v: Vec<String> = named
                    .iter()
                    .map(|(n, t, r)| {
                        format!(
                            "{}{} {n}",
                            if *r { "required " } else { "" },
                            display(ctx, t)
                        )
                    })
                    .collect();
                ps.push(format!("{{{}}}", v.join(", ")));
            }
            let f = format!("{} Function{tp}({})", display(ctx, ret), ps.join(", "));
            // Like the analyzer: `String Function(int)?`.
            if *nullable { format!("{f}?") } else { f }
        }
        Ty::Record {
            pos,
            named,
            nullable,
        } => {
            let mut ps: Vec<String> = pos.iter().map(|a| display(ctx, a)).collect();
            if !named.is_empty() {
                ps.push(format!(
                    "{{{}}}",
                    named
                        .iter()
                        .map(|(n, t)| format!("{} {n}", display(ctx, t)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if pos.len() == 1 && named.is_empty() {
                ps[0].push(',');
            }
            format!("({}){}", ps.join(", "), q(*nullable))
        }
        Ty::Param { name, nullable, .. } => format!("{name}{}", q(*nullable)),
        Ty::FutureOr { arg, nullable } => {
            format!("FutureOr<{}>{}", display(ctx, arg), q(*nullable))
        }
    }
}

/// Key of a top-level element: the identity of the declaration, with the name.
fn element_key(ctx: &Ctx, el: &Element) -> String {
    let p = ctx.program;
    let name = |s| ctx.interner.resolve(s);
    match *el {
        Element::Class(c) => format!("class {}", ctx.class_name(c)),
        Element::Extension(x) => format!("extension #{}", x.0),
        Element::Typedef(t) => format!("typedef #{}", t.0),
        // Implicit field/variable accessor: the identity is the variable (the
        // emitter resolves to the field; the shared inference, to the getter).
        Element::Function(f) => match p.function(f).variable {
            Some(v) => element_key(ctx, &Element::Variable(v)),
            None => format!("function {}#{}", name(p.function(f).name), f.0),
        },
        Element::Variable(v) => format!("variable {}#{}", name(p.variable(v).name), v.0),
        Element::Prefix(..) => "prefix".into(),
    }
}

/// Key of the target the emitter resolved for identifier `name`; `None` for
/// targets without a stable identity (member found by name on the extension's
/// `$this`, extension applicable to `this`).
fn emitter_key(ctx: &Ctx, target: &IdentTarget, name: Option<SymbolId>) -> Option<String> {
    let member = |mk: &MemberKind| match *mk {
        MemberKind::Method(f) | MemberKind::Getter(f) | MemberKind::Setter(f) => {
            element_key(ctx, &Element::Function(f))
        }
        MemberKind::Field(v) => element_key(ctx, &Element::Variable(v)),
    };
    Some(match target {
        IdentTarget::Local(..) => "local".into(),
        IdentTarget::TypeParam(_) => "type-parameter".into(),
        IdentTarget::ThisMember(m) => member(&m.kind),
        IdentTarget::Static(_, mk) => member(mk),
        IdentTarget::Element(el) => element_key(ctx, el),
        IdentTarget::Prefix(_) => "prefix".into(),
        IdentTarget::ExtMember(_, f) | IdentTarget::ExtStatic(_, f) => {
            element_key(ctx, &Element::Function(*f))
        }
        IdentTarget::Unknown => "unknown".into(),
        // Static field of the extension: the emitter keeps the extension; the
        // field is the one with the same name.
        IdentTarget::ExtField(x) => {
            let name = name?;
            let v = ctx
                .program
                .extension(*x)
                .fields
                .iter()
                .find(|&&v| ctx.program.variable(v).name == name)?;
            element_key(ctx, &Element::Variable(*v))
        }
        IdentTarget::ExtThisMember(_) | IdentTarget::ThisExt => return None,
    })
}

/// Key of the shared target, in the same space as [`emitter_key`].
fn shared_key(ctx: &Ctx, r: &Resolved) -> String {
    match r {
        Resolved::Local(_) | Resolved::Parameter { .. } => "local".into(),
        Resolved::TypeParameter(_) => "type-parameter".into(),
        Resolved::Element(el) => element_key(ctx, el),
        Resolved::Member {
            member: MemberRef::Function(f),
            ..
        } => element_key(ctx, &Element::Function(*f)),
        Resolved::Member {
            member: MemberRef::Variable(v),
            ..
        } => element_key(ctx, &Element::Variable(*v)),
        Resolved::Prefix(_) => "prefix".into(),
        Resolved::Dynamic => "dynamic".into(),
        Resolved::ExtensionMember { member, .. } => {
            element_key(ctx, &Element::Function(*member))
        }
        Resolved::Constructor(f) => format!("constructor #{}", f.0),
    }
}

/// Kind of a key (the first word), for categorizing.
fn key_kind(k: &str) -> &str {
    k.split(' ').next().unwrap_or(k)
}
