//! O `InScopeCompletionPass` do analysis server 3.6.2
//! (`AS:src/services/completion/dart/in_scope_completion_pass.dart`, lido
//! inteiro; docs/LSP-ESPECIFICACAO.md §14.13 e §14.14).
//!
//! O contexto é decidido sobre a árvore do **texto real** no formato do
//! analyzer ([`crate::arvore_analyzer`]): o `coveringNode` do `unit.select`
//! (§14.13.4, com as duas exceções de fronteira), o `_completionNode` (a
//! subida enquanto o pai começa no mesmo token e o filho não está numa
//! lista) e um `visit*` por espécie de nó (`isp_visitas`), que grava o
//! `completionLocation` e chama os ajudantes (palavras-chave, declarações,
//! identificadores, rótulos, sobrescritas, URIs) na ordem do Dart: a ordem
//! em que os candidatos chegam ao coletor é a ordem da resposta entre
//! iguais.
//!
//! A semântica (o tipo estático de um alvo, o elemento de um identificador,
//! os elementos declarados) vem da [`Consulta`], que pode ter analisado o
//! texto com o sentinela no lugar da palavra sob o cursor: os offsets do
//! texto real são levados para os dela pelo deslocamento do sentinela
//! ([`Isp::mapear`]); o que vem antes do cursor tem o mesmo offset.
//!
//! O que a recuperação do parser do DartForge não reproduz do fasta (tokens
//! sintéticos de comprimento 0, o `droppedToken`) é lido como "ausente": um
//! token que o fasta criaria sintético é procurado no texto e, se não está
//! lá, vale a posição do próximo token real (onde o fasta o poria).

use super::*;
use crate::arvore_analyzer::{Arvore, Ligacao};
use dartforge_elements::model::UnitId;
use dartforge_frontend::token::{Kind, Op, Token};

/// Os lexemas que o scanner do fasta lê como palavra-chave (`KeywordToken`:
/// reservadas, embutidas e pseudo-palavras): para eles `type == IDENTIFIER`
/// é falso e `isKeyword` é verdadeiro.
pub(super) const PALAVRAS_DO_ANALYZER: &[&str] = &[
    "abstract", "as", "assert", "async", "augment", "await", "base", "break", "case", "catch", "class", "const", "continue", "covariant", "default",
    "deferred", "do", "dynamic", "else", "enum", "export", "extends", "extension", "external", "factory", "false", "final", "finally", "for",
    "Function", "get", "hide", "if", "implements", "import", "in", "inout", "interface", "is", "late", "library", "mixin", "native", "new", "null",
    "of", "on", "operator", "out", "part", "patch", "required", "rethrow", "return", "sealed", "set", "show", "source", "static", "super", "switch",
    "sync", "this", "throw", "true", "try", "typedef", "var", "void", "when", "while", "with", "yield",
];

/// As espécies de nó que são `Expression` no analyzer.
pub(super) const ESPECIES_DE_EXPRESSAO: &[&str] = &[
    "AdjacentStrings", "AsExpression", "AssignmentExpression", "AwaitExpression", "BinaryExpression", "BooleanLiteral", "CascadeExpression",
    "ConditionalExpression", "ConstructorReference", "DotShorthand", "DoubleLiteral", "ExtensionOverride", "FunctionExpression", "FunctionExpressionInvocation",
    "FunctionReference", "IndexExpression", "InstanceCreationExpression", "IntegerLiteral", "IsExpression", "ListLiteral", "MethodInvocation",
    "NamedExpression", "NullLiteral", "ParenthesizedExpression", "PatternAssignment", "PostfixExpression", "PrefixExpression",
    "PrefixedIdentifier", "PropertyAccess", "RecordLiteral", "RethrowExpression", "SetOrMapLiteral", "SimpleIdentifier",
    "SimpleStringLiteral", "StringInterpolation", "SuperExpression", "SwitchExpression", "SymbolLiteral", "ThisExpression",
    "ThrowExpression", "TypeLiteral",
];

/// A espécie é um `DartPattern`.
pub(super) fn e_padrao(e: &str) -> bool {
    matches!(
        e,
        "CastPattern" | "ConstantPattern" | "DeclaredVariablePattern" | "ListPattern" | "LogicalAndPattern" | "LogicalOrPattern" | "MapPattern"
            | "NullAssertPattern" | "NullCheckPattern" | "ObjectPattern" | "ParenthesizedPattern" | "RecordPattern" | "RelationalPattern"
            | "WildcardPattern" | "AssignedVariablePattern"
    )
}

/// A espécie é uma declaração com corpo de membros (`Declaration` onde uma
/// anotação de `@override` se aplica).
pub(super) fn e_declaracao(e: &str) -> bool {
    matches!(e, "ClassDeclaration" | "MixinDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration")
}

/// A espécie é um `Statement`.
pub(super) fn e_comando(e: &str) -> bool {
    matches!(
        e,
        "AssertStatement" | "Block" | "BreakStatement" | "ContinueStatement" | "DoStatement" | "EmptyStatement" | "ExpressionStatement"
            | "ForStatement" | "FunctionDeclarationStatement" | "IfStatement" | "LabeledStatement" | "PatternVariableDeclarationStatement"
            | "ReturnStatement" | "SwitchStatement" | "TryStatement" | "VariableDeclarationStatement" | "WhileStatement" | "YieldStatement"
    )
}

/// A espécie é um `FormalParameter`.
pub(super) fn e_parametro(e: &str) -> bool {
    matches!(e, "SimpleFormalParameter" | "FieldFormalParameter" | "SuperFormalParameter" | "FunctionTypedFormalParameter" | "DefaultFormalParameter")
}

/// Um recurso da linguagem que o completar consulta (`featureSet`), pela
/// versão em que entrou.
#[derive(Debug, Clone, Copy)]
pub(super) enum Recurso {
    ExtensionMethods,
    EnhancedEnums,
    SuperParameters,
    Patterns,
    ClassModifiers,
    SealedClass,
    InlineClass,
}

/// As opções do `DeclarationHelper` (a primeira chamada de
/// `declarationHelper` as fixa; as seguintes as ignoram).
#[derive(Debug, Clone, Default)]
pub(super) struct Flags {
    pub atribuivel: bool,
    pub constante: bool,
    pub estensivel: bool,
    pub implementavel: bool,
    pub misturavel: bool,
    pub nao_void: bool,
    pub estatico: bool,
    pub tipo: bool,
    pub excluir_nomes_de_tipo: bool,
    pub padrao_objeto: bool,
    pub preferir_sem_invocacao: bool,
    pub sem_nome_como_new: bool,
    /// Os nós excluídos (`excludedNodes`): listas de parâmetros de tipo
    /// fantasmas da recuperação.
    pub excluidos: Vec<usize>,
}

