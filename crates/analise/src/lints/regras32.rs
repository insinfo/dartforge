//! O trigésimo segundo lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `unsafe_html`: a atribuição a `href` (de `AnchorElement`), `src` (de
//!   `EmbedElement`, `IFrameElement`, `ScriptElement`) e `srcdoc` (de
//!   `IFrameElement`), a chamada de `createFragment`/`setInnerHtml` (de
//!   `Element`) e de `open` (de `Window`), com o alvo `dynamic` também, e o
//!   construtor `html` de `DocumentFragment`/`Element`, pela cadeia de
//!   superclasses (`extendsClass(…, 'dart.dom.html')`). Os códigos são os
//!   `SecurityLintCode` do próprio emissor (mensagens diferentes das do
//!   `messages.yaml`).
//! * `analyzer_use_new_elements` (interna): só no pacote com o
//!   `analyzer_use_new_elements.txt` na raiz e na biblioteca cujo arquivo
//!   começa por um dos prefixos dele; o tipo nomeado cujo elemento é uma
//!   declaração do `package:analyzer/dart/element/element.dart` (menos
//!   `DirectiveUri`, `DirectiveUriWithRelativeUri`, `ElementAnnotation` e
//!   `ElementKind`), e o identificador (fora do nome de método invocado) ou
//!   a chamada de método cujo tipo estático contém um tipo de interface
//!   desses.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::regras::RelatoDeLint;
use super::regras16::e_invocacao_de_metodo;
use super::CodigoLint;
use crate::Unidade;
use dartforge_elements::model::{ClassId, ClassKind};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId};

/// `_Visitor.unsafeAttributeCode`.
pub static UNSAFE_HTML_ATTRIBUTE: CodigoLint = CodigoLint {
    nome: "unsafe_html",
    unico: "unsafe_html_attribute",
    mensagem: "Avoid unsafe HTML APIs (assigning \"{0}\" attribute).",
    correcao: None,
    documentado: false,
};

/// `_Visitor.unsafeMethodCode`.
pub static UNSAFE_HTML_METHOD: CodigoLint = CodigoLint {
    nome: "unsafe_html",
    unico: "unsafe_html_method",
    mensagem: "Avoid unsafe HTML APIs (calling the '{0}' method of {1}).",
    correcao: None,
    documentado: false,
};

/// `_Visitor.unsafeConstructorCode`.
pub static UNSAFE_HTML_CONSTRUCTOR: CodigoLint = CodigoLint {
    nome: "unsafe_html",
    unico: "unsafe_html_constructor",
    mensagem: "Avoid unsafe HTML APIs (calling the '{0}' constructor of {1}).",
    correcao: None,
    documentado: false,
};

/// `AnalyzerUseNewElements.code`.
pub static ANALYZER_USE_NEW_ELEMENTS: CodigoLint = CodigoLint {
    nome: "analyzer_use_new_elements",
    unico: "analyzer_use_new_elements",
    mensagem: "This code uses the old analyzer element model.",
    correcao: Some("Try using the new elements."),
    documentado: false,
};

