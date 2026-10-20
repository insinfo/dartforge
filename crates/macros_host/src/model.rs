//! The model the host serves to the executor (docs/MACROS-PROTOCOLO.md §5):
//! declarations and type annotations of `elements` as JSON, with identifiers
//! that are **stable** across phases.
//!
//! Between one phase and the next the program is reloaded with the partial
//! augmentation, and the `elements` ids change (a new class from phase 1
//! shifts the `ClassId`s). The assembly merges the results of all phases by
//! the identifier of the augmented type (Rule 1), so the same type must have
//! the same id in phase 2 and in phase 3: the id is that of a textual [`Key`]
//! (library, owner, name, kind), resolved again in the current program on
//! every use.
//!
//! What each declaration shows follows CFE 3.6.2 (the oracle), including
//! where it diverges from the spec (`kernel_macro_introspectors.dart`):
//! members in declaration order (not lexicographic), `hasBody` always true,
//! empty metadata, no type parameters on methods and constructors, and the
//! constructor return type omitted.
use dartforge_elements::model::*;
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind, TypeKind};
use dartforge_intern::{Interner, SymbolId};
use serde_json::{Value, json};
use std::collections::HashMap;

/// What an identifier denotes, in terms that survive the reload.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// Top-level class, mixin, enum or extension type.
    Type {
        lib: String,
        name: String,
    },
    Typedef {
        lib: String,
        name: String,
    },
    Extension {
        lib: String,
        name: String,
    },
    /// Top-level function, getter or setter (`setter` tells the pair apart).
    TopLevelFunction {
        lib: String,
        name: String,
        setter: bool,
    },
    TopLevelVariable {
        lib: String,
        name: String,
    },
    /// Method, getter, setter or operator.
    Method {
        lib: String,
        owner: String,
        name: String,
        setter: bool,
    },
    Field {
        lib: String,
        owner: String,
        name: String,
    },
    /// Constructor (`""` for the unnamed one).
    Constructor {
        lib: String,
        owner: String,
        name: String,
    },
    /// Formal parameter of a member (the member's [`Key`] in `owner`).
    Parameter {
        owner: Box<Key>,
        name: String,
    },
    /// Type parameter of a class or typedef.
    TypeParameter {
        owner: Box<Key>,
        name: String,
    },
    Void,
    Dynamic,
    /// A name that did not resolve (invalid program): emitted as is.
    Unresolved(String),
}

impl Key {
    /// The identifier's name.
    pub fn name(&self) -> &str {
        match self {
            Key::Type { name, .. }
            | Key::Typedef { name, .. }
            | Key::Extension { name, .. }
            | Key::TopLevelFunction { name, .. }
            | Key::TopLevelVariable { name, .. }
            | Key::Method { name, .. }
            | Key::Field { name, .. }
            | Key::Constructor { name, .. }
            | Key::Parameter { name, .. }
            | Key::TypeParameter { name, .. } => name,
            Key::Void => "void",
            Key::Dynamic => "dynamic",
            Key::Unresolved(n) => n,
        }
    }
}

/// Where an omitted type is (for `inferType` and for the assembly).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Omitted {
    pub owner: Key,
    /// `return`, `type` (of a field/variable) or the parameter
    /// name.
    pub place: String,
}

/// The id table of the macro session: persistent across reloads.
#[derive(Debug, Default)]
pub struct Table {
    keys: Vec<Key>,
    by_key: HashMap<Key, u64>,
    omitted: Vec<Omitted>,
    by_omitted: HashMap<Omitted, u64>,
    libraries: Vec<String>,
}

impl Table {
    /// The id of `c` (starting at 1).
    pub fn id(&mut self, c: Key) -> u64 {
        if let Some(&id) = self.by_key.get(&c) {
            return id;
        }
        self.keys.push(c.clone());
        let id = self.keys.len() as u64;
        self.by_key.insert(c, id);
        id
    }

    pub fn key(&self, id: u64) -> Option<&Key> {
        self.keys.get((id as usize).wrapping_sub(1))
    }

