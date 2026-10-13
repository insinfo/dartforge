//! Field initialization in constructors (docs/ANALYZER-ESPECIFICACAO.md,
//! part C, constructors):
//!
//! * the `ConstructorFieldsVerifier`
//!   (`analyzer/lib/src/error/constructor_fields_verifier.dart`): the state
//!   of each field per generative, non-redirecting, non-`external`
//!   constructor — `field_initialized_by_multiple_initializers`,
//!   `field_initialized_in_initializer_and_declaration`,
//!   `field_initialized_in_parameter_and_initializer` and
//!   `final_initialized_in_declaration_and_constructor`;
//! * `ErrorVerifier._checkForValidField` (`this.x`, in every constructor) and
//!   `_checkForInvalidField` (`x = e`):
//!   `initializing_formal_for_non_existent_field`,
//!   `initializer_for_static_field` and `initializer_for_non_existent_field`.
//!
//! The field of a name is `augmented.getField(name)`: the first field
//! declared with it; without a field, the synthetic one of a getter/setter
//! (and the enum's `values`), which does not count as a field. The type
//! comparison (`field_initializing_formal_not_assignable`) lives in
//! `crates/types`.
//!
//! Written without compiling or running (2026-10-05).

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DeclKind, FunctionKind, Initializer, MemberKind};
use dartforge_intern::{Interner, SymbolId};

/// A declared (non-synthetic) field.
struct Field {
    name: SymbolId,
    is_static: bool,
    final_or_const: bool,
    has_initializer: bool,
}

/// What `getField(name)` gives.
enum Found {
    Declared(usize),
    Synthetic,
    Missing,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Uninitialized,
    InDeclaration,
    InParameter,
    InInitializer,
}

struct Owner<'a> {
    fields: Vec<Field>,
    /// Names of getters/setters (synthetic fields) and the enum's `values`.
    synthetics: Vec<SymbolId>,
    members: &'a [ast::MemberId],
    /// Enum: `index` does not enter the verifier's map.
    enum_: bool,
}

impl Owner<'_> {
    fn find(&self, name: SymbolId) -> Found {
        if let Some(i) = self.fields.iter().position(|f| f.name == name) {
            return Found::Declared(i);
        }
        if self.synthetics.contains(&name) {
            return Found::Synthetic;
        }
        Found::Missing
    }
}

fn owner<'a>(u: &Unidade<'a>, d: &'a ast::Decl, names: &Interner) -> Option<Owner<'a>> {
    let a = u.ast;
    let mut fields = Vec::new();
    let mut synthetics = Vec::new();
    let (members, enum_): (&'a [ast::MemberId], bool) = match &d.kind {
        DeclKind::Class(k) if !k.mixin_application => (&k.members, false),
        DeclKind::Mixin(k) => (&k.members, false),
        DeclKind::Enum(k) => {
            // The constants are `const` static fields with an initializer;
            // `values` is synthetic.
            for k in k.constants.iter() {
                fields.push(Field { name: k.name.sym, is_static: true, final_or_const: true, has_initializer: true });
            }
            if let Some(v) = names.lookup("values") {
                synthetics.push(v);
            }
            (&k.members, true)
        }
        DeclKind::ExtensionType(k) => {
            fields.push(Field { name: k.representation_name.sym, is_static: false, final_or_const: true, has_initializer: false });
            (&k.members, false)
        }
        _ => return None,
    };
    for &mid in members {
        let m = a.member(mid);
        match &m.kind {
            MemberKind::Field(l) => {
                for v in l.variables.iter() {
                    fields.push(Field { name: v.name.sym, is_static: l.static_, final_or_const: l.final_ || l.const_, has_initializer: v.initializer.is_some() });
                }
            }
            MemberKind::Method(fid) => {
                let f = a.function(*fid);
                if matches!(f.kind, FunctionKind::Getter | FunctionKind::Setter)
                    && let Some(n) = f.name
                {
                    synthetics.push(n.sym);
                }
            }
            MemberKind::Constructor(_) => {}
        }
    }
    Some(Owner { fields, synthetics, members, enum_ })
}

