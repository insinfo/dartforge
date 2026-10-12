//! Modificadores de declaração lidos e denunciados como no fasta.
//!
//! Porte de `modifier_context.dart` (`_fe_analyzer_shared` 76.0.0) e do
//! caminho rápido que o precede em `parser_impl.dart`
//! (`parseTopLevelMemberImpl`, `parseClassOrMixinOrExtensionOrEnumMemberImpl`):
//! o fasta lê sem erro os modificadores na ordem canônica e, quando sobra
//! algum, passa o resto ao `ModifierContext`, que relata repetição
//! (`DUPLICATED_MODIFIER`), ordem errada (`MODIFIER_OUT_OF_ORDER`), conflito
//! (`CONFLICTING_MODIFIERS`, `CONST_AND_FINAL`, `FINAL_AND_VAR`,
//! `COVARIANT_AND_STATIC`) e, por contexto, o modificador que não cabe ali
//! (`EXTRANEOUS_MODIFIER`).
//!
//! A árvore não muda: os modificadores continuam aceitos em qualquer ordem
//! (superconjunto); aqui só se acrescentam os diagnósticos, e só quando há
//! erro — código válido nunca chega aos ramos que relatam.

use super::Parser;
use crate::token::{Keyword, Kind, Op};
use dartforge_diagnostics::{Codigo, Span, codigos};

/// Posição absoluta do token de cada modificador lido (os campos do
/// `ModifierContext`).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ModifierTokens {
    pub(crate) abstract_: Option<usize>,
    pub(crate) const_: Option<usize>,
    pub(crate) covariant: Option<usize>,
    pub(crate) external: Option<usize>,
    pub(crate) final_: Option<usize>,
    pub(crate) late: Option<usize>,
    pub(crate) required: Option<usize>,
    pub(crate) static_: Option<usize>,
    pub(crate) var_: Option<usize>,
}

impl ModifierTokens {
    /// `varFinalOrConst`.
    pub(crate) fn var_final_or_const(&self) -> Option<usize> {
        self.var_.or(self.final_).or(self.const_)
    }
}

/// O `MemberKind` do fasta que importa ao `parseFormalParameterModifiers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParameterOwner {
    /// Função de topo ou método estático (`covariant` não cabe).
    TopLevelOrStatic,
    /// Método de extension (`EXTRANEOUS_MODIFIER_IN_EXTENSION`).
    Extension,
    /// Método de extension type.
    ExtensionType,
    /// Construtor primário.
    PrimaryConstructor,
    /// Tipo de função genérico (`Function(...)`): `var`/`final` viram
    /// `FUNCTION_TYPED_PARAMETER_VAR`.
    FunctionTypeKind,
    /// `typedef` da forma antiga (`MemberKind.FunctionTypeAlias`).
    TypeAlias,
    /// Parâmetros de um parâmetro-função da forma antiga
    /// (`MemberKind.FunctionTypedParameter`).
    FunctionParameter,
    /// Os demais (método de instância, construtor, função local, `catch`…).
    Other,
}

impl<'s, 'i> Parser<'s, 'i> {
    /// `isModifier` (`modifier_context.dart:12`): `const`/`final`/`var` sempre;
    /// os identificadores embutidos (`abstract`, `covariant`, `external`,
    /// `late`, `required`, `static`) só quando seguidos de palavra (que não
    /// `in`) ou de um tipo record seguido de nome (ou de `this.`/`super.`).
    /// `augment` fica de fora: é lido antes, por `parse_augment_opt`.
    pub(crate) fn is_fasta_modifier(&self, pos: usize) -> bool {
        match self.kind_of(pos) {
            Kind::Keyword(Keyword::Const | Keyword::Final | Keyword::Var) => return true,
            Kind::Ident => {}
            _ => return false,
        }
        if !matches!(
            self.text_of(pos),
            "abstract" | "covariant" | "external" | "late" | "required" | "static"
        ) {
            return false;
        }
        let following = self.kind_of(pos + 1);
        let word = matches!(following, Kind::Ident | Kind::Keyword(_));
        if !word || following == Kind::Keyword(Keyword::In) {
            if following == Kind::Op(Op::LParen)
                && let Some(closes) = self.matching_close(pos + 1)
            {
                let mut after = closes + 1;
                if self.kind_of(after) == Kind::Op(Op::Question) {
                    after += 1;
                }
                return self.kind_of(after) == Kind::Ident || self.this_or_super_with_dot(after);
            }
            return false;
        }
        true
    }

