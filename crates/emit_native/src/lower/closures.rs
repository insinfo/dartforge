//! Closures (P1): expressão de função, função local, tear-offs e a chamada
//! de um valor função.
//!
//! **Convenção uniforme** (a do `dartdevc`/VM para chamadas dinâmicas: um
//! vetor de argumentos e um *arguments descriptor*): todo valor função é
//! chamado por uma **entrada** `i64 @<corpo>$ent(i64 closure, ptr args, ptr
//! desc)`. `args` tem os argumentos (`Ref`), os posicionais primeiro e os
//! nomeados depois, na ordem do descritor; `desc` é
//! `[n_posicionais, n_nomeados, hash(nome)…]` com os nomes ordenados. A
//! entrada confere a aridade contra a assinatura da função
//! (`dartforge_args_casam`, que devolve 0 quando não casa — então lança
//! `NoSuchMethodError`, como a VM), preenche os padrões dos opcionais
//! ausentes e chama o **corpo** com os parâmetros na ordem da declaração.
//!
//! O corpo de uma closure é `i64 @<símbolo>(i64 env, i64 p0, …)`, com todos
//! os parâmetros e o retorno `Ref` (a convenção uniforme; o corpo é inferido
//! por `crates/types`, e [`FnBuilder::resolver_por_nome`] cobre o nome sem
//! resolução gravada). O ambiente guarda, nesta ordem, `this`
//! (quando a closure nasce num membro de instância) e as variáveis livres —
//! o handle da célula das que moram numa (`captura.rs`), ou a cópia do valor.
//!
//! A closure é `AllocClosure { código, env }`; o código é o endereço da
//! entrada uniforme (`ptrtoint`, no emissor), válido entre o módulo do SDK e
//! o do programa. O tear-off de função de topo ou
//! estática é canônico (`TearOff`: o mesmo handle sempre, `identical(f, f)`);
//! o de método de instância é uma closure nova com o receptor no ambiente, e
//! a entrada dele chama o membro com o despacho do receptor.

use super::captura::{self, Raiz};
use super::fn_builder::FnBuilder;
use super::locais::Modo;
use super::membros::Avaliado;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, FunctionBody, FunctionId, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::table::TypeId;

