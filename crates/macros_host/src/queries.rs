//! The queries the executor makes during an execution (`macro.consulta`),
//! served from the current program, and the assembly's [`Resolver`].
//!
//! The behavior follows that of CFE 3.6.2 (`kernel_macro_introspectors.dart`,
//! `identifiers.dart`), the oracle:
//!
//! * `resolveIdentifier(uri, name)` only looks at the library's **local**
//!   members (`name=` asks for the setter);
//! * a type identifier comes out with the URI of its library (a prefix in the
//!   assembly); an instance member comes out without a URI (`this.`); a
//!   static member and a constructor come out as `prefix.Class.name`; a
//!   parameter, just the name;
//! * `typesOf` only returns classes; `topLevelDeclarationsOf` and `valuesOf`
//!   do not exist in CFE 3.6.2 (an implementation error here too).
//!
//! Where `crates/types` is needed (`inferType`, generic subtyping,
//! `asInstanceOf`) the query answers with a clear error: it is the request
//! filed with the owner of `types`, not an approximation.
use crate::assembly::{AugmentedType, Code, IdentifierKind, Part, ResolvedIdentifier, Resolver};
use crate::executor::{QueryError, QueryService};
use crate::modelo::{Key, Table, Vista};
use dartforge_elements::model::*;
use serde_json::{Value, json};

/// A resolved type (`StaticType`), structurally comparable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StaticType {
    Named { key: Key, args: Vec<StaticType>, nullable: bool },
    /// Function and record types: compared by their canonical text.
    Structural { text: String, nullable: bool },
}

/// The `StaticType`s handed to the executor, by the key it sends back in
/// `ehExatamente`/`ehSubtipo` (isExactly/isSubtype). Equal types get **the
/// same** key (the comparison is structural, so the key is just an alias of
/// the type): that way the answer to `resolver` does not depend on how many
/// `resolve` calls came before, and the expansion cache ([`crate::cache`])
/// can revalidate it.
#[derive(Debug, Default)]
pub struct Statics {
    list: Vec<StaticType>,
    by_type: std::collections::HashMap<StaticType, u64>,
}

impl Statics {
    /// The key of `e` (starting at 1), the same for equal types.
    fn key(&mut self, e: StaticType) -> u64 {
        if let Some(&k) = self.by_type.get(&e) {
            return k;
        }
        self.list.push(e.clone());
        let k = self.list.len() as u64;
        self.by_type.insert(e, k);
        k
    }

    fn get(&self, key: u64) -> Option<&StaticType> {
        self.list.get((key as usize).wrapping_sub(1))
    }
}

/// The query service of an execution.
pub struct QueryHandler<'a, 'p> {
    pub view: &'a Vista<'p>,
    pub table: &'a mut Table,
    pub statics: &'a mut Statics,
}

fn arg_u64(args: &Value, k: &str) -> Result<u64, QueryError> {
    args.get(k).and_then(Value::as_u64).ok_or_else(|| QueryError::unexpected(format!("missing argument '{k}'")))
}

impl QueryHandler<'_, '_> {
    fn key(&self, id: u64) -> Result<Key, QueryError> {
        self.table.key(id).cloned().ok_or_else(|| QueryError::unexpected(format!("unknown identifier {id}")))
    }

    fn resolve_identifier(&mut self, args: &Value) -> Result<Value, QueryError> {
        let uri = args.get("uri").and_then(Value::as_str).unwrap_or("");
        let name = args.get("nome").and_then(Value::as_str).unwrap_or("");
        let v = self.view;
        let lib = v
            .library_by_uri(uri)
            .ok_or_else(|| QueryError::implementation(format!("Library at uri {uri} could not be resolved.")))?;
        let (member, setter) = match name.strip_suffix('=') {
            Some(m) => (m, true),
            None => (name, false),
        };
        let not_found = || QueryError::implementation(format!("Unable to find top level identifier \"{name}\" in {uri}"));
        let sym = v.interner.lookup(member).ok_or_else(not_found)?;
        let b = v.program.library(lib).declared.get(&sym).copied().ok_or_else(not_found)?;
        let el = if setter { b.setter } else { b.getter }.ok_or_else(not_found)?;
        let lib_uri = uri.to_string();
        let key = match el {
            Element::Class(_) => Key::Type { lib: lib_uri, name: member.into() },
            Element::Typedef(_) => Key::Typedef { lib: lib_uri, name: member.into() },
            Element::Extension(_) => Key::Extension { lib: lib_uri, name: member.into() },
            Element::Function(_) => Key::TopLevelFunction { lib: lib_uri, name: member.into(), setter },
            Element::Variable(_) => Key::TopLevelVariable { lib: lib_uri, name: member.into() },
            Element::Prefix(..) => return Err(not_found()),
        };
        Ok(json!({"id": self.table.id(key), "nome": name}))
    }

