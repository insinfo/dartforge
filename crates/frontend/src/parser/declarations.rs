//! Unidade de compilação, diretivas, metadata, declarações de topo, membros
//! de classe, construtores e corpos de função (Dart 3.6, sem resolução).
//!
//! Decisões que não são tradução direta da gramática:
//!
//! * **Recuperação de erro.** Uma declaração de topo que falha sem consumir
//!   nada registra um único diagnóstico e pula um token (o fasta relata um
//!   erro por token: `expected_executable`); com progresso, o cursor é
//!   sincronizado até a próxima fronteira plausível (`;` ou `}` no nível de
//!   aninhamento em que a declaração começou, ou um token que inicia
//!   declaração de topo). A recuperação de membros segue o mesmo desenho,
//!   um membro por vez. Sintaxe 3.7+ em biblioteca ≤3.6 é o superconjunto com
//!   `experiment_not_enabled`, e **não** a cascata do fasta 3.6.2: a
//!   ferramenta é 3.13 (D2), e o analyzer 3.13.4 relata o mesmo na mesma
//!   biblioteca e segue analisando (sondado em 2026-09-25; o oráculo desses
//!   arquivos é o 3.13.4, `corpus/diags/sintaxe-nova.json`). A
//!   exceção é `augment` sem o experimento: o scanner do fasta o faz
//!   identificador comum, e a cascata vale nos dois SDKs. O lookahead de
//!   declaração ainda recusa `]`/`}` como continuação (`int? a]` vira
//!   `expected_token` no tipo, não no nome).
//! * **Construtor × método.** `Nome(` é construtor quando `Nome` é o da
//!   classe corrente (o nome é passado ao parser de membros); `Nome.x(` é
//!   sempre construtor, porque métodos não têm nome pontuado; `factory` e
//!   `const Nome(` idem.
//! * **Função × variável.** Após `kind? nome`, `(` ou `<` abre uma função;
//!   qualquer outra coisa é uma lista de variáveis. `get`/`set` seguidos de
//!   identificador são getter/setter mesmo sem tipo de retorno, porque são
//!   identificadores embutidos e não podem nomear um tipo.
//! * **Metadata.** `@Nome (` só é lista de argumentos se o `(` estiver colado
//!   ao nome (ou houver argumentos de tipo), como faz o parser do SDK — caso
//!   contrário o `(` inicia um tipo record da declaração seguinte.
//! * **Redirecionamento de factory.** Em `= D.x;` o `D.x` é lido primeiro
//!   como tipo (possivelmente `prefix.Tipo`); `.x` só vira nome de construtor
//!   se sobrar após o tipo. Distinguir `prefix.Tipo` de `Tipo.construtor`
//!   exige resolução.
//! * **Ordem das diretivas.** Não é imposta: qualquer diretiva é aceita em
//!   qualquer ponto do nível de topo (superconjunto; a fase seguinte valida).
use super::{ComposedGt, PResult, ParseError, Parser};
use crate::ast::ExprId;
use crate::ast::{
    Annotation, AsyncModifier, ClassDecl, ClassModifiers, Combinator, CompilationUnit,
    Configuration, Constructor, Decl, DeclId, DeclKind, Directive, DirectiveKind, EnumConstant,
    EnumDecl, ExtensionDecl, ExtensionTypeDecl, Function, FunctionBody, FunctionId, FunctionKind,
    Initializer, Member, MemberId, MemberKind, MixinDecl, Name, ParameterKind, RedirectTarget,
    TypeAnnotation, TypeId, TypeKind, TypedefDecl, TypedefKind, Variable, VariableList,
};
use super::modifiers::{ParameterOwner, ModifierTokens};
use crate::features::Feature;
use crate::token::{Keyword, Kind, Op};
use dartforge_diagnostics::{Diagnostic, Span, codigos};

/// Cabeçalho de um construtor primário já lido (Dart 3.13).
struct PrimaryHeader {
    /// Da palavra `class`/`enum` ao fim dos parâmetros.
    span: Span,
    const_: bool,
    /// `.id`; `None` sem nome ou com `.new`.
    nome: Option<Name>,
    params: Vec<crate::ast::Parameter>,
}

/// Que declaração está sendo elaborada.
#[derive(Clone, Copy)]
enum Elaborating {
    /// `class`: `has_supertypes` desliga o tipo sintático de declarante sem
    /// tipo (um getter herdado mandaria).
    Class { has_supertypes: bool },
    /// `enum`: o construtor é sempre constante.
    Enum,
}

/// Modificadores que podem preceder um membro ou uma declaração de topo.
///
/// São lidos em qualquer ordem (superconjunto): a gramática fixa uma ordem,
/// mas rejeitar a ordem errada é papel da fase seguinte.
#[derive(Debug, Default, Clone, Copy)]
struct Modifiers {
    external: bool,
    external_span: Option<Span>,
    static_: bool,
    abstract_: bool,
    abstract_span: Option<Span>,
    covariant: bool,
    late: bool,
    final_: bool,
    const_: bool,
    const_span: Option<Span>,
    var_: bool,
    /// Os tokens dos modificadores, para os diagnósticos do fasta.
    modifier_tokens: ModifierTokens,
}

impl Modifiers {
    /// Algum modificador foi lido?
    fn algum(self) -> bool {
        self.external
            || self.static_
            || self.abstract_
            || self.covariant
            || self.late
            || self.final_
            || self.const_
            || self.var_
    }
}

/// O que começa no cursor do topo ([`Parser::top_level_route`]).
enum TopLevelRoute {
    /// Sem modificadores antes da palavra de topo: o despacho de sempre.
    Plain,
    /// Um membro de topo (função, getter, setter ou variáveis).
    Member,
    /// Uma palavra de topo na posição `word`, com modificadores antes.
    Word { word: usize },
}

/// Resultado de `kind? nome …`: função/método/acessor ou lista de variáveis.
enum FunctionOrVariables {
    Function(FunctionId),
    Variables(VariableList),
}

impl<'s, 'i> Parser<'s, 'i> {
    // -----------------------------------------------------------------------
    // Unidade de compilação
    // -----------------------------------------------------------------------

    /// Lê a unidade inteira, com recuperação nas fronteiras de declaração.
    ///
    /// Nunca falha: cada declaração que não pode ser lida vira um diagnóstico
    /// e a análise continua na próxima fronteira plausível.
    pub(crate) fn parse_compilation_unit(&mut self) -> CompilationUnit {
        let mut unit = CompilationUnit::default();
        if self.kind() == Kind::ScriptTag {
            unit.script_tag = Some(self.advance().span);
            self.directive_state = super::DirectiveState::Script;
        }
        while !self.at_eof() {
            let start_pos = self.pos;
            if self.parse_top_level_item(&mut unit).is_err() {
                self.recover_top_level(start_pos);
                self.record_skipped(start_pos);
            }
        }
        unit
    }

    /// Uma diretiva ou uma declaração de topo, com sua metadata.
    fn parse_top_level_item(&mut self, unit: &mut CompilationUnit) -> PResult<()> {
        let start = self.span();
        let metadata = self.parse_metadata()?;
        let word = self.span();
        if let Some(kind) = self.parse_directive_opt()? {
            self.check_directive_order(&kind, word);
            unit.directives.push(Directive {
                span: self.span_from(start),
                metadata,
                kind,
            });
            return Ok(());
        }
        // `checkDeclaration`: depois de uma declaração, diretiva é erro.
        if self.directive_state != super::DirectiveState::PartOf {
            self.directive_state = super::DirectiveState::Declarations;
        }
        let augment = self.parse_augment_opt();
        // `;` solto no topo é `unexpected_token` no fasta 3.6.2 ("Unexpected
        // text ';'"), não `expected_executable` (que vale para `)`, `]` e
        // `}`). No corpo de classe o `;` continua `expected_class_member`.
        if self.at_op(Op::Semicolon) {
            let span = self.span();
            // Sem consumir: a recuperação sem progresso pula o token, como
            // nos demais erros de token solto (ver `recover_top_level`).
            return Err(self.error_at(codigos::parser::UNEXPECTED_TOKEN, span, &[";"]));
        }
        if !self.can_start_declaration() {
            return Err(self.erro(codigos::parser::EXPECTED_EXECUTABLE, &[]));
        }
        let id = self.parse_top_level_declaration(start, metadata, augment)?;
        unit.declarations.push(id);
        Ok(())
    }

    /// A ordem das diretivas (`DirectiveContext` do fasta,
    /// `directive_context.dart`): `import`/`export` depois de `part` são
    /// `IMPORT_DIRECTIVE_AFTER_PART_DIRECTIVE`/`EXPORT_DIRECTIVE_AFTER_PART_DIRECTIVE`;
    /// qualquer diretiva depois de uma declaração,
    /// `DIRECTIVE_AFTER_DECLARATION`; `library` fora do começo,
    /// `LIBRARY_DIRECTIVE_NOT_FIRST`/`MULTIPLE_LIBRARY_DIRECTIVES`; numa
    /// `part of`, outra diretiva é `NON_PART_OF_DIRECTIVE_IN_PART` (salvo
    /// com `enhanced-parts` ou `macros`) e outro `part of`, `MULTIPLE_PART_OF_DIRECTIVES`.
    /// O erro vai na palavra que abre a diretiva.
    fn check_directive_order(&mut self, kind: &DirectiveKind, word: Span) {
        use super::DirectiveState as E;
        use codigos::parser as c;
        // `macros` traz `enhanced-parts` (o experimento depende dele): o
        // `*.macro.dart` materializado é um `part of` com `import`s.
        let partes = self.features.tem(Feature::EnhancedParts) || self.features.tem(Feature::Macros);
        let estado = self.directive_state;
        let erro = match kind {
            DirectiveKind::Import { .. } | DirectiveKind::Export { .. } => {
                let import = matches!(kind, DirectiveKind::Import { .. });
                match estado {
                    E::NoneFound | E::Script | E::Library | E::ImportExport => {
                        self.directive_state = E::ImportExport;
                        None
                    }
                    E::Part => Some(if import { c::IMPORT_DIRECTIVE_AFTER_PART_DIRECTIVE } else { c::EXPORT_DIRECTIVE_AFTER_PART_DIRECTIVE }),
                    E::PartOf if partes => {
                        self.directive_state = E::ImportExport;
                        None
                    }
                    E::PartOf => Some(c::NON_PART_OF_DIRECTIVE_IN_PART),
                    E::Declarations => Some(c::DIRECTIVE_AFTER_DECLARATION),
                }
            }
            DirectiveKind::Part { .. } => match estado {
                E::NoneFound | E::Script | E::Library | E::ImportExport | E::Part => {
                    self.directive_state = E::Part;
                    None
                }
                E::PartOf if partes => {
                    self.directive_state = E::ImportExport;
                    None
                }
                E::PartOf => Some(c::NON_PART_OF_DIRECTIVE_IN_PART),
                E::Declarations => Some(c::DIRECTIVE_AFTER_DECLARATION),
            },
            DirectiveKind::Library { .. } => {
                if estado < E::Library {
                    self.directive_state = E::Library;
                    None
                } else if estado == E::Library {
                    Some(c::MULTIPLE_LIBRARY_DIRECTIVES)
                } else if estado == E::PartOf {
                    Some(c::NON_PART_OF_DIRECTIVE_IN_PART)
                } else {
                    Some(c::LIBRARY_DIRECTIVE_NOT_FIRST)
                }
            }
            DirectiveKind::PartOf { .. } => {
                if estado == E::NoneFound {
                    self.directive_state = E::PartOf;
                    None
                } else if estado == E::PartOf {
                    Some(c::MULTIPLE_PART_OF_DIRECTIVES)
                } else {
                    Some(c::NON_PART_OF_DIRECTIVE_IN_PART)
                }
            }
            _ => None,
        };
        if let Some(code) = erro {
            self.error_at(code, word, &[]);
        }
    }

    /// O modificador `augment` de uma declaração de topo ou de um membro
    /// (docs/AUGMENTATIONS.md), consumido se presente.
    ///
    /// `augment` é identificador embutido só onde o recurso existe: com
    /// `augmentations` ou `macros` ligados, é modificador quando vem antes de
    /// outra palavra (`augment class`, `augment void f()`, `augment C.x(`).
    /// Sem eles, é sempre um identificador comum, como no scanner do fasta
    /// (`abstract_scanner.dart`: `!_enableAugmentations && keyword ==
    /// Keyword.AUGMENT` vira `tokenizeIdentifier`). Até `augment class C {}`
    /// cai na recuperação de topo (`expected_token` e
    /// `missing_const_final_var_or_type` no `augment`), sem
    /// `experiment_not_enabled`: é o que o analyzer 3.13.4 e o 3.6.2 relatam.
    fn parse_augment_opt(&mut self) -> bool {
        if !self.at_ident("augment") {
            return false;
        }
        let enabled_flag =
            self.features.tem(Feature::Augmentations) || self.features.tem(Feature::Macros);
        if !enabled_flag || !matches!(self.kind_at(1), Kind::Ident | Kind::Keyword(_)) {
            return false;
        }
        self.advance();
        true
    }

    /// O token corrente pode iniciar uma declaração de topo ou um membro?
    ///
    /// Serve para falhar cedo em tokens soltos (`;`, `}`, `)`) sem consultar
    /// os *lookaheads* de tipo.
    fn can_start_declaration(&self) -> bool {
        matches!(
            self.kind(),
            Kind::Ident
                | Kind::Op(Op::LParen)
                | Kind::Keyword(
                    Keyword::Class
                        | Keyword::Enum
                        | Keyword::Void
                        | Keyword::Final
                        | Keyword::Const
                        | Keyword::Var
                )
        )
    }

