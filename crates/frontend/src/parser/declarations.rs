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
//!   declaração de topo). Uma classe ou enum sem corpo em biblioteca ≤3.6
//!   (`class A(`, resto de sintaxe 3.13) registra o corpo ausente no token
//!   anterior e continua no token corrente, sem engolir a declaração.
//!   A recuperação de membros segue o mesmo desenho, um membro por vez.
//! * **Construtor × método.** `Nome(` é construtor quando `Nome` é o da
//!   classe corrente (o nome é passado ao parser de membros); `Nome.x(` é
//!   sempre construtor, porque métodos não têm nome pontuado; `factory` e
//!   `const Nome(` idem.
//! * **Função × variável.** Após `tipo? nome`, `(` ou `<` abre uma função;
//!   qualquer outra coisa é uma lista de variáveis. `get`/`set` seguidos de
//!   identificador são getter/setter mesmo sem tipo de retorno, porque são
//!   identificadores embutidos e não podem nomear um tipo.
//! * **Metadata.** `@Nome (` só é lista de argumentos se o `(` estiver colado
//!   ao nome (ou houver argumentos de tipo), como faz o parser do SDK — caso
//!   contrário o `(` inicia um tipo record da declaração seguinte.
//! * **Redirecionamento de factory.** Em `= D.x;` o `D.x` é lido primeiro
//!   como tipo (possivelmente `prefixo.Tipo`); `.x` só vira nome de construtor
//!   se sobrar após o tipo. Distinguir `prefixo.Tipo` de `Tipo.construtor`
//!   exige resolução.
//! * **Ordem das diretivas.** Não é imposta: qualquer diretiva é aceita em
//!   qualquer ponto do nível de topo (superconjunto; a fase seguinte valida).
use super::{ComposedGt, PResult, ParseError, Parser};
use crate::ast::ExprId;
use crate::ast::{
    Annotation, AsyncModifier, ClassDecl, ClassModifiers, Combinator, CompilationUnit,
    Configuration, Constructor, Decl, DeclId, DeclKind, Directive, DirectiveKind, EnumConstant,
    EnumDecl, ExtensionDecl, ExtensionTypeDecl, Function, FunctionBody, FunctionId, FunctionKind,
    Initializer, Member, MemberId, MemberKind, MixinDecl, Name, RedirectTarget, TypeId,
    TypedefDecl, TypedefKind, Variable, VariableList,
};
use crate::features::Feature;
use crate::token::{Keyword, Kind, Op};
use dartforge_diagnostics::{Diagnostic, Span, codigos};

/// Cabeçalho de um construtor primário já lido (Dart 3.13).
struct CabecalhoPrimario {
    /// Da palavra `class`/`enum` ao fim dos parâmetros.
    span: Span,
    const_: bool,
    /// `.id`; `None` sem nome ou com `.new`.
    nome: Option<Name>,
    params: Vec<crate::ast::Parameter>,
}

