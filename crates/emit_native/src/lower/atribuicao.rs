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

    /// Atribuição a um par getter/setter de topo explícito. `f` é o elemento
    /// resolvido (getter ou setter); o setter sai do escopo da biblioteca
    /// que o declara, e a leitura corrente (composta) do getter.
    fn atribuir_acessor_de_topo(
        &mut self,
        f: dartforge_elements::model::FunctionElementId,
        sym: dartforge_intern::SymbolId,
        ast: &ast::Ast,
        op: ast::AssignOp,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let func = &self.ctx.program.functions[f.0 as usize];
        let binding = self.ctx.program.lookup(func.library, sym);
        let setter = if func.kind == FunctionKind::Setter {
            Some(f)
        } else {
            binding.and_then(|b| b.setter).and_then(|el| match el {
                Element::Function(s) => Some(s),
                _ => None,
            })
        };
        let getter = if func.kind == FunctionKind::Setter {
            binding.and_then(|b| b.getter).and_then(|el| match el {
                Element::Function(g) => Some(g),
                _ => None,
            })
        } else {
            Some(f)
        };
        let Some(setter) = setter.filter(|s| super::funcao_do_usuario(self.ctx, s.0 as usize)) else {
            let n = self.ctx.symbol_name(sym).to_string();
            return self.nao_suportado(&format!("atribuição a `{n}` sem setter"), span);
        };
        let cur = match (matches!(op, ast::AssignOp::Compound(_)), getter) {
            (true, Some(g)) => Some(self.chamar_direto(g.0 as usize, None, Vec::new())),
            (true, None) => {
                let n = self.ctx.symbol_name(sym).to_string();
                return self.nao_suportado(&format!("leitura de `{n}` sem getter"), span);
            }
            _ => None,
        };
        let v = self.combinar(ast, op, cur, value);
        self.chamar_direto(setter.0 as usize, None, vec![v.clone()]);
        v
    }

    /// `a?.b op= v` / `a?[i] op= v` (§17.23): `a` é avaliado uma vez; se é
    /// null, nada mais é avaliado e o valor é null; senão, a atribuição
    /// comum sobre o valor de `a`. A cadeia `?.` de `a` (`x?.y?.b = v`)
    /// desvia para o mesmo null.
    fn atribuir_com_null_aware(
        &mut self,
        ast: &ast::Ast,
        op: ast::AssignOp,
        target: ExprId,
        recv: ExprId,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let saida = self.new_block();
        let cadeia_salva = self.cadeia_nula.replace((saida, Vec::new()));
        let recv_op = self.lower_alvo(ast, recv);
        self.desviar_se_nulo(&recv_op);
        self.receptor_pronto = Some((recv, recv_op));
        self.null_aware_tratado = Some(target);
        let mut v = self.lower_atribuicao(ast, op, target, value, span);
        self.receptor_pronto = None;
        // O valor antigo (`a?.x++`) também passa pela junção: null quando a
        // cadeia desviou.
        let mut antigo = self.valor_antigo.take();
        let (saida, mut entradas) = std::mem::replace(&mut self.cadeia_nula, cadeia_salva).expect("cadeia aberta");
        let mut entradas_antigo: Vec<(BlockId, Operand)> =
            entradas.iter().map(|(b, _)| (*b, Operand::Constant(Constant::Null))).collect();
        if !self.is_terminated() {
            v = self.coagir(v, Type::Ref);
            if let Some(a) = antigo.take() {
                let a = self.coagir(a, Type::Ref);
                entradas_antigo.push((self.current_block, a));
            }
            entradas.push((self.current_block, v));
            self.terminate(Terminator::Branch(saida));
        }
        self.set_block(saida);
        if entradas.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        if entradas_antigo.len() == entradas.len() {
            self.valor_antigo = Some(self.emit(Instruction::Phi { incoming: entradas_antigo, ty: Type::Ref }, Type::Ref));
        }
        self.emit(Instruction::Phi { incoming: entradas, ty: Type::Ref }, Type::Ref)
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
                let resolvido = match self.ctx.get_resolved(self.unit_id, target).cloned() {
                    // Sem resolução (corpo de closure) e sem local: pelo
                    // escopo léxico (membro da classe, topo da biblioteca).
                    None if self.buscar_local(sym).is_none() => self.resolver_por_nome(sym),
                    r => r,
                };
                match resolvido {
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
                    // Getter/setter de topo explícitos (`set exitCode(int)`):
                    // o elemento resolvido pode ser o getter; o setter é o
                    // outro lado do mesmo nome no escopo.
                    Some(Resolved::Element(Element::Function(f))) => {
                        self.atribuir_acessor_de_topo(f, sym, ast, op, value, span)
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
                        match self.gravar_local(sym, v) {
                            Some(v) => v,
                            None => self.nao_suportado("atribuição a local desconhecido", span),
                        }
                    }
                }
            }
            ast::ExprKind::Property {
                target: recv,
                name,
                null_aware,
            } => {
                if *null_aware && self.null_aware_tratado != Some(target) {
                    return self.atribuir_com_null_aware(ast, op, target, *recv, value, span);
                }
                self.null_aware_tratado = None;
                // `prefixo.x = v`: variável de topo importada com prefixo.
                if let Some(el) = self.elemento_prefixado(ast, *recv, name.sym) {
                    let vid = match el {
                        Element::Variable(v) => Some(v),
                        Element::Function(f) => self.ctx.program.functions[f.0 as usize].variable,
                        _ => None,
                    };
                    if let Some(vid) = vid {
                        let cur = if composto {
                            Some(self.ler_global(vid, span))
                        } else {
                            None
                        };
                        let v = self.combinar(ast, op, cur, value);
                        return self.gravar_global(vid, v, span);
                    }
                }
                // `super.x = v` (P4).
                if matches!(ast.expr(*recv).kind, ast::ExprKind::Super) {
                    let cur = if composto {
                        Some(self.ler_super(name.sym, span))
                    } else {
                        None
                    };
                    let v = self.combinar(ast, op, cur, value);
                    return self.gravar_super(name.sym, v, span);
                }
                let alvo_e_classe = matches!(
                    self.ctx.get_resolved(self.unit_id, *recv),
                    Some(Resolved::Element(Element::Class(_)))
                );
                if alvo_e_classe {
                    let Some(Resolved::Member { class, member, .. }) =
                        self.ctx.get_resolved(self.unit_id, target).cloned()
                    else {
                        let n = self.ctx.symbol_name(name.sym).to_string();
                        return self.nao_suportado(&format!("atribuição a `{n}`"), span);
                    };
                    let member = if matches!(member, MemberRef::Function(f)
                        if self.ctx.program.functions[f.0 as usize].kind != FunctionKind::Setter
                            && self.ctx.program.functions[f.0 as usize].variable.is_none()) {
                        let key = format!("{}_=", self.ctx.symbol_name(name.sym));
                        self.ctx.interner.lookup(&key)
                            .and_then(|s| self.ctx.program.classes[class.0 as usize].static_members.get(&s).copied())
                            .map(MemberRef::Function)
                            .unwrap_or(member)
                    } else { member };
                    return self.atribuir_membro(None, member, ast, op, value, span);
                }
                let Some((_, member)) = self.membro_do_usuario(target, *recv, name.sym, true)
                else {
                    let n = self.ctx.symbol_name(name.sym).to_string();
                    if self.receptor_dinamico(*recv) {
                        if self.ctx.sdk_da_fonte {
                            // Em SDK da fonte, até um setter inexistente passa
                            // pelo seletor: o runtime produz NoSuchMethodError.
                            // O RHS é avaliado depois do receptor, como em Dart.
                            let recv_op = self.lower_expr(ast, *recv);
                            let cur = composto.then(|| self.chamar_por_nome(
                                recv_op.clone(), super::sdk_fonte::Tipo::Ler, &n, &[],
                            ));
                            let v = self.combinar(ast, op, cur, value);
                            self.chamar_por_nome(
                                recv_op, super::sdk_fonte::Tipo::Gravar, &n,
                                &[(None, v.clone())],
                            );
                            return v;
                        }
                        // Receptor sem tipo útil: campo/setter pela classe
                        // dinâmica.
                        let alvos = self.alvos_de_escrita(&n);
                        if !alvos.is_empty() {
                            let recv_op = self.lower_expr(ast, *recv);
                            let recv_op = self.coagir(recv_op, Type::Ref);
                            let cur = if composto {
                                let leitura = self.alvos_por_nome(&n);
                                let n2 = n.clone();
                                Some(self.despachar(
                                    recv_op.clone(),
                                    &leitura,
                                    super::despacho::Uso::Ler,
                                    &mut |_s: &mut Self| Vec::new(),
                                    &mut |s: &mut Self| s.lancar_nsm(&n2),
                                    span,
                                ))
                            } else {
                                None
                            };
                            let v = self.combinar(ast, op, cur, value);
                            let v2 = v.clone();
                            let n3 = format!("{n}=");
                            return self.despachar(
                                recv_op,
                                &alvos,
                                super::despacho::Uso::Gravar,
                                &mut |_s: &mut Self| vec![(None, v2.clone())],
                                &mut |s: &mut Self| s.lancar_nsm(&n3),
                                span,
                            );
                        }
                        // Mundo fechado sem setter com esse nome. Ainda é uma
                        // expressão válida: avaliar receptor e valor, então
                        // lançar NoSuchMethodError em tempo de execução.
                        self.lower_expr(ast, *recv);
                        if composto {
                            return self.lancar_nsm(&n);
                        }
                        self.lower_rhs(ast, value, Type::Ref);
                        return self.lancar_nsm(&format!("{n}="));
                    }
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
                if *null_aware && self.null_aware_tratado != Some(target) {
                    return self.atribuir_com_null_aware(ast, op, target, *t, value, span);
                }
                self.null_aware_tratado = None;
                let t_op = self.lower_expr(ast, *t);
                let i_op = self.lower_expr(ast, *index);
                if self.ctx.sdk_da_fonte {
                    // SDK da fonte: `[]`/`[]=` pela classe dinâmica.
                    use super::sdk_fonte::Tipo;
                    let cur = composto.then(|| self.chamar_por_nome(t_op.clone(), Tipo::Chamar, "[]", &[(None, i_op.clone())]));
                    let v = self.combinar(ast, op, cur, value);
                    self.chamar_por_nome(t_op, Tipo::Chamar, "[]=", &[(None, i_op), (None, v.clone())]);
                    return v;
                }
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
                if !composto {
                    return self.gravar_indice_dinamico(ast, t_op, i_op, value, span);
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
            if !self.gravar_campo_fonte(obj.clone(), vid, v.clone()) {
                self.gravar_campo(obj, vid, v.clone(), span);
            }
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
        let estatico = func.static_;
        let nome = self.ctx.symbol_name(func.name).to_string();
        let cid = func.class;
        let obj = if estatico { None } else { recv.or_else(|| self.this_param.clone()) };
        let cur = if composto {
            let getter = cid.and_then(|cid| {
                if estatico {
                    let sym = self.ctx.interner.lookup(&nome)?;
                    self.ctx.program.classes[cid.0 as usize].static_members.get(&sym).copied()
                } else {
                    self.membro_na_classe(cid, &nome)
                        .map(|fid| dartforge_elements::model::FunctionElementId(fid as u32))
                }
            });
            let Some(getter) = getter else {
                return self.nao_suportado("getter ausente para atribuição composta", span);
            };
            let membro_getter = MemberRef::Function(getter);
            if estatico {
                Some(self.ler_membro_estatico(membro_getter, span))
            } else {
                let Some(receptor) = obj.clone() else {
                    return self.nao_suportado("setter fora de membro de instância", span);
                };
                let getter_fid = getter.0 as usize;
                if let Some(vid) = self.ctx.program.functions[getter_fid].variable {
                    Some(self.ler_campo_com_late(receptor, vid, span))
                } else {
                    Some(self.chamar_membro(receptor, getter_fid, &[], span))
                }
            }
        } else { None };
        let v = self.combinar(ast, op, cur, value);
        if estatico {
            let args = self.casar_args(fid, &[(None, v.clone())]);
            self.chamar_direto(fid, None, args);
            return v;
        }
        let Some(obj) = obj else {
            return self.nao_suportado("setter fora de membro de instância", span);
        };
        self.chamar_membro(obj, fid, &[(None, v.clone())], span);
        v
    }

    /// `a[i] = v` com `a` sem tipo estático útil: lista ou mapa do runtime
    /// pela classe do valor, `operator []=` de uma classe do programa, senão
    /// `NoSuchMethodError`.
    fn gravar_indice_dinamico(
        &mut self,
        ast: &ast::Ast,
        alvo: Operand,
        indice: Operand,
        value: Rhs,
        span: Span,
    ) -> Operand {
        let v = self.lower_rhs(ast, value, Type::Ref);
        let alvo = self.coagir(alvo, Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(alvo.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let b_lista = self.new_block();
        let b_mapa = self.new_block();
        let b_outro = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::Switch {
            val: cls,
            default: b_outro,
            cases: vec![(-3, b_lista), (-4, b_mapa)],
        });
        self.set_block(b_lista);
        let i = self.coagir(indice.clone(), Type::I64);
        let tag = self.operand_tag(&v);
        let (bits, _) = self.para_bits(v.clone());
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_list_set".to_string(),
                args: vec![
                    (alvo.clone(), Type::Ref),
                    (i, Type::I64),
                    (bits, Type::I64),
                    (Operand::Constant(Constant::Int(i64::from(tag))), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.terminate(Terminator::Branch(fim));
        self.set_block(b_mapa);
        let ktag = self.operand_tag(&indice);
        let (kbits, _) = self.para_bits(indice.clone());
        let vtag = self.operand_tag(&v);
        let (vbits, _) = self.para_bits(v.clone());
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_map_set".to_string(),
                args: vec![
                    (alvo.clone(), Type::Ref),
                    (kbits, Type::I64),
                    (Operand::Constant(Constant::Int(i64::from(ktag))), Type::I8),
                    (vbits, Type::I64),
                    (Operand::Constant(Constant::Int(i64::from(vtag))), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.terminate(Terminator::Branch(fim));
        self.set_block(b_outro);
        let alvos: Vec<_> = self
            .alvos_por_nome("[]=")
            .into_iter()
            .filter(|(_, a)| matches!(a, super::despacho::Alvo::Funcao(_)))
            .collect();
        let (i2, v2) = (indice, v.clone());
        self.despachar(
            alvo,
            &alvos,
            super::despacho::Uso::Chamar,
            &mut |_s: &mut Self| vec![(None, i2.clone()), (None, v2.clone())],
            &mut |s: &mut Self| s.lancar_nsm("[]="),
            span,
        );
        self.terminate(Terminator::Branch(fim));
        self.set_block(fim);
        v
    }
}
