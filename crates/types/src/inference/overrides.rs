//! A inferência de sobrescrita do analyzer 3.6.2 (`InstanceMemberInferrer`,
//! `analyzer/lib/src/task/strong_mode.dart`), que o link roda depois de
//! criar os nós de inicializador e antes de inferi-los
//! (`summary2/top_level_inference.dart:100-107`): o tipo omitido de um membro
//! de instância vem da assinatura combinada (`combineSignatures` com o
//! `topMerge`) dos membros que ele sobrescreve nas superinterfaces diretas
//! (`getOverridden2`).
//!
//! * método (e operador): só quando todos os sobrescritos são métodos
//!   (`_allSameElementKind`); o tipo combinado com os parâmetros de tipo do
//!   próprio método (`_toOverriddenFunctionType`, nulo se a contagem
//!   difere); retorno omitido (fora `[]=`) e cada parâmetro omitido pelo
//!   correspondente (o nomeado pelo nome, o posicional pela posição), senão
//!   `dynamic`;
//! * getter e setter explícitos com tipo omitido: pelos getters e setters
//!   sobrescritos (`_inferAccessorOrField`);
//! * campo sem tipo: em [`super::funcoes`], na inferência sob demanda do
//!   tipo da variável (a mesma regra, antes do inicializador).
//!
//! As classes vão com os supertipos antes (`_inferClass`); um ciclo na
//! hierarquia encerra a classe e as que dependem dela. A interface de cada
//! classe é esquecida depois dos membros dela, para que as subclasses vejam
//! os tipos novos.
//! Escrito sem compilar nem executar (2026-10-05).

use super::BodyInferrer;
use crate::heranca::{Especie, Membro, Nome};
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, FunctionRef};
use dartforge_frontend::ast;
use std::collections::{HashMap, HashSet};

/// Um ciclo na hierarquia (`_CycleException`).
struct Ciclo;

