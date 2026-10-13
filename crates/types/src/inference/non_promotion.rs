//! Why-not-promoted: the context messages that explain why an expression is
//! not promoted (`computeWhyNotPromotedMessages` and the
//! `_WhyNotPromotedVisitor`, `an611:src/generated/resolver.dart:699-739` and
//! `:5561-5736`; the flow's `whyNotPromoted`,
//! `_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart:5581-5822`).
//!
//! On the read of each reference (local variable or property) the
//! `ty -> reason` map of that point is stored, like the
//! `whyNotPromoted(target)` called right after the expression; `this`
//! (including the implicit one) is consulted when the error happens. Whoever
//! reports an error that the analyzer accompanies with these messages calls
//! [`BodyInferrer::attach_non_promotion`].

use super::body::{Base, Body};
use super::flow::NonPromotionReason;
use super::BodyInferrer;
use crate::field_promotion::{self, WhyNotPromoted, LibraryPromotion};
use crate::resolved::LocalId;
use crate::table::TypeId;
use dartforge_diagnostics::{ContextMessage, Span};
use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_intern::SymbolId;
use std::rc::Rc;

/// `ElementKind.displayName` of a class.
fn the_kind(program: &Program, c: ClassId) -> &'static str {
    match program.class(c).kind {
        ClassKind::Class | ClassKind::MixinApplication => "class",
        ClassKind::Mixin => "mixin",
        ClassKind::Enum => "enum",
        ClassKind::ExtensionType => "extension type",
    }
}

/// A context message at a point of `u` (the file, when it is not the
/// error's).
fn context(program: &Program, error_unit: UnitId, u: UnitId, span: Span, message: String) -> ContextMessage {
    ContextMessage { file: (u != error_unit).then(|| program.unit_path(u).into()), span, message: message.into() }
}

