//! Porte da `CssPrinter` de `csslib/src/css_printer.dart` (csslib 1.0.2),
//! no modo compacto (`visitTree(tree, pretty: false)`), que é o que o shim
//! usa. No compacto o `_isTesting` vale: cores viram nome
//! (`hexToColorName`) e o `@import` sai com `url(…)`.
use std::rc::Rc;

use super::Excecao;
use super::arvore::*;
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

    /// `toString`: o buffer sem espaço nas pontas.
    pub fn texto(&self) -> String {
        dart_trim(&self.buff).to_string()
    }

    /// `emitMediaQueries`.
    fn emit_media_queries(&mut self, consultas: &[ConsultaMedia]) -> R<()> {
        for (i, q) in consultas.iter().enumerate() {
            if i > 0 {
                self.emit(",");
            }
            self.visit_media_query(q)?;
        }
        Ok(())
    }

    /// `visitIncludeDirective(node, [topLevel])`.
    fn include(&mut self, i: &Include) {
        // `_emitLf()` no topo: nada no compacto.
        self.emit(&format!("@include {}", i.nome));
        self.emit_semicolon(true);
    }

    /// O `toString` dos seletores simples que a impressora emite inteiro.
    fn seletor_como_texto(s: &SeletorSimples) -> R<String> {
        Ok(match &s.tipo {
            // `ElementSelector.toString` => `name`.
            TipoSimples::Elemento => s.nome().to_string(),
            // `'$namespace|${nameAsSimpleSelector!.name}'`.
            TipoSimples::Namespace { namespace } => {
                let ns = match namespace {
                    None => "",
                    Some(NomeSimples::Curinga) => "*",
                    Some(n) => n.nome(),
                };
                format!("{ns}|{}", s.nome())
            }
            // `'[$name${matchOperator()}${valueToString()}]'`.
            TipoSimples::Atributo { op, valor } => {
                let operador = match *op {
                    tk::EQUALS => "=",
                    tk::INCLUDES => "~=",
                    tk::DASH_MATCH => "|=",
                    tk::PREFIX_MATCH => "^=",
                    tk::SUFFIX_MATCH => "$=",
                    tk::SUBSTRING_MATCH => "*=",
                    tk::NO_MATCH => "",
                    _ => "null",
                };
                let valor = match valor {
                    None => String::new(),
                    Some(ValorAtributo::Ident(i)) => i.como_texto().to_string(),
                    Some(ValorAtributo::Texto(t)) => format!("\"{t}\""),
                };
                format!("[{}{operador}{valor}]", s.nome())
            }
            // `'#$_name'`, `'.$_name'`: o `Identifier.toString`.
            TipoSimples::Id => format!("#{}", nome_como_texto(&s.nome)),
            TipoSimples::Classe => format!(".{}", nome_como_texto(&s.nome)),
            TipoSimples::PseudoClasse => format!(":{}", s.nome()),
            TipoSimples::PseudoElemento { legado } => {
                format!("{}{}", if *legado { ":" } else { "::" }, s.nome())
            }
            _ => return Err(Excecao("seletor sem toString")),
        })
    }
}

/// `_name.toString()` de `IdSelector`/`ClassSelector` (um `Identifier`).
fn nome_como_texto(n: &NomeSimples) -> &str {
    match n {
        NomeSimples::Ident(i) => i.como_texto(),
        outro => outro.nome(),
    }
}

impl Visitor for CssPrinter {
    fn visit_calc_term(&mut self, t: &Termo) -> R<()> {
        self.emit(&format!("{}(", t.texto));
        if let TipoTermo::Calc(expr) = &t.tipo {
            self.visit_termo(expr)?;
        }
        self.emit(")");
        Ok(())
    }

    fn visit_media_expression(&mut self, e: &ExprMedia) -> R<()> {
        self.emit(if e.e { " AND " } else { " " });
        self.emit(&format!("({}", e.recurso.nome));
        if !e.exprs.expressoes.is_empty() {
            self.emit(":");
            self.visit_expressions(&e.exprs)?;
        }
        self.emit(")");
        Ok(())
    }

