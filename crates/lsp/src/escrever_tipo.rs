//! O `DartEditBuilderImpl.writeType`/`_writeType` do `analyzer_plugin` do
//! Dart 3.6.2 (`change_builder_dart.dart:986-1011`, `:1396-1503`,
//! `:1553-1675`, `:1811-1850`; docs/LSP-ESPECIFICACAO.md §13.7.4.0, "Tipos"):
//! o texto de um tipo no ponto de inserção, com o prefixo de import e as
//! bibliotecas a importar.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::refatoracoes::Contexto;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, LibraryId};
use dartforge_types::table::Exibicao;
use dartforge_types::{Type, TypeId, TypeParamOwner};
use std::collections::BTreeSet;

/// O escritor de tipos de um ponto da unidade.
pub(crate) struct Escritor<'a, 'p> {
    consulta: &'a crate::consulta::Consulta,
    _p: std::marker::PhantomData<&'p ()>,
    biblioteca: LibraryId,
    /// `_EnclosingElementFinder`: a `ClassDeclaration` e o executável mais
    /// externos que contêm o ponto.
    classe: Option<ClassId>,
    executavel: Option<FunctionElementId>,
    /// `methodBeingCopied`: o executável cujos parâmetros de tipo são
    /// copiados junto (a sobrescrita do completar).
    pub(crate) copiado: Option<FunctionElementId>,
    /// As bibliotecas que o `_writeLibraryReference` agendou para importar.
    pub(crate) importar: BTreeSet<LibraryId>,
}

impl<'a, 'p> Escritor<'a, 'p> {
    pub(crate) fn novo(cx: &'a Contexto<'p>, offset: usize) -> Escritor<'a, 'p> {
        let prog = cx.p.programa();
        let biblioteca = prog.unit(cx.unidade).library;
        let mut classe = None;
        let mut executavel = None;
        if let Some(n) = cx.arvore.localizar_exclusivo(offset) {
            let mut atual = Some(n);
            while let Some(k) = atual {
                match (cx.especie(k), cx.arvore.nos[k].marca) {
                    ("ClassDeclaration", crate::arvore_analyzer::Marca::Decl(d)) => classe = cx.classe_da_declaracao(cx.unidade, d),
                    ("MethodDeclaration" | "FunctionDeclaration", crate::arvore_analyzer::Marca::Funcao(fid)) => {
                        executavel = cx.p.funcao_do_no(cx.unidade, fid);
                    }
                    ("ConstructorDeclaration", _) => {
                        let s = cx.arvore.span(k);
                        executavel = cx
                            .ast
                            .members
                            .iter()
                            .enumerate()
                            .find(|(_, m)| matches!(m.kind, dartforge_frontend::ast::MemberKind::Constructor(_)) && m.span.start >= s.start && m.span.end <= s.end)
                            .and_then(|(mi, _)| cx.p.construtor_do_no(cx.unidade, dartforge_frontend::ast::MemberId(mi as u32)));
                    }
                    _ => {}
                }
                atual = cx.pai(k);
            }
        }
        Escritor { consulta: &cx.p.consulta, _p: std::marker::PhantomData, biblioteca, classe, executavel, copiado: None, importar: BTreeSet::new() }
    }

    /// O escritor sobre a consulta do completar: a biblioteca do pedido, a
    /// classe e o executável que contêm o ponto (`_EnclosingElementFinder`).
    pub(crate) fn da_consulta(
        consulta: &'a crate::consulta::Consulta,
        biblioteca: LibraryId,
        classe: Option<ClassId>,
        executavel: Option<FunctionElementId>,
    ) -> Escritor<'a, 'p> {
        Escritor { consulta, _p: std::marker::PhantomData, biblioteca, classe, executavel, copiado: None, importar: BTreeSet::new() }
    }

    fn tabela(&self) -> &dartforge_types::TypeTable {
        &self.consulta.tabela
    }

    /// `_getVisibleType(type) != null`.
    fn visivel(&self, t: TypeId) -> bool {
        let prog = &self.consulta.programa;
        match self.tabela().get(t) {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => {
                let c = prog.class(*class);
                !(self.consulta.nome(c.name).starts_with('_') && c.library != self.biblioteca)
            }
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => match self.tabela().param(*param).owner {
                TypeParamOwner::Class(c) => self.classe == Some(c),
                TypeParamOwner::Function(f) => self.executavel == Some(f) || self.copiado == Some(f),
                TypeParamOwner::GenericFunctionType => true,
                _ => false,
            },
            _ => true,
        }
    }

    /// `writeType(type, required: obrigatorio)`: `None` quando nada é
    /// escrito (com `obrigatorio`, `var`).
    pub(crate) fn escrever_tipo(&mut self, t: Option<TypeId>, obrigatorio: bool) -> Option<String> {
        let escrito = match t {
            Some(t) if !matches!(self.tabela().get(t), Type::Dynamic) => self.escrever(t, false),
            _ => None,
        };
        match escrito {
            Some(s) => Some(s),
            None if obrigatorio => Some("var".to_string()),
            None => None,
        }
    }

