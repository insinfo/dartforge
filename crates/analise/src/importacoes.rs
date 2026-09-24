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
use dartforge_frontend::ast::{Combinator, DirectiveKind, ExprKind, TypeKind};
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
