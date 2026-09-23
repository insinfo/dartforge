//! As classes de erro que o runtime representa (ids 1000–1012:
//! `Exception`, `StateError`, `ArgumentError`, `RangeError`…) criadas pelo
//! código das bibliotecas da fonte (P6): `new StateError(m)`,
//! `ArgumentError.value(v, n)`, `Error.throwWithStackTrace(e, s)`.
//!
//! Enquanto essas classes não vêm da fonte (P5d), um objeto delas é o do
//! runtime — é ele que `catch (e) on ArgumentError` testa e que o runtime
//! imprime —, então a construção é a mesma extern que o runtime já usa para
//! os erros que ele lança (`dartforge_argument_error_value`…). Não é caso
//! novo do mecanismo congelado de `sdk_por_nome.rs`: é a representação do
//! runtime das mesmas classes, e sai com a faixa 1000–1012 em P5d.

use super::fn_builder::FnBuilder;
use super::membros::Avaliado;
use crate::hir::*;
use dartforge_diagnostics::Span;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Os argumentos casados com os parâmetros do outline de `fid`, pelo
    /// nome do parâmetro (ausente = null).
    fn args_por_nome(&self, fid: usize, avaliados: &[Avaliado]) -> Vec<(String, Operand)> {
        let Some(dados) = self.ctx.outline.functions.get(fid) else {
            return Vec::new();
        };
        let mut posicionais = avaliados.iter().filter(|(n, _)| n.is_none()).map(|(_, v)| v.clone());
        dados
            .parameters
            .iter()
            .map(|p| {
                let nome = p.name.map(|s| self.ctx.symbol_name(s).to_string()).unwrap_or_default();
                let v = if p.kind == dartforge_frontend::ast::ParameterKind::Named {
                    avaliados.iter().find(|(n, _)| *n == p.name).map(|(_, v)| v.clone())
                } else {
                    posicionais.next()
                };
                (nome, v.unwrap_or(Operand::Constant(Constant::Null)))
            })
            .collect()
    }

    fn arg(args: &[(String, Operand)], i: usize) -> Operand {
        args.get(i).map_or(Operand::Constant(Constant::Null), |(_, v)| v.clone())
    }

    fn runtime_ref(&mut self, nome: &str, args: Vec<(Operand, Type)>) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: nome.to_string(),
                args,
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// `C(...)`/`C.nome(...)` de uma classe de erro do runtime; `None` se não é.
    pub fn construtor_de_erro_do_runtime(&mut self, fid: usize, avaliados: &[Avaliado]) -> Option<Operand> {
        let f = &self.ctx.program.functions[fid];
        let classe = f.class?;
        if self.ctx.program.library(self.ctx.program.classes[classe.0 as usize].library).uri != "dart:core" {
            return None;
        }
        let nome_classe = self.ctx.symbol_name(self.ctx.program.classes[classe.0 as usize].name).to_string();
        let ctor = self.ctx.symbol_name(f.name).to_string();
        let a = self.args_por_nome(fid, avaliados);
        let r = |s: &mut Self, i: usize| {
            let v = Self::arg(&a, i);
            s.coagir(v, Type::Ref)
        };
        Some(match (nome_classe.as_str(), ctor.as_str()) {
            ("Exception", "") => {
                let m = r(self, 0);
                self.runtime_ref("dartforge_exception_new", vec![(m, Type::I64), (Operand::Constant(Constant::Int(1)), Type::I8)])
            }
            ("StateError", "") => {
                let m = r(self, 0);
                self.runtime_ref("dartforge_state_error_new", vec![(m, Type::Ref)])
            }
            ("UnsupportedError", "") => {
                let m = r(self, 0);
                self.runtime_ref("dartforge_unsupported_error_new", vec![(m, Type::Ref)])
            }
            ("UnimplementedError", "") => {
                let m = r(self, 0);
                self.runtime_ref("dartforge_unimplemented_error_new", vec![(m, Type::Ref)])
            }
            ("ArgumentError", "") => {
                let (m, n) = (r(self, 0), r(self, 1));
                self.runtime_ref("dartforge_argument_error_new", vec![(m, Type::Ref), (n, Type::Ref)])
            }
            ("ArgumentError", "value") => {
                let (v, n, m) = (r(self, 0), r(self, 1), r(self, 2));
                self.runtime_ref(
                    "dartforge_argument_error_value",
                    vec![(v, Type::I64), (Operand::Constant(Constant::Int(1)), Type::I8), (n, Type::Ref), (m, Type::Ref)],
                )
            }
            ("ArgumentError", "notNull") => {
                let n = r(self, 0);
                self.runtime_ref("dartforge_argument_error_not_null", vec![(n, Type::Ref)])
            }
            ("TypeError", "") => self.runtime_ref("dartforge_type_error_new", Vec::new()),
            // A lista é do runtime (P5d: `_List`/`_GrowableList` da fonte);
            // `List.filled` do `Future.wait`.
            ("List", "filled") => {
                let n = Self::arg(&a, 0);
                let n = self.coagir(n, Type::I64);
                let v = Self::arg(&a, 1);
                let tag = self.operand_tag(&v);
                let (bits, _) = self.para_bits(v);
                let l = self.emit_call_with_check(
                    Instruction::CallRuntime {
                        name: "dartforge_list_filled".to_string(),
                        args: vec![(n, Type::I64), (bits, Type::I64), (Operand::Constant(Constant::Int(i64::from(tag))), Type::I8)],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                );
                if let Some(t) = self.tipo_da_criacao.take() {
                    self.definir_rti_se_generico(l.clone(), t);
                }
                l
            }
            _ => return None,
        })
    }

    /// Métodos estáticos das classes de erro do runtime que o código da
    /// fonte chama: `Error.throwWithStackTrace`, `ArgumentError.checkNotNull`,
    /// `RangeError.checkNotNegative`. `None` se não é um deles.
    pub fn estatico_de_erro_do_runtime(&mut self, fid: usize, avaliados: &[Avaliado], span: Span) -> Option<Operand> {
        let f = &self.ctx.program.functions[fid];
        let classe = f.class?;
        if self.ctx.program.library(self.ctx.program.classes[classe.0 as usize].library).uri != "dart:core" {
            return None;
        }
        let nome_classe = self.ctx.symbol_name(self.ctx.program.classes[classe.0 as usize].name).to_string();
        let metodo = self.ctx.symbol_name(f.name).to_string();
        let a = self.args_por_nome(fid, avaliados);
        match (nome_classe.as_str(), metodo.as_str()) {
            ("Error", "throwWithStackTrace") => {
                let e = Self::arg(&a, 0);
                let e = self.coagir(e, Type::Ref);
                let s = Self::arg(&a, 1);
                let s = self.coagir(s, Type::Ref);
                self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_throw_with_stack_trace".to_string(),
                        args: vec![(e, Type::I64), (Operand::Constant(Constant::Int(3)), Type::I8), (s, Type::Ref)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                );
                self.desviar_para_tratador_de_excecao();
                Some(Operand::Constant(Constant::Null))
            }
            ("ArgumentError", "checkNotNull") => {
                let v = Self::arg(&a, 0);
                let v = self.coagir(v, Type::Ref);
                let n = Self::arg(&a, 1);
                let n = self.coagir(n, Type::Ref);
                let nulo = self.emit(Instruction::ICmp(ICmpOp::Eq, v.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
                let b_erro = self.new_block();
                let b_ok = self.new_block();
                self.terminate(Terminator::CondBranch { cond: nulo, then_block: b_erro, else_block: b_ok });
                self.set_block(b_erro);
                let e = self.runtime_ref("dartforge_argument_error_not_null", vec![(n, Type::Ref)]);
                self.emit_throw_op(e);
                self.set_block(b_ok);
                Some(v)
            }
            ("RangeError", "checkNotNegative") => {
                let v = Self::arg(&a, 0);
                let v = self.coagir(v, Type::I64);
                let n = Self::arg(&a, 1);
                let n = self.coagir(n, Type::Ref);
                let m = Self::arg(&a, 2);
                let m = self.coagir(m, Type::Ref);
                let neg = self.emit(Instruction::ICmp(ICmpOp::Slt, v.clone(), Operand::Constant(Constant::Int(0))), Type::I1);
                let b_erro = self.new_block();
                let b_ok = self.new_block();
                self.terminate(Terminator::CondBranch { cond: neg, then_block: b_erro, else_block: b_ok });
                self.set_block(b_erro);
                let e = self.runtime_ref(
                    "dartforge_range_error_range",
                    vec![
                        (v.clone(), Type::I64),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                        (Operand::Constant(Constant::Int(i64::MAX)), Type::I64),
                        (n, Type::Ref),
                        (m, Type::Ref),
                    ],
                );
                self.emit_throw_op(e);
                self.set_block(b_ok);
                Some(v)
            }
            _ => {
                let _ = span;
                None
            }
        }
    }

    /// Depois de uma extern que deixou exceção pendente: o desvio para o
    /// tratador corrente (o `finally`, ou a saída da função).
    pub fn desviar_para_tratador_de_excecao(&mut self) {
        let curr_b = self.current_block;
        if let Some(&exc_target) = self.exception_targets.last() {
            self.terminate(Terminator::Branch(exc_target));
        } else if !self.finally_scopes.is_empty() {
            let default_ret = self.default_return_operand();
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((curr_b, 2, default_ret));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::Branch(fin_entry));
        } else {
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
        }
        let dead = self.new_block();
        self.set_block(dead);
    }
}
