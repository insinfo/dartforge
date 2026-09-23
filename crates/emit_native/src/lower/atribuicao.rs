//! Atribuição a identificador, propriedade e índice.
//!
//! Antes só o identificador gravava; `a.b = v` e `a[i] = v` avaliavam o valor
//! e o descartavam — a leitura seguinte via o campo zerado (H4 do plano,
//! `docs/NATIVO-PLANO.md` §6).

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind};
use dartforge_frontend::ast::{self, ExprId};
use dartforge_types::resolved::{MemberRef, Resolved};

/// Lado direito de uma atribuição: uma expressão, ou o `1` de `++`/`--`.
#[derive(Clone, Copy)]
pub enum Rhs {
    Expr(ExprId),
    Um,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    fn lower_rhs(&mut self, ast: &ast::Ast, rhs: Rhs, como: Type) -> Operand {
        match rhs {
            Rhs::Expr(e) => self.lower_expr(ast, e),
            Rhs::Um if como == Type::F64 => Operand::Constant(Constant::Double(1.0)),
            Rhs::Um => Operand::Constant(Constant::Int(1)),
        }
    }

    /// Valor a gravar: `v`, `cur op v`, ou `cur ?? v` (`??=`). O valor
    /// corrente fica em `valor_antigo` para o `x++` pós-fixo.
    fn combinar(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        cur: Option<Operand>,
        value: Rhs,
    ) -> Operand {
        self.valor_antigo = cur.clone();
        match (op, cur) {
            (ast::AssignOp::Compound(ast::BinaryOp::IfNull), Some(cur)) => {
                let e_nulo = self.emit(
                    Instruction::ICmp(ICmpOp::Eq, cur.clone(), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let b_novo = self.new_block();
                let b_fim = self.new_block();
                let origem = self.current_block;
                self.terminate(Terminator::CondBranch {
                    cond: e_nulo,
                    then_block: b_novo,
                    else_block: b_fim,
                });
                self.set_block(b_novo);
                let ty = self.operand_type(&cur);
                let v = self.lower_rhs(ast, value, ty);
                let v = self.coagir(v, ty);
                let fim_novo = self.current_block;
                self.terminate(Terminator::Branch(b_fim));
                self.set_block(b_fim);
                self.emit(
                    Instruction::Phi {
                        incoming: vec![(origem, cur), (fim_novo, v)],
                        ty,
                    },
                    ty,
                )
            }
            (ast::AssignOp::Compound(b), Some(cur)) => {
                let ty = self.operand_type(&cur);
                let rhs = self.lower_rhs(ast, value, ty);
                self.lower_binary_op_helper(b, cur, rhs)
            }
            _ => self.lower_rhs(ast, value, Type::Ref),
        }
    }

    /// Atribuição. Ordem de avaliação do Dart: receptor, índice, leitura
    /// corrente (se composta), valor, gravação.
    pub fn lower_atribuicao(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        target: ExprId,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let composto = matches!(op, ast::AssignOp::Compound(_));
        match &ast.expr(target).kind {
            ast::ExprKind::Identifier(name) => {
                let sym = name.sym;
                match self.ctx.get_resolved(self.unit_id, target).cloned() {
                    Some(Resolved::Member { member, .. }) => {
                        self.atribuir_membro(None, member, ast, op, value, span)
                    }
                    Some(Resolved::Element(Element::Variable(vid))) => {
                        let cur = if composto {
                            Some(self.ler_global(vid, span))
                        } else {
                            None
                        };
                        let v = self.combinar(ast, op, cur, value);
                        self.gravar_global(vid, v, span)
                    }
                    Some(Resolved::Element(Element::Function(f)))
                        if self.ctx.program.functions[f.0 as usize].variable.is_some() =>
                    {
                        let vid = self.ctx.program.functions[f.0 as usize]
                            .variable
                            .expect("acessor");
                        let cur = if composto {
                            Some(self.ler_global(vid, span))
                        } else {
                            None
                        };
                        let v = self.combinar(ast, op, cur, value);
                        self.gravar_global(vid, v, span)
                    }
                    _ => {
                        let cur = if composto {
                            self.ler_local_por_nome(sym)
                        } else {
                            None
                        };
                        if composto && cur.is_none() {
                            return self
                                .nao_suportado("atribuição composta a local desconhecido", span);
                        }
                        let v = self.combinar(ast, op, cur, value);
                        if let Some(ptr) = self.local_ptrs.get(&sym).cloned() {
                            self.emit(
                                Instruction::Store {
                                    ptr,
                                    val: v.clone(),
                                },
                                Type::Void,
                            );
                        }
                        self.named_locals.insert(sym, v.clone());
                        v
                    }
                }
            }
            ast::ExprKind::Property {
                target: recv,
                name,
                null_aware,
            } => {
                if *null_aware {
                    return self.nao_suportado("atribuição com `?.`", span);
                }
                let alvo_e_classe = matches!(
                    self.ctx.get_resolved(self.unit_id, *recv),
                    Some(Resolved::Element(Element::Class(_)))
                );
                if alvo_e_classe {
                    let Some(Resolved::Member { member, .. }) =
                        self.ctx.get_resolved(self.unit_id, target).cloned()
                    else {
                        let n = self.ctx.symbol_name(name.sym).to_string();
                        return self.nao_suportado(&format!("atribuição a `{n}`"), span);
                    };
                    return self.atribuir_membro(None, member, ast, op, value, span);
                }
                let Some((_, member)) = self.membro_do_usuario(target, *recv, name.sym, true)
                else {
                    let n = self.ctx.symbol_name(name.sym).to_string();
                    return self.nao_suportado(&format!("atribuição a `{n}`"), span);
                };
                let recv_op = self.lower_expr(ast, *recv);
                self.atribuir_membro(Some(recv_op), member, ast, op, value, span)
            }
            ast::ExprKind::Index {
                target: t,
                index,
                null_aware,
            } => {
                if *null_aware {
                    return self.nao_suportado("atribuição com `?[`", span);
                }
                let t_op = self.lower_expr(ast, *t);
                let i_op = self.lower_expr(ast, *index);
                if let Some(cid) = self.classe_do_usuario_de(*t) {
                    let (Some(set), get) = (
                        self.membro_na_classe(cid, "[]="),
                        self.membro_na_classe(cid, "[]"),
                    ) else {
                        return self.nao_suportado("operador []= ausente", span);
                    };
                    let cur = if composto {
                        let Some(get) = get else {
                            return self.nao_suportado("operador [] ausente", span);
                        };
                        Some(self.chamar_membro(t_op.clone(), get, &[(None, i_op.clone())], span))
                    } else {
                        None
                    };
                    let v = self.combinar(ast, op, cur, value);
                    self.chamar_membro(t_op, set, &[(None, i_op), (None, v.clone())], span);
                    return v;
                }
                let t_ty = self.ctx.get_type(self.unit_id, *t);
                let e_mapa = t_ty.is_some_and(|ty| self.ctx.is_map(ty));
                let e_lista = t_ty.is_some_and(|ty| self.ctx.is_list(ty));
                if e_lista {
                    let cur = if composto {
                        Some(self.emit_call_with_check(
                            Instruction::CallRuntime {
                                name: "dartforge_list_get_bits".to_string(),
                                args: vec![(t_op.clone(), Type::Ref), (i_op.clone(), Type::I64)],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ))
                    } else {
                        None
                    };
                    let v = self.combinar(ast, op, cur, value);
                    let tag = self.operand_tag(&v);
                    let (bits, _) = self.para_bits(v.clone());
                    self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_list_set".to_string(),
                            args: vec![
                                (t_op, Type::Ref),
                                (i_op, Type::I64),
                                (bits, Type::I64),
                                (Operand::Constant(Constant::Int(i64::from(tag))), Type::I8),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    );
                    return v;
                }
                if e_mapa {
                    let ktag = self.operand_tag(&i_op);
                    let (kbits, _) = self.para_bits(i_op);
                    let cur = if composto {
                        Some(self.emit(
                            Instruction::CallRuntime {
                                name: "dartforge_map_get_bits".to_string(),
                                args: vec![
                                    (t_op.clone(), Type::Ref),
                                    (kbits.clone(), Type::I64),
                                    (Operand::Constant(Constant::Int(i64::from(ktag))), Type::I8),
                                ],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        ))
                    } else {
                        None
                    };
                    let v = self.combinar(ast, op, cur, value);
                    let vtag = self.operand_tag(&v);
                    let (vbits, _) = self.para_bits(v.clone());
                    self.emit_call_with_check(
                        Instruction::CallRuntime {
                            name: "dartforge_map_set".to_string(),
                            args: vec![
                                (t_op, Type::Ref),
                                (kbits, Type::I64),
                                (Operand::Constant(Constant::Int(i64::from(ktag))), Type::I8),
                                (vbits, Type::I64),
                                (Operand::Constant(Constant::Int(i64::from(vtag))), Type::I8),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    );
                    return v;
                }
                self.nao_suportado("atribuição a índice", span)
            }
            _ => self.nao_suportado("alvo de atribuição", span),
        }
    }

    /// Grava num membro: campo (de `recv`, ou de `this` quando `recv` é
    /// None), global estático, ou setter.
    fn atribuir_membro(
        &mut self,
        recv: Option<Operand>,
        member: MemberRef,
        ast: &ast::Ast,
        op: ast::AssignOp,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let composto = matches!(op, ast::AssignOp::Compound(_));
        let vid = match member {
            MemberRef::Variable(v) => Some(v),
            MemberRef::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
        };
        if let Some(vid) = vid {
            if super::e_global(self.ctx, vid) {
                let cur = if composto {
                    Some(self.ler_global(vid, span))
                } else {
                    None
                };
                let v = self.combinar(ast, op, cur, value);
                return self.gravar_global(vid, v, span);
            }
            let Some(obj) = recv.or_else(|| self.this_param.clone()) else {
                return self.nao_suportado("campo de instância fora de membro de instância", span);
            };
            let cur = if composto {
                Some(self.ler_campo_com_late(obj.clone(), vid, span))
            } else {
                None
            };
            let v = self.combinar(ast, op, cur, value);
            self.gravar_campo(obj, vid, v.clone(), span);
            return v;
        }
        let MemberRef::Function(f) = member else {
            unreachable!()
        };
        let fid = f.0 as usize;
        let func = &self.ctx.program.functions[fid];
        if func.kind != FunctionKind::Setter || !super::funcao_do_usuario(self.ctx, fid) {
            return self.nao_suportado("atribuição a membro que não é campo nem setter", span);
        }
        if composto {
            return self.nao_suportado("atribuição composta via setter", span);
        }
        let estatico = func.static_;
        let v = self.lower_rhs(ast, value, Type::Ref);
        if estatico {
            let args = self.casar_args(fid, &[(None, v.clone())]);
            self.chamar_direto(fid, None, args);
            return v;
        }
        let Some(obj) = recv.or_else(|| self.this_param.clone()) else {
            return self.nao_suportado("setter fora de membro de instância", span);
        };
        self.chamar_membro(obj, fid, &[(None, v.clone())], span);
        v
    }
}
