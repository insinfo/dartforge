//! Assistências de anotação de tipo, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Remove type annotation` | `refactor.remove.typeAnnotation` | `RemoveTypeAnnotation.other` |
//! | `Add type annotation` | `refactor.add.typeAnnotation` | `AddTypeAnnotation.bulkFixable` |
//! | `Replace type annotation with 'var'` | `refactor.replace.withVar` | `ReplaceWithVar` |
//! | `Convert into 'Function' syntax` | `refactor.convert.toGenericFunctionSyntax` | `ConvertToGenericFunctionSyntax` |

use crate::actions::CodeAction;
use crate::refactorings::Context;
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, ExprKind, StmtKind};
use dartforge_types::{Type, TypeId};

/// As espécies de `TypeAnnotation`.
const TYPES: &[&str] = &["NamedType", "GenericFunctionType", "RecordTypeAnnotation"];

impl Context<'_> {
    fn type_action(&self, uri: &str, title: &str, kind: &str, edits: Vec<(Span, String)>) -> CodeAction {
        CodeAction {
            title: title.into(),
            kind: kind.into(),
            edits: edits.into_iter().map(|(span, text)| Edit { uri: uri.to_string(), span, text }).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        }
    }

    /// O filho de `n` que é uma anotação de tipo.
    fn type_child(&self, n: usize) -> Option<usize> {
        self.children(n).iter().copied().find(|&f| TYPES.contains(&self.kind(f)))
    }

    /// `RemoveTypeAnnotation.other` (remove_type_annotation.dart): o primeiro
    /// `DeclaredIdentifier`, `SimpleFormalParameter`, `SuperFormalParameter`
    /// ou `VariableDeclarationList` entre o nó da seleção e os ancestrais.
    pub(crate) fn remove_type_annotation(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let target = self.with_parents(no).find(|&k| {
            matches!(self.kind(k), "DeclaredIdentifier" | "SimpleFormalParameter" | "SuperFormalParameter" | "VariableDeclarationList")
        })?;
        let edits = match self.kind(target) {
            "DeclaredIdentifier" => self.remove_from_declared_identifier(target)?,
            "SimpleFormalParameter" | "SuperFormalParameter" => {
                // `_removeTypeAnnotation`: do tipo ao token seguinte; num
                // `super.f(int a)`, também a lista de parâmetros.
                let ty = self.type_child(target)?;
                let type_end = self.tree.nos[ty].fim;
                let next = self.next_token(type_end)?.start;
                let mut v = vec![(Span { start: self.tree.nos[ty].inicio, end: next }, String::new())];
                if self.kind(target) == "SuperFormalParameter"
                    && let Some(list) = self.children(target).iter().copied().find(|&f| self.kind(f) == "FormalParameterList")
                {
                    v.push((self.tree.span(list), String::new()));
                }
                v
            }
            _ => self.remove_from_declaration_list(target, start)?,
        };
        Some(self.type_action(uri, "Remove type annotation", "refactor.remove.typeAnnotation", edits))
    }

    /// A palavra `final`/`const`/`var` da lista ou do identificador
    /// declarado (o `keyword`), entre `start` e `limit`.
    fn declaration_word(&self, start: usize, limit: usize) -> Option<&str> {
        self.tokens
            .iter()
            .filter(|t| t.span.start >= start && t.span.end <= limit)
            .map(|t| &self.source[t.span.start..t.span.end])
            .find(|s| matches!(*s, "final" | "const" | "var"))
    }

    /// `_removeFromDeclaredIdentifier`.
    fn remove_from_declared_identifier(&self, target: usize) -> Option<Vec<(Span, String)>> {
        let ty = self.type_child(target)?;
        // O nome: o último token do nó.
        let name = self.previous_token(self.tree.nos[target].fim)?;
        let word = self.declaration_word(self.tree.nos[target].inicio, self.tree.nos[ty].inicio);
        let range = Span { start: self.tree.nos[ty].inicio, end: name.start };
        Some(vec![(range, if word.is_some_and(|p| p != "var") { String::new() } else { "var ".to_string() })])
    }

    /// `_removeFromDeclarationList`.
    fn remove_from_declaration_list(&self, list: usize, selection: usize) -> Option<Vec<(Span, String)>> {
        let ty = self.type_child(list)?;
        let variables = self.kind_children(list, "VariableDeclaration");
        let first = *variables.first()?;
        let name = self.next_token(self.tree.nos[first].inicio)?;
        // Declaração incompleta (nome sintético).
        if variables.len() == 1 && name.start != self.tree.nos[first].inicio {
            return None;
        }
        if selection > name.end {
            return None;
        }
        // Sem inicializador não há outra fonte para o tipo.
        let mut initializer = *self.children(first).first()?;
        let mut arguments: Option<(String, usize)> = None;
        if self.kind(ty) == "NamedType"
            && let Some(args) = self.children(ty).iter().copied().find(|&f| self.kind(f) == "TypeArgumentList")
        {
            if self.kind(initializer) == "CascadeExpression" {
                initializer = *self.children(initializer).first()?;
            }
            let text = self.node_text(args).to_string();
            match self.kind(initializer) {
                "ListLiteral" | "SetOrMapLiteral" => {
                    if !self.children(initializer).iter().any(|&f| self.kind(f) == "TypeArgumentList") {
                        // O `leftBracket`.
                        let opens = self
                            .tokens
                            .iter()
                            .find(|t| t.span.start >= self.tree.nos[initializer].inicio && matches!(&self.source[t.span.start..t.span.end], "[" | "{"))?;
                        arguments = Some((text, opens.span.start));
                    }
                }
                "InstanceCreationExpression" => {
                    let constructor_name = self.children(initializer).iter().copied().find(|&f| self.kind(f) == "ConstructorName")?;
                    let created_type = self.children(constructor_name).iter().copied().find(|&f| self.kind(f) == "NamedType")?;
                    if !self.children(created_type).iter().any(|&f| self.kind(f) == "TypeArgumentList") {
                        arguments = Some((text, self.tree.nos[created_type].fim));
                    }
                }
                _ => {}
            }
        }
        // Um literal de conjunto ou mapa sem argumentos de tipo ficaria
        // ambíguo.
        if self.kind(initializer) == "SetOrMapLiteral"
            && !self.children(initializer).iter().any(|&f| self.kind(f) == "TypeArgumentList")
            && arguments.is_none()
        {
            return None;
        }
        let word = self.declaration_word(self.tree.nos[list].inicio, self.tree.nos[ty].inicio);
        let range = Span { start: self.tree.nos[ty].inicio, end: name.start };
        let mut v = vec![(range, if word.is_some_and(|p| p != "var") { String::new() } else { "var ".to_string() })];
        if let Some((text, em)) = arguments {
            v.push((Span { start: em, end: em }, text));
        }
        Some(v)
    }

    // -- AddTypeAnnotation -------------------------------------------------------

    /// `AddTypeAnnotation.compute` (add_type_annotation.dart): o nó da
    /// seleção como `SimpleFormalParameter`, `DeclaredVariablePattern` ou
    /// literal tipado; senão o primeiro `VariableDeclarationList`,
    /// `DeclaredIdentifier` ou `ForStatement` entre os ancestrais.
    pub(crate) fn add_type_annotation(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let change = match self.kind(no) {
            "SimpleFormalParameter" => self.simple_parameter_type(no)?,
            "DeclaredVariablePattern" => self.declared_pattern_type(no)?,
            "ListLiteral" | "SetOrMapLiteral" => self.literal_type(no)?,
            _ => {
                let target = self
                    .with_parents(no)
                    .find(|&k| matches!(self.kind(k), "VariableDeclarationList" | "DeclaredIdentifier" | "ForStatement"))?;
                match self.kind(target) {
                    "VariableDeclarationList" => self.declaration_list_type(target, start)?,
                    "DeclaredIdentifier" => self.declared_identifier_type(target)?,
                    _ => {
                        // `ForStatement` com `ForEachPartsWithDeclaration`, o nó
                        // antes do iterável.
                        let parts = self.children(target).iter().copied().find(|&f| self.kind(f) == "ForEachPartsWithDeclaration")?;
                        let declared = self.children(parts).iter().copied().find(|&f| self.kind(f) == "DeclaredIdentifier")?;
                        let iterable = *self.children(parts).last()?;
                        if self.tree.nos[no].inicio >= self.tree.nos[iterable].inicio {
                            return None;
                        }
                        self.declared_identifier_type(declared)?
                    }
                }
            }
        };
        let (edits, import) = change;
        let mut m = crate::refactorings_exec::Change::default();
        for (s, text) in edits {
            m.add(uri, s, text);
        }
        crate::refactorings_move::builder_imports(self, &mut m, &import);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Add type annotation".into(),
            kind: "refactor.add.typeAnnotation".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `_applyChange`: com `var`, o tipo no lugar dele; senão antes do nome
    /// (`canWriteType` falso: nada).
    #[allow(clippy::type_complexity)]
    fn apply_type(&self, word: Option<Span>, name: Span, t: TypeId) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        let mut writer = crate::write_type::Writer::new(self, name.start);
        let text = writer.write(t, false)?;
        let edit = match word {
            Some(p) if &self.source[p.start..p.end] == "var" => (p, text),
            _ => (Span { start: name.start, end: name.start }, format!("{text} ")),
        };
        Some((vec![edit], writer.import))
    }

    /// `InterfaceType` do analyzer: classe, `FutureOr`, tipo de extensão.
    fn is_interface_type(&self, t: TypeId) -> bool {
        matches!(self.p.query.table.get(t), Type::Interface { .. } | Type::FutureOr { .. } | Type::ExtensionType { .. })
    }

    /// `_forSimpleFormalParameter`.
    #[allow(clippy::type_complexity)]
    fn simple_parameter_type(&self, no: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        if self.type_child(no).is_some() {
            return None;
        }
        // O nome: o último token do nó.
        let name = self.previous_token(self.tree.nos[no].fim)?;
        if !self.source[name.start..name.end].chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$') {
            return None;
        }
        let t = self.bodies.tipo_local(name.start)?;
        if !(self.is_interface_type(t) || matches!(self.p.query.table.get(t), Type::Record { .. })) {
            return None;
        }
        self.apply_type(None, name, t)
    }

    /// O `PatternId` do nó de padrão (pelo intervalo).
    fn node_pattern(&self, no: usize) -> Option<ast::PatternId> {
        let s = self.tree.span(no);
        self.ast.patterns.iter().position(|p| p.span == s && matches!(p.kind, ast::PatternKind::Variable { .. })).map(|i| ast::PatternId(i as u32))
    }

    /// `DeclaredVariablePattern`: o `matchedValueType`.
    #[allow(clippy::type_complexity)]
    fn declared_pattern_type(&self, no: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        let pid = self.node_pattern(no)?;
        let ast::PatternKind::Variable { name, .. } = &self.ast.pattern(pid).kind else { return None };
        let t = *self.bodies.tipos_casados.get(&pid)?;
        let word = self
            .tokens
            .iter()
            .find(|tk| tk.span.start >= self.tree.nos[no].inicio && tk.span.end <= name.span.start && matches!(&self.source[tk.span.start..tk.span.end], "var" | "final"))
            .map(|tk| tk.span);
        self.apply_type(word, name.span, t)
    }

    /// `_typedLiteral`: `<arguments>` no `[`/`{`.
    #[allow(clippy::type_complexity)]
    fn literal_type(&self, no: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        let x = self.node_expr(no)?;
        let t = self.bodies.get_type(x)?;
        let args: Vec<TypeId> = match self.p.query.table.get(t) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
            Type::FutureOr { arg, .. } => vec![*arg],
            _ => return None,
        };
        let opens = self
            .tokens
            .iter()
            .find(|tk| tk.span.start >= self.tree.nos[no].inicio && matches!(&self.source[tk.span.start..tk.span.end], "[" | "{"))?
            .span
            .start;
        let mut writer = crate::write_type::Writer::new(self, opens);
        let texts: Vec<String> = args.iter().map(|&a| writer.write_type(Some(a), false).unwrap_or_default()).collect();
        Some((vec![(Span { start: opens, end: opens }, format!("<{}>", texts.join(", ")))], writer.import))
    }

    /// `_forDeclaredIdentifier`.
    #[allow(clippy::type_complexity)]
    fn declared_identifier_type(&self, no: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        if self.type_child(no).is_some() {
            return None;
        }
        let name = self.previous_token(self.tree.nos[no].fim)?;
        let t = self.bodies.tipo_local(name.start)?;
        if !(self.is_interface_type(t)
            || matches!(self.p.query.table.get(t), Type::Function { .. } | Type::Record { .. } | Type::TypeParameter { .. } | Type::Intersection { .. }))
        {
            return None;
        }
        let word = self
            .tokens
            .iter()
            .find(|tk| tk.span.start >= self.tree.nos[no].inicio && tk.span.end <= name.start && matches!(&self.source[tk.span.start..tk.span.end], "var" | "final" | "const"))
            .map(|tk| tk.span);
        self.apply_type(word, name, t)
    }

    /// `_typeForVariable`: o tipo do inicializador; sem ele, num local de
    /// bloco, o LUB dos tipos atribuídos nos comandos seguintes.
    fn variable_type(&self, variable: usize) -> Option<TypeId> {
        let name = self.next_token(self.tree.nos[variable].inicio)?;
        if let Some(&start) = self.children(variable).first() {
            return self.node_expr(start).and_then(|x| self.bodies.get_type(x));
        }
        // O comando de declaração e o bloco que o contém.
        let list = self.parent(variable)?;
        let command = self.parent(list)?;
        let block = self.parent(command)?;
        if self.kind(command) != "VariableDeclarationStatement" || self.kind(block) != "Block" {
            return None;
        }
        let command_end = self.tree.nos[command].fim;
        let block_end = self.tree.nos[block].fim;
        let mut types: Vec<TypeId> = Vec::new();
        for (i, e) in self.ast.exprs.iter().enumerate() {
            let ExprKind::Assign { op: ast::AssignOp::Assign, target, value } = &e.kind else { continue };
            if e.span.start < command_end || e.span.end > block_end {
                continue;
            }
            if !matches!(self.ast.expr(*target).kind, ExprKind::Identifier(_)) || self.bodies.declaracao_local(*target) != Some(name.start) {
                continue;
            }
            let _ = i;
            if let Some(t) = self.bodies.get_type(*value)
                && !types.contains(&t)
            {
                types.push(t);
            }
        }
        let (&first, rest) = types.split_first()?;
        if rest.is_empty() {
            return Some(first);
        }
        // `leastUpperBound` numa cópia da tabela: só vale um tipo que a
        // tabela da consulta já tem (o escritor lê esta).
        let query = &self.p.query;
        let mut table = query.table.clone();
        let mut env = dartforge_types::subtyping::SubtypeEnv::new(&mut table, &query.outline.hierarchy, &query.core);
        let mut best = first;
        for &t in rest {
            best = dartforge_types::bounds::up(best, t, &mut env);
        }
        ((best.0 as usize) < query.table.len()).then_some(best)
    }

    /// `_forVariableDeclaration`.
    #[allow(clippy::type_complexity)]
    fn declaration_list_type(&self, list: usize, selection: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        if self.type_child(list).is_some() {
            return None;
        }
        let variables = self.kind_children(list, "VariableDeclaration");
        let first = *variables.first()?;
        let name = self.next_token(self.tree.nos[first].inicio)?;
        if selection > name.end {
            return None;
        }
        let t = self.variable_type(first)?;
        for &v in &variables[1..] {
            if self.variable_type(v) != Some(t) {
                return None;
            }
        }
        let table = &self.p.query.table;
        let non_null_interface = self.is_interface_type(t) && !matches!(table.get(t), Type::Null);
        if !(non_null_interface || matches!(table.get(t), Type::Function { .. } | Type::Record { .. } | Type::TypeParameter { .. } | Type::Intersection { .. })) {
            return None;
        }
        let word = self
            .tokens
            .iter()
            .find(|tk| tk.span.start >= self.tree.nos[list].inicio && tk.span.end <= name.start && matches!(&self.source[tk.span.start..tk.span.end], "var" | "final" | "const"))
            .map(|tk| tk.span);
        let _ = StmtKind::Empty;
        self.apply_type(word, name, t)
    }

    /// `ReplaceWithVar._findType`: a lista de declaração dá o tipo dela; senão
    /// a anotação de tipo mais próxima.
    fn type_to_swap(&self, no: usize) -> Option<usize> {
        const ANNOTATIONS: &[&str] = &["NamedType", "GenericFunctionType", "RecordTypeAnnotation"];
        if self.kind(no) == "VariableDeclarationList" {
            return self.children(no).iter().copied().find(|&k| ANNOTATIONS.contains(&self.kind(k)));
        }
        self.with_parents(no).find(|&k| ANNOTATIONS.contains(&self.kind(k)))
    }

    /// O tipo declarado do local cujo nome começa em `name` (`dynamic` não
    /// conta: `staticType is DynamicType`).
    fn local_declared_type(&self, name: usize) -> Option<TypeId> {
        let t = *self.bodies.tipos_de_locais.get(&name)?;
        (!matches!(self.p.query.table.get(t), Type::Dynamic)).then_some(t)
    }

    /// `_canConvertVariableDeclarationList`.
    fn list_swappable_to_var(&self, list: usize) -> bool {
        // `node.type?.type`: sem anotação, nada.
        if !self.children(list).iter().any(|&f| matches!(self.kind(f), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation")) {
            return false;
        }
        let variables: Vec<usize> = self.children(list).iter().copied().filter(|&k| self.kind(k) == "VariableDeclaration").collect();
        let Some(&first) = variables.first() else { return false };
        let Some(name) = self.next_token(self.tree.nos[first].inicio) else { return false };
        let Some(declared) = self.local_declared_type(name.start) else { return false };
        variables.iter().all(|&v| {
            self.children(v).first().and_then(|&i| self.node_expr(i)).and_then(|x| self.bodies.get_type(x)) == Some(declared)
        })
    }

    /// `_canReplaceWithVar`, a partir do pai do nó da seleção.
    fn swappable_to_var(&self, no: usize) -> bool {
        let Some(parent) = self.parent(no) else { return false };
        for k in self.with_parents(parent) {
            match self.kind(k) {
                "VariableDeclarationStatement" | "ForPartsWithDeclarations" => {
                    let Some(&list) = self.children(k).iter().find(|&&f| self.kind(f) == "VariableDeclarationList") else { return false };
                    return self.list_swappable_to_var(list);
                }
                "ForEachPartsWithDeclaration" => {
                    let children = self.children(k);
                    let Some(&variable) = children.iter().find(|&&f| self.kind(f) == "DeclaredIdentifier") else { return false };
                    let Some(&iterable) = children.last() else { return false };
                    let Some(name) = self.previous_token(self.tree.nos[variable].fim) else { return false };
                    let Some(declared) = self.local_declared_type(name.start) else { return false };
                    // `loopVariable.type?.type`: sem anotação, nada.
                    if !self.children(variable).iter().any(|&f| matches!(self.kind(f), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation")) {
                        return false;
                    }
                    let Some(t) = self.node_expr(iterable).and_then(|x| self.bodies.get_type(x)) else { return false };
                    let Some(iterable_class) = self.p.query.core.iterable_class else { return false };
                    let query = &self.p.query;
                    let mut table = query.table.clone();
                    let t = dartforge_types::ops::non_nullable(t, &mut table);
                    let Some(instance) = query.outline.hierarchy.supertype_of(t, iterable_class, &mut table, &query.core) else { return false };
                    return matches!(table.get(instance), Type::Interface { args, .. } if args.first() == Some(&declared));
                }
                _ => {}
            }
        }
        false
    }

    /// Os argumentos de tipo (`TypeArgumentList`) filhos de `n`.
    fn type_arguments(&self, n: usize) -> Option<usize> {
        self.children(n).iter().copied().find(|&k| self.kind(k) == "TypeArgumentList")
    }

    /// `ReplaceWithVar` (replace_with_var.dart): o tipo de uma declaração local
    /// (comando, `for` ou `for-in`) igual ao do inicializador vira `var` (ou
    /// some, com `final`/`const`), e os argumentos de tipo passam para o
    /// literal ou a criação sem eles.
    pub(crate) fn swap_to_var(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let ty = self.type_to_swap(no)?;
        if !self.swappable_to_var(no) {
            return None;
        }
        let parent = self.parent(ty)?;
        let grandparent = self.parent(parent)?;
        let final_or_const_word = |list: usize| -> bool {
            let mut k = self.next_token(self.tree.nos[list].inicio);
            while let Some(s) = k {
                if s.start >= self.tree.nos[ty].inicio {
                    return false;
                }
                if matches!(&self.source[s.start..s.end], "final" | "const") {
                    return true;
                }
                k = self.next_token(s.end);
            }
            false
        };
        let mut edits: Vec<(Span, String)> = Vec::new();
        let type_args = (self.kind(ty) == "NamedType").then(|| self.type_arguments(ty)).flatten();
        if self.kind(parent) == "VariableDeclarationList" && matches!(self.kind(grandparent), "VariableDeclarationStatement" | "ForPartsWithDeclarations") {
            let variables: Vec<usize> = self.children(parent).iter().copied().filter(|&k| self.kind(k) == "VariableDeclaration").collect();
            if variables.len() != 1 {
                return None;
            }
            let mut initializer = self.children(variables[0]).first().copied();
            let mut transfer: Option<(String, usize)> = None;
            if let Some(args) = type_args {
                if let Some(i) = initializer
                    && self.kind(i) == "CascadeExpression"
                {
                    initializer = self.children(i).first().copied();
                }
                if let Some(i) = initializer {
                    match self.kind(i) {
                        "ListLiteral" | "SetOrMapLiteral" => {
                            if self.type_arguments(i).is_none() {
                                // `leftBracket.offset`.
                                let opens = self.tokens.iter().find(|t| t.span.start >= self.tree.nos[i].inicio && matches!(&self.source[t.span.start..t.span.end], "[" | "{"))?;
                                transfer = Some((self.node_text(args).to_string(), opens.span.start));
                            }
                        }
                        "InstanceCreationExpression" => {
                            let constructor_name = self.children(i).iter().copied().find(|&k| self.kind(k) == "ConstructorName")?;
                            let constructor_type = *self.children(constructor_name).first()?;
                            if self.type_arguments(constructor_type).is_none() {
                                transfer = Some((self.node_text(args).to_string(), self.tree.nos[constructor_type].fim));
                            }
                        }
                        _ => {}
                    }
                }
            }
            if let Some(i) = initializer
                && self.kind(i) == "SetOrMapLiteral"
                && self.type_arguments(i).is_none()
                && transfer.is_none()
            {
                return None;
            }
            if final_or_const_word(parent) {
                edits.push((Span { start: self.tree.nos[ty].inicio, end: self.tree.nos[variables[0]].inicio }, String::new()));
            } else {
                edits.push((self.tree.span(ty), "var".into()));
            }
            if let Some((text, em)) = transfer {
                edits.push((Span { start: em, end: em }, text));
            }
        } else if self.kind(parent) == "DeclaredIdentifier" && self.kind(grandparent) == "ForEachPartsWithDeclaration" {
            let mut transfer: Option<(String, usize)> = None;
            if let Some(args) = type_args {
                let iterable = *self.children(grandparent).last()?;
                if matches!(self.kind(iterable), "ListLiteral" | "SetOrMapLiteral") && self.type_arguments(iterable).is_none() {
                    transfer = Some((self.node_text(args).to_string(), self.tree.nos[iterable].inicio));
                }
            }
            let name = self.previous_token(self.tree.nos[parent].fim)?;
            if final_or_const_word(parent) {
                edits.push((Span { start: self.tree.nos[ty].inicio, end: name.start }, String::new()));
            } else {
                edits.push((self.tree.span(ty), "var".into()));
            }
            if let Some((text, em)) = transfer {
                edits.push((Span { start: em, end: em }, text));
            }
        } else {
            return None;
        }
        Some(self.type_action(uri, "Replace type annotation with 'var'", "refactor.replace.withVar", edits))
    }

    /// `_allParametersHaveTypes`: todo parâmetro (sem o valor padrão) é
    /// simples com tipo ou função tipada.
    fn typed_parameters(&self, list: usize) -> bool {
        self.children(list).iter().all(|&p| {
            let p = if self.kind(p) == "DefaultFormalParameter" { self.children(p).first().copied().unwrap_or(p) } else { p };
            match self.kind(p) {
                "SimpleFormalParameter" => self.children(p).iter().any(|&k| matches!(self.kind(k), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation")),
                "FunctionTypedFormalParameter" => true,
                _ => false,
            }
        })
    }

    /// `ConvertToGenericFunctionSyntax` (convert_to_generic_function_syntax.dart):
    /// `typedef R F(P);` vira `typedef F = R Function(P);` e o parâmetro
    /// `R f(P)` vira `R Function(P) f`; dentro de uma lista de parâmetros, só
    /// o próprio parâmetro.
    pub(crate) fn convert_to_function_syntax(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let target = self.with_parents(no).find(|&k| matches!(self.kind(k), "FunctionTypeAlias" | "FunctionTypedFormalParameter" | "FormalParameterList"))?;
        let children = self.children(target).to_vec();
        let list = *children.iter().rev().find(|&&k| self.kind(k) == "FormalParameterList")?;
        let parameter_types = children.iter().copied().find(|&k| self.kind(k) == "TypeParameterList");
        let return_ = children.iter().copied().find(|&k| RETURN_TYPES.contains(&self.kind(k)) && self.tree.nos[k].fim <= self.tree.nos[list].inicio);
        let parameters = self.node_text(list).to_string();
        match self.kind(target) {
            "FunctionTypeAlias" => {
                if !self.typed_parameters(list) {
                    return None;
                }
                // O nome: o token antes dos parâmetros de tipo ou da lista.
                let after_name = parameter_types.map_or(self.tree.nos[list].inicio, |t| self.tree.nos[t].inicio);
                let name = self.previous_token(after_name)?;
                let name_end = parameter_types.map_or(name.end, |t| self.tree.nos[t].fim);
                let function_name = &self.source[name.start..name_end];
                let swap = match return_ {
                    None => format!("{function_name} = Function{parameters}"),
                    Some(r) => format!("{function_name} = {} Function{parameters}", self.node_text(r)),
                };
                let typedef = self.next_token(self.tree.nos[target].inicio)?;
                let typedef = if &self.source[typedef.start..typedef.end] == "typedef" {
                    typedef
                } else {
                    self.tokens.iter().find(|t| t.span.start >= self.tree.nos[target].inicio && &self.source[t.span.start..t.span.end] == "typedef")?.span
                };
                let after_typedef = self.next_token(typedef.end)?;
                let semicolon = self.previous_token(self.tree.nos[target].fim)?;
                Some(self.type_action(
                    uri,
                    "Convert into 'Function' syntax",
                    "refactor.convert.toGenericFunctionSyntax",
                    vec![(Span { start: after_typedef.start, end: semicolon.start }, swap)],
                ))
            }
            "FunctionTypedFormalParameter" => {
                if !self.typed_parameters(list) {
                    return None;
                }
                let before_type = return_.map_or_else(
                    || {
                        let after = parameter_types.map_or(self.tree.nos[list].inicio, |t| self.tree.nos[t].inicio);
                        self.previous_token(after).map_or(self.tree.nos[target].inicio, |s| s.start)
                    },
                    |r| self.tree.nos[r].inicio,
                );
                let words: Vec<&str> = self
                    .tokens
                    .iter()
                    .filter(|t| t.span.start >= self.tree.nos[target].inicio && t.span.end <= before_type)
                    .map(|t| &self.source[t.span.start..t.span.end])
                    .collect();
                let required = if words.contains(&"required") { "required " } else { "" };
                let covariant = if words.contains(&"covariant") { "covariant " } else { "" };
                let return_type = return_.map_or(String::new(), |r| format!("{} ", self.node_text(r)));
                let after_name = parameter_types.map_or(self.tree.nos[list].inicio, |t| self.tree.nos[t].inicio);
                let name = self.previous_token(after_name)?;
                let tp = parameter_types.map_or(String::new(), |t| self.node_text(t).to_string());
                let question = if self.source[self.tree.nos[list].fim..self.tree.nos[target].fim].trim() == "?" { "?" } else { "" };
                let swap = format!(
                    "{required}{covariant}{return_type}Function{tp}{parameters}{question} {}",
                    &self.source[name.start..name.end]
                );
                Some(self.type_action(uri, "Convert into 'Function' syntax", "refactor.convert.toGenericFunctionSyntax", vec![(self.tree.span(target), swap)]))
            }
            _ => None,
        }
    }
}

/// As espécies de `TypeAnnotation`, que podem ser o tipo de retorno.
const RETURN_TYPES: &[&str] = &["NamedType", "GenericFunctionType", "RecordTypeAnnotation"];
