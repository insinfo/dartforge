//! Destaques no documento, implementações, definição do tipo e hierarquia
//! de tipos, sobre o mesmo modelo de identidade de `crate::projeto` (o que
//! a posição denota vem de [`Projeto::identificar`]).
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
enum ElementoDoSuper {
    Construtor(dartforge_elements::model::FunctionElementId),
    Classe(ClassId),
    /// O membro herdado de nome `nome` (o de setter com `=`) na `classe`.
    Outro { nome: String, setter: bool, classe: ClassId },
}
use crate::projeto::{Alvo, Concreto, Dono, Projeto};
use crate::refatoracoes::Contexto;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind, LibraryId, UnitId, UnitRole};
use dartforge_frontend::ast::{self, DeclKind};
use dartforge_frontend::token::Kind;
use dartforge_types::table::Exibicao;
use dartforge_types::{Type, TypeId, TypeParamId};
use std::collections::{BTreeSet, HashMap};

impl Projeto {
    /// Os destaques de `offset` em `unidade` (`DartUnitOccurrencesComputer`,
    /// `destaques.rs`): lista vazia quando nada cobre o cursor.
    pub(crate) fn destaques(&self, unidade: UnitId, offset: usize) -> Option<Vec<Span>> {
        Some(self.destaques_do_dart(unidade, offset))
    }

