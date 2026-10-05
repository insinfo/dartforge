//! O décimo lote de regras de lint que só olham a árvore e o texto
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_inlined_adds`, `prefer_spread_collections`,
//! `avoid_field_initializers_in_const_classes`,
//! `avoid_classes_with_only_static_members` e `prefer_final_in_for_each`.
//!
//! Com o mesmo dado do original (as que pedem o elemento só relatam com a
//! semântica da unidade):
//! - `avoid_classes_with_only_static_members`: a interface herdada (membros
//!   de classe que não sejam de `Object`; os de mixin não contam), os
//!   construtores, métodos e campos da classe inteira; sem a semântica, só a
//!   classe sem `extends`, `with` nem `implements`.
//! - `prefer_final_in_for_each`: a mutação pelo elemento
//!   (`super::mutado`), só dentro de corpo de função.
//! - `avoid_field_initializers_in_const_classes`: o uso de parâmetro pelo
//!   elemento e os construtores da classe inteira.
//! - `prefer_spread_collections`: a lista-alvo fora de contexto constante.
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, CollectionElement, DeclKind, ExprKind, ForInTarget, FunctionKind, Initializer, ListPatternElement, MemberId, MemberKind,
    ParameterKind, PatternId, PatternKind, StmtKind,
};
use dartforge_intern::Interner;

/// Os `for-in` de coleção com variável declarada ou padrão: o alvo e o fim
/// do iterável (de onde começa a região em que a variável vive).
fn lacos_de_colecao<'x>(a: &Ast, el: &'x CollectionElement, saida: &mut Vec<(&'x ForInTarget, usize)>) {
    match el {
        CollectionElement::ForIn { target, iterable, body, .. } => {
            saida.push((target, a.expr(*iterable).span.end));
            lacos_de_colecao(a, body, saida);
        }
        CollectionElement::For { body, .. } => lacos_de_colecao(a, body, saida),
        CollectionElement::If { then, else_, .. } => {
            lacos_de_colecao(a, then, saida);
            if let Some(x) = else_ {
                lacos_de_colecao(a, x, saida);
            }
        }
        _ => {}
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `prefer_inlined_adds` e `prefer_spread_collections`: a primeira seção
    // de uma cascata sobre um literal de lista.
    let (inlinar, espalhar) = (ligada("prefer_inlined_adds"), ligada("prefer_spread_collections"));
    if inlinar || espalhar {
        // `target.inConstantContext` (só para `prefer_spread_collections`).
        let pais = dartforge_frontend::pais::Pais::novo(a, u.unit, u.fonte, sem.is_some_and(|s| !super::versao_ao_menos(s, 3, 0)));
        for e in a.exprs.iter() {
            let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
            if !matches!(a.expr(*target).kind, ExprKind::List { .. }) {
                continue;
            }
            let Some(&primeira) = sections.first() else { continue };
            let ExprKind::Call { target: chamado, arguments } = &a.expr(primeira).kind else { continue };
            let ExprKind::Property { target: receptor, name, .. } = &a.expr(*chamado).kind else { continue };
            if !matches!(a.expr(*receptor).kind, ExprKind::CascadeTarget) {
                continue;
            }
            let [argumento] = &arguments.args[..] else { continue };
            let de_lista = matches!(a.expr(argumento.value).kind, ExprKind::List { .. });
            match interner.resolve(name.sym) {
                "add" if inlinar => relatar(&c::PREFER_INLINED_ADDS_SINGLE, name.span, &[]),
                "addAll" if de_lista && inlinar => relatar(&c::PREFER_INLINED_ADDS_MULTIPLE, name.span, &[]),
                "addAll" if !de_lista && espalhar && !pais.em_contexto_constante(a, *target) => relatar(&c::PREFER_SPREAD_COLLECTIONS, name.span, &[]),
                _ => {}
            }
        }
    }

    // `avoid_field_initializers_in_const_classes`.
    if ligada("avoid_field_initializers_in_const_classes") {
        for d in a.decls.iter() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let construtores: Vec<(MemberId, &ast::Constructor)> = x
                .members
                .iter()
                .filter_map(|&m| match &a.member(m).kind {
                    MemberKind::Constructor(k) => Some((m, k)),
                    _ => None,
                })
                .collect();
            // Os construtores da classe inteira (`allConstructors`, com as
            // augmentations): pela semântica; sem ela, os desta declaração.
            let classe = sem.and_then(|s| x.members.first().and_then(|&m| super::classe_do_membro(s, m)).map(|c| (s, c)));
            let (quantos, algum_const) = match classe {
                Some((s, c)) => {
                    let k = &s.program.class(c).constructors;
                    (k.len(), k.values().any(|f| s.program.function(*f).const_))
                }
                None => (construtores.len(), construtores.iter().any(|(_, k)| k.const_)),
            };
            // O campo `final` de instância com inicializador, numa classe
            // com algum construtor `const`.
            if algum_const {
                for &m in x.members.iter() {
                    let membro = a.member(m);
                    if let MemberKind::Field(l) = &membro.kind
                        && !membro.augment
                        && !l.static_
                        && l.final_
                    {
                        for v in l.variables.iter() {
                            if let Some(i) = v.initializer {
                                relatar(&c::AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES, Span { start: v.name.span.start, end: a.expr(i).span.end }, &[]);
                            }
                        }
                    }
                }
            }
            // O inicializador de campo do construtor `const` da classe de um
            // só construtor que não lê parâmetro nenhum (pelo elemento: pede
            // a semântica da unidade).
            if let Some(s) = sem
                && quantos == 1
            {
                for (_, k) in construtores.iter().filter(|(_, k)| k.const_) {
                    let declaracoes: Vec<usize> = k.parameters.iter().filter_map(|p| p.name).map(|n| n.span.start).collect();
                    for i in k.initializers.iter() {
                        if let Initializer::Field { span, value, .. } = i {
                            let regiao = a.expr(*value).span;
                            let usa = a.exprs.iter().enumerate().any(|(j, e)| {
                                matches!(e.kind, ExprKind::Identifier(_))
                                    && regiao.start <= e.span.start
                                    && e.span.end <= regiao.end
                                    && s.corpo.declaracao_local(ast::ExprId(j as u32)).is_some_and(|dd| declaracoes.contains(&dd))
                            });
                            if !usa {
                                relatar(&c::AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES, *span, &[]);
                            }
                        }
                    }
                }
            }
        }
    }
    // `avoid_classes_with_only_static_members`: a classe (não `sealed`, fora
    // de augmentation) cuja interface não tem membro de classe que não seja
    // de `Object` (membros de mixin não contam), sem construtor que não seja
    // o padrão, só com métodos estáticos, e com algum método ou campo não
    // `const` (os acessores contam como campo sintético). Pela semântica, a
    // interface herdada e os membros com as augmentations; sem ela, só a
    // classe sem `extends`, `with` nem `implements`.
    if ligada("avoid_classes_with_only_static_members") {
        for d in a.decls.iter().filter(|d| !d.augment) {
            let DeclKind::Class(x) = &d.kind else { continue };
            if x.modifiers.sealed {
                continue;
            }
            let classe = sem.and_then(|s| x.members.first().and_then(|&m| super::classe_do_membro(s, m)).map(|c| (s, c)));
            let relata = match classe {
                Some((s, c)) => {
                    use dartforge_elements::model::{ClassKind, FunctionKind as Especie};
                    let p = s.program;
                    // A interface: a própria classe e os supertipos.
                    let mut tipos: Vec<dartforge_elements::model::ClassId> = vec![c];
                    if let Some(h) = s.outline.hierarchy.get(c) {
                        tipos.extend(h.supertypes.keys().copied());
                    }
                    let herda_de_classe = tipos.iter().any(|&k| {
                        Some(k) != s.core.object_class
                            && matches!(p.class(k).kind, ClassKind::Class | ClassKind::MixinApplication)
                            && p.class(k).instance_members.values().any(|f| {
                                let g = p.function(*f);
                                // O membro é declarado na classe (não vem de mixin).
                                g.class == Some(k)
                            })
                    });
                    let e = p.class(c);
                    let ctor_proprio = e.constructors.iter().any(|(nome, f)| {
                        let sem_nome = interner.resolve(*nome).is_empty();
                        let obrigatorio = s.outline.functions.get(f.0 as usize).is_some_and(|dd| dd.parameters.iter().any(|q| q.kind == ParameterKind::Required || q.required));
                        !(sem_nome && !obrigatorio)
                    });
                    let metodos: Vec<_> = e
                        .instance_members
                        .values()
                        .chain(e.static_members.values())
                        .filter(|f| matches!(p.function(**f).kind, Especie::Function | Especie::Operator))
                        .collect();
                    let todos_estaticos = metodos.iter().all(|f| p.function(**f).static_);
                    let acessor = e.instance_members.values().chain(e.static_members.values()).any(|f| matches!(p.function(*f).kind, Especie::Getter | Especie::Setter));
                    let campo_nao_const = e.fields.iter().any(|v| !p.variable(*v).const_) || acessor;
                    !herda_de_classe && !ctor_proprio && todos_estaticos && (!metodos.is_empty() || campo_nao_const)
                }
                None => {
                    if x.mixin_application || x.extends.is_some() || !x.with.is_empty() || !x.implements.is_empty() {
                        continue;
                    }
                    let mut de_instancia = false;
                    let mut construtor_proprio = false;
                    let mut metodos_estaticos = false;
                    let mut campo_nao_const = false;
                    for &m in x.members.iter() {
                        match &a.member(m).kind {
                            MemberKind::Field(l) => {
                                de_instancia |= !l.static_;
                                campo_nao_const |= !l.const_;
                            }
                            MemberKind::Method(f) => {
                                let f = a.function(*f);
                                de_instancia |= !f.static_;
                                match f.kind {
                                    FunctionKind::Getter | FunctionKind::Setter => campo_nao_const = true,
                                    FunctionKind::Function | FunctionKind::Operator => metodos_estaticos = true,
                                }
                            }
                            MemberKind::Constructor(k) => {
                                construtor_proprio |= k.name.is_some() || k.parameters.iter().any(|p| p.kind == ParameterKind::Required || p.required);
                            }
                        }
                    }
                    !de_instancia && !construtor_proprio && (metodos_estaticos || campo_nao_const)
                }
            };
            if relata {
                relatar(&c::AVOID_CLASSES_WITH_ONLY_STATIC_MEMBERS, d.span, &[]);
            }
        }
    }
    // `prefer_final_in_for_each`: a variável (ou as variáveis do padrão) de
    // um `for-in` dentro de corpo de função que nenhuma escrita muda (pelo
    // elemento: pede a semântica da unidade).
    if ligada("prefer_final_in_for_each")
        && let Some(s) = sem
    {
        // `potentiallyMutates`: só a variável declarada e não mutada passa.
        let quieto = |p: PatternId| matches!(&a.pattern(p).kind, PatternKind::Variable { name, .. } if !super::mutado(s, a, name.span.start));
        let mut conferir = |alvo: &ForInTarget| match alvo {
            ForInTarget::Declared { final_: false, name, .. } => {
                if !super::mutado(s, a, name.span.start) {
                    relatar(&c::PREFER_FINAL_IN_FOR_EACH_VARIABLE, name.span, &[interner.resolve(name.sym)]);
                }
            }
            ForInTarget::Pattern { final_: false, pattern } => {
                let n = a.pattern(*pattern);
                let relata = match &n.kind {
                    PatternKind::Record { fields } | PatternKind::Object { fields, .. } => fields.iter().all(|f| quieto(f.pattern)),
                    PatternKind::List { elements, .. } => elements.iter().all(|el| matches!(el, ListPatternElement::Pattern(x) if quieto(*x))),
                    PatternKind::Map { entries, rest, .. } => !*rest && entries.iter().all(|en| quieto(en.value)),
                    _ => false,
                };
                if relata {
                    relatar(&c::PREFER_FINAL_IN_FOR_EACH_PATTERN, n.span, &[]);
                }
            }
            _ => {}
        };
        for st in a.stmts.iter() {
            if let StmtKind::ForIn { target, .. } = &st.kind {
                conferir(target);
            }
        }
        // O `for-in` de coleção, só dentro de corpo de função.
        let corpos: Vec<Span> = a
            .functions
            .iter()
            .filter_map(|f| match &f.body {
                ast::FunctionBody::Block(b) => Some(a.stmt(*b).span),
                ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                _ => None,
            })
            .chain(a.members.iter().filter_map(|m| match &m.kind {
                MemberKind::Constructor(k) => match &k.body {
                    ast::FunctionBody::Block(b) => Some(a.stmt(*b).span),
                    ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                    _ => None,
                },
                _ => None,
            }))
            .collect();
        for e in a.exprs.iter() {
            if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
                if !corpos.iter().any(|c| c.start <= e.span.start && e.span.end <= c.end) {
                    continue;
                }
                let mut lacos = Vec::new();
                for el in elements.iter() {
                    lacos_de_colecao(a, el, &mut lacos);
                }
                for (alvo, _) in lacos {
                    conferir(alvo);
                }
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
        relatos.into_iter().map(|r| (r.codigo.unico, fonte[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn cascatas_em_listas() {
        let fonte = "var a = [1]..add(2);\nvar b = [1]..addAll([2, 3]);\nvar c = [1]..addAll(a);\nvar d = [1]..length..add(2);\nvar e = a..add(2);\n";
        assert_eq!(
            com_codigo("prefer_inlined_adds", fonte),
            vec![("prefer_inlined_adds_single", "add".to_string()), ("prefer_inlined_adds_multiple", "addAll".to_string())]
        );
        assert_eq!(com_codigo("prefer_spread_collections", fonte), vec![("prefer_spread_collections", "addAll".to_string())]);
    }

    #[test]
    fn classes() {
        let fonte = "class A {\n  final int a = 1;\n  final int b;\n  static final int c = 2;\n  const A(int x) : b = 0;\n}\nclass B {\n  final int d;\n  const B(int x) : d = x;\n}\n";
        let v: Vec<String> = com_codigo("avoid_field_initializers_in_const_classes", fonte).into_iter().map(|x| x.1).collect();
        // O inicializador que não lê parâmetro pede a semântica da unidade;
        // o campo com inicializador, não.
        assert_eq!(v, vec!["a = 1".to_string()]);
        let fonte = "class A {\n  static int f() => 0;\n}\nclass B {\n  static const int k = 1;\n}\nclass C {\n  static int v = 1;\n  int m() => 0;\n}\nclass D {\n  static int v = 1;\n  D(int x);\n}\nclass E {\n  static int v = 1;\n}\n";
        let v = com_codigo("avoid_classes_with_only_static_members", fonte);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v[0].1.starts_with("class A") && v[1].1.starts_with("class E"));
    }

    #[test]
    fn for_each_final() {
        let fonte = "void f(List<int> l, List<(int, int)> p) {\n  for (var a in l) {}\n  for (var b in l) {\n    b++;\n  }\n  for (final c in l) {}\n  for (var (x, y) in p) {}\n  for (var (z, w) in p) {\n    z = w;\n  }\n  var q = [for (var d in l) d];\n}\n";
        // A mutação pelo elemento pede a semântica da unidade.
        assert!(com_codigo("prefer_final_in_for_each", fonte).is_empty());
    }
}
