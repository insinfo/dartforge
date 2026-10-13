//! Port of the `Visitor` of `csslib/visitor.dart` (csslib 1.0.2): the tree walk
//! that `_ShadowTransformer` inherits and `CssPrinter` overrides.
//!
//! Each default method does what the official one does, including the `!` that
//! throws on null (`KeyFrameDirective.name`, `SupportsDirective.condition`,
//! `VarDefinition._property`). The subclasses of `UnitTerm` all fall into
//! `visit_unit_term`, which is where the official `Visitor` sends them.
use super::Exception;
use super::tree::*;

pub(crate) type R<T> = Result<T, Exception>;

/// `Visitor`.
pub(crate) trait Visitor {
    /// `visitTree`.
    fn visit_tree(&mut self, sheet: &mut StyleSheet) -> R<()> {
        self.visit_style_sheet(sheet)
    }

    /// `visitStyleSheet`.
    fn visit_style_sheet(&mut self, sheet: &mut StyleSheet) -> R<()> {
        for node in &mut sheet.top_level {
            self.visit_node(node)?;
        }
        Ok(())
    }

    /// `TreeNode.visit`: the double dispatch of the list nodes.
    fn visit_node(&mut self, node: &mut Node) -> R<()> {
        match node {
            Node::Rule(r) => self.visit_rule_set(r),
            Node::Declaration(d) => self.visit_any_declaration(d),
            Node::Import { .. } => self.visit_import_directive(node),
            Node::Media { .. } => self.visit_media_directive(node),
            Node::Host { rules } => self.visit_host_directive(rules),
            Node::Page { .. } => self.visit_page_directive(node),
            Node::Charset(c) => self.visit_charset_directive(c),
            Node::Keyframes { .. } => self.visit_key_frame_directive(node),
            Node::FontFace(g) => self.visit_font_face_directive(g),
            Node::Namespace { .. } => self.visit_namespace_directive(node),
            Node::VarDefDirective(d) => self.visit_var_definition_directive(d),
            Node::MixinRules { .. } => self.visit_mixin_ruleset_directive(node),
            Node::MixinDeclarations { .. } => self.visit_mixin_declaration_directive(node),
            Node::Include(i) => self.visit_include_directive(i),
            Node::Document { .. } => self.visit_document_directive(node),
            Node::Supports { .. } => self.visit_supports_directive(node),
            Node::Viewport { .. } => self.visit_viewport_directive(node),
        }
    }

    /// The dispatch of the subclasses of `Declaration`.
    fn visit_any_declaration(&mut self, d: &mut Declaration) -> R<()> {
        match d {
            Declaration::Plain { .. } => self.visit_declaration(d),
            Declaration::VarDef(v) => self.visit_var_definition(v),
            Declaration::Include(i) => self.visit_include_mixin_at_declaration(i),
            Declaration::Extend(s) => self.visit_extend_declaration(s),
        }
    }

    /// `visitMediaExpression`.
    fn visit_media_expression(&mut self, e: &MediaExpr) -> R<()> {
        self.visit_expressions(&e.exprs)
    }

    /// `visitMediaQuery`.
    fn visit_media_query(&mut self, q: &MediaQuery) -> R<()> {
        for e in &q.expressions {
            self.visit_media_expression(e)?;
        }
        Ok(())
    }

