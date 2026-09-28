//! Porte do `_Parser` de `csslib/parser.dart` (csslib 1.0.2), com o
//! `ExpressionsProcessor` e o `_escapeString`.
//!
//! Cada função cita a oficial. O parser não para em erro: o oficial registra
//! a mensagem em `messages` e segue, e o shim só as mostra como aviso. As
//! mensagens em si não mudam a saída; o que muda é a contagem, que a
//! leitura antecipada de seletor aninhado (`_nestedSelector`) consulta — por
//! isso aqui se conta, sem guardar texto.
//!
//! O que no Dart lança exceção (`!` em nulo, `as` de tipo errado, índice
//! fora da lista, `int.parse` fora do intervalo) devolve [`Excecao`]: o
//! builder oficial falharia ali.
//!
//! Os estilos para Dart (`_styleForDart`, `DartStyleExpression`) nunca são
//! impressos nem visitados; deles só se reproduzem as exceções que a
//! construção lança (`exprs.expressions[0]` em lista vazia, `value as int?`,
//! `value as num`).
use super::Excecao;
use super::arvore::*;
use super::token_kind as tk;
use super::tokenizer::{Token, Tokenizer, TokenizerState};

type R<T> = Result<T, Excecao>;

/// `_legacyPseudoElements`.
const PSEUDO_ELEMENTOS_LEGADOS: [&str; 4] = ["after", "before", "first-letter", "first-line"];

/// O aninhamento (produções recursivas abertas: regra, termo, seletor,
/// seletor simples, condição do `@supports`) além do qual se devolve o
/// `StackOverflowError` do oficial. O parser do csslib é recursivo e a pilha
/// da VM acaba num ponto que depende da VM (isolate principal ou não, código
/// já compilado ou não). Medido com o `shimShadowCss` no isolate principal:
/// até 1500 níveis de qualquer forma passa; `:host(` aninhado (duas
/// produções por nível) estoura a partir de 2000, `:not(` a partir de 4000,
/// e parênteses, blocos, `@media` e `@supports` entre 4000 e 8000. Com 4000
/// produções o porte concorda em todos esses pontos, exceto 4000–6000 níveis
/// das formas de uma produção por nível. O que importa sempre é a recursão
/// sem consumo — `@supports` no fim do arquivo recorre sem fim e o oficial
/// sempre estoura.
const LIMITE: usize = 4000;

/// `_Parser.MAX_UNICODE`.
const MAX_UNICODE: i64 = 0x10FFFF;

/// O `processTerm` devolve uma expressão ou, no `@nome` do Less, a lista
/// de expressões (`List<Expression>`).
enum TermoOuLista {
    Um(Expr),
    Lista(Vec<Expr>),
}

/// O `processVariableOrDirective` devolve uma definição ou o tipo da
/// diretiva.
enum VarOuDiretiva {
    VarDefinitionDirective(VarDef),
    VarDefinition,
    Tipo(i32),
}

/// `ParserState`.
struct ParserState {
    espiado: Token,
    anterior: Option<Token>,
    tokenizer: TokenizerState,
}

/// `_Parser`.
pub(crate) struct Parser<'a> {
    fonte: &'a [u16],
    tokenizer: Tokenizer<'a>,
    /// `_previousToken`.
    anterior: Option<Token>,
    /// `_peekToken`.
    espiado: Token,
    /// `messages.messages.length` do `Messages` corrente.
    pub mensagens: usize,
    /// Quantas produções recursivas estão abertas (ver [`LIMITE`]).
    profundidade: usize,
}

impl<'a> Parser<'a> {
    /// `_Parser(file, text)`.
    pub fn novo(fonte: &'a [u16]) -> R<Self> {
        let mut tokenizer = Tokenizer::new(fonte, true);
        let espiado = tokenizer.next(false)?;
        Ok(Parser {
            fonte,
            tokenizer,
            anterior: None,
            espiado,
            mensagens: 0,
            profundidade: 0,
        })
    }

    /// `parse`.
    pub fn parse(&mut self) -> R<Folha> {
        let mut producoes = Vec::new();
        while !self.maybe_eat(tk::END_OF_FILE)? && !self.peek_kind(tk::RBRACE) {
            match self.process_rule(None)? {
                Some(regra) => producoes.push(regra),
                None => break,
            }
        }
        self.check_end_of_file();
        Ok(Folha { topo: producoes })
    }

    /// `checkEndOfFile`.
    fn check_end_of_file(&mut self) {
        if !(self.peek_kind(tk::END_OF_FILE) || self.peek_kind(tk::INCOMPLETE_COMMENT)) {
            self.error();
        }
    }

