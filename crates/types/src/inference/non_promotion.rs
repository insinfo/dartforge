//! Why-not-promoted: as mensagens de contexto que explicam por que uma
//! expressão não está promovida (`computeWhyNotPromotedMessages` e o
//! `_WhyNotPromotedVisitor`, `an611:src/generated/resolver.dart:699-739` e
//! `:5561-5736`; o `whyNotPromoted` do fluxo,
//! `_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart:5581-5822`).
//!
//! Na leitura de cada referência (variável local ou propriedade) guarda-se
//! o mapa `tipo -> motivo` daquele ponto, como o `whyNotPromoted(target)`
//! chamado logo depois da expressão; `this` (inclusive o implícito) é
//! consultado na hora do erro. Quem relata um erro que o analyzer acompanha
//! dessas mensagens chama [`BodyInferrer::anexar_nao_promocao`].

use super::body::{Base, Body};
use super::flow::NonPromotionReason;
use super::BodyInferrer;
use crate::field_promotion::{self, WhyNotPromoted, LibraryPromotion};
use crate::resolved::LocalId;
use crate::table::TypeId;
use dartforge_diagnostics::{Contexto, Span};
use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_intern::SymbolId;
use std::rc::Rc;

/// `ElementKind.displayName` de uma classe.
fn especie(program: &Program, c: ClassId) -> &'static str {
    match program.class(c).kind {
        ClassKind::Class | ClassKind::MixinApplication => "class",
        ClassKind::Mixin => "mixin",
        ClassKind::Enum => "enum",
        ClassKind::ExtensionType => "extension type",
    }
}

/// Uma mensagem de contexto num ponto de `u` (o arquivo, quando não é o do
/// erro).
fn contexto(program: &Program, error_unit: UnitId, u: UnitId, span: Span, message: String) -> Contexto {
    Contexto { arquivo: (u != error_unit).then(|| program.caminho_da_unidade(u).into()), span, mensagem: message.into() }
}

