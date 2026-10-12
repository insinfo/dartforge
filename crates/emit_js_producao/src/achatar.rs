//! Membros de biblioteca como variáveis (`docs/JS-PRODUCAO-TAMANHO.md` §9.2).
//!
//! No contrato do DDC cada biblioteca é um objeto (`var core =
//! Object.create(dart.library)`) e cada membro de topo é uma propriedade dele:
//! `core.print(…)`, `dart.dsend(…)`, `L$pacote__lib.Classe.new(…)`. Num arquivo
//! só, esse objeto não é observado por ninguém além do próprio texto, então
//! `N.x` pode virar a variável `N$$x`, declarada uma vez no topo do arquivo. O
//! renomeio de identificadores do `oxc` passa a encurtá-la como a um local
//! (duas letras em vez de `ns.prop`), e o nome deixa de disputar o espaço
//! dos nomes de propriedade: `dart.throw`, `dart.bind`, `html.document` e o
//! `setAttribute` do ngdart, que ficavam como estavam porque o mesmo nome é
//! propriedade do JavaScript ou do DOM, encurtam também.
//!
//! A regra é conservadora; cada condição tem o porquê:
//!
//! * `N` é declarado no topo do arquivo com `Object.create(…)` e nenhum outro
//!   *binding* do arquivo tem o mesmo nome (senão a referência poderia ser a
//!   outra);
//! * `N` só aparece como objeto de `N.x`/`N["x"]`, ou como primeiro argumento
//!   de `dart.defineLazy`/`dart.copyProperties` (com um objeto literal no
//!   segundo) e de `dart.privateName`. Qualquer outro uso (passado a outra
//!   função, `N[expr]`) pode ler as propriedades por nome em execução, e o
//!   objeto inteiro fica como está;
//! * `x` tem ao menos uma escrita `N.x = …` (senão a variável nunca seria
//!   declarada) e não é chave do `defineLazy`/`copyProperties` de `N` (é
//!   *getter* no objeto);
//! * nenhuma função atribuída a `N.x` lê `this`: chamada como `N.x()` ela
//!   recebe `N` como `this`, e como `N$$x()`, não. O mesmo para `delete N.x`.
//!
//! A troca é de texto, pelos trechos que o `oxc` dá a cada acesso, como na
//! deduplicação de *strings*. `DARTFORGE_JSPROD_ACHATAR=0` desliga.

use oxc_ast::ast::{
    Argument, AssignmentOperator, AssignmentTarget, BindingIdentifier, CallExpression,
    ComputedMemberExpression, Expression, IdentifierReference, ObjectPropertyKind, PropertyKey,
    Statement, StaticMemberExpression, UnaryExpression, UnaryOperator, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use std::collections::{HashMap, HashSet};

/// Separador entre o namespace e o membro no nome da variável. `$$` não
/// aparece em nome que o emissor gera para biblioteca.
pub const SEPARADOR: &str = "$$";

#[derive(Default)]
struct Namespace {
    /// Acessos `N.x`/`N["x"]`: (nome, início, fim).
    acessos: Vec<(String, u32, u32)>,
    /// Membros com escrita `N.x = …`.
    escritos: HashSet<String>,
    /// Membros que não podem virar variável.
    fixos: HashSet<String>,
    /// O objeto é observado por inteiro: nada dele vira variável.
    escapa: bool,
    /// Onde escapou primeiro (diagnóstico, `DARTFORGE_JSPROD_ACHATAR_POR`).
    escapa_em: Option<u32>,
}

struct Analise<'s> {
    fonte: &'s str,
    ns: HashMap<String, Namespace>,
    /// Quantas vezes cada nome é declarado (*binding*) no arquivo.
    bindings: HashMap<String, u32>,
    /// Todos os identificadores do arquivo (para não colidir).
    identificadores: HashSet<String>,
    /// As `t$S…` da deduplicação cujo valor não é identificador: como chave
    /// (`core[t$S5]`) não podem citar um membro achatado.
    chaves_opacas: HashSet<String>,
}

