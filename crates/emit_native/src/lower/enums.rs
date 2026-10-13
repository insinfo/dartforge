//! Enums (P3): cada valor é um objeto canônico do programa, guardado num
//! global preguiçoso (o mesmo mecanismo das variáveis de topo, N6).
//!
//! O objeto de um enum tem, antes dos campos declarados, os dois campos
//! implícitos da especificação (§13 "Enums"): `index` (posição 0, `int`) e
//! `_name` (posição 1, a `String` do nome). O valor é criado como o `const`
//! de um enum aprimorado: aloca, grava `index` e `_name`, e roda o
//! construtor escolhido (`value(args)`/`value.name(args)`) com os campos
//! declarados. `values` é a lista dos valores na ordem de declaração;
//! `toString()` é `Enum.value` (o do `_Enum` da VM), a menos que o enum
//! declare o seu. A igualdade é identidade (o valor é canônico), e a chave
//! de recarga é `(class, name)` (PESQUISA-HOT-RELOAD §4.7): o símbolo do
//! global é o caminho `dfg.<library>.<Enum>.<value>`.

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, VariableId, VariableRef};
use dartforge_frontend::ast::DeclKind;

/// A classe é um `enum` do programa.
pub fn is_enum(ctx: &Context, cid: ClassId) -> bool {
    let c = &ctx.program.classes[cid.0 as usize];
    c.kind == ClassKind::Enum && ctx.compiled_library(c.library)
}

