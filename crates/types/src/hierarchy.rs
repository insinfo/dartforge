//! Instantiated class hierarchy and supertype resolution.
//!
//! For each class, computes the transitive closure of instantiated supertypes
//! with type arguments substituted (`List<int>` seen as `Iterable` is
//! `Iterable<int>`), including mixin applications (`S with M`) and `on` clauses.

use crate::ops::{normalize, nullable, substitute};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_elements::model::ClassId;
use std::collections::{HashMap, HashSet};

/// Instantiated supertype information of a class (transitive closure).
#[derive(Debug, Clone, Default)]
pub struct ClassHierarchyData {
    /// Formal type parameters of the class.
    pub type_params: Box<[TypeParamId]>,
    /// Map from a supertype's `ClassId` → type instantiated in terms of this
    /// class's formal parameters (`Target<args_em_funcao_dos_formais>`).
    pub supertypes: HashMap<ClassId, TypeId>,
    /// Ordered list of all transitive supertypes.
    pub all_supertypes: Vec<TypeId>,
    /// Maximum depth in the tree starting from `Object` (used in the LUB computation).
    pub depth: u32,
}

/// Complete graph of the program's instantiated hierarchy.
#[derive(Debug, Default, Clone)]
pub struct ClassHierarchy {
    classes: Vec<Option<ClassHierarchyData>>,
    /// The extension types that are not subtypes of `Object` (no superinterface
    /// leads to a class): the `Object` that the closure puts at their top is
    /// only that of `Object?` (extension types spec, "Static analysis of
    /// extension types": without `implements`, only `E <: Object?`).
    pub extensions_without_object: HashSet<ClassId>,
}

impl ClassHierarchy {
    /// Creates a hierarchy with capacity for `num_classes`.
    pub fn new(num_classes: usize) -> Self {
        let mut classes = Vec::with_capacity(num_classes);
        classes.resize_with(num_classes, || None);
        Self { classes, extensions_without_object: HashSet::new() }
    }

    /// Returns the hierarchy data for a given class, if any.
    pub fn get(&self, class: ClassId) -> Option<&ClassHierarchyData> {
        self.classes
            .get(class.0 as usize)
            .and_then(|opt| opt.as_ref())
    }

    /// Registers or updates the hierarchy data of a class.
    pub fn set(&mut self, class: ClassId, data: ClassHierarchyData) {
        let idx = class.0 as usize;
        if idx >= self.classes.len() {
            self.classes.resize_with(idx + 1, || None);
        }
        self.classes[idx] = Some(data);
    }

    /// Gets the instantiated supertype of `ty` for a target class `target`.
    ///
    /// Example:
    /// `supertype_of(List<int>, Iterable) == Some(Iterable<int>)`
    /// `supertype_of(Map<String, int>, Object) == Some(Object)`
    pub fn supertype_of(
        &self,
        ty: TypeId,
        target: ClassId,
        table: &mut TypeTable,
        core: &CoreTypes,
    ) -> Option<TypeId> {
        let t = table.get(ty).clone();
        match t {
            Type::Interface {
                class,
                args,
                nullable: is_null,
            } => {
                if class == target {
                    return Some(ty);
                }

                if let Some(obj_class) = core.object_class
                    && target == obj_class
                {
                    return Some(if is_null {
                        core.object_nullable
                    } else {
                        core.object
                    });
                }

                let class_data = self.get(class)?;
                let template_ty = class_data.supertypes.get(&target).copied()?;

                // Substitutes the formal parameters of `class` with the concrete arguments `args`
                let mut mapping = HashMap::with_capacity(class_data.type_params.len());
                for (&param, &arg) in class_data.type_params.iter().zip(args.iter()) {
                    mapping.insert(param, arg);
                }

                let substituted = substitute(template_ty, &mapping, table);
                if is_null {
                    Some(nullable(substituted, table))
                } else {
                    Some(substituted)
                }
            }
            Type::ExtensionType {
                decl,
                args,
                nullable: is_null,
            } => {
                if decl == target {
                    return Some(ty);
                }

                let class_data = self.get(decl)?;
                let template_ty = class_data.supertypes.get(&target).copied()?;

                let mut mapping = HashMap::with_capacity(class_data.type_params.len());
                for (&param, &arg) in class_data.type_params.iter().zip(args.iter()) {
                    mapping.insert(param, arg);
                }

                let substituted = substitute(template_ty, &mapping, table);
                if is_null {
                    Some(nullable(substituted, table))
                } else {
                    Some(substituted)
                }
            }
            Type::Intersection { bound, .. } => self.supertype_of(bound, target, table, core),
            Type::TypeParameter {
                param,
                nullable: is_null,
            } => {
                let bound = table.param(param).bound;
                let super_ty = self.supertype_of(bound, target, table, core)?;
                if is_null {
                    Some(nullable(super_ty, table))
                } else {
                    Some(super_ty)
                }
            }
            _ => None,
        }
    }
}

/// Immediate supertype entry for a class: `(parâmetros_formais, supertipos_diretos)`.
pub type ImmediateSupertypeInput = (Box<[TypeParamId]>, Vec<TypeId>);

