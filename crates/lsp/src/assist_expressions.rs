//! Assistências sobre expressões, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to 'isNotEmpty'` | `refactor.convert.isNotEmpty` | `ConvertIntoIsNotEmpty` |
//! | `Convert to an int literal` | `refactor.convert.toIntLiteral` | `ConvertToIntLiteral` |
//! | `Convert to a spread`, `Inline invocation of 'addAll'` | `refactor.convert.toSpread`, `refactor.inline` | `ConvertAddAllToSpread` |
//! | `Inline invocation of 'add'` | `refactor.inline` | `InlineInvocation` |
//! | `Convert to an 'if' element` | `refactor.convert.toIfElement` | `ConvertConditionalExpressionToIfElement` |
//! | `Convert to map literal` | `refactor.convert.toMapLiteral` | `ConvertToMapLiteral` |
//! | `Convert to set literal` | `refactor.convert.toSetLiteral` | `ConvertToSetLiteral` |
//! | `Convert to a 'for' element` | `refactor.convert.toForElement` | `ConvertMapFromIterableToForLiteral` |

use crate::actions::CodeAction;
use crate::refactorings::Context;
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::FunctionElementId;
use dartforge_types::{MemberRef, Resolved};
use std::collections::HashSet;

impl Context<'_> {
    pub(crate) fn simple_action(&self, uri: &str, title: &str, kind: &str, edits: Vec<(Span, String)>) -> CodeAction {
        CodeAction {
            title: title.into(),
            kind: kind.into(),
            edits: edits.into_iter().map(|(span, text)| Edit { uri: uri.to_string(), span, text }).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        }
    }

    /// `ConvertIntoIsNotEmpty` (convert_into_is_not_empty.dart): `!x.isEmpty`
    /// vira `x.isNotEmpty` quando quem declara o `isEmpty` resolvido declara
    /// também um `isNotEmpty` (`getChildren`).
    pub(crate) fn convert_to_is_not_empty(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        if self.kind(no) != "SimpleIdentifier" {
            return None;
        }
        let access = self.parent(no)?;
        if !matches!(self.kind(access), "PropertyAccess" | "PrefixedIdentifier") {
            return None;
        }
        let identifier = *self.children(access).last()?;
        // O elemento do `isEmpty` (o getter ou o campo).
        let x = self.node_expr(access)?;
        let prog = self.p.program();
        let name_and = |f: FunctionElementId| self.p.name(prog.function(f).name).trim_end_matches('=').to_string();
        let (name, owner_class, owner_extension) = match self.bodies.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
                let fe = prog.function(*f);
                (name_and(*f), fe.class, fe.extension)
            }
            Resolved::Member { member: MemberRef::Variable(v), .. } => {
                let ve = prog.variable(*v);
                (self.p.name(ve.name).to_string(), ve.class, ve.extension)
            }
            _ => return None,
        };
        if name != "isEmpty" {
            return None;
        }
        let has_is_not_empty = |instance: &std::collections::HashMap<dartforge_intern::SymbolId, FunctionElementId>,
                                statics: &std::collections::HashMap<dartforge_intern::SymbolId, FunctionElementId>,
                                fields: &[dartforge_elements::model::VariableId]| {
            instance.keys().chain(statics.keys()).any(|s| self.p.name(*s).trim_end_matches('=') == "isNotEmpty")
                || fields.iter().any(|v| self.p.name(prog.variable(*v).name) == "isNotEmpty")
        };
        let tem = match (owner_class, owner_extension) {
            (Some(c), _) => {
                let ce = prog.class(c);
                has_is_not_empty(&ce.instance_members, &ce.static_members, &ce.fields)
            }
            (None, Some(e)) => {
                let ee = prog.extension(e);
                has_is_not_empty(&ee.instance_members, &ee.static_members, &ee.fields)
            }
            _ => false,
        };
        if !tem {
            return None;
        }
        let prefix = self.parent(access)?;
        if self.kind(prefix) != "PrefixExpression" {
            return None;
        }
        let operator = self.next_token(self.tree.nos[prefix].inicio)?;
        if &self.source[operator.start..operator.end] != "!" {
            return None;
        }
        Some(self.simple_action(
            uri,
            "Convert to 'isNotEmpty'",
            "refactor.convert.isNotEmpty",
            vec![
                // `range.startStart(prefixExpression, prefixExpression.operand)`.
                (Span { start: self.tree.nos[prefix].inicio, end: self.tree.nos[access].inicio }, String::new()),
                (self.tree.span(identifier), "isNotEmpty".into()),
            ],
        ))
    }

    /// `ConvertToIntLiteral` (convert_to_int_literal.dart): um literal
    /// `double` de valor inteiro vira `int` (cortado no `.`, que preserva os
    /// separadores de dígitos, quando não há expoente).
    pub(crate) fn convert_to_int_literal(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        if self.kind(no) != "DoubleLiteral" {
            return None;
        }
        let span = self.tree.span(no);
        let lexeme = &self.source[span.start..span.end];
        let value: f64 = lexeme.replace('_', "").parse().ok()?;
        // `value.truncate()` lança em infinito e NaN; fora da faixa do `int`
        // o resultado não é igual ao valor.
        if !value.is_finite() || value.trunc() != value || value.abs() >= 9.223_372_036_854_775_808e18 {
            return None;
        }
        let integer = value as i64;
        let edit = match lexeme.find('.') {
            Some(dot) if dot > 0 && !lexeme.to_lowercase().contains('e') => (Span { start: span.start + dot, end: span.end }, String::new()),
            _ => (span, integer.to_string()),
        };
        Some(self.simple_action(uri, "Convert to an int literal", "refactor.convert.toIntLiteral", vec![edit]))
    }

    /// Os argumentos de uma `ArgumentList`.
    pub(crate) fn list_arguments(&self, invocation: usize) -> Vec<usize> {
        self.children(invocation)
            .iter()
            .copied()
            .find(|&k| self.kind(k) == "ArgumentList")
            .map(|l| self.children(l).to_vec())
            .unwrap_or_default()
    }

    /// Os elementos de um `ListLiteral` (sem os argumentos de tipo).
    fn list_elements(&self, list: usize) -> Vec<usize> {
        self.children(list).iter().copied().filter(|&k| self.kind(k) != "TypeArgumentList").collect()
    }

    /// `ConvertAddAllToSpread` (convert_add_all_to_spread.dart): no nome do
    /// primeiro `..addAll(x)` em cascata sobre um literal de lista, `x` entra
    /// na lista como `...x` (ou `...?a` de `a ?? []`, `if (c) ...a` de
    /// `c ? a : []`); com um literal de lista, os elementos dele
    /// (`Inline invocation of 'addAll'`).
    pub(crate) fn convert_add_all_to_spread(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let name = self.tree.localizar(start, end)?;
        if self.kind(name) != "SimpleIdentifier" {
            return None;
        }
        let invocation = self.parent(name)?;
        if self.kind(invocation) != "MethodInvocation" || self.method_name(invocation) != Some(name) || self.node_text(name) != "addAll" {
            return None;
        }
        // `isCascaded`: o operador do nome é `..` ou `?..`.
        let operator = self.previous_token(self.tree.nos[name].inicio)?;
        if !matches!(&self.source[operator.start..operator.end], ".." | "?..") {
            return None;
        }
        let arguments = self.list_arguments(invocation);
        if arguments.len() != 1 {
            return None;
        }
        let argument = arguments[0];
        let in_line = self.kind(argument) == "ListLiteral";
        let cascade = self.with_parents(invocation).find(|&k| self.kind(k) == "CascadeExpression")?;
        let fc = self.children(cascade);
        let (target, first) = (*fc.first()?, *fc.get(1)?);
        if self.kind(target) != "ListLiteral" || first != invocation {
            return None;
        }
        let empty_list = |k: usize| self.kind(k) == "ListLiteral" && self.list_elements(k).is_empty();
        let mut text: Option<String> = None;
        match self.kind(argument) {
            "BinaryExpression" if self.binary_operator(argument) == Some("??") => {
                let f = self.children(argument);
                if empty_list(f[1]) {
                    text = Some(format!("...?{}", self.node_text(f[0])));
                }
            }
            "ConditionalExpression" => {
                let f = self.children(argument);
                if empty_list(f[2]) {
                    text = Some(format!("if ({}) ...{}", self.node_text(f[0]), self.node_text(f[1])));
                }
            }
            "ListLiteral" => {
                let elements = self.list_elements(argument);
                let (&first_, &last) = (elements.first()?, elements.last()?);
                text = Some(self.source[self.tree.nos[first_].inicio..self.tree.nos[last].fim].to_string());
            }
            _ => {}
        }
        let text = text.unwrap_or_else(|| format!("...{}", self.node_text(argument)));
        let target_elements = self.list_elements(target);
        let insertion = match target_elements.last() {
            Some(&u) => (Span { start: self.tree.nos[u].fim, end: self.tree.nos[u].fim }, format!(", {text}")),
            None => {
                let opens = self.tokens.iter().find(|t| t.span.start >= self.tree.nos[target].inicio && &self.source[t.span.start..t.span.end] == "[")?.span;
                (Span { start: opens.end, end: opens.end }, text)
            }
        };
        let (title, kind) = if in_line {
            ("Inline invocation of 'addAll'", "refactor.inline")
        } else {
            ("Convert to a spread", "refactor.convert.toSpread")
        };
        Some(self.simple_action(uri, title, kind, vec![insertion, (self.tree.span(invocation), String::new())]))
    }

    /// `InlineInvocation` (inline_invocation.dart): no nome do primeiro
    /// `..add(e)` de uma cascata sobre um literal de lista, `e` entra na lista.
    pub(crate) fn inline_invocation_add(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let name = self.tree.localizar(start, end)?;
        if self.kind(name) != "SimpleIdentifier" || self.node_text(name) != "add" {
            return None;
        }
        let invocation = self.parent(name)?;
        if self.kind(invocation) != "MethodInvocation" || self.method_name(invocation) != Some(name) {
            return None;
        }
        let operator = self.previous_token(self.tree.nos[name].inicio)?;
        if !matches!(&self.source[operator.start..operator.end], ".." | "?..") {
            return None;
        }
        let arguments = self.list_arguments(invocation);
        if arguments.len() != 1 {
            return None;
        }
        let cascade = self.parent(invocation)?;
        if self.kind(cascade) != "CascadeExpression" {
            return None;
        }
        let fc = self.children(cascade);
        let (target, first) = (*fc.first()?, *fc.get(1)?);
        if self.kind(target) != "ListLiteral" || first != invocation {
            return None;
        }
        let text = self.node_text(arguments[0]).to_string();
        let insertion = match self.list_elements(target).last() {
            Some(&u) => (Span { start: self.tree.nos[u].fim, end: self.tree.nos[u].fim }, format!(", {text}")),
            None => {
                let opens = self.tokens.iter().find(|t| t.span.start >= self.tree.nos[target].inicio && &self.source[t.span.start..t.span.end] == "[")?.span;
                (Span { start: opens.end, end: opens.end }, text)
            }
        };
        Some(self.simple_action(uri, "Inline invocation of 'add'", "refactor.inline", vec![insertion, (self.tree.span(invocation), String::new())]))
    }

    /// `ConvertConditionalExpressionToIfElement`: a condicional elemento de um
    /// literal de lista ou de conjunto (através de parênteses) vira
    /// `if (c) a else b`.
    pub(crate) fn conditional_to_if_element(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let conditional = self.with_parents(no).find(|&k| self.kind(k) == "ConditionalExpression")?;
        let mut swap = conditional;
        let mut parent = self.parent(conditional)?;
        while self.kind(parent) == "ParenthesizedExpression" {
            swap = parent;
            parent = self.parent(parent)?;
        }
        let is_set = || {
            // `SetOrMapLiteral.isSet`: o tipo estático é um `Set`.
            let Some(x) = self.node_expr(parent) else { return false };
            let Some(t) = self.bodies.get_type(x) else { return false };
            matches!(self.p.query.table.get(t), dartforge_types::Type::Interface { class, .. } if Some(*class) == self.p.query.core.set_class)
        };
        if !(self.kind(parent) == "ListLiteral" || (self.kind(parent) == "SetOrMapLiteral" && is_set())) {
            return None;
        }
        let f = self.children(conditional);
        let (c, a, b) = (self.without_parens(*f.first()?), self.without_parens(*f.get(1)?), self.without_parens(*f.get(2)?));
        let text = format!("if ({}) {} else {}", self.node_text(c), self.node_text(a), self.node_text(b));
        Some(self.simple_action(uri, "Convert to an 'if' element", "refactor.convert.toIfElement", vec![(self.tree.span(swap), text)]))
    }

    /// Os tokens de um `Comment` de documentação: cada linha `///` ou o
    /// `/** … */` inteiro, com o tipo (`true`: de uma linha).
    fn comment_tokens(&self, comment: usize) -> Vec<(Span, bool)> {
        let span = self.tree.span(comment);
        let text = &self.source[span.start..span.end];
        let mut v = Vec::new();
        let mut i = 0;
        while i < text.len() {
            let rest = &text[i..];
            if rest.starts_with("/*") {
                let end = rest.find("*/").map_or(rest.len(), |k| k + 2);
                v.push((Span { start: span.start + i, end: span.start + i + end }, false));
                i += end;
            } else if rest.starts_with("//") {
                let end = rest.find(['\r', '\n']).unwrap_or(rest.len());
                v.push((Span { start: span.start + i, end: span.start + i + end }, true));
                i += end;
            } else {
                i += rest.chars().next().map_or(1, char::len_utf8);
            }
        }
        v
    }

    /// `ConvertDocumentationIntoBlock`: o comentário de documentação feito
    /// só de linhas `///` vira `/** … */`.
    pub(crate) fn block_documentation(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let comment = self.with_parents(no).find(|&k| self.kind(k) == "Comment")?;
        let tokens = self.comment_tokens(comment);
        if tokens.is_empty() || tokens.iter().any(|&(s, line)| !line || !self.source[s.start..s.end].starts_with("///")) {
            return None;
        }
        let prefix = self.node_prefix(comment);
        let eol = crate::refactorings_exec::Text::new(self.source).eol();
        let mut s = format!("/**{eol}");
        for (t, _) in &tokens {
            s.push_str(&format!("{prefix} *{}{eol}", &self.source[t.start + 3..t.end]));
        }
        s.push_str(&format!("{prefix} */"));
        Some(self.simple_action(uri, "Convert to block documentation comment", "refactor.convert.blockComment", vec![(self.tree.span(comment), s)]))
    }

    /// `ConvertDocumentationIntoLine`: o `/** … */` vira linhas `///`.
    pub(crate) fn line_documentation(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let comment = self.with_parents(no).find(|&k| self.kind(k) == "Comment")?;
        let tokens = self.comment_tokens(comment);
        let [(token, false)] = tokens[..] else { return None };
        let eol = crate::refactorings_exec::Text::new(self.source).eol();
        let prefix = self.node_prefix(comment);
        let mut new: Vec<String> = Vec::new();
        let mut first = true;
        let mut line_prefix = String::new();
        for line in self.source[token.start..token.end].split(eol) {
            if first {
                first = false;
                let mut l = line.strip_prefix("/**")?.trim();
                if let Some(x) = l.strip_suffix("*/") {
                    l = x.trim();
                }
                if !l.is_empty() {
                    new.push(format!("/// {l}"));
                    line_prefix = format!("{eol}{prefix}");
                }
            } else {
                let l = line.trim_start();
                if l.starts_with("*/") {
                    break;
                }
                let mut l = l.strip_prefix('*')?;
                if let Some(x) = l.strip_suffix("*/") {
                    l = x.trim_end();
                }
                new.push(format!("{line_prefix}///{l}"));
                line_prefix = format!("{eol}{prefix}");
            }
        }
        Some(self.simple_action(uri, "Convert to line documentation comment", "refactor.convert.lineComment", vec![(self.tree.span(comment), new.concat())]))
    }

    /// `ConvertToMapLiteral` (convert_to_map_literal.dart): `Map()` ou
    /// `LinkedHashMap()` sem argumentos vira `{}`, com os argumentos de tipo
    /// escritos ou, fora de declaração tipada, os do tipo estático.
    pub(crate) fn convert_to_map_literal(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let creation = self.with_parents(no).find(|&k| self.kind(k) == "InstanceCreationExpression")?;
        let arguments = self.children(creation).iter().copied().find(|&k| self.kind(k) == "ArgumentList")?;
        let constructor_name = self.children(creation).iter().copied().find(|&k| self.kind(k) == "ConstructorName")?;
        if self.tree.nos[no].inicio > self.tree.nos[arguments].inicio || self.children(constructor_name).len() > 1 || !self.children(arguments).is_empty() {
            return None;
        }
        let ty = self.node_expr(creation).and_then(|x| self.bodies.get_type(x))?;
        let query = &self.p.query;
        let prog = self.p.program();
        let dartforge_types::Type::Interface { class, args, .. } = query.table.get(ty) else { return None };
        let is_map = Some(*class) == query.core.map_class
            || (self.p.name(prog.class(*class).name) == "LinkedHashMap" && prog.library(prog.class(*class).library).uri == "dart:collection");
        if !is_map {
            return None;
        }
        let constructor_type = *self.children(constructor_name).first()?;
        let written_arguments = self.children(constructor_type).iter().copied().find(|&k| self.kind(k) == "TypeArgumentList");
        let mut text = String::new();
        let mut import = std::collections::BTreeSet::new();
        match written_arguments {
            Some(a) => text.push_str(self.node_text(a)),
            None => {
                let list = self.with_parents(creation).find(|&k| self.kind(k) == "VariableDeclarationList");
                let typed_list = list.is_some_and(|l| {
                    self.children(l).iter().any(|&f| matches!(self.kind(f), "NamedType" | "GenericFunctionType" | "RecordTypeAnnotation"))
                });
                let all_dynamic = args.first().is_some_and(|&a| matches!(query.table.get(a), dartforge_types::Type::Dynamic))
                    && args.last().is_some_and(|&a| matches!(query.table.get(a), dartforge_types::Type::Dynamic));
                if !typed_list && !args.is_empty() && !all_dynamic {
                    // `writeTypes`: cada um pelo `writeType`, com ", ".
                    let mut writer = crate::write_type::Writer::new(self, self.tree.nos[creation].inicio);
                    let written: Vec<String> = args.iter().map(|&a| writer.write_type(Some(a), false).unwrap_or_default()).collect();
                    import = writer.import;
                    text.push('<');
                    text.push_str(&written.join(", "));
                    text.push('>');
                }
            }
        }
        text.push_str("{}");
        let mut m = crate::refactorings_exec::Change::default();
        m.add(uri, self.tree.span(creation), text);
        crate::refactorings_move::builder_imports(self, &mut m, &import);
        if m.conflict.is_some() {
            return None;
        }
        Some(CodeAction {
            title: "Convert to map literal".into(),
            kind: "refactor.convert.toMapLiteral".into(),
            edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `_isUnambiguousElement`: o elemento faz do literal um conjunto (uma
    /// expressão, através de `for` e `if`).
    fn unambiguous_element(&self, e: usize) -> bool {
        match self.kind(e) {
            "ForElement" => self.children(e).last().is_some_and(|&b| self.unambiguous_element(b)),
            "IfElement" => {
                // Os ramos: os filhos depois da condição (e do `case`).
                self.children(e).iter().skip(1).filter(|&&k| self.kind(k) != "CaseClause").any(|&k| self.unambiguous_element(k))
            }
            "SpreadElement" | "MapLiteralEntry" | "NullAwareElement" => false,
            _ => crate::refactorings::is_expression_kind(self.kind(e)),
        }
    }

    /// `_listHasUnambiguousElement`.
    fn unambiguous_list(&self, n: usize) -> bool {
        self.kind(n) == "ListLiteral" && self.list_elements(n).iter().any(|&e| self.unambiguous_element(e))
    }

    /// O tipo é `Set<…>` (a classe `Set` do `dart:core`).
    fn is_set_type(&self, t: dartforge_types::TypeId) -> bool {
        matches!(self.p.query.table.get(t), dartforge_types::Type::Interface { class, .. } if Some(*class) == self.p.query.core.set_class)
    }

    /// O tipo do `correspondingParameter` do argumento `arg` de uma
    /// invocação de método ou de função.
    fn matching_parameter_type(&self, arg: usize, invocation: usize) -> Option<dartforge_types::TypeId> {
        let list = self.parent(arg)?;
        let prog = self.p.program();
        let x = self.node_expr(invocation)?;
        let f = match self.bodies.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => *f,
            Resolved::Element(dartforge_elements::model::Element::Function(f)) => *f,
            _ => return None,
        };
        let _ = prog;
        let parameters = &self.p.query.outline.functions.get(f.0 as usize)?.parameters;
        let positional = self.children(list).iter().take_while(|&&k| k != arg).filter(|&&k| self.kind(k) != "NamedExpression").count();
        if self.kind(arg) == "NamedExpression" {
            let name = self.node_text(*self.children(arg).first()?).trim_end_matches(':').trim().to_string();
            return parameters
                .iter()
                .find(|p| p.kind == dartforge_frontend::ast::ParameterKind::Named && p.externo.or(p.name).is_some_and(|n| self.p.name(n) == name))
                .map(|p| p.ty);
        }
        parameters.iter().filter(|p| p.kind != dartforge_frontend::ast::ParameterKind::Named).nth(positional).map(|p| p.ty)
    }

    /// `ConvertToSetLiteral` (convert_to_set_literal.dart): `[…].toSet()`,
    /// `Set()`, `Set.from([…])` e `Set.of([…])` viram literal de conjunto, com
    /// `<dynamic>` quando o literal sem argumentos de tipo seria mapa.
    pub(crate) fn convert_to_set_literal(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let node_start = self.tree.nos[no].inicio;
        let opens_and_closes = |list: usize| -> Option<(Span, Span)> {
            let span = self.tree.span(list);
            let opens = self.tokens.iter().find(|t| t.span.start >= span.start && &self.source[t.span.start..t.span.end] == "[")?.span;
            let closes = self.previous_token(span.end)?;
            Some((opens, closes))
        };
        // `_findInvocationOfToSet`.
        if let Some(invocation) = self.with_parents(no).find(|&k| self.kind(k) == "MethodInvocation")
            && let Some(args) = self.children(invocation).iter().copied().find(|&k| self.kind(k) == "ArgumentList")
            && node_start <= self.tree.nos[args].inicio
            && self.method_name(invocation).is_some_and(|n| self.node_text(n) == "toSet")
            && let Some(&target) = self.children(invocation).first()
            && self.kind(target) == "ListLiteral"
            && self.method_name(invocation) != Some(target)
        {
            let (opens, closes) = opens_and_closes(target)?;
            let with_type = self.literal_type_arguments(target);
            let new = if with_type || self.unambiguous_list(target) { "{" } else { "<dynamic>{" };
            return Some(self.simple_action(
                uri,
                "Convert to set literal",
                "refactor.convert.toSetLiteral",
                vec![(opens, new.into()), (Span { start: closes.start, end: self.tree.nos[invocation].fim }, "}".into())],
            ));
        }
        // `_findSetCreation`.
        let creation = self.with_parents(no).find(|&k| self.kind(k) == "InstanceCreationExpression")?;
        let arguments = self.children(creation).iter().copied().find(|&k| self.kind(k) == "ArgumentList")?;
        if node_start > self.tree.nos[arguments].inicio {
            return None;
        }
        let ty = self.node_expr(creation).and_then(|x| self.bodies.get_type(x))?;
        if !self.is_set_type(ty) {
            return None;
        }
        let constructor_name = self.children(creation).iter().copied().find(|&k| self.kind(k) == "ConstructorName")?;
        let constructor_type = *self.children(constructor_name).first()?;
        let constructor_arguments = self.children(constructor_type).iter().copied().find(|&k| self.kind(k) == "TypeArgumentList");
        let mut element_arguments: Option<usize> = None;
        let mut elements: Option<Span> = None;
        match self.children(constructor_name).get(1).map(|&n| self.node_text(n)) {
            None => {}
            Some("from" | "of") => {
                let a = self.children(arguments);
                if a.len() != 1 || self.kind(a[0]) != "ListLiteral" {
                    return None;
                }
                element_arguments = self.children(a[0]).iter().copied().find(|&k| self.kind(k) == "TypeArgumentList");
                let (opens, closes) = opens_and_closes(a[0])?;
                elements = Some(Span { start: opens.end, end: closes.start });
            }
            Some(_) => return None,
        }
        // `_setWouldBeInferred`.
        let would_be_inferred = || -> bool {
            let Some(parent) = self.parent(creation) else { return false };
            if self.kind(parent) == "VariableDeclaration" {
                if let Some(list) = self.parent(parent)
                    && self.kind(list) == "VariableDeclarationList"
                    && let Some(&annotation) = self.children(list).iter().find(|&&f| self.kind(f) == "NamedType")
                    && let Some(&first) = self.children(list).iter().find(|&&f| self.kind(f) == "VariableDeclaration")
                    && let Some(name) = self.next_token(self.tree.nos[first].inicio)
                    && let Some(&t) = self.bodies.tipos_de_locais.get(&name.start)
                    && self.is_set_type(t)
                {
                    let _ = annotation;
                    return true;
                }
            } else if let Some(grandparent) = self.parent(parent)
                && matches!(self.kind(grandparent), "MethodInvocation" | "FunctionExpressionInvocation")
                && let Some(t) = self.matching_parameter_type(creation, grandparent)
                && self.is_set_type(t)
            {
                return true;
            }
            self.children(arguments).first().is_some_and(|&a| self.unambiguous_list(a))
        };
        let mut text = String::new();
        if let Some(a) = constructor_arguments {
            text.push_str(self.node_text(a));
        } else if let Some(a) = element_arguments {
            text.push_str(self.node_text(a));
        } else if !would_be_inferred() {
            text.push_str("<dynamic>");
        }
        text.push('{');
        if let Some(e) = elements {
            text.push_str(&self.source[e.start..e.end]);
        }
        text.push('}');
        Some(self.simple_action(uri, "Convert to set literal", "refactor.convert.toSetLiteral", vec![(self.tree.span(creation), text)]))
    }

    /// O literal tem argumentos de tipo escritos.
    fn literal_type_arguments(&self, n: usize) -> bool {
        self.children(n).iter().any(|&k| self.kind(k) == "TypeArgumentList")
    }

    /// `_extractClosure(name, argument)`: `name: (p) => e` (ou com um bloco
    /// de um só `return e`): o parâmetro (o nó e o nome) e o corpo `e`.
    fn named_closure(&self, name: &str, arg: usize) -> Option<(usize, Span, usize)> {
        if self.kind(arg) != "NamedExpression" {
            return None;
        }
        let f = self.children(arg);
        let label = *f.first()?;
        if self.node_text(label).trim_end_matches(':').trim() != name {
            return None;
        }
        let expression = self.without_parens(*f.get(1)?);
        if self.kind(expression) != "FunctionExpression" {
            return None;
        }
        let fe = self.children(expression);
        let list = fe.iter().copied().find(|&k| self.kind(k) == "FormalParameterList")?;
        let [parameter] = self.children(list)[..] else { return None };
        if self.kind(parameter) != "SimpleFormalParameter" {
            return None;
        }
        let name_p = self.previous_token(self.tree.nos[parameter].fim)?;
        let body = *fe.last()?;
        let body = match self.kind(body) {
            "ExpressionFunctionBody" => *self.children(body).first()?,
            "BlockFunctionBody" => {
                let block = *self.children(body).first()?;
                let [r] = self.children(block)[..] else { return None };
                if self.kind(r) != "ReturnStatement" {
                    return None;
                }
                *self.children(r).first()?
            }
            _ => return None,
        };
        Some((parameter, name_p, body))
    }

    /// `SimpleIdentifier.isQualified`.
    fn qualified(&self, n: usize) -> bool {
        let Some(p) = self.parent(n) else { return false };
        match self.kind(p) {
            "PrefixedIdentifier" | "PropertyAccess" => self.children(p).last() == Some(&n) && self.children(p).len() > 1,
            "MethodInvocation" => self.method_name(p) == Some(n) && self.children(p).first() != Some(&n),
            _ => false,
        }
    }

    /// `_ParameterReferenceFinder`: as citações do parâmetro (nome em
    /// `name`) e os outros nomes não qualificados de `body`.
    fn parameter_citations(&self, body: usize, name_: Span) -> (Vec<usize>, HashSet<String>) {
        let mut refs = Vec::new();
        let mut others = HashSet::new();
        let name_text = &self.source[name_.start..name_.end];
        let mut stack = vec![body];
        while let Some(k) = stack.pop() {
            if self.kind(k) == "SimpleIdentifier" {
                let of_parameter = match self.tree.nos[k].marca {
                    crate::arvore_analyzer::Marca::Expr(x) => match self.bodies.get_resolved(x) {
                        Some(Resolved::Parameter { name, .. }) => self.p.name(*name) == name_text,
                        Some(Resolved::Local(_)) => self.bodies.declaracao_local(x) == Some(name_.start),
                        _ => false,
                    },
                    _ => false,
                };
                if of_parameter {
                    refs.push(self.tree.nos[k].inicio);
                } else if !self.qualified(k) {
                    others.insert(self.node_text(k).to_string());
                }
            }
            stack.extend(self.children(k).iter().copied());
        }
        refs.sort_unstable();
        (refs, others)
    }

    /// `ConvertMapFromIterableToForLiteral`
    /// (convert_map_from_iterable_to_for_literal.dart): `Map.fromIterable(i,
    /// key: (k) => …, value: (v) => …)` vira `{ for (var e in i) … : … }`.
    pub(crate) fn map_from_iterable_to_for(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let no = self.tree.localizar(start, end)?;
        let creation = self.with_parents(no).find(|&k| self.kind(k) == "InstanceCreationExpression")?;
        let constructor_name = self.children(creation).iter().copied().find(|&k| self.kind(k) == "ConstructorName")?;
        if self.children(constructor_name).get(1).map(|&n| self.node_text(n)) != Some("fromIterable") {
            return None;
        }
        let ty = self.node_expr(creation).and_then(|x| self.bodies.get_type(x))?;
        if !matches!(self.p.query.table.get(ty), dartforge_types::Type::Interface { class, .. } if Some(*class) == self.p.query.core.map_class) {
            return None;
        }
        let arguments = self.list_arguments(creation);
        if arguments.len() != 3 {
            return None;
        }
        let iterator = self.without_parens(arguments[0]);
        let key = self.named_closure("key", arguments[1]).or_else(|| self.named_closure("key", arguments[2]))?;
        let value = self.named_closure("value", arguments[2]).or_else(|| self.named_closure("value", arguments[1]))?;
        let (_, name_k, body_k) = key;
        let (_, name_v, body_v) = value;
        let nk = self.source[name_k.start..name_k.end].to_string();
        let nv = self.source[name_v.start..name_v.end].to_string();
        let mut text_k = self.node_text(body_k).to_string();
        let mut text_v = self.node_text(body_v).to_string();
        let swap = |text: &str, refs: &[usize], base: usize, old: usize, new: &str| -> String {
            let mut s = text.to_string();
            for &r in refs.iter().rev() {
                let o = r - base;
                s.replace_range(o..o + old, new);
            }
            s
        };
        let variable = if nk == nv {
            nk
        } else {
            let (refs_k, others_k) = self.parameter_citations(body_k, name_k);
            let (refs_v, others_v) = self.parameter_citations(body_v, name_v);
            let free = || {
                let mut c = "e".to_string();
                let mut i = 1;
                while others_k.contains(&c) || others_v.contains(&c) {
                    c = format!("e{i}");
                    i += 1;
                }
                c
            };
            if refs_v.is_empty() {
                if others_v.contains(&nk) {
                    let n = free();
                    text_k = swap(&text_k, &refs_k, self.tree.nos[body_k].inicio, nk.len(), &n);
                    n
                } else {
                    nk
                }
            } else if refs_k.is_empty() {
                if others_k.contains(&nv) {
                    let n = free();
                    text_v = swap(&text_v, &refs_v, self.tree.nos[body_v].inicio, nv.len(), &n);
                    n
                } else {
                    nv
                }
            } else {
                let n = free();
                text_k = swap(&text_k, &refs_k, self.tree.nos[body_k].inicio, nk.len(), &n);
                text_v = swap(&text_v, &refs_v, self.tree.nos[body_v].inicio, nv.len(), &n);
                n
            }
        };
        let text = format!("{{ for (var {variable} in {}) {text_k} : {text_v} }}", self.node_text(iterator));
        Some(self.simple_action(uri, "Convert to a 'for' element", "refactor.convert.toForElement", vec![(self.tree.span(creation), text)]))
    }
}
