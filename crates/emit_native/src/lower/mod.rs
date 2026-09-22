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
    });

    for (c_idx, class) in ctx.program.classes.iter().enumerate() {
        let name = ctx.symbol_name(class.name).to_string();
        let class_id = (c_idx + 1) as u32; // Evita colisão com Object = 0
        module.classes.push(ClassDef {
            id: class_id,
            name,
            field_count: class.fields.len(),
            vtable: Vec::new(),
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

                // Cria o construtor da função
                let mut builder = fn_builder::FnBuilder::new(
                    ctx,
                    unit,
                    symbol,
                    name.to_string(),
                    Type::Void, // ou tipo de retorno inferido
                );

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

