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
pub(crate) struct Fichas {
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

impl Fichas {
    /// `varFinalOrConst`.
    pub(crate) fn var_final_ou_const(&self) -> Option<usize> {
        self.var_.or(self.final_).or(self.const_)
    }
}

/// O `MemberKind` do fasta que importa ao `parseFormalParameterModifiers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DonoDeParametros {
    /// Função de topo ou método estático (`covariant` não cabe).
    TopoOuEstatico,
    /// Método de extension (`EXTRANEOUS_MODIFIER_IN_EXTENSION`).
    Extension,
    /// Método de extension type.
    ExtensionType,
    /// Construtor primário.
    ConstrutorPrimario,
    /// Tipo de função genérico (`Function(...)`): `var`/`final` viram
    /// `FUNCTION_TYPED_PARAMETER_VAR`.
    TipoDeFuncao,
    /// `typedef` da forma antiga (`MemberKind.FunctionTypeAlias`).
    AliasDeTipo,
    /// Parâmetros de um parâmetro-função da forma antiga
    /// (`MemberKind.FunctionTypedParameter`).
    ParametroFuncao,
    /// Os demais (método de instância, construtor, função local, `catch`…).
    Outro,
}

impl<'s, 'i> Parser<'s, 'i> {
    /// `isModifier` (`modifier_context.dart:12`): `const`/`final`/`var` sempre;
    /// os identificadores embutidos (`abstract`, `covariant`, `external`,
    /// `late`, `required`, `static`) só quando seguidos de palavra (que não
    /// `in`) ou de um tipo record seguido de nome (ou de `this.`/`super.`).
    /// `augment` fica de fora: é lido antes, por `parse_augment_opt`.
    pub(crate) fn e_modificador_fasta(&self, pos: usize) -> bool {
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
        let seguinte = self.kind_of(pos + 1);
        let palavra = matches!(seguinte, Kind::Ident | Kind::Keyword(_));
        if !palavra || seguinte == Kind::Keyword(Keyword::In) {
            if seguinte == Kind::Op(Op::LParen)
                && let Some(fecha) = self.matching_close(pos + 1)
            {
                let mut depois = fecha + 1;
                if self.kind_of(depois) == Kind::Op(Op::Question) {
                    depois += 1;
                }
                return self.kind_of(depois) == Kind::Ident || self.this_ou_super_com_ponto(depois);
            }
            return false;
        }
        true
    }

    /// `_thisOrSuperWithDot`.
    fn this_ou_super_com_ponto(&self, pos: usize) -> bool {
        matches!(self.kind_of(pos), Kind::Keyword(Keyword::This | Keyword::Super))
            && self.kind_of(pos + 1) == Kind::Op(Op::Dot)
    }

    pub(crate) fn span_de(&self, pos: usize) -> Span {
        self.tokens[pos.min(self.tokens.len() - 1)].span
    }

    /// Erro sem argumentos no token em `pos`.
    pub(crate) fn erro_no_token(&mut self, pos: usize, codigo: Codigo) {
        let span = self.span_de(pos);
        self.erro_em(codigo, span, &[]);
    }

    /// `reportExtraneousModifier`: `EXTRANEOUS_MODIFIER` com o texto do token.
    pub(crate) fn modificador_estranho(&mut self, pos: Option<usize>) {
        if let Some(pos) = pos {
            let texto = self.text_of(pos).to_string();
            let span = self.span_de(pos);
            self.erro_em(codigos::parser::EXTRANEOUS_MODIFIER, span, &[&texto]);
        }
    }

    fn modificador_duplicado(&mut self, pos: usize) {
        let texto = self.text_of(pos).to_string();
        let span = self.span_de(pos);
        self.erro_em(codigos::parser::DUPLICATED_MODIFIER, span, &[&texto]);
    }

    /// `reportModifierOutOfOrder`: `pos` devia vir antes de `antes`.
    fn modificador_fora_de_ordem(&mut self, pos: usize, antes: &str) {
        let texto = self.text_of(pos).to_string();
        let span = self.span_de(pos);
        self.erro_em(codigos::parser::MODIFIER_OUT_OF_ORDER, span, &[&texto, antes]);
    }

    /// `reportConflictingModifiers`.
    fn modificadores_conflitantes(&mut self, pos: usize, anterior: usize) {
        let texto = self.text_of(pos).to_string();
        let antes = self.text_of(anterior).to_string();
        let span = self.span_de(pos);
        self.erro_em(codigos::parser::CONFLICTING_MODIFIERS, span, &[&texto, &antes]);
    }

