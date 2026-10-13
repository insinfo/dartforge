//! O `DeclarationHelper` do analysis server 3.6.2
//! (`AS:src/services/completion/dart/declaration_helper.dart`, lido inteiro),
//! o `VisibilityTracker` e a tradução dos candidatos dele em itens
//! (`candidate_suggestion.dart`, `suggestion_builder.dart`).
//!
//! A ordem das sugestões é a do Dart: os locais de dentro para fora (os
//! comandos de um bloco do último para o primeiro), os membros da
//! declaração envolvente, as declarações de topo de cada unidade da
//! biblioteca por categoria (classes, enums, extension types, mixins,
//! typedefs, acessores, extensões, funções, variáveis), os prefixos, os
//! imports na ordem das diretivas da unidade definidora (o `dart:core`
//! implícito por último) e os membros herdados. Os espaços de nomes do
//! Dart são `HashMap` de `String` (a ordem de iteração é a do hash da VM);
//! aqui a ordem dentro de um espaço de nomes é a dos nomes, o que só muda a
//! ordem entre candidatos de mesmo `matcherScore` (o cliente reordena pelo
//! `sortText`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::isp::{Flags, Isp, Operation, Feature};
use super::*;
use dartforge_elements::model::{ExtensionId, TypedefId, UnitId, VariableId, VariableRef};
use dartforge_frontend::token::Op;

/// O `importData` de um candidato: o prefixo pelo qual o elemento é
/// visível e se a biblioteca ainda não está importada.
#[derive(Debug, Clone)]
pub(super) struct Import {
    pub prefix: Option<String>,
    pub not_imported: bool,
}

/// Os elementos de uma unidade por categoria, na ordem de declaração
/// (`CompilationUnitElement.classes`, `.enums`…).
#[derive(Default)]
pub(super) struct OfUnit {
    pub(super) classes: Vec<ClassId>,
    pub(super) enums: Vec<ClassId>,
    pub(super) extension_types: Vec<ClassId>,
    pub(super) mixins: Vec<ClassId>,
    pub(super) typedefs: Vec<TypedefId>,
    pub(super) accessors: Vec<FunctionElementId>,
    pub(super) extensions: Vec<ExtensionId>,
    pub(super) functions: Vec<FunctionElementId>,
    pub(super) variables: Vec<VariableId>,
}

/// O nome de um item sem a chamada.
fn text_without_parens(s: &str) -> &str {
    s.split('(').next().unwrap_or(s)
}

/// O texto de um elemento sem o `=` de setter (`displayName`).
fn without_equal(s: &str) -> String {
    s.trim_end_matches('=').to_string()
}

impl<'a, 'c> Isp<'a, 'c> {
    // -- O estado ------------------------------------------------------------------

    /// `declarationHelper(...)`: a primeira chamada fixa as opções; o tipo
    /// de contexto função com retorno `void` desliga `mustBeNonVoid` e liga
    /// `preferNonInvocation`.
    pub(super) fn dh(&mut self, mut f: Flags) {
        if let Some(t) = self.context_type
            && let Type::Function { ret, .. } = self.query.table.get(t)
            && matches!(self.query.table.get(*ret), Type::Void)
        {
            f.non_void = false;
            f.prefer_without_invocation = true;
        }
        if self.flags.is_none() {
            self.flags = Some(f);
        }
    }

    /// As opções vigentes.
    pub(super) fn fl(&self) -> Flags {
        self.flags.clone().unwrap_or_default()
    }

    /// `state.matcher.score(text)`, `None` quando não casa.
    pub(super) fn match_score(&mut self, text: &str) -> Option<f64> {
        let s = self.score(text);
        (s != -1.0).then_some(s)
    }

    /// `VisibilityTracker.isVisible`.
    pub(super) fn visible(&mut self, name: &str, imp: Option<&Import>) -> bool {
        let q = match imp.and_then(|i| i.prefix.as_deref()) {
            Some(p) => format!("{p}.{name}"),
            None => name.to_string(),
        };
        if imp.is_some_and(|i| i.not_imported) {
            return !self.visible.contains(&q);
        }
        self.visible.insert(q)
    }

    /// `_recordOperation`.
    pub(super) fn register_operation(&mut self, o: Operation) {
        self.operations.push(o);
    }

    /// Marca os itens do coletor desde `before` como do passe, com o score;
    /// `identifier` é o `CompletionSuggestionKind.IDENTIFIER` de um
    /// executável (o rótulo mostra os parâmetros; a inserção não).
    pub(super) fn mark(&mut self, before: usize, score: f64, identifier: bool) {
        let method = self.super_method.clone();
        for i in &mut self.collector.items[before..] {
            i.of_pass = true;
            if identifier {
                i.identifier = true;
            }
            // `superMatchesFeature` e `isNoSuchMethodFeature` (o nome do
            // método que contém o `super.▮` não é penalizado).
            if let Some(m) = &method
                && i.rel.kind.is_some_and(|e| matches!(e, crate::relevance::Kind::Method | crate::relevance::Kind::Field))
                && text_without_parens(&i.insert) == m
            {
                i.rel.super_matches = true;
                i.rel.no_such_method = false;
            }
        }
        self.register(before, score);
    }

    /// `completionPrefix` (`prefix.`) nos itens desde `before`.
    pub(super) fn prefix(&mut self, before: usize, imp: Option<&Import>) {
        if let Some(p) = imp.and_then(|i| i.prefix.clone()) {
            for i in &mut self.collector.items[before..] {
                i.insert = format!("{p}.{}", i.insert);
                i.label = format!("{p}.{}", i.label);
            }
        }
    }

    /// O grupo antigo do item pela biblioteca dona (só para a ordem do
    /// caminho sem o passe).
    pub(super) fn group_of(&self, owning: LibraryId) -> u8 {
        if owning == self.library() { group::LIBRARY } else { group::IMPORTED }
    }

    /// `_executableSuggestionKind == IDENTIFIER`.
    pub(super) fn executable_identifier(&self) -> bool {
        self.fl().prefer_without_invocation
    }

    /// `request.target.isFunctionalArgument()`: o alvo é um argumento cujo
    /// parâmetro tem tipo de função (o tipo de contexto da posição).
    pub(super) fn functional_argument(&self) -> bool {
        let Some(c) = self.covered_node() else { return false };
        let mut n = c;
        if let Some(p) = self.parent(n)
            && self.kind(p) == "NamedExpression"
        {
            n = p;
        }
        self.parent(n).is_some_and(|p| self.kind(p) == "ArgumentList")
            && self.context_type.is_some_and(|t| matches!(self.query.table.get(t), Type::Function { .. }))
    }

    /// `type is VoidType` de um retorno.
    pub(super) fn is_void(&self, t: TypeId) -> bool {
        matches!(self.query.table.get(t), Type::Void)
    }

    /// O retorno de uma função (`returnType`).
    pub(super) fn return_of(&self, f: FunctionElementId) -> TypeId {
        self.query.outline.functions[f.0 as usize].return_type
    }

    /// `isVisibleIn`: da biblioteca do pedido, ou de nome público.
    pub(super) fn visible_in(&self, owning: LibraryId, name: &str) -> bool {
        owning == self.library() || !name.starts_with('_')
    }