/// O tipo de um alvo: `Some(None)` para `dynamic`, `Some(Some(classe))` para
/// um tipo de interface.
#[derive(Clone, Copy)]
enum Alvo {
    Dinamico,
    Classe(ClassId),
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    if ligada("analyzer_use_new_elements") {
        out.extend(novos_elementos(u, interner, s));
    }
    if !ligada("unsafe_html") {
        return out;
    }
    let a = u.ast;
    let program = s.program;
    let table = s.table;
    let corpo = s.corpo;
    // `extendsDartHtmlClass`: a classe ou uma superclasse é `nome` da
    // biblioteca `dart.dom.html`.
    let nome_da_biblioteca = |c: ClassId| -> Option<String> {
        program.library(program.class(c).library).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
    };
    let estende = |alvo: Alvo, nome: &str| -> bool {
        let Alvo::Classe(mut c) = alvo else { return false };
        let mut vistos: Vec<ClassId> = Vec::new();
        loop {
            if vistos.contains(&c) {
                return false;
            }
            vistos.push(c);
            if interner.resolve(program.class(c).name) == nome && nome_da_biblioteca(c).as_deref() == Some("dart.dom.html") {
                return true;
            }
            match program.class(c).supertype_class {
                Some(x) => c = x,
                None => return false,
            }
        }
    };
    let dinamico_ou = |alvo: Alvo, nome: &str| matches!(alvo, Alvo::Dinamico) || estende(alvo, nome);
    let de_tipo = |t: TypeId| -> Option<Alvo> {
        match table.get(t) {
            Type::Dynamic => Some(Alvo::Dinamico),
            Type::Interface { class, .. } => Some(Alvo::Classe(*class)),
            _ => None,
        }
    };
    // As cascatas: o `realTarget` de uma seção é o alvo da cascata.
    let mut cascatas: std::collections::HashMap<ExprId, ExprId> = std::collections::HashMap::new();
    for e in a.exprs.iter() {
        if let ExprKind::Cascade { target, sections, .. } = &e.kind {
            for sec in sections.iter() {
                let mut x = *sec;
                loop {
                    match &a.expr(x).kind {
                        ExprKind::CascadeTarget => {
                            cascatas.insert(x, *target);
                            break;
                        }
                        ExprKind::Property { target, .. } | ExprKind::Index { target, .. } | ExprKind::Call { target, .. } | ExprKind::TypeArguments { target, .. } => x = *target,
                        ExprKind::Assign { target, .. } => x = *target,
                        _ => break,
                    }
                }
            }
        }
    }
    let tipo_real = |x: ExprId| -> Option<Alvo> {
        let x = cascatas.get(&x).copied().unwrap_or(x);
        corpo.get_type(x).and_then(de_tipo)
    };
    // O `thisType` da classe que contém o membro resolvido (só a `class`).
    let da_classe_do_membro = |e: ExprId| -> Option<Alvo> {
        match corpo.get_resolved(e)? {
            Resolved::Member { class, .. } if matches!(program.class(*class).kind, ClassKind::Class | ClassKind::MixinApplication) => Some(Alvo::Classe(*class)),
            _ => None,
        }
    };
    let mut achados: Vec<(dartforge_diagnostics::Span, &'static CodigoLint, Vec<String>)> = Vec::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        match &e.kind {
            // `visitAssignmentExpression`.
            ExprKind::Assign { target, .. } => {
                let mut esq = *target;
                while let ExprKind::Parenthesized(x) = &a.expr(esq).kind {
                    esq = *x;
                }
                let (tipo, nome) = match &a.expr(esq).kind {
                    ExprKind::Identifier(n) => (da_classe_do_membro(esq), n.sym),
                    ExprKind::Property { target: t, name, .. } => (tipo_real(*t), name.sym),
                    _ => continue,
                };
                let Some(tipo) = tipo else { continue };
                let n = interner.resolve(nome);
                let inseguro = match n {
                    "href" => dinamico_ou(tipo, "AnchorElement"),
                    "src" => matches!(tipo, Alvo::Dinamico) || estende(tipo, "EmbedElement") || estende(tipo, "IFrameElement") || estende(tipo, "ScriptElement"),
                    "srcdoc" => dinamico_ou(tipo, "IFrameElement"),
                    _ => false,
                };
                if inseguro {
                    achados.push((e.span, &UNSAFE_HTML_ATTRIBUTE, vec![n.to_string()]));
                }
            }
            // `visitInstanceCreationExpression` (com `new` ou sem).
            ExprKind::InstanceCreation { constructor: Some(c), .. } if interner.resolve(c.sym) == "html" => {
                let Some(tipo) = corpo.get_type(id).and_then(de_tipo) else { continue };
                if estende(tipo, "DocumentFragment") {
                    achados.push((e.span, &UNSAFE_HTML_CONSTRUCTOR, vec!["html".to_string(), "DocumentFragment".to_string()]));
                } else if estende(tipo, "Element") {
                    achados.push((e.span, &UNSAFE_HTML_CONSTRUCTOR, vec!["html".to_string(), "Element".to_string()]));
                }
            }
            ExprKind::Call { target, .. } => {
                if let Some(Resolved::Constructor(f)) = corpo.get_resolved(id) {
                    if interner.resolve(program.function(*f).name) != "html" {
                        continue;
                    }
                    let Some(tipo) = corpo.get_type(id).and_then(de_tipo) else { continue };
                    if estende(tipo, "DocumentFragment") {
                        achados.push((e.span, &UNSAFE_HTML_CONSTRUCTOR, vec!["html".to_string(), "DocumentFragment".to_string()]));
                    } else if estende(tipo, "Element") {
                        achados.push((e.span, &UNSAFE_HTML_CONSTRUCTOR, vec!["html".to_string(), "Element".to_string()]));
                    }
                    continue;
                }
                // `visitMethodInvocation`.
                if !e_invocacao_de_metodo(s, a, *target) {
                    continue;
                }
                let (tipo, nome) = match &a.expr(*target).kind {
                    ExprKind::Identifier(n) => (da_classe_do_membro(*target), n.sym),
                    ExprKind::Property { target: t, name, .. } => (tipo_real(*t), name.sym),
                    _ => continue,
                };
                let Some(tipo) = tipo else { continue };
                let n = interner.resolve(nome);
                let (inseguro, de) = match n {
                    "createFragment" | "setInnerHtml" => (dinamico_ou(tipo, "Element"), "Element"),
                    "open" => (dinamico_ou(tipo, "Window"), "Window"),
                    _ => (false, ""),
                };
                if inseguro {
                    achados.push((e.span, &UNSAFE_HTML_METHOD, vec![n.to_string(), de.to_string()]));
                }
            }
            _ => {}
        }
    }
    for (span, codigo, args) in achados {
        out.push(RelatoDeLint { codigo, span, args });
    }
    out
}

