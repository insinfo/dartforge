//! Destaques no documento, implementações, definição do tipo e hierarquia
//! de tipos, sobre o mesmo modelo de identidade de `crate::project` (o que
//! a posição denota vem de [`Project::identify`]).
//!
//! Regras do servidor do Dart (ver `docs/LSP.md`, "Paridade com o servidor
//! do Dart"):
//!
//! * `documentHighlight` — as ocorrências do elemento sob o cursor no
//!   próprio arquivo, declaração incluída (`DartUnitOccurrencesComputer`);
//! * `implementation` — numa classe (ou num construtor dela), os subtipos
//!   transitivos; num membro de instância, a declaração do membro em cada
//!   subtipo que o declara (ou o recebe de um mixin); nada para locais,
//!   topo que não é classe e membros de extension type
//!   (`ImplementationHandler` + `TypeHierarchyComputerHelper`);
//! * `typeDefinition` — o tipo estático do que está sob o cursor (a classe
//!   num nome de tipo, o tipo declarado numa declaração de variável ou
//!   parâmetro, o tipo estático numa expressão), só quando é tipo de
//!   interface (`TypeDefinitionHandler`);
//! * hierarquia de tipos (`DartLazyTypeHierarchyComputer`) — o alvo é o
//!   tipo escrito do `NamedType` sob o cursor (`List<String>`, `String?`) ou
//!   o `thisType` da classe que o contém; supertipos diretos (superclasse,
//!   `on`, interfaces, mixins) com os argumentos levados pela âncora; subtipos
//!   diretos pelas relações do índice; o item leva o `ElementLocation` da
//!   classe em `data.ref`.

use crate::arvore_analyzer::Marca;

/// O elemento de que o `dart/textDocument/super` procura o "super".
enum SuperElement {
    Constructor(dartforge_elements::model::FunctionElementId),
    Class(ClassId),
    /// O membro herdado de nome `name` (o de setter com `=`) na `class`.
    Other { name: String, setter: bool, class: ClassId },
}
use crate::project::{Target, Concrete, Owner, Project};
use crate::refactorings::Context;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind, LibraryId, UnitId, UnitRole};
use dartforge_frontend::ast::{self, DeclKind};
use dartforge_frontend::token::Kind;
use dartforge_types::table::Exibicao;
use dartforge_types::{Type, TypeId, TypeParamId};
use std::collections::{BTreeSet, HashMap};

impl Project {
    /// Os destaques de `offset` em `unit` (`DartUnitOccurrencesComputer`,
    /// `document_highlight.rs`): lista vazia quando nada cobre o cursor.
    pub(crate) fn document_highlights(&self, unit: UnitId, offset: usize) -> Option<Vec<Span>> {
        Some(self.dart_highlights(unit, offset))
    }

    /// As classes que têm `c` como supertipo (transitivo), com declaração.
    pub(crate) fn subtypes(&self, c: ClassId, direct: bool) -> Vec<ClassId> {
        let p = self.program();
        (0..p.classes.len())
            .map(|i| ClassId(i as u32))
            .filter(|x| *x != c && p.class(*x).decl.is_some() && p.class(*x).kind != ClassKind::MixinApplication)
            .filter(|x| {
                if direct {
                    let cl = p.class(*x);
                    cl.supertype_class.iter().chain(&cl.mixin_classes).chain(&cl.interface_classes).chain(&cl.on_classes).any(|s| {
                        *s == c || (p.class(*s).kind == ClassKind::MixinApplication && self.supertypes(*s).contains(&c))
                    })
                } else {
                    self.supertypes(*x).contains(&c)
                }
            })
            .collect()
    }

    /// `dart/textDocument/super` (docs/LSP-ESPECIFICACAO.md §8.6; o
    /// `_SuperComputer` de `handler_super.dart`): o "super" do elemento sob
    /// o cursor — ou, sem elemento ali, o da declaração que o envolve —: o
    /// construtor da superclasse que um construtor chama, a superclasse de
    /// uma classe, ou o membro herdado do mesmo nome (`getInherited2`) na
    /// classe que envolve o elemento. A posição é a do nome do elemento não
    /// sintético (a classe, no construtor implícito; o campo, no acessor).
    pub(crate) fn upper(&mut self, unit: UnitId, offset: usize) -> Option<(UnitId, Span)> {
        match self.super_element(unit, offset)? {
            SuperElement::Constructor(f) => {
                let target = self.super_constructor(f)?;
                self.function_name(target)
            }
            SuperElement::Class(c) => {
                let s = self.non_synthetic_superclass(c)?;
                self.top_element_name(Element::Class(s))
            }
            SuperElement::Other { name, setter, class } => {
                let member = self.inherited_member(class, &name, setter)?;
                self.function_name(member)
            }
        }
    }

