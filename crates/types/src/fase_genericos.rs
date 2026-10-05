//! `conflicting_generic_interfaces`: `ErrorVerifier._checkForConflictingGenerics`
//! (`analyzer/lib/src/generated/error_verifier.dart:2652-2678`) com os erros
//! de `ClassHierarchy` (`analyzer/lib/src/dart/element/class_hierarchy.dart:41-213`):
//! o `InterfacesMerger` recebe o supertipo, as restrições `on`, as
//! interfaces e os mixins, cada um com as interfaces do seu elemento
//! substituídas; dois tipos do mesmo elemento que o `NNBD_TOP_MERGE`
//! (`top_merge.dart`) não junta são o erro (o primeiro por elemento).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::ops;
use crate::resolve::OutlineTypes;
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{ClassId, ClassKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

struct Contexto<'p, 't> {
    program: &'p Program,
    core: &'p CoreTypes,
    outline: &'p OutlineTypes,
    table: &'t mut TypeTable,
    enum_: Option<ClassId>,
    /// `_getHierarchy`: as interfaces e os erros de cada elemento.
    memo: HashMap<ClassId, Rc<(Vec<TypeId>, Vec<(TypeId, TypeId)>)>>,
}

/// `_ClassInterfaceType`.
struct Coletor {
    objeto: bool,
    simples: TypeId,
    atual: Option<TypeId>,
    erro: Option<(TypeId, TypeId)>,
}

impl Contexto<'_, '_> {
    fn e_objeto_anulavel(&self, t: TypeId) -> bool {
        matches!(self.table.get(t), Type::Interface { class, nullable: true, .. } if Some(*class) == self.core.object_class)
    }

    fn anulavel(&self, t: TypeId) -> bool {
        self.table.get(t).is_declared_nullable()
    }

    fn sem_interrogacao(&mut self, t: TypeId, nullable: bool) -> TypeId {
        let novo = match self.table.get(t).clone() {
            Type::Interface { class, args, .. } => Type::Interface { class, args, nullable },
            Type::Function { type_params, ret, positional, optional, named, .. } => Type::Function { type_params, ret, positional, optional, named, nullable },
            Type::Record { positional, named, .. } => Type::Record { positional, named, nullable },
            Type::TypeParameter { param, .. } => Type::TypeParameter { param, nullable },
            Type::FutureOr { arg, .. } => Type::FutureOr { arg, nullable },
            Type::ExtensionType { decl, args, .. } => Type::ExtensionType { decl, args, nullable },
            _ => return t,
        };
        self.table.intern(novo)
    }

    /// `TopMergeHelper.topMerge`; `None` onde o original lança.
    fn top_merge(&mut self, t: TypeId, s: TypeId, prof: u32) -> Option<TypeId> {
        if prof > 32 {
            return None;
        }
        let (tt, st) = (self.table.get(t).clone(), self.table.get(s).clone());
        let (t_oq, s_oq) = (self.e_objeto_anulavel(t), self.e_objeto_anulavel(s));
        if t_oq && s_oq {
            return Some(t);
        }
        let (t_inv, s_inv) = (self.table.e_invalido(t), self.table.e_invalido(s));
        let t_dyn = matches!(tt, Type::Dynamic) && !t_inv;
        let s_dyn = matches!(st, Type::Dynamic) && !s_inv;
        if t_dyn && s_dyn {
            return Some(self.core.dynamic_);
        }
        if t_inv || s_inv {
            return Some(if t_inv { t } else { s });
        }
        if matches!(tt, Type::Never) && matches!(st, Type::Never) {
            return Some(self.core.never);
        }
        let (t_void, s_void) = (matches!(tt, Type::Void), matches!(st, Type::Void));
        if t_void && s_void {
            return Some(self.core.void_);
        }
        if (t_oq && s_void) || (t_void && s_oq) || (t_dyn && s_void) || (t_void && s_dyn) {
            return Some(self.core.object_nullable);
        }
        if t_oq && s_dyn {
            return Some(t);
        }
        if t_dyn && s_oq {
            return Some(s);
        }
        let (tq, sq) = (self.anulavel(t), self.anulavel(s));
        if tq && sq {
            let tn = self.sem_interrogacao(t, false);
            let sn = self.sem_interrogacao(s, false);
            let r = self.top_merge(tn, sn, prof + 1)?;
            return Some(self.sem_interrogacao(r, true));
        } else if tq || sq {
            return None;
        }
        match (tt, st) {
            (Type::Interface { class: a, args: xa, .. }, Type::Interface { class: b, args: xb, .. }) => {
                if a != b {
                    return None;
                }
                if xa.is_empty() {
                    return Some(t);
                }
                let mut args = Vec::with_capacity(xa.len());
                for (x, y) in xa.iter().zip(xb.iter()) {
                    args.push(self.top_merge(*x, *y, prof + 1)?);
                }
                Some(self.table.intern(Type::Interface { class: a, args: args.into_boxed_slice(), nullable: false }))
            }
            (Type::ExtensionType { decl: a, args: xa, .. }, Type::ExtensionType { decl: b, args: xb, .. }) => {
                if a != b {
                    return None;
                }
                if xa.is_empty() {
                    return Some(t);
                }
                let mut args = Vec::with_capacity(xa.len());
                for (x, y) in xa.iter().zip(xb.iter()) {
                    args.push(self.top_merge(*x, *y, prof + 1)?);
                }
                Some(self.table.intern(Type::ExtensionType { decl: a, args: args.into_boxed_slice(), nullable: false }))
            }
            (Type::FutureOr { arg: a, .. }, Type::FutureOr { arg: b, .. }) => {
                let r = self.top_merge(a, b, prof + 1)?;
                Some(self.table.intern(Type::FutureOr { arg: r, nullable: false }))
            }
            (Type::Function { type_params: tp_a, ret: ra, positional: pa, optional: oa, named: na, .. }, Type::Function { type_params: tp_b, ret: rb, positional: pb, optional: ob, named: nb, .. }) => {
                if tp_a.len() != tp_b.len() || pa.len() != pb.len() || oa.len() != ob.len() || na.len() != nb.len() {
                    return None;
                }
                // Os parâmetros de tipo de `s` renomeados para os de `t`
                // (os limites têm de existir nos dois ou em nenhum e se
                // juntar).
                let mut mapa: HashMap<TypeParamId, TypeId> = HashMap::new();
                for (x, y) in tp_b.iter().zip(tp_a.iter()) {
                    let ty = self.table.intern(Type::TypeParameter { param: *y, nullable: false });
                    mapa.insert(*x, ty);
                }
                for (x, y) in tp_a.iter().zip(tp_b.iter()) {
                    let (dx, dy) = (self.table.param(*x).clone(), self.table.param(*y).clone());
                    if dx.explicito != dy.explicito {
                        return None;
                    }
                    if dx.explicito {
                        let by = ops::substitute(dy.bound, &mapa, self.table);
                        self.top_merge(dx.bound, by, prof + 1)?;
                    }
                }
                let sub = |cx: &mut Self, x: TypeId| ops::substitute(x, &mapa, cx.table);
                let rb = sub(self, rb);
                let ret = self.top_merge(ra, rb, prof + 1)?;
                let mut pos = Vec::new();
                for (x, y) in pa.iter().zip(pb.iter()) {
                    let y = sub(self, *y);
                    pos.push(self.top_merge(*x, y, prof + 1)?);
                }
                let mut opc = Vec::new();
                for (x, y) in oa.iter().zip(ob.iter()) {
                    let y = sub(self, *y);
                    opc.push(self.top_merge(*x, y, prof + 1)?);
                }
                let mut nomeados: Vec<(SymbolId, TypeId, bool)> = Vec::new();
                for ((n1, x, r1), (n2, y, r2)) in na.iter().zip(nb.iter()) {
                    if n1 != n2 {
                        return None;
                    }
                    let y = sub(self, *y);
                    nomeados.push((*n1, self.top_merge(*x, y, prof + 1)?, *r1 || *r2));
                }
                Some(self.table.intern(Type::Function {
                    type_params: tp_a,
                    ret,
                    positional: pos.into_boxed_slice(),
                    optional: opc.into_boxed_slice(),
                    named: nomeados.into_boxed_slice(),
                    nullable: false,
                }))
            }
            (Type::Record { positional: pa, named: na, .. }, Type::Record { positional: pb, named: nb, .. }) => {
                if pa.len() != pb.len() || na.len() != nb.len() {
                    return None;
                }
                let mut pos = Vec::new();
                for (x, y) in pa.iter().zip(pb.iter()) {
                    pos.push(self.top_merge(*x, *y, prof + 1)?);
                }
                let mut nomeados = Vec::new();
                for ((n1, x), (n2, y)) in na.iter().zip(nb.iter()) {
                    if n1 != n2 {
                        return None;
                    }
                    nomeados.push((*n1, self.top_merge(*x, *y, prof + 1)?));
                }
                Some(self.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nomeados.into_boxed_slice(), nullable: false }))
            }
            (Type::TypeParameter { param: a, .. }, Type::TypeParameter { param: b, .. }) if a == b => Some(t),
            (Type::Null, Type::Null) => Some(t),
            _ => None,
        }
    }

    /// `_ClassInterfaceType.update`.
    fn atualizar(&mut self, col: &mut Coletor, t: TypeId) {
        if col.erro.is_some() {
            return;
        }
        if col.atual.is_none() {
            if self.table.canonico(t) == self.table.canonico(col.simples) {
                return;
            }
            col.atual = Some(ops::normalize(col.simples, self.table, self.core));
        }
        let atual = col.atual.expect("atual");
        let nt = ops::normalize(t, self.table, self.core);
        // `_merge`: `Object` e `Object?` dão o não anulável.
        let r = if col.objeto && self.anulavel(atual) != self.anulavel(nt) {
            Some(if self.anulavel(atual) { nt } else { atual })
        } else {
            self.top_merge(atual, nt, 0)
        };
        match r {
            Some(x) => col.atual = Some(x),
            None => col.erro = Some((atual, t)),
        }
    }

    /// O elemento de um tipo de interface (classe, mixin, enum ou tipo de
    /// extensão).
    fn elemento(&self, t: TypeId) -> Option<(ClassId, Box<[TypeId]>)> {
        match self.table.get(t) {
            Type::Interface { class, args, .. } => Some((*class, args.clone())),
            Type::ExtensionType { decl, args, .. } => Some((*decl, args.clone())),
            _ => None,
        }
    }

    /// `_isInterfaceTypeInterface`.
    fn interface_valida(&self, t: TypeId) -> bool {
        match self.table.get(t) {
            Type::Interface { class, nullable: false, .. } => {
                !matches!(self.program.class(*class).kind, ClassKind::Enum | ClassKind::ExtensionType)
                    && Some(*class) != self.core.function_class
                    && Some(*class) != self.core.null_class
            }
            _ => false,
        }
    }

    /// `isValidExtensionTypeSuperinterface`.
    fn superinterface_de_extensao(&self, t: TypeId) -> bool {
        match self.table.get(t) {
            Type::Interface { class, nullable: false, .. } => {
                Some(*class) != self.core.function_class && Some(*class) != self.core.null_class && Some(*class) != self.core.record_class
            }
            Type::ExtensionType { nullable: false, .. } => true,
            _ => false,
        }
    }

    /// Os supertipos diretos na ordem do `_getHierarchy`: supertipo,
    /// restrições `on`, interfaces, mixins.
    fn diretos(&mut self, c: ClassId) -> Vec<TypeId> {
        let ce = self.program.class(c);
        let dados = self.outline.classes[c.0 as usize].clone();
        let mut v = Vec::new();
        match ce.kind {
            ClassKind::Class | ClassKind::MixinApplication => {
                if Some(c) != self.core.object_class {
                    match dados.supertype {
                        Some(t) if self.interface_valida(t) && matches!(self.elemento(t).map(|(k, _)| self.program.class(k).kind), Some(ClassKind::Class | ClassKind::MixinApplication)) => v.push(t),
                        _ => v.push(self.core.object),
                    }
                }
            }
            ClassKind::Enum => {
                if let Some(e) = self.enum_ {
                    v.push(self.table.intern(Type::Interface { class: e, args: Box::new([]), nullable: false }));
                }
            }
            ClassKind::Mixin => {
                let on: Vec<TypeId> = dados.on.iter().copied().filter(|&t| self.interface_valida(t)).collect();
                if on.is_empty() {
                    v.push(self.core.object);
                } else {
                    v.extend(on);
                }
            }
            ClassKind::ExtensionType => {}
        }
        if ce.kind == ClassKind::ExtensionType {
            v.extend(dados.interfaces.iter().copied().filter(|&t| self.superinterface_de_extensao(t)));
        } else {
            v.extend(dados.interfaces.iter().copied().filter(|&t| self.interface_valida(t)));
            v.extend(dados.mixins.iter().copied().filter(|&t| self.interface_valida(t)));
        }
        v
    }

    /// `_getHierarchy(element)`.
    fn hierarquia(&mut self, c: ClassId) -> Rc<(Vec<TypeId>, Vec<(TypeId, TypeId)>)> {
        if let Some(h) = self.memo.get(&c) {
            return h.clone();
        }
        self.memo.insert(c, Rc::new((Vec::new(), Vec::new())));
        let mut ordem: Vec<ClassId> = Vec::new();
        let mut coletores: HashMap<ClassId, Coletor> = HashMap::new();
        let adicionar = |cx: &mut Self, t: TypeId, ordem: &mut Vec<ClassId>, coletores: &mut HashMap<ClassId, Coletor>| {
            let Some((el, _)) = cx.elemento(t) else { return };
            match coletores.get_mut(&el) {
                Some(col) => cx.atualizar(col, t),
                None => {
                    ordem.push(el);
                    coletores.insert(el, Coletor { objeto: Some(el) == cx.core.object_class, simples: t, atual: None, erro: None });
                }
            }
        };
        for t in self.diretos(c) {
            adicionar(self, t, &mut ordem, &mut coletores);
            let Some((el, args)) = self.elemento(t) else { continue };
            let params = self.outline.classes[el.0 as usize].type_params.clone();
            let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            let h = self.hierarquia(el);
            for &cru in h.0.iter() {
                let novo = if mapa.is_empty() { cru } else { ops::substitute(cru, &mapa, self.table) };
                adicionar(self, novo, &mut ordem, &mut coletores);
            }
        }
        let mut interfaces = Vec::new();
        let mut erros = Vec::new();
        for el in ordem {
            let col = &coletores[&el];
            if let Some(e) = col.erro {
                erros.push(e);
            }
            interfaces.push(col.atual.unwrap_or(col.simples));
        }
        let r = Rc::new((interfaces, erros));
        self.memo.insert(c, r.clone());
        r
    }
}

