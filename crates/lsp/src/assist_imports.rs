//! Assistências de diretivas de import, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Add explicit 'show' combinator` | `refactor.add.showCombinator` | `ImportAddShow` |
//! | `Convert to 'package:' import` | `refactor.convert.relativeToPackageImport` | `ConvertToPackageImport` |
//! | `Convert to a relative import` | `refactor.convert.packageToRelativeImport` | `ConvertToRelativeImport` |
//! | `Convert to use a URI` | `refactor.convert.partOfToPartUri` | `ConvertPartOfToUri` |

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

    /// A diretiva de import do nó da seleção (o literal da URI sobe para ela)
    /// e o índice dela na unidade.
    fn import_da_selecao(&self, inicio: usize, fim: usize) -> Option<(usize, usize)> {
        let mut no = self.arvore.localizar(inicio, fim)?;
        if matches!(self.especie(no), "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings") {
            no = self.pai(no)?;
        }
        if self.especie(no) != "ImportDirective" {
            return None;
        }
        let span = self.arvore.span(no);
        let unidade = self.p.programa().unit(self.unidade);
        let indice = unidade.unit.directives.iter().position(|d| d.span.start <= span.start && span.start < d.span.end)?;
        Some((no, indice))
    }

    /// O literal da URI de uma diretiva (o texto e o intervalo dele).
    fn uri_da_diretiva(&self, indice: usize) -> Option<(String, Span)> {
        let unidade = self.p.programa().unit(self.unidade);
        let DirectiveKind::Import { uri, .. } = &unidade.unit.directives.get(indice)?.kind else { return None };
        Some((dartforge_elements::load::string_lit_value(uri)?, uri.span))
    }

    /// `ConvertToPackageImport` (convert_to_package_import.dart): o import
    /// escrito sem `package:` de uma biblioteca cuja URI é `package:` passa a
    /// citá-la por ela.
    pub(crate) fn converter_em_import_de_pacote(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let (_, indice) = self.import_da_selecao(inicio, fim)?;
        let prog = self.p.programa();
        let imp = prog.library(prog.unit(self.unidade).library).imports.iter().find(|i| i.unit == self.unidade && i.directive == indice)?;
        let canonica = &prog.library(imp.library).uri;
        if !canonica.starts_with("package:") {
            return None;
        }
        let (escrita, faixa) = self.uri_da_diretiva(indice)?;
        if escrita.starts_with("package:") {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to 'package:' import".into(),
            especie: "refactor.convert.relativeToPackageImport".into(),
            edicoes: vec![Edicao { uri: uri.to_string(), span: faixa, texto: format!("'{canonica}'") }],
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `ConvertToRelativeImport` (convert_to_relative_import.dart): o import
    /// `package:` do mesmo pacote que a unidade vira caminho relativo.
    pub(crate) fn converter_em_import_relativo(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let (_, indice) = self.import_da_selecao(inicio, fim)?;
        let prog = self.p.programa();
        // `libraryImport?.uri is DirectiveUriWithSource`: o import tem alvo.
        prog.library(prog.unit(self.unidade).library).imports.iter().find(|i| i.unit == self.unidade && i.directive == indice)?;
        let fonte_uri = prog.unit(self.unidade).uri.strip_prefix("package:")?.to_string();
        let (escrita, faixa) = self.uri_da_diretiva(indice)?;
        let importada = escrita.strip_prefix("package:")?;
        let segmentos_fonte: Vec<&str> = fonte_uri.split('/').collect();
        let segmentos_import: Vec<&str> = importada.split('/').collect();
        if segmentos_fonte.is_empty() || segmentos_import.is_empty() || segmentos_fonte[0] != segmentos_import[0] {
            return None;
        }
        // `path.posix.relative(importUri.path, from: dirname(sourceUri.path))`.
        let de: Vec<&str> = segmentos_fonte[..segmentos_fonte.len() - 1].to_vec();
        let comum = de.iter().zip(&segmentos_import).take_while(|(a, b)| a == b).count();
        let mut partes: Vec<&str> = vec![".."; de.len() - comum];
        partes.extend(&segmentos_import[comum..]);
        let relativo = if partes.is_empty() { ".".to_string() } else { partes.join("/") };
        Some(AcaoDeCodigo {
            titulo: "Convert to a relative import".into(),
            especie: "refactor.convert.packageToRelativeImport".into(),
            edicoes: vec![Edicao { uri: uri.to_string(), span: Span { start: faixa.start + 1, end: faixa.end - 1 }, texto: relativo }],
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `ConvertPartOfToUri` (convert_part_of_to_uri.dart): `part of nome;`
    /// passa a citar a biblioteca pelo caminho relativo do arquivo dela.
    pub(crate) fn converter_part_of_em_uri(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let diretiva = self.com_pais(no).find(|&k| self.especie(k) == "PartOfDirective")?;
        let nome = self.filhos(diretiva).iter().copied().find(|&k| self.especie(k) == "LibraryIdentifier" || self.especie(k) == "SimpleIdentifier")?;
        let prog = self.p.programa();
        let biblioteca = prog.library(prog.unit(self.unidade).library);
        let definidora = biblioteca.units.first().map(|&u| prog.unit(u))?;
        let caminho_lib = definidora.path.as_ref()?;
        let caminho_parte = prog.unit(self.unidade).path.as_ref()?;
        let dir = caminho_parte.parent()?;
        // `pathContext.relative` e `toUri` (barras normais, espaços escapados).
        let a: Vec<String> = dir.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        let b: Vec<String> = caminho_lib.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        let comum = a.iter().zip(&b).take_while(|(x, y)| x.eq_ignore_ascii_case(y)).count();
        let mut partes: Vec<String> = vec!["..".to_string(); a.len() - comum];
        partes.extend(b[comum..].iter().cloned());
        let relativo = partes.join("/").replace(' ', "%20");
        Some(AcaoDeCodigo {
            titulo: "Convert to use a URI".into(),
            especie: "refactor.convert.partOfToPartUri".into(),
            edicoes: vec![Edicao { uri: uri.to_string(), span: self.arvore.span(nome), texto: format!("'{relativo}'") }],
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}
