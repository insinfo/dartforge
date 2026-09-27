//! Modificadores de classe (`base`, `final`, `interface`, `sealed`) entre
//! bibliotecas: as verificações do `ErrorVerifier` do analyzer 7.x
//! (`_checkForBaseClassOrMixinImplementedOutsideOfLibrary`,
//! `_checkForInterfaceClassOrMixinSuperclassOutsideOfLibrary`,
//! `_checkForFinalSupertypeOutsideOfLibrary`,
//! `_checkForSealedSupertypeOutsideOfLibrary`) e o
//! `BaseOrFinalTypeVerifier` (`subtype_of_base_or_final_is_not_base_final_or_sealed`).
//!
//! Só nomes e a hierarquia declarada: cada tipo das cláusulas
//! `extends`/`with`/`implements`/`on` é resolvido pelo nome no escopo da
//! biblioteca. Uma cláusula que não resolve para classe ou mixin não é
//! conferida, e uma classe com supertipo proibido (`extends int`) fica toda
//! de fora das verificações do `ErrorVerifier`, como no analyzer.

use dartforge_diagnostics::{codigos::compile_time_error as c, Codigo, Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, TypeKind};
use dartforge_intern::Interner;

/// As cláusulas de uma declaração, como escritas: `(tipo escrito, classe)`.
struct Clausulas {
    extends: Option<(ast::TypeId, Option<ClassId>)>,
    with: Vec<(ast::TypeId, Option<ClassId>)>,
    implements: Vec<(ast::TypeId, Option<ClassId>)>,
    on: Vec<(ast::TypeId, Option<ClassId>)>,
}

struct Contexto<'a> {
    programa: &'a Program,
    nomes: &'a Interner,
    lib: LibraryId,
}

impl Contexto<'_> {
    fn classe(&self, id: ClassId) -> &dartforge_elements::model::ClassElement {
        self.programa.class(id)
    }

    fn nome(&self, id: ClassId) -> String {
        self.nomes.resolve(self.classe(id).name).to_string()
    }

    fn e_base(&self, id: ClassId) -> bool {
        let c = self.classe(id);
        matches!(c.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::MixinApplication) && c.modifiers.base
    }

    fn e_final(&self, id: ClassId) -> bool {
        let c = self.classe(id);
        matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) && c.modifiers.final_
    }

    fn e_selada(&self, id: ClassId) -> bool {
        let c = self.classe(id);
        matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) && c.modifiers.sealed
    }

    fn e_interface(&self, id: ClassId) -> bool {
        let c = self.classe(id);
        matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) && c.modifiers.interface
    }

    fn e_mixin(&self, id: ClassId) -> bool {
        self.classe(id).kind == ClassKind::Mixin
    }

    /// A biblioteca de `id` tem modificadores de classe (linguagem 3.0+)?
    fn com_modificadores(&self, lib: LibraryId) -> bool {
        self.programa.library(lib).features.versao().major >= 3
    }

    /// `_mayIgnoreClassModifiers`: modificador de biblioteca da plataforma,
    /// visto do próprio SDK ou de biblioteca anterior aos modificadores
    /// (`dart:ffi` não entra na primeira exceção do `ErrorVerifier`).
    fn pode_ignorar(&self, de: LibraryId, ffi_conta: bool) -> bool {
        let origem = self.programa.library(de);
        if !origem.is_sdk {
            return false;
        }
        if ffi_conta && origem.uri == "dart:ffi" {
            return false;
        }
        let atual = self.programa.library(self.lib);
        atual.is_sdk || !self.com_modificadores(self.lib)
    }

    /// Supertipos diretos declarados de uma classe (supertipo, interfaces,
    /// mixins, `on`), pelos elementos já ligados no modelo.
    fn diretos(&self, id: ClassId) -> Vec<ClassId> {
        let c = self.classe(id);
        let mut v: Vec<ClassId> = c.supertype_class.into_iter().collect();
        v.extend(c.interface_classes.iter().copied());
        v.extend(c.mixin_classes.iter().copied());
        v.extend(c.on_classes.iter().copied());
        v
    }

    /// `[tipo, ...allSupertypes]`, sem repetição, em profundidade.
    fn todos(&self, id: ClassId) -> Vec<ClassId> {
        let mut vistos = vec![id];
        let mut pilha = vec![id];
        while let Some(x) = pilha.pop() {
            for s in self.diretos(x) {
                if !vistos.contains(&s) {
                    vistos.push(s);
                    pilha.push(s);
                }
            }
        }
        vistos
    }

    /// `_getExplicitlyBaseOrFinalElement`.
    fn base_ou_final_explicito(&self, id: ClassId, visitados: &mut Vec<ClassId>) -> Option<ClassId> {
        if visitados.contains(&id) {
            return None;
        }
        visitados.push(id);
        if (self.e_base(id) || self.e_final(id)) && !self.e_selada(id) {
            return Some(id);
        }
        let c = self.classe(id);
        let ordem = c
            .supertype_class
            .into_iter()
            .chain(c.interface_classes.iter().copied())
            .chain(c.mixin_classes.iter().copied())
            .chain(if c.kind == ClassKind::Mixin { c.on_classes.clone() } else { Vec::new() });
        for s in ordem.collect::<Vec<_>>() {
            if let Some(r) = self.base_ou_final_explicito(s, visitados) {
                return Some(r);
            }
        }
        None
    }
}

