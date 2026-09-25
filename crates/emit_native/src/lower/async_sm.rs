//! `async`/`await` (P6, docs/NATIVO-PLANO.md §7.7): o corpo como máquina de
//! estados, no molde do dart2js (`pkg/compiler/lib/src/js/rewrite_async.dart`)
//! e dos apoios de `sdk_nativo/async/async_patch.dart`.
//!
//! Uma função `async f(p…) { … }` vira duas:
//!
//! * **o stub `f`** (o símbolo de sempre): cria o **quadro** — um objeto do
//!   heap —, o `Completer` (`_makeAsyncAwaitCompleter`), a closure do corpo
//!   (registrada na zona por `_envolverCorpo`) e começa o corpo de forma
//!   síncrona (`_asyncStartSync`), devolvendo `completer.future`;
//! * **o corpo `f$async(env, código, resultado)`**, uma closure com o
//!   quadro no ambiente. O bloco de entrada lê o quadro e salta pelo
//!   `estado` guardado nele: 0 é o começo; cada `await` é um estado.
//!
//! **`await e`.** Grava o estado `k` no quadro, chama `_asyncAwait(e,
//! corpo)` e **retorna**; o bloco de retomada `R_k` só é alcançado pelo
//! `switch` da entrada, quando o `Future` completa e chama o corpo de novo.
//! Com `código == 1` (`_ERRO`) o `resultado` é um `_ErroAssincrono`: o erro é
//! lançado ali, no ponto do `await`, com o rastro dele, e segue o caminho de
//! exceção pendente de sempre — os `try`/`catch`/`finally` que envolvem o
//! `await` são os do lowering (o mesmo `exception_target` de antes da
//! suspensão). Exceção que chega ao topo do corpo vai a `_asyncRethrow`
//! (`completer.completeError`); `return v` vai a `_asyncReturn`.
//!
//! **O que atravessa um `await` mora no quadro** (o contrato G: o quadro é
//! alcançado pelas arestas do heap — ambiente da closure do corpo, o
//! `_FutureListener` que espera —, não pela pilha-sombra, que é desmontada a
//! cada suspensão). Duas passadas sobre a HIR do corpo pronto, que não
//! dependem de como cada construto foi baixado:
//!
//! 1. todo `alloca` (os locais, R6) vira uma posição do quadro, e o
//!    `load`/`store` dele, leitura/gravação da posição;
//! 2. todo valor SSA **vivo na entrada de uma retomada** (vivacidade com a
//!    aresta virtual suspensão → retomada) é gravado numa posição do quadro
//!    logo depois de definido e relido antes de cada uso — o que o LLVM
//!    faz no *coroutine frame* (`CoroSplit`), sem precisar reconstruir o
//!    SSA. Os valores do bloco de entrada (quadro, `this`, ambiente de fora)
//!    são recalculados a cada entrada e ficam de fora.
//!
//! Layout do quadro: `[estado, completer, corpo, this, ambiente de fora,
//! tupla de tipos, parâmetros…, locais…, temporários…]`.

use super::fn_builder::FnBuilder;
use super::locais::{Local, Modo};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, FunctionBody};
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};

/// Id de classe do quadro de uma função `async` (abaixo das formas de
/// record, `context::ID_BASE_DE_FORMA`).
pub const ID_QUADRO_ASYNC: u32 = 0x3FFF_FF00;

const Q_ESTADO: usize = 0;
const Q_COMPLETER: usize = 1;
const Q_CORPO: usize = 2;
const Q_THIS: usize = 3;
const Q_AMBIENTE: usize = 4;
const Q_TUPLA: usize = 5;
const Q_PARAMS: usize = 6;

/// O estado do lowering do corpo de uma função `async`.
#[derive(Debug, Clone)]
pub struct EstadoAsync {
    pub quadro: Operand,
    /// A closure do corpo registrada na zona (a que `_asyncAwait` recebe).
    pub corpo: Operand,
    pub completer: Operand,
    pub codigo: Operand,
    pub resultado: Operand,
    /// `(k, R_k)`: o bloco de retomada de cada estado.
    pub retomadas: Vec<(i64, BlockId)>,
    /// `(A, R_k)`: o bloco que suspende e a retomada dele.
    pub suspensoes: Vec<(BlockId, BlockId)>,
    /// O próximo `Return` é cru (suspensão ou o fim do tratador do topo):
    /// não passa por `_asyncReturn`.
    pub retorno_cru: bool,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Função de topo `nome` do `dart:async` (os apoios do `async_patch`).
    pub fn apoio_async(&self, nome: &str) -> Option<usize> {
        let lib = self.ctx.program.libraries.iter().position(|l| l.uri == "dart:async")?;
        let sym = self.ctx.interner.lookup(nome)?;
        let b = self.ctx.program.lookup(dartforge_elements::model::LibraryId(lib as u32), sym)?;
        match b.getter? {
            dartforge_elements::model::Element::Function(f) => Some(f.0 as usize),
            _ => None,
        }
    }

    /// Chama o apoio `nome` do `dart:async` com os argumentos já avaliados.
    fn chamar_apoio(&mut self, nome: &str, args: Vec<Operand>, span: Span) -> Operand {
        let Some(fid) = self.apoio_async(nome) else {
            return self.nao_suportado(&format!("apoio `{nome}` do dart:async ausente"), span);
        };
        let avaliados: Vec<super::membros::Avaliado> = args.into_iter().map(|a| (None, a)).collect();
        let args = self.casar_args(fid, &avaliados);
        self.chamar_direto(fid, None, args)
    }

