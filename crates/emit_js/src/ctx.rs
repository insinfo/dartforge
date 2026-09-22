//! Contexto global da emissão: programa, tipos, classes conhecidas, conjunto
//! de tipos nativos (`dartx`), nomes de bibliotecas e utilidades de tipos.

use crate::js;
use crate::ty::{Ty, TyParam};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, Program,
    UnitId, VariableId,
};
use dartforge_frontend::ast;
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::BodyTypes;
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// Informações de uma biblioteca para a emissão.
#[derive(Clone, Debug)]
pub struct LibInfo {
    /// Identificador JS (`main`, `lib__util`, `foo__src__a`, `core`).
    pub ident: String,
    /// Caminho do módulo (`main.js`, `packages/foo/a.js`); vazio para SDK.
    pub module_path: String,
    pub is_sdk: bool,
    /// Variável JS do namespace (`L$main` para usuário; `core` para SDK).
    pub js_var: String,
}

/// Membro encontrado numa classe.
#[derive(Clone, Debug)]
pub struct Member {
    pub class: ClassId,
    pub kind: MemberKind,
    /// Substituição dos parâmetros da classe declarante.
    pub subst: HashMap<u32, Ty>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberKind {
    Method(FunctionElementId),
    Getter(FunctionElementId),
    Setter(FunctionElementId),
    /// Campo (getter implícito); `setter` diz se pode ser atribuído.
    Field(VariableId),
}

pub struct Ctx<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub bodies: &'a BodyTypes,
    pub libs: Vec<LibInfo>,
    pub object: Option<ClassId>,
    pub int_: Option<ClassId>,
    pub double_: Option<ClassId>,
    pub num_: Option<ClassId>,
    pub bool_: Option<ClassId>,
    pub string_: Option<ClassId>,
    pub null_: Option<ClassId>,
    pub list_: Option<ClassId>,
    pub map_: Option<ClassId>,
    pub set_: Option<ClassId>,
    pub iterable_: Option<ClassId>,
    pub function_: Option<ClassId>,
    pub record_: Option<ClassId>,
    pub future_: Option<ClassId>,
    pub stream_: Option<ClassId>,
    pub enum_: Option<ClassId>,
    pub underscore_enum: Option<ClassId>,
    pub type_: Option<ClassId>,
    pub symbol_: Option<ClassId>,
    pub comparable_: Option<ClassId>,
    pub js_array: Option<ClassId>,
    pub linked_hash_map: Option<ClassId>,
    pub linked_hash_set: Option<ClassId>,
    pub jsstring: Option<ClassId>,
    pub jsnumber: Option<ClassId>,
    pub jsbool: Option<ClassId>,
    pub stack_trace: Option<ClassId>,
    pub map_entry: Option<ClassId>,
    pub invocation_: Option<ClassId>,
    pub future_or: Option<ClassId>,
    /// Classes nativas (JSArray, JSString…) e interfaces que elas implementam:
    /// membros acessados por `dartx`.
    pub ext_set: HashSet<ClassId>,
    pub native_set: HashSet<ClassId>,
    /// Supertipos diretos (tipo instanciado em função dos parâmetros da classe).
    pub class_supers: Vec<Vec<Ty>>,
    pub class_params: Vec<Vec<TyParam>>,
    /// Bounds por id de parâmetro de tipo.
    pub param_bounds: RefCell<HashMap<u32, Ty>>,
    pub param_names: RefCell<HashMap<u32, String>>,
    pub next_param_id: RefCell<u32>,
    pub empty_sym: Option<SymbolId>,
}

