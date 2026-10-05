//! Imports não usados: o `ImportsVerifier` do analyzer 6.11.0
//! (`src/error/imports_verifier.dart`) — `UNUSED_IMPORT` e
//! `UNUSED_SHOWN_NAME` — com a supressão do `LibraryAnalyzer`
//! (`_hasDiagnosticReportedThatPreventsImportWarnings`: com um nome não
//! resolvido na biblioteca, nenhum aviso de import sai).
//!
//! O uso é o rastreio do analyzer (`ImportsTrackingOfPrefix`), feito sobre
//! as buscas léxicas da unidade com os escopos locais e de membro de
//! `dartforge_types::anotacoes::sombreado`, e as extensões escolhidas na
//! resolução de membros (`BodyTypes::extensoes_usadas`); ver [`nao_usados`].

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

/// Um import da biblioteca, com o que o verificador lê dele.
struct Imp<'a> {
    unit: dartforge_elements::model::UnitId,
    prefix: Option<SymbolId>,
    deferred: bool,
    alvo: LibraryId,
    combinators: &'a [Combinator],
    uri: dartforge_diagnostics::Span,
    texto: String,
}

/// O que o import `imp` fornece sob `nome` (o namespace exportado do alvo
/// com os combinadores).
fn fornece<'p>(program: &'p Program, imp: &Imp<'_>, nome: SymbolId) -> Option<&'p dartforge_elements::model::Binding> {
    let b = program.library(imp.alvo).exported.get(&nome)?;
    imp.combinators
        .iter()
        .all(|c| match c {
            Combinator::Show(ns) => ns.iter().any(|n| n.sym == nome),
            Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == nome),
        })
        .then_some(b)
}

/// As referências de um comentário de documentação (`_parseReferences` do
/// `DocCommentBuilder`), fora dos blocos de código cercados ou indentados:
/// cada `[…]` que não é texto de link (seguido de `(`, de `:` no começo da
/// linha, ou de outro `[`), nem `[:…:]`, fora de `` `…` ``; as partes `a`,
/// `a.b` ou `a.b.c` (sem o `new` da frente).
fn referencias_do_doc(texto: &str) -> Vec<Vec<String>> {
    let mut v = Vec::new();
    // As linhas de conteúdo: sem o `///`, ou sem o `/**`, o `*/` e o `*` do
    // começo.
    let linhas: Vec<&str> = if let Some(r) = texto.strip_prefix("/**") {
        let r = r.strip_suffix("*/").unwrap_or(r);
        r.split('\n')
            .map(|l| {
                let l = l.trim_end_matches('\r');
                let t = l.trim_start();
                t.strip_prefix('*').unwrap_or(l)
            })
            .collect()
    } else {
        texto.split('\n').map(|l| l.trim_start().strip_prefix("///").unwrap_or(l).trim_end_matches('\r')).collect()
    };
    let mut cercado = false;
    let mut anterior_vazia = true;
    for conteudo in linhas {
        let recuo = conteudo.len() - conteudo.trim_start().len();
        if conteudo.trim_start().starts_with("```") {
            cercado = !cercado;
            anterior_vazia = false;
            continue;
        }
        if cercado || (anterior_vazia && recuo >= 4) {
            anterior_vazia = conteudo.trim().is_empty();
            continue;
        }
        anterior_vazia = conteudo.trim().is_empty();
        let b = conteudo.as_bytes();
        let mut i = 0;
        let mut so_brancos = true;
        while i < b.len() {
            match b[i] {
                b'[' => {
                    i += 1;
                    if b.get(i) == Some(&b':') {
                        match conteudo[i + 1..].find(":]") {
                            Some(k) => i = i + 1 + k + 1,
                            None => break,
                        }
                    } else {
                        let ini = i;
                        let Some(k) = conteudo[i..].find(']') else {
                            // Sem o `]` (`_findCommentReferenceEnd`): o
                            // identificador, e o `.identificador` dele.
                            let letra = |c: u8| c.is_ascii_alphabetic() || c == b'_' || c == b'$';
                            let letra_ou_digito = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
                            let mut j = i;
                            if b.get(j).is_some_and(|c| letra(*c)) {
                                while b.get(j).is_some_and(|c| letra_ou_digito(*c)) {
                                    j += 1;
                                }
                                if b.get(j) == Some(&b'.') && b.get(j + 1).is_some_and(|c| letra(*c)) {
                                    j += 1;
                                    while b.get(j).is_some_and(|c| letra_ou_digito(*c)) {
                                        j += 1;
                                    }
                                }
                                v.push(conteudo[ini..j].split('.').map(|x| x.to_string()).collect());
                            }
                            break;
                        };
                        let fim = i + k;
                        // `_isLinkText`.
                        let mut j = fim + 1;
                        let link = match b.get(j) {
                            Some(b'(') => true,
                            Some(b':') if so_brancos => true,
                            _ => {
                                while b.get(j).is_some_and(|c| c.is_ascii_whitespace()) {
                                    j += 1;
                                }
                                b.get(j) == Some(&b'[')
                            }
                        };
                        if !link {
                            let mut partes: Vec<String> = conteudo[ini..fim].trim().split('.').map(|x| x.trim().to_string()).collect();
                            if let Some(primeira) = partes.first_mut()
                                && let Some(r) = primeira.strip_prefix("new ")
                            {
                                *primeira = r.trim().to_string();
                            }
                            let identificador = |x: &String| {
                                !x.is_empty()
                                    && x.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
                                    && x.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
                            };
                            if !partes.is_empty() && partes.len() <= 3 && partes.iter().all(identificador) {
                                v.push(partes);
                            }
                        }
                        i = fim;
                    }
                    so_brancos = false;
                }
                b'`' => {
                    if let Some(k) = conteudo[i + 1..].find('`') {
                        i = i + 1 + k;
                    }
                    so_brancos = false;
                }
                c if !c.is_ascii_whitespace() => so_brancos = false,
                _ => {}
            }
            i += 1;
        }
    }
    v
}

