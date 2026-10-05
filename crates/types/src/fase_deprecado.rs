//! O `DeprecatedMemberUseVerifier` do analyzer
//! (`analyzer/lib/src/error/deprecated_member_use_verifier.dart`, lido por
//! inteiro na 6.11.0; docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.8):
//! `deprecated_member_use` e `deprecated_member_use_from_same_package`, com
//! e sem mensagem, no uso de um elemento anotado com `@deprecated` ou
//! `@Deprecated('…')`.
//!
//! Cobre: identificadores e acessos a propriedade resolvidos (funções,
//! variáveis, campos, métodos, acessores, membros de extensão), construtores
//! (`C.new`, `C.nome`), tipos nomeados (classe e typedef) e imports e
//! exports de biblioteca depreciada. Um uso dentro de uma declaração
//! depreciada (de topo ou membro) não é relatado.
//!
//! Fora: os operadores (binário, índice, prefixo, pós-fixo, atribuição
//! composta), os argumentos passados a parâmetros depreciados, os nomes de
//! `show`, os campos de padrão, os `extension override` e o `call`
//! implícito. A anotação é reconhecida pelo nome, sem conferir o
//! `dart:core`; `expires` não é lido.
//! Escrito sem compilar nem executar (2026-10-04).

use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codigos::hint as h;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprKind, MemberKind, TypeKind};
use dartforge_intern::Interner;

/// `Some(mensagem)` se a lista tem `@deprecated` ou `@Deprecated(...)`.
fn deprecado(metadata: &[ast::Annotation], a: &ast::Ast, interner: &Interner) -> Option<Option<String>> {
    for m in metadata {
        let nomes: Vec<&str> = m.name.iter().map(|n| interner.resolve(n.sym)).collect();
        match &m.arguments {
            None if nomes.last() == Some(&"deprecated") => return Some(None),
            Some(args) if nomes.contains(&"Deprecated") => {
                let mensagem = args.args.iter().find(|x| x.name.is_none()).and_then(|x| match &a.expr(x.value).kind {
                    ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
                    _ => None,
                });
                return Some(mensagem);
            }
            _ => {}
        }
    }
    None
}

/// As anotações de uma função, método ou construtor, com a unidade delas.
pub(crate) fn da_funcao(program: &Program, f: FunctionElementId) -> Option<(&[ast::Annotation], UnitId)> {
    let e = program.function(f);
    // Acessor implícito: as anotações são as da variável.
    if let Some(v) = e.variable {
        return da_variavel(program, v);
    }
    match e.node {
        FunctionRef::Constructor { unit, member } => Some((&program.unit(unit).ast.member(member).metadata[..], unit)),
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            if let Some(m) = a.members.iter().find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function)) {
                return Some((&m.metadata[..], unit));
            }
            a.decls.iter().find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function)).map(|d| (&d.metadata[..], unit))
        }
        FunctionRef::None => None,
    }
}

pub(crate) fn da_variavel(program: &Program, v: VariableId) -> Option<(&[ast::Annotation], UnitId)> {
    match program.variable(v).node {
        VariableRef::TopLevel { unit, decl, .. } => Some((&program.unit(unit).ast.decl(decl).metadata[..], unit)),
        VariableRef::Field { unit, member, .. } => Some((&program.unit(unit).ast.member(member).metadata[..], unit)),
        _ => None,
    }
}

pub(crate) fn do_elemento(program: &Program, e: Element) -> Option<(&[ast::Annotation], UnitId)> {
    let da_declaracao = |d: dartforge_elements::model::DeclRef| Some((&program.unit(d.unit).ast.decl(d.decl).metadata[..], d.unit));
    match e {
        Element::Class(c) => program.class(c).decl.and_then(da_declaracao),
        Element::Typedef(t) => da_declaracao(program.typedef(t).decl),
        Element::Extension(x) => da_declaracao(program.extension(x).decl),
        Element::Function(f) => da_funcao(program, f),
        Element::Variable(v) => da_variavel(program, v),
        Element::Prefix(..) => None,
    }
}

/// A biblioteca é depreciada: a diretiva `library` dela tem a anotação.
fn biblioteca_deprecada(program: &Program, interner: &Interner, lib: LibraryId) -> Option<Option<String>> {
    let primeira = *program.library(lib).units.first()?;
    let unidade = program.unit(primeira);
    let diretiva = unidade.unit.directives.iter().find(|d| matches!(d.kind, DirectiveKind::Library { .. }))?;
    deprecado(&diretiva.metadata, &unidade.ast, interner)
}

