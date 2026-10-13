//! O `UseResultVerifier` do analyzer
//! (`analyzer/lib/src/error/use_result_verifier.dart`, lido por inteiro na
//! 6.11.0; docs/ANALYZER-ESPECIFICACAO-INFRA.md, lote II.8): `unused_result`
//! numa chamada, leitura de propriedade, identificador ou criação de
//! instância cujo elemento tem `@useResult`/`@UseResult(...)` e cujo valor
//! não é usado.
//!
//! O original sobe do nó aos pais (`_isUsed`); aqui se desce dos contextos
//! que não usam o valor: o comando de expressão, a inicialização e as
//! atualizações de um `for`, as seções de uma cascata, o operando de `is` e
//! o valor de um `switch` expressão. Dali o "não usado" atravessa
//! parênteses, `await`, `as`, `?:`, `!` e os operadores prefixos. Ficam de
//! fora os contextos raros que o original também não conta como uso (valor
//! padrão de parâmetro, expressão de padrão constante).
//!
//! A anotação é reconhecida pelo nome, sem conferir o `package:meta`.
//! Escrito sem compilar nem executar (2026-10-04).

use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, FunctionElementId, FunctionRef, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, CollectionElement, DeclKind, ExprId, ExprKind, ForInit, MemberKind, StmtKind, UnaryOp};
use dartforge_intern::Interner;

/// As anotações da declaração de uma função, método ou construtor.
/// A anotação `m`, escrita na unidade `unit`, é a constante `nome` do
/// `package:meta` (o `isDoNotStore`, `isLiteral`… do analyzer comparam o
/// elemento e a biblioteca `meta`): o nome, com ou sem prefixo, resolvido
/// pelos escopos com que o analyzer resolve a anotação (locais, parâmetros,
/// membros do tipo que a contém, unidade) para uma variável de topo de uma
/// biblioteca `package:meta/…`, e sem argumentos.
pub(crate) fn meta_annotation(program: &Program, interner: &Interner, unit: UnitId, m: &ast::Annotation, the_name: &str) -> bool {
    // Pelos escopos do analyzer (locais, membros, unidade): `anotacoes`.
    crate::anotacoes::annotation_and_variable(program, interner, unit, m, the_name, &|uri| uri.starts_with("package:meta/"))
}

/// A unidade em que a função `f` está escrita.
pub(crate) fn function_unit(program: &Program, f: FunctionElementId) -> Option<UnitId> {
    match program.function(f).node {
        FunctionRef::Constructor { unit, .. } | FunctionRef::Function { unit, .. } => Some(unit),
        FunctionRef::None => None,
    }
}

pub(crate) fn function_annotations(program: &Program, f: FunctionElementId) -> &[ast::Annotation] {
    match program.function(f).node {
        FunctionRef::Constructor { unit, member } => &program.unit(unit).ast.member(member).metadata[..],
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            if let Some(m) = a.members.iter().find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function)) {
                return &m.metadata[..];
            }
            a.decls
                .iter()
                .find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function))
                .map_or(&[][..], |d| &d.metadata[..])
        }
        FunctionRef::None => &[],
    }
}

/// As anotações da declaração de uma variável de topo ou de um campo.
fn variable_annotations(program: &Program, v: VariableId) -> &[ast::Annotation] {
    match program.variable(v).node {
        VariableRef::TopLevel { unit, decl, .. } => &program.unit(unit).ast.decl(decl).metadata[..],
        VariableRef::Field { unit, member, .. } => &program.unit(unit).ast.member(member).metadata[..],
        _ => &[],
    }
}

