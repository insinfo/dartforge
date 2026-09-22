//! Lowering do AST e tabelas de tipos para a HIR nativa.

pub mod fn_builder;

use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{ClassId, FunctionElementId, FunctionKind, FunctionRef, Program, UnitId};
use dartforge_frontend::ast::FunctionBody;

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
    }

    // 2. Itera sobre funções do programa
    for (f_idx, func_elem) in ctx.program.functions.iter().enumerate() {
        let func_id = FunctionElementId(f_idx as u32);
        let name = ctx.symbol_name(func_elem.name);

        match func_elem.node {
            FunctionRef::Function { unit, function } => {
                let ast = &ctx.program.unit(unit).ast;
                let ast_func = ast.function(function);

                let is_main = func_elem.class.is_none() && name == "main";
                let symbol = if is_main {
                    "dart_main".to_string()
                } else {
                    format!("df_fn_{f_idx}_{name}")
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
            }
            _ => {}
        }
    }

    module
}