    /// The `StaticType` of a type annotation (the model's JSON).
    fn static_type(&self, t: &Value) -> Result<StaticType, QueryError> {
        let nullable = t.get("anulavel").and_then(Value::as_bool).unwrap_or(false);
        match t.get("t").and_then(Value::as_str) {
            Some("nomeado") => {
                let id = t.get("ident").and_then(|i| i.get("id")).and_then(Value::as_u64).unwrap_or(0);
                let key = self.key(id)?;
                let args = t
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().map(|a| self.static_type(a)).collect::<Result<Vec<_>, _>>())
                    .transpose()?
                    .unwrap_or_default();
                Ok(StaticType::Named { key, args, nullable })
            }
            Some("funcao" | "record") => {
                let mut v = t.clone();
                v.as_object_mut().map(|o| o.remove("anulavel"));
                Ok(StaticType::Structural { text: v.to_string(), nullable })
            }
            Some("omitido") => Err(QueryError::implementation("resolve of an omitted type: use inferType")),
            _ => Err(QueryError::implementation(
                "resolve only accepts NamedTypeAnnotationCode, FunctionTypeAnnotationCode and RecordTypeAnnotationCode",
            )),
        }
    }

    fn static_type_json(&mut self, e: StaticType) -> Value {
        let (declaration, args) = match &e {
            StaticType::Named { key, args, .. } => {
                let d = self.view.declaration_json(self.table, key);
                let a: Vec<Value> = args.clone().into_iter().map(|x| self.static_type_json(x)).collect();
                (d, a)
            }
            _ => (None, Vec::new()),
        };
        let key = self.statics.key(e);
        json!({"chave": key, "declaracao": declaration, "args": args})
    }

    fn static_type_by_key(&self, args: &Value, k: &str) -> Result<StaticType, QueryError> {
        let n = arg_u64(args, k)?;
        self.statics.get(n).cloned().ok_or_else(|| QueryError::unexpected(format!("unknown type {n}")))
    }

    /// Nominal subtyping without type arguments (what can be decided from
    /// `elements`); with arguments, it belongs to inference (`crates/types`).
    fn is_subtype(&self, a: &StaticType, b: &StaticType) -> Result<bool, QueryError> {
        if a == b {
            return Ok(true);
        }
        let (StaticType::Named { key: ka, args: aa, nullable: na }, StaticType::Named { key: kb, args: ab, nullable: nb }) = (a, b)
        else {
            return Err(QueryError::unexpected("isSubtypeOf of function and record types requires crates/types"));
        };
        let top = |c: &Key| matches!(c, Key::Dynamic) || matches!(c, Key::Type { lib, name } if lib == "dart:core" && name == "Object");
        if matches!(kb, Key::Dynamic) || (top(kb) && (*nb || !*na)) {
            return Ok(true);
        }
        if *na && !*nb {
            return Ok(false);
        }
        if !aa.is_empty() || !ab.is_empty() {
            return Err(QueryError::unexpected("isSubtypeOf with type arguments requires crates/types"));
        }
        let (Some(x), Some(y)) = (self.view.class_of(ka), self.view.class_of(kb)) else { return Ok(false) };
        let mut stack = vec![x];
        let mut seen = std::collections::HashSet::new();
        while let Some(c) = stack.pop() {
            if c == y {
                return Ok(true);
            }
            if !seen.insert(c) {
                continue;
            }
            let cl = self.view.program.class(c);
            stack.extend(cl.supertype_class);
            stack.extend(cl.interface_classes.iter().copied());
            stack.extend(cl.mixin_classes.iter().copied());
            stack.extend(cl.on_classes.iter().copied());
        }
        Ok(false)
    }
}

