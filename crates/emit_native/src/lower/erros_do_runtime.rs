//! As classes de erro que o runtime representa (ids 1000–1012:
//! `Exception`, `StateError`, `ArgumentError`, `RangeError`…) criadas pelo
//! código das bibliotecas da fonte (P6): `new StateError(m)`,
//! `ArgumentError.value(v, n)`, `Error.throwWithStackTrace(e, s)`.
//!
//! Enquanto essas classes não vêm da fonte (P5d), um objeto delas é o do
//! runtime — é ele que `catch (e) on ArgumentError` testa e que o runtime
//! imprime —, então a construção é a mesma extern que o runtime já usa para
//! os erros que ele lança (`dartforge_argument_error_value`…). Não é caso
//! novo do casamento por nome (que saiu com o espaço unificado): é a representação do
//! runtime das mesmas classes, e sai com a faixa 1000–1012 em P5d.

use super::fn_builder::FnBuilder;
use super::membros::Avaliado;
use crate::hir::*;
use dartforge_diagnostics::Span;

use crate::otimizar::arc::ERROS_COM_VARIANTE_ARC;

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
        self.novo_erro_do_runtime(nome, args)
    }

    /// Um erro novo de um construtor do runtime (`dartforge_*_error_new`…).
    ///
    /// Sob ARC, a pendência é conferida e um par (bits, é-referência) no
    /// começo dos argumentos leva o valor como `Ref`; a preparação ARC troca
    /// pela variante `dartforge_arc_<nome>_owned_v1` (`excecoes.rs`). Sem a
    /// preparação (o regime legado da CLI), fica a extern de sempre.
    pub fn novo_erro_do_runtime(&mut self, nome: &str, mut args: Vec<(Operand, Type)>) -> Operand {
        let base = nome.strip_prefix("dartforge_").unwrap_or(nome);
        if !self.ctx.memoria_arc || !ERROS_COM_VARIANTE_ARC.contains(&base) {
            return self.emit(Instruction::CallRuntime { name: nome.into(), args, ret_ty: Type::Ref }, Type::Ref);
        }
        // Sob ARC o valor do par (bits, é-referência) vai como `Ref` (a
        // marca fica 1) e a pendência é conferida: a preparação troca pela
        // variante Owned (`otimizar::arc::nativos`), que roda Dart.
        if matches!(base, "exception_new" | "assertion_error_new" | "argument_error_value") {
            let valor = self.coagir(args[0].0.clone(), Type::Ref);
            args[0].0 = valor;
            args[1].0 = Operand::Constant(Constant::Int(1));
        }
        self.emit_call_with_check(Instruction::CallRuntime { name: nome.into(), args, ret_ty: Type::Ref }, Type::Ref)
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
                self.emitir_lancamento(e, Some(s));
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
    /// Lança `valor` (com o `StackTrace` `rastro`, se dado) e deixa o bloco
    /// corrente pronto para o desvio ao tratador.
    ///
    /// Sob tracing, `dartforge_exception_throw` com a etiqueta da
    /// representação (1 `int`, 2 `bool`, 3 `Ref`, 4 `double`), ou
    /// `dartforge_throw_with_stack_trace`. Sob ARC, o valor vai como `Ref`
    /// emprestado (um escalar é encaixotado antes) a
    /// `dartforge_arc_lancar_ref_v1`/`…_com_rastro_ref_v1`, que publicam a
    /// raiz própria da exceção, seguido da guarda de pendência que a
    /// preparação ARC reconhece; o caminho sem pendência é inalcançável.
    pub fn emitir_lancamento(&mut self, valor: Operand, rastro: Option<Operand>) {
        if self.ctx.memoria_arc {
            let valor = self.coagir(valor, Type::Ref);
            let (nome, mut args) = match rastro {
                Some(s) => {
                    let s = self.coagir(s, Type::Ref);
                    ("dartforge_arc_lancar_com_rastro_ref_v1", vec![(s, Type::Ref)])
                }
                None => ("dartforge_arc_lancar_ref_v1", Vec::new()),
            };
            args.insert(0, (valor, Type::Ref));
            self.emit(Instruction::CallRuntime { name: nome.into(), args, ret_ty: Type::Void }, Type::Void);
            let pendente = self.emit(
                Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: Vec::new(), ret_ty: Type::I8 },
                Type::I8,
            );
            let lancou = self.emit(Instruction::ICmp(ICmpOp::Ne, pendente, Operand::Constant(Constant::Int(0))), Type::I1);
            let sai = self.new_block();
            let morto = self.new_block();
            self.terminate(Terminator::CondBranch { cond: lancou, then_block: sai, else_block: morto });
            self.set_block(morto);
            self.terminate(Terminator::Unreachable);
            self.set_block(sai);
            return;
        }
        let (tag, bits) = match self.operand_type(&valor) {
            Type::I64 => (1, valor),
            Type::I1 => (2, self.emit(Instruction::ZExt { op: valor, from: Type::I1, to: Type::I8 }, Type::I8)),
            Type::I8 => (2, valor),
            Type::F64 => (4, valor),
            _ => (3, valor),
        };
        let tag = Operand::Constant(Constant::Int(tag));
        let (nome, args) = match rastro {
            Some(s) => {
                let s = self.coagir(s, Type::Ref);
                ("dartforge_throw_with_stack_trace", vec![(bits, Type::I64), (tag, Type::I8), (s, Type::Ref)])
            }
            None => ("dartforge_exception_throw", vec![(bits, Type::I64), (tag, Type::I8)]),
        };
        self.emit(Instruction::CallRuntime { name: nome.into(), args, ret_ty: Type::Void }, Type::Void);
    }

    /// A exceção pendente como `Ref` (a variável do `catch`, a guardada pelo
    /// `finally`).
    pub fn excecao_pendente(&mut self) -> Operand {
        self.emit(Instruction::CallRuntime { name: "dartforge_exception_peek_ref".into(), args: Vec::new(), ret_ty: Type::Ref }, Type::Ref)
    }

    /// O `StackTrace` da exceção pendente. A preparação ARC dá a ele (e à
    /// exceção) um owner próprio, independente do `clear` que segue.
    pub fn rastro_pendente(&mut self) -> Operand {
        self.emit(Instruction::CallRuntime { name: "dartforge_stack_trace_get".into(), args: Vec::new(), ret_ty: Type::Ref }, Type::Ref)
    }

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