/// Builds the instantiated hierarchy for all classes of the program.
pub fn build_class_hierarchy(
    num_classes: usize,
    immediate_supertypes: &[Option<ImmediateSupertypeInput>],
    table: &mut TypeTable,
    core: &CoreTypes,
) -> ClassHierarchy {
    let mut hierarchy = ClassHierarchy::new(num_classes);
    let mut visiting = HashSet::new();

    for class_idx in 0..num_classes {
        let class_id = ClassId(class_idx as u32);
        compute_class_hierarchy(
            class_id,
            immediate_supertypes,
            &mut hierarchy,
            &mut visiting,
            table,
            core,
        );
    }

    hierarchy
}

fn compute_class_hierarchy(
    class: ClassId,
    immediate: &[Option<ImmediateSupertypeInput>],
    hierarchy: &mut ClassHierarchy,
    visiting: &mut HashSet<ClassId>,
    table: &mut TypeTable,
    core: &CoreTypes,
) {
    if hierarchy.get(class).is_some() {
        return;
    }

    if !visiting.insert(class) {
        // Inheritance cycle (semantic error caught in diagnostics); stops recursion
        return;
    }

    let (formals, direct_supertypes) =
        match immediate.get(class.0 as usize).and_then(|x| x.as_ref()) {
            Some(data) => (data.0.clone(), data.1.clone()),
            None => {
                visiting.remove(&class);
                return;
            }
        };

    let mut supertypes_map = HashMap::new();
    let mut all_supertypes = Vec::new();
    let mut max_depth = 0;
    // `_ClassInterfaceType` of each ancestor: the result of the join (the
    // ones normalized by `topMerge`) and whether it already failed.
    let mut joins: HashMap<ClassId, (Option<TypeId>, bool)> = HashMap::new();
    let mut join = |supertypes_map: &mut HashMap<ClassId, TypeId>, all_supertypes: &mut Vec<TypeId>, table: &mut TypeTable, c: ClassId, ty: TypeId| {
        let Some(&previous) = supertypes_map.get(&c) else {
            supertypes_map.insert(c, ty);
            all_supertypes.push(ty);
            return;
        };
        let state = joins.entry(c).or_insert((None, false));
        if state.1 {
            return;
        }
        let current = match state.0 {
            Some(a) => a,
            None => {
                if ty == previous {
                    return;
                }
                normalize(previous, table, core)
            }
        };
        let n = normalize(ty, table, core);
        match crate::ops::top_merge(table, core, current, n) {
            Some(r) => {
                state.0 = Some(r);
                if let Some(pos) = all_supertypes.iter().position(|&x| x == previous) {
                    all_supertypes[pos] = r;
                }
                supertypes_map.insert(c, r);
            }
            None => {
                state.0 = Some(current);
                state.1 = true;
            }
        }
    };

    for &direct in direct_supertypes.iter() {
        let direct_type = table.get(direct).clone();
        let (direct_class, direct_args) = match direct_type {
            Type::Interface { class: c, args, .. } => (c, args),
            Type::ExtensionType { decl: c, args, .. } => (c, args),
            _ => continue,
        };

        // Ensures that the direct supertype has had its hierarchy computed
        compute_class_hierarchy(direct_class, immediate, hierarchy, visiting, table, core);

        // Inserts the direct supertype itself (along with whatever is there)
        join(&mut supertypes_map, &mut all_supertypes, table, direct_class, direct);

        if let Some(parent_data) = hierarchy.get(direct_class).cloned() {
            if parent_data.depth + 1 > max_depth {
                max_depth = parent_data.depth + 1;
            }

            // Maps the formal parameters of the parent class to the arguments supplied in this class
            let mut parent_subst = HashMap::with_capacity(parent_data.type_params.len());
            for (&p, &a) in parent_data.type_params.iter().zip(direct_args.iter()) {
                parent_subst.insert(p, a);
            }

            // Propagates the parent's transitive supertypes. The order is by
            // `ClassId`: iterating the direct map would inherit the hash seed
            // (a different order on each emission in the same process) and, with
            // a generic diamond (two routes to the same ancestor with different
            // instantiations), the `first-wins` below would pick another
            // binding: the LLVM IR changed with the number of workers
            // (determinism of the native backend). On conflict the winner is
            // arbitrary but stable, like the "rest by `ClassId`" of
            // `ordered_supertypes` (`scope.rs`).
            let mut the_inherited: Vec<(&ClassId, &TypeId)> = parent_data.supertypes.iter().collect();
            the_inherited.sort_by_key(|(c, _)| c.0);
            for (&ancestor_class, &ancestor_ty) in the_inherited {
                let substituted = substitute(ancestor_ty, &parent_subst, table);
                join(&mut supertypes_map, &mut all_supertypes, table, ancestor_class, substituted);
            }
        }
    }

    // Ensures Object at the top of every class (except Object itself)
    if let Some(obj_class) = core.object_class
        && class != obj_class
        && !supertypes_map.contains_key(&obj_class)
    {
        supertypes_map.insert(obj_class, core.object);
        all_supertypes.push(core.object);
        if max_depth == 0 {
            max_depth = 1;
        }
    }

    visiting.remove(&class);

    hierarchy.set(
        class,
        ClassHierarchyData {
            type_params: formals,
            supertypes: supertypes_map,
            all_supertypes,
            depth: max_depth,
        },
    );
}
