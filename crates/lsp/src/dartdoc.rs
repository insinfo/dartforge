//! Comentários de documentação (`///` e `/** … */`): o texto limpo para o
//! hover e para o `completionItem/resolve`, e as referências `[name]`,
//! `[Class.member]` e `[p.Name]` que o renomear altera junto com o elemento.
//!
//! Só a varredura léxica vive aqui; a resolução de cada referência (o que
//! `[name]` denota no escopo da declaração documentada) é de
//! [`crate::project`], com as tabelas do programa carregado.

use dartforge_diagnostics::Span;

/// Um bloco de documentação: as linhas `///` consecutivas ou um `/** */`.
#[derive(Debug, Clone)]
pub(crate) struct Comment {
    /// Do primeiro `/` ao fim da última linha (ou do `*/`).
    pub span: Span,
    /// As referências entre colchetes, cada uma com as partes separadas por
    /// ponto (`[A.b]` tem duas), com o intervalo de cada parte na fonte.
    pub references: Vec<Vec<(Span, String)>>,
}

/// Os blocos de documentação de `source`, na ordem. Strings e comentários
/// comuns não são confundidos com documentação (a varredura usa o lexer).
pub(crate) fn comments(source: &str) -> Vec<Comment> {
    let mut output: Vec<Comment> = Vec::new();
    // Cada lacuna entre tokens é uma cadeia `precedingComments`; o
    // `findDartDoc` acha o começo do comentário de documentação nela (o
    // último `/**`, ou o primeiro `///` depois dele), e o comentário vai até
    // o último `///` da cadeia (`DocCommentBuilder.build`).
    for chain in comment_chains(source) {
        let mut doc: Option<usize> = None;
        let mut multiline = false;
        for (k, &(i, f)) in chain.iter().enumerate() {
            let lexeme = &source[i..f];
            if lexeme.starts_with("///") {
                if !multiline {
                    doc = Some(k);
                    multiline = true;
                }
            } else if lexeme.starts_with("/**") {
                doc = Some(k);
                multiline = false;
            }
        }
        let Some(k) = doc else { continue };
        let tokens: Vec<Span> = chain[k..].iter().map(|&(i, f)| Span { start: i, end: f }).collect();
        let (start, first_end) = chain[k];
        let end = if source[start..].starts_with("///") {
            tokens.iter().filter(|s| source[s.start..s.end].starts_with("///")).map(|s| s.end).last().unwrap_or(first_end)
        } else {
            first_end
        };
        let references = dartforge_frontend::doc_referencias::referencias(source, &tokens).into_iter().map(|r| r.partes).collect();
        output.push(Comment { span: Span { start: start, end: end }, references });
    }
    output
}

