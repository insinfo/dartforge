//! Porte de `short/source_visitor.dart` (`SourceVisitor`): percorre todos os
//! tokens da árvore e passa ao [`Construtor`] o texto, os espaços, as quebras
//! possíveis e os comentários.
//!
//! Cada `visitX` do original virou `visit_x`; os auxiliares mantêm os nomes
//! (`_visitCollectionLiteral` → `visit_collection_literal`). As correções
//! opcionais (`--fix`) não são portadas: o `dart format` só as aplica quando
//! pedidas.

use std::collections::{HashMap, HashSet};

use super::arena::{Arenas, ChunkId, RegraId, custo, recuo};
use super::construtor::{ComentarioFonte, Construtor, TipoComentario};
use crate::sintaxe::arvore::{Arvore, No, NoId, TipoParametro};
use crate::sintaxe::token::{TipoToken, Tk, Tokens};
use dartforge_frontend::token::{Keyword, Op};

/// Ação entre (ou antes/depois de) nós de uma lista, como as funções passadas
/// a `visitNodes` no original.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acao {
    Nada,
    Espaco,
    NovaLinha,
    Split,
    ZeroSplit,
    SplitOrNewline,
    SplitOrTwoNewlines,
    OneOrTwoNewlines,
}

/// Linhas da fonte (`LineInfo`), com a mesma regra do analyzer: `\n`, `\r\n`
/// e `\r` sozinho terminam linha.
pub struct InfoLinhas {
    inicios: Vec<usize>,
}

impl InfoLinhas {
    pub fn de(fonte: &str) -> InfoLinhas {
        let b = fonte.as_bytes();
        let mut inicios = vec![0];
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\r' {
                if b.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                inicios.push(i + 1);
            } else if b[i] == b'\n' {
                inicios.push(i + 1);
            }
            i += 1;
        }
        InfoLinhas { inicios }
    }

    /// Linha (a partir de 1) da posição.
    pub fn linha(&self, pos: usize) -> i32 {
        self.inicios.partition_point(|&s| s <= pos) as i32
    }

    /// Coluna (a partir de 1) da posição.
    pub fn coluna(&self, pos: usize) -> usize {
        let l = self.linha(pos) as usize;
        pos - self.inicios[l - 1] + 1
    }

    /// Início das linhas (para a primeira quebra).
    pub fn inicios(&self) -> &[usize] {
        &self.inicios
    }
}

/// `SourceVisitor`.
pub struct Visitante<'f> {
    pub fonte: &'f str,
    pub tk: &'f Tokens,
    pub arv: &'f Arvore,
    linhas: InfoLinhas,
    pub a: Arenas,
    pilha: Vec<Construtor>,
    ultimo_token: Tk,
    colecoes: Vec<bool>,
    regras_de_bloco: HashMap<Tk, RegraId>,
    chunks_de_bloco: HashMap<Tk, ChunkId>,
    suprimidos: HashSet<Tk>,
}

impl<'f> Visitante<'f> {
    /// `SourceVisitor(formatter, lineInfo, source)`.
    pub fn novo(fonte: &'f str, tk: &'f Tokens, arv: &'f Arvore, largura: usize, recuo: i32, fim_de_linha: String) -> Visitante<'f> {
        let mut a = Arenas::nova(largura, fim_de_linha);
        let topo = Construtor::topo(&mut a, recuo);
        Visitante {
            fonte,
            tk,
            arv,
            linhas: InfoLinhas::de(fonte),
            a,
            pilha: vec![topo],
            ultimo_token: 0,
            colecoes: Vec::new(),
            regras_de_bloco: HashMap::new(),
            chunks_de_bloco: HashMap::new(),
            suprimidos: HashSet::new(),
        }
    }

    /// `run(node)`: visita a unidade e devolve os chunks de topo.
    pub fn executar(mut self, raiz: NoId) -> (Arenas, Vec<ChunkId>) {
        self.visit(Some(raiz));
        // `node.endToken.next`: o EOF (numa unidade vazia, o próprio fim).
        let eof = self.tk.lista.len() as Tk - 1;
        let depois = (self.arv.fim(raiz) + 1).min(eof);
        self.write_preceding_comments_and_newlines(depois);
        let topo = self.pilha.pop().expect("construtor de topo");
        let chunks = topo.terminar(&mut self.a);
        (self.a, chunks)
    }

    // ================================================================ tokens

    fn tipo(&self, t: Tk) -> TipoToken {
        self.tk.lista[t as usize].tipo
    }

    fn lexema(&self, t: Tk) -> &'f str {
        self.tk.texto(self.fonte, t)
    }

    fn e_op(&self, t: Tk, op: Op) -> bool {
        self.tipo(t) == TipoToken::Op(op)
    }

    fn tem_comentarios(&self, t: Tk) -> bool {
        !self.tk.comentarios_de(t).is_empty()
    }

    fn linha_inicio(&self, t: Tk) -> i32 {
        self.linhas.linha(self.tk.lista[t as usize].inicio)
    }

    fn linha_fim(&self, t: Tk) -> i32 {
        let k = &self.tk.lista[t as usize];
        let mut fim = k.fim;
        if k.tipo == TipoToken::ScriptTag {
            // O lexema do analyzer inclui a quebra de linha.
            let b = self.fonte.as_bytes();
            if b.get(fim) == Some(&b'\r') {
                fim += 1;
            }
            if b.get(fim) == Some(&b'\n') {
                fim += 1;
            }
        }
        self.linhas.linha(fim)
    }

    /// `_linesBeforeNextToken`.
    fn linhas_antes_do_proximo(&self) -> i32 {
        let anterior = self.ultimo_token;
        let proximo = anterior + 1;
        let inicio = match self.tk.comentarios_de(proximo).first() {
            Some(c) => self.linhas.linha(c.inicio),
            None => self.linha_inicio(proximo),
        };
        inicio - self.linha_fim(anterior)
    }

    // ============================================================= construtor

    fn b(&mut self) -> (&mut Construtor, &mut Arenas) {
        (self.pilha.last_mut().expect("construtor"), &mut self.a)
    }

    pub fn space(&mut self) {
        self.b().0.espaco();
    }

    pub fn newline(&mut self) {
        self.b().0.nova_linha(false, false, false);
    }

    pub fn two_newlines(&mut self) {
        self.b().0.nova_linha(true, false, false);
    }

    pub fn write_newline(&mut self, duplo: bool, rente: bool, aninhar: bool) {
        self.b().0.nova_linha(duplo, rente, aninhar);
    }

    pub fn split_or_newline(&mut self) {
        if self.linhas_antes_do_proximo() > 0 {
            self.write_newline(false, false, true);
        } else {
            self.split();
        }
    }

    pub fn split_or_two_newlines(&mut self) {
        if self.linhas_antes_do_proximo() > 1 {
            self.two_newlines();
        } else {
            self.split();
        }
    }

    pub fn one_or_two_newlines(&mut self) {
        let duplo = self.linhas_antes_do_proximo() > 1;
        self.write_newline(duplo, false, false);
    }

    /// `split()`: quebra com espaço na regra atual.
    pub fn split(&mut self) -> ChunkId {
        let (b, a) = self.b();
        b.quebra(a, true, true)
    }

    /// `zeroSplit()`.
    pub fn zero_split(&mut self) -> ChunkId {
        let (b, a) = self.b();
        b.quebra(a, true, false)
    }

    /// `builder.split(nest:, space:)`.
    pub fn builder_split(&mut self, aninhar: bool, espaco: bool) -> ChunkId {
        let (b, a) = self.b();
        b.quebra(a, aninhar, espaco)
    }

    /// `soloSplit([cost])`.
    pub fn solo_split(&mut self, custo: i32) -> RegraId {
        let r = self.a.regra(custo);
        self.start_rule(Some(r));
        self.split();
        self.end_rule();
        r
    }

    /// `soloZeroSplit()`.
    pub fn solo_zero_split(&mut self) {
        self.start_rule(None);
        self.builder_split(true, false);
        self.end_rule();
    }

    pub fn start_span(&mut self, custo: i32) {
        self.b().0.iniciar_span(custo);
    }

    pub fn end_span(&mut self) {
        let (b, a) = self.b();
        b.terminar_span(a);
    }

    pub fn start_rule(&mut self, r: Option<RegraId>) {
        let (b, a) = self.b();
        b.iniciar_regra(a, r);
    }

    pub fn start_lazy_rule(&mut self, r: Option<RegraId>) {
        let (b, a) = self.b();
        b.iniciar_regra_preguicosa(a, r);
    }

    pub fn end_rule(&mut self) {
        self.b().0.terminar_regra();
    }

    pub fn current_rule(&self) -> RegraId {
        self.pilha.last().expect("construtor").regra()
    }

    pub fn force_rules(&mut self) {
        let (b, a) = self.b();
        b.forcar_regras(a);
    }

    pub fn nest_expression(&mut self, recuo: Option<i32>, agora: bool) {
        let (b, a) = self.b();
        b.aninhar_expressao(a, recuo, agora);
    }

    /// `unnest()` (que por padrão confirma na hora).
    pub fn unnest(&mut self) {
        let (b, a) = self.b();
        b.desaninhar(a, true);
    }

    pub fn unnest_later(&mut self) {
        let (b, a) = self.b();
        b.desaninhar(a, false);
    }

    pub fn start_block_argument_nesting(&mut self) {
        self.b().0.iniciar_aninhamento_de_argumento();
    }

    pub fn end_block_argument_nesting(&mut self) {
        self.b().0.terminar_aninhamento_de_argumento();
    }

    fn indent(&mut self, espacos: Option<i32>) {
        self.b().0.recuar(espacos);
    }

    fn unindent(&mut self) {
        self.b().0.desrecuar();
    }

    /// `builder = builder.startBlock(...)`.
    fn start_block(&mut self, argumento: Option<ChunkId>, recuar: bool, espaco: bool) {
        let (b, a) = self.b();
        let filho = b.iniciar_bloco(a, argumento, recuar, espaco);
        self.pilha.push(filho);
    }

    /// `builder = builder.endBlock(forceSplit:)`.
    fn end_block(&mut self, forcar: bool) {
        let filho = self.pilha.pop().expect("bloco");
        if filho.terminar_bloco(&mut self.a, forcar) {
            self.force_rules();
        }
    }

    // ================================================================ escrita

    /// `token(token)`.
    pub fn token(&mut self, t: Tk) {
        self.write_preceding_comments_and_newlines(t);
        let texto = self.lexema(t);
        self.write_text(texto, t);
    }

    /// `token(token)` com token opcional.
    pub fn token_opt(&mut self, t: Option<Tk>) {
        if let Some(t) = t {
            self.token(t);
        }
    }

    /// `token(token, after: space)` / `modifier(token)`.
    pub fn modifier(&mut self, t: Option<Tk>) {
        if let Some(t) = t {
            self.token(t);
            self.space();
        }
    }

    /// `token(token, before: space)`: o espaço vem depois dos comentários.
    fn token_space_before(&mut self, t: Option<Tk>) {
        if let Some(t) = t {
            self.write_preceding_comments_and_newlines(t);
            self.space();
            let texto = self.lexema(t);
            self.write_text(texto, t);
        }
    }

    /// `_writeText(text, token)`.
    fn write_text(&mut self, texto: &str, t: Tk) {
        self.write_text_merge(texto, t, true);
    }

    fn write_text_merge(&mut self, texto: &str, t: Tk, juntar: bool) {
        let (b, a) = self.b();
        b.escrever(a, texto, juntar);
        self.ultimo_token = t;
    }

    /// `writePrecedingCommentsAndNewlines(token)`.
    pub fn write_preceding_comments_and_newlines(&mut self, t: Tk) -> bool {
        let comentarios = self.tk.comentarios_de(t);
        if comentarios.is_empty() {
            return false;
        }
        if self.suprimidos.contains(&t) {
            return false;
        }
        let anterior = if t == 0 { None } else { Some(t - 1) };
        let mut linha_anterior = anterior.map(|p| self.linha_fim(p)).unwrap_or(1);
        let linha_token = self.linha_inicio(t);
        if anterior.is_some_and(|p| self.tipo(p) == TipoToken::ScriptTag) {
            linha_anterior = linha_token;
        }
        let mut lista = Vec::with_capacity(comentarios.len());
        for (i, c) in comentarios.iter().enumerate() {
            let linha_c = self.linhas.linha(c.inicio);
            if i == 0 && anterior.is_none() {
                linha_anterior = linha_c;
            }
            let texto = self.fonte[c.inicio..c.fim].trim();
            let mut linhas_antes = linha_c - linha_anterior;
            let mut rente = self.linhas.coluna(c.inicio) == 1;
            let doc_linha = texto.starts_with("///") && !texto.starts_with("////");
            if doc_linha {
                rente = false;
                if i == 0 {
                    linhas_antes = 2;
                }
            }
            let tipo = if doc_linha || (texto.starts_with("/**") && texto != "/**/") {
                TipoComentario::Doc
            } else if c.de_linha {
                TipoComentario::Linha
            } else if linha_c == linha_anterior || linha_c == linha_token {
                TipoComentario::BlocoEmLinha
            } else {
                TipoComentario::Bloco
            };
            lista.push(ComentarioFonte {
                texto: texto.to_string(),
                tipo,
                linhas_antes,
                rente,
            });
            linha_anterior = self.linhas.linha(c.fim);
        }
        let token = self.lexema(t);
        let (b, a) = self.b();
        b.escrever_comentarios(a, &mut lista, linha_token - linha_anterior, token);
        lista[0].linhas_antes > 0
    }

    // ============================================================== consultas

