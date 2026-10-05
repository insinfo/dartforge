//! O nono lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `unnecessary_final`, `hash_and_equals`, `unnecessary_getters_setters`,
//! `recursive_getters` e `prefer_initializing_formals`.
//!
//! Diferenças conhecidas (onde o original resolve um nome, aqui ele é
//! procurado entre os campos e parâmetros da própria declaração):
//! - `unnecessary_getters_setters`: o campo embrulhado tem de ser da mesma
//!   declaração, e "mesmo tipo" é o texto do tipo do campo igual ao do
//!   parâmetro do setter (os dois escritos). Campo herdado, tipo inferido
//!   ou alias não são vistos, e nada se relata.
//! - `recursive_getters`: a referência ao próprio getter é o nome solto ou
//!   `this.nome`; um getter cujo corpo declara variável, parâmetro ou
//!   padrão com o mesmo nome é deixado de fora inteiro.
//! - `prefer_initializing_formals`: o lado esquerdo é `this.x` com `x`
//!   campo declarado na mesma classe.
//! - `hash_and_equals` não soma os membros de augmentations.
//! Escrito sem compilar nem executar (2026-10-05).

use super::andar::{andar, No};
use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, AssignOp, Ast, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody, FunctionId, FunctionKind, Initializer,
    ListPatternElement, MemberId, MemberKind, PatternId, PatternKind, StmtKind,
};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// As variáveis declaradas num padrão (os `DeclaredVariablePattern`).
pub(super) fn variaveis_do_padrao(a: &Ast, p: PatternId, saida: &mut Vec<PatternId>) {
    match &a.pattern(p).kind {
        PatternKind::Variable { .. } => saida.push(p),
        PatternKind::Wildcard { .. } | PatternKind::Constant(_) | PatternKind::Relational { .. } => {}
        PatternKind::Or(x, y) | PatternKind::And(x, y) => {
            variaveis_do_padrao(a, *x, saida);
            variaveis_do_padrao(a, *y, saida);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => {
            variaveis_do_padrao(a, *x, saida)
        }
        PatternKind::List { elements, .. } => {
            for el in elements.iter() {
                if let ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) = el {
                    variaveis_do_padrao(a, *x, saida);
                }
            }
        }
        PatternKind::Map { entries, .. } => {
            for en in entries.iter() {
                variaveis_do_padrao(a, en.value, saida);
            }
        }
        PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
            for campo in fields.iter() {
                variaveis_do_padrao(a, campo.pattern, saida);
            }
        }
    }
}

/// O lugar da palavra `final` em `fonte[de..ate]` (a última: depois de
/// anotações e de `late`/`required`/`covariant`).
fn palavra_final(fonte: &str, de: usize, ate: usize) -> Option<Span> {
    let trecho = fonte.get(de..ate)?;
    let mut busca = trecho.len();
    while let Some(i) = trecho[..busca].rfind("final") {
        let antes = trecho[..i].bytes().next_back();
        let depois = trecho.as_bytes().get(i + 5).copied();
        let de_palavra = |c: u8| c == b'_' || c == b'$' || c.is_ascii_alphanumeric();
        if !antes.is_some_and(de_palavra) && !depois.is_some_and(de_palavra) {
            return Some(Span { start: de + i, end: de + i + 5 });
        }
        busca = i;
    }
    None
}

