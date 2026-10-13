//! Inlay hints (`textDocument/inlayHint`) in the format of the Dart 3.6.2
//! `DartInlayHintComputer`
//! (`pkg/analysis_server/lib/src/computer/computer_inlay_hint.dart`):
//!
//! * type (kind 1) before the name of a variable declared without a type
//!   (`var`, `final`, `const`, fields and top-level variables), of a
//!   `for-in` variable without a type, of a pattern variable without a type
//!   and of a parameter without a type (including a *closure*'s; `this.x`,
//!   `super.x` and function-typed parameters are not);
//! * return type before the name of a method declared without a return type
//!   (including the setter, with `void`), and before the name (or the `get`)
//!   of a function without one (not a top-level setter);
//! * parameter name (kind 2, `name:`) before each positional argument of
//!   every argument list that resolves to a function with declared
//!   parameters: calls, creations, annotations and constructors' `super(…)`
//!   and `this(…)`;
//! * inferred type arguments (`<int>`) before the `[`/`{` of a collection
//!   literal without written arguments, and after the class name in an
//!   instance creation without written arguments;
//! * the chosen type arguments of a generic invocation without written
//!   arguments (`visitInvocationExpression`, the `typeArgumentTypes`), before
//!   the `(`;
//! * the arguments of a named type written without them (`visitNamedType`:
//!   a raw `List` gives `<dynamic>`), after the type.
//!
//! The parameter names of an SDK function are those of the public
//! declaration, not the patch's (the analyzer does not apply patches).
//!
//! The analyzer returns the hints of the whole file, ignoring the requested
//! range; so does this. Types come from the common inference (`local_type`,
//! `get_type`, outline), never from an inference of its own.

use crate::project::Project;
use dartforge_elements::model::{FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberKind, ParameterKind, StmtKind};
use dartforge_types::{MemberRef, Resolved, Type, TypeId};
use std::collections::HashMap;

/// A hint: offset (bytes) where it appears, text, LSP kind (1 type, 2
/// parameter) and whether it takes a space on the right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub offset: usize,
    pub label: String,
    pub kind: u8,
    pub space_after: bool,
}