    pub fn omitted(&mut self, o: Omitted) -> u64 {
        if let Some(&id) = self.by_omitted.get(&o) {
            return id;
        }
        self.omitted.push(o.clone());
        let id = self.omitted.len() as u64;
        self.by_omitted.insert(o, id);
        id
    }

    pub fn omitted_at(&self, id: u64) -> Option<&Omitted> {
        self.omitted.get((id as usize).wrapping_sub(1))
    }

    /// The id of a library (by URI).
    pub fn library(&mut self, uri: &str) -> u64 {
        match self.libraries.iter().position(|u| u == uri) {
            Some(i) => i as u64 + 1,
            None => {
                self.libraries.push(uri.to_string());
                self.libraries.len() as u64
            }
        }
    }

    pub fn library_uri(&self, id: u64) -> Option<&str> {
        self.libraries
            .get((id as usize).wrapping_sub(1))
            .map(String::as_str)
    }
}

/// Type parameters visible in an annotation: name → key.
type Scope = Vec<(SymbolId, Key)>;

/// The current program as seen by the model.
pub struct View<'p> {
    pub program: &'p Program,
    pub interner: &'p Interner,
}

impl<'p> View<'p> {
    fn n(&self, s: SymbolId) -> String {
        self.interner.resolve(s).to_string()
    }

    pub fn library_by_uri(&self, uri: &str) -> Option<LibraryId> {
        self.program
            .libraries
            .iter()
            .position(|l| l.uri == uri)
            .map(|i| LibraryId(i as u32))
    }

    fn uri(&self, lib: LibraryId) -> String {
        self.program.library(lib).uri.clone()
    }

    /// The key of a top-level or class element.
    pub fn class_key(&self, c: ClassId) -> Key {
        let cl = self.program.class(c);
        Key::Type {
            lib: self.uri(cl.library),
            name: self.n(cl.name),
        }
    }

    /// The class of a type key, in the current program.
    pub fn class_of(&self, c: &Key) -> Option<ClassId> {
        let Key::Type { lib, name } = c else {
            return None;
        };
        let l = self.library_by_uri(lib)?;
        let s = self.interner.lookup(name)?;
        match self.program.library(l).declared.get(&s)?.getter? {
            Element::Class(id) => Some(id),
            _ => None,
        }
    }

