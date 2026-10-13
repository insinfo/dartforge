//! Modelo de fluxo (`flow-analysis.md`): cadeias de promoção, tipos de
//! interesse, atribuição definitiva e alcançabilidade.
//!
//! Um [`Fluxo`] é o `FlowModel` da especificação; cada variável local tem um
//! [`ModeloVar`] (`PromotionModel`). As junções seguem `join`/`joinPM`; a
//! atribuição segue `demote` + `toi_promote`; a promoção por teste segue
//! `promote`.

use super::BodyInferrer;
use crate::field_promotion::WhyNotPromoted;
use crate::resolved::LocalId;
use crate::table::{Type, TypeId};
use dartforge_diagnostics::Span;
use dartforge_elements::model::FunctionElementId;
use dartforge_intern::SymbolId;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

/// `NonPromotionReason`: por que um tipo não está (mais) promovido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NonPromotionReason {
    /// `DemoteViaExplicitWrite`: o nome da variável e o nó da escrita (a
    /// atribuição, o `++`/`--`, o identificador do `for-in`, o padrão).
    Escrita { nome: SymbolId, span: Span },
    /// `PropertyNotPromotedForInherentReason` (com o motivo) ou
    /// `PropertyNotPromotedForNonInherentReason` (sem): o nome, o membro
    /// (`propertyMember`) e se a promoção de campos vale na biblioteca do
    /// corpo (`fieldPromotionEnabled`).
    Propriedade { nome: SymbolId, membro: FunctionElementId, inherent: Option<WhyNotPromoted>, enabled: bool },
    /// `ThisNotPromoted`.
    This,
}

/// `NonPromotionHistory`: a lista ligada (o mais recente primeiro) dos tipos
/// que uma escrita desfez.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct History {
    pub tipo: TypeId,
    pub motivo: NonPromotionReason,
    pub anterior: Option<Rc<History>>,
}

/// Versões de escrita (o SSA do `flow-analysis.md`): cada escrita e cada
/// junção de versões diferentes ganha um número novo; só a igualdade importa.
static NEXT_VERSION: AtomicU32 = AtomicU32::new(1);

fn new_version() -> u32 {
    NEXT_VERSION.fetch_add(1, Ordering::Relaxed)
}

/// Estado de uma variável num ponto do programa (`PromotionModel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VarModel {
    /// Cadeia de promoção: o último é o tipo atual.
    pub cadeia: Vec<TypeId>,
    /// Tipos de interesse (testados em algum caminho).
    pub tested: Vec<TypeId>,
    pub assigned: bool,
    pub unassigned: bool,
    /// Escrita por uma closure: não promove mais.
    pub captured: bool,
    /// Versão da última escrita (0: declarada e nunca escrita).
    pub versao: u32,
    /// `nonPromotionHistory`.
    pub history: Option<Rc<History>>,
}

/// Estado de fluxo num ponto (`FlowModel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fluxo {
    pub alcancavel: bool,
    pub vars: Vec<Option<VarModel>>,
}

impl Fluxo {
    pub fn alcancavel() -> Self {
        Fluxo { alcancavel: true, vars: Vec::new() }
    }

    pub fn unreachable(&self) -> Self {
        let mut f = self.clone();
        f.alcancavel = false;
        f
    }

    fn slot(&mut self, id: LocalId) -> &mut Option<VarModel> {
        let i = id.0 as usize;
        if self.vars.len() <= i {
            self.vars.resize(i + 1, None);
        }
        &mut self.vars[i]
    }

    pub fn declarar(&mut self, id: LocalId) {
        *self.slot(id) = Some(VarModel {
            cadeia: Vec::new(),
            tested: Vec::new(),
            assigned: false,
            unassigned: true,
            captured: false,
            versao: 0,
            history: None,
        });
    }

    /// Declaração com valor (inicializador, parâmetro, variável de laço).
    pub fn inicializar(&mut self, id: LocalId) {
        if let Some(m) = self.slot(id) {
            m.assigned = true;
            m.unassigned = false;
            m.versao = new_version();
        }
    }

    /// Versão de escrita atual de uma variável.
    pub fn versao(&self, id: LocalId) -> Option<u32> {
        self.modelo(id).map(|m| m.versao)
    }

    pub fn modelo(&self, id: LocalId) -> Option<&VarModel> {
        self.vars.get(id.0 as usize).and_then(|m| m.as_ref())
    }

    pub fn assigned(&self, id: LocalId) -> bool {
        self.modelo(id).is_some_and(|m| m.assigned)
    }

    pub fn unassigned(&self, id: LocalId) -> bool {
        self.modelo(id).is_some_and(|m| m.unassigned)
    }

