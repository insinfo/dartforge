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
    for (inicio, fim) in trechos_de_comentario(fonte) {
        let texto = &fonte[inicio..fim];
        let linha_doc = texto.starts_with("///") && !texto.starts_with("////");
        let bloco_doc = texto.starts_with("/**") && texto != "/**/";
        if !linha_doc && !bloco_doc {
            continue;
        }
        // Linhas `///` separadas só por espaço formam um bloco.
        if linha_doc
            && let Some(anterior) = saida.last_mut()
            && fonte[anterior.span.start..].starts_with("///")
            && fonte[anterior.span.end..inicio]
                .chars()
                .all(char::is_whitespace)
            && fonte[anterior.span.end..inicio].matches('\n').count() <= 1
        {
            anterior.span.end = fim;
            continue;
        }
        saida.push(Comentario {
            span: Span {
                start: inicio,
                end: fim,
            },
            referencias: Vec::new(),
        });
    }
    for c in &mut saida {
        c.referencias = referencias_em(fonte, c.span);
    }
    saida
}

/// Intervalos dos comentários de `fonte` (fora de strings), pelo que o lexer
/// deixa entre os tokens.
fn trechos_de_comentario(fonte: &str) -> Vec<(usize, usize)> {
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
    let mut saida = Vec::new();
    for (de, ate) in lacunas {
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
    }
    saida
}

fn eh_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// As referências `[a]`, `[a.b]`, `[a.b.c]` dentro de `span`, fora de
/// trechos de código (`` `…` `` e blocos cercados por ```` ``` ````). Links
/// Markdown (`[texto](url)`, `[texto][ref]`) não são referências.
fn referencias_em(fonte: &str, span: Span) -> Vec<Vec<(Span, String)>> {
    let b = fonte.as_bytes();
    let mut saida = Vec::new();
    let mut i = span.start;
    let mut em_cerca = false;
    let mut em_codigo = false;
    while i < span.end {
        if b[i..span.end].starts_with(b"```") {
            em_cerca = !em_cerca;
            i += 3;
            continue;
        }
        let c = b[i];
        if c == b'\n' {
            em_codigo = false;
        }
        if c == b'`' && !em_cerca {
            em_codigo = !em_codigo;
            i += 1;
            continue;
        }
        if c != b'[' || em_cerca || em_codigo {
            i += 1;
            continue;
        }
        let Some(fecha) = fonte[i + 1..span.end].find(']').map(|n| i + 1 + n) else {
            break;
        };
        let depois = b.get(fecha + 1).copied();
        let dentro = &fonte[i + 1..fecha];
        // `[texto](url)`, `[texto][rótulo]` e o próprio `[rótulo]` são links.
        let valido = !dentro.is_empty()
            && !matches!(depois, Some(b'(') | Some(b'['))
            && (i == 0 || b[i - 1] != b']')
            && dentro.split('.').all(|p| {
                !p.is_empty() && !p.as_bytes()[0].is_ascii_digit() && p.bytes().all(eh_ident)
            });
        if valido {
            let mut partes = Vec::new();
            let mut inicio = i + 1;
            for parte in dentro.split('.') {
                partes.push((
                    Span {
                        start: inicio,
                        end: inicio + parte.len(),
                    },
                    parte.to_string(),
                ));
                inicio += parte.len() + 1;
            }
            saida.push(partes);
        }
        i = fecha + 1;
    }
    saida
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
        // Link Markdown não é referência.
        let cs = comentarios("/// [texto](http://x) e [ref][r].\nvar x;\n");
        assert!(cs[0].referencias.is_empty());
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