    /// The function element (method, accessor, constructor) of a key.
    pub fn function(&self, c: &Key) -> Option<FunctionElementId> {
        match c {
            Key::Method {
                lib,
                owner,
                name,
                setter,
            } => {
                let cl = self.program.class(self.class_of(&Key::Type {
                    lib: lib.clone(),
                    name: owner.clone(),
                })?);
                let key = if *setter {
                    format!("{name}_=")
                } else {
                    name.clone()
                };
                let s = self.interner.lookup(&key)?;
                cl.instance_members
                    .get(&s)
                    .or_else(|| cl.static_members.get(&s))
                    .copied()
            }
            Key::Constructor { lib, owner, name } => {
                let cl = self.program.class(self.class_of(&Key::Type {
                    lib: lib.clone(),
                    name: owner.clone(),
                })?);
                let s = self.interner.lookup(name)?;
                cl.constructors.get(&s).copied()
            }
            Key::TopLevelFunction { lib, name, setter } => {
                let l = self.library_by_uri(lib)?;
                let b = self
                    .program
                    .library(l)
                    .declared
                    .get(&self.interner.lookup(name)?)?;
                match if *setter { b.setter } else { b.getter }? {
                    Element::Function(f) => Some(f),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // ------------------------------------------------------------- types

    /// The key of a type name written in a unit.
    fn name_key(&self, lib: LibraryId, name: &[ast::Name], scope: &Scope) -> Key {
        let from_element = |el: Option<Element>, text: String| match el {
            Some(Element::Class(c)) => self.class_key(c),
            Some(Element::Typedef(t)) => {
                let td = self.program.typedef(t);
                Key::Typedef {
                    lib: self.uri(td.library),
                    name: self.n(td.name),
                }
            }
            Some(Element::Extension(e)) => {
                let ex = self.program.extension(e);
                Key::Extension {
                    lib: self.uri(ex.library),
                    name: ex.name.map(|s| self.n(s)).unwrap_or_default(),
                }
            }
            _ => Key::Unresolved(text),
        };
        match name {
            [n] => {
                if let Some((_, c)) = scope.iter().rev().find(|(s, _)| *s == n.sym) {
                    return c.clone();
                }
                let text = self.n(n.sym);
                if text == "dynamic" {
                    return Key::Dynamic;
                }
                from_element(self.program.lookup(lib, n.sym).and_then(|b| b.getter), text)
            }
            [p, n] => from_element(
                self.program
                    .lookup_prefixed(lib, p.sym, n.sym)
                    .and_then(|b| b.getter),
                format!("{}.{}", self.n(p.sym), self.n(n.sym)),
            ),
            _ => Key::Unresolved(
                name.iter()
                    .map(|x| self.n(x.sym))
                    .collect::<Vec<_>>()
                    .join("."),
            ),
        }
    }

    /// A written type annotation, or an omitted one (`None`).
    pub fn type_annotation(
        &self,
        t: &mut Table,
        unit: UnitId,
        ty: Option<ast::TypeId>,
        scope: &Scope,
        omitted: impl FnOnce() -> Omitted,
    ) -> Value {
        let Some(tid) = ty else {
            return json!({"t": "omitted", "key": t.omitted(omitted())});
        };
        self.written_type(t, unit, tid, scope)
    }

    fn written_type(&self, t: &mut Table, unit: UnitId, tid: ast::TypeId, scope: &Scope) -> Value {
        let u = self.program.unit(unit);
        let ty = u.ast.ty(tid);
        let lib = u.library;
        let mut v = match &ty.kind {
            TypeKind::Named { name, args } => {
                let c = self.name_key(lib, name, scope);
                let name = c.name().to_string();
                let args: Vec<Value> = args
                    .iter()
                    .map(|a| self.written_type(t, unit, *a, scope))
                    .collect();
                json!({"t": "named", "ident": {"id": t.id(c), "name": name}, "args": args})
            }
            TypeKind::Void => {
                json!({"t": "named", "ident": {"id": t.id(Key::Void), "name": "void"}, "args": []})
            }
            TypeKind::Function {
                return_type,
                type_params,
                parameters,
            } => {
                let ret = match return_type {
                    Some(r) => self.written_type(t, unit, *r, scope),
                    None => {
                        json!({"t": "named", "ident": {"id": t.id(Key::Dynamic), "name": "dynamic"}, "args": []})
                    }
                };
                let tparams: Vec<Value> = type_params
                    .iter()
                    .map(|p| json!({"name": self.n(p.name.sym)}))
                    .collect();
                let mut positional = Vec::new();
                let mut named = Vec::new();
                for p in parameters.iter() {
                    let ty = match p.ty {
                        Some(x) => self.written_type(t, unit, x, scope),
                        None => {
                            json!({"t": "named", "ident": {"id": t.id(Key::Dynamic), "name": "dynamic"}, "args": []})
                        }
                    };
                    let j = json!({"name": p.name.map(|n| self.n(n.sym)), "type": ty,
                        "named": p.kind == ParameterKind::Named,
                        "required": p.kind == ParameterKind::Required || p.required});
                    if p.kind == ParameterKind::Named {
                        named.push(j)
                    } else {
                        positional.push(j)
                    }
                }
                json!({"t": "function", "return": ret, "positional": positional, "named": named, "tparams": tparams})
            }
            TypeKind::Record { positional, named } => {
                let pos: Vec<Value> = positional
                    .iter()
                    .map(|x| json!({"type": self.written_type(t, unit, *x, scope)}))
                    .collect();
                let nam: Vec<Value> =
                    named.iter().map(|(n, x)| json!({"name": self.n(n.sym), "type": self.written_type(t, unit, *x, scope)})).collect();
                json!({"t": "record", "positional": pos, "named": nam})
            }
        };
        if ty.nullable {
            v["nullable"] = json!(true);
        }
        v
    }

    // ------------------------------------------------------ declarations

    pub fn library_json(&self, t: &mut Table, lib: LibraryId) -> Value {
        let l = self.program.library(lib);
        let v = l.features.version();
        json!({"k": "library", "id": t.library(&l.uri), "uri": l.uri, "version": [v.major, v.minor]})
    }

    fn ident(&self, t: &mut Table, c: Key) -> Value {
        let name = c.name().to_string();
        json!({"id": t.id(c), "name": name})
    }

    fn class_scope(&self, c: ClassId) -> (Key, Scope) {
        let owner = self.class_key(c);
        let scope = self
            .program
            .class(c)
            .type_params
            .iter()
            .map(|p| {
                (
                    p.name,
                    Key::TypeParameter {
                        owner: Box::new(owner.clone()),
                        name: self.n(p.name),
                    },
                )
            })
            .collect();
        (owner, scope)
    }

    /// The declaration of a class, mixin, enum or extension type.
    pub fn class_json(&self, t: &mut Table, c: ClassId) -> Value {
        let cl = self.program.class(c);
        let (key, scope) = self.class_scope(c);
        // The whole library goes in each declaration: a query answer may
        // mention a library the executor has not seen yet.
        let lib = self.library_json(t, cl.library);
        let tparams: Vec<Value> = cl
            .type_params
            .iter()
            .map(|p| {
                let bound = p.bound.map(|(u, b)| self.written_type(t, u, b, &scope));
                json!({"k": "tparam", "ident": self.ident(t, Key::TypeParameter { owner: Box::new(key.clone()), name: self.n(p.name) }),
                    "lib": lib, "bound": bound})
            })
            .collect();
        let types = |t: &mut Table, l: &[(UnitId, ast::TypeId)]| -> Vec<Value> {
            l.iter()
                .map(|(u, x)| self.written_type(t, *u, *x, &scope))
                .collect()
        };
        let ident = self.ident(t, key.clone());
        let m = cl.modifiers;
        match cl.kind {
            ClassKind::Mixin => {
                json!({"k": "mixin", "ident": ident, "lib": lib, "tparams": tparams, "base": m.base,
                "interfaces": types(t, &cl.interfaces), "constraints": types(t, &cl.on)})
            }
            ClassKind::Enum => json!({"k": "enum", "ident": ident, "lib": lib, "tparams": tparams,
                "interfaces": types(t, &cl.interfaces), "mixins": types(t, &cl.mixins)}),
            ClassKind::ExtensionType => {
                let rep = cl
                    .representation
                    .and_then(|v| match self.program.variable(v).node {
                        VariableRef::Representation { unit, decl } => {
                            match &self.program.unit(unit).ast.decl(decl).kind {
                                DeclKind::ExtensionType(et) => {
                                    Some(self.written_type(t, unit, et.representation_type, &scope))
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    });
                json!({"k": "extensionType", "ident": ident, "lib": lib, "tparams": tparams, "representation": rep})
            }
            _ => {
                let superclass = cl
                    .supertype
                    .map(|(u, x)| self.written_type(t, u, x, &scope));
                json!({"k": "class", "ident": ident, "lib": lib, "tparams": tparams, "abstract": m.abstract_,
                    "base": m.base, "external": false, "final": m.final_, "interface": m.interface, "mixin": m.mixin,
                    "sealed": m.sealed, "superclass": superclass,
                    "interfaces": types(t, &cl.interfaces), "mixins": types(t, &cl.mixins)})
            }
        }
    }

    fn parameters(
        &self,
        t: &mut Table,
        unit: UnitId,
        owner: &Key,
        ps: &[ast::Parameter],
        scope: &Scope,
    ) -> (Vec<Value>, Vec<Value>) {
        let lib = self.library_json(t, self.program.unit(unit).library);
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for p in ps {
            let Some(n) = p.name else { continue };
            let name = self.n(n.sym);
            let ty = self.type_annotation(t, unit, p.ty, scope, || Omitted {
                owner: owner.clone(),
                place: name.clone(),
            });
            let style = if p.this_ {
                "this"
            } else if p.super_ {
                "super"
            } else {
                "normal"
            };
            let is_named = p.kind == ParameterKind::Named;
            let j = json!({"k": "parameter",
                "ident": self.ident(t, Key::Parameter { owner: Box::new(owner.clone()), name: name.clone() }),
                "lib": lib, "type": ty, "named": is_named,
                "required": if is_named { p.required } else { p.kind == ParameterKind::Required },
                "style": style});
            if is_named {
                named.push(j)
            } else {
                positional.push(j)
            }
        }
        (positional, named)
    }

    /// The members of `c` of one kind (`fields`, `methods`, `constructors`),
    /// in declaration order along the chain (introductory declaration and
    /// augmentations), without the `augment` declarations (which do not
    /// introduce a member) — like CFE 3.6.2's `fullMemberIterator`.
    pub fn members_json(&self, t: &mut Table, c: ClassId, kind: &str) -> Vec<Value> {
        let (class_key, scope) = self.class_scope(c);
        let Key::Type {
            lib: lib_uri,
            name: owner,
        } = class_key.clone()
        else {
            return Vec::new();
        };
        let lib = self.library_json(t, self.program.class(c).library);
        let definer = self.ident(t, class_key);
        let mut out = Vec::new();
        for (unit, mid) in self.program.class_members(c) {
            let ast = &self.program.unit(unit).ast;
            let m = ast.member(mid);
            if m.augment {
                continue;
            }
            match (&m.kind, kind) {
                (MemberKind::Field(v), "fields") => {
                    for var in v.variables.iter() {
                        let name = self.n(var.name.sym);
                        let key = Key::Field {
                            lib: lib_uri.clone(),
                            owner: owner.clone(),
                            name: name.clone(),
                        };
                        let ty = self.type_annotation(t, unit, v.ty, &scope, || Omitted {
                            owner: key.clone(),
                            place: "type".into(),
                        });
                        out.push(json!({"k": "field", "ident": self.ident(t, key), "lib": lib, "owner": definer,
                            "static": v.static_, "abstract": v.abstract_, "const": v.const_, "external": v.external,
                            "final": v.final_, "late": v.late, "initializer": var.initializer.is_some(), "type": ty}));
                    }
                }
                (MemberKind::Method(fid), "methods") => {
                    let f = ast.function(*fid);
                    let Some(n) = f.name else { continue };
                    let setter = f.kind == ast::FunctionKind::Setter;
                    let key = Key::Method {
                        lib: lib_uri.clone(),
                        owner: owner.clone(),
                        name: self.n(n.sym),
                        setter,
                    };
                    let ret = self.type_annotation(t, unit, f.return_type, &scope, || Omitted {
                        owner: key.clone(),
                        place: "return".into(),
                    });
                    let (positional, named) = self.parameters(
                        t,
                        unit,
                        &key,
                        f.parameters.as_deref().unwrap_or(&[]),
                        &scope,
                    );
                    out.push(json!({"k": "method", "ident": self.ident(t, key), "lib": lib, "owner": definer,
                        "static": f.static_, "body": true, "external": f.external,
                        "operator": f.kind == ast::FunctionKind::Operator, "getter": f.kind == ast::FunctionKind::Getter,
                        "setter": setter, "return": ret, "positional": positional, "named": named, "tparams": []}));
                }
                (MemberKind::Constructor(k), "constructors") => {
                    let name = k.name.map(|n| self.n(n.sym)).unwrap_or_default();
                    let key = Key::Constructor {
                        lib: lib_uri.clone(),
                        owner: owner.clone(),
                        name,
                    };
                    let ret = json!({"t": "omitted", "key": t.omitted(Omitted { owner: key.clone(), place: "return".into() })});
                    let (positional, named) = self.parameters(t, unit, &key, &k.parameters, &scope);
                    out.push(json!({"k": "constructor", "ident": self.ident(t, key), "lib": lib, "owner": definer,
                        "body": true, "external": k.external, "const": k.const_, "factory": k.factory,
                        "return": ret, "positional": positional, "named": named, "tparams": []}));
                }
                _ => {}
            }
        }
        out
    }

    /// The declaration of a key, in the current program.
    pub fn declaration_json(&self, t: &mut Table, c: &Key) -> Option<Value> {
        match c {
            Key::Type { .. } => self.class_of(c).map(|id| self.class_json(t, id)),
            Key::TopLevelFunction { .. } => {
                let f = self.program.function(self.function(c)?);
                let FunctionRef::Function { unit, function } = f.node else {
                    return None;
                };
                let ast = self.program.unit(unit).ast.function(function);
                let lib = self.library_json(t, f.library);
                let scope = Vec::new();
                let ret = self.type_annotation(t, unit, ast.return_type, &scope, || Omitted {
                    owner: c.clone(),
                    place: "return".into(),
                });
                let (positional, named) =
                    self.parameters(t, unit, c, ast.parameters.as_deref().unwrap_or(&[]), &scope);
                Some(
                    json!({"k": "function", "ident": self.ident(t, c.clone()), "lib": lib,
                    "body": true, "external": f.external,
                    "operator": ast.kind == ast::FunctionKind::Operator,
                    "getter": ast.kind == ast::FunctionKind::Getter,
                    "setter": ast.kind == ast::FunctionKind::Setter,
                    "return": ret, "positional": positional, "named": named, "tparams": []}),
                )
            }
            Key::TopLevelVariable { lib, name } => {
                let lid = self.library_by_uri(lib)?;
                let sym = self.interner.lookup(name)?;
                let Element::Variable(id) = self.program.library(lid).declared.get(&sym)?.getter?
                else {
                    return None;
                };
                let v = self.program.variable(id);
                let VariableRef::TopLevel { unit, decl, index } = v.node else {
                    return None;
                };
                let DeclKind::Variables(list) = &self.program.unit(unit).ast.decl(decl).kind else {
                    return None;
                };
                let item = list.variables.get(index)?;
                let library = self.library_json(t, lid);
                let ty = self.type_annotation(t, unit, list.ty, &Vec::new(), || Omitted {
                    owner: c.clone(),
                    place: "type".into(),
                });
                Some(
                    json!({"k": "variable", "ident": self.ident(t, c.clone()), "lib": library,
                    "const": v.const_, "external": v.external, "final": v.final_, "late": v.late,
                    "initializer": item.initializer.is_some(), "type": ty}),
                )
            }
            Key::Method { lib, owner, .. }
            | Key::Constructor { lib, owner, .. }
            | Key::Field { lib, owner, .. } => {
                let cid = self.class_of(&Key::Type {
                    lib: lib.clone(),
                    name: owner.clone(),
                })?;
                let kind = match c {
                    Key::Method { .. } => "methods",
                    Key::Constructor { .. } => "constructors",
                    _ => "fields",
                };
                let id = t.id(c.clone());
                self.members_json(t, cid, kind)
                    .into_iter()
                    .find(|d| d["ident"]["id"].as_u64() == Some(id))
            }
            _ => None,
        }
    }

    /// The type declarations of a library (classes only, like the CFE).
    pub fn library_types(&self, t: &mut Table, lib: LibraryId) -> Vec<Value> {
        let mut out = Vec::new();
        for &u in &self.program.library(lib).units {
            let unit = self.program.unit(u);
            for &d in &unit.unit.declarations {
                let decl = unit.ast.decl(d);
                if decl.augment {
                    continue;
                }
                let name = match &decl.kind {
                    DeclKind::Class(c) => c.name.sym,
                    DeclKind::Mixin(m) => m.name.sym,
                    _ => continue,
                };
                if let Some(Element::Class(c)) = self
                    .program
                    .library(lib)
                    .declared
                    .get(&name)
                    .and_then(|b| b.getter)
                {
                    out.push(self.class_json(t, c));
                }
            }
        }
        out
    }
}
