//! Records com campo nomeado (P3).
//!
//! O record só posicional é o `Value::Record` do runtime (ele o imprime). O
//! runtime não tem a forma (os nomes) de um record com campo nomeado, então
//! cada **forma** do programa — número de posicionais e nomes ordenados,
//! coletada de todos os literais, padrões e tipos antes do lowering
//! (`Context::formas_de_record`) — é uma "classe" do programa: um objeto com
//! os campos posicionais e depois os nomeados (em ordem de nome), todos
//! guardados como referência, com id `ID_BASE_DE_FORMA + k`. `toString()` e
//! `==` são gerados (a especificação §20.6 "Records": igualdade estrutural
//! campo a campo, mesma forma), e o acesso `r.x`/`r.$1` sem tipo estático
//! testa as formas que têm o campo.

use super::fn_builder::FnBuilder;
use crate::context::ID_BASE_DE_FORMA;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, ExprId};

/// Símbolo da igualdade estrutural dos records com forma.
pub const SIMBOLO_IGUAL: &str = "df.$registro.igual";

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `(e1, …, nome: e)` com algum campo nomeado.
    pub fn lower_registro_nomeado(
        &mut self,
        ast: &ast::Ast,
        positional: &[ExprId],
        named: &[(ast::Name, ExprId)],
        span: Span,
    ) -> Operand {
        // A ordem de avaliação é a do texto.
        let mut itens: Vec<(u32, Option<String>, ExprId)> = positional
            .iter()
            .map(|&e| (ast.expr(e).span.start as u32, None, e))
            .collect();
        for (n, e) in named {
            itens.push((
                ast.expr(*e).span.start as u32,
                Some(self.ctx.symbol_name(n.sym).to_string()),
                *e,
            ));
        }
        itens.sort_by_key(|(s, _, _)| *s);
        let mut nomes: Vec<String> = named.iter().map(|(n, _)| self.ctx.symbol_name(n.sym).to_string()).collect();
        nomes.sort();
        let Some(id) = self.ctx.id_da_forma(positional.len(), &nomes) else {
            return self.nao_suportado("record com forma desconhecida", span);
        };
        let mut valores = Vec::with_capacity(itens.len());
        let mut k_pos = 0usize;
        for (_, nome, e) in itens {
            let v = self.lower_expr(ast, e);
            let v = self.coagir(v, Type::Ref);
            let idx = match nome {
                None => {
                    let i = k_pos;
                    k_pos += 1;
                    i
                }
                Some(n) => positional.len() + nomes.iter().position(|x| *x == n).expect("nome da forma"),
            };
            valores.push((idx, v));
        }
        let obj = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_new".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(i64::from(id))), Type::I64),
                    (Operand::Constant(Constant::Int((positional.len() + nomes.len()) as i64)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        for (idx, v) in valores {
            self.emit(
                Instruction::CallRuntime {
                    name: "dartforge_object_set".to_string(),
                    args: vec![
                        (obj.clone(), Type::Ref),
                        (Operand::Constant(Constant::Int(idx as i64)), Type::I64),
                        (v, Type::I64),
                        (Operand::Constant(Constant::Int(1)), Type::I8),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
        obj
    }

    /// As formas com o campo `nome` (`$k` ou nomeado): (id, índice).
    pub fn formas_com_campo(&self, nome: &str) -> Vec<(i64, usize)> {
        let posicional = nome.strip_prefix('$').and_then(|d| d.parse::<usize>().ok()).filter(|k| *k >= 1);
        let mut saida = Vec::new();
        for (k, (npos, nomes)) in self.ctx.formas_de_record.iter().enumerate() {
            let id = i64::from(ID_BASE_DE_FORMA) + k as i64;
            match posicional {
                Some(p) if p <= *npos => saida.push((id, p - 1)),
                Some(_) => {}
                None => {
                    if let Some(j) = nomes.iter().position(|n| n == nome) {
                        saida.push((id, npos + j));
                    }
                }
            }
        }
        saida
    }

    /// Lê o campo `i` (referência) de um record com forma.
    pub fn campo_de_forma(&mut self, obj: Operand, i: usize) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (Operand::Constant(Constant::Int(i as i64)), Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// `r.x`/`r.$k` pela classe dinâmica do receptor (o que não é record vai
    /// a `padrao`); `None` quando nenhuma forma tem o campo e ele não é
    /// posicional.
    pub fn ler_campo_de_registro(
        &mut self,
        recv: Operand,
        nome: &str,
        padrao: &mut dyn FnMut(&mut Self) -> Operand,
    ) -> Option<Operand> {
        let formas = self.formas_com_campo(nome);
        let posicional = nome.strip_prefix('$').and_then(|d| d.parse::<i64>().ok()).filter(|k| *k >= 1);
        if formas.is_empty() && posicional.is_none() {
            return None;
        }
        let recv = self.coagir(recv, Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let juncao = self.new_block();
        let b_padrao = self.new_block();
        let mut casos = Vec::new();
        let mut blocos = Vec::new();
        for &(id, i) in &formas {
            let b = self.new_block();
            casos.push((id, b));
            blocos.push((b, i));
        }
        let b_runtime = self.new_block();
        casos.push((-7, b_runtime));
        self.terminate(Terminator::Switch {
            val: cls,
            default: b_padrao,
            cases: casos,
        });
        let mut entradas = Vec::new();
        for (b, i) in blocos {
            self.set_block(b);
            let v = self.campo_de_forma(recv.clone(), i);
            entradas.push((self.current_block, v));
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(b_runtime);
        match posicional {
            Some(k) => {
                let v = self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_record_get_ref".to_string(),
                        args: vec![(recv.clone(), Type::Ref), (Operand::Constant(Constant::Int(k - 1)), Type::I64)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                );
                entradas.push((self.current_block, v));
                self.terminate(Terminator::Branch(juncao));
            }
            None => {
                // Record posicional do runtime não tem campo nomeado.
                self.lancar_nsm(nome);
            }
        }
        self.set_block(b_padrao);
        // O que não é record: o caminho de sempre; se ele não sabe o nome
        // (o membro só existe em records), `NoSuchMethodError`.
        let n_erros = self.erros.len();
        let mut v = padrao(self);
        if self.erros.len() > n_erros {
            self.erros.truncate(n_erros);
            v = self.lancar_nsm(nome);
        }
        if !matches!(self.operand_type(&v), Type::Void | Type::Ptr) {
            let v = self.coagir(v, Type::Ref);
            if !self.is_terminated() {
                entradas.push((self.current_block, v));
                self.terminate(Terminator::Branch(juncao));
            }
        } else if !self.is_terminated() {
            entradas.push((self.current_block, Operand::Constant(Constant::Null)));
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        if entradas.is_empty() {
            return Some(Operand::Constant(Constant::Null));
        }
        Some(self.emit(
            Instruction::Phi {
                incoming: entradas,
                ty: Type::Ref,
            },
            Type::Ref,
        ))
    }

    /// `toString()` da forma `k`: `(p0, p1, a: x, b: y)`.
    pub fn lower_to_string_de_forma(&mut self, k: usize) {
        let this = Operand::Val(self.add_param("this".to_string(), Type::Ref));
        let (npos, nomes) = self.ctx.formas_de_record[k].clone();
        let mut texto = self.emit(Instruction::Const(Constant::String("(".to_string())), Type::Ref);
        for i in 0..npos + nomes.len() {
            let mut prefixo = String::new();
            if i > 0 {
                prefixo.push_str(", ");
            }
            if i >= npos {
                prefixo.push_str(&nomes[i - npos]);
                prefixo.push_str(": ");
            }
            if !prefixo.is_empty() {
                let p = self.emit(Instruction::Const(Constant::String(prefixo)), Type::Ref);
                texto = self.concatenar(texto, p);
            }
            let v = self.campo_de_forma(this.clone(), i);
            let s = self.texto_de(v);
            texto = self.concatenar(texto, s);
        }
        let fim = self.emit(Instruction::Const(Constant::String(")".to_string())), Type::Ref);
        let r = self.concatenar(texto, fim);
        self.terminate(Terminator::Return(Some(r)));
    }

    fn concatenar(&mut self, a: Operand, b: Operand) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_string_concat".to_string(),
                args: vec![(a, Type::Ref), (b, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// `df.$registro.igual(a, b)`: mesma forma e campos iguais (`==` de cada
    /// campo); o que não é record com forma vai ao `==` do runtime.
    pub fn lower_igualdade_de_registros(&mut self) {
        let a = Operand::Val(self.add_param("a".to_string(), Type::Ref));
        let b = Operand::Val(self.add_param("b".to_string(), Type::Ref));
        let ca = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(a.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let cb = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(b.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let formas = self.ctx.formas_de_record.clone();
        let b_runtime = self.new_block();
        let b_falso = self.new_block();
        let mut casos = Vec::new();
        let mut blocos = Vec::new();
        for (k, (npos, nomes)) in formas.iter().enumerate() {
            let bl = self.new_block();
            casos.push((i64::from(ID_BASE_DE_FORMA) + k as i64, bl));
            blocos.push((bl, k, npos + nomes.len()));
        }
        self.terminate(Terminator::Switch {
            val: ca.clone(),
            default: b_runtime,
            cases: casos,
        });
        for (bl, k, n) in blocos {
            self.set_block(bl);
            let mesma = self.emit(
                Instruction::ICmp(
                    ICmpOp::Eq,
                    cb.clone(),
                    Operand::Constant(Constant::Int(i64::from(ID_BASE_DE_FORMA) + k as i64)),
                ),
                Type::I1,
            );
            let b_campos = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: mesma,
                then_block: b_campos,
                else_block: b_falso,
            });
            self.set_block(b_campos);
            for i in 0..n {
                let x = self.campo_de_forma(a.clone(), i);
                let y = self.campo_de_forma(b.clone(), i);
                let ok = self.operar(ast::BinaryOp::Eq, x, y, false, Span { start: 0, end: 0 });
                let ok = self.para_bool(ok);
                let seguinte = self.new_block();
                self.terminate(Terminator::CondBranch {
                    cond: ok,
                    then_block: seguinte,
                    else_block: b_falso,
                });
                self.set_block(seguinte);
            }
            self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Bool(true)))));
        }
        self.set_block(b_falso);
        self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Bool(false)))));
        self.set_block(b_runtime);
        let r = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_equal".to_string(),
                args: vec![(a, Type::Ref), (b, Type::Ref)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let r = self.emit(
            Instruction::Trunc {
                op: r,
                from: Type::I8,
                to: Type::I1,
            },
            Type::I1,
        );
        self.terminate(Terminator::Return(Some(r)));
    }

    /// Hash de uma forma de record: valores iguais precisam ter o mesmo hash
    /// para que `_CompactLinkedHashSet` e `_CompactLinkedHashMap` encontrem o
    /// segundo record. O valor numérico em si não faz parte da API de Dart.
    pub fn lower_hash_de_forma(&mut self, k: usize) {
        let this = Operand::Val(self.add_param("this".to_string(), Type::Ref));
        let (npos, nomes) = &self.ctx.formas_de_record[k];
        let mut hash = Operand::Constant(Constant::Int(i64::from(ID_BASE_DE_FORMA) + k as i64));
        for i in 0..npos + nomes.len() {
            let campo = self.campo_de_forma(this.clone(), i);
            let codigo = self.chamar_por_nome(campo, super::sdk_fonte::Tipo::Ler, "hashCode", &[]);
            let codigo = self.coagir(codigo, Type::I64);
            let multiplicado = self.emit(
                Instruction::Mul(hash, Operand::Constant(Constant::Int(31))),
                Type::I64,
            );
            hash = self.emit(Instruction::Add(multiplicado, codigo), Type::I64);
        }
        self.terminate(Terminator::Return(Some(hash)));
    }

    /// `r.f(args)` onde `f` é campo de alguma forma de record: nos records
    /// com o campo, lê e chama o valor; nos demais receptores, `padrao`.
    pub fn chamar_campo_de_registro(
        &mut self,
        recv: Operand,
        nome: &str,
        args: &mut dyn FnMut(&mut Self) -> Vec<super::membros::Avaliado>,
        padrao: &mut dyn FnMut(&mut Self) -> Operand,
    ) -> Operand {
        let formas = self.formas_com_campo(nome);
        let recv = self.coagir(recv, Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let juncao = self.new_block();
        let b_padrao = self.new_block();
        let blocos: Vec<(i64, usize, BlockId)> = formas.iter().map(|&(id, i)| (id, i, self.new_block())).collect();
        self.terminate(Terminator::Switch {
            val: cls,
            default: b_padrao,
            cases: blocos.iter().map(|&(id, _, b)| (id, b)).collect(),
        });
        let mut entradas = Vec::new();
        for (_, i, b) in blocos {
            self.set_block(b);
            let f = self.campo_de_forma(recv.clone(), i);
            let av = args(self);
            let r = self.chamar_valor_funcao(f, &av);
            if !self.is_terminated() {
                entradas.push((self.current_block, r));
                self.terminate(Terminator::Branch(juncao));
            }
        }
        self.set_block(b_padrao);
        let n_erros = self.erros.len();
        let mut v = padrao(self);
        if self.erros.len() > n_erros {
            self.erros.truncate(n_erros);
            v = self.lancar_nsm(nome);
        }
        if !self.is_terminated() {
            let v = if matches!(self.operand_type(&v), Type::Void | Type::Ptr) {
                Operand::Constant(Constant::Null)
            } else {
                self.coagir(v, Type::Ref)
            };
            if !self.is_terminated() {
                entradas.push((self.current_block, v));
                self.terminate(Terminator::Branch(juncao));
            }
        }
        self.set_block(juncao);
        if entradas.is_empty() {
            return Operand::Constant(Constant::Null);
        }
        self.emit(
            Instruction::Phi {
                incoming: entradas,
                ty: Type::Ref,
            },
            Type::Ref,
        )
    }
}