    /// `ModifierContext._parseModifiers`: consome todos os modificadores a
    /// partir do cursor, relatando os que repetem, conflitam ou estão fora
    /// de ordem em relação aos já lidos em `f`. `depois_de_factory` é o
    /// `_afterFactory` (modificadores depois de `factory`).
    pub(crate) fn contexto_de_modificadores(&mut self, f: &mut Fichas, depois_de_factory: bool) {
        loop {
            let pos = self.pos;
            if self.e_modificador_fasta(pos) {
                match self.text_of(pos) {
                    "abstract" => self.mod_abstract(f, pos),
                    "const" => self.mod_const(f, pos, depois_de_factory),
                    "covariant" => self.mod_covariant(f, pos, depois_de_factory),
                    "external" => self.mod_external(f, pos, depois_de_factory),
                    "final" => self.mod_final(f, pos, depois_de_factory),
                    "late" => self.mod_late(f, pos),
                    "required" => self.mod_required(f, pos),
                    "static" => self.mod_static(f, pos, depois_de_factory),
                    _ => self.mod_var(f, pos, depois_de_factory),
                }
            } else if depois_de_factory && self.at_ident("factory") {
                self.modificador_duplicado(pos);
            } else {
                break;
            }
            self.advance();
        }
    }

    fn mod_abstract(&mut self, f: &mut Fichas, pos: usize) {
        if f.abstract_.is_none() {
            f.abstract_ = Some(pos);
            if let Some(v) = f.var_final_ou_const() {
                let t = self.text_of(v);
                self.modificador_fora_de_ordem(pos, t);
            } else if let Some(c) = f.covariant {
                let t = self.text_of(c);
                self.modificador_fora_de_ordem(pos, t);
            }
            return;
        }
        self.modificador_duplicado(pos);
    }

    fn mod_const(&mut self, f: &mut Fichas, pos: usize, depois_de_factory: bool) {
        if f.var_final_ou_const().is_none() && f.covariant.is_none() {
            f.const_ = Some(pos);
            if depois_de_factory {
                self.modificador_fora_de_ordem(pos, "factory");
            } else if let Some(l) = f.late {
                self.modificadores_conflitantes(pos, l);
            }
            return;
        }
        if f.const_.is_some() {
            self.modificador_duplicado(pos);
        } else if let Some(c) = f.covariant {
            self.modificadores_conflitantes(pos, c);
        } else if f.final_.is_some() {
            self.erro_no_token(pos, codigos::parser::CONST_AND_FINAL);
        } else if let Some(v) = f.var_ {
            self.modificadores_conflitantes(pos, v);
        }
    }

    fn mod_covariant(&mut self, f: &mut Fichas, pos: usize, depois_de_factory: bool) {
        if f.const_.is_none() && f.covariant.is_none() && f.static_.is_none() && !depois_de_factory {
            f.covariant = Some(pos);
            if let Some(v) = f.var_ {
                let t = self.text_of(v);
                self.modificador_fora_de_ordem(pos, t);
            } else if let Some(v) = f.final_ {
                let t = self.text_of(v);
                self.modificador_fora_de_ordem(pos, t);
            } else if let Some(l) = f.late {
                let t = self.text_of(l);
                self.modificador_fora_de_ordem(pos, t);
            }
            return;
        }
        if f.covariant.is_some() {
            self.modificador_duplicado(pos);
        } else if depois_de_factory {
            self.modificador_estranho(Some(pos));
        } else if let Some(c) = f.const_ {
            self.modificadores_conflitantes(pos, c);
        } else if f.static_.is_some() {
            self.erro_no_token(pos, codigos::parser::COVARIANT_AND_STATIC);
        }
    }

    fn mod_external(&mut self, f: &mut Fichas, pos: usize, depois_de_factory: bool) {
        if f.external.is_none() {
            f.external = Some(pos);
            let antes = if depois_de_factory {
                Some("factory".to_string())
            } else {
                [f.const_, f.static_, f.late, f.var_final_ou_const(), f.covariant]
                    .into_iter()
                    .flatten()
                    .next()
                    .map(|p| self.text_of(p).to_string())
            };
            if let Some(antes) = antes {
                self.modificador_fora_de_ordem(pos, &antes);
            }
            return;
        }
        self.modificador_duplicado(pos);
    }

    fn mod_final(&mut self, f: &mut Fichas, pos: usize, depois_de_factory: bool) {
        if f.var_final_ou_const().is_none() && !depois_de_factory {
            f.final_ = Some(pos);
            return;
        }
        if f.final_.is_some() {
            self.modificador_duplicado(pos);
        } else if depois_de_factory {
            self.modificador_estranho(Some(pos));
        } else if f.const_.is_some() {
            self.erro_no_token(pos, codigos::parser::CONST_AND_FINAL);
        } else if f.var_.is_some() {
            self.erro_no_token(pos, codigos::parser::FINAL_AND_VAR);
        } else if let Some(l) = f.late {
            let t = self.text_of(l);
            self.modificador_fora_de_ordem(pos, t);
        }
    }