    /// Tipo atual (promovido) de uma variável.
    pub fn current_type(&self, id: LocalId, declared: TypeId) -> TypeId {
        self.modelo(id).and_then(|m| m.cadeia.last().copied()).unwrap_or(declared)
    }

    /// Escrita numa closure: sem promoção e, como no `conservativeJoin` do
    /// `functionExpression_begin`/`_end`, nunca mais definitivamente não
    /// atribuída.
    pub fn capture(&mut self, id: LocalId) {
        if let Some(m) = self.slot(id) {
            // `writeCapture`: sem promoções, sem tipos testados e sem
            // histórico.
            m.captured = true;
            m.cadeia.clear();
            m.tested.clear();
            m.unassigned = false;
            m.versao = new_version();
            m.history = None;
        }
    }

    /// Esquece promoções das variáveis dadas (junção conservadora de laços).
    pub fn conservative_join(&mut self, writes: &[LocalId], captured: &[LocalId]) {
        // `discardPromotionsAndMarkNotUnassigned` (o modelo novo não leva o
        // histórico).
        for &id in captured {
            if let Some(m) = self.slot(id) {
                m.cadeia.clear();
                m.unassigned = false;
                m.captured = true;
                m.versao = new_version();
                m.history = None;
            }
        }
        for &id in writes {
            if let Some(m) = self.slot(id) {
                m.cadeia.clear();
                m.unassigned = false;
                m.versao = new_version();
                m.history = None;
            }
        }
    }
}

impl<'a> BodyInferrer<'a> {
    /// `join(M1, M2)`.
    pub(crate) fn juntar(&mut self, a: &Fluxo, b: &Fluxo) -> Fluxo {
        if !a.alcancavel {
            return b.clone();
        }
        if !b.alcancavel {
            return a.clone();
        }
        if a == b {
            return a.clone();
        }
        let n = a.vars.len().min(b.vars.len());
        let mut vars = Vec::with_capacity(n);
        for i in 0..n {
            vars.push(match (&a.vars[i], &b.vars[i]) {
                (Some(x), Some(y)) => Some(join_model(x, y)),
                _ => None,
            });
        }
        Fluxo { alcancavel: true, vars }
    }

    /// Junta uma lista de modelos (vazia → inalcançável a partir de `base`).
    pub(crate) fn join_all(&mut self, base: &Fluxo, list: &[Fluxo]) -> Fluxo {
        let mut it = list.iter();
        let Some(p) = it.next() else { return base.unreachable() };
        let mut acc = p.clone();
        for f in it {
            acc = self.juntar(&acc, f);
        }
        acc
    }

    /// `promote(x, T)`: promoção por teste de tipo (`is`, `as`, `!= null`).
    pub(crate) fn promover(&mut self, flow: &mut Fluxo, id: LocalId, declared: TypeId, t: TypeId) {
        self.promote_tested(flow, id, declared, t, t);
    }

