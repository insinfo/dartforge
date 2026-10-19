//! Per-field inclusion solver, without strong update (§28.2).
//! Consumes explicit constraints, still without HIR extraction/SDK summaries.
//! The absence of a constraint does not certify the absence of program effects.

use super::model::{PointSet, AbstractNode};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

/// Nominal field of a layout; equal indices of distinct layouts do not coincide.
/// The version/validation of the layout belongs to the producer of the constraints.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::points_to::ArcField;
/// let field = ArcField { layout: "No:v1".into(), position: 0 };
/// assert_ne!(field.layout, "Other:v1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArcField {
    /// Nominal identity and version of the physical/semantic schema.
    pub layout: String,
    /// Position in the schema, not a dynamic index without a bounds proof.
    pub position: u32,
}

/// Alias inclusion constraint. Variables are local indices of the solver,
/// not persistent site identities. The producer must record initial/external
/// values, every write and the relevant unknowns.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::points_to::Constraint;
/// let copy = Constraint::Copy { origin: 0, dest: 1 };
/// assert!(matches!(copy, Constraint::Copy { .. }));
/// ```
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Includes a possible nominal origin in the variable.
    Seed { dest: usize, node: AbstractNode },
    /// An opaque value/invalidated information, never the absence of aliases.
    Unknown { dest: usize },
    /// Copy, move, reference cast or a Phi entry.
    Copy { origin: usize, dest: usize },
    /// Includes destinations in every possible receiver; never erases edges.
    Write {
        object: usize,
        field: ArcField,
        value: usize,
    },
    /// Includes every possible destination of the field in each receiver.
    Read {
        object: usize,
        field: ArcField,
        dest: usize,
    },
    /// Read by physical index, selecting the nominal field of each node.
    /// Schemas are local premises of the producer. A receiver without a schema/index
    /// or empty at the end of the solution requires top and invalidates the known fields:
    /// the error/helper path has no heap summary yet.
    ReadBySchema {
        object: usize,
        schemas: Arc<BTreeMap<AbstractNode, Vec<ArcField>>>,
        index: usize,
        dest: usize,
    },
    /// Weak write by schema; never removes the previous destinations.
    /// An uncovered boundary invalidates every field supplied in the schemas,
    /// including objects discovered later, without assuming a singleton receiver.
    WriteBySchema {
        object: usize,
        schemas: Arc<BTreeMap<AbstractNode, Vec<ArcField>>>,
        index: usize,
        value: usize,
    },
    /// An opaque writer may touch any receiver of that field/layout.
    /// It does not make fields of other layouts opaque; additional effects require
    /// other constraints, including when there is reentry/transitivity.
    UnknownField { field: ArcField },
}

/// Fixpoint of the given constraints, without a policy certificate.
/// It does not represent the program heap while the producer does not cover every
/// operation and instantiate the summary placeholders correctly.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::points_to::{solve, Constraint};
/// let r = solve(1, 8, &[Constraint::Unknown { dest: 0 }])?;
/// assert!(r.variable(0).unwrap().nodes().is_none());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct PointsToResult {
    variables: Vec<PointSet>,
    fields: BTreeMap<(AbstractNode, ArcField), PointSet>,
    opaque_fields: BTreeSet<ArcField>,
}

impl PointsToResult {
    /// Queries a variable; a nonexistent index returns `None`, without an empty fact.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analysis::points_to::solve;
    /// assert!(solve(0, 8, &[])?.variable(0).is_none());
    /// # Ok::<(), String>(())
    /// ```
    pub fn variable(&self, id: usize) -> Option<&PointSet> {
        self.variables.get(id)
    }

    /// Queries the materialized field, or `None` for an absent/opaque field.
    /// `None` never certifies an empty field: missing effects/initial values
    /// must be supplied by the producer before using the result.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analysis::{model::AbstractNode, points_to::*};
    /// let r = solve(0, 8, &[])?;
    /// assert!(r.field(&AbstractNode::Global("x".into()), &ArcField { layout: "L".into(), position: 0 }).is_none());
    /// # Ok::<(), String>(())
    /// ```
    pub fn field(&self, node: &AbstractNode, field: &ArcField) -> Option<&PointSet> {
        if self.opaque_fields.contains(field) {
            None
        } else {
            self.fields.get(&(node.clone(), field.clone()))
        }
    }
}

