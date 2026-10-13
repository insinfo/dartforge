//! Descrição de um elemento para o hover, no formato do servidor do Dart
//! (`ElementDisplayStringBuilder` do analyzer): `int soma(int a, [int b =
//! 0])`, `String get nome`, `abstract class B<T> extends A implements C`,
//! mais o tipo estático (variáveis e getters) e a documentação.
//!
//! Tipos vêm da tabela da inferência comum (`TypeTable::format`); nomes e
//! valores padrão, da árvore da declaração. Nada é inventado: um elemento sem
//! forma conhecida não tem hover.

use crate::Hover;
use crate::dartdoc;
use crate::project::{Target, Concrete, Denoted, Project, base_name, word};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, UnitId,
};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind};
use dartforge_types::{Type, TypeId};

impl Project {
    /// O hover do que `d` denota na unidade `unit`.
    pub(crate) fn hover(&self, unit: UnitId, d: &Denoted) -> Option<Hover> {
        // `_targetNode`: o nome do tipo de `A.n(…)` (o `NamedType` de um
        // `ConstructorName` de criação) mostra a criação, o construtor.
        let adjusted;
        let d = match (&d.target, d.concrete, d.expr) {
            (Target::Top(Element::Class(_)), None, Some(e)) => match self.constructor_by_type(unit, e) {
                Some(f) => {
                    adjusted = Denoted { concrete: Some(Concrete::Function(f)), ..d.clone() };
                    &adjusted
                }
                None => d,
            },
            _ => d,
        };
        // Declaração de constante de enum: o analyzer não tem hover ali.
        if d.expr.is_none()
            && let Some(Concrete::Variable(v)) = d.concrete
            && self.program().variable(v).class.is_some_and(|c| self.program().class(c).enum_constants.contains(&v))
            && self.variable_name(v).is_some_and(|(u, s)| u == unit && s == d.name)
        {
            return None;
        }
        let (description, ty, doc_start) = match (&d.target, d.concrete) {
            (
                Target::Local {
                    unit: u,
                    declaration,
                },
                _,
            ) => {
                let (description, ty) = self.describe_local(*u, *declaration, d.expr, unit)?;
                (description, ty, None)
            }
            // A declaração do parâmetro de tipo (`TypeParameter`) não é nó
            // com hover no analyzer.
            (Target::TypeParameter { unit: u, declaration }, _) if *u == unit && d.name.start == *declaration => return None,
            (
                Target::TypeParameter {
                    unit: u,
                    declaration,
                },
                _,
            ) => (
                self.describe_type_parameter(*u, *declaration)?,
                None,
                None,
            ),
            (Target::Prefix { .. }, _) => return None,
            (Target::Top(Element::Class(_)), Some(Concrete::Function(f)))
                if matches!(
                    self.program().function(f).kind,
                    FunctionKind::Constructor | FunctionKind::SyntheticConstructor
                ) =>
            {
                (
                    self.describe_function_with(f, self.member_replacement(unit, d, f)),
                    None,
                    self.query.function_start(f),
                )
            }
            (_, Some(Concrete::Function(f))) => {
                let fe = self.program().function(f);
                // Na declaração (sem expressão), o analyzer não mostra tipo.
                let ty = (matches!(fe.kind, FunctionKind::Getter) && d.expr.is_some())
                    .then(|| {
                        self.reference_type(unit, d.expr)
                            .unwrap_or(self.query.outline.functions[f.0 as usize].return_type)
                    })
                    .map(|t| self.query.format(t));
                (
                    self.describe_function_with(f, self.member_replacement(unit, d, f)),
                    ty,
                    self.query.function_start(f),
                )
            }
            (_, Some(Concrete::Variable(v))) => {
                let declared = self.query.variable_type(v);
                let ty = self.reference_type(unit, d.expr).or(declared)?;
                let text_type = self.query.format(ty);
                let name = self.name(self.program().variable(v).name);
                let declared =
                    declared.map_or_else(|| text_type.clone(), |t| self.query.format(t));
                (
                    format!("{declared} {name}"),
                    Some(text_type),
                    self.query.variable_start(v),
                )
            }
            (Target::Top(el), None) => (
                self.describe_type(*el)?,
                None,
                self.query.element_start(*el),
            ),
            (Target::Constructor(f), None) => (
                self.describe_function(*f),
                None,
                self.query.function_start(*f),
            ),
            (Target::Member { .. }, None) => return None,
        };
        // `this.x` num construtor: o elemento do hover é o parâmetro.
        let formal_field = d.expr.is_none()
            && crate::project::parameter_in(&self.program().unit(unit).ast, d.name.start).is_some_and(|(x, _)| x.this_);
        let documentation = self.computed_documentation(unit, d, doc_start, formal_field);
        // Sem expressão e fora da declaração (metadados, `show`/`hide`,
        // `[ref]`): o analyzer não tem tipo estático para mostrar.
        let ty = if d.expr.is_none() && !matches!(d.target, Target::Local { .. }) && !self.declaration(d).is_some_and(|(u, s)| u == unit && s == d.name) {
            None
        } else {
            ty
        };
        // `this.x`: o `type` do `FieldFormalParameterElement` (um
        // `VariableElement`), como na declaração de um parâmetro.
        let ty = if formal_field { ty.or_else(|| self.field_formal_type(unit, d.name.start)) } else { ty };
        // Chamada: o tipo da invocação (`staticInvokeType`), como o Dart.
        let ty = ty.or_else(|| match d.concrete {
            Some(Concrete::Function(f))
                if matches!(self.program().function(f).kind, FunctionKind::Function | FunctionKind::Operator)
                    && d.expr.is_some_and(|e| self.is_call_target(unit, e)) =>
            {
                self.reference_type(unit, d.expr)
                    .filter(|t| !matches!(self.query.table.get(*t), Type::Dynamic))
                    .map(|t| self.query.format(t))
            }
            // Função local chamada.
            None if matches!(d.target, Target::Local { .. })
                && d.expr.is_some_and(|e| self.is_call_target(unit, e))
                && d.expr.and_then(|e| self.reference_type(unit, Some(e))).is_some_and(|t| matches!(self.query.table.get(t), Type::Function { .. })) =>
            {
                self.reference_type(unit, d.expr).map(|t| self.query.format(t))
            }
            _ => None,
        });
        // Criação sem `new`/`const`: `(new) ` antes da descrição.
        let mut description = description;
        if let Some(Concrete::Function(f)) = d.concrete
            && matches!(self.program().function(f).kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor)
            && d.expr.is_some_and(|e| self.is_call_target(unit, e) || self.is_call_target_by_property(unit, e))
        {
            description = format!("(new) {description}");
        }
        if self.deprecated(d) {
            description = format!("(deprecated) {description}");
        }
        Some(Hover {
            range: d.name,
            description,
            ty,
            documentation,
            library: if formal_field { None } else { self.displayed_library(d) },
        })
    }

