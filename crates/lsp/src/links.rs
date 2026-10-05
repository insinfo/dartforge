//! `textDocument/documentLink` (docs/LSP-ESPECIFICACAO.md §8.4): o
//! `DartDocumentLinkVisitor` do 3.6.2
//! (`analyzer_plugin/lib/utilities/navigation/document_links.dart`). Em cada
//! comentário de documentação da árvore (o `Comment` de uma declaração, de
//! um membro, de uma constante de enum, de uma diretiva, de um parâmetro),
//! cada bloco `{@tool …}` … `{@end-tool}` cujo conteúdo cita
//! `** See code in examples/api/…dart` liga o trecho do caminho ao arquivo
//! sob a pasta ancestral que tem `examples/api` (a convenção do Flutter).
//! Sem essa pasta, nenhum link. O 3.6.2 não liga as URIs das diretivas.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_diagnostics::Span;
use dartforge_frontend::ast;
use dartforge_frontend::comentarios::Comentarios;
use std::path::{Path, PathBuf};

/// Os links de `texto` (o arquivo `caminho`): o trecho e o arquivo alvo.
pub(crate) fn links(texto: &str, caminho: &Path) -> Vec<(Span, PathBuf)> {
    let mut nomes = dartforge_intern::Interner::new();
    let p = dartforge_frontend::parser::parse_com(texto, &mut nomes, dartforge_frontend::LibraryFeatures::default());
    let comentarios = Comentarios::de(texto);
    let mut docs: Vec<Span> = Vec::new();
    let mut doc_de = |inicio: usize| {
        if let Some(s) = comentario_de_documentacao(&comentarios, texto, inicio)
            && !docs.contains(&s)
        {
            docs.push(s);
        }
    };
    let inicio_de = |span: Span, metadata: &[ast::Annotation]| metadata.first().map_or(span.start, |m| m.span.start.min(span.start));
    for d in &p.unit.directives {
        doc_de(inicio_de(d.span, &d.metadata));
    }
    let a = &p.ast;
    for d in &a.decls {
        doc_de(inicio_de(d.span, &d.metadata));
        if let ast::DeclKind::Enum(e) = &d.kind {
            for k in &e.constants {
                doc_de(inicio_de(k.span, &k.metadata));
            }
        }
    }
    for m in &a.members {
        doc_de(inicio_de(m.span, &m.metadata));
        if let ast::MemberKind::Constructor(k) = &m.kind {
            for x in k.parameters.iter() {
                doc_de(inicio_de(x.span, &x.metadata));
            }
        }
    }
    for f in &a.functions {
        for x in f.parameters.iter().flatten() {
            doc_de(inicio_de(x.span, &x.metadata));
        }
    }
    docs.sort_by_key(|s| s.start);
    let mut pasta: Option<Option<PathBuf>> = None;
    let mut saida = Vec::new();
    for d in docs {
        let trecho = &texto[d.start..d.end];
        let mut busca = 0;
        while let Some(k) = trecho[busca..].find("{@tool") {
            let abre = busca + k;
            // `{@tool}` ou `{@tool argumentos}` (não `{@toolbar}`).
            if !matches!(trecho.as_bytes().get(abre + 6), Some(b' ' | b'}' | b'\t' | b'\r' | b'\n')) {
                busca = abre + 6;
                continue;
            }
            let Some(f) = trecho[abre..].find('}') else { break };
            let conteudo = abre + f + 1;
            // Sem `{@end-tool}`, o bloco não fecha e fica de fora.
            let Some(c) = trecho[conteudo..].find("{@end-tool}") else {
                busca = conteudo;
                continue;
            };
            let fim = conteudo + c;
            busca = fim + "{@end-tool}".len();
            let valor = &trecho[conteudo..fim];
            if valor.is_empty() {
                continue;
            }
            const VER: &str = "** See code in ";
            let Some(i) = valor.find("** See code in examples/api/") else { continue };
            let raiz = pasta.get_or_insert_with(|| pasta_com_exemplos(caminho));
            // Sem a pasta, o `return` do visitor: nada mais deste comentário.
            let Some(raiz) = raiz.clone() else { break };
            let comeco = i + VER.len();
            // O primeiro `.dart` do conteúdo (o `indexOf` do visitor). Antes
            // do começo, o `substring` do Dart lançaria: o link fica de fora.
            let Some(termino) = valor.find(".dart").map(|e| e + 5) else { continue };
            if termino < comeco {
                continue;
            }
            let alvo = valor[comeco..termino].split('/').fold(raiz, |p, parte| p.join(parte));
            let inicio = d.start + conteudo + comeco;
            saida.push((Span { start: inicio, end: inicio + (termino - comeco) }, alvo));
        }
    }
    saida
}

/// O `Comment` de documentação do nó que começa em `inicio`
/// (`findDartDoc`): o último `/**`, ou da primeira à última linha da última
/// sequência de `///`.
fn comentario_de_documentacao(c: &Comentarios, texto: &str, inicio: usize) -> Option<Span> {
    let doc = c.dart_doc(texto, inicio)?;
    if !texto[doc.start..doc.end].starts_with("///") {
        return Some(doc);
    }
    let fim = c
        .antes_de(texto, inicio)
        .into_iter()
        .filter(|s| s.start >= doc.start && texto[s.start..s.end].starts_with("///"))
        .map(|s| s.end)
        .max()
        .unwrap_or(doc.end);
    Some(Span { start: doc.start, end: fim })
}

/// A pasta, entre a do arquivo e as ancestrais, que tem `examples/api`.
fn pasta_com_exemplos(caminho: &Path) -> Option<PathBuf> {
    let mut atual = caminho.parent();
    while let Some(p) = atual {
        if p.join("examples").join("api").is_dir() {
            return Some(p.to_path_buf());
        }
        atual = p.parent();
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn liga_o_codigo_de_exemplo_do_bloco_tool() {
        let raiz = std::env::temp_dir().join(format!("dartforge-links-{}", std::process::id()));
        let _ = std::fs::create_dir_all(raiz.join("examples/api"));
        let _ = std::fs::create_dir_all(raiz.join("lib"));
        let texto = "/// Um widget.\n///\n/// {@tool dartpad}\n/// ** See code in examples/api/lib/a/b.0.dart **\n/// {@end-tool}\nclass A {}\n\n/// {@tool snippet}\n/// ** See code in examples/api/lib/c.dart **\nclass B {}\n";
        let ls = links(texto, &raiz.join("lib/a.dart"));
        assert_eq!(ls.len(), 1, "o bloco sem {{@end-tool}} fica de fora: {ls:?}");
        let (s, alvo) = &ls[0];
        assert_eq!(&texto[s.start..s.end], "examples/api/lib/a/b.0.dart");
        assert_eq!(alvo, &raiz.join("examples").join("api").join("lib").join("a").join("b.0.dart"));
        // Sem a pasta `examples/api`, nenhum link.
        let outra = std::env::temp_dir().join(format!("dartforge-links-sem-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&outra);
        assert!(links(texto, &outra.join("a.dart")).is_empty());
        let _ = std::fs::remove_dir_all(&raiz);
        let _ = std::fs::remove_dir_all(&outra);
    }
}
