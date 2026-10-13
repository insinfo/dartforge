//! The checks of the `package:meta` annotations
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, batch II.7), written with the
//! 6.11.0 emitters open:
//!
//! * the `AnnotationVerifier` (`analyzer/lib/src/error/annotation_verifier.dart`):
//!   `invalid_factory_method_decl`, `invalid_factory_method_impl`,
//!   `invalid_internal_annotation`, `invalid_literal_annotation`,
//!   `invalid_non_virtual_annotation`, `invalid_reopen_annotation`,
//!   `invalid_annotation_target` (only the `@redeclare` case),
//!   `undefined_referenced_parameter`, `invalid_visibility_annotation`,
//!   `invalid_visible_for_overriding_annotation` and
//!   `invalid_visible_outside_template_annotation`;
//! * from the `BestPracticesVerifier` (`best_practices_verifier.dart`):
//!   `invalid_required_named_param`, `…_optional_positional_param`,
//!   `…_positional_param`, `import_deferred_library_with_load_function`,
//!   `must_be_immutable`, `invalid_export_of_internal_element` and
//!   `invalid_export_of_internal_element_indirectly` (the
//!   `invalid_override_of_non_virtual_member` needs the
//!   `InheritanceManager3` and lives in `dartforge_types::fase_override`).
//!
//! Annotations are recognized by element, like the analyzer's
//! `ElementAnnotation`: the getter of a library named `meta` (`angular.meta`
//! for the template ones), the constructor of `meta`'s `UseResult`/`Required`
//! and that of `meta_meta`'s `Target` (`dartforge_types::anotacoes::e_getter_de`
//! and `e_construtor_de`). The general `invalid_annotation_target`
//! (`_checkKinds`) reads the `@Target({...})` of the annotation's class, on
//! every target the `BestPracticesVerifier` visits: declarations, members,
//! constants, directives, parameters (`isOptional`), type parameters, local
//! functions and variables, and the extension type representation.
//! Written without compiling or running (2026-10-04; recognition by element
//! on 2026-10-05).

use dartforge_diagnostics::codes::{hint as h, warning as w};
use dartforge_diagnostics::{Code, Diagnostic, Span};
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionRef, LibraryId, Program, UnitId, VariableRef};
use dartforge_frontend::ast::{
    self, Combinator, DeclKind, DirectiveKind, ExprKind, FunctionBody, FunctionKind, MemberKind, ParameterKind, StmtKind, TypeKind,
};
use dartforge_intern::{Interner, SymbolId};

/// Where the annotation is.
#[derive(Clone, Copy)]
enum Target<'a> {
    TopLevel(&'a ast::Decl),
    Member { member: &'a ast::Member, owner: &'a ast::Decl },
    Constant { constant: &'a ast::EnumConstant, owner: &'a ast::Decl },
    Directive,
    /// `FormalParameter`; `optional` is `isOptional` (optional positional or
    /// named without `required`).
    Param { optional: bool },
    /// `TypeParameter` (a `Declaration` with the name).
    TypeParam(ast::Name),
    /// `FunctionDeclaration` local.
    LocalFunction(&'a ast::Function),
    /// The `RepresentationDeclaration` of an extension type.
    Representation,
    /// A list of local variables, a `for-in` variable, a pattern declaration:
    /// no target kind matches.
    Other,
}

struct Ctx<'a> {
    program: &'a Program,
    interner: &'a Interner,
    a: &'a ast::Ast,
    unit: UnitId,
    in_public_api: bool,
    /// The directive being visited is the unit's first.
    first_directive: bool,
    out: Vec<(UnitId, Diagnostic)>,
}