/// Se o corpo da função lê `this` (fora das funções e classes aninhadas, que
/// têm o seu).
struct UsaThis(bool);

impl<'a> Visit<'a> for UsaThis {
    fn visit_this_expression(&mut self, _it: &oxc_ast::ast::ThisExpression) {
        self.0 = true;
    }
    fn visit_function(
        &mut self,
        _it: &oxc_ast::ast::Function<'a>,
        _flags: oxc_syntax::scope::ScopeFlags,
    ) {
    }
    fn visit_class(&mut self, _it: &oxc_ast::ast::Class<'a>) {}
}

fn e_identificador(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

impl<'s> Analise<'s> {
    fn alvo<'b>(&self, e: &'b Expression<'_>) -> Option<&'b str> {
        match e {
            Expression::Identifier(i) if self.ns.contains_key(i.name.as_str()) => {
                Some(i.name.as_str())
            }
            _ => None,
        }
    }

    /// `(N, x)` de um `N.x`/`N["x"]`.
    fn membro_de(&self, e: &Expression<'_>) -> Option<(String, String)> {
        match e {
            Expression::StaticMemberExpression(m) => self
                .alvo(&m.object)
                .map(|n| (n.to_string(), m.property.name.to_string())),
            Expression::ComputedMemberExpression(m) => match &m.expression {
                Expression::StringLiteral(s) if e_identificador(s.value.as_str()) => self
                    .alvo(&m.object)
                    .map(|n| (n.to_string(), s.value.to_string())),
                _ => None,
            },
            _ => None,
        }
    }
}

impl<'a> Visit<'a> for Analise<'_> {
    fn visit_binding_identifier(&mut self, it: &BindingIdentifier<'a>) {
        *self.bindings.entry(it.name.to_string()).or_default() += 1;
        self.identificadores.insert(it.name.to_string());
    }
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        let n = it.name.as_str();
        self.identificadores.insert(n.to_string());
        if let Some(ns) = self.ns.get_mut(n) {
            ns.escapa = true;
            ns.escapa_em.get_or_insert(it.span.start);
        }
    }
    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        if let Some(n) = self.alvo(&it.object).map(str::to_string) {
            let ns = self.ns.get_mut(&n).expect("namespace");
            ns.acessos
                .push((it.property.name.to_string(), it.span.start, it.span.end));
            return;
        }
        walk::walk_static_member_expression(self, it);
    }
    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        if let Some(n) = self.alvo(&it.object).map(str::to_string) {
            match &it.expression {
                Expression::StringLiteral(s) if e_identificador(s.value.as_str()) => {
                    let ns = self.ns.get_mut(&n).expect("namespace");
                    ns.acessos
                        .push((s.value.to_string(), it.span.start, it.span.end));
                }
                // `core["EnumName|get#name"]`: nome que nunca é
                // identificador, fica propriedade do objeto, que continua
                // existindo. O mesmo pela `t$S…` da deduplicação.
                Expression::StringLiteral(_) => {}
                Expression::Identifier(i) if self.chaves_opacas.contains(i.name.as_str()) => {}
                // `N[expr]`: o objeto é lido por nome em execução.
                _ => {
                    let ns = self.ns.get_mut(&n).expect("namespace");
                    ns.escapa = true;
                    ns.escapa_em.get_or_insert(it.span.start);
                    self.visit_expression(&it.expression);
                }
            }
            return;
        }
        walk::walk_computed_member_expression(self, it);
    }
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let funcao = match &it.callee {
            Expression::StaticMemberExpression(m) if matches!(&m.object, Expression::Identifier(o) if o.name == "dart") => {
                Some(m.property.name.as_str())
            }
            _ => None,
        };
        let primeiro = it
            .arguments
            .first()
            .and_then(|a| a.as_expression())
            .and_then(|e| self.alvo(e))
            .map(str::to_string);
        match (funcao, primeiro) {
            (Some("defineLazy" | "copyProperties"), Some(n)) => {
                // As chaves do objeto literal viram propriedades (acessores)
                // de `N`: ficam.
                match it.arguments.get(1).and_then(Argument::as_expression) {
                    Some(Expression::ObjectExpression(o)) => {
                        for p in &o.properties {
                            match p {
                                ObjectPropertyKind::ObjectProperty(p) => match &p.key {
                                    PropertyKey::StaticIdentifier(k) => {
                                        self.ns
                                            .get_mut(&n)
                                            .expect("namespace")
                                            .fixos
                                            .insert(k.name.to_string());
                                    }
                                    PropertyKey::StringLiteral(k) => {
                                        self.ns
                                            .get_mut(&n)
                                            .expect("namespace")
                                            .fixos
                                            .insert(k.value.to_string());
                                    }
                                    _ => self.ns.get_mut(&n).expect("namespace").escapa = true,
                                },
                                ObjectPropertyKind::SpreadProperty(_) => {
                                    self.ns.get_mut(&n).expect("namespace").escapa = true
                                }
                            }
                        }
                    }
                    _ => self.ns.get_mut(&n).expect("namespace").escapa = true,
                }
                self.visit_expression(&it.callee);
                for a in it.arguments.iter().skip(1) {
                    self.visit_argument(a);
                }
            }
            (Some("privateName"), Some(_)) => {
                self.visit_expression(&it.callee);
                for a in it.arguments.iter().skip(1) {
                    self.visit_argument(a);
                }
            }
            _ => walk::walk_call_expression(self, it),
        }
    }
    fn visit_assignment_expression(&mut self, it: &oxc_ast::ast::AssignmentExpression<'a>) {
        let membro = match &it.left {
            AssignmentTarget::StaticMemberExpression(m) => self
                .alvo(&m.object)
                .map(|n| (n.to_string(), m.property.name.to_string())),
            AssignmentTarget::ComputedMemberExpression(m) => match &m.expression {
                Expression::StringLiteral(s) if e_identificador(s.value.as_str()) => self
                    .alvo(&m.object)
                    .map(|n| (n.to_string(), s.value.to_string())),
                _ => None,
            },
            _ => None,
        };
        if let Some((n, x)) = membro {
            let usa_this = it.operator == AssignmentOperator::Assign
                && match &it.right {
                    Expression::FunctionExpression(f) => {
                        let mut u = UsaThis(false);
                        if let Some(b) = &f.body {
                            u.visit_function_body(b);
                        }
                        u.0
                    }
                    _ => false,
                };
            let ns = self.ns.get_mut(&n).expect("namespace");
            ns.escritos.insert(x.clone());
            if usa_this {
                ns.fixos.insert(x);
            }
        }
        walk::walk_assignment_expression(self, it);
    }
    fn visit_unary_expression(&mut self, it: &UnaryExpression<'a>) {
        if it.operator == UnaryOperator::Delete
            && let Some((n, x)) = self.membro_de(&it.argument)
        {
            self.ns.get_mut(&n).expect("namespace").fixos.insert(x);
        }
        walk::walk_unary_expression(self, it);
    }
}

