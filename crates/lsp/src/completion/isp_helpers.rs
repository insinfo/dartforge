//! The smaller helpers of the analysis server 3.6.2 `InScopeCompletionPass`:
//! `IdentifierHelper` (`identifier_helper.dart`), `LabelHelper`
//! (`label_helper.dart`), `OverrideHelper` (`override_helper.dart`, with the
//! `writeOverride` of `change_builder_dart.dart:515-642`), `UriHelper`
//! (`uri_helper.dart`) and the closure, named argument and record named
//! field suggestions (`_addClosureSuggestion`, `NamedArgumentSuggestion`,
//! `_suggestRecordLiteralNamedFields`; `candidate_suggestion.dart`).
//! Written without compiling or running (2026-10-05).

use super::isp::Isp;
use super::isp_visits::InvokedParameter;
use super::*;
use dartforge_frontend::token::Op;

/// The `dart:` libraries that `_addDartSuggestions` offers: those of the
/// 3.6.2 SDK's `sdk_library_metadata/lib/libraries.dart` with categories
/// (not internal), without `implementation: true` and without `dart:_`, in
/// file order.
const LIBRARIES_DART: &[&str] = &[
    "dart:async",
    "dart:collection",
    "dart:concurrent",
    "dart:convert",
    "dart:core",
    "dart:developer",
    "dart:ffi",
    "dart:html",
    "dart:indexed_db",
    "dart:io",
    "dart:isolate",
    "dart:js",
    "dart:js_interop",
    "dart:js_interop_unsafe",
    "dart:js_util",
    "dart:math",
    "dart:mirrors",
    "dart:typed_data",
    "dart:cli",
    "dart:svg",
    "dart:web_audio",
    "dart:web_gl",
];

/// `getCamelWords` (`analyzer_plugin/src/utilities/string_utilities.dart`).
pub(super) fn camel_words(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let b: Vec<u16> = s.encode_utf16().collect();
    let lowercase = |c: u16| (b'a' as u16..=b'z' as u16).contains(&c);
    let uppercase = |c: u16| (b'A' as u16..=b'Z' as u16).contains(&c);
    let mut parts = Vec::new();
    let (mut was_lower, mut was_upper) = (false, false);
    let mut start = 0usize;
    for i in 0..b.len() {
        let c = b[i];
        let (new_min, new_upper) = (lowercase(c), uppercase(c));
        if was_lower && new_upper {
            parts.push(String::from_utf16_lossy(&b[start..i]));
            start = i;
        }
        if was_upper && new_upper && i + 1 < b.len() && lowercase(b[i + 1]) {
            parts.push(String::from_utf16_lossy(&b[start..i]));
            start = i;
        }
        was_lower = new_min;
        was_upper = new_upper;
    }
    parts.push(String::from_utf16_lossy(&b[start..]));
    parts
}

/// `getCamelWordCombinations`.
pub(super) fn camel_combinations(name: &str) -> Vec<String> {
    let parts = camel_words(name);
    (0..parts.len()).map(|i| format!("{}{}", parts[i].to_lowercase(), parts[i + 1..].concat())).collect()
}

/// `String.toUpperCamelCase` (`utilities/extensions/string.dart:59-77`).
fn camel_upper(s: &str) -> Option<String> {
    let capitalize = |w: &str| {
        let mut cs = w.chars();
        match cs.next() {
            None => String::new(),
            Some(c) if w.chars().count() <= 1 => c.to_uppercase().collect(),
            Some(c) => format!("{}{}", c.to_uppercase(), cs.as_str().to_lowercase()),
        }
    };
    let words: Vec<&str> = s.split('_').collect();
    if words.len() < 2 {
        let first = words.first()?;
        if first.is_empty() {
            return None;
        }
        return Some(capitalize(first));
    }
    let mut r = String::new();
    for w in words {
        if w.is_empty() {
            return None;
        }
        r.push_str(&capitalize(w));
    }
    Some(r)
}

/// A lint rule enabled in the `analysis_options.yaml` that applies to the
/// file (`codeStyleOptions`).
fn enabled_rule(path: Option<&std::path::Path>, name: &str) -> bool {
    path.is_some_and(|c| crate::refactorings_exec::rule_enabled_in(c, name))
}

impl<'a, 'c> Isp<'a, 'c> {
    // -- IdentifierHelper ------------------------------------------------------------

