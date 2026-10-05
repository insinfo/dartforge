//! O oitavo lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_conditional_assignment`, `join_return_with_assignment`,
//! `literal_only_boolean_expressions`,
//! `avoid_unused_constructor_parameters`, `unnecessary_const`,
//! `avoid_init_to_null`, `type_init_formals`,
//! `always_put_control_body_on_new_line` e `directives_ordering`.
//!
//! Com o mesmo dado do original (as que pedem o elemento ou o tipo só
//! relatam com a semântica da unidade):
//! - `prefer_conditional_assignment` e `join_return_with_assignment`: o
//!   `canonicalElementsFromIdentifiersAreEqual` (`super::mesmos_elementos`).
//! - `avoid_unused_constructor_parameters`: o uso pelo elemento do
//!   parâmetro, e o `@deprecated`/`@Deprecated(…)` resolvido.
//! - `avoid_init_to_null`: o `isNullable` do tipo do elemento (do local do
//!   corpo, do escrito, do campo, do parâmetro da superclasse), e o
//!   `super.x` só com o padrão do parâmetro da superclasse `null` ou ausente.
//! - `type_init_formals`: o tipo resolvido do `this.x` contra o do campo e o
//!   do `super.x` contra o do parâmetro da superclasse.
//! - `unnecessary_const`: o `inConstantContext` (`dartforge_frontend::pais`).
//! - `directives_ordering`: também os `@docImport` da documentação da
//!   diretiva `library`, com o `stringValue` das URIs.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::cordas::{literais, valor_de_string};
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, BinaryOp, CollectionElement, CreationKeyword, DeclKind, DirectiveKind, ExprId, ExprKind, ForInit, MemberId, MemberKind,
    StmtId, StmtKind, UnaryOp, VariableList,
};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

