//! `async`/`await` (P6, docs/NATIVO-PLANO.md §7.7): o corpo como máquina de
//! estados, no molde do dart2js (`pkg/compiler/lib/src/js/rewrite_async.dart`)
//! e dos apoios de `native_sdk/async/async_patch.dart`.
//!
//! Uma função `async f(p…) { … }` vira duas:
//!
//! * **o stub `f`** (o símbolo de sempre): cria o **quadro** — um objeto do
//!   heap —, o `Completer` (`_makeAsyncAwaitCompleter`), a closure do corpo
//!   (registrada na zona por `_envolverCorpo`) e começa o corpo de forma
//!   síncrona (`_asyncStartSync`), devolvendo `completer.future`;
//! * **o corpo `f$async(env, código, result)`**, uma closure com o
//!   quadro no ambiente. O bloco de entrada lê o quadro e salta pelo
//!   `state` guardado nele: 0 é o começo; cada `await` é um estado.
//!
//! **`await e`.** Grava o estado `k` no quadro, chama `_asyncAwait(e,
//! corpo)` e **returns**; o block de resumption `R_k` só é alcançado pelo
//! `switch` da entrada, quando o `Future` completa e chama o corpo de novo.
//! Com `código == 1` (`_ERRO`) o `result` é um `_ErroAssincrono`: o erro é
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
use super::locals::{Local, Mode};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, FunctionBody};
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};

/// Id de classe do quadro de uma função `async` (abaixo das formas de
/// record, `context::SHAPE_BASE_ID`).
pub const ID_QUADRO_ASYNC: u32 = 0x3FFF_FF00;

const Q_STATE: usize = 0;
const Q_COMPLETER: usize = 1;
const Q_BODY: usize = 2;
const Q_THIS: usize = 3;
const Q_ENV: usize = 4;
const Q_TUPLE: usize = 5;
const Q_PARAMS: usize = 6;

/// De onde sai o tipo de retorno de um corpo `async`/`sync*`/`async*`: o
/// tipo (declarado, ou o estático de uma closure) ou a anotação de uma
/// função local.
#[derive(Clone, Copy)]
pub enum AsyncReturn {
    Tipo(dartforge_types::table::TypeId),
    Annotation(ast::TypeId),
}

/// O tipo de corpo suspenso: `async`, `sync*` ou `async*`.
///
/// Os três usam a mesma máquina de estados (quadro no heap, `switch` pelo
/// estado na entrada, as duas passadas sobre a HIR); mudam o stub, os pontos
/// de suspensão e o que `return` e a exceção não capturada fazem:
///
/// | | stub devolve | suspensão | `return` | exceção no topo |
/// | --- | --- | --- | --- | --- |
/// | `async` | `completer.future` | `await` | `_asyncReturn` | `_asyncRethrow` |
/// | `sync*` | `_SyncStarIterable` | `yield`, `yield*` (devolve `true`) | devolve `false` | sai do corpo |
/// | `async*` | o stream do controlador | `await`, `yield`, `yield*` | `_asyncStarReturn` | `_asyncStarErro` |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyType {
    Async,
    SyncStar,
    AsyncStar,
}

/// O tipo de corpo de um modificador que suspende (`async`, `sync*`,
/// `async*`).
///
/// # Panics
/// Com `AsyncModifier::None`, que não é corpo suspenso.
pub fn body_type(m: ast::AsyncModifier) -> BodyType {
    match m {
        ast::AsyncModifier::Async => BodyType::Async,
        ast::AsyncModifier::SyncStar => BodyType::SyncStar,
        ast::AsyncModifier::AsyncStar => BodyType::AsyncStar,
        ast::AsyncModifier::None => panic!("corpo síncrono não é suspenso"),
    }
}

/// O estado do lowering do corpo de uma função `async`, `sync*` ou `async*`.
#[derive(Debug, Clone)]
pub struct AsyncState {
    pub ty: BodyType,
    /// `sync*`: o iterador que chama o corpo (um parâmetro; nulo nos outros).
    pub iterator: Operand,
    pub frame: Operand,
    /// A closure do corpo registrada na zona (a que `_asyncAwait` recebe).
    pub body: Operand,
    /// O `Completer` (`async`) ou o controlador do stream (`async*`).
    pub completer: Operand,
    pub code: Operand,
    pub result: Operand,
    /// `(k, R_k)`: o bloco de retomada de cada estado.
    pub resumptions: Vec<(i64, BlockId)>,
    /// `(A, R_k)`: o bloco que suspende e a retomada dele.
    pub suspensions: Vec<(BlockId, BlockId)>,
    /// O próximo `Return` é cru (suspensão ou o fim do tratador do topo):
    /// não passa por `_asyncReturn`.
    pub raw_return: bool,
    /// O rastro simbólico (§13.14): a posição de cada `await`, pelo estado.
    pub waits: Vec<(i64, (u32, u32))>,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Função de topo `name` do `dart:async` (os apoios do `async_patch`).
    pub fn async_support(&self, name_str: &str) -> Option<usize> {
        let lib = self.ctx.program.libraries.iter().position(|l| l.uri == "dart:async")?;
        let sym = self.ctx.interner.lookup(name_str)?;
        let b = self.ctx.program.lookup(dartforge_elements::model::LibraryId(lib as u32), sym)?;
        match b.getter? {
            dartforge_elements::model::Element::Function(f) => Some(f.0 as usize),
            _ => None,
        }
    }

    /// Chama o apoio `name` do `dart:async` com os argumentos já avaliados.
    pub(super) fn call_support(&mut self, name_str: &str, args: Vec<Operand>, span: Span) -> Operand {
        let Some(fid) = self.async_support(name_str) else {
            return self.unsupported(&format!("apoio `{name_str}` do dart:async ausente"), span);
        };
        let evaluated: Vec<super::members::Evaluated> = args.into_iter().map(|a| (None, a)).collect();
        let args = self.match_args(fid, &evaluated);
        self.call_direct(fid, None, args)
    }

    /// [`FnBuilder::call_support`] no caminho do `return`: o desvio de
    /// exceção dele, sem tratador, é um retorno cru — senão voltaria a
    /// `async_return`, que chamaria o apoio de novo.
    fn call_support_raw(&mut self, name_str: &str, args: Vec<Operand>) -> Operand {
        let before = self.async_state.as_ref().is_some_and(|e| e.raw_return);
        if let Some(e) = self.async_state.as_mut() {
            e.raw_return = true;
        }
        let r = self.call_support(name_str, args, Span { start: 0, end: 0 });
        if let Some(e) = self.async_state.as_mut() {
            e.raw_return = before;
        }
        r
    }