    /// `_thisOrSuperWithDot`.
    fn this_or_super_with_dot(&self, pos: usize) -> bool {
        matches!(self.kind_of(pos), Kind::Keyword(Keyword::This | Keyword::Super))
            && self.kind_of(pos + 1) == Kind::Op(Op::Dot)
    }

    pub(crate) fn span_of(&self, pos: usize) -> Span {
        self.tokens[pos.min(self.tokens.len() - 1)].span
    }

    /// Erro sem argumentos no token em `pos`.
    pub(crate) fn error_at_token(&mut self, pos: usize, code: Codigo) {
        let span = self.span_of(pos);
        self.error_at(code, span, &[]);
    }

    /// `reportExtraneousModifier`: `EXTRANEOUS_MODIFIER` com o texto do token.
    pub(crate) fn stray_modifier(&mut self, pos: Option<usize>) {
        if let Some(pos) = pos {
            let texto = self.text_of(pos).to_string();
            let span = self.span_of(pos);
            self.error_at(codigos::parser::EXTRANEOUS_MODIFIER, span, &[&texto]);
        }
    }

    fn duplicate_modifier(&mut self, pos: usize) {
        let texto = self.text_of(pos).to_string();
        let span = self.span_of(pos);
        self.error_at(codigos::parser::DUPLICATED_MODIFIER, span, &[&texto]);
    }

    /// `reportModifierOutOfOrder`: `pos` devia vir antes de `antes`.
    fn out_of_order_modifier(&mut self, pos: usize, antes: &str) {
        let texto = self.text_of(pos).to_string();
        let span = self.span_of(pos);
        self.error_at(codigos::parser::MODIFIER_OUT_OF_ORDER, span, &[&texto, antes]);
    }

    /// `reportConflictingModifiers`.
    fn conflicting_modifiers(&mut self, pos: usize, anterior: usize) {
        let texto = self.text_of(pos).to_string();
        let antes = self.text_of(anterior).to_string();
        let span = self.span_of(pos);
        self.error_at(codigos::parser::CONFLICTING_MODIFIERS, span, &[&texto, &antes]);
    }

    /// `ModifierContext._parseModifiers`: consome todos os modificadores a
    /// partir do cursor, relatando os que repetem, conflitam ou estão fora
    /// de ordem em relação aos já lidos em `f`. `after_factory` é o
    /// `_afterFactory` (modificadores depois de `factory`).
    pub(crate) fn modifier_context(&mut self, f: &mut ModifierTokens, after_factory: bool) {
        loop {
            let pos = self.pos;
            if self.is_fasta_modifier(pos) {
                match self.text_of(pos) {
                    "abstract" => self.mod_abstract(f, pos),
                    "const" => self.mod_const(f, pos, after_factory),
                    "covariant" => self.mod_covariant(f, pos, after_factory),
                    "external" => self.mod_external(f, pos, after_factory),
                    "final" => self.mod_final(f, pos, after_factory),
                    "late" => self.mod_late(f, pos),
                    "required" => self.required_mod(f, pos),
                    "static" => self.mod_static(f, pos, after_factory),
                    _ => self.var_mod(f, pos, after_factory),
                }
            } else if after_factory && self.at_ident("factory") {
                self.duplicate_modifier(pos);
            } else {
                break;
            }
            self.advance();
        }
    }

    fn mod_abstract(&mut self, f: &mut ModifierTokens, pos: usize) {
        if f.abstract_.is_none() {
            f.abstract_ = Some(pos);
            if let Some(v) = f.var_final_or_const() {
                let t = self.text_of(v);
                self.out_of_order_modifier(pos, t);
            } else if let Some(c) = f.covariant {
                let t = self.text_of(c);
                self.out_of_order_modifier(pos, t);
            }
            return;
        }
        self.duplicate_modifier(pos);
    }