/// Os usos de elementos depreciados na unidade `u`. `mesmo_pacote` diz se
/// uma biblioteca está no pacote da biblioteca analisada
/// (`_isLibraryInWorkspacePackage`).
pub fn usos_de_deprecados(
    program: &Program,
    interner: &Interner,
    corpo: Option<&UnitBodyTypes>,
    u: UnitId,
    mesmo_pacote: &dyn Fn(LibraryId) -> bool,
) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    // Sem nenhuma das duas anotações internadas, nada a fazer.
    if interner.lookup("deprecated").is_none() && interner.lookup("Deprecated").is_none() {
        return out;
    }
    let unidade = program.unit(u);
    let a = &unidade.ast;
    // `_inDeprecatedMemberStack`: as declarações depreciadas desta unidade.
    let mut regioes: Vec<Span> = Vec::new();
    regioes.extend(a.decls.iter().filter(|d| deprecado(&d.metadata, a, interner).is_some()).map(|d| d.span));
    regioes.extend(a.members.iter().filter(|m| deprecado(&m.metadata, a, interner).is_some()).map(|m| m.span));
    let em_deprecado = |s: Span| regioes.iter().any(|r| r.start <= s.start && s.end <= r.end);
    // `reportError`.
    let mut relatar = |span: Span, nome: &str, mensagem: Option<String>, lib: LibraryId| {
        if em_deprecado(span) {
            return;
        }
        let mesmo = mesmo_pacote(lib);
        let d = match mensagem.as_deref().map(str::trim).filter(|m| !m.is_empty() && *m != ".") {
            None => {
                let codigo = if mesmo { h::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE } else { h::DEPRECATED_MEMBER_USE };
                Diagnostic::com_codigo(codigo, span, [nome])
            }
            Some(m) => {
                let pontuada = if m.ends_with(['.', '?', '!']) { m.to_string() } else { format!("{m}.") };
                let codigo =
                    if mesmo { h::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE } else { h::DEPRECATED_MEMBER_USE_WITH_MESSAGE };
                Diagnostic::com_codigo(codigo, span, [nome, pontuada.as_str()])
            }
        };
        // O ouvinte do analyzer é um conjunto: um relato por posição.
        if !out.iter().any(|x| x.code == d.code && x.span == d.span) {
            out.push(d);
        }
    };
    let mut checar = |anotacoes: Option<(&[ast::Annotation], UnitId)>, span: Span, nome: &str| {
        let Some((metadata, onde)) = anotacoes else { return };
        let dono = program.unit(onde);
        if let Some(mensagem) = deprecado(metadata, &dono.ast, interner) {
            relatar(span, nome, mensagem, dono.library);
        }
    };
    // O nome de exibição de um construtor: `C.new` ou `C.nome`.
    let nome_do_construtor = |f: FunctionElementId| {
        let k = program.function(f);
        let classe = k.class.map_or("", |c| interner.resolve(program.class(c).name));
        let ctor = interner.resolve(k.name);
        format!("{classe}.{}", if ctor.is_empty() { "new" } else { ctor })
    };
    // 1. Tipos nomeados (`namedType`): classe ou typedef, no nome.
    for t in a.types.iter() {
        let TypeKind::Named { name, .. } = &t.kind else { continue };
        let ligacao = match &name[..] {
            [n] => program.lookup_na_unidade(u, n.sym),
            [p, n] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        };
        let (Some(elemento), Some(ultimo)) = (ligacao.and_then(|b| b.getter), name.last()) else { continue };
        if matches!(elemento, Element::Class(_) | Element::Typedef(_)) {
            checar(do_elemento(program, elemento), ultimo.span, interner.resolve(ultimo.sym));
        }
    }
    // 2. Expressões resolvidas.
    if let Some(corpo) = corpo {
        for (i, expr) in a.exprs.iter().enumerate() {
            let Some(Some(r)) = corpo.resolved.get(i) else { continue };
            let (no_nome, texto, inteiro) = match &expr.kind {
                ExprKind::Identifier(n) => (n.span, interner.resolve(n.sym), n.span),
                ExprKind::Property { name, .. } => (name.span, interner.resolve(name.sym), expr.span),
                ExprKind::InstanceCreation { .. } => (expr.span, "", expr.span),
                _ => continue,
            };
            match r {
                // `constructorName`: o nome inteiro do construtor; e a classe
                // de uma criação sem `new`, que o analyzer vê como tipo.
                Resolved::Constructor(f) => {
                    let nome = nome_do_construtor(*f);
                    checar(da_funcao(program, *f), inteiro, &nome);
                    if let (ExprKind::Identifier(_), Some(c)) = (&expr.kind, program.function(*f).class) {
                        checar(do_elemento(program, Element::Class(c)), no_nome, texto);
                    }
                }
                Resolved::Element(e) if !matches!(expr.kind, ExprKind::InstanceCreation { .. }) => {
                    checar(do_elemento(program, *e), no_nome, texto);
                }
                Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
                    // Um construtor não chega aqui; getters e setters levam
                    // o nome escrito.
                    if !matches!(program.function(*f).kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
                        checar(da_funcao(program, *f), no_nome, texto);
                    }
                }
                Resolved::Member { member: MemberRef::Variable(v), .. } => checar(da_variavel(program, *v), no_nome, texto),
                _ => {}
            }
        }
    }
    // 3. Imports e exports de biblioteca depreciada: a diretiva inteira, com
    // a URI da biblioteca como nome.
    let biblioteca = program.library(unidade.library);
    let alvos = biblioteca
        .imports
        .iter()
        .filter(|i| i.unit == u)
        .map(|i| (i.directive, i.library))
        .chain(biblioteca.exports.iter().filter(|e| e.unit == u).map(|e| (e.directive, e.library)));
    for (indice, alvo) in alvos {
        let Some(diretiva) = unidade.unit.directives.get(indice) else { continue };
        if let Some(mensagem) = biblioteca_deprecada(program, interner, alvo) {
            relatar(diretiva.span, program.library(alvo).uri.as_str(), mensagem, alvo);
        }
    }
    out
}