impl<'a> Ctx<'a> {
    fn text(&self, n: ast::Name) -> &'a str {
        self.interner.resolve(n.sym)
    }

    fn report(&mut self, code: Code, span: Span, args: &[&str]) {
        self.out.push((self.unit, Diagnostic::with_code(code, span, args.iter().copied())));
    }

    /// `node.name`: the annotation's identifier (one or two parts; with three,
    /// the third is the constructor name), the position and the text.
    fn annotation_name(&self, m: &ast::Annotation) -> (Span, String) {
        let parts = if m.name.len() >= 3 { &m.name[..2] } else { &m.name[..] };
        let span = match (parts.first(), parts.last()) {
            (Some(p), Some(u)) => Span { start: p.span.start, end: u.span.end },
            _ => m.span,
        };
        (span, parts.iter().map(|n| self.text(*n)).collect::<Vec<_>>().join("."))
    }

    /// `_isPackageMetaGetter` (and the `_isTopGetter` of `angular.meta`).
    fn is_getter_of(&self, m: &ast::Annotation, library: &str, name: &str) -> bool {
        dartforge_types::anotacoes::e_getter_de(self.program, self.interner, self.unit, m, library, name)
    }

    /// `_isConstructor`.
    fn is_constructor_of(&self, m: &ast::Annotation, library: &str, class: &str) -> bool {
        dartforge_types::anotacoes::e_construtor_de(self.program, self.interner, self.unit, m, library, class)
    }

    /// The range of a `VariableDeclaration`: from the name to the end of the
    /// initializer.
    fn variable_span(&self, v: &ast::Variable) -> Span {
        Span { start: v.name.span.start, end: v.initializer.map_or(v.name.span.end, |e| self.a.expr(e).span.end) }
    }

    /// The name of the element declared by a top-level declaration.
    fn top_level_name(&self, d: &ast::Decl) -> Option<&'a str> {
        match &d.kind {
            DeclKind::Class(x) => Some(self.text(x.name)),
            DeclKind::Mixin(x) => Some(self.text(x.name)),
            DeclKind::Enum(x) => Some(self.text(x.name)),
            DeclKind::ExtensionType(x) => Some(self.text(x.name)),
            DeclKind::Extension(x) => x.name.map(|n| self.text(n)),
            DeclKind::Typedef(x) => Some(self.text(x.name)),
            DeclKind::Function(f) => self.a.function(*f).name.map(|n| self.text(n)),
            DeclKind::Variables(_) => None,
        }
    }

    /// The name of the target's element, except for variable lists.
    fn target_name(&self, target: Target<'a>) -> Option<&'a str> {
        match target {
            Target::TopLevel(d) => self.top_level_name(d),
            Target::Member { member, .. } => match &member.kind {
                MemberKind::Method(f) => self.a.function(*f).name.map(|n| self.text(n)),
                MemberKind::Constructor(k) => Some(k.name.map_or("", |n| self.text(n))),
                MemberKind::Field(_) => None,
            },
            Target::Constant { constant, .. } => Some(self.text(constant.name)),
            Target::TypeParam(n) => Some(self.text(n)),
            Target::LocalFunction(f) => f.name.map(|n| self.text(n)),
            Target::Directive | Target::Param { .. } | Target::Representation | Target::Other => None,
        }
    }

    /// The variables of a top-level list or of a field.
    fn variables(&self, target: Target<'a>) -> Option<(&'a ast::VariableList, bool)> {
        match target {
            Target::TopLevel(d) => match &d.kind {
                DeclKind::Variables(l) => Some((l, true)),
                _ => None,
            },
            Target::Member { member, .. } => match &member.kind {
                MemberKind::Field(l) => Some((l, false)),
                _ => None,
            },
            _ => None,
        }
    }

    /// `new Foo()`, `Foo()` (creation without `new`, recognized by the upper-case
    /// initial) or `null`.
    fn is_factory_expr(&self, e: ast::ExprId) -> bool {
        let upper = |n: ast::Name| self.text(n).chars().next().is_some_and(char::is_uppercase);
        match &self.a.expr(e).kind {
            ExprKind::InstanceCreation { .. } | ExprKind::Null => true,
            ExprKind::Call { target, .. } => match &self.a.expr(*target).kind {
                ExprKind::Identifier(n) => upper(*n),
                ExprKind::Property { target, .. } => matches!(&self.a.expr(*target).kind, ExprKind::Identifier(n) if upper(*n)),
                _ => false,
            },
            _ => false,
        }
    }

    /// `_checkFactory`.
    fn factory(&mut self, target: Target<'a>) {
        let Target::Member { member, .. } = target else { return };
        let MemberKind::Method(f) = &member.kind else { return };
        let func = self.a.function(*f);
        let Some(name) = func.name else { return };
        if func.return_type.is_some_and(|t| matches!(self.a.types[t.0 as usize].kind, TypeKind::Void)) {
            self.report(w::INVALID_FACTORY_METHOD_DECL, name.span, &[self.text(name)]);
            return;
        }
        let ok = match &func.body {
            // Abstract (or native): nothing to check.
            FunctionBody::Empty | FunctionBody::Native(_) => true,
            FunctionBody::Expression(e) => self.is_factory_expr(*e),
            FunctionBody::Block(s) => match &self.a.stmts[s.0 as usize].kind {
                StmtKind::Block(stmts) => stmts.last().is_some_and(|u| match &self.a.stmts[u.0 as usize].kind {
                    StmtKind::Return(Some(e)) => self.is_factory_expr(*e),
                    _ => false,
                }),
                _ => false,
            },
        };
        if !ok {
            self.report(w::INVALID_FACTORY_METHOD_IMPL, name.span, &[self.text(name)]);
        }
    }

    /// `_checkInternal`.
    fn internal(&mut self, m: &ast::Annotation, target: Target<'a>) {
        let (on_name, _) = self.annotation_name(m);
        if let Some((list, _)) = self.variables(target) {
            for v in list.variables.iter() {
                if self.text(v.name).starts_with('_') {
                    let span = self.variable_span(v);
                    self.report(w::INVALID_INTERNAL_ANNOTATION, span, &[]);
                }
            }
            return;
        }
        let private = self.target_name(target).is_some_and(|n| n.starts_with('_'));
        let private_class = match target {
            Target::Member { member, owner } if matches!(member.kind, MemberKind::Constructor(_)) => {
                self.top_level_name(owner).is_some_and(|n| n.starts_with('_'))
            }
            _ => false,
        };
        let constructor = matches!(target, Target::Member { member, .. } if matches!(member.kind, MemberKind::Constructor(_)));
        if constructor {
            if private_class || private {
                self.report(w::INVALID_INTERNAL_ANNOTATION, on_name, &[]);
            }
        } else if private || self.in_public_api {
            self.report(w::INVALID_INTERNAL_ANNOTATION, on_name, &[]);
        }
    }

    /// `_checkNonVirtual`.
    fn non_virtual(&mut self, m: &ast::Annotation, target: Target<'a>) {
        let valid = match target {
            Target::Member { member, owner } => match &member.kind {
                MemberKind::Field(l) => !l.static_,
                MemberKind::Method(f) => {
                    let func = self.a.function(*f);
                    let abstrato = matches!(func.body, FunctionBody::Empty) && !func.external;
                    !(matches!(owner.kind, DeclKind::Extension(_) | DeclKind::ExtensionType(_)) || func.static_ || abstrato)
                }
                MemberKind::Constructor(_) => false,
            },
            _ => false,
        };
        if !valid {
            let (span, _) = self.annotation_name(m);
            self.report(w::INVALID_NON_VIRTUAL_ANNOTATION, span, &[]);
        }
    }

    /// `_checkRedeclare`.
    fn redeclare(&mut self, m: &ast::Annotation, target: Target<'a>) {
        let valid = match target {
            Target::Member { member, owner } if matches!(owner.kind, DeclKind::ExtensionType(_)) => match &member.kind {
                MemberKind::Method(f) => !self.a.function(*f).static_,
                _ => true,
            },
            _ => false,
        };
        if !valid {
            let (span, text) = self.annotation_name(m);
            self.report(w::INVALID_ANNOTATION_TARGET, span, &[text.as_str(), "instance members of extension types"]);
        }
    }

    /// `_checkReopen`: only on a class, with the superclass from the same
    /// library and the modifiers the annotation reopens.
    fn reopen(&mut self, m: &ast::Annotation, class: Option<ClassId>) {
        let Some(c) = class else { return };
        let e = self.program.class(c);
        let Some(s) = e.supertype_class.map(|s| self.program.class(s)) else { return };
        if s.kind != ClassKind::Class {
            return;
        }
        let (k, sm) = (e.modifiers, s.modifiers);
        let invalid = if k.final_ || k.mixin || k.sealed || e.library != s.library {
            true
        } else if k.base {
            !sm.final_ && !sm.interface
        } else if !k.interface {
            !sm.interface
        } else {
            false
        };
        if invalid {
            let (span, _) = self.annotation_name(m);
            self.report(w::INVALID_REOPEN_ANNOTATION, span, &[]);
        }
    }

    /// `_checkUseResult`: the parameter of `UseResult.unless` exists.
    fn use_result(&mut self, m: &ast::Annotation, target: Target<'a>) {
        if m.name.len() != 2 || self.text(m.name[1]) != "unless" {
            return;
        }
        let Some(args) = &m.arguments else { return };
        let Some(arg) = args.args.iter().find(|x| x.name.is_some_and(|n| self.text(n) == "parameterDefined")) else { return };
        let ExprKind::String(lit) = &self.a.expr(arg.value).kind else { return };
        let Some(requested) = dartforge_elements::load::string_lit_value(lit) else { return };
        let function = match target {
            Target::TopLevel(d) => match &d.kind {
                DeclKind::Function(f) => Some(self.a.function(*f)),
                _ => None,
            },
            Target::Member { member, .. } => match &member.kind {
                MemberKind::Method(f) => Some(self.a.function(*f)),
                _ => None,
            },
            _ => None,
        };
        let Some(func) = function else { return };
        let (Some(name), Some(params)) = (func.name, &func.parameters) else { return };
        if params.iter().any(|p| p.name.is_some_and(|n| self.text(n) == requested)) {
            return;
        }
        let span = self.a.expr(arg.value).span;
        self.report(w::UNDEFINED_REFERENCED_PARAMETER, span, &[requested.as_str(), self.text(name)]);
    }

    /// `_checkVisibility`.
    fn visibility(&mut self, m: &ast::Annotation, target: Target<'a>, for_overriding: bool) {
        let (on_name, text) = self.annotation_name(m);
        if let Some((list, top_level)) = self.variables(target) {
            for v in list.variables.iter() {
                let name = self.text(v.name);
                if top_level {
                    if name.starts_with('_') {
                        self.report(w::INVALID_VISIBILITY_ANNOTATION, on_name, &[name, text.as_str()]);
                    }
                    // A top-level variable is not overridden.
                    if for_overriding {
                        self.report(w::INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION, on_name, &[]);
                    }
                } else {
                    if list.static_ && for_overriding {
                        self.report(w::INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION, on_name, &[]);
                    }
                    if name.starts_with('_') {
                        self.report(w::INVALID_VISIBILITY_ANNOTATION, on_name, &[name, text.as_str()]);
                    }
                }
            }
            return;
        }
        // A directive is not a declaration: nothing.
        let Some(name) = self.target_name(target) else { return };
        let instance = match target {
            Target::Member { member, owner } => match &member.kind {
                MemberKind::Method(f) => !self.a.function(*f).static_ && !matches!(owner.kind, DeclKind::ExtensionType(_)),
                _ => false,
            },
            _ => false,
        };
        if for_overriding && !instance {
            self.report(w::INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION, on_name, &[]);
        }
        if name.starts_with('_') {
            self.report(w::INVALID_VISIBILITY_ANNOTATION, on_name, &[name, text.as_str()]);
        }
    }

    /// `_checkVisibleOutsideTemplate`: on a member of a class, enum or mixin
    /// annotated with `@visibleForTemplate`.
    fn outside_template(&mut self, m: &ast::Annotation, target: Target<'a>) {
        let owner = match target {
            Target::Member { owner, .. } | Target::Constant { owner, .. } => Some(owner),
            _ => None,
        };
        let valid = owner.is_some_and(|d| {
            matches!(d.kind, DeclKind::Class(_) | DeclKind::Enum(_) | DeclKind::Mixin(_)) && d.metadata.iter().any(|x| self.is_getter_of(x, "angular.meta", "visibleForTemplate"))
        });
        if !valid {
            let (span, _) = self.annotation_name(m);
            self.report(w::INVALID_VISIBLE_OUTSIDE_TEMPLATE_ANNOTATION, span, &[]);
        }
    }

    /// The display name of a `TargetKind` (`meta_meta.dart`).
    fn kind_display(name: &str) -> Option<&'static str> {
        Some(match name {
            "classType" => "classes",
            "constructor" => "constructors",
            "directive" => "directives",
            "enumType" => "enums",
            "enumValue" => "enum values",
            "extension" => "extensions",
            "extensionType" => "extension types",
            "field" => "fields",
            "function" => "top-level functions",
            "library" => "libraries",
            "getter" => "getters",
            "method" => "methods",
            "mixinType" => "mixins",
            "optionalParameter" => "optional parameters",
            "overridableMember" => "overridable members",
            "parameter" => "parameters",
            "setter" => "setters",
            "topLevelVariable" => "top-level variables",
            "type" => "types (classes, enums, mixins, or typedefs)",
            "typedefType" => "typedefs",
            "typeParameter" => "type parameters",
            _ => return None,
        })
    }

    /// The `TargetKind`s of the `@Target({...})` of class `c` (an annotation's
    /// class): the names of the constants, in written order.
    fn class_target_kinds(&self, c: ClassId) -> Vec<&'a str> {
        let Some(d) = self.program.class(c).decl else { return Vec::new() };
        let a = &self.program.unit(d.unit).ast;
        let mut out = Vec::new();
        for m in a.decl(d.decl).metadata.iter() {
            if !dartforge_types::anotacoes::e_construtor_de(self.program, self.interner, d.unit, m, "meta_meta", "Target") {
                continue;
            }
            let Some(first) = m.arguments.as_ref().and_then(|args| args.args.iter().find(|x| x.name.is_none())) else { continue };
            let ExprKind::SetOrMap { elements, .. } = &a.expr(first.value).kind else { continue };
            for el in elements.iter() {
                if let ast::CollectionElement::Expression(e) = el
                    && let ExprKind::Property { name, .. } = &a.expr(*e).kind
                {
                    out.push(self.text(*name));
                }
            }
        }
        out
    }

    /// The class of an annotation and the name it is cited by in the message:
    /// the class of the called constructor, or that of a constant's value.
    fn annotation_class(&self, m: &ast::Annotation) -> Option<(ClassId, String)> {
        let program = self.program;
        let u = self.unit;
        let simple = |n: ast::Name| program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter);
        let (element, constructor) = match &m.name[..] {
            [n] => (simple(*n)?, None),
            [p, n] => match program.lookup_prefixed_na_unidade(u, p.sym, n.sym).and_then(|b| b.getter) {
                Some(e) => (e, None),
                // `Classe.construtor`.
                None => (simple(*p)?, Some(*n)),
            },
            [p, n, k] => (program.lookup_prefixed_na_unidade(u, p.sym, n.sym).and_then(|b| b.getter)?, Some(*k)),
            _ => return None,
        };
        match element {
            Element::Class(c) => {
                let class = self.interner.resolve(program.class(c).name);
                let name = match constructor {
                    Some(k) => format!("{class}.{}", self.text(k)),
                    None => class.to_string(),
                };
                Some((c, name))
            }
            Element::Variable(_) | Element::Function(_) => {
                // A constant (`immutable`): the class of the written type, or
                // that of the creation in the initializer.
                let v = match element {
                    Element::Variable(v) => v,
                    Element::Function(f) => program.function(f).variable?,
                    _ => return None,
                };
                let variable_span = program.variable(v);
                let VariableRef::TopLevel { unit, decl, index } = variable_span.node else { return None };
                let a = &program.unit(unit).ast;
                let DeclKind::Variables(list) = &a.decl(decl).kind else { return None };
                let type_name: ast::Name = match list.ty.map(|t| &a.types[t.0 as usize].kind) {
                    Some(TypeKind::Named { name, .. }) => *name.last()?,
                    _ => {
                        let initializer = list.variables.get(index)?.initializer?;
                        match &a.expr(initializer).kind {
                            ExprKind::InstanceCreation { ty, .. } => match &a.types[ty.0 as usize].kind {
                                TypeKind::Named { name, .. } => *name.last()?,
                                _ => return None,
                            },
                            ExprKind::Call { target, .. } => match &a.expr(*target).kind {
                                ExprKind::Identifier(n) => *n,
                                ExprKind::Property { target, .. } => match &a.expr(*target).kind {
                                    ExprKind::Identifier(n) => *n,
                                    _ => return None,
                                },
                                _ => return None,
                            },
                            _ => return None,
                        }
                    }
                };
                match program.lookup_na_unidade(unit, type_name.sym).and_then(|b| b.getter)? {
                    Element::Class(c) => Some((c, self.interner.resolve(variable_span.name).to_string())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `_checkKinds` and `_isValidTarget` (`annotation_verifier.dart:160-185`,
    /// `:505-568`): the annotation whose class has `@Target({...})` on a target
    /// that is of none of the kinds.
    fn kinds(&mut self, m: &ast::Annotation, target: Target<'a>) {
        let Some((class, name)) = self.annotation_class(m) else { return };
        let kinds = self.class_target_kinds(class);
        if kinds.is_empty() {
            return;
        }
        let has = |k: &str| kinds.contains(&k);
        // `overridableMember`: an instance field or method of a class,
        // extension type or mixin.
        let sobrescrevivel = match target {
            Target::Member { member, owner } if matches!(owner.kind, DeclKind::Class(_) | DeclKind::ExtensionType(_) | DeclKind::Mixin(_)) => {
                match &member.kind {
                    MemberKind::Field(l) => !l.static_,
                    MemberKind::Method(f) => !self.a.function(*f).static_,
                    MemberKind::Constructor(_) => false,
                }
            }
            _ => false,
        };
        let valid = (has("overridableMember") && sobrescrevivel)
            || match target {
                Target::TopLevel(d) => match &d.kind {
                    DeclKind::Class(_) => has("classType") || has("type"),
                    DeclKind::Enum(_) => has("enumType") || has("type"),
                    DeclKind::Mixin(_) => has("mixinType") || has("type"),
                    DeclKind::Typedef(_) => has("typedefType") || has("type"),
                    DeclKind::ExtensionType(_) => has("extensionType"),
                    DeclKind::Extension(_) => has("extension"),
                    DeclKind::Variables(_) => has("topLevelVariable"),
                    DeclKind::Function(f) => match self.a.function(*f).kind {
                        FunctionKind::Getter => has("getter"),
                        FunctionKind::Setter => has("setter"),
                        _ => has("function"),
                    },
                },
                Target::Member { member, .. } => match &member.kind {
                    MemberKind::Constructor(_) => has("constructor"),
                    MemberKind::Field(_) => has("field"),
                    MemberKind::Method(f) => match self.a.function(*f).kind {
                        FunctionKind::Getter => has("getter"),
                        FunctionKind::Setter => has("setter"),
                        _ => has("method"),
                    },
                },
                Target::Constant { .. } => has("enumValue"),
                Target::Directive => has("directive") || (self.first_directive && has("library")),
                Target::Param { optional } => has("parameter") || (optional && has("optionalParameter")),
                Target::TypeParam(_) => has("typeParameter"),
                Target::LocalFunction(f) => match f.kind {
                    FunctionKind::Getter => has("getter"),
                    FunctionKind::Setter => has("setter"),
                    _ => has("function"),
                },
                Target::Representation => has("parameter"),
                Target::Other => false,
            };
        if valid {
            return;
        }
        // The display names, in order, joined with commas and `or`
        // (`commaSeparatedWithOr`).
        let mut names: Vec<&str> = kinds.iter().filter_map(|k| Self::kind_display(k)).collect();
        names.sort_unstable();
        let list = match names.len() {
            0 => String::new(),
            1 => names[0].to_string(),
            2 => format!("{} or {}", names[0], names[1]),
            n => format!("{}, or {}", names[..n - 1].join(", "), names[n - 1]),
        };
        let (span, _) = self.annotation_name(m);
        self.report(w::INVALID_ANNOTATION_TARGET, span, &[name.as_str(), list.as_str()]);
    }

    /// `checkAnnotation`: an annotation, by the first role that matches, and
    /// then the target kinds (`_checkKinds`).
    fn annotation(&mut self, m: &ast::Annotation, target: Target<'a>, class: Option<ClassId>) {
        if self.is_getter_of(m, "meta", "factory") {
            self.factory(target);
        } else if self.is_getter_of(m, "meta", "internal") {
            self.internal(m, target);
        } else if self.is_getter_of(m, "meta", "literal") {
            // `_checkLiteral`: only on a `const` constructor.
            let valid = matches!(target, Target::Member { member, .. } if matches!(&member.kind, MemberKind::Constructor(k) if k.const_));
            if !valid {
                let (span, _) = self.annotation_name(m);
                self.report(w::INVALID_LITERAL_ANNOTATION, span, &[]);
            }
        } else if self.is_getter_of(m, "meta", "nonVirtual") {
            self.non_virtual(m, target);
        } else if self.is_getter_of(m, "meta", "reopen") {
            if matches!(target, Target::TopLevel(d) if matches!(d.kind, DeclKind::Class(_))) {
                self.reopen(m, class);
            }
        } else if self.is_getter_of(m, "meta", "redeclare") {
            self.redeclare(m, target);
        } else if self.is_constructor_of(m, "meta", "UseResult") || self.is_getter_of(m, "meta", "useResult") {
            self.use_result(m, target);
        } else if self.is_getter_of(m, "angular.meta", "visibleForTemplate") || self.is_getter_of(m, "meta", "visibleForTesting") {
            self.visibility(m, target, false);
        } else if self.is_getter_of(m, "meta", "visibleForOverriding") {
            self.visibility(m, target, true);
        } else if self.is_getter_of(m, "angular.meta", "visibleOutsideTemplate") {
            self.visibility(m, target, false);
            self.outside_template(m, target);
        }
        self.kinds(m, target);
    }

    /// The annotations of the parameters (and of the nested ones of a
    /// function-typed parameter, with its type parameters).
    fn param_annotations(&mut self, list: &'a [ast::Parameter]) {
        for p in list {
            let optional = match p.kind {
                ParameterKind::Optional => true,
                ParameterKind::Named => !p.required,
                ParameterKind::Required => false,
            };
            for m in p.metadata.iter() {
                self.annotation(m, Target::Param { optional }, None);
            }
            self.type_param_annotations(&p.function_type_params);
            if let Some(fs) = &p.function_parameters {
                self.param_annotations(fs);
            }
        }
    }

    /// The annotations of a type parameter list.
    fn type_param_annotations(&mut self, list: &'a [ast::TypeParameter]) {
        for tp in list {
            for m in tp.metadata.iter() {
                self.annotation(m, Target::TypeParam(tp.name), None);
            }
        }
    }

    /// `_checkRequiredParameter`: `@required` where it makes no sense.
    fn params(&mut self, list: &[ast::Parameter]) {
        for p in list {
            if !p.metadata.iter().any(|m| self.is_constructor_of(m, "meta", "Required") || self.is_getter_of(m, "meta", "required")) {
                continue;
            }
            let name = p.name.map_or("", |n| self.text(n));
            let code = match p.kind {
                ParameterKind::Optional => w::INVALID_REQUIRED_OPTIONAL_POSITIONAL_PARAM,
                ParameterKind::Required => w::INVALID_REQUIRED_POSITIONAL_PARAM,
                ParameterKind::Named if p.default_value.is_some() => w::INVALID_REQUIRED_NAMED_PARAM,
                ParameterKind::Named => continue,
            };
            self.report(code, p.span, &[name]);
        }
    }
}

fn members_of(d: &ast::Decl) -> &[ast::MemberId] {
    match &d.kind {
        DeclKind::Class(x) => &x.members,
        DeclKind::Mixin(x) => &x.members,
        DeclKind::Enum(x) => &x.members,
        DeclKind::Extension(x) => &x.members,
        DeclKind::ExtensionType(x) => &x.members,
        _ => &[],
    }
}

/// The class, or something it inherits from (mixins, interfaces,
/// superclass), is `@immutable` (`isOrInheritsImmutable`).
fn is_immutable(program: &Program, interner: &Interner, c: ClassId, seen: &mut Vec<ClassId>) -> bool {
    if seen.contains(&c) {
        return false;
    }
    seen.push(c);
    let e = program.class(c);
    let anotada = e.decl.is_some_and(|d| {
        program
            .unit(d.unit)
            .ast
            .decl(d.decl)
            .metadata
            .iter()
            .any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, d.unit, m, "meta", "immutable"))
    });
    anotada
        || e.mixin_classes.iter().any(|m| is_immutable(program, interner, *m, seen))
        || e.interface_classes.iter().any(|m| is_immutable(program, interner, *m, seen))
        || e.supertype_class.is_some_and(|s| is_immutable(program, interner, s, seen))
}

/// `Class.field` of each non-final instance field of the class.
fn non_final_fields(program: &Program, interner: &Interner, c: ClassId, out: &mut Vec<String>) {
    let e = program.class(c);
    let name = interner.resolve(e.name);
    for v in e.fields.iter().map(|v| program.variable(*v)) {
        if !v.final_ && !v.const_ && !v.static_ {
            out.push(format!("{name}.{}", interner.resolve(v.name)));
        }
    }
}

/// The annotations of a top-level element's declaration (those of the
/// variable, for an implicit accessor).
fn top_level_annotations(program: &Program, e: Element) -> &[ast::Annotation] {
    let from_decl = |unit: UnitId, decl: ast::DeclId| &program.unit(unit).ast.decl(decl).metadata[..];
    match e {
        Element::Class(c) => program.class(c).decl.map_or(&[][..], |d| from_decl(d.unit, d.decl)),
        Element::Typedef(t) => {
            let d = program.typedef(t).decl;
            from_decl(d.unit, d.decl)
        }
        Element::Extension(x) => {
            let d = program.extension(x).decl;
            from_decl(d.unit, d.decl)
        }
        Element::Variable(v) => match program.variable(v).node {
            VariableRef::TopLevel { unit, decl, .. } => from_decl(unit, decl),
            _ => &[],
        },
        Element::Function(f) => {
            let element = program.function(f);
            if let Some(v) = element.variable {
                return top_level_annotations(program, Element::Variable(v));
            }
            match element.node {
                FunctionRef::Function { unit, function } => program
                    .unit(unit)
                    .ast
                    .decls
                    .iter()
                    .find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function))
                    .map_or(&[][..], |d| &d.metadata[..]),
                _ => &[],
            }
        }
        Element::Prefix(..) => &[],
    }
}

