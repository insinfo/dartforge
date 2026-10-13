//! Lowering do AST e tabelas de tipos para a HIR nativa.
//!
//! O contrato entre este lowering, o emissor e o runtime está em
//! `docs/NATIVO-PLANO.md` §6 (R, E, N, G).

pub mod async_sm;
pub mod assignment;
/// Os membros de caixas, closures e records em linha (P2 do espaço unificado).
pub mod boxes;
pub mod capture;
pub mod cascade;
pub mod calls;
pub mod closures;
pub mod statements;
pub mod primitive_const;
pub mod constants;
pub mod dispatch;
pub mod typed_entry;
pub mod enums;
pub mod runtime_errors;
pub mod expressions;
pub mod extensions;
pub mod ffi;
pub mod externals;
pub mod fn_builder;
pub mod direct_functions;
pub mod inheritance;
/// Os membros de `_List`/`_GrowableList` em linha (P3 do espaço unificado).
pub mod lists;
pub mod literals;
pub mod locals;
pub mod members;
mod layouts_arc;
pub mod operators;
pub mod patterns;
pub mod records;
pub mod rti;
pub mod sdk_source;
pub mod intrinsics;
pub mod simd;
pub mod strings;
pub mod typed_items;
pub mod extension_types;
pub mod verifier;

use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{FunctionKind, FunctionRef, VariableId, VariableRef};
use dartforge_frontend::ast::{FunctionBody, MemberKind};

pub fn sanitize_symbol(name: &str) -> String {
    name.chars().map(|c| match c {
        '+' => "_plus_".to_string(),
        '-' => "_minus_".to_string(),
        '*' => "_mul_".to_string(),
        '/' => "_div_".to_string(),
        '%' => "_mod_".to_string(),
        '~' => "_tilde_".to_string(),
        '<' => "_lt_".to_string(),
        '>' => "_gt_".to_string(),
        '=' => "_eq_".to_string(),
        '!' => "_excl_".to_string(),
        '[' => "_lbr_".to_string(),
        ']' => "_rbr_".to_string(),
        '&' => "_amp_".to_string(),
        '|' => "_pipe_".to_string(),
        '^' => "_caret_".to_string(),
        c if c.is_ascii_alphanumeric() || c == '_' => c.to_string(),
        _ => format!("_x{:02x}_", c as u32),
    }).collect()
}

/// A função é do programa do usuário (não do SDK)?
///
/// Com a seção `vm` carregada de verdade, `Program::functions` tem o
/// `dart:core` inteiro. O backend nativo não compila o SDK a partir da fonte
/// (ainda — `docs/NATIVO-PLANO.md` §4, item 7): o runtime em Rust implementa
/// o que o corpus usa. Então só as funções do usuário viram símbolo, e uma
/// chamada a uma função do SDK que não tem implementação no runtime é
/// diagnóstico (N1), não `call` para um símbolo que não existe.
pub fn user_function(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    ctx.compiled_library(f.library)
}

/// A função é construtor generativo (inclusive o sintético): não devolve
/// nada e recebe `this` já alocado (N5).
pub fn generative_constructor(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    match f.node {
        // O de tipo de extensão devolve a representação, como uma fábrica
        // (`extension_types.rs`).
        FunctionRef::Constructor { .. } => !f.factory && !f.class.is_some_and(|c| ctx.is_extension_type(c)),
        FunctionRef::None => f.kind == FunctionKind::SyntheticConstructor,
        FunctionRef::Function { .. } => false,
    }
}

/// O dono de um membro no símbolo: a classe, a extensão (`ext:<name>`) ou
/// vazio para o topo.
fn owner_in_symbol(ctx: &Context, class: Option<dartforge_elements::model::ClassId>, ext: Option<dartforge_elements::model::ExtensionId>) -> String {
    if let Some(c) = class {
        return ctx.symbol_name(ctx.program.classes[c.0 as usize].name).to_string();
    }
    if let Some(e) = ext {
        let x = ctx.program.extension(e);
        return match x.name {
            Some(n) => format!("ext:{}", ctx.symbol_name(n)),
            None => format!("ext:@{}", x.decl.decl.0),
        };
    }
    String::new()
}

/// Símbolo estável de uma função do programa (P2, docs/NATIVO-PLANO.md
/// §7.4): `df.<library>.<owner>.<member>`, derivado do **caminho** da
/// declaração — nunca de índices internos do front-end —, com cada parte
/// escapada (`context::escape_str`). O membro é o nome; o setter termina em
/// `=`; o construtor é `new` ou `new:<name>`. `main` da biblioteca de
/// entrada é `dart_main`. Inserir ou reordenar declarações não muda o
/// símbolo de nenhuma outra: é a chave do cache de objeto por biblioteca e da
/// recarga (hot reload R1).
pub fn symbol_of(ctx: &Context, fid: usize) -> String {
    let f = &ctx.program.functions[fid];
    let name_str = ctx.symbol_name(f.name);
    if f.class.is_none() && f.extension.is_none() && name_str == "main" && Some(f.library) == ctx.entry_lib {
        return "dart_main".to_string();
    }
    let member = match f.kind {
        FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
            if name_str.is_empty() { "new".to_string() } else { format!("new:{name_str}") }
        }
        FunctionKind::Setter if !name_str.ends_with('=') => format!("{name_str}="),
        _ => name_str.to_string(),
    };
    format!(
        "df.{}.{}.{}",
        crate::context::escape_str(&ctx.library_name(f.library)),
        crate::context::escape_str(&owner_in_symbol(ctx, f.class, f.extension)),
        crate::context::escape_str(&member)
    )
}