    fn visit_media_query(&mut self, q: &ConsultaMedia) -> R<()> {
        let unario = if q.unario != -1 {
            let v = tk::id_to_value(tk::MEDIA_OPERATORS, q.unario).ok_or(Excecao("TypeError"))?;
            format!(" {}", v.to_uppercase())
        } else {
            String::new()
        };
        let tipo = match &q.tipo {
            Some(t) => format!(" {}", t.nome),
            None => String::new(),
        };
        self.emit(&format!("{unario}{tipo}"));
        for e in &q.expressoes {
            self.visit_media_expression(e)?;
        }
        Ok(())
    }

    fn visit_document_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Documento { funcoes, corpo } = no {
            self.emit("@-moz-document ");
            let primeira = funcoes.first().ok_or(Excecao("StateError"))?;
            self.visit_termo(primeira)?;
            for f in funcoes.iter().skip(1) {
                self.emit(",");
                self.visit_termo(f)?;
            }
            self.emit_l_brace();
            for r in corpo {
                self.visit_no(r)?;
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_supports_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Supports { condicao, corpo } = no {
            self.emit("@supports ");
            let c = condicao.as_mut().ok_or(Excecao("TypeError"))?;
            self.visit_supports_condition(c)?;
            self.emit_l_brace();
            for r in corpo {
                self.visit_no(r)?;
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_supports_condition_in_parens(
        &mut self,
        dentro: &mut Option<Box<DentroDeParenteses>>,
    ) -> R<()> {
        self.emit("(");
        match dentro.as_deref_mut().ok_or(Excecao("TypeError"))? {
            DentroDeParenteses::Declaracao(d) => self.visit_declaracao(d)?,
            DentroDeParenteses::Condicao(c) => self.visit_supports_condition(c)?,
        }
        self.emit(")");
        Ok(())
    }

    fn visit_supports_negation(&mut self, c: &mut CondSupports) -> R<()> {
        self.emit("not");
        self.visit_supports_condition(c)
    }

    fn visit_supports_conjunction(&mut self, cs: &mut [CondSupports]) -> R<()> {
        let (primeira, resto) = cs.split_first_mut().ok_or(Excecao("StateError"))?;
        self.visit_supports_condition(primeira)?;
        for c in resto {
            self.emit("and");
            self.visit_supports_condition(c)?;
        }
        Ok(())
    }

    fn visit_supports_disjunction(&mut self, cs: &mut [CondSupports]) -> R<()> {
        let (primeira, resto) = cs.split_first_mut().ok_or(Excecao("StateError"))?;
        self.visit_supports_condition(primeira)?;
        for c in resto {
            self.emit("or");
            self.visit_supports_condition(c)?;
        }
        Ok(())
    }

    fn visit_viewport_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Viewport { nome, declaracoes } = no {
            self.emit(&format!("@{nome}"));
            self.emit_l_brace();
            self.visit_declaration_group(declaracoes)?;
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_media_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Media { consultas, regras } = no {
            self.emit("@media");
            self.emit_media_queries(consultas)?;
            self.emit_l_brace();
            for r in regras {
                self.visit_no(r)?;
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_host_directive(&mut self, regras: &mut [No]) -> R<()> {
        self.emit("@host");
        self.emit_l_brace();
        for r in regras {
            self.visit_no(r)?;
        }
        self.emit_r_brace();
        Ok(())
    }

    fn visit_page_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Pagina {
            ident,
            pseudo,
            grupos,
        } = no
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
            for g in grupos {
                if g.margem.is_some() {
                    self.visit_margin_group(g)?;
                } else {
                    self.visit_declaration_group(g)?;
                }
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_charset_directive(&mut self, codificacao: &str) -> R<()> {
        self.emit(&format!("@charset \"{codificacao}\""));
        self.emit_semicolon(true);
        Ok(())
    }

    fn visit_import_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Import { import, medias } = no {
            // `_isTesting`: sempre como `url(…)`.
            self.emit(&format!("@import url({import})"));
            self.emit_media_queries(medias)?;
            self.emit_semicolon(true);
        }
        Ok(())
    }

    fn visit_key_frame_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Keyframes { tipo, nome, blocos } = no {
            // `keyFrameName`.
            let nome_diretiva = match *tipo {
                tk::DIRECTIVE_KEYFRAMES | tk::DIRECTIVE_MS_KEYFRAMES => "@keyframes",
                tk::DIRECTIVE_WEB_KIT_KEYFRAMES => "@-webkit-keyframes",
                tk::DIRECTIVE_MOZ_KEYFRAMES => "@-moz-keyframes",
                tk::DIRECTIVE_O_KEYFRAMES => "@-o-keyframes",
                _ => "null",
            };
            self.emit(&format!("{nome_diretiva} "));
            let nome = nome.as_ref().ok_or(Excecao("TypeError"))?;
            self.visit_identifier(nome)?;
            self.emit_l_brace();
            self.is_in_keyframes = true;
            for b in blocos {
                self.visit_key_frame_block(b)?;
            }
            self.is_in_keyframes = false;
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_font_face_directive(&mut self, g: &mut GrupoDeclaracoes) -> R<()> {
        self.emit("@font-face");
        self.emit_l_brace();
        self.visit_declaration_group(g)?;
        self.emit_r_brace();
        Ok(())
    }

    fn visit_key_frame_block(&mut self, b: &mut BlocoKeyframe) -> R<()> {
        self.visit_expressions(&b.seletores)?;
        self.emit_l_brace();
        self.visit_declaration_group(&mut b.declaracoes)?;
        self.emit_r_brace();
        Ok(())
    }

    fn visit_namespace_directive(&mut self, no: &mut No) -> R<()> {
        if let No::Namespace { prefixo, uri } = no {
            let uri = uri.as_deref().ok_or(Excecao("TypeError"))?;
            // `prefix`: com espaço depois, se houver.
            let prefixo = if prefixo.is_empty() {
                String::new()
            } else {
                format!("{prefixo} ")
            };
            // `isStartingQuote(_uri!)`: `'\'"'.contains(uri)`.
            if "'\"".contains(uri) {
                self.emit(&format!("@namespace {prefixo}\"{uri}\""));
            } else {
                self.emit(&format!("@namespace {prefixo}url({uri})"));
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

    fn visit_mixin_ruleset_directive(&mut self, no: &mut No) -> R<()> {
        if let No::MixinRegras { nome, regras } = no {
            self.emit(&format!("@mixin {nome} "));
            self.emit_l_brace();
            for r in regras {
                self.visit_no(r)?;
            }
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_mixin_declaration_directive(&mut self, no: &mut No) -> R<()> {
        if let No::MixinDeclaracoes { nome, declaracoes } = no {
            self.emit(&format!("@mixin {nome}"));
            self.emit_l_brace();
            self.visit_declaration_group(declaracoes)?;
            self.emit_r_brace();
        }
        Ok(())
    }

    fn visit_include_directive(&mut self, i: &mut Include) -> R<()> {
        self.include(i);
        Ok(())
    }

    fn visit_rule_set(&mut self, r: &mut RuleSet) -> R<()> {
        self.visit_selector_group(&mut r.grupo)?;
        self.emit_l_brace();
        self.visit_declaration_group(&mut r.declaracoes)?;
        self.emit_r_brace();
        Ok(())
    }

    fn visit_declaration_group(&mut self, g: &mut GrupoDeclaracoes) -> R<()> {
        let n = g.declaracoes.len();
        for (i, d) in g.declaracoes.iter_mut().enumerate() {
            self.visit_no(d)?;
            // Sem o último `;` no compacto.
            if i + 1 < n {
                self.emit_semicolon(false);
            }
        }
        Ok(())
    }

    fn visit_margin_group(&mut self, g: &mut GrupoDeclaracoes) -> R<()> {
        let nome = g
            .margem
            .and_then(|m| tk::id_to_value(tk::MARGIN_DIRECTIVES, m))
            .unwrap_or("null");
        self.emit(&format!("@{nome}"));
        self.emit_l_brace();
        self.visit_declaration_group(g)?;
        self.emit_r_brace();
        Ok(())
    }

    fn visit_declaration(&mut self, d: &mut Declaracao) -> R<()> {
        if let Declaracao::Comum {
            propriedade,
            expressao,
            importante,
            ie7,
        } = d
        {
            // `property`: com `*` no hack do IE7.
            let prop = if *ie7 {
                format!("*{}", propriedade.nome)
            } else {
                propriedade.nome.clone()
            };
            self.emit(&format!("{prop}:"));
            self.visit_expressions(expressao)?;
            if *importante {
                self.emit("!important");
            }
        }
        Ok(())
    }

    fn visit_var_definition(&mut self, d: &mut VarDef) -> R<()> {
        let nome = d.nome.as_ref().ok_or(Excecao("TypeError"))?;
        self.emit(&format!("var-{}: ", nome.nome));
        let e = d.expressao.as_ref().ok_or(Excecao("TypeError"))?;
        self.visit_expressions(e)
    }

    fn visit_include_mixin_at_declaration(&mut self, i: &mut Include) -> R<()> {
        // Sem quebra de linha: está num grupo de declarações.
        self.include(i);
        Ok(())
    }

    fn visit_extend_declaration(&mut self, seletores: &mut [Rc<SeletorSimples>]) -> R<()> {
        self.emit("@extend ");
        for s in seletores.iter() {
            self.visit_simple(s)?;
        }
        Ok(())
    }

    fn visit_selector_group(&mut self, g: &mut GrupoSeletores) -> R<()> {
        for (i, s) in g.seletores.iter().enumerate() {
            if i > 0 {
                self.emit(",");
            }
            self.visit_selector(s)?;
        }
        Ok(())
    }

    fn visit_simple_selector_sequence(&mut self, seq: &Sequencia) -> R<()> {
        self.emit(seq.combinador_como_texto());
        self.visit_simple(&seq.seletor)
    }

    fn visit_simple_selector(&mut self, s: &SeletorSimples) -> R<()> {
        self.emit(s.nome());
        Ok(())
    }

    fn visit_namespace_selector(&mut self, s: &SeletorSimples) -> R<()> {
        let t = Self::seletor_como_texto(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_element_selector(&mut self, s: &SeletorSimples) -> R<()> {
        let t = Self::seletor_como_texto(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_attribute_selector(&mut self, s: &SeletorSimples) -> R<()> {
        let t = Self::seletor_como_texto(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_id_selector(&mut self, s: &SeletorSimples) -> R<()> {
        let t = Self::seletor_como_texto(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_class_selector(&mut self, s: &SeletorSimples) -> R<()> {
        let t = Self::seletor_como_texto(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_pseudo_class_selector(&mut self, s: &SeletorSimples) -> R<()> {
        let t = Self::seletor_como_texto(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_pseudo_element_selector(&mut self, s: &SeletorSimples) -> R<()> {
        let t = Self::seletor_como_texto(s)?;
        self.emit(&t);
        Ok(())
    }

    fn visit_pseudo_class_function_selector(&mut self, s: &SeletorSimples) -> R<()> {
        if let TipoSimples::PseudoClasseFuncao(arg) = &s.tipo {
            self.emit(&format!(":{}(", s.nome()));
            match arg {
                ArgumentoPseudo::Seletor(sel) => self.visit_selector(sel)?,
                ArgumentoPseudo::Expressao(e) => self.visit_selector_expression(e)?,
            }
            self.emit(")");
        }
        Ok(())
    }

    fn visit_pseudo_element_function_selector(&mut self, s: &SeletorSimples) -> R<()> {
        if let TipoSimples::PseudoElementoFuncao(e) = &s.tipo {
            self.emit(&format!("::{}(", s.nome()));
            self.visit_selector_expression(e)?;
            self.emit(")");
        }
        Ok(())
    }

    fn visit_negation_selector(&mut self, s: &SeletorSimples) -> R<()> {
        if let TipoSimples::Negacao(arg) = &s.tipo {
            self.emit(":not(");
            let arg = arg.as_ref().ok_or(Excecao("TypeError"))?;
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

    fn visit_literal_term(&mut self, t: &Termo) -> R<()> {
        self.emit(&t.texto);
        Ok(())
    }

    fn visit_hex_color_term(&mut self, t: &Termo) -> R<()> {
        let nome = match t.valor {
            Valor::Int(v) => tk::hex_to_color_name(v),
            _ => None,
        };
        match nome {
            Some(n) => self.emit(n),
            None => self.emit(&format!("#{}", t.texto)),
        }
        Ok(())
    }

    fn visit_number_term(&mut self, t: &Termo) -> R<()> {
        self.visit_literal_term(t)
    }

    fn visit_unit_term(&mut self, t: &Termo) -> R<()> {
        // `UnitTerm.toString`: `'$text${unitToString()}'`.
        if let TipoTermo::Unidade { unidade, .. } = t.tipo {
            self.emit(&format!("{}{}", t.texto, tk::unit_to_string(unidade)));
        }
        Ok(())
    }

    fn visit_percentage_term(&mut self, t: &Termo) -> R<()> {
        self.emit(&format!("{}%", t.texto));
        Ok(())
    }

    fn visit_em_term(&mut self, t: &Termo) -> R<()> {
        self.emit(&format!("{}em", t.texto));
        Ok(())
    }

    fn visit_ex_term(&mut self, t: &Termo) -> R<()> {
        self.emit(&format!("{}ex", t.texto));
        Ok(())
    }

    fn visit_fraction_term(&mut self, t: &Termo) -> R<()> {
        self.emit(&format!("{}fr", t.texto));
        Ok(())
    }

    fn visit_uri_term(&mut self, t: &Termo) -> R<()> {
        self.emit(&format!("url(\"{}\")", t.texto));
        Ok(())
    }

    fn visit_function_term(&mut self, t: &Termo) -> R<()> {
        self.emit(&format!("{}(", t.texto));
        if let TipoTermo::Funcao(params) = &t.tipo {
            self.visit_expressions(params)?;
        }
        self.emit(")");
        Ok(())
    }

    fn visit_group_term(&mut self, termos: &[Termo]) -> R<()> {
        self.emit("(");
        for t in termos {
            self.visit_termo(t)?;
        }
        self.emit(")");
        Ok(())
    }

    fn visit_item_term(&mut self, t: &Termo) -> R<()> {
        self.emit(&format!("[{}]", t.texto));
        Ok(())
    }

    fn visit_ie8_term(&mut self, t: &Termo) -> R<()> {
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

    fn visit_var_usage(&mut self, nome: &str, padroes: &[Expr]) -> R<()> {
        self.emit(&format!("var({nome}"));
        if !padroes.is_empty() {
            self.emit(",");
            for p in padroes {
                self.emit(" ");
                self.visit_expr(p)?;
            }
        }
        self.emit(")");
        Ok(())
    }

    fn visit_expressions(&mut self, e: &Expressoes) -> R<()> {
        let operador = |x: &Expr| matches!(x, Expr::Virgula | Expr::Barra);
        let porcentagem = |x: &Expr| {
            matches!(
                x,
                Expr::Termo(Termo {
                    tipo: TipoTermo::Porcentagem,
                    ..
                })
            )
        };
        for (i, x) in e.expressoes.iter().enumerate() {
            // Espaço entre termos sem operador.
            if i > 0 && !operador(x) {
                let anterior = &e.expressoes[i - 1];
                if operador(anterior) {
                    // `_sp`: nada no compacto.
                } else if porcentagem(anterior) && porcentagem(x) && self.is_in_keyframes {
                    self.emit(",");
                } else {
                    self.emit(" ");
                }
            }
            self.visit_expr(x)?;
        }
        Ok(())
    }

    fn visit_identifier(&mut self, i: &Identificador) -> R<()> {
        self.emit(&i.nome);
        Ok(())
    }
}
