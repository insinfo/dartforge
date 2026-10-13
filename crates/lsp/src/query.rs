//! A transient semantic query: the program loaded with the editor's current
//! texts, the outline and the bodies inferred by the common inference of
//! `crates/types` ([`dartforge_types::BodyInferrer`]). Completion and rename
//! read types and resolutions from here, never from an inference of their
//! own.
//!
//! Everything belongs to one request: the query is discarded after
//! responding, and nothing of it survives the next version of the text (the
//! LSP memory plateau).

use dartforge_elements::model::{
    Element, FunctionElementId, FunctionRef, LibraryId, Program, UnitId, VariableId, VariableRef,
};
use dartforge_frontend::ast::{DeclKind, MemberKind};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::scope::MemberResolver;
use dartforge_types::{
    BodyInferrer, BodyTypes, CoreTypes, ProbedScope, OutlineTypes, Type, TypeId, TypeTable,
    resolve_outline,
};

/// How [`Query::swap_unit`] rebuilds the unit's tables.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SwapMode {
    /// Completion's speculative text: initializers count as bodies, and only
    /// the body containing `offset` is inferred, with the scope probe in it.
    Speculative { offset: usize },
    /// The document's new text (`didChange`): all bodies of the unit when its
    /// library is among the inferred ones, otherwise only the initializers that
    /// top-level inference records on demand.
    Final { inferred_bodies: bool, register_locals: bool },
}

/// What [`Query::swap_unit`] replaced.
pub(crate) struct Swap {
    previous: dartforge_elements::incremental::Previous,
    tables: dartforge_types::UnitBodyTypes,
    scope: Option<ProbedScope>,
}

/// Program, types and bodies of one request.
pub(crate) struct Query {
    pub program: Program,
    pub names: Interner,
    pub table: TypeTable,
    pub core: CoreTypes,
    pub outline: OutlineTypes,
    pub bodies: BodyTypes,
    /// Scope captured by the probe, when requested and reached.
    pub scope: Option<ProbedScope>,
}

impl Query {
    /// Resolves the outline and infers the bodies of `libraries`.
    ///
    /// `register_locals` enables the local declarations table (rename); `probe`
    /// is the identifier whose lexical scope completion wants.
    pub fn infer(
        program: Program,
        names: Interner,
        libraries: &[LibraryId],
        register_locals: bool,
        probe: Option<(UnitId, usize)>,
    ) -> Self {
        let mut table = TypeTable::new();
        // The analyzer mode (M2): `InvalidType`, `Never?` and written aliases stay
        // in the display, and the pattern flow is the analyzer's.
        table.preserve_decoration = true;
        let core = CoreTypes::init(&mut table, &program, &names);
        let (mut outline, _) = resolve_outline(&program, &names, &mut table, &core);
        let (bodies, scope) = {
            let mut inferrer =
                BodyInferrer::new(&program, &names, &mut table, &core, &mut outline);
            inferrer.only_libraries = Some(libraries.iter().map(|l| l.0).collect());
            inferrer.allocate_bodies_of(libraries);
            inferrer.record_locals = register_locals;
            inferrer.scope_probe = probe;
            let (bodies, _, scope) = inferrer.infer_all_with_probe();
            (bodies, scope)
        };
        Self {
            program,
            names,
            table,
            core,
            outline,
            bodies,
            scope,
        }
    }

    /// Replaces the text of unit `u` without reloading the program when only
    /// bodies changed (docs/LSP-ESPECIFICACAO.md §16.6, class Body) and rebuilds
    /// its tables through the inference of isolated bodies (§16.5). `None`:
    /// refused (the signature, directives or shape changed, or completion fell
    /// outside a body); the query stays as it was.
    pub fn swap_unit(&mut self, u: UnitId, text: &str, mode: SwapMode) -> Option<Swap> {
        use dartforge_elements::incremental::{restore_unit, swap_unit};
        let speculative = matches!(mode, SwapMode::Speculative { .. });
        let previous = swap_unit(&mut self.program, &mut self.names, u, text, speculative).ok()?;
        if !self.outline.remap_written_types(u, previous.type_map()) {
            restore_unit(&mut self.program, previous);
            return None;
        }
        let (bodies, metadata, probe, register) = match mode {
            SwapMode::Speculative { offset } => {
                // §16.7 rule 1: only with the sentinel inside a body.
                let Some(c) = dartforge_types::body_at_offset(&self.program, u, offset) else {
                    self.outline.remap_written_types(u, &previous.inverse_map());
                    restore_unit(&mut self.program, previous);
                    return None;
                };
                (vec![c], false, Some((u, offset)), false)
            }
            SwapMode::Final { inferred_bodies, register_locals } => {
                (dartforge_types::unit_bodies(&self.program, &self.outline, u, inferred_bodies), inferred_bodies, None, register_locals)
            }
        };
        let r = dartforge_types::infer_bodies(&self.program, &self.names, &mut self.table, &self.core, &mut self.outline, u, &bodies, metadata, probe, register);
        let tables = std::mem::replace(&mut self.bodies.units[u.0 as usize], r.tables);
        let scope = std::mem::replace(&mut self.scope, r.scope);
        Some(Swap { previous, tables, scope })
    }