/// Que declaração está sendo elaborada.
#[derive(Clone, Copy)]
enum Elaborando {
    /// `class`: `tem_supertipos` desliga o tipo sintático de declarante sem
    /// tipo (um getter herdado mandaria).
    Classe { tem_supertipos: bool },
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

/// Resultado de `tipo? nome …`: função/método/acessor ou lista de variáveis.
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
        }
        while !self.at_eof() {
            let start_pos = self.pos;
            if self.parse_top_level_item(&mut unit).is_err() {
                self.recover_top_level(start_pos);
            }
        }
        unit
    }

    /// Uma diretiva ou uma declaração de topo, com sua metadata.
    fn parse_top_level_item(&mut self, unit: &mut CompilationUnit) -> PResult<()> {
        let start = self.span();
        let metadata = self.parse_metadata()?;
        if let Some(kind) = self.parse_directive_opt()? {
            unit.directives.push(Directive {
                span: self.span_from(start),
                metadata,
                kind,
            });
            return Ok(());
        }
        let augment = self.parse_augment_opt();
        if !self.can_start_declaration() {
            return Err(self.erro(codigos::parser::EXPECTED_EXECUTABLE, &[]));
        }
        let id = self.parse_top_level_declaration(start, metadata, augment)?;
        unit.declarations.push(id);
        Ok(())
    }

    /// O modificador `augment` de uma declaração de topo ou de um membro
    /// (docs/AUGMENTATIONS.md), consumido se presente.
    ///
    /// `augment` é identificador embutido só onde o recurso existe: com
    /// `augmentations` ou `macros` ligados, é modificador quando vem antes de
    /// outra palavra (`augment class`, `augment void f()`, `augment C.x(`).
    /// Sem eles, só as formas que não podem ser outra coisa (`augment class`,
    /// `augment mixin`, …) são lidas como modificador — com o diagnóstico de
    /// recurso desligado —, para não quebrar código 3.6 que usa `augment`
    /// como nome.
    fn parse_augment_opt(&mut self) -> bool {
        if !self.at_ident("augment") {
            return false;
        }
        let ligado =
            self.features.tem(Feature::Augmentations) || self.features.tem(Feature::Macros);
        let seguinte_e_palavra = matches!(self.kind_at(1), Kind::Ident | Kind::Keyword(_));
        let inequivoco = matches!(self.kind_at(1), Kind::Keyword(Keyword::Class | Keyword::Enum))
            || (self.kind_at(1) == Kind::Ident
                && matches!(self.text_of(self.pos + 1), "mixin" | "extension" | "abstract" | "base" | "sealed" | "interface")
                && matches!(self.kind_at(2), Kind::Ident | Kind::Keyword(_)));
        if !(seguinte_e_palavra && (ligado || inequivoco)) {
            return false;
        }
        let t = self.advance();
        if !ligado {
            self.exigir(Feature::Augmentations, t.span);
        }
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
            self.advance();
            return;
        }        let mut depth = self.nesting_between(start_pos, self.pos);
        loop {
            match self.kind() {
                Kind::Eof => break,
                Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => {
                    depth += 1;
                    self.advance();
                }
                Kind::Op(Op::RParen | Op::RBracket) => {
                    // Um fechamento solto no nível zero é, ele próprio, uma
                    // fronteira: não vale engolir a declaração seguinte.
                    self.advance();
                    if depth == 0 {
                        break;
                    }
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
    /// Consome até um `;` ou até fechar o `{` do próprio membro; a `}` que
    /// fecha a classe **não** é consumida, para que o laço do corpo termine.
    fn recover_member(&mut self, start_pos: usize) {
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
                    // fronteira: não vale engolir a declaração seguinte.
                    self.advance();
                    if depth == 0 {
                        break;
                    }
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
                Kind::Op(Op::At) if depth == 0 && self.pos > start_pos => break,
                _ => {
                    self.advance();
                }
            }
        }
        if self.pos == start_pos && !self.at_op(Op::RBrace) {
            self.advance();
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
            self.exigir(Feature::Macros, t.span);
            let uri = self.parse_string_literal()?;
            self.expect_op(Op::Semicolon)?;
            return Ok(Some(DirectiveKind::ImportAugment { uri }));
        }
        if self.at_ident("augment") && self.at_ident_at(1, "library") && self.string_at(2) {
            let t = self.advance();
            self.exigir(Feature::Macros, t.span);
            self.advance();
            let uri = self.parse_string_literal()?;
            self.expect_op(Op::Semicolon)?;
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
            self.expect_op(Op::Semicolon)?;
            return Ok(Some(DirectiveKind::Library { name }));
        }
        if self.at_ident("import") && self.string_at(1) {
            self.advance();
            let uri = self.parse_string_literal()?;
            let configurations = self.parse_configurations()?;
            let deferred = self.eat_ident("deferred");
            let prefix = if self.eat_ident("as") {
                Some(self.expect_identifier()?)
            } else {
                None
            };
            let combinators = self.parse_combinators()?;
            self.expect_op(Op::Semicolon)?;
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
            self.expect_op(Op::Semicolon)?;
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
                self.expect_op(Op::Semicolon)?;
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
                self.expect_op(Op::Semicolon)?;
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
            let type_args = if self.at_op(Op::Lt) {
                self.parse_type_arguments_opt()?
            } else {
                Vec::new()
            };
            if self.at_op(Op::Dot) && (self.at_identifier_at(1) || self.at_kw_at(1, Keyword::New)) {
                self.advance();
                let part = self.identifier_or_new()?;
                name.push(part);
            }
            let glued = self.pos > 0 && self.tokens[self.pos - 1].glued;
            let arguments = if self.at_op(Op::LParen) && (glued || !type_args.is_empty()) {
                Some(self.parse_arguments()?)
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
        let kind = if self.class_follows() {
            DeclKind::Class(self.parse_class()?)
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
            let mods = self.parse_modifiers();
            match self.parse_function_or_variables(mods, fstart, mods.external_span)? {
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

    /// `modificadores* class` começa aqui?
    fn class_follows(&self) -> bool {
        let mut i = self.pos;
        loop {
            match self.kind_of(i) {
                Kind::Keyword(Keyword::Final) => i += 1,
                Kind::Ident
                    if matches!(
                        self.text_of(i),
                        "abstract" | "base" | "interface" | "sealed" | "mixin" | "macro"
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
        (self.at_ident("mixin") && self.at_identifier_at(1))
            || (self.at_ident("base") && self.at_ident_at(1, "mixin") && self.at_identifier_at(2))
    }

    /// `classDeclaration`, inclusive a forma `class C = S with M;`.
    fn parse_class(&mut self) -> PResult<ClassDecl> {
        let mut modifiers = ClassModifiers::default();
        loop {
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
                self.exigir(Feature::Macros, t.span);
                modifiers.macro_ = true;
            } else {
                break;
            }
        }
        let class_token = self.expect_kw(Keyword::Class)?;
        // `class const K(...)`: construtor primário constante (3.13).
        let const_primario = self.at_kw(Keyword::Const).then(|| self.advance().span);
        let name_text = self.text();
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_parameters_opt()?;
        if const_primario.is_none() && self.eat_op(Op::Assign) {
            let extends = Some(self.parse_type()?);
            self.expect_kw(Keyword::With)?;
            let with = self.parse_type_list()?;
            let implements = self.parse_implements_opt()?;
            self.expect_op(Op::Semicolon)?;
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
        let primario = self.parse_cabecalho_primario_opt(class_token.span, const_primario)?;
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
        // `class A(` sem o recurso (biblioteca ≤3.6): não há cabeçalho
        // primário, e sim corpo ausente. O fasta relata `expected_class_body`
        // no token anterior e continua no `(` — um `expected_executable` por
        // token (ver `recover_top_level`) — em vez de engolir a declaração
        // inteira num só erro. Só vale para `(`: o resto (`class A B`) mantém
        // o caminho antigo, ainda sem oráculo medido.
        let mut members = if !self.features.tem(Feature::PrimaryConstructors) && self.at_op(Op::LParen)
        {
            let anterior = self.tokens[self.pos - 1].span;
            self.erro_em(codigos::parser::EXPECTED_CLASS_BODY, anterior, &[]);
            Vec::new()
        } else {
            self.parse_class_body_ou_vazio(Some(name_text))?
        };
        let tem_supertipos = extends.is_some() || !with.is_empty() || !implements.is_empty();
        let primary_constructor = self.elaborar_construtor_primario(
            name,
            primario,
            &mut members,
            Elaborando::Classe { tem_supertipos },
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
    fn parse_cabecalho_primario_opt(
        &mut self,
        inicio: Span,
        const_: Option<Span>,
    ) -> PResult<Option<CabecalhoPrimario>> {
        if !self.at_op(Op::LParen) && !(self.at_op(Op::Dot) && self.kind_at(1) != Kind::Op(Op::Dot))
        {
            if let Some(c) = const_ {
                return Err(self.erro_em(codigos::parser::EXTRANEOUS_MODIFIER, c, &["const"]));
            }
            return Ok(None);
        }
        // Sem o recurso (biblioteca ≤3.6), o cabeçalho primário não existe:
        // o `(` ou `.` seguinte é o corpo ausente da declaração, e o fasta o
        // relata ali (`expected_body` na classe, `missing_enum_body` no enum).
        // Consumir aqui esconderia esses diagnósticos e os seguintes, um por
        // token (`expected_executable`), atrás de um só `experiment_not_enabled`.
        if !self.features.tem(Feature::PrimaryConstructors) {
            return Ok(None);
        }
        let comeco = self.span();
        let nome = if self.eat_op(Op::Dot) {
            let n = self.identifier_or_new()?;
            (self.interner.resolve(n.sym) != "new").then_some(n)
        } else {
            None
        };
        let salvo = self.em_construtor_primario;
        self.em_construtor_primario = true;
        let params = self.parse_formal_parameters();
        self.em_construtor_primario = salvo;
        let params = params?;
        let span = self.span_from(comeco);
        self.exigir(Feature::PrimaryConstructors, span);
        Ok(Some(CabecalhoPrimario {
            span: Span {
                start: inicio.start,
                end: span.end,
            },
            const_: const_.is_some(),
            nome,
            params,
        }))
    }

    /// `{ membros }` ou, a partir da 3.13, `;` (corpo vazio).
    fn parse_class_body_ou_vazio(&mut self, class_name: Option<&'s str>) -> PResult<Vec<MemberId>> {
        if self.at_op(Op::Semicolon) {
            let t = self.advance();
            self.exigir(Feature::PrimaryConstructors, t.span);
            return Ok(Vec::new());
        }
        self.parse_class_body(class_name)
    }

    /// Derivação D → D2 do construtor primário (spec 3.13, `:926-1025`),
    /// feita na árvore, logo depois do parse da declaração: cada parâmetro
    /// declarante `var`/`final T p` vira um campo `T p;`/`final T p;` e o
    /// parâmetro vira `T this.p` (o `covariant` passa ao campo); o construtor
    /// `k2` recebe o nome, o `const` do cabeçalho (ou do `enum`), a lista de
    /// inicialização e o corpo da parte `this`. Os consumidores veem código
    /// comum. Devolve o `k2`, que fica no lugar da parte `this` (ou depois dos
    /// campos, se não houver).
    fn elaborar_construtor_primario(
        &mut self,
        classe: Name,
        cab: Option<CabecalhoPrimario>,
        members: &mut Vec<MemberId>,
        onde: Elaborando,
    ) -> Option<MemberId> {
        // Partes `this` no corpo.
        let partes: Vec<usize> = members
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(&self.ast.member(**m).kind, MemberKind::Constructor(c) if c.parte_primaria))
            .map(|(i, _)| i)
            .collect();
        let Some(cab) = cab else {
            for &i in &partes {
                let span = self.ast.member(members[i]).span;
                self.diagnostics.push(Diagnostic::new("A primary constructor body requires a primary constructor in the declaration header.", span));
            }
            return None;
        };
        for &i in partes.iter().skip(1) {
            let span = self.ast.member(members[i]).span;
            self.diagnostics.push(Diagnostic::new(
                "Only one primary constructor body is allowed.",
                span,
            ));
        }
        // Construtor generativo não redirecionador no corpo: proibido (o k2 é
        // o único, para os inicializadores de campo poderem ler os
        // parâmetros); o nome do k2 não pode repetir o de outro construtor.
        for &m in members.iter() {
            if let MemberKind::Constructor(c) = &self.ast.member(m).kind {
                if c.parte_primaria {
                    continue;
                }
                let redireciona = c
                    .initializers
                    .iter()
                    .any(|i| matches!(i, Initializer::Redirect { .. }));
                if !c.factory && !redireciona {
                    let span = self.ast.member(m).span;
                    self.diagnostics.push(Diagnostic::new(
                        "A class with a primary constructor can't have a non-redirecting generative constructor.",
                        span,
                    ));
                }
                if c.name.map(|n| n.sym) == cab.nome.map(|n| n.sym) {
                    let span = self.ast.member(m).span;
                    self.diagnostics.push(Diagnostic::new(
                        "The primary constructor already has this name.",
                        span,
                    ));
                }
            }
        }
        let (tem_supertipos, const_) = match onde {
            Elaborando::Classe { tem_supertipos } => (tem_supertipos, cab.const_),
            // Construtor de enum é sempre constante.
            Elaborando::Enum => (true, true),
        };
        let mut campos: Vec<MemberId> = Vec::new();
        let mut parametros = cab.params;
        for p in parametros.iter_mut() {
            if p.covariant && !p.var_ {
                self.diagnostics.push(Diagnostic::new(
                    "A covariant declaring parameter must be declared with 'var'.",
                    p.span,
                ));
            }
            if p.required && p.default_value.is_some() {
                self.diagnostics.push(Diagnostic::com_codigo(
                    codigos::compile_time_error::DEFAULT_VALUE_ON_REQUIRED_PARAMETER,
                    p.span,
                    Vec::<&str>::new(),
                ));
            }
            if !(p.var_ || p.final_) || p.this_ || p.super_ {
                continue;
            }
            let Some(nome) = p.name else { continue };
            let ty = p.ty.or_else(|| {
                self.tipo_do_declarante_sem_tipo(p.default_value, tem_supertipos, p.span)
            });
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
            campos.push(self.ast.push_member(campo));
            // `var T p` → `T this.p` (com o tipo, se escrito).
            p.this_ = true;
            p.var_ = false;
            p.final_ = false;
            p.covariant = false;
        }
        // Lista de inicialização e corpo da parte `this`.
        let (initializers, body, span_parte) = match partes.first() {
            Some(&i) => {
                let mid = members[i];
                let span = self.ast.member(mid).span;
                let MemberKind::Constructor(c) = &mut self.ast.members[mid.0 as usize].kind else {
                    unreachable!()
                };
                let inits = std::mem::take(&mut c.initializers);
                let body = std::mem::replace(&mut c.body, FunctionBody::Empty);
                (inits, body, Some(span))
            }
            None => (Vec::new().into_boxed_slice(), FunctionBody::Empty, None),
        };
        if let (true, FunctionBody::Block(_), Some(span)) = (const_, &body, span_parte) {
            self.diagnostics.push(Diagnostic::new(
                "A constant primary constructor can't have a body.",
                span,
            ));
        }
        let k2 = Member {
            span: span_parte.unwrap_or(cab.span),
            metadata: Vec::new().into_boxed_slice(),
            kind: MemberKind::Constructor(Constructor {
                external: false,
                const_,
                factory: false,
                class_name: classe,
                name: cab.nome,
                parameters: parametros.into_boxed_slice(),
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
        let mut novos = campos;
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
    fn tipo_do_declarante_sem_tipo(
        &mut self,
        padrao: Option<ExprId>,
        tem_supertipos: bool,
        span: Span,
    ) -> Option<TypeId> {
        use crate::ast::{ExprKind, TypeAnnotation, TypeKind};
        if tem_supertipos {
            return None;
        }
        let (nome, anulavel) = match padrao.map(|e| &self.ast.expr(e).kind) {
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
            nullable: anulavel,
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
        let base = self.eat_ident("base");
        self.expect_ident("mixin")?;
        let name_text = self.text();
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_parameters_opt()?;
        let on = if self.eat_ident("on") {
            self.parse_type_list()?
        } else {
            Vec::new()
        };
        let implements = self.parse_implements_opt()?;
        let members = self.parse_class_body_ou_vazio(Some(name_text))?;
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
        let const_primario = self.at_kw(Keyword::Const).then(|| self.advance().span);
        let name_text = self.text();
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_parameters_opt()?;
        let primario = self.parse_cabecalho_primario_opt(enum_token.span, const_primario)?;
        let with = if self.eat_kw(Keyword::With) {
            self.parse_type_list()?
        } else {
            Vec::new()
        };
        let implements = self.parse_implements_opt()?;
        // `enum E(` sem o recurso (biblioteca ≤3.6): o fasta relata
        // `missing_enum_body` e `expected_executable` no `(` e continua depois
        // dele, onde `tipo nome` volta a ser declaração (ver `parse_class`).
        // A declaração parcial vazia mantém o `enum_without_constants` do
        // verificador semântico, como no oráculo.
        if !self.features.tem(Feature::PrimaryConstructors) && self.at_op(Op::LParen) {
            self.erro(codigos::parser::MISSING_ENUM_BODY, &[]);
            self.erro(codigos::parser::EXPECTED_EXECUTABLE, &[]);
            self.advance();
            return Ok(EnumDecl {
                name,
                type_params: type_params.into_boxed_slice(),
                with: with.into_boxed_slice(),
                implements: implements.into_boxed_slice(),
                constants: Vec::new(),
                members: Vec::new(),
                primary_constructor: None,
            });
        }
        self.expect_op(Op::LBrace)?;
        let mut constants = Vec::new();
        while !self.at_op(Op::RBrace) && !self.at_op(Op::Semicolon) && !self.at_eof() {
            constants.push(self.parse_enum_constant()?);
            if !self.eat_op(Op::Comma) {
                break;
            }
        }
        let mut members = if self.eat_op(Op::Semicolon) {
            self.parse_member_list(Some(name_text))?
        } else {
            self.expect_op(Op::RBrace)?;
            Vec::new()
        };
        let primary_constructor =
            self.elaborar_construtor_primario(name, primario, &mut members, Elaborando::Enum);
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

    /// `extension Nome? <T>? on Tipo { membros }`.
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
        self.expect_ident("on")?;
        let on = self.parse_type()?;
        let members = self.parse_class_body_ou_vazio(name_text)?;
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
        let constructor = if self.eat_op(Op::Dot) {
            Some(self.identifier_or_new()?)
        } else {
            None
        };
        self.expect_op(Op::LParen)?;
        let representation_metadata = self.parse_metadata()?;
        // `final` é a forma declarante do construtor primário (3.13); `var`
        // é erro num extension type (a representação não tem setter).
        if self.at_kw(Keyword::Final) {
            let t = self.advance();
            self.exigir(Feature::PrimaryConstructors, t.span);
        } else if self.at_kw(Keyword::Var) {
            let t = self.advance();
            self.diagnostics.push(Diagnostic::com_codigo(
                codigos::parser::REPRESENTATION_FIELD_MODIFIER,
                t.span,
                Vec::<&str>::new(),
            ));
        }
        let representation_type = self.parse_type()?;
        let representation_name = self.expect_identifier()?;
        self.expect_op(Op::RParen)?;
        let implements = self.parse_implements_opt()?;
        let members = self.parse_class_body_ou_vazio(Some(name_text))?;
        Ok(ExtensionTypeDecl {
            const_,
            name,
            type_params: type_params.into_boxed_slice(),
            constructor,
            representation_metadata: representation_metadata.into_boxed_slice(),
            representation_type,
            representation_name,
            implements: implements.into_boxed_slice(),
            members,
        })
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

    /// `typedef F<T> = tipo;` ou a forma antiga `typedef ret? F<T>(params);`.
    fn parse_typedef(&mut self) -> PResult<TypedefDecl> {
        self.expect_ident("typedef")?;
        if self.at_identifier() {
            let after = self.skip_angles(self.pos + 1);
            if self.kind_of(after) == Kind::Op(Op::Assign) {
                let name = self.identifier();
                let type_params = self.parse_type_parameters_opt()?;
                self.expect_op(Op::Assign)?;
                let ty = self.parse_type()?;
                self.expect_op(Op::Semicolon)?;
                return Ok(TypedefDecl {
                    name,
                    type_params: type_params.into_boxed_slice(),
                    kind: TypedefKind::Alias(ty),
                });
            }
            if self.kind_of(after) == Kind::Op(Op::LParen) {
                let name = self.identifier();
                let type_params = self.parse_type_parameters_opt()?;
                let parameters = self.parse_formal_parameters()?;
                self.expect_op(Op::Semicolon)?;
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
        let name = self.expect_identifier()?;
        let type_params = self.parse_type_parameters_opt()?;
        let parameters = self.parse_formal_parameters()?;
        self.expect_op(Op::Semicolon)?;
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

    /// O identificador embutido corrente é usado como modificador (e não
    /// como nome de função/variável)? `late(x)`, `static = 1`, `external;`
    /// são nomes.
    fn modifier_ok(&self) -> bool {
        // `static ({int a, int b}) f()` e `static (int, int)? g()`: o `(` abre
        // um record type de retorno, não a lista de parâmetros de um método
        // chamado `static`. É modificador quando o grupo é seguido de um nome
        // (com `?` opcional entre eles).
        if self.kind_at(1) == Kind::Op(Op::LParen) {
            if let Some(close) = self.matching_close(self.pos + 1) {
                let mut after = close + 1;
                if self.kind_of(after) == Kind::Op(Op::Question) {
                    after += 1;
                }
                return self.kind_of(after) == Kind::Ident;
            }
            return false;
        }
        !matches!(
            self.kind_at(1),
            Kind::Eof
                | Kind::Op(
                    Op::LParen
                        | Op::Assign
                        | Op::Semicolon
                        | Op::Comma
                        | Op::Dot
                        | Op::Lt
                        | Op::Arrow
                        | Op::Question
                        | Op::RParen
                        | Op::RBrace
                        | Op::RBracket
                        | Op::LBracket
                        | Op::Colon
                )
        )
    }

    /// Modificadores em qualquer ordem: `external static abstract covariant
    /// late final const var`.
    fn parse_modifiers(&mut self) -> Modifiers {
        let mut m = Modifiers::default();
        loop {
            match self.kind() {
                Kind::Keyword(Keyword::Final) => m.final_ = true,
                Kind::Keyword(Keyword::Const) => {
                    m.const_ = true;
                    m.const_span = Some(self.span());
                }
                Kind::Keyword(Keyword::Var) => m.var_ = true,
                Kind::Ident if self.modifier_ok() => match self.text() {
                    "external" => {
                        m.external = true;
                        m.external_span = Some(self.span());
                    }
                    "static" => m.static_ = true,
                    "abstract" => {
                        m.abstract_ = true;
                        m.abstract_span = Some(self.span());
                    }
                    "covariant" => m.covariant = true,
                    "late" => m.late = true,
                    _ => break,
                },
                _ => break,
            }
            self.advance();
        }
        m
    }

    /// `get nome`, `set nome`, `operator op` começam aqui? Devolve o tipo do
    /// acessor sem consumir nada.
    fn accessor_follows(&self) -> Option<FunctionKind> {
        if self.at_ident("get") && self.at_identifier_at(1) {
            Some(FunctionKind::Getter)
        } else if self.at_ident("set") && self.at_identifier_at(1) {
            Some(FunctionKind::Setter)
        } else if self.at_ident("operator") && self.operator_follows() {
            Some(FunctionKind::Operator)
        } else {
            None
        }
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
                self.advance();
                op.text()
            }
            _ => return Err(self.erro_identificador()),
        };
        Ok(self.name_from(text, self.span_from(start)))
    }

    /// `tipo? nome (params) corpo`, `tipo? get nome corpo`, `tipo? set nome
    /// (params) corpo`, `tipo? operator op (params) corpo` ou
    /// `tipo? a = 1, b;`. Os modificadores já foram lidos.
    fn parse_function_or_variables(
        &mut self,
        mods: Modifiers,
        start: Span,
        external_topo: Option<Span>,
    ) -> PResult<FunctionOrVariables> {
        if let Some(kind) = self.accessor_follows() {
            let id = self.parse_accessor(mods, start, None, kind, external_topo)?;
            return Ok(FunctionOrVariables::Function(id));
        }
        let ty = if mods.var_ {
            None
        } else if self.at_kw(Keyword::Void) || self.looks_like_type_then_identifier(self.pos) {
            Some(self.parse_type()?)
        } else {
            None
        };
        if let Some(kind) = self.accessor_follows() {
            let id = self.parse_accessor(mods, start, ty, kind, external_topo)?;
            return Ok(FunctionOrVariables::Function(id));
        }
        // `(` sem tipo nem modificadores não abre declaração: um tipo record
        // pediria um nome depois do `)`. É lixo no topo, e o fasta relata
        // `expected_executable` e continua no token seguinte (ver
        // `recover_top_level`), em vez do `missing_identifier` genérico.
        if ty.is_none() && !mods.algum() && self.at_op(Op::LParen) {
            return Err(self.erro(codigos::parser::EXPECTED_EXECUTABLE, &[]));
        }
        let name = self.expect_identifier()?;
        if self.at_op(Op::LParen) || self.at_op(Op::Lt) {
            let type_params = self.parse_type_parameters_opt()?;
            let parameters = self.parse_formal_parameters()?;
            let inicio_corpo = self.pos;
            let (modifier, body) = self.parse_function_body()?;
            self.conferir_corpo_externo(mods.external, false, inicio_corpo, &body, external_topo);
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
        let variables = self.parse_declared_variables_tail(name)?;
        Ok(FunctionOrVariables::Variables(VariableList {
            external: mods.external,
            static_: mods.static_,
            abstract_: mods.abstract_,
            covariant: mods.covariant,
            late: mods.late,
            final_: mods.final_,
            const_: mods.const_,
            var_: mods.var_,
            ty,
            variables: variables.into_boxed_slice(),
        }))
    }

    /// Getter, setter ou operador a partir de `get`/`set`/`operator`.
    fn parse_accessor(
        &mut self,
        mods: Modifiers,
        start: Span,
        return_type: Option<TypeId>,
        kind: FunctionKind,
        external_topo: Option<Span>,
    ) -> PResult<FunctionId> {
        self.advance();
        let (name, parameters) = match kind {
            FunctionKind::Getter => (self.expect_identifier()?, None),
            FunctionKind::Setter => {
                let name = self.expect_identifier()?;
                (name, Some(self.parse_formal_parameters()?))
            }
            _ => {
                let name = self.parse_operator_name()?;
                (name, Some(self.parse_formal_parameters()?))
            }
        };
        let inicio_corpo = self.pos;
        let (modifier, body) = self.parse_function_body()?;
        self.conferir_corpo_externo(mods.external, false, inicio_corpo, &body, external_topo);
        Ok(self.ast.push_function(Function {
            span: self.span_from(start),
            external: mods.external,
            static_: mods.static_,
            kind,
            return_type,
            name: Some(name),
            type_params: Vec::new().into_boxed_slice(),
            parameters: parameters.map(Vec::into_boxed_slice),
            modifier,
            body,
        }))
    }

    /// `= e`? (`, nome = e`?)* `;` a partir do primeiro nome já lido.
    fn parse_declared_variables_tail(&mut self, first: Name) -> PResult<Vec<Variable>> {
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
            name = self.expect_identifier()?;
        }
        self.expect_op(Op::Semicolon)?;
        Ok(variables)
    }

    // -----------------------------------------------------------------------
    // Membros
    // -----------------------------------------------------------------------

    /// `{ membros }` de classe, mixin, extension ou extension type.
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
                self.erro_esperado("}");
                return Ok(members);
            }
            let start_pos = self.pos;
            match self.parse_member(class_name) {
                Ok(id) => members.push(id),
                Err(ParseError) => self.recover_member(start_pos),
            }
        }
    }

    /// Um `classMemberDefinition`: campo, método, acessor, operador ou
    /// construtor. `class_name` decide se `Nome(` é construtor.
    fn parse_member(&mut self, class_name: Option<&'s str>) -> PResult<MemberId> {
        let start = self.span();
        let metadata = self.parse_metadata()?;
        // `this` (parte de construtor primário) e `new` (construtor sem o
        // nome da classe) também iniciam membro, desde a 3.13.
        if !self.can_start_declaration() && !self.at_kw(Keyword::This) && !self.at_kw(Keyword::New)
        {
            return Err(self.erro(codigos::parser::EXPECTED_CLASS_MEMBER, &[]));
        }
        let augment = self.parse_augment_opt();
        let fstart = self.span();
        let mods = self.parse_modifiers();
        let primarios = self.features.tem(Feature::PrimaryConstructors);
        let kind = if self.at_kw(Keyword::This) && !self.at_op_at(1, Op::Dot) {
            // `this : inits? corpo`: parte de corpo do construtor primário.
            self.parse_parte_primaria(fstart)?
        } else if self.at_kw(Keyword::New)
            && (self.at_op_at(1, Op::LParen) || self.at_identifier_at(1))
        {
            // `new nome?(...)` (3.13): construtor com o nome da classe implícito.
            self.parse_construtor_new(mods, class_name)?
        } else if self.at_ident("factory")
            && (self.at_identifier_at(1) || (primarios && self.at_op_at(1, Op::LParen)))
        {
            self.parse_constructor(mods, true, class_name)?
        } else if self.constructor_follows(class_name, mods) {
            self.parse_constructor(mods, false, class_name)?
        } else {
            match self.parse_function_or_variables(mods, fstart, None)? {
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

    /// `Nome(` com o nome da classe (ou após `const`), ou `Nome.x(`.
    fn constructor_follows(&self, class_name: Option<&str>, mods: Modifiers) -> bool {
        if !self.at_identifier() {
            return false;
        }
        if self.at_op_at(1, Op::LParen) {
            return mods.const_ || class_name == Some(self.text());
        }
        self.at_op_at(1, Op::Dot)
            && (self.at_identifier_at(2) || self.at_kw_at(2, Keyword::New))
            && self.at_op_at(3, Op::LParen)
    }

    /// `factory? C(.nome)? (params) (= Alvo.nome; | (: inits)? corpo)`. Na
    /// 3.13 também `factory nome?(...)`: sem o nome da classe (`factory(`) é
    /// o construtor sem nome, `factory id(` é `C.id` — salvo `id` = `C`, que
    /// continua o sem nome (spec, a exceção para não quebrar código antigo).
    fn parse_constructor(
        &mut self,
        mods: Modifiers,
        factory: bool,
        classe: Option<&'s str>,
    ) -> PResult<MemberKind> {
        if factory {
            let t = self.advance();
            if self.at_op(Op::LParen) {
                // `factory(...)`: só chega aqui na 3.13 (antes é método).
                let class_name = self.nome_da_classe(classe, t.span);
                return self.parse_constructor_resto(mods, true, class_name, None);
            }
            if self.features.tem(Feature::PrimaryConstructors) && self.at_op_at(1, Op::LParen) {
                let id = self.expect_identifier()?;
                let class_name = self.nome_da_classe(classe, t.span);
                let nome = (Some(&self.source[id.span.start..id.span.end]) != classe).then_some(id);
                return self.parse_constructor_resto(mods, true, class_name, nome);
            }
        }
        let class_name = self.expect_identifier()?;
        let name = if self.eat_op(Op::Dot) {
            Some(self.identifier_or_new()?)
        } else {
            None
        };
        self.parse_constructor_resto(mods, factory, class_name, name)
    }

    /// O `Name` da classe dona para um construtor escrito sem ele (`new`,
    /// `factory(`, parte `this`), com o intervalo da palavra-chave.
    fn nome_da_classe(&mut self, classe: Option<&str>, span: Span) -> Name {
        let texto = classe.unwrap_or("").to_string();
        self.name_from(&texto, span)
    }

    /// `new nome?(params) (: inits)? corpo` (3.13): a mesma coisa que
    /// `C.nome(...)`/`C(...)`.
    fn parse_construtor_new(
        &mut self,
        mods: Modifiers,
        classe: Option<&'s str>,
    ) -> PResult<MemberKind> {
        let t = self.advance();
        self.exigir(Feature::PrimaryConstructors, t.span);
        let class_name = self.nome_da_classe(classe, t.span);
        let nome = if self.at_identifier() {
            Some(self.identifier())
        } else {
            None
        };
        self.parse_constructor_resto(mods, false, class_name, nome)
    }

    /// `this (: inits)? corpo` (3.13): a parte de corpo do construtor
    /// primário. Vira um `Constructor { parte_primaria: true }` que a
    /// elaboração funde no `k2` da declaração. `async`/`sync*`/`=>` são erro.
    fn parse_parte_primaria(&mut self, inicio: Span) -> PResult<MemberKind> {
        let t = self.advance();
        self.exigir(Feature::PrimaryConstructors, t.span);
        let mut initializers = Vec::new();
        if self.eat_op(Op::Colon) {
            loop {
                initializers.push(self.parse_initializer()?);
                if !self.eat_op(Op::Comma) {
                    break;
                }
            }
        }
        let (modificador, body) = self.parse_function_body()?;
        if modificador != AsyncModifier::None || matches!(body, FunctionBody::Expression(_)) {
            self.diagnostics.push(Diagnostic::new(
                "A primary constructor body must be a block without 'async' or 'sync*', or ';'.",
                self.span_from(inicio),
            ));
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
    fn parse_constructor_resto(
        &mut self,
        mods: Modifiers,
        factory: bool,
        class_name: Name,
        name: Option<Name>,
    ) -> PResult<MemberKind> {
        let parameters = self.parse_formal_parameters()?;
        let mut initializers = Vec::new();
        let mut redirect = None;
        let body = if factory && self.eat_op(Op::Assign) {
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
            self.expect_op(Op::Semicolon)?;
            FunctionBody::Empty
        } else {
            if self.eat_op(Op::Colon) {
                loop {
                    initializers.push(self.parse_initializer()?);
                    if !self.eat_op(Op::Comma) {
                        break;
                    }
                }
            }
            let inicio_corpo = self.pos;
            let body = self.parse_function_body()?.1;
            self.conferir_corpo_externo(mods.external, factory, inicio_corpo, &body, None);
            if mods.const_ && !factory {
                let delimitador = match body {
                    FunctionBody::Block(_) => Some(Op::LBrace),
                    FunctionBody::Expression(_) => Some(Op::Arrow),
                    FunctionBody::Empty | FunctionBody::Native(_) => None,
                };
                if let Some(op) = delimitador {
                    if let Some(span) = self.tokens[inicio_corpo..self.pos]
                        .iter()
                        .find(|t| t.kind == Kind::Op(op))
                        .map(|t| t.span)
                    {
                        self.erro_em(codigos::parser::CONST_CONSTRUCTOR_WITH_BODY, span, &[]);
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
                self.erro_em(codigos::parser::CONST_FACTORY, span, &[]);
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
        let start = self.span();
        if self.eat_kw(Keyword::Super) {
            let constructor = if self.eat_op(Op::Dot) {
                Some(self.identifier_or_new()?)
            } else {
                None
            };
            let arguments = self.parse_arguments()?;
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
                    return Ok(Initializer::Redirect {
                        span: self.span_from(start),
                        constructor: Some(name),
                        arguments,
                    });
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
            let arguments = self.parse_arguments()?;
            return Ok(Initializer::Redirect {
                span: self.span_from(start),
                constructor: None,
                arguments,
            });
        }
        if self.eat_kw(Keyword::Assert) {
            self.expect_op(Op::LParen)?;
            let condition = self.parse_expression()?;
            let message = if self.eat_op(Op::Comma) && !self.at_op(Op::RParen) {
                Some(self.parse_expression()?)
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

    // -----------------------------------------------------------------------
    // Corpos de função
    // -----------------------------------------------------------------------

    /// O parser oficial relata função de topo no token `external`, mas membro
    /// e construtor no `{` do bloco ou no `=>` (`parseTopLevelMethod` e
    /// `parseMethod`/`parseFactoryMethod` do fasta).
    /// O intervalo desde `inicio` começa antes do modificador `async` e só
    /// inclui tokens desta função, então o primeiro delimitador é o corpo.
    fn conferir_corpo_externo(&mut self, external: bool, factory: bool, inicio: usize, body: &FunctionBody, external_topo: Option<Span>) {
        if !external {
            return;
        }
        let delimitador = match body {
            FunctionBody::Block(_) => Op::LBrace,
            FunctionBody::Expression(_) => Op::Arrow,
            FunctionBody::Empty | FunctionBody::Native(_) => return,
        };
        let span = self.tokens[inicio..self.pos]
            .iter()
            .find(|t| t.kind == Kind::Op(delimitador))
            .map(|t| t.span);
        if let Some(span) = external_topo.or(span) {
            let codigo = if factory {
                codigos::parser::EXTERNAL_FACTORY_WITH_BODY
            } else {
                codigos::parser::EXTERNAL_METHOD_WITH_BODY
            };
            self.erro_em(codigo, span, &[]);
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
            return Ok(FunctionBody::Block(self.parse_block()?));
        }
        if self.eat_op(Op::Arrow) {
            let expr = self.parse_expression()?;
            if expect_semicolon {
                self.expect_op(Op::Semicolon)?;
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
            self.expect_op(Op::Semicolon)?;
            return Ok(FunctionBody::Native(name));
        }
        Err(self.erro(codigos::parser::MISSING_FUNCTION_BODY, &[]))
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        AsyncModifier, DeclKind, DirectiveKind, FunctionBody, FunctionKind, MemberKind, Name,
    };
    use crate::parser::{Parsed, Parser, parse};
    use dartforge_intern::Interner;

    fn parse_ok(src: &str, names: &mut Interner) -> Parsed {
        let out = parse(src, names);
        assert!(out.diagnostics.is_empty(), "{src}: {:?}", out.diagnostics);
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

    #[test]
    fn corpo_externo_marca_abertura_do_bloco_ou_seta_com_codigo_oficial() {
        use dartforge_diagnostics::codigos::parser as c;
        for (src, token, codigo) in [
            ("external int f() => 1;", "external", c::EXTERNAL_METHOD_WITH_BODY),
            ("class C { external C() {} }", "{}", c::EXTERNAL_METHOD_WITH_BODY),
            ("class C { external factory C.x() => C(); }", "=>", c::EXTERNAL_FACTORY_WITH_BODY),
            ("class C { external factory C.x() {} }", "{}", c::EXTERNAL_FACTORY_WITH_BODY),
            ("class C { external int get v => 1; }", "=>", c::EXTERNAL_METHOD_WITH_BODY),
        ] {
            let mut names = Interner::new();
            let out = parse(src, &mut names);
            let inicio = src.find(token).unwrap();
            let fim = inicio + if token == "{}" { 1 } else { token.len() };
            assert!(out.diagnostics.iter().any(|d|
                d.code == Some(codigo) && d.span.start == inicio && d.span.end == fim
            ), "{src}: {:?}", out.diagnostics);
            assert_eq!(out.diagnostics.len(), 1, "{src}: {:?}", out.diagnostics);
        }
        let mut names = Interner::new();
        let out = parse("external int f(); class C { external factory C.x(); }", &mut names);
        assert!(!out.diagnostics.iter().any(|d| matches!(d.code,
            Some(c::EXTERNAL_METHOD_WITH_BODY | c::EXTERNAL_FACTORY_WITH_BODY))));
    }

    #[test]
    fn corpo_externo_bate_com_duas_amostras_do_oraculo_gravado() {
        use dartforge_diagnostics::codigos::parser as c;
        for (src, codigo, inicio, mensagem, correcao) in [
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
                d.code == Some(codigo) && d.span.start == inicio && d.span.end == inicio + 2
                    && d.message == mensagem && d.correcao().as_deref() == correcao
            ), "{src}: {:?}", out.diagnostics);
        }
    }

    #[test]
    fn funcao_externa_de_topo_aponta_para_external_como_fasta() {
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
    fn campo_abstract_static_sem_augmentations_aponta_para_abstract() {
        use crate::features::{Feature, LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/executable_body/ExecutableBody__class_staticField_abstr_5d2b9f54.dart"));
        let span = fonte.find("abstract int foo").unwrap();
        let mut nomes = Interner::new();
        let sem_recurso = parse_com(fonte, &mut nomes, LibraryFeatures::new(LanguageVersion::PISO, &[]));
        assert!(sem_recurso.diagnostics.iter().any(|d|
            d.code == Some(c::ABSTRACT_STATIC_FIELD)
                && d.span.start == span && d.span.end == span + "abstract".len()
                && d.message == "Static fields can't be declared 'abstract'."
                && d.correcao().as_deref() == Some("Try removing the 'abstract' or 'static' keyword.")
        ), "{:?}", sem_recurso.diagnostics);

        let com_recurso = parse_com(fonte, &mut nomes, LibraryFeatures::new(LanguageVersion::PISO, &[Feature::Augmentations]));
        assert!(!com_recurso.diagnostics.iter().any(|d| d.code == Some(c::ABSTRACT_STATIC_FIELD)));
    }

    #[test]
    fn corpo_de_construtor_const_aponta_para_delimitador_como_analyzer() {
        use dartforge_diagnostics::codigos::parser as c;
        for (fonte, inicio, fim) in [
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
            let mut nomes = Interner::new();
            let parsed = parse(fonte, &mut nomes);
            assert!(parsed.diagnostics.iter().any(|d|
                d.code == Some(c::CONST_CONSTRUCTOR_WITH_BODY)
                    && d.span.start == inicio && d.span.end == fim
                    && d.message == "Const constructors can't have a body."
                    && d.correcao().as_deref() == Some("Try removing either the 'const' keyword or the body.")
            ), "{fonte}: {:?}", parsed.diagnostics);
        }

        let mut nomes = Interner::new();
        let parsed = parse("class C { const C(); const factory C.x() => C(); }", &mut nomes);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_CONSTRUCTOR_WITH_BODY)));
    }

    #[test]
    fn const_factory_sem_redirecionamento_aponta_para_const() {
        use dartforge_diagnostics::codigos::parser as c;
        for (fonte, inicio) in [
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
            let mut nomes = Interner::new();
            let parsed = parse(fonte, &mut nomes);
            assert!(parsed.diagnostics.iter().any(|d|
                d.code == Some(c::CONST_FACTORY) && d.span.start == inicio && d.span.end == inicio + 5
                    && d.message == "Only redirecting factory constructors can be declared to be 'const'."
                    && d.correcao().as_deref() == Some("Try removing the 'const' keyword, or replacing the body with '=' followed by a valid target.")
            ), "{fonte}: {:?}", parsed.diagnostics);
        }

        let mut nomes = Interner::new();
        let parsed = parse("class A { const A(); const factory A.named() = A; }", &mut nomes);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_FACTORY)));

        // Declarações do SDK 3.6.2: lib/core/bool.dart e lib/core/int.dart.
        let sdk = "class bool { external const factory bool.fromEnvironment(String name, {bool defaultValue = false}); external const factory bool.hasEnvironment(String name); } class int { external const factory int.fromEnvironment(String name, {int defaultValue = 0}); }";
        let parsed = parse(sdk, &mut nomes);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_FACTORY)), "{:?}", parsed.diagnostics);

        let corpus = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/linguagem/const/native_factory_test.dart"));
        let parsed = parse(corpus, &mut nomes);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_FACTORY)), "{:?}", parsed.diagnostics);
    }

    // -- Independentes dos outros módulos -----------------------------------

    #[test]
    fn script_tag_e_library() {
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
    fn library_sem_nome_e_part_of_pontuado() {
        let mut names = Interner::new();
        let out = parse_ok("library; part of a.b;", &mut names);
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
    fn modificadores_de_classe() {
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
    fn enums_sem_argumentos() {
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
    fn campos_com_modificadores() {
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
    fn getters_abstratos_e_native() {
        let mut names = Interner::new();
        let src = "class A { get x; static get y; external get z; get w native; }";
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
    fn variaveis_de_topo_sem_tipo() {
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
    fn metadata_sem_argumentos() {
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
    fn recuperacao_no_nivel_de_topo() {
        let mut names = Interner::new();
        let out = parse(
            "class {} class A {} enum {} class B {} class {}",
            &mut names,
        );
        assert_eq!(out.diagnostics.len(), 3, "{:?}", out.diagnostics);
        assert_eq!(out.unit.declarations.len(), 2);
        assert_eq!(text(&names, class(&out, 0).name), "A");
        assert_eq!(text(&names, class(&out, 1).name), "B");
    }

    #[test]
    fn recuperacao_pula_o_corpo_da_declaracao_quebrada() {
        let mut names = Interner::new();
        let out = parse("class A B { var x; } class C {}", &mut names);
        assert_eq!(out.diagnostics.len(), 1, "{:?}", out.diagnostics);
        assert_eq!(out.unit.declarations.len(), 1);
        assert_eq!(text(&names, class(&out, 0).name), "C");
    }

    #[test]
    fn recuperacao_entre_membros() {
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
    fn tokens_soltos_no_topo() {
        let mut names = Interner::new();
        let out = parse("; } class A {}", &mut names);
        assert_eq!(out.diagnostics.len(), 2, "{:?}", out.diagnostics);
        assert_eq!(out.unit.declarations.len(), 1);
    }

    /// `class A(` em biblioteca ≤3.6 (resto de sintaxe 3.13): o fasta relata
    /// o corpo ausente no token anterior e um `expected_executable` por token
    /// solto, sem engolir a declaração — ver oráculo `unused_element/
    /// UnusedElement__classPrivate_primaryCons_43fdcc49.dart`.
    #[test]
    fn topo_fasta_classe_com_cabecalho_primario_em_3_6() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "class A([int? a]);\nclass _B([super.a]) extends A;\nvar b = _B();\n";
        let mut nomes = Interner::new();
        let piso = LibraryFeatures::new(LanguageVersion::PISO, &[]);
        let out = parse_com(fonte, &mut nomes, piso);
        let corpo = out
            .diagnostics
            .iter()
            .find(|d| d.code == Some(c::EXPECTED_CLASS_BODY))
            .expect("expected_class_body");
        assert_eq!((corpo.span.start, corpo.span.end), (6, 7));
        assert_eq!(
            corpo.message,
            "A class declaration must have a body, even if it is empty."
        );
        for (inicio, fim) in [(7, 8), (8, 9)] {
            assert!(
                out.diagnostics.iter().any(|d|
                    d.code == Some(c::EXPECTED_EXECUTABLE)
                        && d.span.start == inicio
                        && d.span.end == fim),
                "{fonte}: {:?}",
                out.diagnostics
            );
        }
        assert!(
            out.diagnostics.iter().any(|d|
                d.code == Some(c::EXPECTED_TOKEN)
                    && d.span.start == 14
                    && d.message == "Expected to find ';'."),
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert!(
            !out.diagnostics.iter().any(|d| d.code == Some(c::EXPERIMENT_NOT_ENABLED)),
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert!(matches!(decl(&out, 0), DeclKind::Class(a) if text(&nomes, a.name) == "A" && a.members.is_empty()));
        assert!(matches!(decl(&out, 1), DeclKind::Class(b) if text(&nomes, b.name) == "_B" && b.members.is_empty()));
        // O `var b` do fim sobrevive; `extends A` pode virar uma declaração
        // de variáveis espúria (o lookahead aceita `extends` como tipo —
        // pendente, mesma família do `missing_const_final_var_or_type`).
        let ultimo = out.unit.declarations.len() - 1;
        assert!(matches!(decl(&out, ultimo), DeclKind::Variables(l) if l.variables.iter().any(|v| text(&nomes, v.name) == "b")));

        // Com o recurso ligado (3.13), a mesma fonte é limpa.
        let limpo = parse(fonte, &mut nomes);
        assert!(limpo.diagnostics.is_empty(), "{:?}", limpo.diagnostics);
    }

    /// `enum E(` em biblioteca ≤3.6: `missing_enum_body` e
    /// `expected_executable` no `(`, continua depois dele — ver oráculo
    /// `assert_in_redirecting_constructor/
    /// AssertInRedirectingConstructor__enum_pr_bf07cd7e.dart`.
    #[test]
    fn topo_fasta_enum_com_cabecalho_primario_em_3_6() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "enum E(int x);\nvar b = 0;\n";
        let mut nomes = Interner::new();
        let piso = LibraryFeatures::new(LanguageVersion::PISO, &[]);
        let out = parse_com(fonte, &mut nomes, piso);
        assert!(
            out.diagnostics.iter().any(|d|
                d.code == Some(c::MISSING_ENUM_BODY)
                    && d.span.start == 6
                    && d.span.end == 7
                    && d.message == "An enum definition must have a body with at least one constant name."),
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert!(
            out.diagnostics.iter().any(|d|
                d.code == Some(c::EXPECTED_EXECUTABLE) && d.span.start == 6 && d.span.end == 7),
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert!(
            out.diagnostics.iter().any(|d|
                d.code == Some(c::EXPECTED_TOKEN) && d.span.start == 11),
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert!(
            !out.diagnostics.iter().any(|d| d.code == Some(c::EXPERIMENT_NOT_ENABLED)),
            "{fonte}: {:?}",
            out.diagnostics
        );
        assert!(matches!(decl(&out, 0), DeclKind::Enum(e) if text(&nomes, e.name) == "E" && e.constants.is_empty()));
        assert_eq!(out.unit.declarations.len(), 2);

        let limpo = parse("enum E(int x) { v; }\n", &mut nomes);
        assert!(limpo.diagnostics.is_empty(), "{:?}", limpo.diagnostics);
    }

    /// `(` sem tipo nem modificadores não abre declaração no topo: é
    /// `expected_executable`, não `missing_identifier`.
    #[test]
    fn topo_fasta_parentese_solto_nao_e_identificador() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_com;
        use dartforge_diagnostics::codigos::parser as c;

        let fonte = "(42);\nvar b = 0;\n";
        let mut nomes = Interner::new();
        let piso = LibraryFeatures::new(LanguageVersion::PISO, &[]);
        let out = parse_com(fonte, &mut nomes, piso);
        let primeiro = &out.diagnostics[0];
        assert_eq!(primeiro.code, Some(c::EXPECTED_EXECUTABLE));
        assert_eq!((primeiro.span.start, primeiro.span.end), (0, 1));
        assert!(
            !out.diagnostics.iter().any(|d| d.code == Some(c::MISSING_IDENTIFIER)),
            "{fonte}: {:?}",
            out.diagnostics
        );
        let ultimo = out.unit.declarations.len() - 1;
        assert!(matches!(decl(&out, ultimo), DeclKind::Variables(_)));
    }

    #[test]
    fn corpo_de_funcao_vazio_e_native() {
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
    fn corpo_de_funcao_ausente_restaura_contexto() {
        let mut names = Interner::new();
        let src = "async )";
        let tokens = crate::lexer::lex(src).unwrap();
        let mut p = Parser::new(src, tokens, &mut names);
        assert!(p.parse_function_body().is_err());
        assert_eq!(p.diagnostics.len(), 1);
        assert!(!p.in_async);
    }

    #[test]
    fn nome_de_operador_composto() {
        let mut names = Interner::new();
        for (src, esperado) in [
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
            assert_eq!(text(&names, name), esperado);
        }
    }

    // -- Dependentes de types/expressions/statements ------------------------
    //
    // Estes testes panicam com `not yet implemented` enquanto os outros
    // módulos são stubs; passam quando eles existirem.
    mod dependentes {
        use super::*;
        use crate::ast::{Combinator, Initializer, TypedefKind};

        #[test]
        fn diretivas_com_uri() {
            let mut names = Interner::new();
            let src = "import 'a.dart'; import 'b.dart' deferred as b show x, y hide z; \
                       import 'c.dart' if (dart.library.io == 'x') 'd.dart' if (dart.library.html) 'e.dart' as c; \
                       export 'f.dart' show q; part 'g.dart'; part of 'h.dart';";
            let out = parse_ok(src, &mut names);
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
        fn cabecalhos_de_classe_e_mixin_application() {
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
        fn enum_completo() {
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
        fn extensions_e_extension_types() {
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
        fn funcoes_e_variaveis_de_topo() {
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
        fn campo_chamado_operator() {
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
        fn membros_metodos_e_operadores() {
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
            let esperados = [
                "+", "[]=", "[]", "==", ">=", ">>", ">>>", "<<", "<=", "<", ">", "-", "~", "~/",
            ];
            for (k, esperado) in esperados.iter().enumerate() {
                assert_eq!(op(14 + k), *esperado);
            }
            assert!(matches!(method(28).body, FunctionBody::Native(None)));
            assert!(matches!(method(29).body, FunctionBody::Native(Some(_))));
        }

        #[test]
        fn construtores() {
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
        fn recuperacao_dentro_de_corpo_de_metodo() {
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
        fn metadata_com_argumentos() {
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
        fn augment_em_topo_e_membros() {
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
        fn augment_e_nome_comum_sem_o_recurso() {
            // Sem `augmentations`/`macros`, `augment` continua identificador:
            // código 3.6 que o usa como nome não muda de sentido.
            let mut names = Interner::new();
            let out = com(&[], "var augment = 1; int f() => augment; void augment2() {}", &mut names);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            assert!(!out.ast.decl(out.unit.declarations[0]).augment);
            // A forma inequívoca é lida, com o diagnóstico de recurso desligado.
            let out = com(&[], "augment class C {}", &mut names);
            assert_eq!(out.diagnostics.len(), 1, "{:?}", out.diagnostics);
            assert!(out.diagnostics[0].message.contains("augmentations"));
            assert!(out.ast.decl(out.unit.declarations[0]).augment);
        }

        #[test]
        fn macro_class_e_diretivas_da_forma_36() {
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
        fn texto_gerado_pelo_cfe_para_o_json_codable() {
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
