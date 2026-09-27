//! `TYPE_ARGUMENT_NOT_MATCHING_BOUNDS` dos tipos escritos: o
//! `TypeArgumentsVerifier._checkForTypeArgumentNotMatchingBounds` do analyzer
//! 7.x (`src/error/type_arguments_verifier.dart`).
//!
//! Para cada `C<A1, …, An>` escrito (classe ou alias de tipo), cada
//! argumento precisa ser subtipo do limite do parâmetro com os argumentos
//! substituídos (*regular-bounded*). Se não for, e o lugar aceita tipo
//! *super-bounded* (não é `extends`/`with`/`implements`/`on`, criação de
//! instância, corpo de `typedef` nem tipo de extensão), o tipo invertido
//! (`invertido`: topo por `Never` nas posições não contravariantes, fundo
//! por `Object?` nas contravariantes) é conferido do mesmo jeito, e são os
//! argumentos e limites invertidos que o diagnóstico mostra.
//!
//! **Pelo lado seguro.** A resolução aqui é a do escopo da biblioteca, sem
//! o escopo léxico: um tipo escrito que menciona um nome que é parâmetro de
//! tipo em algum lugar da unidade, um tipo cru genérico ou um tipo de
//! função genérico não é conferido (falta diagnóstico, nunca sobra).

use crate::resolve::OutlineTypes;
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::{codigos::compile_time_error as c, Diagnostic, Span};
use dartforge_elements::model::{Element, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, ParameterKind, TypeKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// Variância de uma posição (`Variance` do analyzer).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Variancia {
    Nenhuma,
    Co,
    Contra,
    In,
}

impl Variancia {
    /// `Variance.combine`: o sinal da posição dentro de outra.
    fn combinar(self, outra: Variancia) -> Variancia {
        use Variancia::*;
        match (self, outra) {
            (Nenhuma, _) | (_, Nenhuma) => Nenhuma,
            (In, _) | (_, In) => In,
            (a, b) if a == b => Co,
            _ => Contra,
        }
    }

    /// `Variance.meet`: duas ocorrências do mesmo parâmetro.
    fn juntar(self, outra: Variancia) -> Variancia {
        use Variancia::*;
        match (self, outra) {
            (Nenhuma, x) | (x, Nenhuma) => x,
            (a, b) if a == b => a,
            _ => In,
        }
    }
}

struct Verificador<'a> {
    program: &'a Program,
    interner: &'a Interner,
    table: &'a mut TypeTable,
    core: &'a CoreTypes,
    outline: &'a OutlineTypes,
    unit: UnitId,
    /// Nomes de parâmetros de tipo declarados em algum lugar da unidade.
    nomes_de_parametros: HashSet<SymbolId>,
}

/// Os `type_argument_not_matching_bounds` dos tipos escritos numa unidade.
pub fn argumentos_fora_dos_limites(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    unit: UnitId,
) -> Vec<Diagnostic> {
    let a = &program.unit(unit).ast;
    let mut v = Verificador { program, interner, table, core, outline, unit, nomes_de_parametros: HashSet::new() };
    v.nomes_de_parametros = nomes_de_parametros(a, &program.unit(unit).unit);
    let sem_super = sem_super_limite(a, &program.unit(unit).unit);
    let mut out = Vec::new();
    for (i, t) in a.types.iter().enumerate() {
        if let TypeKind::Named { name, args } = &t.kind {
            if args.is_empty() {
                continue;
            }
            let id = ast::TypeId(i as u32);
            v.conferir(name, args, !sem_super.contains(&id), &mut out);
        }
    }
    out
}