/// Hash estável do nome de um argumento nomeado (FNV-1a de 64 bits sobre o
/// UTF-8): o mesmo em qualquer módulo, o que o descritor precisa para que uma
/// biblioteca compilada à parte chame outra.
pub fn hash_nome(nome: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in nome.bytes() {
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
    DeFuncao(usize, usize),
}

/// Um parâmetro como a entrada uniforme o vê.
#[derive(Clone)]
pub struct ParamEntrada {
    pub nome: Option<String>,
    pub kind: ParameterKind,
    pub required: bool,
    pub padrao: Padrao,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Pré-passe das células (antes de declarar os parâmetros).
    pub fn preparar_capturas(&mut self, ast: &ast::Ast, raiz: Raiz) {
        self.celulas = captura::analisar(self.ctx, self.unit_id, ast, raiz).celulas;
    }

    /// Nome do corpo de uma closure dentro desta função: `$clo<k>` para as
    /// anônimas (k conta só as anônimas, na ordem do texto) e `$<nome>` para
    /// as funções locais (com `$<k>` se o nome se repete).
    fn nome_de_closure(&mut self, nome: Option<SymbolId>) -> String {
        match nome {
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
                let mut nome = base.clone();
                let mut k = 1;
                while !self.nomes_locais.insert(nome.clone()) {
                    nome = format!("{base}${k}");
                    k += 1;
                }
                nome
            }
        }
    }

    /// Os parâmetros de uma expressão de função, para a entrada uniforme.
    fn params_da_closure(&self, f: &ast::Function) -> Vec<ParamEntrada> {
        let unit = self.unit_id;
        f.parameters
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|p| ParamEntrada {
                nome: p.name.map(|n| self.ctx.symbol_name(n.sym).to_string()),
                kind: p.kind,
                required: p.kind == ParameterKind::Required || p.required,
                padrao: p.default_value.map_or(Padrao::Nenhum, |e| Padrao::Expr(unit, e)),
            })
            .collect()
    }

    /// A ABI tipada de um tipo de função: as representações dos posicionais
    /// e do retorno (`void` volta como `Ref`, o null), e o código delas. Só
    /// para tipos sem parâmetros de tipo, opcionais nem nomeados, até 12
    /// posicionais.
    pub(super) fn abi_do_tipo(&self, t: TypeId) -> Option<(Vec<Type>, Type, i64)> {
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
        let digito = |t: Type| match t {
            Type::I64 => 2,
            Type::F64 => 3,
            Type::I1 => 4,
            _ => 1,
        };
        let mut abi: i64 = 1;
        for &t in params.iter().chain(std::iter::once(&ret)) {
            abi = abi * 5 + digito(t);
        }
        Some((params, ret, abi))
    }

    /// A ABI tipada de uma closure síncrona só com posicionais obrigatórios,
    /// pelo tipo estático dela (o do contexto, já inferido).
    fn abi_da_closure(&self, f: &ast::Function, tipo: Option<TypeId>) -> Option<(Vec<Type>, Type, i64)> {
        let params = f.parameters.as_deref().unwrap_or(&[]);
        if f.modifier != AsyncModifier::None
            || !f.type_params.is_empty()
            || params.iter().any(|p| p.kind != ParameterKind::Required)
        {
            return None;
        }
        let abi = self.abi_do_tipo(tipo?)?;
        (abi.0.len() == params.len()).then_some(abi)
    }

    /// `(params) => e`, `(params) { … }` ou o valor de uma função local.
    /// `tipo`: o tipo estático da expressão da closure (o de contexto), de
    /// onde sai o tipo do valor de um corpo `async`/gerador.
    pub fn lower_closure(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span, tipo: Option<TypeId>) -> Operand {
        let f = ast.function(fid);
        // Variáveis livres que são locais visíveis aqui.
        let (livres, _) = captura::livres(self.ctx, self.unit_id, ast, fid);
        let capturas: Vec<(SymbolId, super::locais::Local)> = livres
            .into_iter()
            .filter_map(|s| self.buscar_local(s).map(|l| (s, l)))
            .collect();
        let com_this = self.this_param.is_some();
        let base = usize::from(com_this);

        // O nome leva a impressão digital do que o corpo espera do ambiente
        // (as capturas, na ordem, com o modo e o tipo; o `this`; a tupla de
        // tipos) e dos parâmetros: numa recarga do JIT, uma closure viva só
        // passa a executar o corpo novo de mesmo nome se ele lê o ambiente
        // dela do mesmo jeito. Uma closure nova inserida antes, ou capturas
        // que mudam, dão outro nome, e as closures vivas seguem no corpo em
        // que foram criadas.
        let mut forma = format!("{com_this}|{}|{}|", self.tupla_de_tipos.is_some(), f.type_params.len());
        // Closure genérica (`T f<T>()` local, `<T>(x) => …`): os argumentos de
        // tipo dela vêm no slot oculto da chamada e se juntam, na tupla do
        // corpo, depois dos de quem a criou (`M<n_fora + i>`).
        let n_fora = self.tamanho_da_tupla();
        let ids_proprios: Vec<dartforge_types::table::TypeParamId> = match tipo.map(|t| self.ctx.table.get(t)) {
            Some(dartforge_types::table::Type::Function { type_params, .. }) if type_params.len() == f.type_params.len() => {
                type_params.to_vec()
            }
            _ => Vec::new(),
        };
        let proprios: Vec<(SymbolId, dartforge_types::table::TypeParamId, usize)> = f
            .type_params
            .iter()
            .enumerate()
            .map(|(i, tp)| {
                let id = ids_proprios.get(i).copied().unwrap_or(dartforge_types::table::TypeParamId(u32::MAX));
                (tp.name.sym, id, n_fora + i)
            })
            .collect();
        let generica = !proprios.is_empty();
        for (sym, l) in &capturas {
            let celula = matches!(l.modo, Modo::Celula(_) | Modo::Ambiente { celula: true, .. });
            forma.push_str(&format!("{}:{celula}:{:?};", self.ctx.symbol_name(*sym), l.ty));
        }
        for p in f.parameters.as_deref().unwrap_or(&[]) {
            let nome = p.name.map_or("", |n| self.ctx.symbol_name(n.sym));
            forma.push_str(&format!("{nome}:{:?}:{};", p.kind, p.required));
        }
        // A ABI tipada do corpo: parâmetros e retorno nas representações do
        // tipo da closure; entra na impressão digital, e uma closure viva
        // segue no corpo da ABI em que nasceu.
        let abi = self.abi_da_closure(f, tipo);
        if let Some((ps, r, codigo)) = &abi {
            forma.push_str(&format!("abi:{codigo}:{ps:?}:{r:?};"));
        }
        let (reprs, ret_repr) = match &abi {
            Some((ps, r, _)) => (Some(ps.clone()), *r),
            None => (None, Type::Ref),
        };
        let simbolo = format!(
            "{}$e{:08x}",
            self.nome_de_closure(f.name.map(|n| n.sym)),
            (hash_nome(&forma) as u64) as u32
        );
        let legivel = f
            .name
            .map_or_else(|| "<closure>".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());

        // --- corpo ---------------------------------------------------------
        let mut b = FnBuilder::new(self.ctx, self.unit_id, simbolo.clone(), legivel.clone(), ret_repr);
        let env_b = Operand::Val(b.add_param("env".to_string(), Type::Ref));
        if com_this {
            let t = b.emit(
                Instruction::EnvGet {
                    env: env_b.clone(),
                    index: 0,
                },
                Type::Ref,
            );
            b.this_param = Some(t);
            b.enclosing_class = self.enclosing_class;
        }
        for (i, (sym, l)) in capturas.iter().enumerate() {
            let celula = matches!(
                l.modo,
                Modo::Celula(_) | Modo::Ambiente { celula: true, .. }
            );
            b.ligar_ambiente(*sym, env_b.clone(), base + i, celula, l.ty, l.late.as_ref());
        }
        // RTI: a closure vê as variáveis de tipo de quem a cria (`T` da
        // função genérica em volta: a tupla vai no fim do ambiente).
        b.params_de_tipo_da_funcao = self.params_de_tipo_da_funcao.clone();
        b.params_locais = self.params_locais.clone();
        b.params_locais.extend(proprios.iter().copied());
        b.extensao_do_this = self.extensao_do_this;
        b.tipo_ext_do_this = self.tipo_ext_do_this;
        b.classe_do_membro = self.classe_do_membro;
        b.classe_por_tupla = self.classe_por_tupla;
        if self.classe_por_tupla {
            // Numa fábrica não há `this`, mas `T` é o da classe.
            b.enclosing_class = self.enclosing_class;
        }
        if self.tupla_de_tipos.is_some() && !generica {
            let t = b.emit(
                Instruction::EnvGet {
                    env: env_b.clone(),
                    index: base + capturas.len(),
                },
                Type::I64,
            );
            b.tupla_de_tipos = Some(t);
        }
        let params = f.parameters.as_deref().unwrap_or(&[]);
        b.preparar_capturas(
            ast,
            Raiz {
                parametros: params,
                corpo: Some(&f.body),
                inicializadores: &[],
            },
        );
        b.abrir_escopo();
        for (i, p) in params.iter().enumerate() {
            let nome = p
                .name
                .map_or_else(|| "arg".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());
            let ty = reprs.as_ref().map_or(Type::Ref, |r| r[i]);
            let vid = b.add_param(nome, ty);
            if let Some(n) = p.name {
                b.declarar_variavel(n.sym, n.span.start as usize, ty, Operand::Val(vid));
            }
        }
        // A tupla juntada (a de fora e a da chamada) chega ao corpo da
        // closure genérica como o último parâmetro.
        if generica {
            let t = b.add_param("$tipos".to_string(), Type::I64);
            b.tupla_de_tipos = Some(Operand::Val(t));
        }
        if f.modifier != AsyncModifier::None {
            // P6: closure `async`, `sync*` ou `async*` — o corpo vira máquina
            // de estados. O tipo do valor/elemento sai da anotação de retorno
            // ou do tipo estático da closure; sem nenhum, `dynamic`.
            use super::async_sm::RetornoAsync;
            let retorno = f.return_type.map(RetornoAsync::Anotacao).or_else(|| {
                tipo.and_then(|t| match self.ctx.table.get(t) {
                    dartforge_types::table::Type::Function { ret, .. } => Some(RetornoAsync::Tipo(*ret)),
                    _ => None,
                })
            });
            b.lower_corpo_async(ast, params, &f.body, span, retorno, super::async_sm::tipo_do_corpo(f.modifier));
        } else {
            match &f.body {
                FunctionBody::Block(s) => b.lower_stmt(ast, *s),
                FunctionBody::Expression(e) => {
                    let r = b.lower_expr(ast, *e);
                    if !b.is_terminated() {
                        let r = b.coagir(r, ret_repr);
                        b.terminate(Terminator::Return(Some(r)));
                    }
                }
                _ => {}
            }
        }
        self.absorver(b);

        // --- entrada uniforme ----------------------------------------------
        let infos = self.params_da_closure(f);
        let simbolo_ent = format!("{simbolo}$ent");
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
        // Os argumentos de tipo que a closure genérica usa quando a chamada
        // não passa nenhum (chamada dinâmica): os limites escritos, como a
        // instanciação pelos limites; `dynamic` sem limite ou quando o limite
        // depende de variáveis de tipo.
        let padrao = generica.then(|| {
            let mut texto = String::from("L<");
            for (i, (_, id, _)) in proprios.iter().enumerate() {
                if i > 0 {
                    texto.push(',');
                }
                let limite = (id.0 != u32::MAX)
                    .then(|| self.ctx.table.param(*id))
                    .filter(|d| d.explicito)
                    .map(|d| self.receita_de_tipo(d.bound))
                    .filter(|r| !r.variaveis);
                texto.push_str(limite.as_ref().map_or("D", |r| r.texto.as_str()));
            }
            texto.push('>');
            super::rti::Receita { texto, variaveis: false }
        });
        if let Some(vals) = e.desempacotar(&infos, args.clone(), desc.clone()) {
            let mut todos = vec![env_e.clone()];
            // O corpo tipado recebe cada argumento na representação dele (o
            // desencaixe confere o tipo, como a entrada dinâmica da VM).
            for (i, v) in vals.into_iter().enumerate() {
                let ty = reprs.as_ref().map_or(Type::Ref, |r| r[i]);
                let v = e.coagir(v, ty);
                todos.push(v);
            }
            if let Some(padrao) = &padrao {
                // A tupla da chamada está no slot depois dos argumentos.
                let propria = e.tupla_do_slot(&args, &desc);
                let de_fora = if self.tupla_de_tipos.is_some() {
                    e.emit(Instruction::EnvGet { env: env_e.clone(), index: base + capturas.len() }, Type::I64)
                } else {
                    Operand::Constant(Constant::Int(0))
                };
                let padrao = e.rti_da_receita(padrao);
                let juntada = e.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_rti_tupla_juntar".to_string(),
                        args: vec![
                            (de_fora, Type::I64),
                            (Operand::Constant(Constant::Int(n_fora as i64)), Type::I64),
                            (propria, Type::I64),
                            (padrao, Type::I64),
                        ],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                todos.push(juntada);
            }
            if !e.is_terminated() {
                let r = e.emit_call_with_check(
                    Instruction::CallStatic {
                        symbol: simbolo.clone(),
                        args: todos,
                        ret_ty: ret_repr,
                    },
                    ret_repr,
                );
                let r = e.coagir(r, Type::Ref);
                if !e.is_terminated() {
                    e.terminate(Terminator::Return(Some(r)));
                }
            }
        }
        self.absorver(e);

        // --- criação ---------------------------------------------------------
        let mut valores = Vec::with_capacity(base + capturas.len());
        if let Some(t) = self.this_param.clone() {
            valores.push(t);
        }
        for (_, l) in &capturas {
            let v = match self.celula_do_local(l) {
                Some(c) => c,
                None => self.ler_local(l),
            };
            valores.push(v);
        }
        if let Some(t) = self.tupla_de_tipos.clone() {
            valores.push(t);
        }
        let env = self.emit(Instruction::AllocEnv { values: valores }, Type::Ref);
        match abi {
            Some((_, _, codigo)) => self.emit(
                Instruction::AllocClosureTipada { code_symbol: simbolo_ent, env, tipado: simbolo, abi: codigo },
                Type::Ref,
            ),
            None => self.emit(
                Instruction::AllocClosure {
                    code_symbol: simbolo_ent,
                    env,
                },
                Type::Ref,
            ),
        }
    }

    /// Entrega ao módulo (pelas `extra_functions` desta função) uma função
    /// construída à parte, com os diagnósticos dela.
    pub fn absorver(&mut self, b: FnBuilder<'a, 'c>) {
        self.erros.extend(b.erros);
        self.globais_extras.extend(b.globais_extras);
        self.extra_functions.push(b.func);
        self.extra_functions.extend(b.extra_functions);
    }

    /// `R f(params) { … }` local: o nome é uma variável que guarda a closure
    /// (numa célula quando a função chama a si mesma ou é capturada e ligada
    /// depois — `captura.rs` marca o nome como atribuído).
    pub fn declarar_funcao_local(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span) {
        let f = ast.function(fid);
        let Some(nome) = f.name else {
            return;
        };
        self.declarar_variavel(
            nome.sym,
            nome.span.start as usize,
            Type::Ref,
            Operand::Constant(Constant::Null),
        );
        let tipo = self.tipo_da_funcao_literal(ast, fid, None);
        let clo = self.lower_closure(ast, fid, span, tipo);
        self.definir_rti_de_closure(clo.clone(), ast, fid, None);
        self.gravar_local(nome.sym, clo);
    }

    /// Desempacota os argumentos da convenção uniforme para os parâmetros
    /// `params` (na ordem da declaração), todos `Ref`. Aridade errada lança
    /// `NoSuchMethodError` e devolve `None` (o bloco já retornou).
    pub fn desempacotar(
        &mut self,
        params: &[ParamEntrada],
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
        let nomeados: Vec<&ParamEntrada> = params
            .iter()
            .filter(|p| p.kind == ParameterKind::Named)
            .collect();
        let mut sig = vec![n_req as i64, n_pos as i64, nomeados.len() as i64];
        for p in &nomeados {
            sig.push(hash_nome(p.nome.as_deref().unwrap_or("")));
        }
        for p in &nomeados {
            sig.push(i64::from(p.required));
        }
        let sig = self.emit(Instruction::ConstArray(sig), Type::Ptr);
        let ok = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_args_casam".to_string(),
                args: vec![(desc.clone(), Type::Ptr), (sig, Type::Ptr)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let ok = self.emit(
            Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let b_ok = self.new_block();
        let b_erro = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ok,
            then_block: b_ok,
            else_block: b_erro,
        });
        self.set_block(b_erro);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_nsm_chamada".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
        self.set_block(b_ok);

        let npos = self.emit(
            Instruction::LoadIndexed {
                base: desc.clone(),
                index: Operand::Constant(Constant::Int(0)),
            },
            Type::I64,
        );
        let mut saida = Vec::with_capacity(params.len());
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
                saida.push(v);
                i_pos += 1;
                continue;
            }
            let (presente, indice) = if p.kind == ParameterKind::Optional {
                let c = self.emit(
                    Instruction::ICmp(ICmpOp::Sgt, npos.clone(), Operand::Constant(Constant::Int(i_pos))),
                    Type::I1,
                );
                let idx = Operand::Constant(Constant::Int(i_pos));
                i_pos += 1;
                (c, idx)
            } else {
                let h = hash_nome(p.nome.as_deref().unwrap_or(""));
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
            let b_sim = self.new_block();
            let b_nao = self.new_block();
            let b_fim = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: presente,
                then_block: b_sim,
                else_block: b_nao,
            });
            self.set_block(b_sim);
            let v_sim = self.emit(
                Instruction::LoadIndexed {
                    base: args.clone(),
                    index: indice,
                },
                Type::Ref,
            );
            let fim_sim = self.current_block;
            self.terminate(Terminator::Branch(b_fim));
            self.set_block(b_nao);
            let v_nao = match p.padrao {
                Padrao::Nenhum => Operand::Constant(Constant::Null),
                Padrao::Expr(unit, e) => self.lower_expr_de(unit, e),
                Padrao::DeFuncao(fid, i) => self
                    .valor_padrao(fid, i)
                    .unwrap_or(Operand::Constant(Constant::Null)),
            };
            let v_nao = self.coagir(v_nao, Type::Ref);
            let mut entradas = vec![(fim_sim, v_sim)];
            if !self.is_terminated() {
                entradas.push((self.current_block, v_nao));
                self.terminate(Terminator::Branch(b_fim));
            }
            self.set_block(b_fim);
            let v = self.emit(
                Instruction::Phi {
                    incoming: entradas,
                    ty: Type::Ref,
                },
                Type::Ref,
            );
            saida.push(v);
        }
        Some(saida)
    }

    /// Chama um valor função (closure, tear-off) com argumentos já
    /// avaliados, pela convenção uniforme. Devolve `Ref`.
    pub fn chamar_valor_funcao(&mut self, callee: Operand, avaliados: &[Avaliado]) -> Operand {
        // O tipo estático do valor chamado (`lower_chamada`): com uma ABI
        // tipada, a chamada tenta primeiro o corpo tipado da closure.
        let tipo = self.tipo_chamado.take();
        let chamada = self.chamada_corrente.take();
        let callee = self.coagir(callee, Type::Ref);
        if let Some((reprs, ret, abi)) = tipo.and_then(|t| self.abi_do_tipo(t))
            && reprs.len() == avaliados.len()
            && avaliados.iter().all(|(n, _)| n.is_none())
        {
            return self.chamar_closure_tipada(callee, avaliados, &reprs, ret, abi);
        }
        let tupla = self.tupla_da_chamada_de_valor(tipo, chamada);
        self.chamar_closure_uniforme(callee, avaliados, tupla)
    }

    /// A tupla de argumentos de tipo que a chamada `chamada` de um valor
    /// função passa no slot oculto: os escritos (`f<int>(…)`); sem eles, os
    /// que se deduzem casando o tipo estático genérico do valor (`tipo`,
    /// `R Function<X…>(…)`) com o da chamada e os dos argumentos, como a
    /// inferência fez (o que não se deduz fica `dynamic`). `0` (nenhum) quando
    /// o valor não é genérico: a closure genérica chamada assim usa os limites.
    fn tupla_da_chamada_de_valor(&mut self, tipo: Option<TypeId>, chamada: Option<ExprId>) -> Operand {
        use dartforge_types::table::Type as T;
        let zero = Operand::Constant(Constant::Int(0));
        let Some(expr) = chamada else { return zero };
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let ast::ExprKind::Call { arguments, .. } = &unit_ast.expr(expr).kind else { return zero };
        if !arguments.type_args.is_empty() {
            let args = arguments.type_args.to_vec();
            let mut r = self.receitas_dos_argumentos_de_tipo(&args);
            r.texto = format!("L<{}>", r.texto);
            return self.rti_da_receita(&r);
        }
        let Some(t) = tipo else { return zero };
        let T::Function { type_params, ret, positional, optional, named, .. } = self.ctx.table.get(t).clone() else {
            return zero;
        };
        if type_params.is_empty() {
            return zero;
        }
        let mut achados: Vec<Option<TypeId>> = vec![None; type_params.len()];
        if let Some(real) = self.ctx.get_type_bruto(self.unit_id, expr) {
            self.unificar(ret, real, &type_params, &mut achados);
        }
        let mut posicionais = positional.iter().chain(optional.iter());
        for a in arguments.args.iter() {
            let decl = match a.name {
                Some(n) => named.iter().find(|(s, _, _)| *s == n.sym).map(|(_, t, _)| *t),
                None => posicionais.next().copied(),
            };
            if let (Some(d), Some(real)) = (decl, self.ctx.get_type_bruto(self.unit_id, a.value)) {
                self.unificar(d, real, &type_params, &mut achados);
            }
        }
        let mut r = super::rti::Receita { texto: "L<".to_string(), variaveis: false };
        for (i, achado) in achados.iter().enumerate() {
            if i > 0 {
                r.texto.push(',');
            }
            let x = self.receita_de_tipo(achado.unwrap_or(self.ctx.core.dynamic_));
            r.texto.push_str(&x.texto);
            r.variaveis |= x.variaveis;
        }
        r.texto.push('>');
        self.rti_da_receita(&r)
    }

    /// A chamada pelo corpo tipado quando a closure tem a ABI `abi`, conferida
    /// em tempo de execução (outra closure, uma classe chamável ou um subtipo
    /// com outra representação seguem pela entrada uniforme): sem vetor de
    /// argumentos, descritor, conferência de aridade nem caixas.
    fn chamar_closure_tipada(&mut self, callee: Operand, avaliados: &[Avaliado], reprs: &[Type], ret: Type, abi: i64) -> Operand {
        // Devolve `Ref`, como a chamada uniforme. O cabeçalho da closure
        // (`heap::CabecalhoDeClosure`: código, ambiente, corpo tipado, ABI)
        // vem de uma chamada pura e é lido em linha; quem não é closure tem
        // ABI 0, que não casa com nenhuma.
        debug_assert_ne!(abi, 0);
        let cab = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_closure_cabecalho".to_string(),
                args: vec![(callee.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let palavra = |s: &mut Self, i: i64, ty: Type| {
            s.emit(
                Instruction::CargaNativa {
                    endereco: cab.clone(),
                    indice: Operand::Constant(Constant::Int(i)),
                    tipo: TipoC::I64,
                },
                ty,
            )
        };
        let abi_da_closure = palavra(self, 3, Type::I64);
        let tem = self.emit(Instruction::ICmp(ICmpOp::Eq, abi_da_closure, Operand::Constant(Constant::Int(abi))), Type::I1);
        let rapido = self.new_block();
        let lento = self.new_block();
        let juncao = self.new_block();
        self.terminate(Terminator::CondBranch { cond: tem, then_block: rapido, else_block: lento });

        self.set_block(rapido);
        let alvo = palavra(self, 2, Type::I64);
        // O ambiente é uma referência (raiz enquanto vivo).
        let env = palavra(self, 1, Type::Ref);
        let mut args = vec![(env, Type::Ref)];
        for ((_, v), &t) in avaliados.iter().zip(reprs) {
            let v = self.coagir(v.clone(), t);
            args.push((v, t));
        }
        let r = self.emit_call_with_check(Instruction::ChamadaTipada { alvo, args, ret }, ret);
        // O contrato de `chamar_valor_funcao` é `Ref` (quem chama junta com
        // outros caminhos); o otimizador desfaz a caixa quando quem usa quer
        // o escalar (`Unbox(Box(x))`).
        let r = self.coagir(r, Type::Ref);
        let fim_rapido = self.current_block;
        let rapido_chega = !self.is_terminated();
        if rapido_chega {
            self.terminate(Terminator::Branch(juncao));
        }

        self.set_block(lento);
        let s = self.chamar_closure_uniforme(callee, avaliados, Operand::Constant(Constant::Int(0)));
        let fim_lento = self.current_block;
        let lento_chega = !self.is_terminated();
        if lento_chega {
            self.terminate(Terminator::Branch(juncao));
        }
        self.set_block(juncao);
        match (rapido_chega, lento_chega) {
            (true, true) => self.emit(Instruction::Phi { incoming: vec![(fim_rapido, r), (fim_lento, s)], ty: Type::Ref }, Type::Ref),
            (true, false) => r,
            (false, true) => s,
            (false, false) => {
                self.terminate(Terminator::Unreachable);
                Operand::Constant(Constant::Null)
            }
        }
    }

    /// A chamada pela convenção uniforme (a entrada `$ent` da closure).
    fn chamar_closure_uniforme(&mut self, callee: Operand, avaliados: &[Avaliado], tupla_tipos: Operand) -> Operand {
        let mut args = Vec::with_capacity(avaliados.len());
        for (n, v) in avaliados {
            if n.is_none() {
                let v = self.coagir(v.clone(), Type::Ref);
                args.push(v);
            }
        }
        let mut nomeados: Vec<(String, Operand)> = avaliados
            .iter()
            .filter_map(|(n, v)| n.map(|n| (self.ctx.symbol_name(n).to_string(), v.clone())))
            .collect();
        nomeados.sort_by(|a, b| a.0.cmp(&b.0));
        let mut nomes = Vec::with_capacity(nomeados.len());
        for (n, v) in nomeados {
            let v = self.coagir(v, Type::Ref);
            args.push(v);
            nomes.push(n);
        }
        self.emit_call_with_check(
            Instruction::CallClosure {
                closure: callee,
                args,
                nomes,
                ret_ty: Type::Ref,
                tupla_tipos,
            },
            Type::Ref,
        )
    }

    /// A tupla de argumentos de tipo que a chamada pela convenção uniforme
    /// passa no slot depois dos argumentos (`0`: nenhuma).
    pub(super) fn tupla_do_slot(&mut self, args: &Operand, desc: &Operand) -> Operand {
        let npos = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(0)) }, Type::I64);
        let nnom = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(1)) }, Type::I64);
        let indice = self.emit(Instruction::Add(npos, nnom), Type::I64);
        self.emit(Instruction::LoadIndexed { base: args.clone(), index: indice }, Type::I64)
    }

    /// Os parâmetros de uma função do programa, para a entrada uniforme.
    pub(super) fn params_da_funcao(&self, fid: usize) -> Vec<ParamEntrada> {
        let Some(dados) = self.ctx.outline.functions.get(fid) else {
            return Vec::new();
        };
        dados
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| ParamEntrada {
                nome: p.name.map(|n| self.ctx.symbol_name(n).to_string()),
                kind: p.kind,
                required: p.kind == ParameterKind::Required || p.required,
                padrao: Padrao::DeFuncao(fid, i),
            })
            .collect()
    }

    /// Tear-off de função de topo ou estática: canônico.
    pub fn tearoff_de_funcao(&mut self, fid: usize) -> Operand {
        let alvo = super::simbolo_de(self.ctx, fid);
        let simbolo_ent = format!("{alvo}$tear");
        if !self.entradas_feitas.contains(&simbolo_ent) {
            self.entradas_feitas.insert(simbolo_ent.clone());
            let infos = self.params_da_funcao(fid);
            let unit = self.unit_id;
            let nome = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, unit, simbolo_ent.clone(), nome, Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let args = Operand::Val(e.add_param("args".to_string(), Type::Ptr));
            let desc = Operand::Val(e.add_param("desc".to_string(), Type::Ptr));
            if let Some(vals) = e.desempacotar(&infos, args.clone(), desc.clone()) {
                // Os argumentos de uma chamada dinâmica, conferidos como na
                // VM (" of 'nome'") antes de convertidos.
                e.conferir_argumentos_da_entrada(fid, &vals);
                // Função genérica: os argumentos de tipo vêm no slot oculto.
                if e.funcao_generica(fid) {
                    e.tupla_armada = Some(e.tupla_do_slot(&args, &desc));
                }
                let reprs: Vec<Type> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .map(|p| e.repr(p.ty))
                    .collect();
                let vals: Vec<Operand> = vals
                    .into_iter()
                    .zip(reprs)
                    .map(|(v, r)| e.coagir(v, r))
                    .collect();
                let r = e.chamar_direto(fid, None, vals);
                let r = e.coagir(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorver(e);
        }
        let t = self.emit(
            Instruction::TearOff {
                code_symbol: simbolo_ent,
            },
            Type::Ref,
        );
        // RTI: a assinatura da função (`f is R Function(P)`).
        self.definir_rti_de_tearoff(t.clone(), fid, None);
        t
    }

    /// Tear-off de método de instância: closure nova com o receptor no
    /// ambiente; a entrada chama o membro com o despacho pelo receptor.
    pub fn tearoff_de_metodo(&mut self, recv: Operand, fid: usize, span: Span) -> Operand {
        let alvo = super::simbolo_de(self.ctx, fid);
        let simbolo_ent = format!("{alvo}$tearm");
        if !self.entradas_feitas.contains(&simbolo_ent) {
            self.entradas_feitas.insert(simbolo_ent.clone());
            let infos = self.params_da_funcao(fid);
            let nome = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, simbolo_ent.clone(), nome, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param("args".to_string(), Type::Ptr));
            let desc = Operand::Val(e.add_param("desc".to_string(), Type::Ptr));
            let env = e.emit(
                Instruction::CallRuntime {
                    name: "dartforge_closure_env".to_string(),
                    args: vec![(clo, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            let receptor = e.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
            if let Some(vals) = e.desempacotar(&infos, args, desc) {
                let avaliados: Vec<Avaliado> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.chamar_membro(receptor, fid, &avaliados, span);
                let r = if matches!(e.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    e.coagir(r, Type::Ref)
                };
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorver(e);
        }
        let recv = self.coagir(recv, Type::Ref);
        let env = self.emit(Instruction::AllocEnv { values: vec![recv.clone()] }, Type::Ref);
        let c = self.emit(
            Instruction::AllocClosure {
                code_symbol: simbolo_ent,
                env,
            },
            Type::Ref,
        );
        self.definir_rti_de_tearoff(c.clone(), fid, Some(recv));
        c
    }

    /// O que um nome sem resolução (corpo de closure) é pelo escopo léxico,
    /// depois dos locais: membro da classe envolvente (subindo as
    /// superclasses do programa), senão o escopo da biblioteca. Devolve a
    /// resolução que a inferência teria gravado, para os caminhos de sempre.
    pub fn resolver_por_nome(&self, sym: SymbolId) -> Option<dartforge_types::resolved::Resolved> {
        use dartforge_types::resolved::{MemberRef, Resolved};
        // No corpo de uma extensão, os membros dela vêm antes dos do tipo
        // `on` (o escopo léxico da extensão envolve o corpo).
        if let Some((e, _)) = self.extensao_do_this {
            let x = &self.ctx.program.extensions[e.0 as usize];
            if let Some(&f) = x.instance_members.get(&sym).or_else(|| x.static_members.get(&sym)) {
                return Some(Resolved::ExtensionMember { extension: e, member: f });
            }
        }
        // No corpo de um tipo de extensão, os membros dele (e dos tipos de
        // extensão que ele implementa).
        if let Some(r) = self.membro_te_por_nome(sym) {
            return Some(r);
        }
        for c in self.enclosing_class.map(|c| crate::lower::membros::linearizacao(self.ctx, c)).unwrap_or_default() {
            let classe = &self.ctx.program.classes[c.0 as usize];
            if !self.ctx.biblioteca_compilada(classe.library) {
                break;
            }
            if let Some(&f) = classe.instance_members.get(&sym).or_else(|| classe.static_members.get(&sym)) {
                return Some(Resolved::Member {
                    class: c,
                    member: MemberRef::Function(f),
                    via_super: false,
                });
            }
            if let Some(&v) = classe
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
            if let Some(v) = constante_de_enum(self.ctx, c, sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
        }
        // Num membro estático, os estáticos da classe que o declara.
        if self.enclosing_class.is_none()
            && let Some(c) = self.classe_do_membro
        {
            let classe = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = classe.static_members.get(&sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Function(f), via_super: false });
            }
            if let Some(&v) = classe.fields.iter().find(|&&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                var.static_ && var.name == sym
            }) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
            if let Some(v) = constante_de_enum(self.ctx, c, sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        let b = self.ctx.program.lookup(lib, sym)?;
        b.getter.or(b.setter).map(Resolved::Element)
    }

    /// Lê um nome sem resolução que não é local (ver `resolver_por_nome`).
    pub fn ler_nome_sem_resolucao(&mut self, sym: SymbolId, span: Span) -> Option<Operand> {
        use dartforge_types::resolved::Resolved;
        match self.resolver_por_nome(sym)? {
            r @ Resolved::Member { .. } if self.membro_te_resolvido(Some(&r)).is_some() => {
                let m = self.membro_te_resolvido(Some(&r))?;
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                let receptor = self.receptor_implicito();
                Some(self.ler_membro_te(this, m, receptor, span))
            }
            Resolved::Member { member, .. } => Some(self.ler_membro_implicito(member, span)),
            Resolved::Element(el) => Some(self.ler_elemento(el, span)),
            Resolved::ExtensionMember { member, .. } => {
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                let receptor = self.receptor_implicito();
                Some(self.ler_extensao(this, member.0 as usize, receptor, span))
            }
            _ => None,
        }
    }

    /// `f<T…>`: a closure de uma função genérica com os argumentos de tipo
    /// fixos (`tupla`, no ambiente). A entrada arma a tupla e chama a
    /// função; o tipo reificado é o da instanciação (`tipo`, estático).
    pub fn tearoff_instanciado(&mut self, fid: usize, tupla: Operand, tipo: Option<TypeId>) -> Operand {
        let alvo = super::simbolo_de(self.ctx, fid);
        let simbolo_ent = format!("{alvo}$teari");
        if !self.entradas_feitas.contains(&simbolo_ent) {
            self.entradas_feitas.insert(simbolo_ent.clone());
            let infos = self.params_da_funcao(fid);
            let nome = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, simbolo_ent.clone(), nome, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param("args".to_string(), Type::Ptr));
            let desc = Operand::Val(e.add_param("desc".to_string(), Type::Ptr));
            let env = e.emit(
                Instruction::CallRuntime {
                    name: "dartforge_closure_env".to_string(),
                    args: vec![(clo, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            let t = e.emit(Instruction::EnvGet { env, index: 0 }, Type::I64);
            if let Some(vals) = e.desempacotar(&infos, args, desc) {
                // Os argumentos de uma chamada dinâmica, conferidos como na
                // VM (" of 'nome'") antes de convertidos.
                e.conferir_argumentos_da_entrada(fid, &vals);
                let reprs: Vec<Type> = self.ctx.outline.functions[fid].parameters.iter().map(|p| e.repr(p.ty)).collect();
                let vals: Vec<Operand> = vals.into_iter().zip(reprs).map(|(v, r)| e.coagir(v, r)).collect();
                e.tupla_armada = Some(t);
                let r = e.chamar_direto(fid, None, vals);
                let r = if matches!(e.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    e.coagir(r, Type::Ref)
                };
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorver(e);
        }
        let env = self.emit(Instruction::AllocEnv { values: vec![tupla] }, Type::Ref);
        let c = self.emit(Instruction::AllocClosure { code_symbol: simbolo_ent, env }, Type::Ref);
        self.definir_rti_de_instanciacao(c.clone(), fid, tipo);
        c
    }

    /// `C<T…>.new`: como `tearoff_de_construtor`, com o tipo do objeto
    /// (`objeto`, `C<T…>`) e a tupla da classe (`tupla`, para uma factory)
    /// no ambiente.
    pub fn tearoff_instanciado_de_construtor(
        &mut self,
        fid: usize,
        objeto: Operand,
        tupla: Operand,
        tipo: Option<TypeId>,
        span: Span,
    ) -> Operand {
        let alvo = super::simbolo_de(self.ctx, fid);
        let simbolo_ent = format!("{alvo}$teari");
        if !self.entradas_feitas.contains(&simbolo_ent) {
            self.entradas_feitas.insert(simbolo_ent.clone());
            let infos = self.params_da_funcao(fid);
            let nome = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, simbolo_ent.clone(), nome, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param("args".to_string(), Type::Ptr));
            let desc = Operand::Val(e.add_param("desc".to_string(), Type::Ptr));
            let env = e.emit(
                Instruction::CallRuntime {
                    name: "dartforge_closure_env".to_string(),
                    args: vec![(clo, Type::Ref)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            let obj = e.emit(Instruction::EnvGet { env: env.clone(), index: 0 }, Type::I64);
            let t = e.emit(Instruction::EnvGet { env, index: 1 }, Type::I64);
            if let Some(vals) = e.desempacotar(&infos, args, desc) {
                let avaliados: Vec<Avaliado> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.instanciar_avaliados_com_rti(
                    dartforge_elements::model::FunctionElementId(fid as u32),
                    &avaliados,
                    span,
                    Some(obj),
                    Some(t),
                );
                let r = e.coagir(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorver(e);
        }
        let env = self.emit(Instruction::AllocEnv { values: vec![objeto, tupla] }, Type::Ref);
        let c = self.emit(Instruction::AllocClosure { code_symbol: simbolo_ent, env }, Type::Ref);
        self.definir_rti_de_instanciacao(c.clone(), fid, tipo);
        c
    }

    /// O tipo reificado de um tear-off instanciado: o tipo estático da
    /// expressão (a assinatura com os argumentos de tipo aplicados); sem
    /// ele, a assinatura declarada.
    fn definir_rti_de_instanciacao(&mut self, clo: Operand, fid: usize, tipo: Option<TypeId>) {
        match tipo {
            Some(t) if matches!(self.ctx.table.get(t), dartforge_types::table::Type::Function { type_params, .. } if type_params.is_empty()) => {
                let r = self.rti_de_tipo(t);
                self.definir_rti(clo, r);
            }
            _ => self.definir_rti_de_tearoff(clo, fid, None),
        }
    }

    /// A resolução de uma expressão, ou (identificador sem resolução, como
    /// num corpo de closure) a do nome pelo escopo léxico.
    pub fn resolucao_ou_nome(&self, ast: &ast::Ast, e: ast::ExprId) -> Option<dartforge_types::resolved::Resolved> {
        if let Some(r) = self.ctx.get_resolved(self.unit_id, e).cloned() {
            return Some(r);
        }
        match &ast.expr(e).kind {
            ast::ExprKind::Identifier(n) if self.buscar_local(n.sym).is_none() => self.resolver_por_nome(n.sym),
            _ => None,
        }
    }

    /// A função genérica (de topo ou estática, do programa) que `alvo`
    /// nomeia, para `alvo<T…>`.
    pub fn funcao_generica_do_alvo(&self, ast: &ast::Ast, alvo: ast::ExprId) -> Option<usize> {
        use dartforge_types::resolved::{MemberRef, Resolved};
        let fid = match self.resolucao_ou_nome(ast, alvo)? {
            Resolved::Element(dartforge_elements::model::Element::Function(f)) => f.0 as usize,
            Resolved::Member { member: MemberRef::Function(f), .. } => f.0 as usize,
            _ => return None,
        };
        let f = &self.ctx.program.functions[fid];
        let livre = f.class.is_none() || f.static_;
        (livre && f.variable.is_none() && f.kind == dartforge_elements::model::FunctionKind::Function && self.funcao_generica(fid))
            .then_some(fid)
    }

    /// As receitas dos argumentos de tipo escritos (`<int, T>`), unidas por
    /// vírgula (`dynamic` para o que não resolve).
    pub fn receitas_dos_argumentos_de_tipo(&self, args: &[ast::TypeId]) -> super::rti::Receita {
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let mut r = super::rti::Receita { texto: String::new(), variaveis: false };
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                r.texto.push(',');
            }
            match self.receita_da_anotacao(unit_ast.ty(*a)) {
                Some(x) => {
                    r.texto.push_str(&x.texto);
                    r.variaveis |= x.variaveis;
                }
                None => r.texto.push('D'),
            }
        }
        r
    }

    /// Tear-off de construtor (`C.new`, `C.nome`): canônico, e a entrada
    /// constrói o objeto com os argumentos recebidos.
    /// Tear-off genérico do construtor `fid` da classe `c` cujo tipo estático
    /// é `C<A…> Function<P…>(…)` (`tipo`, os `P` em `params`, os `A` em
    /// `args`): a chamada passa os `P` no slot oculto, e o objeto nasce
    /// `C<A…>` com `P` trocado por eles (um alias `typedef F<T> = C<int, T>`
    /// dá `C<int, T>`).
    pub fn tearoff_generico_de_construtor(
        &mut self,
        fid: usize,
        c: dartforge_elements::model::ClassId,
        params: &[dartforge_types::table::TypeParamId],
        args: &[TypeId],
        tipo: TypeId,
        span: Span,
    ) -> Operand {
        let vars: Vec<String> = (0..params.len()).map(|i| format!("M{i}")).collect();
        let textos: Vec<String> = args.iter().map(|a| self.receita_de_tipo_com(*a, params, &vars).texto).collect();
        let objeto_texto = format!("C{}<{}>", self.ctx.id_rti(c), textos.join(","));
        let tupla_texto = format!("L<{}>", textos.join(","));
        let alvo = super::simbolo_de(self.ctx, fid);
        let simbolo_ent = format!("{alvo}$teara{:08x}", (hash_nome(&objeto_texto) as u64) as u32);
        if !self.entradas_feitas.contains(&simbolo_ent) {
            self.entradas_feitas.insert(simbolo_ent.clone());
            let infos = self.params_da_funcao(fid);
            let nome = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, simbolo_ent.clone(), nome, Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let a = Operand::Val(e.add_param("args".to_string(), Type::Ptr));
            let d = Operand::Val(e.add_param("desc".to_string(), Type::Ptr));
            if let Some(vals) = e.desempacotar(&infos, a.clone(), d.clone()) {
                let avaliados: Vec<Avaliado> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let slot = e.tupla_do_slot(&a, &d);
                e.tupla_de_tipos = Some(slot);
                let objeto = e.rti_da_receita(&super::rti::Receita { texto: objeto_texto.clone(), variaveis: true });
                let tupla = e.rti_da_receita(&super::rti::Receita { texto: tupla_texto.clone(), variaveis: true });
                let r = e.instanciar_avaliados_com_rti(
                    dartforge_elements::model::FunctionElementId(fid as u32),
                    &avaliados,
                    span,
                    Some(objeto),
                    Some(tupla),
                );
                let r = e.coagir(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorver(e);
        }
        let t = self.emit(Instruction::TearOff { code_symbol: simbolo_ent }, Type::Ref);
        // A assinatura é o tipo estático (a função genérica).
        let r = self.receita_de_tipo(tipo);
        let rti = self.rti_da_receita(&r);
        self.definir_rti(t.clone(), rti);
        t
    }

    pub fn tearoff_de_construtor(&mut self, fid: usize, span: Span) -> Operand {
        let alvo = super::simbolo_de(self.ctx, fid);
        let simbolo_ent = format!("{alvo}$tear");
        if !self.entradas_feitas.contains(&simbolo_ent) {
            self.entradas_feitas.insert(simbolo_ent.clone());
            let infos = self.params_da_funcao(fid);
            let nome = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, simbolo_ent.clone(), nome, Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let args = Operand::Val(e.add_param("args".to_string(), Type::Ptr));
            let desc = Operand::Val(e.add_param("desc".to_string(), Type::Ptr));
            if let Some(vals) = e.desempacotar(&infos, args.clone(), desc.clone()) {
                let avaliados: Vec<Avaliado> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                // Construtor de classe genérica: o tear-off é uma função
                // genérica nos parâmetros da classe (`C<T> Function<T>()`),
                // e os argumentos de tipo da chamada vêm no slot oculto
                // (nenhum: `dynamic`).
                let classe = self.ctx.program.functions[fid].class;
                let n = classe.and_then(|c| self.ctx.outline.classes.get(c.0 as usize)).map_or(0, |d| d.type_params.len());
                let r = match classe.filter(|_| n > 0) {
                    Some(c) => {
                        let tupla = e.tupla_do_slot(&args, &desc);
                        e.tupla_de_tipos = Some(tupla.clone());
                        let vars: Vec<String> = (0..n).map(|i| format!("M{i}")).collect();
                        let objeto = e.rti_da_receita(&super::rti::Receita {
                            texto: format!("C{}<{}>", self.ctx.id_rti(c), vars.join(",")),
                            variaveis: true,
                        });
                        e.instanciar_avaliados_com_rti(
                            dartforge_elements::model::FunctionElementId(fid as u32),
                            &avaliados,
                            span,
                            Some(objeto),
                            Some(tupla),
                        )
                    }
                    None => e.instanciar_avaliados(dartforge_elements::model::FunctionElementId(fid as u32), &avaliados, span),
                };
                let r = e.coagir(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorver(e);
        }
        let t = self.emit(
            Instruction::TearOff {
                code_symbol: simbolo_ent,
            },
            Type::Ref,
        );
        // O construtor também é uma função reificada. Sem a assinatura, o
        // `current as E` do ListIterator rejeita um tear-off guardado em
        // `List<C Function()>` quando E passa a ser propagado pelo SDK.
        self.definir_rti_de_tearoff(t.clone(), fid, None);
        t
    }
}

/// A constante `sym` do `enum` `c`, se `c` é um enum e a declara.
fn constante_de_enum(
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
