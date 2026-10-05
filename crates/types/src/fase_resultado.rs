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
fn anotacoes_da_funcao(program: &Program, f: FunctionElementId) -> &[ast::Annotation] {
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
fn anotacoes_da_variavel(program: &Program, v: VariableId) -> &[ast::Annotation] {
    match program.variable(v).node {
        VariableRef::TopLevel { unit, decl, .. } => &program.unit(unit).ast.decl(decl).metadata[..],
        VariableRef::Field { unit, member, .. } => &program.unit(unit).ast.member(member).metadata[..],
        _ => &[],
    }
}

/// `_getUseResultMetadata`: as anotações do elemento; as do campo para um
/// acessor implícito.
fn anotacoes_do_elemento<'p>(program: &'p Program, r: &Resolved) -> (&'p [ast::Annotation], Option<UnitId>) {
    let da_funcao = |f: FunctionElementId| {
        let e = program.function(f);
        let unidade = match e.node {
            FunctionRef::Constructor { unit, .. } | FunctionRef::Function { unit, .. } => Some(unit),
            FunctionRef::None => None,
        };
        match e.variable {
            Some(v) => (anotacoes_da_variavel(program, v), unidade_da_variavel(program, v)),
            None => (anotacoes_da_funcao(program, f), unidade),
        }
    };
    match r {
        Resolved::Element(Element::Function(f)) | Resolved::Constructor(f) => da_funcao(*f),
        Resolved::ExtensionMember { member, .. } => da_funcao(*member),
        Resolved::Member { member: MemberRef::Function(f), .. } => da_funcao(*f),
        Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => {
            (anotacoes_da_variavel(program, *v), unidade_da_variavel(program, *v))
        }
        _ => (&[][..], None),
    }
}

fn unidade_da_variavel(program: &Program, v: VariableId) -> Option<UnitId> {
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
    let texto_de = |e: ExprId| match &a.expr(e).kind {
        ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
        _ => None,
    };
    for m in metadata {
        let nomes: Vec<&str> = m.name.iter().map(|n| interner.resolve(n.sym)).collect();
        match &m.arguments {
            // `@useResult`: um getter constante, sem mensagem.
            None if nomes.last() == Some(&"useResult") => return Some(Uso { mensagem: None, parametro: None }),
            Some(args) if nomes.contains(&"UseResult") => {
                let nomeado = |nome: &str| {
                    args.args.iter().find(|x| x.name.is_some_and(|n| interner.resolve(n.sym) == nome)).and_then(|x| texto_de(x.value))
                };
                let posicional = args.args.iter().find(|x| x.name.is_none()).and_then(|x| texto_de(x.value));
                return Some(Uso { mensagem: posicional.or_else(|| nomeado("message")), parametro: nomeado("parameterDefined") });
            }
            _ => {}
        }
    }
    None
}

/// Os elementos de coleção `for (init; cond; updates)`: a inicialização e as
/// atualizações são contextos que não usam o valor.
fn raizes_de_colecao(el: &CollectionElement, raizes: &mut Vec<ExprId>) {
    match el {
        CollectionElement::For { init, updates, body, .. } => {
            if let Some(ForInit::Expression(e)) = init {
                raizes.push(*e);
            }
            raizes.extend(updates.iter().copied());
            raizes_de_colecao(body, raizes);
        }
        CollectionElement::ForIn { body, .. } => raizes_de_colecao(body, raizes),
        CollectionElement::If { then, else_, .. } => {
            raizes_de_colecao(then, raizes);
            if let Some(e) = else_ {
                raizes_de_colecao(e, raizes);
            }
        }
        _ => {}
    }
}

/// Os nós "não usados" a partir de uma raiz: a raiz e, através dos nós que
/// só repassam o valor, os operandos deles.
fn descer(a: &ast::Ast, raiz: ExprId, saida: &mut Vec<ExprId>) {
    let mut pilha = vec![raiz];
    while let Some(e) = pilha.pop() {
        saida.push(e);
        match &a.expr(e).kind {
            ExprKind::Parenthesized(x) | ExprKind::Await(x) => pilha.push(*x),
            ExprKind::As { value, .. } => pilha.push(*value),
            ExprKind::Conditional { condition, then, else_ } => pilha.extend([*condition, *then, *else_]),
            // `e!` e os prefixos repassam; `e++` e `e--` contam como uso.
            ExprKind::Unary { op, operand } if !matches!(op, UnaryOp::PostfixInc | UnaryOp::PostfixDec) => pilha.push(*operand),
            _ => {}
        }
    }
}

