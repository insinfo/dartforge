//! Erased extension types (Dart 3.3): what the lowering needs to know
//! about them, computed beforehand, while the type table still accepts new
//! types (the `Context` sees it read-only).
//!
//! At run time a value of an extension type **is** the value of the
//! representation: `Id(3)` is the `int` 3, `is Id` is `is int`, and the `runtimeType`
//! of `<Id>[]` is `List<int>`. The static type only decides the route of the members,
//! which is static (`lower/extension_types.rs`). From here come:
//!
//! * the erasure of each type of the table (`E<A>` → the representation of `E`
//!   with `A`, recursively, inside lists, functions and records);
//! * the type of `this` of each extension type (`E<T>`, with its own
//!   parameters): it is the pattern by which the type arguments of a member are
//!   taken from the static type of the receiver;
//! * the instance of each extension supertype of each extension type of the
//!   table (`Child<int>` implements `Base<List<int>>`), to find those
//!   arguments when the member is inherited.

use dartforge_elements::model::{ClassId, ClassKind, Program};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::table::{Type, TypeId, TypeParamId, TypeTable};
use std::collections::HashMap;

/// The erasure and the auxiliary types of the program's extension types.
#[derive(Debug, Default, Clone)]
pub struct ExtensionTypes {
    /// The erased type of each type of the table (by index); empty when the
    /// program has no extension type.
    erased_ones: Vec<TypeId>,
    /// `E<T…>` of each extension type, with its own parameters.
    pub this: HashMap<ClassId, TypeId>,
    /// `(E<A…>, S) → S<B…>`: the instance of the extension supertype `S` of
    /// `E<A…>`.
    pub supertypes: HashMap<(TypeId, ClassId), TypeId>,
}

impl ExtensionTypes {
    /// The erased type of `t` (itself, with no extension type inside).
    pub fn erase(&self, t: TypeId) -> TypeId {
        self.erased_ones.get(t.0 as usize).copied().unwrap_or(t)
    }
}

/// The type parameters of `c` and the representation type, written with them.
fn representation(
    program: &Program,
    outline: &OutlineTypes,
    c: ClassId,
) -> Option<(Vec<TypeParamId>, TypeId)> {
    let v = program.class(c).representation?;
    let t = outline.variables.get(v.0 as usize)?.declared_type?;
    let params = outline
        .classes
        .get(c.0 as usize)
        .map(|d| d.type_params.to_vec())
        .unwrap_or_default();
    Some((params, t))
}

/// Computes the erasure of the whole table (see the top of the module).
pub fn compute(program: &Program, outline: &OutlineTypes, table: &mut TypeTable) -> ExtensionTypes {
    let types: Vec<ClassId> = (0..program.classes.len() as u32)
        .map(ClassId)
        .filter(|&c| program.class(c).kind == ClassKind::ExtensionType)
        .collect();
    let mut r = ExtensionTypes::default();
    if types.is_empty() {
        return r;
    }
    for &c in &types {
        let params = outline
            .classes
            .get(c.0 as usize)
            .map(|d| d.type_params.to_vec())
            .unwrap_or_default();
        let args: Vec<TypeId> = params
            .iter()
            .map(|&p| {
                table.intern(Type::TypeParameter {
                    param: p,
                    nullable: false,
                })
            })
            .collect();
        let t = table.intern(Type::ExtensionType {
            decl: c,
            args: args.into_boxed_slice(),
            nullable: false,
        });
        r.this.insert(c, t);
    }
    // The extension supertypes of each extension type of the table, through
    // `implements` (with the substitution of the arguments), up to the closure.
    let mut i = 0;
    while i < table.len() {
        let t = TypeId(i as u32);
        i += 1;
        let Type::ExtensionType { decl, args, .. } = table.get(t).clone() else {
            continue;
        };
        let mut pending_set = vec![(decl, args.to_vec())];
        while let Some((d, a)) = pending_set.pop() {
            let Some(data) = outline.classes.get(d.0 as usize) else {
                continue;
            };
            let map: HashMap<TypeParamId, TypeId> = data
                .type_params
                .iter()
                .copied()
                .zip(a.iter().copied())
                .collect();
            for &s in data.interfaces.iter() {
                let Type::ExtensionType { decl: sd, .. } = table.get(s).clone() else {
                    continue;
                };
                let inst = dartforge_types::substitute(s, &map, table);
                if r.supertypes.insert((t, sd), inst).is_none()
                    && let Type::ExtensionType { args: sa, .. } = table.get(inst).clone()
                {
                    pending_set.push((sd, sa.to_vec()));
                }
            }
        }
    }
    let rep = |c: ClassId, args: &[TypeId], table: &mut TypeTable| -> Option<TypeId> {
        let (params, t) = representation(program, outline, c)?;
        let map: HashMap<TypeParamId, TypeId> =
            params.into_iter().zip(args.iter().copied()).collect();
        Some(dartforge_types::substitute(t, &map, table))
    };
    // The types the erasure creates come out already erased: the table from
    // before it is enough.
    let n = table.len();
    r.erased_ones = (0..n as u32).map(TypeId).collect();
    for i in 0..n {
        r.erased_ones[i] = dartforge_types::erase_extension_type(TypeId(i as u32), table, &rep);
    }
    r
}