/// Variável de topo ou campo estático: mora num global do módulo (N6).
pub fn is_global(ctx: &Context, vid: VariableId) -> bool {
    let v = &ctx.program.variables[vid.0 as usize];
    // Campos de extensão são sempre estáticos (§13): globais como os de
    // classe.
    (v.class.is_none() || v.static_)
        && (v.extension.is_none() || v.static_)
        && matches!(
            v.node,
            VariableRef::TopLevel { .. } | VariableRef::Field { .. } | VariableRef::EnumConstant { .. }
        )
}

/// Caminho estável de uma variável de topo ou campo estático (P2).
fn variable_path(ctx: &Context, vid: VariableId) -> String {
    let v = &ctx.program.variables[vid.0 as usize];
    format!(
        "{}.{}.{}",
        crate::context::escape_str(&ctx.library_name(v.library)),
        crate::context::escape_str(&owner_in_symbol(ctx, v.class, v.extension)),
        crate::context::escape_str(ctx.symbol_name(v.name))
    )
}

/// Getter preguiçoso de um global: `df.<path>`.
pub fn global_symbol(ctx: &Context, vid: VariableId) -> String {
    format!("df.{}", variable_path(ctx, vid))
}

/// O valor de um global: `dfg.<path>` (a bandeira é `<value>$ok`).
pub fn global_value_symbol(ctx: &Context, vid: VariableId) -> String {
    format!("dfg.{}", variable_path(ctx, vid))
}

/// Getter dedicado de um campo de instância `late` com inicializador.
pub fn late_field_getter_symbol(ctx: &Context, vid: VariableId) -> String {
    format!("df.late.{}", variable_path(ctx, vid))
}

pub fn lower_program(ctx: &Context) -> Module {
    let mut module = Module::new();
    module.arc_memory = ctx.arc_memory;

    // 1. Registra classes conhecidas
    // Classe Object padrão como id 0
    module.classes.push(ClassDef {
        id: 0,
        name: "Object".to_string(),
        field_count: 0,
        vtable: Vec::new(),
        to_string_symbol: None,
    });

    // O SDK vem sempre da fonte (docs/NATIVO-ESPACO-UNIFICADO.md §4.7): as
    // classes de erro são as dele (P5c).
    module.sdk_mode = true;
    module.sdk_library = ctx.sdk_library;
    module.dwarf = ctx.dwarf;
    lower_classes_and_functions(ctx, module)
}

/// Os campos do `dart:async` que o rastro simbólico percorre para achar quem
/// espera (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14; os
/// `USED_FIELD_LIST` do `AsyncAwareStackUnwinder` da VM), quando este módulo
/// tem as classes deles: a posição de cada um no objeto.
fn trace_fields(ctx: &Context, module: &mut Module) {
    if !ctx.stack_trace {
        return;
    }
    const FIELDS: &[(&str, &str)] = &[
        ("_AsyncAwaitCompleter", "_future"),
        ("_AsyncCompleter", "future"),
        ("_SyncCompleter", "future"),
        ("_Future", "_resultOrListeners"),
        ("_Future", "_state"),
        ("_FutureListener", "state"),
        ("_FutureListener", "callback"),
        ("_FutureListener", "result"),
        ("_FutureListener", "_nextListener"),
        ("_AsyncStarStreamController", "asyncStarBody"),
        ("_AsyncStarStreamController", "controller"),
        ("_BufferingStreamSubscription", "_onData"),
        ("_BufferingStreamSubscription", "_onDone"),
        ("_BufferingStreamSubscription", "_onError"),
        ("_BufferingStreamSubscription", "_state"),
        ("_StreamController", "_state"),
        ("_StreamController", "_varData"),
        ("_StreamControllerAddStreamState", "_varData"),
        ("_StreamIterator", "_hasValue"),
        ("_StreamIterator", "_stateData"),
        ("_AddStreamState", "addStreamFuture"),
    ];
    // O runtime acha o campo pela classe exata do objeto: a declarante e as
    // subclasses dela (o `_SyncStreamController` de um `_StreamController`,
    // o `_ControllerSubscription` de uma `_BufferingStreamSubscription`),
    // como o `Instance::GetField` da VM.
    for (class, field) in FIELDS {
        let Some(declarer) = ctx.sdk_class("async", class) else { continue };
        for (k, _) in ctx.program.classes.iter().enumerate() {
            let c = dartforge_elements::model::ClassId(k as u32);
            let mut ancestor = Some(c);
            let mut descends = false;
            for _ in 0..64 {
                match ancestor {
                    Some(x) if x == declarer => {
                        descends = true;
                        break;
                    }
                    Some(x) => ancestor = ctx.program.class(x).supertype_class,
                    None => break,
                }
            }
            if !descends || !ctx.library_in_module(ctx.program.class(c).library) {
                continue;
            }
            let Some(id) = ctx.class_id_of(c) else { continue };
            let Some(i) = members::layout(ctx, c).iter().position(|&v| ctx.symbol_name(ctx.program.variables[v.0 as usize].name) == *field) else {
                continue;
            };
            module.trace_fields.push((id, format!("{class}.{field}"), i + enums::layout_base(ctx, c)));
        }
    }
}

/// As entradas de tear-off dos ramos de stream do rastro (§13.14) que este
/// módulo definiu: o `_StreamIterator._onData` (o `await for`) e o
/// `_StreamController._add` (o `yield*`).
fn stack_trace_tearoffs(ctx: &Context, module: &mut Module) {
    if !ctx.stack_trace {
        return;
    }
    const TEAROFFS: &[(&str, &str, i64)] = &[("_StreamIterator", "_onData", 1), ("_StreamController", "_add", 2)];
    for (class, method, kind) in TEAROFFS {
        let Some(c) = ctx.sdk_class("async", class) else { continue };
        let Some(name_str) = ctx.interner.lookup(method) else { continue };
        let Some(&fid) = ctx.program.class(c).instance_members.get(&name_str) else { continue };
        let symbol = format!("{}$tearm", symbol_of(ctx, fid.0 as usize));
        if module.functions.iter().any(|f| f.symbol == symbol) {
            module.stack_trace_tearoffs.push((symbol, *kind));
        }
    }
}

