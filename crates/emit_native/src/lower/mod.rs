//! Lowering do AST e tabelas de tipos para a HIR nativa.
//!
//! O contrato entre este lowering, o emissor e o runtime está em
//! `docs/NATIVO-PLANO.md` §6 (R, E, N, G).

pub mod async_sm;
pub mod atribuicao;
pub mod captura;
pub mod cascata;
pub mod chamadas;
pub mod closures;
pub mod comandos;
pub mod constantes;
pub mod despacho;
pub mod enums;
pub mod erros_do_runtime;
pub mod expressoes;
pub mod extensoes;
pub mod ffi;
pub mod externos;
pub mod fn_builder;
pub mod heranca;
pub mod literais;
pub mod locais;
pub mod membros;
pub mod nsm;
pub mod operadores;
pub mod padroes;
pub mod registros;
pub mod rti;
pub mod sdk_fonte;
pub mod sdk_por_nome;
pub mod verificador;

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
pub fn funcao_do_usuario(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    ctx.biblioteca_compilada(f.library)
}

/// A função é construtor generativo (inclusive o sintético): não devolve
/// nada e recebe `this` já alocado (N5).
pub fn construtor_generativo(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    match f.node {
        FunctionRef::Constructor { .. } => !f.factory,
        FunctionRef::None => f.kind == FunctionKind::SyntheticConstructor,
        FunctionRef::Function { .. } => false,
    }
}

/// O dono de um membro no símbolo: a classe, a extensão (`ext:<nome>`) ou
/// vazio para o topo.
fn dono_no_simbolo(ctx: &Context, class: Option<dartforge_elements::model::ClassId>, ext: Option<dartforge_elements::model::ExtensionId>) -> String {
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
/// §7.4): `df.<biblioteca>.<dono>.<membro>`, derivado do **caminho** da
/// declaração — nunca de índices internos do front-end —, com cada parte
/// escapada (`context::escapar`). O membro é o nome; o setter termina em
/// `=`; o construtor é `new` ou `new:<nome>`. `main` da biblioteca de
/// entrada é `dart_main`. Inserir ou reordenar declarações não muda o
/// símbolo de nenhuma outra: é a chave do cache de objeto por biblioteca e da
/// recarga (hot reload R1).
pub fn simbolo_de(ctx: &Context, fid: usize) -> String {
    let f = &ctx.program.functions[fid];
    let nome = ctx.symbol_name(f.name);
    if f.class.is_none() && f.extension.is_none() && nome == "main" && Some(f.library) == ctx.entry_lib {
        return "dart_main".to_string();
    }
    let membro = match f.kind {
        FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
            if nome.is_empty() { "new".to_string() } else { format!("new:{nome}") }
        }
        FunctionKind::Setter if !nome.ends_with('=') => format!("{nome}="),
        _ => nome.to_string(),
    };
    format!(
        "df.{}.{}.{}",
        crate::context::escapar(&ctx.nome_da_biblioteca(f.library)),
        crate::context::escapar(&dono_no_simbolo(ctx, f.class, f.extension)),
        crate::context::escapar(&membro)
    )
}

