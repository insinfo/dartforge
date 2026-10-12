//! Parser de Dart 3.6 completo, descendente recursivo, sem resolução.
//!
//! O parser está dividido em módulos por área da gramática, todos como
//! `impl Parser` sobre o mesmo cursor:
//!
//! * [`types`] — anotações de tipo, argumentos e parâmetros de tipo, listas de
//!   parâmetros formais e os *lookaheads* especulativos (`skip_type`);
//! * [`declarations`] — unidade, diretivas, metadata, classes, mixins, enums,
//!   extensions, extension types, typedefs, membros, construtores e corpos de
//!   função;
//! * [`expressions`] — expressões, literais (strings com interpolação e
//!   decodificação de escapes, coleções, records), argumentos, cascatas,
//!   expressões de função e `switch` como expressão;
//! * [`statements`] — statements, declarações locais e cabeçalhos de `for`;
//! * [`patterns`] — padrões (Dart 3).
//!
//! Regras comuns a todos os módulos:
//!
//! * `>` chega sempre isolado do lexer; em contexto de expressão use
//!   `Parser::composed_gt` para ler `>>`, `>>>`, `>=`, `>>=` e `>>>=`.
//! * Identificadores embutidos e contextuais (`abstract`, `async`, `show`,
//!   `when`, `on`…) são [`Kind::Ident`]; compare o texto com
//!   `Parser::at_ident`. Só palavras reservadas são [`Kind::Keyword`].
//! * Um erro produz um [`Diagnostic`] em `Parser::diagnostics` e devolve
//!   [`ParseError`]; o chamador propaga com `?`. A recuperação, quando houver,
//!   fica nas fronteiras de declaração de topo e de membro.
//! * O parser aceita um **superconjunto** de programas válidos onde a
//!   distinção exigiria resolução (por exemplo, qualquer expressão unária
//!   como padrão constante): rejeitar programas inválidos é papel das fases
//!   seguintes, e aceitar todo programa válido é o critério de aceite aqui.
pub mod declarations;
pub(crate) mod fasta;
pub(crate) mod modifiers;
pub mod expressions;
pub mod patterns;
pub mod statements;
pub mod types;

use crate::ast::{Ast, CompilationUnit, ExprId, ForInTarget, ForInit, Name};
use crate::features::{Feature, LanguageVersion, LibraryFeatures, Referencia};
use crate::lexer;
use crate::token::{Keyword, Kind, Op, Token};
use dartforge_diagnostics::{Codigo, Diagnostic, Span, codigos};
use dartforge_intern::Interner;

/// Marcador de falha: o diagnóstico já foi registrado em `Parser::diagnostics`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError;

/// Resultado das funções do parser.
pub type PResult<T> = Result<T, ParseError>;

/// Composição de `>` isolados em contexto de expressão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposedGt {
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `>>`
    Shr,
    /// `>>>`
    UShr,
    /// `>>=`
    ShrAssign,
    /// `>>>=`
    UShrAssign,
}

impl ComposedGt {
    /// Quantos tokens a composição consome (nunca zero: `>` sozinho é 1).
    #[allow(clippy::len_without_is_empty)]
    pub fn len(self) -> usize {
        match self {
            ComposedGt::Gt => 1,
            ComposedGt::GtEq | ComposedGt::Shr => 2,
            ComposedGt::UShr | ComposedGt::ShrAssign => 3,
            ComposedGt::UShrAssign => 4,
        }
    }
}

/// Cabeçalho de `for (...)` já consumido, incluindo os parênteses.
#[derive(Debug)]
pub enum ForHeader {
    Classic {
        init: Option<ForInit>,
        condition: Option<ExprId>,
        updates: Vec<ExprId>,
    },
    In {
        target: ForInTarget,
        iterable: ExprId,
    },
}

/// Resultado de [`parse`].
#[derive(Debug)]
pub struct Parsed {
    pub unit: CompilationUnit,
    pub ast: Ast,
    pub diagnostics: Vec<Diagnostic>,
    /// Os trechos da fonte cujos tokens a recuperação de erro descartou (do
    /// primeiro ao último token pulado): a declaração, o membro ou o comando
    /// que não fechou, e o que `ensureCloseParen` saltou. Um nome citado
    /// num desses trechos pode ter uso ou declaração que a árvore não tem
    /// (docs/ANALYZER-ESPECIFICACAO.md §G, T5, estrutura (b)).
    pub pulados: Vec<Span>,
    /// Qual analyzer é a referência de nomes de código e de textos desta
    /// unidade: o 3.13.4 se a biblioteca está acima da 3.6 ou se a unidade
    /// usa sintaxe de um recurso desligado que o 3.6.2 não conhece
    /// ([`Feature::DESCONHECIDOS_NO_3_6`]); senão o 3.6.2
    /// (docs/ANALYZER-ESPECIFICACAO.md, T2).
    pub referencia: Referencia,
}

/// Analisa uma unidade de compilação inteira na versão de linguagem corrente
/// ([`LibraryFeatures::atual`]); ver [`parse_com`].
///
/// Os erros do scanner não interrompem a leitura (o lexer segue como o do fasta); todos ficam
/// em [`Parsed::diagnostics`], e a unidade devolvida contém o que foi lido
/// até o ponto de recuperação mais recente. Lista vazia significa que a
/// unidade inteira foi aceita pela gramática.
///
/// ```
/// let mut names = dartforge_intern::Interner::new();
/// let output = dartforge_frontend::parser::parse("void main() { print('oi'); }", &mut names);
/// assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
/// assert_eq!(output.unit.declarations.len(), 1);
/// ```
pub fn parse(source: &str, interner: &mut Interner) -> Parsed {
    parse_com(source, interner, LibraryFeatures::atual())
}

