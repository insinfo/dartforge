//! Building the outline and the namespaces (Step 2 of the Governing Goal).
//!
//! Following docs/FRONTEND-ARQUITETURA.md §3 and PLANO.md:
//! 1. Collects the top-level declarations of every unit (including patches).
//! 2. Applies the SDK's `@patch` over declarations and members of `external`
//!    classes.
//! 3. Builds the exported namespace with fixpoint resolution for transitive
//!    reexports.
//! 4. Builds each library's lexical scope (local declarations > imports >
//!    implicit core).
//! 5. Resolves supertype names (`extends`, `with`, `implements`, `on`) to
//!    `ClassId` and detects inheritance cycles.
use crate::model::*;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// Checks whether an identifier is private (starts with `_`).
pub fn is_private(sym: SymbolId, interner: &Interner) -> bool {
    interner.resolve(sym).starts_with('_')
}

/// Checks whether a list of annotations contains `@patch`.
fn has_patch_annotation(metadata: &[ast::Annotation], interner: &Interner) -> bool {
    metadata
        .iter()
        .any(|m| m.name.len() == 1 && interner.resolve(m.name[0].sym) == "patch")
}

/// Mutable element pools kept apart from units and libraries to satisfy the
/// borrow checker.
pub(crate) struct ElementPools<'a> {
    pub(crate) classes: &'a mut Vec<ClassElement>,
    pub(crate) extensions: &'a mut Vec<ExtensionElement>,
    pub(crate) functions: &'a mut Vec<FunctionElement>,
    pub(crate) variables: &'a mut Vec<VariableElement>,
}

/// The kind of a top-level declaration, for the rule of which declaration a
/// repeated name designates (docs/ANALYZER-ESPECIFICACAO.md §G, T1.1 a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Species {
    Getter,
    Setter,
    Variable,
    Funcao,
    Enum,
    Extension,
    ExtensionType,
    Typedef,
    Mixin,
    Class,
}

impl Species {
    /// The order in which the analyzer puts declarations into the library scope
    /// (`LibraryDeclarations._addLibraryFragment`, `scope.dart:312-332`):
    /// accessors (explicit ones and those of variables), functions, enums,
    /// extensions, extension types, typedefs, mixins, classes. Each insertion
    /// is `??=`: whoever comes first keeps the name.
    fn order(self) -> u8 {
        match self {
            Species::Getter | Species::Setter | Species::Variable => 0,
            Species::Funcao => 1,
            Species::Enum => 2,
            Species::Extension => 3,
            Species::ExtensionType => 4,
            Species::Typedef => 5,
            Species::Mixin => 6,
            Species::Class => 7,
        }
    }
}

/// T1: with a top-level name declared more than once, the name designates
/// the declaration the analyzer would put in the scope first — the one of
/// the earlier kind in [`Species::order`] and, within the same kind, the
/// first in the source —, and not the last one, which is what the
/// declaration loop leaves. Getter and setter are separate slots. Homonyms
/// of the same kind get the [`Library::representante`]. Without a repeated
/// name, nothing changes.
fn resolve_homonyms(lib: &mut Library, candidates: &[(SymbolId, bool, Species, Element, u32)]) {
    let mut per_slot: HashMap<(SymbolId, bool), Vec<usize>> = HashMap::new();
    for (k, (name_, setter, _, _, _)) in candidates.iter().enumerate() {
        per_slot.entry((*name_, *setter)).or_default().push(k);
    }
    for ((name_, setter), group) in &per_slot {
        if group.len() < 2 {
            continue;
        }
        // The winner: the smallest (unit, kind order, source position). The scope
        // is built unit by unit (`library.units.forEach(_addLibraryFragment)`,
        // `scope.dart:286`), and the order by kind only applies within each one.
        let winner = group
            .iter()
            .copied()
            .min_by_key(|&k| (candidates[k].4, candidates[k].2.order(), k))
            .expect("non-empty group");
        let entry_path = lib.declared.entry(*name_).or_default();
        if *setter {
            entry_path.setter = Some(candidates[winner].3);
        } else {
            entry_path.getter = Some(candidates[winner].3);
        }
        // The representatives: the first of each kind in the group.
        let mut first: HashMap<Species, Element> = HashMap::new();
        for &k in group {
            let (_, _, species, element, _) = candidates[k];
            first.entry(species).or_insert(element);
        }
        let mut how_many: HashMap<Species, usize> = HashMap::new();
        for &k in group {
            *how_many.entry(candidates[k].2).or_default() += 1;
        }
        for &k in group {
            let (_, _, species, element, _) = candidates[k];
            if how_many[&species] > 1 {
                lib.representante.insert(element, first[&species]);
            }
        }
    }
}

