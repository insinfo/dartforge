//! Modelo de fluxo (`flow-analysis.md`): cadeias de promoção, tipos de
//! interesse, atribuição definitiva e alcançabilidade.
//!
//! Um [`Fluxo`] é o `FlowModel` da especificação; cada variável local tem um
//! [`ModeloVar`] (`PromotionModel`). As junções seguem `join`/`joinPM`; a
//! atribuição segue `demote` + `toi_promote`; a promoção por teste segue
//! `promote`.

use super::BodyInferrer;
use crate::resolved::LocalId;
use crate::table::{Type, TypeId};
use std::sync::atomic::{AtomicU32, Ordering};

/// Versões de escrita (o SSA do `flow-analysis.md`): cada escrita e cada
/// junção de versões diferentes ganha um número novo; só a igualdade importa.
static PROXIMA_VERSAO: AtomicU32 = AtomicU32::new(1);

fn nova_versao() -> u32 {
    PROXIMA_VERSAO.fetch_add(1, Ordering::Relaxed)
}

/// Estado de uma variável num ponto do programa (`PromotionModel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModeloVar {
    /// Cadeia de promoção: o último é o tipo atual.
    pub cadeia: Vec<TypeId>,
    /// Tipos de interesse (testados em algum caminho).
    pub testados: Vec<TypeId>,
    pub atribuida: bool,
    pub nao_atribuida: bool,
    /// Escrita por uma closure: não promove mais.
    pub capturada: bool,
    /// Versão da última escrita (0: declarada e nunca escrita).
    pub versao: u32,
}

/// Estado de fluxo num ponto (`FlowModel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fluxo {
    pub alcancavel: bool,
    pub vars: Vec<Option<ModeloVar>>,
}

impl Fluxo {
    pub fn alcancavel() -> Self {
        Fluxo { alcancavel: true, vars: Vec::new() }
    }

    pub fn inalcancavel(&self) -> Self {
        let mut f = self.clone();
        f.alcancavel = false;
        f
    }

    fn slot(&mut self, id: LocalId) -> &mut Option<ModeloVar> {
        let i = id.0 as usize;
        if self.vars.len() <= i {
            self.vars.resize(i + 1, None);
        }
        &mut self.vars[i]
    }

    pub fn declarar(&mut self, id: LocalId) {
        *self.slot(id) = Some(ModeloVar {
            cadeia: Vec::new(),
            testados: Vec::new(),
            atribuida: false,
            nao_atribuida: true,
            capturada: false,
            versao: 0,
        });
    }

    /// Declaração com valor (inicializador, parâmetro, variável de laço).
    pub fn inicializar(&mut self, id: LocalId) {
        if let Some(m) = self.slot(id) {
            m.atribuida = true;
            m.nao_atribuida = false;
            m.versao = nova_versao();
        }
    }

    /// Versão de escrita atual de uma variável.
    pub fn versao(&self, id: LocalId) -> Option<u32> {
        self.modelo(id).map(|m| m.versao)
    }

    pub fn modelo(&self, id: LocalId) -> Option<&ModeloVar> {
        self.vars.get(id.0 as usize).and_then(|m| m.as_ref())
    }

    pub fn atribuida(&self, id: LocalId) -> bool {
        self.modelo(id).is_some_and(|m| m.atribuida)
    }

    pub fn nao_atribuida(&self, id: LocalId) -> bool {
        self.modelo(id).is_some_and(|m| m.nao_atribuida)
    }

    /// Tipo atual (promovido) de uma variável.
    pub fn tipo_atual(&self, id: LocalId, declarado: TypeId) -> TypeId {
        self.modelo(id).and_then(|m| m.cadeia.last().copied()).unwrap_or(declarado)
    }

    pub fn capturar(&mut self, id: LocalId) {
        if let Some(m) = self.slot(id) {
            m.capturada = true;
            m.cadeia.clear();
            m.versao = nova_versao();
        }
    }