/// Os layouts ARC declarados das classes do SDK da fonte (as que este módulo
/// não emite), como o módulo de cada biblioteca do SDK os registra
/// (`layouts_arc::register`): os `arc_external_layouts` do programa
/// preparado contra o SDK preparado.
pub fn sdk_arc_layouts(ctx: &Context) -> std::collections::HashMap<u32, Vec<Type>> {
    let mut m = Module::new();
    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        if ctx.library_in_module(class.library) || !ctx.program.library(class.library).is_sdk {
            continue;
        }
        let cid = dartforge_elements::model::ClassId(c_idx as u32);
        let Some(class_id) = ctx.class_id_of(cid) else { continue };
        layouts_arc::register(ctx, &mut m, class_id, &members::layout(ctx, cid), enums::layout_base(ctx, cid));
    }
    m.arc_field_layouts
}

// Compartilhado com a auditoria dos fatos de layout, sem baixar corpos.
fn register_program_classes(ctx: &Context, module: &mut Module) {
    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        // Registra as classes das bibliotecas emitidas neste módulo. O layout
        // declarado do SDK não certifica a forma física usada pelo runtime.
        if !ctx.library_in_module(class.library) {
            continue;
        }
        let name = ctx.symbol_name(class.name).to_string();
        // Id estável (P2): a ordem do caminho da classe, não a de carga.
        let Some(class_id) = ctx.class_id_of(dartforge_elements::model::ClassId(c_idx as u32)) else {
            continue;
        };
        let cid = dartforge_elements::model::ClassId(c_idx as u32);
        let fields = members::layout(ctx, cid);
        let base = enums::layout_base(ctx, cid);
        layouts_arc::register(ctx, module, class_id, &fields, base);
        module.classes.push(ClassDef {
            id: class_id,
            name,
            field_count: fields.len() + base,
            vtable: Vec::new(),
            to_string_symbol: None,
        });

        // Registra arestas de subtipagem
        module.subtyping_edges.push((class_id, 0)); // Todo tipo nominal herda de Object
        if let Some(sup) = class.supertype_class.and_then(|s| ctx.class_id_of(s)) {
            module.subtyping_edges.push((class_id, sup));
        }
        // Enum do programa é subtipo do `Enum` do SDK (especificação §13):
        // sem a aresta, `value is Enum` respondia falso. A superclasse do
        // outline não carrega o `Enum`, então a aresta é registrada aqui.
        if !ctx.program.library(class.library).is_sdk
            && enums::is_enum(ctx, dartforge_elements::model::ClassId(c_idx as u32))
            && let Some(enum_sdk) = ctx.sdk_class("core", "Enum")
            && let Some(enum_id) = ctx.class_id_of(enum_sdk)
        {
            module.subtyping_edges.push((class_id, enum_id));
        }
        for mix in &class.mixin_classes {
            if let Some(m) = ctx.class_id_of(*mix) {
                module.subtyping_edges.push((class_id, m));
            }
        }
        for iface in &class.interface_classes {
            if let Some(i) = ctx.class_id_of(*iface) {
                module.subtyping_edges.push((class_id, i));
            }
        }
    }
}

fn lower_classes_and_functions(ctx: &Context, mut module: Module) -> Module {
    trace_fields(ctx, &mut module);
    register_program_classes(ctx, &mut module);

    // 2. Funções do usuário
    let functions: Vec<usize> = (0..ctx.program.functions.len())
        .filter(|&f| ctx.library_in_module(ctx.program.functions[f].library))
        .collect();
    lower_functions(ctx, &mut module, &functions);
    sdk_source::lower_adapters_and_tables(ctx, &mut module);
    let mut module = lower_globals_and_rest(ctx, module);
    sdk_source::record_shape_tables(ctx, &mut module);
    if !ctx.sdk_library {
        ffi::lower_ffi(ctx, &mut module);
    }
    stack_trace_tearoffs(ctx, &mut module);
    crate::optimize::arc::analysis::origins::register(&mut module, 0, ctx.arc_memory);
    module
}

/// Baixa uma função do módulo: a do SDK da fonte (P5c) é recusada sozinha,
/// com o motivo, quando não baixa (`sdk_source.rs`).
fn lower_one(ctx: &Context, module: &mut Module, f_idx: usize) {
    if ctx.program.library(ctx.program.functions[f_idx].library).is_sdk {
        sdk_source::lower_function_or_reject(ctx, module, f_idx);
    } else {
        lower_function(ctx, module, f_idx);
    }
}

/// Quantas threads baixam as funções: as da máquina, no máximo 4
/// (`DARTFORGE_THREADS_HIR`; 1 = sequencial).
fn lowering_threads() -> usize {
    std::env::var("DARTFORGE_THREADS_HIR")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(4)))
        .max(1)
}