/// Uma operação do `NotImportedCompletionPass` registrada pelo
/// `DeclarationHelper`.
#[derive(Debug, Clone)]
pub(super) enum Operacao {
    /// `StaticMembersOperation`.
    MembrosEstaticos,
    /// `ConstructorsOperation`.
    Construtores,
    /// `InstanceExtensionMembersOperation`.
    MembrosDeExtensao { tipo: TypeId, excluidos: Vec<String>, metodos: bool, setters: bool },
}

/// O resultado do passe.
pub(super) struct ResultadoIsp {
    /// Os candidatos na ordem do coletor e o `matcherScore` de cada um.
    pub itens: Vec<(f64, ItemCompletar)>,
    pub local: Option<String>,
    pub incompleta: bool,
    pub prefere_constantes: bool,
    pub operacoes: Vec<Operacao>,
    /// As opções do `DeclarationHelper` usadas (as do segundo passe).
    pub flags: Option<Flags>,
    /// Os nomes vistos pelo `VisibilityTracker` (com o prefixo de import).
    pub visiveis: HashSet<String>,
    /// `isCommentText`: o cursor está num comentário comum (lista vazia).
    pub comentario: bool,
    /// `replacementRange`: `(início, comprimento)`.
    pub intervalo: (usize, usize),
    /// O `targetPrefix` (o filtro e o truncamento do handler).
    pub prefixo_do_pedido: String,
    /// O prefixo do casador (`TokenData.fromSelection`).
    pub prefixo_do_casador: String,
    /// `request.contextType`.
    pub tipo_de_contexto: Option<TypeId>,
    /// `request.inConstantContext`.
    pub em_contexto_constante: bool,
    /// As bibliotecas não importadas que a consulta não carregou (o
    /// `NotImportedCompletionPass` delas vai pelo resumo de nomes):
    /// `(caminho, URI do import)`.
    pub resumidas: Vec<(PathBuf, String)>,
}

/// O `beginToken` de um nó: um comentário de documentação, um token do
/// texto, ou nenhum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Inicial {
    Comentario,
    Tok(usize),
    Nenhum,
}

pub(super) struct Isp<'a, 'c> {
    /// O texto real, a árvore do parser dele (com os nomes) e a do analyzer.
    pub fonte: &'a str,
    pub a: &'a ast::Ast,
    pub cu: &'a ast::CompilationUnit,
    pub nr: &'a Interner,
    pub arv: &'a Arvore,
    pub toks: Vec<Token>,
    pub offset: usize,
    pub features: LibraryFeatures,
    /// A semântica.
    pub consulta: &'c mut Consulta,
    pub unidade: UnitId,
    /// O trecho trocado pelo sentinela no texto da consulta: `(início, fim
    /// no texto real, comprimento do sentinela, deslocamento depois do
    /// trecho)`.
    pub troca: Option<(usize, usize, usize, isize)>,
    /// As expressões da consulta pelo intervalo.
    pub exprs: HashMap<(usize, usize), ExprId>,
    /// O coletor de itens (os construtores de item do completar).
    pub coletor: Coletor,
    pub scores: Vec<f64>,
    pub local: Option<String>,
    pub prefere_constantes: bool,
    pub incompleta: bool,
    pub flags: Option<Flags>,
    /// `VisibilityTracker._declaredNames`.
    pub visiveis: HashSet<String>,
    /// `_variableDistance`.
    pub distancia: u32,
    /// `IdentifierHelper.includePrivateIdentifiers` (a primeira chamada).
    pub privados: Option<bool>,
    pub casador: Option<crate::casador::Casador>,
    pub operacoes: Vec<Operacao>,
    /// O caminho do arquivo (o nome sugerido para a declaração de topo).
    pub caminho: Option<PathBuf>,
    /// `state.contextType`, quando o pedido o calculou.
    pub tipo_de_contexto: Option<TypeId>,
    /// `_containingMemberName`: o nome do método que contém um `super.▮`.
    pub metodo_do_super: Option<String>,
}

/// Executa o passe: monta a árvore do texto real, acha o alvo (o
/// `CompletionTarget`), o intervalo, os prefixos e o tipo de contexto, e
/// visita o nó de completar.
#[allow(clippy::too_many_arguments)]
pub(super) fn executar(
    consulta: &mut Consulta,
    unidade: UnitId,
    texto: &str,
    features: LibraryFeatures,
    offset: usize,
    troca: Option<(usize, usize, usize, isize)>,
    antes_de_3: bool,
    nao_importadas: Option<&mut dyn FnMut() -> Vec<(PathBuf, String)>>,
) -> ResultadoIsp {
    let mut nomes = Interner::new();
    let analisado = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
    let arv = crate::arvore_analyzer::construir(texto, &analisado.ast, &analisado.unit, antes_de_3);
    let toks = dartforge_frontend::lexer::lex(texto).unwrap_or_default();
    let mut exprs = HashMap::new();
    for (i, e) in consulta.programa.unit(unidade).ast.exprs.iter().enumerate() {
        exprs.entry((e.span.start, e.span.end)).or_insert(ExprId(i as u32));
    }
    let biblioteca = consulta.programa.unit(unidade).library;
    let caminho = consulta.programa.unit(unidade).path.clone();
    let mut isp = Isp {
        fonte: texto,
        a: &analisado.ast,
        cu: &analisado.unit,
        nr: &nomes,
        arv: &arv,
        toks,
        offset,
        features,
        consulta,
        unidade,
        troca,
        exprs,
        coletor: Coletor { itens: Vec::new(), biblioteca, receptor: None },
        scores: Vec::new(),
        local: None,
        prefere_constantes: false,
        incompleta: false,
        flags: None,
        visiveis: HashSet::new(),
        distancia: 0,
        privados: None,
        casador: None,
        operacoes: Vec::new(),
        caminho,
        tipo_de_contexto: None,
        metodo_do_super: None,
    };
    let alvo = isp.alvo();
    if alvo.texto_de_comentario {
        return ResultadoIsp {
            itens: Vec::new(),
            local: None,
            incompleta: false,
            prefere_constantes: false,
            operacoes: Vec::new(),
            flags: None,
            visiveis: HashSet::new(),
            comentario: true,
            intervalo: (offset, 0),
            prefixo_do_pedido: String::new(),
            prefixo_do_casador: String::new(),
            tipo_de_contexto: None,
            em_contexto_constante: false,
            resumidas: Vec::new(),
        };
    }
    let intervalo = isp.intervalo_de_substituicao(&alvo);
    let prefixo_do_pedido = isp.prefixo_do_pedido(&alvo);
    let prefixo_do_casador = isp.prefixo_do_casador();
    let em_contexto_constante = isp.pedido_em_contexto_constante(&alvo);
    let contexto = isp.tipo_de_contexto_em(alvo.no, offset);
    isp.tipo_de_contexto = contexto;
    isp.casador = (!prefixo_do_casador.is_empty()).then(|| crate::casador::Casador::novo(&prefixo_do_casador, crate::casador::Estilo::Texto));
    if let Some(no) = isp.no_de_completar() {
        isp.visitar(no);
    }
    // O `NotImportedCompletionPass`: as operações registradas, sobre cada
    // biblioteca candidata (as carregadas pelos elementos; as outras pelo
    // resumo, fora daqui).
    let mut resumidas = Vec::new();
    if !isp.operacoes.is_empty()
        && !prefixo_do_pedido.is_empty()
        && let Some(candidatas) = nao_importadas
    {
        let chave = |c: &std::path::Path| dartforge_elements::gerado::chave(c);
        let mut por_caminho: HashMap<PathBuf, LibraryId> = HashMap::new();
        for (i, l) in isp.consulta.programa.libraries.iter().enumerate() {
            if let Some(&u) = l.units.first()
                && let Some(c) = isp.consulta.programa.unit(u).path.as_deref()
            {
                por_caminho.insert(chave(c), LibraryId(i as u32));
            }
        }
        for (caminho, uri) in candidatas() {
            match por_caminho.get(&chave(&caminho)) {
                Some(&l) => {
                    let antes = isp.coletor.itens.len();
                    isp.operacoes_nao_importadas(l, &uri);
                    let _ = antes;
                }
                None => resumidas.push((caminho, uri)),
            }
        }
    }
    let Isp { coletor, scores, local, incompleta, prefere_constantes, operacoes, flags, visiveis, .. } = isp;
    let itens = coletor.itens.into_iter().zip(scores).map(|(i, s)| (s, i)).collect();
    ResultadoIsp {
        itens,
        local,
        incompleta,
        prefere_constantes,
        operacoes,
        flags,
        visiveis,
        comentario: false,
        intervalo,
        prefixo_do_pedido,
        prefixo_do_casador,
        tipo_de_contexto: contexto,
        em_contexto_constante,
        resumidas,
    }
}