/// Os `unused_result` da unidade `u`.
pub fn resultados_nao_usados(program: &Program, interner: &Interner, corpo: &UnitBodyTypes, u: UnitId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // Sem nenhuma das duas anotações internadas, nada a fazer.
    if interner.lookup("useResult").is_none() && interner.lookup("UseResult").is_none() {
        return out;
    }
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let mut raizes: Vec<ExprId> = Vec::new();
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Expression(e) => raizes.push(*e),
            StmtKind::For { init, updates, .. } => {
                if let Some(ForInit::Expression(e)) = init {
                    raizes.push(*e);
                }
                raizes.extend(updates.iter().copied());
            }
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Cascade { sections, .. } => raizes.extend(sections.iter().copied()),
            ExprKind::Is { value, .. } | ExprKind::Switch { value, .. } => raizes.push(*value),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    raizes_de_colecao(el, &mut raizes);
                }
            }
            _ => {}
        }
    }
    let mut nao_usados: Vec<ExprId> = Vec::new();
    for r in raizes {
        descer(a, r, &mut nao_usados);
    }
    nao_usados.sort_by_key(|e| e.0);
    nao_usados.dedup();
    let resolvido = |e: ExprId| corpo.resolved.get(e.0 as usize).and_then(|r| r.as_ref());
    for e in nao_usados {
        let expr = a.expr(e);
        // O elemento, o nó a marcar, o nome a mostrar e os argumentos (só
        // de uma chamada de método).
        let (r, span, nome, argumentos): (&Resolved, Span, String, Option<&ast::Arguments>) = match &expr.kind {
            ExprKind::Identifier(n) => match resolvido(e) {
                Some(r) => (r, n.span, interner.resolve(n.sym).to_string(), None),
                None => continue,
            },
            ExprKind::Property { name, .. } => match resolvido(e) {
                Some(r) => (r, name.span, interner.resolve(name.sym).to_string(), None),
                None => continue,
            },
            ExprKind::Call { target, arguments } => {
                let Some(r) = resolvido(*target).or_else(|| resolvido(e)) else { continue };
                match &a.expr(*target).kind {
                    ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => {
                        (r, n.span, interner.resolve(n.sym).to_string(), Some(&**arguments))
                    }
                    _ => continue,
                }
            }
            ExprKind::InstanceCreation { .. } => match resolvido(e) {
                Some(r @ Resolved::Constructor(_)) => (r, expr.span, String::new(), None),
                _ => continue,
            },
            _ => continue,
        };
        let (metadata, onde) = anotacoes_do_elemento(program, r);
        let Some(onde) = onde else { continue };
        let Some(uso) = uso_de_resultado(metadata, &program.unit(onde).ast, interner) else { continue };
        // Uma chamada sem `new` que resolve para construtor é criação de
        // instância: marca o nó inteiro, com o nome do construtor.
        let (span, nome) = match r {
            Resolved::Constructor(f) => {
                let k = program.function(*f);
                let classe = k.class.map_or("", |c| interner.resolve(program.class(c).name));
                let ctor = interner.resolve(k.name);
                (expr.span, if ctor.is_empty() { classe.to_string() } else { format!("{classe}.{ctor}") })
            }
            _ => (span, nome),
        };
        // `_passesUsingParam`: a chamada passa o parâmetro de `unless`.
        if let (Some(p), Some(args), false) = (&uso.parametro, argumentos, matches!(r, Resolved::Constructor(_)))
            && args.args.iter().any(|x| x.name.is_some_and(|n| interner.resolve(n.sym) == p))
        {
            continue;
        }
        match uso.mensagem.as_deref().filter(|m| !m.is_empty()) {
            None => out.push(Diagnostic::com_codigo(w::UNUSED_RESULT, span, [nome.as_str()])),
            Some(m) => out.push(Diagnostic::com_codigo(w::UNUSED_RESULT_WITH_MESSAGE, span, [nome.as_str(), m])),
        }
    }
    out
}