/// Baixa as funções `functions` para `module`. O programa grande vai em
/// pedaços contíguos, um por thread, cada um num módulo próprio, juntados
/// na ordem: o resultado é o da ordem sequencial (o mesmo IR, as mesmas
/// chaves no cache de objetos). Cada função só escreve no módulo as próprias
/// funções, globais, erros e recusas, o `toString` da classe dela e a
/// entrada (`main`); o contexto só tem memórias de consulta.
fn lower_functions(ctx: &Context, module: &mut Module, functions: &[usize]) {
    let n = lowering_threads();
    if n == 1 || functions.len() < 4096 {
        for &f in functions {
            lower_one(ctx, module, f);
        }
        return;
    }
    // Pedaços pequenos distribuídos sob demanda (as funções caras se
    // agrupam), cada um no seu módulo; a junção segue a ordem dos pedaços.
    let sdk_mode = module.sdk_mode;
    let sdk_library = module.sdk_library;
    let classes = &module.classes;
    let chunks: Vec<&[usize]> = functions.chunks(512).collect();
    let done_set: Vec<std::sync::Mutex<Option<Module>>> = chunks.iter().map(|_| std::sync::Mutex::new(None)).collect();
    let next_item = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        let tasks: Vec<_> = (0..n)
            .map(|_| {
                let (chunks, done_set, next_item) = (&chunks, &done_set, &next_item);
                std::thread::Builder::new()
                    // A mesma pilha da thread da compilação (`lib.rs`).
                    .stack_size(64 << 20)
                    .spawn_scoped(s, move || loop {
                        let i = next_item.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(chunk) = chunks.get(i) else { break };
                        let mut m = Module::new();
                        m.sdk_mode = sdk_mode;
                        m.sdk_library = sdk_library;
                        m.classes = classes.clone();
                        for &f in chunk.iter() {
                            lower_one(ctx, &mut m, f);
                        }
                        *done_set[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
                    })
                    .expect("thread do lowering")
            })
            .collect();
        for t in tasks {
            if let Err(p) = t.join() {
                std::panic::resume_unwind(p);
            }
        }
    });
    let parts = done_set.into_iter().map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()).expect("pedaço baixado"));
    for mut m in parts {
        module.functions.extend(std::mem::take(&mut m.functions));
        join_arc_facts(module, &mut m);
        module.globals.extend(m.globals);
        module.errors.extend(m.errors);
        module.source_errors.extend(m.source_errors);
        module.rejected.extend(m.rejected);
        module.helpers.extend(m.helpers);
        if m.entry_symbol.is_some() {
            module.entry_symbol = m.entry_symbol;
            module.entry_params = m.entry_params;
        }
        for (c, d) in module.classes.iter_mut().zip(m.classes.iter()) {
            if c.to_string_symbol.is_none() && d.to_string_symbol.is_some() {
                c.to_string_symbol.clone_from(&d.to_string_symbol);
            }
        }
    }
}

/// P6: os símbolos das funções da fonte que têm corpo — um `external` cujo
/// patch o carregador não ligou (`patched_by` vazio, NATIVO-PEDIDOS) tem o
/// mesmo símbolo do membro do patch, e quem vale é o patch. Calculado uma vez
/// por contexto.
fn with_source_body<'c>(ctx: &'c Context) -> &'c std::collections::HashSet<String> {
    ctx.with_source_body.get_or_init(|| {
        ctx.program
            .functions
            .iter()
            .enumerate()
            .filter(|(i, f)| ctx.from_source.contains(&f.library) && members::has_body(ctx, *i))
            .map(|(i, _)| symbol_of(ctx, i))
            .collect()
    })
}

/// Junta a `dest` os fatos ARC que o lowering de `origin` produziu (por
/// símbolo e por ID SSA da mesma função): parâmetros escalares/RTI/vetores,
/// retornos Ref e uniformes, caches RTI, funções compartilhadas e locais
/// diretas, e sítios de alocação. Um pedaço baixado à parte que perde algum
/// deles muda a convenção ARC da função em silêncio — o retorno Ref sem
/// `dart_ref_returns` deixava de ser Owned.
pub(crate) fn join_arc_facts(dest: &mut Module, origin: &mut Module) {
    dest.dart_scalar_parameters.extend(std::mem::take(&mut origin.dart_scalar_parameters));
    dest.dart_rti_parameters.extend(std::mem::take(&mut origin.dart_rti_parameters));
    dest.dart_vector_parameters.extend(std::mem::take(&mut origin.dart_vector_parameters));
    dest.arc_uniform_returns.extend(std::mem::take(&mut origin.arc_uniform_returns));
    dest.arc_rti_caches.extend(std::mem::take(&mut origin.arc_rti_caches));
    dest.arc_shared_functions.extend(std::mem::take(&mut origin.arc_shared_functions));
    dest.arc_direct_local_functions.extend(std::mem::take(&mut origin.arc_direct_local_functions));
    dest.dart_ref_returns.extend(std::mem::take(&mut origin.dart_ref_returns));
    dest.arc_sites.extend(std::mem::take(&mut origin.arc_sites));
    dest.dart_scalar_globals.extend(std::mem::take(&mut origin.dart_scalar_globals));
}

/// Baixa uma função (de topo, método, construtor) para o módulo.
pub fn lower_function(ctx: &Context, module: &mut Module, f_idx: usize) {
    let start = module.functions.len();
    let symbol = symbol_of(ctx, f_idx);
    lower_one_function(ctx, module, f_idx);
    crate::optimize::arc::analysis::origins::register(module, start, ctx.arc_memory);
    // A convenção pertence ao corpo Dart identificado pelo front-end, não à
    // largura de retornos de thunks/FFI ou de funções extras criadas no corpo.
    if ctx.arc_memory
        && module.functions[start..].iter().any(|f| f.symbol == symbol && f.return_ty == Type::Ref)
    {
        module.dart_ref_returns.insert(symbol);
    }
}