    fn mod_late(&mut self, f: &mut Fichas, pos: usize) {
        if f.late.is_none() {
            f.late = Some(pos);
            if let Some(c) = f.const_ {
                self.modificadores_conflitantes(pos, c);
            } else if let Some(v) = f.var_ {
                let t = self.text_of(v);
                self.modificador_fora_de_ordem(pos, t);
            } else if let Some(v) = f.final_ {
                let t = self.text_of(v);
                self.modificador_fora_de_ordem(pos, t);
            }
            return;
        }
        self.modificador_duplicado(pos);
    }

    fn mod_required(&mut self, f: &mut Fichas, pos: usize) {
        if f.required.is_none() {
            f.required = Some(pos);
            if let Some(p) = f.const_.or(f.covariant).or(f.final_).or(f.var_) {
                let t = self.text_of(p);
                self.modificador_fora_de_ordem(pos, t);
            }
            return;
        }
        self.modificador_duplicado(pos);
    }

    fn mod_static(&mut self, f: &mut Fichas, pos: usize, depois_de_factory: bool) {
        if f.covariant.is_none() && f.static_.is_none() && !depois_de_factory {
            f.static_ = Some(pos);
            if let Some(p) = f.const_.or(f.final_).or(f.var_).or(f.late) {
                let t = self.text_of(p);
                self.modificador_fora_de_ordem(pos, t);
            }
            return;
        }
        if f.covariant.is_some() {
            self.erro_no_token(pos, codigos::parser::COVARIANT_AND_STATIC);
        } else if f.static_.is_some() {
            self.modificador_duplicado(pos);
        } else if depois_de_factory {
            self.modificador_estranho(Some(pos));
        }
    }

    fn mod_var(&mut self, f: &mut Fichas, pos: usize, depois_de_factory: bool) {
        if f.var_final_ou_const().is_none() && !depois_de_factory {
            f.var_ = Some(pos);
            return;
        }
        if f.var_.is_some() {
            self.modificador_duplicado(pos);
        } else if depois_de_factory {
            self.modificador_estranho(Some(pos));
        } else if let Some(c) = f.const_ {
            self.modificadores_conflitantes(pos, c);
        } else if f.final_.is_some() {
            self.erro_no_token(pos, codigos::parser::FINAL_AND_VAR);
        }
    }

