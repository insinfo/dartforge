//! The parent of each expression of a unit, in the role the analyzer's
//! ancestor queries distinguish, and 3.6.2's `inConstantContext`
//! (`ExpressionImpl.constantContext`, `analyzer/lib/src/dart/ast/ast.dart:6208`).
//!
//! `constantContext` climbs from the expression's parent: it goes through
//! expressions, argument lists, collection `if`/`for` elements (through the
//! body and the `if` condition), map entries, `...` and variable
//! declarations; it stops with true at an annotation, at enum constant
//! arguments, at a creation/collection/record with `const` written, at a
//! `const` variable list, at a constant pattern with `const` and at the old
//! `case` (`SwitchCase`, library before 3.0); it stops with false at a
//! variable list without `const`, at a constant pattern without `const` and
//! at any other parent (statement, function body, interpolation, default
//! value, constructor initializer, `for` parts, `when` guard, `switch` case
//! body, `?e` element).
//! Written without compiling or running (2026-10-05).

use crate::ast::{
    Annotation, Arguments, Ast, CollectionElement, CompilationUnit, CreationKeyword, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, MemberKind,
    Parameter, PatternId, PatternKind, StmtKind, TypeKind, TypeParameter, TypedefKind, VariableList,
};

/// Every annotation written in the unit (what a recursive visitor's
/// `visitAnnotation` would see): those of directives, declarations and
/// members, enum constants, the extension type representation, type
/// parameters, parameters (including those of `Function` types and nested
/// ones), local declarations and `for-in` variables.
pub fn todas_as_anotacoes<'a>(a: &'a Ast, unit: &'a CompilationUnit) -> Vec<&'a Annotation> {
    let mut v: Vec<&'a Annotation> = Vec::new();
    fn parameters_<'a>(ps: &'a [Parameter], v: &mut Vec<&'a Annotation>) {
        for p in ps {
            v.extend(p.metadata.iter());
            for t in p.function_type_params.iter() {
                v.extend(t.metadata.iter());
            }
            if let Some(fs) = &p.function_parameters {
                parameters_(fs, v);
            }
        }
    }
    fn types_<'a>(ts: &'a [TypeParameter], v: &mut Vec<&'a Annotation>) {
        for t in ts {
            v.extend(t.metadata.iter());
        }
    }
    for d in unit.directives.iter() {
        v.extend(d.metadata.iter());
    }
    for d in a.decls.iter() {
        v.extend(d.metadata.iter());
        match &d.kind {
            DeclKind::Class(x) => types_(&x.type_params, &mut v),
            DeclKind::Mixin(x) => types_(&x.type_params, &mut v),
            DeclKind::Enum(x) => {
                types_(&x.type_params, &mut v);
                for c in x.constants.iter() {
                    v.extend(c.metadata.iter());
                }
            }
            DeclKind::Extension(x) => types_(&x.type_params, &mut v),
            DeclKind::ExtensionType(x) => {
                types_(&x.type_params, &mut v);
                v.extend(x.representation_metadata.iter());
            }
            DeclKind::Typedef(t) => {
                types_(&t.type_params, &mut v);
                if let TypedefKind::Legacy { parameters, .. } = &t.kind {
                    parameters_(parameters, &mut v);
                }
            }
            DeclKind::Function(_) | DeclKind::Variables(_) => {}
        }
    }
    for m in a.members.iter() {
        v.extend(m.metadata.iter());
        if let MemberKind::Constructor(k) = &m.kind {
            parameters_(&k.parameters, &mut v);
        }
    }
    for f in a.functions.iter() {
        types_(&f.type_params, &mut v);
        if let Some(ps) = &f.parameters {
            parameters_(ps, &mut v);
        }
    }
    for t in a.types.iter() {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
            types_(type_params, &mut v);
            parameters_(parameters, &mut v);
        }
    }
    for (_, ms) in a.metadados_locais.iter() {
        v.extend(ms.iter());
    }
    for s in a.stmts.iter() {
        if let StmtKind::ForIn { target: ForInTarget::Declared { metadata, .. }, .. } = &s.kind {
            v.extend(metadata.iter());
        }
    }
    for e in a.exprs.iter() {
        if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
            fn from_element<'a>(el: &'a CollectionElement, v: &mut Vec<&'a Annotation>) {
                match el {
                    CollectionElement::ForIn { target, body, .. } => {
                        if let ForInTarget::Declared { metadata, .. } = target {
                            v.extend(metadata.iter());
                        }
                        from_element(body, v);
                    }
                    CollectionElement::For { body, .. } => from_element(body, v),
                    CollectionElement::If { then, else_, .. } => {
                        from_element(then, v);
                        if let Some(x) = else_ {
                            from_element(x, v);
                        }
                    }
                    _ => {}
                }
            }
            for el in elements.iter() {
                from_element(el, &mut v);
            }
        }
    }
    v
}