impl Verificador<'_> {
    fn ast(&self) -> &ast::Ast {
        &self.program.unit(self.unit).ast
    }

    fn sub(&mut self, a: TypeId, b: TypeId) -> bool {
        let mut env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        is_subtype(a, b, &mut env)
    }

    fn e_topo(&mut self, t: TypeId) -> bool {
        let env = SubtypeEnv::new(self.table, &self.outline.hierarchy, self.core);
        crate::bounds::is_top(t, &env)
    }

    fn subst(&mut self, t: TypeId, params: &[TypeParamId], args: &[TypeId]) -> TypeId {
        let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
        crate::ops::substitute(t, &mapa, self.table)
    }

    fn formatar(&self, t: TypeId) -> String {
        self.table.format(t, self.interner, self.program)
    }

    /// O elemento que o nome escrito designa na biblioteca.
    fn elemento(&self, name: &[ast::Name]) -> Option<Element> {
        let lib = self.program.unit(self.unit).library;
        if name.iter().any(|n| self.nomes_de_parametros.contains(&n.sym)) {
            return None;
        }
        let b = match name {
            [n] => self.program.lookup(lib, n.sym),
            [p, n] => self.program.lookup_prefixed(lib, p.sym, n.sym),
            _ => None,
        }?;
        if b.ambiguous {
            return None;
        }
        b.getter
    }

    /// Parâmetros de tipo (e o alvo, se alias) do elemento genérico.
    fn parametros(&self, el: Element) -> Option<(Vec<TypeParamId>, Option<TypeId>)> {
        match el {
            Element::Class(c) => Some((self.outline.classes[c.0 as usize].type_params.to_vec(), None)),
            Element::Typedef(t) => {
                let d = &self.outline.typedefs[t.0 as usize];
                Some((d.type_params.to_vec(), Some(d.target_type)))
            }
            _ => None,
        }
    }

    fn conferir(&mut self, name: &[ast::Name], args: &[ast::TypeId], super_permitido: bool, out: &mut Vec<Diagnostic>) {
        let Some(el) = self.elemento(name) else { return };
        let Some((params, alvo)) = self.parametros(el) else { return };
        if params.len() != args.len() || params.is_empty() {
            return;
        }
        // Tipo de extensão nunca é super-bounded.
        let super_permitido = super_permitido
            && !matches!(el, Element::Class(c) if self.program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType);
        let Some(tipos) = args.iter().map(|&x| self.resolver(x)).collect::<Option<Vec<TypeId>>>() else { return };
        let mut problemas = Vec::new();
        for (i, &p) in params.iter().enumerate() {
            let limite = self.table.param(p).bound;
            let limite = self.subst(limite, &params, &tipos);
            if !self.sub(tipos[i], limite) {
                problemas.push((i, tipos[i], limite));
            }
        }
        if problemas.is_empty() {
            return;
        }
        if !super_permitido {
            for (i, arg, limite) in problemas {
                self.relatar(args[i], params[i], arg, limite, out);
            }
            return;
        }
        // Super-bounded: os argumentos invertidos, com a variância de cada
        // parâmetro no alias (numa classe, a posição de fora, covariante).
        let variancias: Vec<Variancia> = match alvo {
            Some(t) => params.iter().map(|&p| variancia_em(self.table, t, p, Variancia::Co)).collect(),
            None => vec![Variancia::Co; params.len()],
        };
        let Some(inv) = args
            .iter()
            .zip(variancias.iter())
            .map(|(&x, &va)| self.invertido(x, va))
            .collect::<Option<Vec<TypeId>>>()
        else {
            return;
        };
        for (i, &p) in params.iter().enumerate() {
            let limite = self.table.param(p).bound;
            let limite = self.subst(limite, &params, &inv);
            if !self.sub(inv[i], limite) {
                self.relatar(args[i], p, inv[i], limite, out);
            }
        }
    }

    fn relatar(&mut self, no: ast::TypeId, p: TypeParamId, arg: TypeId, limite: TypeId, out: &mut Vec<Diagnostic>) {
        let span: Span = self.ast().ty(no).span;
        let a = self.formatar(arg);
        let nome = self.interner.resolve(self.table.param(p).name).to_string();
        let l = self.formatar(limite);
        out.push(Diagnostic::com_codigo(c::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS, span, [a.as_str(), nome.as_str(), l.as_str()]));
    }

    /// O tipo escrito, resolvido no escopo da biblioteca; `None` se depender
    /// de algo que aqui não se resolve com certeza.
    fn resolver(&mut self, t: ast::TypeId) -> Option<TypeId> {
        let no = self.ast().ty(t);
        let anulavel = no.nullable;
        let r = match &no.kind {
            TypeKind::Void => self.core.void_,
            TypeKind::Named { name, args } => {
                let (name, args) = (name.clone(), args.clone());
                if let [n] = &name[..] {
                    match self.interner.resolve(n.sym) {
                        "dynamic" if args.is_empty() => return Some(self.core.dynamic_),
                        "Never" if args.is_empty() => {
                            return Some(if anulavel { self.core.null } else { self.core.never });
                        }
                        _ => {}
                    }
                }
                let el = self.elemento(&name)?;
                let tipos = args.iter().map(|&x| self.resolver(x)).collect::<Option<Vec<TypeId>>>()?;
                match el {
                    Element::Class(cid) => {
                        let cl = self.program.class(cid);
                        let nome = self.interner.resolve(cl.name);
                        if Some(cid) == self.core.null_class {
                            self.core.null
                        } else if nome == "FutureOr" && self.program.library(cl.library).is_sdk {
                            let [arg] = tipos[..] else { return None };
                            self.table.intern(Type::FutureOr { arg, nullable: false })
                        } else {
                            let params = self.outline.classes[cid.0 as usize].type_params.clone();
                            if params.len() != tipos.len() {
                                // Cru e genérico (instanciado para os limites): fora.
                                return None;
                            }
                            if cl.kind == dartforge_elements::model::ClassKind::ExtensionType {
                                self.table.intern(Type::ExtensionType { decl: cid, args: tipos.into_boxed_slice(), nullable: false })
                            } else {
                                self.table.intern(Type::Interface { class: cid, args: tipos.into_boxed_slice(), nullable: false })
                            }
                        }
                    }
                    Element::Typedef(tid) => {
                        let d = &self.outline.typedefs[tid.0 as usize];
                        let (params, alvo) = (d.type_params.to_vec(), d.target_type);
                        if params.len() != tipos.len() {
                            return None;
                        }
                        self.subst(alvo, &params, &tipos)
                    }
                    _ => return None,
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                if !type_params.is_empty() {
                    return None;
                }
                let (ret, parameters) = (*return_type, parameters.clone_params());
                let ret = match ret {
                    Some(r) => self.resolver(r)?,
                    None => self.core.dynamic_,
                };
                let mut pos = Vec::new();
                let mut opt = Vec::new();
                let mut named = Vec::new();
                for (kind, ty, nome, req) in parameters {
                    let t = match ty {
                        Some(x) => self.resolver(x)?,
                        None => self.core.dynamic_,
                    };
                    match kind {
                        ParameterKind::Required => pos.push(t),
                        ParameterKind::Optional => opt.push(t),
                        ParameterKind::Named => named.push((nome?, t, req)),
                    }
                }
                self.table.intern(Type::Function {
                    type_params: Box::new([]),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                })
            }
            TypeKind::Record { positional, named } => {
                let (positional, named) = (positional.clone(), named.clone());
                let pos = positional.iter().map(|&x| self.resolver(x)).collect::<Option<Vec<TypeId>>>()?;
                let mut nm = Vec::new();
                for (n, x) in named.iter() {
                    nm.push((n.sym, self.resolver(*x)?));
                }
                self.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
            }
        };
        Some(if anulavel { crate::ops::nullable(r, self.table) } else { r })
    }

    /// `replaceTopAndBottom` sobre o tipo escrito, numa posição de variância
    /// `va`: topo vira `Never` fora das posições contravariantes; o que é
    /// subtipo de `Never` vira `Object?` nas contravariantes. Classes levam a
    /// variância adiante; aliases a combinam com a do parâmetro no alvo;
    /// funções invertem nos parâmetros.
    fn invertido(&mut self, t: ast::TypeId, va: Variancia) -> Option<TypeId> {
        let inteiro = self.resolver(t)?;
        if va == Variancia::Contra {
            let never = self.core.never;
            if self.sub(inteiro, never) {
                return Some(self.core.object_nullable);
            }
        } else if self.e_topo(inteiro) {
            return Some(self.core.never);
        }
        let no = self.ast().ty(t);
        let anulavel = no.nullable;
        let r = match &no.kind {
            TypeKind::Named { name, args } if !args.is_empty() => {
                let (name, args) = (name.clone(), args.clone());
                let el = self.elemento(&name)?;
                match el {
                    Element::Class(cid) if Some(cid) != self.core.null_class => {
                        let cl = self.program.class(cid);
                        if self.interner.resolve(cl.name) == "FutureOr" && self.program.library(cl.library).is_sdk {
                            let [a] = args[..] else { return None };
                            let arg = self.invertido(a, va)?;
                            self.table.intern(Type::FutureOr { arg, nullable: false })
                        } else {
                            let tipos = args.iter().map(|&x| self.invertido(x, va)).collect::<Option<Vec<TypeId>>>()?;
                            match self.table.get(inteiro).clone() {
                                Type::ExtensionType { .. } => {
                                    self.table.intern(Type::ExtensionType { decl: cid, args: tipos.into_boxed_slice(), nullable: false })
                                }
                                _ => self.table.intern(Type::Interface { class: cid, args: tipos.into_boxed_slice(), nullable: false }),
                            }
                        }
                    }
                    Element::Typedef(tid) => {
                        let d = &self.outline.typedefs[tid.0 as usize];
                        let (params, alvo) = (d.type_params.to_vec(), d.target_type);
                        if params.len() != args.len() {
                            return None;
                        }
                        let mut tipos = Vec::new();
                        for (&x, &p) in args.iter().zip(params.iter()) {
                            let vp = variancia_em(self.table, alvo, p, Variancia::Co);
                            tipos.push(self.invertido(x, vp.combinar(va))?);
                        }
                        self.subst(alvo, &params, &tipos)
                    }
                    _ => return Some(inteiro),
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                if !type_params.is_empty() {
                    return Some(inteiro);
                }
                let (ret, parameters) = (*return_type, parameters.clone_params());
                let ret = match ret {
                    Some(r) => self.invertido(r, va)?,
                    None => {
                        let d = self.core.dynamic_;
                        if va == Variancia::Contra { d } else { self.core.never }
                    }
                };
                let contra = va.combinar(Variancia::Contra);
                let mut pos = Vec::new();
                let mut opt = Vec::new();
                let mut named = Vec::new();
                for (kind, ty, nome, req) in parameters {
                    let t = match ty {
                        Some(x) => self.invertido(x, contra)?,
                        None => {
                            // `dynamic` implícito numa posição contravariante:
                            // não é fundo, fica.
                            let d = self.core.dynamic_;
                            if contra == Variancia::Contra { d } else { self.core.never }
                        }
                    };
                    match kind {
                        ParameterKind::Required => pos.push(t),
                        ParameterKind::Optional => opt.push(t),
                        ParameterKind::Named => named.push((nome?, t, req)),
                    }
                }
                self.table.intern(Type::Function {
                    type_params: Box::new([]),
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opt.into_boxed_slice(),
                    named: named.into_boxed_slice(),
                    nullable: false,
                })
            }
            _ => return Some(inteiro),
        };
        Some(if anulavel { crate::ops::nullable(r, self.table) } else { r })
    }
}

/// Os parâmetros de uma lista escrita, só o que a resolução usa.
trait ParametrosSimples {
    fn clone_params(&self) -> Vec<(ParameterKind, Option<ast::TypeId>, Option<SymbolId>, bool)>;
}

impl ParametrosSimples for Box<[ast::Parameter]> {
    fn clone_params(&self) -> Vec<(ParameterKind, Option<ast::TypeId>, Option<SymbolId>, bool)> {
        self.iter().map(|p| (p.kind, p.ty, p.name.map(|n| n.sym), p.required)).collect()
    }
}

/// Variância do parâmetro `p` no tipo `t` (a posição de fora é `va`).
fn variancia_em(table: &TypeTable, t: TypeId, p: TypeParamId, va: Variancia) -> Variancia {
    match table.get(t) {
        Type::TypeParameter { param, .. } if *param == p => va,
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => {
            args.iter().fold(Variancia::Nenhuma, |acc, &a| acc.juntar(variancia_em(table, a, p, va)))
        }
        Type::FutureOr { arg, .. } => variancia_em(table, *arg, p, va),
        Type::Function { type_params, ret, positional, optional, named, .. } => {
            let contra = va.combinar(Variancia::Contra);
            let mut v = variancia_em(table, *ret, p, va);
            for &x in positional.iter().chain(optional.iter()) {
                v = v.juntar(variancia_em(table, x, p, contra));
            }
            for (_, x, _) in named.iter() {
                v = v.juntar(variancia_em(table, *x, p, contra));
            }
            // Nos limites dos parâmetros de tipo da função: invariante.
            for &tp in type_params.iter() {
                if variancia_em(table, table.param(tp).bound, p, va) != Variancia::Nenhuma {
                    v = v.juntar(Variancia::In);
                }
            }
            v
        }
        Type::Record { positional, named, .. } => {
            let mut v = Variancia::Nenhuma;
            for &x in positional.iter() {
                v = v.juntar(variancia_em(table, x, p, va));
            }
            for (_, x) in named.iter() {
                v = v.juntar(variancia_em(table, *x, p, va));
            }
            v
        }
        Type::Intersection { bound, .. } => variancia_em(table, *bound, p, va),
        _ => Variancia::Nenhuma,
    }
}

/// Todo nome de parâmetro de tipo declarado na unidade.
fn nomes_de_parametros(a: &ast::Ast, unit: &ast::CompilationUnit) -> HashSet<SymbolId> {
    let mut s = HashSet::new();
    let mut tps = |l: &[ast::TypeParameter]| {
        for tp in l {
            s.insert(tp.name.sym);
        }
    };
    for &d in &unit.declarations {
        match &a.decl(d).kind {
            DeclKind::Class(x) => tps(&x.type_params),
            DeclKind::Mixin(x) => tps(&x.type_params),
            DeclKind::Enum(x) => tps(&x.type_params),
            DeclKind::Extension(x) => tps(&x.type_params),
            DeclKind::ExtensionType(x) => tps(&x.type_params),
            DeclKind::Typedef(x) => tps(&x.type_params),
            DeclKind::Function(_) | DeclKind::Variables(_) => {}
        }
    }
    for f in &a.functions {
        tps(&f.type_params);
    }
    for t in &a.types {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
            tps(type_params);
            for p in parameters.iter() {
                tps(&p.function_type_params);
            }
        }
    }
    for f in &a.functions {
        for p in f.parameters.iter().flatten() {
            tps(&p.function_type_params);
        }
    }
    s
}

