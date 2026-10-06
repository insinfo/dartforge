//! `SurroundWith` (surround_with.dart) do `analysis_server` 3.6.2: os
//! comandos selecionados, todos dentro de um bloco, envolvidos por um bloco,
//! um laço, um `if`, um `try` ou um `setState`. As posições ligadas do
//! produtor (`condition`, `item`, `iterable`…) saem como o texto delas.
//!
//! | Título | Espécie |
//! |---|---|
//! | `Surround with block` | `refactor.surround.block` |
//! | `Surround with 'do-while'` | `refactor.surround.doWhile` |
//! | `Surround with 'for'` | `refactor.surround.forEach` |
//! | `Surround with 'for-in'` | `refactor.surround.forIn` |
//! | `Surround with 'if'` | `refactor.surround.if` |
//! | `Surround with 'setState'` | `refactor.surround.setState` |
//! | `Surround with 'try-catch'` | `refactor.surround.tryCatch` |
//! | `Surround with 'try-finally'` | `refactor.surround.tryFinally` |
//! | `Surround with 'while'` | `refactor.surround.while` |

use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::Marca;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Texto, UM_RECUO};
use crate::Edicao;

impl Contexto<'_> {
    /// `ClassElement.isState`: a classe estende, direta ou indiretamente, o
    /// `State` do Flutter (`package:flutter/src/widgets/framework.dart`).
    fn classe_e_state(&self, n: usize) -> bool {
        let Some(declaracao) = self.com_pais(n).find(|&k| self.especie(k) == "ClassDeclaration") else { return false };
        let Marca::Decl(d) = self.arvore.nos[declaracao].marca else { return false };
        let Some(classe) = self.classe_da_declaracao(self.unidade, d) else { return false };
        let prog = self.p.programa();
        let mut atual = prog.class(classe).supertype_class;
        let mut vistos = std::collections::HashSet::new();
        while let Some(c) = atual {
            if !vistos.insert(c) {
                break;
            }
            let ce = prog.class(c);
            if self.p.nome(ce.name) == "State" && prog.library(ce.library).uri == "package:flutter/src/widgets/framework.dart" {
                return true;
            }
            atual = ce.supertype_class;
        }
        false
    }

    /// As assistências de `SurroundWith` para a seleção `[inicio, fim)`.
    pub(crate) fn envolver_comandos(&self, uri: &str, inicio: usize, fim: usize) -> Vec<AcaoDeCodigo> {
        let mut saida = Vec::new();
        // O nó coberto não pode ser a unidade (declarações de topo).
        let Some(no) = self.arvore.localizar(inicio, fim) else { return saida };
        if self.especie(no) == "CompilationUnit" {
            return saida;
        }
        // `StatementAnalyzer`: os nós selecionados, todos comandos de bloco.
        let analise = self.analisar(inicio, fim);
        let selecionados = &analise.selecionados;
        if selecionados.is_empty()
            || !selecionados.iter().all(|&s| self.e_comando(s) && self.pai(s).is_some_and(|p| self.especie(p) == "Block"))
        {
            return saida;
        }
        let (primeiro, ultimo) = (selecionados[0], *selecionados.last().unwrap_or(&selecionados[0]));
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let faixa = tx.faixa_de_linhas(self.arvore.nos[primeiro].inicio, self.arvore.nos[ultimo].fim);
        let velho = self.prefixo_do_no(primeiro);
        let novo = format!("{velho}{UM_RECUO}");
        let recuado = tx.trocar_recuo(&self.fonte[faixa.start..faixa.end], &velho, &novo, true, true);
        let mut acao = |titulo: &str, especie: &str, texto: String| {
            saida.push(AcaoDeCodigo {
                titulo: titulo.into(),
                especie: especie.into(),
                edicoes: vec![Edicao { uri: uri.to_string(), span: faixa, texto }],
                diagnostico: None,
                criar_arquivo: None,
            });
        };
        // `_SurroundWithBlock`: `{` antes, o código recuado e `}` depois.
        acao("Surround with block", "refactor.surround.block", format!("{velho}{{{eol}{recuado}{velho}}}{eol}"));
        acao("Surround with 'do-while'", "refactor.surround.doWhile", format!("{velho}do {{{eol}{recuado}{velho}}} while (condition);{eol}"));
        acao(
            "Surround with 'for'",
            "refactor.surround.forEach",
            format!("{velho}for (var v = init; condition; increment) {{{eol}{recuado}{velho}}}{eol}"),
        );
        acao("Surround with 'for-in'", "refactor.surround.forIn", format!("{velho}for (var item in iterable) {{{eol}{recuado}{velho}}}{eol}"));
        acao("Surround with 'if'", "refactor.surround.if", format!("{velho}if (condition) {{{eol}{recuado}{velho}}}{eol}"));
        if self.pai(no).is_some_and(|p| self.classe_e_state(p)) {
            acao("Surround with 'setState'", "refactor.surround.setState", format!("{velho}setState(() {{{eol}{recuado}{velho}}});{eol}"));
        }
        acao(
            "Surround with 'try-catch'",
            "refactor.surround.tryCatch",
            format!("{velho}try {{{eol}{recuado}{velho}}} on Exception catch (e) {{{eol}{novo}// TODO{eol}{velho}}}{eol}"),
        );
        acao(
            "Surround with 'try-finally'",
            "refactor.surround.tryFinally",
            format!("{velho}try {{{eol}{recuado}{velho}}} finally {{{eol}{novo}// TODO{eol}{velho}}}{eol}"),
        );
        acao("Surround with 'while'", "refactor.surround.while", format!("{velho}while (condition) {{{eol}{recuado}{velho}}}{eol}"));
        saida
    }
}
