//! Private declarations never referenced: the library part of the
//! `UnusedLocalElementsVerifier` of analyzer 6.11.0
//! (`src/error/unused_local_elements_verifier.dart`) —
//! `UNUSED_ELEMENT` (classes, mixins, enums, extension types, typedefs,
//! functions, top-level variables, methods, getters, setters and named
//! constructors that are not accessible from outside the library) and
//! `UNUSED_FIELD` (a private field, or a public static one of a private type,
//! that is never read).
//!
//! The analyzer binds each reference to its element; this layer does not have
//! the receivers' types (`a._x`), so binding is **by name**: a declaration is
//! only reported when no identifier with its name appears in the library
//! outside the declarations (the top-level ones and the members of every
//! unit). Any occurrence counts as a use, even if the analyzer would bind it
//! to another element: the possible error is failing to report (a false
//! negative), never reporting a use that exists. The exceptions follow the
//! analyzer:
//!
//! * field and top-level variable: only a **read** counts — `x = e`, `this.x`
//!   in a constructor and `x = e` in the initializer list write (`x += 1;`
//!   and `x++;` count as reads here, a conservative approximation);
//! * class (`class`, not mixin/enum): references inside the declaration
//!   itself do not count (`_enclosingClass`);
//! * comments do not count (the lexer does not return them);
//! * `values` anywhere reads every enum constant;
//! * a constructor is only reported if the class has more than one;
//! * `@pragma(...)` and `@JS(...)` make the declaration used.
//!
//! Libraries with a syntax error or with `augment` are left out.

use crate::Unit;
use dartforge_diagnostics::codes::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{self, DeclKind, Initializer, MemberKind};
use dartforge_frontend::token::{Kind, Op};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// An identifier in the source.
struct Occurrence {
    unit: usize,
    span: Span,
    write: bool,
}

/// What is reported and how the use is decided.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// `UNUSED_ELEMENT`: any reference uses it.
    Element,
    /// `UNUSED_ELEMENT` of a top-level variable: only a read uses it.
    TopLevelVariable,
    /// `UNUSED_FIELD`: only a read uses it.
    Field,
    /// `UNUSED_FIELD` of an enum constant: a read or `values`.
    EnumConstant,
}

struct Candidate {
    unit: usize,
    /// The name as it appears in the identifiers.
    name: String,
    /// What the message shows (`A.named` for a constructor).
    displayed: String,
    span: Span,
    rule: Rule,
    /// References inside this range do not count (class).
    own: Option<Span>,
}

fn is_private(name: &str) -> bool {
    name.starts_with('_')
}

/// An annotation the analyzer treats as a use (`@pragma('vm:entry-point')`,
/// `@JS`): the declaration with it is left out.
fn is_annotated(metadata: &[ast::Annotation], names: &Interner) -> bool {
    metadata.iter().any(|a| {
        a.name
            .iter()
            .any(|n| matches!(names.resolve(n.sym), "pragma" | "JS" | "js"))
    })
}

/// Collects the candidates and the ranges of every named declaration.
struct Collector<'a> {
    names: &'a Interner,
    candidates: Vec<Candidate>,
    /// `(unit, start)` of the declared names (they are not references).
    decls: HashSet<(usize, usize)>,
    /// `(unit, start)` of identifiers that only write.
    writes: HashSet<(usize, usize)>,
    /// Some `augment` member: the library is left out.
    augment: bool,
    /// Types named by a public `typedef` (`typedef P = _C;`).
    exposed: HashSet<String>,
    /// `(unit, start)` of the fields declared by a primary constructor
    /// parameter (3.13): analyzer 3.13 reports them with their own code
    /// (`UNUSED_FIELD_FROM_PRIMARY_CONSTRUCTOR`), which 3.6 does not have.
    from_primary: HashSet<(usize, usize)>,
}

