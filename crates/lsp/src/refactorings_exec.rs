//! A execução das refatorações (`refactor.perform`, `refactor.validate` e
//! `dart.refactor.move_top_level_to_file`; docs/LSP-ESPECIFICACAO.md §13.11.2
//! a §13.11.9): o `RefactoringStatus`, os utilitários de texto do
//! `CorrectionUtils`, a validação e a sugestão de nomes, e as refatorações
//! Extract Local Variable, Inline Local Variable, Convert Getter to Method e
//! Convert Method to Getter. Extract Method, Inline Method e Move ficam em
//! `refactorings_method.rs`, `refactorings_inline.rs` e
//! `refactorings_move.rs`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Edit;
use crate::project::{Target, Concrete, Project};
use crate::refactorings::{Context, Elem, RefactoringRequest, RefactoringResult, LocalSelection};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use dartforge_frontend::token::Kind;
use dartforge_types::{Resolved, Type, TypeId};
use std::collections::{BTreeSet, HashMap, HashSet};

type R = RefactoringResult;

// -- RefactoringStatus ---------------------------------------------------------

/// A severidade de um problema (`RefactoringProblemSeverity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Severity {
    Warning,
    Error,
    Fatal,
}

/// `RefactoringStatus`: os problemas na ordem de inserção.
#[derive(Debug, Clone, Default)]
pub(crate) struct State {
    pub(crate) problems: Vec<(Severity, String)>,
}

impl State {
    pub(crate) fn fatal(m: impl Into<String>) -> State {
        State { problems: vec![(Severity::Fatal, m.into())] }
    }

    pub(crate) fn error(m: impl Into<String>) -> State {
        State { problems: vec![(Severity::Error, m.into())] }
    }

    pub(crate) fn warning(m: impl Into<String>) -> State {
        State { problems: vec![(Severity::Warning, m.into())] }
    }

    pub(crate) fn add(&mut self, s: Severity, m: impl Into<String>) {
        self.problems.push((s, m.into()));
    }

    /// `addStatus`.
    pub(crate) fn absorb(&mut self, other: State) {
        self.problems.extend(other.problems);
    }

    pub(crate) fn max(&self) -> Option<Severity> {
        self.problems.iter().map(|(s, _)| *s).max()
    }

    pub(crate) fn has_fatal(&self) -> bool {
        self.max() == Some(Severity::Fatal)
    }

    /// `hasError`: `ERROR` ou `FATAL`.
    pub(crate) fn has_error(&self) -> bool {
        self.max().is_some_and(|s| s >= Severity::Error)
    }

    pub(crate) fn ok(&self) -> bool {
        self.problems.is_empty()
    }

    /// `message`: a do primeiro problema de severidade máxima.
    pub(crate) fn message(&self) -> Option<String> {
        let max = self.max()?;
        self.problems.iter().find(|(s, _)| *s == max).map(|(_, m)| m.clone())
    }
}

/// O resultado de uma refatoração legada: `refactor.validate` olha só as
/// condições iniciais; `refactor.perform`, as iniciais (fatal interrompe) e
/// as finais, depois a mudança.
pub(crate) fn finish(
    request: &RefactoringRequest,
    initial: State,
    finals: impl FnOnce() -> State,
    change: impl FnOnce() -> Change,
) -> RefactoringResult {
    if request.only_validate {
        return match (initial.has_error(), initial.message()) {
            (true, Some(m)) => R::Error(m),
            _ => R::Valid,
        };
    }
    let mut all = initial;
    if !all.has_fatal() {
        all.absorb(finals());
    }
    if all.has_error() {
        return R::Error(all.message().unwrap_or_default());
    }
    change().result()
}

// -- SourceChange ----------------------------------------------------------------

/// `SourceChange`: as edições por arquivo, na ordem do `SourceFileEdit.add`
/// (decrescente por offset; uma nova com o mesmo offset entra antes das
/// existentes), e um arquivo novo.
#[derive(Debug, Clone, Default)]
pub(crate) struct Change {
    pub(crate) files: Vec<(String, Vec<Edit>)>,
    pub(crate) create: Option<(String, String)>,
    /// A primeira `ConflictingEditException` (edições que se sobrepõem).
    pub(crate) conflict: Option<String>,
}

impl Change {
    /// `addEdit` / `addEditForSource`.
    pub(crate) fn add(&mut self, uri: &str, span: Span, text: impl Into<String>) {
        let e = Edit { uri: uri.to_string(), span, text: text.into() };
        let list = match self.files.iter().position(|(u, _)| u == uri) {
            Some(i) => &mut self.files[i].1,
            None => {
                self.files.push((uri.to_string(), Vec::new()));
                &mut self.files.last_mut().unwrap().1
            }
        };
        let mut i = 0;
        while i < list.len() && list[i].span.start > e.span.start {
            i += 1;
        }
        // As verificações de sobreposição do `addEditForSource`.
        let conflicts = (i > 0 && e.span.end > list[i - 1].span.start)
            || (i < list.len() && {
                let next = &list[i];
                (e.span.start == next.span.start && e.span.end > e.span.start && next.span.end > next.span.start)
                    || next.span.end > e.span.start
            });
        if conflicts {
            if self.conflict.is_none() {
                self.conflict = Some(format!("ConflictingEditException: {}:{} conflicts with an existing edit", e.span.start, e.span.end - e.span.start));
            }
            return;
        }
        list.insert(i, e);
    }

    pub(crate) fn empty(&self) -> bool {
        self.create.is_none() && self.files.iter().all(|(_, l)| l.is_empty())
    }

    /// A mudança na forma do LSP: `sortSourceEditsForLsp` inverte a lista
    /// de cada arquivo (crescente por offset; inserções no mesmo offset na
    /// ordem em que ficam no texto).
    pub(crate) fn result(self) -> RefactoringResult {
        if let Some(c) = self.conflict {
            return R::InternalError(c);
        }
        R::Change { edits: self.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(), create: self.create }
    }
}

// -- CorrectionUtils ---------------------------------------------------------------

/// Os utilitários de texto do `CorrectionUtils`
/// (`analysis_server_plugin/lib/edit/correction_utils.dart`).
#[derive(Clone, Copy)]
pub(crate) struct Text<'a> {
    pub(crate) t: &'a str,
}

/// Dois espaços (`oneIndent`).
pub(crate) const ONE_INDENT: &str = "  ";

impl<'a> Text<'a> {
    pub(crate) fn new(t: &'a str) -> Text<'a> {
        Text { t }
    }

    fn b(&self, i: usize) -> u8 {
        self.t.as_bytes()[i]
    }

    /// `endOfLine`.
    pub(crate) fn eol(&self) -> &'static str {
        if self.t.contains("\r\n") {
            "\r\n"
        } else if self.t.contains('\n') {
            "\n"
        } else if cfg!(windows) {
            "\r\n"
        } else {
            "\n"
        }
    }

