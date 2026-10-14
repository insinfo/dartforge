//! Anchored Ref Phi graphs whose inputs carry no logical owned or borrowed token.

use super::*;

pub(super) fn prove(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    directly_owned: &HashSet<ValueId>,
) -> HashSet<ValueId> {
    let mut types = HashMap::new();
    for (v, _, ty) in &f.params {
        if types.insert(*v, *ty).is_some() {
            return HashSet::new();
        }
    }
    for (v, _, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if types.insert(*v, *ty).is_some() {
            return HashSet::new();
        }
    }
    let candidates: HashSet<_> = f.blocks.iter().flat_map(|b| &b.instructions)
        .filter_map(|(v, instruction, ty)| (*ty == Type::Ref && !directly_owned.contains(v)
            && !classes.contains_key(v) && matches!(instruction, Instruction::Phi { ty: Type::Ref, incoming } if !incoming.is_empty())).then_some(*v)).collect();
    let mut parents: HashMap<ValueId, Vec<ValueId>> = HashMap::new();
    let mut invalid = VecDeque::new();
    let mut anchors = VecDeque::new();
    for (v, instruction, _) in f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter(|(v, _, _)| candidates.contains(v))
    {
        let Instruction::Phi { incoming, .. } = instruction else {
            unreachable!()
        };
        let mut compatible = true;
        let mut anchored = false;
        for (_, operand) in incoming {
            match operand {
                Operand::Constant(Constant::Null) => anchored = true,
                Operand::Val(value)
                    if types.get(value) == Some(&Type::Ref) && !directly_owned.contains(value) =>
                {
                    if classes.get(value) == Some(&Ownership::Trivial) {
                        anchored = true;
                    } else if candidates.contains(value) {
                        parents.entry(*value).or_default().push(*v);
                    } else {
                        compatible = false;
                    }
                }
                _ => compatible = false,
            }
        }
        if !compatible {
            invalid.push_back(*v);
        }
        if anchored {
            anchors.push_back(*v);
        }
    }
    let mut compatible = candidates;
    while let Some(value) = invalid.pop_front() {
        if compatible.remove(&value) {
            invalid.extend(parents.get(&value).into_iter().flatten().copied());
        }
    }
    let mut proved = HashSet::new();
    while let Some(value) = anchors.pop_front() {
        if compatible.contains(&value) && proved.insert(value) {
            anchors.extend(parents.get(&value).into_iter().flatten().copied());
        }
    }
    // An anchor in one Phi does not certify an unanchored dependency cycle.
    // Reject every dependent of a candidate left outside the proved graph.
    let mut unproved: VecDeque<_> = compatible.difference(&proved).copied().collect();
    while let Some(value) = unproved.pop_front() {
        for &parent in parents.get(&value).into_iter().flatten() {
            if proved.remove(&parent) {
                unproved.push_back(parent);
            }
        }
    }
    proved
}

#[cfg(test)]
mod tests;