impl QueryService for QueryHandler<'_, '_> {
    fn query(&mut self, kind: &str, args: &Value) -> Result<Value, QueryError> {
        match kind {
            "resolverIdentificador" => self.resolve_identifier(args),
            "declaracao" => {
                let c = self.key(arg_u64(args, "ident")?)?;
                self.view
                    .declaration_json(self.table, &c)
                    .ok_or_else(|| QueryError::implementation(format!("Unable to resolve identifier {}", c.name())))
            }
            "membros" => {
                let c = self.key(arg_u64(args, "dono")?)?;
                let member_kind = args.get("tipo").and_then(Value::as_str).unwrap_or("");
                if member_kind == "valores" {
                    return Err(QueryError::implementation("valuesOf does not exist in CFE 3.6.2"));
                }
                let cid = self
                    .view
                    .class_of(&c)
                    .ok_or_else(|| QueryError::implementation("Only introspection on classes is supported"))?;
                Ok(Value::Array(self.view.members_json(self.table, cid, member_kind)))
            }
            "tiposDe" => {
                let id = arg_u64(args, "biblioteca")?;
                let uri = self.table.library_uri(id).map(str::to_string).unwrap_or_default();
                let lib = self
                    .view
                    .library_by_uri(&uri)
                    .ok_or_else(|| QueryError::implementation(format!("Library at uri {uri} could not be resolved.")))?;
                Ok(Value::Array(self.view.library_types(self.table, lib)))
            }
            "declaracoesDe" => Err(QueryError::implementation("topLevelDeclarationsOf does not exist in CFE 3.6.2")),
            "resolver" => {
                let t = args.get("tipo").ok_or_else(|| QueryError::unexpected("missing argument 'tipo'"))?;
                let e = self.static_type(t)?;
                Ok(self.static_type_json(e))
            }
            "ehExatamente" => {
                let (a, b) = (self.static_type_by_key(args, "a")?, self.static_type_by_key(args, "b")?);
                Ok(json!(a == b))
            }
            "ehSubtipo" => {
                let (a, b) = (self.static_type_by_key(args, "a")?, self.static_type_by_key(args, "b")?);
                self.is_subtype(&a, &b).map(|x| json!(x))
            }
            "comoInstanciaDe" => Err(QueryError::unexpected("asInstanceOf requires crates/types")),
            "inferirTipo" => Err(QueryError::unexpected("inferType requires the inference of crates/types")),
            _ => Err(QueryError::unexpected(format!("unknown query: {kind}"))),
        }
    }
}

// ----------------------------------------------------------- assembly

/// The assembly's [`Resolver`] over the current program.
pub struct ProgramResolver<'a, 'p> {
    pub view: &'a Vista<'p>,
    pub table: &'a std::cell::RefCell<&'a mut Table>,
}

/// The code of a model type annotation (like `TypeAnnotation.code`).
pub fn type_code(t: &Value) -> Code {
    let mut parts = Vec::new();
    match t.get("t").and_then(Value::as_str) {
        Some("nomeado") => {
            parts.push(Part::Ident(t["ident"]["id"].as_u64().unwrap_or(0)));
            let args = t.get("args").and_then(Value::as_array).cloned().unwrap_or_default();
            if !args.is_empty() {
                parts.push(Part::Text("<".into()));
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        parts.push(Part::Text(", ".into()));
                    }
                    parts.push(Part::Code(type_code(a)));
                }
                parts.push(Part::Text(">".into()));
            }
        }
        Some("omitido") => parts.push(Part::Omitted(t["chave"].as_u64().unwrap_or(0))),
        _ => parts.push(Part::Text("dynamic".into())),
    }
    if t.get("anulavel").and_then(Value::as_bool) == Some(true) {
        parts.push(Part::Text("?".into()));
    }
    Code { kind: "namedTypeAnnotation".into(), parts }
}

