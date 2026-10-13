//! Tipos em tempo de execução (RTI) no lowering: receitas, o tipo de cada
//! objeto genérico e os testes `is`/`as` com argumentos de tipo.
//!
//! O desenho é o do dart2js (`sdk/lib/_internal/js_runtime/lib/rti.dart`) e
//! está no runtime (`crates/runtime/src/types.rs`): um tipo é um id de um
//! universo canônico; o compilador descreve cada tipo por uma **receita**
//! (texto curto, gramática em `types.rs`), que o runtime lê uma vez — cada
//! receita tem um global preguiçoso (`dfr.<hash>`) e o getter dele
//! (`df.rti.<hash>`). Receita com variáveis (`P<i>`: parâmetro de tipo da
//! classe do código corrente; `M<i>`: argumento de tipo da função corrente)
//! é avaliada no ambiente (`dartforge_rti_avaliar`: o tipo de `this` visto
//! como a classe, e a tupla da função).
//!
//! **Onde o tipo é gravado** (nos metadados do slot, `Heap::metadata`):
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
//! entrada (`register_universe`), só quando o programa usa alguma receita:
//! quem não usa RTI não paga nada.

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{ClassId, Element};
use dartforge_frontend::ast::{self, TypeKind};
use dartforge_intern::SymbolId;
use dartforge_types::table::{Type as T, TypeId, TypeParamId, TypeParamOwner};
use std::collections::{BTreeSet, HashMap};

/// Id de classe do heap do objeto `Type` (`types.rs`, `CLASSE_TIPO`).
pub const CLASSE_TIPO: i64 = 0x3FFF_FF01;

/// A classe `name` do `dart:core`.
fn core_class(ctx: &Context, name_str: &str) -> Option<ClassId> {
    let core = ctx.program.core?;
    let sym = ctx.interner.lookup(name_str)?;
    match ctx.program.lookup(core, sym)?.getter? {
        Element::Class(c) => Some(c),
        _ => None,
    }
}

/// Hash estável (FNV-1a 64) de uma receita: o nome do global dela.
fn recipe_hash(r: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in r.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Uma receita e se ela tem variáveis (`P<i>`/`M<i>`) a avaliar.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub text: String,
    pub variables: bool,
}