/// O mixin escrito sem argumentos de tipo cujos parâmetros a inferência do
/// link tiraria das restrições `on` (não portada): a classe fica de fora.
fn depende_de_inferencia_de_mixin(program: &Program, a: &ast::Ast, with: &[ast::TypeId], mixins: &[TypeId], table: &TypeTable) -> bool {
    with.iter().zip(mixins.iter()).any(|(&w, &t)| {
        let sem_args = matches!(&a.ty(w).kind, ast::TypeKind::Named { args, .. } if args.is_empty());
        let Type::Interface { class, .. } = table.get(t) else { return false };
        let m = program.class(*class);
        sem_args && m.kind == ClassKind::Mixin && !m.type_params.is_empty() && !m.on.is_empty()
    })
}

/// `conflicting_generic_interfaces` nas declarações de `lib`. `aberta`: a
/// porta de `_checkClassInheritance`/`_checkMixinInheritance` (para tipos de
/// extensão não há porta).
pub fn conflitos_genericos(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
    aberta: &dyn Fn(ClassId) -> bool,
) -> Vec<(UnitId, Diagnostic)> {
    let enum_ = program
        .classes
        .iter()
        .position(|ce| ce.decl.is_some() && program.library(ce.library).uri == "dart:core" && interner.resolve(ce.name) == "Enum")
        .map(|i| ClassId(i as u32));
    let mut cx = Contexto { program, core, outline, table, enum_, memo: HashMap::new() };
    let mut saida = Vec::new();
    let mut vistos: HashSet<ClassId> = HashSet::new();
    for (i, ce) in program.classes.iter().enumerate() {
        let id = ClassId(i as u32);
        if ce.library != lib || !vistos.insert(id) {
            continue;
        }
        let Some(d) = ce.decl else { continue };
        let a = &program.unit(d.unit).ast;
        let decl = a.decl(d.decl);
        if decl.augment {
            continue;
        }
        let dados = outline.classes[i].clone();
        let (nome, especie, chamado) = match &decl.kind {
            DeclKind::Class(x) if x.mixin_application => (x.name, "class", aberta(id) && !depende_de_inferencia_de_mixin(program, a, &x.with, &dados.mixins, cx.table)),
            DeclKind::Class(x) => {
                let tem = x.extends.is_some() || !x.with.is_empty() || !x.implements.is_empty();
                (x.name, "class", tem && aberta(id) && !depende_de_inferencia_de_mixin(program, a, &x.with, &dados.mixins, cx.table))
            }
            DeclKind::Enum(x) => {
                let tem = !x.with.is_empty() || !x.implements.is_empty();
                (x.name, "enum", tem && aberta(id) && !depende_de_inferencia_de_mixin(program, a, &x.with, &dados.mixins, cx.table))
            }
            DeclKind::Mixin(x) => (x.name, "mixin", (!x.on.is_empty() || !x.implements.is_empty()) && aberta(id)),
            DeclKind::ExtensionType(x) => (x.name, "extension type", true),
            _ => continue,
        };
        if !chamado {
            continue;
        }
        let h = cx.hierarquia(id);
        let texto = interner.resolve(nome.sym).to_string();
        for &(primeiro, segundo) in h.1.iter() {
            let a1 = cx.table.format_sem_alias(primeiro, interner, program);
            let a2 = cx.table.format_sem_alias(segundo, interner, program);
            saida.push((d.unit, Diagnostic::com_codigo(c::CONFLICTING_GENERIC_INTERFACES, nome.span, [especie, texto.as_str(), a1.as_str(), a2.as_str()])));
        }
    }
    saida
}
