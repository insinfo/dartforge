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

use crate::acoes::{GrupoLigado, Ligado};

/// O texto de uma edição escrito com as marcas do `EditBuilder` do Dart:
/// `addSimpleLinkedEdit` (grupos por nome, na ordem de criação) e
/// `selectHere`/`selectAll`. As posições são relativas ao texto.
#[derive(Debug, Default)]
pub(crate) struct Escrita {
    pub texto: String,
    /// (nome, posições, comprimento, sugestões) por grupo.
    pub grupos: Vec<(String, Vec<usize>, usize, Vec<String>)>,
    pub selecao: Option<(usize, usize)>,
}

impl Escrita {
    /// `write`.
    pub fn escrever(&mut self, t: &str) -> &mut Self {
        self.texto.push_str(t);
        self
    }

    /// `addSimpleLinkedEdit(nome, t, suggestions)`: o grupo ganha a posição
    /// (texto vazio não cria posição, como no Dart).
    pub fn ligado(&mut self, nome: &str, t: &str, sugestoes: &[String]) -> &mut Self {
        let k = self.texto.len();
        self.texto.push_str(t);
        if !t.is_empty() {
            match self.grupos.iter_mut().find(|g| g.0 == nome) {
                Some(g) => {
                    g.1.push(k);
                    g.2 = t.len();
                    g.3.extend(sugestoes.iter().cloned());
                }
                None => self.grupos.push((nome.to_string(), vec![k], t.len(), sugestoes.to_vec())),
            }
        }
        self
    }

    /// `selectHere`.
    pub fn aqui(&mut self) -> &mut Self {
        self.selecao = Some((self.texto.len(), 0));
        self
    }
}

/// Um marcador (`SnippetPlaceholder`): posição relativa ao texto da edição,
/// comprimento, sugestões (só nos grupos), o grupo e se é o tab stop final.
struct Marcador<'a> {
    offset: usize,
    comprimento: usize,
    sugestoes: Option<&'a [String]>,
    grupo: Option<usize>,
    final_: bool,
}

/// O `newText` do `SnippetTextEdit` de uma edição que substitui um trecho por
/// `texto`, que começa em `inicio` no documento final; `ligado` traz a
/// seleção e os grupos em offsets do documento final (como o `SourceChange`
/// do Dart). Marcadores fora do texto são descartados.
pub(crate) fn texto_de_snippet(texto: &str, inicio: usize, ligado: &Ligado) -> String {
    let relativo = |p: usize| p.checked_sub(inicio);
    let mut marcadores: Vec<Marcador<'_>> = Vec::new();
    // A seleção é sempre o número 0 (`isFinal`).
    if let Some((s, n)) = ligado.selecao {
        // Uma seleção antes da edição fica negativa no Dart e sai no filtro.
        if let Some(offset) = relativo(s) {
            marcadores.push(Marcador { offset, comprimento: n, sugestoes: None, grupo: None, final_: true });
        }
    }
    // Sem seleção, com um só grupo e no máximo uma sugestão, as posições do
    // grupo usam o número 0 (`convertEditGroup`, `isFinal`).
    let um_so = ligado.selecao.is_none() && ligado.grupos.len() == 1;
    for (indice, g) in ligado.grupos.iter().enumerate() {
        let GrupoLigado { posicoes, comprimento, sugestoes } = g;
        for &p in posicoes {
            let Some(offset) = relativo(p) else { continue };
            marcadores.push(Marcador {
                offset,
                comprimento: *comprimento,
                sugestoes: Some(sugestoes),
                grupo: Some(indice),
                final_: um_so && sugestoes.len() <= 1,
            });
        }
    }
    marcadores.retain(|m| m.offset + m.comprimento <= texto.len() && texto.is_char_boundary(m.offset) && texto.is_char_boundary(m.offset + m.comprimento));
    // Só os grupos precisam de ordenação (`isPreSorted`); a ordenação é
    // estável, como a do Dart nas listas curtas.
    if !ligado.grupos.is_empty() {
        marcadores.sort_by_key(|m| m.offset);
    }
    let mut saida = Construtor::default();
    let mut numero_do_grupo: Vec<(usize, usize)> = Vec::new();
    let mut offset = 0usize;
    for m in &marcadores {
        // Marcadores sobrepostos: o Dart recortaria de trás para a frente
        // (erro de intervalo); aqui o anterior prevalece.
        if m.offset < offset {
            continue;
        }
        saida.texto(&texto[offset..m.offset]);
        let numero = if m.final_ {
            Some(0)
        } else {
            m.grupo.and_then(|g| numero_do_grupo.iter().find(|(x, _)| *x == g).map(|(_, n)| *n))
        };
        let trecho = &texto[m.offset..m.offset + m.comprimento];
        let usado = match m.sugestoes {
            None => saida.marcador(trecho, numero),
            Some(sugestoes) => {
                // O conjunto (ordem de inserção, sem repetição): o texto
                // atual, se não vazio, e as sugestões.
                let mut escolhas: Vec<&str> = Vec::new();
                for s in std::iter::once(trecho).filter(|t| !t.is_empty()).chain(sugestoes.iter().map(String::as_str)) {
                    if !escolhas.contains(&s) {
                        escolhas.push(s);
                    }
                }
                saida.escolha(&escolhas, numero)
            }
        };
        offset = m.offset + m.comprimento;
        if let Some(g) = m.grupo
            && !numero_do_grupo.iter().any(|(x, _)| *x == g)
        {
            numero_do_grupo.push((g, usado));
        }
    }
    saida.texto(&texto[offset..]);
    saida.valor
}

