//! `conflicting_generic_interfaces`: `ErrorVerifier._checkForConflictingGenerics`
//! (`analyzer/lib/src/generated/error_verifier.dart:2652-2678`) with the errors
//! of `ClassHierarchy` (`analyzer/lib/src/dart/element/class_hierarchy.dart:41-213`):
//! the `InterfacesMerger` receives the supertype, the `on` constraints, the
//! interfaces and the mixins, each with the interfaces of its element
//! substituted; two types of the same element that `NNBD_TOP_MERGE`
//! (`top_merge.dart`) does not merge are the error (the first per element).
//!
//! Written without compiling or running (2026-10-05).

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

struct Context<'p, 't> {
    program: &'p Program,
    core: &'p CoreTypes,
    outline: &'p OutlineTypes,
    table: &'t mut TypeTable,
    enum_: Option<ClassId>,
    /// `_getHierarchy`: the interfaces and the errors of each element.
    memo: HashMap<ClassId, Rc<(Vec<TypeId>, Vec<(TypeId, TypeId)>)>>,
}

/// `_ClassInterfaceType`.
struct Collector {
    object: bool,
    simple: TypeId,
    current: Option<TypeId>,
    error: Option<(TypeId, TypeId)>,
}

impl Context<'_, '_> {
    fn nullable(&self, t: TypeId) -> bool {
        self.table.get(t).is_declared_nullable()
    }

    /// `TopMergeHelper.topMerge`; `None` where the original throws.
    fn top_merge(&mut self, t: TypeId, s: TypeId, _prof: u32) -> Option<TypeId> {
        ops::top_merge(self.table, self.core, t, s)
    }

    /// `_ClassInterfaceType.update`.
    fn update(&mut self, col: &mut Collector, t: TypeId) {
        if col.error.is_some() {
            return;
        }
        if col.current.is_none() {
            if self.table.canonico(t) == self.table.canonico(col.simple) {
                return;
            }
            col.current = Some(ops::normalize(col.simple, self.table, self.core));
        }
        let current = col.current.expect("current");
        let nt = ops::normalize(t, self.table, self.core);
        // `_merge`: `Object` and `Object?` give the non-nullable one.
        let r = if col.object && self.nullable(current) != self.nullable(nt) {
            Some(if self.nullable(current) { nt } else { current })
        } else {
            self.top_merge(current, nt, 0)
        };
        match r {
            Some(x) => col.current = Some(x),
            None => col.error = Some((current, t)),
        }
    }

    /// The element of an interface type (class, mixin, enum or extension type).
    fn element(&self, t: TypeId) -> Option<(ClassId, Box<[TypeId]>)> {
        match self.table.get(t) {
            Type::Interface { class, args, .. } => Some((*class, args.clone())),
            Type::ExtensionType { decl, args, .. } => Some((*decl, args.clone())),
            _ => None,
        }
    }