    /// A referência fica explícita na HIR para a preparação de ownership.
    /// Os bits escalares conservam a conversão da representação `ty`.
    fn read_position(&mut self, obj: Operand, idx: usize, ty: Type) -> Operand {
        if ty == Type::Ref {
            return self.emit(Instruction::GetField { object: obj, index: idx }, Type::Ref);
        }
        let bits = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (Operand::Constant(Constant::Int(idx as i64)), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        self.bits_to(bits, ty)
    }

    /// `dartforge_object_set` com a tag da representação do valor (E1).
    fn store_position(&mut self, obj: Operand, idx: usize, val: Operand) {
        let (bits, is_ref) = self.to_bits(val);
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
    fn stub_locals(&self) -> (Vec<(SymbolId, Local)>, Vec<(SymbolId, Local)>) {
        let mut env = Vec::new();
        let mut others = Vec::new();
        let mut seen_set = HashSet::new();
        for scope in self.scopes.iter().rev() {
            let mut names: Vec<(&SymbolId, &Local)> = scope.iter().collect();
            names.sort_by_key(|(s, l)| (l.offset, self.ctx.symbol_name(**s).to_string()));
            for (s, l) in names {
                if !seen_set.insert(*s) {
                    continue;
                }
                match l.mode {
                    Mode::Environment { .. } => env.push((*s, l.clone())),
                    _ => others.push((*s, l.clone())),
                }
            }
        }
        env.sort_by_key(|(_, l)| match l.mode {
            Mode::Environment { index: idx_value, .. } => idx_value,
            _ => 0,
        });
        others.sort_by_key(|(s, l)| (l.offset, self.ctx.symbol_name(*s).to_string()));
        (env, others)
    }

    /// O corpo de uma função `async`, `sync*` ou `async*`: este builder é o
    /// stub (parâmetros e capturas já declarados); o corpo vira
    /// `<símbolo>$async` e a entrada uniforme dele. Termina o stub.
    /// O tipo do valor/elemento pela anotação de retorno de uma função local
    /// (`Future<T> f() async`, `Iterable<T> g() sync*`, `FutureOr<T>`).
    fn async_annotation_value(&self, ast: &ast::Ast, a: ast::TypeId, ty: BodyType) -> Option<super::rti::Recipe> {
        let ast::TypeKind::Named { name, args } = &ast.ty(a).kind else { return None };
        let name_str = self.ctx.symbol_name(name.last()?.sym);
        let expected = match ty {
            BodyType::Async => name_str == "Future" || name_str == "FutureOr",
            BodyType::SyncStar => name_str == "Iterable",
            BodyType::AsyncStar => name_str == "Stream",
        };
        if !expected {
            return None;
        }
        self.annotation_recipe(ast.ty(*args.first()?))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn lower_async_body(
        &mut self,
        ast: &ast::Ast,
        params: &[ast::Parameter],
        fn_body: &FunctionBody,
        span: Span,
        ret: Option<AsyncReturn>,
        ty: BodyType,
    ) {
        let (captures, parameters) = self.stub_locals();
        let body_symbol = format!("{}$async", self.func.symbol);
        let readable = self.func.name.clone();

        // --- corpo ------------------------------------------------------
        let mut b = FnBuilder::new(self.ctx, self.unit_id, body_symbol.clone(), readable.clone(), Type::Ref);
        let env = Operand::Val(b.add_param("env".to_string(), Type::Ref));
        let iterator = if ty == BodyType::SyncStar {
            Operand::Val(b.add_param("iterador".to_string(), Type::Ref))
        } else {
            Operand::Constant(Constant::Null)
        };
        let code = Operand::Val(b.add_param("codigo".to_string(), Type::Ref));
        let result = Operand::Val(b.add_param("resultado".to_string(), Type::Ref));
        let frame = b.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
        let state = b.read_position(frame.clone(), Q_STATE, Type::I64);
        let completer = b.read_position(frame.clone(), Q_COMPLETER, Type::Ref);
        let closer = b.read_position(frame.clone(), Q_BODY, Type::Ref);
        if self.this_param.is_some() {
            let t = b.read_position(frame.clone(), Q_THIS, Type::Ref);
            b.this_param = Some(t);
            b.finalizable_this = self.finalizable_this;
            b.enclosing_class = self.enclosing_class;
        }
        if !captures.is_empty() {
            let from_outside = b.read_position(frame.clone(), Q_ENV, Type::Ref);
            for (sym, l) in &captures {
                if let Mode::Environment { index: idx_value, cell, .. } = l.mode {
                    b.bind_env(*sym, from_outside.clone(), idx_value, cell, l.ty, l);
                }
            }
        }
        // RTI: as variáveis de tipo do stub (a tupla mora no quadro).
        b.function_type_params = self.function_type_params.clone();
        b.local_params = self.local_params.clone();
        b.this_extension = self.this_extension;
        b.this_ext_type = self.this_ext_type;
        b.member_class = self.member_class;
        b.class_by_tuple = self.class_by_tuple;
        if self.type_tuple.is_some() {
            let t = b.read_position(frame.clone(), Q_TUPLE, Type::I64);
            b.type_tuple = Some(t);
        }
        b.async_state = Some(Box::new(AsyncState {
            ty,
            iterator,
            frame: frame.clone(),
            body: closer,
            completer: completer.clone(),
            code,
            result,
            resumptions: Vec::new(),
            suspensions: Vec::new(),
            raw_return: false,
            waits: Vec::new(),
        }));
        // O rastro simbólico (§13.14): o stub fica fora do rastro (a VM tem
        // um quadro só para a função), e o corpo `async` retomado começa a
        // cadeia de quem espera pelo quadro, que ele empurra na pilha do
        // rastro.
        if self.ctx.stack_trace {
            self.stack_trace.marks |= crate::hir::stack_trace_marks::HIDDEN;
            b.stack_trace.token = self.stack_trace.token;
            if matches!(ty, BodyType::Async | BodyType::AsyncStar) {
                b.stack_trace.marks |= crate::hir::stack_trace_marks::ASYNC_BODY;
                if ty == BodyType::AsyncStar {
                    b.stack_trace.marks |= crate::hir::stack_trace_marks::ASYNC_GENERATOR;
                }
                b.stack_trace.stack = Some((0, frame.clone()));
            }
        }
        let entry = b.current_block;
        let start = b.new_block();
        b.set_block(start);
        // Os `alloca` do corpo viram posições do quadro: sem funções locais
        // diretas (o endereço de um local não sobreviveria à troca).
        b.without_directs = true;
        b.prepare_captures(
            ast,
            super::capture::Root {
                parameters: params,
                body: Some(fn_body),
                initializers: &[],
            },
        );
        b.open_scope();
        for (k, (sym, l)) in parameters.iter().enumerate() {
            let v = b.read_position(frame.clone(), Q_PARAMS + k, l.ty);
            match l.offset {
                Some(off) => b.declare_variable(*sym, off, l.ty, v),
                None => b.declare_local_with_value(*sym, l.ty, v),
            }
            // A representação do quadro não substitui o tipo da declaração.
            b.assign_semantic_type(*sym, l.static_type);
        }
        // O tratador do topo: exceção que ninguém no corpo pegou completa o
        // `Future` com erro (`async`) ou vai ao stream (`async*`). No
        // `sync*` ela sai do corpo, e o `moveNext` a trata.
        let top = b.new_block();
        if ty != BodyType::SyncStar {
            b.exception_targets.push(top);
        }
        // `async*`: o corpo só começa quando alguém escuta, e começa
        // cancelado se a inscrição já foi cancelada (`asyncStarBody(true)`).
        if ty == BodyType::AsyncStar {
            b.exit_if_cancelled();
        }
        match fn_body {
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
        if ty != BodyType::SyncStar {
            b.exception_targets.pop();
        }
        // O tratador do topo não tem tratador: nada nele volta a
        // `_asyncReturn` (um retorno de exceção aqui é cru).
        if let Some(e) = b.async_state.as_mut() {
            e.raw_return = true;
        }
        b.set_block(top);
        let error = b.pending_exception();
        let stack_trace = b.pending_stack_trace();
        b.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        match ty {
            BodyType::Async => {
                b.call_support("_asyncRethrow", vec![error, stack_trace, completer], span);
            }
            BodyType::AsyncStar => {
                b.call_support("_asyncStarErro", vec![completer, error, stack_trace], span);
            }
            // Sem tratador do topo: o bloco não é alcançado.
            BodyType::SyncStar => {}
        }
        b.terminate_raw(Operand::Constant(Constant::Null));

        // O salto pelo estado, no fim do bloco de entrada.
        let st = b.async_state.take().expect("estado async");
        let idx0 = b.func.blocks.iter().position(|x| x.id == entry).expect("entrada");
        b.func.blocks[idx0].terminator = Terminator::Switch {
            val: state,
            default: start,
            cases: st.resumptions.clone(),
        };
        b.terminated_blocks.insert(entry);

        // As duas passadas: locais e temporários que atravessam um `await`
        // vão para o quadro.
        let mut next = b.next_value;
        let locals_base = Q_PARAMS + parameters.len();
        let n_locals = convert_allocas(&mut b.func, &frame, locals_base, &mut next);
        let n_temp = save_live(&mut b.func, &frame, locals_base + n_locals, &st.suspensions, entry, &mut next);
        b.next_value = next;
        let size = locals_base + n_locals + n_temp;
        // O nome do corpo leva a impressão digital dele: um quadro suspenso
        // num `await` guarda o estado e as posições deste corpo, e numa
        // recarga do JIT só pode retomar num corpo idêntico. Um corpo que
        // mudou tem outro nome, e os quadros vivos seguem no código em que
        // começaram (como os quadros ativos na VM).
        let body_symbol = format!(
            "{body_symbol}$q{:08x}",
            (super::closures::hash_name(&format!("{size}|{:?}", b.func.blocks)) as u64) as u32
        );
        b.func.symbol = body_symbol.clone();
        if self.ctx.stack_trace && matches!(ty, BodyType::Async | BodyType::AsyncStar) {
            let n = st.resumptions.iter().map(|(k, _)| *k).max().unwrap_or(0).max(0) as usize;
            let mut waits = vec![(0u32, 0u32); n];
            for (k, p) in &st.waits {
                if let Some(x) = usize::try_from(*k).ok().and_then(|k| k.checked_sub(1)).and_then(|i| waits.get_mut(i)) {
                    *x = *p;
                }
            }
            b.stack_trace.async_body = Some(crate::hir::TraceAsyncBody { entry: format!("{body_symbol}$ent"), waits });
        }
        if self.ctx.arc_memory {
            // Corpo Dart chamado pela entrada: o retorno Ref é Owned.
            b.dart_ref_returns.insert(body_symbol.clone());
        }
        self.absorb(b);

        // --- entrada uniforme do corpo: ([iterador,] código, resultado) --
        let mut infos = Vec::new();
        if ty == BodyType::SyncStar {
            infos.push(super::closures::EntryParam {
                name: Some("iterador".to_string()),
                kind: ast::ParameterKind::Required,
                required: true,
                pattern: super::closures::DefaultValue::Absent,
            });
        }
        infos.extend([
            super::closures::EntryParam {
                name: Some("codigo".to_string()),
                kind: ast::ParameterKind::Required,
                required: true,
                pattern: super::closures::DefaultValue::Absent,
            },
            super::closures::EntryParam {
                name: Some("resultado".to_string()),
                kind: ast::ParameterKind::Required,
                required: true,
                pattern: super::closures::DefaultValue::Absent,
            },
        ]);
        let symbol_ent = format!("{body_symbol}$ent");
        let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), readable, Type::Ref);
        let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
        let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
        e.declare_arc_uniform_return();
        let env_e = e.closure_context(clo);
        if let Some(vals) = e.unpack(&infos, args, desc) {
            let mut all = vec![env_e];
            all.extend(vals);
            let r = e.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: body_symbol,
                    args: all,
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            e.terminate(Terminator::Return(Some(r)));
        }
        self.absorb(e);

        // --- o stub -----------------------------------------------------
        let mut fields: Vec<Operand> = vec![Operand::Constant(Constant::Int(0)); size];
        fields[Q_COMPLETER] = Operand::Constant(Constant::Null);
        fields[Q_BODY] = Operand::Constant(Constant::Null);
        fields[Q_THIS] = self.this_param.clone().map_or(Operand::Constant(Constant::Null), |t| self.coerce(t, Type::Ref));
        fields[Q_ENV] = captures
            .iter()
            .find_map(|(_, l)| match &l.mode {
                Mode::Environment { env, .. } => Some(env.clone()),
                _ => None,
            })
            .unwrap_or(Operand::Constant(Constant::Null));
        if let Some(t) = self.type_tuple.clone() {
            fields[Q_TUPLE] = t;
        }
        for (k, (_, l)) in parameters.iter().enumerate() {
            let v = self.read_local(l);
            fields[Q_PARAMS + k] = v;
        }
        let frame_s = self.emit(
            Instruction::AllocObject {
                class_id: ID_QUADRO_ASYNC,
                fields: fields,
            },
            Type::Ref,
        );
        // O `T` do `Completer<T>`, do `Iterable<T>` ou do `Stream<T>`: para
        // `async`, o tipo do valor do `Future` declarado (especificação §9
        // "future value type": `Future<T>`/`FutureOr<T>` dão `T`); para os
        // geradores, o tipo do elemento (§9 "element type"). O resto,
        // `dynamic`.
        let expected_class = match ty {
            BodyType::Async => self.ctx.core.future_class,
            BodyType::SyncStar => self.ctx.core.iterable_class,
            BodyType::AsyncStar => self.ctx.core.stream_class,
        };
        let val_ref: Option<super::rti::Recipe> = match ret {
            Some(AsyncReturn::Tipo(t)) => match self.ctx.table.get(t) {
                dartforge_types::table::Type::Interface { class, args, .. } if Some(*class) == expected_class => {
                    args.first().map(|a| self.type_recipe(*a))
                }
                dartforge_types::table::Type::FutureOr { arg, .. } if ty == BodyType::Async => {
                    Some(self.type_recipe(*arg))
                }
                _ => None,
            },
            Some(AsyncReturn::Annotation(a)) => self.async_annotation_value(ast, a, ty),
            None => None,
        };
        let arm_tuple = |b: &mut Self| {
            let saved = b.armed_tuple.take();
            if let Some(v) = &val_ref {
                let t = b.recipe_rti(&super::rti::Recipe { text: format!("L<{}>", v.text), variables: v.variables });
                b.armed_tuple = Some(t);
            }
            saved
        };
        match ty {
            BodyType::Async => {
                let saved = arm_tuple(self);
                let completer_s = self.call_support("_makeAsyncAwaitCompleter", Vec::new(), span);
                self.armed_tuple = saved;
                let completer_s = self.coerce(completer_s, Type::Ref);
                self.store_position(frame_s.clone(), Q_COMPLETER, completer_s.clone());
                let body_s = self.close_body(frame_s.clone(), symbol_ent, span);
                let r = self.call_support("_asyncStartSync", vec![body_s, completer_s], span);
                self.terminate(Terminator::Return(Some(r)));
            }
            BodyType::AsyncStar => {
                let saved = arm_tuple(self);
                let controller = self.call_support("_makeAsyncStarController", Vec::new(), span);
                self.armed_tuple = saved;
                let controller = self.coerce(controller, Type::Ref);
                self.store_position(frame_s.clone(), Q_COMPLETER, controller.clone());
                let body_s = self.close_body(frame_s.clone(), symbol_ent, span);
                let r = self.call_support("_asyncStarStart", vec![controller, body_s], span);
                self.terminate(Terminator::Return(Some(r)));
            }
            BodyType::SyncStar => {
                // O quadro do stub é o modelo: cada `iterator` começa o corpo
                // num quadro novo, com os parâmetros dele (`$new`).
                let new_symbol = format!("{symbol_ent}$novo");
                self.new_frame_function(&new_symbol, &symbol_ent, &parameters, size);
                let env_s = self.emit(Instruction::AllocEnv { values: vec![frame_s] }, Type::Ref);
                let new = self.emit(
                    Instruction::AllocClosure {
                        code_symbol: format!("{new_symbol}$ent"),
                        env: env_s,
                    },
                    Type::Ref,
                );
                let saved = arm_tuple(self);
                let r = self.call_support("_makeSyncStarIterable", vec![new], span);
                self.armed_tuple = saved;
                self.terminate(Terminator::Return(Some(r)));
            }
        }
    }

    /// A closure do corpo sobre o quadro, registrada na zona
    /// (`_envolverCorpo`) e guardada no quadro.
    fn close_body(&mut self, frame: Operand, symbol_ent: String, span: Span) -> Operand {
        let env = self.emit(Instruction::AllocEnv { values: vec![frame.clone()] }, Type::Ref);
        let clo = self.emit(
            Instruction::AllocClosure {
                code_symbol: symbol_ent,
                env,
            },
            Type::Ref,
        );
        let fn_body = self.call_support("_envolverCorpo", vec![clo], span);
        let fn_body = self.coerce(fn_body, Type::Ref);
        self.store_position(frame, Q_BODY, fn_body.clone());
        fn_body
    }

    /// `sync*`: a closure sem argumentos `new_symbol` (a entrada uniforme
    /// `<new_symbol>$ent`), que copia do quadro modelo (o ambiente dela)
    /// `this`, as capturas, a tupla de tipos e os parâmetros para um quadro
    /// novo e devolve a closure do corpo sobre ele — o `_stateAtStart._clone()`
    /// da VM.
    fn new_frame_function(
        &mut self,
        new_symbol: &str,
        body_symbol_ent: &str,
        parameters: &[(SymbolId, Local)],
        size: usize,
    ) {
        let readable = self.func.name.clone();
        let mut e = FnBuilder::new(self.ctx, self.unit_id, format!("{new_symbol}$ent"), readable, Type::Ref);
        let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
        let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
        e.declare_arc_uniform_return();
        let env = e.closure_context(clo);
        if e.unpack(&[], args, desc).is_some() {
            let model = e.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
            let mut fields: Vec<Operand> = vec![Operand::Constant(Constant::Int(0)); size];
            fields[Q_COMPLETER] = Operand::Constant(Constant::Null);
            fields[Q_BODY] = Operand::Constant(Constant::Null);
            fields[Q_THIS] = e.read_position(model.clone(), Q_THIS, Type::Ref);
            fields[Q_ENV] = e.read_position(model.clone(), Q_ENV, Type::Ref);
            if self.type_tuple.is_some() {
                fields[Q_TUPLE] = e.read_position(model.clone(), Q_TUPLE, Type::I64);
            }
            for (k, (_, l)) in parameters.iter().enumerate() {
                fields[Q_PARAMS + k] = e.read_position(model.clone(), Q_PARAMS + k, l.ty);
            }
            let frame = e.emit(
                Instruction::AllocObject {
                    class_id: ID_QUADRO_ASYNC,
                    fields: fields,
                },
                Type::Ref,
            );
            let env_q = e.emit(Instruction::AllocEnv { values: vec![frame] }, Type::Ref);
            let fn_body = e.emit(
                Instruction::AllocClosure {
                    code_symbol: body_symbol_ent.to_string(),
                    env: env_q,
                },
                Type::Ref,
            );
            e.terminate(Terminator::Return(Some(fn_body)));
        }
        self.absorb(e);
    }

    /// `async*`: termina o corpo (pelos `finally`) se o valor da retomada diz
    /// que a inscrição foi cancelada.
    fn exit_if_cancelled(&mut self) {
        let result = self.async_state.as_ref().expect("corpo async*").result.clone();
        let cancelled = self.coerce(result, Type::I1);
        let exit = self.new_block();
        let follow = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: cancelled,
            then_block: exit,
            else_block: follow,
        });
        self.set_block(exit);
        self.route_return(None);
        self.set_block(follow);
    }