    fn mod_const(&mut self, f: &mut ModifierTokens, pos: usize, after_factory: bool) {
        if f.var_final_or_const().is_none() && f.covariant.is_none() {
            f.const_ = Some(pos);
            if after_factory {
                self.out_of_order_modifier(pos, "factory");
            } else if let Some(l) = f.late {
                self.conflicting_modifiers(pos, l);
            }
            return;
        }
        if f.const_.is_some() {
            self.duplicate_modifier(pos);
        } else if let Some(c) = f.covariant {
            self.conflicting_modifiers(pos, c);
        } else if f.final_.is_some() {
            self.error_at_token(pos, codigos::parser::CONST_AND_FINAL);
        } else if let Some(v) = f.var_ {
            self.conflicting_modifiers(pos, v);
        }
    }

    fn mod_covariant(&mut self, f: &mut ModifierTokens, pos: usize, after_factory: bool) {
        if f.const_.is_none() && f.covariant.is_none() && f.static_.is_none() && !after_factory {
            f.covariant = Some(pos);
            if let Some(v) = f.var_ {
                let t = self.text_of(v);
                self.out_of_order_modifier(pos, t);
            } else if let Some(v) = f.final_ {
                let t = self.text_of(v);
                self.out_of_order_modifier(pos, t);
            } else if let Some(l) = f.late {
                let t = self.text_of(l);
                self.out_of_order_modifier(pos, t);
            }
            return;
        }
        if f.covariant.is_some() {
            self.duplicate_modifier(pos);
        } else if after_factory {
            self.stray_modifier(Some(pos));
        } else if let Some(c) = f.const_ {
            self.conflicting_modifiers(pos, c);
        } else if f.static_.is_some() {
            self.error_at_token(pos, codigos::parser::COVARIANT_AND_STATIC);
        }
    }

    fn mod_external(&mut self, f: &mut ModifierTokens, pos: usize, after_factory: bool) {
        if f.external.is_none() {
            f.external = Some(pos);
            let antes = if after_factory {
                Some("factory".to_string())
            } else {
                [f.const_, f.static_, f.late, f.var_final_or_const(), f.covariant]
                    .into_iter()
                    .flatten()
                    .next()
                    .map(|p| self.text_of(p).to_string())
            };
            if let Some(antes) = antes {
                self.out_of_order_modifier(pos, &antes);
            }
            return;
        }
        self.duplicate_modifier(pos);
    }

    fn mod_final(&mut self, f: &mut ModifierTokens, pos: usize, after_factory: bool) {
        if f.var_final_or_const().is_none() && !after_factory {
            f.final_ = Some(pos);
            return;
        }
        if f.final_.is_some() {
            self.duplicate_modifier(pos);
        } else if after_factory {
            self.stray_modifier(Some(pos));
        } else if f.const_.is_some() {
            self.error_at_token(pos, codigos::parser::CONST_AND_FINAL);
        } else if f.var_.is_some() {
            self.error_at_token(pos, codigos::parser::FINAL_AND_VAR);
        } else if let Some(l) = f.late {
            let t = self.text_of(l);
            self.out_of_order_modifier(pos, t);
        }
    }

    fn mod_late(&mut self, f: &mut ModifierTokens, pos: usize) {
        if f.late.is_none() {
            f.late = Some(pos);
            if let Some(c) = f.const_ {
                self.conflicting_modifiers(pos, c);
            } else if let Some(v) = f.var_ {
                let t = self.text_of(v);
                self.out_of_order_modifier(pos, t);
            } else if let Some(v) = f.final_ {
                let t = self.text_of(v);
                self.out_of_order_modifier(pos, t);
            }
            return;
        }
        self.duplicate_modifier(pos);
    }

    fn required_mod(&mut self, f: &mut ModifierTokens, pos: usize) {
        if f.required.is_none() {
            f.required = Some(pos);
            if let Some(p) = f.const_.or(f.covariant).or(f.final_).or(f.var_) {
                let t = self.text_of(p);
                self.out_of_order_modifier(pos, t);
            }
            return;
        }
        self.duplicate_modifier(pos);
    }