/// Quantos campos implícitos vêm antes do layout declarado (2 num enum).
pub fn layout_base(ctx: &Context, cid: ClassId) -> usize {
    if is_enum(ctx, cid) { 2 } else { 0 }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `index`/`name` de um valor de enum da classe `cid`.
    pub fn enum_member(&mut self, cid: ClassId, name: &str, value: Operand) -> Option<Operand> {
        if !is_enum(self.ctx, cid) {
            return None;
        }
        let (idx, ty) = match name {
            "index" => (0, Type::I64),
            "name" => (1, Type::Ref),
            _ => return None,
        };
        let value = self.coerce(value, Type::Ref);
        Some(self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![
                    (value, Type::Ref),
                    (Operand::Constant(Constant::Int(idx)), Type::I64),
                ],
                ret_ty: ty,
            },
            ty,
        ))
    }

    /// `E.values`: a lista dos valores, na ordem de declaração.
    pub fn enum_values(&mut self, cid: ClassId, span: Span) -> Operand {
        // `E.values` é uma constante: a mesma lista (`List<E>`, imutável)
        // em toda leitura.
        let class = &self.ctx.program.classes[cid.0 as usize];
        let symbol = format!(
            "dfc.{}.{}.values",
            crate::context::escape_str(&self.ctx.library_name(class.library)),
            crate::context::escape_str(self.ctx.symbol_name(class.name))
        );
        let consts: Vec<VariableId> = class.enum_constants.clone();
        self.generated_constant(&symbol, true, |b| {
            let mut elements = Vec::with_capacity(consts.len());
            for v in consts {
                let x = b.read_global(v, span);
                elements.push((x, 3));
            }
            let l = b.emit(Instruction::AllocList { elements: elements }, Type::Ref);
            if let Some(list) = b.ctx.core.list_class {
                let ty = b.recipe_rti(&super::rti::Recipe {
                    text: format!("C{}<C{}>", b.ctx.id_rti(list), b.ctx.id_rti(cid)),
                    variables: false,
                });
                b.define_rti(l.clone(), ty);
            }
            l
        })
    }

    /// Corpo do getter preguiçoso de um valor de enum: cria o objeto
    /// canônico na primeira leitura.
    pub fn lower_enum_value(&mut self, vid: VariableId) {
        // O getter devolve o valor como as funções Dart: Ref é Owned.
        if self.ctx.arc_memory {
            self.dart_ref_returns.insert(self.func.symbol.clone());
        }
        let var = &self.ctx.program.variables[vid.0 as usize];
        let VariableRef::EnumConstant { unit, decl, index } = var.node else {
            return;
        };
        let Some(cid) = var.class else { return };
        let value = super::global_value_symbol(self.ctx, vid);
        let flag = format!("{value}$ok");
        let ok = self.emit(
            Instruction::LoadGlobal {
                symbol: flag.clone(),
                ty: Type::I8,
            },
            Type::I8,
        );
        let ready = self.emit(
            Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_read = self.new_block();
        let b_init = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ready,
            then_block: b_read,
            else_block: b_init,
        });
        self.set_block(b_read);
        let v = self.emit(
            Instruction::LoadGlobal {
                symbol: value.clone(),
                ty: Type::Ref,
            },
            Type::Ref,
        );
        self.terminate(Terminator::Return(Some(v)));
        self.set_block(b_init);
        self.emit(
            Instruction::StoreGlobal {
                symbol: flag,
                val: Operand::Constant(Constant::Int(1)),
                ty: Type::I8,
                root: None,
            },
            Type::Void,
        );
        let n_fields = super::members::layout(self.ctx, cid).len() + layout_base(self.ctx, cid);
        let id = self.ctx.class_id_of(cid).unwrap_or(0);
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(i64::from(id))), Type::I64),
                    (Operand::Constant(Constant::Int(n_fields as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        // Enum genérico: o valor guarda o tipo dele, como qualquer instância
        // genérica — os argumentos escritos (`a<bool>()`) ou, sem eles, os
        // limites dos parâmetros (`E<Object>` para `T extends Object`).
        if self.generic_class(cid)
            && let DeclKind::Enum(e) = &self.ctx.program.unit(unit).ast.decl(decl).kind
        {
            let written_set = e.constants[index].type_args.to_vec();
            let recipe = if written_set.is_empty() {
                let params = self.ctx.outline.classes[cid.0 as usize].type_params.clone();
                let mut r = super::rti::Recipe { text: String::new(), variables: false };
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    // Sem limite escrito, `dynamic` (instanciar para os limites).
                    let d = self.ctx.table.param(*p);
                    if !d.explicito {
                        r.text.push('D');
                        continue;
                    }
                    let x = self.type_recipe(d.bound);
                    r.text.push_str(&x.text);
                    r.variables |= x.variables;
                }
                r
            } else {
                let saved = std::mem::replace(&mut self.unit_id, unit);
                let r = self.type_argument_recipes(&written_set);
                self.unit_id = saved;
                r
            };
            if !recipe.variables {
                let r = self.recipe_rti(&super::rti::Recipe {
                    text: format!("C{}<{}>", self.ctx.id_rti(cid), recipe.text),
                    variables: false,
                });
                self.define_rti(obj.clone(), r);
            }
        }
        // O objeto já é o valor do global (e raiz) antes do construtor: um
        // valor que se refere a si mesmo no construtor lê o objeto.
        self.emit(
            Instruction::StoreGlobal {
                symbol: value.clone(),
                val: obj.clone(),
                ty: Type::Ref,
                root: Some(vid.0),
            },
            Type::Void,
        );
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_marcar_constante".to_string(),
                args: vec![(obj.clone(), Type::Ref), (Operand::Constant(Constant::Function(self.func.symbol.clone())), Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_set".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(0)), Type::I64),
                    (Operand::Constant(Constant::Int(index as i64)), Type::I64),
                    (Operand::Constant(Constant::Int(0)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let name = self.ctx.symbol_name(var.name).to_string();
        let text = self.emit(Instruction::Const(Constant::String(name)), Type::Ref);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_set".to_string(),
                args: vec![
                    (obj.clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(1)), Type::I64),
                    (text, Type::I64),
                    (Operand::Constant(Constant::Int(1)), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        // O construtor do valor (o sem nome quando o valor não escolhe).
        let ast = &self.ctx.program.unit(unit).ast;
        let DeclKind::Enum(e) = &ast.decl(decl).kind else {
            self.terminate(Terminator::Return(Some(obj)));
            return;
        };
        let constant = &e.constants[index];
        let ctor_name = constant
            .constructor
            .map(|n| n.sym)
            .or_else(|| self.ctx.interner.lookup(""));
        let ctor = ctor_name.and_then(|s| {
            self.ctx.program.classes[cid.0 as usize]
                .constructors
                .get(&s)
                .copied()
        });
        if let Some(f) = ctor {
            let fid = f.0 as usize;
            if super::members::has_body(self.ctx, fid) {
                let saved = self.unit_id;
                self.unit_id = unit;
                let evaluated = match &constant.arguments {
                    Some(a) => self.evaluate_args(ast, &a.args),
                    None => Vec::new(),
                };
                let args = self.match_args(fid, &evaluated);
                self.unit_id = saved;
                self.call_direct(fid, Some(obj.clone()), args);
            }
        }
        self.terminate(Terminator::Return(Some(obj)));
    }

    /// `toString()` padrão de um enum: `Enum.value`.
    pub fn lower_enum_to_string(&mut self, cid: ClassId) {
        // Corpo chamado como função Dart (pelo `toString$c` e pelo
        // despacho): o retorno Ref é Owned.
        if self.ctx.arc_memory {
            self.dart_ref_returns.insert(self.func.symbol.clone());
        }
        let this = Operand::Val(self.add_param("this".to_string(), Type::Ref));
        let name = self.ctx.symbol_name(self.ctx.program.classes[cid.0 as usize].name).to_string();
        let prefix = self.emit(Instruction::Const(Constant::String(format!("{name}."))), Type::Ref);
        let n = self.enum_member(cid, "name", this).expect("enum");
        let r = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_string_concat".to_string(),
                args: vec![(prefix, Type::Ref), (n, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        self.terminate(Terminator::Return(Some(r)));
    }
}
