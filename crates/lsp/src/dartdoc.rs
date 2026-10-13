//! Documentation comments (`///` and `/** … */`): the clean text for hover
//! and for `completionItem/resolve`, and the `[name]`, `[Class.member]` and
//! `[p.Name]` references that rename changes together with the element.
//!
//! Only the lexical scan lives here; resolving each reference (what `[name]`
//! denotes in the scope of the documented declaration) belongs to
//! [`crate::project`], with the tables of the loaded program.

use dartforge_diagnostics::Span;

/// A documentation block: consecutive `///` lines or one `/** */`.
#[derive(Debug, Clone)]
pub(crate) struct Comment {
    /// From the first `/` to the end of the last line (or of the `*/`).
    pub span: Span,
    /// The bracketed references, each with its parts separated by dots (`[A.b]`
    /// has two), with the source range of each part.
    pub references: Vec<Vec<(Span, String)>>,
}

/// The documentation blocks of `source`, in order. Strings and ordinary
/// comments are not mistaken for documentation (the scan uses the lexer).
pub(crate) fn comments(source: &str) -> Vec<Comment> {
    let mut output: Vec<Comment> = Vec::new();
    // Each gap between tokens is a `precedingComments` chain; `findDartDoc`
    // finds the start of the documentation comment in it (the last `/**`, or
    // the first `///` after it), and the comment extends to the chain's last
    // `///` (`DocCommentBuilder.build`).
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

/// The comments of `source` (outside strings) grouped by the gap between
/// tokens, from what the lexer leaves between them.
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
                // Block comments nest in Dart.
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

/// The documentation comment right before `start` (the start of the
/// declaration, with or without metadata), already cleaned: without `///`,
/// `/**`, `*/` and the margin `*`s. `None` when there is none.
pub(crate) fn documentation(source: &str, start: usize) -> Option<String> {
    let start = start.min(source.len());
    // The comment whose end is followed only by whitespace and metadata up to
    // the declaration.
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

/// Removes the comment markers and the common indentation.
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
        // The text of a Markdown link is not a reference; the label of `[t][r]` is
        // (`_parseReferences` reads the `[r]` separately).
        let cs = comments("/// [texto](http://x) e [ref][r].\nvar x;\n");
        let refs: Vec<&str> = cs[0].references.iter().map(|r| r[0].1.as_str()).collect();
        assert_eq!(refs, vec!["r"]);
    }

    #[test]
    fn text_with_multibyte_characters() {
        // Flags and accents (several bytes) before and after the references.
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