impl Collector<'_> {
    fn text(&self, n: ast::Name) -> String {
        self.names.resolve(n.sym).to_string()
    }

    fn declare(&mut self, u: usize, n: ast::Name) {
        self.decls.insert((u, n.span.start));
    }

    fn candidate(
        &mut self,
        u: usize,
        n: ast::Name,
        displayed: String,
        rule: Rule,
        own: Option<Span>,
    ) {
        let name = self.text(n);
        self.candidates.push(Candidate {
            unit: u,
            name,
            displayed,
            span: n.span,
            rule,
            own,
        });
    }

    /// The members of a type. `private_type`: the type (or the extension) has
    /// a private name. `extension`: extension members (public only if it is).
    #[allow(clippy::too_many_arguments)]
    fn members(
        &mut self,
        u: usize,
        ast_: &ast::Ast,
        type_name: &str,
        members: &[ast::MemberId],
        private_type: bool,
        enum_: bool,
        extension: bool,
    ) {
        let constructors = members
            .iter()
            .filter(|&&m| matches!(ast_.member(m).kind, MemberKind::Constructor(_)))
            .count();
        for &mid in members {
            let m = ast_.member(mid);
            self.augment |= m.augment;
            let annotated = is_annotated(&m.metadata, self.names);
            match &m.kind {
                MemberKind::Method(f) => {
                    let func = ast_.function(*f);
                    let Some(n) = func.name else { continue };
                    self.declare(u, n);
                    let name = self.text(n);
                    // `_isPubliclyAccessible`.
                    let accessible =
                        !is_private(&name) && !(private_type && (func.static_ || extension));
                    // Operators and `call` are used without the name (`-x`,
                    // `x[i]`, `x()`): binding by name does not see them.
                    let unnamed = func.kind == ast::FunctionKind::Operator || name == "call";
                    if !accessible && !annotated && !unnamed {
                        self.candidate(u, n, name, Rule::Element, None);
                    }
                }
                MemberKind::Field(list) => {
                    for v in list.variables.iter() {
                        self.declare(u, v.name);
                        let name = self.text(v.name);
                        // `_isReadMember`: a public one only counts if it is a
                        // static one of a private type.
                        let candidate = is_private(&name) || (list.static_ && private_type);
                        if candidate && !annotated && !self.from_primary.contains(&(u, v.name.span.start)) {
                            self.candidate(u, v.name, name, Rule::Field, None);
                        }
                    }
                }
                MemberKind::Constructor(k) => {
                    for p in k.parameters.iter() {
                        if p.this_
                            && let Some(n) = p.name
                        {
                            self.writes.insert((u, n.span.start));
                        }
                    }
                    for i in k.initializers.iter() {
                        if let Initializer::Field { name, .. } = i {
                            self.writes.insert((u, name.span.start));
                        }
                    }
                    let Some(n) = k.name else { continue };
                    self.declare(u, n);
                    let name = self.text(n);
                    // `C.new(…)` is the unnamed constructor (never reported).
                    if name == "new" {
                        continue;
                    }
                    // A public `typedef` for the type uses its public
                    // constructors (`visitGenericTypeAlias`).
                    if !is_private(&name) && self.exposed.contains(type_name) {
                        continue;
                    }
                    let accessible = !is_private(&name) && !private_type && !(enum_ && !k.factory);
                    if constructors > 1 && !accessible && !annotated && !k.primary_body_part {
                        self.candidate(u, n, format!("{type_name}.{name}"), Rule::Element, None);
                    }
                }
            }
        }
    }
}