    /// Undoes a [`Query::swap_unit`] (completion, §16.7 rule 3: the retained
    /// state goes back to the document's text; the types interned in the table
    /// by the speculative inference stay, which is harmless).
    pub fn undo(&mut self, swap: Swap) {
        let Swap { previous, tables, scope } = swap;
        let u = previous.unit();
        self.bodies.units[u.0 as usize] = tables;
        self.scope = scope;
        self.outline.remap_written_types(u, &previous.inverse_map());
        dartforge_elements::incremental::restore_unit(&mut self.program, previous);
    }

    /// Infers the bodies of more `libraries` over the retained outline and
    /// table, unit by unit (`infer_bodies`, §16.5: each body comes out as in
    /// the full inference), without redoing the outline or the bodies already
    /// inferred.
    pub fn infer_libraries(&mut self, libraries: &[LibraryId]) {
        for &lib in libraries {
            let units = self.program.library(lib).units.clone();
            for u in units {
                let bodies = dartforge_types::unit_bodies(&self.program, &self.outline, u, true);
                let r = dartforge_types::infer_bodies(&self.program, &self.names, &mut self.table, &self.core, &mut self.outline, u, &bodies, true, None, true);
                self.bodies.units[u.0 as usize] = r.tables;
            }
        }
    }

    /// The same query over the same program, with the bodies of other libraries
    /// (the Flutter colors widen the list).
    pub fn reinfer(self, libraries: &[LibraryId]) -> Self {
        let Query { program, names, .. } = self;
        Query::infer(program, names, libraries, true, None)
    }

    /// Text of a type as Dart writes it. `DartType.getDisplayString()` (without
    /// `preferTypeAlias`): the alias does not appear; `InvalidType` and `Never?`
    /// do.
    pub fn format(&self, ty: TypeId) -> String {
        self.table.format_without_alias(ty, &self.names, &self.program)
    }

    /// Text of a symbol.
    pub fn name(&self, symbol: SymbolId) -> &str {
        self.names.resolve(symbol)
    }

