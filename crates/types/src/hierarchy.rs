//! Hierarquia instanciada de classes e resolução de supertipos.
//!
//! Para cada classe, computa o fechamento transitivo de supertipos instanciados
//! com argumentos de tipo substituídos (`List<int>` visto como `Iterable` é
//! `Iterable<int>`), incluindo aplicações de mixin (`S with M`) e cláusulas `on`.

use crate::ops::{nullable, substitute};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_elements::model::ClassId;
use std::collections::{HashMap, HashSet};

/// Informações de supertipos instanciados de uma classe (fechamento transitivo).
#[derive(Debug, Clone, Default)]
pub struct ClassHierarchyData {
    /// Parâmetros de tipo formais da classe.
    pub type_params: Box<[TypeParamId]>,
    /// Mapa de `ClassId` de supertipo → tipo instanciado em função dos
    /// parâmetros formais desta classe (`Target<args_em_funcao_dos_formais>`).
    pub supertypes: HashMap<ClassId, TypeId>,
    /// Lista ordenada de todos os supertipos transitivos.
    pub all_supertypes: Vec<TypeId>,
    /// Profundidade máxima na árvore a partir de `Object` (usado no cálculo de LUB).
    pub depth: u32,
}

/// Grafo completo da hierarquia instanciada do programa.
#[derive(Debug, Default)]
pub struct ClassHierarchy {
    classes: Vec<Option<ClassHierarchyData>>,
}

impl ClassHierarchy {
    /// Cria uma hierarquia com capacidade para `num_classes`.
    pub fn new(num_classes: usize) -> Self {
        let mut classes = Vec::with_capacity(num_classes);
        classes.resize_with(num_classes, || None);
        Self { classes }
    }

    /// Retorna os dados da hierarquia para uma dada classe, se existirem.
    pub fn get(&self, class: ClassId) -> Option<&ClassHierarchyData> {
        self.classes
            .get(class.0 as usize)
            .and_then(|opt| opt.as_ref())
    }

    /// Registra ou atualiza os dados da hierarquia de uma classe.
    pub fn set(&mut self, class: ClassId, data: ClassHierarchyData) {
        let idx = class.0 as usize;
        if idx >= self.classes.len() {
            self.classes.resize_with(idx + 1, || None);
        }
        self.classes[idx] = Some(data);
    }

    /// Obtém o supertipo instanciado de `ty` para uma classe alvo `target`.
    ///
    /// Exemplo:
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

                // Substitui os parâmetros formais de `class` pelos argumentos concretos `args`
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

/// Entrada de supertipos imediatos para uma classe: `(parâmetros_formais, supertipos_diretos)`.
pub type ImmediateSupertypeInput = (Box<[TypeParamId]>, Vec<TypeId>);

/// Constrói a hierarquia instanciada para todas as classes do programa.
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
        // Ciclo na herança (erro semântico apanhado no diagnostics); interrompe recursão
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

    for &direct in direct_supertypes.iter() {
        let direct_type = table.get(direct).clone();
        let (direct_class, direct_args) = match direct_type {
            Type::Interface { class: c, args, .. } => (c, args),
            Type::ExtensionType { decl: c, args, .. } => (c, args),
            _ => continue,
        };

        // Garante que o supertipo direto teve sua hierarquia computada
        compute_class_hierarchy(direct_class, immediate, hierarchy, visiting, table, core);

        // Insere o próprio supertipo direto
        if let std::collections::hash_map::Entry::Vacant(e) = supertypes_map.entry(direct_class) {
            e.insert(direct);
            all_supertypes.push(direct);
        }

        if let Some(parent_data) = hierarchy.get(direct_class).cloned() {
            if parent_data.depth + 1 > max_depth {
                max_depth = parent_data.depth + 1;
            }

            // Mapeia os parâmetros formais da classe pai para os argumentos fornecidos nesta classe
            let mut parent_subst = HashMap::with_capacity(parent_data.type_params.len());
            for (&p, &a) in parent_data.type_params.iter().zip(direct_args.iter()) {
                parent_subst.insert(p, a);
            }

            // Propaga os supertipos transitivos do pai. A ordem é por
            // `ClassId`: iterar o mapa direto herdaria a semente do hash
            // (ordem diferente a cada emissão no mesmo processo) e, com
            // diamante genérico (duas rotas ao mesmo ancestral com
            // instanciações diferentes), o `first-wins` abaixo escolheria
            // outra ligação — o LLVM IR mudava com o número de
            // trabalhadores (determinismo do nativo). Em conflito o
            // vencedor é arbitrário mas estável, como o "resto por
            // `ClassId`" de `supertipos_ordenados` (`scope.rs`).
            let mut herdados: Vec<(&ClassId, &TypeId)> = parent_data.supertypes.iter().collect();
            herdados.sort_by_key(|(c, _)| c.0);
            for (&ancestor_class, &ancestor_ty) in herdados {
                if let std::collections::hash_map::Entry::Vacant(e) =
                    supertypes_map.entry(ancestor_class)
                {
                    let substituted = substitute(ancestor_ty, &parent_subst, table);
                    e.insert(substituted);
                    all_supertypes.push(substituted);
                }
            }
        }
    }

    // Garante Object no topo de toda classe (exceto o próprio Object)
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
