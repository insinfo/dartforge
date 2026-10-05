//! O décimo segundo lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_if_elements_to_conditional_expressions`,
//! `prefer_for_elements_to_map_fromIterable`, `prefer_final_parameters`,
//! `prefer_foreach` e `prefer_asserts_in_initializer_lists`.
//!
//! Com o mesmo dado do original (as que pedem o elemento só relatam com a
//! semântica da unidade):
//! - `prefer_for_elements_to_map_fromIterable`: o construtor resolvido do
//!   `Map` do `dart:core`; o parâmetro obrigatório também nomeado.
//! - `prefer_final_parameters`: a mutação pelo elemento.
//! - `prefer_foreach`: o argumento e o alvo pelo elemento; os blocos de um
//!   comando só atravessados.
//! - `prefer_asserts_in_initializer_lists`: os métodos e acessores de
//!   instância das classes do conjunto pelo elemento (também como nome de
//!   propriedade), o conjunto da última classe visitada, os campos dos
//!   `this.x` (e pelo `super.x`).
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, Ast, CollectionElement, DeclKind, ExprId, ExprKind, ForInTarget, FunctionBody, MemberKind, Parameter, StmtKind};
use dartforge_intern::Interner;

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

fn sem_parenteses(a: &Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = a.expr(e).kind {
        e = x;
    }
    e
}

