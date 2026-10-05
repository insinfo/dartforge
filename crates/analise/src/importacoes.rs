//! Imports não usados: o `ImportsVerifier` do analyzer 6.11.0
//! (`src/error/imports_verifier.dart`) — `UNUSED_IMPORT` e
//! `UNUSED_SHOWN_NAME` — com a supressão do `LibraryAnalyzer`
//! (`_hasDiagnosticReportedThatPreventsImportWarnings`: com um nome não
//! resolvido na biblioteca, nenhum aviso de import sai).
//!
//! O analyzer marca um import como usado quando um identificador resolvido
//! aponta para um elemento que ele fornece (`ImportsTracking`). Aqui, sem a
//! resolução completa, a aproximação é **pelo lado seguro** — pode deixar de
//! relatar, nunca relatar a mais:
//! * um nome citado na biblioteca (identificador, tipo anotado, anotação,
//!   referência de comentário `[Nome]`) que não é declarado no topo dela usa
//!   todo import (sem prefixo) cujo namespace o contém — locais e membros que
//!   o escondam não são descontados;
//! * `p.Nome` usa os imports de prefixo `p` que contêm `Nome`;
//! * um import que traz alguma extension conta como usado (o uso implícito de
//!   um membro de extensão precisa de tipos).

use dartforge_diagnostics::codigos::{compile_time_error as c, warning as w};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{Element, LibraryId, Program, UnitRole};
use dartforge_frontend::ast::{Combinator, DirectiveKind, ExprKind, PatternKind, TypeKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashSet;

/// Códigos que, relatados na biblioteca, suprimem os avisos de import.
fn suprime(d: &Diagnostic) -> bool {
    let Some(codigo) = d.code else { return false };
    [
        c::AMBIGUOUS_IMPORT,
        c::CONST_WITH_NON_TYPE,
        c::EXTENDS_NON_CLASS,
        c::IMPLEMENTS_NON_CLASS,
        c::MIXIN_OF_NON_CLASS,
        c::NEW_WITH_NON_TYPE,
        c::NOT_A_TYPE,
        c::PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT,
        c::UNDEFINED_ANNOTATION,
        c::UNDEFINED_CLASS,
        c::UNDEFINED_FUNCTION,
        c::UNDEFINED_IDENTIFIER,
        c::UNDEFINED_PREFIXED_NAME,
        w::DEPRECATED_EXPORT_USE,
    ]
    .contains(&codigo)
}

/// `DEFERRED_IMPORT_OF_EXTENSION` (`ErrorVerifier._checkForDeferredImportOfExtensions`,
/// `analyzer/lib/src/generated/error_verifier.dart:3053-3066`;
/// docs/ANALYZER-ESPECIFICACAO.md §A): um import `deferred` cujo namespace,
/// depois de `show`/`hide`, ainda traz alguma extensão. Na URI do import.
/// Escrito sem compilar nem executar (2026-10-04).
pub fn extensoes_adiadas(program: &Program, lib: LibraryId) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    let mut out = Vec::new();
    for imp in &program.library(lib).imports {
        if !imp.deferred {
            continue;
        }
        let visivel = |nome: SymbolId| -> bool {
            imp.combinators.iter().all(|k| match k {
                Combinator::Show(nomes) => nomes.iter().any(|n| n.sym == nome),
                Combinator::Hide(nomes) => !nomes.iter().any(|n| n.sym == nome),
            })
        };
        let tem_extensao = program
            .library(imp.library)
            .exported
            .iter()
            .any(|(nome, b)| matches!(b.getter, Some(Element::Extension(_))) && visivel(*nome));
        if !tem_extensao {
            continue;
        }
        let Some(dir) = program.unit(imp.unit).unit.directives.get(imp.directive) else { continue };
        let DirectiveKind::Import { uri, .. } = &dir.kind else { continue };
        out.push((imp.unit, Diagnostic::com_codigo(c::DEFERRED_IMPORT_OF_EXTENSION, uri.span, std::iter::empty::<&str>())));
    }
    out
}