/// Os comentários de `source` (fora de strings) agrupados por lacuna entre
/// tokens, pelo que o lexer deixa entre eles.
fn comment_chains(source: &str) -> Vec<Vec<(usize, usize)>> {
    let Ok(tokens) = dartforge_frontend::lexer::lex(source) else {
        return Vec::new();
    };
    let mut gaps = Vec::new();
    let mut previous = 0;
    for t in &tokens {
        if t.span.start > previous {
            gaps.push((previous, t.span.start));
        }
        previous = previous.max(t.span.end);
    }
    if previous < source.len() {
        gaps.push((previous, source.len()));
    }
    let mut chains = Vec::new();
    for (de, until) in gaps {
        let mut output = Vec::new();
        let b = source.as_bytes();
        let mut i = de;
        while i + 1 < until {
            if b[i] == b'/' && b[i + 1] == b'/' {
                let end = source[i..until].find('\n').map_or(until, |n| i + n);
                output.push((i, end));
                i = end;
            } else if b[i] == b'/' && b[i + 1] == b'*' {
                // Comentários de bloco aninham no Dart.
                let mut depth = 0usize;
                let mut j = i;
                while j + 1 < until {
                    if b[j] == b'/' && b[j + 1] == b'*' {
                        depth += 1;
                        j += 2;
                    } else if b[j] == b'*' && b[j + 1] == b'/' {
                        depth -= 1;
                        j += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                output.push((i, j.min(until)));
                i = j;
            } else {
                i += 1;
            }
        }
        if !output.is_empty() {
            chains.push(output);
        }
    }
    chains
}

/// O comentário de documentação imediatamente antes de `start` (o começo
/// da declaração, com ou sem metadados), já limpo: sem `///`, `/**`, `*/` e
/// os `*` de margem. `None` quando não há.
pub(crate) fn documentation(source: &str, start: usize) -> Option<String> {
    let start = start.min(source.len());
    // O comentário cujo fim só é seguido de espaço e de metadados até a
    // declaração.
    let c = comments(&source[..start]).into_iter().last()?;
    let between = &source[c.span.end..start];
    let only_metadata = between
        .lines()
        .map(str::trim)
        .all(|l| l.is_empty() || l.starts_with('@') || l.starts_with(')') || l.ends_with(','));
    if !only_metadata {
        return None;
    }
    Some(clear(&source[c.span.start..c.span.end]))
}

/// Tira os marcadores do comentário e a indentação comum.
pub(crate) fn clear(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    if raw.starts_with("/**") {
        let body = raw.trim_start_matches("/**").trim_end_matches("*/");
        for l in body.lines() {
            let l = l.trim_start();
            let l = l.strip_prefix('*').unwrap_or(l);
            lines.push(l.strip_prefix(' ').unwrap_or(l).trim_end().to_string());
        }
    } else {
        for l in raw.lines() {
            let l = l.trim_start();
            let l = l.strip_prefix("///").unwrap_or(l);
            lines.push(l.strip_prefix(' ').unwrap_or(l).trim_end().to_string());
        }
    }
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_block_and_references() {
        let source = "/// Soma [a] com [b].\n/// Veja [Caixa.valor] e `[naoRef]`.\n@deprecated\nint soma(int a, int b) => a + b;\n// [comum] não conta\n";
        let cs = comments(source);
        assert_eq!(cs.len(), 1);
        let refs: Vec<Vec<String>> = cs[0]
            .references
            .iter()
            .map(|r| r.iter().map(|(_, t)| t.clone()).collect())
            .collect();
        assert_eq!(refs, vec![vec!["a"], vec!["b"], vec!["Caixa", "valor"]]);
        let (s, _) = &cs[0].references[2][1];
        assert_eq!(&source[s.start..s.end], "valor");
        let start = source.find("@deprecated").unwrap();
        assert_eq!(
            documentation(source, start).as_deref(),
            Some("Soma [a] com [b].\nVeja [Caixa.valor] e `[naoRef]`.")
        );
    }

    #[test]
    fn block_with_asterisks_and_without_documentation() {
        let source = "/**\n * Primeira.\n *\n * Segunda [x].\n */\nclass A {}\nvar b = 0; // fim\nclass C {}\n";
        let start = source.find("class A").unwrap();
        assert_eq!(
            documentation(source, start).as_deref(),
            Some("Primeira.\n\nSegunda [x].")
        );
        assert_eq!(documentation(source, source.find("class C").unwrap()), None);
        // O texto do link Markdown não é referência; o rótulo de `[t][r]`
        // é (`_parseReferences` lê o `[r]` à parte).
        let cs = comments("/// [texto](http://x) e [ref][r].\nvar x;\n");
        let refs: Vec<&str> = cs[0].references.iter().map(|r| r[0].1.as_str()).collect();
        assert_eq!(refs, vec!["r"]);
    }

    #[test]
    fn text_with_multibyte_characters() {
        // Bandeiras e acentos (vários bytes) antes e depois das referências.
        let source = "/// Região 🇵🇹 usa [ação] e `código 🇧🇷` [b].\nvar b;\n";
        let cs = comments(source);
        let refs: Vec<&str> = cs[0].references.iter().map(|r| r[0].1.as_str()).collect();
        assert_eq!(refs, vec!["b"]);
        assert_eq!(
            documentation(source, source.find("var").unwrap()).as_deref(),
            Some("Região 🇵🇹 usa [ação] e `código 🇧🇷` [b].")
        );
    }

    #[test]
    fn string_with_slashes_is_not_comment() {
        let source = "var s = '/// [x]';\n";
        assert!(comments(source).is_empty());
    }
}