    /// `_isInterfaceTypeInterface`.
    fn valid_interface(&self, t: TypeId) -> bool {
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
    fn extension_superinterface(&self, t: TypeId) -> bool {
        match self.table.get(t) {
            Type::Interface { class, nullable: false, .. } => {
                Some(*class) != self.core.function_class && Some(*class) != self.core.null_class && Some(*class) != self.core.record_class
            }
            Type::ExtensionType { nullable: false, .. } => true,
            _ => false,
        }
    }

    /// The direct supertypes in `_getHierarchy` order: supertype, `on`
    /// constraints, interfaces, mixins.
    fn direct(&mut self, c: ClassId) -> Vec<TypeId> {
        let ce = self.program.class(c);
        let data = self.outline.classes[c.0 as usize].clone();
        let mut v = Vec::new();
        match ce.kind {
            ClassKind::Class | ClassKind::MixinApplication => {
                if Some(c) != self.core.object_class {
                    match data.supertype {
                        Some(t) if self.valid_interface(t) && matches!(self.element(t).map(|(k, _)| self.program.class(k).kind), Some(ClassKind::Class | ClassKind::MixinApplication)) => v.push(t),
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
                let on: Vec<TypeId> = data.on.iter().copied().filter(|&t| self.valid_interface(t)).collect();
                if on.is_empty() {
                    v.push(self.core.object);
                } else {
                    v.extend(on);
                }
            }
            ClassKind::ExtensionType => {}
        }
        if ce.kind == ClassKind::ExtensionType {
            v.extend(data.interfaces.iter().copied().filter(|&t| self.extension_superinterface(t)));
        } else {
            v.extend(data.interfaces.iter().copied().filter(|&t| self.valid_interface(t)));
            v.extend(data.mixins.iter().copied().filter(|&t| self.valid_interface(t)));
        }
        v
    }

    /// `_getHierarchy(element)`.
    fn hierarchy(&mut self, c: ClassId) -> Rc<(Vec<TypeId>, Vec<(TypeId, TypeId)>)> {
        if let Some(h) = self.memo.get(&c) {
            return h.clone();
        }
        self.memo.insert(c, Rc::new((Vec::new(), Vec::new())));
        let mut order: Vec<ClassId> = Vec::new();
        let mut collectors: HashMap<ClassId, Collector> = HashMap::new();
        let add = |cx: &mut Self, t: TypeId, order: &mut Vec<ClassId>, collectors: &mut HashMap<ClassId, Collector>| {
            let Some((el, _)) = cx.element(t) else { return };
            match collectors.get_mut(&el) {
                Some(col) => cx.update(col, t),
                None => {
                    order.push(el);
                    collectors.insert(el, Collector { object: Some(el) == cx.core.object_class, simple: t, current: None, error: None });
                }
            }
        };
        for t in self.direct(c) {
            add(self, t, &mut order, &mut collectors);
            let Some((el, args)) = self.element(t) else { continue };
            let params = self.outline.classes[el.0 as usize].type_params.clone();
            let map: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            let h = self.hierarchy(el);
            for &raw in h.0.iter() {
                let new_ = if map.is_empty() { raw } else { ops::substitute(raw, &map, self.table) };
                add(self, new_, &mut order, &mut collectors);
            }
        }
        let mut interfaces = Vec::new();
        let mut errors = Vec::new();
        for el in order {
            let col = &collectors[&el];
            if let Some(e) = col.error {
                errors.push(e);
            }
            interfaces.push(col.current.unwrap_or(col.simple));
        }
        let r = Rc::new((interfaces, errors));
        self.memo.insert(c, r.clone());
        r
    }
}

/// `conflicting_generic_interfaces` in the declarations of `lib`. `open`:
/// the gate of `_checkClassInheritance`/`_checkMixinInheritance` (for
/// extension types there is no gate).
pub fn conflitos_genericos(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
    open: &dyn Fn(ClassId) -> bool,
) -> Vec<(UnitId, Diagnostic)> {
    let enum_ = program
        .classes
        .iter()
        .position(|ce| ce.decl.is_some() && program.library(ce.library).uri == "dart:core" && interner.resolve(ce.name) == "Enum")
        .map(|i| ClassId(i as u32));
    let mut cx = Context { program, core, outline, table, enum_, memo: HashMap::new() };
    let mut output = Vec::new();
    let mut seen: HashSet<ClassId> = HashSet::new();
    for (i, ce) in program.classes.iter().enumerate() {
        let id = ClassId(i as u32);
        if ce.library != lib || !seen.insert(id) {
            continue;
        }
        let Some(d) = ce.decl else { continue };
        let a = &program.unit(d.unit).ast;
        let decl = a.decl(d.decl);
        if decl.augment {
            continue;
        }
        let (name, kind, callee) = match &decl.kind {
            DeclKind::Class(x) if x.mixin_application => (x.name, "class", open(id)),
            DeclKind::Class(x) => {
                let has = x.extends.is_some() || !x.with.is_empty() || !x.implements.is_empty();
                (x.name, "class", has && open(id))
            }
            DeclKind::Enum(x) => {
                let has = !x.with.is_empty() || !x.implements.is_empty();
                (x.name, "enum", has && open(id))
            }
            DeclKind::Mixin(x) => (x.name, "mixin", (!x.on.is_empty() || !x.implements.is_empty()) && open(id)),
            DeclKind::ExtensionType(x) => (x.name, "extension type", true),
            _ => continue,
        };
        if !callee {
            continue;
        }
        let h = cx.hierarchy(id);
        let text = interner.resolve(name.sym).to_string();
        for &(first, second) in h.1.iter() {
            let a1 = cx.table.format_sem_alias(first, interner, program);
            let a2 = cx.table.format_sem_alias(second, interner, program);
            output.push((d.unit, Diagnostic::com_codigo(c::CONFLICTING_GENERIC_INTERFACES, name.span, [kind, text.as_str(), a1.as_str(), a2.as_str()])));
        }
    }
    output
}
