//! Cláusulas de herança (`extends`/`with`/`implements`/`on`) das classes,
//! aliases de classe, enums e mixins de uma biblioteca, como o analyzer
//! 6.11.0 as confere em dois lugares:
//!
//! * `InheritanceOverrideVerifier._checkDirectSuperTypes`
//!   (`src/error/inheritance_override.dart`): tipo que não se estende nem
//!   implementa (`subtype_of_disallowed_type`) e `Enum` como superinterface de
//!   classe concreta (`concrete_class_has_enum_superinterface`);
//! * `ErrorVerifier._checkClassInheritance` e `_checkMixinInheritance`
//!   (`src/generated/error_verifier.dart`): uma **porta** — supertipo
//!   proibido, cláusula adiada, erro de mixin ou superclasse só com
//!   `factory` — que, se algo foi relatado, desliga as verificações
//!   seguintes (`class_used_as_mixin`, modificadores fora da biblioteca…).
//!
//! A porta é exposta ([`porta`]) para que os verificadores de modificadores
//! (`modificadores.rs`) sigam a mesma regra. Onde a porta depende de tipos
//! que esta camada não calcula (restrições `on` do mixin aplicado, membros
//! invocados por `super` no mixin), ela responde [`Porta::Incerta`] e nada
//! do que depende dela é emitido.

use dartforge_diagnostics::{Codigo, Diagnostic, Span, codigos::compile_time_error as c};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionKind, LibraryId, Program, UnitId,
};
use dartforge_frontend::ast::{self, DeclKind, TypeKind, TypedefKind};
use dartforge_intern::{Interner, SymbolId};

/// As decisões da camada de tipos sobre os mixins (`MixinElement`) das
/// cláusulas `with` (`dartforge_types::fase_mixins::decisoes`): por
/// `(classe, posição no with)`, `None` sem relato ou o relato
/// (`mixin_application_not_implemented_interface`,
/// `mixin_application_no_concrete_super_invoked_*`,
/// `mixin_application_concrete_super_invoked_member_type`). Sem decisão, a
/// porta fica incerta como antes.
pub type DecisoesDeMixins = std::collections::HashMap<(ClassId, usize), Option<Diagnostic>>;

thread_local! {
    static DECISOES_DE_MIXINS: std::cell::RefCell<Option<std::rc::Rc<DecisoesDeMixins>>> = const { std::cell::RefCell::new(None) };
}

/// Define (ou limpa, com `None`) as decisões dos mixins para as análises
/// seguintes desta thread.
pub fn definir_decisoes_de_mixins(d: Option<DecisoesDeMixins>) {
    DECISOES_DE_MIXINS.with(|c| *c.borrow_mut() = d.map(std::rc::Rc::new));
}

fn decisao_de_mixin(id: ClassId, i: usize) -> Option<Option<Diagnostic>> {
    DECISOES_DE_MIXINS.with(|c| c.borrow().as_ref().and_then(|m| m.get(&(id, i)).cloned()))
}

/// O resultado da porta do `ErrorVerifier` para uma declaração.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Porta {
    /// Nada relatado: as verificações seguintes rodam.
    Aberta,
    /// Algo foi relatado: o analyzer para aqui.
    Fechada,
    /// Depende de tipos que esta camada não calcula; quem depende da porta
    /// não emite nada.
    Incerta,
}

/// O que um tipo escrito numa cláusula nomeia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alvo {
    /// Uma classe, mixin, enum ou tipo de extensão (tipo de interface).
    Classe(ClassId),
    /// Um tipo que não é de interface (função, registro, `void`, `dynamic`).
    NaoInterface,
    /// Não resolvido aqui (nome desconhecido, ambíguo, parâmetro de tipo).
    Desconhecido,
}

/// O elemento de um tipo de cláusula para o `ResolutionVisitor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resolvido {
    /// Classe, mixin, enum ou tipo de extensão.
    Classe(ClassId),
    /// Outro elemento de tipo: parâmetro de tipo, `dynamic`, `Never`, alias
    /// de função, de registro ou de `void`.
    NaoClasse,
    /// Já relatado pela resolução do nome, ou não decidido aqui.
    Ignorar,
    /// Alias que se expande num parâmetro de tipo dele
    /// (`_verifyTypeAliasForContext`): `*_type_alias_expands_to_type_parameter`.
    AliasDeParametro,
}

/// Leitor das cláusulas no escopo de uma biblioteca.
struct Leitor<'a> {
    programa: &'a Program,
    nomes: &'a Interner,
}

