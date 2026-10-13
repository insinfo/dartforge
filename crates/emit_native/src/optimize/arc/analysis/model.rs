//! Nominal identity and finite inclusion domain (§28.1).
//! Integration with the HIR producers/clones and SDK summaries is still needed.

use std::collections::BTreeSet;

/// Origin identity of an allocation, independent of the SSA ID of its copy.
/// The producer must preserve `origin` and remap specialization/context in the
/// clones. Building this key proves neither producer stability nor singleton.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::model::ArcSite;
/// let site = ArcSite { function: "create".into(), origin: 7, specialization: "int".into() };
/// assert_eq!(site.clone(), site);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArcSite {
    /// Nominal symbol of the origin body, not the address of the linked function.
    pub function: String,
    /// Persistent identifier assigned by the producer before cloning SSA.
    pub origin: u64,
    /// Generic instance/nominal specialization of the origin.
    pub specialization: String,
}

/// A possible heap node or a parameterized placeholder of a summary.
/// Parameters/globals are not fresh objects: the instantiation of summaries must
/// replace them with the real aliases. Contexts are keys bounded by the
/// producer; this domain does not explore an unbounded call stack.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::model::AbstractNode;
/// let arg = AbstractNode::Parameter { function: "link".into(), index: 0 };
/// assert_ne!(arg, AbstractNode::Global("root".into()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractNode {
    /// A site/context may represent many instances; it does not by itself allow
    /// erasing old field edges by strong update.
    Allocation {
        /// Persistent origin and generic instance.
        site: ArcSite,
        /// Abstract context bounded and versioned by the producer.
        context: String,
    },
    /// Placeholder of the argument, including the receiver when applicable.
    Parameter {
        /// Symbol of the summary that defines the argument.
        function: String,
        /// Position in the semantic ABI of the summary.
        index: u32,
    },
    /// Placeholder of an external result, without a freshness or separation hypothesis.
    ExternalResult(String),
    /// Placeholder of a global root, to be replaced by its possible reach.
    Global(String),
    /// Explicit origin of an environment, cell, frame or another synthetic unit.
    SyntheticUnit {
        /// Origin produced for the synthetic unit.
        site: ArcSite,
        /// Abstract context of the unit.
        context: String,
        /// Nominal role: do not confuse cell, closure and environment.
        role: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Points {
    KnownSet(BTreeSet<AbstractNode>),
    Top,
}

/// Bounded points-to set, with a top distinct from the empty set.
/// Null/trivial values may have an empty set. Missing facts, opaque
/// operations and an exceeded budget require top, which cannot be reduced by union.
/// The bound is immutable per set; options/versioning belong to the plan.
/// This domain does not certify cardinality, liveness, escape or ARC policies.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::model::{PointSet, AbstractNode};
/// let mut pt = PointSet::empty(1);
/// assert!(pt.include(AbstractNode::Global("a".into())));
/// assert!(pt.include(AbstractNode::Global("b".into())));
/// assert!(pt.nodes().is_none()); // an exceeded budget does not mean absence of aliases
/// assert!(!pt.join_set(&PointSet::empty(1)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointSet {
    limit: usize,
    points: Points,
}

impl PointSet {
    pub(super) fn internal_limit(&self) -> usize {
        self.limit
    }

    /// Creates the empty set; only use it when the absence of nodes is a fact.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analysis::model::PointSet;
    /// assert!(PointSet::empty(0).nodes().unwrap().is_empty());
    /// ```
    pub fn empty(limit: usize) -> Self {
        Self {
            limit,
            points: Points::KnownSet(BTreeSet::new()),
        }
    }

    /// Creates top for missing or invalidated information; never equivalent to empty.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analysis::model::PointSet;
    /// assert!(PointSet::unknown(8).nodes().is_none());
    /// ```
    pub fn unknown(limit: usize) -> Self {
        Self {
            limit,
            points: Points::Top,
        }
    }

    /// Queries all the known nodes, or `None` when there are unknown destinations.
    /// Immutable access prevents truncating or refining top outside the solver.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analysis::model::PointSet;
    /// assert_eq!(PointSet::empty(8).nodes().unwrap().len(), 0);
    /// ```
    pub fn nodes(&self) -> Option<&BTreeSet<AbstractNode>> {
        match &self.points {
            Points::KnownSet(nodes) => Some(nodes),
            Points::Top => None,
        }
    }