/// Os namespaces candidatos: `var|let|const N = Object.create(…)` no topo do
/// corpo da função que embrulha o arquivo.
fn candidatos(programa: &oxc_ast::ast::Program<'_>) -> Vec<String> {
    fn corpo<'b, 'a>(stmts: &'b [Statement<'a>]) -> Option<&'b [Statement<'a>]> {
        for s in stmts {
            if let Statement::ExpressionStatement(e) = s
                && let Expression::CallExpression(c) = &e.expression
                && let Expression::ParenthesizedExpression(p) = &c.callee
                && let Expression::FunctionExpression(f) = &p.expression
            {
                return f.body.as_ref().map(|b| b.statements.as_slice());
            }
        }
        None
    }
    let Some(stmts) = corpo(&programa.body) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let e_create = |d: &VariableDeclarator<'_>| {
        matches!(&d.init, Some(Expression::CallExpression(c)) if matches!(&c.callee, Expression::StaticMemberExpression(m)
            if matches!(&m.object, Expression::Identifier(o) if o.name == "Object") && m.property.name == "create"))
    };
    for s in stmts {
        if let Statement::VariableDeclaration(v) = s {
            for d in &v.declarations {
                if let Some(id) = d.id.get_binding_identifier()
                    && e_create(d)
                {
                    out.push(id.name.to_string());
                }
            }
        }
    }
    out
}

