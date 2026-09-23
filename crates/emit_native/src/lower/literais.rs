//! Literais de coleção com elementos de controle (P3): `...e`, `...?e`,
//! `?e`, `if (c) e else f`, `for (…) e`, e o literal de conjunto.
//!
//! A especificação (§17.9 "Collection literals") avalia os elementos em
//! ordem e acrescenta cada valor produzido; `...e` acrescenta os elementos de
//! `e` (`...?e` ignora null); `if` e `for` produzem zero ou mais valores. O
//! literal só de expressões continua na alocação direta
//! (`AllocList`/`AllocMap`); os outros começam vazios e acrescentam.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, CollectionElement, ExprId, ForInTarget, ForInit};

/// Que coleção o literal constrói.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Colecao {
    Lista,
    Conjunto,
    Mapa,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// O literal `{…}` é um conjunto (e não um mapa)? Pelo tipo estático, ou
    /// pela forma dos elementos (`{}` vazio é mapa).
    pub fn literal_e_conjunto(&self, expr_id: ExprId, elements: &[CollectionElement]) -> bool {
        if let Some(t) = self.ctx.get_type(self.unit_id, expr_id)
            && let dartforge_types::table::Type::Interface { class, .. } = self.ctx.table.get(t)
        {
            match self.ctx.symbol_name(self.ctx.program.classes[class.0 as usize].name) {
                "Set" | "LinkedHashSet" => return true,
                "Map" | "LinkedHashMap" => return false,
                _ => {}
            }
        }
        fn folha(el: &CollectionElement) -> Option<bool> {
            match el {
                CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => Some(true),
                CollectionElement::MapEntry { .. } => Some(false),
                CollectionElement::If { then, .. } => folha(then),
                CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => folha(body),
                CollectionElement::Spread { .. } => None,
            }
        }
        elements.iter().find_map(folha).unwrap_or(false)
    }

    /// Coleção vazia do tipo pedido.
    fn colecao_vazia(&mut self, tipo: Colecao) -> Operand {
        if self.ctx.sdk_da_fonte && tipo != Colecao::Lista {
            return self.colecao_vazia_fonte(tipo == Colecao::Mapa);
        }
        match tipo {
            Colecao::Lista => self.emit(Instruction::AllocList { elements: Vec::new() }, Type::Ref),
            Colecao::Mapa => self.emit(Instruction::AllocMap { entries: Vec::new() }, Type::Ref),
            Colecao::Conjunto => {
                // `set_new` com zero pares: o ponteiro é de um vetor
                // constante qualquer (não é lido).
                let vazio = self.emit(Instruction::ConstArray(vec![0]), Type::Ptr);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_set_new".to_string(),
                        args: vec![(vazio, Type::Ptr), (Operand::Constant(Constant::Int(0)), Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )
            }
        }
    }

    /// Acrescenta um valor à lista ou ao conjunto.
    fn acrescentar(&mut self, alvo: Operand, tipo: Colecao, v: Operand) {
        if self.ctx.sdk_da_fonte && tipo == Colecao::Conjunto {
            self.chamar_por_nome(alvo, super::sdk_fonte::Tipo::Chamar, "add", &[(None, v)]);
            return;
        }
        let tag = self.operand_tag(&v);
        let (bits, _) = self.para_bits(v);
        let (nome, ret) = match tipo {
            Colecao::Lista => ("dartforge_list_push", Type::Void),
            _ => ("dartforge_set_add", Type::I8),
        };
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: nome.to_string(),
                args: vec![
                    (alvo, Type::Ref),
                    (bits, Type::I64),
                    (Operand::Constant(Constant::Int(i64::from(tag))), Type::I8),
                ],
                ret_ty: ret,
            },
            ret,
        );
    }

    fn acrescentar_entrada(&mut self, alvo: Operand, k: Operand, v: Operand) {
        if self.ctx.sdk_da_fonte {
            self.chamar_por_nome(alvo, super::sdk_fonte::Tipo::Chamar, "[]=", &[(None, k), (None, v)]);
            return;
        }
        let ktag = self.operand_tag(&k);
        let (kbits, _) = self.para_bits(k);
        let vtag = self.operand_tag(&v);
        let (vbits, _) = self.para_bits(v);
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_map_set".to_string(),
                args: vec![
                    (alvo, Type::Ref),
                    (kbits, Type::I64),
                    (Operand::Constant(Constant::Int(i64::from(ktag))), Type::I8),
                    (vbits, Type::I64),
                    (Operand::Constant(Constant::Int(i64::from(vtag))), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }

    /// Literal de coleção com elementos de controle (ou de conjunto).
    pub fn lower_literal_de_colecao(
        &mut self,
        ast: &ast::Ast,
        tipo: Colecao,
        elements: &[CollectionElement],
        span: Span,
    ) -> Operand {
        let alvo = self.colecao_vazia(tipo);
        for el in elements {
            self.elemento_de_colecao(ast, alvo.clone(), tipo, el, span);
        }
        alvo
    }

    fn elemento_de_colecao(
        &mut self,
        ast: &ast::Ast,
        alvo: Operand,
        tipo: Colecao,
        el: &CollectionElement,
        span: Span,
    ) {
        match el {
            CollectionElement::Expression(e) => {
                let v = self.lower_expr(ast, *e);
                if tipo == Colecao::Mapa {
                    self.nao_suportado("expressão num literal de mapa", span);
                    return;
                }
                self.acrescentar(alvo, tipo, v);
            }
            CollectionElement::NullAwareExpression(e) => {
                let v = self.lower_expr(ast, *e);
                self.se_nao_nulo(v, |s, v| s.acrescentar(alvo.clone(), tipo, v));
            }
            CollectionElement::MapEntry {
                key,
                value,
                null_aware_key,
                null_aware_value,
            } => {
                if *null_aware_key || *null_aware_value {
                    self.nao_suportado("entrada de mapa null-aware", span);
                    return;
                }
                let k = self.lower_expr(ast, *key);
                let v = self.lower_expr(ast, *value);
                self.acrescentar_entrada(alvo, k, v);
            }
            CollectionElement::Spread { value, null_aware } => {
                let fonte = self.lower_expr(ast, *value);
                let fonte = self.coagir(fonte, Type::Ref);
                if self.ctx.sdk_da_fonte {
                    // SDK da fonte: `...e` é o `addAll` da coleção (§17.9.1).
                    let espalhar = |s: &mut Self, f: Operand| {
                        s.chamar_por_nome(alvo.clone(), super::sdk_fonte::Tipo::Chamar, "addAll", &[(None, f)]);
                    };
                    if *null_aware {
                        self.se_nao_nulo(fonte, espalhar);
                    } else {
                        espalhar(self, fonte);
                    }
                    return;
                }
                if tipo == Colecao::Mapa {
                    self.nao_suportado("espalhamento num literal de mapa", span);
                    return;
                }
                let espalhar = |s: &mut Self, f: Operand| {
                    let n = s.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_generic_len".to_string(),
                            args: vec![(f.clone(), Type::Ref)],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    );
                    s.laco_indice(n, |s2, i| {
                        let x = s2.ler_elemento_iteravel(f.clone(), i, Type::Ref);
                        s2.acrescentar(alvo.clone(), tipo, x);
                    });
                };
                if *null_aware {
                    self.se_nao_nulo(fonte, espalhar);
                } else {
                    espalhar(self, fonte);
                }
            }
            CollectionElement::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                let c = self.lower_expr(ast, *condition);
                let b_senao = self.new_block();
                let fim = self.new_block();
                self.abrir_escopo();
                if let Some(p) = case_pattern {
                    let mut ligados = std::collections::HashSet::new();
                    let salvo = std::mem::replace(&mut self.padrao_refutavel, true);
                    self.casar(ast, *p, c, b_senao, super::padroes::Ligacao::Declarar, &mut ligados, *condition);
                    self.padrao_refutavel = salvo;
                    if let Some(g) = guard {
                        let ok = self.lower_expr(ast, *g);
                        let ok = self.para_bool(ok);
                        let segue = self.new_block();
                        self.terminate(Terminator::CondBranch {
                            cond: ok,
                            then_block: segue,
                            else_block: b_senao,
                        });
                        self.set_block(segue);
                    }
                } else {
                    let c = self.para_bool(c);
                    let b_entao = self.new_block();
                    self.terminate(Terminator::CondBranch {
                        cond: c,
                        then_block: b_entao,
                        else_block: b_senao,
                    });
                    self.set_block(b_entao);
                }
                self.elemento_de_colecao(ast, alvo.clone(), tipo, then, span);
                self.fechar_escopo();
                self.terminate(Terminator::Branch(fim));
                self.set_block(b_senao);
                if let Some(e) = else_ {
                    self.elemento_de_colecao(ast, alvo, tipo, e, span);
                }
                self.terminate(Terminator::Branch(fim));
                self.set_block(fim);
            }
            CollectionElement::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.abrir_escopo();
                match init {
                    Some(ForInit::Variables(lista)) => {
                        for var in lista.variables.iter() {
                            let ty = self.repr_do_local(var.name.span.start as usize);
                            let v = match var.initializer {
                                Some(i) => self.lower_expr(ast, i),
                                None => Self::valor_zero(ty),
                            };
                            self.declarar_variavel(var.name.sym, var.name.span.start as usize, ty, v);
                        }
                    }
                    Some(ForInit::Expression(e)) => {
                        self.lower_expr(ast, *e);
                    }
                    None => {}
                }
                let cabeca = self.new_block();
                let corpo = self.new_block();
                let fim = self.new_block();
                self.terminate(Terminator::Branch(cabeca));
                self.set_block(cabeca);
                match condition {
                    Some(c) => {
                        let c = self.lower_expr(ast, *c);
                        let c = self.para_bool(c);
                        self.terminate(Terminator::CondBranch {
                            cond: c,
                            then_block: corpo,
                            else_block: fim,
                        });
                    }
                    None => self.terminate(Terminator::Branch(corpo)),
                }
                self.set_block(corpo);
                self.elemento_de_colecao(ast, alvo, tipo, body, span);
                if let Some(ForInit::Variables(lista)) = init {
                    for var in lista.variables.iter() {
                        self.renovar_celula(var.name.sym);
                    }
                }
                for &u in updates.iter() {
                    self.lower_expr(ast, u);
                }
                self.terminate(Terminator::Branch(cabeca));
                self.set_block(fim);
                self.fechar_escopo();
            }
            CollectionElement::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                let fonte = self.lower_expr(ast, *iterable);
                let fonte = self.coagir(fonte, Type::Ref);
                if self.ctx.sdk_da_fonte {
                    let corpo: &CollectionElement = body;
                    self.iterar_fonte(fonte, &mut |s: &mut Self, x: Operand| {
                        s.abrir_escopo();
                        s.ligar_alvo_de_for_in(ast, target, x, *iterable, span);
                        s.elemento_de_colecao(ast, alvo.clone(), tipo, corpo, span);
                        s.fechar_escopo();
                    });
                    return;
                }
                let n = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_generic_len".to_string(),
                        args: vec![(fonte.clone(), Type::Ref)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let corpo: &CollectionElement = body;
                self.laco_indice(n, |s, i| {
                    s.abrir_escopo();
                    match target {
                        ForInTarget::Declared { name, .. } => {
                            let ty = s.repr_do_local(name.span.start as usize);
                            let x = s.ler_elemento_iteravel(fonte.clone(), i, ty);
                            s.declarar_variavel(name.sym, name.span.start as usize, ty, x);
                        }
                        ForInTarget::Pattern { pattern, .. } => {
                            let x = s.ler_elemento_iteravel(fonte.clone(), i, Type::Ref);
                            s.casar_irrefutavel(ast, *pattern, x, super::padroes::Ligacao::Declarar, *iterable);
                        }
                        ForInTarget::Expression(e) => {
                            if let ast::ExprKind::Identifier(id) = &ast.expr(*e).kind {
                                let x = s.ler_elemento_iteravel(fonte.clone(), i, Type::Ref);
                                s.gravar_local(id.sym, x);
                            } else {
                                s.nao_suportado("alvo de for-in em literal", span);
                            }
                        }
                    }
                    s.elemento_de_colecao(ast, alvo.clone(), tipo, corpo, span);
                    s.fechar_escopo();
                });
            }
        }
    }

    /// `if (v != null) f(v)`.
    fn se_nao_nulo(&mut self, v: Operand, f: impl FnOnce(&mut Self, Operand)) {
        if self.operand_type(&v) != Type::Ref {
            f(self, v);
            return;
        }
        let nn = self.emit(
            Instruction::ICmp(ICmpOp::Ne, v.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: nn,
            then_block: b,
            else_block: fim,
        });
        self.set_block(b);
        f(self, v);
        self.terminate(Terminator::Branch(fim));
        self.set_block(fim);
    }

    /// `for (i = 0; i < n; i++) f(i)` com o índice num `alloca` (R6).
    pub fn laco_indice(&mut self, n: Operand, f: impl FnOnce(&mut Self, Operand)) {
        let p = self.alloca_na_entrada(Type::I64);
        self.emit(
            Instruction::Store {
                ptr: p.clone(),
                val: Operand::Constant(Constant::Int(0)),
            },
            Type::Void,
        );
        let cabeca = self.new_block();
        let corpo = self.new_block();
        let fim = self.new_block();
        self.terminate(Terminator::Branch(cabeca));
        self.set_block(cabeca);
        let i = self.emit(
            Instruction::Load {
                ptr: p.clone(),
                ty: Type::I64,
            },
            Type::I64,
        );
        let c = self.emit(Instruction::ICmp(ICmpOp::Slt, i.clone(), n), Type::I1);
        self.terminate(Terminator::CondBranch {
            cond: c,
            then_block: corpo,
            else_block: fim,
        });
        self.set_block(corpo);
        f(self, i.clone());
        let prox = self.emit(Instruction::Add(i, Operand::Constant(Constant::Int(1))), Type::I64);
        self.emit(Instruction::Store { ptr: p, val: prox }, Type::Void);
        self.terminate(Terminator::Branch(cabeca));
        self.set_block(fim);
    }

    /// O elemento `i` de uma lista ou conjunto (`for-in`, espalhamento) na
    /// representação `repr` (R5).
    pub fn ler_elemento_iteravel(&mut self, fonte: Operand, i: Operand, repr: Type) -> Operand {
        if repr == Type::Ref {
            return self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_iteravel_get_ref".to_string(),
                    args: vec![(fonte, Type::Ref), (i, Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
        }
        let bits = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_iteravel_get_bits".to_string(),
                args: vec![(fonte, Type::Ref), (i, Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        self.bits_para(bits, repr)
    }
}