/// Solves monotonic constraints with a worklist with variable/field dependencies.
/// A write through an unknown receiver makes the field opaque for every node,
/// including nodes discovered later. The budget is per set; exceeding it propagates
/// top to readers/copies. The queue only schedules dependents of changed facts.
/// Variables/fields start at the bottom of the inclusion system: the producer must
/// supply initialization/extern facts; this does not infer an initial zero of the heap.
///
/// # Errors
/// A variable index outside the declared domain or incompatible positions
/// in the schema of a selected access. Validation precedes the solution.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::{model::AbstractNode, points_to::*};
/// let r = solve(2, 4, &[
///     Constraint::Copy { origin: 0, dest: 1 },
///     Constraint::Seed { dest: 0, node: AbstractNode::Global("root".into()) },
/// ])?;
/// assert_eq!(r.variable(0), r.variable(1));
/// # Ok::<(), String>(())
/// ```
pub fn solve(
    n: usize,
    limit: usize,
    constraints: &[Constraint],
) -> Result<PointsToResult, String> {
    let mut dependents = vec![Vec::new(); n];
    let mut readers: BTreeMap<ArcField, Vec<usize>> = BTreeMap::new();
    for (id, r) in constraints.iter().enumerate() {
        let (uses, dest) = match r {
            Constraint::Seed { dest, .. } | Constraint::Unknown { dest } => {
                (vec![], Some(*dest))
            }
            Constraint::Copy { origin, dest } => (vec![*origin], Some(*dest)),
            Constraint::Write { object, value, .. }
            | Constraint::WriteBySchema { object, value, .. } => (vec![*object, *value], None),
            Constraint::Read {
                object,
                field,
                dest,
            } => {
                readers.entry(field.clone()).or_default().push(id);
                (vec![*object], Some(*dest))
            }
            Constraint::UnknownField { .. } => (vec![], None),
            Constraint::ReadBySchema {
                object,
                schemas,
                index,
                dest,
            } => {
                for field in schemas.values().filter_map(|cs| cs.get(*index)) {
                    readers.entry(field.clone()).or_default().push(id);
                }
                (vec![*object], Some(*dest))
            }
        };
        if let Constraint::ReadBySchema { schemas, .. }
        | Constraint::WriteBySchema { schemas, .. } = r
            && schemas
                .values()
                .any(|cs| cs.iter().enumerate().any(|(i, c)| c.position as usize != i))
        {
            return Err(format!("points-to: incompatible schema in constraint {id}"));
        }
        if uses.iter().chain(dest.iter()).any(|&v| v >= n) {
            return Err(format!(
                "points-to: variable outside the domain in constraint {id}"
            ));
        }
        for v in uses {
            dependents[v].push(id);
        }
    }
    let mut result = PointsToResult {
        variables: vec![PointSet::empty(limit); n],
        fields: BTreeMap::new(),
        opaque_fields: BTreeSet::new(),
    };
    let mut queue: VecDeque<_> = (0..constraints.len()).collect();
    let mut scheduled = vec![true; constraints.len()];
    loop {
        while let Some(id) = queue.pop_front() {
            scheduled[id] = false;
            let r = &constraints[id];
            let mut changed_variable = None;
            let mut changed_fields = BTreeSet::new();
            match r {
                Constraint::Seed { dest, node } => {
                    if result.variables[*dest].include(node.clone()) {
                        changed_variable = Some(*dest);
                    }
                }
                Constraint::Unknown { dest } => {
                    if result.variables[*dest].join_set(&PointSet::unknown(limit)) {
                        changed_variable = Some(*dest);
                    }
                }
                Constraint::Copy { origin, dest } => {
                    let origin = result.variables[*origin].clone();
                    if result.variables[*dest].join_set(&origin) {
                        changed_variable = Some(*dest);
                    }
                }
                Constraint::Write {
                    object,
                    field,
                    value,
                } => {
                    if let Some(nodes) = result.variables[*object].nodes() {
                        let value = &result.variables[*value];
                        for node in nodes {
                            let dests = result
                                .fields
                                .entry((node.clone(), field.clone()))
                                .or_insert_with(|| PointSet::empty(limit));
                            if dests.join_set(value) {
                                changed_fields.insert(field);
                            }
                        }
                    } else if result.opaque_fields.insert(field.clone()) {
                        changed_fields.insert(field);
                    }
                }
                Constraint::UnknownField { field } => {
                    if result.opaque_fields.insert(field.clone()) {
                        changed_fields.insert(field);
                    }
                }
                Constraint::Read {
                    object,
                    field,
                    dest,
                } => {
                    let mut value = PointSet::empty(limit);
                    if let Some(nodes) = result.variables[*object]
                        .nodes()
                        .filter(|_| !result.opaque_fields.contains(field))
                    {
                        for node in nodes {
                            if let Some(c) = result.fields.get(&(node.clone(), field.clone())) {
                                value.join_set(c);
                            }
                        }
                    } else {
                        value = PointSet::unknown(limit);
                    }
                    if result.variables[*dest].join_set(&value) {
                        changed_variable = Some(*dest);
                    }
                }
                Constraint::ReadBySchema {
                    object,
                    schemas,
                    index,
                    dest,
                }
                | Constraint::WriteBySchema {
                    object,
                    schemas,
                    index,
                    value: dest,
                } => {
                    let nodes = result.variables[*object].nodes();
                    let boundary = nodes.is_none_or(|ns| {
                        ns.iter()
                            .any(|node| schemas.get(node).and_then(|cs| cs.get(*index)).is_none())
                    });
                    if boundary {
                        for field in schemas.values().flatten() {
                            if result.opaque_fields.insert(field.clone()) {
                                changed_fields.insert(field);
                            }
                        }
                    }
                    if matches!(r, Constraint::ReadBySchema { .. }) {
                        let mut value = if boundary {
                            PointSet::unknown(limit)
                        } else {
                            PointSet::empty(limit)
                        };
                        if let Some(nodes) = nodes {
                            for node in nodes {
                                if let Some(field) = schemas.get(node).and_then(|cs| cs.get(*index))
                                {
                                    if result.opaque_fields.contains(field) {
                                        value.join_set(&PointSet::unknown(limit));
                                    } else if let Some(c) =
                                        result.fields.get(&(node.clone(), field.clone()))
                                    {
                                        value.join_set(c);
                                    }
                                }
                            }
                        }
                        if result.variables[*dest].join_set(&value) {
                            changed_variable = Some(*dest);
                        }
                    } else if !boundary && let Some(nodes) = nodes {
                        let value = &result.variables[*dest];
                        for node in nodes {
                            let field = &schemas[node][*index];
                            if result
                                .fields
                                .entry((node.clone(), field.clone()))
                                .or_insert_with(|| PointSet::empty(limit))
                                .join_set(value)
                            {
                                changed_fields.insert(field);
                            }
                        }
                    }
                }
            }
            let events = changed_variable
                .into_iter()
                .flat_map(|v| dependents[v].iter())
                .chain(
                    changed_fields
                        .into_iter()
                        .flat_map(|c| readers.get(c).into_iter().flatten()),
                );
            for &next_item in events {
                if !scheduled[next_item] {
                    scheduled[next_item] = true;
                    queue.push_back(next_item);
                }
            }
        }
        // An intermediate bottom may receive late aliases. Only after
        // emptying the queue does a still empty receiver stop being a covered access:
        // null/absence of an object does not prove the absence of the helper's effects.
        let mut events: BTreeSet<usize> = BTreeSet::new();
        for r in constraints {
            let (object, schemas, dest) = match r {
                Constraint::ReadBySchema {
                    object,
                    schemas,
                    dest,
                    ..
                } => (*object, schemas, Some(*dest)),
                Constraint::WriteBySchema {
                    object, schemas, ..
                } => (*object, schemas, None),
                _ => continue,
            };
            if result.variables[object]
                .nodes()
                .is_some_and(|ns| ns.is_empty())
            {
                if let Some(dest) = dest
                    && result.variables[dest].join_set(&PointSet::unknown(limit))
                {
                    events.extend(dependents[dest].iter().copied());
                }
                for field in schemas.values().flatten() {
                    if result.opaque_fields.insert(field.clone()) {
                        events.extend(readers.get(field).into_iter().flatten().copied());
                    }
                }
            }
        }
        if events.is_empty() {
            break;
        }
        for id in events {
            if !scheduled[id] {
                scheduled[id] = true;
                queue.push_back(id);
            }
        }
    }
    verify(n, limit, constraints, &result)?;
    Ok(result)
}