/// As `var t$S… = "…"` do topo do arquivo (a deduplicação de *strings*) cujo
/// valor não tem forma de identificador.
fn chaves_opacas(programa: &oxc_ast::ast::Program<'_>) -> HashSet<String> {
    struct V(HashSet<String>);
    impl<'a> Visit<'a> for V {
        fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
            if let Some(id) = it.id.get_binding_identifier()
                && id.name.starts_with("t$S")
                && let Some(Expression::StringLiteral(l)) = &it.init
                && !e_identificador(l.value.as_str())
            {
                self.0.insert(id.name.to_string());
            }
        }
        fn visit_function_body(&mut self, _it: &oxc_ast::ast::FunctionBody<'a>) {}
    }
    let mut v = V(HashSet::new());
    // Só o topo: o embrulho do arquivo e o corpo dele.
    for s in &programa.body {
        if let Statement::ExpressionStatement(e) = s
            && let Expression::CallExpression(c) = &e.expression
            && let Expression::ParenthesizedExpression(p) = &c.callee
            && let Expression::FunctionExpression(f) = &p.expression
            && let Some(b) = &f.body
        {
            for st in &b.statements {
                if let Statement::VariableDeclaration(d) = st {
                    for decl in &d.declarations {
                        v.visit_variable_declarator(decl);
                    }
                }
            }
        }
    }
    v.0
}

/// Aplica a troca a `fonte` (o arquivo já embrulhado em `(function () {…})();`).
/// Devolve a fonte nova e quantos membros viraram variável.
///
/// # Erros
///
/// O texto não é JS válido para o parser.
pub fn aplicar(fonte: &str) -> Result<(String, usize), String> {
    let alocador = oxc_allocator::Allocator::default();
    let lido = oxc_parser::Parser::new(&alocador, fonte, oxc_span::SourceType::cjs()).parse();
    if let Some(e) = lido.diagnostics.first() {
        return Err(format!("oxc não leu o arquivo de produção: {e}"));
    }
    let mut a = Analise {
        fonte,
        ns: HashMap::new(),
        bindings: HashMap::new(),
        identificadores: HashSet::new(),
        chaves_opacas: chaves_opacas(&lido.program),
    };
    for n in candidatos(&lido.program) {
        a.ns.insert(n, Namespace::default());
    }
    if a.ns.is_empty() {
        return Ok((fonte.to_string(), 0));
    }
    a.visit_program(&lido.program);
    let _ = a.fonte;
    // Troca por trecho, e as declarações no começo do corpo.
    let mut trocas: Vec<(u32, u32, String)> = Vec::new();
    let mut declarados: Vec<String> = Vec::new();
    let mut nomes: Vec<&String> = a.ns.keys().collect();
    nomes.sort();
    let diag = std::env::var("DARTFORGE_JSPROD_ACHATAR_POR").ok();
    for n in nomes {
        let ns = &a.ns[n];
        if diag
            .as_deref()
            .is_some_and(|d| d.split(',').any(|x| x == n) || (d == "*" && ns.escapa))
        {
            let trecho = ns
                .escapa_em
                .map(|i| &fonte[i as usize..(i as usize + 120).min(fonte.len())]);
            eprintln!(
                "[jsprod] achatar {n}: escapa={} bindings={:?} em {:?}",
                ns.escapa,
                a.bindings.get(n),
                trecho
            );
        }
        if ns.escapa || a.bindings.get(n).copied().unwrap_or(0) != 1 {
            continue;
        }
        let mut membros: HashSet<&str> = HashSet::new();
        for x in &ns.escritos {
            let var = format!("{n}{SEPARADOR}{x}");
            if !ns.fixos.contains(x) && !a.identificadores.contains(&var) {
                membros.insert(x.as_str());
            }
        }
        if diag.is_some() {
            let mut usos: HashMap<&str, usize> = HashMap::new();
            for (x, _, _) in &ns.acessos {
                *usos.entry(x.as_str()).or_default() += 1;
            }
            let mortos: Vec<&&str> = membros.iter().filter(|x| usos.get(**x).copied().unwrap_or(0) <= 1).collect();
            if !mortos.is_empty() {
                eprintln!("[jsprod] achatar {n}: {} membro(s) só escrito(s): {:?}", mortos.len(), mortos.iter().take(8).collect::<Vec<_>>());
            }
        }
        let mut ordenados: Vec<&&str> = membros.iter().collect();
        ordenados.sort();
        declarados.extend(ordenados.iter().map(|x| format!("{n}{SEPARADOR}{x}")));
        for (x, i, f) in &ns.acessos {
            if membros.contains(x.as_str()) {
                trocas.push((*i, *f, format!("{n}{SEPARADOR}{x}")));
            }
        }
    }
    if declarados.is_empty() {
        return Ok((fonte.to_string(), 0));
    }
    trocas.sort_by_key(|t| t.0);
    let cabeca = "(function () {\n";
    if !fonte.starts_with(cabeca) {
        return Ok((fonte.to_string(), 0));
    }
    let mut out = String::with_capacity(fonte.len());
    out.push_str(cabeca);
    out.push_str("var ");
    out.push_str(&declarados.join(", "));
    out.push_str(";\n");
    let mut pos = cabeca.len();
    for (i, f, nome) in trocas {
        let (i, f) = (i as usize, f as usize);
        if i < pos {
            continue;
        }
        out.push_str(&fonte[pos..i]);
        out.push_str(&nome);
        pos = f;
    }
    out.push_str(&fonte[pos..]);
    Ok((out, declarados.len()))
}