    /// Profundidade de `()[]{}` acumulada entre duas posições de token.
    fn nesting_between(&self, from: usize, to: usize) -> usize {
        let mut depth = 0usize;
        for token in &self.tokens[from.min(self.tokens.len())..to.min(self.tokens.len())] {
            match token.kind {
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => depth += 1,
                Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        depth
    }

    /// Sincroniza após uma declaração de topo falhar em `start_pos`.
    ///
    /// Sem progresso (nada consumido: token solto), um token basta — o fasta
    /// relata um erro por token (`expected_executable`) e continua no
    /// seguinte; engolir até `;`/`}` esconderia as declarações válidas no meio
    /// do lixo (FN em cascata). Com progresso, pula até fechar o nível em que
    /// a declaração começou (`;` ou `}` com profundidade zero) ou até um token
    /// que inicia declaração de topo. Garante progresso: consome ao menos um
    /// token.
    fn recover_top_level(&mut self, start_pos: usize) {
        if self.pos == start_pos && !self.at_eof() {
            // `parseInvalidTopLevelDeclaration`: um `{` leva o bloco inteiro
            // (`parseInvalidBlock`, até o `endGroup`).
            if self.at_op(Op::LBrace)
                && let Some(closes) = self.matching_close(self.pos)
            {
                let de = self.pos;
                self.pos = closes + 1;
                self.record_skipped(de);
                return;
            }
            self.advance();
            return;
        }
        let mut depth = self.nesting_between(start_pos, self.pos);
        loop {
            match self.kind() {
                Kind::Eof => break,
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => {
                    depth += 1;
                    self.advance();
                }
                Kind::Op(Op::RParen | Op::RBracket) => {
                    // Um fechamento solto no nível zero é, ele próprio, uma
                    // fronteira e é denunciado à parte (`expected_executable`
                    // nele, como faz o fasta): não é consumido aqui — a
                    // próxima volta do topo o relata e a recuperação sem
                    // progresso o pula. Avançar esconderia o erro.
                    if depth == 0 {
                        break;
                    }
                    self.advance();
                    depth -= 1;
                }
                Kind::Op(Op::RBrace) => {
                    depth = depth.saturating_sub(1);
                    self.advance();
                    if depth == 0 {
                        break;
                    }
                }
                Kind::Op(Op::Semicolon) => {
                    self.advance();
                    if depth == 0 {
                        break;
                    }
                }
                _ if depth == 0 && self.pos > start_pos && self.starts_top_level_declaration() => {
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
        if self.pos == start_pos {
            self.advance();
        }
    }

    /// O token corrente é um início plausível de declaração de topo (para a
    /// sincronização de erro)?
    fn starts_top_level_declaration(&self) -> bool {
        match self.kind() {
            Kind::Op(Op::At) => true,
            Kind::Keyword(
                Keyword::Class
                | Keyword::Enum
                | Keyword::Final
                | Keyword::Const
                | Keyword::Var
                | Keyword::Void,
            ) => true,
            Kind::Ident => matches!(
                self.text(),
                "typedef"
                    | "mixin"
                    | "extension"
                    | "abstract"
                    | "import"
                    | "export"
                    | "part"
                    | "library"
                    | "external"
                    | "base"
                    | "interface"
                    | "sealed"
                    | "late"
                    | "static"
            ),
            _ => false,
        }
    }

    /// Sincroniza após um membro falhar em `start_pos`, sem sair do corpo.
    ///
    /// Espelha [`Parser::recover_top_level`]: sem progresso (nada consumido:
    /// token solto), um token basta — o fasta relata um erro por token
    /// (`expected_class_member`) e continua no seguinte; engolir até `;`
    /// esconderia os membros válidos no meio do lixo (`42` antes de
    /// `int x = 1;` apagava o campo). Com progresso, pula até fechar o nível
    /// em que o membro começou (`;` ou `}` com profundidade zero) ou até um
    /// token que inicia membro — onde tenta o resto como membro, de forma
    /// especulativa: se vingar (ex.: a cauda `set foo...` de
    /// `augment static set foo...` sem o recurso), fica e os membros
    /// seguintes sobrevivem; se falhar, os diagnósticos da tentativa são
    /// descartados e a varredura continua no modo antigo, sem nova parada
    /// (um erro pelo membro quebrado, não um por token do resto). A `}` que
    /// fecha a classe **não** é consumida, para que o laço do corpo termine.
    /// Garante progresso: consome ao menos um token.
    fn recover_member(
        &mut self,
        start_pos: usize,
        class_name: Option<&'s str>,
        members: &mut Vec<MemberId>,
    ) {
        if self.pos == start_pos && !self.at_eof() {
            if !self.at_op(Op::RBrace) {
                self.advance();
            }
            return;
        }
        let mut depth = self.nesting_between(start_pos, self.pos);
        let mut speculative = true;
        loop {
            match self.kind() {
                Kind::Eof => break,
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => {
                    depth += 1;
                    self.advance();
                }
                Kind::Op(Op::RParen | Op::RBracket) => {
                    // Um fechamento solto no nível zero é, ele próprio, uma
                    // fronteira e é denunciado à parte (`expected_class_member`
                    // nele, como faz o fasta): não é consumido aqui — a
                    // próxima volta do corpo o relata e a recuperação sem
                    // progresso o pula. Avançar esconderia o erro.
                    if depth == 0 {
                        break;
                    }
                    self.advance();
                    depth -= 1;
                }
                Kind::Op(Op::RBrace) => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    self.advance();
                    if depth == 0 {
                        break;
                    }
                }
                Kind::Op(Op::Semicolon) => {
                    self.advance();
                    if depth == 0 {
                        break;
                    }
                }
                _ if speculative
                    && depth == 0
                    && self.pos > start_pos
                    && self.starts_member() =>
                {
                    let marco = self.diagnostics.len();
                    self.speculating = true;
                    let r = self.parse_member(class_name);
                    self.speculating = false;
                    match r {
                        Ok(id) => {
                            members.push(id);
                            return;
                        }
                        Err(ParseError) => {
                            self.diagnostics.truncate(marco);
                            speculative = false;
                        }
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
        if self.pos == start_pos && !self.at_op(Op::RBrace) && !self.at_eof() {
            self.advance();
        }
    }

    /// O token corrente é um início plausível de membro (para a
    /// sincronização de erro)? Identificadores (nomes, tipos, modificadores
    /// contextuais como `static`/`factory`/`get`), metadata (`@`) e as
    /// palavras que só abrem declaração (`final`/`const`/`var`/`void`;
    /// `this`/`new` abrem membro na 3.13).
    fn starts_member(&self) -> bool {
        match self.kind() {
            Kind::Ident | Kind::Op(Op::At) => true,
            Kind::Keyword(
                Keyword::Final
                | Keyword::Const
                | Keyword::Var
                | Keyword::Void
                | Keyword::This
                | Keyword::New,
            ) => true,
            _ => false,
        }
    }

    // -----------------------------------------------------------------------
    // Diretivas
    // -----------------------------------------------------------------------

    /// O token `n` à frente é um literal de string?
    fn string_at(&self, n: usize) -> bool {
        matches!(self.kind_at(n), Kind::Str(_) | Kind::StrBegin(..))
    }

    /// Lê uma diretiva se o token corrente inicia uma; `None` caso contrário
    /// (sem consumir nada). `library`, `import`, `export` e `part` são
    /// identificadores embutidos, então a decisão olha o token seguinte.
    fn parse_directive_opt(&mut self) -> PResult<Option<DirectiveKind>> {
        // `import augment 'uri';` e `augment library 'uri';`: a forma de
        // biblioteca de augmentation que o SDK 3.6 aceita com
        // `--enable-experiment=macros` (e a que o CFE 3.6.2 gera para a saída
        // das macros).
        if self.at_ident("import") && self.at_ident_at(1, "augment") && self.string_at(2) {
            self.advance();
            let t = self.advance();
            self.require(Feature::Macros, t.span);
            let uri = self.parse_string_literal()?;
            self.ensure_semicolon_forced();
            return Ok(Some(DirectiveKind::ImportAugment { uri }));
        }
        if self.at_ident("augment") && self.at_ident_at(1, "library") && self.string_at(2) {
            let t = self.advance();
            self.require(Feature::Macros, t.span);
            self.advance();
            let uri = self.parse_string_literal()?;
            self.ensure_semicolon_forced();
            return Ok(Some(DirectiveKind::AugmentLibrary { uri }));
        }
        if self.at_ident("library") && (self.at_identifier_at(1) || self.at_op_at(1, Op::Semicolon))
        {
            self.advance();
            let name = if self.at_identifier() {
                self.parse_dotted_name()?
            } else {
                Vec::new()
            };
            self.ensure_semicolon_forced();
            return Ok(Some(DirectiveKind::Library { name }));
        }
        if self.at_ident("import") && self.string_at(1) {
            self.advance();
            let uri = self.parse_string_literal()?;
            let configurations = self.parse_configurations()?;
            let deferred_span = self.at_ident("deferred").then(|| self.span());
            let deferred = self.eat_ident("deferred");
            let prefix = if self.eat_ident("as") {
                Some(self.expect_identifier()?)
            } else {
                None
            };
            // `parseImport`: `deferred` sem `as` é
            // `MISSING_PREFIX_IN_DEFERRED_IMPORT` no `deferred`.
            if let (Some(span), None) = (deferred_span, prefix) {
                self.error_at(codigos::parser::MISSING_PREFIX_IN_DEFERRED_IMPORT, span, &[]);
            }
            let combinators = self.parse_combinators()?;
            self.ensure_semicolon_forced();
            return Ok(Some(DirectiveKind::Import {
                uri,
                configurations,
                deferred,
                prefix,
                combinators,
            }));
        }
        if self.at_ident("export") && self.string_at(1) {
            self.advance();
            let uri = self.parse_string_literal()?;
            let configurations = self.parse_configurations()?;
            let combinators = self.parse_combinators()?;
            self.ensure_semicolon_forced();
            return Ok(Some(DirectiveKind::Export {
                uri,
                configurations,
                combinators,
            }));
        }
        if self.at_ident("part") {
            if self.string_at(1) {
                self.advance();
                let uri = self.parse_string_literal()?;
                self.ensure_semicolon_forced();
                return Ok(Some(DirectiveKind::Part { uri }));
            }
            if self.at_ident_at(1, "of") && (self.string_at(2) || self.at_identifier_at(2)) {
                self.advance();
                self.advance();
                let (uri, name) = if self.string_at(0) {
                    (Some(self.parse_string_literal()?), Vec::new())
                } else {
                    (None, self.parse_dotted_name()?)
                };
                self.ensure_semicolon_forced();
                return Ok(Some(DirectiveKind::PartOf { uri, name }));
            }
        }
        Ok(None)
    }

    /// `a`, `a.b.c` — nome de biblioteca ou teste de configuração.
    fn parse_dotted_name(&mut self) -> PResult<Vec<Name>> {
        let mut names = vec![self.expect_identifier()?];
        while self.at_op(Op::Dot) && self.at_identifier_at(1) {
            self.advance();
            names.push(self.identifier());
        }
        Ok(names)
    }

    /// Zero ou mais `if (dart.library.io == 'x') 'uri'`.
    fn parse_configurations(&mut self) -> PResult<Vec<Configuration>> {
        let mut out = Vec::new();
        while self.at_kw(Keyword::If) {
            let start = self.span();
            self.advance();
            self.expect_op(Op::LParen)?;
            let test = self.parse_dotted_name()?;
            let value = if self.eat_op(Op::EqEq) {
                Some(self.parse_string_literal()?)
            } else {
                None
            };
            self.expect_op(Op::RParen)?;
            let uri = self.parse_string_literal()?;
            out.push(Configuration {
                span: self.span_from(start),
                test,
                value,
                uri,
            });
        }
        Ok(out)
    }

    /// Zero ou mais `show a, b` / `hide c`, em qualquer ordem.
    fn parse_combinators(&mut self) -> PResult<Vec<Combinator>> {
        let mut out = Vec::new();
        loop {
            if self.eat_ident("show") {
                out.push(Combinator::Show(self.parse_identifier_list()?));
            } else if self.eat_ident("hide") {
                out.push(Combinator::Hide(self.parse_identifier_list()?));
            } else {
                return Ok(out);
            }
        }
    }

    /// `a, b, c` — ao menos um identificador.
    fn parse_identifier_list(&mut self) -> PResult<Vec<Name>> {
        let mut names = vec![self.expect_identifier()?];
        while self.eat_op(Op::Comma) {
            names.push(self.expect_identifier()?);
        }
        Ok(names)
    }

    // -----------------------------------------------------------------------
    // Metadata
    // -----------------------------------------------------------------------

    /// Zero ou mais `@anotação`.
    ///
    /// Formas: `@a`, `@p.a`, `@C.ctor(args)`, `@p.C.ctor(args)`, `@C<T>(args)`,
    /// `@p.C<T>.ctor(args)`. Os argumentos só são lidos se o `(` estiver
    /// colado ao nome (ou se houver argumentos de tipo), para não engolir um
    /// tipo record que inicie a declaração seguinte.
    pub(crate) fn parse_metadata(&mut self) -> PResult<Vec<Annotation>> {
        let mut out = Vec::new();
        while self.at_op(Op::At) {
            let start = self.span();
            self.advance();
            let mut name = vec![self.expect_identifier()?];
            if self.at_op(Op::Dot) && self.at_identifier_at(1) {
                self.advance();
                name.push(self.identifier());
            }
            let with_types = self.at_op(Op::Lt);
            // `endMetadata` (`ast_builder.dart:2512-2518`): argumentos de tipo
            // sem `generic-metadata`, no `<`.
            if with_types {
                let less = self.span();
                self.require(Feature::GenericMetadata, less);
            }
            let type_args = if with_types {
                self.parse_type_arguments_opt()?
            } else {
                Vec::new()
            };
            if self.at_op(Op::Dot) && (self.at_identifier_at(1) || self.at_kw_at(1, Keyword::New)) {
                self.advance();
                let part = self.identifier_or_new()?;
                name.push(part);
            }
            // `parseMetadata` (`parser_impl.dart:1346-1349`): com argumentos de
            // tipo, o que segue tem de ser `(`; o erro vai no último token
            // lido da anotação (o `>` ou o `.nome`).
            if with_types && !self.at_op(Op::LParen) {
                let ultimo = self.tokens[self.pos - 1].span;
                self.error_at(codigos::parser::ANNOTATION_WITH_TYPE_ARGUMENTS_UNINSTANTIATED, ultimo, &[]);
            }
            // `parseArgumentsOptMetadata` (`:7718-7752`): `(` colado são os
            // argumentos; separado, só com argumentos de tipo ou quando o que
            // segue o `)` é `class`/`enum`, com
            // `ANNOTATION_SPACE_BEFORE_PARENTHESIS` no `(`.
            let glued = self.pos > 0 && self.tokens[self.pos - 1].glued;
            let arguments = if self.at_op(Op::LParen) {
                let after_closer = self.matching_close(self.pos).map(|f| f + 1 - self.pos);
                let class_or_enum = after_closer.is_some_and(|n| self.at_kw_at(n, Keyword::Class) || self.at_kw_at(n, Keyword::Enum));
                if glued {
                    Some(self.parse_arguments()?)
                } else if with_types || class_or_enum {
                    let parens = self.span();
                    self.error_at(codigos::parser::ANNOTATION_SPACE_BEFORE_PARENTHESIS, parens, &[]);
                    Some(self.parse_arguments()?)
                } else {
                    None
                }
            } else {
                None
            };
            out.push(Annotation {
                span: self.span_from(start),
                name,
                type_args,
                arguments,
            });
        }
        Ok(out)
    }

    /// `identifierOrNew`: um identificador ou a palavra `new` (nome de
    /// construtor `C.new`).
    fn identifier_or_new(&mut self) -> PResult<Name> {
        if self.at_kw(Keyword::New) {
            let token = self.advance();
            Ok(self.name_from("new", token.span))
        } else {
            self.expect_identifier()
        }
    }

    // -----------------------------------------------------------------------
    // Declarações de topo
    // -----------------------------------------------------------------------

    /// Despacha a declaração de topo que começa no token corrente.
    fn parse_top_level_declaration(
        &mut self,
        start: Span,
        metadata: Vec<Annotation>,
        augment: bool,
    ) -> PResult<DeclId> {
        let mut pre = ClassModifiers::default();
        let member = match self.top_level_route() {
            TopLevelRoute::Member => true,
            TopLevelRoute::Plain => false,
            TopLevelRoute::Word { word } => {
                self.modifiers_before_word(word, &mut pre);
                false
            }
        };
        self.mixin_and_enum_prefixes();
        let kind = if member {
            let fstart = self.span();
            let mods = self.parse_modifiers(true);
            match self.parse_function_or_variables(mods, fstart, mods.external_span, true)? {
                FunctionOrVariables::Function(id) => DeclKind::Function(id),
                FunctionOrVariables::Variables(list) => DeclKind::Variables(list),
            }
        } else if self.class_follows() {
            DeclKind::Class(self.parse_class(pre)?)
        } else if self.mixin_follows() {
            DeclKind::Mixin(self.parse_mixin()?)
        } else if self.at_kw(Keyword::Enum) {
            DeclKind::Enum(self.parse_enum()?)
        } else if self.at_ident("typedef")
            && (self.at_identifier_at(1) || self.at_kw_at(1, Keyword::Void))
        {
            DeclKind::Typedef(self.parse_typedef()?)
        } else if self.at_ident("extension")
            && (self.at_identifier_at(1) || self.at_op_at(1, Op::Lt))
        {
            self.parse_extension_or_extension_type()?
        } else {
            let fstart = self.span();
            let mods = self.parse_modifiers(true);
            match self.parse_function_or_variables(mods, fstart, mods.external_span, true)? {
                FunctionOrVariables::Function(id) => DeclKind::Function(id),
                FunctionOrVariables::Variables(list) => DeclKind::Variables(list),
            }
        };
        Ok(self.ast.push_decl(Decl {
            span: self.span_from(start),
            metadata: metadata.into_boxed_slice(),
            kind,
            augment,
        }))
    }

    /// A decisão de `parseTopLevelDeclarationImpl` (`parser_impl.dart:528`)
    /// sobre o que começa no cursor: uma palavra de topo (`class`, `enum`,
    /// `mixin`…) precedida de modificadores (`isModifier` do token, sem olhar
    /// o seguinte) e de no máximo um `macro`/`sealed`/`base`/`interface`; ou
    /// um membro de topo — sempre que começa por `var`, `late`, `final` (não
    /// antes de `class`/`mixin`/`enum`) ou `const` (não antes de `class`), e
    /// quando os modificadores não levam a uma palavra de topo
    /// (`final abstract class` é um campo `final` mal formado).
    fn top_level_route(&self) -> TopLevelRoute {
        let p0 = self.pos;
        let word = |p: usize| match self.kind_of(p) {
            Kind::Ident | Kind::Keyword(_) => self.text_of(p),
            _ => "",
        };
        let mut j = p0;
        if super::fasta::is_modifier(word(p0)) && word(p0) != "augment" {
            let following = word(p0 + 1);
            match word(p0) {
                "var" | "late" => return TopLevelRoute::Member,
                "final" if !matches!(following, "class" | "mixin" | "enum") => return TopLevelRoute::Member,
                "const" if following != "class" => return TopLevelRoute::Member,
                _ => {}
            }
            while super::fasta::is_modifier(word(j)) && word(j) != "augment" {
                j += 1;
            }
        }
        let mut k = j;
        match word(j) {
            "macro" if word(j + 1) == "class" => k = j + 1,
            "sealed" | "base" | "interface" if matches!(word(j + 1), "class" | "mixin" | "enum") => k = j + 1,
            "sealed" if word(j + 1) == "abstract" && word(j + 2) == "class" => k = j + 2,
            _ => {}
        }
        if super::fasta::is_top_level_word(word(k)) {
            if j == p0 {
                return TopLevelRoute::Plain;
            }
            return TopLevelRoute::Word { word: k };
        }
        // Modificadores seguidos de nome, ou `sealed`/`base`/… fora da ordem
        // (`base abstract class`, `sealed sealed class`): membro.
        if j > p0 || (k == j && self.class_follows()) {
            return TopLevelRoute::Member;
        }
        TopLevelRoute::Plain
    }

    /// Os modificadores de membro antes de uma palavra de topo, lidos pelo
    /// `ModifierContext` (`parseClassModifiers`, `parseEnumModifiers`,
    /// `parseMixinModifiers`…): o que não cabe ali é relatado e
    /// descartado; `abstract`/`final` de classe vão para `pre`.
    fn modifiers_before_word(&mut self, word: usize, pre: &mut ClassModifiers) {
        let mut f = ModifierTokens::default();
        self.modifier_context(&mut f, false);
        // `mixin class` é classe (`_handleModifiersForClassDeclaration`).
        let mut texto = self.text_of(word);
        if texto == "mixin" && self.text_of(word + 1) == "class" {
            texto = "class";
        }
        self.report_modifiers_before_top_level(&f, texto);
        match texto {
            "class" => {
                pre.abstract_ = f.abstract_.is_some();
                pre.final_ = f.final_.is_some();
                if f.final_.is_some() && self.text_of(word) == "mixin" {
                    self.error_at_token(f.final_.unwrap_or_default(), codigos::parser::FINAL_MIXIN_CLASS);
                }
            }
            "mixin" => {
                if let Some(fi) = f.final_ {
                    self.error_at_token(fi, codigos::parser::FINAL_MIXIN);
                }
            }
            "enum" => {
                if let Some(fi) = f.final_ {
                    self.error_at_token(fi, codigos::parser::FINAL_ENUM);
                }
            }
            _ => {}
        }
    }

    /// `sealed`/`base`/`interface` antes de `mixin`, `mixin class` ou
    /// `enum` (`parseTopLevelKeywordDeclaration`): só `base` cabe em mixin,
    /// nenhum em enum; o que não cabe é relatado (`SEALED_MIXIN`,
    /// `INTERFACE_MIXIN_CLASS`, `BASE_ENUM`…) e descartado. As formas
    /// válidas (`base mixin`, `sealed class`…) seguem para o despacho comum.
    fn mixin_and_enum_prefixes(&mut self) {
        if self.kind() != Kind::Ident || !matches!(self.text(), "sealed" | "base" | "interface") {
            return;
        }
        let prefix = self.pos;
        let texto = self.text();
        let following = match self.kind_at(1) {
            Kind::Ident | Kind::Keyword(_) => self.text_at(1),
            _ => return,
        };
        let code = match (following, texto) {
            ("enum", "sealed") => codigos::parser::SEALED_ENUM,
            ("enum", "base") => codigos::parser::BASE_ENUM,
            ("enum", _) => codigos::parser::INTERFACE_ENUM,
            ("mixin", _) if self.text_at(2) == "class" && self.kind_at(2) == Kind::Keyword(Keyword::Class) => match texto {
                "sealed" => codigos::parser::SEALED_MIXIN_CLASS,
                "interface" => codigos::parser::INTERFACE_MIXIN_CLASS,
                _ => return,
            },
            ("mixin", "sealed") => codigos::parser::SEALED_MIXIN,
            ("mixin", "interface") => codigos::parser::INTERFACE_MIXIN,
            _ => return,
        };
        self.error_at_token(prefix, code);
        self.advance();
    }

    /// `modifiers_* class` começa aqui?
    fn class_follows(&self) -> bool {
        let mut i = self.pos;
        loop {
            match self.kind_of(i) {
                Kind::Keyword(Keyword::Final) => i += 1,
                // `mixin` só abre classe colado a `class` (`mixin base class`
                // é um mixin chamado `base`, como no fasta).
                Kind::Ident if self.text_of(i) == "mixin" => {
                    return self.kind_of(i + 1) == Kind::Keyword(Keyword::Class);
                }
                Kind::Ident
                    if matches!(
                        self.text_of(i),
                        "abstract" | "base" | "interface" | "sealed" | "macro"
                    ) =>
                {
                    i += 1
                }
                Kind::Keyword(Keyword::Class) => return true,
                _ => return false,
            }
        }
    }

    /// `base? mixin Nome` começa aqui?
    fn mixin_follows(&self) -> bool {
        // `parseTopLevelKeywordDeclaration`: depois de `mixin`, só `(`, `.` e
        // `<` fazem dele um nome comum (membro de topo); o resto abre um
        // mixin, mesmo que o nome falte (`mixin final M {}`).
        let abre = |i: usize| !matches!(self.kind_of(i), Kind::Op(Op::LParen | Op::Dot | Op::Lt) | Kind::Eof);
        (self.at_ident("mixin") && abre(self.pos + 1))
            || (self.at_ident("base") && self.at_ident_at(1, "mixin") && abre(self.pos + 2))
    }

    /// `AstBuilder.beginClassDeclaration`/`beginNamedMixinApplication`
    /// (3.6.2, `ast_builder.dart:261-300`, `:525-570`): sem `sealed-class`,
    /// o `sealed`; sem `class-modifiers`, o `base`, o `interface`, o `final`
    /// e o `mixin` são `EXPERIMENT_NOT_ENABLED` no token, e o modificador é
    /// esquecido. Os tokens são os que precedem o `class` corrente.
    fn class_modifiers_without_feature(&mut self, m: &mut ClassModifiers) {
        let sealed = self.features.tem(Feature::SealedClass);
        let modifiers_ = self.features.tem(Feature::ClassModifiers);
        if sealed && modifiers_ {
            return;
        }
        let mut found_items: Vec<(String, Span)> = Vec::new();
        let mut i = self.pos.saturating_sub(1);
        while i > 0 {
            i -= 1;
            let tk = &self.tokens[i];
            let texto = &self.source[tk.span.start..tk.span.end];
            // O nome de uma anotação (`@sealed class C`) não é modificador.
            let from_annotation = i > 0 && &self.source[self.tokens[i - 1].span.start..self.tokens[i - 1].span.end] == "@";
            match texto {
                "abstract" | "base" | "interface" | "final" | "sealed" | "mixin" | "macro" | "augment" if !from_annotation => {
                    found_items.push((texto.to_string(), tk.span));
                }
                _ => break,
            }
        }
        found_items.reverse();
        for (texto, sp) in found_items {
            match texto.as_str() {
                "sealed" if !sealed && m.sealed => {
                    self.require(Feature::SealedClass, sp);
                    m.sealed = false;
                }
                "base" if !modifiers_ && m.base => {
                    self.require(Feature::ClassModifiers, sp);
                    m.base = false;
                }
                "interface" if !modifiers_ && m.interface => {
                    self.require(Feature::ClassModifiers, sp);
                    m.interface = false;
                }
                "final" if !modifiers_ && m.final_ => {
                    self.require(Feature::ClassModifiers, sp);
                    m.final_ = false;
                }
                "mixin" if !modifiers_ && m.mixin => {
                    self.require(Feature::ClassModifiers, sp);
                    m.mixin = false;
                }
                _ => {}
            }
        }
    }

    /// `classDeclaration`, inclusive a forma `class C = S with M;`.
    fn parse_class(&mut self, pre: ClassModifiers) -> PResult<ClassDecl> {
        let mut modifiers = pre;
        let mut sealed = None;
        loop {
            if self.at_ident("sealed") {
                sealed = Some(self.span());
            }
            if self.eat_kw(Keyword::Final) {
                modifiers.final_ = true;
            } else if self.eat_ident("abstract") {
                modifiers.abstract_ = true;
            } else if self.eat_ident("base") {
                modifiers.base = true;
            } else if self.eat_ident("interface") {
                modifiers.interface = true;
            } else if self.eat_ident("sealed") {
                modifiers.sealed = true;
            } else if self.eat_ident("mixin") {
                modifiers.mixin = true;
            } else if self.at_ident("macro") {
                let t = self.advance();
                self.require(Feature::Macros, t.span);
                modifiers.macro_ = true;
            } else {
                break;
            }
        }
        let class_token = self.expect_kw(Keyword::Class)?;
        self.class_modifiers_without_feature(&mut modifiers);
        // `parseClassOrNamedMixinApplication`: `abstract` com `sealed` é
        // `ABSTRACT_SEALED_CLASS` no `sealed` (em qualquer ordem).
        if modifiers.abstract_
            && let Some(span) = sealed
        {
            self.error_at(codigos::parser::ABSTRACT_SEALED_CLASS, span, &[]);
        }
        // `class const K(...)`: construtor primário constante (3.13).
        let primary_const = self.at_kw(Keyword::Const).then(|| self.advance().span);
        let (name_text, name) = self.declaration_name()?;
        let type_params = self.parse_type_parameters_opt_variance()?;
        if self.at_op(Op::Assign) {
            // `class const C = S with M;`: o `const` é
            // `CONST_WITHOUT_PRIMARY_CONSTRUCTOR`, com ou sem o recurso
            // (`parseClassOrNamedMixinApplication`, `parser_impl.dart:2983-2986`).
            if let Some(c) = primary_const {
                self.error_at(codigos::parser::CONST_WITHOUT_PRIMARY_CONSTRUCTOR, c, &[]);
            }
            self.advance();
            let extends = Some(self.parse_type()?);
            self.expect_kw(Keyword::With)?;
            let with = self.parse_type_list()?;
            let implements = self.parse_implements_opt()?;
            self.ensure_semicolon()?;
            return Ok(ClassDecl {
                modifiers,
                name,
                type_params: type_params.into_boxed_slice(),
                extends,
                with: with.into_boxed_slice(),
                implements: implements.into_boxed_slice(),
                mixin_application: true,
                members: Vec::new(),
                primary_constructor: None,
            });
        }
        let primary_ = self.parse_primary_header_opt(class_token.span, primary_const)?;
        let extends = if self.eat_kw(Keyword::Extends) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let with = if self.eat_kw(Keyword::With) {
            self.parse_type_list()?
        } else {
            Vec::new()
        };
        let implements = self.parse_implements_opt()?;
        // `parseClassHeaderOpt`: `native 'nome'?` depois das cláusulas (o
        // analyzer aceita a forma antiga; fora do SDK o `ErrorVerifier` dá
        // `NATIVE_CLAUSE_IN_NON_SDK_CODE`, em `analise::nativos`).
        if self.eat_ident("native") && self.string_at(0) {
            self.parse_string_literal()?;
        }
        self.owner = super::MemberOwner::Class;
        let mut members = self.parse_class_body_or_empty(Some(name_text))?;
        let has_supertypes = extends.is_some() || !with.is_empty() || !implements.is_empty();
        let primary_constructor = self.elaborate_primary_constructor(
            name,
            primary_,
            &mut members,
            Elaborating::Class { has_supertypes },
        );
        Ok(ClassDecl {
            modifiers,
            name,
            type_params: type_params.into_boxed_slice(),
            extends,
            with: with.into_boxed_slice(),
            implements: implements.into_boxed_slice(),
            mixin_application: false,
            members,
            primary_constructor,
        })
    }

    /// `(.id)? (params)` depois do nome e dos parâmetros de tipo de uma
    /// classe ou enum: o cabeçalho de um construtor primário (Dart 3.13,
    /// `<primaryConstructor>`). `var`/`final` nos parâmetros declaram campo.
    fn parse_primary_header_opt(
        &mut self,
        start_offset: Span,
        const_: Option<Span>,
    ) -> PResult<Option<PrimaryHeader>> {
        if !self.at_op(Op::LParen) && !(self.at_op(Op::Dot) && self.kind_at(1) != Kind::Op(Op::Dot))
        {
            // `parsePrimaryConstructorOpt` sem cabeçalho (3.13.4,
            // `parser_impl.dart:3829-3862`): o `const` é
            // `CONST_WITHOUT_PRIMARY_CONSTRUCTOR` com o recurso ligado e
            // `UNEXPECTED_TOKEN` sem ele, e a declaração segue.
            if let Some(c) = const_ {
                if self.features.tem(Feature::PrimaryConstructors) {
                    self.error_at(codigos::parser::CONST_WITHOUT_PRIMARY_CONSTRUCTOR, c, &[]);
                } else {
                    self.error_at(codigos::parser::UNEXPECTED_TOKEN, c, &["const"]);
                }
            }
            return Ok(None);
        }
        let beginning = self.span();
        let nome = if self.eat_op(Op::Dot) {
            let n = self.identifier_or_new()?;
            (self.interner.resolve(n.sym) != "new").then_some(n)
        } else {
            None
        };
        let salvo = self.in_primary_constructor;
        self.in_primary_constructor = true;
        self.params_owner = ParameterOwner::PrimaryConstructor;
        let params = self.parse_formal_parameters();
        self.in_primary_constructor = salvo;
        let params = params?;
        let span = self.span_from(beginning);
        // O analyzer 3.13.4 relata o recurso desligado no primeiro token do
        // cabeçalho (`(`, ou o `.` de `C.nome(`), não no cabeçalho inteiro.
        self.require(Feature::PrimaryConstructors, beginning);
        Ok(Some(PrimaryHeader {
            span: Span {
                start: start_offset.start,
                end: span.end,
            },
            const_: const_.is_some(),
            nome,
            params,
        }))
    }

    /// `{ members_ }` ou, a partir da 3.13, `;` (corpo vazio).
    fn parse_class_body_or_empty(&mut self, class_name: Option<&'s str>) -> PResult<Vec<MemberId>> {
        let synthetic = self.synthetic_name.take();
        if self.at_op(Op::Semicolon) {
            let t = self.advance();
            self.require_in_ast(Feature::PrimaryConstructors, t.span);
            return Ok(Vec::new());
        }
        // `ensureBlock` (`parser_impl.dart:4204`) com o `BlockKind` da
        // declaração: sem `{`, `EXPECTED_CLASS_BODY`/`EXPECTED_MIXIN_BODY`/…
        // no último token lido (no lugar do nome sintético, se houver) e um
        // corpo vazio sintético; o resto é lido como declarações seguintes.
        if !self.at_op(Op::LBrace) && self.pos > 0 {
            let code = match self.owner {
                super::MemberOwner::Class => codigos::parser::EXPECTED_CLASS_BODY,
                super::MemberOwner::Mixin => codigos::parser::EXPECTED_MIXIN_BODY,
                super::MemberOwner::Extension => codigos::parser::EXPECTED_EXTENSION_BODY,
                super::MemberOwner::ExtensionType => codigos::parser::EXPECTED_EXTENSION_TYPE_BODY,
                super::MemberOwner::Enum => return self.parse_class_body(class_name),
            };
            let span = synthetic.unwrap_or(self.tokens[self.pos - 1].span);
            self.error_at(code, span, &[]);
            return Ok(Vec::new());
        }
        self.parse_class_body(class_name)
    }

    /// O nome de uma classe ou mixin (`ClassOrMixinOrExtensionIdentifierContext`,
    /// `identifier_context_impl.dart`): uma palavra que abre a declaração
    /// seguinte (`mixin mixin class C`, `mixin final M`) ou que só caberia
    /// depois do nome não é o nome — `MISSING_IDENTIFIER` nela e um nome
    /// sintético vazio, sem consumir; outra palavra reservada é
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` e vira o nome.
    fn declaration_name(&mut self) -> PResult<(&'s str, Name)> {
        let segue = |p: &Self, i: usize| {
            matches!(p.kind_of(i), Kind::Op(Op::Lt | Op::LBrace | Op::Assign | Op::LParen | Op::Dot) | Kind::Eof)
                || matches!(p.kind_of(i), Kind::Keyword(Keyword::Extends | Keyword::With))
                || (p.kind_of(i) == Kind::Ident && matches!(p.text_of(i), "implements" | "on"))
        };
        let pos = self.pos;
        let pseudo = self.kind() == Kind::Ident
            && matches!(super::fasta::word_style(self.text()), None | Some(super::fasta::WordStyle::Pseudo));
        if !pseudo {
            let recover = self.at_eof()
                || (self.looks_like_top_level_start(pos) && !segue(self, pos + 1))
                || (segue(self, pos) && !segue(self, pos + 1));
            if recover {
                let here = self.span();
                self.error_at(codigos::parser::MISSING_IDENTIFIER, here, &[]);
                self.synthetic_name = Some(here);
                let nome = self.name_from("", Span { start: here.start, end: here.start });
                return Ok(("", nome));
            }
        }
        let texto = self.text();
        Ok((texto, self.expect_identifier()?))
    }

    /// Derivação D → D2 do construtor primário (spec 3.13, `:926-1025`),
    /// feita na árvore, logo depois do parse da declaração: cada parâmetro
    /// declarante `var`/`final T p` vira um campo `T p;`/`final T p;` e o
    /// parâmetro vira `T this.p` (o `covariant` passa ao campo); o construtor
    /// `k2` recebe o nome, o `const` do cabeçalho (ou do `enum`), a lista de
    /// inicialização e o corpo da parte `this`. Os consumidores veem código
    /// comum. Devolve o `k2`, que fica no lugar da parte `this` (ou depois dos
    /// campos, se não houver).
    fn elaborate_primary_constructor(
        &mut self,
        class: Name,
        cab: Option<PrimaryHeader>,
        members: &mut Vec<MemberId>,
        where_: Elaborating,
    ) -> Option<MemberId> {
        // Partes `this` no corpo.
        let partes: Vec<usize> = members
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(&self.ast.member(**m).kind, MemberKind::Constructor(c) if c.parte_primaria))
            .map(|(i, _)| i)
            .collect();
        // `_checkForMultiplePrimaryConstructorBodyDeclarations` (3.13.4,
        // `error_verifier.dart:6293-6305`): da segunda parte em diante, com ou
        // sem o cabeçalho primário.
        for &i in partes.iter().skip(1) {
            let span = self.this_span(members[i]);
            self.error_at(codigos::compile_time_error::MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS, span, &[]);
        }
        let Some(cab) = cab else {
            for &i in &partes {
                let span = self.this_span(members[i]);
                self.error_at(codigos::compile_time_error::PRIMARY_CONSTRUCTOR_BODY_WITHOUT_DECLARATION, span, &[]);
            }
            return None;
        };
        // Construtor generativo não redirecionador no corpo: proibido (o k2 é
        // o único, para os inicializadores de campo poderem ler os
        // parâmetros); o nome do k2 não pode repetir o de outro construtor.
        for &m in members.iter() {
            if let MemberKind::Constructor(c) = &self.ast.member(m).kind {
                if c.parte_primaria {
                    continue;
                }
                let redirects = c
                    .initializers
                    .iter()
                    .any(|i| matches!(i, Initializer::Redirect { .. }));
                if !c.factory && !redirects {
                    // No nome do construtor (`C` ou `C.nome`), como o analyzer.
                    let end_offset = c.name.map_or(c.class_name.span.end, |n| n.span.end);
                    let span = Span { start: c.class_name.span.start, end: end_offset };
                    self.diagnostics.push(Diagnostic::com_codigo(
                        codigos::compile_time_error::NON_REDIRECTING_GENERATIVE_CONSTRUCTOR_WITH_PRIMARY,
                        span,
                        Vec::<&str>::new(),
                    ));
                }
                if c.name.map(|n| n.sym) == cab.nome.map(|n| n.sym) {
                    // O construtor do corpo repete o nome do primário: o
                    // `duplicate_constructor` do analyzer, no nome do segundo.
                    let end_offset = c.name.map_or(c.class_name.span.end, |n| n.span.end);
                    let span = Span { start: c.class_name.span.start, end: end_offset };
                    let d = match c.name {
                        None => Diagnostic::com_codigo(
                            codigos::compile_time_error::DUPLICATE_CONSTRUCTOR_DEFAULT,
                            span,
                            Vec::<&str>::new(),
                        ),
                        Some(n) => Diagnostic::com_codigo(
                            codigos::compile_time_error::DUPLICATE_CONSTRUCTOR_NAME,
                            span,
                            [&self.source[n.span.start..n.span.end]],
                        ),
                    };
                    self.diagnostics.push(d);
                }
            }
        }
        let (has_supertypes, const_) = match where_ {
            Elaborating::Class { has_supertypes } => (has_supertypes, cab.const_),
            // Construtor de enum é sempre constante.
            Elaborating::Enum => (true, true),
        };
        let mut fields_: Vec<MemberId> = Vec::new();
        let mut parameters_ = cab.params;
        for p in parameters_.iter_mut() {
            // O `covariant` sobrando é do `parseFormalParameterModifiers`
            // (`report_parameter_modifiers`, `PrimaryConstructor`).
            if !(p.var_ || p.final_) || p.this_ || p.super_ {
                continue;
            }
            let Some(nome) = p.name else { continue };
            // O declarante no estilo antigo de parâmetro função
            // (`final int _f()`): o campo tem o tipo função inteiro
            // (`int Function()`), não só o retorno. Os parâmetros do tipo são
            // cópias rasas (nome, tipo, espécie): um tipo função não tem
            // metadados nem valores padrão; o parâmetro do construtor fica
            // como está.
            let ty = if let Some(fps) = &p.function_parameters {
                let parameters_: Vec<crate::ast::Parameter> = fps
                    .iter()
                    .map(|q| crate::ast::Parameter {
                        span: q.span,
                        metadata: Vec::new().into_boxed_slice(),
                        kind: q.kind,
                        required: q.required,
                        covariant: q.covariant,
                        final_: q.final_,
                        var_: q.var_,
                        const_: q.const_,
                        ty: q.ty,
                        this_: false,
                        super_: false,
                        name: q.name,
                        function_type_params: Vec::new().into_boxed_slice(),
                        function_parameters: None,
                        function_nullable: false,
                        default_value: None,
                        public_name: q.public_name,
                        declarante: false,
                    })
                    .collect();
                let types_: Vec<crate::ast::TypeParameter> = p
                    .function_type_params
                    .iter()
                    .map(|t| crate::ast::TypeParameter { span: t.span, metadata: Vec::new().into_boxed_slice(), name: t.name, bound: t.bound, variance: t.variance })
                    .collect();
                Some(self.ast.push_type(TypeAnnotation {
                    span: p.span,
                    nullable: p.function_nullable,
                    kind: TypeKind::Function { return_type: p.ty, type_params: types_.into_boxed_slice(), parameters: parameters_.into_boxed_slice() },
                }))
            } else {
                p.ty.or_else(|| self.untyped_declarant_type(p.default_value, has_supertypes, p.span))
            };
            let campo = Member {
                span: p.span,
                metadata: Vec::new().into_boxed_slice(),
                kind: MemberKind::Field(VariableList {
                    external: false,
                    static_: false,
                    abstract_: false,
                    covariant: p.covariant,
                    late: false,
                    final_: p.final_,
                    const_: false,
                    var_: p.var_ && ty.is_none(),
                    ty,
                    variables: vec![Variable {
                        name: nome,
                        initializer: None,
                    }]
                    .into_boxed_slice(),
                }),
                augment: false,
            };
            fields_.push(self.ast.push_member(campo));
            // `var T p` → `T this.p` (com o tipo, se escrito).
            p.this_ = true;
            p.declarante = true;
            p.var_ = false;
            p.final_ = false;
            p.covariant = false;
        }
        // Lista de inicialização e corpo da parte `this`.
        let (initializers, body, part_span, body_part) = match partes.first() {
            Some(&i) => {
                let mid = members[i];
                let span = self.ast.member(mid).span;
                let MemberKind::Constructor(c) = &mut self.ast.members[mid.0 as usize].kind else {
                    unreachable!()
                };
                let inits = std::mem::take(&mut c.initializers);
                let body = std::mem::replace(&mut c.body, FunctionBody::Empty);
                let body_node = self.primary_bodies.get(&c.class_name.span.start).copied();
                // `this : this.nome()`: o primário não redireciona
                // (`PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT` no `this` do
                // redirecionamento). O inicializador fica (conta para o ciclo
                // constante e para o uso do alvo); o ciclo gerador o ignora.
                for i in inits.iter() {
                    if let Initializer::Redirect { span: sp, .. } = i {
                        let s = Span { start: sp.start, end: sp.start + 4 };
                        self.diagnostics.push(Diagnostic::com_codigo(codigos::compile_time_error::PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT, s, Vec::<&str>::new()));
                    }
                }
                (inits, body, Some(span), body_node)
            }
            None => (Vec::new().into_boxed_slice(), FunctionBody::Empty, None, None),
        };
        // Corpo da parte `this`, no `{`/`=>` que o abre (registrado no parse):
        // `const` não aceita corpo nenhum; fora dele, `=>` é o erro.
        if let Some(span) = body_part {
            let code = match (&body, const_) {
                (FunctionBody::Block(_), true) => Some(codigos::parser::CONST_PRIMARY_CONSTRUCTOR_WITH_BLOCK_BODY),
                (FunctionBody::Expression(_), true) => Some(codigos::parser::CONST_PRIMARY_CONSTRUCTOR_WITH_EXPRESSION_BODY),
                (FunctionBody::Expression(_), false) => {
                    Some(codigos::compile_time_error::PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY)
                }
                _ => None,
            };
            if let Some(code) = code {
                self.error_at(code, span, &[]);
            }
        }
        let k2 = Member {
            span: part_span.unwrap_or(cab.span),
            metadata: Vec::new().into_boxed_slice(),
            kind: MemberKind::Constructor(Constructor {
                external: false,
                const_,
                factory: false,
                class_name: class,
                name: cab.nome,
                parameters: parameters_.into_boxed_slice(),
                initializers,
                redirect: None,
                body,
                parte_primaria: false,
            }),
            augment: false,
        };
        let k2 = match partes.first() {
            Some(&i) => {
                // O k2 fica no lugar da parte `this`.
                let mid = members[i];
                self.ast.members[mid.0 as usize] = k2;
                mid
            }
            None => self.ast.push_member(k2),
        };
        // Campos primeiro (na ordem dos parâmetros), depois o corpo; o k2
        // entra depois dos campos quando não havia parte `this`.
        let mut novos = fields_;
        if partes.is_empty() {
            novos.push(k2);
        }
        novos.append(members);
        *members = novos;
        Some(k2)
    }

    /// Tipo de um parâmetro declarante sem tipo (spec 3.13, `:945-962`): o
    /// do getter herdado de mesmo nome; senão o do valor padrão; senão
    /// `Object?`. Aqui, sintaticamente: um default literal dá o tipo dele,
    /// `null` ou nenhum default dá `Object?` — mas só quando a classe não
    /// tem supertipo declarado, porque aí um getter herdado poderia mandar
    /// (sem tipo, o campo fica para a inferência do `types`).
    fn untyped_declarant_type(
        &mut self,
        padrao: Option<ExprId>,
        has_supertypes: bool,
        span: Span,
    ) -> Option<TypeId> {
        use crate::ast::{ExprKind, TypeAnnotation, TypeKind};
        if has_supertypes {
            return None;
        }
        let (nome, is_nullable) = match padrao.map(|e| &self.ast.expr(e).kind) {
            None | Some(ExprKind::Null) => ("Object", true),
            Some(ExprKind::Int(_)) => ("int", false),
            Some(ExprKind::Double(_)) => ("double", false),
            Some(ExprKind::String(_)) => ("String", false),
            Some(ExprKind::Bool(_)) => ("bool", false),
            Some(_) => return None,
        };
        let n = self.name_from(nome, span);
        Some(self.ast.push_type(TypeAnnotation {
            span,
            nullable: is_nullable,
            kind: TypeKind::Named {
                name: vec![n].into_boxed_slice(),
                args: Vec::new().into_boxed_slice(),
            },
        }))
    }

    /// `implements A, B` opcional.
    fn parse_implements_opt(&mut self) -> PResult<Vec<TypeId>> {
        if self.eat_ident("implements") {
            self.parse_type_list()
        } else {
            Ok(Vec::new())
        }
    }

    /// `A, B, C` — ao menos um tipo.
    fn parse_type_list(&mut self) -> PResult<Vec<TypeId>> {
        let mut out = vec![self.parse_type()?];
        while self.eat_op(Op::Comma) {
            out.push(self.parse_type()?);
        }
        Ok(out)
    }

    /// `mixinDeclaration`.
    fn parse_mixin(&mut self) -> PResult<MixinDecl> {
        let base_span = self.at_ident("base").then(|| self.span());
        let mut base = self.eat_ident("base");
        // `beginMixinDeclaration` (`ast_builder.dart:488-497`): sem
        // `class-modifiers`, o `base` é o recurso desligado e é esquecido.
        if let Some(sp) = base_span
            && !self.features.tem(Feature::ClassModifiers)
        {
            self.require(Feature::ClassModifiers, sp);
            base = false;
        }
        self.expect_ident("mixin")?;
        let (name_text, name) = self.declaration_name()?;
        let type_params = self.parse_type_parameters_opt_variance()?;
        // Mixin não tem construtor primário: `(` ou `.nome(` depois do nome
        // é `UNEXPECTED_TOKEN` no primeiro token, e o grupo é pulado
        // (analyzer 3.13.4, `primary_constructors/syntax/mixin_error_test`).
        if self.at_op(Op::LParen) || (self.at_op(Op::Dot) && self.at_identifier_at(1)) {
            let texto = self.text().to_string();
            self.erro(codigos::parser::UNEXPECTED_TOKEN, &[&texto]);
            if self.eat_op(Op::Dot) {
                self.advance();
            }
            if self.at_op(Op::LParen) {
                match self.matching_close(self.pos) {
                    Some(f) if self.kind_of(f) == Kind::Op(Op::RParen) => {
                        // O 3.13.4 ainda lê os parâmetros: o `var` de um
                        // parâmetro (declarante) é o recurso desligado; o
                        // `final` era permitido antes.
                        let vars: Vec<Span> = (self.pos + 1..f)
                            .filter(|&i| self.kind_of(i) == Kind::Keyword(Keyword::Var))
                            .map(|i| self.tokens[i].span)
                            .collect();
                        for sp in vars {
                            self.require_in_ast(Feature::PrimaryConstructors, sp);
                        }
                        self.pos = f + 1;
                    }
                    _ => {}
                }
            }
        }
        // Recuperação do cabeçalho do mixin: um nome solto antes de `on`,
        // `implements` ou `{` é `UNEXPECTED_TOKEN` e é pulado
        // (`mixin sealed M {}`).
        if self.kind() == Kind::Ident
            && !self.at_ident("on")
            && !self.at_ident("implements")
            && (self.at_op_at(1, Op::LBrace) || self.at_ident_at(1, "on") || self.at_ident_at(1, "implements"))
        {
            let texto = self.text().to_string();
            self.erro(codigos::parser::UNEXPECTED_TOKEN, &[&texto]);
            self.advance();
        }
        let on = if self.eat_ident("on") {
            self.parse_type_list()?
        } else {
            Vec::new()
        };
        let implements = self.parse_implements_opt()?;
        self.owner = super::MemberOwner::Mixin;
        let members = self.parse_class_body_or_empty(Some(name_text))?;
        Ok(MixinDecl {
            base,
            name,
            type_params: type_params.into_boxed_slice(),
            on: on.into_boxed_slice(),
            implements: implements.into_boxed_slice(),
            members,
        })
    }

    /// `enumType`: constantes (com metadata, argumentos de tipo, construtor
    /// e argumentos), vírgula final opcional, `;` e membros opcionais.
    fn parse_enum(&mut self) -> PResult<EnumDecl> {
        let enum_token = self.expect_kw(Keyword::Enum)?;
        let primary_const = self.at_kw(Keyword::Const).then(|| self.advance().span);
        let name_text = self.text();
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_parameters_opt_variance()?;
        let primary_ = self.parse_primary_header_opt(enum_token.span, primary_const)?;
        let with = if self.eat_kw(Keyword::With) {
            self.parse_type_list()?
        } else {
            Vec::new()
        };
        let implements = self.parse_implements_opt()?;
        // Corpo vazio `;` (3.13, construtores primários): sem constantes nem
        // membros (o `ENUM_WITHOUT_CONSTANTS` sai depois).
        if self.at_op(Op::Semicolon) {
            let t = self.advance();
            self.require_in_ast(Feature::PrimaryConstructors, t.span);
            let mut members = Vec::new();
            let primary_constructor = self.elaborate_primary_constructor(name, primary_, &mut members, Elaborating::Enum);
            return Ok(EnumDecl {
                name,
                type_params: type_params.into_boxed_slice(),
                with: with.into_boxed_slice(),
                implements: implements.into_boxed_slice(),
                constants: Vec::new(),
                members,
                primary_constructor,
            });
        }
        let abre = self.pos;
        self.expect_op(Op::LBrace)?;
        let mut constants = Vec::new();
        while !self.at_op(Op::RBrace) && !self.at_op(Op::Semicolon) && !self.at_eof() {
            constants.push(self.parse_enum_constant()?);
            if self.eat_op(Op::Comma) || self.at_op(Op::RBrace) || self.at_op(Op::Semicolon) {
                continue;
            }
            // `parseEnum` (`parser_impl.dart`): outro nome é a constante
            // seguinte depois de uma vírgula que falta (`EXPECTED_TOKEN` `,`
            // nele); qualquer outra coisa é `EXPECTED_TOKEN` `}` nela e o
            // corpo acaba no `}` casado.
            match self.matching_close(abre) {
                Some(closes) if self.kind_of(closes) == Kind::Op(Op::RBrace) => {
                    if self.at_identifier() {
                        self.erro(codigos::parser::EXPECTED_TOKEN, &[","]);
                    } else {
                        self.erro(codigos::parser::EXPECTED_TOKEN, &["}"]);
                        self.pos = closes;
                        break;
                    }
                }
                _ => break,
            }
        }
        self.owner = super::MemberOwner::Enum;
        let mut members = if self.eat_op(Op::Semicolon) {
            self.parse_member_list(Some(name_text))?
        } else {
            self.expect_op(Op::RBrace)?;
            Vec::new()
        };
        let primary_constructor =
            self.elaborate_primary_constructor(name, primary_, &mut members, Elaborating::Enum);
        Ok(EnumDecl {
            name,
            type_params: type_params.into_boxed_slice(),
            with: with.into_boxed_slice(),
            implements: implements.into_boxed_slice(),
            constants,
            members,
            primary_constructor,
        })
    }

    /// `@meta nome<T>.ctor(args)`.
    fn parse_enum_constant(&mut self) -> PResult<EnumConstant> {
        let start = self.span();
        let metadata = self.parse_metadata()?;
        let name = self.expect_identifier()?;
        let type_args = if self.at_op(Op::Lt) {
            self.parse_type_arguments_opt()?
        } else {
            Vec::new()
        };
        let constructor = if self.eat_op(Op::Dot) {
            Some(self.identifier_or_new()?)
        } else {
            None
        };
        let arguments = if self.at_op(Op::LParen) {
            Some(self.parse_arguments()?)
        } else {
            None
        };
        Ok(EnumConstant {
            span: self.span_from(start),
            metadata: metadata.into_boxed_slice(),
            name,
            type_args: type_args.into_boxed_slice(),
            constructor,
            arguments,
        })
    }

    /// `extension` já foi visto: decide entre `extension [Nome]<T> on Tipo`
    /// e `extension type`. `extension type on X {}` é uma extension chamada
    /// `type`; `extension type Nome(...)` e `extension type const` são
    /// extension types.
    fn parse_extension_or_extension_type(&mut self) -> PResult<DeclKind> {
        let is_extension_type = self.at_ident_at(1, "type")
            && (self.at_kw_at(2, Keyword::Const)
                || (self.at_identifier_at(2)
                    && (!self.at_ident_at(2, "on")
                        || matches!(self.kind_at(3), Kind::Op(Op::LParen | Op::Lt | Op::Dot)))));
        if is_extension_type {
            Ok(DeclKind::ExtensionType(self.parse_extension_type()?))
        } else {
            Ok(DeclKind::Extension(self.parse_extension()?))
        }
    }

    /// `extension Nome? <T>? on Tipo { members_ }`.
    fn parse_extension(&mut self) -> PResult<ExtensionDecl> {
        self.expect_ident("extension")?;
        // `on` só é nome da extension se ainda houver um `on` (ou `<`) depois.
        let named = self.at_identifier()
            && (!self.at_ident("on") || self.at_ident_at(1, "on") || self.at_op_at(1, Op::Lt));
        let (name_text, name) = if named {
            let text = self.text();
            (Some(text), Some(self.identifier()))
        } else {
            (None, None)
        };
        let type_params = self.parse_type_parameters_opt()?;
        // `parseExtensionDeclaration` (`parser_impl.dart`): sem `on`,
        // `extends`/`implements`/`with` fazem as vezes dele com
        // `EXPECTED_INSTEAD`; senão `EXPECTED_TOKEN` (`on`) no último token
        // lido e um `on` sintético. Sem tipo depois (`{`), `EXPECTED_TYPE_NAME`
        // no `{` e um tipo sintético; o corpo é lido normalmente.
        if !self.eat_ident("on") {
            if self.at_kw(Keyword::Extends) || self.at_kw(Keyword::With) || self.at_ident("implements") {
                self.erro(codigos::parser::EXPECTED_INSTEAD, &["on"]);
                self.advance();
            } else {
                let ultimo = self.tokens[self.pos - 1].span;
                self.error_at(codigos::parser::EXPECTED_TOKEN, ultimo, &["on"]);
            }
        }
        let on = if matches!(self.kind(), Kind::Op(Op::LBrace | Op::Semicolon | Op::RBrace) | Kind::Eof) {
            let here = self.span();
            self.error_at(codigos::parser::EXPECTED_TYPE_NAME, here, &[]);
            let s = Span { start: here.start, end: here.start };
            let nome = self.name_from("", s);
            self.ast.push_type(TypeAnnotation {
                span: s,
                nullable: false,
                kind: TypeKind::Named { name: vec![nome].into_boxed_slice(), args: Vec::new().into_boxed_slice() },
            })
        } else {
            self.parse_type()?
        };
        self.owner = super::MemberOwner::Extension;
        let members = self.parse_class_body_or_empty(name_text)?;
        Ok(ExtensionDecl {
            name,
            type_params: type_params.into_boxed_slice(),
            on,
            members,
        })
    }

    /// `extension type const? Nome<T>(.ctor)? (@meta final? Tipo nome) implements I { }`.
    fn parse_extension_type(&mut self) -> PResult<ExtensionTypeDecl> {
        self.expect_ident("extension")?;
        self.expect_ident("type")?;
        let const_ = self.eat_kw(Keyword::Const);
        let name_text = self.text();
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_parameters_opt()?;
        let has_primary = self.at_op(Op::LParen) || self.at_op(Op::Dot);
        let constructor = if self.eat_op(Op::Dot) {
            Some(self.identifier_or_new()?)
        } else {
            None
        };
        let representation_start = self.span();
        let (representation_metadata, representation_type, representation_name) = if self.at_op(Op::LParen) {
            self.parse_representation()?
        } else {
            // `parseExtensionTypeDeclaration`: sem `(` nem `.`,
            // `MISSING_PRIMARY_CONSTRUCTOR` no último token lido (o nome ou o
            // `>`); com `.nome` sem `(`, `MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS`
            // no nome. O resto da declaração é lido normalmente, com a
            // representação sintética (vazia, sem largura).
            let code = if has_primary {
                codigos::parser::MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS
            } else {
                codigos::parser::MISSING_PRIMARY_CONSTRUCTOR
            };
            let ultimo = self.tokens[self.pos - 1].span;
            self.error_at(code, ultimo, &[]);
            let s = Span { start: ultimo.end, end: ultimo.end };
            let nome = self.name_from("", s);
            let ty = self.ast.push_type(TypeAnnotation {
                span: s,
                nullable: false,
                kind: TypeKind::Named { name: vec![nome].into_boxed_slice(), args: Vec::new().into_boxed_slice() },
            });
            (Vec::new(), ty, nome)
        };
        let representation_span = self.span_from(representation_start);
        let implements = self.parse_implements_opt()?;
        self.owner = super::MemberOwner::ExtensionType;
        let members = self.parse_class_body_or_empty(Some(name_text))?;
        // A segunda parte `this` em diante: `MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS`
        // (o mesmo `_checkForMultiplePrimaryConstructorBodyDeclarations` da classe).
        let partes: Vec<MemberId> = members
            .iter()
            .copied()
            .filter(|m| matches!(&self.ast.member(*m).kind, MemberKind::Constructor(c) if c.parte_primaria))
            .collect();
        for &m in partes.iter().skip(1) {
            let span = self.this_span(m);
            self.error_at(codigos::compile_time_error::MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS, span, &[]);
        }
        // A parte de corpo `this …` do primário do tipo de extensão (a
        // representação), como a de classe (`elaborate_primary_constructor`):
        // `const` não aceita corpo nenhum; fora dele, `=>` é o erro.
        for &mid in members.iter() {
            let MemberKind::Constructor(c) = &self.ast.member(mid).kind else { continue };
            if !c.parte_primaria {
                continue;
            }
            let Some(span) = self.primary_bodies.get(&c.class_name.span.start).copied() else { continue };
            let code = match (&c.body, const_) {
                (FunctionBody::Block(_), true) => Some(codigos::parser::CONST_PRIMARY_CONSTRUCTOR_WITH_BLOCK_BODY),
                (FunctionBody::Expression(_), true) => Some(codigos::parser::CONST_PRIMARY_CONSTRUCTOR_WITH_EXPRESSION_BODY),
                (FunctionBody::Expression(_), false) => Some(codigos::compile_time_error::PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY),
                _ => None,
            };
            if let Some(code) = code {
                self.error_at(code, span, &[]);
            }
        }
        Ok(ExtensionTypeDecl {
            const_,
            name,
            type_params: type_params.into_boxed_slice(),
            constructor,
            representation_metadata: representation_metadata.into_boxed_slice(),
            representation_type,
            representation_name,
            representation_span,
            implements: implements.into_boxed_slice(),
            members,
        })
    }

    /// A lista de parâmetros do construtor primário de um extension type
    /// (`parseFormalParameters` com `MemberKind.PrimaryConstructor`) e a
    /// representação que o `AstBuilder.endPrimaryConstructor` tira dela: o
    /// primeiro parâmetro, se é posicional simples (senão
    /// `EXPECTED_REPRESENTATION_FIELD` no token depois do `(`, e tipo e nome
    /// sintéticos); sem tipo, `EXPECTED_REPRESENTATION_TYPE`; `var`/`final`
    /// (antes da 3.13, em que `final` declara), `REPRESENTATION_FIELD_MODIFIER`;
    /// a vírgula depois dele, `REPRESENTATION_FIELD_TRAILING_COMMA` (um só
    /// parâmetro) ou `MULTIPLE_REPRESENTATION_FIELDS`.
    fn parse_representation(&mut self) -> PResult<(Vec<Annotation>, TypeId, Name)> {
        let abre = self.pos;
        if !self.at_op(Op::LParen) {
            return Err(self.expected_error("("));
        }
        let salvo = self.in_primary_constructor;
        self.in_primary_constructor = true;
        self.params_owner = ParameterOwner::PrimaryConstructor;
        let saved_repr = std::mem::replace(&mut self.in_representation, true);
        let params = self.parse_formal_parameters();
        self.in_representation = saved_repr;
        self.in_primary_constructor = salvo;
        let mut params = params?;
        let after_open = self.tokens[abre + 1].span;
        let synthetic = |p: &mut Self| {
            let s = Span { start: after_open.start, end: after_open.start };
            let nome = p.name_from("", s);
            let ty = p.ast.push_type(TypeAnnotation {
                span: s,
                nullable: false,
                kind: TypeKind::Named { name: vec![nome].into_boxed_slice(), args: Vec::new().into_boxed_slice() },
            });
            (ty, nome)
        };
        let simple = params.first().is_some_and(|p| {
            p.kind == ParameterKind::Required && !p.this_ && !p.super_ && p.function_parameters.is_none() && p.name.is_some()
        });
        // O analyzer 3.13.4 relata mais de um parâmetro como
        // `MULTIPLE_REPRESENTATION_FIELDS` na primeira vírgula, qualquer que
        // seja a forma do primeiro; o 3.6.2, o campo que falta. Qual dos
        // dois fica depende da referência da unidade, que só se conhece no
        // fim (`Parsed::referencia`): saem os dois, cada um marcado.
        let n = params.len();
        // O 3.13.4 confere o nome do único parâmetro da representação contra
        // os membros de `Object` (`EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT`
        // no nome, `error_verifier.dart` do checkout main, `:5641-5655`),
        // qualquer que seja a forma do parâmetro; o 3.6.2 não confere.
        if n == 1
            && let Some(p) = params.first()
            && !p.this_
            && !p.super_
            && let Some(nome) = p.name
            && matches!(&self.source[nome.span.start..nome.span.end], "hashCode" | "noSuchMethod" | "runtimeType" | "toString" | "==")
        {
            self.error_at(codigos::compile_time_error::EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT, nome.span, &[]);
            self.so_3_13.push((codigos::compile_time_error::EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT, nome.span));
        }
        if n > 1 && !simple {
            let end_offset = params[0].span.end;
            if let Some(virgula) =
                self.tokens[abre..self.pos].iter().find(|t| t.span.start >= end_offset && t.kind == Kind::Op(Op::Comma))
            {
                let span = virgula.span;
                self.error_at(codigos::parser::MULTIPLE_REPRESENTATION_FIELDS, span, &[]);
                self.so_3_13.push((codigos::parser::MULTIPLE_REPRESENTATION_FIELDS, span));
            }
            self.error_at(codigos::parser::EXPECTED_REPRESENTATION_FIELD, after_open, &[]);
            self.so_3_6.push((codigos::parser::EXPECTED_REPRESENTATION_FIELD, after_open));
            let (ty, nome) = synthetic(self);
            return Ok((Vec::new(), ty, nome));
        }
        if !simple {
            // O 3.13.4 relata no `ErrorVerifier`
            // (`_checkForExtensionTypeRepresentationErrorCodes`): o `this`
            // de `int this.x`, o `super` de `int super.x`, o parâmetro sem
            // nome inteiro, o delimitador de `[`/`{` (recurso desligado) e o
            // começo do parâmetro de função; o 3.6.2, o token depois do `(`.
            let in_3_13 = params.first().and_then(|p| {
                let word = |pp: &Self, k: Keyword| {
                    pp.tokens[abre..pp.pos].iter().find(|t| t.span.start >= p.span.start && t.kind == Kind::Keyword(k)).map(|t| t.span)
                };
                let start_offset = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
                if p.this_ {
                    word(self, Keyword::This)
                } else if p.super_ {
                    word(self, Keyword::Super)
                } else if p.name.is_none() {
                    Some(Span { start: start_offset, end: p.span.end })
                } else if p.kind != ParameterKind::Required && !self.features.tem(Feature::PrimaryConstructors) {
                    self.tokens[abre..self.pos]
                        .iter()
                        .find(|t| matches!(t.kind, Kind::Op(Op::LBracket | Op::LBrace)))
                        .map(|t| t.span)
                } else if p.function_parameters.is_some() && !self.features.tem(Feature::PrimaryConstructors) {
                    self.tokens[abre..self.pos].iter().find(|t| t.span.start >= start_offset).map(|t| t.span)
                } else {
                    None
                }
            });
            match in_3_13 {
                Some(sp) if sp != after_open => {
                    self.error_at(codigos::parser::EXPECTED_REPRESENTATION_FIELD, after_open, &[]);
                    self.so_3_6.push((codigos::parser::EXPECTED_REPRESENTATION_FIELD, after_open));
                    self.error_at(codigos::parser::EXPECTED_REPRESENTATION_FIELD, sp, &[]);
                    self.so_3_13.push((codigos::parser::EXPECTED_REPRESENTATION_FIELD, sp));
                }
                _ => {
                    self.error_at(codigos::parser::EXPECTED_REPRESENTATION_FIELD, after_open, &[]);
                }
            }
            let (ty, nome) = synthetic(self);
            return Ok((Vec::new(), ty, nome));
        }
        let p = params.swap_remove(0);
        let with_type = p.ty.is_some();
        let ty = match p.ty {
            Some(ty) => ty,
            // Com `final`/`var` e o recurso ligado, a forma é a declarante,
            // que aceita sem tipo. Com ele desligado (caso c23 de
            // `corpus/especificacao/t2/v`), o 3.6.2 relata o tipo que falta
            // no token depois do `(` e o 3.13.4, no nome.
            None if p.final_ || p.var_ => {
                if !self.features.tem(Feature::PrimaryConstructors) {
                    self.error_at(codigos::parser::EXPECTED_REPRESENTATION_TYPE, after_open, &[]);
                    self.so_3_6.push((codigos::parser::EXPECTED_REPRESENTATION_TYPE, after_open));
                    // Com mais de um parâmetro, o 3.13.4 só relata
                    // `MULTIPLE_REPRESENTATION_FIELDS` (`extension type
                    // ET3(final i, final x)`).
                    if let Some(nome) = p.name
                        && n == 1
                    {
                        self.error_at(codigos::parser::EXPECTED_REPRESENTATION_TYPE, nome.span, &[]);
                        self.so_3_13.push((codigos::parser::EXPECTED_REPRESENTATION_TYPE, nome.span));
                    }
                }
                synthetic(self).0
            }
            None => {
                // O 3.13.4 relata no nome (`@anno int`: `int` é o nome).
                match p.name {
                    Some(nome) if nome.span != after_open => {
                        self.error_at(codigos::parser::EXPECTED_REPRESENTATION_TYPE, after_open, &[]);
                        self.so_3_6.push((codigos::parser::EXPECTED_REPRESENTATION_TYPE, after_open));
                        self.error_at(codigos::parser::EXPECTED_REPRESENTATION_TYPE, nome.span, &[]);
                        self.so_3_13.push((codigos::parser::EXPECTED_REPRESENTATION_TYPE, nome.span));
                    }
                    _ => {
                        self.error_at(codigos::parser::EXPECTED_REPRESENTATION_TYPE, after_open, &[]);
                    }
                }
                synthetic(self).0
            }
        };
        // O argumento só entra no molde do 3.13.4, que escreve `var` também
        // para o `final` (casos c02 e c23). Com mais de um parâmetro o
        // 3.13.4 só relata `MULTIPLE_REPRESENTATION_FIELDS`
        // (`extension type E(final i, final x)`): o do modificador é só do
        // 3.6.2. O `var` com o recurso desligado é, no 3.13.4, uso de
        // construtor primário (relatado pelo `AstBuilder`, versão `3.13`), e
        // faz dele a referência da unidade — só com tipo depois do `var`
        // (`parseFormalParameter`, `typeInfo != noType`): `(var i)` não.
        let modifier = p.var_ || (p.final_ && !self.features.tem(Feature::PrimaryConstructors));
        if modifier {
            let target_expr = if p.var_ { Keyword::Var } else { Keyword::Final };
            if let Some(t) = self.tokens[abre..self.pos].iter().find(|t| t.span.start >= p.span.start && t.kind == Kind::Keyword(target_expr)) {
                let span = t.span;
                if p.var_ && with_type {
                    self.require_in_ast(Feature::PrimaryConstructors, span);
                }
                self.error_at(codigos::parser::REPRESENTATION_FIELD_MODIFIER, span, &["var"]);
                if n > 1 {
                    self.so_3_6.push((codigos::parser::REPRESENTATION_FIELD_MODIFIER, span));
                }
            }
        }
        if let Some(virgula) = self.tokens[abre..self.pos].iter().find(|t| t.span.start >= p.span.end).filter(|t| t.kind == Kind::Op(Op::Comma)) {
            let span = virgula.span;
            let code = if n == 1 {
                codigos::parser::REPRESENTATION_FIELD_TRAILING_COMMA
            } else {
                codigos::parser::MULTIPLE_REPRESENTATION_FIELDS
            };
            self.error_at(code, span, &[]);
        }
        let nome = p.name.expect("parâmetro simples tem nome");
        Ok((p.metadata.into_vec(), ty, nome))
    }

    /// Posição após `<...>` iniciado em `pos` contando só `<`/`>` (serve para
    /// parâmetros de tipo, que podem ter `extends`); `pos` se não houver
    /// `<` ou se a lista não fechar antes de `;`/`{`/`}`/`=`.
    fn skip_angles(&self, pos: usize) -> usize {
        if self.kind_of(pos) != Kind::Op(Op::Lt) {
            return pos;
        }
        let mut depth = 0usize;
        let mut i = pos;
        loop {
            match self.kind_of(i) {
                Kind::Op(Op::Lt) => depth += 1,
                Kind::Op(Op::Gt) => {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1;
                    }
                }
                Kind::Op(Op::Semicolon | Op::LBrace | Op::RBrace | Op::Assign | Op::Arrow)
                | Kind::Eof => return pos,
                _ => {}
            }
            i += 1;
        }
    }

    /// `typedef F<T> = kind;` ou a forma antiga `typedef ret? F<T>(params);`.
    /// `ensureIdentifierPotentiallyRecovered(…, typedefDeclaration,
    /// isRecovered: true)` (`identifier_context_impl.dart:1166-1189`): o nome
    /// do typedef de estilo novo (antes do `=`), ou do antigo seguido de `(`,
    /// que é palavra embutida (ou o `Function`, pseudo) é
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` no nome; a outra pseudo-palavra
    /// passa. O `BUILT_IN_IDENTIFIER_IN_DECLARATION` sai do `ErrorVerifier`.
    fn recovered_typedef_name(&mut self) {
        let texto = self.text().to_string();
        let relata = match super::fasta::word_style(&texto) {
            Some(super::fasta::WordStyle::BuiltIn) => true,
            Some(super::fasta::WordStyle::Pseudo) => texto == "Function",
            _ => false,
        };
        if relata {
            self.erro(codigos::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, &[&texto]);
        }
    }

    fn parse_typedef(&mut self) -> PResult<TypedefDecl> {
        self.expect_ident("typedef")?;
        if self.at_identifier() {
            // `computeTypeParamOrArg(inDeclaration: true)`: a lista inteira,
            // inclusive limites com tipos de função de parâmetros nomeados.
            let after = self.skip_type_arguments(self.pos + 1).unwrap_or_else(|| self.skip_angles(self.pos + 1));
            if self.kind_of(after) == Kind::Op(Op::Assign) {
                self.recovered_typedef_name();
                let name = self.identifier();
                let type_params = self.parse_type_parameters_opt()?;
                let equals_ = self.span();
                self.expect_op(Op::Assign)?;
                let ty = self.parse_type()?;
                // `endTypedef` (`ast_builder.dart:3496-3501`): o alvo que não é
                // `GenericFunctionType` sem `nonfunction-type-aliases`, no `=`.
                if !matches!(self.ast.ty(ty).kind, TypeKind::Function { .. }) {
                    self.require(Feature::NonfunctionTypeAliases, equals_);
                }
                self.ensure_semicolon()?;
                return Ok(TypedefDecl {
                    name,
                    type_params: type_params.into_boxed_slice(),
                    kind: TypedefKind::Alias(ty),
                });
            }
            if self.kind_of(after) == Kind::Op(Op::LParen) {
                self.recovered_typedef_name();
                let name = self.identifier();
                let type_params = self.parse_type_parameters_opt()?;
                self.params_owner = ParameterOwner::TypeAlias;
                let parameters = self.parse_formal_parameters()?;
                self.ensure_semicolon()?;
                return Ok(TypedefDecl {
                    name,
                    type_params: type_params.into_boxed_slice(),
                    kind: TypedefKind::Legacy {
                        return_type: None,
                        parameters: parameters.into_boxed_slice(),
                    },
                });
            }
        }
        let return_type = Some(self.parse_type()?);
        if self.at_identifier() {
            let after = self.skip_type_arguments(self.pos + 1).unwrap_or_else(|| self.skip_angles(self.pos + 1));
            if self.kind_of(after) == Kind::Op(Op::LParen) {
                self.recovered_typedef_name();
            }
        }
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_parameters_opt()?;
        self.params_owner = ParameterOwner::TypeAlias;
        let parameters = self.parse_formal_parameters()?;
        self.ensure_semicolon()?;
        Ok(TypedefDecl {
            name,
            type_params: type_params.into_boxed_slice(),
            kind: TypedefKind::Legacy {
                return_type,
                parameters: parameters.into_boxed_slice(),
            },
        })
    }

    // -----------------------------------------------------------------------
    // Modificadores, funções e variáveis (comum a topo e membros)
    // -----------------------------------------------------------------------

    /// Modificadores em qualquer ordem (superconjunto), lidos como no fasta
    /// (`read_top_level_modifiers`/`read_member_modifiers`, que
    /// relatam ordem errada, repetição e o que não cabe no contexto).
    fn parse_modifiers(&mut self, topo: bool) -> Modifiers {
        let f = if topo { self.read_top_level_modifiers() } else { self.read_member_modifiers() };
        let span = |p: Option<usize>| p.map(|p| self.tokens[p].span);
        Modifiers {
            external: f.external.is_some(),
            external_span: span(f.external),
            static_: f.static_.is_some(),
            abstract_: f.abstract_.is_some(),
            abstract_span: span(f.abstract_),
            covariant: f.covariant.is_some(),
            late: f.late.is_some(),
            final_: f.final_.is_some(),
            const_: f.const_.is_some(),
            const_span: span(f.const_),
            var_: f.var_.is_some(),
            modifier_tokens: f,
        }
    }

    /// `get nome`, `set nome`, `operator op` começam aqui? Devolve o tipo do
    /// acessor sem consumir nada.
    /// Com `!topo` (membro), também `get`/`set` seguido de palavra
    /// reservada que indica método ou campo (`parseClassOrMixin…MemberImpl`:
    /// "Getter or setter followed by a reserved word (name)").
    fn accessor_follows_at(&self, topo: bool) -> Option<FunctionKind> {
        let name_after = |p: &Self| p.at_identifier_at(1) || (!topo && matches!(p.kind_at(1), Kind::Keyword(_)) && p.indicates_method_or_field(p.pos + 2));
        if self.at_ident("get") && name_after(self) {
            Some(FunctionKind::Getter)
        } else if self.at_ident("set") && name_after(self) {
            Some(FunctionKind::Setter)
        } else if self.at_ident("operator") && self.operator_follows() {
            Some(FunctionKind::Operator)
        } else {
            None
        }
    }

    /// `indicatesMethodOrField` (`parser_impl.dart`).
    pub(crate) fn indicates_method_or_field(&self, pos: usize) -> bool {
        matches!(self.kind_of(pos), Kind::Op(Op::Semicolon | Op::Assign | Op::LParen | Op::LBrace | Op::Arrow | Op::Lt))
    }

    /// O nome de uma declaração: identificador, ou a palavra reservada que
    /// a recuperação do fasta toma como nome quando o que segue indica
    /// método ou campo (`EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` nela).
    fn declaration_name_or_reserved(&mut self) -> PResult<Name> {
        if !self.at_identifier() && matches!(self.kind(), Kind::Keyword(_)) && self.indicates_method_or_field(self.pos + 1) {
            let texto = self.text().to_string();
            let token = self.advance();
            self.error_at(codigos::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, token.span, &[&texto]);
            return Ok(self.name_from(&texto, token.span));
        }
        self.expect_identifier()
    }

    /// O token após `operator` é um operador declarável? `operator<T>()` é
    /// um método chamado `operator`, então `<` só conta se `(` vier depois.
    fn operator_follows(&self) -> bool {
        self.operator_follows_at(self.pos)
    }

    /// Nome de operador como texto: `+`, `[]`, `[]=`, `>=`, `>>`, `>>>`…
    /// (`>` chega isolado do lexer e é composto aqui).
    fn parse_operator_name(&mut self) -> PResult<Name> {
        let start = self.span();
        let text: &str = match self.kind() {
            Kind::Op(Op::LBracket) => {
                self.advance();
                self.expect_op(Op::RBracket)?;
                if self.eat_op(Op::Assign) { "[]=" } else { "[]" }
            }
            Kind::Op(Op::Gt) => {
                let composed = self.composed_gt().unwrap_or(ComposedGt::Gt);
                let text = match composed {
                    ComposedGt::Gt => ">",
                    ComposedGt::GtEq => ">=",
                    ComposedGt::Shr => ">>",
                    ComposedGt::UShr => ">>>",
                    ComposedGt::ShrAssign | ComposedGt::UShrAssign => {
                        return Err({ let t = self.text().to_string(); self.erro(codigos::parser::INVALID_OPERATOR, &[&t]) });
                    }
                };
                self.eat_composed_gt(composed);
                text
            }
            Kind::Op(
                op @ (Op::Tilde
                | Op::Lt
                | Op::LtEq
                | Op::LtLt
                | Op::Plus
                | Op::Minus
                | Op::Star
                | Op::Slash
                | Op::TildeSlash
                | Op::Percent
                | Op::Pipe
                | Op::Caret
                | Op::Amp
                | Op::EqEq),
            ) => {
                // `operator ===`: o nome é o lexema (o scanner já relatou o
                // operador).
                let texto = if op == Op::EqEq && self.span().end - self.span().start == 3 { "===" } else { op.text() };
                self.advance();
                texto
            }
            _ => return Err(self.identifier_error()),
        };
        Ok(self.name_from(text, self.span_from(start)))
    }

    /// `kind? nome (params) body_node`, `kind? get nome body_node`, `tipo? set nome
    /// (params) corpo`, `tipo? operator op (params) corpo` ou
    /// `kind? a = 1, b;`. Os modificadores já foram lidos.
    fn parse_function_or_variables(
        &mut self,
        mods: Modifiers,
        start: Span,
        external_topo: Option<Span>,
        topo: bool,
    ) -> PResult<FunctionOrVariables> {
        if let Some(kind) = self.accessor_follows_at(topo) {
            let id = self.parse_accessor(mods, start, None, kind, external_topo, topo)?;
            return Ok(FunctionOrVariables::Function(id));
        }
        // A palavra reservada seguida do que indica método ou campo é o nome,
        // e o identificador, o tipo (`int new() => 1;`), sem `var`/`final`/`const`.
        let reserved_as_name = !mods.var_
            && !mods.final_
            && !mods.const_
            && matches!(self.kind_at(1), Kind::Keyword(_))
            && self.indicates_method_or_field(self.pos + 2);
        // `foo;`, `foo = e;`, `foo, ...` (também `static = 1;`, `external;`,
        // `C;`): o identificador é o NOME do campo sem tipo — `modifier_ok`
        // já recusou o papel de modificador onde cabia. O fasta 3.6.2 relata
        // `missing_const_final_var_or_type` no nome, salvo com
        // `const`/`final`/`var`, que dispensam o tipo sem erro (sondado no
        // SDK local, membro e topo). Seguido de palavra reservada
        // (`augment class C {}` sem o experimento), o identificador também é
        // o nome: uma palavra reservada não pode ser o nome depois de um tipo
        // (`computeType` do fasta devolve `noType`), e o `;` que falta é
        // relatado depois dele.
        if self.at_identifier()
            && !reserved_as_name
            && matches!(
                self.kind_at(1),
                Kind::Op(Op::Semicolon | Op::Assign | Op::Comma) | Kind::Keyword(_)
            )
        {
            let nome = self.identifier();
            if !mods.var_ && !mods.final_ && !mods.const_ {
                self.error_at(
                    codigos::parser::MISSING_CONST_FINAL_VAR_OR_TYPE,
                    nome.span,
                    &[],
                );
            }
            self.report_field_modifiers(mods, topo, nome.span);
            let variables = self.parse_declared_variables_tail(nome)?;
            return Ok(FunctionOrVariables::Variables(VariableList {
                external: mods.external,
                static_: mods.static_,
                abstract_: mods.abstract_ && !topo,
                covariant: mods.covariant,
                late: mods.late,
                final_: mods.final_,
                const_: mods.const_,
                var_: mods.var_,
                ty: None,
                variables: variables.into_boxed_slice(),
            }));
        }
        // `parseTopLevelMemberImpl`/`parseClassOrMixin…MemberImpl`: com
        // `var`/`final`/`const`, um padrão externo seguido de `=` (`const
        // f() = e`, `var (a, b) = e`) é
        // `PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD` sobre o
        // padrão, que é descartado; o resto é um campo de nome sintético.
        if (mods.var_ || mods.final_ || mods.const_)
            && let Some(end_offset) = self.outer_pattern_followed_by_equals(self.pos)
        {
            let start_offset = self.span();
            let span = Span { start: start_offset.start, end: self.tokens[end_offset].span.end };
            self.error_at(codigos::parser::PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD, span, &[]);
            self.pos = end_offset + 1;
            let nome = self.name_from("", Span { start: start_offset.start, end: start_offset.start });
            let variables = self.parse_declared_variables_tail(nome)?;
            return Ok(FunctionOrVariables::Variables(VariableList {
                external: mods.external,
                static_: mods.static_,
                abstract_: mods.abstract_ && !topo,
                covariant: mods.covariant,
                late: mods.late,
                final_: mods.final_,
                const_: mods.const_,
                var_: mods.var_,
                ty: None,
                variables: variables.into_boxed_slice(),
            }));
        }
        let ty = if mods.var_ {
            None
        } else if self.at_kw(Keyword::Void)
            || self.looks_like_type_then_identifier_in_declaration(self.pos)
            // "<return type>? <reserved word> <token indicating method or
            // field>": a palavra reservada é o nome (`int new() => 1;`).
            || (!mods.final_
                && !mods.const_
                && self.skip_type(self.pos).is_some_and(|end_offset| matches!(self.kind_of(end_offset), Kind::Keyword(_)) && self.indicates_method_or_field(end_offset + 1)))
        {
            Some(self.parse_type()?)
        } else {
            None
        };
        if let Some(kind) = self.accessor_follows_at(topo) {
            let id = self.parse_accessor(mods, start, ty, kind, external_topo, topo)?;
            return Ok(FunctionOrVariables::Function(id));
        }
        // `(` sem tipo nem modificadores não abre declaração: um tipo record
        // pediria um nome depois do `)`. É lixo no topo, e o fasta relata
        // `expected_executable` e continua no token seguinte (ver
        // `recover_top_level`), em vez do `missing_identifier` genérico.
        if topo && ty.is_none() && !mods.algum() && self.at_op(Op::LParen) {
            return Err(self.erro(codigos::parser::EXPECTED_EXECUTABLE, &[]));
        }
        // Fasta `TopLevelDeclarationIdentifierContext`: no topo, uma palavra
        // que abre a declaração seguinte (`mixin M`, `extension X`,
        // `typedef`, `import`…) não é aceita como nome depois de um tipo, a
        // menos que venha `;`, `=`, `,` ou os parâmetros de uma função
        // (`(`, `<`). O nome vira sintético (antes
        // dela), com `missing_identifier` na palavra, e o `;` que falta é
        // relatado no tipo (`augment mixin M {}` sem o experimento).
        // O mesmo vale, depois de modificadores ou de um tipo, para as
        // palavras reservadas que abrem declaração (`class`, `enum`, `final`,
        // `const`, `var`, `void`): `final abstract class C {}` e
        // `final final class C {}` são um campo sem nome (`;` que falta no
        // último modificador) seguido da classe.
        // `looksLikeStartOfNextTopLevelDeclaration`
        // (identifier_context_impl.dart:1330): `isTopLevelKeyword`, ou
        // `const`, `get`, `final`, `set`, `var`, `void`.
        let texto = self.text_of(self.pos);
        let next_declaration = matches!(self.kind(), Kind::Ident | Kind::Keyword(_))
            && (super::fasta::is_top_level_word(texto) || matches!(texto, "const" | "get" | "final" | "set" | "var" | "void"));
        if topo
            && (ty.is_some() || mods.algum())
            && next_declaration
            // Seguida de parâmetros é o nome de uma função (`String
            // extension(String path)` do `package:path`), como no Fasta.
            // Os `followingValues` do contexto: `;`, `=`, `,` no de variável;
            // `<`, `(`, `{`, `=>` no de função (o que vem depois do nome
            // decide qual é).
            && !matches!(self.kind_at(1), Kind::Op(Op::Semicolon | Op::Assign | Op::Comma | Op::LParen | Op::Lt | Op::LBrace | Op::Arrow))
        {
            // `insertSyntheticIdentifier` antes da palavra: a variável fica,
            // com o nome sintético (o analyzer ainda relata o que falta nela,
            // `FINAL_NOT_INITIALIZED` e afins, com o nome vazio), e o
            // `ensureSemicolon` relata o `;` que falta no token anterior.
            let word = self.span();
            self.error_at(codigos::parser::MISSING_IDENTIFIER, word, &[]);
            let _ = self.expected_error(";");
            let nome = self.name_from("", Span { start: word.start, end: word.start });
            self.report_field_modifiers(mods, topo, nome.span);
            return Ok(FunctionOrVariables::Variables(VariableList {
                external: mods.external,
                static_: mods.static_,
                abstract_: mods.abstract_ && !topo,
                covariant: mods.covariant,
                late: mods.late,
                final_: mods.final_,
                const_: mods.const_,
                var_: mods.var_,
                ty,
                variables: vec![crate::ast::Variable { name: nome, initializer: None }].into_boxed_slice(),
            }));
        }
        // `recoverFromInvalidMember`: num membro, `(` ou `{` no lugar do nome
        // é método de nome sintético (`MISSING_IDENTIFIER` no token).
        let name = if !topo && matches!(self.kind(), Kind::Op(Op::LParen | Op::LBrace)) {
            let s = self.span();
            self.error_at(codigos::parser::MISSING_IDENTIFIER, s, &[]);
            self.name_from("", Span { start: s.start, end: s.start })
        } else {
            self.declaration_name_or_reserved()?
        };
        // `parseTopLevelMemberImpl`/`parseClassOrMixin...MemberImpl`: nome
        // seguido de `{` ou `=>` é método sem parâmetros
        // (`parseGetterOrFormalParameters`: `MISSING_FUNCTION_PARAMETERS`, ou
        // `MISSING_METHOD_PARAMETERS` em classe, no nome) e o corpo é lido.
        // Com `.` depois do nome também (`augment core.int foo();` sem o
        // experimento: `augment` é o tipo e `core`, a função).
        let without_parameters = self.at_op(Op::LBrace) || self.at_op(Op::Arrow) || self.at_op(Op::Dot);
        if self.at_op(Op::LParen) || self.at_op(Op::Lt) || without_parameters {
            self.report_method_modifiers(mods, topo, FunctionKind::Function);
            let type_params = self.parse_type_parameters_opt()?;
            self.params_owner = self.parameter_owner(mods, topo);
            let parameters = if without_parameters {
                let code = if !topo && !matches!(self.owner, super::MemberOwner::Extension | super::MemberOwner::ExtensionType) {
                    codigos::parser::MISSING_METHOD_PARAMETERS
                } else {
                    codigos::parser::MISSING_FUNCTION_PARAMETERS
                };
                // No nome sintético, o erro vai para o token seguinte.
                let target_expr = if name.span.start == name.span.end { self.span() } else { name.span };
                self.error_at(code, target_expr, &[]);
                Vec::new()
            } else {
                self.parse_formal_parameters()?
            };
            let body_start = self.pos;
            let (modifier, body) = self.parse_function_body()?;
            self.check_external_body(mods.external, false, body_start, &body, external_topo);
            self.check_empty_body(&body, Self::allows_abstract(mods, modifier, topo));
            self.check_abstract_in_extension(mods, topo, name.span, &body);
            let id = self.ast.push_function(Function {
                span: self.span_from(start),
                external: mods.external,
                static_: mods.static_,
                kind: FunctionKind::Function,
                return_type: ty,
                name: Some(name),
                type_params: type_params.into_boxed_slice(),
                parameters: Some(parameters.into_boxed_slice()),
                modifier,
                body,
            });
            return Ok(FunctionOrVariables::Function(id));
        }
        self.report_field_modifiers(mods, topo, name.span);
        // `parseFields`: sem tipo e sem `var`/`final`/`const`,
        // `MISSING_CONST_FINAL_VAR_OR_TYPE` no nome (`interface interface
        // class C {}`: o identificador embutido não é tipo).
        if ty.is_none() && !mods.var_ && !mods.final_ && !mods.const_ {
            self.error_at(codigos::parser::MISSING_CONST_FINAL_VAR_OR_TYPE, name.span, &[]);
        }
        let variables = self.parse_declared_variables_tail(name)?;
        Ok(FunctionOrVariables::Variables(VariableList {
            external: mods.external,
            static_: mods.static_,
            abstract_: mods.abstract_ && !topo,
            covariant: mods.covariant,
            late: mods.late,
            final_: mods.final_,
            const_: mods.const_,
            var_: mods.var_,
            ty,
            variables: variables.into_boxed_slice(),
        }))
    }

    /// `skipOuterPattern` + `=`: o último token de um padrão externo
    /// (`Nome(...)`, `p.Nome(...)`, `(...)`, `[...]`, `{...}`) que começa em
    /// `pos` e é seguido de `=`.
    fn outer_pattern_followed_by_equals(&self, pos: usize) -> Option<usize> {
        let mut i = pos;
        if self.kind_of(i) == Kind::Ident {
            i += 1;
            if self.kind_of(i) == Kind::Op(Op::Dot) && self.kind_of(i + 1) == Kind::Ident {
                i += 2;
            }
            if self.kind_of(i) == Kind::Op(Op::Lt) {
                i = self.skip_type_arguments(i)?;
            }
            if self.kind_of(i) != Kind::Op(Op::LParen) {
                return None;
            }
        } else if !matches!(self.kind_of(i), Kind::Op(Op::LParen | Op::LBracket | Op::LBrace)) {
            return None;
        }
        let closes = self.matching_close(i)?;
        (self.kind_of(closes + 1) == Kind::Op(Op::Assign)).then_some(closes)
    }

    /// Getter, setter ou operador a partir de `get`/`set`/`operator`.
    fn parse_accessor(
        &mut self,
        mods: Modifiers,
        start: Span,
        return_type: Option<TypeId>,
        kind: FunctionKind,
        external_topo: Option<Span>,
        topo: bool,
    ) -> PResult<FunctionId> {
        self.report_method_modifiers(mods, topo, kind);
        self.advance();
        let mut type_params = Vec::new();
        let (name, parameters) = match kind {
            FunctionKind::Getter => {
                let name = self.declaration_name_or_reserved()?;
                self.check_member_name(name.span);
                // `parseGetterOrFormalParameters`: `get x(...)` é
                // `GETTER_WITH_PARAMETERS` no `(`, e a lista é lida.
                if self.at_op(Op::LParen) {
                    self.erro(codigos::parser::GETTER_WITH_PARAMETERS, &[]);
                    self.params_owner = self.parameter_owner(mods, topo);
                    self.parse_formal_parameters()?;
                }
                (name, None)
            }
            FunctionKind::Setter => {
                let name = self.declaration_name_or_reserved()?;
                self.check_member_name(name.span);
                // `parseGetterOrFormalParameters`: sem `(`, o erro de
                // parâmetros que faltam no nome (`missingParameterMessage`:
                // de método em classe, mixin e enum; de função no resto) e
                // uma lista vazia sintética.
                if !self.at_op(Op::LParen) {
                    let code = if !topo && !matches!(self.owner, super::MemberOwner::Extension | super::MemberOwner::ExtensionType) {
                        codigos::parser::MISSING_METHOD_PARAMETERS
                    } else {
                        codigos::parser::MISSING_FUNCTION_PARAMETERS
                    };
                    self.error_at(code, name.span, &[]);
                    (name, Some(Vec::new()))
                } else {
                    self.params_owner = self.parameter_owner(mods, topo);
                    (name, Some(self.parse_formal_parameters()?))
                }
            }
            _ => {
                let name = self.parse_operator_name()?;
                // `parseMethodTypeVar` lê os parâmetros de tipo do operador e
                // o `AstBuilder.endClassMethod` os recusa no intervalo `<…>`
                // (`TYPE_PARAMETER_ON_OPERATOR`); a árvore fica com eles.
                if self.at_op(Op::Lt) {
                    let abre = self.span();
                    type_params = self.parse_type_parameters_opt()?;
                    let list = self.span_from(abre);
                    self.error_at(codigos::parser::TYPE_PARAMETER_ON_OPERATOR, list, &[]);
                }
                self.params_owner = self.parameter_owner(mods, topo);
                (name, Some(self.parse_formal_parameters()?))
            }
        };
        let body_start = self.pos;
        let (modifier, body) = self.parse_function_body()?;
        self.check_external_body(mods.external, false, body_start, &body, external_topo);
        self.check_empty_body(&body, Self::allows_abstract(mods, modifier, topo));
        self.check_abstract_in_extension(mods, topo, name.span, &body);
        Ok(self.ast.push_function(Function {
            span: self.span_from(start),
            external: mods.external,
            static_: mods.static_,
            kind,
            return_type,
            name: Some(name),
            type_params: type_params.into_boxed_slice(),
            parameters: parameters.map(Vec::into_boxed_slice),
            modifier,
            body,
        }))
    }

    /// Os erros de modificador de um método, getter, setter ou operador: o
    /// ramo de método de `parseTopLevelMemberImpl` (`var` é
    /// `VAR_RETURN_TYPE`; `final`/`const`/`late`, `EXTRANEOUS_MODIFIER`) e o
    /// começo de `parseMethod` (`parser_impl.dart:4827`) para membros;
    /// O `MemberKind` do fasta para os parâmetros de uma função de topo ou
    /// de um membro (`parseMethod`): `covariant` não cabe em função de topo
    /// nem em método estático, e tem código próprio em extension e
    /// extension type.
    fn parameter_owner(&self, mods: Modifiers, topo: bool) -> ParameterOwner {
        use super::MemberOwner as D;
        match self.owner {
            _ if topo => ParameterOwner::TopLevelOrStatic,
            D::Extension => ParameterOwner::Extension,
            D::ExtensionType => ParameterOwner::ExtensionType,
            _ if mods.static_ => ParameterOwner::TopLevelOrStatic,
            _ => ParameterOwner::Other,
        }
    }

    /// Chamado assim que a declaração se revela método.
    fn report_method_modifiers(&mut self, mods: Modifiers, topo: bool, kind: FunctionKind) {
        let f = mods.modifier_tokens;
        if topo {
            if let Some(v) = f.var_final_or_const() {
                if f.var_.is_some() {
                    self.error_at_token(v, codigos::parser::VAR_RETURN_TYPE);
                } else {
                    self.stray_modifier(Some(v));
                }
            } else {
                self.stray_modifier(f.late);
            }
            return;
        }
        if let Some(a) = f.abstract_ {
            self.error_at_token(a, codigos::parser::ABSTRACT_CLASS_MEMBER);
        }
        self.stray_modifier(f.late);
        let accessor = matches!(kind, FunctionKind::Getter | FunctionKind::Setter);
        if let Some(st) = f.static_ {
            if kind == FunctionKind::Operator {
                self.error_at_token(st, codigos::parser::STATIC_OPERATOR);
            }
        } else if let Some(c) = f.covariant
            && kind != FunctionKind::Setter
        {
            self.error_at_token(c, codigos::parser::COVARIANT_MEMBER);
        }
        if let Some(c) = f.const_ {
            if accessor {
                self.stray_modifier(Some(c));
            } else {
                self.error_at_token(c, codigos::parser::CONST_METHOD);
            }
        } else if let Some(v) = f.var_ {
            self.error_at_token(v, codigos::parser::VAR_RETURN_TYPE);
        } else {
            self.stray_modifier(f.final_);
        }
    }

    /// Numa extension, método com corpo `;` sem `external` é
    /// `EXTENSION_DECLARES_ABSTRACT_MEMBER` no nome (no operador, para
    /// `operator`; `parser_impl.dart:5084`).
    fn check_abstract_in_extension(&mut self, mods: Modifiers, topo: bool, nome: Span, body: &FunctionBody) {
        if !topo
            && self.owner == super::MemberOwner::Extension
            && matches!(body, FunctionBody::Empty)
            && !mods.external
        {
            self.error_at(codigos::parser::EXTENSION_DECLARES_ABSTRACT_MEMBER, nome, &[]);
        }
    }

    /// Os erros de modificador de um campo (`parseFields`,
    /// `parser_impl.dart:3656`): `covariant final` sem `late` é
    /// `FINAL_AND_COVARIANT`; `covariant late final` com inicializador,
    /// `FINAL_AND_COVARIANT_LATE_WITH_INITIALIZER`; `abstract external`,
    /// `ABSTRACT_EXTERNAL_FIELD`; numa extension, sem `static` nem
    /// `external`, `EXTENSION_DECLARES_INSTANCE_FIELD` no primeiro nome
    /// (`parser_impl.dart:3809`).
    /// Chamado com o cursor logo depois do primeiro nome.
    fn report_field_modifiers(&mut self, mods: Modifiers, topo: bool, nome: Span) {
        let f = mods.modifier_tokens;
        if let (Some(c), Some(_)) = (f.covariant, f.final_) {
            if f.late.is_none() {
                self.error_at_token(c, codigos::parser::FINAL_AND_COVARIANT);
            } else if self.at_op(Op::Assign) {
                self.error_at_token(c, codigos::parser::FINAL_AND_COVARIANT_LATE_WITH_INITIALIZER);
            }
        }
        if let (Some(a), Some(_)) = (f.abstract_, f.external) {
            self.error_at_token(a, codigos::parser::ABSTRACT_EXTERNAL_FIELD);
        }
        if !topo
            && self.owner == super::MemberOwner::Extension
            && f.static_.is_none()
            && f.external.is_none()
        {
            self.error_at(codigos::parser::EXTENSION_DECLARES_INSTANCE_FIELD, nome, &[]);
        }
    }

    /// `parseFieldInitializerOpt` e o ramo de getter/setter de `parseMethod`
    /// (`parser_impl.dart:3934`, `:5003`): campo, getter ou setter com o
    /// nome da declaração envolvente é `MEMBER_WITH_CLASS_NAME` no nome.
    fn check_member_name(&mut self, nome: Span) {
        if self.enclosing_name.is_some_and(|n| n == &self.source[nome.start..nome.end]) {
            self.error_at(codigos::parser::MEMBER_WITH_CLASS_NAME, nome, &[]);
        }
    }

    /// `= e`? (`, nome = e`?)* `;` a partir do primeiro nome já lido.
    fn parse_declared_variables_tail(&mut self, first: Name) -> PResult<Vec<Variable>> {
        self.check_member_name(first.span);
        let mut variables = Vec::new();
        let mut name = first;
        loop {
            let initializer = if self.eat_op(Op::Assign) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            variables.push(Variable { name, initializer });
            if !self.eat_op(Op::Comma) {
                break;
            }
            // `ensureIdentifier` no contexto de declaração de campo/variável:
            // diante de `;`, `=`, `,` ou `}` (e do fim), o fasta relata
            // `MISSING_IDENTIFIER` e insere um identificador sintético, e a
            // lista segue com a variável sem nome (`Object? foo,;`).
            if matches!(self.kind(), Kind::Op(Op::Semicolon | Op::Assign | Op::Comma | Op::RBrace) | Kind::Eof) {
                self.erro(codigos::parser::MISSING_IDENTIFIER, &[]);
                let start_offset = self.span().start;
                name = self.name_from("", Span { start: start_offset, end: start_offset });
                continue;
            }
            name = self.expect_identifier()?;
            self.check_member_name(name.span);
        }
        // Sem inicializador, a declaração acabou no nome: o `;` que falta é
        // inserido diante de qualquer token, como no fasta (`augment Object?
        // foo();` é o campo `Object` seguido do `?` solto). Com inicializador
        // a expressão pode ter parado antes do que o fasta leria, e vale a
        // regra prudente de `ensure_semicolon`.
        if variables.last().is_some_and(|v| v.initializer.is_none()) && !self.speculating {
            self.ensure_semicolon_forced();
            return Ok(variables);
        }
        self.ensure_semicolon()?;
        Ok(variables)
    }

    // -----------------------------------------------------------------------
    // Membros
    // -----------------------------------------------------------------------

    /// `{ members_ }` de classe, mixin, extension ou extension type.
    /// `primary` diz se a declaração tem cabeçalho primário: nele, `this`
    /// e `new` abrem parte de corpo mesmo sem o recurso ligado (o oráculo
    /// 3.6 silencia pela cascata do cabeçalho, que aceitamos por superconjunto).
    fn parse_class_body(&mut self, class_name: Option<&'s str>) -> PResult<Vec<MemberId>> {
        self.expect_op(Op::LBrace)?;
        self.parse_member_list(class_name)
    }

    /// Membros até a `}` de fechamento (inclusive), com recuperação por
    /// membro. Um fim de arquivo prematuro registra o erro mas devolve os
    /// membros já lidos.
    fn parse_member_list(&mut self, class_name: Option<&'s str>) -> PResult<Vec<MemberId>> {
        let mut members = Vec::new();
        loop {
            if self.eat_op(Op::RBrace) {
                return Ok(members);
            }
            if self.at_eof() {
                self.expected_error("}");
                return Ok(members);
            }
            let start_pos = self.pos;
            match self.parse_member(class_name) {
                Ok(id) => members.push(id),
                Err(ParseError) => {
                    self.recover_member(start_pos, class_name, &mut members);
                    self.record_skipped(start_pos);
                }
            }
        }
    }

    /// Um `classMemberDefinition`: campo, método, acessor, operador ou
    /// construtor. `class_name` decide se `Nome(` é construtor.
    fn parse_member(&mut self, class_name: Option<&'s str>) -> PResult<MemberId> {
        self.enclosing_name = class_name;
        let r = self.parse_member_inside(class_name);
        self.enclosing_name = None;
        r
    }

    fn parse_member_inside(&mut self, class_name: Option<&'s str>) -> PResult<MemberId> {
        let start = self.span();
        let metadata = self.parse_metadata()?;
        // `=>` onde um membro era esperado: método sem nome nem parâmetros
        // (o fasta denuncia os dois no `=>`, sondado no SDK 3.6.2 local).
        if self.at_op(Op::Arrow) {
            let span = self.span();
            self.error_at(codigos::parser::MISSING_IDENTIFIER, span, &[]);
            self.error_at(codigos::parser::MISSING_METHOD_PARAMETERS, span, &[]);
            return Err(self.skip_broken_member());
        }
        // `this` (parte de construtor primário) e `new` (construtor sem o
        // nome da classe) iniciam membro em qualquer versão: sem o recurso,
        // o analyzer 3.13.4 lê o membro como com ele (com os diagnósticos
        // próprios da parte `this`) e só acrescenta `experiment_not_enabled`
        // no `this`/`new` (sondado numa biblioteca 3.6). O superconjunto
        // vale até em classe sem cabeçalho primário.
        if !self.can_start_declaration() && !self.at_kw(Keyword::This) && !self.at_kw(Keyword::New) {
            return Err(self.erro(codigos::parser::EXPECTED_CLASS_MEMBER, &[]));
        }
        let augment = self.parse_augment_opt();
        let fstart = self.span();
        let mods = self.parse_modifiers(false);
        // `this` abre parte de corpo com qualquer corpo (os diagnósticos
        // próprios valem), com ou sem o recurso.
        let part = self.at_kw(Keyword::This) && !self.at_op_at(1, Op::Dot);
        let kind = if part {
            // `this : inits? body_node` / `this;`: parte de corpo do construtor
            // primário.
            self.parse_primary_part(&mods)?
        } else if self.at_kw(Keyword::New)
            && (self.at_op_at(1, Op::LParen) || self.at_identifier_at(1))
        {
            // `new nome?(...)` (3.13): construtor com o nome da classe implícito.
            self.parse_new_constructor(mods, class_name)?
        } else if let Some(end_offset) = self.type_before_new() {
            // `parseClassMember` do 3.13.4: o `new` depois do tipo é o nome
            // de um construtor (`newToken`, `_isConstructor`): o tipo é
            // `CONSTRUCTOR_WITH_RETURN_TYPE`, e o resto é `new nome?(…)`. O
            // 3.6.2 leria um método chamado `new`, mas o `experiment_not_enabled`
            // do `new` já põe a unidade na referência 3.13.
            let tstart = self.span();
            // O tipo não fica na árvore (o `AstBuilder` o descarta).
            let types_before = self.ast.types.len();
            let _ty = self.parse_type()?;
            self.ast.types.truncate(types_before);
            self.error_at(codigos::parser::CONSTRUCTOR_WITH_RETURN_TYPE, self.span_from(tstart), &[]);
            debug_assert_eq!(self.pos, end_offset);
            self.parse_new_constructor(mods, class_name)?
        } else if let Some(end_offset) = self.type_before_factory() {
            // `parseClassMember`: um tipo antes de `factory` é
            // `TYPE_BEFORE_FACTORY` no último token dele, e o resto é a
            // factory (`augment factory C.f()` sem o experimento).
            let span = self.tokens[end_offset - 1].span;
            self.error_at(codigos::parser::TYPE_BEFORE_FACTORY, span, &[]);
            self.pos = end_offset;
            self.parse_constructor(mods, true, class_name)?
        } else if self.at_ident("factory")
            && (self.at_identifier_at(1)
                || (self.features.tem(Feature::PrimaryConstructors) && self.at_op_at(1, Op::LParen))
                || self.factory_new_as_constructor())
        {
            // `factory(` só é construtor a partir da 3.13; antes, é um método
            // chamado `factory` (a versão decide, VERSOES-LINGUAGEM.md §4.5).
            self.parse_constructor(mods, true, class_name)?
        } else if self.constructor_follows(class_name) {
            self.parse_constructor(mods, false, class_name)?
        } else if self.constructor_with_return(class_name) {
            // `T C(` (C é a classe) ou `T X.Y(` / `T X.Y` sem parênteses:
            // construtor com tipo de retorno — o fasta relata
            // `constructor_with_return_type` em T e, sem parênteses,
            // `missing_method_parameters` em X (sondado no SDK 3.6.2 local).
            // Sem isso, `augment A(...) : inits` caía no caminho de método
            // e a cauda falhava em cascata.
            let tstart = self.span();
            // O tipo não fica na árvore (o `AstBuilder` o descarta): nem os
            // verificadores dos tipos escritos o veem.
            let types_before = self.ast.types.len();
            let _ty = self.parse_type()?;
            self.ast.types.truncate(types_before);
            self.error_at(
                codigos::parser::CONSTRUCTOR_WITH_RETURN_TYPE,
                self.span_from(tstart),
                &[],
            );
            if self.constructor_follows(class_name) {
                self.parse_constructor(mods, false, class_name)?
            } else if self.method_without_parameters() {
                // `T X.Y` sem parênteses (`int C.named;`): X é denunciado.
                let span = self.span();
                self.error_at(
                    codigos::parser::MISSING_METHOD_PARAMETERS,
                    span,
                    &[],
                );
                return Err(self.skip_broken_member());
            } else {
                // Forma exótica (`T get.Y;`): sem regra sondada, só falha
                // para a recuperação sem novos erros.
                return Err(ParseError);
            }
        } else if self.method_without_parameters() {
            // `X.Y` / `X.new` sem `(` em membro (`C.named;`, `foo.bar = 1;`,
            // `C.named : ...`): o fasta relata `missing_method_parameters`
            // em X (sondado no SDK 3.6.2 local). `X.Y(` é construtor,
            // `X.Y<` é outro grupo e `X.Y nome` pode ser campo de tipo
            // prefixado — esses seguem o caminho antigo. O resto do membro
            // é pulado sem novos erros (cada código tem a sua vez).
            let span = self.span();
            self.error_at(
                codigos::parser::MISSING_METHOD_PARAMETERS,
                span,
                &[],
            );
            return Err(self.skip_broken_member());
        } else {
            match self.parse_function_or_variables(mods, fstart, None, false)? {
                FunctionOrVariables::Function(id) => MemberKind::Method(id),
                FunctionOrVariables::Variables(list) => {
                    // Fasta `AstBuilder._endClassFields`: a restrição deixa de
                    // valer com o experimento de augmentations.
                    if mods.static_ && !self.features.tem(Feature::Augmentations) {
                        if let Some(span) = mods.abstract_span {
                            self.diagnostics.push(Diagnostic::com_codigo(
                                codigos::parser::ABSTRACT_STATIC_FIELD,
                                span,
                                std::iter::empty::<&str>(),
                            ));
                        }
                    }
                    MemberKind::Field(list)
                }
            }
        };
        Ok(self.ast.push_member(Member {
            span: self.span_from(start),
            metadata: metadata.into_boxed_slice(),
            kind,
            augment,
        }))
    }

    /// Pula o resto do membro quebrado até `;` (consumido) ou `}`/fim
    /// (preservados), sem novos erros — cada código tem a sua vez — e
    /// devolve a falha para a recuperação continuar depois.
    fn skip_broken_member(&mut self) -> ParseError {
        loop {
            match self.kind() {
                Kind::Eof | Kind::Op(Op::RBrace) => break,
                Kind::Op(Op::Semicolon) => {
                    self.advance();
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
        ParseError
    }

    /// `X.Y` / `X.new` sem `(` nem `<` começam aqui, com o membro
    /// claramente terminado depois (`;`, `=`, `,`, `:`, `=>`, fecho, fim)?
    /// `get`/`set`/`operator`/`typedef`/`factory` e nomes seguidos de nome
    /// (`p.Foo x`) seguem o caminho antigo.
    fn method_without_parameters(&self) -> bool {
        if !self.at_identifier()
            || self.at_ident("get")
            || self.at_ident("set")
            || self.at_ident("operator")
            || self.at_ident("typedef")
            || self.at_ident("factory")
        {
            return false;
        }
        if !self.at_op_at(1, Op::Dot)
            || (!self.at_identifier_at(2) && !self.at_kw_at(2, Keyword::New))
        {
            return false;
        }
        matches!(
            self.kind_at(3),
            Kind::Op(
                Op::Semicolon
                    | Op::Assign
                    | Op::Comma
                    | Op::Colon
                    | Op::Arrow
                    | Op::RParen
                    | Op::RBracket
                    | Op::RBrace
            ) | Kind::Eof
        )
    }

    /// `T C(` (C é a classe) ou `T X.Y(` começam aqui? `get`/`set`/
    /// `operator`/`typedef`/`factory` têm caminho próprio e `void` segue o
    /// antigo; o resto com forma de construtor após o tipo é retorno.
    fn constructor_with_return(&self, class_name: Option<&str>) -> bool {
        if !self.at_identifier()
            || self.at_ident("get")
            || self.at_ident("set")
            || self.at_ident("operator")
            || self.at_ident("typedef")
            || self.at_ident("factory")
        {
            return false;
        }
        let Some(end) = self.skip_type(self.pos) else {
            return false;
        };
        if self.kind_of(end) != Kind::Ident {
            return false;
        }
        if self.kind_of(end + 1) == Kind::Op(Op::LParen) {
            return Some(self.text_of(end)) == class_name;
        }
        // Com parênteses (`T X.Y(`) o construtor é lido; sem (`T X.Y;`,
        // `int C.named;`) o X ainda cai na regra do método sem parâmetros.
        // `X.Y<` é outro grupo e `X.Y nome` pode ser campo prefixado.
        self.kind_of(end + 1) == Kind::Op(Op::Dot)
            && (self.kind_of(end + 2) == Kind::Ident
                || self.kind_of(end + 2) == Kind::Keyword(Keyword::New))
            && (self.kind_of(end + 3) == Kind::Op(Op::LParen)
                || matches!(
                    self.kind_of(end + 3),
                    Kind::Op(
                        Op::Semicolon
                            | Op::Assign
                            | Op::Comma
                            | Op::Colon
                            | Op::Arrow
                            | Op::RParen
                            | Op::RBracket
                            | Op::RBrace
                    ) | Kind::Eof
                ))
    }

    /// `Nome(` com o nome da classe (ou após `const`), ou `Nome.x(`.
    fn constructor_follows(&self, class_name: Option<&str>) -> bool {
        if !self.at_identifier() {
            return false;
        }
        if self.at_op_at(1, Op::LParen) {
            // `parseMethod`: com lista de inicialização (`Nome(...) :`) é
            // construtor qualquer que seja o nome (`INVALID_CONSTRUCTOR_NAME`
            // se não for o da classe).
            let with_initializers = self
                .matching_close(self.pos + 1)
                .is_some_and(|f| self.kind_of(f + 1) == Kind::Op(Op::Colon));
            // `const nome(` com outro nome é método (`CONST_METHOD`), como no
            // `parseMethod` do fasta.
            return class_name == Some(self.text()) || with_initializers;
        }
        // `Nome<T>(` com o nome da classe: construtor com parâmetros de tipo
        // (erro, relatado em `parse_constructor`).
        if class_name == Some(self.text())
            && self.at_op_at(1, Op::Lt)
            && self.skip_type_arguments(self.pos + 1).is_some_and(|f| self.kind_of(f) == Kind::Op(Op::LParen))
        {
            return true;
        }
        self.at_op_at(1, Op::Dot)
            && (self.at_identifier_at(2) || self.at_kw_at(2, Keyword::New))
            && self.at_op_at(3, Op::LParen)
    }

    /// `factory? C(.nome)? (params) (= Alvo.nome; | (: inits)? body_node)`. Na
    /// 3.13 também `factory nome?(...)`: sem o nome da classe (`factory(`) é
    /// o construtor sem nome, `factory id(` é `C.id` — salvo `id` = `C`, que
    /// continua o sem nome (spec, a exceção para não quebrar código antigo).
    fn parse_constructor(
        &mut self,
        mods: Modifiers,
        factory: bool,
        class: Option<&'s str>,
    ) -> PResult<MemberKind> {
        if factory {
            // `parseFactoryMethod` (`parser_impl.dart:5108`): `static`,
            // `covariant`, `var` e `final` antes de `factory` são
            // `EXTRANEOUS_MODIFIER`; `abstract`, `ABSTRACT_CLASS_MEMBER`
            // (`parseClassOrMixinOrExtensionOrEnumMemberImpl`).
            let f = mods.modifier_tokens;
            if let Some(a) = f.abstract_ {
                self.error_at_token(a, codigos::parser::ABSTRACT_CLASS_MEMBER);
            }
            self.stray_modifier(f.static_.or(f.covariant));
            self.stray_modifier(f.var_.or(f.final_));
            self.check_constructor_in_mixin_or_extension(self.span());
            let t = self.advance();
            if self.at_op(Op::LParen) {
                // `factory(...)`: só chega aqui na 3.13 (antes é método).
                let class_name = self.class_name(class, t.span);
                return self.parse_constructor_rest(mods, true, class_name, None);
            }
            if self.features.tem(Feature::PrimaryConstructors) && self.at_op_at(1, Op::LParen) {
                let id = self.expect_identifier()?;
                let class_name = self.class_name(class, t.span);
                let nome = (Some(&self.source[id.span.start..id.span.end]) != class).then_some(id);
                return self.parse_constructor_rest(mods, true, class_name, nome);
            }
        }
        // `factory new(…)` (3.13.4, `parseFactoryMethod`): o `new` é o nome,
        // com `FACTORY_CONSTRUCTOR_NEW_NAME`; o nome que não é o da classe é,
        // depois, `INVALID_FACTORY_NAME_NOT_A_CLASS`.
        if factory && self.at_kw(Keyword::New) && !self.at_op_at(1, Op::Dot) {
            let t = self.advance();
            self.error_at(codigos::parser::FACTORY_CONSTRUCTOR_NEW_NAME, t.span, &[]);
            let class_name = self.name_from("new", t.span);
            return self.parse_constructor_rest(mods, true, class_name, None);
        }
        let class_name = self.expect_identifier()?;
        if !factory {
            self.check_constructor_in_mixin_or_extension(class_name.span);
            // `parseMethod` (`parser_impl.dart:5014`): construtor com outro
            // nome (`Other.x()`) é `INVALID_CONSTRUCTOR_NAME` no nome;
            // `static`, `STATIC_CONSTRUCTOR`.
            if Some(&self.source[class_name.span.start..class_name.span.end]) != class {
                self.error_at(codigos::parser::INVALID_CONSTRUCTOR_NAME, class_name.span, &[]);
            }
            if let Some(st) = mods.modifier_tokens.static_ {
                self.error_at_token(st, codigos::parser::STATIC_CONSTRUCTOR);
            }
            // `AstBuilder.endClassConstructor` (`ast_builder.dart:5897`):
            // parâmetros de tipo num construtor são
            // `TYPE_PARAMETER_ON_CONSTRUCTOR` sobre a lista, que é lida.
            if self.at_op(Op::Lt) {
                let abre = self.span();
                self.parse_type_parameters_opt()?;
                let list = self.span_from(abre);
                self.error_at(codigos::parser::TYPE_PARAMETER_ON_CONSTRUCTOR, list, &[]);
            }
        }
        let name = if self.eat_op(Op::Dot) {
            Some(self.identifier_or_new()?)
        } else {
            None
        };
        self.parse_constructor_rest(mods, factory, class_name, name)
    }

    /// Construtor em mixin ou extension: `MIXIN_DECLARES_CONSTRUCTOR` /
    /// `EXTENSION_DECLARES_CONSTRUCTOR` no nome (no `factory`, se houver;
    /// `parser_impl.dart:5042` e `:5182`).
    fn check_constructor_in_mixin_or_extension(&mut self, span: Span) {
        let code = match self.owner {
            super::MemberOwner::Mixin => codigos::parser::MIXIN_DECLARES_CONSTRUCTOR,
            super::MemberOwner::Extension => codigos::parser::EXTENSION_DECLARES_CONSTRUCTOR,
            _ => return,
        };
        self.error_at(code, span, &[]);
    }

    /// O `Name` da classe dona para um construtor escrito sem ele (`new`,
    /// `factory(`, parte `this`), com o intervalo da palavra-chave.
    fn class_name(&mut self, class: Option<&str>, span: Span) -> Name {
        let texto = class.unwrap_or("").to_string();
        self.name_from(&texto, span)
    }

    /// `new nome?(params) (: inits)? body_node` (3.13): a mesma coisa que
    /// `C.nome(...)`/`C(...)`.
    fn parse_new_constructor(
        &mut self,
        mods: Modifiers,
        class: Option<&'s str>,
    ) -> PResult<MemberKind> {
        let t = self.advance();
        self.require_in_ast(Feature::PrimaryConstructors, t.span);
        // `_isConstructor` com `newToken`: o `static` é `STATIC_CONSTRUCTOR`
        // (`parseMethod` do 3.13.4).
        if let Some(st) = mods.modifier_tokens.static_ {
            self.error_at_token(st, codigos::parser::STATIC_CONSTRUCTOR);
        }
        let class_name = self.class_name(class, t.span);
        let nome = if self.at_identifier() {
            Some(self.identifier())
        } else {
            None
        };
        self.parse_constructor_rest(mods, false, class_name, nome)
    }

    /// `this (: inits)? body_node` (3.13): a parte de corpo do construtor
    /// primário. Vira um `Constructor { parte_primaria: true }` que a
    /// elaboração funde no `k2` da declaração. `async`/`sync*`/`=>` são erro.
    /// O `this` de uma parte de construtor primário (o `class_name` dela).
    fn this_span(&self, member: MemberId) -> Span {
        match &self.ast.member(member).kind {
            MemberKind::Constructor(c) => c.class_name.span,
            _ => self.ast.member(member).span,
        }
    }

    fn parse_primary_part(&mut self, mods: &Modifiers) -> PResult<MemberKind> {
        let t = self.advance();
        self.require_in_ast(Feature::PrimaryConstructors, t.span);
        // Os modificadores antes do `this` são `EXTRANEOUS_MODIFIER`, nesta
        // ordem (`parseClassOrMixinOrExtensionOrEnumMemberImpl` do 3.13.4,
        // `parser_impl.dart:5514-5556`): `var`/`final`/`const`, `external`,
        // `static`, `covariant`, `late`.
        let f = mods.modifier_tokens;
        for p in [f.var_final_or_const(), f.external, f.static_, f.covariant, f.late].into_iter().flatten() {
            let tok = self.tokens[p];
            let texto = self.text_of(p).to_string();
            self.error_at(codigos::parser::EXTRANEOUS_MODIFIER, tok.span, &[&texto]);
        }
        let mut initializers = Vec::new();
        if self.eat_op(Op::Colon) {
            loop {
                initializers.push(self.parse_initializer()?);
                if !self.eat_op(Op::Comma) {
                    break;
                }
            }
        }
        let antes = self.pos;
        let (modifier, body) = self.parse_function_body()?;
        // `async`, `async*`, `sync*` antes do corpo: o analyzer relata no
        // `async`/`sync`, com o modificador inteiro no texto. O corpo
        // (`=>`/`{`) é julgado na elaboração, que sabe se é `const`.
        let mut body_node = antes;
        if modifier != AsyncModifier::None {
            let texto = match modifier {
                AsyncModifier::Async => "async",
                AsyncModifier::AsyncStar => "async*",
                _ => "sync*",
            };
            let span = self.tokens[antes].span;
            self.error_at(codigos::parser::PRIMARY_CONSTRUCTOR_BODY_WITH_MODIFIER, span, &[texto]);
            body_node += if texto.ends_with('*') { 2 } else { 1 };
        }
        if let Some(tok) = self.tokens.get(body_node) {
            self.primary_bodies.insert(t.span.start, tok.span);
        }
        Ok(MemberKind::Constructor(Constructor {
            external: false,
            const_: false,
            factory: false,
            class_name: Name {
                sym: self.interner.intern("this"),
                span: t.span,
            },
            name: None,
            parameters: Vec::new().into_boxed_slice(),
            initializers: initializers.into_boxed_slice(),
            redirect: None,
            body,
            parte_primaria: true,
        }))
    }

    /// O que segue o nome de um construtor: parâmetros, redirecionamento
    /// de factory ou lista de inicialização, e o corpo.
    fn parse_constructor_rest(
        &mut self,
        mods: Modifiers,
        factory: bool,
        class_name: Name,
        name: Option<Name>,
    ) -> PResult<MemberKind> {
        if !factory {
            self.params_owner = self.parameter_owner(mods, false);
        }
        let parameters = self.parse_formal_parameters()?;
        // `_buildConstructorDeclaration` do AstBuilder (`ast_builder.dart:5911-5920`):
        // construtor `external` (não factory), um erro por `this.x`, no `this`.
        if mods.external && !factory {
            let fonte = self.source;
            for p in parameters.iter().filter(|p| p.this_) {
                let until = p.name.map_or(p.span.end, |n| n.span.start);
                if let Some(i) = fonte.get(p.span.start..until).and_then(|s| s.rfind("this")) {
                    let start_ = p.span.start + i;
                    self.error_at(codigos::parser::EXTERNAL_CONSTRUCTOR_WITH_FIELD_INITIALIZERS, Span { start: start_, end: start_ + 4 }, &[]);
                }
            }
        }
        let mut initializers = Vec::new();
        let mut redirect = None;
        // `parseFactoryMethod` (`parser_impl.dart:5139-5145`): o modificador
        // do corpo de uma factory é `NON_SYNC_FACTORY`, no token
        // `async`/`sync`, antes do teste do `=` — vale também na factory
        // redirecionadora, que não tem corpo (ali o modificador é só pulado).
        if factory && (self.at_ident("async") || (self.at_ident("sync") && self.at_op_at(1, Op::Star))) {
            let span = self.span();
            self.error_at(codigos::compile_time_error::NON_SYNC_FACTORY, span, &[]);
            let modifier_tokens_ = if self.at_ident("sync") || self.at_op_at(1, Op::Star) { 2 } else { 1 };
            if self.at_op_at(modifier_tokens_, Op::Assign) {
                for _ in 0..modifier_tokens_ {
                    self.advance();
                }
            }
        }
        let body = if self.at_op(Op::Assign) {
            let eq = self.advance();
            // `parseFactoryMethod` (`parser_impl.dart:5146-5149`).
            if factory && mods.external {
                self.error_at(codigos::parser::EXTERNAL_FACTORY_REDIRECTION, eq.span, &[]);
            }
            if !factory {
                // `Foo() = Bar;` sem `factory`: o alvo é lido normalmente e
                // só o `=` é denunciado (fasta 3.6.2, sem cascata).
                self.error_at(
                    codigos::parser::REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR,
                    eq.span,
                    &[],
                );
            }
            let rstart = self.span();
            let ty = self.parse_type()?;
            let constructor = if self.eat_op(Op::Dot) {
                Some(self.identifier_or_new()?)
            } else {
                None
            };
            redirect = Some(RedirectTarget {
                span: self.span_from(rstart),
                ty,
                constructor,
            });
            self.ensure_semicolon()?;
            FunctionBody::Empty
        } else {
            // `parseMethod` (`parser_impl.dart:5030`): construtor `external`
            // com lista de inicialização é
            // `EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER` no `:`.
            if mods.external && !factory && self.at_op(Op::Colon) {
                self.erro(codigos::parser::EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER, &[]);
            }
            if self.eat_op(Op::Colon) {
                loop {
                    initializers.push(self.parse_initializer()?);
                    if !self.eat_op(Op::Comma) {
                        break;
                    }
                }
            }
            let body_start = self.pos;
            let (modifier, body) = self.parse_function_body()?;
            // `_checkForInvalidModifierOnBody` (`error_verifier.dart:4044-4053`):
            // construtor (gerador ou factory) com `async`, `async*` ou
            // `sync*`, no primeiro token do modificador; o argumento é o
            // texto dele (`async` também para `async*`).
            if modifier != AsyncModifier::None {
                let span = self.tokens[body_start].span;
                let word = if modifier == AsyncModifier::SyncStar { "sync" } else { "async" };
                self.error_at(codigos::compile_time_error::INVALID_MODIFIER_ON_CONSTRUCTOR, span, &[word]);
            }
            self.check_external_body(mods.external, factory, body_start, &body, None);
            // `parseFactoryMethod`: só a factory `external` dispensa o corpo;
            // o construtor gerador segue a regra dos métodos (`parseMethod`).
            let allows = if factory { mods.external } else { Self::allows_abstract(mods, modifier, false) };
            self.check_empty_body(&body, allows);
            if mods.const_ && !factory {
                let delimiter = match body {
                    FunctionBody::Block(_) => Some(Op::LBrace),
                    FunctionBody::Expression(_) => Some(Op::Arrow),
                    FunctionBody::Empty | FunctionBody::Native(_) => None,
                };
                if let Some(op) = delimiter {
                    if let Some(span) = self.tokens[body_start..self.pos]
                        .iter()
                        .find(|t| t.kind == Kind::Op(op))
                        .map(|t| t.span)
                    {
                        self.error_at(codigos::parser::CONST_CONSTRUCTOR_WITH_BODY, span, &[]);
                    }
                }
            }
            body
        };
        // O SDK declara factories constantes `external` (por exemplo
        // `bool.fromEnvironment`); o ErrorVerifier só aplica `constFactory`
        // ao ramo sem `externalKeyword`.
        if factory && !mods.external && redirect.is_none() {
            if let Some(span) = mods.const_span {
                self.error_at(codigos::parser::CONST_FACTORY, span, &[]);
            }
        }
        Ok(MemberKind::Constructor(Constructor {
            external: mods.external,
            const_: mods.const_,
            factory,
            class_name,
            name,
            parameters: parameters.into_boxed_slice(),
            initializers: initializers.into_boxed_slice(),
            redirect,
            body,
            parte_primaria: false,
        }))
    }

    /// Uma entrada da lista de inicializadores: `super(...)`, `super.n(...)`,
    /// `this(...)`/`this.n(...)` (redirecionamento), `this.x = e`, `x = e`
    /// ou `assert(c, m)`.
    fn parse_initializer(&mut self) -> PResult<Initializer> {
        // `parseInitializers` do fasta desliga `mayParseFunctionExpressions`.
        let salvo = std::mem::replace(&mut self.without_named_function, true);
        let lido = self.parse_initializer_inner();
        self.without_named_function = salvo;
        lido
    }

    fn parse_initializer_inner(&mut self) -> PResult<Initializer> {
        let start = self.span();
        if self.eat_kw(Keyword::Super) {
            // `super?.x()`: `INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER`
            // no `?.` (`parseSuperExpression`), lido como `.`.
            if self.at_op(Op::QuestionDot) {
                let span = self.span();
                self.error_at(codigos::parser::INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER, span, &[]);
            }
            let mut via_primary = false;
            let constructor = if self.eat_op(Op::Dot) || self.eat_op(Op::QuestionDot) {
                // `parseSuperInitializerExpression`: o `ensureIdentifier`
                // (`expressionContinuation`) e, no `parseInitializerExpressionRest`,
                // a releitura como expressão. Uma palavra reservada leva os
                // dois erros (`super.const()`: a palavra como nome e, relida
                // pela `parsePrimary`, o `const ()` recusado como nome).
                if self.at_kw(Keyword::New) || self.at_identifier() {
                    Some(self.identifier_or_new()?)
                } else if matches!(self.kind(), Kind::Keyword(_)) && !self.looks_like_statement_start(self.pos) {
                    let texto = self.text().to_string();
                    let span = self.span();
                    self.error_at(codigos::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, span, &[&texto]);
                    via_primary = true;
                    Some(self.member_name(true)?)
                } else {
                    Some(self.member_name(false)?)
                }
            } else {
                None
            };
            // `super` ou `super.x` sem `(` nem `=`: `EXPECTED_TOKEN` (`(`) no
            // token seguinte e parênteses sintéticos (`insertParens`, em
            // `parseSuperInitializerExpression`).
            if !via_primary && !self.at_op(Op::LParen) && !self.at_op(Op::Assign) {
                self.erro(codigos::parser::EXPECTED_TOKEN, &["("]);
                let s = self.span().start;
                return Ok(Initializer::Super {
                    span: self.span_from(start),
                    constructor,
                    arguments: crate::ast::Arguments { span: Span { start: s, end: s }, type_args: Box::default(), args: Box::default() },
                });
            }
            // `buildInitializer`: relido pela `parsePrimary`, `super.x` sem
            // argumentos não é chamada do construtor da superclasse
            // (`INVALID_SUPER_IN_INITIALIZER`).
            if via_primary && !self.at_op(Op::LParen) {
                self.error_at(codigos::parser::INVALID_SUPER_IN_INITIALIZER, Span { start: start.start, end: start.start + 5 }, &[]);
                self.skip_rest_of_initializer();
                let s = self.span().start;
                // Não é invocação de construtor no `AstBuilder`: sem o nome,
                // para a análise não procurar o construtor `const`.
                let _ = constructor;
                return Ok(Initializer::Super {
                    span: self.span_from(start),
                    constructor: None,
                    arguments: crate::ast::Arguments { span: Span { start: s, end: s }, type_args: Box::default(), args: Box::default() },
                });
            }
            let arguments = self.parse_arguments()?;
            // `buildInitializer`: `super(…)`/`super.n(…)` seguido de mais
            // seletores não é chamada do construtor da superclasse:
            // `INVALID_SUPER_IN_INITIALIZER` no `super` (o nó vira a chamada
            // com a última lista de argumentos).
            if self.selector_after_initializer() {
                self.error_at(codigos::parser::INVALID_SUPER_IN_INITIALIZER, Span { start: start.start, end: start.start + 5 }, &[]);
                self.skip_rest_of_initializer();
            }
            return Ok(Initializer::Super {
                span: self.span_from(start),
                constructor,
                arguments,
            });
        }
        if self.eat_kw(Keyword::This) {
            if self.eat_op(Op::Dot) {
                let name = self.identifier_or_new()?;
                if self.at_op(Op::LParen) {
                    let arguments = self.parse_arguments()?;
                    if self.selector_after_initializer() {
                        self.error_at(codigos::parser::INVALID_THIS_IN_INITIALIZER, Span { start: start.start, end: start.start + 4 }, &[]);
                        self.skip_rest_of_initializer();
                    }
                    self.check_redirecting_body();
                    return Ok(Initializer::Redirect {
                        span: self.span_from(start),
                        constructor: Some(name),
                        arguments,
                    });
                }
                if !self.at_op(Op::Assign) {
                    return self.initializer_without_assignment(start);
                }
                self.expect_op(Op::Assign)?;
                let value = self.parse_expression()?;
                return Ok(Initializer::Field {
                    span: self.span_from(start),
                    this_: true,
                    name,
                    value,
                });
            }
            if !self.at_op(Op::LParen) {
                // `this` sem `.` nem `(`: `EXPECTED_TOKEN` (`.`) no que segue
                // (`parseInitializer`, `parser_impl.dart:4098`).
                self.expected_error(".");
                return self.initializer_without_assignment(start);
            }
            let arguments = self.parse_arguments()?;
            if self.selector_after_initializer() {
                self.error_at(codigos::parser::INVALID_THIS_IN_INITIALIZER, Span { start: start.start, end: start.start + 4 }, &[]);
                self.skip_rest_of_initializer();
            }
            self.check_redirecting_body();
            return Ok(Initializer::Redirect {
                span: self.span_from(start),
                constructor: None,
                arguments,
            });
        }
        if self.eat_kw(Keyword::Assert) {
            self.expect_op(Op::LParen)?;
            // `parseAssert` religa as funções literais.
            let condition = self.with_functions(|p| p.parse_expression())?;
            let message = if self.eat_op(Op::Comma) && !self.at_op(Op::RParen) {
                Some(self.with_functions(|p| p.parse_expression())?)
            } else {
                None
            };
            self.eat_op(Op::Comma);
            self.expect_op(Op::RParen)?;
            return Ok(Initializer::Assert {
                span: self.span_from(start),
                condition,
                message,
            });
        }
        let name = self.expect_identifier()?;
        self.expect_op(Op::Assign)?;
        let value = self.parse_expression()?;
        Ok(Initializer::Field {
            span: self.span_from(start),
            this_: false,
            name,
            value,
        })
    }

    /// `parseInitializer` (`parser_impl.dart:4089`): `this(...)` seguido de
    /// `{` ou `=>` é `REDIRECTING_CONSTRUCTOR_WITH_BODY` nele.
    fn check_redirecting_body(&mut self) {
        if self.at_op(Op::LBrace) || self.at_op(Op::Arrow) {
            self.erro(codigos::parser::REDIRECTING_CONSTRUCTOR_WITH_BODY, &[]);
        }
    }

    /// A recuperação final de `parseInitializer` (`parser_impl.dart:4134`):
    /// `this.x`/`this` sem atribuição é `MISSING_ASSIGNMENT_IN_INITIALIZER` no
    /// `this`, e o inicializador vira `<sintético> = expressão`, com a
    /// expressão lida desde o `this`.
    /// Depois dos argumentos de `super(…)`/`this(…)` vem outro seletor
    /// (`.`, `?.`, `..`, `?..`, `[`, `(`).
    fn selector_after_initializer(&self) -> bool {
        matches!(
            self.kind(),
            Kind::Op(Op::Dot | Op::QuestionDot | Op::DotDot | Op::QuestionDotDot | Op::LBracket | Op::LParen)
        )
    }

    /// Pula o resto de um inicializador (os seletores, com os grupos
    /// casados) até a `,` seguinte, o corpo (`{`, `=>`), o `;` ou o fim.
    fn skip_rest_of_initializer(&mut self) {
        while !self.at_eof() {
            match self.kind() {
                Kind::Op(Op::Comma | Op::LBrace | Op::Semicolon | Op::Arrow) => break,
                Kind::Op(Op::LParen | Op::LBracket) => match self.matching_close(self.pos) {
                    Some(f) => self.pos = f + 1,
                    None => break,
                },
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn initializer_without_assignment(&mut self, start: Span) -> PResult<Initializer> {
        self.error_at(codigos::parser::MISSING_ASSIGNMENT_IN_INITIALIZER, start, &[]);
        self.pos = self.tokens.iter().position(|t| t.span.start == start.start).unwrap_or(self.pos);
        let value = self.parse_expression()?;
        let name = self.name_from("", Span { start: start.start, end: start.start });
        Ok(Initializer::Field {
            span: self.span_from(start),
            this_: false,
            name,
            value,
        })
    }

    // -----------------------------------------------------------------------
    // Corpos de função
    // -----------------------------------------------------------------------

    /// O parser oficial relata função de topo no token `external`, mas membro
    /// e construtor no `{` do bloco ou no `=>` (`parseTopLevelMethod` e
    /// `parseMethod`/`parseFactoryMethod` do fasta).
    /// O intervalo desde `start_offset` começa antes do modificador `async` e só
    /// inclui tokens desta função, então o primeiro delimitador é o corpo.
    fn check_external_body(&mut self, external: bool, factory: bool, start_offset: usize, body: &FunctionBody, external_topo: Option<Span>) {
        if !external {
            return;
        }
        let delimiter = match body {
            FunctionBody::Block(_) => Op::LBrace,
            FunctionBody::Expression(_) => Op::Arrow,
            FunctionBody::Empty | FunctionBody::Native(_) => return,
        };
        let span = self.tokens[start_offset..self.pos]
            .iter()
            .find(|t| t.kind == Kind::Op(delimiter))
            .map(|t| t.span);
        if let Some(span) = external_topo.or(span) {
            let code = if factory {
                codigos::parser::EXTERNAL_FACTORY_WITH_BODY
            } else {
                codigos::parser::EXTERNAL_METHOD_WITH_BODY
            };
            self.error_at(code, span, &[]);
        }
    }

    /// O fim do tipo que precede `factory nome` num membro, se houver.
    /// `void` seguido de `new` e de `(` ou de um identificador
    /// (`static void new() {}`): o fim do tipo, no `new`. Só o `void`: o
    /// `computeType` do 3.13.4 só aceita os outros tipos quando o token
    /// seguinte parece um nome (`looksLikeName`), e o `new` não parece —
    /// `int new() => 1` fica um método de nome inválido
    /// (`EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`), como no 3.6.2.
    fn type_before_new(&self) -> Option<usize> {
        if !self.at_kw(Keyword::Void) {
            return None;
        }
        let end_offset = self.pos + 1;
        let n = 1;
        (self.at_kw_at(n, Keyword::New) && (self.at_op_at(n + 1, Op::LParen) || self.at_identifier_at(n + 1))).then_some(end_offset)
    }

    /// `factory new(` (sem `.` depois do `new`): o 3.13.4 lê um construtor
    /// de fábrica chamado `new`; o 3.6.2, um campo `factory` (o `new` não é
    /// identificador). Fora do modo 3.13, segue o 3.6.2 e anota a
    /// divergência ([`Parser::diverged_313`]).
    fn factory_new_as_constructor(&mut self) -> bool {
        if !(self.at_kw_at(1, Keyword::New) && !self.at_op_at(2, Op::Dot)) {
            return false;
        }
        if self.mode_313 {
            return true;
        }
        self.diverged_313 = true;
        false
    }

    fn type_before_factory(&self) -> Option<usize> {
        if self.at_ident("factory") {
            return None;
        }
        let end_offset = self.skip_type(self.pos)?;
        (end_offset > self.pos && self.kind_of(end_offset) == Kind::Ident && self.text_of(end_offset) == "factory" && self.kind_of(end_offset + 1) == Kind::Ident)
            .then_some(end_offset)
    }

    /// O `allowAbstract` com que o fasta lê o corpo: no topo
    /// (`parseTopLevelMethod`), só `external` dispensa o corpo; num membro
    /// (`parseMethod`), o que não é `static` (ou é `external`) e não tem
    /// modificador `async`/`sync*` (`inPlainSync`).
    fn allows_abstract(mods: Modifiers, modifier: AsyncModifier, topo: bool) -> bool {
        if topo {
            mods.external
        } else {
            (!mods.static_ || mods.external) && modifier == AsyncModifier::None
        }
    }

    /// `parseFunctionBody` do fasta: corpo `;` onde `allowAbstract` é falso
    /// dá `MISSING_FUNCTION_BODY` no `;` (a árvore fica com o corpo vazio).
    fn check_empty_body(&mut self, body: &FunctionBody, allows_abstract: bool) {
        if !allows_abstract && matches!(body, FunctionBody::Empty) && self.pos > 0 {
            let span = self.tokens[self.pos - 1].span;
            self.error_at(codigos::parser::MISSING_FUNCTION_BODY, span, &[]);
        }
    }

    /// `async`/`async*`/`sync*` opcional seguido de `{...}`, `=> e;`, `;` ou
    /// `native ...;`. Ajusta `in_async`/`in_generator` durante o corpo.
    pub(crate) fn parse_function_body(&mut self) -> PResult<(AsyncModifier, FunctionBody)> {
        self.parse_function_body_ex(true)
    }

    /// Como [`Parser::parse_function_body`], mas em expressões de função
    /// (`(x) => x + 1`) não há `;` após a expressão: passe `false`.
    ///
    /// O contexto `async`/generator do corpo é o do modificador lido aqui —
    /// uma função aninhada não herda o do exterior — e o contexto anterior é
    /// restaurado ao sair, mesmo em caso de erro.
    pub(crate) fn parse_function_body_ex(
        &mut self,
        expect_semicolon: bool,
    ) -> PResult<(AsyncModifier, FunctionBody)> {
        let modifier = self.parse_async_modifier();
        let saved = (self.in_async, self.in_generator);
        self.in_async = matches!(modifier, AsyncModifier::Async | AsyncModifier::AsyncStar);
        self.in_generator = matches!(modifier, AsyncModifier::AsyncStar | AsyncModifier::SyncStar);
        let body = self.parse_body_after_modifier(expect_semicolon);
        (self.in_async, self.in_generator) = saved;
        Ok((modifier, body?))
    }

    /// `async`, `async*`, `sync*` (identificadores contextuais; `*` é um
    /// token separado) ou nada.
    fn parse_async_modifier(&mut self) -> AsyncModifier {
        if self.at_ident("async") {
            self.advance();
            if self.eat_op(Op::Star) {
                AsyncModifier::AsyncStar
            } else {
                AsyncModifier::Async
            }
        } else if self.at_ident("sync") && self.at_op_at(1, Op::Star) {
            self.advance();
            self.advance();
            AsyncModifier::SyncStar
        } else {
            AsyncModifier::None
        }
    }

    /// O corpo propriamente dito, após o modificador.
    fn parse_body_after_modifier(&mut self, expect_semicolon: bool) -> PResult<FunctionBody> {
        if self.at_op(Op::LBrace) {
            return Ok(FunctionBody::Block(self.parse_body_block()?));
        }
        if self.eat_op(Op::Arrow) {
            let expr = self.parse_expression()?;
            if expect_semicolon {
                self.ensure_semicolon()?;
            }
            return Ok(FunctionBody::Expression(expr));
        }
        if self.eat_op(Op::Semicolon) {
            return Ok(FunctionBody::Empty);
        }
        if self.eat_ident("native") {
            let name = if self.string_at(0) {
                Some(self.parse_string_literal()?)
            } else {
                None
            };
            self.ensure_semicolon()?;
            return Ok(FunctionBody::Native(name));
        }
        // Recuperação de `parseFunctionBody` (`parser_impl.dart`): `=` ou
        // `return` no lugar de `=>` são `MISSING_FUNCTION_BODY` neles e a
        // expressão é lida como corpo `=>`; uma palavra solta antes de `=>`
        // ou `{` é `UNEXPECTED_TOKEN` e é pulada; qualquer outra coisa é
        // `MISSING_FUNCTION_BODY` nela, com um bloco vazio sintético, sem
        // consumir nada (`ensureBlock`).
        if self.at_op(Op::Assign) || self.at_kw(Keyword::Return) {
            self.erro(codigos::parser::MISSING_FUNCTION_BODY, &[]);
            self.advance();
            let expr = self.parse_expression()?;
            if expect_semicolon {
                self.ensure_semicolon()?;
            }
            return Ok(FunctionBody::Expression(expr));
        }
        if matches!(self.kind(), Kind::Ident | Kind::Keyword(_))
            && matches!(self.kind_at(1), Kind::Op(Op::Arrow | Op::LBrace))
        {
            let texto = self.text().to_string();
            self.erro(codigos::parser::UNEXPECTED_TOKEN, &[&texto]);
            self.advance();
            return self.parse_body_after_modifier(expect_semicolon);
        }
        self.erro(codigos::parser::MISSING_FUNCTION_BODY, &[]);
        let here = self.span().start;
        let empty = Span { start: here, end: here };
        let block = self.ast.push_stmt(crate::ast::Stmt { span: empty, kind: crate::ast::StmtKind::Block(Box::default()) });
        Ok(FunctionBody::Block(block))
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        AsyncModifier, DeclKind, DirectiveKind, FunctionBody, FunctionKind, MemberKind, Name,
    };
    use crate::parser::{Parsed, Parser, parse};
    use dartforge_intern::Interner;

    /// Representação de extension type pelo `AstBuilder.endPrimaryConstructor`
    /// e parâmetros de tipo em operador (conferidos com o `dart analyze`
    /// 3.6.2).
    #[test]
    fn representation_and_operator_with_type_parameters() {
        let fonte = "// @dart = 3.6\n\
                     extension type A(final int x) {}\n\
                     extension type C([int x = 0]) {}\n\
                     extension type D(x) {}\n\
                     extension type E(int x,) {}\n\
                     extension type F(int x, int y) {}\n\
                     extension type G() {}\n\
                     extension type H(this.x) {}\n\
                     class K { bool operator ==<T>(Object o) => true; }\n";
        let mut names = Interner::new();
        let features = crate::features::LibraryFeatures::piso();
        let out = crate::parser::parse_com(fonte, &mut names, features);
        let v: Vec<(&str, &str)> = out
            .diagnostics
            .iter()
            .map(|d| (d.code.map_or("", |c| c.info().nome), &fonte[d.span.start..d.span.end]))
            .collect();
        assert_eq!(
            v,
            [
                ("representation_field_modifier", "final"),
                ("expected_representation_field", "["),
                ("expected_representation_type", "x"),
                ("representation_field_trailing_comma", ","),
                ("multiple_representation_fields", ","),
                ("expected_representation_field", ")"),
                ("expected_representation_field", "this"),
                ("type_parameter_on_operator", "<T>"),
            ]
        );
    }

    /// `MISSING_FUNCTION_BODY` no `;` onde o fasta lê o corpo sem
    /// `allowAbstract` (conferido com o `dart analyze` 3.6.2): função e
    /// acessor de topo sem `external`, membro `static`, membro `async`,
    /// factory sem `external`. Método de instância, construtor gerador e
    /// `external` aceitam `;`.
    #[test]
    fn empty_body_without_allow_abstract() {
        let fonte = "class A {\n  static int get foo;\n  static set foo(int _);\n  static void m();\n  void i();\n  \
                     A();\n  factory A.f();\n  external factory A.g();\n  void a() async;\n  external static void e();\n}\n\
                     void f(int x);\nint get g;\nexternal void h();\n";
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        let lines_: Vec<usize> = out
            .diagnostics
            .iter()
            .map(|d| {
                assert_eq!(d.code, Some(dartforge_diagnostics::codigos::parser::MISSING_FUNCTION_BODY), "{d:?}");
                assert_eq!(&fonte[d.span.start..d.span.end], ";");
                fonte[..d.span.start].matches('\n').count() + 1
            })
            .collect();
        assert_eq!(lines_, [2, 3, 4, 7, 9, 12, 13]);
    }

    fn parse_ok(src: &str, names: &mut Interner) -> Parsed {
        let out = parse(src, names);
        assert!(out.diagnostics.is_empty(), "{src}: {:?}", out.diagnostics);
        out
    }

    /// Como [`parse_ok`], tolerando só os erros de ordem de diretivas (as
    /// amostras juntam `library`, `part` e `part of` numa unidade só).
    fn parse_ok_except_order(src: &str, names: &mut Interner) -> Parsed {
        use dartforge_diagnostics::codigos::parser as c;
        let out = parse(src, names);
        let order = [c::NON_PART_OF_DIRECTIVE_IN_PART, c::DIRECTIVE_AFTER_DECLARATION];
        assert!(
            out.diagnostics.iter().all(|d| d.code.is_some_and(|k| order.contains(&k))),
            "{src}: {:?}",
            out.diagnostics
        );
        out
    }

    fn text(names: &Interner, name: Name) -> &str {
        names.resolve(name.sym)
    }

    fn decl(out: &Parsed, i: usize) -> &DeclKind {
        &out.ast.decl(out.unit.declarations[i]).kind
    }

    fn class(out: &Parsed, i: usize) -> &crate::ast::ClassDecl {
        match decl(out, i) {
            DeclKind::Class(class) => class,
            other => panic!("esperava classe, veio {other:?}"),
        }
    }

    fn member<'a>(out: &'a Parsed, class: &crate::ast::ClassDecl, i: usize) -> &'a MemberKind {
        &out.ast.member(class.members[i]).kind
    }

    /// `extension`, `get`, `set`, `mixin`… são identificadores embutidos: no
    /// topo, depois de um tipo, com parâmetros, são o nome de uma função
    /// (`String extension(String path, [int level = 1])` do `package:path`).
    #[test]
    fn builtin_identifier_as_top_level_function_name() {
        let mut names = Interner::new();
        let out = parse_ok(
            "String extension(String p, [int n = 1]) => p;\nT get<T>(T x) => x;\nint mixin() => 1;\n",
            &mut names,
        );
        for (i, expected_) in ["extension", "get", "mixin"].into_iter().enumerate() {
            let DeclKind::Function(f) = decl(&out, i) else { panic!("esperava função") };
            assert_eq!(text(&names, out.ast.function(*f).name.expect("nome")), expected_);
        }
    }

    #[test]
    fn external_body_marks_block_opening_or_arrow_with_official_code() {
        use dartforge_diagnostics::codigos::parser as c;
        for (src, token, code) in [
            ("external int f() => 1;", "external", c::EXTERNAL_METHOD_WITH_BODY),
            ("class C { external C() {} }", "{}", c::EXTERNAL_METHOD_WITH_BODY),
            ("class C { external factory C.x() => C(); }", "=>", c::EXTERNAL_FACTORY_WITH_BODY),
            ("class C { external factory C.x() {} }", "{}", c::EXTERNAL_FACTORY_WITH_BODY),
            ("class C { external int get v => 1; }", "=>", c::EXTERNAL_METHOD_WITH_BODY),
        ] {
            let mut names = Interner::new();
            let out = parse(src, &mut names);
            let start_offset = src.find(token).unwrap();
            let end_offset = start_offset + if token == "{}" { 1 } else { token.len() };
            assert!(out.diagnostics.iter().any(|d|
                d.code == Some(code) && d.span.start == start_offset && d.span.end == end_offset
            ), "{src}: {:?}", out.diagnostics);
            assert_eq!(out.diagnostics.len(), 1, "{src}: {:?}", out.diagnostics);
        }
        let mut names = Interner::new();
        let out = parse("external int f(); class C { external factory C.x(); }", &mut names);
        assert!(!out.diagnostics.iter().any(|d| matches!(d.code,
            Some(c::EXTERNAL_METHOD_WITH_BODY | c::EXTERNAL_FACTORY_WITH_BODY))));
    }

    #[test]
    fn external_body_matches_two_recorded_oracle_samples() {
        use dartforge_diagnostics::codigos::parser as c;
        for (src, code, start_offset, message, correcao) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/constructor_body/ConstructorBody__class_secondaryConstru_0c2b43e4.dart")),
                c::EXTERNAL_METHOD_WITH_BODY,
                25,
                "An external or native method can't have a body.",
                None,
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/constructor_body/ConstructorBody__class_secondaryConstru_42cae48d.dart")),
                c::EXTERNAL_FACTORY_WITH_BODY,
                39,
                "External factories can't have a body.",
                Some("Try removing the body of the factory, or removing the keyword 'external'."),
            ),
        ] {
            let mut names = Interner::new();
            let out = parse(src, &mut names);
            assert!(out.diagnostics.iter().any(|d|
                d.code == Some(code) && d.span.start == start_offset && d.span.end == start_offset + 2
                    && d.message == message && d.correcao().as_deref() == correcao
            ), "{src}: {:?}", out.diagnostics);
        }
    }

    #[test]
    fn external_top_level_function_points_to_external_like_fasta() {
        use dartforge_diagnostics::codigos::parser as c;
        let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/executable_body/ExecutableBody__topLevelFunction_extern_0464d54b.dart"));
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        assert!(out.diagnostics.iter().any(|d|
            d.code == Some(c::EXTERNAL_METHOD_WITH_BODY)
                && d.span.start == 43 && d.span.end == 51
                && d.message == "An external or native method can't have a body."
        ), "{out:?}");
    }

    #[test]
    fn abstract_static_field_without_augmentations_points_to_abstract() {
        use crate::features::{Feature, LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/executable_body/ExecutableBody__class_staticField_abstr_5d2b9f54.dart"));
        let span = fonte.find("abstract int foo").unwrap();
        let mut names = Interner::new();
        let without_feature = parse_com(fonte, &mut names, LibraryFeatures::new(LanguageVersion::PISO, &[]));
        assert!(without_feature.diagnostics.iter().any(|d|
            d.code == Some(c::ABSTRACT_STATIC_FIELD)
                && d.span.start == span && d.span.end == span + "abstract".len()
                && d.message == "Static fields can't be declared 'abstract'."
                && d.correcao().as_deref() == Some("Try removing the 'abstract' or 'static' keyword.")
        ), "{:?}", without_feature.diagnostics);

        let with_feature = parse_com(fonte, &mut names, LibraryFeatures::new(LanguageVersion::PISO, &[Feature::Augmentations]));
        assert!(!with_feature.diagnostics.iter().any(|d| d.code == Some(c::ABSTRACT_STATIC_FIELD)));
    }

    #[test]
    fn const_constructor_body_points_to_delimiter_like_analyzer() {
        use dartforge_diagnostics::codigos::parser as c;
        for (fonte, start_offset, end_offset) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/constructor_body/ConstructorBody__class_secondaryConstru_482771f3.dart")),
                41,
                43,
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/constructor_body/ConstructorBody__class_secondaryConstru_4a7cd473.dart")),
                50,
                52,
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/constructor_body/ConstructorBody__class_secondaryConstru_79e52946.dart")),
                50,
                51,
            ),
        ] {
            let mut names = Interner::new();
            let parsed = parse(fonte, &mut names);
            assert!(parsed.diagnostics.iter().any(|d|
                d.code == Some(c::CONST_CONSTRUCTOR_WITH_BODY)
                    && d.span.start == start_offset && d.span.end == end_offset
                    && d.message == "Const constructors can't have a body."
                    && d.correcao().as_deref() == Some("Try removing either the 'const' keyword or the body.")
            ), "{fonte}: {:?}", parsed.diagnostics);
        }

        let mut names = Interner::new();
        let parsed = parse("class C { const C(); const factory C.x() => C(); }", &mut names);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_CONSTRUCTOR_WITH_BODY)));
    }

    #[test]
    fn const_factory_without_redirect_points_to_const() {
        use dartforge_diagnostics::codigos::parser as c;
        for (fonte, start_offset) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/constructor_body/ConstructorBody__class_secondaryConstru_6ecedb96.dart")),
                12,
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/constructor_body/ConstructorBody__class_secondaryConstru_c135c0a2.dart")),
                25,
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/constructor_body/ConstructorBody__class_secondaryConstru_d7023785.dart")),
                55,
            ),
        ] {
            let mut names = Interner::new();
            let parsed = parse(fonte, &mut names);
            assert!(parsed.diagnostics.iter().any(|d|
                d.code == Some(c::CONST_FACTORY) && d.span.start == start_offset && d.span.end == start_offset + 5
                    && d.message == "Only redirecting factory constructors can be declared to be 'const'."
                    && d.correcao().as_deref() == Some("Try removing the 'const' keyword, or replacing the body with '=' followed by a valid target.")
            ), "{fonte}: {:?}", parsed.diagnostics);
        }

        let mut names = Interner::new();
        let parsed = parse("class A { const A(); const factory A.named() = A; }", &mut names);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_FACTORY)));

        // Declarações do SDK 3.6.2: lib/core/bool.dart e lib/core/int.dart.
        let sdk = "class bool { external const factory bool.fromEnvironment(String name, {bool defaultValue = false}); external const factory bool.hasEnvironment(String name); } class int { external const factory int.fromEnvironment(String name, {int defaultValue = 0}); }";
        let parsed = parse(sdk, &mut names);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_FACTORY)), "{:?}", parsed.diagnostics);

        let corpus = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/linguagem/const/native_factory_test.dart"));
        let parsed = parse(corpus, &mut names);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_FACTORY)), "{:?}", parsed.diagnostics);
    }

    // -- Independentes dos outros módulos -----------------------------------

    #[test]
    fn script_tag_and_library() {
        let mut names = Interner::new();
        let out = parse_ok("#!/usr/bin/env dart\nlibrary a.b.c;\n", &mut names);
        assert!(out.unit.script_tag.is_some());
        assert_eq!(out.unit.directives.len(), 1);
        match &out.unit.directives[0].kind {
            DirectiveKind::Library { name } => {
                let partes: Vec<_> = name.iter().map(|n| text(&names, *n)).collect();
                assert_eq!(partes, ["a", "b", "c"]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unnamed_library_and_dotted_part_of() {
        let mut names = Interner::new();
        let out = parse_ok_except_order("library; part of a.b;", &mut names);
        assert_eq!(out.unit.directives.len(), 2);
        assert!(
            matches!(&out.unit.directives[0].kind, DirectiveKind::Library { name } if name.is_empty())
        );
        assert!(matches!(
            &out.unit.directives[1].kind,
            DirectiveKind::PartOf { uri: None, name } if name.len() == 2
        ));
    }

    #[test]
    fn class_modifiers() {
        let mut names = Interner::new();
        let src = "class A {} abstract class B {} base class C {} interface class D {} \
                   final class E {} sealed class F {} mixin class G {} \
                   abstract base class H {} abstract interface class I {} \
                   abstract mixin class J {} base mixin class K {} abstract final class L {}";
        let out = parse_ok(src, &mut names);
        assert_eq!(out.unit.declarations.len(), 12);
        let m = |i: usize| class(&out, i).modifiers;
        assert!(!m(0).abstract_ && !m(0).base && !m(0).mixin);
        assert!(m(1).abstract_);
        assert!(m(2).base);
        assert!(m(3).interface);
        assert!(m(4).final_);
        assert!(m(5).sealed);
        assert!(m(6).mixin);
        assert!(m(7).abstract_ && m(7).base);
        assert!(m(8).abstract_ && m(8).interface);
        assert!(m(9).abstract_ && m(9).mixin);
        assert!(m(10).base && m(10).mixin);
        assert!(m(11).abstract_ && m(11).final_);
        assert_eq!(text(&names, class(&out, 11).name), "L");
        assert!(!class(&out, 0).mixin_application);
    }

    #[test]
    fn mixins() {
        let mut names = Interner::new();
        let out = parse_ok("mixin M {} base mixin N {}", &mut names);
        match decl(&out, 0) {
            DeclKind::Mixin(m) => assert!(!m.base && text(&names, m.name) == "M"),
            other => panic!("{other:?}"),
        }
        match decl(&out, 1) {
            DeclKind::Mixin(m) => assert!(m.base && text(&names, m.name) == "N"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn enums_without_arguments() {
        let mut names = Interner::new();
        let src = "enum A { a, b } enum B { a, b, } enum C { a; } enum D { a.named, b; var x; } enum E { a.new }";
        let out = parse_ok(src, &mut names);
        let e = |i: usize| match decl(&out, i) {
            DeclKind::Enum(e) => e,
            other => panic!("{other:?}"),
        };
        assert_eq!(e(0).constants.len(), 2);
        assert_eq!(e(1).constants.len(), 2);
        assert_eq!(e(2).constants.len(), 1);
        assert!(e(2).members.is_empty());
        assert_eq!(e(3).constants.len(), 2);
        assert_eq!(
            text(&names, e(3).constants[0].constructor.unwrap()),
            "named"
        );
        assert_eq!(e(3).members.len(), 1);
        assert_eq!(text(&names, e(4).constants[0].constructor.unwrap()), "new");
    }

    #[test]
    fn fields_with_modifiers() {
        let mut names = Interner::new();
        let src = "class A { var x; static var y, z; late var w; external var e; abstract var f; covariant var c; }";
        let out = parse_ok(src, &mut names);
        let a = class(&out, 0);
        assert_eq!(a.members.len(), 6);
        let field = |i: usize| match member(&out, a, i) {
            MemberKind::Field(list) => list,
            other => panic!("{other:?}"),
        };
        assert!(field(0).var_ && field(0).variables.len() == 1);
        assert!(field(1).static_ && field(1).variables.len() == 2);
        assert_eq!(text(&names, field(1).variables[1].name), "z");
        assert!(field(2).late);
        assert!(field(3).external);
        assert!(field(4).abstract_);
        assert!(field(5).covariant);
    }

    #[test]
    fn abstract_and_native_getters() {
        let mut names = Interner::new();
        let src = "class A { get x; static get y => 1; external get z; get w native; }";
        let out = parse_ok(src, &mut names);
        let a = class(&out, 0);
        assert_eq!(a.members.len(), 4);
        let getter = |i: usize| match member(&out, a, i) {
            MemberKind::Method(id) => out.ast.function(*id),
            other => panic!("{other:?}"),
        };
        assert_eq!(getter(0).kind, FunctionKind::Getter);
        assert!(getter(0).parameters.is_none());
        assert!(matches!(getter(0).body, FunctionBody::Empty));
        assert!(getter(1).static_);
        assert!(getter(2).external);
        assert!(matches!(getter(3).body, FunctionBody::Native(None)));
        assert_eq!(text(&names, getter(3).name.unwrap()), "w");
    }

    #[test]
    fn untyped_top_level_variables() {
        let mut names = Interner::new();
        let out = parse_ok("var x, y; late var z;", &mut names);
        assert_eq!(out.unit.declarations.len(), 2);
        match decl(&out, 0) {
            DeclKind::Variables(list) => assert_eq!(list.variables.len(), 2),
            other => panic!("{other:?}"),
        }
        match decl(&out, 1) {
            DeclKind::Variables(list) => assert!(list.late && list.var_),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn metadata_without_arguments() {
        let mut names = Interner::new();
        let out = parse_ok("@a @b.c @d.e.f class A {} @override var x;", &mut names);
        let a = out.ast.decl(out.unit.declarations[0]);
        assert_eq!(a.metadata.len(), 3);
        assert_eq!(a.metadata[0].name.len(), 1);
        assert_eq!(a.metadata[1].name.len(), 2);
        assert_eq!(a.metadata[2].name.len(), 3);
        assert_eq!(text(&names, a.metadata[2].name[2]), "f");
        assert!(a.metadata.iter().all(|m| m.arguments.is_none()));
        let x = out.ast.decl(out.unit.declarations[1]);
        assert_eq!(text(&names, x.metadata[0].name[0]), "override");
    }

    #[test]
    fn recovery_at_top_level() {
        let mut names = Interner::new();
        let out = parse(
            "class {} class A {} enum {} class B {} class {}",
            &mut names,
        );
        assert_eq!(out.diagnostics.len(), 3, "{:?}", out.diagnostics);
        // `class {}` vira classe de nome sintético vazio (como no fasta);
        // `enum {}` ainda cai na recuperação de topo.
        assert_eq!(out.unit.declarations.len(), 4);
        assert_eq!(text(&names, class(&out, 0).name), "");
        assert_eq!(text(&names, class(&out, 1).name), "A");
        assert_eq!(text(&names, class(&out, 2).name), "B");
    }

    #[test]
    fn recovery_skips_the_broken_declaration_body() {
        let mut names = Interner::new();
        let out = parse("class A B { var x; } class C {}", &mut names);
        // `ensureBlock`: `EXPECTED_CLASS_BODY` no `A` e corpo sintético; `B
        // { … }` é função sem parâmetros (`MISSING_FUNCTION_PARAMETERS`).
        assert_eq!(out.diagnostics.len(), 2, "{:?}", out.diagnostics);
        assert_eq!(out.unit.declarations.len(), 3);
        assert_eq!(text(&names, class(&out, 0).name), "A");
        assert_eq!(text(&names, class(&out, 2).name), "C");
    }

    #[test]
    fn recovery_between_members() {
        let mut names = Interner::new();
        let out = parse(
            "class A { var x; ; var y; ) var z; } class B {}",
            &mut names,
        );
        assert_eq!(out.diagnostics.len(), 2, "{:?}", out.diagnostics);
        assert_eq!(out.unit.declarations.len(), 2);
        assert_eq!(class(&out, 0).members.len(), 3);
    }

    #[test]
    fn stray_top_level_tokens() {
        let mut names = Interner::new();
        let out = parse("; } class A {}", &mut names);
        assert_eq!(out.diagnostics.len(), 2, "{:?}", out.diagnostics);
        assert_eq!(out.unit.declarations.len(), 1);
    }

    /// Membro inválido no estilo do fasta: um erro por token, sem engolir o
    /// membro válido seguinte (sondado no SDK 3.6.2 local: `42` dá um único
    /// `EXPECTED_CLASS_MEMBER` e `int x` sobrevive; idem `;` e `)`).
    #[test]
    fn fasta_member_stray_token_skips_one_and_keeps_valid() {
        use dartforge_diagnostics::codigos::parser as c;

        for (garbage, start_offset, end_offset) in [("42", 12, 14), (";", 12, 13), (")", 12, 13)] {
            let fonte = format!("class C {{\n  {garbage}\n  int x = 1;\n}}\n");
            let mut names = Interner::new();
            let out = parse(&fonte, &mut names);
            assert_eq!(out.diagnostics.len(), 1, "{fonte}: {:?}", out.diagnostics);
            let single = &out.diagnostics[0];
            assert_eq!(single.code, Some(c::EXPECTED_CLASS_MEMBER), "{fonte}");
            assert_eq!((single.span.start, single.span.end), (start_offset, end_offset), "{fonte}");
            assert_eq!(out.unit.declarations.len(), 1, "{fonte}");
            let campo = match member(&out, class(&out, 0), 0) {
                MemberKind::Field(list) => list,
                other => panic!("{fonte}: {other:?}"),
            };
            assert_eq!(text(&names, campo.variables[0].name), "x", "{fonte}");
        }
    }

    /// O `;` que falta é apontado no token anterior (`1`) e inserido, como
    /// no `ensureSemicolon` do fasta: `x` fica, `garbage` vira um campo sem
    /// tipo (`MISSING_CONST_FINAL_VAR_OR_TYPE`) e o `var y` não é engolido
    /// (conferido com o `dart analyze` 3.6.2).
    #[test]
    fn fasta_member_boundary_does_not_swallow_valid() {
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class C {\n  var x = 1 garbage;\n  var y = 2;\n}\n";
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        assert!(
            out.diagnostics.iter().any(|d|
                d.code == Some(c::EXPECTED_TOKEN)
                    && d.span.start == 20
                    && d.span.end == 21
                    && d.message == "Expected to find ';'."),
            "{fonte}: {:?}",
            out.diagnostics
        );
        let class = class(&out, 0);
        let field_names: Vec<_> = class.members.iter().map(|m| match &out.ast.member(*m).kind {
            MemberKind::Field(list) => text(&names, list.variables[0].name).to_string(),
            other => panic!("{fonte}: {other:?}"),
        }).collect();
        assert_eq!(field_names, ["x", "garbage", "y"], "{fonte}: {:?}", out.diagnostics);
    }

    /// Membros válidos de todas as formas nunca tocam a recuperação.
    #[test]
    fn valid_member_does_not_touch_recovery() {
        let mut names = Interner::new();
        let out = parse_ok(
            "class C { int x = 1; static var y; C(); C.nomeado(this.x); factory C.f() => C(); get g => x; set s(int v) {} int m() => 1; }",
            &mut names,
        );
        assert_eq!(class(&out, 0).members.len(), 8);
    }

    /// Sem o recurso, `augment` vira o tipo de um campo `int` (o `;` que
    /// falta é inserido depois dele, como no `ensureSemicolon` do fasta) e
    /// a cauda `get foo => 0;` é retida como membro — o `dart analyze` 3.6.2
    /// dá `expected_token` em `int` e `undefined_class` em `augment`.
    #[test]
    fn member_speculative_recovery_keeps_valid_tail() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class C {\n  augment int get foo => 0;\n}\n";
        let mut names = Interner::new();
        let out = parse_com(fonte, &mut names, LibraryFeatures::new(LanguageVersion::PISO, &[]));
        assert_eq!(out.diagnostics.len(), 1, "{fonte}: {:?}", out.diagnostics);
        assert_eq!(out.diagnostics[0].code, Some(c::EXPECTED_TOKEN));
        let class = class(&out, 0);
        assert_eq!(class.members.len(), 2, "{fonte}: {:?}", out.diagnostics);
        assert_eq!((out.diagnostics[0].span.start, out.diagnostics[0].span.end), (20, 23));
        match member(&out, class, 1) {
            MemberKind::Method(id) => {
                assert_eq!(text(&names, out.ast.function(*id).name.unwrap()), "foo");
            }
            other => panic!("{fonte}: {other:?}"),
        }
    }

    /// `extension type A {}`: `MISSING_PRIMARY_CONSTRUCTOR` no nome (o
    /// último token lido) e o corpo lido normalmente; `A.n {}`,
    /// `MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS` em `n` (fasta 3.6.2).
    #[test]
    fn extension_type_without_primary_constructor() {
        use dartforge_diagnostics::codigos::parser as c;
        for (fonte, code, start_offset, end_offset, members_) in [
            ("extension type A<T> { int get x => 0; }", c::MISSING_PRIMARY_CONSTRUCTOR, 18, 19, 1),
            ("extension type A.n { }", c::MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS, 17, 18, 0),
        ] {
            let mut names = Interner::new();
            let out = parse(fonte, &mut names);
            assert_eq!(out.diagnostics.len(), 1, "{fonte}: {:?}", out.diagnostics);
            let d = &out.diagnostics[0];
            assert_eq!((d.code, d.span.start, d.span.end), (Some(code), start_offset, end_offset), "{fonte}");
            match &out.ast.decl(out.unit.declarations[0]).kind {
                DeclKind::ExtensionType(e) => assert_eq!(e.members.len(), members_, "{fonte}"),
                other => panic!("{fonte}: {other:?}"),
            }
        }
    }

    /// Tentativa com tipo de retorno: `augment C.named() : ...` lê o tipo,
    /// denuncia `constructor_with_return_type` nele e lê o construtor com
    /// o redirect — sem cascata (sondado no SDK 3.6.2 local: só o erro no
    /// `augment` mais o semântico de redirect).
    #[test]
    fn member_speculative_recovery_discards_cascade() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class C {\n  augment C.named() : this.missing();\n}\n";
        let mut names = Interner::new();
        let out = parse_com(fonte, &mut names, LibraryFeatures::new(LanguageVersion::PISO, &[]));
        let codigos: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            codigos,
            [(Some(c::CONSTRUCTOR_WITH_RETURN_TYPE), (12, 19))],
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert_eq!(class(&out, 0).members.len(), 1, "{fonte}: {:?}", out.diagnostics);
    }

    /// Token solto pula um: `[` não engole o resto (`int? a]` continua
    /// declaração e falta `;` no nome, cada fecho solto é denunciado e o
    /// `;` final é inesperado; o `var b` do fim sobrevive). Sequência
    /// sondada no SDK 3.6.2 local, byte a byte nos 5 diagnósticos.
    #[test]
    fn fasta_top_level_stray_token_skips_one() {
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "[int? a]);\nvar b = 0;\n";
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        let codigos: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            codigos,
            [
                (Some(c::EXPECTED_EXECUTABLE), (0, 1)),
                (Some(c::EXPECTED_TOKEN), (6, 7)),
                (Some(c::EXPECTED_EXECUTABLE), (7, 8)),
                (Some(c::EXPECTED_EXECUTABLE), (8, 9)),
                (Some(c::UNEXPECTED_TOKEN), (9, 10)),
            ],
            "{fonte}: {:?}",
            out.diagnostics
        );
        // `int? a` vira campo com o `;` inserido; `var b` vem depois.
        assert_eq!(out.unit.declarations.len(), 2);
        assert!(matches!(decl(&out, 1), DeclKind::Variables(l) if l.variables.iter().any(|v| text(&names, v.name) == "b")));
    }

    /// `Foo() = Bar;` sem `factory`: o alvo é lido e só o `=` é
    /// denunciado, sem cascata (sondado no SDK 3.6.2 local).
    #[test]
    fn fasta_member_redirect_without_factory_reports_equals() {
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class Foo {\n  Foo()\n  = Bar\n  ;\n}\n";
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        let codigos: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            codigos,
            [(Some(c::REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR), (22, 23))],
            "{fonte}: {:?}",
            out.diagnostics
        );
    }

    /// `augment C(...) : campo = e;` com o recurso desligado lê o tipo como
    /// retorno de construtor (`constructor_with_return_type` nele, sondado
    /// no SDK 3.6.2 local) e a cauda como inits — sem cascata.
    #[test]
    fn fasta_member_augment_constructor_reads_as_constructor() {
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class A {\n  int value;\n  augment A(int? p1) : value = p1;\n}\n";
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        let codigos: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            codigos,
            [(Some(c::CONSTRUCTOR_WITH_RETURN_TYPE), (25, 32))],
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert_eq!(class(&out, 0).members.len(), 2, "{fonte}: {:?}", out.diagnostics);
    }

    /// `this :` em classe COM cabeçalho primário é parte de corpo mesmo sem
    /// o recurso (o oráculo 3.6 silencia pela cascata do cabeçalho, que
    /// aceitamos por superconjunto): sem `expected_class_member`.
    #[test]
    fn fasta_member_this_in_primary_class_does_not_report() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;

        let fonte = "class A(int x) {\n  A.named() : this(0);\n  this : assert(x > 0);\n}\n";
        let mut names = Interner::new();
        let out = parse_com(fonte, &mut names, LibraryFeatures::new(LanguageVersion::PISO, &[]));
        assert!(
            !out.diagnostics.iter().any(|d| d.code == Some(
                dartforge_diagnostics::codigos::parser::EXPECTED_CLASS_MEMBER
            )),
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert_eq!(class(&out, 0).members.len(), 2, "{fonte}: {:?}", out.diagnostics);
    }

    /// `this => 0;` sem o recurso: o analyzer 3.13.4 lê a parte `this` como
    /// com o recurso e só acrescenta `experiment_not_enabled` — no `(` do
    /// cabeçalho (`3.13.0`, do parser) e no `this` (`3.13`, do
    /// `AstBuilder`) —, sem `expected_class_member` (sondado numa
    /// biblioteca 3.6). `=> 0;` em classe dá só o par sem-nome/sem-parâmetros.
    #[test]
    fn member_this_arrow_without_feature_follows_3_13_4() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class A(int x) {\n  this => 0;\n}\n";
        let mut names = Interner::new();
        let out = parse_com(fonte, &mut names, LibraryFeatures::new(LanguageVersion::PISO, &[]));
        let feature: Vec<_> = out
            .diagnostics
            .iter()
            .filter(|d| d.code == Some(c::EXPERIMENT_NOT_ENABLED))
            .map(|d| ((d.span.start, d.span.end), d.args[1].to_string()))
            .collect();
        assert_eq!(feature, [((7, 8), "3.13.0".to_string()), ((19, 23), "3.13".to_string())], "{:?}", out.diagnostics);
        assert!(!out.diagnostics.iter().any(|d| d.code == Some(c::EXPECTED_CLASS_MEMBER)), "{:?}", out.diagnostics);

        let fonte = "class C {\n  => 0;\n}\n";
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        let codigos: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            codigos,
            [
                (Some(c::MISSING_IDENTIFIER), (12, 14)),
                (Some(c::MISSING_METHOD_PARAMETERS), (12, 14)),
            ],
            "{fonte}: {:?}",
            out.diagnostics
        );
    }

    /// `new foo();` sem o recurso é o construtor da 3.13 com
    /// `experiment_not_enabled` no `new` (versão `3.13`) e nada mais de
    /// sintaxe (sondado no analyzer 3.13.4, biblioteca 3.6).
    #[test]
    fn member_new_without_feature_is_constructor_with_feature_off() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class C {\n  new foo();\n  static int foo = 0;\n}\n";
        let mut names = Interner::new();
        let out = parse_com(fonte, &mut names, LibraryFeatures::new(LanguageVersion::PISO, &[]));
        let codigos: Vec<_> = out
            .diagnostics
            .iter()
            .map(|d| (d.code, (d.span.start, d.span.end), d.args.iter().map(|a| a.to_string()).collect::<Vec<_>>()))
            .collect();
        assert_eq!(
            codigos,
            [(Some(c::EXPERIMENT_NOT_ENABLED), (12, 15), vec!["primary-constructors".to_string(), "3.13".to_string(), "3.13".to_string()])],
            "{:?}",
            out.diagnostics
        );
        assert_eq!(class(&out, 0).members.len(), 2);
    }

    /// Nomeado privado sem nome público (`{this._123}`) e sem o recurso: o
    /// analyzer 3.13.4 relata só `experiment_not_enabled` no nome (oráculo
    /// gravado, biblioteca 3.6); a falta de nome público só é erro com o
    /// recurso ligado.
    #[test]
    fn private_named_without_feature_only_reports_the_feature() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class C {\n  int? _123;\n  C({this._123});\n}\n";
        let start_offset = fonte.rfind("_123").unwrap();
        let mut names = Interner::new();
        let out = parse_com(fonte, &mut names, LibraryFeatures::new(LanguageVersion::PISO, &[]));
        let codigos: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(codigos, [(Some(c::EXPERIMENT_NOT_ENABLED), (start_offset, start_offset + 4))], "{:?}", out.diagnostics);
    }

    /// `T X.Y` sem parênteses: os dois erros (`int C.named;`, sondado).
    #[test]
    fn fasta_member_type_and_method_without_parameters() {
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class C {\n  int C.named;\n}\n";
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        let codigos: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            codigos,
            [
                (Some(c::CONSTRUCTOR_WITH_RETURN_TYPE), (12, 15)),
                (Some(c::MISSING_METHOD_PARAMETERS), (16, 17)),
            ],
            "{fonte}: {:?}",
            out.diagnostics
        );
    }

    /// `X.Y` / `X.new` sem `(` em membro: `missing_method_parameters` em X
    /// (sondado no SDK 3.6.2 local: `C.named;`, `foo.bar = 1;`,
    /// `C.named : x = 1;`, `C.new;`).
    #[test]
    fn fasta_member_method_without_parameters() {
        use dartforge_diagnostics::codigos::parser as c;

        for (member, start_offset, end_offset) in [
            ("C.named;", 12, 13),
            ("foo.bar = 1;", 12, 15),
            ("C.new;", 12, 13),
        ] {
            let fonte = format!("class C {{\n  {member}\n}}\n");
            let mut names = Interner::new();
            let out = parse(&fonte, &mut names);
            let first = &out.diagnostics[0];
            assert_eq!(first.code, Some(c::MISSING_METHOD_PARAMETERS), "{fonte}: {:?}", out.diagnostics);
            assert_eq!((first.span.start, first.span.end), (start_offset, end_offset), "{fonte}");
        }
    }

    /// `foo;`, `foo = 1;`, `foo, bar;` em membro: o nome é campo sem tipo —
    /// `missing_const_final_var_or_type` nele (salvo `const`/`final`/`var`),
    /// sondado no SDK 3.6.2 local. Também `static = 1;` e `C;`.
    #[test]
    fn fasta_member_untyped_field_reports_at_name() {
        use dartforge_diagnostics::codigos::parser as c;

        for (member, start_offset, end_offset, errors) in [
            ("foo;", 12, 15, 1),
            ("foo = 1;", 12, 15, 1),
            ("foo, bar;", 12, 15, 1),
            ("static = 1;", 12, 18, 1),
            ("external;", 12, 20, 1),
            ("int;", 12, 15, 1),
            ("const foo;", 0, 0, 0),
            ("final foo;", 0, 0, 0),
            ("var foo;", 0, 0, 0),
        ] {
            let fonte = format!("class C {{\n  {member}\n}}\n");
            let mut names = Interner::new();
            let out = parse(&fonte, &mut names);
            assert_eq!(out.diagnostics.len(), errors, "{fonte}: {:?}", out.diagnostics);
            if errors == 1 {
                let single = &out.diagnostics[0];
                assert_eq!(single.code, Some(c::MISSING_CONST_FINAL_VAR_OR_TYPE), "{fonte}");
                assert_eq!((single.span.start, single.span.end), (start_offset, end_offset), "{fonte}");
            }
        }
    }
    /// `int? ab]` em membro continua declaração: falta `;` no nome e o `]`
    /// é `expected_class_member` nele (sondado no SDK 3.6.2 local).
    #[test]
    fn fasta_member_nullable_type_with_closer_continues_declaration() {
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class C {\n  int? ab]\n}\n";
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        let codigos: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            codigos,
            [
                (Some(c::EXPECTED_TOKEN), (17, 19)),
                (Some(c::EXPECTED_CLASS_MEMBER), (19, 20)),
            ],
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert_eq!(out.unit.declarations.len(), 1, "{fonte}");
        // O `;` inserido fecha o campo `int? ab` (`ensureSemicolon`).
        assert_eq!(class(&out, 0).members.len(), 1, "{fonte}: {:?}", out.diagnostics);
    }

    /// `(` sem tipo nem modificadores não abre declaração no topo: é
    /// `expected_executable`, não `missing_identifier`.
    #[test]
    fn fasta_top_level_stray_paren_is_not_identifier() {
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "(42);\nvar b = 0;\n";
        let mut names = Interner::new();
        let out = parse(fonte, &mut names);
        let first = &out.diagnostics[0];
        assert_eq!(first.code, Some(c::EXPECTED_EXECUTABLE));
        assert_eq!((first.span.start, first.span.end), (0, 1));
        assert!(
            !out.diagnostics.iter().any(|d| d.code == Some(c::MISSING_IDENTIFIER)),
            "{fonte}: {:?}",
            out.diagnostics
        );
        let ultimo = out.unit.declarations.len() - 1;
        assert!(matches!(decl(&out, ultimo), DeclKind::Variables(_)));
    }

    #[test]
    fn empty_and_native_function_body() {
        let mut names = Interner::new();
        for (src, modifier, native) in [
            (";", AsyncModifier::None, false),
            ("async;", AsyncModifier::Async, false),
            ("async*;", AsyncModifier::AsyncStar, false),
            ("sync*;", AsyncModifier::SyncStar, false),
            ("native;", AsyncModifier::None, true),
        ] {
            let tokens = crate::lexer::lex(src).unwrap();
            let mut p = Parser::new(src, tokens, &mut names);
            let (m, body) = p.parse_function_body().unwrap();
            assert_eq!(m, modifier, "{src}");
            assert!(p.diagnostics.is_empty());
            assert!(p.at_eof());
            assert!(!p.in_async && !p.in_generator, "contexto restaurado: {src}");
            match body {
                FunctionBody::Empty => assert!(!native),
                FunctionBody::Native(None) => assert!(native),
                other => panic!("{src}: {other:?}"),
            }
        }
    }

    #[test]
    fn missing_function_body_restores_context() {
        let mut names = Interner::new();
        let src = "async )";
        let tokens = crate::lexer::lex(src).unwrap();
        let mut p = Parser::new(src, tokens, &mut names);
        // `ensureBlock` do fasta: corpo ausente vira bloco vazio sintético,
        // sem consumir o `)`.
        assert!(matches!(p.parse_function_body(), Ok((_, FunctionBody::Block(_)))));
        assert_eq!(p.diagnostics.len(), 1);
        assert!(p.at_op(crate::token::Op::RParen));
        assert!(!p.in_async);
    }

    #[test]
    fn composite_operator_name() {
        let mut names = Interner::new();
        for (src, expected_) in [
            (">=", ">="),
            (">>", ">>"),
            (">>>", ">>>"),
            (">", ">"),
            ("[]", "[]"),
            ("[]=", "[]="),
            ("~/", "~/"),
            ("==", "=="),
            ("-", "-"),
        ] {
            let tokens = crate::lexer::lex(src).unwrap();
            let mut p = Parser::new(src, tokens, &mut names);
            let name = p.parse_operator_name().unwrap();
            assert!(p.at_eof(), "{src}");
            assert_eq!(text(&names, name), expected_);
        }
    }

    // -- Dependentes de types/expressions/statements ------------------------
    //
    // Estes testes panicam com `not yet implemented` enquanto os outros
    // módulos são stubs; passam quando eles existirem.
    mod dependents {
        use super::*;
        use crate::ast::{Combinator, Initializer, TypedefKind};

        #[test]
        fn directives_with_uri() {
            let mut names = Interner::new();
            let src = "import 'a.dart'; import 'b.dart' deferred as b show x, y hide z; \
                       import 'c.dart' if (dart.library.io == 'x') 'd.dart' if (dart.library.html) 'e.dart' as c; \
                       export 'f.dart' show q; part 'g.dart'; part of 'h.dart';";
            let out = parse_ok_except_order(src, &mut names);
            assert_eq!(out.unit.directives.len(), 6);
            match &out.unit.directives[1].kind {
                DirectiveKind::Import {
                    deferred,
                    prefix,
                    combinators,
                    ..
                } => {
                    assert!(*deferred);
                    assert_eq!(text(&names, prefix.unwrap()), "b");
                    assert_eq!(combinators.len(), 2);
                    assert!(matches!(&combinators[0], Combinator::Show(n) if n.len() == 2));
                    assert!(matches!(&combinators[1], Combinator::Hide(n) if n.len() == 1));
                }
                other => panic!("{other:?}"),
            }
            match &out.unit.directives[2].kind {
                DirectiveKind::Import { configurations, .. } => {
                    assert_eq!(configurations.len(), 2);
                    assert_eq!(configurations[0].test.len(), 3);
                    assert!(configurations[0].value.is_some());
                    assert!(configurations[1].value.is_none());
                }
                other => panic!("{other:?}"),
            }
            assert!(matches!(
                &out.unit.directives[5].kind,
                DirectiveKind::PartOf { uri: Some(_), .. }
            ));
        }

        #[test]
        fn class_and_mixin_application_headers() {
            let mut names = Interner::new();
            let src = "class A<T> extends B<T> with M1, M2 implements I, J {} \
                       class C<T> = S with M1, M2 implements I; \
                       mixin M<T> on A, B implements C {}";
            let out = parse_ok(src, &mut names);
            let a = class(&out, 0);
            assert!(a.extends.is_some() && a.with.len() == 2 && a.implements.len() == 2);
            assert_eq!(a.type_params.len(), 1);
            let c = class(&out, 1);
            assert!(c.mixin_application && c.extends.is_some() && c.with.len() == 2);
            match decl(&out, 2) {
                DeclKind::Mixin(m) => assert!(m.on.len() == 2 && m.implements.len() == 1),
                other => panic!("{other:?}"),
            }
        }

        #[test]
        fn full_enum() {
            let mut names = Interner::new();
            let src = "enum E<T> with M implements I { @deprecated a, b('x'), c<int>.named(1); \
                       final int x; const E([this.x = 0]); int get y => x; static E of(int v) => a; }";
            let out = parse_ok(src, &mut names);
            match decl(&out, 0) {
                DeclKind::Enum(e) => {
                    assert_eq!(e.constants.len(), 3);
                    assert_eq!(e.constants[0].metadata.len(), 1);
                    assert!(e.constants[1].arguments.is_some());
                    assert_eq!(e.constants[2].type_args.len(), 1);
                    assert!(e.constants[2].constructor.is_some());
                    assert_eq!(e.members.len(), 4);
                    assert!(
                        matches!(&out.ast.member(e.members[1]).kind, MemberKind::Constructor(c) if c.const_)
                    );
                }
                other => panic!("{other:?}"),
            }
        }

        #[test]
        fn extensions_and_extension_types() {
            let mut names = Interner::new();
            let src = "extension E<T> on List<T> { T get first => this[0]; } \
                       extension on int { int get dobro => this * 2; } \
                       extension type E2<T>(int x) implements I { E2.named(int y) : this(y); } \
                       extension type const E3._(final int x) {} \
                       extension type on X {}";
            let out = parse_ok(src, &mut names);
            assert_eq!(out.unit.declarations.len(), 5);
            match decl(&out, 0) {
                DeclKind::Extension(e) => assert!(e.name.is_some() && e.members.len() == 1),
                other => panic!("{other:?}"),
            }
            match decl(&out, 1) {
                DeclKind::Extension(e) => assert!(e.name.is_none()),
                other => panic!("{other:?}"),
            }
            match decl(&out, 2) {
                DeclKind::ExtensionType(e) => {
                    assert!(!e.const_ && e.constructor.is_none() && e.implements.len() == 1);
                    assert_eq!(text(&names, e.representation_name), "x");
                    assert!(
                        matches!(&out.ast.member(e.members[0]).kind, MemberKind::Constructor(c)
                        if matches!(c.initializers[0], Initializer::Redirect { .. }))
                    );
                }
                other => panic!("{other:?}"),
            }
            match decl(&out, 3) {
                DeclKind::ExtensionType(e) => {
                    assert!(e.const_);
                    assert_eq!(text(&names, e.constructor.unwrap()), "_");
                }
                other => panic!("{other:?}"),
            }
            match decl(&out, 4) {
                DeclKind::Extension(e) => assert_eq!(text(&names, e.name.unwrap()), "type"),
                other => panic!("{other:?}"),
            }
        }

        #[test]
        fn typedefs() {
            let mut names = Interner::new();
            let src = "typedef F<T> = int Function(T); typedef X<T> = Map<T, T>; \
                       typedef int G<T>(T x); typedef H(int x); typedef void K<T>(T x); typedef L<T>(T x);";
            let out = parse_ok(src, &mut names);
            assert_eq!(out.unit.declarations.len(), 6);
            let td = |i: usize| match decl(&out, i) {
                DeclKind::Typedef(t) => t,
                other => panic!("{other:?}"),
            };
            assert!(matches!(td(0).kind, TypedefKind::Alias(_)) && td(0).type_params.len() == 1);
            assert!(matches!(td(1).kind, TypedefKind::Alias(_)));
            assert!(matches!(
                td(2).kind,
                TypedefKind::Legacy {
                    return_type: Some(_),
                    ..
                }
            ));
            assert!(matches!(
                td(3).kind,
                TypedefKind::Legacy {
                    return_type: None,
                    ..
                }
            ));
            assert!(matches!(
                td(4).kind,
                TypedefKind::Legacy {
                    return_type: Some(_),
                    ..
                }
            ));
            assert!(matches!(
                td(5).kind,
                TypedefKind::Legacy {
                    return_type: None,
                    ..
                }
            ));
            assert_eq!(td(5).type_params.len(), 1);
        }

        #[test]
        fn top_level_functions_and_variables() {
            let mut names = Interner::new();
            let src = "T f<U>(U u) async { return u; } main() {} get x => 1; int get y => 2; \
                       set z(int v) {} external void e(); int a = 1, b; final c = 1; const int d = 1; \
                       late final int l; external int ext; Stream<int> g() async* { yield 1; } \
                       Iterable<int> h() sync* {} int Function(int) k() => (x) => x; (int, int) r() => (1, 2);";
            let out = parse_ok(src, &mut names);
            assert_eq!(out.unit.declarations.len(), 15);
            let f = |i: usize| match decl(&out, i) {
                DeclKind::Function(id) => out.ast.function(*id),
                other => panic!("{other:?}"),
            };
            assert_eq!(f(0).modifier, AsyncModifier::Async);
            assert_eq!(f(0).type_params.len(), 1);
            assert!(f(1).return_type.is_none());
            assert_eq!(f(2).kind, FunctionKind::Getter);
            assert!(f(2).return_type.is_none());
            assert!(f(3).return_type.is_some());
            assert_eq!(f(4).kind, FunctionKind::Setter);
            assert!(f(5).external && matches!(f(5).body, FunctionBody::Empty));
            match decl(&out, 6) {
                DeclKind::Variables(v) => {
                    assert_eq!(v.variables.len(), 2);
                    assert!(
                        v.variables[0].initializer.is_some()
                            && v.variables[1].initializer.is_none()
                    );
                }
                other => panic!("{other:?}"),
            }
            assert!(matches!(decl(&out, 7), DeclKind::Variables(v) if v.final_ && v.ty.is_none()));
            assert!(matches!(decl(&out, 8), DeclKind::Variables(v) if v.const_ && v.ty.is_some()));
            assert!(matches!(decl(&out, 9), DeclKind::Variables(v) if v.late && v.final_));
            assert!(matches!(decl(&out, 10), DeclKind::Variables(v) if v.external));
            assert_eq!(f(11).modifier, AsyncModifier::AsyncStar);
            assert_eq!(f(12).modifier, AsyncModifier::SyncStar);
            assert!(matches!(f(13).body, FunctionBody::Expression(_)));
            assert!(f(14).return_type.is_some());
        }

        /// `operator` é identificador embutido: serve de nome de campo.
        #[test]
        fn field_named_operator() {
            let mut names = Interner::new();
            let out = parse_ok(
                "class A { final O operator; A(this.operator); }",
                &mut names,
            );
            let a = class(&out, 0);
            assert_eq!(a.members.len(), 2);
            assert!(matches!(member(&out, a, 0), MemberKind::Field(_)));
        }

        #[test]
        fn members_methods_and_operators() {
            let mut names = Interner::new();
            let src = "class A { static const int x = 1; final int a, b; late String s; covariant int c; \
                       abstract int d; external int e; List<int> Function() f = () => []; \
                       static T m<T>(T t) => t; external void ext(); void abs(); \
                       int get g => 1; set g(int v) {} get h; set i(v); \
                       operator +(A o) => this; void operator []=(int k, A v) {} A operator [](int k) => this; \
                       bool operator ==(Object o) => true; bool operator >=(A o) => true; \
                       int operator >>(int n) => 1; int operator >>>(int n) => 1; int operator <<(int n) => 1; \
                       bool operator <=(A o) => true; bool operator <(A o) => true; bool operator >(A o) => true; \
                       A operator -() => this; A operator ~() => this; int operator ~/(int o) => 1; \
                       int foo() native; int bar() native 'bar'; }";
            let out = parse_ok(src, &mut names);
            let a = class(&out, 0);
            assert_eq!(a.members.len(), 30);
            let method = |i: usize| match member(&out, a, i) {
                MemberKind::Method(id) => out.ast.function(*id),
                other => panic!("{other:?}"),
            };
            assert!(matches!(member(&out, a, 0), MemberKind::Field(f) if f.static_ && f.const_));
            assert!(
                matches!(member(&out, a, 1), MemberKind::Field(f) if f.final_ && f.variables.len() == 2)
            );
            assert!(matches!(member(&out, a, 4), MemberKind::Field(f) if f.abstract_));
            assert!(method(7).static_ && method(7).type_params.len() == 1);
            assert!(method(8).external);
            assert!(matches!(method(9).body, FunctionBody::Empty));
            assert_eq!(method(10).kind, FunctionKind::Getter);
            assert_eq!(method(11).kind, FunctionKind::Setter);
            let op = |i: usize| {
                assert_eq!(method(i).kind, FunctionKind::Operator);
                text(&names, method(i).name.unwrap())
            };
            let expected_items = [
                "+", "[]=", "[]", "==", ">=", ">>", ">>>", "<<", "<=", "<", ">", "-", "~", "~/",
            ];
            for (k, expected_) in expected_items.iter().enumerate() {
                assert_eq!(op(14 + k), *expected_);
            }
            assert!(matches!(method(28).body, FunctionBody::Native(None)));
            assert!(matches!(method(29).body, FunctionBody::Native(Some(_))));
        }

        #[test]
        fn constructors_() {
            let mut names = Interner::new();
            let src = "class C<T> { final int x; int _z; \
                       C(this.x, {super.key, required this.y}) : assert(x > 0, 'msg'), _z = x, super.named(1) {} \
                       C.named() : this(1); const C.k() : x = 0, _z = 0; \
                       factory C.f() => C(1); factory C.r() = D<T>.y; const factory C.s() = C.k; \
                       external C.e(); C.arrow() : x = 1, _z = 2; C.dotted() : this.x = 1, this._z = 2; }";
            let out = parse_ok(src, &mut names);
            let c = class(&out, 0);
            assert_eq!(c.members.len(), 11);
            let ctor = |i: usize| match member(&out, c, i) {
                MemberKind::Constructor(k) => k,
                other => panic!("{other:?}"),
            };
            assert!(ctor(2).name.is_none() && ctor(2).initializers.len() == 3);
            assert!(matches!(
                ctor(2).initializers[0],
                Initializer::Assert {
                    message: Some(_),
                    ..
                }
            ));
            assert!(matches!(
                ctor(2).initializers[1],
                Initializer::Field { this_: false, .. }
            ));
            assert!(matches!(
                ctor(2).initializers[2],
                Initializer::Super {
                    constructor: Some(_),
                    ..
                }
            ));
            assert!(matches!(
                ctor(3).initializers[0],
                Initializer::Redirect {
                    constructor: None,
                    ..
                }
            ));
            assert!(ctor(4).const_);
            assert!(ctor(5).factory && matches!(ctor(5).body, FunctionBody::Expression(_)));
            assert!(ctor(6).factory && ctor(6).redirect.as_ref().unwrap().constructor.is_some());
            assert!(ctor(7).const_ && ctor(7).factory && ctor(7).redirect.is_some());
            assert!(ctor(8).external);
            assert!(matches!(
                ctor(10).initializers[1],
                Initializer::Field { this_: true, .. }
            ));
        }

        #[test]
        fn recovery_inside_method_body() {
            let mut names = Interner::new();
            let out = parse(
                "class A { void f() { x = ; } void g() {} } class B { int x } class C {}",
                &mut names,
            );
            assert_eq!(out.diagnostics.len(), 2, "{:?}", out.diagnostics);
            assert_eq!(out.unit.declarations.len(), 3);
            assert_eq!(class(&out, 0).members.len(), 2);
        }

        #[test]
        fn metadata_with_arguments() {
            let mut names = Interner::new();
            let src = "@pragma('vm:entry-point') @p.Nome.ctor(1) @Nome<int>(2) @Nome<int>.ctor(3) @a.b.c class A {} \
                       @meta (int, int) f() => (1, 2);";
            let out = parse_ok(src, &mut names);
            let a = out.ast.decl(out.unit.declarations[0]);
            assert_eq!(a.metadata.len(), 5);
            assert!(a.metadata[0].arguments.is_some());
            assert_eq!(a.metadata[1].name.len(), 3);
            assert_eq!(a.metadata[2].type_args.len(), 1);
            assert_eq!(a.metadata[3].name.len(), 2);
            assert!(a.metadata[4].arguments.is_none());
            let f = out.ast.decl(out.unit.declarations[1]);
            assert!(f.metadata[0].arguments.is_none());
            assert!(matches!(f.kind, DeclKind::Function(_)));
        }
    }

    /// `augment`, `macro class`, `import augment` e `augment library`
    /// (docs/AUGMENTATIONS.md, experimentos `augmentations`/`macros`).
    mod augmentations {
        use super::*;
        use crate::features::{Feature, LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;

        fn com(exp: &[Feature], src: &str, names: &mut Interner) -> Parsed {
            parse_com(src, names, LibraryFeatures::new(LanguageVersion::PISO, exp))
        }

        #[test]
        fn augment_on_top_level_and_members() {
            let mut names = Interner::new();
            let src = "augment class C { augment void f() {} int g() => 1; augment C.x() : y = 1; augment int get z => 2; }
                       augment String saudacao(String n) => n;";
            let out = com(&[Feature::Macros], src, &mut names);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            let d = out.ast.decl(out.unit.declarations[0]);
            assert!(d.augment);
            let c = class(&out, 0);
            let aug: Vec<bool> = c.members.iter().map(|&m| out.ast.member(m).augment).collect();
            assert_eq!(aug, [true, false, true, true]);
            assert!(matches!(member(&out, c, 2), MemberKind::Constructor(_)));
            assert!(out.ast.decl(out.unit.declarations[1]).augment);
        }

        #[test]
        fn augment_is_a_plain_name_without_the_feature() {
            // Sem `augmentations`/`macros`, `augment` continua identificador:
            // código 3.6 que o usa como nome não muda de sentido.
            let mut names = Interner::new();
            let out = com(&[], "var augment = 1; int f() => augment; void augment2() {}", &mut names);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            assert!(!out.ast.decl(out.unit.declarations[0]).augment);
            // Nem `augment class` é modificador: o fasta tokeniza `augment`
            // como identificador e a recuperação de topo relata o par do
            // analyzer 3.13.4 no `augment` (sondado), sem recurso desligado.
            let out = com(&[], "augment class C {}", &mut names);
            let mut codigos: Vec<_> = out.diagnostics.iter().map(|d| (d.code, d.span.start, d.span.end)).collect();
            codigos.sort();
            use dartforge_diagnostics::codigos::parser as c;
            let mut expected_ = [(Some(c::EXPECTED_TOKEN), 0, 7), (Some(c::MISSING_CONST_FINAL_VAR_OR_TYPE), 0, 7)];
            expected_.sort();
            assert_eq!(
                codigos,
                expected_,
                "{:?}",
                out.diagnostics
            );
            assert!(out.unit.declarations.iter().all(|&d| !out.ast.decl(d).augment));
        }

        /// Sem o experimento, `augment` antes de uma declaração é nome comum
        /// e cai na recuperação de topo. Códigos sintáticos e intervalos do
        /// analyzer 3.13.4 num pacote 3.6 (sondados; `undefined_class` e
        /// `unused_element` são semânticos e ficam fora).
        #[test]
        fn augment_without_the_feature_follows_fasta_recovery() {
            use dartforge_diagnostics::codigos::parser as c;
            let cases_: [(&str, &[(dartforge_diagnostics::Codigo, usize, usize)]); 5] = [
                ("augment class C {}", &[(c::EXPECTED_TOKEN, 0, 7), (c::MISSING_CONST_FINAL_VAR_OR_TYPE, 0, 7)]),
                ("augment enum E { a }", &[(c::EXPECTED_TOKEN, 0, 7), (c::MISSING_CONST_FINAL_VAR_OR_TYPE, 0, 7)]),
                ("augment mixin M {}", &[(c::EXPECTED_TOKEN, 0, 7), (c::MISSING_IDENTIFIER, 8, 13)]),
                ("augment extension X on int {}", &[(c::EXPECTED_TOKEN, 0, 7), (c::MISSING_IDENTIFIER, 8, 17)]),
                ("augment abstract class D {}", &[(c::EXPECTED_TOKEN, 8, 16)]),
            ];
            for (fonte, expected_) in cases_ {
                let mut names = Interner::new();
                let out = com(&[], fonte, &mut names);
                let mut found: Vec<_> = out.diagnostics.iter().map(|d| (d.code.unwrap(), d.span.start, d.span.end)).collect();
                let mut expected_ = expected_.to_vec();
                found.sort_by_key(|&(c, s, e)| (s, e, c));
                expected_.sort_by_key(|&(c, s, e)| (s, e, c));
                assert_eq!(found, expected_, "{fonte}: {:?}", out.diagnostics);
            }
        }

        #[test]
        fn macro_class_and_directives_in_36_form() {
            let mut names = Interner::new();
            let src = "import augment 'a_aug.dart';
macro class M implements Macro { const M(); }";
            let out = com(&[Feature::Macros], src, &mut names);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            assert!(matches!(out.unit.directives[0].kind, DirectiveKind::ImportAugment { .. }));
            assert!(class(&out, 0).modifiers.macro_);
            let out = com(&[Feature::Macros], "augment library 'main.dart';
import 'dart:core' as p;", &mut names);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            assert!(matches!(out.unit.directives[0].kind, DirectiveKind::AugmentLibrary { .. }));
            // Sem `macros`: diagnóstico, não erro de sintaxe.
            let out = com(&[], "macro class M {}", &mut names);
            assert_eq!(out.diagnostics.len(), 1, "{:?}", out.diagnostics);
            assert!(class(&out, 0).modifiers.macro_);
        }

        #[test]
        fn text_generated_by_cfe_for_json_codable() {
            // A augmentation que o CFE 3.6.2 gera para o `@JsonCodable`.
            let mut names = Interner::new();
            let src = "augment library 'main.dart';

import 'dart:core' as prefix0;

                       augment class Usuario {
                         external Usuario.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json);
                         augment Usuario.fromJson(prefix0.Map<prefix0.String, prefix0.Object?> json, )
                             : this.nome = json[r'nome'] as prefix0.String;
                         augment prefix0.Map<prefix0.String, prefix0.Object?> toJson() {
                           final json = <prefix0.String, prefix0.Object?>{};
    return json;
  }
}
";
            let out = com(&[Feature::Macros], src, &mut names);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            let c = class(&out, 0);
            assert_eq!(c.members.len(), 3);
            assert!(!out.ast.member(c.members[0]).augment);
            assert!(out.ast.member(c.members[1]).augment);
            assert!(matches!(member(&out, c, 1), MemberKind::Constructor(_)));
            assert!(matches!(member(&out, c, 2), MemberKind::Method(_)));
        }
    }
}