    /// `getLineContentEnd`.
    pub(crate) fn content_end(&self, mut i: usize) -> usize {
        let n = self.t.len();
        while i < n && matches!(self.b(i), b' ' | b'\t') {
            i += 1;
        }
        if i < n && self.b(i) == b'\r' {
            i += 1;
        }
        if i < n && self.b(i) == b'\n' {
            i += 1;
        }
        i
    }

    /// `getLineContentStart`.
    pub(crate) fn content_start(&self, mut i: usize) -> usize {
        while i > 0 && matches!(self.b(i - 1), b' ' | b'\t') {
            i -= 1;
        }
        i
    }

    /// `getLineNext`.
    pub(crate) fn next_line(&self, mut i: usize) -> usize {
        let n = self.t.len();
        while i < n && !matches!(self.b(i), b'\r' | b'\n') {
            i += 1;
        }
        if i < n && self.b(i) == b'\r' {
            i += 1;
        }
        if i < n && self.b(i) == b'\n' {
            i += 1;
        }
        i
    }

    /// `getLineThis`.
    pub(crate) fn line_start(&self, mut i: usize) -> usize {
        while i > 0 && !matches!(self.b(i - 1), b'\r' | b'\n') {
            i -= 1;
        }
        i
    }

    /// `getLinePrefix`.
    pub(crate) fn line_prefix(&self, i: usize) -> &'a str {
        let start = self.line_start(i);
        let mut j = start;
        let n = self.t.len();
        while j < n && matches!(self.b(j), b' ' | b'\t') {
            j += 1;
        }
        &self.t[start..j]
    }

    /// `getPrefix`.
    pub(crate) fn prefix(&self, i: usize) -> &'a str {
        &self.t[self.content_start(i)..i]
    }

    /// `getLinesRange`.
    pub(crate) fn line_range(&self, start_: usize, end: usize) -> Span {
        let start = self.content_start(start_);
        let mut after = end;
        if self.line_start(start) == start {
            after = self.content_end(end);
        }
        Span { start: start, end: after }
    }

    /// `indentSourceLeftRight(indentLeft: true)`.
    pub(crate) fn dedent(&self, source: &str) -> String {
        let eol = self.eol();
        let lines: Vec<&str> = source.split(eol).collect();
        let mut s = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i == lines.len() - 1 && line.is_empty() {
                break;
            }
            s.push_str(line.strip_prefix(ONE_INDENT).unwrap_or(line));
            s.push_str(eol);
        }
        s
    }

    /// `replaceSourceIndent`.
    pub(crate) fn swap_indent(&self, source: &str, old: &str, new: &str, include_start: bool, ensure_eol: bool) -> String {
        // As faixas dos tokens de string (uma linha que começa dentro de
        // uma string de várias linhas não muda).
        let strings: Vec<Span> = dartforge_frontend::lexer::lex(source)
            .map(|v| {
                v.iter()
                    .filter(|t| matches!(t.kind, Kind::Str(_) | Kind::StrBegin(..) | Kind::StrMid(..) | Kind::StrEnd(_)))
                    .map(|t| t.span)
                    .collect()
            })
            .unwrap_or_default();
        let eol = self.eol();
        let lines: Vec<&str> = source.split(eol).collect();
        let mut s = String::new();
        let mut line_offset = 0usize;
        for (i, &line) in lines.iter().enumerate() {
            if i == lines.len() - 1 && line.is_empty() {
                break;
            }
            let swap = i != 0 || include_start;
            let eol_after = i != lines.len() - 1 || ensure_eol;
            let mut in_string = false;
            for r in &strings {
                if line_offset > r.start && line_offset < r.end {
                    in_string = true;
                }
                if line_offset > r.end {
                    break;
                }
            }
            line_offset += line.len() + eol.len();
            if !in_string && swap {
                s.push_str(new);
                s.push_str(line.strip_prefix(old).unwrap_or(line));
            } else {
                s.push_str(line);
            }
            if eol_after {
                s.push_str(eol);
            }
        }
        s
    }
}

impl Context<'_> {
    /// `getNodePrefix`.
    pub(crate) fn node_prefix(&self, n: usize) -> String {
        let tx = Text::new(self.source);
        let o = self.tree.nos[n].inicio;
        if self.kind(n) == "FunctionExpression" { tx.line_prefix(o).to_string() } else { tx.prefix(o).to_string() }
    }

    /// O texto de um nó.
    pub(crate) fn node_text(&self, n: usize) -> &str {
        let no = &self.tree.nos[n];
        &self.source[no.inicio..no.fim]
    }

    /// A expressão da árvore do parser do nó: a da marca de um
    /// `SimpleIdentifier`, senão a mais externa com o mesmo intervalo (os pais
    /// entram na arena depois dos filhos).
    pub(crate) fn node_expr(&self, n: usize) -> Option<ExprId> {
        let no = &self.tree.nos[n];
        if let crate::arvore_analyzer::Marca::Expr(x) = no.marca
            && self.kind(n) == "SimpleIdentifier"
        {
            return Some(x);
        }
        self.ast
            .exprs
            .iter()
            .enumerate()
            .filter(|(_, e)| e.span.start == no.inicio && e.span.end == no.fim)
            .map(|(i, _)| ExprId(i as u32))
            .max_by_key(|x| x.0)
    }

    /// O tipo estático (`staticType`) do nó de expressão.
    pub(crate) fn node_type(&self, n: usize) -> Option<TypeId> {
        let x = self.node_expr(n)?;
        self.bodies.get_type(x)
    }
}

/// A regra de lint `name` está ligada para o arquivo da unidade
/// (`analysis_options.yaml` da subpasta ou da raiz).
pub(crate) fn enabled_rule(p: &Project, unit: UnitId, name: &str) -> bool {
    let Some(path) = p.program().unit(unit).path.as_deref() else { return false };
    rule_enabled_in(path, name)
}

/// Como [`enabled_rule`], pelo caminho do arquivo.
pub(crate) fn rule_enabled_in(path: &std::path::Path, name: &str) -> bool {
    let root = crate::project::project_root(path);
    let file = dartforge_parity::filtros::Opcoes::de_subpasta(path, &root).unwrap_or_else(|| root.join("analysis_options.yaml"));
    dartforge_parity::filtros::Opcoes::ler_arquivo(&file).regras.get(name).copied().unwrap_or(false)
}

/// `options['name'] as String` com `options != null`: o nome, ou o erro do
/// `as` que vira `UnhandledError`.
pub(crate) fn options_name(request: &RefactoringRequest) -> Result<Option<String>, RefactoringResult> {
    match &request.options {
        None => Ok(None),
        Some(m) => match m.get("name") {
            Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
            Some(v) => Err(R::InternalError(format!("type '{}' is not a subtype of type 'String' in type cast", json_type(v)))),
            None => Err(R::InternalError("type 'Null' is not a subtype of type 'String' in type cast".to_string())),
        },
    }
}

fn json_type(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "Null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => "int",
        serde_json::Value::Number(_) => "double",
        serde_json::Value::String(_) => "String",
        serde_json::Value::Array(_) => "List<dynamic>",
        serde_json::Value::Object(_) => "_Map<String, dynamic>",
    }
}

