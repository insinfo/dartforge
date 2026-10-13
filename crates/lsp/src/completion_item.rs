//! The Dart 3.6.2 server's `CompletionItem` (`toCompletionItem`,
//! `MAP:974-1179`; docs/LSP-ESPECIFICACAO.md §14.8): label, `filterText`,
//! `kind` by preference and the client's `valueSet`, `detail` and
//! `labelDetails`, the inserted text with the call snippet
//! (`_buildInsertText`), `InsertReplaceEdit`/`TextEdit`, `itemDefaults`,
//! documentation on the item and the `data` of not-imported items; and the
//! code snippets (§14.8.9).
//! Written without compiling or running (2026-10-05).

use crate::completion::{Call, CompletionItem};
use serde_json::{Value, json};

/// What the client announced for completion (`client_capabilities.dart:155-202`).
#[derive(Debug, Clone, Default)]
pub(crate) struct Capabilities {
    pub deprecated: bool,
    pub deprecated_tag: bool,
    /// `documentationFormat` (`None`: the client did not send the field).
    pub doc_formats: Option<Vec<String>>,
    pub snippet: bool,
    pub insert_replace: bool,
    pub insertion_modes: Vec<u64>,
    pub label_details: bool,
    pub kinds: Option<Vec<u64>>,
    /// `completionList.itemDefaults` contains `editRange` / `insertTextMode`.
    pub default_range: bool,
    pub default_mode: bool,
}

impl Capabilities {
    pub(crate) fn of_initialize(c: &Value) -> Capabilities {
        let item = c.pointer("/textDocument/completion/completionItem");
        let b = |p: &str| item.and_then(|i| i.pointer(p)).and_then(Value::as_bool).unwrap_or(false);
        let patterns: Vec<String> = c
            .pointer("/textDocument/completion/completionList/itemDefaults")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        Capabilities {
            deprecated: b("/deprecatedSupport"),
            deprecated_tag: item.and_then(|i| i.pointer("/tagSupport/valueSet")).and_then(Value::as_array).is_some_and(|l| l.iter().any(|x| x.as_u64() == Some(1))),
            doc_formats: item.and_then(|i| i.get("documentationFormat")).and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()),
            snippet: b("/snippetSupport"),
            insert_replace: b("/insertReplaceSupport"),
            insertion_modes: item.and_then(|i| i.pointer("/insertTextModeSupport/valueSet")).and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_u64).collect()).unwrap_or_default(),
            label_details: b("/labelDetailsSupport"),
            kinds: c.pointer("/textDocument/completion/completionItemKind/valueSet").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_u64).collect()),
            default_range: patterns.iter().any(|p| p == "editRange"),
            default_mode: patterns.iter().any(|p| p == "insertTextMode"),
        }
    }

    /// The first `kind` of the preference that the client accepts (without a
    /// `valueSet`, the LSP default set: 1 to 18).
    fn kind(&self, preference: &[u64]) -> Option<u64> {
        preference.iter().copied().find(|k| match &self.kinds {
            Some(l) => l.contains(k),
            None => (1..=18).contains(k),
        })
    }

    /// The documentation in the client's format (`MAP:1619-1634`, `:79-85`).
    fn documentation(&self, text: String) -> Value {
        match &self.doc_formats {
            None => json!(text),
            Some(l) => {
                let markdown = l.is_empty() || l.iter().any(|f| f == "markdown") || !l.iter().any(|f| f == "plaintext");
                json!({"kind": if markdown { "markdown" } else { "plaintext" }, "value": text})
            }
        }
    }
}

/// The context of a completion request.
pub(crate) struct Request<'a> {
    /// `replacementRange` and `insert` (already in LSP positions).
    pub replace: Value,
    pub insert: Value,
    pub equal: bool,
    /// The current `itemDefaults.editRange`.
    pub pattern: bool,
    /// The effective `completeFunctionCalls` (configuration, and no argument
    /// list at the target).
    pub calls: bool,
    /// The path of the file being edited (`data.file`).
    pub file: &'a str,
    /// The `documentation` preference (`none`/`summary`/`full`).
    pub doc_preference: &'a str,
}

/// `escapeSnippetPlainText`.
fn escape(t: &str) -> String {
    t.replace('\\', "\\\\").replace('$', "\\$")
}

