//! Comentários de documentação (`///` e `/** … */`): o texto limpo para o
//! hover e para o `completionItem/resolve`, e as referências `[nome]`,
//! `[Classe.membro]` e `[p.Nome]` que o renomear altera junto com o elemento.
//!
//! Só a varredura léxica vive aqui; a resolução de cada referência (o que
//! `[nome]` denota no escopo da declaração documentada) é de
//! [`crate::projeto`], com as tabelas do programa carregado.

use dartforge_diagnostics::Span;

/// Um bloco de documentação: as linhas `///` consecutivas ou um `/** */`.
#[derive(Debug, Clone)]
pub(crate) struct Comentario {
    /// Do primeiro `/` ao fim da última linha (ou do `*/`).
    pub span: Span,
    /// As referências entre colchetes, cada uma com as partes separadas por
    /// ponto (`[A.b]` tem duas), com o intervalo de cada parte na fonte.
    pub referencias: Vec<Vec<(Span, String)>>,
}

/// Os blocos de documentação de `fonte`, na ordem. Strings e comentários
/// comuns não são confundidos com documentação (a varredura usa o lexer).
pub(crate) fn comentarios(fonte: &str) -> Vec<Comentario> {
    let mut saida: Vec<Comentario> = Vec::new();
    // Cada lacuna entre tokens é uma cadeia `precedingComments`; o
    // `findDartDoc` acha o começo do comentário de documentação nela (o
    // último `/**`, ou o primeiro `///` depois dele), e o comentário vai até
    // o último `///` da cadeia (`DocCommentBuilder.build`).
    for cadeia in cadeias_de_comentario(fonte) {
        let mut doc: Option<usize> = None;
        let mut multilinha = false;
        for (k, &(i, f)) in cadeia.iter().enumerate() {
            let lexema = &fonte[i..f];
            if lexema.starts_with("///") {
                if !multilinha {
                    doc = Some(k);
                    multilinha = true;
                }
            } else if lexema.starts_with("/**") {
                doc = Some(k);
                multilinha = false;
            }
        }
        let Some(k) = doc else { continue };
        let tokens: Vec<Span> = cadeia[k..].iter().map(|&(i, f)| Span { start: i, end: f }).collect();
        let (inicio, fim_do_primeiro) = cadeia[k];
        let fim = if fonte[inicio..].starts_with("///") {
            tokens.iter().filter(|s| fonte[s.start..s.end].starts_with("///")).map(|s| s.end).last().unwrap_or(fim_do_primeiro)
        } else {
            fim_do_primeiro
        };
        let referencias = dartforge_frontend::doc_referencias::referencias(fonte, &tokens).into_iter().map(|r| r.partes).collect();
        saida.push(Comentario { span: Span { start: inicio, end: fim }, referencias });
    }
    saida
}

