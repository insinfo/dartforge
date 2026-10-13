//! The analyzer's `SdkConstraintVerifier`
//! (`analyzer/lib/src/hint/sdk_constraint_verifier.dart`, read in full in
//! 6.11.0; docs/ANALYZER-ESPECIFICACAO-INFRA.md §6.4 and batch II.8):
//!
//! * `sdk_version_gt_gt_gt_operator`: the `>>>` operator, used or declared,
//!   when the SDK constraint in `pubspec.yaml` admits a version earlier than
//!   2.14.0;
//! * `sdk_version_since`: a reference to an SDK element annotated with
//!   `@Since('x.y')` (on itself or on its enclosing class) that the
//!   constraint does not guarantee.
//!
//! Left out: positional arguments passed to parameters with `@Since`, index
//! and assignment operators, function expression invocation, and the
//! `index` property (the original only reports it when the target is
//! exactly `Enum`). A union constraint (`||`) is not read.
//! Written without compiling or running (2026-10-04).

use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionRef, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, BinaryOp, DeclKind, ExprKind, FunctionKind, MemberKind, TypeKind};
use dartforge_intern::Interner;

/// A `major.minor.patch` version, without pre-release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// `2.15`, `2.15.0`, `3.0.0-dev.1`: missing parts are zero; whatever comes
    /// after `-` or `+` is discarded.
    pub fn de_texto(text: &str) -> Option<Version> {
        let core_lib = text.trim().split(['-', '+']).next()?;
        let mut parts = core_lib.split('.').map(|p| p.parse::<u32>());
        let greater = parts.next()?.ok()?;
        let less = parts.next().unwrap_or(Ok(0)).ok()?;
        let fix = parts.next().unwrap_or(Ok(0)).ok()?;
        Some(Version(greater, less, fix))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The `environment: sdk:` constraint of `pubspec.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestricaoDeSdk {
    /// The minimum (from `^v`, `>=v`, `>v` or the exact version); `None` without
    /// a lower bound.
    pub minimo: Option<Version>,
    /// The constraint text, with normalized spaces (the `toString`).
    pub texto: String,
}

impl RestricaoDeSdk {
    /// `^3.6.0`, `>=2.12.0 <4.0.0`, `any`, `3.6.0`. `None` for what cannot be
    /// read (unions with `||`, invalid text): the verifier does not run.
    pub fn de_texto(text: &str) -> Option<RestricaoDeSdk> {
        let normal = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if normal.contains("||") {
            return None;
        }
        if normal.is_empty() || normal == "any" {
            return Some(RestricaoDeSdk { minimo: None, texto: "any".to_string() });
        }
        let mut minimum = None;
        for parte in normal.split(' ') {
            if let Some(v) = parte.strip_prefix('^').or_else(|| parte.strip_prefix(">=")).or_else(|| parte.strip_prefix('>')) {
                minimum = Some(Version::de_texto(v)?);
            } else if let Some(v) = parte.strip_prefix("<=").or_else(|| parte.strip_prefix('<')) {
                Version::de_texto(v)?;
            } else {
                minimum = Some(Version::de_texto(parte)?);
            }
        }
        Some(RestricaoDeSdk { minimo: minimum, texto: normal })
    }

    /// `requiresAtLeast`: the constraint's minimum is at least `version`.
    fn guarantees(&self, version: Version) -> bool {
        self.minimo.is_some_and(|m| m >= version)
    }

    /// `checkTripleShift`: the constraint admits a version earlier than 2.14.0.
    fn before_triple_shift(&self) -> bool {
        self.minimo.is_none_or(|m| m < Version(2, 14, 0))
    }
}

/// The `@Since('x.y')` version from a list of annotations.
fn desde(metadata: &[ast::Annotation], a: &ast::Ast, interner: &Interner) -> Option<Version> {
    metadata.iter().find_map(|m| {
        let args = m.arguments.as_ref()?;
        if m.name.last().map(|n| interner.resolve(n.sym)) != Some("Since") {
            return None;
        }
        let first = args.args.iter().find(|x| x.name.is_none())?;
        match &a.expr(first.value).kind {
            ExprKind::String(lit) => Version::de_texto(&dartforge_elements::load::string_lit_value(lit)?),
            _ => None,
        }
    })
}

fn since_of_class(program: &Program, interner: &Interner, c: ClassId) -> Option<Version> {
    let e = program.class(c);
    if !program.library(e.library).uri.starts_with("dart:") {
        return None;
    }
    let d = e.decl?;
    let a = &program.unit(d.unit).ast;
    desde(&a.decl(d.decl).metadata, a, interner)
}

fn since_of_variable(program: &Program, interner: &Interner, v: VariableId) -> Option<Version> {
    let e = program.variable(v);
    if !program.library(e.library).uri.starts_with("dart:") {
        return None;
    }
    let own = match e.node {
        VariableRef::TopLevel { unit, decl, .. } => {
            let a = &program.unit(unit).ast;
            desde(&a.decl(decl).metadata, a, interner)
        }
        VariableRef::Field { unit, member, .. } => {
            let a = &program.unit(unit).ast;
            desde(&a.member(member).metadata, a, interner)
        }
        _ => None,
    };
    // `sinceSdkVersion`: the greater of the element's and its container's.
    own.max(e.class.and_then(|c| since_of_class(program, interner, c)))
}

fn since_of_function(program: &Program, interner: &Interner, f: FunctionElementId) -> Option<Version> {
    let e = program.function(f);
    if let Some(v) = e.variable {
        return since_of_variable(program, interner, v);
    }
    if !program.library(e.library).uri.starts_with("dart:") {
        return None;
    }
    let own = match e.node {
        FunctionRef::Constructor { unit, member } => {
            let a = &program.unit(unit).ast;
            desde(&a.member(member).metadata, a, interner)
        }
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            let of_member = a.members.iter().find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function)).map(|m| &m.metadata[..]);
            let of_declaration = || a.decls.iter().find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function)).map(|d| &d.metadata[..]);
            of_member.or_else(of_declaration).and_then(|m| desde(m, a, interner))
        }
        FunctionRef::None => None,
    };
    own.max(e.class.and_then(|c| since_of_class(program, interner, c)))
}

/// The `SdkConstraintVerifier` reports in unit `u`.
pub fn restricao_de_sdk(
    program: &Program,
    interner: &Interner,
    body: Option<&UnitBodyTypes>,
    u: UnitId,
    constraint: &RestricaoDeSdk,
) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let unit_id = program.unit(u);
    let a = &unit_id.ast;
    // `>>>`: use and declaration of the operator.
    if constraint.before_triple_shift() {
        for e in a.exprs.iter() {
            if let ExprKind::Binary { op: BinaryOp::UShr, left, right } = &e.kind {
                let (of, until) = (a.expr(*left).span.end, a.expr(*right).span.start);
                if let Some(k) = unit_id.source.get(of..until).and_then(|t| t.find(">>>")) {
                    out.push(Diagnostic::com_codigo(w::SDK_VERSION_GT_GT_GT_OPERATOR, Span { start: of + k, end: of + k + 3 }, std::iter::empty::<&str>()));
                }
            }
        }
        for f in a.functions.iter() {
            if let (FunctionKind::Operator, Some(n)) = (f.kind, f.name)
                && interner.resolve(n.sym) == ">>>"
            {
                out.push(Diagnostic::com_codigo(w::SDK_VERSION_GT_GT_GT_OPERATOR, n.span, std::iter::empty::<&str>()));
            }
        }
    }
    // `_checkSinceSdkVersion`.
    let mut report = |version: Option<Version>, span: Span| {
        let Some(v) = version else { return };
        if constraint.guarantees(v) {
            return;
        }
        // The listener is a set: one report per position.
        if !out.iter().any(|x| x.code == Some(w::SDK_VERSION_SINCE) && x.span == span) {
            let text = v.to_string();
            out.push(Diagnostic::com_codigo(w::SDK_VERSION_SINCE, span, [text.as_str(), constraint.texto.as_str()]));
        }
    };
    // Named types: the SDK class, at the name.
    for t in a.types.iter() {
        let TypeKind::Named { name, .. } = &t.kind else { continue };
        let binding = match &name[..] {
            [n] => program.lookup_na_unidade(u, n.sym),
            [p, n] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        };
        if let (Some(Element::Class(c)), Some(last)) = (binding.and_then(|b| b.getter), name.last()) {
            report(since_of_class(program, interner, c), last.span);
        }
    }
    if let Some(body) = body {
        for (i, expr) in a.exprs.iter().enumerate() {
            let Some(Some(r)) = body.resolved.get(i) else { continue };
            let at_name = match &expr.kind {
                ExprKind::Identifier(n) => n,
                ExprKind::Property { name, .. } => name,
                _ => continue,
            };
            let version = match r {
                Resolved::Constructor(f) => since_of_function(program, interner, *f),
                Resolved::Element(Element::Class(c)) => since_of_class(program, interner, *c),
                Resolved::Element(Element::Function(f)) => since_of_function(program, interner, *f),
                Resolved::Element(Element::Variable(v)) => since_of_variable(program, interner, *v),
                // `_shouldReportEnumIndex`: `index` only with a target that is exactly
                // `Enum`, which is not checked here.
                Resolved::Member { .. } | Resolved::ExtensionMember { .. } if interner.resolve(at_name.sym) == "index" => None,
                Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
                    since_of_function(program, interner, *f)
                }
                Resolved::Member { member: MemberRef::Variable(v), .. } => since_of_variable(program, interner, *v),
                _ => None,
            };
            report(version, at_name.span);
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn restricoes() {
        let caret = RestricaoDeSdk::de_texto("^3.6.0").unwrap();
        assert_eq!(caret.minimo, Some(Version(3, 6, 0)));
        assert!(caret.guarantees(Version(2, 15, 0)) && !caret.guarantees(Version(3, 7, 0)));
        assert!(!caret.before_triple_shift());
        let range = RestricaoDeSdk::de_texto(">=2.12.0   <4.0.0").unwrap();
        assert_eq!(range.texto, ">=2.12.0 <4.0.0");
        assert!(range.before_triple_shift() && !range.guarantees(Version(2, 15, 0)));
        assert!(RestricaoDeSdk::de_texto("any").unwrap().before_triple_shift());
        assert!(RestricaoDeSdk::de_texto(">=2.0.0 <3.0.0 || >=3.1.0").is_none());
        assert_eq!(Version::de_texto("2.15"), Some(Version(2, 15, 0)));
        assert_eq!(Version::de_texto("3.0.0-dev.1"), Some(Version(3, 0, 0)));
    }
}
