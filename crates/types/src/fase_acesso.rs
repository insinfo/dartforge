//! O `_InvalidAccessVerifier` e a checagem de classe `@sealed` do
//! `BestPracticesVerifier`
//! (`analyzer/lib/src/error/best_practices_verifier.dart`, lidos na 6.11.0;
//! docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.7):
//!
//! * `invalid_use_of_internal_member`, `invalid_use_of_protected_member`,
//!   `invalid_use_of_visible_for_template_member`,
//!   `invalid_use_of_visible_for_testing_member` e
//!   `invalid_use_of_visible_for_overriding_member`, no uso de um elemento
//!   de **outra** biblioteca anotado com `@internal`, `@protected`,
//!   `@visibleForTemplate`, `@visibleForTesting` ou `@visibleForOverriding`;
//! * `subtype_of_sealed_class` e `mixin_on_sealed_class`, na classe ou no
//!   mixin que tem por supertipo uma classe `@sealed` de outro pacote.
//!
//! Cobre identificadores e acessos a propriedade resolvidos, construtores e
//! tipos nomeados. Fora: os operadores (`verifyBinary`), o import de
//! biblioteca `@internal`, os campos de padrão, a chamada implícita ao
//! construtor da superclasse, o `@doNotSubmit` e os nomes de combinadores.
//! As anotações são reconhecidas pelo nome. A posição dos dois de classe
//! selada é a da declaração sem o comentário de documentação.
//! Escrito sem compilar nem executar (2026-10-04).

use crate::fase_deprecado::{of_function, of_variable, of_element};
use crate::resolve::OutlineTypes;
use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberKind, TypeKind};
use dartforge_intern::Interner;

/// O elemento referido: as anotações dele, a unidade em que está declarado
/// e a classe que o contém.
struct Referenced<'p> {
    anotacoes: &'p [ast::Annotation],
    unidade: UnitId,
    classe: Option<ClassId>,
    /// Classe, enum ou mixin: fora da regra do `@visibleForTemplate`.
    is_type: bool,
}

fn referenced<'p>(program: &'p Program, r: &Resolved) -> Option<Referenced<'p>> {
    let (finding, class, is_type) = match r {
        Resolved::Constructor(f) | Resolved::Element(Element::Function(f)) => (of_function(program, *f), program.function(*f).class, false),
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
            (of_function(program, *f), program.function(*f).class, false)
        }
        Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => {
            (of_variable(program, *v), program.variable(*v).class, false)
        }
        Resolved::Element(e @ Element::Class(_)) => (of_element(program, *e), None, true),
        Resolved::Element(e @ (Element::Typedef(_) | Element::Extension(_))) => (of_element(program, *e), None, false),
        _ => return None,
    };
    let (annotations, unit) = finding?;
    Some(Referenced { anotacoes: annotations, unidade: unit, classe: class, is_type })
}

/// As anotações da declaração de uma classe.
fn da_classe(program: &Program, c: ClassId) -> &[ast::Annotation] {
    program.class(c).decl.map_or(&[][..], |d| &program.unit(d.unit).ast.decl(d.decl).metadata[..])
}

