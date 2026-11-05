//! Restoring element slots before scopes can contain both kept and fresh IDs.
//!
//! Surviving declarations across all reloaded libraries reserve their previous
//! slots; new ones take the unclaimed suffix slots in source order. A library
//! can become noncontiguous without moving another library's declarations.
//! A deletion that leaves a surviving slot beyond the new suffix, or an
//! ambiguous name, refuses the permutation without changing the program.

use dartforge_elements::frozen::Checkpoint;
use dartforge_elements::model::*;
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};
use std::mem::{Discriminant, discriminant};

#[derive(Clone, PartialEq, Eq, Hash)]
enum Owner {
    TopLevel,
    Class(SymbolId, Discriminant<ClassKind>),
    Extension(Option<SymbolId>),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Class(SymbolId, Discriminant<ClassKind>),
    Extension(Option<SymbolId>),
    Typedef(SymbolId),
    Function(SymbolId, Discriminant<FunctionKind>, Owner, Option<bool>),
    Variable(SymbolId, Discriminant<VariableRef>, Owner),
}

pub(super) struct Elements([Vec<(u32, Key)>; 5]);

impl Elements {
    /// A stable library belongs to the reloaded suffix. The checkpoint is an
    /// arena boundary, so kept elements cannot contribute to its identity.
    pub(super) fn capture(program: &Program, lib: LibraryId, from: &Checkpoint) -> Self {
        let owner = |class: Option<ClassId>, extension: Option<ExtensionId>| {
            if let Some(class) = class {
                let c = program.class(class);
                Owner::Class(c.name, discriminant(&c.kind))
            } else if let Some(extension) = extension {
                Owner::Extension(program.extension(extension).name)
            } else {
                Owner::TopLevel
            }
        };
        Self([
            program
                .classes
                .iter()
                .enumerate()
                .skip(from.classes)
                .filter(|(_, x)| x.library == lib)
                .map(|(i, x)| (i as u32, Key::Class(x.name, discriminant(&x.kind))))
                .collect(),
            program
                .extensions
                .iter()
                .enumerate()
                .skip(from.extensions)
                .filter(|(_, x)| x.library == lib)
                .map(|(i, x)| (i as u32, Key::Extension(x.name)))
                .collect(),
            program
                .typedefs
                .iter()
                .enumerate()
                .skip(from.typedefs)
                .filter(|(_, x)| x.library == lib)
                .map(|(i, x)| (i as u32, Key::Typedef(x.name)))
                .collect(),
            program
                .functions
                .iter()
                .enumerate()
                .skip(from.functions)
                .filter(|(_, x)| x.library == lib)
                .map(|(i, x)| {
                    let setter = x
                        .variable
                        .map(|v| program.variable(v).setter == Some(FunctionElementId(i as u32)));
                    (
                        i as u32,
                        Key::Function(
                            x.name,
                            discriminant(&x.kind),
                            owner(x.class, x.extension),
                            setter,
                        ),
                    )
                })
                .collect(),
            program
                .variables
                .iter()
                .enumerate()
                .skip(from.variables)
                .filter(|(_, x)| x.library == lib)
                .map(|(i, x)| {
                    (
                        i as u32,
                        Key::Variable(x.name, discriminant(&x.node), owner(x.class, x.extension)),
                    )
                })
                .collect(),
        ])
    }

    pub(super) fn preserved(&self, current: &Self) -> bool {
        self.0.iter().zip(&current.0).all(|(before, after)| {
            let old: HashMap<&Key, u32> = before.iter().map(|(id, key)| (key, *id)).collect();
            let new: HashMap<&Key, u32> = after.iter().map(|(id, key)| (key, *id)).collect();
            old.len() == before.len()
                && new.len() == after.len()
                && new
                    .iter()
                    .all(|(key, id)| old.get(key).is_none_or(|old_id| old_id == id))
        })
    }
}

/// A permutation of the complete fresh suffix, independently of library ranges.
/// Surviving declarations reserve their previous slots; new declarations fill
/// unclaimed slots after every library has made its reservations.
struct Arena {
    start: usize,
    destinations: Vec<usize>,
    claimed: Vec<bool>,
}

impl Arena {
    fn new(start: usize, end: usize) -> Self {
        Self {
            start,
            destinations: vec![usize::MAX; end - start],
            claimed: vec![false; end - start],
        }
    }