    /// Promove para `t` registrando `testado` como tipo de interesse (o ramo
    /// falso de `x is T` testa `T` e promove para `factor(S, T)`;
    /// `_finishTypeTest` do analisador).
    pub(crate) fn promote_tested(&mut self, flow: &mut Fluxo, id: LocalId, declared: TypeId, t: TypeId, the_tested: TypeId) {
        // Também no código inalcançável: o `tryPromoteForTypeCheck` do
        // modelo não olha a alcançabilidade (`flow_analysis.dart`), e o
        // código morto continua resolvido com as promoções.
        let Some(m) = flow.modelo(id).cloned() else { return };
        if m.captured {
            return;
        }
        let s = m.cadeia.last().copied().unwrap_or(declared);
        // `tryPromoteForTypeCheck` (3.6.2 `flow_analysis.dart:2534-2542`): o
        // `tryPromoteToType` dá `T` quando `T <: S`, e só não promove quando o
        // resultado é o próprio tipo anterior. Subtipos mútuos e diferentes
        // (`dynamic Function()` e `void Function()`) promovem.
        if t == s {
            // O mesmo tipo: não promove, mas registra o tipo de interesse.
            if !m.tested.contains(&the_tested) {
                if let Some(Some(mm)) = flow.vars.get_mut(id.0 as usize) {
                    mm.tested.push(the_tested);
                }
            }
            return;
        }
        let t1 = if self.sub(t, s) {
            Some(t)
        } else {
            // S é X (limite R) ou X & R: promove para X & T se T <: R.
            match self.table.get(s).clone() {
                Type::TypeParameter { param, nullable: false } if param != self.core.unknown_param => {
                    let b = self.table.param(param).bound;
                    if self.sub(t, b) {
                        Some(self.table.intern(Type::Intersection { param, bound: t }))
                    } else {
                        None
                    }
                }
                Type::Intersection { param, bound } => {
                    if self.sub(t, bound) {
                        Some(self.table.intern(Type::Intersection { param, bound: t }))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        if let Some(Some(mm)) = flow.vars.get_mut(id.0 as usize) {
            if !mm.tested.contains(&the_tested) {
                mm.tested.push(the_tested);
            }
            if let Some(t1) = t1 {
                mm.cadeia.push(t1);
            }
        }
        if let Some(t1) = t1 {
            if matches!(self.table.get(t1), Type::Never) {
                flow.alcancavel = false;
            }
        }
    }

    /// O ramo falso de `x is T` (`tryPromoteForTypeCheck`, 3.6.2
    /// `flow_analysis.dart:2544-2557`, com o `_finishTypeTest` de `:2633-2664`):
    /// o tipo fatorado entra direto na cadeia, sem o `tryPromoteToType` do ramo
    /// verdadeiro (`FutureOr<void>` menos `Future` é `void`, que é supertipo),
    /// e `T` fica registrado como testado.
    pub(crate) fn promote_factored(&mut self, flow: &mut Fluxo, id: LocalId, factored: Option<TypeId>, the_tested: TypeId) {
        let Some(m) = flow.modelo(id) else { return };
        if m.captured {
            return;
        }
        if let Some(Some(mm)) = flow.vars.get_mut(id.0 as usize) {
            if !mm.tested.contains(&the_tested) {
                mm.tested.push(the_tested);
            }
            if let Some(t) = factored {
                mm.cadeia.push(t);
            }
        }
    }

    /// `promoteToNonNull(x)`.
    pub(crate) fn promote_non_null(&mut self, flow: &mut Fluxo, id: LocalId, declared: TypeId) {
        let current = flow.current_type(id, declared);
        let nn = self.non_null_promotion(current);
        if nn != current {
            self.promover(flow, id, declared, nn);
        }
    }

    /// **NonNull**(`T`) para promoção (`T?` de variável de tipo vira `T`).
    pub(crate) fn non_null_promotion(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Null => self.core.never,
            Type::FutureOr { arg, nullable: true } => self.table.intern(Type::FutureOr { arg, nullable: false }),
            // NonNull(X) = X & NonNull(B) quando o limite B é anulável
            // (promoteToNonNull do analyzer): T? vira T & Object.
            Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                let b = self.table.param(param).bound;
                let nb = self.non_null_promotion(b);
                if nb != b && !matches!(self.table.get(b), Type::Dynamic) {
                    self.table.intern(Type::Intersection { param, bound: nb })
                } else {
                    self.table.intern(Type::TypeParameter { param, nullable: false })
                }
            }
            _ => self.non_null(t),
        }
    }

    /// `rebaseForward` (§7.10): as promoções guardadas numa variável de
    /// condição valem de novo sobre o estado atual, para as variáveis que
    /// não foram escritas desde então (mesma versão).
    pub(crate) fn reapply(&mut self, current: &Fluxo, stored: &Fluxo) -> Fluxo {
        if !stored.alcancavel {
            return current.unreachable();
        }
        let mut r = current.clone();
        for (i, g) in stored.vars.iter().enumerate() {
            let Some(g) = g else { continue };
            let Some(Some(c)) = r.vars.get(i) else { continue };
            if g.versao != c.versao || c.captured {
                continue;
            }
            let mut c = c.clone();
            let before = (c.cadeia.len(), c.tested.len());
            for &t in &g.cadeia {
                if !c.cadeia.contains(&t) && c.cadeia.last().is_none_or(|&l| self.sub(t, l)) {
                    c.cadeia.push(t);
                }
            }
            for &t in &g.tested {
                if !c.tested.contains(&t) {
                    c.tested.push(t);
                }
            }
            // `_identicalOrNew`: um modelo novo não leva o histórico.
            if (c.cadeia.len(), c.tested.len()) != before {
                c.history = None;
            }
            r.vars[i] = Some(c);
        }
        r
    }

    /// `assign(x, T)`: demove e aplica a promoção por tipo de interesse.
    /// `motivo`: o `DemoteViaExplicitWrite` da escrita, que entra no
    /// histórico de cada tipo desfeito.
    pub(crate) fn assign_flow(&mut self, flow: &mut Fluxo, id: LocalId, declared: TypeId, written: TypeId, reason: Option<NonPromotionReason>) {
        self.write_flow(flow, id, declared, written, true, reason);
    }

    /// Escrita com ou sem promoção por tipo de interesse (a inicialização de
    /// uma variável `final` ou sem tipo escrito não promove, `_initialize`).
    pub(crate) fn write_flow(&mut self, flow: &mut Fluxo, id: LocalId, declared: TypeId, written: TypeId, toi: bool, reason: Option<NonPromotionReason>) {
        let Some(m) = flow.modelo(id).cloned() else { return };
        let mut m = m;
        let assigned_before = m.assigned;
        m.assigned = true;
        m.unassigned = false;
        m.versao = new_version();
        if m.captured {
            m.history = None;
        } else {
            let chain_before = m.cadeia.clone();
            // O `InvalidType` escrito é subtipo de tudo (promove ao não
            // anulável do declarado como um `Never`); a escrita de `dynamic`
            // num local tipado é cast implícito: o tipo escrito efetivo é o
            // declarado.
            let written = if self.table.e_invalido(written) && !self.e_dynamic(declared) {
                self.core.never
            } else if self.e_dynamic(written) && !self.e_dynamic(declared) {
                declared
            } else {
                written
            };
            // demote (`_demoteViaAssignment`): a cadeia é decrescente, então
            // o que fica é um prefixo; cada tipo desfeito entra no histórico,
            // do último ao primeiro (o primeiro desfeito fica na frente).
            let mut chain: Vec<TypeId> = Vec::new();
            for &t in &m.cadeia {
                if self.sub(written, t) {
                    chain.push(t);
                }
            }
            let mut history = m.history.clone();
            if let Some(reason) = &reason {
                for &t in m.cadeia[chain.len()..].iter().rev() {
                    history = Some(Rc::new(History { tipo: t, motivo: reason.clone(), anterior: history }));
                }
            }
            // toi_promote
            if !toi {
                if !self.sub(written, declared) {
                    chain.clear();
                }
            } else if self.sub(written, declared) {
                let provisional = chain.last().copied().unwrap_or(declared);
                if written != provisional {
                    let mut interesting: Vec<TypeId> = Vec::new();
                    let nn = self.non_null_promotion(declared);
                    if nn != declared {
                        interesting.push(nn);
                    }
                    for &t in &m.tested {
                        interesting.push(t);
                        let nt = self.non_null_promotion(t);
                        interesting.push(nt);
                    }
                    interesting.retain(|t| *t != provisional);
                    interesting.dedup();
                    if interesting.contains(&written) {
                        chain.push(written);
                    } else {
                        let mut p3: Vec<TypeId> = Vec::new();
                        for &t in &interesting {
                            if self.sub(written, t) && self.sub(t, provisional) && !p3.contains(&t) {
                                p3.push(t);
                            }
                        }
                        let mut best = None;
                        for &t in &p3 {
                            if p3.iter().all(|&u| self.sub(t, u)) {
                                best = Some(t);
                                break;
                            }
                        }
                        if let Some(t) = best {
                            chain.push(t);
                        }
                    }
                }
            } else {
                chain.clear();
            }
            // `PromotionModel.write`: sem mudança na cadeia de uma variável já
            // atribuída, o modelo novo não leva histórico; senão leva o da
            // demoção. Os tipos testados caem quando a promoção some de vez.
            if chain == chain_before && assigned_before {
                m.history = None;
            } else {
                if chain.is_empty() && !chain_before.is_empty() {
                    m.tested.clear();
                }
                m.history = history;
            }
            m.cadeia = chain;
        }
        *flow_slot(flow, id) = Some(m);
    }
}

fn flow_slot(f: &mut Fluxo, id: LocalId) -> &mut Option<VarModel> {
    let i = id.0 as usize;
    if f.vars.len() <= i {
        f.vars.resize(i + 1, None);
    }
    &mut f.vars[i]
}

/// `joinPM`: cadeias pela maior subsequência comum, testados pela união. O
/// histórico só fica quando o resultado é um dos dois modelos
/// (`PromotionModel.join` devolve `first` ou `second` sem mudança).
fn join_model(a: &VarModel, b: &VarModel) -> VarModel {
    let mut r = join_model_without_history(a, b);
    let equal = |x: &VarModel, r: &VarModel| {
        x.cadeia == r.cadeia && x.tested == r.tested && x.assigned == r.assigned && x.unassigned == r.unassigned && x.captured == r.captured && x.versao == r.versao
    };
    r.history = if equal(a, &r) {
        a.history.clone()
    } else if equal(b, &r) {
        b.history.clone()
    } else {
        None
    };
    r
}

fn join_model_without_history(a: &VarModel, b: &VarModel) -> VarModel {
    let chain: Vec<TypeId> = a.cadeia.iter().copied().filter(|t| b.cadeia.contains(t)).collect();
    let mut tested = a.tested.clone();
    for t in &b.tested {
        if !tested.contains(t) {
            tested.push(*t);
        }
    }
    VarModel {
        cadeia: chain,
        tested,
        assigned: a.assigned && b.assigned,
        unassigned: a.unassigned && b.unassigned,
        captured: a.captured || b.captured,
        versao: if a.versao == b.versao { a.versao } else { new_version() },
        history: None,
    }
}