/// O corpo de uma única expressão (`=> e`, ou `{ return e; }`).
fn corpo_de_uma_expressao(a: &Ast, b: &FunctionBody) -> bool {
    match b {
        FunctionBody::Expression(_) => true,
        FunctionBody::Block(s) => match &a.stmt(*s).kind {
            StmtKind::Block(l) => matches!(&l[..], [x] if matches!(a.stmt(*x).kind, StmtKind::Return(_))),
            _ => false,
        },
        _ => false,
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

    // `prefer_if_elements_to_conditional_expressions`: `c ? a : b`
    // (talvez entre parênteses) como elemento direto de lista ou conjunto.
    if ligada("prefer_if_elements_to_conditional_expressions") {
        for e in a.exprs.iter() {
            let (elementos, conjunto) = match &e.kind {
                ExprKind::List { elements, .. } => (elements, true),
                ExprKind::SetOrMap { elements, type_args, .. } => {
                    let mapa = type_args.len() == 2
                        || (type_args.is_empty()
                            && (elements.is_empty() || elements.iter().any(|x| matches!(x, CollectionElement::MapEntry { .. }))));
                    (elements, !mapa)
                }
                _ => continue,
            };
            if !conjunto {
                continue;
            }
            for el in elements_iter(elementos) {
                if let CollectionElement::Expression(x) = el
                    && matches!(a.expr(sem_parenteses(a, *x)).kind, ExprKind::Conditional { .. })
                {
                    relatar(&c::PREFER_IF_ELEMENTS_TO_CONDITIONAL_EXPRESSIONS, a.expr(*x).span, &[]);
                }
            }
        }
    }

    // `prefer_for_elements_to_map_fromIterable`: a criação (com ou sem
    // `new`) resolvida ao construtor `fromIterable` do `Map` do `dart:core`,
    // com três argumentos e os fechos `key:` e `value:` de um parâmetro
    // obrigatório e corpo de uma expressão. Pede a semântica da unidade.
    if ligada("prefer_for_elements_to_map_fromIterable")
        && let Some(s) = sem
    {
        let fecho = |nome: &str, arg: &ast::Argument| -> bool {
            if arg.name.is_none_or(|n| interner.resolve(n.sym) != nome) {
                return false;
            }
            let ExprKind::FunctionExpression(f) = a.expr(sem_parenteses(a, arg.value)).kind else { return false };
            let f = a.function(f);
            // `isRequired`: o posicional obrigatório ou o nomeado `required`.
            let um_requerido = f.parameters.as_ref().is_some_and(|ps| {
                ps.len() == 1 && (ps[0].kind == ast::ParameterKind::Required || (ps[0].kind == ast::ParameterKind::Named && ps[0].required))
            });
            um_requerido && corpo_de_uma_expressao(a, &f.body)
        };
        for (k, e) in a.exprs.iter().enumerate() {
            let argumentos = match &e.kind {
                ExprKind::InstanceCreation { arguments, .. } | ExprKind::Call { arguments, .. } => arguments,
                _ => continue,
            };
            let Some(dartforge_types::resolved::Resolved::Constructor(f)) = s.corpo.get_resolved(ExprId(k as u32)) else { continue };
            let g = s.program.function(*f);
            if interner.resolve(g.name) != "fromIterable" || g.class != s.core.map_class {
                continue;
            }
            let [_, segundo, terceiro] = &argumentos.args[..] else { continue };
            let chave = fecho("key", segundo) || fecho("key", terceiro);
            let valor = fecho("value", terceiro) || fecho("value", segundo);
            if chave && valor {
                relatar(&c::PREFER_FOR_ELEMENTS_TO_MAP_FROMITERABLE, e.span, &[]);
            }
        }
    }

    // `prefer_final_parameters`: o parâmetro (não `final`, `const`, `this.x`
    // nem `super.x`) do construtor, da função e do método que nenhuma
    // escrita muda (pelo elemento). Pede a semântica da unidade.
    if ligada("prefer_final_parameters")
        && let Some(s) = sem
    {
        let mut achados: Vec<(Span, String)> = Vec::new();
        let mut conferir = |ps: &[Parameter]| {
            for p in ps {
                if p.final_ || p.const_ || p.this_ || p.super_ {
                    continue;
                }
                let Some(n) = p.name else { continue };
                if super::mutado(s, a, n.span.start) {
                    continue;
                }
                // O nó do parâmetro sem o valor padrão.
                let fim = match p.default_value {
                    None => p.span.end,
                    Some(_) if p.function_parameters.is_some() => dartforge_frontend::fonte::fim_do_parametro_funcao(fonte, n.span.end),
                    Some(_) => n.span.end,
                };
                achados.push((Span { start: p.span.start, end: fim }, interner.resolve(n.sym).to_string()));
            }
        };
        // Funções de topo, locais, expressões de função e métodos.
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                conferir(ps);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind
                && !k.parte_primaria
            {
                conferir(&k.parameters);
            }
        }
        achados.sort_by_key(|x| (x.0.start, x.0.end));
        achados.dedup_by_key(|x| x.0);
        for (x, nome) in achados {
            relatar(&c::PREFER_FINAL_PARAMETERS, x, &[&nome]);
        }
    }

    // `prefer_foreach`: o `for (… x in e)` cujo corpo (atravessando blocos de
    // um comando só e parênteses) é a chamada com o único argumento `x` (pelo
    // elemento), e, na chamada de método, o alvo não cita `x`. Pede a
    // semântica da unidade.
    if ligada("prefer_foreach")
        && let Some(s) = sem
    {
        for st in a.stmts.iter() {
            let StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, body, .. } = &st.kind else { continue };
            let variavel = name.span.start;
            let e_a_variavel = |e: ExprId| matches!(a.expr(e).kind, ExprKind::Identifier(_)) && s.corpo.declaracao_local(e) == Some(variavel);
            let mut corpo = *body;
            while let StmtKind::Block(l) = &a.stmt(corpo).kind {
                let [unico] = &l[..] else { break };
                corpo = *unico;
            }
            let StmtKind::Expression(x) = &a.stmt(corpo).kind else { continue };
            let ExprKind::Call { target, arguments } = &a.expr(sem_parenteses(a, *x)).kind else { continue };
            let [arg] = &arguments.args[..] else { continue };
            if arg.name.is_some() || !e_a_variavel(arg.value) {
                continue;
            }
            // Na chamada de método, o alvo não pode citar a variável
            // (`_ReferenceFinder`).
            let alvo_cita = match &a.expr(*target).kind {
                ExprKind::Property { target: alvo, .. } => {
                    let r = a.expr(*alvo).span;
                    a.exprs.iter().enumerate().any(|(j, e)| dentro(e.span, r) && e_a_variavel(ExprId(j as u32)))
                }
                _ => false,
            };
            if !alvo_cita {
                relatar(&c::PREFER_FOREACH, st.span, &[]);
            }
        }
    }

    // `prefer_asserts_in_initializer_lists`: os `assert` do começo do corpo
    // em bloco de um construtor (não `factory`) que não pedem a instância:
    // nem `this`, nem identificador (também nome de propriedade) de método
    // ou acessor de instância das classes do conjunto (a classe da última
    // declaração de classe visitada, os mixins e as superclasses), salvo o
    // acessor do campo de um `this.x` (ou do `this.x` do construtor da
    // superclasse, pelo `super.x`). Pede a semântica da unidade.
    if ligada("prefer_asserts_in_initializer_lists")
        && let Some(s) = sem
    {
        use dartforge_elements::model::{ClassId, FunctionKind as Especie};
        use dartforge_types::resolved::{MemberRef, Resolved};
        let p = s.program;
        let classe_da_decl = |d: ast::DeclId| {
            p.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == s.unidade && r.decl == d)).map(|i| ClassId(i as u32))
        };
        let conjunto_de = |c: ClassId| {
            let mut v: std::collections::HashSet<ClassId> = std::collections::HashSet::new();
            let mut pilha = vec![c];
            while let Some(k) = pilha.pop() {
                if v.insert(k) {
                    pilha.extend(p.class(k).mixin_classes.iter().copied());
                    pilha.extend(p.class(k).supertype_class);
                }
            }
            v
        };
        let mut ultima: Option<std::collections::HashSet<ClassId>> = None;
        for &d in u.unit.declarations.iter() {
            let decl = a.decl(d);
            let membros: &[ast::MemberId] = match &decl.kind {
                DeclKind::Class(x) => {
                    ultima = classe_da_decl(d).map(conjunto_de);
                    &x.members
                }
                DeclKind::Enum(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                _ => continue,
            };
            let no_conjunto = |c: Option<ClassId>| c.is_some_and(|c| ultima.as_ref().is_some_and(|v| v.contains(&c)));
            for &m in membros {
                let MemberKind::Constructor(k) = &a.member(m).kind else { continue };
                if k.factory {
                    continue;
                }
                let FunctionBody::Block(b) = &k.body else { continue };
                let StmtKind::Block(comandos) = &a.stmt(*b).kind else { continue };
                // Os campos dos `this.x` (e dos `this.x` da superclasse pelo
                // `super.x`).
                let mut campos: Vec<dartforge_elements::model::VariableId> = Vec::new();
                for (i, q) in k.parameters.iter().enumerate() {
                    let Some(n) = q.name else { continue };
                    if q.this_ {
                        campos.extend(super::campo_da_classe(s, m, n.sym));
                    } else if q.super_
                        && let Some((f, j)) = super::parametro_do_super(s, interner, m, i)
                        && let dartforge_elements::model::FunctionRef::Constructor { unit, member } = p.function(f).node
                        && let MemberKind::Constructor(kk) = &p.unit(unit).ast.member(member).kind
                        && let Some(qq) = kk.parameters.get(j)
                        && qq.this_
                        && let Some(nn) = qq.name
                        && let Some(classe_sup) = p.function(f).class
                    {
                        campos.extend(p.class(classe_sup).fields.iter().copied().find(|v| p.variable(*v).name == nn.sym));
                    }
                }
                let pede_instancia = |e: ExprId| -> bool {
                    match s.corpo.get_resolved(e) {
                        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                            let g = p.function(*f);
                            match g.kind {
                                Especie::Function | Especie::Operator => !g.static_ && no_conjunto(g.class),
                                Especie::Getter | Especie::Setter | Especie::ImplicitAccessor => {
                                    !g.static_ && no_conjunto(g.class) && !g.variable.is_some_and(|v| campos.contains(&v))
                                }
                                _ => false,
                            }
                        }
                        Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                            let x = p.variable(*v);
                            !x.static_ && no_conjunto(x.class) && !campos.contains(v)
                        }
                        _ => false,
                    }
                };
                for &st in comandos.iter() {
                    if !matches!(a.stmt(st).kind, StmtKind::Assert { .. }) {
                        break;
                    }
                    let r = a.stmt(st).span;
                    let usa = a.exprs.iter().enumerate().filter(|(_, e)| dentro(e.span, r)).any(|(j, e)| match &e.kind {
                        ExprKind::This => true,
                        ExprKind::Identifier(_) | ExprKind::Property { .. } => pede_instancia(ExprId(j as u32)),
                        _ => false,
                    });
                    if !usa {
                        relatar(&c::PREFER_ASSERTS_IN_INITIALIZER_LISTS, Span { start: r.start, end: r.start + "assert".len() }, &[]);
                    }
                }
            }
        }
    }

    out
}

/// Os elementos diretos de um literal (os de `if` e `for` não são diretos).
fn elements_iter(l: &[CollectionElement]) -> impl Iterator<Item = &CollectionElement> {
    l.iter()
}
