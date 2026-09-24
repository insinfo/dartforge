//! Aplicações de macro: detecção e ordem (spec de macros, "Application
//! order", `:250-286`; a ordem de execução é a do CFE 3.6.2,
//! `kernel_macro_macro.dart`).
//!
//! **Detecção** (regra de custo zero): uma anotação só é aplicação se o nome
//! dela resolve para um construtor de classe declarada com `macro`
//! ([`Program::classes_macro`]); sem classe `macro` no programa, [`detectar`]
//! volta na primeira linha, sem olhar anotação nenhuma. A aplicação é sempre
//! uma chamada de construtor (`@M()`, `@M.nome(…)`, `@p.M(…)`).
//!
//! **Ordem**: numa declaração, da direita para a esquerda (`@C @B @A` roda
//! `A` primeiro); membros antes do tipo que os contém. Entre declarações, a
//! do CFE 3.6.2, que é o oráculo do texto:
//!
//! * fases de tipos e de definições, por biblioteca: as da diretiva
//!   `library`, depois as de funções e variáveis de topo, depois, por classe
//!   na ordem do fonte, as dos membros (campos e métodos, depois
//!   construtores) e as da classe;
//! * fase de declarações: as classes na ordem da hierarquia (supertipos
//!   antes) — membros, depois a classe —, e só então, por biblioteca, as da
//!   diretiva e as de topo.
use crate::executor::Fase;
use crate::modelo::{Chave, Vista};
use dartforge_diagnostics::Span;
use dartforge_elements::model::*;
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind, MemberKind};
use serde_json::{Value, json};
use std::collections::HashSet;

/// O que recebe a aplicação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Alvo {
    Biblioteca(String),
    Declaracao(Chave),
}

/// Onde a aplicação entra na ordem do CFE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grupo {
    /// Na diretiva `library`.
    Biblioteca,
    /// Numa função ou variável de topo.
    Topo,
    /// Num membro da classe `classe` (`construtor` vai depois dos outros).
    Membro { classe: Chave, construtor: bool },
    /// Na própria classe.
    Classe(Chave),
}

#[derive(Debug, Clone)]
pub struct Aplicacao {
    /// URI da biblioteca da anotação (a que recebe a augmentation).
    pub biblioteca: String,
    pub alvo: Alvo,
    pub grupo: Grupo,
    /// `uri#Classe` da macro.
    pub macro_: String,
    /// Nome do construtor (`""` para o sem nome).
    pub construtor: String,
    /// `{posicionais, nomeados}` no formato de argumento do protocolo.
    pub argumentos: Value,
    /// Unidade (URI) e intervalo da anotação, para diagnósticos.
    pub unidade: String,
    pub span: Span,
}

/// A classe `macro` que uma anotação invoca, e o nome do construtor.
fn macro_da_anotacao(v: &Vista<'_>, lib: LibraryId, a: &ast::Annotation, macros: &HashSet<ClassId>) -> Option<(ClassId, String)> {
    a.arguments.as_ref()?;
    let classe = |el: Option<Element>| match el {
        Some(Element::Class(c)) if macros.contains(&c) => Some(c),
        _ => None,
    };
    let nome = |s: dartforge_intern::SymbolId| v.interner.resolve(s).to_string();
    match a.name.as_slice() {
        [c] => classe(v.program.lookup(lib, c.sym).and_then(|b| b.getter)).map(|c| (c, String::new())),
        [x, y] => {
            if let Some(c) = classe(v.program.lookup(lib, x.sym).and_then(|b| b.getter)) {
                return Some((c, nome(y.sym)));
            }
            classe(v.program.lookup_prefixed(lib, x.sym, y.sym).and_then(|b| b.getter)).map(|c| (c, String::new()))
        }
        [p, c, k] => classe(v.program.lookup_prefixed(lib, p.sym, c.sym).and_then(|b| b.getter)).map(|c| (c, nome(k.sym))),
        _ => None,
    }
}

