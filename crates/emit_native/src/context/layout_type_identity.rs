//! Stable semantic field-type identities for live-instance migration.

use dartforge_elements::model::ClassId;
use dartforge_intern::SymbolId;
use dartforge_types::table::{Type, TypeId, TypeParamId, TypeTable};

/// Encodes a resolved type, excluding alias spelling and transient arena IDs.
/// Declaration resolvers must return unambiguous length-prefixed owner keys;
/// unresolved declarations or excessive recursive structure return `None`.
pub(super) fn encode(
    table: &TypeTable,
    ty: TypeId,
    class: &dyn Fn(ClassId) -> Option<String>,
    parameter: &dyn Fn(TypeParamId) -> Option<String>,
    name: &dyn Fn(SymbolId) -> String,
) -> Option<String> {
    let mut encoder = Encoder { table, class, parameter, name, binders: Vec::new(), active_parameters: Vec::new(), active_types: Vec::new(), nodes_remaining: 16_384 };
    Some(format!("type-v1:{}", encoder.ty(ty)?))
}

/// Length prefixes prevent URI, name and structural delimiter collisions.
pub(super) fn atom(text: &str) -> String {
    format!("{}:{text}", text.len())
}

struct Encoder<'a> {
    table: &'a TypeTable,
    class: &'a dyn Fn(ClassId) -> Option<String>,
    parameter: &'a dyn Fn(TypeParamId) -> Option<String>,
    name: &'a dyn Fn(SymbolId) -> String,
    binders: Vec<Vec<TypeParamId>>,
    active_parameters: Vec<TypeParamId>,
    active_types: Vec<TypeId>,
    nodes_remaining: usize,
}