/// Builds the program's complete outline: declarations, members, namespaces
/// and supertypes.
pub fn build_outline(
    program: &mut Program,
    interner: &mut Interner,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let empty_sym = interner.intern("");
    let mut t_phase = std::time::Instant::now();

    // -----------------------------------------------------------------------
    // Phase 1: collecting the declared elements per library
    // -----------------------------------------------------------------------
    let num_libs = program.libraries.len();
    for lib_idx in 0..num_libs {
        let lib_id = LibraryId(lib_idx as u32);
        let mut unit_ids = program.libraries[lib_idx].units.clone();
        // Makes sure source units (Library and Part) are declared before the
        // patches
        unit_ids.sort_by_key(|&u| {
            if program.units[u.0 as usize].role == UnitRole::Patch {
                1
            } else {
                0
            }
        });

        let mut pools = ElementPools {
            classes: &mut program.classes,
            extensions: &mut program.extensions,
            functions: &mut program.functions,
            variables: &mut program.variables,
        };

        let mut cadeias: Vec<(ClassId, DeclRef)> = Vec::new();
        let mut classes_macro: Vec<ClassId> = Vec::new();
        // T1: each top-level declaration of the source units, in source order —
        // (name, is it the setter slot?, kind, element). With a repeated name, what
        // it designates comes out of `resolve_homonyms`.
        let mut candidates: Vec<(SymbolId, bool, Species, Element, u32)> = Vec::new();
        let has_patch = unit_ids.iter().any(|u| program.units[u.0 as usize].role == UnitRole::Patch);
        for (unit_order, unit_id) in unit_ids.into_iter().enumerate() {
            let unit_order = unit_order as u32;
            let role = program.units[unit_id.0 as usize].role;
            let decl_ids = program.units[unit_id.0 as usize].unit.declarations.clone();
            let ast = &program.units[unit_id.0 as usize].ast;

            for decl_id in decl_ids {
                let decl = ast.decl(decl_id);
                // `augment`: attaches to the declaration of the same name that came before
                // (docs/AUGMENTATIONS.md). The units are in application order (preorder of
                // the parts tree, `load.rs`).
                if decl.augment && role != UnitRole::Patch {
                    let mut merge = crate::augmentation::Merge {
                        units: &program.units,
                        library: &mut program.libraries[lib_idx],
                        lib_id,
                        empty_sym,
                        interner,
                        diagnostics,
                        cadeias: &mut cadeias,
                    };
                    crate::augmentation::aplicar(&mut merge, &mut pools, unit_id, decl_id);
                    continue;
                }
                let is_patch =
                    role == UnitRole::Patch && has_patch_annotation(&decl.metadata, interner);

                match &decl.kind {
                    DeclKind::Class(c) => {
                        let class_name = c.name.sym;
                        // If it is a patch, or a class in a patch, with a name that already exists
                        // in the library: merge
                        let existing_class = if role == UnitRole::Patch {
                            program.libraries[lib_idx]
                                .declared
                                .get(&class_name)
                                .and_then(|b| match b.getter {
                                    Some(Element::Class(cid)) => Some(cid),
                                    _ => None,
                                })
                        } else {
                            None
                        };

                        if let Some(existing_cid) = existing_class {
                            // Merges the patch's members into the existing class
                            merge_class_patch(
                                &mut pools,
                                ast,
                                existing_cid,
                                unit_id,
                                &c.members,
                                is_patch,
                                interner,
                            );
                        } else {
                            let class_id = create_class_element(
                                &mut pools, ast, lib_id, unit_id, decl_id, c, empty_sym, interner,
                            );
                            if c.modifiers.macro_ {
                                classes_macro.push(class_id);
                            }
                            let entry = program.libraries[lib_idx]
                                .declared
                                .entry(class_name)
                                .or_default();
                            entry.getter = Some(Element::Class(class_id));
                            candidates.push((class_name, false, Species::Class, Element::Class(class_id), unit_order));
                        }
                    }
                    DeclKind::Mixin(m) => {
                        let mixin_id = create_mixin_element(
                            &mut pools, ast, lib_id, unit_id, decl_id, m, empty_sym, interner,
                        );
                        let entry = program.libraries[lib_idx]
                            .declared
                            .entry(m.name.sym)
                            .or_default();
                        entry.getter = Some(Element::Class(mixin_id));
                        candidates.push((m.name.sym, false, Species::Mixin, Element::Class(mixin_id), unit_order));
                    }
                    DeclKind::Enum(e) => {
                        let enum_id = create_enum_element(
                            &mut pools, ast, lib_id, unit_id, decl_id, e, empty_sym, interner,
                        );
                        let entry = program.libraries[lib_idx]
                            .declared
                            .entry(e.name.sym)
                            .or_default();
                        entry.getter = Some(Element::Class(enum_id));
                        candidates.push((e.name.sym, false, Species::Enum, Element::Class(enum_id), unit_order));
                    }
                    DeclKind::Extension(ext) => {
                        // `@patch extension E` merges the members into the library's extension `E`
                        // (like `merge_class_patch`).
                        let existing_one = if role == UnitRole::Patch {
                            ext.name.and_then(|n| {
                                program.libraries[lib_idx].declared.get(&n.sym).and_then(|b| match b.getter {
                                    Some(Element::Extension(e)) => Some(e),
                                    _ => None,
                                })
                            })
                        } else {
                            None
                        };
                        if let Some(e) = existing_one {
                            merge_extension_patch(&mut pools, ast, e, unit_id, ext, is_patch, interner);
                            continue;
                        }
                        let ext_id = create_extension_element(
                            &mut pools, ast, lib_id, unit_id, decl_id, ext, empty_sym, interner,
                        );
                        if let Some(name) = ext.name {
                            let entry = program.libraries[lib_idx]
                                .declared
                                .entry(name.sym)
                                .or_default();
                            entry.getter = Some(Element::Extension(ext_id));
                            candidates.push((name.sym, false, Species::Extension, Element::Extension(ext_id), unit_order));
                        }
                    }
                    DeclKind::ExtensionType(et) => {
                        let ext_type_id = create_extension_type_element(
                            &mut pools, ast, lib_id, unit_id, decl_id, et, empty_sym, interner,
                        );
                        let entry = program.libraries[lib_idx]
                            .declared
                            .entry(et.name.sym)
                            .or_default();
                        entry.getter = Some(Element::Class(ext_type_id));
                        candidates.push((et.name.sym, false, Species::ExtensionType, Element::Class(ext_type_id), unit_order));
                    }
                    DeclKind::Typedef(td) => {
                        let td_id = TypedefId(program.typedefs.len() as u32);
                        let type_params = td
                            .type_params
                            .iter()
                            .map(|p| TypeParameterElement {
                                name: p.name.sym,
                                bound: p.bound.map(|b| (unit_id, b)),
                            })
                            .collect();
                        program.typedefs.push(TypedefElement {
                            name: td.name.sym,
                            library: lib_id,
                            decl: DeclRef {
                                unit: unit_id,
                                decl: decl_id,
                            },
                            type_params,
                        });
                        let entry = program.libraries[lib_idx]
                            .declared
                            .entry(td.name.sym)
                            .or_default();
                        entry.getter = Some(Element::Typedef(td_id));
                        candidates.push((td.name.sym, false, Species::Typedef, Element::Typedef(td_id), unit_order));
                    }
                    DeclKind::Function(fid) => {
                        let ast_fn = ast.function(*fid);
                        if let Some(fn_name) = ast_fn.name {
                            let fn_sym = fn_name.sym;
                            let fn_kind = match ast_fn.kind {
                                ast::FunctionKind::Getter => FunctionKind::Getter,
                                ast::FunctionKind::Setter => FunctionKind::Setter,
                                ast::FunctionKind::Operator => FunctionKind::Operator,
                                _ => FunctionKind::Function,
                            };
                            let fn_id = FunctionElementId(pools.functions.len() as u32);
                            pools.functions.push(FunctionElement {
                                name: fn_sym,
                                library: lib_id,
                                class: None,
                                extension: None,
                                kind: fn_kind,
                                static_: ast_fn.static_,
                                abstract_: false,
                                external: ast_fn.external,
                                const_: false,
                                factory: false,
                                node: FunctionRef::Function {
                                    unit: unit_id,
                                    function: *fid,
                                },
                                variable: None,
                                patched_by: None,
                                declaracao_publica: None,
                            });

                            candidates.push(match fn_kind {
                                FunctionKind::Getter => (fn_sym, false, Species::Getter, Element::Function(fn_id), unit_order),
                                FunctionKind::Setter => (fn_sym, true, Species::Setter, Element::Function(fn_id), unit_order),
                                _ => (fn_sym, false, Species::Funcao, Element::Function(fn_id), unit_order),
                            });
                            let entry = program.libraries[lib_idx]
                                .declared
                                .entry(fn_sym)
                                .or_default();
                            match fn_kind {
                                FunctionKind::Getter => {
                                    if is_patch && entry.getter.is_some() {
                                        if let Some(Element::Function(old_id)) = entry.getter {
                                            pools.functions[old_id.0 as usize].patched_by =
                                                Some(fn_id);
                                            pools.functions[fn_id.0 as usize].declaracao_publica = Some(pools.functions[old_id.0 as usize].declaracao_publica.unwrap_or(old_id));
                                        }
                                    }
                                    entry.getter = Some(Element::Function(fn_id));
                                }
                                FunctionKind::Setter => {
                                    if is_patch && entry.setter.is_some() {
                                        if let Some(Element::Function(old_id)) = entry.setter {
                                            pools.functions[old_id.0 as usize].patched_by =
                                                Some(fn_id);
                                            pools.functions[fn_id.0 as usize].declaracao_publica = Some(pools.functions[old_id.0 as usize].declaracao_publica.unwrap_or(old_id));
                                        }
                                    }
                                    entry.setter = Some(Element::Function(fn_id));
                                }
                                _ => {
                                    if is_patch && entry.getter.is_some() {
                                        if let Some(Element::Function(old_id)) = entry.getter {
                                            pools.functions[old_id.0 as usize].patched_by =
                                                Some(fn_id);
                                            pools.functions[fn_id.0 as usize].declaracao_publica = Some(pools.functions[old_id.0 as usize].declaracao_publica.unwrap_or(old_id));
                                        }
                                    }
                                    entry.getter = Some(Element::Function(fn_id));
                                }
                            }
                        }
                    }
                    DeclKind::Variables(vars) => {
                        for (idx, var) in vars.variables.iter().enumerate() {
                            let var_sym = var.name.sym;
                            let var_id = VariableId(pools.variables.len() as u32);

                            // Implicit getter
                            let getter_id = FunctionElementId(pools.functions.len() as u32);
                            pools.functions.push(FunctionElement {
                                name: var_sym,
                                library: lib_id,
                                class: None,
                                extension: None,
                                kind: FunctionKind::ImplicitAccessor,
                                static_: vars.static_,
                                abstract_: vars.abstract_,
                                external: vars.external,
                                const_: vars.const_,
                                factory: false,
                                node: FunctionRef::None,
                                variable: Some(var_id),
                                patched_by: None,
                                declaracao_publica: None,
                            });

                            // Implicit setter if not const/final
                            let setter_id = if !vars.final_ && !vars.const_ {
                                let sid = FunctionElementId(pools.functions.len() as u32);
                                pools.functions.push(FunctionElement {
                                    name: var_sym,
                                    library: lib_id,
                                    class: None,
                                    extension: None,
                                    kind: FunctionKind::ImplicitAccessor,
                                    static_: vars.static_,
                                    abstract_: vars.abstract_,
                                    external: vars.external,
                                    const_: vars.const_,
                                    factory: false,
                                    node: FunctionRef::None,
                                    variable: Some(var_id),
                                    patched_by: None,
                                    declaracao_publica: None,
                                });
                                Some(sid)
                            } else {
                                None
                            };

                            pools.variables.push(VariableElement {
                                name: var_sym,
                                library: lib_id,
                                class: None,
                                extension: None,
                                static_: vars.static_,
                                final_: vars.final_,
                                const_: vars.const_,
                                late: vars.late,
                                external: vars.external,
                                node: VariableRef::TopLevel {
                                    unit: unit_id,
                                    decl: decl_id,
                                    index: idx,
                                },
                                getter: Some(getter_id),
                                setter: setter_id,
                            });

                            let entry = program.libraries[lib_idx]
                                .declared
                                .entry(var_sym)
                                .or_default();
                            entry.getter = Some(Element::Variable(var_id));
                            candidates.push((var_sym, false, Species::Variable, Element::Variable(var_id), unit_order));
                            if setter_id.is_some() {
                                entry.setter = Some(Element::Variable(var_id));
                                candidates.push((var_sym, true, Species::Variable, Element::Variable(var_id), unit_order));
                            }
                        }
                    }
                }
            }
        }
        // A library with patches (the SDK) has no homonyms: there the patch
        // replaces the declaration, and the order is the one the loop above gave.
        if !has_patch {
            resolve_homonyms(&mut program.libraries[lib_idx], &candidates);
        }
        for (c, d) in cadeias {
            program.augmentacoes.entry(c).or_default().push(d);
        }
        program.classes_macro.append(&mut classes_macro);
    }

    // -----------------------------------------------------------------------
    // Phase 2: building Library::exported (with a fixpoint for reexports)
    program.tempos.outline_declaracoes += t_phase.elapsed();
    t_phase = std::time::Instant::now();
    // -----------------------------------------------------------------------
    // Initializes `exported` with what is declared locally and not private
    for lib in &mut program.libraries {
        for (&sym, &binding) in &lib.declared {
            if !is_private(sym, interner) {
                lib.exported.insert(sym, binding);
            }
        }
    }

    // Iterates transitive reexports until reaching a fixpoint
    let mut changed = true;
    let mut iterations = 0;
    while changed && iterations < 500 {
        changed = false;
        iterations += 1;
        for lib_idx in 0..program.libraries.len() {
            let exports = program.libraries[lib_idx].exports.clone();
            for export in exports {
                let target_exported = program.libraries[export.library.0 as usize]
                    .exported
                    .clone();
                let filtered = filter_namespace(&target_exported, &export.combinators);
                let current_exported = &mut program.libraries[lib_idx].exported;

                for (sym, binding) in filtered {
                    if !is_private(sym, interner) {
                        let entry = current_exported.entry(sym).or_default();
                        if entry.getter.is_none() && binding.getter.is_some() {
                            entry.getter = binding.getter;
                            changed = true;
                        }
                        if entry.setter.is_none() && binding.setter.is_some() {
                            entry.setter = binding.setter;
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Phase 3: building each library's lexical scope (Library::scope)
    program.tempos.outline_reexports += t_phase.elapsed();
    t_phase = std::time::Instant::now();
    // -----------------------------------------------------------------------
    for lib_idx in 0..program.libraries.len() {
        let mut scope = program.libraries[lib_idx].declared.clone();
        let mut prefixes: HashMap<SymbolId, Namespace> = HashMap::new();
        let imports = program.libraries[lib_idx].imports.clone();

        for import in imports {
            let target_exported = program.libraries[import.library.0 as usize]
                .exported
                .clone();
            let filtered = filter_namespace(&target_exported, &import.combinators);

            if let Some(prefix_sym) = import.prefix {
                // Prefixed import: accessible via p.member
                let prefix_ns = prefixes.entry(prefix_sym).or_default();
                for (sym, binding) in filtered {
                    let entry = prefix_ns.entry(sym).or_default();
                    merge_binding_with(entry, binding, &|e| sdk_element(program, e));
                }
                // Records the prefix in the general scope
                scope.insert(
                    prefix_sym,
                    Binding {
                        getter: Some(Element::Prefix(import.library, prefix_sym)),
                        setter: None,
                        ambiguous: false,
                    },
                );
            } else {
                // Unprefixed import: goes straight into the scope
                for (sym, binding) in filtered {
                    // A local declaration always wins over an import
                    if program.libraries[lib_idx].declared.contains_key(&sym) {
                        continue;
                    }
                    let entry = scope.entry(sym).or_default();
                    merge_binding_with(entry, binding, &|e| sdk_element(program, e));
                }
            }
        }

        // Implicit dart:core: the lowest precedence of all. Only without an
        // explicit `dart:core` import (`import 'dart:core' as core;` removes even
        // `int` from the scope) in the library file: the 3.6.2 analyzer decides by
        // the directives of the `LibraryFileKind` (`file_state.dart:288-300`,
        // `hasDartCoreImport`), and the `import 'dart:core' as prefix0;` of an
        // augmentation (the macros' one) or of a part does not count.
        let principal = program.libraries[lib_idx].units.first().copied();
        let imports_core = program.libraries[lib_idx]
            .imports
            .iter()
            .any(|i| Some(i.library) == program.core && Some(i.unit) == principal);
        if let Some(core_id) = program.core
            && !imports_core
        {
            if LibraryId(lib_idx as u32) != core_id {
                let core_exported = program.libraries[core_id.0 as usize].exported.clone();
                for (sym, binding) in core_exported {
                    // Only goes in if nothing in the library or in the previous imports took
                    // the name
                    scope.entry(sym).or_insert(binding);
                }
            }
        }

        program.libraries[lib_idx].scope = scope;
        program.libraries[lib_idx].prefixes = prefixes;
        let scopes = escopos_de_unidade(program, LibraryId(lib_idx as u32));
        program.libraries[lib_idx].escopos_de_unidade = scopes;
    }

    // -----------------------------------------------------------------------
    // Phase 4: supertype resolution and cycle checking
    program.tempos.outline_escopos += t_phase.elapsed();
    t_phase = std::time::Instant::now();
    // -----------------------------------------------------------------------
    let object_sym = interner.intern("Object");
    let core_id = program.core;

    for class_idx in 0..program.classes.len() {
        let class_id = ClassId(class_idx as u32);
        let lib_id = program.classes[class_idx].library;
        let class_name = program.classes[class_idx].name;
        let class_kind = program.classes[class_idx].kind;
        let supertype_ref = program.classes[class_idx].supertype;
        let mixin_refs = program.classes[class_idx].mixins.clone();
        let interface_refs = program.classes[class_idx].interfaces.clone();
        let on_refs = program.classes[class_idx].on.clone();

        // 1. Resolves extends
        let resolved_super = if let Some((unit_id, type_id)) = supertype_ref {
            resolve_type_to_class(program, lib_id, unit_id, type_id, diagnostics)
        } else if class_kind == ClassKind::Class || class_kind == ClassKind::MixinApplication {
            // If omitted and not the Object class: the default is dart:core's Object
            if class_name != object_sym {
                resolve_name_to_class(program, lib_id, None, object_sym, core_id)
            } else {
                None
            }
        } else {
            None
        };
        program.classes[class_idx].supertype_class = resolved_super;

        // 2. Resolve with (mixins)
        let mut resolved_mixins = Vec::new();
        for (unit_id, type_id) in mixin_refs {
            if let Some(cid) = resolve_type_to_class(program, lib_id, unit_id, type_id, diagnostics)
            {
                resolved_mixins.push(cid);
            }
        }
        program.classes[class_idx].mixin_classes = resolved_mixins;

        // 3. Resolves implements (interfaces)
        let mut resolved_interfaces = Vec::new();
        for (unit_id, type_id) in interface_refs {
            if let Some(cid) = resolve_type_to_class(program, lib_id, unit_id, type_id, diagnostics)
            {
                resolved_interfaces.push(cid);
            }
        }
        program.classes[class_idx].interface_classes = resolved_interfaces;

        // 4. Resolves on (for mixins)
        let mut resolved_on = Vec::new();
        for (unit_id, type_id) in on_refs {
            if let Some(cid) = resolve_type_to_class(program, lib_id, unit_id, type_id, diagnostics)
            {
                resolved_on.push(cid);
            }
        }
        program.classes[class_idx].on_classes = resolved_on;

        // 5. Detects cycles in the supertype chain
        let mut visited = HashSet::new();
        visited.insert(class_id);
        let mut curr = resolved_super;
        while let Some(next_id) = curr {
            if !visited.insert(next_id) {
                let name_str = interner.resolve(class_name);
                let span = program.classes[class_idx]
                    .decl
                    .map(|d| program.units[d.unit.0 as usize].ast.decl(d.decl).span)
                    .unwrap_or(Span { start: 0, end: 0 });
                diagnostics.push(Diagnostic::new(
                    format!("inheritance cycle detected in class '{name_str}'"),
                    span,
                ));
                break;
            }
            curr = program.classes[next_id.0 as usize].supertype_class;
        }
    }
    compute_homonym_owners(program);
    program.tempos.outline_supertipos += t_phase.elapsed();
}

/// T1 (docs/ANALYZER-ESPECIFICACAO.md §G, T1.1 d): the owner of the
/// interface and hierarchy caches of each group of homonymous classes, that
/// is, the declaration the analyzer consults first (the cache is per
/// element, and homonyms are the same element as a key). Resolution walks
/// the library's declarations in source order: the first class of the
/// group with `this` or `super` in its body asks for its own interface; a
/// declaration outside the group that mentions the name before that asks
/// for that of the element the name designates. With no request during
/// resolution, the inheritance checker asks for the first in source order
/// (the representative). Mentioning the name counts as a request (an
/// approximation: the analyzer only asks on member access).
fn compute_homonym_owners(program: &mut Program) {
    let inside = |a: Span, b: Span| a.start >= b.start && a.end <= b.end;
    for li in 0..program.libraries.len() {
        if program.libraries[li].representante.is_empty() {
            continue;
        }
        let mut groups: HashMap<ClassId, Vec<ClassId>> = HashMap::new();
        for (e, r) in &program.libraries[li].representante {
            if let (Element::Class(c), Element::Class(rc)) = (e, r) {
                groups.entry(*rc).or_default().push(*c);
            }
        }
        let mut owners: HashMap<ClassId, ClassId> = HashMap::new();
        for (rep, members_) in groups {
            let name_ = program.classes[rep.0 as usize].name;
            let designated = match program.libraries[li].declared.get(&name_).and_then(|d| d.getter) {
                Some(Element::Class(c)) if members_.contains(&c) => Some(c),
                _ => None,
            };
            let mut owner = None;
            'unidades: for &u in &program.libraries[li].units {
                let unidade = &program.units[u.0 as usize];
                let a = &unidade.ast;
                for &d in &unidade.unit.declarations {
                    let span = a.decl(d).span;
                    let of_group = members_
                        .iter()
                        .copied()
                        .find(|&c| program.classes[c.0 as usize].decl.is_some_and(|r| r.unit == u && r.decl == d));
                    match of_group {
                        Some(c) => {
                            let asks = a
                                .exprs
                                .iter()
                                .any(|e| matches!(e.kind, ast::ExprKind::This | ast::ExprKind::Super) && inside(e.span, span));
                            if asks {
                                owner = Some(c);
                                break 'unidades;
                            }
                        }
                        None => {
                            let Some(designated) = designated else { continue };
                            let quote_char = a
                                .exprs
                                .iter()
                                .any(|e| matches!(&e.kind, ast::ExprKind::Identifier(n) if n.sym == name_) && inside(e.span, span))
                                || a.types.iter().any(|t| {
                                    matches!(&t.kind, ast::TypeKind::Named { name, .. } if name.len() == 1 && name[0].sym == name_)
                                        && inside(t.span, span)
                                });
                            if quote_char {
                                owner = Some(designated);
                                break 'unidades;
                            }
                        }
                    }
                }
            }
            let owner = owner.unwrap_or(rep);
            for c in members_ {
                owners.insert(c, owner);
            }
        }
        program.libraries[li].dono_do_grupo = owners;
    }
}

/// The per-unit scopes of a library with imports in parts
/// (`accepted/future-releases/parts-with-imports`, "Scopes"): a file's
/// import scope is its imports, whose parent is the import scope of the
/// file that included it; the inner ones hide the outer ones (bare name and
/// prefix). The library's declarations hide every import, and the implicit
/// `dart:core` has the lowest precedence. Checked against the CFE and the
/// 3.13.4 analyzer (`--enable-experiment=enhanced-parts`): in a part that
/// imports `b.dart`, `name()` is `b`'s even with the library file importing
/// another `name` from `a.dart`.
///
/// Empty when only the library file imports: then the library scope
/// applies to every unit.
fn escopos_de_unidade(program: &Program, lib_id: LibraryId) -> HashMap<UnitId, UnitScope> {
    let lib = &program.libraries[lib_id.0 as usize];
    if lib.is_sdk {
        return patch_scopes(program, lib);
    }
    let mut output = HashMap::new();
    let with_imports_in_part = lib
        .imports
        .iter()
        .any(|i| program.units[i.unit.0 as usize].role != UnitRole::Library);
    if lib.is_sdk || !with_imports_in_part {
        return output;
    }
    // Each file's imports, filtered by the combinators.
    let mut without_prefix: HashMap<UnitId, Namespace> = HashMap::new();
    let mut with_prefix: HashMap<UnitId, HashMap<SymbolId, (Namespace, LibraryId)>> = HashMap::new();
    for import in &lib.imports {
        let exported_ = &program.libraries[import.library.0 as usize].exported;
        let filtered_ = filter_namespace(exported_, &import.combinators);
        match import.prefix {
            Some(p) => {
                let (ns, owner) = with_prefix.entry(import.unit).or_default().entry(p).or_insert_with(|| (Namespace::new(), import.library));
                *owner = import.library;
                for (sym, b) in filtered_ {
                    merge_binding_with(ns.entry(sym).or_default(), b, &|e| sdk_element(program, e));
                }
            }
            None => {
                let ns = without_prefix.entry(import.unit).or_default();
                for (sym, b) in filtered_ {
                    merge_binding_with(ns.entry(sym).or_default(), b, &|e| sdk_element(program, e));
                }
            }
        }
    }
    for &unidade in &lib.units {
        if program.units[unidade.0 as usize].role == UnitRole::Patch {
            continue;
        }
        // The chain from the file up to the library's, from outside in.
        let mut chain = vec![unidade];
        let mut current = unidade;
        while let Some(&parent) = lib.pais.get(&current) {
            if chain.contains(&parent) {
                break;
            }
            chain.push(parent);
            current = parent;
        }
        chain.reverse();
        let mut scope_ = UnitScope { scope: lib.declared.clone(), prefixes: HashMap::new() };
        for file in &chain {
            if let Some(ns) = without_prefix.get(file) {
                for (sym, b) in ns {
                    if lib.declared.contains_key(sym) {
                        continue;
                    }
                    scope_.scope.insert(*sym, *b);
                }
            }
            if let Some(ps) = with_prefix.get(file) {
                for (p, (ns, owner)) in ps {
                    scope_.prefixes.insert(*p, ns.clone());
                    scope_.scope.insert(*p, Binding { getter: Some(Element::Prefix(*owner, *p)), setter: None, ambiguous: false });
                }
            }
        }
        if let Some(core_id) = program.core {
            if lib_id != core_id {
                for (sym, b) in &program.libraries[core_id.0 as usize].exported {
                    scope_.scope.entry(*sym).or_insert(*b);
                }
            }
        }
        output.insert(unidade, scope_);
    }
    output
}

/// The scopes of an SDK library's patch files that import: as in the CFE, a
/// patch's imports apply in it and hide the library's —
/// `js_interop_unsafe_patch.dart` imports `dart:_foreign_helper show JS` and
/// `dart:js_interop hide JS`, and the `JS(...)` in the body is the
/// intrinsic, not the `@JS` annotation the library file brings. Names the
/// patch does not import fall to the library scope (declarations, imports,
/// `dart:core`).
fn patch_scopes(program: &Program, lib: &crate::model::Library) -> HashMap<UnitId, UnitScope> {
    let mut output: HashMap<UnitId, UnitScope> = HashMap::new();
    for import in &lib.imports {
        if program.units[import.unit.0 as usize].role != UnitRole::Patch {
            continue;
        }
        let scope_ = output
            .entry(import.unit)
            .or_insert_with(|| UnitScope { scope: HashMap::new(), prefixes: HashMap::new() });
        let exported_ = &program.libraries[import.library.0 as usize].exported;
        let filtered_ = filter_namespace(exported_, &import.combinators);
        match import.prefix {
            Some(p) => {
                let ns = scope_.prefixes.entry(p).or_default();
                for (sym, b) in filtered_ {
                    merge_binding_with(ns.entry(sym).or_default(), b, &|e| sdk_element(program, e));
                }
                scope_.scope.insert(p, Binding { getter: Some(Element::Prefix(import.library, p)), setter: None, ambiguous: false });
            }
            None => {
                for (sym, b) in filtered_ {
                    if lib.declared.contains_key(&sym) {
                        continue;
                    }
                    merge_binding_with(scope_.scope.entry(sym).or_default(), b, &|e| sdk_element(program, e));
                }
            }
        }
    }
    for scope_ in output.values_mut() {
        for (sym, b) in &lib.scope {
            scope_.scope.entry(*sym).or_insert(*b);
        }
        for (p, ns) in &lib.prefixes {
            scope_.prefixes.entry(*p).or_insert_with(|| ns.clone());
        }
    }
    output
}

// ---------------------------------------------------------------------------
// Element building helpers
// ---------------------------------------------------------------------------

fn create_class_element(
    pools: &mut ElementPools,
    ast: &dartforge_frontend::ast::Ast,
    lib_id: LibraryId,
    unit_id: UnitId,
    decl_id: ast::DeclId,
    c: &ast::ClassDecl,
    empty_sym: SymbolId,
    interner: &mut Interner,
) -> ClassId {
    let class_id = ClassId(pools.classes.len() as u32);
    let type_params = extract_type_params(&c.type_params, unit_id);
    let supertype = c.extends.map(|t| (unit_id, t));
    let mixins = c.with.iter().map(|t| (unit_id, *t)).collect();
    let interfaces = c.implements.iter().map(|t| (unit_id, *t)).collect();

    let mut elem = ClassElement {
        name: c.name.sym,
        library: lib_id,
        decl: Some(DeclRef {
            unit: unit_id,
            decl: decl_id,
        }),
        kind: if c.mixin_application {
            ClassKind::MixinApplication
        } else {
            ClassKind::Class
        },
        modifiers: c.modifiers,
        type_params,
        supertype,
        mixins,
        interfaces,
        on: Vec::new(),
        supertype_class: None,
        mixin_classes: Vec::new(),
        interface_classes: Vec::new(),
        on_classes: Vec::new(),
        instance_members: HashMap::new(),
        instancia_sobrepostos: Vec::new(),
        static_members: HashMap::new(),
        constructors: std::collections::BTreeMap::new(),
        fields: Vec::new(),
        enum_constants: Vec::new(),
        representation: None,
    };

    extract_members(
        pools, ast, &mut elem, class_id, unit_id, &c.members, empty_sym, interner,
    );

    // Default synthetic constructor in case none was declared
    if elem.constructors.is_empty() && !elem.modifiers.abstract_ && elem.kind == ClassKind::Class {
        let synth_id = FunctionElementId(pools.functions.len() as u32);
        pools.functions.push(FunctionElement {
            name: empty_sym,
            library: lib_id,
            class: Some(class_id),
            extension: None,
            kind: FunctionKind::SyntheticConstructor,
            static_: false,
            abstract_: false,
            external: false,
            const_: false,
            factory: false,
            node: FunctionRef::None,
            variable: None,
            patched_by: None,
            declaracao_publica: None,
        });
        elem.constructors.insert(empty_sym, synth_id);
    }

    pools.classes.push(elem);
    class_id
}

fn create_mixin_element(
    pools: &mut ElementPools,
    ast: &dartforge_frontend::ast::Ast,
    lib_id: LibraryId,
    unit_id: UnitId,
    decl_id: ast::DeclId,
    m: &ast::MixinDecl,
    empty_sym: SymbolId,
    interner: &mut Interner,
) -> ClassId {
    let class_id = ClassId(pools.classes.len() as u32);
    let type_params = extract_type_params(&m.type_params, unit_id);
    let on = m.on.iter().map(|t| (unit_id, *t)).collect();
    let interfaces = m.implements.iter().map(|t| (unit_id, *t)).collect();

    let mut elem = ClassElement {
        name: m.name.sym,
        library: lib_id,
        decl: Some(DeclRef {
            unit: unit_id,
            decl: decl_id,
        }),
        kind: ClassKind::Mixin,
        modifiers: ast::ClassModifiers {
            base: m.base,
            ..Default::default()
        },
        type_params,
        supertype: None,
        mixins: Vec::new(),
        interfaces,
        on,
        supertype_class: None,
        mixin_classes: Vec::new(),
        interface_classes: Vec::new(),
        on_classes: Vec::new(),
        instance_members: HashMap::new(),
        instancia_sobrepostos: Vec::new(),
        static_members: HashMap::new(),
        constructors: std::collections::BTreeMap::new(),
        fields: Vec::new(),
        enum_constants: Vec::new(),
        representation: None,
    };

    extract_members(
        pools, ast, &mut elem, class_id, unit_id, &m.members, empty_sym, interner,
    );
    pools.classes.push(elem);
    class_id
}

fn create_enum_element(
    pools: &mut ElementPools,
    ast: &dartforge_frontend::ast::Ast,
    lib_id: LibraryId,
    unit_id: UnitId,
    decl_id: ast::DeclId,
    e: &ast::EnumDecl,
    empty_sym: SymbolId,
    interner: &mut Interner,
) -> ClassId {
    let class_id = ClassId(pools.classes.len() as u32);
    let type_params = extract_type_params(&e.type_params, unit_id);
    let mixins = e.with.iter().map(|t| (unit_id, *t)).collect();
    let interfaces = e.implements.iter().map(|t| (unit_id, *t)).collect();

    let mut elem = ClassElement {
        name: e.name.sym,
        library: lib_id,
        decl: Some(DeclRef {
            unit: unit_id,
            decl: decl_id,
        }),
        kind: ClassKind::Enum,
        modifiers: Default::default(),
        type_params,
        supertype: None,
        mixins,
        interfaces,
        on: Vec::new(),
        supertype_class: None,
        mixin_classes: Vec::new(),
        interface_classes: Vec::new(),
        on_classes: Vec::new(),
        instance_members: HashMap::new(),
        instancia_sobrepostos: Vec::new(),
        static_members: HashMap::new(),
        constructors: std::collections::BTreeMap::new(),
        fields: Vec::new(),
        enum_constants: Vec::new(),
        representation: None,
    };

    // Synthetic enum members: values, index, name
    let values_sym = interner.intern("values");
    let index_sym = interner.intern("index");
    let name_sym = interner.intern("name");

    let values_fn = FunctionElementId(pools.functions.len() as u32);
    pools.functions.push(FunctionElement {
        name: values_sym,
        library: lib_id,
        class: Some(class_id),
        extension: None,
        kind: FunctionKind::Getter,
        static_: true,
        abstract_: false,
        external: false,
        const_: true,
        factory: false,
        node: FunctionRef::None,
        variable: None,
        patched_by: None,
        declaracao_publica: None,
    });
    elem.static_members.insert(values_sym, values_fn);

    let index_fn = FunctionElementId(pools.functions.len() as u32);
    pools.functions.push(FunctionElement {
        name: index_sym,
        library: lib_id,
        class: Some(class_id),
        extension: None,
        kind: FunctionKind::Getter,
        static_: false,
        abstract_: false,
        external: false,
        const_: false,
        factory: false,
        node: FunctionRef::None,
        variable: None,
        patched_by: None,
        declaracao_publica: None,
    });
    elem.instance_members.insert(index_sym, index_fn);

    let name_fn = FunctionElementId(pools.functions.len() as u32);
    pools.functions.push(FunctionElement {
        name: name_sym,
        library: lib_id,
        class: Some(class_id),
        extension: None,
        kind: FunctionKind::Getter,
        static_: false,
        abstract_: false,
        external: false,
        const_: false,
        factory: false,
        node: FunctionRef::None,
        variable: None,
        patched_by: None,
        declaracao_publica: None,
    });
    elem.instance_members.insert(name_sym, name_fn);

    // The enum's constants
    for (idx, ec) in e.constants.iter().enumerate() {
        let const_id = VariableId(pools.variables.len() as u32);
        pools.variables.push(VariableElement {
            name: ec.name.sym,
            library: lib_id,
            class: Some(class_id),
            extension: None,
            static_: true,
            final_: true,
            const_: true,
            late: false,
            external: false,
            node: VariableRef::EnumConstant {
                unit: unit_id,
                decl: decl_id,
                index: idx,
            },
            getter: None,
            setter: None,
        });
        elem.enum_constants.push(const_id);
    }

    extract_members(
        pools, ast, &mut elem, class_id, unit_id, &e.members, empty_sym, interner,
    );
    pools.classes.push(elem);
    class_id
}

fn create_extension_element(
    pools: &mut ElementPools,
    ast: &dartforge_frontend::ast::Ast,
    lib_id: LibraryId,
    unit_id: UnitId,
    decl_id: ast::DeclId,
    ext: &ast::ExtensionDecl,
    _empty_sym: SymbolId,
    interner: &mut Interner,
) -> ExtensionId {
    let ext_id = ExtensionId(pools.extensions.len() as u32);
    let type_params = extract_type_params(&ext.type_params, unit_id);

    let mut ext_elem = ExtensionElement {
        name: ext.name.map(|n| n.sym),
        library: lib_id,
        decl: DeclRef {
            unit: unit_id,
            decl: decl_id,
        },
        type_params,
        on: (unit_id, ext.on),
        instance_members: HashMap::new(),
        static_members: HashMap::new(),
        fields: Vec::new(),
    };

    extension_members(pools, ast, ext_id, lib_id, unit_id, ext, &mut ext_elem, false, interner);
    pools.extensions.push(ext_elem);
    ext_id
}

/// Merges the members of a `@patch extension` into extension `ext_id`.
fn merge_extension_patch(
    pools: &mut ElementPools,
    ast: &dartforge_frontend::ast::Ast,
    ext_id: ExtensionId,
    unit_id: UnitId,
    ext: &ast::ExtensionDecl,
    is_patch: bool,
    interner: &mut Interner,
) {
    // The element leaves the pool while the members go in (both are borrowed
    // mutably).
    let mut elem = std::mem::replace(
        &mut pools.extensions[ext_id.0 as usize],
        ExtensionElement {
            name: None,
            library: LibraryId(0),
            decl: DeclRef { unit: unit_id, decl: ast::DeclId(0) },
            type_params: Vec::new(),
            on: (unit_id, ext.on),
            instance_members: HashMap::new(),
            static_members: HashMap::new(),
            fields: Vec::new(),
        },
    );
    let lib_id = elem.library;
    extension_members(pools, ast, ext_id, lib_id, unit_id, ext, &mut elem, is_patch, interner);
    pools.extensions[ext_id.0 as usize] = elem;
}

#[allow(clippy::too_many_arguments)]
fn extension_members(
    pools: &mut ElementPools,
    ast: &dartforge_frontend::ast::Ast,
    ext_id: ExtensionId,
    lib_id: LibraryId,
    unit_id: UnitId,
    ext: &ast::ExtensionDecl,
    ext_elem: &mut ExtensionElement,
    is_patch: bool,
    interner: &mut Interner,
) {
    for &member_id in &ext.members {
        let member = ast.member(member_id);
        match &member.kind {
            MemberKind::Method(fid) => {
                let ast_fn = ast.function(*fid);
                if let Some(name) = ast_fn.name {
                    let mut fn_sym = name.sym;
                    let fn_kind = match ast_fn.kind {
                        ast::FunctionKind::Getter => FunctionKind::Getter,
                        ast::FunctionKind::Setter => FunctionKind::Setter,
                        ast::FunctionKind::Operator => FunctionKind::Operator,
                        _ => FunctionKind::Function,
                    };
                    // `operator -()` without parameters is unary minus: key `unary-`.
                    if fn_kind == FunctionKind::Operator
                        && interner.resolve(fn_sym) == "-"
                        && ast_fn.parameters.as_ref().is_some_and(|p| p.is_empty())
                    {
                        fn_sym = interner.intern("unary-");
                    }
                    let fn_id = FunctionElementId(pools.functions.len() as u32);
                    pools.functions.push(FunctionElement {
                        name: fn_sym,
                        library: lib_id,
                        class: None,
                        extension: Some(ext_id),
                        kind: fn_kind,
                        static_: ast_fn.static_,
                        abstract_: false,
                        external: ast_fn.external,
                        const_: false,
                        factory: false,
                        node: FunctionRef::Function {
                            unit: unit_id,
                            function: *fid,
                        },
                        variable: None,
                        patched_by: None,
                        declaracao_publica: None,
                    });
                    let key = if fn_kind == FunctionKind::Setter {
                        let setter_str = format!("{}_=", interner.resolve(fn_sym));
                        interner.intern(&setter_str)
                    } else {
                        fn_sym
                    };
                    let members_ = if ast_fn.static_ {
                        &mut ext_elem.static_members
                    } else {
                        &mut ext_elem.instance_members
                    };
                    // A `@patch` member replaces the declaration's `external` one.
                    let old_ = members_.insert(key, fn_id);
                    if is_patch && let Some(old_) = old_ {
                        pools.functions[old_.0 as usize].patched_by = Some(fn_id);
                        pools.functions[fn_id.0 as usize].declaracao_publica = Some(pools.functions[old_.0 as usize].declaracao_publica.unwrap_or(old_));
                    }
                }
            }
            MemberKind::Field(vars) => {
                for (idx, var) in vars.variables.iter().enumerate() {
                    let var_id = VariableId(pools.variables.len() as u32);
                    // The instance field (the `external` one, like `dart:ffi`'s
                    // `IntAddress.address`; the other is `EXTENSION_DECLARES_INSTANCE_FIELD`)
                    // has an implicit getter and, if not `final`, setter, instance members of
                    // the extension (`FieldElementImpl` with synthetic accessors). Static ones
                    // are still read through the variable.
                    let (getter, setter) = if vars.static_ {
                        (None, None)
                    } else {
                        let accessor = |pools: &mut ElementPools| {
                            let id = FunctionElementId(pools.functions.len() as u32);
                            pools.functions.push(FunctionElement {
                                name: var.name.sym,
                                library: lib_id,
                                class: None,
                                extension: Some(ext_id),
                                kind: FunctionKind::ImplicitAccessor,
                                static_: false,
                                abstract_: false,
                                external: vars.external,
                                const_: vars.const_,
                                factory: false,
                                node: FunctionRef::None,
                                variable: Some(var_id),
                                patched_by: None,
                                declaracao_publica: None,
                            });
                            id
                        };
                        let g = accessor(pools);
                        ext_elem.instance_members.insert(var.name.sym, g);
                        let s = if !vars.final_ && !vars.const_ {
                            let s = accessor(pools);
                            let chave = interner.intern(&format!("{}_=", interner.resolve(var.name.sym)));
                            ext_elem.instance_members.insert(chave, s);
                            Some(s)
                        } else {
                            None
                        };
                        (Some(g), s)
                    };
                    pools.variables.push(VariableElement {
                        name: var.name.sym,
                        library: lib_id,
                        class: None,
                        extension: Some(ext_id),
                        static_: vars.static_,
                        final_: vars.final_,
                        const_: vars.const_,
                        late: vars.late,
                        external: vars.external,
                        node: VariableRef::Field {
                            unit: unit_id,
                            member: member_id,
                            index: idx,
                        },
                        getter,
                        setter,
                    });
                    ext_elem.fields.push(var_id);
                }
            }
            _ => {}
        }
    }

}

fn create_extension_type_element(
    pools: &mut ElementPools,
    ast: &dartforge_frontend::ast::Ast,
    lib_id: LibraryId,
    unit_id: UnitId,
    decl_id: ast::DeclId,
    et: &ast::ExtensionTypeDecl,
    empty_sym: SymbolId,
    interner: &mut Interner,
) -> ClassId {
    let class_id = ClassId(pools.classes.len() as u32);
    let type_params = extract_type_params(&et.type_params, unit_id);
    let interfaces = et.implements.iter().map(|t| (unit_id, *t)).collect();

    // The representation is handled as a special field
    let rep_id = VariableId(pools.variables.len() as u32);
    pools.variables.push(VariableElement {
        name: et.representation_name.sym,
        library: lib_id,
        class: Some(class_id),
        extension: None,
        static_: false,
        final_: true,
        const_: false,
        late: false,
        external: false,
        node: VariableRef::Representation {
            unit: unit_id,
            decl: decl_id,
        },
        getter: None,
        setter: None,
    });

    let mut elem = ClassElement {
        name: et.name.sym,
        library: lib_id,
        decl: Some(DeclRef {
            unit: unit_id,
            decl: decl_id,
        }),
        kind: ClassKind::ExtensionType,
        modifiers: Default::default(),
        type_params,
        supertype: None,
        mixins: Vec::new(),
        interfaces,
        on: Vec::new(),
        supertype_class: None,
        mixin_classes: Vec::new(),
        interface_classes: Vec::new(),
        on_classes: Vec::new(),
        instance_members: HashMap::new(),
        instancia_sobrepostos: Vec::new(),
        static_members: HashMap::new(),
        constructors: std::collections::BTreeMap::new(),
        fields: vec![rep_id],
        enum_constants: Vec::new(),
        representation: Some(rep_id),
    };

    extract_members(
        pools,
        ast,
        &mut elem,
        class_id,
        unit_id,
        &et.members,
        empty_sym,
        interner,
    );
    pools.classes.push(elem);
    class_id
}

fn extract_type_params(
    params: &[ast::TypeParameter],
    unit_id: UnitId,
) -> Vec<TypeParameterElement> {
    params
        .iter()
        .map(|p| TypeParameterElement {
            name: p.name.sym,
            bound: p.bound.map(|b| (unit_id, b)),
        })
        .collect()
}

/// Extracts methods, constructors and fields of a class/mixin/enum.
pub(crate) fn extract_members(
    pools: &mut ElementPools,
    ast: &dartforge_frontend::ast::Ast,
    elem: &mut ClassElement,
    class_id: ClassId,
    unit_id: UnitId,
    member_ids: &[ast::MemberId],
    empty_sym: SymbolId,
    interner: &mut Interner,
) {
    let lib_id = elem.library;

    for &mid in member_ids {
        let member = ast.member(mid);
        match &member.kind {
            MemberKind::Method(fid) => {
                let ast_fn = ast.function(*fid);
                if let Some(name) = ast_fn.name {
                    let mut fn_sym = name.sym;
                    let fn_kind = match ast_fn.kind {
                        ast::FunctionKind::Getter => FunctionKind::Getter,
                        ast::FunctionKind::Setter => FunctionKind::Setter,
                        ast::FunctionKind::Operator => FunctionKind::Operator,
                        _ => FunctionKind::Function,
                    };
                    // `operator -()` without parameters is unary minus: key `unary-`.
                    if fn_kind == FunctionKind::Operator
                        && interner.resolve(fn_sym) == "-"
                        && ast_fn.parameters.as_ref().is_some_and(|p| p.is_empty())
                    {
                        fn_sym = interner.intern("unary-");
                    }
                    let fn_id = FunctionElementId(pools.functions.len() as u32);
                    pools.functions.push(FunctionElement {
                        name: fn_sym,
                        library: lib_id,
                        class: Some(class_id),
                        extension: None,
                        kind: fn_kind,
                        static_: ast_fn.static_,
                        abstract_: matches!(ast_fn.body, ast::FunctionBody::Empty)
                            && !ast_fn.external,
                        external: ast_fn.external,
                        const_: false,
                        factory: false,
                        node: FunctionRef::Function {
                            unit: unit_id,
                            function: *fid,
                        },
                        variable: None,
                        patched_by: None,
                        declaracao_publica: None,
                    });

                    let key = if fn_kind == FunctionKind::Setter {
                        let setter_str = format!("{}_=", interner.resolve(fn_sym));
                        interner.intern(&setter_str)
                    } else {
                        fn_sym
                    };

                    if ast_fn.static_ {
                        elem.static_members.insert(key, fn_id);
                    } else if let Some(old) = elem.instance_members.insert(key, fn_id) {
                        elem.instancia_sobrepostos.push((key, old));
                    }
                }
            }
            MemberKind::Constructor(ctor) => {
                // `C.new(…)` declares the unnamed constructor (the name `new` is the
                // element's `''`).
                let ctor_sym = ctor.name.map(|n| n.sym).filter(|s| interner.resolve(*s) != "new").unwrap_or(empty_sym);
                let fn_id = FunctionElementId(pools.functions.len() as u32);
                pools.functions.push(FunctionElement {
                    name: ctor_sym,
                    library: lib_id,
                    class: Some(class_id),
                    extension: None,
                    kind: FunctionKind::Constructor,
                    static_: true,
                    abstract_: false,
                    external: ctor.external,
                    const_: ctor.const_,
                    factory: ctor.factory,
                    node: FunctionRef::Constructor {
                        unit: unit_id,
                        member: mid,
                    },
                    variable: None,
                    patched_by: None,
                    declaracao_publica: None,
                });
                // Repeated constructor (`DUPLICATE_CONSTRUCTOR`): the name resolves to the
                // first one declared (`InterfaceElement.getNamedConstructor`, the first of
                // the list).
                elem.constructors.entry(ctor_sym).or_insert(fn_id);
            }
            MemberKind::Field(vars) => {
                for (idx, var) in vars.variables.iter().enumerate() {
                    let var_id = VariableId(pools.variables.len() as u32);
                    let var_sym = var.name.sym;

                    let getter_id = FunctionElementId(pools.functions.len() as u32);
                    pools.functions.push(FunctionElement {
                        name: var_sym,
                        library: lib_id,
                        class: Some(class_id),
                        extension: None,
                        kind: FunctionKind::ImplicitAccessor,
                        static_: vars.static_,
                        abstract_: vars.abstract_,
                        external: vars.external,
                        const_: vars.const_,
                        factory: false,
                        node: FunctionRef::None,
                        variable: Some(var_id),
                        patched_by: None,
                        declaracao_publica: None,
                    });

                    let setter_id = if !vars.final_ && !vars.const_ {
                        let sid = FunctionElementId(pools.functions.len() as u32);
                        pools.functions.push(FunctionElement {
                            name: var_sym,
                            library: lib_id,
                            class: Some(class_id),
                            extension: None,
                            kind: FunctionKind::ImplicitAccessor,
                            static_: vars.static_,
                            abstract_: vars.abstract_,
                            external: vars.external,
                            const_: vars.const_,
                            factory: false,
                            node: FunctionRef::None,
                            variable: Some(var_id),
                            patched_by: None,
                            declaracao_publica: None,
                        });
                        Some(sid)
                    } else {
                        None
                    };

                    pools.variables.push(VariableElement {
                        name: var_sym,
                        library: lib_id,
                        class: Some(class_id),
                        extension: None,
                        static_: vars.static_,
                        final_: vars.final_,
                        const_: vars.const_,
                        late: vars.late,
                        external: vars.external,
                        node: VariableRef::Field {
                            unit: unit_id,
                            member: mid,
                            index: idx,
                        },
                        getter: Some(getter_id),
                        setter: setter_id,
                    });
                    elem.fields.push(var_id);

                    if vars.static_ {
                        elem.static_members.insert(var_sym, getter_id);
                        if let Some(sid) = setter_id {
                            let setter_key =
                                interner.intern(&format!("{}_=", interner.resolve(var_sym)));
                            elem.static_members.insert(setter_key, sid);
                        }
                    } else {
                        if let Some(old) = elem.instance_members.insert(var_sym, getter_id) {
                            elem.instancia_sobrepostos.push((var_sym, old));
                        }
                        if let Some(sid) = setter_id {
                            let setter_key =
                                interner.intern(&format!("{}_=", interner.resolve(var_sym)));
                            if let Some(old) = elem.instance_members.insert(setter_key, sid) {
                                elem.instancia_sobrepostos.push((setter_key, old));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Applies a patch to the members of an existing class.
fn merge_class_patch(
    pools: &mut ElementPools,
    ast: &dartforge_frontend::ast::Ast,
    class_id: ClassId,
    unit_id: UnitId,
    member_ids: &[ast::MemberId],
    _is_patch: bool,
    interner: &mut Interner,
) {
    let lib_id = pools.classes[class_id.0 as usize].library;
    let empty_sym = interner.intern("");

    for &mid in member_ids {
        let member = ast.member(mid);
        match &member.kind {
            MemberKind::Method(fid) => {
                let ast_fn = ast.function(*fid);
                if let Some(name) = ast_fn.name {
                    let mut fn_sym = name.sym;
                    let fn_kind = match ast_fn.kind {
                        ast::FunctionKind::Getter => FunctionKind::Getter,
                        ast::FunctionKind::Setter => FunctionKind::Setter,
                        ast::FunctionKind::Operator => FunctionKind::Operator,
                        _ => FunctionKind::Function,
                    };
                    // `operator -()` without parameters is unary minus: key `unary-`.
                    if fn_kind == FunctionKind::Operator
                        && interner.resolve(fn_sym) == "-"
                        && ast_fn.parameters.as_ref().is_some_and(|p| p.is_empty())
                    {
                        fn_sym = interner.intern("unary-");
                    }
                    let fn_id = FunctionElementId(pools.functions.len() as u32);
                    pools.functions.push(FunctionElement {
                        name: fn_sym,
                        library: lib_id,
                        class: Some(class_id),
                        extension: None,
                        kind: fn_kind,
                        static_: ast_fn.static_,
                        abstract_: false,
                        external: ast_fn.external,
                        const_: false,
                        factory: false,
                        node: FunctionRef::Function {
                            unit: unit_id,
                            function: *fid,
                        },
                        variable: None,
                        patched_by: None,
                        declaracao_publica: None,
                    });

                    let key = if fn_kind == FunctionKind::Setter {
                        let setter_str = format!("{}_=", interner.resolve(fn_sym));
                        interner.intern(&setter_str)
                    } else {
                        fn_sym
                    };

                    let target_map = if ast_fn.static_ {
                        &mut pools.classes[class_id.0 as usize].static_members
                    } else {
                        &mut pools.classes[class_id.0 as usize].instance_members
                    };

                    if let Some(&existing_fn_id) = target_map.get(&key) {
                        // A member replaces an external member
                        pools.functions[existing_fn_id.0 as usize].patched_by = Some(fn_id);
                        pools.functions[fn_id.0 as usize].declaracao_publica = Some(pools.functions[existing_fn_id.0 as usize].declaracao_publica.unwrap_or(existing_fn_id));
                        target_map.insert(key, fn_id);
                    } else {
                        // A new member is added
                        target_map.insert(key, fn_id);
                    }
                }
            }
            MemberKind::Constructor(ctor) => {
                // `C.new(…)` declares the unnamed constructor (the name `new` is the
                // element's `''`).
                let ctor_sym = ctor.name.map(|n| n.sym).filter(|s| interner.resolve(*s) != "new").unwrap_or(empty_sym);
                let fn_id = FunctionElementId(pools.functions.len() as u32);
                pools.functions.push(FunctionElement {
                    name: ctor_sym,
                    library: lib_id,
                    class: Some(class_id),
                    extension: None,
                    kind: FunctionKind::Constructor,
                    static_: true,
                    abstract_: false,
                    external: ctor.external,
                    const_: ctor.const_,
                    factory: ctor.factory,
                    node: FunctionRef::Constructor {
                        unit: unit_id,
                        member: mid,
                    },
                    variable: None,
                    patched_by: None,
                    declaracao_publica: None,
                });
                let constructors = &mut pools.classes[class_id.0 as usize].constructors;
                if let Some(&existing_ctor_id) = constructors.get(&ctor_sym) {
                    pools.functions[existing_ctor_id.0 as usize].patched_by = Some(fn_id);
                    pools.functions[fn_id.0 as usize].declaracao_publica = Some(pools.functions[existing_ctor_id.0 as usize].declaracao_publica.unwrap_or(existing_ctor_id));
                }
                constructors.insert(ctor_sym, fn_id);
            }
            MemberKind::Field(_) => {
                // Fields declared in the patch (the VM `DateTime`'s `final int _value;`):
                // they go into the source class like the declaration's, through the same
                // path.
                let idx = class_id.0 as usize;
                let c = &mut pools.classes[idx];
                let mut tmp = ClassElement {
                    name: c.name,
                    library: c.library,
                    decl: c.decl,
                    kind: c.kind,
                    modifiers: c.modifiers,
                    type_params: Vec::new(),
                    supertype: None,
                    mixins: Vec::new(),
                    interfaces: Vec::new(),
                    on: Vec::new(),
                    supertype_class: None,
                    mixin_classes: Vec::new(),
                    interface_classes: Vec::new(),
                    on_classes: Vec::new(),
                    instance_members: std::mem::take(&mut c.instance_members),
                    instancia_sobrepostos: std::mem::take(&mut c.instancia_sobrepostos),
                    static_members: std::mem::take(&mut c.static_members),
                    constructors: std::mem::take(&mut c.constructors),
                    fields: std::mem::take(&mut c.fields),
                    enum_constants: Vec::new(),
                    representation: None,
                };
                extract_members(pools, ast, &mut tmp, class_id, unit_id, &[mid], empty_sym, interner);
                let c = &mut pools.classes[idx];
                c.instance_members = tmp.instance_members;
                c.instancia_sobrepostos = tmp.instancia_sobrepostos;
                c.static_members = tmp.static_members;
                c.constructors = tmp.constructors;
                c.fields = tmp.fields;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Namespace filters and combinations
// ---------------------------------------------------------------------------

fn filter_namespace(ns: &Namespace, combinators: &[ast::Combinator]) -> Namespace {
    let mut out = ns.clone();
    for comb in combinators {
        match comb {
            ast::Combinator::Show(names) => {
                let show_set: HashSet<SymbolId> = names.iter().map(|n| n.sym).collect();
                out.retain(|k, _| show_set.contains(k));
            }
            ast::Combinator::Hide(names) => {
                let hide_set: HashSet<SymbolId> = names.iter().map(|n| n.sym).collect();
                out.retain(|k, _| !hide_set.contains(k));
            }
        }
    }
    out
}

/// The SDK library that declares the element (for the conflict rule).
fn sdk_element(program: &Program, el: Element) -> bool {
    let lib = match el {
        Element::Class(c) => program.classes[c.0 as usize].library,
        Element::Extension(e) => program.extensions[e.0 as usize].library,
        Element::Typedef(t) => program.typedefs[t.0 as usize].library,
        Element::Function(f) => program.functions[f.0 as usize].library,
        Element::Variable(v) => program.variables[v.0 as usize].library,
        Element::Prefix(..) => return false,
    };
    program.libraries[lib.0 as usize].is_sdk
}

/// Merges two imports of the same name. Dart rule (specification,
/// "Imports"): if one comes from a system library (`dart:`) and the other
/// does not, the system one is hidden — there is no ambiguity.
fn merge_binding_with(entry: &mut Binding, incoming: Binding, from_sdk: &dyn Fn(Element) -> bool) {
    let join = |current: &mut Option<Element>, new_: Option<Element>, ambiguous: &mut bool| match (*current, new_) {
        (None, n) => *current = n,
        (Some(a), Some(n)) if a != n => match (from_sdk(a), from_sdk(n)) {
            (true, false) => *current = Some(n),
            (false, true) => {}
            _ => *ambiguous = true,
        },
        _ => {}
    };
    let mut ambiguous = entry.ambiguous;
    join(&mut entry.getter, incoming.getter, &mut ambiguous);
    join(&mut entry.setter, incoming.setter, &mut ambiguous);
    entry.ambiguous = ambiguous;

    if incoming.ambiguous {
        entry.ambiguous = true;
    }
}

// ---------------------------------------------------------------------------
// Resolving types and names to ClassId
// ---------------------------------------------------------------------------

fn resolve_type_to_class(
    program: &Program,
    lib_id: LibraryId,
    unit_id: UnitId,
    type_id: ast::TypeId,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ClassId> {
    let ast_ty = program.units[unit_id.0 as usize].ast.ty(type_id);
    match &ast_ty.kind {
        ast::TypeKind::Named { name, .. } => {
            if name.is_empty() {
                return None;
            }
            if name.len() == 1 {
                resolve_name_to_class(program, lib_id, Some(unit_id), name[0].sym, program.core)
            } else if name.len() == 2 {
                let prefix_sym = name[0].sym;
                let target_sym = name[1].sym;
                program
                    .lookup_prefixed_na_unidade(unit_id, prefix_sym, target_sym)
                    .and_then(|b| {
                        if b.ambiguous {
                            diagnostics.push(Diagnostic::new(
                                "ambiguous reference in a prefixed supertype",
                                name[1].span,
                            ));
                            None
                        } else {
                            match b.getter {
                                Some(Element::Class(cid)) => Some(cid),
                                Some(Element::Typedef(tid)) => typedef_class(program, tid, 0),
                                _ => None,
                            }
                        }
                    })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The class a `typedef X = C<…>;` names, for an alias used as a supertype,
/// mixin or interface (`with MapMixin<K, V>`, with
/// `typedef MapMixin<K, V> = MapBase<K, V>` in `dart:collection`). Follows
/// aliases of aliases up to a small depth; a type alias that is not a class
/// gives `None`.
fn typedef_class(program: &Program, tid: TypedefId, depth: u32) -> Option<ClassId> {
    if depth > 8 {
        return None;
    }
    let t = program.typedef(tid);
    let ast = &program.unit(t.decl.unit).ast;
    let ast::DeclKind::Typedef(d) = &ast.decl(t.decl.decl).kind else { return None };
    let ast::TypedefKind::Alias(ty) = d.kind else { return None };
    let ast::TypeKind::Named { name, .. } = &ast.ty(ty).kind else { return None };
    let b = match &name[..] {
        [n] => program.lookup_na_unidade(t.decl.unit, n.sym),
        [p, n] => program.lookup_prefixed_na_unidade(t.decl.unit, p.sym, n.sym),
        _ => None,
    }?;
    match b.getter {
        Some(Element::Class(cid)) => Some(cid),
        Some(Element::Typedef(other)) => typedef_class(program, other, depth + 1),
        _ => None,
    }
}

/// `unit`: the unit where the name is written (its scope, with
/// `parts-with-imports`); `None` uses the library's.
fn resolve_name_to_class(
    program: &Program,
    lib_id: LibraryId,
    unit: Option<UnitId>,
    name_sym: SymbolId,
    core_id: Option<LibraryId>,
) -> Option<ClassId> {
    let found = match unit {
        Some(u) => program.lookup_na_unidade(u, name_sym),
        None => program.lookup(lib_id, name_sym),
    };
    if let Some(b) = found {
        if !b.ambiguous {
            if let Some(Element::Class(cid)) = b.getter {
                return Some(cid);
            }
            if let Some(Element::Typedef(tid)) = b.getter {
                return typedef_class(program, tid, 0);
            }
        }
    }
    // Tries the core fallback directly if it is not in the scope
    if let Some(cid) = core_id {
        if let Some(b) = program.libraries[cid.0 as usize].exported.get(&name_sym) {
            if let Some(Element::Class(cid)) = b.getter {
                return Some(cid);
            }
        }
    }
    None
}
