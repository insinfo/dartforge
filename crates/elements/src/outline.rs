//! Construção do outline e dos namespaces (Passo 2 da Meta Governante).
//!
//! Conforme docs/FRONTEND-ARQUITETURA.md §3 e PLANO.md:
//! 1. Coleta declarações de topo de todas as unidades (incluindo patches).
//! 2. Aplica `@patch` do SDK sobre declarações e membros de classes `external`.
//! 3. Constrói o namespace exportado com resolução de ponto fixo para reexports transitivos.
//! 4. Constrói o escopo léxico de cada biblioteca (declarações locais > imports > core implícito).
//! 5. Resolve nomes de supertipos (`extends`, `with`, `implements`, `on`) para `ClassId`
//!    e detecta ciclos de herança.
use crate::model::*;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// Verifica se um identificador é privado (inicia com `_`).
pub fn is_private(sym: SymbolId, interner: &Interner) -> bool {
    interner.resolve(sym).starts_with('_')
}

/// Verifica se uma lista de anotações contém `@patch`.
fn has_patch_annotation(metadata: &[ast::Annotation], interner: &Interner) -> bool {
    metadata
        .iter()
        .any(|m| m.name.len() == 1 && interner.resolve(m.name[0].sym) == "patch")
}

/// Pools de elementos mutáveis separados de units e libraries para satisfazer o borrow checker.
pub(crate) struct ElementPools<'a> {
    pub(crate) classes: &'a mut Vec<ClassElement>,
    pub(crate) extensions: &'a mut Vec<ExtensionElement>,
    pub(crate) functions: &'a mut Vec<FunctionElement>,
    pub(crate) variables: &'a mut Vec<VariableElement>,
}