impl Encoder<'_> {
    fn list(&mut self, types: &[TypeId]) -> Option<String> {
        Some(format!("{}[{}]", types.len(), types.iter().map(|&t| self.ty(t)).collect::<Option<Vec<_>>>()?.join("")))
    }

    fn parameter(&mut self, param: TypeParamId, nullable: bool) -> Option<String> {
        for (depth, params) in self.binders.iter().rev().enumerate() {
            if let Some(index) = params.iter().position(|&p| p == param) {
                return Some(format!("B{depth}:{index}:{};", u8::from(nullable)));
            }
        }
        if self.active_parameters.len() >= 128 {
            return None;
        }
        let owner = (self.parameter)(param)?;
        if self.active_parameters.contains(&param) {
            return Some(format!("R{}:{};", atom(&owner), u8::from(nullable)));
        }
        self.active_parameters.push(param);
        let bound = self.ty(self.table.param(param).bound);
        self.active_parameters.pop();
        Some(format!("P{}:{}:{}", atom(&owner), u8::from(nullable), bound?))
    }

    fn ty(&mut self, ty: TypeId) -> Option<String> {
        // Shared structural DAGs can branch exponentially even at shallow
        // depth. A global work budget makes refusal bounded as well.
        if self.nodes_remaining == 0 {
            return None;
        }
        self.nodes_remaining -= 1;
        match self.table.decoration(ty) {
            Some(dartforge_types::table::DisplayDecoration::Invalid) => return None,
            Some(dartforge_types::table::DisplayDecoration::NeverNullable) => return Some("Z;".into()),
            _ => {}
        }
        let ty = self.table.canonical(ty);
        // Named F-bounds use owner backreferences; other cycles are unproved.
        if self.active_types.len() >= 128 {
            return None;
        }
        if let Type::TypeParameter { param, nullable } = self.table.get(ty) {
            return self.parameter(*param, *nullable);
        }
        if self.active_parameters.is_empty() && self.active_types.contains(&ty) {
            return None;
        }
        // A field C<T> may revisit C<T> inside T's F-bound. Let the stable
        // parameter backreference terminate it; cycles without such an anchor
        // fail the bounded structural-depth check above.
        self.active_types.push(ty);
        let result = self.non_parameter(self.table.get(ty));
        self.active_types.pop();
        result
    }

    fn non_parameter(&mut self, ty: &Type) -> Option<String> {
        match ty {
            Type::Dynamic => Some("D;".into()),
            Type::Void => Some("V;".into()),
            Type::Never => Some("N;".into()),
            Type::Null => Some("Z;".into()),
            Type::Interface { class, args, nullable } => {
                Some(format!("I{}:{}:{}", atom(&(self.class)(*class)?), u8::from(*nullable), self.list(args)?))
            }
            Type::ExtensionType { decl, args, nullable } => {
                Some(format!("E{}:{}:{}", atom(&(self.class)(*decl)?), u8::from(*nullable), self.list(args)?))
            }
            Type::FutureOr { arg, nullable } => Some(format!("U{}:{}", u8::from(*nullable), self.ty(*arg)?)),
            Type::Record { positional, named, nullable } => {
                let mut named = named.iter().map(|&(name, ty)| ((self.name)(name), ty)).collect::<Vec<_>>();
                named.sort_by(|a, b| a.0.cmp(&b.0));
                let fields = named.iter().map(|(name, ty)| Some(format!("{}{}", atom(name), self.ty(*ty)?))).collect::<Option<Vec<_>>>()?;
                Some(format!("T{}:{}:{}[{}]", u8::from(*nullable), self.list(positional)?, fields.len(), fields.join("")))
            }
            Type::Function { type_params, ret, positional, optional, named, nullable } => {
                self.binders.push(type_params.to_vec());
                let result = (|| {
                    let bounds = type_params.iter().map(|&p| self.ty(self.table.param(p).bound)).collect::<Option<Vec<_>>>()?;
                    let mut named = named.iter().map(|&(name, ty, required)| ((self.name)(name), ty, required)).collect::<Vec<_>>();
                    named.sort_by(|a, b| a.0.cmp(&b.0));
                    let fields = named.iter().map(|(name, ty, required)| Some(format!("{}{}{}", atom(name), u8::from(*required), self.ty(*ty)?))).collect::<Option<Vec<_>>>()?;
                    Some(format!("F{}:{}[{}]{}{}{}{}[{}]", u8::from(*nullable), bounds.len(), bounds.join(""), self.ty(*ret)?, self.list(positional)?, self.list(optional)?, fields.len(), fields.join("")))
                })();
                self.binders.pop();
                result
            }
            Type::Intersection { .. } => None, // Flow-only types cannot prove a declared field identity.
            Type::TypeParameter { .. } => unreachable!("handled before structural recursion"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_types::table::{DisplayDecoration, TypeParamOwner, Variance};

    fn symbol(index: u32) -> SymbolId {
        let mut interner = dartforge_intern::Interner::new();
        (0..=index).map(|i| interner.intern(&format!("name{i}"))).last().unwrap()
    }

    fn identity(table: &TypeTable, ty: TypeId) -> Option<String> {
        encode(table, ty, &|id| match id.0 { 1 => Some(format!("C{}{}", atom("a.dart"), atom("Foo"))), 2 => Some(format!("C{}{}", atom("b.dart"), atom("Foo"))), _ => None }, &|_| Some("class-owner:parameter:0".into()), &|name| format!("name{}", name.as_u32()))
    }

    #[test]
    fn library_nullability_arguments_and_aliases_have_semantic_identity() {
        let mut table = TypeTable::new();
        let a = table.intern(Type::Interface { class: ClassId(1), args: Box::new([]), nullable: false });
        let b = table.intern(Type::Interface { class: ClassId(2), args: Box::new([]), nullable: false });
        let nullable = table.intern(Type::Interface { class: ClassId(1), args: Box::new([]), nullable: true });
        let generic = table.intern(Type::Interface { class: ClassId(1), args: vec![b].into_boxed_slice(), nullable: false });
        assert_ne!(identity(&table, a), identity(&table, b));
        assert_ne!(identity(&table, a), identity(&table, nullable));
        assert_ne!(identity(&table, a), identity(&table, generic));
        table.preserve_decoration = true;
        let alias = table.decorate(a, DisplayDecoration::Alias { typedef: dartforge_elements::model::TypedefId(77), args: Box::new([]) });
        assert_eq!(identity(&table, a), identity(&table, alias));
        let invalid = table.decorate(a, DisplayDecoration::Invalid);
        assert_eq!(identity(&table, invalid), None);
        let never = table.intern(Type::Never);
        let nullable_never = table.decorate(never, DisplayDecoration::NeverNullable);
        let null = table.intern(Type::Null);
        assert_eq!(identity(&table, nullable_never), identity(&table, null));
        let unknown = table.intern(Type::Interface { class: ClassId(99), args: Box::new([]), nullable: false });
        assert_eq!(identity(&table, unknown), None);
    }

    #[test]
    fn generic_function_binders_are_alpha_equivalent_but_keep_bounds() {
        let mut table = TypeTable::new();
        let dynamic = table.intern(Type::Dynamic);
        let p = table.alloc_type_param(symbol(1), TypeParamOwner::GenericFunctionType, dynamic, Variance::Invariant);
        let q = table.alloc_type_param(symbol(2), TypeParamOwner::GenericFunctionType, dynamic, Variance::Invariant);
        let pt = table.intern(Type::TypeParameter { param: p, nullable: false });
        let qt = table.intern(Type::TypeParameter { param: q, nullable: false });
        let make = |param, ty| Type::Function { type_params: vec![param].into_boxed_slice(), ret: ty, positional: vec![ty].into_boxed_slice(), optional: Box::new([]), named: Box::new([]), nullable: false };
        let f = table.intern(make(p, pt));
        let g = table.intern(make(q, qt));
        assert_eq!(identity(&table, f), identity(&table, g));
        let null = table.intern(Type::Null);
        table.set_type_param_bound(q, null);
        assert_ne!(identity(&table, f), identity(&table, g));
    }

    #[test]
    fn records_order_names_by_text_and_keep_required_function_parameters() {
        let mut table = TypeTable::new();
        let dynamic = table.intern(Type::Dynamic);
        let a = table.intern(Type::Record { positional: Box::new([]), named: vec![(symbol(1), dynamic), (symbol(2), dynamic)].into_boxed_slice(), nullable: false });
        let b = table.intern(Type::Record { positional: Box::new([]), named: vec![(symbol(2), dynamic), (symbol(1), dynamic)].into_boxed_slice(), nullable: false });
        assert_eq!(identity(&table, a), identity(&table, b));
        let make = |required| Type::Function { type_params: Box::new([]), ret: dynamic, positional: Box::new([]), optional: Box::new([]), named: vec![(symbol(1), dynamic, required)].into_boxed_slice(), nullable: false };
        let f = table.intern(make(false));let g = table.intern(make(true));
        assert_ne!(identity(&table, f), identity(&table, g));
    }
    #[test]
    fn nested_generic_functions_preserve_lexical_binder_depth() {
        let mut table = TypeTable::new();
        let dynamic = table.intern(Type::Dynamic);
        let p = table.alloc_type_param(symbol(1), TypeParamOwner::GenericFunctionType, dynamic, Variance::Invariant);
        let q = table.alloc_type_param(symbol(2), TypeParamOwner::GenericFunctionType, dynamic, Variance::Invariant);
        let r = table.alloc_type_param(symbol(3), TypeParamOwner::GenericFunctionType, dynamic, Variance::Invariant);
        let s = table.alloc_type_param(symbol(4), TypeParamOwner::GenericFunctionType, dynamic, Variance::Invariant);
        let pt = table.intern(Type::TypeParameter { param: p, nullable: false });
        let qt = table.intern(Type::TypeParameter { param: q, nullable: false });
        let rt = table.intern(Type::TypeParameter { param: r, nullable: false });
        let make = |param, ret| Type::Function { type_params: vec![param].into_boxed_slice(), ret, positional: Box::new([]), optional: Box::new([]), named: Box::new([]), nullable: false };
        let inner_p = table.intern(make(r, pt));
        let inner_q = table.intern(make(s, qt));
        let inner_r = table.intern(make(r, rt));
        let outer_p = table.intern(make(p, inner_p));
        let outer_q = table.intern(make(q, inner_q));
        let different = table.intern(make(p, inner_r));
        assert_eq!(identity(&table, outer_p), identity(&table, outer_q));
        assert_ne!(identity(&table, outer_p), identity(&table, different));
    }

    #[test]
    fn external_parameter_bounds_and_f_bounds_are_not_display_only() {
        let mut table = TypeTable::new();
        let dynamic = table.intern(Type::Dynamic);
        let p = table.alloc_type_param(symbol(1), TypeParamOwner::Class(ClassId(1)), dynamic, Variance::Invariant);
        let pt = table.intern(Type::TypeParameter { param: p, nullable: false });
        let old = identity(&table, pt).unwrap();
        let bound = table.intern(Type::Interface { class: ClassId(1), args: vec![pt].into_boxed_slice(), nullable: false });
        table.set_type_param_bound(p, bound);
        let new = identity(&table, pt).unwrap();
        assert_ne!(old, new);
        assert!(new.contains("R"), "F-bound must use a stable owner backreference");
        assert!(identity(&table, bound).is_some(), "C<T> must also encode when T extends C<T>");
        assert_eq!(encode(&table, pt, &|_| Some("class".into()), &|_| None, &|_| "name".into()), None);
    }

    #[test]
    fn identity_is_independent_of_arena_numbering_and_delimiter_spellings() {
        let mut first = TypeTable::new();
        let a = first.intern(Type::Interface { class: ClassId(1), args: Box::new([]), nullable: false });
        let mut second = TypeTable::new();
        second.intern(Type::Null);
        second.intern(Type::Never);
        let b = second.intern(Type::Interface { class: ClassId(77), args: Box::new([]), nullable: false });
        let encode_a = encode(&first, a, &|_| Some("7:package5:Foo:A".into()), &|_| None, &|_| String::new());
        let encode_b = encode(&second, b, &|_| Some("7:package5:Foo:A".into()), &|_| None, &|_| String::new());
        assert_eq!(encode_a, encode_b);
        assert_ne!(atom("a:b"), atom("ab:"));
        let mut deep = first.intern(Type::Null);
        for _ in 0..130 {
            deep = first.intern(Type::FutureOr { arg: deep, nullable: false });
        }
        assert_eq!(identity(&first, deep), None);
        let mut branching = first.intern(Type::Null);
        for _ in 0..16 {
            branching = first.intern(Type::Record { positional: vec![branching, branching].into_boxed_slice(), named: Box::new([]), nullable: false });
        }
        assert_eq!(identity(&first, branching), None, "bounded work even for exponentially shared DAGs");
    }

}

impl super::Context<'_> {
    pub(super) fn layout_type_identity(&self, ty: TypeId) -> Option<String> {
        encode(self.table, ty, &|class| self.layout_class_key(class), &|param| self.layout_parameter_key(param), &|name| self.symbol_name(name).to_owned())
    }

    fn layout_class_key(&self, class: ClassId) -> Option<String> {
        let declaration = self.program.classes.get(class.0 as usize)?;
        // Named SDK outline declarations retain the dart: URI even without an
        // AST body. Anonymous synthetic application classes have no proven key.
        if declaration.decl.is_none() && !self.program.library(declaration.library).is_sdk {
            return None;
        }
        let name = self.symbol_name(declaration.name);
        if name.is_empty() {
            return None;
        }
        Some(format!("C{}{}", atom(&self.library_name(declaration.library)), atom(name)))
    }

    fn layout_parameter_key(&self, param: TypeParamId) -> Option<String> {
        use dartforge_types::table::TypeParamOwner;
        let data = self.table.param(param);
        let (owner, parameters): (String, &[TypeParamId]) = match data.owner {
            TypeParamOwner::Class(class) => (self.layout_class_key(class)?, &self.outline.classes.get(class.0 as usize)?.type_params),
            TypeParamOwner::Typedef(id) => {
                let declaration = self.program.typedefs.get(id.0 as usize)?;
                (format!("A{}{}", atom(&self.library_name(declaration.library)), atom(self.symbol_name(declaration.name))), &self.outline.typedefs.get(id.0 as usize)?.type_params)
            }
            TypeParamOwner::Extension(id) => {
                let declaration = self.program.extensions.get(id.0 as usize)?;
                let name = declaration.name?;
                (format!("X{}{}", atom(&self.library_name(declaration.library)), atom(self.symbol_name(name))), &self.outline.extensions.get(id.0 as usize)?.type_params)
            }
            TypeParamOwner::Function(id) => {
                let declaration = self.program.functions.get(id.0 as usize)?;
                let owner = if let Some(class) = declaration.class {
                    self.layout_class_key(class)?
                } else if let Some(extension) = declaration.extension {
                    let extension = self.program.extensions.get(extension.0 as usize)?;
                    format!("X{}{}", atom(&self.library_name(extension.library)), atom(self.symbol_name(extension.name?)))
                } else {
                    format!("L{}", atom(&self.library_name(declaration.library)))
                };
                let kind = match declaration.kind {
                    dartforge_elements::model::FunctionKind::Function => "function",
                    dartforge_elements::model::FunctionKind::Getter => "getter",
                    dartforge_elements::model::FunctionKind::Setter => "setter",
                    dartforge_elements::model::FunctionKind::Operator => "operator",
                    dartforge_elements::model::FunctionKind::Constructor => "constructor",
                    dartforge_elements::model::FunctionKind::SyntheticConstructor => return None,
                    dartforge_elements::model::FunctionKind::ImplicitAccessor => return None,
                };
                (format!("M{}{}{}", atom(&owner), atom(kind), atom(self.symbol_name(declaration.name))), &self.outline.functions.get(id.0 as usize)?.type_params)
            }
            TypeParamOwner::GenericFunctionType => return None,
        };
        let index = parameters.iter().position(|&candidate| candidate == param)?;
        Some(format!("{}:{index}", atom(&owner)))
    }
}
