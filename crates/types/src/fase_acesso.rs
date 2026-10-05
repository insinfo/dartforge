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

use crate::fase_deprecado::{da_funcao, da_variavel, do_elemento};
use crate::resolve::OutlineTypes;
use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberKind, TypeKind};
use dartforge_intern::Interner;

/// O elemento referido: as anotações dele, a unidade em que está declarado
/// e a classe que o contém.
struct Referido<'p> {
    anotacoes: &'p [ast::Annotation],
    unidade: UnitId,
    classe: Option<ClassId>,
    /// Classe, enum ou mixin: fora da regra do `@visibleForTemplate`.
    e_tipo: bool,
}

fn referido<'p>(program: &'p Program, r: &Resolved) -> Option<Referido<'p>> {
    let (achado, classe, e_tipo) = match r {
        Resolved::Constructor(f) | Resolved::Element(Element::Function(f)) => (da_funcao(program, *f), program.function(*f).class, false),
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
            (da_funcao(program, *f), program.function(*f).class, false)
        }
        Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => {
            (da_variavel(program, *v), program.variable(*v).class, false)
        }
        Resolved::Element(e @ Element::Class(_)) => (do_elemento(program, *e), None, true),
        Resolved::Element(e @ (Element::Typedef(_) | Element::Extension(_))) => (do_elemento(program, *e), None, false),
        _ => return None,
    };
    let (anotacoes, unidade) = achado?;
    Some(Referido { anotacoes, unidade, classe, e_tipo })
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
    corpo: Option<&UnitBodyTypes>,
    u: UnitId,
    mesmo_pacote: &dyn Fn(LibraryId) -> bool,
) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let nomes = ["internal", "protected", "visibleForTemplate", "visibleForTesting", "visibleForOverriding", "sealed"];
    if !nomes.iter().any(|n| interner.lookup(n).is_some()) {
        return out;
    }
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let lib = unidade.library;
    let tem = |metadata: &[ast::Annotation], nome: &str| metadata.iter().any(|m| m.name.last().is_some_and(|n| interner.resolve(n.sym) == nome));
    // `_inTemplateSource` e `inTestDir`, pelo caminho do arquivo.
    let caminho = unidade.path.as_ref().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
    let em_template = caminho.contains(".template");
    let em_teste = ["/test/", "/integration_test/", "/test_driver/", "/testing/"].iter().any(|d| caminho.contains(d));
    // As classes desta unidade, pela posição: `_enclosingClass`.
    let classes_daqui: Vec<(Span, ClassId)> = program
        .classes
        .iter()
        .enumerate()
        .filter(|(_, c)| c.library == lib && matches!(c.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::Enum))
        .filter_map(|(i, c)| c.decl.filter(|d| d.unit == u).map(|d| (a.decl(d.decl).span, ClassId(i as u32))))
        .collect();
    let classe_em = |s: Span| classes_daqui.iter().find(|(r, _)| r.start <= s.start && s.end <= r.end).map(|(_, c)| *c);
    // O método que contém a posição, com o nome dele.
    let metodo_em = |s: Span| {
        a.members.iter().find_map(|m| match &m.kind {
            MemberKind::Method(f) if m.span.start <= s.start && s.end <= m.span.end => a.function(*f).name.map(|n| interner.resolve(n.sym)),
            _ => None,
        })
    };
    let herda = |de: Option<ClassId>, definidora: ClassId| {
        de.is_some_and(|c| c == definidora || outline.hierarchy.get(c).is_some_and(|d| d.supertypes.contains_key(&definidora)))
    };
    // `_checkForInvalidInternalAccess` e `_checkForOtherInvalidAccess`.
    let mut checar = |alvo: &Referido<'_>, span: Span, nome: &str, onde: Span, via_super: bool| {
        let dona = program.unit(alvo.unidade);
        // `_inCurrentLibrary`.
        if dona.library == lib {
            return;
        }
        let mut relatar = |d: Diagnostic| {
            if !out.iter().any(|x| x.code == d.code && x.span == d.span) {
                out.push(d);
            }
        };
        if tem(alvo.anotacoes, "internal") && !mesmo_pacote(dona.library) {
            relatar(Diagnostic::com_codigo(w::INVALID_USE_OF_INTERNAL_MEMBER, span, [nome]));
        }
        let protegido = tem(alvo.anotacoes, "protected");
        if protegido && alvo.classe.is_some_and(|definidora| herda(classe_em(onde), definidora)) {
            return;
        }
        let da_dona = alvo.classe.map_or(&[][..], |c| da_classe(program, c));
        let para_template = !alvo.e_tipo
            && (tem(alvo.anotacoes, "visibleForTemplate") || tem(da_dona, "visibleForTemplate"))
            && !(tem(alvo.anotacoes, "visibleOutsideTemplate") || tem(da_dona, "visibleOutsideTemplate"));
        if para_template && em_template {
            return;
        }
        let para_teste = tem(alvo.anotacoes, "visibleForTesting");
        if para_teste && em_teste {
            return;
        }
        let para_sobrescrita = tem(alvo.anotacoes, "visibleForOverriding");
        // `super.m()` dentro do próprio `m` é o uso previsto.
        if para_sobrescrita && via_super && metodo_em(onde) == Some(nome) {
            return;
        }
        let uri = dona.uri.as_str();
        if protegido {
            relatar(Diagnostic::com_codigo(w::INVALID_USE_OF_PROTECTED_MEMBER, span, [nome, uri]));
        }
        if para_template {
            relatar(Diagnostic::com_codigo(w::INVALID_USE_OF_VISIBLE_FOR_TEMPLATE_MEMBER, span, [nome, uri]));
        }
        if para_teste {
            relatar(Diagnostic::com_codigo(w::INVALID_USE_OF_VISIBLE_FOR_TESTING_MEMBER, span, [nome, uri]));
        }
        if para_sobrescrita {
            relatar(Diagnostic::com_codigo(w::INVALID_USE_OF_VISIBLE_FOR_OVERRIDING_MEMBER, span, [nome]));
        }
    };
    // Tipos nomeados (`verifyNamedType`).
    for t in a.types.iter() {
        let TypeKind::Named { name, .. } = &t.kind else { continue };
        let ligacao = match &name[..] {
            [n] => program.lookup_na_unidade(u, n.sym),
            [p, n] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        };
        let (Some(e @ (Element::Class(_) | Element::Typedef(_))), Some(ultimo)) = (ligacao.and_then(|b| b.getter), name.last()) else {
            continue;
        };
        if let Some(alvo) = referido(program, &Resolved::Element(e)) {
            checar(&alvo, ultimo.span, interner.resolve(ultimo.sym), t.span, false);
        }
    }
    // Identificadores e propriedades resolvidos (`verify`).
    if let Some(corpo) = corpo {
        for (i, expr) in a.exprs.iter().enumerate() {
            let Some(Some(r)) = corpo.resolved.get(i) else { continue };
            let (no_nome, via_super) = match &expr.kind {
                ExprKind::Identifier(n) => (*n, false),
                ExprKind::Property { name, target, .. } => (*name, matches!(a.expr(*target).kind, ExprKind::Super)),
                _ => continue,
            };
            let Some(alvo) = referido(program, r) else { continue };
            // Um construtor leva o nome inteiro como está escrito (`C.nome`).
            let (span, nome) = match r {
                Resolved::Constructor(_) => (expr.span, unidade.source.get(expr.span.start..expr.span.end).unwrap_or("")),
                _ => (no_nome.span, interner.resolve(no_nome.sym)),
            };
            checar(&alvo, span, nome, expr.span, via_super);
        }
    }
    // `_checkForInvalidSealedSuperclass`: classe, aplicação de mixin e
    // mixin desta unidade.
    for (i, classe) in program.classes.iter().enumerate() {
        if classe.library != lib || !matches!(classe.kind, ClassKind::Class | ClassKind::Mixin) {
            continue;
        }
        let Some(decl) = classe.decl.filter(|d| d.unit == u) else { continue };
        if !matches!(a.decl(decl.decl).kind, DeclKind::Class(_) | DeclKind::Mixin(_)) {
            continue;
        }
        let cid = ClassId(i as u32);
        let Some(dados) = outline.hierarchy.get(cid) else { continue };
        let mut supertipos: Vec<ClassId> = dados.supertypes.keys().copied().filter(|s| *s != cid).collect();
        supertipos.sort_by_key(|c| c.0);
        for s in supertipos {
            let superclasse = program.class(s);
            if !tem(da_classe(program, s), "sealed") || mesmo_pacote(superclasse.library) {
                continue;
            }
            let codigo = if classe.kind == ClassKind::Mixin && classe.on_classes.contains(&s) {
                w::MIXIN_ON_SEALED_CLASS
            } else {
                w::SUBTYPE_OF_SEALED_CLASS
            };
            out.push(Diagnostic::com_codigo(codigo, a.decl(decl.decl).span, [interner.resolve(superclasse.name)]));
        }
    }
    out
}
