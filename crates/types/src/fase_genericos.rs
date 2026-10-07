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
use dartforge_frontend::ast::DeclKind;
use dartforge_intern::Interner;
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
    fn anulavel(&self, t: TypeId) -> bool {
        self.table.get(t).is_declared_nullable()
    }

    /// `TopMergeHelper.topMerge`; `None` onde o original lança.
    fn top_merge(&mut self, t: TypeId, s: TypeId, _prof: u32) -> Option<TypeId> {
        ops::top_merge(self.table, self.core, t, s)
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
        let (nome, especie, chamado) = match &decl.kind {
            DeclKind::Class(x) if x.mixin_application => (x.name, "class", aberta(id)),
            DeclKind::Class(x) => {
                let tem = x.extends.is_some() || !x.with.is_empty() || !x.implements.is_empty();
                (x.name, "class", tem && aberta(id))
            }
            DeclKind::Enum(x) => {
                let tem = !x.with.is_empty() || !x.implements.is_empty();
                (x.name, "enum", tem && aberta(id))
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