// -- Nomes -------------------------------------------------------------------------

/// As palavras reservadas (`KeywordStyle.reserved`); as outras palavras-chave
/// são embutidas ou pseudo.
const RESERVED: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "false", "final", "finally", "for", "if",
    "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while", "with",
];

/// As embutidas e as pseudo (`isBuiltInOrPseudo`).
const INLINE: &[&str] = &[
    "abstract", "as", "async", "augment", "await", "base", "covariant", "deferred", "dynamic", "export", "extension", "external", "factory", "Function",
    "get", "hide", "implements", "import", "inout", "interface", "late", "library", "mixin", "native", "of", "on", "operator", "out", "part", "patch",
    "required", "sealed", "set", "show", "source", "static", "sync", "typedef", "when", "yield",
];

/// `_validateLowerCamelCase(name, desc, allowBuiltIn: true)`
/// (`legacy/naming_conventions.dart:144-220`).
pub(crate) fn validate_name(name: &str, desc: &str) -> State {
    let desc = format!("{desc} name");
    if name != name.trim() {
        return State::fatal(format!("{desc} must not start or end with a blank."));
    }
    if name.is_empty() {
        return State::fatal(format!("{desc} must not be empty."));
    }
    if INLINE.contains(&name) {
        return State::warning("Avoid using built-in identifiers as names.");
    }
    if RESERVED.contains(&name) {
        return State::fatal(format!("{desc} must not be a keyword."));
    }
    for c in name.chars() {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '$') {
            return State::fatal(format!("{desc} must not contain '{c}'."));
        }
    }
    let first = name.as_bytes()[0];
    if !(first.is_ascii_alphabetic() || first == b'_' || first == b'$') {
        return State::fatal(format!("{desc} must begin with a lowercase letter or underscore."));
    }
    if first == b'_' || first == b'$' {
        return State::default();
    }
    if !first.is_ascii_lowercase() {
        return State::warning(format!("{desc} should start with a lowercase letter."));
    }
    State::default()
}

/// `getCamelWords`.
pub(crate) fn camel_words(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let b = s.as_bytes();
    let mut parts = Vec::new();
    let (mut was_lower, mut was_upper) = (false, false);
    let mut start = 0;
    for i in 0..b.len() {
        let c = b[i];
        let min = c.is_ascii_lowercase();
        let upper = c.is_ascii_uppercase();
        if was_lower && upper {
            parts.push(s[start..i].to_string());
            start = i;
        }
        if was_upper && upper && i + 1 < b.len() && b[i + 1].is_ascii_lowercase() {
            parts.push(s[start..i].to_string());
            start = i;
        }
        was_lower = min;
        was_upper = upper;
    }
    parts.push(s[start..].to_string());
    parts
}

/// `getCamelWordCombinations`.
pub(crate) fn camel_combinations(name: &str) -> Vec<String> {
    let parts = camel_words(name);
    (0..parts.len()).map(|i| format!("{}{}", parts[i].to_lowercase(), parts[i + 1..].concat())).collect()
}

/// `capitalize`.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `_addAll`: o primeiro de `item`, `item2`, `item3`… não excluído.
pub(crate) fn add_all(excluded: &HashSet<String>, output: &mut Vec<String>, items: Vec<String>, prefix: Option<&str>) {
    for item in items {
        let mut suffix = 1;
        loop {
            let name = if suffix > 1 { format!("{item}{suffix}") } else { item.clone() };
            if !excluded.contains(&name) {
                let final_ = match prefix {
                    Some(p) => format!("{p}{}", capitalize(&name)),
                    None => name,
                };
                if !output.contains(&final_) {
                    output.push(final_);
                }
                break;
            }
            suffix += 1;
        }
    }
}

/// `_addSingleCharacterName`.
pub(crate) fn add_letter(excluded: &HashSet<String>, output: &mut Vec<String>, mut c: u8) {
    while c < b'z' {
        let name = (c as char).to_string();
        if !excluded.contains(&name) {
            if !output.contains(&name) {
                output.push(name);
            }
            break;
        }
        c += 1;
    }
}

/// `getVariableNameSuggestionsForText`.
pub(crate) fn names_to_text(text: &str, excluded: &HashSet<String>) -> Vec<String> {
    let filtered: String = text.chars().filter(|c| c.is_alphabetic() || matches!(c, ' ' | '\t' | '\r' | '\n')).collect();
    let mut camel = String::new();
    for (i, word) in filtered.split(' ').enumerate() {
        if i > 0 {
            camel.push_str(&capitalize(word));
        } else {
            camel.push_str(word);
        }
    }
    let mut output = Vec::new();
    add_all(excluded, &mut output, camel_combinations(&camel), None);
    output
}