/// The `FieldFormalParameter` node: from the metadata (or the first modifier)
/// to the name, or to the end of a function parameter's list (with the `?`).
fn param_span(source: &str, p: &ast::Parameter) -> Span {
    let start = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
    let mut end = p.name.map_or(p.span.end, |n| n.span.end);
    if p.function_parameters.is_some() {
        let b = source.as_bytes();
        let mut i = end;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        // Type parameters and the list: up to the matching `)`.
        let mut depth = 0i32;
        while i < b.len() {
            match b[i] {
                b'(' | b'<' => depth += 1,
                b')' | b'>' => {
                    depth -= 1;
                    if depth == 0 && b[i] == b')' {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        end = i;
        if p.function_nullable && b.get(end) == Some(&b'?') {
            end += 1;
        }
    }
    Span { start, end: end.min(p.span.end).max(start) }
}

pub fn check(units: &[Unidade<'_>], names: &Interner) -> Vec<(usize, Diagnostic)> {
    let mut out = Vec::new();
    for (iu, u) in units.iter().enumerate() {
        let a = u.ast;
        for &did in &u.unit.declarations {
            let decl = a.decl(did);
            let Some(owner) = owner(u, decl, names) else { continue };
            // The verifier's map: the declared fields (without the enum's
            // `index`), with the starting state.
            let index = names.lookup("index");
            let initial: Vec<Option<State>> = owner
                .fields
                .iter()
                .map(|f| {
                    if owner.enum_ && Some(f.name) == index {
                        None
                    } else if f.has_initializer {
                        Some(State::InDeclaration)
                    } else {
                        Some(State::Uninitialized)
                    }
                })
                .collect();
            // The primary constructor (3.13, the verifier's `isPrimary`): the
            // class's or enum's elaborated one, and in an extension type the
            // `this : …` body part, whose primary already initialized the
            // representation through the parameter.
            let (primary, in_extension_type) = match &decl.kind {
                DeclKind::Class(k) => (k.primary_constructor, false),
                DeclKind::Enum(k) => (k.primary_constructor, false),
                DeclKind::ExtensionType(_) => (None, true),
                _ => (None, false),
            };
            for &mid in owner.members {
                let m = a.member(mid);
                let MemberKind::Constructor(k) = &m.kind else { continue };
                let is_primary = primary == Some(mid) || (in_extension_type && k.parte_primaria);
                // `_checkForValidField`: todo construtor.
                for p in k.parameters.iter().filter(|p| p.this_ && !p.super_) {
                    let name = p.name.map(|n| names.resolve(n.sym).to_string()).unwrap_or_default();
                    let target = p.name.map_or(Found::Missing, |n| owner.find(n.sym));
                    let sp = param_span(u.fonte, p);
                    match target {
                        Found::Missing | Found::Synthetic => {
                            out.push((iu, Diagnostic::com_codigo(c::INITIALIZING_FORMAL_FOR_NON_EXISTENT_FIELD, sp, [name.as_str()])));
                        }
                        Found::Declared(i) if owner.fields[i].is_static => {
                            // The primary constructor (3.13.4,
                            // `_checkForValidField`): on the `this`.
                            let sp = match p.name {
                                Some(n) if is_primary => u.fonte[sp.start..n.span.start]
                                    .rfind("this")
                                    .map_or(sp, |i| Span { start: sp.start + i, end: sp.start + i + 4 }),
                                _ => sp,
                            };
                            out.push((iu, Diagnostic::com_codigo(c::INITIALIZER_FOR_STATIC_FIELD, sp, [name.as_str()])));
                        }
                        Found::Declared(_) => {}
                    }
                }
                // `_checkForInvalidField`: every `x = e`.
                for init in k.initializers.iter() {
                    if let Initializer::Field { span, name, .. } = init {
                        let text = names.resolve(name.sym).to_string();
                        match owner.find(name.sym) {
                            Found::Missing | Found::Synthetic => {
                                out.push((iu, Diagnostic::com_codigo(c::INITIALIZER_FOR_NON_EXISTENT_FIELD, *span, [text.as_str()])));
                            }
                            Found::Declared(i) if owner.fields[i].is_static => {
                                out.push((iu, Diagnostic::com_codigo(c::INITIALIZER_FOR_STATIC_FIELD, *span, [text.as_str()])));
                            }
                            Found::Declared(_) => {}
                        }
                    }
                }
                // The `ConstructorFieldsVerifier`: only a generative,
                // non-redirecting (`= B`), non-`external` one.
                if k.factory || k.redirect.is_some() || k.external {
                    continue;
                }
                let mut states = initial.clone();
                if in_extension_type && is_primary {
                    // The representation (the owner's first field).
                    states[0] = Some(State::InParameter);
                }
                if !m.augment {
                    for p in k.parameters.iter().filter(|p| p.this_ && !p.super_) {
                        let Some(n) = p.name else { continue };
                        let Found::Declared(i) = owner.find(n.sym) else { continue };
                        match states[i] {
                            Some(State::Uninitialized) => states[i] = Some(State::InParameter),
                            // 3.13.4's `updateWithParameters`
                            // (`constructor_fields_verifier.dart:296-322`): in
                            // the primary, its own code on the name, and the
                            // field becomes initialized by the parameter.
                            Some(State::InDeclaration) if is_primary => {
                                out.push((
                                    iu,
                                    Diagnostic::com_codigo(c::FIELD_INITIALIZED_IN_DECLARATION_AND_PARAMETER_OF_PRIMARY_CONSTRUCTOR, n.span, Vec::<&str>::new()),
                                ));
                                states[i] = Some(State::InParameter);
                            }
                            Some(State::InDeclaration) if owner.fields[i].final_or_const => {
                                out.push((
                                    iu,
                                    Diagnostic::com_codigo(c::FINAL_INITIALIZED_IN_DECLARATION_AND_CONSTRUCTOR, n.span, [names.resolve(n.sym)]),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                for init in k.initializers.iter() {
                    let Initializer::Field { name, .. } = init else { continue };
                    let Found::Declared(i) = owner.find(name.sym) else { continue };
                    match states[i] {
                        Some(State::Uninitialized) => states[i] = Some(State::InInitializer),
                        // 3.13.4's `updateWithInitializers` (`:262-281`).
                        Some(State::InDeclaration) if is_primary => {
                            out.push((
                                iu,
                                Diagnostic::com_codigo(c::FIELD_INITIALIZED_IN_DECLARATION_AND_INITIALIZER_OF_PRIMARY_CONSTRUCTOR, name.span, Vec::<&str>::new()),
                            ));
                            states[i] = Some(State::InInitializer);
                        }
                        Some(State::InDeclaration) => {
                            if owner.fields[i].final_or_const {
                                out.push((iu, Diagnostic::com_codigo(c::FIELD_INITIALIZED_IN_INITIALIZER_AND_DECLARATION, name.span, Vec::<&str>::new())));
                            }
                        }
                        Some(State::InParameter) => {
                            out.push((iu, Diagnostic::com_codigo(c::FIELD_INITIALIZED_IN_PARAMETER_AND_INITIALIZER, name.span, Vec::<&str>::new())));
                        }
                        Some(State::InInitializer) => {
                            out.push((iu, Diagnostic::com_codigo(c::FIELD_INITIALIZED_BY_MULTIPLE_INITIALIZERS, name.span, [names.resolve(name.sym)])));
                        }
                        None => {}
                    }
                }
            }
        }
    }
    out
}