/// `UNDEFINED_SHOWN_NAME` (`DeadCodeVerifier._checkCombinator`,
/// `analyzer/lib/src/error/dead_code_verifier.dart:131-155`;
/// docs/ANALYZER-ESPECIFICACAO.md §F): um nome de `show`, em `import` ou
/// `export`, que a biblioteca alvo não exporta (nem como getter nem como
/// setter). No identificador do combinador; não depende de uso nem da
/// supressão dos imports. Escrito sem compilar nem executar (2026-10-04).
pub fn nomes_mostrados_indefinidos(program: &Program, lib: LibraryId, interner: &Interner) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    let mut out = Vec::new();
    let biblioteca = program.library(lib);
    let diretivas = biblioteca
        .imports
        .iter()
        .map(|i| (i.unit, i.library, &i.combinators))
        .chain(biblioteca.exports.iter().map(|e| (e.unit, e.library, &e.combinators)));
    for (unidade, alvo, combinadores) in diretivas {
        let exportado = &program.library(alvo).exported;
        let uri = program.library(alvo).uri.as_str();
        for k in combinadores.iter() {
            let Combinator::Show(nomes) = k else { continue };
            for n in nomes.iter() {
                if !exportado.contains_key(&n.sym) {
                    out.push((unidade, Diagnostic::com_codigo(w::UNDEFINED_SHOWN_NAME, n.span, [uri, interner.resolve(n.sym)])));
                }
            }
        }
    }
    out
}

/// A biblioteca que declara um elemento de topo.
fn biblioteca_do_elemento(program: &Program, e: Element) -> Option<LibraryId> {
    Some(match e {
        Element::Class(c) => program.class(c).library,
        Element::Extension(x) => program.extension(x).library,
        Element::Typedef(t) => program.typedef(t).library,
        Element::Function(f) => program.function(f).library,
        Element::Variable(v) => program.variable(v).library,
        Element::Prefix(..) => return None,
    })
}

/// `AMBIGUOUS_EXPORT` (`ErrorVerifier._checkForAmbiguousExport`,
/// `analyzer/lib/src/generated/error_verifier.dart:2085-2111`): os exports em
/// ordem, com um mapa compartilhado nome → elemento; o primeiro nome de uma
/// diretiva que já veio de outra com OUTRO elemento é relatado na URI dela,
/// e o resto da diretiva não entra. Escrito sem compilar nem executar.
pub fn exports_ambiguos(program: &Program, lib: LibraryId, interner: &Interner) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    let mut out = Vec::new();
    // (nome, é a vaga do setter) → elemento.
    let mut vistos: std::collections::HashMap<(SymbolId, bool), Element> = std::collections::HashMap::new();
    for exp in &program.library(lib).exports {
        let visivel = |nome: SymbolId| -> bool {
            exp.combinators.iter().all(|k| match k {
                Combinator::Show(nomes) => nomes.iter().any(|n| n.sym == nome),
                Combinator::Hide(nomes) => !nomes.iter().any(|n| n.sym == nome),
            })
        };
        // A ordem dos nomes de um namespace não é a da fonte: pela ordem dos
        // textos, para o relato ser estável.
        let mut entradas: Vec<((SymbolId, bool), Element)> = Vec::new();
        for (nome, b) in program.library(exp.library).exported.iter() {
            if !visivel(*nome) {
                continue;
            }
            if let Some(e) = b.getter {
                entradas.push(((*nome, false), e));
            }
            if let Some(e) = b.setter {
                entradas.push(((*nome, true), e));
            }
        }
        entradas.sort_by(|x, y| interner.resolve(x.0 .0).cmp(interner.resolve(y.0 .0)).then(x.0 .1.cmp(&y.0 .1)));
        let mut conflito: Option<(SymbolId, Element, Element)> = None;
        let mut novos: Vec<((SymbolId, bool), Element)> = Vec::new();
        for (chave, e) in entradas {
            match vistos.get(&chave) {
                Some(anterior) if *anterior != e => {
                    conflito = Some((chave.0, *anterior, e));
                    break;
                }
                Some(_) => {}
                None => novos.push((chave, e)),
            }
        }
        for (chave, e) in novos {
            vistos.insert(chave, e);
        }
        let Some((nome, anterior, atual)) = conflito else { continue };
        let Some(dir) = program.unit(exp.unit).unit.directives.get(exp.directive) else { continue };
        let DirectiveKind::Export { uri, .. } = &dir.kind else { continue };
        let uri_de = |e: Element| -> String { biblioteca_do_elemento(program, e).map(|l| program.library(l).uri.clone()).unwrap_or_default() };
        let (um, outro) = (uri_de(anterior), uri_de(atual));
        out.push((exp.unit, Diagnostic::com_codigo(c::AMBIGUOUS_EXPORT, uri.span, [interner.resolve(nome), um.as_str(), outro.as_str()])));
    }
    out
}

