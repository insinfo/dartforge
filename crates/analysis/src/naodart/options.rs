//! The validation of `analysis_options.yaml`: `analyzeAnalysisOptions` and the
//! validators of `analyzer/lib/src/task/options.dart` and
//! `analyzer/lib/src/lint/options_rule_validator.dart`, read in 6.11.0
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, batch II.10).
//!
//! Covers: the YAML syntax error, the `include:` chain (not found, recursive,
//! warnings and syntax error of the included file, reported on the initial
//! file's `include:`), the `analyzer` sections (keys, `strong-mode`, `errors`,
//! `language`, `optional-checks`, `cannot-ignore`, `plugins`), `code-style`,
//! `formatter`, `linter` and the rules of `linter: rules:`.
//!
//! Differences: the syntax error text is `yaml-rust2`'s, not `package:yaml`'s,
//! and its range has length zero; the `enable-experiment` names are not
//! checked (only the section's format); a removed rule always comes out as
//! `removed_lint`, because the rule table does not keep the replacement
//! (`replaced_lint`).
//! Written without compiling or running (2026-10-05).

use super::codigos_g::opcoes as c;
use super::nomes_g::NOMES_DE_CODIGO;
use super::yaml::{self, No, Valor};
use super::{CodigoNaoDart, Report};
use crate::lints::{self, EstadoDaRegra};
use dartforge_diagnostics::Span;
use std::path::{Path, PathBuf};

/// What the validation needs to know from outside the file.
pub struct OptionsContext<'a> {
    /// The initial options file.
    pub file: &'a Path,
    /// The root of the analysis context, for the include-not-found message.
    pub context_root: &'a str,
    /// `sourceFactory.resolveUri(from, uri)`: the file of an `include:`.
    pub resolver: &'a dyn Fn(&Path, &str) -> Option<PathBuf>,
    /// `sdkVersionConstraint.allows(version)`; `None` without an SDK constraint.
    pub sdk_allows: Option<&'a dyn Fn((u32, u32, u32)) -> bool>,
}

/// `quotedAndCommaSeparatedWithAnd`.
fn quoted_list(items: &[&str]) -> String {
    let citados: Vec<String> = items.iter().map(|i| format!("'{i}'")).collect();
    match citados.len() {
        0 => String::new(),
        1 => citados[0].clone(),
        2 => format!("{} and {}", citados[0], citados[1]),
        n => format!("{}, and {}", citados[..n - 1].join(", "), citados[n - 1]),
    }
}

/// The value of a scalar as text (`value.toString()`); `None` for null, list
/// and map.
fn text_of(node: &No) -> Option<&str> {
    match &node.valor {
        Valor::Texto(t) | Valor::Outro(t) => Some(t),
        _ => None,
    }
}

const ANALYZER_TOP_LEVEL: [&str; 8] =
    ["cannot-ignore", "enable-experiment", "errors", "exclude", "language", "optional-checks", "plugins", "strong-mode"];
const STRONG_MODE_KEYS: [&str; 3] = ["declaration-casts", "implicit-casts", "implicit-dynamic"];
const LANGUAGE_KEYS: [&str; 3] = ["strict-casts", "strict-inference", "strict-raw-types"];
const OPTIONAL_CHECKS: [&str; 2] = ["chrome-os-manifest-checks", "propagate-linter-exceptions"];
const TRUE_OR_FALSE: [&str; 2] = ["true", "false"];
const SEVERIDADES: [&str; 3] = ["error", "info", "warning"];
const ERROR_VALUES: [&str; 7] = ["ignore", "false", "include", "true", "error", "info", "warning"];

/// The name (in upper case) is that of a diagnostic code, of a lint rule, or of
/// the removed code that is still accepted.
fn is_known_code(upper: &str) -> bool {
    NOMES_DE_CODIGO.binary_search(&upper).is_ok()
        || lints::tabela_g::REGRAS.iter().any(|r| r.nome.eq_ignore_ascii_case(upper))
        || upper == "MISSING_RETURN"
}

struct Validator<'a> {
    out: Vec<Report>,
    sdk_allows: Option<&'a dyn Fn((u32, u32, u32)) -> bool>,
    is_root_file: bool,
}

