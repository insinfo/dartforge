//! `SnippetTextEdit` das ações de código (docs/LSP-ESPECIFICACAO.md
//! §13.8.1-G): o texto de uma edição na sintaxe de snippet do LSP, com a
//! seleção da mudança como `$0` e os grupos ligados como tab stops.
//!
//! É o porte de `_buildSnippetString` e do `SnippetBuilder`
//! (`AS:src/lsp/snippets.dart:58-137`, `:139-381`) para o caso das ações
//! (`buildSnippetStringForEditGroups`, chamado por `toSnippetTextEdits`,
//! `AS:src/lsp/mapping.dart:1412-1444`): sem tab stops avulsos, só a seleção
//! e os grupos. Os offsets aqui são bytes do texto (o Dart usa unidades
//! UTF-16); o recorte é o mesmo porque todo marcador cai em fronteira de
//! caractere.

use crate::actions::{LinkedGroup, Linked};

/// O texto de uma edição escrito com as marcas do `EditBuilder` do Dart:
/// `addSimpleLinkedEdit` (grupos por nome, na ordem de criação) e
/// `selectHere`/`selectAll`. As posições são relativas ao texto.
#[derive(Debug, Default)]
pub(crate) struct Write {
    pub text: String,
    /// (nome, posições, comprimento, sugestões) por grupo.
    pub groups: Vec<(String, Vec<usize>, usize, Vec<String>)>,
    pub selection: Option<(usize, usize)>,
}

impl Write {
    /// `write`.
    pub fn write(&mut self, t: &str) -> &mut Self {
        self.text.push_str(t);
        self
    }

    /// `addSimpleLinkedEdit(name, t, suggestions)`: o grupo ganha a posição
    /// (texto vazio não cria posição, como no Dart).
    pub fn linked(&mut self, name: &str, t: &str, suggestions: &[String]) -> &mut Self {
        let k = self.text.len();
        self.text.push_str(t);
        if !t.is_empty() {
            match self.groups.iter_mut().find(|g| g.0 == name) {
                Some(g) => {
                    g.1.push(k);
                    g.2 = t.len();
                    g.3.extend(suggestions.iter().cloned());
                }
                None => self.groups.push((name.to_string(), vec![k], t.len(), suggestions.to_vec())),
            }
        }
        self
    }

    /// `selectHere`.
    pub fn here(&mut self) -> &mut Self {
        self.selection = Some((self.text.len(), 0));
        self
    }
}

/// Um marcador (`SnippetPlaceholder`): posição relativa ao texto da edição,
/// comprimento, sugestões (só nos grupos), o grupo e se é o tab stop final.
struct Marker<'a> {
    offset: usize,
    length: usize,
    suggestions: Option<&'a [String]>,
    group: Option<usize>,
    final_: bool,
}

/// O `newText` do `SnippetTextEdit` de uma edição que substitui um trecho por
/// `text`, que começa em `start` no documento final; `linked` traz a
/// seleção e os grupos em offsets do documento final (como o `SourceChange`
/// do Dart). Marcadores fora do texto são descartados.
pub(crate) fn snippet_text(text: &str, start: usize, linked: &Linked) -> String {
    let relative = |p: usize| p.checked_sub(start);
    let mut markers: Vec<Marker<'_>> = Vec::new();
    // A seleção é sempre o número 0 (`isFinal`).
    if let Some((s, n)) = linked.selection {
        // Uma seleção antes da edição fica negativa no Dart e sai no filtro.
        if let Some(offset) = relative(s) {
            markers.push(Marker { offset, length: n, suggestions: None, group: None, final_: true });
        }
    }
    // Sem seleção, com um só grupo e no máximo uma sugestão, as posições do
    // grupo usam o número 0 (`convertEditGroup`, `isFinal`).
    let just_one = linked.selection.is_none() && linked.groups.len() == 1;
    for (index, g) in linked.groups.iter().enumerate() {
        let LinkedGroup { positions, length, suggestions } = g;
        for &p in positions {
            let Some(offset) = relative(p) else { continue };
            markers.push(Marker {
                offset,
                length: *length,
                suggestions: Some(suggestions),
                group: Some(index),
                final_: just_one && suggestions.len() <= 1,
            });
        }
    }
    markers.retain(|m| m.offset + m.length <= text.len() && text.is_char_boundary(m.offset) && text.is_char_boundary(m.offset + m.length));
    // Só os grupos precisam de ordenação (`isPreSorted`); a ordenação é
    // estável, como a do Dart nas listas curtas.
    if !linked.groups.is_empty() {
        markers.sort_by_key(|m| m.offset);
    }
    let mut output = Constructor::default();
    let mut group_number: Vec<(usize, usize)> = Vec::new();
    let mut offset = 0usize;
    for m in &markers {
        // Marcadores sobrepostos: o Dart recortaria de trás para a frente
        // (erro de intervalo); aqui o anterior prevalece.
        if m.offset < offset {
            continue;
        }
        output.text(&text[offset..m.offset]);
        let number = if m.final_ {
            Some(0)
        } else {
            m.group.and_then(|g| group_number.iter().find(|(x, _)| *x == g).map(|(_, n)| *n))
        };
        let snippet = &text[m.offset..m.offset + m.length];
        let used = match m.suggestions {
            None => output.marker(snippet, number),
            Some(suggestions) => {
                // O conjunto (ordem de inserção, sem repetição): o texto
                // atual, se não vazio, e as sugestões.
                let mut choices: Vec<&str> = Vec::new();
                for s in std::iter::once(snippet).filter(|t| !t.is_empty()).chain(suggestions.iter().map(String::as_str)) {
                    if !choices.contains(&s) {
                        choices.push(s);
                    }
                }
                output.choice(&choices, number)
            }
        };
        offset = m.offset + m.length;
        if let Some(g) = m.group
            && !group_number.iter().any(|(x, _)| *x == g)
        {
            group_number.push((g, used));
        }
    }
    output.text(&text[offset..]);
    output.value
}