    /// `yield e` e `yield* e` (especificação §18.16, §18.17) num corpo
    /// `sync*` ou `async*`.
    pub fn lower_yield(&mut self, ast: &ast::Ast, star: bool, e: ast::ExprId, span: Span) {
        let ty = self.async_state.as_ref().map(|e| e.ty);
        if !matches!(ty, Some(BodyType::SyncStar | BodyType::AsyncStar)) {
            self.unsupported("yield fora de gerador", span);
            return;
        }
        let v = self.lower_expr(ast, e);
        let v = if matches!(self.operand_type(&v), Type::Void) {
            Operand::Constant(Constant::Null)
        } else {
            self.coerce(v, Type::Ref)
        };
        if self.is_terminated() {
            return;
        }
        let st = self.async_state.as_ref().expect("corpo gerador");
        let (frame, iterator, controller) = (st.frame.clone(), st.iterator.clone(), st.completer.clone());
        let k = st.resumptions.len() as i64 + 1;
        if ty == Some(BodyType::SyncStar) {
            let support = if star { "_syncStarYieldStar" } else { "_syncStarYield" };
            self.call_support(support, vec![iterator, v], span);
            if self.is_terminated() {
                return;
            }
            self.store_position(frame, Q_STATE, Operand::Constant(Constant::Int(k)));
            let suspends = self.current_block;
            self.terminate_raw(Operand::Constant(Constant::Bool(true)));
            let resumption = self.register_resumption(k, suspends);
            // A exceção do iterador de um `yield*` volta lançada aqui.
            let st = self.async_state.as_ref().expect("corpo sync*");
            let (code, result) = (st.code.clone(), st.result.clone());
            self.set_block(resumption);
            self.throw_if_error(code, result, span);
        } else {
            let support = if star { "_asyncStarAddStream" } else { "_asyncStarAdd" };
            let stop = self.call_support(support, vec![controller, v], span);
            if self.is_terminated() {
                return;
            }
            let stop = self.coerce(stop, Type::I1);
            let exit = self.new_block();
            let suspend = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: stop,
                then_block: exit,
                else_block: suspend,
            });
            self.set_block(exit);
            self.route_return(None);
            self.set_block(suspend);
            // A posição do `yield` (a do comando): a do quadro do gerador
            // suspenso aqui, no rastro.
            let position = self.position.unwrap_or((0, 0));
            if let Some(e) = self.async_state.as_mut() {
                e.waits.push((k, position));
            }
            self.store_position(frame, Q_STATE, Operand::Constant(Constant::Int(k)));
            let suspends = self.current_block;
            self.terminate_raw(Operand::Constant(Constant::Null));
            let resumption = self.register_resumption(k, suspends);
            self.set_block(resumption);
            self.exit_if_cancelled();
        }
    }

    /// `await for (alvo in s) body` (especificação §18.6.3), baixado como o
    /// kernel da VM o desaçucara (`native_sdk/async/async_patch.dart`):
    /// `StreamIterator`, `while (await it.moveNext())`, e o `finally` que
    /// cancela a inscrição se ela ainda existe — por `break`, `return` ou
    /// exceção no corpo.
    pub fn lower_await_for(
        &mut self,
        ast: &ast::Ast,
        target: &ast::ForInTarget,
        iterable: ast::ExprId,
        body: ast::StmtId,
        span: Span,
    ) {
        if !self.async_state.as_ref().is_some_and(|e| e.ty != BodyType::SyncStar) {
            self.unsupported("await for fora de função async", span);
            return;
        }
        let s = self.lower_expr(ast, iterable);
        let s = self.coerce(s, Type::Ref);
        let it = self.call_support("_awaitForIterador", vec![s], span);
        let it = self.coerce(it, Type::Ref);
        if self.is_terminated() {
            return;
        }
        let labels = std::mem::take(&mut self.pending_labels);
        let it_c = it.clone();
        let mut loop_fn = |b: &mut Self| {
            let head = b.new_block();
            let fn_body = b.new_block();
            let end = b.new_block();
            for &r in &labels {
                b.labeled_break_targets.insert(r, end);
                b.labeled_continue_targets.insert(r, head);
            }
            b.terminate(Terminator::Branch(head));
            b.set_block(head);
            let next_item = b.call_by_selector(it_c.clone(), "c:moveNext".to_string(), &[]);
            let ok = b.await_value(next_item, span);
            let ok = b.coerce(ok, Type::I1);
            b.terminate(Terminator::CondBranch { cond: ok, then_block: fn_body, else_block: end });
            b.set_block(fn_body);
            b.emit_safepoint();
            b.break_targets.push(end);
            b.continue_targets.push(head);
            b.open_scope();
            let x = b.call_by_selector(it_c.clone(), "g:current".to_string(), &[]);
            b.bind_for_in_target(ast, target, x, iterable, span);
            b.lower_stmt(ast, body);
            b.close_scope();
            b.terminate(Terminator::Branch(head));
            b.break_targets.pop();
            b.continue_targets.pop();
            for &r in &labels {
                b.labeled_break_targets.remove(&r);
                b.labeled_continue_targets.remove(&r);
            }
            b.set_block(end);
        };
        let mut cancel = |b: &mut Self| {
            let active = b.call_support("_awaitForAtivo", vec![it.clone()], span);
            let active = b.coerce(active, Type::I1);
            let yes = b.new_block();
            let after = b.new_block();
            b.terminate(Terminator::CondBranch { cond: active, then_block: yes, else_block: after });
            b.set_block(yes);
            let f = b.call_by_selector(it.clone(), "c:cancel".to_string(), &[]);
            b.await_value(f, span);
            b.terminate(Terminator::Branch(after));
            b.set_block(after);
        };
        self.lower_try_with(ast, &mut loop_fn, &[], Some(&mut cancel));
    }

    /// Cria o bloco de retomada do estado `k`, suspenso em `suspends`.
    fn register_resumption(&mut self, k: i64, suspends: BlockId) -> BlockId {
        let resumption = self.new_block();
        let st = self.async_state.as_mut().expect("corpo suspenso");
        st.resumptions.push((k, resumption));
        st.suspensions.push((suspends, resumption));
        resumption
    }

    /// Na retomada: com `código == _ERRO`, lança o `_ErroAssincrono` do
    /// `result` ali; senão segue. Devolve o bloco que desvia para o
    /// caminho sem erro (o único predecessor dele).
    fn throw_if_error(&mut self, code: Operand, result: Operand, span: Span) -> BlockId {
        // `código` chega como `int` numa posição `Ref` (Smi).
        let c = self.coerce(code, Type::I64);
        let is_error = self.emit(
            Instruction::ICmp(ICmpOp::Eq, c, Operand::Constant(Constant::Int(1))),
            Type::I1,
        );
        let b_error = self.new_block();
        let b_ok = self.new_block();
        let origin = self.current_block;
        self.terminate(Terminator::CondBranch {
            cond: is_error,
            then_block: b_error,
            else_block: b_ok,
        });
        self.set_block(b_error);
        self.throw_async_error(result, span);
        self.set_block(b_ok);
        origin
    }

    /// `Return` que não passa por `_asyncReturn`.
    fn terminate_raw(&mut self, v: Operand) {
        let before = self.async_state.as_ref().is_some_and(|e| e.raw_return);
        if let Some(e) = self.async_state.as_mut() {
            e.raw_return = true;
        }
        self.terminate(Terminator::Return(Some(v)));
        if let Some(e) = self.async_state.as_mut() {
            e.raw_return = before;
        }
    }

    /// O `return` de um corpo `async` (chamado por `terminate`): `_asyncReturn(v,
    /// completer)` e o ret cru. Uma exceção de `_asyncReturn` (o `as T`
    /// do valor) vai ao tratador do topo.
    pub fn async_return(&mut self, v: Option<Operand>) {
        let st = self.async_state.as_ref().expect("corpo async");
        let completer = st.completer.clone();
        match st.ty {
            BodyType::Async => {}
            BodyType::SyncStar => {
                self.terminate_raw(Operand::Constant(Constant::Bool(false)));
                return;
            }
            BodyType::AsyncStar => {
                self.call_support_raw("_asyncStarReturn", vec![completer]);
                self.terminate_raw(Operand::Constant(Constant::Null));
                return;
            }
        }
        let v = match v {
            Some(v) if !matches!(self.operand_type(&v), Type::Void) => self.coerce(v, Type::Ref),
            _ => Operand::Constant(Constant::Null),
        };
        self.call_support_raw("_asyncReturn", vec![v, completer]);
        self.terminate_raw(Operand::Constant(Constant::Null));
    }

    /// `await e` (especificação §17.34): suspende até o `Future` completar.
    pub fn lower_await(&mut self, ast: &ast::Ast, e: ast::ExprId, span: Span) -> Operand {
        if !self.async_state.as_ref().is_some_and(|e| e.ty != BodyType::SyncStar) {
            return self.unsupported("await fora de função async", span);
        }
        let v = self.lower_expr(ast, e);
        self.await_value(v, span)
    }

    /// `await` de um valor já avaliado (o de `lower_await`, e os do
    /// `await for`).
    pub fn await_value(&mut self, v: Operand, span: Span) -> Operand {
        if !self.async_state.as_ref().is_some_and(|e| e.ty != BodyType::SyncStar) {
            return self.unsupported("await fora de função async", span);
        }
        let v = if matches!(self.operand_type(&v), Type::Void) {
            Operand::Constant(Constant::Null)
        } else {
            self.coerce(v, Type::Ref)
        };
        if self.is_terminated() {
            return Operand::Constant(Constant::Null);
        }
        let st = self.async_state.as_ref().expect("corpo async");
        let (frame, fn_body) = (st.frame.clone(), st.body.clone());
        let k = st.resumptions.len() as i64 + 1;
        // A posição do `await` (o `fileOffset` dele): a do quadro de quem
        // espera aqui, no rastro.
        let position = self.position.unwrap_or((0, 0));
        if let Some(e) = self.async_state.as_mut() {
            e.waits.push((k, position));
        }
        self.store_position(frame, Q_STATE, Operand::Constant(Constant::Int(k)));
        self.call_support("_asyncAwait", vec![v, fn_body], span);
        let suspends = self.current_block;
        self.terminate_raw(Operand::Constant(Constant::Null));
        let resumption = self.register_resumption(k, suspends);
        let st = self.async_state.as_ref().expect("corpo async");
        let (code, result) = (st.code.clone(), st.result.clone());
        self.set_block(resumption);
        let origin = self.throw_if_error(code, result.clone(), span);
        // O valor retomado é o parâmetro `result`, que cada entrada do
        // corpo troca: sai dele uma definição SSA própria, aqui, para que a
        // passada 2 (`save_live`) o leve ao quadro quando ele atravessa
        // outro ponto de suspensão (`[await a, await b]`, `f(await a, await
        // b)`, `await a + await b`, `'${await a}${await b}'`…). Os
        // parâmetros do corpo ficam de fora daquela passada.
        self.emit(
            Instruction::Phi {
                incoming: vec![(origin, result)],
                ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// Lança, no ponto do `await`, o erro de um `_ErroAssincrono` com o
    /// rastro dele.
    fn throw_async_error(&mut self, val_ref: Operand, span: Span) {
        let fields = self.async_error_fields();
        let Some((ve, vr)) = fields else {
            self.unsupported("classe `_ErroAssincrono` do async_patch ausente", span);
            return;
        };
        let error = self.read_field(val_ref.clone(), ve, span);
        let stack_trace = self.read_field(val_ref, vr, span);
        self.emit_throw_value(error, Some(stack_trace));
        self.branch_to_handler();
    }

    /// Os campos `error` e `stack_trace` de `_ErroAssincrono`.
    fn async_error_fields(
        &self,
    ) -> Option<(dartforge_elements::model::VariableId, dartforge_elements::model::VariableId)> {
        let lib = self.ctx.program.libraries.iter().position(|l| l.uri == "dart:async")?;
        let sym = self.ctx.interner.lookup("_ErroAssincrono")?;
        let b = self.ctx.program.lookup(dartforge_elements::model::LibraryId(lib as u32), sym)?;
        let dartforge_elements::model::Element::Class(c) = b.getter? else {
            return None;
        };
        let class = &self.ctx.program.classes[c.0 as usize];
        let field = |n: &str| {
            class
                .fields
                .iter()
                .copied()
                .find(|v| self.ctx.symbol_name(self.ctx.program.variables[v.0 as usize].name) == n)
        };
        Some((field("erro")?, field("rastro")?))
    }

    /// Depois de deixar uma exceção pendente: o desvio de `emit_throw_op`
    /// (tratador corrente, `finally` ou saída).
    fn branch_to_handler(&mut self) {
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
fn function_types(func: &Function) -> HashMap<ValueId, Type> {
    let mut t = HashMap::new();
    for (v, _, ty) in &func.params {
        t.insert(*v, *ty);
    }
    for b in &func.blocks {
        for (v, inst, ty) in &b.instructions {
            t.insert(*v, crate::llvm::LlvmEmitter::result_type(inst, *ty));
        }
    }
    t
}

fn new_value(next: &mut u32) -> ValueId {
    let v = ValueId(*next);
    *next += 1;
    v
}

/// Instruções que leem a posição `slot` do quadro para o valor `dest`
/// na representação `ty`.
fn read_from_frame(frame: &Operand, slot: usize, ty: Type, dest: ValueId, next: &mut u32) -> Vec<(ValueId, Instruction, Type)> {
    let get = |ret: Type| Instruction::CallRuntime {
        name: "dartforge_object_get".to_string(),
        args: vec![(frame.clone(), Type::Ref), (Operand::Constant(Constant::Int(slot as i64)), Type::I64)],
        ret_ty: ret,
    };
    match ty {
        // A restauração de locais e SSA usa a mesma representação explícita
        // das leituras do cabeçalho. Só a preparação ARC escolhe o empréstimo
        // auditado; tracing conserva seu acesso físico pelo emissor.
        Type::Ref => vec![(dest, Instruction::GetField { object: frame.clone(), index: slot }, Type::Ref)],
        Type::I64 => vec![(dest, get(ty), ty)],
        Type::F64 => {
            let t = new_value(next);
            vec![
                (t, get(Type::I64), Type::I64),
                (dest, Instruction::Bitcast { op: Operand::Val(t), to: Type::F64 }, Type::F64),
            ]
        }
        Type::I1 => {
            let t = new_value(next);
            vec![
                (t, get(Type::I64), Type::I64),
                (dest, Instruction::ICmp(ICmpOp::Ne, Operand::Val(t), Operand::Constant(Constant::Int(0))), Type::I1),
            ]
        }
        Type::I8 => {
            let t = new_value(next);
            vec![
                (t, get(Type::I64), Type::I64),
                (dest, Instruction::Trunc { op: Operand::Val(t), from: Type::I64, to: Type::I8 }, Type::I8),
            ]
        }
        Type::Void | Type::Ptr => Vec::new(),
        // Vetores SIMD sem caixa só existem em funções síncronas
        // (`lower/simd.rs` não os produz num corpo assíncrono).
        Type::V4F32 | Type::V4I32 | Type::V2F64 => unreachable!("vetor SIMD sem caixa num quadro assíncrono"),
    }
}

#[cfg(test)]
mod ref_restoration_tests {
    use super::*;
    use crate::optimize::arc::*;

    #[test]
    fn ref_restored_from_frame_gets_keepalive_before_mutating_child() {
        let mut next = 2;
        let mut instructions = read_from_frame(&Operand::Val(ValueId(0)), 40, Type::Ref, ValueId(1), &mut next);
        instructions.push((ValueId(2), Instruction::SetField {
            object: Operand::Val(ValueId(1)), index: 2, value: Operand::Constant(Constant::Int(42)),
        }, Type::Void));
        let f = Function { symbol: "restaurar".into(), name: "restaurar".into(), debug_info: None,
            params: vec![(ValueId(0), "quadro".into(), Type::Ref)], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions: instructions,
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] };
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions.push(f.clone());
        m.arc_field_layouts.insert(123, vec![Type::Ref, Type::I64, Type::I64]);
        m.classes.push(ClassDef { id: 123, name: "Filho".into(), field_count: 3,
            vtable: vec![], to_string_symbol: None });
        let mut p = DartFunctionPlan::default();
        p.tokens.ret = TokenReturn::Owned;
        let mut plans = HashMap::from([("restaurar".into(), p)]);
        assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (1, 0));
        let reader = m.functions[0].blocks.iter().flat_map(|b| &b.instructions).find_map(|(v, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_ler_campo_ref_v1").then_some(*v)).unwrap();
        assert_eq!(plans["restaurar"].classes[&reader], Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(0)), scope: 0,
        });
        assert_eq!(plans["restaurar"].classes[&ValueId(1)], Ownership::Owned);
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains("i64 %v0, i64 40)"));
        assert!(ir.find("call void @dartforge_arc_retain").unwrap()
            < ir.find("call void @dartforge_arc_gravar_campo_escalar_conferido_v1").unwrap());
        assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
        // O mesmo acesso tipado é emitível em tracing; os tokens ARC surgem
        // somente na preparação explicitamente selecionada acima.
        let mut tracing = Module::new();
        tracing.functions.push(f);
        let mut without_plans = HashMap::new();
        let before = format!("{tracing:?}");
        assert_eq!(prepare_arc_dart_module(&mut tracing, &mut without_plans).unwrap(), (0, 0));
        assert_eq!(format!("{tracing:?}"), before);
        let ir = crate::llvm::LlvmEmitter::new(&tracing).emit_all();
        assert!(!ir.contains("call i64 @dartforge_arc_ler_campo_ref_v1"));
    }
}

/// Instruções que gravam `value` (na representação `ty`) na posição `slot`.
fn store_in_frame(frame: &Operand, slot: usize, val_ref: Operand, ty: Type, next: &mut u32) -> Vec<(ValueId, Instruction, Type)> {
    let mut exit = Vec::new();
    let (bits, is_ref) = match ty {
        Type::Ref => (val_ref, 1),
        Type::I64 => (val_ref, 0),
        Type::F64 => {
            let t = new_value(next);
            exit.push((t, Instruction::Bitcast { op: val_ref, to: Type::I64 }, Type::I64));
            (Operand::Val(t), 0)
        }
        Type::I1 | Type::I8 => {
            let t = new_value(next);
            exit.push((t, Instruction::ZExt { op: val_ref, from: ty, to: Type::I64 }, Type::I64));
            (Operand::Val(t), 0)
        }
        Type::Void | Type::Ptr => return exit,
        Type::V4F32 | Type::V4I32 | Type::V2F64 => unreachable!("vetor SIMD sem caixa num quadro assíncrono"),
    };
    exit.push((
        new_value(next),
        Instruction::CallRuntime {
            name: "dartforge_object_set".to_string(),
            args: vec![
                (frame.clone(), Type::Ref),
                (Operand::Constant(Constant::Int(slot as i64)), Type::I64),
                (bits, Type::I64),
                (Operand::Constant(Constant::Int(is_ref)), Type::I8),
            ],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    exit
}

/// Passada 1: cada `alloca` vira uma posição do quadro a partir de `base`.
/// Devolve quantas posições usou.
fn convert_allocas(func: &mut Function, frame: &Operand, base: usize, next: &mut u32) -> usize {
    let mut position: HashMap<ValueId, (usize, Type)> = HashMap::new();
    for b in &func.blocks {
        for (v, inst, _) in &b.instructions {
            if let Instruction::Alloca(t) = inst {
                let s = base + position.len();
                position.insert(*v, (s, *t));
            }
        }
    }
    if position.is_empty() {
        return 0;
    }
    let types = function_types(func);
    let type_of = |op: &Operand| match op {
        Operand::Val(v) => types.get(v).copied().unwrap_or(Type::I64),
        Operand::Constant(Constant::Int(_)) => Type::I64,
        Operand::Constant(Constant::Double(_)) => Type::F64,
        Operand::Constant(Constant::Bool(_)) => Type::I1,
        Operand::Constant(_) => Type::Ref,
    };
    for b in &mut func.blocks {
        let old_ones = std::mem::take(&mut b.instructions);
        for (v, inst, ty) in old_ones {
            match &inst {
                Instruction::Alloca(_) if position.contains_key(&v) => {}
                Instruction::Load { ptr: Operand::Val(p), ty: t } if position.contains_key(p) => {
                    let (s, _) = position[p];
                    b.instructions.extend(read_from_frame(frame, s, *t, v, next));
                }
                Instruction::Store { ptr: Operand::Val(p), val } if position.contains_key(p) => {
                    let (s, t) = position[p];
                    // O valor chega na representação do local (R6, já
                    // coagido pelo lowering); só a largura pode diferir.
                    let ty_gravar = if t == Type::Ref { Type::Ref } else { type_of(val) };
                    b.instructions.extend(store_in_frame(frame, s, val.clone(), ty_gravar, next));
                }
                _ => b.instructions.push((v, inst, ty)),
            }
        }
    }
    position.len()
}

/// Os valores lidos por uma instrução.
pub(crate) fn uses_of(inst: &Instruction) -> Vec<ValueId> {
    let mut u = Vec::new();
    let mut op = |o: &Operand| {
        if let Operand::Val(v) = o {
            u.push(*v);
        }
    };
    match inst {
        Instruction::Const(_) | Instruction::Alloca(_) | Instruction::LoadGlobal { .. } | Instruction::TearOff { .. } | Instruction::ConstArray(_) | Instruction::FunctionTable(_) => {}
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
        | Instruction::ArcCopy { value: a }
        | Instruction::ArcDrop { value: a }
        | Instruction::ArcMove { value: a }
        | Instruction::Trunc { op: a, .. }
        | Instruction::Bitcast { op: a, .. }
        | Instruction::Box { op: a, .. }
        | Instruction::Unbox { op: a, .. } => op(a),
        Instruction::AllocObject { fields, .. } => fields.iter().for_each(&mut op),
        Instruction::AllocList { elements } | Instruction::AllocRecord { elements } => {
            elements.iter().for_each(|(a, _)| op(a))
        }
        Instruction::ArcLoadStrong { slot: StrongSlot::Quadro { frame, .. } } => op(frame),
        Instruction::ArcLoadStrong { slot: StrongSlot::Global { .. } } => {},
        Instruction::ArcStoreStrong { slot: StrongSlot::Global { .. }, value, .. } => op(value),
        Instruction::ArcStoreStrong { slot: StrongSlot::Quadro { frame, .. }, value, .. } => {
            op(frame);
            op(value);
        }
        Instruction::AllocCell { value } => op(value),
        Instruction::AllocEnv { values } => values.iter().for_each(&mut op),
        Instruction::ConcatStrings { parts } => parts.iter().for_each(&mut op),
        Instruction::AllocClosure { env, .. } | Instruction::AllocClosureTyped { env, .. } => op(env),
        Instruction::TypedCall { alvo, args, .. } => {
            op(alvo);
            args.iter().for_each(|(a, _)| op(a));
        }
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
        Instruction::CallClosure { closure, args, tuple_types, .. } => {
            op(closure);
            args.iter().for_each(&mut op);
            op(tuple_types);
        }
        Instruction::CallRuntime { args, .. } => args.iter().for_each(|(a, _)| op(a)),
        Instruction::IsClass { object, .. } => op(object),
        Instruction::StoreGlobal { val, .. } => op(val),
        Instruction::Phi { .. } => {}
        Instruction::LoadIndexed { base, index } => {
            op(base);
            op(index);
        }
        Instruction::CallSelector { recv, args, tuple_types, .. } => {
            op(recv);
            args.iter().for_each(&mut op);
            op(tuple_types);
        }
        Instruction::CallClosureForward { closure, args, desc, .. } => {
            op(closure);
            op(args);
            op(desc);
        }
        Instruction::CallSelectorForward { recv, args, desc, .. } => {
            op(recv);
            op(args);
            op(desc);
        }
        Instruction::NativeCall { alvo, args, .. } => {
            op(alvo);
            args.iter().for_each(|(a, _)| op(a));
        }
        Instruction::NativeLoad { address, index: idx_value, .. } => {
            op(address);
            op(idx_value);
        }
        Instruction::NativeWrite { address, index: idx_value, value: val_ref, .. } => {
            op(address);
            op(idx_value);
            op(val_ref);
        }
        Instruction::CompositeNativeCall { alvo, args, dest, .. } => {
            op(alvo);
            args.iter().for_each(|(a, _)| op(a));
            dest.iter().for_each(&mut op);
        }
        Instruction::Simd { args, .. } => args.iter().for_each(&mut op),
    }
    u
}

/// Troca cada uso de `de` por `to` numa instrução (sem os `phi`).
fn replace_uses(inst: &mut Instruction, swap: &dyn Fn(ValueId) -> Option<ValueId>) {
    let t = |o: &mut Operand| {
        if let Operand::Val(v) = o
            && let Some(n) = swap(*v)
        {
            *v = n;
        }
    };
    match inst {
        Instruction::Const(_) | Instruction::Alloca(_) | Instruction::LoadGlobal { .. } | Instruction::TearOff { .. } | Instruction::ConstArray(_) | Instruction::FunctionTable(_) | Instruction::Phi { .. } => {}
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
        | Instruction::ArcCopy { value: a }
        | Instruction::ArcDrop { value: a }
        | Instruction::ArcMove { value: a }
        | Instruction::Trunc { op: a, .. }
        | Instruction::Bitcast { op: a, .. }
        | Instruction::Box { op: a, .. }
        | Instruction::Unbox { op: a, .. } => t(a),
        Instruction::AllocObject { fields, .. } => fields.iter_mut().for_each(t),
        Instruction::AllocList { elements } | Instruction::AllocRecord { elements } => {
            elements.iter_mut().for_each(|(a, _)| t(a))
        }
        Instruction::ArcLoadStrong { slot: StrongSlot::Quadro { frame, .. } } => t(frame),
        Instruction::ArcLoadStrong { slot: StrongSlot::Global { .. } } => {},
        Instruction::ArcStoreStrong { slot: StrongSlot::Global { .. }, value, .. } => t(value),
        Instruction::ArcStoreStrong { slot: StrongSlot::Quadro { frame, .. }, value, .. } => {
            t(frame);
            t(value);
        }
        Instruction::AllocCell { value } => t(value),
        Instruction::AllocEnv { values } => values.iter_mut().for_each(t),
        Instruction::ConcatStrings { parts } => parts.iter_mut().for_each(t),
        Instruction::AllocClosure { env, .. } | Instruction::AllocClosureTyped { env, .. } => t(env),
        Instruction::TypedCall { alvo, args, .. } => {
            t(alvo);
            args.iter_mut().for_each(|(a, _)| t(a));
        }
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
        Instruction::CallClosure { closure, args, tuple_types, .. } => {
            t(closure);
            args.iter_mut().for_each(t);
            t(tuple_types);
        }
        Instruction::CallRuntime { args, .. } => args.iter_mut().for_each(|(a, _)| t(a)),
        Instruction::IsClass { object, .. } => t(object),
        Instruction::StoreGlobal { val, .. } => t(val),
        Instruction::LoadIndexed { base, index } => {
            t(base);
            t(index);
        }
        Instruction::CallSelector { recv, args, tuple_types, .. } => {
            t(recv);
            args.iter_mut().for_each(t);
            t(tuple_types);
        }
        Instruction::CallClosureForward { closure, args, desc, .. } => {
            t(closure);
            t(args);
            t(desc);
        }
        Instruction::CallSelectorForward { recv, args, desc, .. } => {
            t(recv);
            t(args);
            t(desc);
        }
        Instruction::NativeCall { alvo, args, .. } => {
            t(alvo);
            args.iter_mut().for_each(|(a, _)| t(a));
        }
        Instruction::NativeLoad { address, index: idx_value, .. } => {
            t(address);
            t(idx_value);
        }
        Instruction::NativeWrite { address, index: idx_value, value: val_ref, .. } => {
            t(address);
            t(idx_value);
            t(val_ref);
        }
        Instruction::CompositeNativeCall { alvo, args, dest, .. } => {
            t(alvo);
            args.iter_mut().for_each(|(a, _)| t(a));
            if let Some(d) = dest { t(d) }
        }
        Instruction::Simd { args, .. } => args.iter_mut().for_each(t),
    }
}

pub(crate) fn terminator_uses(term: &Terminator) -> Vec<ValueId> {
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

fn replace_in_terminator(term: &mut Terminator, swap: &dyn Fn(ValueId) -> Option<ValueId>) {
    let t = |o: &mut Operand| {
        if let Operand::Val(v) = o
            && let Some(n) = swap(*v)
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

pub(crate) fn successors(term: &Terminator) -> Vec<BlockId> {
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
fn save_live(
    func: &mut Function,
    frame: &Operand,
    base: usize,
    suspensions: &[(BlockId, BlockId)],
    entry: BlockId,
    next: &mut u32,
) -> usize {
    if suspensions.is_empty() {
        return 0;
    }
    let idx: HashMap<BlockId, usize> = func.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    let n = func.blocks.len();
    let mut defs: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    let mut uses: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    // Usos de `phi` atribuídos ao fim do bloco de origem.
    let mut end_uses: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    for (i, b) in func.blocks.iter().enumerate() {
        for (v, inst, _) in &b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (origin, o) in incoming {
                    if let (Operand::Val(x), Some(&j)) = (o, idx.get(origin)) {
                        end_uses[j].insert(*x);
                    }
                }
            } else {
                for u in uses_of(inst) {
                    if !defs[i].contains(&u) {
                        uses[i].insert(u);
                    }
                }
            }
            defs[i].insert(*v);
        }
        for u in terminator_uses(&b.terminator) {
            if !defs[i].contains(&u) {
                uses[i].insert(u);
            }
        }
    }
    let mut succ: Vec<Vec<usize>> = func
        .blocks
        .iter()
        .map(|b| successors(&b.terminator).iter().filter_map(|s| idx.get(s).copied()).collect())
        .collect();
    for (a, r) in suspensions {
        if let (Some(&ia), Some(&ir)) = (idx.get(a), idx.get(r)) {
            succ[ia].push(ir);
        }
    }
    let mut live_in: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    let mut changed = true;
    while changed {
        changed = false;
        for i in (0..n).rev() {
            let mut out: HashSet<ValueId> = end_uses[i].clone();
            for &s in &succ[i] {
                out.extend(live_in[s].iter().copied());
            }
            let mut inn: HashSet<ValueId> = uses[i].clone();
            for x in out {
                if !defs[i].contains(&x) {
                    inn.insert(x);
                }
            }
            if inn.len() != live_in[i].len() {
                live_in[i] = inn;
                changed = true;
            }
        }
    }
    let params: HashSet<ValueId> = func.params.iter().map(|p| p.0).collect();
    let from_entry = idx.get(&entry).map(|&i| defs[i].clone()).unwrap_or_default();
    let mut save: Vec<ValueId> = suspensions
        .iter()
        .filter_map(|(_, r)| idx.get(r))
        .flat_map(|&i| live_in[i].iter().copied())
        .filter(|v| !params.contains(v) && !from_entry.contains(v))
        .collect();
    let types = function_types(func);
    save.retain(|v| !matches!(types.get(v), Some(Type::Void | Type::Ptr) | None));
    save.sort_by_key(|v| v.0);
    save.dedup();
    if save.is_empty() {
        return 0;
    }
    let position: HashMap<ValueId, (usize, Type)> = save
        .iter()
        .enumerate()
        .map(|(k, v)| (*v, (base + k, types[v])))
        .collect();

    // A guarda de pendência no fim de um bloco (chamada, leitura da
    // pendência, comparação e desvio): a forma que a preparação ARC
    // reconhece. Nada entra entre a chamada e o desvio.
    fn guard(b: &BasicBlock) -> Option<(usize, BlockId)> {
        let n = b.instructions.len();
        let Terminator::CondBranch { cond: Operand::Val(c), else_block, .. } = &b.terminator else { return None };
        let [(p, Instruction::CallRuntime { name, .. }, _), (ic, Instruction::ICmp(ICmpOp::Ne, Operand::Val(pv), _), _)] =
            b.instructions.get(n.checked_sub(2)?..)? else { return None };
        (name == "dartforge_exception_pending" && pv == p && ic == c && n >= 3).then_some((n - 3, *else_block))
    }
    // Gravações do resultado de uma chamada guardada: vão para a aresta de
    // sucesso (bloco novo), depois da guarda.
    let mut on_edges: Vec<(BlockId, BlockId, Vec<(ValueId, Instruction, Type)>)> = Vec::new();
    for b in &mut func.blocks {
        let saved = guard(b);
        // Releituras no fim do bloco para as entradas de `phi` dos sucessores
        // são feitas numa segunda volta (abaixo).
        let old_ones = std::mem::take(&mut b.instructions);
        let n_phis = old_ones.iter().take_while(|(_, i, _)| matches!(i, Instruction::Phi { .. })).count();
        let mut new_ones: Vec<(ValueId, Instruction, Type)> = Vec::with_capacity(old_ones.len());
        let mut after_phis: Vec<(ValueId, Instruction, Type)> = Vec::new();
        for (k, (v, mut inst, ty)) in old_ones.into_iter().enumerate() {
            if !matches!(inst, Instruction::Phi { .. }) {
                // Releitura de cada valor guardado que a instrução usa.
                let mut swaps: HashMap<ValueId, ValueId> = HashMap::new();
                for u in uses_of(&inst) {
                    if let Some(&(s, t)) = position.get(&u)
                        && !swaps.contains_key(&u)
                    {
                        let nv = new_value(next);
                        new_ones.extend(read_from_frame(frame, s, t, nv, next));
                        swaps.insert(u, nv);
                    }
                }
                if !swaps.is_empty() {
                    replace_uses(&mut inst, &|x| swaps.get(&x).copied());
                }
            }
            let is_phi = matches!(inst, Instruction::Phi { .. });
            new_ones.push((v, inst, ty));
            if let Some(&(s, t)) = position.get(&v) {
                let writes = store_in_frame(frame, s, Operand::Val(v), t, next);
                if is_phi {
                    after_phis.extend(writes);
                } else if let Some((idx_value, success)) = saved
                    && idx_value == k
                {
                    on_edges.push((b.id, success, writes));
                } else {
                    new_ones.extend(writes);
                }
            }
            if k + 1 == n_phis {
                new_ones.append(&mut after_phis);
            }
        }
        new_ones.append(&mut after_phis);
        // O terminador.
        let mut swaps: HashMap<ValueId, ValueId> = HashMap::new();
        for u in terminator_uses(&b.terminator) {
            if let Some(&(s, t)) = position.get(&u)
                && !swaps.contains_key(&u)
            {
                let nv = new_value(next);
                new_ones.extend(read_from_frame(frame, s, t, nv, next));
                swaps.insert(u, nv);
            }
        }
        if !swaps.is_empty() {
            replace_in_terminator(&mut b.terminator, &|x| swaps.get(&x).copied());
        }
        b.instructions = new_ones;
    }
    let mut next_block = func.blocks.iter().map(|b| b.id.0).max().map_or(0, |m| m + 1);
    for (origin, success, writes) in on_edges {
        let edge = BlockId(next_block);
        next_block += 1;
        for b in &mut func.blocks {
            if b.id == origin
                && let Terminator::CondBranch { else_block, .. } = &mut b.terminator
            {
                *else_block = edge;
            }
            if b.id == success {
                for (_, inst, _) in &mut b.instructions {
                    if let Instruction::Phi { incoming, .. } = inst {
                        for (de, _) in incoming.iter_mut().filter(|(de, _)| *de == origin) {
                            *de = edge;
                        }
                    }
                }
            }
        }
        func.blocks.push(BasicBlock { id: edge, instructions: writes, terminator: Terminator::Branch(success) });
    }
    // Entradas de `phi` com valor guardado: releitura no fim do bloco de
    // origem (antes do terminador dele; antes da chamada guardada, que não
    // grava no quadro).
    let mut rereads: Vec<(BlockId, ValueId, ValueId)> = Vec::new(); // (origem, antigo, novo)
    for b in &mut func.blocks {
        for (_, inst, _) in &mut b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (origin, o) in incoming.iter_mut() {
                    if let Operand::Val(x) = o
                        && position.contains_key(x)
                    {
                        let nv = new_value(next);
                        rereads.push((*origin, *x, nv));
                        *x = nv;
                    }
                }
            }
        }
    }
    for (origin, old, new) in rereads {
        if let Some(b) = func.blocks.iter_mut().find(|b| b.id == origin) {
            let (s, t) = position[&old];
            let read = read_from_frame(frame, s, t, new, next);
            match guard(b) {
                Some((idx_value, _)) => {
                    b.instructions.splice(idx_value..idx_value, read);
                }
                None => b.instructions.extend(read),
            }
        }
    }
    save.len()
}