    /// Includes a node; when exceeding the bound, raises the whole set to top.
    /// Returns whether the state changed, to feed a monotonic worklist.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analysis::model::{PointSet, AbstractNode};
    /// let mut pt = PointSet::empty(0);
    /// assert!(pt.include(AbstractNode::Global("root".into())));
    /// assert!(!pt.include(AbstractNode::Global("other".into())));
    /// ```
    pub fn include(&mut self, node: AbstractNode) -> bool {
        let Points::KnownSet(nodes) = &mut self.points else {
            return false;
        };
        if nodes.contains(&node) {
            return false;
        }
        if nodes.len() == self.limit {
            self.points = Points::Top;
        } else {
            nodes.insert(node);
        }
        true
    }

    /// Joins possible information using the destination's budget, without truncation.
    /// Top is absorbing even when the origin has another bound. The boolean
    /// indicates a change without assigning a reason or proof to the resulting set.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analysis::model::PointSet;
    /// let mut pt = PointSet::empty(8);
    /// assert!(pt.join_set(&PointSet::unknown(0)));
    /// assert!(!pt.join_set(&PointSet::empty(100)));
    /// ```
    pub fn join_set(&mut self, other: &Self) -> bool {
        if matches!(self.points, Points::Top) {
            return false;
        }
        let Some(nodes) = other.nodes() else {
            self.points = Points::Top;
            return true;
        };
        let mut changed = false;
        for node in nodes {
            changed |= self.include(node.clone());
            if self.nodes().is_none() {
                break;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(mask: u8, limit: usize) -> PointSet {
        let mut pt = PointSet::empty(limit);
        for n in 0..3 {
            if mask & (1 << n) != 0 {
                pt.include(AbstractNode::Global(n.to_string()));
            }
        }
        pt
    }

    #[test]
    fn bounded_union_is_monotonic_commutative_associative_and_idempotent() {
        for limit in 0..=3 {
            for a in 0..8 {
                for b in 0..8 {
                    for c in 0..8 {
                        let x = set(a, limit);
                        let y = set(b, limit);
                        let z = set(c, limit);
                        let mut xy = x.clone();
                        xy.join_set(&y);
                        let mut yx = y.clone();
                        yx.join_set(&x);
                        assert_eq!(xy, yx);
                        assert_eq!(xy, set(a | b, limit));
                        assert!(!xy.join_set(&xy.clone()));
                        let mut left = xy;
                        left.join_set(&z);
                        let mut yz = y;
                        yz.join_set(&z);
                        let mut right = x;
                        right.join_set(&yz);
                        assert_eq!(left, right);
                    }
                }
            }
        }
    }

    #[test]
    fn unknown_never_becomes_empty_and_smaller_limit_does_not_truncate_aliases() {
        let mut pt = PointSet::empty(1);
        assert!(pt.join_set(&set(3, 3)));
        assert!(pt.nodes().is_none());
        assert!(!pt.join_set(&PointSet::empty(8)));
        let mut empty = PointSet::empty(8);
        assert!(empty.join_set(&PointSet::unknown(0)));
        assert!(empty.nodes().is_none());
    }

    #[test]
    fn placeholders_and_contexts_are_not_collapsed_by_width_or_name() {
        let site = ArcSite {
            function: "f".into(),
            origin: 1,
            specialization: "T".into(),
        };
        let mut pt = PointSet::empty(5);
        for node in [
            AbstractNode::Allocation {
                site: site.clone(),
                context: "a".into(),
            },
            AbstractNode::Allocation {
                site: site.clone(),
                context: "b".into(),
            },
            AbstractNode::Parameter {
                function: "f".into(),
                index: 1,
            },
            AbstractNode::ExternalResult("f".into()),
            AbstractNode::SyntheticUnit {
                site,
                context: "a".into(),
                role: "celula".into(),
            },
        ] {
            assert!(pt.include(node));
        }
        assert_eq!(pt.nodes().unwrap().len(), 5);
    }
}