impl Context<'_> {
    /// `getVariableNameSuggestionsForExpression(tipo, n, excluidos,
    /// isMethod)` (`name_suggestion.dart:29-76`).
    pub(crate) fn names_to_expression(&self, ty: Option<TypeId>, n: usize, excluded: &HashSet<String>, method: bool) -> Vec<String> {
        let mut prefix: Option<&str> = None;
        if method
            && let Some(m) = self.with_parents(n).find(|&k| self.kind(k) == "MethodDeclaration")
            && let crate::arvore_analyzer::Marca::Funcao(fid) = self.tree.nos[m].marca
            && let Some(name) = self.ast.function(fid).name
            && self.source[name.span.start..name.span.end].starts_with("build")
        {
            prefix = Some("build");
        }
        let mut output = Vec::new();
        if let Some(mut name) = self.expression_name_base(n) {
            if let Some(r) = name.strip_prefix('_') {
                name = r.to_string();
            }
            add_all(excluded, &mut output, camel_combinations(&name), prefix);
        }
        if let Some(name) = self.name_by_position(n) {
            add_all(excluded, &mut output, camel_combinations(&name), None);
        }
        if let Some(t) = ty {
            let table = &self.p.query.table;
            let core = &self.p.query.core;
            match table.get(t) {
                Type::Dynamic => {}
                _ if table.canonico(t) == table.canonico(core.int) => add_letter(excluded, &mut output, b'i'),
                _ if table.canonico(t) == table.canonico(core.double) => add_letter(excluded, &mut output, b'd'),
                _ if table.canonico(t) == table.canonico(core.string) => add_letter(excluded, &mut output, b's'),
                Type::Interface { class, .. } => {
                    let name = self.p.name(self.p.program().class(*class).name).to_string();
                    add_all(excluded, &mut output, camel_combinations(&name), None);
                }
                _ => {}
            }
        }
        output
    }

    /// `_getBaseNameFromExpression`.
    fn expression_name_base(&self, mut n: usize) -> Option<String> {
        while matches!(self.kind(n), "AsExpression" | "ParenthesizedExpression") {
            n = *self.children(n).first()?;
        }
        let text = |k: usize| self.node_text(k).to_string();
        let name: String = match self.kind(n) {
            "SimpleIdentifier" => return Some(text(n)),
            "PrefixedIdentifier" => return self.children(n).get(1).map(|&k| text(k)),
            "PropertyAccess" => return self.children(n).last().map(|&k| text(k)),
            "MethodInvocation" => text(self.method_name(n)?),
            "InstanceCreationExpression" => {
                let cn = *self.children(n).iter().find(|&&k| self.kind(k) == "ConstructorName")?;
                let ty = *self.children(cn).first()?;
                // O nome do `NamedType` (depois do prefixo de import).
                let no = &self.tree.nos[ty];
                let start = self.children(ty).iter().find(|&&k| self.kind(k) == "ImportPrefixReference").map_or(no.inicio, |&k| self.tree.nos[k].fim);
                let rest = &self.source[start..no.fim];
                let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(rest.len());
                return Some(rest[..end].trim().to_string());
            }
            "IndexExpression" => {
                let target = self.real_index_target(n)?;
                let mut name = self.expression_name_base(target)?;
                if name.ends_with('s') {
                    name.pop();
                }
                name
            }
            _ => return None,
        };
        for p in ["get", "is", "to"] {
            if let Some(r) = name.strip_prefix(p) {
                if r.is_empty() {
                    return None;
                }
                if r.as_bytes()[0].is_ascii_uppercase() {
                    return Some(r.to_string());
                }
            }
        }
        Some(name)
    }

    /// `IndexExpression.realTarget`: o alvo, ou o da cascata.
    fn real_index_target(&self, n: usize) -> Option<usize> {
        let children = self.children(n);
        if children.len() >= 2 {
            return children.first().copied();
        }
        // `..[i]`: o alvo da cascata.
        let mut k = n;
        while let Some(p) = self.parent(k) {
            if self.kind(p) == "CascadeExpression" {
                return self.children(p).first().copied();
            }
            k = p;
        }
        None
    }

    /// `_getBaseNameFromLocationInParent`: o rótulo do argumento nomeado, ou
    /// o nome do parâmetro posicional correspondente.
    fn name_by_position(&self, n: usize) -> Option<String> {
        let p = self.parent(n)?;
        if self.kind(p) == "NamedExpression" && self.children(p).get(1) == Some(&n) {
            let label = *self.children(p).first()?;
            let id = *self.children(label).first()?;
            return Some(self.node_text(id).to_string());
        }
        if self.kind(p) != "ArgumentList" {
            return None;
        }
        // O índice entre os posicionais.
        let positional: Vec<usize> = self.children(p).iter().copied().filter(|&k| self.kind(k) != "NamedExpression").collect();
        let index = positional.iter().position(|&k| k == n)?;
        let owner = self.parent(p)?;
        let f = match self.kind(owner) {
            "MethodInvocation" => match self.identifier_element(self.method_name(owner)?, true) {
                Elem::Function(f) => f,
                _ => return None,
            },
            "InstanceCreationExpression" => {
                let x = self.node_expr(owner)?;
                match self.bodies.get_resolved(x) {
                    Some(Resolved::Constructor(f)) => *f,
                    _ => {
                        let cn = *self.children(owner).iter().find(|&&k| self.kind(k) == "ConstructorName")?;
                        let _ = cn;
                        let target = self.constructors_by_target.iter().find(|(a, _)| {
                            let s = self.ast.expr(**a).span;
                            s.start >= self.tree.nos[owner].inicio && s.end <= self.tree.nos[owner].fim
                        })?;
                        *target.1
                    }
                }
            }
            _ => return None,
        };
        let data = self.p.query.outline.functions.get(f.0 as usize)?;
        let parameter = data.parameters.iter().filter(|q| q.kind != ast::ParameterKind::Named).nth(index)?;
        Some(self.p.name(parameter.name?).to_string())
    }
}

// -- Despacho ----------------------------------------------------------------------

/// `refactor.perform`/`refactor.validate` de uma refatoração legada.
pub(crate) fn run(p: &Project, uri: &str, request: &RefactoringRequest) -> RefactoringResult {
    let Some(unit) = p.uri_unit(uri) else { return R::NotAnalyzed };
    let cx = Context::new(p, unit);
    let command = if request.only_validate { "Validate Refactor" } else { "Perform Refactor" };
    match request.kind.as_str() {
        "EXTRACT_METHOD" => crate::refactorings_method::run(&cx, uri, request),
        "EXTRACT_LOCAL_VARIABLE" => extract_local(&cx, uri, request),
        "EXTRACT_WIDGET" => extract_widget(&cx, request),
        "INLINE_LOCAL_VARIABLE" => inline_local(&cx, uri, request),
        "INLINE_METHOD" => crate::refactorings_inline::run(&cx, uri, request),
        "CONVERT_GETTER_TO_METHOD" => getter_to_method(&cx, request, command),
        "CONVERT_METHOD_TO_GETTER" => method_to_getter(&cx, request, command),
        k => R::InvalidArguments(format!("Unknown RefactoringKind RefactoringKind.{k} was supplied to {command}")),
    }
}

/// `dart.refactor.move_top_level_to_file`.
pub(crate) fn move_(p: &Project, uri: &str, offset: usize, length: usize, target: &str) -> RefactoringResult {
    let Some(unit) = p.uri_unit(uri) else { return R::NotAnalyzed };
    let cx = Context::new(p, unit);
    crate::refactorings_move::run(&cx, uri, offset, length, target)
}

/// `EXTRACT_WIDGET`: sem o Flutter, nenhuma expressão cria `Widget` nem
/// método o devolve (§13.11.10).
fn extract_widget(cx: &Context<'_>, request: &RefactoringRequest) -> RefactoringResult {
    if let Err(e) = options_name(request) {
        return e;
    }
    let end = request.offset + request.length;
    let no = cx.tree.localizar(request.offset, end);
    let message = match no.map(|n| cx.kind(n)) {
        Some("Block") => "The last selected statement must return a widget.",
        _ => "Can only extract a widget expression or a method returning widget.",
    };
    let initial = State::fatal(message);
    finish(request, initial, State::default, Change::default)
}

// -- Extract Local Variable ----------------------------------------------------------