    /// `isPrematureEndOfFile`.
    fn is_premature_end_of_file(&mut self) -> R<bool> {
        if self.maybe_eat(tk::END_OF_FILE)? {
            self.error();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // ---------------------------------------------------------------
    // Suporte básico
    // ---------------------------------------------------------------

    /// Entra numa produção recursiva: além de [`LIMITE`], o oficial já
    /// teria estourado a pilha.
    fn entrar(&mut self) -> R<()> {
        self.profundidade += 1;
        if self.profundidade > LIMITE {
            return Err(Excecao("StackOverflowError"));
        }
        Ok(())
    }

    /// `_peek`.
    fn peek(&self) -> i32 {
        self.espiado.kind
    }

    /// `_next`.
    fn next(&mut self) -> R<Token> {
        self.next_ur(false)
    }

    /// `_next(unicodeRange: …)`.
    fn next_ur(&mut self, unicode_range: bool) -> R<Token> {
        let novo = self.tokenizer.next(unicode_range)?;
        let proximo = std::mem::replace(&mut self.espiado, novo);
        self.anterior = Some(proximo.clone());
        Ok(proximo)
    }

    /// `_peekKind`.
    fn peek_kind(&self, kind: i32) -> bool {
        self.espiado.kind == kind
    }

    /// `_peekIdentifier`.
    fn peek_identifier(&self) -> bool {
        tk::is_identifier(self.espiado.kind)
    }

    /// `_mark`.
    fn mark(&self) -> ParserState {
        ParserState {
            espiado: self.espiado.clone(),
            anterior: self.anterior.clone(),
            tokenizer: self.tokenizer.mark(),
        }
    }

    /// `_restore`.
    fn restore(&mut self, m: ParserState) {
        self.tokenizer.restore(m.tokenizer);
        self.espiado = m.espiado;
        self.anterior = m.anterior;
    }

    /// `_maybeEat`.
    fn maybe_eat(&mut self, kind: i32) -> R<bool> {
        self.maybe_eat_ur(kind, false)
    }

    /// `_maybeEat(kind, unicodeRange: …)`.
    fn maybe_eat_ur(&mut self, kind: i32, unicode_range: bool) -> R<bool> {
        if self.espiado.kind == kind {
            let novo = self.tokenizer.next(unicode_range)?;
            self.anterior = Some(std::mem::replace(&mut self.espiado, novo));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// `_eat`.
    fn eat(&mut self, kind: i32) -> R<()> {
        self.eat_ur(kind, false)
    }

    /// `_eat(kind, unicodeRange: …)`.
    fn eat_ur(&mut self, kind: i32, unicode_range: bool) -> R<()> {
        if !self.maybe_eat_ur(kind, unicode_range)? {
            self.error_expected()?;
        }
        Ok(())
    }

    /// `_errorExpected`: consome o token e registra o erro (a montagem da
    /// mensagem, que pode falhar no `kindToString`, fica num `try`).
    fn error_expected(&mut self) -> R<()> {
        self.next()?;
        self.error();
        Ok(())
    }

    /// `_error`.
    fn error(&mut self) {
        self.mensagens += 1;
    }

    /// `_warning` (sem `warningsAsErrors`, o aviso também entra em
    /// `messages.messages`).
    fn warning(&mut self) {
        self.mensagens += 1;
    }

    /// `_peekToken.text`.
    fn texto_espiado(&self) -> String {
        self.espiado.texto(self.fonte)
    }

    /// `_previousToken!.end != _peekToken.start` e afins.
    fn anterior_fim(&self) -> Option<usize> {
        self.anterior.as_ref().map(|t| t.fim)
    }

    // ---------------------------------------------------------------
    // Produções do topo
    // ---------------------------------------------------------------

    /// `processMediaQueryList`.
    fn process_media_query_list(&mut self) -> R<Vec<ConsultaMedia>> {
        let mut consultas = Vec::new();
        while let Some(c) = self.process_media_query()? {
            consultas.push(c);
            if !self.maybe_eat(tk::COMMA)? {
                break;
            }
        }
        Ok(consultas)
    }

    /// `processMediaQuery`.
    fn process_media_query(&mut self) -> R<Option<ConsultaMedia>> {
        let op: Vec<u16> = self.texto_espiado().encode_utf16().collect();
        let unary_op = tk::match_media_operator(&op);
        if unary_op != -1 {
            self.next()?;
        }
        let mut tipo = None;
        if self.peek_identifier() {
            tipo = Some(self.identifier()?);
        }
        let mut exprs = Vec::new();
        loop {
            let and_op = !exprs.is_empty() || tipo.is_some();
            if and_op {
                let op: Vec<u16> = self.texto_espiado().encode_utf16().collect();
                if tk::match_media_operator(&op) != tk::MEDIA_OP_AND {
                    break;
                }
                self.next()?;
            }
            match self.process_media_expression(and_op)? {
                Some(e) => exprs.push(e),
                None => break,
            }
        }
        if unary_op != -1 || tipo.is_some() || !exprs.is_empty() {
            return Ok(Some(ConsultaMedia {
                unario: unary_op,
                tipo,
                expressoes: exprs,
            }));
        }
        Ok(None)
    }

    /// `processMediaExpression`.
    fn process_media_expression(&mut self, and_operator: bool) -> R<Option<ExprMedia>> {
        if self.maybe_eat(tk::LPAREN)? && self.peek_identifier() {
            let recurso = self.identifier()?;
            let exprs = if self.maybe_eat(tk::COLON)? {
                self.process_expr(false)?
            } else {
                Expressoes::default()
            };
            if self.maybe_eat(tk::RPAREN)? {
                return Ok(Some(ExprMedia {
                    e: and_operator,
                    recurso,
                    exprs,
                }));
            }
        }
        Ok(None)
    }

    /// `processDirective`.
    fn process_directive(&mut self) -> R<Option<No>> {
        let tok_id = match self.process_variable_or_directive(false)? {
            VarOuDiretiva::VarDefinitionDirective(d) => return Ok(Some(No::VarDefDiretiva(d))),
            // Só com `mixinParameter`.
            VarOuDiretiva::VarDefinition => return Err(Excecao("TypeError")),
            VarOuDiretiva::Tipo(t) => t,
        };
        match tok_id {
            tk::DIRECTIVE_IMPORT => {
                self.next()?;
                // `@import "uri"` ou `@import url("uri")`.
                let mut import_str = None;
                if self.peek_identifier() {
                    let nome = self.identifier()?;
                    if let Expr::Termo(Termo {
                        tipo: TipoTermo::Uri,
                        texto,
                        ..
                    }) = self.process_function(nome)?
                    {
                        import_str = Some(texto);
                    }
                } else {
                    import_str = Some(self.process_quoted_string(false)?);
                }
                let medias = self.process_media_query_list()?;
                if import_str.is_none() {
                    self.error();
                }
                let import = import_str.ok_or(Excecao("TypeError"))?;
                Ok(Some(No::Import {
                    import: super::tokenizer::dart_trim(&import).to_string(),
                    medias,
                }))
            }
            tk::DIRECTIVE_MEDIA => {
                self.next()?;
                let consultas = self.process_media_query_list()?;
                let mut regras = Vec::new();
                if self.maybe_eat(tk::LBRACE)? {
                    while !self.maybe_eat(tk::END_OF_FILE)? {
                        match self.process_rule(None)? {
                            Some(r) => regras.push(r),
                            None => break,
                        }
                    }
                    if !self.maybe_eat(tk::RBRACE)? {
                        self.error();
                    }
                } else {
                    self.error();
                }
                Ok(Some(No::Media { consultas, regras }))
            }
            tk::DIRECTIVE_HOST => {
                self.next()?;
                let mut regras = Vec::new();
                if self.maybe_eat(tk::LBRACE)? {
                    while !self.maybe_eat(tk::END_OF_FILE)? {
                        match self.process_rule(None)? {
                            Some(r) => regras.push(r),
                            None => break,
                        }
                    }
                    if !self.maybe_eat(tk::RBRACE)? {
                        self.error();
                    }
                } else {
                    self.error();
                }
                Ok(Some(No::Host { regras }))
            }
            tk::DIRECTIVE_PAGE => {
                // @page S* IDENT? pseudo_page? S* '{' … '}'
                self.next()?;
                let mut nome = None;
                if self.peek_identifier() {
                    nome = Some(self.identifier()?);
                }
                let mut pseudo_page = None;
                if self.maybe_eat(tk::COLON)? && self.peek_identifier() {
                    pseudo_page = Some(self.identifier()?);
                }
                let pseudo = pseudo_page.map(|p| p.nome).unwrap_or_default();
                let ident = nome.map(|n| n.nome).unwrap_or_default();
                let grupos = self.process_margins_declarations()?;
                Ok(Some(No::Pagina {
                    ident,
                    pseudo,
                    grupos,
                }))
            }
            tk::DIRECTIVE_CHARSET => {
                // @charset S* STRING S* ';'
                self.next()?;
                let codificacao = self.process_quoted_string(false)?;
                Ok(Some(No::Charset(codificacao)))
            }
            tk::DIRECTIVE_KEYFRAMES
            | tk::DIRECTIVE_WEB_KIT_KEYFRAMES
            | tk::DIRECTIVE_MOZ_KEYFRAMES
            | tk::DIRECTIVE_O_KEYFRAMES
            | tk::DIRECTIVE_MS_KEYFRAMES => {
                // @[browser]? keyframes [IDENT|STRING] '{' keyframes-blocks '}'
                self.next()?;
                let mut nome = None;
                if self.peek_identifier() {
                    nome = Some(self.identifier()?);
                }
                self.eat(tk::LBRACE)?;
                let mut blocos = Vec::new();
                loop {
                    let mut seletores = Expressoes::default();
                    loop {
                        // `processTerm() as Expression`: nulo ou lista lança.
                        match self.process_term(false)? {
                            Some(TermoOuLista::Um(e)) => seletores.expressoes.push(e),
                            _ => return Err(Excecao("TypeError")),
                        }
                        if !self.maybe_eat(tk::COMMA)? {
                            break;
                        }
                    }
                    let declaracoes = self.process_declarations(true)?;
                    blocos.push(BlocoKeyframe {
                        seletores,
                        declaracoes,
                    });
                    if self.maybe_eat(tk::RBRACE)? || self.is_premature_end_of_file()? {
                        break;
                    }
                }
                Ok(Some(No::Keyframes {
                    tipo: tok_id,
                    nome,
                    blocos,
                }))
            }
            tk::DIRECTIVE_FONTFACE => {
                self.next()?;
                Ok(Some(No::FontFace(self.process_declarations(true)?)))
            }
            tk::DIRECTIVE_STYLET => {
                // @stylet IDENT '{' ruleset '}'
                self.next()?;
                if self.peek_identifier() {
                    self.identifier()?;
                }
                self.eat(tk::LBRACE)?;
                while !self.maybe_eat(tk::END_OF_FILE)? {
                    if self.process_rule(None)?.is_none() {
                        break;
                    }
                }
                self.eat(tk::RBRACE)?;
                // `StyletDirective(name as String, …)`: o nome é
                // `Identifier` ou nulo, e o `as String` sempre lança.
                Err(Excecao("TypeError"))
            }
            tk::DIRECTIVE_NAMESPACE => {
                // @namespace S* [namespace_prefix S*]? [STRING|URI] S* ';' S*
                self.next()?;
                let mut prefixo = None;
                if self.peek_identifier() {
                    prefixo = Some(self.identifier()?);
                }
                let mut uri = None;
                if self.peek_identifier() {
                    let nome = self.identifier()?;
                    if let Expr::Termo(Termo {
                        tipo: TipoTermo::Uri,
                        texto,
                        ..
                    }) = self.process_function(nome)?
                    {
                        uri = Some(texto);
                    }
                } else if prefixo.as_ref().is_some_and(|p| p.nome == "url") {
                    let nome = prefixo.clone().ok_or(Excecao("TypeError"))?;
                    if let Expr::Termo(Termo {
                        tipo: TipoTermo::Uri,
                        texto,
                        ..
                    }) = self.process_function(nome)?
                    {
                        // @namespace url("");
                        uri = Some(texto);
                        prefixo = None;
                    }
                } else {
                    uri = Some(self.process_quoted_string(false)?);
                }
                Ok(Some(No::Namespace {
                    prefixo: prefixo.map(|p| p.nome).unwrap_or_default(),
                    uri,
                }))
            }
            tk::DIRECTIVE_MIXIN => self.process_mixin(),
            tk::DIRECTIVE_INCLUDE => Ok(Some(No::Include(self.process_include(true)?))),
            tk::DIRECTIVE_CONTENT => {
                // `@content not implemented.`
                self.warning();
                Ok(None)
            }
            tk::DIRECTIVE_MOZ_DOCUMENT => Ok(Some(self.process_document_directive()?)),
            tk::DIRECTIVE_SUPPORTS => Ok(Some(self.process_supports_directive()?)),
            tk::DIRECTIVE_VIEWPORT | tk::DIRECTIVE_MS_VIEWPORT => {
                Ok(Some(self.process_viewport_directive()?))
            }
            _ => Ok(None),
        }
    }

    /// `processMixin`.
    fn process_mixin(&mut self) -> R<Option<No>> {
        self.next()?;
        let nome = self.identifier()?;
        // Os parâmetros (`definedArgs`) nem o visitante nem a impressora
        // leem; o que importa é o que o laço consome.
        if self.maybe_eat(tk::LPAREN)? {
            let mut must_have_param = false;
            let mut keep_going = true;
            while keep_going {
                let antes = (
                    self.espiado.ini,
                    self.espiado.kind,
                    self.tokenizer.posicao(),
                );
                let var_def = self.process_variable_or_directive(true)?;
                let e_param = matches!(
                    var_def,
                    VarOuDiretiva::VarDefinitionDirective(_) | VarOuDiretiva::VarDefinition
                );
                if !e_param && must_have_param {
                    self.warning();
                    keep_going = false;
                }
                if self.maybe_eat(tk::COMMA)? {
                    must_have_param = true;
                    continue;
                }
                keep_going = !self.maybe_eat(tk::RPAREN)?;
                // O oficial não termina quando nada mais é consumido (por
                // exemplo `@mixin m(` no fim do arquivo): o builder trava.
                if keep_going
                    && antes
                        == (
                            self.espiado.ini,
                            self.espiado.kind,
                            self.tokenizer.posicao(),
                        )
                {
                    return Err(Excecao("laço sem fim no processMixin"));
                }
            }
        }
        self.eat(tk::LBRACE)?;

        let mut producoes: Vec<No> = Vec::new();
        // `mixinDirective`; `regras` quando o laço sai guardando o
        // `MixinRulesetDirective(productions)` — a mesma lista que a
        // conferência depois do laço usa.
        let mut mixin: Option<No> = None;
        let mut regras = false;
        while !self.maybe_eat(tk::END_OF_FILE)? {
            if let Some(diretiva) = self.process_directive()? {
                producoes.push(diretiva);
                continue;
            }
            let mut grupo = self.process_declarations(false)?;
            let alguma_declaracao = grupo.declaracoes.iter().any(|d| {
                matches!(
                    d,
                    No::Declaracao(
                        Declaracao::Comum { .. } | Declaracao::VarDef(_) | Declaracao::Extend(_)
                    )
                )
            });
            if alguma_declaracao {
                // Com declarações, o mixin é de declarações: os `@include`
                // de antes entram nelas, o resto se perde com aviso.
                let mut novas = Vec::new();
                for include in std::mem::take(&mut producoes) {
                    match include {
                        No::Include(i) => novas.push(No::Declaracao(Declaracao::Include(i))),
                        _ => self.warning(),
                    }
                }
                novas.append(&mut grupo.declaracoes);
                grupo.declaracoes = novas;
            } else {
                // Só `@include`: vira lista de produções.
                for d in std::mem::take(&mut grupo.declaracoes) {
                    producoes.push(match d {
                        No::Declaracao(Declaracao::Include(i)) => No::Include(i),
                        outro => outro,
                    });
                }
            }
            if !grupo.declaracoes.is_empty() {
                if producoes.is_empty() {
                    mixin = Some(No::MixinDeclaracoes {
                        nome: nome.nome.clone(),
                        declaracoes: grupo,
                    });
                    break;
                }
                for d in std::mem::take(&mut grupo.declaracoes) {
                    producoes.push(match d {
                        No::Declaracao(Declaracao::Include(i)) => No::Include(i),
                        outro => outro,
                    });
                }
            } else {
                regras = true;
                break;
            }
        }
        if regras || !producoes.is_empty() {
            mixin = Some(No::MixinRegras {
                nome: nome.nome.clone(),
                regras: producoes,
            });
        }
        self.eat(tk::RBRACE)?;
        Ok(mixin)
    }

    /// `processVariableOrDirective`.
    fn process_variable_or_directive(&mut self, mixin_parameter: bool) -> R<VarOuDiretiva> {
        let mut tok_id = self.peek();
        // `@ nome`, com espaço entre a arroba e o nome.
        if tok_id == tk::AT {
            self.next()?;
            tok_id = self.peek();
            if self.peek_identifier() {
                let diretiva: Vec<u16> = self.texto_espiado().encode_utf16().collect();
                tok_id = tk::match_directives(&diretiva);
                if tok_id == -1 {
                    tok_id = tk::match_margin_directives(&diretiva);
                }
            }
            if tok_id == -1 {
                // Less (`lessSupport` vale por omissão):
                //    @name: value;  =>  var-name: value;  (VarDefinition)
                let mut nome = None;
                if self.peek_identifier() {
                    nome = Some(self.identifier()?);
                }
                let mut exprs = None;
                if mixin_parameter && self.maybe_eat(tk::COLON)? {
                    exprs = Some(self.process_expr(false)?);
                } else if !mixin_parameter {
                    self.eat(tk::COLON)?;
                    exprs = Some(self.process_expr(false)?);
                }
                return Ok(VarOuDiretiva::VarDefinitionDirective(VarDef {
                    nome,
                    expressao: exprs,
                }));
            }
        } else if mixin_parameter && self.espiado.kind == tk::VAR_DEFINITION {
            self.next()?;
            if self.peek_identifier() {
                self.identifier()?;
            }
            if self.maybe_eat(tk::COLON)? {
                self.process_expr(false)?;
            }
            return Ok(VarOuDiretiva::VarDefinition);
        }
        Ok(VarOuDiretiva::Tipo(tok_id))
    }

    /// `processInclude`.
    fn process_include(&mut self, eat_semi_colon: bool) -> R<Include> {
        // @include IDENT [(args,...)];
        self.next()?;
        let mut nome = None;
        if self.peek_identifier() {
            nome = Some(self.identifier()?);
        }
        let mut params: Vec<Vec<Expr>> = Vec::new();
        if self.maybe_eat(tk::LPAREN)? {
            let mut termos: Vec<Expr> = Vec::new();
            let mut keep_going = true;
            while keep_going {
                let Some(expr) = self.process_term(false)? else {
                    break;
                };
                // `(expr is List ? expr[0] : expr) as Expression`.
                termos.push(match expr {
                    TermoOuLista::Um(e) => e,
                    TermoOuLista::Lista(mut l) => {
                        if l.is_empty() {
                            return Err(Excecao("RangeError"));
                        }
                        l.swap_remove(0)
                    }
                });
                keep_going = !self.peek_kind(tk::RPAREN);
                if keep_going && self.maybe_eat(tk::COMMA)? {
                    params.push(std::mem::take(&mut termos));
                }
            }
            params.push(termos);
            self.maybe_eat(tk::RPAREN)?;
        }
        if eat_semi_colon {
            self.eat(tk::SEMICOLON)?;
        }
        let nome = nome.ok_or(Excecao("TypeError"))?;
        Ok(Include {
            nome: nome.nome,
            args: params,
        })
    }

    /// `processDocumentDirective`.
    fn process_document_directive(&mut self) -> R<No> {
        self.next()?; // '@-moz-document'
        let mut funcoes = Vec::new();
        loop {
            // IDENT '('
            let ident = self.identifier()?;
            self.eat(tk::LPAREN)?;
            let funcao = if ident.nome == "url-prefix" || ident.nome == "domain" {
                // Aceita o argumento sem aspas e as repõe na saída.
                let valor = self.process_quoted_string(true)?;
                let argumento = if valor.is_empty() {
                    String::new()
                } else {
                    format!("\"{valor}\"")
                };
                self.eat(tk::RPAREN)?;
                let argumentos = Expressoes {
                    expressoes: vec![Expr::Termo(Termo::novo(
                        TipoTermo::Literal,
                        Valor::Texto(argumento.clone()),
                        argumento,
                    ))],
                };
                Termo::novo(
                    TipoTermo::Funcao(argumentos),
                    Valor::Texto(ident.nome.clone()),
                    ident.nome.clone(),
                )
            } else {
                // `processFunction(ident) as LiteralTerm`.
                match self.process_function(ident)? {
                    Expr::Termo(t) => t,
                    _ => return Err(Excecao("TypeError")),
                }
            };
            funcoes.push(funcao);
            if !self.maybe_eat(tk::COMMA)? {
                break;
            }
        }
        self.eat(tk::LBRACE)?;
        let corpo = self.process_group_rule_body()?;
        self.eat(tk::RBRACE)?;
        Ok(No::Documento { funcoes, corpo })
    }

    /// `processSupportsDirective`.
    fn process_supports_directive(&mut self) -> R<No> {
        self.next()?; // '@supports'
        let condicao = self.process_supports_condition()?;
        self.eat(tk::LBRACE)?;
        let corpo = self.process_group_rule_body()?;
        self.eat(tk::RBRACE)?;
        Ok(No::Supports { condicao, corpo })
    }

    /// `processSupportsCondition`.
    fn process_supports_condition(&mut self) -> R<Option<CondSupports>> {
        self.entrar()?;
        let r = self.process_supports_condition_();
        self.profundidade -= 1;
        r
    }

    /// O corpo de [`Self::process_supports_condition`].
    fn process_supports_condition_(&mut self) -> R<Option<CondSupports>> {
        if self.peek_kind(tk::IDENTIFIER) {
            return self.process_supports_negation();
        }
        // 0: nenhuma; 1: conjunção; 2: disjunção.
        let mut conditions = Vec::new();
        let mut clause_type = 0;
        loop {
            conditions.push(self.process_supports_condition_in_parens()?);
            let texto = self.texto_espiado().to_lowercase();
            let tipo = if texto == "and" {
                1
            } else if texto == "or" {
                2
            } else {
                break;
            };
            if clause_type == 0 {
                clause_type = tipo;
            } else if clause_type != tipo {
                self.error();
                break;
            }
            self.next()?;
        }
        Ok(Some(match clause_type {
            1 => CondSupports::Conjuncao(conditions),
            2 => CondSupports::Disjuncao(conditions),
            _ => conditions.swap_remove(0),
        }))
    }

    /// `processSupportsNegation`.
    fn process_supports_negation(&mut self) -> R<Option<CondSupports>> {
        if self.texto_espiado().to_lowercase() != "not" {
            return Ok(None);
        }
        self.next()?; // 'not'
        let condicao = self.process_supports_condition_in_parens()?;
        Ok(Some(CondSupports::Negacao(Box::new(condicao))))
    }

    /// `processSupportsConditionInParens`.
    fn process_supports_condition_in_parens(&mut self) -> R<CondSupports> {
        self.eat(tk::LPAREN)?;
        if let Some(condicao) = self.process_supports_condition()? {
            self.eat(tk::RPAREN)?;
            return Ok(CondSupports::EmParenteses(Some(Box::new(
                DentroDeParenteses::Condicao(condicao),
            ))));
        }
        let declaracao = self.process_declaration()?;
        self.eat(tk::RPAREN)?;
        Ok(CondSupports::EmParenteses(
            declaracao.map(|d| Box::new(DentroDeParenteses::Declaracao(d))),
        ))
    }

    /// `processViewportDirective`.
    fn process_viewport_directive(&mut self) -> R<No> {
        let nome = self.next()?.texto(self.fonte);
        let declaracoes = self.process_declarations(true)?;
        Ok(No::Viewport { nome, declaracoes })
    }

    /// `processRule`.
    fn process_rule(&mut self, grupo: Option<GrupoSeletores>) -> R<Option<No>> {
        self.entrar()?;
        let r = self.process_rule_(grupo);
        self.profundidade -= 1;
        r
    }

    /// O corpo de [`Self::process_rule`].
    fn process_rule_(&mut self, grupo: Option<GrupoSeletores>) -> R<Option<No>> {
        let grupo = match grupo {
            Some(g) => Some(g),
            None => {
                if let Some(diretiva) = self.process_directive()? {
                    self.maybe_eat(tk::SEMICOLON)?;
                    return Ok(Some(diretiva));
                }
                self.process_selector_group()?
            }
        };
        match grupo {
            Some(grupo) => {
                let declaracoes = self.process_declarations(true)?;
                Ok(Some(No::Regra(RuleSet { grupo, declaracoes })))
            }
            None => Ok(None),
        }
    }

    /// `processGroupRuleBody`.
    fn process_group_rule_body(&mut self) -> R<Vec<No>> {
        let mut nos = Vec::new();
        while !(self.peek_kind(tk::RBRACE) || self.peek_kind(tk::END_OF_FILE)) {
            match self.process_rule(None)? {
                Some(r) => nos.push(r),
                None => break,
            }
        }
        Ok(nos)
    }

    /// `_nestedSelector`: lê um grupo de seletores adiante; se não é seletor
    /// aninhado (sem `{` depois, ou com alguma mensagem), volta atrás.
    fn nested_selector(&mut self) -> R<Option<GrupoSeletores>> {
        let antigas = self.mensagens;
        self.mensagens = 0;
        let marca = self.mark();
        let grupo = self.process_selector_group()?;
        let aninhado = grupo.is_some() && self.peek_kind(tk::LBRACE) && self.mensagens == 0;
        if !aninhado {
            self.restore(marca);
            self.mensagens = antigas;
            Ok(None)
        } else {
            self.mensagens += antigas;
            Ok(grupo)
        }
    }

    /// `processDeclarations`.
    fn process_declarations(&mut self, check_brace: bool) -> R<GrupoDeclaracoes> {
        if check_brace {
            self.eat(tk::LBRACE)?;
        }
        let mut decls = Vec::new();
        loop {
            let mut grupo = self.nested_selector()?;
            while let Some(g) = grupo {
                // Seletor aninhado: uma regra.
                let regra = self.process_rule(Some(g))?.ok_or(Excecao("TypeError"))?;
                decls.push(regra);
                grupo = self.nested_selector()?;
            }
            if let Some(decl) = self.process_declaration()? {
                decls.push(No::Declaracao(decl));
            }
            if !self.maybe_eat(tk::SEMICOLON)? {
                break;
            }
        }
        if check_brace {
            self.eat(tk::RBRACE)?;
        }
        Ok(GrupoDeclaracoes {
            declaracoes: decls,
            margem: None,
        })
    }

    /// `processMarginsDeclarations`.
    fn process_margins_declarations(&mut self) -> R<Vec<GrupoDeclaracoes>> {
        let mut grupos = Vec::new();
        self.eat(tk::LBRACE)?;
        let mut decls = Vec::new();
        loop {
            let antes = (
                self.espiado.ini,
                self.espiado.kind,
                self.tokenizer.posicao(),
            );
            match self.peek() {
                tk::MARGIN_DIRECTIVE_TOPLEFTCORNER
                | tk::MARGIN_DIRECTIVE_TOPLEFT
                | tk::MARGIN_DIRECTIVE_TOPCENTER
                | tk::MARGIN_DIRECTIVE_TOPRIGHT
                | tk::MARGIN_DIRECTIVE_TOPRIGHTCORNER
                | tk::MARGIN_DIRECTIVE_BOTTOMLEFTCORNER
                | tk::MARGIN_DIRECTIVE_BOTTOMLEFT
                | tk::MARGIN_DIRECTIVE_BOTTOMCENTER
                | tk::MARGIN_DIRECTIVE_BOTTOMRIGHT
                | tk::MARGIN_DIRECTIVE_BOTTOMRIGHTCORNER
                | tk::MARGIN_DIRECTIVE_LEFTTOP
                | tk::MARGIN_DIRECTIVE_LEFTMIDDLE
                | tk::MARGIN_DIRECTIVE_LEFTBOTTOM
                | tk::MARGIN_DIRECTIVE_RIGHTTOP
                | tk::MARGIN_DIRECTIVE_RIGHTMIDDLE
                | tk::MARGIN_DIRECTIVE_RIGHTBOTTOM => {
                    // margin_sym S* '{' declaration [ ';' S* declaration? ]* '}' S*
                    let margem = self.peek();
                    self.next()?;
                    let grupo = self.process_declarations(true)?;
                    grupos.push(GrupoDeclaracoes {
                        declaracoes: grupo.declaracoes,
                        margem: Some(margem),
                    });
                }
                _ => {
                    if let Some(decl) = self.process_declaration()? {
                        decls.push(No::Declaracao(decl));
                    }
                    self.maybe_eat(tk::SEMICOLON)?;
                }
            }
            if self.maybe_eat(tk::RBRACE)? || self.is_premature_end_of_file()? {
                break;
            }
            // O oficial não termina quando nada é consumido (por exemplo um
            // `.x` dentro de `@page { }`): o builder trava.
            if antes
                == (
                    self.espiado.ini,
                    self.espiado.kind,
                    self.tokenizer.posicao(),
                )
            {
                return Err(Excecao("laço sem fim no processMarginsDeclarations"));
            }
        }
        if !decls.is_empty() {
            grupos.push(GrupoDeclaracoes {
                declaracoes: decls,
                margem: None,
            });
        }
        Ok(grupos)
    }

    /// `processSelectorGroup`.
    pub fn process_selector_group(&mut self) -> R<Option<GrupoSeletores>> {
        let mut seletores = Vec::new();
        self.tokenizer.in_selector = true;
        loop {
            if let Some(s) = self.process_selector()? {
                seletores.push(s);
            }
            if !self.maybe_eat(tk::COMMA)? {
                break;
            }
        }
        self.tokenizer.in_selector = false;
        if !seletores.is_empty() {
            return Ok(Some(GrupoSeletores { seletores }));
        }
        Ok(None)
    }

    /// `processSelector`.
    fn process_selector(&mut self) -> R<Option<Seletor>> {
        self.entrar()?;
        let r = self.process_selector_();
        self.profundidade -= 1;
        r
    }

    /// O corpo de [`Self::process_selector`].
    fn process_selector_(&mut self) -> R<Option<Seletor>> {
        let mut sequencias = Vec::new();
        while let Some(s) = self.simple_selector_sequence(sequencias.is_empty())? {
            sequencias.push(s);
        }
        if sequencias.is_empty() {
            return Ok(None);
        }
        Ok(Some(Seletor { sequencias }))
    }

    /// `processCompoundSelector`: um erro para cada combinador.
    fn process_compound_selector(&mut self) -> R<Option<Seletor>> {
        let seletor = self.process_selector()?;
        if let Some(s) = &seletor {
            for seq in &s.sequencias {
                if !seq.sem_combinador() {
                    self.error();
                }
            }
        }
        Ok(seletor)
    }

    /// `simpleSelectorSequence`.
    fn simple_selector_sequence(
        &mut self,
        force_combinator_none: bool,
    ) -> R<Option<std::rc::Rc<Sequencia>>> {
        let mut combinador = tk::COMBINATOR_NONE;
        let mut this_operator = false;
        match self.peek() {
            tk::PLUS => {
                self.eat(tk::PLUS)?;
                combinador = tk::COMBINATOR_PLUS;
            }
            tk::GREATER => {
                self.eat(tk::GREATER)?;
                combinador = tk::COMBINATOR_GREATER;
            }
            tk::TILDE => {
                self.eat(tk::TILDE)?;
                combinador = tk::COMBINATOR_TILDE;
            }
            tk::AMPERSAND => {
                self.eat(tk::AMPERSAND)?;
                this_operator = true;
            }
            _ => {}
        }
        // Espaço entre os tokens: descendente.
        if combinador == tk::COMBINATOR_NONE
            && !force_combinator_none
            && self.anterior_fim().is_some_and(|f| f != self.espiado.ini)
        {
            combinador = tk::COMBINATOR_DESCENDANT;
        }
        let mut seletor = if this_operator {
            Some(std::rc::Rc::new(SeletorSimples {
                tipo: TipoSimples::Elemento,
                nome: NomeSimples::Este,
                tem_span: true,
            }))
        } else {
            self.simple_selector()?
        };
        if seletor.is_none()
            && (combinador == tk::COMBINATOR_PLUS
                || combinador == tk::COMBINATOR_GREATER
                || combinador == tk::COMBINATOR_TILDE)
        {
            // `+ &`, `~ &`, `> &`: uma sequência sem nome para guardar o
            // combinador.
            seletor = Some(std::rc::Rc::new(SeletorSimples {
                tipo: TipoSimples::Elemento,
                nome: NomeSimples::Ident(Identificador {
                    nome: String::new(),
                    texto: None,
                }),
                tem_span: true,
            }));
        }
        Ok(seletor.map(|s| Sequencia::nova(s, combinador)))
    }

    /// `simpleSelector`.
    fn simple_selector(&mut self) -> R<Option<std::rc::Rc<SeletorSimples>>> {
        self.entrar()?;
        let r = self.simple_selector_();
        self.profundidade -= 1;
        r
    }

    /// O corpo de [`Self::simple_selector`].
    fn simple_selector_(&mut self) -> R<Option<std::rc::Rc<SeletorSimples>>> {
        let mut first: Option<NomeSimples> = None;
        match self.peek() {
            tk::ASTERISK => {
                self.next()?;
                first = Some(NomeSimples::Curinga);
            }
            tk::IDENTIFIER => first = Some(NomeSimples::Ident(self.identifier()?)),
            _ => {
                if tk::is_kind_identifier(self.peek()) {
                    first = Some(NomeSimples::Ident(self.identifier()?));
                } else if self.peek_kind(tk::SEMICOLON) {
                    // Não é seletor se achou `;`.
                    return Ok(None);
                }
            }
        }
        if self.maybe_eat(tk::NAMESPACE)? {
            let elemento = match self.peek() {
                tk::ASTERISK => {
                    self.next()?;
                    NomeSimples::Curinga
                }
                tk::IDENTIFIER => NomeSimples::Ident(self.identifier()?),
                // O erro monta a mensagem com `$_peekToken`, cujo
                // `kindToString` lança `StateError` num tipo sem nome; se
                // não, o `element!.span!` lança com o elemento nulo.
                k => {
                    return Err(Excecao(if tk::kind_to_string(k).is_none() {
                        "StateError"
                    } else {
                        "TypeError"
                    }));
                }
            };
            let elemento = std::rc::Rc::new(SeletorSimples {
                tipo: TipoSimples::Elemento,
                nome: elemento,
                tem_span: true,
            });
            return Ok(Some(std::rc::Rc::new(SeletorSimples {
                tipo: TipoSimples::Namespace { namespace: first },
                nome: NomeSimples::Seletor(elemento),
                tem_span: true,
            })));
        }
        if let Some(first) = first {
            return Ok(Some(std::rc::Rc::new(SeletorSimples {
                tipo: TipoSimples::Elemento,
                nome: first,
                tem_span: true,
            })));
        }
        // HASH | class | attrib | pseudo | negation
        self.simple_selector_tail()
    }

    /// `_anyWhiteSpaceBeforePeekToken`.
    fn any_white_space_before_peek_token(&self, kind: i32) -> bool {
        match &self.anterior {
            Some(t) if t.kind == kind => t.fim != self.espiado.ini,
            _ => false,
        }
    }

    /// `simpleSelectorTail`.
    fn simple_selector_tail(&mut self) -> R<Option<std::rc::Rc<SeletorSimples>>> {
        match self.peek() {
            tk::HASH => {
                self.eat(tk::HASH)?;
                if self.any_white_space_before_peek_token(tk::HASH) {
                    self.error();
                    return Ok(None);
                }
                let id = self.identifier()?;
                Ok(Some(simples(TipoSimples::Id, id)))
            }
            tk::DOT => {
                self.eat(tk::DOT)?;
                if self.any_white_space_before_peek_token(tk::DOT) {
                    self.error();
                    return Ok(None);
                }
                let classe = self.identifier()?;
                Ok(Some(simples(TipoSimples::Classe, classe)))
            }
            tk::COLON => self.process_pseudo_selector(),
            tk::LBRACK => self.process_attribute(),
            tk::DOUBLE => {
                self.error();
                self.next()?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// `processPseudoSelector`.
    fn process_pseudo_selector(&mut self) -> R<Option<std::rc::Rc<SeletorSimples>>> {
        self.eat(tk::COLON)?;
        let pseudo_element = self.maybe_eat(tk::COLON)?;
        if !self.peek_identifier() {
            return Ok(None);
        }
        let pseudo_name = self.identifier()?;
        let nome = pseudo_name.nome.to_lowercase();

        // Pseudo com função?
        if self.espiado.kind == tk::LPAREN {
            if !pseudo_element && nome == "not" {
                self.eat(tk::LPAREN)?;
                // Negation: ':NOT(' S* negation_arg S* ')'
                let neg_arg = self.simple_selector()?;
                self.eat(tk::RPAREN)?;
                return Ok(Some(std::rc::Rc::new(SeletorSimples {
                    tipo: TipoSimples::Negacao(neg_arg),
                    nome: NomeSimples::Negacao,
                    tem_span: true,
                })));
            } else if !pseudo_element
                && (nome == "host"
                    || nome == "host-context"
                    || nome == "global-context"
                    || nome == "-acx-global-context")
            {
                self.eat(tk::LPAREN)?;
                let Some(seletor) = self.process_compound_selector()? else {
                    self.error_expected()?;
                    return Ok(None);
                };
                self.eat(tk::RPAREN)?;
                return Ok(Some(simples(
                    TipoSimples::PseudoClasseFuncao(ArgumentoPseudo::Seletor(std::rc::Rc::new(
                        seletor,
                    ))),
                    pseudo_name,
                )));
            } else {
                // Em expressão de pseudo o `-` é operador; o modo muda antes
                // de comer o `(`, que já busca o token seguinte.
                self.tokenizer.in_selector_expression = true;
                self.eat(tk::LPAREN)?;
                let expr = self.process_selector_expression()?;
                self.tokenizer.in_selector_expression = false;
                return match expr {
                    Some(expr) => {
                        self.eat(tk::RPAREN)?;
                        Ok(Some(if pseudo_element {
                            simples(TipoSimples::PseudoElementoFuncao(expr), pseudo_name)
                        } else {
                            simples(
                                TipoSimples::PseudoClasseFuncao(ArgumentoPseudo::Expressao(expr)),
                                pseudo_name,
                            )
                        }))
                    }
                    None => {
                        self.error_expected()?;
                        Ok(None)
                    }
                };
            }
        }
        // Os pseudoelementos do CSS2.1 com `:` só.
        if pseudo_element || PSEUDO_ELEMENTOS_LEGADOS.contains(&nome.as_str()) {
            Ok(Some(simples(
                TipoSimples::PseudoElemento {
                    legado: !pseudo_element,
                },
                pseudo_name,
            )))
        } else {
            Ok(Some(simples(TipoSimples::PseudoClasse, pseudo_name)))
        }
    }

    /// `processSelectorExpression`: a `SelectorExpression`, ou `None` onde o
    /// oficial devolve um `LiteralTerm` (argumento entre aspas).
    fn process_selector_expression(&mut self) -> R<Option<Vec<Expr>>> {
        let mut expressoes = Vec::new();
        let mut term_token: Option<Token> = None;
        let mut valor: Option<Valor> = None;
        let mut keep_parsing = true;
        while keep_parsing {
            match self.peek() {
                tk::PLUS => {
                    term_token = Some(self.next()?);
                    expressoes.push(Expr::Mais);
                }
                tk::MINUS => {
                    term_token = Some(self.next()?);
                    expressoes.push(Expr::Menos);
                }
                tk::INTEGER => {
                    let t = self.next()?;
                    valor = Some(Valor::Int(int_parse(&t.texto(self.fonte))?));
                    term_token = Some(t);
                }
                tk::DOUBLE => {
                    let t = self.next()?;
                    valor = Some(Valor::Double(double_parse(&t.texto(self.fonte))?));
                    term_token = Some(t);
                }
                tk::SINGLE_QUOTE | tk::DOUBLE_QUOTE => {
                    self.process_quoted_string(false)?;
                    return Ok(None);
                }
                tk::IDENTIFIER => {
                    valor = Some(Valor::Ident(self.identifier()?));
                }
                _ => keep_parsing = false,
            }
            if keep_parsing && let Some(v) = valor.take() {
                let termo = self.process_dimension(term_token.as_ref(), v)?;
                expressoes.push(Expr::Termo(termo));
            }
        }
        Ok(Some(expressoes))
    }

    /// `processAttribute`.
    fn process_attribute(&mut self) -> R<Option<std::rc::Rc<SeletorSimples>>> {
        if !self.maybe_eat(tk::LBRACK)? {
            return Ok(None);
        }
        let attr_name = self.identifier()?;
        let op = match self.peek() {
            tk::EQUALS
            | tk::INCLUDES
            | tk::DASH_MATCH
            | tk::PREFIX_MATCH
            | tk::SUFFIX_MATCH
            | tk::SUBSTRING_MATCH => {
                let op = self.peek();
                self.next()?;
                op
            }
            _ => tk::NO_MATCH,
        };
        let mut valor = None;
        if op != tk::NO_MATCH {
            // Com operador, exige valor.
            valor = Some(if self.peek_identifier() {
                ValorAtributo::Ident(self.identifier()?)
            } else {
                ValorAtributo::Texto(self.process_quoted_string(false)?)
            });
        }
        self.eat(tk::RBRACK)?;
        Ok(Some(simples(
            TipoSimples::Atributo { op, valor },
            attr_name,
        )))
    }

    /// `processDeclaration`.
    fn process_declaration(&mut self) -> R<Option<Declaracao>> {
        // `*IDENT`: hack do IE7.
        let ie7 = self.peek_kind(tk::ASTERISK);
        if ie7 {
            self.next()?;
        }
        if tk::is_identifier(self.espiado.kind) {
            // IDENT ':' expr '!important'?
            let propriedade = self.identifier()?;
            let ie_filter_property = propriedade.nome.to_lowercase() == "filter";
            self.eat(tk::COLON)?;
            let exprs = self.process_expr(ie_filter_property)?;
            style_for_dart(&propriedade, &exprs)?;
            let importante = self.maybe_eat(tk::IMPORTANT)?;
            return Ok(Some(Declaracao::Comum {
                propriedade,
                expressao: exprs,
                importante,
                ie7,
            }));
        } else if self.espiado.kind == tk::VAR_DEFINITION {
            self.next()?;
            let mut nome = None;
            if self.peek_identifier() {
                nome = Some(self.identifier()?);
            }
            self.eat(tk::COLON)?;
            let exprs = self.process_expr(false)?;
            return Ok(Some(Declaracao::VarDef(VarDef {
                nome,
                expressao: Some(exprs),
            })));
        } else if self.espiado.kind == tk::DIRECTIVE_INCLUDE {
            // @include no meio das declarações.
            let include = self.process_include(false)?;
            return Ok(Some(Declaracao::Include(include)));
        } else if self.espiado.kind == tk::DIRECTIVE_EXTEND {
            let mut seletores = Vec::new();
            self.next()?;
            match self.simple_selector()? {
                None => self.warning(),
                Some(s) => seletores.push(s),
            }
            if self.peek_kind(tk::COLON) {
                match self.process_pseudo_selector()? {
                    Some(p) if p.e_pseudo_elemento() || p.e_pseudo_classe() => seletores.push(p),
                    _ => self.warning(),
                }
            }
            return Ok(Some(Declaracao::Extend(seletores)));
        }
        Ok(None)
    }

    /// `processExpr`.
    fn process_expr(&mut self, ie_filter: bool) -> R<Expressoes> {
        let mut expressoes = Expressoes::default();
        let mut keep_going = true;
        while keep_going {
            let Some(expr) = self.process_term(ie_filter)? else {
                break;
            };
            let mut op: Option<Expr> = None;
            let mut ie8 = false;
            match self.peek() {
                tk::SLASH => op = Some(Expr::Barra),
                tk::COMMA => op = Some(Expr::Virgula),
                tk::BACKSLASH => {
                    // `\9` no fim da expressão: IE8 ou anterior.
                    self.next()?;
                    if self.peek_kind(tk::INTEGER) {
                        let num = self.next()?;
                        let valor = int_parse(&num.texto(self.fonte))?;
                        if valor == 9 {
                            op = Some(Expr::Termo(Termo::novo(
                                TipoTermo::Ie8,
                                Valor::Texto("\\9".into()),
                                "\\9".into(),
                            )));
                            ie8 = true;
                        }
                    }
                }
                _ => {}
            }
            match expr {
                TermoOuLista::Lista(l) => expressoes.expressoes.extend(l),
                TermoOuLista::Um(e) => expressoes.expressoes.push(e),
            }
            if let Some(op) = op {
                expressoes.expressoes.push(op);
                if ie8 {
                    keep_going = false;
                } else {
                    self.next()?;
                }
            }
        }
        Ok(expressoes)
    }

    /// `processTerm`.
    fn process_term(&mut self, ie_filter: bool) -> R<Option<TermoOuLista>> {
        self.entrar()?;
        let r = self.process_term_(ie_filter);
        self.profundidade -= 1;
        r
    }

    /// O corpo de [`Self::process_term`].
    fn process_term_(&mut self, ie_filter: bool) -> R<Option<TermoOuLista>> {
        let start = self.espiado.ini;
        let um = |t: Termo| Ok(Some(TermoOuLista::Um(Expr::Termo(t))));
        match self.peek() {
            tk::HASH => {
                self.eat(tk::HASH)?;
                if !self.any_white_space_before_peek_token(tk::HASH) {
                    let mut hex_text = None;
                    if self.peek_kind(tk::INTEGER) {
                        let hex_text1 = self.texto_espiado();
                        self.next()?;
                        // Junta o identificador só se não houver espaço.
                        if self.peek_identifier() && self.anterior_fim() == Some(self.espiado.ini) {
                            let resto = self.identifier()?.nome;
                            hex_text = Some(format!("{hex_text1}{resto}"));
                        } else {
                            hex_text = Some(hex_text1);
                        }
                    } else if self.peek_identifier() {
                        hex_text = Some(self.identifier()?.nome);
                    }
                    if let Some(h) = hex_text {
                        return um(self.parse_hex(h));
                    }
                }
                // `#<espaço>número`: `(processTerm() as LiteralTerm).text`.
                let texto = match self.process_term(false)? {
                    Some(TermoOuLista::Um(Expr::Termo(t))) => t.texto,
                    _ => return Err(Excecao("TypeError")),
                };
                um(self.parse_hex(format!(" {texto}")))
            }
            tk::INTEGER => {
                let t = self.next()?;
                let valor = int_parse(&t.texto(self.fonte))?;
                um(self.process_dimension(Some(&t), Valor::Int(valor))?)
            }
            tk::DOUBLE => {
                let t = self.next()?;
                let valor = double_parse(&t.texto(self.fonte))?;
                um(self.process_dimension(Some(&t), Valor::Double(valor))?)
            }
            tk::SINGLE_QUOTE => {
                let valor = self.process_quoted_string(false)?;
                let valor = format!("'{}'", escape_string(&valor, true));
                um(Termo::novo(
                    TipoTermo::Literal,
                    Valor::Texto(valor.clone()),
                    valor,
                ))
            }
            tk::DOUBLE_QUOTE => {
                let valor = self.process_quoted_string(false)?;
                let valor = format!("\"{}\"", escape_string(&valor, false));
                um(Termo::novo(
                    TipoTermo::Literal,
                    Valor::Texto(valor.clone()),
                    valor,
                ))
            }
            tk::LPAREN => {
                self.next()?;
                let mut grupo = Vec::new();
                loop {
                    let termo = self.process_term(false)?;
                    let nulo = termo.is_none();
                    if let Some(TermoOuLista::Um(Expr::Termo(t))) = termo {
                        grupo.push(t);
                    }
                    if nulo || self.maybe_eat(tk::RPAREN)? || self.is_premature_end_of_file()? {
                        break;
                    }
                }
                Ok(Some(TermoOuLista::Um(Expr::Grupo(grupo))))
            }
            tk::LBRACK => {
                self.next()?;
                // `processTerm() as LiteralTerm`.
                let termo = match self.process_term(false)? {
                    Some(TermoOuLista::Um(Expr::Termo(t))) => t,
                    _ => return Err(Excecao("TypeError")),
                };
                if !termo.e_numero() {
                    self.error();
                }
                self.eat(tk::RBRACK)?;
                um(Termo::novo(TipoTermo::Item, termo.valor, termo.texto))
            }
            tk::IDENTIFIER => self.process_identifier(ie_filter, start),
            tk::UNICODE_RANGE => {
                let mut first = None;
                let mut second = None;
                self.eat_ur(tk::UNICODE_RANGE, true)?;
                if self.maybe_eat_ur(tk::HEX_INTEGER, true)? {
                    let f = self.anterior_texto();
                    let first_number = hex_parse(&f)?;
                    if first_number > MAX_UNICODE {
                        self.error();
                    }
                    first = Some(f);
                    if self.maybe_eat_ur(tk::MINUS, true)?
                        && self.maybe_eat_ur(tk::HEX_INTEGER, true)?
                    {
                        let s = self.anterior_texto();
                        let second_number = hex_parse(&s)?;
                        if second_number > MAX_UNICODE {
                            self.error();
                        }
                        if first_number > second_number {
                            self.error();
                        }
                        second = Some(s);
                    }
                } else if self.maybe_eat_ur(tk::HEX_RANGE, true)? {
                    first = Some(self.anterior_texto());
                }
                Ok(Some(TermoOuLista::Um(Expr::UnicodeRange {
                    primeiro: first,
                    segundo: second,
                })))
            }
            tk::AT => {
                // Less (`lessSupport`): `property: @name` => `var(name)`.
                self.next()?;
                let mut expr = self.process_expr(false)?;
                let Some(param) = expr.expressoes.first() else {
                    return Err(Excecao("RangeError"));
                };
                let Expr::Termo(param) = param else {
                    return Err(Excecao("TypeError"));
                };
                let nome = param.texto.clone();
                expr.expressoes[0] = Expr::VarUsage {
                    nome,
                    padroes: Vec::new(),
                };
                Ok(Some(TermoOuLista::Lista(expr.expressoes)))
            }
            _ => {
                // Tokens de identificador sintetizados (`PT`, `PX`…).
                if tk::is_kind_identifier(self.peek()) {
                    self.process_identifier(ie_filter, start)
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// `_previousToken!.text`.
    fn anterior_texto(&self) -> String {
        self.anterior
            .as_ref()
            .map(|t| t.texto(self.fonte))
            .unwrap_or_default()
    }

    /// O `processIdentifier` local do `processTerm`.
    fn process_identifier(&mut self, ie_filter: bool, start: usize) -> R<Option<TermoOuLista>> {
        let nome = self.identifier()?;
        if !ie_filter && self.maybe_eat(tk::LPAREN)? {
            if let Some(calc) = self.process_calc(&nome)? {
                return Ok(Some(TermoOuLista::Um(Expr::Termo(calc))));
            }
            // FUNCTION
            return Ok(Some(TermoOuLista::Um(self.process_function(nome)?)));
        }
        if ie_filter {
            // `filter:progid:…` ou `filter:<nome>`: os dois caminhos do
            // oficial dão no mesmo `processIEFilter` (o `_maybeEat(COLON)`
            // vem antes do `&&`).
            self.maybe_eat(tk::COLON)?;
            return Ok(self
                .process_ie_filter(start)?
                .map(|t| TermoOuLista::Um(Expr::Termo(t))));
        }
        let texto = nome.nome.clone();
        if texto == "from" {
            return Ok(Some(TermoOuLista::Um(Expr::Termo(Termo::novo(
                TipoTermo::Literal,
                Valor::Ident(nome),
                texto,
            )))));
        }
        // Nome de cor?
        match tk::match_color_name(&texto) {
            None => Ok(Some(TermoOuLista::Um(Expr::Termo(Termo::novo(
                TipoTermo::Literal,
                Valor::Ident(nome),
                texto,
            ))))),
            Some(valor) => {
                // A cor como valor RGB.
                let rgb = tk::decimal_to_hex(valor, 6);
                Ok(Some(TermoOuLista::Um(Expr::Termo(self.parse_hex(rgb)))))
            }
        }
    }

    /// `processDimension`.
    fn process_dimension(&mut self, t: Option<&Token>, valor: Valor) -> R<Termo> {
        let unidade = self.peek();
        let texto_t =
            |p: &Self| -> R<String> { t.map(|t| t.texto(p.fonte)).ok_or(Excecao("TypeError")) };
        let tipo = match unidade {
            tk::UNIT_EM => TipoTermo::Em,
            tk::UNIT_EX => TipoTermo::Ex,
            tk::UNIT_LENGTH_PX
            | tk::UNIT_LENGTH_CM
            | tk::UNIT_LENGTH_MM
            | tk::UNIT_LENGTH_IN
            | tk::UNIT_LENGTH_PT
            | tk::UNIT_LENGTH_PC => TipoTermo::Unidade {
                unidade,
                comprimento: true,
            },
            tk::UNIT_ANGLE_DEG
            | tk::UNIT_ANGLE_RAD
            | tk::UNIT_ANGLE_GRAD
            | tk::UNIT_ANGLE_TURN
            | tk::UNIT_TIME_MS
            | tk::UNIT_TIME_S
            | tk::UNIT_FREQ_HZ
            | tk::UNIT_FREQ_KHZ
            | tk::UNIT_RESOLUTION_DPI
            | tk::UNIT_RESOLUTION_DPCM
            | tk::UNIT_RESOLUTION_DPPX
            | tk::UNIT_CH
            | tk::UNIT_REM
            | tk::UNIT_VIEWPORT_VW
            | tk::UNIT_VIEWPORT_VH
            | tk::UNIT_VIEWPORT_VMIN
            | tk::UNIT_VIEWPORT_VMAX
            | tk::UNIT_LH
            | tk::UNIT_RLH => TipoTermo::Unidade {
                unidade,
                comprimento: false,
            },
            tk::PERCENT => TipoTermo::Porcentagem,
            tk::UNIT_FRACTION => TipoTermo::Fracao,
            _ => {
                return match valor {
                    Valor::Ident(i) => {
                        let texto = i.nome.clone();
                        Ok(Termo::novo(TipoTermo::Literal, Valor::Ident(i), texto))
                    }
                    v => Ok(Termo::novo(TipoTermo::Numero, v, texto_t(self)?)),
                };
            }
        };
        // `span.expand(_next().span)`, depois o `t!.text`.
        self.next()?;
        Ok(Termo::novo(tipo, valor, texto_t(self)?))
    }

    /// `processQuotedString`.
    fn process_quoted_string(&mut self, url_string: bool) -> R<String> {
        // O URI engole tudo entre aspas ou entre parênteses.
        let mut stop_token = if url_string { tk::RPAREN } else { -1 };
        // Sem pular espaço dentro da string.
        let in_string = self.tokenizer.in_string;
        self.tokenizer.in_string = false;
        match self.peek() {
            tk::SINGLE_QUOTE => {
                stop_token = tk::SINGLE_QUOTE;
                self.next()?;
            }
            tk::DOUBLE_QUOTE => {
                stop_token = tk::DOUBLE_QUOTE;
                self.next()?;
            }
            _ => {
                if url_string {
                    if self.peek() == tk::LPAREN {
                        self.next()?;
                    }
                    stop_token = tk::RPAREN;
                } else {
                    self.error();
                }
            }
        }
        // Tudo até o token de parada.
        let mut valor = String::new();
        while self.peek() != stop_token && self.peek() != tk::END_OF_FILE {
            let t = self.next()?;
            valor.push_str(&t.texto(self.fonte));
        }
        self.tokenizer.in_string = in_string;
        if stop_token != tk::RPAREN {
            self.next()?; // a aspa
        }
        Ok(valor)
    }

    /// `processIEFilter`: o texto cru do filtro, do início do termo até o
    /// `;`/`}` ou até fechar o parêntese; nulo no fim do arquivo.
    fn process_ie_filter(&mut self, start: usize) -> R<Option<Termo>> {
        let literal = |p: &Self| {
            let texto = p.tokenizer.texto_entre(start, p.espiado.ini);
            Termo::novo(TipoTermo::Literal, Valor::Texto(texto.clone()), texto)
        };
        let kind = self.peek();
        if kind == tk::SEMICOLON || kind == tk::RBRACE {
            return Ok(Some(literal(self)));
        }
        let mut parens = 0i64;
        while self.peek() != tk::END_OF_FILE {
            match self.peek() {
                tk::LPAREN => {
                    self.eat(tk::LPAREN)?;
                    parens += 1;
                }
                tk::RPAREN => {
                    self.eat(tk::RPAREN)?;
                    parens -= 1;
                    if parens == 0 {
                        return Ok(Some(literal(self)));
                    }
                }
                k => self.eat(k)?,
            }
        }
        Ok(None)
    }

    /// `processCalcExpression`: o texto cru até o `)` que fecha.
    fn process_calc_expression(&mut self) -> R<String> {
        let in_string = self.tokenizer.in_string;
        self.tokenizer.in_string = false;
        let mut valor = String::new();
        let mut left = 1;
        let mut matching_parens = false;
        while self.peek() != tk::END_OF_FILE && !matching_parens {
            let token = self.peek();
            if token == tk::LPAREN {
                left += 1;
            } else if token == tk::RPAREN {
                left -= 1;
            }
            matching_parens = left == 0;
            if !matching_parens {
                let t = self.next()?;
                valor.push_str(&t.texto(self.fonte));
            }
        }
        if !matching_parens {
            self.error();
        }
        self.tokenizer.in_string = in_string;
        Ok(valor)
    }

    /// `processCalc`.
    fn process_calc(&mut self, func: &Identificador) -> R<Option<Termo>> {
        let nome = func.nome.as_str();
        if ["calc", "-webkit-calc", "-moz-calc", "min", "max", "clamp"].contains(&nome) {
            let expressao = self.process_calc_expression()?;
            let calc_expr = Termo::novo(
                TipoTermo::Literal,
                Valor::Texto(expressao.clone()),
                expressao,
            );
            if !self.maybe_eat(tk::RPAREN)? {
                self.error();
            }
            return Ok(Some(Termo::novo(
                TipoTermo::Calc(Box::new(calc_expr)),
                Valor::Texto(nome.to_string()),
                nome.to_string(),
            )));
        }
        Ok(None)
    }

    /// `processFunction`: `UriTerm`, `VarUsage` ou `FunctionTerm`.
    fn process_function(&mut self, func: Identificador) -> R<Expr> {
        let nome = func.nome;
        match nome.as_str() {
            "url" => {
                let url_param = self.process_quoted_string(true)?;
                if self.peek() == tk::END_OF_FILE {
                    self.error();
                }
                if self.peek() == tk::RPAREN {
                    self.next()?;
                }
                Ok(Expr::Termo(Termo::novo(
                    TipoTermo::Uri,
                    Valor::Texto(url_param.clone()),
                    url_param,
                )))
            }
            "var" => {
                let mut expr = self.process_expr(false)?;
                if !self.maybe_eat(tk::RPAREN)? {
                    self.error();
                }
                // `(expr.expressions[0] as LiteralTerm).text`.
                let param_name = match expr.expressoes.first() {
                    None => return Err(Excecao("RangeError")),
                    Some(Expr::Termo(t)) => t.texto.clone(),
                    Some(_) => return Err(Excecao("TypeError")),
                };
                // [0] - nome, [1] - vírgula, [2..] - valor padrão.
                let padroes = if expr.expressoes.len() >= 3 {
                    expr.expressoes.split_off(2)
                } else {
                    Vec::new()
                };
                Ok(Expr::VarUsage {
                    nome: param_name,
                    padroes,
                })
            }
            _ => {
                let expr = self.process_expr(false)?;
                if !self.maybe_eat(tk::RPAREN)? {
                    self.error();
                }
                Ok(Expr::Termo(Termo::novo(
                    TipoTermo::Funcao(expr),
                    Valor::Texto(nome.clone()),
                    nome,
                )))
            }
        }
    }

    /// `identifier`: o token seguinte como identificador (vazio se não é
    /// identificador; o texto é sempre o do token).
    fn identifier(&mut self) -> R<Identificador> {
        let tok = self.next()?;
        let texto = Some(String::from_utf16_lossy(&self.fonte[tok.ini..tok.fim]));
        if !tk::is_identifier(tok.kind) && !tk::is_kind_identifier(tok.kind) {
            return Ok(Identificador {
                nome: String::new(),
                texto,
            });
        }
        Ok(Identificador {
            nome: tok.texto(self.fonte),
            texto,
        })
    }

    /// `_parseHex`.
    fn parse_hex(&mut self, hex_text: String) -> Termo {
        let mut hex_value: i64 = 0;
        for c in hex_text.encode_utf16() {
            let digito = hex_digit(c);
            if digito < 0 {
                self.warning();
                return Termo::novo(TipoTermo::Hex, Valor::HexInvalido, hex_text);
            }
            hex_value = hex_value.wrapping_shl(4).wrapping_add(digito);
        }
        // #RRGGBB => #RGB quando cada par repete o dígito.
        let b = hex_text.as_bytes();
        let texto = if b.len() == 6 && b[0] == b[1] && b[2] == b[3] && b[4] == b[5] {
            format!("{}{}{}", b[0] as char, b[2] as char, b[4] as char)
        } else if b.len() == 4 && b[0] == b[1] && b[2] == b[3] {
            format!("{}{}", b[0] as char, b[2] as char)
        } else if b.len() == 2 && b[0] == b[1] {
            (b[0] as char).to_string()
        } else {
            hex_text
        };
        Termo::novo(TipoTermo::Hex, Valor::Int(hex_value), texto)
    }
}

/// Um seletor simples de nome `Identifier`, com posição.
fn simples(tipo: TipoSimples, nome: Identificador) -> std::rc::Rc<SeletorSimples> {
    std::rc::Rc::new(SeletorSimples {
        tipo,
        nome: NomeSimples::Ident(nome),
        tem_span: true,
    })
}

/// `_Parser._hexDigit`.
fn hex_digit(c: u16) -> i64 {
    match c {
        48..=57 => i64::from(c) - 48,
        97..=102 => i64::from(c) - 87,
        65..=70 => i64::from(c) - 55,
        _ => -1,
    }
}

/// `int.parse` de um texto de token numérico (sinal e dígitos): fora do
/// intervalo de 64 bits o Dart lança `FormatException`.
fn int_parse(texto: &str) -> R<i64> {
    texto.parse::<i64>().map_err(|_| Excecao("FormatException"))
}

/// `double.parse` de um texto de token numérico.
fn double_parse(texto: &str) -> R<f64> {
    texto.parse::<f64>().map_err(|_| Excecao("FormatException"))
}

/// `int.parse('0x…')`: até 64 bits (com o bit de sinal, como o Dart).
fn hex_parse(texto: &str) -> R<i64> {
    u64::from_str_radix(texto, 16)
        .map(|v| v as i64)
        .map_err(|_| Excecao("FormatException"))
}

/// `_escapeString`: escapa a aspa que delimita a string.
pub(crate) fn escape_string(texto: &str, single: bool) -> String {
    let mut saida = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '"' if !single => saida.push_str("\\\""),
            '\'' if single => saida.push_str("\\'"),
            _ => saida.push(c),
        }
    }
    saida
}

/// `_styleForDart` e `buildDartStyleNode`, só no que lança: os estilos que
/// constroem não são lidos por ninguém.
fn style_for_dart(propriedade: &Identificador, exprs: &Expressoes) -> R<()> {
    let e = &exprs.expressoes;
    let primeiro = || e.first().ok_or(Excecao("RangeError"));
    match propriedade.nome.to_lowercase().as_str() {
        // `font`, `font-family`, `font-size`: o `ExpressionsProcessor` só lê
        // `LengthTerm`, cujo valor é sempre número.
        "font" | "font-family" | "font-size" | "font-style" | "font-variant" => {}
        "font-weight" => {
            // `expr.value as int?` num `NumberTerm`.
            if let Expr::Termo(t) = primeiro()?
                && t.e_numero()
                && !matches!(t.valor, Valor::Int(_))
            {
                return Err(Excecao("TypeError"));
            }
        }
        "line-height" => {
            if e.len() == 1
                && let Expr::Termo(t) = &e[0]
            {
                let px_ou_pt = matches!(
                    t.tipo,
                    TipoTermo::Unidade {
                        unidade: tk::UNIT_LENGTH_PX | tk::UNIT_LENGTH_PT,
                        ..
                    }
                );
                if px_ou_pt || t.e_numero() {
                    como_num(&t.valor)?;
                }
            }
        }
        "margin" | "padding" => {
            // `processFourNums`.
            if (1..=4).contains(&e.len()) {
                for x in e {
                    margin_value(x)?;
                }
            }
        }
        "border" => {
            for x in e {
                if margin_value(x)?.is_some() {
                    break;
                }
            }
        }
        "border-width" => {
            margin_value(primeiro()?)?;
        }
        "margin-left"
        | "margin-right"
        | "margin-top"
        | "margin-bottom"
        | "border-left"
        | "border-right"
        | "border-top"
        | "border-bottom"
        | "border-left-width"
        | "border-top-width"
        | "border-right-width"
        | "border-bottom-width"
        | "height"
        | "width"
        | "padding-left"
        | "padding-top"
        | "padding-right"
        | "padding-bottom" => {
            if let Some(x) = e.first() {
                margin_value(x)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// `value as num`.
fn como_num(valor: &Valor) -> R<()> {
    match valor {
        Valor::Int(_) | Valor::Double(_) => Ok(()),
        _ => Err(Excecao("TypeError")),
    }
}

/// `marginValue`: o número de um `UnitTerm` ou `NumberTerm`.
fn margin_value(expr: &Expr) -> R<Option<()>> {
    match expr {
        Expr::Termo(t) if matches!(t.tipo, TipoTermo::Unidade { .. }) || t.e_numero() => {
            como_num(&t.valor)?;
            Ok(Some(()))
        }
        _ => Ok(None),
    }
}