    /// O elemento de `offset` para o `super`: o que o nome ali denota, ou o
    /// da declaração que o envolve (o `getElementOfNode` subindo pelos nós).
    fn super_element(&self, unit: UnitId, offset: usize) -> Option<SuperElement> {
        let p = self.program();
        let is_constructor = |f: dartforge_elements::model::FunctionElementId| matches!(p.function(f).kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor);
        if let Ok(Some(d)) = self.identify(unit, offset) {
            return match (&d.target, d.concrete) {
                (_, Some(Concrete::Function(f))) if is_constructor(f) => Some(SuperElement::Constructor(f)),
                (Target::Constructor(f), _) => Some(SuperElement::Constructor(*f)),
                (Target::Top(Element::Class(c)), _) => {
                    // A mixin não tem `supertype` (só as restrições `on`).
                    (p.class(*c).kind != ClassKind::Mixin).then_some(SuperElement::Class(*c))
                }
                (Target::Member { owner: Owner::Class(c), name, .. }, concrete) => {
                    let setter = matches!(concrete, Some(Concrete::Function(f)) if self.is_setter(f));
                    Some(SuperElement::Other { name: name.clone(), setter, class: *c })
                }
                // Um local ou um parâmetro de tipo: o nome dele na classe que
                // o envolve (`thisOrAncestorOfType<InterfaceElement>`).
                (Target::Local { .. } | Target::TypeParameter { .. }, _) => {
                    let class = self.enclosing_class(unit, offset)?;
                    let source = &p.unit(unit).source;
                    Some(SuperElement::Other { name: source[d.name.start..d.name.end].to_string(), setter: false, class })
                }
                _ => None,
            };
        }
        self.enclosing_declaration(unit, offset)
    }

    fn is_setter(&self, f: dartforge_elements::model::FunctionElementId) -> bool {
        let p = self.program();
        let e = p.function(f);
        match e.kind {
            FunctionKind::Setter => true,
            FunctionKind::ImplicitAccessor => e.variable.is_some_and(|v| p.variable(v).setter == Some(f)),
            _ => false,
        }
    }

    /// A classe (de interface: classe, mixin, enum, extension type) cuja
    /// declaração contém `offset`.
    fn enclosing_class(&self, unit: UnitId, offset: usize) -> Option<ClassId> {
        let p = self.program();
        let ast = &p.unit(unit).ast;
        let decl = ast.decls.iter().position(|d| {
            d.span.start <= offset
                && offset <= d.span.end
                && matches!(
                    d.kind,
                    dartforge_frontend::ast::DeclKind::Class(_)
                        | dartforge_frontend::ast::DeclKind::Mixin(_)
                        | dartforge_frontend::ast::DeclKind::Enum(_)
                        | dartforge_frontend::ast::DeclKind::ExtensionType(_)
                )
        })?;
        (0..p.classes.len())
            .map(|i| ClassId(i as u32))
            .find(|c| p.class(*c).decl.is_some_and(|d| d.unit == unit && d.decl.0 as usize == decl))
    }

    /// Sem nome sob o cursor: a função (local, ou de expressão, que não tem
    /// nome) mais interna que contém `offset`, o membro, ou a classe.
    fn enclosing_declaration(&self, unit_: UnitId, offset: usize) -> Option<SuperElement> {
        use dartforge_elements::model::FunctionRef;
        use dartforge_frontend::ast::MemberKind;
        let p = self.program();
        let ast = &p.unit(unit_).ast;
        let contains = |s: Span| s.start <= offset && offset <= s.end;
        // O membro que contém o offset.
        let member_ = ast.members.iter().enumerate().filter(|(_, m)| contains(m.span)).min_by_key(|(_, m)| m.span.end - m.span.start);
        // A função mais interna (local ou de expressão) dentro do membro.
        let member_function = member_.and_then(|(_, m)| match m.kind {
            MemberKind::Method(f) => Some(f),
            _ => None,
        });
        let internal = ast
            .functions
            .iter()
            .enumerate()
            .filter(|(i, f)| contains(f.span) && Some(dartforge_frontend::ast::FunctionId(*i as u32)) != member_function)
            .filter(|(i, _)| !ast.decls.iter().any(|d| matches!(d.kind, dartforge_frontend::ast::DeclKind::Function(x) if x.0 as usize == *i)))
            .min_by_key(|(_, f)| f.span.end - f.span.start);
        if let Some((_, f)) = internal {
            // A função de expressão tem nome vazio: nenhum membro herdado.
            let name = f.name?;
            let class = self.enclosing_class(unit_, offset)?;
            return Some(SuperElement::Other { name: self.name(name.sym).to_string(), setter: false, class });
        }
        if let Some((i, m)) = member_ {
            let class = self.enclosing_class(unit_, offset);
            return match &m.kind {
                MemberKind::Constructor(_) => {
                    let f = (0..p.functions.len())
                        .map(|k| dartforge_elements::model::FunctionElementId(k as u32))
                        .find(|f| matches!(p.function(*f).node, FunctionRef::Constructor { unit, member } if unit == unit_ && member.0 as usize == i))?;
                    Some(SuperElement::Constructor(f))
                }
                MemberKind::Method(fid) => {
                    let f = ast.function(*fid);
                    let setter = f.kind == dartforge_frontend::ast::FunctionKind::Setter;
                    Some(SuperElement::Other { name: self.name(f.name?.sym).to_string(), setter, class: class? })
                }
                MemberKind::Field(l) => {
                    let v = l.variables.iter().find(|v| v.initializer.is_some_and(|e| contains(ast.expr(e).span)) || contains(v.name.span)).or_else(|| l.variables.first())?;
                    Some(SuperElement::Other { name: self.name(v.name.sym).to_string(), setter: false, class: class? })
                }
            };
        }
        let class = self.enclosing_class(unit_, offset)?;
        (p.class(class).kind != ClassKind::Mixin).then_some(SuperElement::Class(class))
    }

    /// A superclasse de `c`, passando pelas aplicações de mixin sintéticas
    /// (o analyzer não as tem: `supertype` de `C extends S with M` é `S`).
    pub(crate) fn non_synthetic_superclass(&self, c: ClassId) -> Option<ClassId> {
        let p = self.program();
        let mut s = p.class(c).supertype_class?;
        let mut steps = 0;
        while p.class(s).decl.is_none() && p.class(s).kind == ClassKind::MixinApplication && steps < 64 {
            s = p.class(s).supertype_class?;
            steps += 1;
        }
        Some(s)
    }