/// `buildSnippetStringWithTabStops(text, pairs)`.
fn snippet_with_stops(text: &str, pairs: &[(usize, usize)]) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let valid: Vec<(usize, usize)> = pairs.iter().copied().filter(|&(o, l)| o + l <= units.len()).collect();
    let mut s = String::new();
    let mut pos = 0usize;
    let single = valid.len() == 1;
    for (i, &(o, l)) in valid.iter().enumerate() {
        s.push_str(&escape(&String::from_utf16_lossy(&units[pos..o])));
        let n = if single { 0 } else { i + 1 };
        let inside = String::from_utf16_lossy(&units[o..o + l]);
        if inside.is_empty() {
            s.push_str(&format!("${n}"));
        } else {
            let e = inside.replace('\\', "\\\\").replace('$', "\\$").replace('}', "\\}");
            s.push_str(&format!("${{{n}:{e}}}"));
        }
        pos = o + l;
    }
    s.push_str(&escape(&String::from_utf16_lossy(&units[pos..])));
    s
}

/// `computeCompletionDefaultArgumentList`: the text and the pairs (UTF-16).
fn list_pattern(parameters: &[String]) -> Option<(String, Vec<(usize, usize)>)> {
    if parameters.is_empty() {
        return None;
    }
    let mut text = String::new();
    let mut pairs = Vec::new();
    for p in parameters {
        if !text.is_empty() {
            text.push_str(", ");
        }
        match p.strip_suffix(": ") {
            Some(n) => {
                text.push_str(n);
                text.push_str(": ");
                let o = text.encode_utf16().count();
                text.push_str(n);
                pairs.push((o, n.encode_utf16().count()));
            }
            None => {
                let o = text.encode_utf16().count();
                text.push_str(p);
                pairs.push((o, p.encode_utf16().count()));
            }
        }
    }
    Some((text, pairs))
}

