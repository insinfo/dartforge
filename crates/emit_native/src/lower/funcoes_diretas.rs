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
//! `captura.rs` (`analisar_com`): as referências, a forma (sem parâmetros
//! de tipo próprios, síncrona, só posicionais obrigatórios, sem ler um
//! `late` de fora) e o ponto fixo, porque uma função que deixa de ser
//! direta vira closure e o que ela chama também passa a escapar por ela.
//! A função local genérica fica closure: a tupla dos argumentos de tipo
//! dela (`$tipos`) vem da entrada uniforme.
//!
//! **O código.** A função direta vira uma função do módulo,
//! `<de fora>$<nome>$d<impressão>`, com os parâmetros
//! `[this] capturas… [tupla de tipos de fora] parâmetros…`, e a chamada é um
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
//!   `allocas_no_quadro`): o que a função direta grava fica enraizado, sem
//!   célula (o `listenerValueOrError` e o `source` do
//!   `_Future._propagateToListeners`);
//! * **célula** — também capturada por uma closure que escapa: o handle da
//!   célula de sempre.
//!
//! Uma função direta que chama outra declarada fora dela recebe também as
//! capturas dela (e as repassa): a chamada nunca procura uma captura pelo
//! nome, que um bloco de dentro pode ter sombreado.

use super::fn_builder::FnBuilder;
use super::locais::{Local, Modo};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, FunctionBody, FunctionId};
use dartforge_intern::SymbolId;

/// Como uma captura chega à função direta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Passagem {
    Valor,
    Endereco,
    Celula,
}

/// Uma captura de uma função direta: o nome, como passa, a representação
/// da variável e onde ela mora no construtor que registrou a função.
#[derive(Clone, Debug)]
pub struct Captura {
    pub sym: SymbolId,
    pub passagem: Passagem,
    pub ty: Type,
    pub local: Local,
}

/// Uma função direta visível no construtor corrente.
#[derive(Clone, Debug)]
pub struct Direta {
    pub simbolo: String,
    pub com_this: bool,
    pub com_tupla: bool,
    pub capturas: Vec<Captura>,
    pub reprs: Vec<Type>,
    pub ret: Type,
}