/// Os avisos de import da biblioteca `lib`, com a unidade de cada um.
/// `ja_relatados`: os diagnósticos da biblioteca até aqui (para a supressão).
pub fn nao_usados(
    program: &Program,
    lib: LibraryId,
    interner: &Interner,
    ja_relatados: &[Diagnostic],
) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    if ja_relatados.iter().any(suprime) {
        return Vec::new();
    }
    let biblioteca = program.library(lib);
    // Nomes citados, sem prefixo e com prefixo.
    let mut soltos: HashSet<SymbolId> = HashSet::new();
    let mut prefixados: HashSet<(SymbolId, SymbolId)> = HashSet::new();
    let mut prefixo_em_comentario: HashSet<String> = HashSet::new();
    for &u in &biblioteca.units {
        let unit = program.unit(u);
        if unit.role == UnitRole::Patch {
            continue;
        }
        let ast = &unit.ast;
        for e in &ast.exprs {
            match &e.kind {
                ExprKind::Identifier(n) => {
                    soltos.insert(n.sym);
                }
                ExprKind::Property { target, name, .. } => {
                    if let ExprKind::Identifier(p) = &ast.expr(*target).kind {
                        prefixados.insert((p.sym, name.sym));
                    }
                }
                _ => {}
            }
        }
        // `case nome:`: num padrão refutável, o identificador solto é uma
        // constante (o parser o guarda como variável sem tipo). Contá-lo
        // sempre como citado é pelo lado seguro.
        for p in &ast.patterns {
            if let PatternKind::Variable { name, ty: None, final_: false, var_: false } = &p.kind {
                soltos.insert(name.sym);
            }
        }
        for t in &ast.types {
            if let TypeKind::Named { name, .. } = &t.kind {
                match name.len() {
                    1 => {
                        soltos.insert(name[0].sym);
                    }
                    2 => {
                        prefixados.insert((name[0].sym, name[1].sym));
                        // `a.B` também pode ser `a` sem prefixo (tipo aninhado não existe
                        // em Dart, mas a recuperação pode produzir): conta os dois.
                        soltos.insert(name[0].sym);
                    }
                    _ => {}
                }
            }
        }
        for d in &ast.decls {
            anotacoes(&d.metadata, &mut soltos, &mut prefixados);
        }
        for m in &ast.members {
            anotacoes(&m.metadata, &mut soltos, &mut prefixados);
            if let dartforge_frontend::ast::MemberKind::Constructor(k) = &m.kind {
                parametros(&k.parameters, &mut soltos, &mut prefixados);
            }
        }
        for f in &ast.functions {
            if let Some(ps) = &f.parameters {
                parametros(ps, &mut soltos, &mut prefixados);
            }
        }
        for d in &unit.unit.directives {
            anotacoes(&d.metadata, &mut soltos, &mut prefixados);
        }
        // Referências de comentário de documentação (`[Nome]`, `[p.Nome]`).
        referencias_de_comentario(&unit.source, interner, &mut soltos, &mut prefixados, &mut prefixo_em_comentario);
    }
    // Declarados no topo escondem os importados.
    let declarados: HashSet<SymbolId> = biblioteca.declared.keys().copied().collect();

    let mut out = Vec::new();
    // Os imports usados: unidade, prefixo, URI (posição e texto), elementos
    // citados e extensões trazidas.
    #[allow(clippy::type_complexity)]
    let mut usados: Vec<(dartforge_elements::model::UnitId, Option<SymbolId>, dartforge_diagnostics::Span, String, Vec<Element>, Vec<Element>)> =
        Vec::new();
    for imp in &biblioteca.imports {
        let alvo = program.library(imp.library);
        let unit = program.unit(imp.unit);
        let Some(dir) = unit.unit.directives.get(imp.directive) else { continue };
        let DirectiveKind::Import { uri, combinators, .. } = &dir.kind else { continue };
        let Some(texto) = dartforge_elements::load::string_lit_value(uri) else { continue };
        // `dart:core` explícito e alvo que não existe (relatado em outro lugar).
        if alvo.uri == "dart:core" || alvo.units.is_empty() {
            continue;
        }
        if let Some(p) = imp.prefix {
            if prefixo_em_comentario.contains(interner.resolve(p)) {
                continue;
            }
        }
        let visivel = |nome: SymbolId| -> bool {
            if !alvo.exported.contains_key(&nome) {
                return false;
            }
            combinators.iter().all(|c| match c {
                Combinator::Show(ns) => ns.iter().any(|n| n.sym == nome),
                Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == nome),
            })
        };
        let traz_extensao = alvo.exported.iter().any(|(k, b)| {
            visivel(*k) && matches!(b.getter, Some(Element::Extension(_)))
        });
        let usado = traz_extensao
            || match imp.prefix {
                None => soltos.iter().any(|n| !declarados.contains(n) && visivel(*n)),
                // `p` sozinho também usa o prefixo (`prefix_identifier_not_followed_by_dot`).
                Some(p) => soltos.contains(&p) || prefixados.iter().any(|(q, n)| *q == p && visivel(*n)),
            };
        if !usado {
            out.push((imp.unit, Diagnostic::com_codigo(w::UNUSED_IMPORT, uri.span, [texto.as_str()])));
            continue;
        }
        // Os "elementos usados" deste import, para o `unnecessary_import`:
        // os citados pelo nome e, à parte, as extensões que ele traz (o uso
        // de uma extensão é implícito e não se vê pelo nome).
        let mut nomeados: Vec<Element> = Vec::new();
        let mut extensoes: Vec<Element> = Vec::new();
        for (k, b) in alvo.exported.iter() {
            if !visivel(*k) {
                continue;
            }
            if let Some(x @ Element::Extension(_)) = b.getter {
                extensoes.push(x);
                continue;
            }
            let citado = match imp.prefix {
                None => soltos.contains(k) && !declarados.contains(k),
                Some(p) => prefixados.contains(&(p, *k)),
            };
            if citado {
                nomeados.extend(b.getter);
                nomeados.extend(b.setter);
            }
        }
        usados.push((imp.unit, imp.prefix, uri.span, texto.clone(), nomeados, extensoes));
        // `UNUSED_SHOWN_NAME`, só em import usado.
        for comb in combinators.iter() {
            if let Combinator::Show(ns) = comb {
                for n in ns.iter() {
                    // Extension mostrada: usada implicitamente (precisa de tipos).
                    match alvo.exported.get(&n.sym) {
                        None => continue,
                        Some(b) if matches!(b.getter, Some(Element::Extension(_))) => continue,
                        Some(_) => {}
                    }
                    let citado = match imp.prefix {
                        None => soltos.contains(&n.sym),
                        Some(p) => prefixados.contains(&(p, n.sym)),
                    };
                    if !citado {
                        let nome = interner.resolve(n.sym);
                        out.push((imp.unit, Diagnostic::com_codigo(w::UNUSED_SHOWN_NAME, n.span, [nome])));
                    }
                }
            }
        }
    }
    // `generateUnnecessaryImportHints` (`imports_verifier.dart:173-229`): um
    // import usado é desnecessário se outro import usado do mesmo arquivo e
    // do mesmo prefixo provê tudo o que ele provê e estritamente mais. O
    // "estritamente mais" é medido só nos elementos citados pelo nome: com
    // os citados iguais, a diferença estaria em extensões de uso incerto.
    for (i, primeiro) in usados.iter().enumerate() {
        for (j, segundo) in usados.iter().enumerate() {
            if i == j || primeiro.0 != segundo.0 || primeiro.1 != segundo.1 {
                continue;
            }
            let contem_tudo = primeiro.4.iter().all(|e| segundo.4.contains(e)) && primeiro.5.iter().all(|e| segundo.5.contains(e));
            let distintos = |v: &[Element]| v.iter().enumerate().filter(|(k, e)| !v[..*k].contains(*e)).count();
            if contem_tudo && distintos(&segundo.4) > distintos(&primeiro.4) {
                out.push((
                    primeiro.0,
                    Diagnostic::com_codigo(dartforge_diagnostics::codigos::hint::UNNECESSARY_IMPORT, primeiro.2, [primeiro.3.as_str(), segundo.3.as_str()]),
                ));
                break;
            }
        }
    }
    out
}