impl<'a> BodyInferrer<'a> {
    /// `TopLevelInference._performOverrideInference`, uma vez por outline.
    pub(crate) fn infer_overrides(&mut self) {
        if self.outline.inferred_overrides {
            return;
        }
        self.outline.inferred_overrides = true;
        let mut done: HashSet<ClassId> = HashSet::new();
        let mut in_progress: HashSet<ClassId> = HashSet::new();
        // `inferCompilationUnit`: classes (com os aliases), enums e mixins de
        // cada unidade, na ordem.
        let mut classes: Vec<ClassId> = (0..self.program.classes.len() as u32)
            .map(ClassId)
            .filter(|&c| {
                let k = self.program.class(c);
                k.decl.is_some() && matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication | ClassKind::Enum | ClassKind::Mixin)
            })
            .collect();
        classes.sort_by_key(|&c| {
            let k = self.program.class(c);
            let order = match k.kind {
                ClassKind::Class | ClassKind::MixinApplication => 0,
                ClassKind::Enum => 1,
                _ => 2,
            };
            let d = k.decl.expect("filtrado");
            (d.unit.0, order, d.decl.0)
        });
        for c in classes {
            let _ = self.infer_class(c, &mut done, &mut in_progress);
        }
        // As interfaces montadas durante a passada têm tipos de antes dela.
        self.heranca = crate::heranca::Heranca::default();
    }

    /// `_inferClass`.
    fn infer_class(&mut self, c: ClassId, done: &mut HashSet<ClassId>, in_progress: &mut HashSet<ClassId>) -> Result<(), Ciclo> {
        if done.contains(&c) {
            return Ok(());
        }
        if !in_progress.insert(c) {
            return Err(Ciclo);
        }
        let r = (|| {
            let k = self.program.class(c);
            let mut supers: Vec<ClassId> = Vec::new();
            if let Some(s) = k.supertype_class {
                supers.push(s);
            }
            supers.extend(k.mixin_classes.iter().copied());
            supers.extend(k.interface_classes.iter().copied());
            for s in supers {
                if self.program.class(s).decl.is_some() {
                    self.infer_class(s, done, in_progress)?;
                }
            }
            // Os acessores explícitos, depois os métodos.
            let mut members: Vec<FunctionElementId> = self.program.class(c).instance_members.values().copied().collect();
            members.sort();
            for &f in &members {
                if matches!(self.program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter) {
                    self.infer_accessor(c, f);
                }
            }
            for &f in &members {
                if matches!(self.program.function(f).kind, FunctionKind::Function | FunctionKind::Operator) {
                    self.infer_method(c, f);
                }
            }
            Ok(())
        })();
        in_progress.remove(&c);
        if r.is_ok() {
            done.insert(c);
        }
        // A interface da classe muda com os tipos novos.
        self.heranca.forget(self.program, c);
        r
    }

    /// O nó da função na árvore. Um membro `augment` não é nó de
    /// inferência: os tipos omitidos dele são os da declaração que ele
    /// aumenta (o fragmento do analyzer), já postos pelo esboço.
    fn function_node(&self, f: FunctionElementId) -> Option<&'a ast::Function> {
        match self.program.function(f).node {
            FunctionRef::Function { unit, function } => {
                let a = &self.program.unit(unit).ast;
                if a.members.iter().any(|m| m.augment && matches!(m.kind, ast::MemberKind::Method(g) if g == function)) {
                    return None;
                }
                Some(a.function(function))
            }
            _ => None,
        }
    }

    /// `getOverridden2` pelo `Name` da biblioteca do membro.
    fn member_overridden(&mut self, c: ClassId, f: FunctionElementId, setter: bool) -> Option<Vec<Membro>> {
        let e = self.program.function(f);
        let lib = e.library;
        let text = self.interner.resolve(e.name);
        let base = text.strip_suffix("_=").unwrap_or(text).to_string();
        let key = if setter { self.interner.lookup(&format!("{base}_="))? } else { self.interner.lookup(&base)? };
        let name = Nome::novo(self.interner, lib, key);
        let mut h = std::mem::take(&mut self.heranca);
        let r = h.sobrescritos(self, c, name);
        self.heranca = h;
        r
    }

    /// `combineSignatures(..., doTopMerge: true)`.
    fn combined(&mut self, c: ClassId, candidates: &[Membro], name: Nome) -> Option<Membro> {
        if candidates.is_empty() {
            return None;
        }
        crate::heranca::combinar(self, c, candidates, true, name, None)
    }

    /// `combinedGetterType`.
    pub(crate) fn combined_getter_type(&mut self, c: ClassId, getters: &[Membro], name: Nome) -> TypeId {
        match self.combined(c, getters, name) {
            Some(m) => match self.table.get(m.tipo) {
                Type::Function { ret, .. } => *ret,
                _ => self.core.dynamic_,
            },
            None => self.core.dynamic_,
        }
    }

    /// `combinedSetterType`.
    pub(crate) fn combined_setter_type(&mut self, c: ClassId, setters: &[Membro], name: Nome) -> TypeId {
        match self.combined(c, setters, name) {
            Some(m) => match self.table.get(m.tipo) {
                Type::Function { positional, .. } => positional.first().copied().unwrap_or(self.core.dynamic_),
                _ => self.core.dynamic_,
            },
            None => self.core.dynamic_,
        }
    }

    /// Os sobrescritos de getter e de setter de um nome (`_inferAccessorOrField`).
    pub(crate) fn overridden_getters_and_setters(&mut self, c: ClassId, lib: dartforge_elements::model::LibraryId, name: dartforge_intern::SymbolId) -> (Vec<Membro>, Vec<Membro>, Nome, Option<Nome>) {
        let n_getter = Nome::novo(self.interner, lib, name);
        let n_setter = self.setter_key(name).map(|k| Nome::novo(self.interner, lib, k));
        let mut h = std::mem::take(&mut self.heranca);
        let getters: Vec<Membro> = h.sobrescritos(self, c, n_getter).unwrap_or_default().into_iter().filter(|m| m.especie == Especie::Getter).collect();
        let setters: Vec<Membro> = match n_setter {
            Some(n) => h.sobrescritos(self, c, n).unwrap_or_default(),
            None => Vec::new(),
        };
        self.heranca = h;
        (getters, setters, n_getter, n_setter)
    }

    /// `_inferAccessorOrField` de um getter ou setter explícito.
    fn infer_accessor(&mut self, c: ClassId, f: FunctionElementId) {
        let e = self.program.function(f);
        if e.static_ {
            return;
        }
        let Some(no) = self.function_node(f) else { return };
        // Só um tipo omitido pede a busca.
        let omitted = match e.kind {
            FunctionKind::Getter => no.return_type.is_none(),
            FunctionKind::Setter => no.parameters.as_ref().and_then(|ps| ps.first()).is_some_and(implicit_parameter),
            _ => false,
        };
        if !omitted {
            return;
        }
        let text = self.interner.resolve(e.name);
        let base = text.strip_suffix("_=").unwrap_or(text);
        let Some(name) = self.interner.lookup(base) else { return };
        let (getters, setters, n_getter, n_setter) = self.overridden_getters_and_setters(c, e.library, name);
        match e.kind {
            FunctionKind::Getter => {
                if no.return_type.is_some() {
                    return;
                }
                let t = if !getters.is_empty() {
                    self.combined_getter_type(c, &getters, n_getter)
                } else if let (false, Some(ns)) = (setters.is_empty(), n_setter) {
                    self.combined_setter_type(c, &setters, ns)
                } else {
                    self.core.dynamic_
                };
                self.define_return(f, t);
            }
            FunctionKind::Setter => {
                let Some(p) = no.parameters.as_ref().and_then(|ps| ps.first()) else { return };
                if !implicit_parameter(p) {
                    return;
                }
                let t = if !getters.is_empty() && setters.is_empty() {
                    self.combined_getter_type(c, &getters, n_getter)
                } else if let (false, Some(ns)) = (setters.is_empty(), n_setter) {
                    self.combined_setter_type(c, &setters, ns)
                } else {
                    self.core.dynamic_
                };
                self.define_parameter(f, 0, t);
            }
            _ => {}
        }
    }

    /// `_inferExecutable` de um método ou operador.
    fn infer_method(&mut self, c: ClassId, f: FunctionElementId) {
        let e = self.program.function(f);
        if e.static_ {
            return;
        }
        let Some(no) = self.function_node(f) else { return };
        let parameters: &'a [ast::Parameter] = no.parameters.as_deref().unwrap_or(&[]);
        let implicit_return = no.return_type.is_none();
        let some_implicit = implicit_return || parameters.iter().any(implicit_parameter);
        // Nada omitido: nada a inferir (a covariância herdada é calculada
        // pela herança).
        if !some_implicit {
            return;
        }
        // Sem sobrescrito, ou com um que não é método, o omitido fica como o
        // `TypesBuilder` o deixou: `dynamic` (o retorno de `[]=`, `void`).
        let overridden = match self.member_overridden(c, f, false) {
            Some(s) if s.iter().all(|m| m.especie == Especie::Metodo) => s,
            _ => {
                self.omitted_default(f, parameters, implicit_return);
                return;
            }
        };
        let mut combined: Option<TypeId> = None;
        if some_implicit {
            let name = Nome::novo(self.interner, e.library, e.name);
            if let Some(m) = self.combined(c, &overridden, name) {
                combined = self.for_overridden_type(f, m.tipo);
            }
        }
        let text = self.interner.resolve(e.name);
        if implicit_return && text == "[]=" {
            self.define_return(f, self.core.void_);
        }
        if implicit_return && text != "[]=" {
            let t = match combined.map(|t| self.table.get(t).clone()) {
                Some(Type::Function { ret, .. }) => ret,
                _ => self.core.dynamic_,
            };
            self.define_return(f, t);
        }
        // Os parâmetros omitidos pelo correspondente do tipo combinado.
        let mut position = 0usize;
        for (i, p) in parameters.iter().enumerate() {
            let positional_one = p.kind != ast::ParameterKind::Named;
            if implicit_parameter(p) {
                let t = match combined.map(|t| self.table.get(t).clone()) {
                    Some(Type::Function { positional, optional, named, .. }) => {
                        if positional_one {
                            positional.iter().chain(optional.iter()).nth(position).copied().unwrap_or(self.core.dynamic_)
                        } else {
                            let n = p.nome_externo().map(|n| n.sym);
                            named.iter().find(|(s, _, _)| Some(*s) == n).map_or(self.core.dynamic_, |x| x.1)
                        }
                    }
                    _ => self.core.dynamic_,
                };
                self.define_parameter(f, i, t);
            }
            if positional_one {
                position += 1;
            }
        }
    }

    /// `_toOverriddenFunctionType`: o tipo combinado com os parâmetros de
    /// tipo do próprio método; nulo se a contagem difere.
    fn for_overridden_type(&mut self, f: FunctionElementId, t: TypeId) -> Option<TypeId> {
        let own: Vec<TypeParamId> = self.outline.functions.get(f.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
        let Type::Function { type_params, .. } = self.table.get(t).clone() else { return None };
        if own.len() != type_params.len() {
            return None;
        }
        if own.is_empty() {
            return Some(t);
        }
        let mut s: HashMap<TypeParamId, TypeId> = HashMap::new();
        for (o, p) in type_params.iter().zip(own.iter()) {
            let tp = self.table.intern(Type::TypeParameter { param: *p, nullable: false });
            s.insert(*o, tp);
        }
        // O corpo do tipo com os parâmetros próprios; a lista de parâmetros
        // de tipo passa a ser a do método. A substituição vai no tipo **sem**
        // os formais: no genérico, eles sombreiam o mapeamento e nada muda
        // (`R bar<R>(R x)` herdado por `bar<Q>(x)` deixava `x: R`).
        let without_formals = match self.table.get(t).clone() {
            Type::Function { ret, positional, optional, named, nullable, .. } => {
                self.table.intern(Type::Function { type_params: Box::new([]), ret, positional, optional, named, nullable })
            }
            _ => return None,
        };
        let body = crate::ops::substitute(without_formals, &s, self.table);
        match self.table.get(body).clone() {
            Type::Function { ret, positional, optional, named, nullable, .. } => Some(self.table.intern(Type::Function {
                type_params: own.into_boxed_slice(),
                ret,
                positional,
                optional,
                named,
                nullable,
            })),
            _ => None,
        }
    }

    /// Os omitidos como o `TypesBuilder` os deixa: `dynamic`, e o retorno de
    /// `[]=` `void`.
    fn omitted_default(&mut self, f: FunctionElementId, parameters: &[ast::Parameter], implicit_return: bool) {
        if implicit_return {
            let t = if self.interner.resolve(self.program.function(f).name) == "[]=" { self.core.void_ } else { self.core.dynamic_ };
            self.define_return(f, t);
        }
        for (i, p) in parameters.iter().enumerate() {
            if implicit_parameter(p) {
                self.define_parameter(f, i, self.core.dynamic_);
            }
        }
    }

    fn define_return(&mut self, f: FunctionElementId, t: TypeId) {
        if let Some(d) = self.outline.functions.get_mut(f.0 as usize) {
            d.return_type = t;
        }
        self.redo_signature(f);
    }

    fn define_parameter(&mut self, f: FunctionElementId, i: usize, t: TypeId) {
        if let Some(p) = self.outline.functions.get_mut(f.0 as usize).and_then(|d| d.parameters.get_mut(i)) {
            p.ty = t;
        }
        self.redo_signature(f);
    }

    /// A assinatura pelo retorno e pelos parâmetros.
    fn redo_signature(&mut self, f: FunctionElementId) {
        let Some(fd) = self.outline.functions.get(f.0 as usize) else { return };
        let (mut pos, mut opt, mut the_named) = (Vec::new(), Vec::new(), Vec::new());
        for par in fd.parameters.iter() {
            match par.kind {
                ast::ParameterKind::Required => pos.push(par.ty),
                ast::ParameterKind::Optional => opt.push(par.ty),
                ast::ParameterKind::Named => {
                    if let Some(n) = par.externo {
                        the_named.push((n, par.ty, par.required));
                    }
                }
            }
        }
        let sig = self.table.intern(Type::Function {
            type_params: fd.type_params.clone(),
            ret: fd.return_type,
            positional: pos.into_boxed_slice(),
            optional: opt.into_boxed_slice(),
            named: the_named.into_boxed_slice(),
            nullable: false,
        });
        self.outline.functions[f.0 as usize].signature = sig;
    }
}

/// `parameter.hasImplicitType`: sem tipo escrito, nem forma de função, nem
/// `this.x`/`super.x`.
fn implicit_parameter(p: &ast::Parameter) -> bool {
    p.ty.is_none() && p.function_parameters.is_none() && !p.this_ && !p.super_
}
