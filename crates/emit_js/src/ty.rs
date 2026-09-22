//! Tipos do emissor: representação própria, independente da `TypeTable`, para
//! poder construir tipos livremente durante a emissão (a tabela é imutável aqui).

use dartforge_elements::model::ClassId;
use std::collections::HashMap;

/// Parâmetro de tipo: `id` único (o da `TypeTable` para os do outline; ids
/// altos para os que o emissor cria em closures genéricas).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TyParam {
    pub id: u32,
    pub name: String,
    pub bound: Box<Ty>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Dynamic,
    Void,
    Never,
    Null,
    Iface {
        class: ClassId,
        args: Vec<Ty>,
        nullable: bool,
    },
    Fn {
        type_params: Vec<TyParam>,
        ret: Box<Ty>,
        pos: Vec<Ty>,
        opt: Vec<Ty>,
        /// `(nome, tipo, required)`, ordenados por nome.
        named: Vec<(String, Ty, bool)>,
        nullable: bool,
    },
    Record {
        pos: Vec<Ty>,
        /// Ordenados por nome.
        named: Vec<(String, Ty)>,
        nullable: bool,
    },
    Param {
        id: u32,
        name: String,
        nullable: bool,
    },
    FutureOr {
        arg: Box<Ty>,
        nullable: bool,
    },
}

impl Ty {
    pub fn iface(class: ClassId) -> Ty {
        Ty::Iface { class, args: vec![], nullable: false }
    }
    pub fn iface_args(class: ClassId, args: Vec<Ty>) -> Ty {
        Ty::Iface { class, args, nullable: false }
    }
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Ty::Dynamic)
    }
    pub fn is_nullable(&self) -> bool {
        match self {
            Ty::Dynamic | Ty::Void | Ty::Null => true,
            Ty::Never => false,
            Ty::Iface { nullable, .. }
            | Ty::Fn { nullable, .. }
            | Ty::Record { nullable, .. }
            | Ty::Param { nullable, .. }
            | Ty::FutureOr { nullable, .. } => *nullable,
        }
    }
    pub fn with_nullable(&self, n: bool) -> Ty {
        let mut t = self.clone();
        match &mut t {
            Ty::Iface { nullable, .. }
            | Ty::Fn { nullable, .. }
            | Ty::Record { nullable, .. }
            | Ty::Param { nullable, .. }
            | Ty::FutureOr { nullable, .. } => *nullable = n,
            Ty::Never if n => return Ty::Null,
            _ => {}
        }
        t
    }
    pub fn non_null(&self) -> Ty {
        match self {
            Ty::Null => Ty::Never,
            _ => self.with_nullable(false),
        }
    }
    pub fn class(&self) -> Option<ClassId> {
        match self {
            Ty::Iface { class, .. } => Some(*class),
            _ => None,
        }
    }
    pub fn args(&self) -> &[Ty] {
        match self {
            Ty::Iface { args, .. } => args,
            _ => &[],
        }
    }
    pub fn is_class(&self, c: Option<ClassId>) -> bool {
        match (self, c) {
            (Ty::Iface { class, .. }, Some(c)) => *class == c,
            _ => false,
        }
    }
    /// Substitui parâmetros de tipo pelo mapa.
    pub fn subst(&self, map: &HashMap<u32, Ty>) -> Ty {
        if map.is_empty() {
            return self.clone();
        }
        match self {
            Ty::Dynamic | Ty::Void | Ty::Never | Ty::Null => self.clone(),
            Ty::Iface { class, args, nullable } => Ty::Iface {
                class: *class,
                args: args.iter().map(|a| a.subst(map)).collect(),
                nullable: *nullable,
            },
            Ty::Fn { type_params, ret, pos, opt, named, nullable } => Ty::Fn {
                type_params: type_params
                    .iter()
                    .map(|p| TyParam { id: p.id, name: p.name.clone(), bound: Box::new(p.bound.subst(map)) })
                    .collect(),
                ret: Box::new(ret.subst(map)),
                pos: pos.iter().map(|a| a.subst(map)).collect(),
                opt: opt.iter().map(|a| a.subst(map)).collect(),
                named: named.iter().map(|(n, t, r)| (n.clone(), t.subst(map), *r)).collect(),
                nullable: *nullable,
            },
            Ty::Record { pos, named, nullable } => Ty::Record {
                pos: pos.iter().map(|a| a.subst(map)).collect(),
                named: named.iter().map(|(n, t)| (n.clone(), t.subst(map))).collect(),
                nullable: *nullable,
            },
            Ty::Param { id, nullable, .. } => match map.get(id) {
                Some(t) => {
                    if *nullable { t.with_nullable(true) } else { t.clone() }
                }
                None => self.clone(),
            },
            Ty::FutureOr { arg, nullable } => Ty::FutureOr { arg: Box::new(arg.subst(map)), nullable: *nullable },
        }
    }
    /// Coleta ids de parâmetros de tipo usados.
    pub fn collect_params(&self, out: &mut Vec<u32>) {
        match self {
            Ty::Dynamic | Ty::Void | Ty::Never | Ty::Null => {}
            Ty::Iface { args, .. } => args.iter().for_each(|a| a.collect_params(out)),
            Ty::Fn { type_params, ret, pos, opt, named, .. } => {
                ret.collect_params(out);
                pos.iter().for_each(|a| a.collect_params(out));
                opt.iter().for_each(|a| a.collect_params(out));
                named.iter().for_each(|(_, t, _)| t.collect_params(out));
                type_params.iter().for_each(|p| p.bound.collect_params(out));
                // Os próprios parâmetros do tipo de função são ligados; removidos.
                out.retain(|id| !type_params.iter().any(|p| p.id == *id));
            }
            Ty::Record { pos, named, .. } => {
                pos.iter().for_each(|a| a.collect_params(out));
                named.iter().for_each(|(_, t)| t.collect_params(out));
            }
            Ty::Param { id, .. } => {
                if !out.contains(id) {
                    out.push(*id)
                }
            }
            Ty::FutureOr { arg, .. } => arg.collect_params(out),
        }
    }
    pub fn mentions_params(&self) -> bool {
        let mut v = Vec::new();
        self.collect_params(&mut v);
        !v.is_empty()
    }

    /// Verdadeiro se **nenhum** `Ty::Param` aparece na árvore, inclusive os
    /// ligados por um tipo de função. Não aloca (ao contrário de
    /// [`Ty::mentions_params`]) e é conservador: serve para decidir se o
    /// resultado de uma conversão depende do ambiente de tipos em execução.
    pub fn sem_parametros(&self) -> bool {
        match self {
            Ty::Dynamic | Ty::Void | Ty::Never | Ty::Null => true,
            Ty::Param { .. } => false,
            Ty::Iface { args, .. } => args.iter().all(|a| a.sem_parametros()),
            Ty::Fn { type_params, ret, pos, opt, named, .. } => {
                type_params.is_empty()
                    && ret.sem_parametros()
                    && pos.iter().all(|a| a.sem_parametros())
                    && opt.iter().all(|a| a.sem_parametros())
                    && named.iter().all(|(_, t, _)| t.sem_parametros())
            }
            Ty::Record { pos, named, .. } => {
                pos.iter().all(|a| a.sem_parametros()) && named.iter().all(|(_, t)| t.sem_parametros())
            }
            Ty::FutureOr { arg, .. } => arg.sem_parametros(),
        }
    }
}
