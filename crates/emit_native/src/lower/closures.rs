//! Closures (P1): expressão de função, função local, tear-offs e a chamada
//! de um valor função.
//!
//! **Convenção uniforme** (a do `dartdevc`/VM para chamadas dinâmicas: um
//! vetor de argumentos e um *arguments descriptor*): todo valor função é
//! chamado por uma **entrada** `i64 @<corpo>$ent(i64 closure, ptr args, ptr
//! desc)`. `args` has os arguments (`Ref`), os posicionais primeiro e os
//! nomeados depois, na ordem do descritor; `desc` é
//! `[n_posicionais, n_nomeados, hash(name)…]` com os nomes ordenados. A
//! entrada confere a aridade contra a assinatura da função
//! (`dartforge_args_casam`, que devolve 0 quando não casa — então lança
//! `NoSuchMethodError`, como a VM), preenche os padrões dos opcionais
//! ausentes e chama o **corpo** com os parâmetros na ordem da declaração.
//!
//! O corpo de uma closure é `i64 @<símbolo>(i64 env, i64 p0, …)`, com todos
//! os parâmetros e o retorno `Ref` (a convenção uniforme; o corpo é inferido
//! por `crates/types`, e [`FnBuilder::resolve_by_name`] cobre o nome sem
//! resolução gravada). O ambiente guarda, nesta ordem, `this`
//! (quando a closure nasce num membro de instância) e as variáveis livres —
//! o handle da célula das que moram numa (`capture.rs`), ou a cópia do valor.
//!
//! A closure é `AllocClosure { código, env }`; o código é o endereço da
//! entrada uniforme (`ptrtoint`, no emissor), válido entre o módulo do SDK e
//! o do programa. O tear-off de função de topo ou
//! estática é canônico (`TearOff`: o mesmo handle sempre, `identical(f, f)`);
//! o de método de instância é uma closure nova com o receptor no ambiente, e
//! a entrada dele chama o membro com o despacho do receptor.

use super::capture::{self, Root};
use super::fn_builder::FnBuilder;
use super::locals::Modo;
use super::members::Evaluated;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, FunctionBody, FunctionId, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::table::TypeId;

/// Hash estável do nome de um argumento nomeado (FNV-1a de 64 bits sobre o
/// UTF-8): o mesmo em qualquer módulo, o que o descritor precisa para que uma
/// biblioteca compilada à parte chame outra.
pub fn hash_name(name: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h as i64
}

/// De onde vem o valor padrão de um parâmetro opcional ausente.
#[derive(Clone, Copy)]
pub enum Padrao {
    /// Sem padrão: null.
    Nenhum,
    /// A expressão escrita na declaração da closure.
    Expr(UnitId, ExprId),
    /// O padrão do parâmetro `i` da função `fid` (o de `super.x` inclusive).
    OfFunction(usize, usize),
}

/// Um parâmetro como a entrada uniforme o vê.
#[derive(Clone)]
pub struct EntryParam {
    pub name: Option<String>,
    pub kind: ParameterKind,
    pub required: bool,
    pub pattern: Padrao,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Pré-passe das células (antes de declarar os parâmetros).
    /// Com as funções locais diretas (`direct_functions.rs`), salvo num
    /// corpo `async`/gerador (`without_directs`).
    pub fn prepare_captures(&mut self, ast: &ast::Ast, root: Root) {
        // Os `late` de fora visíveis aqui (capturas já ligadas): uma função
        // que os lê não pode ser direta (a marca de inicialização não vai
        // pelo parâmetro).
        let outside_lates: std::collections::HashSet<SymbolId> = self
            .scopes
            .iter()
            .flat_map(|e| e.iter())
            .filter(|(_, l)| l.late.is_some())
            .map(|(s, _)| *s)
            .collect();
        let c = capture::analyze_with(self.ctx, self.unit_id, ast, root, !self.without_directs, &outside_lates);
        self.cells = c.cells;
        self.assigned = c.assigned;
        self.allowed_directs = c.directs;
    }

    /// Nome do corpo de uma closure dentro desta função: `$clo<k>` para as
    /// anônimas (k conta só as anônimas, na ordem do texto) e `$<name>` para
    /// as funções locais (com `$<k>` se o nome se repete).
    fn closure_name(&mut self, name: Option<SymbolId>) -> String {
        match name {
            None => {
                let k = self.n_closures;
                self.n_closures += 1;
                format!("{}$clo{k}", self.func.symbol)
            }
            Some(s) => {
                let base = format!(
                    "{}${}",
                    self.func.symbol,
                    super::sanitize_symbol(self.ctx.symbol_name(s))
                );
                let mut name = base.clone();
                let mut k = 1;
                while !self.local_names.insert(name.clone()) {
                    name = format!("{base}${k}");
                    k += 1;
                }
                name
            }
        }
    }