impl<'a> Ctx<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a TypeTable,
        core: &'a CoreTypes,
        outline: &'a OutlineTypes,
        bodies: &'a BodyTypes,
    ) -> Ctx<'a> {
        let find_lib = |uri: &str| -> Option<LibraryId> {
            program
                .libraries
                .iter()
                .position(|l| l.uri == uri)
                .map(|i| LibraryId(i as u32))
        };
        let find_class = |lib: &str, name: &str| -> Option<ClassId> {
            let lib = find_lib(lib)?;
            let sym = interner.lookup(name)?;
            match program.library(lib).declared.get(&sym)?.getter {
                Some(Element::Class(c)) => Some(c),
                _ => None,
            }
        };
        let libs = compute_libs(program);
        let mut ctx = Ctx {
            program,
            interner,
            table,
            core,
            outline,
            bodies,
            libs,
            object: find_class("dart:core", "Object"),
            int_: find_class("dart:core", "int"),
            double_: find_class("dart:core", "double"),
            num_: find_class("dart:core", "num"),
            bool_: find_class("dart:core", "bool"),
            string_: find_class("dart:core", "String"),
            null_: find_class("dart:core", "Null"),
            list_: find_class("dart:core", "List"),
            map_: find_class("dart:core", "Map"),
            set_: find_class("dart:core", "Set"),
            iterable_: find_class("dart:core", "Iterable"),
            function_: find_class("dart:core", "Function"),
            record_: find_class("dart:core", "Record"),
            future_: find_class("dart:async", "Future"),
            stream_: find_class("dart:async", "Stream"),
            enum_: find_class("dart:core", "Enum"),
            underscore_enum: find_class("dart:core", "_Enum"),
            type_: find_class("dart:core", "Type"),
            symbol_: find_class("dart:core", "Symbol"),
            comparable_: find_class("dart:core", "Comparable"),
            js_array: find_class("dart:_interceptors", "JSArray"),
            linked_hash_map: find_class("dart:collection", "LinkedHashMap"),
            linked_hash_set: find_class("dart:collection", "LinkedHashSet"),
            jsstring: find_class("dart:_interceptors", "JSString"),
            jsnumber: find_class("dart:_interceptors", "JSNumber"),
            jsbool: find_class("dart:_interceptors", "JSBool"),
            stack_trace: find_class("dart:core", "StackTrace"),
            map_entry: find_class("dart:core", "MapEntry"),
            invocation_: find_class("dart:core", "Invocation"),
            future_or: find_class("dart:async", "FutureOr"),
            ext_set: HashSet::new(),
            native_set: HashSet::new(),
            class_supers: Vec::new(),
            class_params: Vec::new(),
            param_bounds: RefCell::new(HashMap::new()),
            param_names: RefCell::new(HashMap::new()),
            next_param_id: RefCell::new(1 << 30),
            empty_sym: interner.lookup(""),
        };
        ctx.compute_hierarchy();
        ctx.compute_ext_set(find_lib);
        ctx
    }

    fn compute_hierarchy(&mut self) {
        let n = self.program.classes.len();
        let mut supers = Vec::with_capacity(n);
        let mut params = Vec::with_capacity(n);
        for i in 0..n {
            let data = &self.outline.classes[i];
            let mut ps = Vec::new();
            for &pid in data.type_params.iter() {
                ps.push(self.ty_param_of(pid));
            }
            params.push(ps);
            let mut s = Vec::new();
            if let Some(t) = data.supertype {
                s.push(self.ty_of(t));
            } else if self.program.classes[i].kind == ClassKind::Enum {
                if let Some(e) = self.underscore_enum {
                    s.push(Ty::iface(e));
                }
            }
            for &t in data.mixins.iter() {
                s.push(self.ty_of(t));
            }
            for &t in data.interfaces.iter() {
                s.push(self.ty_of(t));
            }
            for &t in data.on.iter() {
                s.push(self.ty_of(t));
            }
            supers.push(s);
        }
        self.class_supers = supers;
        self.class_params = params;
    }

    fn compute_ext_set(&mut self, find_lib: impl Fn(&str) -> Option<LibraryId>) {
        let mut natives = Vec::new();
        for lib_uri in ["dart:_interceptors", "dart:_native_typed_data"] {
            if let Some(lib) = find_lib(lib_uri) {
                for (i, c) in self.program.classes.iter().enumerate() {
                    if c.library == lib && self.has_native_annotation(ClassId(i as u32)) {
                        natives.push(ClassId(i as u32));
                    }
                }
            }
        }
        for c in [self.int_, self.double_, self.bool_, self.string_].into_iter().flatten() {
            natives.push(c);
        }
        let mut extensible: Vec<ClassId> = Vec::new();
        if let Some(f) = self.function_ {
            extensible.push(f);
        }
        for (lib, names) in [
            ("dart:core", &["Comparable", "Map"][..]),
            ("dart:collection", &["ListBase", "MapBase"][..]),
            ("dart:math", &["Rectangle"][..]),
        ] {
            if let Some(l) = find_lib(lib) {
                for n in names {
                    if let Some(sym) = self.interner.lookup(n) {
                        if let Some(Element::Class(c)) =
                            self.program.library(l).declared.get(&sym).and_then(|b| b.getter)
                        {
                            extensible.push(c);
                        }
                    }
                }
            }
        }
        let mut ext_set = HashSet::new();
        let mut native_set = HashSet::new();
        let mut queue: Vec<ClassId> = Vec::new();
        for c in natives {
            native_set.insert(c);
            ext_set.insert(c);
            queue.push(c);
        }
        for c in extensible {
            ext_set.insert(c);
            queue.push(c);
        }
        while let Some(c) = queue.pop() {
            for s in &self.class_supers[c.0 as usize] {
                if let Some(sc) = s.class() {
                    if Some(sc) == self.object {
                        continue;
                    }
                    if ext_set.insert(sc) {
                        queue.push(sc);
                    }
                }
            }
        }
        if let Some(o) = self.object {
            ext_set.insert(o);
        }
        self.ext_set = ext_set;
        self.native_set = native_set;
    }

    fn has_native_annotation(&self, c: ClassId) -> bool {
        let class = self.program.class(c);
        let Some(decl) = class.decl else { return false };
        let unit = self.program.unit(decl.unit);
        let d = unit.ast.decl(decl.decl);
        d.metadata.iter().any(|a| {
            let n = a.name.first().map(|n| self.interner.resolve(n.sym)).unwrap_or("");
            n == "Native" || n == "JsPeerInterface"
        })
    }

    // -----------------------------------------------------------------------
    // Conversão de tipos
    // -----------------------------------------------------------------------

    pub fn ty_param_of(&self, pid: TypeParamId) -> TyParam {
        let data = self.table.param(pid);
        let bound = self.ty_of(data.bound);
        let name = self.interner.resolve(data.name).to_string();
        self.param_bounds.borrow_mut().insert(pid.0, bound.clone());
        self.param_names.borrow_mut().insert(pid.0, name.clone());
        TyParam { id: pid.0, name, bound: Box::new(bound) }
    }

    pub fn ty_of(&self, id: TypeId) -> Ty {
        match self.table.get(id) {
            Type::Dynamic => Ty::Dynamic,
            Type::Void => Ty::Void,
            Type::Never => Ty::Never,
            Type::Null => Ty::Null,
            Type::Interface { class, args, nullable } => Ty::Iface {
                class: *class,
                args: args.iter().map(|a| self.ty_of(*a)).collect(),
                nullable: *nullable,
            },
            Type::Function { type_params, ret, positional, optional, named, nullable } => Ty::Fn {
                type_params: type_params.iter().map(|p| self.ty_param_of(*p)).collect(),
                ret: Box::new(self.ty_of(*ret)),
                pos: positional.iter().map(|a| self.ty_of(*a)).collect(),
                opt: optional.iter().map(|a| self.ty_of(*a)).collect(),
                named: named
                    .iter()
                    .map(|(n, t, r)| (self.interner.resolve(*n).to_string(), self.ty_of(*t), *r))
                    .collect(),
                nullable: *nullable,
            },
            Type::Record { positional, named, nullable } => Ty::Record {
                pos: positional.iter().map(|a| self.ty_of(*a)).collect(),
                named: named.iter().map(|(n, t)| (self.interner.resolve(*n).to_string(), self.ty_of(*t))).collect(),
                nullable: *nullable,
            },
            Type::TypeParameter { param, nullable } => {
                let data = self.table.param(*param);
                let name = self.interner.resolve(data.name).to_string();
                if !self.param_bounds.borrow().contains_key(&param.0) {
                    // Registra o bound (evitando recursão infinita em F-bounds).
                    self.param_bounds.borrow_mut().insert(param.0, Ty::Dynamic);
                    let b = self.ty_of(data.bound);
                    self.param_bounds.borrow_mut().insert(param.0, b);
                    self.param_names.borrow_mut().insert(param.0, name.clone());
                }
                Ty::Param { id: param.0, name, nullable: *nullable }
            }
            Type::FutureOr { arg, nullable } => Ty::FutureOr { arg: Box::new(self.ty_of(*arg)), nullable: *nullable },
            Type::ExtensionType { decl, args, nullable } => {
                // Apaga para o tipo de representação.
                let class = self.program.class(*decl);
                if let Some(rep) = class.representation {
                    let v = &self.outline.variables[rep.0 as usize];
                    if let Some(t) = v.declared_type.or(v.inferred) {
                        let mut map = HashMap::new();
                        for (p, a) in self.class_params[decl.0 as usize].iter().zip(args.iter()) {
                            map.insert(p.id, self.ty_of(*a));
                        }
                        let t = self.ty_of(t).subst(&map);
                        return if *nullable { t.with_nullable(true) } else { t };
                    }
                }
                Ty::Dynamic
            }
        }
    }

    pub fn fresh_param(&self, name: &str, bound: Ty) -> TyParam {
        let mut n = self.next_param_id.borrow_mut();
        let id = *n;
        *n += 1;
        self.param_bounds.borrow_mut().insert(id, bound.clone());
        self.param_names.borrow_mut().insert(id, name.to_string());
        TyParam { id, name: name.to_string(), bound: Box::new(bound) }
    }

    pub fn bound_of(&self, id: u32) -> Ty {
        self.param_bounds.borrow().get(&id).cloned().unwrap_or(Ty::Dynamic)
    }

    // -----------------------------------------------------------------------
    // Tipos bem conhecidos
    // -----------------------------------------------------------------------

    pub fn t_int(&self) -> Ty {
        self.int_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_double(&self) -> Ty {
        self.double_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_num(&self) -> Ty {
        self.num_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_bool(&self) -> Ty {
        self.bool_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_string(&self) -> Ty {
        self.string_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_object(&self) -> Ty {
        self.object.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_object_q(&self) -> Ty {
        self.object.map(|c| Ty::Iface { class: c, args: vec![], nullable: true }).unwrap_or(Ty::Dynamic)
    }
    pub fn t_list(&self, e: Ty) -> Ty {
        self.list_.map(|c| Ty::iface_args(c, vec![e])).unwrap_or(Ty::Dynamic)
    }
    pub fn t_map(&self, k: Ty, v: Ty) -> Ty {
        self.map_.map(|c| Ty::iface_args(c, vec![k, v])).unwrap_or(Ty::Dynamic)
    }
    pub fn t_set(&self, e: Ty) -> Ty {
        self.set_.map(|c| Ty::iface_args(c, vec![e])).unwrap_or(Ty::Dynamic)
    }
    pub fn t_iterable(&self, e: Ty) -> Ty {
        self.iterable_.map(|c| Ty::iface_args(c, vec![e])).unwrap_or(Ty::Dynamic)
    }
    pub fn t_future(&self, e: Ty) -> Ty {
        self.future_.map(|c| Ty::iface_args(c, vec![e])).unwrap_or(Ty::Dynamic)
    }
    pub fn t_stream(&self, e: Ty) -> Ty {
        self.stream_.map(|c| Ty::iface_args(c, vec![e])).unwrap_or(Ty::Dynamic)
    }
    pub fn t_type(&self) -> Ty {
        self.type_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }

    pub fn is_int(&self, t: &Ty) -> bool {
        t.is_class(self.int_) && !t.is_nullable()
    }
    pub fn is_double(&self, t: &Ty) -> bool {
        t.is_class(self.double_) && !t.is_nullable()
    }
    pub fn is_num_like(&self, t: &Ty) -> bool {
        (t.is_class(self.int_) || t.is_class(self.double_) || t.is_class(self.num_)) && !t.is_nullable()
    }
    pub fn is_num_like_nullable(&self, t: &Ty) -> bool {
        t.is_class(self.int_) || t.is_class(self.double_) || t.is_class(self.num_)
    }
    pub fn is_string(&self, t: &Ty) -> bool {
        t.is_class(self.string_) && !t.is_nullable()
    }
    pub fn is_bool(&self, t: &Ty) -> bool {
        t.is_class(self.bool_) && !t.is_nullable()
    }
    /// Tipos cuja representação JS é primitiva (número, string, bool, null).
    pub fn is_js_primitive(&self, t: &Ty) -> bool {
        let t = self.resolve_param_bound(t);
        match &t {
            Ty::Null => true,
            Ty::Iface { class, .. } => {
                Some(*class) == self.int_
                    || Some(*class) == self.double_
                    || Some(*class) == self.num_
                    || Some(*class) == self.bool_
                    || Some(*class) == self.string_
                    || Some(*class) == self.null_
                    || Some(*class) == self.jsnumber
                    || Some(*class) == self.jsstring
                    || Some(*class) == self.jsbool
            }
            _ => false,
        }
    }
    pub fn resolve_param_bound(&self, t: &Ty) -> Ty {
        let mut t = t.clone();
        let mut guard = 0;
        while let Ty::Param { id, nullable, .. } = &t {
            let b = self.bound_of(*id);
            let n = *nullable;
            t = if n { b.with_nullable(true) } else { b };
            guard += 1;
            if guard > 8 {
                break;
            }
        }
        t
    }
    pub fn is_enum_class(&self, c: ClassId) -> bool {
        self.program.class(c).kind == ClassKind::Enum
    }

    // -----------------------------------------------------------------------
    // Hierarquia e membros
    // -----------------------------------------------------------------------

    pub fn class_name(&self, c: ClassId) -> &str {
        self.interner.resolve(self.program.class(c).name)
    }

    /// Supertipos diretos de `ty` (Iface) com argumentos substituídos.
    pub fn direct_supers(&self, ty: &Ty) -> Vec<Ty> {
        let Ty::Iface { class, args, .. } = ty else { return vec![] };
        let params = &self.class_params[class.0 as usize];
        let mut map = HashMap::new();
        for (p, a) in params.iter().zip(args.iter()) {
            map.insert(p.id, a.clone());
        }
        for p in params.iter().skip(args.len()) {
            map.insert(p.id, Ty::Dynamic);
        }
        self.class_supers[class.0 as usize].iter().map(|s| s.subst(&map)).collect()
    }

    /// `ty` visto como instância da classe `target` (`List<int>` → `Iterable<int>`).
    pub fn as_super(&self, ty: &Ty, target: ClassId) -> Option<Ty> {
        let ty = self.resolve_param_bound(ty);
        let Ty::Iface { class, .. } = &ty else { return None };
        if *class == target {
            return Some(ty.clone());
        }
        let mut seen = HashSet::new();
        let mut queue = vec![ty.clone()];
        while let Some(t) = queue.pop() {
            for s in self.direct_supers(&t) {
                if let Some(c) = s.class() {
                    if c == target {
                        return Some(s);
                    }
                    if seen.insert(c) {
                        queue.push(s);
                    }
                }
            }
        }
        if Some(target) == self.object {
            return self.object.map(Ty::iface);
        }
        None
    }

    pub fn is_subclass(&self, c: ClassId, target: ClassId) -> bool {
        if c == target {
            return true;
        }
        let mut seen = HashSet::new();
        let mut queue = vec![c];
        while let Some(x) = queue.pop() {
            for s in &self.class_supers[x.0 as usize] {
                if let Some(sc) = s.class() {
                    if sc == target {
                        return true;
                    }
                    if seen.insert(sc) {
                        queue.push(sc);
                    }
                }
            }
        }
        Some(target) == self.object
    }

    /// Cadeia de superclasses (extends) e mixins, da classe para cima.
    pub fn superclass_of(&self, c: ClassId) -> Option<ClassId> {
        self.program.class(c).supertype_class
    }

    /// Procura membro de instância declarado na classe (não herdado).
    pub fn declared_member(&self, c: ClassId, name: &str, setter: bool) -> Option<MemberKind> {
        let class = self.program.class(c);
        let key = if setter { format!("{name}_=") } else { name.to_string() };
        let found = self.interner.lookup(&key).and_then(|sym| class.instance_members.get(&sym).copied());
        let Some(fid) = found else {
            if setter {
                // `late final` sem inicializador aceita uma atribuição.
                let fsym = self.interner.lookup(name)?;
                let vid = class.fields.iter().copied().find(|v| {
                    let var = self.program.variable(*v);
                    var.name == fsym && !var.static_ && var.late
                })?;
                return Some(MemberKind::Field(vid));
            }
            return None;
        };
        let f = self.program.function(fid);
        if class.kind == ClassKind::Enum && name == "name" && matches!(f.node, dartforge_elements::model::FunctionRef::None) {
            return None;
        }
        Some(match f.kind {
            FunctionKind::ImplicitAccessor => {
                if setter {
                    MemberKind::Setter(fid)
                } else {
                    MemberKind::Field(f.variable?)
                }
            }
            FunctionKind::Getter => MemberKind::Getter(fid),
            FunctionKind::Setter => MemberKind::Setter(fid),
            _ => MemberKind::Method(fid),
        })
    }

    pub fn declared_static(&self, c: ClassId, name: &str, setter: bool) -> Option<MemberKind> {
        let class = self.program.class(c);
        let key = if setter { format!("{name}_=") } else { name.to_string() };
        let sym = self.interner.lookup(&key)?;
        let fid = *class.static_members.get(&sym)?;
        let f = self.program.function(fid);
        Some(match f.kind {
            FunctionKind::ImplicitAccessor => {
                if setter {
                    MemberKind::Setter(fid)
                } else {
                    MemberKind::Field(f.variable?)
                }
            }
            FunctionKind::Getter => MemberKind::Getter(fid),
            FunctionKind::Setter => MemberKind::Setter(fid),
            _ => MemberKind::Method(fid),
        })
    }

    /// Procura membro de instância em `ty` e supertipos (BFS), com substituição.
    pub fn lookup_member(&self, ty: &Ty, name: &str, setter: bool) -> Option<Member> {
        let ty = self.resolve_param_bound(ty);
        let start = match &ty {
            Ty::Iface { .. } => ty.clone(),
            Ty::Fn { .. } | Ty::Record { .. } | Ty::Never | Ty::Dynamic | Ty::Void => {
                if matches!(ty, Ty::Fn { .. }) && name == "call" {
                    return None;
                }
                self.object.map(Ty::iface)?
            }
            Ty::Null => self.null_.map(Ty::iface).or(self.object.map(Ty::iface))?,
            Ty::FutureOr { .. } => self.object.map(Ty::iface)?,
            Ty::Param { .. } => self.object.map(Ty::iface)?,
        };
        let mut seen = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start);
        while let Some(t) = queue.pop_front() {
            let Ty::Iface { class, args, .. } = &t else { continue };
            if !seen.insert(*class) {
                continue;
            }
            if let Some(kind) = self.declared_member(*class, name, setter) {
                let params = &self.class_params[class.0 as usize];
                let mut subst = HashMap::new();
                for (p, a) in params.iter().zip(args.iter()) {
                    subst.insert(p.id, a.clone());
                }
                for p in params.iter().skip(args.len()) {
                    subst.insert(p.id, Ty::Dynamic);
                }
                return Some(Member { class: *class, kind, subst });
            }
            for s in self.direct_supers(&t) {
                queue.push_back(s);
            }
        }
        // Object como último recurso.
        if let Some(o) = self.object {
            if !seen.contains(&o) {
                if let Some(kind) = self.declared_member(o, name, setter) {
                    return Some(Member { class: o, kind, subst: HashMap::new() });
                }
            }
        }
        None
    }

    /// Tipo de um membro já substituído: getter/campo → tipo do valor; método → tipo função; setter → tipo do parâmetro.
    pub fn member_ty(&self, m: &Member) -> Ty {
        let raw = match m.kind {
            MemberKind::Method(fid) => self.fn_ty(fid),
            MemberKind::Getter(fid) => self.ty_of(self.outline.functions[fid.0 as usize].return_type),
            MemberKind::Setter(fid) => self.outline.functions[fid.0 as usize]
                .parameters
                .first()
                .map(|p| self.ty_of(p.ty))
                .unwrap_or(Ty::Dynamic),
            MemberKind::Field(vid) => self.var_ty(vid),
        };
        raw.subst(&m.subst)
    }

    pub fn var_ty(&self, vid: VariableId) -> Ty {
        let v = &self.outline.variables[vid.0 as usize];
        let var = self.program.variable(vid);
        if let dartforge_elements::model::VariableRef::EnumConstant { .. } = var.node {
            if let Some(c) = var.class {
                return Ty::iface(c);
            }
        }
        match v.declared_type.or(v.inferred) {
            Some(t) => self.ty_of(t),
            None => Ty::Dynamic,
        }
    }

    /// Membro concreto (não abstrato) na cadeia de superclasses e mixins.
    pub fn has_concrete_member(&self, c: ClassId, name: &str, setter: bool) -> bool {
        let mut seen = HashSet::new();
        let mut queue = vec![c];
        while let Some(k) = queue.pop() {
            if !seen.insert(k) {
                continue;
            }
            if let Some(mk) = self.declared_member(k, name, setter) {
                let abstract_ = match mk {
                    MemberKind::Method(f) | MemberKind::Getter(f) | MemberKind::Setter(f) => self.program.function(f).abstract_,
                    MemberKind::Field(v) => self.program.variable(v).external && false,
                };
                if !abstract_ {
                    return true;
                }
            }
            let class = self.program.class(k);
            queue.extend(class.mixin_classes.iter().copied());
            if let Some(s) = class.supertype_class {
                queue.push(s);
            }
        }
        false
    }

    /// Classe (própria ou herdada, sem contar Object) que declara `noSuchMethod`.
    pub fn has_user_nsm(&self, c: ClassId) -> bool {
        let mut cur = Some(c);
        while let Some(k) = cur {
            if Some(k) == self.object {
                return false;
            }
            if self.declared_member(k, "noSuchMethod", false).is_some() {
                return true;
            }
            let class = self.program.class(k);
            for &mx in &class.mixin_classes {
                if self.declared_member(mx, "noSuchMethod", false).is_some() {
                    return true;
                }
            }
            cur = class.supertype_class;
        }
        false
    }

    /// Membros abstratos (nome, kind) alcançáveis pelos supertipos que não têm implementação concreta.
    pub fn unimplemented_abstract(&self, c: ClassId) -> Vec<(String, MemberKind)> {
        let mut out: Vec<(String, MemberKind)> = Vec::new();
        let mut seen = HashSet::new();
        let mut queue = vec![c];
        while let Some(k) = queue.pop() {
            if !seen.insert(k) || Some(k) == self.object {
                continue;
            }
            let class = self.program.class(k);
            for (&sym, &fid) in &class.instance_members {
                let f = self.program.function(fid);
                if !f.abstract_ {
                    continue;
                }
                let key = self.interner.resolve(sym).to_string();
                let (name, setter) = match key.strip_suffix("_=") {
                    Some(n) => (n.to_string(), true),
                    None => (key.clone(), false),
                };
                if out.iter().any(|(n, mk)| *n == name && matches!(mk, MemberKind::Setter(_)) == setter) {
                    continue;
                }
                if self.has_concrete_member(c, &name, setter) {
                    continue;
                }
                let mk = match f.kind {
                    FunctionKind::Getter => MemberKind::Getter(fid),
                    FunctionKind::Setter => MemberKind::Setter(fid),
                    FunctionKind::ImplicitAccessor => {
                        if setter { MemberKind::Setter(fid) } else { MemberKind::Getter(fid) }
                    }
                    _ => MemberKind::Method(fid),
                };
                out.push((name, mk));
            }
            for s in &self.class_supers[k.0 as usize] {
                if let Some(sc) = s.class() {
                    queue.push(sc);
                }
            }
        }
        out
    }

    pub fn fn_ty(&self, fid: FunctionElementId) -> Ty {
        let data = &self.outline.functions[fid.0 as usize];
        let f = self.program.function(fid);
        if f.kind == FunctionKind::ImplicitAccessor {
            return self.var_ty(f.variable.expect("acessor implícito sem variável"));
        }
        // Reconstrói a partir dos parâmetros (o `signature` de construtores ignora nomeados opcionais? usa mesmo).
        let mut pos = Vec::new();
        let mut opt = Vec::new();
        let mut named = Vec::new();
        for p in data.parameters.iter() {
            let t = self.ty_of(p.ty);
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    let n = p.name.map(|s| self.interner.resolve(s).to_string()).unwrap_or_default();
                    named.push((n, t, p.required));
                }
            }
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));
        Ty::Fn {
            type_params: data.type_params.iter().map(|p| self.ty_param_of(*p)).collect(),
            ret: Box::new(self.ty_of(data.return_type)),
            pos,
            opt,
            named,
            nullable: false,
        }
    }

    /// `C<dynamic, …>`: classe como literal de tipo sem argumentos.
    pub fn this_ty_default(&self, c: ClassId) -> Ty {
        let args = self.class_params[c.0 as usize].iter().map(|_| Ty::Dynamic).collect();
        Ty::Iface { class: c, args, nullable: false }
    }

    /// Tipo `this` de uma classe: `C<T1..Tn>`.
    pub fn this_ty(&self, c: ClassId) -> Ty {
        let args = self.class_params[c.0 as usize]
            .iter()
            .map(|p| Ty::Param { id: p.id, name: p.name.clone(), nullable: false })
            .collect();
        Ty::Iface { class: c, args, nullable: false }
    }

    /// Classe que declara o membro visto a partir de `ty` (para decidir `dartx`).
    pub fn is_ext_member(&self, ty: &Ty, name: &str, setter: bool) -> bool {
        if name.starts_with('_') {
            return false;
        }
        let t = self.resolve_param_bound(ty);
        let class = match &t {
            Ty::Iface { class, .. } => *class,
            Ty::Fn { .. } => match self.function_ {
                Some(f) => f,
                None => return false,
            },
            Ty::Null => match self.null_ {
                Some(n) => n,
                None => return false,
            },
            _ => return false,
        };
        // Receptor de tipo nativo: quase tudo é `dartx` (exceto `String.length`).
        let impl_class = if Some(class) == self.int_
            || Some(class) == self.double_
            || Some(class) == self.num_
        {
            self.jsnumber
        } else if Some(class) == self.bool_ {
            self.jsbool
        } else if Some(class) == self.string_ {
            self.jsstring
        } else {
            None
        };
        if let Some(ic) = impl_class {
            if Some(class) == self.string_ && name == "length" && !setter {
                return false;
            }
            let _ = ic;
            return true;
        }
        if self.native_set.contains(&class) {
            return true;
        }
        match self.lookup_member(&t, name, setter) {
            Some(m) => self.ext_set.contains(&m.class),
            None => self.ext_set.contains(&class),
        }
    }

    // -----------------------------------------------------------------------
    // Subtipagem aproximada e LUB
    // -----------------------------------------------------------------------

    pub fn is_subtype(&self, a: &Ty, b: &Ty) -> bool {
        self.is_subtype_depth(a, b, 0)
    }

    fn is_subtype_depth(&self, a: &Ty, b: &Ty, depth: u32) -> bool {
        if depth > 16 {
            return true;
        }
        if a == b {
            return true;
        }
        match (a, b) {
            (_, Ty::Dynamic) | (_, Ty::Void) => true,
            (Ty::Dynamic, _) => true,
            (Ty::Never, _) => true,
            (_, Ty::Iface { class, nullable, .. }) if Some(*class) == self.object => {
                *nullable || !a.is_nullable()
            }
            (Ty::Null, b) => b.is_nullable(),
            (a, Ty::FutureOr { arg, nullable }) => {
                if a.is_nullable() && !*nullable {
                    return false;
                }
                let an = a.non_null();
                self.is_subtype_depth(&an, arg, depth + 1)
                    || self.is_subtype_depth(&an, &self.t_future((**arg).clone()), depth + 1)
            }
            (Ty::FutureOr { arg, nullable }, b) => {
                if *nullable && !b.is_nullable() {
                    return false;
                }
                self.is_subtype_depth(arg, b, depth + 1)
                    && self.is_subtype_depth(&self.t_future((**arg).clone()), b, depth + 1)
            }
            (Ty::Param { id, nullable, .. }, b) => {
                if let Ty::Param { id: id2, nullable: n2, .. } = b {
                    if id == id2 {
                        return !*nullable || *n2;
                    }
                }
                if *nullable && !b.is_nullable() {
                    return false;
                }
                let bound = self.bound_of(*id);
                self.is_subtype_depth(&bound, b, depth + 1)
            }
            (_, Ty::Param { .. }) => false,
            (Ty::Iface { class, nullable, .. }, Ty::Iface { class: c2, args: args2, nullable: n2 }) => {
                if *nullable && !*n2 {
                    return false;
                }
                let Some(sup) = self.as_super(&a.non_null(), *c2) else {
                    return false;
                };
                let _ = class;
                let sargs = sup.args();
                if sargs.len() != args2.len() {
                    return true;
                }
                sargs.iter().zip(args2.iter()).all(|(x, y)| self.is_subtype_depth(x, y, depth + 1))
            }
            (Ty::Iface { class, nullable, .. }, Ty::Fn { nullable: n2, .. }) => {
                let _ = (class, nullable, n2);
                false
            }
            (Ty::Fn { nullable, .. }, Ty::Iface { class, nullable: n2, .. }) => {
                (!*nullable || *n2) && Some(*class) == self.function_
            }
            (Ty::Fn { ret, pos, opt, named, nullable, .. }, Ty::Fn { ret: r2, pos: p2, opt: o2, named: n2, nullable: nn2, .. }) => {
                if *nullable && !*nn2 {
                    return false;
                }
                if !self.is_subtype_depth(ret, r2, depth + 1) {
                    return false;
                }
                if pos.len() > p2.len() || pos.len() + opt.len() < p2.len() + o2.len() {
                    return false;
                }
                for (i, t2) in p2.iter().chain(o2.iter()).enumerate() {
                    let t1 = if i < pos.len() { &pos[i] } else { &opt[i - pos.len()] };
                    if !self.is_subtype_depth(t2, t1, depth + 1) {
                        return false;
                    }
                }
                for (n, t2, _) in n2 {
                    match named.iter().find(|(m, _, _)| m == n) {
                        Some((_, t1, _)) => {
                            if !self.is_subtype_depth(t2, t1, depth + 1) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                true
            }
            (Ty::Record { pos, named, nullable }, Ty::Record { pos: p2, named: n2, nullable: nn2 }) => {
                (!*nullable || *nn2)
                    && pos.len() == p2.len()
                    && named.len() == n2.len()
                    && pos.iter().zip(p2).all(|(x, y)| self.is_subtype_depth(x, y, depth + 1))
                    && named.iter().zip(n2).all(|((a, x), (b, y))| a == b && self.is_subtype_depth(x, y, depth + 1))
            }
            (Ty::Record { nullable, .. }, Ty::Iface { class, nullable: n2, .. }) => {
                (!*nullable || *n2) && Some(*class) == self.record_
            }
            _ => false,
        }
    }

    /// Menor supertipo comum (aproximado).
    pub fn lub(&self, a: &Ty, b: &Ty) -> Ty {
        if a == b {
            return a.clone();
        }
        match (a, b) {
            (Ty::Never, x) | (x, Ty::Never) => x.clone(),
            (Ty::Null, x) | (x, Ty::Null) => x.with_nullable(true),
            (Ty::Dynamic, _) | (_, Ty::Dynamic) => Ty::Dynamic,
            (Ty::Void, _) | (_, Ty::Void) => Ty::Void,
            _ => {
                let nullable = a.is_nullable() || b.is_nullable();
                let an = a.non_null();
                let bn = b.non_null();
                if self.is_subtype(&an, &bn) {
                    return bn.with_nullable(nullable);
                }
                if self.is_subtype(&bn, &an) {
                    return an.with_nullable(nullable);
                }
                // int/double → num
                if self.is_num_like(&an) && self.is_num_like(&bn) {
                    return self.t_num().with_nullable(nullable);
                }
                // Procura superclasse comum na cadeia de `extends` de `a`.
                if let (Ty::Iface { .. }, Ty::Iface { .. }) = (&an, &bn) {
                    let mut cur = Some(an.clone());
                    while let Some(t) = cur {
                        if self.is_subtype(&bn, &t) {
                            return t.with_nullable(nullable);
                        }
                        let supers = self.direct_supers(&t);
                        cur = supers.first().cloned();
                    }
                }
                if let (Ty::Fn { .. }, Ty::Fn { .. }) = (&an, &bn) {
                    return self.function_.map(Ty::iface).unwrap_or(Ty::Dynamic).with_nullable(nullable);
                }
                self.t_object().with_nullable(nullable)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Nomes
    // -----------------------------------------------------------------------

    pub fn lib_of_class(&self, c: ClassId) -> LibraryId {
        self.program.class(c).library
    }

    pub fn lib_ident(&self, lib: LibraryId) -> &str {
        &self.libs[lib.0 as usize].ident
    }

    /// Prefixo de receita rti da classe: `core|int`, `main|P`.
    pub fn class_recipe(&self, c: ClassId) -> String {
        let lib = self.lib_of_class(c);
        format!("{}|{}", self.lib_ident(lib), self.class_name(c))
    }

    pub fn sym(&self, name: &str) -> Option<SymbolId> {
        self.interner.lookup(name)
    }

    pub fn name(&self, s: SymbolId) -> &str {
        self.interner.resolve(s)
    }

    pub fn unit(&self, u: UnitId) -> &'a dartforge_elements::model::Unit {
        self.program.unit(u)
    }

    /// Extensões visíveis numa biblioteca: as declaradas nela e nas importadas
    /// (e em `dart:core`).
    pub fn visible_extensions(&self, lib: LibraryId) -> Vec<ExtensionId> {
        let mut libs: Vec<LibraryId> = vec![lib];
        let l = self.program.library(lib);
        for imp in &l.imports {
            if !libs.contains(&imp.library) {
                libs.push(imp.library);
            }
        }
        if let Some(c) = self.program.core {
            if !libs.contains(&c) {
                libs.push(c);
            }
        }
        // Exports transitivos das bibliotecas visíveis.
        let mut i = 0;
        while i < libs.len() {
            let cur = libs[i];
            for exp in &self.program.library(cur).exports {
                if !libs.contains(&exp.library) {
                    libs.push(exp.library);
                }
            }
            i += 1;
        }
        let mut out = Vec::new();
        for (i, e) in self.program.extensions.iter().enumerate() {
            if libs.contains(&e.library) {
                out.push(ExtensionId(i as u32));
            }
        }
        out
    }
}

/// Deriva identificadores e caminhos de módulo das bibliotecas.
fn compute_libs(program: &Program) -> Vec<LibInfo> {
    // Diretório da entrada, para caminhos relativos.
    let entry_dir: Option<String> = program.entry.and_then(|e| {
        let lib = program.library(e);
        let uri = &lib.uri;
        uri.strip_prefix("file:///").map(|p| {
            let p = p.replace('\\', "/");
            match p.rfind('/') {
                Some(i) => p[..=i].to_string(),
                None => String::new(),
            }
        })
    });
    let mut used: HashSet<String> = HashSet::new();
    let mut infos = Vec::with_capacity(program.libraries.len());
    for lib in &program.libraries {
        let uri = &lib.uri;
        let info = if let Some(name) = uri.strip_prefix("dart:") {
            LibInfo { ident: name.to_string(), module_path: String::new(), is_sdk: true, js_var: name.to_string() }
        } else if let Some(rest) = uri.strip_prefix("package:") {
            let path = rest.strip_suffix(".dart").unwrap_or(rest);
            let ident = path_ident(path);
            LibInfo { ident: ident.clone(), module_path: format!("packages/{path}.js"), is_sdk: false, js_var: format!("L${ident}") }
        } else {
            let p = uri.strip_prefix("file:///").unwrap_or(uri).replace('\\', "/");
            let rel = match &entry_dir {
                Some(d) if p.starts_with(d.as_str()) => p[d.len()..].to_string(),
                _ => p.replace(':', "_"),
            };
            let stem = rel.strip_suffix(".dart").unwrap_or(&rel).to_string();
            let ident = path_ident(&stem);
            LibInfo { ident: ident.clone(), module_path: format!("{stem}.js"), is_sdk: false, js_var: format!("L${ident}") }
        };
        let mut info = info;
        if !info.is_sdk {
            let mut candidate = info.ident.clone();
            let mut n = 1;
            while used.contains(&candidate) {
                candidate = format!("{}${n}", info.ident);
                n += 1;
            }
            used.insert(candidate.clone());
            info.js_var = format!("L${candidate}");
            info.ident = candidate;
        }
        infos.push(info);
    }
    infos
}

/// `lib/src/a-b` → `lib__src__a_b`, com caracteres inválidos escapados.
pub fn path_ident(path: &str) -> String {
    let mut out = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '/' | '\\' => out.push_str("__"),
            '-' => out.push('_'),
            '.' => {
                if chars.peek() == Some(&'.') {
                    chars.next();
                    out.push_str("__");
                } else {
                    out.push('.');
                }
            }
            _ => out.push(c),
        }
    }
    let mut res = String::new();
    for c in out.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            res.push(c);
        } else {
            res.push_str(&format!("${}", c as u32));
        }
    }
    if res.chars().next().is_some_and(|c| c.is_ascii_digit()) || js::is_reserved(&res) {
        res.insert(0, '$');
    }
    res
}