fn lower_one_function(ctx: &Context, module: &mut Module, f_idx: usize) {
    let func_elem = &ctx.program.functions[f_idx];
    if func_elem.external {
        // `external` (inclusive construtor): o corpo é o do patch ou o
        // native (`sdk_source::call_external`); nada a baixar aqui — um
        // corpo vazio com o mesmo símbolo tomaria o lugar do patch.
        return;
    }
    {
        let name = ctx.symbol_name(func_elem.name);
        let symbol = symbol_of(ctx, f_idx);

        match func_elem.node {
            FunctionRef::Function { unit, function } => {
                let ast = &ctx.program.unit(unit).ast;
                let ast_func = ast.function(function);
                if matches!(ast_func.body, FunctionBody::Empty | FunctionBody::Native(_)) {
                    // P6: `external` de biblioteca da fonte com native — o
                    // corpo é a chamada ao runtime (`externals.rs`).
                    if func_elem.external
                        && func_elem.patched_by.is_none()
                        && ctx.from_source.contains(&func_elem.library)
                        && !with_source_body(ctx).contains(&symbol)
                    {
                        let ret_ty = ctx
                            .outline
                            .functions
                            .get(f_idx)
                            .map_or(Type::Void, |d| ctx.to_hir_type(d.return_type));
                        let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret_ty);
                        let is_instance = func_elem.class.is_some() && !func_elem.static_;
                        builder.declare_parameters(f_idx, is_instance);
                        if !builder.lower_external(f_idx, ast_func.span) {
                            builder.unsupported(&format!("external `{name}` sem native"), ast_func.span);
                            builder.terminate(Terminator::Return(None));
                        }
                        builder.finish(module);
                    }
                    // Abstrato ou externo: não há corpo a compilar.
                    return;
                }

                let is_main = symbol == "dart_main";
                if is_main {
                    module.entry_symbol = Some(symbol.clone());
                    module.entry_params = ast_func.parameters.as_deref().map_or(0, <[_]>::len);
                }

                if let Some(c_id) = func_elem.class
                    && name == "toString"
                    && !func_elem.static_
                    && let Some(id) = ctx.class_id_of(c_id)
                    && let Some(c) = module.classes.iter_mut().find(|c| c.id == id)
                {
                    c.to_string_symbol = Some(symbol.clone());
                }

                let ret_ty = if is_main {
                    Type::Void
                } else if ctx.outline.functions.get(f_idx).is_some() {
                    ctx.hir_return(f_idx)
                } else {
                    Type::Void
                };

                let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret_ty);
                // O rastro simbólico (§13.14): o token da função, os apoios
                // ocultos do `async_patch` e `_FutureListener.handleValue`,
                // cujo ouvinte começa a cadeia de quem espera.
                if ctx.stack_trace {
                    builder.stack_trace.token = ast_func.name.and_then(|n| ctx.line_and_column(unit, n.span.start));
                    if crate::source::has_pragma(ctx.program, ctx.interner, f_idx, "dartforge:rastro-oculto") {
                        builder.stack_trace.marks |= crate::hir::stack_trace_marks::HIDDEN;
                    }
                    if name == "handleValue"
                        && ctx.program.library(func_elem.library).uri == "dart:async"
                        && func_elem.class.is_some_and(|c| ctx.symbol_name(ctx.program.class(c).name) == "_FutureListener")
                    {
                        builder.stack_trace.marks |= crate::hir::stack_trace_marks::LISTEN;
                    }
                }
                // A análise de capturas percorre o corpo inteiro; o corpo
                // podado (C7) não o usa.
                if !ctx.pruned_function(f_idx) {
                    builder.prepare_captures(
                        ast,
                        capture::Root {
                            parameters: ast_func.parameters.as_deref().unwrap_or(&[]),
                            body: Some(&ast_func.body),
                            initializers: &[],
                        },
                    );
                }
                let is_instance_member = func_elem.class.is_some() && !func_elem.static_;
                if let Some(e) = func_elem.extension.filter(|_| !func_elem.static_) {
                    // Membro de instância de extensão (P4): o receptor é o
                    // primeiro parâmetro, na representação do tipo `on` (com
                    // fato de parâmetro escalar quando ela é escalar).
                    let r = builder.extension_receiver_repr(e);
                    let this = builder.add_param_dart("this".to_string(), r);
                    builder.this_param = Some(Operand::Val(this));
                    builder.finalizable_this = ctx.classify_this(f_idx);
                    builder.declare_parameters(f_idx, false);
                    let on = ctx.outline.extensions[e.0 as usize].on;
                    builder.this_extension = Some((e, on));
                    if let dartforge_types::table::Type::Interface { class, .. } = ctx.table.get(on)
                        && ctx.compiled_library(ctx.program.classes[class.0 as usize].library)
                    {
                        builder.enclosing_class = Some(*class);
                    }
                } else if let Some(c) = extension_types::instance_owner(ctx, f_idx) {
                    // Membro de instância de tipo de extensão: o receptor é o
                    // primeiro parâmetro, na representação; os parâmetros de
                    // tipo do tipo vêm na tupla (`extension_types.rs`).
                    let r = builder.static_this_repr(f_idx).unwrap_or(Type::Ref);
                    let this = builder.add_param_dart("this".to_string(), r);
                    builder.this_param = Some(Operand::Val(this));
                    builder.finalizable_this = ctx.classify_this(f_idx);
                    builder.declare_parameters(f_idx, false);
                    builder.this_ext_type = ctx.te.this.get(&c).map(|&t| (c, t));
                } else {
                    builder.declare_parameters(f_idx, is_instance_member);
                }
                if is_instance_member && extension_types::instance_owner(ctx, f_idx).is_none() {
                    builder.enclosing_class = func_elem.class;
                }
                builder.member_class = func_elem.class;
                if builder.stack_trace.marks & crate::hir::stack_trace_marks::LISTEN != 0
                    && let Some(t) = builder.this_param.clone()
                {
                    builder.stack_trace.stack = Some((1, t));
                }
                // RTI: a função genérica recebe a tupla dos argumentos de
                // tipo no último parâmetro (`M<i>` das receitas).
                if builder.generic_function(f_idx) {
                    let t = builder.add_param_rti("$tipos".to_string());
                    builder.type_tuple = Some(Operand::Val(t));
                    builder.function_type_params =
                        builder.type_params_of(f_idx).iter().map(|&p| ctx.table.param(p).name).collect();
                }

                // Fora do mundo fechado do programa (C7, `native_world.rs`):
                // o símbolo fica (tabelas e entradas o citam), o corpo lança.
                if ctx.pruned_function(f_idx) {
                    builder.pruned_body();
                    builder.finish(module);
                    return;
                }
                match ast_func.modifier {
                    // P6: o corpo `async` vira máquina de estados
                    // (`async_sm.rs`); os geradores `sync*`/`async*` também.
                    dartforge_frontend::ast::AsyncModifier::None => match &ast_func.body {
                        FunctionBody::Block(stmt_id) => {
                            builder.lower_stmt(ast, *stmt_id);
                        }
                        FunctionBody::Expression(expr_id) => {
                            let ret_op = builder.lower_expr(ast, *expr_id);
                            builder.terminate(Terminator::Return(Some(ret_op)));
                        }
                        _ => {}
                    },
                    modifier => {
                        builder.lower_async_body(
                            ast,
                            ast_func.parameters.as_deref().unwrap_or(&[]),
                            &ast_func.body,
                            ast_func.span,
                            ctx.outline.functions.get(f_idx).map(|d| async_sm::AsyncReturn::Tipo(d.return_type)),
                            async_sm::body_type(modifier),
                        );
                    }
                }
                builder.finish(module);
            }
            FunctionRef::Constructor { unit, member } => {
                let ast = &ctx.program.unit(unit).ast;
                let MemberKind::Constructor(ctor) = &ast.member(member).kind else {
                    return;
                };
                let Some(cid) = func_elem.class else { return };
                if !func_elem.factory && ctx.is_extension_type(cid) {
                    // Construtor generativo de tipo de extensão: devolve a
                    // representação (`extension_types.rs`).
                    let ret = ctx.te.this.get(&cid).map_or(Type::Ref, |&t| ctx.to_hir_type(t));
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret);
                    builder.prepare_captures(
                        ast,
                        capture::Root {
                            parameters: &ctor.parameters,
                            body: Some(&ctor.body),
                            initializers: &ctor.initializers,
                        },
                    );
                    builder.declare_parameters(f_idx, false);
                    builder.lower_constructor_te(ast, cid, ctor, ast.member(member).span);
                    builder.finish(module);
                    return;
                }
                if func_elem.factory {
                    // A fábrica de tipo de extensão devolve a representação.
                    let ret = if ctx.is_extension_type(cid) {
                        ctx.te.this.get(&cid).map_or(Type::Ref, |&t| ctx.to_hir_type(t))
                    } else {
                        Type::Ref
                    };
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret);
                    builder.prepare_captures(
                        ast,
                        capture::Root {
                            parameters: &ctor.parameters,
                            body: Some(&ctor.body),
                            initializers: &ctor.initializers,
                        },
                    );
                    builder.declare_parameters(f_idx, false);
                    // RTI: a fábrica de classe genérica recebe os argumentos
                    // de tipo da classe na tupla (não há `this`).
                    builder.enclosing_class = Some(cid);
                    if builder.generic_class(cid) {
                        let t = builder.add_param_rti("$tipos".to_string());
                        builder.type_tuple = Some(Operand::Val(t));
                        builder.class_by_tuple = true;
                    }
                    if let Some(r) = &ctor.redirect {
                        // `factory C(…) = D.name;` (P4): os parâmetros passam
                        // como estão (posicionais em ordem, nomeados pelo
                        // nome) para o construtor alvo.
                        let span = ast.member(member).span;
                        let alvo = builder.type_constructor(ast.ty(r.ty), r.constructor.map(|n| n.sym));
                        match alvo {
                            Some(t) => {
                                let mut evaluated = Vec::new();
                                for (i, p) in ctx.outline.functions[f_idx].parameters.iter().enumerate() {
                                    let Some(sym) = p.name else { continue };
                                    let v = builder.read_local_by_name(sym).unwrap_or(Operand::Constant(Constant::Null));
                                    let name_str = (ctor.parameters.get(i).map(|q| q.kind)
                                        == Some(dartforge_frontend::ast::ParameterKind::Named))
                                    .then_some(sym);
                                    evaluated.push((name_str, v));
                                }
                                // `factory C<T>(…) = C<T>._` deve entregar os
                                // parâmetros reificados ao construtor alvo.
                                // Sem isso, `MapEntry<K,V>` nasce como
                                // `MapEntry<dynamic,dynamic>` mesmo quando a
                                // fábrica recebeu `L<K,V>` pela ABI.
                                let target_class = ctx.program.functions[t.0 as usize].class;
                                if target_class == Some(cid) {
                                    builder.creation_type = Some(ctx.outline.functions[f_idx].return_type);
                                }
                                let (target_rti, target_tuple) = if target_class
                                    .is_some_and(|alvo| builder.generic_class(alvo))
                                {
                                    let annotation = ast.ty(r.ty);
                                    let rti = builder.annotation_recipe(annotation)
                                        .map(|recipe| builder.recipe_rti(&recipe));
                                    let tuple = builder.annotation_tuple(annotation);
                                    (rti, tuple)
                                } else {
                                    (None, None)
                                };
                                let r = builder.instantiate_evaluated_with_rti(
                                    t, &evaluated, span, target_rti, target_tuple,
                                );
                                builder.terminate(Terminator::Return(Some(r)));
                            }
                            // `factory E.x(R v) = E;` para o primário de um
                            // tipo de extensão: o próprio argumento.
                            None if ctx.is_extension_type(cid)
                                && extension_types::primary_name(ctx, cid)
                                    == r.constructor.map(|n| n.sym).or_else(|| ctx.interner.lookup(""))
                                && let Some(v) = ctx.outline.functions[f_idx]
                                    .parameters
                                    .first()
                                    .and_then(|p| p.name)
                                    .and_then(|n| builder.read_local_by_name(n)) =>
                            {
                                let ret = builder.func.return_ty;
                                let v = builder.coerce(v, ret);
                                builder.terminate(Terminator::Return(Some(v)));
                            }
                            None => {
                                builder.unsupported("factory redirecionadora", span);
                                builder.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
                            }
                        }
                    } else {
                        match &ctor.body {
                            FunctionBody::Block(stmt_id) => builder.lower_stmt(ast, *stmt_id),
                            FunctionBody::Expression(expr_id) => {
                                let r = builder.lower_expr(ast, *expr_id);
                                builder.terminate(Terminator::Return(Some(r)));
                            }
                            _ => {}
                        }
                    }
                    builder.finish(module);
                } else {
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), Type::Void);
                    builder.prepare_captures(
                        ast,
                        capture::Root {
                            parameters: &ctor.parameters,
                            body: Some(&ctor.body),
                            initializers: &ctor.initializers,
                        },
                    );
                    builder.declare_parameters(f_idx, true);
                    builder.enclosing_class = Some(cid);
                    builder.lower_constructor(ast, cid, ctor, ast.member(member).span);
                    builder.finish(module);
                }
            }
            FunctionRef::None => {
                // Construtor padrão sintético (`class A { int x = 1; }`):
                // inicializadores de campo e `super()` implícito.
                if func_elem.kind != FunctionKind::SyntheticConstructor {
                    return;
                }
                let Some(cid) = func_elem.class else { return };
                let Some(decl) = ctx.program.classes[cid.0 as usize].decl else { return };
                let mut builder = fn_builder::FnBuilder::new(ctx, decl.unit, symbol, name.to_string(), Type::Void);
                builder.declare_parameters(f_idx, true);
                builder.enclosing_class = Some(cid);
                let ast = &ctx.program.unit(decl.unit).ast;
                builder.lower_synthetic_constructor(ast, cid);
                builder.finish(module);
            }
        }
    }
}