    /// Consome `var`/`final`/`const` ou `late final?` no cursor (o último
    /// degrau do caminho rápido do fasta). `const` só sem `covariant`.
    fn caminho_rapido_vfc(&mut self, f: &mut Fichas) {
        if !self.e_modificador_fasta(self.pos) {
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
                if self.e_modificador_fasta(self.pos) && self.at_kw(Keyword::Final) {
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
    pub(crate) fn ler_modificadores_de_topo(&mut self) -> Fichas {
        let mut f = Fichas::default();
        if self.e_modificador_fasta(self.pos) && self.at_ident("external") {
            f.external = Some(self.pos);
            self.advance();
        }
        self.caminho_rapido_vfc(&mut f);
        // Outro `var`/`final`/`const` depois de um deles não é lido: começa
        // a declaração seguinte (`final final class C {}`).
        if f.var_final_ou_const().is_some()
            && matches!(self.kind(), Kind::Keyword(Keyword::Final | Keyword::Var | Keyword::Const))
        {
            return f;
        }
        if self.e_modificador_fasta(self.pos) {
            self.contexto_de_modificadores(&mut f, false);
            self.modificador_estranho(f.abstract_);
            self.modificador_estranho(f.covariant);
            self.modificador_estranho(f.required);
            self.modificador_estranho(f.static_);
        }
        f
    }

    /// Modificadores de membro de classe, mixin, enum, extension ou
    /// extension type (`parseClassOrMixinOrExtensionOrEnumMemberImpl`):
    /// caminho rápido `external`/`abstract`, depois `static`/`covariant`,
    /// depois `var`/`final`/`const` ou `late final?`; o que sobrar vai ao
    /// `ModifierContext` com `parseClassMemberModifiers`.
    pub(crate) fn ler_modificadores_de_membro(&mut self) -> Fichas {
        let mut f = Fichas::default();
        if self.e_modificador_fasta(self.pos) {
            if self.at_ident("external") {
                f.external = Some(self.pos);
                self.advance();
            } else if self.at_ident("abstract") {
                f.abstract_ = Some(self.pos);
                self.advance();
            }
        }
        if self.e_modificador_fasta(self.pos) {
            if self.at_ident("static") {
                f.static_ = Some(self.pos);
                self.advance();
            } else if self.at_ident("covariant") {
                f.covariant = Some(self.pos);
                self.advance();
            }
        }
        self.caminho_rapido_vfc(&mut f);
        if self.e_modificador_fasta(self.pos) {
            self.contexto_de_modificadores(&mut f, false);
            self.modificador_estranho(f.required);
        }
        f
    }

    /// `parseFormalParameterModifiers` (`modifier_context.dart:213`), depois
    /// de o `ModifierContext` ler os modificadores de um parâmetro.
    pub(crate) fn relatar_modificadores_de_parametro(
        &mut self,
        f: &Fichas,
        opcional_nomeado: bool,
        dono: DonoDeParametros,
    ) {
        if !opcional_nomeado {
            self.modificador_estranho(f.required);
        }
        match dono {
            DonoDeParametros::TopoOuEstatico => self.modificador_estranho(f.covariant),
            DonoDeParametros::Extension => {
                if let Some(c) = f.covariant {
                    let span = self.span_de(c);
                    self.erro_em(codigos::parser::INVALID_USE_OF_COVARIANT_IN_EXTENSION, span, &["covariant"]);
                }
            }
            DonoDeParametros::ExtensionType => {
                if let Some(c) = f.covariant {
                    let span = self.span_de(c);
                    self.erro_em(codigos::parser::EXTRANEOUS_MODIFIER_IN_EXTENSION_TYPE, span, &["covariant"]);
                }
            }
            // `modifier_context.dart:234-249` (checkout main): sem o recurso,
            // todo `covariant`; com ele, o que não acompanha `var`.
            DonoDeParametros::ConstrutorPrimario => {
                if let Some(c) = f.covariant
                    && (!self.features.tem(crate::Feature::PrimaryConstructors) || f.var_.is_none())
                {
                    let span = self.span_de(c);
                    self.erro_em(codigos::parser::EXTRANEOUS_MODIFIER_IN_PRIMARY_CONSTRUCTOR, span, &["covariant"]);
                }
            }
            DonoDeParametros::TipoDeFuncao
            | DonoDeParametros::AliasDeTipo
            | DonoDeParametros::ParametroFuncao
            | DonoDeParametros::Outro => {}
        }
        if f.const_.is_some() {
            self.modificador_estranho(f.const_);
        } else if dono == DonoDeParametros::TipoDeFuncao
            && let Some(v) = f.var_final_ou_const()
        {
            self.erro_no_token(v, codigos::parser::FUNCTION_TYPED_PARAMETER_VAR);
        }
        self.modificador_estranho(f.abstract_);
        self.modificador_estranho(f.external);
        self.modificador_estranho(f.late);
        self.modificador_estranho(f.static_);
    }

    /// `reportTopLevelModifierError` + os `reportExtraneousModifier` de
    /// `parseClassModifiers`/`parseEnumModifiers`/`parseMixinModifiers`/
    /// `parseExtensionModifiers`/`parseTypedefModifiers`/
    /// `parseTopLevelKeywordModifiers` para os modificadores de membro que
    /// precedem uma palavra de topo (`static class`, `const enum`…). `palavra`
    /// é o texto da palavra de topo; `abstract` e `final` só valem em
    /// `class` (e `final` também em `enum`/`mixin`/`extension`).
    pub(crate) fn relatar_modificadores_antes_de_topo(&mut self, f: &Fichas, palavra: &str) {
        let classe = palavra == "class";
        let diretiva_ou_typedef = matches!(palavra, "typedef" | "library" | "import" | "export" | "part");
        if let Some(c) = f.const_ {
            if classe {
                self.erro_no_token(c, codigos::parser::CONST_CLASS);
            } else {
                self.modificador_estranho(Some(c));
            }
        }
        if let Some(e) = f.external {
            match palavra {
                "class" => self.erro_no_token(e, codigos::parser::EXTERNAL_CLASS),
                "enum" => self.erro_no_token(e, codigos::parser::EXTERNAL_ENUM),
                "typedef" => self.erro_no_token(e, codigos::parser::EXTERNAL_TYPEDEF),
                _ => self.modificador_estranho(Some(e)),
            }
        }
        if !classe {
            self.modificador_estranho(f.abstract_);
        }
        self.modificador_estranho(f.covariant);
        if diretiva_ou_typedef {
            self.modificador_estranho(f.final_);
        }
        self.modificador_estranho(f.late);
        self.modificador_estranho(f.required);
        self.modificador_estranho(f.static_);
        self.modificador_estranho(f.var_);
    }
}
