//! `Change to` e os `Create …` do Dart 3.6.2 (docs/LSP-ESPECIFICACAO.md
//! §13.7.4.2 e §13.7.4.3): `change_to.dart`, `create_class.dart`,
//! `create_mixin.dart`, `create_field.dart`, `create_getter.dart`,
//! `create_setter.dart`, `create_local_variable.dart`,
//! `create_parameter.dart`, `create_method_or_function.dart`,
//! `create_function.dart`, `create_method.dart` e
//! `create_extension_member.dart`, sobre a árvore do analyzer.
//!
//! Os `undefined_*` não são publicados por este servidor: cada nome sem
//! resolução das linhas pedidas recebe o código que o analyzer daria
//! (`UNDEFINED_IDENTIFIER`, `UNDEFINED_FUNCTION`, `UNDEFINED_CLASS`,
//! `UNDEFINED_METHOD`, `UNDEFINED_GETTER`, `UNDEFINED_SETTER`, os de
//! extensão) e os produtores desse código, na ordem do `fix_internal.dart`.
//! Os códigos publicados (`cast_to_non_type`, `not_a_type`,
//! `extends_non_class`…) usam os mesmos produtores pelo diagnóstico.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Edit;
use crate::actions::CodeAction;
use crate::arvore_analyzer::Marca;
use crate::write_type::Writer;
use crate::insert::Filter;
use crate::project::Project;
use crate::refactorings::Context;
use crate::refactorings_exec::{Change, Text, ONE_INDENT};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, UnitId};
use dartforge_frontend::ast;
use dartforge_types::{MemberRef, Resolved, Type, TypeId};
use std::collections::BTreeSet;

/// O código de erro que escolhe os produtores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Code {
    Identifier,
    Function,
    Class,
    Method,
    Getter,
    Setter,
    ExtensionGetter,
    ExtensionMethod,
    ExtensionSetter,
    /// `CAST_TO_NON_TYPE`, `NOT_A_TYPE`.
    NotType,
    /// `EXTENDS_NON_CLASS`, `IMPLEMENTS_NON_CLASS`, `MIXIN_OF_NON_CLASS`,
    /// `NEW_WITH_NON_TYPE`.
    NonClass,
    /// `INVALID_ANNOTATION`, `UNDEFINED_ANNOTATION`.
    Annotation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Producer {
    SwapClass,
    SwapAnnotation,
    SwapFunction,
    SwapGetterOrSetter,
    SwapMethod,
    CreateClass,
    CreateMixin,
    CreateField,
    CreateGetter,
    CreateSetter,
    CreateLocal,
    CreateParameter,
    CreateMethodOrFunction,
    CreateFunction,
    CreateMethod,
    CreateExtensionGetter,
    CreateExtensionMethod,
    CreateExtensionSetter,
}

/// Os produtores de cada código (`nonLintProducerMap`).
fn producers(c: Code) -> &'static [Producer] {
    use Producer::*;
    match c {
        Code::Identifier => &[SwapGetterOrSetter, CreateClass, CreateField, CreateGetter, CreateLocal, CreateParameter, CreateMethodOrFunction, CreateMixin, CreateSetter],
        Code::Function => &[SwapFunction, CreateClass, CreateFunction],
        Code::Class => &[SwapClass, CreateClass, CreateMixin],
        Code::Method => &[SwapMethod, CreateClass, CreateExtensionMethod, CreateFunction, CreateMethod],
        Code::Getter => &[SwapGetterOrSetter, CreateClass, CreateExtensionGetter, CreateField, CreateGetter, CreateLocal, CreateMethodOrFunction, CreateMixin],
        Code::Setter => &[SwapGetterOrSetter, CreateExtensionSetter, CreateField, CreateSetter],
        Code::ExtensionGetter => &[SwapGetterOrSetter, CreateGetter],
        Code::ExtensionMethod => &[SwapMethod, CreateMethod],
        Code::ExtensionSetter => &[SwapGetterOrSetter, CreateSetter],
        Code::NotType => &[SwapClass, CreateClass, CreateMixin],
        Code::NonClass => &[SwapClass, CreateClass],
        Code::Annotation => &[SwapAnnotation, CreateClass],
    }
}

/// Distância de Levenshtein com limiar (`levenshtein.dart`): `1 << 20`
/// quando passa do limiar ou a diferença de tamanhos passa dele.
fn levenshtein(s: &str, t: &str, threshold: usize) -> usize {
    const INFINITE: usize = 1 << 20;
    let s: Vec<char> = s.chars().collect();
    let t: Vec<char> = t.chars().collect();
    if s.len().abs_diff(t.len()) > threshold {
        return INFINITE;
    }
    let mut previous: Vec<usize> = (0..=t.len()).collect();
    for (i, a) in s.iter().enumerate() {
        let mut current = vec![i + 1; t.len() + 1];
        let mut min = current[0];
        for (j, b) in t.iter().enumerate() {
            let cost = usize::from(a != b);
            current[j + 1] = (previous[j] + cost).min(previous[j + 1] + 1).min(current[j] + 1);
            min = min.min(current[j + 1]);
        }
        if min > threshold {
            return INFINITE;
        }
        previous = current;
    }
    let d = previous[t.len()];
    if d > threshold { INFINITE } else { d }
}

/// `_ClosestElementFinder`: o primeiro candidato com distância menor que a
/// melhor (começando em 3).
struct Closest {
    target: String,
    distance: usize,
    name: Option<String>,
}

impl Closest {
    fn new(target: &str) -> Closest {
        Closest { target: target.to_string(), distance: 3, name: None }
    }

    /// `name` (o comparado; `x=` num setter) e o proposto.
    fn see(&mut self, name: &str, proposed: &str) {
        let d = levenshtein(name, &self.target, self.distance);
        if d < self.distance {
            self.distance = d;
            self.name = Some(proposed.to_string());
        }
    }
}

/// O `inGetterContext` de um `SimpleIdentifier`.
fn in_read_context(cx: &Context<'_>, n: usize) -> bool {
    let Some(parent0) = cx.parent(n) else { return true };
    let (mut parent, mut target) = (parent0, n);
    match cx.kind(parent0) {
        "PrefixedIdentifier" | "PropertyAccess" => {
            if cx.children(parent0).first() == Some(&n) {
                return true;
            }
            let Some(p) = cx.parent(parent0) else { return true };
            parent = p;
            target = parent0;
        }
        _ => {}
    }
    match cx.kind(parent) {
        "Label" => false,
        "AssignmentExpression" => !(cx.children(parent).first() == Some(&target) && assignment_operator(cx, parent) == "="),
        "ForEachPartsWithIdentifier" => cx.children(parent).first() != Some(&target),
        "FieldFormalParameter" => false,
        _ => true,
    }
}

/// O `inSetterContext` de um `SimpleIdentifier`.
fn in_write_context(cx: &Context<'_>, n: usize) -> bool {
    let Some(parent0) = cx.parent(n) else { return false };
    let (mut parent, mut target) = (parent0, n);
    match cx.kind(parent0) {
        "PrefixedIdentifier" | "PropertyAccess" => {
            if cx.children(parent0).first() == Some(&n) {
                return false;
            }
            let Some(p) = cx.parent(parent0) else { return false };
            parent = p;
            target = parent0;
        }
        _ => {}
    }
    match cx.kind(parent) {
        "PrefixExpression" | "PostfixExpression" => {
            let t = cx.node_text(parent);
            t.starts_with("++") || t.starts_with("--") || t.ends_with("++") || t.ends_with("--")
        }
        "AssignmentExpression" => cx.children(parent).first() == Some(&target),
        "ForEachPartsWithIdentifier" => cx.children(parent).first() == Some(&target),
        _ => false,
    }
}

/// O operador de uma `AssignmentExpression`.
fn assignment_operator<'c>(cx: &'c Context<'_>, a: usize) -> &'c str {
    let Some(&left) = cx.children(a).first() else { return "" };
    match cx.next_token(cx.tree.nos[left].fim) {
        Some(t) => &cx.source[t.start..t.end],
        None => "",
    }
}

/// `climbPropertyAccess`.
fn climb_access(cx: &Context<'_>, mut n: usize) -> usize {
    while let Some(p) = cx.parent(n) {
        let e = cx.kind(p);
        if (e == "PrefixedIdentifier" || e == "PropertyAccess") && cx.children(p).last() == Some(&n) && cx.children(p).len() > 1 {
            n = p;
        } else {
            break;
        }
    }
    n
}

/// O alvo de um nome qualificado: o prefixo do `PrefixedIdentifier` ou o
/// alvo do `PropertyAccess` (`getQualifiedPropertyTarget`).
fn qualified_target(cx: &Context<'_>, n: usize) -> Option<usize> {
    let p = cx.parent(n)?;
    match cx.kind(p) {
        "PrefixedIdentifier" | "PropertyAccess" if cx.children(p).last() == Some(&n) && cx.children(p).len() > 1 => cx.children(p).first().copied(),
        _ => None,
    }
}