/// `getCompletionDetail`: `truncatedParams` and `truncatedSignature` of a
/// `detail` already in the form `(params) → ret` (or just the type).
fn signatures(detail: Option<&str>, callable: bool) -> (String, String) {
    let Some(d) = detail.filter(|d| !d.is_empty()) else { return (String::new(), String::new()) };
    if callable && d.starts_with('(') {
        let mut level = 0usize;
        let mut end = d.len();
        for (i, c) in d.char_indices() {
            match c {
                '(' | '<' | '[' | '{' => level += 1,
                ')' | '>' | ']' | '}' => {
                    level = level.saturating_sub(1);
                    if level == 0 && c == ')' {
                        end = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let truncated = if &d[..end] == "()" { "()".to_string() } else { "(…)".to_string() };
        let return_ = d[end..].trim_start().strip_prefix('→').map(str::trim).filter(|r| !r.is_empty());
        let signature = match return_ {
            Some(r) => format!("{truncated} → {r}"),
            None => truncated.clone(),
        };
        return (truncated, signature);
    }
    (String::new(), format!(" {d}"))
}

/// `docSummary`: the first paragraph.
fn summary(doc: &str) -> String {
    let mut lines = Vec::new();
    for l in doc.lines() {
        if l.trim().is_empty() {
            break;
        }
        lines.push(l);
    }
    lines.join("\n")
}

/// The kind preference of an item (`elementKindToCompletionItemKind` and
/// `suggestionKindToCompletionItemKind`).
fn preference(k: u32) -> Vec<u64> {
    match k {
        20 => vec![20, 13],
        25 => vec![25, 6],
        17 => vec![17, 9],
        19 => vec![19, 9],
        0 => Vec::new(),
        k => vec![u64::from(k)],
    }
}

/// The `CompletionItem` of a server item.
pub(crate) fn item(cap: &Capabilities, p: &Request<'_>, i: &CompletionItem, documentation: Option<String>) -> Value {
    let callable = i.call.is_some();
    // `label` (the `displayText ?? completion`), `filterText`.
    let mut label = i.display.clone().unwrap_or_else(|| i.insert.clone());
    let starts_with_pattern = label.starts_with("=>") || label.starts_with('(');
    let filter = if starts_with_pattern {
        label.clone()
    } else {
        label.split(|c| c == '(').next().unwrap_or("").split("=>").next().unwrap_or("").trim().to_string()
    };
    if cap.label_details {
        label = filter.clone();
    }
    if label.ends_with(',') {
        label.pop();
    }
    let (truncated, signature) = signatures(i.detail.as_deref(), callable);
    // The short parameters only without `displayText` (`MAP:1069-1073`).
    if !cap.label_details && i.display.is_none() {
        label.push_str(&truncated);
    }
    // `_buildInsertText`.
    let mut text = i.insert.clone();
    let mut snippet = false;
    // `isInvocation`: an `IDENTIFIER` (tear-off, redirection) does not call.
    let mut calls = p.calls && callable && !i.identifier && !i.insert.contains('(');
    if i.insert.contains('(') {
        calls = false;
    }
    if cap.snippet && calls {
        snippet = true;
        let suffix = match &i.call {
            Some(Call::Parameters(ps)) => match list_pattern(ps) {
                Some((t, pairs)) => snippet_with_stops(&t, &pairs),
                None => String::new(),
            },
            Some(Call::Unknown) => "$0".to_string(),
            None => String::new(),
        };
        text = format!("{}({suffix})", escape(&i.insert));
    } else if cap.snippet
        && let Some((o, l)) = i.selection
        && o != 0
        && o != i.insert.encode_utf16().count()
    {
        // A selection before the end becomes a stop (`buildSnippetStringWithTabStops`).
        snippet = true;
        text = snippet_with_stops(&i.insert, &[(o, l)]);
    }
    let mut v = json!({"label": label});
    if let Some(k) = cap.kind(&preference(i.kind)) {
        v["kind"] = json!(k);
    }
    // `detail`: the full signature.
    if let Some(d) = i.detail.as_ref().filter(|d| !d.is_empty()) {
        v["detail"] = json!(d);
    }
    if cap.label_details {
        let mut m = serde_json::Map::new();
        if !signature.is_empty() {
            m.insert("detail".into(), json!(signature));
        }
        if let Some(imp) = &i.import {
            m.insert("description".into(), json!(display_uri(&imp.uri, p.file)));
        }
        if !m.is_empty() {
            v["labelDetails"] = Value::Object(m);
        }
    }
    // Documentation: only without `data` (not-imported items resolve later).
    if i.import.is_none()
        && let Some(doc) = documentation
    {
        let doc = match p.doc_preference {
            "none" => None,
            "summary" => Some(summary(&doc)),
            _ => Some(doc),
        };
        if let Some(d) = doc.filter(|d| !d.is_empty()) {
            v["documentation"] = cap.documentation(d);
        }
    }
    v["sortText"] = json!(i.sort_text);
    if filter != v["label"].as_str().unwrap_or("") {
        v["filterText"] = json!(filter);
    }
    if snippet {
        v["insertTextFormat"] = json!(2);
    }
    if !cap.default_mode && cap.insertion_modes.contains(&1) && text.contains('\n') {
        v["insertTextMode"] = json!(1);
    }
    if p.pattern {
        if text != v["label"].as_str().unwrap_or("") {
            v["textEditText"] = json!(text);
        }
    } else if i.replace_empty {
        // `replacementLength: 0`: inserts at the cursor without replacing anything.
        v["textEdit"] = json!({"range": p.insert, "newText": text});
    } else if cap.insert_replace && !p.equal {
        v["textEdit"] = json!({"insert": p.insert, "replace": p.replace, "newText": text});
    } else {
        v["textEdit"] = json!({"range": p.replace, "newText": text});
    }
    if let Some(imp) = &i.import {
        let name = i.insert.trim_end_matches([':', ' ']);
        v["data"] = json!({"file": p.file, "importUris": [imp.uri], "ref": format!("{};{};{}", imp.uri, imp.uri, name)});
    }
    v
}

/// `getCompletionDisplayUriString`: `file:` relative to the file's folder.
pub(crate) fn display_uri(uri: &str, file: &str) -> String {
    if let Ok(u) = url::Url::parse(uri)
        && u.scheme() == "file"
        && let Ok(target) = u.to_file_path()
        && let Some(folder) = std::path::Path::new(file).parent()
    {
        return crate::refactorings_method::relative_path(&target, folder);
    }
    if !uri.contains(':') {
        return uri.to_string();
    }
    uri.to_string()
}

/// The snippet context (`DartSnippetRequest._getContext`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnippetContext {
    Top,
    Block,
    Class,
    Expression,
    Other,
}

/// The code snippets (§14.8.9) of the context, filtered by the prefix:
/// (prefix, label, documentation, body).
pub(crate) fn snippets(context: SnippetContext, indent: &str, eol: &str, in_tests: bool, finals: bool) -> Vec<(&'static str, &'static str, &'static str, String)> {
    let q = |s: &str| s.replace("⏎", &format!("{eol}{indent}"));
    let var = if finals { "final" } else { "var" };
    let mut v: Vec<(&'static str, &'static str, &'static str, String)> = Vec::new();
    match context {
        SnippetContext::Top => {
            v.push(("class", "class", "Insert a class definition.", q("class ${1:ClassName} {⏎  $0⏎}")));
            v.push(("fun", "fun", "Insert a function definition.", q("${1:void} ${2:name}(${3:params}) {⏎  $0⏎}")));
            let main = if in_tests { "void main() {\n  $0\n}".to_string() } else { "void main(List<String> args) {\n  $0\n}".to_string() };
            v.push(("main", "main()", "Insert a main function, used as an entry point.", main));
        }
        SnippetContext::Block => {
            v.push(("do", "do while", "Insert a do-while loop.", q("do {⏎  $0⏎} while (${1:condition});")));
            v.push(("forin", "for in", "Insert a for-in loop.", q(&format!("for ({var} ${{1:element}} in ${{2:collection}}) {{⏎  $0⏎}}"))));
            v.push(("for", "for", "Insert a for loop.", q("for (var i = 0; i < ${1:count}; i++) {⏎  $0⏎}")));
            v.push(("fun", "fun", "Insert a function definition.", q("${1:void} ${2:name}(${3:params}) {⏎  $0⏎}")));
            v.push(("ife", "ife", "Insert an if/else statement.", q("if (${1:condition}) {⏎  $0⏎} else {⏎  ⏎}")));
            v.push(("if", "if", "Insert an if statement.", q("if (${1:condition}) {⏎  $0⏎}")));
            v.push(("switch", "switch statement", "Insert a switch statement.", q("switch (${1:expression}) {⏎  case ${2:value}:⏎    $0⏎    break;⏎  default:⏎}")));
            if in_tests {
                v.push(("test", "test", "Insert a test block.", q("test('${1:test name}', () {⏎  $0⏎});")));
                v.push(("group", "group", "Insert a test group block.", q("group('${1:group name}', () {⏎  $0⏎});")));
            }
            v.push(("try", "try", "Insert a try/catch statement.", q("try {⏎  $0⏎} catch (${1:e}) {⏎  ⏎}")));
            v.push(("while", "while", "Insert a while loop.", q("while (${1:condition}) {⏎  $0⏎}")));
        }
        SnippetContext::Class => {
            v.push(("fun", "fun", "Insert a function definition.", q("${1:void} ${2:name}(${3:params}) {⏎  $0⏎}")));
        }
        SnippetContext::Expression => {
            v.push(("switch", "switch expression", "Insert a switch expression.", q("switch (${1:expression}) {⏎  ${2:pattern} => ${3:value},$0⏎}")));
        }
        SnippetContext::Other => {}
    }
    v
}

/// The item of a snippet (`snippetToCompletionItem`, `MAP:788-878`).
pub(crate) fn snippet_item(cap: &Capabilities, p: &Request<'_>, prefix: &str, label: &str, doc: &str, body: &str) -> Value {
    let mut v = json!({"label": label, "kind": 15, "sortText": format!("zzz{prefix}"), "insertTextFormat": 2});
    if prefix != label {
        v["filterText"] = json!(prefix);
    }
    v["documentation"] = cap.documentation(doc.to_string());
    if cap.insertion_modes.contains(&1) {
        v["insertTextMode"] = json!(1);
    }
    if p.pattern {
        v["textEditText"] = json!(body);
    } else {
        v["textEdit"] = json!({"insertTextFormat": 2, "range": p.replace, "newText": body});
    }
    v
}

/// The completion target (`CompletionTarget.forOffset`,
/// `completion_target.dart:141-260`) over the syntax tree: the containing
/// node and the entity (node or token).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Entity {
    No(usize),
    Token(usize),
    None,
}

/// The snippet context of the text (`DartSnippetRequest._getContext`);
/// `None` inside a comment.
pub(crate) fn snippet_context(text: &str, offset: usize) -> Option<SnippetContext> {
    use dartforge_frontend::token::Kind;
    // Comment: `inComment`.
    let comments = dartforge_frontend::comentarios::Comentarios::de(text);
    for c in comments.todos() {
        let line = text[c.start..c.end].starts_with("//");
        if c.start < offset && (offset < c.end || (line && offset == c.end)) {
            return Some(SnippetContext::Other);
        }
    }
    let mut names = dartforge_intern::Interner::new();
    let analyzed = dartforge_frontend::parser::parse(text, &mut names);
    let tree = crate::arvore_analyzer::construir(text, &analyzed.ast, &analyzed.unit, false);
    let tokens: Vec<dartforge_frontend::token::Token> = dartforge_frontend::lexer::lex(text).unwrap_or_default().into_iter().filter(|t| t.kind != Kind::Eof).collect();
    let word = |t: &dartforge_frontend::token::Token| matches!(t.kind, Kind::Ident | Kind::Keyword(_));
    let candidate = |t: &dartforge_frontend::token::Token| offset < t.span.end || (offset == t.span.end && (word(t) || t.span.start == t.span.end));
    let (mut cont, mut entity) = (0usize, Entity::None);
    'external: loop {
        let node = &tree.nos[cont];
        // The entities: the children and the tokens outside them, in order.
        let mut ents: Vec<(usize, Entity)> = node.filhos.iter().map(|&f| (tree.nos[f].inicio, Entity::No(f))).collect();
        let (start, end) = if cont == 0 { (0, text.len()) } else { (node.inicio, node.fim) };
        for (k, t) in tokens.iter().enumerate() {
            if t.span.start < start || t.span.end > end {
                continue;
            }
            if node.filhos.iter().any(|&f| tree.nos[f].inicio <= t.span.start && t.span.end <= tree.nos[f].fim) {
                continue;
            }
            ents.push((t.span.start, Entity::Token(k)));
        }
        ents.sort_by_key(|(o, _)| *o);
        for (_, e) in ents {
            match e {
                Entity::Token(k) => {
                    if candidate(&tokens[k]) {
                        entity = Entity::Token(k);
                        break 'external;
                    }
                }
                Entity::No(f) => {
                    let nf = &tree.nos[f];
                    let last = tokens.iter().rev().find(|t| t.span.end <= nf.fim && t.span.start >= nf.inicio);
                    if !last.is_some_and(candidate) {
                        continue;
                    }
                    let first = tokens.iter().find(|t| t.span.start >= nf.inicio);
                    let is_candidate = match first {
                        Some(b) if word(b) => candidate(b),
                        _ => offset <= nf.inicio,
                    };
                    if is_candidate {
                        entity = Entity::No(f);
                        break 'external;
                    }
                    cont = f;
                    continue 'external;
                }
                Entity::None => {}
            }
        }
        break;
    }
    match entity {
        Entity::Token(k) if matches!(tokens[k].kind, Kind::Str(_) | Kind::StrBegin(..) | Kind::StrMid(..) | Kind::StrEnd(_)) => return Some(SnippetContext::Other),
        Entity::No(f) if tree.nos[f].especie == "NamedExpression" => {
            let name = tree.nos[f].filhos.first().map(|&l| (tree.nos[l].inicio, tree.nos[l].fim));
            if name.is_some_and(|(a, b)| a <= offset && offset <= b) {
                return Some(SnippetContext::Other);
            }
        }
        _ => {}
    }
    let mut current = Some(cont);
    while let Some(k) = current {
        if k == 0 {
            break;
        }
        let e = tree.nos[k].especie;
        match e {
            "ReturnStatement" if offset > tree.nos[k].inicio + "return".len() => return Some(SnippetContext::Expression),
            "Comment" | "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings" => return Some(SnippetContext::Other),
            "VariableDeclaration" => return Some(SnippetContext::Expression),
            "VariableDeclarationList" => return Some(SnippetContext::Other),
            "PropertyAccess" | "FieldFormalParameter" | "PrefixedIdentifier" | "ConstructorReference" | "InstanceCreationExpression" => return Some(SnippetContext::Other),
            "Block" => return Some(SnippetContext::Block),
            _ if crate::refactorings::is_command_kind(e) => return Some(SnippetContext::Other),
            "SwitchExpression" => return Some(SnippetContext::Other),
            _ if crate::refactorings::is_expression_kind(e) => return Some(SnippetContext::Expression),
            "Annotation" => return Some(SnippetContext::Other),
            "BlockFunctionBody" => return Some(SnippetContext::Block),
            "ClassDeclaration" | "ExtensionDeclaration" | "MixinDeclaration" | "EnumDeclaration" => return Some(SnippetContext::Class),
            _ => {}
        }
        current = tree.nos[k].pai;
    }
    Some(SnippetContext::Top)
}