/// `ExtractLocalRefactoringImpl` (§13.11.4).
fn extract_local(cx: &Context<'_>, uri: &str, request: &RefactoringRequest) -> RefactoringResult {
    let preferred = match options_name(request) {
        Ok(n) => n,
        Err(e) => return e,
    };
    let selection = cx.local_selection(request.offset, request.length);
    let (initial, sel) = match selection {
        Err(m) => (State::fatal(m), None),
        Ok(s) => (State::default(), Some(s)),
    };
    // `findPossibleLocalVariableConflicts(selectionOffset)` e os nomes.
    let excluded = cx.local_conflicts(request.offset);
    let names = match &sel {
        Some(s) => match (&s.string_part, s.expression) {
            (Some(part), _) => names_to_text(part, &excluded),
            (None, Some(e)) => cx.names_to_expression(cx.node_type(e), e, &excluded, false),
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    let name = preferred.or_else(|| names.first().cloned()).unwrap_or_else(|| "newVariable".to_string());
    let finals = || {
        let mut e = validate_name(&name, "Variable");
        if excluded.contains(&name) {
            e.add(Severity::Error, format!("The name '{name}' is already used in the scope."));
        }
        e
    };
    let sel2 = sel.clone();
    finish(request, initial, finals, || match sel2 {
        Some(s) => cx.extract_local_change(uri, &s, &name),
        None => Change::default(),
    })
}

impl Context<'_> {
    /// `findPossibleLocalVariableConflicts(offset)`: no `Block` mais interno
    /// do nó do offset, os nomes referidos sem prefixo e os declarados por
    /// `VariableDeclaration`.
    pub(crate) fn local_conflicts(&self, offset: usize) -> HashSet<String> {
        let mut names = HashSet::new();
        let Some(n) = self.tree.localizar(offset, offset) else { return names };
        let Some(block) = self.with_parents(n).find(|&k| self.kind(k) == "Block") else { return names };
        let mut stack = vec![block];
        while let Some(k) = stack.pop() {
            match self.kind(k) {
                "NamedType" => {
                    let has_prefix = self.children(k).iter().any(|&f| self.kind(f) == "ImportPrefixReference");
                    if !has_prefix {
                        let no = &self.tree.nos[k];
                        let rest = &self.source[no.inicio..no.fim];
                        let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(rest.len());
                        names.insert(rest[..end].to_string());
                    }
                }
                "SimpleIdentifier" => {
                    if !self.prefixed_identifier(k) && !self.parent(k).is_some_and(|p| self.kind(p) == "Label") {
                        names.insert(self.node_text(k).to_string());
                    }
                }
                "VariableDeclaration" => {
                    let no = &self.tree.nos[k];
                    let rest = &self.source[no.inicio..no.fim];
                    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(rest.len());
                    names.insert(rest[..end].to_string());
                }
                _ => {}
            }
            stack.extend(self.children(k).iter().copied());
        }
        names
    }

    /// `_ReferencedUnprefixedNamesCollector._isPrefixed`.
    fn prefixed_identifier(&self, n: usize) -> bool {
        let Some(p) = self.parent(n) else { return false };
        match self.kind(p) {
            "ConstructorName" => self.children(p).get(1) == Some(&n),
            "MethodInvocation" => {
                self.method_name(p) == Some(n)
                    && (self.children(p).first() != Some(&n) || self.parent(p).is_some_and(|c| self.kind(c) == "CascadeExpression" && self.children(c).first() != Some(&p)))
            }
            "PrefixedIdentifier" => self.children(p).get(1) == Some(&n),
            "PropertyAccess" => self.children(p).len() > 1 && self.children(p).first() == Some(&n),
            _ => false,
        }
    }

    /// `_isPartOfConstantExpression`.
    fn constant_part(&self, n: Option<usize>) -> bool {
        let Some(n) = n else { return false };
        match self.kind(n) {
            "ListLiteral" | "SetOrMapLiteral" | "InstanceCreationExpression" => self.constant(n),
            "ArgumentList" | "ConditionalExpression" | "BinaryExpression" | "ParenthesizedExpression" | "PrefixExpression" | "MapLiteralEntry"
            | "IntegerLiteral" | "DoubleLiteral" | "BooleanLiteral" | "NullLiteral" | "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings"
            | "SymbolLiteral" | "RecordLiteral" => self.constant_part(self.parent(n)),
            _ => false,
        }
    }

    /// `TypedLiteral.isConst`/`InstanceCreationExpression.isConst`: `const`
    /// escrito, ou (sem palavra-chave) o contexto constante.
    fn constant(&self, n: usize) -> bool {
        let no = &self.tree.nos[n];
        if self.source[no.inicio..no.fim].starts_with("const") && !self.source[no.inicio + 5..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            return true;
        }
        if self.source[no.inicio..no.fim].starts_with("new") && !self.source[no.inicio + 3..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            return false;
        }
        let Some(x) = self.node_expr(n) else { return false };
        let u = self.p.program().unit(self.unit);
        let before_3 = self.p.program().library(u.library).features.versao().major < 3;
        let parents = dartforge_frontend::pais::Pais::novo(self.ast, &u.unit, self.source, before_3);
        parents.em_contexto_constante(self.ast, x)
    }

    /// `_declarationKeywordAndType`.
    fn word_and_type(&self, s: &LocalSelection) -> String {
        let use_const = s.string_part.is_none() && self.constant_part(s.expression);
        let use_final = enabled_rule(self.p, self.unit, "prefer_final_locals");
        let ty: Option<String> = if enabled_rule(self.p, self.unit, "always_specify_types") {
            match (s.expression, &s.string_part) {
                (Some(e), _) => self.node_type(e).map(|t| self.p.query.format(t)),
                (None, Some(_)) => Some("String".to_string()),
                _ => None,
            }
        } else {
            None
        };
        match (use_const, use_final, ty) {
            (true, _, Some(t)) => format!("const {t}"),
            (true, _, None) => "const".to_string(),
            (false, true, Some(t)) => format!("final {t}"),
            (false, true, None) => "final".to_string(),
            (false, false, Some(t)) => t,
            (false, false, None) => "var".to_string(),
        }
    }

    /// `ExtractLocalRefactoringImpl.createChange` com `extractAll = false`.
    fn extract_local_change(&self, uri: &str, s: &LocalSelection, name: &str) -> Change {
        let mut m = Change::default();
        let tx = Text::new(self.source);
        let (start, end) = s.range;
        let word = self.word_and_type(s);
        // Atalho: a expressão inteira de um `ExpressionStatement`.
        if let Some(e) = s.expression
            && self.parent(e).is_some_and(|p| self.kind(p) == "ExpressionStatement")
        {
            let o = self.tree.nos[e].inicio;
            m.add(uri, Span { start: o, end: o }, format!("{word} {name} = "));
            return m;
        }
        let mut declaration = format!("{word} ");
        match &s.string_part {
            Some(part) => declaration.push_str(&format!("{name} = '{part}';")),
            None => declaration.push_str(&format!("{name} = {};", &self.source[start..end])),
        }
        let eol = tx.eol();
        match self.declaration_target(start) {
            Some(Target2::Command(target)) => {
                let prefix = self.node_prefix(target);
                let o = self.tree.nos[target].inicio;
                m.add(uri, Span { start: o, end: o }, format!("{declaration}{eol}{prefix}"));
            }
            Some(Target2::ExpressionBody(body)) => {
                let prefix = self.parent(body).map(|p| self.node_prefix(p)).unwrap_or_default();
                let expr = *self.children(body).first().unwrap_or(&body);
                let (ci, cf) = (self.tree.nos[body].inicio, self.tree.nos[body].fim);
                let (ei, ef) = (self.tree.nos[expr].inicio, self.tree.nos[expr].fim);
                let code = format!("{{{eol}{prefix}{ONE_INDENT}{declaration}{eol}{prefix}{ONE_INDENT}return ");
                m.add(uri, Span { start: ci, end: ei }, code);
                m.add(uri, Span { start: ef, end: cf }, format!(";{eol}{prefix}}}"));
            }
            None => {}
        }
        let swap = if s.string_part.is_some() { format!("${{{name}}}") } else { name.to_string() };
        m.add(uri, Span { start: start, end: end }, swap);
        m
    }

    /// `_findDeclarationTarget([selectionRange])`.
    fn declaration_target(&self, offset: usize) -> Option<Target2> {
        let common = self.tree.localizar(offset, offset)?;
        let path: Vec<usize> = self.with_parents(common).collect();
        let _ = &path;
        // `Block`, `SwitchCase`, `SwitchPatternCase`: o filho no caminho (com
        // uma ocorrência, o comum é o próprio nó, que não é um deles na
        // prática; a regra vale igual).
        if matches!(self.kind(common), "Block" | "SwitchCase" | "SwitchPatternCase") {
            return None;
        }
        // `ExpressionFunctionBody` antes de qualquer comando.
        for k in self.with_parents(common) {
            if crate::refactorings::is_command_kind(self.kind(k)) {
                break;
            }
            if self.kind(k) == "ExpressionFunctionBody" {
                return Some(Target2::ExpressionBody(k));
            }
        }
        // O comando de nível de bloco.
        let mut target = self.with_parents(common).find(|&k| crate::refactorings::is_command_kind(self.kind(k)))?;
        loop {
            let parent = self.parent(target)?;
            if matches!(self.kind(parent), "Block" | "SwitchCase" | "SwitchPatternCase") {
                return Some(Target2::Command(target));
            }
            target = parent;
        }
    }
}

/// O alvo da declaração do novo local.
enum Target2 {
    Command(usize),
    ExpressionBody(usize),
}

// -- Inline Local Variable -----------------------------------------------------------

/// A espécie de uma referência a um local (`search.dart:1572-1601`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceKind {
    Read,
    Write,
    ReadWrite,
    Invocation,
}