impl Leitor<'_> {
    fn nome(&self, s: SymbolId) -> &str {
        self.nomes.resolve(s)
    }

    /// O elemento que `nome` (ou `prefixo.nome`) designa no escopo da
    /// unidade `u` (com `parts-with-imports`, o dela).
    fn elemento(&self, u: UnitId, nome: &[ast::Name]) -> Option<Element> {
        let b = match nome {
            [n] => self.programa.lookup_na_unidade(u, n.sym),
            [p, n] => self.programa.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        }?;
        if b.ambiguous {
            return None;
        }
        b.getter
    }

    /// O alvo do tipo `t` (da árvore `ast`, escrito na unidade `u`),
    /// seguindo aliases de tipo até a classe que eles nomeiam.
    fn alvo(&self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, profundidade: u32) -> Alvo {
        if profundidade > 8 {
            return Alvo::Desconhecido;
        }
        let no = ast_.ty(t);
        let TypeKind::Named { name, .. } = &no.kind else {
            return Alvo::NaoInterface;
        };
        let Some(e) = self.elemento(u, name) else {
            return match name.last().map(|n| self.nome(n.sym)) {
                Some("dynamic") if name.len() == 1 => Alvo::NaoInterface,
                _ => Alvo::Desconhecido,
            };
        };
        match e {
            Element::Class(id) => Alvo::Classe(id),
            Element::Typedef(tid) => {
                let td = self.programa.typedef(tid);
                let ast_td = &self.programa.unit(td.decl.unit).ast;
                let DeclKind::Typedef(d) = &ast_td.decl(td.decl.decl).kind else {
                    return Alvo::Desconhecido;
                };
                let TypedefKind::Alias(corpo) = d.kind else {
                    return Alvo::NaoInterface;
                };
                if let TypeKind::Named { name: n2, .. } = &ast_td.ty(corpo).kind
                    && let [unico] = &n2[..]
                    && d.type_params.iter().any(|p| p.name.sym == unico.sym)
                {
                    // Alias que se expande num parâmetro de tipo: o analyzer
                    // relata `supertype_expands_to_type_parameter` (outro
                    // verificador) e não confere a cláusula.
                    return Alvo::Desconhecido;
                }
                self.alvo(td.decl.unit, ast_td, corpo, profundidade + 1)
            }
            _ => Alvo::Desconhecido,
        }
    }

    /// O tipo escrito como o analyzer o exibe (`DartType.getDisplayString`),
    /// com o nome do alias quando o tipo vem de um `typedef`; `None` onde a
    /// exibição dependeria de tipos que aqui não se calculam.
    fn exibir(
        &self,
        u: UnitId,
        ast_: &ast::Ast,
        t: ast::TypeId,
        params: &[SymbolId],
    ) -> Option<String> {
        let no = ast_.ty(t);
        let base = match &no.kind {
            TypeKind::Void => "void".to_string(),
            TypeKind::Named { name, args } => {
                let ultimo = name.last()?;
                let texto = self.nome(ultimo.sym).to_string();
                let parametros: Vec<bool> = match self.elemento(u, name) {
                    Some(Element::Class(id)) => self
                        .programa
                        .class(id)
                        .type_params
                        .iter()
                        .map(|p| p.bound.is_some())
                        .collect(),
                    Some(Element::Typedef(tid)) => self
                        .programa
                        .typedef(tid)
                        .type_params
                        .iter()
                        .map(|p| p.bound.is_some())
                        .collect(),
                    None if name.len() == 1
                        && (params.contains(&ultimo.sym) || texto == "dynamic") =>
                    {
                        if !args.is_empty() {
                            return None;
                        }
                        Vec::new()
                    }
                    _ => return None,
                };
                if args.is_empty() {
                    if parametros.is_empty() {
                        texto
                    } else if parametros.iter().all(|b| !b) {
                        // Instanciado aos limites: sem limite, `dynamic`.
                        format!("{texto}<{}>", vec!["dynamic"; parametros.len()].join(", "))
                    } else {
                        return None;
                    }
                } else if args.len() == parametros.len() {
                    let partes: Option<Vec<String>> = args
                        .iter()
                        .map(|&a| self.exibir(u, ast_, a, params))
                        .collect();
                    format!("{texto}<{}>", partes?.join(", "))
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        Some(if no.nullable {
            format!("{base}?")
        } else {
            base
        })
    }

    /// `namedType.isDeferred`: o prefixo do tipo vem de um import `deferred`.
    fn adiado(&self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId) -> bool {
        let TypeKind::Named { name, .. } = &ast_.ty(t).kind else {
            return false;
        };
        let [p, _] = &name[..] else { return false };
        self.programa
            .library(self.programa.unit(u).library)
            .imports
            .iter()
            .any(|i| i.deferred && i.prefix == Some(p.sym))
    }

    /// `TypeProvider.isNonSubtypableClass`.
    fn proibida(&self, id: ClassId) -> bool {
        let c = self.programa.class(id);
        match &self.programa.library(c.library).uri[..] {
            "dart:core" => {
                matches!(
                    self.nome(c.name),
                    "bool" | "double" | "Enum" | "int" | "Null" | "num" | "Record" | "String"
                )
            }
            "dart:async" => self.nome(c.name) == "FutureOr",
            "dart:typed_data" => false,
            _ => false,
        }
    }

    /// O tipo escrito é anulável: `?` nele ou num alias que ele expande.
    fn anulavel(&self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, prof: u32) -> bool {
        let no = ast_.ty(t);
        if no.nullable {
            return true;
        }
        let TypeKind::Named { name, .. } = &no.kind else { return false };
        if prof > 8 {
            return false;
        }
        match self.elemento(u, name) {
            Some(Element::Typedef(tid)) => {
                let td = self.programa.typedef(tid);
                let ast_td = &self.programa.unit(td.decl.unit).ast;
                match &ast_td.decl(td.decl.decl).kind {
                    DeclKind::Typedef(d) => match d.kind {
                        TypedefKind::Alias(corpo) => self.anulavel(td.decl.unit, ast_td, corpo, prof + 1),
                        TypedefKind::Legacy { .. } => false,
                    },
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// A classe `nome` do `dart:core`.
    fn do_core(&self, nome: &str) -> Option<ClassId> {
        self.programa.classes.iter().position(|c| {
            c.decl.is_some() && self.programa.library(c.library).uri == "dart:core" && self.nome(c.name) == nome
        })
        .map(|i| ClassId(i as u32))
    }

    /// O que o `ResolutionVisitor._resolveType` vê num tipo de cláusula:
    /// o elemento do tipo, seguindo aliases; [`Resolvido::Ignorar`] quando a
    /// resolução do nome já relatou erro (nome indefinido, não é tipo) ou
    /// aqui não se decide.
    fn resolver(
        &self,
        u: UnitId,
        ast_: &ast::Ast,
        t: ast::TypeId,
        params: &[SymbolId],
        prof: u32,
        ignora_nao_tipo: bool,
    ) -> Resolvido {
        if prof > 8 {
            return Resolvido::Ignorar;
        }
        let TypeKind::Named { name, .. } = &ast_.ty(t).kind else {
            return Resolvido::Ignorar;
        };
        if let [n] = &name[..]
            && params.contains(&n.sym)
        {
            return Resolvido::NaoClasse;
        }
        // Nome ambíguo (`ambiguous_import`): não se decide.
        let ligacao = match &name[..] {
            [n] => self.programa.lookup_na_unidade(u, n.sym),
            [p, n] => self.programa.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        };
        if ligacao.is_some_and(|b| b.ambiguous) {
            return Resolvido::Ignorar;
        }
        match self.elemento(u, name) {
            Some(Element::Class(id)) => Resolvido::Classe(id),
            Some(Element::Typedef(tid)) => {
                let td = self.programa.typedef(tid);
                let ast_td = &self.programa.unit(td.decl.unit).ast;
                let DeclKind::Typedef(d) = &ast_td.decl(td.decl.decl).kind else {
                    return Resolvido::Ignorar;
                };
                let TypedefKind::Alias(corpo) = d.kind else {
                    return Resolvido::NaoClasse;
                };
                let ps: Vec<SymbolId> = d.type_params.iter().map(|p| p.name.sym).collect();
                match &ast_td.ty(corpo).kind {
                    TypeKind::Named { name: n2, .. } => {
                        if let [unico] = &n2[..]
                            && ps.contains(&unico.sym)
                        {
                            // `aliasedType is TypeParameterType`: o alias
                            // escrito na cláusula. Um alias que chega a outro
                            // depende da substituição dos argumentos (não se
                            // decide aqui).
                            return if prof == 0 { Resolvido::AliasDeParametro } else { Resolvido::Ignorar };
                        }
                        self.resolver(td.decl.unit, ast_td, corpo, &[], prof + 1, ignora_nao_tipo)
                    }
                    _ => Resolvido::NaoClasse,
                }
            }
            // Um nome que não é tipo, ou que não existe: em `extends`,
            // `implements` e `with` o `NamedTypeResolver` não relata nada
            // (`reportNullOrNonTypeElement`), e o erro é o daqui.
            Some(_) => {
                if ignora_nao_tipo {
                    Resolvido::NaoClasse
                } else {
                    Resolvido::Ignorar
                }
            }
            None => match &name[..] {
                [n] if matches!(self.nome(n.sym), "dynamic" | "Never")
                    && self.programa.lookup_na_unidade(u, n.sym).is_none() =>
                {
                    Resolvido::NaoClasse
                }
                [.., n] if ignora_nao_tipo && prof == 0 => {
                    let prefixo = if name.len() == 2 { Some(name[0].sym) } else { None };
                    if self.ignora_indefinido(u, prefixo, n.sym) {
                        Resolvido::Ignorar
                    } else {
                        Resolvido::NaoClasse
                    }
                }
                _ => Resolvido::Ignorar,
            },
        }
    }

    /// `CompilationUnitElementImpl.shouldIgnoreUndefined`: o nome pode vir de
    /// um import que não existe (com o prefixo, ou num `show`), ou, se começa
    /// com `_$`, de uma parte gerada que ainda não existe.
    fn ignora_indefinido(&self, u: UnitId, prefixo: Option<SymbolId>, nome: SymbolId) -> bool {
        let lib_id = self.programa.unit(u).library;
        let lib = self.programa.library(lib_id);
        for &uid in &lib.units {
            let unidade = self.programa.unit(uid);
            for (i, d) in unidade.unit.directives.iter().enumerate() {
                match &d.kind {
                    ast::DirectiveKind::Import { prefix, combinators, .. } => {
                        if prefix.map(|p| p.sym) != prefixo {
                            continue;
                        }
                        let sintetica = !lib
                            .imports
                            .iter()
                            .any(|im| im.unit == uid && im.directive == i && !self.programa.library(im.library).units.is_empty());
                        if !sintetica {
                            continue;
                        }
                        let shows: Vec<&Vec<ast::Name>> = combinators
                            .iter()
                            .filter_map(|c| match c {
                                ast::Combinator::Show(v) => Some(v),
                                ast::Combinator::Hide(_) => None,
                            })
                            .collect();
                        if prefixo.is_some() && shows.is_empty() {
                            return true;
                        }
                        if shows.iter().any(|v| v.iter().any(|n| n.sym == nome)) {
                            return true;
                        }
                    }
                    ast::DirectiveKind::Part { uri } if prefixo.is_none() && self.nome(nome).starts_with("_$") => {
                        let Some(texto) = dartforge_elements::load::string_lit_value(uri) else { continue };
                        let gerado = [".g.dart", ".pb.dart", ".pbenum.dart", ".pbserver.dart", ".pbjson.dart", ".template.dart"]
                            .iter()
                            .any(|s| texto.ends_with(s));
                        let existe = texto.contains(':')
                            || unidade.path.as_ref().and_then(|p| p.parent()).is_none_or(|d| d.join(&texto).is_file());
                        if gerado && !existe {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
        false
    }

    /// O elemento como o analyzer o exibe (`ElementDisplayStringBuilder.writeClassElement`):
    /// modificadores, nome, parâmetros de tipo e cláusulas; `None` onde a
    /// exibição dependeria de tipos que aqui não se calculam.
    fn exibir_classe(&self, id: ClassId) -> Option<String> {
        let c = self.programa.class(id);
        if !matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) {
            return None;
        }
        let decl = c.decl?;
        let ast_ = &self.programa.unit(decl.unit).ast;
        let DeclKind::Class(d) = &ast_.decl(decl.decl).kind else { return None };
        let m = c.modifiers;
        let mut s = String::new();
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
        s.push_str("class ");
        s.push_str(self.nome(c.name));
        let params: Vec<SymbolId> = d.type_params.iter().map(|p| p.name.sym).collect();
        if !d.type_params.is_empty() {
            let mut ps = Vec::new();
            for p in d.type_params.iter() {
                let mut x = self.nome(p.name.sym).to_string();
                if let Some(b) = p.bound {
                    x.push_str(" extends ");
                    x.push_str(&self.exibir(decl.unit, ast_, b, &params)?);
                }
                ps.push(x);
            }
            s.push_str(&format!("<{}>", ps.join(", ")));
        }
        if let Some(t) = d.extends {
            let e = self.exibir(decl.unit, ast_, t, &params)?;
            if e != "Object" {
                s.push_str(" extends ");
                s.push_str(&e);
            }
        }
        for (palavra, tipos) in [(" with ", &d.with), (" implements ", &d.implements)] {
            if !tipos.is_empty() {
                let v: Option<Vec<String>> = tipos.iter().map(|&t| self.exibir(decl.unit, ast_, t, &params)).collect();
                s.push_str(palavra);
                s.push_str(&v?.join(", "));
            }
        }
        Some(s)
    }

    /// A classe (ou mixin) declara campo de instância?
    fn tem_campo_de_instancia(&self, id: ClassId) -> bool {
        let c = self.programa.class(id);
        if matches!(c.kind, ClassKind::Enum | ClassKind::ExtensionType) {
            return false;
        }
        let Some(decl) = c.decl else { return false };
        let ast_ = &self.programa.unit(decl.unit).ast;
        let membros = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => &d.members,
            DeclKind::Mixin(d) => &d.members,
            _ => return false,
        };
        membros.iter().any(|&m| matches!(&ast_.member(m).kind, ast::MemberKind::Field(vl) if !vl.static_))
    }

    fn e_enum_do_core(&self, id: ClassId) -> bool {
        let c = self.programa.class(id);
        self.programa.library(c.library).uri == "dart:core" && self.nome(c.name) == "Enum"
    }

    /// Tem construtor gerador escrito (nem sintético nem `factory`)?
    fn declara_construtor(&self, id: ClassId) -> bool {
        self.programa.class(id).constructors.values().any(|&f| {
            let f = self.programa.function(f);
            f.kind == FunctionKind::Constructor && !f.factory
        })
    }

    /// Só construtores `factory` (e pelo menos um)? Uma classe sem construtor
    /// escrito tem o sintético, gerador.
    fn so_factory(&self, id: ClassId) -> bool {
        let c = self.programa.class(id);
        !c.constructors.is_empty()
            && c.constructors
                .values()
                .all(|&f| self.programa.function(f).factory)
    }
}

/// As cláusulas de uma declaração de classe, alias, enum ou mixin.
struct Clausulas<'a> {
    extends: Option<ast::TypeId>,
    with: &'a [ast::TypeId],
    implements: &'a [ast::TypeId],
    on: &'a [ast::TypeId],
    params: Vec<SymbolId>,
}

fn clausulas(k: &DeclKind) -> Option<Clausulas<'_>> {
    let ps = |v: &[ast::TypeParameter]| v.iter().map(|p| p.name.sym).collect::<Vec<_>>();
    match k {
        DeclKind::Class(x) => Some(Clausulas {
            extends: x.extends,
            with: &x.with,
            implements: &x.implements,
            on: &[],
            params: ps(&x.type_params),
        }),
        DeclKind::Enum(x) => Some(Clausulas {
            extends: None,
            with: &x.with,
            implements: &x.implements,
            on: &[],
            params: ps(&x.type_params),
        }),
        DeclKind::Mixin(x) => Some(Clausulas {
            extends: None,
            with: &[],
            implements: &x.implements,
            on: &x.on,
            params: ps(&x.type_params),
        }),
        _ => None,
    }
}

/// O que a porta decidiu e o que ela relatou no caminho.
struct Resultado {
    porta: Porta,
    relatos: Vec<Diagnostic>,
}

/// A porta do `ErrorVerifier` para a classe `id` (com declaração em `lib`),
/// com os diagnósticos que ela mesma relata.
fn avaliar(l: &Leitor<'_>, lib: LibraryId, id: ClassId) -> Option<Resultado> {
    let classe = l.programa.class(id);
    let decl = classe.decl?;
    let u = decl.unit;
    let ast_ = &l.programa.unit(u).ast;
    let k = &ast_.decl(decl.decl).kind;
    let cl = clausulas(k)?;
    let sdk = l.programa.library(lib).is_sdk;
    let mut relatos = Vec::new();
    let mut fechada = false;
    let mut incerta = false;
    let alvo = |t: ast::TypeId| l.alvo(u, ast_, t, 0);
    // `_checkForExtendsOrImplementsDisallowedClass`: nunca numa biblioteca `dart:`.
    let proibido = |a: Alvo| !sdk && matches!(a, Alvo::Classe(c) if l.proibida(c));

    if classe.kind == ClassKind::Mixin {
        // `_checkMixinInheritance`: `on` e `implements`.
        for &t in cl.on {
            match alvo(t) {
                a if proibido(a) => fechada = true,
                Alvo::Classe(_) => {
                    if l.adiado(u, ast_, t) {
                        relatos.push(Diagnostic::com_codigo(
                            c::MIXIN_SUPER_CLASS_CONSTRAINT_DEFERRED_CLASS,
                            ast_.ty(t).span,
                            [] as [&str; 0],
                        ));
                        fechada = true;
                    }
                }
                Alvo::NaoInterface => {}
                Alvo::Desconhecido => incerta = true,
            }
        }
        implements(
            l,
            u,
            ast_,
            cl.implements,
            &mut relatos,
            &mut fechada,
            &mut incerta,
            sdk,
        );
    } else {
        // `_checkClassInheritance`, na ordem dos `&&`: um passo que relata
        // encerra os seguintes.
        if let Some(t) = cl.extends {
            match alvo(t) {
                a if proibido(a) => fechada = true,
                Alvo::Desconhecido => incerta = true,
                _ => {}
            }
        }
        if !fechada {
            implements(
                l,
                u,
                ast_,
                cl.implements,
                &mut relatos,
                &mut fechada,
                &mut incerta,
                sdk,
            );
        }
        if !fechada {
            mixins(
                l,
                u,
                id,
                ast_,
                cl.with,
                &mut relatos,
                &mut fechada,
                &mut incerta,
                sdk,
            );
        }
        // `_checkForNoGenerativeConstructorsInSuperclass`: com mixins, o
        // supertipo é a aplicação sintética `S with M`, cujos construtores
        // são os geradores de `S` repassados — nunca só `factory`.
        if !fechada
            && classe.kind == ClassKind::Class
            && cl.with.is_empty()
            && let (Some(t), Some(s)) = (cl.extends, classe.supertype_class)
            && !l.so_factory(id)
            && l.so_factory(s)
        {
            relatos.push(Diagnostic::com_codigo(
                c::NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS,
                ast_.ty(t).span,
                [l.nome(classe.name), l.nome(l.programa.class(s).name)],
            ));
            fechada = true;
        }
    }
    let porta = if fechada {
        Porta::Fechada
    } else if incerta {
        Porta::Incerta
    } else {
        Porta::Aberta
    };
    Some(Resultado { porta, relatos })
}

/// O construtor sem nome que uma classe oferece a uma subclasse:
/// `Ok(f)` declarado, `Err(())` o sintético sem parâmetros (também o de
/// uma abstrata, que o modelo não cria); numa aplicação de mixin, o gerador
/// repassado da base.
fn sem_nome_da_superclasse(l: &Leitor<'_>, s: ClassId) -> Option<Result<dartforge_elements::model::FunctionElementId, ()>> {
    let vazio = l.nomes.lookup("");
    let mut atual = s;
    let mut repassado = false;
    for _ in 0..16 {
        let ce = l.programa.class(atual);
        if ce.kind == ClassKind::MixinApplication {
            atual = ce.supertype_class?;
            repassado = true;
            continue;
        }
        if let Some(&f) = vazio.and_then(|v| ce.constructors.get(&v)) {
            let fe = l.programa.function(f);
            if repassado && fe.factory {
                return None;
            }
            if matches!(fe.node, dartforge_elements::model::FunctionRef::None) {
                return Some(Err(()));
            }
            return Some(Ok(f));
        }
        return (ce.constructors.is_empty() && ce.kind == ClassKind::Class).then_some(Err(()));
    }
    None
}

/// `_checkForNoDefaultSuperConstructorImplicit`
/// (`an611:src/generated/error_verifier.dart:4634-4678`): classe sem
/// construtor declarado cujo `extends` é uma classe com o sem nome factory
/// (`non_generative_implicit_constructor`) ou sem um sem nome que dispense
/// argumentos (`no_default_super_constructor`, calado para as classes que
/// não se estendem). No nome da classe. Fica de fora o `extends` que não
/// resolve aqui (o supertipo seria `Object`, e a porta já está incerta).
fn super_implicito_da_classe(
    l: &Leitor<'_>,
    u: UnitId,
    ast_: &ast::Ast,
    nome: SymbolId,
    nome_span: dartforge_diagnostics::Span,
    t: ast::TypeId,
    params: &[SymbolId],
) -> Option<Diagnostic> {
    let Alvo::Classe(s) = l.alvo(u, ast_, t, 0) else { return None };
    let se = l.programa.class(s);
    if !matches!(se.kind, ClassKind::Class | ClassKind::MixinApplication) || l.anulavel(u, ast_, t, 0) {
        return None;
    }
    if l.programa.library(se.library).uri == "dart:core" && matches!(l.nome(se.name), "Function" | "Null") {
        return None;
    }
    let classe = l.nome(nome).to_string();
    match sem_nome_da_superclasse(l, s) {
        Some(Ok(f)) if l.programa.function(f).factory => {
            let super_nome = l.nome(se.name).to_string();
            let exibido = exibir_construtor_sem_nome(l, s, f);
            return Some(Diagnostic::com_codigo(
                c::NON_GENERATIVE_IMPLICIT_CONSTRUCTOR,
                nome_span,
                [super_nome.as_str(), classe.as_str(), exibido.as_str()],
            ));
        }
        // `isDefaultConstructor`: nenhum parâmetro obrigatório.
        Some(Err(())) => return None,
        Some(Ok(f)) => {
            let dartforge_elements::model::FunctionRef::Constructor { unit, member } = l.programa.function(f).node else { return None };
            let ast::MemberKind::Constructor(k) = &l.programa.unit(unit).ast.member(member).kind else { return None };
            if !k.parameters.iter().any(|p| p.kind == ast::ParameterKind::Required || (p.kind == ast::ParameterKind::Named && p.required)) {
                return None;
            }
        }
        None => {}
    }
    if l.proibida(s) {
        return None;
    }
    let exibido = l.exibir(u, ast_, t, params)?;
    Some(Diagnostic::com_codigo(c::NO_DEFAULT_SUPER_CONSTRUCTOR_IMPLICIT, nome_span, [exibido.as_str(), classe.as_str()]))
}

/// `ConstructorElement.getDisplayString()` do sem nome `f` de `s` (só vai à
/// correção de `non_generative_implicit_constructor`): `A A(int x)`, com os
/// tipos como escritos.
fn exibir_construtor_sem_nome(l: &Leitor<'_>, s: ClassId, f: dartforge_elements::model::FunctionElementId) -> String {
    let se = l.programa.class(s);
    let params_de_s: Vec<SymbolId> = se.type_params.iter().map(|p| p.name).collect();
    let mut texto = l.nome(se.name).to_string();
    if !params_de_s.is_empty() {
        let nomes: Vec<&str> = params_de_s.iter().map(|&p| l.nome(p)).collect();
        texto = format!("{texto}<{}>", nomes.join(", "));
    }
    let mut s_ = format!("{texto} {}(", l.nome(se.name));
    if let dartforge_elements::model::FunctionRef::Constructor { unit, member } = l.programa.function(f).node
        && let ast::MemberKind::Constructor(k) = &l.programa.unit(unit).ast.member(member).kind
    {
        let ast_k = &l.programa.unit(unit).ast;
        let fonte = &l.programa.unit(unit).source;
        let mut ultimo: Option<ast::ParameterKind> = None;
        let mut fecho = "";
        for (i, p) in k.parameters.iter().enumerate() {
            if i != 0 {
                s_.push_str(", ");
            }
            if ultimo != Some(p.kind) {
                s_.push_str(fecho);
                let (abre, fecha) = match p.kind {
                    ast::ParameterKind::Required => ("", ""),
                    ast::ParameterKind::Optional => ("[", "]"),
                    ast::ParameterKind::Named => ("{", "}"),
                };
                s_.push_str(abre);
                fecho = fecha;
                ultimo = Some(p.kind);
            }
            if p.kind == ast::ParameterKind::Named && p.required {
                s_.push_str("required ");
            }
            let tipo = match p.ty {
                Some(ty) => l.exibir(unit, ast_k, ty, &params_de_s).unwrap_or_else(|| {
                    let sp = ast_k.ty(ty).span;
                    fonte[sp.start..sp.end].to_string()
                }),
                None => "dynamic".to_string(),
            };
            s_.push_str(&tipo);
            s_.push(' ');
            if let Some(n) = p.nome_externo() {
                s_.push_str(l.nome(n.sym));
            }
            if let Some(d) = p.default_value {
                let sp = ast_k.expr(d).span;
                s_.push_str(" = ");
                s_.push_str(&fonte[sp.start..sp.end]);
            }
        }
        s_.push_str(fecho);
    }
    s_.push(')');
    s_
}

/// `_checkForImplementsClauseErrorCodes`.
#[allow(clippy::too_many_arguments)]
fn implements(
    l: &Leitor<'_>,
    u: UnitId,
    ast_: &ast::Ast,
    tipos: &[ast::TypeId],
    relatos: &mut Vec<Diagnostic>,
    fechada: &mut bool,
    incerta: &mut bool,
    sdk: bool,
) {
    for &t in tipos {
        match l.alvo(u, ast_, t, 0) {
            Alvo::Classe(c) if !sdk && l.proibida(c) => *fechada = true,
            Alvo::Desconhecido => *incerta = true,
            _ => {
                if l.adiado(u, ast_, t) {
                    relatos.push(Diagnostic::com_codigo(
                        c::IMPLEMENTS_DEFERRED_CLASS,
                        ast_.ty(t).span,
                        [] as [&str; 0],
                    ));
                    *fechada = true;
                }
            }
        }
    }
}

/// `_checkForAllMixinErrorCodes`.
#[allow(clippy::too_many_arguments)]
fn mixins(
    l: &Leitor<'_>,
    u: UnitId,
    id: ClassId,
    ast_: &ast::Ast,
    tipos: &[ast::TypeId],
    relatos: &mut Vec<Diagnostic>,
    fechada: &mut bool,
    incerta: &mut bool,
    sdk: bool,
) {
    let mut anteriores: Vec<ClassId> = Vec::new();
    for (posicao, &t) in tipos.iter().enumerate() {
        let m = match l.alvo(u, ast_, t, 0) {
            Alvo::Classe(m) => m,
            Alvo::NaoInterface => continue,
            Alvo::Desconhecido => {
                *incerta = true;
                continue;
            }
        };
        if !sdk && l.proibida(m) {
            *fechada = true;
            continue;
        }
        let span = ast_.ty(t).span;
        let anteriores_aqui = anteriores.clone();
        anteriores.push(m);
        let anteriores = anteriores_aqui;
        if l.adiado(u, ast_, t) {
            relatos.push(Diagnostic::com_codigo(
                c::MIXIN_DEFERRED_CLASS,
                span,
                [] as [&str; 0],
            ));
            *fechada = true;
        }
        let el = l.programa.class(m);
        let duplicada = l
            .programa
            .classes
            .iter()
            .filter(|c| c.library == el.library && c.name == el.name)
            .count()
            > 1;
        if m == id && duplicada {
            // `class A {}` e depois `class A with A` (`duplicate_definition`):
            // para o analyzer o nome é o da primeira declaração, e aqui ele
            // resolveu para a segunda; não se decide.
            *incerta = true;
            continue;
        }
        match el.kind {
            ClassKind::ExtensionType => {}
            ClassKind::Mixin => {
                // Decidido com tipos (`fase_mixins`): o relato fecha a porta.
                if let Some(decisao) = decisao_de_mixin(id, posicao) {
                    if let Some(d) = decisao {
                        relatos.push(d);
                        *fechada = true;
                    }
                    continue;
                }
                // Restrições `on` e membros invocados por `super` dependem
                // de tipos: só um mixin sem `on` e sem `super` é seguro.
                let decl = el.decl.map(|d| {
                    let unidade = l.programa.unit(d.unit);
                    let sp = unidade.ast.decl(d.decl).span;
                    unidade.source
                        .get(sp.start..sp.end)
                        .unwrap_or("")
                        .contains("super")
                });
                if decl != Some(false) {
                    *incerta = true;
                } else if !el.on.is_empty() {
                    // `_checkForMixinSuperclassConstraints`.
                    match restricoes_satisfeitas(l, id, &anteriores, m) {
                        Some(Ok(())) => {}
                        // `mixin_application_not_implemented_interface`, no
                        // nome do mixin, fecha a porta.
                        Some(Err(i)) => {
                            *fechada = true;
                            if let Some(d) = nao_implementa(l, u, id, ast_, t, m, i) {
                                relatos.push(d);
                            }
                        }
                        None => *incerta = true,
                    }
                }
            }
            ClassKind::Enum => *incerta = true,
            ClassKind::Class | ClassKind::MixinApplication => {
                let nome = l.nome(el.name);
                if !el.modifiers.mixin && l.declara_construtor(m) {
                    relatos.push(Diagnostic::com_codigo(
                        c::MIXIN_CLASS_DECLARES_CONSTRUCTOR,
                        span,
                        [nome],
                    ));
                    *fechada = true;
                }
                // `_checkForMixinInheritsNotFromObject`.
                let objeto = match el.supertype_class {
                    None => el.supertype.is_none(),
                    Some(s) => {
                        let sc = l.programa.class(s);
                        sc.supertype_class.is_none()
                            && l.programa.library(sc.library).uri == "dart:core"
                    }
                };
                let resolvida = el.supertype.is_none() || el.supertype_class.is_some();
                if !resolvida || el.mixins.len() != el.mixin_classes.len() {
                    *incerta = true;
                    continue;
                }
                let herda = !objeto
                    || !(el.mixins.is_empty()
                        || el.kind == ClassKind::MixinApplication && el.mixins.len() < 2);
                if herda {
                    relatos.push(Diagnostic::com_codigo(
                        c::MIXIN_INHERITS_FROM_NOT_OBJECT,
                        span,
                        [nome],
                    ));
                    *fechada = true;
                }
            }
        }
    }
}

/// `_checkForMixinClassErrorCodes` de uma `mixin class` (ou alias `mixin
/// class A = …`): construtor gerador não trivial, e supertipo que não é
/// `Object` (pelo `extends` ou pela cláusula `with`).
fn classe_mixin(
    l: &Leitor<'_>,
    u: UnitId,
    id: ClassId,
    ast_: &ast::Ast,
    fonte: &str,
) -> Vec<Diagnostic> {
    let classe = l.programa.class(id);
    let nome = l.nome(classe.name);
    let Some(decl) = classe.decl else {
        return Vec::new();
    };
    let DeclKind::Class(d) = &ast_.decl(decl.decl).kind else {
        return Vec::new();
    };
    let mut saida = Vec::new();
    for &mid in d.members.iter() {
        let ast::MemberKind::Constructor(k) = &ast_.member(mid).kind else {
            continue;
        };
        // Construtor primário e membro `new(…)` são sintaxe 3.13, em que o
        // analyzer relata outro código
        // (`mixin_class_declares_non_trivial_generative_constructor`).
        let escrito = fonte
            .get(k.class_name.span.start..k.class_name.span.end)
            .unwrap_or("");
        if k.factory || k.parte_primaria {
            continue;
        }
        // `_checkForMixinClassErrorCodes` do 3.13 (checkout main,
        // `error_verifier.dart:6657-6712`): o primário com parâmetros, no
        // nome (`A` ou `A.nome`); sem parâmetros, a parte `this` com
        // inicializadores (no `:`) ou com corpo bloco (no `{`). Sai com o
        // código 3.6, que a tradução do 3.13 troca pelo
        // `MIXIN_CLASS_DECLARES_NON_TRIVIAL_GENERATIVE_CONSTRUCTOR`.
        if d.primary_constructor == Some(mid) {
            let sp = if !k.parameters.is_empty() {
                Some(Span { start: k.class_name.span.start, end: k.name.map_or(k.class_name.span.end, |n| n.span.end) })
            } else {
                let parte = ast_.member(mid).span;
                let texto = fonte.get(parte.start..parte.end).unwrap_or("");
                if !texto.starts_with("this") {
                    None
                } else if !k.initializers.is_empty() {
                    texto.find(':').map(|i| Span { start: parte.start + i, end: parte.start + i + 1 })
                } else if matches!(k.body, ast::FunctionBody::Block(_)) {
                    texto.find('{').map(|i| Span { start: parte.start + i, end: parte.start + i + 1 })
                } else {
                    None
                }
            };
            if let Some(sp) = sp {
                saida.push(Diagnostic::com_codigo(c::MIXIN_CLASS_DECLARES_CONSTRUCTOR, sp, [nome]));
            }
            continue;
        }
        // O membro `new(…)` (3.13) tem o mesmo teste de trivialidade, no `new`.
        let _ = escrito;
        // `ConstructorDeclaration.isTrivial`: `A();` sem nada mais.
        let trivial = k.redirect.is_none()
            && k.parameters.is_empty()
            && k.initializers.is_empty()
            && matches!(k.body, ast::FunctionBody::Empty)
            && !k.external;
        if !trivial {
            saida.push(Diagnostic::com_codigo(
                c::MIXIN_CLASS_DECLARES_CONSTRUCTOR,
                k.class_name.span,
                [nome],
            ));
        }
    }
    // `superclass.typeOrThrow.isDartCoreObject`; um `extends` que não se
    // resolve aqui não é decidido.
    let estende_outra = match d.extends {
        None => Some(false),
        Some(t) => match l.alvo(u, ast_, t, 0) {
            Alvo::Classe(s) => Some(
                !(l.programa.class(s).supertype_class.is_none()
                    && l.programa.library(l.programa.class(s).library).uri == "dart:core"),
            ),
            Alvo::NaoInterface => Some(true),
            Alvo::Desconhecido => None,
        },
    };
    match (estende_outra, d.extends) {
        (Some(true), Some(t)) => saida.push(Diagnostic::com_codigo(
            c::MIXIN_CLASS_DECLARATION_EXTENDS_NOT_OBJECT,
            ast_.ty(t).span,
            [nome],
        )),
        (Some(false), _) => {
            if let (Some(&primeiro), Some(&ultimo)) = (d.with.first(), d.with.last()) {
                let alias_simples = d.mixin_application && d.with.len() < 2;
                // O nó `WithClause` começa na palavra `with`, logo antes do primeiro tipo.
                let antes = fonte[..ast_.ty(primeiro).span.start].trim_end();
                if !alias_simples && antes.ends_with("with") {
                    let span = dartforge_diagnostics::Span {
                        start: antes.len() - 4,
                        end: ast_.ty(ultimo).span.end,
                    };
                    saida.push(Diagnostic::com_codigo(
                        c::MIXIN_CLASS_DECLARATION_EXTENDS_NOT_OBJECT,
                        span,
                        [nome],
                    ));
                }
            }
        }
        _ => {}
    }
    saida
}

/// Os supertipos de `x`, transitivos, com ela mesma (pelas classes ligadas).
fn supertipos(l: &Leitor<'_>, x: ClassId, saida: &mut Vec<ClassId>) {
    if saida.contains(&x) {
        return;
    }
    saida.push(x);
    let c = l.programa.class(x);
    let diretos: Vec<ClassId> = c
        .supertype_class
        .into_iter()
        .chain(c.mixin_classes.iter().copied())
        .chain(c.interface_classes.iter().copied())
        .chain(c.on_classes.iter().copied())
        .collect();
    for s in diretos {
        supertipos(l, s, saida);
    }
}

/// As restrições `on` do mixin `m`, aplicado em `id` depois dos mixins
/// `anteriores`, são satisfeitas? Tipos de interface são subtipos só
/// nominalmente, então basta a classe da restrição estar entre os
/// supertipos da superclasse ou dos mixins anteriores; com argumentos de
/// tipo em jogo (restrição genérica) a resposta depende de tipos: `None`.
fn restricoes_satisfeitas(
    l: &Leitor<'_>,
    id: ClassId,
    anteriores: &[ClassId],
    m: ClassId,
) -> Option<Result<(), usize>> {
    let mixin = l.programa.class(m);
    if mixin.on.len() != mixin.on_classes.len() {
        return None;
    }
    // Mixin com `augment` (a restrição pode vir da augmentation, que o
    // analyzer sem o experimento não lê) ou classe em ciclo de herança.
    let aumentado = l.programa.library(mixin.library).units.iter().any(|&u| {
        let a = &l.programa.unit(u).ast;
        l.programa.unit(u).unit.declarations.iter().any(|&d| {
            a.decl(d).augment && matches!(&a.decl(d).kind, DeclKind::Mixin(x) if x.name.sym == mixin.name)
        })
    });
    let repetido = l.programa.classes.iter().filter(|c| c.library == mixin.library && c.name == mixin.name).count() > 1;
    let mut passos = 0;
    if aumentado || repetido || ciclo(l, id, id, &mut Vec::new(), &mut passos).is_some() || passos > 20_000 {
        return None;
    }
    for &(u, t) in &mixin.on {
        if let TypeKind::Named { args, .. } = &l.programa.unit(u).ast.ty(t).kind
            && !args.is_empty()
        {
            return None;
        }
    }
    if mixin
        .on_classes
        .iter()
        .any(|&c| !l.programa.class(c).type_params.is_empty())
    {
        return None;
    }
    // `_enclosingClass.supertype`: num enum, `Enum`.
    let base = match l.programa.class(id).supertype_class {
        Some(b) => b,
        None if l.programa.class(id).kind == ClassKind::Enum => l.do_core("Enum")?,
        None => return None,
    };
    let mut conhecidos = Vec::new();
    supertipos(l, base, &mut conhecidos);
    for &a in anteriores {
        supertipos(l, a, &mut conhecidos);
    }
    match mixin.on_classes.iter().position(|c| !conhecidos.contains(c)) {
        None => Some(Ok(())),
        Some(i) => Some(Err(i)),
    }
}

/// O `mixin_application_not_implemented_interface` do mixin `m` (tipo `t`
/// na cláusula `with` de `id`), cuja restrição `on` de índice `i` a
/// superclasse não implementa: `'M' can't be mixed onto 'S' because 'S'
/// doesn't implement 'A'.` O supertipo é o `extends` escrito (`Object`
/// omitido, `Enum` num enum).
fn nao_implementa(
    l: &Leitor<'_>,
    u: UnitId,
    id: ClassId,
    ast_: &ast::Ast,
    t: ast::TypeId,
    m: ClassId,
    i: usize,
) -> Option<Diagnostic> {
    let classe = l.programa.class(id);
    let decl = classe.decl?;
    let params: Vec<SymbolId> = clausulas(&ast_.decl(decl.decl).kind)?.params;
    let mixin_txt = l.exibir(u, ast_, t, &params)?;
    let mixin_txt = mixin_txt.strip_suffix('?').unwrap_or(&mixin_txt).to_string();
    let super_txt = match (&ast_.decl(decl.decl).kind, clausulas(&ast_.decl(decl.decl).kind)?.extends) {
        (_, Some(e)) => {
            let x = l.exibir(u, ast_, e, &params)?;
            x.strip_suffix('?').unwrap_or(&x).to_string()
        }
        (DeclKind::Enum(_), None) => "Enum".to_string(),
        (_, None) => "Object".to_string(),
    };
    let mixin = l.programa.class(m);
    let (mu, mt) = *mixin.on.get(i)?;
    let mast = &l.programa.unit(mu).ast;
    let restricao = l.exibir(mu, mast, mt, &[])?;
    let TypeKind::Named { name, .. } = &ast_.ty(t).kind else { return None };
    let span = name.last()?.span;
    Some(Diagnostic::com_codigo(
        c::MIXIN_APPLICATION_NOT_IMPLEMENTED_INTERFACE,
        span,
        [mixin_txt.as_str(), super_txt.as_str(), restricao.as_str()],
    ))
}

/// Onde começam os tipos de `extends`, `implements` e `with` de classes,
/// aliases, enums e mixins da biblioteca `lib`. Ali o `NamedTypeResolver`
/// não relata nome indefinido nem nome que não é tipo (o
/// `ResolutionVisitor` relata `*_non_class`, [`verificar`]): quem junta os
/// diagnósticos de `types` descarta `undefined_class`/`not_a_type` nesses
/// pontos.
pub fn nomes_de_clausulas(programa: &Program, lib: LibraryId) -> Vec<(UnitId, usize)> {
    let mut v = Vec::new();
    for classe in &programa.classes {
        if classe.library != lib {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let ast_ = &programa.unit(decl.unit).ast;
        let Some(cl) = clausulas(&ast_.decl(decl.decl).kind) else { continue };
        for &t in cl.extends.iter().chain(cl.with).chain(cl.implements) {
            if let TypeKind::Named { name, .. } = &ast_.ty(t).kind
                && let Some(p) = name.first()
            {
                v.push((decl.unit, p.span.start));
            }
        }
    }
    v
}

/// Os supertipos diretos de `x` como o modelo de elementos do analyzer os
/// guarda: a superclasse (uma classe; `extends` de mixin, enum ou tipo que
/// não é classe não entra), os mixins, as restrições `on` e as interfaces.
fn diretos(l: &Leitor<'_>, x: ClassId) -> (Option<ClassId>, Vec<ClassId>, Vec<ClassId>, Vec<ClassId>) {
    let c = l.programa.class(x);
    let Some(decl) = c.decl else { return (None, Vec::new(), Vec::new(), Vec::new()) };
    let ast_ = &l.programa.unit(decl.unit).ast;
    let Some(cl) = clausulas(&ast_.decl(decl.decl).kind) else { return (None, Vec::new(), Vec::new(), Vec::new()) };
    let classe = |t: ast::TypeId, aceita_mixin: bool| match l.alvo(decl.unit, ast_, t, 0) {
        Alvo::Classe(y) => match l.programa.class(y).kind {
            ClassKind::Class | ClassKind::MixinApplication => Some(y),
            ClassKind::Mixin if aceita_mixin => Some(y),
            _ => None,
        },
        _ => None,
    };
    let sup = cl.extends.and_then(|t| classe(t, false));
    let mixins = cl.with.iter().filter_map(|&t| classe(t, true)).collect();
    let on = cl.on.iter().filter_map(|&t| classe(t, true)).collect();
    let interfaces = cl.implements.iter().filter_map(|&t| classe(t, true)).collect();
    (sup, mixins, on, interfaces)
}

/// `_checkForRecursiveInterfaceInheritance`: a busca em profundidade do
/// analyzer (superclasse, mixins, `on`, interfaces), com o caminho até voltar
/// a `alvo`. Um limite de passos evita a explosão em hierarquias grandes (aí
/// nada se relata).
fn ciclo(l: &Leitor<'_>, alvo: ClassId, el: ClassId, caminho: &mut Vec<ClassId>, passos: &mut u32) -> Option<Vec<ClassId>> {
    *passos += 1;
    if *passos > 20_000 {
        return None;
    }
    // T1.1 c: homônimos da mesma espécie são o mesmo elemento para a
    // hierarquia do analyzer; `augment class A extends A` (sem o
    // experimento: a segunda `A` estende a primeira) é "A implementa a si
    // mesma".
    let mesmo = el == alvo || l.programa.representante_da_classe(el) == l.programa.representante_da_classe(alvo);
    if !caminho.is_empty() && mesmo {
        return Some(caminho.clone());
    }
    if caminho.iter().position(|&p| p == el).is_some_and(|i| i > 0) {
        return None;
    }
    caminho.push(el);
    let (sup, mixins, on, interfaces) = diretos(l, el);
    for s in sup.into_iter().chain(mixins).chain(on).chain(interfaces) {
        if let Some(r) = ciclo(l, alvo, s, caminho, passos) {
            return Some(r);
        }
    }
    caminho.pop();
    None
}

/// O ciclo de `alvo` como o analyzer 3.13 o guarda (`interfaceCycle`,
/// `summary2/interface_cycles.dart` do main): o componente fortemente conexo
/// que o `DependencyWalker` (`_fe_analyzer_shared/lib/src/util/
/// dependency_walker.dart`, o algoritmo de Tarjan) acha ao andar as
/// declarações da biblioteca em ordem, com as dependências na ordem
/// superclasse, mixins, interfaces e restrições `on`. Os nós saem da pilha
/// do último visitado para o primeiro, e todas as classes do componente
/// levam a mesma lista (`class A extends B`, `B extends C`, `C extends A`:
/// `C, B, A` nas três). `None` se `alvo` não está num componente de mais de
/// uma classe, ou se a hierarquia é grande demais.
fn componente_3_13(l: &Leitor<'_>, lib: LibraryId, alvo: ClassId) -> Option<Vec<ClassId>> {
    struct Andar {
        /// (índice, menor índice alcançado) dos nós vistos neste passeio.
        indices: std::collections::HashMap<ClassId, (u32, u32)>,
        avaliados: std::collections::HashSet<ClassId>,
        pilha: Vec<ClassId>,
        proximo: u32,
        passos: u32,
        achado: Option<Vec<ClassId>>,
    }
    fn conectar(l: &Leitor<'_>, a: &mut Andar, no: ClassId, alvo: ClassId) {
        a.passos += 1;
        let meu = a.proximo;
        a.proximo += 1;
        a.indices.insert(no, (meu, meu));
        a.pilha.push(no);
        let (sup, mixins, on, interfaces) = diretos(l, no);
        for d in sup.into_iter().chain(mixins).chain(interfaces).chain(on) {
            if a.passos > 20_000 {
                return;
            }
            if d == no || a.avaliados.contains(&d) {
                continue;
            }
            let visto = match a.indices.get(&d) {
                Some(&(i, _)) => i,
                None => {
                    conectar(l, a, d, alvo);
                    a.indices.get(&d).map_or(u32::MAX, |x| x.1)
                }
            };
            if let Some(eu) = a.indices.get_mut(&no)
                && visto < eu.1
            {
                eu.1 = visto;
            }
        }
        if a.indices.get(&no).is_some_and(|&(i, menor)| i == menor) {
            let mut componente = Vec::new();
            while let Some(outro) = a.pilha.pop() {
                componente.push(outro);
                a.avaliados.insert(outro);
                if outro == no {
                    break;
                }
            }
            if componente.len() > 1 && componente.contains(&alvo) {
                a.achado = Some(componente);
            }
        }
    }
    let mut a = Andar {
        indices: std::collections::HashMap::new(),
        avaliados: std::collections::HashSet::new(),
        pilha: Vec::new(),
        proximo: 1,
        passos: 0,
        achado: None,
    };
    for (i, classe) in l.programa.classes.iter().enumerate() {
        let id = ClassId(i as u32);
        if classe.library != lib || classe.decl.is_none() || a.avaliados.contains(&id) {
            continue;
        }
        // Cada passeio recomeça os índices; os nós de passeios anteriores
        // já estão todos avaliados.
        a.indices.clear();
        a.pilha.clear();
        a.proximo = 1;
        conectar(l, &mut a, id, alvo);
        if a.passos > 20_000 {
            return None;
        }
        if a.achado.is_some() || a.avaliados.contains(&alvo) {
            break;
        }
    }
    a.achado
}

/// As classes, aliases, enums e mixins de `lib` em que o
/// `InheritanceOverrideVerifier.verify()` do analyzer passa das verificações
/// que o encerram cedo (`_checkDirectSuperTypes`, `Enum` em classe concreta,
/// herança recursiva) e chega às sobrescritas. Declaração repetida na
/// biblioteca ou `augment` não entra.
pub fn verificador_de_heranca_prossegue(programa: &Program, lib: LibraryId, nomes: &Interner) -> Vec<ClassId> {
    let l = Leitor { programa, nomes };
    let consumidora = programa.library(lib);
    let versao = consumidora.features.versao();
    let enums_melhorados = (versao.major, versao.minor) >= (2, 17);
    let mut v = Vec::new();
    for (i, classe) in programa.classes.iter().enumerate() {
        if classe.library != lib {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let id = ClassId(i as u32);
        let ast_ = &programa.unit(decl.unit).ast;
        if ast_.decl(decl.decl).augment {
            continue;
        }
        let Some(cl) = clausulas(&ast_.decl(decl.decl).kind) else { continue };
        // T1.4 passo 4: cada homônimo é conferido pelas próprias cláusulas,
        // com os nomes resolvidos pela regra do escopo (passo 1).
        let pode_ter_enum =
            classe.kind == ClassKind::Enum || classe.kind == ClassKind::Mixin || classe.modifiers.abstract_;
        let mut erro = false;
        for &t in cl.implements.iter().chain(cl.on).chain(cl.extends.as_slice()).chain(cl.with) {
            match l.alvo(decl.unit, ast_, t, 0) {
                Alvo::Classe(a) => {
                    if !consumidora.is_sdk {
                        if l.e_enum_do_core(a) && enums_melhorados {
                            erro |= !pode_ter_enum;
                        } else if l.proibida(a) {
                            erro = true;
                        }
                    }
                }
                // Não resolvido aqui: não se decide.
                Alvo::Desconhecido => erro = true,
                Alvo::NaoInterface => {}
            }
        }
        if classe.kind == ClassKind::Enum {
            for &t in cl.with {
                if let Alvo::Classe(m) = l.alvo(decl.unit, ast_, t, 0)
                    && l.tem_campo_de_instancia(m)
                {
                    erro = true;
                }
            }
        }
        // `verify()`: classe concreta com `Enum` entre os supertipos
        // (transitivos) relata `concrete_class_has_enum_superinterface` e para.
        if classe.kind != ClassKind::Enum && classe.kind != ClassKind::Mixin && !classe.modifiers.abstract_ && !classe.modifiers.sealed {
            let mut todos = Vec::new();
            supertipos(&l, id, &mut todos);
            if todos.iter().any(|&s| s != id && l.e_enum_do_core(s)) {
                erro = true;
            }
        }
        if erro {
            continue;
        }
        let mut passos = 0;
        if ciclo(&l, id, id, &mut Vec::new(), &mut passos).is_some() || passos > 20_000 {
            continue;
        }
        v.push(id);
    }
    v
}

/// `_checkForRepeatedType`: o segundo tipo de interface com o mesmo elemento.
fn repetidos(
    l: &Leitor<'_>,
    u: UnitId,
    ast_: &ast::Ast,
    tipos: &[ast::TypeId],
    codigo: Codigo,
    saida: &mut Vec<(UnitId, Diagnostic)>,
) {
    let mut vistos: Vec<ClassId> = Vec::new();
    for &t in tipos {
        if let Alvo::Classe(x) = l.alvo(u, ast_, t, 0) {
            if vistos.contains(&x) {
                let nome = l.nome(l.programa.class(x).name);
                saida.push((u, Diagnostic::com_codigo(codigo, ast_.ty(t).span, [nome])));
            } else {
                vistos.push(x);
            }
        }
    }
}

/// A porta do `ErrorVerifier` para a classe `id`: [`Porta::Aberta`] quando
/// as verificações de cláusula seguintes (modificadores fora da biblioteca,
/// `class_used_as_mixin`) rodariam no analyzer.
pub fn porta(programa: &Program, lib: LibraryId, nomes: &Interner, id: ClassId) -> Porta {
    let l = Leitor { programa, nomes };
    avaliar(&l, lib, id).map_or(Porta::Incerta, |r| r.porta)
}

/// Os diagnósticos das cláusulas de herança da biblioteca `lib`.
pub fn verificar(
    programa: &Program,
    lib: LibraryId,
    nomes: &Interner,
) -> Vec<(UnitId, Diagnostic)> {
    let l = Leitor { programa, nomes };
    let consumidora = programa.library(lib);
    let mut saida = Vec::new();
    for (i, classe) in programa.classes.iter().enumerate() {
        if classe.library != lib {
            continue;
        }
        let Some(decl) = classe.decl else { continue };
        let id = ClassId(i as u32);
        let u = decl.unit;
        let ast_ = &programa.unit(u).ast;
        if let DeclKind::ExtensionType(x) = &ast_.decl(decl.decl).kind {
            // `visitExtensionTypeDeclaration`: `implements` repetido, sem porta.
            repetidos(&l, u, ast_, &x.implements, c::IMPLEMENTS_REPEATED, &mut saida);
            continue;
        }
        let Some(cl) = clausulas(&ast_.decl(decl.decl).kind) else {
            continue;
        };

        // `_checkForBadFunctionUse` (`error_verifier.dart:2226-2269`): sem
        // `class-modifiers` (antes da 3.0), `Function` do `dart:core` no
        // `extends`, no primeiro `implements` que o cita e em cada `with` de uma
        // classe ou alias de classe.
        if matches!(&ast_.decl(decl.decl).kind, DeclKind::Class(_)) && consumidora.features.versao().major < 3 {
            let e_function = |t: ast::TypeId| {
                matches!(l.alvo(u, ast_, t, 0), Alvo::Classe(f) if l.programa.library(l.programa.class(f).library).uri == "dart:core" && l.nome(l.programa.class(f).name) == "Function")
                    && !l.anulavel(u, ast_, t, 0)
            };
            let vazio: [&str; 0] = [];
            if let Some(t) = cl.extends
                && e_function(t)
            {
                saida.push((decl.unit, Diagnostic::com_codigo(dartforge_diagnostics::codigos::warning::DEPRECATED_EXTENDS_FUNCTION, ast_.ty(t).span, vazio)));
            }
            if let Some(&t) = cl.implements.iter().find(|&&t| e_function(t)) {
                saida.push((decl.unit, Diagnostic::com_codigo(dartforge_diagnostics::codigos::warning::DEPRECATED_IMPLEMENTS_FUNCTION, ast_.ty(t).span, vazio)));
            }
            for &t in cl.with {
                if e_function(t) {
                    saida.push((decl.unit, Diagnostic::com_codigo(dartforge_diagnostics::codigos::warning::DEPRECATED_MIXIN_FUNCTION, ast_.ty(t).span, vazio)));
                }
            }
        }

        // `ResolutionVisitor._resolveType`: cada tipo de cláusula precisa
        // nomear uma classe (ou mixin, fora do `extends`).
        let mixin_application = matches!(&ast_.decl(decl.decl).kind, DeclKind::Class(d) if d.mixin_application);
        let grupos_de_resolucao: [(&[ast::TypeId], Codigo, bool); 4] = [
            (
                cl.extends.as_slice(),
                if mixin_application || !cl.with.is_empty() {
                    c::MIXIN_WITH_NON_CLASS_SUPERCLASS
                } else {
                    c::EXTENDS_NON_CLASS
                },
                false,
            ),
            (cl.with, c::MIXIN_OF_NON_CLASS, true),
            (cl.implements, c::IMPLEMENTS_NON_CLASS, true),
            (cl.on, c::MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE, true),
        ];
        for (tipos, codigo, aceita_mixin) in grupos_de_resolucao {
            let fora_do_on = codigo != c::MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE;
            for &t in tipos {
                let resolvido = l.resolver(u, ast_, t, &cl.params, 0, fora_do_on);
                // `NamedTypeResolver._verifyNullability`: `?` escrito num tipo
                // de cláusula (o nó inteiro).
                if l.anulavel(u, ast_, t, 0) && matches!(resolvido, Resolvido::Classe(_)) {
                    let codigo_nulo = if codigo == c::IMPLEMENTS_NON_CLASS {
                        c::NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE
                    } else if codigo == c::MIXIN_OF_NON_CLASS {
                        c::NULLABLE_TYPE_IN_WITH_CLAUSE
                    } else if codigo == c::MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE {
                        c::NULLABLE_TYPE_IN_ON_CLAUSE
                    } else {
                        c::NULLABLE_TYPE_IN_EXTENDS_CLAUSE
                    };
                    saida.push((decl.unit, Diagnostic::com_codigo(codigo_nulo, ast_.ty(t).span, [] as [&str; 0])));
                }
                // `_verifyTypeAliasForContext`: o código da cláusula, e o tipo
                // vira `InvalidType` (nada de `*_non_class`).
                if resolvido == Resolvido::AliasDeParametro {
                    let codigo_alias = if codigo == c::IMPLEMENTS_NON_CLASS {
                        c::IMPLEMENTS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER
                    } else if codigo == c::MIXIN_OF_NON_CLASS {
                        c::MIXIN_OF_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER
                    } else if codigo == c::MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE {
                        c::MIXIN_ON_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER
                    } else {
                        c::EXTENDS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER
                    };
                    if let TypeKind::Named { name, .. } = &ast_.ty(t).kind
                        && let (Some(p), Some(n)) = (name.first(), name.last())
                    {
                        let span = dartforge_diagnostics::Span { start: p.span.start, end: n.span.end };
                        saida.push((decl.unit, Diagnostic::com_codigo(codigo_alias, span, [] as [&str; 0])));
                    }
                    continue;
                }
                let erro = match resolvido {
                    Resolvido::Ignorar | Resolvido::AliasDeParametro => false,
                    Resolvido::NaoClasse => true,
                    Resolvido::Classe(x) => match programa.class(x).kind {
                        ClassKind::Class | ClassKind::MixinApplication => false,
                        ClassKind::Mixin => !aceita_mixin,
                        ClassKind::Enum | ClassKind::ExtensionType => true,
                    },
                };
                if erro
                    && let TypeKind::Named { name, .. } = &ast_.ty(t).kind
                    && let (Some(p), Some(n)) = (name.first(), name.last())
                {
                    let span = dartforge_diagnostics::Span { start: p.span.start, end: n.span.end };
                    saida.push((decl.unit, Diagnostic::com_codigo(codigo, span, [] as [&str; 0])));
                }
            }
        }

        // `InheritanceOverrideVerifier._checkDirectSuperTypes`; `direto_erro`
        // é o `true` que ele devolve (e que encerra o `verify()`).
        let mut direto_erro = false;
        if !consumidora.is_sdk {
            let versao = consumidora.features.versao();
            let enums_melhorados = (versao.major, versao.minor) >= (2, 17);
            let pode_ter_enum = classe.kind == ClassKind::Enum
                || classe.kind == ClassKind::Mixin
                || classe.modifiers.abstract_;
            let grupos: [(&[ast::TypeId], Codigo); 4] = [
                (cl.implements, c::IMPLEMENTS_DISALLOWED_CLASS),
                (cl.on, c::MIXIN_SUPER_CLASS_CONSTRAINT_DISALLOWED_CLASS),
                (cl.extends.as_slice(), c::EXTENDS_DISALLOWED_CLASS),
                (cl.with, c::MIXIN_OF_DISALLOWED_CLASS),
            ];
            for (tipos, codigo) in grupos {
                for &t in tipos {
                    let Alvo::Classe(alvo) = l.alvo(u, ast_, t, 0) else {
                        continue;
                    };
                    let span = ast_.ty(t).span;
                    if l.e_enum_do_core(alvo) && enums_melhorados {
                        if !pode_ter_enum {
                            direto_erro = true;
                            saida.push((
                                decl.unit,
                                Diagnostic::com_codigo(
                                    c::CONCRETE_CLASS_HAS_ENUM_SUPERINTERFACE,
                                    span,
                                    [] as [&str; 0],
                                ),
                            ));
                        }
                        continue;
                    }
                    if l.proibida(alvo) {
                        direto_erro = true;
                    }
                    if l.proibida(alvo)
                        && let Some(texto) = l.exibir(u, ast_, t, &cl.params)
                    {
                        saida.push((
                            decl.unit,
                            Diagnostic::com_codigo(codigo, span, [texto.as_str()]),
                        ));
                    }
                }
            }
        }

        // `_checkMixinOfEnum` (`inheritance_override.dart:742-763`): mixin com
        // campo de instância num enum, no tipo inteiro.
        if classe.kind == ClassKind::Enum {
            for &t in cl.with {
                if let Alvo::Classe(m) = l.alvo(u, ast_, t, 0)
                    && l.tem_campo_de_instancia(m)
                {
                    saida.push((decl.unit, Diagnostic::com_codigo(c::ENUM_MIXIN_WITH_INSTANCE_VARIABLE, ast_.ty(t).span, [] as [&str; 0])));
                    direto_erro = true;
                }
            }
        }

        // `InheritanceOverrideVerifier.verify`: supertipo de si mesmo. Com a
        // declaração repetida, o homônimo conta como a própria classe
        // (`ciclo`, T1.1 c).
        if !direto_erro && !ast_.decl(decl.decl).augment {
            let mut passos = 0;
            let mut caminho = Vec::new();
            if let Some(ciclo) = ciclo(&l, id, id, &mut caminho, &mut passos) {
                let nome = l.nome(classe.name);
                let span = match &ast_.decl(decl.decl).kind {
                    DeclKind::Class(d) => d.name.span,
                    DeclKind::Enum(d) => d.name.span,
                    DeclKind::Mixin(d) => d.name.span,
                    _ => ast_.decl(decl.decl).span,
                };
                let d = if ciclo.len() > 1 {
                    let mut texto: Vec<&str> = ciclo.iter().map(|&x| l.nome(programa.class(x).name)).collect();
                    texto.push(nome);
                    let antigo = texto.join(", ");
                    // O terceiro argumento é o caminho como o 3.13.4 o
                    // escreve (T2, caso c21); só a variante 3.13 o usa.
                    match componente_3_13(&l, lib, id) {
                        Some(componente) => {
                            let novo = componente.iter().map(|&x| l.nome(programa.class(x).name)).collect::<Vec<&str>>().join(", ");
                            Diagnostic::com_codigo(c::RECURSIVE_INTERFACE_INHERITANCE, span, [nome, antigo.as_str(), novo.as_str()])
                        }
                        None => Diagnostic::com_codigo(c::RECURSIVE_INTERFACE_INHERITANCE, span, [nome, antigo.as_str()]),
                    }
                } else {
                    let (sup, mixins, on, _) = diretos(&l, id);
                    let codigo = if sup == Some(id) {
                        c::RECURSIVE_INTERFACE_INHERITANCE_EXTENDS
                    } else if on.contains(&id) {
                        c::RECURSIVE_INTERFACE_INHERITANCE_ON
                    } else if mixins.contains(&id) {
                        c::RECURSIVE_INTERFACE_INHERITANCE_WITH
                    } else {
                        c::RECURSIVE_INTERFACE_INHERITANCE_IMPLEMENTS
                    };
                    Diagnostic::com_codigo(codigo, span, [nome])
                };
                saida.push((decl.unit, d));
            }
        }

        // `ErrorVerifier._checkForMixinClassErrorCodes`: `mixin class`.
        if matches!(classe.kind, ClassKind::Class | ClassKind::MixinApplication)
            && classe.modifiers.mixin
        {
            saida.extend(
                classe_mixin(&l, u, id, ast_, &programa.unit(decl.unit).source)
                    .into_iter()
                    .map(|d| (decl.unit, d)),
            );
        }

        // `ErrorVerifier._checkClassInheritance` / `_checkMixinInheritance`.
        let tem_clausula = cl.extends.is_some()
            || !cl.with.is_empty()
            || !cl.implements.is_empty()
            || !cl.on.is_empty();
        if !tem_clausula {
            continue;
        }
        let Some(r) = avaliar(&l, lib, id) else {
            continue;
        };
        saida.extend(r.relatos.into_iter().map(|d| (decl.unit, d)));
        if r.porta != Porta::Aberta {
            continue;
        }
        if classe.kind == ClassKind::Mixin {
            // `_checkMixinInheritance`, depois da porta.
            repetidos(&l, u, ast_, cl.on, c::ON_REPEATED, &mut saida);
            repetidos(&l, u, ast_, cl.implements, c::IMPLEMENTS_REPEATED, &mut saida);
            continue;
        }
        // `_checkClassInheritance`, depois da porta: `implements` repetido,
        // e a superclasse de novo em `implements` ou `with`.
        repetidos(&l, u, ast_, cl.implements, c::IMPLEMENTS_REPEATED, &mut saida);
        // `_checkForNoDefaultSuperConstructorImplicit`, com a porta aberta.
        if classe.kind == ClassKind::Class
            && let DeclKind::Class(x) = &ast_.decl(decl.decl).kind
            && !x.mixin_application
            && !x.members.iter().any(|&m| matches!(ast_.member(m).kind, ast::MemberKind::Constructor(_)))
            && let Some(t) = cl.extends
            && let Some(d) = super_implicito_da_classe(&l, u, ast_, classe.name, x.name.span, t, &cl.params)
        {
            saida.push((decl.unit, d));
        }
        let superclasse = match (classe.kind, cl.extends) {
            (ClassKind::Enum, _) => l.do_core("Enum"),
            (_, None) => l.do_core("Object"),
            (_, Some(t)) => match l.alvo(u, ast_, t, 0) {
                Alvo::Classe(s) => Some(s),
                _ => None,
            },
        };
        if let Some(s) = superclasse {
            for (tipos, codigo) in [(cl.implements, c::IMPLEMENTS_SUPER_CLASS), (cl.with, c::MIXINS_SUPER_CLASS)] {
                for &t in tipos {
                    if l.alvo(u, ast_, t, 0) == Alvo::Classe(s)
                        && let Some(texto) = l.exibir_classe(s)
                    {
                        saida.push((decl.unit, Diagnostic::com_codigo(codigo, ast_.ty(t).span, [texto.as_str()])));
                    }
                }
            }
        }
        // Depois da porta: `extends` adiado e `class_used_as_mixin`.
        if let Some(t) = cl.extends
            && l.adiado(u, ast_, t)
        {
            saida.push((
                decl.unit,
                Diagnostic::com_codigo(c::EXTENDS_DEFERRED_CLASS, ast_.ty(t).span, [] as [&str; 0]),
            ));
        }
        for &t in cl.with {
            let Alvo::Classe(m) = l.alvo(u, ast_, t, 0) else {
                continue;
            };
            let alvo = programa.class(m);
            // `ClassElementImpl`: classe ou alias de classe (`class B = A with M`).
            if !matches!(alvo.kind, ClassKind::Class | ClassKind::MixinApplication)
                || alvo.modifiers.mixin
            {
                continue;
            }
            let origem = programa.library(alvo.library);
            if origem.features.versao().major < 3 {
                continue;
            }
            // `_mayIgnoreClassModifiers`.
            if origem.is_sdk
                && origem.uri != "dart:ffi"
                && (consumidora.is_sdk || consumidora.features.versao().major < 3)
            {
                continue;
            }
            saida.push((
                decl.unit,
                Diagnostic::com_codigo(
                    c::CLASS_USED_AS_MIXIN,
                    ast_.ty(t).span,
                    [nomes.resolve(alvo.name)],
                ),
            ));
        }
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_elements::{load::load_lenient, sdk::SdkLayout};
    use std::fs;
    use std::path::PathBuf;

    /// Um SDK mínimo com as classes proibidas do `dart:core` e o `FutureOr`.
    fn sdk(raiz: &std::path::Path) -> SdkLayout {
        let _ = fs::remove_dir_all(raiz);
        fs::create_dir_all(raiz.join("sdk/lib/core")).unwrap();
        fs::create_dir_all(raiz.join("sdk/lib/async")).unwrap();
        fs::write(
            raiz.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]},"async":{"uri":"async/async.dart","patches":[]}}}}"#,
        )
        .unwrap();
        fs::write(
            raiz.join("sdk/lib/core/core.dart"),
            "class Object {} class int {} class String {} class Null {} abstract interface class Enum {} abstract interface class Comparable<T> {}",
        )
        .unwrap();
        fs::write(
            raiz.join("sdk/lib/async/async.dart"),
            "abstract class FutureOr<T> {}",
        )
        .unwrap();
        SdkLayout::load(&raiz.join("sdk/lib"), "dartdevc").unwrap()
    }

    /// `(código, trecho, mensagem)` de cada diagnóstico de `fonte`.
    fn rodar(nome: &str, fonte: &str) -> Vec<(String, String, String)> {
        let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/clausulas-{nome}-{}",
            std::process::id()
        ));
        let sdk = sdk(&raiz);
        let entrada = raiz.join("main.dart");
        fs::write(&entrada, fonte).unwrap();
        let mut nomes = Interner::new();
        let (programa, _) = load_lenient(&entrada, &sdk, None, &mut nomes);
        let mut v: Vec<(String, String, String)> =
            verificar(&programa, programa.entry.unwrap(), &nomes)
                .into_iter()
                .map(|(_, d)| {
                    (
                        d.code.map_or("", |c| c.info().nome).to_string(),
                        fonte[d.span.start..d.span.end].to_string(),
                        d.message,
                    )
                })
                .collect();
        v.sort();
        fs::remove_dir_all(&raiz).unwrap();
        v
    }

    fn corpus(caminho: &str) -> String {
        fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus/diagnosticos/analyzer")
                .join(caminho),
        )
        .unwrap()
    }

    /// Os casos de `class_used_as_mixin` do corpus, com o que o analyzer
    /// 3.6.2 relata (`oraculo.jsonl`): uma classe com construtor escrito dá
    /// `mixin_class_declares_constructor`, que fecha a porta.
    #[test]
    fn classe_como_mixin_segue_o_oraculo() {
        let casos: [(&str, Option<(&str, &str)>); 6] = [
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside.dart",
                Some(("class_used_as_mixin", "Foo")),
            ),
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside_class_hasGener_5838f665.dart",
                Some(("mixin_class_declares_constructor", "A")),
            ),
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside_enum_hasGenera_724bb742.dart",
                Some(("mixin_class_declares_constructor", "A")),
            ),
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside_classTypeAlias_69274ad9.dart",
                Some(("mixin_class_declares_constructor", "A")),
            ),
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside_mixinClass.dart",
                None,
            ),
            // `Enum` é proibido para a porta, e classe abstrata pode tê-lo.
            (
                "class_used_as_mixin/ClassUsedAsMixin__coreLib_dartCoreEnum.dart",
                None,
            ),
        ];
        for (i, (arquivo, esperado)) in casos.into_iter().enumerate() {
            let v = rodar(&format!("mixin{i}"), &corpus(arquivo));
            match esperado {
                Some((codigo, trecho)) => {
                    assert_eq!(v.len(), 1, "{arquivo}: {v:?}");
                    assert_eq!(
                        (v[0].0.as_str(), v[0].1.as_str()),
                        (codigo, trecho),
                        "{arquivo}"
                    );
                }
                None => assert!(v.is_empty(), "{arquivo}: {v:?}"),
            }
        }
        // Antes dos modificadores de classe (`// @dart=2.19`), nada.
        let legado =
            corpus("class_used_as_mixin/ClassUsedAsMixin__inside_beforeClassModifiers.dart")
                .replace(
                    "// %before-language-feature: class-modifiers",
                    "// @dart=2.19",
                );
        assert!(rodar("antes-dos-modificadores", &legado).is_empty());
    }

    #[test]
    fn classe_legada_importada_pode_ser_usada_como_mixin_em_codigo_atual() {
        let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/clausulas-legado-{}",
            std::process::id()
        ));
        let sdk = sdk(&raiz);
        fs::write(raiz.join("legacy.dart"), "// @dart=2.19\nclass Legacy {}\n").unwrap();
        let entrada = raiz.join("main.dart");
        let fonte = "import 'legacy.dart';\nclass Current {}\nclass A with Legacy {}\nclass B with Current {}\n";
        fs::write(&entrada, fonte).unwrap();
        let mut nomes = Interner::new();
        let (programa, _) = load_lenient(&entrada, &sdk, None, &mut nomes);
        let diags = verificar(&programa, programa.entry.unwrap(), &nomes);
        assert_eq!(diags.len(), 1, "{diags:?}");
        let d = &diags[0].1;
        assert_eq!(&fonte[d.span.start..d.span.end], "Current");
        assert_eq!(d.code, Some(c::CLASS_USED_AS_MIXIN));
        fs::remove_dir_all(&raiz).unwrap();
    }

    /// `subtype_of_disallowed_type` com o tipo como o analyzer o exibe: o
    /// alias pelo nome, `FutureOr` cru instanciado aos limites. Conferido
    /// com o `dart analyze` 3.6.2 (as mesmas formas estão em
    /// `extends_disallowed_class`, `mixin_of_disallowed_class` e
    /// `implements_disallowed_class` do corpus).
    #[test]
    fn tipo_proibido_com_alias_e_limites() {
        let fonte = "import 'dart:async';\n\
            typedef T<X> = FutureOr<X>;\n\
            typedef P<X> = X;\n\
            typedef F = FutureOr<int>;\n\
            class M {}\n\
            class A extends int {}\n\
            class B implements F {}\n\
            class C with T<int> {}\n\
            class H with P<int> {}\n\
            class D implements FutureOr {}\n\
            class E<U> extends FutureOr<U> {}\n\
            class G = String with M;\n";
        let v = rodar("proibido", fonte);
        let v: Vec<(&str, &str, &str)> = v
            .iter()
            .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
            .collect();
        // `class G = String with M`: a porta fecha e `class_used_as_mixin` não
        // sai. `with P<int>` é `supertype_expands_to_type_parameter`, não
        // tipo proibido.
        assert_eq!(
            v,
            vec![
                (
                    "subtype_of_disallowed_type",
                    "F",
                    "Classes and mixins can't implement 'F'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "FutureOr",
                    "Classes and mixins can't implement 'FutureOr<dynamic>'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "FutureOr<U>",
                    "Classes can't extend 'FutureOr<U>'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "String",
                    "Classes can't extend 'String'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "T<int>",
                    "Classes can't mixin 'T<int>'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "int",
                    "Classes can't extend 'int'."
                ),
                (
                    "supertype_expands_to_type_parameter",
                    "P",
                    "A type alias that expands to a type parameter can't be mixed in."
                ),
            ]
        );
    }

    /// `Enum` como superinterface: só a classe concreta relata; nenhuma das
    /// três passa pela porta.
    #[test]
    fn enum_como_superinterface() {
        let v = rodar(
            "enum",
            "class M {}\nclass A extends Enum {}\nabstract class B implements Enum {}\nabstract class C extends Enum with M {}\n",
        );
        assert_eq!(
            v,
            vec![(
                "concrete_class_has_enum_superinterface".to_string(),
                "Enum".to_string(),
                "Concrete classes can't have 'Enum' as a superinterface.".to_string()
            )]
        );
    }

    /// `mixin class`: construtor não trivial e supertipo que não é `Object`,
    /// nas posições do `dart analyze` 3.6.2.
    #[test]
    fn classe_mixin_com_construtor_ou_supertipo() {
        let fonte = "mixin M1 {}\nmixin M2 {}\nclass A {}\nmixin class B extends A {}\n\
            mixin class C extends Object with M1 {}\nmixin class D = Object with M1, M2;\nmixin class E = Object with M1;\n\
            mixin class F { F(); F.a() {} F.b(int x); factory F.c() => throw 0; }\n";
        let v = rodar("classe-mixin", fonte);
        let mut v: Vec<(&str, &str)> = v.iter().map(|(a, b, _)| (a.as_str(), b.as_str())).collect();
        v.sort();
        assert_eq!(
            v,
            vec![
                ("mixin_class_declaration_extends_not_object", "A"),
                ("mixin_class_declaration_extends_not_object", "with M1"),
                ("mixin_class_declaration_extends_not_object", "with M1, M2"),
                ("mixin_class_declares_constructor", "F"),
                ("mixin_class_declares_constructor", "F"),
            ]
        );
    }

    /// Restrição `on` satisfeita pela superclasse abre a porta; não
    /// satisfeita, `mixin_application_not_implemented_interface` fecha.
    #[test]
    fn restricao_on_do_mixin_aplicado() {
        let v = rodar(
            "restricao",
            "class S {}\nmixin M on S {}\nclass N {}\nclass A extends S with M, N {}\nclass B with M, N {}\n",
        );
        let v: Vec<(&str, &str)> = v.iter().map(|(a, b, _)| (a.as_str(), b.as_str())).collect();
        assert_eq!(v, vec![("class_used_as_mixin", "N"), ("mixin_application_not_implemented_interface", "M")]);
    }

    /// Um alias de classe com um só mixin não herda de outra classe, e ele
    /// mesmo é uma classe comum usada como mixin (`dart analyze` 3.6.2).
    #[test]
    fn alias_de_classe_como_mixin() {
        let v = rodar(
            "alias",
            "class A {}\nclass B = Object with A;\nclass C extends Object with B {}\n",
        );
        let v: Vec<(&str, &str)> = v.iter().map(|(a, b, _)| (a.as_str(), b.as_str())).collect();
        assert_eq!(
            v,
            vec![("class_used_as_mixin", "A"), ("class_used_as_mixin", "B")]
        );
    }

    /// `mixin_inherits_from_not_object` fecha a porta (o `class_used_as_mixin`
    /// do mesmo tipo não sai), e a superclasse só com `factory`.
    #[test]
    fn mixin_que_herda_e_superclasse_so_factory() {
        let fonte = "class S {}\nmixin class N {}\nclass A extends S {}\nclass B extends Object with N {}\n\
            class C with A {}\nclass D with B {}\nclass F { factory F() => throw 0; }\nclass G extends F {}\n";
        let v = rodar("herda", fonte);
        let v: Vec<(&str, &str)> = v.iter().map(|(a, b, _)| (a.as_str(), b.as_str())).collect();
        assert_eq!(
            v,
            vec![
                ("mixin_inherits_from_not_object", "A"),
                ("mixin_inherits_from_not_object", "B"),
                ("no_generative_constructors_in_superclass", "F"),
            ]
        );
    }

    /// Cláusulas repetidas e a superclasse de novo em `implements`/`with`
    /// (`implements_repeated`, `on_repeated`, `implements_super_class`), nas
    /// formas do corpus (oráculo 3.6.2): o nome da classe e o elemento
    /// exibido (`class A`, `mixin class A`, `class Object`).
    #[test]
    fn repetidos_e_superclasse() {
        let v = rodar(
            "repetidos",
            "class A {}\nmixin class B {}\ntypedef TA = A;\nclass C extends A implements A, TA {}\nclass D extends B with B {}\nclass E implements Object {}\nmixin M on A, A implements B, B {}\nenum F implements A, A { v }\nextension type X(int it) implements int, int {}\n",
        );
        let esperado = [
            ("implements_repeated", "A", "'A' can only be implemented once."),
            ("implements_repeated", "B", "'B' can only be implemented once."),
            ("implements_repeated", "TA", "'A' can only be implemented once."),
            ("implements_repeated", "int", "'int' can only be implemented once."),
            ("implements_super_class", "A", "'class A' can't be used in both the 'extends' and 'implements' clauses."),
            ("implements_super_class", "B", "'mixin class B' can't be used in both the 'extends' and 'with' clauses."),
            ("implements_super_class", "Object", "'class Object' can't be used in both the 'extends' and 'implements' clauses."),
            ("implements_super_class", "TA", "'class A' can't be used in both the 'extends' and 'implements' clauses."),
            ("on_repeated", "A", "The type 'A' can be included in the superclass constraints only once."),
        ];
        let esperado: Vec<(String, String, String)> =
            esperado.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect();
        assert_eq!(v, esperado);
    }

    /// Tipos de cláusula que não nomeiam classe (`ResolutionVisitor._resolveType`):
    /// variável, nome indefinido, enum, parâmetro de tipo, `dynamic`; um nome
    /// de import inexistente com prefixo é ignorado (`shouldIgnoreUndefined`).
    #[test]
    fn clausula_que_nao_nomeia_classe() {
        let v = rodar(
            "nao-classe",
            "import 'nao_existe.dart' as p;\nint v = 0;\nenum En { a }\nmixin M {}\nclass A extends v {}\nclass B extends Indefinida {}\nclass C<T> implements T, En {}\nclass D extends M {}\nclass E with dynamic {}\nclass F extends p.X {}\nclass G = v with M;\nclass H extends v with M {}\n",
        );
        let esperado = [
            ("extends_non_class", "Indefinida", "Classes can only extend other classes."),
            ("extends_non_class", "M", "Classes can only extend other classes."),
            ("extends_non_class", "v", "Classes can only extend other classes."),
            ("implements_non_class", "En", "Classes and mixins can only implement other classes and mixins."),
            ("implements_non_class", "T", "Classes and mixins can only implement other classes and mixins."),
            ("mixin_of_non_class", "dynamic", "Classes can only mix in mixins and classes."),
            ("mixin_with_non_class_superclass", "v", "Mixin can only be applied to class."),
            ("mixin_with_non_class_superclass", "v", "Mixin can only be applied to class."),
        ];
        let esperado: Vec<(String, String, String)> =
            esperado.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect();
        assert_eq!(v, esperado);
    }

    /// `_checkForRecursiveInterfaceInheritance`: o caminho da busca em
    /// profundidade (corpus `recursive_interface_inheritance`, oráculo 3.6.2)
    /// e os casos de base.
    #[test]
    fn supertipo_de_si_mesmo() {
        let v = rodar(
            "ciclo",
            "class A extends B {}\nclass B implements C {}\nclass C extends A {}\nclass D extends B {}\nclass E extends E {\n  var foo = 0;\n}\nmixin M on M {}\nclass F with F {}\nclass G implements G {}\n",
        );
        let esperado = [
            ("recursive_interface_inheritance", "A", "'A' can't be a superinterface of itself: A, B, C, A."),
            ("recursive_interface_inheritance", "B", "'B' can't be a superinterface of itself: B, C, A, B."),
            ("recursive_interface_inheritance", "C", "'C' can't be a superinterface of itself: C, A, B, C."),
            ("recursive_interface_inheritance", "E", "'E' can't extend itself."),
            ("recursive_interface_inheritance", "F", "'F' can't use itself as a mixin."),
            ("recursive_interface_inheritance", "G", "'G' can't implement itself."),
            ("recursive_interface_inheritance", "M", "'M' can't use itself as a superclass constraint."),
        ];
        let esperado: Vec<(String, String, String)> =
            esperado.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect();
        let v: Vec<_> = v.into_iter().filter(|x| x.0 == "recursive_interface_inheritance").collect();
        assert_eq!(v, esperado);
    }

    /// `_verifyNullability` nos tipos de cláusula: `?` escrito ou vindo de
    /// um alias (corpus `nullable_type_in_*_clause`, oráculo 3.6.2).
    #[test]
    fn tipo_anulavel_em_clausula() {
        let v = rodar(
            "anulavel",
            "class A {}\nmixin M {}\ntypedef B = A?;\nclass C extends A? {}\nclass D implements B {}\nclass E with M? {}\nmixin N on A? {}\n",
        );
        let v: Vec<(&str, &str)> = v.iter().map(|(c, t, _)| (c.as_str(), t.as_str())).collect();
        assert_eq!(
            v,
            vec![
                ("nullable_type_in_extends_clause", "A?"),
                ("nullable_type_in_implements_clause", "B"),
                ("nullable_type_in_on_clause", "A?"),
                ("nullable_type_in_with_clause", "M?"),
            ]
        );
    }

    /// `mixin_application_not_implemented_interface` (corpus, oráculo 3.6.2):
    /// no nome do mixin, com o supertipo (`Object` omitido) e a restrição.
    #[test]
    fn mixin_sem_a_restricao_on() {
        let v = rodar("restricao-on", "class A {}\nclass C {}\nmixin M on A {}\nclass B with M {}\nclass D extends C with M {}\nclass E extends A with M {}\n");
        let v: Vec<(&str, &str)> = v.iter().filter(|x| x.0 == "mixin_application_not_implemented_interface").map(|x| (x.1.as_str(), x.2.as_str())).collect();
        assert_eq!(
            v,
            vec![
                ("M", "'M' can't be mixed onto 'C' because 'C' doesn't implement 'A'."),
                ("M", "'M' can't be mixed onto 'Object' because 'Object' doesn't implement 'A'."),
            ]
        );
    }
}