    /// Esquece promoções das variáveis dadas (junção conservadora de laços).
    pub fn juncao_conservadora(&mut self, escritas: &[LocalId], capturadas: &[LocalId]) {
        for &id in capturadas {
            if let Some(m) = self.slot(id) {
                m.cadeia.clear();
                m.nao_atribuida = false;
                m.capturada = true;
                m.versao = nova_versao();
            }
        }
        for &id in escritas {
            if let Some(m) = self.slot(id) {
                m.cadeia.clear();
                m.nao_atribuida = false;
                m.versao = nova_versao();
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
                (Some(x), Some(y)) => Some(juntar_modelo(x, y)),
                _ => None,
            });
        }
        Fluxo { alcancavel: true, vars }
    }

    /// Junta uma lista de modelos (vazia → inalcançável a partir de `base`).
    pub(crate) fn juntar_todos(&mut self, base: &Fluxo, lista: &[Fluxo]) -> Fluxo {
        let mut it = lista.iter();
        let Some(p) = it.next() else { return base.inalcancavel() };
        let mut acc = p.clone();
        for f in it {
            acc = self.juntar(&acc, f);
        }
        acc
    }

    /// `promote(x, T)`: promoção por teste de tipo (`is`, `as`, `!= null`).
    pub(crate) fn promover(&mut self, fluxo: &mut Fluxo, id: LocalId, declarado: TypeId, t: TypeId) {
        self.promover_testado(fluxo, id, declarado, t, t);
    }

    /// Promove para `t` registrando `testado` como tipo de interesse (o ramo
    /// falso de `x is T` testa `T` e promove para `factor(S, T)`;
    /// `_finishTypeTest` do analisador).
    pub(crate) fn promover_testado(&mut self, fluxo: &mut Fluxo, id: LocalId, declarado: TypeId, t: TypeId, testado: TypeId) {
        if !fluxo.alcancavel {
            return;
        }
        let Some(m) = fluxo.modelo(id).cloned() else { return };
        if m.capturada {
            return;
        }
        let s = m.cadeia.last().copied().unwrap_or(declarado);
        if self.sub(s, t) {
            // Já é subtipo: não promove, mas registra o tipo de interesse.
            if !m.testados.contains(&testado) {
                if let Some(Some(mm)) = fluxo.vars.get_mut(id.0 as usize) {
                    mm.testados.push(testado);
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
        if let Some(Some(mm)) = fluxo.vars.get_mut(id.0 as usize) {
            if !mm.testados.contains(&testado) {
                mm.testados.push(testado);
            }
            if let Some(t1) = t1 {
                mm.cadeia.push(t1);
            }
        }
        if let Some(t1) = t1 {
            if matches!(self.table.get(t1), Type::Never) {
                fluxo.alcancavel = false;
            }
        }
    }

    /// `promoteToNonNull(x)`.
    pub(crate) fn promover_nao_nulo(&mut self, fluxo: &mut Fluxo, id: LocalId, declarado: TypeId) {
        let atual = fluxo.tipo_atual(id, declarado);
        let nn = self.nao_nulo_promocao(atual);
        if nn != atual {
            self.promover(fluxo, id, declarado, nn);
        }
    }

    /// **NonNull**(`T`) para promoção (`T?` de variável de tipo vira `T`).
    pub(crate) fn nao_nulo_promocao(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Null => self.core.never,
            Type::FutureOr { arg, nullable: true } => self.table.intern(Type::FutureOr { arg, nullable: false }),
            // NonNull(X) = X & NonNull(B) quando o limite B é anulável
            // (promoteToNonNull do analyzer): T? vira T & Object.
            Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                let b = self.table.param(param).bound;
                let nb = self.nao_nulo_promocao(b);
                if nb != b && !matches!(self.table.get(b), Type::Dynamic) {
                    self.table.intern(Type::Intersection { param, bound: nb })
                } else {
                    self.table.intern(Type::TypeParameter { param, nullable: false })
                }
            }
            _ => self.nao_nulo(t),
        }
    }

    /// `rebaseForward` (§7.10): as promoções guardadas numa variável de
    /// condição valem de novo sobre o estado atual, para as variáveis que
    /// não foram escritas desde então (mesma versão).
    pub(crate) fn reaplicar(&mut self, atual: &Fluxo, guardado: &Fluxo) -> Fluxo {
        if !guardado.alcancavel {
            return atual.inalcancavel();
        }
        let mut r = atual.clone();
        for (i, g) in guardado.vars.iter().enumerate() {
            let Some(g) = g else { continue };
            let Some(Some(c)) = r.vars.get(i) else { continue };
            if g.versao != c.versao || c.capturada {
                continue;
            }
            let mut c = c.clone();
            for &t in &g.cadeia {
                if !c.cadeia.contains(&t) && c.cadeia.last().is_none_or(|&l| self.sub(t, l)) {
                    c.cadeia.push(t);
                }
            }
            for &t in &g.testados {
                if !c.testados.contains(&t) {
                    c.testados.push(t);
                }
            }
            r.vars[i] = Some(c);
        }
        r
    }

    /// `assign(x, T)`: demove e aplica a promoção por tipo de interesse.
    pub(crate) fn atribuir_fluxo(&mut self, fluxo: &mut Fluxo, id: LocalId, declarado: TypeId, escrito: TypeId) {
        self.escrever_fluxo(fluxo, id, declarado, escrito, true);
    }

    /// Escrita com ou sem promoção por tipo de interesse (a inicialização de
    /// uma variável `final` ou sem tipo escrito não promove, `_initialize`).
    pub(crate) fn escrever_fluxo(&mut self, fluxo: &mut Fluxo, id: LocalId, declarado: TypeId, escrito: TypeId, toi: bool) {
        let Some(m) = fluxo.modelo(id).cloned() else { return };
        let mut m = m;
        m.atribuida = true;
        m.nao_atribuida = false;
        m.versao = nova_versao();
        if !m.capturada {
            // Escrita de `dynamic` num local tipado é cast implícito: o tipo
            // escrito efetivo é o declarado.
            let escrito = if self.e_dynamic(escrito) && !self.e_dynamic(declarado) { declarado } else { escrito };
            // demote
            let mut cadeia: Vec<TypeId> = Vec::new();
            for &t in &m.cadeia {
                if self.sub(escrito, t) {
                    cadeia.push(t);
                }
            }
            // toi_promote
            if !toi {
                if !self.sub(escrito, declarado) {
                    cadeia.clear();
                }
            } else if self.sub(escrito, declarado) {
                let provisorio = cadeia.last().copied().unwrap_or(declarado);
                if escrito != provisorio {
                    let mut interesse: Vec<TypeId> = Vec::new();
                    let nn = self.nao_nulo_promocao(declarado);
                    if nn != declarado {
                        interesse.push(nn);
                    }
                    for &t in &m.testados {
                        interesse.push(t);
                        let nt = self.nao_nulo_promocao(t);
                        interesse.push(nt);
                    }
                    interesse.retain(|t| *t != provisorio);
                    interesse.dedup();
                    if interesse.contains(&escrito) {
                        cadeia.push(escrito);
                    } else {
                        let mut p3: Vec<TypeId> = Vec::new();
                        for &t in &interesse {
                            if self.sub(escrito, t) && self.sub(t, provisorio) && !p3.contains(&t) {
                                p3.push(t);
                            }
                        }
                        let mut melhor = None;
                        for &t in &p3 {
                            if p3.iter().all(|&u| self.sub(t, u)) {
                                melhor = Some(t);
                                break;
                            }
                        }
                        if let Some(t) = melhor {
                            cadeia.push(t);
                        }
                    }
                }
            } else {
                cadeia.clear();
            }
            m.cadeia = cadeia;
        }
        *fluxo_slot(fluxo, id) = Some(m);
    }
}

fn fluxo_slot(f: &mut Fluxo, id: LocalId) -> &mut Option<ModeloVar> {
    let i = id.0 as usize;
    if f.vars.len() <= i {
        f.vars.resize(i + 1, None);
    }
    &mut f.vars[i]
}

/// `joinPM`: cadeias pela maior subsequência comum, testados pela união.
fn juntar_modelo(a: &ModeloVar, b: &ModeloVar) -> ModeloVar {
    let cadeia: Vec<TypeId> = a.cadeia.iter().copied().filter(|t| b.cadeia.contains(t)).collect();
    let mut testados = a.testados.clone();
    for t in &b.testados {
        if !testados.contains(t) {
            testados.push(*t);
        }
    }
    ModeloVar {
        cadeia,
        testados,
        atribuida: a.atribuida && b.atribuida,
        nao_atribuida: a.nao_atribuida && b.nao_atribuida,
        capturada: a.capturada || b.capturada,
        versao: if a.versao == b.versao { a.versao } else { nova_versao() },
    }
}