impl Context<'_> {
    /// O elemento resolvido de um nó identificador.
    fn resolved(&self, n: usize) -> Option<&Resolved> {
        let x = match self.tree.nos[n].marca {
            Marca::Expr(x) => x,
            _ => self.node_expr(n)?,
        };
        self.bodies.get_resolved(x)
    }

    /// A classe (ou mixin, enum, tipo de extensão) que o identificador nomeia.
    fn named_class(&self, n: usize) -> Option<ClassId> {
        match self.resolved(n) {
            Some(Resolved::Element(Element::Class(c))) => Some(*c),
            _ => None,
        }
    }

    /// A extensão que o identificador nomeia, ou a do `ExtensionOverride`
    /// (`E(x)` / `p.E(x)` como invocação cujo nome resolve para a extensão).
    fn target_extension(&self, target: usize) -> Option<ExtensionId> {
        if let Some(Resolved::Element(Element::Extension(x))) = self.resolved(target) {
            return Some(*x);
        }
        if matches!(self.kind(target), "MethodInvocation" | "FunctionExpressionInvocation") {
            for &f in self.children(target) {
                if self.kind(f) == "SimpleIdentifier"
                    && let Some(Resolved::Element(Element::Extension(x))) = self.resolved(f)
                {
                    return Some(*x);
                }
            }
        }
        None
    }

    /// O alvo é um `ExtensionOverride` (`E(x)`), não o nome da extensão.
    fn is_extension_override(&self, target: usize) -> bool {
        matches!(self.kind(target), "MethodInvocation" | "FunctionExpressionInvocation") && self.target_extension(target).is_some()
    }

    /// `getTargetInterfaceElement2(target)`.
    fn target_interface(&self, target: usize) -> Option<ClassId> {
        // O nome de uma classe como receptor (`A.m()`): o analyzer não dá
        // tipo estático ao literal de tipo e cai no elemento.
        if let Some(c) = self.named_class(target) {
            return Some(c);
        }
        match self.p.query.table.get(self.node_type(target)?) {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(*class),
            _ => None,
        }
    }

    /// `enclosingInterfaceElement2`: a `ClassDeclaration` ou
    /// `MixinDeclaration` envolvente.
    fn enclosing_interface(&self, n: usize) -> Option<(ClassId, usize)> {
        let mut current = Some(n);
        while let Some(k) = current {
            if matches!(self.kind(k), "ClassDeclaration" | "MixinDeclaration")
                && let Marca::Decl(d) = self.tree.nos[k].marca
            {
                return self.declaration_class(self.unit, d).map(|c| (c, k));
            }
            current = self.parent(k);
        }
        None
    }

    /// `enclosingExtensionElement2`.
    fn enclosing_extension(&self, n: usize) -> Option<(ExtensionId, usize)> {
        let prog = self.p.program();
        let mut current = Some(n);
        while let Some(k) = current {
            if self.kind(k) == "ExtensionDeclaration"
                && let Marca::Decl(d) = self.tree.nos[k].marca
            {
                let x = (0..prog.extensions.len()).map(|i| ExtensionId(i as u32)).find(|&x| prog.extension(x).decl.unit == self.unit && prog.extension(x).decl.decl == d)?;
                return Some((x, k));
            }
            current = self.parent(k);
        }
        None
    }

    /// `inStaticContext`.
    fn in_static_context(&self, n: usize) -> bool {
        let mut current = Some(n);
        while let Some(k) = current {
            match self.kind(k) {
                "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer" => return true,
                "FieldDeclaration" => {
                    let words = self.initial_tokens(k);
                    let is_static = words.iter().any(|s| s == "static");
                    let late = words.iter().any(|s| s == "late");
                    return is_static || !late;
                }
                "MethodDeclaration" => return self.initial_tokens(k).iter().any(|s| s == "static"),
                _ => {}
            }
            current = self.parent(k);
        }
        false
    }

    /// As palavras do começo de uma declaração (depois de doc e anotações).
    pub(crate) fn initial_tokens_pub(&self, n: usize) -> Vec<String> {
        self.initial_tokens(n)
    }

    fn initial_tokens(&self, n: usize) -> Vec<String> {
        let mut v = Vec::new();
        let Some(mut t) = self.first_token_after_comment_and_metadata(n) else { return v };
        loop {
            let s = &self.source[t.start..t.end];
            if !matches!(s, "static" | "late" | "final" | "const" | "var" | "abstract" | "external" | "covariant") {
                break;
            }
            v.push(s.to_string());
            match self.next_token(t.end) {
                Some(p) => t = p,
                None => break,
            }
        }
        v
    }

    /// O parâmetro (tipo) que recebe o argumento `arg` (`staticParameterElement`
    /// / `correspondingParameter`).
    fn matching_parameter(&self, arg: usize) -> Option<TypeId> {
        let (value, name) = match self.parent(arg) {
            Some(p) if self.kind(p) == "NamedExpression" => (p, self.children(p).first().and_then(|&r| self.children(r).first()).map(|&i| self.node_text(i).to_string())),
            _ => (arg, None),
        };
        let list = self.parent(value).filter(|&l| self.kind(l) == "ArgumentList")?;
        let call = self.parent(list)?;
        let table = &self.p.query.table;
        let type_f = match self.kind(call) {
            "MethodInvocation" | "FunctionExpressionInvocation" => {
                let target = match self.kind(call) {
                    "MethodInvocation" => self.method_name(call)?,
                    _ => *self.children(call).first()?,
                };
                match self.resolved(call) {
                    Some(Resolved::Constructor(f)) => Some(self.p.query.outline.functions.get(f.0 as usize)?.signature),
                    _ => self.node_type(target),
                }
            }
            "InstanceCreationExpression" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" => match self.resolved(call) {
                Some(Resolved::Constructor(f)) => Some(self.p.query.outline.functions.get(f.0 as usize)?.signature),
                _ => None,
            },
            _ => None,
        }?;
        let Type::Function { positional, optional, named, .. } = table.get(type_f) else { return None };
        match name {
            Some(n) => named.iter().find(|(s, _, _)| self.p.name(*s) == n).map(|(_, t, _)| *t),
            None => {
                let position = self.children(list).iter().take_while(|&&a| a != value).filter(|&&a| self.kind(a) != "NamedExpression").count();
                positional.iter().chain(optional.iter()).nth(position).copied()
            }
        }
    }

    /// O tipo de retorno do executável envolvente.
    fn enclosing_return(&self, n: usize) -> Option<TypeId> {
        let mut current = self.parent(n);
        while let Some(k) = current {
            match self.kind(k) {
                "FunctionExpression" => {
                    if let Marca::Funcao(fid) = self.tree.nos[k].marca {
                        if let Some(f) = self.p.node_function(self.unit, fid) {
                            return self.p.query.outline.functions.get(f.0 as usize).map(|d| d.return_type);
                        }
                        let name = self.ast.function(fid).name?;
                        return match self.bodies.tipo_local(name.span.start).map(|t| self.p.query.table.get(t)) {
                            Some(Type::Function { ret, .. }) => Some(*ret),
                            _ => None,
                        };
                    }
                    // A closure: o tipo da expressão de função.
                    let t = self.node_type(k)?;
                    return match self.p.query.table.get(t) {
                        Type::Function { ret, .. } => Some(*ret),
                        _ => None,
                    };
                }
                "MethodDeclaration" | "FunctionDeclaration" => {
                    let Marca::Funcao(fid) = self.tree.nos[k].marca else { return None };
                    let f = self.p.node_function(self.unit, fid)?;
                    return self.p.query.outline.functions.get(f.0 as usize).map(|d| d.return_type);
                }
                "ConstructorDeclaration" => return None,
                _ => {}
            }
            current = self.parent(k);
        }
        None
    }

    /// `inferUndefinedExpressionType(expression)`.
    fn undefined_inferred_type(&self, e: usize) -> Option<TypeId> {
        let core = &self.p.query.core;
        let parent = self.parent(e)?;
        let bool_ = Some(core.bool_);
        if self.kind(e) == "MethodInvocation" {
            if self.kind(parent) == "CascadeExpression" && self.parent(parent).is_some_and(|g| self.kind(g) == "ExpressionStatement") {
                return Some(core.void_);
            }
            if self.kind(parent) == "ExpressionStatement" {
                return Some(core.void_);
            }
        }
        match self.kind(parent) {
            "ConditionalExpression" => {
                if self.children(parent).first() == Some(&e) {
                    return bool_;
                }
                let t = self.matching_parameter(parent)?;
                return Some(t);
            }
            "ExpressionFunctionBody" | "ReturnStatement" => return self.enclosing_return(e),
            "VariableDeclaration" => {
                if self.children(parent).first() == Some(&e) {
                    let s = crate::project::word(self.source, self.tree.nos[parent].inicio)?;
                    return self.bodies.tipo_local(s.start).or_else(|| {
                        crate::document_highlight::variable_declared_in(self.p, self.unit, s.start).and_then(|v| self.p.query.variable_type(v))
                    });
                }
            }
            "AssignmentExpression" => {
                let f = self.children(parent);
                if f.first() == Some(&e) {
                    return f.get(1).and_then(|&d| self.node_type(d));
                }
                if f.get(1) == Some(&e) {
                    if assignment_operator(self, parent) == "=" {
                        // `writeType`: o tipo do alvo da escrita.
                        return self.node_type(f[0]);
                    }
                    return self.operator_parameter(parent);
                }
            }
            "BinaryExpression" => {
                if self.children(parent).get(1) == Some(&e)
                    && let Some(t) = self.operator_parameter(parent)
                {
                    return Some(t);
                }
            }
            "ArgumentList" | "NamedExpression" => return self.matching_parameter(e),
            _ => {}
        }
        match self.kind(parent) {
            "AssertStatement" | "IfStatement" | "WhileStatement" if self.children(parent).first() == Some(&e) => bool_,
            "DoStatement" if self.children(parent).last() == Some(&e) => bool_,
            "PrefixExpression" if self.node_text(parent).starts_with('!') => bool_,
            "BinaryExpression" => {
                let left = *self.children(parent).first()?;
                let op = self.next_token(self.tree.nos[left].fim)?;
                matches!(&self.source[op.start..op.end], "&&" | "||").then_some(core.bool_)
            }
            _ => None,
        }
    }

    /// O tipo do único parâmetro do operador de um binário ou de um `op=`.
    fn operator_parameter(&self, n: usize) -> Option<TypeId> {
        let x = self.node_expr(n)?;
        let f = match self.bodies.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => *f,
            _ => return None,
        };
        let d = self.p.query.outline.functions.get(f.0 as usize)?;
        (d.parameters.len() == 1).then(|| d.parameters[0].ty)
    }

    /// `getMembers(clazz)`: os membros não sintéticos (acessores, campos,
    /// métodos) de cada supertipo (`allSupertypes`) e da própria classe:
    /// (nome comparado, nome exibido, espécie).
    fn members(&self, c: ClassId) -> Vec<(String, String, Member)> {
        let prog = self.p.program();
        let mut order = all_supertypes(self.p, c);
        order.push(c);
        let mut v = Vec::new();
        for k in order {
            let cl = prog.class(k);
            let mut fs: Vec<FunctionElementId> = cl.instance_members.values().chain(cl.static_members.values()).copied().collect();
            fs.sort_by_key(|f| f.0);
            // Acessores explícitos.
            for &f in &fs {
                let fe = prog.function(f);
                match fe.kind {
                    FunctionKind::Getter => v.push((self.p.name(fe.name).to_string(), self.p.name(fe.name).to_string(), Member::Getter)),
                    FunctionKind::Setter => {
                        let n = self.p.name(fe.name).to_string();
                        let base = crate::project::base_name(&n).to_string();
                        v.push((format!("{base}="), base, Member::Setter));
                    }
                    _ => {}
                }
            }
            // Campos.
            for &var in &cl.fields {
                let ve = prog.variable(var);
                let n = self.p.name(ve.name).to_string();
                v.push((n.clone(), n, Member::Field { getter: ve.getter.is_some(), setter: ve.setter.is_some() }));
            }
            // Métodos.
            for &f in &fs {
                let fe = prog.function(f);
                match fe.kind {
                    FunctionKind::Function => v.push((self.p.name(fe.name).to_string(), self.p.name(fe.name).to_string(), Member::Method)),
                    FunctionKind::Operator => v.push((self.p.name(fe.name).to_string(), self.p.name(fe.name).to_string(), Member::Operator)),
                    _ => {}
                }
            }
        }
        v
    }

    /// `getExtensionMembers(extension)`.
    fn extension_members(&self, x: ExtensionId) -> Vec<(String, String, Member)> {
        let prog = self.p.program();
        let e = prog.extension(x);
        let mut fs: Vec<FunctionElementId> = e.instance_members.values().chain(e.static_members.values()).copied().collect();
        fs.sort_by_key(|f| f.0);
        let mut v = Vec::new();
        for &f in &fs {
            let fe = prog.function(f);
            let n = self.p.name(fe.name).to_string();
            match fe.kind {
                FunctionKind::Getter => v.push((n.clone(), n, Member::Getter)),
                FunctionKind::Setter => {
                    let base = crate::project::base_name(&n).to_string();
                    v.push((format!("{base}="), base, Member::Setter));
                }
                _ => {}
            }
        }
        for &var in &e.fields {
            let ve = prog.variable(var);
            let n = self.p.name(ve.name).to_string();
            v.push((n.clone(), n, Member::Field { getter: ve.getter.is_some(), setter: ve.setter.is_some() }));
        }
        for &f in &fs {
            let fe = prog.function(f);
            let n = self.p.name(fe.name).to_string();
            match fe.kind {
                FunctionKind::Function => v.push((n.clone(), n, Member::Method)),
                FunctionKind::Operator => v.push((n.clone(), n, Member::Operator)),
                _ => {}
            }
        }
        v
    }
}