/// Os comentários de `fonte` (fora de strings) agrupados por lacuna entre
/// tokens, pelo que o lexer deixa entre eles.
fn cadeias_de_comentario(fonte: &str) -> Vec<Vec<(usize, usize)>> {
    let Ok(tokens) = dartforge_frontend::lexer::lex(fonte) else {
        return Vec::new();
    };
    let mut lacunas = Vec::new();
    let mut anterior = 0;
    for t in &tokens {
        if t.span.start > anterior {
            lacunas.push((anterior, t.span.start));
        }
        anterior = anterior.max(t.span.end);
    }
    if anterior < fonte.len() {
        lacunas.push((anterior, fonte.len()));
    }
    let mut cadeias = Vec::new();
    for (de, ate) in lacunas {
        let mut saida = Vec::new();
        let b = fonte.as_bytes();
        let mut i = de;
        while i + 1 < ate {
            if b[i] == b'/' && b[i + 1] == b'/' {
                let fim = fonte[i..ate].find('\n').map_or(ate, |n| i + n);
                saida.push((i, fim));
                i = fim;
            } else if b[i] == b'/' && b[i + 1] == b'*' {
                // Comentários de bloco aninham no Dart.
                let mut profundidade = 0usize;
                let mut j = i;
                while j + 1 < ate {
                    if b[j] == b'/' && b[j + 1] == b'*' {
                        profundidade += 1;
                        j += 2;
                    } else if b[j] == b'*' && b[j + 1] == b'/' {
                        profundidade -= 1;
                        j += 2;
                        if profundidade == 0 {
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                saida.push((i, j.min(ate)));
                i = j;
            } else {
                i += 1;
            }
        }
        if !saida.is_empty() {
            cadeias.push(saida);
        }
    }
    cadeias
}

/// O comentário de documentação imediatamente antes de `inicio` (o começo
/// da declaração, com ou sem metadados), já limpo: sem `///`, `/**`, `*/` e
/// os `*` de margem. `None` quando não há.
pub(crate) fn documentacao(fonte: &str, inicio: usize) -> Option<String> {
    let inicio = inicio.min(fonte.len());
    // O comentário cujo fim só é seguido de espaço e de metadados até a
    // declaração.
    let c = comentarios(&fonte[..inicio]).into_iter().last()?;
    let entre = &fonte[c.span.end..inicio];
    let so_metadados = entre
        .lines()
        .map(str::trim)
        .all(|l| l.is_empty() || l.starts_with('@') || l.starts_with(')') || l.ends_with(','));
    if !so_metadados {
        return None;
    }
    Some(limpar(&fonte[c.span.start..c.span.end]))
}

/// Tira os marcadores do comentário e a indentação comum.
pub(crate) fn limpar(bruto: &str) -> String {
    let mut linhas: Vec<String> = Vec::new();
    if bruto.starts_with("/**") {
        let corpo = bruto.trim_start_matches("/**").trim_end_matches("*/");
        for l in corpo.lines() {
            let l = l.trim_start();
            let l = l.strip_prefix('*').unwrap_or(l);
            linhas.push(l.strip_prefix(' ').unwrap_or(l).trim_end().to_string());
        }
    } else {
        for l in bruto.lines() {
            let l = l.trim_start();
            let l = l.strip_prefix("///").unwrap_or(l);
            linhas.push(l.strip_prefix(' ').unwrap_or(l).trim_end().to_string());
        }
    }
    while linhas.first().is_some_and(|l| l.is_empty()) {
        linhas.remove(0);
    }
    while linhas.last().is_some_and(|l| l.is_empty()) {
        linhas.pop();
    }
    linhas.join("\n")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn bloco_de_linhas_e_referencias() {
        let fonte = "/// Soma [a] com [b].\n/// Veja [Caixa.valor] e `[naoRef]`.\n@deprecated\nint soma(int a, int b) => a + b;\n// [comum] não conta\n";
        let cs = comentarios(fonte);
        assert_eq!(cs.len(), 1);
        let refs: Vec<Vec<String>> = cs[0]
            .referencias
            .iter()
            .map(|r| r.iter().map(|(_, t)| t.clone()).collect())
            .collect();
        assert_eq!(refs, vec![vec!["a"], vec!["b"], vec!["Caixa", "valor"]]);
        let (s, _) = &cs[0].referencias[2][1];
        assert_eq!(&fonte[s.start..s.end], "valor");
        let inicio = fonte.find("@deprecated").unwrap();
        assert_eq!(
            documentacao(fonte, inicio).as_deref(),
            Some("Soma [a] com [b].\nVeja [Caixa.valor] e `[naoRef]`.")
        );
    }

    #[test]
    fn bloco_com_asteriscos_e_sem_documentacao() {
        let fonte = "/**\n * Primeira.\n *\n * Segunda [x].\n */\nclass A {}\nvar b = 0; // fim\nclass C {}\n";
        let inicio = fonte.find("class A").unwrap();
        assert_eq!(
            documentacao(fonte, inicio).as_deref(),
            Some("Primeira.\n\nSegunda [x].")
        );
        assert_eq!(documentacao(fonte, fonte.find("class C").unwrap()), None);
        // O texto do link Markdown não é referência; o rótulo de `[t][r]`
        // é (`_parseReferences` lê o `[r]` à parte).
        let cs = comentarios("/// [texto](http://x) e [ref][r].\nvar x;\n");
        let refs: Vec<&str> = cs[0].referencias.iter().map(|r| r[0].1.as_str()).collect();
        assert_eq!(refs, vec!["r"]);
    }

    #[test]
    fn texto_com_caracteres_de_varios_bytes() {
        // Bandeiras e acentos (vários bytes) antes e depois das referências.
        let fonte = "/// Região 🇵🇹 usa [ação] e `código 🇧🇷` [b].\nvar b;\n";
        let cs = comentarios(fonte);
        let refs: Vec<&str> = cs[0].referencias.iter().map(|r| r[0].1.as_str()).collect();
        assert_eq!(refs, vec!["b"]);
        assert_eq!(
            documentacao(fonte, fonte.find("var").unwrap()).as_deref(),
            Some("Região 🇵🇹 usa [ação] e `código 🇧🇷` [b].")
        );
    }

    #[test]
    fn string_com_barras_nao_e_comentario() {
        let fonte = "var s = '/// [x]';\n";
        assert!(comentarios(fonte).is_empty());
    }
}
