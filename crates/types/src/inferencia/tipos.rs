//! Operações de tipo usadas pela inferência: subtipagem, UP/DOWN, fechos de
//! esquema, `flatten`, construção de tipos do núcleo e resolução de anotações
//! escritas dentro de corpos.

use super::corpo::Corpo;
use super::BodyInferrer;
use crate::bounds;
use crate::ops::{non_nullable, nullable, substitute};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{Type, TypeId, TypeParamId, TypeParamOwner, Variance};
use dartforge_elements::model::{ClassId, ClassKind, Element, UnitId};
use dartforge_frontend::ast;
use std::collections::HashMap;

impl<'a> BodyInferrer<'a> {
    pub(crate) fn env(&mut self) -> SubtypeEnv<'_> {
        SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core)
    }

    pub(crate) fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        if a == b {
            return true;
        }
        let mut env = self.env();
        is_subtype(a, b, &mut env)
    }

    pub(crate) fn up(&mut self, a: TypeId, b: TypeId) -> TypeId {
        let mut env = self.env();
        bounds::up(a, b, &mut env)
    }

    pub(crate) fn down(&mut self, a: TypeId, b: TypeId) -> TypeId {
        let mut env = self.env();
        bounds::down(a, b, &mut env)
    }

    pub(crate) fn anulavel(&mut self, t: TypeId) -> TypeId {
        nullable(t, self.table)
    }

    pub(crate) fn nao_nulo(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t) {
            Type::FutureOr { arg, nullable: true } => {
                let a = *arg;
                self.table.intern(Type::FutureOr { arg: a, nullable: false })
            }
            _ => non_nullable(t, self.table),
        }
    }

    /// `Null <: t`.
    pub(crate) fn e_anulavel(&mut self, t: TypeId) -> bool {
        let n = self.core.null;
        self.sub(n, t)
    }

    /// `t <: Object`.
    pub(crate) fn e_nao_anulavel(&mut self, t: TypeId) -> bool {
        let o = self.core.object;
        self.sub(t, o)
    }

    pub(crate) fn e_dynamic(&self, t: TypeId) -> bool {
        matches!(self.table.get(t), Type::Dynamic)
    }

    /// **BOTTOM**(`t`).
    pub(crate) fn e_fundo(&mut self, t: TypeId) -> bool {
        let env = self.env();
        bounds::is_bottom(t, &env)
    }

    pub(crate) fn e_desconhecido(&self, t: TypeId) -> bool {
        self.core.is_unknown(self.table, t)
    }


    /// Fecho maior de um esquema em relação a `_`.
    pub(crate) fn fecho_maior(&mut self, t: TypeId) -> TypeId {
        if self.e_desconhecido(t) {
            return self.core.object_nullable;
        }
        let mut env = self.env();
        bounds::schema_greatest(t, &mut env)
    }


    pub(crate) fn iface(&mut self, class: Option<ClassId>, args: Vec<TypeId>) -> TypeId {
        match class {
            Some(c) => self.table.intern(Type::Interface { class: c, args: args.into_boxed_slice(), nullable: false }),
            None => self.core.dynamic_,
        }
    }

    pub(crate) fn futuro(&mut self, t: TypeId) -> TypeId {
        let c = self.core.future_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn futuro_ou(&mut self, t: TypeId) -> TypeId {
        self.table.intern(Type::FutureOr { arg: t, nullable: false })
    }

    pub(crate) fn iteravel(&mut self, t: TypeId) -> TypeId {
        let c = self.core.iterable_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn fluxo_de(&mut self, t: TypeId) -> TypeId {
        let c = self.core.stream_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn lista(&mut self, t: TypeId) -> TypeId {
        let c = self.core.list_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn conjunto(&mut self, t: TypeId) -> TypeId {
        let c = self.core.set_class;
        self.iface(c, vec![t])
    }

    pub(crate) fn tipo_mapa(&mut self, k: TypeId, v: TypeId) -> TypeId {
        let c = self.core.map_class;
        self.iface(c, vec![k, v])
    }

    /// Um tipo de interface cru (`List` sem argumentos, como o outline às
    /// vezes guarda) ganha os argumentos da instanciação para os limites.
    pub(crate) fn completar_args(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Interface { class, args, nullable } if args.is_empty() => {
                let params = self.outline.classes[class.0 as usize].type_params.clone();
                if params.is_empty() {
                    return t;
                }
                let args = self.instanciar_para_limites(&params);
                self.table.intern(Type::Interface { class, args: args.into_boxed_slice(), nullable })
            }
            _ => t,
        }
    }

    /// Argumentos de `class` em `t` visto como instância dela (`List<int>`
    /// como `Iterable` dá `[int]`), ignorando a anulabilidade de `t`.
    pub(crate) fn como_instancia_de(&mut self, t: TypeId, class: Option<ClassId>) -> Option<Vec<TypeId>> {
        let class = class?;
        let t = self.nao_nulo(t);
        let t = self.completar_args(t);
        match self.table.get(t).clone() {
            Type::Interface { class: c, args, .. } if c == class => return Some(args.to_vec()),
            Type::Interface { .. } | Type::ExtensionType { .. } => {}
            Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                let b = self.table.param(param).bound;
                if b == t {
                    return None;
                }
                return self.como_instancia_de(b, Some(class));
            }
            _ => return None,
        }
        let s = self.outline.hierarchy.supertype_of(t, class, self.table, self.core)?;
        match self.table.get(s) {
            Type::Interface { args, .. } => Some(args.to_vec()),
            _ => None,
        }
    }

    /// `flatten(T)` (especificação, "Function Expressions"; `flatten` do CFE).
    pub(crate) fn flatten(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Dynamic | Type::Void => t,
            Type::FutureOr { arg, nullable: n } => {
                if n {
                    self.anulavel(arg)
                } else {
                    arg
                }
            }
            ty if ty.is_declared_nullable() => {
                let s = self.nao_nulo(t);
                let f = self.flatten(s);
                self.anulavel(f)
            }
            Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                let b = self.table.param(param).bound;
                if b == t || self.table.param(param).bound == self.core.object_nullable {
                    return t;
                }
                // `X extends Future<S>`: flatten é S.
                match self.como_instancia_de(b, self.core.future_class) {
                    Some(a) => a[0],
                    None => t,
                }
            }
            _ => match self.como_instancia_de(t, self.core.future_class) {
                Some(a) => a[0],
                None => t,
            },
        }
    }

    /// **futureValueTypeSchema**(`S`) (`inference.md`).
    pub(crate) fn tipo_valor_futuro_esquema(&mut self, s: TypeId) -> TypeId {
        if self.e_desconhecido(s) {
            return s;
        }
        match self.table.get(s).clone() {
            Type::Void | Type::Dynamic => s,
            Type::FutureOr { arg, .. } => arg,
            Type::Interface { class, args, .. } if Some(class) == self.core.future_class && args.len() == 1 => args[0],
            _ => self.core.object_nullable,
        }
    }

    /// Atribuível: subtipo, `dynamic` (cast implícito) ou coerção por `call`.
    pub(crate) fn atribuivel(&mut self, de: TypeId, para: TypeId) -> bool {
        if self.e_dynamic(de) || self.sub(de, para) {
            return true;
        }
        if matches!(self.table.get(de), Type::Interface { .. }) {
            if let Some(call) = self.sym.call {
                if let Some(m) = self.membro_de_interface(de, call, false) {
                    if m.metodo {
                        return self.sub(m.tipo, para);
                    }
                }
            }
        }
        false
    }

    /// Tipo `this` da classe (argumentos = os próprios parâmetros).
    pub(crate) fn tipo_this_classe(&mut self, c: ClassId) -> TypeId {
        let params = self.outline.classes[c.0 as usize].type_params.clone();
        let args: Vec<TypeId> = params
            .iter()
            .map(|&p| self.table.intern(Type::TypeParameter { param: p, nullable: false }))
            .collect();
        if self.program.class(c).kind == ClassKind::ExtensionType {
            self.table.intern(Type::ExtensionType { decl: c, args: args.into_boxed_slice(), nullable: false })
        } else {
            self.table.intern(Type::Interface { class: c, args: args.into_boxed_slice(), nullable: false })
        }
    }

    /// Instanciação para os limites (`instantiate to bounds`) dos parâmetros
    /// de uma classe usada crua (`List` → `List<dynamic>`).
    pub(crate) fn instanciar_para_limites(&mut self, params: &[TypeParamId]) -> Vec<TypeId> {
        let mut args: Vec<TypeId> = Vec::with_capacity(params.len());
        for &p in params {
            let b = self.table.param(p).bound;
            args.push(if b == self.core.object_nullable { self.core.dynamic_ } else { b });
        }
        // Limites que mencionam outros parâmetros: substitui e fecha.
        let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().map(|_| self.core.dynamic_)).collect();
        for a in args.iter_mut() {
            *a = substitute(*a, &mapa, self.table);
        }
        args
    }

    /// Mapa de substituição `params → args`.
    pub(crate) fn mapa(&self, params: &[TypeParamId], args: &[TypeId]) -> HashMap<TypeParamId, TypeId> {
        params.iter().copied().zip(args.iter().copied()).collect()
    }

    pub(crate) fn subst(&mut self, t: TypeId, mapa: &HashMap<TypeParamId, TypeId>) -> TypeId {
        substitute(t, mapa, self.table)
    }

    /// Tipo de uma anotação escrita num corpo (parâmetros de tipo em escopo
    /// vêm de `cx`).
    pub(crate) fn tipo_de_anotacao(&mut self, cx: &Corpo, t: ast::TypeId) -> TypeId {
        let unit = cx.unit;
        let lib = cx.lib;
        let escopo = cx.parametros_de_tipo_visiveis();
        self.resolver_anotacao(unit, lib, t, &escopo)
    }

    pub(crate) fn resolver_anotacao(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        t: ast::TypeId,
        escopo: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> TypeId {
        let node = self.program.unit(unit).ast.ty(t);
        let anulavel = node.nullable;
        let r = match &node.kind {
            ast::TypeKind::Void => return self.core.void_,
            ast::TypeKind::Named { name, args } => {
                let binding = if name.len() == 2 {
                    self.program.lookup_prefixed(lib, name[0].sym, name[1].sym)
                } else {
                    let sym = name[0].sym;
                    if let Some(&pid) = escopo.get(&sym) {
                        let tp = self.table.intern(Type::TypeParameter { param: pid, nullable: false });
                        return if anulavel { self.anulavel(tp) } else { tp };
                    }
                    match self.interner.resolve(sym) {
                        "dynamic" if self.program.lookup(lib, sym).is_none() => return self.core.dynamic_,
                        "void" => return self.core.void_,
                        "Never" if self.program.lookup(lib, sym).and_then(|b| b.getter).is_none() => {
                            return if anulavel { self.core.null } else { self.core.never };
                        }
                        _ => {}
                    }
                    self.program.lookup(lib, sym)
                };
                let args: Vec<ast::TypeId> = args.to_vec();
                let resolvidos: Vec<TypeId> = args.iter().map(|&a| self.resolver_anotacao(unit, lib, a, escopo)).collect();
                match binding.and_then(|b| b.getter) {
                    Some(Element::Class(cid)) => self.tipo_de_classe_com_args(cid, resolvidos),
                    Some(Element::Typedef(tid)) => {
                        let data = self.outline.typedefs[tid.0 as usize].clone();
                        let args = if resolvidos.len() == data.type_params.len() {
                            resolvidos
                        } else {
                            self.instanciar_para_limites(&data.type_params)
                        };
                        let mapa = self.mapa(&data.type_params, &args);
                        self.subst(data.target_type, &mapa)
                    }
                    _ => {
                        let nome = self.interner.resolve(name[name.len() - 1].sym);
                        match nome {
                            "dynamic" => return self.core.dynamic_,
                            "Never" => return if anulavel { self.core.null } else { self.core.never },
                            "Null" => return self.core.null,
                            _ => self.core.dynamic_,
                        }
                    }
                }
            }
            ast::TypeKind::Function { return_type, type_params, parameters } => {
                let mut local = escopo.clone();
                let mut tps = Vec::new();
                for tp in type_params.iter() {
                    let pid = self.table.alloc_type_param(
                        tp.name.sym,
                        TypeParamOwner::GenericFunctionType,
                        self.core.object_nullable,
                        Variance::Unspecified,
                    );
                    local.insert(tp.name.sym, pid);
                    tps.push(pid);
                }
                for (tp, &pid) in type_params.iter().zip(tps.iter()) {
                    if let Some(b) = tp.bound {
                        let bt = self.resolver_anotacao(unit, lib, b, &local);
                        self.table.set_type_param_bound(pid, bt);
                    }
                }
                let ret = match return_type {
                    Some(r) => self.resolver_anotacao(unit, lib, *r, &local),
                    None => self.core.dynamic_,
                };
                let (pos, opt, named) = self.tipos_de_parametros(unit, lib, parameters, &local);
                self.table.intern(Type::Function {
                    type_params: tps.into_boxed_slice(),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                })
            }
            ast::TypeKind::Record { positional, named } => {
                let positional: Vec<ast::TypeId> = positional.to_vec();
                let named: Vec<(dartforge_intern::SymbolId, ast::TypeId)> = named.iter().map(|(n, t)| (n.sym, *t)).collect();
                let pos: Vec<TypeId> = positional.iter().map(|&t| self.resolver_anotacao(unit, lib, t, escopo)).collect();
                let nm: Vec<_> = named.iter().map(|(n, t)| (*n, self.resolver_anotacao(unit, lib, *t, escopo))).collect();
                self.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
            }
        };
        if anulavel {
            self.anulavel(r)
        } else {
            r
        }
    }

    /// `C<args>` (com instanciação para os limites se faltarem argumentos).
    pub(crate) fn tipo_de_classe_com_args(&mut self, cid: ClassId, args: Vec<TypeId>) -> TypeId {
        let params = self.outline.classes[cid.0 as usize].type_params.clone();
        let args = if args.len() == params.len() { args } else { self.instanciar_para_limites(&params) };
        let class = self.program.class(cid);
        if class.kind == ClassKind::ExtensionType {
            return self.table.intern(Type::ExtensionType { decl: cid, args: args.into_boxed_slice(), nullable: false });
        }
        if Some(class.library) == self.core.async_library && self.interner.resolve(class.name) == "FutureOr" {
            let a = args.first().copied().unwrap_or(self.core.dynamic_);
            return self.table.intern(Type::FutureOr { arg: a, nullable: false });
        }
        self.table.intern(Type::Interface { class: cid, args: args.into_boxed_slice(), nullable: false })
    }

    /// Tipos (posicionais, opcionais, nomeados) de uma lista de parâmetros escrita.
    pub(crate) fn tipos_de_parametros(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        parameters: &[ast::Parameter],
        escopo: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> (Vec<TypeId>, Vec<TypeId>, Vec<(dartforge_intern::SymbolId, TypeId, bool)>) {
        let mut pos = Vec::new();
        let mut opt = Vec::new();
        let mut named = Vec::new();
        for p in parameters.iter() {
            let t = self.tipo_de_parametro_escrito(unit, lib, p, escopo).unwrap_or(self.core.dynamic_);
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, t, p.required));
                    }
                }
            }
        }
        (pos, opt, named)
    }

    /// Tipo escrito de um parâmetro (inclusive a forma antiga `int f(int x)`).
    pub(crate) fn tipo_de_parametro_escrito(
        &mut self,
        unit: UnitId,
        lib: dartforge_elements::model::LibraryId,
        p: &ast::Parameter,
        escopo: &HashMap<dartforge_intern::SymbolId, TypeParamId>,
    ) -> Option<TypeId> {
        if let Some(fps) = &p.function_parameters {
            let mut local = escopo.clone();
            let mut tps = Vec::new();
            for tp in p.function_type_params.iter() {
                let pid = self.table.alloc_type_param(
                    tp.name.sym,
                    TypeParamOwner::GenericFunctionType,
                    self.core.object_nullable,
                    Variance::Unspecified,
                );
                local.insert(tp.name.sym, pid);
                tps.push(pid);
            }
            let ret = match p.ty {
                Some(r) => self.resolver_anotacao(unit, lib, r, &local),
                None => self.core.dynamic_,
            };
            let (pos, opt, named) = self.tipos_de_parametros(unit, lib, fps, &local);
            let f = self.table.intern(Type::Function {
                type_params: tps.into_boxed_slice(),
                ret,
                positional: pos.into_boxed_slice(),
                optional: opt.into_boxed_slice(),
                named: named.into_boxed_slice(),
                nullable: false,
            });
            // `int f(int x)?` não existe; o `?` do parâmetro-função fica no nó.
            return Some(f);
        }
        p.ty.map(|t| self.resolver_anotacao(unit, lib, t, escopo))
    }
}