    /// A substituição do `Member` que o hover mostra: um método ou operador
    /// de instância lido por `target.m`, pelos argumentos do tipo do alvo; um
    /// construtor, pelos do tipo criado. Getters, setters e campos passam
    /// pelo `nonSynthetic` e ficam como declarados.
    fn member_replacement(&self, unit: UnitId, d: &Denoted, f: FunctionElementId) -> Option<Replacement> {
        let p = self.program();
        let fe = p.function(f);
        let class = fe.class?;
        let e = d.expr?;
        let ast = &p.unit(unit).ast;
        let bodies = self.query.bodies.units.get(unit.0 as usize)?;
        let ty = match fe.kind {
            FunctionKind::Function | FunctionKind::Operator if !fe.static_ => match &ast.expr(e).kind {
                ast::ExprKind::Property { target, .. } => bodies.get_type(*target)?,
                _ => return None,
            },
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                // A criação: `C(…)`, `C.n(…)`, `p.C(…)` (a chamada cujo alvo
                // é o nome, ou a propriedade sobre ele).
                let call = ast.exprs.iter().enumerate().find_map(|(i, x)| match &x.kind {
                    ast::ExprKind::Call { target, .. } if *target == e => Some(i),
                    ast::ExprKind::Call { target, .. } => match &ast.expr(*target).kind {
                        ast::ExprKind::Property { target: t2, .. } if *t2 == e => Some(i),
                        ast::ExprKind::TypeArguments { target: t2, .. } if *t2 == e => Some(i),
                        _ => None,
                    },
                    _ => None,
                })?;
                bodies.get_type(ast::ExprId(call as u32))?
            }
            _ => return None,
        };
        let mut table = self.query.table.clone();
        let seen = self.query.outline.hierarchy.supertype_of(ty, class, &mut table, &self.query.core)?;
        let Type::Interface { args, .. } = table.get(seen).clone() else { return None };
        let params = &self.query.outline.classes.get(class.0 as usize)?.type_params;
        if params.len() != args.len() || params.is_empty() {
            return None;
        }
        Some((params.iter().copied().zip(args.iter().copied()).collect(), table))
    }

    /// O construtor de `A.n(…)`, `A<T>.n(…)` ou `p.A.n(…)` quando `e` é o
    /// nome do tipo (`A`).
    fn constructor_by_type(&self, unit: UnitId, e: ast::ExprId) -> Option<FunctionElementId> {
        let ast = &self.program().unit(unit).ast;
        let bodies = self.query.bodies.units.get(unit.0 as usize)?;
        let parent = |x: ast::ExprId| {
            ast.exprs.iter().position(|y| match &y.kind {
                ast::ExprKind::Property { target, .. } | ast::ExprKind::TypeArguments { target, .. } => *target == x,
                _ => false,
            })
        };
        let mut current = ast::ExprId(parent(e)? as u32);
        if matches!(ast.expr(current).kind, ast::ExprKind::TypeArguments { .. }) {
            current = ast::ExprId(parent(current)? as u32);
        }
        if !matches!(ast.expr(current).kind, ast::ExprKind::Property { .. }) {
            return None;
        }
        ast.exprs.iter().enumerate().find_map(|(i, y)| match &y.kind {
            ast::ExprKind::Call { target, .. } if *target == current => match bodies.get_resolved(ast::ExprId(i as u32)) {
                Some(dartforge_types::Resolved::Constructor(f)) => Some(*f),
                _ => None,
            },
            _ => None,
        })
    }

    /// O tipo de `this.x` (o do parâmetro no outline do construtor).
    fn field_formal_type(&self, unit: UnitId, name: usize) -> Option<String> {
        let p = self.program();
        let ast = &p.unit(unit).ast;
        let (_, crate::project::OwnerParameter::Constructor(m)) = crate::project::parameter_in(ast, name)? else { return None };
        let MemberKind::Constructor(k) = &ast.member(m).kind else { return None };
        let i = k.parameters.iter().position(|x| x.name.is_some_and(|n| n.span.start == name))?;
        let f = self.node_constructor(unit, m)?;
        let t = self.query.outline.functions.get(f.0 as usize)?.parameters.get(i)?.ty;
        Some(self.query.format(t))
    }

    /// `DartUnitHoverComputer.computeDocumentation`: o elemento (o campo de
    /// `this.x`; o executável dono de um parâmetro), ele e os membros que ele
    /// sobrescreve (`findOverriddenElements`: as superclasses e os mixins,
    /// depois as interfaces), o primeiro com comentário de documentação; um
    /// setter sem ela usa a do getter correspondente. Vinda de outra classe,
    /// a documentação ganha o `Copied from` dela. Acessor sintético de campo
    /// não tem comentário.
    fn computed_documentation(&self, unit: UnitId, d: &Denoted, doc_start: Option<(UnitId, usize)>, formal_field: bool) -> Option<String> {
        let p = self.program();
        let seed: Option<Documentable> = if formal_field {
            let (prm, owner) = crate::project::parameter_in(&p.unit(unit).ast, d.name.start)?;
            let crate::project::OwnerParameter::Constructor(m) = owner else { return None };
            let class = p.function(self.node_constructor(unit, m)?).class?;
            let name = prm.name?.sym;
            Some(Documentable::Variable(p.class(class).fields.iter().copied().find(|&v| p.variable(v).name == name)?))
        } else {
            match (&d.target, d.concrete) {
                (Target::Local { unit: u, declaration }, _) => {
                    let ast = &p.unit(*u).ast;
                    match crate::project::parameter_in(ast, *declaration)?.1 {
                        crate::project::OwnerParameter::Function(fid) => match self.node_function(*u, fid) {
                            Some(f) => Some(Documentable::Function(f)),
                            // Função local: o comentário dela; closure não tem.
                            None => {
                                ast.function(fid).name?;
                                return dartdoc::documentation(&p.unit(*u).source, ast.function(fid).span.start);
                            }
                        },
                        crate::project::OwnerParameter::Constructor(m) => Some(Documentable::Function(self.node_constructor(*u, m)?)),
                    }
                }
                (_, Some(Concrete::Function(f))) => match (p.function(f).kind, p.function(f).variable) {
                    (FunctionKind::ImplicitAccessor, Some(v)) => Some(Documentable::Variable(v)),
                    _ => Some(Documentable::Function(f)),
                },
                (_, Some(Concrete::Variable(v))) => Some(Documentable::Variable(v)),
                _ => None,
            }
        };
        let Some(seed) = seed else {
            return doc_start.and_then(|(u, start)| dartdoc::documentation(&p.unit(u).source, start));
        };
        let owner = |x: Documentable| match x {
            Documentable::Function(f) => p.function(f).class,
            Documentable::Variable(v) => p.variable(v).class,
        };
        let comment = |x: Documentable| -> Option<String> {
            let (u, start) = match x {
                Documentable::Function(f) if p.function(f).kind == FunctionKind::ImplicitAccessor => return None,
                Documentable::Function(f) => self.query.function_start(f)?,
                Documentable::Variable(v) => self.query.variable_start(v)?,
            };
            dartdoc::documentation(&p.unit(u).source, start)
        };
        let mut candidates = vec![seed];
        if let Some(class) = owner(seed) {
            let library = match seed {
                Documentable::Function(f) => p.function(f).library,
                Documentable::Variable(v) => p.variable(v).library,
            };
            let (name, kinds): (String, Vec<FunctionKind>) = match seed {
                Documentable::Variable(v) => {
                    let var = p.variable(v);
                    let mut e = vec![FunctionKind::Getter];
                    if !var.final_ && !var.const_ {
                        e.push(FunctionKind::Setter);
                    }
                    (self.name(var.name).to_string(), e)
                }
                Documentable::Function(f) => {
                    let fe = p.function(f);
                    let name = self.name(fe.name);
                    match fe.kind {
                        FunctionKind::Function | FunctionKind::Operator => (name.to_string(), vec![FunctionKind::Function]),
                        FunctionKind::Getter => (name.to_string(), vec![FunctionKind::Getter]),
                        FunctionKind::Setter => (name.trim_end_matches("_=").trim_end_matches('=').to_string(), vec![FunctionKind::Setter]),
                        _ => (name.to_string(), Vec::new()),
                    }
                }
            };
            if !kinds.is_empty() {
                let mut supers = Vec::new();
                let mut seen = std::collections::HashSet::new();
                self.override_supers(class, false, &mut seen, &mut supers);
                let mut interfaces = Vec::new();
                seen.clear();
                self.override_interfaces(class, false, &mut seen, &mut interfaces);
                let mut found_super: Vec<FunctionElementId> = Vec::new();
                for s in supers {
                    if let Some(m) = self.override_member(s, &name, &kinds, library)
                        && !found_super.contains(&m)
                    {
                        found_super.push(m);
                    }
                }
                let mut found_interface: Vec<FunctionElementId> = Vec::new();
                for s in interfaces {
                    if let Some(m) = self.override_member(s, &name, &kinds, library)
                        && !found_interface.contains(&m)
                        && !found_super.contains(&m)
                    {
                        found_interface.push(m);
                    }
                }
                candidates.extend(found_super.into_iter().chain(found_interface).map(Documentable::Function));
            }
        }
        let mut documented: Option<(Documentable, String)> = None;
        let mut of_getter: Option<(Documentable, String)> = None;
        for &c in candidates.iter() {
            if let Some(text) = comment(c) {
                documented = Some((c, text));
                break;
            }
            if of_getter.is_none()
                && let Documentable::Function(f) = c
                && p.function(f).kind == FunctionKind::Setter
                && let Some(class) = p.function(f).class
                && let Some(key) = self.query.names.lookup(self.name(p.function(f).name).trim_end_matches("_=").trim_end_matches('='))
                && let Some(&g) = p.class(class).instance_members.get(&key).or_else(|| p.class(class).static_members.get(&key))
                && let Some(text) = comment(Documentable::Function(g))
            {
                of_getter = Some((Documentable::Function(g), text));
            }
        }
        let (element, mut text) = documented.or(of_getter)?;
        if let Some(c) = owner(element)
            && owner(element) != owner(seed)
        {
            text = format!("{text}\n\nCopied from `{}`.", self.name(p.class(c).name));
        }
        Some(text)
    }

    /// A superclasse declarada (pulando as aplicações de mixin sintéticas) e
    /// os mixins na ordem escrita.
    fn super_and_mixins(&self, c: ClassId) -> (Option<ClassId>, Vec<ClassId>) {
        let p = self.program();
        let mut synthetic: Vec<ClassId> = Vec::new();
        let mut s = p.class(c).supertype_class;
        let mut steps = 0;
        while let Some(x) = s
            && p.class(x).decl.is_none()
            && p.class(x).kind == ClassKind::MixinApplication
            && steps < 64
        {
            synthetic.extend(p.class(x).mixin_classes.iter().rev());
            s = p.class(x).supertype_class;
            steps += 1;
        }
        synthetic.reverse();
        synthetic.extend(p.class(c).mixin_classes.iter().copied());
        (s, synthetic)
    }

    /// `_addSuperOverrides`.
    fn override_supers(&self, c: ClassId, with_this: bool, seen: &mut std::collections::HashSet<ClassId>, output: &mut Vec<ClassId>) {
        if !seen.insert(c) {
            return;
        }
        if with_this {
            output.push(c);
        }
        let (s, mixins) = self.super_and_mixins(c);
        if let Some(s) = s {
            self.override_supers(s, true, seen, output);
        }
        for m in mixins {
            self.override_supers(m, true, seen, output);
        }
        if self.program().class(c).kind == ClassKind::Mixin {
            for o in self.program().class(c).on_classes.clone() {
                self.override_supers(o, true, seen, output);
            }
        }
    }

    /// `_addInterfaceOverrides`.
    fn override_interfaces(&self, c: ClassId, verify: bool, seen: &mut std::collections::HashSet<ClassId>, output: &mut Vec<ClassId>) {
        if !seen.insert(c) {
            return;
        }
        if verify {
            output.push(c);
        }
        for i in self.program().class(c).interface_classes.clone() {
            self.override_interfaces(i, true, seen, output);
        }
        if let (Some(s), _) = self.super_and_mixins(c) {
            self.override_interfaces(s, verify, seen, output);
        }
    }

    /// `_lookupMember`: o método, o getter ou o setter declarado em `c` com
    /// o nome (privado só na mesma biblioteca).
    fn override_member(&self, c: ClassId, name: &str, kinds: &[FunctionKind], library: dartforge_elements::model::LibraryId) -> Option<FunctionElementId> {
        let p = self.program();
        let k = p.class(c);
        if name.starts_with('_') && k.library != library {
            return None;
        }
        let find = |key: &str, accepts: &dyn Fn(FunctionKind) -> bool| -> Option<FunctionElementId> {
            let s = self.query.names.lookup(key)?;
            [k.instance_members.get(&s), k.static_members.get(&s)].into_iter().flatten().copied().find(|&f| accepts(p.function(f).kind))
        };
        if kinds.contains(&FunctionKind::Function)
            && let Some(f) = find(name, &|e| matches!(e, FunctionKind::Function | FunctionKind::Operator))
        {
            return Some(f);
        }
        if kinds.contains(&FunctionKind::Getter)
            && let Some(f) = find(name, &|e| matches!(e, FunctionKind::Getter | FunctionKind::ImplicitAccessor))
        {
            return Some(f);
        }
        if kinds.contains(&FunctionKind::Setter)
            && let Some(f) = find(&format!("{name}_="), &|e| matches!(e, FunctionKind::Setter | FunctionKind::ImplicitAccessor))
        {
            return Some(f);
        }
        None
    }

    /// `e` é o alvo de uma chamada (`e(…)`).
    fn is_call_target(&self, unit: UnitId, e: ast::ExprId) -> bool {
        self.program().unit(unit).ast.exprs.iter().any(|x| matches!(&x.kind, ast::ExprKind::Call { target, .. } if *target == e))
    }

    /// `e` é a classe de `A.name(…)` (o alvo da propriedade chamada).
    fn is_call_target_by_property(&self, unit: UnitId, e: ast::ExprId) -> bool {
        let ast = &self.program().unit(unit).ast;
        ast.exprs.iter().enumerate().any(|(i, x)| {
            matches!(&x.kind, ast::ExprKind::Property { target, .. } if *target == e)
                && self.is_call_target(unit, ast::ExprId(i as u32))
        })
    }

    /// A biblioteca do elemento não local, como o hover do Dart a mostra.
    fn displayed_library(&self, d: &Denoted) -> Option<String> {
        let p = self.program();
        let lib = match (&d.target, d.concrete) {
            // A variável de um `for` de coleção não tem executável como
            // `enclosingElement3`: o analyzer mostra a biblioteca.
            (Target::Local { unit, declaration }, _) if in_collection_for(&p.unit(*unit).ast, *declaration) => p.unit(*unit).library,
            (Target::Local { .. }, _) | (Target::Prefix { .. }, _) => return None,
            (Target::TypeParameter { unit, declaration }, _) => {
                // Parâmetro de tipo de método ou função: local.
                let ast = &p.unit(*unit).ast;
                if ast.functions.iter().any(|f| f.type_params.iter().any(|t| t.name.span.start == *declaration)) {
                    return None;
                }
                p.unit(*unit).library
            }
            (_, Some(Concrete::Function(f))) => p.function(f).library,
            (_, Some(Concrete::Variable(v))) => p.variable(v).library,
            (Target::Top(el), None) => self.element_library(*el),
            (Target::Constructor(f), None) => p.function(*f).library,
            (Target::Member { .. }, None) => return None,
        };
        let uri = &p.library(lib).uri;
        if !uri.starts_with("file:") {
            return Some(uri.clone());
        }
        let path = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        let root = crate::project::project_root(&path);
        let relative = dartforge_elements::config::sem_verbatim(path.clone());
        let relative = relative.strip_prefix(dartforge_elements::config::sem_verbatim(root.clone())).ok()?;
        let parts: Vec<String> = relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        // Em `lib/` de um pacote, o analyzer vê a URI `package:`.
        if parts.first().is_some_and(|c| c == "lib")
            && let Some(package) = crate::index::package_name(&root)
        {
            return Some(format!("package:{package}/{}", parts[1..].join("/")));
        }
        Some(parts.join("/"))
    }

    /// A declaração do elemento tem `@deprecated` ou `@Deprecated(…)`.
    fn deprecated(&self, d: &Denoted) -> bool {
        let p = self.program();
        let metadata: Option<(UnitId, &[ast::Annotation])> = match (&d.target, d.concrete) {
            (_, Some(Concrete::Function(f))) => match p.function(f).node {
                FunctionRef::Function { unit, function } => {
                    let ast = &p.unit(unit).ast;
                    ast.decls
                        .iter()
                        .find(|x| matches!(x.kind, DeclKind::Function(g) if g == function))
                        .map(|x| (unit, &x.metadata[..]))
                        .or_else(|| ast.members.iter().find(|m| matches!(m.kind, MemberKind::Method(g) if g == function)).map(|m| (unit, &m.metadata[..])))
                }
                FunctionRef::Constructor { unit, member } => Some((unit, &p.unit(unit).ast.member(member).metadata[..])),
                FunctionRef::None => None,
            },
            (Target::Top(Element::Class(c)), None) => p.class(*c).decl.map(|dr| (dr.unit, &p.unit(dr.unit).ast.decl(dr.decl).metadata[..])),
            _ => None,
        };
        metadata.is_some_and(|(u, ms)| {
            let source = &p.unit(u).source;
            ms.iter().any(|a| a.name.last().is_some_and(|n| matches!(&source[n.span.start..n.span.end], "deprecated" | "Deprecated")))
        })
    }

    /// O tipo estático da expressão (com promoções), quando o cursor está
    /// num uso.
    fn reference_type(&self, unit: UnitId, expr: Option<ast::ExprId>) -> Option<TypeId> {
        let e = expr?;
        let bodies = self.query.bodies.units.get(unit.0 as usize)?;
        // O alvo de uma atribuição (`x = …`, `x += …`, `o.x = …`): o
        // `writeType` dela (`_getTypeOfDeclarationOrReference`).
        let ast = &self.program().unit(unit).ast;
        if let Some(i) = ast.exprs.iter().position(|x| matches!(&x.kind, ast::ExprKind::Assign { target, .. } if *target == e))
            && let Some(&t) = bodies.tipos_de_escrita.get(&ast::ExprId(i as u32))
        {
            return Some(t);
        }
        bodies.get_type(e)
    }

    /// Local, parâmetro ou função local.
    fn describe_local(
        &self,
        u: UnitId,
        declaration: usize,
        expr: Option<ast::ExprId>,
        unit_expr: UnitId,
    ) -> Option<(String, Option<String>)> {
        let unit = self.program().unit(u);
        let name_span = word(&unit.source, declaration)?;
        let name = &unit.source[name_span.start..name_span.end];
        let bodies = &self.query.bodies.units[u.0 as usize];
        let declared = bodies.tipo_local(declaration)?;
        // Função local: a assinatura escrita.
        if let Some(fid) = unit
            .ast
            .functions
            .iter()
            .position(|f| f.name.is_some_and(|n| n.span.start == declaration))
        {
            let f = unit.ast.function(ast::FunctionId(fid as u32));
            return Some((self.local_signature(u, f, name, declared), None));
        }
        let current = self
            .reference_type(unit_expr, expr)
            .unwrap_or(declared);
        // Parâmetro: com os delimitadores do tipo dele (`[int b = 0]`,
        // `{required int c}`), como o `writeFormalParameter` do analyzer.
        let mut text = format!("{} {name}", self.query.format(declared));
        if let Some((p, _)) = crate::project::parameter_in(&unit.ast, declaration)
            && p.name.is_some_and(|n| n.span.start == declaration)
        {
            let pattern = p.default_value.map_or(String::new(), |e| {
                let s = unit.ast.expr(e).span;
                format!(" = {}", &unit.source[s.start..s.end])
            });
            text = match p.kind {
                ParameterKind::Required => text,
                ParameterKind::Optional => format!("[{text}{pattern}]"),
                ParameterKind::Named => format!("{{{}{text}{pattern}}}", if p.required { "required " } else { "" }),
            };
        }
        Some((text, Some(self.query.format(current))))
    }

    /// Assinatura de uma função local pelo tipo inferido e pela árvore.
    fn local_signature(&self, u: UnitId, f: &ast::Function, name: &str, ty: TypeId) -> String {
        let Type::Function {
            ret,
            positional,
            optional,
            named,
            ..
        } = self.query.table.get(ty).clone()
        else {
            return format!("{} {name}", self.query.format(ty));
        };
        let parameters = f.parameters.as_deref().unwrap_or(&[]);
        // Posicionais pela ordem; nomeados pelo nome (o tipo os guarda
        // na ordem canônica, não na escrita).
        let mut positional_ = positional.iter().chain(optional.iter());
        let types: Vec<Option<TypeId>> = parameters
            .iter()
            .map(|p| match p.kind {
                ParameterKind::Named => {
                    let symbol = p.public_name.or(p.name)?.sym;
                    named
                        .iter()
                        .find(|(n, _, _)| *n == symbol)
                        .map(|(_, t, _)| *t)
                }
                _ => positional_.next().copied(),
            })
            .collect();
        format!(
            "{} {name}{}{}",
            self.query.format(ret),
            self.written_type_parameters(&f.type_params, &self.program().unit(u).source),
            self.parameter_list(Some(u), parameters, &types)
        )
    }

    /// `<T extends B, U>` como escrito na fonte (vazio sem parâmetros).
    fn written_type_parameters(&self, ps: &[ast::TypeParameter], source: &str) -> String {
        if ps.is_empty() {
            return String::new();
        }
        let parts: Vec<&str> = ps
            .iter()
            .map(|t| &source[t.span.start..t.span.end])
            .collect();
        format!("<{}>", parts.join(", "))
    }

    /// `(int a, [int b = 0], {required int c})`: `types` alinhados com os
    /// parâmetros escritos em `u`.
    fn parameter_list(
        &self,
        u: Option<UnitId>,
        ps: &[ast::Parameter],
        types: &[Option<TypeId>],
    ) -> String {
        self.parameter_list_with(u, ps, types, &|t| self.query.format(t))
    }

    /// [`Project::parameter_list`] com os tipos formatados por `fmt`.
    fn parameter_list_with(
        &self,
        u: Option<UnitId>,
        ps: &[ast::Parameter],
        types: &[Option<TypeId>],
        fmt: &dyn Fn(TypeId) -> String,
    ) -> String {
        let (source, tree) = match u {
            Some(u) => (
                self.program().unit(u).source.as_str(),
                Some(&self.program().unit(u).ast),
            ),
            None => ("", None),
        };
        // Como o `_writeFormalParameters` do analyzer com `multiline`: com
        // três parâmetros ou mais, um por linha, com vírgula final.
        let multiline = ps.len() >= 3;
        let mut items: Vec<(ParameterKind, String)> = Vec::new();
        for (i, p) in ps.iter().enumerate() {
            let ty = types
                .get(i)
                .copied()
                .flatten()
                .map_or_else(|| "dynamic".to_string(), fmt);
            let name = p
                .public_name
                .or(p.name)
                .map_or("", |n| &source[n.span.start..n.span.end]);
            let pattern = match (p.default_value, tree) {
                (Some(e), Some(a)) => {
                    let s = a.expr(e).span;
                    format!(" = {}", &source[s.start..s.end])
                }
                _ => String::new(),
            };
            let text = if name.is_empty() {
                ty
            } else {
                format!("{ty} {name}{pattern}")
            };
            let text = if p.kind == ParameterKind::Named && p.required {
                format!("required {text}")
            } else {
                text
            };
            items.push((p.kind, text));
        }
        let (opens_group, separator, end, prefix) =
            if multiline { (" ", ",", ",
", "
  ") } else { ("", ", ", "", "") };
        let mut s = String::from("(");
        let mut last: Option<ParameterKind> = None;
        let mut closes = "";
        for (i, (k, text)) in items.iter().enumerate() {
            if i != 0 {
                s.push_str(separator);
            }
            if last != Some(*k) {
                s.push_str(closes);
                if last.is_some() {
                    s.push_str(opens_group);
                }
                let (opens, f) = match k {
                    ParameterKind::Required => ("", ""),
                    ParameterKind::Optional => ("[", "]"),
                    ParameterKind::Named => ("{", "}"),
                };
                s.push_str(opens);
                closes = f;
                last = Some(*k);
            }
            s.push_str(prefix);
            s.push_str(text);
        }
        if !items.is_empty() {
            s.push_str(end);
        }
        s.push_str(closes);
        s.push(')');
        s
    }

    /// Assinatura de uma função, método, getter, setter ou construtor.
    pub(crate) fn describe_function(&self, f: FunctionElementId) -> String {
        self.describe_function_with(f, None)
    }

    /// [`Project::describe_function`] do `Member` substituído (`map`: os
    /// parâmetros de tipo da classe pelos argumentos do receptor).
    fn describe_function_with(&self, f: FunctionElementId, replacement: Option<Replacement>) -> String {
        let p = self.program();
        let fe = p.function(f);
        let data = &self.query.outline.functions[f.0 as usize];
        let name = base_name(self.name(fe.name)).to_string();
        let (map, mut copy) = match replacement {
            Some((m, tb)) => (Some(m), Some(tb)),
            None => (None, None),
        };
        let mut sub = |t: TypeId| -> TypeId {
            match (copy.as_mut(), map.as_ref()) {
                (Some(tb), Some(m)) => dartforge_types::ops::substitute(t, m, tb),
                _ => t,
            }
        };
        let return_t = sub(data.return_type);
        let types: Vec<Option<TypeId>> = data.parameters.iter().map(|q| Some(sub(q.ty))).collect();
        let table = copy.as_ref().unwrap_or(&self.query.table);
        let fmt = |t: TypeId| table.format_sem_alias(t, &self.query.names, &self.query.program);
        let return_ = fmt(return_t);
        let (unit_, parameters, written_types): (
            Option<UnitId>,
            &[ast::Parameter],
            &[ast::TypeParameter],
        ) = match fe.node {
            FunctionRef::Function { unit, function } => {
                let func = p.unit(unit).ast.function(function);
                (
                    Some(unit),
                    func.parameters.as_deref().unwrap_or(&[]),
                    &func.type_params,
                )
            }
            FunctionRef::Constructor { unit, member } => {
                match &p.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => (Some(unit), &k.parameters, &[]),
                    _ => (None, &[], &[]),
                }
            }
            FunctionRef::None => (None, &[], &[]),
        };
        let source = unit_.map_or("", |u| p.unit(u).source.as_str());
        let list = self.parameter_list_with(unit_, parameters, &types, &fmt);
        match fe.kind {
            FunctionKind::Getter => format!("{return_} get {name}"),
            FunctionKind::Setter => format!("set {name}{list}"),
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {
                let class = fe
                    .class
                    .map_or(String::new(), |c| self.name(p.class(c).name).to_string());
                let displayed = if name.is_empty() {
                    class.clone()
                } else {
                    format!("{class}.{name}")
                };
                format!("{return_} {displayed}{list}")
            }
            FunctionKind::ImplicitAccessor => {
                let ty = fe
                    .variable
                    .and_then(|v| self.query.variable_type(v))
                    .unwrap_or(data.return_type);
                format!("{} {name}", self.query.format(ty))
            }
            _ => format!(
                "{return_} {name}{}{list}",
                self.written_type_parameters(written_types, source)
            ),
        }
    }

    /// Classe, mixin, enum, extension type, extensão ou typedef.
    fn describe_type(&self, el: Element) -> Option<String> {
        let p = self.program();
        let fmt = |t: TypeId| self.query.format(t);
        match el {
            Element::Class(c) => {
                let cl = p.class(c);
                let data = self.query.outline.classes.get(c.0 as usize)?;
                let d = cl.decl?;
                let source = &p.unit(d.unit).source;
                let decl = p.unit(d.unit).ast.decl(d.decl);
                let name = self.name(cl.name);
                let list =
                    |ts: &[TypeId]| ts.iter().map(|t| fmt(*t)).collect::<Vec<_>>().join(", ");
                let types = |ps: &[ast::TypeParameter]| self.written_type_parameters(ps, source);
                let mut s = String::new();
                match (&decl.kind, cl.kind) {
                    (DeclKind::Class(k), _) => {
                        let m = k.modifiers;
                        if m.sealed {
                            s.push_str("sealed ");
                        } else if m.abstract_ {
                            s.push_str("abstract ");
                        }
                        if m.base {
                            s.push_str("base ");
                        } else if m.interface {
                            s.push_str("interface ");
                        } else if m.final_ {
                            s.push_str("final ");
                        }
                        if m.mixin {
                            s.push_str("mixin ");
                        }
                        s.push_str(&format!("class {name}{}", types(&k.type_params)));
                        if let Some(sup) = data.supertype
                            && !self.is_object(sup)
                        {
                            s.push_str(&format!(" extends {}", fmt(sup)));
                        }
                        if !data.mixins.is_empty() {
                            s.push_str(&format!(" with {}", list(&data.mixins)));
                        }
                        if !data.interfaces.is_empty() {
                            s.push_str(&format!(" implements {}", list(&data.interfaces)));
                        }
                    }
                    (DeclKind::Mixin(k), _) => {
                        if k.base {
                            s.push_str("base ");
                        }
                        s.push_str(&format!("mixin {name}{}", types(&k.type_params)));
                        let on: Vec<TypeId> = data
                            .on
                            .iter()
                            .copied()
                            .filter(|t| !self.is_object(*t))
                            .collect();
                        if !on.is_empty() {
                            s.push_str(&format!(" on {}", list(&on)));
                        }
                        if !data.interfaces.is_empty() {
                            s.push_str(&format!(" implements {}", list(&data.interfaces)));
                        }
                    }
                    (DeclKind::Enum(k), _) => {
                        s.push_str(&format!("enum {name}{}", types(&k.type_params)));
                        if !data.mixins.is_empty() {
                            s.push_str(&format!(" with {}", list(&data.mixins)));
                        }
                        if !data.interfaces.is_empty() {
                            s.push_str(&format!(" implements {}", list(&data.interfaces)));
                        }
                    }
                    (DeclKind::ExtensionType(k), _) => {
                        let rep = &source[p.unit(d.unit).ast.ty(k.representation_type).span.start
                            ..k.representation_name.span.end];
                        s.push_str(&format!(
                            "extension type {name}{}({rep})",
                            types(&k.type_params)
                        ));
                        if !data.interfaces.is_empty() {
                            s.push_str(&format!(" implements {}", list(&data.interfaces)));
                        }
                    }
                    (_, ClassKind::MixinApplication) => return None,
                    _ => return None,
                }
                Some(s)
            }
            Element::Extension(x) => {
                let ext = p.extension(x);
                let data = self.query.outline.extensions.get(x.0 as usize)?;
                let source = &p.unit(ext.decl.unit).source;
                let DeclKind::Extension(k) = &p.unit(ext.decl.unit).ast.decl(ext.decl.decl).kind
                else {
                    return None;
                };
                let name = ext.name.map_or("", |n| self.name(n));
                let separator = if name.is_empty() { "" } else { " " };
                Some(format!(
                    "extension{separator}{name}{} on {}",
                    self.written_type_parameters(&k.type_params, source),
                    fmt(data.on)
                ))
            }
            Element::Typedef(t) => {
                let td = p.typedef(t);
                let data = self.query.outline.typedefs.get(t.0 as usize)?;
                let source = &p.unit(td.decl.unit).source;
                let DeclKind::Typedef(k) = &p.unit(td.decl.unit).ast.decl(td.decl.decl).kind else {
                    return None;
                };
                Some(format!(
                    "typedef {}{} = {}",
                    self.name(td.name),
                    self.written_type_parameters(&k.type_params, source),
                    fmt(data.target_type)
                ))
            }
            _ => None,
        }
    }

    /// `t` é `Object` (não se escreve `extends Object`).
    fn is_object(&self, t: TypeId) -> bool {
        matches!(self.query.table.get(t), Type::Interface { class, .. } if Some(*class) == self.query.core.object_class)
    }

    /// `T extends B` como declarado.
    fn describe_type_parameter(&self, u: UnitId, declaration: usize) -> Option<String> {
        let unit = self.program().unit(u);
        let ast = &unit.ast;
        let find = |ps: &[ast::TypeParameter]| {
            ps.iter()
                .find(|t| t.name.span.start == declaration)
                .map(|t| t.span)
        };
        let span = ast
            .decls
            .iter()
            .find_map(|d| match &d.kind {
                DeclKind::Class(c) => find(&c.type_params),
                DeclKind::Mixin(m) => find(&m.type_params),
                DeclKind::Enum(e) => find(&e.type_params),
                DeclKind::Extension(x) => find(&x.type_params),
                DeclKind::ExtensionType(x) => find(&x.type_params),
                DeclKind::Typedef(t) => find(&t.type_params),
                _ => None,
            })
            .or_else(|| ast.functions.iter().find_map(|f| find(&f.type_params)))
            .or_else(|| {
                ast.types.iter().find_map(|t| match &t.kind {
                    ast::TypeKind::Function { type_params, .. } => find(type_params),
                    _ => None,
                })
            })?;
        Some(unit.source[span.start..span.end].to_string())
    }
}

