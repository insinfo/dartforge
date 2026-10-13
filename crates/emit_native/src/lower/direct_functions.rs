//! Funções locais que não escapam, chamadas diretamente (N17,
//! docs/NATIVO-PLANO.md §9).
//!
//! **O problema.** Uma função local (`void skipWS() { … }` dentro de um
//! método) era sempre um valor: a cada execução da declaração, um ambiente
//! com as capturas, a closure e — porque o nome conta como atribuído —
//! uma célula para o nome; cada chamada ia pela entrada uniforme da
//! closure. O `_HeaderValue._parse` do `dart:_http` declara seis (oito com
//! as de dentro de `parseParameters`) por chamada, e o servidor HTTP fazia
//! 103 closures e 103 ambientes por requisição.
//!
//! **A regra.** O que a VM faz no grafo de fluxo (as closures que não
//! escapam são chamadas estaticamente e, quando dá, embutidas) e o que o
//! CFE garante: uma função local cujo nome só aparece como alvo de chamada
//! `f(…)` — nunca lida como valor, passada, guardada ou devolvida — e só de
//! dentro da função que a declara ou de outras funções diretas não pode ser
//! chamada depois de a função de fora retornar. A análise está em
//! `capture.rs` (`analyze_with`): as referências, a forma (sem parâmetros
//! de tipo próprios, síncrona, só posicionais obrigatórios, sem ler um
//! `late` de fora) e o ponto fixo, porque uma função que deixa de ser
//! direta vira closure e o que ela chama também passa a escapar por ela.
//! A função local genérica fica closure: a tupla dos argumentos de tipo
//! dela (`$types`) vem da entrada uniforme.
//!
//! **O código.** A função direta vira uma função do módulo,
//! `<de outside>$<name>$d<impressão>`, com os parâmetros
//! `[this] captures… [tuple de types de outside] parâmetros…`, e a chamada é um
//! `call` estático — sem closure, ambiente nem célula do nome. Cada
//! captura vai de um de três jeitos, decididos na declaração:
//!
//! * **valor** — a variável nunca é atribuída (o parâmetro, o `final`, a
//!   `var` só inicializada): o valor lido na hora da chamada;
//! * **endereço** — atribuída, e nenhuma função que escapa a captura: o
//!   endereço do `alloca` de quem declara, e a função lê e grava por ele (o
//!   `index` do `_parse`). A função direta só roda enquanto o quadro de quem
//!   a declara existe. O `alloca` de um `Ref` cujo endereço sai assim mora
//!   no próprio slot do quadro de raízes (`llvm/mod.rs`,
//!   `frame_allocas`): o que a função direta grava fica enraizado, sem
//!   célula (o `listenerValueOrError` e o `source` do
//!   `_Future._propagateToListeners`);
//! * **célula** — também capturada por uma closure que escapa: o handle da
//!   célula de sempre.
//!
//! Uma função direta que chama outra declarada fora dela recebe também as
//! capturas dela (e as repassa): a chamada nunca procura uma captura pelo
//! nome, que um bloco de dentro pode ter sombreado.

use super::fn_builder::FnBuilder;
use super::locals::{Local, Mode};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, FunctionBody, FunctionId};
use dartforge_intern::SymbolId;

/// Como uma captura chega à função direta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Passing {
    Value,
    Address,
    Cell,
}

/// Uma captura de uma função direta: o nome, como passa, a representação
/// da variável e onde ela mora no construtor que registrou a função.
#[derive(Clone, Debug)]
pub struct Capture {
    pub sym: SymbolId,
    pub passing: Passing,
    pub ty: Type,
    pub local: Local,
}

/// Uma função direta visível no construtor corrente.
#[derive(Clone, Debug)]
pub struct Direct {
    pub symbol: String,
    pub with_this: bool,
    pub with_tuple: bool,
    pub captures: Vec<Capture>,
    pub reprs: Vec<Type>,
    pub ret: Type,
}

