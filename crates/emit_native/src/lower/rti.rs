//! Tipos em tempo de execução (RTI) no lowering: receitas, o tipo de cada
//! objeto genérico e os testes `is`/`as` com argumentos de tipo.
//!
//! O desenho é o do dart2js (`sdk/lib/_internal/js_runtime/lib/rti.dart`) e
//! está no runtime (`crates/runtime/src/tipos.rs`): um tipo é um id de um
//! universo canônico; o compilador descreve cada tipo por uma **receita**
//! (texto curto, gramática em `tipos.rs`), que o runtime lê uma vez — cada
//! receita tem um global preguiçoso (`dfr.<hash>`) e o getter dele
//! (`df.rti.<hash>`). Receita com variáveis (`P<i>`: parâmetro de tipo da
//! classe do código corrente; `M<i>`: argumento de tipo da função corrente)
//! é avaliada no ambiente (`dartforge_rti_avaliar`: o tipo de `this` visto
//! como a classe, e a tupla da função).
//!
//! **Onde o tipo é gravado** (nos metadados do slot, `Heap::metadados`):
//! na criação de uma instância de classe genérica (o tipo estático da
//! criação, `C<T…>`), nos literais de coleção com tipo de elemento, e nas
//! closures (a assinatura). Um objeto sem tipo gravado tem o tipo cru da
//! classe (argumentos `dynamic`).
//!
//! **Onde ele é lido:** `is`/`as`, `catch (e) on T`, padrões de tipo e de
//! coleção com argumentos. O teste pela classe (`dartforge_is_subclass`)
//! continua onde basta (classe sem argumentos de tipo): nele a resposta é a
//! mesma e não há leitura de tipo.
//!
//! As classes que as receitas citam e os supertipos delas são registrados na
//! entrada (`registrar_universo`), só quando o programa usa alguma receita:
//! quem não usa RTI não paga nada.

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{ClassId, Element};
use dartforge_frontend::ast::{self, TypeKind};
use dartforge_intern::SymbolId;
use dartforge_types::table::{Type as T, TypeId, TypeParamId, TypeParamOwner};
use std::collections::{BTreeSet, HashMap};

/// Id de classe do heap do objeto `Type` (`tipos.rs`, `CLASSE_TIPO`).
pub const CLASSE_TIPO: i64 = 0x3FFF_FF01;

/// A classe `nome` do `dart:core`.
fn classe_do_core(ctx: &Context, nome: &str) -> Option<ClassId> {
    let core = ctx.program.core?;
    let sym = ctx.interner.lookup(nome)?;
    match ctx.program.lookup(core, sym)?.getter? {
        Element::Class(c) => Some(c),
        _ => None,
    }
}