impl Resolver for ProgramResolver<'_, '_> {
    fn identifier(&self, id: u64) -> Result<ResolvedIdentifier, String> {
        let key = self.table.borrow().key(id).cloned().ok_or_else(|| format!("unknown identifier {id}"))?;
        let name = key.name().to_string();
        let top = |uri: Option<String>| ResolvedIdentifier { name: name.clone(), kind: IdentifierKind::TopLevel, uri, scope: None };
        Ok(match &key {
            Key::Type { lib, .. } | Key::Typedef { lib, .. } | Key::TopLevelFunction { lib, .. } | Key::TopLevelVariable { lib, .. } => {
                top(Some(lib.clone()))
            }
            // CFE 3.6.2 gives no URI to an extension (a TODO of theirs): the name comes out.
            Key::Extension { .. } | Key::TypeParameter { .. } | Key::Void | Key::Unresolved(_) => top(None),
            Key::Dynamic => top(Some("dart:core".into())),
            Key::Parameter { .. } => ResolvedIdentifier { name, kind: IdentifierKind::Local, uri: None, scope: None },
            Key::Constructor { lib, owner, .. } => {
                ResolvedIdentifier { name, kind: IdentifierKind::Static, uri: Some(lib.clone()), scope: Some(owner.clone()) }
            }
            Key::Method { lib, owner, .. } | Key::Field { lib, owner, .. } => {
                let is_static = match &key {
                    Key::Method { .. } => self.view.function(&key).is_some_and(|f| self.view.program.function(f).static_),
                    _ => self
                        .view
                        .class_of(&Key::Type { lib: lib.clone(), name: owner.clone() })
                        .and_then(|c| {
                            self.view.program.class(c).fields.iter().copied().find(|v| {
                                self.view.interner.resolve(self.view.program.variable(*v).name) == name
                            })
                        })
                        .is_some_and(|v| self.view.program.variable(v).static_),
                };
                if is_static {
                    ResolvedIdentifier { name, kind: IdentifierKind::Static, uri: Some(lib.clone()), scope: Some(owner.clone()) }
                } else {
                    ResolvedIdentifier { name, kind: IdentifierKind::Instance, uri: None, scope: None }
                }
            }
        })
    }

    fn augmented_type(&self, id: u64) -> Result<AugmentedType, String> {
        let key = self.table.borrow().key(id).cloned().ok_or_else(|| format!("unknown identifier {id}"))?;
        let c = self.view.class_of(&key).ok_or_else(|| format!("Unsupported augmentation type {}", key.name()))?;
        let cl = self.view.program.class(c);
        let m = cl.modifiers;
        let (keyword, modifiers) = match cl.kind {
            ClassKind::Class => (
                "class",
                [("abstract", m.abstract_), ("base", m.base), ("final", m.final_), ("interface", m.interface), ("mixin", m.mixin), ("sealed", m.sealed)]
                    .into_iter()
                    .filter_map(|(p, s)| s.then_some(p))
                    .collect(),
            ),
            ClassKind::Mixin => ("mixin", if m.base { vec!["base"] } else { vec![] }),
            ClassKind::Enum => ("enum", vec![]),
            _ => return Err(format!("Unsupported augmentation type {}", key.name())),
        };
        let json = {
            let mut t = self.table.borrow_mut();
            self.view.class_json(&mut t, c)
        };
        let parameters = json["tparams"]
            .as_array()
            .map(|l| {
                l.iter()
                    .map(|p| {
                        let mut parts = vec![Part::Text(p["ident"]["nome"].as_str().unwrap_or("").to_string())];
                        if !p["limite"].is_null() {
                            parts.push(Part::Text(" extends ".into()));
                            parts.push(Part::Code(type_code(&p["limite"])));
                        }
                        Code { kind: "typeParameter".into(), parts }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(AugmentedType { keyword, modifiers, name: key.name().to_string(), parameters })
    }

    fn inferred_type(&self, _omitted: u64) -> Option<Code> {
        // `inferType` belongs to the inference of `crates/types` (request
        // filed); without it, text that uses an omitted type is not
        // assembled.
        None
    }
}