/// Variável de topo ou campo estático: mora num global do módulo (N6).
pub fn e_global(ctx: &Context, vid: VariableId) -> bool {
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
fn caminho_da_variavel(ctx: &Context, vid: VariableId) -> String {
    let v = &ctx.program.variables[vid.0 as usize];
    format!(
        "{}.{}.{}",
        crate::context::escapar(&ctx.nome_da_biblioteca(v.library)),
        crate::context::escapar(&dono_no_simbolo(ctx, v.class, v.extension)),
        crate::context::escapar(ctx.symbol_name(v.name))
    )
}

/// Getter preguiçoso de um global: `df.<caminho>`.
pub fn simbolo_global(ctx: &Context, vid: VariableId) -> String {
    format!("df.{}", caminho_da_variavel(ctx, vid))
}

/// O valor de um global: `dfg.<caminho>` (a bandeira é `<valor>$ok`).
pub fn simbolo_valor_global(ctx: &Context, vid: VariableId) -> String {
    format!("dfg.{}", caminho_da_variavel(ctx, vid))
}

/// Getter dedicado de um campo de instância `late` com inicializador.
pub fn simbolo_getter_campo_late(ctx: &Context, vid: VariableId) -> String {
    format!("df.late.{}", caminho_da_variavel(ctx, vid))
}

pub fn lower_program(ctx: &Context) -> Module {
    let mut module = Module::new();

    // 1. Registra classes conhecidas
    // Classe Object padrão como id 0
    module.classes.push(ClassDef {
        id: 0,
        name: "Object".to_string(),
        field_count: 0,
        vtable: Vec::new(),
        to_string_symbol: None,
    });

    module.modo_sdk = ctx.sdk_da_fonte;
    module.biblioteca_sdk = ctx.biblioteca_sdk;
    if ctx.sdk_da_fonte {
        // As classes de erro são as do SDK da fonte (P5c).
        return lower_classes_e_funcoes(ctx, module);
    }
    // Classes e interfaces de erro da biblioteca padrão
    module.classes.push(ClassDef { id: 1000, name: "Exception".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1001, name: "FormatException".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1002, name: "StateError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1003, name: "ArgumentError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1004, name: "RangeError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1005, name: "UnsupportedError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1006, name: "StackTrace".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1007, name: "Error".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1008, name: "UnimplementedError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1009, name: "AssertionError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1010, name: "ConcurrentModificationError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1011, name: "TypeError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });
    module.classes.push(ClassDef { id: 1012, name: "NoSuchMethodError".to_string(), field_count: 1, vtable: Vec::new(), to_string_symbol: None });

    module.subtyping_edges.push((1000, 0)); // Exception <: Object
    module.subtyping_edges.push((1001, 1000)); // FormatException <: Exception
    module.subtyping_edges.push((1001, 0)); // FormatException <: Object
    module.subtyping_edges.push((1007, 0)); // Error <: Object
    module.subtyping_edges.push((1002, 1007)); // StateError <: Error
    module.subtyping_edges.push((1002, 0));
    module.subtyping_edges.push((1003, 1007)); // ArgumentError <: Error
    module.subtyping_edges.push((1003, 0));
    module.subtyping_edges.push((1004, 1003)); // RangeError <: ArgumentError
    module.subtyping_edges.push((1004, 1007));
    module.subtyping_edges.push((1004, 0));
    module.subtyping_edges.push((1005, 1007)); // UnsupportedError <: Error
    module.subtyping_edges.push((1005, 0));
    module.subtyping_edges.push((1008, 1005)); // UnimplementedError <: UnsupportedError
    module.subtyping_edges.push((1008, 1007)); // UnimplementedError <: Error
    module.subtyping_edges.push((1008, 0));
    module.subtyping_edges.push((1009, 1007)); // AssertionError <: Error
    module.subtyping_edges.push((1009, 0));
    module.subtyping_edges.push((1010, 1007)); // ConcurrentModificationError <: Error
    module.subtyping_edges.push((1010, 0));
    module.subtyping_edges.push((1011, 1007)); // TypeError <: Error
    module.subtyping_edges.push((1011, 0));
    module.subtyping_edges.push((1012, 1007)); // NoSuchMethodError <: Error
    module.subtyping_edges.push((1012, 0));
    module.subtyping_edges.push((1006, 0)); // StackTrace <: Object
    lower_classes_e_funcoes(ctx, module)
}

fn lower_classes_e_funcoes(ctx: &Context, mut module: Module) -> Module {
    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        // Classes do SDK não viram objetos do nosso heap (o runtime tem as
        // suas próprias representações); só as do usuário são registradas.
        if !ctx.biblioteca_no_modulo(class.library) {
            continue;
        }
        let name = ctx.symbol_name(class.name).to_string();
        // Id estável (P2): a ordem do caminho da classe, não a de carga.
        let Some(class_id) = ctx.id_de_classe(dartforge_elements::model::ClassId(c_idx as u32)) else {
            continue;
        };
        module.classes.push(ClassDef {
            id: class_id,
            name,
            field_count: membros::layout(ctx, dartforge_elements::model::ClassId(c_idx as u32)).len()
                + enums::base_do_layout(ctx, dartforge_elements::model::ClassId(c_idx as u32)),
            vtable: Vec::new(),
            to_string_symbol: None,
        });

        // Registra arestas de subtipagem
        module.subtyping_edges.push((class_id, 0)); // Todo tipo nominal herda de Object
        if let Some(sup) = class.supertype_class.and_then(|s| ctx.id_de_classe(s)) {
            module.subtyping_edges.push((class_id, sup));
        }
        // Enum do programa é subtipo do `Enum` do SDK (especificação §13):
        // sem a aresta, `valor is Enum` respondia falso. A superclasse do
        // outline não carrega o `Enum`, então a aresta é registrada aqui.
        // Sem id do SDK (modo sem fonte), não há o que registrar.
        if ctx.sdk_da_fonte
            && !ctx.program.library(class.library).is_sdk
            && enums::e_enum(ctx, dartforge_elements::model::ClassId(c_idx as u32))
            && let Some(enum_sdk) = ctx.classe_do_sdk("core", "Enum")
            && let Some(enum_id) = ctx.id_de_classe(enum_sdk)
        {
            module.subtyping_edges.push((class_id, enum_id));
        }
        let mut nomes_de_supertipo: Vec<&str> = Vec::new();
        if let Some(sup) = class.supertype_class {
            nomes_de_supertipo.push(ctx.symbol_name(ctx.program.classes[sup.0 as usize].name));
        }
        for mix in &class.mixin_classes {
            if let Some(m) = ctx.id_de_classe(*mix) {
                module.subtyping_edges.push((class_id, m));
            }
        }
        for iface in &class.interface_classes {
            if let Some(i) = ctx.id_de_classe(*iface) {
                module.subtyping_edges.push((class_id, i));
            }
            nomes_de_supertipo.push(ctx.symbol_name(ctx.program.classes[iface.0 as usize].name));
        }
        for sup_name in nomes_de_supertipo {
            if ctx.sdk_da_fonte {
                // As classes de erro são as do SDK da fonte: arestas reais.
                break;
            }
            let builtin = match sup_name {
                "Exception" => Some(1000),
                "FormatException" => Some(1001),
                "StateError" => Some(1002),
                "ArgumentError" => Some(1003),
                "RangeError" => Some(1004),
                "UnsupportedError" => Some(1005),
                "StackTrace" => Some(1006),
                "Error" => Some(1007),
                "UnimplementedError" => Some(1008),
                "AssertionError" => Some(1009),
                "ConcurrentModificationError" => Some(1010),
                "TypeError" => Some(1011),
                "NoSuchMethodError" => Some(1012),
                _ => None,
            };
            if let Some(b) = builtin {
                module.subtyping_edges.push((class_id, b));
            }
        }
    }

    // 2. Funções do usuário
    for f_idx in 0..ctx.program.functions.len() {
        if !ctx.biblioteca_no_modulo(ctx.program.functions[f_idx].library) {
            continue;
        }
        if ctx.sdk_da_fonte && ctx.program.library(ctx.program.functions[f_idx].library).is_sdk {
            // Módulo do SDK da fonte (P5c): o membro que não baixa é
            // recusado sozinho, com o motivo (`sdk_fonte.rs`).
            sdk_fonte::lower_funcao_ou_recusa(ctx, &mut module, f_idx);
        } else {
            lower_funcao(ctx, &mut module, f_idx);
        }
    }
    if ctx.sdk_da_fonte {
        sdk_fonte::lower_adaptadores_e_tabelas(ctx, &mut module);
    }
    let mut module = lower_globais_e_resto(ctx, module);
    if ctx.sdk_da_fonte {
        sdk_fonte::tabelas_das_formas_de_record(ctx, &mut module);
        if !ctx.biblioteca_sdk {
            ffi::lower_ffi(ctx, &mut module);
        }
    }
    module
}