/// `_checkForInternalExport` (`best_practices_verifier.dart:952-990`): in a
/// public API library, the `export` that brings an `@internal` element
/// (`invalid_export_of_internal_element`), or a function whose signature uses
/// an `@internal` `typedef` (`invalid_export_of_internal_element_indirectly`).
/// The bounds of the function's type parameters are not looked at.
fn internal_exports(program: &Program, lib: LibraryId, interner: &Interner, out: &mut Vec<(UnitId, Diagnostic)>) {
    let Some(internal_sym) = interner.lookup("internal") else { return };
    let internal = |m: &[ast::Annotation]| m.iter().any(|x| x.arguments.is_none() && x.name.last().is_some_and(|n| n.sym == internal_sym));
    let library = program.library(lib);
    for exp in library.exports.iter() {
        let Some(directive) = program.unit(exp.unit).unit.directives.get(exp.directive) else { continue };
        let target = program.library(exp.library);
        // The exported library itself is `@internal`.
        let from_library = target
            .units
            .first()
            .and_then(|u| program.unit(*u).unit.directives.iter().find(|d| matches!(d.kind, DirectiveKind::Library { .. })));
        if let Some(d) = from_library
            && internal(&d.metadata)
        {
            let name = target.name.as_ref().map(|parts| parts.iter().map(|s| interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default();
            out.push((exp.unit, Diagnostic::with_code(w::INVALID_EXPORT_OF_INTERNAL_ELEMENT, directive.span, [name.as_str()])));
        }
        // The namespace the directive exports, in name order.
        let mut names: Vec<SymbolId> = target
            .exported
            .keys()
            .copied()
            .filter(|k| {
                exp.combinators.iter().all(|c| match c {
                    Combinator::Show(ns) => ns.iter().any(|n| n.sym == *k),
                    Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == *k),
                })
            })
            .collect();
        names.sort_by_key(|k| interner.resolve(*k).to_string());
        for k in names {
            let Some(binding) = target.exported.get(&k) else { continue };
            let text = interner.resolve(k);
            for e in binding.getter.into_iter().chain(binding.setter.filter(|s| Some(*s) != binding.getter)) {
                if internal(top_level_annotations(program, e)) {
                    out.push((exp.unit, Diagnostic::with_code(w::INVALID_EXPORT_OF_INTERNAL_ELEMENT, directive.span, [text])));
                    continue;
                }
                // A function: the written types of the signature that are an
                // `@internal` `typedef`.
                let Element::Function(f) = e else { continue };
                let FunctionRef::Function { unit, function } = program.function(f).node else { continue };
                let a = &program.unit(unit).ast;
                let func = a.function(function);
                let types = func.parameters.iter().flat_map(|ps| ps.iter()).filter_map(|p| p.ty).chain(func.return_type);
                for t in types {
                    let ast::TypeKind::Named { name, .. } = &a.types[t.0 as usize].kind else { continue };
                    let found = match &name[..] {
                        [n] => program.lookup_na_unidade(unit, n.sym),
                        [p, n] => program.lookup_prefixed_na_unidade(unit, p.sym, n.sym),
                        _ => None,
                    };
                    if let Some(alias @ Element::Typedef(td)) = found.and_then(|b| b.getter)
                        && internal(top_level_annotations(program, alias))
                    {
                        let alias_name = interner.resolve(program.typedef(td).name);
                        out.push((
                            exp.unit,
                            Diagnostic::with_code(w::INVALID_EXPORT_OF_INTERNAL_ELEMENT_INDIRECTLY, directive.span, [alias_name, text]),
                        ));
                    }
                }
            }
        }
    }
}

/// The reports of the `package:meta` annotations in library `lib`.
/// `in_public_api`: the library belongs to the package's public API (it is in
/// `lib/`, outside `lib/src/`).
/// `ElementAnnotation.targetKinds` of an annotation of unit `u`: the kinds of
/// the `@Target({...})` of the annotation's class (that of the called
/// constructor, or that of the constant's value), without repetition.
pub(crate) fn annotation_target_kinds(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> Vec<String> {
    let ctx = Ctx { program, interner, a: &program.unit(u).ast, unit: u, in_public_api: false, first_directive: false, out: Vec::new() };
    let Some((class, _)) = ctx.annotation_class(m) else { return Vec::new() };
    let mut v: Vec<String> = Vec::new();
    for k in ctx.class_target_kinds(class) {
        if is_known_display(k) && !v.iter().any(|x| x == k) {
            v.push(k.to_string());
        }
    }
    v
}

/// The name is that of a `TargetKind` (`_targetKindsByName` drops the others).
fn is_known_display(name: &str) -> bool {
    Ctx::kind_display(name).is_some()
}

pub fn check(program: &Program, lib: LibraryId, interner: &Interner, in_public_api: bool) -> Vec<(UnitId, Diagnostic)> {
    let library = program.library(lib);
    let mut out: Vec<(UnitId, Diagnostic)> = Vec::new();
    // This library's classes, by declaration.
    let class_of = |u: UnitId, d: ast::DeclId| {
        program.classes.iter().position(|c| c.library == lib && c.decl.is_some_and(|x| x.unit == u && x.decl == d)).map(|i| ClassId(i as u32))
    };
    for &u in &library.units {
        let unit = program.unit(u);
        let a = &unit.ast;
        let mut ctx = Ctx { program, interner, a, unit: u, in_public_api, first_directive: false, out: Vec::new() };
        for (index, directive) in unit.unit.directives.iter().enumerate() {
            ctx.first_directive = index == 0;
            for m in directive.metadata.iter() {
                ctx.annotation(m, Target::Directive, None);
            }
        }
        for &d in unit.unit.declarations.iter() {
            let decl = a.decl(d);
            let class = class_of(u, d);
            for m in decl.metadata.iter() {
                ctx.annotation(m, Target::TopLevel(decl), class);
            }
            if let DeclKind::Enum(x) = &decl.kind {
                for constant in x.constants.iter() {
                    for m in constant.metadata.iter() {
                        ctx.annotation(m, Target::Constant { constant, owner: decl }, None);
                    }
                }
            }
            for &mid in members_of(decl) {
                let member = a.member(mid);
                for m in member.metadata.iter() {
                    ctx.annotation(m, Target::Member { member, owner: decl }, None);
                }
                if let MemberKind::Constructor(k) = &member.kind {
                    ctx.params(&k.parameters);
                }
            }
            // `must_be_immutable`: class, mixin application or mixin.
            let name = match &decl.kind {
                DeclKind::Class(x) => Some(x.name),
                DeclKind::Mixin(x) => Some(x.name),
                _ => None,
            };
            if let (Some(name), Some(c)) = (name, class)
                && is_immutable(program, interner, c, &mut Vec::new())
            {
                let mut fields: Vec<String> = Vec::new();
                let mut seen: Vec<ClassId> = Vec::new();
                let mut current = Some(c);
                while let Some(x) = current {
                    if seen.contains(&x) {
                        break;
                    }
                    seen.push(x);
                    non_final_fields(program, interner, x, &mut fields);
                    for m in program.class(x).mixin_classes.iter() {
                        non_final_fields(program, interner, *m, &mut fields);
                    }
                    current = program.class(x).supertype_class;
                }
                if !fields.is_empty() {
                    let list = fields.join(", ");
                    ctx.report(w::MUST_BE_IMMUTABLE, name.span, &[list.as_str()]);
                }
            }
        }
        // `@required` in the parameter lists of functions and methods.
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                ctx.params(ps);
            }
        }
        // `_checkKinds` on the targets that are neither members nor top-level
        // declarations: parameters, type parameters, local functions and
        // variables, the extension type representation.
        for &d in unit.unit.declarations.iter() {
            let decl = a.decl(d);
            match &decl.kind {
                DeclKind::Class(x) => ctx.type_param_annotations(&x.type_params),
                DeclKind::Mixin(x) => ctx.type_param_annotations(&x.type_params),
                DeclKind::Enum(x) => ctx.type_param_annotations(&x.type_params),
                DeclKind::Extension(x) => ctx.type_param_annotations(&x.type_params),
                DeclKind::ExtensionType(x) => {
                    ctx.type_param_annotations(&x.type_params);
                    for m in x.representation_metadata.iter() {
                        ctx.annotation(m, Target::Representation, None);
                    }
                }
                DeclKind::Typedef(x) => {
                    ctx.type_param_annotations(&x.type_params);
                    if let ast::TypedefKind::Legacy { parameters, .. } = &x.kind {
                        ctx.param_annotations(parameters);
                    }
                }
                _ => {}
            }
            for &mid in members_of(decl) {
                if let MemberKind::Constructor(k) = &a.member(mid).kind {
                    ctx.param_annotations(&k.parameters);
                }
            }
        }
        for f in a.functions.iter() {
            ctx.type_param_annotations(&f.type_params);
            if let Some(ps) = &f.parameters {
                ctx.param_annotations(ps);
            }
        }
        for t in a.types.iter() {
            if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
                ctx.type_param_annotations(type_params);
                ctx.param_annotations(parameters);
            }
        }
        for (sid, meta) in a.metadados_locais.iter() {
            let target = match &a.stmt(*sid).kind {
                StmtKind::Function(f) => Target::LocalFunction(a.function(*f)),
                _ => Target::Other,
            };
            for m in meta.iter() {
                ctx.annotation(m, target, None);
            }
        }
        for s in a.stmts.iter() {
            if let StmtKind::ForIn { target: ast::ForInTarget::Declared { metadata, .. }, .. } = &s.kind {
                for m in metadata.iter() {
                    ctx.annotation(m, Target::Other, None);
                }
            }
        }
        out.append(&mut ctx.out);
    }
    if in_public_api {
        internal_exports(program, lib, interner, &mut out);
    }
    // `import_deferred_library_with_load_function`: the deferred import of a
    // library that exports `loadLibrary`, on the whole directive.
    if let Some(sym) = interner.lookup("loadLibrary") {
        for imp in library.imports.iter().filter(|i| i.deferred && i.prefix.is_some()) {
            let exports = program.library(imp.library).exported.contains_key(&sym)
                && imp.combinators.iter().all(|c| match c {
                    Combinator::Show(ns) => ns.iter().any(|n| n.sym == sym),
                    Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == sym),
                });
            let directive = program.unit(imp.unit).unit.directives.get(imp.directive);
            if let (true, Some(d)) = (exports, directive)
                && matches!(d.kind, DirectiveKind::Import { .. })
            {
                out.push((imp.unit, Diagnostic::with_code(h::IMPORT_DEFERRED_LIBRARY_WITH_LOAD_FUNCTION, d.span, std::iter::empty::<&str>())));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the checks that do not depend on the program: a context with the
    /// tree of a loose unit cannot be built without a `Program`, so the tests
    /// here stay on the pure functions.
    #[test]
    fn members_of_decl_without_members() {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse("void f() {}\n", &mut names);
        assert!(members_of(p.ast.decl(p.unit.declarations[0])).is_empty());
    }
}