    /// `identifierHelper(includePrivateIdentifiers: …)`: the first call fixes
    /// the option.
    pub(super) fn ih(&mut self, private: bool) {
        if self.private.is_none() {
            self.private = Some(private);
        }
    }

    /// `_createNameSuggestion` / `IdentifierSuggestion` (`suggestName`, 500).
    pub(super) fn item_name(&mut self, name: &str, body: bool) {
        if name.is_empty() {
            return;
        }
        let Some(s) = self.match_score(name) else { return };
        let completion = if body { format!("{name} {{}}") } else { name.to_string() };
        let selection = name.encode_utf16().count() + if body { 2 } else { 0 };
        let before = self.collector.items.len();
        let end = completion.encode_utf16().count();
        let item = self.collector.push(group::LOCAL, kind::VARIABLE, completion.clone(), completion, None);
        item.rel.fixed = Some(500);
        if selection != end {
            item.selection = Some((selection, 0));
        }
        self.mark(before, s, false);
    }

    /// `addSuggestionsFromTypeName`.
    pub(super) fn type_name_ih(&mut self, type_name: &str) {
        let mut names = camel_combinations(type_name);
        if let Some(i) = names.iter().position(|n| n == type_name) {
            names.remove(i);
        }
        let private = self.private.unwrap_or(false);
        for n in names {
            self.item_name(&n, false);
            if private {
                self.item_name(&format!("_{n}"), false);
            }
        }
    }

    /// `addVariable(type)`: only a `NamedType`.
    pub(super) fn ih_variable(&mut self, ty: Option<usize>) {
        if let Some(t) = ty
            && self.kind(t) == "NamedType"
        {
            let name = self.named_type_name(t);
            self.type_name_ih(&name);
        }
    }

    /// `addTopLevelName`: the file name in `UpperCamelCase`, if the library does
    /// not declare that name yet.
    pub(super) fn top_name_ih(&mut self, body: bool) {
        let Some(path) = self.path.clone() else { return };
        let Some(base) = path.file_stem().and_then(|s| s.to_str()) else { return };
        let Some(candidate) = camel_upper(base) else { return };
        let lib = self.query.program.library(self.library());
        if lib.declared.keys().any(|s| self.query.name(*s) == candidate) {
            return;
        }
        self.item_name(&candidate, body);
    }

    // -- LabelHelper ----------------------------------------------------------------

    /// `addLabels`: the labels visible at the `break`/`continue`.
    pub(super) fn labels(&mut self, command: usize) {
        let with_cases = self.kind(command) == "ContinueStatement";
        let mut current = Some(command);
        while let Some(k) = current {
            match self.kind(k) {
                "SwitchStatement" => {
                    if with_cases {
                        for m in self.children_of(k, &["SwitchCase", "SwitchDefault", "SwitchPatternCase"]) {
                            let labels = self.children_of(m, &["Label"]);
                            self.visit_labels(&labels);
                        }
                    }
                }
                "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody" => return,
                "LabeledStatement" => {
                    let labels = self.children_of(k, &["Label"]);
                    self.visit_labels(&labels);
                }
                _ => {}
            }
            current = self.parent(k);
        }
    }

    /// `_visitLabels` / `suggestLabel` (`Relevance.label`, 1000).
    pub(super) fn visit_labels(&mut self, labels: &[usize]) {
        for &r in labels {
            let name = self.source[self.start(r)..self.end(r)].trim_end_matches(':').trim().to_string();
            let Some(s) = self.match_score(&name) else { continue };
            if name.is_empty() || name == "_" {
                continue;
            }
            let before = self.collector.items.len();
            // `ElementKind.LABEL` → `CompletionItemKind.Text`.
            let item = self.collector.push(group::LOCAL, 1, name.clone(), name, None);
            item.rel.fixed = Some(1000);
            self.mark(before, s, false);
        }
    }

    // -- OverrideHelper -------------------------------------------------------------

    /// `interface.isSuperImplemented(name)`: there is a concrete implementation
    /// in the superclass chain (with the mixins).
    pub(super) fn implemented_super(&self, c: ClassId, name: SymbolId) -> bool {
        let p = &self.query.program;
        let mut current = p.class(p.dono_da_classe(c)).supertype_class;
        let mut seen = HashSet::new();
        while let Some(x) = current {
            if !seen.insert(x) {
                break;
            }
            let cl = p.class(x);
            let concrete = |k: ClassId| p.class(k).instance_members.get(&name).is_some_and(|&f| !p.function(f).abstract_);
            if concrete(x) || cl.mixin_classes.iter().any(|&m| concrete(m)) {
                return true;
            }
            current = cl.supertype_class;
        }
        false
    }