/// Os diagnósticos de modificadores de classe da biblioteca `lib`.
pub fn fora_da_biblioteca(programa: &Program, lib: LibraryId, nomes: &Interner) -> Vec<(UnitId, Diagnostic)> {
    let cx = Contexto { programa, nomes, lib };
    let mut saida = Vec::new();
    for (i, classe) in programa.classes.iter().enumerate() {
        if classe.library != lib {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let id = ClassId(i as u32);
        let ast = &programa.unit(decl.unit).ast;
        let Some(cl) = clausulas(programa, lib, ast, &ast.decl(decl.decl).kind) else { continue };
        let span = |t: ast::TypeId| ast.ty(t).span;
        // O analyzer descarta o erro repetido (mesmo código, intervalo e
        // mensagem): o `ErrorVerifier` e o `BaseOrFinalTypeVerifier` relatam
        // `class X implements SealedDeBase` cada um.
        let mut por = |codigo: Codigo, sp: Span, args: &[&str]| {
            let d = Diagnostic::com_codigo(codigo, sp, args.iter().copied());
            if !saida.iter().any(|(u, x): &(UnitId, Diagnostic)| *u == decl.unit && x.span == d.span && x.message == d.message) {
                saida.push((decl.unit, d));
            }
        };

        // `ErrorVerifier`: só sem supertipo proibido.
        let proibido = cl.extends.iter().chain(cl.implements.iter()).chain(cl.with.iter()).chain(cl.on.iter())
            .any(|(_, c)| c.is_some_and(|c| proibida(&cx, c)));
        if !proibido {
            // base implementada fora da biblioteca.
            for &(t, alvo) in &cl.implements {
                let Some(alvo) = alvo else { continue };
                for e in cx.todos(alvo) {
                    if cx.e_base(e) && cx.classe(e).library != lib && !cx.pode_ignorar(cx.classe(e).library, true) {
                        if cx.classe(e).kind != ClassKind::Mixin && !cx.e_selada(e) {
                            por(c::BASE_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.nome(e)]);
                        } else if cx.e_mixin(e) {
                            por(c::BASE_MIXIN_IMPLEMENTED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.nome(e)]);
                        }
                        break;
                    }
                }
            }
            // interface estendida fora da biblioteca.
            if let Some((t, Some(s))) = cl.extends {
                // Depois de `no_generative_constructors_in_superclass`
                // (`extends Finalizable` só relata este; `extends Pointer`,
                // `final` e também só com `factory`, relata o modificador).
                if cx.e_interface(s)
                    && !cx.e_selada(s)
                    && cx.classe(s).library != lib
                    && !cx.pode_ignorar(cx.classe(s).library, true)
                    && !sem_construtor_generativo(&cx, s)
                {
                    por(c::INTERFACE_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.nome(s)]);
                }
            }
            // final: estendida, implementada, restrição de mixin.
            if let Some((t, Some(s))) = cl.extends {
                if cx.e_final(s) && !cx.e_selada(s) && cx.classe(s).library != lib && !cx.pode_ignorar(cx.classe(s).library, true) {
                    por(c::FINAL_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.nome(s)]);
                }
            }
            for &(t, alvo) in &cl.implements {
                let Some(alvo) = alvo else { continue };
                for e in cx.todos(alvo) {
                    if cx.e_final(e) && !cx.e_selada(e) && cx.classe(e).library != lib && !cx.pode_ignorar(cx.classe(e).library, true) {
                        // Indireta numa biblioteca com modificadores: a
                        // declaração mais próxima já relata.
                        if e != alvo && cx.com_modificadores(cx.classe(alvo).library) {
                            continue;
                        }
                        por(c::FINAL_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY, span(t), &[&cx.nome(e)]);
                        break;
                    }
                }
            }
            for &(t, alvo) in &cl.on {
                let Some(s) = alvo else { continue };
                if cx.e_final(s) && !cx.e_selada(s) && cx.classe(s).library != lib && !cx.pode_ignorar(cx.classe(s).library, true) {
                    por(c::FINAL_CLASS_USED_AS_MIXIN_CONSTRAINT_OUTSIDE_OF_LIBRARY, span(t), &[&cx.nome(s)]);
                }
            }
            // sealed: qualquer cláusula.
            for &(t, alvo) in cl.extends.iter().chain(cl.with.iter()).chain(cl.implements.iter()).chain(cl.on.iter()) {
                let Some(s) = alvo else { continue };
                if cx.e_selada(s) && cx.classe(s).library != lib {
                    por(c::SEALED_CLASS_SUBTYPE_OUTSIDE_OF_LIBRARY, span(t), &[&cx.nome(s)]);
                }
            }
        }

        // `BaseOrFinalTypeVerifier.checkElement`: classes, aliases de classe e
        // mixins (enums não).
        if !matches!(classe.kind, ClassKind::Class | ClassKind::Mixin | ClassKind::MixinApplication) {
            continue;
        }
        let Some(nome_sp) = nome_da_declaracao(&ast.decl(decl.decl).kind) else { continue };
        let supertipo: Vec<(Option<ast::TypeId>, ClassId)> = cl.extends.and_then(|(_, c)| c).map(|c| (None, c)).into_iter().collect();
        let interfaces: Vec<(Option<ast::TypeId>, ClassId)> =
            cl.implements.iter().filter_map(|&(t, c)| c.map(|c| (Some(t), c))).collect();
        let mixins: Vec<(Option<ast::TypeId>, ClassId)> = cl.with.iter().filter_map(|&(_, c)| c.map(|c| (None, c))).collect();
        let on: Vec<(Option<ast::TypeId>, ClassId)> = cl.on.iter().filter_map(|&(_, c)| c.map(|c| (None, c))).collect();
        for grupo in [supertipo, interfaces, mixins, on] {
            let mut relatou = false;
            for (implementado, sup) in grupo {
                if let Some((codigo, sp, args)) = restricao(&cx, id, sup, implementado.map(span), nome_sp) {
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    por(codigo, sp, &args);
                    relatou = true;
                    break;
                }
            }
            if relatou {
                break;
            }
        }
    }
    saida
}

