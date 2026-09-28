//! Porte do `Visitor` de `csslib/visitor.dart` (csslib 1.0.2): o percurso
//! da árvore que o `_ShadowTransformer` herda e a `CssPrinter` sobrepõe.
//!
//! Cada método padrão faz o que o oficial faz, inclusive o `!` que lança em
//! nulo (`KeyFrameDirective.name`, `SupportsDirective.condition`,
//! `VarDefinition._property`). As subclasses de `UnitTerm` caem todas em
//! `visit_unit_term`, que é para onde o `Visitor` oficial as manda.
use super::Excecao;
use super::arvore::*;

pub(crate) type R<T> = Result<T, Excecao>;

/// `Visitor`.
pub(crate) trait Visitor {
    /// `visitTree`.
    fn visit_tree(&mut self, folha: &mut Folha) -> R<()> {
        self.visit_style_sheet(folha)
    }

    /// `visitStyleSheet`.
    fn visit_style_sheet(&mut self, folha: &mut Folha) -> R<()> {
        for no in &mut folha.topo {
            self.visit_no(no)?;
        }
        Ok(())
    }

    /// `TreeNode.visit`: o despacho duplo dos nós de lista.
    fn visit_no(&mut self, no: &mut No) -> R<()> {
        match no {
            No::Regra(r) => self.visit_rule_set(r),
            No::Declaracao(d) => self.visit_declaracao(d),
            No::Import { .. } => self.visit_import_directive(no),
            No::Media { .. } => self.visit_media_directive(no),
            No::Host { regras } => self.visit_host_directive(regras),
            No::Pagina { .. } => self.visit_page_directive(no),
            No::Charset(c) => self.visit_charset_directive(c),
            No::Keyframes { .. } => self.visit_key_frame_directive(no),
            No::FontFace(g) => self.visit_font_face_directive(g),
            No::Namespace { .. } => self.visit_namespace_directive(no),
            No::VarDefDiretiva(d) => self.visit_var_definition_directive(d),
            No::MixinRegras { .. } => self.visit_mixin_ruleset_directive(no),
            No::MixinDeclaracoes { .. } => self.visit_mixin_declaration_directive(no),
            No::Include(i) => self.visit_include_directive(i),
            No::Documento { .. } => self.visit_document_directive(no),
            No::Supports { .. } => self.visit_supports_directive(no),
            No::Viewport { .. } => self.visit_viewport_directive(no),
        }
    }

    /// O despacho das subclasses de `Declaration`.
    fn visit_declaracao(&mut self, d: &mut Declaracao) -> R<()> {
        match d {
            Declaracao::Comum { .. } => self.visit_declaration(d),
            Declaracao::VarDef(v) => self.visit_var_definition(v),
            Declaracao::Include(i) => self.visit_include_mixin_at_declaration(i),
            Declaracao::Extend(s) => self.visit_extend_declaration(s),
        }
    }

    /// `visitMediaExpression`.
    fn visit_media_expression(&mut self, e: &ExprMedia) -> R<()> {
        self.visit_expressions(&e.exprs)
    }

    /// `visitMediaQuery`.
    fn visit_media_query(&mut self, q: &ConsultaMedia) -> R<()> {
        for e in &q.expressoes {
            self.visit_media_expression(e)?;
        }
        Ok(())
    }

