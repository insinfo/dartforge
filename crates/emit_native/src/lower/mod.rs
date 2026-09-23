//! Lowering do AST e tabelas de tipos para a HIR nativa.
//!
//! O contrato entre este lowering, o emissor e o runtime está em
//! `docs/NATIVO-PLANO.md` §6 (R, E, N, G).

pub mod atribuicao;
pub mod chamadas;
pub mod comandos;
pub mod expressoes;
pub mod fn_builder;
pub mod locais;
pub mod membros;
pub mod operadores;
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
    !ctx.program.library(f.library).is_sdk
}

/// Símbolo LLVM de uma função do usuário.
///
/// Construtor generativo (inclusive o sintético) é `df_ctor_<id>(this, …)`
/// e não devolve nada (N5); `main` da biblioteca de entrada é `dart_main`;
/// o resto é `df_fn_<id>_<nome>`.
pub fn simbolo_de(ctx: &Context, fid: usize) -> String {
    let f = &ctx.program.functions[fid];
    let nome = ctx.symbol_name(f.name);
    if f.class.is_none() && nome == "main" && Some(f.library) == ctx.entry_lib {
        return "dart_main".to_string();
    }
    let construtor_generativo = match f.node {
        FunctionRef::Constructor { .. } => !f.factory,
        FunctionRef::None => f.kind == FunctionKind::SyntheticConstructor,
        FunctionRef::Function { .. } => false,
    };
    if construtor_generativo {
        format!("df_ctor_{fid}")
    } else {
        format!("df_fn_{fid}_{}", sanitize_symbol(nome))
    }
}

/// Variável de topo ou campo estático: mora num global do módulo (N6).
pub fn e_global(ctx: &Context, vid: VariableId) -> bool {
    let v = &ctx.program.variables[vid.0 as usize];
    (v.class.is_none() || v.static_)
        && v.extension.is_none()
        && matches!(v.node, VariableRef::TopLevel { .. } | VariableRef::Field { .. })
}

