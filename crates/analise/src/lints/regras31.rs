//! O trigésimo primeiro lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `always_specify_types`: a variável de `for-in` e a de padrão sem tipo
//!   (pela palavra-chave, ou pelo nome), o literal de coleção sem argumentos
//!   de tipo, o tipo nomeado genérico sem argumentos (fora de `is` e de
//!   `@optionalTypeArgs`; também o da criação sem `new`, que aqui é uma
//!   chamada), o parâmetro simples sem tipo (o de closure também, pelo tipo
//!   inferido) e a lista de variáveis sem tipo (pelos tipos dos
//!   inicializadores: o local lido sem a promoção). Os códigos e os
//!   argumentos são os do emissor (`add_type`, `replace_keyword`,
//!   `specify_type`, `split_to_types`).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionRef};
use dartforge_frontend::ast::{self, CollectionElement, ExprId, ExprKind, ForInTarget, ForInit, MemberKind, ParameterKind, PatternKind, StmtKind, TypeKind};
use dartforge_frontend::token::Token;
use dartforge_intern::Interner;
use dartforge_types::exibicao::Exibidor;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId};
use std::collections::HashSet;

/// O token imediatamente antes do offset `pos` (o início de um token).
fn token_antes(t: &[Token], pos: usize) -> Option<Span> {
    let i = t.partition_point(|x| x.span.start < pos);
    (i > 0).then(|| t[i - 1].span)
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    if !ligada("always_specify_types") {
        return out;
    }
    let Ok(tokens) = dartforge_frontend::lexer::lex(u.fonte) else { return out };
    let a = u.ast;
    let fonte = u.fonte;
    let program = s.program;
    let table = s.table;
    let corpo = s.corpo;
    let ex = Exibidor { table, interner, program };
    // O argumento `DartType` (com alias) e o texto de `getDisplayString()`.
    let tipo = |t: TypeId| ex.tipo(t, true);
    let texto = |t: TypeId| ex.tipo(t, false);
    let e_dynamic = |t: TypeId| matches!(table.get(t), Type::Dynamic);
    let mut achados: Vec<(Span, &'static CodigoLint, Vec<String>)> = Vec::new();
    let palavra = |sp: Span| &fonte[sp.start..sp.end];

    // `visitDeclaredIdentifier`: a variável de `for (var x in …)`.
    let mut declaradas: Vec<(bool, bool, ast::Name)> = Vec::new();
    for st in a.stmts.iter() {
        if let StmtKind::ForIn { target: ForInTarget::Declared { ty: None, name, .. }, .. } = &st.kind {
            declaradas.push((false, false, *name));
        }
    }
    fn elementos(x: &CollectionElement, v: &mut Vec<(bool, bool, ast::Name)>) {
        match x {
            CollectionElement::ForIn { target: ForInTarget::Declared { ty: None, name, .. }, body, .. } => {
                v.push((false, false, *name));
                elementos(body, v);
            }
            CollectionElement::ForIn { body, .. } | CollectionElement::For { body, .. } => elementos(body, v),
            CollectionElement::If { then, else_, .. } => {
                elementos(then, v);
                if let Some(e) = else_ {
                    elementos(e, v);
                }
            }
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
            for x in elements.iter() {
                elementos(x, &mut declaradas);
            }
        }
    }
    for (_, _, name) in declaradas {
        // A palavra-chave: o token antes do nome (`var`, `final`, `const`).
        let Some(k) = token_antes(&tokens, name.span.start) else { continue };
        let p = palavra(k);
        if !matches!(p, "var" | "final" | "const") {
            continue;
        }
        let Some(t) = corpo.tipo_local(name.span.start) else { continue };
        if p == "var" {
            achados.push((k, &c::ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD, vec![p.to_string(), tipo(t)]));
        } else {
            achados.push((k, &c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE, vec![tipo(t)]));
        }
    }

    // `visitDeclaredVariablePattern` (fora dos padrões de atribuição).
    let atribuicoes: Vec<Span> = a
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::PatternAssign { pattern, .. } => Some(a.pattern(*pattern).span),
            _ => None,
        })
        .collect();
    for (k, p) in a.patterns.iter().enumerate() {
        let PatternKind::Variable { final_, var_, ty: None, name } = &p.kind else { continue };
        if atribuicoes.iter().any(|sp| sp.start <= p.span.start && p.span.end <= sp.end) {
            continue;
        }
        let Some(t) = corpo.tipos_casados.get(&ast::PatternId(k as u32)).copied() else { continue };
        let palavra_chave = if *final_ || *var_ { token_antes(&tokens, name.span.start) } else { None };
        match palavra_chave {
            Some(kw) if *var_ => achados.push((kw, &c::ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD, vec![palavra(kw).to_string(), tipo(t)])),
            Some(kw) => achados.push((kw, &c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE, vec![tipo(t)])),
            None => achados.push((name.span, &c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE, vec![tipo(t)])),
        }
    }

    // `checkLiteral`: o literal de coleção sem argumentos de tipo.
    for e in a.exprs.iter() {
        if let ExprKind::List { type_args, .. } | ExprKind::SetOrMap { type_args, .. } = &e.kind
            && type_args.is_empty()
        {
            let i = tokens.partition_point(|x| x.span.start < e.span.start);
            if let Some(t) = tokens.get(i) {
                achados.push((t.span, &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new()));
            }
        }
    }

    // `visitNamedType`: o tipo de interface genérico sem argumentos.
    let anotado_opcional = |metadata: &[ast::Annotation], unidade| metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, unidade, m, "meta", "optionalTypeArgs"));
    // Os parâmetros de tipo do elemento e o `hasOptionalTypeArgs`.
    let generico = |el: Element| -> bool {
        match el {
            Element::Class(c) => {
                let x = program.class(c);
                !x.type_params.is_empty() && !x.decl.is_some_and(|r| anotado_opcional(&program.unit(r.unit).ast.decl(r.decl).metadata, r.unit))
            }
            Element::Typedef(t) => {
                let x = program.typedef(t);
                let r = x.decl;
                !x.type_params.is_empty() && !anotado_opcional(&program.unit(r.unit).ast.decl(r.decl).metadata, r.unit)
            }
            _ => false,
        }
    };
    let de_interface = |t: TypeId| matches!(table.get(t), Type::Interface { .. } | Type::FutureOr { .. } | Type::ExtensionType { .. });
    let em_is: HashSet<ast::TypeId> = a
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::Is { ty, .. } => Some(*ty),
            _ => None,
        })
        .collect();
    for (k, t) in a.types.iter().enumerate() {
        let id = ast::TypeId(k as u32);
        let TypeKind::Named { name, args } = &t.kind else { continue };
        if !args.is_empty() || em_is.contains(&id) {
            continue;
        }
        let Some(r) = corpo.tipos_de_anotacoes.get(&id).copied().or_else(|| s.outline.tipos_escritos.get(&(s.unidade, id)).copied()) else { continue };
        if !de_interface(r) {
            continue;
        }
        let b = match &name[..] {
            [n] => program.lookup_na_unidade(s.unidade, n.sym),
            [p, n] => program.lookup_prefixed_na_unidade(s.unidade, p.sym, n.sym),
            _ => None,
        };
        let Some(el) = b.and_then(|b| b.getter) else { continue };
        if generico(el) {
            achados.push((t.span, &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new()));
        }
    }
    // A criação sem `new` (`Foo()`, `Foo.nome()`, `p.Foo()`): o `NamedType`
    // do `ConstructorName`.
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        let ExprKind::Call { target, arguments } = &e.kind else { continue };
        if !matches!(corpo.get_resolved(id), Some(Resolved::Constructor(_))) || !arguments.type_args.is_empty() {
            continue;
        }
        // O nome do tipo: o identificador (ou `p.Foo`) antes do nome do
        // construtor.
        let mut x = *target;
        if matches!(a.expr(x).kind, ExprKind::TypeArguments { .. }) {
            continue;
        }
        let tipo_no = loop {
            match &a.expr(x).kind {
                ExprKind::Identifier(_) => break Some(x),
                ExprKind::Property { target: t, .. } => {
                    if matches!(corpo.get_resolved(x), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_)))) {
                        break Some(x);
                    }
                    if matches!(a.expr(*t).kind, ExprKind::TypeArguments { .. }) {
                        break None;
                    }
                    x = *t;
                }
                _ => break None,
            }
        };
        let Some(no) = tipo_no else { continue };
        let el = match corpo.get_resolved(no) {
            Some(Resolved::Element(el @ (Element::Class(_) | Element::Typedef(_)))) => *el,
            _ => continue,
        };
        let Some(r) = corpo.get_type(id) else { continue };
        if de_interface(r) && generico(el) {
            achados.push((a.expr(no).span, &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new()));
        }
    }

    // `visitSimpleFormalParameter`.
    // O tipo do elemento de cada parâmetro: o da função declarada, o do
    // construtor, o inferido da closure, o da função local.
    let tipos_da_funcao = |f: ast::FunctionId| -> Option<Vec<TypeId>> {
        let fid = program.functions.iter().position(|x| matches!(x.node, FunctionRef::Function { unit, function } if unit == s.unidade && function == f));
        if let Some(i) = fid {
            return s.outline.functions.get(i).map(|d| d.parameters.iter().map(|p| p.ty).collect());
        }
        let g = a.function(f);
        let tipo = match g.name {
            Some(n) => corpo.tipo_local(n.span.start),
            None => a.exprs.iter().position(|e| matches!(e.kind, ExprKind::FunctionExpression(x) if x == f)).and_then(|k| corpo.get_type(ExprId(k as u32))),
        }?;
        let Type::Function { positional, optional, named, .. } = table.get(tipo) else { return None };
        let ps = g.parameters.as_deref().unwrap_or(&[]);
        let (mut i, mut j) = (0, 0);
        let mut v = Vec::new();
        for p in ps {
            let t = match p.kind {
                ParameterKind::Required => {
                    i += 1;
                    positional.get(i - 1).copied()
                }
                ParameterKind::Optional => {
                    j += 1;
                    optional.get(j - 1).copied()
                }
                ParameterKind::Named => p.nome_externo().and_then(|n| named.iter().find(|(m, _, _)| *m == n.sym).map(|(_, t, _)| *t)),
            };
            v.push(t.unwrap_or(s.core.dynamic_));
        }
        Some(v)
    };
    let mut listas: Vec<(&[ast::Parameter], Option<Vec<TypeId>>)> = Vec::new();
    for (k, f) in a.functions.iter().enumerate() {
        if let Some(ps) = f.parameters.as_deref() {
            listas.push((ps, tipos_da_funcao(ast::FunctionId(k as u32))));
        }
    }
    for (k, m) in a.members.iter().enumerate() {
        if let MemberKind::Constructor(kc) = &m.kind {
            let fid = program.functions.iter().position(|x| matches!(x.node, FunctionRef::Constructor { unit, member } if unit == s.unidade && member == ast::MemberId(k as u32)));
            let tipos = fid.and_then(|i| s.outline.functions.get(i)).map(|d| d.parameters.iter().map(|p| p.ty).collect());
            listas.push((&kc.parameters, tipos));
        }
    }
    // Os parâmetros internos (de parâmetros-função, de tipos de função e de
    // typedef antigo): sem tipo escrito, `dynamic`.
    let mut internas: Vec<&[ast::Parameter]> = Vec::new();
    fn aninhadas<'x>(ps: &'x [ast::Parameter], v: &mut Vec<&'x [ast::Parameter]>) {
        for p in ps {
            if let Some(i) = &p.function_parameters {
                v.push(i);
                aninhadas(i, v);
            }
        }
    }
    for (ps, _) in listas.iter() {
        aninhadas(ps, &mut internas);
    }
    for t in a.types.iter() {
        if let TypeKind::Function { parameters, .. } = &t.kind {
            internas.push(parameters);
            aninhadas(parameters, &mut internas);
        }
    }
    for d in a.decls.iter() {
        if let ast::DeclKind::Typedef(x) = &d.kind
            && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
        {
            internas.push(parameters);
            aninhadas(parameters, &mut internas);
        }
    }
    for ps in internas {
        listas.push((ps, None));
    }
    for (ps, tipos) in listas {
        for (j, p) in ps.iter().enumerate() {
            // Só o `SimpleFormalParameter`.
            if p.this_ || p.super_ || p.function_parameters.is_some() || p.ty.is_some() {
                continue;
            }
            let Some(n) = p.name else { continue };
            let t = interner.resolve(n.sym);
            if !t.is_empty() && t.bytes().all(|b| b == b'_') {
                continue;
            }
            let tipo_do_param = tipos.as_ref().and_then(|v| v.get(j).copied()).unwrap_or(s.core.dynamic_);
            if p.var_ || p.final_ || p.const_ {
                let Some(kw) = token_antes(&tokens, n.span.start) else { continue };
                if p.var_ && !e_dynamic(tipo_do_param) {
                    achados.push((kw, &c::ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD, vec![palavra(kw).to_string(), tipo(tipo_do_param)]));
                } else {
                    achados.push((kw, &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new()));
                }
            } else {
                let no = Span { start: p.span.start, end: n.span.end };
                if e_dynamic(tipo_do_param) {
                    achados.push((no, &c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new()));
                } else {
                    achados.push((no, &c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE, vec![tipo(tipo_do_param)]));
                }
            }
        }
    }

    // `visitVariableDeclarationList`.
    let mut listas_de_variaveis: Vec<&ast::VariableList> = Vec::new();
    for d in a.decls.iter() {
        if let ast::DeclKind::Variables(l) = &d.kind {
            listas_de_variaveis.push(l);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind {
            listas_de_variaveis.push(l);
        }
    }
    for st in a.stmts.iter() {
        match &st.kind {
            StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => listas_de_variaveis.push(l),
            _ => {}
        }
    }
    fn de_colecao<'x>(x: &'x CollectionElement, v: &mut Vec<&'x ast::VariableList>) {
        match x {
            CollectionElement::For { init: Some(ForInit::Variables(l)), body, .. } => {
                v.push(l);
                de_colecao(body, v);
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => de_colecao(body, v),
            CollectionElement::If { then, else_, .. } => {
                de_colecao(then, v);
                if let Some(e) = else_ {
                    de_colecao(e, v);
                }
            }
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
            for x in elements.iter() {
                de_colecao(x, &mut listas_de_variaveis);
            }
        }
    }
    for l in listas_de_variaveis {
        if l.ty.is_some() || !(l.var_ || l.final_ || l.const_) {
            continue;
        }
        let Some(primeira) = l.variables.first() else { continue };
        let Some(kw) = token_antes(&tokens, primeira.name.span.start) else { continue };
        let p = palavra(kw);
        if !matches!(p, "var" | "final" | "const") {
            continue;
        }
        // `_getTypes`: o tipo de cada inicializador (o local lido pelo tipo
        // do elemento, sem a promoção).
        let mut tipos: Vec<String> = Vec::new();
        for v in l.variables.iter() {
            let Some(i) = v.initializer else { continue };
            let t = match corpo.get_resolved(i) {
                Some(Resolved::Local(_)) if matches!(a.expr(i).kind, ExprKind::Identifier(_)) => corpo.declaracao_local(i).and_then(|d| corpo.tipo_local(d)),
                _ => None,
            }
            .or_else(|| corpo.get_type(i));
            if let Some(t) = t {
                let x = texto(t);
                if !tipos.contains(&x) {
                    tipos.push(x);
                }
            }
        }
        let unico = tipos.len() == 1;
        let (codigo, args): (&'static CodigoLint, Vec<String>) = if tipos.is_empty() {
            (&c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new())
        } else if p == "var" {
            if unico {
                (&c::ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD, vec![p.to_string(), tipos[0].clone()])
            } else {
                (&c::ALWAYS_SPECIFY_TYPES_SPLIT_TO_TYPES, Vec::new())
            }
        } else if unico {
            (&c::ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE, vec![tipos[0].clone()])
        } else {
            (&c::ALWAYS_SPECIFY_TYPES_ADD_TYPE, Vec::new())
        };
        achados.push((kw, codigo, args));
    }

    achados.sort_by_key(|(sp, _, _)| (sp.start, sp.end));
    for (span, codigo, args) in achados {
        out.push(RelatoDeLint { codigo, span, args });
    }
    out
}