impl BodyInferrer<'_> {
    /// The `FieldPromotability` of a library (computed once).
    pub(crate) fn library_promotion(&mut self, lib: LibraryId) -> Rc<LibraryPromotion> {
        if let Some(p) = self.promotion_by_library.get(&lib) {
            return p.clone();
        }
        let p = Rc::new(field_promotion::compute(self.program, self.interner, lib));
        self.promotion_by_library.insert(lib, p.clone());
        p
    }

    /// `_handleProperty`: `fieldPromotionEnabled` in the body (`lib`) and the
    /// member's `isPropertyPromotable` (the getter of a field, or the abstract
    /// getter, whose field has `isPromotable`).
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

    /// `whyPropertyIsNotPromotable` (with the public name handled first, as the
    /// flow does): `None` is the non-inherent reason (a conflict, or the feature
    /// being off).
    fn why_not_promoted(&mut self, name: SymbolId, f: FunctionElementId) -> Option<WhyNotPromoted> {
        if !self.interner.resolve(name).starts_with('_') {
            return Some(WhyNotPromoted::NotPrivate);
        }
        let fe = self.program.function(f);
        // A method, or a written getter (its field is synthetic).
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

    /// `whyNotPromoted` of the read `e` of variable `id`: from the non-promotion
    /// history, each type of which the current type is not a subtype (the first
    /// reason for each type is kept).
    pub(crate) fn record_local_non_promotion(&mut self, cx: &Body, e: ExprId, id: LocalId) {
        let key = (cx.unit, e);
        let Some(m) = cx.flow.model(id) else {
            self.non_promotions.remove(&key);
            return;
        };
        let current = m.chain.last().copied().unwrap_or(cx.local(id).ty);
        let mut history = m.history.clone();
        let mut r: Vec<(TypeId, NonPromotionReason)> = Vec::new();
        while let Some(no) = history {
            if !self.sub(current, no.ty) && !r.iter().any(|(t, _)| *t == no.ty) {
                r.push((no.ty, no.reason.clone()));
            }
            history = no.previous.clone();
        }
        if r.is_empty() {
            self.non_promotions.remove(&key);
        } else {
            self.non_promotions.insert(key, r);
        }
    }

    /// `whyNotPromoted` of the read `e` of a non-promotable property: the
    /// promoted types (in the current flow) of each earlier generation, from
    /// newest to oldest, all with the same reason.
    pub(crate) fn record_property_non_promotion(&mut self, cx: &Body, e: ExprId, property_key: (Base, u32, SymbolId), f: FunctionElementId) {
        let key = (cx.unit, e);
        let own = cx.read_generation.get(&e).copied();
        let mut lists: Vec<Vec<TypeId>> = Vec::new();
        if let Some(generations) = cx.generations.get(&property_key) {
            for &g in generations.iter().rev() {
                if Some(g) == own {
                    continue;
                }
                if let Some(m) = cx.flow.model(g) {
                    if !m.chain.is_empty() {
                        lists.push(m.chain.clone());
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
        let reason = NonPromotionReason::Property { nome: name, membro: f, inherent, enabled };
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

    /// `whyNotPromotedImplicitThis` (and that of written `this`/`super`): each
    /// promoted type of `this`, with `ThisNotPromoted`.
    fn this_reasons(&self, cx: &Body) -> Vec<(TypeId, NonPromotionReason)> {
        let Some(id) = cx.local_this else { return Vec::new() };
        let Some(m) = cx.flow.model(id) else { return Vec::new() };
        m.chain.iter().map(|&t| (t, NonPromotionReason::This)).collect()
    }

    /// `computeWhyNotPromotedMessages(errorEntity, whyNotPromoted(target))`: the
    /// messages of the first non-potentially-nullable type in the map. `target`
    /// `None` is the implicit `this`; `error` is the `errorEntity`.
    pub(crate) fn non_promotion_messages(&mut self, cx: &Body, target: Option<ExprId>, error: Span) -> Vec<ContextMessage> {
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
    fn reason_messages(&mut self, cx: &Body, m: &NonPromotionReason, error: Span) -> Vec<ContextMessage> {
        match m {
            NonPromotionReason::Escrita { nome: name, span } => {
                let text = format!("Variable '{}' could not be promoted due to an assignment", self.interner.resolve(*name));
                vec![ContextMessage { file: None, span: *span, message: text.into() }]
            }
            NonPromotionReason::This => vec![ContextMessage { file: None, span: error, message: "'this' can't be promoted".into() }],
            NonPromotionReason::Property { nome: name, membro: member, inherent, enabled } => {
                let program = self.program;
                let fe = program.function(*member);
                // `receiverElement is PropertyAccessorElement`: only getters.
                if !matches!(fe.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) {
                    return Vec::new();
                }
                let text = self.interner.resolve(*name).to_string();
                let where_ = program.non_synthetic_function_name(*member);
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
                            r.push(context(program, cx.unit, u, s, message));
                            if !enabled {
                                r.push(context(program, cx.unit, u, s, unavailable));
                            }
                        }
                    }
                    None => {
                        let info = self.library_promotion(fe.library);
                        if let Some(i) = info.info.get(name) {
                            let conflict = |ty: &str, c: Option<ClassId>| {
                                let (k, n) = match c {
                                    Some(c) => (the_kind(program, c), self.interner.resolve(program.class(c).name).to_string()),
                                    None => ("class", String::new()),
                                };
                                format!("'{text}' couldn't be promoted because there is a conflicting {ty} in {k} '{n}'")
                            };
                            for &v in &i.fields {
                                if let Some((u, s)) = program.variable_name(v) {
                                    r.push(context(program, cx.unit, u, s, conflict("non-promotable field", program.variable(v).class)));
                                }
                            }
                            for &g in &i.getters {
                                if let Some((u, s)) = program.non_synthetic_function_name(g) {
                                    r.push(context(program, cx.unit, u, s, conflict("getter", program.function(g).class)));
                                }
                            }
                            for &c in &i.nsm_classes {
                                if let Some((u, s)) = program.class_name(c) {
                                    r.push(context(program, cx.unit, u, s, conflict("noSuchMethod forwarder", Some(c))));
                                }
                            }
                        }
                        if !enabled {
                            if let Some((u, s)) = where_ {
                                r.push(context(program, cx.unit, u, s, unavailable));
                            }
                        }
                    }
                }
                r
            }
        }
    }

    /// Attaches the why-not-promoted messages of `target` to the diagnostics
    /// reported since `since` (the error that the analyzer accompanies with
    /// them).
    pub(crate) fn attach_non_promotion(&mut self, since: usize, cx: &Body, target: Option<ExprId>, error: Span) {
        if self.diagnostics.len() <= since {
            return;
        }
        let messages = self.non_promotion_messages(cx, target, error);
        if messages.is_empty() {
            return;
        }
        for d in &mut self.diagnostics[since..] {
            d.context.extend(messages.iter().cloned());
        }
    }
}
