//! Compilation unit, directives, metadata, top-level declarations, class
//! members, constructors and function bodies (Dart 3.6, without
//! resolution).
//!
//! Decisions that are not a direct translation of the grammar:
//!
//! * **Error recovery.** A top-level declaration that fails without
//!   consuming anything records a single diagnostic and skips one token
//!   (fasta reports one error per token: `expected_executable`); with
//!   progress, the cursor is synchronized up to the next plausible boundary
//!   (`;` or `}` at the nesting level where the declaration started, or a
//!   token that starts a top-level declaration). Member recovery follows the
//!   same design, one member at a time. 3.7+ syntax in a ≤3.6 library is the
//!   superset with `experiment_not_enabled`, and **not** fasta 3.6.2's
//!   cascade: the tool is 3.13 (D2), and the 3.13.4 analyzer reports the
//!   same in the same library and keeps analyzing (probed on 2026-09-25; the
//!   oracle for those files is 3.13.4, `corpus/diagnosticos/sintaxe-nova.json`).
//!   The exception is `augment` without the experiment: fasta's scanner
//!   makes it a plain identifier, and the cascade applies in both SDKs. The
//!   declaration lookahead still rejects `]`/`}` as a continuation
//!   (`int? a]` becomes `expected_token` at the type, not at the name).
//! * **Constructor × method.** `Name(` is a constructor when `Name` is the
//!   current class's (the name is passed to the member parser); `Name.x(` is
//!   always a constructor, because methods have no dotted name; `factory`
//!   and `const Name(` likewise.
//! * **Function × variable.** After `type? name`, `(` or `<` opens a
//!   function; anything else is a variable list. `get`/`set` followed by an
//!   identifier are a getter/setter even without a return type, because they
//!   are built-in identifiers and cannot name a type.
//! * **Metadata.** `@Name (` is only an argument list if the `(` is glued to
//!   the name (or there are type arguments), as the SDK parser does —
//!   otherwise the `(` starts a record type of the next declaration.
//! * **Factory redirection.** In `= D.x;` the `D.x` is first read as a type
//!   (possibly `prefix.Type`); `.x` only becomes a constructor name if it is
//!   left over after the type. Telling `prefix.Type` from `Type.constructor`
//!   needs resolution.
//! * **Directive order.** It is not enforced: any directive is accepted
//!   anywhere at top level (superset; the next phase validates).
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
use dartforge_diagnostics::{Diagnostic, Span, codes};

/// Header of a primary constructor already read (Dart 3.13).
struct PrimaryHeader {
    /// From the `class`/`enum` word to the end of the parameters.
    span: Span,
    const_: bool,
    /// `.id`; `None` without a name or with `.new`.
    name: Option<Name>,
    params: Vec<crate::ast::Parameter>,
}

/// Which declaration is being elaborated.
#[derive(Clone, Copy)]
enum Elaborating {
    /// `class`: `has_supertypes` turns off the syntactic type of an untyped
    /// declaring parameter (an inherited getter would decide).
    Class { has_supertypes: bool },
    /// `enum`: the constructor is always constant.
    Enum,
}

/// Modifiers that may precede a member or a top-level declaration.
///
/// They are read in any order (superset): the grammar fixes an order, but
/// rejecting the wrong order is the job of the next phase.
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
    /// The modifier tokens, for fasta's diagnostics.
    modifier_tokens: ModifierTokens,
}