fn lower_globals_and_rest(ctx: &Context, mut module: Module) -> Module {
    // O inicializador não pode ser expandido em cada ponto de leitura: uma
    // auto-referência deve chamar o getter em runtime e observar reentrância.
    for (v_idx, v) in ctx.program.variables.iter().enumerate() {
        let vid = VariableId(v_idx as u32);
        let VariableRef::Field { unit, member, index } = v.node else { continue };
        if !ctx.library_in_module(v.library) || !v.late || v.static_ || v.class.is_none() {
            continue;
        }
        let MemberKind::Field(list) = &ctx.program.unit(unit).ast.member(member).kind else { continue };
        if list.variables.get(index).and_then(|x| x.initializer).is_none() {
            continue;
        }
        let build = |m: &mut Module| {
            let mut b = fn_builder::FnBuilder::new(
                ctx,
                unit,
                late_field_getter_symbol(ctx, vid),
                ctx.symbol_name(v.name).to_string(),
                Type::Ref,
            );
            let obj = Operand::Val(b.add_param("this".to_string(), Type::Ref));
            b.this_param = Some(obj.clone());
            b.finalizable_this = v.class.and_then(|c| ctx.classify_finalizable_class(c));
            b.enclosing_class = v.class;
            // Campo fora do mundo fechado (C7): ninguém o lê.
            if ctx.pruned_field(vid) {
                b.pruned_body();
                b.finish(m);
                return;
            }
            b.lower_late_field_getter(obj, vid, ctx.program.unit(unit).ast.member(member).span);
            b.finish(m);
        };
        // O getter que não baixa vira recusa, como os outros membros
        // (`sdk_source::lower_function_or_reject`).
        sdk_source::lower_late_getter_or_reject(ctx, &mut module, vid, unit, build);
    }

    // 3. Variáveis de topo e campos estáticos do usuário: um getter
    // preguiçoso por global (N6).
    for (v_idx, v) in ctx.program.variables.iter().enumerate() {
        let vid = VariableId(v_idx as u32);
        if !ctx.library_in_module(v.library) || !is_global(ctx, vid) {
            continue;
        }
        let unit = match v.node {
            VariableRef::TopLevel { unit, .. }
            | VariableRef::Field { unit, .. }
            | VariableRef::EnumConstant { unit, .. } => unit,
            _ => continue,
        };
        if matches!(v.node, VariableRef::EnumConstant { .. }) {
            // Valor de enum (P3): objeto canônico criado na primeira leitura.
            module.globals.push((vid.0, Type::Ref, global_value_symbol(ctx, vid)));
            let mut builder = fn_builder::FnBuilder::new(
                ctx,
                unit,
                global_symbol(ctx, vid),
                ctx.symbol_name(v.name).to_string(),
                Type::Ref,
            );
            builder.lower_enum_value(vid);
            builder.finish(&mut module);
            continue;
        }
        let ty = members::variable_type(ctx, vid);
        let repr = ctx.to_hir_type(ty);
        let repr = if repr == Type::Void { Type::Ref } else { repr };
        module.globals.push((vid.0, repr, global_value_symbol(ctx, vid)));
        if ctx.arc_memory && matches!(repr, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
            module.dart_scalar_globals.insert(global_value_symbol(ctx, vid));
        }
        // O getter (ou a recusa dele) e o setter que outro módulo chama para
        // gravar (P5c). Fora do mundo fechado (C7), o getter lança.
        if ctx.pruned_global(vid) {
            sdk_source::lower_pruned_global(ctx, &mut module, vid, unit, repr);
            continue;
        }
        sdk_source::lower_global_or_reject(ctx, &mut module, vid, unit, repr);
    }

    // As entradas de tear-off são geradas por quem as usa; duas funções que
    // tiram o mesmo tear-off geram a mesma entrada. Fica a primeira.
    let mut seen_set = std::collections::HashSet::new();
    module.functions.retain(|f| seen_set.insert(f.symbol.clone()));
    let mut seen_set = std::collections::HashSet::new();
    module.globals.retain(|g| seen_set.insert(g.2.clone()));

    // P6: o laço de eventos (quem usa `dart:async`): a classe do quadro das
    // funções `async` e a função que o runtime usa para chamar uma closure
    // (o único ponto em que o runtime chama Dart, `events.rs`).
    if ctx.uses_dart_async
        && let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied())
    {
        module.classes.push(ClassDef {
            id: async_sm::ID_QUADRO_ASYNC,
            name: "_AsyncFrame".to_string(),
            field_count: 0,
            vtable: Vec::new(),
            to_string_symbol: None,
        });
        let symbol = "dartforge_chamar_dart0".to_string();
        let mut b = fn_builder::FnBuilder::new(ctx, u, symbol.clone(), "chamar".to_string(), Type::Ref);
        let clo = Operand::Val(b.add_param("closure".to_string(), Type::Ref));
        let r = b.call_function_value(clo, &[]);
        b.terminate(Terminator::Return(Some(r)));
        b.finish(&mut module);
        // Sob ARC, o runtime a chama como porta Dart (`dart_r1`, o laço de
        // eventos): um callback com retorno Owned, que o escopo nativo solta.
        if ctx.arc_memory {
            module.dart_ref_returns.insert(symbol.clone());
            module.helpers.push((symbol.clone(), symbol.clone()));
        }
        module.call_dart = Some(symbol);
    }

    // P6: as bibliotecas da fonte entram inteiras; fica o que o programa
    // alcança (`source.rs`).
    if !ctx.from_source.is_empty() {
        crate::source::prune(&mut module);
    }
    // RTI: o universo de tipos das receitas que ficaram (`rti.rs`).
    rti::register_universe(ctx, &mut module);

    // E3: o verificador roda sobre o módulo pronto; problema aqui é bug do
    // compilador, e o módulo não é emitido.
    if module.errors.is_empty() {
        let problems = verifier::verify(&module);
        module.errors.extend(problems);
    }

    // Formas de record com campo nomeado (P3, `records.rs`): uma "classe"
    // por forma, com `toString()` gerado, e a igualdade estrutural.
    for k in 0..ctx.record_shapes.len() {
        let id = crate::context::SHAPE_BASE_ID + k as u32;
        let (npos, names) = &ctx.record_shapes[k];
        let symbol = format!("df.$registro.{k}.toString");
        module.classes.push(ClassDef {
            id,
            name: "Record".to_string(),
            field_count: npos + names.len(),
            vtable: Vec::new(),
            to_string_symbol: Some(symbol.clone()),
        });
        module.subtyping_edges.push((id, 0));
        // Com o SDK da fonte, `is Record`/`is Object` perguntam pelas
        // classes do `dart:core` (o `(x: 1) is Record` dava falso).
        for name_str in ["Record", "Object"] {
            if let Some(c) = ctx.sdk_class("core", name_str).and_then(|c| ctx.class_id_of(c)) {
                module.subtyping_edges.push((id, c));
            }
        }
        let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
            continue;
        };
        let mut builder = fn_builder::FnBuilder::new(ctx, u, symbol, "toString".to_string(), Type::Ref);
        builder.lower_shape_to_string(k);
        builder.finish(&mut module);
        let hash_symbol = format!("df.$registro.{k}.hashCode");
        let mut builder = fn_builder::FnBuilder::new(ctx, u, hash_symbol, "hashCode".to_string(), Type::I64);
        builder.lower_shape_hash(k);
        builder.finish(&mut module);
    }
    if !ctx.record_shapes.is_empty()
        && let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied())
    {
        let mut builder = fn_builder::FnBuilder::new(
            ctx,
            u,
            records::EQUAL_SYMBOL.to_string(),
            "==".to_string(),
            Type::I1,
        );
        builder.lower_record_equality();
        builder.finish(&mut module);
    }

    // `toString()` padrão dos enums do programa (`Enum.value`), quando o
    // enum não declara o seu.
    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        let cid = dartforge_elements::model::ClassId(c_idx as u32);
        if !enums::is_enum(ctx, cid) || !ctx.library_in_module(class.library) {
            continue;
        }
        let Some(id) = ctx.class_id_of(cid) else { continue };
        if module.classes.iter().any(|c| c.id == id && c.to_string_symbol.is_some()) {
            continue;
        }
        let Some(decl) = class.decl else { continue };
        let symbol = format!(
            "df.{}.{}.toString",
            crate::context::escape_str(&ctx.library_name(class.library)),
            crate::context::escape_str(ctx.symbol_name(class.name))
        );
        let mut builder = fn_builder::FnBuilder::new(ctx, decl.unit, symbol.clone(), "toString".to_string(), Type::Ref);
        builder.lower_enum_to_string(cid);
        builder.finish(&mut module);
        if let Some(c) = module.classes.iter_mut().find(|c| c.id == id) {
            c.to_string_symbol = Some(symbol);
        }
    }

    // Propaga to_string_symbol para subclasses que não o sobrescreveram
    for c_idx in 0..ctx.program.classes.len() {
        let Some(class_id) = ctx.class_id_of(dartforge_elements::model::ClassId(c_idx as u32)) else {
            continue;
        };
        let mut curr_cid = ctx.program.classes[c_idx].supertype_class;
        while let Some(sup) = curr_cid {
            let Some(sup_class_id) = ctx.class_id_of(sup) else {
                break;
            };
            let sup_ts = module.classes.iter().find(|c| c.id == sup_class_id).and_then(|c| c.to_string_symbol.clone());
            if let Some(ts) = sup_ts {
                if let Some(c_def) = module.classes.iter_mut().find(|c| c.id == class_id) {
                    if c_def.to_string_symbol.is_none() {
                        c_def.to_string_symbol = Some(ts);
                    }
                }
                break;
            }
            curr_cid = ctx.program.classes[sup.0 as usize].supertype_class;
        }
    }

    module
}