/// `ImportsVerifier.generateUnusedImportHints`, `generateUnusedShownNameHints`
/// e `generateUnnecessaryImportHints` da 3.6.2, sobre o rastreio de uso do
/// analyzer (`ImportsTrackingOfPrefix`, `dart/element/scope.dart`): cada
/// busca léxica de um nome que não para num escopo local ou de membro
/// (`anotacoes::sombreado`) nem nas declarações da biblioteca (ou num
/// prefixo) e chega ao escopo de imports marca, em cada import sem prefixo
/// que fornece o nome, o getter e o setter achados; `p.x` marca os imports
/// do prefixo `p` (o `loadLibrary` de um prefixo `deferred` não marca); a
/// extensão escolhida numa resolução de membro (`BodyTypes::extensoes_usadas`)
/// marca os imports que a fornecem. As buscas: identificadores de
/// expressão, nomes de tipo, nomes de anotação, padrões constantes soltos e
/// as referências de comentário de documentação (no escopo da declaração
/// documentada); `[p]` de um prefixo num comentário libera os imports dele.
/// Sem os corpos (`corpos` ausente), a extensão trazida conta como usada.
pub fn nao_usados(
    program: &Program,
    lib: LibraryId,
    interner: &Interner,
    ja_relatados: &[Diagnostic],
    corpos: Option<&dartforge_types::resolved::BodyTypes>,
) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    if ja_relatados.iter().any(suprime) {
        return Vec::new();
    }
    let biblioteca = program.library(lib);
    // Os imports com alvo.
    let mut imps: Vec<Imp<'_>> = Vec::new();
    for imp in &biblioteca.imports {
        let unit = program.unit(imp.unit);
        let Some(dir) = unit.unit.directives.get(imp.directive) else { continue };
        let DirectiveKind::Import { uri, combinators, .. } = &dir.kind else { continue };
        let Some(texto) = dartforge_elements::load::string_lit_value(uri) else { continue };
        imps.push(Imp {
            unit: imp.unit,
            prefix: imp.prefix,
            deferred: imp.deferred,
            alvo: imp.library,
            combinators,
            uri: uri.span,
            texto,
        });
    }
    let prefixos: HashSet<SymbolId> = imps.iter().filter_map(|i| i.prefix).collect();
    let declarados = &biblioteca.declared;
    let load_library = interner.lookup("loadLibrary");
    let mut usados: Vec<HashSet<Element>> = vec![HashSet::new(); imps.len()];
    let mut prefixo_em_comentario: HashSet<SymbolId> = HashSet::new();

    // A busca léxica de `nome` (já sem os escopos locais): as declarações
    // da biblioteca e os prefixos param; senão, os imports sem prefixo.
    let marcar = |usados: &mut Vec<HashSet<Element>>, nome: SymbolId| {
        if declarados.contains_key(&nome) || prefixos.contains(&nome) {
            return;
        }
        for (i, imp) in imps.iter().enumerate() {
            if imp.prefix.is_none()
                && let Some(b) = fornece(program, imp, nome)
            {
                usados[i].extend(b.getter);
                usados[i].extend(b.setter);
            }
        }
    };
    // `p.x` com `p` prefixo.
    let marcar_prefixado = |usados: &mut Vec<HashSet<Element>>, p: SymbolId, nome: SymbolId| {
        for (i, imp) in imps.iter().enumerate() {
            if imp.prefix != Some(p) {
                continue;
            }
            if imp.deferred && Some(nome) == load_library {
                continue;
            }
            if let Some(b) = fornece(program, imp, nome) {
                usados[i].extend(b.getter);
                usados[i].extend(b.setter);
            }
        }
    };
    // `p` é um prefixo nesse lugar (não sombreado nem declarado).
    let e_prefixo = |a: &dartforge_frontend::ast::Ast, pos: usize, p: SymbolId| {
        prefixos.contains(&p) && !declarados.contains_key(&p) && !dartforge_types::anotacoes::sombreado(a, interner, pos, p)
    };

    for &u in &biblioteca.units {
        let unit = program.unit(u);
        if unit.role == UnitRole::Patch {
            continue;
        }
        let a = &unit.ast;
        let fonte = unit.source.as_str();
        // Os identificadores que são o prefixo de um `p.x`.
        let mut de_prefixo: HashSet<dartforge_frontend::ast::ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            if let ExprKind::Property { target, name, .. } = &e.kind
                && let ExprKind::Identifier(p) = &a.expr(*target).kind
                && e_prefixo(a, p.span.start, p.sym)
            {
                de_prefixo.insert(*target);
                marcar_prefixado(&mut usados, p.sym, name.sym);
            }
        }
        // Os identificadores de expressão.
        for (k, e) in a.exprs.iter().enumerate() {
            if let ExprKind::Identifier(n) = &e.kind
                && !de_prefixo.contains(&dartforge_frontend::ast::ExprId(k as u32))
                && !dartforge_types::anotacoes::sombreado(a, interner, n.span.start, n.sym)
            {
                marcar(&mut usados, n.sym);
            }
        }
        // Os nomes de tipo.
        for t in a.types.iter() {
            if let TypeKind::Named { name, .. } = &t.kind {
                match &name[..] {
                    [p, n, ..] if e_prefixo(a, p.span.start, p.sym) => marcar_prefixado(&mut usados, p.sym, n.sym),
                    [n, ..] => {
                        if !dartforge_types::anotacoes::sombreado(a, interner, n.span.start, n.sym) {
                            marcar(&mut usados, n.sym);
                        }
                    }
                    [] => {}
                }
            }
        }
        // Os nomes de anotação.
        for m in dartforge_frontend::pais::todas_as_anotacoes(a, &unit.unit) {
            match &m.name[..] {
                [p, n, ..] if e_prefixo(a, m.span.start, p.sym) => marcar_prefixado(&mut usados, p.sym, n.sym),
                [n, ..] => {
                    if !dartforge_types::anotacoes::sombreado(a, interner, m.span.start, n.sym) {
                        marcar(&mut usados, n.sym);
                    }
                }
                [] => {}
            }
        }
        // O nome solto num padrão refutável: um padrão constante.
        for n in constantes_de_padrao(a) {
            if !dartforge_types::anotacoes::sombreado(a, interner, n.span.start, n.sym) {
                marcar(&mut usados, n.sym);
            }
        }
        // As referências dos comentários de documentação, no escopo da
        // declaração documentada.
        let comentarios = dartforge_frontend::comentarios::Comentarios::de(fonte);
        for (doc, pos, extras) in documentacoes(a, &unit.unit, fonte, &comentarios) {
            for partes in referencias_do_doc(&fonte[doc.start..doc.end]) {
                let simbolos: Vec<Option<SymbolId>> = partes.iter().map(|x| interner.lookup(x)).collect();
                let Some(primeiro) = simbolos[0] else { continue };
                if extras.contains(&primeiro) || dartforge_types::anotacoes::sombreado(a, interner, pos, primeiro) {
                    continue;
                }
                let prefixo = prefixos.contains(&primeiro) && !declarados.contains_key(&primeiro);
                if prefixo {
                    match simbolos.get(1) {
                        Some(Some(n)) => marcar_prefixado(&mut usados, primeiro, *n),
                        Some(None) => {}
                        // `[p]`: o prefixo citado num comentário.
                        None => {
                            prefixo_em_comentario.insert(primeiro);
                        }
                    }
                } else {
                    marcar(&mut usados, primeiro);
                }
            }
        }
    }
    // `notifyExtensionUsed`.
    match corpos {
        Some(c) => {
            for &(l, x) in c.extensoes_usadas.iter() {
                if l != lib {
                    continue;
                }
                let Some(nome) = program.extension(x).name else { continue };
                for (i, imp) in imps.iter().enumerate() {
                    if fornece(program, imp, nome).is_some_and(|b| b.getter == Some(Element::Extension(x))) {
                        usados[i].insert(Element::Extension(x));
                    }
                }
            }
        }
        None => {
            for (i, imp) in imps.iter().enumerate() {
                for (k, b) in program.library(imp.alvo).exported.iter() {
                    if let Some(x @ Element::Extension(_)) = b.getter
                        && fornece(program, imp, *k).is_some()
                    {
                        usados[i].insert(x);
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    // `generateUnusedImportHints`.
    let mut nao_usado = vec![false; imps.len()];
    for (i, imp) in imps.iter().enumerate() {
        if imp.prefix.is_some_and(|p| prefixo_em_comentario.contains(&p)) {
            continue;
        }
        let alvo = program.library(imp.alvo);
        // `dart:core` explícito e alvo que não existe (relatado em outro lugar).
        if alvo.uri == "dart:core" || alvo.units.is_empty() {
            continue;
        }
        if usados[i].is_empty() {
            nao_usado[i] = true;
            out.push((imp.unit, Diagnostic::com_codigo(w::UNUSED_IMPORT, imp.uri, [imp.texto.as_str()])));
        }
    }
    // `generateUnusedShownNameHints`: o nome mostrado cujo elemento não foi
    // usado deste import.
    for (i, imp) in imps.iter().enumerate() {
        let alvo = program.library(imp.alvo);
        if nao_usado[i] || alvo.uri == "dart:core" || alvo.units.is_empty() {
            continue;
        }
        for comb in imp.combinators.iter() {
            let Combinator::Show(ns) = comb else { continue };
            for n in ns.iter() {
                let Some(b) = alvo.exported.get(&n.sym) else { continue };
                let Some(elemento) = b.getter.or(b.setter) else { continue };
                if !usados[i].contains(&elemento) {
                    out.push((imp.unit, Diagnostic::com_codigo(w::UNUSED_SHOWN_NAME, n.span, [interner.resolve(n.sym)])));
                }
            }
        }
    }
    // `generateUnnecessaryImportHints`: um import usado é desnecessário se
    // outro import usado do mesmo prefixo provê todos os elementos usados
    // dele e estritamente mais (sem os de export depreciado).
    let acessados: Vec<HashSet<Element>> = imps
        .iter()
        .enumerate()
        .map(|(i, imp)| usados[i].iter().copied().filter(|e| !de_export_depreciado(program, interner, imp.alvo, *e)).collect())
        .collect();
    for (i, primeiro) in imps.iter().enumerate() {
        if nao_usado[i] || program.library(primeiro.alvo).uri == "dart:core" || program.library(primeiro.alvo).units.is_empty() {
            continue;
        }
        for (j, segundo) in imps.iter().enumerate() {
            if i == j || nao_usado[j] || program.library(segundo.alvo).units.is_empty() || primeiro.prefix != segundo.prefix {
                continue;
            }
            if acessados[i].is_subset(&acessados[j]) && acessados[j].len() > acessados[i].len() {
                out.push((
                    primeiro.unit,
                    Diagnostic::com_codigo(
                        dartforge_diagnostics::codigos::hint::UNNECESSARY_IMPORT,
                        primeiro.uri,
                        [primeiro.texto.as_str(), segundo.texto.as_str()],
                    ),
                ));
                break;
            }
        }
    }
    out
}

/// `isFromDeprecatedExport`: o elemento não é declarado na biblioteca
/// `alvo` e todos os `export` dela que o fornecem são `@deprecated`.
fn de_export_depreciado(program: &Program, interner: &Interner, alvo: LibraryId, e: Element) -> bool {
    let b = program.library(alvo);
    if b.declared.values().any(|x| x.getter == Some(e) || x.setter == Some(e)) {
        return false;
    }
    let mut algum = false;
    for ex in b.exports.iter() {
        let fornece = program.library(ex.library).exported.iter().any(|(nome, x)| {
            (x.getter == Some(e) || x.setter == Some(e))
                && ex.combinators.iter().all(|c| match c {
                    Combinator::Show(ns) => ns.iter().any(|n| n.sym == *nome),
                    Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == *nome),
                })
        });
        if !fornece {
            continue;
        }
        algum = true;
        let Some(dir) = program.unit(ex.unit).unit.directives.get(ex.directive) else { return false };
        if !dir.metadata.iter().any(|m| dartforge_types::anotacoes::e_deprecated(program, interner, ex.unit, m)) {
            return false;
        }
    }
    algum
}

/// Os nomes soltos de padrões refutáveis (`case x:`, `if (v case x)`): os
/// padrões constantes que o parser guarda como variável sem tipo.
fn constantes_de_padrao(a: &dartforge_frontend::ast::Ast) -> Vec<dartforge_frontend::ast::Name> {
    use dartforge_frontend::ast::{CollectionElement, ListPatternElement, PatternId, StmtKind};
    let mut raizes: Vec<PatternId> = Vec::new();
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Switch { cases, .. } => raizes.extend(cases.iter().filter_map(|c| c.pattern)),
            StmtKind::If { case_pattern: Some(p), .. } => raizes.push(*p),
            _ => {}
        }
    }
    fn de_colecao(el: &CollectionElement, raizes: &mut Vec<PatternId>) {
        match el {
            CollectionElement::If { case_pattern, then, else_, .. } => {
                raizes.extend(case_pattern.iter().copied());
                de_colecao(then, raizes);
                if let Some(x) = else_ {
                    de_colecao(x, raizes);
                }
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => de_colecao(body, raizes),
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Switch { cases, .. } => raizes.extend(cases.iter().map(|c| c.pattern)),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    de_colecao(el, &mut raizes);
                }
            }
            _ => {}
        }
    }
    let mut v = Vec::new();
    while let Some(p) = raizes.pop() {
        match &a.pattern(p).kind {
            PatternKind::Variable { name, final_: false, var_: false, ty: None } => v.push(*name),
            PatternKind::Or(l, r) | PatternKind::And(l, r) => raizes.extend([*l, *r]),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => raizes.push(*x),
            PatternKind::List { elements, .. } => raizes.extend(elements.iter().filter_map(|e| match e {
                ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => Some(*x),
                ListPatternElement::Rest(None) => None,
            })),
            PatternKind::Map { entries, .. } => raizes.extend(entries.iter().map(|e| e.value)),
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                // O `:x` de campo é a variável, não uma busca.
                for f in fields.iter() {
                    let sp = a.pattern(f.pattern).span;
                    let atalho = f.name.is_some_and(|n| n.span.start >= sp.start && n.span.end <= sp.end);
                    if !atalho {
                        raizes.push(f.pattern);
                    }
                }
            }
            _ => {}
        }
    }
    v
}