/// The role of an expression's parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pai {
    /// A parent that `constantContext` does not go through (and one not seen).
    Fronteira,
    /// An expression that contains it through a transparent role (operand,
    /// target, argument, collection element, collection `if` condition, record
    /// field, cascade section…).
    Expr(ExprId),
    /// Annotation argument.
    Anotacao,
    /// Enum constant argument (`EnumConstantArguments`).
    ConstanteDeEnum,
    /// Variable initializer of a list (`const` written or not).
    Variavel { constant: bool },
    /// Expression of a constant pattern (`const` written or not) or of the old
    /// `case` (constant).
    PadraoConstante { constant: bool },
}

/// The parents of a unit's expressions, by `ExprId`.
pub struct Pais {
    pais: Vec<Pai>,
}

impl Pais {
    /// `before_3`: the library predates 3.0 (the `switch`'s `case e:` is the
    /// old `SwitchCase`, a constant context).
    pub fn novo(a: &Ast, unit: &CompilationUnit, fonte: &str, before_3: bool) -> Pais {
        let mut p = Constructor { a, fonte, pais: vec![Pai::Fronteira; a.exprs.len()] };
        p.everything(unit, before_3);
        Pais { pais: p.pais }
    }

    pub fn pai(&self, e: ExprId) -> Pai {
        self.pais.get(e.0 as usize).copied().unwrap_or(Pai::Fronteira)
    }

    /// `Expression.inConstantContext`.
    pub fn em_contexto_constante(&self, a: &Ast, e: ExprId) -> bool {
        let mut atual = self.pai(e);
        // The tree is finite; the limit only guards against a cycle caused by a
        // bug.
        for _ in 0..a.exprs.len() + 1 {
            match atual {
                Pai::Fronteira => return false,
                Pai::Anotacao | Pai::ConstanteDeEnum => return true,
                Pai::Variavel { constant } | Pai::PadraoConstante { constant } => return constant,
                Pai::Expr(p) => {
                    let constant = match &a.expr(p).kind {
                        ExprKind::InstanceCreation { keyword, .. } => *keyword == Some(CreationKeyword::Const),
                        ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } | ExprKind::Record { const_, .. } => *const_,
                        _ => false,
                    };
                    if constant {
                        return true;
                    }
                    atual = self.pai(p);
                }
            }
        }
        false
    }
}

struct Constructor<'a> {
    a: &'a Ast,
    fonte: &'a str,
    pais: Vec<Pai>,
}