impl Validator<'_> {
    fn report(&mut self, code: &'static CodigoNaoDart, node: &No, args: &[&str]) {
        self.out.push(Report::new(code, node.span, args));
    }

    /// `ErrorBuilder.reportError`: the unsupported option, with the proposal.
    fn unsupported(&mut self, scope: &str, node: &No, supported: &[&str]) {
        let Some(value) = text_of(node) else { return };
        let proposta = quoted_list(supported);
        match supported.len() {
            0 => self.report(&c::UNSUPPORTED_OPTION_WITHOUT_VALUES, node, &[scope, value]),
            1 => self.report(&c::UNSUPPORTED_OPTION_WITH_LEGAL_VALUE, node, &[scope, value, proposta.as_str()]),
            _ => self.report(&c::UNSUPPORTED_OPTION_WITH_LEGAL_VALUES, node, &[scope, value, proposta.as_str()]),
        }
    }

    /// `TopLevelOptionValidator`: the keys of a top-level section.
    fn top_level_keys(&mut self, options: &No, section: &str, supported: &[&str]) {
        let Some(pairs) = options.campo(section).and_then(No::mapa) else { return };
        for (k, _) in pairs {
            if k.escalar() && !text_of(k).is_some_and(|t| supported.contains(&t)) {
                self.unsupported(section, k, supported);
            }
        }
    }

    /// A value that must be `true` or `false`.
    fn boolean(&mut self, key: &str, v: &No) {
        if !v.escalar() {
            return;
        }
        let ok = text_of(v).is_some_and(|t| TRUE_OR_FALSE.contains(&t.to_lowercase().as_str()));
        if !ok && let Some(value) = text_of(v) {
            let proposta = quoted_list(&TRUE_OR_FALSE);
            self.report(&c::UNSUPPORTED_VALUE, v, &[key, value, proposta.as_str()]);
        }
    }

    /// `StrongModeOptionValueValidator`.
    fn strong_mode(&mut self, analyzer: &No) {
        let Some(node) = analyzer.campo("strong-mode") else { return };
        let Some(pairs) = node.mapa() else {
            self.report(&c::INVALID_SECTION_FORMAT, node, &["strong-mode"]);
            return;
        };
        for (k, v) in pairs {
            if !k.escalar() {
                continue;
            }
            match text_of(k) {
                Some(key) if STRONG_MODE_KEYS.contains(&key) => {
                    if key == "declaration-casts" {
                        if let Some(value) = text_of(v) {
                            let proposta = quoted_list(&TRUE_OR_FALSE);
                            self.report(&c::UNSUPPORTED_VALUE, v, &["strong-mode", value, proposta.as_str()]);
                        }
                    } else {
                        self.boolean(key, v);
                    }
                }
                _ => self.unsupported("strong-mode", k, &STRONG_MODE_KEYS),
            }
        }
    }

    /// `ErrorFilterOptionValidator`. The argument of the invalid format is
    /// `enable-experiment`, as in the original.
    fn error_filters(&mut self, analyzer: &No) {
        let Some(node) = analyzer.campo("errors") else { return };
        let Some(pairs) = node.mapa() else {
            self.report(&c::INVALID_SECTION_FORMAT, node, &["enable-experiment"]);
            return;
        };
        for (k, v) in pairs {
            if k.escalar()
                && let Some(name) = text_of(k)
                && !is_known_code(&name.to_uppercase())
            {
                self.report(&c::UNRECOGNIZED_ERROR_CODE, k, &[name]);
            }
            if v.escalar() {
                let value = text_of(v).unwrap_or("null");
                if !ERROR_VALUES.contains(&value.to_lowercase().as_str()) {
                    let proposta = quoted_list(&ERROR_VALUES);
                    self.report(&c::UNSUPPORTED_OPTION_WITH_LEGAL_VALUES, v, &["errors", value, proposta.as_str()]);
                }
            } else {
                self.report(&c::INVALID_SECTION_FORMAT, v, &["enable-experiment"]);
            }
        }
    }

    /// `EnabledExperimentsValidator`: only the section's format.
    fn experiments(&mut self, analyzer: &No) {
        if let Some(node) = analyzer.campo("enable-experiment")
            && node.lista().is_none()
        {
            self.report(&c::INVALID_SECTION_FORMAT, node, &["enable-experiment"]);
        }
    }

    /// `LanguageOptionValidator`.
    fn language(&mut self, analyzer: &No) {
        let Some(node) = analyzer.campo("language") else { return };
        let Some(pairs) = node.mapa() else {
            // A non-null scalar or a list.
            if !node.nulo() {
                self.report(&c::INVALID_SECTION_FORMAT, node, &["language"]);
            }
            return;
        };
        for (k, v) in pairs {
            if !k.escalar() {
                continue;
            }
            match text_of(k) {
                Some(key) if LANGUAGE_KEYS.contains(&key) => self.boolean(key, v),
                _ => self.unsupported("language", k, &LANGUAGE_KEYS),
            }
        }
    }

    /// `OptionalChecksValueValidator`.
    fn optional_checks(&mut self, analyzer: &No) {
        let Some(node) = analyzer.campo("optional-checks") else { return };
        let scope = "chrome-os-manifest-checks";
        if node.escalar() {
            if text_of(node).map(str::to_lowercase).as_deref() != Some(scope) {
                self.unsupported(scope, node, &OPTIONAL_CHECKS);
            }
        } else if let Some(pairs) = node.mapa() {
            for (k, v) in pairs {
                if !k.escalar() {
                    continue;
                }
                if text_of(k) == Some(scope) {
                    self.boolean(scope, v);
                } else {
                    self.unsupported(scope, k, &OPTIONAL_CHECKS);
                }
            }
        } else {
            self.report(&c::INVALID_SECTION_FORMAT, node, &["enable-experiment"]);
        }
    }

    /// `CannotIgnoreOptionValidator`.
    fn cannot_ignore(&mut self, analyzer: &No) {
        let Some(node) = analyzer.campo("cannot-ignore") else { return };
        let Some(items) = node.lista() else {
            self.report(&c::INVALID_SECTION_FORMAT, node, &["cannot-ignore"]);
            return;
        };
        for item in items {
            match item.texto() {
                Some(name) => {
                    if !SEVERIDADES.contains(&name) && !is_known_code(&name.to_uppercase()) {
                        self.report(&c::UNRECOGNIZED_ERROR_CODE, item, &[name]);
                    }
                }
                None => self.report(&c::INVALID_SECTION_FORMAT, item, &["cannot-ignore"]),
            }
        }
    }

    /// `CodeStyleOptionsValidator`.
    fn code_style(&mut self, options: &No, source: &str) {
        let Some(node) = options.campo("code-style") else { return };
        let Some(pairs) = node.mapa() else {
            if !node.nulo() {
                self.report(&c::INVALID_SECTION_FORMAT, node, &["code-style"]);
            }
            return;
        };
        for (k, v) in pairs {
            if text_of(k) == Some("format") {
                if !v.escalar() {
                    self.report(&c::INVALID_SECTION_FORMAT, v, &["format"]);
                } else {
                    self.boolean("format", v);
                }
            } else {
                let key = source.get(k.span.start..k.span.end).unwrap_or("");
                self.report(&c::UNSUPPORTED_OPTION_WITHOUT_VALUES, k, &["code-style", key]);
            }
        }
    }

    /// `FormatterOptionsValidator`.
    fn formatter(&mut self, options: &No, source: &str) {
        let Some(node) = options.campo("formatter") else { return };
        let Some(pairs) = node.mapa() else {
            if !node.nulo() {
                self.report(&c::INVALID_SECTION_FORMAT, node, &["formatter"]);
            }
            return;
        };
        for (k, v) in pairs {
            let key = source.get(k.span.start..k.span.end).unwrap_or("");
            if text_of(k) == Some("page_width") {
                let positive = matches!(&v.valor, Valor::Outro(t) if t.parse::<i64>().is_ok_and(|n| n > 0));
                if !positive {
                    self.report(&c::INVALID_OPTION, v, &[key, "\"page_width\" must be a positive integer."]);
                }
            } else {
                self.report(&c::UNSUPPORTED_OPTION_WITHOUT_VALUES, k, &["formatter", key]);
            }
        }
    }

    /// `LinterRuleOptionsValidator._validateRules`.
    fn lint_rules(&mut self, options: &No) {
        let Some(regras) = options.campo("linter").filter(|l| l.mapa().is_some()).and_then(|l| l.campo("rules")) else { return };
        let mut seen: Vec<&str> = Vec::new();
        let entries: Vec<(&No, bool)> = match &regras.valor {
            Valor::Lista(items) => items.iter().map(|n| (n, true)).collect(),
            Valor::Mapa(pairs) => pairs.iter().map(|(k, v)| (k, matches!(&v.valor, Valor::Outro(t) if t.eq_ignore_ascii_case("true")))).collect(),
            _ => Vec::new(),
        };
        for (node, enabled) in entries {
            let Some(name) = text_of(node) else { continue };
            let Some(rule) = lints::rule(name) else {
                self.report(&c::UNDEFINED_LINT, node, &[name]);
                continue;
            };
            if enabled {
                if let Some(other) = rule.incompativeis.iter().copied().find(|i| seen.contains(i)) {
                    self.report(&c::INCOMPATIBLE_LINT, node, &[name, other]);
                } else if seen.contains(&rule.nome) {
                    self.report(&c::DUPLICATE_RULE, node, &[name]);
                } else {
                    seen.push(rule.nome);
                }
            }
            if self.is_root_file {
                // `currentSdkAllows`: without `since`, yes; without a constraint, no.
                let in_sdk = match rule.desde {
                    None => true,
                    Some(v) => self.sdk_allows.is_some_and(|f| f(v)),
                };
                match rule.estado {
                    EstadoDaRegra::Depreciada if in_sdk => self.report(&c::DEPRECATED_LINT, node, &[name]),
                    EstadoDaRegra::Removida if in_sdk => {
                        let desde = rule.desde.map_or_else(|| "null".to_string(), |(a, b, k)| format!("{a}.{b}.{k}"));
                        self.report(&c::REMOVED_LINT, node, &[name, desde.as_str()]);
                    }
                    _ => {}
                }
            }
        }
    }

    /// `OptionsFileValidator.validate`, in validator order.
    fn file_options(&mut self, options: &No, source: &str) {
        self.top_level_keys(options, "analyzer", &ANALYZER_TOP_LEVEL);
        if let Some(analyzer) = options.campo("analyzer").filter(|a| a.mapa().is_some()) {
            self.strong_mode(analyzer);
            self.error_filters(analyzer);
            self.experiments(analyzer);
            self.language(analyzer);
            self.optional_checks(analyzer);
            self.cannot_ignore(analyzer);
        }
        self.code_style(options, source);
        self.formatter(options, source);
        self.top_level_keys(options, "linter", &["rules"]);
        self.lint_rules(options);
    }

    /// `PluginsOptionValidator`: more than one legacy plugin.
    fn plugins(&mut self, options: &No, first_included: Option<&str>) {
        let Some(plugins) = options.campo("analyzer").filter(|a| a.mapa().is_some()).and_then(|a| a.campo("plugins")) else { return };
        let nodes: Vec<&No> = match &plugins.valor {
            Valor::Lista(items) => items.iter().collect(),
            Valor::Mapa(pairs) => pairs.iter().map(|(k, _)| k).collect(),
            Valor::Nulo => return,
            // A scalar: it only conflicts with the plugin of an included file.
            _ => {
                if let Some(p) = first_included
                    && text_of(plugins) != Some(p)
                {
                    self.report(&c::MULTIPLE_PLUGINS, plugins, &[p]);
                }
                return;
            }
        };
        match first_included {
            Some(p) => {
                for node in nodes {
                    if text_of(node) != Some(p) {
                        self.report(&c::MULTIPLE_PLUGINS, node, &[p]);
                    }
                }
            }
            None if nodes.len() > 1 => {
                let mut first: Option<&str> = None;
                for node in nodes {
                    match first {
                        None => first = node.texto(),
                        Some(p) if text_of(node) != Some(p) => self.report(&c::MULTIPLE_PLUGINS, node, &[p]),
                        Some(_) => {}
                    }
                }
            }
            None => {}
        }
    }
}