    /// `containingNode.hasOverride`: the member containing the point already
    /// has `@override`.
    pub(super) fn member_has_override(&self) -> bool {
        let Some(c) = self.covered_node() else { return false };
        let Some(m) = self.ancestor(c, &["MethodDeclaration", "FieldDeclaration"]) else { return false };
        self.children_of(m, &["Annotation"]).iter().any(|&a| self.source[self.start(a)..self.end(a)].trim() == "@override")
    }

    /// `computeOverridesFor(interfaceElement, SourceRange(offset, 0), skipAt)`.
    pub(super) fn overrides(&mut self, c: ClassId, without_at_sign: bool) {
        let p = &self.query.program;
        let owning = p.dono_da_classe(c);
        let lib_c = p.class(owning).library;
        let declared: HashSet<SymbolId> = p.class(owning).instance_members.keys().copied().collect();
        let members = self.interface_members(c, false, false);
        let already_has_override = self.member_has_override();
        for f in members {
            let fe = self.query.program.function(f);
            let symbol = fe.name;
            if declared.contains(&symbol) {
                continue;
            }
            let raw_name = self.query.name(symbol).to_string();
            if raw_name.starts_with('_') && fe.library != lib_c {
                continue;
            }
            if self.member_annotations(f).iter().any(|a| a == "nonVirtual") {
                continue;
            }
            let display = raw_name.trim_end_matches('=').to_string();
            let s = self.score("override").max(self.score("operator")).max(self.score(&display));
            if s == -1.0 {
                continue;
            }
            let invoke_super = self.implemented_super(c, symbol);
            self.item_override(c, f, invoke_super, without_at_sign, already_has_override, s);
        }
    }