/// `InlineLocalRefactoringImpl` (§13.11.5).
fn inline_local(cx: &Context<'_>, uri: &str, request: &RefactoringRequest) -> RefactoringResult {
    let o = request.offset;
    let message = "Local variable declaration or reference must be selected to activate this refactoring.";
    let prepare = || -> Result<(usize, usize, usize, usize), State> {
        // O elemento e a declaração dele.
        let n = cx.tree.localizar(o, o).ok_or_else(|| State::fatal(message))?;
        let decl = match cx.kind(n) {
            "SimpleIdentifier" => {
                if cx.identifier_element(n, true) != Elem::LocalVariable {
                    return Err(State::fatal(message));
                }
                let crate::arvore_analyzer::Marca::Expr(x) = cx.tree.nos[n].marca else { return Err(State::fatal(message)) };
                cx.bodies.declaracao_local(x).ok_or_else(|| State::fatal(message))?
            }
            "VariableDeclaration" if cx.tree.nos[n].marca == crate::arvore_analyzer::Marca::VariavelLocal => cx.tree.nos[n].inicio,
            _ => return Err(State::fatal(message)),
        };
        // `getElementDeclaration`: o nó da declaração é `VariableDeclaration`.
        let vd = cx
            .tree
            .nos
            .iter()
            .position(|k| k.especie == "VariableDeclaration" && k.inicio == decl)
            .ok_or_else(|| State::fatal(message))?;
        // `_declarationStatement`.
        let list = cx.parent(vd).filter(|&l| cx.kind(l) == "VariableDeclarationList").ok_or_else(|| State::fatal(message))?;
        let command = cx.parent(list).filter(|&s| cx.kind(s) == "VariableDeclarationStatement").ok_or_else(|| State::fatal(message))?;
        if !cx.parent(command).is_some_and(|b| matches!(cx.kind(b), "Block" | "SwitchCase" | "SwitchPatternCase")) {
            return Err(State::fatal(message));
        }
        let name = {
            let no = &cx.tree.nos[vd];
            let rest = &cx.source[no.inicio..no.fim];
            let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(rest.len());
            rest[..end].to_string()
        };
        let Some(&initializer) = cx.children(vd).first() else {
            return Err(State::fatal(format!("Local variable '{name}' is not initialized at declaration.")));
        };
        // Toda referência é leitura.
        if cx.local_references(decl).iter().any(|(_, e)| *e != ReferenceKind::Read) {
            return Err(State::fatal(format!("Local variable '[{name}]' is assigned more than once.")));
        }
        Ok((vd, command, initializer, decl))
    };
    match prepare() {
        Err(e) => finish(request, e, State::default, Change::default),
        Ok((_vd, command, initializer, decl)) => finish(request, State::default(), State::default, || cx.inline_local_change(uri, command, initializer, decl)),
    }
}