    fn keep(&mut self, before: &[(u32, Key)], after: &[(u32, Key)]) -> bool {
        let old: HashMap<&Key, u32> = before.iter().map(|(id, key)| (key, *id)).collect();
        let unique: HashSet<&Key> = after.iter().map(|(_, key)| key).collect();
        if old.len() != before.len() || unique.len() != after.len() {
            return false;
        }
        for (source, key) in after {
            let Some(&target) = old.get(key) else {
                continue;
            };
            let (Some(source), Some(target)) = (
                (*source as usize).checked_sub(self.start),
                (target as usize).checked_sub(self.start),
            ) else {
                return false;
            };
            let (Some(destination), Some(claimed)) = (
                self.destinations.get_mut(source),
                self.claimed.get_mut(target),
            ) else {
                return false;
            };
            if *destination != usize::MAX || *claimed {
                return false;
            }
            *destination = target;
            *claimed = true;
        }
        true
    }

    fn finish(&mut self) {
        let mut available = self
            .claimed
            .iter()
            .enumerate()
            .filter_map(|(i, used)| (!used).then_some(i));
        for destination in &mut self.destinations {
            if *destination == usize::MAX {
                *destination = available.next().expect("one destination per source");
            }
        }
    }

    fn is_identity(&self) -> bool {
        self.destinations.iter().enumerate().all(|(i, d)| i == *d)
    }

    fn id(&self, id: u32) -> u32 {
        (id as usize)
            .checked_sub(self.start)
            .and_then(|i| self.destinations.get(i))
            .map_or(id, |&target| (self.start + target) as u32)
    }

    fn apply<T>(&self, arena: &mut [T]) {
        let items = &mut arena[self.start..self.start + self.destinations.len()];
        let mut destinations = self.destinations.clone();
        for source in 0..destinations.len() {
            while destinations[source] != source {
                let target = destinations[source];
                items.swap(source, target);
                destinations.swap(source, target);
            }
        }
    }
}

pub(super) struct Slots([Arena; 5]);

impl Slots {
    pub(super) fn new(program: &Program, from: &Checkpoint) -> Self {
        Self([
            Arena::new(from.classes, program.classes.len()),
            Arena::new(from.extensions, program.extensions.len()),
            Arena::new(from.typedefs, program.typedefs.len()),
            Arena::new(from.functions, program.functions.len()),
            Arena::new(from.variables, program.variables.len()),
        ])
    }

    pub(super) fn keep(&mut self, before: &Elements, after: &Elements) -> bool {
        self.0
            .iter_mut()
            .zip(before.0.iter().zip(&after.0))
            .all(|(arena, (before, after))| arena.keep(before, after))
    }

    fn id(&self, arena: usize, id: u32) -> u32 {
        self.0[arena].id(id)
    }

    fn class(&self, id: &mut ClassId) {
        id.0 = self.id(0, id.0);
    }
    fn extension(&self, id: &mut ExtensionId) {
        id.0 = self.id(1, id.0);
    }
    fn typedef(&self, id: &mut TypedefId) {
        id.0 = self.id(2, id.0);
    }
    fn function(&self, id: &mut FunctionElementId) {
        id.0 = self.id(3, id.0);
    }
    fn variable(&self, id: &mut VariableId) {
        id.0 = self.id(4, id.0);
    }

    fn element(&self, element: &mut Element) {
        match element {
            Element::Class(id) => self.class(id),
            Element::Extension(id) => self.extension(id),
            Element::Typedef(id) => self.typedef(id),
            Element::Function(id) => self.function(id),
            Element::Variable(id) => self.variable(id),
            Element::Prefix(..) => {}
        }
    }

