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

use super::codes_g::options as c;
use super::names_g::CODE_NAMES;
use super::yaml::{self, Node, Value};
use super::{NonDartCode, Report};
use crate::lints::{self, RuleState};
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
    let quoted: Vec<String> = items.iter().map(|i| format!("'{i}'")).collect();
    match quoted.len() {
        0 => String::new(),
        1 => quoted[0].clone(),
        2 => format!("{} and {}", quoted[0], quoted[1]),
        n => format!("{}, and {}", quoted[..n - 1].join(", "), quoted[n - 1]),
    }
}

/// The value of a scalar as text (`value.toString()`); `None` for null, list
/// and map.
fn text_of(node: &Node) -> Option<&str> {
    match &node.value {
        Value::Text(t) | Value::Other(t) => Some(t),
        _ => None,
    }
}

const ANALYZER_TOP_LEVEL: [&str; 8] =
    ["cannot-ignore", "enable-experiment", "errors", "exclude", "language", "optional-checks", "plugins", "strong-mode"];
const STRONG_MODE_KEYS: [&str; 3] = ["declaration-casts", "implicit-casts", "implicit-dynamic"];
const LANGUAGE_KEYS: [&str; 3] = ["strict-casts", "strict-inference", "strict-raw-types"];
const OPTIONAL_CHECKS: [&str; 2] = ["chrome-os-manifest-checks", "propagate-linter-exceptions"];
const TRUE_OR_FALSE: [&str; 2] = ["true", "false"];
const SEVERITIES: [&str; 3] = ["error", "info", "warning"];
const ERROR_VALUES: [&str; 7] = ["ignore", "false", "include", "true", "error", "info", "warning"];

/// The name (in upper case) is that of a diagnostic code, of a lint rule, or of
/// the removed code that is still accepted.
fn is_known_code(upper: &str) -> bool {
    CODE_NAMES.binary_search(&upper).is_ok()
        || lints::table_g::RULES.iter().any(|r| r.name.eq_ignore_ascii_case(upper))
        || upper == "MISSING_RETURN"
}

struct Validator<'a> {
    out: Vec<Report>,
    sdk_allows: Option<&'a dyn Fn((u32, u32, u32)) -> bool>,
    is_root_file: bool,
}