    /// `ConstructorElement.superConstructor`: o construtor da superclasse
    /// que o construtor generativo `f` chama (o do `super(…)`/`super.name(…)`
    /// escrito, ou o sem nome); nenhum numa fábrica ou num redirecionador.
    pub(crate) fn super_constructor(&self, f: dartforge_elements::model::FunctionElementId) -> Option<dartforge_elements::model::FunctionElementId> {
        use dartforge_elements::model::FunctionRef;
        use dartforge_frontend::ast::{Initializer, MemberKind};
        let p = self.program();
        let fe = p.function(f);
        if fe.factory {
            return None;
        }
        let class = fe.class?;
        let name = match fe.node {
            FunctionRef::Constructor { unit, member } => match &p.unit(unit).ast.member(member).kind {
                MemberKind::Constructor(k) => {
                    if k.initializers.iter().any(|i| matches!(i, Initializer::Redirect { .. })) {
                        return None;
                    }
                    k.initializers.iter().find_map(|i| match i {
                        Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
                        _ => None,
                    })
                }
                _ => None,
            }
            .flatten(),
            _ => None,
        };
        let s = self.non_synthetic_superclass(class)?;
        let key = match name {
            Some(n) => n,
            None => self.query.names.lookup("")?,
        };
        p.class(s).constructors.get(&key).copied()
    }

    /// `InheritanceManager3.getInherited2(classe, Name(biblioteca da classe,
    /// nome))`.
    fn inherited_member(&mut self, class: ClassId, name: &str, setter: bool) -> Option<dartforge_elements::model::FunctionElementId> {
        let crate::query::Query { program, names, core, outline, table, .. } = &mut self.query;
        let symbol = names.lookup(name)?;
        let mut n = dartforge_types::heranca::Nome::novo(names, program.class(class).library, symbol);
        if setter {
            n = n.de_setter(names)?;
        }
        let mut provider = dartforge_types::heranca::ProvedorDoOutline { program: program, interner: names, core, outline, table: table };
        let mut inheritance = dartforge_types::heranca::Heranca::default();
        inheritance.herdado(&mut provider, class, n).map(|m| m.funcao)
    }

    /// As implementações do que `offset` denota: subtipos de uma classe, ou
    /// o membro declarado em cada subtipo.
    pub(crate) fn implementations(&self, unit: UnitId, offset: usize) -> Vec<(UnitId, Span)> {
        let Ok(Some(d)) = self.identify(unit, offset) else { return Vec::new() };
        let p = self.program();
        let (class, member) = match (&d.target, d.concrete) {
            (Target::Top(Element::Class(c)), _) => (*c, None),
            (Target::Constructor(f), _) => match p.function(*f).class {
                Some(c) => (c, None),
                None => return Vec::new(),
            },
            (Target::Member { owner: Owner::Class(c), name, is_static }, concrete) => {
                // Construtor chamado por `A.name()` resolve para a classe.
                if let Some(Concrete::Function(f)) = concrete
                    && matches!(p.function(f).kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor)
                {
                    (*c, None)
                } else {
                    (*c, Some((name.clone(), *is_static)))
                }
            }
            _ => return Vec::new(),
        };
        // Membros de extension type não sobrescrevem: só redeclaram.
        let is_extension_type = |c: ClassId| {
            p.class(c).decl.is_some_and(|dr| matches!(p.unit(dr.unit).ast.decl(dr.decl).kind, dartforge_frontend::ast::DeclKind::ExtensionType(_)))
        };
        // `TypeHierarchyComputerHelper`: a espécie do pivô (o elemento do
        // nó: o getter de uma leitura, o setter de uma escrita, o campo na
        // declaração) e a biblioteca dele.
        let kind = match d.concrete {
            Some(Concrete::Function(f)) => {
                let fe = p.function(f);
                match fe.kind {
                    FunctionKind::Getter => PivotKind::Getter,
                    FunctionKind::Setter => PivotKind::Setter,
                    FunctionKind::ImplicitAccessor if self.name(fe.name).ends_with('=') => PivotKind::Setter,
                    FunctionKind::ImplicitAccessor => PivotKind::Getter,
                    _ => PivotKind::Method,
                }
            }
            Some(Concrete::Variable(v)) => match d.expr {
                Some(e) if self.write_target(unit, e) => PivotKind::Setter,
                Some(_) => PivotKind::Getter,
                None => PivotKind::Field { final_: p.variable(v).final_ || p.variable(v).const_ },
            },
            None => PivotKind::Method,
        };
        let pivot = match d.concrete {
            Some(Concrete::Function(f)) => Some(f),
            _ => None,
        };
        let pivot_library = match d.concrete {
            Some(Concrete::Function(f)) => p.function(f).library,
            Some(Concrete::Variable(v)) => p.variable(v).library,
            None => p.class(class).library,
        };
        // `needsMember`: o pivô tem o membro na própria classe.
        let needs_member = match &member {
            Some((name, _)) => self.implementation_member(class, name, kind, pivot_library, pivot).is_some() || {
                // O próprio pivô (o lookup acha ele mesmo, sem mixin).
                pivot.is_some_and(|f| p.function(f).class == Some(class))
                    || matches!(d.concrete, Some(Concrete::Variable(v)) if p.variable(v).class == Some(class))
            },
            None => false,
        };
        let mut seen = BTreeSet::new();
        let mut output = Vec::new();
        for x in self.subtypes(class, false) {
            let local = match &member {
                Some(_) if needs_member && (is_extension_type(class) || is_extension_type(x)) => None,
                Some((name, _)) if needs_member => self.implementation_member(x, name, kind, pivot_library, pivot),
                _ => self.top_element_name(Element::Class(x)),
            };
            if let Some((u, s)) = local
                && seen.insert((u, s.start))
            {
                output.push((u, s));
            }
        }
        output
    }

