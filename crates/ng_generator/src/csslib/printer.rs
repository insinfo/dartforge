//! Port of the `CssPrinter` of `csslib/src/css_printer.dart` (csslib 1.0.2), in
//! compact mode (`visitTree(tree, pretty: false)`), which is what the shim
//! uses. In compact mode `_isTesting` holds: colors become names
//! (`hexToColorName`) and `@import` is written with `url(…)`.
use std::rc::Rc;

use super::Exception;
use super::tree::*;
use super::token_kind as tk;
use super::tokenizer::dart_trim;
use super::visitor::{R, Visitor};

/// `CssPrinter`.
#[derive(Default)]
pub(crate) struct CssPrinter {
    buff: String,
    is_in_keyframes: bool,
}

impl CssPrinter {
    /// `emit`.
    fn emit(&mut self, s: &str) {
        self.buff.push_str(s);
    }

    /// `_emitLBrace`.
    fn emit_l_brace(&mut self) {
        self.buff.push('{');
    }

    /// `_emitRBrace`.
    fn emit_r_brace(&mut self) {
        self.buff.push('}');
    }

    /// `_emitSemicolon`.
    fn emit_semicolon(&mut self, force_lf: bool) {
        self.buff.push(';');
        if force_lf {
            self.buff.push('\n');
        }
    }

    /// `toString`: the buffer without whitespace at the ends.
    pub fn text(&self) -> String {
        dart_trim(&self.buff).to_string()
    }

    /// `emitMediaQueries`.
    fn emit_media_queries(&mut self, queries: &[MediaQuery]) -> R<()> {
        for (i, q) in queries.iter().enumerate() {
            if i > 0 {
                self.emit(",");
            }
            self.visit_media_query(q)?;
        }
        Ok(())
    }

    /// `visitIncludeDirective(node, [topLevel])`.
    fn include(&mut self, i: &Include) {
        // `_emitLf()` at the top level: nothing in compact mode.
        self.emit(&format!("@include {}", i.name));
        self.emit_semicolon(true);
    }

    /// The `toString` of the simple selectors that the printer emits whole.
    fn selector_as_text(s: &SimpleSelector) -> R<String> {
        Ok(match &s.kind {
            // `ElementSelector.toString` => `name`.
            SimpleKind::Element => s.name().to_string(),
            // `'$namespace|${nameAsSimpleSelector!.name}'`.
            SimpleKind::Namespace { namespace } => {
                let ns = match namespace {
                    None => "",
                    Some(SimpleName::Wildcard) => "*",
                    Some(n) => n.name(),
                };
                format!("{ns}|{}", s.name())
            }
            // `'[$name${matchOperator()}${valueToString()}]'`.
            SimpleKind::Attribute { op, value } => {
                let operator = match *op {
                    tk::EQUALS => "=",
                    tk::INCLUDES => "~=",
                    tk::DASH_MATCH => "|=",
                    tk::PREFIX_MATCH => "^=",
                    tk::SUFFIX_MATCH => "$=",
                    tk::SUBSTRING_MATCH => "*=",
                    tk::NO_MATCH => "",
                    _ => "null",
                };
                let value = match value {
                    None => String::new(),
                    Some(AttributeValue::Ident(i)) => i.as_text().to_string(),
                    Some(AttributeValue::Str(t)) => format!("\"{t}\""),
                };
                format!("[{}{operator}{value}]", s.name())
            }
            // `'#$_name'`, `'.$_name'`: the `Identifier.toString`.
            SimpleKind::Id => format!("#{}", name_as_text(&s.name)),
            SimpleKind::Class => format!(".{}", name_as_text(&s.name)),
            SimpleKind::PseudoClass => format!(":{}", s.name()),
            SimpleKind::PseudoElement { legacy } => {
                format!("{}{}", if *legacy { ":" } else { "::" }, s.name())
            }
            _ => return Err(Exception("seletor sem toString")),
        })
    }
}

/// `_name.toString()` of `IdSelector`/`ClassSelector` (an `Identifier`).
fn name_as_text(n: &SimpleName) -> &str {
    match n {
        SimpleName::Ident(i) => i.as_text(),
        other => other.name(),
    }
}

