//! Assistências de laços, portadas dos produtores do `analysis_server`
//! 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to for-index loop` | `refactor.convert.forEachToForIndex` | `ConvertIntoForIndex` |

use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::Marca;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Texto, UM_RECUO};
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_types::{Resolved, Type};

/// O `toSource` de um `DeclaredIdentifier` (metadados, palavra-chave, tipo e
/// nome separados por um espaço), a partir do texto: os brancos viram um
/// espaço, e os de dentro dos argumentos de tipo somem como no
/// `ToSourceVisitor` (`<`, `>` e `, `).
pub(crate) fn como_fonte(texto: &str) -> String {
    let mut s = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    for (de, para) in [("< ", "<"), (" <", "<"), (" >", ">"), (" ,", ","), (" ?", "?")] {
        while s.contains(de) {
            s = s.replace(de, para);
        }
    }
    s
}

impl Contexto<'_> {
    /// `ConvertIntoForIndex` (convert_into_for_index.dart): `for (final x in
    /// lista) { … }` sobre uma variável local ou parâmetro de tipo `List`
    /// vira `for (int i = 0; i < lista.length; i++)` com `final x =
    /// lista[i];` no começo do bloco.
    pub(crate) fn converter_em_for_com_indice(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let laco = self.com_pais(no).find(|&k| self.especie(k) == "ForStatement")?;
        let partes = self.filhos(laco).iter().copied().find(|&k| self.especie(k).starts_with("ForEachParts"))?;
        let corpo = *self.filhos(laco).last()?;
        // `selectionOffset < forStatement.offset || rightParenthesis.end < selectionOffset`.
        let fecha = self.token_anterior(self.arvore.nos[corpo].inicio)?;
        if inicio < self.arvore.nos[laco].inicio || fecha.end < inicio {
            return None;
        }
        if self.especie(partes) != "ForEachPartsWithDeclaration" {
            return None;
        }
        let filhos = self.filhos(partes);
        let variavel = *filhos.iter().find(|&&k| self.especie(k) == "DeclaredIdentifier")?;
        let iteravel = *filhos.last()?;
        if self.especie(iteravel) != "SimpleIdentifier" {
            return None;
        }
        // `iterable.element is VariableElement2`: local ou parâmetro (o
        // campo e a variável de topo resolvem para o getter).
        let Marca::Expr(x) = self.arvore.nos[iteravel].marca else { return None };
        if !matches!(self.corpos.get_resolved(x), Some(Resolved::Local(_) | Resolved::Parameter { .. })) {
            return None;
        }
        let lista = self.p.consulta.core.list_class?;
        let tipo = *self.corpos.static_types.get(x.0 as usize)?;
        if !matches!(self.p.consulta.tabela.get(tipo), Type::Interface { class, .. } if *class == lista) {
            return None;
        }
        if self.especie(corpo) != "Block" {
            return None;
        }
        let conflitos = self.conflitos_de_local(self.arvore.nos[laco].inicio);
        let indice = ["i", "j", "k"].into_iter().find(|n| !conflitos.contains(*n))?;
        let nome_da_lista = self.texto_do_no(iteravel).to_string();
        let tx = Texto::novo(self.fonte);
        let prefixo = self.prefixo_do_no(laco);
        // `getLineContentEnd(body.leftBracket.end)`.
        let primeira_linha = tx.fim_do_conteudo(self.arvore.nos[corpo].inicio + 1);
        let variavel_fonte = como_fonte(self.texto_do_no(variavel));
        let eol = tx.eol();
        Some(AcaoDeCodigo {
            titulo: "Convert to for-index loop".into(),
            especie: "refactor.convert.forEachToForIndex".into(),
            edicoes: vec![
                Edicao {
                    uri: uri.to_string(),
                    span: Span { start: self.arvore.nos[laco].inicio, end: fecha.end },
                    texto: format!("for (int {indice} = 0; {indice} < {nome_da_lista}.length; {indice}++)"),
                },
                Edicao {
                    uri: uri.to_string(),
                    span: Span { start: primeira_linha, end: primeira_linha },
                    texto: format!("{prefixo}{UM_RECUO}{variavel_fonte} = {nome_da_lista}[{indice}];{eol}"),
                },
            ],
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}
