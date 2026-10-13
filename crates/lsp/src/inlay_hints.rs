//! Dicas embutidas (`textDocument/inlayHint`) no formato do
//! `DartInlayHintComputer` do Dart 3.6.2
//! (`pkg/analysis_server/lib/src/computer/computer_inlay_hint.dart`):
//!
//! * tipo (espécie 1) antes do nome de variável declarada sem tipo (`var`,
//!   `final`, `const`, campos e variáveis de topo), de variável de `for-in`
//!   sem tipo, de variável de padrão sem tipo e de parâmetro sem tipo (o de
//!   *closure* inclusive; `this.x`, `super.x` e parâmetros-função não);
//! * tipo de retorno antes do nome de método declarado sem tipo de retorno
//!   (o setter inclusive, com `void`), e antes do nome (ou do `get`) de
//!   função sem ele (setter de topo não);
//! * nome do parâmetro (espécie 2, `name:`) antes de cada argumento
//!   posicional de toda lista de argumentos que resolve para uma função com
//!   parâmetros declarados: chamadas, criações, anotações e os `super(…)` e
//!   `this(…)` de construtores;
//! * argumentos de tipo inferidos (`<int>`) antes do `[`/`{` de literal de
//!   coleção sem argumentos escritos, e depois do nome da classe numa
//!   criação de instância sem argumentos escritos;
//! * os argumentos de tipo escolhidos de uma invocação genérica sem
//!   argumentos escritos (`visitInvocationExpression`, os `typeArgumentTypes`),
//!   antes do `(`;
//! * os argumentos de um tipo nomeado escrito sem eles (`visitNamedType`:
//!   `List` cru dá `<dynamic>`), depois do tipo.
//!
//! Os nomes de parâmetro de uma função do SDK são os da declaração pública,
//! não os do patch (o analyzer não aplica patches).
//!
//! O analyzer devolve as dicas do arquivo inteiro, sem olhar o intervalo
//! pedido; aqui também. Tipos vêm da inferência comum (`local_type`,
//! `get_type`, outline), nunca de uma inferência própria.

use crate::project::Project;
use dartforge_elements::model::{FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberKind, ParameterKind, StmtKind};
use dartforge_types::{MemberRef, Resolved, Type, TypeId};
use std::collections::HashMap;

/// Uma dica: offset (bytes) onde aparece, texto, espécie LSP (1 tipo, 2
/// parâmetro) e se leva espaço à direita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub offset: usize,
    pub label: String,
    pub kind: u8,
    pub space_after: bool,
}

impl Project {
    /// As dicas de `unit`.
    pub(crate) fn hints(&self, unit_: UnitId) -> Vec<Hint> {
        let p = self.program();
        let u = p.unit(unit_);
        let ast = &u.ast;
        let bodies = &self.query.bodies.units[unit_.0 as usize];
        let mut output = Vec::new();
        let ty_ = |offset: usize, t: TypeId, output: &mut Vec<Hint>| {
            output.push(Hint { offset, label: self.query.format(t), kind: 1, space_after: true });
        };
        // Variáveis de topo e campos: pelo elemento.
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
        // Função da árvore -> elemento (retornos sem tipo escrito).
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
                    // Parâmetros sem tipo (`SimpleFormalParameter` sem tipo
                    // escrito); o redirecionador não tem corpo, o tipo vem
                    // do outline.
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
                    // `super(…)` e `this(…)`: os nomes dos parâmetros do
                    // construtor chamado.
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
        // Os argumentos das anotações (`visitArgumentList`).
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
                // `for (var i = 0; …)`: a lista do cabeçalho também é
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
        // Parâmetros sem tipo (de funções, métodos e closures).
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
        // Os tipos nomeados das criações (`new C()`) têm a dica pela criação.
        let mut of_creation: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for e in ast.exprs.iter() {
            match &e.kind {
                ExprKind::InstanceCreation { ty, .. } => {
                    of_creation.insert(ty.0);
                }
                // `[for (var i = 0; …) i]`: o cabeçalho do `for` de coleção.
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
                                    // `[for (var x in l) …]`: o `DeclaredIdentifier` sem tipo.
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
        // O tipo de um padrão objeto é o requerido, com os argumentos
        // inferidos (`MapEntry(key: k)` dá `MapEntry<K, V>`).
        let mut of_pattern: HashMap<u32, TypeId> = HashMap::new();
        for (i, pt) in ast.patterns.iter().enumerate() {
            if let ast::PatternKind::Object { ty, .. } = &pt.kind
                && let Some(&t) = bodies.tipos_de_padroes.get(&ast::PatternId(i as u32))
            {
                of_pattern.insert(ty.0, t);
            }
        }
        // Um argumento de tipo aninhado (`Command` em `List<Command>`) não
        // tem registro próprio: o tipo vem do argumento do pai.
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
        // `visitNamedType`: tipo escrito sem argumentos cujo tipo os tem.
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
                    // `visitInvocationExpression`: os argumentos de tipo
                    // escolhidos, antes do `(`, sem argumentos escritos (a
                    // criação implícita não é invocação).
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
                    // `Caixa('a')` sem `new`: os argumentos de tipo inferidos
                    // depois do nome da classe.
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

    /// Tipo de retorno de uma função ou método sem tipo escrito: o método
    /// (`visitMethodDeclaration`) sempre antes do nome, o setter inclusive;
    /// a função (`visitFunctionDeclaration`) antes do `get`, e o setter não.
    fn return_(&self, unit: UnitId, f: ast::FunctionId, element: Option<FunctionElementId>, method: bool, output: &mut Vec<Hint>) {
        let p = self.program();
        let Some(fe) = element else { return };
        let func = p.unit(unit).ast.function(f);
        if func.return_type.is_some() || (!method && (func.kind == ast::FunctionKind::Setter || p.function(fe).kind == FunctionKind::Setter)) {
            return;
        }
        let Some(n) = func.name else { return };
        // Getter de topo: antes do `get`.
        let source = &p.unit(unit).source;
        let offset = if !method && func.kind == ast::FunctionKind::Getter {
            source[..n.span.start].trim_end().strip_suffix("get").map_or(n.span.start, str::len)
        } else {
            n.span.start
        };
        let t = self.query.outline.functions[fe.0 as usize].return_type;
        output.push(Hint { offset, label: self.query.format(t), kind: 1, space_after: true });
    }

    /// O construtor que a anotação `@C(…)`, `@C.n(…)`, `@p.C(…)` ou
    /// `@p.C.n(…)` invoca.
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

    /// Nomes dos parâmetros posicionais de `f`, na ordem (os da declaração
    /// pública de um membro de patch do SDK).
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

    /// `name:` antes de cada argumento posicional.
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

    /// `<A, B>` dos argumentos de um tipo de interface, em `offset`.
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

    /// `_appendTypePart`: o nome do elemento (não o do alias), os argumentos
    /// e o `?`; o resto pelo `getDisplayString`.
    fn type_text(&self, t: TypeId) -> String {
        self.query.table.format_sem_alias(t, &self.query.names, &self.query.program)
    }
}

/// Nomes dos parâmetros posicionais da função local declarada em `decl`.
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
