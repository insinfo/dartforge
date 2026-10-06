//! Assistências de diretivas de import, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Add explicit 'show' combinator` | `refactor.add.showCombinator` | `ImportAddShow` |

use crate::acoes::AcaoDeCodigo;
use crate::refatoracoes::Contexto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_elements::model::Element;
use dartforge_frontend::ast::DirectiveKind;
use dartforge_types::Resolved;

impl Contexto<'_> {
    /// `ImportAddShow` (import_add_show.dart): o import ancestral do nó da
    /// seleção, sem combinadores, ganha `show` com os nomes que a unidade usa
    /// dele (o `_ReferenceFinder`, em ordem de texto), antes do `;`.
    pub(crate) fn import_add_show(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let diretiva = self.com_pais(no).find(|&k| self.especie(k) == "ImportDirective")?;
        let span = self.arvore.span(diretiva);
        let prog = self.p.programa();
        let unidade = prog.unit(self.unidade);
        let indice = unidade.unit.directives.iter().position(|d| d.span.start <= span.start && span.start < d.span.end)?;
        let DirectiveKind::Import { combinators, .. } = &unidade.unit.directives[indice].kind else { return None };
        if !combinators.is_empty() {
            return None;
        }
        let imp = prog.library(unidade.library).imports.iter().find(|i| i.unit == self.unidade && i.directive == indice)?;
        let mut nomes: Vec<String> = dartforge_analise::importacoes::nomes_usados_do_import(prog, self.unidade, indice, &self.p.consulta.nomes)?
            .into_iter()
            .map(|s| self.p.nome(s).to_string())
            .collect();
        // `_addImplicitExtensionName`: a extensão do membro resolvido (método,
        // propriedade, operador, índice, `call`), se o namespace do import a
        // tem sob o nome dela; o import com prefixo indexa por `p.x`.
        if imp.prefix.is_none() {
            let exportado = &prog.library(imp.library).exported;
            for r in self.corpos.resolved.iter().flatten() {
                let Resolved::ExtensionMember { extension, .. } = r else { continue };
                let Some(nome) = prog.extension(*extension).name else { continue };
                if exportado.get(&nome).and_then(|b| b.getter) == Some(Element::Extension(*extension)) {
                    nomes.push(self.p.nome(nome).to_string());
                }
            }
        }
        // `SplayTreeSet<String>`: sem repetição, na ordem do `compareTo` do
        // Dart (unidades UTF-16).
        nomes.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        nomes.dedup();
        if nomes.is_empty() {
            return None;
        }
        // `importDirective.end - 1`: antes do `;`.
        let em = span.end.checked_sub(1)?;
        Some(AcaoDeCodigo {
            titulo: "Add explicit 'show' combinator".into(),
            especie: "refactor.add.showCombinator".into(),
            edicoes: vec![Edicao { uri: uri.to_string(), span: Span { start: em, end: em }, texto: format!(" show {}", nomes.join(", ")) }],
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}
