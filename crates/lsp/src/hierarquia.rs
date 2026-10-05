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
//! * hierarquia de tipos — supertipos diretos (superclasse, mixins,
//!   interfaces, `on`) e subtipos diretos.

use crate::projeto::{Alvo, Concreto, Dono, Projeto};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind, UnitId};
use dartforge_types::Type;
use std::collections::BTreeSet;

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

    /// A classe que `offset` denota (nome da declaração ou referência), para
    /// a hierarquia de tipos.
    pub(crate) fn classe_denotada(&self, unidade: UnitId, offset: usize) -> Option<ClassId> {
        let d = self.identificar(unidade, offset).ok()??;
        match (&d.alvo, d.concreto) {
            (Alvo::Topo(Element::Class(c)), _) => Some(*c),
            (Alvo::Construtor(f), _) => self.programa().function(*f).class,
            _ => None,
        }
    }

    /// A classe de partida da hierarquia em `offset`: a do nome de tipo sob
    /// o cursor, senão a classe, mixin ou enum que o contém
    /// (`DartLazyTypeHierarchyComputer.findTarget`).
    pub(crate) fn classe_alvo_da_hierarquia(&self, unidade: UnitId, offset: usize) -> Option<ClassId> {
        if let Some(c) = self.classe_denotada(unidade, offset) {
            return Some(c);
        }
        let p = self.programa();
        let ast = &p.unit(unidade).ast;
        let decl = ast
            .decls
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                d.span.start <= offset
                    && offset <= d.span.end
                    && matches!(
                        d.kind,
                        dartforge_frontend::ast::DeclKind::Class(_)
                            | dartforge_frontend::ast::DeclKind::Mixin(_)
                            | dartforge_frontend::ast::DeclKind::Enum(_)
                            | dartforge_frontend::ast::DeclKind::ExtensionType(_)
                    )
            })
            .min_by_key(|(_, d)| d.span.end - d.span.start)?
            .0;
        (0..p.classes.len())
            .map(|i| ClassId(i as u32))
            .find(|c| p.class(*c).decl.is_some_and(|d| d.unit == unidade && d.decl.0 as usize == decl))
    }

    /// O item de `c`: o nome exibido (o dado, ou o nome com os parâmetros de
    /// tipo, como o `thisType`), o arquivo, a declaração e o nome.
    pub(crate) fn item_de_tipo(&self, c: ClassId, nome: Option<String>) -> Option<crate::ItemDeTipo> {
        let p = self.programa();
        let d = p.class(c).decl?;
        let unidade = p.unit(d.unit);
        let decl = unidade.ast.decl(d.decl);
        let (_, selecao) = self.nome_do_elemento_de_topo(Element::Class(c))?;
        let nome = nome.unwrap_or_else(|| {
            let parametros: Vec<&str> = match &decl.kind {
                dartforge_frontend::ast::DeclKind::Class(k) => k.type_params.iter().map(|t| self.nome(t.name.sym)).collect(),
                dartforge_frontend::ast::DeclKind::Mixin(k) => k.type_params.iter().map(|t| self.nome(t.name.sym)).collect(),
                dartforge_frontend::ast::DeclKind::Enum(k) => k.type_params.iter().map(|t| self.nome(t.name.sym)).collect(),
                dartforge_frontend::ast::DeclKind::ExtensionType(k) => k.type_params.iter().map(|t| self.nome(t.name.sym)).collect(),
                _ => Vec::new(),
            };
            let base = self.nome(p.class(c).name).to_string();
            if parametros.is_empty() { base } else { format!("{base}<{}>", parametros.join(", ")) }
        });
        Some(crate::ItemDeTipo {
            nome,
            uri: self.uri_da_unidade(d.unit)?,
            intervalo: decl.span,
            selecao,
        })
    }

    /// Supertipos diretos como o analyzer os lista: a superclasse (`Object`
    /// quando não há `extends`), as restrições `on`, as interfaces e os
    /// mixins, cada um com os argumentos de tipo escritos.
    pub(crate) fn itens_de_supertipos(&self, c: ClassId) -> Vec<crate::ItemDeTipo> {
        let Some(dados) = self.consulta.outline.classes.get(c.0 as usize) else { return Vec::new() };
        let p = self.programa();
        let eh_mixin = p.class(c).decl.is_some_and(|d| matches!(p.unit(d.unit).ast.decl(d.decl).kind, dartforge_frontend::ast::DeclKind::Mixin(_)));
        let mut tipos: Vec<dartforge_types::TypeId> = Vec::new();
        match dados.supertype {
            Some(s) => tipos.push(s),
            None if !eh_mixin && Some(c) != self.consulta.core.object_class => {
                if let Some(o) = self.consulta.core.object_class {
                    return std::iter::once(o)
                        .filter_map(|o| self.item_de_tipo(o, None))
                        .chain(self.itens_de_tipos(dados.on.iter().chain(dados.interfaces.iter()).chain(dados.mixins.iter()).copied()))
                        .collect();
                }
            }
            None => {}
        }
        tipos.extend(dados.on.iter().chain(dados.interfaces.iter()).chain(dados.mixins.iter()).copied());
        self.itens_de_tipos(tipos.into_iter())
    }

    fn itens_de_tipos(&self, tipos: impl Iterator<Item = dartforge_types::TypeId>) -> Vec<crate::ItemDeTipo> {
        tipos
            .filter_map(|t| {
                let classe = match self.consulta.tabela.get(t) {
                    Type::Interface { class, .. } => *class,
                    Type::ExtensionType { decl, .. } => *decl,
                    _ => return None,
                };
                let nome = self.consulta.formatar(t).trim_end_matches('?').to_string();
                self.item_de_tipo(classe, Some(nome))
            })
            .collect()
    }
}