/// Um argumento de aplicação (spec, "Macro arguments"): literal, coleção de
/// literais; qualquer outra expressão vai como `Code` com o texto dela.
fn argumento(u: &Unit, e: ast::ExprId) -> Value {
    let ex = u.ast.expr(e);
    let texto = |s: Span| u.source.get(s.start..s.end).unwrap_or("").to_string();
    let elementos = |els: &[ast::CollectionElement]| -> Option<Vec<Value>> {
        els.iter()
            .map(|el| match el {
                ast::CollectionElement::Expression(x) => Some(argumento(u, *x)),
                _ => None,
            })
            .collect()
    };
    match &ex.kind {
        ExprKind::Null => json!({"t": "null"}),
        ExprKind::Bool(b) => json!({"t": "bool", "v": b}),
        ExprKind::Int(s) => json!({"t": "int", "v": texto(*s).replace('_', "")}),
        ExprKind::Double(s) => {
            json!({"t": "double", "v": texto(*s).replace('_', "").parse::<f64>().unwrap_or(0.0)})
        }
        ExprKind::String(l) if l.parts.iter().all(|p| matches!(p, ast::StringPart::Text(_))) => {
            let s: String = l
                .parts
                .iter()
                .map(|p| match p {
                    ast::StringPart::Text(t) => t.to_string_lossy(),
                    _ => String::new(),
                })
                .collect();
            json!({"t": "string", "v": s})
        }
        ExprKind::Parenthesized(x) => argumento(u, *x),
        ExprKind::List { elements, .. } => match elementos(elements) {
            Some(l) => json!({"t": "lista", "v": l}),
            None => codigo(texto(ex.span)),
        },
        ExprKind::SetOrMap { elements, .. } => {
            let pares: Option<Vec<Value>> = elements
                .iter()
                .map(|el| match el {
                    ast::CollectionElement::MapEntry { key, value, null_aware_key: false, null_aware_value: false } => {
                        Some(json!([argumento(u, *key), argumento(u, *value)]))
                    }
                    _ => None,
                })
                .collect();
            match pares {
                Some(p) if !p.is_empty() || elements.is_empty() => json!({"t": "mapa", "v": p}),
                _ => match elementos(elements) {
                    Some(l) => json!({"t": "set", "v": l}),
                    None => codigo(texto(ex.span)),
                },
            }
        }
        _ => codigo(texto(ex.span)),
    }
}

fn codigo(texto: String) -> Value {
    json!({"t": "codigo", "v": {"k": "expression", "p": [texto]}})
}

fn argumentos(u: &Unit, a: &ast::Annotation) -> Value {
    let mut pos = Vec::new();
    let mut nom = serde_json::Map::new();
    for arg in a.arguments.iter().flat_map(|x| x.args.iter()) {
        match arg.name {
            Some(n) => {
                let nome = u.source.get(n.span.start..n.span.end).unwrap_or("").to_string();
                nom.insert(nome, argumento(u, arg.value));
            }
            None => pos.push(argumento(u, arg.value)),
        }
    }
    json!({"posicionais": pos, "nomeados": nom})
}