/// Constrói o outline completo do programa: declarações, membros, namespaces e supertipos.
pub fn build_outline(
    program: &mut Program,
    interner: &mut Interner,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let empty_sym = interner.intern("");
    let mut t_fase = std::time::Instant::now();

    // -----------------------------------------------------------------------
    // Fase 1: Coleta de elementos declarados por biblioteca
    // -----------------------------------------------------------------------
    let num_libs = program.libraries.len();
    for lib_idx in 0..num_libs {
        let lib_id = LibraryId(lib_idx as u32);
        let mut unit_ids = program.libraries[lib_idx].units.clone();
        // Garante que unidades de origem (Library e Part) sejam declaradas antes dos patches
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
        for unit_id in unit_ids {
            let role = program.units[unit_id.0 as usize].role;
            let decl_ids = program.units[unit_id.0 as usize].unit.declarations.clone();
            let ast = &program.units[unit_id.0 as usize].ast;

            for decl_id in decl_ids {
                let decl = ast.decl(decl_id);
                // `augment`: liga-se à declaração de mesmo nome que veio antes
                // (docs/AUGMENTATIONS.md). As unidades estão na ordem de
                // aplicação (pré-ordem da árvore de partes, `load.rs`).
                if decl.augment && role != UnitRole::Patch {
                    let mut fusao = crate::augmentation::Fusao {
                        units: &program.units,
                        library: &mut program.libraries[lib_idx],
                        lib_id,
                        empty_sym,
                        interner,
                        diagnostics,
                        cadeias: &mut cadeias,
                    };
                    crate::augmentation::aplicar(&mut fusao, &mut pools, unit_id, decl_id);
                    continue;
                }
                let is_patch =
                    role == UnitRole::Patch && has_patch_annotation(&decl.metadata, interner);

                match &decl.kind {
                    DeclKind::Class(c) => {
                        let class_name = c.name.sym;
                        // Se for patch ou classe em patch com mesmo nome já existente na biblioteca: funde
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
                            // Funde membros do patch na classe existente
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
                            let entry = program.libraries[lib_idx]
                                .declared
                                .entry(class_name)
                                .or_default();
                            entry.getter = Some(Element::Class(class_id));
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
                    }
                    DeclKind::Extension(ext) => {
                        let ext_id = create_extension_element(
                            &mut pools, ast, lib_id, unit_id, decl_id, ext, empty_sym, interner,
                        );
                        if let Some(name) = ext.name {
                            let entry = program.libraries[lib_idx]
                                .declared
                                .entry(name.sym)
                                .or_default();
                            entry.getter = Some(Element::Extension(ext_id));
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
                                        }
                                    }
                                    entry.getter = Some(Element::Function(fn_id));
                                }
                                FunctionKind::Setter => {
                                    if is_patch && entry.setter.is_some() {
                                        if let Some(Element::Function(old_id)) = entry.setter {
                                            pools.functions[old_id.0 as usize].patched_by =
                                                Some(fn_id);
                                        }
                                    }
                                    entry.setter = Some(Element::Function(fn_id));
                                }
                                _ => {
                                    if is_patch && entry.getter.is_some() {
                                        if let Some(Element::Function(old_id)) = entry.getter {
                                            pools.functions[old_id.0 as usize].patched_by =
                                                Some(fn_id);
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

                            // Getter implícito
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
                            });

                            // Setter implícito se não for const/final
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
                            if setter_id.is_some() {
                                entry.setter = Some(Element::Variable(var_id));
                            }
                        }
                    }
                }
            }
        }
        for (c, d) in cadeias {
            program.augmentacoes.entry(c).or_default().push(d);
        }
    }

    // -----------------------------------------------------------------------
    // Fase 2: Construção de Library::exported (com ponto fixo para reexports)
    program.tempos.outline_declaracoes += t_fase.elapsed();
    t_fase = std::time::Instant::now();
    // -----------------------------------------------------------------------
    // Inicializa `exported` com o que é declarado localmente e não privado
    for lib in &mut program.libraries {
        for (&sym, &binding) in &lib.declared {
            if !is_private(sym, interner) {
                lib.exported.insert(sym, binding);
            }
        }
    }

    // Itera reexports transitivos até atingir ponto fixo
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
    // Fase 3: Construção do escopo léxico de cada biblioteca (Library::scope)
    program.tempos.outline_reexports += t_fase.elapsed();
    t_fase = std::time::Instant::now();
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
                // Import com prefixo: acessível via p.membro
                let prefix_ns = prefixes.entry(prefix_sym).or_default();
                for (sym, binding) in filtered {
                    let entry = prefix_ns.entry(sym).or_default();
                    merge_binding_com(entry, binding, &|e| elemento_do_sdk(program, e));
                }
                // Registra o prefixo no escopo geral
                scope.insert(
                    prefix_sym,
                    Binding {
                        getter: Some(Element::Prefix(import.library, prefix_sym)),
                        setter: None,
                        ambiguous: false,
                    },
                );
            } else {
                // Import sem prefixo: vai direto para o escopo
                for (sym, binding) in filtered {
                    // Declaração local sempre vence import
                    if program.libraries[lib_idx].declared.contains_key(&sym) {
                        continue;
                    }
                    let entry = scope.entry(sym).or_default();
                    merge_binding_com(entry, binding, &|e| elemento_do_sdk(program, e));
                }
            }
        }

        // dart:core implícito: menor precedência de todas
        if let Some(core_id) = program.core {
            if LibraryId(lib_idx as u32) != core_id {
                let core_exported = program.libraries[core_id.0 as usize].exported.clone();
                for (sym, binding) in core_exported {
                    // Só entra se nada na biblioteca ou nos imports anteriores ocupou o nome
                    scope.entry(sym).or_insert(binding);
                }
            }
        }

        program.libraries[lib_idx].scope = scope;
        program.libraries[lib_idx].prefixes = prefixes;
    }

    // -----------------------------------------------------------------------
    // Fase 4: Resolução de supertipos e verificação de ciclos
    program.tempos.outline_escopos += t_fase.elapsed();
    t_fase = std::time::Instant::now();
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

        // 1. Resolve extends
        let resolved_super = if let Some((unit_id, type_id)) = supertype_ref {
            resolve_type_to_class(program, lib_id, unit_id, type_id, diagnostics)
        } else if class_kind == ClassKind::Class || class_kind == ClassKind::MixinApplication {
            // Se omitido e não for a classe Object: default é Object de dart:core
            if class_name != object_sym {
                resolve_name_to_class(program, lib_id, object_sym, core_id)
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

        // 3. Resolve implements (interfaces)
        let mut resolved_interfaces = Vec::new();
        for (unit_id, type_id) in interface_refs {
            if let Some(cid) = resolve_type_to_class(program, lib_id, unit_id, type_id, diagnostics)
            {
                resolved_interfaces.push(cid);
            }
        }
        program.classes[class_idx].interface_classes = resolved_interfaces;

        // 4. Resolve on (para mixins)
        let mut resolved_on = Vec::new();
        for (unit_id, type_id) in on_refs {
            if let Some(cid) = resolve_type_to_class(program, lib_id, unit_id, type_id, diagnostics)
            {
                resolved_on.push(cid);
            }
        }
        program.classes[class_idx].on_classes = resolved_on;

        // 5. Detecta ciclos na cadeia de supertipos
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
                    format!("ciclo de herança detectado na classe '{name_str}'"),
                    span,
                ));
                break;
            }
            curr = program.classes[next_id.0 as usize].supertype_class;
        }
    }
    program.tempos.outline_supertipos += t_fase.elapsed();
}