    /// `visitDocumentDirective`.
    fn visit_document_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Document { functions, body } = node {
            for f in functions.iter() {
                self.visit_term(f)?;
            }
            for r in body {
                self.visit_node(r)?;
            }
        }
        Ok(())
    }

    /// `visitSupportsDirective`.
    fn visit_supports_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Supports { condition, body } = node {
            let c = condition.as_mut().ok_or(Exception("TypeError"))?;
            self.visit_supports_condition(c)?;
            for r in body {
                self.visit_node(r)?;
            }
        }
        Ok(())
    }

    /// The dispatch of the subclasses of `SupportsCondition`.
    fn visit_supports_condition(&mut self, c: &mut SupportsCondition) -> R<()> {
        match c {
            SupportsCondition::InParens(inside) => self.visit_supports_condition_in_parens(inside),
            SupportsCondition::Negation(c) => self.visit_supports_negation(c),
            SupportsCondition::Conjunction(cs) => self.visit_supports_conjunction(cs),
            SupportsCondition::Disjunction(cs) => self.visit_supports_disjunction(cs),
        }
    }

    /// `visitSupportsConditionInParens`.
    fn visit_supports_condition_in_parens(
        &mut self,
        inside: &mut Option<Box<InsideParens>>,
    ) -> R<()> {
        match inside.as_deref_mut().ok_or(Exception("TypeError"))? {
            InsideParens::Declaration(d) => self.visit_any_declaration(d),
            InsideParens::Condition(c) => self.visit_supports_condition(c),
        }
    }

    /// `visitSupportsNegation`.
    fn visit_supports_negation(&mut self, c: &mut SupportsCondition) -> R<()> {
        self.visit_supports_condition(c)
    }

    /// `visitSupportsConjunction`.
    fn visit_supports_conjunction(&mut self, cs: &mut [SupportsCondition]) -> R<()> {
        for c in cs {
            self.visit_supports_condition(c)?;
        }
        Ok(())
    }

    /// `visitSupportsDisjunction`.
    fn visit_supports_disjunction(&mut self, cs: &mut [SupportsCondition]) -> R<()> {
        for c in cs {
            self.visit_supports_condition(c)?;
        }
        Ok(())
    }

    /// `visitViewportDirective`.
    fn visit_viewport_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Viewport { declarations, .. } = node {
            self.visit_declaration_group(declarations)?;
        }
        Ok(())
    }

    /// `visitMediaDirective`.
    fn visit_media_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Media { queries, rules } = node {
            for q in queries.iter() {
                self.visit_media_query(q)?;
            }
            for r in rules {
                self.visit_node(r)?;
            }
        }
        Ok(())
    }

    /// `visitHostDirective`.
    fn visit_host_directive(&mut self, rules: &mut [Node]) -> R<()> {
        for r in rules {
            self.visit_node(r)?;
        }
        Ok(())
    }

    /// `visitPageDirective`.
    fn visit_page_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Page { groups, .. } = node {
            for g in groups {
                if g.margin.is_some() {
                    self.visit_margin_group(g)?;
                } else {
                    self.visit_declaration_group(g)?;
                }
            }
        }
        Ok(())
    }

    /// `visitCharsetDirective`.
    fn visit_charset_directive(&mut self, _encoding: &str) -> R<()> {
        Ok(())
    }

    /// `visitImportDirective`.
    fn visit_import_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Import { media_queries, .. } = node {
            for q in media_queries.iter() {
                self.visit_media_query(q)?;
            }
        }
        Ok(())
    }

    /// `visitKeyFrameDirective`.
    fn visit_key_frame_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::Keyframes { name, blocks, .. } = node {
            let name = name.as_ref().ok_or(Exception("TypeError"))?;
            self.visit_identifier(name)?;
            for b in blocks {
                self.visit_key_frame_block(b)?;
            }
        }
        Ok(())
    }

    /// `visitKeyFrameBlock`.
    fn visit_key_frame_block(&mut self, b: &mut KeyframeBlock) -> R<()> {
        self.visit_expressions(&b.selectors)?;
        self.visit_declaration_group(&mut b.declarations)
    }

    /// `visitFontFaceDirective`.
    fn visit_font_face_directive(&mut self, g: &mut DeclarationGroup) -> R<()> {
        self.visit_declaration_group(g)
    }

    /// `visitNamespaceDirective`.
    fn visit_namespace_directive(&mut self, _node: &mut Node) -> R<()> {
        Ok(())
    }

    /// `visitVarDefinitionDirective`.
    fn visit_var_definition_directive(&mut self, d: &mut VarDef) -> R<()> {
        self.visit_var_definition(d)
    }

    /// `visitMixinRulesetDirective`.
    fn visit_mixin_ruleset_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::MixinRules { rules, .. } = node {
            for r in rules {
                self.visit_node(r)?;
            }
        }
        Ok(())
    }

    /// `visitMixinDeclarationDirective`.
    fn visit_mixin_declaration_directive(&mut self, node: &mut Node) -> R<()> {
        if let Node::MixinDeclarations { declarations, .. } = node {
            self.visit_declaration_group(declarations)?;
        }
        Ok(())
    }

    /// `visitIncludeDirective`.
    fn visit_include_directive(&mut self, i: &mut Include) -> R<()> {
        for arg in &i.args {
            for e in arg {
                self.visit_expr(e)?;
            }
        }
        Ok(())
    }

    /// `visitRuleSet`.
    fn visit_rule_set(&mut self, r: &mut RuleSet) -> R<()> {
        self.visit_selector_group(&mut r.group)?;
        self.visit_declaration_group(&mut r.declarations)
    }

    /// `visitDeclarationGroup`.
    fn visit_declaration_group(&mut self, g: &mut DeclarationGroup) -> R<()> {
        for d in &mut g.declarations {
            self.visit_node(d)?;
        }
        Ok(())
    }

    /// `visitMarginGroup`.
    fn visit_margin_group(&mut self, g: &mut DeclarationGroup) -> R<()> {
        self.visit_declaration_group(g)
    }

    /// `visitDeclaration`.
    fn visit_declaration(&mut self, d: &mut Declaration) -> R<()> {
        if let Declaration::Plain {
            property,
            expression,
            ..
        } = d
        {
            self.visit_identifier(property)?;
            self.visit_expressions(expression)?;
        }
        Ok(())
    }

    /// `visitVarDefinition`.
    fn visit_var_definition(&mut self, d: &mut VarDef) -> R<()> {
        let name = d.name.as_ref().ok_or(Exception("TypeError"))?;
        self.visit_identifier(name)?;
        if let Some(e) = &d.expression {
            self.visit_expressions(e)?;
        }
        Ok(())
    }

    /// `visitIncludeMixinAtDeclaration`.
    fn visit_include_mixin_at_declaration(&mut self, i: &mut Include) -> R<()> {
        self.visit_include_directive(i)
    }

    /// `visitExtendDeclaration`.
    fn visit_extend_declaration(&mut self, selectors: &mut [std::rc::Rc<SimpleSelector>]) -> R<()> {
        for s in selectors.iter() {
            self.visit_simple(s)?;
        }
        Ok(())
    }

    /// `visitSelectorGroup`.
    fn visit_selector_group(&mut self, g: &mut SelectorGroup) -> R<()> {
        for s in &g.selectors {
            self.visit_selector(s)?;
        }
        Ok(())
    }

    /// `visitSelector`.
    fn visit_selector(&mut self, s: &Selector) -> R<()> {
        for seq in &s.sequences {
            self.visit_simple_selector_sequence(seq)?;
        }
        Ok(())
    }

    /// `visitSimpleSelectorSequence`.
    fn visit_simple_selector_sequence(&mut self, seq: &Sequence) -> R<()> {
        self.visit_simple(&seq.selector)
    }

    /// `SimpleSelector.visit`: the dispatch of the subclasses.
    fn visit_simple(&mut self, s: &SimpleSelector) -> R<()> {
        match &s.kind {
            SimpleKind::Element => self.visit_element_selector(s),
            SimpleKind::Namespace { .. } => self.visit_namespace_selector(s),
            SimpleKind::Attribute { .. } => self.visit_attribute_selector(s),
            SimpleKind::Id => self.visit_id_selector(s),
            SimpleKind::Class => self.visit_class_selector(s),
            SimpleKind::PseudoClass => self.visit_pseudo_class_selector(s),
            SimpleKind::PseudoElement { .. } => self.visit_pseudo_element_selector(s),
            SimpleKind::PseudoClassFunction(_) => self.visit_pseudo_class_function_selector(s),
            SimpleKind::PseudoElementFunction(_) => self.visit_pseudo_element_function_selector(s),
            SimpleKind::Negation(_) => self.visit_negation_selector(s),
        }
    }

    /// `visitSimpleSelector`: visits the `_name`.
    fn visit_simple_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_name(&s.name)
    }

    /// `(_name as TreeNode).visit(this)`.
    fn visit_simple_name(&mut self, n: &SimpleName) -> R<()> {
        match n {
            SimpleName::Ident(i) => self.visit_identifier(i),
            SimpleName::Selector(s) => self.visit_simple(s),
            SimpleName::Wildcard | SimpleName::This | SimpleName::Negation => Ok(()),
        }
    }

    /// `visitNamespaceSelector`.
    fn visit_namespace_selector(&mut self, s: &SimpleSelector) -> R<()> {
        if let SimpleKind::Namespace {
            namespace: Some(ns),
        } = &s.kind
        {
            self.visit_simple_name(ns)?;
        }
        self.visit_simple_name(&s.name)
    }

    /// `visitElementSelector`.
    fn visit_element_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitAttributeSelector`.
    fn visit_attribute_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitIdSelector`.
    fn visit_id_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitClassSelector`.
    fn visit_class_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitPseudoClassSelector`.
    fn visit_pseudo_class_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitPseudoElementSelector`.
    fn visit_pseudo_element_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitPseudoClassFunctionSelector`.
    fn visit_pseudo_class_function_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitPseudoElementFunctionSelector`.
    fn visit_pseudo_element_function_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitNegationSelector`.
    fn visit_negation_selector(&mut self, s: &SimpleSelector) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitSelectorExpression`.
    fn visit_selector_expression(&mut self, exprs: &[Expr]) -> R<()> {
        for e in exprs {
            self.visit_expr(e)?;
        }
        Ok(())
    }

    /// `Expression.visit`: the dispatch of expressions.
    fn visit_expr(&mut self, e: &Expr) -> R<()> {
        match e {
            Expr::Term(t) => self.visit_term(t),
            Expr::UnicodeRange { first, second } => {
                self.visit_unicode_range_term(first.as_deref(), second.as_deref())
            }
            Expr::VarUsage { name, defaults } => self.visit_var_usage(name, defaults),
            Expr::Slash => self.visit_operator_slash(),
            Expr::Comma => self.visit_operator_comma(),
            Expr::Plus => self.visit_operator_plus(),
            Expr::Minus => self.visit_operator_minus(),
            Expr::Group(terms) => self.visit_group_term(terms),
        }
    }

    /// `LiteralTerm.visit` and subclasses.
    fn visit_term(&mut self, t: &Term) -> R<()> {
        match &t.kind {
            TermKind::Literal => self.visit_literal_term(t),
            TermKind::Number => self.visit_number_term(t),
            TermKind::Item => self.visit_item_term(t),
            TermKind::Unit { .. } => self.visit_unit_term(t),
            TermKind::Percentage => self.visit_percentage_term(t),
            TermKind::Em => self.visit_em_term(t),
            TermKind::Ex => self.visit_ex_term(t),
            TermKind::Fraction => self.visit_fraction_term(t),
            TermKind::Uri => self.visit_uri_term(t),
            TermKind::Hex => self.visit_hex_color_term(t),
            TermKind::Ie8 => self.visit_ie8_term(t),
            TermKind::Function(_) => self.visit_function_term(t),
            TermKind::Calc(_) => self.visit_calc_term(t),
        }
    }

    /// `visitCalcTerm`.
    fn visit_calc_term(&mut self, t: &Term) -> R<()> {
        self.visit_literal_term(t)?;
        if let TermKind::Calc(expr) = &t.kind {
            self.visit_literal_term(expr)?;
        }
        Ok(())
    }

    /// `visitUnicodeRangeTerm`.
    fn visit_unicode_range_term(&mut self, _p: Option<&str>, _s: Option<&str>) -> R<()> {
        Ok(())
    }

    /// `visitLiteralTerm`.
    fn visit_literal_term(&mut self, _t: &Term) -> R<()> {
        Ok(())
    }

    /// `visitHexColorTerm`.
    fn visit_hex_color_term(&mut self, _t: &Term) -> R<()> {
        Ok(())
    }

    /// `visitNumberTerm`.
    fn visit_number_term(&mut self, _t: &Term) -> R<()> {
        Ok(())
    }

    /// `visitUnitTerm` (and the subclasses, which the official code sends to
    /// it).
    fn visit_unit_term(&mut self, _t: &Term) -> R<()> {
        Ok(())
    }

    /// `visitPercentageTerm`.
    fn visit_percentage_term(&mut self, t: &Term) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitEmTerm`.
    fn visit_em_term(&mut self, t: &Term) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitExTerm`.
    fn visit_ex_term(&mut self, t: &Term) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitFractionTerm`.
    fn visit_fraction_term(&mut self, t: &Term) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitUriTerm`.
    fn visit_uri_term(&mut self, t: &Term) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitFunctionTerm`.
    fn visit_function_term(&mut self, t: &Term) -> R<()> {
        self.visit_literal_term(t)?;
        if let TermKind::Function(params) = &t.kind {
            self.visit_expressions(params)?;
        }
        Ok(())
    }

    /// `visitGroupTerm`.
    fn visit_group_term(&mut self, terms: &[Term]) -> R<()> {
        for t in terms {
            self.visit_term(t)?;
        }
        Ok(())
    }

    /// `visitItemTerm`.
    fn visit_item_term(&mut self, t: &Term) -> R<()> {
        self.visit_number_term(t)
    }

    /// `visitIE8Term`.
    fn visit_ie8_term(&mut self, _t: &Term) -> R<()> {
        Ok(())
    }

    /// `visitOperatorSlash`.
    fn visit_operator_slash(&mut self) -> R<()> {
        Ok(())
    }

    /// `visitOperatorComma`.
    fn visit_operator_comma(&mut self) -> R<()> {
        Ok(())
    }

    /// `visitOperatorPlus`.
    fn visit_operator_plus(&mut self) -> R<()> {
        Ok(())
    }

    /// `visitOperatorMinus`.
    fn visit_operator_minus(&mut self) -> R<()> {
        Ok(())
    }

    /// `visitVarUsage`.
    fn visit_var_usage(&mut self, _name: &str, defaults: &[Expr]) -> R<()> {
        for e in defaults {
            self.visit_expr(e)?;
        }
        Ok(())
    }

    /// `visitExpressions`.
    fn visit_expressions(&mut self, e: &Expressions) -> R<()> {
        for x in &e.expressions {
            self.visit_expr(x)?;
        }
        Ok(())
    }

    /// `visitIdentifier`.
    fn visit_identifier(&mut self, _i: &Identifier) -> R<()> {
        Ok(())
    }
}
