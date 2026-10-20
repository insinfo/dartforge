//! `ConvertToSuperParameters` (convert_to_super_parameters.dart) from
//! `analysis_server` 3.6.2: the constructor parameters that are only passed
//! on to `super(…)` become `super.` parameters.
//!
//! | Title | Kind |
//! |---|---|
//! | `Convert to using super parameters` | `refactor.convert.toSuperParameters` |

use crate::actions::CodeAction;
use crate::analyzer_tree::Mark;
use crate::refactorings::Context;
use crate::refactorings_exec::Change;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionElementId, FunctionRef, LibraryId, UnitId};
use dartforge_frontend::ast::{self, ExprId, MemberKind, ParameterKind};
use dartforge_types::{Resolved, Type, TypeId};
use std::collections::{HashMap, HashSet};

/// A parameter of the superclass constructor.
struct SuperParameter {
    name: String,
    named: bool,
    ty: TypeId,
    /// The default value expression (unit, library, expression).
    pattern: Option<(UnitId, LibraryId, ExprId)>,
}

/// A parameter to convert (`_ParameterData`).
struct Data {
    final_: Option<Span>,
    type_to_remove: Option<(Option<Span>, Option<Span>)>,
    name: Span,
    null_initializer: bool,
    pattern_range: Option<Span>,
    parameter_index: usize,
    argument_index: usize,
}