// ---------------------------------------------------------------------------
// Auxiliares de construção de elementos
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
        static_members: HashMap::new(),
        constructors: std::collections::BTreeMap::new(),
        fields: Vec::new(),
        enum_constants: Vec::new(),
        representation: None,
    };

    extract_members(
        pools, ast, &mut elem, class_id, unit_id, &c.members, empty_sym, interner,
    );

    // Construtor sintético padrão caso nenhum tenha sido declarado
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
        static_members: HashMap::new(),
        constructors: std::collections::BTreeMap::new(),
        fields: Vec::new(),
        enum_constants: Vec::new(),
        representation: None,
    };

    // Membros sintéticos de enum: values, index, name
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
    });
    elem.instance_members.insert(name_sym, name_fn);

    // Constantes do enum
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
                    // `operator -()` sem parâmetros é o menos unário: chave `unary-`.
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
                    });
                    let key = if fn_kind == FunctionKind::Setter {
                        let setter_str = format!("{}_=", interner.resolve(fn_sym));
                        interner.intern(&setter_str)
                    } else {
                        fn_sym
                    };
                    if ast_fn.static_ {
                        ext_elem.static_members.insert(key, fn_id);
                    } else {
                        ext_elem.instance_members.insert(key, fn_id);
                    }
                }
            }
            MemberKind::Field(vars) => {
                for (idx, var) in vars.variables.iter().enumerate() {
                    let var_id = VariableId(pools.variables.len() as u32);
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
                        getter: None,
                        setter: None,
                    });
                    ext_elem.fields.push(var_id);
                }
            }
            _ => {}
        }
    }

    pools.extensions.push(ext_elem);
    ext_id
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

    // Representação é tratada como campo especial
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

/// Extrai métodos, construtores e campos de uma classe/mixin/enum.
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
                    // `operator -()` sem parâmetros é o menos unário: chave `unary-`.
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
                    });

                    let key = if fn_kind == FunctionKind::Setter {
                        let setter_str = format!("{}_=", interner.resolve(fn_sym));
                        interner.intern(&setter_str)
                    } else {
                        fn_sym
                    };

                    if ast_fn.static_ {
                        elem.static_members.insert(key, fn_id);
                    } else {
                        elem.instance_members.insert(key, fn_id);
                    }
                }
            }
            MemberKind::Constructor(ctor) => {
                let ctor_sym = ctor.name.map(|n| n.sym).unwrap_or(empty_sym);
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
                });
                elem.constructors.insert(ctor_sym, fn_id);
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
                        elem.instance_members.insert(var_sym, getter_id);
                        if let Some(sid) = setter_id {
                            let setter_key =
                                interner.intern(&format!("{}_=", interner.resolve(var_sym)));
                            elem.instance_members.insert(setter_key, sid);
                        }
                    }
                }
            }
        }
    }
}