/// Tipos escritos onde o analyzer não aceita *super-bounded*: `extends`,
/// `with`, `implements`, `on` de mixin, o corpo de `typedef X = T` e o tipo
/// da criação de instância.
fn sem_super_limite(a: &ast::Ast, unit: &ast::CompilationUnit) -> HashSet<ast::TypeId> {
    let mut s = HashSet::new();
    for &d in &unit.declarations {
        match &a.decl(d).kind {
            DeclKind::Class(x) => {
                s.extend(x.extends);
                s.extend(x.with.iter().copied());
                s.extend(x.implements.iter().copied());
            }
            DeclKind::Mixin(x) => {
                s.extend(x.on.iter().copied());
                s.extend(x.implements.iter().copied());
            }
            DeclKind::Enum(x) => {
                s.extend(x.with.iter().copied());
                s.extend(x.implements.iter().copied());
            }
            DeclKind::ExtensionType(x) => s.extend(x.implements.iter().copied()),
            DeclKind::Typedef(x) => {
                if let ast::TypedefKind::Alias(t) = &x.kind {
                    s.insert(*t);
                }
            }
            DeclKind::Extension(_) | DeclKind::Function(_) | DeclKind::Variables(_) => {}
        }
    }
    for e in &a.exprs {
        if let ExprKind::InstanceCreation { ty, .. } = &e.kind {
            s.insert(*ty);
        }
    }
    s
}
