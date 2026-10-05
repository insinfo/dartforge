//! O nono lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `unnecessary_final`, `hash_and_equals`, `unnecessary_getters_setters`,
//! `recursive_getters` e `prefer_initializing_formals`.
//!
//! As quatro regras que o original decide pelo elemento usam a semântica
//! da unidade (sem ela, não relatam; `hash_and_equals` fica com os membros
//! da própria declaração):
//! - `unnecessary_getters_setters`: o acessor sintético do campo privado da
//!   mesma classe, o tipo de escrita igual ao tipo do valor, o parâmetro pelo
//!   elemento.
//! - `recursive_getters`: o identificador (ou `this.x`) resolvido ao próprio
//!   getter, com as exceções de prefixo, alvo de `?.` e literal constante.
//! - `prefer_initializing_formals`: o campo canônico do alvo e o parâmetro
//!   pelo elemento.
//! - `hash_and_equals`: os membros da classe inteira, com as augmentations.
//! Escrito sem compilar nem executar (2026-10-05).

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
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
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

    // Os membros de cada classe, enum e extension type.
    struct Declaracao<'x> {
        classe: bool,
        tipo_de_extensao: bool,
        aumento: bool,
        membros: &'x [MemberId],
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
            Some(Declaracao { classe, tipo_de_extensao, aumento: d.augment, membros })
        })
        .collect();

    // `hash_and_equals`: a classe que declara `==` sem `hashCode` (campo ou
    // método, em toda a classe aumentada: `allFields`/`allMethods`), ou o
    // inverso. Sem a semântica, os membros são os desta declaração.
    if ligada("hash_and_equals") {
        let simbolo = |s: &str| interner.lookup(s);
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
            // Os membros da classe inteira (com as augmentations).
            let classe = sem.and_then(|s| d.membros.first().and_then(|&m| super::classe_do_membro(s, m)).map(|c| (s, c)));
            let tem_metodo = |nome: &str| match classe {
                Some((s, c)) => simbolo(nome).and_then(|x| s.program.class(c).instance_members.get(&x)).is_some_and(|f| {
                    matches!(s.program.function(*f).kind, dartforge_elements::model::FunctionKind::Function | dartforge_elements::model::FunctionKind::Operator)
                }),
                None => match nome {
                    "==" => igual.is_some(),
                    _ => false,
                },
            };
            let tem_campo = |nome: &str| match classe {
                Some((s, c)) => {
                    let x = s.program.class(c);
                    x.fields.iter().any(|v| interner.resolve(s.program.variable(*v).name) == nome)
                        || simbolo(nome).and_then(|y| x.instance_members.get(&y)).is_some_and(|f| {
                            matches!(s.program.function(*f).kind, dartforge_elements::model::FunctionKind::Getter | dartforge_elements::model::FunctionKind::Setter)
                        })
                }
                None => hash.is_some(),
            };
            match (igual, hash) {
                (None, Some(h)) if !tem_metodo("==") => relatar(&c::HASH_AND_EQUALS, h, &["==", "hashCode"]),
                (Some(i), None) if !tem_campo("hashCode") && !tem_metodo("hashCode") => relatar(&c::HASH_AND_EQUALS, i, &["hashCode", "=="]),
                _ => {}
            }
        }
    }
    // `unnecessary_getters_setters`: o par `get x`/`set x` da classe ou do
    // tipo de extensão (fora de augmentation) sem anotações, com o getter
    // devolvendo o acessor sintético de um campo privado da mesma
    // declaração e o setter atribuindo (`=`) o seu único parâmetro a um
    // acessor sintético, com o tipo de escrita igual ao do parâmetro. Pede a
    // semântica da unidade.
    if ligada("unnecessary_getters_setters")
        && let Some(s) = sem
    {
        use dartforge_types::resolved::{MemberRef, Resolved};
        // O campo (de acessor sintético) que a expressão lê ou escreve.
        let campo_sintetico = |e: ExprId| -> Option<dartforge_elements::model::VariableId> {
            match s.corpo.get_resolved(e)? {
                Resolved::Member { member: MemberRef::Variable(v), .. } => Some(*v),
                Resolved::Member { member: MemberRef::Function(f), .. } => s.program.function(*f).variable,
                _ => None,
            }
        };
        for d in declaracoes.iter().filter(|d| (d.classe || d.tipo_de_extensao) && !d.aumento) {
            let Some(classe) = d.membros.first().and_then(|&m| super::classe_do_membro(s, m)) else { continue };
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
            let mut achados: Vec<Span> = Vec::new();
            for (nome, &g) in getters.iter() {
                let Some(&st) = setters.get(nome) else { continue };
                let (MemberKind::Method(fg), MemberKind::Method(fs)) = (&a.member(g).kind, &a.member(st).kind) else { continue };
                if !a.member(g).metadata.is_empty() || !a.member(st).metadata.is_empty() {
                    continue;
                }
                let (getter, setter) = (a.function(*fg), a.function(*fs));
                // `_checkForSimpleGetter`.
                let devolve_campo = expressao_unica(a, &getter.body, true).is_some_and(|e| {
                    matches!(a.expr(e).kind, ExprKind::Identifier(_))
                        && campo_sintetico(e).is_some_and(|v| {
                            let x = s.program.variable(v);
                            x.class == Some(classe) && interner.resolve(x.name).starts_with('_')
                        })
                });
                // `_checkForSimpleSetter`.
                let atribui_campo = expressao_unica(a, &setter.body, false).is_some_and(|e| {
                    let ExprKind::Assign { op: AssignOp::Assign, target, value } = &a.expr(e).kind else { return false };
                    if !matches!(a.expr(*target).kind, ExprKind::Identifier(_)) || !matches!(a.expr(*value).kind, ExprKind::Identifier(_)) {
                        return false;
                    }
                    let Some(v) = campo_sintetico(*target) else { return false };
                    let Some([parametro]) = setter.parameters.as_deref() else { return false };
                    let mesmo_tipo = match (super::tipo_da_variavel(s, v), s.corpo.get_type(*value)) {
                        (Some(x), Some(y)) => s.table.canonico(x) == s.table.canonico(y),
                        _ => false,
                    };
                    mesmo_tipo && parametro.name.is_some_and(|n| s.corpo.declaracao_local(*value) == Some(n.span.start))
                });
                if devolve_campo && atribui_campo && let Some(n) = getter.name {
                    achados.push(n.span);
                }
            }
            achados.sort_by_key(|x| x.start);
            for x in achados {
                relatar(&c::UNNECESSARY_GETTERS_SETTERS, x, &[]);
            }
        }
    }
    // `prefer_initializing_formals`: no corpo, o comando de atribuição cujo
    // alvo é um campo público, não sintético, da classe do construtor, e
    // cujo valor é o parâmetro de mesmo nome; na lista de inicializadores,
    // o `x = x` com `x` parâmetro e campo público. (A condição de "usado
    // mais de uma vez" do original é sempre verdadeira: o mesmo nome já é
    // exigido.) Pede a semântica da unidade.
    if ligada("prefer_initializing_formals")
        && let Some(s) = sem
    {
        for d in declaracoes.iter() {
            for &m in d.membros {
                let MemberKind::Constructor(k) = &a.member(m).kind else { continue };
                if k.factory {
                    continue;
                }
                let Some(classe) = super::classe_do_membro(s, m) else { continue };
                // O parâmetro do construtor que a expressão lê.
                let parametro = |e: ExprId| -> Option<&ast::Parameter> {
                    let decl = s.corpo.declaracao_local(e)?;
                    k.parameters.iter().find(|p| p.name.is_some_and(|n| n.span.start == decl))
                };
                if let FunctionBody::Block(corpo) = &k.body
                    && let StmtKind::Block(comandos) = &a.stmt(*corpo).kind
                {
                    for &st in comandos.iter() {
                        let StmtKind::Expression(e) = &a.stmt(st).kind else { continue };
                        let ExprKind::Assign { target, value, .. } = &a.expr(*e).kind else { continue };
                        let Some(super::Canonico::Variavel(v)) = super::canonico(s, interner, *target) else { continue };
                        let campo = s.program.variable(v);
                        let Some(p) = parametro(*value).filter(|_| matches!(a.expr(*value).kind, ExprKind::Identifier(_))) else { continue };
                        let Some(n) = p.name else { continue };
                        let nome = interner.resolve(campo.name);
                        if campo.class == Some(classe) && !nome.starts_with('_') && campo.name == n.sym {
                            relatar(&c::PREFER_INITIALIZING_FORMALS, a.expr(*e).span, &[interner.resolve(n.sym)]);
                        }
                    }
                }
                for i in k.initializers.iter() {
                    if let Initializer::Field { span, name, value, .. } = i
                        && matches!(&a.expr(*value).kind, ExprKind::Identifier(v) if v.sym == name.sym)
                        && parametro(*value).is_some()
                        && super::campo_da_classe(s, m, name.sym).is_some()
                        && !interner.resolve(name.sym).starts_with('_')
                    {
                        relatar(&c::PREFER_INITIALIZING_FORMALS, *span, &[interner.resolve(name.sym)]);
                    }
                }
            }
        }
    }
    // `recursive_getters`: no getter de topo ou de membro, o identificador
    // (ou o `this.x`) cujo elemento é o próprio getter, fora de literal
    // `const`, que não é prefixo de `x.y` nem alvo de `x?.y` (o alvo de uma
    // chamada `x.f()` conta). Pede a semântica da unidade.
    if ligada("recursive_getters")
        && let Some(s) = sem
    {
        use dartforge_types::resolved::{MemberRef, Resolved};
        let versao_antiga = !super::versao_ao_menos(s, 3, 0);
        let pais = dartforge_frontend::pais::Pais::novo(a, u.unit, u.fonte, versao_antiga);
        // Os getters (sem lista de parâmetros) e o elemento de cada um.
        let mut getters: Vec<(Span, dartforge_elements::model::FunctionElementId, SymbolId)> = Vec::new();
        for (i, f) in a.functions.iter().enumerate() {
            if f.parameters.is_some() || f.kind != FunctionKind::Getter {
                continue;
            }
            let Some(n) = f.name else { continue };
            let Some(e) = super::elemento_da_funcao(s, FunctionId(i as u32)) else { continue };
            let corpo = match &f.body {
                FunctionBody::Block(b) => a.stmt(*b).span,
                FunctionBody::Expression(x) => a.expr(*x).span,
                _ => continue,
            };
            getters.push((corpo, e, n.sym));
        }
        let do_getter = |e: ExprId, alvo: dartforge_elements::model::FunctionElementId| match s.corpo.get_resolved(e) {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) => {
                *f == alvo
            }
            _ => false,
        };
        // O nó está dentro de um literal de lista, conjunto ou mapa
        // constante (o visitante não desce nele).
        let em_literal_const = |e: ExprId, limite: Span| {
            let mut atual = pais.pai(e);
            while let dartforge_frontend::pais::Pai::Expr(p) = atual {
                let n = a.expr(p);
                if n.span.start < limite.start || n.span.end > limite.end {
                    break;
                }
                let e_const = match &n.kind {
                    ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } => *const_ || pais.em_contexto_constante(a, p),
                    _ => false,
                };
                if e_const {
                    return true;
                }
                atual = pais.pai(p);
            }
            false
        };
        let mut achados: Vec<(Span, SymbolId)> = Vec::new();
        for (k, ex) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let Some(&(corpo, elemento, nome)) = getters.iter().find(|(c, _, _)| c.start <= ex.span.start && ex.span.end <= c.end) else { continue };
            let pai = match pais.pai(id) {
                dartforge_frontend::pais::Pai::Expr(p) => Some(p),
                _ => None,
            };
            match &ex.kind {
                ExprKind::Identifier(n) => {
                    if !do_getter(id, elemento) || em_literal_const(id, corpo) {
                        continue;
                    }
                    if let Some(p) = pai {
                        match &a.expr(p).kind {
                            // Alvo de atribuição ou de `++`/`--`: sem
                            // `staticElement`.
                            ExprKind::Assign { target, .. } if *target == id => continue,
                            ExprKind::Unary {
                                op: dartforge_frontend::ast::UnaryOp::PrefixInc
                                | dartforge_frontend::ast::UnaryOp::PrefixDec
                                | dartforge_frontend::ast::UnaryOp::PostfixInc
                                | dartforge_frontend::ast::UnaryOp::PostfixDec,
                                ..
                            } => continue,
                            // Prefixo de `x.y`/alvo de `x?.y` (não de `x.f()`).
                            ExprKind::Property { target, .. } if *target == id => {
                                let alvo_de_chamada = matches!(pais.pai(p), dartforge_frontend::pais::Pai::Expr(c)
                                    if matches!(&a.expr(c).kind, ExprKind::Call { target, .. } if *target == p));
                                if !alvo_de_chamada {
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    achados.push((n.span, nome));
                }
                // `this.x`.
                ExprKind::Property { target, name, .. } if matches!(a.expr(*target).kind, ExprKind::This) => {
                    if do_getter(id, elemento) && !em_literal_const(id, corpo) && !matches!(pai.map(|p| &a.expr(p).kind), Some(ExprKind::Assign { target, .. }) if *target == id) {
                        achados.push((name.span, nome));
                    }
                }
                _ => {}
            }
        }
        for (x, nome) in achados {
            relatar(&c::RECURSIVE_GETTERS, x, &[interner.resolve(nome)]);
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
        // O acessor e os tipos pedem a semântica da unidade.
        assert!(so("unnecessary_getters_setters", fonte).is_empty());
        let fonte = "class A {\n  int get a => a;\n  int get b => this.b + 1;\n  int get c {\n    final c = 1;\n    return c;\n  }\n  int get d => e;\n  int get e => 0;\n}\n";
        assert!(so("recursive_getters", fonte).is_empty());
    }

    #[test]
    fn formais_inicializadores() {
        let fonte = "class A {\n  int x;\n  int y;\n  int _z;\n  A(int x, int y, int z)\n      : y = y,\n        _z = z,\n        x = 0 {\n    this.x = x;\n  }\n  factory A.f(int x) => A(x, 0, 0);\n}\n";
        // O campo e o parâmetro pelo elemento pedem a semântica da unidade.
        assert!(so("prefer_initializing_formals", fonte).is_empty());
    }
}