/// A espécie de um membro candidato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Member {
    Getter,
    Setter,
    Field { getter: bool, setter: bool },
    Method,
    Operator,
}

/// `allSupertypes` (`ClassHierarchy.implementedInterfaces`): o supertipo e os
/// dele, as restrições `on`, as interfaces e os mixins, na ordem do
/// `InterfacesMerger`; as aplicações de mixin sintéticas se desfazem no
/// supertipo e nos mixins.
/// As marcas de `e` (a declaração escrita) dentro de `text`, onde ela
/// entrou (pelo `insertIntoUnitMember` ou com o separador antes).
fn marks_in(text: &str, e: crate::snippets::Write) -> Option<(usize, crate::snippets::Write)> {
    text.find(&e.text).map(|k| (k, e))
}

/// `addSuperTypesAsSuggestions(type)` num contexto só de leitura: os
/// supertipos instanciados são internados numa cópia da tabela de tipos,
/// feita na primeira vez (`copy`).
pub(crate) fn supertype_suggestions_at(p: &Project, copy: &mut Option<dartforge_types::TypeTable>, t: TypeId) -> Vec<String> {
    let Type::Interface { class, args, nullable } = p.query.table.get(t).clone() else { return Vec::new() };
    let mut v = vec![p.query.format(t)];
    let classes = all_supertypes(p, class);
    if classes.is_empty() {
        return v;
    }
    let table = copy.get_or_insert_with(|| p.query.table.clone());
    let base = table.intern(Type::Interface { class, args, nullable: false });
    for s in classes {
        if let Some(mut st) = p.query.outline.hierarchy.supertype_of(base, s, table, &p.query.core) {
            if nullable && let Type::Interface { class, args, .. } = table.get(st).clone() {
                st = table.intern(Type::Interface { class, args, nullable: true });
            }
            v.push(table.format_sem_alias(st, &p.query.names, p.program()));
        }
    }
    v
}

pub(crate) fn all_supertypes(p: &Project, c: ClassId) -> Vec<ClassId> {
    fn direct(p: &Project, c: ClassId) -> (Option<ClassId>, Vec<ClassId>, Vec<ClassId>, Vec<ClassId>) {
        let prog = p.program();
        let cl = prog.class(c);
        let mut sup = cl.supertype_class;
        let mut mixins: Vec<ClassId> = Vec::new();
        // `S with M1, M2`: a cadeia sintética `S&M1&M2`.
        while let Some(s) = sup {
            let k = prog.class(s);
            if k.kind == ClassKind::MixinApplication && k.decl.is_none() {
                let mut m = k.mixin_classes.clone();
                m.extend(mixins);
                mixins = m;
                sup = k.supertype_class;
            } else {
                break;
            }
        }
        mixins.extend(cl.mixin_classes.iter().copied());
        (sup, cl.on_classes.clone(), cl.interface_classes.clone(), mixins)
    }
    fn implemented(p: &Project, c: ClassId, memo: &mut std::collections::HashMap<ClassId, Vec<ClassId>>, stack: &mut Vec<ClassId>) -> Vec<ClassId> {
        if let Some(v) = memo.get(&c) {
            return v.clone();
        }
        if stack.contains(&c) {
            return Vec::new();
        }
        stack.push(c);
        let mut v: Vec<ClassId> = Vec::new();
        let (sup, on, interfaces, mixins) = direct(p, c);
        let join = |t: ClassId, v: &mut Vec<ClassId>, memo: &mut std::collections::HashMap<ClassId, Vec<ClassId>>, stack: &mut Vec<ClassId>| {
            if !v.contains(&t) {
                v.push(t);
            }
            for x in implemented(p, t, memo, stack) {
                if !v.contains(&x) {
                    v.push(x);
                }
            }
        };
        if let Some(s) = sup {
            join(s, &mut v, memo, stack);
        }
        for t in on.into_iter().chain(interfaces).chain(mixins) {
            join(t, &mut v, memo, stack);
        }
        stack.pop();
        memo.insert(c, v.clone());
        v
    }
    implemented(p, c, &mut std::collections::HashMap::new(), &mut Vec::new())
}

/// `nameOfType`: o nome de um `SimpleIdentifier` filho de `NamedType` ou
/// com a primeira letra igual à maiúscula dela.
fn type_name(cx: &Context<'_>, n: usize) -> Option<String> {
    if cx.kind(n) != "SimpleIdentifier" {
        return None;
    }
    let name = cx.node_text(n).to_string();
    let parent_type = cx.parent(n).is_some_and(|p| cx.kind(p) == "NamedType");
    let first = name.chars().next()?;
    let uppercase = first.to_uppercase().next() == Some(first);
    (parent_type || uppercase).then_some(name)
}

/// O resultado de um produtor: título, espécie, edições (por URI) e
/// bibliotecas a importar na unidade de destino.
struct Proposal {
    title: String,
    kind: &'static str,
    unit: UnitId,
    edits: Vec<(Span, String)>,
    import: BTreeSet<LibraryId>,
    /// Os grupos ligados (e a seleção) da declaração escrita, com o offset
    /// dela no texto da primeira edição: o `SnippetTextEdit` dos `Create …`.
    marks: Option<(usize, crate::snippets::Write)>,
}

/// O `CompilationUnitMember` de topo que contém o nó.
fn top_member(cx: &Context<'_>, n: usize) -> Option<usize> {
    let mut current = Some(n);
    while let Some(k) = current {
        if cx.parent(k) == Some(0) {
            return Some(k);
        }
        current = cx.parent(k);
    }
    None
}

/// O `CompilationUnitMember` mais interno (uma função local também é).
fn innermost_member(cx: &Context<'_>, n: usize) -> Option<usize> {
    let mut current = Some(n);
    while let Some(k) = current {
        if matches!(
            cx.kind(k),
            "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "FunctionDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias" | "ClassTypeAlias" | "TopLevelVariableDeclaration"
        ) {
            return Some(k);
        }
        current = cx.parent(k);
    }
    None
}

/// O nó da declaração de uma classe ou extensão na unidade dela, com o
/// contexto daquela unidade.
fn declaration_in_unit<'p>(p: &'p Project, unit: UnitId, decl: ast::DeclId) -> Option<(Context<'p>, usize)> {
    let cx = Context::new(p, unit);
    let n = cx.tree.nos.iter().position(|no| matches!(no.marca, Marca::Decl(d) if d == decl) && no.especie.ends_with("Declaration"))?;
    Some((cx, n))
}