/// A única expressão do corpo: a de `=> e`, ou a do único comando do bloco
/// (um `return e;` se `de_retorno`, senão um comando de expressão).
fn expressao_unica(a: &Ast, corpo: &FunctionBody, de_retorno: bool) -> Option<ExprId> {
    match corpo {
        FunctionBody::Expression(e) => Some(*e),
        FunctionBody::Block(s) => {
            let StmtKind::Block(comandos) = &a.stmt(*s).kind else { return None };
            let [unico] = &comandos[..] else { return None };
            match &a.stmt(*unico).kind {
                StmtKind::Return(e) if de_retorno => *e,
                StmtKind::Expression(e) if !de_retorno => Some(*e),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, _sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let compacto = |s: Span| -> String { fonte.get(s.start..s.end).unwrap_or("").chars().filter(|x| !x.is_whitespace()).collect() };
    let com_tipo = |tem: bool| if tem { &c::UNNECESSARY_FINAL_WITH_TYPE } else { &c::UNNECESSARY_FINAL_WITHOUT_TYPE };

    // `unnecessary_final`: parâmetros, variáveis locais, a variável do
    // `for-in` e as variáveis de padrão.
    if ligada("unnecessary_final") {
        // Toda lista de parâmetros (o `FormalParameterList`), com as
        // aninhadas e as dos tipos função.
        fn listas<'x>(lista: &'x [ast::Parameter], saida: &mut Vec<&'x [ast::Parameter]>) {
            saida.push(lista);
            for p in lista {
                if let Some(internos) = &p.function_parameters {
                    listas(internos, saida);
                }
            }
        }
        let mut todas: Vec<&[ast::Parameter]> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                listas(ps, &mut todas);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                listas(&k.parameters, &mut todas);
            }
        }
        for t in a.types.iter() {
            if let ast::TypeKind::Function { parameters, .. } = &t.kind {
                listas(parameters, &mut todas);
            }
        }
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
            {
                listas(parameters, &mut todas);
            }
        }
        for lista in todas {
            // O parâmetro-função não tem a palavra (`getParameterDetails`).
            for p in lista.iter().filter(|p| p.final_ && p.function_parameters.is_none()) {
                let ate = p.ty.map(|t| a.ty(t).span.start).or(p.name.map(|n| n.span.start)).unwrap_or(p.span.end);
                if let Some(palavra) = palavra_final(fonte, p.span.start, ate) {
                    relatar(com_tipo(p.ty.is_some()), palavra, &[]);
                }
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) if l.final_ => {
                    let ate = l.ty.map(|t| a.ty(t).span.start).or(l.variables.first().map(|v| v.name.span.start)).unwrap_or(s.span.end);
                    if let Some(palavra) = palavra_final(fonte, s.span.start, ate) {
                        relatar(com_tipo(l.ty.is_some()), palavra, &[]);
                    }
                }
                StmtKind::ForIn { target: ForInTarget::Declared { final_: true, ty, name, .. }, .. } => {
                    let ate = ty.map_or(name.span.start, |t| a.ty(t).span.start);
                    if let Some(palavra) = palavra_final(fonte, s.span.start, ate) {
                        relatar(com_tipo(ty.is_some()), palavra, &[]);
                    }
                }
                StmtKind::ForIn { target: ForInTarget::Pattern { final_: true, pattern }, .. } => {
                    if let Some(palavra) = palavra_final(fonte, s.span.start, a.pattern(*pattern).span.start) {
                        relatar(&c::UNNECESSARY_FINAL_WITHOUT_TYPE, palavra, &[]);
                    }
                }
                // `final (a, b) = …`: cada variável do padrão relata na
                // palavra da declaração (o condutor tira os repetidos).
                StmtKind::PatternVariables { final_: true, pattern, .. } | StmtKind::For { init: Some(ForInit::Pattern { final_: true, pattern, .. }), .. } => {
                    let mut variaveis = Vec::new();
                    variaveis_do_padrao(a, *pattern, &mut variaveis);
                    let sem_palavra = variaveis.iter().any(|v| !matches!(a.pattern(*v).kind, PatternKind::Variable { final_: true, .. } | PatternKind::Variable { var_: true, .. }));
                    if sem_palavra && let Some(palavra) = palavra_final(fonte, s.span.start, a.pattern(*pattern).span.start) {
                        relatar(&c::UNNECESSARY_FINAL_WITH_TYPE, palavra, &[]);
                    }
                }
                _ => {}
            }
        }
        // `case final x`, `final int x` num padrão: a palavra é a do padrão.
        for p in a.patterns.iter() {
            if let PatternKind::Variable { final_: true, name, ty, .. } = &p.kind {
                let ate = ty.map_or(name.span.start, |t| a.ty(t).span.start);
                if let Some(palavra) = palavra_final(fonte, p.span.start, ate) {
                    relatar(&c::UNNECESSARY_FINAL_WITH_TYPE, palavra, &[]);
                }
            }
        }
    }

    // Os membros de cada classe, enum e extension type, e os campos por nome.
    struct Declaracao<'x> {
        classe: bool,
        tipo_de_extensao: bool,
        aumento: bool,
        membros: &'x [MemberId],
        /// Nome do campo → (tipo escrito, é `final` ou `const`).
        campos: HashMap<SymbolId, (Option<ast::TypeId>, bool)>,
    }
    let declaracoes: Vec<Declaracao<'_>> = a
        .decls
        .iter()
        .filter_map(|d| {
            let (membros, classe, tipo_de_extensao): (&[MemberId], bool, bool) = match &d.kind {
                DeclKind::Class(x) => (&x.members[..], true, false),
                DeclKind::Enum(x) => (&x.members[..], false, false),
                DeclKind::ExtensionType(x) => (&x.members[..], false, true),
                _ => return None,
            };
            let mut campos = HashMap::new();
            for &m in membros {
                if let MemberKind::Field(l) = &a.member(m).kind {
                    for v in l.variables.iter() {
                        campos.insert(v.name.sym, (l.ty, l.final_ || l.const_));
                    }
                }
            }
            Some(Declaracao { classe, tipo_de_extensao, aumento: d.augment, membros, campos })
        })
        .collect();

    // `hash_and_equals`: `==` sem `hashCode`, ou o inverso, na classe.
    if ligada("hash_and_equals") {
        for d in declaracoes.iter().filter(|d| d.classe) {
            let mut igual: Option<Span> = None;
            let mut hash: Option<Span> = None;
            for &m in d.membros {
                match &a.member(m).kind {
                    MemberKind::Method(f) => {
                        let Some(n) = a.function(*f).name else { continue };
                        match interner.resolve(n.sym) {
                            "==" => igual = Some(n.span),
                            "hashCode" => hash = Some(n.span),
                            _ => {}
                        }
                    }
                    MemberKind::Field(l) => {
                        if let Some(v) = l.variables.iter().find(|v| interner.resolve(v.name.sym) == "hashCode") {
                            hash = Some(v.name.span);
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
            match (igual, hash) {
                (None, Some(h)) => relatar(&c::HASH_AND_EQUALS, h, &["==", "hashCode"]),
                (Some(i), None) => relatar(&c::HASH_AND_EQUALS, i, &["hashCode", "=="]),
                _ => {}
            }
        }
    }
    // `unnecessary_getters_setters`: o par que só embrulha um campo privado.
    if ligada("unnecessary_getters_setters") {
        for d in declaracoes.iter().filter(|d| (d.classe || d.tipo_de_extensao) && !d.aumento) {
            let mut getters: HashMap<SymbolId, MemberId> = HashMap::new();
            let mut setters: HashMap<SymbolId, MemberId> = HashMap::new();
            for &m in d.membros {
                if let MemberKind::Method(f) = &a.member(m).kind
                    && let Some(n) = a.function(*f).name
                {
                    match a.function(*f).kind {
                        FunctionKind::Getter => {
                            getters.insert(n.sym, m);
                        }
                        FunctionKind::Setter => {
                            setters.insert(n.sym, m);
                        }
                        _ => {}
                    }
                }
            }
            for (nome, &g) in getters.iter() {
                let Some(&s) = setters.get(nome) else { continue };
                let (MemberKind::Method(fg), MemberKind::Method(fs)) = (&a.member(g).kind, &a.member(s).kind) else { continue };
                if !a.member(g).metadata.is_empty() || !a.member(s).metadata.is_empty() {
                    continue;
                }
                let (getter, setter) = (a.function(*fg), a.function(*fs));
                // O getter devolve um campo privado da declaração.
                let devolve_campo = expressao_unica(a, &getter.body, true).is_some_and(|e| {
                    matches!(&a.expr(e).kind, ExprKind::Identifier(n) if interner.resolve(n.sym).starts_with('_') && d.campos.contains_key(&n.sym))
                });
                // O setter atribui o seu único parâmetro a um campo da
                // declaração, de mesmo tipo.
                let atribui_campo = expressao_unica(a, &setter.body, false).is_some_and(|e| {
                    let ExprKind::Assign { op: AssignOp::Assign, target, value } = &a.expr(e).kind else { return false };
                    let (ExprKind::Identifier(alvo), ExprKind::Identifier(valor)) = (&a.expr(*target).kind, &a.expr(*value).kind) else { return false };
                    let Some([parametro]) = setter.parameters.as_deref() else { return false };
                    let Some(&(tipo_do_campo, imutavel)) = d.campos.get(&alvo.sym) else { return false };
                    !imutavel
                        && parametro.name.is_some_and(|n| n.sym == valor.sym)
                        && matches!((tipo_do_campo, parametro.ty), (Some(x), Some(y)) if compacto(a.ty(x).span) == compacto(a.ty(y).span))
                });
                if devolve_campo && atribui_campo && let Some(n) = getter.name {
                    relatar(&c::UNNECESSARY_GETTERS_SETTERS, n.span, &[]);
                }
            }
        }
    }
    // `prefer_initializing_formals`: `this.x = x;` no corpo e `x = x` na
    // lista de inicializadores, com `x` parâmetro e campo público.
    if ligada("prefer_initializing_formals") {
        for d in declaracoes.iter() {
            for &m in d.membros {
                let MemberKind::Constructor(k) = &a.member(m).kind else { continue };
                if k.factory {
                    continue;
                }
                let e_parametro = |s: SymbolId| k.parameters.iter().any(|p| p.name.is_some_and(|n| n.sym == s));
                let publico = |s: SymbolId| !interner.resolve(s).starts_with('_');
                if let FunctionBody::Block(corpo) = &k.body
                    && let StmtKind::Block(comandos) = &a.stmt(*corpo).kind
                {
                    for &s in comandos.iter() {
                        let StmtKind::Expression(e) = &a.stmt(s).kind else { continue };
                        let ExprKind::Assign { target, value, .. } = &a.expr(*e).kind else { continue };
                        let ExprKind::Property { target: receptor, name, null_aware: false } = &a.expr(*target).kind else { continue };
                        if matches!(a.expr(*receptor).kind, ExprKind::This)
                            && matches!(&a.expr(*value).kind, ExprKind::Identifier(v) if v.sym == name.sym)
                            && publico(name.sym)
                            && d.campos.contains_key(&name.sym)
                            && e_parametro(name.sym)
                        {
                            relatar(&c::PREFER_INITIALIZING_FORMALS, a.expr(*e).span, &[interner.resolve(name.sym)]);
                        }
                    }
                }
                for i in k.initializers.iter() {
                    if let Initializer::Field { span, name, value, .. } = i
                        && matches!(&a.expr(*value).kind, ExprKind::Identifier(v) if v.sym == name.sym)
                        && publico(name.sym)
                        && e_parametro(name.sym)
                    {
                        relatar(&c::PREFER_INITIALIZING_FORMALS, *span, &[interner.resolve(name.sym)]);
                    }
                }
            }
        }
    }
    // `recursive_getters`: o getter que se refere a si mesmo.
    if ligada("recursive_getters") {
        // Por getter: os achados e se o corpo declara o mesmo nome.
        let mut por_getter: HashMap<FunctionId, (Vec<Span>, bool)> = HashMap::new();
        // O getter mais próximo entre os ancestrais, com o nome.
        let getter_de = |pilha: &[No]| -> Option<(usize, FunctionId, SymbolId)> {
            pilha.iter().enumerate().rev().find_map(|(i, n)| match n {
                No::Funcao(f) if a.function(*f).kind == FunctionKind::Getter => a.function(*f).name.map(|n| (i, *f, n.sym)),
                _ => None,
            })
        };
        andar(u, &mut |no, pilha| {
            let Some((indice, f, nome)) = getter_de(pilha) else { return };
            let registro = por_getter.entry(f).or_default();
            match no {
                No::Expr(e) => match &a.expr(e).kind {
                    ExprKind::Identifier(n) if n.sym == nome => {
                        // Dentro de literal `const` nada é visitado.
                        let em_const = pilha[indice..].iter().any(|x| {
                            matches!(x, No::Expr(y) if matches!(a.expr(*y).kind, ExprKind::List { const_: true, .. } | ExprKind::SetOrMap { const_: true, .. }))
                        });
                        // O prefixo de `g.x` não conta (`PrefixedIdentifier`).
                        let prefixo = matches!(pilha.last(), Some(No::Expr(p)) if matches!(&a.expr(*p).kind, ExprKind::Property { target, .. } if *target == e));
                        if !em_const && !prefixo {
                            registro.0.push(n.span);
                        }
                    }
                    ExprKind::Property { target, name, .. } if name.sym == nome && matches!(a.expr(*target).kind, ExprKind::This) => {
                        registro.0.push(name.span);
                    }
                    _ => {}
                },
                No::Stmt(s) => match &a.stmt(s).kind {
                    StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                        registro.1 |= l.variables.iter().any(|v| v.name.sym == nome);
                    }
                    StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => registro.1 |= name.sym == nome,
                    StmtKind::Function(g) => registro.1 |= a.function(*g).name.is_some_and(|n| n.sym == nome),
                    StmtKind::Try { catches, .. } => {
                        registro.1 |= catches.iter().any(|k| k.exception.is_some_and(|n| n.sym == nome) || k.stack_trace.is_some_and(|n| n.sym == nome));
                    }
                    _ => {}
                },
                No::Funcao(g) => {
                    registro.1 |= a.function(g).parameters.iter().flat_map(|ps| ps.iter()).any(|p| p.name.is_some_and(|n| n.sym == nome));
                }
                No::Padrao(p) => {
                    registro.1 |= matches!(&a.pattern(p).kind, PatternKind::Variable { name, .. } if name.sym == nome);
                }
                _ => {}
            }
        });
        for (f, (achados, sombreado)) in por_getter {
            if sombreado {
                continue;
            }
            let nome = a.function(f).name.map_or("", |n| interner.resolve(n.sym));
            for s in achados {
                relatar(&c::RECURSIVE_GETTERS, s, &[nome]);
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_codigo(regra: &str, fonte: &str) -> Vec<(&'static str, String)> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut relatos = executar(u, &nomes, &|r| r == regra, None);
        relatos.sort_by_key(|r| (r.span.start, r.span.end));
        relatos.dedup_by_key(|r| (r.span.start, r.span.end));
        relatos.into_iter().map(|r| (r.codigo.unico, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    fn so(regra: &str, fonte: &str) -> Vec<String> {
        com_codigo(regra, fonte).into_iter().map(|x| x.1).collect()
    }

    #[test]
    fn final_desnecessario() {
        let fonte = "void f(final int a, final b, List<int> l) {\n  final x = 1;\n  final int y = 2;\n  var z = 3;\n  for (final e in l) {}\n  final (p, q) = (1, 2);\n}\nclass A {\n  final int c = 0;\n}\n";
        let v = com_codigo("unnecessary_final", fonte);
        let codigos: Vec<&str> = v.iter().map(|x| x.0).collect();
        assert_eq!(
            codigos,
            vec![
                "unnecessary_final_with_type",
                "unnecessary_final_without_type",
                "unnecessary_final_without_type",
                "unnecessary_final_with_type",
                "unnecessary_final_without_type",
                "unnecessary_final_with_type",
            ]
        );
        assert!(v.iter().all(|x| x.1 == "final"));
    }

    #[test]
    fn igualdade_e_hash() {
        assert_eq!(
            so("hash_and_equals", "class A {\n  @override\n  bool operator ==(Object o) => true;\n}\nclass B {\n  @override\n  int get hashCode => 0;\n}\nclass C {\n  @override\n  bool operator ==(Object o) => true;\n  @override\n  final int hashCode = 0;\n}\n"),
            vec!["==".to_string(), "hashCode".to_string()]
        );
    }

    #[test]
    fn getters_e_setters() {
        let fonte = "class A {\n  int _x = 0;\n  int get x => _x;\n  set x(int v) {\n    _x = v;\n  }\n  int _y = 0;\n  int get y => _y;\n  set y(int v) => _y = v + 1;\n}\n";
        assert_eq!(so("unnecessary_getters_setters", fonte), vec!["x".to_string()]);
        let fonte = "class A {\n  int get a => a;\n  int get b => this.b + 1;\n  int get c {\n    final c = 1;\n    return c;\n  }\n  int get d => e;\n  int get e => 0;\n}\n";
        assert_eq!(so("recursive_getters", fonte), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn formais_inicializadores() {
        let fonte = "class A {\n  int x;\n  int y;\n  int _z;\n  A(int x, int y, int z)\n      : y = y,\n        _z = z,\n        x = 0 {\n    this.x = x;\n  }\n  factory A.f(int x) => A(x, 0, 0);\n}\n";
        assert_eq!(so("prefer_initializing_formals", fonte), vec!["y = y".to_string(), "this.x = x".to_string()]);
    }
}