/// Os comentários de documentação da unidade, cada um com a posição para a
/// busca léxica das referências e os nomes do escopo da declaração
/// documentada que a posição não cobre (os parâmetros e os parâmetros de
/// tipo da função, do método ou do typedef documentado).
fn documentacoes(
    a: &dartforge_frontend::ast::Ast,
    unidade: &dartforge_frontend::ast::CompilationUnit,
    fonte: &str,
    comentarios: &dartforge_frontend::comentarios::Comentarios,
) -> Vec<(dartforge_diagnostics::Span, usize, Vec<SymbolId>)> {
    use dartforge_frontend::ast::{Annotation, DeclKind, MemberKind, Parameter, TypedefKind};
    // `_findComment(metadata, tokenAfterMetadata)`.
    let doc = |metadata: &[Annotation], inicio: usize| {
        let depois = match metadata.last() {
            Some(m) => dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), m.span.end),
            None => inicio,
        };
        comentarios.dart_doc(fonte, depois).or_else(|| metadata.iter().rev().find_map(|m| comentarios.dart_doc(fonte, m.span.start)))
    };
    // O comentário inteiro: o `/**`, ou a sequência de `///` que começa no
    // achado.
    let inteiro = |d: dartforge_diagnostics::Span| {
        if fonte[d.start..d.end].starts_with("///") {
            let mut fim = d.end;
            for c in comentarios.todos().iter().filter(|c| c.start > d.start) {
                let entre = &fonte[fim..c.start];
                if !entre.trim().is_empty() || !fonte[c.start..c.end].starts_with("///") || entre.matches('\n').count() > 1 {
                    break;
                }
                fim = c.end;
            }
            dartforge_diagnostics::Span { start: d.start, end: fim }
        } else {
            d
        }
    };
    let nomes_de = |ps: &[Parameter], tps: &[dartforge_frontend::ast::TypeParameter]| -> Vec<SymbolId> {
        ps.iter().filter_map(|p| p.name.map(|n| n.sym)).chain(tps.iter().map(|t| t.name.sym)).collect()
    };
    let mut v = Vec::new();
    for d in unidade.directives.iter() {
        if let Some(x) = doc(&d.metadata, d.span.start) {
            v.push((inteiro(x), d.span.start, Vec::new()));
        }
    }
    for decl in a.decls.iter() {
        let Some(x) = doc(&decl.metadata, decl.span.start) else { continue };
        let (pos, extras) = match &decl.kind {
            DeclKind::Function(f) => {
                let g = a.function(*f);
                (decl.span.start, nomes_de(g.parameters.as_deref().unwrap_or(&[]), &g.type_params))
            }
            DeclKind::Typedef(t) => {
                let ps: &[Parameter] = match &t.kind {
                    TypedefKind::Legacy { parameters, .. } => parameters,
                    TypedefKind::Alias(_) => &[],
                };
                (decl.span.start, nomes_de(ps, &t.type_params))
            }
            // O tipo: dentro dele (os membros e os parâmetros de tipo).
            DeclKind::Class(c) => (c.name.span.end + 1, Vec::new()),
            DeclKind::Mixin(c) => (c.name.span.end + 1, Vec::new()),
            DeclKind::Enum(c) => (c.name.span.end + 1, Vec::new()),
            DeclKind::ExtensionType(c) => (c.name.span.end + 1, Vec::new()),
            DeclKind::Extension(c) => (c.name.map_or(decl.span.start, |n| n.span.end) + 1, c.type_params.iter().map(|t| t.name.sym).collect()),
            DeclKind::Variables(_) => (decl.span.start, Vec::new()),
        };
        v.push((inteiro(x), pos, extras));
        if let DeclKind::Enum(c) = &decl.kind {
            for k in c.constants.iter() {
                if let Some(y) = doc(&k.metadata, k.span.start) {
                    v.push((inteiro(y), k.span.start, Vec::new()));
                }
            }
        }
    }
    for m in a.members.iter() {
        let Some(x) = doc(&m.metadata, m.span.start) else { continue };
        let extras = match &m.kind {
            MemberKind::Method(f) => {
                let g = a.function(*f);
                nomes_de(g.parameters.as_deref().unwrap_or(&[]), &g.type_params)
            }
            MemberKind::Constructor(k) => nomes_de(&k.parameters, &[]),
            MemberKind::Field(_) => Vec::new(),
        };
        v.push((inteiro(x), m.span.start, extras));
    }
    v
}