/// Getter preguiçoso de um global (`df_global_<id>`).
pub fn simbolo_global(vid: VariableId) -> String {
    format!("df_global_{}", vid.0)
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

    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        // Classes do SDK não viram objetos do nosso heap (o runtime tem as
        // suas próprias representações); só as do usuário são registradas.
        if ctx.program.library(class.library).is_sdk {
            continue;
        }
        let name = ctx.symbol_name(class.name).to_string();
        let class_id = (c_idx + 1) as u32; // Evita colisão com Object = 0
        module.classes.push(ClassDef {
            id: class_id,
            name,
            field_count: membros::layout(ctx, dartforge_elements::model::ClassId(c_idx as u32)).len(),
            vtable: Vec::new(),
            to_string_symbol: None,
        });

        // Registra arestas de subtipagem
        module.subtyping_edges.push((class_id, 0)); // Todo tipo nominal herda de Object
        if let Some(sup) = class.supertype_class {
            module.subtyping_edges.push((class_id, (sup.0 + 1) as u32));
        }
        let mut nomes_de_supertipo: Vec<&str> = Vec::new();
        if let Some(sup) = class.supertype_class {
            nomes_de_supertipo.push(ctx.symbol_name(ctx.program.classes[sup.0 as usize].name));
        }
        for mix in &class.mixin_classes {
            module.subtyping_edges.push((class_id, (mix.0 + 1) as u32));
        }
        for iface in &class.interface_classes {
            module.subtyping_edges.push((class_id, (iface.0 + 1) as u32));
            nomes_de_supertipo.push(ctx.symbol_name(ctx.program.classes[iface.0 as usize].name));
        }
        for sup_name in nomes_de_supertipo {
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
    for (f_idx, func_elem) in ctx.program.functions.iter().enumerate() {
        if !funcao_do_usuario(ctx, f_idx) {
            continue;
        }
        // Membros de extensão não são compilados (o receptor implícito não
        // tem lowering ainda); o USO de um deles é que é diagnóstico (N1), e
        // uma extensão declarada e nunca chamada não impede o programa.
        if func_elem.extension.is_some() {
            continue;
        }
        let name = ctx.symbol_name(func_elem.name);
        let symbol = simbolo_de(ctx, f_idx);

        match func_elem.node {
            FunctionRef::Function { unit, function } => {
                let ast = &ctx.program.unit(unit).ast;
                let ast_func = ast.function(function);
                if matches!(ast_func.body, FunctionBody::Empty | FunctionBody::Native(_)) {
                    // Abstrato ou externo: não há corpo a compilar.
                    continue;
                }

                let is_main = symbol == "dart_main";
                if is_main {
                    module.entry_symbol = Some(symbol.clone());
                }

                if let Some(c_id) = func_elem.class
                    && name == "toString"
                    && !func_elem.static_
                    && let Some(c) = module.classes.iter_mut().find(|c| c.id == c_id.0 + 1)
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
                let is_instance_member = func_elem.class.is_some() && !func_elem.static_;
                builder.declarar_parametros(f_idx, is_instance_member);
                if is_instance_member {
                    builder.enclosing_class = func_elem.class;
                }

                match &ast_func.body {
                    FunctionBody::Block(stmt_id) => {
                        builder.lower_stmt(ast, *stmt_id);
                    }
                    FunctionBody::Expression(expr_id) => {
                        let ret_op = builder.lower_expr(ast, *expr_id);
                        builder.terminate(Terminator::Return(Some(ret_op)));
                    }
                    _ => {}
                }
                builder.finalizar(&mut module);
            }
            FunctionRef::Constructor { unit, member } => {
                let ast = &ctx.program.unit(unit).ast;
                let MemberKind::Constructor(ctor) = &ast.member(member).kind else {
                    continue;
                };
                let Some(cid) = func_elem.class else { continue };
                if func_elem.factory {
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), Type::Ref);
                    builder.declarar_parametros(f_idx, false);
                    if ctor.redirect.is_some() {
                        let span = ast.member(member).span;
                        builder.nao_suportado("factory redirecionadora", span);
                        builder.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
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
                    builder.finalizar(&mut module);
                } else {
                    let mut builder = fn_builder::FnBuilder::new(ctx, unit, symbol, name.to_string(), Type::Void);
                    builder.declarar_parametros(f_idx, true);
                    builder.enclosing_class = Some(cid);
                    builder.lower_construtor(ast, cid, ctor, ast.member(member).span);
                    builder.finalizar(&mut module);
                }
            }
            FunctionRef::None => {
                // Construtor padrão sintético (`class A { int x = 1; }`):
                // inicializadores de campo e `super()` implícito.
                if func_elem.kind != FunctionKind::SyntheticConstructor {
                    continue;
                }
                let Some(cid) = func_elem.class else { continue };
                let Some(decl) = ctx.program.classes[cid.0 as usize].decl else { continue };
                let mut builder = fn_builder::FnBuilder::new(ctx, decl.unit, symbol, name.to_string(), Type::Void);
                builder.declarar_parametros(f_idx, true);
                builder.enclosing_class = Some(cid);
                let ast = &ctx.program.unit(decl.unit).ast;
                builder.lower_construtor_sintetico(ast, cid);
                builder.finalizar(&mut module);
            }
        }
    }

    // 3. Variáveis de topo e campos estáticos do usuário: um getter
    // preguiçoso por global (N6).
    for (v_idx, v) in ctx.program.variables.iter().enumerate() {
        let vid = VariableId(v_idx as u32);
        if ctx.program.library(v.library).is_sdk || !e_global(ctx, vid) {
            continue;
        }
        let unit = match v.node {
            VariableRef::TopLevel { unit, .. } | VariableRef::Field { unit, .. } => unit,
            _ => continue,
        };
        let ty = membros::tipo_da_variavel(ctx, vid);
        let repr = ctx.to_hir_type(ty);
        let repr = if repr == Type::Void { Type::Ref } else { repr };
        module.globais.push((vid.0, repr));
        let mut builder = fn_builder::FnBuilder::new(
            ctx,
            unit,
            simbolo_global(vid),
            ctx.symbol_name(v.name).to_string(),
            repr,
        );
        builder.lower_getter_global(vid, repr);
        builder.finalizar(&mut module);
    }

    // E3: o verificador roda sobre o módulo pronto; problema aqui é bug do
    // compilador, e o módulo não é emitido.
    if module.erros.is_empty() {
        let problemas = verificador::verificar(&module);
        module.erros.extend(problemas);
    }

    // Propaga to_string_symbol para subclasses que não o sobrescreveram
    for c_idx in 0..ctx.program.classes.len() {
        let class_id = (c_idx + 1) as u32;
        let mut curr_cid = ctx.program.classes[c_idx].supertype_class;
        while let Some(sup) = curr_cid {
            let sup_class_id = (sup.0 + 1) as u32;
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