    pub(super) fn apply(&mut self, program: &mut Program, from: &Checkpoint) -> bool {
        for arena in &mut self.0 {
            arena.finish();
        }
        if self.0.iter().all(Arena::is_identity) {
            return true;
        }
        // ClassElement::constructors walks in ID order. Preserve that source
        // order until constructor iteration has an independent ordering key.
        if program.classes[from.classes..].iter().any(|c| {
            let mut ids: Vec<_> = c.constructors.values().map(|f| f.0).collect();
            ids.sort_unstable();
            !ids.into_iter().map(|id| self.id(3, id)).is_sorted()
        }) {
            return false;
        }
        // This hook runs before a new index exists. Invalidate an unexpected
        // full index; the ordinary kept prefix index still covers unchanged IDs.
        if program.element_index.get().is_some() {
            program.forget_index();
        }
        for c in &mut program.classes[from.classes..] {
            for f in c
                .instance_members
                .values_mut()
                .chain(c.static_members.values_mut())
                .chain(c.constructors.values_mut())
            {
                self.function(f);
            }
            for (_, f) in &mut c.shadowed_instance_members {
                self.function(f);
            }
            for v in c
                .fields
                .iter_mut()
                .chain(c.enum_constants.iter_mut())
                .chain(c.representation.iter_mut())
            {
                self.variable(v);
            }
            for id in c
                .supertype_class
                .iter_mut()
                .chain(c.mixin_classes.iter_mut())
                .chain(c.interface_classes.iter_mut())
                .chain(c.on_classes.iter_mut())
            {
                self.class(id);
            }
        }
        for e in &mut program.extensions[from.extensions..] {
            for f in e
                .instance_members
                .values_mut()
                .chain(e.static_members.values_mut())
            {
                self.function(f);
            }
            for v in &mut e.fields {
                self.variable(v);
            }
        }
        for f in &mut program.functions[from.functions..] {
            if let Some(c) = &mut f.class {
                self.class(c);
            }
            if let Some(e) = &mut f.extension {
                self.extension(e);
            }
            if let Some(v) = &mut f.variable {
                self.variable(v);
            }
            for f in f
                .patched_by
                .iter_mut()
                .chain(f.public_declaration.iter_mut())
            {
                self.function(f);
            }
        }
        for v in &mut program.variables[from.variables..] {
            if let Some(c) = &mut v.class {
                self.class(c);
            }
            if let Some(e) = &mut v.extension {
                self.extension(e);
            }
            for f in v.getter.iter_mut().chain(v.setter.iter_mut()) {
                self.function(f);
            }
        }
        for lib in &mut program.libraries[from.libraries..] {
            for b in lib.declared.values_mut() {
                for e in b.getter.iter_mut().chain(b.setter.iter_mut()) {
                    self.element(e);
                }
            }
            lib.representative = std::mem::take(&mut lib.representative)
                .into_iter()
                .map(|(mut a, mut b)| {
                    self.element(&mut a);
                    self.element(&mut b);
                    (a, b)
                })
                .collect();
        }
        program.augmentations = std::mem::take(&mut program.augmentations)
            .into_iter()
            .map(|(mut c, d)| {
                // The map also contains prefix classes, whose mapping is identity.
                self.class(&mut c);
                (c, d)
            })
            .collect();
        for c in &mut program.classes_macro {
            self.class(c);
        }
        for (arena, permutation) in self.0.iter().enumerate() {
            if !permutation.is_identity() {
                program
                    .element_order
                    .permute(arena, permutation.start, &permutation.destinations);
            }
        }
        self.0[0].apply(&mut program.classes);
        self.0[1].apply(&mut program.extensions);
        self.0[2].apply(&mut program.typedefs);
        self.0[3].apply(&mut program.functions);
        self.0[4].apply(&mut program.variables);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiple_libraries_reserve_slots_before_new_declarations() {
        let mut names = dartforge_intern::Interner::new();
        let mut key = |name| Key::Typedef(names.intern(name));
        let (a, b, c, d, added) = (key("A"), key("B"), key("C"), key("D"), key("Added"));
        let first_old = vec![(7, a.clone()), (8, b.clone())];
        let first_new = vec![(7, a.clone()), (8, added.clone()), (9, b.clone())];
        let second_old = vec![(9, c.clone()), (10, d.clone())];
        let second_new = vec![(10, c.clone()), (11, d.clone())];
        let mut arena = Arena::new(7, 12);
        assert!(arena.keep(&first_old, &first_new));
        assert!(arena.keep(&second_old, &second_new));
        arena.finish();
        let mut values = vec![added.clone(); 7];
        values.extend([a.clone(), added.clone(), b.clone(), c.clone(), d.clone()]);
        arena.apply(&mut values);
        assert!(values[..7].iter().all(|k| *k == added));
        assert!(values[7..] == [a, b, c, d, added]);
        assert_eq!(arena.id(8), 11);
        assert_eq!(arena.id(9), 8);
    }

    #[test]
    fn ambiguous_or_out_of_range_survivors_refuse_the_permutation() {
        let mut names = dartforge_intern::Interner::new();
        let (a, b, c) = (
            Key::Typedef(names.intern("A")),
            Key::Typedef(names.intern("B")),
            Key::Typedef(names.intern("C")),
        );
        let before = vec![(2, a.clone()), (3, b.clone()), (4, c.clone())];
        assert!(!Arena::new(2, 4).keep(&before, &[(2, a.clone()), (3, c)]));
        assert!(!Arena::new(2, 5).keep(&before, &[(2, a.clone()), (3, a.clone()), (4, b.clone())]));
        assert!(!Arena::new(2, 5).keep(&[(2, a.clone()), (3, a.clone())], &[(2, a), (3, b)]));
    }
}