/// Aplica patch nos membros de uma classe existente.
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
                    // `operator -()` sem parâmetros é o menos unário: chave `unary-`.
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
                        // Membro substitui membro external
                        pools.functions[existing_fn_id.0 as usize].patched_by = Some(fn_id);
                        target_map.insert(key, fn_id);
                    } else {
                        // Membro novo acrescentado
                        target_map.insert(key, fn_id);
                    }
                }
            }
            MemberKind::Constructor(ctor) => {
                let ctor_sym = ctor.name.map(|n| n.sym).unwrap_or(empty_sym);
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
                });
                let constructors = &mut pools.classes[class_id.0 as usize].constructors;
                if let Some(&existing_ctor_id) = constructors.get(&ctor_sym) {
                    pools.functions[existing_ctor_id.0 as usize].patched_by = Some(fn_id);
                }
                constructors.insert(ctor_sym, fn_id);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Filtros e combinações de namespace
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

/// A biblioteca do SDK que declara o elemento (para a regra de conflito).
fn elemento_do_sdk(program: &Program, el: Element) -> bool {
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

/// Junta dois imports do mesmo nome. Regra do Dart (especificação,
/// "Imports"): se um vem de biblioteca do sistema (`dart:`) e o outro não,
/// o do sistema fica oculto — não há ambiguidade.
fn merge_binding_com(entry: &mut Binding, incoming: Binding, do_sdk: &dyn Fn(Element) -> bool) {
    let juntar = |atual: &mut Option<Element>, novo: Option<Element>, amb: &mut bool| match (*atual, novo) {
        (None, n) => *atual = n,
        (Some(a), Some(n)) if a != n => match (do_sdk(a), do_sdk(n)) {
            (true, false) => *atual = Some(n),
            (false, true) => {}
            _ => *amb = true,
        },
        _ => {}
    };
    let mut amb = entry.ambiguous;
    juntar(&mut entry.getter, incoming.getter, &mut amb);
    juntar(&mut entry.setter, incoming.setter, &mut amb);
    entry.ambiguous = amb;

    if incoming.ambiguous {
        entry.ambiguous = true;
    }
}

// ---------------------------------------------------------------------------
// Resolução de tipos e nomes para ClassId
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
                resolve_name_to_class(program, lib_id, name[0].sym, program.core)
            } else if name.len() == 2 {
                let prefix_sym = name[0].sym;
                let target_sym = name[1].sym;
                program
                    .lookup_prefixed(lib_id, prefix_sym, target_sym)
                    .and_then(|b| {
                        if b.ambiguous {
                            diagnostics.push(Diagnostic::new(
                                "referência ambígua em supertipo prefixado",
                                name[1].span,
                            ));
                            None
                        } else {
                            match b.getter {
                                Some(Element::Class(cid)) => Some(cid),
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

fn resolve_name_to_class(
    program: &Program,
    lib_id: LibraryId,
    name_sym: SymbolId,
    core_id: Option<LibraryId>,
) -> Option<ClassId> {
    if let Some(b) = program.lookup(lib_id, name_sym) {
        if !b.ambiguous {
            if let Some(Element::Class(cid)) = b.getter {
                return Some(cid);
            }
        }
    }
    // Tenta fallback em core diretamente se não estiver no escopo
    if let Some(cid) = core_id {
        if let Some(b) = program.libraries[cid.0 as usize].exported.get(&name_sym) {
            if let Some(Element::Class(cid)) = b.getter {
                return Some(cid);
            }
        }
    }
    None
}
