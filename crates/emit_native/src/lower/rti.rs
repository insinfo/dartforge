//! Runtime types (RTI) in the lowering: recipes, the type of each
//! generic object and the `is`/`as` tests with type arguments.
//!
//! The design is dart2js's (`sdk/lib/_internal/js_runtime/lib/rti.dart`) and
//! lives in the runtime (`crates/runtime/src/types.rs`): a type is an id of a
//! canonical universe; the compiler describes each type by a **recipe**
//! (a short text, grammar in `types.rs`), which the runtime reads once — each
//! recipe has a lazy global (`dfr.<hash>`) and its getter
//! (`df.rti.<hash>`). A recipe with variables (`P<i>`: type parameter of the
//! class of the current code; `M<i>`: type argument of the current function)
//! is evaluated in the environment (`dartforge_rti_eval`: the type of `this` seen
//! as the class, and the tuple of the function).
//!
//! **Where the type is stored** (in the slot metadata, `Heap::metadata`):
//! at the creation of an instance of a generic class (the static type of the
//! creation, `C<T…>`), in collection literals with an element type, and in
//! closures (the signature). An object without a stored type has the raw type of the
//! class (`dynamic` arguments).
//!
//! **Where it is read:** `is`/`as`, `catch (e) on T`, type and
//! collection patterns with arguments. The test by class (`dartforge_is_subclass`)
//! stays where it is enough (a class without type arguments): there the answer is the
//! same and there is no type read.
//!
//! The classes that the recipes cite and their supertypes are registered at the
//! entry (`register_universe`), only when the program uses some recipe:
//! whoever does not use RTI pays nothing.

use super::fn_builder::FnBuilder;
use crate::context::Context;
use crate::hir::*;
use dartforge_elements::model::{ClassId, Element};
use dartforge_frontend::ast::{self, TypeKind};
use dartforge_intern::SymbolId;
use dartforge_types::table::{Type as T, TypeId, TypeParamId, TypeParamOwner};
use std::collections::{BTreeSet, HashMap};

/// Heap class id of the `Type` object (`types.rs`, the type class constant).
pub const CLASSE_TIPO: i64 = 0x3FFF_FF01;

/// The class `name` of `dart:core`.
fn core_class(ctx: &Context, name_str: &str) -> Option<ClassId> {
    let core = ctx.program.core?;
    let sym = ctx.interner.lookup(name_str)?;
    match ctx.program.lookup(core, sym)?.getter? {
        Element::Class(c) => Some(c),
        _ => None,
    }
}

/// Stable hash (FNV-1a 64) of a recipe: the name of its global.
fn recipe_hash(r: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in r.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// A recipe and whether it has variables (`P<i>`/`M<i>`) to evaluate.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub text: String,
    pub variables: bool,
}