impl Project {
    /// As propostas de um produtor para o nó.
    fn propose(&self, cx: &Context<'_>, producer: Producer, node: usize) -> Option<Proposal> {
        use Producer::*;
        let eol = Text::new(cx.source).eol();
        let prog = self.program();
        match producer {
            SwapClass | SwapAnnotation => {
                let mut target = node;
                if producer == SwapAnnotation {
                    // `Annotation` com nome sem elemento e com argumentos.
                    let annotation = cx.this_or_ancestor_pub(node, "Annotation")?;
                    let name = *cx.children(annotation).first()?;
                    if cx.resolved(name).is_some() || cx.kind_children(annotation, "ArgumentList").is_empty() {
                        return None;
                    }
                    target = name;
                }
                let (prefix, token) = match cx.kind(target) {
                    "NamedType" => {
                        let pr = cx.kind_children(target, "ImportPrefixReference").first().map(|&p| cx.node_text(p).trim_end_matches('.').trim().to_string());
                        let start = cx.kind_children(target, "ImportPrefixReference").first().map_or(cx.tree.nos[target].inicio, |&p| cx.tree.nos[p].fim);
                        (pr, crate::project::word(cx.source, cx.next_token(start)?.start)?)
                    }
                    "PrefixedIdentifier" => {
                        let f = cx.children(target);
                        let p = *f.first()?;
                        if !matches!(cx.resolved(p), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) {
                            return None;
                        }
                        (Some(cx.node_text(p).to_string()), cx.tree.span(*f.get(1)?))
                    }
                    "SimpleIdentifier" => (None, cx.tree.span(target)),
                    _ => return None,
                };
                let lib = prog.unit(cx.unit).library;
                let mut best = Closest::new(&cx.source[token.start..token.end]);
                if prefix.is_none() {
                    // `unit.classes` de cada unidade (só classes).
                    for &u in prog.library(lib).units.iter() {
                        let mut cs: Vec<(usize, ClassId)> = (0..prog.classes.len())
                            .map(|i| ClassId(i as u32))
                            .filter_map(|c| {
                                let d = prog.class(c).decl?;
                                (d.unit == u && matches!(prog.class(c).kind, ClassKind::Class | ClassKind::MixinApplication)).then(|| (prog.unit(u).ast.decl(d.decl).span.start, c))
                            })
                            .collect();
                        cs.sort();
                        for (_, c) in cs {
                            let n = self.name(prog.class(c).name);
                            best.see(n, n);
                        }
                    }
                }
                for imp in prog.library(lib).imports.iter().filter(|i| prog.library(lib).units.first() == Some(&i.unit)) {
                    let pr = imp.prefix.map(|s| self.name(s).to_string());
                    if pr != prefix {
                        continue;
                    }
                    let mut names: Vec<(&str, Element)> = prog
                        .library(imp.library)
                        .exported
                        .iter()
                        .filter_map(|(s, b)| b.getter.map(|g| (self.name(*s), g)))
                        .filter(|(n, _)| {
                            imp.combinators.iter().all(|c| match c {
                                ast::Combinator::Show(ns) => ns.iter().any(|x| self.name(x.sym) == *n),
                                ast::Combinator::Hide(ns) => !ns.iter().any(|x| self.name(x.sym) == *n),
                            })
                        })
                        .collect();
                    names.sort_by(|a, b| a.0.cmp(b.0));
                    for (n, el) in names {
                        if matches!(el, Element::Class(_)) {
                            best.see(n, n);
                        }
                    }
                }
                let proposed = best.name?;
                Some(Proposal { title: format!("Change to '{proposed}'"), kind: "quickfix.change.to", unit: cx.unit, edits: vec![(token, proposed)], import: BTreeSet::new(), marks: None })
            }
            SwapFunction => {
                if cx.kind(node) != "SimpleIdentifier" {
                    return None;
                }
                let mut prefix = None;
                if let Some(parent) = cx.parent(node)
                    && cx.kind(parent) == "MethodInvocation"
                    && cx.method_name(parent) == Some(node)
                    && let Some(&target) = cx.children(parent).first()
                    && target != node
                    && matches!(cx.resolved(target), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..))))
                {
                    prefix = Some(cx.node_text(target).to_string());
                }
                let lib = prog.unit(cx.unit).library;
                let mut best = Closest::new(cx.node_text(node));
                if prefix.is_none() {
                    for &u in prog.library(lib).units.iter() {
                        let mut fs: Vec<(usize, &str)> = prog
                            .unit(u)
                            .ast
                            .decls
                            .iter()
                            .filter_map(|d| match &d.kind {
                                ast::DeclKind::Function(f) => {
                                    let func = prog.unit(u).ast.function(*f);
                                    (func.kind == ast::FunctionKind::Function).then(|| func.name.map(|n| (n.span.start, self.name(n.sym))))?
                                }
                                _ => None,
                            })
                            .collect();
                        fs.sort();
                        for (_, n) in fs {
                            best.see(n, n);
                        }
                    }
                }
                for imp in prog.library(lib).imports.iter().filter(|i| prog.library(lib).units.first() == Some(&i.unit)) {
                    if imp.prefix.map(|s| self.name(s).to_string()) != prefix {
                        continue;
                    }
                    let mut names: Vec<&str> = prog
                        .library(imp.library)
                        .exported
                        .iter()
                        .filter(|(_, b)| matches!(b.getter, Some(Element::Function(f)) if prog.function(f).kind == FunctionKind::Function))
                        .map(|(s, _)| self.name(*s))
                        .filter(|n| {
                            imp.combinators.iter().all(|c| match c {
                                ast::Combinator::Show(ns) => ns.iter().any(|x| self.name(x.sym) == *n),
                                ast::Combinator::Hide(ns) => !ns.iter().any(|x| self.name(x.sym) == *n),
                            })
                        })
                        .collect();
                    names.sort();
                    for n in names {
                        best.see(n, n);
                    }
                }
                let proposed = best.name?;
                Some(Proposal { title: format!("Change to '{proposed}'"), kind: "quickfix.change.to", unit: cx.unit, edits: vec![(cx.tree.span(node), proposed)], import: BTreeSet::new(), marks: None })
            }
            SwapGetterOrSetter | SwapMethod => {
                if cx.kind(node) != "SimpleIdentifier" {
                    return None;
                }
                let parent = cx.parent(node)?;
                let (target, predicate): (Option<usize>, Box<dyn Fn(Member) -> bool>) = if producer == SwapMethod {
                    if cx.kind(parent) != "MethodInvocation" || cx.method_name(parent) != Some(node) {
                        return None;
                    }
                    let target = cx.children(parent).first().copied().filter(|&a| a != node);
                    (target, Box::new(|m| m == Member::Method))
                } else {
                    let le = in_read_context(cx, node);
                    let writes = in_write_context(cx, node);
                    (
                        qualified_target(cx, node),
                        Box::new(move |m| match m {
                            Member::Getter => le,
                            Member::Setter => writes,
                            Member::Field { getter, setter } => (le && getter) || (writes && setter),
                            _ => false,
                        }),
                    )
                };
                let members = match target {
                    None => {
                        // Só a `ClassDeclaration` envolvente.
                        let class = cx.this_or_ancestor_pub(node, "ClassDeclaration")?;
                        let Marca::Decl(d) = cx.tree.nos[class].marca else { return None };
                        cx.members(cx.declaration_class(cx.unit, d)?)
                    }
                    Some(a) => match cx.target_extension(a) {
                        Some(x) => cx.extension_members(x),
                        None => cx.members(cx.target_interface(a)?),
                    },
                };
                let mut best = Closest::new(cx.node_text(node));
                for (compared, displayed, m) in &members {
                    if predicate(*m) {
                        best.see(compared, displayed);
                    }
                }
                let proposed = best.name?;
                Some(Proposal { title: format!("Change to '{proposed}'"), kind: "quickfix.change.to", unit: cx.unit, edits: vec![(cx.tree.span(node), proposed)], import: BTreeSet::new(), marks: None })
            }
            CreateClass | CreateMixin => {
                let mut target = node;
                let mut arguments = None;
                let mut requires_const = false;
                if cx.kind(target) == "Annotation" {
                    if producer == CreateMixin {
                        return None;
                    }
                    let name = *cx.children(target).first()?;
                    arguments = cx.kind_children(target, "ArgumentList").first().copied();
                    if cx.resolved(name).is_some() || arguments.is_none() {
                        return None;
                    }
                    target = name;
                    requires_const = true;
                }
                let (name, prefix): (String, Option<usize>) = match cx.kind(target) {
                    "NamedType" => {
                        let pr = cx.kind_children(target, "ImportPrefixReference").first().copied();
                        if let Some(p) = pr {
                            let s = cx.node_text(p).trim_end_matches('.').trim();
                            let symbol = self.query.names.lookup(s)?;
                            if !crate::project::is_prefix(prog, prog.unit(cx.unit).library, symbol) {
                                return None;
                            }
                        }
                        let start = pr.map_or(cx.tree.nos[target].inicio, |p| cx.tree.nos[p].fim);
                        let s = crate::project::word(cx.source, cx.next_token(start)?.start)?;
                        requires_const |= requires_const_constructor(cx, target);
                        (cx.source[s.start..s.end].to_string(), pr)
                    }
                    "SimpleIdentifier" => {
                        if producer == CreateMixin {
                            // Não o identificador de `a.b` nem a propriedade.
                            if qualified_target(cx, target).is_some() {
                                return None;
                            }
                            (cx.node_text(target).to_string(), None)
                        } else {
                            requires_const |= requires_const_constructor(cx, target);
                            (type_name(cx, target)?, None)
                        }
                    }
                    "PrefixedIdentifier" => {
                        if producer == CreateMixin && cx.parent(target).is_some_and(|p| cx.kind(p) == "InstanceCreationExpression") {
                            return None;
                        }
                        let f = cx.children(target);
                        let p = *f.first()?;
                        if !matches!(cx.resolved(p), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) {
                            return None;
                        }
                        let id = *f.get(1)?;
                        let n = if producer == CreateMixin { cx.node_text(id).to_string() } else { type_name(cx, id)? };
                        (n, Some(p))
                    }
                    _ => return None,
                };
                // Onde.
                let (unit, offset, before, after) = match prefix {
                    None => {
                        let member = top_member(cx, target)?;
                        (cx.unit, cx.tree.nos[member].fim, format!("{eol}{eol}"), String::new())
                    }
                    Some(p) => {
                        let s = cx.node_text(p).trim_end_matches('.').trim().to_string();
                        let lib = prog.unit(cx.unit).library;
                        let imp = prog.library(lib).imports.iter().find(|i| i.prefix.is_some_and(|x| self.name(x) == s))?;
                        let &u = prog.library(imp.library).units.first()?;
                        let source = &prog.unit(u).source;
                        let eol_target = Text::new(source).eol();
                        (u, source.len(), eol_target.to_string(), eol_target.to_string())
                    }
                };
                let eol_d = Text::new(&prog.unit(unit).source).eol().to_string();
                // O grupo `NAME` no nome da classe (e no do construtor) e os
                // dos parâmetros (`create_class.dart:118-139`).
                let mut e = crate::snippets::Write::default();
                e.write(&before);
                let text = if producer == CreateMixin {
                    e.write("mixin ").linked("NAME", &name, &[]).write(&format!(" {{{eol_d}}}{after}"));
                    e.text.clone()
                } else if arguments.is_none() && !requires_const {
                    e.write("class ").linked("NAME", &name, &[]).write(&format!(" {{{eol_d}}}{after}"));
                    e.text.clone()
                } else {
                    let target_ = Context::new(self, unit);
                    let mut writer = Writer::new(&target_, offset);
                    let c = if requires_const { "const " } else { "" };
                    e.write("class ").linked("NAME", &name, &[]).write(&format!(" {{{eol_d}  {c}")).linked("NAME", &name, &[]).write("(");
                    if let Some(a) = arguments {
                        cx.parameters_to_arguments_in(a, &mut writer, &mut e);
                    }
                    e.write(&format!(");{eol_d}}}{after}"));
                    let t = e.text.clone();
                    return Some(Proposal {
                        title: format!("Create class '{name}'"),
                        kind: "quickfix.create.class",
                        unit,
                        edits: vec![(Span { start: offset, end: offset }, t)],
                        import: writer.import, marks: Some((0, e)),
                    });
                };
                let (title, kind) = if producer == CreateMixin {
                    (format!("Create mixin '{name}'"), "quickfix.create.mixin")
                } else {
                    (format!("Create class '{name}'"), "quickfix.create.class")
                };
                Some(Proposal { title, kind, unit, edits: vec![(Span { start: offset, end: offset }, text)], import: BTreeSet::new(), marks: Some((0, e)) })
            }
            CreateField | CreateGetter | CreateSetter => self.create_field_getter_setter(cx, producer, node),
            CreateLocal => {
                if cx.kind(node) != "SimpleIdentifier" {
                    return None;
                }
                let name = cx.node_text(node).to_string();
                if let Some(a) = cx.parent(node)
                    && cx.kind(a) == "AssignmentExpression"
                    && cx.children(a).first() == Some(&node)
                    && assignment_operator(cx, a) == "="
                    && cx.parent(a).is_some_and(|s| cx.kind(s) == "ExpressionStatement")
                {
                    let o = cx.tree.nos[node].inicio;
                    return Some(Proposal { title: format!("Create local variable '{name}'"), kind: "quickfix.create.localVariable", unit: cx.unit, edits: vec![(Span { start: o, end: o }, "var ".into())], import: BTreeSet::new(), marks: None });
                }
                if qualified_target(cx, node).is_some() || cx.parent(node).is_some_and(|p| matches!(cx.kind(p), "PrefixedIdentifier" | "PropertyAccess")) {
                    return None;
                }
                let command = (0..).scan(Some(node), |a, _| {
                    let k = (*a)?;
                    *a = cx.parent(k);
                    Some(k)
                })
                .find(|&k| cx.is_command(k))?;
                let prefix = cx.node_prefix(command);
                let ty = cx.undefined_inferred_type(node);
                if let Some(t) = ty
                    && !matches!(self.query.table.get(t), Type::Interface { .. } | Type::Function { .. } | Type::Record { .. } | Type::FutureOr { .. } | Type::ExtensionType { .. })
                {
                    return None;
                }
                let o = cx.tree.nos[command].inicio;
                let mut writer = Writer::new(cx, o);
                // `writeLocalVariableDeclaration(nameGroupName: 'NAME',
                // typeGroupName: 'TYPE')`.
                let mut e = crate::snippets::Write::default();
                match ty.and_then(|t| writer.write(t, false)) {
                    Some(t) => {
                        e.linked("TYPE", &t, &[]);
                    }
                    None if ty.is_some() => {}
                    None => {
                        e.write("var");
                    }
                }
                e.write(" ").linked("NAME", &name, &[]).write(&format!(";{eol}{prefix}"));
                let text = e.text.clone();
                Some(Proposal {
                    title: format!("Create local variable '{name}'"),
                    kind: "quickfix.create.localVariable",
                    unit: cx.unit,
                    edits: vec![(Span { start: o, end: o }, text)],
                    import: writer.import, marks: Some((0, e)),
                })
            }
            CreateParameter => self.create_parameter(cx, node),
            CreateMethodOrFunction => self.create_method_or_function(cx, node),
            CreateFunction => {
                if cx.kind(node) != "SimpleIdentifier" {
                    return None;
                }
                let inv = cx.parent(node).filter(|&p| cx.kind(p) == "MethodInvocation")?;
                if cx.method_name(inv) != Some(node) || cx.children(inv).first() != Some(&node) {
                    return None;
                }
                let member = innermost_member(cx, node)?;
                let o = cx.tree.nos[member].fim;
                let name = cx.node_text(node).to_string();
                let mut writer = Writer::new(cx, o);
                let ret = cx.undefined_inferred_type(inv).and_then(|t| writer.write_type(Some(t), false));
                let list = cx.kind_children(inv, "ArgumentList").first().copied()?;
                // `RETURN_TYPE`, `NAME` e os grupos dos parâmetros
                // (`create_function.dart:51-69`).
                let mut e = crate::snippets::Write::default();
                e.write(&format!("{eol}{eol}"));
                if let Some(r) = &ret {
                    e.linked("RETURN_TYPE", r, &[]).write(" ");
                }
                e.linked("NAME", &name, &[]).write("(");
                cx.parameters_to_arguments_in(list, &mut writer, &mut e);
                e.write(&format!(") {{{eol}}}"));
                let text = e.text.clone();
                Some(Proposal { title: format!("Create function '{name}'"), kind: "quickfix.create.function", unit: cx.unit, edits: vec![(Span { start: o, end: o }, text)], import: writer.import, marks: Some((0, e)) })
            }
            CreateMethod => self.create_method(cx, node),
            CreateExtensionGetter | CreateExtensionMethod | CreateExtensionSetter => self.create_extension_member(cx, producer, node),
        }
    }

    /// `CreateField`/`CreateGetter`/`CreateSetter` (`create_field.dart`,
    /// `create_getter.dart`, `create_setter.dart`).
    fn create_field_getter_setter(&self, cx: &Context<'_>, producer: Producer, node: usize) -> Option<Proposal> {
        let prog = self.program();
        // `this.x` sem campo (só o `CreateField`).
        if producer == Producer::CreateField
            && let Some(param) = cx.this_or_ancestor_pub(node, "FieldFormalParameter")
        {
            let constructor = cx.this_or_ancestor_pub(param, "ConstructorDeclaration")?;
            let container = cx.parent(constructor)?;
            if !matches!(cx.kind(container), "ClassDeclaration" | "EnumDeclaration") {
                return None;
            }
            let name = crate::project::word(cx.source, cx.tree.nos[param].fim.checked_sub(1)?)?;
            let name = cx.source[name.start..name.end].to_string();
            let constant = cx.initial_tokens(constructor).iter().any(|s| s == "const");
            let o = cx.insert_in_member(container, Filter::Field, "")?.0;
            let mut writer = Writer::new(cx, o);
            let ty = cx.ast.members.iter().find_map(|m| match &m.kind {
                ast::MemberKind::Constructor(k) => k.parameters.iter().find(|q| q.this_ && q.span.start >= cx.tree.nos[param].inicio && q.span.end <= cx.tree.nos[param].fim),
                _ => None,
            });
            let ty = ty.and_then(|q| q.ty).and_then(|t| self.query.outline.tipos_escritos.get(&(cx.unit, t)).copied());
            let decl = field_declaration(&mut writer, &name, constant, false, ty);
            let (o, text) = cx.insert_in_member(container, Filter::Field, &decl.text)?;
            let marks = marks_in(&text, decl);
            return Some(Proposal { title: format!("Create field '{name}'"), kind: "quickfix.create.field", unit: cx.unit, edits: vec![(Span { start: o, end: o }, text)], import: writer.import, marks });
        }
        if cx.kind(node) != "SimpleIdentifier" {
            return None;
        }
        let name = cx.node_text(node).to_string();
        if producer == Producer::CreateGetter && !in_read_context(cx, node) {
            return None;
        }
        if producer == Producer::CreateSetter && !in_write_context(cx, node) {
            return None;
        }
        let target_ = qualified_target(cx, node);
        // O destino: classe ou extensão, e o `static`.
        enum Target {
            Class(ClassId),
            Extension(ExtensionId),
        }
        let (target, is_static) = match target_ {
            Some(a) if producer != Producer::CreateField && cx.is_extension_override(a) => (Target::Extension(cx.target_extension(a)?), false),
            Some(a) if producer != Producer::CreateField && cx.target_extension(a).is_some() => (Target::Extension(cx.target_extension(a)?), true),
            Some(a) => {
                let c = if producer == Producer::CreateField {
                    let c = cx.target_interface(a);
                    if cx.kind(a) == "SimpleIdentifier" && cx.resolved(a).is_none() {
                        return None;
                    }
                    c?
                } else if cx.named_class(a).is_some() {
                    // `K.x` com `K` uma classe: o `staticType` do nome não é
                    // um tipo de interface (ou é o `Type` do SDK), e o
                    // `CreateGetter`/`CreateSetter` do Dart não oferece nada.
                    return None;
                } else {
                    match self.query.table.get(cx.node_type(a)?) {
                        Type::Interface { class, .. } => *class,
                        _ => return None,
                    }
                };
                let is_static = cx.named_class(a).is_some_and(|k| prog.class(k).kind == ClassKind::Class);
                (Target::Class(c), is_static)
            }
            None => {
                let d = match cx.enclosing_interface(node) {
                    Some((c, _)) => Target::Class(c),
                    None => {
                        if producer == Producer::CreateField {
                            // Também enum (`enclosingInterfaceElement2` só dá
                            // classe e mixin).
                            return None;
                        }
                        Target::Extension(cx.enclosing_extension(node)?.0)
                    }
                };
                (d, cx.in_static_context(node))
            }
        };
        let type_node = climb_access(cx, node);
        if producer == Producer::CreateField
            && let Target::Class(c) = &target
            && prog.class(*c).kind == ClassKind::Enum
            && cx.parent(type_node).is_some_and(|p| cx.kind(p) == "AssignmentExpression" && cx.children(p).first() == Some(&type_node))
        {
            return None;
        }
        let ty = cx.undefined_inferred_type(type_node);
        // A declaração de destino (fora do SDK), na unidade dela.
        let (lib, decl) = match &target {
            Target::Class(c) => (prog.class(*c).library, prog.class(*c).decl?),
            Target::Extension(x) => (prog.extension(*x).library, prog.extension(*x).decl),
        };
        if prog.library(lib).is_sdk {
            return None;
        }
        let (dcx, container) = declaration_in_unit(self, decl.unit, decl.decl)?;
        let allowed = match producer {
            Producer::CreateField => matches!(dcx.kind(container), "ClassDeclaration" | "EnumDeclaration" | "MixinDeclaration"),
            _ => matches!(dcx.kind(container), "ClassDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration"),
        };
        if !allowed {
            return None;
        }
        let filter = if producer == Producer::CreateField { Filter::Field } else { Filter::Getter };
        let o = dcx.insert_in_member(container, filter, "")?.0;
        let mut writer = Writer::new(&dcx, o);
        let s = if is_static { "static " } else { "" };
        // Os grupos `TYPE` e `NAME` de `writeFieldDeclaration`,
        // `writeGetterDeclaration` e `writeSetterDeclaration`.
        let mut decl = crate::snippets::Write::default();
        decl.write(s);
        let (title, kind) = match producer {
            Producer::CreateField => {
                let final_ = dcx.kind(container) == "EnumDeclaration";
                let field = field_declaration(&mut writer, &name, false, final_, ty);
                let base = decl.text.len();
                decl.text.push_str(&field.text);
                for (n, positions, c, sug) in field.groups {
                    decl.groups.push((n, positions.into_iter().map(|p| base + p).collect(), c, sug));
                }
                (format!("Create field '{name}'"), "quickfix.create.field")
            }
            Producer::CreateGetter => {
                let t = ty.filter(|t| !matches!(self.query.table.get(*t), Type::Dynamic)).and_then(|t| writer.write(t, false));
                if let Some(t) = &t {
                    decl.linked("TYPE", t, &[]).write(" ");
                }
                decl.write("get ").linked("NAME", &name, &[]).write(" => null;");
                (format!("Create getter '{name}'"), "quickfix.create.getter")
            }
            _ => {
                let t = ty.filter(|t| !matches!(self.query.table.get(*t), Type::Dynamic)).and_then(|t| writer.write(t, false));
                decl.write("set ").linked("NAME", &name, &[]).write("(");
                if let Some(t) = &t {
                    decl.linked("TYPE", t, &[]).write(" ");
                }
                decl.write(&format!("{name}) {{}}"));
                (format!("Create setter '{name}'"), "quickfix.create.setter")
            }
        };
        let (o, text) = dcx.insert_in_member(container, filter, &decl.text)?;
        let marks = marks_in(&text, decl);
        Some(Proposal { title, kind, unit: dcx.unit, edits: vec![(Span { start: o, end: o }, text)], import: writer.import, marks })
    }

    /// `CreateParameter` (`create_parameter.dart`).
    fn create_parameter(&self, cx: &Context<'_>, node: usize) -> Option<Proposal> {
        if cx.kind(node) != "SimpleIdentifier" {
            return None;
        }
        let name = cx.node_text(node).to_string();
        // A lista: a da `FunctionExpression` mais interna, senão do método,
        // senão do construtor.
        let owner = ["FunctionExpression", "MethodDeclaration", "ConstructorDeclaration"].iter().find_map(|e| cx.this_or_ancestor_pub(node, e))?;
        let list = cx.kind_children(owner, "FormalParameterList").first().copied()?;
        let ty = cx.undefined_inferred_type(node);
        let o0 = cx.tree.nos[list].inicio;
        let mut writer = Writer::new(cx, o0);
        let t = writer.write(ty.unwrap_or(self.query.core.dynamic_), true).unwrap_or_else(|| "dynamic".into());
        let parameters: Vec<usize> = cx.children(list).to_vec();
        let required_ = |p: usize| {
            let s = cx.tree.span(p);
            cx.ast
                .functions
                .iter()
                .flat_map(|f| f.parameters.iter().flatten())
                .chain(cx.ast.members.iter().flat_map(|m| match &m.kind {
                    ast::MemberKind::Constructor(k) => k.parameters.iter().collect::<Vec<_>>(),
                    _ => Vec::new(),
                }))
                .find(|q| q.span.start >= s.start && q.span.end <= s.end)
                .map(|q| q.kind)
        };
        let r: Vec<usize> = parameters.iter().copied().filter(|&p| required_(p) == Some(ast::ParameterKind::Required)).collect();
        let n: Vec<usize> = parameters.iter().copied().filter(|&p| required_(p) == Some(ast::ParameterKind::Named)).collect();
        let last = r.last().or(n.last()).copied();
        let trailing_comma = parameters.last().and_then(|&p| cx.next_token(cx.tree.nos[p].fim)).is_some_and(|t| &cx.source[t.start..t.end] == ",");
        let has_next = (!r.is_empty() && parameters.iter().any(|&p| required_(p) != Some(ast::ParameterKind::Required)))
            || (r.is_empty() && parameters.iter().any(|&p| required_(p) != Some(ast::ParameterKind::Named)));
        let required = r.is_empty() && last.is_some() && !ty.is_some_and(|x| self.query.table.get(x).is_declared_nullable());
        let req = if required { "required " } else { "" };
        let tx = Text::new(cx.source);
        let (offset, text) = match last {
            None => {
                let mut s = format!("{t} {name}");
                if trailing_comma {
                    s.push(',');
                } else if has_next {
                    s.push_str(", ");
                }
                (o0 + 1, s)
            }
            Some(u) if trailing_comma => {
                let v = cx.next_token(cx.tree.nos[u].fim)?;
                (v.end, format!("{}{}{req}{t} {name},", tx.eol(), tx.line_prefix(cx.tree.nos[u].inicio)))
            }
            Some(u) if has_next => {
                let v = cx.next_token(cx.tree.nos[u].fim)?;
                (v.end + 1, format!("{t} {name}, "))
            }
            Some(u) => (cx.tree.nos[u].fim, format!(", {req}{t} {name}")),
        };
        // `writeParameter(nameGroupName: 'NAME', typeGroupName: 'TYPE')`: o
        // tipo com as sugestões de supertipos e o nome.
        let marks = text.rfind(&format!("{t} {name}")).map(|k| {
            let mut e = crate::snippets::Write { text: text.clone(), ..Default::default() };
            let suggestions = ty.map(|x| supertype_suggestions_at(self, &mut None, x)).unwrap_or_default();
            e.groups.push(("TYPE".into(), vec![k], t.len(), suggestions));
            e.groups.push(("NAME".into(), vec![k + t.len() + 1], name.len(), Vec::new()));
            (0, e)
        });
        Some(Proposal {
            title: format!("Create required positional parameter '{name}'"),
            kind: "quickfix.create.parameter",
            unit: cx.unit,
            edits: vec![(Span { start: offset, end: offset }, text)],
            import: writer.import, marks,
        })
    }

    /// `CreateMethodOrFunction` (`create_method_or_function.dart`).
    fn create_method_or_function(&self, cx: &Context<'_>, node: usize) -> Option<Proposal> {
        if cx.kind(node) != "SimpleIdentifier" {
            return None;
        }
        let prog = self.program();
        let name = cx.node_text(node).to_string();
        let (class_, mut argument) = match qualified_target(cx, node) {
            Some(a) => match self.query.table.get(cx.node_type(a)?) {
                Type::Interface { class, .. } => (Some(*class), cx.parent(node)?),
                _ => return None,
            },
            None => (cx.enclosing_interface(node).map(|(c, _)| c), node),
        };
        if let Some(p) = cx.parent(argument)
            && cx.kind(p) == "NamedExpression"
        {
            argument = p;
        }
        let mut parameter = cx.matching_parameter(argument);
        if let Some(p) = cx.parent(argument)
            && cx.kind(p) == "ConditionalExpression"
        {
            if cx.children(p).first() == Some(&argument) {
                return None;
            }
            parameter = cx.matching_parameter(p);
        }
        let type_f = parameter?;
        // `Function` vale `dynamic Function()`.
        let (ret, positional, optional, named): (TypeId, Vec<TypeId>, Vec<TypeId>, Vec<(dartforge_intern::SymbolId, TypeId, bool)>) = match self.query.table.get(type_f).clone() {
            Type::Interface { class, .. } if Some(class) == self.query.core.function_class => (self.query.core.dynamic_, Vec::new(), Vec::new(), Vec::new()),
            Type::Function { ret, positional, optional, named, .. } => (ret, positional.to_vec(), optional.to_vec(), named.to_vec()),
            _ => return None,
        };
        // `RETURN_TYPE`, `NAME` e o `writeFormalParameters` do tipo de função
        // (`TYPE<i>` com as sugestões de supertipos; `PARAM<i>` nos não
        // nomeados), `create_method_or_function.dart:105-128`.
        let write = |writer: &mut Writer<'_, '_>| -> crate::snippets::Write {
            let mut copy: Option<dartforge_types::TypeTable> = None;
            let mut e = crate::snippets::Write::default();
            if let Some(r) = writer.write(ret, false) {
                e.linked("RETURN_TYPE", &r, &[]).write(" ");
            }
            e.linked("NAME", &name, &[]);
            // `writeFormalParameters`: os posicionais sem nome viram `p1`…
            e.write("(");
            let mut i = 0usize;
            let mut used: BTreeSet<String> = named.iter().map(|(n, _, _)| self.name(*n).to_string()).collect();
            let generate = |used: &mut BTreeSet<String>| {
                let mut k = 1;
                while used.contains(&format!("p{k}")) {
                    k += 1;
                }
                let n = format!("p{k}");
                used.insert(n.clone());
                n
            };
            let mut opened_o = false;
            let mut opened_n = false;
            for (q, optional_) in positional.iter().map(|q| (*q, false)).chain(optional.iter().map(|q| (*q, true))) {
                if i > 0 {
                    e.write(", ");
                }
                if optional_ && !opened_o {
                    e.write("[");
                    opened_o = true;
                }
                let n = generate(&mut used);
                if let Some(t) = writer.write(q, false) {
                    let suggestions = supertype_suggestions_at(self, &mut copy, q);
                    e.linked(&format!("TYPE{i}"), &t, &suggestions).write(" ");
                }
                e.linked(&format!("PARAM{i}"), &n, &[]);
                i += 1;
            }
            for (n, q, required) in named.iter() {
                if i > 0 {
                    e.write(", ");
                }
                if !opened_n {
                    e.write("{");
                    opened_n = true;
                }
                if *required {
                    e.write("required ");
                }
                if let Some(t) = writer.write(*q, false) {
                    let suggestions = supertype_suggestions_at(self, &mut copy, *q);
                    e.linked(&format!("TYPE{i}"), &t, &suggestions).write(" ");
                }
                e.write(self.name(*n));
                i += 1;
            }
            if opened_n {
                e.write("}");
            }
            if opened_o {
                e.write("]");
            }
            e.write(")");
            e
        };
        match class_ {
            Some(c) if matches!(prog.class(c).kind, ClassKind::Class | ClassKind::Mixin) => {
                let decl = prog.class(c).decl?;
                let (dcx, container) = declaration_in_unit(self, decl.unit, decl.decl)?;
                let o = dcx.tree.nos[container].fim - 1;
                let eol = Text::new(dcx.source).eol();
                let mut writer = Writer::new(&dcx, o);
                let before = if dcx.container_members(container).is_empty() { "" } else { eol };
                let s = if cx.in_static_context(node) { "static " } else { "" };
                let body = write(&mut writer);
                let start = format!("{before}  {s}");
                let text = format!("{start}{} {{{eol}  }}{eol}", body.text);
                Some(Proposal { title: format!("Create method '{name}'"), kind: "quickfix.create.method", unit: dcx.unit, edits: vec![(Span { start: o, end: o }, text)], import: writer.import, marks: Some((start.len(), body)) })
            }
            Some(_) => None,
            None => {
                let o = cx.source.len();
                let eol = Text::new(cx.source).eol();
                let mut writer = Writer::new(cx, o);
                let body = write(&mut writer);
                let text = format!("{eol}{} {{{eol}}}{eol}", body.text);
                Some(Proposal { title: format!("Create function '{name}'"), kind: "quickfix.create.function", unit: cx.unit, edits: vec![(Span { start: o, end: o }, text)], import: writer.import, marks: Some((eol.len(), body)) })
            }
        }
    }

    /// `CreateMethod.method` (`create_method.dart:92-205`).
    fn create_method(&self, cx: &Context<'_>, node: usize) -> Option<Proposal> {
        let prog = self.program();
        if cx.kind(node) != "SimpleIdentifier" {
            return None;
        }
        let inv = cx.parent(node).filter(|&p| cx.kind(p) == "MethodInvocation")?;
        let name = cx.node_text(node).to_string();
        let target = cx.children(inv).first().copied().filter(|&a| a != node);
        let (unit, decl, is_static): (UnitId, Option<ast::DeclId>, bool) = match target {
            Some(a) if cx.is_extension_override(a) => {
                let x = prog.extension(cx.target_extension(a)?).decl;
                (x.unit, Some(x.decl), false)
            }
            Some(a) if cx.target_extension(a).is_some() => {
                let x = prog.extension(cx.target_extension(a)?).decl;
                (x.unit, Some(x.decl), true)
            }
            None => {
                let member = (0..)
                    .scan(Some(node), |s, _| {
                        let k = (*s)?;
                        *s = cx.parent(k);
                        Some(k)
                    })
                    .find(|&k| matches!(cx.kind(k), "MethodDeclaration" | "FieldDeclaration" | "ConstructorDeclaration"))?;
                let container = cx.parent(member)?;
                let Marca::Decl(d) = cx.tree.nos[container].marca else { return None };
                (cx.unit, Some(d), cx.in_static_context(node))
            }
            Some(a) => {
                let c = cx.target_interface(a)?;
                let k = prog.class(c);
                if prog.library(k.library).is_sdk {
                    return None;
                }
                if !matches!(k.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::ExtensionType | ClassKind::MixinApplication) {
                    return None;
                }
                let d = k.decl?;
                let is_static = cx.named_class(a).is_some();
                (d.unit, Some(d.decl), is_static)
            }
        };
        let (dcx, container) = declaration_in_unit(self, unit, decl?)?;
        let o = dcx.insert_in_member(container, Filter::Method, "")?.0;
        let mut writer = Writer::new(&dcx, o);
        let s = if is_static { "static " } else { "" };
        let ret = cx.undefined_inferred_type(inv).and_then(|t| writer.write_type(Some(t), false));
        let list = cx.kind_children(inv, "ArgumentList").first().copied()?;
        // `RETURN_TYPE`, `NAME` e os parâmetros (`create_method.dart:183-200`).
        let mut e = crate::snippets::Write::default();
        e.write(s);
        if let Some(r) = &ret {
            e.linked("RETURN_TYPE", r, &[]).write(" ");
        }
        e.linked("NAME", &name, &[]).write("(");
        cx.parameters_to_arguments_in(list, &mut writer, &mut e);
        e.write(") {}");
        let (o, text) = dcx.insert_in_member(container, Filter::Method, &e.text)?;
        let marks = marks_in(&text, e);
        Some(Proposal { title: format!("Create method '{name}'"), kind: "quickfix.create.method", unit: dcx.unit, edits: vec![(Span { start: o, end: o }, text)], import: writer.import, marks })
    }

    /// `CreateExtensionGetter`/`Method`/`Setter`
    /// (`create_extension_member.dart`).
    fn create_extension_member(&self, cx: &Context<'_>, producer: Producer, node: usize) -> Option<Proposal> {
        if cx.kind(node) != "SimpleIdentifier" {
            return None;
        }
        let name = cx.node_text(node).to_string();
        let (target, inv) = match producer {
            Producer::CreateExtensionMethod => {
                let inv = cx.parent(node).filter(|&p| cx.kind(p) == "MethodInvocation")?;
                if cx.method_name(inv) != Some(node) {
                    return None;
                }
                (cx.children(inv).first().copied().filter(|&a| a != node)?, Some(inv))
            }
            Producer::CreateExtensionGetter => {
                if !in_read_context(cx, node) {
                    return None;
                }
                (qualified_target(cx, node)?, None)
            }
            _ => {
                if !in_write_context(cx, node) {
                    return None;
                }
                (qualified_target(cx, node)?, None)
            }
        };
        // O nome de uma classe como alvo (`K.m()`): o `staticType` dele no
        // analyzer é nulo, e o Dart não oferece o membro de extensão.
        if cx.named_class(target).is_some() {
            return None;
        }
        let type_target = cx.node_type(target)?;
        if matches!(self.query.table.get(type_target), Type::Dynamic) || matches!(self.query.table.exibicao(type_target), Some(dartforge_types::table::Exibicao::Invalido)) {
            return None;
        }
        let eol = Text::new(cx.source).eol();
        // A extensão existente aplicável (a primeira da unidade).
        let existing = self.applicable_extension(cx, type_target);
        // A declaração e os grupos de cada produtor: o `VALUE` do getter, o
        // `NAME`/`TYPE` do `writeSetterDeclaration`, e `RETURN_TYPE`, `NAME`
        // e os parâmetros do método (`create_extension_member.dart`).
        let member = |writer: &mut Writer<'_, '_>| -> crate::snippets::Write {
            let mut e = crate::snippets::Write::default();
            match producer {
                Producer::CreateExtensionGetter => {
                    let t = cx.undefined_inferred_type(climb_access(cx, node)).and_then(|t| writer.write_type(Some(t), false)).map(|t| format!("{t} ")).unwrap_or_default();
                    e.write(&format!("{t}get {name} => ")).linked("VALUE", "null", &[]).write(";");
                }
                Producer::CreateExtensionSetter => {
                    let t = cx.undefined_inferred_type(climb_access(cx, node)).and_then(|t| writer.write_type(Some(t), false));
                    e.write("set ").linked("NAME", &name, &[]).write("(");
                    if let Some(t) = &t {
                        e.linked("TYPE", t, &[]).write(" ");
                    }
                    e.write(&format!("{name}) {{}}"));
                }
                _ => {
                    let inv = inv.unwrap_or(node);
                    let t = cx.undefined_inferred_type(inv).and_then(|t| writer.write_type(Some(t), false));
                    if let Some(t) = &t {
                        e.linked("RETURN_TYPE", t, &[]).write(" ");
                    }
                    e.linked("NAME", &name, &[]).write("(");
                    if let Some(l) = cx.kind_children(inv, "ArgumentList").first().copied() {
                        cx.parameters_to_arguments_in(l, writer, &mut e);
                    }
                    e.write(") {}");
                }
            }
            e
        };
        let (title, kind) = match producer {
            Producer::CreateExtensionGetter => (format!("Create extension getter '{name}'"), "quickfix.create.extension.getter"),
            Producer::CreateExtensionSetter => (format!("Create extension setter '{name}'"), "quickfix.create.extension.setter"),
            _ => (format!("Create extension method '{name}'"), "quickfix.create.extension.method"),
        };
        let (o, text, import, marks) = match existing {
            Some(ext) => {
                let filter = if producer == Producer::CreateExtensionMethod { Filter::Method } else { Filter::Getter };
                let o0 = cx.insert_in_member(ext, filter, "")?.0;
                let mut writer = Writer::new(cx, o0);
                let m = member(&mut writer);
                let (o, t) = cx.insert_in_member(ext, filter, &m.text)?;
                let marks = marks_in(&t, m);
                (o, t, writer.import, marks)
            }
            None => {
                let top = top_member(cx, node)?;
                let o = cx.tree.nos[top].fim;
                let mut writer = Writer::new(cx, o);
                let ty = writer.write(type_target, true).unwrap_or_else(|| "dynamic".into());
                let m = member(&mut writer);
                let before = format!("{eol}{eol}extension on {ty} {{{eol}{ONE_INDENT}");
                let text = format!("{before}{}{eol}}}", m.text);
                (o, text, writer.import, Some((before.len(), m)))
            }
        };
        Some(Proposal { title, kind, unit: cx.unit, edits: vec![(Span { start: o, end: o }, text)], import, marks })
    }

    /// `_existingExtension`: a primeira `ExtensionDeclaration` da unidade que
    /// se aplica ao tipo.
    fn applicable_extension(&self, cx: &Context<'_>, ty: TypeId) -> Option<usize> {
        let prog = self.program();
        for (k, no) in cx.tree.nos.iter().enumerate() {
            if no.especie != "ExtensionDeclaration" || cx.parent(k) != Some(0) {
                continue;
            }
            let Marca::Decl(d) = no.marca else { continue };
            let Some(x) = (0..prog.extensions.len()).map(|i| ExtensionId(i as u32)).find(|&x| prog.extension(x).decl.unit == cx.unit && prog.extension(x).decl.decl == d) else { continue };
            if extension_applies(self, x, ty) {
                return Some(k);
            }
        }
        None
    }

    /// As propostas de um código para o nó, como ações.
    fn code_actions(&self, cx: &Context<'_>, code: Code, node: usize, d: Option<&Diagnostic>) -> Vec<CodeAction> {
        let mut output = Vec::new();
        for &pr in producers(code) {
            let Some(p) = self.propose(cx, pr, node) else { continue };
            let Some(uri) = self.unit_uri(p.unit) else { continue };
            let main = p.edits.first().cloned();
            let mut m = Change::default();
            for (s, t) in p.edits {
                m.add(&uri, s, t);
            }
            if !p.import.is_empty() {
                let target = Context::new(self, p.unit);
                crate::refactorings_move::builder_imports(&target, &mut m, &p.import);
            }
            if m.conflict.is_some() {
                continue;
            }
            let edits: Vec<Edit> = m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect();
            let mut a = CodeAction { title: p.title, kind: p.kind.into(), edits, diagnostic: d.cloned(), create_file: None, linked: Default::default() };
            // Os grupos e a seleção da declaração, na edição dela.
            if let (Some((base, e)), Some((s, t))) = (&p.marks, &main)
                && let Some(i) = a.edits.iter().position(|x| x.span == *s && x.text == *t)
            {
                a = a.with_write_at(i, *base, e);
            }
            output.push(a);
        }
        output
    }

    /// As correções de criação e troca dos códigos publicados (o `node` do
    /// diagnóstico).
    pub(crate) fn create_for_diagnostic(&self, unit: UnitId, d: &Diagnostic, code: Code) -> Vec<CodeAction> {
        let cx = Context::new(self, unit);
        let Some(node) = cx.tree.localizar(d.span.start, d.span.end) else { return Vec::new() };
        self.code_actions(&cx, code, node, Some(d))
    }

    /// As correções de criação e troca para os nomes sem resolução das
    /// linhas `start..end` de `unit`, cada um com o código que o
    /// analyzer daria.
    pub(crate) fn create_undefined(&mut self, _uri: &str, unit: UnitId, start: usize, end: usize) -> Vec<CodeAction> {
        let cx = Context::new(self, unit);
        let prog = self.program();
        let lib = prog.unit(unit).library;
        let mut uses: Vec<(Code, usize)> = Vec::new();
        for (n, no) in cx.tree.nos.iter().enumerate() {
            if !(no.inicio <= end && start <= no.fim) {
                continue;
            }
            match no.especie {
                "NamedType" => {
                    // Tipo escrito sem resolução: `UNDEFINED_CLASS`.
                    let Some(t) = cx.ast.types.iter().position(|t| t.span.start == no.inicio && t.span.end == no.fim) else { continue };
                    let ast::TypeKind::Named { name, .. } = &cx.ast.ty(ast::TypeId(t as u32)).kind else { continue };
                    let [nm] = &name[..] else { continue };
                    let text = self.name(nm.sym);
                    if prog.lookup(lib, nm.sym).is_some()
                        || ["dynamic", "Never", "void", "Function", "Record"].contains(&text)
                        || crate::project::type_parameter_declaration(cx.ast, nm.span.start, nm.sym).is_some()
                    {
                        continue;
                    }
                    uses.push((Code::Class, n));
                }
                "SimpleIdentifier" => {
                    let Marca::Expr(x) = no.marca else { continue };
                    if cx.bodies.get_resolved(x).is_some() || cx.bodies.declaracao_local(x).is_some() {
                        continue;
                    }
                    let Some(parent) = cx.parent(n) else { continue };
                    let qualified = qualified_target(&cx, n);
                    let code = if cx.kind(parent) == "MethodInvocation" && cx.method_name(parent) == Some(n) {
                        match cx.children(parent).first().copied().filter(|&a| a != n) {
                            None => {
                                if self.query.names.lookup(cx.node_text(n)).is_some_and(|s| prog.lookup(lib, s).is_some()) {
                                    continue;
                                }
                                if cx.this_or_ancestor_pub(n, "ClassDeclaration").is_some()
                                    || cx.this_or_ancestor_pub(n, "MixinDeclaration").is_some()
                                    || cx.this_or_ancestor_pub(n, "EnumDeclaration").is_some()
                                    || cx.this_or_ancestor_pub(n, "ExtensionTypeDeclaration").is_some()
                                    || cx.this_or_ancestor_pub(n, "ExtensionDeclaration").is_some()
                                {
                                    Code::Method
                                } else {
                                    Code::Function
                                }
                            }
                            Some(a) => {
                                if matches!(cx.resolved(a), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) {
                                    Code::Function
                                } else if cx.target_extension(a).is_some() {
                                    Code::ExtensionMethod
                                } else if cx.node_type(a).is_some_and(|t| matches!(self.query.table.get(t), Type::Dynamic)) {
                                    continue;
                                } else {
                                    Code::Method
                                }
                            }
                        }
                    } else if let Some(a) = qualified {
                        if matches!(cx.resolved(a), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) {
                            // `UNDEFINED_PREFIXED_NAME`: só o `DataDriven`.
                            continue;
                        }
                        if cx.node_type(a).is_some_and(|t| matches!(self.query.table.get(t), Type::Dynamic)) && cx.named_class(a).is_none() {
                            continue;
                        }
                        let ext = cx.target_extension(a).is_some();
                        match (in_write_context(&cx, n), ext) {
                            (true, true) => Code::ExtensionSetter,
                            (true, false) => Code::Setter,
                            (false, true) => Code::ExtensionGetter,
                            (false, false) => Code::Getter,
                        }
                    } else {
                        let symbol = self.query.names.lookup(cx.node_text(n));
                        if symbol.is_some_and(|s| prog.lookup(lib, s).is_some()) {
                            continue;
                        }
                        Code::Identifier
                    };
                    uses.push((code, n));
                }
                _ => {}
            }
        }
        let mut output = Vec::new();
        for (code, n) in uses {
            output.extend(self.code_actions(&cx, code, n, None));
        }
        output
    }
}