fn sem_parenteses(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn nulo(a: &Ast, e: ExprId) -> bool {
    matches!(a.expr(sem_parenteses(a, e)).kind, ExprKind::Null)
}

/// `_onlyLiterals`.
fn so_literais(a: &Ast, e: Option<ExprId>) -> bool {
    let Some(e) = e else { return false };
    match &a.expr(sem_parenteses(a, e)).kind {
        ExprKind::Int(_)
        | ExprKind::Double(_)
        | ExprKind::Bool(_)
        | ExprKind::Null
        | ExprKind::String(_)
        | ExprKind::Symbol(_)
        | ExprKind::List { .. }
        | ExprKind::SetOrMap { .. }
        | ExprKind::Record { .. } => true,
        ExprKind::Unary { op: UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot | UnaryOp::PrefixInc | UnaryOp::PrefixDec, operand } => {
            so_literais(a, Some(*operand))
        }
        ExprKind::Binary { op: BinaryOp::IfNull, left, .. } => so_literais(a, Some(*left)),
        ExprKind::Binary { left, right, .. } => so_literais(a, Some(*left)) && so_literais(a, Some(*right)),
        _ => false,
    }
}

/// As condições `when` das coleções (o `if (x case p when c)` elemento).
fn guardas_de_elemento(el: &CollectionElement, saida: &mut Vec<ExprId>) {
    match el {
        CollectionElement::If { guard, then, else_, .. } => {
            saida.extend(guard.iter().copied());
            guardas_de_elemento(then, saida);
            if let Some(x) = else_ {
                guardas_de_elemento(x, saida);
            }
        }
        CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => guardas_de_elemento(body, saida),
        _ => {}
    }
}

/// O primeiro token a partir de `inicio` (o `beginToken` de um comando).
fn primeiro_token(fonte: &str, inicio: usize) -> Span {
    let b = fonte.as_bytes();
    let de_palavra = |c: u8| c == b'_' || c == b'$' || c.is_ascii_alphanumeric();
    let mut fim = inicio;
    match b.get(inicio).copied() {
        Some(c) if de_palavra(c) => {
            while b.get(fim).copied().is_some_and(de_palavra) {
                fim += 1;
            }
        }
        Some(b'\'' | b'"') => {
            fim = literais(fonte, Span { start: inicio, end: fonte.len() }).first().map_or(inicio + 1, |l| l.span.end);
        }
        Some(c @ (b'+' | b'-')) if b.get(inicio + 1) == Some(&c) => fim = inicio + 2,
        Some(_) => fim = inicio + fonte[inicio..].chars().next().map_or(1, char::len_utf8),
        None => {}
    }
    Span { start: inicio, end: fim }
}

/// Os `@docImport` da documentação da diretiva `library` (o
/// `docImportDirectives` do `directives_ordering`): cada linha da
/// documentação (fora de bloco de código cercado ou indentado) que começa
/// com `@docImport `, com o trecho do `ImportDirective` sintético (do
/// `import ` imaginado antes da URI ao fim do último token) e o
/// `stringValue` da URI.
fn doc_imports(fonte: &str, u: Unidade<'_>) -> Vec<(Span, Option<String>)> {
    let mut v = Vec::new();
    let Some(biblioteca) = u.unit.directives.iter().find(|d| matches!(d.kind, DirectiveKind::Library { .. })) else { return v };
    let comentarios = dartforge_frontend::comentarios::Comentarios::de(fonte);
    let depois = match biblioteca.metadata.last() {
        Some(m) => dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), m.span.end),
        None => biblioteca.span.start,
    };
    let doc = comentarios
        .dart_doc(fonte, depois)
        .or_else(|| biblioteca.metadata.iter().rev().find_map(|m| comentarios.dart_doc(fonte, m.span.start)));
    let Some(doc) = doc else { return v };
    // As linhas da documentação: a sequência de `///` a partir de `doc`, ou
    // as linhas do `/** … */`; (posição do conteúdo, conteúdo).
    let mut linhas: Vec<(usize, &str)> = Vec::new();
    if fonte[doc.start..doc.end].starts_with("///") {
        for c in comentarios.antes_de(fonte, depois).into_iter().filter(|c| c.start >= doc.start) {
            let texto = &fonte[c.start..c.end];
            let Some(conteudo) = texto.strip_prefix("///") else { break };
            linhas.push((c.start + 3, conteudo));
        }
    } else {
        let corpo = &fonte[doc.start + 3..doc.end.saturating_sub(2).max(doc.start + 3)];
        let mut base = doc.start + 3;
        for linha in corpo.split('\n') {
            let sem_espaco = linha.trim_start();
            let (desloc, conteudo) = match sem_espaco.strip_prefix('*') {
                Some(r) => (linha.len() - r.len(), r),
                None => (linha.len() - sem_espaco.len(), sem_espaco),
            };
            linhas.push((base + desloc, conteudo.trim_end_matches('\r')));
            base += linha.len() + 1;
        }
    }
    let mut cercado = false;
    let mut anterior_vazia = true;
    for (pos, conteudo) in linhas {
        let recuo = conteudo.len() - conteudo.trim_start().len();
        let resto = conteudo.trim_start();
        if resto.starts_with("```") {
            cercado = !cercado;
            anterior_vazia = false;
            continue;
        }
        if cercado || (anterior_vazia && recuo >= 4) {
            anterior_vazia = conteudo.trim().is_empty();
            continue;
        }
        anterior_vazia = conteudo.trim().is_empty();
        let Some(depois_da_tag) = resto.strip_prefix("@docImport ") else { continue };
        let uri_rel = depois_da_tag.len() - depois_da_tag.trim_start().len();
        let inicio_da_uri = pos + recuo + "@docImport ".len() + uri_rel;
        let texto = depois_da_tag.trim_start();
        // O fim: o `;`, ou o fim do texto sem os brancos.
        let fim_rel = texto.find(';').map(|k| k + 1).unwrap_or_else(|| texto.trim_end().len());
        let uri = literais(fonte, Span { start: inicio_da_uri, end: inicio_da_uri + fim_rel }).first().map(|l| l.span);
        let valor = uri.and_then(|s| valor_de_string(fonte, s));
        v.push((Span { start: inicio_da_uri.saturating_sub("import ".len()), end: inicio_da_uri + fim_rel }, valor));
    }
    v
}