    /// Member lookup over static receivers (the same as in `crates/types`).
    pub fn resolver(&mut self) -> MemberResolver<'_> {
        MemberResolver::new(
            &self.program,
            &self.outline,
            &self.names,
            &self.outline.hierarchy,
            &mut self.table,
            &self.core,
        )
    }

    /// Type of a top-level variable or field: the written one, else the inferred one.
    pub fn variable_type(&self, variable: VariableId) -> Option<TypeId> {
        let data = self.outline.variables.get(variable.0 as usize)?;
        data.declared_type.or(data.inferred)
    }

    /// Detail of a function (`(int x, {String? name}) → void`) from the
    /// signature `signature` (already substituted by the receiver, when there is
    /// one) and the names of the declared parameters.
    pub fn function_detail(&self, function: FunctionElementId, signature: TypeId) -> String {
        let Type::Function {
            ret,
            positional,
            optional,
            named,
            ..
        } = self.table.get(signature).clone()
        else {
            return self.format(signature);
        };
        let data = self.outline.functions.get(function.0 as usize);
        let positional_names: Vec<Option<SymbolId>> = data
            .map(|d| {
                d.parameters
                    .iter()
                    .filter(|p| p.kind != dartforge_frontend::ast::ParameterKind::Named)
                    .map(|p| p.name)
                    .collect()
            })
            .unwrap_or_default();
        let mut parts = Vec::new();
        for (i, t) in positional.iter().enumerate() {
            parts.push(self.parameter(*t, positional_names.get(i).copied().flatten()));
        }
        if !optional.is_empty() {
            let optional_: Vec<String> = optional
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    self.parameter(
                        *t,
                        positional_names
                            .get(positional.len() + i)
                            .copied()
                            .flatten(),
                    )
                })
                .collect();
            parts.push(format!("[{}]", optional_.join(", ")));
        }
        if !named.is_empty() {
            // Declaration order, not the type's canonical order (by symbol).
            let mut named_: Vec<(usize, String)> = named
                .iter()
                .map(|(n, t, required)| {
                    let order = data
                        .and_then(|d| {
                            d.parameters
                                .iter()
                                .position(|p| p.external_name.or(p.name) == Some(*n))
                        })
                        .unwrap_or(usize::MAX);
                    let prefix = if *required { "required " } else { "" };
                    (
                        order,
                        format!("{prefix}{} {}", self.format(*t), self.name(*n)),
                    )
                })
                .collect();
            named_.sort();
            parts.push(format!(
                "{{{}}}",
                named_
                    .into_iter()
                    .map(|(_, s)| s)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        format!("({}) → {}", parts.join(", "), self.format(ret))
    }

    fn parameter(&self, ty: TypeId, name: Option<SymbolId>) -> String {
        match name {
            Some(n) => format!("{} {}", self.format(ty), self.name(n)),
            None => self.format(ty),
        }
    }

    /// Simple name of a top-level element, when it has a name.
    pub fn element_name(&self, element: Element) -> Option<SymbolId> {
        Some(match element {
            Element::Class(c) => self.program.class(c).name,
            Element::Extension(x) => self.program.extension(x).name?,
            Element::Typedef(t) => self.program.typedef(t).name,
            Element::Function(f) => self.program.function(f).name,
            Element::Variable(v) => self.program.variable(v).name,
            Element::Prefix(_, p) => p,
        })
    }

    /// Unit and start (with metadata) of the declaration of a top-level element.
    pub fn element_start(&self, el: Element) -> Option<(UnitId, usize)> {
        let p = &self.program;
        let d = match el {
            Element::Class(c) => p.class(c).decl?,
            Element::Extension(x) => p.extension(x).decl,
            Element::Typedef(t) => p.typedef(t).decl,
            Element::Function(f) => return self.function_start(f),
            Element::Variable(v) => return self.variable_start(v),
            Element::Prefix(..) => return None,
        };
        Some((d.unit, p.unit(d.unit).ast.decl(d.decl).span.start))
    }

    /// Unit and start (with metadata) of a function, method or constructor.
    pub fn function_start(&self, f: FunctionElementId) -> Option<(UnitId, usize)> {
        let p = &self.program;
        match p.function(f).node {
            FunctionRef::Function { unit, function } => {
                let ast = &p.unit(unit).ast;
                let start = ast
                    .decls
                    .iter()
                    .find(|d| matches!(d.kind, DeclKind::Function(x) if x == function))
                    .map(|d| d.span.start)
                    .or_else(|| {
                        ast.members
                            .iter()
                            .find(|m| matches!(m.kind, MemberKind::Method(x) if x == function))
                            .map(|m| m.span.start)
                    })
                    .unwrap_or(ast.function(function).span.start);
                Some((unit, start))
            }
            FunctionRef::Constructor { unit, member } => {
                Some((unit, p.unit(unit).ast.member(member).span.start))
            }
            FunctionRef::None => p
                .function(f)
                .variable
                .and_then(|v| self.variable_start(v)),
        }
    }

    /// Unit and start (with metadata) of a top-level variable, field or enum
    /// constant.
    pub fn variable_start(&self, v: VariableId) -> Option<(UnitId, usize)> {
        let p = &self.program;
        match p.variable(v).node {
            VariableRef::TopLevel { unit, decl, .. } => {
                Some((unit, p.unit(unit).ast.decl(decl).span.start))
            }
            VariableRef::Field { unit, member, .. } => {
                Some((unit, p.unit(unit).ast.member(member).span.start))
            }
            VariableRef::EnumConstant { unit, decl, index } => {
                match &p.unit(unit).ast.decl(decl).kind {
                    DeclKind::Enum(e) => Some((unit, e.constants.get(index)?.span.start)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// File and start (with metadata) of a declaration, for the documentation
    /// that `completionItem/resolve` fetches later.
    pub fn origin(&self, local: Option<(UnitId, usize)>) -> Option<(std::path::PathBuf, usize)> {
        let (u, start) = local?;
        Some((self.program.unit(u).path.clone()?, start))
    }
}
