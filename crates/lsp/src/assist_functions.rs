//! Assists on function bodies, ported from the `analysis_server` 3.6.2
//! producers over the analyzer-shaped tree.
//!
//! | Title | Kind | Producer |
//! |---|---|---|
//! | `Convert to async function body` | `refactor.convert.bodyToAsync` | `ConvertIntoAsyncBody` |

use crate::actions::CodeAction;
use crate::arvore_analyzer::Marca;
use crate::refactorings::Context;
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast;
use dartforge_types::{Type, TypeId};

impl Context<'_> {
    /// `getEnclosingFunctionBody` (correction_producer.dart:729-747).
    pub(crate) fn enclosing_body(&self, node: usize) -> Option<usize> {
        let ancestor = |kind: &str| self.with_parents(node).find(|&k| self.kind(k) == kind);
        let body_of = |n: usize| self.children(n).iter().copied().rev().find(|&f| self.kind(f).ends_with("FunctionBody"));
        if let Some(f) = ancestor("FunctionExpression") {
            return body_of(f);
        }
        if let Some(d) = ancestor("FunctionDeclaration") {
            let f = self.children(d).iter().copied().find(|&k| self.kind(k) == "FunctionExpression")?;
            return body_of(f);
        }
        if let Some(c) = ancestor("ConstructorDeclaration") {
            return body_of(c);
        }
        let m = ancestor("MethodDeclaration")?;
        body_of(m)
    }

    /// The parser function whose body is `body` (the parent's mark).
    fn body_function(&self, body: usize) -> Option<ast::FunctionId> {
        match self.tree.nos[self.parent(body)?].marca {
            Marca::Funcao(fid) => Some(fid),
            _ => {
                // The `FunctionExpression` of a `FunctionDeclaration`: the mark is on the
                // grandparent.
                let grandparent = self.parent(self.parent(body)?)?;
                match self.tree.nos[grandparent].marca {
                    Marca::Funcao(fid) => Some(fid),
                    _ => None,
                }
            }
        }
    }

    /// `typeSystem.flatten`: `Future<S>` and `FutureOr<S>` give `S`.
    fn flatten(&self, t: TypeId) -> TypeId {
        let table = &self.p.query.table;
        match table.get(t) {
            Type::FutureOr { arg, .. } => *arg,
            Type::Interface { class, args, .. } if Some(*class) == self.p.query.core.future_class && args.len() == 1 => args[0],
            _ => t,
        }
    }

    /// `ConvertIntoAsyncBody` (convert_into_async_body.dart) with the
    /// `convertFunctionFromSyncToAsync` of `DartFileEditBuilder`.
    pub(crate) fn convert_to_async_body(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let node = self.tree.localizar(start, end)?;
        let body = self.enclosing_body(node)?;
        if self.kind(body) == "EmptyFunctionBody" {
            return None;
        }
        let parent = self.parent(body)?;
        if self.kind(parent) == "ConstructorDeclaration" {
            return None;
        }
        let fid = self.body_function(body)?;
        let f = self.ast.function(fid);
        // `body.keyword != null`: `async`, `async*`, `sync*`.
        if f.modifier != ast::AsyncModifier::None {
            return None;
        }
        // Only at the start of the body: up to the end of the `{` or of the `=>`.
        let first = self.next_token(self.tree.nos[body].inicio)?;
        if start > first.end {
            return None;
        }
        // A closure has no written return type, and Dart does not convert it.
        let declaration = if self.kind(parent) == "FunctionExpression" {
            let d = self.parent(parent).filter(|&d| self.kind(d) == "FunctionDeclaration")?;
            Some(d)
        } else if self.kind(parent) == "MethodDeclaration" {
            Some(parent)
        } else {
            None
        };
        let mut edits = Vec::new();
        // `async ` at the start of the body, with a space before it if the body is
        // attached to the previous token.
        let attached = self.previous_token(first.start).is_some_and(|t| t.end == first.start);
        edits.push((Span { start: first.start, end: first.start }, if attached { " async ".to_string() } else { "async ".to_string() }));
        let mut import = std::collections::BTreeSet::new();
        // `_replaceReturnTypeWithFuture` → `replaceTypeWithFuture`.
        if declaration.is_some()
            && let Some(annotation) = f.return_type
        {
            let ty = match self.p.node_function(self.unit, fid) {
                Some(e) => self.p.query.outline.functions.get(e.0 as usize).map(|d| d.return_type),
                None => self.bodies.tipos_de_anotacoes.get(&annotation).copied(),
            };
            if let Some(t) = ty {
                let table = &self.p.query.table;
                let is_future = matches!(table.get(t), Type::Interface { class, .. } if Some(*class) == self.p.query.core.future_class);
                if !matches!(table.get(t), Type::Dynamic) && !is_future {
                    let value = self.flatten(t);
                    let s = self.ast.ty(annotation).span;
                    let mut writer = crate::write_type::Writer::new(self, s.start);
                    let internal = writer.write(value, true).unwrap_or_else(|| "dynamic".to_string());
                    let future = match self.p.query.core.future_class {
                        Some(c) => writer.reference_of(dartforge_elements::model::Element::Class(c), "Future"),
                        None => "Future".to_string(),
                    };
                    edits.push((s, format!("{future}<{internal}>")));
                    import = writer.import;
                }
            }
        }
        let mut m = crate::refactorings_exec::Change::default();
        for (s, text) in edits {
            m.add(uri, s, text);
        }
        crate::refactorings_move::builder_imports(self, &mut m, &import);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Convert to async function body".into(),
            kind: "refactor.convert.bodyToAsync".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect::<Vec<Edit>>(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `_ReturnTypeComputer`: the `leastUpperBound` of the types of the body's
    /// `return`s with a value (without those of nested functions), excluding
    /// those of type `Never`; and whether there was any `return`.
    fn return_types_type(&self, body: usize) -> (Option<TypeId>, bool) {
        let mut ty: Option<TypeId> = None;
        let mut happened = false;
        let mut stack: Vec<usize> = self.children(body).to_vec();
        let query = &self.p.query;
        let mut table = query.table.clone();
        while let Some(k) = stack.pop() {
            match self.kind(k) {
                "FunctionExpression" | "FunctionDeclarationStatement" | "BlockFunctionBody" => continue,
                "ReturnStatement" => {
                    happened = true;
                    if let Some(t) = self.children(k).first().and_then(|&e| self.node_expr(e)).and_then(|x| self.bodies.get_type(x))
                        && !matches!(table.get(t), Type::Never)
                    {
                        ty = Some(match ty {
                            None => t,
                            Some(current) => {
                                let mut env = dartforge_types::subtyping::SubtypeEnv::new(&mut table, &query.outline.hierarchy, &query.core);
                                dartforge_types::bounds::up(current, t, &mut env)
                            }
                        });
                    }
                }
                _ => {}
            }
            stack.extend(self.children(k).iter().copied());
        }
        // Only a type that the query's table already has is valid (the writer
        // reads that table).
        (ty.filter(|t| (t.0 as usize) < query.table.len()), happened)
    }

    /// `AddReturnType` (add_return_type.dart): on the name of a method or
    /// function without a return type (and not a setter), the type inferred from
    /// the body before the `operator`/`get` or the name.
    pub(crate) fn add_return_type(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let node = self.tree.localizar(start, end)?;
        let declaration = match self.kind(node) {
            "MethodDeclaration" | "FunctionDeclaration" => node,
            _ => return None,
        };
        let Marca::Funcao(fid) = self.tree.nos[declaration].marca else { return None };
        let function = self.ast.function(fid);
        let name = function.name?.span;
        // `executable.name == token`: the selection is on the name.
        if !(name.start <= start && start <= name.end) {
            return None;
        }
        if function.return_type.is_some() {
            return None;
        }
        // Setter and the keywords before the name.
        let previous = self.previous_token(name.start);
        let word = previous.map(|s| &self.source[s.start..s.end]);
        if word == Some("set") {
            return None;
        }
        let before = match word {
            Some("get" | "operator") => previous?.start,
            _ => name.start,
        };
        // The body.
        let body = if self.kind(declaration) == "FunctionDeclaration" {
            let expression = self.children(declaration).iter().copied().find(|&k| self.kind(k) == "FunctionExpression")?;
            *self.children(expression).last()?
        } else {
            *self.children(declaration).last()?
        };
        let query = &self.p.query;
        let base = match self.kind(body) {
            "ExpressionFunctionBody" => self.children(body).first().and_then(|&e| self.node_expr(e)).and_then(|x| self.bodies.get_type(x))?,
            "BlockFunctionBody" => match self.return_types_type(body) {
                (Some(t), _) => t,
                (None, true) => query.core.void_,
                (None, false) => return None,
            },
            _ => return None,
        };
        let mut writer = crate::write_type::Writer::new(self, before);
        let written = if matches!(query.table.get(base), Type::Dynamic) {
            "dynamic".to_string()
        } else {
            let text = writer.write(base, false)?;
            // `instantiate(typeArguments: [base], nullabilitySuffix: base.nullabilitySuffix)`.
            let nullable = matches!(
                query.table.get(base),
                Type::Interface { nullable: true, .. }
                    | Type::Function { nullable: true, .. }
                    | Type::Record { nullable: true, .. }
                    | Type::TypeParameter { nullable: true, .. }
                    | Type::FutureOr { nullable: true, .. }
                    | Type::ExtensionType { nullable: true, .. }
            );
            let suffix = if nullable { "?" } else { "" };
            match function.modifier {
                ast::AsyncModifier::Async => format!("Future<{text}>{suffix}"),
                ast::AsyncModifier::AsyncStar => format!("Stream<{text}>{suffix}"),
                ast::AsyncModifier::SyncStar => format!("Iterable<{text}>{suffix}"),
                _ => text,
            }
        };
        let mut m = crate::refactorings_exec::Change::default();
        m.add(uri, Span { start: before, end: before }, format!("{written} "));
        crate::refactorings_move::builder_imports(self, &mut m, &writer.import);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Add return type".into(),
            kind: "refactor.add.returnType".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }
}