/// A identidade de onde uma variável mora (para juntar as capturas de
/// funções diferentes que são a mesma variável).
fn identity(l: &Local) -> String {
    format!("{:?}", l.mode)
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// A declaração `f` é de uma função local direta (`capture.rs`).
    pub fn direct_local_function(&self, fid: FunctionId) -> bool {
        self.allowed_directs.contains(&fid)
    }

    /// Como a variável `l` passa para uma função direta declarada aqui.
    fn passing_of(&self, l: &Local) -> Passing {
        match &l.mode {
            Mode::Cell(_) | Mode::Environment { cell: true, .. } => Passing::Cell,
            Mode::Memory(_)
                if l.offset.is_some_and(|o| self.assigned.contains(&o) || self.pointers.contains(&o)) =>
            {
                Passing::Address
            }
            _ => Passing::Value,
        }
    }

    /// `R f(params) { … }` local e direta: baixa o corpo como função do
    /// módulo e registra o nome (sem valor: nunca é lido).
    pub fn declare_direct_function(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span) {
        let f = ast.function(fid);
        let Some(name) = f.name else { return };
        // O nome, visível no escopo (a chamada o acha pelo offset); o valor
        // nunca é lido.
        self.declare_variable(name.sym, name.span.start as usize, Type::Ref, Operand::Constant(Constant::Null));

        // As capturas: as variáveis livres visíveis aqui, e as das funções
        // diretas que o corpo chama (repassadas).
        let (free_set, _) = super::capture::free_set(self.ctx, self.unit_id, ast, fid);
        let mut captures: Vec<Capture> = Vec::new();
        let mut calls: Vec<(SymbolId, usize)> = Vec::new();
        for s in free_set {
            let Some(l) = self.lookup_local(s) else { continue };
            if let Some(o) = l.offset
                && (self.direct_functions.contains_key(&o) || o == name.span.start as usize)
            {
                calls.push((s, o));
                continue;
            }
            let passing = self.passing_of(&l);
            if !captures.iter().any(|c| identity(&c.local) == identity(&l)) {
                captures.push(Capture { sym: s, passing, ty: l.ty, local: l });
            }
        }
        for (_, o) in &calls {
            if let Some(d) = self.direct_functions.get(o) {
                for c in d.captures.clone() {
                    if !captures.iter().any(|x| identity(&x.local) == identity(&c.local)) {
                        captures.push(c);
                    }
                }
            }
        }
        let with_this = self.this_param.is_some();
        let with_tuple = self.type_tuple.is_some();
        let params = f.parameters.as_deref().unwrap_or(&[]);
        let ty = self.literal_function_type(ast, fid, None);
        let (reprs, ret) = match ty.and_then(|t| self.type_abi(t)) {
            Some((ps, r, _)) if ps.len() == params.len() => (ps, r),
            _ => (vec![Type::Ref; params.len()], Type::Ref),
        };
        // O nome leva a impressão digital da assinatura (como o corpo de
        // uma closure, `closures.rs`): numa recarga do JIT, quem chama
        // uma versão com outras capturas chama outro símbolo.
        let mut shape = format!("{with_this}|{with_tuple}|");
        for c in &captures {
            shape.push_str(&format!("{}:{:?}:{:?};", self.ctx.symbol_name(c.sym), c.passing, c.ty));
        }
        shape.push_str(&format!("{reprs:?}->{ret:?}"));
        let base = format!("{}${}", self.func.symbol, super::sanitize_symbol(self.ctx.symbol_name(name.sym)));
        let mut symbol = base.clone();
        let mut k = 1;
        while !self.local_names.insert(symbol.clone()) {
            symbol = format!("{base}${k}");
            k += 1;
        }
        let symbol = format!("{symbol}$d{:08x}", (super::closures::hash_name(&shape) as u64) as u32);
        let direct = Direct { symbol: symbol.clone(), with_this, with_tuple, captures, reprs, ret };
        self.direct_functions.insert(name.span.start as usize, direct.clone());

        // --- corpo ---------------------------------------------------------
        let readable = self.ctx.symbol_name(name.sym).to_string();
        let mut b = FnBuilder::new(self.ctx, self.unit_id, symbol, readable, ret);
        b.open_scope();
        if with_this {
            let t = b.add_param("this".to_string(), Type::Ref);
            b.this_param = Some(Operand::Val(t));
            b.finalizable_this = self.finalizable_this;
            b.enclosing_class = self.enclosing_class;
        }
        // Cada captura, ligada no corpo; a função repassa o mesmo lugar às
        // diretas que ela chama.
        let mut linked_set: Vec<(String, Local)> = Vec::new();
        for c in &direct.captures {
            let name_p = self.ctx.symbol_name(c.sym).to_string();
            let local = match c.passing {
                Passing::Value => {
                    let v = b.add_param_dart(name_p, c.ty);
                    Local { mode: Mode::Value(Operand::Val(v)), ty: c.ty, static_type: c.local.static_type, finalizable: c.local.finalizable, scope: 0, offset: c.local.offset, late: None }
                }
                Passing::Address => {
                    let v = b.add_param(name_p, Type::Ptr);
                    if let Some(o) = c.local.offset {
                        b.pointers.insert(o);
                    }
                    Local { mode: Mode::Memory(Operand::Val(v)), ty: c.ty, static_type: c.local.static_type, finalizable: c.local.finalizable, scope: 0, offset: c.local.offset, late: None }
                }
                Passing::Cell => {
                    let v = b.add_param(name_p, Type::Ref);
                    let ptr = b.entry_alloca(Type::Ref);
                    b.emit(Instruction::Store { ptr: ptr.clone(), val: Operand::Val(v) }, Type::Void);
                    Local { mode: Mode::Cell(ptr), ty: c.ty, static_type: c.local.static_type, finalizable: c.local.finalizable, scope: 0, offset: c.local.offset, late: None }
                }
            };
            linked_set.push((identity(&c.local), local.clone()));
            b.bind_local_as(c.sym, local);
        }
        if with_tuple {
            let t = b.add_param_rti("$tipos_de_fora".to_string());
            b.type_tuple = Some(Operand::Val(t));
        }
        b.function_type_params = self.function_type_params.clone();
        b.local_params = self.local_params.clone();
        b.this_extension = self.this_extension;
        b.this_ext_type = self.this_ext_type;
        b.member_class = self.member_class;
        b.class_by_tuple = self.class_by_tuple;
        if self.class_by_tuple {
            b.enclosing_class = self.enclosing_class;
        }
        // As diretas que o corpo chama (a própria, na recursão), com as
        // capturas trocadas pelas ligações de lá.
        for (s, o) in &calls {
            let Some(d) = self.direct_functions.get(o) else { continue };
            let mut d = d.clone();
            for c in d.captures.iter_mut() {
                if let Some((_, l)) = linked_set.iter().find(|(id, _)| *id == identity(&c.local)) {
                    c.local = l.clone();
                }
            }
            b.declare_variable(*s, *o, Type::Ref, Operand::Constant(Constant::Null));
            b.direct_functions.insert(*o, d);
        }
        b.prepare_captures(
            ast,
            super::capture::Root { parameters: params, body: Some(&f.body), initializers: &[] },
        );
        b.open_scope();
        for (i, p) in params.iter().enumerate() {
            let name_p = p.name.map_or_else(|| "arg".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());
            let v = b.add_param_dart(name_p, direct.reprs[i]);
            if let Some(n) = p.name {
                b.declare_variable(n.sym, n.span.start as usize, direct.reprs[i], Operand::Val(v));
            }
        }
        match &f.body {
            FunctionBody::Block(s) => b.lower_stmt(ast, *s),
            FunctionBody::Expression(e) => {
                let r = b.lower_expr(ast, *e);
                if !b.is_terminated() {
                    let r = b.coerce(r, ret);
                    b.terminate(Terminator::Return(Some(r)));
                }
            }
            _ => {
                b.unsupported("função local direta sem corpo", span);
            }
        }
        if self.ctx.arc_memory {
            self.arc_direct_local_functions.insert(b.func.symbol.clone());
            // Corpo Dart: o retorno Ref é Owned, como o de uma função.
            if b.func.return_ty == Type::Ref {
                b.dart_ref_returns.insert(b.func.symbol.clone());
            }
        }
        self.absorb(b);
    }

    /// A função direta registrada no local `l` (o nome de uma função
    /// local direta), se é uma.
    pub fn local_direct(&self, l: &Local) -> Option<Direct> {
        self.direct_functions.get(&l.offset?).cloned()
    }

    /// `f(evaluated)` de uma função direta: `call` estático com as
    /// capturas lidas agora (o valor, o endereço ou a célula).
    pub fn call_direct_fn(&mut self, d: &Direct, evaluated: &[super::members::Evaluated]) -> Operand {
        let mut args = Vec::with_capacity(d.captures.len() + evaluated.len() + 2);
        if d.with_this {
            let t = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
            args.push(self.coerce(t, Type::Ref));
        }
        for c in &d.captures {
            let v = match c.passing {
                Passing::Value => {
                    let v = self.read_local(&c.local);
                    self.coerce(v, c.ty)
                }
                Passing::Address => match &c.local.mode {
                    Mode::Memory(ptr) => ptr.clone(),
                    _ => unreachable!("captura por endereço fora da memória"),
                },
                Passing::Cell => self.local_cell(&c.local).unwrap_or(Operand::Constant(Constant::Null)),
            };
            args.push(v);
        }
        if d.with_tuple {
            args.push(self.type_tuple.clone().unwrap_or(Operand::Constant(Constant::Int(0))));
        }
        for (i, (_, v)) in evaluated.iter().enumerate() {
            let ty = d.reprs.get(i).copied().unwrap_or(Type::Ref);
            args.push(self.coerce(v.clone(), ty));
        }
        self.emit_call_with_check(Instruction::CallStatic { symbol: d.symbol.clone(), args, ret_ty: d.ret }, d.ret)
    }
}
