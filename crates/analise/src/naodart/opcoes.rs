//! A validação do `analysis_options.yaml`: `analyzeAnalysisOptions` e os
//! validadores de `analyzer/lib/src/task/options.dart` e
//! `analyzer/lib/src/lint/options_rule_validator.dart`, lidos na 6.11.0
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.10).
//!
//! Cobre: o erro de sintaxe do YAML, a cadeia de `include:` (não achado,
//! recursivo, avisos e erro de sintaxe do incluído, relatados no `include:`
//! do arquivo inicial), as seções `analyzer` (chaves, `strong-mode`,
//! `errors`, `language`, `optional-checks`, `cannot-ignore`, `plugins`),
//! `code-style`, `formatter`, `linter` e as regras de `linter: rules:`.
//!
//! Diferenças: o texto do erro de sintaxe é o do `yaml-rust2`, não o do
//! `package:yaml`, e o intervalo dele tem comprimento zero; os nomes de
//! `enable-experiment` não são conferidos (só o formato da seção); uma regra
//! removida sai sempre como `removed_lint`, porque a tabela de regras não
//! guarda a substituta (`replaced_lint`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g::opcoes as c;
use super::nomes_g::NOMES_DE_CODIGO;
use super::yaml::{self, No, Valor};
use super::{CodigoNaoDart, Relato};
use crate::lints::{self, EstadoDaRegra};
use dartforge_diagnostics::Span;
use std::path::{Path, PathBuf};

/// O que a validação precisa saber de fora do arquivo.
pub struct Contexto<'a> {
    /// O arquivo de opções inicial.
    pub arquivo: &'a Path,
    /// A raiz do contexto de análise, para a mensagem do include não achado.
    pub raiz_do_contexto: &'a str,
    /// `sourceFactory.resolveUri(de, uri)`: o arquivo de um `include:`.
    pub resolver: &'a dyn Fn(&Path, &str) -> Option<PathBuf>,
    /// `sdkVersionConstraint.allows(versão)`; `None` sem restrição de SDK.
    pub sdk_permite: Option<&'a dyn Fn((u32, u32, u32)) -> bool>,
}

/// `quotedAndCommaSeparatedWithAnd`.
fn entre_aspas(itens: &[&str]) -> String {
    let citados: Vec<String> = itens.iter().map(|i| format!("'{i}'")).collect();
    match citados.len() {
        0 => String::new(),
        1 => citados[0].clone(),
        2 => format!("{} and {}", citados[0], citados[1]),
        n => format!("{}, and {}", citados[..n - 1].join(", "), citados[n - 1]),
    }
}

/// O valor de um escalar como texto (`value.toString()`); `None` para nulo,
/// lista e mapa.
fn texto_de(no: &No) -> Option<&str> {
    match &no.valor {
        Valor::Texto(t) | Valor::Outro(t) => Some(t),
        _ => None,
    }
}

const TOPO_DO_ANALYZER: [&str; 8] =
    ["cannot-ignore", "enable-experiment", "errors", "exclude", "language", "optional-checks", "plugins", "strong-mode"];
const DE_STRONG_MODE: [&str; 3] = ["declaration-casts", "implicit-casts", "implicit-dynamic"];
const DE_LINGUAGEM: [&str; 3] = ["strict-casts", "strict-inference", "strict-raw-types"];
const DE_CHECAGENS: [&str; 2] = ["chrome-os-manifest-checks", "propagate-linter-exceptions"];
const VERDADEIRO_OU_FALSO: [&str; 2] = ["true", "false"];
const SEVERIDADES: [&str; 3] = ["error", "info", "warning"];
const VALORES_DE_ERRO: [&str; 7] = ["ignore", "false", "include", "true", "error", "info", "warning"];

/// O nome (em maiúsculas) é de um código de diagnóstico, de uma regra de
/// lint, ou do código removido que ainda se aceita.
fn codigo_conhecido(maiusculo: &str) -> bool {
    NOMES_DE_CODIGO.binary_search(&maiusculo).is_ok()
        || lints::tabela_g::REGRAS.iter().any(|r| r.nome.eq_ignore_ascii_case(maiusculo))
        || maiusculo == "MISSING_RETURN"
}

struct Validador<'a> {
    out: Vec<Relato>,
    sdk_permite: Option<&'a dyn Fn((u32, u32, u32)) -> bool>,
    e_o_da_raiz: bool,
}