/// Like [`unused`], with the T5 rule: a private declaration is only left out
/// when its name is cited in some fragment the parser skipped in any unit of
/// the library (`skipped`, one per unit, in the same order).
pub fn unused_with_skipped(
    units: &[Unit<'_>],
    names: &Interner,
    skipped: &[crate::Skipped<'_>],
) -> Vec<(usize, Diagnostic)> {
    let mut out = unused(units, names);
    if skipped.iter().any(|p| !p.is_empty()) {
        out.retain(|(i, d)| {
            let name = units.get(*i).and_then(|u| u.source.get(d.span.start..d.span.end)).unwrap_or("");
            !skipped.iter().any(|p| p.mentions(name))
        });
    }
    out
}

/// The diagnostics of unused private declarations of the library formed by
/// `units` (in library order), with the unit's index.
pub fn unused(units: &[Unit<'_>], names: &Interner) -> Vec<(usize, Diagnostic)> {
    // `visitGenericTypeAlias`: the type of a public `typedef`, by name.
    let mut exposed = HashSet::new();
    for un in units {
        for &d in un.unit.declarations.iter() {
            if let DeclKind::Typedef(t) = &un.ast.decl(d).kind
                && !is_private(names.resolve(t.name.sym))
                && let ast::TypedefKind::Alias(body) = t.kind
                && let ast::TypeKind::Named { name, .. } = &un.ast.ty(body).kind
                && let Some(last) = name.last()
            {
                exposed.insert(names.resolve(last.sym).to_string());
            }
        }
    }
    let mut c = Collector {
        names,
        candidates: Vec::new(),
        decls: HashSet::new(),
        writes: HashSet::new(),
        augment: false,
        exposed,
        from_primary: HashSet::new(),
    };
    for (u, un) in units.iter().enumerate() {
        let ast_ = un.ast;
        for &d in un.unit.declarations.iter() {
            let decl = ast_.decl(d);
            if decl.augment {
                return Vec::new();
            }
            let annotated = is_annotated(&decl.metadata, names);
            match &decl.kind {
                DeclKind::Class(k) => {
                    c.declare(u, k.name);
                    let name = c.text(k.name);
                    let p = is_private(&name);
                    // The class alias (`class _A = S with M;`) is not visited
                    // by the analyzer's verifier: it is never reported.
                    if p && !annotated && !k.mixin_application {
                        c.candidate(u, k.name, name.clone(), Rule::Element, Some(decl.span));
                    }
                    if let Some(k2) = k.primary_constructor
                        && let MemberKind::Constructor(k2) = &ast_.member(k2).kind
                    {
                        c.from_primary.extend(k2.parameters.iter().filter_map(|p| p.name).map(|n| (u, n.span.start)));
                    }
                    c.members(u, ast_, &name, &k.members, p, false, false);
                }
                DeclKind::Mixin(k) => {
                    c.declare(u, k.name);
                    let name = c.text(k.name);
                    let p = is_private(&name);
                    if p && !annotated {
                        c.candidate(u, k.name, name.clone(), Rule::Element, None);
                    }
                    c.members(u, ast_, &name, &k.members, p, false, false);
                }
                DeclKind::Enum(k) => {
                    c.declare(u, k.name);
                    let name = c.text(k.name);
                    let p = is_private(&name);
                    if p && !annotated {
                        c.candidate(u, k.name, name.clone(), Rule::Element, None);
                    }
                    for k2 in &k.constants {
                        c.declare(u, k2.name);
                        let n2 = c.text(k2.name);
                        if (p || is_private(&n2)) && !is_annotated(&k2.metadata, names) {
                            c.candidate(u, k2.name, n2, Rule::EnumConstant, None);
                        }
                    }
                    if k.primary_constructor.is_some() {
                        // Primary constructor (3.13): the constructor count and
                        // accessibility depend on the new form; nothing is decided.
                        continue;
                    }
                    c.members(u, ast_, &name, &k.members, p, true, false);
                }
                DeclKind::ExtensionType(k) => {
                    c.declare(u, k.name);
                    c.declare(u, k.representation_name);
                    if let Some(n) = k.constructor {
                        c.declare(u, n);
                    }
                    let name = c.text(k.name);
                    let p = is_private(&name);
                    if p && !annotated {
                        c.candidate(u, k.name, name.clone(), Rule::Element, None);
                    }
                    // The primary constructor also counts for the analyzer;
                    // here only the written ones are counted, which at most
                    // fails to report a constructor (with only one written).
                    c.members(u, ast_, &name, &k.members, p, false, false);
                }
                DeclKind::Extension(k) => {
                    let (name, p) = match k.name {
                        Some(n) => {
                            c.declare(u, n);
                            let t = c.text(n);
                            let p = is_private(&t);
                            (t, p)
                        }
                        // An unnamed extension: the accessibility of the public
                        // members depends on the empty name; only the private ones.
                        None => (String::new(), false),
                    };
                    c.members(u, ast_, &name, &k.members, p, false, true);
                }
                DeclKind::Typedef(t) => {
                    c.declare(u, t.name);
                    let name = c.text(t.name);
                    if is_private(&name) && !annotated {
                        c.candidate(u, t.name, name, Rule::Element, None);
                    }
                }
                DeclKind::Function(f) => {
                    let func = ast_.function(*f);
                    let Some(n) = func.name else { continue };
                    c.declare(u, n);
                    let name = c.text(n);
                    if is_private(&name) && !annotated {
                        c.candidate(u, n, name, Rule::Element, None);
                    }
                }
                DeclKind::Variables(list) => {
                    for v in list.variables.iter() {
                        c.declare(u, v.name);
                        let name = c.text(v.name);
                        if is_private(&name) && !annotated {
                            c.candidate(u, v.name, name, Rule::TopLevelVariable, None);
                        }
                    }
                }
            }
        }
    }
    if c.candidates.is_empty() || c.augment {
        return Vec::new();
    }

    // The identifiers of every unit.
    let mut occurrences: HashMap<&str, Vec<Occurrence>> = HashMap::new();
    for (u, un) in units.iter().enumerate() {
        let Ok(tokens) = dartforge_frontend::lexer::lex(un.source) else {
            return Vec::new();
        };
        for (i, t) in tokens.iter().enumerate() {
            if t.kind != Kind::Ident || c.decls.contains(&(u, t.span.start)) {
                continue;
            }
            let assigned = tokens
                .get(i + 1)
                .is_some_and(|p| p.kind == Kind::Op(Op::Assign));
            let write = assigned || c.writes.contains(&(u, t.span.start));
            occurrences
                .entry(t.text(un.source))
                .or_default()
                .push(Occurrence {
                    unit: u,
                    span: t.span,
                    write,
                });
        }
    }
    let values_read = occurrences.contains_key("values");

    let mut out = Vec::new();
    for k in &c.candidates {
        let used = occurrences.get(k.name.as_str()).is_some_and(|v| {
            v.iter().any(|o| {
                let inside = k.own.is_some_and(|p| {
                    o.unit == k.unit && p.start <= o.span.start && o.span.end <= p.end
                });
                let count = match k.rule {
                    Rule::Element => true,
                    Rule::TopLevelVariable | Rule::Field | Rule::EnumConstant => !o.write,
                };
                !inside && count
            })
        }) || (k.rule == Rule::EnumConstant && values_read);
        if used {
            continue;
        }
        let code = match k.rule {
            Rule::Element | Rule::TopLevelVariable => w::UNUSED_ELEMENT,
            Rule::Field | Rule::EnumConstant => w::UNUSED_FIELD,
        };
        out.push((
            k.unit,
            Diagnostic::with_code(code, k.span, [k.displayed.as_str()]),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Vec<(String, String, String)> {
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut interner);
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        let u = Unit {
            ast: &p.ast,
            unit: &p.unit,
            source,
        };
        let mut v: Vec<_> = unused(&[u], &interner)
            .into_iter()
            .map(|(_, d)| {
                (
                    d.code.unwrap().info().name.to_string(),
                    source[d.span.start..d.span.end].to_string(),
                    d.message,
                )
            })
            .collect();
        v.sort();
        v
    }

    /// Checked against `dart analyze` 3.6.2: everything it reports in this
    /// file, except the private extension's operator (used without the name;
    /// left out here, a false negative).
    #[test]
    fn matches_oracle_except_operator() {
        let source = "class _A {\n  static _A criar() => _A();\n}\n\nclass _Usada {}\n\nclass C {\n  int _lido = 0;\n  \
            int _escrito = 0;\n  int _formal;\n  int _inicializado;\n  static int _estatico = 1;\n  \
            C(this._formal) : _inicializado = 2;\n  C._um();\n  C._outro();\n  void _metodo() {}\n  void _chamado() {}\n  \
            int get _getter => 1;\n  set _setter(int v) {}\n  void f() {\n    _escrito = 3;\n    print(_lido);\n    \
            _chamado();\n    print(_Usada);\n    print(C._um);\n  }\n}\n\nenum _E { a, b }\n\nenum E2 { x, _y }\n\n\
            typedef _F = int Function();\n\nint _topo = 0;\nint _topoLido = 0;\n\nvoid _funcao() {}\n\n\
            void main() {\n  _topo = 1;\n  print(_topoLido);\n}\n\nextension _X on int {\n  int operator -() => 0;\n  \
            int get dobro => 2;\n}\n\nmixin class _M {}\n\nclass _Alias = Object with _M;\n\n\
            /// [_comentado] só no comentário.\nint _comentado = 0;\n";
        let v = run(source);
        let element = |n: &str| ("unused_element".to_string(), n.to_string());
        let field = |n: &str| ("unused_field".to_string(), n.to_string());
        let mut expected = vec![
            element("_A"),
            element("criar"),
            field("_escrito"),
            field("_formal"),
            field("_inicializado"),
            field("_estatico"),
            element("_outro"),
            element("_metodo"),
            element("_getter"),
            element("_setter"),
            element("_E"),
            field("a"),
            field("b"),
            field("_y"),
            element("_F"),
            element("_topo"),
            element("_funcao"),
            element("dobro"),
            element("_comentado"),
        ];
        expected.sort();
        let obtained: Vec<(String, String)> =
            v.iter().map(|(a, b, _)| (a.clone(), b.clone())).collect();
        assert_eq!(obtained, expected);
        let other = v.iter().find(|x| x.1 == "_outro").unwrap();
        assert_eq!(other.2, "The declaration 'C._outro' isn't referenced.");
        let e = v.iter().find(|x| x.1 == "_escrito").unwrap();
        assert_eq!(e.2, "The value of the field '_escrito' isn't used.");
    }

    /// `values` reads every constant; a public `typedef` uses the type's
    /// public constructors; a single constructor is not reported.
    #[test]
    fn values_public_typedef_and_single_constructor() {
        let source = "enum _E { a, b }\nvoid f() { print(_E.values); }\n\
            class _C { _C.um(); _C.dois(); }\ntypedef P = _C;\n\
            class D { D._so(); }\n";
        assert_eq!(run(source), vec![]);
    }

    /// With `@pragma` the declaration counts as used.
    #[test]
    fn pragma_makes_used() {
        assert_eq!(run("@pragma('vm:entry-point')\nvoid _f() {}\n"), vec![]);
    }
}
