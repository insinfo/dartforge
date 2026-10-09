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
pub(crate) fn classificar(
    table: &TypeTable,
    hierarchy: &ClassHierarchy,
    marcador: Option<ClassId>,
    mut tipo: TypeId,
) -> Option<bool> {
    let Some(marcador) = marcador else {
        return Some(false);
    };
    let mut vistos = HashSet::new();
    loop {
        if !vistos.insert(tipo) {
            return None;
        }
        match table.get(tipo) {
            Type::Never
            | Type::Null
            | Type::Dynamic
            | Type::Void
            | Type::Function { .. }
            | Type::Record { .. } => return Some(false),
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                if *class == marcador {
                    return Some(true);
                }
                return hierarchy
                    .get(*class)
                    .map(|h| h.supertypes.contains_key(&marcador));
            }
            Type::FutureOr { arg, .. } => tipo = *arg,
            Type::TypeParameter { param, .. } => tipo = table.param(*param).bound,
            Type::Intersection { bound, .. } => tipo = *bound,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::{
        hierarchy::ClassHierarchyData,
        table::{TypeParamOwner, Variance},
    };

    #[test]
    fn identidade_supertipos_nulabilidade_futureor_e_extensao() {
        let mut table = TypeTable::new();
        let marcador = ClassId(0);
        let alvo = table.intern(Type::Interface {
            class: marcador,
            args: Box::new([]),
            nullable: false,
        });
        let mut hierarchy = ClassHierarchy::new(3);
        let mut sub = ClassHierarchyData::default();
        sub.supertypes.insert(marcador, alvo);
        hierarchy.set(ClassId(1), sub);
        hierarchy.set(ClassId(2), ClassHierarchyData::default());
        for class in [marcador, ClassId(1), ClassId(2)] {
            for nullable in [false, true] {
                let t = table.intern(Type::Interface {
                    class,
                    args: Box::new([]),
                    nullable,
                });
                let futuro = table.intern(Type::FutureOr { arg: t, nullable });
                let ext = table.intern(Type::ExtensionType {
                    decl: class,
                    args: Box::new([]),
                    nullable,
                });
                for tipo in [t, futuro, ext] {
                    assert_eq!(
                        classificar(&table, &hierarchy, Some(marcador), tipo),
                        Some(class != ClassId(2))
                    );
                }
            }
        }
        let never = table.intern(Type::Never);
        let futuro = table.intern(Type::FutureOr {
            arg: never,
            nullable: true,
        });
        assert_eq!(
            classificar(&table, &hierarchy, Some(marcador), futuro),
            Some(false)
        );
        let desconhecido = table.intern(Type::Interface {
            class: ClassId(3),
            args: Box::new([]),
            nullable: false,
        });
        assert_eq!(
            classificar(&table, &hierarchy, Some(marcador), desconhecido),
            None
        );
    }

    #[test]
    fn limites_promocao_e_recursao_nao_fornecem_falsa_prova() {
        let mut table = TypeTable::new();
        let mut interner = Interner::new();
        let marcador = ClassId(0);
        let alvo = table.intern(Type::Interface {
            class: marcador,
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
            classificar(&table, &hierarchy, Some(marcador), t),
            Some(true)
        );
        let promovido = table.intern(Type::Intersection { param, bound: alvo });
        assert_eq!(
            classificar(&table, &hierarchy, Some(marcador), promovido),
            Some(true)
        );
        table.set_type_param_bound(param, t);
        assert_eq!(classificar(&table, &hierarchy, Some(marcador), t), None);
    }
}