impl Validador<'_> {
    fn relatar(&mut self, codigo: &'static CodigoNaoDart, no: &No, args: &[&str]) {
        self.out.push(Relato::novo(codigo, no.span, args));
    }

    /// `ErrorBuilder.reportError`: a opção não suportada, com a proposta.
    fn nao_suportada(&mut self, escopo: &str, no: &No, suportadas: &[&str]) {
        let Some(valor) = texto_de(no) else { return };
        let proposta = entre_aspas(suportadas);
        match suportadas.len() {
            0 => self.relatar(&c::UNSUPPORTED_OPTION_WITHOUT_VALUES, no, &[escopo, valor]),
            1 => self.relatar(&c::UNSUPPORTED_OPTION_WITH_LEGAL_VALUE, no, &[escopo, valor, proposta.as_str()]),
            _ => self.relatar(&c::UNSUPPORTED_OPTION_WITH_LEGAL_VALUES, no, &[escopo, valor, proposta.as_str()]),
        }
    }

    /// `TopLevelOptionValidator`: as chaves de uma seção de topo.
    fn chaves_de_topo(&mut self, opcoes: &No, secao: &str, suportadas: &[&str]) {
        let Some(pares) = opcoes.campo(secao).and_then(No::mapa) else { return };
        for (k, _) in pares {
            if k.escalar() && !texto_de(k).is_some_and(|t| suportadas.contains(&t)) {
                self.nao_suportada(secao, k, suportadas);
            }
        }
    }

    /// Um valor que tem de ser `true` ou `false`.
    fn booleano(&mut self, chave: &str, v: &No) {
        if !v.escalar() {
            return;
        }
        let ok = texto_de(v).is_some_and(|t| VERDADEIRO_OU_FALSO.contains(&t.to_lowercase().as_str()));
        if !ok && let Some(valor) = texto_de(v) {
            let proposta = entre_aspas(&VERDADEIRO_OU_FALSO);
            self.relatar(&c::UNSUPPORTED_VALUE, v, &[chave, valor, proposta.as_str()]);
        }
    }

    /// `StrongModeOptionValueValidator`.
    fn strong_mode(&mut self, analyzer: &No) {
        let Some(no) = analyzer.campo("strong-mode") else { return };
        let Some(pares) = no.mapa() else {
            self.relatar(&c::INVALID_SECTION_FORMAT, no, &["strong-mode"]);
            return;
        };
        for (k, v) in pares {
            if !k.escalar() {
                continue;
            }
            match texto_de(k) {
                Some(chave) if DE_STRONG_MODE.contains(&chave) => {
                    if chave == "declaration-casts" {
                        if let Some(valor) = texto_de(v) {
                            let proposta = entre_aspas(&VERDADEIRO_OU_FALSO);
                            self.relatar(&c::UNSUPPORTED_VALUE, v, &["strong-mode", valor, proposta.as_str()]);
                        }
                    } else {
                        self.booleano(chave, v);
                    }
                }
                _ => self.nao_suportada("strong-mode", k, &DE_STRONG_MODE),
            }
        }
    }

    /// `ErrorFilterOptionValidator`. O argumento do formato inválido é
    /// `enable-experiment`, como no original.
    fn filtros_de_erro(&mut self, analyzer: &No) {
        let Some(no) = analyzer.campo("errors") else { return };
        let Some(pares) = no.mapa() else {
            self.relatar(&c::INVALID_SECTION_FORMAT, no, &["enable-experiment"]);
            return;
        };
        for (k, v) in pares {
            if k.escalar()
                && let Some(nome) = texto_de(k)
                && !codigo_conhecido(&nome.to_uppercase())
            {
                self.relatar(&c::UNRECOGNIZED_ERROR_CODE, k, &[nome]);
            }
            if v.escalar() {
                let valor = texto_de(v).unwrap_or("null");
                if !VALORES_DE_ERRO.contains(&valor.to_lowercase().as_str()) {
                    let proposta = entre_aspas(&VALORES_DE_ERRO);
                    self.relatar(&c::UNSUPPORTED_OPTION_WITH_LEGAL_VALUES, v, &["errors", valor, proposta.as_str()]);
                }
            } else {
                self.relatar(&c::INVALID_SECTION_FORMAT, v, &["enable-experiment"]);
            }
        }
    }

    /// `EnabledExperimentsValidator`: só o formato da seção.
    fn experimentos(&mut self, analyzer: &No) {
        if let Some(no) = analyzer.campo("enable-experiment")
            && no.lista().is_none()
        {
            self.relatar(&c::INVALID_SECTION_FORMAT, no, &["enable-experiment"]);
        }
    }

    /// `LanguageOptionValidator`.
    fn linguagem(&mut self, analyzer: &No) {
        let Some(no) = analyzer.campo("language") else { return };
        let Some(pares) = no.mapa() else {
            // Um escalar não nulo ou uma lista.
            if !no.nulo() {
                self.relatar(&c::INVALID_SECTION_FORMAT, no, &["language"]);
            }
            return;
        };
        for (k, v) in pares {
            if !k.escalar() {
                continue;
            }
            match texto_de(k) {
                Some(chave) if DE_LINGUAGEM.contains(&chave) => self.booleano(chave, v),
                _ => self.nao_suportada("language", k, &DE_LINGUAGEM),
            }
        }
    }

    /// `OptionalChecksValueValidator`.
    fn checagens_opcionais(&mut self, analyzer: &No) {
        let Some(no) = analyzer.campo("optional-checks") else { return };
        let escopo = "chrome-os-manifest-checks";
        if no.escalar() {
            if texto_de(no).map(str::to_lowercase).as_deref() != Some(escopo) {
                self.nao_suportada(escopo, no, &DE_CHECAGENS);
            }
        } else if let Some(pares) = no.mapa() {
            for (k, v) in pares {
                if !k.escalar() {
                    continue;
                }
                if texto_de(k) == Some(escopo) {
                    self.booleano(escopo, v);
                } else {
                    self.nao_suportada(escopo, k, &DE_CHECAGENS);
                }
            }
        } else {
            self.relatar(&c::INVALID_SECTION_FORMAT, no, &["enable-experiment"]);
        }
    }

    /// `CannotIgnoreOptionValidator`.
    fn nao_ignoraveis(&mut self, analyzer: &No) {
        let Some(no) = analyzer.campo("cannot-ignore") else { return };
        let Some(itens) = no.lista() else {
            self.relatar(&c::INVALID_SECTION_FORMAT, no, &["cannot-ignore"]);
            return;
        };
        for item in itens {
            match item.texto() {
                Some(nome) => {
                    if !SEVERIDADES.contains(&nome) && !codigo_conhecido(&nome.to_uppercase()) {
                        self.relatar(&c::UNRECOGNIZED_ERROR_CODE, item, &[nome]);
                    }
                }
                None => self.relatar(&c::INVALID_SECTION_FORMAT, item, &["cannot-ignore"]),
            }
        }
    }

    /// `CodeStyleOptionsValidator`.
    fn estilo_de_codigo(&mut self, opcoes: &No, fonte: &str) {
        let Some(no) = opcoes.campo("code-style") else { return };
        let Some(pares) = no.mapa() else {
            if !no.nulo() {
                self.relatar(&c::INVALID_SECTION_FORMAT, no, &["code-style"]);
            }
            return;
        };
        for (k, v) in pares {
            if texto_de(k) == Some("format") {
                if !v.escalar() {
                    self.relatar(&c::INVALID_SECTION_FORMAT, v, &["format"]);
                } else {
                    self.booleano("format", v);
                }
            } else {
                let chave = fonte.get(k.span.start..k.span.end).unwrap_or("");
                self.relatar(&c::UNSUPPORTED_OPTION_WITHOUT_VALUES, k, &["code-style", chave]);
            }
        }
    }

    /// `FormatterOptionsValidator`.
    fn formatador(&mut self, opcoes: &No, fonte: &str) {
        let Some(no) = opcoes.campo("formatter") else { return };
        let Some(pares) = no.mapa() else {
            if !no.nulo() {
                self.relatar(&c::INVALID_SECTION_FORMAT, no, &["formatter"]);
            }
            return;
        };
        for (k, v) in pares {
            let chave = fonte.get(k.span.start..k.span.end).unwrap_or("");
            if texto_de(k) == Some("page_width") {
                let positivo = matches!(&v.valor, Valor::Outro(t) if t.parse::<i64>().is_ok_and(|n| n > 0));
                if !positivo {
                    self.relatar(&c::INVALID_OPTION, v, &[chave, "\"page_width\" must be a positive integer."]);
                }
            } else {
                self.relatar(&c::UNSUPPORTED_OPTION_WITHOUT_VALUES, k, &["formatter", chave]);
            }
        }
    }

    /// `LinterRuleOptionsValidator._validateRules`.
    fn regras(&mut self, opcoes: &No) {
        let Some(regras) = opcoes.campo("linter").filter(|l| l.mapa().is_some()).and_then(|l| l.campo("rules")) else { return };
        let mut vistas: Vec<&str> = Vec::new();
        let entradas: Vec<(&No, bool)> = match &regras.valor {
            Valor::Lista(itens) => itens.iter().map(|n| (n, true)).collect(),
            Valor::Mapa(pares) => pares.iter().map(|(k, v)| (k, matches!(&v.valor, Valor::Outro(t) if t.eq_ignore_ascii_case("true")))).collect(),
            _ => Vec::new(),
        };
        for (no, ligada) in entradas {
            let Some(nome) = texto_de(no) else { continue };
            let Some(regra) = lints::regra(nome) else {
                self.relatar(&c::UNDEFINED_LINT, no, &[nome]);
                continue;
            };
            if ligada {
                if let Some(outra) = regra.incompativeis.iter().copied().find(|i| vistas.contains(i)) {
                    self.relatar(&c::INCOMPATIBLE_LINT, no, &[nome, outra]);
                } else if vistas.contains(&regra.nome) {
                    self.relatar(&c::DUPLICATE_RULE, no, &[nome]);
                } else {
                    vistas.push(regra.nome);
                }
            }
            if self.e_o_da_raiz {
                // `currentSdkAllows`: sem `since`, sim; sem restrição, não.
                let no_sdk = match regra.desde {
                    None => true,
                    Some(v) => self.sdk_permite.is_some_and(|f| f(v)),
                };
                match regra.estado {
                    EstadoDaRegra::Depreciada if no_sdk => self.relatar(&c::DEPRECATED_LINT, no, &[nome]),
                    EstadoDaRegra::Removida if no_sdk => {
                        let desde = regra.desde.map_or_else(|| "null".to_string(), |(a, b, k)| format!("{a}.{b}.{k}"));
                        self.relatar(&c::REMOVED_LINT, no, &[nome, desde.as_str()]);
                    }
                    _ => {}
                }
            }
        }
    }

    /// `OptionsFileValidator.validate`, na ordem dos validadores.
    fn arquivo(&mut self, opcoes: &No, fonte: &str) {
        self.chaves_de_topo(opcoes, "analyzer", &TOPO_DO_ANALYZER);
        if let Some(analyzer) = opcoes.campo("analyzer").filter(|a| a.mapa().is_some()) {
            self.strong_mode(analyzer);
            self.filtros_de_erro(analyzer);
            self.experimentos(analyzer);
            self.linguagem(analyzer);
            self.checagens_opcionais(analyzer);
            self.nao_ignoraveis(analyzer);
        }
        self.estilo_de_codigo(opcoes, fonte);
        self.formatador(opcoes, fonte);
        self.chaves_de_topo(opcoes, "linter", &["rules"]);
        self.regras(opcoes);
    }

    /// `PluginsOptionValidator`: mais de um plugin legado.
    fn plugins(&mut self, opcoes: &No, primeiro_incluido: Option<&str>) {
        let Some(plugins) = opcoes.campo("analyzer").filter(|a| a.mapa().is_some()).and_then(|a| a.campo("plugins")) else { return };
        let nos: Vec<&No> = match &plugins.valor {
            Valor::Lista(itens) => itens.iter().collect(),
            Valor::Mapa(pares) => pares.iter().map(|(k, _)| k).collect(),
            Valor::Nulo => return,
            // Um escalar: só conflita com o plugin de um arquivo incluído.
            _ => {
                if let Some(p) = primeiro_incluido
                    && texto_de(plugins) != Some(p)
                {
                    self.relatar(&c::MULTIPLE_PLUGINS, plugins, &[p]);
                }
                return;
            }
        };
        match primeiro_incluido {
            Some(p) => {
                for no in nos {
                    if texto_de(no) != Some(p) {
                        self.relatar(&c::MULTIPLE_PLUGINS, no, &[p]);
                    }
                }
            }
            None if nos.len() > 1 => {
                let mut primeiro: Option<&str> = None;
                for no in nos {
                    match primeiro {
                        None => primeiro = no.texto(),
                        Some(p) if texto_de(no) != Some(p) => self.relatar(&c::MULTIPLE_PLUGINS, no, &[p]),
                        Some(_) => {}
                    }
                }
            }
            None => {}
        }
    }
}