impl Context<'_> {
    /// As referências ao local declarado em `decl` (o offset do nome) na
    /// unidade, com a espécie, em ordem de offset.
    fn local_references(&self, decl: usize) -> Vec<(Span, ReferenceKind)> {
        let mut v = Vec::new();
        let invoked: HashSet<ExprId> = self
            .ast
            .exprs
            .iter()
            .filter_map(|e| match &e.kind {
                ExprKind::Call { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        let compound: HashSet<ExprId> = self
            .ast
            .exprs
            .iter()
            .filter_map(|e| match &e.kind {
                ExprKind::Assign { op: ast::AssignOp::Compound(_), target, .. } => Some(*target),
                ExprKind::Unary {
                    op: ast::UnaryOp::PrefixInc | ast::UnaryOp::PrefixDec | ast::UnaryOp::PostfixInc | ast::UnaryOp::PostfixDec,
                    operand,
                } => Some(*operand),
                _ => None,
            })
            .collect();
        for (i, e) in self.ast.exprs.iter().enumerate() {
            let x = ExprId(i as u32);
            let ExprKind::Identifier(n) = &e.kind else { continue };
            if self.bodies.declaracao_local(x) != Some(decl) {
                continue;
            }
            let kind = if compound.contains(&x) {
                ReferenceKind::ReadWrite
            } else if self.writes.contains(&x) {
                ReferenceKind::Write
            } else if invoked.contains(&x) {
                ReferenceKind::Invocation
            } else {
                ReferenceKind::Read
            };
            v.push((n.span, kind));
        }
        // `AssignedVariablePattern`: escrita.
        for (pid, d) in self.bodies.declaracoes_de_padroes.iter() {
            if *d == decl {
                v.push((self.ast.pattern(*pid).span, ReferenceKind::Write));
            }
        }
        v.sort_by_key(|(s, _)| s.start);
        v
    }

    /// `InlineLocalRefactoringImpl.createChange`.
    fn inline_local_change(&self, uri: &str, command: usize, initializer: usize, decl: usize) -> Change {
        let mut m = Change::default();
        let tx = Text::new(self.source);
        let (ci, cf) = (self.tree.nos[command].inicio, self.tree.nos[command].fim);
        m.add(uri, tx.line_range(ci, cf), "");
        let code = self.node_text(initializer).to_string();
        let init_node = &self.tree.nos[initializer];
        for (s, _) in self.local_references(decl) {
            let Some(n) = self.tree.localizar(s.start, s.start) else { continue };
            let parent = self.parent(n);
            let interpolation = parent.filter(|&p| self.kind(p) == "InterpolationExpression");
            let (range, text) = if let Some(ip) = interpolation {
                let target = self.parent(ip);
                let simple_or_interp = matches!(init_node.especie, "SimpleStringLiteral" | "StringInterpolation");
                let merge = target.is_some_and(|a| self.kind(a) == "StringInterpolation")
                    && simple_or_interp
                    && !raw_string(&code)
                    && single_quotes(&code) == single_quotes(self.node_text(target.unwrap()))
                    && (!multiline_string(&code) || multiline_string(self.node_text(target.unwrap())));
                let ipn = &self.tree.nos[ip];
                if merge {
                    (Span { start: ipn.inicio, end: ipn.fim }, string_content(&code).to_string())
                } else if !self.source[ipn.inicio..].starts_with("${") && init_node.especie != "SimpleIdentifier" {
                    (s, format!("{{{code}}}"))
                } else {
                    (s, code.clone())
                }
            } else if self.needs_parens(initializer, n) {
                (s, format!("({code})"))
            } else {
                (s, code.clone())
            };
            m.add(uri, range, text);
        }
        m
    }

    /// `_shouldUseParenthesis(init, node)`.
    pub(crate) fn needs_parens(&self, init: usize, n: usize) -> bool {
        if let Some(p) = self.parent(n)
            && self.kind(p) == "SwitchExpression"
            && self.children(p).first() == Some(&n)
        {
            return false;
        }
        if self.precedence(init) < self.parent_precedence(n) {
            return true;
        }
        if self.kind(init) == "PrefixExpression"
            && let Some(p) = self.parent(n)
            && self.kind(p) == "PrefixExpression"
            && self.node_text(p).starts_with('-')
            && !self.node_text(p).starts_with("--")
        {
            let t = self.node_text(init);
            return t.starts_with('-');
        }
        false
    }

    /// `Expression.precedence` (`AN:dart/ast/precedence.dart`), em números
    /// crescentes de `none` (0) a `primary` (17).
    pub(crate) fn precedence(&self, n: usize) -> u8 {
        match self.kind(n) {
            "IntegerLiteral" | "DoubleLiteral" | "BooleanLiteral" | "NullLiteral" | "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings"
            | "SymbolLiteral" | "ListLiteral" | "SetOrMapLiteral" | "RecordLiteral" | "SimpleIdentifier" | "ParenthesizedExpression"
            | "InstanceCreationExpression" | "FunctionExpression" | "ThisExpression" | "SuperExpression" | "SwitchExpression" => 17,
            "MethodInvocation" | "PropertyAccess" | "PrefixedIdentifier" | "IndexExpression" | "PostfixExpression" | "FunctionExpressionInvocation"
            | "FunctionReference" | "DotShorthand" => 16,
            "PrefixExpression" | "AwaitExpression" => 15,
            "BinaryExpression" => self.binary_precedence(n),
            "AsExpression" | "IsExpression" => 8,
            "ConditionalExpression" => 3,
            "CascadeExpression" => 2,
            "AssignmentExpression" | "ThrowExpression" | "RethrowExpression" | "PatternAssignment" => 1,
            _ => 0,
        }
    }

    /// `Precedence.forTokenType` do operador da `BinaryExpression`.
    fn binary_precedence(&self, n: usize) -> u8 {
        let children = self.children(n);
        let (Some(&a), Some(&b)) = (children.first(), children.get(1)) else { return 0 };
        let between = &self.source[self.tree.nos[a].fim..self.tree.nos[b].inicio];
        let operator = dartforge_frontend::lexer::lex(between)
            .ok()
            .and_then(|v| {
                let ops: Vec<String> = v.iter().filter(|t| t.kind != Kind::Eof).map(|t| between[t.span.start..t.span.end].to_string()).collect();
                Some(ops.concat())
            })
            .unwrap_or_default();
        match operator.as_str() {
            "??" => 4,
            "||" => 5,
            "&&" => 6,
            "==" | "!=" => 7,
            "<" | ">" | "<=" | ">=" => 8,
            "|" => 9,
            "^" => 10,
            "&" => 11,
            "<<" | ">>" | ">>>" => 12,
            "+" | "-" => 13,
            "*" | "/" | "%" | "~/" => 14,
            _ => 0,
        }
    }

    /// `getExpressionParentPrecedence` (`util.dart:162-178`).
    pub(crate) fn parent_precedence(&self, n: usize) -> u8 {
        let Some(p) = self.parent(n) else { return 0 };
        match self.kind(p) {
            "ParenthesizedExpression" => 1,
            "IndexExpression" if self.children(p).get(1) == Some(&n) => 1,
            "AssignmentExpression" if self.children(p).get(1) == Some(&n) && self.parent(p).is_some_and(|g| self.kind(g) == "CascadeExpression") => 3,
            _ if self.is_expression(p) => self.precedence(p),
            _ => 0,
        }
    }
}

/// O nome no começo do texto de uma `VariableDeclaration`.
pub(crate) fn name_at_start_pub(t: &str) -> String {
    let end = t.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).unwrap_or(t.len());
    t[..end].to_string()
}

/// A string literal é crua (`r'…'`).
fn raw_string(code: &str) -> bool {
    code.starts_with('r') || code.starts_with('R')
}

/// A string literal usa aspas simples.
fn single_quotes(code: &str) -> bool {
    code.trim_start_matches(['r', 'R']).starts_with('\'')
}

/// A string literal é de três aspas.
fn multiline_string(code: &str) -> bool {
    let s = code.trim_start_matches(['r', 'R']);
    s.starts_with("'''") || s.starts_with("\"\"\"")
}

/// `contentsOffset…contentsEnd`: o texto sem as aspas.
fn string_content(code: &str) -> &str {
    let s = code.trim_start_matches(['r', 'R']);
    let n = if multiline_string(code) { 3 } else { 1 };
    if s.len() < 2 * n {
        return "";
    }
    &s[n..s.len() - n]
}

// -- Convert Getter to Method / Method to Getter -----------------------------------

/// As referências de `f` no projeto (sem as declarações), deduplicadas.
pub(crate) fn function_references(p: &Project, f: FunctionElementId, getter: bool) -> Vec<(UnitId, Span)> {
    let target_ = p.function_member(f);
    let declarations = p.declarations(&target_);
    let Ok(occurrences) = p.occurrences(&target_, false) else { return Vec::new() };
    let mut v = Vec::new();
    let mut seen: BTreeSet<(UnitId, usize, usize)> = BTreeSet::new();
    for (u, start, end) in occurrences {
        if declarations.contains(&(u, start, end)) || !seen.insert((u, start, end)) {
            continue;
        }
        let Ok(Some(d)) = p.identify(u, start) else { continue };
        if d.concrete != Some(Concrete::Function(f)) {
            continue;
        }
        // Um getter só é referido onde é lido: a escrita simples é do setter.
        if getter
            && let Some(x) = d.expr
            && p.program().unit(u).ast.exprs.iter().any(|e| matches!(&e.kind, ExprKind::Assign { op: ast::AssignOp::Assign, target, .. } if *target == x))
        {
            continue;
        }
        v.push((u, Span { start: start, end: end }));
    }
    v
}

/// A unidade e o `ast::FunctionId` da declaração de `f`.
fn function_declaration(p: &Project, f: FunctionElementId) -> Option<(UnitId, ast::FunctionId)> {
    match p.program().function(f).node {
        FunctionRef::Function { unit, function } => Some((unit, function)),
        _ => None,
    }
}

/// O elemento sob o cursor para os dois `Convert` (`getElementOfNode`).
fn element_to_convert(cx: &Context<'_>, o: usize) -> Elem {
    cx.element_at_cursor(o)
}

/// `ConvertGetterToMethodRefactoringImpl`.
fn getter_to_method(cx: &Context<'_>, request: &RefactoringRequest, command: &str) -> RefactoringResult {
    let p = cx.p;
    let e = element_to_convert(cx, request.offset);
    // `element is PropertyAccessorElement`: getter, setter ou acessor
    // implícito.
    let accessor = match e {
        Elem::Function(f) => matches!(p.program().function(f).kind, FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor),
        Elem::Variable => true,
        _ => false,
    };
    if !accessor {
        return R::InvalidArguments(format!("Location supplied to {command} RefactoringKind.CONVERT_GETTER_TO_METHOD is not longer valid"));
    }
    let initial = if !cx.in_workspace(e) {
        State::fatal("Only getters in your workspace can be converted.")
    } else if !matches!(e, Elem::Function(f) if p.program().function(f).kind == FunctionKind::Getter) {
        State::fatal("Only explicit getters can be converted to methods.")
    } else {
        State::default()
    };
    finish(request, initial, State::default, || {
        let Elem::Function(g) = e else { return Change::default() };
        let mut m = Change::default();
        let fe = p.program().function(g);
        let members: Vec<FunctionElementId> = if fe.class.is_none() && fe.extension.is_none() {
            vec![g]
        } else {
            match p.function_member(g) {
                Target::Member { owner, name, is_static } => match p.family(owner, &name, is_static, false) {
                    Ok(fam) => {
                        let mut v: Vec<FunctionElementId> =
                            fam.functions.into_iter().filter(|&x| p.program().function(x).kind == FunctionKind::Getter).collect();
                        v.sort_by_key(|x| x.0);
                        v
                    }
                    Err(_) => vec![g],
                },
                _ => vec![g],
            }
        };
        for f in members {
            if let Some((u, fid)) = function_declaration(p, f)
                && let Some(uri) = p.unit_uri(u)
            {
                let unit = p.program().unit(u);
                let function = unit.ast.function(fid);
                if let Some(name) = function.name {
                    // O `get` antes do nome.
                    let before = unit.source[..name.span.start].trim_end();
                    if before.ends_with("get") {
                        let get = before.len() - 3;
                        m.add(&uri, Span { start: get, end: name.span.start }, "");
                    }
                    m.add(&uri, Span { start: name.span.end, end: name.span.end }, "()");
                }
            }
            for (u, s) in function_references(p, f, true) {
                if let Some(uri) = p.unit_uri(u) {
                    m.add(&uri, Span { start: s.end, end: s.end }, "()");
                }
            }
        }
        m
    })
}

/// `ConvertMethodToGetterRefactoringImpl`.
fn method_to_getter(cx: &Context<'_>, request: &RefactoringRequest, command: &str) -> RefactoringResult {
    let p = cx.p;
    let e = element_to_convert(cx, request.offset);
    if !cx.executable(e) {
        return R::InvalidArguments(format!("Location supplied to {command} RefactoringKind.CONVERT_METHOD_TO_GETTER is not longer valid"));
    }
    let initial = match cx.convert_method_error(e) {
        Some(m) => State::fatal(m),
        None => State::default(),
    };
    finish(request, initial, State::default, || {
        let mut m = Change::default();
        let members: Vec<FunctionElementId> = match e {
            Elem::Function(f) if cx.is_function(e) => vec![f],
            Elem::Function(f) => match p.function_member(f) {
                Target::Member { owner, name, is_static } => match p.family(owner, &name, is_static, false) {
                    Ok(fam) => {
                        let mut v: Vec<FunctionElementId> = fam
                            .functions
                            .into_iter()
                            .filter(|&x| matches!(p.program().function(x).kind, FunctionKind::Function | FunctionKind::Operator))
                            .collect();
                        v.sort_by_key(|x| x.0);
                        v
                    }
                    Err(_) => vec![f],
                },
                _ => vec![f],
            },
            _ => Vec::new(),
        };
        let mut contexts: HashMap<UnitId, Context<'_>> = HashMap::new();
        for f in members {
            // A declaração: `get ` no nome e sem a lista de parâmetros.
            if let Some((u, fid)) = function_declaration(p, f)
                && let Some(uri) = p.unit_uri(u)
            {
                let unit = p.program().unit(u);
                let function = unit.ast.function(fid);
                if let (Some(name), Some(_)) = (function.name, &function.parameters) {
                    let cxu = contexts.entry(u).or_insert_with(|| Context::new(p, u));
                    let list = cxu
                        .tree
                        .nos
                        .iter()
                        .enumerate()
                        .find(|(_, k)| k.marca == crate::arvore_analyzer::Marca::Funcao(fid) && matches!(k.especie, "MethodDeclaration" | "FunctionDeclaration"))
                        .and_then(|(i, k)| {
                            let target = if k.especie == "FunctionDeclaration" {
                                cxu.children(i).iter().copied().find(|&c| cxu.kind(c) == "FunctionExpression").unwrap_or(i)
                            } else {
                                i
                            };
                            cxu.children(target).iter().copied().find(|&c| cxu.kind(c) == "FormalParameterList")
                        })
                        .map(|l| (cxu.tree.nos[l].inicio, cxu.tree.nos[l].fim));
                    m.add(&uri, Span { start: name.span.start, end: name.span.start }, "get ");
                    if let Some((li, lf)) = list {
                        m.add(&uri, Span { start: li, end: lf }, "");
                    }
                }
            }
            // As referências: sem os argumentos da invocação.
            for (u, s) in function_references(p, f, false) {
                let Some(uri) = p.unit_uri(u) else { continue };
                let cxu = contexts.entry(u).or_insert_with(|| Context::new(p, u));
                let Some(n) = cxu.tree.localizar(s.start, s.start) else { continue };
                if let Some(inv) = cxu.with_parents(n).find(|&k| cxu.kind(k) == "MethodInvocation") {
                    let end = cxu.tree.nos[inv].fim;
                    m.add(&uri, Span { start: s.end, end: end }, "");
                }
            }
        }
        m
    })
}