/// `_firstPluginName`.
fn first_plugin(options: &No) -> Option<String> {
    let plugins = options.campo("analyzer").filter(|a| a.mapa().is_some())?.campo("plugins")?;
    match &plugins.valor {
        Valor::Texto(t) => Some(t.clone()),
        Valor::Lista(items) => items.first().and_then(No::texto).map(str::to_string),
        Valor::Mapa(pairs) => pairs.first().and_then(|(k, _)| k.texto()).map(str::to_string),
        _ => None,
    }
}

/// `getOptionsFromString`: the text's map; a document that is not a map (or
/// is empty) is an empty map.
fn map_of(text: &str) -> Result<No, yaml::YamlError> {
    let empty = No { span: Span { start: 0, end: 0 }, valor: Valor::Mapa(Vec::new()) };
    Ok(match yaml::ler(text)? {
        Some(node) if node.mapa().is_some() => node,
        _ => empty,
    })
}

struct OptionsAnalysis<'a> {
    ctx: &'a OptionsContext<'a>,
    errors: Vec<Report>,
    /// The range of the initial file's `include:`.
    initial_include: Option<Span>,
    first_plugin: Option<String>,
    /// The files already included, with the range of the `include:` that brought them.
    chain: Vec<(PathBuf, Span)>,
}