/// Como [`parse`], com os recursos da biblioteca a que a unidade pertence
/// (`docs/VERSOES-LINGUAGEM.md`). A gramática aceita é sempre o
/// superconjunto; um recurso desligado vira diagnóstico, e onde a gramática
/// **diverge** entre versões (`factory() {}`, `var`/`final` em parâmetro) a
/// versão decide.
pub fn parse_com(source: &str, interner: &mut Interner, features: LibraryFeatures) -> Parsed {
    parse_lexed_with(source, lexer::lex(source), interner, features)
}

/// Os erros do scanner do fasta que não interrompem a leitura (o token
/// sintético segue para o parser), tirados dos lexemas dos tokens:
/// `UNSUPPORTED_OPERATOR` no começo de `===`/`!==` (com o lexema),
/// `MISSING_DIGIT` no último caractere de um `double` cujo expoente não tem
/// dígito e `MISSING_HEX_DIGIT` no último caractere de um `0x` sem dígito
/// hexadecimal (`fe:scanner/errors.dart`, `translateErrorToken`). Todos com
/// comprimento 1.
fn recoverable_scanner_errors(source: &str, tokens: &[Token]) -> Vec<Diagnostic> {
    use crate::token::{Kind, Op};
    let mut output = Vec::new();
    let one = |start_offset: usize| Span { start: start_offset, end: start_offset + 1 };
    for t in tokens {
        let lexeme = &source[t.span.start..t.span.end];
        match t.kind {
            Kind::Op(Op::EqEq | Op::BangEq) if lexeme.len() == 3 => {
                output.push(Diagnostic::com_codigo(dartforge_diagnostics::codigos::scanner::UNSUPPORTED_OPERATOR, one(t.span.start), [lexeme]));
            }
            Kind::Double => {
                if let Some(i) = lexeme.find(['e', 'E']) {
                    let exponent = &lexeme[i + 1..];
                    if !exponent.bytes().any(|b| b.is_ascii_digit()) {
                        output.push(Diagnostic::com_codigo(dartforge_diagnostics::codigos::scanner::MISSING_DIGIT, one(t.span.end - 1), Vec::<&str>::new()));
                    }
                }
            }
            // `translateErrorToken` (`errors.dart:36`): no caractere antes do
            // `endOffset`, que é a quebra de linha ou o fim do arquivo.
            Kind::Str(f) | Kind::StrEnd(f) if f.aberta => {
                let mut ultimo = t.span.end.saturating_sub(1);
                while ultimo > 0 && !source.is_char_boundary(ultimo) {
                    ultimo -= 1;
                }
                output.push(Diagnostic::com_codigo(dartforge_diagnostics::codigos::scanner::UNTERMINATED_STRING_LITERAL, one(ultimo), Vec::<&str>::new()));
            }
            // `NonAsciiIdentifierToken`: cada caractere fora do
            // identificador ASCII, com o ponto de código.
            Kind::Ident if !lexeme.chars().all(crate::lexer::identifier_char) => {
                for (i, ch) in lexeme.char_indices() {
                    if !crate::lexer::identifier_char(ch) {
                        let start_offset = t.span.start + i;
                        let n = (ch as u32).to_string();
                        output.push(Diagnostic::com_codigo(
                            dartforge_diagnostics::codigos::scanner::ILLEGAL_CHARACTER,
                            Span { start: start_offset, end: start_offset + ch.len_utf8() },
                            [n.as_str()],
                        ));
                    }
                }
            }
            Kind::Int => {
                if (lexeme.starts_with("0x") || lexeme.starts_with("0X")) && !lexeme[2..].bytes().any(|b| b.is_ascii_hexdigit()) {
                    output.push(Diagnostic::com_codigo(dartforge_diagnostics::codigos::scanner::MISSING_HEX_DIGIT, one(t.span.end - 1), Vec::<&str>::new()));
                }
            }
            _ => {}
        }
    }
    // Entre os tokens só há espaço e comentário, mais o que o scanner pulou
    // (controle ASCII, espaço não ASCII) e o comentário de bloco sem fecho
    // (`UNTERMINATED_MULTI_LINE_COMMENT` no último caractere, `errors.dart:39-44`).
    let mut previous_end = 0usize;
    for t in tokens {
        lacuna(source, previous_end, t.span.start, &mut output);
        previous_end = previous_end.max(t.span.end);
    }
    output
}