impl Modifiers {
    /// Was any modifier read?
    fn any_(self) -> bool {
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

/// What starts at the top-level cursor ([`Parser::top_level_route`]).
enum TopLevelRoute {
    /// No modifiers before the top-level word: the usual dispatch.
    Plain,
    /// A top-level member (function, getter, setter or variables).
    Member,
    /// A top-level word at position `word`, with modifiers before it.
    Word { word: usize },
}

/// Result of `type? name …`: a function/method/accessor or a variable list.
enum FunctionOrVariables {
    Function(FunctionId),
    Variables(VariableList),
}

impl<'s, 'i> Parser<'s, 'i> {
    // -----------------------------------------------------------------------
    // Compilation unit
    // -----------------------------------------------------------------------

    /// Reads the whole unit, with recovery at declaration boundaries.
    ///
    /// Never fails: each declaration that cannot be read becomes a diagnostic
    /// and parsing continues at the next plausible boundary.
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

    /// A directive or a top-level declaration, with its metadata.
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
        // `checkDeclaration`: after a declaration, a directive is an error.
        if self.directive_state != super::DirectiveState::PartOf {
            self.directive_state = super::DirectiveState::Declarations;
        }
        let augment = self.parse_augment_opt();
        // A stray `;` at top level is `unexpected_token` in fasta 3.6.2
        // ("Unexpected text ';'"), not `expected_executable` (which applies to `)`,
        // `]` and `}`). In a class body the `;` stays `expected_class_member`.
        if self.at_op(Op::Semicolon) {
            let span = self.span();
            // Without consuming: recovery without progress skips the token, as with the
            // other stray-token errors (see `recover_top_level`).
            return Err(self.error_at(codes::parser::UNEXPECTED_TOKEN, span, &[";"]));
        }
        if !self.can_start_declaration() {
            return Err(self.error(codes::parser::EXPECTED_EXECUTABLE, &[]));
        }
        let id = self.parse_top_level_declaration(start, metadata, augment)?;
        unit.declarations.push(id);
        Ok(())
    }

    /// Directive order (fasta's `DirectiveContext`, `directive_context.dart`):
    /// `import`/`export` after `part` are
    /// `IMPORT_DIRECTIVE_AFTER_PART_DIRECTIVE`/`EXPORT_DIRECTIVE_AFTER_PART_DIRECTIVE`;
    /// any directive after a declaration, `DIRECTIVE_AFTER_DECLARATION`;
    /// `library` not at the start,
    /// `LIBRARY_DIRECTIVE_NOT_FIRST`/`MULTIPLE_LIBRARY_DIRECTIVES`; in a
    /// `part of`, another directive is `NON_PART_OF_DIRECTIVE_IN_PART` (except
    /// with `enhanced-parts` or `macros`) and another `part of`,
    /// `MULTIPLE_PART_OF_DIRECTIVES`. The error goes at the word that opens the
    /// directive.
    fn check_directive_order(&mut self, kind: &DirectiveKind, word: Span) {
        use super::DirectiveState as E;
        use codes::parser as c;
        // `macros` brings `enhanced-parts` (the experiment depends on it): the
        // materialized `*.macro.dart` is a `part of` with `import`s.
        let parts = self.features.has(Feature::EnhancedParts) || self.features.has(Feature::Macros);
        let state = self.directive_state;
        let error = match kind {
            DirectiveKind::Import { .. } | DirectiveKind::Export { .. } => {
                let import = matches!(kind, DirectiveKind::Import { .. });
                match state {
                    E::NoneFound | E::Script | E::Library | E::ImportExport => {
                        self.directive_state = E::ImportExport;
                        None
                    }
                    E::Part => Some(if import { c::IMPORT_DIRECTIVE_AFTER_PART_DIRECTIVE } else { c::EXPORT_DIRECTIVE_AFTER_PART_DIRECTIVE }),
                    E::PartOf if parts => {
                        self.directive_state = E::ImportExport;
                        None
                    }
                    E::PartOf => Some(c::NON_PART_OF_DIRECTIVE_IN_PART),
                    E::Declarations => Some(c::DIRECTIVE_AFTER_DECLARATION),
                }
            }
            DirectiveKind::Part { .. } => match state {
                E::NoneFound | E::Script | E::Library | E::ImportExport | E::Part => {
                    self.directive_state = E::Part;
                    None
                }
                E::PartOf if parts => {
                    self.directive_state = E::ImportExport;
                    None
                }
                E::PartOf => Some(c::NON_PART_OF_DIRECTIVE_IN_PART),
                E::Declarations => Some(c::DIRECTIVE_AFTER_DECLARATION),
            },
            DirectiveKind::Library { .. } => {
                if state < E::Library {
                    self.directive_state = E::Library;
                    None
                } else if state == E::Library {
                    Some(c::MULTIPLE_LIBRARY_DIRECTIVES)
                } else if state == E::PartOf {
                    Some(c::NON_PART_OF_DIRECTIVE_IN_PART)
                } else {
                    Some(c::LIBRARY_DIRECTIVE_NOT_FIRST)
                }
            }
            DirectiveKind::PartOf { .. } => {
                if state == E::NoneFound {
                    self.directive_state = E::PartOf;
                    None
                } else if state == E::PartOf {
                    Some(c::MULTIPLE_PART_OF_DIRECTIVES)
                } else {
                    Some(c::NON_PART_OF_DIRECTIVE_IN_PART)
                }
            }
            _ => None,
        };
        if let Some(code) = error {
            self.error_at(code, word, &[]);
        }
    }

    /// The `augment` modifier of a top-level declaration or a member
    /// (docs/AUGMENTATIONS.md), consumed if present.
    ///
    /// `augment` is a built-in identifier only where the feature exists: with
    /// `augmentations` or `macros` on, it is a modifier when it comes before
    /// another word (`augment class`, `augment void f()`, `augment C.x(`).
    /// Without them, it is always a plain identifier, as in fasta's scanner
    /// (`abstract_scanner.dart`: `!_enableAugmentations && keyword ==
    /// Keyword.AUGMENT` becomes `tokenizeIdentifier`). Even `augment class C {}`
    /// falls into top-level recovery (`expected_token` and
    /// `missing_const_final_var_or_type` at the `augment`), without
    /// `experiment_not_enabled`: that is what the 3.13.4 and 3.6.2 analyzers
    /// report.
    fn parse_augment_opt(&mut self) -> bool {
        if !self.at_ident("augment") {
            return false;
        }
        let enabled_flag =
            self.features.has(Feature::Augmentations) || self.features.has(Feature::Macros);
        if !enabled_flag || !matches!(self.kind_at(1), Kind::Ident | Kind::Keyword(_)) {
            return false;
        }
        self.advance();
        true
    }

    /// Can the current token start a top-level declaration or a member?
    ///
    /// Used to fail early on stray tokens (`;`, `}`, `)`) without asking the
    /// type *lookaheads*.
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

    /// `()[]{}` depth accumulated between two token positions.
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

    /// Synchronizes after a top-level declaration failed at `start_pos`.
    ///
    /// Without progress (nothing consumed: a stray token), one token is enough —
    /// fasta reports one error per token (`expected_executable`) and continues
    /// with the next one; swallowing up to `;`/`}` would hide the valid
    /// declarations in the middle of the garbage (cascading false negatives).
    /// With progress, skips until the level where the declaration started
    /// closes (`;` or `}` at depth zero) or until a token that starts a
    /// top-level declaration. Guarantees progress: consumes at least one token.
    fn recover_top_level(&mut self, start_pos: usize) {
        if self.pos == start_pos && !self.at_eof() {
            // `parseInvalidTopLevelDeclaration`: a `{` takes the whole block
            // (`parseInvalidBlock`, up to the `endGroup`).
            if self.at_op(Op::LBrace)
                && let Some(closes) = self.matching_close(self.pos)
            {
                let from = self.pos;
                self.pos = closes + 1;
                self.record_skipped(from);
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
                    // A stray closer at level zero is itself a boundary and is flagged
                    // separately (`expected_executable` at it, as fasta does): it is not
                    // consumed here — the next top-level round reports it and recovery without
                    // progress skips it. Advancing would hide the error.
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

    /// Is the current token a plausible start of a top-level declaration (for
    /// error synchronization)?
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

    /// Synchronizes after a member failed at `start_pos`, without leaving the
    /// body.
    ///
    /// Mirrors [`Parser::recover_top_level`]: without progress (nothing
    /// consumed: a stray token), one token is enough — fasta reports one error
    /// per token (`expected_class_member`) and continues with the next one;
    /// swallowing up to `;` would hide the valid members in the middle of the
    /// garbage (`42` before `int x = 1;` erased the field). With progress, skips
    /// until the level where the member started closes (`;` or `}` at depth
    /// zero) or until a token that starts a member — where it tries the rest as
    /// a member, speculatively: if that works (e.g. the `set foo...` tail of
    /// `augment static set foo...` without the feature), it stays and the
    /// following members survive; if it fails, the attempt's diagnostics are
    /// discarded and the scan continues the old way, without a new stop (one
    /// error for the broken member, not one per token of the rest). The `}`
    /// that closes the class is **not** consumed, so that the body loop ends.
    /// Guarantees progress: consumes at least one token.
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
                    // A stray closer at level zero is itself a boundary and is flagged
                    // separately (`expected_class_member` at it, as fasta does): it is not
                    // consumed here — the next body round reports it and recovery without
                    // progress skips it. Advancing would hide the error.
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
                    let mark = self.diagnostics.len();
                    self.speculating = true;
                    let r = self.parse_member(class_name);
                    self.speculating = false;
                    match r {
                        Ok(id) => {
                            members.push(id);
                            return;
                        }
                        Err(ParseError) => {
                            self.diagnostics.truncate(mark);
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

    /// Is the current token a plausible start of a member (for error
    /// synchronization)? Identifiers (names, types, contextual modifiers like
    /// `static`/`factory`/`get`), metadata (`@`) and the words that only open a
    /// declaration (`final`/`const`/`var`/`void`; `this`/`new` open a member in
    /// 3.13).
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
    // Directives
    // -----------------------------------------------------------------------

    /// Is the token `n` ahead a string literal?
    fn string_at(&self, n: usize) -> bool {
        matches!(self.kind_at(n), Kind::Str(_) | Kind::StrBegin(..))
    }

    /// Reads a directive if the current token starts one; `None` otherwise
    /// (without consuming anything). `library`, `import`, `export` and `part`
    /// are built-in identifiers, so the decision looks at the next token.
    fn parse_directive_opt(&mut self) -> PResult<Option<DirectiveKind>> {
        // `import augment 'uri';` and `augment library 'uri';`: the augmentation
        // library form SDK 3.6 accepts with `--enable-experiment=macros` (and the
        // one the 3.6.2 CFE generates for the macro output).
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
            // `parseImport`: `deferred` without `as` is
            // `MISSING_PREFIX_IN_DEFERRED_IMPORT` at the `deferred`.
            if let (Some(span), None) = (deferred_span, prefix) {
                self.error_at(codes::parser::MISSING_PREFIX_IN_DEFERRED_IMPORT, span, &[]);
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

    /// `a`, `a.b.c` — a library name or a configuration test.
    fn parse_dotted_name(&mut self) -> PResult<Vec<Name>> {
        let mut names = vec![self.expect_identifier()?];
        while self.at_op(Op::Dot) && self.at_identifier_at(1) {
            self.advance();
            names.push(self.identifier());
        }
        Ok(names)
    }

    /// Zero or more `if (dart.library.io == 'x') 'uri'`.
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

    /// Zero or more `show a, b` / `hide c`, in any order.
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

    /// `a, b, c` — at least one identifier.
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

    /// Zero or more `@annotation`.
    ///
    /// Forms: `@a`, `@p.a`, `@C.ctor(args)`, `@p.C.ctor(args)`, `@C<T>(args)`,
    /// `@p.C<T>.ctor(args)`. The arguments are only read if the `(` is glued to
    /// the name (or there are type arguments), so as not to swallow a record
    /// type that starts the next declaration.
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
            // `endMetadata` (`ast_builder.dart:2512-2518`): type arguments without
            // `generic-metadata`, at the `<`.
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
            // `parseMetadata` (`parser_impl.dart:1346-1349`): with type arguments, what
            // follows must be `(`; the error goes at the last token read of the
            // annotation (the `>` or the `.name`).
            if with_types && !self.at_op(Op::LParen) {
                let last = self.tokens[self.pos - 1].span;
                self.error_at(codes::parser::ANNOTATION_WITH_TYPE_ARGUMENTS_UNINSTANTIATED, last, &[]);
            }
            // `parseArgumentsOptMetadata` (`:7718-7752`): a glued `(` is the
            // arguments; a separate one, only with type arguments or when what follows
            // the `)` is `class`/`enum`, with `ANNOTATION_SPACE_BEFORE_PARENTHESIS` at
            // the `(`.
            let glued = self.pos > 0 && self.tokens[self.pos - 1].glued;
            let arguments = if self.at_op(Op::LParen) {
                let after_closer = self.matching_close(self.pos).map(|f| f + 1 - self.pos);
                let class_or_enum = after_closer.is_some_and(|n| self.at_kw_at(n, Keyword::Class) || self.at_kw_at(n, Keyword::Enum));
                if glued {
                    Some(self.parse_arguments()?)
                } else if with_types || class_or_enum {
                    let parens = self.span();
                    self.error_at(codes::parser::ANNOTATION_SPACE_BEFORE_PARENTHESIS, parens, &[]);
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

    /// `identifierOrNew`: an identifier or the word `new` (constructor name
    /// `C.new`).
    fn identifier_or_new(&mut self) -> PResult<Name> {
        if self.at_kw(Keyword::New) {
            let token = self.advance();
            Ok(self.name_from("new", token.span))
        } else {
            self.expect_identifier()
        }
    }

    // -----------------------------------------------------------------------
    // Top-level declarations
    // -----------------------------------------------------------------------

    /// Dispatches the top-level declaration starting at the current token.
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

    /// The decision of `parseTopLevelDeclarationImpl` (`parser_impl.dart:528`)
    /// on what starts at the cursor: a top-level word (`class`, `enum`,
    /// `mixin`…) preceded by modifiers (the token's `isModifier`, without
    /// looking at the next one) and at most one `macro`/`sealed`/`base`/
    /// `interface`; or a top-level member — whenever it starts with `var`,
    /// `late`, `final` (not before `class`/`mixin`/`enum`) or `const` (not
    /// before `class`), and when the modifiers do not lead to a top-level word
    /// (`final abstract class` is a malformed `final` field).
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
        // Modifiers followed by a name, or `sealed`/`base`/… out of order
        // (`base abstract class`, `sealed sealed class`): a member.
        if j > p0 || (k == j && self.class_follows()) {
            return TopLevelRoute::Member;
        }
        TopLevelRoute::Plain
    }

    /// The member modifiers before a top-level word, read by the
    /// `ModifierContext` (`parseClassModifiers`, `parseEnumModifiers`,
    /// `parseMixinModifiers`…): what does not fit there is reported and
    /// discarded; the class's `abstract`/`final` go to `pre`.
    fn modifiers_before_word(&mut self, word: usize, pre: &mut ClassModifiers) {
        let mut f = ModifierTokens::default();
        self.modifier_context(&mut f, false);
        // `mixin class` is a class (`_handleModifiersForClassDeclaration`).
        let mut text = self.text_of(word);
        if text == "mixin" && self.text_of(word + 1) == "class" {
            text = "class";
        }
        self.report_modifiers_before_top_level(&f, text);
        match text {
            "class" => {
                pre.abstract_ = f.abstract_.is_some();
                pre.final_ = f.final_.is_some();
                if f.final_.is_some() && self.text_of(word) == "mixin" {
                    self.error_at_token(f.final_.unwrap_or_default(), codes::parser::FINAL_MIXIN_CLASS);
                }
            }
            "mixin" => {
                if let Some(fi) = f.final_ {
                    self.error_at_token(fi, codes::parser::FINAL_MIXIN);
                }
            }
            "enum" => {
                if let Some(fi) = f.final_ {
                    self.error_at_token(fi, codes::parser::FINAL_ENUM);
                }
            }
            _ => {}
        }
    }

    /// `sealed`/`base`/`interface` before `mixin`, `mixin class` or `enum`
    /// (`parseTopLevelKeywordDeclaration`): only `base` fits on a mixin, none on
    /// an enum; what does not fit is reported (`SEALED_MIXIN`,
    /// `INTERFACE_MIXIN_CLASS`, `BASE_ENUM`…) and discarded. The valid forms
    /// (`base mixin`, `sealed class`…) go on to the common dispatch.
    fn mixin_and_enum_prefixes(&mut self) {
        if self.kind() != Kind::Ident || !matches!(self.text(), "sealed" | "base" | "interface") {
            return;
        }
        let prefix = self.pos;
        let text = self.text();
        let following = match self.kind_at(1) {
            Kind::Ident | Kind::Keyword(_) => self.text_at(1),
            _ => return,
        };
        let code = match (following, text) {
            ("enum", "sealed") => codes::parser::SEALED_ENUM,
            ("enum", "base") => codes::parser::BASE_ENUM,
            ("enum", _) => codes::parser::INTERFACE_ENUM,
            ("mixin", _) if self.text_at(2) == "class" && self.kind_at(2) == Kind::Keyword(Keyword::Class) => match text {
                "sealed" => codes::parser::SEALED_MIXIN_CLASS,
                "interface" => codes::parser::INTERFACE_MIXIN_CLASS,
                _ => return,
            },
            ("mixin", "sealed") => codes::parser::SEALED_MIXIN,
            ("mixin", "interface") => codes::parser::INTERFACE_MIXIN,
            _ => return,
        };
        self.error_at_token(prefix, code);
        self.advance();
    }

    /// Does `modifiers_* class` start here?
    fn class_follows(&self) -> bool {
        let mut i = self.pos;
        loop {
            match self.kind_of(i) {
                Kind::Keyword(Keyword::Final) => i += 1,
                // `mixin` only opens a class when glued to `class` (`mixin base class` is a
                // mixin named `base`, as in fasta).
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

    /// Does `base? mixin Name` start here?
    fn mixin_follows(&self) -> bool {
        // `parseTopLevelKeywordDeclaration`: after `mixin`, only `(`, `.` and `<`
        // make it a plain name (a top-level member); anything else opens a mixin,
        // even if the name is missing (`mixin final M {}`).
        let opener = |i: usize| !matches!(self.kind_of(i), Kind::Op(Op::LParen | Op::Dot | Op::Lt) | Kind::Eof);
        (self.at_ident("mixin") && opener(self.pos + 1))
            || (self.at_ident("base") && self.at_ident_at(1, "mixin") && opener(self.pos + 2))
    }

    /// `AstBuilder.beginClassDeclaration`/`beginNamedMixinApplication` (3.6.2,
    /// `ast_builder.dart:261-300`, `:525-570`): without `sealed-class`, the
    /// `sealed`; without `class-modifiers`, the `base`, `interface`, `final` and
    /// `mixin` are `EXPERIMENT_NOT_ENABLED` at the token, and the modifier is
    /// forgotten. The tokens are those preceding the current `class`.
    fn class_modifiers_without_feature(&mut self, m: &mut ClassModifiers) {
        let sealed = self.features.has(Feature::SealedClass);
        let modifiers_ = self.features.has(Feature::ClassModifiers);
        if sealed && modifiers_ {
            return;
        }
        let mut found_items: Vec<(String, Span)> = Vec::new();
        let mut i = self.pos.saturating_sub(1);
        while i > 0 {
            i -= 1;
            let tk = &self.tokens[i];
            let text = &self.source[tk.span.start..tk.span.end];
            // An annotation's name (`@sealed class C`) is not a modifier.
            let from_annotation = i > 0 && &self.source[self.tokens[i - 1].span.start..self.tokens[i - 1].span.end] == "@";
            match text {
                "abstract" | "base" | "interface" | "final" | "sealed" | "mixin" | "macro" | "augment" if !from_annotation => {
                    found_items.push((text.to_string(), tk.span));
                }
                _ => break,
            }
        }
        found_items.reverse();
        for (text, sp) in found_items {
            match text.as_str() {
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

    /// `classDeclaration`, including the form `class C = S with M;`.
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
        // `parseClassOrNamedMixinApplication`: `abstract` with `sealed` is
        // `ABSTRACT_SEALED_CLASS` at the `sealed` (in any order).
        if modifiers.abstract_
            && let Some(span) = sealed
        {
            self.error_at(codes::parser::ABSTRACT_SEALED_CLASS, span, &[]);
        }
        // `class const K(...)`: a constant primary constructor (3.13).
        let primary_const = self.at_kw(Keyword::Const).then(|| self.advance().span);
        let (name_text, name) = self.declaration_name()?;
        let type_params = self.parse_type_parameters_opt_variance()?;
        if self.at_op(Op::Assign) {
            // `class const C = S with M;`: the `const` is
            // `CONST_WITHOUT_PRIMARY_CONSTRUCTOR`, with or without the feature
            // (`parseClassOrNamedMixinApplication`, `parser_impl.dart:2983-2986`).
            if let Some(c) = primary_const {
                self.error_at(codes::parser::CONST_WITHOUT_PRIMARY_CONSTRUCTOR, c, &[]);
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
        // `parseClassHeaderOpt`: `native 'name'?` after the clauses (the analyzer
        // accepts the old form; outside the SDK the `ErrorVerifier` gives
        // `NATIVE_CLAUSE_IN_NON_SDK_CODE`, in `analise::nativos`).
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

    /// `(.id)? (params)` after the name and type parameters of a class or enum:
    /// the header of a primary constructor (Dart 3.13, `<primaryConstructor>`).
    /// `var`/`final` in the parameters declare a field.
    fn parse_primary_header_opt(
        &mut self,
        start_offset: Span,
        const_: Option<Span>,
    ) -> PResult<Option<PrimaryHeader>> {
        if !self.at_op(Op::LParen) && !(self.at_op(Op::Dot) && self.kind_at(1) != Kind::Op(Op::Dot))
        {
            // `parsePrimaryConstructorOpt` without a header (3.13.4,
            // `parser_impl.dart:3829-3862`): the `const` is
            // `CONST_WITHOUT_PRIMARY_CONSTRUCTOR` with the feature on and
            // `UNEXPECTED_TOKEN` without it, and the declaration goes on.
            if let Some(c) = const_ {
                if self.features.has(Feature::PrimaryConstructors) {
                    self.error_at(codes::parser::CONST_WITHOUT_PRIMARY_CONSTRUCTOR, c, &[]);
                } else {
                    self.error_at(codes::parser::UNEXPECTED_TOKEN, c, &["const"]);
                }
            }
            return Ok(None);
        }
        let beginning = self.span();
        let name = if self.eat_op(Op::Dot) {
            let n = self.identifier_or_new()?;
            (self.interner.resolve(n.sym) != "new").then_some(n)
        } else {
            None
        };
        let saved = self.in_primary_constructor;
        self.in_primary_constructor = true;
        self.params_owner = ParameterOwner::PrimaryConstructor;
        let params = self.parse_formal_parameters();
        self.in_primary_constructor = saved;
        let params = params?;
        let span = self.span_from(beginning);
        // The 3.13.4 analyzer reports the disabled feature at the header's first
        // token (`(`, or the `.` of `C.name(`), not on the whole header.
        self.require(Feature::PrimaryConstructors, beginning);
        Ok(Some(PrimaryHeader {
            span: Span {
                start: start_offset.start,
                end: span.end,
            },
            const_: const_.is_some(),
            name,
            params,
        }))
    }

    /// `{ members }` or, from 3.13 on, `;` (empty body).
    fn parse_class_body_or_empty(&mut self, class_name: Option<&'s str>) -> PResult<Vec<MemberId>> {
        let synthetic = self.synthetic_name.take();
        if self.at_op(Op::Semicolon) {
            let t = self.advance();
            self.require_in_ast(Feature::PrimaryConstructors, t.span);
            return Ok(Vec::new());
        }
        // `ensureBlock` (`parser_impl.dart:4204`) with the declaration's
        // `BlockKind`: without `{`, `EXPECTED_CLASS_BODY`/`EXPECTED_MIXIN_BODY`/…
        // at the last token read (in place of the synthetic name, if any) and a
        // synthetic empty body; the rest is read as the following declarations.
        if !self.at_op(Op::LBrace) && self.pos > 0 {
            let code = match self.owner {
                super::MemberOwner::Class => codes::parser::EXPECTED_CLASS_BODY,
                super::MemberOwner::Mixin => codes::parser::EXPECTED_MIXIN_BODY,
                super::MemberOwner::Extension => codes::parser::EXPECTED_EXTENSION_BODY,
                super::MemberOwner::ExtensionType => codes::parser::EXPECTED_EXTENSION_TYPE_BODY,
                super::MemberOwner::Enum => return self.parse_class_body(class_name),
            };
            let span = synthetic.unwrap_or(self.tokens[self.pos - 1].span);
            self.error_at(code, span, &[]);
            return Ok(Vec::new());
        }
        self.parse_class_body(class_name)
    }

    /// The name of a class or mixin (`ClassOrMixinOrExtensionIdentifierContext`,
    /// `identifier_context_impl.dart`): a word that opens the next declaration
    /// (`mixin mixin class C`, `mixin final M`) or that would only fit after the
    /// name is not the name — `MISSING_IDENTIFIER` at it and an empty synthetic
    /// name, without consuming; another reserved word is
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` and becomes the name.
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
                self.error_at(codes::parser::MISSING_IDENTIFIER, here, &[]);
                self.synthetic_name = Some(here);
                let name = self.name_from("", Span { start: here.start, end: here.start });
                return Ok(("", name));
            }
        }
        let text = self.text();
        Ok((text, self.expect_identifier()?))
    }

    /// The D → D2 derivation of the primary constructor (3.13 spec,
    /// `:926-1025`), done on the tree right after the declaration is parsed:
    /// each declaring parameter `var`/`final T p` becomes a field
    /// `T p;`/`final T p;` and the parameter becomes `T this.p` (the
    /// `covariant` moves to the field); the constructor `k2` gets the name, the
    /// header's `const` (or the `enum`'s), the initializer list and the body of
    /// the `this` part. Consumers see regular code. Returns `k2`, which takes
    /// the place of the `this` part (or goes after the fields, if there is
    /// none).
    fn elaborate_primary_constructor(
        &mut self,
        class: Name,
        cab: Option<PrimaryHeader>,
        members: &mut Vec<MemberId>,
        where_: Elaborating,
    ) -> Option<MemberId> {
        // `this` parts in the body.
        let parts: Vec<usize> = members
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(&self.ast.member(**m).kind, MemberKind::Constructor(c) if c.primary_body_part))
            .map(|(i, _)| i)
            .collect();
        // `_checkForMultiplePrimaryConstructorBodyDeclarations` (3.13.4,
        // `error_verifier.dart:6293-6305`): from the second part on, with or
        // without the primary header.
        for &i in parts.iter().skip(1) {
            let span = self.this_span(members[i]);
            self.error_at(codes::compile_time_error::MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS, span, &[]);
        }
        let Some(cab) = cab else {
            for &i in &parts {
                let span = self.this_span(members[i]);
                self.error_at(codes::compile_time_error::PRIMARY_CONSTRUCTOR_BODY_WITHOUT_DECLARATION, span, &[]);
            }
            return None;
        };
        // A non-redirecting generative constructor in the body: forbidden (k2 is
        // the only one, so the field initializers can read the parameters); k2's
        // name cannot repeat another constructor's.
        for &m in members.iter() {
            if let MemberKind::Constructor(c) = &self.ast.member(m).kind {
                if c.primary_body_part {
                    continue;
                }
                let redirects = c
                    .initializers
                    .iter()
                    .any(|i| matches!(i, Initializer::Redirect { .. }));
                if !c.factory && !redirects {
                    // At the constructor's name (`C` or `C.name`), like the analyzer.
                    let end_offset = c.name.map_or(c.class_name.span.end, |n| n.span.end);
                    let span = Span { start: c.class_name.span.start, end: end_offset };
                    self.diagnostics.push(Diagnostic::with_code(
                        codes::compile_time_error::NON_REDIRECTING_GENERATIVE_CONSTRUCTOR_WITH_PRIMARY,
                        span,
                        Vec::<&str>::new(),
                    ));
                }
                if c.name.map(|n| n.sym) == cab.name.map(|n| n.sym) {
                    // The body's constructor repeats the primary one's name: the analyzer's
                    // `duplicate_constructor`, at the second one's name.
                    let end_offset = c.name.map_or(c.class_name.span.end, |n| n.span.end);
                    let span = Span { start: c.class_name.span.start, end: end_offset };
                    let d = match c.name {
                        None => Diagnostic::with_code(
                            codes::compile_time_error::DUPLICATE_CONSTRUCTOR_DEFAULT,
                            span,
                            Vec::<&str>::new(),
                        ),
                        Some(n) => Diagnostic::with_code(
                            codes::compile_time_error::DUPLICATE_CONSTRUCTOR_NAME,
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
            // An enum constructor is always constant.
            Elaborating::Enum => (true, true),
        };
        let mut fields_: Vec<MemberId> = Vec::new();
        let mut parameters_ = cab.params;
        for p in parameters_.iter_mut() {
            // The leftover `covariant` belongs to `parseFormalParameterModifiers`
            // (`report_parameter_modifiers`, `PrimaryConstructor`).
            if !(p.var_ || p.final_) || p.this_ || p.super_ {
                continue;
            }
            let Some(name) = p.name else { continue };
            // A declaring parameter in the old function-parameter style
            // (`final int _f()`): the field has the whole function type
            // (`int Function()`), not just the return type. The type's parameters are
            // shallow copies (name, type, kind): a function type has no metadata or
            // default values; the constructor parameter stays as it is.
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
                        declaring: false,
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
            let field_ = Member {
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
                        name,
                        initializer: None,
                    }]
                    .into_boxed_slice(),
                }),
                augment: false,
            };
            fields_.push(self.ast.push_member(field_));
            // `var T p` → `T this.p` (with the type, if written).
            p.this_ = true;
            p.declaring = true;
            p.var_ = false;
            p.final_ = false;
            p.covariant = false;
        }
        // Initializer list and body of the `this` part.
        let (initializers, body, part_span, body_part) = match parts.first() {
            Some(&i) => {
                let mid = members[i];
                let span = self.ast.member(mid).span;
                let MemberKind::Constructor(c) = &mut self.ast.members[mid.0 as usize].kind else {
                    unreachable!()
                };
                let inits = std::mem::take(&mut c.initializers);
                let body = std::mem::replace(&mut c.body, FunctionBody::Empty);
                let body_node = self.primary_bodies.get(&c.class_name.span.start).copied();
                // `this : this.name()`: the primary constructor does not redirect
                // (`PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT` at the redirection's `this`). The
                // initializer stays (it counts for the constant cycle and for the use of
                // the target); the generative cycle ignores it.
                for i in inits.iter() {
                    if let Initializer::Redirect { span: sp, .. } = i {
                        let s = Span { start: sp.start, end: sp.start + 4 };
                        self.diagnostics.push(Diagnostic::with_code(codes::compile_time_error::PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT, s, Vec::<&str>::new()));
                    }
                }
                (inits, body, Some(span), body_node)
            }
            None => (Vec::new().into_boxed_slice(), FunctionBody::Empty, None, None),
        };
        // Body of the `this` part, at the `{`/`=>` that opens it (recorded during
        // the parse): `const` accepts no body at all; otherwise, `=>` is the error.
        if let Some(span) = body_part {
            let code = match (&body, const_) {
                (FunctionBody::Block(_), true) => Some(codes::parser::CONST_PRIMARY_CONSTRUCTOR_WITH_BLOCK_BODY),
                (FunctionBody::Expression(_), true) => Some(codes::parser::CONST_PRIMARY_CONSTRUCTOR_WITH_EXPRESSION_BODY),
                (FunctionBody::Expression(_), false) => {
                    Some(codes::compile_time_error::PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY)
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
                name: cab.name,
                parameters: parameters_.into_boxed_slice(),
                initializers,
                redirect: None,
                body,
                primary_body_part: false,
            }),
            augment: false,
        };
        let k2 = match parts.first() {
            Some(&i) => {
                // k2 takes the place of the `this` part.
                let mid = members[i];
                self.ast.members[mid.0 as usize] = k2;
                mid
            }
            None => self.ast.push_member(k2),
        };
        // Fields first (in parameter order), then the body; k2 goes after the
        // fields when there was no `this` part.
        let mut new_fields = fields_;
        if parts.is_empty() {
            new_fields.push(k2);
        }
        new_fields.append(members);
        *members = new_fields;
        Some(k2)
    }

    /// Type of an untyped declaring parameter (3.13 spec, `:945-962`): that of
    /// the inherited getter of the same name; otherwise that of the default
    /// value; otherwise `Object?`. Here, syntactically: a literal default gives
    /// its type, `null` or no default gives `Object?` — but only when the class
    /// has no declared supertype, because then an inherited getter could decide
    /// (without a type, the field is left to `types` inference).
    fn untyped_declarant_type(
        &mut self,
        default_value: Option<ExprId>,
        has_supertypes: bool,
        span: Span,
    ) -> Option<TypeId> {
        use crate::ast::{ExprKind, TypeAnnotation, TypeKind};
        if has_supertypes {
            return None;
        }
        let (name, is_nullable) = match default_value.map(|e| &self.ast.expr(e).kind) {
            None | Some(ExprKind::Null) => ("Object", true),
            Some(ExprKind::Int(_)) => ("int", false),
            Some(ExprKind::Double(_)) => ("double", false),
            Some(ExprKind::String(_)) => ("String", false),
            Some(ExprKind::Bool(_)) => ("bool", false),
            Some(_) => return None,
        };
        let n = self.name_from(name, span);
        Some(self.ast.push_type(TypeAnnotation {
            span,
            nullable: is_nullable,
            kind: TypeKind::Named {
                name: vec![n].into_boxed_slice(),
                args: Vec::new().into_boxed_slice(),
            },
        }))
    }

    /// Optional `implements A, B`.
    fn parse_implements_opt(&mut self) -> PResult<Vec<TypeId>> {
        if self.eat_ident("implements") {
            self.parse_type_list()
        } else {
            Ok(Vec::new())
        }
    }

    /// `A, B, C` — at least one type.
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
        // `beginMixinDeclaration` (`ast_builder.dart:488-497`): without
        // `class-modifiers`, the `base` is the disabled feature and is forgotten.
        if let Some(sp) = base_span
            && !self.features.has(Feature::ClassModifiers)
        {
            self.require(Feature::ClassModifiers, sp);
            base = false;
        }
        self.expect_ident("mixin")?;
        let (name_text, name) = self.declaration_name()?;
        let type_params = self.parse_type_parameters_opt_variance()?;
        // A mixin has no primary constructor: `(` or `.name(` after the name is
        // `UNEXPECTED_TOKEN` at the first token, and the group is skipped (3.13.4
        // analyzer, `primary_constructors/syntax/mixin_error_test`).
        if self.at_op(Op::LParen) || (self.at_op(Op::Dot) && self.at_identifier_at(1)) {
            let text = self.text().to_string();
            self.error(codes::parser::UNEXPECTED_TOKEN, &[&text]);
            if self.eat_op(Op::Dot) {
                self.advance();
            }
            if self.at_op(Op::LParen) {
                match self.matching_close(self.pos) {
                    Some(f) if self.kind_of(f) == Kind::Op(Op::RParen) => {
                        // 3.13.4 still reads the parameters: a parameter's `var` (declaring) is the
                        // disabled feature; `final` was allowed before.
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
        // Mixin header recovery: a stray name before `on`, `implements` or `{` is
        // `UNEXPECTED_TOKEN` and is skipped (`mixin sealed M {}`).
        if self.kind() == Kind::Ident
            && !self.at_ident("on")
            && !self.at_ident("implements")
            && (self.at_op_at(1, Op::LBrace) || self.at_ident_at(1, "on") || self.at_ident_at(1, "implements"))
        {
            let text = self.text().to_string();
            self.error(codes::parser::UNEXPECTED_TOKEN, &[&text]);
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

    /// `enumType`: constants (with metadata, type arguments, constructor and
    /// arguments), optional trailing comma, `;` and optional members.
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
        // Empty `;` body (3.13, primary constructors): no constants or members
        // (`ENUM_WITHOUT_CONSTANTS` comes out later).
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
        let opener = self.pos;
        self.expect_op(Op::LBrace)?;
        let mut constants = Vec::new();
        while !self.at_op(Op::RBrace) && !self.at_op(Op::Semicolon) && !self.at_eof() {
            constants.push(self.parse_enum_constant()?);
            if self.eat_op(Op::Comma) || self.at_op(Op::RBrace) || self.at_op(Op::Semicolon) {
                continue;
            }
            // `parseEnum` (`parser_impl.dart`): another name is the next constant after
            // a missing comma (`EXPECTED_TOKEN` `,` at it); anything else is
            // `EXPECTED_TOKEN` `}` at it and the body ends at the matching `}`.
            match self.matching_close(opener) {
                Some(closes) if self.kind_of(closes) == Kind::Op(Op::RBrace) => {
                    if self.at_identifier() {
                        self.error(codes::parser::EXPECTED_TOKEN, &[","]);
                    } else {
                        self.error(codes::parser::EXPECTED_TOKEN, &["}"]);
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

    /// `@meta name<T>.ctor(args)`.
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

    /// `extension` was already seen: decides between
    /// `extension [Name]<T> on Type` and `extension type`.
    /// `extension type on X {}` is an extension named `type`;
    /// `extension type Name(...)` and `extension type const` are extension
    /// types.
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
        // `on` is only the extension's name if there is another `on` (or `<`)
        // after it.
        let named = self.at_identifier()
            && (!self.at_ident("on") || self.at_ident_at(1, "on") || self.at_op_at(1, Op::Lt));
        let (name_text, name) = if named {
            let text = self.text();
            (Some(text), Some(self.identifier()))
        } else {
            (None, None)
        };
        let type_params = self.parse_type_parameters_opt()?;
        // `parseExtensionDeclaration` (`parser_impl.dart`): without `on`,
        // `extends`/`implements`/`with` stand in for it with `EXPECTED_INSTEAD`;
        // otherwise `EXPECTED_TOKEN` (`on`) at the last token read and a synthetic
        // `on`. Without a type after it (`{`), `EXPECTED_TYPE_NAME` at the `{` and
        // a synthetic type; the body is read normally.
        if !self.eat_ident("on") {
            if self.at_kw(Keyword::Extends) || self.at_kw(Keyword::With) || self.at_ident("implements") {
                self.error(codes::parser::EXPECTED_INSTEAD, &["on"]);
                self.advance();
            } else {
                let last = self.tokens[self.pos - 1].span;
                self.error_at(codes::parser::EXPECTED_TOKEN, last, &["on"]);
            }
        }
        let on = if matches!(self.kind(), Kind::Op(Op::LBrace | Op::Semicolon | Op::RBrace) | Kind::Eof) {
            let here = self.span();
            self.error_at(codes::parser::EXPECTED_TYPE_NAME, here, &[]);
            let s = Span { start: here.start, end: here.start };
            let name = self.name_from("", s);
            self.ast.push_type(TypeAnnotation {
                span: s,
                nullable: false,
                kind: TypeKind::Named { name: vec![name].into_boxed_slice(), args: Vec::new().into_boxed_slice() },
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

    /// `extension type const? Nome<T>(.ctor)? (@meta final? Tipo name) implements I { }`.
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
            // `parseExtensionTypeDeclaration`: without `(` or `.`,
            // `MISSING_PRIMARY_CONSTRUCTOR` at the last token read (the name or the
            // `>`); with `.name` without `(`, `MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS`
            // at the name. The rest of the declaration is read normally, with the
            // synthetic representation (empty, zero width).
            let code = if has_primary {
                codes::parser::MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS
            } else {
                codes::parser::MISSING_PRIMARY_CONSTRUCTOR
            };
            let last = self.tokens[self.pos - 1].span;
            self.error_at(code, last, &[]);
            let s = Span { start: last.end, end: last.end };
            let name = self.name_from("", s);
            let ty = self.ast.push_type(TypeAnnotation {
                span: s,
                nullable: false,
                kind: TypeKind::Named { name: vec![name].into_boxed_slice(), args: Vec::new().into_boxed_slice() },
            });
            (Vec::new(), ty, name)
        };
        let representation_span = self.span_from(representation_start);
        let implements = self.parse_implements_opt()?;
        self.owner = super::MemberOwner::ExtensionType;
        let members = self.parse_class_body_or_empty(Some(name_text))?;
        // From the second `this` part on:
        // `MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS` (the same
        // `_checkForMultiplePrimaryConstructorBodyDeclarations` as for classes).
        let parts: Vec<MemberId> = members
            .iter()
            .copied()
            .filter(|m| matches!(&self.ast.member(*m).kind, MemberKind::Constructor(c) if c.primary_body_part))
            .collect();
        for &m in parts.iter().skip(1) {
            let span = self.this_span(m);
            self.error_at(codes::compile_time_error::MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS, span, &[]);
        }
        // The `this …` body part of the extension type's primary constructor (the
        // representation), like the class one (`elaborate_primary_constructor`):
        // `const` accepts no body at all; otherwise, `=>` is the error.
        for &mid in members.iter() {
            let MemberKind::Constructor(c) = &self.ast.member(mid).kind else { continue };
            if !c.primary_body_part {
                continue;
            }
            let Some(span) = self.primary_bodies.get(&c.class_name.span.start).copied() else { continue };
            let code = match (&c.body, const_) {
                (FunctionBody::Block(_), true) => Some(codes::parser::CONST_PRIMARY_CONSTRUCTOR_WITH_BLOCK_BODY),
                (FunctionBody::Expression(_), true) => Some(codes::parser::CONST_PRIMARY_CONSTRUCTOR_WITH_EXPRESSION_BODY),
                (FunctionBody::Expression(_), false) => Some(codes::compile_time_error::PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY),
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

    /// The parameter list of an extension type's primary constructor
    /// (`parseFormalParameters` with `MemberKind.PrimaryConstructor`) and the
    /// representation `AstBuilder.endPrimaryConstructor` takes from it: the
    /// first parameter, if it is a simple positional one (otherwise
    /// `EXPECTED_REPRESENTATION_FIELD` at the token after the `(`, and a
    /// synthetic type and name); without a type, `EXPECTED_REPRESENTATION_TYPE`;
    /// `var`/`final` (before 3.13, where `final` declares),
    /// `REPRESENTATION_FIELD_MODIFIER`; the comma after it,
    /// `REPRESENTATION_FIELD_TRAILING_COMMA` (a single parameter) or
    /// `MULTIPLE_REPRESENTATION_FIELDS`.
    fn parse_representation(&mut self) -> PResult<(Vec<Annotation>, TypeId, Name)> {
        let opener = self.pos;
        if !self.at_op(Op::LParen) {
            return Err(self.expected_error("("));
        }
        let saved = self.in_primary_constructor;
        self.in_primary_constructor = true;
        self.params_owner = ParameterOwner::PrimaryConstructor;
        let saved_repr = std::mem::replace(&mut self.in_representation, true);
        let params = self.parse_formal_parameters();
        self.in_representation = saved_repr;
        self.in_primary_constructor = saved;
        let mut params = params?;
        let after_open = self.tokens[opener + 1].span;
        let synthetic = |p: &mut Self| {
            let s = Span { start: after_open.start, end: after_open.start };
            let name = p.name_from("", s);
            let ty = p.ast.push_type(TypeAnnotation {
                span: s,
                nullable: false,
                kind: TypeKind::Named { name: vec![name].into_boxed_slice(), args: Vec::new().into_boxed_slice() },
            });
            (ty, name)
        };
        let simple = params.first().is_some_and(|p| {
            p.kind == ParameterKind::Required && !p.this_ && !p.super_ && p.function_parameters.is_none() && p.name.is_some()
        });
        // The 3.13.4 analyzer reports more than one parameter as
        // `MULTIPLE_REPRESENTATION_FIELDS` at the first comma, whatever the shape
        // of the first one; 3.6.2, the missing field. Which of the two stays
        // depends on the unit's reference, only known at the end
        // (`Parsed::reference`): both come out, each one marked.
        let n = params.len();
        // 3.13.4 checks the name of the representation's single parameter against
        // the members of `Object` (`EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT` at
        // the name, `error_verifier.dart` in the main checkout, `:5641-5655`),
        // whatever the parameter's shape; 3.6.2 does not check.
        if n == 1
            && let Some(p) = params.first()
            && !p.this_
            && !p.super_
            && let Some(name) = p.name
            && matches!(&self.source[name.span.start..name.span.end], "hashCode" | "noSuchMethod" | "runtimeType" | "toString" | "==")
        {
            self.error_at(codes::compile_time_error::EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT, name.span, &[]);
            self.only_3_13.push((codes::compile_time_error::EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT, name.span));
        }
        if n > 1 && !simple {
            let end_offset = params[0].span.end;
            if let Some(comma) =
                self.tokens[opener..self.pos].iter().find(|t| t.span.start >= end_offset && t.kind == Kind::Op(Op::Comma))
            {
                let span = comma.span;
                self.error_at(codes::parser::MULTIPLE_REPRESENTATION_FIELDS, span, &[]);
                self.only_3_13.push((codes::parser::MULTIPLE_REPRESENTATION_FIELDS, span));
            }
            self.error_at(codes::parser::EXPECTED_REPRESENTATION_FIELD, after_open, &[]);
            self.only_3_6.push((codes::parser::EXPECTED_REPRESENTATION_FIELD, after_open));
            let (ty, name) = synthetic(self);
            return Ok((Vec::new(), ty, name));
        }
        if !simple {
            // 3.13.4 reports in the `ErrorVerifier`
            // (`_checkForExtensionTypeRepresentationErrorCodes`): the `this` of
            // `int this.x`, the `super` of `int super.x`, the whole unnamed parameter,
            // the `[`/`{` delimiter (disabled feature) and the start of a function
            // parameter; 3.6.2, the token after the `(`.
            let in_3_13 = params.first().and_then(|p| {
                let word = |pp: &Self, k: Keyword| {
                    pp.tokens[opener..pp.pos].iter().find(|t| t.span.start >= p.span.start && t.kind == Kind::Keyword(k)).map(|t| t.span)
                };
                let start_offset = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
                if p.this_ {
                    word(self, Keyword::This)
                } else if p.super_ {
                    word(self, Keyword::Super)
                } else if p.name.is_none() {
                    Some(Span { start: start_offset, end: p.span.end })
                } else if p.kind != ParameterKind::Required && !self.features.has(Feature::PrimaryConstructors) {
                    self.tokens[opener..self.pos]
                        .iter()
                        .find(|t| matches!(t.kind, Kind::Op(Op::LBracket | Op::LBrace)))
                        .map(|t| t.span)
                } else if p.function_parameters.is_some() && !self.features.has(Feature::PrimaryConstructors) {
                    self.tokens[opener..self.pos].iter().find(|t| t.span.start >= start_offset).map(|t| t.span)
                } else {
                    None
                }
            });
            match in_3_13 {
                Some(sp) if sp != after_open => {
                    self.error_at(codes::parser::EXPECTED_REPRESENTATION_FIELD, after_open, &[]);
                    self.only_3_6.push((codes::parser::EXPECTED_REPRESENTATION_FIELD, after_open));
                    self.error_at(codes::parser::EXPECTED_REPRESENTATION_FIELD, sp, &[]);
                    self.only_3_13.push((codes::parser::EXPECTED_REPRESENTATION_FIELD, sp));
                }
                _ => {
                    self.error_at(codes::parser::EXPECTED_REPRESENTATION_FIELD, after_open, &[]);
                }
            }
            let (ty, name) = synthetic(self);
            return Ok((Vec::new(), ty, name));
        }
        let p = params.swap_remove(0);
        let with_type = p.ty.is_some();
        let ty = match p.ty {
            Some(ty) => ty,
            // With `final`/`var` and the feature on, the form is the declaring one,
            // which accepts no type. With it off (case c23 of
            // `corpus/especificacao/t2/v`), 3.6.2 reports the missing type at the token
            // after the `(` and 3.13.4, at the name.
            None if p.final_ || p.var_ => {
                if !self.features.has(Feature::PrimaryConstructors) {
                    self.error_at(codes::parser::EXPECTED_REPRESENTATION_TYPE, after_open, &[]);
                    self.only_3_6.push((codes::parser::EXPECTED_REPRESENTATION_TYPE, after_open));
                    // With more than one parameter, 3.13.4 only reports
                    // `MULTIPLE_REPRESENTATION_FIELDS` (`extension type
                    // ET3(final i, final x)`).
                    if let Some(name) = p.name
                        && n == 1
                    {
                        self.error_at(codes::parser::EXPECTED_REPRESENTATION_TYPE, name.span, &[]);
                        self.only_3_13.push((codes::parser::EXPECTED_REPRESENTATION_TYPE, name.span));
                    }
                }
                synthetic(self).0
            }
            None => {
                // 3.13.4 reports at the name (`@anno int`: `int` is the name).
                match p.name {
                    Some(name) if name.span != after_open => {
                        self.error_at(codes::parser::EXPECTED_REPRESENTATION_TYPE, after_open, &[]);
                        self.only_3_6.push((codes::parser::EXPECTED_REPRESENTATION_TYPE, after_open));
                        self.error_at(codes::parser::EXPECTED_REPRESENTATION_TYPE, name.span, &[]);
                        self.only_3_13.push((codes::parser::EXPECTED_REPRESENTATION_TYPE, name.span));
                    }
                    _ => {
                        self.error_at(codes::parser::EXPECTED_REPRESENTATION_TYPE, after_open, &[]);
                    }
                }
                synthetic(self).0
            }
        };
        // The argument only goes into the 3.13.4 template, which writes `var` for
        // `final` too (cases c02 and c23). With more than one parameter 3.13.4 only
        // reports `MULTIPLE_REPRESENTATION_FIELDS`
        // (`extension type E(final i, final x)`): the modifier one is 3.6.2's only.
        // `var` with the feature off is, in 3.13.4, a use of the primary
        // constructor (reported by the `AstBuilder`, version `3.13`), and makes it
        // the unit's reference — only with a type after the `var`
        // (`parseFormalParameter`, `typeInfo != noType`): `(var i)` does not.
        let modifier = p.var_ || (p.final_ && !self.features.has(Feature::PrimaryConstructors));
        if modifier {
            let target_expr = if p.var_ { Keyword::Var } else { Keyword::Final };
            if let Some(t) = self.tokens[opener..self.pos].iter().find(|t| t.span.start >= p.span.start && t.kind == Kind::Keyword(target_expr)) {
                let span = t.span;
                if p.var_ && with_type {
                    self.require_in_ast(Feature::PrimaryConstructors, span);
                }
                self.error_at(codes::parser::REPRESENTATION_FIELD_MODIFIER, span, &["var"]);
                if n > 1 {
                    self.only_3_6.push((codes::parser::REPRESENTATION_FIELD_MODIFIER, span));
                }
            }
        }
        if let Some(comma) = self.tokens[opener..self.pos].iter().find(|t| t.span.start >= p.span.end).filter(|t| t.kind == Kind::Op(Op::Comma)) {
            let span = comma.span;
            let code = if n == 1 {
                codes::parser::REPRESENTATION_FIELD_TRAILING_COMMA
            } else {
                codes::parser::MULTIPLE_REPRESENTATION_FIELDS
            };
            self.error_at(code, span, &[]);
        }
        let name = p.name.expect("a simple parameter has a name");
        Ok((p.metadata.into_vec(), ty, name))
    }

    /// Position after the `<...>` starting at `pos`, counting only `<`/`>` (used
    /// for type parameters, which may have `extends`); `pos` if there is no `<`
    /// or if the list does not close before `;`/`{`/`}`/`=`.
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

    /// `typedef F<T> = type;` or the old form `typedef ret? F<T>(params);`.
    /// `ensureIdentifierPotentiallyRecovered(…, typedefDeclaration,
    /// isRecovered: true)` (`identifier_context_impl.dart:1166-1189`): the name
    /// of a new-style typedef (before the `=`), or of an old-style one followed
    /// by `(`, that is a built-in word (or `Function`, a pseudo-keyword) is
    /// `EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` at the name; another pseudo-keyword
    /// passes. `BUILT_IN_IDENTIFIER_IN_DECLARATION` comes from the
    /// `ErrorVerifier`.
    fn recovered_typedef_name(&mut self) {
        let text = self.text().to_string();
        let reports = match super::fasta::word_style(&text) {
            Some(super::fasta::WordStyle::BuiltIn) => true,
            Some(super::fasta::WordStyle::Pseudo) => text == "Function",
            _ => false,
        };
        if reports {
            self.error(codes::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, &[&text]);
        }
    }

    fn parse_typedef(&mut self) -> PResult<TypedefDecl> {
        self.expect_ident("typedef")?;
        if self.at_identifier() {
            // `computeTypeParamOrArg(inDeclaration: true)`: the whole list, including
            // bounds with function types of named parameters.
            let after = self.skip_type_arguments(self.pos + 1).unwrap_or_else(|| self.skip_angles(self.pos + 1));
            if self.kind_of(after) == Kind::Op(Op::Assign) {
                self.recovered_typedef_name();
                let name = self.identifier();
                let type_params = self.parse_type_parameters_opt()?;
                let equals_ = self.span();
                self.expect_op(Op::Assign)?;
                let ty = self.parse_type()?;
                // `endTypedef` (`ast_builder.dart:3496-3501`): a target that is not a
                // `GenericFunctionType` without `nonfunction-type-aliases`, at the `=`.
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
    // Modifiers, functions and variables (shared by top level and members)
    // -----------------------------------------------------------------------

    /// Modifiers in any order (superset), read as in fasta
    /// (`read_top_level_modifiers`/`read_member_modifiers`, which report wrong
    /// order, repetition and what does not fit in the context).
    fn parse_modifiers(&mut self, top_level: bool) -> Modifiers {
        let f = if top_level { self.read_top_level_modifiers() } else { self.read_member_modifiers() };
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

    /// Do `get name`, `set name`, `operator op` start here? Returns the accessor
    /// kind without consuming anything.
    /// With `!top_level` (a member), also `get`/`set` followed by a reserved
    /// word that indicates a method or field (`parseClassOrMixin…MemberImpl`:
    /// "Getter or setter followed by a reserved word (name)").
    fn accessor_follows_at(&self, top_level: bool) -> Option<FunctionKind> {
        let name_after = |p: &Self| p.at_identifier_at(1) || (!top_level && matches!(p.kind_at(1), Kind::Keyword(_)) && p.indicates_method_or_field(p.pos + 2));
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

    /// A declaration's name: an identifier, or the reserved word that fasta's
    /// recovery takes as the name when what follows indicates a method or field
    /// (`EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD` at it).
    fn declaration_name_or_reserved(&mut self) -> PResult<Name> {
        if !self.at_identifier() && matches!(self.kind(), Kind::Keyword(_)) && self.indicates_method_or_field(self.pos + 1) {
            let text = self.text().to_string();
            let token = self.advance();
            self.error_at(codes::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, token.span, &[&text]);
            return Ok(self.name_from(&text, token.span));
        }
        self.expect_identifier()
    }

    /// Is the token after `operator` a declarable operator? `operator<T>()` is a
    /// method named `operator`, so `<` only counts if `(` comes after it.
    fn operator_follows(&self) -> bool {
        self.operator_follows_at(self.pos)
    }

    /// Operator name as text: `+`, `[]`, `[]=`, `>=`, `>>`, `>>>`… (`>` arrives
    /// isolated from the lexer and is composed here).
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
                        return Err({ let t = self.text().to_string(); self.error(codes::parser::INVALID_OPERATOR, &[&t]) });
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
                // `operator ===`: the name is the lexeme (the scanner already reported the
                // operator).
                let text = if op == Op::EqEq && self.span().end - self.span().start == 3 { "===" } else { op.text() };
                self.advance();
                text
            }
            _ => return Err(self.identifier_error()),
        };
        Ok(self.name_from(text, self.span_from(start)))
    }

    /// `type? name (params) body`, `type? get name body`,
    /// `type? set name (params) body`, `type? operator op (params) body` or
    /// `type? a = 1, b;`. The modifiers were already read.
    fn parse_function_or_variables(
        &mut self,
        mods: Modifiers,
        start: Span,
        external_top_level: Option<Span>,
        top_level: bool,
    ) -> PResult<FunctionOrVariables> {
        if let Some(kind) = self.accessor_follows_at(top_level) {
            let id = self.parse_accessor(mods, start, None, kind, external_top_level, top_level)?;
            return Ok(FunctionOrVariables::Function(id));
        }
        // A reserved word followed by what indicates a method or field is the name,
        // and the identifier is the type (`int new() => 1;`), without
        // `var`/`final`/`const`.
        let reserved_as_name = !mods.var_
            && !mods.final_
            && !mods.const_
            && matches!(self.kind_at(1), Kind::Keyword(_))
            && self.indicates_method_or_field(self.pos + 2);
        // `foo;`, `foo = e;`, `foo, ...` (also `static = 1;`, `external;`, `C;`):
        // the identifier is the NAME of an untyped field — `modifier_ok` already
        // refused the modifier role where it fit. fasta 3.6.2 reports
        // `missing_const_final_var_or_type` at the name, except with
        // `const`/`final`/`var`, which make the type unnecessary without an error
        // (probed on the local SDK, member and top level). Followed by a reserved
        // word (`augment class C {}` without the experiment), the identifier is
        // also the name: a reserved word cannot be the name after a type (fasta's
        // `computeType` returns `noType`), and the missing `;` is reported after
        // it.
        if self.at_identifier()
            && !reserved_as_name
            && matches!(
                self.kind_at(1),
                Kind::Op(Op::Semicolon | Op::Assign | Op::Comma) | Kind::Keyword(_)
            )
        {
            let name = self.identifier();
            if !mods.var_ && !mods.final_ && !mods.const_ {
                self.error_at(
                    codes::parser::MISSING_CONST_FINAL_VAR_OR_TYPE,
                    name.span,
                    &[],
                );
            }
            self.report_field_modifiers(mods, top_level, name.span);
            let variables = self.parse_declared_variables_tail(name)?;
            return Ok(FunctionOrVariables::Variables(VariableList {
                external: mods.external,
                static_: mods.static_,
                abstract_: mods.abstract_ && !top_level,
                covariant: mods.covariant,
                late: mods.late,
                final_: mods.final_,
                const_: mods.const_,
                var_: mods.var_,
                ty: None,
                variables: variables.into_boxed_slice(),
            }));
        }
        // `parseTopLevelMemberImpl`/`parseClassOrMixin…MemberImpl`: with
        // `var`/`final`/`const`, an outer pattern followed by `=` (`const
        // f() = e`, `var (a, b) = e`) is
        // `PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD` over the
        // pattern, which is discarded; the rest is a field with a synthetic name.
        if (mods.var_ || mods.final_ || mods.const_)
            && let Some(end_offset) = self.outer_pattern_followed_by_equals(self.pos)
        {
            let start_offset = self.span();
            let span = Span { start: start_offset.start, end: self.tokens[end_offset].span.end };
            self.error_at(codes::parser::PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD, span, &[]);
            self.pos = end_offset + 1;
            let name = self.name_from("", Span { start: start_offset.start, end: start_offset.start });
            let variables = self.parse_declared_variables_tail(name)?;
            return Ok(FunctionOrVariables::Variables(VariableList {
                external: mods.external,
                static_: mods.static_,
                abstract_: mods.abstract_ && !top_level,
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
            // "<return type>? <reserved word> <token indicating method or field>": the
            // reserved word is the name (`int new() => 1;`).
            || (!mods.final_
                && !mods.const_
                && self.skip_type(self.pos).is_some_and(|end_offset| matches!(self.kind_of(end_offset), Kind::Keyword(_)) && self.indicates_method_or_field(end_offset + 1)))
        {
            Some(self.parse_type()?)
        } else {
            None
        };
        if let Some(kind) = self.accessor_follows_at(top_level) {
            let id = self.parse_accessor(mods, start, ty, kind, external_top_level, top_level)?;
            return Ok(FunctionOrVariables::Function(id));
        }
        // `(` without a type or modifiers does not open a declaration: a record
        // type would ask for a name after the `)`. It is garbage at top level, and
        // fasta reports `expected_executable` and continues at the next token (see
        // `recover_top_level`), instead of the generic `missing_identifier`.
        if top_level && ty.is_none() && !mods.any_() && self.at_op(Op::LParen) {
            return Err(self.error(codes::parser::EXPECTED_EXECUTABLE, &[]));
        }
        // fasta `TopLevelDeclarationIdentifierContext`: at top level, a word that
        // opens the next declaration (`mixin M`, `extension X`, `typedef`,
        // `import`…) is not accepted as a name after a type, unless `;`, `=`, `,`
        // or a function's parameters (`(`, `<`) follow. The name becomes synthetic
        // (before it), with `missing_identifier` at the word, and the missing `;`
        // is reported at the type (`augment mixin M {}` without the experiment).
        // The same holds, after modifiers or a type, for the reserved words that
        // open a declaration (`class`, `enum`, `final`, `const`, `var`, `void`):
        // `final abstract class C {}` and `final final class C {}` are an unnamed
        // field (missing `;` at the last modifier) followed by the class.
        // `looksLikeStartOfNextTopLevelDeclaration`
        // (identifier_context_impl.dart:1330): `isTopLevelKeyword`, or `const`,
        // `get`, `final`, `set`, `var`, `void`.
        let text = self.text_of(self.pos);
        let next_declaration = matches!(self.kind(), Kind::Ident | Kind::Keyword(_))
            && (super::fasta::is_top_level_word(text) || matches!(text, "const" | "get" | "final" | "set" | "var" | "void"));
        if top_level
            && (ty.is_some() || mods.any_())
            && next_declaration
            // Followed by parameters it is a function's name (`String
            // extension(String path)` from `package:path`), as in fasta. The context's
            // `followingValues`: `;`, `=`, `,` for a variable; `<`, `(`, `{`, `=>` for
            // a function (what comes after the name decides which).
            && !matches!(self.kind_at(1), Kind::Op(Op::Semicolon | Op::Assign | Op::Comma | Op::LParen | Op::Lt | Op::LBrace | Op::Arrow))
        {
            // `insertSyntheticIdentifier` before the word: the variable stays, with the
            // synthetic name (the analyzer still reports what is missing in it,
            // `FINAL_NOT_INITIALIZED` and the like, with the empty name), and
            // `ensureSemicolon` reports the missing `;` at the previous token.
            let word = self.span();
            self.error_at(codes::parser::MISSING_IDENTIFIER, word, &[]);
            let _ = self.expected_error(";");
            let name = self.name_from("", Span { start: word.start, end: word.start });
            self.report_field_modifiers(mods, top_level, name.span);
            return Ok(FunctionOrVariables::Variables(VariableList {
                external: mods.external,
                static_: mods.static_,
                abstract_: mods.abstract_ && !top_level,
                covariant: mods.covariant,
                late: mods.late,
                final_: mods.final_,
                const_: mods.const_,
                var_: mods.var_,
                ty,
                variables: vec![crate::ast::Variable { name, initializer: None }].into_boxed_slice(),
            }));
        }
        // `recoverFromInvalidMember`: in a member, `(` or `{` in place of the name
        // is a method with a synthetic name (`MISSING_IDENTIFIER` at the token).
        let name = if !top_level && matches!(self.kind(), Kind::Op(Op::LParen | Op::LBrace)) {
            let s = self.span();
            self.error_at(codes::parser::MISSING_IDENTIFIER, s, &[]);
            self.name_from("", Span { start: s.start, end: s.start })
        } else {
            self.declaration_name_or_reserved()?
        };
        // `parseTopLevelMemberImpl`/`parseClassOrMixin...MemberImpl`: a name
        // followed by `{` or `=>` is a method without parameters
        // (`parseGetterOrFormalParameters`: `MISSING_FUNCTION_PARAMETERS`, or
        // `MISSING_METHOD_PARAMETERS` in a class, at the name) and the body is
        // read. Also with `.` after the name (`augment core.int foo();` without the
        // experiment: `augment` is the type and `core`, the function).
        let without_parameters = self.at_op(Op::LBrace) || self.at_op(Op::Arrow) || self.at_op(Op::Dot);
        if self.at_op(Op::LParen) || self.at_op(Op::Lt) || without_parameters {
            self.report_method_modifiers(mods, top_level, FunctionKind::Function);
            let type_params = self.parse_type_parameters_opt()?;
            self.params_owner = self.parameter_owner(mods, top_level);
            let parameters = if without_parameters {
                let code = if !top_level && !matches!(self.owner, super::MemberOwner::Extension | super::MemberOwner::ExtensionType) {
                    codes::parser::MISSING_METHOD_PARAMETERS
                } else {
                    codes::parser::MISSING_FUNCTION_PARAMETERS
                };
                // At a synthetic name, the error goes to the next token.
                let target_expr = if name.span.start == name.span.end { self.span() } else { name.span };
                self.error_at(code, target_expr, &[]);
                Vec::new()
            } else {
                self.parse_formal_parameters()?
            };
            let body_start = self.pos;
            let (modifier, body) = self.parse_function_body()?;
            self.check_external_body(mods.external, false, body_start, &body, external_top_level);
            self.check_empty_body(&body, Self::allows_abstract(mods, modifier, top_level));
            self.check_abstract_in_extension(mods, top_level, name.span, &body);
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
        self.report_field_modifiers(mods, top_level, name.span);
        // `parseFields`: without a type and without `var`/`final`/`const`,
        // `MISSING_CONST_FINAL_VAR_OR_TYPE` at the name (`interface interface
        // class C {}`: the built-in identifier is not a type).
        if ty.is_none() && !mods.var_ && !mods.final_ && !mods.const_ {
            self.error_at(codes::parser::MISSING_CONST_FINAL_VAR_OR_TYPE, name.span, &[]);
        }
        let variables = self.parse_declared_variables_tail(name)?;
        Ok(FunctionOrVariables::Variables(VariableList {
            external: mods.external,
            static_: mods.static_,
            abstract_: mods.abstract_ && !top_level,
            covariant: mods.covariant,
            late: mods.late,
            final_: mods.final_,
            const_: mods.const_,
            var_: mods.var_,
            ty,
            variables: variables.into_boxed_slice(),
        }))
    }

    /// `skipOuterPattern` + `=`: the last token of an outer pattern
    /// (`Name(...)`, `p.Name(...)`, `(...)`, `[...]`, `{...}`) starting at `pos`
    /// and followed by `=`.
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

    /// Getter, setter or operator starting from `get`/`set`/`operator`.
    fn parse_accessor(
        &mut self,
        mods: Modifiers,
        start: Span,
        return_type: Option<TypeId>,
        kind: FunctionKind,
        external_top_level: Option<Span>,
        top_level: bool,
    ) -> PResult<FunctionId> {
        self.report_method_modifiers(mods, top_level, kind);
        self.advance();
        let mut type_params = Vec::new();
        let (name, parameters) = match kind {
            FunctionKind::Getter => {
                let name = self.declaration_name_or_reserved()?;
                self.check_member_name(name.span);
                // `parseGetterOrFormalParameters`: `get x(...)` is `GETTER_WITH_PARAMETERS`
                // at the `(`, and the list is read.
                if self.at_op(Op::LParen) {
                    self.error(codes::parser::GETTER_WITH_PARAMETERS, &[]);
                    self.params_owner = self.parameter_owner(mods, top_level);
                    self.parse_formal_parameters()?;
                }
                (name, None)
            }
            FunctionKind::Setter => {
                let name = self.declaration_name_or_reserved()?;
                self.check_member_name(name.span);
                // `parseGetterOrFormalParameters`: without `(`, the missing-parameters
                // error at the name (`missingParameterMessage`: method's in a class, mixin
                // and enum; function's elsewhere) and a synthetic empty list.
                if !self.at_op(Op::LParen) {
                    let code = if !top_level && !matches!(self.owner, super::MemberOwner::Extension | super::MemberOwner::ExtensionType) {
                        codes::parser::MISSING_METHOD_PARAMETERS
                    } else {
                        codes::parser::MISSING_FUNCTION_PARAMETERS
                    };
                    self.error_at(code, name.span, &[]);
                    (name, Some(Vec::new()))
                } else {
                    self.params_owner = self.parameter_owner(mods, top_level);
                    (name, Some(self.parse_formal_parameters()?))
                }
            }
            _ => {
                let name = self.parse_operator_name()?;
                // `parseMethodTypeVar` reads the operator's type parameters and
                // `AstBuilder.endClassMethod` rejects them on the `<…>` range
                // (`TYPE_PARAMETER_ON_OPERATOR`); the tree keeps them.
                if self.at_op(Op::Lt) {
                    let opener = self.span();
                    type_params = self.parse_type_parameters_opt()?;
                    let list = self.span_from(opener);
                    self.error_at(codes::parser::TYPE_PARAMETER_ON_OPERATOR, list, &[]);
                }
                self.params_owner = self.parameter_owner(mods, top_level);
                (name, Some(self.parse_formal_parameters()?))
            }
        };
        let body_start = self.pos;
        let (modifier, body) = self.parse_function_body()?;
        self.check_external_body(mods.external, false, body_start, &body, external_top_level);
        self.check_empty_body(&body, Self::allows_abstract(mods, modifier, top_level));
        self.check_abstract_in_extension(mods, top_level, name.span, &body);
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

    /// The modifier errors of a method, getter, setter or operator: the method
    /// branch of `parseTopLevelMemberImpl` (`var` is `VAR_RETURN_TYPE`;
    /// `final`/`const`/`late`, `EXTRANEOUS_MODIFIER`) and the start of
    /// `parseMethod` (`parser_impl.dart:4827`) for members;
    /// fasta's `MemberKind` for the parameters of a top-level function or a
    /// member (`parseMethod`): `covariant` does not fit in a top-level function
    /// or a static method, and has its own code in extensions and extension
    /// types.
    fn parameter_owner(&self, mods: Modifiers, top_level: bool) -> ParameterOwner {
        use super::MemberOwner as D;
        match self.owner {
            _ if top_level => ParameterOwner::TopLevelOrStatic,
            D::Extension => ParameterOwner::Extension,
            D::ExtensionType => ParameterOwner::ExtensionType,
            _ if mods.static_ => ParameterOwner::TopLevelOrStatic,
            _ => ParameterOwner::Other,
        }
    }

    /// Called as soon as the declaration turns out to be a method.
    fn report_method_modifiers(&mut self, mods: Modifiers, top_level: bool, kind: FunctionKind) {
        let f = mods.modifier_tokens;
        if top_level {
            if let Some(v) = f.var_final_or_const() {
                if f.var_.is_some() {
                    self.error_at_token(v, codes::parser::VAR_RETURN_TYPE);
                } else {
                    self.stray_modifier(Some(v));
                }
            } else {
                self.stray_modifier(f.late);
            }
            return;
        }
        if let Some(a) = f.abstract_ {
            self.error_at_token(a, codes::parser::ABSTRACT_CLASS_MEMBER);
        }
        self.stray_modifier(f.late);
        let accessor = matches!(kind, FunctionKind::Getter | FunctionKind::Setter);
        if let Some(st) = f.static_ {
            if kind == FunctionKind::Operator {
                self.error_at_token(st, codes::parser::STATIC_OPERATOR);
            }
        } else if let Some(c) = f.covariant
            && kind != FunctionKind::Setter
        {
            self.error_at_token(c, codes::parser::COVARIANT_MEMBER);
        }
        if let Some(c) = f.const_ {
            if accessor {
                self.stray_modifier(Some(c));
            } else {
                self.error_at_token(c, codes::parser::CONST_METHOD);
            }
        } else if let Some(v) = f.var_ {
            self.error_at_token(v, codes::parser::VAR_RETURN_TYPE);
        } else {
            self.stray_modifier(f.final_);
        }
    }

    /// In an extension, a method with a `;` body and no `external` is
    /// `EXTENSION_DECLARES_ABSTRACT_MEMBER` at the name (at the operator, for
    /// `operator`; `parser_impl.dart:5084`).
    fn check_abstract_in_extension(&mut self, mods: Modifiers, top_level: bool, name: Span, body: &FunctionBody) {
        if !top_level
            && self.owner == super::MemberOwner::Extension
            && matches!(body, FunctionBody::Empty)
            && !mods.external
        {
            self.error_at(codes::parser::EXTENSION_DECLARES_ABSTRACT_MEMBER, name, &[]);
        }
    }

    /// The modifier errors of a field (`parseFields`, `parser_impl.dart:3656`):
    /// `covariant final` without `late` is `FINAL_AND_COVARIANT`;
    /// `covariant late final` with an initializer,
    /// `FINAL_AND_COVARIANT_LATE_WITH_INITIALIZER`; `abstract external`,
    /// `ABSTRACT_EXTERNAL_FIELD`; in an extension, without `static` or
    /// `external`, `EXTENSION_DECLARES_INSTANCE_FIELD` at the first name
    /// (`parser_impl.dart:3809`).
    /// Called with the cursor right after the first name.
    fn report_field_modifiers(&mut self, mods: Modifiers, top_level: bool, name: Span) {
        let f = mods.modifier_tokens;
        if let (Some(c), Some(_)) = (f.covariant, f.final_) {
            if f.late.is_none() {
                self.error_at_token(c, codes::parser::FINAL_AND_COVARIANT);
            } else if self.at_op(Op::Assign) {
                self.error_at_token(c, codes::parser::FINAL_AND_COVARIANT_LATE_WITH_INITIALIZER);
            }
        }
        if let (Some(a), Some(_)) = (f.abstract_, f.external) {
            self.error_at_token(a, codes::parser::ABSTRACT_EXTERNAL_FIELD);
        }
        if !top_level
            && self.owner == super::MemberOwner::Extension
            && f.static_.is_none()
            && f.external.is_none()
        {
            self.error_at(codes::parser::EXTENSION_DECLARES_INSTANCE_FIELD, name, &[]);
        }
    }

    /// `parseFieldInitializerOpt` and the getter/setter branch of `parseMethod`
    /// (`parser_impl.dart:3934`, `:5003`): a field, getter or setter with the
    /// enclosing declaration's name is `MEMBER_WITH_CLASS_NAME` at the name.
    fn check_member_name(&mut self, name: Span) {
        if self.enclosing_name.is_some_and(|n| n == &self.source[name.start..name.end]) {
            self.error_at(codes::parser::MEMBER_WITH_CLASS_NAME, name, &[]);
        }
    }

    /// `= e`? (`, name = e`?)* `;` from the first name already read.
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
            // `ensureIdentifier` in the field/variable declaration context: before `;`,
            // `=`, `,` or `}` (and the end), fasta reports `MISSING_IDENTIFIER` and
            // inserts a synthetic identifier, and the list goes on with the unnamed
            // variable (`Object? foo,;`).
            if matches!(self.kind(), Kind::Op(Op::Semicolon | Op::Assign | Op::Comma | Op::RBrace) | Kind::Eof) {
                self.error(codes::parser::MISSING_IDENTIFIER, &[]);
                let start_offset = self.span().start;
                name = self.name_from("", Span { start: start_offset, end: start_offset });
                continue;
            }
            name = self.expect_identifier()?;
            self.check_member_name(name.span);
        }
        // Without an initializer, the declaration ended at the name: the missing
        // `;` is inserted before any token, as in fasta (`augment Object?
        // foo();` is the field `Object` followed by the stray `?`). With an
        // initializer the expression may have stopped before what fasta would read,
        // and the cautious rule of `ensure_semicolon` applies.
        if variables.last().is_some_and(|v| v.initializer.is_none()) && !self.speculating {
            self.ensure_semicolon_forced();
            return Ok(variables);
        }
        self.ensure_semicolon()?;
        Ok(variables)
    }

    // -----------------------------------------------------------------------
    // Members
    // -----------------------------------------------------------------------

    /// `{ members }` of a class, mixin, extension or extension type. `primary`
    /// says whether the declaration has a primary header: in it, `this` and
    /// `new` open a body part even without the feature on (the 3.6 oracle is
    /// silenced by the header's cascade, which we accept as a superset).
    fn parse_class_body(&mut self, class_name: Option<&'s str>) -> PResult<Vec<MemberId>> {
        self.expect_op(Op::LBrace)?;
        self.parse_member_list(class_name)
    }

    /// Members up to the closing `}` (inclusive), with per-member recovery. A
    /// premature end of file records the error but returns the members already
    /// read.
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

    /// A `classMemberDefinition`: field, method, accessor, operator or
    /// constructor. `class_name` decides whether `Name(` is a constructor.
    fn parse_member(&mut self, class_name: Option<&'s str>) -> PResult<MemberId> {
        self.enclosing_name = class_name;
        let r = self.parse_member_inside(class_name);
        self.enclosing_name = None;
        r
    }

    fn parse_member_inside(&mut self, class_name: Option<&'s str>) -> PResult<MemberId> {
        let start = self.span();
        let metadata = self.parse_metadata()?;
        // `=>` where a member was expected: a method without a name or parameters
        // (fasta flags both at the `=>`, probed on the local SDK 3.6.2).
        if self.at_op(Op::Arrow) {
            let span = self.span();
            self.error_at(codes::parser::MISSING_IDENTIFIER, span, &[]);
            self.error_at(codes::parser::MISSING_METHOD_PARAMETERS, span, &[]);
            return Err(self.skip_broken_member());
        }
        // `this` (a primary constructor part) and `new` (a constructor without the
        // class name) start a member in any version: without the feature, the
        // 3.13.4 analyzer reads the member as with it (with the `this` part's own
        // diagnostics) and only adds `experiment_not_enabled` at the `this`/`new`
        // (probed in a 3.6 library). The superset applies even in a class without
        // a primary header.
        if !self.can_start_declaration() && !self.at_kw(Keyword::This) && !self.at_kw(Keyword::New) {
            return Err(self.error(codes::parser::EXPECTED_CLASS_MEMBER, &[]));
        }
        let augment = self.parse_augment_opt();
        let fstart = self.span();
        let mods = self.parse_modifiers(false);
        // `this` opens a body part with any body (its own diagnostics apply), with
        // or without the feature.
        let part = self.at_kw(Keyword::This) && !self.at_op_at(1, Op::Dot);
        let kind = if part {
            // `this : inits? body` / `this;`: the primary constructor's body part.
            self.parse_primary_part(&mods)?
        } else if self.at_kw(Keyword::New)
            && (self.at_op_at(1, Op::LParen) || self.at_identifier_at(1))
        {
            // `new name?(...)` (3.13): a constructor with the class name implied.
            self.parse_new_constructor(mods, class_name)?
        } else if let Some(end_offset) = self.type_before_new() {
            // 3.13.4's `parseClassMember`: the `new` after the type is a constructor's
            // name (`newToken`, `_isConstructor`): the type is
            // `CONSTRUCTOR_WITH_RETURN_TYPE`, and the rest is `new name?(…)`. 3.6.2
            // would read a method named `new`, but the `new`'s
            // `experiment_not_enabled` already puts the unit on the 3.13 reference.
            let tstart = self.span();
            // The type does not stay in the tree (the `AstBuilder` discards it).
            let types_before = self.ast.types.len();
            let _ty = self.parse_type()?;
            self.ast.types.truncate(types_before);
            self.error_at(codes::parser::CONSTRUCTOR_WITH_RETURN_TYPE, self.span_from(tstart), &[]);
            debug_assert_eq!(self.pos, end_offset);
            self.parse_new_constructor(mods, class_name)?
        } else if let Some(end_offset) = self.type_before_factory() {
            // `parseClassMember`: a type before `factory` is `TYPE_BEFORE_FACTORY` at
            // its last token, and the rest is the factory (`augment factory C.f()`
            // without the experiment).
            let span = self.tokens[end_offset - 1].span;
            self.error_at(codes::parser::TYPE_BEFORE_FACTORY, span, &[]);
            self.pos = end_offset;
            self.parse_constructor(mods, true, class_name)?
        } else if self.at_ident("factory")
            && (self.at_identifier_at(1)
                || (self.features.has(Feature::PrimaryConstructors) && self.at_op_at(1, Op::LParen))
                || self.factory_new_as_constructor())
        {
            // `factory(` is only a constructor from 3.13 on; before that, it is a
            // method named `factory` (the version decides, VERSOES-LINGUAGEM.md §4.5).
            self.parse_constructor(mods, true, class_name)?
        } else if self.constructor_follows(class_name) {
            self.parse_constructor(mods, false, class_name)?
        } else if self.constructor_with_return(class_name) {
            // `T C(` (C is the class) or `T X.Y(` / `T X.Y` without parentheses: a
            // constructor with a return type — fasta reports
            // `constructor_with_return_type` at T and, without parentheses,
            // `missing_method_parameters` at X (probed on the local SDK 3.6.2). Without
            // this, `augment A(...) : inits` fell into the method path and the tail
            // failed in cascade.
            let tstart = self.span();
            // The type does not stay in the tree (the `AstBuilder` discards it): not
            // even the checkers of written types see it.
            let types_before = self.ast.types.len();
            let _ty = self.parse_type()?;
            self.ast.types.truncate(types_before);
            self.error_at(
                codes::parser::CONSTRUCTOR_WITH_RETURN_TYPE,
                self.span_from(tstart),
                &[],
            );
            if self.constructor_follows(class_name) {
                self.parse_constructor(mods, false, class_name)?
            } else if self.method_without_parameters() {
                // `T X.Y` without parentheses (`int C.named;`): X is flagged.
                let span = self.span();
                self.error_at(
                    codes::parser::MISSING_METHOD_PARAMETERS,
                    span,
                    &[],
                );
                return Err(self.skip_broken_member());
            } else {
                // Exotic form (`T get.Y;`): no probed rule, it just fails for recovery
                // without new errors.
                return Err(ParseError);
            }
        } else if self.method_without_parameters() {
            // `X.Y` / `X.new` without `(` in a member (`C.named;`, `foo.bar = 1;`,
            // `C.named : ...`): fasta reports `missing_method_parameters` at X (probed
            // on the local SDK 3.6.2). `X.Y(` is a constructor, `X.Y<` is another
            // group and `X.Y name` may be a field of a prefixed type — those follow the
            // old path. The rest of the member is skipped without new errors (each code
            // has its turn).
            let span = self.span();
            self.error_at(
                codes::parser::MISSING_METHOD_PARAMETERS,
                span,
                &[],
            );
            return Err(self.skip_broken_member());
        } else {
            match self.parse_function_or_variables(mods, fstart, None, false)? {
                FunctionOrVariables::Function(id) => MemberKind::Method(id),
                FunctionOrVariables::Variables(list) => {
                    // fasta `AstBuilder._endClassFields`: the restriction no longer applies
                    // with the augmentations experiment.
                    if mods.static_ && !self.features.has(Feature::Augmentations) {
                        if let Some(span) = mods.abstract_span {
                            self.diagnostics.push(Diagnostic::with_code(
                                codes::parser::ABSTRACT_STATIC_FIELD,
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

    /// Skips the rest of the broken member up to `;` (consumed) or `}`/end
    /// (kept), without new errors — each code has its turn — and returns the
    /// failure so that recovery continues afterwards.
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

    /// Do `X.Y` / `X.new` without `(` or `<` start here, with the member clearly
    /// finished afterwards (`;`, `=`, `,`, `:`, `=>`, closer, end)?
    /// `get`/`set`/`operator`/`typedef`/`factory` and names followed by a name
    /// (`p.Foo x`) follow the old path.
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

    /// Do `T C(` (C is the class) or `T X.Y(` start here? `get`/`set`/
    /// `operator`/`typedef`/`factory` have their own path and `void` follows the
    /// old one; the rest with a constructor shape after the type is a return
    /// type.
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
        // With parentheses (`T X.Y(`) the constructor is read; without (`T X.Y;`,
        // `int C.named;`) the X still falls into the parameterless-method rule.
        // `X.Y<` is another group and `X.Y name` may be a prefixed field.
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

    /// `Name(` with the class name (or after `const`), or `Name.x(`.
    fn constructor_follows(&self, class_name: Option<&str>) -> bool {
        if !self.at_identifier() {
            return false;
        }
        if self.at_op_at(1, Op::LParen) {
            // `parseMethod`: with an initializer list (`Name(...) :`) it is a
            // constructor whatever the name (`INVALID_CONSTRUCTOR_NAME` if it is not
            // the class's).
            let with_initializers = self
                .matching_close(self.pos + 1)
                .is_some_and(|f| self.kind_of(f + 1) == Kind::Op(Op::Colon));
            // `const name(` with another name is a method (`CONST_METHOD`), as in
            // fasta's `parseMethod`.
            return class_name == Some(self.text()) || with_initializers;
        }
        // `Name<T>(` with the class name: a constructor with type parameters (an
        // error, reported in `parse_constructor`).
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

    /// `factory? C(.name)? (params) (= Target.name; | (: inits)? body)`. In
    /// 3.13 also `factory name?(...)`: without the class name (`factory(`) it is
    /// the unnamed constructor, `factory id(` is `C.id` — except `id` = `C`,
    /// which stays the unnamed one (spec, the exception so as not to break old
    /// code).
    fn parse_constructor(
        &mut self,
        mods: Modifiers,
        factory: bool,
        class: Option<&'s str>,
    ) -> PResult<MemberKind> {
        if factory {
            // `parseFactoryMethod` (`parser_impl.dart:5108`): `static`, `covariant`,
            // `var` and `final` before `factory` are `EXTRANEOUS_MODIFIER`; `abstract`,
            // `ABSTRACT_CLASS_MEMBER` (`parseClassOrMixinOrExtensionOrEnumMemberImpl`).
            let f = mods.modifier_tokens;
            if let Some(a) = f.abstract_ {
                self.error_at_token(a, codes::parser::ABSTRACT_CLASS_MEMBER);
            }
            self.stray_modifier(f.static_.or(f.covariant));
            self.stray_modifier(f.var_.or(f.final_));
            self.check_constructor_in_mixin_or_extension(self.span());
            let t = self.advance();
            if self.at_op(Op::LParen) {
                // `factory(...)`: only reaches here in 3.13 (before that it is a method).
                let class_name = self.class_name(class, t.span);
                return self.parse_constructor_rest(mods, true, class_name, None);
            }
            if self.features.has(Feature::PrimaryConstructors) && self.at_op_at(1, Op::LParen) {
                let id = self.expect_identifier()?;
                let class_name = self.class_name(class, t.span);
                let name = (Some(&self.source[id.span.start..id.span.end]) != class).then_some(id);
                return self.parse_constructor_rest(mods, true, class_name, name);
            }
        }
        // `factory new(…)` (3.13.4, `parseFactoryMethod`): the `new` is the name,
        // with `FACTORY_CONSTRUCTOR_NEW_NAME`; a name that is not the class's is,
        // afterwards, `INVALID_FACTORY_NAME_NOT_A_CLASS`.
        if factory && self.at_kw(Keyword::New) && !self.at_op_at(1, Op::Dot) {
            let t = self.advance();
            self.error_at(codes::parser::FACTORY_CONSTRUCTOR_NEW_NAME, t.span, &[]);
            let class_name = self.name_from("new", t.span);
            return self.parse_constructor_rest(mods, true, class_name, None);
        }
        let class_name = self.expect_identifier()?;
        if !factory {
            self.check_constructor_in_mixin_or_extension(class_name.span);
            // `parseMethod` (`parser_impl.dart:5014`): a constructor with another name
            // (`Other.x()`) is `INVALID_CONSTRUCTOR_NAME` at the name; `static`,
            // `STATIC_CONSTRUCTOR`.
            if Some(&self.source[class_name.span.start..class_name.span.end]) != class {
                self.error_at(codes::parser::INVALID_CONSTRUCTOR_NAME, class_name.span, &[]);
            }
            if let Some(st) = mods.modifier_tokens.static_ {
                self.error_at_token(st, codes::parser::STATIC_CONSTRUCTOR);
            }
            // `AstBuilder.endClassConstructor` (`ast_builder.dart:5897`): type
            // parameters on a constructor are `TYPE_PARAMETER_ON_CONSTRUCTOR` over the
            // list, which is read.
            if self.at_op(Op::Lt) {
                let opener = self.span();
                self.parse_type_parameters_opt()?;
                let list = self.span_from(opener);
                self.error_at(codes::parser::TYPE_PARAMETER_ON_CONSTRUCTOR, list, &[]);
            }
        }
        let name = if self.eat_op(Op::Dot) {
            Some(self.identifier_or_new()?)
        } else {
            None
        };
        self.parse_constructor_rest(mods, factory, class_name, name)
    }

    /// A constructor in a mixin or extension: `MIXIN_DECLARES_CONSTRUCTOR` /
    /// `EXTENSION_DECLARES_CONSTRUCTOR` at the name (at the `factory`, if any;
    /// `parser_impl.dart:5042` and `:5182`).
    fn check_constructor_in_mixin_or_extension(&mut self, span: Span) {
        let code = match self.owner {
            super::MemberOwner::Mixin => codes::parser::MIXIN_DECLARES_CONSTRUCTOR,
            super::MemberOwner::Extension => codes::parser::EXTENSION_DECLARES_CONSTRUCTOR,
            _ => return,
        };
        self.error_at(code, span, &[]);
    }

    /// The owner class's `Name` for a constructor written without it (`new`,
    /// `factory(`, `this` part), with the keyword's range.
    fn class_name(&mut self, class: Option<&str>, span: Span) -> Name {
        let text = class.unwrap_or("").to_string();
        self.name_from(&text, span)
    }

    /// `new name?(params) (: inits)? body` (3.13): the same as
    /// `C.name(...)`/`C(...)`.
    fn parse_new_constructor(
        &mut self,
        mods: Modifiers,
        class: Option<&'s str>,
    ) -> PResult<MemberKind> {
        let t = self.advance();
        self.require_in_ast(Feature::PrimaryConstructors, t.span);
        // `_isConstructor` with `newToken`: the `static` is `STATIC_CONSTRUCTOR`
        // (3.13.4's `parseMethod`).
        if let Some(st) = mods.modifier_tokens.static_ {
            self.error_at_token(st, codes::parser::STATIC_CONSTRUCTOR);
        }
        let class_name = self.class_name(class, t.span);
        let name = if self.at_identifier() {
            Some(self.identifier())
        } else {
            None
        };
        self.parse_constructor_rest(mods, false, class_name, name)
    }

    /// `this (: inits)? body` (3.13): the primary constructor's body part. It
    /// becomes a `Constructor { primary_body_part: true }` that elaboration merges
    /// into the declaration's `k2`. `async`/`sync*`/`=>` are errors.
    /// The `this` of a primary constructor part (its `class_name`).
    fn this_span(&self, member: MemberId) -> Span {
        match &self.ast.member(member).kind {
            MemberKind::Constructor(c) => c.class_name.span,
            _ => self.ast.member(member).span,
        }
    }

    fn parse_primary_part(&mut self, mods: &Modifiers) -> PResult<MemberKind> {
        let t = self.advance();
        self.require_in_ast(Feature::PrimaryConstructors, t.span);
        // The modifiers before the `this` are `EXTRANEOUS_MODIFIER`, in this order
        // (3.13.4's `parseClassOrMixinOrExtensionOrEnumMemberImpl`,
        // `parser_impl.dart:5514-5556`): `var`/`final`/`const`, `external`,
        // `static`, `covariant`, `late`.
        let f = mods.modifier_tokens;
        for p in [f.var_final_or_const(), f.external, f.static_, f.covariant, f.late].into_iter().flatten() {
            let tok = self.tokens[p];
            let text = self.text_of(p).to_string();
            self.error_at(codes::parser::EXTRANEOUS_MODIFIER, tok.span, &[&text]);
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
        let before = self.pos;
        let (modifier, body) = self.parse_function_body()?;
        // `async`, `async*`, `sync*` before the body: the analyzer reports at the
        // `async`/`sync`, with the whole modifier in the text. The body (`=>`/`{`)
        // is judged during elaboration, which knows whether it is `const`.
        let mut body_node = before;
        if modifier != AsyncModifier::None {
            let text = match modifier {
                AsyncModifier::Async => "async",
                AsyncModifier::AsyncStar => "async*",
                _ => "sync*",
            };
            let span = self.tokens[before].span;
            self.error_at(codes::parser::PRIMARY_CONSTRUCTOR_BODY_WITH_MODIFIER, span, &[text]);
            body_node += if text.ends_with('*') { 2 } else { 1 };
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
            primary_body_part: true,
        }))
    }

    /// What follows a constructor's name: parameters, factory redirection or
    /// initializer list, and the body.
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
        // The AstBuilder's `_buildConstructorDeclaration`
        // (`ast_builder.dart:5911-5920`): an `external` (non-factory) constructor,
        // one error per `this.x`, at the `this`.
        if mods.external && !factory {
            let source = self.source;
            for p in parameters.iter().filter(|p| p.this_) {
                let until = p.name.map_or(p.span.end, |n| n.span.start);
                if let Some(i) = source.get(p.span.start..until).and_then(|s| s.rfind("this")) {
                    let start_ = p.span.start + i;
                    self.error_at(codes::parser::EXTERNAL_CONSTRUCTOR_WITH_FIELD_INITIALIZERS, Span { start: start_, end: start_ + 4 }, &[]);
                }
            }
        }
        let mut initializers = Vec::new();
        let mut redirect = None;
        // `parseFactoryMethod` (`parser_impl.dart:5139-5145`): the body modifier of
        // a factory is `NON_SYNC_FACTORY`, at the `async`/`sync` token, before the
        // `=` test — it also applies to the redirecting factory, which has no body
        // (there the modifier is just skipped).
        if factory && (self.at_ident("async") || (self.at_ident("sync") && self.at_op_at(1, Op::Star))) {
            let span = self.span();
            self.error_at(codes::compile_time_error::NON_SYNC_FACTORY, span, &[]);
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
                self.error_at(codes::parser::EXTERNAL_FACTORY_REDIRECTION, eq.span, &[]);
            }
            if !factory {
                // `Foo() = Bar;` without `factory`: the target is read normally and only
                // the `=` is flagged (fasta 3.6.2, without a cascade).
                self.error_at(
                    codes::parser::REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR,
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
            // `parseMethod` (`parser_impl.dart:5030`): an `external` constructor with
            // an initializer list is `EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER` at the
            // `:`.
            if mods.external && !factory && self.at_op(Op::Colon) {
                self.error(codes::parser::EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER, &[]);
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
            // `_checkForInvalidModifierOnBody` (`error_verifier.dart:4044-4053`): a
            // constructor (generative or factory) with `async`, `async*` or `sync*`, at
            // the modifier's first token; the argument is its text (`async` also for
            // `async*`).
            if modifier != AsyncModifier::None {
                let span = self.tokens[body_start].span;
                let word = if modifier == AsyncModifier::SyncStar { "sync" } else { "async" };
                self.error_at(codes::compile_time_error::INVALID_MODIFIER_ON_CONSTRUCTOR, span, &[word]);
            }
            self.check_external_body(mods.external, factory, body_start, &body, None);
            // `parseFactoryMethod`: only an `external` factory may omit the body; the
            // generative constructor follows the method rule (`parseMethod`).
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
                        self.error_at(codes::parser::CONST_CONSTRUCTOR_WITH_BODY, span, &[]);
                    }
                }
            }
            body
        };
        // The SDK declares `external` constant factories (for example
        // `bool.fromEnvironment`); the ErrorVerifier only applies `constFactory` to
        // the branch without `externalKeyword`.
        if factory && !mods.external && redirect.is_none() {
            if let Some(span) = mods.const_span {
                self.error_at(codes::parser::CONST_FACTORY, span, &[]);
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
            primary_body_part: false,
        }))
    }

    /// One entry of the initializer list: `super(...)`, `super.n(...)`,
    /// `this(...)`/`this.n(...)` (redirection), `this.x = e`, `x = e` or
    /// `assert(c, m)`.
    fn parse_initializer(&mut self) -> PResult<Initializer> {
        // `parseInitializers` do fasta desliga `mayParseFunctionExpressions`.
        let saved = std::mem::replace(&mut self.without_named_function, true);
        let read_ = self.parse_initializer_inner();
        self.without_named_function = saved;
        read_
    }

    fn parse_initializer_inner(&mut self) -> PResult<Initializer> {
        let start = self.span();
        if self.eat_kw(Keyword::Super) {
            // `super?.x()`: `INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER` at the
            // `?.` (`parseSuperExpression`), read as `.`.
            if self.at_op(Op::QuestionDot) {
                let span = self.span();
                self.error_at(codes::parser::INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER, span, &[]);
            }
            let mut via_primary = false;
            let constructor = if self.eat_op(Op::Dot) || self.eat_op(Op::QuestionDot) {
                // `parseSuperInitializerExpression`: the `ensureIdentifier`
                // (`expressionContinuation`) and, in `parseInitializerExpressionRest`, the
                // reread as an expression. A reserved word gets both errors
                // (`super.const()`: the word as a name and, reread by `parsePrimary`, the
                // `const ()` rejected as a name).
                if self.at_kw(Keyword::New) || self.at_identifier() {
                    Some(self.identifier_or_new()?)
                } else if matches!(self.kind(), Kind::Keyword(_)) && !self.looks_like_statement_start(self.pos) {
                    let text = self.text().to_string();
                    let span = self.span();
                    self.error_at(codes::parser::EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD, span, &[&text]);
                    via_primary = true;
                    Some(self.member_name(true)?)
                } else {
                    Some(self.member_name(false)?)
                }
            } else {
                None
            };
            // `super` or `super.x` without `(` or `=`: `EXPECTED_TOKEN` (`(`) at the
            // next token and synthetic parentheses (`insertParens`, in
            // `parseSuperInitializerExpression`).
            if !via_primary && !self.at_op(Op::LParen) && !self.at_op(Op::Assign) {
                self.error(codes::parser::EXPECTED_TOKEN, &["("]);
                let s = self.span().start;
                return Ok(Initializer::Super {
                    span: self.span_from(start),
                    constructor,
                    arguments: crate::ast::Arguments { span: Span { start: s, end: s }, type_args: Box::default(), args: Box::default() },
                });
            }
            // `buildInitializer`: reread by `parsePrimary`, `super.x` without arguments
            // is not a call to the superclass constructor
            // (`INVALID_SUPER_IN_INITIALIZER`).
            if via_primary && !self.at_op(Op::LParen) {
                self.error_at(codes::parser::INVALID_SUPER_IN_INITIALIZER, Span { start: start.start, end: start.start + 5 }, &[]);
                self.skip_rest_of_initializer();
                let s = self.span().start;
                // Not a constructor invocation in the `AstBuilder`: without the name, so
                // that analysis does not look for the `const` constructor.
                let _ = constructor;
                return Ok(Initializer::Super {
                    span: self.span_from(start),
                    constructor: None,
                    arguments: crate::ast::Arguments { span: Span { start: s, end: s }, type_args: Box::default(), args: Box::default() },
                });
            }
            let arguments = self.parse_arguments()?;
            // `buildInitializer`: `super(…)`/`super.n(…)` followed by more selectors is
            // not a call to the superclass constructor: `INVALID_SUPER_IN_INITIALIZER`
            // at the `super` (the node becomes the call with the last argument list).
            if self.selector_after_initializer() {
                self.error_at(codes::parser::INVALID_SUPER_IN_INITIALIZER, Span { start: start.start, end: start.start + 5 }, &[]);
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
                        self.error_at(codes::parser::INVALID_THIS_IN_INITIALIZER, Span { start: start.start, end: start.start + 4 }, &[]);
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
                // `this` without `.` or `(`: `EXPECTED_TOKEN` (`.`) at what follows
                // (`parseInitializer`, `parser_impl.dart:4098`).
                self.expected_error(".");
                return self.initializer_without_assignment(start);
            }
            let arguments = self.parse_arguments()?;
            if self.selector_after_initializer() {
                self.error_at(codes::parser::INVALID_THIS_IN_INITIALIZER, Span { start: start.start, end: start.start + 4 }, &[]);
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
            // `parseAssert` turns function literals back on.
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

    /// `parseInitializer` (`parser_impl.dart:4089`): `this(...)` followed by
    /// `{` or `=>` is `REDIRECTING_CONSTRUCTOR_WITH_BODY` at it.
    fn check_redirecting_body(&mut self) {
        if self.at_op(Op::LBrace) || self.at_op(Op::Arrow) {
            self.error(codes::parser::REDIRECTING_CONSTRUCTOR_WITH_BODY, &[]);
        }
    }

    /// The final recovery of `parseInitializer` (`parser_impl.dart:4134`):
    /// `this.x`/`this` without an assignment is
    /// `MISSING_ASSIGNMENT_IN_INITIALIZER` at the `this`, and the initializer
    /// becomes `<synthetic> = expression`, with the expression read from the
    /// `this`.
    /// After the arguments of `super(…)`/`this(…)` comes another selector
    /// (`.`, `?.`, `..`, `?..`, `[`, `(`).
    fn selector_after_initializer(&self) -> bool {
        matches!(
            self.kind(),
            Kind::Op(Op::Dot | Op::QuestionDot | Op::DotDot | Op::QuestionDotDot | Op::LBracket | Op::LParen)
        )
    }

    /// Skips the rest of an initializer (the selectors, with matched groups)
    /// up to the next `,`, the body (`{`, `=>`), the `;` or the end.
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
        self.error_at(codes::parser::MISSING_ASSIGNMENT_IN_INITIALIZER, start, &[]);
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
    // Function bodies
    // -----------------------------------------------------------------------

    /// The official parser reports a top-level function at the `external`
    /// token, but members and constructors at the block's `{` or at the `=>`
    /// (fasta's `parseTopLevelMethod` and `parseMethod`/`parseFactoryMethod`).
    /// The range from `start_offset` starts before the `async` modifier and
    /// only includes this function's tokens, so the first delimiter is the
    /// body.
    fn check_external_body(&mut self, external: bool, factory: bool, start_offset: usize, body: &FunctionBody, external_top_level: Option<Span>) {
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
        if let Some(span) = external_top_level.or(span) {
            let code = if factory {
                codes::parser::EXTERNAL_FACTORY_WITH_BODY
            } else {
                codes::parser::EXTERNAL_METHOD_WITH_BODY
            };
            self.error_at(code, span, &[]);
        }
    }

    /// The end of the type that precedes `factory name` in a member, if any.
    /// `void` followed by `new` and by `(` or an identifier
    /// (`static void new() {}`): the end of the type, at the `new`. Only
    /// `void`: 3.13.4's `computeType` only accepts the other types when the
    /// next token looks like a name (`looksLikeName`), and `new` does not —
    /// `int new() => 1` stays a method with an invalid name
    /// (`EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD`), as in 3.6.2.
    fn type_before_new(&self) -> Option<usize> {
        if !self.at_kw(Keyword::Void) {
            return None;
        }
        let end_offset = self.pos + 1;
        let n = 1;
        (self.at_kw_at(n, Keyword::New) && (self.at_op_at(n + 1, Op::LParen) || self.at_identifier_at(n + 1))).then_some(end_offset)
    }

    /// `factory new(` (without `.` after the `new`): 3.13.4 reads a factory
    /// constructor named `new`; 3.6.2, a `factory` field (`new` is not an
    /// identifier). Outside 3.13 mode, it follows 3.6.2 and records the
    /// divergence ([`Parser::diverged_313`]).
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

    /// The `allowAbstract` with which fasta reads the body: at top level
    /// (`parseTopLevelMethod`), only `external` makes the body optional; in a
    /// member (`parseMethod`), whatever is not `static` (or is `external`) and
    /// has no `async`/`sync*` modifier (`inPlainSync`).
    fn allows_abstract(mods: Modifiers, modifier: AsyncModifier, top_level: bool) -> bool {
        if top_level {
            mods.external
        } else {
            (!mods.static_ || mods.external) && modifier == AsyncModifier::None
        }
    }

    /// fasta's `parseFunctionBody`: a `;` body where `allowAbstract` is false
    /// gives `MISSING_FUNCTION_BODY` at the `;` (the tree keeps the empty
    /// body).
    fn check_empty_body(&mut self, body: &FunctionBody, allows_abstract: bool) {
        if !allows_abstract && matches!(body, FunctionBody::Empty) && self.pos > 0 {
            let span = self.tokens[self.pos - 1].span;
            self.error_at(codes::parser::MISSING_FUNCTION_BODY, span, &[]);
        }
    }

    /// Optional `async`/`async*`/`sync*` followed by `{...}`, `=> e;`, `;` or
    /// `native ...;`. Adjusts `in_async`/`in_generator` during the body.
    pub(crate) fn parse_function_body(&mut self) -> PResult<(AsyncModifier, FunctionBody)> {
        self.parse_function_body_ex(true)
    }

    /// Like [`Parser::parse_function_body`], but in function expressions
    /// (`(x) => x + 1`) there is no `;` after the expression: pass `false`.
    ///
    /// The body's `async`/generator context is that of the modifier read here —
    /// a nested function does not inherit the outer one — and the previous
    /// context is restored on exit, even on error.
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

    /// `async`, `async*`, `sync*` (contextual identifiers; `*` is a separate
    /// token) or nothing.
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

    /// The body proper, after the modifier.
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
        // `parseFunctionBody` recovery (`parser_impl.dart`): `=` or `return` in
        // place of `=>` are `MISSING_FUNCTION_BODY` at them and the expression is
        // read as a `=>` body; a stray word before `=>` or `{` is
        // `UNEXPECTED_TOKEN` and is skipped; anything else is
        // `MISSING_FUNCTION_BODY` at it, with a synthetic empty block, without
        // consuming anything (`ensureBlock`).
        if self.at_op(Op::Assign) || self.at_kw(Keyword::Return) {
            self.error(codes::parser::MISSING_FUNCTION_BODY, &[]);
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
            let text = self.text().to_string();
            self.error(codes::parser::UNEXPECTED_TOKEN, &[&text]);
            self.advance();
            return self.parse_body_after_modifier(expect_semicolon);
        }
        self.error(codes::parser::MISSING_FUNCTION_BODY, &[]);
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

    /// Extension type representation by `AstBuilder.endPrimaryConstructor` and
    /// type parameters on operators (checked against `dart analyze` 3.6.2).
    #[test]
    fn representation_and_operator_with_type_parameters() {
        let source = "// @dart = 3.6\n\
                     extension type A(final int x) {}\n\
                     extension type C([int x = 0]) {}\n\
                     extension type D(x) {}\n\
                     extension type E(int x,) {}\n\
                     extension type F(int x, int y) {}\n\
                     extension type G() {}\n\
                     extension type H(this.x) {}\n\
                     class K { bool operator ==<T>(Object o) => true; }\n";
        let mut names = Interner::new();
        let features = crate::features::LibraryFeatures::floor();
        let out = crate::parser::parse_with(source, &mut names, features);
        let v: Vec<(&str, &str)> = out
            .diagnostics
            .iter()
            .map(|d| (d.code.map_or("", |c| c.info().name), &source[d.span.start..d.span.end]))
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

    /// `MISSING_FUNCTION_BODY` at the `;` where fasta reads the body without
    /// `allowAbstract` (checked against `dart analyze` 3.6.2): top-level
    /// function and accessor without `external`, `static` member, `async`
    /// member, factory without `external`. An instance method, a generative
    /// constructor and `external` accept `;`.
    #[test]
    fn empty_body_without_allow_abstract() {
        let source = "class A {\n  static int get foo;\n  static set foo(int _);\n  static void m();\n  void i();\n  \
                     A();\n  factory A.f();\n  external factory A.g();\n  void a() async;\n  external static void e();\n}\n\
                     void f(int x);\nint get g;\nexternal void h();\n";
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        let lines_: Vec<usize> = out
            .diagnostics
            .iter()
            .map(|d| {
                assert_eq!(d.code, Some(dartforge_diagnostics::codes::parser::MISSING_FUNCTION_BODY), "{d:?}");
                assert_eq!(&source[d.span.start..d.span.end], ";");
                source[..d.span.start].matches('\n').count() + 1
            })
            .collect();
        assert_eq!(lines_, [2, 3, 4, 7, 9, 12, 13]);
    }

    fn parse_ok(src: &str, names: &mut Interner) -> Parsed {
        let out = parse(src, names);
        assert!(out.diagnostics.is_empty(), "{src}: {:?}", out.diagnostics);
        out
    }

    /// Like [`parse_ok`], tolerating only directive-order errors (the samples
    /// put `library`, `part` and `part of` together in a single unit).
    fn parse_ok_except_order(src: &str, names: &mut Interner) -> Parsed {
        use dartforge_diagnostics::codes::parser as c;
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
            other => panic!("expected a class, got {other:?}"),
        }
    }

    fn member<'a>(out: &'a Parsed, class: &crate::ast::ClassDecl, i: usize) -> &'a MemberKind {
        &out.ast.member(class.members[i]).kind
    }

    /// `extension`, `get`, `set`, `mixin`… are built-in identifiers: at top
    /// level, after a type, with parameters, they are a function's name
    /// (`String extension(String path, [int level = 1])` from `package:path`).
    #[test]
    fn builtin_identifier_as_top_level_function_name() {
        let mut names = Interner::new();
        let out = parse_ok(
            "String extension(String p, [int n = 1]) => p;\nT get<T>(T x) => x;\nint mixin() => 1;\n",
            &mut names,
        );
        for (i, expected_) in ["extension", "get", "mixin"].into_iter().enumerate() {
            let DeclKind::Function(f) = decl(&out, i) else { panic!("expected a function") };
            assert_eq!(text(&names, out.ast.function(*f).name.expect("name")), expected_);
        }
    }

    #[test]
    fn external_body_marks_block_opening_or_arrow_with_official_code() {
        use dartforge_diagnostics::codes::parser as c;
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
        use dartforge_diagnostics::codes::parser as c;
        for (src, code, start_offset, message, correction) in [
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
                    && d.message == message && d.correction().as_deref() == correction
            ), "{src}: {:?}", out.diagnostics);
        }
    }

    #[test]
    fn external_top_level_function_points_to_external_like_fasta() {
        use dartforge_diagnostics::codes::parser as c;
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/executable_body/ExecutableBody__topLevelFunction_extern_0464d54b.dart"));
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        assert!(out.diagnostics.iter().any(|d|
            d.code == Some(c::EXTERNAL_METHOD_WITH_BODY)
                && d.span.start == 43 && d.span.end == 51
                && d.message == "An external or native method can't have a body."
        ), "{out:?}");
    }

    #[test]
    fn abstract_static_field_without_augmentations_points_to_abstract() {
        use crate::features::{Feature, LanguageVersion, LibraryFeatures};
        use crate::parser::parse_with;
        use dartforge_diagnostics::codes::parser as c;

        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/executable_body/ExecutableBody__class_staticField_abstr_5d2b9f54.dart"));
        let span = source.find("abstract int foo").unwrap();
        let mut names = Interner::new();
        let without_feature = parse_with(source, &mut names, LibraryFeatures::new(LanguageVersion::FLOOR, &[]));
        assert!(without_feature.diagnostics.iter().any(|d|
            d.code == Some(c::ABSTRACT_STATIC_FIELD)
                && d.span.start == span && d.span.end == span + "abstract".len()
                && d.message == "Static fields can't be declared 'abstract'."
                && d.correction().as_deref() == Some("Try removing the 'abstract' or 'static' keyword.")
        ), "{:?}", without_feature.diagnostics);

        let with_feature = parse_with(source, &mut names, LibraryFeatures::new(LanguageVersion::FLOOR, &[Feature::Augmentations]));
        assert!(!with_feature.diagnostics.iter().any(|d| d.code == Some(c::ABSTRACT_STATIC_FIELD)));
    }

    #[test]
    fn const_constructor_body_points_to_delimiter_like_analyzer() {
        use dartforge_diagnostics::codes::parser as c;
        for (source, start_offset, end_offset) in [
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
            let parsed = parse(source, &mut names);
            assert!(parsed.diagnostics.iter().any(|d|
                d.code == Some(c::CONST_CONSTRUCTOR_WITH_BODY)
                    && d.span.start == start_offset && d.span.end == end_offset
                    && d.message == "Const constructors can't have a body."
                    && d.correction().as_deref() == Some("Try removing either the 'const' keyword or the body.")
            ), "{source}: {:?}", parsed.diagnostics);
        }

        let mut names = Interner::new();
        let parsed = parse("class C { const C(); const factory C.x() => C(); }", &mut names);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_CONSTRUCTOR_WITH_BODY)));
    }

    #[test]
    fn const_factory_without_redirect_points_to_const() {
        use dartforge_diagnostics::codes::parser as c;
        for (source, start_offset) in [
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
            let parsed = parse(source, &mut names);
            assert!(parsed.diagnostics.iter().any(|d|
                d.code == Some(c::CONST_FACTORY) && d.span.start == start_offset && d.span.end == start_offset + 5
                    && d.message == "Only redirecting factory constructors can be declared to be 'const'."
                    && d.correction().as_deref() == Some("Try removing the 'const' keyword, or replacing the body with '=' followed by a valid target.")
            ), "{source}: {:?}", parsed.diagnostics);
        }

        let mut names = Interner::new();
        let parsed = parse("class A { const A(); const factory A.named() = A; }", &mut names);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_FACTORY)));

        // SDK 3.6.2 declarations: lib/core/bool.dart and lib/core/int.dart.
        let sdk = "class bool { external const factory bool.fromEnvironment(String name, {bool defaultValue = false}); external const factory bool.hasEnvironment(String name); } class int { external const factory int.fromEnvironment(String name, {int defaultValue = 0}); }";
        let parsed = parse(sdk, &mut names);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_FACTORY)), "{:?}", parsed.diagnostics);

        let corpus = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/linguagem/const/native_factory_test.dart"));
        let parsed = parse(corpus, &mut names);
        assert!(!parsed.diagnostics.iter().any(|d| d.code == Some(c::CONST_FACTORY)), "{:?}", parsed.diagnostics);
    }

    // -- Independent of the other modules --------------------------------------

    #[test]
    fn script_tag_and_library() {
        let mut names = Interner::new();
        let out = parse_ok("#!/usr/bin/env dart\nlibrary a.b.c;\n", &mut names);
        assert!(out.unit.script_tag.is_some());
        assert_eq!(out.unit.directives.len(), 1);
        match &out.unit.directives[0].kind {
            DirectiveKind::Library { name } => {
                let parts: Vec<_> = name.iter().map(|n| text(&names, *n)).collect();
                assert_eq!(parts, ["a", "b", "c"]);
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
        // `class {}` becomes a class with an empty synthetic name (as in fasta);
        // `enum {}` still falls into top-level recovery.
        assert_eq!(out.unit.declarations.len(), 4);
        assert_eq!(text(&names, class(&out, 0).name), "");
        assert_eq!(text(&names, class(&out, 1).name), "A");
        assert_eq!(text(&names, class(&out, 2).name), "B");
    }

    #[test]
    fn recovery_skips_the_broken_declaration_body() {
        let mut names = Interner::new();
        let out = parse("class A B { var x; } class C {}", &mut names);
        // `ensureBlock`: `EXPECTED_CLASS_BODY` at the `A` and a synthetic body;
        // `B { … }` is a function without parameters (`MISSING_FUNCTION_PARAMETERS`).
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

    /// An invalid member in fasta's style: one error per token, without
    /// swallowing the next valid member (probed on the local SDK 3.6.2: `42`
    /// gives a single `EXPECTED_CLASS_MEMBER` and `int x` survives; likewise
    /// `;` and `)`).
    #[test]
    fn fasta_member_stray_token_skips_one_and_keeps_valid() {
        use dartforge_diagnostics::codes::parser as c;

        for (garbage, start_offset, end_offset) in [("42", 12, 14), (";", 12, 13), (")", 12, 13)] {
            let source = format!("class C {{\n  {garbage}\n  int x = 1;\n}}\n");
            let mut names = Interner::new();
            let out = parse(&source, &mut names);
            assert_eq!(out.diagnostics.len(), 1, "{source}: {:?}", out.diagnostics);
            let single = &out.diagnostics[0];
            assert_eq!(single.code, Some(c::EXPECTED_CLASS_MEMBER), "{source}");
            assert_eq!((single.span.start, single.span.end), (start_offset, end_offset), "{source}");
            assert_eq!(out.unit.declarations.len(), 1, "{source}");
            let field_ = match member(&out, class(&out, 0), 0) {
                MemberKind::Field(list) => list,
                other => panic!("{source}: {other:?}"),
            };
            assert_eq!(text(&names, field_.variables[0].name), "x", "{source}");
        }
    }

    /// The missing `;` is pointed at the previous token (`1`) and inserted, as
    /// in fasta's `ensureSemicolon`: `x` stays, `garbage` becomes an untyped
    /// field (`MISSING_CONST_FINAL_VAR_OR_TYPE`) and `var y` is not swallowed
    /// (checked against `dart analyze` 3.6.2).
    #[test]
    fn fasta_member_boundary_does_not_swallow_valid() {
        use dartforge_diagnostics::codes::parser as c;

        let source = "class C {\n  var x = 1 garbage;\n  var y = 2;\n}\n";
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        assert!(
            out.diagnostics.iter().any(|d|
                d.code == Some(c::EXPECTED_TOKEN)
                    && d.span.start == 20
                    && d.span.end == 21
                    && d.message == "Expected to find ';'."),
            "{source}: {:?}",
            out.diagnostics
        );
        let class = class(&out, 0);
        let field_names: Vec<_> = class.members.iter().map(|m| match &out.ast.member(*m).kind {
            MemberKind::Field(list) => text(&names, list.variables[0].name).to_string(),
            other => panic!("{source}: {other:?}"),
        }).collect();
        assert_eq!(field_names, ["x", "garbage", "y"], "{source}: {:?}", out.diagnostics);
    }

    /// Valid members of every shape never touch recovery.
    #[test]
    fn valid_member_does_not_touch_recovery() {
        let mut names = Interner::new();
        let out = parse_ok(
            "class C { int x = 1; static var y; C(); C.nomeado(this.x); factory C.f() => C(); get g => x; set s(int v) {} int m() => 1; }",
            &mut names,
        );
        assert_eq!(class(&out, 0).members.len(), 8);
    }

    /// Without the feature, `augment` becomes the type of an `int` field (the
    /// missing `;` is inserted after it, as in fasta's `ensureSemicolon`) and
    /// the `get foo => 0;` tail is kept as a member — `dart analyze` 3.6.2
    /// gives `expected_token` at `int` and `undefined_class` at `augment`.
    #[test]
    fn member_speculative_recovery_keeps_valid_tail() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_with;
        use dartforge_diagnostics::codes::parser as c;

        let source = "class C {\n  augment int get foo => 0;\n}\n";
        let mut names = Interner::new();
        let out = parse_with(source, &mut names, LibraryFeatures::new(LanguageVersion::FLOOR, &[]));
        assert_eq!(out.diagnostics.len(), 1, "{source}: {:?}", out.diagnostics);
        assert_eq!(out.diagnostics[0].code, Some(c::EXPECTED_TOKEN));
        let class = class(&out, 0);
        assert_eq!(class.members.len(), 2, "{source}: {:?}", out.diagnostics);
        assert_eq!((out.diagnostics[0].span.start, out.diagnostics[0].span.end), (20, 23));
        match member(&out, class, 1) {
            MemberKind::Method(id) => {
                assert_eq!(text(&names, out.ast.function(*id).name.unwrap()), "foo");
            }
            other => panic!("{source}: {other:?}"),
        }
    }

    /// `extension type A {}`: `MISSING_PRIMARY_CONSTRUCTOR` at the name (the
    /// last token read) and the body read normally; `A.n {}`,
    /// `MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS` at `n` (fasta 3.6.2).
    #[test]
    fn extension_type_without_primary_constructor() {
        use dartforge_diagnostics::codes::parser as c;
        for (source, code, start_offset, end_offset, members_) in [
            ("extension type A<T> { int get x => 0; }", c::MISSING_PRIMARY_CONSTRUCTOR, 18, 19, 1),
            ("extension type A.n { }", c::MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS, 17, 18, 0),
        ] {
            let mut names = Interner::new();
            let out = parse(source, &mut names);
            assert_eq!(out.diagnostics.len(), 1, "{source}: {:?}", out.diagnostics);
            let d = &out.diagnostics[0];
            assert_eq!((d.code, d.span.start, d.span.end), (Some(code), start_offset, end_offset), "{source}");
            match &out.ast.decl(out.unit.declarations[0]).kind {
                DeclKind::ExtensionType(e) => assert_eq!(e.members.len(), members_, "{source}"),
                other => panic!("{source}: {other:?}"),
            }
        }
    }

    /// Attempt with a return type: `augment C.named() : ...` reads the type,
    /// flags `constructor_with_return_type` at it and reads the constructor
    /// with the redirect — without a cascade (probed on the local SDK 3.6.2:
    /// only the error at `augment` plus the semantic redirect one).
    #[test]
    fn member_speculative_recovery_discards_cascade() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_with;
        use dartforge_diagnostics::codes::parser as c;

        let source = "class C {\n  augment C.named() : this.missing();\n}\n";
        let mut names = Interner::new();
        let out = parse_with(source, &mut names, LibraryFeatures::new(LanguageVersion::FLOOR, &[]));
        let found_codes: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            found_codes,
            [(Some(c::CONSTRUCTOR_WITH_RETURN_TYPE), (12, 19))],
            "{source}: {:?}",
            out.diagnostics
        );
        assert_eq!(class(&out, 0).members.len(), 1, "{source}: {:?}", out.diagnostics);
    }

    /// A stray token skips one: `[` does not swallow the rest (`int? a]` stays
    /// a declaration with a missing `;` at the name, each stray closer is
    /// flagged and the final `;` is unexpected; the trailing `var b` survives).
    /// Sequence probed on the local SDK 3.6.2, byte for byte across the 5
    /// diagnostics.
    #[test]
    fn fasta_top_level_stray_token_skips_one() {
        use dartforge_diagnostics::codes::parser as c;

        let source = "[int? a]);\nvar b = 0;\n";
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        let found_codes: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            found_codes,
            [
                (Some(c::EXPECTED_EXECUTABLE), (0, 1)),
                (Some(c::EXPECTED_TOKEN), (6, 7)),
                (Some(c::EXPECTED_EXECUTABLE), (7, 8)),
                (Some(c::EXPECTED_EXECUTABLE), (8, 9)),
                (Some(c::UNEXPECTED_TOKEN), (9, 10)),
            ],
            "{source}: {:?}",
            out.diagnostics
        );
        // `int? a` becomes a field with the inserted `;`; `var b` comes after.
        assert_eq!(out.unit.declarations.len(), 2);
        assert!(matches!(decl(&out, 1), DeclKind::Variables(l) if l.variables.iter().any(|v| text(&names, v.name) == "b")));
    }

    /// `Foo() = Bar;` without `factory`: the target is read and only the `=` is
    /// flagged, without a cascade (probed on the local SDK 3.6.2).
    #[test]
    fn fasta_member_redirect_without_factory_reports_equals() {
        use dartforge_diagnostics::codes::parser as c;

        let source = "class Foo {\n  Foo()\n  = Bar\n  ;\n}\n";
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        let found_codes: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            found_codes,
            [(Some(c::REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR), (22, 23))],
            "{source}: {:?}",
            out.diagnostics
        );
    }

    /// `augment C(...) : field = e;` with the feature off reads the type as a
    /// constructor return type (`constructor_with_return_type` at it, probed on
    /// the local SDK 3.6.2) and the tail as inits — without a cascade.
    #[test]
    fn fasta_member_augment_constructor_reads_as_constructor() {
        use dartforge_diagnostics::codes::parser as c;

        let source = "class A {\n  int value;\n  augment A(int? p1) : value = p1;\n}\n";
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        let found_codes: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            found_codes,
            [(Some(c::CONSTRUCTOR_WITH_RETURN_TYPE), (25, 32))],
            "{source}: {:?}",
            out.diagnostics
        );
        assert_eq!(class(&out, 0).members.len(), 2, "{source}: {:?}", out.diagnostics);
    }

    /// `this :` in a class WITH a primary header is a body part even without
    /// the feature (the 3.6 oracle is silenced by the header's cascade, which we
    /// accept as a superset): no `expected_class_member`.
    #[test]
    fn fasta_member_this_in_primary_class_does_not_report() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_with;

        let source = "class A(int x) {\n  A.named() : this(0);\n  this : assert(x > 0);\n}\n";
        let mut names = Interner::new();
        let out = parse_with(source, &mut names, LibraryFeatures::new(LanguageVersion::FLOOR, &[]));
        assert!(
            !out.diagnostics.iter().any(|d| d.code == Some(
                dartforge_diagnostics::codes::parser::EXPECTED_CLASS_MEMBER
            )),
            "{source}: {:?}",
            out.diagnostics
        );
        assert_eq!(class(&out, 0).members.len(), 2, "{source}: {:?}", out.diagnostics);
    }

    /// `this => 0;` without the feature: the 3.13.4 analyzer reads the `this`
    /// part as with the feature and only adds `experiment_not_enabled` — at
    /// the header's `(` (`3.13.0`, from the parser) and at the `this` (`3.13`,
    /// from the `AstBuilder`) —, without `expected_class_member` (probed in a
    /// 3.6 library). `=> 0;` in a class only gives the no-name/no-parameters
    /// pair.
    #[test]
    fn member_this_arrow_without_feature_follows_3_13_4() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_with;
        use dartforge_diagnostics::codes::parser as c;

        let source = "class A(int x) {\n  this => 0;\n}\n";
        let mut names = Interner::new();
        let out = parse_with(source, &mut names, LibraryFeatures::new(LanguageVersion::FLOOR, &[]));
        let feature: Vec<_> = out
            .diagnostics
            .iter()
            .filter(|d| d.code == Some(c::EXPERIMENT_NOT_ENABLED))
            .map(|d| ((d.span.start, d.span.end), d.args[1].to_string()))
            .collect();
        assert_eq!(feature, [((7, 8), "3.13.0".to_string()), ((19, 23), "3.13".to_string())], "{:?}", out.diagnostics);
        assert!(!out.diagnostics.iter().any(|d| d.code == Some(c::EXPECTED_CLASS_MEMBER)), "{:?}", out.diagnostics);

        let source = "class C {\n  => 0;\n}\n";
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        let found_codes: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            found_codes,
            [
                (Some(c::MISSING_IDENTIFIER), (12, 14)),
                (Some(c::MISSING_METHOD_PARAMETERS), (12, 14)),
            ],
            "{source}: {:?}",
            out.diagnostics
        );
    }

    /// `new foo();` without the feature is the 3.13 constructor with
    /// `experiment_not_enabled` at the `new` (version `3.13`) and no other
    /// syntax error (probed on the 3.13.4 analyzer, 3.6 library).
    #[test]
    fn member_new_without_feature_is_constructor_with_feature_off() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_with;
        use dartforge_diagnostics::codes::parser as c;

        let source = "class C {\n  new foo();\n  static int foo = 0;\n}\n";
        let mut names = Interner::new();
        let out = parse_with(source, &mut names, LibraryFeatures::new(LanguageVersion::FLOOR, &[]));
        let found_codes: Vec<_> = out
            .diagnostics
            .iter()
            .map(|d| (d.code, (d.span.start, d.span.end), d.args.iter().map(|a| a.to_string()).collect::<Vec<_>>()))
            .collect();
        assert_eq!(
            found_codes,
            [(Some(c::EXPERIMENT_NOT_ENABLED), (12, 15), vec!["primary-constructors".to_string(), "3.13".to_string(), "3.13".to_string()])],
            "{:?}",
            out.diagnostics
        );
        assert_eq!(class(&out, 0).members.len(), 2);
    }

    /// A private named parameter without a public name (`{this._123}`) and
    /// without the feature: the 3.13.4 analyzer only reports
    /// `experiment_not_enabled` at the name (recorded oracle, 3.6 library); the
    /// missing public name is only an error with the feature on.
    #[test]
    fn private_named_without_feature_only_reports_the_feature() {
        use crate::features::{LanguageVersion, LibraryFeatures};
        use crate::parser::parse_with;
        use dartforge_diagnostics::codes::parser as c;

        let source = "class C {\n  int? _123;\n  C({this._123});\n}\n";
        let start_offset = source.rfind("_123").unwrap();
        let mut names = Interner::new();
        let out = parse_with(source, &mut names, LibraryFeatures::new(LanguageVersion::FLOOR, &[]));
        let found_codes: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(found_codes, [(Some(c::EXPERIMENT_NOT_ENABLED), (start_offset, start_offset + 4))], "{:?}", out.diagnostics);
    }

    /// `T X.Y` without parentheses: both errors (`int C.named;`, probed).
    #[test]
    fn fasta_member_type_and_method_without_parameters() {
        use dartforge_diagnostics::codes::parser as c;

        let source = "class C {\n  int C.named;\n}\n";
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        let found_codes: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            found_codes,
            [
                (Some(c::CONSTRUCTOR_WITH_RETURN_TYPE), (12, 15)),
                (Some(c::MISSING_METHOD_PARAMETERS), (16, 17)),
            ],
            "{source}: {:?}",
            out.diagnostics
        );
    }

    /// `X.Y` / `X.new` without `(` in a member: `missing_method_parameters` at X
    /// (probed on the local SDK 3.6.2: `C.named;`, `foo.bar = 1;`,
    /// `C.named : x = 1;`, `C.new;`).
    #[test]
    fn fasta_member_method_without_parameters() {
        use dartforge_diagnostics::codes::parser as c;

        for (member, start_offset, end_offset) in [
            ("C.named;", 12, 13),
            ("foo.bar = 1;", 12, 15),
            ("C.new;", 12, 13),
        ] {
            let source = format!("class C {{\n  {member}\n}}\n");
            let mut names = Interner::new();
            let out = parse(&source, &mut names);
            let first = &out.diagnostics[0];
            assert_eq!(first.code, Some(c::MISSING_METHOD_PARAMETERS), "{source}: {:?}", out.diagnostics);
            assert_eq!((first.span.start, first.span.end), (start_offset, end_offset), "{source}");
        }
    }

    /// `foo;`, `foo = 1;`, `foo, bar;` in a member: the name is an untyped
    /// field — `missing_const_final_var_or_type` at it (except with
    /// `const`/`final`/`var`), probed on the local SDK 3.6.2. Also `static = 1;`
    /// and `C;`.
    #[test]
    fn fasta_member_untyped_field_reports_at_name() {
        use dartforge_diagnostics::codes::parser as c;

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
            let source = format!("class C {{\n  {member}\n}}\n");
            let mut names = Interner::new();
            let out = parse(&source, &mut names);
            assert_eq!(out.diagnostics.len(), errors, "{source}: {:?}", out.diagnostics);
            if errors == 1 {
                let single = &out.diagnostics[0];
                assert_eq!(single.code, Some(c::MISSING_CONST_FINAL_VAR_OR_TYPE), "{source}");
                assert_eq!((single.span.start, single.span.end), (start_offset, end_offset), "{source}");
            }
        }
    }
    /// `int? ab]` in a member stays a declaration: `;` missing at the name and
    /// the `]` is `expected_class_member` at it (probed on the local SDK 3.6.2).
    #[test]
    fn fasta_member_nullable_type_with_closer_continues_declaration() {
        use dartforge_diagnostics::codes::parser as c;

        let source = "class C {\n  int? ab]\n}\n";
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        let found_codes: Vec<_> = out.diagnostics.iter().map(|d| (d.code, (d.span.start, d.span.end))).collect();
        assert_eq!(
            found_codes,
            [
                (Some(c::EXPECTED_TOKEN), (17, 19)),
                (Some(c::EXPECTED_CLASS_MEMBER), (19, 20)),
            ],
            "{source}: {:?}",
            out.diagnostics
        );
        assert_eq!(out.unit.declarations.len(), 1, "{source}");
        // The inserted `;` closes the field `int? ab` (`ensureSemicolon`).
        assert_eq!(class(&out, 0).members.len(), 1, "{source}: {:?}", out.diagnostics);
    }

    /// `(` without a type or modifiers does not open a declaration at top
    /// level: it is `expected_executable`, not `missing_identifier`.
    #[test]
    fn fasta_top_level_stray_paren_is_not_identifier() {
        use dartforge_diagnostics::codes::parser as c;

        let source = "(42);\nvar b = 0;\n";
        let mut names = Interner::new();
        let out = parse(source, &mut names);
        let first = &out.diagnostics[0];
        assert_eq!(first.code, Some(c::EXPECTED_EXECUTABLE));
        assert_eq!((first.span.start, first.span.end), (0, 1));
        assert!(
            !out.diagnostics.iter().any(|d| d.code == Some(c::MISSING_IDENTIFIER)),
            "{source}: {:?}",
            out.diagnostics
        );
        let last = out.unit.declarations.len() - 1;
        assert!(matches!(decl(&out, last), DeclKind::Variables(_)));
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
            assert!(!p.in_async && !p.in_generator, "context restored: {src}");
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
        // fasta's `ensureBlock`: a missing body becomes a synthetic empty block,
        // without consuming the `)`.
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

    // -- Depending on types/expressions/statements -----------------------------
    //
    // These tests panic with `not yet implemented` while the other modules are
    // stubs; they pass once those exist.
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

        /// `operator` is a built-in identifier: it can be a field name.
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

    /// `augment`, `macro class`, `import augment` and `augment library`
    /// (docs/AUGMENTATIONS.md, `augmentations`/`macros` experiments).
    mod augmentations {
        use super::*;
        use crate::features::{Feature, LanguageVersion, LibraryFeatures};
        use crate::parser::parse_with;

        fn with_features(exp: &[Feature], src: &str, names: &mut Interner) -> Parsed {
            parse_with(src, names, LibraryFeatures::new(LanguageVersion::FLOOR, exp))
        }

        #[test]
        fn augment_on_top_level_and_members() {
            let mut names = Interner::new();
            let src = "augment class C { augment void f() {} int g() => 1; augment C.x() : y = 1; augment int get z => 2; }
                       augment String saudacao(String n) => n;";
            let out = with_features(&[Feature::Macros], src, &mut names);
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
            // Without `augmentations`/`macros`, `augment` stays an identifier: 3.6
            // code that uses it as a name does not change meaning.
            let mut names = Interner::new();
            let out = with_features(&[], "var augment = 1; int f() => augment; void augment2() {}", &mut names);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            assert!(!out.ast.decl(out.unit.declarations[0]).augment);
            // Not even `augment class` is a modifier: fasta tokenizes `augment` as an
            // identifier and top-level recovery reports the 3.13.4 analyzer's pair at
            // the `augment` (probed), without a disabled feature.
            let out = with_features(&[], "augment class C {}", &mut names);
            let mut found_codes: Vec<_> = out.diagnostics.iter().map(|d| (d.code, d.span.start, d.span.end)).collect();
            found_codes.sort();
            use dartforge_diagnostics::codes::parser as c;
            let mut expected_ = [(Some(c::EXPECTED_TOKEN), 0, 7), (Some(c::MISSING_CONST_FINAL_VAR_OR_TYPE), 0, 7)];
            expected_.sort();
            assert_eq!(
                found_codes,
                expected_,
                "{:?}",
                out.diagnostics
            );
            assert!(out.unit.declarations.iter().all(|&d| !out.ast.decl(d).augment));
        }

        /// Without the experiment, `augment` before a declaration is a plain name
        /// and falls into top-level recovery. Syntactic codes and ranges of the
        /// 3.13.4 analyzer in a 3.6 package (probed; `undefined_class` and
        /// `unused_element` are semantic and left out).
        #[test]
        fn augment_without_the_feature_follows_fasta_recovery() {
            use dartforge_diagnostics::codes::parser as c;
            let cases_: [(&str, &[(dartforge_diagnostics::Code, usize, usize)]); 5] = [
                ("augment class C {}", &[(c::EXPECTED_TOKEN, 0, 7), (c::MISSING_CONST_FINAL_VAR_OR_TYPE, 0, 7)]),
                ("augment enum E { a }", &[(c::EXPECTED_TOKEN, 0, 7), (c::MISSING_CONST_FINAL_VAR_OR_TYPE, 0, 7)]),
                ("augment mixin M {}", &[(c::EXPECTED_TOKEN, 0, 7), (c::MISSING_IDENTIFIER, 8, 13)]),
                ("augment extension X on int {}", &[(c::EXPECTED_TOKEN, 0, 7), (c::MISSING_IDENTIFIER, 8, 17)]),
                ("augment abstract class D {}", &[(c::EXPECTED_TOKEN, 8, 16)]),
            ];
            for (source, expected_) in cases_ {
                let mut names = Interner::new();
                let out = with_features(&[], source, &mut names);
                let mut found: Vec<_> = out.diagnostics.iter().map(|d| (d.code.unwrap(), d.span.start, d.span.end)).collect();
                let mut expected_ = expected_.to_vec();
                found.sort_by_key(|&(c, s, e)| (s, e, c));
                expected_.sort_by_key(|&(c, s, e)| (s, e, c));
                assert_eq!(found, expected_, "{source}: {:?}", out.diagnostics);
            }
        }

        #[test]
        fn macro_class_and_directives_in_36_form() {
            let mut names = Interner::new();
            let src = "import augment 'a_aug.dart';
macro class M implements Macro { const M(); }";
            let out = with_features(&[Feature::Macros], src, &mut names);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            assert!(matches!(out.unit.directives[0].kind, DirectiveKind::ImportAugment { .. }));
            assert!(class(&out, 0).modifiers.macro_);
            let out = with_features(&[Feature::Macros], "augment library 'main.dart';
import 'dart:core' as p;", &mut names);
            assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
            assert!(matches!(out.unit.directives[0].kind, DirectiveKind::AugmentLibrary { .. }));
            // Without `macros`: a diagnostic, not a syntax error.
            let out = with_features(&[], "macro class M {}", &mut names);
            assert_eq!(out.diagnostics.len(), 1, "{:?}", out.diagnostics);
            assert!(class(&out, 0).modifiers.macro_);
        }

        #[test]
        fn text_generated_by_cfe_for_json_codable() {
            // The augmentation the 3.6.2 CFE generates for `@JsonCodable`.
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
            let out = with_features(&[Feature::Macros], src, &mut names);
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