/// P6: os símbolos das funções da fonte que têm corpo — um `external` cujo
/// patch o carregador não ligou (`patched_by` vazio, NATIVO-PEDIDOS) tem o
/// mesmo símbolo do membro do patch, e quem vale é o patch. Calculado uma vez
/// por contexto.
fn com_corpo_da_fonte<'c>(ctx: &'c Context) -> &'c std::collections::HashSet<String> {
    ctx.com_corpo_da_fonte.get_or_init(|| {
        ctx.program
            .functions
            .iter()
            .enumerate()
            .filter(|(i, f)| ctx.da_fonte.contains(&f.library) && membros::tem_corpo(ctx, *i))
            .map(|(i, _)| simbolo_de(ctx, i))
            .collect()
    })
}

/// Baixa uma função (de topo, método, construtor) para o módulo.
pub fn lower_funcao(ctx: &Context, module: &mut Module, f_idx: usize) {
    let func_elem = &ctx.program.functions[f_idx];
    if func_elem.external && ctx.sdk_da_fonte {
        // `external` (inclusive construtor): o corpo é o do patch ou o
        // native (`sdk_fonte::chamar_externo`); nada a baixar aqui — um
        // corpo vazio com o mesmo símbolo tomaria o lugar do patch.
        return;
    }
    {
        let name = ctx.symbol_name(func_elem.name);
        let symbol = simbolo_de(ctx, f_idx);

        match func_elem.node {
            FunctionRef::Function { unit, function } => {
                let ast = &ctx.program.unit(unit).ast;
                let ast_func = ast.function(function);
                if matches!(ast_func.body, FunctionBody::Empty | FunctionBody::Native(_)) {
                    // P6: `external` de biblioteca da fonte com native — o
                    // corpo é a chamada ao runtime (`externos.rs`).
                    if func_elem.external
                        && func_elem.patched_by.is_none()
                        && ctx.da_fonte.contains(&func_elem.library)
                        && !com_corpo_da_fonte(ctx).contains(&symbol)
                    {
                        let ret_ty = ctx
                            .outline
                            .functions
                            .get(f_idx)
                            .map_or(Type::Void, |d| ctx.to_hir_type(d.return_type));
                        let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret_ty);
                        let e_instancia = func_elem.class.is_some() && !func_elem.static_;
                        builder.declarar_parametros(f_idx, e_instancia);
                        if !builder.lower_externo(f_idx, ast_func.span) {
                            builder.nao_suportado(&format!("external `{name}` sem native"), ast_func.span);
                            builder.terminate(Terminator::Return(None));
                        }
                        builder.finalizar(module);
                    }
                    // Abstrato ou externo: não há corpo a compilar.
                    return;
                }

                let is_main = symbol == "dart_main";
                if is_main {
                    module.entry_symbol = Some(symbol.clone());
                }

                if let Some(c_id) = func_elem.class
                    && name == "toString"
                    && !func_elem.static_
                    && let Some(id) = ctx.id_de_classe(c_id)
                    && let Some(c) = module.classes.iter_mut().find(|c| c.id == id)
                {
                    c.to_string_symbol = Some(symbol.clone());
                }

                let ret_ty = if is_main {
                    Type::Void
                } else if let Some(fdata) = ctx.outline.functions.get(f_idx) {
                    ctx.to_hir_type(fdata.return_type)
                } else {
                    Type::Void
                };

                let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), ret_ty);
                builder.preparar_capturas(
                    ast,
                    captura::Raiz {
                        parametros: ast_func.parameters.as_deref().unwrap_or(&[]),
                        corpo: Some(&ast_func.body),
                        inicializadores: &[],
                    },
                );
                let is_instance_member = func_elem.class.is_some() && !func_elem.static_;
                if let Some(e) = func_elem.extension.filter(|_| !func_elem.static_) {
                    // Membro de instância de extensão (P4): o receptor é o
                    // primeiro parâmetro, na representação do tipo `on`.
                    let r = builder.repr_do_receptor_de_extensao(e);
                    let this = builder.add_param("this".to_string(), r);
                    builder.this_param = Some(Operand::Val(this));
                    builder.declarar_parametros(f_idx, false);
                    let on = ctx.outline.extensions[e.0 as usize].on;
                    builder.extensao_do_this = Some((e, on));
                    if let dartforge_types::table::Type::Interface { class, .. } = ctx.table.get(on)
                        && ctx.biblioteca_compilada(ctx.program.classes[class.0 as usize].library)
                    {
                        builder.enclosing_class = Some(*class);
                    }
                } else {
                    builder.declarar_parametros(f_idx, is_instance_member);
                }
                if is_instance_member {
                    builder.enclosing_class = func_elem.class;
                }
                // RTI: a função genérica recebe a tupla dos argumentos de
                // tipo no último parâmetro (`M<i>` das receitas).
                if builder.funcao_generica(f_idx) {
                    let t = builder.add_param("$tipos".to_string(), Type::I64);
                    builder.tupla_de_tipos = Some(Operand::Val(t));
                    builder.params_de_tipo_da_funcao =
                        builder.params_de_tipo_de(f_idx).iter().map(|&p| ctx.table.param(p).name).collect();
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
                    modificador => {
                        builder.lower_corpo_async(
                            ast,
                            ast_func.parameters.as_deref().unwrap_or(&[]),
                            &ast_func.body,
                            ast_func.span,
                            ctx.outline.functions.get(f_idx).map(|d| d.return_type),
                            async_sm::tipo_do_corpo(modificador),
                        );
                    }
                }
                builder.finalizar(module);
            }
            FunctionRef::Constructor { unit, member } => {
                let ast = &ctx.program.unit(unit).ast;
                let MemberKind::Constructor(ctor) = &ast.member(member).kind else {
                    return;
                };
                let Some(cid) = func_elem.class else { return };
                if func_elem.factory {
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), Type::Ref);
                    builder.preparar_capturas(
                        ast,
                        captura::Raiz {
                            parametros: &ctor.parameters,
                            corpo: Some(&ctor.body),
                            inicializadores: &ctor.initializers,
                        },
                    );
                    builder.declarar_parametros(f_idx, false);
                    // RTI: a fábrica de classe genérica recebe os argumentos
                    // de tipo da classe na tupla (não há `this`).
                    builder.enclosing_class = Some(cid);
                    if builder.classe_generica(cid) {
                        let t = builder.add_param("$tipos".to_string(), Type::I64);
                        builder.tupla_de_tipos = Some(Operand::Val(t));
                        builder.classe_por_tupla = true;
                    }
                    if let Some(r) = &ctor.redirect {
                        // `factory C(…) = D.nome;` (P4): os parâmetros passam
                        // como estão (posicionais em ordem, nomeados pelo
                        // nome) para o construtor alvo.
                        let span = ast.member(member).span;
                        let alvo = builder.construtor_do_tipo(ast.ty(r.ty), r.constructor.map(|n| n.sym));
                        match alvo {
                            Some(t) => {
                                let mut avaliados = Vec::new();
                                for (i, p) in ctx.outline.functions[f_idx].parameters.iter().enumerate() {
                                    let Some(sym) = p.name else { continue };
                                    let v = builder.ler_local_por_nome(sym).unwrap_or(Operand::Constant(Constant::Null));
                                    let nome = (ctor.parameters.get(i).map(|q| q.kind)
                                        == Some(dartforge_frontend::ast::ParameterKind::Named))
                                    .then_some(sym);
                                    avaliados.push((nome, v));
                                }
                                // `factory C<T>(…) = C<T>._` deve entregar os
                                // parâmetros reificados ao construtor alvo.
                                // Sem isso, `MapEntry<K,V>` nasce como
                                // `MapEntry<dynamic,dynamic>` mesmo quando a
                                // fábrica recebeu `L<K,V>` pela ABI.
                                let classe_alvo = ctx.program.functions[t.0 as usize].class;
                                if classe_alvo == Some(cid) {
                                    builder.tipo_da_criacao = Some(ctx.outline.functions[f_idx].return_type);
                                }
                                let (rti_alvo, tupla_alvo) = if classe_alvo
                                    .is_some_and(|alvo| builder.classe_generica(alvo))
                                {
                                    let anotacao = ast.ty(r.ty);
                                    let rti = builder.receita_da_anotacao(anotacao)
                                        .map(|receita| builder.rti_da_receita(&receita));
                                    let tupla = builder.tupla_da_anotacao(anotacao);
                                    (rti, tupla)
                                } else {
                                    (None, None)
                                };
                                let r = builder.instanciar_avaliados_com_rti(
                                    t, &avaliados, span, rti_alvo, tupla_alvo,
                                );
                                builder.terminate(Terminator::Return(Some(r)));
                            }
                            None => {
                                builder.nao_suportado("factory redirecionadora", span);
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
                    builder.finalizar(module);
                } else {
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), Type::Void);
                    builder.preparar_capturas(
                        ast,
                        captura::Raiz {
                            parametros: &ctor.parameters,
                            corpo: Some(&ctor.body),
                            inicializadores: &ctor.initializers,
                        },
                    );
                    builder.declarar_parametros(f_idx, true);
                    builder.enclosing_class = Some(cid);
                    builder.lower_construtor(ast, cid, ctor, ast.member(member).span);
                    builder.finalizar(module);
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
                builder.declarar_parametros(f_idx, true);
                builder.enclosing_class = Some(cid);
                let ast = &ctx.program.unit(decl.unit).ast;
                builder.lower_construtor_sintetico(ast, cid);
                builder.finalizar(module);
            }
        }
    }
}