impl OptionsAnalysis<'_> {
    /// `addDirectErrorOrIncludedError`.
    fn append(&mut self, reports: Vec<Report>, file: &Path, is_root_file: bool) {
        match (is_root_file, self.initial_include) {
            (false, Some(where_)) => {
                for r in reports {
                    let name = file.to_string_lossy();
                    let (de, until) = (r.span.start.to_string(), (r.span.end.saturating_sub(1)).to_string());
                    let message = r.message();
                    self.errors.push(Report::new(&c::INCLUDED_FILE_WARNING, where_, &[name.as_ref(), de.as_str(), until.as_str(), message.as_str()]));
                }
            }
            _ => self.errors.extend(reports),
        }
    }

    /// `validate`: the options of `file` and, through `include:`, the included ones.
    fn validate(&mut self, file: &Path, options: &No, source: &str) {
        let is_root_file = self.initial_include.is_none();
        let mut v = Validator { out: Vec::new(), sdk_allows: self.ctx.sdk_allows, is_root_file };
        v.file_options(options, source);
        self.append(v.out, file, is_root_file);
        let Some(include) = options.campo("include") else {
            let mut v = Validator { out: Vec::new(), sdk_allows: None, is_root_file };
            v.plugins(options, None);
            self.append(v.out, file, is_root_file);
            return;
        };
        let where_ = *self.initial_include.get_or_insert(include.span);
        // `includeSpan.text`: the node's text as written (with quotes, if
        // any).
        let uri = source.get(include.span.start..include.span.end).unwrap_or("");
        let name = file.to_string_lossy();
        let included = (self.ctx.resolver)(file, uri);
        if included.as_deref() == Some(self.ctx.file) {
            self.errors.push(Report::new(&c::RECURSIVE_INCLUDE_FILE, where_, &[uri, name.as_ref()]));
            return;
        }
        let Some(included) = included.filter(|p| p.is_file()) else {
            self.errors.push(Report::new(&c::INCLUDE_FILE_NOT_FOUND, where_, &[uri, name.as_ref(), self.ctx.context_root]));
            return;
        };
        if let Some((_, previous)) = self.chain.iter().find(|(p, _)| *p == included) {
            let (path, de, length) =
                (included.to_string_lossy().into_owned(), previous.start.to_string(), (previous.end - previous.start).to_string());
            self.errors.push(Report::new(
                &c::INCLUDED_FILE_WARNING,
                where_,
                &[path.as_str(), de.as_str(), length.as_str(), "The file includes itself recursively."],
            ));
            return;
        }
        self.chain.push((included.clone(), include.span));
        let text = std::fs::read_to_string(&included).unwrap_or_default();
        match map_of(&text) {
            Ok(incluidas) => {
                self.validate(&included, &incluidas, &text);
                if self.first_plugin.is_none() {
                    self.first_plugin = first_plugin(&incluidas);
                }
                let mut v = Validator { out: Vec::new(), sdk_allows: None, is_root_file };
                v.plugins(options, self.first_plugin.as_deref());
                self.append(v.out, file, is_root_file);
            }
            Err(e) => {
                let (path, pos) = (included.to_string_lossy().into_owned(), e.offset.to_string());
                self.errors.push(Report::new(
                    &c::INCLUDED_FILE_PARSE_ERROR,
                    where_,
                    &[path.as_str(), pos.as_str(), pos.as_str(), e.message.as_str()],
                ));
            }
        }
    }
}