impl Visitor for CssPrinter {
    fn visit_calc_term(&mut self, t: &Term) -> R<()> {
        self.emit(&format!("{}(", t.text));
        if let TermKind::Calc(expr) = &t.kind {
            self.visit_term(expr)?;
        }
        self.emit(")");
        Ok(())
    }

    fn visit_media_expression(&mut self, e: &MediaExpr) -> R<()> {
        self.emit(if e.and_operator { " AND " } else { " " });
        self.emit(&format!("({}", e.feature.name));
        if !e.exprs.expressions.is_empty() {
            self.emit(":");
            self.visit_expressions(&e.exprs)?;
        }
        self.emit(")");
        Ok(())
    }

    fn visit_media_query(&mut self, q: &MediaQuery) -> R<()> {
        let unary = if q.unary != -1 {
            let v = tk::id_to_value(tk::MEDIA_OPERATORS, q.unary).ok_or(Exception("TypeError"))?;
            format!(" {}", v.to_uppercase())
        } else {
            String::new()
        };
        let kind = match &q.kind {
            Some(t) => format!(" {}", t.name),
            None => String::new(),
        };
        self.emit(&format!("{unary}{kind}"));
        for e in &q.expressions {
            self.visit_media_expression(e)?;
        }
        Ok(())
    }