/// Os relatos de acesso inválido na unidade `u`. `mesmo_pacote` diz se uma
/// biblioteca está no pacote da biblioteca analisada.
pub fn acessos_invalidos(
    program: &Program,
    interner: &Interner,
    outline: &OutlineTypes,
    body: Option<&UnitBodyTypes>,
    u: UnitId,
    same_package: &dyn Fn(LibraryId) -> bool,
) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let names = ["internal", "protected", "visibleForTemplate", "visibleForTesting", "visibleForOverriding", "sealed"];
    if !names.iter().any(|n| interner.lookup(n).is_some()) {
        return out;
    }
    let unit = program.unit(u);
    let a = &unit.ast;
    let lib = unit.library;
    let has = |metadata: &[ast::Annotation], the_name: &str| metadata.iter().any(|m| m.name.last().is_some_and(|n| interner.resolve(n.sym) == the_name));
    // `_inTemplateSource` e `inTestDir`, pelo caminho do arquivo.
    let path = unit.path.as_ref().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
    let in_template = path.contains(".template");
    let in_test = ["/test/", "/integration_test/", "/test_driver/", "/testing/"].iter().any(|d| path.contains(d));
    // As classes desta unidade, pela posição: `_enclosingClass`.
    let local_classes: Vec<(Span, ClassId)> = program
        .classes
        .iter()
        .enumerate()
        .filter(|(_, c)| c.library == lib && matches!(c.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::Enum))
        .filter_map(|(i, c)| c.decl.filter(|d| d.unit == u).map(|d| (a.decl(d.decl).span, ClassId(i as u32))))
        .collect();
    let class_in = |s: Span| local_classes.iter().find(|(r, _)| r.start <= s.start && s.end <= r.end).map(|(_, c)| *c);
    // O método que contém a posição, com o nome dele.
    let method_in = |s: Span| {
        a.members.iter().find_map(|m| match &m.kind {
            MemberKind::Method(f) if m.span.start <= s.start && s.end <= m.span.end => a.function(*f).name.map(|n| interner.resolve(n.sym)),
            _ => None,
        })
    };
    let inherits = |of: Option<ClassId>, defining: ClassId| {
        of.is_some_and(|c| c == defining || outline.hierarchy.get(c).is_some_and(|d| d.supertypes.contains_key(&defining)))
    };
    // `_checkForInvalidInternalAccess` e `_checkForOtherInvalidAccess`.
    let mut check = |tgt: &Referenced<'_>, span: Span, the_name: &str, where_: Span, via_super: bool| {
        let owner = program.unit(tgt.unidade);
        // `_inCurrentLibrary`.
        if owner.library == lib {
            return;
        }
        let mut report = |d: Diagnostic| {
            if !out.iter().any(|x| x.code == d.code && x.span == d.span) {
                out.push(d);
            }
        };
        if has(tgt.anotacoes, "internal") && !same_package(owner.library) {
            report(Diagnostic::com_codigo(w::INVALID_USE_OF_INTERNAL_MEMBER, span, [the_name]));
        }
        let protected = has(tgt.anotacoes, "protected");
        if protected && tgt.classe.is_some_and(|defining| inherits(class_in(where_), defining)) {
            return;
        }
        let of_owner = tgt.classe.map_or(&[][..], |c| da_classe(program, c));
        let for_template = !tgt.is_type
            && (has(tgt.anotacoes, "visibleForTemplate") || has(of_owner, "visibleForTemplate"))
            && !(has(tgt.anotacoes, "visibleOutsideTemplate") || has(of_owner, "visibleOutsideTemplate"));
        if for_template && in_template {
            return;
        }
        let for_test = has(tgt.anotacoes, "visibleForTesting");
        if for_test && in_test {
            return;
        }
        let for_override = has(tgt.anotacoes, "visibleForOverriding");
        // `super.m()` dentro do próprio `m` é o uso previsto.
        if for_override && via_super && method_in(where_) == Some(the_name) {
            return;
        }
        let uri = owner.uri.as_str();
        if protected {
            report(Diagnostic::com_codigo(w::INVALID_USE_OF_PROTECTED_MEMBER, span, [the_name, uri]));
        }
        if for_template {
            report(Diagnostic::com_codigo(w::INVALID_USE_OF_VISIBLE_FOR_TEMPLATE_MEMBER, span, [the_name, uri]));
        }
        if for_test {
            report(Diagnostic::com_codigo(w::INVALID_USE_OF_VISIBLE_FOR_TESTING_MEMBER, span, [the_name, uri]));
        }
        if for_override {
            report(Diagnostic::com_codigo(w::INVALID_USE_OF_VISIBLE_FOR_OVERRIDING_MEMBER, span, [the_name]));
        }
    };
    // Tipos nomeados (`verifyNamedType`).
    for t in a.types.iter() {
        let TypeKind::Named { name, .. } = &t.kind else { continue };
        let binding = match &name[..] {
            [n] => program.lookup_na_unidade(u, n.sym),
            [p, n] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        };
        let (Some(e @ (Element::Class(_) | Element::Typedef(_))), Some(last)) = (binding.and_then(|b| b.getter), name.last()) else {
            continue;
        };
        if let Some(tgt) = referenced(program, &Resolved::Element(e)) {
            check(&tgt, last.span, interner.resolve(last.sym), t.span, false);
        }
    }
    // Identificadores e propriedades resolvidos (`verify`).
    if let Some(body) = body {
        for (i, expr) in a.exprs.iter().enumerate() {
            let Some(Some(r)) = body.resolved.get(i) else { continue };
            let (at_name, via_super) = match &expr.kind {
                ExprKind::Identifier(n) => (*n, false),
                ExprKind::Property { name, target, .. } => (*name, matches!(a.expr(*target).kind, ExprKind::Super)),
                _ => continue,
            };
            let Some(tgt) = referenced(program, r) else { continue };
            // Um construtor leva o nome inteiro como está escrito (`C.nome`).
            let (span, the_name) = match r {
                Resolved::Constructor(_) => (expr.span, unit.source.get(expr.span.start..expr.span.end).unwrap_or("")),
                _ => (at_name.span, interner.resolve(at_name.sym)),
            };
            check(&tgt, span, the_name, expr.span, via_super);
        }
    }
    // `_checkForInvalidSealedSuperclass`: classe, aplicação de mixin e
    // mixin desta unidade.
    for (i, class) in program.classes.iter().enumerate() {
        if class.library != lib || !matches!(class.kind, ClassKind::Class | ClassKind::Mixin) {
            continue;
        }
        let Some(decl) = class.decl.filter(|d| d.unit == u) else { continue };
        if !matches!(a.decl(decl.decl).kind, DeclKind::Class(_) | DeclKind::Mixin(_)) {
            continue;
        }
        let cid = ClassId(i as u32);
        let Some(data) = outline.hierarchy.get(cid) else { continue };
        let mut supertypes: Vec<ClassId> = data.supertypes.keys().copied().filter(|s| *s != cid).collect();
        supertypes.sort_by_key(|c| c.0);
        for s in supertypes {
            let superclass = program.class(s);
            if !has(da_classe(program, s), "sealed") || same_package(superclass.library) {
                continue;
            }
            let code = if class.kind == ClassKind::Mixin && class.on_classes.contains(&s) {
                w::MIXIN_ON_SEALED_CLASS
            } else {
                w::SUBTYPE_OF_SEALED_CLASS
            };
            out.push(Diagnostic::com_codigo(code, a.decl(decl.decl).span, [interner.resolve(superclass.name)]));
        }
    }
    out
}