/// `analyzeAnalysisOptions`: the reports of the options file with text
/// `contents`, all with a range in the initial file.
pub fn analyze(contents: &str, ctx: &OptionsContext<'_>) -> Vec<Report> {
    let mut analysis = OptionsAnalysis { ctx, errors: Vec::new(), initial_include: None, first_plugin: None, chain: Vec::new() };
    match map_of(contents) {
        Ok(options) => analysis.validate(ctx.file, &options, contents),
        Err(e) => analysis.errors.push(Report::new(&c::PARSE_ERROR, Span { start: e.offset, end: e.offset }, &[e.message.as_str()])),
    }
    analysis.errors
}

#[cfg(test)]
mod testes {
    use super::*;

    /// `(code name, range text)` of each report, without includes.
    fn findings(source: &str) -> Vec<(&'static str, String)> {
        let without_includes = |_: &Path, _: &str| None;
        let ctx = OptionsContext { file: Path::new("analysis_options.yaml"), context_root: ".", resolver: &without_includes, sdk_allows: None };
        analyze(source, &ctx).into_iter().map(|r| (r.code.nome, source[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn secao_do_analyzer() {
        assert_eq!(findings("analyzer:\n  exclude: [a]\n  erros: {}\n"), vec![("unsupported_option_with_legal_values", "erros".to_string())]);
        assert_eq!(
            findings("analyzer:\n  errors:\n    nao_existe: ignore\n    dead_code: fatal\n"),
            vec![("unrecognized_error_code", "nao_existe".to_string()), ("unsupported_option_with_legal_values", "fatal".to_string())]
        );
        assert_eq!(findings("analyzer:\n  language:\n    strict-casts: sim\n"), vec![("unsupported_value", "sim".to_string())]);
        assert!(findings("analyzer:\n  language:\n    strict-casts: true\n  errors:\n    dead_code: ignore\n    todo: warning\n").is_empty());
    }

    #[test]
    fn regras_de_lint() {
        assert_eq!(findings("linter:\n  rules:\n    - regra_que_nao_existe\n"), vec![("undefined_lint", "regra_que_nao_existe".to_string())]);
        assert_eq!(
            findings("linter:\n  rules:\n    - annotate_overrides\n    - annotate_overrides\n"),
            vec![("duplicate_rule", "annotate_overrides".to_string())]
        );
        assert_eq!(
            findings("linter:\n  rules:\n    - always_use_package_imports\n    - prefer_relative_imports\n"),
            vec![("incompatible_lint", "prefer_relative_imports".to_string())]
        );
        assert_eq!(findings("linter:\n  regras: []\n"), vec![("unsupported_option_with_legal_value", "regras".to_string())]);
    }

    #[test]
    fn include_e_sintaxe() {
        assert_eq!(findings("include: nao_existe.yaml\n"), vec![("include_file_not_found", "nao_existe.yaml".to_string())]);
        assert_eq!(findings("analyzer: [\n").first().map(|r| r.0), Some("parse_error"));
    }

    #[test]
    fn lista_entre_aspas() {
        assert_eq!(quoted_list(&["a"]), "'a'");
        assert_eq!(quoted_list(&["a", "b"]), "'a' and 'b'");
        assert_eq!(quoted_list(&["a", "b", "c"]), "'a', 'b', and 'c'");
    }
}