/// `writeFieldDeclaration(name, isFinal, isStatic, type)` (com `static`
/// posto pelo chamador).
/// `writeFieldDeclaration(name, nameGroupName: 'NAME', type,
/// typeGroupName: 'TYPE')`: o ty written é o group `TYPE` (o `dynamic`
/// depois de `final` não), o nome é o `NAME`.
fn field_declaration(writer: &mut Writer<'_, '_>, name: &str, final_: bool, enum_final: bool, ty: Option<TypeId>) -> crate::snippets::Write {
    let final_ = final_ || enum_final;
    let mut e = crate::snippets::Write::default();
    if final_ {
        e.write("final ");
    }
    match ty {
        Some(t) => {
            let written = if final_ { writer.write(t, false).map(|x| (x, true)).or_else(|| Some(("dynamic".into(), false))) } else { writer.write_type(Some(t), true).map(|x| (x, true)) };
            if let Some((x, group)) = written {
                if group {
                    e.linked("TYPE", &x, &[]);
                } else {
                    e.write(&x);
                }
                e.write(" ");
            }
        }
        None if !final_ => {
            e.write("var ");
        }
        None => {}
    }
    e.linked("NAME", name, &[]).write(";");
    e
}

/// `_requiresConstConstructor`.
fn requires_const_constructor(cx: &Context<'_>, n: usize) -> bool {
    let Some(parent) = cx.parent(n) else { return false };
    match (cx.kind(n), cx.kind(parent)) {
        ("SimpleIdentifier", "NamedType") => requires_const_constructor(cx, parent),
        ("SimpleIdentifier", "MethodInvocation") => in_constant_context(cx, parent),
        ("NamedType", "ConstructorName") => requires_const_constructor(cx, parent),
        ("ConstructorName", "InstanceCreationExpression") => cx.next_token(cx.tree.nos[parent].inicio).is_some_and(|t| &cx.source[t.start..t.end] == "const"),
        _ => false,
    }
}