    /// `findMemberElement`: o membro da espécie do pivô declarado em `c`
    /// (acessível na biblioteca do pivô), senão no último mixin que o
    /// declara; a localização do não sintético (o campo de um acessor
    /// implícito).
    fn implementation_member(&self, c: ClassId, name: &str, kind: PivotKind, library: LibraryId, pivot: Option<dartforge_elements::model::FunctionElementId>) -> Option<(UnitId, Span)> {
        let p = self.program();
        let find = |x: ClassId| -> Option<dartforge_elements::model::FunctionElementId> {
            let k = p.class(x);
            let declared = |key: &str, accepts: &dyn Fn(FunctionKind) -> bool| -> Option<dartforge_elements::model::FunctionElementId> {
                let s = self.query.names.lookup(key)?;
                [k.instance_members.get(&s), k.static_members.get(&s)].into_iter().flatten().copied().find(|&f| accepts(p.function(f).kind))
            };
            let method = |_: ()| declared(name, &|e| matches!(e, FunctionKind::Function | FunctionKind::Operator));
            let getter = |_: ()| declared(name, &|e| matches!(e, FunctionKind::Getter | FunctionKind::ImplicitAccessor));
            let setter = |_: ()| declared(&format!("{name}_="), &|e| matches!(e, FunctionKind::Setter | FunctionKind::ImplicitAccessor));
            match kind {
                PivotKind::Method => method(()),
                PivotKind::Getter => getter(()),
                PivotKind::Setter => setter(()),
                PivotKind::Field { final_ } => getter(()).or_else(|| if final_ { None } else { setter(()) }),
            }
        };
        let accessible = |x: ClassId| !name.starts_with('_') || p.class(x).library == library;
        let local = |f: dartforge_elements::model::FunctionElementId| match (p.function(f).kind, p.function(f).variable) {
            (FunctionKind::ImplicitAccessor, Some(v)) => self.variable_name(v),
            _ => self.function_name(f),
        };
        if let Some(f) = find(c)
            && accessible(c)
        {
            return local(f);
        }
        for &m in p.class(c).mixin_classes.iter().rev() {
            if let Some(f) = find(m).filter(|_| accessible(m)) {
                if Some(f) == pivot {
                    return None;
                }
                return local(f);
            }
        }
        None
    }

    /// A declaração do tipo estático do que `offset` denota.
    pub(crate) fn type_definition(&self, unit: UnitId, offset: usize) -> Option<(UnitId, Span)> {
        let p = self.program();
        let ast = &p.unit(unit).ast;
        let inside = |s: Span| s.start <= offset && offset <= s.end;
        // O nome de uma anotação: o `SimpleIdentifier` da classe dá o
        // `thisType`; o de uma variável constante (`@override`), o prefixo e
        // o nome do construtor não têm tipo estático.
        for a in crate::project::metadata(ast, &p.unit(unit).unit) {
            let Some(i) = a.name.iter().position(|n| inside(n.span)) else { continue };
            let lib = p.unit(unit).library;
            let prefixed = a.name.len() >= 2 && crate::project::is_prefix(p, lib, a.name[0].sym);
            let (class_index, binding) = if prefixed {
                (1, p.lookup_prefixed_na_unidade(unit, a.name[0].sym, a.name[1].sym))
            } else {
                (0, p.lookup_na_unidade(unit, a.name[0].sym))
            };
            return match binding.and_then(|b| b.getter) {
                Some(Element::Class(c)) if i == class_index => self.top_element_name(Element::Class(c)),
                _ => None,
            };
        }
        // O `returnType` da declaração de construtor (`A(…)`, o `A` de
        // `A.n(…)`) é a classe; o nome do construtor (`n`) não é nó com tipo;
        // o campo de um inicializador (`x = …`) também não.
        for (i, m) in ast.members.iter().enumerate() {
            let ast::MemberKind::Constructor(k) = &m.kind else { continue };
            if inside(k.class_name.span) {
                let f = self.node_constructor(unit, ast::MemberId(i as u32))?;
                return self.top_element_name(Element::Class(p.function(f).class?));
            }
            if k.initializers.iter().any(|x| matches!(x, ast::Initializer::Field { name, .. } if inside(name.span))) {
                return None;
            }
        }
        let d = self.identify(unit, offset).ok()??;
        // O nome de uma declaração de classe, função, método ou constante
        // de enum não é um nó com tipo para o analyzer.
        let declared = match (&d.target, d.concrete) {
            (Target::Top(Element::Class(c)), None) => self.top_element_name(Element::Class(*c)),
            (_, Some(Concrete::Function(f))) => self.function_name(f),
            (_, Some(Concrete::Variable(v)))
                if self.program().variable(v).class.is_some_and(|c| self.program().class(c).enum_constants.contains(&v)) =>
            {
                self.variable_name(v)
            }
            _ => None,
        };
        if d.expr.is_none() && declared.is_some_and(|(u, s)| u == unit && s == d.name) {
            return None;
        }
        let bodies = &self.query.bodies.units[unit.0 as usize];
        let ty = match (&d.target, d.concrete) {
            // Nome de classe (tipo, referência, construtor sem nome).
            (Target::Top(Element::Class(c)), _) => return self.top_element_name(Element::Class(*c)),
            // Construtor nomeado: o analyzer olha o nome (expressão sem tipo
            // de interface).
            (Target::Constructor(_), _) => return None,
            (_, Some(Concrete::Function(f)))
                if !matches!(self.program().function(f).kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) =>
            {
                // Método ou função: o tipo é uma função.
                return None;
            }
            _ => match d.expr {
                // `inSetterContext`: o tipo da variável do `writeElement`.
                Some(e) if self.write_target(unit, e) && !matches!(d.target, Target::Local { .. }) => match d.concrete {
                    Some(Concrete::Variable(v)) => self.query.variable_type(v)?,
                    Some(Concrete::Function(f)) => match (self.program().function(f).kind, self.program().function(f).variable) {
                        (FunctionKind::ImplicitAccessor, Some(v)) => self.query.variable_type(v)?,
                        (FunctionKind::Setter, _) => self.query.outline.functions[f.0 as usize].parameters.first()?.ty,
                        _ => bodies.get_type(e)?,
                    },
                    None => bodies.get_type(e)?,
                },
                Some(e) => bodies.get_type(e)?,
                None => match (&d.target, d.concrete) {
                    (Target::Local { unit: u, declaration }, _) => self.query.bodies.units[u.0 as usize].tipo_local(*declaration)?,
                    (_, Some(Concrete::Variable(v))) => self.query.variable_type(v)?,
                    (_, Some(Concrete::Function(f))) => self.query.outline.functions[f.0 as usize].return_type,
                    _ => return None,
                },
            },
        };
        let class_ = match self.query.table.get(ty) {
            Type::Interface { class, .. } => *class,
            Type::ExtensionType { decl, .. } => *decl,
            _ => return None,
        };
        self.top_element_name(Element::Class(class_))
    }