impl Constructor<'_> {
    fn set_parent(&mut self, child: ExprId, pai: Pai) {
        if let Some(x) = self.pais.get_mut(child.0 as usize) {
            *x = pai;
        }
    }

    fn arguments_(&mut self, args: &Arguments, pai: Pai) {
        for x in args.args.iter() {
            self.set_parent(x.value, pai);
        }
    }

    fn annotations(&mut self, ms: &[Annotation]) {
        for m in ms {
            if let Some(args) = &m.arguments {
                self.arguments_(args, Pai::Anotacao);
            }
        }
    }

    fn type_parameters_(&mut self, ps: &[TypeParameter]) {
        for p in ps {
            self.annotations(&p.metadata);
        }
    }

    fn parameters_(&mut self, ps: &[Parameter]) {
        for p in ps {
            self.annotations(&p.metadata);
            self.type_parameters_(&p.function_type_params);
            if let Some(fs) = &p.function_parameters {
                self.parameters_(fs);
            }
        }
    }

    fn list(&mut self, l: &VariableList) {
        for v in l.variables.iter() {
            if let Some(i) = v.initializer {
                self.set_parent(i, Pai::Variavel { constant: l.const_ });
            }
        }
    }

    /// The collection elements of the literal `owner`.
    fn elements_(&mut self, owner: ExprId, es: &[CollectionElement]) {
        for e in es {
            self.element(owner, e);
        }
    }

    fn element(&mut self, owner: ExprId, e: &CollectionElement) {
        let pai = Pai::Expr(owner);
        match e {
            CollectionElement::Expression(x) => self.set_parent(*x, pai),
            // `NullAwareElement` is not traversed.
            CollectionElement::NullAwareExpression(_) => {}
            CollectionElement::MapEntry { key, value, .. } => {
                self.set_parent(*key, pai);
                self.set_parent(*value, pai);
            }
            CollectionElement::Spread { value, .. } => self.set_parent(*value, pai),
            CollectionElement::If { condition, then, else_, .. } => {
                // The condition is a child of the `IfElement`; the pattern and the guard
                // are not.
                self.set_parent(*condition, pai);
                self.element(owner, then);
                if let Some(x) = else_ {
                    self.element(owner, x);
                }
            }
            CollectionElement::For { init, body, .. } => {
                // The loop parts are children of the `ForParts…`: a boundary; their
                // variable list follows its own rule.
                if let Some(ForInit::Variables(l)) = init {
                    self.list(l);
                }
                self.element(owner, body);
            }
            CollectionElement::ForIn { target, body, .. } => {
                if let ForInTarget::Declared { metadata, .. } = target {
                    self.annotations(metadata);
                }
                self.element(owner, body);
            }
        }
    }

    fn constant_pattern(&mut self, p: PatternId, old_case: bool) {
        let pt = self.a.pattern(p);
        if let PatternKind::Constant(e) = &pt.kind {
            let written = crate::fonte::palavra_em(self.fonte, pt.span.start, "const");
            self.set_parent(*e, Pai::PadraoConstante { constant: written || old_case });
        }
    }

    fn everything(&mut self, unit: &CompilationUnit, before_3: bool) {
        let a = self.a;
        for d in unit.directives.iter() {
            self.annotations(&d.metadata);
        }
        // The children of each expression.
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let pai = Pai::Expr(id);
            match &e.kind {
                ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.set_parent(*x, pai),
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => self.elements_(id, elements),
                ExprKind::Record { positional, named, .. } => {
                    for &x in positional.iter() {
                        self.set_parent(x, pai);
                    }
                    for (_, x) in named.iter() {
                        self.set_parent(*x, pai);
                    }
                }
                ExprKind::InstanceCreation { arguments, .. } => self.arguments_(arguments, pai),
                ExprKind::Property { target, .. } | ExprKind::TypeArguments { target, .. } => self.set_parent(*target, pai),
                ExprKind::Index { target, index, .. } => {
                    self.set_parent(*target, pai);
                    self.set_parent(*index, pai);
                }
                ExprKind::Call { target, arguments } => {
                    self.set_parent(*target, pai);
                    self.arguments_(arguments, pai);
                }
                ExprKind::Unary { operand, .. } => self.set_parent(*operand, pai),
                ExprKind::Binary { left, right, .. } => {
                    self.set_parent(*left, pai);
                    self.set_parent(*right, pai);
                }
                ExprKind::Conditional { condition, then, else_ } => {
                    self.set_parent(*condition, pai);
                    self.set_parent(*then, pai);
                    self.set_parent(*else_, pai);
                }
                ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.set_parent(*value, pai),
                ExprKind::Assign { target, value, .. } => {
                    self.set_parent(*target, pai);
                    self.set_parent(*value, pai);
                }
                ExprKind::PatternAssign { value, .. } => self.set_parent(*value, pai),
                ExprKind::Cascade { target, sections, .. } => {
                    self.set_parent(*target, pai);
                    for &s in sections.iter() {
                        self.set_parent(s, pai);
                    }
                }
                // The `switch` value is a child of the expression; the guard and the body
                // of each case are not.
                ExprKind::Switch { value, .. } => self.set_parent(*value, pai),
                // Interpolations (`InterpolationExpression`) and the body of a function
                // expression: a boundary.
                _ => {}
            }
        }
        // Constant patterns (anywhere: `case`, `if-case`, declaration, assignment,
        // subpattern).
        for k in 0..a.patterns.len() {
            self.constant_pattern(PatternId(k as u32), false);
        }
        // The old `switch` `case e:` (before 3.0): `SwitchCase`.
        if before_3 {
            for s in a.stmts.iter() {
                if let StmtKind::Switch { cases, .. } = &s.kind {
                    for c in cases.iter() {
                        if let Some(p) = c.pattern {
                            self.constant_pattern(p, true);
                        }
                    }
                }
            }
        }
        // The declarations.
        for d in a.decls.iter() {
            self.annotations(&d.metadata);
            match &d.kind {
                DeclKind::Class(x) => self.type_parameters_(&x.type_params),
                DeclKind::Mixin(x) => self.type_parameters_(&x.type_params),
                DeclKind::Enum(x) => {
                    self.type_parameters_(&x.type_params);
                    for c in x.constants.iter() {
                        self.annotations(&c.metadata);
                        if let Some(args) = &c.arguments {
                            self.arguments_(args, Pai::ConstanteDeEnum);
                        }
                    }
                }
                DeclKind::Extension(x) => self.type_parameters_(&x.type_params),
                DeclKind::ExtensionType(x) => {
                    self.type_parameters_(&x.type_params);
                    self.annotations(&x.representation_metadata);
                }
                DeclKind::Typedef(t) => {
                    self.type_parameters_(&t.type_params);
                    if let TypedefKind::Legacy { parameters, .. } = &t.kind {
                        self.parameters_(parameters);
                    }
                }
                DeclKind::Function(_) => {}
                DeclKind::Variables(l) => self.list(l),
            }
        }
        for m in a.members.iter() {
            self.annotations(&m.metadata);
            match &m.kind {
                MemberKind::Field(l) => self.list(l),
                MemberKind::Constructor(k) => self.parameters_(&k.parameters),
                MemberKind::Method(_) => {}
            }
        }
        for f in a.functions.iter() {
            self.type_parameters_(&f.type_params);
            if let Some(ps) = &f.parameters {
                self.parameters_(ps);
            }
        }
        for t in a.types.iter() {
            if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
                self.type_parameters_(type_params);
                self.parameters_(parameters);
            }
        }
        for (_, ms) in a.metadados_locais.iter() {
            self.annotations(ms);
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) => self.list(l),
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => self.list(l),
                StmtKind::ForIn { target: ForInTarget::Declared { metadata, .. }, .. } => self.annotations(metadata),
                _ => {}
            }
        }
    }
}
