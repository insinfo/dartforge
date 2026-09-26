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
//! os parâmetros e o retorno `Ref` (o corpo de uma closure ainda não é
//! inferido: tudo nele é `dynamic`). O ambiente guarda, nesta ordem, `this`
//! (quando a closure nasce num membro de instância) e as variáveis livres —
//! o handle da célula das que moram numa (`captura.rs`), ou a cópia do valor.
//!
//! A closure é `AllocClosure { código, env }`; o código é o índice da entrada
//! em `@df_code_table`, que o emissor monta. O tear-off de função de topo ou
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

    /// `(params) => e`, `(params) { … }` ou o valor de uma função local.
    pub fn lower_closure(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span) -> Operand {
        let f = ast.function(fid);
        // Variáveis livres que são locais visíveis aqui.
        let (livres, _) = captura::livres(self.ctx, self.unit_id, ast, fid);
        let capturas: Vec<(SymbolId, super::locais::Local)> = livres
            .into_iter()
            .filter_map(|s| self.buscar_local(s).map(|l| (s, l)))
            .collect();
        let com_this = self.this_param.is_some();
        let base = usize::from(com_this);

        let simbolo = self.nome_de_closure(f.name.map(|n| n.sym));
        let legivel = f
            .name
            .map_or_else(|| "<closure>".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());

        // --- corpo ---------------------------------------------------------
        let mut b = FnBuilder::new(self.ctx, self.unit_id, simbolo.clone(), legivel.clone(), Type::Ref);
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
        b.extensao_do_this = self.extensao_do_this;
        b.classe_por_tupla = self.classe_por_tupla;
        if self.classe_por_tupla {
            // Numa fábrica não há `this`, mas `T` é o da classe.
            b.enclosing_class = self.enclosing_class;
        }
        if self.tupla_de_tipos.is_some() {
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
        for p in params {
            let nome = p
                .name
                .map_or_else(|| "arg".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());
            let vid = b.add_param(nome, Type::Ref);
            if let Some(n) = p.name {
                b.declarar_variavel(n.sym, n.span.start as usize, Type::Ref, Operand::Val(vid));
            }
        }
        if f.modifier != AsyncModifier::None {
            // P6: closure `async`, `sync*` ou `async*` — o corpo vira máquina
            // de estados. O tipo do elemento/valor fica `dynamic`.
            b.lower_corpo_async(ast, params, &f.body, span, None, super::async_sm::tipo_do_corpo(f.modifier));
        } else {
            match &f.body {
                FunctionBody::Block(s) => b.lower_stmt(ast, *s),
                FunctionBody::Expression(e) => {
                    let r = b.lower_expr(ast, *e);
                    b.terminate(Terminator::Return(Some(r)));
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
        if let Some(vals) = e.desempacotar(&infos, args, desc) {
            let mut todos = vec![env_e];
            todos.extend(vals);
            let r = e.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: simbolo,
                    args: todos,
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            e.terminate(Terminator::Return(Some(r)));
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
        self.emit(
            Instruction::AllocClosure {
                code_symbol: simbolo_ent,
                env,
            },
            Type::Ref,
        )
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
        let clo = self.lower_closure(ast, fid, span);
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
        let callee = self.coagir(callee, Type::Ref);
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
            },
            Type::Ref,
        )
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
            if let Some(vals) = e.desempacotar(&infos, args, desc) {
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
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        let b = self.ctx.program.lookup(lib, sym)?;
        b.getter.or(b.setter).map(Resolved::Element)
    }

    /// Lê um nome sem resolução que não é local (ver `resolver_por_nome`).
    pub fn ler_nome_sem_resolucao(&mut self, sym: SymbolId, span: Span) -> Option<Operand> {
        use dartforge_types::resolved::Resolved;
        match self.resolver_por_nome(sym)? {
            Resolved::Member { member, .. } => Some(self.ler_membro_implicito(member, span)),
            Resolved::Element(el) => Some(self.ler_elemento(el, span)),
            Resolved::ExtensionMember { member, .. } => {
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                let receptor = self.extensao_do_this.map(|(_, on)| on);
                Some(self.ler_extensao(this, member.0 as usize, receptor, span))
            }
            _ => None,
        }
    }

    /// Tear-off de construtor (`C.new`, `C.nome`): canônico, e a entrada
    /// constrói o objeto com os argumentos recebidos.
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
            if let Some(vals) = e.desempacotar(&infos, args, desc) {
                let avaliados: Vec<Avaliado> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.instanciar_avaliados(dartforge_elements::model::FunctionElementId(fid as u32), &avaliados, span);
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