    /// `e` é escrito: o alvo de uma atribuição ou de `++`/`--`.
    fn write_target(&self, unit: UnitId, e: dartforge_frontend::ast::ExprId) -> bool {
        use dartforge_frontend::ast::{ExprKind, UnaryOp};
        self.program().unit(unit).ast.exprs.iter().any(|x| match &x.kind {
            ExprKind::Assign { target, .. } => *target == e,
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => *operand == e,
            _ => false,
        })
    }

    // -- Hierarquia de tipos (`DartLazyTypeHierarchyComputer`) ---------------

    /// A classe e os argumentos de um tipo que o analyzer tem como
    /// `InterfaceType`: classe, mixin, enum, extension type, `FutureOr` e
    /// `Null`; nada no `InvalidType`, no `Never?` e nos outros tipos.
    pub(crate) fn interface_class(&self, t: TypeId) -> Option<(ClassId, Vec<TypeId>)> {
        let table = &self.query.table;
        if matches!(table.exibicao(t), Some(Exibicao::Invalido | Exibicao::NeverAnulavel)) {
            return None;
        }
        match table.get(t) {
            Type::Interface { class, args, .. } => Some((*class, args.to_vec())),
            Type::ExtensionType { decl, args, .. } => Some((*decl, args.to_vec())),
            Type::FutureOr { arg, .. } => Some((self.sdk_class(self.query.core.async_library?, "FutureOr")?, vec![*arg])),
            Type::Null => Some((self.query.core.null_class?, Vec::new())),
            _ => None,
        }
    }