impl BodyInferrer<'_> {
    /// O `FieldPromotability` de uma biblioteca (calculado uma vez).
    pub(crate) fn library_promotion(&mut self, lib: LibraryId) -> Rc<LibraryPromotion> {
        if let Some(p) = self.promotion_by_library.get(&lib) {
            return p.clone();
        }
        let p = Rc::new(field_promotion::calcular(self.program, self.interner, lib));
        self.promotion_by_library.insert(lib, p.clone());
        p
    }

    /// `_handleProperty`: `fieldPromotionEnabled` no corpo (`lib`) e
    /// `isPropertyPromotable` do membro (o getter de um campo, ou o getter
    /// abstrato, cujo campo tem `isPromotable`).
    pub(crate) fn promotable_property(&mut self, lib: LibraryId, f: FunctionElementId) -> bool {
        if !field_promotion::enabled(self.program, lib) {
            return false;
        }
        let fe = self.program.function(f);
        match fe.kind {
            FunctionKind::ImplicitAccessor => match fe.variable {
                Some(v) => {
                    let owner = self.program.variable(v).library;
                    self.library_promotion(owner).campos.contains(&v)
                }
                None => false,
            },
            FunctionKind::Getter => {
                let owner = fe.library;
                self.library_promotion(owner).getters.contains(&f)
            }
            _ => false,
        }
    }

    /// O `whyPropertyIsNotPromotable` (com o nome público tratado antes,
    /// como o fluxo faz): `None` é o motivo não inerente (conflito, ou o
    /// recurso desligado).
    fn why_not_promoted(&mut self, name: SymbolId, f: FunctionElementId) -> Option<WhyNotPromoted> {
        if !self.interner.resolve(name).starts_with('_') {
            return Some(WhyNotPromoted::NotPrivate);
        }
        let fe = self.program.function(f);
        // Um método, ou um getter escrito (o campo dele é sintético).
        if fe.kind != FunctionKind::ImplicitAccessor {
            return Some(WhyNotPromoted::NotAField);
        }
        let Some(v) = fe.variable else { return Some(WhyNotPromoted::NotAField) };
        let ve = self.program.variable(v);
        if ve.class.is_none() && ve.extension.is_none() {
            return Some(WhyNotPromoted::NotAField);
        }
        let (external, final_, owner) = (ve.external, ve.final_, ve.library);
        if self.library_promotion(owner).campos.contains(&v) {
            return None;
        }
        if external {
            return Some(WhyNotPromoted::External);
        }
        if !final_ {
            return Some(WhyNotPromoted::NonFinal);
        }
        None
    }

    /// `whyNotPromoted` da leitura `e` da variável `id`: do histórico de
    /// não promoção, cada tipo de que o tipo atual não é subtipo (o
    /// primeiro motivo de cada tipo fica).
    pub(crate) fn record_local_non_promotion(&mut self, cx: &Body, e: ExprId, id: LocalId) {
        let key = (cx.unit, e);
        let Some(m) = cx.flow.modelo(id) else {
            self.non_promotions.remove(&key);
            return;
        };
        let current = m.cadeia.last().copied().unwrap_or(cx.local(id).tipo);
        let mut history = m.history.clone();
        let mut r: Vec<(TypeId, NonPromotionReason)> = Vec::new();
        while let Some(no) = history {
            if !self.sub(current, no.tipo) && !r.iter().any(|(t, _)| *t == no.tipo) {
                r.push((no.tipo, no.motivo.clone()));
            }
            history = no.anterior.clone();
        }
        if r.is_empty() {
            self.non_promotions.remove(&key);
        } else {
            self.non_promotions.insert(key, r);
        }
    }

    /// `whyNotPromoted` da leitura `e` de uma propriedade não promovível: os
    /// tipos promovidos (no fluxo de agora) de cada geração anterior, da mais
    /// nova à mais antiga, todos com o mesmo motivo.
    pub(crate) fn record_property_non_promotion(&mut self, cx: &Body, e: ExprId, property_key: (Base, u32, SymbolId), f: FunctionElementId) {
        let key = (cx.unit, e);
        let own = cx.read_generation.get(&e).copied();
        let mut lists: Vec<Vec<TypeId>> = Vec::new();
        if let Some(generations) = cx.generations.get(&property_key) {
            for &g in generations.iter().rev() {
                if Some(g) == own {
                    continue;
                }
                if let Some(m) = cx.flow.modelo(g) {
                    if !m.cadeia.is_empty() {
                        lists.push(m.cadeia.clone());
                    }
                }
            }
        }
        if lists.is_empty() {
            self.non_promotions.remove(&key);
            return;
        }
        let name = property_key.2;
        let inherent = self.why_not_promoted(name, f);
        let enabled = field_promotion::enabled(self.program, cx.lib);
        let reason = NonPromotionReason::Propriedade { nome: name, membro: f, inherent, enabled };
        let mut r: Vec<(TypeId, NonPromotionReason)> = Vec::new();
        for l in lists {
            for t in l {
                if !r.iter().any(|(u, _)| *u == t) {
                    r.push((t, reason.clone()));
                }
            }
        }
        self.non_promotions.insert(key, r);
    }

    /// `whyNotPromotedImplicitThis` (e o de `this`/`super` escritos): cada
    /// tipo promovido de `this`, com `ThisNotPromoted`.
    fn this_reasons(&self, cx: &Body) -> Vec<(TypeId, NonPromotionReason)> {
        let Some(id) = cx.local_this else { return Vec::new() };
        let Some(m) = cx.flow.modelo(id) else { return Vec::new() };
        m.cadeia.iter().map(|&t| (t, NonPromotionReason::This)).collect()
    }

    /// `computeWhyNotPromotedMessages(errorEntity, whyNotPromoted(alvo))`:
    /// as mensagens do primeiro tipo não potencialmente anulável do mapa.
    /// `alvo` `None` é o `this` implícito; `erro` é o `errorEntity`.
    pub(crate) fn non_promotion_messages(&mut self, cx: &Body, target: Option<ExprId>, error: Span) -> Vec<Contexto> {
        let reasons = match target {
            Some(e) => {
                let a = &self.program.unit(cx.unit).ast;
                let mut x = e;
                while let ExprKind::Parenthesized(i) = &a.expr(x).kind {
                    x = *i;
                }
                match &a.expr(x).kind {
                    ExprKind::This | ExprKind::Super => self.this_reasons(cx),
                    _ => self.non_promotions.get(&(cx.unit, x)).cloned().unwrap_or_default(),
                }
            }
            None => self.this_reasons(cx),
        };
        for (t, m) in reasons {
            if !self.is_non_nullable(t) {
                continue;
            }
            return self.reason_messages(cx, &m, error);
        }
        Vec::new()
    }

    /// O `_WhyNotPromotedVisitor`.
    fn reason_messages(&mut self, cx: &Body, m: &NonPromotionReason, error: Span) -> Vec<Contexto> {
        match m {
            NonPromotionReason::Escrita { nome: name, span } => {
                let text = format!("Variable '{}' could not be promoted due to an assignment", self.interner.resolve(*name));
                vec![Contexto { arquivo: None, span: *span, mensagem: text.into() }]
            }
            NonPromotionReason::This => vec![Contexto { arquivo: None, span: error, mensagem: "'this' can't be promoted".into() }],
            NonPromotionReason::Propriedade { nome: name, membro: member, inherent, enabled } => {
                let program = self.program;
                let fe = program.function(*member);
                // `receiverElement is PropertyAccessorElement`: só getters.
                if !matches!(fe.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) {
                    return Vec::new();
                }
                let text = self.interner.resolve(*name).to_string();
                let where_ = program.nome_nao_sintetico_da_funcao(*member);
                let unavailable = format!("'{text}' couldn't be promoted because field promotion is only available in Dart 3.2 and above.");
                let mut r = Vec::new();
                match inherent {
                    Some(pq) => {
                        let message = match pq {
                            WhyNotPromoted::NotAField => format!("'{text}' refers to a getter so it couldn't be promoted."),
                            WhyNotPromoted::NotPrivate => format!("'{text}' refers to a public property so it couldn't be promoted."),
                            WhyNotPromoted::External => format!("'{text}' refers to an external field so it couldn't be promoted."),
                            WhyNotPromoted::NonFinal => format!("'{text}' refers to a non-final field so it couldn't be promoted."),
                        };
                        if let Some((u, s)) = where_ {
                            r.push(contexto(program, cx.unit, u, s, message));
                            if !enabled {
                                r.push(contexto(program, cx.unit, u, s, unavailable));
                            }
                        }
                    }
                    None => {
                        let info = self.library_promotion(fe.library);
                        if let Some(i) = info.info.get(name) {
                            let conflict = |ty: &str, c: Option<ClassId>| {
                                let (k, n) = match c {
                                    Some(c) => (especie(program, c), self.interner.resolve(program.class(c).name).to_string()),
                                    None => ("class", String::new()),
                                };
                                format!("'{text}' couldn't be promoted because there is a conflicting {ty} in {k} '{n}'")
                            };
                            for &v in &i.campos {
                                if let Some((u, s)) = program.nome_da_variavel(v) {
                                    r.push(contexto(program, cx.unit, u, s, conflict("non-promotable field", program.variable(v).class)));
                                }
                            }
                            for &g in &i.getters {
                                if let Some((u, s)) = program.nome_nao_sintetico_da_funcao(g) {
                                    r.push(contexto(program, cx.unit, u, s, conflict("getter", program.function(g).class)));
                                }
                            }
                            for &c in &i.nsm_classes {
                                if let Some((u, s)) = program.nome_da_classe(c) {
                                    r.push(contexto(program, cx.unit, u, s, conflict("noSuchMethod forwarder", Some(c))));
                                }
                            }
                        }
                        if !enabled {
                            if let Some((u, s)) = where_ {
                                r.push(contexto(program, cx.unit, u, s, unavailable));
                            }
                        }
                    }
                }
                r
            }
        }
    }

    /// Anexa as mensagens do why-not-promoted de `alvo` aos diagnósticos
    /// relatados desde `desde` (o erro que o analyzer acompanha delas).
    pub(crate) fn attach_non_promotion(&mut self, since: usize, cx: &Body, target: Option<ExprId>, error: Span) {
        if self.diagnostics.len() <= since {
            return;
        }
        let messages = self.non_promotion_messages(cx, target, error);
        if messages.is_empty() {
            return;
        }
        for d in &mut self.diagnostics[since..] {
            d.contexto.extend(messages.iter().cloned());
        }
    }
}