/// Independently checks whether the solution covers every given constraint.
/// Does not run the worklist nor require the least fixpoint: over-approximations are
/// valid, top included. Checks indices, budgets and the inclusion of each edge.
/// Does not prove that the producer covered the HIR, initializations, observers or the SDK;
/// it is not a certificate of lifetime, singleton or ARC policy.
///
/// # Errors
/// Incompatible domain/budget, invalid index or an unsatisfied constraint.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::{model::*, points_to::*};
/// let rs = vec![Constraint::Seed { dest: 0, node: AbstractNode::Global("g".into()) },
///     Constraint::Copy { origin: 0, dest: 1 }];
/// let r = solve(2, 8, &rs)?;
/// verify(2, 8, &rs, &r)?;
/// assert!(verify(3, 8, &rs, &r).is_err());
/// # Ok::<(), String>(())
/// ```
pub fn verify(
    n: usize,
    limit: usize,
    constraints: &[Constraint],
    r: &PointsToResult,
) -> Result<(), String> {
    if r.variables.len() != n {
        return Err("points-to verifier: incompatible domain".into());
    }
    if r.variables
        .iter()
        .chain(r.fields.values())
        .any(|p| p.internal_limit() != limit || p.nodes().is_some_and(|nodes| nodes.len() > limit))
    {
        return Err("points-to verifier: incompatible budget".into());
    }
    for (id, constraint) in constraints.iter().enumerate() {
        let variable = |v: usize| {
            r.variables
                .get(v)
                .ok_or_else(|| format!("points-to verifier: index in constraint {id}"))
        };
        let valid = match constraint {
            Constraint::Seed { dest, node } => {
                variable(*dest)?.nodes().is_none_or(|nodes| nodes.contains(node))
            }
            Constraint::Unknown { dest } => variable(*dest)?.nodes().is_none(),
            Constraint::Copy { origin, dest } => {
                includes(variable(*dest)?, variable(*origin)?)
            }
            Constraint::UnknownField { field } => r.opaque_fields.contains(field),
            Constraint::Write {
                object,
                field,
                value,
            } => {
                let receiver = variable(*object)?;
                let value = variable(*value)?;
                match receiver.nodes() {
                    None => r.opaque_fields.contains(field),
                    Some(nodes) => {
                        r.opaque_fields.contains(field)
                            || nodes.iter().all(|node| {
                                r.fields
                                    .get(&(node.clone(), field.clone()))
                                    .is_some_and(|p| includes(p, value))
                            })
                    }
                }
            }
            Constraint::Read {
                object,
                field,
                dest,
            } => {
                let receiver = variable(*object)?;
                let dest = variable(*dest)?;
                if r.opaque_fields.contains(field) || receiver.nodes().is_none() {
                    dest.nodes().is_none()
                } else {
                    receiver.nodes().unwrap().iter().all(|node| {
                        // Absent is bottom only in the constraint system; it does not
                        // prove an empty initialization of a field of the real heap.
                        r.fields
                            .get(&(node.clone(), field.clone()))
                            .is_none_or(|p| includes(dest, p))
                    })
                }
            }
            Constraint::ReadBySchema {
                object,
                schemas,
                index,
                dest,
            }
            | Constraint::WriteBySchema {
                object,
                schemas,
                index,
                value: dest,
            } => {
                if schemas
                    .values()
                    .any(|cs| cs.iter().enumerate().any(|(i, c)| c.position as usize != i))
                {
                    return Err(format!("points-to verifier: schema in constraint {id}"));
                }
                let receiver = variable(*object)?;
                let value = variable(*dest)?;
                let boundary = receiver.nodes().is_none_or(|ns| {
                    ns.is_empty()
                        || ns
                            .iter()
                            .any(|node| schemas.get(node).and_then(|cs| cs.get(*index)).is_none())
                });
                if boundary {
                    schemas
                        .values()
                        .flatten()
                        .all(|c| r.opaque_fields.contains(c))
                        && (!matches!(constraint, Constraint::ReadBySchema { .. })
                            || value.nodes().is_none())
                } else {
                    receiver.nodes().unwrap().iter().all(|node| {
                        let field = &schemas[node][*index];
                        if matches!(constraint, Constraint::ReadBySchema { .. }) {
                            if r.opaque_fields.contains(field) {
                                value.nodes().is_none()
                            } else {
                                r.fields
                                    .get(&(node.clone(), field.clone()))
                                    .is_none_or(|c| includes(value, c))
                            }
                        } else {
                            r.opaque_fields.contains(field)
                                || r.fields
                                    .get(&(node.clone(), field.clone()))
                                    .is_some_and(|c| includes(c, value))
                        }
                    })
                }
            }
        };
        if !valid {
            return Err(format!(
                "points-to verifier: constraint {id} not satisfied"
            ));
        }
    }
    Ok(())
}