fn lower_globais_e_resto(ctx: &Context, mut module: Module) -> Module {
    // O inicializador não pode ser expandido em cada ponto de leitura: uma
    // auto-referência deve chamar o getter em runtime e observar reentrância.
    for (v_idx, v) in ctx.program.variables.iter().enumerate() {
        let vid = VariableId(v_idx as u32);
        let VariableRef::Field { unit, member, index } = v.node else { continue };
        if !ctx.biblioteca_no_modulo(v.library) || !v.late || v.static_ || v.class.is_none() {
            continue;
        }
        let MemberKind::Field(list) = &ctx.program.unit(unit).ast.member(member).kind else { continue };
        if list.variables.get(index).and_then(|x| x.initializer).is_none() {
            continue;
        }
        let construir = |m: &mut Module| {
            let mut b = fn_builder::FnBuilder::new(
                ctx,
                unit,
                simbolo_getter_campo_late(ctx, vid),
                ctx.symbol_name(v.name).to_string(),
                Type::Ref,
            );
            let obj = Operand::Val(b.add_param("this".to_string(), Type::Ref));
            b.this_param = Some(obj.clone());
            b.enclosing_class = v.class;
            b.lower_getter_campo_late(obj, vid, ctx.program.unit(unit).ast.member(member).span);
            b.finalizar(m);
        };
        if ctx.sdk_da_fonte {
            // SDK da fonte: o getter que não baixa vira recusa, como os
            // outros membros (`sdk_fonte::lower_funcao_ou_recusa`).
            sdk_fonte::lower_getter_late_ou_recusa(ctx, &mut module, vid, unit, construir);
        } else {
            construir(&mut module);
        }
    }

    // 3. Variáveis de topo e campos estáticos do usuário: um getter
    // preguiçoso por global (N6).
    for (v_idx, v) in ctx.program.variables.iter().enumerate() {
        let vid = VariableId(v_idx as u32);
        if !ctx.biblioteca_no_modulo(v.library) || !e_global(ctx, vid) {
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
            module.globais.push((vid.0, Type::Ref, simbolo_valor_global(ctx, vid)));
            let mut builder = fn_builder::FnBuilder::new(
                ctx,
                unit,
                simbolo_global(ctx, vid),
                ctx.symbol_name(v.name).to_string(),
                Type::Ref,
            );
            builder.lower_valor_de_enum(vid);
            builder.finalizar(&mut module);
            continue;
        }
        let ty = membros::tipo_da_variavel(ctx, vid);
        let repr = ctx.to_hir_type(ty);
        let repr = if repr == Type::Void { Type::Ref } else { repr };
        module.globais.push((vid.0, repr, simbolo_valor_global(ctx, vid)));
        if ctx.sdk_da_fonte {
            // SDK da fonte (P5c): o getter (ou a recusa dele) e o setter que
            // outro módulo chama para gravar.
            sdk_fonte::lower_global_ou_recusa(ctx, &mut module, vid, unit, repr);
            continue;
        }
        let mut builder = fn_builder::FnBuilder::new(
            ctx,
            unit,
            simbolo_global(ctx, vid),
            ctx.symbol_name(v.name).to_string(),
            repr,
        );
        builder.lower_getter_global(vid, repr);
        builder.finalizar(&mut module);
    }

    // As entradas de tear-off são geradas por quem as usa; duas funções que
    // tiram o mesmo tear-off geram a mesma entrada. Fica a primeira.
    let mut vistos = std::collections::HashSet::new();
    module.functions.retain(|f| vistos.insert(f.symbol.clone()));
    let mut vistos = std::collections::HashSet::new();
    module.globais.retain(|g| vistos.insert(g.2.clone()));

    // P6: o laço de eventos (quem usa `dart:async`): a classe do quadro das
    // funções `async` e a função que o runtime usa para chamar uma closure
    // (o único ponto em que o runtime chama Dart, `eventos.rs`).
    if ctx.usa_dart_async
        && let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied())
    {
        module.classes.push(ClassDef {
            id: async_sm::ID_QUADRO_ASYNC,
            name: "_AsyncFrame".to_string(),
            field_count: 0,
            vtable: Vec::new(),
            to_string_symbol: None,
        });
        let simbolo = "dartforge_chamar_dart0".to_string();
        let mut b = fn_builder::FnBuilder::new(ctx, u, simbolo.clone(), "chamar".to_string(), Type::Ref);
        let clo = Operand::Val(b.add_param("closure".to_string(), Type::Ref));
        let r = b.chamar_valor_funcao(clo, &[]);
        b.terminate(Terminator::Return(Some(r)));
        b.finalizar(&mut module);
        module.chamar_dart = Some(simbolo);
    }

    // P6: as bibliotecas da fonte entram inteiras; fica o que o programa
    // alcança (`fonte.rs`).
    if !ctx.da_fonte.is_empty() {
        crate::fonte::podar(&mut module);
    }
    // RTI: o universo de tipos das receitas que ficaram (`rti.rs`).
    rti::registrar_universo(ctx, &mut module);

    // E3: o verificador roda sobre o módulo pronto; problema aqui é bug do
    // compilador, e o módulo não é emitido.
    if module.erros.is_empty() {
        let problemas = verificador::verificar(&module);
        module.erros.extend(problemas);
    }

    // Formas de record com campo nomeado (P3, `registros.rs`): uma "classe"
    // por forma, com `toString()` gerado, e a igualdade estrutural.
    for k in 0..ctx.formas_de_record.len() {
        let id = crate::context::ID_BASE_DE_FORMA + k as u32;
        let (npos, nomes) = &ctx.formas_de_record[k];
        let simbolo = format!("df.$registro.{k}.toString");
        module.classes.push(ClassDef {
            id,
            name: "Record".to_string(),
            field_count: npos + nomes.len(),
            vtable: Vec::new(),
            to_string_symbol: Some(simbolo.clone()),
        });
        module.subtyping_edges.push((id, 0));
        let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
            continue;
        };
        let mut builder = fn_builder::FnBuilder::new(ctx, u, simbolo, "toString".to_string(), Type::Ref);
        builder.lower_to_string_de_forma(k);
        builder.finalizar(&mut module);
        if ctx.sdk_da_fonte {
            let simbolo_hash = format!("df.$registro.{k}.hashCode");
            let mut builder = fn_builder::FnBuilder::new(ctx, u, simbolo_hash, "hashCode".to_string(), Type::I64);
            builder.lower_hash_de_forma(k);
            builder.finalizar(&mut module);
        }
    }
    if !ctx.formas_de_record.is_empty()
        && let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied())
    {
        let mut builder = fn_builder::FnBuilder::new(
            ctx,
            u,
            registros::SIMBOLO_IGUAL.to_string(),
            "==".to_string(),
            Type::I1,
        );
        builder.lower_igualdade_de_registros();
        builder.finalizar(&mut module);
    }

    // `toString()` padrão dos enums do programa (`Enum.valor`), quando o
    // enum não declara o seu.
    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        let cid = dartforge_elements::model::ClassId(c_idx as u32);
        if !enums::e_enum(ctx, cid) || !ctx.biblioteca_no_modulo(class.library) {
            continue;
        }
        let Some(id) = ctx.id_de_classe(cid) else { continue };
        if module.classes.iter().any(|c| c.id == id && c.to_string_symbol.is_some()) {
            continue;
        }
        let Some(decl) = class.decl else { continue };
        let simbolo = format!(
            "df.{}.{}.toString",
            crate::context::escapar(&ctx.nome_da_biblioteca(class.library)),
            crate::context::escapar(ctx.symbol_name(class.name))
        );
        let mut builder = fn_builder::FnBuilder::new(ctx, decl.unit, simbolo.clone(), "toString".to_string(), Type::Ref);
        builder.lower_to_string_de_enum(cid);
        builder.finalizar(&mut module);
        if let Some(c) = module.classes.iter_mut().find(|c| c.id == id) {
            c.to_string_symbol = Some(simbolo);
        }
    }

    // Propaga to_string_symbol para subclasses que não o sobrescreveram
    for c_idx in 0..ctx.program.classes.len() {
        let Some(class_id) = ctx.id_de_classe(dartforge_elements::model::ClassId(c_idx as u32)) else {
            continue;
        };
        let mut curr_cid = ctx.program.classes[c_idx].supertype_class;
        while let Some(sup) = curr_cid {
            let Some(sup_class_id) = ctx.id_de_classe(sup) else {
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