    /// `visitDocumentDirective`.
    fn visit_document_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Documento { funcoes, corpo } = no {
            for f in funcoes.iter() {
                self.visit_termo(f)?;
            }
            for r in corpo {
                self.visit_no(r)?;
            }
        }
        Ok(())
    }

    /// `visitSupportsDirective`.
    fn visit_supports_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Supports { condicao, corpo } = no {
            let c = condicao.as_mut().ok_or(Excecao("TypeError"))?;
            self.visit_supports_condition(c)?;
            for r in corpo {
                self.visit_no(r)?;
            }
        }
        Ok(())
    }

    /// O despacho das subclasses de `SupportsCondition`.
    fn visit_supports_condition(&mut self, c: &mut CondSupports) -> R<()> {
        match c {
            CondSupports::EmParenteses(dentro) => self.visit_supports_condition_in_parens(dentro),
            CondSupports::Negacao(c) => self.visit_supports_negation(c),
            CondSupports::Conjuncao(cs) => self.visit_supports_conjunction(cs),
            CondSupports::Disjuncao(cs) => self.visit_supports_disjunction(cs),
        }
    }

    /// `visitSupportsConditionInParens`.
    fn visit_supports_condition_in_parens(
        &mut self,
        dentro: &mut Option<Box<DentroDeParenteses>>,
    ) -> R<()> {
        match dentro.as_deref_mut().ok_or(Excecao("TypeError"))? {
            DentroDeParenteses::Declaracao(d) => self.visit_declaracao(d),
            DentroDeParenteses::Condicao(c) => self.visit_supports_condition(c),
        }
    }

    /// `visitSupportsNegation`.
    fn visit_supports_negation(&mut self, c: &mut CondSupports) -> R<()> {
        self.visit_supports_condition(c)
    }

    /// `visitSupportsConjunction`.
    fn visit_supports_conjunction(&mut self, cs: &mut [CondSupports]) -> R<()> {
        for c in cs {
            self.visit_supports_condition(c)?;
        }
        Ok(())
    }

    /// `visitSupportsDisjunction`.
    fn visit_supports_disjunction(&mut self, cs: &mut [CondSupports]) -> R<()> {
        for c in cs {
            self.visit_supports_condition(c)?;
        }
        Ok(())
    }

    /// `visitViewportDirective`.
    fn visit_viewport_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Viewport { declaracoes, .. } = no {
            self.visit_declaration_group(declaracoes)?;
        }
        Ok(())
    }

    /// `visitMediaDirective`.
    fn visit_media_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Media { consultas, regras } = no {
            for q in consultas.iter() {
                self.visit_media_query(q)?;
            }
            for r in regras {
                self.visit_no(r)?;
            }
        }
        Ok(())
    }

    /// `visitHostDirective`.
    fn visit_host_directive(&mut self, regras: &mut [No]) -> R<()> {
        for r in regras {
            self.visit_no(r)?;
        }
        Ok(())
    }

    /// `visitPageDirective`.
    fn visit_page_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Pagina { grupos, .. } = no {
            for g in grupos {
                if g.margem.is_some() {
                    self.visit_margin_group(g)?;
                } else {
                    self.visit_declaration_group(g)?;
                }
            }
        }
        Ok(())
    }

    /// `visitCharsetDirective`.
    fn visit_charset_directive(&mut self, _codificacao: &str) -> R<()> {
        Ok(())
    }

    /// `visitImportDirective`.
    fn visit_import_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Import { medias, .. } = no {
            for q in medias.iter() {
                self.visit_media_query(q)?;
            }
        }
        Ok(())
    }

    /// `visitKeyFrameDirective`.
    fn visit_key_frame_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Keyframes { nome, blocos, .. } = no {
            let nome = nome.as_ref().ok_or(Excecao("TypeError"))?;
            self.visit_identifier(nome)?;
            for b in blocos {
                self.visit_key_frame_block(b)?;
            }
        }
        Ok(())
    }

    /// `visitKeyFrameBlock`.
    fn visit_key_frame_block(&mut self, b: &mut BlocoKeyframe) -> R<()> {
        self.visit_expressions(&b.seletores)?;
        self.visit_declaration_group(&mut b.declaracoes)
    }

    /// `visitFontFaceDirective`.
    fn visit_font_face_directive(&mut self, g: &mut GrupoDeclaracoes) -> R<()> {
        self.visit_declaration_group(g)
    }

    /// `visitNamespaceDirective`.
    fn visit_namespace_directive(&mut self, _no: &mut No) -> R<()> {
        Ok(())
    }

    /// `visitVarDefinitionDirective`.
    fn visit_var_definition_directive(&mut self, d: &mut VarDef) -> R<()> {
        self.visit_var_definition(d)
    }

    /// `visitMixinRulesetDirective`.
    fn visit_mixin_ruleset_directive(&mut self, no: &mut No) -> R<()> {
        if let No::MixinRegras { regras, .. } = no {
            for r in regras {
                self.visit_no(r)?;
            }
        }
        Ok(())
    }

    /// `visitMixinDeclarationDirective`.
    fn visit_mixin_declaration_directive(&mut self, no: &mut No) -> R<()> {
        if let No::MixinDeclaracoes { declaracoes, .. } = no {
            self.visit_declaration_group(declaracoes)?;
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
        self.visit_selector_group(&mut r.grupo)?;
        self.visit_declaration_group(&mut r.declaracoes)
    }

    /// `visitDeclarationGroup`.
    fn visit_declaration_group(&mut self, g: &mut GrupoDeclaracoes) -> R<()> {
        for d in &mut g.declaracoes {
            self.visit_no(d)?;
        }
        Ok(())
    }

    /// `visitMarginGroup`.
    fn visit_margin_group(&mut self, g: &mut GrupoDeclaracoes) -> R<()> {
        self.visit_declaration_group(g)
    }

    /// `visitDeclaration`.
    fn visit_declaration(&mut self, d: &mut Declaracao) -> R<()> {
        if let Declaracao::Comum {
            propriedade,
            expressao,
            ..
        } = d
        {
            self.visit_identifier(propriedade)?;
            self.visit_expressions(expressao)?;
        }
        Ok(())
    }

    /// `visitVarDefinition`.
    fn visit_var_definition(&mut self, d: &mut VarDef) -> R<()> {
        let nome = d.nome.as_ref().ok_or(Excecao("TypeError"))?;
        self.visit_identifier(nome)?;
        if let Some(e) = &d.expressao {
            self.visit_expressions(e)?;
        }
        Ok(())
    }

    /// `visitIncludeMixinAtDeclaration`.
    fn visit_include_mixin_at_declaration(&mut self, i: &mut Include) -> R<()> {
        self.visit_include_directive(i)
    }

    /// `visitExtendDeclaration`.
    fn visit_extend_declaration(&mut self, seletores: &mut [std::rc::Rc<SeletorSimples>]) -> R<()> {
        for s in seletores.iter() {
            self.visit_simple(s)?;
        }
        Ok(())
    }

    /// `visitSelectorGroup`.
    fn visit_selector_group(&mut self, g: &mut GrupoSeletores) -> R<()> {
        for s in &g.seletores {
            self.visit_selector(s)?;
        }
        Ok(())
    }

    /// `visitSelector`.
    fn visit_selector(&mut self, s: &Seletor) -> R<()> {
        for seq in &s.sequencias {
            self.visit_simple_selector_sequence(seq)?;
        }
        Ok(())
    }

    /// `visitSimpleSelectorSequence`.
    fn visit_simple_selector_sequence(&mut self, seq: &Sequencia) -> R<()> {
        self.visit_simple(&seq.seletor)
    }

    /// `SimpleSelector.visit`: o despacho das subclasses.
    fn visit_simple(&mut self, s: &SeletorSimples) -> R<()> {
        match &s.tipo {
            TipoSimples::Elemento => self.visit_element_selector(s),
            TipoSimples::Namespace { .. } => self.visit_namespace_selector(s),
            TipoSimples::Atributo { .. } => self.visit_attribute_selector(s),
            TipoSimples::Id => self.visit_id_selector(s),
            TipoSimples::Classe => self.visit_class_selector(s),
            TipoSimples::PseudoClasse => self.visit_pseudo_class_selector(s),
            TipoSimples::PseudoElemento { .. } => self.visit_pseudo_element_selector(s),
            TipoSimples::PseudoClasseFuncao(_) => self.visit_pseudo_class_function_selector(s),
            TipoSimples::PseudoElementoFuncao(_) => self.visit_pseudo_element_function_selector(s),
            TipoSimples::Negacao(_) => self.visit_negation_selector(s),
        }
    }

    /// `visitSimpleSelector`: visita o `_name`.
    fn visit_simple_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_nome_simples(&s.nome)
    }

    /// `(_name as TreeNode).visit(this)`.
    fn visit_nome_simples(&mut self, n: &NomeSimples) -> R<()> {
        match n {
            NomeSimples::Ident(i) => self.visit_identifier(i),
            NomeSimples::Seletor(s) => self.visit_simple(s),
            NomeSimples::Curinga | NomeSimples::Este | NomeSimples::Negacao => Ok(()),
        }
    }

    /// `visitNamespaceSelector`.
    fn visit_namespace_selector(&mut self, s: &SeletorSimples) -> R<()> {
        if let TipoSimples::Namespace {
            namespace: Some(ns),
        } = &s.tipo
        {
            self.visit_nome_simples(ns)?;
        }
        self.visit_nome_simples(&s.nome)
    }

    /// `visitElementSelector`.
    fn visit_element_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitAttributeSelector`.
    fn visit_attribute_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitIdSelector`.
    fn visit_id_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitClassSelector`.
    fn visit_class_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitPseudoClassSelector`.
    fn visit_pseudo_class_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitPseudoElementSelector`.
    fn visit_pseudo_element_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitPseudoClassFunctionSelector`.
    fn visit_pseudo_class_function_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitPseudoElementFunctionSelector`.
    fn visit_pseudo_element_function_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitNegationSelector`.
    fn visit_negation_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.visit_simple_selector(s)
    }

    /// `visitSelectorExpression`.
    fn visit_selector_expression(&mut self, exprs: &[Expr]) -> R<()> {
        for e in exprs {
            self.visit_expr(e)?;
        }
        Ok(())
    }

    /// `Expression.visit`: o despacho das expressões.
    fn visit_expr(&mut self, e: &Expr) -> R<()> {
        match e {
            Expr::Termo(t) => self.visit_termo(t),
            Expr::UnicodeRange { primeiro, segundo } => {
                self.visit_unicode_range_term(primeiro.as_deref(), segundo.as_deref())
            }
            Expr::VarUsage { nome, padroes } => self.visit_var_usage(nome, padroes),
            Expr::Barra => self.visit_operator_slash(),
            Expr::Virgula => self.visit_operator_comma(),
            Expr::Mais => self.visit_operator_plus(),
            Expr::Menos => self.visit_operator_minus(),
            Expr::Grupo(termos) => self.visit_group_term(termos),
        }
    }

    /// `LiteralTerm.visit` e subclasses.
    fn visit_termo(&mut self, t: &Termo) -> R<()> {
        match &t.tipo {
            TipoTermo::Literal => self.visit_literal_term(t),
            TipoTermo::Numero => self.visit_number_term(t),
            TipoTermo::Item => self.visit_item_term(t),
            TipoTermo::Unidade { .. } => self.visit_unit_term(t),
            TipoTermo::Porcentagem => self.visit_percentage_term(t),
            TipoTermo::Em => self.visit_em_term(t),
            TipoTermo::Ex => self.visit_ex_term(t),
            TipoTermo::Fracao => self.visit_fraction_term(t),
            TipoTermo::Uri => self.visit_uri_term(t),
            TipoTermo::Hex => self.visit_hex_color_term(t),
            TipoTermo::Ie8 => self.visit_ie8_term(t),
            TipoTermo::Funcao(_) => self.visit_function_term(t),
            TipoTermo::Calc(_) => self.visit_calc_term(t),
        }
    }

    /// `visitCalcTerm`.
    fn visit_calc_term(&mut self, t: &Termo) -> R<()> {
        self.visit_literal_term(t)?;
        if let TipoTermo::Calc(expr) = &t.tipo {
            self.visit_literal_term(expr)?;
        }
        Ok(())
    }

    /// `visitUnicodeRangeTerm`.
    fn visit_unicode_range_term(&mut self, _p: Option<&str>, _s: Option<&str>) -> R<()> {
        Ok(())
    }

    /// `visitLiteralTerm`.
    fn visit_literal_term(&mut self, _t: &Termo) -> R<()> {
        Ok(())
    }

    /// `visitHexColorTerm`.
    fn visit_hex_color_term(&mut self, _t: &Termo) -> R<()> {
        Ok(())
    }

    /// `visitNumberTerm`.
    fn visit_number_term(&mut self, _t: &Termo) -> R<()> {
        Ok(())
    }

    /// `visitUnitTerm` (e as subclasses, que o oficial manda para ele).
    fn visit_unit_term(&mut self, _t: &Termo) -> R<()> {
        Ok(())
    }

    /// `visitPercentageTerm`.
    fn visit_percentage_term(&mut self, t: &Termo) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitEmTerm`.
    fn visit_em_term(&mut self, t: &Termo) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitExTerm`.
    fn visit_ex_term(&mut self, t: &Termo) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitFractionTerm`.
    fn visit_fraction_term(&mut self, t: &Termo) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitUriTerm`.
    fn visit_uri_term(&mut self, t: &Termo) -> R<()> {
        self.visit_literal_term(t)
    }

    /// `visitFunctionTerm`.
    fn visit_function_term(&mut self, t: &Termo) -> R<()> {
        self.visit_literal_term(t)?;
        if let TipoTermo::Funcao(params) = &t.tipo {
            self.visit_expressions(params)?;
        }
        Ok(())
    }

    /// `visitGroupTerm`.
    fn visit_group_term(&mut self, termos: &[Termo]) -> R<()> {
        for t in termos {
            self.visit_termo(t)?;
        }
        Ok(())
    }

    /// `visitItemTerm`.
    fn visit_item_term(&mut self, t: &Termo) -> R<()> {
        self.visit_number_term(t)
    }

    /// `visitIE8Term`.
    fn visit_ie8_term(&mut self, _t: &Termo) -> R<()> {
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
    fn visit_var_usage(&mut self, _nome: &str, padroes: &[Expr]) -> R<()> {
        for e in padroes {
            self.visit_expr(e)?;
        }
        Ok(())
    }

    /// `visitExpressions`.
    fn visit_expressions(&mut self, e: &Expressoes) -> R<()> {
        for x in &e.expressoes {
            self.visit_expr(x)?;
        }
        Ok(())
    }

    /// `visitIdentifier`.
    fn visit_identifier(&mut self, _i: &Identificador) -> R<()> {
        Ok(())
    }
}