impl Context<'_> {
    /// O id RTI de uma classe: o id de classe do heap para as do programa
    /// (e as da fonte); para as do SDK, a faixa `0x2000_0000 +` a ordem do
    /// caminho estável; `Object` é 0 (o id de classe do heap dele).
    pub fn id_rti(&self, c: ClassId) -> i64 {
        if let Some(id) = self.class_id_of(c) {
            return i64::from(id);
        }
        if Some(c) == self.core.object_class {
            return 0;
        }
        self.sdk_rti_ids.get(&c).map_or(0, |&k| 0x2000_0000 + i64::from(k))
    }

    /// A classe de um id RTI (o inverso de `id_rti`).
    pub fn rti_id_class(&self, id: i64) -> Option<ClassId> {
        if id == 0 {
            return self.core.object_class;
        }
        if id >= 0x2000_0000 {
            let k = u32::try_from(id - 0x2000_0000).ok()?;
            return self.sdk_rti_ids.iter().find(|(_, v)| **v == k).map(|(c, _)| *c);
        }
        self.class_ids.iter().position(|x| *x == Some(id as u32)).map(|i| ClassId(i as u32))
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Índice de `p` nos parâmetros de tipo do seu dono, com a letra da
    /// variável: `P` (classe do código corrente) ou `M` (função corrente, ou
    /// a classe numa fábrica, onde os argumentos vêm na tupla).
    fn variable_of(&self, p: TypeParamId) -> Option<(char, usize)> {
        // O de uma closure genérica em volta (ou a corrente).
        if let Some(&(_, _, i)) = self.local_params.iter().rev().find(|(_, x, _)| *x == p) {
            return Some(('M', i));
        }
        match self.ctx.table.param(p).owner {
            TypeParamOwner::Class(c) => {
                let i = self.ctx.outline.classes.get(c.0 as usize)?.type_params.iter().position(|&x| x == p)?;
                // Os de um tipo de extensão vêm sempre na tupla (não há
                // objeto que os guarde).
                Some((if self.class_by_tuple || self.ctx.is_extension_type(c) { 'M' } else { 'P' }, i))
            }
            // Na tupla de um membro de extensão, os parâmetros da extensão
            // vêm antes dos do membro (`type_params_of`).
            TypeParamOwner::Function(f) => {
                let i = self.ctx.outline.functions.get(f.0 as usize)?.type_params.iter().position(|&x| x == p)?;
                let base = self.receiver_pattern(f.0 as usize).map_or(0, |(n, _)| n);
                Some(('M', base + i))
            }
            TypeParamOwner::Extension(e) => {
                let i = self.ctx.outline.extensions.get(e.0 as usize)?.type_params.iter().position(|&x| x == p)?;
                Some(('M', i))
            }
            _ => None,
        }
    }

    /// Receita de um tipo do `crates/types`.
    pub fn type_recipe(&self, t: TypeId) -> Recipe {
        let mut r = Recipe { text: String::new(), variables: false };
        self.write_type(t, &mut Vec::new(), &HashMap::new(), &mut r);
        r
    }

    /// A receita de `t` com os parâmetros de tipo de `subst` trocados pelos
    /// tipos dados (o parâmetro de uma classe genérica visto pela criação
    /// `C<int>(…)`, `typed_entry.rs`).
    pub(super) fn substituted_type_recipe(&self, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>) -> Recipe {
        let empty = HashMap::new();
        let mut r = Recipe { text: String::new(), variables: false };
        self.write_type(t, &mut Vec::new(), subst.unwrap_or(&empty), &mut r);
        r
    }

    fn write_type(
        &self,
        t: TypeId,
        linked_set: &mut Vec<TypeParamId>,
        subst: &HashMap<TypeParamId, TypeId>,
        r: &mut Recipe,
    ) {
        // Um tipo de extensão é, em tempo de execução, o da representação.
        if matches!(self.ctx.table.get(t), T::ExtensionType { .. }) {
            let erased = self.ctx.erase(t);
            if erased != t {
                return self.write_type(erased, linked_set, subst, r);
            }
        }
        let nullable = self.ctx.table.get(t).is_declared_nullable();
        match self.ctx.table.get(t) {
            T::Dynamic => r.text.push('D'),
            T::Void => r.text.push('V'),
            T::Never => r.text.push('N'),
            T::Null => r.text.push('U'),
            T::Interface { class, args, .. } => {
                r.text.push_str(&format!("C{}", self.ctx.id_rti(*class)));
                if !args.is_empty() {
                    r.text.push('<');
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            r.text.push(',');
                        }
                        self.write_type(*a, linked_set, subst, r);
                    }
                    r.text.push('>');
                }
            }
            T::FutureOr { arg, .. } => {
                r.text.push_str("O<");
                self.write_type(*arg, linked_set, subst, r);
                r.text.push('>');
            }
            // `X & B` (variável de tipo promovida) só existe na análise
            // estática; reificado, é a própria `X` (como no dart2js e na
            // NORM da especificação da inferência).
            T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
                if let Some(&s) = subst.get(param) {
                    self.write_type(s, linked_set, subst, r);
                } else if let Some(i) = linked_set.iter().rposition(|x| x == param) {
                    r.text.push_str(&format!("B{i}"));
                } else if let Some((letter, i)) = self.variable_of(*param) {
                    r.text.push_str(&format!("{letter}{i}"));
                    r.variables = true;
                } else {
                    r.text.push('D');
                }
            }
            T::Function { type_params, ret, positional, optional, named, .. } => {
                let n = linked_set.len();
                linked_set.extend(type_params.iter().copied());
                r.text.push_str(&format!("F<{};", type_params.len()));
                self.write_type(*ret, linked_set, subst, r);
                r.text.push_str(&format!(";{};", positional.len()));
                for (i, p) in positional.iter().chain(optional.iter()).enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_type(*p, linked_set, subst, r);
                }
                r.text.push(';');
                for (i, (name_str, p, req)) in named.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(*name_str));
                    r.text.push(':');
                    self.write_type(*p, linked_set, subst, r);
                    if *req {
                        r.text.push('!');
                    }
                }
                r.text.push('>');
                linked_set.truncate(n);
            }
            T::Record { positional, named, .. } => {
                r.text.push_str("R<");
                for (i, p) in positional.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_type(*p, linked_set, subst, r);
                }
                r.text.push(';');
                for (i, (name_str, p)) in named.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(*name_str));
                    r.text.push(':');
                    self.write_type(*p, linked_set, subst, r);
                }
                r.text.push('>');
            }
            T::ExtensionType { .. } => r.text.push('D'),
        }
        if nullable && !matches!(self.ctx.table.get(t), T::Dynamic | T::Void | T::Null) {
            r.text.push('?');
        }
    }

    /// A posição na tupla (`M<i>`) do parâmetro de tipo `sym` de uma
    /// closure genérica em escopo (o mais interno com esse nome).
    pub(super) fn local_type_index(&self, sym: SymbolId) -> Option<usize> {
        self.local_params.iter().rev().find(|(s, _, _)| *s == sym).map(|&(_, _, i)| i)
    }

    /// Quantos argumentos de tipo a tupla corrente carrega: os da função
    /// (e da extensão), os da classe numa fábrica, os das closures genéricas
    /// em volta. Zero sem tupla.
    pub(super) fn tuple_size(&self) -> usize {
        if self.type_tuple.is_none() {
            return 0;
        }
        let class = if self.class_by_tuple { self.class_params().len() } else { 0 };
        let locals = self.local_params.iter().map(|&(_, _, i)| i + 1).max().unwrap_or(0);
        self.function_type_params.len().max(class).max(locals)
    }

    /// Os nomes dos parâmetros de tipo da classe envolvente (na ordem).
    pub(super) fn class_params(&self) -> Vec<SymbolId> {
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
    pub fn annotation_recipe(&self, a: &ast::TypeAnnotation) -> Option<Recipe> {
        let mut r = Recipe { text: String::new(), variables: false };
        self.write_annotation(a, &mut Vec::new(), &mut r)?;
        Some(r)
    }

    /// Argumentos reificados de um tipo anotado, para a ABI de uma factory
    /// redirecionadora. A receita usa o ambiente da factory de origem:
    /// `Impl<T>` precisa ler `T` da tupla recebida por ela.
    pub fn annotation_tuple(&mut self, a: &ast::TypeAnnotation) -> Option<Operand> {
        let TypeKind::Named { args, .. } = &a.kind else { return None };
        if args.is_empty() { return None; }
        let args = args.clone();
        let mut r = Recipe { text: "L<".to_string(), variables: false };
        for (i, arg) in args.iter().enumerate() {
            if i > 0 { r.text.push(','); }
            let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
            let sub = self.annotation_recipe(unit_ast.ty(*arg))?;
            r.text.push_str(&sub.text);
            r.variables |= sub.variables;
        }
        r.text.push('>');
        Some(self.recipe_rti(&r))
    }

    /// Receita de uma variável de tipo em escopo, pelo nome: da função (e da
    /// extensão), depois da classe. `None` quando `sym` não é uma delas.
    pub fn type_variable_recipe(&self, sym: SymbolId) -> Option<Recipe> {
        let text = if let Some(i) = self.local_type_index(sym) {
            format!("M{i}")
        } else if let Some(i) = self.function_type_params.iter().position(|s| *s == sym) {
            format!("M{i}")
        } else {
            let i = self.class_params().iter().position(|s| *s == sym)?;
            format!("{}{i}", if self.class_by_tuple { 'M' } else { 'P' })
        };
        Some(Recipe { text, variables: true })
    }

    fn write_annotation(&self, a: &ast::TypeAnnotation, linked_set: &mut Vec<SymbolId>, r: &mut Recipe) -> Option<()> {
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        match &a.kind {
            TypeKind::Void => r.text.push('V'),
            TypeKind::Named { name, args } => {
                let last_one = name.last()?;
                let name_str = self.ctx.symbol_name(last_one.sym);
                let simple = name.len() == 1;
                if simple && let Some(i) = linked_set.iter().rposition(|s| *s == last_one.sym) {
                    r.text.push_str(&format!("B{i}"));
                } else if simple && let Some(i) = self.local_type_index(last_one.sym) {
                    r.text.push_str(&format!("M{i}"));
                    r.variables = true;
                } else if simple && let Some(i) = self.function_type_params.iter().position(|s| *s == last_one.sym) {
                    r.text.push_str(&format!("M{i}"));
                    r.variables = true;
                } else if simple && let Some(i) = self.class_params().iter().position(|s| *s == last_one.sym) {
                    r.text.push_str(&format!("{}{i}", if self.class_by_tuple { 'M' } else { 'P' }));
                    r.variables = true;
                } else if simple && name_str == "dynamic" {
                    r.text.push('D');
                } else if simple && name_str == "Never" {
                    r.text.push('N');
                } else if simple && name_str == "Null" {
                    r.text.push('U');
                } else if simple && name_str == "FutureOr" {
                    r.text.push_str("O<");
                    match args.first() {
                        Some(x) => self.write_annotation(unit_ast.ty(*x), linked_set, r)?,
                        None => r.text.push('D'),
                    }
                    r.text.push('>');
                } else {
                    let lib = self.ctx.program.unit(self.unit_id).library;
                    let b = match &name[..] {
                        [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
                        _ => self.ctx.program.lookup(lib, last_one.sym),
                    }?;
                    match b.getter? {
                        // Tipo de extensão: a receita da representação, com
                        // os argumentos escritos (como um typedef).
                        Element::Class(c) if self.ctx.is_extension_type(c) => {
                            let this = *self.ctx.te.this.get(&c)?;
                            let params = self.ctx.outline.classes.get(c.0 as usize)?.type_params.clone();
                            let mut sub = Recipe { text: String::new(), variables: false };
                            let mut strings = Vec::new();
                            for i in 0..params.len() {
                                sub.text.clear();
                                match args.get(i) {
                                    Some(x) => self.write_annotation(unit_ast.ty(*x), linked_set, &mut sub)?,
                                    None => sub.text.push('D'),
                                }
                                r.variables |= sub.variables;
                                strings.push(sub.text.clone());
                            }
                            let mut alvo = self.type_recipe_with(self.ctx.erase(this), &params, &strings);
                            r.variables |= alvo.variables;
                            if a.nullable && !alvo.text.ends_with('?') {
                                alvo.text.push('?');
                            }
                            r.text.push_str(&alvo.text);
                            return Some(());
                        }
                        Element::Class(c) => {
                            r.text.push_str(&format!("C{}", self.ctx.id_rti(c)));
                            let n = self.ctx.outline.classes.get(c.0 as usize).map_or(0, |d| d.type_params.len());
                            if n > 0 {
                                r.text.push('<');
                                for i in 0..n {
                                    if i > 0 {
                                        r.text.push(',');
                                    }
                                    match args.get(i) {
                                        Some(x) => self.write_annotation(unit_ast.ty(*x), linked_set, r)?,
                                        None => r.text.push('D'),
                                    }
                                }
                                r.text.push('>');
                            }
                        }
                        Element::Typedef(td) => {
                            let data = self.ctx.outline.typedefs.get(td.0 as usize)?;
                            // Os argumentos do typedef entram pela receita de
                            // cada um (sub-receitas escritas em texto).
                            let mut sub = Recipe { text: String::new(), variables: false };
                            let mut strings = Vec::new();
                            for x in args.iter() {
                                sub.text.clear();
                                self.write_annotation(unit_ast.ty(*x), linked_set, &mut sub)?;
                                strings.push(sub.text.clone());
                            }
                            r.variables |= sub.variables;
                            let mut alvo = self.type_recipe_with(data.target_type, &data.type_params, &strings);
                            r.variables |= alvo.variables;
                            if a.nullable {
                                alvo.text.push('?');
                            }
                            r.text.push_str(&alvo.text);
                            return Some(());
                        }
                        _ => return None,
                    }
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                let n = linked_set.len();
                linked_set.extend(type_params.iter().map(|p| p.name.sym));
                r.text.push_str(&format!("F<{};", type_params.len()));
                match return_type {
                    Some(x) => self.write_annotation(unit_ast.ty(*x), linked_set, r)?,
                    None => r.text.push('D'),
                }
                let oblig = parameters.iter().filter(|p| p.kind == ast::ParameterKind::Required).count();
                r.text.push_str(&format!(";{oblig};"));
                let mut first_one = true;
                for p in parameters.iter().filter(|p| p.kind != ast::ParameterKind::Named) {
                    if !first_one {
                        r.text.push(',');
                    }
                    first_one = false;
                    match p.ty {
                        Some(x) => self.write_annotation(unit_ast.ty(x), linked_set, r)?,
                        None => r.text.push('D'),
                    }
                }
                r.text.push(';');
                let mut first_one = true;
                for p in parameters.iter().filter(|p| p.kind == ast::ParameterKind::Named) {
                    if !first_one {
                        r.text.push(',');
                    }
                    first_one = false;
                    r.text.push_str(self.ctx.symbol_name(p.name?.sym));
                    r.text.push(':');
                    match p.ty {
                        Some(x) => self.write_annotation(unit_ast.ty(x), linked_set, r)?,
                        None => r.text.push('D'),
                    }
                    if p.required {
                        r.text.push('!');
                    }
                }
                r.text.push('>');
                linked_set.truncate(n);
            }
            TypeKind::Record { positional, named } => {
                r.text.push_str("R<");
                for (i, x) in positional.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_annotation(unit_ast.ty(*x), linked_set, r)?;
                }
                r.text.push(';');
                let mut fields: Vec<&(ast::Name, ast::TypeId)> = named.iter().collect();
                fields.sort_by_key(|(n, _)| self.ctx.symbol_name(n.sym).to_string());
                for (i, (n, x)) in fields.into_iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(n.sym));
                    r.text.push(':');
                    self.write_annotation(unit_ast.ty(*x), linked_set, r)?;
                }
                r.text.push('>');
            }
        }
        let is_nullable_by_itself = matches!(r.text.as_bytes().last(), Some(b'D' | b'V' | b'U'));
        if a.nullable && !is_nullable_by_itself {
            r.text.push('?');
        }
        Some(())
    }

    /// Receita de `t` com os parâmetros `params` trocados pelas receitas
    /// `args` (os de um typedef).
    pub(super) fn type_recipe_with(&self, t: TypeId, params: &[TypeParamId], args: &[String]) -> Recipe {
        // Escreve com marcadores e troca: os parâmetros do typedef não são de
        // classe nem de função, então `write_type` os escreveria como `D`.
        let mut r = Recipe { text: String::new(), variables: false };
        self.write_typedef_type(t, params, args, &mut Vec::new(), &mut r);
        r
    }

    fn write_typedef_type(
        &self,
        t: TypeId,
        params: &[TypeParamId],
        args: &[String],
        linked_set: &mut Vec<TypeParamId>,
        r: &mut Recipe,
    ) {
        if let T::TypeParameter { param, nullable } = self.ctx.table.get(t)
            && let Some(i) = params.iter().position(|p| p == param)
        {
            r.text.push_str(args.get(i).map_or("D", String::as_str));
            if *nullable {
                r.text.push('?');
            }
            return;
        }
        // Os demais casos: a receita normal, descendo pelos argumentos com a
        // mesma troca (os tipos compostos são reescritos por partes).
        match self.ctx.table.get(t) {
            T::Interface { class, args: a, nullable } => {
                r.text.push_str(&format!("C{}", self.ctx.id_rti(*class)));
                if !a.is_empty() {
                    r.text.push('<');
                    for (i, x) in a.iter().enumerate() {
                        if i > 0 {
                            r.text.push(',');
                        }
                        self.write_typedef_type(*x, params, args, linked_set, r);
                    }
                    r.text.push('>');
                }
                if *nullable {
                    r.text.push('?');
                }
            }
            T::Function { type_params, ret, positional, optional, named, nullable } => {
                let n = linked_set.len();
                linked_set.extend(type_params.iter().copied());
                r.text.push_str(&format!("F<{};", type_params.len()));
                self.write_typedef_type(*ret, params, args, linked_set, r);
                r.text.push_str(&format!(";{};", positional.len()));
                for (i, p) in positional.iter().chain(optional.iter()).enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_typedef_type(*p, params, args, linked_set, r);
                }
                r.text.push(';');
                for (i, (name_str, p, req)) in named.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(*name_str));
                    r.text.push(':');
                    self.write_typedef_type(*p, params, args, linked_set, r);
                    if *req {
                        r.text.push('!');
                    }
                }
                r.text.push('>');
                linked_set.truncate(n);
                if *nullable {
                    r.text.push('?');
                }
            }
            T::FutureOr { arg, nullable } => {
                r.text.push_str("O<");
                self.write_typedef_type(*arg, params, args, linked_set, r);
                r.text.push('>');
                if *nullable {
                    r.text.push('?');
                }
            }
            T::Record { positional, named, nullable } => {
                r.text.push_str("R<");
                for (i, p) in positional.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_typedef_type(*p, params, args, linked_set, r);
                }
                r.text.push(';');
                for (i, (name_str, p)) in named.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(*name_str));
                    r.text.push(':');
                    self.write_typedef_type(*p, params, args, linked_set, r);
                }
                r.text.push('>');
                if *nullable {
                    r.text.push('?');
                }
            }
            _ => self.write_type(t, linked_set, &HashMap::new(), r),
        }
    }

    /// O tipo da receita, como `I64`, no ambiente corrente.
    pub fn recipe_rti(&mut self, r: &Recipe) -> Operand {
        let h = recipe_hash(&r.text);
        // A mesma receita pode surgir no programa e em várias bibliotecas do
        // SDK. Cada biblioteca possui seu próprio cache preguiçoso; nomes
        // distintos evitam definições múltiplas na ligação ThinLTO.
        let lib = self.ctx.program.unit(self.unit_id).library;
        let owner = crate::context::escape_str(&self.ctx.library_name(lib));
        let getter = format!("df.rti.{h}.{owner}");
        let global = format!("dfr.{h}.{owner}");
        if !self.done_entries.contains(&getter) {
            self.done_entries.insert(getter.clone());
            let mut g = FnBuilder::new(self.ctx, self.unit_id, getter.clone(), "rti".to_string(), Type::I64);
            let ok = g.emit(
                Instruction::LoadGlobal {
                    symbol: format!("{global}$ok"),
                    ty: Type::I8,
                },
                Type::I8,
            );
            let ready = g.emit(Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))), Type::I1);
            let b_ready = g.new_block();
            let b_read = g.new_block();
            g.terminate(Terminator::CondBranch {
                cond: ready,
                then_block: b_ready,
                else_block: b_read,
            });
            g.set_block(b_ready);
            let v = g.emit(
                Instruction::LoadGlobal {
                    symbol: global.clone(),
                    ty: Type::I64,
                },
                Type::I64,
            );
            g.terminate(Terminator::Return(Some(v)));
            g.set_block(b_read);
            let s = g.emit(Instruction::Const(Constant::String(r.text.clone())), Type::Ref);
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
                    symbol: global.clone(),
                    val: t.clone(),
                    ty: Type::I64,
                    root: None,
                },
                Type::Void,
            );
            g.emit(
                Instruction::StoreGlobal {
                    symbol: format!("{global}$ok"),
                    val: Operand::Constant(Constant::Int(1)),
                    ty: Type::I8,
                    root: None,
                },
                Type::Void,
            );
            g.terminate(Terminator::Return(Some(t)));
            if self.ctx.arc_memory { self.arc_rti_caches.insert(global.clone()); }
            self.extra_globals.push((0, Type::I64, global));
            self.absorb(g);
        }
        let base = self.emit(
            Instruction::CallStatic {
                symbol: getter,
                args: Vec::new(),
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        if !r.variables {
            return base;
        }
        let this = if self.class_by_tuple {
            Operand::Constant(Constant::Null)
        } else {
            self.this_param.clone().map_or(Operand::Constant(Constant::Null), |t| self.coerce(t, Type::Ref))
        };
        let class = self.enclosing_class.map_or(0, |c| self.ctx.id_rti(c));
        let tuple = self.type_tuple.clone().unwrap_or(Operand::Constant(Constant::Int(0)));
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_avaliar".to_string(),
                args: vec![
                    (base, Type::I64),
                    (this, Type::Ref),
                    (Operand::Constant(Constant::Int(class)), Type::I64),
                    (tuple, Type::I64),
                ],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// O tipo `t` do `crates/types`, como `I64`, no ambiente corrente.
    pub fn type_rti(&mut self, t: TypeId) -> Operand {
        let r = self.type_recipe(t);
        self.recipe_rti(&r)
    }

    /// Grava o tipo em tempo de execução de um objeto (`Ref`).
    pub fn define_rti(&mut self, obj: Operand, ty: Operand) {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_definir".to_string(),
                args: vec![(obj, Type::Ref), (ty, Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }

    /// Grava em `obj` o tipo estático `t` quando ele diz algo que a classe
    /// não diz (tem argumentos de tipo, ou é tipo de função).
    pub fn define_rti_if_generic(&mut self, obj: Operand, t: TypeId) {
        let useful = match self.ctx.table.get(t) {
            T::Interface { args, .. } => args.iter().any(|a| *a != self.ctx.core.dynamic_),
            T::Function { .. } => true,
            _ => false,
        };
        if useful {
            let ty = self.type_rti(t);
            self.define_rti(obj, ty);
        }
    }

    /// O literal de coleção `l` com o tipo estático da expressão (quando o
    /// literal não terminou o bloco).
    pub fn literal_rti(&mut self, l: Operand, expr: ast::ExprId) -> Operand {
        if !self.is_terminated()
            && self.operand_type(&l) == Type::Ref
            && let Some(t) = self.ctx.get_type(self.unit_id, expr)
        {
            self.define_rti_if_generic(l.clone(), t);
        }
        l
    }

    /// `op is <ty>` (I1) pelo RTI.
    pub fn test_rti(&mut self, op: Operand, ty: Operand) -> Operand {
        let v = self.coerce(op, Type::Ref);
        let r = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_e".to_string(),
                args: vec![(v, Type::Ref), (ty, Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        self.emit(Instruction::ICmp(ICmpOp::Ne, r, Operand::Constant(Constant::Int(0))), Type::I1)
    }

    /// `op as <ty>` pelo RTI: `TypeError` pendente se falha.
    pub fn cast_rti(&mut self, op: Operand, ty: Operand) {
        self.cast_rti_in(op, ty, CastContext::How);
    }

    /// [`Self::cast_rti`] com o contexto que dá o fim da mensagem da VM.
    pub fn cast_rti_in(&mut self, op: Operand, ty: Operand, context: CastContext) {
        let v = self.coerce(op, Type::Ref);
        // O nome do parâmetro só entra na mensagem do `TypeError`: a
        // conferência vai antes pelo `is` (`dartforge_rti_e`, o mesmo teste
        // do `como_em`), e o texto do nome só é montado no caminho da
        // falha. Antes, toda entrada covariante ou dinâmica (o `[]=` de
        // `_List`, os adaptadores da tabela de métodos) pagava um
        // `dartforge_string_new` por parâmetro a cada chamada.
        let (code, name_str, end) = match context {
            CastContext::How => (0, Operand::Constant(Constant::Null), None),
            CastContext::Implicit => (1, Operand::Constant(Constant::Null), None),
            CastContext::Parameter(n) => {
                let ok = self.test_rti(v.clone(), ty.clone());
                let b_fail = self.new_block();
                let b_end = self.new_block();
                self.terminate(Terminator::CondBranch {
                    cond: ok,
                    then_block: b_end,
                    else_block: b_fail,
                });
                self.set_block(b_fail);
                (2, self.emit(Instruction::Const(Constant::String(n)), Type::Ref), Some(b_end))
            }
        };
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_rti_como_em".to_string(),
                args: vec![(v, Type::Ref), (ty, Type::I64), (Operand::Constant(Constant::Int(code)), Type::I64), (name_str, Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        if let Some(b_end) = end {
            if !self.is_terminated() {
                self.terminate(Terminator::Branch(b_end));
            }
            self.set_block(b_end);
        }
    }

    /// Na entrada de uma chamada dinâmica (tear-off, adaptador da tabela de
    /// métodos), confere cada argumento contra o tipo do parâmetro antes de
    /// convertê-lo, como a VM: "type 'S' is not a subtype of type 'T' of
    /// 'nome'". Só os parâmetros de tipo nominal (classe) e os covariantes
    /// de classe; os outros (`dynamic`, `Object?`, tipos de função e
    /// parâmetros de tipo da função) ficam com a conversão de sempre.
    pub fn check_entry_arguments(&mut self, fid: usize, vals: &[Operand]) {
        self.check_entry_arguments_with(fid, vals, false);
    }

    /// [`Self::check_entry_arguments`]; na entrada tipada
    /// (`only_covariants`, `typed_entry.rs`), só os parâmetros
    /// covariantes: os outros o chamador estático já garantiu.
    pub fn check_entry_arguments_with(&mut self, fid: usize, vals: &[Operand], only_covariants: bool) {
        for (i, (p, v)) in self.ctx.outline.functions[fid].parameters.clone().iter().zip(vals).enumerate() {
            if !super::typed_entry::checked_at_entry(self.ctx, fid, p.ty) {
                continue;
            }
            if only_covariants && !super::typed_entry::covariant_parameter(self.ctx, fid, i) {
                continue;
            }
            let name_str = p.name.map(|n| self.ctx.symbol_name(n).to_string()).unwrap_or_default();
            let ty = self.type_rti(p.ty);
            self.cast_rti_in(v.clone(), ty, CastContext::Parameter(name_str));
        }
    }

    /// Arma a tupla de argumentos de tipo da chamada `expr` à função `fid`
    /// (se ela é genérica): os escritos (`f<int>(…)`), ou os inferidos,
    /// casando o retorno declarado com o tipo estático da chamada e cada
    /// parâmetro com o tipo do argumento. O que não se deduz fica `dynamic`.
    pub fn arm_tuple(&mut self, fid: usize, expr: ast::ExprId, arguments: &ast::Arguments) {
        self.arm_tuple_with_receiver(fid, None, Some((expr, arguments)));
    }

    /// `arm_tuple` de um membro de extensão: os argumentos de tipo da
    /// extensão saem do tipo estático do receptor casado com o `on` (como
    /// na inferência da aplicação de extensão), os do membro da chamada.
    /// Sem `call` (getter, setter, operador), só a parte da extensão.
    pub fn arm_tuple_with_receiver(
        &mut self,
        fid: usize,
        receiver: Option<TypeId>,
        call: Option<(ast::ExprId, &ast::Arguments)>,
    ) {
        if !self.generic_function(fid) {
            return;
        }
        let params = self.type_params_of(fid);
        let extension = self.receiver_pattern(fid);
        let n_ext = extension.map_or(0, |(n, _)| n);
        let mut found_items: Vec<Option<TypeId>> = vec![None; params.len()];
        if let (Some((_, on)), Some(r)) = (extension, receiver) {
            self.unify(on, r, &params, &mut found_items);
        }
        let data = &self.ctx.outline.functions[fid];
        let mut written_set: Vec<Recipe> = Vec::new();
        if let Some((expr, arguments)) = call {
            let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
            if !arguments.type_args.is_empty() {
                for a in &arguments.type_args {
                    let x = self.annotation_recipe(unit_ast.ty(*a));
                    written_set.push(x.unwrap_or(Recipe { text: "D".to_string(), variables: false }));
                }
            } else {
                if let Some(real) = self.ctx.get_type_raw(self.unit_id, expr) {
                    self.unify(data.return_type, real, &params, &mut found_items);
                }
                let mut positionals = arguments.args.iter().filter(|a| a.name.is_none());
                for p in data.parameters.iter() {
                    let arg = if p.kind == ast::ParameterKind::Named {
                        arguments.args.iter().find(|a| a.name.map(|n| n.sym) == p.name)
                    } else {
                        positionals.next()
                    };
                    if let Some(a) = arg
                        && let Some(real) = self.ctx.get_type_raw(self.unit_id, a.value)
                    {
                        self.unify(p.ty, real, &params, &mut found_items);
                    }
                }
            }
        }
        let mut r = Recipe { text: "L<".to_string(), variables: false };
        for (i, found) in found_items.iter().enumerate() {
            if i > 0 {
                r.text.push(',');
            }
            let x = match written_set.get(i.wrapping_sub(n_ext)).filter(|_| i >= n_ext) {
                Some(e) => e.clone(),
                None => self.type_recipe(found.unwrap_or(self.ctx.core.dynamic_)),
            };
            r.text.push_str(&x.text);
            r.variables |= x.variables;
        }
        r.text.push('>');
        let t = self.recipe_rti(&r);
        self.armed_tuple = Some(t);
    }

    /// Casa o tipo declarado `decl` (com os parâmetros `params`) com o tipo
    /// real `real`, gravando em `found_items` o que cada parâmetro vale.
    /// Se `C<args>` é um `Future`, o argumento do supertipo `Future<X>`
    /// (`Some(None)` quando `X` não se escreve só com os `args`); `None`
    /// quando não é `Future`.
    fn future_argument(&self, class: dartforge_elements::model::ClassId, args: &[TypeId]) -> Option<Option<TypeId>> {
        let future = self.ctx.core.future_class?;
        if class == future {
            return Some(args.first().copied());
        }
        let data = self.ctx.outline.hierarchy.get(class)?;
        let model = *data.supertypes.get(&future)?;
        let T::Interface { args: margs, .. } = self.ctx.table.get(model) else { return Some(None) };
        let Some(&x) = margs.first() else { return Some(None) };
        Some(match self.ctx.table.get(x) {
            T::TypeParameter { param, .. } => data.type_params.iter().position(|p| p == param).and_then(|i| args.get(i).copied()),
            _ if !self.contains_variable(x) => Some(x),
            _ => None,
        })
    }

    /// O tipo menciona algum parâmetro de tipo.
    fn contains_variable(&self, t: TypeId) -> bool {
        match self.ctx.table.get(t) {
            T::TypeParameter { .. } | T::Intersection { .. } => true,
            T::Interface { args, .. } => args.iter().any(|a| self.contains_variable(*a)),
            T::FutureOr { arg, .. } => self.contains_variable(*arg),
            T::Function { .. } | T::Record { .. } => true,
            _ => false,
        }
    }

    pub(super) fn unify(&self, decl: TypeId, real: TypeId, params: &[TypeParamId], found_items: &mut [Option<TypeId>]) {
        if real == self.ctx.core.dynamic_ {
            return;
        }
        // Tipos de extensão: pelos argumentos quando os dois são do mesmo
        // (ou o real implementa o do padrão); senão pelos apagados.
        match (self.ctx.table.get(decl), self.ctx.table.get(real)) {
            (T::ExtensionType { decl: d1, args: a1, .. }, T::ExtensionType { decl: d2, args: a2, .. }) => {
                if d1 == d2 {
                    for (x, y) in a1.iter().zip(a2.iter()) {
                        self.unify(*x, *y, params, found_items);
                    }
                } else if let Some(&s) = self.ctx.te.supertypes.get(&(real, *d1)) {
                    self.unify(decl, s, params, found_items);
                }
                return;
            }
            (T::ExtensionType { .. }, _) | (_, T::ExtensionType { .. }) => {
                let (a, b) = (self.ctx.erase(decl), self.ctx.erase(real));
                if (a, b) != (decl, real) {
                    self.unify(a, b, params, found_items);
                }
                return;
            }
            _ => {}
        }
        match (self.ctx.table.get(decl), self.ctx.table.get(real)) {
            (T::TypeParameter { param, nullable }, _) => {
                // `X?` casado com `T?` dá `X = T` (a inferência tira o `?`
                // que o padrão já tem): o `nonNulls` de uma `List<T?>` é
                // `Iterable<T>`. Sem a forma não anulável na tabela (só de
                // leitura aqui), fica o `T?`.
                let real = if *nullable { self.without_question_mark(real).unwrap_or(real) } else { real };
                if let Some(i) = params.iter().position(|p| p == param)
                    && found_items[i].is_none()
                {
                    found_items[i] = Some(real);
                }
            }
            (T::Interface { class: c1, args: a1, .. }, T::Interface { class: c2, args: a2, .. }) if c1 == c2 => {
                for (x, y) in a1.iter().zip(a2.iter()) {
                    self.unify(*x, *y, params, found_items);
                }
            }
            // Classes diferentes: o padrão casa com o supertipo do real
            // (`Iterable<X>` contra `List<int>` é `Iterable<int>`), como a
            // inferência faz pela interface do tipo real.
            (T::Interface { class: c1, args: a1, .. }, T::Interface { class: c2, args: a2, .. }) => {
                let Some(data) = self.ctx.outline.hierarchy.get(*c2) else { return };
                let Some(&model) = data.supertypes.get(c1) else { return };
                let T::Interface { args: margs, .. } = self.ctx.table.get(model) else { return };
                for (x, m) in a1.iter().zip(margs.iter()) {
                    self.unify_by_model(*x, *m, &data.type_params, a2, params, found_items);
                }
            }
            (T::FutureOr { arg, .. }, T::FutureOr { arg: b, .. }) => self.unify(*arg, *b, params, found_items),
            // `FutureOr<X>` contra um `Future` casa `X` com o argumento do
            // `Future` (a inferência tenta `Future<X>` primeiro): também
            // para um subtipo (`_Future<T>` implementa `Future<T>`).
            (T::FutureOr { arg, .. }, T::Interface { class, args, .. }) => {
                match self.future_argument(*class, args) {
                    Some(Some(x)) => self.unify(*arg, x, params, found_items),
                    // É `Future`, mas o argumento não se deduz: fica livre.
                    Some(None) => {}
                    None => self.unify(*arg, real, params, found_items),
                }
            }
            (
                T::Function { ret: r1, positional: p1, .. },
                T::Function { ret: r2, positional: p2, .. },
            ) => {
                self.unify(*r1, *r2, params, found_items);
                for (x, y) in p1.iter().zip(p2.iter()) {
                    self.unify(*x, *y, params, found_items);
                }
            }
            // Registros: campo a campo (`extension E<A, B> on (P<A>, P<B>)`
            // contra `(P<String>, P<int>)`; o `toSequenceParser` do
            // petitparser), posicionais pela posição e nomeados pelo nome.
            (
                T::Record { positional: p1, named: n1, .. },
                T::Record { positional: p2, named: n2, .. },
            ) if p1.len() == p2.len() && n1.len() == n2.len() => {
                for (x, y) in p1.iter().zip(p2.iter()) {
                    self.unify(*x, *y, params, found_items);
                }
                for (name_str, x) in n1.iter() {
                    if let Some((_, y)) = n2.iter().find(|(m, _)| m == name_str) {
                        self.unify(*x, *y, params, found_items);
                    }
                }
            }
            _ => {}
        }
    }

    /// [`Self::unify`] do padrão `decl` com o argumento `model` de um
    /// supertipo, escrito nos parâmetros `formals` da classe real, cujos
    /// argumentos são `real_set`: onde o modelo é um formal, o real é o
    /// argumento correspondente; um modelo sem variáveis casa como está.
    fn unify_by_model(
        &self,
        decl: TypeId,
        model: TypeId,
        formals: &[TypeParamId],
        real_set: &[TypeId],
        params: &[TypeParamId],
        found_items: &mut [Option<TypeId>],
    ) {
        match (self.ctx.table.get(decl), self.ctx.table.get(model)) {
            (_, T::TypeParameter { param, nullable: false }) => {
                if let Some(&r) = formals.iter().position(|f| f == param).and_then(|k| real_set.get(k)) {
                    self.unify(decl, r, params, found_items);
                }
            }
            (T::Interface { class: c1, args: a1, .. }, T::Interface { class: c2, args: m2, .. }) if c1 == c2 => {
                for (x, m) in a1.iter().zip(m2.iter()) {
                    self.unify_by_model(*x, *m, formals, real_set, params, found_items);
                }
            }
            _ if !self.contains_variable(model) => self.unify(decl, model, params, found_items),
            _ => {}
        }
    }

    /// O tipo do objeto e a tupla da classe genérica `c` de um tear-off de
    /// construtor cujo tipo estático (`ty`) já é instanciado pela
    /// inferência (`C<int> Function()`, sem parâmetros de tipo próprios);
    /// `None` para classe não genérica ou tear-off genérico.
    pub fn tearoff_instantiation(&mut self, c: ClassId, ty: TypeId) -> Option<(Operand, Operand)> {
        let n = self.ctx.outline.classes.get(c.0 as usize)?.type_params.len();
        if n == 0 {
            return None;
        }
        let T::Function { type_params, ret, .. } = self.ctx.table.get(ty) else { return None };
        if !type_params.is_empty() {
            return None;
        }
        let T::Interface { class, args, .. } = self.ctx.table.get(*ret) else { return None };
        if *class != c || args.len() != n {
            return None;
        }
        let args = args.clone();
        let mut strings = Vec::with_capacity(n);
        let mut variables = false;
        for a in args.iter() {
            let r = self.type_recipe(*a);
            variables |= r.variables;
            strings.push(r.text);
        }
        let object = self.recipe_rti(&Recipe { text: format!("C{}<{}>", self.ctx.id_rti(c), strings.join(",")), variables });
        let tuple = self.recipe_rti(&Recipe { text: format!("L<{}>", strings.join(",")), variables });
        Some((object, tuple))
    }

    /// A forma não anulável de `t` (`T?` → `T`, `C<A>?` → `C<A>`), se a
    /// tabela a tem.
    fn without_question_mark(&self, t: TypeId) -> Option<TypeId> {
        let ty = match self.ctx.table.get(t).clone() {
            T::TypeParameter { param, nullable: true } => T::TypeParameter { param, nullable: false },
            T::Interface { class, args, nullable: true } => T::Interface { class, args, nullable: false },
            T::FutureOr { arg, nullable: true } => T::FutureOr { arg, nullable: false },
            T::Record { positional, named, nullable: true } => T::Record { positional, named, nullable: false },
            T::ExtensionType { decl, args, nullable: true } => T::ExtensionType { decl, args, nullable: false },
            T::Function { type_params, ret, positional, optional, named, nullable: true } => {
                T::Function { type_params, ret, positional, optional, named, nullable: false }
            }
            _ => return None,
        };
        self.ctx.table.intern_lookup(ty)
    }

    /// Grava a assinatura de uma closure (`F<…>`): o tipo estático da
    /// expressão de função, se a inferência o tem, senão o que a declaração
    /// escreve (parâmetros sem tipo são `dynamic`). É o `$signature` do
    /// dart2js: o que `f is void Function(Object, StackTrace)` pergunta.
    /// O tipo de uma função literal (`expr`) ou local nomeada: o de execução
    /// de um gerador sem retorno escrito (a regra do CFE,
    /// `tipos_de_execucao_de_funcoes`), senão o inferido — o da expressão, ou
    /// o do local, cujo retorno sem tipo escrito vem do corpo (`g() { return
    /// 1; }` é `() => int`, não `() => dynamic`).
    pub fn literal_function_type(&self, ast: &ast::Ast, fid: ast::FunctionId, expr: Option<ast::ExprId>) -> Option<dartforge_types::table::TypeId> {
        let execution = self.ctx.bodies.units.get(self.unit_id.0 as usize).and_then(|u| u.tipo_de_execucao_de_funcao(fid));
        execution.or_else(|| match expr {
            Some(e) => self.ctx.get_type(self.unit_id, e),
            None => ast.function(fid).name.and_then(|n| self.ctx.local_type(self.unit_id, n.span.start as usize)),
        })
    }

    pub fn define_closure_rti(&mut self, clo: Operand, ast: &ast::Ast, fid: ast::FunctionId, expr: Option<ast::ExprId>) {
        // O tipo inferido: o da expressão (função literal) ou o do local
        // (função local nomeada, cujo retorno sem tipo escrito é inferido do
        // corpo — `g() { return 1; }` é `() => int`, não `() => dynamic`).
        if let Some(t) = self.literal_function_type(ast, fid, expr)
            && matches!(self.ctx.table.get(t), T::Function { .. })
        {
            let r = self.type_recipe(t);
            let ty = self.recipe_rti(&r);
            self.define_rti(clo, ty);
            return;
        }
        let f = ast.function(fid);
        let params = f.parameters.as_deref().unwrap_or(&[]);
        let mut r = Recipe { text: format!("F<{};", f.type_params.len()), variables: false };
        let mut linked_set: Vec<SymbolId> = f.type_params.iter().map(|p| p.name.sym).collect();
        let type_of = |s: &Self, t: Option<ast::TypeId>, r: &mut Recipe, linked_set: &mut Vec<SymbolId>| match t {
            Some(x) => {
                let mut sub = Recipe { text: String::new(), variables: false };
                if s.write_annotation(ast.ty(x), linked_set, &mut sub).is_some() {
                    r.text.push_str(&sub.text);
                    r.variables |= sub.variables;
                } else {
                    r.text.push('D');
                }
            }
            None => r.text.push('D'),
        };
        type_of(self, f.return_type, &mut r, &mut linked_set);
        let oblig = params.iter().filter(|p| p.kind == ast::ParameterKind::Required).count();
        r.text.push_str(&format!(";{oblig};"));
        let mut first_one = true;
        for p in params.iter().filter(|p| p.kind != ast::ParameterKind::Named) {
            if !first_one {
                r.text.push(',');
            }
            first_one = false;
            type_of(self, p.ty, &mut r, &mut linked_set);
        }
        r.text.push(';');
        let mut first_one = true;
        for p in params.iter().filter(|p| p.kind == ast::ParameterKind::Named) {
            let Some(n) = p.name else { continue };
            if !first_one {
                r.text.push(',');
            }
            first_one = false;
            r.text.push_str(self.ctx.symbol_name(n.sym));
            r.text.push(':');
            type_of(self, p.ty, &mut r, &mut linked_set);
            if p.required {
                r.text.push('!');
            }
        }
        r.text.push('>');
        let ty = self.recipe_rti(&r);
        self.define_rti(clo, ty);
    }

    /// Grava a assinatura do tear-off da função `fid` (a do outline; num
    /// método, avaliada no tipo do receptor `recv`).
    pub fn define_tearoff_rti(&mut self, clo: Operand, fid: usize, recv: Option<Operand>) {
        self.define_tearoff_rti_with_tuple(clo, fid, recv, None);
    }

    /// [`Self::define_tearoff_rti`] de um membro de extensão ou de tipo
    /// de extensão genéricos: os parâmetros de tipo do dono vêm de `tuple`
    /// (a de `arm_tuple_with_receiver`, calculada aqui), não de um objeto.
    pub fn define_tearoff_rti_with_tuple(&mut self, clo: Operand, fid: usize, recv: Option<Operand>, tuple: Option<Operand>) {
        let Some(sig) = self.ctx.outline.functions.get(fid).map(|d| d.signature) else {
            return;
        };
        let saved = (self.this_param.clone(), self.finalizable_this, self.enclosing_class, self.class_by_tuple, self.type_tuple.clone());
        if let Some(r) = recv {
            self.this_param = Some(r);
            self.finalizable_this = self.ctx.classify_this(fid);
            // Um tipo de extensão não tem objeto com os argumentos de tipo.
            self.enclosing_class = self.ctx.program.functions[fid].class.filter(|&c| !self.ctx.is_extension_type(c));
            self.class_by_tuple = false;
        }
        self.type_tuple = tuple;
        let r = self.type_recipe(sig);
        let ty = self.recipe_rti(&r);
        (self.this_param, self.finalizable_this, self.enclosing_class, self.class_by_tuple, self.type_tuple) = saved;
        self.define_rti(clo, ty);
    }

    /// O teste de tipo da anotação precisa do RTI? (argumentos de tipo que
    /// não são triviais, variável de tipo, `FutureOr`, tipo de função ou de
    /// record, typedef.)
    pub fn annotation_needs_rti(&self, a: &ast::TypeAnnotation) -> bool {
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        match &a.kind {
            TypeKind::Void => false,
            TypeKind::Function { .. } | TypeKind::Record { .. } => true,
            TypeKind::Named { name, args } => {
                let Some(last_one) = name.last() else { return false };
                let name_str = self.ctx.symbol_name(last_one.sym);
                if name.len() == 1
                    && (self.function_type_params.contains(&last_one.sym)
                        || self.local_type_index(last_one.sym).is_some()
                        || self.class_params().contains(&last_one.sym)
                        || name_str == "FutureOr")
                {
                    return true;
                }
                let lib = self.ctx.program.unit(self.unit_id).library;
                let b = match &name[..] {
                    [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
                    _ => self.ctx.program.lookup(lib, last_one.sym),
                };
                if matches!(b.and_then(|b| b.getter), Some(Element::Typedef(_))) {
                    return true;
                }
                // Tipo de extensão: o teste é o da representação (sem classe
                // própria no heap).
                if let Some(Element::Class(c)) = b.and_then(|b| b.getter)
                    && self.ctx.is_extension_type(c)
                {
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
pub fn register_universe(ctx: &Context, module: &mut Module) {
    // O universo é global ao executável; os objetos do SDK só fornecem as
    // receitas que usam. A entrada do programa registra o universo completo.
    if module.sdk_library {
        return;
    }
    let mut recipes: Vec<String> = Vec::new();
    let mut uses_rti = false;
    for f in &module.functions {
        let is_recipe = f.symbol.starts_with("df.rti.");
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    Instruction::Const(Constant::String(s)) if is_recipe => recipes.push(s.clone()),
                    Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_rti_") => uses_rti = true,
                    _ => {}
                }
            }
        }
    }
    if recipes.is_empty() && !uses_rti && !module.sdk_mode {
        return;
    }
    // O objeto `Type` (`dartforge_rti_objeto_tipo`): a classe e os seletores
    // usados pelo SDK da fonte. Cada tipo tem um único objeto canônico.
    {
        let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
            return;
        };
        let symbol = "df.$tipo.toString".to_string();
        let mut t = FnBuilder::new(ctx, u, symbol.clone(), "toString".to_string(), Type::Ref);
        let obj = Operand::Val(t.add_param("this".to_string(), Type::Ref));
        // O campo 0 do `_Type` é o ID RTI, escalar (`dartforge_rti_objeto_tipo`).
        // Sob ARC, a leitura confere o mapa real do campo; o texto ganha owner
        // na preparação ARC (`optimize/arc/natives.rs`), não aqui.
        let read = if ctx.arc_memory { "dartforge_arc_ler_campo_escalar_v1" } else { "dartforge_object_get" };
        let id = t.emit(
            Instruction::CallRuntime {
                name: read.to_string(),
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
        t.finish(module);
        // Corpo sintetizado: a convenção Dart (retorno Ref Owned) não vem de
        // `lower_function`; o chamador uniforme `toString$c` a exige.
        if ctx.arc_memory {
            module.dart_ref_returns.insert(symbol.clone());
        }
        let mut equality = FnBuilder::new(ctx, u, "df.$tipo.$3d$3d$c".to_string(), "==".to_string(), Type::Ref);
        let recv = Operand::Val(equality.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(equality.add_param_vector(DartEntryVector::Argumentos));
        equality.add_param_vector(DartEntryVector::Descriptor);
        equality.declare_arc_uniform_return();
        let other = equality.emit(
            Instruction::LoadIndexed { base: args, index: Operand::Constant(Constant::Int(0)) },
            Type::Ref,
        );
        let equal = equality.emit(Instruction::ICmp(ICmpOp::Eq, recv, other), Type::I1);
        let equal = equality.coerce(equal, Type::Ref);
        equality.terminate(Terminator::Return(Some(equal)));
        equality.finish(module);
        let mut text = FnBuilder::new(ctx, u, "df.$tipo.toString$c".to_string(), "toString".to_string(), Type::Ref);
        let recv = Operand::Val(text.add_param("this".to_string(), Type::Ref));
        text.add_param_vector(DartEntryVector::Argumentos);
        text.add_param_vector(DartEntryVector::Descriptor);
        text.declare_arc_uniform_return();
        let value = text.emit_call_with_check(
            Instruction::CallStatic { symbol: symbol.clone(), args: vec![recv], ret_ty: Type::Ref },
            Type::Ref,
        );
        text.terminate(Terminator::Return(Some(value)));
        text.finish(module);
        let mut methods = vec![
            ("c:==".to_string(), "df.$tipo.$3d$3d$c".to_string()),
            ("c:toString".to_string(), "df.$tipo.toString$c".to_string()),
        ];
        // `_Type` é criado pelo runtime, fora da hierarquia de elementos do
        // programa. Ele ainda herda os membros de `Object`, incluindo o
        // getter privado usado por `identityHashCode(Object)` no hashSeed.
        if let Some(object) = ctx.core.object_class {
            for (selector, symbol) in super::sdk_source::method_table(ctx, object) {
                if !methods.iter().any(|(existing, _)| *existing == selector) {
                    methods.push((selector, symbol));
                }
            }
        }
        module.method_tables.push((CLASSE_TIPO as u32, "df.mt.$tipo".to_string(), methods));
        module.classes.push(ClassDef {
            id: CLASSE_TIPO as u32,
            name: "_Type".to_string(),
            field_count: 1,
            vtable: Vec::new(),
            to_string_symbol: Some(symbol),
        });
        // O objeto `Type` criado no runtime não tem um ClassElement do
        // programa. A RTI conhece seu tipo, mas os testes nominais da fonte
        // usam a tabela separada de `dartforge_is_subclass`.
        if let Some(ty) = core_class(ctx, "Type").and_then(|c| ctx.class_id_of(c)) {
            module.subtyping_edges.push((CLASSE_TIPO as u32, ty));
        }
    }
    let mut cited: BTreeSet<i64> = BTreeSet::new();
    for r in &recipes {
        let b = r.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'C' && i + 1 < b.len() && (b[i + 1].is_ascii_digit() || b[i + 1] == b'-') {
                let mut j = i + 1;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'-') {
                    j += 1;
                }
                if let Ok(n) = r[i + 1..j].parse::<i64>() {
                    cited.insert(n);
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
    // As formas do runtime.
    let core = ctx.core;
    let shapes: Vec<(i64, Option<ClassId>)> = vec![
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
        (14, core_class(ctx, "Type")),
    ];
    let mut classes: Vec<ClassId> = Vec::new();
    let mut seen_set: std::collections::HashSet<ClassId> = std::collections::HashSet::new();
    let mut queue: Vec<ClassId> = Vec::new();
    for id in &cited {
        if let Some(c) = ctx.rti_id_class(*id) {
            queue.push(c);
        }
    }
    for (_, c) in &shapes {
        queue.extend(c.iter().copied());
    }
    for (i, _) in ctx.program.classes.iter().enumerate() {
        if ctx.class_id_of(ClassId(i as u32)).is_some() {
            queue.push(ClassId(i as u32));
        }
    }
    while let Some(c) = queue.pop() {
        if !seen_set.insert(c) {
            continue;
        }
        classes.push(c);
        if let Some(d) = ctx.outline.hierarchy.get(c) {
            for sup in d.supertypes.keys() {
                queue.push(*sup);
            }
        }
    }
    classes.sort_by_key(|c| ctx.id_rti(*c));
    let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
        return;
    };
    // Os registros vão numa TABELA constante que o runtime lê de uma vez
    // (`dartforge_rti_iniciar_tabela`), não em código: são milhares de
    // classes do SDK, e uma chamada por registro dava uma função de dezenas
    // de milhares de instruções que o LLVM compilava a cada `dartforge run`,
    // a cada recarga do JIT e a cada ligação AOT. Uma linha por registro:
    // `C<id> <parâmetros> <name>`, `R<id> <recipe>`, `F<shape> <id>`.
    let mut table = String::new();
    let mut b = FnBuilder::new(ctx, u, "dartforge_rti_iniciar".to_string(), "rti".to_string(), Type::Void);
    for &c in &classes {
        let id = ctx.id_rti(c);
        let name_str = visible_name(ctx, c);
        let n = ctx.outline.classes.get(c.0 as usize).map_or(0, |d| d.type_params.len());
        table.push_str(&format!("C{id} {n} {name_str}\n"));
        // A biblioteca que declara a classe e o nome declarado, para o
        // `reflectClass` do `dart:mirrors` (`runtime/src/mirrors.rs`).
        let class = &ctx.program.classes[c.0 as usize];
        let uri = ctx.program.library(class.library).uri.replace(' ', "%20");
        table.push_str(&format!("B{id} {uri} {}\n", ctx.symbol_name(class.name)));
        let Some(d) = ctx.outline.hierarchy.get(c) else { continue };
        let mut sups: Vec<(i64, TypeId)> = d.supertypes.iter().map(|(k, t)| (ctx.id_rti(*k), *t)).collect();
        sups.sort_by_key(|x| x.0);
        for (_, t) in sups {
            // A regra é escrita nos parâmetros da própria classe (`P<i>`).
            let saved = (b.enclosing_class, b.class_by_tuple);
            b.enclosing_class = Some(c);
            b.class_by_tuple = false;
            let r = b.type_recipe(t);
            b.enclosing_class = saved.0;
            b.class_by_tuple = saved.1;
            table.push_str(&format!("R{id} {}\n", r.text));
        }
    }
    for (shape, c) in shapes {
        if let Some(c) = c {
            table.push_str(&format!("F{shape} {}\n", ctx.id_rti(c)));
        }
    }
    // As classes de erro que o runtime representa (ids 1000–1012).
    for (heap_id, name_str) in [
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
        let Some(sym) = ctx.interner.lookup(name_str) else { continue };
        let Some(Element::Class(c)) = ctx.program.lookup(core_lib, sym).and_then(|b| b.getter) else { continue };
        table.push_str(&format!("F{heap_id} {}\n", ctx.id_rti(c)));
    }
    // A tabela grande vai comprimida (`dartforge_runtime::rti_table`): 1,37
    // MB de texto no new_sali/backend, ~260 KB comprimida.
    let bytes = table.into_bytes();
    let bytes = if bytes.len() >= dartforge_runtime::tabela_rti::LIMIAR { dartforge_runtime::tabela_rti::comprimir(&bytes) } else { bytes };
    // Os bytes empacotados em palavras (little-endian, como todos os alvos).
    let words: Vec<i64> = bytes
        .chunks(8)
        .map(|c| {
            let mut w = [0u8; 8];
            w[..c.len()].copy_from_slice(c);
            i64::from_le_bytes(w)
        })
        .collect();
    let data = b.emit(Instruction::ConstArray(words), Type::Ptr);
    b.emit(
        Instruction::CallRuntime {
            name: "dartforge_rti_iniciar_tabela".to_string(),
            args: vec![(data, Type::Ptr), (Operand::Constant(Constant::Int(bytes.len() as i64)), Type::I64)],
            ret_ty: Type::Void,
        },
        Type::Void,
    );
    b.terminate(Terminator::Return(None));
    b.finish(module);
    module.start_rti = Some("dartforge_rti_iniciar".to_string());
}

/// O nome de uma classe em `Type.toString()`: o `Class::UserVisibleName` da
/// VM troca as implementações internas do `dart:core` e do
/// `dart:typed_data` pela interface pública (`_Smi` → `int`,
/// `_GrowableList` → `List`, `_Uint8List` → `Uint8List`…); as demais —
/// inclusive as visões (`_Uint8ArrayView`) e `_Map` — ficam com o nome
/// declarado. Fora dos ids predefinidos, a VM só troca o `_Future` do
/// `dart:async` por `Future` (o mesmo `Class::GenerateUserVisibleName`
/// compara o nome com `Symbols::_Future()` e a biblioteca com
/// `Library::AsyncLibrary()`); `_SyncCompleter`, `_ControllerStream`… ficam
/// com o nome declarado.
pub fn visible_name(ctx: &Context, c: ClassId) -> String {
    let class = &ctx.program.classes[c.0 as usize];
    let name_str = ctx.symbol_name(class.name);
    let lib = ctx.program.library(class.library).uri.as_str();
    let visible = match (lib, name_str) {
        ("dart:core", "_Smi" | "_Mint") => Some("int"),
        ("dart:core", "_Double") => Some("double"),
        ("dart:core", "_OneByteString" | "_TwoByteString" | "_ExternalOneByteString" | "_ExternalTwoByteString") => {
            Some("String")
        }
        ("dart:core", "_List" | "_ImmutableList" | "_GrowableList") => Some("List"),
        ("dart:async", "_Future") => Some("Future"),
        ("dart:typed_data", n) => match n {
            "_Int8List" | "_ExternalInt8Array" => Some("Int8List"),
            "_Uint8List" | "_ExternalUint8Array" => Some("Uint8List"),
            "_Uint8ClampedList" | "_ExternalUint8ClampedArray" => Some("Uint8ClampedList"),
            "_Int16List" | "_ExternalInt16Array" => Some("Int16List"),
            "_Uint16List" | "_ExternalUint16Array" => Some("Uint16List"),
            "_Int32List" | "_ExternalInt32Array" => Some("Int32List"),
            "_Uint32List" | "_ExternalUint32Array" => Some("Uint32List"),
            "_Int64List" | "_ExternalInt64Array" => Some("Int64List"),
            "_Uint64List" | "_ExternalUint64Array" => Some("Uint64List"),
            "_Float32List" | "_ExternalFloat32Array" => Some("Float32List"),
            "_Float64List" | "_ExternalFloat64Array" => Some("Float64List"),
            "_Float32x4List" | "_ExternalFloat32x4Array" => Some("Float32x4List"),
            "_Int32x4List" | "_ExternalInt32x4Array" => Some("Int32x4List"),
            "_Float64x2List" | "_ExternalFloat64x2Array" => Some("Float64x2List"),
            "_Float32x4" => Some("Float32x4"),
            "_Int32x4" => Some("Int32x4"),
            "_Float64x2" => Some("Float64x2"),
            _ => None,
        },
        _ => None,
    };
    visible.unwrap_or(name_str).to_string()
}

/// Onde uma conferência de tipo acontece: muda o fim da mensagem da VM.
#[derive(Debug, Clone)]
pub enum CastContext {
    /// `v as T`: " in type cast".
    How,
    /// Atribuição implícita de um `dynamic`: sem sufixo.
    Implicit,
    /// O parâmetro de uma chamada dinâmica ou covariante: " of 'nome'".
    Parameter(String),
}