/// `_getUseResultMetadata`: as anotações do elemento; as do campo para um
/// acessor implícito.
pub(crate) fn element_annotations<'p>(program: &'p Program, r: &Resolved) -> (&'p [ast::Annotation], Option<UnitId>) {
    let of_function = |f: FunctionElementId| {
        let e = program.function(f);
        let unit_id = match e.node {
            FunctionRef::Constructor { unit, .. } | FunctionRef::Function { unit, .. } => Some(unit),
            FunctionRef::None => None,
        };
        match e.variable {
            Some(v) => (variable_annotations(program, v), variable_unit(program, v)),
            None => (function_annotations(program, f), unit_id),
        }
    };
    match r {
        Resolved::Element(Element::Function(f)) | Resolved::Constructor(f) => of_function(*f),
        Resolved::ExtensionMember { member, .. } => of_function(*member),
        Resolved::Member { member: MemberRef::Function(f), .. } => of_function(*f),
        Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => {
            (variable_annotations(program, *v), variable_unit(program, *v))
        }
        _ => (&[][..], None),
    }
}

fn variable_unit(program: &Program, v: VariableId) -> Option<UnitId> {
    match program.variable(v).node {
        VariableRef::TopLevel { unit, .. } | VariableRef::Field { unit, .. } => Some(unit),
        _ => None,
    }
}

/// O que a anotação de uso de resultado diz.
struct Uso {
    /// `UseResult('msg')` ou `message:`.
    mensagem: Option<String>,
    /// `UseResult.unless(parameterDefined: 'p')`.
    parametro: Option<String>,
}

/// A primeira anotação `@useResult` ou `@UseResult(...)` da lista. `a` é a
/// árvore da unidade em que a anotação está escrita.
fn uso_de_resultado(metadata: &[ast::Annotation], a: &ast::Ast, interner: &Interner) -> Option<Uso> {
    let text_of = |e: ExprId| match &a.expr(e).kind {
        ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
        _ => None,
    };
    for m in metadata {
        let names: Vec<&str> = m.name.iter().map(|n| interner.resolve(n.sym)).collect();
        match &m.arguments {
            // `@useResult`: um getter constante, sem mensagem.
            None if names.last() == Some(&"useResult") => return Some(Uso { mensagem: None, parametro: None }),
            Some(args) if names.contains(&"UseResult") => {
                let named = |the_name: &str| {
                    args.args.iter().find(|x| x.name.is_some_and(|n| interner.resolve(n.sym) == the_name)).and_then(|x| text_of(x.value))
                };
                let positional = args.args.iter().find(|x| x.name.is_none()).and_then(|x| text_of(x.value));
                return Some(Uso { mensagem: positional.or_else(|| named("message")), parametro: named("parameterDefined") });
            }
            _ => {}
        }
    }
    None
}

/// Os elementos de coleção `for (init; cond; updates)`: a inicialização e as
/// atualizações são contextos que não usam o valor.
fn collection_roots(el: &CollectionElement, roots: &mut Vec<ExprId>) {
    match el {
        CollectionElement::For { init, updates, body, .. } => {
            if let Some(ForInit::Expression(e)) = init {
                roots.push(*e);
            }
            roots.extend(updates.iter().copied());
            collection_roots(body, roots);
        }
        CollectionElement::ForIn { body, .. } => collection_roots(body, roots),
        CollectionElement::If { then, else_, .. } => {
            collection_roots(then, roots);
            if let Some(e) = else_ {
                collection_roots(e, roots);
            }
        }
        _ => {}
    }
}

/// Os nós "não usados" a partir de uma raiz: a raiz e, através dos nós que
/// só repassam o valor, os operandos deles.
fn descend(a: &ast::Ast, root: ExprId, output: &mut Vec<ExprId>) {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        output.push(e);
        match &a.expr(e).kind {
            ExprKind::Parenthesized(x) | ExprKind::Await(x) => stack.push(*x),
            ExprKind::As { value, .. } => stack.push(*value),
            ExprKind::Conditional { condition, then, else_ } => stack.extend([*condition, *then, *else_]),
            // `e!` e os prefixos repassam; `e++` e `e--` contam como uso.
            ExprKind::Unary { op, operand } if !matches!(op, UnaryOp::PostfixInc | UnaryOp::PostfixDec) => stack.push(*operand),
            _ => {}
        }
    }
}