impl Context<'_> {
    /// `_findConstructor`.
    fn selection_constructor(&self, node: usize) -> Option<usize> {
        match self.kind(node) {
            "ConstructorDeclaration" => Some(node),
            "SimpleIdentifier" => {
                let p = self.parent(node)?;
                match self.kind(p) {
                    "ConstructorDeclaration" => Some(p),
                    "ConstructorName" => self
                        .parent(p)
                        .filter(|&a| self.kind(a) == "ConstructorDeclaration"),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The parameters of the superclass constructor, with the types as seen from
    /// the class (`ConstructorMember`: the superclass type arguments
    /// substituted).
    fn super_parameters(
        &self,
        class: dartforge_elements::model::ClassId,
        sf: FunctionElementId,
        table: &mut dartforge_types::TypeTable,
    ) -> Option<Vec<SuperParameter>> {
        let prog = self.p.program();
        let query = &self.p.query;
        let data = query.outline.functions.get(sf.0 as usize)?;
        let sfe = prog.function(sf);
        let owner = sfe.class?;
        // `C<…>` with its own parameters, seen as the class of the super.
        let parameters_c: Vec<TypeId> = query
            .outline
            .classes
            .get(class.0 as usize)
            .map(|c| {
                c.type_params
                    .iter()
                    .map(|&p| {
                        table.intern(Type::TypeParameter {
                            param: p,
                            nullable: false,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let this = table.intern(Type::Interface {
            class,
            args: parameters_c.into_boxed_slice(),
            nullable: false,
        });
        let mut map: HashMap<dartforge_types::TypeParamId, TypeId> = HashMap::new();
        if let Some(seen) = query
            .outline
            .hierarchy
            .supertype_of(this, owner, table, &query.core)
            && let Type::Interface { args, .. } = table.get(seen).clone()
            && let Some(cd) = query.outline.classes.get(owner.0 as usize)
        {
            for (p, a) in cd.type_params.iter().zip(args.iter()) {
                map.insert(*p, *a);
            }
        }
        // The default values, from the AST of the superclass constructor.
        let patterns: Vec<Option<(UnitId, LibraryId, ExprId)>> = match sfe.node {
            FunctionRef::Constructor { unit, member } => {
                match &prog.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => k
                        .parameters
                        .iter()
                        .map(|p| p.default_value.map(|d| (unit, prog.unit(unit).library, d)))
                        .collect(),
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        };
        Some(
            data.parameters
                .iter()
                .enumerate()
                .map(|(i, p)| SuperParameter {
                    name: p
                        .external_name
                        .or(p.name)
                        .map(|n| self.p.name(n).to_string())
                        .unwrap_or_default(),
                    named: p.kind == ParameterKind::Named,
                    ty: dartforge_types::ops::substitute(p.ty, &map, table),
                    pattern: patterns.get(i).copied().flatten(),
                })
                .collect(),
        )
    }

    /// `ConvertToSuperParameters`.
    pub(crate) fn convert_to_super_parameters(
        &self,
        uri: &str,
        start: usize,
        end: usize,
    ) -> Option<CodeAction> {
        let prog = self.p.program();
        if prog
            .library(prog.unit(self.unit).library)
            .features
            .version()
            < dartforge_frontend::features::LanguageVersion::new(2, 17)
        {
            return None;
        }
        let node = self.tree.locate(start, end)?;
        let constructor = self.selection_constructor(node)?;
        // `_superInvocation`: the last `super(…)` among the initializers.
        let initializers: Vec<usize> = self
            .children(constructor)
            .iter()
            .copied()
            .filter(|&k| {
                matches!(
                    self.kind(k),
                    "ConstructorFieldInitializer"
                        | "SuperConstructorInvocation"
                        | "RedirectingConstructorInvocation"
                        | "AssertInitializer"
                )
            })
            .collect();
        let invocation = *initializers
            .iter()
            .rev()
            .find(|&&k| self.kind(k) == "SuperConstructorInvocation")?;
        let node_end = self.tree.nodes[constructor].end;
        let mi =
            self.ast.members.iter().position(|m| {
                matches!(m.kind, MemberKind::Constructor(_)) && m.span.end == node_end
            })?;
        let MemberKind::Constructor(k) = &self.ast.member(ast::MemberId(mi as u32)).kind else {
            return None;
        };
        let f = self
            .p
            .node_constructor(self.unit, ast::MemberId(mi as u32))?;
        let sf = self.p.super_constructor(f)?;
        let class = prog.function(f).class?;
        let query = &self.p.query;
        let mut table = query.table.clone();
        let super_parameters_ = self.super_parameters(class, sf, &mut table)?;
        let types_of_this: Vec<TypeId> = query
            .outline
            .functions
            .get(f.0 as usize)?
            .parameters
            .iter()
            .map(|p| p.ty)
            .collect();
        let list = self
            .children(constructor)
            .iter()
            .copied()
            .find(|&c| self.kind(c) == "FormalParameterList")?;
        let parameter_nodes = self.children(list).to_vec();
        // `_parameterMap`: the simple and the function-typed ones, by name offset.
        let mut map: HashMap<usize, usize> = HashMap::new();
        for (i, &p) in parameter_nodes.iter().enumerate() {
            let internal = if self.kind(p) == "DefaultFormalParameter" {
                *self.children(p).first()?
            } else {
                p
            };
            if matches!(
                self.kind(internal),
                "SimpleFormalParameter" | "FunctionTypedFormalParameter"
            ) && let Some(n) = k.parameters.get(i).and_then(|x| x.name)
            {
                map.insert(n.span.start, i);
            }
        }
        // The parameter (index) that an identifier refers to.
        let parameter_of = |e: usize| -> Option<usize> {
            if self.kind(e) != "SimpleIdentifier" {
                return None;
            }
            let Mark::Expr(x) = self.tree.nodes[e].mark else {
                return None;
            };
            let decl = match self.bodies.get_resolved(x)? {
                Resolved::Local(_) => self.bodies.local_declaration(x)?,
                Resolved::Parameter { name, .. } => {
                    k.parameters
                        .iter()
                        .find(|p| p.name.is_some_and(|n| n.sym == *name))?
                        .name?
                        .span
                        .start
                }
                _ => return None,
            };
            map.get(&decl).copied()
        };
        // `_referencedParameters`: the parameters referenced in the body.
        let mut cited: HashSet<usize> = HashSet::new();
        if let Some(&body) = self
            .children(constructor)
            .iter()
            .find(|&&c| matches!(self.kind(c), "BlockFunctionBody" | "ExpressionFunctionBody"))
        {
            let mut stack = vec![body];
            while let Some(n) = stack.pop() {
                if self.kind(n) == "SimpleIdentifier"
                    && let Mark::Expr(x) = self.tree.nodes[n].mark
                {
                    match self.bodies.get_resolved(x) {
                        Some(Resolved::Parameter { name, .. }) => {
                            if let Some(i) = k
                                .parameters
                                .iter()
                                .position(|p| p.name.is_some_and(|q| q.sym == *name))
                            {
                                cited.insert(i);
                            }
                        }
                        Some(Resolved::Local(_)) => {
                            if let Some(d) = self.bodies.local_declaration(x)
                                && let Some(&i) = map.get(&d)
                            {
                                cited.insert(i);
                            }
                        }
                        _ => {}
                    }
                }
                stack.extend(self.children(n).iter().copied());
            }
        }
        let arguments = self.list_arguments(invocation);
        let mut engine = None;
        let mut data_of = |i: usize,
                           index_arg: usize,
                           super_p: &SuperParameter,
                           table: &mut dartforge_types::TypeTable|
         -> Option<Data> {
            let this_type = *types_of_this.get(i)?;
            let mut env = dartforge_types::subtyping::SubtypeEnv::new(
                table,
                &query.outline.hierarchy,
                &query.core,
            );
            if !dartforge_types::subtyping::is_subtype(this_type, super_p.ty, &mut env) {
                return None;
            }
            let p = k.parameters.get(i)?;
            let name_ = p.name?.span;
            let node_p = parameter_nodes[i];
            let internal = if self.kind(node_p) == "DefaultFormalParameter" {
                *self.children(node_p).first()?
            } else {
                node_p
            };
            // `_defaultValueRange`: a default equal to the super's is dropped.
            let pattern_range = match (p.default_value, super_p.pattern) {
                (Some(d), Some((su, sl, se))) => {
                    let m = engine.get_or_insert_with(|| table.clone());
                    let mut mt = dartforge_types::const_eval::evaluation::Engine::new(
                        prog,
                        &query.names,
                        m,
                        &query.core,
                        &query.outline,
                        &query.bodies,
                        &self.p.libraries,
                    );
                    let lib = prog.unit(self.unit).library;
                    let this = mt.evaluate(
                        &dartforge_types::const_eval::evaluation::Ctx::simple(self.unit, lib),
                        d,
                        true,
                    );
                    let its = mt.evaluate(
                        &dartforge_types::const_eval::evaluation::Ctx::simple(su, sl),
                        se,
                        true,
                    );
                    match (its.value(), this.value()) {
                        (Some(a), Some(b)) if mt.equal(a, b) => Some(Span {
                            start: name_.end,
                            end: self.ast.expr(d).span.end,
                        }),
                        _ => None,
                    }
                }
                _ => None,
            };
            let final_ = if p.final_ && self.kind(internal) == "SimpleFormalParameter" {
                self.tokens
                    .iter()
                    .find(|t| {
                        t.span.start >= self.tree.nodes[internal].start
                            && t.span.end <= name_.start
                            && &self.source[t.span.start..t.span.end] == "final"
                    })
                    .map(|t| t.span)
            } else {
                None
            };
            let null_initializer = p.kind != ParameterKind::Required
                && !p.required
                && p.default_value.is_none()
                && super_p.pattern.is_some();
            let type_to_remove = if this_type == super_p.ty {
                match self.kind(internal) {
                    "SimpleFormalParameter" => p.ty.map(|t| {
                        (
                            Some(Span {
                                start: self.ast.ty(t).span.start,
                                end: name_.start,
                            }),
                            None,
                        )
                    }),
                    "FunctionTypedFormalParameter" => {
                        let list_f = self
                            .children(internal)
                            .iter()
                            .rev()
                            .find(|&&c| self.kind(c) == "FormalParameterList")
                            .map(|&l| self.tree.span(l));
                        Some((
                            p.ty.map(|t| Span {
                                start: self.ast.ty(t).span.start,
                                end: name_.start,
                            }),
                            list_f,
                        ))
                    }
                    _ => None,
                }
            } else {
                None
            };
            Some(Data {
                final_,
                type_to_remove,
                name: name_,
                null_initializer,
                pattern_range,
                parameter_index: i,
                argument_index: index_arg,
            })
        };
        let mut positional: Option<Vec<Data>> = Some(Vec::new());
        let mut named: Vec<Data> = Vec::new();
        let mut position = 0usize;
        let super_positional: Vec<&SuperParameter> =
            super_parameters_.iter().filter(|p| !p.named).collect();
        for (ia, &arg) in arguments.iter().enumerate() {
            if self.kind(arg) == "NamedExpression" {
                let f = self.children(arg);
                let label = self
                    .node_text(*f.first()?)
                    .trim_end_matches(':')
                    .trim()
                    .to_string();
                let Some(&exp) = f.get(1) else { continue };
                if let Some(i) = parameter_of(exp)
                    && k.parameters[i].kind == ParameterKind::Named
                    && k.parameters[i]
                        .external_name()
                        .is_some_and(|n| self.p.name(n.sym) == label)
                    && !cited.contains(&i)
                    && let Some(sp) = super_parameters_
                        .iter()
                        .find(|p| p.named && p.name == label)
                    && let Some(d) = data_of(i, ia, sp, &mut table)
                {
                    named.push(d);
                }
            } else {
                let super_p = super_positional.get(position).copied();
                position += 1;
                if positional.is_some() {
                    match parameter_of(arg) {
                        Some(i)
                            if k.parameters[i].kind != ParameterKind::Named
                                && !cited.contains(&i) =>
                        {
                            match super_p.and_then(|sp| data_of(i, ia, sp, &mut table)) {
                                Some(d) => positional.as_mut().expect("positionals").push(d),
                                None => positional = None,
                            }
                        }
                        _ => positional = None,
                    }
                }
            }
        }
        // `_inOrder`.
        if let Some(p) = &positional
            && p.windows(2)
                .any(|w| w[1].parameter_index < w[0].parameter_index)
        {
            positional = None;
        }
        if positional.as_ref().is_none_or(Vec::is_empty) && named.is_empty() {
            return None;
        }
        let all: Vec<Data> = positional
            .unwrap_or_default()
            .into_iter()
            .chain(named)
            .collect();
        let mut to_remove: Vec<usize> = all.iter().map(|d| d.argument_index).collect();
        to_remove.sort_unstable();
        let mut m = Change::default();
        let before = |m: &mut Change, s: usize| m.add(uri, Span { start: s, end: s }, "super.");
        for d in &all {
            let insert_super = |m: &mut Change| match d.final_ {
                None => before(m, d.name.start),
                Some(kw) => {
                    let after = self.next_token(kw.end).map_or(d.name.start, |s| s.start);
                    if after == d.name.start {
                        m.add(
                            uri,
                            Span {
                                start: kw.start,
                                end: after,
                            },
                            "super.",
                        );
                    } else {
                        m.add(
                            uri,
                            Span {
                                start: kw.start,
                                end: after,
                            },
                            String::new(),
                        );
                        before(m, d.name.start);
                    }
                }
            };
            match &d.type_to_remove {
                None => insert_super(&mut m),
                Some((primary, parameter_range)) => {
                    match primary {
                        None => before(&mut m, d.name.start),
                        Some(pr) => match d.final_ {
                            None => m.add(uri, *pr, "super."),
                            Some(kw) => {
                                let after = self.next_token(kw.end).map_or(pr.start, |s| s.start);
                                if after == pr.start {
                                    m.add(
                                        uri,
                                        Span {
                                            start: kw.start,
                                            end: pr.end,
                                        },
                                        "super.",
                                    );
                                } else {
                                    m.add(
                                        uri,
                                        Span {
                                            start: kw.start,
                                            end: after,
                                        },
                                        String::new(),
                                    );
                                    m.add(uri, *pr, "super.");
                                }
                            }
                        },
                    }
                    if d.null_initializer {
                        m.add(
                            uri,
                            Span {
                                start: d.name.end,
                                end: d.name.end,
                            },
                            " = null",
                        );
                    }
                    if let Some(fp) = parameter_range {
                        m.add(uri, *fp, String::new());
                    }
                }
            }
            if let Some(fp) = d.pattern_range {
                m.add(uri, fp, String::new());
            }
        }
        let super_name = self
            .children(invocation)
            .iter()
            .any(|&c| self.kind(c) == "SimpleIdentifier");
        let argument_list = self
            .children(invocation)
            .iter()
            .copied()
            .find(|&c| self.kind(c) == "ArgumentList")?;
        if to_remove.len() == arguments.len() {
            if !super_name {
                let range = if initializers.len() == 1 {
                    Span {
                        start: self.tree.nodes[list].end,
                        end: self.tree.nodes[invocation].end,
                    }
                } else {
                    let i = initializers.iter().position(|&x| x == invocation)?;
                    if i == 0 {
                        Span {
                            start: self.tree.nodes[invocation].start,
                            end: self.tree.nodes[initializers[1]].start,
                        }
                    } else {
                        Span {
                            start: self.tree.nodes[initializers[i - 1]].end,
                            end: self.tree.nodes[invocation].end,
                        }
                    }
                };
                m.add(uri, range, String::new());
            } else {
                let s = self.tree.span(argument_list);
                m.add(
                    uri,
                    Span {
                        start: s.start + 1,
                        end: s.end - 1,
                    },
                    String::new(),
                );
            }
        } else {
            // `nodesInList`.
            let mut groups: Vec<(usize, usize)> = Vec::new();
            for &i in &to_remove {
                match groups.last_mut() {
                    Some((_, end)) if *end + 1 == i => *end = i,
                    _ => groups.push((i, i)),
                }
            }
            for (from, until) in groups {
                let range = if from == 0 {
                    Span {
                        start: self.tree.nodes[arguments[from]].start,
                        end: self.tree.nodes[arguments[until + 1]].start,
                    }
                } else {
                    Span {
                        start: self.tree.nodes[arguments[from - 1]].end,
                        end: self.tree.nodes[arguments[until]].end,
                    }
                };
                m.add(uri, range, String::new());
            }
        }
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Convert to using super parameters".into(),
            kind: "refactor.convert.toSuperParameters".into(),
            edits: m
                .files
                .into_iter()
                .flat_map(|(_, l)| l.into_iter().rev())
                .collect(),
            diagnostic: None,
            create_file: None,
            linked: Default::default(),
        })
    }
}