impl Project {
    /// The hints of `unit`.
    pub(crate) fn hints(&self, unit_: UnitId) -> Vec<Hint> {
        let p = self.program();
        let u = p.unit(unit_);
        let ast = &u.ast;
        let bodies = &self.query.bodies.units[unit_.0 as usize];
        let mut output = Vec::new();
        let ty_ = |offset: usize, t: TypeId, output: &mut Vec<Hint>| {
            output.push(Hint { offset, label: self.query.format(t), kind: 1, space_after: true });
        };
        // Top-level variables and fields: through the element.
        let mut variables: HashMap<(bool, u32, usize), dartforge_elements::model::VariableId> = HashMap::new();
        for (i, v) in p.variables.iter().enumerate() {
            let id = dartforge_elements::model::VariableId(i as u32);
            match v.node {
                VariableRef::TopLevel { unit, decl, index } if unit == unit_ => {
                    variables.insert((true, decl.0, index), id);
                }
                VariableRef::Field { unit, member, index } if unit == unit_ => {
                    variables.insert((false, member.0, index), id);
                }
                _ => {}
            }
        }
        // Tree function -> element (returns without a written type).
        let functions: HashMap<u32, FunctionElementId> = p
            .functions
            .iter()
            .enumerate()
            .filter_map(|(i, x)| match x.node {
                FunctionRef::Function { unit, function } if unit == unit_ => Some((function.0, FunctionElementId(i as u32))),
                _ => None,
            })
            .collect();
        for (i, d) in ast.decls.iter().enumerate() {
            match &d.kind {
                DeclKind::Variables(vl) if vl.ty.is_none() => {
                    for (k, var) in vl.variables.iter().enumerate() {
                        if let Some(t) = variables.get(&(true, i as u32, k)).and_then(|v| self.query.variable_type(*v)) {
                            ty_(var.name.span.start, t, &mut output);
                        }
                    }
                }
                DeclKind::Function(f) => self.return_(unit_, *f, functions.get(&f.0).copied(), false, &mut output),
                _ => {}
            }
        }
        for (i, m) in ast.members.iter().enumerate() {
            match &m.kind {
                MemberKind::Field(vl) if vl.ty.is_none() => {
                    for (k, var) in vl.variables.iter().enumerate() {
                        if let Some(t) = variables.get(&(false, i as u32, k)).and_then(|v| self.query.variable_type(*v)) {
                            ty_(var.name.span.start, t, &mut output);
                        }
                    }
                }
                MemberKind::Method(f) => self.return_(unit_, *f, functions.get(&f.0).copied(), true, &mut output),
                MemberKind::Constructor(k) => {
                    let element = self.node_constructor(unit_, ast::MemberId(i as u32));
                    // Parameters without a type (`SimpleFormalParameter` without a written
                    // type); a redirecting constructor has no body, the type comes from the
                    // outline.
                    for (j, prm) in k.parameters.iter().enumerate() {
                        if prm.ty.is_none()
                            && !prm.this_
                            && !prm.super_
                            && prm.function_parameters.is_none()
                            && let Some(n) = prm.name
                            && let Some(t) = bodies.tipo_local(n.span.start).or_else(|| {
                                element.and_then(|f| self.query.outline.functions.get(f.0 as usize)).and_then(|d| d.parameters.get(j)).map(|d| d.ty)
                            })
                        {
                            ty_(n.span.start, t, &mut output);
                        }
                    }
                    // `super(…)` and `this(…)`: the parameter names of the called constructor.
                    for start in k.initializers.iter() {
                        let (callee, arguments_) = match start {
                            ast::Initializer::Super { arguments, .. } => (element.and_then(|f| self.super_constructor(f)), arguments),
                            ast::Initializer::Redirect { constructor, arguments, .. } => {
                                let callee = element.and_then(|f| p.function(f).class).and_then(|c| {
                                    let name_ = match constructor {
                                        Some(n) => Some(n.sym),
                                        None => self.query.names.lookup(""),
                                    }?;
                                    p.class(c).constructors.get(&name_).copied()
                                });
                                (callee, arguments)
                            }
                            _ => continue,
                        };
                        if let Some(f) = callee {
                            let names = self.positional_names(f);
                            self.argument_names(ast, &u.source, arguments_, &names, &mut output);
                        }
                    }
                }
                _ => {}
            }
        }
        // The arguments of annotations (`visitArgumentList`).
        let mut annotations: Vec<&ast::Annotation> = Vec::new();
        for d in ast.decls.iter() {
            annotations.extend(d.metadata.iter());
            if let DeclKind::Enum(e) = &d.kind {
                for k in e.constants.iter() {
                    annotations.extend(k.metadata.iter());
                }
            }
        }
        for m in ast.members.iter() {
            annotations.extend(m.metadata.iter());
        }
        for d in u.unit.directives.iter() {
            annotations.extend(d.metadata.iter());
        }
        for f in ast.functions.iter() {
            for prm in f.parameters.iter().flatten() {
                annotations.extend(prm.metadata.iter());
            }
        }
        for m in ast.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                for prm in k.parameters.iter() {
                    annotations.extend(prm.metadata.iter());
                }
            }
        }
        for a in annotations {
            if let Some(arguments_) = &a.arguments
                && let Some(f) = self.annotation_constructor(unit_, a)
            {
                let names = self.positional_names(f);
                self.argument_names(ast, &u.source, arguments_, &names, &mut output);
            }
        }
        for s in &ast.stmts {
            match &s.kind {
                StmtKind::Variables(vl) if vl.ty.is_none() => {
                    for var in vl.variables.iter() {
                        if let Some(t) = bodies.tipo_local(var.name.span.start) {
                            ty_(var.name.span.start, t, &mut output);
                        }
                    }
                }
                // `for (var i = 0; …)`: the header's list is also a
                // `VariableDeclarationList` (`visitVariableDeclaration`).
                StmtKind::For { init: Some(ast::ForInit::Variables(vl)), .. } if vl.ty.is_none() => {
                    for var in vl.variables.iter() {
                        if let Some(t) = bodies.tipo_local(var.name.span.start) {
                            ty_(var.name.span.start, t, &mut output);
                        }
                    }
                }
                StmtKind::ForIn { target: ast::ForInTarget::Declared { ty: None, name, .. }, .. } => {
                    if let Some(t) = bodies.tipo_local(name.span.start) {
                        ty_(name.span.start, t, &mut output);
                    }
                }
                StmtKind::Function(f) => {
                    let func = ast.function(*f);
                    if func.return_type.is_none()
                        && func.kind != ast::FunctionKind::Setter
                        && let Some(n) = func.name
                        && let Some(t) = bodies.tipo_local(n.span.start)
                        && let Type::Function { ret, .. } = self.query.table.get(t)
                    {
                        ty_(n.span.start, *ret, &mut output);
                    }
                }
                _ => {}
            }
        }
        for pt in &ast.patterns {
            if let ast::PatternKind::Variable { ty: None, name, .. } = &pt.kind
                && let Some(t) = bodies.tipo_local(name.span.start)
            {
                ty_(name.span.start, t, &mut output);
            }
        }
        // Parameters without a type (of functions, methods and closures).
        for f in &ast.functions {
            for prm in f.parameters.iter().flatten() {
                if prm.ty.is_none()
                    && !prm.this_
                    && !prm.super_
                    && prm.function_parameters.is_none()
                    && let Some(n) = prm.name
                    && let Some(t) = bodies.tipo_local(n.span.start)
                {
                    ty_(n.span.start, t, &mut output);
                }
            }
        }
        // The named types of creations (`new C()`) get their hint through the creation.
        let mut of_creation: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for e in ast.exprs.iter() {
            match &e.kind {
                ExprKind::InstanceCreation { ty, .. } => {
                    of_creation.insert(ty.0);
                }
                // `[for (var i = 0; …) i]`: the header of a collection `for`.
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                    fn scan(els: &[ast::CollectionElement], bodies: &dartforge_types::UnitBodyTypes, f: &mut dyn FnMut(usize, TypeId)) {
                        for el in els {
                            match el {
                                ast::CollectionElement::For { init, body, .. } => {
                                    if let Some(ast::ForInit::Variables(vl)) = init
                                        && vl.ty.is_none()
                                    {
                                        for var in vl.variables.iter() {
                                            if let Some(t) = bodies.tipo_local(var.name.span.start) {
                                                f(var.name.span.start, t);
                                            }
                                        }
                                    }
                                    scan(std::slice::from_ref(body), bodies, f);
                                }
                                ast::CollectionElement::ForIn { target, body, .. } => {
                                    // `[for (var x in l) …]`: the `DeclaredIdentifier` without a type.
                                    if let ast::ForInTarget::Declared { ty: None, name, .. } = target
                                        && let Some(t) = bodies.tipo_local(name.span.start)
                                    {
                                        f(name.span.start, t);
                                    }
                                    scan(std::slice::from_ref(body), bodies, f)
                                }
                                ast::CollectionElement::If { then, else_, .. } => {
                                    scan(std::slice::from_ref(then), bodies, f);
                                    if let Some(x) = else_ {
                                        scan(std::slice::from_ref(x), bodies, f);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    let mut found: Vec<(usize, TypeId)> = Vec::new();
                    scan(elements, bodies, &mut |o, t| found.push((o, t)));
                    for (o, t) in found {
                        ty_(o, t, &mut output);
                    }
                }
                _ => {}
            }
        }
        // The type of an object pattern is the required one, with the inferred
        // arguments (`MapEntry(key: k)` gives `MapEntry<K, V>`).
        let mut of_pattern: HashMap<u32, TypeId> = HashMap::new();
        for (i, pt) in ast.patterns.iter().enumerate() {
            if let ast::PatternKind::Object { ty, .. } = &pt.kind
                && let Some(&t) = bodies.tipos_de_padroes.get(&ast::PatternId(i as u32))
            {
                of_pattern.insert(ty.0, t);
            }
        }
        // A nested type argument (`Command` in `List<Command>`) has no record of
        // its own: the type comes from the parent's argument.
        let mut parent_of: HashMap<u32, (u32, usize)> = HashMap::new();
        for (i, ty) in ast.types.iter().enumerate() {
            if let ast::TypeKind::Named { args, .. } = &ty.kind {
                for (k, a) in args.iter().enumerate() {
                    parent_of.insert(a.0, (i as u32, k));
                }
            }
        }
        let direct = |i: u32| -> Option<TypeId> {
            let id = ast::TypeId(i);
            of_pattern.get(&i).or_else(|| bodies.tipos_de_anotacoes.get(&id)).or_else(|| self.query.outline.tipos_escritos.get(&(unit_, id))).copied()
        };
        let resolve = |i: u32| -> Option<TypeId> {
            let mut path: Vec<usize> = Vec::new();
            let mut current = i;
            let mut base = direct(current);
            while base.is_none() {
                let &(parent, k) = parent_of.get(&current)?;
                path.push(k);
                current = parent;
                base = direct(current);
            }
            let mut t = base?;
            for &k in path.iter().rev() {
                t = match self.query.table.get(t) {
                    Type::Interface { args, .. } | Type::ExtensionType { args, .. } => *args.get(k)?,
                    _ => return None,
                };
            }
            Some(t)
        };
        // `visitNamedType`: a type written without arguments whose type has them.
        for (i, ty) in ast.types.iter().enumerate() {
            let ast::TypeKind::Named { args, .. } = &ty.kind else { continue };
            if !args.is_empty() || of_creation.contains(&(i as u32)) {
                continue;
            }
            let resolved = resolve(i as u32);
            self.type_arguments(resolved, ty.span.end, &mut output);
        }
        for (i, e) in ast.exprs.iter().enumerate() {
            let id = ast::ExprId(i as u32);
            match &e.kind {
                ExprKind::Call { target, arguments } => {
                    // `visitInvocationExpression`: the chosen type arguments, before the `(`,
                    // without written arguments (the implicit creation is not an invocation).
                    if arguments.type_args.is_empty()
                        && !matches!(bodies.get_resolved(id), Some(Resolved::Constructor(_)))
                        && let Some(chosen) = bodies.instanciacao(arguments.span.start)
                        && !chosen.is_empty()
                    {
                        let opens = u.source[arguments.span.start..].find('(').map_or(arguments.span.start, |k| arguments.span.start + k);
                        let texts: Vec<String> = chosen.iter().map(|a| self.type_text(*a)).collect();
                        output.push(Hint { offset: opens, label: format!("<{}>", texts.join(", ")), kind: 1, space_after: false });
                    }
                    let f = match bodies.get_resolved(id) {
                        Some(Resolved::Constructor(f)) => Some(*f),
                        _ => match bodies.get_resolved(*target) {
                            Some(Resolved::Member { member: MemberRef::Function(f), .. })
                            | Some(Resolved::ExtensionMember { member: f, .. })
                            | Some(Resolved::Element(dartforge_elements::model::Element::Function(f)))
                            | Some(Resolved::Constructor(f)) => Some(*f),
                            _ => None,
                        },
                    };
                    let names = match f {
                        Some(f) => self.positional_names(f),
                        None => bodies.declaracao_local(*target).map_or_else(Vec::new, |d| local_function_names(ast, &u.source, d)),
                    };
                    self.argument_names(ast, &u.source, arguments, &names, &mut output);
                    // `Caixa('a')` without `new`: the inferred type arguments after the class name.
                    if let Some(Resolved::Constructor(_)) = bodies.get_resolved(id)
                        && arguments.type_args.is_empty()
                    {
                        let class = match &ast.expr(*target).kind {
                            ExprKind::Identifier(n) => Some(n.span.end),
                            ExprKind::Property { target: t, .. } => match &ast.expr(*t).kind {
                                ExprKind::Identifier(n) => Some(n.span.end),
                                ExprKind::Property { name, .. } => Some(name.span.end),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(end) = class {
                            self.type_arguments(bodies.get_type(id), end, &mut output);
                        }
                    }
                }
                ExprKind::InstanceCreation { ty, arguments, .. } => {
                    if let Some(Resolved::Constructor(f)) = bodies.get_resolved(id) {
                        let names = self.positional_names(*f);
                        self.argument_names(ast, &u.source, arguments, &names, &mut output);
                    }
                    if let ast::TypeKind::Named { args, name } = &ast.ty(*ty).kind
                        && args.is_empty()
                        && let Some(n) = name.last()
                    {
                        self.type_arguments(bodies.get_type(id), n.span.end, &mut output);
                    }
                }
                ExprKind::List { type_args, .. } | ExprKind::SetOrMap { type_args, .. } if type_args.is_empty() => {
                    let opens = if matches!(e.kind, ExprKind::List { .. }) { '[' } else { '{' };
                    if let Some(k) = u.source[e.span.start..e.span.end].find(opens) {
                        self.type_arguments(bodies.get_type(id), e.span.start + k, &mut output);
                    }
                }
                _ => {}
            }
        }
        output.sort_by_key(|d| d.offset);
        output
    }

    /// Return type of a function or method without a written type: the method
    /// (`visitMethodDeclaration`) always before the name, including the setter;
    /// the function (`visitFunctionDeclaration`) before the `get`, and not the
    /// setter.
    fn return_(&self, unit: UnitId, f: ast::FunctionId, element: Option<FunctionElementId>, method: bool, output: &mut Vec<Hint>) {
        let p = self.program();
        let Some(fe) = element else { return };
        let func = p.unit(unit).ast.function(f);
        if func.return_type.is_some() || (!method && (func.kind == ast::FunctionKind::Setter || p.function(fe).kind == FunctionKind::Setter)) {
            return;
        }
        let Some(n) = func.name else { return };
        // Top-level getter: before the `get`.
        let source = &p.unit(unit).source;
        let offset = if !method && func.kind == ast::FunctionKind::Getter {
            source[..n.span.start].trim_end().strip_suffix("get").map_or(n.span.start, str::len)
        } else {
            n.span.start
        };
        let t = self.query.outline.functions[fe.0 as usize].return_type;
        output.push(Hint { offset, label: self.query.format(t), kind: 1, space_after: true });
    }

    /// The constructor that the annotation `@C(…)`, `@C.n(…)`, `@p.C(…)` or
    /// `@p.C.n(…)` invokes.
    fn annotation_constructor(&self, unit: UnitId, a: &ast::Annotation) -> Option<FunctionElementId> {
        let p = self.program();
        let lib = p.unit(unit).library;
        let class_of = |b: Option<dartforge_elements::model::Binding>| match b.and_then(|b| b.getter) {
            Some(dartforge_elements::model::Element::Class(c)) => Some(c),
            _ => None,
        };
        let (class, name) = match a.name.as_slice() {
            [c] => (class_of(p.lookup_na_unidade(unit, c.sym))?, None),
            [x, y] if crate::project::is_prefix(p, lib, x.sym) => (class_of(p.lookup_prefixed_na_unidade(unit, x.sym, y.sym))?, None),
            [x, y] => (class_of(p.lookup_na_unidade(unit, x.sym))?, Some(y.sym)),
            [x, y, z] => (class_of(p.lookup_prefixed_na_unidade(unit, x.sym, y.sym))?, Some(z.sym)),
            _ => return None,
        };
        let name = match name {
            Some(n) => n,
            None => self.query.names.lookup("")?,
        };
        p.class(class).constructors.get(&name).copied()
    }

    /// Names of the positional parameters of `f`, in order (those of the public
    /// declaration of an SDK patch member).
    fn positional_names(&self, f: FunctionElementId) -> Vec<String> {
        let p = self.program();
        let f = p.function(f).declaracao_publica.unwrap_or(f);
        let (unit, ps): (UnitId, &[ast::Parameter]) = match p.function(f).node {
            FunctionRef::Function { unit, function } => (unit, p.unit(unit).ast.function(function).parameters.as_deref().unwrap_or(&[])),
            FunctionRef::Constructor { unit, member } => match &p.unit(unit).ast.member(member).kind {
                MemberKind::Constructor(k) => (unit, &k.parameters),
                _ => return Vec::new(),
            },
            FunctionRef::None => return Vec::new(),
        };
        let source = &p.unit(unit).source;
        ps.iter()
            .filter(|x| x.kind != ParameterKind::Named)
            .map(|x| x.name.map_or(String::new(), |n| source[n.span.start..n.span.end].to_string()))
            .collect()
    }

    /// `name:` before each positional argument.
    fn argument_names(&self, ast: &ast::Ast, _source: &str, arguments: &ast::Arguments, names: &[String], output: &mut Vec<Hint>) {
        let mut k = 0;
        for a in arguments.args.iter() {
            if a.name.is_some() {
                continue;
            }
            if let Some(n) = names.get(k).filter(|n| !n.is_empty()) {
                output.push(Hint { offset: ast.expr(a.value).span.start, label: format!("{n}:"), kind: 2, space_after: true });
            }
            k += 1;
        }
    }

    /// `<A, B>` of the arguments of an interface type, at `offset`.
    fn type_arguments(&self, ty: Option<TypeId>, offset: usize, output: &mut Vec<Hint>) {
        let Some(t) = ty else { return };
        let args = match self.query.table.get(t) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.clone(),
            _ => return,
        };
        if args.is_empty() {
            return;
        }
        let texts: Vec<String> = args.iter().map(|a| self.type_text(*a)).collect();
        output.push(Hint { offset, label: format!("<{}>", texts.join(", ")), kind: 1, space_after: false });
    }

    /// `_appendTypePart`: the element's name (not the alias's), the arguments
    /// and the `?`; the rest through `getDisplayString`.
    fn type_text(&self, t: TypeId) -> String {
        self.query.table.format_sem_alias(t, &self.query.names, &self.query.program)
    }
}

/// Names of the positional parameters of the local function declared at `decl`.
fn local_function_names(ast: &ast::Ast, source: &str, decl: usize) -> Vec<String> {
    let Some(f) = ast.functions.iter().find(|f| f.name.is_some_and(|n| n.span.start == decl)) else {
        return Vec::new();
    };
    f.parameters
        .iter()
        .flatten()
        .filter(|x| x.kind != ParameterKind::Named)
        .map(|x| x.name.map_or(String::new(), |n| source[n.span.start..n.span.end].to_string()))
        .collect()
}