fn includes(dest: &PointSet, origin: &PointSet) -> bool {
    match (dest.nodes(), origin.nodes()) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(d), Some(o)) => o.is_subset(d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_reads_match_walks_in_343_graphs() {
        let schemas = Arc::new(
            (0..3)
                .map(|i| (node(&format!("n{i}")), vec![field(&format!("L{i}"))]))
                .collect::<BTreeMap<_, _>>(),
        );
        let mut checked_set = 0;
        for mask in 0..512usize {
            // Each node has an exit: the boundary tests treat null/empty
            // receivers separately, without assuming they are pure reads.
            if (0..3).any(|v| mask & (7 << (3 * v)) == 0) {
                continue;
            }
            checked_set += 1;
            let mut rs = Vec::new();
            for step in 0..7 {
                rs.push(Constraint::ReadBySchema {
                    object: if step == 0 { 0 } else { 2 + step },
                    schemas: schemas.clone(),
                    index: 0,
                    dest: 3 + step,
                });
            }
            for v in 0..3 {
                for w in 0..3 {
                    if mask & (1 << (3 * v + w)) != 0 {
                        rs.push(Constraint::Write {
                            object: v,
                            field: field(&format!("L{v}")),
                            value: w,
                        });
                    }
                }
                rs.push(Constraint::Seed {
                    dest: v,
                    node: node(&format!("n{v}")),
                });
            }
            // Oracle of walks of exact length in the original graph;
            // it does not use constraints, the queue, the solver's schema or its verifier.
            let mut current_ones = BTreeSet::from([0]);
            let mut expected_set = Vec::new();
            for _ in 0..7 {
                current_ones = (0..3)
                    .filter(|&w| current_ones.iter().any(|&v| mask & (1 << (3 * v + w)) != 0))
                    .collect();
                expected_set.push(
                    current_ones
                        .iter()
                        .map(|i| node(&format!("n{i}")))
                        .collect::<BTreeSet<_>>(),
                );
            }
            for _ in 0..2 {
                let r = solve(10, 8, &rs).unwrap();
                for (step, expected) in expected_set.iter().enumerate() {
                    assert_eq!(
                        r.variable(3 + step).unwrap().nodes().unwrap(),
                        expected,
                        "grafo {mask}, passo {step}"
                    );
                }
                assert!(r.opaque_fields.is_empty());
                rs.reverse();
            }
        }
        assert_eq!(checked_set, 343);
    }

    #[test]
    fn schema_selection_preserves_layouts_weak_write_and_late_aliases() {
        let schemas = Arc::new(BTreeMap::from([
            (node("a"), vec![field("A")]),
            (node("b"), vec![field("B")]),
        ]));
        let mut rs = vec![
            Constraint::ReadBySchema {
                object: 0,
                schemas: schemas.clone(),
                index: 0,
                dest: 3,
            },
            Constraint::ReadBySchema {
                object: 6,
                schemas: schemas.clone(),
                index: 0,
                dest: 7,
            },
            Constraint::WriteBySchema {
                object: 0,
                schemas,
                index: 0,
                value: 4,
            },
            Constraint::Write {
                object: 5,
                field: field("A"),
                value: 1,
            },
            Constraint::Write {
                object: 6,
                field: field("B"),
                value: 2,
            },
            // Same index, another layout: this edge is not read in b.B.
            Constraint::Write {
                object: 6,
                field: field("A"),
                value: 1,
            },
            Constraint::Seed {
                dest: 5,
                node: node("a"),
            },
            Constraint::Seed {
                dest: 6,
                node: node("b"),
            },
            Constraint::Copy {
                origin: 5,
                dest: 0,
            },
            Constraint::Copy {
                origin: 6,
                dest: 0,
            },
            Constraint::Seed {
                dest: 1,
                node: node("x"),
            },
            Constraint::Seed {
                dest: 2,
                node: node("y"),
            },
            Constraint::Seed {
                dest: 4,
                node: node("z"),
            },
        ];
        for _ in 0..2 {
            let mut r = solve(8, 8, &rs).unwrap();
            assert_eq!(
                r.variable(3).unwrap().nodes().unwrap(),
                &BTreeSet::from([node("x"), node("y"), node("z")])
            );
            assert_eq!(
                r.variable(7).unwrap().nodes().unwrap(),
                &BTreeSet::from([node("y"), node("z")])
            );
            assert!(r.opaque_fields.is_empty());
            r.variables[3] = PointSet::empty(8);
            assert!(verify(8, 8, &rs, &r).is_err());
            r = solve(8, 8, &rs).unwrap();
            r.fields
                .insert((node("b"), field("B")), PointSet::empty(8));
            assert!(verify(8, 8, &rs, &r).is_err());
            rs.reverse();
        }
    }

    #[test]
    fn schema_selection_closes_boundaries_and_verifier_rejects_erased_opacity() {
        let schemas = Arc::new(BTreeMap::from([(node("a"), vec![field("A")])]));
        for case in 0..5 {
            let mut rs = vec![Constraint::ReadBySchema {
                object: 0,
                schemas: schemas.clone(),
                index: if case == 3 { 1 } else { 0 },
                dest: 1,
            }];
            match case {
                0 => {} // An empty receiver is only closed after the aliases converge.
                1 => rs.push(Constraint::Unknown { dest: 0 }),
                2 => rs.push(Constraint::Seed {
                    dest: 0,
                    node: node("sem esquema"),
                }),
                3 | 4 => rs.push(Constraint::Seed {
                    dest: 0,
                    node: node("a"),
                }),
                _ => unreachable!(),
            }
            let limit = if case == 4 { 0 } else { 8 };
            let mut r = solve(2, limit, &rs).unwrap();
            assert!(r.variable(1).unwrap().nodes().is_none(), "caso {case}");
            assert!(r.opaque_fields.contains(&field("A")));
            r.opaque_fields.clear();
            assert!(verify(2, limit, &rs, &r).is_err());
        }
        let rs = [Constraint::WriteBySchema {
            object: 0,
            schemas,
            index: 0,
            value: 1,
        }];
        let mut r = solve(2, 8, &rs).unwrap();
        assert!(r.opaque_fields.contains(&field("A")));
        r.opaque_fields.clear();
        assert!(verify(2, 8, &rs, &r).is_err());
        let invalid = Arc::new(BTreeMap::from([(
            node("a"),
            vec![ArcField {
                layout: "A".into(),
                position: 9,
            }],
        )]));
        assert!(
            solve(
                2,
                8,
                &[Constraint::ReadBySchema {
                    object: 0,
                    schemas: invalid,
                    index: 0,
                    dest: 1
                }]
            )
            .is_err()
        );
    }
    #[test]
    fn verifier_rejects_erased_truncation_copy_and_unknown() {
        let rs = vec![
            Constraint::Seed {
                dest: 0,
                node: node("a"),
            },
            Constraint::Copy {
                origin: 0,
                dest: 1,
            },
            Constraint::Unknown { dest: 2 },
        ];
        let mut r = solve(3, 8, &rs).unwrap();
        r.variables[1] = PointSet::empty(8);
        assert!(verify(3, 8, &rs, &r).is_err());
        r = solve(3, 8, &rs).unwrap();
        r.variables[0] = PointSet::empty(8);
        assert!(verify(3, 8, &rs, &r).is_err());
        r = solve(3, 8, &rs).unwrap();
        r.variables[2] = PointSet::empty(8);
        assert!(verify(3, 8, &rs, &r).is_err());
        r = solve(3, 8, &rs).unwrap();
        assert!(verify(3, 7, &rs, &r).is_err());
        assert!(verify(2, 8, &rs, &r).is_err());
        assert!(
            verify(
                3,
                8,
                &[Constraint::Copy {
                    origin: 99,
                    dest: 0
                }],
                &r
            )
            .is_err()
        );
        // Accept a safe over-approximation without depending on equality with the solver.
        r.variables.fill(PointSet::unknown(8));
        verify(3, 8, &rs, &r).unwrap();
    }

    #[test]
    fn verifier_rejects_omitted_store_read_and_opacity_edges() {
        let rs = vec![
            Constraint::Seed {
                dest: 0,
                node: node("obj"),
            },
            Constraint::Seed {
                dest: 1,
                node: node("filho"),
            },
            Constraint::Write {
                object: 0,
                field: field("L"),
                value: 1,
            },
            Constraint::Read {
                object: 0,
                field: field("L"),
                dest: 2,
            },
        ];
        let mut r = solve(3, 8, &rs).unwrap();
        r.fields.clear();
        assert!(verify(3, 8, &rs, &r).is_err());
        r = solve(3, 8, &rs).unwrap();
        r.fields
            .insert((node("obj"), field("L")), PointSet::empty(8));
        assert!(verify(3, 8, &rs, &r).is_err());
        r = solve(3, 8, &rs).unwrap();
        r.variables[2] = PointSet::empty(8);
        assert!(verify(3, 8, &rs, &r).is_err());
        let mut opaque_set = rs;
        opaque_set.push(Constraint::UnknownField { field: field("L") });
        r = solve(3, 8, &opaque_set).unwrap();
        r.opaque_fields.clear();
        assert!(verify(3, 8, &opaque_set, &r).is_err());
    }

    #[test]
    fn top_receiver_requires_global_opacity_for_future_receivers() {
        let rs = vec![
            Constraint::Unknown { dest: 0 },
            Constraint::Write {
                object: 0,
                field: field("L"),
                value: 1,
            },
            Constraint::Seed {
                dest: 2,
                node: node("tardio"),
            },
            Constraint::Read {
                object: 2,
                field: field("L"),
                dest: 3,
            },
        ];
        let mut r = solve(4, 8, &rs).unwrap();
        r.opaque_fields.clear();
        assert!(verify(4, 8, &rs, &r).is_err());
    }

    #[test]
    fn inclusion_matches_transitive_closure_in_all_three_variable_graphs() {
        for mask in 0..512 {
            let mut rs = Vec::new();
            let mut reachability = [[false; 3]; 3];
            for (v, row) in reachability.iter_mut().enumerate() {
                row[v] = true;
                rs.push(Constraint::Seed {
                    dest: v,
                    node: node(&v.to_string()),
                });
                for (w, reachable) in row.iter_mut().enumerate() {
                    if mask & (1 << (3 * v + w)) != 0 {
                        *reachable = true;
                        rs.push(Constraint::Copy {
                            origin: v,
                            dest: w,
                        });
                    }
                }
            }
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        reachability[v][w] |= reachability[v][k] && reachability[k][w];
                    }
                }
            }
            for limit in 1..=3 {
                let r = solve(3, limit, &rs).unwrap();
                for (w, _) in reachability[0].iter().enumerate() {
                    let expected: BTreeSet<_> = (0..3)
                        .filter(|&v| reachability[v][w])
                        .map(|v| node(&v.to_string()))
                        .collect();
                    let obtained = r.variable(w).unwrap().nodes();
                    if expected.len() > limit {
                        assert!(obtained.is_none());
                    } else {
                        assert_eq!(obtained.unwrap(), &expected, "grafo {mask}");
                    }
                }
            }
        }
    }
    fn node(n: &str) -> AbstractNode {
        AbstractNode::Global(n.into())
    }
    fn field(n: &str) -> ArcField {
        ArcField {
            layout: n.into(),
            position: 0,
        }
    }

    #[test]
    fn fields_and_late_aliases_converge_without_erasing_edges() {
        let mut rs = vec![
            Constraint::Read {
                object: 2,
                field: field("L"),
                dest: 3,
            },
            Constraint::Write {
                object: 0,
                field: field("L"),
                value: 1,
            },
            Constraint::Copy {
                origin: 0,
                dest: 2,
            },
            Constraint::Seed {
                dest: 0,
                node: node("a"),
            },
            Constraint::Seed {
                dest: 1,
                node: node("b"),
            },
            Constraint::Seed {
                dest: 1,
                node: node("c"),
            },
        ];
        let r = solve(4, 8, &rs).unwrap();
        assert_eq!(r.variable(3), r.variable(1));
        assert_eq!(r.field(&node("a"), &field("L")), r.variable(1));
        rs.reverse();
        let inverse = solve(4, 8, &rs).unwrap();
        assert_eq!(r.variables, inverse.variables);
        assert_eq!(r.fields, inverse.fields);
    }

    #[test]
    fn opaque_writer_and_budget_invalidate_readers_without_contaminating_other_layout() {
        for opaque in [true, false] {
            let rs = vec![
                Constraint::Read {
                    object: 0,
                    field: field("L"),
                    dest: 3,
                },
                Constraint::Read {
                    object: 0,
                    field: field("Outro"),
                    dest: 4,
                },
                Constraint::Write {
                    object: 0,
                    field: field("Outro"),
                    value: 2,
                },
                Constraint::Write {
                    object: 1,
                    field: field("L"),
                    value: 2,
                },
                Constraint::Seed {
                    dest: 0,
                    node: node("a"),
                },
                Constraint::Seed {
                    dest: 1,
                    node: node("a"),
                },
                Constraint::Seed {
                    dest: 2,
                    node: node("b"),
                },
                if opaque {
                    Constraint::Unknown { dest: 1 }
                } else {
                    Constraint::Seed {
                        dest: 1,
                        node: node("c"),
                    }
                },
            ];
            let r = solve(5, 1, &rs).unwrap();
            assert!(r.variable(3).unwrap().nodes().is_none());
            assert_eq!(r.variable(4), r.variable(2));
            assert!(r.field(&node("a"), &field("L")).is_none());
        }
    }

    #[test]
    fn deep_chain_and_recursive_phi_terminate_by_dependencies() {
        let n = 20_000;
        let mut rs: Vec<_> = (1..n)
            .rev()
            .map(|v| Constraint::Copy {
                origin: v - 1,
                dest: v,
            })
            .collect();
        rs.push(Constraint::Copy {
            origin: n - 1,
            dest: 0,
        });
        rs.push(Constraint::Seed {
            dest: 0,
            node: node("raiz"),
        });
        let r = solve(n, 1, &rs).unwrap();
        assert!(r.variables.iter().all(|p| p == &r.variables[0]));
        assert_eq!(r.variables[0].nodes().unwrap().len(), 1);
    }

    #[test]
    fn invalid_indices_are_errors_and_never_empty_facts() {
        assert!(
            solve(
                1,
                8,
                &[Constraint::Copy {
                    origin: 1,
                    dest: 0
                }]
            )
            .is_err()
        );
        assert!(
            solve(
                1,
                8,
                &[Constraint::Read {
                    object: 0,
                    field: field("L"),
                    dest: 1
                }]
            )
            .is_err()
        );
    }
}