/// `compareDirectives`: pelo pacote (até a primeira `/`) e depois pelo resto.
fn comparar_diretivas(x: &str, y: &str) -> std::cmp::Ordering {
    if (!x.starts_with("package:") || !y.starts_with("package:")) && !x.starts_with('/') && !y.starts_with('/') {
        return x.cmp(y);
    }
    let (Some(i), Some(j)) = (x.find('/'), y.find('/')) else { return x.cmp(y) };
    x[..i].cmp(&y[..j]).then_with(|| x[i + 1..].cmp(&y[j + 1..]))
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    // O número da linha (a partir de 0) de um lugar.
    // `LineInfo`: `\n`, `\r\n` e `\r` sozinho terminam linha.
    let bs = fonte.as_bytes();
    let mut comecos: Vec<usize> = vec![0];
    comecos.extend(
        bs.iter()
            .enumerate()
            .filter(|(i, b)| **b == b'\n' || (**b == b'\r' && bs.get(i + 1) != Some(&b'\n')))
            .map(|(i, _)| i + 1),
    );
    let linha_de = |lugar: usize| comecos.partition_point(|&x| x <= lugar);

    // `prefer_conditional_assignment`: `if (x == null) x = v;`, com o lado
    // esquerdo e o testado com os mesmos elementos canônicos (pede a
    // semântica da unidade).
    if ligada("prefer_conditional_assignment")
        && let Some(s) = sem
    {
        fn atribui(s: &super::Semantica<'_>, interner: &Interner, a: &Ast, st: StmtId, testada: ExprId) -> bool {
            match &a.stmt(st).kind {
                StmtKind::Expression(e) => {
                    matches!(&a.expr(*e).kind, ExprKind::Assign { target, .. } if super::mesmos_elementos(s, interner, a, Some(*target), Some(testada)))
                }
                StmtKind::Block(ss) if ss.len() == 1 => atribui(s, interner, a, ss[0], testada),
                _ => false,
            }
        }
        for st in a.stmts.iter() {
            let StmtKind::If { condition, then, else_: None, .. } = &st.kind else { continue };
            let ExprKind::Binary { op: BinaryOp::Eq, left, right } = &a.expr(sem_parenteses(a, *condition)).kind else { continue };
            let testada = if nulo(a, *right) {
                *left
            } else if nulo(a, *left) {
                *right
            } else {
                continue;
            };
            if atribui(s, interner, a, *then, testada) {
                relatar(&c::PREFER_CONDITIONAL_ASSIGNMENT, st.span, &[]);
            }
        }
    }
    // `join_return_with_assignment`: `x = v; return x;` no fim de um bloco,
    // pelos elementos canônicos (pede a semântica da unidade).
    if ligada("join_return_with_assignment")
        && let Some(s) = sem
    {
        // O alvo da atribuição, ou o operando de um `x++`/`x!`/`-x`, de um
        // comando de expressão.
        let alvo = |st: StmtId| -> Option<ExprId> {
            let StmtKind::Expression(e) = &a.stmt(st).kind else { return None };
            match &a.expr(sem_parenteses(a, *e)).kind {
                ExprKind::Assign { target, .. } => Some(*target),
                ExprKind::Unary { operand, .. } => Some(*operand),
                _ => None,
            }
        };
        for st in a.stmts.iter() {
            let StmtKind::Block(ss) = &st.kind else { continue };
            let n = ss.len();
            if n < 2 {
                continue;
            }
            let StmtKind::Return(Some(devolvida)) = &a.stmt(ss[n - 1]).kind else { continue };
            let Some(penultimo) = alvo(ss[n - 2]) else { continue };
            let antepenultimo = if n >= 3 { alvo(ss[n - 3]) } else { None };
            if !super::mesmos_elementos(s, interner, a, Some(penultimo), antepenultimo)
                && super::mesmos_elementos(s, interner, a, Some(*devolvida), Some(penultimo))
            {
                relatar(&c::JOIN_RETURN_WITH_ASSIGNMENT, a.stmt(ss[n - 2]).span, &[]);
            }
        }
    }
    // `literal_only_boolean_expressions`: `do`, `for`, `if`, `while` (menos
    // `while (true)`) e as cláusulas `when`.
    if ligada("literal_only_boolean_expressions") {
        let mut guardas: Vec<ExprId> = Vec::new();
        for s in a.stmts.iter() {
            let relata = match &s.kind {
                StmtKind::DoWhile { condition, .. } => so_literais(a, Some(*condition)),
                StmtKind::For { condition, .. } => so_literais(a, *condition),
                StmtKind::If { condition, case_pattern, guard, .. } => {
                    guardas.extend(guard.iter().copied());
                    case_pattern.is_none() && so_literais(a, Some(*condition))
                }
                StmtKind::While { condition, .. } => !matches!(a.expr(*condition).kind, ExprKind::Bool(true)) && so_literais(a, Some(*condition)),
                StmtKind::Switch { cases, .. } => {
                    guardas.extend(cases.iter().filter_map(|k| k.guard));
                    false
                }
                _ => false,
            };
            if relata {
                relatar(&c::LITERAL_ONLY_BOOLEAN_EXPRESSIONS, s.span, &[]);
            }
        }
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Switch { cases, .. } => guardas.extend(cases.iter().filter_map(|k| k.guard)),
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                    for el in elements.iter() {
                        guardas_de_elemento(el, &mut guardas);
                    }
                }
                _ => {}
            }
        }
        for g in guardas {
            if so_literais(a, Some(g)) {
                let s = a.expr(g).span;
                let inicio = fonte.get(..s.start).and_then(|t| t.rfind("when")).unwrap_or(s.start);
                relatar(&c::LITERAL_ONLY_BOOLEAN_EXPRESSIONS, Span { start: inicio, end: s.end }, &[]);
            }
        }
    }


    // `type_init_formals`: o tipo escrito do `this.x` igual ao do campo, e o
    // do `super.x` igual ao do parâmetro do construtor da superclasse (os
    // tipos resolvidos; pede a semântica da unidade).
    if ligada("type_init_formals")
        && let Some(s) = sem
    {
        for (mi, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            for (pi, p) in k.parameters.iter().enumerate() {
                let Some(t) = p.ty else { continue };
                let Some(escrito) = super::tipo_escrito(s, t) else { continue };
                let alvo = if p.this_ {
                    p.name.and_then(|n| super::campo_da_classe(s, MemberId(mi as u32), n.sym)).and_then(|v| super::tipo_da_variavel(s, v))
                } else if p.super_ {
                    super::parametro_do_super(s, interner, MemberId(mi as u32), pi)
                        .and_then(|(f, j)| s.outline.functions.get(f.0 as usize).and_then(|d| d.parameters.get(j)).map(|x| x.ty))
                } else {
                    continue;
                };
                if alvo.is_some_and(|x| s.table.canonico(x) == s.table.canonico(escrito)) {
                    relatar(&c::TYPE_INIT_FORMALS, a.ty(t).span, &[]);
                }
            }
        }
    }
    // `avoid_init_to_null`: a variável não `final`/`const` e o parâmetro com
    // padrão `null` cujo tipo do elemento é anulável (`isNullable`); o
    // `super.x` só quando o padrão do parâmetro da superclasse é `null` ou
    // falta. Pede a semântica da unidade.
    if ligada("avoid_init_to_null")
        && let Some(s) = sem
    {
        // Os elementos das variáveis de topo e dos campos da unidade.
        let mut de_topo: HashMap<(usize, usize), dartforge_elements::model::VariableId> = HashMap::new();
        let mut de_campo: HashMap<(usize, usize), dartforge_elements::model::VariableId> = HashMap::new();
        for (i, v) in s.program.variables.iter().enumerate() {
            let id = dartforge_elements::model::VariableId(i as u32);
            match v.node {
                dartforge_elements::model::VariableRef::TopLevel { unit, decl, index } if unit == s.unidade => {
                    de_topo.insert((decl.0 as usize, index), id);
                }
                dartforge_elements::model::VariableRef::Field { unit, member, index } if unit == s.unidade => {
                    de_campo.insert((member.0 as usize, index), id);
                }
                _ => {}
            }
        }
        let mut achados: Vec<Span> = Vec::new();
        let mut da_lista = |l: &VariableList, tipo: &dyn Fn(usize, ast::Name) -> Option<dartforge_types::table::TypeId>, achados: &mut Vec<Span>| {
            if l.final_ || l.const_ {
                return;
            }
            for (k, v) in l.variables.iter().enumerate() {
                if let Some(i) = v.initializer
                    && nulo(a, i)
                    && tipo(k, v.name).is_some_and(|t| super::anulavel(s, t))
                {
                    achados.push(Span { start: v.name.span.start, end: a.expr(i).span.end });
                }
            }
        };
        for (di, d) in a.decls.iter().enumerate() {
            if let DeclKind::Variables(l) = &d.kind {
                da_lista(l, &|k, _| de_topo.get(&(di, k)).and_then(|v| super::tipo_da_variavel(s, *v)), &mut achados);
            }
        }
        for (mi, m) in a.members.iter().enumerate() {
            if let MemberKind::Field(l) = &m.kind {
                da_lista(l, &|k, _| de_campo.get(&(mi, k)).and_then(|v| super::tipo_da_variavel(s, *v)), &mut achados);
            }
        }
        let local = |_: usize, n: ast::Name| s.corpo.tipo_local(n.span.start);
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => da_lista(l, &local, &mut achados),
                _ => {}
            }
        }
        // Os parâmetros com padrão `null`: o tipo do elemento (o do local do
        // corpo; sem corpo, o escrito, o do campo ou o do parâmetro da
        // superclasse).
        let tipo_do_parametro = |mi: Option<usize>, pi: usize, p: &ast::Parameter| -> Option<dartforge_types::table::TypeId> {
            let n = p.name?;
            if let Some(t) = s.corpo.tipo_local(n.span.start) {
                return Some(t);
            }
            if let Some(t) = p.ty {
                return super::tipo_escrito(s, t);
            }
            if p.this_ {
                return super::campo_da_classe(s, MemberId(mi? as u32), n.sym).and_then(|v| super::tipo_da_variavel(s, v));
            }
            if p.super_ {
                return super::parametro_do_super(s, interner, MemberId(mi? as u32), pi)
                    .and_then(|(f, j)| s.outline.functions.get(f.0 as usize).and_then(|d| d.parameters.get(j)).map(|x| x.ty));
            }
            if p.function_parameters.is_some() {
                // A forma antiga sem retorno: um tipo de função, anulável
                // só com `?`.
                return Some(if p.function_nullable { s.core.dynamic_ } else { s.core.function });
            }
            Some(s.core.dynamic_)
        };
        // O padrão do parâmetro da superclasse é `null` (ou falta).
        let padrao_do_super_nulo = |mi: usize, pi: usize| -> bool {
            let Some((f, j)) = super::parametro_do_super(s, interner, MemberId(mi as u32), pi) else { return false };
            let dartforge_elements::model::FunctionRef::Constructor { unit, member } = s.program.function(f).node else { return true };
            let au = &s.program.unit(unit).ast;
            let MemberKind::Constructor(kk) = &au.member(member).kind else { return true };
            match kk.parameters.get(j).and_then(|x| x.default_value) {
                None => true,
                Some(d) => {
                    let sp = au.expr(d).span;
                    s.program.unit(unit).source.get(sp.start..sp.end) == Some("null")
                }
            }
        };
        fn listas<'x>(ps: &'x [ast::Parameter], saida: &mut Vec<&'x [ast::Parameter]>) {
            saida.push(ps);
            for p in ps {
                if let Some(internos) = &p.function_parameters {
                    listas(internos, saida);
                }
            }
        }
        let mut sem_dono: Vec<&[ast::Parameter]> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                listas(ps, &mut sem_dono);
            }
        }
        for lista in sem_dono {
            for (pi, p) in lista.iter().enumerate() {
                if p.default_value.is_some_and(|d| nulo(a, d)) && tipo_do_parametro(None, pi, p).is_some_and(|t| super::anulavel(s, t)) {
                    achados.push(p.span);
                }
            }
        }
        for (mi, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            for (pi, p) in k.parameters.iter().enumerate() {
                if let Some(internos) = &p.function_parameters {
                    let mut ls = Vec::new();
                    listas(internos, &mut ls);
                    for lista in ls {
                        for (qi, q) in lista.iter().enumerate() {
                            if q.default_value.is_some_and(|d| nulo(a, d)) && tipo_do_parametro(None, qi, q).is_some_and(|t| super::anulavel(s, t)) {
                                achados.push(q.span);
                            }
                        }
                    }
                }
                if !p.default_value.is_some_and(|d| nulo(a, d)) {
                    continue;
                }
                if p.super_ && !padrao_do_super_nulo(mi, pi) {
                    continue;
                }
                if tipo_do_parametro(Some(mi), pi, p).is_some_and(|t| super::anulavel(s, t)) {
                    achados.push(p.span);
                }
            }
        }
        achados.sort_by_key(|x| (x.start, x.end));
        achados.dedup();
        for x in achados {
            relatar(&c::AVOID_INIT_TO_NULL, x, &[]);
        }
    }
    // `avoid_unused_constructor_parameters`: o parâmetro (não `this.x`,
    // `super.x`, depreciado nem só de sublinhados) que nenhum identificador
    // do corpo e dos inicializadores lê (pelo elemento; pede a semântica da
    // unidade). Fora: augmentation, factory redirecionadora, `external`.
    if ligada("avoid_unused_constructor_parameters")
        && let Some(s) = sem
    {
        let primarios: HashSet<MemberId> = a
            .decls
            .iter()
            .filter_map(|d| match &d.kind {
                DeclKind::Class(x) => x.primary_constructor,
                DeclKind::Enum(x) => x.primary_constructor,
                _ => None,
            })
            .collect();
        for (i, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            if m.augment || k.redirect.is_some() || k.external || k.parte_primaria || primarios.contains(&MemberId(i as u32)) {
                continue;
            }
            // As regiões visitadas: o corpo e cada inicializador.
            let mut regioes: Vec<Span> = k
                .initializers
                .iter()
                .map(|x| match x {
                    ast::Initializer::Field { span, .. }
                    | ast::Initializer::Super { span, .. }
                    | ast::Initializer::Redirect { span, .. }
                    | ast::Initializer::Assert { span, .. } => *span,
                })
                .collect();
            match &k.body {
                ast::FunctionBody::Block(b) => regioes.push(a.stmt(*b).span),
                ast::FunctionBody::Expression(e) => regioes.push(a.expr(*e).span),
                _ => {}
            }
            let usados: HashSet<usize> = a
                .exprs
                .iter()
                .enumerate()
                .filter(|(_, e)| matches!(e.kind, ExprKind::Identifier(_)) && regioes.iter().any(|r| r.start <= e.span.start && e.span.end <= r.end))
                .filter_map(|(k, _)| s.corpo.declaracao_local(ExprId(k as u32)))
                .collect();
            for p in k.parameters.iter().filter(|p| !p.this_ && !p.super_) {
                let Some(n) = p.name else { continue };
                let texto = interner.resolve(n.sym);
                let depreciado = p.metadata.iter().any(|x| dartforge_types::anotacoes::e_deprecated(s.program, interner, s.unidade, x));
                if depreciado || texto.bytes().all(|b| b == b'_') || usados.contains(&n.span.start) {
                    continue;
                }
                relatar(&c::AVOID_UNUSED_CONSTRUCTOR_PARAMETERS, p.span, &[texto]);
            }
        }
    }
    // `always_put_control_body_on_new_line`.
    if ligada("always_put_control_body_on_new_line") {
        // O corpo não começa na linha em que o controle termina.
        let mut conferir = |corpo: StmtId, fim_do_controle: usize| {
            let n = a.stmt(corpo);
            let primeiro = match &n.kind {
                StmtKind::Block(ss) => match ss.first() {
                    Some(&x) => a.stmt(x).span.start,
                    None => return,
                },
                _ => n.span.start,
            };
            if linha_de(fim_do_controle) == linha_de(primeiro) {
                relatar(&c::ALWAYS_PUT_CONTROL_BODY_ON_NEW_LINE, primeiro_token(fonte, n.span.start), &[]);
            }
        };
        // O fim do `)` que fecha o cabeçalho, antes do corpo.
        let fecha = |inicio: usize, corpo: StmtId| -> Option<usize> {
            let ate = a.stmt(corpo).span.start;
            fonte.get(inicio..ate).and_then(|t| t.rfind(')')).map(|i| inicio + i + 1)
        };
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::DoWhile { body, .. } => conferir(*body, s.span.start + 2),
                StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } => {
                    if let Some(f) = fecha(s.span.start, *body) {
                        conferir(*body, f);
                    }
                }
                StmtKind::If { then, else_, .. } => {
                    if let Some(f) = fecha(s.span.start, *then) {
                        conferir(*then, f);
                    }
                    if let Some(senao) = else_
                        && !matches!(a.stmt(*senao).kind, StmtKind::If { .. })
                        && let Some(palavra) = fonte.get(a.stmt(*then).span.end..a.stmt(*senao).span.start).and_then(|t| t.rfind("else"))
                    {
                        conferir(*senao, a.stmt(*then).span.end + palavra + 4);
                    }
                }
                _ => {}
            }
        }
    }
    // `directives_ordering`: os imports, os exports e os `@docImport` da
    // documentação da diretiva `library`, com o `stringValue` das URIs.
    if ligada("directives_ordering") {
        #[derive(Clone, Copy, PartialEq)]
        enum Tipo {
            Import,
            Export,
            DocImport,
        }
        // (trecho do nó, tipo, `stringValue`).
        let mut diretivas: Vec<(Span, Tipo, Option<String>)> = Vec::new();
        for d in u.unit.directives.iter() {
            let (uri, tipo) = match &d.kind {
                DirectiveKind::Import { uri, .. } => (uri, Tipo::Import),
                DirectiveKind::Export { uri, .. } => (uri, Tipo::Export),
                _ => continue,
            };
            diretivas.push((d.span, tipo, valor_de_string(fonte, uri.span)));
        }
        for (span, uri) in doc_imports(fonte, u) {
            diretivas.push((span, Tipo::DocImport, uri));
        }
        let mut relatadas: HashSet<(usize, usize)> = HashSet::new();
        let mut relatar_uma = |span: Span, codigo: &'static CodigoLint, args: &[&str]| {
            if relatadas.insert((span.start, span.end)) {
                relatar(codigo, span, args);
            }
        };
        let uri_comeca = |x: &(Span, Tipo, Option<String>), p: &str| x.2.as_deref().is_some_and(|u| u.starts_with(p));
        let de_dart = |x: &&(Span, Tipo, Option<String>)| uri_comeca(x, "dart:");
        let de_pacote = |x: &&(Span, Tipo, Option<String>)| uri_comeca(x, "package:");
        let absoluta = |x: &&(Span, Tipo, Option<String>)| x.2.as_deref().is_some_and(|u| u.contains(':'));
        let relativa = |x: &&(Span, Tipo, Option<String>)| x.2.as_deref().is_some_and(|u| !u.contains(':'));
        let palavras = [(Tipo::Import, "import"), (Tipo::Export, "export"), (Tipo::DocImport, "@docImport")];
        // `_checkDartDirectiveGoFirst`.
        for (tipo, palavra) in palavras {
            for x in diretivas.iter().filter(|x| x.1 == tipo).skip_while(de_dart).filter(de_dart) {
                relatar_uma(x.0, &c::DIRECTIVES_ORDERING_DART, &[palavra]);
            }
        }
        // `_checkPackageDirectiveBeforeRelative`.
        for (tipo, palavra) in palavras {
            for x in diretivas.iter().filter(|x| x.1 == tipo && !de_dart(x)).skip_while(absoluta).filter(de_pacote) {
                relatar_uma(x.0, &c::DIRECTIVES_ORDERING_PACKAGE_BEFORE_RELATIVE, &[palavra]);
            }
        }
        // `_checkExportDirectiveAfterImportDirective`.
        let do_fim: Vec<usize> = (0..u.unit.directives.len())
            .rev()
            .skip_while(|&i| matches!(u.unit.directives[i].kind, DirectiveKind::Part { .. }))
            .skip_while(|&i| matches!(u.unit.directives[i].kind, DirectiveKind::Export { .. }))
            .filter(|&i| matches!(u.unit.directives[i].kind, DirectiveKind::Export { .. }))
            .collect();
        for i in do_fim {
            relatar_uma(u.unit.directives[i].span, &c::DIRECTIVES_ORDERING_EXPORTS, &[]);
        }
        // `_checkDirectiveSectionOrderedAlphabetically`: cada seção em ordem,
        // comparando com a URI anterior da seção.
        let secoes: [&dyn Fn(&&(Span, Tipo, Option<String>)) -> bool; 3] = [&de_dart, &relativa, &de_pacote];
        for secao in secoes {
            for tipo in [Tipo::Import, Tipo::Export, Tipo::DocImport] {
                let da_secao: Vec<&(Span, Tipo, Option<String>)> = diretivas.iter().filter(|x| x.1 == tipo).filter(|x| secao(x)).collect();
                for par in da_secao.windows(2) {
                    if let (Some(anterior), Some(atual)) = (&par[0].2, &par[1].2)
                        && comparar_diretivas(anterior, atual) == std::cmp::Ordering::Greater
                    {
                        relatar_uma(par[1].0, &c::DIRECTIVES_ORDERING_ALPHABETICAL, &[]);
                    }
                }
            }
        }
    }
    // `unnecessary_const`: o `const` escrito numa criação, coleção ou
    // record em contexto constante (`inConstantContext`); a coleção que é
    // direto a expressão de um padrão constante fica fora.
    if ligada("unnecessary_const") {
        let versao_antiga = sem.is_some_and(|s| !super::versao_ao_menos(s, 3, 0));
        let pais = dartforge_frontend::pais::Pais::novo(a, u.unit, fonte, versao_antiga);
        for (k, n) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let candidato = match &n.kind {
                ExprKind::InstanceCreation { keyword: Some(CreationKeyword::Const), .. } | ExprKind::Record { const_: true, .. } => true,
                ExprKind::List { const_: true, .. } | ExprKind::SetOrMap { const_: true, .. } => {
                    !matches!(pais.pai(id), dartforge_frontend::pais::Pai::PadraoConstante { .. })
                }
                _ => false,
            };
            if candidato && fonte.get(n.span.start..).is_some_and(|t| t.starts_with("const")) && pais.em_contexto_constante(a, id) {
                relatar(&c::UNNECESSARY_CONST, Span { start: n.span.start, end: n.span.start + 5 }, &[]);
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn so(regra: &str, fonte: &str) -> Vec<String> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut relatos = executar(u, &nomes, &|r| r == regra, None);
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| fonte[r.span.start..r.span.end].to_string()).collect()
    }

    #[test]
    fn atribuicoes() {
        // Os elementos canônicos pedem a semântica da unidade.
        assert!(so("prefer_conditional_assignment", "void f(int? x) {\n  if (x == null) x = 1;\n}\n").is_empty());
        assert!(so("join_return_with_assignment", "int f(int x) {\n  x = 1;\n  return x;\n}\n").is_empty());
    }

    #[test]
    fn literais_em_condicoes() {
        let fonte = "void f(int x) {\n  if (true) {}\n  while (true) {}\n  while (1 > 2) {}\n  for (; 1 == 1;) {}\n  if (x > 0) {}\n  switch (x) {\n    case 1 when true:\n      break;\n  }\n}\n";
        assert_eq!(
            so("literal_only_boolean_expressions", fonte),
            vec!["if (true) {}".to_string(), "while (1 > 2) {}".to_string(), "for (; 1 == 1;) {}".to_string(), "when true".to_string()]
        );
    }

    #[test]
    fn construtores() {
        let fonte = "class A {\n  final int? a;\n  int b;\n  A(int? this.a, this.b, int c, int d, int _) : assert(d > 0);\n  A.n(int e) : a = null, b = 0;\n}\n";
        // Os tipos resolvidos e os usos pelo elemento pedem a semântica.
        assert!(so("type_init_formals", fonte).is_empty());
        assert!(so("avoid_unused_constructor_parameters", fonte).is_empty());
    }

    #[test]
    fn nulos_iniciais() {
        let fonte = "int? a = null;\nvar b = null;\nfinal int? c = null;\nint d = 0;\nclass A {\n  int? e;\n  A([this.e = null, int? f = null, int g = 0]);\n}\n";
        // O tipo do elemento pede a semântica da unidade.
        assert!(so("avoid_init_to_null", fonte).is_empty());
    }

    #[test]
    fn const_desnecessario() {
        let fonte = "class A {\n  const A([Object? x]);\n}\nconst a = const A();\nconst b = [const A(), const [1]];\nfinal c = const A(const A());\nfinal d = const A();\n@A(const A())\nvoid f([x = const A()]) {}\n";
        assert_eq!(so("unnecessary_const", fonte).len(), 5);
    }

    #[test]
    fn corpo_na_mesma_linha() {
        let fonte = "void f(bool x) {\n  if (x) return;\n  if (x) {\n    return;\n  } else return;\n  while (x) { f(x); }\n  do f(x); while (x);\n  if (x)\n    return;\n}\n";
        assert_eq!(
            so("always_put_control_body_on_new_line", fonte),
            vec!["return".to_string(), "return".to_string(), "{".to_string(), "f".to_string()]
        );
    }

    #[test]
    fn ordem_das_diretivas() {
        assert_eq!(comparar_diretivas("package:a/z.dart", "package:b/a.dart"), std::cmp::Ordering::Less);
        assert_eq!(comparar_diretivas("b.dart", "a.dart"), std::cmp::Ordering::Greater);
        let fonte = "import 'package:b/b.dart';\nimport 'dart:io';\nimport 'package:a/a.dart';\nexport 'x.dart';\nimport 'z.dart';\nimport 'y.dart';\n";
        assert_eq!(
            so("directives_ordering", fonte),
            vec![
                "import 'dart:io';".to_string(),
                "import 'package:a/a.dart';".to_string(),
                "export 'x.dart';".to_string(),
                "import 'y.dart';".to_string(),
            ]
        );
    }
}