/// O elemento cuja documentação o hover procura.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Documentable {
    Function(FunctionElementId),
    Variable(dartforge_elements::model::VariableId),
}

/// Os parâmetros de tipo da classe pelos argumentos do receptor, com a
/// cópia da tabela em que esses argumentos existem.
type Replacement = (std::collections::HashMap<dartforge_types::TypeParamId, TypeId>, dartforge_types::TypeTable);

/// A declaração em `declaration` é variável de um `for` de coleção
/// (`[for (var i = 0; …) …]`, `{for (var x in l) …}`, inclusive por padrão).
fn in_collection_for(ast: &ast::Ast, declaration: usize) -> bool {
    fn node_pattern(ast: &ast::Ast, p: ast::PatternId, d: usize) -> bool {
        let s = ast.pattern(p).span;
        s.start <= d && d < s.end
    }
    fn scan(ast: &ast::Ast, els: &[ast::CollectionElement], d: usize) -> bool {
        els.iter().any(|el| match el {
            ast::CollectionElement::For { init, body, .. } => {
                let here = match init {
                    Some(ast::ForInit::Variables(vl)) => vl.variables.iter().any(|v| v.name.span.start == d),
                    Some(ast::ForInit::Pattern { pattern, .. }) => node_pattern(ast, *pattern, d),
                    _ => false,
                };
                here || scan(ast, std::slice::from_ref(body), d)
            }
            ast::CollectionElement::ForIn { target, body, .. } => {
                let here = match target {
                    ast::ForInTarget::Declared { name, .. } => name.span.start == d,
                    ast::ForInTarget::Pattern { pattern, .. } => node_pattern(ast, *pattern, d),
                    ast::ForInTarget::Expression(_) => false,
                };
                here || scan(ast, std::slice::from_ref(body), d)
            }
            ast::CollectionElement::If { then, else_, .. } => {
                scan(ast, std::slice::from_ref(then), d) || else_.as_ref().is_some_and(|x| scan(ast, std::slice::from_ref(x), d))
            }
            _ => false,
        })
    }
    ast.exprs.iter().any(|e| match &e.kind {
        ast::ExprKind::List { elements, .. } | ast::ExprKind::SetOrMap { elements, .. } => scan(ast, elements, declaration),
        _ => false,
    })
}