/// O `SnippetBuilder`: a numeração dos tab stops e os escapes.
struct Construtor {
    valor: String,
    proximo: usize,
}

impl Default for Construtor {
    fn default() -> Self {
        Construtor { valor: String::new(), proximo: 1 }
    }
}

impl Construtor {
    /// `_usePlaceholderNumber`: o número dado ou o próximo livre.
    fn usar(&mut self, numero: Option<usize>) -> usize {
        let n = numero.unwrap_or(self.proximo);
        self.proximo = self.proximo.max(n + 1);
        n
    }

    /// `appendText`: fora dos marcadores escapam `$` e `\`.
    fn texto(&mut self, t: &str) {
        escapar(&mut self.valor, t, &['$', '\\']);
    }

    /// `appendPlaceholder`: `${n:texto}` (escapando `$`, `}` e `\`), ou só
    /// o tab stop `$n` com texto vazio.
    fn marcador(&mut self, t: &str, numero: Option<usize>) -> usize {
        let n = self.usar(numero);
        if t.is_empty() {
            self.valor.push_str(&format!("${n}"));
        } else {
            self.valor.push_str(&format!("${{{n}:"));
            escapar(&mut self.valor, t, &['$', '}', '\\']);
            self.valor.push('}');
        }
        n
    }

    /// `appendChoice`: com 0 ou 1 escolha, um marcador; senão
    /// `${n|a,b|}` (escapando `\`, `|` e `,`), nunca com o número 0.
    fn escolha(&mut self, escolhas: &[&str], numero: Option<usize>) -> usize {
        if escolhas.len() <= 1 {
            return self.marcador(escolhas.first().copied().unwrap_or(""), numero);
        }
        let n = self.usar(numero.filter(|&n| n != 0));
        self.valor.push_str(&format!("${{{n}|"));
        for (i, e) in escolhas.iter().enumerate() {
            if i > 0 {
                self.valor.push(',');
            }
            escapar(&mut self.valor, e, &['\\', '|', ',']);
        }
        self.valor.push_str("|}");
        n
    }
}

fn escapar(saida: &mut String, t: &str, especiais: &[char]) {
    for c in t.chars() {
        if especiais.contains(&c) {
            saida.push('\\');
        }
        saida.push(c);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn grupo(posicoes: &[usize], comprimento: usize, sugestoes: &[&str]) -> GrupoLigado {
        GrupoLigado { posicoes: posicoes.to_vec(), comprimento, sugestoes: sugestoes.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn selecao_vira_tab_stop_final() {
        let l = Ligado { selecao: Some((15, 0)), grupos: vec![] };
        assert_eq!(texto_de_snippet(" {\n  return 1;\n}", 10, &l), " {\n  $0return 1;\n}");
        let l = Ligado { selecao: Some((12, 4)), grupos: vec![] };
        assert_eq!(texto_de_snippet("$a ${x} \\b", 9, &l), "\\$a ${0:\\${x\\}} \\\\b");
    }

    #[test]
    fn grupo_unico_sem_selecao_usa_o_zero() {
        let l = Ligado { selecao: None, grupos: vec![grupo(&[4, 19], 3, &[])] };
        assert_eq!(texto_de_snippet("var abc = 1; print(abc);", 0, &l), "var ${0:abc} = 1; print(${0:abc});");
    }

    #[test]
    fn grupos_com_sugestoes_viram_escolhas() {
        // Numerados pela ordem no texto, não pela de criação; a seleção no
        // fim do texto ainda vale.
        let l = Ligado { selecao: Some((12, 0)), grupos: vec![grupo(&[4], 1, &["a", "b"]), grupo(&[0], 3, &[])] };
        assert_eq!(texto_de_snippet("var x = f();", 0, &l), "${1:var} ${2|x,a,b|} = f();$0");
        // Uma só sugestão igual ao texto: degrada para marcador.
        let l = Ligado { selecao: None, grupos: vec![grupo(&[4], 1, &["x"]), grupo(&[8], 1, &[])] };
        assert_eq!(texto_de_snippet("var x = y;", 0, &l), "var ${1:x} = ${2:y};");
    }

    #[test]
    fn escolha_unica_com_varias_sugestoes_nao_e_zero() {
        let l = Ligado { selecao: None, grupos: vec![grupo(&[0], 3, &["a", "b"])] };
        assert_eq!(texto_de_snippet("int,x", 0, &l), "${1|int,a,b|},x");
        let l = Ligado { selecao: None, grupos: vec![grupo(&[0], 0, &["a,b", "c|d"])] };
        assert_eq!(texto_de_snippet("", 0, &l), "${1|a\\,b,c\\|d|}");
    }

    #[test]
    fn fora_do_texto_sai() {
        let l = Ligado { selecao: Some((2, 0)), grupos: vec![grupo(&[100], 3, &[])] };
        assert_eq!(texto_de_snippet("abc", 5, &l), "abc");
    }
}
