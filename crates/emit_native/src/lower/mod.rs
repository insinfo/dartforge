//! Lowering do AST e tabelas de tipos para a HIR nativa.

pub mod fn_builder;

use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{ClassId, FunctionElementId, FunctionKind, FunctionRef, Program, UnitId};
use dartforge_frontend::ast::FunctionBody;

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
        let name = ctx.symbol_name(class.name).to_string();
        let class_id = (c_idx + 1) as u32; // Evita colisão com Object = 0
        module.classes.push(ClassDef {
            id: class_id,
            name,
            field_count: class.fields.len(),
            vtable: Vec::new(),
            to_string_symbol: None,
        });

        // Registra arestas de subtipagem
        module.subtyping_edges.push((class_id, 0)); // Todo tipo nominal herda de Object
        if let Some(sup) = class.supertype_class {
            module.subtyping_edges.push((class_id, (sup.0 + 1) as u32));
        }
        if let Some((unit, tid)) = class.supertype {
            let ast_ty = ctx.program.unit(unit).ast.ty(tid);
            if let dartforge_frontend::ast::TypeKind::Named { name, .. } = &ast_ty.kind {
                if let Some(first) = name.first() {
                    let sup_name = ctx.symbol_name(first.sym);
                    match sup_name {
                        "Exception" => module.subtyping_edges.push((class_id, 1000)),
                        "FormatException" => module.subtyping_edges.push((class_id, 1001)),
                        "StateError" => module.subtyping_edges.push((class_id, 1002)),
                        "ArgumentError" => module.subtyping_edges.push((class_id, 1003)),
                        "RangeError" => module.subtyping_edges.push((class_id, 1004)),
                        "UnsupportedError" => module.subtyping_edges.push((class_id, 1005)),
                        "StackTrace" => module.subtyping_edges.push((class_id, 1006)),
                        "Error" => module.subtyping_edges.push((class_id, 1007)),
                        "UnimplementedError" => module.subtyping_edges.push((class_id, 1008)),
                        "AssertionError" => module.subtyping_edges.push((class_id, 1009)),
                        "ConcurrentModificationError" => module.subtyping_edges.push((class_id, 1010)),
                        "TypeError" => module.subtyping_edges.push((class_id, 1011)),
                        "NoSuchMethodError" => module.subtyping_edges.push((class_id, 1012)),
                        _ => {}
                    }
                }
            }
        }
        for mix in &class.mixin_classes {
            module.subtyping_edges.push((class_id, (mix.0 + 1) as u32));
        }
        for iface in &class.interface_classes {
            module.subtyping_edges.push((class_id, (iface.0 + 1) as u32));
        }
        for (unit, tid) in &class.interfaces {
            let ast_ty = ctx.program.unit(*unit).ast.ty(*tid);
            if let dartforge_frontend::ast::TypeKind::Named { name, .. } = &ast_ty.kind {
                if let Some(first) = name.first() {
                    let iface_name = ctx.symbol_name(first.sym);
                    match iface_name {
                        "Exception" => module.subtyping_edges.push((class_id, 1000)),
                        "FormatException" => module.subtyping_edges.push((class_id, 1001)),
                        "StateError" => module.subtyping_edges.push((class_id, 1002)),
                        "ArgumentError" => module.subtyping_edges.push((class_id, 1003)),
                        "RangeError" => module.subtyping_edges.push((class_id, 1004)),
                        "UnsupportedError" => module.subtyping_edges.push((class_id, 1005)),
                        "StackTrace" => module.subtyping_edges.push((class_id, 1006)),
                        "Error" => module.subtyping_edges.push((class_id, 1007)),
                        "UnimplementedError" => module.subtyping_edges.push((class_id, 1008)),
                        "AssertionError" => module.subtyping_edges.push((class_id, 1009)),
                        "ConcurrentModificationError" => module.subtyping_edges.push((class_id, 1010)),
                        "TypeError" => module.subtyping_edges.push((class_id, 1011)),
                        "NoSuchMethodError" => module.subtyping_edges.push((class_id, 1012)),
                        _ => {}
                    }
                }
            }
        }
    }

    // 2. Itera sobre funções do programa
    for (f_idx, func_elem) in ctx.program.functions.iter().enumerate() {
        let name = ctx.symbol_name(func_elem.name);
        let sanitized_name = sanitize_symbol(name);

        match func_elem.node {
            FunctionRef::Function { unit, function } => {
                let ast = &ctx.program.unit(unit).ast;
                let ast_func = ast.function(function);

                let is_main = func_elem.class.is_none() && name == "main";
                let symbol = if is_main {
                    "dart_main".to_string()
                } else {
                    format!("df_fn_{f_idx}_{sanitized_name}")
                };

                if is_main {
                    module.entry_symbol = Some(symbol.clone());
                }

                if let Some(c_id) = func_elem.class {
                    if name == "toString" && !func_elem.static_ {
                        module.classes[c_id.0 as usize + 1].to_string_symbol = Some(symbol.clone());
                    }
                }

                let ret_ty = if is_main {
                    Type::Void
                } else if let Some(fdata) = ctx.outline.functions.get(f_idx) {
                    ctx.to_hir_type(fdata.return_type)
                } else {
                    Type::Void
                };

                // Cria o construtor da função
                let mut builder = fn_builder::FnBuilder::new(
                    ctx,
                    unit,
                    symbol,
                    name.to_string(),
                    ret_ty,
                );

                let is_instance_member = func_elem.class.is_some() && !func_elem.static_;
                if is_instance_member {
                    let this_vid = builder.add_param("this".to_string(), Type::Ref);
                    builder.this_param = Some(Operand::Val(this_vid));
                    builder.enclosing_class = func_elem.class;
                }

                if let Some(fdata) = ctx.outline.functions.get(f_idx) {
                    for p in fdata.parameters.iter() {
                        let p_name = p.name.map(|s| ctx.symbol_name(s).to_string()).unwrap_or_else(|| "arg".to_string());
                        let p_ty = ctx.to_hir_type(p.ty);
                        let vid = builder.add_param(p_name, p_ty);
                        if let Some(sym) = p.name {
                            builder.named_locals.insert(sym, Operand::Val(vid));
                        }
                    }
                }

                // Baixa o corpo
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

                module.functions.push(builder.func);
                module.functions.extend(builder.extra_functions);
            }
            _ => {}
        }
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