/// Os erros do scanner no trecho `[de, until)` entre dois tokens.
fn lacuna(source: &str, de: usize, until: usize, output: &mut Vec<Diagnostic>) {
    if de >= until {
        return;
    }
    let bytes = source.as_bytes();
    let mut i = de;
    while i < until {
        if bytes[i..].starts_with(b"//") {
            while i < until && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"/*") {
            let mut prof = 1usize;
            i += 2;
            while i < bytes.len() && prof != 0 {
                if bytes[i..].starts_with(b"/*") {
                    prof += 1;
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    prof -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if prof != 0 {
                let end_offset = bytes.len().saturating_sub(1);
                output.push(Diagnostic::com_codigo(
                    dartforge_diagnostics::codigos::scanner::UNTERMINATED_MULTI_LINE_COMMENT,
                    Span { start: end_offset, end: end_offset + 1 },
                    Vec::<&str>::new(),
                ));
            }
            continue;
        }
        let Some(ch) = source[i..].chars().next() else { break };
        if !ch.is_ascii_whitespace() && crate::lexer::skipped_char(ch) {
            let n = (ch as u32).to_string();
            output.push(Diagnostic::com_codigo(
                dartforge_diagnostics::codigos::scanner::ILLEGAL_CHARACTER,
                Span { start: i, end: i + ch.len_utf8() },
                [n.as_str()],
            ));
        }
        i += ch.len_utf8();
    }
}

/// Como [`parse`], com a fonte já lexada. O lexer é puro (não interna nomes),
/// então pode correr noutra thread; só a análise sintática precisa do
/// `Interner` e fica sequencial.
pub fn parse_lexed(
    source: &str,
    tokens: Result<Vec<Token>, Diagnostic>,
    interner: &mut Interner,
) -> Parsed {
    parse_lexed_with(source, tokens, interner, LibraryFeatures::atual())
}

/// [`parse_lexed`] com os recursos da biblioteca; ver [`parse_com`].
pub fn parse_lexed_with(
    source: &str,
    tokens: Result<Vec<Token>, Diagnostic>,
    interner: &mut Interner,
    features: LibraryFeatures,
) -> Parsed {
    let tokens = match tokens {
        Ok(tokens) => tokens,
        Err(diagnostic) => {
            return Parsed {
                unit: CompilationUnit::default(),
                ast: Ast::default(),
                diagnostics: vec![diagnostic],
                // Erro léxico: nada foi lido, a fonte inteira ficou de fora.
                pulados: vec![Span { start: 0, end: source.len() }],
                referencia: if features.versao() > LanguageVersion::PISO { Referencia::V3_13 } else { Referencia::V3_6 },
            };
        }
    };
    let mut from_scanner = recoverable_scanner_errors(source, &tokens);
    // Os fechos que faltam (e o erro de cada um), como o scanner do fasta.
    let (tokens, group_of, mut closers) = crate::grouping::group_tokens(source, tokens);
    for f in &mut closers {
        f.erro += from_scanner.len();
    }
    from_scanner.extend(group_of);
    // A leitura, no modo do 3.13.4 ou não. A árvore de algumas construções
    // diverge entre as referências (`factory new()`: construtor no 3.13.4,
    // campo `factory` no 3.6.2), e a referência só se sabe no fim da unidade
    // (a sintaxe nova pode vir depois): a unidade que divergiu e terminou na
    // referência 3.13 é lida de novo no modo 3.13.
    let ler = |interner: &mut Interner, mode_313: bool| {
        let mut parser = Parser::new(source, tokens.clone(), interner);
        parser.synthetic_closers = closers.clone();
        parser.features = features;
        parser.mode_313 = mode_313;
        parser.diagnostics.extend(from_scanner.iter().cloned());
        let unit = parser.parse_compilation_unit();
        let redo = !mode_313 && parser.diverged_313 && parser.new_syntax;
        (unit, parser.ast, parser.diagnostics, parser.pulados, parser.new_syntax, parser.so_3_6, parser.so_3_13, redo)
    };
    let mut reading = ler(interner, features.versao() > LanguageVersion::PISO);
    if reading.7 {
        reading = ler(interner, true);
    }
    let (unit, mut ast, diags, pulados, new_syntax, so_3_6, so_3_13, _) = reading;
    // A árvore devolvida vive muito (um editor a retém por arquivo aberto) e
    // as arenas cresceram em potências de dois: no `new_sali` a folga era
    // 29 MiB de 132 MiB. Uma realocação por arena aqui é mais barata do que
    // reter a folga pela vida inteira da unidade.
    ast.shrink_to_fit();
    let referencia =
        if features.versao() > LanguageVersion::PISO || new_syntax { Referencia::V3_13 } else { Referencia::V3_6 };
    // Os diagnósticos que só a outra referência relata saem agora, que se
    // sabe qual é a desta unidade.
    let mut diagnostics = diags;
    let fora = if referencia == Referencia::V3_13 { &so_3_6 } else { &so_3_13 };
    if !fora.is_empty() {
        diagnostics.retain(|d| !d.code.is_some_and(|c| fora.contains(&(c, d.span))));
    }
    Parsed {
        unit,
        ast,
        diagnostics,
        pulados,
        referencia,
    }
}

/// Uma reescrita do fluxo de tokens, para as tentativas a desfazerem.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Splice {
    /// `n` tokens inseridos em `em`.
    Insertion { em: usize, n: usize },
    /// O fecho sintético `closer` saiu de `de` (onde era `token`, em `em`)
    /// para `to`; o erro dele tinha `error_span`.
    Move { de: usize, to: usize, token: Token, closer: usize, em: usize, error_span: Option<Span> },
}

/// Cursor sobre os tokens de uma unidade mais as arenas em construção.
pub struct Parser<'s, 'i> {
    pub(crate) source: &'s str,
    pub(crate) tokens: Vec<Token>,
    pub(crate) pos: usize,
    pub(crate) ast: Ast,
    pub(crate) interner: &'i mut Interner,
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Ver [`Parsed::pulados`].
    pub(crate) pulados: Vec<Span>,
    /// Profundidade de aninhamento de expressões/statements, para recusar
    /// entradas patológicas antes de estourar a pilha.
    pub(crate) depth: u32,
    /// Dentro de um corpo `async`/`async*`: `await` e `yield` são palavras-chave.
    pub(crate) in_async: bool,
    /// Dentro de um corpo `sync*`/`async*`: `yield` é palavra-chave.
    pub(crate) in_generator: bool,
    /// Dentro de `<...>` de tipo: `>` nunca se compõe.
    pub(crate) in_type_args: u32,
    /// Rascunho de argumentos de chamada, compartilhado por todas as
    /// chamadas (aninhadas empilham acima da anterior); ver
    /// [`Parser::parse_arguments`].
    pub(crate) scratch_args: Vec<crate::ast::Argument>,
    /// Rascunho de trechos de string literal; ver
    /// [`Parser::parse_string_literal`].
    pub(crate) scratch_parts: Vec<crate::ast::StringPart>,
    /// Recursos da biblioteca (versão de linguagem e experimentos).
    pub(crate) features: LibraryFeatures,
    /// Lendo a lista de parâmetros de um construtor primário (Dart 3.13):
    /// `var`/`final` declaram campo e nomeado privado declarante é permitido.
    pub(crate) in_primary_constructor: bool,
    /// Lendo a representação de um extension type (que relata o `var` da
    /// representação à parte, em `parse_representation`).
    pub(crate) in_representation: bool,
    /// O token que abre o corpo (`{` ou `=>`) de cada parte `this` de
    /// construtor primário, pelo início do `this`: a elaboração, que sabe se
    /// o construtor é `const`, relata ali os erros de corpo do analyzer.
    pub(crate) primary_bodies: std::collections::HashMap<usize, Span>,
    /// As reinserções de tokens no fluxo (o `rewriter.insertToken` do
    /// fasta), como `(índice, quantidade)`, na ordem em que foram feitas:
    /// as tentativas especulativas as desfazem ao recuar
    /// ([`Parser::undo_splices`]).
    pub(crate) splices: Vec<Splice>,
    /// Os fechos sintéticos do scanner ([`crate::grouping::group_tokens`]);
    /// o `erro` de cada um é o índice em `diagnostics`.
    pub(crate) synthetic_closers: Vec<crate::grouping::SyntheticCloser>,
    /// Numa tentativa de `spelled_out_operator` (o `_currentlyRecovering` do
    /// fasta): não se tenta outra dentro dela.
    pub(crate) recovering_operator: bool,
    /// A declaração cujos membros estão sendo lidos (o `DeclarationKind` do
    /// fasta): decide os erros próprios de extension e mixin.
    pub(crate) owner: MemberOwner,
    /// O dono da próxima lista de parâmetros formais (o `MemberKind` do
    /// fasta), posto por quem a lê logo antes; a lista o consome (as listas
    /// aninhadas, de parâmetro-função, são `Other`).
    pub(crate) params_owner: modifiers::ParameterOwner,
    /// O nome da declaração cujo membro está sendo lido (`None` no topo):
    /// campo, getter ou setter com ele é `MEMBER_WITH_CLASS_NAME`.
    pub(crate) enclosing_name: Option<&'s str>,
    /// O `PatternContext` do fasta do padrão em leitura.
    pub(crate) default_context: patterns::PatternContext,
    /// O `DirectiveState` do fasta (`directive_context.dart`).
    pub(crate) directive_state: DirectiveState,
    /// Numa tentativa especulativa de `recover_member`: o `;` que falta não
    /// é inserido diante de qualquer token (a tentativa só vinga com
    /// membro bem terminado).
    pub(crate) speculating: bool,
    /// O lugar do nome sintético da classe/mixin em leitura (recuperação de
    /// `declaration_name`), onde vai o erro de corpo ausente.
    pub(crate) synthetic_name: Option<dartforge_diagnostics::Span>,
    /// A unidade usa sintaxe de um recurso desligado que o analyzer 3.6.2
    /// não conhece: a referência dela é o 3.13.4 ([`Parsed::referencia`]).
    pub(crate) new_syntax: bool,
    /// Lê as construções em que a árvore do 3.6.2 e a do 3.13.4 divergem
    /// pelo caminho do 3.13.4 (`factory new()`): a unidade com versão acima
    /// do piso, ou a segunda leitura de uma unidade que terminou na
    /// referência 3.13 ([`Self::diverged_313`]).
    pub(crate) mode_313: bool,
    /// A leitura fora do [`Self::mode_313`] passou por uma construção em que
    /// as árvores divergem: se a unidade terminar na referência 3.13, ela é
    /// lida de novo no modo 3.13.
    pub(crate) diverged_313: bool,
    /// Lendo os elementos de um literal de lista: o elemento comum não lê
    /// `: value` (`parseLiteralListSuffix` lê as entradas com
    /// `parseExpression`); só o null-aware `?k: v` vira entrada.
    pub(crate) in_list: bool,
    /// Os diagnósticos (código e lugar) que só o analyzer 3.6.2 relata; saem
    /// da lista se a referência da unidade acabar sendo o 3.13.4. Guardados
    /// por código e lugar, não por índice: as tentativas especulativas
    /// truncam a lista de diagnósticos.
    pub(crate) so_3_6: Vec<(Codigo, Span)>,
    /// O inverso: os que só o 3.13.4 relata.
    pub(crate) so_3_13: Vec<(Codigo, Span)>,
    /// O `!mayParseFunctionExpressions` do fasta: lendo um inicializador de
    /// construtor ou o padrão e a guarda de um caso de `switch` expressão,
    /// onde `nome(…) {` e `nome(…) =>` não são função literal nomeada (o
    /// `{` é o corpo do construtor e o `=>` é o do caso).
    pub(crate) without_named_function: bool,
}

/// `DirectiveState` (`directive_context.dart`), na ordem do enum do fasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DirectiveState {
    NoneFound,
    Script,
    Library,
    ImportExport,
    Part,
    PartOf,
    Declarations,
}