    fn mod_static(&mut self, f: &mut ModifierTokens, pos: usize, after_factory: bool) {
        if f.covariant.is_none() && f.static_.is_none() && !after_factory {
            f.static_ = Some(pos);
            if let Some(p) = f.const_.or(f.final_).or(f.var_).or(f.late) {
                let t = self.text_of(p);
                self.out_of_order_modifier(pos, t);
            }
            return;
        }
        if f.covariant.is_some() {
            self.error_at_token(pos, codigos::parser::COVARIANT_AND_STATIC);
        } else if f.static_.is_some() {
            self.duplicate_modifier(pos);
        } else if after_factory {
            self.stray_modifier(Some(pos));
        }
    }

    fn var_mod(&mut self, f: &mut ModifierTokens, pos: usize, after_factory: bool) {
        if f.var_final_or_const().is_none() && !after_factory {
            f.var_ = Some(pos);
            return;
        }
        if f.var_.is_some() {
            self.duplicate_modifier(pos);
        } else if after_factory {
            self.stray_modifier(Some(pos));
        } else if let Some(c) = f.const_ {
            self.conflicting_modifiers(pos, c);
        } else if f.final_.is_some() {
            self.error_at_token(pos, codigos::parser::FINAL_AND_VAR);
        }
    }

    /// Consome `var`/`final`/`const` ou `late final?` no cursor (o último
    /// degrau do caminho rápido do fasta). `const` só sem `covariant`.
    fn vfc_fast_path(&mut self, f: &mut ModifierTokens) {
        if !self.is_fasta_modifier(self.pos) {
            return;
        }
        let pos = self.pos;
        match self.text_of(pos) {
            "final" => f.final_ = Some(pos),
            "var" => f.var_ = Some(pos),
            "const" if f.covariant.is_none() => f.const_ = Some(pos),
            "late" => {
                f.late = Some(pos);
                self.advance();
                if self.is_fasta_modifier(self.pos) && self.at_kw(Keyword::Final) {
                    f.final_ = Some(self.pos);
                    self.advance();
                }
                return;
            }
            _ => return,
        }
        self.advance();
    }

    /// Modificadores de membro de topo (`parseTopLevelMemberImpl`): caminho
    /// rápido `external`, depois `var`/`final`/`const` ou `late final?`; o
    /// que sobrar vai ao `ModifierContext` com
    /// `parseTopLevelMemberModifiers`.
    pub(crate) fn read_top_level_modifiers(&mut self) -> ModifierTokens {
        let mut f = ModifierTokens::default();
        if self.is_fasta_modifier(self.pos) && self.at_ident("external") {
            f.external = Some(self.pos);
            self.advance();
        }
        self.vfc_fast_path(&mut f);
        // Outro `var`/`final`/`const` depois de um deles não é lido: começa
        // a declaração seguinte (`final final class C {}`).
        if f.var_final_or_const().is_some()
            && matches!(self.kind(), Kind::Keyword(Keyword::Final | Keyword::Var | Keyword::Const))
        {
            return f;
        }
        if self.is_fasta_modifier(self.pos) {
            self.modifier_context(&mut f, false);
            self.stray_modifier(f.abstract_);
            self.stray_modifier(f.covariant);
            self.stray_modifier(f.required);
            self.stray_modifier(f.static_);
        }
        f
    }

    /// Modificadores de membro de classe, mixin, enum, extension ou
    /// extension type (`parseClassOrMixinOrExtensionOrEnumMemberImpl`):
    /// caminho rápido `external`/`abstract`, depois `static`/`covariant`,
    /// depois `var`/`final`/`const` ou `late final?`; o que sobrar vai ao
    /// `ModifierContext` com `parseClassMemberModifiers`.
    pub(crate) fn read_member_modifiers(&mut self) -> ModifierTokens {
        let mut f = ModifierTokens::default();
        if self.is_fasta_modifier(self.pos) {
            if self.at_ident("external") {
                f.external = Some(self.pos);
                self.advance();
            } else if self.at_ident("abstract") {
                f.abstract_ = Some(self.pos);
                self.advance();
            }
        }
        if self.is_fasta_modifier(self.pos) {
            if self.at_ident("static") {
                f.static_ = Some(self.pos);
                self.advance();
            } else if self.at_ident("covariant") {
                f.covariant = Some(self.pos);
                self.advance();
            }
        }
        self.vfc_fast_path(&mut f);
        if self.is_fasta_modifier(self.pos) {
            self.modifier_context(&mut f, false);
            self.stray_modifier(f.required);
        }
        f
    }