/// `_firstPluginName`.
fn primeiro_plugin(opcoes: &No) -> Option<String> {
    let plugins = opcoes.campo("analyzer").filter(|a| a.mapa().is_some())?.campo("plugins")?;
    match &plugins.valor {
        Valor::Texto(t) => Some(t.clone()),
        Valor::Lista(itens) => itens.first().and_then(No::texto).map(str::to_string),
        Valor::Mapa(pares) => pares.first().and_then(|(k, _)| k.texto()).map(str::to_string),
        _ => None,
    }
}

/// `getOptionsFromString`: o mapa do texto; um documento que não é mapa
/// (ou vazio) é um mapa vazio.
fn mapa_de(texto: &str) -> Result<No, yaml::ErroDeYaml> {
    let vazio = No { span: Span { start: 0, end: 0 }, valor: Valor::Mapa(Vec::new()) };
    Ok(match yaml::ler(texto)? {
        Some(no) if no.mapa().is_some() => no,
        _ => vazio,
    })
}

struct Analise<'a> {
    ctx: &'a Contexto<'a>,
    erros: Vec<Relato>,
    /// O intervalo do `include:` do arquivo inicial.
    include_inicial: Option<Span>,
    primeiro_plugin: Option<String>,
    /// Os arquivos já incluídos, com o intervalo do `include:` que os trouxe.
    cadeia: Vec<(PathBuf, Span)>,
}