    /// `dartforge_object_get` na representação `ty`.
    fn ler_posicao(&mut self, obj: Operand, idx: usize, ty: Type) -> Operand {
        let ret = if ty == Type::Ref { Type::Ref } else { Type::I64 };
        let bits = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (Operand::Constant(Constant::Int(idx as i64)), Type::I64)],
                ret_ty: ret,
            },
            ret,
        );
        self.bits_para(bits, ty)
    }

    /// `dartforge_object_set` com a tag da representação do valor (E1).
    fn gravar_posicao(&mut self, obj: Operand, idx: usize, val: Operand) {
        let (bits, is_ref) = self.para_bits(val);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_set".to_string(),
                args: vec![
                    (obj, Type::Ref),
                    (Operand::Constant(Constant::Int(idx as i64)), Type::I64),
                    (bits, Type::I64),
                    (Operand::Constant(Constant::Int(i64::from(is_ref))), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }

    /// Os locais visíveis no stub, em ordem determinística: os ligados ao
    /// ambiente da closure (pela posição) e os demais — os parâmetros — pelo
    /// offset da declaração.
    fn locais_do_stub(&self) -> (Vec<(SymbolId, Local)>, Vec<(SymbolId, Local)>) {
        let mut ambiente = Vec::new();
        let mut outros = Vec::new();
        let mut vistos = HashSet::new();
        for escopo in self.escopos.iter().rev() {
            let mut nomes: Vec<(&SymbolId, &Local)> = escopo.iter().collect();
            nomes.sort_by_key(|(s, l)| (l.offset, self.ctx.symbol_name(**s).to_string()));
            for (s, l) in nomes {
                if !vistos.insert(*s) {
                    continue;
                }
                match l.modo {
                    Modo::Ambiente { .. } => ambiente.push((*s, l.clone())),
                    _ => outros.push((*s, l.clone())),
                }
            }
        }
        ambiente.sort_by_key(|(_, l)| match l.modo {
            Modo::Ambiente { indice, .. } => indice,
            _ => 0,
        });
        outros.sort_by_key(|(s, l)| (l.offset, self.ctx.symbol_name(*s).to_string()));
        (ambiente, outros)
    }

    /// O corpo de uma função `async`: este builder é o stub (parâmetros e
    /// capturas já declarados); o corpo vira `<símbolo>$async` e a entrada
    /// uniforme dele. Termina o stub.
    pub fn lower_corpo_async(
        &mut self,
        ast: &ast::Ast,
        params: &[ast::Parameter],
        corpo: &FunctionBody,
        span: Span,
        retorno: Option<dartforge_types::table::TypeId>,
    ) {
        let (capturas, parametros) = self.locais_do_stub();
        let simbolo_corpo = format!("{}$async", self.func.symbol);
        let legivel = self.func.name.clone();

        // --- corpo ------------------------------------------------------
        let mut b = FnBuilder::new(self.ctx, self.unit_id, simbolo_corpo.clone(), legivel.clone(), Type::Ref);
        let env = Operand::Val(b.add_param("env".to_string(), Type::Ref));
        let codigo = Operand::Val(b.add_param("codigo".to_string(), Type::Ref));
        let resultado = Operand::Val(b.add_param("resultado".to_string(), Type::Ref));
        let quadro = b.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
        let estado = b.ler_posicao(quadro.clone(), Q_ESTADO, Type::I64);
        let completer = b.ler_posicao(quadro.clone(), Q_COMPLETER, Type::Ref);
        let fechamento = b.ler_posicao(quadro.clone(), Q_CORPO, Type::Ref);
        if self.this_param.is_some() {
            let t = b.ler_posicao(quadro.clone(), Q_THIS, Type::Ref);
            b.this_param = Some(t);
            b.enclosing_class = self.enclosing_class;
        }
        if !capturas.is_empty() {
            let de_fora = b.ler_posicao(quadro.clone(), Q_AMBIENTE, Type::Ref);
            for (sym, l) in &capturas {
                if let Modo::Ambiente { indice, celula, .. } = l.modo {
                    b.ligar_ambiente(*sym, de_fora.clone(), indice, celula, l.ty, l.late.as_ref());
                }
            }
        }
        // RTI: as variáveis de tipo do stub (a tupla mora no quadro).
        b.params_de_tipo_da_funcao = self.params_de_tipo_da_funcao.clone();
        b.classe_por_tupla = self.classe_por_tupla;
        if self.tupla_de_tipos.is_some() {
            let t = b.ler_posicao(quadro.clone(), Q_TUPLA, Type::I64);
            b.tupla_de_tipos = Some(t);
        }
        b.async_estado = Some(Box::new(EstadoAsync {
            quadro: quadro.clone(),
            corpo: fechamento,
            completer: completer.clone(),
            codigo,
            resultado,
            retomadas: Vec::new(),
            suspensoes: Vec::new(),
            retorno_cru: false,
        }));
        let entrada = b.current_block;
        let inicio = b.new_block();
        b.set_block(inicio);
        b.preparar_capturas(
            ast,
            super::captura::Raiz {
                parametros: params,
                corpo: Some(corpo),
                inicializadores: &[],
            },
        );
        b.abrir_escopo();
        for (k, (sym, l)) in parametros.iter().enumerate() {
            let v = b.ler_posicao(quadro.clone(), Q_PARAMS + k, l.ty);
            match l.offset {
                Some(off) => b.declarar_variavel(*sym, off, l.ty, v),
                None => b.declarar_local_com_valor(*sym, l.ty, v),
            }
        }
        // O tratador do topo: exceção que ninguém no corpo pegou completa o
        // `Future` com erro.
        let topo = b.new_block();
        b.exception_targets.push(topo);
        match corpo {
            FunctionBody::Block(s) => b.lower_stmt(ast, *s),
            FunctionBody::Expression(e) => {
                let r = b.lower_expr(ast, *e);
                b.route_return(Some(r));
            }
            _ => {}
        }
        if !b.is_terminated() {
            b.terminate(Terminator::Return(None));
        }
        b.exception_targets.pop();
        // O tratador do topo não tem tratador: nada nele volta a
        // `_asyncReturn` (um retorno de exceção aqui é cru).
        if let Some(e) = b.async_estado.as_mut() {
            e.retorno_cru = true;
        }
        b.set_block(topo);
        let erro = b.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_peek_ref".to_string(),
                args: Vec::new(),
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        let rastro = b.emit(
            Instruction::CallRuntime {
                name: "dartforge_stack_trace_get".to_string(),
                args: Vec::new(),
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        b.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        b.chamar_apoio("_asyncRethrow", vec![erro, rastro, completer], span);
        b.terminar_cru(Operand::Constant(Constant::Null));

        // O salto pelo estado, no fim do bloco de entrada.
        let est = b.async_estado.take().expect("estado async");
        let idx0 = b.func.blocks.iter().position(|x| x.id == entrada).expect("entrada");
        b.func.blocks[idx0].terminator = Terminator::Switch {
            val: estado,
            default: inicio,
            cases: est.retomadas.clone(),
        };
        b.terminated_blocks.insert(entrada);

        // As duas passadas: locais e temporários que atravessam um `await`
        // vão para o quadro.
        let mut prox = b.next_value;
        let base_locais = Q_PARAMS + parametros.len();
        let n_locais = converter_allocas(&mut b.func, &quadro, base_locais, &mut prox);
        let n_temp = guardar_vivos(&mut b.func, &quadro, base_locais + n_locais, &est.suspensoes, entrada, &mut prox);
        b.next_value = prox;
        let tamanho = base_locais + n_locais + n_temp;
        self.absorver(b);

        // --- entrada uniforme do corpo: (código, resultado) --------------
        let infos = vec![
            super::closures::ParamEntrada {
                nome: Some("codigo".to_string()),
                kind: ast::ParameterKind::Required,
                required: true,
                padrao: super::closures::Padrao::Nenhum,
            },
            super::closures::ParamEntrada {
                nome: Some("resultado".to_string()),
                kind: ast::ParameterKind::Required,
                required: true,
                padrao: super::closures::Padrao::Nenhum,
            },
        ];
        let simbolo_ent = format!("{simbolo_corpo}$ent");
        let mut e = FnBuilder::new(self.ctx, self.unit_id, simbolo_ent.clone(), legivel, Type::Ref);
        let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(e.add_param("args".to_string(), Type::Ptr));
        let desc = Operand::Val(e.add_param("desc".to_string(), Type::Ptr));
        let env_e = e.emit(
            Instruction::CallRuntime {
                name: "dartforge_closure_env".to_string(),
                args: vec![(clo, Type::Ref)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        if let Some(vals) = e.desempacotar(&infos, args, desc) {
            let mut todos = vec![env_e];
            todos.extend(vals);
            let r = e.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: simbolo_corpo,
                    args: todos,
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            e.terminate(Terminator::Return(Some(r)));
        }
        self.absorver(e);

        // --- o stub -----------------------------------------------------
        let mut campos: Vec<Operand> = vec![Operand::Constant(Constant::Int(0)); tamanho];
        campos[Q_COMPLETER] = Operand::Constant(Constant::Null);
        campos[Q_CORPO] = Operand::Constant(Constant::Null);
        campos[Q_THIS] = self.this_param.clone().map_or(Operand::Constant(Constant::Null), |t| self.coagir(t, Type::Ref));
        campos[Q_AMBIENTE] = capturas
            .iter()
            .find_map(|(_, l)| match &l.modo {
                Modo::Ambiente { env, .. } => Some(env.clone()),
                _ => None,
            })
            .unwrap_or(Operand::Constant(Constant::Null));
        if let Some(t) = self.tupla_de_tipos.clone() {
            campos[Q_TUPLA] = t;
        }
        for (k, (_, l)) in parametros.iter().enumerate() {
            let v = self.ler_local(l);
            campos[Q_PARAMS + k] = v;
        }
        let quadro_s = self.emit(
            Instruction::AllocObject {
                class_id: ID_QUADRO_ASYNC,
                fields: campos,
            },
            Type::Ref,
        );
        // O `T` do `Completer<T>`: o tipo do valor do `Future` declarado
        // (especificação §9 "future value type": `Future<T>`/`FutureOr<T>` dão
        // `T`; o resto, `dynamic`).
        let valor = retorno.and_then(|t| match self.ctx.table.get(t) {
            dartforge_types::table::Type::Interface { class, args, .. }
                if Some(*class) == self.ctx.core.future_class =>
            {
                args.first().copied()
            }
            dartforge_types::table::Type::FutureOr { arg, .. } => Some(*arg),
            _ => None,
        });
        let salvo = self.tupla_armada.take();
        if let Some(v) = valor {
            let t = self.tupla_de_tipos_rti(&[v]);
            self.tupla_armada = Some(t);
        }
        let completer_s = self.chamar_apoio("_makeAsyncAwaitCompleter", Vec::new(), span);
        self.tupla_armada = salvo;
        let completer_s = self.coagir(completer_s, Type::Ref);
        self.gravar_posicao(quadro_s.clone(), Q_COMPLETER, completer_s.clone());
        let env_s = self.emit(Instruction::AllocEnv { values: vec![quadro_s.clone()] }, Type::Ref);
        let clo_s = self.emit(
            Instruction::AllocClosure {
                code_symbol: simbolo_ent,
                env: env_s,
            },
            Type::Ref,
        );
        let corpo_s = self.chamar_apoio("_envolverCorpo", vec![clo_s], span);
        let corpo_s = self.coagir(corpo_s, Type::Ref);
        self.gravar_posicao(quadro_s, Q_CORPO, corpo_s.clone());
        let r = self.chamar_apoio("_asyncStartSync", vec![corpo_s, completer_s], span);
        self.terminate(Terminator::Return(Some(r)));
    }

    /// `Return` que não passa por `_asyncReturn`.
    fn terminar_cru(&mut self, v: Operand) {
        let antes = self.async_estado.as_ref().is_some_and(|e| e.retorno_cru);
        if let Some(e) = self.async_estado.as_mut() {
            e.retorno_cru = true;
        }
        self.terminate(Terminator::Return(Some(v)));
        if let Some(e) = self.async_estado.as_mut() {
            e.retorno_cru = antes;
        }
    }

    /// O `return` de um corpo `async` (chamado por `terminate`): `_asyncReturn(v,
    /// completer)` e o retorno cru. Uma exceção de `_asyncReturn` (o `as T`
    /// do valor) vai ao tratador do topo.
    pub fn retorno_async(&mut self, v: Option<Operand>) {
        let est = self.async_estado.as_ref().expect("corpo async");
        let completer = est.completer.clone();
        let v = match v {
            Some(v) if !matches!(self.operand_type(&v), Type::Void) => self.coagir(v, Type::Ref),
            _ => Operand::Constant(Constant::Null),
        };
        self.chamar_apoio("_asyncReturn", vec![v, completer], Span { start: 0, end: 0 });
        self.terminar_cru(Operand::Constant(Constant::Null));
    }

    /// `await e` (especificação §17.34): suspende até o `Future` completar.
    pub fn lower_await(&mut self, ast: &ast::Ast, e: ast::ExprId, span: Span) -> Operand {
        if self.async_estado.is_none() {
            return self.nao_suportado("await fora de função async", span);
        }
        let v = self.lower_expr(ast, e);
        let v = if matches!(self.operand_type(&v), Type::Void) {
            Operand::Constant(Constant::Null)
        } else {
            self.coagir(v, Type::Ref)
        };
        if self.is_terminated() {
            return Operand::Constant(Constant::Null);
        }
        let est = self.async_estado.as_ref().expect("corpo async");
        let (quadro, corpo) = (est.quadro.clone(), est.corpo.clone());
        let k = est.retomadas.len() as i64 + 1;
        self.gravar_posicao(quadro, Q_ESTADO, Operand::Constant(Constant::Int(k)));
        self.chamar_apoio("_asyncAwait", vec![v, corpo], span);
        let suspende = self.current_block;
        self.terminar_cru(Operand::Constant(Constant::Null));
        let retomada = self.new_block();
        let est = self.async_estado.as_mut().expect("corpo async");
        est.retomadas.push((k, retomada));
        est.suspensoes.push((suspende, retomada));
        let (codigo, resultado) = (est.codigo.clone(), est.resultado.clone());
        self.set_block(retomada);
        // `código` chega como `int` numa posição `Ref` (Smi).
        let c = self.coagir(codigo, Type::I64);
        let e_erro = self.emit(
            Instruction::ICmp(ICmpOp::Eq, c, Operand::Constant(Constant::Int(1))),
            Type::I1,
        );
        let b_erro = self.new_block();
        let b_ok = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: e_erro,
            then_block: b_erro,
            else_block: b_ok,
        });
        self.set_block(b_erro);
        self.lancar_erro_assincrono(resultado.clone(), span);
        self.set_block(b_ok);
        resultado
    }

    /// Lança, no ponto do `await`, o erro de um `_ErroAssincrono` com o
    /// rastro dele.
    fn lancar_erro_assincrono(&mut self, valor: Operand, span: Span) {
        let campos = self.campos_do_erro_assincrono();
        let Some((ve, vr)) = campos else {
            self.nao_suportado("classe `_ErroAssincrono` do async_patch ausente", span);
            return;
        };
        let erro = self.ler_campo(valor.clone(), ve, span);
        let rastro = self.ler_campo(valor, vr, span);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_throw_with_stack_trace".to_string(),
                args: vec![
                    (erro, Type::I64),
                    (Operand::Constant(Constant::Int(3)), Type::I8),
                    (rastro, Type::Ref),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.desviar_para_tratador();
    }

    /// Os campos `erro` e `rastro` de `_ErroAssincrono`.
    fn campos_do_erro_assincrono(
        &self,
    ) -> Option<(dartforge_elements::model::VariableId, dartforge_elements::model::VariableId)> {
        let lib = self.ctx.program.libraries.iter().position(|l| l.uri == "dart:async")?;
        let sym = self.ctx.interner.lookup("_ErroAssincrono")?;
        let b = self.ctx.program.lookup(dartforge_elements::model::LibraryId(lib as u32), sym)?;
        let dartforge_elements::model::Element::Class(c) = b.getter? else {
            return None;
        };
        let classe = &self.ctx.program.classes[c.0 as usize];
        let campo = |n: &str| {
            classe
                .fields
                .iter()
                .copied()
                .find(|v| self.ctx.symbol_name(self.ctx.program.variables[v.0 as usize].name) == n)
        };
        Some((campo("erro")?, campo("rastro")?))
    }

    /// Depois de deixar uma exceção pendente: o desvio de `emit_throw_op`
    /// (tratador corrente, `finally` ou saída).
    fn desviar_para_tratador(&mut self) {
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

// ---------------------------------------------------------------------------
// As passadas sobre a HIR do corpo.

/// O tipo que o emissor dá a cada valor (o de `LlvmEmitter`).
fn tipos_da_funcao(func: &Function) -> HashMap<ValueId, Type> {
    let mut t = HashMap::new();
    for (v, _, ty) in &func.params {
        t.insert(*v, *ty);
    }
    for b in &func.blocks {
        for (v, inst, ty) in &b.instructions {
            t.insert(*v, crate::llvm::LlvmEmitter::tipo_do_resultado(inst, *ty));
        }
    }
    t
}

fn novo_valor(prox: &mut u32) -> ValueId {
    let v = ValueId(*prox);
    *prox += 1;
    v
}

/// Instruções que leem a posição `slot` do quadro para o valor `destino`
/// na representação `ty`.
fn ler_do_quadro(quadro: &Operand, slot: usize, ty: Type, destino: ValueId, prox: &mut u32) -> Vec<(ValueId, Instruction, Type)> {
    let get = |ret: Type| Instruction::CallRuntime {
        name: "dartforge_object_get".to_string(),
        args: vec![(quadro.clone(), Type::Ref), (Operand::Constant(Constant::Int(slot as i64)), Type::I64)],
        ret_ty: ret,
    };
    match ty {
        Type::Ref | Type::I64 => vec![(destino, get(ty), ty)],
        Type::F64 => {
            let t = novo_valor(prox);
            vec![
                (t, get(Type::I64), Type::I64),
                (destino, Instruction::Bitcast { op: Operand::Val(t), to: Type::F64 }, Type::F64),
            ]
        }
        Type::I1 => {
            let t = novo_valor(prox);
            vec![
                (t, get(Type::I64), Type::I64),
                (destino, Instruction::ICmp(ICmpOp::Ne, Operand::Val(t), Operand::Constant(Constant::Int(0))), Type::I1),
            ]
        }
        Type::I8 => {
            let t = novo_valor(prox);
            vec![
                (t, get(Type::I64), Type::I64),
                (destino, Instruction::Trunc { op: Operand::Val(t), from: Type::I64, to: Type::I8 }, Type::I8),
            ]
        }
        Type::Void | Type::Ptr => Vec::new(),
    }
}

/// Instruções que gravam `valor` (na representação `ty`) na posição `slot`.
fn gravar_no_quadro(quadro: &Operand, slot: usize, valor: Operand, ty: Type, prox: &mut u32) -> Vec<(ValueId, Instruction, Type)> {
    let mut saida = Vec::new();
    let (bits, is_ref) = match ty {
        Type::Ref => (valor, 1),
        Type::I64 => (valor, 0),
        Type::F64 => {
            let t = novo_valor(prox);
            saida.push((t, Instruction::Bitcast { op: valor, to: Type::I64 }, Type::I64));
            (Operand::Val(t), 0)
        }
        Type::I1 | Type::I8 => {
            let t = novo_valor(prox);
            saida.push((t, Instruction::ZExt { op: valor, from: ty, to: Type::I64 }, Type::I64));
            (Operand::Val(t), 0)
        }
        Type::Void | Type::Ptr => return saida,
    };
    saida.push((
        novo_valor(prox),
        Instruction::CallRuntime {
            name: "dartforge_object_set".to_string(),
            args: vec![
                (quadro.clone(), Type::Ref),
                (Operand::Constant(Constant::Int(slot as i64)), Type::I64),
                (bits, Type::I64),
                (Operand::Constant(Constant::Int(is_ref)), Type::I8),
            ],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    saida
}

/// Passada 1: cada `alloca` vira uma posição do quadro a partir de `base`.
/// Devolve quantas posições usou.
fn converter_allocas(func: &mut Function, quadro: &Operand, base: usize, prox: &mut u32) -> usize {
    let mut posicao: HashMap<ValueId, (usize, Type)> = HashMap::new();
    for b in &func.blocks {
        for (v, inst, _) in &b.instructions {
            if let Instruction::Alloca(t) = inst {
                let s = base + posicao.len();
                posicao.insert(*v, (s, *t));
            }
        }
    }
    if posicao.is_empty() {
        return 0;
    }
    let tipos = tipos_da_funcao(func);
    let tipo_de = |op: &Operand| match op {
        Operand::Val(v) => tipos.get(v).copied().unwrap_or(Type::I64),
        Operand::Constant(Constant::Int(_)) => Type::I64,
        Operand::Constant(Constant::Double(_)) => Type::F64,
        Operand::Constant(Constant::Bool(_)) => Type::I1,
        Operand::Constant(_) => Type::Ref,
    };
    for b in &mut func.blocks {
        let antigas = std::mem::take(&mut b.instructions);
        for (v, inst, ty) in antigas {
            match &inst {
                Instruction::Alloca(_) if posicao.contains_key(&v) => {}
                Instruction::Load { ptr: Operand::Val(p), ty: t } if posicao.contains_key(p) => {
                    let (s, _) = posicao[p];
                    b.instructions.extend(ler_do_quadro(quadro, s, *t, v, prox));
                }
                Instruction::Store { ptr: Operand::Val(p), val } if posicao.contains_key(p) => {
                    let (s, t) = posicao[p];
                    // O valor chega na representação do local (R6, já
                    // coagido pelo lowering); só a largura pode diferir.
                    let ty_gravar = if t == Type::Ref { Type::Ref } else { tipo_de(val) };
                    b.instructions.extend(gravar_no_quadro(quadro, s, val.clone(), ty_gravar, prox));
                }
                _ => b.instructions.push((v, inst, ty)),
            }
        }
    }
    posicao.len()
}

/// Os valores lidos por uma instrução.
fn usos_de(inst: &Instruction) -> Vec<ValueId> {
    let mut u = Vec::new();
    let mut op = |o: &Operand| {
        if let Operand::Val(v) = o {
            u.push(*v);
        }
    };
    match inst {
        Instruction::Const(_) | Instruction::Alloca(_) | Instruction::LoadGlobal { .. } | Instruction::TearOff { .. } | Instruction::ConstArray(_) => {}
        Instruction::Add(a, b)
        | Instruction::Sub(a, b)
        | Instruction::Mul(a, b)
        | Instruction::SDiv(a, b)
        | Instruction::SRem(a, b)
        | Instruction::Shl(a, b)
        | Instruction::AShr(a, b)
        | Instruction::LShr(a, b)
        | Instruction::And(a, b)
        | Instruction::Or(a, b)
        | Instruction::Xor(a, b)
        | Instruction::FAdd(a, b)
        | Instruction::FSub(a, b)
        | Instruction::FMul(a, b)
        | Instruction::FDiv(a, b)
        | Instruction::ICmp(_, a, b)
        | Instruction::FCmp(_, a, b) => {
            op(a);
            op(b);
        }
        Instruction::Neg(a)
        | Instruction::Not(a)
        | Instruction::FNeg(a)
        | Instruction::LNot(a)
        | Instruction::IntToDouble(a)
        | Instruction::DoubleToInt(a)
        | Instruction::CheckNotNull(a) => op(a),
        Instruction::ZExt { op: a, .. }
        | Instruction::Trunc { op: a, .. }
        | Instruction::Bitcast { op: a, .. }
        | Instruction::Box { op: a, .. }
        | Instruction::Unbox { op: a, .. } => op(a),
        Instruction::AllocObject { fields, .. } => fields.iter().for_each(&mut op),
        Instruction::AllocList { elements } | Instruction::AllocSet { elements } | Instruction::AllocRecord { elements } => {
            elements.iter().for_each(|(a, _)| op(a))
        }
        Instruction::AllocMap { entries } => entries.iter().for_each(|((k, _), (v, _))| {
            op(k);
            op(v);
        }),
        Instruction::AllocCell { value } => op(value),
        Instruction::AllocEnv { values } => values.iter().for_each(&mut op),
        Instruction::AllocClosure { env, .. } => op(env),
        Instruction::Load { ptr, .. } => op(ptr),
        Instruction::Store { ptr, val } => {
            op(ptr);
            op(val);
        }
        Instruction::GetField { object, .. } => op(object),
        Instruction::SetField { object, value, .. } => {
            op(object);
            op(value);
        }
        Instruction::GetListElement { list, index } => {
            op(list);
            op(index);
        }
        Instruction::SetListElement { list, index, value } => {
            op(list);
            op(index);
            op(value);
        }
        Instruction::CellGet { cell } => op(cell),
        Instruction::CellSet { cell, value } => {
            op(cell);
            op(value);
        }
        Instruction::EnvGet { env, .. } => op(env),
        Instruction::CallStatic { args, .. } => args.iter().for_each(&mut op),
        Instruction::CallInterface { receiver, args, .. } | Instruction::CallDynamic { receiver, args, .. } => {
            op(receiver);
            args.iter().for_each(&mut op);
        }
        Instruction::CallClosure { closure, args, .. } => {
            op(closure);
            args.iter().for_each(&mut op);
        }
        Instruction::CallRuntime { args, .. } => args.iter().for_each(|(a, _)| op(a)),
        Instruction::IsClass { object, .. } => op(object),
        Instruction::StoreGlobal { val, .. } => op(val),
        Instruction::Phi { .. } => {}
        Instruction::LoadIndexed { base, index } => {
            op(base);
            op(index);
        }
        Instruction::CallSeletor { recv, args, tupla_tipos, .. } => {
            op(recv);
            args.iter().for_each(&mut op);
            op(tupla_tipos);
        }
        Instruction::CallClosureRepasse { closure, args, desc } => {
            op(closure);
            op(args);
            op(desc);
        }
    }
    u
}

/// Troca cada uso de `de` por `para` numa instrução (sem os `phi`).
fn trocar_usos(inst: &mut Instruction, troca: &dyn Fn(ValueId) -> Option<ValueId>) {
    let t = |o: &mut Operand| {
        if let Operand::Val(v) = o
            && let Some(n) = troca(*v)
        {
            *v = n;
        }
    };
    match inst {
        Instruction::Const(_) | Instruction::Alloca(_) | Instruction::LoadGlobal { .. } | Instruction::TearOff { .. } | Instruction::ConstArray(_) | Instruction::Phi { .. } => {}
        Instruction::Add(a, b)
        | Instruction::Sub(a, b)
        | Instruction::Mul(a, b)
        | Instruction::SDiv(a, b)
        | Instruction::SRem(a, b)
        | Instruction::Shl(a, b)
        | Instruction::AShr(a, b)
        | Instruction::LShr(a, b)
        | Instruction::And(a, b)
        | Instruction::Or(a, b)
        | Instruction::Xor(a, b)
        | Instruction::FAdd(a, b)
        | Instruction::FSub(a, b)
        | Instruction::FMul(a, b)
        | Instruction::FDiv(a, b)
        | Instruction::ICmp(_, a, b)
        | Instruction::FCmp(_, a, b) => {
            t(a);
            t(b);
        }
        Instruction::Neg(a)
        | Instruction::Not(a)
        | Instruction::FNeg(a)
        | Instruction::LNot(a)
        | Instruction::IntToDouble(a)
        | Instruction::DoubleToInt(a)
        | Instruction::CheckNotNull(a) => t(a),
        Instruction::ZExt { op: a, .. }
        | Instruction::Trunc { op: a, .. }
        | Instruction::Bitcast { op: a, .. }
        | Instruction::Box { op: a, .. }
        | Instruction::Unbox { op: a, .. } => t(a),
        Instruction::AllocObject { fields, .. } => fields.iter_mut().for_each(t),
        Instruction::AllocList { elements } | Instruction::AllocSet { elements } | Instruction::AllocRecord { elements } => {
            elements.iter_mut().for_each(|(a, _)| t(a))
        }
        Instruction::AllocMap { entries } => entries.iter_mut().for_each(|((k, _), (v, _))| {
            t(k);
            t(v);
        }),
        Instruction::AllocCell { value } => t(value),
        Instruction::AllocEnv { values } => values.iter_mut().for_each(t),
        Instruction::AllocClosure { env, .. } => t(env),
        Instruction::Load { ptr, .. } => t(ptr),
        Instruction::Store { ptr, val } => {
            t(ptr);
            t(val);
        }
        Instruction::GetField { object, .. } => t(object),
        Instruction::SetField { object, value, .. } => {
            t(object);
            t(value);
        }
        Instruction::GetListElement { list, index } => {
            t(list);
            t(index);
        }
        Instruction::SetListElement { list, index, value } => {
            t(list);
            t(index);
            t(value);
        }
        Instruction::CellGet { cell } => t(cell),
        Instruction::CellSet { cell, value } => {
            t(cell);
            t(value);
        }
        Instruction::EnvGet { env, .. } => t(env),
        Instruction::CallStatic { args, .. } => args.iter_mut().for_each(t),
        Instruction::CallInterface { receiver, args, .. } | Instruction::CallDynamic { receiver, args, .. } => {
            t(receiver);
            args.iter_mut().for_each(t);
        }
        Instruction::CallClosure { closure, args, .. } => {
            t(closure);
            args.iter_mut().for_each(t);
        }
        Instruction::CallRuntime { args, .. } => args.iter_mut().for_each(|(a, _)| t(a)),
        Instruction::IsClass { object, .. } => t(object),
        Instruction::StoreGlobal { val, .. } => t(val),
        Instruction::LoadIndexed { base, index } => {
            t(base);
            t(index);
        }
        Instruction::CallSeletor { recv, args, tupla_tipos, .. } => {
            t(recv);
            args.iter_mut().for_each(t);
            t(tupla_tipos);
        }
        Instruction::CallClosureRepasse { closure, args, desc } => {
            t(closure);
            t(args);
            t(desc);
        }
    }
}

fn usos_do_terminador(term: &Terminator) -> Vec<ValueId> {
    let op = |o: &Operand| match o {
        Operand::Val(v) => Some(*v),
        _ => None,
    };
    match term {
        Terminator::Return(Some(o)) | Terminator::Throw(o) => op(o).into_iter().collect(),
        Terminator::CondBranch { cond, .. } => op(cond).into_iter().collect(),
        Terminator::Switch { val, .. } => op(val).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn trocar_no_terminador(term: &mut Terminator, troca: &dyn Fn(ValueId) -> Option<ValueId>) {
    let t = |o: &mut Operand| {
        if let Operand::Val(v) = o
            && let Some(n) = troca(*v)
        {
            *v = n;
        }
    };
    match term {
        Terminator::Return(Some(o)) | Terminator::Throw(o) => t(o),
        Terminator::CondBranch { cond, .. } => t(cond),
        Terminator::Switch { val, .. } => t(val),
        _ => {}
    }
}

fn sucessores(term: &Terminator) -> Vec<BlockId> {
    match term {
        Terminator::Branch(b) => vec![*b],
        Terminator::CondBranch { then_block, else_block, .. } => vec![*then_block, *else_block],
        Terminator::Switch { default, cases, .. } => {
            let mut v = vec![*default];
            v.extend(cases.iter().map(|(_, b)| *b));
            v
        }
        _ => Vec::new(),
    }
}

/// Passada 2: os valores SSA vivos na entrada de uma retomada vão para o
/// quadro a partir de `base` (gravados depois da definição, relidos antes de
/// cada uso). Devolve quantas posições usou.
fn guardar_vivos(
    func: &mut Function,
    quadro: &Operand,
    base: usize,
    suspensoes: &[(BlockId, BlockId)],
    entrada: BlockId,
    prox: &mut u32,
) -> usize {
    if suspensoes.is_empty() {
        return 0;
    }
    let idx: HashMap<BlockId, usize> = func.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    let n = func.blocks.len();
    let mut defs: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    let mut usos: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    // Usos de `phi` atribuídos ao fim do bloco de origem.
    let mut usos_fim: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    for (i, b) in func.blocks.iter().enumerate() {
        for (v, inst, _) in &b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (origem, o) in incoming {
                    if let (Operand::Val(x), Some(&j)) = (o, idx.get(origem)) {
                        usos_fim[j].insert(*x);
                    }
                }
            } else {
                for u in usos_de(inst) {
                    if !defs[i].contains(&u) {
                        usos[i].insert(u);
                    }
                }
            }
            defs[i].insert(*v);
        }
        for u in usos_do_terminador(&b.terminator) {
            if !defs[i].contains(&u) {
                usos[i].insert(u);
            }
        }
    }
    let mut succ: Vec<Vec<usize>> = func
        .blocks
        .iter()
        .map(|b| sucessores(&b.terminator).iter().filter_map(|s| idx.get(s).copied()).collect())
        .collect();
    for (a, r) in suspensoes {
        if let (Some(&ia), Some(&ir)) = (idx.get(a), idx.get(r)) {
            succ[ia].push(ir);
        }
    }
    let mut vivo_in: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    let mut mudou = true;
    while mudou {
        mudou = false;
        for i in (0..n).rev() {
            let mut out: HashSet<ValueId> = usos_fim[i].clone();
            for &s in &succ[i] {
                out.extend(vivo_in[s].iter().copied());
            }
            let mut inn: HashSet<ValueId> = usos[i].clone();
            for x in out {
                if !defs[i].contains(&x) {
                    inn.insert(x);
                }
            }
            if inn.len() != vivo_in[i].len() {
                vivo_in[i] = inn;
                mudou = true;
            }
        }
    }
    let params: HashSet<ValueId> = func.params.iter().map(|p| p.0).collect();
    let da_entrada = idx.get(&entrada).map(|&i| defs[i].clone()).unwrap_or_default();
    let mut guardar: Vec<ValueId> = suspensoes
        .iter()
        .filter_map(|(_, r)| idx.get(r))
        .flat_map(|&i| vivo_in[i].iter().copied())
        .filter(|v| !params.contains(v) && !da_entrada.contains(v))
        .collect();
    let tipos = tipos_da_funcao(func);
    guardar.retain(|v| !matches!(tipos.get(v), Some(Type::Void | Type::Ptr) | None));
    guardar.sort_by_key(|v| v.0);
    guardar.dedup();
    if guardar.is_empty() {
        return 0;
    }
    let posicao: HashMap<ValueId, (usize, Type)> = guardar
        .iter()
        .enumerate()
        .map(|(k, v)| (*v, (base + k, tipos[v])))
        .collect();

    for b in &mut func.blocks {
        // Releituras no fim do bloco para as entradas de `phi` dos sucessores
        // são feitas numa segunda volta (abaixo).
        let antigas = std::mem::take(&mut b.instructions);
        let n_phis = antigas.iter().take_while(|(_, i, _)| matches!(i, Instruction::Phi { .. })).count();
        let mut novas: Vec<(ValueId, Instruction, Type)> = Vec::with_capacity(antigas.len());
        let mut depois_dos_phis: Vec<(ValueId, Instruction, Type)> = Vec::new();
        for (k, (v, mut inst, ty)) in antigas.into_iter().enumerate() {
            if !matches!(inst, Instruction::Phi { .. }) {
                // Releitura de cada valor guardado que a instrução usa.
                let mut trocas: HashMap<ValueId, ValueId> = HashMap::new();
                for u in usos_de(&inst) {
                    if let Some(&(s, t)) = posicao.get(&u)
                        && !trocas.contains_key(&u)
                    {
                        let nv = novo_valor(prox);
                        novas.extend(ler_do_quadro(quadro, s, t, nv, prox));
                        trocas.insert(u, nv);
                    }
                }
                if !trocas.is_empty() {
                    trocar_usos(&mut inst, &|x| trocas.get(&x).copied());
                }
            }
            let e_phi = matches!(inst, Instruction::Phi { .. });
            novas.push((v, inst, ty));
            if let Some(&(s, t)) = posicao.get(&v) {
                let grava = gravar_no_quadro(quadro, s, Operand::Val(v), t, prox);
                if e_phi {
                    depois_dos_phis.extend(grava);
                } else {
                    novas.extend(grava);
                }
            }
            if k + 1 == n_phis {
                novas.append(&mut depois_dos_phis);
            }
        }
        novas.append(&mut depois_dos_phis);
        // O terminador.
        let mut trocas: HashMap<ValueId, ValueId> = HashMap::new();
        for u in usos_do_terminador(&b.terminator) {
            if let Some(&(s, t)) = posicao.get(&u)
                && !trocas.contains_key(&u)
            {
                let nv = novo_valor(prox);
                novas.extend(ler_do_quadro(quadro, s, t, nv, prox));
                trocas.insert(u, nv);
            }
        }
        if !trocas.is_empty() {
            trocar_no_terminador(&mut b.terminator, &|x| trocas.get(&x).copied());
        }
        b.instructions = novas;
    }
    // Entradas de `phi` com valor guardado: releitura no fim do bloco de
    // origem (antes do terminador dele).
    let mut releituras: Vec<(BlockId, ValueId, ValueId)> = Vec::new(); // (origem, antigo, novo)
    for b in &mut func.blocks {
        for (_, inst, _) in &mut b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (origem, o) in incoming.iter_mut() {
                    if let Operand::Val(x) = o
                        && posicao.contains_key(x)
                    {
                        let nv = novo_valor(prox);
                        releituras.push((*origem, *x, nv));
                        *x = nv;
                    }
                }
            }
        }
    }
    for (origem, antigo, novo) in releituras {
        if let Some(b) = func.blocks.iter_mut().find(|b| b.id == origem) {
            let (s, t) = posicao[&antigo];
            b.instructions.extend(ler_do_quadro(quadro, s, t, novo, prox));
        }
    }
    guardar.len()
}