/// O `SnippetBuilder`: a numeração dos tab stops e os escapes.
struct Constructor {
    value: String,
    next: usize,
}

impl Default for Constructor {
    fn default() -> Self {
        Constructor { value: String::new(), next: 1 }
    }
}

impl Constructor {
    /// `_usePlaceholderNumber`: o número dado ou o próximo livre.
    fn use_(&mut self, number: Option<usize>) -> usize {
        let n = number.unwrap_or(self.next);
        self.next = self.next.max(n + 1);
        n
    }

    /// `appendText`: fora dos marcadores escapam `$` e `\`.
    fn text(&mut self, t: &str) {
        escape(&mut self.value, t, &['$', '\\']);
    }

    /// `appendPlaceholder`: `${n:text}` (escapando `$`, `}` e `\`), ou só
    /// o tab stop `$n` com texto vazio.
    fn marker(&mut self, t: &str, number: Option<usize>) -> usize {
        let n = self.use_(number);
        if t.is_empty() {
            self.value.push_str(&format!("${n}"));
        } else {
            self.value.push_str(&format!("${{{n}:"));
            escape(&mut self.value, t, &['$', '}', '\\']);
            self.value.push('}');
        }
        n
    }

    /// `appendChoice`: com 0 ou 1 escolha, um marcador; senão
    /// `${n|a,b|}` (escapando `\`, `|` e `,`), nunca com o número 0.
    fn choice(&mut self, choices: &[&str], number: Option<usize>) -> usize {
        if choices.len() <= 1 {
            return self.marker(choices.first().copied().unwrap_or(""), number);
        }
        let n = self.use_(number.filter(|&n| n != 0));
        self.value.push_str(&format!("${{{n}|"));
        for (i, e) in choices.iter().enumerate() {
            if i > 0 {
                self.value.push(',');
            }
            escape(&mut self.value, e, &['\\', '|', ',']);
        }
        self.value.push_str("|}");
        n
    }
}

fn escape(output: &mut String, t: &str, special: &[char]) {
    for c in t.chars() {
        if special.contains(&c) {
            output.push('\\');
        }
        output.push(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(positions: &[usize], length: usize, suggestions: &[&str]) -> LinkedGroup {
        LinkedGroup { positions: positions.to_vec(), length, suggestions: suggestions.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn selection_becomes_final_tab_stop() {
        let l = Linked { selection: Some((15, 0)), groups: vec![] };
        assert_eq!(snippet_text(" {\n  return 1;\n}", 10, &l), " {\n  $0return 1;\n}");
        let l = Linked { selection: Some((12, 4)), groups: vec![] };
        assert_eq!(snippet_text("$a ${x} \\b", 9, &l), "\\$a ${0:\\${x\\}} \\\\b");
    }

    #[test]
    fn single_group_without_selection_uses_zero() {
        let l = Linked { selection: None, groups: vec![group(&[4, 19], 3, &[])] };
        assert_eq!(snippet_text("var abc = 1; print(abc);", 0, &l), "var ${0:abc} = 1; print(${0:abc});");
    }

    #[test]
    fn groups_with_suggestions_become_choices() {
        // Numerados pela ordem no texto, não pela de criação; a seleção no
        // fim do texto ainda vale.
        let l = Linked { selection: Some((12, 0)), groups: vec![group(&[4], 1, &["a", "b"]), group(&[0], 3, &[])] };
        assert_eq!(snippet_text("var x = f();", 0, &l), "${1:var} ${2|x,a,b|} = f();$0");
        // Uma só sugestão igual ao texto: degrada para marcador.
        let l = Linked { selection: None, groups: vec![group(&[4], 1, &["x"]), group(&[8], 1, &[])] };
        assert_eq!(snippet_text("var x = y;", 0, &l), "var ${1:x} = ${2:y};");
    }

    #[test]
    fn single_choice_with_several_suggestions_is_not_zero() {
        let l = Linked { selection: None, groups: vec![group(&[0], 3, &["a", "b"])] };
        assert_eq!(snippet_text("int,x", 0, &l), "${1|int,a,b|},x");
        let l = Linked { selection: None, groups: vec![group(&[0], 0, &["a,b", "c|d"])] };
        assert_eq!(snippet_text("", 0, &l), "${1|a\\,b,c\\|d|}");
    }

    #[test]
    fn outside_text_exits() {
        let l = Linked { selection: Some((2, 0)), groups: vec![group(&[100], 3, &[])] };
        assert_eq!(snippet_text("abc", 5, &l), "abc");
    }
}