    /// `suggestOverride`: the text of `writeOverride`, the selection and the label.
    pub(super) fn item_override(&mut self, c: ClassId, f: FunctionElementId, invoke_super: bool, without_at_sign: bool, already_has_override: bool, score: f64) {
        let fe = self.query.program.function(f).clone();
        let raw_name = self.query.name(fe.name).to_string();
        let name = raw_name.trim_end_matches('=').to_string();
        let setter = fe.kind == FunctionKind::Setter || (fe.kind == FunctionKind::ImplicitAccessor && raw_name.ends_with('='));
        let getter = !setter && matches!(fe.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor);
        let operator = fe.kind == FunctionKind::Operator;
        // The member as the class sees it (the supertypes' type parameters
        // substituted).
        let this = self.type_this(c);
        let lib = self.library();
        let search = if setter { self.query.names.lookup(&name) } else { Some(fe.name) };
        let ty = search
            .and_then(|s| self.query.resolver().lookup_member(this, s, setter, lib))
            .map(|(_, t)| t)
            .unwrap_or_else(|| super::declared_type(self.query, f));
        let (return_, positional_, optional_, named_, type_parameters) = match self.query.table.get(ty).clone() {
            Type::Function { ret, positional, optional, named, type_params, .. } if !getter && !setter => (ret, positional.to_vec(), optional.to_vec(), named.to_vec(), type_params.to_vec()),
            _ if setter => (self.query.core.void_, vec![ty], Vec::new(), Vec::new(), Vec::new()),
            _ => (ty, Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        };
        let parameters_ast = self.member_parameters(f);
        let mut writer = crate::write_type::Writer::from_query(&*self.query, lib, Some(c), None);
        writer.copied = Some(f);
        let mut s = String::from("@override\n  ");
        let mut display = String::new();
        let selection: (usize, usize);
        if getter {
            s.push_str(&format!("// TODO: implement {name}\n  "));
        }
        if !setter && let Some(t) = writer.write_type(Some(return_), false) {
            s.push_str(&t);
            s.push(' ');
        }
        if getter {
            s.push_str("get ");
        } else if setter {
            s.push_str("set ");
        } else if operator {
            s.push_str("operator ");
        }
        s.push_str(&name);
        display.push_str(&name);
        if getter {
            s.push_str(" => ");
            let body = if invoke_super { format!("super.{name}") } else { "throw UnimplementedError()".to_string() };
            selection = (s.len(), body.len());
            s.push_str(&body);
            s.push_str(if invoke_super { ";\n" } else { ";" });
            display.push_str(" => …");
        } else {
            // `writeTypeParameters` + `writeParameters`.
            let mut head = String::new();
            if !type_parameters.is_empty() {
                head.push('<');
                for (i, tp) in type_parameters.iter().enumerate() {
                    if i > 0 {
                        head.push_str(", ");
                    }
                    let data = self.query.table.param(*tp);
                    head.push_str(self.query.name(data.name));
                    if data.explicito
                        && let Some(b) = writer.write(data.bound, false)
                    {
                        head.push_str(" extends ");
                        head.push_str(&b);
                    }
                }
                head.push('>');
            }
            let parameter_types: Vec<TypeId> = positional_.iter().chain(optional_.iter()).copied().collect();
            let mut used_names: HashSet<String> = parameters_ast.iter().filter_map(|x| x.name.clone()).collect();
            head.push('(');
            let (mut saw_named, mut saw_optional) = (false, false);
            let mut call_names: Vec<(bool, String)> = Vec::new();
            for (i, prm) in parameters_ast.iter().enumerate() {
                if i > 0 {
                    head.push_str(", ");
                }
                if prm.named && !saw_named {
                    head.push('{');
                    saw_named = true;
                } else if prm.optional && !saw_optional {
                    head.push('[');
                    saw_optional = true;
                }
                let name_p = match &prm.name {
                    Some(n) if !n.is_empty() => n.clone(),
                    _ => {
                        let mut k = 1;
                        while used_names.contains(&format!("p{k}")) {
                            k += 1;
                        }
                        let n = format!("p{k}");
                        used_names.insert(n.clone());
                        n
                    }
                };
                let type_p = if prm.named {
                    named_.iter().find(|(s, _, _)| self.query.name(*s) == name_p).map(|(_, t, _)| *t)
                } else {
                    parameter_types.get(prm.position).copied()
                };
                if prm.covariant {
                    head.push_str("covariant ");
                }
                if prm.required_named {
                    head.push_str("required ");
                }
                match writer.write_type(type_p, false) {
                    Some(t) => {
                        head.push_str(&t);
                        head.push(' ');
                        head.push_str(&name_p);
                    }
                    None => head.push_str(&name_p),
                }
                if let Some(d) = &prm.pattern {
                    head.push_str(" = ");
                    head.push_str(d);
                }
                call_names.push((prm.named, name_p));
            }
            if saw_named {
                head.push('}');
            }
            if saw_optional {
                head.push(']');
            }
            head.push(')');
            s.push_str(&head);
            display.push_str(&head);
            s.push_str(" {\n    ");
            s.push_str(&format!("// TODO: implement {name}"));
            let invocation = {
                let mut x = String::from(if operator { " " } else { "." });
                x.push_str(&name);
                x.push_str(if operator { " " } else { "(" });
                for (i, (named2, n)) in call_names.iter().enumerate() {
                    if i > 0 {
                        x.push_str(", ");
                    }
                    if *named2 {
                        x.push_str(n);
                        x.push_str(": ");
                    }
                    x.push_str(n);
                }
                x.push_str(if operator { ";" } else { ");" });
                x
            };
            let is_void = matches!(self.query.table.get(return_), Type::Void);
            if setter {
                if invoke_super {
                    s.push_str("\n    ");
                    let first = call_names.first().map(|(_, n)| n.clone()).unwrap_or_default();
                    let body = format!("super.{name} = {first};");
                    selection = (s.len(), body.len());
                    s.push_str(&body);
                } else {
                    selection = (s.len(), 0);
                }
            } else if is_void {
                if invoke_super {
                    s.push_str("\n    ");
                    let body = format!("super{invocation}");
                    selection = (s.len(), body.len());
                    s.push_str(&body);
                } else {
                    selection = (s.len(), 0);
                }
            } else {
                s.push_str("\n    ");
                let body = if invoke_super { format!("return super{invocation}") } else { "throw UnimplementedError();".to_string() };
                selection = (s.len(), body.len());
                s.push_str(&body);
            }
            s.push_str("\n  }");
            display.push_str(" { … }");
        }
        let import: Vec<LibraryId> = writer.import.iter().copied().collect();
        drop(writer);
        // `completion = replacement.trim()`, without the `@override` the member
        // already has, or without the already typed `@`.
        let mut completion = s.trim().to_string();
        let mut start = s.find(&completion).unwrap_or(0);
        if already_has_override && completion.starts_with("@override") {
            let rest = completion["@override".len()..].trim().to_string();
            start = s.find(&rest).unwrap_or(start);
            completion = rest;
        }
        if without_at_sign && completion.starts_with("@override") {
            completion = completion[1..].to_string();
            start += 1;
        }
        if completion.is_empty() || display.is_empty() {
            return;
        }
        if without_at_sign {
            display = format!("override {display}");
        }
        let utf16 = |x: &str| x.encode_utf16().count();
        let sel_start = selection.0.saturating_sub(start);
        let selection_utf16 = (utf16(&s[start..start + sel_start.min(s.len() - start)]), utf16(&s[selection.0..(selection.0 + selection.1).min(s.len())]));
        let lsp_kind = if getter || setter { kind::PROPERTY } else { kind::METHOD };
        let before = self.collector.items.len();
        let origin = self.query.origin(self.query.function_start(f));
        let item = self.collector.push(group::MEMBER, lsp_kind, completion.clone(), completion, None);
        item.display = Some(display);
        item.selection = Some(selection_utf16);
        item.rel.fixed = Some(750);
        item.origin = origin;
        item.match_ = Some(if operator { "override_operator".to_string() } else { format!("override_{name}") });
        if let Some(&l) = import.first() {
            let uri = self.query.program.library(l).uri.clone();
            if uri.starts_with("dart:") || uri.starts_with("package:") {
                let (span, new) = crate::actions::insert_import(self.source, self.cu, &uri);
                item.import = Some(AutoImport { uri, span, text: new });
            }
        }
        self.mark(before, score, false);
    }

    /// The declared parameters of a member, in order (name, kind, `required`,
    /// `covariant`, the text of the default value).
    pub(super) fn member_parameters(&self, f: FunctionElementId) -> Vec<WrittenParameter> {
        let p = &self.query.program;
        let fe = p.function(f);
        match fe.node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                let u = p.unit(unit);
                let func = u.ast.function(function);
                let mut position = 0usize;
                func.parameters
                    .iter()
                    .flatten()
                    .map(|x| {
                        let named = x.kind == ast::ParameterKind::Named;
                        let pos = position;
                        if !named {
                            position += 1;
                        }
                        WrittenParameter {
                            name: x.nome_externo().map(|n| u.source[n.span.start..n.span.end].to_string()),
                            named,
                            optional: x.kind == ast::ParameterKind::Optional,
                            required_named: named && x.required,
                            covariant: x.covariant,
                            pattern: x.default_value.map(|e| {
                                let s = u.ast.expr(e).span;
                                u.source[s.start..s.end].to_string()
                            }),
                            position: pos,
                        }
                    })
                    .collect()
            }
            _ => {
                // The implicit setter of a field: `_x`.
                if fe.kind == FunctionKind::ImplicitAccessor && self.query.name(fe.name).ends_with('=') {
                    let base = self.query.name(fe.name).trim_end_matches('=').to_string();
                    let covariant = fe.variable.is_some_and(|v| match p.variable(v).node {
                        dartforge_elements::model::VariableRef::Field { unit, member, .. } => {
                            p.unit(unit).ast.member(member).span.start < p.unit(unit).source.len()
                                && p.unit(unit).source[p.unit(unit).ast.member(member).span.start..p.unit(unit).ast.member(member).span.end].contains("covariant ")
                        }
                        _ => false,
                    });
                    vec![WrittenParameter { name: Some(format!("_{base}")), named: false, optional: false, required_named: false, covariant, pattern: None, position: 0 }]
                } else {
                    Vec::new()
                }
            }
        }
    }

    // -- UriHelper ------------------------------------------------------------------

    /// `addSuggestions(uri)`: the node is the directive's `SimpleStringLiteral`.
    pub(super) fn uris(&mut self, n: usize) {
        let (start, end) = (self.start(n), self.end(n));
        let size = self.source.len();
        let last_is_quote = || {
            let c = self.source[..end].chars().last();
            matches!(c, Some('"') | Some('\''))
        };
        let ok = if self.offset > start {
            if self.offset < end {
                true
            } else if self.offset == end {
                end == start + 1 || (end == size && !last_is_quote())
            } else {
                false
            }
        } else {
            self.offset == start && self.offset == end && end == size && last_is_quote()
        };
        if ok {
            self.uri_literal(n);
        }
    }

    /// `_simpleStringLiteral`.
    pub(super) fn uri_literal(&mut self, n: usize) {
        let Some(parent) = self.parent(n) else { return };
        let Some(partial) = self.partial_uri(n) else { return };
        match self.kind(parent) {
            "Configuration" | "ImportDirective" | "ExportDirective" => {
                self.uris_dart();
                self.package_uris(&partial);
                self.file_uris(&partial);
            }
            "PartDirective" | "PartOfDirective" => self.file_uris(&partial),
            _ => {}
        }
    }

    /// The literal's `contentsOffset` (after the `r` and the quotes).
    pub(super) fn content_start(&self, n: usize) -> usize {
        let t = &self.source[self.start(n)..self.end(n)];
        let mut k = 0usize;
        if t.starts_with('r') {
            k += 1;
        }
        let rest = &t[k..];
        if rest.starts_with("'''") || rest.starts_with("\"\"\"") {
            k += 3;
        } else if rest.starts_with('\'') || rest.starts_with('"') {
            k += 1;
        }
        self.start(n) + k
    }

    /// `_extractPartialUri`.
    pub(super) fn partial_uri(&self, n: usize) -> Option<String> {
        let c = self.content_start(n);
        if self.offset < c {
            return None;
        }
        Some(self.source[c..self.offset.min(self.source.len())].to_string())
    }

    /// `_suggestUri` (`Relevance.import` 900; `dart:core` 100).
    pub(super) fn item_uri(&mut self, uri: &str) {
        let Some(s) = self.match_score(uri) else { return };
        let lsp_kind = if uri.starts_with("dart:") {
            kind::MODULE
        } else if uri.ends_with(".dart") {
            17
        } else {
            19
        };
        let before = self.collector.items.len();
        let item = self.collector.push(group::LIBRARY, lsp_kind, uri.to_string(), uri.to_string(), None);
        item.rel.fixed = Some(if uri == "dart:core" { 100 } else { 900 });
        self.mark(before, s, false);
    }

    /// `_addDartSuggestions`.
    pub(super) fn uris_dart(&mut self) {
        self.item_uri("dart:");
        for u in LIBRARIES_DART {
            self.item_uri(u);
        }
    }

    /// `_addPackageSuggestions`.
    pub(super) fn package_uris(&mut self, partial: &str) {
        let Some(file) = self.path.clone() else { return };
        let Some(config) = dartforge_elements::config::PackageConfig::discover(&file).and_then(|c| dartforge_elements::config::PackageConfig::load(&c).ok()) else {
            return;
        };
        self.item_uri("package:");
        for (name, folder) in config.package_dirs.iter() {
            let prefix = format!("package:{name}/");
            self.item_uri(&prefix);
            if folder.is_dir() {
                self.package_folder(partial, &prefix, folder);
            }
        }
    }

    /// `_addPackageFolderSuggestions`.
    pub(super) fn package_folder(&mut self, partial: &str, prefix: &str, folder: &std::path::Path) {
        let Ok(entries) = std::fs::read_dir(folder) else { return };
        let mut children: Vec<std::path::PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        children.sort();
        for f in children {
            let Some(name) = f.file_name().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            if f.is_dir() {
                let child = format!("{prefix}{name}/");
                self.item_uri(&child);
                if partial.starts_with(&child) {
                    self.package_folder(partial, &child, &f);
                }
            } else if name.ends_with(".dart") {
                self.item_uri(&format!("{prefix}{name}"));
            }
        }
    }

    /// `_addFileSuggestions`.
    pub(super) fn file_uris(&mut self, partial: &str) {
        let Some(file) = self.path.clone() else { return };
        let parent_uri = if partial.ends_with('/') {
            partial.to_string()
        } else {
            let d = match partial.rfind('/') {
                Some(0) => "/".to_string(),
                Some(i) => partial[..i].to_string(),
                None => ".".to_string(),
            };
            if d != "." && !d.ends_with('/') { format!("{d}/") } else { d }
        };
        let prefix_uri = if parent_uri == "." { String::new() } else { parent_uri.clone() };
        let scheme = parent_uri.split_once(':').map(|(e, _)| e).filter(|e| !e.contains('/') && e.len() > 1);
        if !parent_uri.starts_with("file://") && scheme.is_some() {
            return;
        }
        let dir_path = if let Some(r) = parent_uri.strip_prefix("file://") { std::path::PathBuf::from(r) } else { std::path::PathBuf::from(&parent_uri) };
        let source_dir = file.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
        let mut dir = dir_path.clone();
        if dir.is_relative() {
            dir = source_dir.join(&dir_path);
            let normalize = |p: &std::path::Path| {
                let mut v: Vec<std::path::Component> = Vec::new();
                for c in p.components() {
                    match c {
                        std::path::Component::CurDir => {}
                        std::path::Component::ParentDir => {
                            v.pop();
                        }
                        x => v.push(x),
                    }
                }
                v.iter().collect::<std::path::PathBuf>()
            };
            dir = normalize(&dir);
            let in_lib = |p: &std::path::Path| p.components().any(|c| c.as_os_str() == "lib");
            if in_lib(&source_dir) && !in_lib(&dir) {
                return;
            }
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        let mut children: Vec<std::path::PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        children.sort();
        let own = file.file_name().and_then(|s| s.to_str()).map(str::to_string);
        for f in children {
            let Some(name) = f.file_name().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            let completion = if f.is_dir() {
                (!name.starts_with('.')).then(|| format!("{prefix_uri}{name}/"))
            } else if name.ends_with(".dart") {
                Some(format!("{prefix_uri}{name}"))
            } else {
                None
            };
            if let Some(c) = completion
                && Some(&c) != own.as_ref()
            {
                self.item_uri(&c);
            }
        }
    }

    // -- Closures, named arguments and record fields -----------------------------------

    /// `_addClosureSuggestion` without the positional names (generated `p0`…).
    pub(super) fn suggest_closure(&mut self, ty: TypeId, comma: bool) {
        self.suggest_closure_with_names(ty, &[], comma);
    }

    /// `_addClosureSuggestion`: the block one and the arrow one, both with
    /// `matcherScore` 0 (`Relevance.closure`, 900). `names` are those of the
    /// positionals of the written function type (DartForge's type does not
    /// keep them).
    pub(super) fn suggest_closure_with_names(&mut self, ty: TypeId, names: &[String], comma: bool) {
        let Type::Function { positional, optional, named, .. } = self.query.table.get(ty).clone() else { return };
        let include_types = enabled_rule(self.path.as_deref(), "always_specify_types");
        let indent = self.line_indent();
        let mut parameters: Vec<(String, TypeId, u8, bool)> = Vec::new();
        for (i, &t) in positional.iter().enumerate() {
            parameters.push((names.get(i).cloned().unwrap_or_default(), t, 0, false));
        }
        for (i, &t) in optional.iter().enumerate() {
            parameters.push((names.get(positional.len() + i).cloned().unwrap_or_default(), t, 1, false));
        }
        // `FunctionType.parameters`: the named ones in lexical order.
        let mut named2: Vec<(String, TypeId, bool)> = named.iter().map(|(n, t, r)| (self.query.name(*n).to_string(), *t, *r)).collect();
        named2.sort_by(|a, b| a.0.cmp(&b.0));
        for (n, t, r) in named2 {
            parameters.push((n, t, 2, r));
        }
        let build = |isp: &Self, types: bool, words: bool| -> String {
            let existing: HashSet<String> = parameters.iter().map(|p| p.0.clone()).collect();
            let mut b = String::from("(");
            let (mut named_, mut optional_) = (false, false);
            for (i, (name, t, k, req)) in parameters.iter().enumerate() {
                if i != 0 {
                    b.push_str(", ");
                }
                if *k == 2 && !named_ {
                    named_ = true;
                    b.push('{');
                } else if *k == 1 && !optional_ {
                    optional_ = true;
                    b.push('[');
                }
                if types {
                    b.push_str(&isp.query.format(*t));
                    b.push(' ');
                }
                let mut n = name.clone();
                if n.is_empty() {
                    n = format!("p{i}");
                    let mut index = 1;
                    while existing.contains(&n) {
                        n = format!("p{i}_{index}");
                        index += 1;
                    }
                }
                if words && *req {
                    b.push_str("required ");
                }
                b.push_str(&n);
            }
            if named_ {
                b.push('}');
            } else if optional_ {
                b.push(']');
            }
            b.push(')');
            b
        };
        let complete = build(self, include_types, true);
        let short = build(self, false, false);
        for block in [true, false] {
            let mut text = complete.clone();
            let display;
            let selection;
            if block {
                display = format!("{short} {{}}");
                text.push_str(" {\n");
                text.push_str(&indent);
                text.push_str("  ");
                selection = text.encode_utf16().count();
                text.push('\n');
                text.push_str(&indent);
                text.push('}');
            } else {
                display = format!("{short} =>");
                text.push_str(" => ");
                selection = text.encode_utf16().count();
            }
            if comma {
                text.push(',');
            }
            let before = self.collector.items.len();
            let item = self.collector.push(group::NAMED, kind::METHOD, display.clone(), text, None);
            item.display = Some(display);
            item.selection = Some((selection, 0));
            item.rel.fixed = Some(900);
            self.mark(before, 0.0, false);
        }
    }

    /// `NamedArgumentSuggestion` (`Relevance.requiredNamedArgument` 950 /
    /// `namedArgument` 900): `name: ` (+ `[]` for a widget-list parameter, +
    /// `,`); `replace == Some(0)` is `replacementLength: 0`.
    pub(super) fn suggest_named_argument(&mut self, p: &InvokedParameter, colon: bool, comma: bool, replace: Option<usize>) {
        let Some(s) = self.match_score(&p.name) else { return };
        let mut completion = p.name.clone();
        if colon {
            completion.push_str(": ");
        }
        let mut selection = completion.encode_utf16().count();
        if p.widget && colon && self.is_list(p.ty) {
            let n = completion.encode_utf16().count();
            completion.push_str("[]");
            selection = n + 1;
        }
        if comma {
            completion.push(',');
        }
        let detail = self.query.format(p.ty);
        let before = self.collector.items.len();
        let end = completion.encode_utf16().count();
        let item = self.collector.push(group::NAMED, kind::VARIABLE, completion.clone(), completion.clone(), Some(detail));
        item.display = Some(completion);
        if selection != end {
            item.selection = Some((selection, 0));
        }
        item.rel.fixed = Some(if p.required || p.annotated_required { 950 } else { 900 });
        item.replace_empty = replace == Some(0);
        self.mark(before, s, false);
    }

    /// `type.isDartCoreList`.
    pub(super) fn is_list(&self, t: TypeId) -> bool {
        matches!(self.query.table.get(t), Type::Interface { class, .. } if Some(*class) == self.query.core.list_class)
    }

    /// `_suggestRecordLiteralNamedFields`: `container` is the visited node,
    /// `literal` the `RecordLiteral` (or none), `new` whether it is a new field
    /// (`name: `, with the comma when the displaced token is neither `,` nor
    /// `)`).
    pub(super) fn suggest_record_named_fields(&mut self, container: usize, literal: Option<usize>, new: bool) {
        let target = literal.unwrap_or(container);
        let context = self.parent(target).and_then(|p| self.context_type_in(p, self.start(target)));
        let Some(t) = context else { return };
        let Type::Record { named, .. } = self.query.table.get(t).clone() else { return };
        // `_computeDisplacedToken`: the first token of the node at `offset` or after.
        let shifted = self.tok_after(self.offset).filter(|&i| self.span_tok(i).start < self.end(container).max(self.offset + 1));
        let Some(d) = shifted else { return };
        let included: HashSet<String> = match literal {
            Some(l) => self
                .children_of(l, &["NamedExpression"])
                .iter()
                .filter_map(|&e| self.child(e, "Label"))
                .map(|r| self.source[self.start(r)..self.end(r)].trim_end_matches(':').trim().to_string())
                .collect(),
            None => HashSet::new(),
        };
        let comma = !self.is_op(d, Op::Comma) && !self.is_op(d, Op::RParen);
        for (n, ty) in named.iter() {
            let name = self.query.name(*n).to_string();
            if included.contains(&name) {
                continue;
            }
            let Some(s) = self.match_score(&name) else { continue };
            let mut completion = name.clone();
            if new {
                completion.push_str(": ");
            }
            let selection = completion.encode_utf16().count();
            if new && comma {
                completion.push(',');
            }
            let detail = self.query.format(*ty);
            let before = self.collector.items.len();
            let end = completion.encode_utf16().count();
            let item = self.collector.push(group::NAMED, kind::VARIABLE, completion.clone(), completion.clone(), Some(detail));
            item.display = Some(completion);
            if selection != end {
                item.selection = Some((selection, 0));
            }
            item.rel.fixed = Some(950);
            self.mark(before, s, false);
        }
    }
}

/// A declared parameter, for the override's `writeParameters`.
#[derive(Debug, Clone)]
pub(super) struct WrittenParameter {
    pub name: Option<String>,
    pub named: bool,
    pub optional: bool,
    pub required_named: bool,
    pub covariant: bool,
    pub pattern: Option<String>,
    /// The position among the positionals (for the substituted type).
    pub position: usize,
}