impl Validator<'_> {
    fn report(&mut self, code: &'static NonDartCode, node: &Node, args: &[&str]) {
        self.out.push(Report::new(code, node.span, args));
    }

    /// `ErrorBuilder.reportError`: the unsupported option, with the proposal.
    fn unsupported(&mut self, scope: &str, node: &Node, supported: &[&str]) {
        let Some(value) = text_of(node) else { return };
        let proposed = quoted_list(supported);
        match supported.len() {
            0 => self.report(&c::UNSUPPORTED_OPTION_WITHOUT_VALUES, node, &[scope, value]),
            1 => self.report(&c::UNSUPPORTED_OPTION_WITH_LEGAL_VALUE, node, &[scope, value, proposed.as_str()]),
            _ => self.report(&c::UNSUPPORTED_OPTION_WITH_LEGAL_VALUES, node, &[scope, value, proposed.as_str()]),
        }
    }

    /// `TopLevelOptionValidator`: the keys of a top-level section.
    fn top_level_keys(&mut self, options: &Node, section: &str, supported: &[&str]) {
        let Some(pairs) = options.field(section).and_then(Node::map) else { return };
        for (k, _) in pairs {
            if k.is_scalar() && !text_of(k).is_some_and(|t| supported.contains(&t)) {
                self.unsupported(section, k, supported);
            }
        }
    }

    /// A value that must be `true` or `false`.
    fn boolean(&mut self, key: &str, v: &Node) {
        if !v.is_scalar() {
            return;
        }
        let ok = text_of(v).is_some_and(|t| TRUE_OR_FALSE.contains(&t.to_lowercase().as_str()));
        if !ok && let Some(value) = text_of(v) {
            let proposed = quoted_list(&TRUE_OR_FALSE);
            self.report(&c::UNSUPPORTED_VALUE, v, &[key, value, proposed.as_str()]);
        }
    }

    /// `StrongModeOptionValueValidator`.
    fn strong_mode(&mut self, analyzer: &Node) {
        let Some(node) = analyzer.field("strong-mode") else { return };
        let Some(pairs) = node.map() else {
            self.report(&c::INVALID_SECTION_FORMAT, node, &["strong-mode"]);
            return;
        };
        for (k, v) in pairs {
            if !k.is_scalar() {
                continue;
            }
            match text_of(k) {
                Some(key) if STRONG_MODE_KEYS.contains(&key) => {
                    if key == "declaration-casts" {
                        if let Some(value) = text_of(v) {
                            let proposed = quoted_list(&TRUE_OR_FALSE);
                            self.report(&c::UNSUPPORTED_VALUE, v, &["strong-mode", value, proposed.as_str()]);
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
    fn error_filters(&mut self, analyzer: &Node) {
        let Some(node) = analyzer.field("errors") else { return };
        let Some(pairs) = node.map() else {
            self.report(&c::INVALID_SECTION_FORMAT, node, &["enable-experiment"]);
            return;
        };
        for (k, v) in pairs {
            if k.is_scalar()
                && let Some(name) = text_of(k)
                && !is_known_code(&name.to_uppercase())
            {
                self.report(&c::UNRECOGNIZED_ERROR_CODE, k, &[name]);
            }
            if v.is_scalar() {
                let value = text_of(v).unwrap_or("null");
                if !ERROR_VALUES.contains(&value.to_lowercase().as_str()) {
                    let proposed = quoted_list(&ERROR_VALUES);
                    self.report(&c::UNSUPPORTED_OPTION_WITH_LEGAL_VALUES, v, &["errors", value, proposed.as_str()]);
                }
            } else {
                self.report(&c::INVALID_SECTION_FORMAT, v, &["enable-experiment"]);
            }
        }
    }

    /// `EnabledExperimentsValidator`: only the section's format.
    fn experiments(&mut self, analyzer: &Node) {
        if let Some(node) = analyzer.field("enable-experiment")
            && node.list().is_none()
        {
            self.report(&c::INVALID_SECTION_FORMAT, node, &["enable-experiment"]);
        }
    }

    /// `LanguageOptionValidator`.
    fn language(&mut self, analyzer: &Node) {
        let Some(node) = analyzer.field("language") else { return };
        let Some(pairs) = node.map() else {
            // A non-null scalar or a list.
            if !node.is_null() {
                self.report(&c::INVALID_SECTION_FORMAT, node, &["language"]);
            }
            return;
        };
        for (k, v) in pairs {
            if !k.is_scalar() {
                continue;
            }
            match text_of(k) {
                Some(key) if LANGUAGE_KEYS.contains(&key) => self.boolean(key, v),
                _ => self.unsupported("language", k, &LANGUAGE_KEYS),
            }
        }
    }

    /// `OptionalChecksValueValidator`.
    fn optional_checks(&mut self, analyzer: &Node) {
        let Some(node) = analyzer.field("optional-checks") else { return };
        let scope = "chrome-os-manifest-checks";
        if node.is_scalar() {
            if text_of(node).map(str::to_lowercase).as_deref() != Some(scope) {
                self.unsupported(scope, node, &OPTIONAL_CHECKS);
            }
        } else if let Some(pairs) = node.map() {
            for (k, v) in pairs {
                if !k.is_scalar() {
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
    fn cannot_ignore(&mut self, analyzer: &Node) {
        let Some(node) = analyzer.field("cannot-ignore") else { return };
        let Some(items) = node.list() else {
            self.report(&c::INVALID_SECTION_FORMAT, node, &["cannot-ignore"]);
            return;
        };
        for item in items {
            match item.text() {
                Some(name) => {
                    if !SEVERITIES.contains(&name) && !is_known_code(&name.to_uppercase()) {
                        self.report(&c::UNRECOGNIZED_ERROR_CODE, item, &[name]);
                    }
                }
                None => self.report(&c::INVALID_SECTION_FORMAT, item, &["cannot-ignore"]),
            }
        }
    }

    /// `CodeStyleOptionsValidator`.
    fn code_style(&mut self, options: &Node, source: &str) {
        let Some(node) = options.field("code-style") else { return };
        let Some(pairs) = node.map() else {
            if !node.is_null() {
                self.report(&c::INVALID_SECTION_FORMAT, node, &["code-style"]);
            }
            return;
        };
        for (k, v) in pairs {
            if text_of(k) == Some("format") {
                if !v.is_scalar() {
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
    fn formatter(&mut self, options: &Node, source: &str) {
        let Some(node) = options.field("formatter") else { return };
        let Some(pairs) = node.map() else {
            if !node.is_null() {
                self.report(&c::INVALID_SECTION_FORMAT, node, &["formatter"]);
            }
            return;
        };
        for (k, v) in pairs {
            let key = source.get(k.span.start..k.span.end).unwrap_or("");
            if text_of(k) == Some("page_width") {
                let positive = matches!(&v.value, Value::Other(t) if t.parse::<i64>().is_ok_and(|n| n > 0));
                if !positive {
                    self.report(&c::INVALID_OPTION, v, &[key, "\"page_width\" must be a positive integer."]);
                }
            } else {
                self.report(&c::UNSUPPORTED_OPTION_WITHOUT_VALUES, k, &["formatter", key]);
            }
        }
    }

    /// `LinterRuleOptionsValidator._validateRules`.
    fn lint_rules(&mut self, options: &Node) {
        let Some(rules) = options.field("linter").filter(|l| l.map().is_some()).and_then(|l| l.field("rules")) else { return };
        let mut seen: Vec<&str> = Vec::new();
        let entries: Vec<(&Node, bool)> = match &rules.value {
            Value::List(items) => items.iter().map(|n| (n, true)).collect(),
            Value::Map(pairs) => pairs.iter().map(|(k, v)| (k, matches!(&v.value, Value::Other(t) if t.eq_ignore_ascii_case("true")))).collect(),
            _ => Vec::new(),
        };
        for (node, enabled) in entries {
            let Some(name) = text_of(node) else { continue };
            let Some(rule) = lints::rule(name) else {
                self.report(&c::UNDEFINED_LINT, node, &[name]);
                continue;
            };
            if enabled {
                if let Some(other) = rule.incompatible.iter().copied().find(|i| seen.contains(i)) {
                    self.report(&c::INCOMPATIBLE_LINT, node, &[name, other]);
                } else if seen.contains(&rule.name) {
                    self.report(&c::DUPLICATE_RULE, node, &[name]);
                } else {
                    seen.push(rule.name);
                }
            }
            if self.is_root_file {
                // `currentSdkAllows`: without `since`, yes; without a constraint, no.
                let in_sdk = match rule.since {
                    None => true,
                    Some(v) => self.sdk_allows.is_some_and(|f| f(v)),
                };
                match rule.state {
                    RuleState::Deprecated if in_sdk => self.report(&c::DEPRECATED_LINT, node, &[name]),
                    RuleState::Removed if in_sdk => {
                        let since = rule.since.map_or_else(|| "null".to_string(), |(a, b, k)| format!("{a}.{b}.{k}"));
                        self.report(&c::REMOVED_LINT, node, &[name, since.as_str()]);
                    }
                    _ => {}
                }
            }
        }
    }

    /// `OptionsFileValidator.validate`, in validator order.
    fn file_options(&mut self, options: &Node, source: &str) {
        self.top_level_keys(options, "analyzer", &ANALYZER_TOP_LEVEL);
        if let Some(analyzer) = options.field("analyzer").filter(|a| a.map().is_some()) {
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
    fn plugins(&mut self, options: &Node, first_included: Option<&str>) {
        let Some(plugins) = options.field("analyzer").filter(|a| a.map().is_some()).and_then(|a| a.field("plugins")) else { return };
        let nodes: Vec<&Node> = match &plugins.value {
            Value::List(items) => items.iter().collect(),
            Value::Map(pairs) => pairs.iter().map(|(k, _)| k).collect(),
            Value::Null => return,
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
                        None => first = node.text(),
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
fn first_plugin(options: &Node) -> Option<String> {
    let plugins = options.field("analyzer").filter(|a| a.map().is_some())?.field("plugins")?;
    match &plugins.value {
        Value::Text(t) => Some(t.clone()),
        Value::List(items) => items.first().and_then(Node::text).map(str::to_string),
        Value::Map(pairs) => pairs.first().and_then(|(k, _)| k.text()).map(str::to_string),
        _ => None,
    }
}

/// `getOptionsFromString`: the text's map; a document that is not a map (or
/// is empty) is an empty map.
fn map_of(text: &str) -> Result<Node, yaml::YamlError> {
    let empty = Node { span: Span { start: 0, end: 0 }, value: Value::Map(Vec::new()) };
    Ok(match yaml::parse(text)? {
        Some(node) if node.map().is_some() => node,
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
                    let (from, until) = (r.span.start.to_string(), (r.span.end.saturating_sub(1)).to_string());
                    let message = r.message();
                    self.errors.push(Report::new(&c::INCLUDED_FILE_WARNING, where_, &[name.as_ref(), from.as_str(), until.as_str(), message.as_str()]));
                }
            }
            _ => self.errors.extend(reports),
        }
    }

    /// `validate`: the options of `file` and, through `include:`, the included ones.
    fn validate(&mut self, file: &Path, options: &Node, source: &str) {
        let is_root_file = self.initial_include.is_none();
        let mut v = Validator { out: Vec::new(), sdk_allows: self.ctx.sdk_allows, is_root_file };
        v.file_options(options, source);
        self.append(v.out, file, is_root_file);
        let Some(include) = options.field("include") else {
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
            let (path, from, length) =
                (included.to_string_lossy().into_owned(), previous.start.to_string(), (previous.end - previous.start).to_string());
            self.errors.push(Report::new(
                &c::INCLUDED_FILE_WARNING,
                where_,
                &[path.as_str(), from.as_str(), length.as_str(), "The file includes itself recursively."],
            ));
            return;
        }
        self.chain.push((included.clone(), include.span));
        let text = std::fs::read_to_string(&included).unwrap_or_default();
        match map_of(&text) {
            Ok(included_map) => {
                self.validate(&included, &included_map, &text);
                if self.first_plugin.is_none() {
                    self.first_plugin = first_plugin(&included_map);
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
mod tests {
    use super::*;

    /// `(code name, range text)` of each report, without includes.
    fn findings(source: &str) -> Vec<(&'static str, String)> {
        let without_includes = |_: &Path, _: &str| None;
        let ctx = OptionsContext { file: Path::new("analysis_options.yaml"), context_root: ".", resolver: &without_includes, sdk_allows: None };
        analyze(source, &ctx).into_iter().map(|r| (r.code.name, source[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn analyzer_section() {
        assert_eq!(findings("analyzer:\n  exclude: [a]\n  erros: {}\n"), vec![("unsupported_option_with_legal_values", "erros".to_string())]);
        assert_eq!(
            findings("analyzer:\n  errors:\n    nao_existe: ignore\n    dead_code: fatal\n"),
            vec![("unrecognized_error_code", "nao_existe".to_string()), ("unsupported_option_with_legal_values", "fatal".to_string())]
        );
        assert_eq!(findings("analyzer:\n  language:\n    strict-casts: sim\n"), vec![("unsupported_value", "sim".to_string())]);
        assert!(findings("analyzer:\n  language:\n    strict-casts: true\n  errors:\n    dead_code: ignore\n    todo: warning\n").is_empty());
    }

    #[test]
    fn lint_rules_section() {
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
    fn include_and_syntax() {
        assert_eq!(findings("include: nao_existe.yaml\n"), vec![("include_file_not_found", "nao_existe.yaml".to_string())]);
        assert_eq!(findings("analyzer: [\n").first().map(|r| r.0), Some("parse_error"));
    }

    #[test]
    fn quoted_list_formats() {
        assert_eq!(quoted_list(&["a"]), "'a'");
        assert_eq!(quoted_list(&["a", "b"]), "'a' and 'b'");
        assert_eq!(quoted_list(&["a", "b", "c"]), "'a', 'b', and 'c'");
    }
}
