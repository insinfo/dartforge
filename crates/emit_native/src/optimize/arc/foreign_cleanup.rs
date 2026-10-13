//! Certificado do inventário usado pelo cleanup estrangeiro dos pousos.
use super::*;
use std::collections::{HashMap, HashSet};

/// Inventário de cleanup ligado ao corpo e às tabelas que foram verificados.
///
/// Criado pela preparação ARC do conjunto de funções; campos privados evitam
/// inventar owners no emissor. Mudanças posteriores exigem nova preparação.
/// Quadros locais acompanham a pilha LIFO validada; aliases e quadros
/// importados continuam fora do protocolo verificado.
///
/// ```
/// use dartforge_emit_native::optimize::arc::PlanoFuncaoDart;
/// let plano = PlanoFuncaoDart::default();
/// assert!(plano.tabelas.cleanup_estrangeiro.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignCleanup {
    body: String,
    invocations: HashMap<ValueId, BlockId>,
    landing_pads: HashSet<BlockId>,
    exits: HashMap<BlockId, ExceptionExit>,
    checks_stack: bool,
    owners: HashMap<BlockId, Vec<ValueId>>,
    frames: HashMap<BlockId, Vec<ValueId>>,
}

impl ForeignCleanup {
    pub(super) fn new(f: &Function, plan: &DartFunctionPlan) -> Result<Option<Self>, String> {
        if !plan
            .tables
            .landing_pads
            .iter()
            .any(|b| plan.tables.exits.get(b) != Some(&ExceptionExit::Resume))
        {
            return Ok(None);
        }
        let frames = super::frames::at_landing_pad_entry(f, &plan.tables)?;
        Ok(Some(Self {
            body: format!("{f:?}"),
            invocations: plan.tables.invocations.clone(),
            landing_pads: plan.tables.landing_pads.clone(),
            exits: plan.tables.exits.clone(),
            checks_stack: plan.tables.checks_stack,
            owners: plan.owners_at_landing_pad.clone(),
            frames,
        }))
    }

    pub(crate) fn check(&self, f: &Function, t: &FunctionTables) -> Result<(), String> {
        if self.body != format!("{f:?}")
            || self.invocations != t.invocations
            || self.landing_pads != t.landing_pads
            || self.exits != t.exits
            || self.checks_stack != t.checks_stack
        {
            return Err(format!("cleanup estrangeiro obsoleto em {}", f.symbol));
        }
        Ok(())
    }

    pub(crate) fn owners(&self, b: BlockId) -> &[ValueId] {
        self.owners
            .get(&b)
            .expect("pouso sem inventário certificado")
    }
    pub(crate) fn frames(&self, b: BlockId) -> &[ValueId] {
        self.frames
            .get(&b)
            .expect("pouso sem inventário de quadros certificado")
    }
}