/// `_reportRestrictionError`: o diagnóstico, se houver.
fn restricao(cx: &Contexto<'_>, elemento: ClassId, sup: ClassId, implementado: Option<Span>, nome_sp: Span) -> Option<(Codigo, Span, Vec<String>)> {
    let lib_el = cx.classe(elemento).library;
    let lib_sup = cx.classe(sup).library;
    // Uma `sealed` que estende `base`/`final` herda o modificador (é
    // `base`/`final` induzida, como no elemento do analyzer).
    let induzida = cx.e_selada(sup) && cx.base_ou_final_explicito(sup, &mut Vec::new()).is_some();
    if !(cx.e_base(sup) || cx.e_final(sup) || induzida || (!cx.com_modificadores(lib_sup) && cx.com_modificadores(lib_el))) {
        return None;
    }
    let bf = cx.base_ou_final_explicito(sup, &mut Vec::new())?;
    let lib_bf = cx.classe(bf).library;
    if cx.pode_ignorar(lib_bf, false) {
        return None;
    }
    if let Some(sp) = implementado {
        if cx.e_selada(sup) && lib_bf != lib_el && cx.e_base(bf) {
            let codigo = if cx.e_mixin(bf) { c::BASE_MIXIN_IMPLEMENTED_OUTSIDE_OF_LIBRARY } else { c::BASE_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY };
            return Some((codigo, sp, vec![cx.nome(bf)]));
        }
    }
    if cx.e_base(elemento) || cx.e_final(elemento) || cx.e_selada(elemento) {
        return None;
    }
    let mixin = cx.e_mixin(elemento);
    if cx.e_final(bf) {
        if lib_bf != lib_el
            && (cx.com_modificadores(lib_sup) || !cx.programa.library(lib_bf).is_sdk || implementado.is_some())
        {
            return None;
        }
        let codigo = if mixin { c::MIXIN_SUBTYPE_OF_FINAL_IS_NOT_BASE } else { c::SUBTYPE_OF_FINAL_IS_NOT_BASE_FINAL_OR_SEALED };
        return Some((codigo, nome_sp, vec![cx.nome(elemento), cx.nome(bf)]));
    }
    if cx.e_base(bf) {
        let codigo = if mixin { c::MIXIN_SUBTYPE_OF_BASE_IS_NOT_BASE } else { c::SUBTYPE_OF_BASE_IS_NOT_BASE_FINAL_OR_SEALED };
        return Some((codigo, nome_sp, vec![cx.nome(elemento), cx.nome(bf)]));
    }
    None
}

