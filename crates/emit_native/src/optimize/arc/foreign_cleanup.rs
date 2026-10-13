//! Certificate of the inventory used by the foreign cleanup of landing pads.
use super::*;
use std::collections::{HashMap, HashSet};

/// Cleanup inventory bound to the body and the tables that were verified.
///
/// Created by the ARC preparation of the function set; private fields avoid
/// inventing owners in the emitter. Later changes require a new preparation.
/// Local frames follow the validated LIFO stack; aliases and imported frames
/// stay outside the verified protocol.
///
/// ```
/// use dartforge_emit_native::optimize::arc::DartFunctionPlan;
/// let plan = DartFunctionPlan::default();
/// assert!(plan.tables.foreign_cleanup.is_none());
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