/// Hash estável (FNV-1a 64) de uma receita: o nome do global dela.
fn hash_receita(r: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in r.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Uma receita e se ela tem variáveis (`P<i>`/`M<i>`) a avaliar.
#[derive(Debug, Clone)]
pub struct Receita {
    pub texto: String,
    pub variaveis: bool,
}

impl Context<'_> {
    /// O id RTI de uma classe: o id de classe do heap para as do programa
    /// (e as da fonte); para as do SDK, a faixa `0x2000_0000 +` a ordem do
    /// caminho estável; `Object` é 0 (o id de classe do heap dele).
    pub fn id_rti(&self, c: ClassId) -> i64 {
        if let Some(id) = self.id_de_classe(c) {
            return i64::from(id);
        }
        if Some(c) == self.core.object_class {
            return 0;
        }
        self.ids_rti_sdk.get(&c).map_or(0, |&k| 0x2000_0000 + i64::from(k))
    }

    /// A classe de um id RTI (o inverso de `id_rti`).
    pub fn classe_do_id_rti(&self, id: i64) -> Option<ClassId> {
        if id == 0 {
            return self.core.object_class;
        }
        if id >= 0x2000_0000 {
            let k = u32::try_from(id - 0x2000_0000).ok()?;
            return self.ids_rti_sdk.iter().find(|(_, v)| **v == k).map(|(c, _)| *c);
        }
        self.ids_de_classe.iter().position(|x| *x == Some(id as u32)).map(|i| ClassId(i as u32))
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Índice de `p` nos parâmetros de tipo do seu dono, com a letra da
    /// variável: `P` (classe do código corrente) ou `M` (função corrente, ou
    /// a classe numa fábrica, onde os argumentos vêm na tupla).
    fn variavel_de(&self, p: TypeParamId) -> Option<(char, usize)> {
        match self.ctx.table.param(p).owner {
            TypeParamOwner::Class(c) => {
                let i = self.ctx.outline.classes.get(c.0 as usize)?.type_params.iter().position(|&x| x == p)?;
                Some((if self.classe_por_tupla { 'M' } else { 'P' }, i))
            }
            TypeParamOwner::Function(f) => {
                let i = self.ctx.outline.functions.get(f.0 as usize)?.type_params.iter().position(|&x| x == p)?;
                Some(('M', i))
            }
            _ => None,
        }
    }

    /// Receita de um tipo do `crates/types`.
    pub fn receita_de_tipo(&self, t: TypeId) -> Receita {
        let mut r = Receita { texto: String::new(), variaveis: false };
        self.escrever_tipo(t, &mut Vec::new(), &HashMap::new(), &mut r);
        r
    }

    fn escrever_tipo(
        &self,
        t: TypeId,
        ligadas: &mut Vec<TypeParamId>,
        subst: &HashMap<TypeParamId, TypeId>,
        r: &mut Receita,
    ) {
        let anulavel = self.ctx.table.get(t).is_declared_nullable();
        match self.ctx.table.get(t) {
            T::Dynamic => r.texto.push('D'),
            T::Void => r.texto.push('V'),
            T::Never => r.texto.push('N'),
            T::Null => r.texto.push('U'),
            T::Interface { class, args, .. } => {
                r.texto.push_str(&format!("C{}", self.ctx.id_rti(*class)));
                if !args.is_empty() {
                    r.texto.push('<');
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            r.texto.push(',');
                        }
                        self.escrever_tipo(*a, ligadas, subst, r);
                    }
                    r.texto.push('>');
                }
            }
            T::FutureOr { arg, .. } => {
                r.texto.push_str("O<");
                self.escrever_tipo(*arg, ligadas, subst, r);
                r.texto.push('>');
            }
            // `X & B` (variável de tipo promovida) só existe na análise
            // estática; reificado, é a própria `X` (como no dart2js e na
            // NORM da especificação da inferência).
            T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
                if let Some(&s) = subst.get(param) {
                    self.escrever_tipo(s, ligadas, subst, r);
                } else if let Some(i) = ligadas.iter().rposition(|x| x == param) {
                    r.texto.push_str(&format!("B{i}"));
                } else if let Some((letra, i)) = self.variavel_de(*param) {
                    r.texto.push_str(&format!("{letra}{i}"));
                    r.variaveis = true;
                } else {
                    r.texto.push('D');
                }
            }
            T::Function { type_params, ret, positional, optional, named, .. } => {
                let n = ligadas.len();
                ligadas.extend(type_params.iter().copied());
                r.texto.push_str(&format!("F<{};", type_params.len()));
                self.escrever_tipo(*ret, ligadas, subst, r);
                r.texto.push_str(&format!(";{};", positional.len()));
                for (i, p) in positional.iter().chain(optional.iter()).enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    self.escrever_tipo(*p, ligadas, subst, r);
                }
                r.texto.push(';');
                for (i, (nome, p, req)) in named.iter().enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    r.texto.push_str(self.ctx.symbol_name(*nome));
                    r.texto.push(':');
                    self.escrever_tipo(*p, ligadas, subst, r);
                    if *req {
                        r.texto.push('!');
                    }
                }
                r.texto.push('>');
                ligadas.truncate(n);
            }
            T::Record { positional, named, .. } => {
                r.texto.push_str("R<");
                for (i, p) in positional.iter().enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    self.escrever_tipo(*p, ligadas, subst, r);
                }
                r.texto.push(';');
                for (i, (nome, p)) in named.iter().enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    r.texto.push_str(self.ctx.symbol_name(*nome));
                    r.texto.push(':');
                    self.escrever_tipo(*p, ligadas, subst, r);
                }
                r.texto.push('>');
            }
            T::ExtensionType { .. } => r.texto.push('D'),
        }
        if anulavel && !matches!(self.ctx.table.get(t), T::Dynamic | T::Void | T::Null) {
            r.texto.push('?');
        }
    }

    /// Os nomes dos parâmetros de tipo da classe envolvente (na ordem).
    fn params_da_classe(&self) -> Vec<SymbolId> {
        let Some(c) = self.enclosing_class else { return Vec::new() };
        self.ctx
            .outline
            .classes
            .get(c.0 as usize)
            .map(|d| d.type_params.iter().map(|&p| self.ctx.table.param(p).name).collect())
            .unwrap_or_default()
    }

    /// Receita de uma anotação de tipo do código (`x is List<T>`), resolvida
    /// pelo escopo: variáveis de tipo da função e da classe, o escopo da
    /// biblioteca (com prefixo). `None` quando o nome não resolve.
    pub fn receita_da_anotacao(&self, a: &ast::TypeAnnotation) -> Option<Receita> {
        let mut r = Receita { texto: String::new(), variaveis: false };
        self.escrever_anotacao(a, &mut Vec::new(), &mut r)?;
        Some(r)
    }

    fn escrever_anotacao(&self, a: &ast::TypeAnnotation, ligadas: &mut Vec<SymbolId>, r: &mut Receita) -> Option<()> {
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        match &a.kind {
            TypeKind::Void => r.texto.push('V'),
            TypeKind::Named { name, args } => {
                let ultimo = name.last()?;
                let nome = self.ctx.symbol_name(ultimo.sym);
                let simples = name.len() == 1;
                if simples && let Some(i) = ligadas.iter().rposition(|s| *s == ultimo.sym) {
                    r.texto.push_str(&format!("B{i}"));
                } else if simples && let Some(i) = self.params_de_tipo_da_funcao.iter().position(|s| *s == ultimo.sym) {
                    r.texto.push_str(&format!("M{i}"));
                    r.variaveis = true;
                } else if simples && let Some(i) = self.params_da_classe().iter().position(|s| *s == ultimo.sym) {
                    r.texto.push_str(&format!("{}{i}", if self.classe_por_tupla { 'M' } else { 'P' }));
                    r.variaveis = true;
                } else if simples && nome == "dynamic" {
                    r.texto.push('D');
                } else if simples && nome == "Never" {
                    r.texto.push('N');
                } else if simples && nome == "Null" {
                    r.texto.push('U');
                } else if simples && nome == "FutureOr" {
                    r.texto.push_str("O<");
                    match args.first() {
                        Some(x) => self.escrever_anotacao(unit_ast.ty(*x), ligadas, r)?,
                        None => r.texto.push('D'),
                    }
                    r.texto.push('>');
                } else {
                    let lib = self.ctx.program.unit(self.unit_id).library;
                    let b = match &name[..] {
                        [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
                        _ => self.ctx.program.lookup(lib, ultimo.sym),
                    }?;
                    match b.getter? {
                        Element::Class(c) => {
                            r.texto.push_str(&format!("C{}", self.ctx.id_rti(c)));
                            let n = self.ctx.outline.classes.get(c.0 as usize).map_or(0, |d| d.type_params.len());
                            if n > 0 {
                                r.texto.push('<');
                                for i in 0..n {
                                    if i > 0 {
                                        r.texto.push(',');
                                    }
                                    match args.get(i) {
                                        Some(x) => self.escrever_anotacao(unit_ast.ty(*x), ligadas, r)?,
                                        None => r.texto.push('D'),
                                    }
                                }
                                r.texto.push('>');
                            }
                        }
                        Element::Typedef(td) => {
                            let dados = self.ctx.outline.typedefs.get(td.0 as usize)?;
                            // Os argumentos do typedef entram pela receita de
                            // cada um (sub-receitas escritas em texto).
                            let mut sub = Receita { texto: String::new(), variaveis: false };
                            let mut textos = Vec::new();
                            for x in args.iter() {
                                sub.texto.clear();
                                self.escrever_anotacao(unit_ast.ty(*x), ligadas, &mut sub)?;
                                textos.push(sub.texto.clone());
                            }
                            r.variaveis |= sub.variaveis;
                            let mut alvo = self.receita_de_tipo_com(dados.target_type, &dados.type_params, &textos);
                            r.variaveis |= alvo.variaveis;
                            if a.nullable {
                                alvo.texto.push('?');
                            }
                            r.texto.push_str(&alvo.texto);
                            return Some(());
                        }
                        _ => return None,
                    }
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                let n = ligadas.len();
                ligadas.extend(type_params.iter().map(|p| p.name.sym));
                r.texto.push_str(&format!("F<{};", type_params.len()));
                match return_type {
                    Some(x) => self.escrever_anotacao(unit_ast.ty(*x), ligadas, r)?,
                    None => r.texto.push('D'),
                }
                let obrig = parameters.iter().filter(|p| p.kind == ast::ParameterKind::Required).count();
                r.texto.push_str(&format!(";{obrig};"));
                let mut primeiro = true;
                for p in parameters.iter().filter(|p| p.kind != ast::ParameterKind::Named) {
                    if !primeiro {
                        r.texto.push(',');
                    }
                    primeiro = false;
                    match p.ty {
                        Some(x) => self.escrever_anotacao(unit_ast.ty(x), ligadas, r)?,
                        None => r.texto.push('D'),
                    }
                }
                r.texto.push(';');
                let mut primeiro = true;
                for p in parameters.iter().filter(|p| p.kind == ast::ParameterKind::Named) {
                    if !primeiro {
                        r.texto.push(',');
                    }
                    primeiro = false;
                    r.texto.push_str(self.ctx.symbol_name(p.name?.sym));
                    r.texto.push(':');
                    match p.ty {
                        Some(x) => self.escrever_anotacao(unit_ast.ty(x), ligadas, r)?,
                        None => r.texto.push('D'),
                    }
                    if p.required {
                        r.texto.push('!');
                    }
                }
                r.texto.push('>');
                ligadas.truncate(n);
            }
            TypeKind::Record { positional, named } => {
                r.texto.push_str("R<");
                for (i, x) in positional.iter().enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    self.escrever_anotacao(unit_ast.ty(*x), ligadas, r)?;
                }
                r.texto.push(';');
                let mut campos: Vec<&(ast::Name, ast::TypeId)> = named.iter().collect();
                campos.sort_by_key(|(n, _)| self.ctx.symbol_name(n.sym).to_string());
                for (i, (n, x)) in campos.into_iter().enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    r.texto.push_str(self.ctx.symbol_name(n.sym));
                    r.texto.push(':');
                    self.escrever_anotacao(unit_ast.ty(*x), ligadas, r)?;
                }
                r.texto.push('>');
            }
        }
        let e_nulavel_por_si = matches!(r.texto.as_bytes().last(), Some(b'D' | b'V' | b'U'));
        if a.nullable && !e_nulavel_por_si {
            r.texto.push('?');
        }
        Some(())
    }

    /// Receita de `t` com os parâmetros `params` trocados pelas receitas
    /// `args` (os de um typedef).
    fn receita_de_tipo_com(&self, t: TypeId, params: &[TypeParamId], args: &[String]) -> Receita {
        // Escreve com marcadores e troca: os parâmetros do typedef não são de
        // classe nem de função, então `escrever_tipo` os escreveria como `D`.
        let mut r = Receita { texto: String::new(), variaveis: false };
        self.escrever_tipo_typedef(t, params, args, &mut Vec::new(), &mut r);
        r
    }

    fn escrever_tipo_typedef(
        &self,
        t: TypeId,
        params: &[TypeParamId],
        args: &[String],
        ligadas: &mut Vec<TypeParamId>,
        r: &mut Receita,
    ) {
        if let T::TypeParameter { param, nullable } = self.ctx.table.get(t)
            && let Some(i) = params.iter().position(|p| p == param)
        {
            r.texto.push_str(args.get(i).map_or("D", String::as_str));
            if *nullable {
                r.texto.push('?');
            }
            return;
        }
        // Os demais casos: a receita normal, descendo pelos argumentos com a
        // mesma troca (os tipos compostos são reescritos por partes).
        match self.ctx.table.get(t) {
            T::Interface { class, args: a, nullable } => {
                r.texto.push_str(&format!("C{}", self.ctx.id_rti(*class)));
                if !a.is_empty() {
                    r.texto.push('<');
                    for (i, x) in a.iter().enumerate() {
                        if i > 0 {
                            r.texto.push(',');
                        }
                        self.escrever_tipo_typedef(*x, params, args, ligadas, r);
                    }
                    r.texto.push('>');
                }
                if *nullable {
                    r.texto.push('?');
                }
            }
            T::Function { type_params, ret, positional, optional, named, nullable } => {
                let n = ligadas.len();
                ligadas.extend(type_params.iter().copied());
                r.texto.push_str(&format!("F<{};", type_params.len()));
                self.escrever_tipo_typedef(*ret, params, args, ligadas, r);
                r.texto.push_str(&format!(";{};", positional.len()));
                for (i, p) in positional.iter().chain(optional.iter()).enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    self.escrever_tipo_typedef(*p, params, args, ligadas, r);
                }
                r.texto.push(';');
                for (i, (nome, p, req)) in named.iter().enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    r.texto.push_str(self.ctx.symbol_name(*nome));
                    r.texto.push(':');
                    self.escrever_tipo_typedef(*p, params, args, ligadas, r);
                    if *req {
                        r.texto.push('!');
                    }
                }
                r.texto.push('>');
                ligadas.truncate(n);
                if *nullable {
                    r.texto.push('?');
                }
            }
            T::FutureOr { arg, nullable } => {
                r.texto.push_str("O<");
                self.escrever_tipo_typedef(*arg, params, args, ligadas, r);
                r.texto.push('>');
                if *nullable {
                    r.texto.push('?');
                }
            }
            T::Record { positional, named, nullable } => {
                r.texto.push_str("R<");
                for (i, p) in positional.iter().enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    self.escrever_tipo_typedef(*p, params, args, ligadas, r);
                }
                r.texto.push(';');
                for (i, (nome, p)) in named.iter().enumerate() {
                    if i > 0 {
                        r.texto.push(',');
                    }
                    r.texto.push_str(self.ctx.symbol_name(*nome));
                    r.texto.push(':');
                    self.escrever_tipo_typedef(*p, params, args, ligadas, r);
                }
                r.texto.push('>');
                if *nullable {
                    r.texto.push('?');
                }
            }
            _ => self.escrever_tipo(t, ligadas, &HashMap::new(), r),
        }
    }

    /// O tipo da receita, como `I64`, no ambiente corrente.
    pub fn rti_da_receita(&mut self, r: &Receita) -> Operand {
        let h = hash_receita(&r.texto);
        // A mesma receita pode surgir no programa e em várias bibliotecas do
        // SDK. Cada biblioteca possui seu próprio cache preguiçoso; nomes
        // distintos evitam definições múltiplas na ligação ThinLTO.
        let lib = self.ctx.program.unit(self.unit_id).library;
        let dono = crate::context::escapar(&self.ctx.nome_da_biblioteca(lib));
        let getter = format!("df.rti.{h}.{dono}");
        let global = format!("dfr.{h}.{dono}");
        if !self.entradas_feitas.contains(&getter) {
            self.entradas_feitas.insert(getter.clone());
            let mut g = FnBuilder::new(self.ctx, self.unit_id, getter.clone(), "rti".to_string(), Type::I64);
            let ok = g.emit(
                Instruction::LoadGlobal {
                    simbolo: format!("{global}$ok"),
                    ty: Type::I8,
                },
                Type::I8,
            );
            let pronto = g.emit(Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))), Type::I1);
            let b_pronto = g.new_block();
            let b_ler = g.new_block();
            g.terminate(Terminator::CondBranch {
                cond: pronto,
                then_block: b_pronto,
                else_block: b_ler,
            });
            g.set_block(b_pronto);
            let v = g.emit(
                Instruction::LoadGlobal {
                    simbolo: global.clone(),
                    ty: Type::I64,
                },
                Type::I64,
            );
            g.terminate(Terminator::Return(Some(v)));
            g.set_block(b_ler);
            let s = g.emit(Instruction::Const(Constant::String(r.texto.clone())), Type::Ref);
            let t = g.emit(
                Instruction::CallRuntime {
                    name: "dartforge_rti_receita".to_string(),
                    args: vec![(s, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
            g.emit(
                Instruction::StoreGlobal {
                    simbolo: global.clone(),
                    val: t.clone(),
                    ty: Type::I64,
                    raiz: None,
                },
                Type::Void,
            );
            g.emit(
                Instruction::StoreGlobal {
                    simbolo: format!("{global}$ok"),
                    val: Operand::Constant(Constant::Int(1)),
                    ty: Type::I8,
                    raiz: None,
                },
                Type::Void,
            );
            g.terminate(Terminator::Return(Some(t)));
            self.globais_extras.push((0, Type::I64, global));
            self.absorver(g);
        }
        let base = self.emit(
            Instruction::CallStatic {
                symbol: getter,
                args: Vec::new(),
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        if !r.variaveis {
            return base;
        }
        let this = if self.classe_por_tupla {
            Operand::Constant(Constant::Null)
        } else {
            self.this_param.clone().map_or(Operand::Constant(Constant::Null), |t| self.coagir(t, Type::Ref))
        };
        let classe = self.enclosing_class.map_or(0, |c| self.ctx.id_rti(c));
        let tupla = self.tupla_de_tipos.clone().unwrap_or(Operand::Constant(Constant::Int(0)));
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_avaliar".to_string(),
                args: vec![
                    (base, Type::I64),
                    (this, Type::Ref),
                    (Operand::Constant(Constant::Int(classe)), Type::I64),
                    (tupla, Type::I64),
                ],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// O tipo `t` do `crates/types`, como `I64`, no ambiente corrente.
    pub fn rti_de_tipo(&mut self, t: TypeId) -> Operand {
        let r = self.receita_de_tipo(t);
        self.rti_da_receita(&r)
    }

    /// Grava o tipo em tempo de execução de um objeto (`Ref`).
    pub fn definir_rti(&mut self, obj: Operand, tipo: Operand) {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_definir".to_string(),
                args: vec![(obj, Type::Ref), (tipo, Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }

    /// Grava em `obj` o tipo estático `t` quando ele diz algo que a classe
    /// não diz (tem argumentos de tipo, ou é tipo de função).
    pub fn definir_rti_se_generico(&mut self, obj: Operand, t: TypeId) {
        let util = match self.ctx.table.get(t) {
            T::Interface { args, .. } => args.iter().any(|a| *a != self.ctx.core.dynamic_),
            T::Function { .. } => true,
            _ => false,
        };
        if util {
            let tipo = self.rti_de_tipo(t);
            self.definir_rti(obj, tipo);
        }
    }

    /// O literal de coleção `l` com o tipo estático da expressão (quando o
    /// literal não terminou o bloco).
    pub fn rti_do_literal(&mut self, l: Operand, expr: ast::ExprId) -> Operand {
        if !self.is_terminated()
            && self.operand_type(&l) == Type::Ref
            && let Some(t) = self.ctx.get_type(self.unit_id, expr)
        {
            self.definir_rti_se_generico(l.clone(), t);
        }
        l
    }

    /// `op is <tipo>` (I1) pelo RTI.
    pub fn testar_rti(&mut self, op: Operand, tipo: Operand) -> Operand {
        let v = self.coagir(op, Type::Ref);
        let r = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_e".to_string(),
                args: vec![(v, Type::Ref), (tipo, Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        self.emit(Instruction::ICmp(ICmpOp::Ne, r, Operand::Constant(Constant::Int(0))), Type::I1)
    }

    /// `op as <tipo>` pelo RTI: `TypeError` pendente se falha.
    pub fn cast_rti(&mut self, op: Operand, tipo: Operand) {
        let v = self.coagir(op, Type::Ref);
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_rti_como".to_string(),
                args: vec![(v, Type::Ref), (tipo, Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }

    /// Arma a tupla de argumentos de tipo da chamada `expr` à função `fid`
    /// (se ela é genérica): os escritos (`f<int>(…)`), ou os inferidos,
    /// casando o retorno declarado com o tipo estático da chamada e cada
    /// parâmetro com o tipo do argumento. O que não se deduz fica `dynamic`.
    pub fn armar_tupla(&mut self, fid: usize, expr: ast::ExprId, arguments: &ast::Arguments) {
        if !self.funcao_generica(fid) {
            return;
        }
        let dados = &self.ctx.outline.functions[fid];
        let params: Vec<TypeParamId> = dados.type_params.to_vec();
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        if !arguments.type_args.is_empty() {
            let mut r = Receita { texto: "L<".to_string(), variaveis: false };
            for (i, a) in arguments.type_args.iter().enumerate() {
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
            r.texto.push('>');
            let t = self.rti_da_receita(&r);
            self.tupla_armada = Some(t);
            return;
        }
        let mut achados: Vec<Option<TypeId>> = vec![None; params.len()];
        if let Some(real) = self.ctx.get_type(self.unit_id, expr) {
            self.unificar(dados.return_type, real, &params, &mut achados);
        }
        let mut posicionais = arguments.args.iter().filter(|a| a.name.is_none());
        for p in dados.parameters.iter() {
            let arg = if p.kind == ast::ParameterKind::Named {
                arguments.args.iter().find(|a| a.name.map(|n| n.sym) == p.name)
            } else {
                posicionais.next()
            };
            if let Some(a) = arg
                && let Some(real) = self.ctx.get_type(self.unit_id, a.value)
            {
                self.unificar(p.ty, real, &params, &mut achados);
            }
        }
        let args: Vec<TypeId> = achados.into_iter().map(|t| t.unwrap_or(self.ctx.core.dynamic_)).collect();
        let t = self.tupla_de_tipos_rti(&args);
        self.tupla_armada = Some(t);
    }

    /// Casa o tipo declarado `decl` (com os parâmetros `params`) com o tipo
    /// real `real`, gravando em `achados` o que cada parâmetro vale.
    fn unificar(&self, decl: TypeId, real: TypeId, params: &[TypeParamId], achados: &mut [Option<TypeId>]) {
        if real == self.ctx.core.dynamic_ {
            return;
        }
        match (self.ctx.table.get(decl), self.ctx.table.get(real)) {
            (T::TypeParameter { param, .. }, _) => {
                // (`T?` casado com `X?` dá `X?`: a tabela de tipos é só de
                // leitura aqui, e o `?` a mais não muda a resposta dos testes
                // que o código genérico faz com `T`.)
                if let Some(i) = params.iter().position(|p| p == param)
                    && achados[i].is_none()
                {
                    achados[i] = Some(real);
                }
            }
            (T::Interface { class: c1, args: a1, .. }, T::Interface { class: c2, args: a2, .. }) if c1 == c2 => {
                for (x, y) in a1.iter().zip(a2.iter()) {
                    self.unificar(*x, *y, params, achados);
                }
            }
            (T::FutureOr { arg, .. }, T::FutureOr { arg: b, .. }) => self.unificar(*arg, *b, params, achados),
            (T::FutureOr { arg, .. }, T::Interface { class, args, .. }) => {
                if Some(*class) == self.ctx.core.future_class {
                    if let Some(x) = args.first() {
                        self.unificar(*arg, *x, params, achados);
                    }
                } else {
                    self.unificar(*arg, real, params, achados);
                }
            }
            (
                T::Function { ret: r1, positional: p1, .. },
                T::Function { ret: r2, positional: p2, .. },
            ) => {
                self.unificar(*r1, *r2, params, achados);
                for (x, y) in p1.iter().zip(p2.iter()) {
                    self.unificar(*x, *y, params, achados);
                }
            }
            _ => {}
        }
    }

    /// Grava a assinatura de uma closure (`F<…>`): o tipo estático da
    /// expressão de função, se a inferência o tem, senão o que a declaração
    /// escreve (parâmetros sem tipo são `dynamic`). É o `$signature` do
    /// dart2js: o que `f is void Function(Object, StackTrace)` pergunta.
    pub fn definir_rti_de_closure(&mut self, clo: Operand, ast: &ast::Ast, fid: ast::FunctionId, expr: Option<ast::ExprId>) {
        if let Some(e) = expr
            && let Some(t) = self.ctx.get_type(self.unit_id, e)
            && matches!(self.ctx.table.get(t), T::Function { .. })
        {
            let r = self.receita_de_tipo(t);
            let tipo = self.rti_da_receita(&r);
            self.definir_rti(clo, tipo);
            return;
        }
        let f = ast.function(fid);
        let params = f.parameters.as_deref().unwrap_or(&[]);
        let mut r = Receita { texto: format!("F<{};", f.type_params.len()), variaveis: false };
        let mut ligadas: Vec<SymbolId> = f.type_params.iter().map(|p| p.name.sym).collect();
        let tipo_de = |s: &Self, t: Option<ast::TypeId>, r: &mut Receita, ligadas: &mut Vec<SymbolId>| match t {
            Some(x) => {
                let mut sub = Receita { texto: String::new(), variaveis: false };
                if s.escrever_anotacao(ast.ty(x), ligadas, &mut sub).is_some() {
                    r.texto.push_str(&sub.texto);
                    r.variaveis |= sub.variaveis;
                } else {
                    r.texto.push('D');
                }
            }
            None => r.texto.push('D'),
        };
        tipo_de(self, f.return_type, &mut r, &mut ligadas);
        let obrig = params.iter().filter(|p| p.kind == ast::ParameterKind::Required).count();
        r.texto.push_str(&format!(";{obrig};"));
        let mut primeiro = true;
        for p in params.iter().filter(|p| p.kind != ast::ParameterKind::Named) {
            if !primeiro {
                r.texto.push(',');
            }
            primeiro = false;
            tipo_de(self, p.ty, &mut r, &mut ligadas);
        }
        r.texto.push(';');
        let mut primeiro = true;
        for p in params.iter().filter(|p| p.kind == ast::ParameterKind::Named) {
            let Some(n) = p.name else { continue };
            if !primeiro {
                r.texto.push(',');
            }
            primeiro = false;
            r.texto.push_str(self.ctx.symbol_name(n.sym));
            r.texto.push(':');
            tipo_de(self, p.ty, &mut r, &mut ligadas);
            if p.required {
                r.texto.push('!');
            }
        }
        r.texto.push('>');
        let tipo = self.rti_da_receita(&r);
        self.definir_rti(clo, tipo);
    }

    /// Grava a assinatura do tear-off da função `fid` (a do outline; num
    /// método, avaliada no tipo do receptor `recv`).
    pub fn definir_rti_de_tearoff(&mut self, clo: Operand, fid: usize, recv: Option<Operand>) {
        let Some(sig) = self.ctx.outline.functions.get(fid).map(|d| d.signature) else {
            return;
        };
        let salvo = (self.this_param.clone(), self.enclosing_class, self.classe_por_tupla, self.tupla_de_tipos.clone());
        if let Some(r) = recv {
            self.this_param = Some(r);
            self.enclosing_class = self.ctx.program.functions[fid].class;
            self.classe_por_tupla = false;
        }
        self.tupla_de_tipos = None;
        let r = self.receita_de_tipo(sig);
        let tipo = self.rti_da_receita(&r);
        (self.this_param, self.enclosing_class, self.classe_por_tupla, self.tupla_de_tipos) = salvo;
        self.definir_rti(clo, tipo);
    }

    /// O teste de tipo da anotação precisa do RTI? (argumentos de tipo que
    /// não são triviais, variável de tipo, `FutureOr`, tipo de função ou de
    /// record, typedef.)
    pub fn anotacao_precisa_rti(&self, a: &ast::TypeAnnotation) -> bool {
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        match &a.kind {
            TypeKind::Void => false,
            TypeKind::Function { .. } | TypeKind::Record { .. } => true,
            TypeKind::Named { name, args } => {
                let Some(ultimo) = name.last() else { return false };
                let nome = self.ctx.symbol_name(ultimo.sym);
                if name.len() == 1
                    && (self.params_de_tipo_da_funcao.contains(&ultimo.sym)
                        || self.params_da_classe().contains(&ultimo.sym)
                        || nome == "FutureOr")
                {
                    return true;
                }
                let lib = self.ctx.program.unit(self.unit_id).library;
                let b = match &name[..] {
                    [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
                    _ => self.ctx.program.lookup(lib, ultimo.sym),
                };
                if matches!(b.and_then(|b| b.getter), Some(Element::Typedef(_))) {
                    return true;
                }
                let trivial = |t: &ast::TypeAnnotation| match &t.kind {
                    TypeKind::Named { name, args } if args.is_empty() => name.last().is_some_and(|n| {
                        let s = self.ctx.symbol_name(n.sym);
                        s == "dynamic" || (s == "Object" && t.nullable)
                    }),
                    _ => false,
                };
                !args.iter().all(|x| trivial(unit_ast.ty(*x)))
            }
        }
    }
}

/// Registro do universo na entrada do programa: nome e número de parâmetros
/// de cada classe citada pelas receitas (e pelas classes do programa), as
/// regras de supertipo (o fecho: os supertipos das citadas também), as
/// formas do runtime. Vazio quando o programa não usa receita nenhuma.
pub fn registrar_universo(ctx: &Context, module: &mut Module) {
    // O universo é global ao executável; os objetos do SDK só fornecem as
    // receitas que usam. A entrada do programa registra o universo completo.
    if module.biblioteca_sdk {
        return;
    }
    let mut receitas: Vec<String> = Vec::new();
    let mut usa_rti = false;
    for f in &module.functions {
        let e_receita = f.symbol.starts_with("df.rti.");
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    Instruction::Const(Constant::String(s)) if e_receita => receitas.push(s.clone()),
                    Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_rti_") => usa_rti = true,
                    _ => {}
                }
            }
        }
    }
    if receitas.is_empty() && !usa_rti && !module.modo_sdk {
        return;
    }
    // O objeto `Type` (`dartforge_rti_objeto_tipo`): a classe e os seletores
    // usados pelo SDK da fonte. Cada tipo tem um único objeto canônico.
    {
        let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
            return;
        };
        let simbolo = "df.$tipo.toString".to_string();
        let mut t = FnBuilder::new(ctx, u, simbolo.clone(), "toString".to_string(), Type::Ref);
        let obj = Operand::Val(t.add_param("this".to_string(), Type::Ref));
        let id = t.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let s = t.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_texto".to_string(),
                args: vec![(id, Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        t.terminate(Terminator::Return(Some(s)));
        t.finalizar(module);
        let mut igualdade = FnBuilder::new(ctx, u, "df.$tipo.$3d$3d$c".to_string(), "==".to_string(), Type::Ref);
        let recv = Operand::Val(igualdade.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(igualdade.add_param("args".to_string(), Type::Ptr));
        igualdade.add_param("desc".to_string(), Type::Ptr);
        let outro = igualdade.emit(
            Instruction::LoadIndexed { base: args, index: Operand::Constant(Constant::Int(0)) },
            Type::Ref,
        );
        let igual = igualdade.emit(Instruction::ICmp(ICmpOp::Eq, recv, outro), Type::I1);
        let igual = igualdade.coagir(igual, Type::Ref);
        igualdade.terminate(Terminator::Return(Some(igual)));
        igualdade.finalizar(module);
        let mut texto = FnBuilder::new(ctx, u, "df.$tipo.toString$c".to_string(), "toString".to_string(), Type::Ref);
        let recv = Operand::Val(texto.add_param("this".to_string(), Type::Ref));
        texto.add_param("args".to_string(), Type::Ptr);
        texto.add_param("desc".to_string(), Type::Ptr);
        let valor = texto.emit_call_with_check(
            Instruction::CallStatic { symbol: simbolo.clone(), args: vec![recv], ret_ty: Type::Ref },
            Type::Ref,
        );
        texto.terminate(Terminator::Return(Some(valor)));
        texto.finalizar(module);
        let mut metodos = vec![
            ("c:==".to_string(), "df.$tipo.$3d$3d$c".to_string()),
            ("c:toString".to_string(), "df.$tipo.toString$c".to_string()),
        ];
        // `_Type` é criado pelo runtime, fora da hierarquia de elementos do
        // programa. Ele ainda herda os membros de `Object`, incluindo o
        // getter privado usado por `identityHashCode(Object)` no hashSeed.
        if ctx.sdk_da_fonte && let Some(objeto) = ctx.core.object_class {
            for (seletor, simbolo) in super::sdk_fonte::tabela_de_metodos(ctx, objeto) {
                if !metodos.iter().any(|(existente, _)| *existente == seletor) {
                    metodos.push((seletor, simbolo));
                }
            }
        }
        module.tabelas_de_metodos.push((CLASSE_TIPO as u32, "df.mt.$tipo".to_string(), metodos));
        module.classes.push(ClassDef {
            id: CLASSE_TIPO as u32,
            name: "_Type".to_string(),
            field_count: 1,
            vtable: Vec::new(),
            to_string_symbol: Some(simbolo),
        });
    }
    let mut citadas: BTreeSet<i64> = BTreeSet::new();
    for r in &receitas {
        let b = r.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'C' && i + 1 < b.len() && (b[i + 1].is_ascii_digit() || b[i + 1] == b'-') {
                let mut j = i + 1;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'-') {
                    j += 1;
                }
                if let Ok(n) = r[i + 1..j].parse::<i64>() {
                    citadas.insert(n);
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
    // As formas do runtime.
    let core = ctx.core;
    let formas: Vec<(i64, Option<ClassId>)> = vec![
        (0, core.int_class),
        (1, core.double_class),
        (2, core.bool_class),
        (3, core.string_class),
        (4, core.list_class),
        (5, core.map_class),
        (6, core.set_class),
        (7, core.function_class),
        (8, core.record_class),
        (9, core.future_class),
        (10, core.object_class),
        (14, classe_do_core(ctx, "Type")),
    ];
    let mut classes: Vec<ClassId> = Vec::new();
    let mut vistas: std::collections::HashSet<ClassId> = std::collections::HashSet::new();
    let mut fila: Vec<ClassId> = Vec::new();
    for id in &citadas {
        if let Some(c) = ctx.classe_do_id_rti(*id) {
            fila.push(c);
        }
    }
    for (_, c) in &formas {
        fila.extend(c.iter().copied());
    }
    for (i, _) in ctx.program.classes.iter().enumerate() {
        if ctx.id_de_classe(ClassId(i as u32)).is_some() {
            fila.push(ClassId(i as u32));
        }
    }
    while let Some(c) = fila.pop() {
        if !vistas.insert(c) {
            continue;
        }
        classes.push(c);
        if let Some(d) = ctx.outline.hierarchy.get(c) {
            for sup in d.supertypes.keys() {
                fila.push(*sup);
            }
        }
    }
    classes.sort_by_key(|c| ctx.id_rti(*c));
    let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
        return;
    };
    let mut b = FnBuilder::new(ctx, u, "dartforge_rti_iniciar".to_string(), "rti".to_string(), Type::Void);
    for &c in &classes {
        let id = ctx.id_rti(c);
        let nome = ctx.symbol_name(ctx.program.classes[c.0 as usize].name).to_string();
        let n = ctx.outline.classes.get(c.0 as usize).map_or(0, |d| d.type_params.len());
        let s = b.emit(Instruction::Const(Constant::String(nome)), Type::Ref);
        b.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_classe_nome".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(id)), Type::I64),
                    (s, Type::Ref),
                    (Operand::Constant(Constant::Int(n as i64)), Type::I64),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let Some(d) = ctx.outline.hierarchy.get(c) else { continue };
        let mut sups: Vec<(i64, TypeId)> = d.supertypes.iter().map(|(k, t)| (ctx.id_rti(*k), *t)).collect();
        sups.sort_by_key(|x| x.0);
        for (_, t) in sups {
            // A regra é escrita nos parâmetros da própria classe (`P<i>`).
            let salvo = (b.enclosing_class, b.classe_por_tupla);
            b.enclosing_class = Some(c);
            b.classe_por_tupla = false;
            let r = b.receita_de_tipo(t);
            b.enclosing_class = salvo.0;
            b.classe_por_tupla = salvo.1;
            let s = b.emit(Instruction::Const(Constant::String(r.texto)), Type::Ref);
            let m = b.emit(
                Instruction::CallRuntime {
                    name: "dartforge_rti_receita".to_string(),
                    args: vec![(s, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
            b.emit(
                Instruction::CallRuntime {
                    name: "dartforge_rti_regra".to_string(),
                    args: vec![(Operand::Constant(Constant::Int(id)), Type::I64), (m, Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
    }
    for (forma, c) in formas {
        if let Some(c) = c {
            b.emit(
                Instruction::CallRuntime {
                    name: "dartforge_rti_classe_do_runtime".to_string(),
                    args: vec![
                        (Operand::Constant(Constant::Int(forma)), Type::I64),
                        (Operand::Constant(Constant::Int(ctx.id_rti(c))), Type::I64),
                    ],
                    ret_ty: Type::Void,
                },
                Type::Void,
            );
        }
    }
    // As classes de erro que o runtime representa (ids 1000–1012).
    for (heap_id, nome) in [
        (1000, "Exception"),
        (1001, "FormatException"),
        (1002, "StateError"),
        (1003, "ArgumentError"),
        (1004, "RangeError"),
        (1005, "UnsupportedError"),
        (1006, "StackTrace"),
        (1007, "Error"),
        (1008, "UnimplementedError"),
        (1009, "AssertionError"),
        (1010, "ConcurrentModificationError"),
        (1011, "TypeError"),
        (1012, "NoSuchMethodError"),
        (CLASSE_TIPO, "Type"),
    ] {
        let Some(core_lib) = ctx.program.core else { continue };
        let Some(sym) = ctx.interner.lookup(nome) else { continue };
        let Some(Element::Class(c)) = ctx.program.lookup(core_lib, sym).and_then(|b| b.getter) else { continue };
        b.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_classe_do_runtime".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(heap_id)), Type::I64),
                    (Operand::Constant(Constant::Int(ctx.id_rti(c))), Type::I64),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }
    b.terminate(Terminator::Return(None));
    b.finalizar(module);
    module.iniciar_rti = Some("dartforge_rti_iniciar".to_string());
}