/// A identidade de onde uma variável mora (para juntar as capturas de
/// funções diferentes que são a mesma variável).
fn identidade(l: &Local) -> String {
    format!("{:?}", l.modo)
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// A declaração `f` é de uma função local direta (`captura.rs`).
    pub fn funcao_local_direta(&self, fid: FunctionId) -> bool {
        self.diretas_permitidas.contains(&fid)
    }

    /// Como a variável `l` passa para uma função direta declarada aqui.
    fn passagem_de(&self, l: &Local) -> Passagem {
        match &l.modo {
            Modo::Celula(_) | Modo::Ambiente { celula: true, .. } => Passagem::Celula,
            Modo::Memoria(_)
                if l.offset.is_some_and(|o| self.atribuidos.contains(&o) || self.ponteiros.contains(&o)) =>
            {
                Passagem::Endereco
            }
            _ => Passagem::Valor,
        }
    }

    /// `R f(params) { … }` local e direta: baixa o corpo como função do
    /// módulo e registra o nome (sem valor: nunca é lido).
    pub fn declarar_funcao_direta(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span) {
        let f = ast.function(fid);
        let Some(nome) = f.name else { return };
        // O nome, visível no escopo (a chamada o acha pelo offset); o valor
        // nunca é lido.
        self.declarar_variavel(nome.sym, nome.span.start as usize, Type::Ref, Operand::Constant(Constant::Null));

        // As capturas: as variáveis livres visíveis aqui, e as das funções
        // diretas que o corpo chama (repassadas).
        let (livres, _) = super::captura::livres(self.ctx, self.unit_id, ast, fid);
        let mut capturas: Vec<Captura> = Vec::new();
        let mut chamadas: Vec<(SymbolId, usize)> = Vec::new();
        for s in livres {
            let Some(l) = self.buscar_local(s) else { continue };
            if let Some(o) = l.offset
                && (self.funcoes_diretas.contains_key(&o) || o == nome.span.start as usize)
            {
                chamadas.push((s, o));
                continue;
            }
            let passagem = self.passagem_de(&l);
            if !capturas.iter().any(|c| identidade(&c.local) == identidade(&l)) {
                capturas.push(Captura { sym: s, passagem, ty: l.ty, local: l });
            }
        }
        for (_, o) in &chamadas {
            if let Some(d) = self.funcoes_diretas.get(o) {
                for c in d.capturas.clone() {
                    if !capturas.iter().any(|x| identidade(&x.local) == identidade(&c.local)) {
                        capturas.push(c);
                    }
                }
            }
        }
        let com_this = self.this_param.is_some();
        let com_tupla = self.tupla_de_tipos.is_some();
        let params = f.parameters.as_deref().unwrap_or(&[]);
        let tipo = self.tipo_da_funcao_literal(ast, fid, None);
        let (reprs, ret) = match tipo.and_then(|t| self.abi_do_tipo(t)) {
            Some((ps, r, _)) if ps.len() == params.len() => (ps, r),
            _ => (vec![Type::Ref; params.len()], Type::Ref),
        };
        // O nome leva a impressão digital da assinatura (como o corpo de
        // uma closure, `closures.rs`): numa recarga do JIT, quem chama
        // uma versão com outras capturas chama outro símbolo.
        let mut forma = format!("{com_this}|{com_tupla}|");
        for c in &capturas {
            forma.push_str(&format!("{}:{:?}:{:?};", self.ctx.symbol_name(c.sym), c.passagem, c.ty));
        }
        forma.push_str(&format!("{reprs:?}->{ret:?}"));
        let base = format!("{}${}", self.func.symbol, super::sanitize_symbol(self.ctx.symbol_name(nome.sym)));
        let mut simbolo = base.clone();
        let mut k = 1;
        while !self.nomes_locais.insert(simbolo.clone()) {
            simbolo = format!("{base}${k}");
            k += 1;
        }
        let simbolo = format!("{simbolo}$d{:08x}", (super::closures::hash_nome(&forma) as u64) as u32);
        let direta = Direta { simbolo: simbolo.clone(), com_this, com_tupla, capturas, reprs, ret };
        self.funcoes_diretas.insert(nome.span.start as usize, direta.clone());

        // --- corpo ---------------------------------------------------------
        let legivel = self.ctx.symbol_name(nome.sym).to_string();
        let mut b = FnBuilder::new(self.ctx, self.unit_id, simbolo, legivel, ret);
        b.abrir_escopo();
        if com_this {
            let t = b.add_param("this".to_string(), Type::Ref);
            b.this_param = Some(Operand::Val(t));
            b.this_finalizavel = self.this_finalizavel;
            b.enclosing_class = self.enclosing_class;
        }
        // Cada captura, ligada no corpo; a função repassa o mesmo lugar às
        // diretas que ela chama.
        let mut ligadas: Vec<(String, Local)> = Vec::new();
        for c in &direta.capturas {
            let nome_p = self.ctx.symbol_name(c.sym).to_string();
            let local = match c.passagem {
                Passagem::Valor => {
                    let v = b.add_param(nome_p, c.ty);
                    Local { modo: Modo::Valor(Operand::Val(v)), ty: c.ty, tipo_estatico: c.local.tipo_estatico, finalizavel: c.local.finalizavel, escopo: 0, offset: c.local.offset, late: None }
                }
                Passagem::Endereco => {
                    let v = b.add_param(nome_p, Type::Ptr);
                    if let Some(o) = c.local.offset {
                        b.ponteiros.insert(o);
                    }
                    Local { modo: Modo::Memoria(Operand::Val(v)), ty: c.ty, tipo_estatico: c.local.tipo_estatico, finalizavel: c.local.finalizavel, escopo: 0, offset: c.local.offset, late: None }
                }
                Passagem::Celula => {
                    let v = b.add_param(nome_p, Type::Ref);
                    let ptr = b.alloca_na_entrada(Type::Ref);
                    b.emit(Instruction::Store { ptr: ptr.clone(), val: Operand::Val(v) }, Type::Void);
                    Local { modo: Modo::Celula(ptr), ty: c.ty, tipo_estatico: c.local.tipo_estatico, finalizavel: c.local.finalizavel, escopo: 0, offset: c.local.offset, late: None }
                }
            };
            ligadas.push((identidade(&c.local), local.clone()));
            b.ligar_local_como(c.sym, local);
        }
        if com_tupla {
            let t = b.add_param_rti("$tipos_de_fora".to_string());
            b.tupla_de_tipos = Some(Operand::Val(t));
        }
        b.params_de_tipo_da_funcao = self.params_de_tipo_da_funcao.clone();
        b.params_locais = self.params_locais.clone();
        b.extensao_do_this = self.extensao_do_this;
        b.tipo_ext_do_this = self.tipo_ext_do_this;
        b.classe_do_membro = self.classe_do_membro;
        b.classe_por_tupla = self.classe_por_tupla;
        if self.classe_por_tupla {
            b.enclosing_class = self.enclosing_class;
        }
        // As diretas que o corpo chama (a própria, na recursão), com as
        // capturas trocadas pelas ligações de lá.
        for (s, o) in &chamadas {
            let Some(d) = self.funcoes_diretas.get(o) else { continue };
            let mut d = d.clone();
            for c in d.capturas.iter_mut() {
                if let Some((_, l)) = ligadas.iter().find(|(id, _)| *id == identidade(&c.local)) {
                    c.local = l.clone();
                }
            }
            b.declarar_variavel(*s, *o, Type::Ref, Operand::Constant(Constant::Null));
            b.funcoes_diretas.insert(*o, d);
        }
        b.preparar_capturas(
            ast,
            super::captura::Raiz { parametros: params, corpo: Some(&f.body), inicializadores: &[] },
        );
        b.abrir_escopo();
        for (i, p) in params.iter().enumerate() {
            let nome_p = p.name.map_or_else(|| "arg".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());
            let v = b.add_param(nome_p, direta.reprs[i]);
            if let Some(n) = p.name {
                b.declarar_variavel(n.sym, n.span.start as usize, direta.reprs[i], Operand::Val(v));
            }
        }
        match &f.body {
            FunctionBody::Block(s) => b.lower_stmt(ast, *s),
            FunctionBody::Expression(e) => {
                let r = b.lower_expr(ast, *e);
                if !b.is_terminated() {
                    let r = b.coagir(r, ret);
                    b.terminate(Terminator::Return(Some(r)));
                }
            }
            _ => {
                b.nao_suportado("função local direta sem corpo", span);
            }
        }
        self.absorver(b);
    }

    /// A função direta registrada no local `l` (o nome de uma função
    /// local direta), se é uma.
    pub fn direta_do_local(&self, l: &Local) -> Option<Direta> {
        self.funcoes_diretas.get(&l.offset?).cloned()
    }

    /// `f(avaliados)` de uma função direta: `call` estático com as
    /// capturas lidas agora (o valor, o endereço ou a célula).
    pub fn chamar_direta(&mut self, d: &Direta, avaliados: &[super::membros::Avaliado]) -> Operand {
        let mut args = Vec::with_capacity(d.capturas.len() + avaliados.len() + 2);
        if d.com_this {
            let t = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
            args.push(self.coagir(t, Type::Ref));
        }
        for c in &d.capturas {
            let v = match c.passagem {
                Passagem::Valor => {
                    let v = self.ler_local(&c.local);
                    self.coagir(v, c.ty)
                }
                Passagem::Endereco => match &c.local.modo {
                    Modo::Memoria(ptr) => ptr.clone(),
                    _ => unreachable!("captura por endereço fora da memória"),
                },
                Passagem::Celula => self.celula_do_local(&c.local).unwrap_or(Operand::Constant(Constant::Null)),
            };
            args.push(v);
        }
        if d.com_tupla {
            args.push(self.tupla_de_tipos.clone().unwrap_or(Operand::Constant(Constant::Int(0))));
        }
        for (i, (_, v)) in avaliados.iter().enumerate() {
            let ty = d.reprs.get(i).copied().unwrap_or(Type::Ref);
            args.push(self.coagir(v.clone(), ty));
        }
        self.emit_call_with_check(Instruction::CallStatic { symbol: d.simbolo.clone(), args, ret_ty: d.ret }, d.ret)
    }
}