impl Analise<'_> {
    /// `addDirectErrorOrIncludedError`.
    fn acrescentar(&mut self, relatos: Vec<Relato>, arquivo: &Path, e_o_da_raiz: bool) {
        match (e_o_da_raiz, self.include_inicial) {
            (false, Some(onde)) => {
                for r in relatos {
                    let nome = arquivo.to_string_lossy();
                    let (de, ate) = (r.span.start.to_string(), (r.span.end.saturating_sub(1)).to_string());
                    let mensagem = r.mensagem();
                    self.erros.push(Relato::novo(&c::INCLUDED_FILE_WARNING, onde, &[nome.as_ref(), de.as_str(), ate.as_str(), mensagem.as_str()]));
                }
            }
            _ => self.erros.extend(relatos),
        }
    }

    /// `validate`: as opções de `arquivo` e, pelo `include:`, as incluídas.
    fn validar(&mut self, arquivo: &Path, opcoes: &No, fonte: &str) {
        let e_o_da_raiz = self.include_inicial.is_none();
        let mut v = Validador { out: Vec::new(), sdk_permite: self.ctx.sdk_permite, e_o_da_raiz };
        v.arquivo(opcoes, fonte);
        self.acrescentar(v.out, arquivo, e_o_da_raiz);
        let Some(include) = opcoes.campo("include") else {
            let mut v = Validador { out: Vec::new(), sdk_permite: None, e_o_da_raiz };
            v.plugins(opcoes, None);
            self.acrescentar(v.out, arquivo, e_o_da_raiz);
            return;
        };
        let onde = *self.include_inicial.get_or_insert(include.span);
        // `includeSpan.text`: o texto do nó como está escrito (com aspas,
        // se houver).
        let uri = fonte.get(include.span.start..include.span.end).unwrap_or("");
        let nome = arquivo.to_string_lossy();
        let incluido = (self.ctx.resolver)(arquivo, uri);
        if incluido.as_deref() == Some(self.ctx.arquivo) {
            self.erros.push(Relato::novo(&c::RECURSIVE_INCLUDE_FILE, onde, &[uri, nome.as_ref()]));
            return;
        }
        let Some(incluido) = incluido.filter(|p| p.is_file()) else {
            self.erros.push(Relato::novo(&c::INCLUDE_FILE_NOT_FOUND, onde, &[uri, nome.as_ref(), self.ctx.raiz_do_contexto]));
            return;
        };
        if let Some((_, anterior)) = self.cadeia.iter().find(|(p, _)| *p == incluido) {
            let (caminho, de, comprimento) =
                (incluido.to_string_lossy().into_owned(), anterior.start.to_string(), (anterior.end - anterior.start).to_string());
            self.erros.push(Relato::novo(
                &c::INCLUDED_FILE_WARNING,
                onde,
                &[caminho.as_str(), de.as_str(), comprimento.as_str(), "The file includes itself recursively."],
            ));
            return;
        }
        self.cadeia.push((incluido.clone(), include.span));
        let texto = std::fs::read_to_string(&incluido).unwrap_or_default();
        match mapa_de(&texto) {
            Ok(incluidas) => {
                self.validar(&incluido, &incluidas, &texto);
                if self.primeiro_plugin.is_none() {
                    self.primeiro_plugin = primeiro_plugin(&incluidas);
                }
                let mut v = Validador { out: Vec::new(), sdk_permite: None, e_o_da_raiz };
                v.plugins(opcoes, self.primeiro_plugin.as_deref());
                self.acrescentar(v.out, arquivo, e_o_da_raiz);
            }
            Err(e) => {
                let (caminho, posicao) = (incluido.to_string_lossy().into_owned(), e.offset.to_string());
                self.erros.push(Relato::novo(
                    &c::INCLUDED_FILE_PARSE_ERROR,
                    onde,
                    &[caminho.as_str(), posicao.as_str(), posicao.as_str(), e.mensagem.as_str()],
                ));
            }
        }
    }
}