    /// As classes que têm `c` como supertipo (transitivo), com declaração.
    pub(crate) fn subtipos(&self, c: ClassId, diretos: bool) -> Vec<ClassId> {
        let p = self.programa();
        (0..p.classes.len())
            .map(|i| ClassId(i as u32))
            .filter(|x| *x != c && p.class(*x).decl.is_some() && p.class(*x).kind != ClassKind::MixinApplication)
            .filter(|x| {
                if diretos {
                    let cl = p.class(*x);
                    cl.supertype_class.iter().chain(&cl.mixin_classes).chain(&cl.interface_classes).chain(&cl.on_classes).any(|s| {
                        *s == c || (p.class(*s).kind == ClassKind::MixinApplication && self.supertipos(*s).contains(&c))
                    })
                } else {
                    self.supertipos(*x).contains(&c)
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
    pub(crate) fn superior(&mut self, unidade: UnitId, offset: usize) -> Option<(UnitId, Span)> {
        match self.elemento_do_super(unidade, offset)? {
            ElementoDoSuper::Construtor(f) => {
                let alvo = self.construtor_super(f)?;
                self.nome_da_funcao(alvo)
            }
            ElementoDoSuper::Classe(c) => {
                let s = self.superclasse_nao_sintetica(c)?;
                self.nome_do_elemento_de_topo(Element::Class(s))
            }
            ElementoDoSuper::Outro { nome, setter, classe } => {
                let membro = self.membro_herdado(classe, &nome, setter)?;
                self.nome_da_funcao(membro)
            }
        }
    }

    /// O elemento de `offset` para o `super`: o que o nome ali denota, ou o
    /// da declaração que o envolve (o `getElementOfNode` subindo pelos nós).
    fn elemento_do_super(&self, unidade: UnitId, offset: usize) -> Option<ElementoDoSuper> {
        let p = self.programa();
        let eh_construtor = |f: dartforge_elements::model::FunctionElementId| matches!(p.function(f).kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor);
        if let Ok(Some(d)) = self.identificar(unidade, offset) {
            return match (&d.alvo, d.concreto) {
                (_, Some(Concreto::Funcao(f))) if eh_construtor(f) => Some(ElementoDoSuper::Construtor(f)),
                (Alvo::Construtor(f), _) => Some(ElementoDoSuper::Construtor(*f)),
                (Alvo::Topo(Element::Class(c)), _) => {
                    // A mixin não tem `supertype` (só as restrições `on`).
                    (p.class(*c).kind != ClassKind::Mixin).then_some(ElementoDoSuper::Classe(*c))
                }
                (Alvo::Membro { dono: Dono::Classe(c), nome, .. }, concreto) => {
                    let setter = matches!(concreto, Some(Concreto::Funcao(f)) if self.eh_setter(f));
                    Some(ElementoDoSuper::Outro { nome: nome.clone(), setter, classe: *c })
                }
                // Um local ou um parâmetro de tipo: o nome dele na classe que
                // o envolve (`thisOrAncestorOfType<InterfaceElement>`).
                (Alvo::Local { .. } | Alvo::ParametroDeTipo { .. }, _) => {
                    let classe = self.classe_que_envolve(unidade, offset)?;
                    let fonte = &p.unit(unidade).source;
                    Some(ElementoDoSuper::Outro { nome: fonte[d.nome.start..d.nome.end].to_string(), setter: false, classe })
                }
                _ => None,
            };
        }
        self.declaracao_que_envolve(unidade, offset)
    }

    fn eh_setter(&self, f: dartforge_elements::model::FunctionElementId) -> bool {
        let p = self.programa();
        let e = p.function(f);
        match e.kind {
            FunctionKind::Setter => true,
            FunctionKind::ImplicitAccessor => e.variable.is_some_and(|v| p.variable(v).setter == Some(f)),
            _ => false,
        }
    }

    /// A classe (de interface: classe, mixin, enum, extension type) cuja
    /// declaração contém `offset`.
    fn classe_que_envolve(&self, unidade: UnitId, offset: usize) -> Option<ClassId> {
        let p = self.programa();
        let ast = &p.unit(unidade).ast;
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
            .find(|c| p.class(*c).decl.is_some_and(|d| d.unit == unidade && d.decl.0 as usize == decl))
    }

    /// Sem nome sob o cursor: a função (local, ou de expressão, que não tem
    /// nome) mais interna que contém `offset`, o membro, ou a classe.
    fn declaracao_que_envolve(&self, unidade: UnitId, offset: usize) -> Option<ElementoDoSuper> {
        use dartforge_elements::model::FunctionRef;
        use dartforge_frontend::ast::MemberKind;
        let p = self.programa();
        let ast = &p.unit(unidade).ast;
        let contem = |s: Span| s.start <= offset && offset <= s.end;
        // O membro que contém o offset.
        let membro = ast.members.iter().enumerate().filter(|(_, m)| contem(m.span)).min_by_key(|(_, m)| m.span.end - m.span.start);
        // A função mais interna (local ou de expressão) dentro do membro.
        let funcao_do_membro = membro.and_then(|(_, m)| match m.kind {
            MemberKind::Method(f) => Some(f),
            _ => None,
        });
        let interna = ast
            .functions
            .iter()
            .enumerate()
            .filter(|(i, f)| contem(f.span) && Some(dartforge_frontend::ast::FunctionId(*i as u32)) != funcao_do_membro)
            .filter(|(i, _)| !ast.decls.iter().any(|d| matches!(d.kind, dartforge_frontend::ast::DeclKind::Function(x) if x.0 as usize == *i)))
            .min_by_key(|(_, f)| f.span.end - f.span.start);
        if let Some((_, f)) = interna {
            // A função de expressão tem nome vazio: nenhum membro herdado.
            let nome = f.name?;
            let classe = self.classe_que_envolve(unidade, offset)?;
            return Some(ElementoDoSuper::Outro { nome: self.nome(nome.sym).to_string(), setter: false, classe });
        }
        if let Some((i, m)) = membro {
            let classe = self.classe_que_envolve(unidade, offset);
            return match &m.kind {
                MemberKind::Constructor(_) => {
                    let f = (0..p.functions.len())
                        .map(|k| dartforge_elements::model::FunctionElementId(k as u32))
                        .find(|f| matches!(p.function(*f).node, FunctionRef::Constructor { unit, member } if unit == unidade && member.0 as usize == i))?;
                    Some(ElementoDoSuper::Construtor(f))
                }
                MemberKind::Method(fid) => {
                    let f = ast.function(*fid);
                    let setter = f.kind == dartforge_frontend::ast::FunctionKind::Setter;
                    Some(ElementoDoSuper::Outro { nome: self.nome(f.name?.sym).to_string(), setter, classe: classe? })
                }
                MemberKind::Field(l) => {
                    let v = l.variables.iter().find(|v| v.initializer.is_some_and(|e| contem(ast.expr(e).span)) || contem(v.name.span)).or_else(|| l.variables.first())?;
                    Some(ElementoDoSuper::Outro { nome: self.nome(v.name.sym).to_string(), setter: false, classe: classe? })
                }
            };
        }
        let classe = self.classe_que_envolve(unidade, offset)?;
        (p.class(classe).kind != ClassKind::Mixin).then_some(ElementoDoSuper::Classe(classe))
    }

    /// A superclasse de `c`, passando pelas aplicações de mixin sintéticas
    /// (o analyzer não as tem: `supertype` de `C extends S with M` é `S`).
    pub(crate) fn superclasse_nao_sintetica(&self, c: ClassId) -> Option<ClassId> {
        let p = self.programa();
        let mut s = p.class(c).supertype_class?;
        let mut passos = 0;
        while p.class(s).decl.is_none() && p.class(s).kind == ClassKind::MixinApplication && passos < 64 {
            s = p.class(s).supertype_class?;
            passos += 1;
        }
        Some(s)
    }

    /// `ConstructorElement.superConstructor`: o construtor da superclasse
    /// que o construtor generativo `f` chama (o do `super(…)`/`super.nome(…)`
    /// escrito, ou o sem nome); nenhum numa fábrica ou num redirecionador.
    pub(crate) fn construtor_super(&self, f: dartforge_elements::model::FunctionElementId) -> Option<dartforge_elements::model::FunctionElementId> {
        use dartforge_elements::model::FunctionRef;
        use dartforge_frontend::ast::{Initializer, MemberKind};
        let p = self.programa();
        let fe = p.function(f);
        if fe.factory {
            return None;
        }
        let classe = fe.class?;
        let nome = match fe.node {
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
        let s = self.superclasse_nao_sintetica(classe)?;
        let chave = match nome {
            Some(n) => n,
            None => self.consulta.nomes.lookup("")?,
        };
        p.class(s).constructors.get(&chave).copied()
    }

    /// `InheritanceManager3.getInherited2(classe, Name(biblioteca da classe,
    /// nome))`.
    fn membro_herdado(&mut self, classe: ClassId, nome: &str, setter: bool) -> Option<dartforge_elements::model::FunctionElementId> {
        let crate::consulta::Consulta { programa, nomes, core, outline, tabela, .. } = &mut self.consulta;
        let simbolo = nomes.lookup(nome)?;
        let mut n = dartforge_types::heranca::Nome::novo(nomes, programa.class(classe).library, simbolo);
        if setter {
            n = n.de_setter(nomes)?;
        }
        let mut provedor = dartforge_types::heranca::ProvedorDoOutline { program: programa, interner: nomes, core, outline, table: tabela };
        let mut heranca = dartforge_types::heranca::Heranca::default();
        heranca.herdado(&mut provedor, classe, n).map(|m| m.funcao)
    }

    /// As implementações do que `offset` denota: subtipos de uma classe, ou
    /// o membro declarado em cada subtipo.
    pub(crate) fn implementacoes(&self, unidade: UnitId, offset: usize) -> Vec<(UnitId, Span)> {
        let Ok(Some(d)) = self.identificar(unidade, offset) else { return Vec::new() };
        let p = self.programa();
        let (classe, membro) = match (&d.alvo, d.concreto) {
            (Alvo::Topo(Element::Class(c)), _) => (*c, None),
            (Alvo::Construtor(f), _) => match p.function(*f).class {
                Some(c) => (c, None),
                None => return Vec::new(),
            },
            (Alvo::Membro { dono: Dono::Classe(c), nome, estatico }, concreto) => {
                // Construtor chamado por `A.nome()` resolve para a classe.
                if let Some(Concreto::Funcao(f)) = concreto
                    && matches!(p.function(f).kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor)
                {
                    (*c, None)
                } else {
                    (*c, Some((nome.clone(), *estatico)))
                }
            }
            _ => return Vec::new(),
        };
        // Membros de extension type não sobrescrevem: só redeclaram.
        let eh_extension_type = |c: ClassId| {
            p.class(c).decl.is_some_and(|dr| matches!(p.unit(dr.unit).ast.decl(dr.decl).kind, dartforge_frontend::ast::DeclKind::ExtensionType(_)))
        };
        let mut vistos = BTreeSet::new();
        let mut saida = Vec::new();
        for x in self.subtipos(classe, false) {
            let local = match &membro {
                None => self.nome_do_elemento_de_topo(Element::Class(x)),
                Some(_) if eh_extension_type(classe) || eh_extension_type(x) => None,
                Some((nome, estatico)) => {
                    let cl = p.class(x);
                    // No próprio subtipo; senão no último mixin que o declara.
                    let funcoes = self.declarados(x, nome, *estatico);
                    let campo = cl.fields.iter().copied().find(|v| self.nome(p.variable(*v).name) == nome && !p.variable(*v).static_);
                    match (campo, funcoes.first()) {
                        (Some(v), _) => self.nome_da_variavel(v),
                        (None, Some(f)) => self.nome_da_funcao(*f),
                        (None, None) => cl
                            .mixin_classes
                            .iter()
                            .rev()
                            .find_map(|m| self.declarados(*m, nome, *estatico).first().and_then(|f| self.nome_da_funcao(*f))),
                    }
                }
            };
            if let Some((u, s)) = local
                && vistos.insert((u, s.start))
            {
                saida.push((u, s));
            }
        }
        saida
    }

    /// A declaração do tipo estático do que `offset` denota.
    pub(crate) fn definicao_de_tipo(&self, unidade: UnitId, offset: usize) -> Option<(UnitId, Span)> {
        let p = self.programa();
        let ast = &p.unit(unidade).ast;
        let dentro = |s: Span| s.start <= offset && offset <= s.end;
        // O nome de uma anotação: o `SimpleIdentifier` da classe dá o
        // `thisType`; o de uma variável constante (`@override`), o prefixo e
        // o nome do construtor não têm tipo estático.
        for a in crate::projeto::metadados(ast, &p.unit(unidade).unit) {
            let Some(i) = a.name.iter().position(|n| dentro(n.span)) else { continue };
            let lib = p.unit(unidade).library;
            let prefixado = a.name.len() >= 2 && crate::projeto::eh_prefixo(p, lib, a.name[0].sym);
            let (indice_da_classe, binding) = if prefixado {
                (1, p.lookup_prefixed_na_unidade(unidade, a.name[0].sym, a.name[1].sym))
            } else {
                (0, p.lookup_na_unidade(unidade, a.name[0].sym))
            };
            return match binding.and_then(|b| b.getter) {
                Some(Element::Class(c)) if i == indice_da_classe => self.nome_do_elemento_de_topo(Element::Class(c)),
                _ => None,
            };
        }
        // O `returnType` da declaração de construtor (`A(…)`, o `A` de
        // `A.n(…)`) é a classe; o nome do construtor (`n`) não é nó com tipo;
        // o campo de um inicializador (`x = …`) também não.
        for (i, m) in ast.members.iter().enumerate() {
            let ast::MemberKind::Constructor(k) = &m.kind else { continue };
            if dentro(k.class_name.span) {
                let f = self.construtor_do_no(unidade, ast::MemberId(i as u32))?;
                return self.nome_do_elemento_de_topo(Element::Class(p.function(f).class?));
            }
            if k.initializers.iter().any(|x| matches!(x, ast::Initializer::Field { name, .. } if dentro(name.span))) {
                return None;
            }
        }
        let d = self.identificar(unidade, offset).ok()??;
        // O nome de uma declaração de classe, função, método ou constante
        // de enum não é um nó com tipo para o analyzer.
        let declarado = match (&d.alvo, d.concreto) {
            (Alvo::Topo(Element::Class(c)), None) => self.nome_do_elemento_de_topo(Element::Class(*c)),
            (_, Some(Concreto::Funcao(f))) => self.nome_da_funcao(f),
            (_, Some(Concreto::Variavel(v)))
                if self.programa().variable(v).class.is_some_and(|c| self.programa().class(c).enum_constants.contains(&v)) =>
            {
                self.nome_da_variavel(v)
            }
            _ => None,
        };
        if d.expr.is_none() && declarado.is_some_and(|(u, s)| u == unidade && s == d.nome) {
            return None;
        }
        let corpos = &self.consulta.corpos.units[unidade.0 as usize];
        let tipo = match (&d.alvo, d.concreto) {
            // Nome de classe (tipo, referência, construtor sem nome).
            (Alvo::Topo(Element::Class(c)), _) => return self.nome_do_elemento_de_topo(Element::Class(*c)),
            // Construtor nomeado: o analyzer olha o nome (expressão sem tipo
            // de interface).
            (Alvo::Construtor(_), _) => return None,
            (_, Some(Concreto::Funcao(f)))
                if !matches!(self.programa().function(f).kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) =>
            {
                // Método ou função: o tipo é uma função.
                return None;
            }
            _ => match d.expr {
                // `inSetterContext`: o tipo da variável do `writeElement`.
                Some(e) if self.alvo_de_escrita(unidade, e) && !matches!(d.alvo, Alvo::Local { .. }) => match d.concreto {
                    Some(Concreto::Variavel(v)) => self.consulta.tipo_da_variavel(v)?,
                    Some(Concreto::Funcao(f)) => match (self.programa().function(f).kind, self.programa().function(f).variable) {
                        (FunctionKind::ImplicitAccessor, Some(v)) => self.consulta.tipo_da_variavel(v)?,
                        (FunctionKind::Setter, _) => self.consulta.outline.functions[f.0 as usize].parameters.first()?.ty,
                        _ => corpos.get_type(e)?,
                    },
                    None => corpos.get_type(e)?,
                },
                Some(e) => corpos.get_type(e)?,
                None => match (&d.alvo, d.concreto) {
                    (Alvo::Local { unidade: u, declaracao }, _) => self.consulta.corpos.units[u.0 as usize].tipo_local(*declaracao)?,
                    (_, Some(Concreto::Variavel(v))) => self.consulta.tipo_da_variavel(v)?,
                    (_, Some(Concreto::Funcao(f))) => self.consulta.outline.functions[f.0 as usize].return_type,
                    _ => return None,
                },
            },
        };
        let classe = match self.consulta.tabela.get(tipo) {
            Type::Interface { class, .. } => *class,
            Type::ExtensionType { decl, .. } => *decl,
            _ => return None,
        };
        self.nome_do_elemento_de_topo(Element::Class(classe))
    }

    /// `e` é escrito: o alvo de uma atribuição ou de `++`/`--`.
    fn alvo_de_escrita(&self, unidade: UnitId, e: dartforge_frontend::ast::ExprId) -> bool {
        use dartforge_frontend::ast::{ExprKind, UnaryOp};
        self.programa().unit(unidade).ast.exprs.iter().any(|x| match &x.kind {
            ExprKind::Assign { target, .. } => *target == e,
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => *operand == e,
            _ => false,
        })
    }

    // -- Hierarquia de tipos (`DartLazyTypeHierarchyComputer`) ---------------

    /// A classe e os argumentos de um tipo que o analyzer tem como
    /// `InterfaceType`: classe, mixin, enum, extension type, `FutureOr` e
    /// `Null`; nada no `InvalidType`, no `Never?` e nos outros tipos.
    pub(crate) fn classe_de_interface(&self, t: TypeId) -> Option<(ClassId, Vec<TypeId>)> {
        let tabela = &self.consulta.tabela;
        if matches!(tabela.exibicao(t), Some(Exibicao::Invalido | Exibicao::NeverAnulavel)) {
            return None;
        }
        match tabela.get(t) {
            Type::Interface { class, args, .. } => Some((*class, args.to_vec())),
            Type::ExtensionType { decl, args, .. } => Some((*decl, args.to_vec())),
            Type::FutureOr { arg, .. } => Some((self.classe_do_sdk(self.consulta.core.async_library?, "FutureOr")?, vec![*arg])),
            Type::Null => Some((self.consulta.core.null_class?, Vec::new())),
            _ => None,
        }
    }

    /// A classe `nome` declarada na biblioteca `lib`.
    fn classe_do_sdk(&self, lib: LibraryId, nome: &str) -> Option<ClassId> {
        let simbolo = self.consulta.nomes.lookup(nome)?;
        match self.programa().library(lib).declared.get(&simbolo)?.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        }
    }

    /// O `thisType` de `c`: os argumentos são os próprios parâmetros de tipo.
    fn tipo_this(&mut self, c: ClassId) -> (TypeId, Vec<TypeId>) {
        let parametros: Vec<TypeParamId> = self.consulta.outline.classes.get(c.0 as usize).map(|d| d.type_params.to_vec()).unwrap_or_default();
        let extensao = self.programa().class(c).kind == ClassKind::ExtensionType;
        let tabela = &mut self.consulta.tabela;
        let args: Vec<TypeId> = parametros.iter().map(|&param| tabela.intern(Type::TypeParameter { param, nullable: false })).collect();
        let tipo = if extensao {
            tabela.intern(Type::ExtensionType { decl: c, args: args.clone().into_boxed_slice(), nullable: false })
        } else {
            tabela.intern(Type::Interface { class: c, args: args.clone().into_boxed_slice(), nullable: false })
        };
        (tipo, args)
    }

    /// `_isInterfaceTypeInterface`: um tipo de interface não anulável que não
    /// é enum, extension type, `Function` nem `Null`.
    fn interface_valida(&self, t: TypeId) -> bool {
        let Some((c, _)) = self.classe_de_interface(t) else { return false };
        let core = &self.consulta.core;
        !matches!(self.programa().class(c).kind, ClassKind::Enum | ClassKind::ExtensionType)
            && Some(c) != core.function_class
            && Some(c) != core.null_class
            && !self.consulta.tabela.get(t).is_declared_nullable()
    }

    /// `_isInterfaceTypeClass`: a interface válida cujo elemento é classe.
    fn classe_valida(&self, t: TypeId) -> bool {
        self.interface_valida(t)
            && self.classe_de_interface(t).is_some_and(|(c, _)| matches!(self.programa().class(c).kind, ClassKind::Class | ClassKind::MixinApplication))
    }

    /// `isValidExtensionTypeSuperinterface`.
    fn superinterface_de_extension_type(&self, t: TypeId) -> bool {
        let Some((c, _)) = self.classe_de_interface(t) else { return false };
        let core = &self.consulta.core;
        let future_or = core.async_library.and_then(|l| self.classe_do_sdk(l, "FutureOr"));
        !self.consulta.tabela.get(t).is_declared_nullable()
            && Some(c) != future_or
            && Some(c) != core.function_class
            && Some(c) != core.null_class
            && Some(c) != core.record_class
    }

    /// `_getSupertypes` de `c<args>`: a superclasse (`Object` sem `extends`
    /// válido; `Enum` num enum; nenhuma em `Object`, mixin e extension type),
    /// as restrições `on` (`[Object]` num mixin sem elas), as interfaces e os
    /// mixins, com os parâmetros de `c` substituídos por `args`.
    fn supertipos_do_tipo(&mut self, c: ClassId, args: &[TypeId]) -> Vec<TypeId> {
        let Some(dados) = self.consulta.outline.classes.get(c.0 as usize).cloned() else { return Vec::new() };
        let especie = self.programa().class(c).kind;
        let core = &self.consulta.core;
        let objeto = core.object;
        let eh_objeto = Some(c) == core.object_class;
        let enum_ = core.core_library.and_then(|l| self.classe_do_sdk(l, "Enum"));
        let mut lista: Vec<TypeId> = Vec::new();
        match especie {
            ClassKind::Class | ClassKind::MixinApplication => match dados.supertype.filter(|&s| self.classe_valida(s)) {
                Some(s) => lista.push(s),
                None if !eh_objeto => lista.push(objeto),
                None => {}
            },
            ClassKind::Enum => match enum_ {
                Some(e) => lista.push(self.consulta.tabela.intern(Type::Interface { class: e, args: Box::new([]), nullable: false })),
                None => lista.push(objeto),
            },
            ClassKind::Mixin | ClassKind::ExtensionType => {}
        }
        if especie == ClassKind::Mixin {
            let restricoes: Vec<TypeId> = dados.on.iter().copied().filter(|&t| self.interface_valida(t)).collect();
            if restricoes.is_empty() {
                lista.push(objeto);
            } else {
                lista.extend(restricoes);
            }
        }
        if especie == ClassKind::ExtensionType {
            lista.extend(dados.interfaces.iter().copied().filter(|&t| self.superinterface_de_extension_type(t)));
        } else {
            lista.extend(dados.interfaces.iter().copied().filter(|&t| self.interface_valida(t)));
        }
        lista.extend(dados.mixins.iter().copied().filter(|&t| self.interface_valida(t)));
        if dados.type_params.is_empty() {
            return lista;
        }
        let mapa: HashMap<TypeParamId, TypeId> = dados.type_params.iter().copied().zip(args.iter().copied()).collect();
        lista.into_iter().map(|t| dartforge_types::substitute(t, &mapa, &mut self.consulta.tabela)).collect()
    }

    /// A URI do `Source` de uma unidade: a da biblioteca na unidade que a
    /// define; numa parte de biblioteca `dart:`, `dart:core/list.dart`; senão
    /// a URI canônica da unidade.
    fn uri_de_origem(&self, u: UnitId) -> String {
        let p = self.programa();
        let unidade = p.unit(u);
        let lib = p.library(unidade.library);
        if lib.units.first() == Some(&u) {
            return lib.uri.clone();
        }
        if lib.uri.starts_with("dart:")
            && let (Some(caminho), Some(base)) = (unidade.path.as_deref(), lib.units.first().and_then(|f| p.unit(*f).path.as_deref()).and_then(|x| x.parent()))
            && let Ok(relativo) = caminho.strip_prefix(base)
        {
            return format!("{}/{}", lib.uri, relativo.to_string_lossy().replace('\\', "/"));
        }
        unidade.uri.clone()
    }

    /// O `ElementLocation.encoding` de uma classe: a URI da biblioteca, a da
    /// unidade e o nome, separados por `;` (o `;` de um componente dobrado).
    pub(crate) fn referencia_da_classe(&self, c: ClassId) -> Option<String> {
        let p = self.programa();
        let classe = p.class(c);
        let d = classe.decl?;
        let componentes = [p.library(classe.library).uri.clone(), self.uri_de_origem(d.unit), self.nome(classe.name).to_string()];
        Some(componentes.iter().map(|s| s.replace(';', ";;")).collect::<Vec<_>>().join(";"))
    }

    /// `session.locateElement(ElementLocationImpl.con2(referencia))` restrito
    /// a classes: a biblioteca, a unidade e o nome; a declaração `augment`
    /// não é o elemento.
    pub(crate) fn classe_da_referencia(&self, referencia: &str) -> Option<ClassId> {
        let componentes = decodificar_referencia(referencia);
        let [lib, unidade, nome] = componentes.as_slice() else { return None };
        let p = self.programa();
        (0..p.classes.len()).map(|i| ClassId(i as u32)).find(|&k| {
            let classe = p.class(k);
            classe.decl.is_some_and(|d| {
                !p.unit(d.unit).ast.decl(d.decl).augment
                    && self.nome(classe.name) == nome
                    && p.library(classe.library).uri == *lib
                    && self.uri_de_origem(d.unit) == *unidade
            })
        })
    }

    /// A classe declarada por `did` em `u`.
    fn classe_da_declaracao(&self, u: UnitId, did: ast::DeclId) -> Option<ClassId> {
        let p = self.programa();
        (0..p.classes.len()).map(|i| ClassId(i as u32)).find(|&c| p.class(c).decl.is_some_and(|d| d.unit == u && d.decl == did))
    }

    /// O `NamedType.type` do nó `k`: o tipo construído numa criação de
    /// instância, senão o da anotação de tipo com o mesmo intervalo.
    fn tipo_do_tipo_nomeado(&self, cx: &Contexto<'_>, k: usize) -> Option<TypeId> {
        if let Some(pai) = cx.pai(k)
            && cx.especie(pai) == "ConstructorName"
            && let Some(avo) = cx.pai(pai)
            && cx.especie(avo) == "InstanceCreationExpression"
        {
            return cx.tipo_do_no(avo);
        }
        let s = cx.arvore.span(k);
        cx.ast
            .types
            .iter()
            .enumerate()
            .filter(|(_, t)| t.span == s && matches!(t.kind, ast::TypeKind::Named { .. } | ast::TypeKind::Void))
            .find_map(|(i, _)| {
                let id = ast::TypeId(i as u32);
                cx.corpos.tipos_de_anotacoes.get(&id).or_else(|| self.consulta.outline.tipos_escritos.get(&(cx.unidade, id))).copied()
            })
    }

    /// `findTarget`: o tipo do `NamedType` que contém o nó que cobre o
    /// cursor (`nodeCovering`); sem ele, o `thisType` da classe, mixin, enum
    /// ou extension type que o contém. Só tipos de interface.
    pub(crate) fn alvo_da_hierarquia(&mut self, unidade: UnitId, offset: usize) -> Option<(ClassId, TypeId)> {
        let (escrito, declarada) = {
            let cx = Contexto::novo(self, unidade);
            let n = cx.cobertura(offset, 0)?;
            let cadeia = cx.arvore.cadeia(n);
            match cadeia.iter().copied().find(|&k| cx.especie(k) == "NamedType") {
                Some(k) => (Some(self.tipo_do_tipo_nomeado(&cx, k)?), None),
                None => {
                    let d = cadeia
                        .iter()
                        .copied()
                        .find(|&k| matches!(cx.especie(k), "ClassDeclaration" | "MixinDeclaration" | "ExtensionTypeDeclaration" | "EnumDeclaration"))?;
                    let Marca::Decl(did) = cx.arvore.nos[d].marca else { return None };
                    (None, Some(self.classe_da_declaracao(unidade, did)?))
                }
            }
        };
        let tipo = match (escrito, declarada) {
            (Some(t), _) => t,
            (None, Some(c)) => self.tipo_this(c).0,
            (None, None) => return None,
        };
        let (c, _) = self.classe_de_interface(tipo)?;
        Some((c, tipo))
    }

    /// `TypeHierarchyItem.forType`: o `getDisplayString()` do tipo, a
    /// referência da classe, o arquivo, o código da declaração (com a
    /// documentação e as anotações) e o nome.
    pub(crate) fn item_de_tipo(&self, c: ClassId, tipo: TypeId) -> Option<crate::ItemDeTipo> {
        self.itens_de_tipo(&[(c, tipo)]).pop()
    }

    /// Os itens de vários tipos, com uma árvore por arquivo.
    fn itens_de_tipo(&self, tipos: &[(ClassId, TypeId)]) -> Vec<crate::ItemDeTipo> {
        let p = self.programa();
        let mut arvores: HashMap<UnitId, Contexto<'_>> = HashMap::new();
        let mut saida = Vec::new();
        for &(c, tipo) in tipos {
            let Some(d) = p.class(c).decl else { continue };
            let Some((u, selecao)) = self.nome_do_elemento_de_topo(Element::Class(c)) else { continue };
            let cx = arvores.entry(d.unit).or_insert_with(|| Contexto::novo(self, d.unit));
            let Some(no) = cx.arvore.nos.iter().position(|n| matches!(n.marca, Marca::Decl(x) if x == d.decl)) else { continue };
            let intervalo = cx.arvore.span(no);
            let (Some(uri), Some(referencia)) = (self.uri_da_unidade(u), self.referencia_da_classe(c)) else { continue };
            saida.push(crate::ItemDeTipo {
                nome: self.consulta.tabela.format_sem_alias(tipo, &self.consulta.nomes, &self.consulta.programa),
                uri,
                intervalo,
                selecao,
                referencia,
                ancora: None,
            });
        }
        saida
    }

    /// `_locateTargetFromAnchor`: do `thisType` da âncora, segue os índices
    /// pelos supertipos; o tipo encontrado vale se o elemento é `alvo`.
    fn tipo_pela_ancora(&mut self, referencia: &str, caminho: &[usize], alvo: ClassId) -> Option<(ClassId, Vec<TypeId>)> {
        let raiz = self.classe_da_referencia(referencia)?;
        let mut atual = Some((raiz, self.tipo_this(raiz).1));
        for &i in caminho {
            let Some((c, args)) = atual else { break };
            let supertipos = self.supertipos_do_tipo(c, &args);
            atual = supertipos.get(i).and_then(|t| self.classe_de_interface(*t));
        }
        atual.filter(|(c, _)| *c == alvo)
    }

    /// `findSupertypes` (`typeHierarchy/supertypes`): `None` quando a
    /// referência não localiza uma classe. Os supertipos com argumentos de
    /// tipo levam a âncora (a do pedido, ou a do próprio item com caminho
    /// vazio) com o índice acrescentado.
    pub(crate) fn supertipos_da_referencia(&mut self, referencia: &str, ancora: Option<(&str, &[usize])>) -> Option<Vec<crate::ItemDeTipo>> {
        let c = self.classe_da_referencia(referencia)?;
        let mut alvo = (c, self.tipo_this(c).1);
        let (ref_da_ancora, caminho) = match ancora {
            Some((r, caminho)) => {
                if let Some(t) = self.tipo_pela_ancora(r, caminho, c) {
                    alvo = t;
                }
                (r.to_string(), caminho.to_vec())
            }
            None => (referencia.to_string(), Vec::new()),
        };
        let supertipos = self.supertipos_do_tipo(alvo.0, &alvo.1);
        let mut saida = Vec::new();
        for (i, t) in supertipos.into_iter().enumerate() {
            let Some((k, args)) = self.classe_de_interface(t) else { continue };
            let Some(mut item) = self.item_de_tipo(k, t) else { continue };
            if !args.is_empty() {
                let mut novo = caminho.clone();
                novo.push(i);
                item.ancora = Some((ref_da_ancora.clone(), novo));
            }
            saida.push(item);
        }
        Some(saida)
    }

    /// `findSubtypes` (`typeHierarchy/subtypes`): `None` quando a referência
    /// não localiza uma classe; cada subtipo direto pelo seu `thisType`.
    pub(crate) fn subtipos_da_referencia(&mut self, referencia: &str) -> Option<Vec<crate::ItemDeTipo>> {
        let c = self.classe_da_referencia(referencia)?;
        let subtipos = self.subtipos_do_indice(c);
        let tipos: Vec<(ClassId, TypeId)> = subtipos.into_iter().map(|k| (k, self.tipo_this(k).0)).collect();
        Some(self.itens_de_tipo(&tipos))
    }

    /// O tipo escrito `t` de `u` nomeia `alvo` diretamente (o
    /// `NamedType.element` do índice: um alias `typedef` não conta).
    fn nomeia_a_classe(&self, u: UnitId, t: ast::TypeId, alvo: ClassId) -> bool {
        let Some(&tipo) = self.consulta.outline.tipos_escritos.get(&(u, t)) else { return false };
        if matches!(self.consulta.tabela.exibicao(tipo), Some(Exibicao::Alias { .. })) {
            return false;
        }
        self.classe_de_interface(tipo).is_some_and(|(c, _)| c == alvo)
    }

    /// `searchSubtypes` (`SearchEngineImpl._searchDirectSubtypes` e
    /// `Search.subTypes`): as relações `IS_EXTENDED_BY` (inclusive a
    /// implícita de `Object` numa classe sem `extends`), `IS_MIXED_IN_BY`,
    /// `IS_IMPLEMENTED_BY` e `CONSTRAINS` de `alvo`, nos arquivos que citam o
    /// nome (num nome privado, só os da biblioteca) mais os da biblioteca de
    /// `alvo`; uma por relação, na ordem dos arquivos e das cláusulas. A
    /// declaração `augment` fica de fora.
    fn subtipos_do_indice(&self, alvo: ClassId) -> Vec<ClassId> {
        let p = self.programa();
        let Some(d) = p.class(alvo).decl else { return Vec::new() };
        let nome = self.nome(p.class(alvo).name);
        let lib = p.library(p.class(alvo).library);
        let comum = |u: UnitId| p.unit(u).role != UnitRole::Patch;
        let mut arquivos: Vec<UnitId> = Vec::new();
        if nome.starts_with('_') {
            arquivos.extend(lib.units.iter().copied().filter(|&u| comum(u) && (u == d.unit || cita_o_nome(&p.unit(u).source, nome))));
        } else {
            arquivos.extend((0..p.units.len()).map(|i| UnitId(i as u32)).filter(|&u| comum(u) && cita_o_nome(&p.unit(u).source, nome)));
            for &u in &lib.units {
                if comum(u) && !arquivos.contains(&u) {
                    arquivos.push(u);
                }
            }
        }
        let objeto = Some(alvo) == self.consulta.core.object_class;
        let por_declaracao: HashMap<(UnitId, ast::DeclId), ClassId> =
            (0..p.classes.len()).map(|i| ClassId(i as u32)).filter_map(|c| p.class(c).decl.map(|d| ((d.unit, d.decl), c))).collect();
        let mut saida = Vec::new();
        for u in arquivos {
            let unidade = p.unit(u);
            for &did in &unidade.unit.declarations {
                let decl = unidade.ast.decl(did);
                let (implicito, clausulas): (bool, Vec<ast::TypeId>) = match &decl.kind {
                    DeclKind::Class(k) if k.mixin_application => (false, k.extends.iter().chain(k.with.iter()).chain(k.implements.iter()).copied().collect()),
                    DeclKind::Class(k) => (k.extends.is_none(), k.extends.iter().chain(k.with.iter()).chain(k.implements.iter()).copied().collect()),
                    DeclKind::Mixin(k) => (false, k.on.iter().chain(k.implements.iter()).copied().collect()),
                    DeclKind::Enum(k) => (false, k.with.iter().chain(k.implements.iter()).copied().collect()),
                    DeclKind::ExtensionType(k) => (false, k.implements.to_vec()),
                    _ => continue,
                };
                let Some(&classe) = por_declaracao.get(&(u, did)) else { continue };
                if decl.augment {
                    continue;
                }
                if implicito && objeto && classe != alvo {
                    saida.push(classe);
                }
                for t in clausulas {
                    if self.nomeia_a_classe(u, t, alvo) {
                        saida.push(classe);
                    }
                }
            }
        }
        saida
    }
}

/// O `referencedNames` do arquivo contém `nome`: um identificador do texto
/// (sem comentários) igual a ele.
fn cita_o_nome(fonte: &str, nome: &str) -> bool {
    if !fonte.contains(nome) {
        return false;
    }
    match dartforge_frontend::lexer::lex(fonte) {
        Ok(tokens) => tokens.iter().any(|t| matches!(t.kind, Kind::Ident) && &fonte[t.span.start..t.span.end] == nome),
        Err(_) => true,
    }
}

/// `ElementLocationImpl._decode`: os componentes separados por `;`, com
/// `;;` como um `;` literal.
fn decodificar_referencia(texto: &str) -> Vec<String> {
    let mut componentes = Vec::new();
    let mut atual = String::new();
    let mut caracteres = texto.chars().peekable();
    while let Some(ch) = caracteres.next() {
        if ch == ';' {
            if caracteres.peek() == Some(&';') {
                atual.push(';');
                caracteres.next();
            } else {
                componentes.push(std::mem::take(&mut atual));
            }
        } else {
            atual.push(ch);
        }
    }
    componentes.push(atual);
    componentes
}