    pub fn no(&self, id: NoId) -> &'f No {
        self.arv.no(id)
    }

    fn pai(&self, id: NoId) -> Option<NoId> {
        self.arv.pai(id)
    }

    fn pai_e(&self, id: NoId, f: fn(&No) -> bool) -> bool {
        self.pai(id).is_some_and(|p| f(self.no(p)))
    }

    /// `commaAfter`.
    pub fn comma_after(&self, id: NoId) -> Option<Tk> {
        let proximo = self.arv.fim(id) + 1;
        if self.e_op(proximo, Op::Comma) {
            return Some(proximo);
        }
        if self.e_op(proximo, Op::Question) && self.e_op(proximo + 1, Op::Comma) {
            return Some(proximo + 1);
        }
        None
    }

    pub fn has_comma_after(&self, id: NoId) -> bool {
        self.comma_after(id).is_some()
    }

    pub fn list_has_comma_after(&self, nos: &[NoId]) -> bool {
        nos.last().is_some_and(|&n| self.has_comma_after(n))
    }

    /// `canSplit(rightBracket)`.
    fn can_split(&self, nos: &[NoId], direito: Tk) -> bool {
        !nos.is_empty() || self.tem_comentarios(direito)
    }

    /// `hasLineCommentBefore` do token.
    fn has_line_comment_before(&self, t: Tk) -> bool {
        self.tk.comentarios_de(t).iter().any(|c| c.de_linha)
    }

    /// `beginToken.hasLineCommentBefore` de um nó. Nós anotados com
    /// comentário de documentação começam no próprio comentário no analyzer,
    /// e um token de comentário não tem comentários antes.
    fn node_has_line_comment_before(&self, id: NoId) -> bool {
        let inicio = self.arv.inicio(id);
        if matches!(self.no(id), No::EnumConstantDeclaration { .. }) && self.tem_doc(inicio) {
            return false;
        }
        self.has_line_comment_before(inicio)
    }

    /// Se os comentários antes de `t` têm um comentário de documentação
    /// (`findDartDoc`: o último `/**`, ou uma sequência de `///`).
    fn tem_doc(&self, t: Tk) -> bool {
        self.tk.comentarios_de(t).iter().any(|c| {
            let s = &self.fonte[c.inicio..c.fim];
            s.starts_with("///") || s.starts_with("/**")
        })
    }

    /// `containsLineComments([rightBracket])`.
    fn contains_line_comments(&self, nos: &[NoId], direito: Option<Tk>) -> bool {
        nos.iter().any(|&n| self.node_has_line_comment_before(n)) || direito.is_some_and(|t| self.has_line_comment_before(t))
    }

    /// `hasNonEmptyBody`.
    fn has_non_empty_body(&self, id: NoId) -> bool {
        let corpo = match self.no(id) {
            No::MethodDeclaration { body, .. } => Some(*body),
            No::FunctionDeclarationStatement { function_declaration } => self.corpo_de_declaracao(*function_declaration),
            No::FunctionDeclaration { .. } => self.corpo_de_declaracao(id),
            _ => None,
        };
        corpo.is_some_and(|c| self.bloco_nao_vazio(c))
    }

    fn corpo_de_declaracao(&self, decl: NoId) -> Option<NoId> {
        if let No::FunctionDeclaration { function_expression, .. } = self.no(decl)
            && let No::FunctionExpression { body, .. } = self.no(*function_expression)
        {
            return Some(*body);
        }
        None
    }

    /// `body is BlockFunctionBody && body.block.statements.isNotEmpty`.
    fn bloco_nao_vazio(&self, corpo: NoId) -> bool {
        if let No::BlockFunctionBody { block, .. } = self.no(corpo)
            && let No::Block { statements, .. } = self.no(*block)
        {
            return !statements.is_empty();
        }
        false
    }

    pub fn is_collection_literal(&self, id: NoId) -> bool {
        matches!(self.no(id), No::ListLiteral { .. } | No::RecordLiteral { .. } | No::SetOrMapLiteral { .. })
    }

    fn is_control_flow_element(&self, id: NoId) -> bool {
        matches!(self.no(id), No::IfElement { .. } | No::ForElement { .. })
    }

    /// `isFunctionExpressionBody`.
    fn is_function_expression_body(&self, id: NoId) -> bool {
        match self.pai(id) {
            Some(p) if matches!(self.no(p), No::FunctionExpression { .. }) => !self.pai_e(p, |n| matches!(n, No::FunctionDeclaration { .. })),
            _ => false,
        }
    }

    /// `spreadCollectionBracket`.
    fn spread_collection_bracket(&self, id: NoId) -> Option<Tk> {
        if let No::SpreadElement { expression, .. } = self.no(id) {
            match self.no(*expression) {
                No::ListLiteral {
                    elements,
                    left_bracket,
                    right_bracket,
                    ..
                }
                | No::SetOrMapLiteral {
                    elements,
                    left_bracket,
                    right_bracket,
                    ..
                } => {
                    if self.can_split(elements, *right_bracket) {
                        return Some(*left_bracket);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// `isTrailingCommaArgument`.
    fn is_trailing_comma_argument(&self, id: NoId) -> bool {
        let mut pai = self.pai(id);
        if let Some(p) = pai
            && matches!(self.no(p), No::NamedExpression { .. })
        {
            pai = self.pai(p);
        }
        match pai.map(|p| self.no(p)) {
            Some(No::ArgumentList { arguments, .. }) => self.list_has_comma_after(arguments),
            _ => false,
        }
    }

    /// `looksLikeStaticCall`.
    pub fn looks_like_static_call(&self, id: NoId) -> bool {
        let No::MethodInvocation { target, method_name, .. } = self.no(id) else {
            return false;
        };
        let Some(mut alvo) = *target else {
            return false;
        };
        if matches!(self.no(alvo), No::SimpleIdentifier { .. }) && looks_like_class_name(self.nome_simples(*method_name)) {
            return true;
        }
        if let No::PrefixedIdentifier { identifier, .. } = self.no(alvo) {
            alvo = *identifier;
        }
        matches!(self.no(alvo), No::SimpleIdentifier { .. }) && looks_like_class_name(self.nome_simples(alvo))
    }

    pub fn nome_simples(&self, id: NoId) -> &'f str {
        match self.no(id) {
            No::SimpleIdentifier { token } => self.lexema(*token),
            _ => "",
        }
    }

    /// `allowInline` da cascata.
    fn allow_inline(&self, target: NoId, secoes: usize) -> bool {
        if secoes > 1 {
            return false;
        }
        !matches!(
            self.no(target),
            No::ConditionalExpression { .. } | No::BinaryExpression { .. } | No::PrefixExpression { .. } | No::AwaitExpression { .. }
        )
    }

    /// `isMultiline` de `SingleStringLiteral`.
    pub fn is_multiline_string(&self, id: NoId) -> bool {
        let primeiro = match self.no(id) {
            No::SimpleStringLiteral { literal } => *literal,
            No::StringInterpolation { elements } => match elements.first().map(|e| self.no(*e)) {
                Some(No::InterpolationString { contents }) => *contents,
                _ => return false,
            },
            _ => return false,
        };
        let s = self.lexema(primeiro);
        let s = s.strip_prefix('r').unwrap_or(s);
        s.starts_with("'''") || s.starts_with("\"\"\"")
    }

    fn is_string_literal(&self, id: NoId) -> bool {
        matches!(
            self.no(id),
            No::SimpleStringLiteral { .. } | No::StringInterpolation { .. } | No::AdjacentStrings { .. }
        )
    }

    fn named_parts(&self, id: NoId) -> (Tk, Tk, NoId) {
        let No::NamedExpression { name, expression } = self.no(id) else {
            panic!("NamedExpression esperado");
        };
        let No::Label { label, colon } = self.no(*name) else {
            panic!("Label esperado");
        };
        let No::SimpleIdentifier { token } = self.no(*label) else {
            panic!("SimpleIdentifier esperado");
        };
        (*token, *colon, *expression)
    }

    fn param_kind(&self, id: NoId) -> TipoParametro {
        match self.no(id) {
            No::SimpleFormalParameter { kind, .. }
            | No::FieldFormalParameter { kind, .. }
            | No::SuperFormalParameter { kind, .. }
            | No::FunctionTypedFormalParameter { kind, .. }
            | No::DefaultFormalParameter { kind, .. } => *kind,
            _ => TipoParametro::Obrigatorio,
        }
    }

    // ================================================================ visita

    /// `visit(node)`.
    pub fn visit(&mut self, no: Option<NoId>) {
        if let Some(id) = no {
            self.aceitar(id);
        }
    }

    fn visit_before(&mut self, no: Option<NoId>, antes: Acao) {
        if let Some(id) = no {
            self.acao(antes);
            self.aceitar(id);
        }
    }

    fn visit_after(&mut self, no: Option<NoId>, depois: Acao) {
        if let Some(id) = no {
            self.aceitar(id);
            self.acao(depois);
        }
    }

    fn acao(&mut self, a: Acao) {
        match a {
            Acao::Nada => {}
            Acao::Espaco => self.space(),
            Acao::NovaLinha => self.newline(),
            Acao::Split => {
                self.split();
            }
            Acao::ZeroSplit => {
                self.zero_split();
            }
            Acao::SplitOrNewline => self.split_or_newline(),
            Acao::SplitOrTwoNewlines => self.split_or_two_newlines(),
            Acao::OneOrTwoNewlines => self.one_or_two_newlines(),
        }
    }

    /// `visitNodes(nodes, before:, between:, after:)`.
    fn visit_nodes(&mut self, nos: &[NoId], antes: Acao, entre: Acao, depois: Acao) {
        if nos.is_empty() {
            return;
        }
        self.acao(antes);
        self.aceitar(nos[0]);
        for &n in &nos[1..] {
            self.acao(entre);
            self.aceitar(n);
        }
        self.acao(depois);
    }

    /// `visitCommaSeparatedNodes(nodes, between:)`.
    fn visit_comma_separated_nodes(&mut self, nos: &[NoId], entre: Acao) {
        for (i, &n) in nos.iter().enumerate() {
            if i > 0 {
                self.acao(entre);
            }
            self.aceitar(n);
            let proximo = self.arv.fim(n) + 1;
            if self.lexema(proximo) == "," {
                self.token(proximo);
            }
        }
    }

    /// `visitMetadata(metadata)`.
    fn visit_metadata(&mut self, metadata: &[NoId]) {
        self.visit_nodes(metadata, Acao::Nada, Acao::NovaLinha, Acao::NovaLinha);
    }

    /// `_writeCommaAfter(node)`.
    fn write_comma_after(&mut self, id: NoId) {
        let c = self.comma_after(id);
        self.token_opt(c);
    }

    /// `node.accept(this)`.
    fn aceitar(&mut self, id: NoId) {
        let n = self.no(id);
        match n {
            No::CompilationUnit { .. } => self.visit_compilation_unit(id),
            No::LibraryDirective { metadata, library_keyword, name2, semicolon } => {
                self.visit_directive_metadata(id, metadata);
                self.nest_expression(None, false);
                self.token(*library_keyword);
                self.visit_before(*name2, Acao::Espaco);
                self.token(*semicolon);
                self.unnest();
            }
            No::ImportDirective { .. } => self.visit_import_directive(id),
            No::ExportDirective {
                metadata,
                export_keyword,
                uri,
                configurations,
                combinators,
                semicolon,
            } => {
                self.visit_directive_metadata(id, metadata);
                self.nest_expression(None, false);
                self.token(*export_keyword);
                self.space();
                self.visit(Some(*uri));
                self.visit_configurations(configurations);
                self.visit_combinators(combinators);
                self.token(*semicolon);
                self.unnest();
            }
            No::PartDirective {
                metadata,
                part_keyword,
                uri,
                semicolon,
            } => {
                self.visit_directive_metadata(id, metadata);
                self.nest_expression(None, false);
                self.token(*part_keyword);
                self.space();
                self.visit(Some(*uri));
                self.token(*semicolon);
                self.unnest();
            }
            No::PartOfDirective {
                metadata,
                part_keyword,
                of_keyword,
                library_name,
                uri,
                semicolon,
            } => {
                self.visit_directive_metadata(id, metadata);
                self.nest_expression(None, false);
                self.token(*part_keyword);
                self.space();
                self.token(*of_keyword);
                self.space();
                self.visit(*library_name);
                self.visit(*uri);
                self.token(*semicolon);
                self.unnest();
            }
            No::Configuration {
                if_keyword,
                left_parenthesis,
                name,
                equal_token,
                value,
                right_parenthesis,
                uri,
            } => {
                self.token(*if_keyword);
                self.space();
                self.token(*left_parenthesis);
                self.visit(Some(*name));
                if let Some(eq) = equal_token {
                    self.nest_expression(None, false);
                    self.space();
                    self.token(*eq);
                    self.solo_split(custo::NORMAL);
                    self.visit(*value);
                    self.unnest();
                }
                self.token(*right_parenthesis);
                self.space();
                self.visit(Some(*uri));
            }
            No::DottedName { components } => {
                for (i, &c) in components.iter().enumerate() {
                    if i > 0 {
                        let ponto = self.arv.inicio(c) - 1;
                        self.token(ponto);
                    }
                    self.visit(Some(c));
                }
            }
            No::LibraryIdentifier { components } => {
                self.visit(Some(components[0]));
                for &c in &components[1..] {
                    let ponto = self.arv.inicio(c) - 1;
                    self.token(ponto);
                    self.visit(Some(c));
                }
            }
            No::ShowCombinator { keyword, shown_names } => self.visit_combinator(*keyword, shown_names),
            No::HideCombinator { keyword, hidden_names } => self.visit_combinator(*keyword, hidden_names),
            No::ClassDeclaration { .. } => self.visit_class_declaration(id),
            No::ExtendsClause { extends_keyword, superclass } => {
                self.solo_split(custo::NORMAL);
                self.token(*extends_keyword);
                self.space();
                self.visit(Some(*superclass));
            }
            No::WithClause { with_keyword, mixin_types } => self.visit_combinator(*with_keyword, mixin_types),
            No::ImplementsClause {
                implements_keyword,
                interfaces,
            } => self.visit_combinator(*implements_keyword, interfaces),
            No::MixinOnClause {
                on_keyword,
                superclass_constraints,
            } => self.visit_combinator(*on_keyword, superclass_constraints),
            No::NativeClause { native_keyword, name } => {
                self.token(*native_keyword);
                self.visit_before(*name, Acao::Espaco);
            }
            No::MixinDeclaration { .. } => self.visit_mixin_declaration(id),
            No::ExtensionDeclaration { .. } => self.visit_extension_declaration(id),
            No::ExtensionOnClause { .. } => panic!("ExtensionOnClause é visitada pela extensão"),
            No::ExtensionTypeDeclaration { .. } => self.visit_extension_type_declaration(id),
            No::RepresentationDeclaration { .. } => self.visit_representation_declaration(id),
            No::RepresentationConstructorName { period, name } => {
                self.token(*period);
                self.token(*name);
            }
            No::EnumDeclaration { .. } => self.visit_enum_declaration(id),
            No::EnumConstantDeclaration { metadata, name, arguments } => {
                self.visit_metadata(metadata);
                self.token(*name);
                if let Some(args) = arguments
                    && let No::EnumConstantArguments {
                        type_arguments,
                        constructor_selector,
                        argument_list,
                    } = self.no(*args)
                {
                    self.nest_expression(None, false);
                    self.visit(*type_arguments);
                    if let Some(sel) = constructor_selector
                        && let No::ConstructorSelector { period, name } = self.no(*sel)
                    {
                        self.token(*period);
                        self.visit(Some(*name));
                    }
                    self.visit_argument_list(*argument_list, false);
                    self.unnest();
                }
            }
            No::EnumConstantArguments { .. } | No::ConstructorSelector { .. } => {
                panic!("visitado pela constante do enum")
            }
            No::ClassTypeAlias { .. } => self.visit_class_type_alias(id),
            No::FunctionTypeAlias {
                metadata,
                typedef_keyword,
                return_type,
                name,
                type_parameters,
                parameters,
                semicolon,
                ..
            } => {
                self.visit_metadata(metadata);
                self.nest_expression(None, false);
                self.token(*typedef_keyword);
                self.space();
                self.visit_after(*return_type, Acao::Espaco);
                self.token(*name);
                self.visit(*type_parameters);
                self.visit(Some(*parameters));
                self.token(*semicolon);
                self.unnest();
            }
            No::GenericTypeAlias {
                metadata,
                typedef_keyword,
                name,
                type_parameters,
                equals,
                type_,
                semicolon,
                ..
            } => {
                self.visit_nodes(metadata, Acao::Nada, Acao::NovaLinha, Acao::NovaLinha);
                self.nest_expression(None, false);
                self.visit_generic_type_alias_header(*typedef_keyword, *name, *type_parameters, *equals);
                self.space();
                self.visit(Some(*type_));
                self.token(*semicolon);
                self.unnest();
            }
            No::FunctionDeclaration {
                metadata,
                external_keyword,
                return_type,
                property_keyword,
                name,
                function_expression,
                ..
            } => {
                let No::FunctionExpression {
                    type_parameters,
                    parameters,
                    body,
                } = self.no(*function_expression)
                else {
                    panic!("FunctionExpression esperada");
                };
                self.visit_function_or_method_declaration(
                    metadata,
                    *external_keyword,
                    *property_keyword,
                    None,
                    None,
                    *name,
                    *return_type,
                    *type_parameters,
                    *parameters,
                    *body,
                );
            }
            No::TopLevelVariableDeclaration {
                metadata,
                external_keyword,
                variables,
                semicolon,
                ..
            } => {
                self.visit_metadata(metadata);
                self.nest_expression(None, false);
                self.modifier(*external_keyword);
                self.visit(Some(*variables));
                self.token(*semicolon);
                self.unnest();
            }
            No::ConstructorDeclaration { .. } => self.visit_constructor_declaration(id),
            No::MethodDeclaration {
                metadata,
                external_keyword,
                modifier_keyword,
                return_type,
                property_keyword,
                operator_keyword,
                name,
                type_parameters,
                parameters,
                body,
                ..
            } => self.visit_function_or_method_declaration(
                metadata,
                *external_keyword,
                *property_keyword,
                *modifier_keyword,
                *operator_keyword,
                *name,
                *return_type,
                *type_parameters,
                *parameters,
                *body,
            ),
            No::FieldDeclaration {
                metadata,
                abstract_keyword,
                covariant_keyword,
                external_keyword,
                static_keyword,
                fields,
                semicolon,
                ..
            } => {
                self.visit_metadata(metadata);
                self.nest_expression(None, false);
                self.modifier(*external_keyword);
                self.modifier(*static_keyword);
                self.modifier(*abstract_keyword);
                self.modifier(*covariant_keyword);
                self.visit(Some(*fields));
                self.token(*semicolon);
                self.unnest();
            }
            No::ConstructorFieldInitializer {
                this_keyword,
                period,
                field_name,
                equals,
                expression,
            } => {
                self.nest_expression(None, false);
                self.token_opt(*this_keyword);
                self.token_opt(*period);
                self.visit(Some(*field_name));
                self.visit_assignment(*equals, *expression, false);
                self.unnest();
            }
            No::SuperConstructorInvocation {
                super_keyword: kw,
                period,
                constructor_name,
                argument_list,
            }
            | No::RedirectingConstructorInvocation {
                this_keyword: kw,
                period,
                constructor_name,
                argument_list,
            } => {
                self.start_span(custo::NORMAL);
                self.token(*kw);
                self.token_opt(*period);
                self.visit(*constructor_name);
                self.visit(Some(*argument_list));
                self.end_span();
            }
            No::AssertInitializer {
                assert_keyword,
                left_parenthesis,
                condition,
                message,
                right_parenthesis,
                ..
            } => {
                self.token(*assert_keyword);
                let mut args = vec![*condition];
                if let Some(m) = message {
                    args.push(*m);
                }
                if self.list_has_comma_after(&args) {
                    self.visit_collection_literal(*left_parenthesis, &args, *right_parenthesis, None, None, None, false, false);
                    return;
                }
                self.nest_expression(None, false);
                super::argumentos::ArgumentListVisitor::for_arguments(self, *left_parenthesis, *right_parenthesis, &args).visit(self);
                self.unnest();
            }
            No::BlockFunctionBody { keyword, star, block } => {
                self.space();
                self.token_opt(*keyword);
                self.token_opt(*star);
                if keyword.is_some() {
                    self.space();
                }
                self.visit(Some(*block));
            }
            No::ExpressionFunctionBody { .. } => self.visit_expression_function_body(id),
            No::EmptyFunctionBody { semicolon } | No::EmptyStatement { semicolon } => self.token(*semicolon),
            No::NativeFunctionBody {
                native_keyword,
                string_literal,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.nest_expression(None, true);
                self.solo_split(custo::NORMAL);
                self.token(*native_keyword);
                self.visit_before(*string_literal, Acao::Espaco);
                self.unnest();
                self.token(*semicolon);
                self.unnest();
            }
            No::FormalParameterList { .. } => self.visit_formal_parameter_list(id, true),
            No::SimpleFormalParameter {
                metadata,
                keyword,
                type_,
                name,
                ..
            } => {
                let (metadata, keyword, type_, name) = (metadata.clone(), *keyword, *type_, *name);
                self.visit_parameter_metadata(&metadata, |v| {
                    v.begin_formal_parameter(id);
                    v.modifier(keyword);
                    v.visit(type_);
                    if name.is_some() {
                        v.separator_between_type_and_variable(type_, false);
                    }
                    v.token_opt(name);
                    v.end_formal_parameter();
                });
            }
            No::FieldFormalParameter {
                metadata,
                keyword,
                type_,
                this_keyword: kw,
                period,
                name,
                type_parameters,
                parameters,
                question,
                ..
            }
            | No::SuperFormalParameter {
                metadata,
                keyword,
                type_,
                super_keyword: kw,
                period,
                name,
                type_parameters,
                parameters,
                question,
                ..
            } => {
                let e_super = matches!(n, No::SuperFormalParameter { .. });
                let (keyword, type_, kw, period, name, tps, ps, q) =
                    (*keyword, *type_, *kw, *period, *name, *type_parameters, *parameters, *question);
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.begin_formal_parameter(id);
                    if let Some(k) = keyword {
                        v.token(k);
                        v.space();
                    }
                    if e_super {
                        v.visit_after(type_, Acao::Split);
                    } else {
                        v.visit(type_);
                        v.separator_between_type_and_variable(type_, false);
                    }
                    v.token(kw);
                    v.token(period);
                    v.token(name);
                    v.visit(tps);
                    v.visit(ps);
                    v.token_opt(q);
                    v.end_formal_parameter();
                });
            }
            No::FunctionTypedFormalParameter {
                metadata,
                covariant_keyword,
                required_keyword,
                return_type,
                name,
                type_parameters,
                parameters,
                question,
                ..
            } => {
                let (cov, req, rt, name, tps, ps, q) = (
                    *covariant_keyword,
                    *required_keyword,
                    *return_type,
                    *name,
                    *type_parameters,
                    *parameters,
                    *question,
                );
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.modifier(req);
                    v.modifier(cov);
                    v.visit_after(rt, Acao::Espaco);
                    v.start_span(custo::NORMAL);
                    v.token(name);
                    v.visit_parameter_signature(tps, Some(ps));
                    v.token_opt(q);
                    v.end_span();
                });
            }
            No::DefaultFormalParameter {
                parameter,
                separator,
                default_value,
                ..
            } => {
                self.visit(Some(*parameter));
                if let Some(sep) = separator {
                    self.start_span(custo::NORMAL);
                    self.nest_expression(None, false);
                    if self.e_op(*sep, Op::Assign) {
                        self.space();
                    }
                    self.token(*sep);
                    let dv = default_value.expect("valor padrão");
                    let c = self.assignment_cost(dv);
                    self.solo_split(c);
                    self.visit(Some(dv));
                    self.unnest();
                    self.end_span();
                }
            }
            No::NamedType {
                import_prefix,
                name2,
                type_arguments,
                question,
            } => {
                if let Some((prefixo, ponto)) = import_prefix {
                    self.start_span(custo::NORMAL);
                    self.token(*prefixo);
                    self.solo_zero_split();
                    self.token(*ponto);
                    self.token(*name2);
                    self.end_span();
                } else {
                    self.token(*name2);
                }
                self.visit(*type_arguments);
                self.token_opt(*question);
            }
            No::GenericFunctionType {
                return_type,
                function_keyword,
                type_parameters,
                parameters,
                question,
            } => {
                self.visit_generic_function_type(*return_type, *function_keyword, *type_parameters, *parameters);
                self.token_opt(*question);
            }
            No::RecordTypeAnnotation { .. } => self.visit_record_type_annotation(id),
            No::RecordTypeAnnotationNamedFields { .. } => panic!("visitado pelo tipo de registro"),
            No::RecordTypeAnnotationPositionalField { metadata, type_, name } => {
                let (t, nm) = (*type_, *name);
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.visit(Some(t));
                    v.token_space_before(nm);
                });
            }
            No::RecordTypeAnnotationNamedField { metadata, type_, name } => {
                let (t, nm) = (*type_, *name);
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.visit(Some(t));
                    v.token_space_before(Some(nm));
                });
            }
            No::TypeArgumentList {
                left_bracket,
                arguments,
                right_bracket,
            } => self.visit_generic_list(*left_bracket, *right_bracket, arguments),
            No::TypeParameterList {
                left_bracket,
                type_parameters,
                right_bracket,
            } => self.visit_generic_list(*left_bracket, *right_bracket, type_parameters),
            No::TypeParameter {
                metadata,
                name,
                extends_keyword,
                bound,
            } => {
                let (nm, ext, bd) = (*name, *extends_keyword, *bound);
                let metadata = metadata.clone();
                self.visit_parameter_metadata(&metadata, |v| {
                    v.token(nm);
                    if let Some(e) = ext {
                        v.write_preceding_comments_and_newlines(e);
                        v.space();
                        let t = v.lexema(e);
                        v.write_text(t, e);
                        v.space();
                    }
                    v.visit(bd);
                });
            }
            No::Annotation {
                at_sign,
                name,
                type_arguments,
                period,
                constructor_name,
                arguments,
            } => {
                self.token(*at_sign);
                self.visit(Some(*name));
                self.nest_expression(None, false);
                self.visit(*type_arguments);
                self.token_opt(*period);
                self.visit(*constructor_name);
                if let Some(args) = arguments {
                    self.visit_argument_list(*args, false);
                }
                self.unnest();
            }
            No::Block {
                left_bracket,
                statements,
                right_bracket,
            } => {
                if !self.can_split(statements, *right_bracket) {
                    self.token(*left_bracket);
                    if self.split_empty_block(id) {
                        self.newline();
                    }
                    self.token(*right_bracket);
                    return;
                }
                self.visit_body(*left_bracket, statements, *right_bracket);
            }
            No::ExpressionStatement { expression, semicolon } => {
                self.nest_expression(None, false);
                self.visit(Some(*expression));
                self.token_opt(*semicolon);
                self.unnest();
            }
            No::VariableDeclarationStatement { variables, semicolon } => {
                self.nest_expression(None, false);
                self.visit(Some(*variables));
                self.token(*semicolon);
                self.unnest();
            }
            No::VariableDeclarationList { .. } => self.visit_variable_declaration_list(id),
            No::VariableDeclaration {
                name,
                equals,
                initializer,
            } => {
                self.token(*name);
                let Some(init) = initializer else {
                    return;
                };
                let multiplas = match self.pai(id).map(|p| self.no(p)) {
                    Some(No::VariableDeclarationList { variables, .. }) => variables.len() > 1,
                    _ => false,
                };
                self.visit_assignment(equals.expect("="), *init, multiplas);
            }
            No::FunctionDeclarationStatement { function_declaration } => self.visit(Some(*function_declaration)),
            No::IfStatement { .. } => self.visit_if_statement(id),
            No::CaseClause { .. } | No::GuardedPattern { .. } | No::WhenClause { .. } => {
                panic!("visitado pelo if/case")
            }
            No::ForStatement {
                await_keyword,
                for_keyword,
                left_parenthesis,
                for_loop_parts,
                right_parenthesis,
                body,
            } => {
                self.nest_expression(None, false);
                self.modifier(*await_keyword);
                self.token(*for_keyword);
                self.space();
                self.token(*left_parenthesis);
                self.start_rule(None);
                self.visit(Some(*for_loop_parts));
                self.token(*right_parenthesis);
                self.end_rule();
                self.unnest();
                self.visit_loop_body(*body);
            }
            No::ForPartsWithDeclarations { .. } | No::ForPartsWithExpression { .. } | No::ForPartsWithPattern { .. } => {
                self.visit_for_parts(id)
            }
            No::ForEachPartsWithDeclaration {
                loop_variable,
                in_keyword,
                iterable,
            } => {
                let metadata = match self.no(*loop_variable) {
                    No::DeclaredIdentifier { metadata, .. } => metadata.clone(),
                    _ => Vec::new(),
                };
                self.visit_nodes(&metadata, Acao::Nada, Acao::Split, Acao::Split);
                self.visit(Some(*loop_variable));
                self.visit_for_each_parts_from_in(*in_keyword, *iterable);
            }
            No::ForEachPartsWithIdentifier {
                identifier,
                in_keyword,
                iterable,
            } => {
                self.visit(Some(*identifier));
                self.visit_for_each_parts_from_in(*in_keyword, *iterable);
            }
            No::ForEachPartsWithPattern {
                metadata,
                keyword,
                pattern,
                in_keyword,
                iterable,
            } => {
                self.start_block_argument_nesting();
                self.visit_nodes(metadata, Acao::Nada, Acao::Split, Acao::Split);
                self.token(*keyword);
                self.space();
                self.visit(Some(*pattern));
                self.end_block_argument_nesting();
                self.visit_for_each_parts_from_in(*in_keyword, *iterable);
            }
            No::DeclaredIdentifier { keyword, type_, name, .. } => {
                self.modifier(*keyword);
                self.visit_after(*type_, Acao::Espaco);
                self.token(*name);
            }
            No::WhileStatement {
                while_keyword,
                left_parenthesis,
                condition,
                right_parenthesis,
                body,
            } => {
                self.nest_expression(None, false);
                self.token(*while_keyword);
                self.space();
                self.token(*left_parenthesis);
                self.solo_zero_split();
                self.visit(Some(*condition));
                self.token(*right_parenthesis);
                self.unnest();
                self.visit_loop_body(*body);
            }
            No::DoStatement {
                do_keyword,
                body,
                while_keyword,
                left_parenthesis,
                condition,
                right_parenthesis,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.token(*do_keyword);
                self.space();
                self.unnest_later();
                self.visit(Some(*body));
                self.nest_expression(None, false);
                self.space();
                self.token(*while_keyword);
                self.space();
                self.token(*left_parenthesis);
                self.solo_zero_split();
                self.visit(Some(*condition));
                self.token(*right_parenthesis);
                self.token(*semicolon);
                self.unnest();
            }
            No::SwitchStatement { .. } => self.visit_switch_statement(id),
            No::SwitchCase { .. } | No::SwitchPatternCase { .. } | No::SwitchDefault { .. } => {
                panic!("visitado pelo switch")
            }
            No::TryStatement {
                try_keyword,
                body,
                catch_clauses,
                finally_keyword,
                finally_block,
            } => {
                self.token(*try_keyword);
                self.space();
                self.visit(Some(*body));
                self.visit_nodes(catch_clauses, Acao::Espaco, Acao::Espaco, Acao::Nada);
                if let Some(f) = finally_keyword {
                    self.write_preceding_comments_and_newlines(*f);
                    self.space();
                    let t = self.lexema(*f);
                    self.write_text(t, *f);
                    self.space();
                }
                self.visit(*finally_block);
            }
            No::CatchClause {
                on_keyword,
                exception_type,
                catch_keyword,
                left_parenthesis,
                exception_parameter,
                comma,
                stack_trace_parameter,
                right_parenthesis,
                body,
            } => {
                self.modifier(*on_keyword);
                self.visit(*exception_type);
                if let Some(ck) = catch_keyword {
                    if exception_type.is_some() {
                        self.space();
                    }
                    self.token(*ck);
                    self.space();
                    self.token_opt(*left_parenthesis);
                    self.visit(*exception_parameter);
                    self.modifier(*comma);
                    self.visit(*stack_trace_parameter);
                    self.token_opt(*right_parenthesis);
                    self.space();
                } else {
                    self.space();
                }
                self.visit(Some(*body));
            }
            No::CatchClauseParameter { name } => self.token(*name),
            No::ReturnStatement {
                return_keyword,
                expression,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.token(*return_keyword);
                self.visit_before(*expression, Acao::Espaco);
                self.token(*semicolon);
                self.unnest();
            }
            No::BreakStatement {
                break_keyword: kw,
                label,
                semicolon,
            }
            | No::ContinueStatement {
                continue_keyword: kw,
                label,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.token(*kw);
                self.visit_before(*label, Acao::Espaco);
                self.token(*semicolon);
                self.unnest();
            }
            No::YieldStatement {
                yield_keyword,
                star,
                expression,
                semicolon,
            } => {
                self.nest_expression(None, false);
                self.token(*yield_keyword);
                self.token_opt(*star);
                self.space();
                self.visit(Some(*expression));
                self.token(*semicolon);
                self.unnest();
            }
            No::LabeledStatement { labels, statement } => {
                self.visit_nodes(labels, Acao::Nada, Acao::NovaLinha, Acao::NovaLinha);
                self.visit(Some(*statement));
            }
            No::Label { label, colon } => {
                self.visit(Some(*label));
                self.token(*colon);
            }
            No::AssertStatement {
                assert_keyword,
                left_parenthesis,
                condition,
                message,
                right_parenthesis,
                semicolon,
                ..
            } => {
                self.nest_expression(None, false);
                self.token(*assert_keyword);
                let mut args = vec![*condition];
                if let Some(m) = message {
                    args.push(*m);
                }
                if self.list_has_comma_after(&args) {
                    self.visit_collection_literal(*left_parenthesis, &args, *right_parenthesis, None, None, None, false, false);
                } else {
                    super::argumentos::ArgumentListVisitor::for_arguments(self, *left_parenthesis, *right_parenthesis, &args).visit(self);
                }
                self.token(*semicolon);
                self.unnest();
            }
            No::PatternVariableDeclarationStatement { declaration, semicolon } => {
                self.visit(Some(*declaration));
                self.token(*semicolon);
            }
            No::PatternVariableDeclaration {
                metadata,
                keyword,
                pattern,
                equals,
                expression,
            } => {
                self.visit_metadata(metadata);
                self.nest_expression(None, false);
                self.token(*keyword);
                self.space();
                self.visit(Some(*pattern));
                self.visit_assignment(*equals, *expression, false);
                self.unnest();
            }
            No::AdjacentStrings { .. } => self.visit_adjacent_strings(id),
            No::AsExpression {
                expression,
                as_operator: op,
                type_,
            }
            | No::CastPattern {
                pattern: expression,
                as_token: op,
                type_,
            } => {
                self.start_span(custo::NORMAL);
                self.nest_expression(None, false);
                self.visit(Some(*expression));
                self.solo_split(custo::NORMAL);
                self.token(*op);
                self.space();
                self.visit(Some(*type_));
                self.unnest();
                self.end_span();
            }
            No::AssignmentExpression {
                left_hand_side,
                operator,
                right_hand_side,
            } => {
                self.nest_expression(None, false);
                self.visit(Some(*left_hand_side));
                self.visit_assignment(*operator, *right_hand_side, false);
                self.unnest();
            }
            No::AwaitExpression {
                await_keyword: kw,
                expression,
            }
            | No::ThrowExpression {
                throw_keyword: kw,
                expression,
            } => {
                self.token(*kw);
                self.space();
                self.visit(Some(*expression));
            }
            No::BinaryExpression { operator, .. } => {
                let aninhar = !self.pai_e(id, |p| matches!(p, No::ExpressionFunctionBody { .. }));
                let prec = precedencia(self.lexema(*operator));
                self.visit_binary(id, Some(prec), aninhar);
            }
            No::LogicalAndPattern { .. } | No::LogicalOrPattern { .. } => self.visit_binary(id, None, true),
            No::BooleanLiteral { literal }
            | No::DoubleLiteral { literal }
            | No::IntegerLiteral { literal }
            | No::NullLiteral { literal } => self.token(*literal),
            No::CascadeExpression { .. } => self.visit_cascade_expression(id),
            No::ConditionalExpression { .. } => self.visit_conditional_expression(id),
            No::FunctionExpression {
                type_parameters,
                parameters,
                body,
            } => self.visit_function_body(*type_parameters, *parameters, *body, |_| {}),
            No::FunctionExpressionInvocation {
                function,
                type_arguments,
                argument_list,
            } => {
                self.start_span(custo::NORMAL);
                self.nest_expression(None, false);
                self.visit(Some(*function));
                self.visit(*type_arguments);
                self.visit_argument_list(*argument_list, false);
                self.unnest();
                self.end_span();
            }
            No::FunctionReference {
                function,
                type_arguments,
            } => {
                self.visit(Some(*function));
                self.visit(*type_arguments);
            }
            No::IndexExpression { target, period, .. } => {
                self.nest_expression(None, false);
                if target.is_none() && period.is_some() {
                    self.token_opt(*period);
                } else {
                    self.visit(*target);
                }
                self.finish_index_expression(id);
                self.unnest();
            }
            No::InstanceCreationExpression {
                keyword,
                constructor_name,
                argument_list,
            } => {
                self.start_span(custo::NORMAL);
                self.modifier(*keyword);
                self.start_span(custo::CONSTRUCTOR_NAME);
                self.nest_expression(None, false);
                self.visit(Some(*constructor_name));
                self.end_span();
                self.visit_argument_list(*argument_list, false);
                self.end_span();
                self.unnest();
            }
            No::ConstructorName { type_, period, name } => {
                self.visit(Some(*type_));
                self.token_opt(*period);
                self.visit(*name);
            }
            No::IsExpression {
                expression,
                is_operator,
                not_operator,
                type_,
            } => {
                self.start_span(custo::NORMAL);
                self.nest_expression(None, false);
                self.visit(Some(*expression));
                self.solo_split(custo::NORMAL);
                self.token(*is_operator);
                self.token_opt(*not_operator);
                self.space();
                self.visit(Some(*type_));
                self.unnest();
                self.end_span();
            }
            No::ListLiteral {
                const_keyword,
                type_arguments,
                left_bracket,
                elements,
                right_bracket,
            } => {
                let c = if elements.len() <= 1 { custo::SINGLE_ELEMENT_LIST } else { custo::NORMAL };
                self.visit_collection_literal(*left_bracket, elements, *right_bracket, *const_keyword, *type_arguments, Some(c), true, false);
            }
            No::SetOrMapLiteral {
                const_keyword,
                type_arguments,
                left_bracket,
                elements,
                right_bracket,
            } => self.visit_collection_literal(*left_bracket, elements, *right_bracket, *const_keyword, *type_arguments, None, true, false),
            No::ListPattern {
                type_arguments,
                left_bracket,
                elements,
                right_bracket,
            }
            | No::MapPattern {
                type_arguments,
                left_bracket,
                elements,
                right_bracket,
            } => self.visit_collection_literal(*left_bracket, elements, *right_bracket, None, *type_arguments, None, false, false),
            No::MapLiteralEntry { key, separator, value } | No::MapPatternEntry { key, separator, value } => {
                self.nest_expression(None, false);
                self.visit(Some(*key));
                self.token(*separator);
                self.solo_split(custo::NORMAL);
                self.visit(Some(*value));
                self.unnest();
            }
            No::MethodInvocation { .. } => self.visit_method_invocation(id),
            No::NamedExpression { .. } => {
                let (nome, dois_pontos, expr) = self.named_parts(id);
                self.visit_named_node(nome, dois_pontos, expr, None);
            }
            No::ParenthesizedExpression {
                left_parenthesis,
                expression,
                right_parenthesis,
            }
            | No::ParenthesizedPattern {
                left_parenthesis,
                pattern: expression,
                right_parenthesis,
            } => {
                self.nest_expression(None, false);
                self.token(*left_parenthesis);
                self.visit(Some(*expression));
                self.unnest();
                self.token(*right_parenthesis);
            }
            No::PostfixExpression { operand, operator } => {
                self.visit(Some(*operand));
                self.token(*operator);
            }
            No::PrefixExpression { operator, operand } => {
                self.token(*operator);
                if let No::PrefixExpression { operator: op2, .. } = self.no(*operand) {
                    let l = self.lexema(*op2);
                    if l == "-" || l == "--" {
                        self.space();
                    }
                }
                self.visit(Some(*operand));
            }
            No::PrefixedIdentifier { .. } => super::cadeia::CallChainVisitor::novo(self, id).visit(self),
            No::PropertyAccess {
                target,
                operator,
                property_name,
            } => {
                if target.is_none() {
                    self.token(*operator);
                    self.visit(Some(*property_name));
                    return;
                }
                super::cadeia::CallChainVisitor::novo(self, id).visit(self);
            }
            No::RecordLiteral {
                const_keyword,
                left_parenthesis,
                fields,
                right_parenthesis,
            } => {
                self.modifier(*const_keyword);
                self.visit_collection_literal(*left_parenthesis, fields, *right_parenthesis, None, None, None, false, true);
            }
            No::RecordPattern {
                left_parenthesis,
                fields,
                right_parenthesis,
            } => self.visit_collection_literal(*left_parenthesis, fields, *right_parenthesis, None, None, None, false, true),
            No::RethrowExpression { rethrow_keyword } => self.token(*rethrow_keyword),
            No::SimpleIdentifier { token } => self.token(*token),
            No::SimpleStringLiteral { literal } => self.write_string_literal(*literal),
            No::StringInterpolation { elements } => {
                for &e in elements {
                    self.visit(Some(e));
                }
            }
            No::InterpolationExpression {
                left_bracket,
                expression,
                right_bracket,
            } => {
                self.b().0.impedir_quebras();
                self.token(*left_bracket);
                self.start_span(custo::NORMAL);
                self.visit(Some(*expression));
                self.end_span();
                self.token_opt(*right_bracket);
                self.b().0.liberar_quebras();
            }
            No::InterpolationString { contents } => self.write_string_literal(*contents),
            No::SuperExpression { super_keyword: kw } | No::ThisExpression { this_keyword: kw } => self.token(*kw),
            No::SymbolLiteral { pound_sign, components } => {
                self.token(*pound_sign);
                for &c in components {
                    if c > 0 && self.lexema(c - 1) == "." {
                        self.token(c - 1);
                    }
                    self.token(c);
                }
            }
            No::SwitchExpression { .. } => self.visit_switch_expression(id),
            No::SwitchExpressionCase { .. } => self.visit_switch_expression_case(id),
            No::ArgumentList { .. } => self.visit_argument_list(id, true),
            No::SpreadElement {
                spread_operator,
                expression,
            } => {
                self.token(*spread_operator);
                self.visit(Some(*expression));
            }
            No::IfElement { .. } => self.visit_if_element(id),
            No::ForElement { .. } => self.visit_for_element(id),
            No::AssignedVariablePattern { name } => self.token(*name),
            No::ConstantPattern {
                const_keyword,
                expression,
            } => {
                self.modifier(*const_keyword);
                self.visit(Some(*expression));
            }
            No::DeclaredVariablePattern { keyword, type_, name } | No::WildcardPattern { keyword, type_, name } => {
                self.modifier(*keyword);
                if let Some(t) = type_ {
                    self.visit(Some(*t));
                    self.solo_split(custo::NORMAL);
                }
                self.token(*name);
            }
            No::NullAssertPattern { pattern, operator } | No::NullCheckPattern { pattern, operator } => {
                self.visit(Some(*pattern));
                self.token(*operator);
            }
            No::ObjectPattern {
                type_,
                left_parenthesis,
                fields,
                right_parenthesis,
            } => {
                self.visit(Some(*type_));
                self.visit_collection_literal(*left_parenthesis, fields, *right_parenthesis, None, None, None, false, false);
            }
            No::PatternField { name, pattern } => match name.map(|n| self.no(n)) {
                Some(No::PatternFieldName { name: Some(nm), colon }) => {
                    self.visit_named_node(*nm, *colon, *pattern, None);
                }
                Some(No::PatternFieldName { name: None, colon }) => {
                    self.token(*colon);
                    self.visit(Some(*pattern));
                }
                _ => self.visit(Some(*pattern)),
            },
            No::PatternFieldName { .. } => panic!("visitado pelo campo"),
            No::RelationalPattern { operator, operand } => {
                self.token(*operator);
                self.space();
                self.visit(Some(*operand));
            }
            No::RestPatternElement { operator, pattern } => {
                self.token(*operator);
                self.visit(*pattern);
            }
            No::PatternAssignment {
                pattern,
                equals,
                expression,
            } => {
                self.visit(Some(*pattern));
                self.visit_assignment(*equals, *expression, false);
            }
        }
    }

    // ======================================================== declarações

    fn visit_compilation_unit(&mut self, id: NoId) {
        let No::CompilationUnit {
            script_tag,
            directives,
            declarations,
        } = self.no(id)
        else {
            unreachable!()
        };
        if let Some(st) = script_tag {
            let t = self.lexema(*st).trim();
            self.write_text(t, *st);
            self.two_newlines();
        }
        let mut diretivas: &[NoId] = directives;
        if let Some(&primeira) = diretivas.first()
            && matches!(self.no(primeira), No::LibraryDirective { .. })
        {
            self.visit(Some(primeira));
            self.two_newlines();
            diretivas = &diretivas[1..];
        }
        self.visit_nodes(diretivas, Acao::Nada, Acao::OneOrTwoNewlines, Acao::Nada);

        let mut precisa_dupla = true;
        for &d in declarations {
            let tem_corpo = matches!(
                self.no(d),
                No::ClassDeclaration { .. } | No::EnumDeclaration { .. } | No::ExtensionDeclaration { .. }
            );
            if tem_corpo {
                precisa_dupla = true;
            }
            if precisa_dupla {
                self.two_newlines();
            } else {
                self.one_or_two_newlines();
            }
            self.visit(Some(d));
            precisa_dupla = false;
            if tem_corpo {
                precisa_dupla = true;
            } else if matches!(self.no(d), No::FunctionDeclaration { .. })
                && let Some(corpo) = self.corpo_de_declaracao(d)
                && matches!(self.no(corpo), No::BlockFunctionBody { .. })
            {
                precisa_dupla = self.bloco_nao_vazio(corpo);
            }
        }
    }

    /// `_visitDirectiveMetadata(directive)`.
    fn visit_directive_metadata(&mut self, id: NoId, metadata: &[NoId]) {
        let primeira = match self.pai(id).map(|p| self.no(p)) {
            Some(No::CompilationUnit { directives, .. }) => directives.first() == Some(&id),
            _ => false,
        };
        let depois = if primeira { Acao::OneOrTwoNewlines } else { Acao::NovaLinha };
        self.visit_nodes(metadata, Acao::Nada, Acao::NovaLinha, depois);
    }

    fn visit_import_directive(&mut self, id: NoId) {
        let No::ImportDirective {
            metadata,
            import_keyword,
            uri,
            configurations,
            deferred_keyword,
            as_keyword,
            prefix,
            combinators,
            semicolon,
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_directive_metadata(id, metadata);
        self.nest_expression(None, false);
        self.token(*import_keyword);
        self.space();
        self.visit(Some(*uri));
        let mut escreveu_config = false;
        if let Some(asw) = as_keyword
            && let Some(&c0) = configurations.first()
            && let No::Configuration { if_keyword, .. } = self.no(c0)
            && self.tk.lista[*if_keyword as usize].inicio < self.tk.lista[*asw as usize].inicio
        {
            self.visit_configurations(configurations);
            escreveu_config = true;
        }
        if let Some(asw) = as_keyword {
            self.solo_split(custo::NORMAL);
            self.modifier(*deferred_keyword);
            self.token(*asw);
            self.space();
            self.visit(*prefix);
        }
        if !escreveu_config {
            self.visit_configurations(configurations);
        }
        self.visit_combinators(combinators);
        self.token(*semicolon);
        self.unnest();
    }

    fn visit_configurations(&mut self, configs: &[NoId]) {
        if configs.is_empty() {
            return;
        }
        self.start_rule(None);
        for &c in configs {
            self.split();
            self.visit(Some(c));
        }
        self.end_rule();
    }

    fn visit_combinators(&mut self, combinators: &[NoId]) {
        let r = self.a.regra_combinador();
        self.start_rule(Some(r));
        self.visit_nodes(combinators, Acao::Nada, Acao::Nada, Acao::Nada);
        self.end_rule();
    }

    /// `_visitCombinator(keyword, nodes)`.
    fn visit_combinator(&mut self, keyword: Tk, nos: &[NoId]) {
        let regra = self.current_rule();
        debug_assert!(self.a.e_combinador(regra));
        let c = self.split();
        self.a.adicionar_combinador(regra, c);
        self.nest_expression(None, false);
        self.token(keyword);
        let c = self.split();
        self.a.adicionar_nome(regra, c);
        for (i, &n) in nos.iter().enumerate() {
            if i > 0 {
                let c = self.split();
                self.a.adicionar_nome(regra, c);
            }
            self.aceitar(n);
            let proximo = self.arv.fim(n) + 1;
            if self.lexema(proximo) == "," {
                self.token(proximo);
            }
        }
        self.unnest();
    }

    /// `_visitClauses(withClause, implementsClause)`.
    fn visit_clauses(&mut self, with: Option<NoId>, implements: Option<NoId>) {
        let r = self.a.regra_combinador();
        self.start_rule(Some(r));
        self.visit(with);
        self.visit(implements);
        self.end_rule();
    }

    fn visit_class_declaration(&mut self, id: NoId) {
        let No::ClassDeclaration {
            metadata,
            abstract_keyword,
            macro_keyword,
            sealed_keyword,
            base_keyword,
            interface_keyword,
            final_keyword,
            mixin_keyword,
            class_keyword,
            name,
            type_parameters,
            extends_clause,
            with_clause,
            implements_clause,
            native_clause,
            left_bracket,
            members,
            right_bracket,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.modifier(*abstract_keyword);
        self.modifier(*base_keyword);
        self.modifier(*interface_keyword);
        self.modifier(*final_keyword);
        self.modifier(*sealed_keyword);
        self.modifier(*mixin_keyword);
        self.modifier(*macro_keyword);
        self.token(*class_keyword);
        self.space();
        self.token(*name);
        self.visit(*type_parameters);
        self.visit(*extends_clause);
        self.visit_clauses(*with_clause, *implements_clause);
        self.visit_before(*native_clause, Acao::Espaco);
        self.space();
        self.unnest();
        self.visit_body(*left_bracket, members, *right_bracket);
    }

    fn visit_class_type_alias(&mut self, id: NoId) {
        let No::ClassTypeAlias {
            metadata,
            abstract_keyword,
            sealed_keyword,
            base_keyword,
            interface_keyword,
            final_keyword,
            mixin_keyword,
            typedef_keyword,
            name,
            type_parameters,
            equals,
            superclass,
            with_clause,
            implements_clause,
            semicolon,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.modifier(*abstract_keyword);
        self.modifier(*base_keyword);
        self.modifier(*interface_keyword);
        self.modifier(*final_keyword);
        self.modifier(*sealed_keyword);
        self.modifier(*mixin_keyword);
        self.token(*typedef_keyword);
        self.space();
        self.token(*name);
        self.visit(*type_parameters);
        self.space();
        self.token(*equals);
        self.space();
        self.visit(Some(*superclass));
        self.visit_clauses(*with_clause, *implements_clause);
        self.token(*semicolon);
        self.unnest();
    }

    fn visit_mixin_declaration(&mut self, id: NoId) {
        let No::MixinDeclaration {
            metadata,
            base_keyword,
            mixin_keyword,
            name,
            type_parameters,
            on_clause,
            implements_clause,
            left_bracket,
            members,
            right_bracket,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.modifier(*base_keyword);
        self.token(*mixin_keyword);
        self.space();
        self.token(*name);
        self.visit(*type_parameters);
        let restricoes: &[NoId] = match on_clause.map(|o| self.no(o)) {
            Some(No::MixinOnClause {
                superclass_constraints, ..
            }) => superclass_constraints,
            _ => &[],
        };
        if let Some(o) = on_clause
            && restricoes.len() == 1
            && let No::MixinOnClause { on_keyword, .. } = self.no(*o)
        {
            self.solo_split(custo::NORMAL);
            self.token(*on_keyword);
            self.space();
            self.visit(Some(restricoes[0]));
        }
        let r = self.a.regra_combinador();
        self.start_rule(Some(r));
        if restricoes.len() > 1 {
            self.visit(*on_clause);
        }
        self.visit(*implements_clause);
        self.end_rule();
        self.space();
        self.unnest();
        self.visit_body(*left_bracket, members, *right_bracket);
    }

    fn visit_extension_declaration(&mut self, id: NoId) {
        let No::ExtensionDeclaration {
            metadata,
            extension_keyword,
            name,
            type_parameters,
            on_clause,
            left_bracket,
            members,
            right_bracket,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.token(*extension_keyword);
        self.token_space_before(*name);
        self.visit(*type_parameters);
        if let Some(o) = on_clause
            && let No::ExtensionOnClause { on_keyword, extended_type } = self.no(*o)
        {
            self.solo_split(custo::NORMAL);
            self.token(*on_keyword);
            self.space();
            self.visit(Some(*extended_type));
        }
        self.space();
        self.unnest();
        self.visit_body(*left_bracket, members, *right_bracket);
    }

    fn visit_extension_type_declaration(&mut self, id: NoId) {
        let No::ExtensionTypeDeclaration {
            metadata,
            extension_keyword,
            type_keyword,
            const_keyword,
            name,
            type_parameters,
            representation,
            implements_clause,
            left_bracket,
            members,
            right_bracket,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.token(*extension_keyword);
        self.space();
        self.token(*type_keyword);
        self.token_space_before(*const_keyword);
        self.space();
        self.token(*name);
        self.nest_expression(None, false);
        self.visit(*type_parameters);
        self.visit(Some(*representation));
        self.unnest();
        let r = self.a.regra_combinador();
        self.start_rule(Some(r));
        self.visit(*implements_clause);
        self.end_rule();
        self.space();
        self.unnest();
        self.visit_body(*left_bracket, members, *right_bracket);
    }

    fn visit_representation_declaration(&mut self, id: NoId) {
        let No::RepresentationDeclaration {
            constructor_name,
            left_parenthesis,
            field_metadata,
            field_type,
            field_name,
            right_parenthesis,
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit(*constructor_name);
        self.token(*left_parenthesis);
        let regra = self.a.regra_posicional(None, 1, 0, 0);
        self.start_rule(Some(regra));
        let c = self.zero_split();
        self.a.antes_do_argumento(regra, Some(c));
        self.start_block_argument_nesting();
        self.start_span(custo::NORMAL);
        let (ft, fname) = (*field_type, *field_name);
        self.visit_parameter_metadata(field_metadata, |v| {
            let r = v.a.regra(custo::PARAMETER_TYPE);
            v.start_lazy_rule(Some(r));
            v.nest_expression(None, false);
            v.visit(Some(ft));
            v.separator_between_type_and_variable(Some(ft), false);
            v.token(fname);
            v.unnest();
            v.end_rule();
        });
        self.end_block_argument_nesting();
        self.end_span();
        self.end_rule();
        self.token(*right_parenthesis);
    }

    fn visit_enum_declaration(&mut self, id: NoId) {
        let No::EnumDeclaration {
            metadata,
            enum_keyword,
            name,
            type_parameters,
            with_clause,
            implements_clause,
            left_bracket,
            constants,
            members,
            right_bracket,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.token(*enum_keyword);
        self.space();
        self.token(*name);
        self.visit(*type_parameters);
        self.visit_clauses(*with_clause, *implements_clause);
        self.space();
        self.unnest();
        self.begin_body(*left_bracket, true);
        self.visit_comma_separated_nodes(constants, Acao::SplitOrTwoNewlines);
        let ultima = *constants.last().expect("enum com constantes");
        let virgula_final = self.comma_after(ultima);
        if virgula_final.is_some() {
            self.force_rules();
        }
        let depois = self.arv.fim(ultima) + 1;
        let mut ponto_e_virgula = None;
        if self.e_op(depois, Op::Semicolon) {
            ponto_e_virgula = Some(depois);
        } else if let Some(v) = virgula_final
            && self.e_op(v + 1, Op::Semicolon)
        {
            ponto_e_virgula = Some(depois + 1);
        }
        if let Some(pv) = ponto_e_virgula {
            if virgula_final.is_some() {
                self.newline();
            }
            self.token(pv);
            if !members.is_empty() {
                self.two_newlines();
            }
        }
        self.visit_body_contents(members);
        let forcar = ponto_e_virgula.is_some()
            || virgula_final.is_some()
            || !members.is_empty()
            || self.contains_line_comments(constants, None);
        self.end_body(*right_bracket, forcar);
    }

    fn visit_constructor_declaration(&mut self, id: NoId) {
        let No::ConstructorDeclaration {
            metadata,
            external_keyword,
            const_keyword,
            factory_keyword,
            return_type,
            period,
            name,
            parameters,
            initializers,
            redirected_constructor,
            body,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.modifier(*external_keyword);
        self.modifier(*const_keyword);
        self.modifier(*factory_keyword);
        self.visit(Some(*return_type));
        self.token_opt(*period);
        self.token_opt(*name);
        if !initializers.is_empty() {
            self.start_rule(None);
        }
        if redirected_constructor.is_some() {
            self.nest_expression(None, false);
        }
        let redir = *redirected_constructor;
        let tem_inicializadores = !initializers.is_empty();
        self.visit_function_body(None, Some(*parameters), *body, |v| {
            if redir.is_some() {
                v.visit_constructor_redirects(id);
                v.unnest();
            } else if tem_inicializadores {
                v.visit_constructor_initializers(id);
                v.end_rule();
            }
        });
    }

    fn visit_constructor_redirects(&mut self, id: NoId) {
        let No::ConstructorDeclaration {
            separator,
            initializers,
            redirected_constructor,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        self.token_space_before(*separator);
        self.solo_split(custo::NORMAL);
        self.visit_comma_separated_nodes(initializers, Acao::Espaco);
        self.visit(*redirected_constructor);
    }

    fn visit_constructor_initializers(&mut self, id: NoId) {
        let No::ConstructorDeclaration {
            parameters,
            separator,
            initializers,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        let params: &[NoId] = match self.no(*parameters) {
            No::FormalParameterList { parameters, .. } => parameters,
            _ => &[],
        };
        let virgula_final = self.list_has_comma_after(params);
        let sep = separator.expect("separador dos inicializadores");
        if virgula_final {
            self.space();
            if initializers.len() > 1 {
                let ultimo = *params.last().expect("parâmetro");
                let k = self.param_kind(ultimo);
                let enchimento = if k == TipoParametro::Nomeado || k == TipoParametro::OpcionalPosicional {
                    " "
                } else {
                    "  "
                };
                self.write_text(enchimento, sep);
            }
            self.token(sep);
            self.space();
            self.indent(Some(6));
        } else {
            self.indent(Some(recuo::CONSTRUCTOR_INITIALIZER));
            self.split();
            self.token(sep);
            self.space();
            self.indent(Some(2));
        }
        for (i, &ini) in initializers.iter().enumerate() {
            if i > 0 {
                let virgula = self.arv.inicio(ini) - 1;
                self.token(virgula);
                self.newline();
            }
            self.aceitar(ini);
        }
        self.unindent();
        if !virgula_final {
            self.unindent();
        }
    }

    /// `_visitFunctionOrMethodDeclaration`.
    #[allow(clippy::too_many_arguments)]
    fn visit_function_or_method_declaration(
        &mut self,
        metadata: &[NoId],
        external_keyword: Option<Tk>,
        property_keyword: Option<Tk>,
        modifier_keyword: Option<Tk>,
        operator_keyword: Option<Tk>,
        name: Tk,
        return_type: Option<NoId>,
        type_parameters: Option<NoId>,
        formal_parameters: Option<NoId>,
        body: NoId,
    ) {
        self.visit_metadata(metadata);
        self.nest_expression(None, false);
        self.start_span(custo::NORMAL);
        self.modifier(external_keyword);
        self.modifier(modifier_keyword);
        if let Some(rt) = return_type {
            self.visit(Some(rt));
            self.solo_split(custo::NORMAL);
        }
        self.modifier(property_keyword);
        self.modifier(operator_keyword);
        self.token(name);
        self.end_span();
        let expr = matches!(self.no(body), No::ExpressionFunctionBody { .. });
        self.visit_function_body(type_parameters, formal_parameters, body, |v| {
            if !expr {
                v.unnest();
            }
        });
        if expr {
            self.unnest();
        }
    }

    /// `_visitFunctionBody(typeParameters, parameters, body, beforeBody)`.
    fn visit_function_body(&mut self, type_parameters: Option<NoId>, parameters: Option<NoId>, body: NoId, antes_do_corpo: impl FnOnce(&mut Self)) {
        let expr = matches!(self.no(body), No::ExpressionFunctionBody { .. });
        if expr {
            self.nest_expression(None, false);
            let r = self.a.regra(custo::ARROW);
            self.start_lazy_rule(Some(r));
        }
        self.visit_parameter_signature(type_parameters, parameters);
        antes_do_corpo(self);
        self.visit(Some(body));
        if expr {
            self.unnest();
        }
    }

    /// `_visitParameterSignature(typeParameters, parameters)`.
    fn visit_parameter_signature(&mut self, type_parameters: Option<NoId>, parameters: Option<NoId>) {
        self.nest_expression(None, false);
        self.visit(type_parameters);
        if let Some(p) = parameters {
            self.visit_formal_parameter_list(p, false);
        }
        self.unnest();
    }

    fn visit_expression_function_body(&mut self, id: NoId) {
        let No::ExpressionFunctionBody {
            keyword,
            star,
            function_definition,
            expression,
            semicolon,
        } = self.no(id)
        else {
            unreachable!()
        };
        self.space();
        self.token_opt(*keyword);
        self.token_opt(*star);
        if keyword.is_some() || star.is_some() {
            self.space();
        }
        let corpo_de_funcao = self.is_function_expression_body(id);
        if corpo_de_funcao {
            self.start_span(custo::NORMAL);
        }
        self.token(*function_definition);
        self.split();
        let binaria = matches!(self.no(*expression), No::BinaryExpression { .. });
        if !binaria {
            self.end_rule();
        }
        if corpo_de_funcao {
            self.end_span();
        }
        let mut arg_com_virgula = false;
        if let Some(p) = self.pai(id)
            && matches!(self.no(p), No::FunctionExpression { .. })
        {
            arg_com_virgula = self.is_trailing_comma_argument(p);
        }
        if !arg_com_virgula {
            self.start_block_argument_nesting();
        }
        self.start_span(custo::NORMAL);
        self.visit(Some(*expression));
        self.end_span();
        if !arg_com_virgula {
            self.end_block_argument_nesting();
        }
        if binaria {
            self.end_rule();
        }
        self.token_opt(*semicolon);
    }

    // ============================================================ parâmetros

    /// `visitFormalParameterList(node, nestExpression:)`.
    fn visit_formal_parameter_list(&mut self, id: NoId, aninhar: bool) {
        let No::FormalParameterList {
            left_parenthesis,
            parameters,
            left_delimiter,
            right_delimiter,
            right_parenthesis,
        } = self.no(id)
        else {
            unreachable!()
        };
        if parameters.is_empty() {
            self.token(*left_parenthesis);
            if self.tem_comentarios(*right_parenthesis) {
                self.solo_zero_split();
            }
            self.token(*right_parenthesis);
            return;
        }
        if self.list_has_comma_after(parameters) {
            self.visit_trailing_comma_parameter_list(id);
            return;
        }
        let obrigatorios: Vec<NoId> = parameters
            .iter()
            .copied()
            .filter(|&p| !matches!(self.no(p), No::DefaultFormalParameter { .. }))
            .collect();
        let opcionais: Vec<NoId> = parameters
            .iter()
            .copied()
            .filter(|&p| matches!(self.no(p), No::DefaultFormalParameter { .. }))
            .collect();
        if aninhar {
            self.nest_expression(None, false);
        }
        self.token(*left_parenthesis);
        let mut regra = None;
        if !obrigatorios.is_empty() {
            let r = self.a.regra_posicional(None, obrigatorios.len(), 0, 0);
            regra = Some(r);
            self.start_rule(Some(r));
            if self.is_function_expression_body(id) {
                self.a.antes_do_argumento(r, None);
            } else {
                let c = self.zero_split();
                self.a.antes_do_argumento(r, Some(c));
            }
            self.start_block_argument_nesting();
            self.start_span(custo::NORMAL);
            for (i, &p) in obrigatorios.iter().enumerate() {
                self.visit(Some(p));
                self.write_comma_after(p);
                if i + 1 != obrigatorios.len() {
                    let c = self.split();
                    self.a.antes_do_argumento(r, Some(c));
                }
            }
            self.end_block_argument_nesting();
            self.end_span();
            self.end_rule();
        }
        if !opcionais.is_empty() {
            let nomeada = self.a.regra_nomeada(None, 0, 0);
            if let Some(r) = regra {
                self.a.restricoes_de_nomeados(r, nomeada);
            }
            self.start_rule(Some(nomeada));
            self.start_block_argument_nesting();
            let c = self.builder_split(true, !obrigatorios.is_empty());
            self.a.antes_do_argumento(nomeada, Some(c));
            self.token_opt(*left_delimiter);
            for (i, &p) in opcionais.iter().enumerate() {
                self.visit(Some(p));
                self.write_comma_after(p);
                if i + 1 != opcionais.len() {
                    let c = self.split();
                    self.a.antes_do_argumento(nomeada, Some(c));
                }
            }
            self.end_block_argument_nesting();
            self.end_rule();
            self.token_opt(*right_delimiter);
        }
        self.token(*right_parenthesis);
        if aninhar {
            self.unnest();
        }
    }

    /// `_visitTrailingCommaParameterList(parameters)`.
    fn visit_trailing_comma_parameter_list(&mut self, id: NoId) {
        let No::FormalParameterList {
            left_parenthesis,
            parameters,
            left_delimiter,
            right_delimiter,
            right_parenthesis,
        } = self.no(id)
        else {
            unreachable!()
        };
        let dura = self.a.regra_dura();
        self.start_rule(Some(dura));
        self.token(*left_parenthesis);
        let mut ultimo_obrigatorio = None;
        for (i, &p) in parameters.iter().enumerate() {
            if matches!(self.no(p), No::DefaultFormalParameter { .. }) {
                if i > 0 {
                    ultimo_obrigatorio = Some(parameters[i - 1]);
                }
                break;
            }
        }
        if matches!(self.no(parameters[0]), No::DefaultFormalParameter { .. }) {
            self.token_opt(*left_delimiter);
        }
        self.start_block(None, true, false);
        for &p in parameters {
            self.write_newline(false, false, false);
            self.visit(Some(p));
            self.write_comma_after(p);
            if Some(p) == ultimo_obrigatorio {
                self.space();
                self.token_opt(*left_delimiter);
                ultimo_obrigatorio = None;
            }
        }
        let primeiro_delim = right_delimiter.unwrap_or(*right_parenthesis);
        if self.tem_comentarios(primeiro_delim) {
            self.write_newline(false, false, false);
            self.write_preceding_comments_and_newlines(primeiro_delim);
        }
        self.end_block(true);
        self.end_rule();
        let t = self.lexema(primeiro_delim);
        self.write_text(t, primeiro_delim);
        if primeiro_delim != *right_parenthesis {
            self.token(*right_parenthesis);
        }
    }

    /// `_beginFormalParameter(node)`.
    fn begin_formal_parameter(&mut self, id: NoId) {
        let r = self.a.regra(custo::PARAMETER_TYPE);
        self.start_lazy_rule(Some(r));
        self.nest_expression(None, false);
        let (req, cov) = match self.no(id) {
            No::SimpleFormalParameter {
                required_keyword,
                covariant_keyword,
                ..
            }
            | No::FieldFormalParameter {
                required_keyword,
                covariant_keyword,
                ..
            }
            | No::SuperFormalParameter {
                required_keyword,
                covariant_keyword,
                ..
            }
            | No::FunctionTypedFormalParameter {
                required_keyword,
                covariant_keyword,
                ..
            } => (*required_keyword, *covariant_keyword),
            _ => (None, None),
        };
        self.modifier(req);
        self.modifier(cov);
    }

    /// `_endFormalParameter(node)`.
    fn end_formal_parameter(&mut self) {
        self.unnest();
        self.end_rule();
    }

    /// `visitParameterMetadata(metadata, visitParameter)`.
    fn visit_parameter_metadata(&mut self, metadata: &[NoId], visitar: impl FnOnce(&mut Self)) {
        if metadata.is_empty() {
            visitar(self);
            return;
        }
        self.start_lazy_rule(None);
        self.visit_nodes(metadata, Acao::Nada, Acao::Split, Acao::Split);
        visitar(self);
        self.end_rule();
    }

    /// `_separatorBetweenTypeAndVariable(type, isSolo:)`.
    fn separator_between_type_and_variable(&mut self, tipo: Option<NoId>, solo: bool) {
        let Some(t) = tipo else {
            return;
        };
        let tipo_bloco = match self.no(t) {
            No::GenericFunctionType { parameters, .. } => match self.no(*parameters) {
                No::FormalParameterList { parameters, .. } => !parameters.is_empty() && self.list_has_comma_after(parameters),
                _ => false,
            },
            No::RecordTypeAnnotation { .. } => true,
            _ => false,
        };
        if tipo_bloco {
            self.space();
        } else if solo {
            self.solo_split(custo::NORMAL);
        } else {
            self.split();
        }
    }

    /// `_visitGenericFunctionType`.
    fn visit_generic_function_type(&mut self, return_type: Option<NoId>, function_keyword: Tk, type_parameters: Option<NoId>, parameters: NoId) {
        self.start_lazy_rule(None);
        self.nest_expression(None, false);
        self.visit_after(return_type, Acao::Split);
        self.token(function_keyword);
        self.unnest();
        self.end_rule();
        self.visit_parameter_signature(type_parameters, Some(parameters));
    }

    /// `_visitGenericTypeAliasHeader`.
    fn visit_generic_type_alias_header(&mut self, typedef_keyword: Tk, name: Tk, type_parameters: Option<NoId>, equals: Tk) {
        self.token(typedef_keyword);
        self.space();
        self.start_rule(None);
        self.token(name);
        self.visit(type_parameters);
        self.split();
        self.token(equals);
        self.end_rule();
    }

    fn visit_record_type_annotation(&mut self, id: NoId) {
        let No::RecordTypeAnnotation {
            left_parenthesis,
            positional_fields,
            named_fields,
            right_parenthesis,
            question,
        } = self.no(id)
        else {
            unreachable!()
        };
        if positional_fields.is_empty() && named_fields.is_none() {
            self.token(*left_parenthesis);
            if self.tem_comentarios(*right_parenthesis) {
                self.solo_zero_split();
            }
            self.token(*right_parenthesis);
            self.token_opt(*question);
            return;
        }
        let nomeados = named_fields.map(|n| match self.no(n) {
            No::RecordTypeAnnotationNamedFields {
                left_bracket,
                fields,
                right_bracket,
            } => (*left_bracket, fields, *right_bracket),
            _ => unreachable!(),
        });
        self.token(*left_parenthesis);
        self.start_rule(None);
        if positional_fields.is_empty()
            && let Some((lb, _, _)) = nomeados
        {
            self.token(lb);
        }
        self.start_block(None, true, false);
        for (i, &f) in positional_fields.iter().enumerate() {
            self.builder_split(false, i != 0);
            self.visit(Some(f));
            self.write_comma_after(f);
        }
        let mut primeiro_fecho = *right_parenthesis;
        if let Some((lb, campos, rb)) = nomeados {
            if !positional_fields.is_empty() {
                self.space();
                self.token(lb);
            }
            for (i, &f) in campos.iter().enumerate() {
                self.builder_split(false, i != 0);
                self.visit(Some(f));
                self.write_comma_after(f);
            }
            primeiro_fecho = rb;
        }
        if self.tem_comentarios(primeiro_fecho) {
            self.newline();
            self.write_preceding_comments_and_newlines(primeiro_fecho);
        }
        let forcar = match nomeados {
            None => positional_fields.len() > 1 && self.has_comma_after(*positional_fields.last().expect("campo")),
            Some((_, campos, _)) => campos.last().is_some_and(|&c| self.has_comma_after(c)),
        };
        self.end_block(forcar);
        self.end_rule();
        let t = self.lexema(primeiro_fecho);
        self.write_text(t, primeiro_fecho);
        if nomeados.is_some() {
            self.token(*right_parenthesis);
        }
        self.token_opt(*question);
    }

    /// `_visitGenericList(leftBracket, rightBracket, nodes)`.
    fn visit_generic_list(&mut self, esquerdo: Tk, direito: Tk, nos: &[NoId]) {
        let regra = self.a.regra_argumento_de_tipo();
        self.start_lazy_rule(Some(regra));
        self.start_span(custo::NORMAL);
        self.nest_expression(None, false);
        self.token(esquerdo);
        let c = self.zero_split();
        self.a.antes_do_argumento(regra, Some(c));
        self.start_block_argument_nesting();
        for (i, &n) in nos.iter().enumerate() {
            self.visit(Some(n));
            if i + 1 != nos.len() {
                let mut virgula = self.arv.fim(n) + 1;
                if self.lexema(virgula) == "?" {
                    virgula += 1;
                }
                self.token(virgula);
                let c = self.split();
                self.a.antes_do_argumento(regra, Some(c));
            }
        }
        self.token(direito);
        self.end_block_argument_nesting();
        self.unnest();
        self.end_span();
        self.end_rule();
    }

    // ============================================================ instruções

    /// `_visitLoopBody(body)`.
    fn visit_loop_body(&mut self, corpo: NoId) {
        match self.no(corpo) {
            No::EmptyStatement { .. } => self.visit(Some(corpo)),
            No::Block { .. } => {
                self.space();
                self.visit(Some(corpo));
            }
            _ => {
                self.indent(None);
                self.start_rule(None);
                self.builder_split(false, true);
                self.visit(Some(corpo));
                self.end_rule();
                self.unindent();
            }
        }
    }

    fn visit_for_parts(&mut self, id: NoId) {
        let (left_separator, condition, right_separator, updaters) = match self.no(id) {
            No::ForPartsWithDeclarations {
                variables,
                left_separator,
                condition,
                right_separator,
                updaters,
            } => {
                self.nest_expression(None, false);
                self.start_rule(None);
                if let No::VariableDeclarationList {
                    metadata,
                    keyword,
                    type_,
                    variables: vars,
                    ..
                } = self.no(*variables)
                {
                    self.visit_nodes(metadata, Acao::Nada, Acao::Split, Acao::Split);
                    self.modifier(*keyword);
                    self.visit_after(*type_, Acao::Espaco);
                    self.visit_comma_separated_nodes(vars, Acao::Split);
                }
                self.end_rule();
                self.unnest();
                (*left_separator, *condition, *right_separator, updaters)
            }
            No::ForPartsWithExpression {
                initialization,
                left_separator,
                condition,
                right_separator,
                updaters,
            } => {
                self.visit(*initialization);
                (*left_separator, *condition, *right_separator, updaters)
            }
            No::ForPartsWithPattern {
                variables,
                left_separator,
                condition,
                right_separator,
                updaters,
            } => {
                self.start_block_argument_nesting();
                self.nest_expression(None, false);
                if let No::PatternVariableDeclaration {
                    metadata,
                    keyword,
                    pattern,
                    equals,
                    expression,
                } = self.no(*variables)
                {
                    self.visit_nodes(metadata, Acao::Nada, Acao::Split, Acao::Split);
                    self.token(*keyword);
                    self.space();
                    self.visit(Some(*pattern));
                    self.visit_assignment(*equals, *expression, false);
                }
                self.unnest();
                self.end_block_argument_nesting();
                (*left_separator, *condition, *right_separator, updaters)
            }
            _ => unreachable!(),
        };
        self.token(left_separator);
        if condition.is_some() {
            self.split();
        }
        self.visit(condition);
        self.token(right_separator);
        if !updaters.is_empty() {
            self.split();
            self.start_rule(None);
            self.visit_comma_separated_nodes(updaters, Acao::Split);
            self.end_rule();
        }
    }

    fn visit_for_each_parts_from_in(&mut self, in_keyword: Tk, iterable: NoId) {
        self.solo_split(custo::NORMAL);
        self.token(in_keyword);
        self.space();
        self.visit(Some(iterable));
    }

    fn visit_if_statement(&mut self, id: NoId) {
        let No::IfStatement {
            if_keyword,
            left_parenthesis,
            expression,
            case_clause,
            right_parenthesis,
            then_statement,
            else_keyword,
            else_statement,
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_if_condition(*if_keyword, *left_parenthesis, *expression, *case_clause, *right_parenthesis);
        let tem_else = else_statement.is_some();
        let visitar_clausula = |v: &mut Self, clausula: NoId| {
            if matches!(v.no(clausula), No::Block { .. } | No::IfStatement { .. }) {
                v.space();
                v.visit(Some(clausula));
            } else {
                v.indent(None);
                v.start_rule(None);
                if tem_else {
                    v.write_newline(false, false, false);
                } else {
                    v.builder_split(false, true);
                }
                v.visit(Some(clausula));
                v.end_rule();
                v.unindent();
            }
        };
        visitar_clausula(self, *then_statement);
        if let Some(senao) = else_statement {
            if matches!(self.no(*then_statement), No::Block { .. }) {
                self.space();
            } else {
                self.newline();
            }
            self.token_opt(*else_keyword);
            visitar_clausula(self, *senao);
        }
    }

    /// `_visitIfCondition`.
    fn visit_if_condition(&mut self, if_keyword: Tk, left_parenthesis: Tk, condition: NoId, case_clause: Option<NoId>, right_parenthesis: Tk) {
        self.nest_expression(None, false);
        self.token(if_keyword);
        self.space();
        self.token(left_parenthesis);
        match case_clause {
            None => self.visit(Some(condition)),
            Some(cc) => {
                let No::CaseClause {
                    case_keyword,
                    guarded_pattern,
                } = self.no(cc)
                else {
                    unreachable!()
                };
                let No::GuardedPattern { pattern, when_clause } = self.no(*guarded_pattern) else {
                    unreachable!()
                };
                let regra_case = self.a.regra(custo::NORMAL);
                self.start_rule(Some(regra_case));
                self.visit(Some(condition));
                self.split();
                self.token(*case_keyword);
                self.space();
                self.start_block_argument_nesting();
                self.nest_expression(None, true);
                self.visit(Some(*pattern));
                self.unnest();
                self.end_block_argument_nesting();
                self.end_rule();
                if let Some(w) = when_clause {
                    self.start_rule(None);
                    self.split();
                    self.start_block_argument_nesting();
                    self.nest_expression(None, false);
                    self.visit_when_clause(*w);
                    self.unnest();
                    self.end_block_argument_nesting();
                    self.end_rule();
                }
            }
        }
        self.token(right_parenthesis);
        self.unnest();
    }

    fn visit_when_clause(&mut self, w: NoId) {
        let No::WhenClause { when_keyword, expression } = self.no(w) else {
            unreachable!()
        };
        self.token(*when_keyword);
        self.space();
        self.visit(Some(*expression));
    }

    fn visit_switch_statement(&mut self, id: NoId) {
        let No::SwitchStatement {
            switch_keyword,
            left_parenthesis,
            expression,
            right_parenthesis,
            left_bracket,
            members,
            right_bracket,
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_switch_value(*switch_keyword, *left_parenthesis, *expression, *right_parenthesis);
        self.begin_body(*left_bracket, false);
        for &m in members {
            let (labels, keyword, colon, statements) = match self.no(m) {
                No::SwitchCase {
                    labels,
                    keyword,
                    colon,
                    statements,
                    ..
                }
                | No::SwitchPatternCase {
                    labels,
                    keyword,
                    colon,
                    statements,
                    ..
                }
                | No::SwitchDefault {
                    labels,
                    keyword,
                    colon,
                    statements,
                } => (labels, *keyword, *colon, statements),
                _ => unreachable!(),
            };
            self.visit_nodes(labels, Acao::Nada, Acao::NovaLinha, Acao::NovaLinha);
            self.token(keyword);
            match self.no(m) {
                No::SwitchCase { expression, .. } => {
                    self.space();
                    self.visit(Some(*expression));
                }
                No::SwitchPatternCase { guarded_pattern, .. } => {
                    self.space();
                    let No::GuardedPattern { pattern, when_clause } = self.no(*guarded_pattern) else {
                        unreachable!()
                    };
                    match when_clause {
                        None => {
                            self.indent(None);
                            self.visit(Some(*pattern));
                            self.unindent();
                        }
                        Some(w) => {
                            self.start_rule(None);
                            self.nest_expression(None, false);
                            self.start_block_argument_nesting();
                            self.visit(Some(*pattern));
                            self.split();
                            self.visit_when_clause(*w);
                            self.end_block_argument_nesting();
                            self.unnest();
                            self.end_rule();
                        }
                    }
                }
                _ => {}
            }
            self.token(colon);
            if !statements.is_empty() {
                self.indent(None);
                self.newline();
                self.visit_nodes(statements, Acao::Nada, Acao::OneOrTwoNewlines, Acao::Nada);
                self.unindent();
                self.one_or_two_newlines();
            } else {
                self.write_newline(false, false, false);
            }
        }
        if !members.is_empty() {
            self.newline();
        }
        self.end_body(*right_bracket, !members.is_empty());
    }

    /// `_visitSwitchValue`.
    fn visit_switch_value(&mut self, switch_keyword: Tk, left_parenthesis: Tk, value: NoId, right_parenthesis: Tk) {
        self.nest_expression(None, false);
        self.token(switch_keyword);
        self.space();
        self.token(left_parenthesis);
        self.solo_zero_split();
        self.visit(Some(value));
        self.token(right_parenthesis);
        self.space();
        self.unnest();
    }

    fn visit_switch_expression(&mut self, id: NoId) {
        let No::SwitchExpression {
            switch_keyword,
            left_parenthesis,
            expression,
            right_parenthesis,
            left_bracket,
            cases,
            right_bracket,
        } = self.no(id)
        else {
            unreachable!()
        };
        if !self.can_split(cases, *right_bracket) {
            self.visit_switch_value(*switch_keyword, *left_parenthesis, *expression, *right_parenthesis);
            self.token(*left_bracket);
            self.token(*right_bracket);
            return;
        }
        self.start_rule(None);
        self.visit_switch_value(*switch_keyword, *left_parenthesis, *expression, *right_parenthesis);
        self.token(*left_bracket);
        self.start_block(None, true, !cases.is_empty());
        self.visit_comma_separated_nodes(cases, Acao::Split);
        let virgula_final = cases.last().is_some_and(|&c| self.comma_after(c).is_some());
        let forcar = self.contains_line_comments(cases, Some(*right_bracket));
        self.end_body(*right_bracket, virgula_final || forcar);
    }

    fn visit_switch_expression_case(&mut self, id: NoId) {
        let No::SwitchExpressionCase {
            guarded_pattern,
            arrow,
            expression,
        } = self.no(id)
        else {
            unreachable!()
        };
        let No::GuardedPattern { pattern, when_clause } = self.no(*guarded_pattern) else {
            unreachable!()
        };
        let mut ramos = Vec::new();
        let mut ops = Vec::new();
        self.achatar_ou(*pattern, &mut ramos, &mut ops);
        self.start_lazy_rule(None);
        for i in 0..ramos.len() - 1 {
            self.visit(Some(ramos[i]));
            self.space();
            self.token(ops[i]);
            self.split();
        }
        self.nest_expression(Some(recuo::BLOCK), false);
        if when_clause.is_some() {
            self.start_lazy_rule(None);
            self.nest_expression(Some(recuo::BLOCK), false);
        }
        self.visit(ramos.last().copied());
        if let Some(w) = when_clause {
            self.split();
            self.start_block_argument_nesting();
            self.visit_when_clause(*w);
            self.end_block_argument_nesting();
            self.unnest();
            self.end_rule();
        }
        self.space();
        self.token(*arrow);
        self.split();
        self.end_rule();
        self.start_block_argument_nesting();
        self.visit(Some(*expression));
        self.end_block_argument_nesting();
        self.unnest();
    }

    fn achatar_ou(&self, p: NoId, ramos: &mut Vec<NoId>, ops: &mut Vec<Tk>) {
        if let No::LogicalOrPattern {
            left_operand,
            operator,
            right_operand,
        } = self.no(p)
        {
            self.achatar_ou(*left_operand, ramos, ops);
            ops.push(*operator);
            self.achatar_ou(*right_operand, ramos, ops);
        } else {
            ramos.push(p);
        }
    }

    /// `_splitEmptyBlock(node)`.
    fn split_empty_block(&self, id: NoId) -> bool {
        let Some(p) = self.pai(id) else {
            return false;
        };
        if let No::IfStatement {
            else_statement,
            then_statement,
            ..
        } = self.no(p)
        {
            return else_statement.is_some() && *then_statement == id;
        }
        if matches!(self.no(p), No::CatchClause { .. })
            && let Some(avo) = self.pai(p)
            && let No::TryStatement {
                finally_block,
                catch_clauses,
                ..
            } = self.no(avo)
        {
            let ultimo_corpo = catch_clauses.last().and_then(|c| match self.no(*c) {
                No::CatchClause { body, .. } => Some(*body),
                _ => None,
            });
            return finally_block.is_some() || Some(id) != ultimo_corpo;
        }
        false
    }

    fn visit_variable_declaration_list(&mut self, id: NoId) {
        let No::VariableDeclarationList {
            metadata,
            late_keyword,
            keyword,
            type_,
            variables,
        } = self.no(id)
        else {
            unreachable!()
        };
        self.visit_metadata(metadata);
        self.start_span(custo::NORMAL);
        self.modifier(*late_keyword);
        self.modifier(*keyword);
        self.visit(*type_);
        self.separator_between_type_and_variable(*type_, true);
        self.end_span();
        self.start_rule(None);
        if variables.len() > 1 {
            self.start_block_argument_nesting();
        }
        self.visit_comma_separated_nodes(variables, Acao::Split);
        if variables.len() > 1 {
            self.end_block_argument_nesting();
        }
        self.end_rule();
    }

    // ============================================================ expressões

    fn visit_adjacent_strings(&mut self, id: NoId) {
        let No::AdjacentStrings { strings } = self.no(id) else {
            unreachable!()
        };
        let mut aninhar = true;
        if let Some(p) = self.pai(id) {
            match self.no(p) {
                No::ArgumentList { arguments, .. } => {
                    aninhar = false;
                    for &arg in arguments {
                        if arg == id {
                            continue;
                        }
                        if self.is_string_literal(arg) {
                            aninhar = true;
                            break;
                        }
                    }
                }
                No::AssertInitializer { condition, message, .. } | No::AssertStatement { condition, message, .. } => {
                    aninhar = false;
                    if *condition != id && self.is_string_literal(*condition) {
                        aninhar = true;
                    }
                    if let Some(m) = message
                        && *m != id
                        && self.is_string_literal(*m)
                    {
                        aninhar = true;
                    }
                }
                No::VariableDeclaration { .. } => aninhar = false,
                No::AssignmentExpression { right_hand_side, .. }
                    if *right_hand_side == id && self.pai_e(p, |n| matches!(n, No::ExpressionStatement { .. })) =>
                {
                    aninhar = false
                }
                No::NamedExpression { .. } | No::ExpressionFunctionBody { .. } => aninhar = false,
                _ => {}
            }
        }
        self.start_span(custo::NORMAL);
        self.start_rule(None);
        if aninhar {
            self.nest_expression(None, false);
        }
        self.visit_nodes(strings, Acao::Nada, Acao::SplitOrNewline, Acao::Nada);
        if aninhar {
            self.unnest();
        }
        self.end_rule();
        self.end_span();
    }

    /// `visitArgumentList(node, nestExpression:)`.
    pub fn visit_argument_list(&mut self, id: NoId, aninhar: bool) {
        let No::ArgumentList {
            left_parenthesis,
            arguments,
            right_parenthesis,
        } = self.no(id)
        else {
            unreachable!()
        };
        if arguments.is_empty() {
            self.token(*left_parenthesis);
            if self.tem_comentarios(*right_parenthesis) {
                self.solo_zero_split();
            }
            self.token(*right_parenthesis);
            return;
        }
        if self.list_has_comma_after(arguments) {
            self.visit_collection_literal(*left_parenthesis, arguments, *right_parenthesis, None, None, None, false, false);
            return;
        }
        if aninhar {
            self.nest_expression(None, false);
        }
        super::argumentos::ArgumentListVisitor::novo(self, id).visit(self);
        if aninhar {
            self.unnest();
        }
    }

    fn visit_cascade_expression(&mut self, id: NoId) {
        let No::CascadeExpression {
            target,
            cascade_sections,
        } = self.no(id)
        else {
            unreachable!()
        };
        if cascade_sections.len() > 1 {
            self.visit_split_cascade(*target, cascade_sections);
            return;
        }
        let mut quebra_com_alvo = true;
        if self.is_collection_literal(*target) {
            quebra_com_alvo = false;
        } else {
            match self.no(*target) {
                No::MethodInvocation { argument_list, .. }
                | No::FunctionExpressionInvocation { argument_list, .. }
                | No::InstanceCreationExpression { argument_list, .. } => {
                    if let No::ArgumentList { arguments, .. } = self.no(*argument_list) {
                        quebra_com_alvo = !self.list_has_comma_after(arguments);
                    }
                }
                _ => {}
            }
        }
        let inline = self.allow_inline(*target, cascade_sections.len());
        if quebra_com_alvo {
            let r = if inline { self.a.regra(custo::NORMAL) } else { self.a.regra_dura() };
            self.start_lazy_rule(Some(r));
        }
        self.visit(Some(*target));
        self.nest_expression(Some(recuo::CASCADE), true);
        self.start_block_argument_nesting();
        if !quebra_com_alvo {
            let r = if inline { self.a.regra(custo::NORMAL) } else { self.a.regra_dura() };
            self.start_rule(Some(r));
        }
        self.zero_split();
        if !quebra_com_alvo {
            self.end_rule();
        }
        self.visit_nodes(cascade_sections, Acao::Nada, Acao::ZeroSplit, Acao::Nada);
        if quebra_com_alvo {
            self.end_rule();
        }
        self.end_block_argument_nesting();
        self.unnest();
    }

    fn visit_split_cascade(&mut self, target: NoId, secoes: &[NoId]) {
        let dura = self.a.regra_dura();
        self.start_lazy_rule(Some(dura));
        self.visit(Some(target));
        self.nest_expression(Some(recuo::CASCADE), true);
        self.start_block_argument_nesting();
        let primeiro = self.arv.inicio(secoes[0]);
        self.write_preceding_comments_and_newlines(primeiro);
        self.suprimidos.insert(primeiro);
        self.start_block(None, false, false);
        for &s in &secoes[..secoes.len() - 1] {
            self.newline();
            self.visit(Some(s));
        }
        let ultimo = self.arv.inicio(*secoes.last().expect("seção"));
        self.write_preceding_comments_and_newlines(ultimo);
        self.suprimidos.insert(ultimo);
        self.end_block(true);
        self.visit(secoes.last().copied());
        self.end_rule();
        self.end_block_argument_nesting();
        self.unnest();
    }

    fn visit_conditional_expression(&mut self, id: NoId) {
        let No::ConditionalExpression {
            condition,
            question,
            then_expression,
            colon,
            else_expression,
        } = self.no(id)
        else {
            unreachable!()
        };
        self.nest_expression(None, false);
        self.start_lazy_rule(None);
        self.visit(Some(*condition));
        self.nest_expression(Some(recuo::BLOCK), true);
        self.start_block_argument_nesting();
        self.unnest();
        self.start_span(custo::NORMAL);
        self.split();
        self.token(*question);
        self.space();
        self.nest_expression(None, false);
        self.visit(Some(*then_expression));
        self.unnest();
        self.split();
        self.token(*colon);
        self.space();
        self.visit(Some(*else_expression));
        if self.pai_e(id, |p| matches!(p, No::ConditionalExpression { .. })) {
            self.force_rules();
        }
        self.end_rule();
        self.end_span();
        self.end_block_argument_nesting();
        self.unnest();
    }

    /// `finishIndexExpression(node)`.
    pub fn finish_index_expression(&mut self, id: NoId) {
        let No::IndexExpression {
            target,
            question,
            left_bracket,
            index,
            right_bracket,
            ..
        } = self.no(id)
        else {
            unreachable!()
        };
        if let Some(t) = target
            && matches!(self.no(*t), No::IndexExpression { .. })
        {
            self.solo_zero_split();
        }
        self.start_span(custo::INDEX);
        self.token_opt(*question);
        self.token(*left_bracket);
        self.solo_zero_split();
        self.visit(Some(*index));
        self.token(*right_bracket);
        self.end_span();
    }

    fn visit_method_invocation(&mut self, id: NoId) {
        let No::MethodInvocation {
            target,
            operator,
            method_name,
            type_arguments,
            argument_list,
        } = self.no(id)
        else {
            unreachable!()
        };
        if target.is_none() || self.looks_like_static_call(id) {
            self.nest_expression(None, false);
            self.start_span(custo::NORMAL);
            if target.is_some() {
                self.start_span(custo::CONSTRUCTOR_NAME);
                self.visit(*target);
                self.solo_zero_split();
            }
            self.token_opt(*operator);
            self.visit(Some(*method_name));
            if target.is_some() {
                self.end_span();
            }
            self.nest_expression(None, false);
            self.visit(*type_arguments);
            self.visit_argument_list(*argument_list, false);
            self.unnest();
            self.end_span();
            self.unnest();
            return;
        }
        super::cadeia::CallChainVisitor::novo(self, id).visit(self);
    }

    /// `visitNamedNode(name, colon, node, [rule])`.
    pub fn visit_named_node(&mut self, nome: Tk, dois_pontos: Tk, no: NoId, regra: Option<RegraId>) {
        self.nest_expression(None, false);
        self.start_span(custo::NORMAL);
        self.token(nome);
        self.token(dois_pontos);
        if matches!(self.no(no), No::ListLiteral { .. } | No::SetOrMapLiteral { .. } | No::RecordLiteral { .. }) {
            self.space();
        } else {
            let r = self.solo_split(custo::NORMAL);
            if let Some(nomeada) = regra {
                self.a.restringir_quando_quebra(r, nomeada);
            }
        }
        self.visit(Some(no));
        self.end_span();
        self.unnest();
    }

    /// `_visitAssignment(equalsOperator, rightHandSide, nest:)`.
    fn visit_assignment(&mut self, igual: Tk, direito: NoId, aninhar: bool) {
        self.space();
        self.token(igual);
        if aninhar {
            self.nest_expression(None, true);
        }
        let c = self.assignment_cost(direito);
        self.solo_split(c);
        self.start_span(custo::NORMAL);
        self.visit(Some(direito));
        self.end_span();
        if aninhar {
            self.unnest();
        }
    }

    /// `_assignmentCost(rightHandSide)`.
    fn assignment_cost(&self, direito: NoId) -> i32 {
        match self.no(direito) {
            No::ListLiteral { .. } | No::SetOrMapLiteral { .. } | No::CascadeExpression { .. } => custo::ASSIGN_BLOCK,
            _ => custo::ASSIGN,
        }
    }

    /// `_visitBinary(node, destructure, precedence:, nest:)`.
    fn visit_binary(&mut self, id: NoId, precedencia_alvo: Option<u8>, aninhar: bool) {
        self.start_span(custo::NORMAL);
        if aninhar {
            self.nest_expression(None, false);
        }
        self.start_lazy_rule(None);
        self.start_block_argument_nesting();
        let e_padrao_e = matches!(self.no(id), No::LogicalAndPattern { .. });
        let e_padrao_ou = matches!(self.no(id), No::LogicalOrPattern { .. });
        self.percorrer_binario(id, precedencia_alvo, e_padrao_e, e_padrao_ou);
        self.end_block_argument_nesting();
        if aninhar {
            self.unnest();
        }
        self.end_span();
        self.end_rule();
    }

    fn percorrer_binario(&mut self, e: NoId, prec: Option<u8>, padrao_e: bool, padrao_ou: bool) {
        let partes = match self.no(e) {
            No::BinaryExpression {
                left_operand,
                operator,
                right_operand,
            } if !padrao_e && !padrao_ou => Some((*left_operand, *operator, *right_operand)),
            No::LogicalAndPattern {
                left_operand,
                operator,
                right_operand,
            } if padrao_e => Some((*left_operand, *operator, *right_operand)),
            No::LogicalOrPattern {
                left_operand,
                operator,
                right_operand,
            } if padrao_ou => Some((*left_operand, *operator, *right_operand)),
            _ => None,
        };
        let Some((esq, op, dir)) = partes else {
            self.visit(Some(e));
            return;
        };
        if let Some(p) = prec
            && precedencia(self.lexema(op)) != p
        {
            self.visit(Some(e));
            return;
        }
        self.percorrer_binario(esq, prec, padrao_e, padrao_ou);
        self.space();
        self.token(op);
        self.split();
        self.percorrer_binario(dir, prec, padrao_e, padrao_ou);
    }

    fn visit_if_element(&mut self, id: NoId) {
        let mut elementos = Vec::new();
        let mut atual = Some(id);
        while let Some(n) = atual {
            if let No::IfElement { else_element, .. } = self.no(n) {
                elementos.push(n);
                atual = *else_element;
            } else {
                break;
            }
        }
        let regra_spread = self.a.regra(custo::NORMAL);
        let mut spreads: Vec<NoId> = Vec::new();
        for &el in &elementos {
            let No::IfElement { then_element, .. } = self.no(el) else {
                unreachable!()
            };
            if let Some(colchete) = self.spread_collection_bracket(*then_element) {
                spreads.push(el);
                self.before_block(colchete, regra_spread, None);
            }
        }
        let ultimo = *elementos.last().expect("if");
        let ultimo_else = match self.no(ultimo) {
            No::IfElement { else_element, .. } => *else_element,
            _ => None,
        };
        if let Some(le) = ultimo_else
            && let Some(colchete) = self.spread_collection_bracket(le)
        {
            spreads.push(le);
            self.before_block(colchete, regra_spread, None);
        }

        let visitar_filho = |v: &mut Self, elemento: NoId, filho: NoId, spreads: &[NoId]| {
            v.nest_expression(Some(2), true);
            let e_spread = spreads.contains(&elemento);
            if e_spread {
                v.space();
            } else {
                v.split();
                v.start_block_argument_nesting();
            }
            v.visit(Some(filho));
            if !e_spread {
                v.end_block_argument_nesting();
            }
            v.unnest();
        };

        self.start_lazy_rule(None);
        let mut fluxo_interno = false;
        for &el in &elementos {
            let No::IfElement {
                if_keyword,
                left_parenthesis,
                expression,
                case_clause,
                right_parenthesis,
                then_element,
                else_keyword,
                else_element,
            } = self.no(el)
            else {
                unreachable!()
            };
            self.visit_if_condition(*if_keyword, *left_parenthesis, *expression, *case_clause, *right_parenthesis);
            visitar_filho(self, el, *then_element, &spreads);
            if self.is_control_flow_element(*then_element) {
                fluxo_interno = true;
            }
            if else_element.is_some() {
                if spreads.contains(&el) {
                    self.space();
                } else {
                    self.split();
                }
                self.token_opt(*else_keyword);
                if el != ultimo {
                    self.space();
                }
            }
        }
        if let Some(le) = ultimo_else {
            visitar_filho(self, le, le, &spreads);
            if self.is_control_flow_element(le) {
                fluxo_interno = true;
            }
        }
        if fluxo_interno {
            self.force_rules();
        }
        self.end_rule();
    }

    fn visit_for_element(&mut self, id: NoId) {
        let No::ForElement {
            await_keyword,
            for_keyword,
            left_parenthesis,
            for_loop_parts,
            right_parenthesis,
            body,
        } = self.no(id)
        else {
            unreachable!()
        };
        let corpo_spread = self.spread_collection_bracket(*body).is_some();
        self.nest_expression(None, false);
        self.modifier(*await_keyword);
        self.token(*for_keyword);
        self.space();
        self.token(*left_parenthesis);
        self.start_rule(None);
        self.start_rule(None);
        self.visit(Some(*for_loop_parts));
        self.token(*right_parenthesis);
        self.end_rule();
        self.unnest();
        self.nest_expression(Some(recuo::BLOCK), true);
        if corpo_spread {
            self.space();
        } else {
            self.split();
            self.start_block_argument_nesting();
        }
        self.visit(Some(*body));
        if !corpo_spread {
            self.end_block_argument_nesting();
        }
        self.unnest();
        if self.is_control_flow_element(*body) {
            self.force_rules();
        }
        self.end_rule();
    }

    // ============================================================= coleções

    /// `beforeBlock(token, rule, [previousChunk])`.
    pub fn before_block(&mut self, t: Tk, regra: RegraId, anterior: Option<ChunkId>) {
        self.regras_de_bloco.insert(t, regra);
        if let Some(c) = anterior {
            self.chunks_de_bloco.insert(t, c);
        }
    }

    /// `_visitCollectionLiteral`.
    #[allow(clippy::too_many_arguments)]
    pub fn visit_collection_literal(
        &mut self,
        esquerdo: Tk,
        elementos: &[NoId],
        direito: Tk,
        const_keyword: Option<Tk>,
        type_arguments: Option<NoId>,
        custo_opt: Option<i32>,
        dividir_externa: bool,
        e_registro: bool,
    ) {
        let _ = custo_opt;
        self.modifier(const_keyword);
        if let Some(ta) = type_arguments
            && let No::TypeArgumentList {
                left_bracket,
                arguments,
                right_bracket,
            } = self.no(ta)
        {
            self.start_span(custo::NORMAL);
            self.nest_expression(None, false);
            self.token(*left_bracket);
            let r = self.a.regra(custo::TYPE_ARGUMENT);
            self.start_rule(Some(r));
            for (i, &arg) in arguments.iter().enumerate() {
                self.visit(Some(arg));
                if i + 1 != arguments.len() {
                    let mut virgula = self.arv.fim(arg) + 1;
                    if self.lexema(virgula) == "?" {
                        virgula += 1;
                    }
                    self.token(virgula);
                    self.split();
                }
            }
            self.token(*right_bracket);
            self.end_rule();
            self.unnest();
            self.end_span();
        }
        if elementos.is_empty() {
            self.visit_body(esquerdo, elementos, direito);
            return;
        }
        if dividir_externa {
            for c in self.colecoes.iter_mut() {
                *c = true;
            }
            self.colecoes.push(false);
        }
        self.begin_body(esquerdo, false);
        if self.contains_line_comments(elementos, Some(direito)) {
            let mut regra_linha = self.a.regra_argumento_de_tipo();
            self.start_lazy_rule(Some(regra_linha));
            for &el in elementos {
                let inicio = self.arv.inicio(el);
                if self.linha_fim(inicio - 1) != self.linha_inicio(inicio) {
                    self.one_or_two_newlines();
                    self.end_rule();
                    regra_linha = self.a.regra_argumento_de_tipo();
                    self.start_lazy_rule(Some(regra_linha));
                } else {
                    let c = self.split();
                    self.a.antes_do_argumento(regra_linha, Some(c));
                }
                self.visit(Some(el));
                self.write_comma_after(el);
            }
            self.end_rule();
        } else {
            for (i, &el) in elementos.iter().enumerate() {
                self.builder_split(false, i != 0);
                self.visit(Some(el));
                self.write_comma_after(el);
            }
        }
        let mut forcar = false;
        if dividir_externa {
            forcar = self.colecoes.pop().expect("coleção");
        }
        let registro_unico = e_registro && elementos.len() == 1;
        if self.list_has_comma_after(elementos) && !registro_unico {
            forcar = true;
        }
        self.end_body(direito, forcar);
    }

    /// `_beginBody(leftBracket, space:)`.
    fn begin_body(&mut self, esquerdo: Tk, espaco: bool) {
        self.token(esquerdo);
        let regra = self.regras_de_bloco.get(&esquerdo).copied();
        self.start_rule(regra);
        let anterior = self.chunks_de_bloco.get(&esquerdo).copied();
        self.start_block(anterior, true, espaco);
    }

    /// `_endBody(rightBracket, forceSplit:)`.
    fn end_body(&mut self, direito: Tk, forcar: bool) {
        let nova_linha = self.write_preceding_comments_and_newlines(direito);
        self.end_block(nova_linha || forcar);
        self.end_rule();
        let t = self.lexema(direito);
        self.write_text(t, direito);
    }

    /// `_visitBody(leftBracket, nodes, rightBracket)`.
    fn visit_body(&mut self, esquerdo: Tk, nos: &[NoId], direito: Tk) {
        if !self.can_split(nos, direito) {
            self.token(esquerdo);
            self.token(direito);
            return;
        }
        self.begin_body(esquerdo, false);
        self.visit_body_contents(nos);
        self.end_body(direito, !nos.is_empty());
    }

    /// `_visitBodyContents(nodes)`.
    fn visit_body_contents(&mut self, nos: &[NoId]) {
        for (i, &n) in nos.iter().enumerate() {
            self.visit(Some(n));
            if i + 1 != nos.len() {
                if self.has_non_empty_body(n) {
                    self.two_newlines();
                } else {
                    self.one_or_two_newlines();
                }
            }
        }
    }

    // =============================================================== strings

    /// `_writeStringLiteral(string)`.
    fn write_string_literal(&mut self, t: Tk) {
        self.write_preceding_comments_and_newlines(t);
        let lexema = self.lexema(t);
        let linhas = dividir_linhas(lexema);
        if linhas.len() > 1 {
            self.write_string_first_line(linhas[0], t);
        } else {
            self.write_text(linhas[0], t);
        }
        for linha in &linhas[1..] {
            self.write_newline(false, true, true);
            self.write_text_merge(linha, t, false);
        }
    }

    /// `_writeStringFirstLine(line, string)`.
    fn write_string_first_line(&mut self, linha: &str, t: Tk) {
        let b = linha.as_bytes();
        let inicio_aspas = usize::from(linha.starts_with('r'));
        let fim_aspas = inicio_aspas + 3;
        let mut barras = 0;
        if b.len() > fim_aspas && (linha[inicio_aspas..].starts_with("'''") || linha[inicio_aspas..].starts_with("\"\"\"")) {
            let mut cursor = fim_aspas;
            loop {
                let mut c = b[cursor];
                if c == b'\\' {
                    cursor += 1;
                    barras += 1;
                    if cursor >= b.len() {
                        break;
                    }
                    c = b[cursor];
                }
                if c != b' ' && c != b'\t' {
                    break;
                }
                cursor += 1;
                if cursor >= b.len() {
                    break;
                }
            }
            if cursor == b.len() {
                let mut texto = linha[..fim_aspas].to_string();
                if barras > 0 {
                    texto.push('\\');
                    barras -= 1;
                    while barras > 0 {
                        texto.push_str(" \\");
                        barras -= 1;
                    }
                }
                self.write_text(&texto, t);
                return;
            }
        }
        self.write_text(linha, t);
    }
}

/// Divide um lexema nas quebras `\r\n`, `\r` e `\n` (`_lineTerminatorRE`).
fn dividir_linhas(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut v = Vec::new();
    let mut inicio = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' {
            v.push(&s[inicio..i]);
            if b.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
            inicio = i + 1;
        } else if b[i] == b'\n' {
            v.push(&s[inicio..i]);
            inicio = i + 1;
        }
        i += 1;
    }
    v.push(&s[inicio..]);
    v
}

/// `_looksLikeClassName(name)`.
pub fn looks_like_class_name(nome: &str) -> bool {
    if matches!(nome, "bool" | "double" | "int" | "num") {
        return true;
    }
    let b = nome.as_bytes();
    let mut i = 0;
    let Some(&primeiro) = b.first() else {
        return false;
    };
    let mut c = primeiro;
    i += 1;
    if c == b'_' {
        if b.len() == 1 {
            return false;
        }
        c = b[i];
        i += 1;
    }
    if !c.is_ascii_uppercase() {
        return false;
    }
    b[i..].iter().any(|c| c.is_ascii_lowercase())
}

/// Precedência de um operador binário (`TokenType.precedence`).
pub fn precedencia(op: &str) -> u8 {
    match op {
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
        "*" | "/" | "~/" | "%" => 14,
        _ => 0,
    }
}

#[allow(dead_code)]
fn _usa(k: Keyword) -> Keyword {
    k
}