/// Classes do `dart:core` que não se estendem nem implementam
/// (`EXTENDS_DISALLOWED_CLASS` e afins).
fn proibida(cx: &Contexto<'_>, id: ClassId) -> bool {
    let c = cx.classe(id);
    match &cx.programa.library(c.library).uri[..] {
        "dart:core" => matches!(cx.nomes.resolve(c.name), "bool" | "double" | "int" | "Null" | "num" | "Record" | "String"),
        "dart:async" => cx.nomes.resolve(c.name) == "FutureOr",
        _ => false,
    }
}

/// `_checkForNoGenerativeConstructorsInSuperclass`: a superclasse só tem
/// construtores `factory` (como `Finalizable`).
fn sem_construtor_generativo(cx: &Contexto<'_>, id: ClassId) -> bool {
    let c = cx.classe(id);
    c.kind == ClassKind::Class
        && !c.constructors.is_empty()
        && c.constructors.values().all(|&f| cx.programa.function(f).factory)
}

fn nome_da_declaracao(k: &DeclKind) -> Option<Span> {
    match k {
        DeclKind::Class(x) => Some(x.name.span),
        DeclKind::Mixin(x) => Some(x.name.span),
        _ => None,
    }
}

fn clausulas(programa: &Program, lib: LibraryId, ast: &ast::Ast, k: &DeclKind) -> Option<Clausulas> {
    let r = |t: ast::TypeId| (t, classe_do_tipo(programa, lib, ast, t));
    match k {
        DeclKind::Class(x) => Some(Clausulas {
            extends: x.extends.map(r),
            with: x.with.iter().map(|&t| r(t)).collect(),
            implements: x.implements.iter().map(|&t| r(t)).collect(),
            on: Vec::new(),
        }),
        DeclKind::Mixin(x) => Some(Clausulas {
            extends: None,
            with: Vec::new(),
            implements: x.implements.iter().map(|&t| r(t)).collect(),
            on: x.on.iter().map(|&t| r(t)).collect(),
        }),
        DeclKind::Enum(x) => Some(Clausulas {
            extends: None,
            with: x.with.iter().map(|&t| r(t)).collect(),
            implements: x.implements.iter().map(|&t| r(t)).collect(),
            on: Vec::new(),
        }),
        _ => None,
    }
}

