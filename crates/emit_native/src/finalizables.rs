//! Classificação estática da obrigação de vida de Finalizable (§22.4).
//! Não emite proteção: escopos, this, saídas e suspensão pertencem ao lowering.

use dartforge_elements::model::ClassId;
use dartforge_types::{
    TypeId,
    hierarchy::ClassHierarchy,
    table::{Type, TypeTable},
};
use std::collections::HashSet;

/// None exige completar a prova; não é autorização para omitir proteção.
pub(crate) fn classify(
    table: &TypeTable,
    hierarchy: &ClassHierarchy,
    marker: Option<ClassId>,
    mut ty: TypeId,
) -> Option<bool> {
    let Some(marker) = marker else {
        return Some(false);
    };
    let mut seen_set = HashSet::new();
    loop {
        if !seen_set.insert(ty) {
            return None;
        }
        match table.get(ty) {
            Type::Never
            | Type::Null
            | Type::Dynamic
            | Type::Void
            | Type::Function { .. }
            | Type::Record { .. } => return Some(false),
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                return classify_class(hierarchy, Some(marker), *class);
            }
            Type::FutureOr { arg, .. } => ty = *arg,
            Type::TypeParameter { param, .. } => ty = table.param(*param).bound,
            Type::Intersection { bound, .. } => ty = *bound,
        }
    }
}

/// Classifica o receptor por identidade estática, sem fabricar um TypeId.
pub(crate) fn classify_class(
    hierarchy: &ClassHierarchy,
    marker: Option<ClassId>,
    class: ClassId,
) -> Option<bool> {
    let Some(marker) = marker else {
        return Some(false);
    };
    if class == marker {
        return Some(true);
    }
    hierarchy
        .get(class)
        .map(|h| h.supertypes.contains_key(&marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::{
        hierarchy::ClassHierarchyData,
        table::{TypeParamOwner, Variance},
    };

    #[test]
    fn identity_supertypes_nullability_futureor_and_extension() {
        let mut table = TypeTable::new();
        let marker = ClassId(0);
        let alvo = table.intern(Type::Interface {
            class: marker,
            args: Box::new([]),
            nullable: false,
        });
        let mut hierarchy = ClassHierarchy::new(3);
        let mut sub = ClassHierarchyData::default();
        sub.supertypes.insert(marker, alvo);
        hierarchy.set(ClassId(1), sub);
        hierarchy.set(ClassId(2), ClassHierarchyData::default());
        for class in [marker, ClassId(1), ClassId(2)] {
            for nullable in [false, true] {
                let t = table.intern(Type::Interface {
                    class,
                    args: Box::new([]),
                    nullable,
                });
                let future = table.intern(Type::FutureOr { arg: t, nullable });
                let ext = table.intern(Type::ExtensionType {
                    decl: class,
                    args: Box::new([]),
                    nullable,
                });
                for ty in [t, future, ext] {
                    assert_eq!(
                        classify(&table, &hierarchy, Some(marker), ty),
                        Some(class != ClassId(2))
                    );
                }
            }
        }
        let never = table.intern(Type::Never);
        let future = table.intern(Type::FutureOr {
            arg: never,
            nullable: true,
        });
        assert_eq!(
            classify(&table, &hierarchy, Some(marker), future),
            Some(false)
        );
        let unknown = table.intern(Type::Interface {
            class: ClassId(3),
            args: Box::new([]),
            nullable: false,
        });
        assert_eq!(
            classify(&table, &hierarchy, Some(marker), unknown),
            None
        );
    }

    #[test]
    fn limits_promotion_and_recursion_give_no_false_proof() {
        let mut table = TypeTable::new();
        let mut interner = Interner::new();
        let marker = ClassId(0);
        let alvo = table.intern(Type::Interface {
            class: marker,
            args: Box::new([]),
            nullable: false,
        });
        let param = table.alloc_type_param(
            interner.intern("T"),
            TypeParamOwner::GenericFunctionType,
            alvo,
            Variance::Unspecified,
        );
        let t = table.intern(Type::TypeParameter {
            param,
            nullable: true,
        });
        let hierarchy = ClassHierarchy::new(1);
        assert_eq!(
            classify(&table, &hierarchy, Some(marker), t),
            Some(true)
        );
        let promoted = table.intern(Type::Intersection { param, bound: alvo });
        assert_eq!(
            classify(&table, &hierarchy, Some(marker), promoted),
            Some(true)
        );
        table.set_type_param_bound(param, t);
        assert_eq!(classify(&table, &hierarchy, Some(marker), t), None);
    }
}