    fn visit_document_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Document { functions, body } = node {
            self.emit("@-moz-document ");
            let first = functions.first().ok_or(Exception("StateError"))?;
            self.visit_term(first)?;
            for f in functions.iter().skip(1) {
                self.emit(",");
                self.visit_term(f)?;
            }
            self.emit_l_brace();
            for r in body {
                self.visit_node(r)?;
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_supports_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Supports { condition, body } = node {
            self.emit("@supports ");
            let c = condition.as_mut().ok_or(Exception("TypeError"))?;
            self.visit_supports_condition(c)?;
            self.emit_l_brace();
            for r in body {
                self.visit_node(r)?;
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_supports_condition_in_parens(
        &mut self,
        inside: &mut Option<Box<InsideParens>>,
    ) -> R<()> {
        self.emit("(");
        match inside.as_deref_mut().ok_or(Exception("TypeError"))? {
            InsideParens::Declaration(d) => self.visit_any_declaration(d)?,
            InsideParens::Condition(c) => self.visit_supports_condition(c)?,
        }
        self.emit(")");
        Ok(())
    }

    fn visit_supports_negation(&mut self, c: &mut SupportsCondition) -> R<()> {
        self.emit("not");
        self.visit_supports_condition(c)
    }

    fn visit_supports_conjunction(&mut self, cs: &mut [SupportsCondition]) -> R<()> {
        let (first, rest) = cs.split_first_mut().ok_or(Exception("StateError"))?;
        self.visit_supports_condition(first)?;
        for c in rest {
            self.emit("and");
            self.visit_supports_condition(c)?;
        }
        Ok(())
    }

    fn visit_supports_disjunction(&mut self, cs: &mut [SupportsCondition]) -> R<()> {
        let (first, rest) = cs.split_first_mut().ok_or(Exception("StateError"))?;
        self.visit_supports_condition(first)?;
        for c in rest {
            self.emit("or");
            self.visit_supports_condition(c)?;
        }
        Ok(())
    }

    fn visit_viewport_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Viewport { name, declarations } = node {
            self.emit(&format!("@{name}"));
            self.emit_l_brace();
            self.visit_declaration_group(declarations)?;
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_media_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Media { queries, rules } = node {
            self.emit("@media");
            self.emit_media_queries(queries)?;
            self.emit_l_brace();
            for r in rules {
                self.visit_node(r)?;
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_host_directive(&mut self, rules: &mut [Node]) -> R<()> {
        self.emit("@host");
        self.emit_l_brace();
        for r in rules {
            self.visit_node(r)?;
        }
        self.emit_r_brace();
        Ok(())
    }

    fn visit_page_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Page {
            ident,
            pseudo,
            groups,
        } = node
        {
            self.emit("@page");
            if !ident.is_empty() || !pseudo.is_empty() {
                if !ident.is_empty() {
                    self.emit(" ");
                }
                let ident = ident.clone();
                self.emit(&ident);
                if !pseudo.is_empty() {
                    let p = format!(":{pseudo}");
                    self.emit(&p);
                }
            }
            self.emit_l_brace();
            for g in groups {
                if g.margin.is_some() {
                    self.visit_margin_group(g)?;
                } else {
                    self.visit_declaration_group(g)?;
                }
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_charset_directive(&mut self, encoding: &str) -> R<()> {
        self.emit(&format!("@charset \"{encoding}\""));
        self.emit_semicolon(true);
        Ok(())
    }

    fn visit_import_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Import { import, media_queries } = node {
            // `_isTesting`: always as `url(…)`.
            self.emit(&format!("@import url({import})"));
            self.emit_media_queries(media_queries)?;
            self.emit_semicolon(true);
        }
        Ok(())
    }

    fn visit_key_frame_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Keyframes { kind, name, blocks } = node {
            // `keyFrameName`.
            let directive_name = match *kind {
                tk::DIRECTIVE_KEYFRAMES | tk::DIRECTIVE_MS_KEYFRAMES => "@keyframes",
                tk::DIRECTIVE_WEB_KIT_KEYFRAMES => "@-webkit-keyframes",
                tk::DIRECTIVE_MOZ_KEYFRAMES => "@-moz-keyframes",
                tk::DIRECTIVE_O_KEYFRAMES => "@-o-keyframes",
                _ => "null",
            };
            self.emit(&format!("{directive_name} "));
            let name = name.as_ref().ok_or(Exception("TypeError"))?;
            self.visit_identifier(name)?;
            self.emit_l_brace();
            self.is_in_keyframes = true;
            for b in blocks {
                self.visit_key_frame_block(b)?;
            }
            self.is_in_keyframes = false;
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_font_face_directive(&mut self, g: &mut DeclarationGroup) -> R<()> {
        self.emit("@font-face");
        self.emit_l_brace();
        self.visit_declaration_group(g)?;
        self.emit_r_brace();
        Ok(())
    }

    fn visit_key_frame_block(&mut self, b: &mut KeyframeBlock) -> R<()> {
        self.visit_expressions(&b.selectors)?;
        self.emit_l_brace();
        self.visit_declaration_group(&mut b.declarations)?;
        self.emit_r_brace();
        Ok(())
    }

    fn visit_namespace_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Namespace { prefix, uri } = node {
            let uri = uri.as_deref().ok_or(Exception("TypeError"))?;
            // `prefix`: followed by a space, if present.
            let prefix = if prefix.is_empty() {
                String::new()
            } else {
                format!("{prefix} ")
            };
            // `isStartingQuote(_uri!)`: `'\'"'.contains(uri)`.
            if "'\"".contains(uri) {
                self.emit(&format!("@namespace {prefix}\"{uri}\""));
            } else {
                self.emit(&format!("@namespace {prefix}url({uri})"));
            }
            self.emit_semicolon(true);
        }
        Ok(())
    }

    fn visit_var_definition_directive(&mut self, d: &mut VarDef) -> R<()> {
        self.visit_var_definition(d)?;
        self.emit_semicolon(false);
        Ok(())
    }

    fn visit_mixin_ruleset_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::MixinRules { name, rules } = node {
            self.emit(&format!("@mixin {name} "));
            self.emit_l_brace();
            for r in rules {
                self.visit_node(r)?;
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_mixin_declaration_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::MixinDeclarations { name, declarations } = node {
            self.emit(&format!("@mixin {name}"));
            self.emit_l_brace();
            self.visit_declaration_group(declarations)?;
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_include_directive(&mut self, i: &mut Include) -> R<()> {
        self.include(i);
        Ok(())
    }

    fn visit_rule_set(&mut self, r: &mut RuleSet) -> R<()> {
        self.visit_selector_group(&mut r.group)?;
        self.emit_l_brace();
        self.visit_declaration_group(&mut r.declarations)?;
        self.emit_r_brace();
        Ok(())
    }

    fn visit_declaration_group(&mut self, g: &mut DeclarationGroup) -> R<()> {
        let n = g.declarations.len();
        for (i, d) in g.declarations.iter_mut().enumerate() {
            self.visit_node(d)?;
            // No final `;` in compact mode.
            if i + 1 < n {
                self.emit_semicolon(false);
            }
        }
        Ok(())
    }

    fn visit_margin_group(&mut self, g: &mut DeclarationGroup) -> R<()> {
        let name = g
            .margin
            .and_then(|m| tk::id_to_value(tk::MARGIN_DIRECTIVES, m))
            .unwrap_or("null");
        self.emit(&format!("@{name}"));
        self.emit_l_brace();
        self.visit_declaration_group(g)?;
        self.emit_r_brace();
        Ok(())
    }

    fn visit_declaration(&mut self, d: &mut Declaration) -> R<()> {
        if let Declaration::Plain {
            property,
            expression,
            important,
            ie7,
        } = d
        {
            // `property`: with `*` for the IE7 hack.
            let prop = if *ie7 {
                format!("*{}", property.name)
            } else {
                property.name.clone()
            };
            self.emit(&format!("{prop}:"));
            self.visit_expressions(expression)?;
            if *important {
                self.emit("!important");
            }
        }
        Ok(())
    }

    fn visit_var_definition(&mut self, d: &mut VarDef) -> R<()> {
        let name = d.name.as_ref().ok_or(Exception("TypeError"))?;
        self.emit(&format!("var-{}: ", name.name));
        let e = d.expression.as_ref().ok_or(Exception("TypeError"))?;
        self.visit_expressions(e)
    }

    fn visit_include_mixin_at_declaration(&mut self, i: &mut Include) -> R<()> {
        // No line break: it is inside a declaration group.
        self.include(i);
        Ok(())
    }

    fn visit_extend_declaration(&mut self, selectors: &mut [Rc<SimpleSelector>]) -> R<()> {
        self.emit("@extend ");
        for s in selectors.iter() {
            self.visit_simple(s)?;
        }
        Ok(())
    }

    fn visit_selector_group(&mut self, g: &mut SelectorGroup) -> R<()> {
        for (i, s) in g.selectors.iter().enumerate() {
            if i > 0 {
                self.emit(",");
            }
            self.visit_selector(s)?;
        }
        Ok(())
    }

    fn visit_simple_selector_sequence(&mut self, seq: &Sequence) -> R<()> {
        self.emit(seq.combinator_as_str());
        self.visit_simple(&seq.selector)
    }

    fn visit_simple_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.emit(s.name());
        Ok(())
    }

    fn visit_namespace_selector(&mut self, s: &SimpleSelector) -> R<()> {
        let t = Self::selector_as_text(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_element_selector(&mut self, s: &SimpleSelector) -> R<()> {
        let t = Self::selector_as_text(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_attribute_selector(&mut self, s: &SimpleSelector) -> R<()> {
        let t = Self::selector_as_text(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_id_selector(&mut self, s: &SimpleSelector) -> R<()> {
        let t = Self::selector_as_text(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_class_selector(&mut self, s: &SimpleSelector) -> R<()> {
        let t = Self::selector_as_text(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_pseudo_class_selector(&mut self, s: &SimpleSelector) -> R<()> {
        let t = Self::selector_as_text(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_pseudo_element_selector(&mut self, s: &SimpleSelector) -> R<()> {
        let t = Self::selector_as_text(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_pseudo_class_function_selector(&mut self, s: &SimpleSelector) -> R<()> {
        if let SimpleKind::PseudoClassFunction(arg) = &s.kind {
            self.emit(&format!(":{}(", s.name()));
            match arg {
                PseudoArgument::Selector(sel) => self.visit_selector(sel)?,
                PseudoArgument::Expression(e) => self.visit_selector_expression(e)?,
            }
            self.emit(")");
        }
        Ok(())
    }

    fn visit_pseudo_element_function_selector(&mut self, s: &SimpleSelector) -> R<()> {
        if let SimpleKind::PseudoElementFunction(e) = &s.kind {
            self.emit(&format!("::{}(", s.name()));
            self.visit_selector_expression(e)?;
            self.emit(")");
        }
        Ok(())
    }

    fn visit_negation_selector(&mut self, s: &SimpleSelector) -> R<()> {
        if let SimpleKind::Negation(arg) = &s.kind {
            self.emit(":not(");
            let arg = arg.as_ref().ok_or(Exception("TypeError"))?;
            self.visit_simple(arg)?;
            self.emit(")");
        }
        Ok(())
    }

    fn visit_selector_expression(&mut self, exprs: &[Expr]) -> R<()> {
        for e in exprs {
            self.visit_expr(e)?;
        }
        Ok(())
    }

    fn visit_unicode_range_term(&mut self, p: Option<&str>, s: Option<&str>) -> R<()> {
        let p = p.unwrap_or("null");
        match s {
            Some(s) => self.emit(&format!("U+{p}-{s}")),
            None => self.emit(&format!("U+{p}")),
        }
        Ok(())
    }

    fn visit_literal_term(&mut self, t: &Term) -> R<()> {
        self.emit(&t.text);
        Ok(())
    }

    fn visit_hex_color_term(&mut self, t: &Term) -> R<()> {
        let name = match t.value {
            Value::Int(v) => tk::hex_to_color_name(v),
            _ => None,
        };
        match name {
            Some(n) => self.emit(n),
            None => self.emit(&format!("#{}", t.text)),
        }
        Ok(())
    }

    fn visit_number_term(&mut self, t: &Term) -> R<()> {
        self.visit_literal_term(t)
    }

    fn visit_unit_term(&mut self, t: &Term) -> R<()> {
        // `UnitTerm.toString`: `'$text${unitToString()}'`.
        if let TermKind::Unit { unit, .. } = t.kind {
            self.emit(&format!("{}{}", t.text, tk::unit_to_string(unit)));
        }
        Ok(())
    }

    fn visit_percentage_term(&mut self, t: &Term) -> R<()> {
        self.emit(&format!("{}%", t.text));
        Ok(())
    }

    fn visit_em_term(&mut self, t: &Term) -> R<()> {
        self.emit(&format!("{}em", t.text));
        Ok(())
    }

    fn visit_ex_term(&mut self, t: &Term) -> R<()> {
        self.emit(&format!("{}ex", t.text));
        Ok(())
    }

    fn visit_fraction_term(&mut self, t: &Term) -> R<()> {
        self.emit(&format!("{}fr", t.text));
        Ok(())
    }

    fn visit_uri_term(&mut self, t: &Term) -> R<()> {
        self.emit(&format!("url(\"{}\")", t.text));
        Ok(())
    }

    fn visit_function_term(&mut self, t: &Term) -> R<()> {
        self.emit(&format!("{}(", t.text));
        if let TermKind::Function(params) = &t.kind {
            self.visit_expressions(params)?;
        }
        self.emit(")");
        Ok(())
    }

    fn visit_group_term(&mut self, terms: &[Term]) -> R<()> {
        self.emit("(");
        for t in terms {
            self.visit_term(t)?;
        }
        self.emit(")");
        Ok(())
    }

    fn visit_item_term(&mut self, t: &Term) -> R<()> {
        self.emit(&format!("[{}]", t.text));
        Ok(())
    }

    fn visit_ie8_term(&mut self, t: &Term) -> R<()> {
        self.visit_literal_term(t)
    }

    fn visit_operator_slash(&mut self) -> R<()> {
        self.emit("/");
        Ok(())
    }

    fn visit_operator_comma(&mut self) -> R<()> {
        self.emit(",");
        Ok(())
    }

    fn visit_operator_plus(&mut self) -> R<()> {
        self.emit("+");
        Ok(())
    }

    fn visit_operator_minus(&mut self) -> R<()> {
        self.emit("-");
        Ok(())
    }

    fn visit_var_usage(&mut self, name: &str, defaults: &[Expr]) -> R<()> {
        self.emit(&format!("var({name}"));
        if !defaults.is_empty() {
            self.emit(",");
            for p in defaults {
                self.emit(" ");
                self.visit_expr(p)?;
            }
        }
        self.emit(")");
        Ok(())
    }

    fn visit_expressions(&mut self, e: &Expressions) -> R<()> {
        let operator = |x: &Expr| matches!(x, Expr::Comma | Expr::Slash);
        let percentage = |x: &Expr| {
            matches!(
                x,
                Expr::Term(Term {
                    kind: TermKind::Percentage,
                    ..
                })
            )
        };
        for (i, x) in e.expressions.iter().enumerate() {
            // Space between terms without an operator.
            if i > 0 && !operator(x) {
                let previous = &e.expressions[i - 1];
                if operator(previous) {
                    // `_sp`: nothing in compact mode.
                } else if percentage(previous) && percentage(x) && self.is_in_keyframes {
                    self.emit(",");
                } else {
                    self.emit(" ");
                }
            }
            self.visit_expr(x)?;
        }
        Ok(())
    }

    fn visit_identifier(&mut self, i: &Identifier) -> R<()> {
        self.emit(&i.name);
        Ok(())
    }
}