/// O `DeclarationKind` do fasta para os membros em leitura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemberOwner {
    Class,
    Mixin,
    Enum,
    Extension,
    ExtensionType,
}

/// Limite de aninhamento antes de um diagnóstico de profundidade.
pub const MAX_DEPTH: u32 = 512;

impl<'s, 'i> Parser<'s, 'i> {
    pub(crate) fn new(source: &'s str, tokens: Vec<Token>, interner: &'i mut Interner) -> Self {
        Parser {
            source,
            tokens,
            pos: 0,
            ast: Ast::default(),
            interner,
            diagnostics: Vec::new(),
            pulados: Vec::new(),
            depth: 0,
            in_async: false,
            in_generator: false,
            in_type_args: 0,
            scratch_args: Vec::new(),
            scratch_parts: Vec::new(),
            features: LibraryFeatures::atual(),
            in_primary_constructor: false,
            in_representation: false,
            primary_bodies: std::collections::HashMap::new(),
            splices: Vec::new(),
            synthetic_closers: Vec::new(),
            recovering_operator: false,
            owner: MemberOwner::Class,
            params_owner: modifiers::ParameterOwner::Other,
            enclosing_name: None,
            default_context: patterns::PatternContext::Matching,
            directive_state: DirectiveState::NoneFound,
            speculating: false,
            synthetic_name: None,
            new_syntax: false,
            mode_313: false,
            diverged_313: false,
            in_list: false,
            so_3_6: Vec::new(),
            so_3_13: Vec::new(),
            without_named_function: false,
        }
    }