impl<'a, 'c> Isp<'a, 'c> {
    // -- O coletor -------------------------------------------------------------

    /// `featureSet.isEnabled(recurso)`.
    pub(super) fn tem_recurso(&self, r: Recurso) -> bool {
        let v = match r {
            Recurso::ExtensionMethods => (2, 6),
            Recurso::EnhancedEnums | Recurso::SuperParameters => (2, 17),
            Recurso::Patterns | Recurso::ClassModifiers | Recurso::SealedClass => (3, 0),
            Recurso::InlineClass => (3, 3),
        };
        let atual = self.features.versao();
        (atual.major, atual.minor) >= v
    }

    /// `state.matcher.score(texto)`: 0 sem prefixo (`NoPrefixMatcher`), −1
    /// quando não casa.
    pub(super) fn score(&mut self, texto: &str) -> f64 {
        match self.casador.as_mut() {
            Some(c) => c.score(texto),
            None => 0.0,
        }
    }

    /// Registra os itens que o coletor ganhou desde `antes` com o score.
    pub(super) fn registrar(&mut self, antes: usize, score: f64) {
        while self.scores.len() < self.coletor.itens.len() {
            self.scores.push(score);
        }
        let _ = antes;
    }

    /// O `completionLocation` (atribuição simples).
    pub(super) fn local(&mut self, l: &str) {
        self.local = Some(l.to_string());
    }

    /// `completionLocation ??= l`.
    pub(super) fn local_se_vazio(&mut self, l: &str) {
        if self.local.is_none() {
            self.local = Some(l.to_string());
        }
    }

    // -- Nós ---------------------------------------------------------------------