    /// A classe `name` declarada na biblioteca `lib`.
    fn sdk_class(&self, lib: LibraryId, name: &str) -> Option<ClassId> {
        let symbol = self.query.names.lookup(name)?;
        match self.program().library(lib).declared.get(&symbol)?.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        }
    }

    /// O `thisType` de `c`: os argumentos são os próprios parâmetros de tipo.
    fn type_this(&mut self, c: ClassId) -> (TypeId, Vec<TypeId>) {
        let parameters: Vec<TypeParamId> = self.query.outline.classes.get(c.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
        let extension = self.program().class(c).kind == ClassKind::ExtensionType;
        let table = &mut self.query.table;
        let args: Vec<TypeId> = parameters.iter().map(|&param| table.intern(Type::TypeParameter { param, nullable: false })).collect();
        let ty = if extension {
            table.intern(Type::ExtensionType { decl: c, args: args.clone().into_boxed_slice(), nullable: false })
        } else {
            table.intern(Type::Interface { class: c, args: args.clone().into_boxed_slice(), nullable: false })
        };
        (ty, args)
    }

    /// `_isInterfaceTypeInterface`: um tipo de interface não anulável que não
    /// é enum, extension type, `Function` nem `Null`.
    fn valid_interface(&self, t: TypeId) -> bool {
        let Some((c, _)) = self.interface_class(t) else { return false };
        let core = &self.query.core;
        !matches!(self.program().class(c).kind, ClassKind::Enum | ClassKind::ExtensionType)
            && Some(c) != core.function_class
            && Some(c) != core.null_class
            && !self.query.table.get(t).is_declared_nullable()
    }

    /// `_isInterfaceTypeClass`: a interface válida cujo elemento é classe.
    fn valid_class(&self, t: TypeId) -> bool {
        self.valid_interface(t)
            && self.interface_class(t).is_some_and(|(c, _)| matches!(self.program().class(c).kind, ClassKind::Class | ClassKind::MixinApplication))
    }

    /// `isValidExtensionTypeSuperinterface`.
    fn extension_type_superinterface(&self, t: TypeId) -> bool {
        let Some((c, _)) = self.interface_class(t) else { return false };
        let core = &self.query.core;
        let future_or = core.async_library.and_then(|l| self.sdk_class(l, "FutureOr"));
        !self.query.table.get(t).is_declared_nullable()
            && Some(c) != future_or
            && Some(c) != core.function_class
            && Some(c) != core.null_class
            && Some(c) != core.record_class
    }

    /// `_getSupertypes` de `c<args>`: a superclasse (`Object` sem `extends`
    /// válido; `Enum` num enum; nenhuma em `Object`, mixin e extension type),
    /// as restrições `on` (`[Object]` num mixin sem elas), as interfaces e os
    /// mixins, com os parâmetros de `c` substituídos por `args`.
    fn type_supertypes(&mut self, c: ClassId, args: &[TypeId]) -> Vec<TypeId> {
        let Some(data) = self.query.outline.classes.get(c.0 as usize).cloned() else { return Vec::new() };
        let kind = self.program().class(c).kind;
        let core = &self.query.core;
        let object = core.object;
        let is_object = Some(c) == core.object_class;
        let enum_ = core.core_library.and_then(|l| self.sdk_class(l, "Enum"));
        let mut list: Vec<TypeId> = Vec::new();
        match kind {
            ClassKind::Class | ClassKind::MixinApplication => match data.supertype.filter(|&s| self.valid_class(s)) {
                Some(s) => list.push(s),
                None if !is_object => list.push(object),
                None => {}
            },
            ClassKind::Enum => match enum_ {
                Some(e) => list.push(self.query.table.intern(Type::Interface { class: e, args: Box::new([]), nullable: false })),
                None => list.push(object),
            },
            ClassKind::Mixin | ClassKind::ExtensionType => {}
        }
        if kind == ClassKind::Mixin {
            let constraints: Vec<TypeId> = data.on.iter().copied().filter(|&t| self.valid_interface(t)).collect();
            if constraints.is_empty() {
                list.push(object);
            } else {
                list.extend(constraints);
            }
        }
        if kind == ClassKind::ExtensionType {
            list.extend(data.interfaces.iter().copied().filter(|&t| self.extension_type_superinterface(t)));
        } else {
            list.extend(data.interfaces.iter().copied().filter(|&t| self.valid_interface(t)));
        }
        list.extend(data.mixins.iter().copied().filter(|&t| self.valid_interface(t)));
        if data.type_params.is_empty() {
            return list;
        }
        let map: HashMap<TypeParamId, TypeId> = data.type_params.iter().copied().zip(args.iter().copied()).collect();
        list.into_iter().map(|t| dartforge_types::substitute(t, &map, &mut self.query.table)).collect()
    }

    /// A URI do `Source` de uma unidade: a da biblioteca na unidade que a
    /// define; numa parte de biblioteca `dart:`, `dart:core/list.dart`; senão
    /// a URI canônica da unidade.
    fn origin_uri(&self, u: UnitId) -> String {
        let p = self.program();
        let unit = p.unit(u);
        let lib = p.library(unit.library);
        if lib.units.first() == Some(&u) {
            return lib.uri.clone();
        }
        if lib.uri.starts_with("dart:")
            && let (Some(path), Some(base)) = (unit.path.as_deref(), lib.units.first().and_then(|f| p.unit(*f).path.as_deref()).and_then(|x| x.parent()))
            && let Ok(relative) = path.strip_prefix(base)
        {
            return format!("{}/{}", lib.uri, relative.to_string_lossy().replace('\\', "/"));
        }
        unit.uri.clone()
    }

    /// O `ElementLocation.encoding` de uma classe: a URI da biblioteca, a da
    /// unidade e o nome, separados por `;` (o `;` de um componente dobrado).
    pub(crate) fn class_reference(&self, c: ClassId) -> Option<String> {
        let p = self.program();
        let class = p.class(c);
        let d = class.decl?;
        let components = [p.library(class.library).uri.clone(), self.origin_uri(d.unit), self.name(class.name).to_string()];
        Some(components.iter().map(|s| s.replace(';', ";;")).collect::<Vec<_>>().join(";"))
    }

    /// `session.locateElement(ElementLocationImpl.con2(reference))` restrito
    /// a classes: a biblioteca, a unidade e o nome; a declaração `augment`
    /// não é o elemento.
    pub(crate) fn reference_class(&self, reference: &str) -> Option<ClassId> {
        let components = decode_reference(reference);
        let [lib, unit, name] = components.as_slice() else { return None };
        let p = self.program();
        (0..p.classes.len()).map(|i| ClassId(i as u32)).find(|&k| {
            let class = p.class(k);
            class.decl.is_some_and(|d| {
                !p.unit(d.unit).ast.decl(d.decl).augment
                    && self.name(class.name) == name
                    && p.library(class.library).uri == *lib
                    && self.origin_uri(d.unit) == *unit
            })
        })
    }

    /// A classe declarada por `did` em `u`.
    fn declaration_class(&self, u: UnitId, did: ast::DeclId) -> Option<ClassId> {
        let p = self.program();
        (0..p.classes.len()).map(|i| ClassId(i as u32)).find(|&c| p.class(c).decl.is_some_and(|d| d.unit == u && d.decl == did))
    }

    /// O `NamedType.type` do nó `k`: o tipo construído numa criação de
    /// instância, senão o da anotação de tipo com o mesmo intervalo.
    fn named_type_type(&self, cx: &Context<'_>, k: usize) -> Option<TypeId> {
        if let Some(parent) = cx.parent(k)
            && cx.kind(parent) == "ConstructorName"
            && let Some(grandparent) = cx.parent(parent)
            && cx.kind(grandparent) == "InstanceCreationExpression"
        {
            return cx.node_type(grandparent);
        }
        let s = cx.tree.span(k);
        cx.ast
            .types
            .iter()
            .enumerate()
            .filter(|(_, t)| t.span == s && matches!(t.kind, ast::TypeKind::Named { .. } | ast::TypeKind::Void))
            .find_map(|(i, _)| {
                let id = ast::TypeId(i as u32);
                cx.bodies.tipos_de_anotacoes.get(&id).or_else(|| self.query.outline.tipos_escritos.get(&(cx.unit, id))).copied()
            })
    }

    /// `findTarget`: o tipo do `NamedType` que contém o nó que cobre o
    /// cursor (`nodeCovering`); sem ele, o `thisType` da classe, mixin, enum
    /// ou extension type que o contém. Só tipos de interface.
    pub(crate) fn hierarchy_target(&mut self, unit: UnitId, offset: usize) -> Option<(ClassId, TypeId)> {
        let (written, declared) = {
            let cx = Context::new(self, unit);
            let n = cx.coverage(offset, 0)?;
            let chain = cx.tree.cadeia(n);
            match chain.iter().copied().find(|&k| cx.kind(k) == "NamedType") {
                Some(k) => (Some(self.named_type_type(&cx, k)?), None),
                None => {
                    let d = chain
                        .iter()
                        .copied()
                        .find(|&k| matches!(cx.kind(k), "ClassDeclaration" | "MixinDeclaration" | "ExtensionTypeDeclaration" | "EnumDeclaration"))?;
                    let Marca::Decl(did) = cx.tree.nos[d].marca else { return None };
                    (None, Some(self.declaration_class(unit, did)?))
                }
            }
        };
        let ty = match (written, declared) {
            (Some(t), _) => t,
            (None, Some(c)) => self.type_this(c).0,
            (None, None) => return None,
        };
        let (c, _) = self.interface_class(ty)?;
        Some((c, ty))
    }

    /// `TypeHierarchyItem.forType`: o `getDisplayString()` do tipo, a
    /// referência da classe, o arquivo, o código da declaração (com a
    /// documentação e as anotações) e o nome.
    pub(crate) fn type_item(&self, c: ClassId, ty: TypeId) -> Option<crate::TypeItem> {
        self.type_items(&[(c, ty)]).pop()
    }

    /// Os itens de vários tipos, com uma árvore por arquivo.
    fn type_items(&self, types: &[(ClassId, TypeId)]) -> Vec<crate::TypeItem> {
        let p = self.program();
        let mut trees: HashMap<UnitId, Context<'_>> = HashMap::new();
        let mut output = Vec::new();
        for &(c, ty) in types {
            let Some(d) = p.class(c).decl else { continue };
            let Some((u, selection)) = self.top_element_name(Element::Class(c)) else { continue };
            let cx = trees.entry(d.unit).or_insert_with(|| Context::new(self, d.unit));
            let Some(no) = cx.tree.nos.iter().position(|n| matches!(n.marca, Marca::Decl(x) if x == d.decl)) else { continue };
            let range = cx.tree.span(no);
            let (Some(uri), Some(reference)) = (self.unit_uri(u), self.class_reference(c)) else { continue };
            output.push(crate::TypeItem {
                name: self.query.table.format_sem_alias(ty, &self.query.names, &self.query.program),
                uri,
                range,
                selection,
                reference,
                anchor: None,
            });
        }
        output
    }

    /// `_locateTargetFromAnchor`: do `thisType` da âncora, segue os índices
    /// pelos supertipos; o tipo encontrado vale se o elemento é `target`.
    fn type_by_anchor(&mut self, reference: &str, path: &[usize], target: ClassId) -> Option<(ClassId, Vec<TypeId>)> {
        let root = self.reference_class(reference)?;
        let mut current = Some((root, self.type_this(root).1));
        for &i in path {
            let Some((c, args)) = current else { break };
            let supertypes = self.type_supertypes(c, &args);
            current = supertypes.get(i).and_then(|t| self.interface_class(*t));
        }
        current.filter(|(c, _)| *c == target)
    }

    /// `findSupertypes` (`typeHierarchy/supertypes`): `None` quando a
    /// referência não localiza uma classe. Os supertipos com argumentos de
    /// tipo levam a âncora (a do pedido, ou a do próprio item com caminho
    /// vazio) com o índice acrescentado.
    pub(crate) fn reference_supertypes(&mut self, reference: &str, anchor: Option<(&str, &[usize])>) -> Option<Vec<crate::TypeItem>> {
        let c = self.reference_class(reference)?;
        let mut target = (c, self.type_this(c).1);
        let (anchor_ref, path) = match anchor {
            Some((r, path)) => {
                if let Some(t) = self.type_by_anchor(r, path, c) {
                    target = t;
                }
                (r.to_string(), path.to_vec())
            }
            None => (reference.to_string(), Vec::new()),
        };
        let supertypes = self.type_supertypes(target.0, &target.1);
        let mut output = Vec::new();
        for (i, t) in supertypes.into_iter().enumerate() {
            let Some((k, args)) = self.interface_class(t) else { continue };
            let Some(mut item) = self.type_item(k, t) else { continue };
            if !args.is_empty() {
                let mut new = path.clone();
                new.push(i);
                item.anchor = Some((anchor_ref.clone(), new));
            }
            output.push(item);
        }
        Some(output)
    }

    /// `findSubtypes` (`typeHierarchy/subtypes`): `None` quando a referência
    /// não localiza uma classe; cada subtipo direto pelo seu `thisType`.
    pub(crate) fn reference_subtypes(&mut self, reference: &str) -> Option<Vec<crate::TypeItem>> {
        let c = self.reference_class(reference)?;
        let subtypes = self.index_subtypes(c);
        let types: Vec<(ClassId, TypeId)> = subtypes.into_iter().map(|k| (k, self.type_this(k).0)).collect();
        Some(self.type_items(&types))
    }

    /// O tipo escrito `t` de `u` nomeia `target` diretamente (o
    /// `NamedType.element` do índice: um alias `typedef` não conta).
    fn names_a_class(&self, u: UnitId, t: ast::TypeId, target: ClassId) -> bool {
        let Some(&ty) = self.query.outline.tipos_escritos.get(&(u, t)) else { return false };
        if matches!(self.query.table.exibicao(ty), Some(Exibicao::Alias { .. })) {
            return false;
        }
        self.interface_class(ty).is_some_and(|(c, _)| c == target)
    }

    /// `searchSubtypes` (`SearchEngineImpl._searchDirectSubtypes` e
    /// `Search.subTypes`): as relações `IS_EXTENDED_BY` (inclusive a
    /// implícita de `Object` numa classe sem `extends`), `IS_MIXED_IN_BY`,
    /// `IS_IMPLEMENTED_BY` e `CONSTRAINS` de `target`, nos arquivos que citam o
    /// nome (num nome privado, só os da biblioteca) mais os da biblioteca de
    /// `target`; uma por relação, na ordem dos arquivos e das cláusulas. A
    /// declaração `augment` fica de fora.
    fn index_subtypes(&self, target: ClassId) -> Vec<ClassId> {
        let p = self.program();
        let Some(d) = p.class(target).decl else { return Vec::new() };
        let name = self.name(p.class(target).name);
        let lib = p.library(p.class(target).library);
        let common = |u: UnitId| p.unit(u).role != UnitRole::Patch;
        let mut files: Vec<UnitId> = Vec::new();
        if name.starts_with('_') {
            files.extend(lib.units.iter().copied().filter(|&u| common(u) && (u == d.unit || cites_name(&p.unit(u).source, name))));
        } else {
            files.extend((0..p.units.len()).map(|i| UnitId(i as u32)).filter(|&u| common(u) && cites_name(&p.unit(u).source, name)));
            for &u in &lib.units {
                if common(u) && !files.contains(&u) {
                    files.push(u);
                }
            }
        }
        let object = Some(target) == self.query.core.object_class;
        let by_declaration: HashMap<(UnitId, ast::DeclId), ClassId> =
            (0..p.classes.len()).map(|i| ClassId(i as u32)).filter_map(|c| p.class(c).decl.map(|d| ((d.unit, d.decl), c))).collect();
        let mut output = Vec::new();
        for u in files {
            let unit = p.unit(u);
            for &did in &unit.unit.declarations {
                let decl = unit.ast.decl(did);
                let (implicit, clausulas): (bool, Vec<ast::TypeId>) = match &decl.kind {
                    DeclKind::Class(k) if k.mixin_application => (false, k.extends.iter().chain(k.with.iter()).chain(k.implements.iter()).copied().collect()),
                    DeclKind::Class(k) => (k.extends.is_none(), k.extends.iter().chain(k.with.iter()).chain(k.implements.iter()).copied().collect()),
                    DeclKind::Mixin(k) => (false, k.on.iter().chain(k.implements.iter()).copied().collect()),
                    DeclKind::Enum(k) => (false, k.with.iter().chain(k.implements.iter()).copied().collect()),
                    DeclKind::ExtensionType(k) => (false, k.implements.to_vec()),
                    _ => continue,
                };
                let Some(&class) = by_declaration.get(&(u, did)) else { continue };
                if decl.augment {
                    continue;
                }
                if implicit && object && class != target {
                    output.push(class);
                }
                for t in clausulas {
                    if self.names_a_class(u, t, target) {
                        output.push(class);
                    }
                }
            }
        }
        output
    }
}

/// O `referencedNames` do arquivo contém `name`: um identificador do texto
/// (sem comentários) igual a ele.
fn cites_name(source: &str, name: &str) -> bool {
    if !source.contains(name) {
        return false;
    }
    match dartforge_frontend::lexer::lex(source) {
        Ok(tokens) => tokens.iter().any(|t| matches!(t.kind, Kind::Ident) && &source[t.span.start..t.span.end] == name),
        Err(_) => true,
    }
}

/// `ElementLocationImpl._decode`: os componentes separados por `;`, com
/// `;;` como um `;` literal.
fn decode_reference(text: &str) -> Vec<String> {
    let mut components = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == ';' {
            if chars.peek() == Some(&';') {
                current.push(';');
                chars.next();
            } else {
                components.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    components.push(current);
    components
}

/// A espécie do elemento-pivô da busca de implementações (`pivotKind`).
#[derive(Clone, Copy)]
enum PivotKind {
    Method,
    Getter,
    Setter,
    Field { final_: bool },
}