fn parametros(
    ps: &[dartforge_frontend::ast::Parameter],
    soltos: &mut HashSet<SymbolId>,
    prefixados: &mut HashSet<(SymbolId, SymbolId)>,
) {
    for p in ps {
        anotacoes(&p.metadata, soltos, prefixados);
        if let Some(fp) = &p.function_parameters {
            parametros(fp, soltos, prefixados);
        }
    }
}

fn anotacoes(
    metadata: &[dartforge_frontend::ast::Annotation],
    soltos: &mut HashSet<SymbolId>,
    prefixados: &mut HashSet<(SymbolId, SymbolId)>,
) {
    for a in metadata {
        if let Some(primeiro) = a.name.first() {
            soltos.insert(primeiro.sym);
        }
        if a.name.len() >= 2 {
            prefixados.insert((a.name[0].sym, a.name[1].sym));
        }
    }
}

/// `[Nome]` e `[p.Nome]` em comentários `///` e `/** */` (o lexer descarta
/// comentários; o analyzer resolve essas referências e elas contam).
fn referencias_de_comentario(
    fonte: &str,
    interner: &Interner,
    soltos: &mut HashSet<SymbolId>,
    prefixados: &mut HashSet<(SymbolId, SymbolId)>,
    prefixo_em_comentario: &mut HashSet<String>,
) {
    for linha in fonte.lines() {
        let t = linha.trim_start();
        if !(t.starts_with("///") || t.starts_with('*') || t.starts_with("/**")) {
            continue;
        }
        let mut resto = t;
        while let Some(i) = resto.find('[') {
            let depois = &resto[i + 1..];
            let Some(j) = depois.find(']') else { break };
            let dentro = depois[..j].trim();
            let partes: Vec<&str> = dentro.split('.').collect();
            let ok = !partes.is_empty()
                && partes.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$'));
            if ok {
                if let Some(s) = interner.lookup(partes[0]) {
                    soltos.insert(s);
                }
                prefixo_em_comentario.insert(partes[0].to_string());
                if partes.len() >= 2 {
                    if let (Some(p), Some(n)) = (interner.lookup(partes[0]), interner.lookup(partes[1])) {
                        prefixados.insert((p, n));
                    }
                }
            }
            resto = &depois[j + 1..];
        }
    }
}