/// `inConstantContext`: dentro de criação `const`, literal `const`,
/// inicializador de variável `const`, anotação ou padrão constante.
pub(crate) fn in_constant_context(cx: &Context<'_>, n: usize) -> bool {
    let mut current = cx.parent(n);
    while let Some(k) = current {
        match cx.kind(k) {
            "InstanceCreationExpression" | "ListLiteral" | "SetOrMapLiteral" | "RecordLiteral" => {
                if cx.next_token(cx.tree.nos[k].inicio).is_some_and(|t| &cx.source[t.start..t.end] == "const") {
                    return true;
                }
            }
            "VariableDeclarationList" => {
                if cx.initial_tokens_pub(k).iter().any(|s| s == "const") {
                    return true;
                }
            }
            "Annotation" | "ConstantPattern" | "SwitchCase" => return true,
            "FunctionBody" | "BlockFunctionBody" | "ExpressionFunctionBody" | "FunctionExpression" => return false,
            _ => {}
        }
        current = cx.parent(k);
    }
    false
}

/// `extension.applicableTo(targetType, strictCasts: true)`: o receptor é
/// subtipo do tipo estendido com os parâmetros da extensão nos limites.
fn extension_applies(p: &Project, x: ExtensionId, recv: TypeId) -> bool {
    let query = &p.query;
    let Some(data) = query.outline.extensions.get(x.0 as usize) else { return false };
    let mut table = query.table.clone();
    let on = if data.type_params.is_empty() {
        data.on
    } else {
        let map: std::collections::HashMap<dartforge_types::TypeParamId, TypeId> = data.type_params.iter().map(|&tp| (tp, table.param(tp).bound)).collect();
        dartforge_types::substitute(data.on, &map, &mut table)
    };
    let mut env = dartforge_types::SubtypeEnv::new(&mut table, &query.outline.hierarchy, &query.core);
    dartforge_types::is_subtype(recv, on, &mut env)
}