/// `_isOldModelElement`: a classe declarada na unidade
/// `package:analyzer/dart/element/element.dart`, fora das que não migram.
fn modelo_antigo(s: &super::Semantica<'_>, interner: &Interner, c: ClassId) -> bool {
    let k = s.program.class(c);
    let Some(r) = k.decl else { return false };
    if s.program.unit(r.unit).uri != "package:analyzer/dart/element/element.dart" {
        return false;
    }
    !matches!(interner.resolve(k.name), "DirectiveUri" | "DirectiveUriWithRelativeUri" | "ElementAnnotation" | "ElementKind")
}

/// `_isOldModelType`: o `RecursiveTypeVisitor` pelos tipos de interface.
fn tipo_antigo(s: &super::Semantica<'_>, interner: &Interner, t: TypeId, prof: u32) -> bool {
    if prof > 32 {
        return false;
    }
    let r = |x: TypeId| tipo_antigo(s, interner, x, prof + 1);
    match s.table.get(t) {
        Type::Interface { class, args, .. } => modelo_antigo(s, interner, *class) || args.iter().any(|x| r(*x)),
        Type::ExtensionType { args, .. } => args.iter().any(|x| r(*x)),
        Type::FutureOr { arg, .. } => r(*arg),
        Type::Function { ret, positional, optional, named, .. } => r(*ret) || positional.iter().chain(optional.iter()).any(|x| r(*x)) || named.iter().any(|(_, x, _)| r(*x)),
        Type::Record { positional, named, .. } => positional.iter().any(|x| r(*x)) || named.iter().any(|(_, x)| r(*x)),
        _ => false,
    }
}

/// `analyzer_use_new_elements`.
fn novos_elementos(u: Unidade<'_>, interner: &Interner, s: &super::Semantica<'_>) -> Vec<RelatoDeLint> {
    let mut out = Vec::new();
    let program = s.program;
    // `_isEnabledForFile`: o pacote pub, o arquivo de opção e os prefixos.
    let lib = program.library(program.unit(s.unidade).library);
    let Some(arquivo) = lib.units.first().and_then(|x| program.unit(*x).path.clone()) else { return out };
    let Some(raiz) = arquivo.ancestors().skip(1).find(|d| d.join("pubspec.yaml").is_file()).map(std::path::Path::to_path_buf) else { return out };
    let Ok(texto) = std::fs::read_to_string(raiz.join("analyzer_use_new_elements.txt")) else { return out };
    let separador = std::path::MAIN_SEPARATOR.to_string();
    let prefixos: Vec<String> = texto.trim().split('\n').map(|l| l.trim().replace('/', &separador)).collect();
    let caminho = arquivo.to_string_lossy().to_string();
    let base = raiz.to_string_lossy().to_string();
    if !caminho.starts_with(&base) || caminho.len() <= base.len() {
        return out;
    }
    let relativo = &caminho[base.len() + 1..];
    if !prefixos.iter().any(|p| relativo.starts_with(p.as_str())) {
        return out;
    }
    let a = u.ast;
    // `visitNamedType`.
    for t in a.types.iter() {
        let dartforge_frontend::ast::TypeKind::Named { name, .. } = &t.kind else { continue };
        let b = match &name[..] {
            [n] => program.lookup_na_unidade(s.unidade, n.sym),
            [p, n] => program.lookup_prefixed_na_unidade(s.unidade, p.sym, n.sym),
            _ => None,
        };
        if let Some(dartforge_elements::model::Element::Class(c)) = b.and_then(|b| b.getter)
            && modelo_antigo(s, interner, c)
            && let Some(ultimo) = name.last()
        {
            out.push(RelatoDeLint { codigo: &ANALYZER_USE_NEW_ELEMENTS, span: ultimo.span, args: Vec::new() });
        }
    }
    // Os nomes de método invocados: só pelo `visitMethodInvocation`.
    let mut nomes_de_metodo: std::collections::HashSet<ExprId> = std::collections::HashSet::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let ExprKind::Call { target, .. } = &e.kind else { continue };
        if matches!(s.corpo.get_resolved(ExprId(k as u32)), Some(Resolved::Constructor(_))) || !e_invocacao_de_metodo(s, a, *target) {
            continue;
        }
        nomes_de_metodo.insert(*target);
        if s.corpo.get_type(ExprId(k as u32)).is_some_and(|t| tipo_antigo(s, interner, t, 0)) {
            let sp = match &a.expr(*target).kind {
                ExprKind::Property { name, .. } => name.span,
                _ => a.expr(*target).span,
            };
            out.push(RelatoDeLint { codigo: &ANALYZER_USE_NEW_ELEMENTS, span: sp, args: Vec::new() });
        }
    }
    // `visitSimpleIdentifier`: o identificador e o nome de propriedade.
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        if nomes_de_metodo.contains(&id) {
            continue;
        }
        let sp = match &e.kind {
            ExprKind::Identifier(n) => n.span,
            ExprKind::Property { name, .. } => name.span,
            _ => continue,
        };
        if s.corpo.get_type(id).is_some_and(|t| tipo_antigo(s, interner, t, 0)) {
            out.push(RelatoDeLint { codigo: &ANALYZER_USE_NEW_ELEMENTS, span: sp, args: Vec::new() });
        }
    }
    out.sort_by_key(|r| (r.span.start, r.span.end));
    out
}