    /// Os parâmetros de uma expressão de função, para a entrada uniforme.
    fn params_da_closure(&self, f: &ast::Function) -> Vec<EntryParam> {
        let unit = self.unit_id;
        f.parameters
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|p| EntryParam {
                name: p.name.map(|n| self.ctx.symbol_name(n.sym).to_string()),
                kind: p.kind,
                required: p.kind == ParameterKind::Required || p.required,
                pattern: p.default_value.map_or(Padrao::Nenhum, |e| Padrao::Expr(unit, e)),
            })
            .collect()
    }

    /// A ABI tipada de um tipo de função: as representações dos posicionais
    /// e do retorno (`void` volta como `Ref`, o null), e o código delas. Só
    /// para tipos sem parâmetros de tipo, opcionais nem nomeados, até 12
    /// posicionais.
    pub(super) fn type_abi(&self, t: TypeId) -> Option<(Vec<Type>, Type, i64)> {
        let dartforge_types::table::Type::Function { type_params, ret, positional, optional, named, nullable: false } =
            self.ctx.table.get(t)
        else {
            return None;
        };
        if !type_params.is_empty() || !optional.is_empty() || !named.is_empty() || positional.len() > 12 {
            return None;
        }
        let repr = |t: TypeId| match self.ctx.to_hir_type(t) {
            r @ (Type::I64 | Type::F64 | Type::I1) => r,
            _ => Type::Ref,
        };
        let params: Vec<Type> = positional.iter().map(|&p| repr(p)).collect();
        let ret = repr(*ret);
        let digit = |t: Type| match t {
            Type::I64 => 2,
            Type::F64 => 3,
            Type::I1 => 4,
            _ => 1,
        };
        let mut abi: i64 = 1;
        for &t in params.iter().chain(std::iter::once(&ret)) {
            abi = abi * 5 + digit(t);
        }
        Some((params, ret, abi))
    }

    /// A ABI tipada de uma closure síncrona só com posicionais obrigatórios,
    /// pelo tipo estático dela (o do contexto, já inferido).
    fn closure_abi(&self, f: &ast::Function, typ: Option<TypeId>) -> Option<(Vec<Type>, Type, i64)> {
        let params = f.parameters.as_deref().unwrap_or(&[]);
        if f.modifier != AsyncModifier::None
            || !f.type_params.is_empty()
            || params.iter().any(|p| p.kind != ParameterKind::Required)
        {
            return None;
        }
        let abi = self.type_abi(typ?)?;
        (abi.0.len() == params.len()).then_some(abi)
    }

    /// `(params) => e`, `(params) { … }` ou o valor de uma função local.
    /// `ty`: o tipo estático da expressão da closure (o de contexto), de
    /// onde sai o tipo do valor de um corpo `async`/gerador.
    pub fn lower_closure(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span, typ: Option<TypeId>) -> Operand {
        let f = ast.function(fid);
        // Variáveis livres que são locais visíveis aqui.
        let (free_set, _) = capture::free_set(self.ctx, self.unit_id, ast, fid);
        let captures: Vec<(SymbolId, super::locals::Local)> = free_set
            .into_iter()
            .filter_map(|s| self.lookup_local(s).map(|l| (s, l)))
            .collect();
        let with_this = self.this_param.is_some();
        let base = usize::from(with_this);

        // O nome leva a impressão digital do que o corpo espera do ambiente
        // (as capturas, na ordem, com o modo e o tipo; o `this`; a tupla de
        // tipos) e dos parâmetros: numa recarga do JIT, uma closure viva só
        // passa a executar o corpo novo de mesmo nome se ele lê o ambiente
        // dela do mesmo jeito. Uma closure nova inserida antes, ou capturas
        // que mudam, dão outro nome, e as closures vivas seguem no corpo em
        // que foram criadas.
        let mut shape = format!("{with_this}|{}|{}|", self.type_tuple.is_some(), f.type_params.len());
        // Closure genérica (`T f<T>()` local, `<T>(x) => …`): os argumentos de
        // tipo dela vêm no slot oculto da chamada e se juntam, na tupla do
        // corpo, depois dos de quem a criou (`M<n_outside + i>`).
        let n_outside = self.tuple_size();
        let own_ids: Vec<dartforge_types::table::TypeParamId> = match typ.map(|t| self.ctx.table.get(t)) {
            Some(dartforge_types::table::Type::Function { type_params, .. }) if type_params.len() == f.type_params.len() => {
                type_params.to_vec()
            }
            _ => Vec::new(),
        };
        let own_set: Vec<(SymbolId, dartforge_types::table::TypeParamId, usize)> = f
            .type_params
            .iter()
            .enumerate()
            .map(|(i, tp)| {
                let id = own_ids.get(i).copied().unwrap_or(dartforge_types::table::TypeParamId(u32::MAX));
                (tp.name.sym, id, n_outside + i)
            })
            .collect();
        let generic = !own_set.is_empty();
        // Ambiente direto: com um valor só no ambiente — o `this` sem
        // capturas, ou uma captura `Ref` (ou o handle da célula dela) sem
        // `this` —, a closure guarda o próprio valor no lugar do ambiente, e
        // o corpo o recebe como o parâmetro `env`: sem o objeto do ambiente
        // (uma alocação e uma indireção a menos; a VM guarda a captura no
        // `Context` só quando há mais de uma). Fora: corpo `async`/gerador
        // (o quadro guarda o ambiente), genérica, tupla de tipos, `late`.
        let direct = f.modifier == AsyncModifier::None
            && !generic
            && self.type_tuple.is_none()
            && match (with_this, &captures[..]) {
                (true, []) => true,
                (false, [(_, l)]) => {
                    l.late.is_none()
                        && (matches!(l.mode, Modo::Cell(_) | Modo::Environment { cell: true, .. }) || l.ty == Type::Ref)
                }
                _ => false,
            };
        if direct {
            shape.push_str("direto;");
        }
        for (sym, l) in &captures {
            let cell = matches!(l.mode, Modo::Cell(_) | Modo::Environment { cell: true, .. });
            shape.push_str(&format!("{}:{cell}:{:?};", self.ctx.symbol_name(*sym), l.ty));
        }
        for p in f.parameters.as_deref().unwrap_or(&[]) {
            let name = p.name.map_or("", |n| self.ctx.symbol_name(n.sym));
            shape.push_str(&format!("{name}:{:?}:{};", p.kind, p.required));
        }
        // A ABI tipada do corpo: parâmetros e retorno nas representações do
        // tipo da closure; entra na impressão digital, e uma closure viva
        // segue no corpo da ABI em que nasceu.
        let abi = self.closure_abi(f, typ);
        if let Some((ps, r, code)) = &abi {
            shape.push_str(&format!("abi:{code}:{ps:?}:{r:?};"));
        }
        let (reprs, ret_repr) = match &abi {
            Some((ps, r, _)) => (Some(ps.clone()), *r),
            None => (None, Type::Ref),
        };
        let symbol = format!(
            "{}$e{:08x}",
            self.closure_name(f.name.map(|n| n.sym)),
            (hash_name(&shape) as u64) as u32
        );
        let readable = f
            .name
            .map_or_else(|| "<closure>".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());

        // --- corpo ---------------------------------------------------------
        let mut b = FnBuilder::new(self.ctx, self.unit_id, symbol.clone(), readable.clone(), ret_repr);
        // O rastro simbólico (§13.14): o token da closure, a entrada pela
        // qual o runtime a acha, o elo com quem espera e o oculto herdado.
        if self.ctx.stack_trace {
            b.stack_trace.token = self.ctx.line_and_column(self.unit_id, f.name.map_or(f.span.start, |n| n.span.start));
            b.stack_trace.marks |= self.stack_trace.marks & crate::hir::stack_trace_marks::HIDDEN;
            b.stack_trace.closure_entry = Some(format!("{symbol}$ent"));
            b.stack_trace.chain_link = captures
                .iter()
                .enumerate()
                .find(|(_, (_, l))| l.offset.is_some_and(|o| self.wait_link_at(ast, o)))
                .map(|(i, (_, l))| crate::hir::WaitLink {
                    index: if direct { 0 } else { base + i },
                    direct,
                    cell: matches!(l.mode, Modo::Cell(_) | Modo::Environment { cell: true, .. }),
                });
        }
        let env_b = Operand::Val(b.add_param("env".to_string(), Type::Ref));
        if direct && with_this {
            b.this_param = Some(env_b.clone());
            b.finalizable_this = self.finalizable_this;
            b.enclosing_class = self.enclosing_class;
        } else if direct {
            let (sym, l) = &captures[0];
            let cell = matches!(l.mode, Modo::Cell(_) | Modo::Environment { cell: true, .. });
            let mode = if cell {
                let ptr = b.entry_alloca(Type::Ref);
                b.emit(Instruction::Store { ptr: ptr.clone(), val: env_b.clone() }, Type::Void);
                Modo::Cell(ptr)
            } else {
                Modo::Value(env_b.clone())
            };
            b.bind_local_as(*sym, super::locals::Local { mode, ty: l.ty, static_type: l.static_type, finalizable: l.finalizable, scope: 0, offset: None, late: None });
        } else if with_this {
            let t = b.emit(
                Instruction::EnvGet {
                    env: env_b.clone(),
                    index: 0,
                },
                Type::Ref,
            );
            b.this_param = Some(t);
            b.finalizable_this = self.finalizable_this;
            b.enclosing_class = self.enclosing_class;
        }
        for (i, (sym, l)) in captures.iter().enumerate().filter(|_| !direct) {
            let cell = matches!(
                l.mode,
                Modo::Cell(_) | Modo::Environment { cell: true, .. }
            );
            // Um vetor SIMD capturado vai no ambiente na caixa (a posição
            // tem 64 bits).
            let ty = if l.ty.is_vector() { Type::Ref } else { l.ty };
            b.bind_env(*sym, env_b.clone(), base + i, cell, ty, l);
        }
        // RTI: a closure vê as variáveis de tipo de quem a cria (`T` da
        // função genérica em volta: a tupla vai no fim do ambiente).
        b.function_type_params = self.function_type_params.clone();
        b.local_params = self.local_params.clone();
        b.local_params.extend(own_set.iter().copied());
        b.this_extension = self.this_extension;
        b.this_ext_type = self.this_ext_type;
        b.member_class = self.member_class;
        b.class_by_tuple = self.class_by_tuple;
        if self.class_by_tuple {
            // Numa fábrica não há `this`, mas `T` é o da classe.
            b.enclosing_class = self.enclosing_class;
        }
        if self.type_tuple.is_some() && !generic {
            let t = b.emit(
                Instruction::EnvGet {
                    env: env_b.clone(),
                    index: base + captures.len(),
                },
                Type::I64,
            );
            b.type_tuple = Some(t);
        }
        let params = f.parameters.as_deref().unwrap_or(&[]);
        b.prepare_captures(
            ast,
            Root {
                parameters: params,
                body: Some(&f.body),
                initializers: &[],
            },
        );
        b.open_scope();
        for (i, p) in params.iter().enumerate() {
            let name = p
                .name
                .map_or_else(|| "arg".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());
            let ty = reprs.as_ref().map_or(Type::Ref, |r| r[i]);
            let vid = b.add_param_dart(name, ty);
            if let Some(n) = p.name {
                b.declare_variable(n.sym, n.span.start as usize, ty, Operand::Val(vid));
            }
        }
        // A tupla juntada (a de fora e a da chamada) chega ao corpo da
        // closure genérica como o último parâmetro.
        if generic {
            let t = b.add_param_rti("$tipos".to_string());
            b.type_tuple = Some(Operand::Val(t));
        }
        if f.modifier != AsyncModifier::None {
            // P6: closure `async`, `sync*` ou `async*` — o corpo vira máquina
            // de estados. O tipo do valor/elemento sai da anotação de retorno
            // ou do tipo estático da closure; sem nenhum, `dynamic`.
            use super::async_sm::AsyncReturn;
            let ret_val = f.return_type.map(AsyncReturn::Annotation).or_else(|| {
                typ.and_then(|t| match self.ctx.table.get(t) {
                    dartforge_types::table::Type::Function { ret, .. } => Some(AsyncReturn::Tipo(*ret)),
                    _ => None,
                })
            });
            b.lower_async_body(ast, params, &f.body, span, ret_val, super::async_sm::body_type(f.modifier));
        } else {
            match &f.body {
                FunctionBody::Block(s) => b.lower_stmt(ast, *s),
                FunctionBody::Expression(e) => {
                    let r = b.lower_expr(ast, *e);
                    if !b.is_terminated() {
                        let r = b.coerce(r, ret_repr);
                        b.terminate(Terminator::Return(Some(r)));
                    }
                }
                _ => {}
            }
        }
        // O corpo da closure é um corpo Dart: o retorno Ref é Owned, como o
        // de uma função (`lower_function`).
        if self.ctx.arc_memory && b.func.return_ty == Type::Ref {
            b.dart_ref_returns.insert(symbol.clone());
        }
        self.absorb(b);

        // --- entrada uniforme ----------------------------------------------
        let infos = self.params_da_closure(f);
        let symbol_ent = format!("{symbol}$ent");
        let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), readable, Type::Ref);
        let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
        let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
        e.declare_arc_uniform_return();
        let env_e = e.closure_context(clo);
        // Os argumentos de tipo que a closure genérica usa quando a chamada
        // não passa nenhum (chamada dinâmica): os limites escritos, como a
        // instanciação pelos limites; `dynamic` sem limite ou quando o limite
        // depende de variáveis de tipo.
        let pattern = generic.then(|| {
            let mut text = String::from("L<");
            for (i, (_, id, _)) in own_set.iter().enumerate() {
                if i > 0 {
                    text.push(',');
                }
                let limit = (id.0 != u32::MAX)
                    .then(|| self.ctx.table.param(*id))
                    .filter(|d| d.explicito)
                    .map(|d| self.type_recipe(d.bound))
                    .filter(|r| !r.variables);
                text.push_str(limit.as_ref().map_or("D", |r| r.text.as_str()));
            }
            text.push('>');
            super::rti::Recipe { text, variables: false }
        });
        if let Some(vals) = e.unpack(&infos, args.clone(), desc.clone()) {
            let mut all = vec![env_e.clone()];
            // O corpo tipado recebe cada argumento na representação dele (o
            // desencaixe confere o tipo, como a entrada dinâmica da VM).
            for (i, v) in vals.into_iter().enumerate() {
                let ty = reprs.as_ref().map_or(Type::Ref, |r| r[i]);
                let v = e.coerce(v, ty);
                all.push(v);
            }
            if let Some(pattern) = &pattern {
                // A tupla da chamada está no slot depois dos argumentos.
                let own = e.slot_tuple(&args, &desc);
                let from_outside = if self.type_tuple.is_some() {
                    e.emit(Instruction::EnvGet { env: env_e.clone(), index: base + captures.len() }, Type::I64)
                } else {
                    Operand::Constant(Constant::Int(0))
                };
                let pattern = e.recipe_rti(pattern);
                let joined = e.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_rti_tupla_juntar".to_string(),
                        args: vec![
                            (from_outside, Type::I64),
                            (Operand::Constant(Constant::Int(n_outside as i64)), Type::I64),
                            (own, Type::I64),
                            (pattern, Type::I64),
                        ],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                all.push(joined);
            }
            if !e.is_terminated() {
                let r = e.emit_call_with_check(
                    Instruction::CallStatic {
                        symbol: symbol.clone(),
                        args: all,
                        ret_ty: ret_repr,
                    },
                    ret_repr,
                );
                let r = e.coerce(r, Type::Ref);
                if !e.is_terminated() {
                    e.terminate(Terminator::Return(Some(r)));
                }
            }
        }
        self.absorb(e);

        // --- criação ---------------------------------------------------------
        let mut values = Vec::with_capacity(base + captures.len());
        if let Some(t) = self.this_param.clone() {
            values.push(t);
        }
        for (_, l) in &captures {
            let v = match self.local_cell(l) {
                Some(c) => c,
                None => self.read_local(l),
            };
            // O vetor SIMD sem caixa vai na caixa (`bind_env` com `Ref`).
            let v = if self.operand_type(&v).is_vector() { self.coerce(v, Type::Ref) } else { v };
            values.push(v);
        }
        if let Some(t) = self.type_tuple.clone() {
            // A tupla vai crua no contexto (`I64`), como o `EnvGet` do corpo a lê.
            let t = self.coerce(t, Type::I64);
            values.push(t);
        }
        let env = if direct {
            let v = values.pop().expect("o valor do ambiente direto");
            self.coerce(v, Type::Ref)
        } else {
            self.emit(Instruction::AllocEnv { values: values }, Type::Ref)
        };
        match abi {
            Some((_, _, code)) => self.emit(
                Instruction::AllocClosureTyped { code_symbol: symbol_ent, env, typed: symbol, abi: code, direct },
                Type::Ref,
            ),
            // Sem ABI tipada, o corpo tipado não é chamado (`abi` 0 não casa
            // com nenhuma chamada tipada): a entrada no lugar dele.
            None if direct => self.emit(
                Instruction::AllocClosureTyped { code_symbol: symbol_ent.clone(), env, typed: symbol_ent, abi: 0, direct },
                Type::Ref,
            ),
            None => self.emit(
                Instruction::AllocClosure {
                    code_symbol: symbol_ent,
                    env,
                },
                Type::Ref,
            ),
        }
    }

    /// O parâmetro declarado em `offset` tem `@pragma('vm:awaiter-link')`:
    /// a closure que o captura leva ao quadro de quem espera
    /// (`StackTraceUtils`, `TryGetAwaiterLink`).
    fn wait_link_at(&self, ast: &ast::Ast, offset: usize) -> bool {
        ast.functions.iter().any(|f| {
            f.parameters.iter().flatten().any(|p| {
                p.name.is_some_and(|n| n.span.start == offset)
                    && p.metadata.iter().any(|a| crate::source::pragma_with_string(self.ctx.interner, ast, a, "vm:awaiter-link"))
            })
        })
    }

    /// Entrega ao módulo (pelas `extra_functions` desta função) uma função
    /// construída à parte, com os diagnósticos dela.
    pub fn absorb(&mut self, mut b: FnBuilder<'a, 'c>) {
        b.close_stack_trace();
        self.errors.extend(b.errors);
        self.extra_globals.extend(b.extra_globals);
        self.dart_scalar_parameters.extend(b.dart_scalar_parameters);
        self.dart_rti_parameters.extend(b.dart_rti_parameters);
        self.dart_vector_parameters.extend(b.dart_vector_parameters);
        self.arc_uniform_returns.extend(b.arc_uniform_returns);
        self.dart_ref_returns.extend(b.dart_ref_returns);
        self.arc_rti_caches.extend(b.arc_rti_caches);
        self.arc_shared_functions.extend(b.arc_shared_functions);
        self.arc_direct_local_functions.extend(b.arc_direct_local_functions);
        self.extra_functions.push(b.func);
        self.extra_functions.extend(b.extra_functions);
    }

    /// `R f(params) { … }` local: o nome é uma variável que guarda a closure
    /// (numa célula quando a função chama a si mesma ou é capturada e ligada
    /// depois — `capture.rs` marca o nome como atribuído).
    pub fn declare_local_function(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span) {
        let f = ast.function(fid);
        let Some(name) = f.name else {
            return;
        };
        self.declare_variable(
            name.sym,
            name.span.start as usize,
            Type::Ref,
            Operand::Constant(Constant::Null),
        );
        let typ = self.literal_function_type(ast, fid, None);
        let clo = self.lower_closure(ast, fid, span, typ);
        self.define_closure_rti(clo.clone(), ast, fid, None);
        self.store_local(name.sym, clo);
    }

    /// Desempacota os argumentos da convenção uniforme para os parâmetros
    /// `params` (na ordem da declaração), todos `Ref`. Aridade errada lança
    /// `NoSuchMethodError` e devolve `None` (o bloco já retornou).
    pub fn unpack(
        &mut self,
        params: &[EntryParam],
        args: Operand,
        desc: Operand,
    ) -> Option<Vec<Operand>> {
        let n_req = params
            .iter()
            .filter(|p| p.kind == ParameterKind::Required)
            .count();
        let n_pos = params
            .iter()
            .filter(|p| p.kind != ParameterKind::Named)
            .count();
        let named: Vec<&EntryParam> = params
            .iter()
            .filter(|p| p.kind == ParameterKind::Named)
            .collect();
        let mut sig = vec![n_req as i64, n_pos as i64, named.len() as i64];
        for p in &named {
            sig.push(hash_name(p.name.as_deref().unwrap_or("")));
        }
        for p in &named {
            sig.push(i64::from(p.required));
        }
        // A entrada tipada (`typed_entry.rs`): a aridade e os nomes vêm
        // garantidos pelo chamador estático.
        if !self.guaranteed_arity {
            let ok = if named.is_empty() {
                // Sem nomeados na assinatura, a conferência do
                // `dartforge_args_casam` é: nenhum nomeado passado e
                // `n_req <= positionals <= n_pos` — em linha, sem a chamada.
                let int = |x: usize| Operand::Constant(Constant::Int(x as i64));
                let npos = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: int(0) }, Type::I64);
                let nnom = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: int(1) }, Type::I64);
                if n_req == n_pos {
                    let x = self.emit(Instruction::Xor(npos, int(n_req)), Type::I64);
                    let y = self.emit(Instruction::Or(x, nnom), Type::I64);
                    self.emit(Instruction::ICmp(ICmpOp::Eq, y, int(0)), Type::I1)
                } else {
                    let lo = self.emit(Instruction::ICmp(ICmpOp::Sge, npos.clone(), int(n_req)), Type::I1);
                    let hi = self.emit(Instruction::ICmp(ICmpOp::Sle, npos, int(n_pos)), Type::I1);
                    let z = self.emit(Instruction::ICmp(ICmpOp::Eq, nnom, int(0)), Type::I1);
                    let a = self.emit(Instruction::And(lo, hi), Type::I64);
                    let a = self.emit(Instruction::And(a, z), Type::I64);
                    self.emit(Instruction::ICmp(ICmpOp::Ne, a, int(0)), Type::I1)
                }
            } else {
                let sig = self.emit(Instruction::ConstArray(sig), Type::Ptr);
                let ok = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_args_casam".to_string(),
                        args: vec![(desc.clone(), Type::Ptr), (sig, Type::Ptr)],
                        ret_ty: Type::I8,
                    },
                    Type::I8,
                );
                self.emit(Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))), Type::I1)
            };
            let b_ok = self.new_block();
            let b_error = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: ok,
                then_block: b_ok,
                else_block: b_error,
            });
            self.set_block(b_error);
            let call = Instruction::CallRuntime {
                name: "dartforge_nsm_chamada".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            };
            if self.ctx.arc_memory {
                // O contrato de reentrada/pendência precisa de saída de erro
                // explícita para validar o cleanup dos owners da entrada.
                self.emit_call_with_check(call, Type::Void);
            } else {
                self.emit(call, Type::Void);
            }
            self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
            self.set_block(b_ok);
        }

        let npos = self.emit(
            Instruction::LoadIndexed {
                base: desc.clone(),
                index: Operand::Constant(Constant::Int(0)),
            },
            Type::I64,
        );
        let mut exit = Vec::with_capacity(params.len());
        let mut i_pos = 0i64;
        for p in params {
            if p.kind == ParameterKind::Required {
                let v = self.emit(
                    Instruction::LoadIndexed {
                        base: args.clone(),
                        index: Operand::Constant(Constant::Int(i_pos)),
                    },
                    Type::Ref,
                );
                exit.push(v);
                i_pos += 1;
                continue;
            }
            let (present, index) = if p.kind == ParameterKind::Optional {
                let c = self.emit(
                    Instruction::ICmp(ICmpOp::Sgt, npos.clone(), Operand::Constant(Constant::Int(i_pos))),
                    Type::I1,
                );
                let idx = Operand::Constant(Constant::Int(i_pos));
                i_pos += 1;
                (c, idx)
            } else {
                let h = hash_name(p.name.as_deref().unwrap_or(""));
                let j = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_arg_indice".to_string(),
                        args: vec![(desc.clone(), Type::Ptr), (Operand::Constant(Constant::Int(h)), Type::I64)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let c = self.emit(
                    Instruction::ICmp(ICmpOp::Sge, j.clone(), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let idx = self.emit(Instruction::Add(npos.clone(), j), Type::I64);
                (c, idx)
            };
            let b_yes = self.new_block();
            let b_no = self.new_block();
            let b_end = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: present,
                then_block: b_yes,
                else_block: b_no,
            });
            self.set_block(b_yes);
            let v_yes = self.emit(
                Instruction::LoadIndexed {
                    base: args.clone(),
                    index: index,
                },
                Type::Ref,
            );
            let yes_end = self.current_block;
            self.terminate(Terminator::Branch(b_end));
            self.set_block(b_no);
            let v_no = match p.pattern {
                Padrao::Nenhum => Operand::Constant(Constant::Null),
                Padrao::Expr(unit, e) => self.lower_expr_of(unit, e),
                Padrao::OfFunction(fid, i) => self
                    .default_value(fid, i)
                    .unwrap_or(Operand::Constant(Constant::Null)),
            };
            let v_no = self.coerce(v_no, Type::Ref);
            let mut entries = vec![(yes_end, v_yes)];
            if !self.is_terminated() {
                entries.push((self.current_block, v_no));
                self.terminate(Terminator::Branch(b_end));
            }
            self.set_block(b_end);
            let v = self.emit(
                Instruction::Phi {
                    incoming: entries,
                    ty: Type::Ref,
                },
                Type::Ref,
            );
            exit.push(v);
        }
        Some(exit)
    }

    /// Chama um valor função (closure, tear-off) com argumentos já
    /// avaliados, pela convenção uniforme. Devolve `Ref`.
    pub fn call_function_value(&mut self, callee: Operand, evaluated: &[Evaluated]) -> Operand {
        // O tipo estático do valor chamado (`lower_call`): com uma ABI
        // tipada, a chamada tenta primeiro o corpo tipado da closure.
        let typ = self.callee_type.take();
        let call = self.current_call.take();
        let callee = self.coerce(callee, Type::Ref);
        if let Some((reprs, ret, abi)) = typ.and_then(|t| self.type_abi(t))
            && reprs.len() == evaluated.len()
            && evaluated.iter().all(|(n, _)| n.is_none())
        {
            return self.call_typed_closure(callee, evaluated, &reprs, ret, abi);
        }
        let tuple = self.value_call_tuple(typ, call);
        self.call_uniform_closure(callee, evaluated, tuple)
    }

    /// A tupla de argumentos de tipo que a chamada `call` de um valor
    /// função passa no slot oculto: os escritos (`f<int>(…)`); sem eles, os
    /// que se deduzem casando o tipo estático genérico do valor (`ty`,
    /// `R Function<X…>(…)`) com o da chamada e os dos argumentos, como a
    /// inferência fez (o que não se deduz fica `dynamic`). `0` (nenhum) quando
    /// o valor não é genérico: a closure genérica chamada assim usa os limites.
    fn value_call_tuple(&mut self, typ: Option<TypeId>, call: Option<ExprId>) -> Operand {
        use dartforge_types::table::Type as T;
        let zero = Operand::Constant(Constant::Int(0));
        let Some(expr) = call else { return zero };
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let ast::ExprKind::Call { arguments, .. } = &unit_ast.expr(expr).kind else { return zero };
        if !arguments.type_args.is_empty() {
            let args = arguments.type_args.to_vec();
            let mut r = self.type_argument_recipes(&args);
            r.text = format!("L<{}>", r.text);
            return self.recipe_rti(&r);
        }
        let Some(t) = typ else { return zero };
        let T::Function { type_params, ret, positional, optional, named, .. } = self.ctx.table.get(t).clone() else {
            return zero;
        };
        if type_params.is_empty() {
            return zero;
        }
        let mut found_items: Vec<Option<TypeId>> = vec![None; type_params.len()];
        if let Some(real) = self.ctx.get_type_raw(self.unit_id, expr) {
            self.unify(ret, real, &type_params, &mut found_items);
        }
        let mut positionals = positional.iter().chain(optional.iter());
        for a in arguments.args.iter() {
            let decl = match a.name {
                Some(n) => named.iter().find(|(s, _, _)| *s == n.sym).map(|(_, t, _)| *t),
                None => positionals.next().copied(),
            };
            if let (Some(d), Some(real)) = (decl, self.ctx.get_type_raw(self.unit_id, a.value)) {
                self.unify(d, real, &type_params, &mut found_items);
            }
        }
        let mut r = super::rti::Recipe { text: "L<".to_string(), variables: false };
        for (i, found) in found_items.iter().enumerate() {
            if i > 0 {
                r.text.push(',');
            }
            let x = self.type_recipe(found.unwrap_or(self.ctx.core.dynamic_));
            r.text.push_str(&x.text);
            r.variables |= x.variables;
        }
        r.text.push('>');
        self.recipe_rti(&r)
    }

    /// A chamada pelo corpo tipado quando a closure tem a ABI `abi`, conferida
    /// em tempo de execução (outra closure, uma classe chamável ou um subtipo
    /// com outra representação seguem pela entrada uniforme): sem vetor de
    /// argumentos, descritor, conferência de aridade nem caixas.
    fn call_typed_closure(&mut self, callee: Operand, evaluated: &[Evaluated], reprs: &[Type], ret: Type, abi: i64) -> Operand {
        // Devolve `Ref`, como a chamada uniforme. Os campos da `_Closure`
        // (código, contexto, corpo tipado, ABI) são lidos do bloco em linha,
        // depois de conferir a classe (`@df.class` em linha): só um bloco de
        // `_Closure` tem os quatro campos; quem não é closure segue pela
        // entrada uniforme.
        debug_assert_ne!(abi, 0);
        let fast = self.new_block();
        let slow = self.new_block();
        let join = self.new_block();
        let check_abi = self.new_block();
        let class = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(callee.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let is_closure = self.emit(
            Instruction::ICmp(
                ICmpOp::Eq,
                class,
                Operand::Constant(Constant::Int(i64::from(dartforge_runtime::layout::cid::CLOSURE))),
            ),
            Type::I1,
        );
        self.terminate(Terminator::CondBranch { cond: is_closure, then_block: check_abi, else_block: slow });

        self.set_block(check_abi);
        let closure_abi = self.object_word(callee.clone(), super::boxes::CLOSURE_ABI, Type::I64);
        let has = self.emit(Instruction::ICmp(ICmpOp::Eq, closure_abi, Operand::Constant(Constant::Int(abi))), Type::I1);
        self.terminate(Terminator::CondBranch { cond: has, then_block: fast, else_block: slow });

        self.set_block(fast);
        let alvo = self.object_word(callee.clone(), super::boxes::CLOSURE_TIPADO, Type::I64);
        // O contexto é uma referência (raiz enquanto vivo).
        let env = self.closure_context(callee.clone());
        let mut args = vec![(env, Type::Ref)];
        for ((_, v), &t) in evaluated.iter().zip(reprs) {
            let v = self.coerce(v.clone(), t);
            args.push((v, t));
        }
        let r = self.emit_call_with_check(Instruction::TypedCall { alvo, args, ret }, ret);
        // O contrato de `call_function_value` é `Ref` (quem chama junta com
        // outros caminhos); o otimizador desfaz a caixa quando quem usa quer
        // o escalar (`Unbox(Box(x))`).
        let r = self.coerce(r, Type::Ref);
        let fast_end = self.current_block;
        let fast_arrives = !self.is_terminated();
        if fast_arrives {
            self.terminate(Terminator::Branch(join));
        }

        self.set_block(slow);
        let s = self.call_uniform_closure(callee, evaluated, Operand::Constant(Constant::Int(0)));
        let slow_end = self.current_block;
        let slow_arrives = !self.is_terminated();
        if slow_arrives {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        match (fast_arrives, slow_arrives) {
            (true, true) => self.emit(Instruction::Phi { incoming: vec![(fast_end, r), (slow_end, s)], ty: Type::Ref }, Type::Ref),
            (true, false) => r,
            (false, true) => s,
            (false, false) => {
                self.terminate(Terminator::Unreachable);
                Operand::Constant(Constant::Null)
            }
        }
    }

    /// A chamada pela convenção uniforme (a entrada `$ent` da closure).
    fn call_uniform_closure(&mut self, callee: Operand, evaluated: &[Evaluated], tuple_types: Operand) -> Operand {
        let mut args = Vec::with_capacity(evaluated.len());
        for (n, v) in evaluated {
            if n.is_none() {
                let v = self.coerce(v.clone(), Type::Ref);
                args.push(v);
            }
        }
        let mut named: Vec<(String, Operand)> = evaluated
            .iter()
            .filter_map(|(n, v)| n.map(|n| (self.ctx.symbol_name(n).to_string(), v.clone())))
            .collect();
        named.sort_by(|a, b| a.0.cmp(&b.0));
        let mut names = Vec::with_capacity(named.len());
        for (n, v) in named {
            let v = self.coerce(v, Type::Ref);
            args.push(v);
            names.push(n);
        }
        self.emit_call_with_check(
            Instruction::CallClosure {
                closure: callee,
                args,
                names,
                ret_ty: Type::Ref,
                tuple_types,
            },
            Type::Ref,
        )
    }

    /// A tupla de argumentos de tipo que a chamada pela convenção uniforme
    /// passa no slot depois dos argumentos (`0`: nenhuma).
    pub(super) fn slot_tuple(&mut self, args: &Operand, desc: &Operand) -> Operand {
        let npos = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(0)) }, Type::I64);
        let nnom = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(1)) }, Type::I64);
        let index = self.emit(Instruction::Add(npos, nnom), Type::I64);
        self.emit(Instruction::LoadIndexed { base: args.clone(), index: index }, Type::I64)
    }

    /// Os parâmetros de uma função do programa, para a entrada uniforme.
    pub(super) fn function_params(&self, fid: usize) -> Vec<EntryParam> {
        let Some(data) = self.ctx.outline.functions.get(fid) else {
            return Vec::new();
        };
        data
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| EntryParam {
                name: p.name.map(|n| self.ctx.symbol_name(n).to_string()),
                kind: p.kind,
                required: p.kind == ParameterKind::Required || p.required,
                pattern: Padrao::OfFunction(fid, i),
            })
            .collect()
    }

    /// Tear-off de função de topo ou estática: canônico.
    pub fn function_tearoff(&mut self, fid: usize) -> Operand {
        let alvo = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{alvo}$tear");
        if self.ctx.arc_memory { self.arc_shared_functions.insert(symbol_ent.clone()); }
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let unit = self.unit_id;
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, unit, symbol_ent.clone(), name, Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            if let Some(vals) = e.unpack(&infos, args.clone(), desc.clone()) {
                // Os argumentos de uma chamada dinâmica, conferidos como na
                // VM (" of 'nome'") antes de convertidos.
                e.check_entry_arguments(fid, &vals);
                // Função genérica: os argumentos de tipo vêm no slot oculto.
                if e.generic_function(fid) {
                    e.armed_tuple = Some(e.slot_tuple(&args, &desc));
                }
                let reprs: Vec<Type> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .map(|p| e.repr(p.ty))
                    .collect();
                let vals: Vec<Operand> = vals
                    .into_iter()
                    .zip(reprs)
                    .map(|(v, r)| e.coerce(v, r))
                    .collect();
                let r = e.call_direct(fid, None, vals);
                let r = e.coerce(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let t = self.emit(
            Instruction::TearOff {
                code_symbol: symbol_ent,
            },
            Type::Ref,
        );
        // RTI: a assinatura da função (`f is R Function(P)`).
        self.define_tearoff_rti(t.clone(), fid, None);
        t
    }

    /// Tear-off de método de instância: closure nova com o receptor no
    /// ambiente; a entrada chama o membro com o despacho pelo receptor.
    pub fn method_tearoff(&mut self, recv: Operand, fid: usize, span: Span) -> Operand {
        let alvo = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{alvo}$tearm");
        if self.ctx.arc_memory { self.arc_shared_functions.insert(symbol_ent.clone()); }
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), name, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            let env = e.closure_context(clo);
            let receiver = e.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
            if self.tearoff_by_selector(fid) {
                // O tear-off é o da implementação que o despacho escolhe
                // (como na VM): os argumentos são conferidos pelos
                // parâmetros dela, e os opcionais omitidos ganham os padrões
                // dela, não os do membro estático. `Def<E>` com
                // `hash(Object? e)` implementando `Eq<E>.hash(E e)`, tirado
                // de um `Def<Never>`, aceita qualquer argumento (o
                // `DefaultEquality<Never>` do `package:collection` no
                // `SetEquality<String>().equals` do freezed, B01). O vetor
                // de argumentos, o descritor e a tupla de tipos seguem como
                // vieram para a entrada da implementação pelo seletor sem o
                // `t`, a que confere tudo (`typed_entry.rs`).
                let f = &self.ctx.program.functions[fid];
                let selector = super::sdk_source::selector_string(
                    self.ctx,
                    super::sdk_source::Tipo::Call,
                    self.ctx.symbol_name(f.name),
                    f.library,
                );
                let r = e.emit_call_with_check(
                    Instruction::CallSelectorForward {
                        arc_owned: false,
                        selector,
                        recv: receiver,
                        args,
                        desc,
                    },
                    Type::Ref,
                );
                e.terminate(Terminator::Return(Some(r)));
            } else if let Some(vals) = e.unpack(&infos, args, desc) {
                // Implementação única: os argumentos conferidos como na
                // entrada uniforme (" of 'nome'"), com o receptor guardado
                // no lugar do `this`, antes da chamada direta. Membro do SDK
                // com uma implementação só: a conferência é a da entrada
                // dela (os nomes dos parâmetros são os dela).
                let checked = self.single_implementation(fid).unwrap_or(fid);
                let saved = (e.this_param.replace(receiver.clone()), e.enclosing_class);
                e.enclosing_class = self.ctx.program.functions[checked].class;
                e.check_entry_arguments(checked, &vals);
                (e.this_param, e.enclosing_class) = saved;
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.call_member(receiver, fid, &evaluated, span);
                let r = if matches!(e.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    e.coerce(r, Type::Ref)
                };
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let recv = self.coerce(recv, Type::Ref);
        let env = self.emit(Instruction::AllocEnv { values: vec![recv.clone()] }, Type::Ref);
        let c = self.emit(
            Instruction::AllocClosure {
                code_symbol: symbol_ent,
                env,
            },
            Type::Ref,
        );
        self.define_tearoff_rti(c.clone(), fid, Some(recv));
        c
    }

    /// A entrada do tear-off de `fid` chama pelo seletor em vez de conferir
    /// os argumentos pelos parâmetros de `fid` e chamar direto: `fid` pode
    /// não ser a implementação que o receptor executa (membro abstrato ou
    /// sobrescrito no programa, ou membro aberto do SDK), e uma sobrescrita
    /// pode alargar o tipo de um parâmetro (`Object?` no lugar do `E` da
    /// interface). Toda classe tem a tabela de seletores (SDK da fonte).
    fn tearoff_by_selector(&self, fid: usize) -> bool {
        use super::sdk_source::{Implementation, implementations, closed_member};
        let f = &self.ctx.program.functions[fid];
        let Some(cid) = f.class else { return false };
        if f.static_ || self.single_implementation(fid).is_some() {
            return false;
        }
        let name = self.ctx.symbol_name(f.name).to_string();
        !closed_member(self.ctx, cid, &name)
            || implementations(self.ctx, cid, &name)[..] != [Implementation::Function(fid)]
    }

    /// O que um nome sem resolução (corpo de closure) é pelo escopo léxico,
    /// depois dos locais: membro da classe envolvente (subindo as
    /// superclasses do programa), senão o escopo da biblioteca. Devolve a
    /// resolução que a inferência teria gravado, para os caminhos de sempre.
    pub fn resolve_by_name(&self, sym: SymbolId) -> Option<dartforge_types::resolved::Resolved> {
        use dartforge_types::resolved::{MemberRef, Resolved};
        // No corpo de uma extensão, os membros dela vêm antes dos do tipo
        // `on` (o escopo léxico da extensão envolve o corpo).
        if let Some((e, _)) = self.this_extension {
            let x = &self.ctx.program.extensions[e.0 as usize];
            if let Some(&f) = x.instance_members.get(&sym).or_else(|| x.static_members.get(&sym)) {
                return Some(Resolved::ExtensionMember { extension: e, member: f });
            }
        }
        // No corpo de um tipo de extensão, os membros dele (e dos tipos de
        // extensão que ele implementa).
        if let Some(r) = self.member_te_by_name(sym) {
            return Some(r);
        }
        for c in self.enclosing_class.map(|c| crate::lower::members::linearization(self.ctx, c)).unwrap_or_default() {
            let class = &self.ctx.program.classes[c.0 as usize];
            if !self.ctx.compiled_library(class.library) {
                break;
            }
            if let Some(&f) = class.instance_members.get(&sym).or_else(|| class.static_members.get(&sym)) {
                return Some(Resolved::Member {
                    class: c,
                    member: MemberRef::Function(f),
                    via_super: false,
                });
            }
            if let Some(&v) = class
                .fields
                .iter()
                .find(|&&v| self.ctx.program.variables[v.0 as usize].name == sym)
            {
                return Some(Resolved::Member {
                    class: c,
                    member: MemberRef::Variable(v),
                    via_super: false,
                });
            }
            // As constantes de um `enum` são membros estáticos dele, mas o
            // modelo as guarda à parte (`enum_constants`): `case ida:` no
            // corpo do próprio enum é o padrão constante `Direcao.ida`.
            if let Some(v) = enum_constant(self.ctx, c, sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
        }
        // Num membro estático, os estáticos da classe que o declara.
        if self.enclosing_class.is_none()
            && let Some(c) = self.member_class
        {
            let class = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = class.static_members.get(&sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Function(f), via_super: false });
            }
            if let Some(&v) = class.fields.iter().find(|&&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                var.static_ && var.name == sym
            }) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
            if let Some(v) = enum_constant(self.ctx, c, sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        let b = self.ctx.program.lookup(lib, sym)?;
        b.getter.or(b.setter).map(Resolved::Element)
    }

    /// Lê um nome sem resolução que não é local (ver `resolve_by_name`).
    pub fn read_unresolved_name(&mut self, sym: SymbolId, span: Span) -> Option<Operand> {
        use dartforge_types::resolved::Resolved;
        match self.resolve_by_name(sym)? {
            r @ Resolved::Member { .. } if self.resolved_member_te(Some(&r)).is_some() => {
                let m = self.resolved_member_te(Some(&r))?;
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                let receiver = self.implicit_receiver();
                Some(self.read_member_te(this, m, receiver, span))
            }
            Resolved::Member { member, .. } => Some(self.read_implicit_member(member, span)),
            Resolved::Element(el) => Some(self.read_element(el, span)),
            Resolved::ExtensionMember { member, .. } => {
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                let receiver = self.implicit_receiver();
                Some(self.read_extension(this, member.0 as usize, receiver, span))
            }
            _ => None,
        }
    }

    /// `f<T…>`: a closure de uma função genérica com os argumentos de tipo
    /// fixos (`tuple`, no ambiente). A entrada arma a tupla e chama a
    /// função; o tipo reificado é o da instanciação (`ty`, estático).
    pub fn instantiated_tearoff(&mut self, fid: usize, tuple: Operand, typ: Option<TypeId>) -> Operand {
        let alvo = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{alvo}$teari");
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), name, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            let env = e.closure_context(clo);
            let t = e.emit(Instruction::EnvGet { env, index: 0 }, Type::I64);
            if let Some(vals) = e.unpack(&infos, args, desc) {
                // Os argumentos de uma chamada dinâmica, conferidos como na
                // VM (" of 'nome'") antes de convertidos.
                e.check_entry_arguments(fid, &vals);
                let reprs: Vec<Type> = self.ctx.outline.functions[fid].parameters.iter().map(|p| e.repr(p.ty)).collect();
                let vals: Vec<Operand> = vals.into_iter().zip(reprs).map(|(v, r)| e.coerce(v, r)).collect();
                e.armed_tuple = Some(t);
                let r = e.call_direct(fid, None, vals);
                let r = if matches!(e.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    e.coerce(r, Type::Ref)
                };
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        // A tupla vai crua (`I64`), como a entrada a lê (`EnvGet` em `I64`).
        let tuple = self.coerce(tuple, Type::I64);
        let env = self.emit(Instruction::AllocEnv { values: vec![tuple] }, Type::Ref);
        let c = self.emit(Instruction::AllocClosure { code_symbol: symbol_ent, env }, Type::Ref);
        self.define_instantiation_rti(c.clone(), fid, typ);
        c
    }

    /// `C<T…>.new`: como `constructor_tearoff`, com o tipo do objeto
    /// (`object`, `C<T…>`) e a tupla da classe (`tuple`, para uma factory)
    /// no ambiente.
    pub fn instantiated_constructor_tearoff(
        &mut self,
        fid: usize,
        object: Operand,
        tuple: Operand,
        typ: Option<TypeId>,
        span: Span,
    ) -> Operand {
        let alvo = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{alvo}$teari");
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), name, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            let env = e.closure_context(clo);
            let obj = e.emit(Instruction::EnvGet { env: env.clone(), index: 0 }, Type::I64);
            let t = e.emit(Instruction::EnvGet { env, index: 1 }, Type::I64);
            if let Some(vals) = e.unpack(&infos, args, desc) {
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.instantiate_evaluated_with_rti(
                    dartforge_elements::model::FunctionElementId(fid as u32),
                    &evaluated,
                    span,
                    Some(obj),
                    Some(t),
                );
                let r = e.coerce(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        // O tipo do objeto e a tupla vão crus (`I64`), como a entrada os lê.
        let object = self.coerce(object, Type::I64);
        let tuple = self.coerce(tuple, Type::I64);
        let env = self.emit(Instruction::AllocEnv { values: vec![object, tuple] }, Type::Ref);
        let c = self.emit(Instruction::AllocClosure { code_symbol: symbol_ent, env }, Type::Ref);
        self.define_instantiation_rti(c.clone(), fid, typ);
        c
    }

    /// O tipo reificado de um tear-off instanciado: o tipo estático da
    /// expressão (a assinatura com os argumentos de tipo aplicados); sem
    /// ele, a assinatura declarada.
    fn define_instantiation_rti(&mut self, clo: Operand, fid: usize, typ: Option<TypeId>) {
        match typ {
            Some(t) if matches!(self.ctx.table.get(t), dartforge_types::table::Type::Function { type_params, .. } if type_params.is_empty()) => {
                let r = self.type_rti(t);
                self.define_rti(clo, r);
            }
            _ => self.define_tearoff_rti(clo, fid, None),
        }
    }

    /// A resolução de uma expressão, ou (identificador sem resolução, como
    /// num corpo de closure) a do nome pelo escopo léxico.
    pub fn resolution_or_name(&self, ast: &ast::Ast, e: ast::ExprId) -> Option<dartforge_types::resolved::Resolved> {
        if let Some(r) = self.ctx.get_resolved(self.unit_id, e).cloned() {
            return Some(r);
        }
        match &ast.expr(e).kind {
            ast::ExprKind::Identifier(n) if self.lookup_local(n.sym).is_none() => self.resolve_by_name(n.sym),
            _ => None,
        }
    }

    /// A função genérica (de topo ou estática, do programa) que `alvo`
    /// nomeia, para `alvo<T…>`.
    pub fn target_generic_function(&self, ast: &ast::Ast, alvo: ast::ExprId) -> Option<usize> {
        use dartforge_types::resolved::{MemberRef, Resolved};
        let fid = match self.resolution_or_name(ast, alvo)? {
            Resolved::Element(dartforge_elements::model::Element::Function(f)) => f.0 as usize,
            Resolved::Member { member: MemberRef::Function(f), .. } => f.0 as usize,
            _ => return None,
        };
        let f = &self.ctx.program.functions[fid];
        let free = f.class.is_none() || f.static_;
        (free && f.variable.is_none() && f.kind == dartforge_elements::model::FunctionKind::Function && self.generic_function(fid))
            .then_some(fid)
    }

    /// As receitas dos argumentos de tipo escritos (`<int, T>`), unidas por
    /// vírgula (`dynamic` para o que não resolve).
    pub fn type_argument_recipes(&self, args: &[ast::TypeId]) -> super::rti::Recipe {
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let mut r = super::rti::Recipe { text: String::new(), variables: false };
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                r.text.push(',');
            }
            match self.annotation_recipe(unit_ast.ty(*a)) {
                Some(x) => {
                    r.text.push_str(&x.text);
                    r.variables |= x.variables;
                }
                None => r.text.push('D'),
            }
        }
        r
    }

    /// Tear-off de construtor (`C.new`, `C.name`): canônico, e a entrada
    /// constrói o objeto com os argumentos recebidos.
    /// Tear-off genérico do construtor `fid` da classe `c` cujo tipo estático
    /// é `C<A…> Function<P…>(…)` (`ty`, os `P` em `params`, os `A` em
    /// `args`): a chamada passa os `P` no slot oculto, e o objeto nasce
    /// `C<A…>` com `P` trocado por eles (um alias `typedef F<T> = C<int, T>`
    /// dá `C<int, T>`).
    pub fn generic_constructor_tearoff(
        &mut self,
        fid: usize,
        c: dartforge_elements::model::ClassId,
        params: &[dartforge_types::table::TypeParamId],
        args: &[TypeId],
        typ: TypeId,
        span: Span,
    ) -> Operand {
        let vars: Vec<String> = (0..params.len()).map(|i| format!("M{i}")).collect();
        let strings: Vec<String> = args.iter().map(|a| self.type_recipe_with(*a, params, &vars).text).collect();
        let string_object = format!("C{}<{}>", self.ctx.id_rti(c), strings.join(","));
        let tuple_text = format!("L<{}>", strings.join(","));
        let alvo = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{alvo}$teara{:08x}", (hash_name(&string_object) as u64) as u32);
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), name, Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let a = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
            let d = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            if let Some(vals) = e.unpack(&infos, a.clone(), d.clone()) {
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let slot = e.slot_tuple(&a, &d);
                e.type_tuple = Some(slot);
                let object = e.recipe_rti(&super::rti::Recipe { text: string_object.clone(), variables: true });
                let tuple = e.recipe_rti(&super::rti::Recipe { text: tuple_text.clone(), variables: true });
                let r = e.instantiate_evaluated_with_rti(
                    dartforge_elements::model::FunctionElementId(fid as u32),
                    &evaluated,
                    span,
                    Some(object),
                    Some(tuple),
                );
                let r = e.coerce(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let t = self.emit(Instruction::TearOff { code_symbol: symbol_ent }, Type::Ref);
        // A assinatura é o tipo estático (a função genérica).
        let r = self.type_recipe(typ);
        let rti = self.recipe_rti(&r);
        self.define_rti(t.clone(), rti);
        t
    }

    /// Tear-off genérico sem instanciação (`final f = C.new;`): a assinatura
    /// é o tipo estático, a função genérica (`<Y0>(Y0) => C<Y0>` na VM), não a
    /// do construtor com os parâmetros da classe em `dynamic`.
    pub fn tearoff_generic_signature(&mut self, t: &Operand, e: ast::ExprId) {
        if let Some(typ) = self.ctx.get_type(self.unit_id, e)
            && matches!(self.ctx.table.get(typ), dartforge_types::table::Type::Function { type_params, .. } if !type_params.is_empty())
        {
            let r = self.type_recipe(typ);
            let rti = self.recipe_rti(&r);
            self.define_rti(t.clone(), rti);
        }
    }

    pub fn constructor_tearoff(&mut self, fid: usize, span: Span) -> Operand {
        let alvo = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{alvo}$tear");
        if self.ctx.arc_memory { self.arc_shared_functions.insert(symbol_ent.clone()); }
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), name, Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Argumentos));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            if let Some(vals) = e.unpack(&infos, args.clone(), desc.clone()) {
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                // Construtor de classe genérica: o tear-off é uma função
                // genérica nos parâmetros da classe (`C<T> Function<T>()`),
                // e os argumentos de tipo da chamada vêm no slot oculto
                // (nenhum: `dynamic`).
                let class = self.ctx.program.functions[fid].class;
                let n = class.and_then(|c| self.ctx.outline.classes.get(c.0 as usize)).map_or(0, |d| d.type_params.len());
                let r = match class.filter(|_| n > 0) {
                    Some(c) => {
                        let tuple = e.slot_tuple(&args, &desc);
                        e.type_tuple = Some(tuple.clone());
                        let vars: Vec<String> = (0..n).map(|i| format!("M{i}")).collect();
                        let object = e.recipe_rti(&super::rti::Recipe {
                            text: format!("C{}<{}>", self.ctx.id_rti(c), vars.join(",")),
                            variables: true,
                        });
                        e.instantiate_evaluated_with_rti(
                            dartforge_elements::model::FunctionElementId(fid as u32),
                            &evaluated,
                            span,
                            Some(object),
                            Some(tuple),
                        )
                    }
                    None => e.instantiate_evaluated(dartforge_elements::model::FunctionElementId(fid as u32), &evaluated, span),
                };
                let r = e.coerce(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let t = self.emit(
            Instruction::TearOff {
                code_symbol: symbol_ent,
            },
            Type::Ref,
        );
        // O construtor também é uma função reificada. Sem a assinatura, o
        // `current as E` do ListIterator rejeita um tear-off guardado em
        // `List<C Function()>` quando E passa a ser propagado pelo SDK.
        self.define_tearoff_rti(t.clone(), fid, None);
        t
    }
}

/// A constante `sym` do `enum` `c`, se `c` é um enum e a declara.
fn enum_constant(
    ctx: &crate::context::Context,
    c: dartforge_elements::model::ClassId,
    sym: SymbolId,
) -> Option<dartforge_elements::model::VariableId> {
    ctx.program.classes[c.0 as usize]
        .enum_constants
        .iter()
        .copied()
        .find(|v| ctx.program.variables[v.0 as usize].name == sym)
}
