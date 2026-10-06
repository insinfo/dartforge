//! O vigésimo oitavo lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `public_member_api_docs`: só num pacote pub (há `pubspec.yaml` acima da
//!   biblioteca) e dentro do `lib/` dele. As declarações públicas sem
//!   comentário de documentação que não sobrescrevem um membro herdado
//!   (`overriddenMember`: algum supertipo da classe, mixin ou enum que as
//!   contém tem um membro de instância com o nome, o privado só na mesma
//!   biblioteca), com as exclusões do emissor: `@internal` (`meta`) na
//!   declaração de topo que contém, o nome privado, o `main`, o setter cujo
//!   getter está documentado, o construtor de enum e o da classe `sealed`,
//!   `abstract final` ou `abstract interface`, a extensão sem nome, o campo
//!   de instância de tipo de extensão. O nó relatado é o nome (o tipo de
//!   retorno no construtor sem nome).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::regras14::{depois_das_anotacoes, doc_de, sem_augment};
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::ClassId;
use dartforge_frontend::ast::{self, Annotation, DeclId, DeclKind, FunctionKind, MemberId, MemberKind};
use dartforge_frontend::comentarios::Comentarios;
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    if !ligada("public_member_api_docs") {
        return out;
    }
    let a = u.ast;
    let fonte = u.fonte;
    let program = s.program;
    // `package.canHavePublicApi` (só o pacote pub) e `isInLibDir` (pela
    // unidade que define a biblioteca).
    let lib = program.library(program.unit(s.unidade).library);
    let Some(arquivo) = lib.units.first().and_then(|x| program.unit(*x).path.clone()) else { return out };
    let Some(raiz) = arquivo.ancestors().skip(1).find(|d| d.join("pubspec.yaml").is_file()).map(std::path::Path::to_path_buf) else { return out };
    if !arquivo.starts_with(raiz.join("lib")) {
        return out;
    }
    let comentarios = Comentarios::de(fonte);
    let privado = |n: SymbolId| interner.resolve(n).starts_with('_');
    let interno = |metadata: &[Annotation]| metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unidade, m, "meta", "internal"));
    let documentado = |metadata: &[Annotation], pos: usize| doc_de(fonte, &comentarios, metadata, pos).is_some();
    let classe_da_decl = |d: DeclId| -> Option<ClassId> {
        (0..program.classes.len()).map(|i| ClassId(i as u32)).find(|c| program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == d))
    };
    // `overriddenMember`: o nome entre os membros de instância dos
    // supertipos da classe que contém (o setter com `_=`).
    let sobrescreve = |classe: Option<ClassId>, nome: SymbolId, setter: bool| -> bool {
        let Some(classe) = classe else { return false };
        let chave = if setter { interner.lookup(&format!("{}_=", interner.resolve(nome))) } else { Some(nome) };
        let Some(chave) = chave else { return false };
        let minha = program.class(classe).library;
        let e_privado = privado(nome);
        s.outline.hierarchy.get(classe).is_some_and(|h| {
            h.supertypes.keys().any(|sc| {
                *sc != classe && program.class(*sc).instance_members.get(&chave).is_some_and(|f| !e_privado || program.function(*f).library == minha)
            })
        })
    };
    let mut achados: Vec<Span> = Vec::new();
    let checar = |doc: bool, sobrescrito: bool, no: Span, achados: &mut Vec<Span>| -> bool {
        if !doc && !sobrescrito {
            achados.push(no);
            return true;
        }
        false
    };
    let pos_do_membro = |m: MemberId| depois_das_anotacoes(fonte, &a.member(m).metadata, a.member(m).span.start);
    let pos_da_decl = |d: &ast::Decl| {
        let mut pos = depois_das_anotacoes(fonte, &d.metadata, d.span.start);
        if d.augment && matches!(d.kind, DeclKind::Enum(_) | DeclKind::Extension(_) | DeclKind::ExtensionType(_) | DeclKind::Typedef(_) | DeclKind::Mixin(_)) {
            pos = sem_augment(fonte, pos);
        }
        pos
    };
    // `checkMethods`: os métodos (getters, setters, operadores) públicos.
    let metodos = |membros: &[MemberId], classe: Option<ClassId>, achados: &mut Vec<Span>| {
        let mut getters: HashMap<SymbolId, MemberId> = HashMap::new();
        let mut ordem_dos_getters: Vec<SymbolId> = Vec::new();
        let mut setters: Vec<MemberId> = Vec::new();
        let mut outros: Vec<MemberId> = Vec::new();
        for &m in membros {
            let MemberKind::Method(f) = &a.member(m).kind else { continue };
            let f = a.function(*f);
            let Some(n) = f.name else { continue };
            if privado(n.sym) {
                continue;
            }
            match f.kind {
                FunctionKind::Getter => {
                    if getters.insert(n.sym, m).is_none() {
                        ordem_dos_getters.push(n.sym);
                    }
                }
                FunctionKind::Setter => setters.push(m),
                _ => outros.push(m),
            }
        }
        let mut sem_doc: Vec<MemberId> = Vec::new();
        let checar_metodo = |m: MemberId, achados: &mut Vec<Span>| -> bool {
            let MemberKind::Method(f) = &a.member(m).kind else { return false };
            let f = a.function(*f);
            let Some(n) = f.name else { return false };
            let doc = documentado(&a.member(m).metadata, pos_do_membro(m));
            let sobrescrito = sobrescreve(classe, n.sym, f.kind == FunctionKind::Setter);
            if !doc && !sobrescrito {
                achados.push(n.span);
                return true;
            }
            false
        };
        for n in ordem_dos_getters {
            let m = getters[&n];
            if checar_metodo(m, achados) {
                sem_doc.push(m);
            }
        }
        for m in setters {
            let MemberKind::Method(f) = &a.member(m).kind else { continue };
            let Some(n) = a.function(*f).name else { continue };
            if getters.get(&n.sym).is_some_and(|g| sem_doc.contains(g)) {
                checar_metodo(m, achados);
            }
        }
        for m in outros {
            checar_metodo(m, achados);
        }
    };

    // `visitCompilationUnit`: as funções de topo.
    {
        let mut getters: HashMap<SymbolId, &ast::Decl> = HashMap::new();
        let mut ordem: Vec<SymbolId> = Vec::new();
        let mut setters: Vec<&ast::Decl> = Vec::new();
        let mut funcoes: Vec<&ast::Decl> = Vec::new();
        for d in a.decls.iter() {
            let DeclKind::Function(f) = &d.kind else { continue };
            let f = a.function(*f);
            let Some(n) = f.name else { continue };
            if privado(n.sym) || interner.resolve(n.sym) == "main" {
                continue;
            }
            match f.kind {
                FunctionKind::Getter => {
                    if getters.insert(n.sym, d).is_none() {
                        ordem.push(n.sym);
                    }
                }
                FunctionKind::Setter => setters.push(d),
                _ => funcoes.push(d),
            }
        }
        let nome = |d: &ast::Decl| match &d.kind {
            DeclKind::Function(f) => a.function(*f).name,
            _ => None,
        };
        let mut sem_doc: Vec<*const ast::Decl> = Vec::new();
        for n in ordem {
            let d = getters[&n];
            if checar(documentado(&d.metadata, pos_da_decl(d)), false, nome(d).map_or(d.span, |x| x.span), &mut achados) {
                sem_doc.push(d as *const ast::Decl);
            }
        }
        for d in setters {
            let Some(n) = nome(d) else { continue };
            if getters.get(&n.sym).is_some_and(|g| sem_doc.contains(&(*g as *const ast::Decl))) {
                checar(documentado(&d.metadata, pos_da_decl(d)), false, n.span, &mut achados);
            }
        }
        for d in funcoes {
            // `isEffectivelyPrivate`: o `@internal` da própria função.
            if interno(&d.metadata) {
                continue;
            }
            let n = nome(d).map_or(d.span, |x| x.span);
            checar(documentado(&d.metadata, pos_da_decl(d)), false, n, &mut achados);
        }
    }

    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        let classe = classe_da_decl(id);
        let doc = || documentado(&d.metadata, pos_da_decl(d));
        // O próprio tipo "sobrescreve" quando um supertipo tem membro com o
        // nome dele (`overriddenMember` do elemento da classe).
        match &d.kind {
            DeclKind::Class(x) if x.mixin_application => {
                // `visitClassTypeAlias`.
                if !privado(x.name.sym) {
                    let sob = sobrescreve(classe, x.name.sym, false);
                    checar(doc(), sob, x.name.span, &mut achados);
                }
                continue;
            }
            DeclKind::Class(x) => {
                if interno(&d.metadata) || privado(x.name.sym) {
                    continue;
                }
                let sob = sobrescreve(classe, x.name.sym, false);
                checar(doc(), sob, x.name.span, &mut achados);
                metodos(&x.members, classe, &mut achados);
            }
            DeclKind::Mixin(x) => {
                if interno(&d.metadata) || privado(x.name.sym) {
                    continue;
                }
                let sob = sobrescreve(classe, x.name.sym, false);
                checar(doc(), sob, x.name.span, &mut achados);
                metodos(&x.members, classe, &mut achados);
            }
            DeclKind::Enum(x) => {
                if privado(x.name.sym) || interno(&d.metadata) {
                    continue;
                }
                let sob = sobrescreve(classe, x.name.sym, false);
                checar(doc(), sob, x.name.span, &mut achados);
                metodos(&x.members, classe, &mut achados);
            }
            DeclKind::Extension(x) => {
                let Some(n) = x.name else { continue };
                if privado(n.sym) || interno(&d.metadata) {
                    continue;
                }
                checar(doc(), false, n.span, &mut achados);
                metodos(&x.members, None, &mut achados);
            }
            DeclKind::ExtensionType(x) => {
                if interno(&d.metadata) || privado(x.name.sym) {
                    continue;
                }
                let sob = sobrescreve(classe, x.name.sym, false);
                checar(doc(), sob, x.name.span, &mut achados);
                metodos(&x.members, classe, &mut achados);
            }
            DeclKind::Typedef(x) => {
                if !privado(x.name.sym) {
                    checar(doc(), false, x.name.span, &mut achados);
                }
                continue;
            }
            DeclKind::Variables(l) => {
                // `visitTopLevelVariableDeclaration`: o comentário é o da
                // declaração.
                for v in l.variables.iter() {
                    if !privado(v.name.sym) {
                        checar(doc(), false, v.name.span, &mut achados);
                    }
                }
                continue;
            }
            DeclKind::Function(_) => continue,
        }
    }

    // `visitConstructorDeclaration`, `visitFieldDeclaration` e
    // `visitEnumConstantDeclaration`, pelo pai.
    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        let classe = classe_da_decl(id);
        // `inPrivateMember` do pai, e o `isInternal` da declaração de topo.
        let (nome_do_pai, membros, e_enum, e_tipo_de_extensao): (Option<SymbolId>, &[MemberId], bool, bool) = match &d.kind {
            DeclKind::Class(x) => (Some(x.name.sym), &x.members, false, false),
            DeclKind::Mixin(x) => (Some(x.name.sym), &x.members, false, false),
            DeclKind::Enum(x) => (Some(x.name.sym), &x.members, true, false),
            DeclKind::Extension(x) => (x.name.map(|n| n.sym), &x.members, false, false),
            DeclKind::ExtensionType(x) => (Some(x.name.sym), &x.members, false, true),
            _ => continue,
        };
        let pai_privado = nome_do_pai.is_none_or(privado);
        let pai_interno = interno(&d.metadata);
        // `isEffectivelyPrivate` do pai (para os construtores).
        let pai_efetivamente_privado = pai_interno
            || match &d.kind {
                DeclKind::Class(x) => {
                    let m = &x.modifiers;
                    m.sealed || (m.abstract_ && (m.final_ || m.interface))
                }
                _ => false,
            };
        if let DeclKind::Enum(x) = &d.kind
            && !pai_interno
            && !pai_privado
        {
            for k in x.constants.iter() {
                if privado(k.name.sym) {
                    continue;
                }
                let doc = documentado(&k.metadata, k.name.span.start);
                let sob = sobrescreve(classe, k.name.sym, false);
                checar(doc, sob, k.name.span, &mut achados);
            }
        }
        for &m in membros {
            let membro = a.member(m);
            match &membro.kind {
                MemberKind::Constructor(kc) => {
                    if pai_privado || kc.name.is_some_and(|n| privado(n.sym)) || e_enum || pai_efetivamente_privado {
                        continue;
                    }
                    let no = kc.name.map_or(kc.class_name.span, |n| n.span);
                    let nome = match kc.name {
                        Some(n) => Some(n.sym),
                        None => interner.lookup(""),
                    };
                    let sob = nome.is_some_and(|n| sobrescreve(classe, n, false));
                    checar(documentado(&membro.metadata, pos_do_membro(m)), sob, no, &mut achados);
                }
                MemberKind::Field(l) => {
                    if pai_interno || pai_privado || (e_tipo_de_extensao && !l.static_) {
                        continue;
                    }
                    let doc = documentado(&membro.metadata, pos_do_membro(m));
                    for v in l.variables.iter() {
                        if privado(v.name.sym) {
                            continue;
                        }
                        let sob = sobrescreve(classe, v.name.sym, false);
                        checar(doc, sob, v.name.span, &mut achados);
                    }
                }
                _ => {}
            }
        }
    }

    achados.sort_by_key(|x| (x.start, x.end));
    achados.dedup();
    for sp in achados {
        relatar(&mut out, &c::PUBLIC_MEMBER_API_DOCS, sp);
    }
    out
}

fn relatar(out: &mut Vec<RelatoDeLint>, codigo: &'static CodigoLint, span: Span) {
    out.push(RelatoDeLint { codigo, span, args: Vec::new() });
}