/// A classe (ou mixin) que o tipo escrito nomeia no escopo da biblioteca.
fn classe_do_tipo(programa: &Program, lib: LibraryId, ast: &ast::Ast, t: ast::TypeId) -> Option<ClassId> {
    let TypeKind::Named { name, .. } = &ast.ty(t).kind else { return None };
    let b = match &name[..] {
        [n] => programa.lookup(lib, n.sym),
        [p, n] => programa.lookup_prefixed(lib, p.sym, n.sym),
        _ => None,
    }?;
    if b.ambiguous {
        return None;
    }
    match b.getter? {
        Element::Class(c) if matches!(programa.class(c).kind, ClassKind::Class | ClassKind::Mixin | ClassKind::MixinApplication) => Some(c),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_elements::{load::load_lenient, sdk::SdkLayout};
    use std::fs;

    #[test]
    fn modificadores_de_outra_biblioteca() {
        let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../target/tmp-agent/modificadores-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raiz);
        fs::create_dir_all(raiz.join("sdk/lib/core")).unwrap();
        fs::write(raiz.join("sdk/lib/libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#).unwrap();
        fs::write(raiz.join("sdk/lib/core/core.dart"), "class Object {}").unwrap();
        let sdk = SdkLayout::load(&raiz.join("sdk/lib"), "dartdevc").unwrap();
        fs::write(
            raiz.join("a.dart"),
            "base class B {}\nfinal class F {}\ninterface class I {}\nsealed class S {}\nsealed class SB extends B {}\n",
        )
        .unwrap();
        let fonte = "import 'a.dart';\n\
            class X extends B {}\n\
            base class Y implements B {}\n\
            final class Z extends F {}\n\
            class W extends I {}\n\
            final class V extends S {}\n\
            base class Q implements SB {}\n\
            base class Ok extends B {}\n";
        let entrada = raiz.join("main.dart");
        fs::write(&entrada, fonte).unwrap();
        let mut nomes = Interner::new();
        let (programa, _) = load_lenient(&entrada, &sdk, None, &mut nomes);
        let diags = fora_da_biblioteca(&programa, programa.entry.unwrap(), &nomes);
        let v: Vec<(&str, &str, &str)> = diags
            .iter()
            .map(|(_, d)| (d.code.map_or("", |c| c.info().nome), &fonte[d.span.start..d.span.end], d.message.as_str()))
            .collect();
        assert_eq!(
            v,
            vec![
                ("subtype_of_base_or_final_is_not_base_final_or_sealed", "X", "The type 'X' must be 'base', 'final' or 'sealed' because the supertype 'B' is 'base'."),
                ("invalid_use_of_type_outside_library", "B", "The class 'B' can't be implemented outside of its library because it's a base class."),
                ("invalid_use_of_type_outside_library", "F", "The class 'F' can't be extended outside of its library because it's a final class."),
                ("invalid_use_of_type_outside_library", "I", "The class 'I' can't be extended outside of its library because it's an interface class."),
                ("invalid_use_of_type_outside_library", "S", "The class 'S' can't be extended, implemented, or mixed in outside of its library because it's a sealed class."),
                // `ErrorVerifier` e `BaseOrFinalTypeVerifier` relatam o mesmo
                // erro; o analyzer guarda um só.
                ("invalid_use_of_type_outside_library", "SB", "The class 'B' can't be implemented outside of its library because it's a base class."),
                ("invalid_use_of_type_outside_library", "SB", "The class 'SB' can't be extended, implemented, or mixed in outside of its library because it's a sealed class."),
            ],
            "{diags:?}"
        );
        fs::remove_dir_all(&raiz).unwrap();
    }
}