    /// `_writeType(type, required)`.
    pub(crate) fn escrever(&mut self, t: TypeId, obrigatorio: bool) -> Option<String> {
        if !self.visivel(t) {
            return None;
        }
        let exibicao = self.tabela().exibicao(t).cloned();
        if matches!(exibicao, Some(Exibicao::Invalido)) || matches!(self.tabela().get(t), Type::Dynamic) {
            return obrigatorio.then(|| "dynamic".to_string());
        }
        if matches!(self.tabela().get(t), Type::Never) && !matches!(exibicao, Some(Exibicao::NeverAnulavel)) {
            return Some("Never".to_string());
        }
        let anulavel = self.tabela().get(t).is_declared_nullable();
        let sufixo = if anulavel { "?" } else { "" };
        if let Some(Exibicao::Alias { typedef, args }) = exibicao {
            let s = self.elemento_com_argumentos(Element::Typedef(typedef), &args);
            return Some(format!("{s}{sufixo}"));
        }
        if matches!(exibicao, Some(Exibicao::NeverAnulavel)) {
            return Some("Never?".to_string());
        }
        let p = self.consulta;
        match self.tabela().get(t).clone() {
            Type::Function { type_params, ret, positional, optional, named, nullable } => {
                let mut s = String::new();
                if let Some(r) = self.escrever(ret, false) {
                    s.push_str(&r);
                    s.push(' ');
                }
                s.push_str("Function");
                if !type_params.is_empty() {
                    s.push('<');
                    for (i, tp) in type_params.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        let dados = self.tabela().param(*tp);
                        let (nome, limite, explicito) = (dados.name, dados.bound, dados.explicito);
                        s.push_str(p.nome(nome));
                        if explicito && let Some(b) = self.escrever(limite, false) {
                            s.push_str(" extends ");
                            s.push_str(&b);
                        }
                    }
                    s.push('>');
                }
                // `writeParameters(includeDefaultValues: false,
                // requiredTypes: true)`; os posicionais sem nome viram `p1`…
                let mut usados: BTreeSet<String> = named.iter().map(|(n, _, _)| p.nome(*n).to_string()).collect();
                s.push('(');
                let mut i = 0usize;
                let mut abriu_opcional = false;
                let mut abriu_nomeado = false;
                let gerar = |usados: &mut BTreeSet<String>| {
                    let mut k = 1;
                    while usados.contains(&format!("p{k}")) {
                        k += 1;
                    }
                    let n = format!("p{k}");
                    usados.insert(n.clone());
                    n
                };
                for &q in positional.iter() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    let nome = gerar(&mut usados);
                    let tipo = self.escrever(q, true);
                    s.push_str(&parametro(tipo, &nome, false));
                    i += 1;
                }
                for &q in optional.iter() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    if !abriu_opcional {
                        s.push('[');
                        abriu_opcional = true;
                    }
                    let nome = gerar(&mut usados);
                    let tipo = self.escrever(q, true);
                    s.push_str(&parametro(tipo, &nome, false));
                    i += 1;
                }
                for (n, q, requerido) in named.iter() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    if !abriu_nomeado {
                        s.push('{');
                        abriu_nomeado = true;
                    }
                    let tipo = self.escrever(*q, true);
                    s.push_str(&parametro(tipo, p.nome(*n), *requerido));
                    i += 1;
                }
                if abriu_nomeado {
                    s.push('}');
                }
                if abriu_opcional {
                    s.push(']');
                }
                s.push(')');
                if nullable {
                    s.push('?');
                }
                Some(s)
            }
            Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => {
                Some(format!("{}{sufixo}", self.elemento_com_argumentos(Element::Class(class), &args)))
            }
            Type::FutureOr { arg, .. } => match self.classe_future_or() {
                Some(c) => Some(format!("{}{sufixo}", self.elemento_com_argumentos(Element::Class(c), &[arg]))),
                None => Some(format!("FutureOr{sufixo}")),
            },
            Type::Null => match p.core.null_class {
                Some(c) => Some(self.elemento_com_argumentos(Element::Class(c), &[])),
                None => Some("Null".to_string()),
            },
            Type::Never => Some(format!("Never{sufixo}")),
            Type::TypeParameter { param, .. } | Type::Intersection { param, .. } => {
                Some(format!("{}{sufixo}", p.nome(self.tabela().param(param).name)))
            }
            Type::Void => Some("void".to_string()),
            Type::Record { positional, named, .. } => {
                let mut s = String::from("(");
                let mut primeiro = true;
                for &q in positional.iter() {
                    if !primeiro {
                        s.push_str(", ");
                    }
                    primeiro = false;
                    s.push_str(&self.escrever(q, false).unwrap_or_default());
                }
                if !named.is_empty() {
                    s.push_str(if primeiro { "{" } else { ", {" });
                    let mut primeiro_nomeado = true;
                    let mut nomeados: Vec<(String, TypeId)> = named.iter().map(|(n, q)| (p.nome(*n).to_string(), *q)).collect();
                    nomeados.sort_by(|a, b| a.0.cmp(&b.0));
                    for (n, q) in nomeados {
                        if !primeiro_nomeado {
                            s.push_str(", ");
                        }
                        primeiro_nomeado = false;
                        s.push_str(&self.escrever(q, false).unwrap_or_default());
                        s.push(' ');
                        s.push_str(&n);
                    }
                    s.push('}');
                }
                s.push(')');
                s.push_str(sufixo);
                Some(s)
            }
            Type::Dynamic => obrigatorio.then(|| "dynamic".to_string()),
        }
    }

    /// `writeType(typeProvider.futureType(x))` com `x` já achatado
    /// (`anulavel`: o `flatten` de um `FutureOr<X>?`/`Future<X>?` é `X?`).
    pub(crate) fn escrever_future(&mut self, x: TypeId, anulavel: bool) -> Option<String> {
        let futuro = self.consulta.core.future_class?;
        let mut s = self.referencia(Element::Class(futuro));
        s.push_str("Future");
        let dinamico = matches!(self.tabela().get(x), Type::Dynamic);
        if !dinamico && self.visivel(x) {
            let mut interno = self.escrever(x, true).unwrap_or_default();
            let ja = self.tabela().get(x).is_declared_nullable() || matches!(self.tabela().get(x), Type::Void | Type::Null);
            if anulavel && !ja {
                interno.push('?');
            }
            s.push('<');
            s.push_str(&interno);
            s.push('>');
        }
        Some(s)
    }

    /// `_writeTypeElementArguments`: o prefixo, o nome e os argumentos (só
    /// se algum não é `dynamic` e todos são visíveis).
    fn elemento_com_argumentos(&mut self, el: Element, args: &[TypeId]) -> String {
        let mut s = self.referencia(el);
        let p = self.consulta;
        let prog = &p.programa;
        s.push_str(match el {
            Element::Class(c) => p.nome(prog.class(c).name),
            Element::Typedef(t) => p.nome(prog.typedef(t).name),
            _ => "",
        });
        if !args.is_empty() {
            let algum = args.iter().any(|a| !matches!(self.tabela().get(*a), Type::Dynamic));
            let todos_visiveis = args.iter().all(|a| self.visivel(*a));
            if algum && todos_visiveis {
                s.push('<');
                for (i, &a) in args.iter().enumerate() {
                    if i != 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&self.escrever(a, true).unwrap_or_default());
                }
                s.push('>');
            }
        }
        s
    }

    /// `_writeLibraryReference`: o prefixo do primeiro import (o implícito
    /// de `dart:core` por último) cujo namespace tem o elemento; sem import,
    /// a biblioteca do elemento é agendada (sem prefixo).
    fn referencia(&mut self, el: Element) -> String {
        let p = self.consulta;
        let prog = &p.programa;
        let (lib, nome) = match el {
            Element::Class(c) => (prog.class(c).library, prog.class(c).name),
            Element::Typedef(t) => (prog.typedef(t).library, prog.typedef(t).name),
            Element::Extension(x) => match prog.extension(x).name {
                Some(n) => (prog.extension(x).library, n),
                None => return String::new(),
            },
            _ => return String::new(),
        };
        if lib == self.biblioteca {
            return String::new();
        }
        let atual = prog.library(self.biblioteca);
        let definidora = atual.units.first().copied();
        let no_namespace = |alvo: LibraryId, combinadores: &[dartforge_frontend::ast::Combinator]| {
            prog.library(alvo).exported.get(&nome).and_then(|b| b.getter) == Some(el)
                && combinadores.iter().all(|c| match c {
                    dartforge_frontend::ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == nome),
                    dartforge_frontend::ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == nome),
                })
        };
        let mut core_explicito = false;
        for i in atual.imports.iter().filter(|i| Some(i.unit) == definidora) {
            if Some(i.library) == p.core.core_library {
                core_explicito = true;
            }
            if no_namespace(i.library, &i.combinators) {
                return i.prefix.map(|x| format!("{}.", p.nome(x))).unwrap_or_default();
            }
        }
        if !core_explicito
            && let Some(core) = p.core.core_library
            && no_namespace(core, &[])
        {
            return String::new();
        }
        self.importar.insert(lib);
        String::new()
    }

    /// `writeReference(element)`: o prefixo e o nome.
    pub(crate) fn referencia_de(&mut self, el: Element, nome: &str) -> String {
        format!("{}{nome}", self.referencia(el))
    }

    /// A classe `FutureOr` de `dart:async`.
    fn classe_future_or(&self) -> Option<ClassId> {
        let p = self.consulta;
        let prog = &p.programa;
        let lib = p.core.async_library?;
        prog.library(lib).exported.iter().find(|(s, _)| p.nome(**s) == "FutureOr").and_then(|(_, b)| match b.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        })
    }
}

/// `writeParameter(name, type, isRequiredType: true)` sem default.
fn parametro(tipo: Option<String>, nome: &str, requerido: bool) -> String {
    let mut s = String::new();
    if requerido {
        s.push_str("required ");
    }
    if let Some(t) = &tipo {
        s.push_str(t);
        if !nome.is_empty() {
            s.push(' ');
        }
    }
    s.push_str(nome);
    s
}