/// `analyzeAnalysisOptions`: os relatos do arquivo de opções de texto
/// `conteudo`, todos com intervalo no arquivo inicial.
pub fn analisar(conteudo: &str, ctx: &Contexto<'_>) -> Vec<Relato> {
    let mut analise = Analise { ctx, erros: Vec::new(), include_inicial: None, primeiro_plugin: None, cadeia: Vec::new() };
    match mapa_de(conteudo) {
        Ok(opcoes) => analise.validar(ctx.arquivo, &opcoes, conteudo),
        Err(e) => analise.erros.push(Relato::novo(&c::PARSE_ERROR, Span { start: e.offset, end: e.offset }, &[e.mensagem.as_str()])),
    }
    analise.erros
}

#[cfg(test)]
mod testes {
    use super::*;

    /// `(nome do código, texto do intervalo)` de cada relato, sem includes.
    fn achados(fonte: &str) -> Vec<(&'static str, String)> {
        let sem_includes = |_: &Path, _: &str| None;
        let ctx = Contexto { arquivo: Path::new("analysis_options.yaml"), raiz_do_contexto: ".", resolver: &sem_includes, sdk_permite: None };
        analisar(fonte, &ctx).into_iter().map(|r| (r.codigo.nome, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn secao_do_analyzer() {
        assert_eq!(achados("analyzer:\n  exclude: [a]\n  erros: {}\n"), vec![("unsupported_option_with_legal_values", "erros".to_string())]);
        assert_eq!(
            achados("analyzer:\n  errors:\n    nao_existe: ignore\n    dead_code: fatal\n"),
            vec![("unrecognized_error_code", "nao_existe".to_string()), ("unsupported_option_with_legal_values", "fatal".to_string())]
        );
        assert_eq!(achados("analyzer:\n  language:\n    strict-casts: sim\n"), vec![("unsupported_value", "sim".to_string())]);
        assert!(achados("analyzer:\n  language:\n    strict-casts: true\n  errors:\n    dead_code: ignore\n    todo: warning\n").is_empty());
    }

    #[test]
    fn regras_de_lint() {
        assert_eq!(achados("linter:\n  rules:\n    - regra_que_nao_existe\n"), vec![("undefined_lint", "regra_que_nao_existe".to_string())]);
        assert_eq!(
            achados("linter:\n  rules:\n    - annotate_overrides\n    - annotate_overrides\n"),
            vec![("duplicate_rule", "annotate_overrides".to_string())]
        );
        assert_eq!(
            achados("linter:\n  rules:\n    - always_use_package_imports\n    - prefer_relative_imports\n"),
            vec![("incompatible_lint", "prefer_relative_imports".to_string())]
        );
        assert_eq!(achados("linter:\n  regras: []\n"), vec![("unsupported_option_with_legal_value", "regras".to_string())]);
    }

    #[test]
    fn include_e_sintaxe() {
        assert_eq!(achados("include: nao_existe.yaml\n"), vec![("include_file_not_found", "nao_existe.yaml".to_string())]);
        assert_eq!(achados("analyzer: [\n").first().map(|r| r.0), Some("parse_error"));
    }

    #[test]
    fn lista_entre_aspas() {
        assert_eq!(entre_aspas(&["a"]), "'a'");
        assert_eq!(entre_aspas(&["a", "b"]), "'a' and 'b'");
        assert_eq!(entre_aspas(&["a", "b", "c"]), "'a', 'b', and 'c'");
    }
}