impl Context<'_> {
    /// The RTI id of a class: the heap class id for those of the program
    /// (and those from source); for those of the SDK, the range `0x2000_0000 +` the order of the
    /// stable path; `Object` is 0 (its heap class id).
    pub fn id_rti(&self, c: ClassId) -> i64 {
        if let Some(id) = self.class_id_of(c) {
            return i64::from(id);
        }
        if Some(c) == self.core.object_class {
            return 0;
        }
        self.sdk_rti_ids.get(&c).map_or(0, |&k| 0x2000_0000 + i64::from(k))
    }

    /// The class of an RTI id (the inverse of `id_rti`).
    pub fn rti_id_class(&self, id: i64) -> Option<ClassId> {
        if id == 0 {
            return self.core.object_class;
        }
        if id >= 0x2000_0000 {
            let k = u32::try_from(id - 0x2000_0000).ok()?;
            return self.sdk_rti_ids.iter().find(|(_, v)| **v == k).map(|(c, _)| *c);
        }
        self.class_ids.iter().position(|x| *x == Some(id as u32)).map(|i| ClassId(i as u32))
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Index of `p` in the type parameters of its owner, with the letter of the
    /// variable: `P` (class of the current code) or `M` (current function, or
    /// the class in a factory, where the arguments come in the tuple).
    fn variable_of(&self, p: TypeParamId) -> Option<(char, usize)> {
        // That of an enclosing generic closure (or the current one).
        if let Some(&(_, _, i)) = self.local_params.iter().rev().find(|(_, x, _)| *x == p) {
            return Some(('M', i));
        }
        match self.ctx.table.param(p).owner {
            TypeParamOwner::Class(c) => {
                let i = self.ctx.outline.classes.get(c.0 as usize)?.type_params.iter().position(|&x| x == p)?;
                // Those of an extension type always come in the tuple (there is no
                // object that keeps them).
                Some((if self.class_by_tuple || self.ctx.is_extension_type(c) { 'M' } else { 'P' }, i))
            }
            // In the tuple of an extension member, the parameters of the extension
            // come before those of the member (`type_params_of`).
            TypeParamOwner::Function(f) => {
                let i = self.ctx.outline.functions.get(f.0 as usize)?.type_params.iter().position(|&x| x == p)?;
                let base = self.receiver_pattern(f.0 as usize).map_or(0, |(n, _)| n);
                Some(('M', base + i))
            }
            TypeParamOwner::Extension(e) => {
                let i = self.ctx.outline.extensions.get(e.0 as usize)?.type_params.iter().position(|&x| x == p)?;
                Some(('M', i))
            }
            _ => None,
        }
    }

    /// Recipe of a type from `crates/types`.
    pub fn type_recipe(&self, t: TypeId) -> Recipe {
        let mut r = Recipe { text: String::new(), variables: false };
        self.write_type(t, &mut Vec::new(), &HashMap::new(), &mut r);
        r
    }

    /// The recipe of `t` with the type parameters of `subst` replaced by the
    /// given types (the parameter of a generic class seen by the creation
    /// `C<int>(…)`, `typed_entry.rs`).
    pub(super) fn substituted_type_recipe(&self, t: TypeId, subst: Option<&HashMap<TypeParamId, TypeId>>) -> Recipe {
        let empty = HashMap::new();
        let mut r = Recipe { text: String::new(), variables: false };
        self.write_type(t, &mut Vec::new(), subst.unwrap_or(&empty), &mut r);
        r
    }

    fn write_type(
        &self,
        t: TypeId,
        linked_set: &mut Vec<TypeParamId>,
        subst: &HashMap<TypeParamId, TypeId>,
        r: &mut Recipe,
    ) {
        // An extension type is, at runtime, that of the representation.
        if matches!(self.ctx.table.get(t), T::ExtensionType { .. }) {
            let erased = self.ctx.erase(t);
            if erased != t {
                return self.write_type(erased, linked_set, subst, r);
            }
        }
        let nullable = self.ctx.table.get(t).is_declared_nullable();
        match self.ctx.table.get(t) {
            T::Dynamic => r.text.push('D'),
            T::Void => r.text.push('V'),
            T::Never => r.text.push('N'),
            T::Null => r.text.push('U'),
            T::Interface { class, args, .. } => {
                r.text.push_str(&format!("C{}", self.ctx.id_rti(*class)));
                if !args.is_empty() {
                    r.text.push('<');
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            r.text.push(',');
                        }
                        self.write_type(*a, linked_set, subst, r);
                    }
                    r.text.push('>');
                }
            }
            T::FutureOr { arg, .. } => {
                r.text.push_str("O<");
                self.write_type(*arg, linked_set, subst, r);
                r.text.push('>');
            }
            // `X & B` (a promoted type variable) only exists in the static
            // analysis; reified, it is `X` itself (as in dart2js and in the
            // NORM of the inference specification).
            T::TypeParameter { param, .. } | T::Intersection { param, .. } => {
                if let Some(&s) = subst.get(param) {
                    self.write_type(s, linked_set, subst, r);
                } else if let Some(i) = linked_set.iter().rposition(|x| x == param) {
                    r.text.push_str(&format!("B{i}"));
                } else if let Some((letter, i)) = self.variable_of(*param) {
                    r.text.push_str(&format!("{letter}{i}"));
                    r.variables = true;
                } else {
                    r.text.push('D');
                }
            }
            T::Function { type_params, ret, positional, optional, named, .. } => {
                let n = linked_set.len();
                linked_set.extend(type_params.iter().copied());
                r.text.push_str(&format!("F<{};", type_params.len()));
                self.write_type(*ret, linked_set, subst, r);
                r.text.push_str(&format!(";{};", positional.len()));
                for (i, p) in positional.iter().chain(optional.iter()).enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_type(*p, linked_set, subst, r);
                }
                r.text.push(';');
                for (i, (name_str, p, req)) in named.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(*name_str));
                    r.text.push(':');
                    self.write_type(*p, linked_set, subst, r);
                    if *req {
                        r.text.push('!');
                    }
                }
                r.text.push('>');
                linked_set.truncate(n);
            }
            T::Record { positional, named, .. } => {
                r.text.push_str("R<");
                for (i, p) in positional.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_type(*p, linked_set, subst, r);
                }
                r.text.push(';');
                for (i, (name_str, p)) in named.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(*name_str));
                    r.text.push(':');
                    self.write_type(*p, linked_set, subst, r);
                }
                r.text.push('>');
            }
            T::ExtensionType { .. } => r.text.push('D'),
        }
        if nullable && !matches!(self.ctx.table.get(t), T::Dynamic | T::Void | T::Null) {
            r.text.push('?');
        }
    }

    /// The position in the tuple (`M<i>`) of the type parameter `sym` of a
    /// generic closure in scope (the innermost with that name).
    pub(super) fn local_type_index(&self, sym: SymbolId) -> Option<usize> {
        self.local_params.iter().rev().find(|(s, _, _)| *s == sym).map(|&(_, _, i)| i)
    }

    /// How many type arguments the current tuple carries: those of the function
    /// (and of the extension), those of the class in a factory, those of the enclosing generic
    /// closures. Zero without a tuple.
    pub(super) fn tuple_size(&self) -> usize {
        if self.type_tuple.is_none() {
            return 0;
        }
        let class = if self.class_by_tuple { self.class_params().len() } else { 0 };
        let locals = self.local_params.iter().map(|&(_, _, i)| i + 1).max().unwrap_or(0);
        self.function_type_params.len().max(class).max(locals)
    }

    /// The names of the type parameters of the enclosing class (in order).
    pub(super) fn class_params(&self) -> Vec<SymbolId> {
        let Some(c) = self.enclosing_class else { return Vec::new() };
        self.ctx
            .outline
            .classes
            .get(c.0 as usize)
            .map(|d| d.type_params.iter().map(|&p| self.ctx.table.param(p).name).collect())
            .unwrap_or_default()
    }

    /// Recipe of a type annotation of the code (`x is List<T>`), resolved
    /// through the scope: type variables of the function and of the class, the scope of the
    /// library (with a prefix). `None` when the name does not resolve.
    pub fn annotation_recipe(&self, a: &ast::TypeAnnotation) -> Option<Recipe> {
        let mut r = Recipe { text: String::new(), variables: false };
        self.write_annotation(a, &mut Vec::new(), &mut r)?;
        Some(r)
    }

    /// Reified arguments of an annotated type, for the ABI of a redirecting
    /// factory. The recipe uses the environment of the source factory:
    /// `Impl<T>` needs to read `T` from the tuple it received.
    pub fn annotation_tuple(&mut self, a: &ast::TypeAnnotation) -> Option<Operand> {
        let TypeKind::Named { args, .. } = &a.kind else { return None };
        if args.is_empty() { return None; }
        let args = args.clone();
        let mut r = Recipe { text: "L<".to_string(), variables: false };
        for (i, arg) in args.iter().enumerate() {
            if i > 0 { r.text.push(','); }
            let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
            let sub = self.annotation_recipe(unit_ast.ty(*arg))?;
            r.text.push_str(&sub.text);
            r.variables |= sub.variables;
        }
        r.text.push('>');
        Some(self.recipe_rti(&r))
    }

    /// Recipe of a type variable in scope, by name: of the function (and of the
    /// extension), then of the class. `None` when `sym` is none of them.
    pub fn type_variable_recipe(&self, sym: SymbolId) -> Option<Recipe> {
        let text = if let Some(i) = self.local_type_index(sym) {
            format!("M{i}")
        } else if let Some(i) = self.function_type_params.iter().position(|s| *s == sym) {
            format!("M{i}")
        } else {
            let i = self.class_params().iter().position(|s| *s == sym)?;
            format!("{}{i}", if self.class_by_tuple { 'M' } else { 'P' })
        };
        Some(Recipe { text, variables: true })
    }

    fn write_annotation(&self, a: &ast::TypeAnnotation, linked_set: &mut Vec<SymbolId>, r: &mut Recipe) -> Option<()> {
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        match &a.kind {
            TypeKind::Void => r.text.push('V'),
            TypeKind::Named { name, args } => {
                let last_one = name.last()?;
                let name_str = self.ctx.symbol_name(last_one.sym);
                let simple = name.len() == 1;
                if simple && let Some(i) = linked_set.iter().rposition(|s| *s == last_one.sym) {
                    r.text.push_str(&format!("B{i}"));
                } else if simple && let Some(i) = self.local_type_index(last_one.sym) {
                    r.text.push_str(&format!("M{i}"));
                    r.variables = true;
                } else if simple && let Some(i) = self.function_type_params.iter().position(|s| *s == last_one.sym) {
                    r.text.push_str(&format!("M{i}"));
                    r.variables = true;
                } else if simple && let Some(i) = self.class_params().iter().position(|s| *s == last_one.sym) {
                    r.text.push_str(&format!("{}{i}", if self.class_by_tuple { 'M' } else { 'P' }));
                    r.variables = true;
                } else if simple && name_str == "dynamic" {
                    r.text.push('D');
                } else if simple && name_str == "Never" {
                    r.text.push('N');
                } else if simple && name_str == "Null" {
                    r.text.push('U');
                } else if simple && name_str == "FutureOr" {
                    r.text.push_str("O<");
                    match args.first() {
                        Some(x) => self.write_annotation(unit_ast.ty(*x), linked_set, r)?,
                        None => r.text.push('D'),
                    }
                    r.text.push('>');
                } else {
                    let lib = self.ctx.program.unit(self.unit_id).library;
                    let b = match &name[..] {
                        [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
                        _ => self.ctx.program.lookup(lib, last_one.sym),
                    }?;
                    match b.getter? {
                        // Extension type: the recipe of the representation, with
                        // the written arguments (like a typedef).
                        Element::Class(c) if self.ctx.is_extension_type(c) => {
                            let this = *self.ctx.te.this.get(&c)?;
                            let params = self.ctx.outline.classes.get(c.0 as usize)?.type_params.clone();
                            let mut sub = Recipe { text: String::new(), variables: false };
                            let mut strings = Vec::new();
                            for i in 0..params.len() {
                                sub.text.clear();
                                match args.get(i) {
                                    Some(x) => self.write_annotation(unit_ast.ty(*x), linked_set, &mut sub)?,
                                    None => sub.text.push('D'),
                                }
                                r.variables |= sub.variables;
                                strings.push(sub.text.clone());
                            }
                            let mut alvo = self.type_recipe_with(self.ctx.erase(this), &params, &strings);
                            r.variables |= alvo.variables;
                            if a.nullable && !alvo.text.ends_with('?') {
                                alvo.text.push('?');
                            }
                            r.text.push_str(&alvo.text);
                            return Some(());
                        }
                        Element::Class(c) => {
                            r.text.push_str(&format!("C{}", self.ctx.id_rti(c)));
                            let n = self.ctx.outline.classes.get(c.0 as usize).map_or(0, |d| d.type_params.len());
                            if n > 0 {
                                r.text.push('<');
                                for i in 0..n {
                                    if i > 0 {
                                        r.text.push(',');
                                    }
                                    match args.get(i) {
                                        Some(x) => self.write_annotation(unit_ast.ty(*x), linked_set, r)?,
                                        None => r.text.push('D'),
                                    }
                                }
                                r.text.push('>');
                            }
                        }
                        Element::Typedef(td) => {
                            let data = self.ctx.outline.typedefs.get(td.0 as usize)?;
                            // The arguments of the typedef enter through the recipe of
                            // each one (sub-recipes written as text).
                            let mut sub = Recipe { text: String::new(), variables: false };
                            let mut strings = Vec::new();
                            for x in args.iter() {
                                sub.text.clear();
                                self.write_annotation(unit_ast.ty(*x), linked_set, &mut sub)?;
                                strings.push(sub.text.clone());
                            }
                            r.variables |= sub.variables;
                            let mut alvo = self.type_recipe_with(data.target_type, &data.type_params, &strings);
                            r.variables |= alvo.variables;
                            if a.nullable {
                                alvo.text.push('?');
                            }
                            r.text.push_str(&alvo.text);
                            return Some(());
                        }
                        _ => return None,
                    }
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                let n = linked_set.len();
                linked_set.extend(type_params.iter().map(|p| p.name.sym));
                r.text.push_str(&format!("F<{};", type_params.len()));
                match return_type {
                    Some(x) => self.write_annotation(unit_ast.ty(*x), linked_set, r)?,
                    None => r.text.push('D'),
                }
                let oblig = parameters.iter().filter(|p| p.kind == ast::ParameterKind::Required).count();
                r.text.push_str(&format!(";{oblig};"));
                let mut first_one = true;
                for p in parameters.iter().filter(|p| p.kind != ast::ParameterKind::Named) {
                    if !first_one {
                        r.text.push(',');
                    }
                    first_one = false;
                    match p.ty {
                        Some(x) => self.write_annotation(unit_ast.ty(x), linked_set, r)?,
                        None => r.text.push('D'),
                    }
                }
                r.text.push(';');
                let mut first_one = true;
                for p in parameters.iter().filter(|p| p.kind == ast::ParameterKind::Named) {
                    if !first_one {
                        r.text.push(',');
                    }
                    first_one = false;
                    r.text.push_str(self.ctx.symbol_name(p.name?.sym));
                    r.text.push(':');
                    match p.ty {
                        Some(x) => self.write_annotation(unit_ast.ty(x), linked_set, r)?,
                        None => r.text.push('D'),
                    }
                    if p.required {
                        r.text.push('!');
                    }
                }
                r.text.push('>');
                linked_set.truncate(n);
            }
            TypeKind::Record { positional, named } => {
                r.text.push_str("R<");
                for (i, x) in positional.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_annotation(unit_ast.ty(*x), linked_set, r)?;
                }
                r.text.push(';');
                let mut fields: Vec<&(ast::Name, ast::TypeId)> = named.iter().collect();
                fields.sort_by_key(|(n, _)| self.ctx.symbol_name(n.sym).to_string());
                for (i, (n, x)) in fields.into_iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(n.sym));
                    r.text.push(':');
                    self.write_annotation(unit_ast.ty(*x), linked_set, r)?;
                }
                r.text.push('>');
            }
        }
        let is_nullable_by_itself = matches!(r.text.as_bytes().last(), Some(b'D' | b'V' | b'U'));
        if a.nullable && !is_nullable_by_itself {
            r.text.push('?');
        }
        Some(())
    }

    /// Recipe of `t` with the parameters `params` replaced by the recipes
    /// `args` (those of a typedef).
    pub(super) fn type_recipe_with(&self, t: TypeId, params: &[TypeParamId], args: &[String]) -> Recipe {
        // Writes with markers and replaces: the typedef parameters are neither of a
        // class nor of a function, so `write_type` would write them as `D`.
        let mut r = Recipe { text: String::new(), variables: false };
        self.write_typedef_type(t, params, args, &mut Vec::new(), &mut r);
        r
    }

    fn write_typedef_type(
        &self,
        t: TypeId,
        params: &[TypeParamId],
        args: &[String],
        linked_set: &mut Vec<TypeParamId>,
        r: &mut Recipe,
    ) {
        if let T::TypeParameter { param, nullable } = self.ctx.table.get(t)
            && let Some(i) = params.iter().position(|p| p == param)
        {
            r.text.push_str(args.get(i).map_or("D", String::as_str));
            if *nullable {
                r.text.push('?');
            }
            return;
        }
        // The other cases: the normal recipe, descending through the arguments with the
        // same replacement (the compound types are rewritten piece by piece).
        match self.ctx.table.get(t) {
            T::Interface { class, args: a, nullable } => {
                r.text.push_str(&format!("C{}", self.ctx.id_rti(*class)));
                if !a.is_empty() {
                    r.text.push('<');
                    for (i, x) in a.iter().enumerate() {
                        if i > 0 {
                            r.text.push(',');
                        }
                        self.write_typedef_type(*x, params, args, linked_set, r);
                    }
                    r.text.push('>');
                }
                if *nullable {
                    r.text.push('?');
                }
            }
            T::Function { type_params, ret, positional, optional, named, nullable } => {
                let n = linked_set.len();
                linked_set.extend(type_params.iter().copied());
                r.text.push_str(&format!("F<{};", type_params.len()));
                self.write_typedef_type(*ret, params, args, linked_set, r);
                r.text.push_str(&format!(";{};", positional.len()));
                for (i, p) in positional.iter().chain(optional.iter()).enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_typedef_type(*p, params, args, linked_set, r);
                }
                r.text.push(';');
                for (i, (name_str, p, req)) in named.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(*name_str));
                    r.text.push(':');
                    self.write_typedef_type(*p, params, args, linked_set, r);
                    if *req {
                        r.text.push('!');
                    }
                }
                r.text.push('>');
                linked_set.truncate(n);
                if *nullable {
                    r.text.push('?');
                }
            }
            T::FutureOr { arg, nullable } => {
                r.text.push_str("O<");
                self.write_typedef_type(*arg, params, args, linked_set, r);
                r.text.push('>');
                if *nullable {
                    r.text.push('?');
                }
            }
            T::Record { positional, named, nullable } => {
                r.text.push_str("R<");
                for (i, p) in positional.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    self.write_typedef_type(*p, params, args, linked_set, r);
                }
                r.text.push(';');
                for (i, (name_str, p)) in named.iter().enumerate() {
                    if i > 0 {
                        r.text.push(',');
                    }
                    r.text.push_str(self.ctx.symbol_name(*name_str));
                    r.text.push(':');
                    self.write_typedef_type(*p, params, args, linked_set, r);
                }
                r.text.push('>');
                if *nullable {
                    r.text.push('?');
                }
            }
            _ => self.write_type(t, linked_set, &HashMap::new(), r),
        }
    }

    /// The type of the recipe, as `I64`, in the current environment.
    pub fn recipe_rti(&mut self, r: &Recipe) -> Operand {
        let h = recipe_hash(&r.text);
        // The same recipe may appear in the program and in several libraries of the
        // SDK. Each library has its own lazy cache; distinct
        // names avoid multiple definitions in the ThinLTO link.
        let lib = self.ctx.program.unit(self.unit_id).library;
        let owner = crate::context::escape_str(&self.ctx.library_name(lib));
        let getter = format!("df.rti.{h}.{owner}");
        let global = format!("dfr.{h}.{owner}");
        if !self.done_entries.contains(&getter) {
            self.done_entries.insert(getter.clone());
            let mut g = FnBuilder::new(self.ctx, self.unit_id, getter.clone(), "rti".to_string(), Type::I64);
            let ok = g.emit(
                Instruction::LoadGlobal {
                    symbol: format!("{global}$ok"),
                    ty: Type::I8,
                },
                Type::I8,
            );
            let ready = g.emit(Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))), Type::I1);
            let b_ready = g.new_block();
            let b_read = g.new_block();
            g.terminate(Terminator::CondBranch {
                cond: ready,
                then_block: b_ready,
                else_block: b_read,
            });
            g.set_block(b_ready);
            let v = g.emit(
                Instruction::LoadGlobal {
                    symbol: global.clone(),
                    ty: Type::I64,
                },
                Type::I64,
            );
            g.terminate(Terminator::Return(Some(v)));
            g.set_block(b_read);
            let s = g.emit(Instruction::Const(Constant::String(r.text.clone())), Type::Ref);
            let t = g.emit(
                Instruction::CallRuntime {
                    name: "dartforge_rti_recipe".to_string(),
                    args: vec![(s, Type::Ref)],
                    ret_ty: Type::I64,
                },
                Type::I64,
            );
            g.emit(
                Instruction::StoreGlobal {
                    symbol: global.clone(),
                    val: t.clone(),
                    ty: Type::I64,
                    root: None,
                },
                Type::Void,
            );
            g.emit(
                Instruction::StoreGlobal {
                    symbol: format!("{global}$ok"),
                    val: Operand::Constant(Constant::Int(1)),
                    ty: Type::I8,
                    root: None,
                },
                Type::Void,
            );
            g.terminate(Terminator::Return(Some(t)));
            if self.ctx.arc_memory { self.arc_rti_caches.insert(global.clone()); }
            self.extra_globals.push((0, Type::I64, global));
            self.absorb(g);
        }
        let base = self.emit(
            Instruction::CallStatic {
                symbol: getter,
                args: Vec::new(),
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        if !r.variables {
            return base;
        }
        let this = if self.class_by_tuple {
            Operand::Constant(Constant::Null)
        } else {
            self.this_param.clone().map_or(Operand::Constant(Constant::Null), |t| self.coerce(t, Type::Ref))
        };
        let class = self.enclosing_class.map_or(0, |c| self.ctx.id_rti(c));
        let tuple = self.type_tuple.clone().unwrap_or(Operand::Constant(Constant::Int(0)));
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_eval".to_string(),
                args: vec![
                    (base, Type::I64),
                    (this, Type::Ref),
                    (Operand::Constant(Constant::Int(class)), Type::I64),
                    (tuple, Type::I64),
                ],
                ret_ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// The type `t` from `crates/types`, as `I64`, in the current environment.
    pub fn type_rti(&mut self, t: TypeId) -> Operand {
        let r = self.type_recipe(t);
        self.recipe_rti(&r)
    }

    /// Stores the runtime type of an object (`Ref`).
    pub fn define_rti(&mut self, obj: Operand, ty: Operand) {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_set".to_string(),
                args: vec![(obj, Type::Ref), (ty, Type::I64)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }

    /// Stores in `obj` the static type `t` when it says something that the class
    /// does not say (it has type arguments, or it is a function type).
    pub fn define_rti_if_generic(&mut self, obj: Operand, t: TypeId) {
        let useful = match self.ctx.table.get(t) {
            T::Interface { args, .. } => args.iter().any(|a| *a != self.ctx.core.dynamic_),
            T::Function { .. } => true,
            _ => false,
        };
        if useful {
            let ty = self.type_rti(t);
            self.define_rti(obj, ty);
        }
    }

    /// The collection literal `l` with the static type of the expression (when the
    /// literal did not terminate the block).
    pub fn literal_rti(&mut self, l: Operand, expr: ast::ExprId) -> Operand {
        if !self.is_terminated()
            && self.operand_type(&l) == Type::Ref
            && let Some(t) = self.ctx.get_type(self.unit_id, expr)
        {
            self.define_rti_if_generic(l.clone(), t);
        }
        l
    }

    /// `op is <ty>` (I1) pelo RTI.
    pub fn test_rti(&mut self, op: Operand, ty: Operand) -> Operand {
        let v = self.coerce(op, Type::Ref);
        let r = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_e".to_string(),
                args: vec![(v, Type::Ref), (ty, Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        self.emit(Instruction::ICmp(ICmpOp::Ne, r, Operand::Constant(Constant::Int(0))), Type::I1)
    }

    /// `op as <ty>` through the RTI: a pending `TypeError` if it fails.
    pub fn cast_rti(&mut self, op: Operand, ty: Operand) {
        self.cast_rti_in(op, ty, CastContext::How);
    }

    /// [`Self::cast_rti`] with the context that gives the end of the VM's message.
    pub fn cast_rti_in(&mut self, op: Operand, ty: Operand, context: CastContext) {
        let v = self.coerce(op, Type::Ref);
        // The name of the parameter only enters the message of the `TypeError`: the
        // check goes first through `is` (`dartforge_rti_e`, the same test
        // as `cast_rti_in`), and the text of the name is only built on the
        // failure path. Before, every covariant or dynamic entry (the `[]=` of
        // `_List`, the adapters of the method table) paid a
        // `dartforge_string_new` per parameter on each call.
        let (code, name_str, end) = match context {
            CastContext::How => (0, Operand::Constant(Constant::Null), None),
            CastContext::Implicit => (1, Operand::Constant(Constant::Null), None),
            CastContext::Parameter(n) => {
                let ok = self.test_rti(v.clone(), ty.clone());
                let b_fail = self.new_block();
                let b_end = self.new_block();
                self.terminate(Terminator::CondBranch {
                    cond: ok,
                    then_block: b_end,
                    else_block: b_fail,
                });
                self.set_block(b_fail);
                (2, self.emit(Instruction::Const(Constant::String(n)), Type::Ref), Some(b_end))
            }
        };
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_rti_as_with_context".to_string(),
                args: vec![(v, Type::Ref), (ty, Type::I64), (Operand::Constant(Constant::Int(code)), Type::I64), (name_str, Type::Ref)],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        if let Some(b_end) = end {
            if !self.is_terminated() {
                self.terminate(Terminator::Branch(b_end));
            }
            self.set_block(b_end);
        }
    }

    /// At the entry of a dynamic call (tear-off, adapter of the method
    /// table), checks each argument against the type of the parameter before
    /// converting it, like the VM: "type 'S' is not a subtype of type 'T' of
    /// 'name'". Only the parameters of nominal (class) type and the class
    /// covariant ones; the others (`dynamic`, `Object?`, function types and
    /// type parameters of the function) keep the usual conversion.
    pub fn check_entry_arguments(&mut self, fid: usize, vals: &[Operand]) {
        self.check_entry_arguments_with(fid, vals, false);
    }

    /// [`Self::check_entry_arguments`]; at the typed entry
    /// (`only_covariants`, `typed_entry.rs`), only the covariant
    /// parameters: the static caller already guaranteed the others.
    pub fn check_entry_arguments_with(&mut self, fid: usize, vals: &[Operand], only_covariants: bool) {
        for (i, (p, v)) in self.ctx.outline.functions[fid].parameters.clone().iter().zip(vals).enumerate() {
            if !super::typed_entry::checked_at_entry(self.ctx, fid, p.ty) {
                continue;
            }
            if only_covariants && !super::typed_entry::covariant_parameter(self.ctx, fid, i) {
                continue;
            }
            let name_str = p.name.map(|n| self.ctx.symbol_name(n).to_string()).unwrap_or_default();
            let ty = self.type_rti(p.ty);
            self.cast_rti_in(v.clone(), ty, CastContext::Parameter(name_str));
        }
    }

    /// Builds the type argument tuple of the call `expr` to the function `fid`
    /// (if it is generic): the written ones (`f<int>(…)`), or the inferred ones,
    /// matching the declared return with the static type of the call and each
    /// parameter with the type of the argument. What cannot be deduced stays `dynamic`.
    pub fn arm_tuple(&mut self, fid: usize, expr: ast::ExprId, arguments: &ast::Arguments) {
        self.arm_tuple_with_receiver(fid, None, Some((expr, arguments)));
    }

    /// `arm_tuple` of an extension member: the type arguments of the
    /// extension come from the static type of the receiver matched with the `on` (as
    /// in the inference of the extension application), those of the member from the call.
    /// Without `call` (getter, setter, operator), only the extension part.
    pub fn arm_tuple_with_receiver(
        &mut self,
        fid: usize,
        receiver: Option<TypeId>,
        call: Option<(ast::ExprId, &ast::Arguments)>,
    ) {
        if !self.generic_function(fid) {
            return;
        }
        let params = self.type_params_of(fid);
        let extension = self.receiver_pattern(fid);
        let n_ext = extension.map_or(0, |(n, _)| n);
        let mut found_items: Vec<Option<TypeId>> = vec![None; params.len()];
        if let (Some((_, on)), Some(r)) = (extension, receiver) {
            self.unify(on, r, &params, &mut found_items);
        }
        let data = &self.ctx.outline.functions[fid];
        let mut written_set: Vec<Recipe> = Vec::new();
        if let Some((expr, arguments)) = call {
            let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
            if !arguments.type_args.is_empty() {
                for a in &arguments.type_args {
                    let x = self.annotation_recipe(unit_ast.ty(*a));
                    written_set.push(x.unwrap_or(Recipe { text: "D".to_string(), variables: false }));
                }
            } else {
                if let Some(real) = self.ctx.get_type_raw(self.unit_id, expr) {
                    self.unify(data.return_type, real, &params, &mut found_items);
                }
                let mut positionals = arguments.args.iter().filter(|a| a.name.is_none());
                for p in data.parameters.iter() {
                    let arg = if p.kind == ast::ParameterKind::Named {
                        arguments.args.iter().find(|a| a.name.map(|n| n.sym) == p.name)
                    } else {
                        positionals.next()
                    };
                    if let Some(a) = arg
                        && let Some(real) = self.ctx.get_type_raw(self.unit_id, a.value)
                    {
                        self.unify(p.ty, real, &params, &mut found_items);
                    }
                }
            }
        }
        let mut r = Recipe { text: "L<".to_string(), variables: false };
        for (i, found) in found_items.iter().enumerate() {
            if i > 0 {
                r.text.push(',');
            }
            let x = match written_set.get(i.wrapping_sub(n_ext)).filter(|_| i >= n_ext) {
                Some(e) => e.clone(),
                None => self.type_recipe(found.unwrap_or(self.ctx.core.dynamic_)),
            };
            r.text.push_str(&x.text);
            r.variables |= x.variables;
        }
        r.text.push('>');
        let t = self.recipe_rti(&r);
        self.armed_tuple = Some(t);
    }

    /// Matches the declared type `decl` (with the parameters `params`) with the
    /// real type `real`, storing in `found_items` what each parameter is worth.
    /// If `C<args>` is a `Future`, the argument of the supertype `Future<X>`
    /// (`Some(None)` when `X` cannot be written with the `args` alone); `None`
    /// when it is not a `Future`.
    fn future_argument(&self, class: dartforge_elements::model::ClassId, args: &[TypeId]) -> Option<Option<TypeId>> {
        let future = self.ctx.core.future_class?;
        if class == future {
            return Some(args.first().copied());
        }
        let data = self.ctx.outline.hierarchy.get(class)?;
        let model = *data.supertypes.get(&future)?;
        let T::Interface { args: margs, .. } = self.ctx.table.get(model) else { return Some(None) };
        let Some(&x) = margs.first() else { return Some(None) };
        Some(match self.ctx.table.get(x) {
            T::TypeParameter { param, .. } => data.type_params.iter().position(|p| p == param).and_then(|i| args.get(i).copied()),
            _ if !self.contains_variable(x) => Some(x),
            _ => None,
        })
    }

    /// The type mentions some type parameter.
    fn contains_variable(&self, t: TypeId) -> bool {
        match self.ctx.table.get(t) {
            T::TypeParameter { .. } | T::Intersection { .. } => true,
            T::Interface { args, .. } => args.iter().any(|a| self.contains_variable(*a)),
            T::FutureOr { arg, .. } => self.contains_variable(*arg),
            T::Function { .. } | T::Record { .. } => true,
            _ => false,
        }
    }

    pub(super) fn unify(&self, decl: TypeId, real: TypeId, params: &[TypeParamId], found_items: &mut [Option<TypeId>]) {
        if real == self.ctx.core.dynamic_ {
            return;
        }
        // Extension types: by the arguments when both are of the same one
        // (or the real one implements that of the pattern); otherwise by the erased ones.
        match (self.ctx.table.get(decl), self.ctx.table.get(real)) {
            (T::ExtensionType { decl: d1, args: a1, .. }, T::ExtensionType { decl: d2, args: a2, .. }) => {
                if d1 == d2 {
                    for (x, y) in a1.iter().zip(a2.iter()) {
                        self.unify(*x, *y, params, found_items);
                    }
                } else if let Some(&s) = self.ctx.te.supertypes.get(&(real, *d1)) {
                    self.unify(decl, s, params, found_items);
                }
                return;
            }
            (T::ExtensionType { .. }, _) | (_, T::ExtensionType { .. }) => {
                let (a, b) = (self.ctx.erase(decl), self.ctx.erase(real));
                if (a, b) != (decl, real) {
                    self.unify(a, b, params, found_items);
                }
                return;
            }
            _ => {}
        }
        match (self.ctx.table.get(decl), self.ctx.table.get(real)) {
            (T::TypeParameter { param, nullable }, _) => {
                // `X?` matched with `T?` gives `X = T` (inference removes the `?`
                // that the pattern already has): the `nonNulls` of a `List<T?>` is
                // `Iterable<T>`. Without the non-nullable form in the table (read
                // only here), `T?` stays.
                let real = if *nullable { self.without_question_mark(real).unwrap_or(real) } else { real };
                if let Some(i) = params.iter().position(|p| p == param)
                    && found_items[i].is_none()
                {
                    found_items[i] = Some(real);
                }
            }
            (T::Interface { class: c1, args: a1, .. }, T::Interface { class: c2, args: a2, .. }) if c1 == c2 => {
                for (x, y) in a1.iter().zip(a2.iter()) {
                    self.unify(*x, *y, params, found_items);
                }
            }
            // Different classes: the pattern matches the supertype of the real one
            // (`Iterable<X>` against `List<int>` is `Iterable<int>`), as
            // inference does through the interface of the real type.
            (T::Interface { class: c1, args: a1, .. }, T::Interface { class: c2, args: a2, .. }) => {
                let Some(data) = self.ctx.outline.hierarchy.get(*c2) else { return };
                let Some(&model) = data.supertypes.get(c1) else { return };
                let T::Interface { args: margs, .. } = self.ctx.table.get(model) else { return };
                for (x, m) in a1.iter().zip(margs.iter()) {
                    self.unify_by_model(*x, *m, &data.type_params, a2, params, found_items);
                }
            }
            (T::FutureOr { arg, .. }, T::FutureOr { arg: b, .. }) => self.unify(*arg, *b, params, found_items),
            // `FutureOr<X>` against a `Future` matches `X` with the argument of the
            // `Future` (inference tries `Future<X>` first): also
            // for a subtype (`_Future<T>` implements `Future<T>`).
            (T::FutureOr { arg, .. }, T::Interface { class, args, .. }) => {
                match self.future_argument(*class, args) {
                    Some(Some(x)) => self.unify(*arg, x, params, found_items),
                    // It is a `Future`, but the argument cannot be deduced: it stays free.
                    Some(None) => {}
                    None => self.unify(*arg, real, params, found_items),
                }
            }
            (
                T::Function { ret: r1, positional: p1, .. },
                T::Function { ret: r2, positional: p2, .. },
            ) => {
                self.unify(*r1, *r2, params, found_items);
                for (x, y) in p1.iter().zip(p2.iter()) {
                    self.unify(*x, *y, params, found_items);
                }
            }
            // Records: field by field (`extension E<A, B> on (P<A>, P<B>)`
            // against `(P<String>, P<int>)`; petitparser's
            // `toSequenceParser`), positional by position and named by name.
            (
                T::Record { positional: p1, named: n1, .. },
                T::Record { positional: p2, named: n2, .. },
            ) if p1.len() == p2.len() && n1.len() == n2.len() => {
                for (x, y) in p1.iter().zip(p2.iter()) {
                    self.unify(*x, *y, params, found_items);
                }
                for (name_str, x) in n1.iter() {
                    if let Some((_, y)) = n2.iter().find(|(m, _)| m == name_str) {
                        self.unify(*x, *y, params, found_items);
                    }
                }
            }
            _ => {}
        }
    }

    /// [`Self::unify`] of the pattern `decl` with the argument `model` of a
    /// supertype, written in the parameters `formals` of the real class, whose
    /// arguments are `real_set`: where the model is a formal, the real one is the
    /// corresponding argument; a model without variables matches as it is.
    fn unify_by_model(
        &self,
        decl: TypeId,
        model: TypeId,
        formals: &[TypeParamId],
        real_set: &[TypeId],
        params: &[TypeParamId],
        found_items: &mut [Option<TypeId>],
    ) {
        match (self.ctx.table.get(decl), self.ctx.table.get(model)) {
            (_, T::TypeParameter { param, nullable: false }) => {
                if let Some(&r) = formals.iter().position(|f| f == param).and_then(|k| real_set.get(k)) {
                    self.unify(decl, r, params, found_items);
                }
            }
            (T::Interface { class: c1, args: a1, .. }, T::Interface { class: c2, args: m2, .. }) if c1 == c2 => {
                for (x, m) in a1.iter().zip(m2.iter()) {
                    self.unify_by_model(*x, *m, formals, real_set, params, found_items);
                }
            }
            _ if !self.contains_variable(model) => self.unify(decl, model, params, found_items),
            _ => {}
        }
    }

    /// The type of the object and the tuple of the generic class `c` of a
    /// constructor tear-off whose static type (`ty`) is already instantiated by
    /// inference (`C<int> Function()`, without type parameters of its own);
    /// `None` for a non-generic class or a generic tear-off.
    pub fn tearoff_instantiation(&mut self, c: ClassId, ty: TypeId) -> Option<(Operand, Operand)> {
        let n = self.ctx.outline.classes.get(c.0 as usize)?.type_params.len();
        if n == 0 {
            return None;
        }
        let T::Function { type_params, ret, .. } = self.ctx.table.get(ty) else { return None };
        if !type_params.is_empty() {
            return None;
        }
        let T::Interface { class, args, .. } = self.ctx.table.get(*ret) else { return None };
        if *class != c || args.len() != n {
            return None;
        }
        let args = args.clone();
        let mut strings = Vec::with_capacity(n);
        let mut variables = false;
        for a in args.iter() {
            let r = self.type_recipe(*a);
            variables |= r.variables;
            strings.push(r.text);
        }
        let object = self.recipe_rti(&Recipe { text: format!("C{}<{}>", self.ctx.id_rti(c), strings.join(",")), variables });
        let tuple = self.recipe_rti(&Recipe { text: format!("L<{}>", strings.join(",")), variables });
        Some((object, tuple))
    }

    /// The non-nullable form of `t` (`T?` → `T`, `C<A>?` → `C<A>`), if the
    /// table has it.
    fn without_question_mark(&self, t: TypeId) -> Option<TypeId> {
        let ty = match self.ctx.table.get(t).clone() {
            T::TypeParameter { param, nullable: true } => T::TypeParameter { param, nullable: false },
            T::Interface { class, args, nullable: true } => T::Interface { class, args, nullable: false },
            T::FutureOr { arg, nullable: true } => T::FutureOr { arg, nullable: false },
            T::Record { positional, named, nullable: true } => T::Record { positional, named, nullable: false },
            T::ExtensionType { decl, args, nullable: true } => T::ExtensionType { decl, args, nullable: false },
            T::Function { type_params, ret, positional, optional, named, nullable: true } => {
                T::Function { type_params, ret, positional, optional, named, nullable: false }
            }
            _ => return None,
        };
        self.ctx.table.intern_lookup(ty)
    }

    /// Stores the signature of a closure (`F<…>`): the static type of the
    /// function expression, if inference has it, otherwise what the declaration
    /// writes (parameters without a type are `dynamic`). It is dart2js's `$signature`:
    /// what `f is void Function(Object, StackTrace)` asks.
    /// The type of a function literal (`expr`) or a named local: the runtime one
    /// of a generator without a written return (the CFE rule,
    /// `tipos_de_execucao_de_funcoes`), otherwise the inferred one — that of the expression, or
    /// that of the local, whose return without a written type comes from the body (`g() { return
    /// 1; }` is `() => int`, not `() => dynamic`).
    pub fn literal_function_type(&self, ast: &ast::Ast, fid: ast::FunctionId, expr: Option<ast::ExprId>) -> Option<dartforge_types::table::TypeId> {
        let execution = self.ctx.bodies.units.get(self.unit_id.0 as usize).and_then(|u| u.function_runtime_type(fid));
        execution.or_else(|| match expr {
            Some(e) => self.ctx.get_type(self.unit_id, e),
            None => ast.function(fid).name.and_then(|n| self.ctx.local_type(self.unit_id, n.span.start as usize)),
        })
    }

    pub fn define_closure_rti(&mut self, clo: Operand, ast: &ast::Ast, fid: ast::FunctionId, expr: Option<ast::ExprId>) {
        // The inferred type: that of the expression (function literal) or that of the local
        // (named local function, whose return without a written type is inferred from the
        // body — `g() { return 1; }` is `() => int`, not `() => dynamic`).
        if let Some(t) = self.literal_function_type(ast, fid, expr)
            && matches!(self.ctx.table.get(t), T::Function { .. })
        {
            let r = self.type_recipe(t);
            let ty = self.recipe_rti(&r);
            self.define_rti(clo, ty);
            return;
        }
        let f = ast.function(fid);
        let params = f.parameters.as_deref().unwrap_or(&[]);
        let mut r = Recipe { text: format!("F<{};", f.type_params.len()), variables: false };
        let mut linked_set: Vec<SymbolId> = f.type_params.iter().map(|p| p.name.sym).collect();
        let type_of = |s: &Self, t: Option<ast::TypeId>, r: &mut Recipe, linked_set: &mut Vec<SymbolId>| match t {
            Some(x) => {
                let mut sub = Recipe { text: String::new(), variables: false };
                if s.write_annotation(ast.ty(x), linked_set, &mut sub).is_some() {
                    r.text.push_str(&sub.text);
                    r.variables |= sub.variables;
                } else {
                    r.text.push('D');
                }
            }
            None => r.text.push('D'),
        };
        type_of(self, f.return_type, &mut r, &mut linked_set);
        let oblig = params.iter().filter(|p| p.kind == ast::ParameterKind::Required).count();
        r.text.push_str(&format!(";{oblig};"));
        let mut first_one = true;
        for p in params.iter().filter(|p| p.kind != ast::ParameterKind::Named) {
            if !first_one {
                r.text.push(',');
            }
            first_one = false;
            type_of(self, p.ty, &mut r, &mut linked_set);
        }
        r.text.push(';');
        let mut first_one = true;
        for p in params.iter().filter(|p| p.kind == ast::ParameterKind::Named) {
            let Some(n) = p.name else { continue };
            if !first_one {
                r.text.push(',');
            }
            first_one = false;
            r.text.push_str(self.ctx.symbol_name(n.sym));
            r.text.push(':');
            type_of(self, p.ty, &mut r, &mut linked_set);
            if p.required {
                r.text.push('!');
            }
        }
        r.text.push('>');
        let ty = self.recipe_rti(&r);
        self.define_rti(clo, ty);
    }

    /// Stores the signature of the tear-off of the function `fid` (that of the outline; in a
    /// method, evaluated in the type of the receiver `recv`).
    pub fn define_tearoff_rti(&mut self, clo: Operand, fid: usize, recv: Option<Operand>) {
        self.define_tearoff_rti_with_tuple(clo, fid, recv, None);
    }

    /// [`Self::define_tearoff_rti`] of a generic extension member or extension
    /// type member: the type parameters of the owner come from `tuple`
    /// (that of `arm_tuple_with_receiver`, computed here), not from an object.
    pub fn define_tearoff_rti_with_tuple(&mut self, clo: Operand, fid: usize, recv: Option<Operand>, tuple: Option<Operand>) {
        let Some(sig) = self.ctx.outline.functions.get(fid).map(|d| d.signature) else {
            return;
        };
        let saved = (self.this_param.clone(), self.finalizable_this, self.enclosing_class, self.class_by_tuple, self.type_tuple.clone());
        if let Some(r) = recv {
            self.this_param = Some(r);
            self.finalizable_this = self.ctx.classify_this(fid);
            // An extension type has no object with the type arguments.
            self.enclosing_class = self.ctx.program.functions[fid].class.filter(|&c| !self.ctx.is_extension_type(c));
            self.class_by_tuple = false;
        }
        self.type_tuple = tuple;
        let r = self.type_recipe(sig);
        let ty = self.recipe_rti(&r);
        (self.this_param, self.finalizable_this, self.enclosing_class, self.class_by_tuple, self.type_tuple) = saved;
        self.define_rti(clo, ty);
    }

    /// Does the type test of the annotation need the RTI? (type arguments that
    /// are not trivial, a type variable, `FutureOr`, a function or
    /// record type, a typedef.)
    pub fn annotation_needs_rti(&self, a: &ast::TypeAnnotation) -> bool {
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        match &a.kind {
            TypeKind::Void => false,
            TypeKind::Function { .. } | TypeKind::Record { .. } => true,
            TypeKind::Named { name, args } => {
                let Some(last_one) = name.last() else { return false };
                let name_str = self.ctx.symbol_name(last_one.sym);
                if name.len() == 1
                    && (self.function_type_params.contains(&last_one.sym)
                        || self.local_type_index(last_one.sym).is_some()
                        || self.class_params().contains(&last_one.sym)
                        || name_str == "FutureOr")
                {
                    return true;
                }
                let lib = self.ctx.program.unit(self.unit_id).library;
                let b = match &name[..] {
                    [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
                    _ => self.ctx.program.lookup(lib, last_one.sym),
                };
                if matches!(b.and_then(|b| b.getter), Some(Element::Typedef(_))) {
                    return true;
                }
                // Extension type: the test is that of the representation (without its own
                // class in the heap).
                if let Some(Element::Class(c)) = b.and_then(|b| b.getter)
                    && self.ctx.is_extension_type(c)
                {
                    return true;
                }
                let trivial = |t: &ast::TypeAnnotation| match &t.kind {
                    TypeKind::Named { name, args } if args.is_empty() => name.last().is_some_and(|n| {
                        let s = self.ctx.symbol_name(n.sym);
                        s == "dynamic" || (s == "Object" && t.nullable)
                    }),
                    _ => false,
                };
                !args.iter().all(|x| trivial(unit_ast.ty(*x)))
            }
        }
    }
}

/// Registration of the universe at the program entry: name and number of parameters
/// of each class cited by the recipes (and by the classes of the program), the
/// supertype rules (the closure: the supertypes of the cited ones too), the
/// runtime shapes. Empty when the program uses no recipe at all.
pub fn register_universe(ctx: &Context, module: &mut Module) {
    // The universe is global to the executable; the SDK objects only supply the
    // recipes they use. The program entry registers the complete universe.
    if module.sdk_library {
        return;
    }
    let mut recipes: Vec<String> = Vec::new();
    let mut uses_rti = false;
    for f in &module.functions {
        let is_recipe = f.symbol.starts_with("df.rti.");
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                match inst {
                    Instruction::Const(Constant::String(s)) if is_recipe => recipes.push(s.clone()),
                    Instruction::CallRuntime { name, .. } if name.starts_with("dartforge_rti_") => uses_rti = true,
                    _ => {}
                }
            }
        }
    }
    if recipes.is_empty() && !uses_rti && !module.sdk_mode {
        return;
    }
    // The `Type` object (`dartforge_rti_object_type`): the class and the selectors
    // used by the source SDK. Each type has a single canonical object.
    {
        let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
            return;
        };
        let symbol = "df.$type.toString".to_string();
        let mut t = FnBuilder::new(ctx, u, symbol.clone(), "toString".to_string(), Type::Ref);
        let obj = Operand::Val(t.add_param("this".to_string(), Type::Ref));
        // Field 0 of `_Type` is the RTI ID, a scalar (`dartforge_rti_object_type`).
        // Under ARC, the read checks the real map of the field; the text gets an owner
        // in the ARC preparation (`optimize/arc/natives.rs`), not here.
        let read = if ctx.arc_memory { "dartforge_arc_read_field_scalar_v1" } else { "dartforge_object_get" };
        let id = t.emit(
            Instruction::CallRuntime {
                name: read.to_string(),
                args: vec![(obj, Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let s = t.emit(
            Instruction::CallRuntime {
                name: "dartforge_rti_string".to_string(),
                args: vec![(id, Type::I64)],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        );
        t.terminate(Terminator::Return(Some(s)));
        t.finish(module);
        // Synthesized body: the Dart convention (Ref Owned return) does not come from
        // `lower_function`; the uniform caller `toString$c` requires it.
        if ctx.arc_memory {
            module.dart_ref_returns.insert(symbol.clone());
        }
        let mut equality = FnBuilder::new(ctx, u, "df.$type.$3d$3d$c".to_string(), "==".to_string(), Type::Ref);
        let recv = Operand::Val(equality.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(equality.add_param_vector(DartEntryVector::Arguments));
        equality.add_param_vector(DartEntryVector::Descriptor);
        equality.declare_arc_uniform_return();
        let other = equality.emit(
            Instruction::LoadIndexed { base: args, index: Operand::Constant(Constant::Int(0)) },
            Type::Ref,
        );
        let equal = equality.emit(Instruction::ICmp(ICmpOp::Eq, recv, other), Type::I1);
        let equal = equality.coerce(equal, Type::Ref);
        equality.terminate(Terminator::Return(Some(equal)));
        equality.finish(module);
        let mut text = FnBuilder::new(ctx, u, "df.$type.toString$c".to_string(), "toString".to_string(), Type::Ref);
        let recv = Operand::Val(text.add_param("this".to_string(), Type::Ref));
        text.add_param_vector(DartEntryVector::Arguments);
        text.add_param_vector(DartEntryVector::Descriptor);
        text.declare_arc_uniform_return();
        let value = text.emit_call_with_check(
            Instruction::CallStatic { symbol: symbol.clone(), args: vec![recv], ret_ty: Type::Ref },
            Type::Ref,
        );
        text.terminate(Terminator::Return(Some(value)));
        text.finish(module);
        let mut methods = vec![
            ("c:==".to_string(), "df.$type.$3d$3d$c".to_string()),
            ("c:toString".to_string(), "df.$type.toString$c".to_string()),
        ];
        // `_Type` is created by the runtime, outside the element hierarchy of the
        // program. It still inherits the members of `Object`, including the
        // private getter used by `identityHashCode(Object)` in hashSeed.
        if let Some(object) = ctx.core.object_class {
            for (selector, symbol) in super::sdk_source::method_table(ctx, object) {
                if !methods.iter().any(|(existing, _)| *existing == selector) {
                    methods.push((selector, symbol));
                }
            }
        }
        module.method_tables.push((CLASSE_TIPO as u32, "df.mt.$type".to_string(), methods));
        module.classes.push(ClassDef {
            id: CLASSE_TIPO as u32,
            name: "_Type".to_string(),
            field_count: 1,
            vtable: Vec::new(),
            to_string_symbol: Some(symbol),
        });
        // The `Type` object created in the runtime has no ClassElement of the
        // program. The RTI knows its type, but the nominal tests of the source
        // use the separate table of `dartforge_is_subclass`.
        if let Some(ty) = core_class(ctx, "Type").and_then(|c| ctx.class_id_of(c)) {
            module.subtyping_edges.push((CLASSE_TIPO as u32, ty));
        }
    }
    let mut cited: BTreeSet<i64> = BTreeSet::new();
    for r in &recipes {
        let b = r.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'C' && i + 1 < b.len() && (b[i + 1].is_ascii_digit() || b[i + 1] == b'-') {
                let mut j = i + 1;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'-') {
                    j += 1;
                }
                if let Ok(n) = r[i + 1..j].parse::<i64>() {
                    cited.insert(n);
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
    // The runtime shapes.
    let core = ctx.core;
    let shapes: Vec<(i64, Option<ClassId>)> = vec![
        (0, core.int_class),
        (1, core.double_class),
        (2, core.bool_class),
        (3, core.string_class),
        (4, core.list_class),
        (5, core.map_class),
        (6, core.set_class),
        (7, core.function_class),
        (8, core.record_class),
        (9, core.future_class),
        (10, core.object_class),
        (14, core_class(ctx, "Type")),
    ];
    let mut classes: Vec<ClassId> = Vec::new();
    let mut seen_set: std::collections::HashSet<ClassId> = std::collections::HashSet::new();
    let mut queue: Vec<ClassId> = Vec::new();
    for id in &cited {
        if let Some(c) = ctx.rti_id_class(*id) {
            queue.push(c);
        }
    }
    for (_, c) in &shapes {
        queue.extend(c.iter().copied());
    }
    for (i, _) in ctx.program.classes.iter().enumerate() {
        if ctx.class_id_of(ClassId(i as u32)).is_some() {
            queue.push(ClassId(i as u32));
        }
    }
    while let Some(c) = queue.pop() {
        if !seen_set.insert(c) {
            continue;
        }
        classes.push(c);
        if let Some(d) = ctx.outline.hierarchy.get(c) {
            for sup in d.supertypes.keys() {
                queue.push(*sup);
            }
        }
    }
    classes.sort_by_key(|c| ctx.id_rti(*c));
    let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else {
        return;
    };
    // The registrations go into a constant TABLE that the runtime reads at once
    // (`dartforge_rti_init_table`), not into code: there are thousands of
    // SDK classes, and one call per registration gave a function of tens
    // of thousands of instructions that LLVM compiled on every `dartforge run`,
    // every JIT reload and every AOT link. One line per registration:
    // `C<id> <parameters> <name>`, `R<id> <recipe>`, `F<shape> <id>`.
    let mut table = String::new();
    let mut b = FnBuilder::new(ctx, u, "dartforge_rti_init".to_string(), "rti".to_string(), Type::Void);
    for &c in &classes {
        let id = ctx.id_rti(c);
        let name_str = visible_name(ctx, c);
        let n = ctx.outline.classes.get(c.0 as usize).map_or(0, |d| d.type_params.len());
        table.push_str(&format!("C{id} {n} {name_str}\n"));
        // The library that declares the class and the declared name, for the
        // `reflectClass` of `dart:mirrors` (`runtime/src/mirrors.rs`).
        let class = &ctx.program.classes[c.0 as usize];
        let uri = ctx.program.library(class.library).uri.replace(' ', "%20");
        table.push_str(&format!("B{id} {uri} {}\n", ctx.symbol_name(class.name)));
        let Some(d) = ctx.outline.hierarchy.get(c) else { continue };
        let mut sups: Vec<(i64, TypeId)> = d.supertypes.iter().map(|(k, t)| (ctx.id_rti(*k), *t)).collect();
        sups.sort_by_key(|x| x.0);
        for (_, t) in sups {
            // The rule is written in the parameters of the class itself (`P<i>`).
            let saved = (b.enclosing_class, b.class_by_tuple);
            b.enclosing_class = Some(c);
            b.class_by_tuple = false;
            let r = b.type_recipe(t);
            b.enclosing_class = saved.0;
            b.class_by_tuple = saved.1;
            table.push_str(&format!("R{id} {}\n", r.text));
        }
    }
    for (shape, c) in shapes {
        if let Some(c) = c {
            table.push_str(&format!("F{shape} {}\n", ctx.id_rti(c)));
        }
    }
    // The error classes that the runtime represents (ids 1000–1012).
    for (heap_id, name_str) in [
        (1000, "Exception"),
        (1001, "FormatException"),
        (1002, "StateError"),
        (1003, "ArgumentError"),
        (1004, "RangeError"),
        (1005, "UnsupportedError"),
        (1006, "StackTrace"),
        (1007, "Error"),
        (1008, "UnimplementedError"),
        (1009, "AssertionError"),
        (1010, "ConcurrentModificationError"),
        (1011, "TypeError"),
        (1012, "NoSuchMethodError"),
        (CLASSE_TIPO, "Type"),
    ] {
        let Some(core_lib) = ctx.program.core else { continue };
        let Some(sym) = ctx.interner.lookup(name_str) else { continue };
        let Some(Element::Class(c)) = ctx.program.lookup(core_lib, sym).and_then(|b| b.getter) else { continue };
        table.push_str(&format!("F{heap_id} {}\n", ctx.id_rti(c)));
    }
    // The large table goes compressed (`dartforge_runtime::rti_table`): 1.37
    // MB of text in new_sali/backend, ~260 KB compressed.
    let bytes = table.into_bytes();
    let bytes = if bytes.len() >= dartforge_runtime::tabela_rti::LIMIAR { dartforge_runtime::tabela_rti::comprimir(&bytes) } else { bytes };
    // The bytes packed into words (little-endian, like all the targets).
    let words: Vec<i64> = bytes
        .chunks(8)
        .map(|c| {
            let mut w = [0u8; 8];
            w[..c.len()].copy_from_slice(c);
            i64::from_le_bytes(w)
        })
        .collect();
    let data = b.emit(Instruction::ConstArray(words), Type::Ptr);
    b.emit(
        Instruction::CallRuntime {
            name: "dartforge_rti_init_table".to_string(),
            args: vec![(data, Type::Ptr), (Operand::Constant(Constant::Int(bytes.len() as i64)), Type::I64)],
            ret_ty: Type::Void,
        },
        Type::Void,
    );
    b.terminate(Terminator::Return(None));
    b.finish(module);
    module.start_rti = Some("dartforge_rti_init".to_string());
}

/// The name of a class in `Type.toString()`: the VM's `Class::UserVisibleName`
/// replaces the internal implementations of `dart:core` and of
/// `dart:typed_data` by the public interface (`_Smi` → `int`,
/// `_GrowableList` → `List`, `_Uint8List` → `Uint8List`…); the others —
/// including the views (`_Uint8ArrayView`) and `_Map` — keep the declared
/// name. Outside the predefined ids, the VM only replaces the `_Future` of
/// `dart:async` by `Future` (the same `Class::GenerateUserVisibleName`
/// compares the name with `Symbols::_Future()` and the library with
/// `Library::AsyncLibrary()`); `_SyncCompleter`, `_ControllerStream`… keep
/// the declared name.
pub fn visible_name(ctx: &Context, c: ClassId) -> String {
    let class = &ctx.program.classes[c.0 as usize];
    let name_str = ctx.symbol_name(class.name);
    let lib = ctx.program.library(class.library).uri.as_str();
    let visible = match (lib, name_str) {
        ("dart:core", "_Smi" | "_Mint") => Some("int"),
        ("dart:core", "_Double") => Some("double"),
        ("dart:core", "_OneByteString" | "_TwoByteString" | "_ExternalOneByteString" | "_ExternalTwoByteString") => {
            Some("String")
        }
        ("dart:core", "_List" | "_ImmutableList" | "_GrowableList") => Some("List"),
        ("dart:async", "_Future") => Some("Future"),
        ("dart:typed_data", n) => match n {
            "_Int8List" | "_ExternalInt8Array" => Some("Int8List"),
            "_Uint8List" | "_ExternalUint8Array" => Some("Uint8List"),
            "_Uint8ClampedList" | "_ExternalUint8ClampedArray" => Some("Uint8ClampedList"),
            "_Int16List" | "_ExternalInt16Array" => Some("Int16List"),
            "_Uint16List" | "_ExternalUint16Array" => Some("Uint16List"),
            "_Int32List" | "_ExternalInt32Array" => Some("Int32List"),
            "_Uint32List" | "_ExternalUint32Array" => Some("Uint32List"),
            "_Int64List" | "_ExternalInt64Array" => Some("Int64List"),
            "_Uint64List" | "_ExternalUint64Array" => Some("Uint64List"),
            "_Float32List" | "_ExternalFloat32Array" => Some("Float32List"),
            "_Float64List" | "_ExternalFloat64Array" => Some("Float64List"),
            "_Float32x4List" | "_ExternalFloat32x4Array" => Some("Float32x4List"),
            "_Int32x4List" | "_ExternalInt32x4Array" => Some("Int32x4List"),
            "_Float64x2List" | "_ExternalFloat64x2Array" => Some("Float64x2List"),
            "_Float32x4" => Some("Float32x4"),
            "_Int32x4" => Some("Int32x4"),
            "_Float64x2" => Some("Float64x2"),
            _ => None,
        },
        _ => None,
    };
    visible.unwrap_or(name_str).to_string()
}

/// Where a type check happens: it changes the end of the VM's message.
#[derive(Debug, Clone)]
pub enum CastContext {
    /// `v as T`: " in type cast".
    How,
    /// Implicit assignment of a `dynamic`: no suffix.
    Implicit,
    /// The parameter of a dynamic or covariant call: " of 'name'".
    Parameter(String),
}