    pub(super) fn especie(&self, n: usize) -> &'static str {
        self.arv.nos[n].especie
    }

    pub(super) fn pai(&self, n: usize) -> Option<usize> {
        self.arv.nos[n].pai
    }

    pub(super) fn filhos(&self, n: usize) -> &[usize] {
        &self.arv.nos[n].filhos
    }

    pub(super) fn ini(&self, n: usize) -> usize {
        self.arv.nos[n].inicio
    }

    pub(super) fn fim(&self, n: usize) -> usize {
        self.arv.nos[n].fim
    }

    pub(super) fn lig(&self, n: usize) -> Ligacao {
        self.arv.nos[n].ligacao
    }

    /// O primeiro filho da espécie.
    pub(super) fn filho(&self, n: usize, especie: &str) -> Option<usize> {
        self.filhos(n).iter().copied().find(|&f| self.especie(f) == especie)
    }

    /// Os filhos de uma das espécies.
    pub(super) fn filhos_de(&self, n: usize, especies: &[&str]) -> Vec<usize> {
        self.filhos(n).iter().copied().filter(|&f| especies.contains(&self.especie(f))).collect()
    }

    /// O ancestral mais próximo (o próprio incluso) de uma das espécies.
    pub(super) fn ancestral(&self, n: usize, especies: &[&str]) -> Option<usize> {
        let mut atual = Some(n);
        while let Some(k) = atual {
            if especies.contains(&self.especie(k)) {
                return Some(k);
            }
            atual = self.pai(k);
        }
        None
    }

    /// `coversOffset`: `ini <= offset <= fim`.
    pub(super) fn cobre(&self, ini: usize, fim: usize) -> bool {
        ini <= self.offset && self.offset <= fim
    }

    pub(super) fn cobre_no(&self, n: usize) -> bool {
        self.cobre(self.ini(n), self.fim(n))
    }

    /// `elementBefore(offset)`: o último com `fim <= offset`.
    pub(super) fn elemento_antes(&self, nos: &[usize]) -> Option<usize> {
        nos.iter().copied().filter(|&k| self.fim(k) <= self.offset).last()
    }

    // -- Tokens ------------------------------------------------------------------

    /// O índice do primeiro token que começa em `off` ou depois.
    pub(super) fn tok_desde(&self, off: usize) -> usize {
        self.toks.partition_point(|t| t.span.start < off)
    }

    /// O token que começa exatamente em `off`.
    pub(super) fn tok_em(&self, off: usize) -> Option<usize> {
        let i = self.tok_desde(off);
        self.toks.get(i).filter(|t| t.span.start == off && t.kind != Kind::Eof).map(|_| i)
    }

    /// O último token que termina até `off`.
    pub(super) fn tok_antes(&self, off: usize) -> Option<usize> {
        self.toks.partition_point(|t| t.span.end <= off).checked_sub(1)
    }

    /// O primeiro token que começa em `off` ou depois (fora o EOF).
    pub(super) fn tok_depois(&self, off: usize) -> Option<usize> {
        let i = self.tok_desde(off);
        self.toks.get(i).filter(|t| t.kind != Kind::Eof).map(|_| i)
    }

    pub(super) fn span_tok(&self, i: usize) -> Span {
        self.toks[i].span
    }

    pub(super) fn texto_tok(&self, i: usize) -> &'a str {
        self.toks[i].text(self.fonte)
    }

    /// `isKeywordOrIdentifier`.
    pub(super) fn palavra(&self, i: usize) -> bool {
        matches!(self.toks.get(i).map(|t| t.kind), Some(Kind::Ident | Kind::Keyword(_)))
    }

    /// `type.isKeyword` (o `KeywordToken` do fasta).
    pub(super) fn e_palavra_chave(&self, i: usize) -> bool {
        self.palavra(i) && PALAVRAS_DO_ANALYZER.contains(&self.texto_tok(i))
    }

    /// `type == TokenType.IDENTIFIER`.
    pub(super) fn e_identificador(&self, i: usize) -> bool {
        self.palavra(i) && !self.e_palavra_chave(i)
    }

    pub(super) fn e_op(&self, i: usize, op: Op) -> bool {
        self.toks.get(i).is_some_and(|t| t.kind == Kind::Op(op))
    }

    /// O primeiro token `op` em `[de, ate)`, fora de parênteses e colchetes
    /// abertos depois de `de`.
    pub(super) fn op_em(&self, op: Op, de: usize, ate: usize) -> Option<usize> {
        let mut i = self.tok_desde(de);
        let mut fundo = 0i32;
        while let Some(t) = self.toks.get(i) {
            if t.span.start >= ate || t.kind == Kind::Eof {
                return None;
            }
            if fundo == 0 && t.kind == Kind::Op(op) {
                return Some(i);
            }
            match t.kind {
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => fundo += 1,
                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => fundo -= 1,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// O primeiro token com o texto em `[de, ate)` (no nível de fora).
    pub(super) fn palavra_em(&self, texto: &str, de: usize, ate: usize) -> Option<usize> {
        let mut i = self.tok_desde(de);
        let mut fundo = 0i32;
        while let Some(t) = self.toks.get(i) {
            if t.span.start >= ate || t.kind == Kind::Eof {
                return None;
            }
            if fundo == 0 && matches!(t.kind, Kind::Ident | Kind::Keyword(_)) && t.text(self.fonte) == texto {
                return Some(i);
            }
            match t.kind {
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => fundo += 1,
                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => fundo -= 1,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// O par de um `(`/`[`/`{` (ou o contrário).
    pub(super) fn par(&self, i: usize) -> Option<usize> {
        let (abre, fecha, para_frente) = match self.toks.get(i)?.kind {
            Kind::Op(Op::LParen) => (Op::LParen, Op::RParen, true),
            Kind::Op(Op::LBracket) => (Op::LBracket, Op::RBracket, true),
            Kind::Op(Op::LBrace) => (Op::LBrace, Op::RBrace, true),
            Kind::Op(Op::RParen) => (Op::LParen, Op::RParen, false),
            Kind::Op(Op::RBracket) => (Op::LBracket, Op::RBracket, false),
            Kind::Op(Op::RBrace) => (Op::LBrace, Op::RBrace, false),
            _ => return None,
        };
        let mut fundo = 0i32;
        if para_frente {
            let mut j = i;
            while let Some(t) = self.toks.get(j) {
                if t.kind == Kind::Op(abre) {
                    fundo += 1;
                } else if t.kind == Kind::Op(fecha) {
                    fundo -= 1;
                    if fundo == 0 {
                        return Some(j);
                    }
                }
                j += 1;
            }
            None
        } else {
            let mut j = i as isize;
            while j >= 0 {
                let t = self.toks[j as usize];
                if t.kind == Kind::Op(fecha) {
                    fundo += 1;
                } else if t.kind == Kind::Op(abre) {
                    fundo -= 1;
                    if fundo == 0 {
                        return Some(j as usize);
                    }
                }
                j -= 1;
            }
            None
        }
    }

    /// O `offset` de um token que o fasta criaria sintético quando não está
    /// no texto: o do próximo token real depois de `depois_de`.
    pub(super) fn sintetico(&self, depois_de: usize) -> Span {
        let p = self.tok_depois(depois_de).map_or(self.fonte.len(), |i| self.span_tok(i).start);
        Span { start: p, end: p }
    }

    /// O `beginToken` do nó.
    pub(super) fn inicial(&self, n: usize) -> Inicial {
        let ini = self.ini(n);
        if n == 0 {
            return self.tok_depois(0).map_or(Inicial::Nenhum, Inicial::Tok);
        }
        let resto = &self.fonte[ini.min(self.fonte.len())..];
        if resto.starts_with("//") || resto.starts_with("/*") {
            return Inicial::Comentario;
        }
        match self.tok_em(ini) {
            Some(i) => Inicial::Tok(i),
            None => self.tok_depois(ini).map_or(Inicial::Nenhum, Inicial::Tok),
        }
    }

    /// O primeiro token do nó (depois de comentários e anotações, quando
    /// `depois_de_anotacoes`): o `firstTokenAfterCommentAndMetadata`.
    pub(super) fn primeiro_depois_das_anotacoes(&self, n: usize) -> Option<usize> {
        let mut depois = self.ini(n);
        for &f in self.filhos(n) {
            if matches!(self.especie(f), "Comment" | "Annotation") {
                depois = depois.max(self.fim(f));
            }
        }
        self.tok_depois(depois).filter(|&i| self.span_tok(i).start < self.fim(n).max(depois + 1))
    }

    /// `isFullySynthetic` aqui: o nó não tem nenhum token no texto.
    pub(super) fn todo_sintetico(&self, n: usize) -> bool {
        self.ini(n) >= self.fim(n)
    }

    // -- O nó de completar (§14.13.4) -------------------------------------------

    /// `contém(nó)` do `nodeCovering`, com as duas exceções de fronteira.
    fn contem(&self, n: usize) -> bool {
        let (ini, fim) = (self.ini(n), self.fim(n));
        if self.offset == ini
            && let Some(i) = self.tok_antes(ini)
            && self.span_tok(i).end == self.offset
            && self.e_identificador(i)
        {
            return false;
        }
        if self.offset == fim
            && let Some(i) = self.tok_depois(fim)
            && self.span_tok(i).start == self.offset
            && self.e_identificador(i)
        {
            return false;
        }
        ini <= self.offset && self.offset <= fim
    }

    /// `unit.select(offset, 0).coveringNode`.
    pub(super) fn no_coberto(&self) -> Option<usize> {
        if self.offset > self.fonte.len() {
            return None;
        }
        let mut n = 0usize;
        loop {
            match self.filhos(n).iter().copied().find(|&f| self.contem(f)) {
                Some(f) => n = f,
                None => return Some(n),
            }
        }
    }

    /// `isChildInList` (`in_scope_completion_pass.dart:3964-4048`).
    pub(super) fn filho_em_lista(&self, pai: usize, filho: usize) -> bool {
        let f = self.especie(filho);
        let anotacao = f == "Annotation";
        match self.especie(pai) {
            "AdjacentStrings" | "ArgumentList" | "Block" | "FormalParameterList" | "RecordLiteral" | "HideCombinator" | "ShowCombinator"
            | "DottedName" | "LibraryIdentifier" | "TypeArgumentList" | "TypeParameterList" | "ImplementsClause" | "WithClause"
            | "MixinOnClause" | "RecordTypeAnnotationNamedFields" => true,
            "CascadeExpression" => self.filhos(pai).first() != Some(&filho),
            "ClassDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration" => {
                anotacao || matches!(f, "FieldDeclaration" | "MethodDeclaration" | "ConstructorDeclaration")
            }
            "EnumDeclaration" => anotacao || matches!(f, "EnumConstantDeclaration" | "FieldDeclaration" | "MethodDeclaration" | "ConstructorDeclaration"),
            "Comment" => f == "CommentReference",
            "CompilationUnit" => f != "ScriptTag",
            "ConstructorDeclaration" => {
                anotacao || matches!(f, "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer")
            }
            "ExportDirective" | "ImportDirective" => anotacao || matches!(f, "ShowCombinator" | "HideCombinator" | "Configuration"),
            "ClassTypeAlias" | "DeclaredIdentifier" | "EnumConstantDeclaration" | "FieldDeclaration" | "ForEachPartsWithPattern"
            | "SimpleFormalParameter" | "FieldFormalParameter" | "SuperFormalParameter" | "FunctionTypedFormalParameter"
            | "DefaultFormalParameter" | "FunctionDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias" | "LibraryDirective"
            | "MethodDeclaration" | "PartDirective" | "PartOfDirective" | "PatternVariableDeclaration" | "RecordTypeAnnotationNamedField"
            | "RecordTypeAnnotationPositionalField" | "RepresentationDeclaration" | "TopLevelVariableDeclaration" | "TypeParameter"
            | "VariableDeclaration" => anotacao,
            "ForPartsWithDeclarations" | "ForPartsWithExpression" | "ForPartsWithPattern" => {
                self.arv.partes_de_for.get(&pai).is_some_and(|p| p.atualizacoes.contains(&filho))
            }
            "LabeledStatement" => f == "Label",
            "ListLiteral" | "SetOrMapLiteral" | "ListPattern" | "MapPattern" => f != "TypeArgumentList",
            "ObjectPattern" | "RecordPattern" => f == "PatternField",
            "RecordTypeAnnotation" => f == "RecordTypeAnnotationPositionalField",
            "SwitchExpression" => f == "SwitchExpressionCase",
            "SwitchDefault" => true,
            "SwitchPatternCase" => f != "GuardedPattern",
            // `case e:` antigo: os rótulos e os comandos; a expressão (o
            // primeiro filho que não é rótulo) não.
            "SwitchCase" => self.filhos(pai).iter().copied().find(|&k| self.especie(k) != "Label") != Some(filho),
            "SwitchStatement" => matches!(f, "SwitchCase" | "SwitchDefault" | "SwitchPatternCase"),
            "TryStatement" => f == "CatchClause",
            "VariableDeclarationList" => anotacao || f == "VariableDeclaration",
            _ => false,
        }
    }

    /// `_completionNode` (`in_scope_completion_pass.dart:123-153`).
    pub(super) fn no_de_completar(&self) -> Option<usize> {
        let c = self.no_coberto()?;
        let b = self.inicial(c);
        let b_palavra = matches!(b, Inicial::Tok(i) if self.palavra(i));
        if !b_palavra && self.especie(c) != "SimpleIdentifier" {
            return Some(c);
        }
        // `isCoveredByToken(beginToken)`.
        let coberto = match b {
            Inicial::Tok(i) => self.cobre(self.span_tok(i).start, self.span_tok(i).end),
            _ => false,
        };
        if !coberto {
            return Some(c);
        }
        let mut filho = c;
        let mut pai = self.pai(c);
        while let Some(p) = pai {
            if self.inicial(p) != b || (self.especie(filho) != "SimpleIdentifier" && self.filho_em_lista(p, filho)) {
                break;
            }
            filho = p;
            pai = self.pai(filho);
        }
        if let Some(p) = pai
            && !(self.especie(filho) != "SimpleIdentifier" && self.filho_em_lista(p, filho))
        {
            return Some(p);
        }
        Some(filho)
    }

    // -- Ligação com o AST real ---------------------------------------------------

    pub(super) fn expr_real(&self, n: usize) -> Option<ExprId> {
        match self.lig(n) {
            Ligacao::Expr(e) => Some(e),
            _ => None,
        }
    }

    pub(super) fn stmt_real(&self, n: usize) -> Option<&'a ast::Stmt> {
        match self.lig(n) {
            Ligacao::Stmt(s) => Some(self.a.stmt(s)),
            _ => None,
        }
    }

    pub(super) fn decl_real(&self, n: usize) -> Option<&'a ast::Decl> {
        match self.lig(n) {
            Ligacao::Decl(d) => Some(self.a.decl(d)),
            _ => None,
        }
    }

    pub(super) fn membro_real(&self, n: usize) -> Option<&'a ast::Member> {
        match self.lig(n) {
            Ligacao::Membro(m) => Some(self.a.member(m)),
            _ => None,
        }
    }

    /// A função de um `MethodDeclaration`, `FunctionDeclaration` ou
    /// `FunctionExpression`.
    pub(super) fn funcao_real(&self, n: usize) -> Option<&'a ast::Function> {
        match self.lig(n) {
            Ligacao::Funcao(f) => Some(self.a.function(f)),
            Ligacao::Decl(d) => match &self.a.decl(d).kind {
                ast::DeclKind::Function(f) => Some(self.a.function(*f)),
                _ => None,
            },
            Ligacao::Membro(m) => match &self.a.member(m).kind {
                ast::MemberKind::Method(f) => Some(self.a.function(*f)),
                _ => None,
            },
            Ligacao::Expr(e) => match &self.a.expr(e).kind {
                ast::ExprKind::FunctionExpression(f) => Some(self.a.function(*f)),
                _ => None,
            },
            Ligacao::Stmt(s) => match &self.a.stmt(s).kind {
                ast::StmtKind::Function(f) => Some(self.a.function(*f)),
                _ => None,
            },
            _ => None,
        }
    }

    /// O construtor de um `ConstructorDeclaration`.
    pub(super) fn construtor_real(&self, n: usize) -> Option<&'a ast::Constructor> {
        match &self.membro_real(n)?.kind {
            ast::MemberKind::Constructor(k) => Some(k),
            _ => None,
        }
    }

    /// A lista de variáveis de um `VariableDeclarationList` (pelo pai).
    pub(super) fn lista_real(&self, n: usize) -> Option<&'a ast::VariableList> {
        let p = if self.especie(n) == "VariableDeclarationList" { self.pai(n)? } else { n };
        match self.lig(p) {
            Ligacao::Stmt(s) => match &self.a.stmt(s).kind {
                ast::StmtKind::Variables(l) => Some(l),
                ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => Some(l),
                _ => None,
            },
            Ligacao::Decl(d) => match &self.a.decl(d).kind {
                ast::DeclKind::Variables(l) => Some(l),
                _ => None,
            },
            Ligacao::Membro(m) => match &self.a.member(m).kind {
                ast::MemberKind::Field(l) => Some(l),
                _ => None,
            },
            _ => {
                // `ForPartsWithDeclarations` (de comando ou de elemento).
                let avo = self.pai(p)?;
                match self.lig(avo) {
                    Ligacao::Stmt(s) => match &self.a.stmt(s).kind {
                        ast::StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => Some(l),
                        _ => None,
                    },
                    Ligacao::Elemento(_) => self.elemento_real(avo).and_then(|el| match el {
                        ast::CollectionElement::For { init: Some(ast::ForInit::Variables(l)), .. } => Some(l),
                        _ => None,
                    }),
                    _ => None,
                }
            }
        }
    }

    /// A variável de um `VariableDeclaration` (pelo nome).
    pub(super) fn variavel_real(&self, n: usize) -> Option<&'a ast::Variable> {
        let lista = self.lista_real(self.pai(n)?)?;
        lista.variables.iter().find(|v| v.name.span.start == self.ini(n))
    }

    /// O `ast::Parameter` de um nó de parâmetro.
    pub(super) fn parametro_real(&self, n: usize) -> Option<&'a ast::Parameter> {
        let Ligacao::Parametro(end) = self.lig(n) else { return None };
        let achar = |ps: &'a [ast::Parameter]| ps.iter().find(|p| crate::arvore_analyzer::endereco(*p) == end);
        for f in self.a.functions.iter() {
            if let Some(ps) = &f.parameters {
                if let Some(p) = achar(ps) {
                    return Some(p);
                }
                for p in ps.iter() {
                    if let Some(fps) = &p.function_parameters
                        && let Some(q) = achar(fps)
                    {
                        return Some(q);
                    }
                }
            }
        }
        for m in self.a.members.iter() {
            if let ast::MemberKind::Constructor(k) = &m.kind
                && let Some(p) = achar(&k.parameters)
            {
                return Some(p);
            }
        }
        for t in self.a.types.iter() {
            if let ast::TypeKind::Function { parameters, .. } = &t.kind
                && let Some(p) = achar(parameters)
            {
                return Some(p);
            }
        }
        for d in self.a.decls.iter() {
            if let ast::DeclKind::Typedef(x) = &d.kind
                && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
                && let Some(p) = achar(parameters)
            {
                return Some(p);
            }
        }
        None
    }

    /// O `ast::Initializer` de um nó de inicializador.
    pub(super) fn inicializador_real(&self, n: usize) -> Option<&'a ast::Initializer> {
        let Ligacao::Inicializador(end) = self.lig(n) else { return None };
        for m in self.a.members.iter() {
            if let ast::MemberKind::Constructor(k) = &m.kind
                && let Some(i) = k.initializers.iter().find(|i| crate::arvore_analyzer::endereco(*i) == end)
            {
                return Some(i);
            }
        }
        None
    }

    /// O `ast::SwitchCase` de um membro de `switch`.
    pub(super) fn caso_real(&self, n: usize) -> Option<&'a ast::SwitchCase> {
        let Ligacao::Caso(end) = self.lig(n) else { return None };
        for s in self.a.stmts.iter() {
            if let ast::StmtKind::Switch { cases, .. } = &s.kind
                && let Some(c) = cases.iter().find(|c| crate::arvore_analyzer::endereco(*c) == end)
            {
                return Some(c);
            }
        }
        None
    }

    /// O `ast::Directive`.
    pub(super) fn diretiva_real(&self, n: usize) -> Option<&'a ast::Directive> {
        let Ligacao::Diretiva(end) = self.lig(n) else { return None };
        self.cu.directives.iter().find(|d| crate::arvore_analyzer::endereco(*d) == end)
    }

    /// O `ast::CatchClause`.
    pub(super) fn catch_real(&self, n: usize) -> Option<&'a ast::CatchClause> {
        let Ligacao::Catch(end) = self.lig(n) else { return None };
        for s in self.a.stmts.iter() {
            if let ast::StmtKind::Try { catches, .. } = &s.kind
                && let Some(c) = catches.iter().find(|c| crate::arvore_analyzer::endereco(*c) == end)
            {
                return Some(c);
            }
        }
        None
    }

    /// O `ast::CollectionElement`.
    pub(super) fn elemento_real(&self, n: usize) -> Option<&'a ast::CollectionElement> {
        let Ligacao::Elemento(end) = self.lig(n) else { return None };
        fn em<'x>(els: &'x [ast::CollectionElement], end: usize) -> Option<&'x ast::CollectionElement> {
            for el in els {
                if crate::arvore_analyzer::endereco(el) == end {
                    return Some(el);
                }
                let dentro: Vec<&ast::CollectionElement> = match el {
                    ast::CollectionElement::If { then, else_, .. } => std::iter::once(&**then).chain(else_.as_deref()).collect(),
                    ast::CollectionElement::For { body, .. } | ast::CollectionElement::ForIn { body, .. } => vec![&**body],
                    _ => Vec::new(),
                };
                for d in dentro {
                    if let Some(x) = em(std::slice::from_ref(d), end) {
                        return Some(x);
                    }
                }
            }
            None
        }
        for e in self.a.exprs.iter() {
            if let ast::ExprKind::List { elements, .. } | ast::ExprKind::SetOrMap { elements, .. } = &e.kind
                && let Some(x) = em(elements, end)
            {
                return Some(x);
            }
        }
        None
    }

    /// O `ast::Arguments` de uma `ArgumentList`.
    pub(super) fn argumentos_reais(&self, n: usize) -> Option<&'a ast::Arguments> {
        let Ligacao::Argumentos(end) = self.lig(n) else { return None };
        let e = |a: &'a ast::Arguments| (crate::arvore_analyzer::endereco(a) == end).then_some(a);
        for x in self.a.exprs.iter() {
            match &x.kind {
                ast::ExprKind::Call { arguments, .. } | ast::ExprKind::InstanceCreation { arguments, .. } => {
                    if let Some(a) = e(arguments) {
                        return Some(a);
                    }
                }
                _ => {}
            }
        }
        for m in self.a.members.iter() {
            if let ast::MemberKind::Constructor(k) = &m.kind {
                for i in k.initializers.iter() {
                    if let ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } = i
                        && let Some(a) = e(arguments)
                    {
                        return Some(a);
                    }
                }
            }
        }
        for d in self.a.decls.iter() {
            if let ast::DeclKind::Enum(x) = &d.kind {
                for k in x.constants.iter() {
                    if let Some(a) = k.arguments.as_ref().and_then(e) {
                        return Some(a);
                    }
                }
            }
        }
        let anot = |ms: &'a [ast::Annotation]| ms.iter().find_map(|m| m.arguments.as_ref().and_then(e));
        for d in self.a.decls.iter() {
            if let Some(a) = anot(&d.metadata) {
                return Some(a);
            }
        }
        for m in self.a.members.iter() {
            if let Some(a) = anot(&m.metadata) {
                return Some(a);
            }
        }
        None
    }

    /// O nome de um `ast::Name` do texto real.
    pub(super) fn nome_real(&self, n: ast::Name) -> &'a str {
        self.nr.resolve(n.sym)
    }

    // -- Semântica (a consulta) --------------------------------------------------

    /// O offset de um **início** de trecho do texto real no texto da
    /// consulta: antes da palavra trocada, igual; dentro dela, o início do
    /// sentinela; do fim dela em diante, deslocado (com a palavra vazia, o
    /// que começa no cursor é o token real seguinte).
    pub(super) fn mapear(&self, off: usize) -> usize {
        match self.troca {
            None => off,
            Some((ini, fim, _, delta)) => {
                if off < ini || (off == ini && ini < fim) {
                    off
                } else if off >= fim {
                    (off as isize + delta) as usize
                } else {
                    ini
                }
            }
        }
    }

    /// O offset de um **fim** de trecho: o que termina dentro ou no fim da
    /// palavra trocada termina no fim do sentinela.
    pub(super) fn mapear_fim(&self, off: usize) -> usize {
        match self.troca {
            None => off,
            Some((ini, fim, tamanho, delta)) => {
                if off <= ini {
                    off
                } else if off <= fim && ini < fim {
                    ini + tamanho
                } else {
                    (off as isize + delta) as usize
                }
            }
        }
    }

    /// A expressão da consulta para a expressão real `e`.
    pub(super) fn expr_da_consulta(&self, e: ExprId) -> Option<ExprId> {
        let s = self.a.expr(e).span;
        self.exprs.get(&(self.mapear(s.start), self.mapear_fim(s.end))).copied()
    }

    /// `staticType` da expressão real `e`.
    pub(super) fn tipo_estatico(&self, e: ExprId) -> Option<TypeId> {
        let c = self.expr_da_consulta(e)?;
        self.consulta.corpos.units[self.unidade.0 as usize].get_type(c)
    }

    /// `staticElement` da expressão real `e`.
    pub(super) fn resolvido(&self, e: ExprId) -> Option<Resolved> {
        let c = self.expr_da_consulta(e)?;
        self.consulta.corpos.units[self.unidade.0 as usize].get_resolved(c).cloned()
    }

    /// O tipo de um local ou parâmetro de função local pelo offset do nome.
    pub(super) fn tipo_do_local(&self, nome: usize) -> Option<TypeId> {
        self.consulta.corpos.units[self.unidade.0 as usize].tipos_de_locais.get(&self.mapear(nome)).copied()
    }

    /// A classe (ou mixin, enum, tipo de extensão) declarada com o nome em
    /// `nome` (offset real).
    pub(super) fn classe_em(&self, nome: usize) -> Option<ClassId> {
        let alvo = self.mapear(nome);
        let p = &self.consulta.programa;
        (0..p.classes.len()).map(|i| ClassId(i as u32)).find(|&c| {
            p.class(c).decl.is_some_and(|d| {
                d.unit == self.unidade
                    && match &p.unit(d.unit).ast.decl(d.decl).kind {
                        ast::DeclKind::Class(x) => x.name.span.start == alvo,
                        ast::DeclKind::Mixin(x) => x.name.span.start == alvo,
                        ast::DeclKind::Enum(x) => x.name.span.start == alvo,
                        ast::DeclKind::ExtensionType(x) => x.name.span.start == alvo,
                        _ => false,
                    }
            })
        })
    }

    /// A extensão declarada com o nome em `nome` (ou, sem nome, que começa
    /// em `inicio`).
    pub(super) fn extensao_em(&self, inicio: usize, nome: Option<usize>) -> Option<dartforge_elements::model::ExtensionId> {
        let p = &self.consulta.programa;
        (0..p.extensions.len()).map(|i| dartforge_elements::model::ExtensionId(i as u32)).find(|&x| {
            let ex = p.extension(x);
            ex.decl.unit == self.unidade
                && match (&p.unit(ex.decl.unit).ast.decl(ex.decl.decl).kind, nome) {
                    (ast::DeclKind::Extension(e), Some(n)) => e.name.is_some_and(|k| k.span.start == self.mapear(n)),
                    (ast::DeclKind::Extension(_), None) => p.unit(ex.decl.unit).ast.decl(ex.decl.decl).span.start >= self.mapear(inicio),
                    _ => false,
                }
        })
    }

    /// A classe cuja declaração contém o nó (o `enclosingInterfaceElement`
    /// com o elemento declarado).
    pub(super) fn classe_do_no(&self, n: usize) -> Option<ClassId> {
        let d = self.decl_real(n)?;
        let nome = match &d.kind {
            ast::DeclKind::Class(x) => x.name,
            ast::DeclKind::Mixin(x) => x.name,
            ast::DeclKind::Enum(x) => x.name,
            ast::DeclKind::ExtensionType(x) => x.name,
            _ => return None,
        };
        self.classe_em(nome.span.start)
    }

    /// O elemento do construtor declarado no membro `n`.
    pub(super) fn construtor_declarado(&self, n: usize) -> Option<FunctionElementId> {
        let k = self.construtor_real(n)?;
        let alvo = self.mapear(k.name.unwrap_or(k.class_name).span.start);
        let p = &self.consulta.programa;
        (0..p.functions.len()).map(|i| FunctionElementId(i as u32)).find(|&f| match p.function(f).node {
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => {
                unit == self.unidade
                    && match &p.unit(unit).ast.member(member).kind {
                        ast::MemberKind::Constructor(c) => c.name.unwrap_or(c.class_name).span.start == alvo,
                        _ => false,
                    }
            }
            _ => false,
        })
    }

    /// O elemento da função (topo, método, local) declarada com o nome em
    /// `nome`.
    pub(super) fn funcao_declarada(&self, nome: usize) -> Option<FunctionElementId> {
        let alvo = self.mapear(nome);
        let p = &self.consulta.programa;
        (0..p.functions.len()).map(|i| FunctionElementId(i as u32)).find(|&f| match p.function(f).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                unit == self.unidade && p.unit(unit).ast.function(function).name.is_some_and(|n| n.span.start == alvo)
            }
            _ => false,
        })
    }

    /// A biblioteca do pedido.
    pub(super) fn biblioteca(&self) -> LibraryId {
        self.consulta.programa.unit(self.unidade).library
    }

    // -- Propriedades de contexto ------------------------------------------------

    /// `inStaticContext` (`:3935-3947`): sobe pelos pais; o primeiro método
    /// diz se é estático; o corpo de um construtor é de instância; a raiz é
    /// estática.
    pub(super) fn contexto_estatico(&self, n: usize) -> bool {
        let mut atual = self.pai(n);
        while let Some(k) = atual {
            match self.especie(k) {
                "MethodDeclaration" => {
                    return self.funcao_real(k).is_some_and(|f| f.static_);
                }
                "BlockFunctionBody" | "ExpressionFunctionBody" | "EmptyFunctionBody" | "NativeFunctionBody"
                    if self.pai(k).is_some_and(|p| self.especie(p) == "ConstructorDeclaration") =>
                {
                    return false;
                }
                _ => {}
            }
            atual = self.pai(k);
        }
        true
    }

    /// O nó começa com a palavra `const`.
    pub(super) fn comeca_com_const(&self, n: usize) -> bool {
        matches!(self.inicial(n), Inicial::Tok(i) if self.texto_tok(i) == "const")
    }

    /// `inConstantContext` (`constantContext(includeSelf: false)`,
    /// `AN:src/dart/ast/ast.dart:6208-6262`): sobe pelos pais; anotação,
    /// argumentos de constante de enum e `case` antigo são contexto
    /// constante; criação, record e literal tipado com `const` também; o
    /// padrão constante só com `const`, e a lista de variáveis decide (com
    /// `const` ou não); argumento, expressão, elemento de coleção e
    /// declaração de variável passam adiante; qualquer outro nó encerra.
    pub(super) fn contexto_constante(&self, n: usize) -> bool {
        let mut atual = self.pai(n);
        while let Some(k) = atual {
            let e = self.especie(k);
            match e {
                "Annotation" | "EnumConstantArguments" | "SwitchCase" => return true,
                "ConstantPattern" => return self.comeca_com_const(k),
                "InstanceCreationExpression" | "RecordLiteral" | "ListLiteral" | "SetOrMapLiteral" => {
                    if self.comeca_com_const(k) {
                        return true;
                    }
                }
                "VariableDeclarationList" => return self.lista_real(k).is_some_and(|l| l.const_),
                "ArgumentList" | "IfElement" | "ForElement" | "MapLiteralEntry" | "SpreadElement" | "VariableDeclaration" => {}
                _ if ESPECIES_DE_EXPRESSAO.contains(&e) => {}
                _ => return false,
            }
            atual = self.pai(k);
        }
        false
    }

    /// O `FunctionBody` que contém o nó (o próprio incluso:
    /// `thisOrAncestorOfType<FunctionBody>`).
    pub(super) fn corpo_que_contem(&self, n: usize) -> Option<usize> {
        self.ancestral(n, &["BlockFunctionBody", "ExpressionFunctionBody", "EmptyFunctionBody", "NativeFunctionBody"])
    }

    /// O modificador do corpo que contém o nó (`body.keyword`, `body.star`).
    fn modificador(&self, n: usize) -> Option<ast::AsyncModifier> {
        let corpo = self.corpo_que_contem(n)?;
        let dono = self.pai(corpo)?;
        match self.especie(dono) {
            "FunctionExpression" | "MethodDeclaration" | "FunctionDeclaration" => self.funcao_real(dono).map(|f| f.modifier),
            _ => Some(ast::AsyncModifier::None),
        }
    }

    /// `inAsyncMethodOrFunction`: o corpo é `async` sem `*`.
    pub(super) fn em_async(&self, n: usize) -> bool {
        self.modificador(n) == Some(ast::AsyncModifier::Async)
    }

    /// `inAsyncStarOrSyncStarMethodOrFunction`.
    pub(super) fn em_gerador(&self, n: usize) -> bool {
        matches!(self.modificador(n), Some(ast::AsyncModifier::AsyncStar | ast::AsyncModifier::SyncStar))
    }

    /// `inLoop` (`inDoLoop || inForLoop || inWhileLoop`): algum ancestral
    /// (o próprio incluso) é um laço; não para em funções.
    pub(super) fn em_laco(&self, n: usize) -> bool {
        self.ancestral(n, &["ForStatement", "WhileStatement", "DoStatement"]).is_some()
    }

    /// `inSwitch`: algum ancestral é um `switch` de comando.
    pub(super) fn em_switch(&self, n: usize) -> bool {
        self.ancestral(n, &["SwitchStatement"]).is_some()
    }

    /// `inCatchClause`.
    pub(super) fn em_catch(&self, n: usize) -> bool {
        self.ancestral(n, &["CatchClause"]).is_some()
    }

    /// O comando anterior no mesmo `Block` (`precedingStatement`).
    pub(super) fn comando_anterior(&self, n: usize) -> Option<usize> {
        let p = self.pai(n)?;
        if self.especie(p) != "Block" {
            return None;
        }
        let fs = self.filhos(p);
        let i = fs.iter().position(|&k| k == n)?;
        i.checked_sub(1).map(|j| fs[j])
    }

    /// O membro anterior no corpo da classe (`precedingMember`).
    pub(super) fn membro_anterior(&self, n: usize) -> Option<usize> {
        let p = self.pai(n)?;
        if !matches!(self.especie(p), "ClassDeclaration" | "EnumDeclaration" | "ExtensionDeclaration" | "ExtensionTypeDeclaration" | "MixinDeclaration") {
            return None;
        }
        let membros = self.filhos_de(p, &["FieldDeclaration", "MethodDeclaration", "ConstructorDeclaration"]);
        let i = membros.iter().position(|&k| k == n)?;
        i.checked_sub(1).map(|j| membros[j])
    }

    /// `sortedDirectivesAndDeclarations` da unidade.
    pub(super) fn membros_da_unidade(&self) -> Vec<usize> {
        self.filhos(0).iter().copied().filter(|&k| self.especie(k) != "ScriptTag").collect()
    }

    /// `membersBeforeAndAfterMember`.
    pub(super) fn vizinhos_do_membro(&self, m: usize) -> (Option<usize>, Option<usize>) {
        let ms = self.membros_da_unidade();
        let Some(i) = ms.iter().position(|&k| k == m) else { return (None, None) };
        (i.checked_sub(1).map(|j| ms[j]), ms.get(i + 1).copied())
    }

    /// `membersBeforeAndAfterOffset`.
    pub(super) fn vizinhos_do_offset(&self) -> (Option<usize>, Option<usize>) {
        let mut anterior = None;
        for k in self.membros_da_unidade() {
            if self.offset < self.ini(k) {
                return (anterior, Some(k));
            }
            anterior = Some(k);
        }
        (anterior, None)
    }

    pub(super) fn e_diretiva(&self, n: usize) -> bool {
        self.especie(n).ends_with("Directive")
    }
}