/// Os `unused_result` da unidade `u`.
pub fn resultados_nao_usados(program: &Program, interner: &Interner, the_body: &UnitBodyTypes, u: UnitId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Sem nenhuma das duas anotações internadas, nada a fazer.
    if interner.lookup("useResult").is_none() && interner.lookup("UseResult").is_none() {
        return out;
    }
    let unit_id = program.unit(u);
    let a = &unit_id.ast;
    let mut roots: Vec<ExprId> = Vec::new();
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Expression(e) => roots.push(*e),
            StmtKind::For { init, updates, .. } => {
                if let Some(ForInit::Expression(e)) = init {
                    roots.push(*e);
                }
                roots.extend(updates.iter().copied());
            }
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Cascade { sections, .. } => roots.extend(sections.iter().copied()),
            ExprKind::Is { value, .. } | ExprKind::Switch { value, .. } => roots.push(*value),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    collection_roots(el, &mut roots);
                }
            }
            _ => {}
        }
    }
    let mut unused: Vec<ExprId> = Vec::new();
    for r in roots {
        descend(a, r, &mut unused);
    }
    unused.sort_by_key(|e| e.0);
    unused.dedup();
    let resolved = |e: ExprId| the_body.resolved.get(e.0 as usize).and_then(|r| r.as_ref());
    for e in unused {
        let expr = a.expr(e);
        // O elemento, o nó a marcar, o nome a mostrar e os argumentos (só
        // de uma chamada de método).
        let (r, span, the_name, args_list): (&Resolved, Span, String, Option<&ast::Arguments>) = match &expr.kind {
            ExprKind::Identifier(n) => match resolved(e) {
                Some(r) => (r, n.span, interner.resolve(n.sym).to_string(), None),
                None => continue,
            },
            ExprKind::Property { name, .. } => match resolved(e) {
                Some(r) => (r, name.span, interner.resolve(name.sym).to_string(), None),
                None => continue,
            },
            ExprKind::Call { target, arguments } => {
                let Some(r) = resolved(*target).or_else(|| resolved(e)) else { continue };
                match &a.expr(*target).kind {
                    ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => {
                        (r, n.span, interner.resolve(n.sym).to_string(), Some(&**arguments))
                    }
                    _ => continue,
                }
            }
            ExprKind::InstanceCreation { .. } => match resolved(e) {
                Some(r @ Resolved::Constructor(_)) => (r, expr.span, String::new(), None),
                _ => continue,
            },
            _ => continue,
        };
        let (metadata, where_) = element_annotations(program, r);
        let Some(where_) = where_ else { continue };
        let Some(use_) = uso_de_resultado(metadata, &program.unit(where_).ast, interner) else { continue };
        // Uma chamada sem `new` que resolve para construtor é criação de
        // instância: marca o nó inteiro, com o nome do construtor.
        let (span, the_name) = match r {
            Resolved::Constructor(f) => {
                let k = program.function(*f);
                let class = k.class.map_or("", |c| interner.resolve(program.class(c).name));
                let ctor = interner.resolve(k.name);
                (expr.span, if ctor.is_empty() { class.to_string() } else { format!("{class}.{ctor}") })
            }
            _ => (span, the_name),
        };
        // `_passesUsingParam`: a chamada passa o parâmetro de `unless`.
        if let (Some(p), Some(args), false) = (&use_.parametro, args_list, matches!(r, Resolved::Constructor(_)))
            && args.args.iter().any(|x| x.name.is_some_and(|n| interner.resolve(n.sym) == p))
        {
            continue;
        }
        match use_.mensagem.as_deref().filter(|m| !m.is_empty()) {
            None => out.push(Diagnostic::com_codigo(w::UNUSED_RESULT, span, [the_name.as_str()])),
            Some(m) => out.push(Diagnostic::com_codigo(w::UNUSED_RESULT_WITH_MESSAGE, span, [the_name.as_str(), m])),
        }
    }
    out
}