/// As aplicações do programa, na ordem do fonte (a ordem de fase vem de
/// [`ordem`]). Vazio — sem custo — quando não há classe `macro`.
pub fn detectar(v: &Vista<'_>) -> Vec<Aplicacao> {
    if v.program.classes_macro.is_empty() {
        return Vec::new();
    }
    let macros: HashSet<ClassId> = v.program.classes_macro.iter().copied().collect();
    let mut out = Vec::new();
    for (li, lib) in v.program.libraries.iter().enumerate() {
        if lib.is_sdk {
            continue;
        }
        let lib_id = LibraryId(li as u32);
        let mut coletar = |out: &mut Vec<Aplicacao>, uid: UnitId, meta: &[ast::Annotation], alvo: Alvo, grupo: Grupo| {
            let u = v.program.unit(uid);
            // Da direita para a esquerda: a última anotação roda primeiro.
            for a in meta.iter().rev() {
                let Some((c, construtor)) = macro_da_anotacao(v, lib_id, a, &macros) else { continue };
                let mc = v.program.class(c);
                out.push(Aplicacao {
                    biblioteca: lib.uri.clone(),
                    alvo: alvo.clone(),
                    grupo: grupo.clone(),
                    macro_: format!("{}#{}", v.program.library(mc.library).uri, v.interner.resolve(mc.name)),
                    construtor,
                    argumentos: argumentos(u, a),
                    unidade: u.uri.clone(),
                    span: a.span,
                });
            }
        };
        for &uid in &lib.units {
            let u = v.program.unit(uid);
            for d in &u.unit.directives {
                if matches!(d.kind, DirectiveKind::Library { .. }) {
                    coletar(&mut out, uid, &d.metadata, Alvo::Biblioteca(lib.uri.clone()), Grupo::Biblioteca);
                }
            }
            for &did in &u.unit.declarations {
                let d = u.ast.decl(did);
                if d.augment {
                    continue;
                }
                let n = |s: dartforge_intern::SymbolId| v.interner.resolve(s).to_string();
                match &d.kind {
                    DeclKind::Class(c) => {
                        let chave = Chave::Tipo { lib: lib.uri.clone(), nome: n(c.name.sym) };
                        membros(v, &mut out, &mut coletar, uid, &c.members, &chave);
                        coletar(&mut out, uid, &d.metadata, Alvo::Declaracao(chave.clone()), Grupo::Classe(chave));
                    }
                    DeclKind::Mixin(m) => {
                        let chave = Chave::Tipo { lib: lib.uri.clone(), nome: n(m.name.sym) };
                        membros(v, &mut out, &mut coletar, uid, &m.members, &chave);
                        coletar(&mut out, uid, &d.metadata, Alvo::Declaracao(chave.clone()), Grupo::Classe(chave));
                    }
                    DeclKind::Enum(e) => {
                        let chave = Chave::Tipo { lib: lib.uri.clone(), nome: n(e.name.sym) };
                        coletar(&mut out, uid, &d.metadata, Alvo::Declaracao(chave.clone()), Grupo::Classe(chave));
                    }
                    DeclKind::Function(fid) => {
                        let f = u.ast.function(*fid);
                        let Some(nome) = f.name else { continue };
                        let chave = Chave::FuncaoDeTopo {
                            lib: lib.uri.clone(),
                            nome: n(nome.sym),
                            setter: f.kind == ast::FunctionKind::Setter,
                        };
                        coletar(&mut out, uid, &d.metadata, Alvo::Declaracao(chave), Grupo::Topo);
                    }
                    DeclKind::Variables(l) => {
                        for var in l.variables.iter() {
                            let chave = Chave::VariavelDeTopo { lib: lib.uri.clone(), nome: n(var.name.sym) };
                            coletar(&mut out, uid, &d.metadata, Alvo::Declaracao(chave), Grupo::Topo);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

type Coletor<'c> = dyn FnMut(&mut Vec<Aplicacao>, UnitId, &[ast::Annotation], Alvo, Grupo) + 'c;

/// Membros de uma classe: campos e métodos na ordem, depois construtores
/// (`localMemberIterator` e `localConstructorIterator` do CFE).
fn membros(v: &Vista<'_>, out: &mut Vec<Aplicacao>, coletar: &mut Coletor<'_>, uid: UnitId, ids: &[ast::MemberId], classe: &Chave) {
    let u = v.program.unit(uid);
    let Chave::Tipo { lib, nome: dono } = classe else { return };
    let n = |s: dartforge_intern::SymbolId| v.interner.resolve(s).to_string();
    for construtores in [false, true] {
        for &mid in ids {
            let m = u.ast.member(mid);
            if m.metadata.is_empty() || m.augment {
                continue;
            }
            let chave = match (&m.kind, construtores) {
                (MemberKind::Constructor(k), true) => {
                    Chave::Construtor { lib: lib.clone(), dono: dono.clone(), nome: k.name.map(|x| n(x.sym)).unwrap_or_default() }
                }
                (MemberKind::Method(fid), false) => {
                    let f = u.ast.function(*fid);
                    let Some(nome) = f.name else { continue };
                    Chave::Metodo { lib: lib.clone(), dono: dono.clone(), nome: n(nome.sym), setter: f.kind == ast::FunctionKind::Setter }
                }
                (MemberKind::Field(l), false) => {
                    for var in l.variables.iter() {
                        let chave = Chave::Campo { lib: lib.clone(), dono: dono.clone(), nome: n(var.name.sym) };
                        coletar(out, uid, &m.metadata, Alvo::Declaracao(chave), Grupo::Membro { classe: classe.clone(), construtor: false });
                    }
                    continue;
                }
                _ => continue,
            };
            coletar(out, uid, &m.metadata, Alvo::Declaracao(chave), Grupo::Membro { classe: classe.clone(), construtor: construtores });
        }
    }
}

/// A ordem de execução das aplicações numa fase (índices em `apps`).
pub fn ordem(fase: Fase, apps: &[Aplicacao], v: &Vista<'_>) -> Vec<usize> {
    let mut bibliotecas: Vec<&str> = Vec::new();
    for a in apps {
        if !bibliotecas.contains(&a.biblioteca.as_str()) {
            bibliotecas.push(&a.biblioteca);
        }
    }
    let classes_da_biblioteca = |lib: &str| -> Vec<Chave> {
        let mut cs: Vec<Chave> = Vec::new();
        for a in apps.iter().filter(|a| a.biblioteca == lib) {
            if let Grupo::Membro { classe, .. } | Grupo::Classe(classe) = &a.grupo {
                if !cs.contains(classe) {
                    cs.push(classe.clone());
                }
            }
        }
        cs
    };
    let da_classe = |c: &Chave, out: &mut Vec<usize>| {
        for construtor in [false, true] {
            out.extend(apps.iter().enumerate().filter(|(_, a)| matches!(&a.grupo, Grupo::Membro { classe, construtor: k } if classe == c && *k == construtor)).map(|(i, _)| i));
        }
        out.extend(apps.iter().enumerate().filter(|(_, a)| matches!(&a.grupo, Grupo::Classe(k) if k == c)).map(|(i, _)| i));
    };
    let de_topo = |lib: &str, out: &mut Vec<usize>| {
        out.extend(apps.iter().enumerate().filter(|(_, a)| a.biblioteca == lib && a.grupo == Grupo::Biblioteca).map(|(i, _)| i));
        out.extend(apps.iter().enumerate().filter(|(_, a)| a.biblioteca == lib && a.grupo == Grupo::Topo).map(|(i, _)| i));
    };
    let mut out = Vec::new();
    match fase {
        Fase::Tipos | Fase::Definicoes => {
            for lib in &bibliotecas {
                de_topo(lib, &mut out);
                for c in classes_da_biblioteca(lib) {
                    da_classe(&c, &mut out);
                }
            }
        }
        Fase::Declaracoes => {
            // Supertipos antes: pós-ordem da hierarquia (`sortedSourceClassBuilders`).
            let todas: Vec<Chave> = bibliotecas.iter().flat_map(|l| classes_da_biblioteca(l)).collect();
            let ids: Vec<Option<ClassId>> = todas.iter().map(|c| v.classe(c)).collect();
            let mut feitas: HashSet<usize> = HashSet::new();
            fn visitar(i: usize, ids: &[Option<ClassId>], v: &Vista<'_>, feitas: &mut HashSet<usize>, seq: &mut Vec<usize>, pilha: &mut HashSet<usize>) {
                if feitas.contains(&i) || !pilha.insert(i) {
                    return;
                }
                if let Some(c) = ids[i] {
                    let cl = v.program.class(c);
                    let supers = cl.supertype_class.into_iter().chain(cl.mixin_classes.iter().copied()).chain(cl.interface_classes.iter().copied()).chain(cl.on_classes.iter().copied());
                    for s in supers {
                        if let Some(j) = ids.iter().position(|x| *x == Some(s)) {
                            visitar(j, ids, v, feitas, seq, pilha);
                        }
                    }
                }
                pilha.remove(&i);
                if feitas.insert(i) {
                    seq.push(i);
                }
            }
            let mut seq = Vec::new();
            for i in 0..todas.len() {
                visitar(i, &ids, v, &mut feitas, &mut seq, &mut HashSet::new());
            }
            for i in seq {
                da_classe(&todas[i], &mut out);
            }
            for lib in &bibliotecas {
                de_topo(lib, &mut out);
            }
        }
    }
    out
}