    /// `parseFormalParameterModifiers` (`modifier_context.dart:213`), depois
    /// de o `ModifierContext` ler os modificadores de um parâmetro.
    pub(crate) fn report_parameter_modifiers(
        &mut self,
        f: &ModifierTokens,
        optional_named: bool,
        owner: ParameterOwner,
    ) {
        if !optional_named {
            self.stray_modifier(f.required);
        }
        match owner {
            ParameterOwner::TopLevelOrStatic => self.stray_modifier(f.covariant),
            ParameterOwner::Extension => {
                if let Some(c) = f.covariant {
                    let span = self.span_of(c);
                    self.error_at(codigos::parser::INVALID_USE_OF_COVARIANT_IN_EXTENSION, span, &["covariant"]);
                }
            }
            ParameterOwner::ExtensionType => {
                if let Some(c) = f.covariant {
                    let span = self.span_of(c);
                    self.error_at(codigos::parser::EXTRANEOUS_MODIFIER_IN_EXTENSION_TYPE, span, &["covariant"]);
                }
            }
            // `modifier_context.dart:234-249` (checkout main): sem o recurso,
            // todo `covariant`; com ele, o que não acompanha `var`.
            ParameterOwner::PrimaryConstructor => {
                if let Some(c) = f.covariant
                    && (!self.features.tem(crate::Feature::PrimaryConstructors) || f.var_.is_none())
                {
                    let span = self.span_of(c);
                    self.error_at(codigos::parser::EXTRANEOUS_MODIFIER_IN_PRIMARY_CONSTRUCTOR, span, &["covariant"]);
                }
            }
            ParameterOwner::FunctionTypeKind
            | ParameterOwner::TypeAlias
            | ParameterOwner::FunctionParameter
            | ParameterOwner::Other => {}
        }
        if f.const_.is_some() {
            self.stray_modifier(f.const_);
        } else if owner == ParameterOwner::FunctionTypeKind
            && let Some(v) = f.var_final_or_const()
        {
            self.error_at_token(v, codigos::parser::FUNCTION_TYPED_PARAMETER_VAR);
        }
        self.stray_modifier(f.abstract_);
        self.stray_modifier(f.external);
        self.stray_modifier(f.late);
        self.stray_modifier(f.static_);
    }

    /// `reportTopLevelModifierError` + os `reportExtraneousModifier` de
    /// `parseClassModifiers`/`parseEnumModifiers`/`parseMixinModifiers`/
    /// `parseExtensionModifiers`/`parseTypedefModifiers`/
    /// `parseTopLevelKeywordModifiers` para os modificadores de membro que
    /// precedem uma palavra de topo (`static class`, `const enum`…). `word`
    /// é o texto da palavra de topo; `abstract` e `final` só valem em
    /// `class` (e `final` também em `enum`/`mixin`/`extension`).
    pub(crate) fn report_modifiers_before_top_level(&mut self, f: &ModifierTokens, word: &str) {
        let class = word == "class";
        let directive_or_typedef = matches!(word, "typedef" | "library" | "import" | "export" | "part");
        if let Some(c) = f.const_ {
            if class {
                self.error_at_token(c, codigos::parser::CONST_CLASS);
            } else {
                self.stray_modifier(Some(c));
            }
        }
        if let Some(e) = f.external {
            match word {
                "class" => self.error_at_token(e, codigos::parser::EXTERNAL_CLASS),
                "enum" => self.error_at_token(e, codigos::parser::EXTERNAL_ENUM),
                "typedef" => self.error_at_token(e, codigos::parser::EXTERNAL_TYPEDEF),
                _ => self.stray_modifier(Some(e)),
            }
        }
        if !class {
            self.stray_modifier(f.abstract_);
        }
        self.stray_modifier(f.covariant);
        if directive_or_typedef {
            self.stray_modifier(f.final_);
        }
        self.stray_modifier(f.late);
        self.stray_modifier(f.required);
        self.stray_modifier(f.static_);
        self.stray_modifier(f.var_);
    }
}