    /// O `thisType` de uma classe (os parâmetros dela como argumentos).
    pub(super) fn type_this(&mut self, c: ClassId) -> TypeId {
        let params: Vec<_> = self.query.outline.classes.get(c.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
        let args: Box<[TypeId]> = params.iter().map(|&p| self.query.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let ty = if self.query.program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType {
            Type::ExtensionType { decl: c, args, nullable: false }
        } else {
            Type::Interface { class: c, args, nullable: false }
        };
        self.query.table.intern(ty)
    }

    /// A versão de linguagem de uma biblioteca habilita o recurso.
    pub(super) fn library_feature(&self, l: LibraryId, r: Feature) -> bool {
        let v = self.query.program.library(l).features.versao();
        let min = match r {
            Feature::ExtensionMethods => (2, 6),
            Feature::EnhancedEnums | Feature::SuperParameters => (2, 17),
            Feature::Patterns | Feature::ClassModifiers | Feature::SealedClass => (3, 0),
            Feature::InlineClass => (3, 3),
        };
        (v.major, v.minor) >= min
    }

    /// `isWildcardVariable`: `_` com o recurso de curingas.
    pub(super) fn wildcard(&self, name: &str) -> bool {
        name == "_" && self.features.tem(dartforge_frontend::Feature::WildcardVariables)
    }

    // -- Os itens ------------------------------------------------------------------

    /// Um local (variável ou parâmetro): `LocalVariableSuggestion`,
    /// `FormalParameterSuggestion`.
    pub(super) fn item_local(&mut self, name: &str, ty: Option<TypeId>, parameter: bool, score: f64) {
        let distance = self.distance;
        self.distance += 1;
        let before = self.collector.items.len();
        let detail = ty.map(|t| self.query.format(t));
        let item = self.collector.push(group::LOCAL, kind::VARIABLE, name.to_string(), name.to_string(), detail);
        item.rel.kind = Some(if parameter { crate::relevance::Kind::Parameter } else { crate::relevance::Kind::Local });
        item.rel.ty = ty;
        item.rel.local = true;
        item.rel.distance = Some(distance);
        self.mark(before, score, false);
    }

    /// O detalhe `(T a, …) → R` de um tipo de função com os nomes dos
    /// posicionais dados.
    pub(super) fn detail_with_names(&self, ty: TypeId, names: &[Option<String>]) -> String {
        let Type::Function { ret, positional, optional, named, .. } = self.query.table.get(ty).clone() else {
            return self.query.format(ty);
        };
        let p = |i: usize, t: TypeId| match names.get(i).cloned().flatten() {
            Some(n) => format!("{} {n}", self.query.format(t)),
            None => self.query.format(t),
        };
        let mut parts: Vec<String> = positional.iter().enumerate().map(|(i, &t)| p(i, t)).collect();
        if !optional.is_empty() {
            let o: Vec<String> = optional.iter().enumerate().map(|(i, &t)| p(positional.len() + i, t)).collect();
            parts.push(format!("[{}]", o.join(", ")));
        }
        if !named.is_empty() {
            let n: Vec<String> = named
                .iter()
                .map(|(s, t, r)| format!("{}{} {}", if *r { "required " } else { "" }, self.query.format(*t), self.query.name(*s)))
                .collect();
            parts.push(format!("{{{}}}", n.join(", ")));
        }
        format!("({}) → {}", parts.join(", "), self.query.format(ret))
    }

    /// Uma função local (`LocalFunctionSuggestion`): o nó é o
    /// `FunctionDeclaration` dela.
    pub(super) fn local_function_item(&mut self, no: usize, name: &str, ty: Option<TypeId>, score: f64) {
        let f = self.real_function(no);
        let names: Vec<Option<String>> = f
            .and_then(|f| f.parameters.as_ref())
            .map(|ps| ps.iter().filter(|p| p.kind != ast::ParameterKind::Named).map(|p| p.name.map(|n| self.real_name(n).to_string())).collect())
            .unwrap_or_default();
        let required: Vec<String> = f
            .and_then(|f| f.parameters.as_ref())
            .map(|ps| {
                ps.iter()
                    .filter_map(|p| {
                        let n = self.real_name(p.nome_externo()?).to_string();
                        match p.kind {
                            ast::ParameterKind::Required => Some(n),
                            ast::ParameterKind::Named if p.required => Some(format!("{n}: ")),
                            _ => None,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let detail = ty.map(|t| self.detail_with_names(t, &names));
        let before = self.collector.items.len();
        let item = self.collector.push(group::LOCAL, kind::FUNCTION, name.to_string(), name.to_string(), detail);
        item.call = Some(Call::Parameters(required));
        item.rel.kind = Some(crate::relevance::Kind::Function);
        item.rel.ty = ty.and_then(|t| return_(self.query, t));
        let ident = self.executable_identifier();
        self.mark(before, score, ident);
    }

    /// Um parâmetro de tipo (`TypeParameterSuggestion`).
    pub(super) fn type_parameter_item(&mut self, name: &str, score: f64) {
        let before = self.collector.items.len();
        self.collector.push(group::LOCAL, kind::TYPE_PARAMETER, name.to_string(), name.to_string(), None).rel.kind =
            Some(crate::relevance::Kind::TypeParameter);
        self.mark(before, score, false);
    }

    /// Um membro (método, acessor) com o tipo visto pelo receptor; a
    /// distância de herança é a partir de `reference`.
    pub(super) fn item_member(&mut self, f: FunctionElementId, ty: TypeId, reference: Option<ClassId>, score: f64, identifier: bool) {
        let before = self.collector.items.len();
        let of_object = self.query.core.object_class.is_some() && self.query.program.function(f).class == self.query.core.object_class;
        let g = if of_object { group::OBJECT_MEMBER } else { group::MEMBER };
        self.collector.receiver = reference;
        self.collector.function(&*self.query, g, f, ty);
        self.collector.receiver = None;
        self.mark(before, score, identifier);
    }

    /// Um campo (`FieldSuggestion`): constante de enum vira `Enum.value`.
    pub(super) fn item_field(&mut self, v: VariableId, ty: Option<TypeId>, reference: Option<ClassId>, score: f64) {
        let p = &self.query.program;
        let ve = p.variable(v);
        let enum_constant = ve.class.is_some_and(|c| p.class(c).enum_constants.contains(&v));
        let ty = ty.or_else(|| self.query.variable_type(v)).unwrap_or(self.query.core.dynamic_);
        let before = self.collector.items.len();
        match ve.getter {
            Some(g) => {
                self.collector.receiver = reference;
                self.collector.function(&*self.query, group::MEMBER, g, ty);
                self.collector.receiver = None;
            }
            None => {
                let name = self.query.name(ve.name).to_string();
                let detail = self.query.format(ty);
                let origin = self.query.origin(self.query.variable_start(v));
                let item = self.collector.push(group::MEMBER, kind::FIELD, name.clone(), name, Some(detail));
                item.origin = origin;
                item.rel.kind = Some(crate::relevance::Kind::Field);
                item.rel.ty = Some(ty);
            }
        }
        if enum_constant && let Some(c) = ve.class {
            let e = self.query.name(p.class(c).name).to_string();
            for i in &mut self.collector.items[before..] {
                i.insert = format!("{e}.{}", i.insert);
                i.label = i.insert.clone();
                i.rel.kind = Some(crate::relevance::Kind::NoTable);
                i.rel.distance = None;
            }
        }
        self.mark(before, score, false);
    }

    /// Um elemento de topo (classe, enum, mixin, extensão, typedef, função,
    /// acessor, variável) com o prefixo do import.
    pub(super) fn item_element(&mut self, el: Element, imp: Option<&Import>, score: f64, identifier: bool) {
        let before = self.collector.items.len();
        let g = self.group_of(super::element_library(self.query, el));
        self.collector.element(&*self.query, g, el);
        self.prefix(before, imp);
        self.mark(before, score, identifier);
    }

    /// Um construtor (`ConstructorSuggestion` pelo `suggestConstructor`): o
    /// texto já calculado, `INVOCATION` ou `IDENTIFIER`.
    pub(super) fn item_constructor(&mut self, f: FunctionElementId, completion: String, invocation: bool, imp: Option<&Import>, score: f64) {
        let query = &*self.query;
        let class = query.program.function(f).class;
        let signature = query.outline.functions[f.0 as usize].signature;
        let detail = query.function_detail(f, signature);
        let parameters: Vec<String> = query.outline.functions[f.0 as usize]
            .parameters
            .iter()
            .filter_map(|p| {
                let n = query.name(p.externo.or(p.name)?).to_string();
                match p.kind {
                    ast::ParameterKind::Required => Some(n),
                    ast::ParameterKind::Named if p.required => Some(format!("{n}: ")),
                    _ => None,
                }
            })
            .collect();
        let origin = query.origin(query.function_start(f));
        let owning = query.program.function(f).library;
        let g = self.group_of(owning);
        let before = self.collector.items.len();
        let item = self.collector.push(g, kind::CONSTRUCTOR, completion.clone(), completion, Some(detail));
        item.call = Some(Call::Parameters(parameters));
        item.origin = origin;
        item.rel.kind = Some(crate::relevance::Kind::Constructor);
        // `ctx(inst(class envolvente))`: a classe com argumentos `Never`.
        item.class = class;
        self.prefix(before, imp);
        self.mark(before, score, !invocation);
    }

    // -- addLexicalDeclarations ----------------------------------------------------

    /// `addLexicalDeclarations`.
    pub(super) fn lexical_dh(&mut self, n: usize) {
        let ty = self.fl().ty;
        let member = if ty { self.local_types(n) } else { self.local_declarations(n) };
        let Some(m) = member else { return };
        let mut parent = Some(self.parent(m).unwrap_or(m));
        if let Some(p) = parent {
            if matches!(self.kind(p), "ConstructorDeclaration" | "FieldDeclaration" | "MethodDeclaration") {
                parent = self.parent(p);
            } else if p == 0 {
                parent = Some(m);
            }
        }
        let mut top = None;
        if let Some(p) = parent
            && p != 0
            && self.parent(p) == Some(0)
        {
            top = Some(p);
            self.enclosing_node_members(p);
            parent = self.parent(p);
        }
        if parent == Some(0) {
            self.top_declarations();
            self.dh_import_prefixes();
            self.imported_declarations();
            self.register_operation(Operation::StaticMembers);
        }
        let f = self.fl();
        if let Some(t) = top
            && !f.is_static
            && !f.ty
        {
            self.inherited_members(t);
        }
    }

    /// `_addLocalDeclarations`: devolve o membro que contém as declarações.
    pub(super) fn local_declarations(&mut self, n: usize) -> Option<usize> {
        let mut previous: Option<usize> = None;
        let mut current = Some(n);
        while let Some(k) = current {
            match self.kind(k) {
                "Block" => {
                    let commands: Vec<usize> = self.children(k).to_vec();
                    self.visit_commands(&commands, previous);
                }
                "CatchClause" => self.visit_catch(k),
                "CommentReference" => return self.visit_comment_reference(k),
                "ConstructorDeclaration" => {
                    self.visit_parameter_list(self.child(k, "FormalParameterList"));
                    return Some(k);
                }
                "DeclaredVariablePattern" => self.visit_variable_pattern(k),
                "FieldDeclaration" => return Some(k),
                "ForElement" | "ForStatement" => {
                    let parts = self.children(k).first().copied();
                    if let Some(p) = parts
                        && Some(p) != previous
                    {
                        self.visit_for_parts(p);
                    }
                }
                "ForPartsWithDeclarations" => {
                    let variables = self.child(k, "VariableDeclarationList");
                    if variables != previous {
                        self.visit_for_parts(k);
                    }
                }
                "FunctionDeclaration" => {
                    if self.parent(k).is_none_or(|p| self.kind(p) != "FunctionDeclarationStatement") {
                        return Some(k);
                    }
                }
                "FunctionDeclarationStatement" => {
                    if let Some(d) = self.child(k, "FunctionDeclaration") {
                        self.suggest_local_function(d);
                    }
                }
                "FunctionExpression" => {
                    self.visit_parameter_list(self.child(k, "FormalParameterList"));
                    self.visit_type_parameters(self.child(k, "TypeParameterList"));
                }
                "IfElement" | "IfStatement" => self.visit_if(k),
                "MethodDeclaration" => {
                    self.visit_parameter_list(self.child(k, "FormalParameterList"));
                    self.visit_type_parameters(self.child(k, "TypeParameterList"));
                    return Some(k);
                }
                "SwitchCase" | "SwitchDefault" => {
                    let commands: Vec<usize> = self.children(k).iter().copied().filter(|&c| super::isp::is_command(self.kind(c))).collect();
                    self.visit_commands(&commands, previous);
                }
                "SwitchExpressionCase" => self.visit_switch_expression_case(k),
                "SwitchPatternCase" => self.visit_pattern_case(k, previous),
                "VariableDeclarationList" => self.visit_variable_list(k, previous),
                "CompilationUnit" => return Some(k),
                e if self.parent(k) == Some(0) && e != "ScriptTag" && !self.is_directive(k) => return Some(k),
                _ => {}
            }
            previous = Some(k);
            current = self.parent(k);
        }
        None
    }

    /// `_addLocalTypes`.
    pub(super) fn local_types(&mut self, n: usize) -> Option<usize> {
        let mut current = Some(n);
        while let Some(k) = current {
            match self.kind(k) {
                "CommentReference" => return Some(k),
                "ConstructorDeclaration" => {
                    self.visit_parameter_list(self.child(k, "FormalParameterList"));
                    return Some(k);
                }
                "FieldDeclaration" => return Some(k),
                "FunctionDeclaration" => {
                    if self.parent(k).is_none_or(|p| self.kind(p) != "FunctionDeclarationStatement") {
                        return Some(k);
                    }
                }
                "FunctionExpression" | "GenericFunctionType" => self.visit_type_parameters(self.child(k, "TypeParameterList")),
                "MethodDeclaration" => {
                    self.visit_type_parameters(self.child(k, "TypeParameterList"));
                    return Some(k);
                }
                "CompilationUnit" => return Some(k),
                e if self.parent(k) == Some(0) && e != "ScriptTag" && !self.is_directive(k) => return Some(k),
                _ => {}
            }
            current = self.parent(k);
        }
        None
    }

    /// `_suggestVariable` de um local declarado com o nome em `name_off`
    /// (`constant`: a declaração é `const`).
    pub(super) fn suggest_variable(&mut self, name: &str, name_off: usize, constant: bool) {
        if self.wildcard(name) || !self.visible(name, None) {
            return;
        }
        if self.fl().constant && !constant {
            return;
        }
        if let Some(s) = self.match_score(name) {
            let ty = self.local_type(name_off);
            self.item_local(name, ty, false, s);
        }
    }

    /// `_suggestParameter` (o nó é o do parâmetro).
    pub(super) fn suggest_parameter(&mut self, no: usize) {
        let Some(p) = self.real_parameter(no) else { return };
        let Some(name) = p.name else { return };
        let text = self.real_name(name).to_string();
        if !self.visible(&text, None) {
            return;
        }
        if self.fl().constant || text == "_" {
            return;
        }
        if let Some(s) = self.match_score(&text) {
            let ty = self.local_type(name.span.start).or_else(|| self.declared_parameter_type(no, &text));
            self.item_local(&text, ty, true, s);
        }
    }

    /// O tipo de um parâmetro pela assinatura da função ou do construtor
    /// declarado (quando o corpo não registrou o local).
    pub(super) fn declared_parameter_type(&self, no: usize, name: &str) -> Option<TypeId> {
        let list = self.parameter_list_of(no).or_else(|| self.parent(no))?;
        let owner = self.parent(list)?;
        let f = match self.kind(owner) {
            "ConstructorDeclaration" => self.declared_constructor(owner),
            "MethodDeclaration" => self.real_function(owner).and_then(|x| x.name).and_then(|n| self.declared_function(n.span.start)),
            "FunctionExpression" => {
                let d = self.parent(owner).filter(|&d| self.kind(d) == "FunctionDeclaration")?;
                self.real_function(d).and_then(|x| x.name).and_then(|n| self.declared_function(n.span.start))
            }
            _ => None,
        }?;
        self.query.outline.functions[f.0 as usize].parameters.iter().find(|p| p.name.is_some_and(|s| self.query.name(s) == name)).map(|p| p.ty)
    }

    /// `_suggestLocalFunction` (o nó é o `FunctionDeclaration`).
    pub(super) fn suggest_local_function(&mut self, d: usize) {
        let Some(f) = self.real_function(d) else { return };
        let Some(name) = f.name else { return };
        let text = self.real_name(name).to_string();
        if !self.visible(&text, None) {
            return;
        }
        let ty = self.local_type(name.span.start);
        let fl = self.fl();
        let ret_void = ty.is_some_and(|t| matches!(self.query.table.get(t), Type::Function { ret, .. } if self.is_void(*ret)));
        if fl.assignable || fl.constant || (fl.non_void && ret_void) {
            return;
        }
        if text == "_" {
            return;
        }
        if let Some(s) = self.match_score(&text) {
            self.local_function_item(d, &text, ty, s);
        }
    }

    /// `_visitCatchClause`.
    pub(super) fn visit_catch(&mut self, k: usize) {
        for p in self.children_of(k, &["CatchClauseParameter"]) {
            let name = self.source[self.start(p)..self.end(p)].to_string();
            let off = self.start(p);
            self.suggest_variable(&name, off, false);
        }
    }

    /// `_visitCommentReference`.
    pub(super) fn visit_comment_reference(&mut self, k: usize) -> Option<usize> {
        let comment = self.parent(k)?;
        if let Some(member) = self.parent(comment) {
            match self.kind(member) {
                "ConstructorDeclaration" => self.visit_parameter_list(self.child(member, "FormalParameterList")),
                "FunctionDeclaration" => {
                    if let Some(fe) = self.child(member, "FunctionExpression") {
                        self.visit_parameter_list(self.child(fe, "FormalParameterList"));
                        self.visit_type_parameters(self.child(fe, "TypeParameterList"));
                    }
                }
                "FunctionExpression" | "MethodDeclaration" => {
                    self.visit_parameter_list(self.child(member, "FormalParameterList"));
                    self.visit_type_parameters(self.child(member, "TypeParameterList"));
                }
                _ => {}
            }
        }
        Some(comment)
    }

    /// `_visitDeclaredVariablePattern`.
    pub(super) fn visit_variable_pattern(&mut self, k: usize) {
        if let Some(i) = self.variable_pattern_name(k) {
            let name = self.text_tok(i).to_string();
            let off = self.span_tok(i).start;
            self.suggest_variable(&name, off, false);
        }
    }

    /// `_visitForLoopParts`.
    pub(super) fn visit_for_parts(&mut self, k: usize) {
        match self.kind(k) {
            "ForEachPartsWithDeclaration" => {
                if let Some(d) = self.child(k, "DeclaredIdentifier")
                    && let Some(i) = self.tok_before(self.end(d))
                {
                    let name = self.text_tok(i).to_string();
                    let off = self.span_tok(i).start;
                    self.suggest_variable(&name, off, false);
                }
            }
            "ForEachPartsWithPattern" => {
                if let Some(p) = self.children(k).iter().copied().find(|&c| super::isp::is_pattern(self.kind(c))) {
                    self.visit_pattern(p);
                }
            }
            "ForPartsWithDeclarations" => {
                if let Some(l) = self.child(k, "VariableDeclarationList") {
                    let constant = self.real_list(l).is_some_and(|x| x.const_);
                    for v in self.children_of(l, &["VariableDeclaration"]) {
                        if let Some(var) = self.real_variable(v) {
                            let name = self.real_name(var.name).to_string();
                            self.suggest_variable(&name, var.name.span.start, constant);
                        }
                    }
                }
            }
            "ForPartsWithPattern" => {
                if let Some(d) = self.child(k, "PatternVariableDeclaration")
                    && let Some(p) = self.children(d).iter().copied().find(|&c| super::isp::is_pattern(self.kind(c)))
                {
                    self.visit_pattern(p);
                }
            }
            _ => {}
        }
    }

    /// `_visitIfElement` / `_visitIfStatement`.
    pub(super) fn visit_if(&mut self, k: usize) {
        let opens = self.op_in(Op::LParen, self.start(k), self.end(k));
        let after = opens.and_then(|a| self.pair(a)).map_or(self.start(k), |f| self.span_tok(f).end);
        let else_ = self.word_in("else", after, self.end(k));
        if else_.is_none_or(|e| self.offset < self.span_tok(e).start)
            && let Some(c) = self.child(k, "CaseClause")
            && let Some(g) = self.child(c, "GuardedPattern")
            && let Some(&p) = self.children(g).first()
        {
            self.visit_pattern(p);
        }
    }

    /// `_visitParameterList`.
    pub(super) fn visit_parameter_list(&mut self, list: Option<usize>) {
        let Some(l) = list else { return };
        for p in self.children(l).to_vec() {
            if super::isp::is_parameter(self.kind(p)) {
                self.suggest_parameter(p);
            }
        }
    }

    /// `_visitPattern`.
    pub(super) fn visit_pattern(&mut self, p: usize) {
        match self.kind(p) {
            "DeclaredVariablePattern" => self.visit_variable_pattern(p),
            "CastPattern" | "ListPattern" | "LogicalAndPattern" | "LogicalOrPattern" | "MapPattern" | "NullAssertPattern" | "NullCheckPattern"
            | "ObjectPattern" | "ParenthesizedPattern" | "RecordPattern" => {
                for c in self.children(p).to_vec() {
                    match self.kind(c) {
                        e if super::isp::is_pattern(e) => {
                            if !matches!(self.kind(p), "ObjectPattern" | "RecordPattern" | "MapPattern") {
                                self.visit_pattern(c);
                            }
                        }
                        "RestPatternElement" => {
                            if matches!(self.kind(p), "ListPattern" | "MapPattern")
                                && let Some(q) = self.children(c).iter().copied().find(|&x| super::isp::is_pattern(self.kind(x)))
                            {
                                self.visit_pattern(q);
                            }
                        }
                        "PatternField" => {
                            if let Some(q) = self.children(c).iter().copied().find(|&x| super::isp::is_pattern(self.kind(x))) {
                                self.visit_pattern(q);
                            }
                        }
                        "MapPatternEntry" => {
                            if let Some(&q) = self.children(c).last()
                                && super::isp::is_pattern(self.kind(q))
                            {
                                self.visit_pattern(q);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// `_visitStatements`: do último para o primeiro, sem o filho de onde se
    /// veio.
    pub(super) fn visit_commands(&mut self, commands: &[usize], child: Option<usize>) {
        for &c in commands.iter().rev() {
            if Some(c) == child {
                continue;
            }
            if self.start(c) >= self.offset {
                continue;
            }
            match self.kind(c) {
                "VariableDeclarationStatement" => {
                    let Some(l) = self.child(c, "VariableDeclarationList") else { continue };
                    let constant = self.real_list(l).is_some_and(|x| x.const_);
                    for v in self.children_of(l, &["VariableDeclaration"]) {
                        if self.end(v) < self.offset
                            && let Some(var) = self.real_variable(v)
                        {
                            let name = self.real_name(var.name).to_string();
                            self.suggest_variable(&name, var.name.span.start, constant);
                        }
                    }
                }
                "FunctionDeclarationStatement" => {
                    if let Some(d) = self.child(c, "FunctionDeclaration")
                        && self.start(d) < self.offset
                        && self.real_function(d).and_then(|f| f.name).is_some_and(|n| !self.real_name(n).is_empty())
                    {
                        self.suggest_local_function(d);
                    }
                }
                "PatternVariableDeclarationStatement" => {
                    if let Some(d) = self.child(c, "PatternVariableDeclaration")
                        && self.end(d) < self.offset
                        && let Some(p) = self.children(d).iter().copied().find(|&x| super::isp::is_pattern(self.kind(x)))
                    {
                        self.visit_pattern(p);
                    }
                }
                _ => {}
            }
        }
    }

    /// `_visitSwitchExpressionCase`.
    pub(super) fn visit_switch_expression_case(&mut self, k: usize) {
        let gp = self.child(k, "GuardedPattern");
        let arrow = self.op_in(Op::Arrow, gp.map_or(self.start(k), |g| self.end(g)), self.end(k).max(self.start(k) + 1));
        let end_arrow = arrow.map_or_else(|| self.synthetic(gp.map_or(self.start(k), |g| self.end(g))).end, |s| self.span_tok(s).end);
        if self.offset >= end_arrow
            && let Some(g) = gp
            && let Some(&p) = self.children(g).first()
        {
            self.visit_pattern(p);
        }
    }

    /// `_visitSwitchPatternCase`.
    pub(super) fn visit_pattern_case(&mut self, k: usize, child: Option<usize>) {
        let colon = self.case_colon(k);
        let end_dp = match self.tok_at(colon) {
            Some(i) if self.is_op(i, Op::Colon) => self.span_tok(i).end,
            _ => colon,
        };
        if self.offset < end_dp {
            return;
        }
        let commands: Vec<usize> = self.children(k).iter().copied().filter(|&c| super::isp::is_command(self.kind(c))).collect();
        self.visit_commands(&commands, child);
        if let Some(g) = self.child(k, "GuardedPattern")
            && let Some(&p) = self.children(g).first()
        {
            self.visit_pattern(p);
        }
        if let Some(sw) = self.parent(k)
            && self.kind(sw) == "SwitchStatement"
        {
            let members = self.children_of(sw, &["SwitchCase", "SwitchDefault", "SwitchPatternCase"]);
            if let Some(i) = members.iter().position(|&m| m == k) {
                for &m in members[..i].iter().rev() {
                    let empty = !self.children(m).iter().any(|&c| super::isp::is_command(self.kind(c)));
                    if self.kind(m) == "SwitchPatternCase" && empty {
                        if let Some(g) = self.child(m, "GuardedPattern")
                            && let Some(&p) = self.children(g).first()
                        {
                            self.visit_pattern(p);
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }

    /// `_visitTypeParameterList`.
    pub(super) fn visit_type_parameters(&mut self, list: Option<usize>) {
        let Some(l) = list else { return };
        if self.fl().excluded.contains(&l) {
            return;
        }
        for tp in self.children_of(l, &["TypeParameter"]) {
            let name = self.first_after_annotations(tp).map(|i| self.text_tok(i).to_string());
            if let Some(n) = name
                && n != "_"
            {
                self.suggest_type_parameter(&n);
            }
        }
    }

    /// `_suggestTypeParameter`.
    pub(super) fn suggest_type_parameter(&mut self, name: &str) {
        if self.visible(name, None)
            && let Some(s) = self.match_score(name)
        {
            self.type_parameter_item(name, s);
        }
    }

    /// `_visitVariableDeclarationList`: as variáveis antes do filho.
    pub(super) fn visit_variable_list(&mut self, k: usize, child: Option<usize>) {
        let Some(f) = child.filter(|&f| self.kind(f) == "VariableDeclaration") else { return };
        // Só listas locais declaram `LocalVariableElement`.
        let local = self.parent(k).is_some_and(|p| matches!(self.kind(p), "VariableDeclarationStatement" | "ForPartsWithDeclarations"));
        if !local {
            return;
        }
        let constant = self.real_list(k).is_some_and(|x| x.const_);
        let vs = self.children_of(k, &["VariableDeclaration"]);
        let Some(i) = vs.iter().position(|&v| v == f) else { return };
        for &v in vs[..i].iter().rev() {
            if let Some(var) = self.real_variable(v) {
                let name = self.real_name(var.name).to_string();
                self.suggest_variable(&name, var.name.span.start, constant);
            }
        }
    }

    // -- A declaração envolvente -----------------------------------------------------

    /// `_addMembersOfEnclosingNode`.
    pub(super) fn enclosing_node_members(&mut self, d: usize) {
        let ty = self.fl().ty;
        match self.kind(d) {
            "ClassDeclaration" | "EnumDeclaration" | "MixinDeclaration" | "ExtensionTypeDeclaration" => {
                let Some(c) = self.node_class(d) else { return };
                if !ty {
                    self.enclosing_instance_members(Some(c), None);
                    if self.kind(d) == "ExtensionTypeDeclaration"
                        && let Some(r) = self.query.program.class(c).representation
                    {
                        self.suggest_field(r, None, None);
                    }
                }
                let names: Vec<String> = self.query.program.class(c).type_params.iter().map(|t| self.query.name(t.name).to_string()).collect();
                self.suggest_type_parameters(&names);
            }
            "ExtensionDeclaration" => {
                let name = self.real_decl(d).and_then(|x| match &x.kind {
                    ast::DeclKind::Extension(e) => e.name.map(|n| n.span.start),
                    _ => None,
                });
                let Some(x) = self.extension_in(self.start(d), name) else { return };
                if !ty {
                    self.enclosing_instance_members(None, Some(x));
                }
                let names: Vec<String> = self.query.program.extension(x).type_params.iter().map(|t| self.query.name(t.name).to_string()).collect();
                self.suggest_type_parameters(&names);
            }
            "ClassTypeAlias" => {
                if let Some(c) = self.node_class(d) {
                    let names: Vec<String> = self.query.program.class(c).type_params.iter().map(|t| self.query.name(t.name).to_string()).collect();
                    self.suggest_type_parameters(&names);
                }
            }
            "FunctionTypeAlias" | "GenericTypeAlias" => {
                if let Some(t) = self.node_typedef(d) {
                    let names: Vec<String> = self.query.program.typedef(t).type_params.iter().map(|t| self.query.name(t.name).to_string()).collect();
                    self.suggest_type_parameters(&names);
                }
            }
            _ => {}
        }
    }

    /// O typedef declarado pelo nó.
    pub(super) fn node_typedef(&self, d: usize) -> Option<TypedefId> {
        let target = self.map(self.real_decl(d)?.span.start);
        let p = &self.query.program;
        (0..p.typedefs.len())
            .map(|i| TypedefId(i as u32))
            .find(|&t| p.typedef(t).decl.unit == self.unit && p.unit(self.unit).ast.decl(p.typedef(t).decl.decl).span.start == target)
    }

    /// `_suggestTypeParameters`.
    pub(super) fn suggest_type_parameters(&mut self, names: &[String]) {
        for n in names {
            if n != "_" {
                self.suggest_type_parameter(n);
            }
        }
    }

    /// Os membros declarados de uma classe (ou extensão): acessores
    /// declarados, campos, métodos, cada grupo na ordem de declaração.
    pub(super) fn declared_members(&self, c: Option<ClassId>, x: Option<ExtensionId>) -> (Vec<FunctionElementId>, Vec<VariableId>, Vec<FunctionElementId>) {
        let p = &self.query.program;
        let (inst, est, fields, constants): (Vec<FunctionElementId>, Vec<FunctionElementId>, Vec<VariableId>, Vec<VariableId>) = match (c, x) {
            (Some(c), _) => {
                let cl = p.class(c);
                (cl.instance_members.values().copied().collect(), cl.static_members.values().copied().collect(), cl.fields.clone(), cl.enum_constants.clone())
            }
            (None, Some(x)) => {
                let e = p.extension(x);
                (e.instance_members.values().copied().collect(), e.static_members.values().copied().collect(), e.fields.clone(), Vec::new())
            }
            _ => return (Vec::new(), Vec::new(), Vec::new()),
        };
        let mut all: Vec<FunctionElementId> = inst.into_iter().chain(est).collect();
        all.sort();
        all.dedup();
        let accessors = all.iter().copied().filter(|&f| matches!(p.function(f).kind, FunctionKind::Getter | FunctionKind::Setter)).collect();
        let methods = all.iter().copied().filter(|&f| matches!(p.function(f).kind, FunctionKind::Function | FunctionKind::Operator)).collect();
        let mut vs = constants;
        for v in fields {
            if !vs.contains(&v) {
                vs.push(v);
            }
        }
        (accessors, vs, methods)
    }

    /// `_addMembersOfEnclosingInstance`.
    pub(super) fn enclosing_instance_members(&mut self, c: Option<ClassId>, x: Option<ExtensionId>) {
        let is_static = self.fl().is_static;
        let reference = match (c, x) {
            (Some(c), _) => Some(c),
            (None, Some(x)) => {
                let on = self.query.outline.extensions[x.0 as usize].on;
                match self.query.table.get(on) {
                    Type::Interface { class, .. } => Some(*class),
                    Type::ExtensionType { decl, .. } => Some(*decl),
                    _ => None,
                }
            }
            _ => None,
        };
        let (accessors, fields, methods) = self.declared_members(c, x);
        for f in accessors {
            if !is_static || self.query.program.function(f).static_ {
                self.suggest_property(f, None, false, None, reference);
            }
        }
        for v in fields {
            let ve = self.query.program.variable(v);
            if !is_static || ve.static_ {
                self.suggest_field(v, None, reference);
            }
        }
        for f in methods {
            if !is_static || self.query.program.function(f).static_ {
                self.suggest_method(f, None, false, None, reference);
            }
        }
        let this = match (c, x) {
            (Some(c), _) => Some(self.type_this(c)),
            (None, Some(x)) => Some(self.query.outline.extensions[x.0 as usize].on),
            _ => None,
        };
        if let Some(t) = this
            && matches!(self.query.table.get(t), Type::Interface { .. } | Type::ExtensionType { .. })
        {
            self.extension_members(t, &HashSet::new(), true, true);
        }
    }

    // -- As sugestões de membros -----------------------------------------------------

    /// `_suggestField`.
    pub(super) fn suggest_field(&mut self, v: VariableId, ty: Option<TypeId>, reference: Option<ClassId>) {
        let ve = self.query.program.variable(v).clone();
        let name = self.query.name(ve.name).to_string();
        if !self.visible(&name, None) {
            return;
        }
        let f = self.fl();
        if (f.assignable && ve.setter.is_none()) || (f.constant && !ve.const_) {
            return;
        }
        if let Some(s) = self.match_score(&name) {
            self.item_field(v, ty, reference, s);
        }
    }

    /// `_suggestMethod`.
    pub(super) fn suggest_method(&mut self, m: FunctionElementId, ty: Option<TypeId>, ignore_visibility: bool, imp: Option<&Import>, reference: Option<ClassId>) {
        let name = without_equal(self.query.name(self.query.program.function(m).name));
        if !ignore_visibility && !self.visible(&name, imp) {
            return;
        }
        let f = self.fl();
        let ty = ty.unwrap_or_else(|| super::declared_type(self.query, m));
        let ret = match self.query.table.get(ty) {
            Type::Function { ret, .. } => *ret,
            _ => self.return_of(m),
        };
        if f.assignable || f.constant || (f.non_void && self.is_void(ret)) {
            return;
        }
        let Some(s) = self.match_score(&name) else { return };
        // Operadores: o `_getCompletionString` é nulo.
        if self.query.program.function(m).kind == FunctionKind::Operator {
            return;
        }
        if name == "setState" && self.is_exact_state(m) {
            self.item_set_state(m, reference, s);
            return;
        }
        let ident = f.prefer_without_invocation || self.functional_argument();
        let before = self.collector.items.len();
        self.item_member(m, ty, reference, s, ident);
        self.prefix(before, imp);
    }

    /// `ClassElement.isExactState`: o `State` do Flutter.
    pub(super) fn is_exact_state(&self, m: FunctionElementId) -> bool {
        let p = &self.query.program;
        let Some(c) = p.function(m).class else { return false };
        self.query.name(p.class(c).name) == "State" && p.library(p.class(c).library).uri == "package:flutter/src/widgets/framework.dart"
    }

    /// `SetStateMethodSuggestion`: `setState(() {\n$indent  \n$indent});`.
    pub(super) fn item_set_state(&mut self, m: FunctionElementId, reference: Option<ClassId>, score: f64) {
        let indent = self.line_indent();
        let completion = format!("setState(() {{\n{indent}  \n{indent}}});");
        let selection = format!("setState(() {{\n{indent}  ").encode_utf16().count();
        let ty = super::declared_type(self.query, m);
        let before = self.collector.items.len();
        self.item_member(m, ty, reference, score, true);
        for i in &mut self.collector.items[before..] {
            i.insert = completion.clone();
            i.display = Some("setState(() {});".to_string());
            i.selection = Some((selection, 0));
            i.call = None;
        }
    }

    /// `state.indent`: o recuo da linha do cursor.
    pub(super) fn line_indent(&self) -> String {
        // `getRequestLineIndent`: o branco do começo da linha até o primeiro
        // caractere que não é branco antes do cursor.
        let o = self.offset.min(self.source.len());
        let start = self.source[..o].rfind('\n').map_or(0, |i| i + 1);
        self.source[start..o].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
    }

    /// `_suggestProperty`: `f` é o getter ou o setter declarado (ou o
    /// acessor implícito de um campo).
    pub(super) fn suggest_property(&mut self, f: FunctionElementId, ty: Option<TypeId>, ignore_visibility: bool, imp: Option<&Import>, reference: Option<ClassId>) {
        let fe = self.query.program.function(f).clone();
        let name = without_equal(self.query.name(fe.name));
        if !ignore_visibility && !self.visible(&name, imp) {
            return;
        }
        let fl = self.fl();
        let getter = matches!(fe.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) && !self.query.name(fe.name).ends_with('=');
        let without_setter = getter && self.matching_setter(f).is_none();
        let ret = if getter { ty.unwrap_or_else(|| self.return_of(f)) } else { self.query.core.void_ };
        if (fl.assignable && without_setter) || fl.constant || (fl.non_void && self.is_void(ret)) {
            return;
        }
        let Some(s) = self.match_score(&name) else { return };
        if fe.kind == FunctionKind::ImplicitAccessor {
            // Sintético: só o getter, como `FieldSuggestion` da variável.
            if getter && let Some(v) = fe.variable {
                let before = self.collector.items.len();
                self.item_field(v, ty, reference, s);
                self.prefix(before, imp);
            }
            return;
        }
        let ty = ty.unwrap_or_else(|| super::declared_type(self.query, f));
        let before = self.collector.items.len();
        self.item_member(f, ty, reference, s, false);
        self.prefix(before, imp);
    }

    /// `correspondingSetter` de um getter (de membro ou de topo).
    pub(super) fn matching_setter(&self, f: FunctionElementId) -> Option<FunctionElementId> {
        let p = &self.query.program;
        let fe = p.function(f);
        if let Some(v) = fe.variable {
            return p.variable(v).setter;
        }
        let name = format!("{}=", self.query.name(fe.name));
        let s = self.query.names.lookup(&name)?;
        if let Some(c) = fe.class {
            let cl = p.class(c);
            return if fe.static_ { cl.static_members.get(&s).copied() } else { cl.instance_members.get(&s).copied() };
        }
        if let Some(x) = fe.extension {
            let e = p.extension(x);
            return if fe.static_ { e.static_members.get(&s).copied() } else { e.instance_members.get(&s).copied() };
        }
        let b = p.library(fe.library).declared.get(&self.query.names.lookup(self.query.name(fe.name))?)?;
        match b.setter {
            Some(Element::Function(g)) => Some(g),
            _ => None,
        }
    }

    // -- Os membros de instância ------------------------------------------------------

    /// Os membros de instância visíveis numa classe, na ordem dos mapas do
    /// `InheritanceManager3` (`inheritance_manager3.dart`): o
    /// `getInterface(c).map`; com `so_super`, o `getInheritedConcreteMap2`
    /// (`superImplemented.last`); com `inherited`, o `getInheritedMap2` (os
    /// `overridden`, na ordem dos candidatos).
    pub(super) fn interface_members(&self, c: ClassId, so_super: bool, inherited: bool) -> Vec<FunctionElementId> {
        let mut cache = HashMap::new();
        let mut processing = HashSet::new();
        let i = interface_of(self.query, self.query.core.object_class, self.query.program.dono_da_classe(c), &mut cache, &mut processing);
        let map = if so_super {
            &i.implemented_super
        } else if inherited {
            &i.overridden
        } else {
            &i.map
        };
        map.order.iter().map(|k| map.values[k]).collect()
    }

    /// `_canAccessInstanceMember`: não estático, acessível (nome privado só
    /// na biblioteca dele), e as anotações `@internal`, `@protected` e
    /// `@visibleForTesting` do `package:meta`.
    pub(super) fn can_access_member(&self, f: FunctionElementId) -> bool {
        let p = &self.query.program;
        let fe = p.function(f);
        if fe.static_ {
            return false;
        }
        let lib = self.library();
        if self.query.name(fe.name).starts_with('_') && fe.library != lib {
            return false;
        }
        let annotations = self.member_annotations(f);
        let same_package = || {
            let a = p.library(fe.library).uri.split('/').next().map(str::to_string);
            let b = p.library(lib).uri.split('/').next().map(str::to_string);
            fe.library == lib || (a.is_some() && a == b && p.library(fe.library).uri.starts_with("package:")) || !p.library(fe.library).uri.starts_with("package:")
        };
        if annotations.iter().any(|a| a == "internal") && !same_package() {
            return false;
        }
        if annotations.iter().any(|a| a == "protected")
            && let Some(owning) = fe.class
            && p.class(owning).library != lib
        {
            let Some(context) = self.target_enclosing_class() else { return false };
            if inheritance_distance(self.query, context, owning).is_none() {
                return false;
            }
        }
        if annotations.iter().any(|a| a == "visibleForTesting") && fe.library != lib {
            if !same_package() {
                return false;
            }
            let in_test = p.unit(self.unit).path.as_ref().is_some_and(|c| c.components().any(|x| x.as_os_str() == "test"));
            if !in_test {
                return false;
            }
        }
        true
    }

    /// Os nomes das anotações de um membro (o último segmento).
    pub(super) fn member_annotations(&self, f: FunctionElementId) -> Vec<String> {
        let p = &self.query.program;
        let meta: &[ast::Annotation] = match p.function(f).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                let a = &p.unit(unit).ast;
                match a.members.iter().find(|m| matches!(m.kind, ast::MemberKind::Method(x) if x == function)) {
                    Some(m) => &m.metadata,
                    None => &[],
                }
            }
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => &p.unit(unit).ast.member(member).metadata,
            dartforge_elements::model::FunctionRef::None => match p.function(f).variable.map(|v| p.variable(v).node) {
                Some(VariableRef::Field { unit, member, .. }) => &p.unit(unit).ast.member(member).metadata,
                _ => &[],
            },
        };
        meta.iter().filter_map(|m| m.name.last().map(|n| self.query.name(n.sym).to_string())).collect()
    }

    /// `request.target.enclosingInterfaceElement`.
    pub(super) fn target_enclosing_class(&self) -> Option<ClassId> {
        let c = self.covered_node()?;
        let d = self.ancestor(c, &["ClassDeclaration", "MixinDeclaration", "EnumDeclaration", "ExtensionTypeDeclaration"])?;
        self.node_class(d)
    }

    /// `_addInstanceMembers`.
    pub(super) fn instance_members(&mut self, ty: TypeId, excluded: &HashSet<String>, methods: bool, setters: bool, so_super: bool) {
        let class_ = match self.query.table.get(ty) {
            Type::Interface { class, .. } => Some(*class),
            Type::ExtensionType { decl, .. } => Some(*decl),
            Type::FutureOr { .. } | Type::Null => self.query.core.object_class,
            _ => None,
        };
        let Some(c) = class_ else { return };
        let lib = self.library();
        let members = self.interface_members(c, so_super, false);
        // `membersByName` pelo `displayName`; `bestMember`: o getter antes
        // do setter.
        let mut by_name: Vec<(String, Vec<FunctionElementId>)> = Vec::new();
        for f in members {
            if !self.can_access_member(f) {
                continue;
            }
            let name = without_equal(self.query.name(self.query.program.function(f).name));
            match by_name.iter_mut().find(|(n, _)| *n == name) {
                Some((_, v)) => v.push(f),
                None => by_name.push((name, vec![f])),
            }
        }
        let reference = Some(c);
        for (name, list) in by_name {
            let p = &self.query.program;
            let mut best = list[0];
            if p.function(best).kind == FunctionKind::Setter
                && let Some(&g) = list.iter().find(|&&g| matches!(p.function(g).kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor))
            {
                best = g;
            }
            let fe = p.function(best);
            let symbol = fe.name;
            let is_setter = fe.kind == FunctionKind::Setter || (fe.kind == FunctionKind::ImplicitAccessor && self.query.name(symbol).ends_with('='));
            match fe.kind {
                FunctionKind::Function => {
                    if methods {
                        let t = self.query.resolver().lookup_member(ty, symbol, false, lib).map(|(_, t)| t);
                        self.suggest_method(best, t, false, None, reference);
                    }
                }
                FunctionKind::Operator => {
                    if methods {
                        self.suggest_method(best, None, false, None, reference);
                    }
                }
                FunctionKind::Getter | FunctionKind::ImplicitAccessor | FunctionKind::Setter => {
                    if (!is_setter && !excluded.contains(&name)) || (setters && is_setter) {
                        let search = if is_setter { self.query.names.lookup(&name) } else { Some(symbol) };
                        let t = search.and_then(|s| self.query.resolver().lookup_member(ty, s, is_setter, lib)).map(|(_, t)| t);
                        self.suggest_property(best, t, false, None, reference);
                    }
                }
                _ => {}
            }
        }
        // Os campos (sem acessor no mapa de membros): pelo getter implícito.
        let function = self.query.core.function_class;
        let is_function = Some(c) == function;
        let extends_function = function.is_some_and(|fc| inheritance_distance(self.query, c, fc).is_some_and(|d| d > 0));
        if (is_function && !so_super) || extends_function {
            self.suggest_call();
        }
        self.extension_members(ty, excluded, methods, setters);
        self.register_operation(Operation::ExtensionMembers {
            ty,
            excluded: excluded.iter().cloned().collect(),
            methods,
            setters,
        });
    }

    /// `_suggestFunctionCall`.
    pub(super) fn suggest_call(&mut self) {
        if let Some(s) = self.match_score("call") {
            let before = self.collector.items.len();
            let item = self.collector.push(group::MEMBER, kind::METHOD, "call".into(), "call".into(), Some("() → void".into()));
            item.display = Some("call()".into());
            item.call = Some(Call::Parameters(Vec::new()));
            item.rel.fixed = Some(200);
            self.mark(before, s, false);
        }
    }

    /// `_addExtensionMembers`.
    pub(super) fn extension_members(&mut self, ty: TypeId, _excluded: &HashSet<String>, methods: bool, setters: bool) {
        let target = if matches!(self.query.table.get(ty), Type::Null) {
            ty
        } else {
            dartforge_types::non_nullable(ty, &mut self.query.table)
        };
        let lib = self.library();
        let applicable = {
            let Query { program, names, table, core, outline, .. } = &mut *self.query;
            dartforge_types::extensoes_aplicaveis(program, names, table, core, outline, lib, target)
        };
        for (x, _) in applicable {
            let (accessors, fields, ms) = self.declared_members(None, Some(x));
            if methods {
                for m in ms {
                    if !self.query.program.function(m).static_ {
                        self.suggest_method(m, None, false, None, None);
                    }
                }
            }
            for f in accessors {
                let fe = self.query.program.function(f);
                if fe.static_ {
                    continue;
                }
                let is_setter = fe.kind == FunctionKind::Setter;
                if !is_setter || setters {
                    self.suggest_property(f, None, false, None, None);
                }
            }
            for v in fields {
                if !self.query.program.variable(v).static_ {
                    self.suggest_field(v, None, None);
                }
            }
        }
    }

    /// `_addFieldsOfRecordType`.
    pub(super) fn record_fields(&mut self, ty: TypeId, excluded: &HashSet<String>) {
        let Type::Record { positional, named, .. } = self.query.table.get(ty).clone() else { return };
        for (i, t) in positional.iter().enumerate() {
            self.record_field_item(&format!("${}", i + 1), *t);
        }
        for (n, t) in named.iter() {
            let name = self.query.name(*n).to_string();
            if !excluded.contains(&name) {
                self.record_field_item(&name, *t);
            }
        }
    }

    /// `_suggestRecordField`.
    pub(super) fn record_field_item(&mut self, name: &str, ty: TypeId) {
        if let Some(s) = self.match_score(name) {
            let before = self.collector.items.len();
            let detail = self.query.format(ty);
            let item = self.collector.push(group::MEMBER, kind::VARIABLE, name.to_string(), name.to_string(), Some(detail));
            item.rel.ty = Some(ty);
            self.mark(before, s, false);
        }
    }

    /// `_addMembersOfDartCoreObject`.
    pub(super) fn object_members(&mut self) {
        let o = self.query.core.object;
        self.instance_members(o, &HashSet::new(), true, true, false);
    }

    /// `addInstanceMembersOfType`.
    pub(super) fn dh_instance_members(&mut self, ty: TypeId, so_super: bool) {
        let mut t = ty;
        for _ in 0..16 {
            match self.query.table.get(t).clone() {
                Type::TypeParameter { param, .. } => t = self.query.table.param(param).bound,
                Type::Intersection { bound, .. } => t = bound,
                _ => break,
            }
        }
        let assignable = self.fl().assignable;
        match self.query.table.get(t).clone() {
            Type::Interface { .. } | Type::ExtensionType { .. } | Type::FutureOr { .. } | Type::Null => {
                self.instance_members(t, &HashSet::new(), !assignable, true, so_super);
            }
            Type::Record { .. } => {
                self.record_fields(t, &HashSet::new());
                self.object_members();
                self.extension_members(t, &HashSet::new(), !assignable, true);
                self.register_operation(Operation::ExtensionMembers { ty: t, excluded: Vec::new(), methods: !assignable, setters: true });
            }
            Type::Function { .. } => {
                self.suggest_call();
                self.object_members();
            }
            Type::Dynamic => self.object_members(),
            _ => {}
        }
    }

    /// `addGetters`.
    pub(super) fn dh_getters(&mut self, ty: TypeId, excluded: &HashSet<String>) {
        match self.query.table.get(ty).clone() {
            Type::Interface { .. } | Type::ExtensionType { .. } => self.instance_members(ty, excluded, false, false, false),
            Type::Record { .. } => {
                self.record_fields(ty, excluded);
                self.object_members();
            }
            _ => {}
        }
    }

    /// `_addInheritedMembers`.
    pub(super) fn inherited_members(&mut self, d: usize) {
        match self.kind(d) {
            "ExtensionDeclaration" => {
                let name = self.real_decl(d).and_then(|x| match &x.kind {
                    ast::DeclKind::Extension(e) => e.name.map(|n| n.span.start),
                    _ => None,
                });
                let Some(x) = self.extension_in(self.start(d), name) else { return };
                if self.fl().is_static {
                    return;
                }
                let on = self.query.outline.extensions[x.0 as usize].on;
                if matches!(self.query.table.get(on), Type::Interface { .. } | Type::ExtensionType { .. }) {
                    self.instance_members(on, &HashSet::new(), true, true, false);
                }
            }
            "ClassDeclaration" | "EnumDeclaration" | "MixinDeclaration" | "ExtensionTypeDeclaration" | "ClassTypeAlias" => {
                let Some(c) = self.node_class(d) else { return };
                let this = self.type_this(c);
                let lib = self.library();
                for f in self.interface_members(c, false, true) {
                    let fe = self.query.program.function(f);
                    let symbol = fe.name;
                    match fe.kind {
                        FunctionKind::Function => {
                            let t = self.query.resolver().lookup_member(this, symbol, false, lib).map(|(_, t)| t);
                            self.suggest_method(f, t, false, None, Some(c));
                        }
                        FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                            let is_setter = self.query.name(symbol).ends_with('=');
                            let search = if is_setter { self.query.names.lookup(self.query.name(symbol).trim_end_matches('=')) } else { Some(symbol) };
                            let t = search.and_then(|s| self.query.resolver().lookup_member(this, s, is_setter, lib)).map(|(_, t)| t);
                            self.suggest_property(f, t, false, None, Some(c));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // -- As declarações de topo ------------------------------------------------------

    /// Os elementos de cada categoria de uma unidade, na ordem de
    /// declaração.
    pub(super) fn unit_elements(&self, u: UnitId) -> OfUnit {
        use dartforge_elements::model::ClassKind as K;
        let p = &self.query.program;
        let ast_u = &p.unit(u).ast;
        let pos_decl = |d: ast::DeclId| ast_u.decl(d).span.start;
        let mut r = OfUnit::default();
        let mut classes: Vec<(usize, ClassId)> = Vec::new();
        for (i, c) in p.classes.iter().enumerate() {
            if let Some(d) = c.decl
                && d.unit == u
            {
                classes.push((pos_decl(d.decl), ClassId(i as u32)));
            }
        }
        classes.sort();
        for (_, c) in classes {
            match p.class(c).kind {
                K::Class | K::MixinApplication => r.classes.push(c),
                K::Enum => r.enums.push(c),
                K::ExtensionType => r.extension_types.push(c),
                K::Mixin => r.mixins.push(c),
            }
        }
        let mut tds: Vec<(usize, TypedefId)> =
            p.typedefs.iter().enumerate().filter(|(_, t)| t.decl.unit == u).map(|(i, t)| (pos_decl(t.decl.decl), TypedefId(i as u32))).collect();
        tds.sort();
        r.typedefs = tds.into_iter().map(|(_, t)| t).collect();
        let mut xs: Vec<(usize, ExtensionId)> =
            p.extensions.iter().enumerate().filter(|(_, x)| x.decl.unit == u).map(|(i, x)| (pos_decl(x.decl.decl), ExtensionId(i as u32))).collect();
        xs.sort();
        r.extensions = xs.into_iter().map(|(_, x)| x).collect();
        for (i, f) in p.functions.iter().enumerate() {
            if f.class.is_some() || f.extension.is_some() {
                continue;
            }
            let dartforge_elements::model::FunctionRef::Function { unit, .. } = f.node else { continue };
            if unit != u {
                continue;
            }
            let id = FunctionElementId(i as u32);
            match f.kind {
                FunctionKind::Getter | FunctionKind::Setter => r.accessors.push(id),
                FunctionKind::Function => r.functions.push(id),
                _ => {}
            }
        }
        for (i, v) in p.variables.iter().enumerate() {
            if let VariableRef::TopLevel { unit, .. } = v.node
                && unit == u
            {
                r.variables.push(VariableId(i as u32));
            }
        }
        r
    }

    /// `_addTopLevelDeclarations` da biblioteca do pedido.
    pub(super) fn top_declarations(&mut self) {
        let units = self.query.program.library(self.library()).units.clone();
        let ty = self.fl().ty;
        for u in units {
            let e = self.unit_elements(u);
            for c in e.classes {
                self.suggest_class(c, None);
            }
            for c in e.enums {
                self.suggest_enum(c, None);
            }
            for c in e.extension_types {
                self.suggest_extension_type(c, None);
            }
            for c in e.mixins {
                self.suggest_mixin(c, None);
            }
            for t in e.typedefs {
                self.suggest_typedef(t, None);
            }
            if !ty {
                for f in e.accessors {
                    let getter = self.query.program.function(f).kind == FunctionKind::Getter;
                    if getter || self.top_getter(f).is_none() {
                        self.suggest_top_property(Element::Function(f), None);
                    }
                }
                for x in e.extensions {
                    if self.query.program.extension(x).name.is_some() {
                        self.suggest_extension(x, None);
                    }
                }
                for f in e.functions {
                    self.suggest_top_function(f, None);
                }
                for v in e.variables {
                    self.suggest_top_variable(v, None);
                }
            }
        }
    }

    /// `correspondingGetter` de um setter de topo.
    pub(super) fn top_getter(&self, f: FunctionElementId) -> Option<FunctionElementId> {
        let p = &self.query.program;
        let fe = p.function(f);
        let s = self.query.names.lookup(self.query.name(fe.name).trim_end_matches('='))?;
        match p.library(fe.library).declared.get(&s)?.getter {
            Some(Element::Function(g)) => Some(g),
            _ => None,
        }
    }

    /// Os imports da unidade definidora (`definingCompilationUnit.libraryImports`)
    /// na ordem, com o `dart:core` implícito por último quando nenhum o
    /// importa.
    pub(super) fn defining_imports(&self) -> Vec<dartforge_elements::model::Import> {
        let p = &self.query.program;
        let lib = p.library(self.library());
        let defining = lib.units.first().copied();
        let mut v: Vec<dartforge_elements::model::Import> = lib.imports.iter().filter(|i| Some(i.unit) == defining).cloned().collect();
        v.sort_by_key(|i| i.directive);
        if let Some(core) = p.core
            && !v.iter().any(|i| i.library == core)
            && self.library() != core
            && let Some(u) = defining
        {
            v.push(dartforge_elements::model::Import { unit: u, directive: usize::MAX, library: core, prefix: None, deferred: false, combinators: Vec::new() });
        }
        v
    }

    /// O espaço de nomes de um import (`importElement.namespace`): o
    /// exportado da biblioteca com os combinadores, por nome.
    pub(super) fn import_space(&self, i: &dartforge_elements::model::Import) -> Vec<(SymbolId, dartforge_elements::model::Binding)> {
        let p = &self.query.program;
        let mut v: Vec<(SymbolId, dartforge_elements::model::Binding)> = p.library(i.library).exported.iter().map(|(&s, &b)| (s, b)).collect();
        for c in &i.combinators {
            match c {
                ast::Combinator::Show(ns) => {
                    let shown: HashSet<SymbolId> = ns.iter().map(|n| n.sym).collect();
                    v.retain(|(s, _)| shown.contains(s));
                }
                ast::Combinator::Hide(ns) => {
                    let hidden: HashSet<SymbolId> = ns.iter().map(|n| n.sym).collect();
                    v.retain(|(s, _)| !hidden.contains(s));
                }
            }
        }
        // A ordem do `Namespace.definedNames`: a do `exportNamespace` da
        // biblioteca (as declarações dela na ordem da fonte, unidade por
        // unidade, depois as reexportadas, export por export).
        let order = export_order(&self.query.program, i.library, &mut HashSet::new());
        let position: HashMap<SymbolId, usize> = order.iter().enumerate().map(|(k, &s)| (s, k)).collect();
        v.sort_by_key(|(s, _)| (position.get(s).copied().unwrap_or(usize::MAX), self.query.name(*s).to_string()));
        v
    }

    /// `_addImportedDeclarations`.
    pub(super) fn imported_declarations(&mut self) {
        let ty = self.fl().ty;
        let core = self.query.program.core;
        for i in self.defining_imports() {
            let prefix = i.prefix.map(|s| self.query.name(s).to_string());
            if prefix.as_deref() == Some("_") {
                continue;
            }
            let imp = Import { prefix, not_imported: false };
            let space = self.import_space(&i);
            self.external_declarations(&space, &imp);
            if Some(i.library) == core && ty {
                let name = "Never";
                if let Some(s) = self.match_score(name) {
                    let before = self.collector.items.len();
                    let item = self.collector.push(group::WORD, kind::VARIABLE, name.into(), name.into(), None);
                    item.rel.fixed = Some(500);
                    self.mark(before, s, false);
                }
            }
        }
    }

    /// `_addExternalTopLevelDeclarations`.
    pub(super) fn external_declarations(&mut self, space: &[(SymbolId, dartforge_elements::model::Binding)], imp: &Import) {
        use dartforge_elements::model::ClassKind as K;
        let ty = self.fl().ty;
        for (_, b) in space {
            for el in [b.getter, b.setter].into_iter().flatten() {
                match el {
                    Element::Class(c) => match self.query.program.class(c).kind {
                        K::Class | K::MixinApplication => self.suggest_class(c, Some(imp)),
                        K::Enum => self.suggest_enum(c, Some(imp)),
                        K::ExtensionType => self.suggest_extension_type(c, Some(imp)),
                        K::Mixin => self.suggest_mixin(c, Some(imp)),
                    },
                    Element::Extension(x) => {
                        if !ty {
                            self.suggest_extension(x, Some(imp));
                        }
                    }
                    Element::Typedef(t) => self.suggest_typedef(t, Some(imp)),
                    Element::Function(f) => {
                        if !ty {
                            match self.query.program.function(f).kind {
                                FunctionKind::Function => self.suggest_top_function(f, Some(imp)),
                                _ => self.suggest_top_property(el, Some(imp)),
                            }
                        }
                    }
                    Element::Variable(_) => {
                        // O getter sintético (o setter sintético não entra).
                        if !ty && b.getter == Some(el) {
                            self.suggest_top_property(el, Some(imp));
                        }
                    }
                    Element::Prefix(..) => {}
                }
            }
        }
    }

    /// `addImportPrefixes`.
    pub(super) fn dh_import_prefixes(&mut self) {
        for i in self.defining_imports() {
            let Some(p) = i.prefix else { continue };
            let name = self.query.name(p).to_string();
            if !self.visible(&name, None) || name.is_empty() || self.wildcard(&name) {
                continue;
            }
            if let Some(s) = self.match_score(&name) {
                let before = self.collector.items.len();
                // O elemento é a biblioteca: `ElementKind.LIBRARY` → `File`/`Module`.
                self.collector.push(group::PREFIX, 17, name.clone(), name, None).rel.kind = Some(crate::relevance::Kind::Prefix);
                self.mark(before, s, false);
            }
        }
    }

    /// `addDeclarationsThroughImportPrefix`.
    pub(super) fn dh_by_prefix(&mut self, prefix: SymbolId) {
        let p = &self.query.program;
        let lib = p.library(self.library());
        let unit = self.unit;
        // Os imports com o prefixo na unidade do pedido (o fragmento que o
        // declara).
        let mut imports: Vec<dartforge_elements::model::Import> =
            lib.imports.iter().filter(|i| i.prefix == Some(prefix) && (i.unit == unit || lib.units.first() == Some(&i.unit))).cloned().collect();
        imports.sort_by_key(|i| (i.unit != unit, i.directive));
        for i in imports {
            let space = self.import_space(&i);
            let imp = Import { prefix: None, not_imported: false };
            self.external_declarations(&space, &imp);
            if i.deferred
                && let Some(s) = self.match_score("loadLibrary")
            {
                let before = self.collector.items.len();
                let item = self.collector.push(group::MEMBER, kind::FUNCTION, "loadLibrary".into(), "loadLibrary".into(), Some("() → Future<dynamic>".into()));
                item.call = Some(Call::Parameters(Vec::new()));
                item.rel.fixed = Some(200);
                self.mark(before, s, false);
            }
        }
    }

    /// `addFromLibrary` (nomes de `show`/`hide`): sem rastreador nem
    /// opções.
    pub(super) fn library_dh(&mut self, lib: LibraryId, excluded: &HashSet<String>) {
        let p = &self.query.program;
        let mut v: Vec<(SymbolId, dartforge_elements::model::Binding)> = p.library(lib).exported.iter().map(|(&s, &b)| (s, b)).collect();
        v.sort_by(|a, b| self.query.name(a.0).cmp(self.query.name(b.0)));
        for (s, b) in v {
            let name = self.query.name(s).to_string();
            for (el, setter) in [(b.getter, false), (b.setter, true)] {
                let Some(el) = el else { continue };
                let key = if setter { format!("{name}=") } else { name.clone() };
                if excluded.contains(&key) {
                    continue;
                }
                // `_addImportedElement`: o setter sintético não vira sugestão.
                if setter && matches!(el, Element::Variable(_)) {
                    continue;
                }
                let display = match el {
                    Element::Function(f) => without_equal(self.query.name(self.query.program.function(f).name)),
                    _ => name.clone(),
                };
                if let Some(sc) = self.match_score(&display) {
                    let ident = matches!(el, Element::Function(f) if self.query.program.function(f).kind == FunctionKind::Function) && self.executable_identifier();
                    self.item_element(el, None, sc, ident);
                }
            }
        }
    }

    // -- Os elementos de topo ---------------------------------------------------------

    /// `_suggestClass`.
    pub(super) fn suggest_class(&mut self, c: ClassId, imp: Option<&Import>) {
        let p = &self.query.program;
        let cl = p.class(c);
        if cl.kind == dartforge_elements::model::ClassKind::MixinApplication && cl.decl.is_none() {
            return;
        }
        let name = self.query.name(cl.name).to_string();
        if !self.visible(&name, imp) {
            return;
        }
        let f = self.fl();
        let lib = self.library();
        let cl = self.query.program.class(c);
        let m = cl.modifiers;
        let same = cl.library == lib;
        let extensible = same || (!m.interface && !m.final_ && !m.sealed);
        let implementable = same || (!m.base && !m.final_ && !m.sealed);
        let mixable = same || !self.library_feature(cl.library, Feature::ClassModifiers) || (m.mixin && !m.interface && !m.final_ && !m.sealed);
        if (f.extensible && !extensible) || (f.implementable && !implementable) || (f.mixable && !mixable) {
            return;
        }
        if !(f.constant && !f.object_pattern)
            && !f.exclude_type_names
            && let Some(s) = self.match_score(&name)
        {
            self.item_element(Element::Class(c), imp, s, false);
        }
        if !f.ty {
            self.suggest_static_fields(c, imp);
            let abstract_ = m.abstract_;
            self.suggest_constructors(c, imp, !abstract_);
        }
    }

    /// `_suggestEnum`.
    pub(super) fn suggest_enum(&mut self, c: ClassId, imp: Option<&Import>) {
        let name = self.query.name(self.query.program.class(c).name).to_string();
        if !self.visible(&name, imp) {
            return;
        }
        let f = self.fl();
        if f.extensible || f.implementable || f.mixable {
            return;
        }
        if let Some(s) = self.match_score(&name) {
            self.item_element(Element::Class(c), imp, s, false);
        }
        if !f.ty {
            self.suggest_static_fields(c, imp);
            self.suggest_constructors(c, imp, false);
        }
    }

    /// `_suggestExtension`.
    pub(super) fn suggest_extension(&mut self, x: ExtensionId, imp: Option<&Import>) {
        let Some(n) = self.query.program.extension(x).name else { return };
        let name = self.query.name(n).to_string();
        if !self.visible(&name, imp) {
            return;
        }
        let f = self.fl();
        if f.extensible || f.implementable || f.mixable {
            return;
        }
        if let Some(s) = self.match_score(&name) {
            self.item_element(Element::Extension(x), imp, s, false);
        }
        if !f.ty {
            let fields = self.query.program.extension(x).fields.clone();
            for v in fields {
                if self.visible_in(self.query.program.variable(v).library, self.query.name(self.query.program.variable(v).name)) {
                    self.suggest_static_field(v, imp, Some(name.clone()));
                }
            }
        }
    }

    /// `_suggestExtensionType`.
    pub(super) fn suggest_extension_type(&mut self, c: ClassId, imp: Option<&Import>) {
        let name = self.query.name(self.query.program.class(c).name).to_string();
        if !self.visible(&name, imp) {
            return;
        }
        let f = self.fl();
        if f.extensible || f.implementable || f.mixable {
            return;
        }
        if let Some(s) = self.match_score(&name) {
            self.item_element(Element::Class(c), imp, s, false);
        }
        if !f.ty {
            self.suggest_static_fields(c, imp);
            self.suggest_constructors(c, imp, true);
        }
    }

    /// `_suggestMixin`.
    pub(super) fn suggest_mixin(&mut self, c: ClassId, imp: Option<&Import>) {
        let name = self.query.name(self.query.program.class(c).name).to_string();
        if !self.visible(&name, imp) {
            return;
        }
        let f = self.fl();
        let cl = self.query.program.class(c);
        let implementable = cl.library == self.library() || !cl.modifiers.base;
        if f.extensible || (f.implementable && !implementable) {
            return;
        }
        if let Some(s) = self.match_score(&name) {
            self.item_element(Element::Class(c), imp, s, false);
        }
        if !f.ty {
            self.suggest_static_fields(c, imp);
        }
    }

    /// `_suggestTypeAlias`.
    pub(super) fn suggest_typedef(&mut self, t: TypedefId, imp: Option<&Import>) {
        let name = self.query.name(self.query.program.typedef(t).name).to_string();
        if !self.visible(&name, imp) {
            return;
        }
        if let Some(s) = self.match_score(&name) {
            self.item_element(Element::Typedef(t), imp, s, false);
        }
        if !self.fl().ty {
            self.alias_constructors(t, imp);
        }
    }

    /// `_addConstructorsForAliasedElement`.
    pub(super) fn alias_constructors(&mut self, t: TypedefId, imp: Option<&Import>) {
        use dartforge_elements::model::ClassKind as K;
        let target = self.query.outline.typedefs[t.0 as usize].target_type;
        let class_ = match self.query.table.get(target) {
            Type::Interface { class, .. } => Some(*class),
            Type::ExtensionType { decl, .. } => Some(*decl),
            _ => None,
        };
        let Some(c) = class_ else { return };
        let cl = self.query.program.class(c);
        match cl.kind {
            K::Class | K::MixinApplication => {
                let abstract_ = cl.modifiers.abstract_;
                self.suggest_constructors(c, imp, !abstract_);
            }
            K::ExtensionType | K::Mixin => self.suggest_constructors(c, imp, true),
            K::Enum => {}
        }
    }

    /// `_suggestTopLevelFunction`.
    pub(super) fn suggest_top_function(&mut self, f: FunctionElementId, imp: Option<&Import>) {
        let name = self.query.name(self.query.program.function(f).name).to_string();
        if !self.visible(&name, imp) {
            return;
        }
        let fl = self.fl();
        if fl.assignable || fl.constant || (fl.non_void && self.is_void(self.return_of(f))) || fl.ty {
            return;
        }
        if let Some(s) = self.match_score(&name) {
            let ident = fl.prefer_without_invocation;
            self.item_element(Element::Function(f), imp, s, ident);
        }
    }

    /// `_suggestTopLevelProperty`: um getter/setter declarado
    /// (`Element::Function`) ou o getter sintético de uma variável
    /// (`Element::Variable`).
    pub(super) fn suggest_top_property(&mut self, el: Element, imp: Option<&Import>) {
        let p = &self.query.program;
        let (name, getter, without_setter, constant, ret) = match el {
            Element::Variable(v) => {
                let ve = p.variable(v);
                let t = self.query.variable_type(v).unwrap_or(self.query.core.dynamic_);
                (self.query.name(ve.name).to_string(), true, ve.setter.is_none(), ve.const_, t)
            }
            Element::Function(f) => {
                let fe = p.function(f);
                let getter = fe.kind == FunctionKind::Getter;
                let ret = if getter { self.return_of(f) } else { self.query.core.void_ };
                let without_setter = getter && self.matching_setter(f).is_none();
                (without_equal(self.query.name(fe.name)), getter, without_setter, false, ret)
            }
            _ => return,
        };
        if !self.visible(&name, imp) {
            return;
        }
        let fl = self.fl();
        if (fl.assignable && getter && without_setter) || (fl.constant && !constant) || (fl.non_void && self.is_void(ret)) || fl.ty {
            return;
        }
        if let Some(s) = self.match_score(&name) {
            self.item_element(el, imp, s, false);
        }
    }

    /// `_suggestTopLevelVariable`.
    pub(super) fn suggest_top_variable(&mut self, v: VariableId, imp: Option<&Import>) {
        let ve = self.query.program.variable(v);
        let name = self.query.name(ve.name).to_string();
        if !self.visible(&name, imp) {
            return;
        }
        let fl = self.fl();
        let ve = self.query.program.variable(v);
        if (fl.assignable && ve.setter.is_none()) || (fl.constant && !ve.const_) || fl.ty {
            return;
        }
        if let Some(s) = self.match_score(&name) {
            self.item_element(Element::Variable(v), imp, s, false);
        }
    }

    // -- Campos estáticos e construtores ------------------------------------------------

    /// `_suggestStaticFields` de uma classe: as constantes de enum, o
    /// `values`, os campos declarados e os getters estáticos (campos
    /// sintéticos), na ordem de `fields`.
    pub(super) fn suggest_static_fields(&mut self, c: ClassId, imp: Option<&Import>) {
        let p = &self.query.program;
        let cl = p.class(c);
        let class_name_ = self.query.name(cl.name).to_string();
        let mut fields: Vec<VariableId> = cl.enum_constants.clone();
        fields.extend(cl.fields.iter().copied().filter(|v| !cl.enum_constants.contains(v)));
        let mut getters: Vec<FunctionElementId> = cl.static_members.values().copied().filter(|&f| p.function(f).kind == FunctionKind::Getter).collect();
        getters.sort();
        let values = if cl.kind == dartforge_elements::model::ClassKind::Enum {
            self.query.names.lookup("values").and_then(|s| cl.static_members.get(&s).copied())
        } else {
            None
        };
        for v in fields {
            let ve = self.query.program.variable(v);
            if self.visible_in(ve.library, self.query.name(ve.name)) {
                self.suggest_static_field(v, imp, Some(class_name_.clone()));
            }
        }
        if let Some(f) = values {
            self.suggest_static_getter(f, imp, &class_name_, true);
        }
        for g in getters {
            if Some(g) == values {
                continue;
            }
            let fe = self.query.program.function(g);
            if self.visible_in(fe.library, self.query.name(fe.name)) {
                self.suggest_static_getter(g, imp, &class_name_, false);
            }
        }
    }

    /// `_suggestStaticField` de um campo declarado.
    pub(super) fn suggest_static_field(&mut self, v: VariableId, imp: Option<&Import>, owner: Option<String>) {
        let ve = self.query.program.variable(v).clone();
        let fl = self.fl();
        if !ve.static_ || (fl.assignable && !(ve.final_ || ve.const_)) || (fl.constant && !ve.const_) {
            return;
        }
        let Some(context) = self.context_type else { return };
        let ty = self.query.variable_type(v).unwrap_or(self.query.core.dynamic_);
        if !self.subtype(ty, context) {
            return;
        }
        let name = self.query.name(ve.name).to_string();
        let owner = owner.unwrap_or_default();
        let enum_constant = ve.class.is_some_and(|c| self.query.program.class(c).enum_constants.contains(&v));
        let completion = format!("{owner}.{name}");
        let target = if enum_constant { completion.clone() } else { name.clone() };
        let Some(s) = self.match_score(&target) else { return };
        let before = self.collector.items.len();
        let detail = self.query.format(ty);
        let origin = self.query.origin(self.query.variable_start(v));
        let lsp_kind = if enum_constant { kind::ENUM_MEMBER } else { kind::FIELD };
        let item = self.collector.push(group::MEMBER, lsp_kind, completion.clone(), completion, Some(detail));
        item.origin = origin;
        item.rel.kind = Some(if enum_constant { crate::relevance::Kind::NoTable } else { crate::relevance::Kind::Field });
        item.rel.ty = Some(ty);
        self.prefix(before, imp);
        self.mark(before, s, false);
    }

    /// `_suggestStaticField` de um campo sintético (getter estático; o
    /// `values` de um enum): o getter sintético vira `FieldSuggestion` com o
    /// nome só; o declarado, `PropertyAccessSuggestion` com o nome do dono.
    pub(super) fn suggest_static_getter(&mut self, g: FunctionElementId, imp: Option<&Import>, owner: &str, synthetic: bool) {
        let fl = self.fl();
        if fl.constant || fl.assignable {
            return;
        }
        let Some(context) = self.context_type else { return };
        let ty = self.return_of(g);
        if !self.subtype(ty, context) {
            return;
        }
        let name = without_equal(self.query.name(self.query.program.function(g).name));
        let Some(s) = self.match_score(&name) else { return };
        let before = self.collector.items.len();
        self.item_member(g, ty, None, s, false);
        if !synthetic {
            for i in &mut self.collector.items[before..] {
                i.insert = format!("{owner}.{}", i.insert);
                i.label = i.insert.clone();
            }
            self.prefix(before, imp);
        }
    }

    /// `isSubtypeOf`.
    pub(super) fn subtype(&mut self, a: TypeId, b: TypeId) -> bool {
        let Query { table, outline, core, .. } = &mut *self.query;
        let mut env = dartforge_types::SubtypeEnv::new(table, &outline.hierarchy, core);
        dartforge_types::is_subtype(a, b, &mut env)
    }

    /// `_suggestConstructors`.
    pub(super) fn suggest_constructors(&mut self, c: ClassId, imp: Option<&Import>, non_factories: bool) {
        if self.fl().assignable {
            return;
        }
        let list = self.query.program.class(c).construtores();
        for (n, f) in list {
            let fe = self.query.program.function(f);
            if self.visible_in(fe.library, self.query.name(n)) && (non_factories || fe.factory) {
                self.suggest_constructor(f, false, imp, false);
            }
        }
    }

    /// `_suggestConstructor`.
    pub(super) fn suggest_constructor(&mut self, f: FunctionElementId, has_class_name: bool, imp: Option<&Import>, redirection: bool) {
        let fl = self.fl();
        if fl.assignable {
            return;
        }
        let fe = self.query.program.function(f).clone();
        let name_ctor = self.query.name(fe.name).to_string();
        if !self.visible_in(fe.library, &name_ctor) {
            return;
        }
        let Some(c) = fe.class else { return };
        let class = self.query.name(self.query.program.class(self.query.program.dono_da_classe(c)).name).to_string();
        // O rastreador: a classe fica conhecida (ou, não importada, tem de
        // estar visível).
        if imp.is_some_and(|i| i.not_imported) {
            if !self.visible(&class, imp) {
                return;
            }
        } else {
            self.visible(&class, imp);
        }
        let display = if name_ctor.is_empty() { class.clone() } else { format!("{class}.{name_ctor}") };
        let Some(s) = self.match_score(&display) else { return };
        let tear_off = fl.prefer_without_invocation || (fl.constant && !fe.const_);
        let as_new = fl.unnamed_as_new || tear_off;
        let mut completion = name_ctor.clone();
        if completion.is_empty() && as_new {
            completion = "new".into();
        }
        if !has_class_name {
            completion = if completion.is_empty() { class } else { format!("{class}.{completion}") };
        }
        if completion.is_empty() {
            return;
        }
        self.item_constructor(f, completion, !(tear_off || redirection), imp, s);
    }

    /// `addConstructorInvocations`.
    pub(super) fn dh_constructor_invocations(&mut self) {
        use dartforge_elements::model::ClassKind as K;
        let units = self.query.program.library(self.library()).units.clone();
        let own = Import { prefix: None, not_imported: false };
        for u in units {
            let e = self.unit_elements(u);
            for c in e.classes {
                let abstract_ = self.query.program.class(c).modifiers.abstract_;
                self.suggest_constructors(c, Some(&own), !abstract_);
            }
            for c in e.enums {
                self.suggest_constructors(c, Some(&own), true);
            }
            for c in e.extension_types {
                self.suggest_constructors(c, Some(&own), true);
            }
            for t in e.typedefs {
                self.alias_constructors(t, Some(&own));
            }
        }
        for i in self.defining_imports() {
            let imp = Import { prefix: i.prefix.map(|s| self.query.name(s).to_string()), not_imported: false };
            for (_, b) in self.import_space(&i) {
                match b.getter {
                    Some(Element::Class(c)) => match self.query.program.class(c).kind {
                        K::Class | K::MixinApplication => {
                            let abstract_ = self.query.program.class(c).modifiers.abstract_;
                            self.suggest_constructors(c, Some(&imp), !abstract_);
                        }
                        K::ExtensionType => self.suggest_constructors(c, Some(&imp), true),
                        _ => {}
                    },
                    Some(Element::Typedef(t)) => self.alias_constructors(t, Some(&imp)),
                    _ => {}
                }
            }
        }
        self.register_operation(Operation::Constructors);
    }

    /// `addConstructorNamesForElement`.
    pub(super) fn dh_element_constructor_names(&mut self, c: ClassId) {
        let list = self.query.program.class(c).construtores();
        for (_, f) in list {
            self.suggest_constructor(f, true, None, false);
        }
    }

    /// `addConstructorNamesForType`.
    pub(super) fn dh_type_constructor_names(&mut self, ty: TypeId, exclude: Option<String>) {
        let c = match self.query.table.get(ty) {
            Type::Interface { class, .. } => *class,
            Type::ExtensionType { decl, .. } => *decl,
            _ => return,
        };
        let constant = self.fl().constant;
        let list = self.query.program.class(c).construtores();
        for (n, f) in list {
            let name = self.query.name(n).to_string();
            if !name.is_empty() && Some(&name) != exclude.as_ref() && !(constant && !self.query.program.function(f).const_) {
                self.suggest_constructor(f, true, None, false);
            }
        }
    }

    /// `addStaticMembersOfElement`.
    pub(super) fn static_dh(&mut self, el: Element) {
        use dartforge_elements::model::ClassKind as K;
        let el = match el {
            Element::Typedef(t) => match self.query.table.get(self.query.outline.typedefs[t.0 as usize].target_type) {
                Type::Interface { class, .. } => Element::Class(*class),
                Type::ExtensionType { decl, .. } => Element::Class(*decl),
                _ => el,
            },
            _ => el,
        };
        match el {
            Element::Class(c) => {
                let cl = self.query.program.class(c);
                let class_ = cl.kind == K::Class || cl.kind == K::MixinApplication;
                let non_factories = class_ && !cl.modifiers.abstract_;
                let has_constructors = cl.kind != K::Mixin;
                self.static_members(Some(c), None, has_constructors, non_factories);
            }
            Element::Extension(x) => self.static_members(None, Some(x), false, false),
            _ => {}
        }
    }

    /// `_addStaticMembers`.
    pub(super) fn static_members(&mut self, c: Option<ClassId>, x: Option<ExtensionId>, constructors: bool, non_factories: bool) {
        let (accessors, fields, methods) = self.declared_members(c, x);
        let lib = self.library();
        let p = &self.query.program;
        let is_enum = c.is_some_and(|c| p.class(c).kind == dartforge_elements::model::ClassKind::Enum);
        for f in accessors {
            let fe = self.query.program.function(f);
            if fe.static_ && self.visible_in(fe.library, self.query.name(fe.name)) {
                self.suggest_property(f, None, false, None, None);
            }
        }
        for v in fields {
            let ve = self.query.program.variable(v);
            if !ve.static_ || !self.visible_in(ve.library, self.query.name(ve.name)) {
                continue;
            }
            let enum_constant = c.is_some_and(|c| self.query.program.class(c).enum_constants.contains(&v));
            if enum_constant {
                let e = self.query.name(self.query.program.class(c.unwrap_or(ClassId(0))).name).to_string();
                let name = self.query.name(ve.name).to_string();
                if let Some(s) = self.match_score(&format!("{e}.{name}")) {
                    // `includeEnumName: false` → `suggestField(…, 0.0)`.
                    let before = self.collector.items.len();
                    let ty = self.query.variable_type(v);
                    match self.query.program.variable(v).getter {
                        Some(g) => {
                            let t = ty.unwrap_or(self.query.core.dynamic_);
                            self.collector.function(&*self.query, group::MEMBER, g, t);
                        }
                        None => {
                            self.collector.push(group::MEMBER, kind::ENUM_MEMBER, name.clone(), name, None);
                        }
                    }
                    for i in &mut self.collector.items[before..] {
                        i.rel.kind = Some(crate::relevance::Kind::Field);
                        i.rel.distance = Some(0);
                    }
                    self.mark(before, s, false);
                }
            } else {
                self.suggest_field(v, None, None);
            }
        }
        // O `values` sintético do enum entra entre os campos.
        if is_enum
            && let Some(c) = c
            && let Some(s) = self.query.names.lookup("values")
            && let Some(&g) = self.query.program.class(c).static_members.get(&s)
        {
            self.suggest_property(g, None, false, None, None);
        }
        if !self.fl().assignable {
            if constructors && let Some(c) = c {
                let list = self.query.program.class(c).construtores();
                for (n, f) in list {
                    let fe = self.query.program.function(f);
                    if self.visible_in(fe.library, self.query.name(n)) && (non_factories || fe.factory) {
                        self.suggest_constructor(f, true, None, false);
                    }
                }
            }
            for m in methods {
                let fe = self.query.program.function(m);
                if fe.static_ && self.visible_in(fe.library, self.query.name(fe.name)) {
                    self.suggest_method(m, None, false, None, None);
                }
            }
        }
        let _ = lib;
    }

    /// `addPossibleRedirectionsInLibrary`.
    pub(super) fn dh_redirections(&mut self, redirecting: FunctionElementId) {
        use dartforge_elements::model::ClassKind as K;
        let Some(c) = self.query.program.function(redirecting).class else { return };
        let c = self.query.program.dono_da_classe(c);
        let type_c = self.type_this(c);
        let lib = self.library();
        let units = self.query.program.library(lib).units.clone();
        for u in units {
            let e = self.unit_elements(u);
            for k in e.classes {
                if self.query.program.class(k).kind != K::Class && self.query.program.class(k).kind != K::MixinApplication {
                    continue;
                }
                let type_k = self.type_this(k);
                if !self.subtype(type_k, type_c) {
                    continue;
                }
                let list = self.query.program.class(k).construtores();
                for (n, f) in list {
                    let fe = self.query.program.function(f);
                    let accessible = fe.library == lib || !self.query.name(n).starts_with('_');
                    if f != redirecting && accessible {
                        self.suggest_constructor(f, false, None, true);
                    }
                }
            }
        }
    }

    /// `addFieldsForInitializers`.
    pub(super) fn dh_fields_for_initializers(&mut self, ctor: usize, include: Option<VariableId>) {
        let Some(class) = self.parent(ctor).and_then(|c| self.node_class(c)) else { return };
        let p = &self.query.program;
        let mut skip: HashSet<VariableId> = HashSet::new();
        let field_by_name = |name_: &str| p.class(class).fields.iter().copied().find(|&v| self.query.name(p.variable(v).name) == name_);
        for i in self.children_of(ctor, &["ConstructorFieldInitializer"]) {
            if let Some(ast::Initializer::Field { name, .. }) = self.real_initializer(i)
                && let Some(v) = field_by_name(self.real_name(*name))
            {
                skip.insert(v);
            }
        }
        if let Some(k) = self.real_constructor(ctor) {
            for prm in k.parameters.iter() {
                if prm.this_
                    && let Some(n) = prm.name
                    && let Some(v) = field_by_name(self.real_name(n))
                {
                    skip.insert(v);
                }
            }
        }
        if let Some(v) = include {
            skip.remove(&v);
        }
        let fields = p.class(class).fields.clone();
        for v in fields {
            let ve = self.query.program.variable(v);
            if ve.static_ || skip.contains(&v) {
                continue;
            }
            let has_initializer = self.field_has_initializer(v);
            if !(ve.final_ || ve.const_) || !has_initializer {
                self.suggest_field(v, None, None);
            }
        }
    }

    /// `field.hasInitializer`.
    pub(super) fn field_has_initializer(&self, v: VariableId) -> bool {
        let p = &self.query.program;
        match p.variable(v).node {
            VariableRef::Field { unit, member, index } => match &p.unit(unit).ast.member(member).kind {
                ast::MemberKind::Field(l) => l.variables.get(index).is_some_and(|x| x.initializer.is_some()),
                _ => false,
            },
            _ => false,
        }
    }

    /// `addParametersFromSuperConstructor` (o nó é o `SuperFormalParameter`).
    pub(super) fn dh_super_parameters(&mut self, n: usize) {
        self.dh(Flags::default());
        let Some(prm) = self.real_parameter(n) else { return };
        if !prm.super_ {
            return;
        }
        let Some(ctor) = self.ancestor(n, &["ConstructorDeclaration"]) else { return };
        let Some(k) = self.real_constructor(ctor) else { return };
        let Some(class) = self.parent(ctor).and_then(|c| self.node_class(c)) else { return };
        let p = &self.query.program;
        let Some(sup) = p.class(class).supertype_class else { return };
        // `superConstructor`: o da invocação `super(...)`/`super.n(...)`, ou
        // o sem nome.
        let name_sup = k
            .initializers
            .iter()
            .find_map(|i| match i {
                ast::Initializer::Super { constructor, .. } => Some(constructor.map(|c| self.real_name(c).to_string()).unwrap_or_default()),
                _ => None,
            })
            .unwrap_or_default();
        let Some(f_sup) = p.class(p.dono_da_classe(sup)).construtores().into_iter().find(|(s, _)| self.query.name(*s) == name_sup).map(|(_, f)| f) else {
            return;
        };
        let parameters_sup = self.query.outline.functions[f_sup.0 as usize].parameters.clone();
        if prm.kind == ast::ParameterKind::Named {
            let mut specified: HashSet<String> = k.parameters.iter().filter_map(|x| x.name.map(|n| self.real_name(n).to_string())).collect();
            for i in k.initializers.iter() {
                if let ast::Initializer::Super { arguments, .. } = i {
                    for a in arguments.args.iter() {
                        if let Some(n) = a.name {
                            specified.insert(self.real_name(n).to_string());
                        }
                    }
                }
            }
            for ps in parameters_sup.iter() {
                if ps.kind == ast::ParameterKind::Named
                    && let Some(name) = ps.externo.or(ps.name)
                {
                    let text = self.query.name(name).to_string();
                    if !specified.contains(&text) {
                        self.super_parameter_item(&text, ps.ty);
                    }
                }
            }
        } else {
            let positional_here: Vec<&ast::Parameter> = k.parameters.iter().filter(|x| x.super_ && x.kind != ast::ParameterKind::Named).collect();
            let index = positional_here.iter().position(|x| std::ptr::eq(*x, prm));
            let positional_sup: Vec<_> = parameters_sup.iter().filter(|x| x.kind != ast::ParameterKind::Named).collect();
            if let Some(i) = index
                && let Some(ps) = positional_sup.get(i)
                && let Some(name) = ps.name
            {
                let text = self.query.name(name).to_string();
                self.super_parameter_item(&text, ps.ty);
            }
        }
    }

    /// `_suggestSuperParameter` (`Relevance.superFormalParameter`).
    pub(super) fn super_parameter_item(&mut self, name: &str, ty: TypeId) {
        if let Some(s) = self.match_score(name) {
            let before = self.collector.items.len();
            let detail = self.query.format(ty);
            let item = self.collector.push(group::LOCAL, kind::VARIABLE, name.to_string(), name.to_string(), Some(detail));
            item.rel.fixed = Some(1000);
            self.mark(before, s, false);
        }
    }
}


/// O `Name` do analyzer: o nome, e a biblioteca quando é privado.
type Key = (SymbolId, Option<dartforge_elements::model::LibraryId>);

/// Um `Map<Name, ExecutableElement>` do Dart: a ordem é a da primeira
/// inserção, e substituir mantém a posição.
#[derive(Default, Clone)]
struct SortedMap {
    order: Vec<Key>,
    values: HashMap<Key, FunctionElementId>,
}

impl SortedMap {
    fn insert(&mut self, k: Key, v: FunctionElementId) {
        if self.values.insert(k, v).is_none() {
            self.order.push(k);
        }
    }

    fn contains(&self, k: &Key) -> bool {
        self.values.contains_key(k)
    }

    fn entries(&self) -> impl Iterator<Item = (Key, FunctionElementId)> + '_ {
        self.order.iter().map(|k| (*k, self.values[k]))
    }

    /// O `_addCandidates` seguido do `_findMostSpecificFromNamedCandidates`
    /// quando há um candidato só: os nomes ausentes entram no fim, e o
    /// primeiro candidato de cada nome fica.
    fn append_missing(&mut self, other: &SortedMap) {
        for (k, v) in other.entries() {
            if !self.contains(&k) {
                self.insert(k, v);
            }
        }
    }
}

/// A `Interface` do `InheritanceManager3`, só com o que o completar lê.
#[derive(Default)]
struct Interface {
    map: SortedMap,
    implemented: SortedMap,
    implemented_super: SortedMap,
    overridden: SortedMap,
    forwarded: HashSet<Key>,
}

fn key(q: &Query, f: FunctionElementId) -> Key {
    let p = &q.program;
    let fe = p.function(f);
    let private = q.name(fe.name).starts_with('_');
    (fe.name, private.then_some(fe.library))
}

/// `_getTypeMembers` (com `include_abstract`) ou `_addImplemented` (sem): os
/// métodos e depois os acessores, na ordem de declaração.
fn declared(q: &Query, c: ClassId, include_abstract: bool) -> Vec<(Key, FunctionElementId)> {
    let p = &q.program;
    let mut v: Vec<FunctionElementId> = p
        .class(c)
        .instance_members
        .values()
        .copied()
        .filter(|&f| !p.function(f).static_ && (include_abstract || !p.function(f).abstract_))
        .collect();
    v.sort_by_key(|&f| (!matches!(p.function(f).kind, FunctionKind::Function | FunctionKind::Operator), f));
    v.into_iter().map(|f| (key(q, f), f)).collect()
}

fn of_object(p: &dartforge_elements::model::Program, object: Option<ClassId>, f: FunctionElementId) -> bool {
    object.is_some() && p.function(f).class.map(|c| p.dono_da_classe(c)) == object
}

/// `getInterface`: `_getInterfaceClass`, `_getInterfaceMixin` e
/// `_getInterfaceExtensionType`.
fn interface_of(
    q: &Query,
    object: Option<ClassId>,
    c: ClassId,
    cache: &mut HashMap<ClassId, std::rc::Rc<Interface>>,
    processing: &mut HashSet<ClassId>,
) -> std::rc::Rc<Interface> {
    if let Some(i) = cache.get(&c) {
        return i.clone();
    }
    if !processing.insert(c) {
        return std::rc::Rc::new(Interface::default());
    }
    let p = &q.program;
    let cl = p.class(c);
    let sub = |x: ClassId, cache: &mut HashMap<ClassId, std::rc::Rc<Interface>>, processing: &mut HashSet<ClassId>| {
        interface_of(q, object, p.dono_da_classe(x), cache, processing)
    };
    let i = match cl.kind {
        ClassKind::Mixin => {
            // `superclassConstraints` (`Object` quando não há `on`).
            let constraints: Vec<ClassId> = if cl.on_classes.is_empty() { object.into_iter().collect() } else { cl.on_classes.clone() };
            let mut super_candidates = SortedMap::default();
            for r in constraints {
                let i = sub(r, cache, processing);
                super_candidates.append_missing(&i.map);
            }
            let mut candidates = super_candidates.clone();
            for &x in &cl.interface_classes {
                let i = sub(x, cache, processing);
                candidates.append_missing(&i.map);
            }
            let mut map = SortedMap::default();
            for (k, f) in declared(q, c, true) {
                map.insert(k, f);
            }
            map.append_missing(&candidates);
            let mut implemented = SortedMap::default();
            for (k, f) in declared(q, c, false) {
                implemented.insert(k, f);
            }
            Interface { map, implemented, implemented_super: super_candidates, overridden: candidates, forwarded: HashSet::new() }
        }
        ClassKind::ExtensionType => {
            let mut declared_map = SortedMap::default();
            for (k, f) in declared(q, c, false) {
                declared_map.insert(k, f);
            }
            // Os nomes precluídos: os declarados, o setter de um método e o
            // método de um setter declarados.
            let name = |k: &Key| q.name(k.0).to_string();
            let mut precluded: HashSet<String> = HashSet::new();
            for (k, f) in declared_map.entries() {
                let n = name(&k);
                precluded.insert(n.clone());
                match p.function(f).kind {
                    FunctionKind::Function | FunctionKind::Operator => {
                        precluded.insert(format!("{n}="));
                    }
                    FunctionKind::Setter => {
                        precluded.insert(n.trim_end_matches('=').to_string());
                    }
                    FunctionKind::ImplicitAccessor if n.ends_with('=') => {
                        precluded.insert(n.trim_end_matches('=').to_string());
                    }
                    _ => {}
                }
            }
            let interfaces: Vec<ClassId> = if cl.interface_classes.is_empty() { object.into_iter().collect() } else { cl.interface_classes.clone() };
            let mut of_extension = SortedMap::default();
            let mut non_extension = SortedMap::default();
            for x in interfaces {
                let i = sub(x, cache, processing);
                for (k, f) in i.map.entries() {
                    let is_extension_type_member = p.function(f).class.is_some_and(|d| p.class(d).kind == ClassKind::ExtensionType);
                    let target = if is_extension_type_member { &mut of_extension } else { &mut non_extension };
                    if !target.contains(&k) {
                        target.insert(k, f);
                    }
                }
            }
            let mut map = declared_map.clone();
            let mut redeclared = SortedMap::default();
            for (k, f) in of_extension.entries() {
                redeclared.insert(k, f);
                if precluded.contains(&name(&k)) || non_extension.contains(&k) {
                    continue;
                }
                map.insert(k, f);
            }
            for (k, f) in non_extension.entries() {
                if !redeclared.contains(&k) {
                    redeclared.insert(k, f);
                }
                if precluded.contains(&name(&k)) || of_extension.contains(&k) {
                    continue;
                }
                map.insert(k, f);
            }
            Interface { implemented: map.clone(), map, implemented_super: SortedMap::default(), overridden: redeclared, forwarded: HashSet::new() }
        }
        ClassKind::Class | ClassKind::Enum | ClassKind::MixinApplication => {
            let mut candidates = SortedMap::default();
            let mut implemented = SortedMap::default();
            let mut super_forwarded = HashSet::new();
            if let Some(s) = cl.supertype_class {
                let i = sub(s, cache, processing);
                candidates.append_missing(&i.map);
                implemented = i.implemented.clone();
                super_forwarded = i.forwarded.clone();
            }
            let mut implemented_super = implemented.clone();
            for &m in &cl.mixin_classes {
                let owner_m = p.dono_da_classe(m);
                let im = sub(m, cache, processing);
                for (k, f) in im.map.entries() {
                    // O membro declarado no próprio mixin substitui o da
                    // superclasse; o que o mixin herda do `on` fica com o
                    // mais específico, que é o já presente.
                    if !candidates.contains(&k) || p.function(f).class.map(|d| p.dono_da_classe(d)) == Some(owner_m) {
                        candidates.insert(k, f);
                    }
                }
                for (k, f) in im.implemented.entries() {
                    if p.function(f).abstract_ || of_object(p, object, f) {
                        continue;
                    }
                    implemented.insert(k, f);
                }
                implemented_super = implemented.clone();
            }
            for &x in &cl.interface_classes {
                let i = sub(x, cache, processing);
                candidates.append_missing(&i.map);
            }
            for (k, f) in declared(q, c, false) {
                implemented.insert(k, f);
            }
            let mut map = SortedMap::default();
            for (k, f) in declared(q, c, true) {
                map.insert(k, f);
            }
            map.append_missing(&candidates);
            // Os encaminhadores de `noSuchMethod`: numa classe abstrata,
            // os da superclasse; numa concreta com `noSuchMethod` que não é
            // o de `Object`, os nomes da interface sem implementação.
            let is_abstract = cl.kind == ClassKind::Class && cl.modifiers.abstract_ || cl.decl.is_none();
            let mut forwarded = HashSet::new();
            if is_abstract {
                forwarded = super_forwarded;
            } else {
                let nsm = implemented.entries().find(|(k, _)| q.name(k.0) == "noSuchMethod").map(|(_, f)| f);
                if let Some(n) = nsm
                    && !of_object(p, object, n)
                {
                    for (k, f) in map.entries() {
                        if !implemented.contains(&k) || super_forwarded.contains(&k) {
                            implemented.insert(k, f);
                            forwarded.insert(k);
                        }
                    }
                }
            }
            Interface { map, implemented, implemented_super, overridden: candidates, forwarded }
        }
    };
    processing.remove(&c);
    let i = std::rc::Rc::new(i);
    cache.insert(c, i.clone());
    i
}

/// O lugar de um elemento de topo na biblioteca `lib`: a unidade (na ordem
/// das unidades dela), o início da declaração e a posição na lista de
/// variáveis.
fn place_in_library(p: &dartforge_elements::model::Program, lib: dartforge_elements::model::LibraryId, el: Element) -> Option<(usize, usize, usize)> {
    use dartforge_elements::model::{FunctionRef, VariableRef};
    let units = &p.library(lib).units;
    let of_unit = |u: UnitId| units.iter().position(|&x| x == u);
    let of_variable = |v: VariableId| match p.variable(v).node {
        VariableRef::TopLevel { unit, decl, index } => Some((of_unit(unit)?, p.unit(unit).ast.decl(decl).span.start, index)),
        _ => None,
    };
    match el {
        Element::Class(c) => {
            let d = p.class(c).decl?;
            Some((of_unit(d.unit)?, p.unit(d.unit).ast.decl(d.decl).span.start, 0))
        }
        Element::Typedef(x) => {
            let d = p.typedef(x).decl;
            Some((of_unit(d.unit)?, p.unit(d.unit).ast.decl(d.decl).span.start, 0))
        }
        Element::Extension(x) => {
            let d = p.extension(x).decl;
            Some((of_unit(d.unit)?, p.unit(d.unit).ast.decl(d.decl).span.start, 0))
        }
        Element::Function(f) => {
            let fe = p.function(f);
            if let Some(v) = fe.variable {
                return of_variable(v);
            }
            match fe.node {
                FunctionRef::Function { unit, function } => Some((of_unit(unit)?, p.unit(unit).ast.functions[function.0 as usize].span.start, 0)),
                _ => None,
            }
        }
        Element::Variable(v) => of_variable(v),
        Element::Prefix(..) => None,
    }
}

fn element_library(p: &dartforge_elements::model::Program, el: Element) -> Option<dartforge_elements::model::LibraryId> {
    Some(match el {
        Element::Class(c) => p.class(c).library,
        Element::Typedef(x) => p.typedef(x).library,
        Element::Extension(x) => p.extension(x).library,
        Element::Function(f) => p.function(f).library,
        Element::Variable(v) => p.variable(v).library,
        Element::Prefix(..) => return None,
    })
}

/// Os nomes do `exportNamespace` de `lib` na ordem do analyzer: os
/// declarados nela pela fonte, depois os de cada `export` na ordem das
/// diretivas (com os combinadores), sem repetir.
fn export_order(
    p: &dartforge_elements::model::Program,
    lib: dartforge_elements::model::LibraryId,
    seen: &mut HashSet<dartforge_elements::model::LibraryId>,
) -> Vec<SymbolId> {
    if !seen.insert(lib) {
        return Vec::new();
    }
    let l = p.library(lib);
    let mut locals: Vec<((usize, usize, usize), SymbolId)> = l
        .exported
        .iter()
        .filter_map(|(&s, b)| {
            let el = b.getter.or(b.setter)?;
            (element_library(p, el) == Some(lib)).then_some(())?;
            Some((place_in_library(p, lib, el)?, s))
        })
        .collect();
    locals.sort();
    let mut order: Vec<SymbolId> = locals.into_iter().map(|(_, s)| s).collect();
    let mut present: HashSet<SymbolId> = order.iter().copied().collect();
    let mut exports = l.exports.clone();
    let unit_position = |u: UnitId| l.units.iter().position(|&x| x == u).unwrap_or(usize::MAX);
    exports.sort_by_key(|e| (unit_position(e.unit), e.directive));
    for e in exports {
        for s in export_order(p, e.library, seen) {
            let passes = e.combinators.iter().all(|c| match c {
                ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == s),
                ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == s),
            });
            if passes && l.exported.contains_key(&s) && present.insert(s) {
                order.push(s);
            }
        }
    }
    seen.remove(&lib);
    order
}
