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
    pub(crate) fn inferir_sobrescritas(&mut self) {
        if self.outline.sobrescritas_inferidas {
            return;
        }
        self.outline.sobrescritas_inferidas = true;
        let mut feitas: HashSet<ClassId> = HashSet::new();
        let mut em_curso: HashSet<ClassId> = HashSet::new();
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
            let ordem = match k.kind {
                ClassKind::Class | ClassKind::MixinApplication => 0,
                ClassKind::Enum => 1,
                _ => 2,
            };
            let d = k.decl.expect("filtrado");
            (d.unit.0, ordem, d.decl.0)
        });
        for c in classes {
            let _ = self.inferir_classe(c, &mut feitas, &mut em_curso);
        }
        // As interfaces montadas durante a passada têm tipos de antes dela.
        self.heranca = crate::heranca::Heranca::default();
    }

    /// `_inferClass`.
    fn inferir_classe(&mut self, c: ClassId, feitas: &mut HashSet<ClassId>, em_curso: &mut HashSet<ClassId>) -> Result<(), Ciclo> {
        if feitas.contains(&c) {
            return Ok(());
        }
        if !em_curso.insert(c) {
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
                    self.inferir_classe(s, feitas, em_curso)?;
                }
            }
            // Os acessores explícitos, depois os métodos.
            let mut membros: Vec<FunctionElementId> = self.program.class(c).instance_members.values().copied().collect();
            membros.sort();
            for &f in &membros {
                if matches!(self.program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter) {
                    self.inferir_acessor(c, f);
                }
            }
            for &f in &membros {
                if matches!(self.program.function(f).kind, FunctionKind::Function | FunctionKind::Operator) {
                    self.inferir_metodo(c, f);
                }
            }
            Ok(())
        })();
        em_curso.remove(&c);
        if r.is_ok() {
            feitas.insert(c);
        }
        // A interface da classe muda com os tipos novos.
        self.heranca.esquecer(self.program, c);
        r
    }

    /// O nó da função na árvore. Um membro `augment` não é nó de
    /// inferência: os tipos omitidos dele são os da declaração que ele
    /// aumenta (o fragmento do analyzer), já postos pelo esboço.
    fn no_da_funcao(&self, f: FunctionElementId) -> Option<&'a ast::Function> {
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
    fn sobrescritos_do_membro(&mut self, c: ClassId, f: FunctionElementId, setter: bool) -> Option<Vec<Membro>> {
        let e = self.program.function(f);
        let lib = e.library;
        let texto = self.interner.resolve(e.name);
        let base = texto.strip_suffix("_=").unwrap_or(texto).to_string();
        let chave = if setter { self.interner.lookup(&format!("{base}_="))? } else { self.interner.lookup(&base)? };
        let nome = Nome::novo(self.interner, lib, chave);
        let mut h = std::mem::take(&mut self.heranca);
        let r = h.sobrescritos(self, c, nome);
        self.heranca = h;
        r
    }

    /// `combineSignatures(..., doTopMerge: true)`.
    fn combinado(&mut self, c: ClassId, candidatos: &[Membro], nome: Nome) -> Option<Membro> {
        if candidatos.is_empty() {
            return None;
        }
        crate::heranca::combinar(self, c, candidatos, true, nome, None)
    }

    /// `combinedGetterType`.
    pub(crate) fn tipo_de_getter_combinado(&mut self, c: ClassId, getters: &[Membro], nome: Nome) -> TypeId {
        match self.combinado(c, getters, nome) {
            Some(m) => match self.table.get(m.tipo) {
                Type::Function { ret, .. } => *ret,
                _ => self.core.dynamic_,
            },
            None => self.core.dynamic_,
        }
    }

    /// `combinedSetterType`.
    pub(crate) fn tipo_de_setter_combinado(&mut self, c: ClassId, setters: &[Membro], nome: Nome) -> TypeId {
        match self.combinado(c, setters, nome) {
            Some(m) => match self.table.get(m.tipo) {
                Type::Function { positional, .. } => positional.first().copied().unwrap_or(self.core.dynamic_),
                _ => self.core.dynamic_,
            },
            None => self.core.dynamic_,
        }
    }

    /// Os sobrescritos de getter e de setter de um nome (`_inferAccessorOrField`).
    pub(crate) fn getters_e_setters_sobrescritos(&mut self, c: ClassId, lib: dartforge_elements::model::LibraryId, nome: dartforge_intern::SymbolId) -> (Vec<Membro>, Vec<Membro>, Nome, Option<Nome>) {
        let n_getter = Nome::novo(self.interner, lib, nome);
        let n_setter = self.chave_setter(nome).map(|k| Nome::novo(self.interner, lib, k));
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
    fn inferir_acessor(&mut self, c: ClassId, f: FunctionElementId) {
        let e = self.program.function(f);
        if e.static_ {
            return;
        }
        let Some(no) = self.no_da_funcao(f) else { return };
        // Só um tipo omitido pede a busca.
        let omitido = match e.kind {
            FunctionKind::Getter => no.return_type.is_none(),
            FunctionKind::Setter => no.parameters.as_ref().and_then(|ps| ps.first()).is_some_and(parametro_implicito),
            _ => false,
        };
        if !omitido {
            return;
        }
        let texto = self.interner.resolve(e.name);
        let base = texto.strip_suffix("_=").unwrap_or(texto);
        let Some(nome) = self.interner.lookup(base) else { return };
        let (getters, setters, n_getter, n_setter) = self.getters_e_setters_sobrescritos(c, e.library, nome);
        match e.kind {
            FunctionKind::Getter => {
                if no.return_type.is_some() {
                    return;
                }
                let t = if !getters.is_empty() {
                    self.tipo_de_getter_combinado(c, &getters, n_getter)
                } else if let (false, Some(ns)) = (setters.is_empty(), n_setter) {
                    self.tipo_de_setter_combinado(c, &setters, ns)
                } else {
                    self.core.dynamic_
                };
                self.definir_retorno(f, t);
            }
            FunctionKind::Setter => {
                let Some(p) = no.parameters.as_ref().and_then(|ps| ps.first()) else { return };
                if !parametro_implicito(p) {
                    return;
                }
                let t = if !getters.is_empty() && setters.is_empty() {
                    self.tipo_de_getter_combinado(c, &getters, n_getter)
                } else if let (false, Some(ns)) = (setters.is_empty(), n_setter) {
                    self.tipo_de_setter_combinado(c, &setters, ns)
                } else {
                    self.core.dynamic_
                };
                self.definir_parametro(f, 0, t);
            }
            _ => {}
        }
    }

    /// `_inferExecutable` de um método ou operador.
    fn inferir_metodo(&mut self, c: ClassId, f: FunctionElementId) {
        let e = self.program.function(f);
        if e.static_ {
            return;
        }
        let Some(no) = self.no_da_funcao(f) else { return };
        let parametros: &'a [ast::Parameter] = no.parameters.as_deref().unwrap_or(&[]);
        let retorno_implicito = no.return_type.is_none();
        let algum_implicito = retorno_implicito || parametros.iter().any(parametro_implicito);
        // Nada omitido: nada a inferir (a covariância herdada é calculada
        // pela herança).
        if !algum_implicito {
            return;
        }
        // Sem sobrescrito, ou com um que não é método, o omitido fica como o
        // `TypesBuilder` o deixou: `dynamic` (o retorno de `[]=`, `void`).
        let sobrescritos = match self.sobrescritos_do_membro(c, f, false) {
            Some(s) if s.iter().all(|m| m.especie == Especie::Metodo) => s,
            _ => {
                self.padrao_dos_omitidos(f, parametros, retorno_implicito);
                return;
            }
        };
        let mut combinado: Option<TypeId> = None;
        if algum_implicito {
            let nome = Nome::novo(self.interner, e.library, e.name);
            if let Some(m) = self.combinado(c, &sobrescritos, nome) {
                combinado = self.para_o_tipo_sobrescrito(f, m.tipo);
            }
        }
        let texto = self.interner.resolve(e.name);
        if retorno_implicito && texto == "[]=" {
            self.definir_retorno(f, self.core.void_);
        }
        if retorno_implicito && texto != "[]=" {
            let t = match combinado.map(|t| self.table.get(t).clone()) {
                Some(Type::Function { ret, .. }) => ret,
                _ => self.core.dynamic_,
            };
            self.definir_retorno(f, t);
        }
        // Os parâmetros omitidos pelo correspondente do tipo combinado.
        let mut posicao = 0usize;
        for (i, p) in parametros.iter().enumerate() {
            let posicional = p.kind != ast::ParameterKind::Named;
            if parametro_implicito(p) {
                let t = match combinado.map(|t| self.table.get(t).clone()) {
                    Some(Type::Function { positional, optional, named, .. }) => {
                        if posicional {
                            positional.iter().chain(optional.iter()).nth(posicao).copied().unwrap_or(self.core.dynamic_)
                        } else {
                            let n = p.nome_externo().map(|n| n.sym);
                            named.iter().find(|(s, _, _)| Some(*s) == n).map_or(self.core.dynamic_, |x| x.1)
                        }
                    }
                    _ => self.core.dynamic_,
                };
                self.definir_parametro(f, i, t);
            }
            if posicional {
                posicao += 1;
            }
        }
    }

    /// `_toOverriddenFunctionType`: o tipo combinado com os parâmetros de
    /// tipo do próprio método; nulo se a contagem difere.
    fn para_o_tipo_sobrescrito(&mut self, f: FunctionElementId, t: TypeId) -> Option<TypeId> {
        let proprios: Vec<TypeParamId> = self.outline.functions.get(f.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
        let Type::Function { type_params, .. } = self.table.get(t).clone() else { return None };
        if proprios.len() != type_params.len() {
            return None;
        }
        if proprios.is_empty() {
            return Some(t);
        }
        let mut s: HashMap<TypeParamId, TypeId> = HashMap::new();
        for (o, p) in type_params.iter().zip(proprios.iter()) {
            let tp = self.table.intern(Type::TypeParameter { param: *p, nullable: false });
            s.insert(*o, tp);
        }
        // O corpo do tipo com os parâmetros próprios; a lista de parâmetros
        // de tipo passa a ser a do método. A substituição vai no tipo **sem**
        // os formais: no genérico, eles sombreiam o mapeamento e nada muda
        // (`R bar<R>(R x)` herdado por `bar<Q>(x)` deixava `x: R`).
        let sem_formais = match self.table.get(t).clone() {
            Type::Function { ret, positional, optional, named, nullable, .. } => {
                self.table.intern(Type::Function { type_params: Box::new([]), ret, positional, optional, named, nullable })
            }
            _ => return None,
        };
        let corpo = crate::ops::substitute(sem_formais, &s, self.table);
        match self.table.get(corpo).clone() {
            Type::Function { ret, positional, optional, named, nullable, .. } => Some(self.table.intern(Type::Function {
                type_params: proprios.into_boxed_slice(),
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
    fn padrao_dos_omitidos(&mut self, f: FunctionElementId, parametros: &[ast::Parameter], retorno_implicito: bool) {
        if retorno_implicito {
            let t = if self.interner.resolve(self.program.function(f).name) == "[]=" { self.core.void_ } else { self.core.dynamic_ };
            self.definir_retorno(f, t);
        }
        for (i, p) in parametros.iter().enumerate() {
            if parametro_implicito(p) {
                self.definir_parametro(f, i, self.core.dynamic_);
            }
        }
    }

    fn definir_retorno(&mut self, f: FunctionElementId, t: TypeId) {
        if let Some(d) = self.outline.functions.get_mut(f.0 as usize) {
            d.return_type = t;
        }
        self.refazer_assinatura(f);
    }

    fn definir_parametro(&mut self, f: FunctionElementId, i: usize, t: TypeId) {
        if let Some(p) = self.outline.functions.get_mut(f.0 as usize).and_then(|d| d.parameters.get_mut(i)) {
            p.ty = t;
        }
        self.refazer_assinatura(f);
    }

    /// A assinatura pelo retorno e pelos parâmetros.
    fn refazer_assinatura(&mut self, f: FunctionElementId) {
        let Some(fd) = self.outline.functions.get(f.0 as usize) else { return };
        let (mut pos, mut opt, mut nom) = (Vec::new(), Vec::new(), Vec::new());
        for par in fd.parameters.iter() {
            match par.kind {
                ast::ParameterKind::Required => pos.push(par.ty),
                ast::ParameterKind::Optional => opt.push(par.ty),
                ast::ParameterKind::Named => {
                    if let Some(n) = par.externo {
                        nom.push((n, par.ty, par.required));
                    }
                }
            }
        }
        let sig = self.table.intern(Type::Function {
            type_params: fd.type_params.clone(),
            ret: fd.return_type,
            positional: pos.into_boxed_slice(),
            optional: opt.into_boxed_slice(),
            named: nom.into_boxed_slice(),
            nullable: false,
        });
        self.outline.functions[f.0 as usize].signature = sig;
    }
}

/// `parameter.hasImplicitType`: sem tipo escrito, nem forma de função, nem
/// `this.x`/`super.x`.
fn parametro_implicito(p: &ast::Parameter) -> bool {
    p.ty.is_none() && p.function_parameters.is_none() && !p.this_ && !p.super_
}