    // -- Cursor ---------------------------------------------------------

    /// Token corrente (o último é sempre [`Kind::Eof`]).
    pub(crate) fn peek(&self) -> Token {
        self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    /// Token `n` posições à frente, saturando em `Eof`.
    pub(crate) fn peek_at(&self, n: usize) -> Token {
        self.tokens[(self.pos + n).min(self.tokens.len() - 1)]
    }

    pub(crate) fn kind(&self) -> Kind {
        self.peek().kind
    }

    pub(crate) fn kind_at(&self, n: usize) -> Kind {
        self.peek_at(n).kind
    }

    pub(crate) fn span(&self) -> Span {
        self.peek().span
    }

    /// Texto do token corrente.
    pub(crate) fn text(&self) -> &'s str {
        let token = self.peek();
        &self.source[token.span.start..token.span.end]
    }

    pub(crate) fn text_at(&self, n: usize) -> &'s str {
        let token = self.peek_at(n);
        &self.source[token.span.start..token.span.end]
    }

    /// Texto de um token qualquer.
    pub(crate) fn token_text(&self, token: Token) -> &'s str {
        &self.source[token.span.start..token.span.end]
    }

    /// Avança e devolve o token consumido.
    pub(crate) fn advance(&mut self) -> Token {
        let token = self.peek();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        token
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.kind() == Kind::Eof
    }

    pub(crate) fn at_op(&self, op: Op) -> bool {
        self.kind() == Kind::Op(op)
    }

    pub(crate) fn at_op_at(&self, n: usize, op: Op) -> bool {
        self.kind_at(n) == Kind::Op(op)
    }

    pub(crate) fn at_kw(&self, kw: Keyword) -> bool {
        self.kind() == Kind::Keyword(kw)
    }

    pub(crate) fn at_kw_at(&self, n: usize, kw: Keyword) -> bool {
        self.kind_at(n) == Kind::Keyword(kw)
    }

    /// O token corrente é um identificador (qualquer, inclusive embutido).
    pub(crate) fn at_identifier(&self) -> bool {
        self.kind() == Kind::Ident
    }

    pub(crate) fn at_identifier_at(&self, n: usize) -> bool {
        self.kind_at(n) == Kind::Ident
    }

    /// O token corrente é o identificador contextual `text`.
    pub(crate) fn at_ident(&self, text: &str) -> bool {
        self.kind() == Kind::Ident && self.text() == text
    }

    pub(crate) fn at_ident_at(&self, n: usize, text: &str) -> bool {
        self.kind_at(n) == Kind::Ident && self.text_at(n) == text
    }

    /// Consome o operador se for o corrente.
    pub(crate) fn eat_op(&mut self, op: Op) -> bool {
        if self.at_op(op) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn eat_kw(&mut self, kw: Keyword) -> bool {
        if self.at_kw(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consome o identificador contextual se for o corrente.
    pub(crate) fn eat_ident(&mut self, text: &str) -> bool {
        if self.at_ident(text) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(crate) fn expect_op(&mut self, op: Op) -> PResult<Token> {
        if self.at_op(op) {
            Ok(self.advance())
        } else {
            Err(self.expected_error(op.text()))
        }
    }

    pub(crate) fn expect_kw(&mut self, kw: Keyword) -> PResult<Token> {
        if self.at_kw(kw) {
            Ok(self.advance())
        } else {
            Err(self.expected_error(kw.text()))
        }
    }

    /// Exige o identificador contextual `text`.
    pub(crate) fn expect_ident(&mut self, text: &str) -> PResult<Token> {
        if self.at_ident(text) {
            Ok(self.advance())
        } else {
            Err(self.expected_error(text))
        }
    }

    /// Exige um identificador qualquer e devolve seu [`Name`].
    pub(crate) fn expect_identifier(&mut self) -> PResult<Name> {
        if self.at_identifier() {
            Ok(self.identifier())
        } else {
            Err(self.identifier_error())
        }
    }

    /// Consome o identificador corrente (precondição: [`Parser::at_identifier`]).
    pub(crate) fn identifier(&mut self) -> Name {
        let token = self.advance();
        let sym = self
            .interner
            .intern(&self.source[token.span.start..token.span.end]);
        Name {
            sym,
            span: token.span,
        }
    }

    /// Interna um texto arbitrário como nome (operadores, `unary-`, `[]=`).
    pub(crate) fn name_from(&mut self, text: &str, span: Span) -> Name {
        Name {
            sym: self.interner.intern(text),
            span,
        }
    }

    /// Span do início dado até o fim do último token consumido.
    /// Reinsere no cursor cópias dos tokens de `indices`, nessa ordem (o
    /// `rewriter.insertToken` do fasta): são os próximos a serem lidos.
    pub(crate) fn reinject(&mut self, indices: &[usize]) {
        let copies: Vec<Token> = indices.iter().map(|&i| Token { glued: false, ..self.tokens[i] }).collect();
        let n = copies.len();
        self.tokens.splice(self.pos..self.pos, copies);
        self.splices.push(Splice::Insertion { em: self.pos, n });
    }

    /// `rewriter.moveSynthetic` (`token_stream_rewriter.dart:80`): se o fecho
    /// do abridor em `abre` é sintético, ele (e o erro do scanner dele) vai
    /// para o cursor, na posição do token corrente, e é o próximo a ser lido.
    pub(crate) fn move_synthetic_closer(&mut self, abre: usize) -> bool {
        let Some(start_offset) = self.tokens.get(abre).map(|t| t.span.start) else { return false };
        let Some(k) = self.synthetic_closers.iter().position(|f| f.abre == start_offset) else { return false };
        let f = self.synthetic_closers[k];
        let Some(j) = (self.pos..self.tokens.len())
            .find(|&j| self.tokens[j].span.start == f.em && self.tokens[j].span.end == f.em && self.tokens[j].kind != Kind::Eof)
        else {
            return false;
        };
        if j == self.pos {
            return true;
        }
        let token = self.tokens.remove(j);
        let novo = self.tokens[self.pos].span.start;
        self.tokens.insert(self.pos, Token { span: Span { start: novo, end: novo }, ..token });
        let error_span = self.diagnostics.get(f.erro).map(|d| d.span);
        if let Some(d) = self.diagnostics.get_mut(f.erro) {
            d.span = Span { start: novo, end: novo + 1 };
        }
        self.synthetic_closers[k].em = novo;
        self.splices.push(Splice::Move { de: j, to: self.pos, token, closer: k, em: f.em, error_span });
        true
    }

    /// Desfaz as emendas feitas depois que havia `mark` delas.
    pub(crate) fn undo_splices(&mut self, mark: usize) {
        while self.splices.len() > mark {
            match self.splices.pop().expect("emenda") {
                Splice::Insertion { em, n } => {
                    self.tokens.drain(em..em + n);
                }
                Splice::Move { de, to, token, closer, em, error_span } => {
                    self.tokens.remove(to);
                    self.tokens.insert(de, token);
                    self.synthetic_closers[closer].em = em;
                    let erro = self.synthetic_closers[closer].erro;
                    if let (Some(d), Some(s)) = (self.diagnostics.get_mut(erro), error_span) {
                        d.span = s;
                    }
                }
            }
        }
    }

    pub(crate) fn span_from(&self, start: Span) -> Span {
        let end = if self.pos == 0 {
            start.end
        } else {
            self.tokens[self.pos - 1].span.end
        };
        Span {
            start: start.start,
            end: end.max(start.start),
        }
    }

    /// Fim do último token consumido.
    pub(crate) fn last_end(&self) -> usize {
        if self.pos == 0 {
            0
        } else {
            self.tokens[self.pos - 1].span.end
        }
    }

    // -- Diagnósticos ---------------------------------------------------

    /// Registra o diagnóstico `code` (do `ParserErrorCode`/`ScannerErrorCode`
    /// do analyzer, mensagem oficial em inglês) em `span` e devolve o marcador.
    pub(crate) fn error_at(&mut self, code: Codigo, span: Span, args: &[&str]) -> ParseError {
        self.diagnostics.push(Diagnostic::com_codigo(code, span, args.iter().copied()));
        ParseError
    }

    /// `code` no token corrente — onde o fasta reporta o que falta ou sobra.
    pub(crate) fn erro(&mut self, code: Codigo, args: &[&str]) -> ParseError {
        // `findNonZeroLengthToken`: o erro num fecho sintético vai para o
        // primeiro token de verdade depois dele.
        let i = (self.pos..self.tokens.len())
            .find(|&i| self.tokens[i].span.start < self.tokens[i].span.end || self.tokens[i].kind == Kind::Eof)
            .unwrap_or(self.pos);
        let span = if i == self.pos { self.span() } else { self.tokens[i].span };
        self.error_at(code, span, args)
    }

    /// `EXPECTED_TOKEN`, no lugar em que o analyzer o põe: `;` que falta vai no
    /// token **anterior** (`ensureSemicolon` do fasta); fecho `)`/`]`/`}` que
    /// falta no fim do arquivo é erro do scanner, no fim, com comprimento 1
    /// (`translateErrorToken`); o resto, no token corrente.
    pub(crate) fn expected_error(&mut self, texto: &str) -> ParseError {
        if texto == ";" && self.pos > 0 {
            // `findPreviousNonZeroLengthToken`: os sintéticos não contam.
            let anterior = (0..self.pos).rev().find(|&i| self.tokens[i].span.start < self.tokens[i].span.end).unwrap_or(self.pos - 1);
            let span = self.tokens[anterior].span;
            return self.error_at(codigos::parser::EXPECTED_TOKEN, span, &[";"]);
        }
        if matches!(texto, ")" | "]" | "}") && self.kind() == Kind::Eof {
            let s = self.span().start;
            return self.error_at(codigos::scanner::EXPECTED_TOKEN, Span { start: s, end: s + 1 }, &[texto]);
        }
        // Um `>` que o scanner do fasta lê junto do seguinte (`>=`, `>>`,
        // `>>=`…) é um token só lá: o erro cobre a composição inteira.
        if self.in_type_args == 0
            && let Some(c) = self.composed_gt()
            && c.len() > 1
        {
            let s = self.span().start;
            let end_offset = self.peek_at(c.len() - 1).span.end;
            return self.error_at(codigos::parser::EXPECTED_TOKEN, Span { start: s, end: end_offset }, &[texto]);
        }
        self.erro(codigos::parser::EXPECTED_TOKEN, &[texto])
    }

    /// Como [`Parser::error_at`], sem repetir um diagnóstico igual (código e
    /// intervalo) já registrado: o analyzer não relata duas vezes o mesmo
    /// erro no mesmo lugar (o fasta às vezes o pede duas vezes, como o
    /// `INVALID_CONSTANT_CONST_PREFIX` de `const 1 + 2`).
    pub(crate) fn single_error(&mut self, code: Codigo, span: Span, args: &[&str]) {
        if !self.diagnostics.iter().any(|d| d.code == Some(code) && d.span == span) {
            self.error_at(code, span, args);
        }
    }

    /// `ensureSemicolon` do fasta: sem `;`, `EXPECTED_TOKEN` (`;`) no último
    /// token lido e a análise segue como se ele estivesse ali (o fasta
    /// insere um `;` sintético), sem pular nada.
    pub(crate) fn ensure_semicolon(&mut self) -> PResult<()> {
        if !self.eat_op(Op::Semicolon) {
            self.expected_error(";");
        }
        Ok(())
    }

    /// `ensureSemicolon` sem a prudência de [`Parser::ensure_semicolon`]:
    /// para onde a construção acabou num token certo (nome, URI, prefixo), o
    /// `;` que falta é relatado no último token lido e a análise segue no
    /// token corrente, qualquer que seja.
    pub(crate) fn ensure_semicolon_forced(&mut self) {
        if !self.eat_op(Op::Semicolon) {
            self.expected_error(";");
        }
    }

    /// Registra como pulados os tokens de `de` até o cursor (o que uma
    /// recuperação acabou de descartar), juntando com o trecho anterior
    /// quando são contíguos ou se sobrepõem.
    pub(crate) fn record_skipped(&mut self, de: usize) {
        if self.pos <= de || de >= self.tokens.len() {
            return;
        }
        let start_offset = self.tokens[de].span.start;
        let end_offset = self.tokens[self.pos - 1].span.end;
        if end_offset <= start_offset {
            return;
        }
        if let Some(ultimo) = self.pulados.last_mut() {
            if start_offset <= ultimo.end && ultimo.start <= end_offset {
                ultimo.start = ultimo.start.min(start_offset);
                ultimo.end = ultimo.end.max(end_offset);
                return;
            }
        }
        self.pulados.push(Span { start: start_offset, end: end_offset });
    }

    /// `ensureCloseParen` do fasta: o `)` que fecha o `(` na posição absoluta
    /// `abre`. Se o token corrente não é ele, `EXPECTED_TOKEN` (`)`) no token
    /// corrente e o cursor pula para depois do `)` casado — o que fica no
    /// meio não é lido, como no fasta. Sem `)` casado (fim de arquivo), o
    /// erro de sempre.
    pub(crate) fn ensure_close_paren(&mut self, abre: usize) -> PResult<()> {
        if self.eat_op(Op::RParen) {
            return Ok(());
        }
        // `ensureCloseParen`: o `)` sintético fora do lugar vem para cá (o
        // erro é o do scanner).
        if self.move_synthetic_closer(abre) {
            self.advance();
            return Ok(());
        }
        match self.matching_close(abre) {
            Some(closes) if closes >= self.pos && self.kind_of(closes) == Kind::Op(Op::RParen) && !self.at_eof() => {
                self.expected_error(")");
                let de = self.pos;
                self.pos = closes;
                self.record_skipped(de);
                self.pos = closes + 1;
                Ok(())
            }
            _ => Err(self.expected_error(")")),
        }
    }

    /// Faltou um comando: `MISSING_STATEMENT` no token corrente, precedido de
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` se ele é uma palavra reservada
    /// (o fasta tenta o comando de expressão e tropeça no identificador).
    pub(crate) fn statement_error(&mut self) -> ParseError {
        if let Kind::Keyword(_) = self.kind() {
            let texto = self.text().to_string();
            let span = self.span();
            self.error_at(codigos::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, span, &[&texto]);
        }
        self.erro(codigos::parser::MISSING_STATEMENT, &[])
    }

    /// Faltou um identificador: `MISSING_IDENTIFIER` no token corrente, ou
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` se ele é uma palavra reservada.
    pub(crate) fn identifier_error(&mut self) -> ParseError {
        if let Kind::Keyword(_) = self.kind() {
            let texto = self.text().to_string();
            return self.erro(codigos::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, &[&texto]);
        }
        self.erro(codigos::parser::MISSING_IDENTIFIER, &[])
    }
    /// Registra que `span` usa o recurso `f`: diagnóstico se a versão da
    /// biblioteca não o liga. A análise continua (o superconjunto é aceito).
    ///
    /// Os argumentos são `[recurso, versão que o 3.6.2 escreve, versão que o
    /// 3.13.4 escreve]`: o molde usa o segundo, e a variante 3.13 do código
    /// troca-o pelo terceiro (`Diagnostic::na_referencia`).
    pub(crate) fn require(&mut self, f: Feature, span: Span) {
        if !self.features.tem(f) {
            if Feature::DESCONHECIDOS_NO_3_6.contains(&f) {
                self.new_syntax = true;
            }
            // `EXPERIMENT_NOT_ENABLED` (com a versão em que o recurso liga) ou
            // `EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT` (experimento sem versão).
            match f.enabled_in() {
                Some(v) => {
                    let nova = format!("{v}.0");
                    let old = f.version_reported_in_3_6().map_or_else(|| nova.clone(), |a| format!("{a}.0"));
                    self.error_at(codigos::parser::EXPERIMENT_NOT_ENABLED, span, &[f.nome(), &old, &nova]);
                }
                None => {
                    self.error_at(codigos::parser::EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT, span, &[f.nome()]);
                }
            }
        }
    }

    /// Como [`Parser::require`], para os recursos que o analyzer confere ao
    /// montar a árvore (`AstBuilder`), não no parser do fasta: a correção
    /// leva a versão como `x.y` (`3.13`), não `x.y.0`. São o corpo `;`, os
    /// membros `this`/`new` e o `final` declarante (medido no oráculo 3.13.4).
    pub(crate) fn require_in_ast(&mut self, f: Feature, span: Span) {
        if self.features.tem(f) {
            return;
        }
        match f.enabled_in() {
            Some(v) => {
                if Feature::DESCONHECIDOS_NO_3_6.contains(&f) {
                    self.new_syntax = true;
                }
                let nova = v.to_string();
                let old = f.version_reported_in_3_6().map_or_else(|| nova.clone(), |a| a.to_string());
                self.error_at(codigos::parser::EXPERIMENT_NOT_ENABLED, span, &[f.nome(), &old, &nova]);
            }
            None => self.require(f, span),
        }
    }

    /// Entra num nível de aninhamento; falha além de [`MAX_DEPTH`].
    pub(crate) fn enter(&mut self) -> PResult<()> {
        if self.depth >= MAX_DEPTH {
            return Err(self.erro(codigos::parser::STACK_OVERFLOW, &[]));
        }
        self.depth += 1;
        Ok(())
    }

    pub(crate) fn leave(&mut self) {
        self.depth -= 1;
    }

    // -- Composição de `>` ---------------------------------------------

    /// Lê, sem consumir, a composição de `>` que começa no token corrente.
    ///
    /// Só compõe tokens **colados** (`glued`), então `a > >b` nunca vira
    /// shift. Devolve `None` se o token corrente não é `>`.
    pub(crate) fn composed_gt(&self) -> Option<ComposedGt> {
        if !self.at_op(Op::Gt) {
            return None;
        }
        let t0 = self.peek();
        let t1 = self.peek_at(1);
        let t2 = self.peek_at(2);
        let t3 = self.peek_at(3);
        let g1 = t0.glued && t1.kind == Kind::Op(Op::Gt);
        let g1_eq = t0.glued && t1.kind == Kind::Op(Op::Assign);
        if !g1 {
            return Some(if g1_eq {
                ComposedGt::GtEq
            } else {
                ComposedGt::Gt
            });
        }
        let g2 = t1.glued && t2.kind == Kind::Op(Op::Gt);
        let g2_eq = t1.glued && t2.kind == Kind::Op(Op::Assign);
        if !g2 {
            return Some(if g2_eq {
                ComposedGt::ShrAssign
            } else {
                ComposedGt::Shr
            });
        }
        let g3_eq = t2.glued && t3.kind == Kind::Op(Op::Assign);
        Some(if g3_eq {
            ComposedGt::UShrAssign
        } else {
            ComposedGt::UShr
        })
    }

    /// Consome a composição devolvida por `Parser::composed_gt`.
    pub(crate) fn eat_composed_gt(&mut self, composed: ComposedGt) {
        for _ in 0..composed.len() {
            self.advance();
        }
    }

    // -- Utilidades de lookahead ----------------------------------------

    /// Índice do token que fecha o delimitador aberto em `open_pos`, ou
    /// `None` se não houver. Considera `( ) [ ] { }` e trata `<`/`>` **não**
    /// como delimitadores. Usado por lookaheads que precisam pular um grupo.
    pub(crate) fn matching_close(&self, open_pos: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut i = open_pos;
        while i < self.tokens.len() {
            match self.tokens[i].kind {
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => depth += 1,
                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                Kind::Eof => return None,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// Texto do token na posição absoluta `pos`.
    pub(crate) fn text_of(&self, pos: usize) -> &'s str {
        let token = self.tokens[pos.min(self.tokens.len() - 1)];
        &self.source[token.span.start..token.span.end]
    }

    /// Kind do token na posição absoluta `pos`.
    pub(crate) fn kind_of(&self, pos: usize) -> Kind {
        self.tokens[pos.min(self.tokens.len() - 1)].kind
    }
}