/// O membro `x` de um nome achatado `N$$x` cujo namespace é `ns`.
pub fn membro<'n>(nome: &'n str, ns: &str) -> Option<&'n str> {
    nome.strip_prefix(ns)?.strip_prefix(SEPARADOR)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn rodar(js: &str) -> String {
        aplicar(&format!("(function () {{\n{js}\n}})();\n"))
            .unwrap()
            .0
    }

    #[test]
    fn membro_escrito_vira_variavel() {
        let s = rodar(
            "const dart = Object.create(null);\nvar core = Object.create(dart);\ncore.f = function(a) { return a; };\ncore.f(1);",
        );
        assert!(s.contains("var core$$f;"), "{s}");
        assert!(s.contains("core$$f = function(a)"), "{s}");
        assert!(s.contains("core$$f(1)"), "{s}");
    }

    #[test]
    fn objeto_observado_fica() {
        let s = rodar("var core = Object.create(null);\ncore.f = 1;\nlet k = 'f'; core[k];");
        assert!(!s.contains("core$$f"), "{s}");
        let s = rodar("var core = Object.create(null);\ncore.f = 1;\nObject.keys(core);");
        assert!(!s.contains("core$$f"), "{s}");
    }

    #[test]
    fn lazy_this_delete_e_sem_escrita_ficam() {
        let s = rodar(
            "const dart = Object.create(null);\nvar L = Object.create(null);\ndart.defineLazy = function(a, b) {};\ndart.defineLazy(L, {get x() { return 1; }});\nL.x = 2;\nL.m = function() { return this; };\nL.d = 1; delete L.d;\nL.y;\nL.ok = () => 1;",
        );
        assert!(!s.contains("L$$x"), "{s}");
        assert!(!s.contains("L$$m"), "{s}");
        assert!(!s.contains("L$$d"), "{s}");
        assert!(!s.contains("L$$y"), "{s}");
        assert!(s.contains("L$$ok = () => 1"), "{s}");
        assert!(s.contains("dart$$defineLazy(L, {"), "{s}");
    }

    #[test]
    fn nome_sombreado_fica() {
        let s = rodar(
            "var core = Object.create(null);\ncore.f = 1;\nfunction g(core) { return core.f; }",
        );
        assert!(!s.contains("core$$f"), "{s}");
    }
}
