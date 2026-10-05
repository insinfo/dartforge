//! Verificações da família A que só dependem da árvore e do contexto
//! sintático em que o nó aparece (docs/ANALYZER-ESPECIFICACAO.md §A):
//!
//! * `super_in_invalid_context`, `super_in_extension`,
//!   `super_in_extension_type` — o `SuperContext.of` do analyzer
//!   (`generated/super_context.dart`, relatado em
//!   `element_resolver.dart:404-424`);
//! * `async_for_in_wrong_context` — `await for` fora de função `async`;
//! * `await_in_late_local_variable_initializer`;
//! * `expected_one_list_type_arguments`, `expected_one_set_type_arguments`,
//!   `expected_two_map_type_arguments` (`type_arguments_verifier.dart:160-221`);
//! * `extension_type_constructor_with_super_invocation` e
//!   `extension_type_constructor_with_super_formal_parameter`
//!   (`error_verifier.dart:3393-3402`, `:1456-1466`).
//!
//! Escrito sem compilar nem executar (2026-10-04).

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_frontend::ast::{
    self, AsyncModifier, CollectionElement, DeclKind, ExprId, ExprKind, FunctionBody, MemberId, MemberKind, PatternId,
    PatternKind, StmtId, StmtKind,
};

/// O contexto de um `super` (`SuperContext`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sup {
    /// Membro de instância de classe, enum ou mixin.
    Valido,
    /// Topo, membro estático, construtor de fábrica, inicializador de
    /// construtor, inicializador de campo estático ou não `late`.
    Estatico,
    Extensao,
    TipoDeExtensao,
}

/// O tipo que contém o membro visitado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dono {
    Classe,
    Extensao,
    TipoDeExtensao,
}

impl Dono {
    fn sup(self) -> Sup {
        match self {
            Dono::Classe => Sup::Valido,
            Dono::Extensao => Sup::Extensao,
            Dono::TipoDeExtensao => Sup::TipoDeExtensao,
        }
    }
}

struct Visita<'a> {
    a: &'a ast::Ast,
    fonte: &'a str,
    saida: Vec<Diagnostic>,
    sup: Sup,
    /// O modificador do corpo da função mais interna.
    modificador: AsyncModifier,
    /// Dentro do inicializador de uma variável local `late` (uma função
    /// literal no meio desliga).
    em_late_local: bool,
}

/// Os diagnósticos de contexto de uma unidade.
pub fn verificar(u: Unidade<'_>) -> Vec<Diagnostic> {
    let mut v = Visita { a: u.ast, fonte: u.fonte, saida: Vec::new(), sup: Sup::Estatico, modificador: AsyncModifier::None, em_late_local: false };
    for &d in u.unit.declarations.iter() {
        v.declaracao(d);
    }
    v.saida
}

impl<'a> Visita<'a> {
    fn relatar(&mut self, codigo: Codigo, span: Span, args: &[&str]) {
        self.saida.push(Diagnostic::com_codigo(codigo, span, args.iter().copied()));
    }

    // -- Declarações -------------------------------------------------------------

    fn declaracao(&mut self, d: ast::DeclId) {
        let a = self.a;
        match &a.decl(d).kind {
            DeclKind::Variables(l) => {
                self.sup = Sup::Estatico;
                for var in l.variables.iter() {
                    if let Some(init) = var.initializer {
                        self.expr(init);
                    }
                }
            }
            DeclKind::Function(f) => {
                self.sup = Sup::Estatico;
                self.funcao(*f);
            }
            DeclKind::Class(k) => self.membros(&k.members, Dono::Classe),
            DeclKind::Mixin(k) => self.membros(&k.members, Dono::Classe),
            DeclKind::Enum(k) => {
                // Os argumentos de uma constante de enum são avaliados fora
                // de qualquer membro: contexto de instância não há.
                self.sup = Sup::Valido;
                for cst in k.constants.iter() {
                    if let Some(args) = &cst.arguments {
                        for x in args.args.iter() {
                            self.expr(x.value);
                        }
                    }
                }
                self.membros(&k.members, Dono::Classe);
            }
            DeclKind::Extension(k) => self.membros(&k.members, Dono::Extensao),
            DeclKind::ExtensionType(k) => self.membros(&k.members, Dono::TipoDeExtensao),
            DeclKind::Typedef(_) => {}
        }
        self.sup = Sup::Estatico;
    }

    fn membros(&mut self, membros: &'a [MemberId], dono: Dono) {
        let a = self.a;
        for &mid in membros {
            match &a.member(mid).kind {
                MemberKind::Field(l) => {
                    // Campo estático, ou de instância que não é `late`: o
                    // inicializador roda sem `this`.
                    self.sup = if l.static_ || !l.late { Sup::Estatico } else { dono.sup() };
                    for var in l.variables.iter() {
                        if let Some(init) = var.initializer {
                            self.expr(init);
                        }
                    }
                }
                MemberKind::Method(f) => {
                    self.sup = if a.function(*f).static_ { Sup::Estatico } else { dono.sup() };
                    self.funcao(*f);
                }
                MemberKind::Constructor(k) => {
                    let do_corpo = if k.factory { Sup::Estatico } else { dono.sup() };
                    // Parâmetros: os valores padrão e, num tipo de extensão,
                    // o `super.x` que ele não pode declarar.
                    self.sup = do_corpo;
                    for p in k.parameters.iter() {
                        if dono == Dono::TipoDeExtensao && p.super_ {
                            if let Some(n) = p.name {
                                // `super.x`: a palavra fica seis bytes antes do nome.
                                let inicio = n.span.start.saturating_sub(6);
                                if self.fonte.get(inicio..inicio + 5) == Some("super") {
                                    self.relatar(c::EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_FORMAL_PARAMETER, Span { start: inicio, end: inicio + 5 }, &[]);
                                }
                            }
                        }
                    }
                    self.parametros(&k.parameters);
                    // Todo inicializador de construtor é contexto estático.
                    self.sup = Sup::Estatico;
                    for init in k.initializers.iter() {
                        match init {
                            ast::Initializer::Field { value, .. } => self.expr(*value),
                            ast::Initializer::Assert { condition, message, .. } => {
                                self.expr(*condition);
                                if let Some(m) = message {
                                    self.expr(*m);
                                }
                            }
                            ast::Initializer::Super { span, arguments, .. } => {
                                if dono == Dono::TipoDeExtensao {
                                    let palavra = self.palavra(*span, "super");
                                    self.relatar(c::EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_INVOCATION, palavra, &[]);
                                }
                                for x in arguments.args.iter() {
                                    self.expr(x.value);
                                }
                            }
                            ast::Initializer::Redirect { arguments, .. } => {
                                for x in arguments.args.iter() {
                                    self.expr(x.value);
                                }
                            }
                        }
                    }
                    self.sup = do_corpo;
                    let antes = std::mem::replace(&mut self.modificador, AsyncModifier::None);
                    self.corpo(&k.body);
                    self.modificador = antes;
                }
            }
        }
    }

    /// A palavra `texto` dentro de `span` (o começo do intervalo, quando não
    /// é achada).
    fn palavra(&self, span: Span, texto: &str) -> Span {
        let inicio = self
            .fonte
            .get(span.start..span.end.min(self.fonte.len()))
            .and_then(|t| t.find(texto))
            .map_or(span.start, |i| span.start + i);
        Span { start: inicio, end: inicio + texto.len() }
    }

    fn parametros(&mut self, ps: &'a [ast::Parameter]) {
        for p in ps {
            if let Some(fp) = &p.function_parameters {
                self.parametros(fp);
            }
            if let Some(d) = p.default_value {
                self.expr(d);
            }
        }
    }

    /// Uma função (declarada ou literal): o corpo tem o modificador dela, e
    /// o inicializador `late` de fora não vale lá dentro.
    fn funcao(&mut self, f: ast::FunctionId) {
        let a = self.a;
        let func = a.function(f);
        if let Some(ps) = &func.parameters {
            self.parametros(ps);
        }
        let modificador = std::mem::replace(&mut self.modificador, func.modifier);
        let em_late = std::mem::replace(&mut self.em_late_local, false);
        self.corpo(&func.body);
        self.modificador = modificador;
        self.em_late_local = em_late;
    }

    fn corpo(&mut self, b: &FunctionBody) {
        match b {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(e) => self.expr(*e),
            _ => {}
        }
    }

    // -- Comandos ------------------------------------------------------------------

    /// `await for` fora de função `async`/`async*`: no token `await`.
    fn await_for(&mut self, inicio: usize) {
        if matches!(self.modificador, AsyncModifier::Async | AsyncModifier::AsyncStar) {
            return;
        }
        let palavra = self.palavra(Span { start: inicio, end: inicio + 16 }, "await");
        self.relatar(c::ASYNC_FOR_IN_WRONG_CONTEXT, palavra, &[]);
    }

    fn lista_local(&mut self, l: &'a ast::VariableList) {
        let antes = self.em_late_local;
        if l.late {
            self.em_late_local = true;
        }
        for var in l.variables.iter() {
            if let Some(init) = var.initializer {
                self.expr(init);
            }
        }
        self.em_late_local = antes;
    }

    fn inicio_de_for(&mut self, init: &'a Option<ast::ForInit>) {
        match init {
            Some(ast::ForInit::Variables(l)) => self.lista_local(l),
            Some(ast::ForInit::Expression(e)) => self.expr(*e),
            Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                self.padrao(*pattern);
                self.expr(*value);
            }
            None => {}
        }
    }

    fn alvo_de_for_in(&mut self, alvo: &'a ast::ForInTarget) {
        match alvo {
            ast::ForInTarget::Declared { .. } => {}
            ast::ForInTarget::Pattern { pattern, .. } => self.padrao(*pattern),
            ast::ForInTarget::Expression(e) => self.expr(*e),
        }
    }

    fn stmt(&mut self, s: StmtId) {
        let a = self.a;
        let no = a.stmt(s);
        match &no.kind {
            StmtKind::Block(xs) => {
                for x in xs.iter() {
                    self.stmt(*x);
                }
            }
            StmtKind::Variables(l) => self.lista_local(l),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.padrao(*pattern);
                self.expr(*value);
            }
            StmtKind::Function(f) => self.funcao(*f),
            StmtKind::Expression(e) => self.expr(*e),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.padrao(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.stmt(*then);
                if let Some(e) = else_ {
                    self.stmt(*e);
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                self.inicio_de_for(init);
                if let Some(x) = condition {
                    self.expr(*x);
                }
                for x in updates.iter() {
                    self.expr(*x);
                }
                self.stmt(*body);
            }
            StmtKind::ForIn { await_, target, iterable, body } => {
                if *await_ {
                    self.await_for(no.span.start);
                }
                self.alvo_de_for_in(target);
                self.expr(*iterable);
                self.stmt(*body);
            }
            StmtKind::While { condition, body } => {
                self.expr(*condition);
                self.stmt(*body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.stmt(*body);
                self.expr(*condition);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value);
                for caso in cases.iter() {
                    if let Some(p) = caso.pattern {
                        self.padrao(p);
                    }
                    if let Some(g) = caso.guard {
                        self.expr(g);
                    }
                    for x in caso.body.iter() {
                        self.stmt(*x);
                    }
                }
            }
            StmtKind::Return(Some(e)) => self.expr(*e),
            StmtKind::Yield { value, .. } => self.expr(*value),
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(*body);
                for k in catches.iter() {
                    self.stmt(k.body);
                }
                if let Some(f) = finally_ {
                    self.stmt(*f);
                }
            }
            StmtKind::Labeled { body, .. } => self.stmt(*body),
            StmtKind::Assert { condition, message } => {
                self.expr(*condition);
                if let Some(m) = message {
                    self.expr(*m);
                }
            }
            _ => {}
        }
    }

    // -- Padrões -------------------------------------------------------------------

    fn padrao(&mut self, p: PatternId) {
        let a = self.a;
        match &a.pattern(p).kind {
            PatternKind::Constant(e) => self.expr(*e),
            PatternKind::Relational { value, .. } => self.expr(*value),
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.padrao(*x);
                self.padrao(*y);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => self.padrao(*x),
            PatternKind::Cast { pattern, .. } => self.padrao(*pattern),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => self.padrao(*x),
                        _ => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    self.expr(en.key);
                    self.padrao(en.value);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.padrao(f.pattern);
                }
            }
            _ => {}
        }
    }

    // -- Expressões ----------------------------------------------------------------

    fn elemento(&mut self, el: &'a CollectionElement, inicio_da_colecao: usize) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => self.expr(*x),
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key);
                self.expr(*value);
            }
            CollectionElement::Spread { value, .. } => self.expr(*value),
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.padrao(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.elemento(then, inicio_da_colecao);
                if let Some(x) = else_ {
                    self.elemento(x, inicio_da_colecao);
                }
            }
            CollectionElement::For { init, condition, updates, body, .. } => {
                self.inicio_de_for(init);
                if let Some(x) = condition {
                    self.expr(*x);
                }
                for x in updates.iter() {
                    self.expr(*x);
                }
                self.elemento(body, inicio_da_colecao);
            }
            CollectionElement::ForIn { await_, target, iterable, body } => {
                if *await_ {
                    // O elemento não guarda o próprio intervalo: o `await`
                    // é o último antes do iterável.
                    let fim = self.a.expr(*iterable).span.start;
                    let trecho = self.fonte.get(inicio_da_colecao..fim).unwrap_or("");
                    if !matches!(self.modificador, AsyncModifier::Async | AsyncModifier::AsyncStar) {
                        if let Some(i) = trecho.rfind("await") {
                            let ini = inicio_da_colecao + i;
                            self.relatar(c::ASYNC_FOR_IN_WRONG_CONTEXT, Span { start: ini, end: ini + 5 }, &[]);
                        }
                    }
                }
                self.alvo_de_for_in(target);
                self.expr(*iterable);
                self.elemento(body, inicio_da_colecao);
            }
        }
    }

    /// A lista `<…>` de argumentos de tipo de um literal: do `<` antes do
    /// primeiro argumento ao `>` depois do último.
    fn lista_de_tipos(&self, tipos: &[ast::TypeId]) -> Option<Span> {
        let primeiro = self.a.ty(*tipos.first()?).span;
        let ultimo = self.a.ty(*tipos.last()?).span;
        let bytes = self.fonte.as_bytes();
        let mut ini = primeiro.start;
        while ini > 0 && bytes.get(ini - 1).is_some_and(|b| b.is_ascii_whitespace()) {
            ini -= 1;
        }
        if ini == 0 || bytes.get(ini - 1) != Some(&b'<') {
            return None;
        }
        let mut fim = ultimo.end;
        while bytes.get(fim).is_some_and(|b| b.is_ascii_whitespace()) {
            fim += 1;
        }
        if bytes.get(fim) != Some(&b'>') {
            return None;
        }
        Some(Span { start: ini - 1, end: fim + 1 })
    }

    fn expr(&mut self, e: ExprId) {
        let a = self.a;
        let no = a.expr(e);
        match &no.kind {
            ExprKind::Super => {
                let codigo = match self.sup {
                    Sup::Valido => None,
                    Sup::Estatico => Some(c::SUPER_IN_INVALID_CONTEXT),
                    Sup::Extensao => Some(c::SUPER_IN_EXTENSION),
                    Sup::TipoDeExtensao => Some(c::SUPER_IN_EXTENSION_TYPE),
                };
                if let Some(codigo) = codigo {
                    self.relatar(codigo, no.span, &[]);
                }
            }
            ExprKind::Await(x) => {
                if self.em_late_local {
                    let palavra = Span { start: no.span.start, end: no.span.start + 5 };
                    self.relatar(c::AWAIT_IN_LATE_LOCAL_VARIABLE_INITIALIZER, palavra, &[]);
                }
                self.expr(*x);
            }
            ExprKind::List { type_args, elements, .. } => {
                if type_args.len() > 1 {
                    if let Some(sp) = self.lista_de_tipos(type_args) {
                        let n = type_args.len().to_string();
                        self.relatar(c::EXPECTED_ONE_LIST_TYPE_ARGUMENTS, sp, &[n.as_str()]);
                    }
                }
                for el in elements.iter() {
                    self.elemento(el, no.span.start);
                }
            }
            ExprKind::SetOrMap { type_args, elements, .. } => {
                // Com um argumento é conjunto e com dois é mapa; com três ou
                // mais, a forma vem dos elementos (`{}` vazio é mapa).
                if type_args.len() > 2 {
                    if let Some(sp) = self.lista_de_tipos(type_args) {
                        let n = type_args.len().to_string();
                        let tem_entrada = elementos_com_entrada(elements);
                        let conjunto = !tem_entrada && !elements.is_empty();
                        let codigo = if conjunto { c::EXPECTED_ONE_SET_TYPE_ARGUMENTS } else { c::EXPECTED_TWO_MAP_TYPE_ARGUMENTS };
                        self.relatar(codigo, sp, &[n.as_str()]);
                    }
                }
                for el in elements.iter() {
                    self.elemento(el, no.span.start);
                }
            }
            ExprKind::Record { positional, named, .. } => {
                for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                    self.expr(*x);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => {
                for x in arguments.args.iter() {
                    self.expr(x.value);
                }
            }
            ExprKind::FunctionExpression(f) => self.funcao(*f),
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Throw(x) => self.expr(*x),
            ExprKind::Property { target, .. } => self.expr(*target),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target);
                self.expr(*index);
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target);
                for x in arguments.args.iter() {
                    self.expr(x.value);
                }
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target),
            ExprKind::Unary { operand, .. } => self.expr(*operand),
            ExprKind::Binary { left, right, .. } => {
                self.expr(*left);
                self.expr(*right);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(*condition);
                self.expr(*then);
                self.expr(*else_);
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.expr(*value),
            ExprKind::Assign { target, value, .. } => {
                self.expr(*target);
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.padrao(*pattern);
                self.expr(*value);
            }
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(*target);
                for s in sections.iter() {
                    self.expr(*s);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value);
                for caso in cases.iter() {
                    self.padrao(caso.pattern);
                    if let Some(g) = caso.guard {
                        self.expr(g);
                    }
                    self.expr(caso.body);
                }
            }
            _ => {}
        }
    }
}

/// Algum elemento de topo do literal (passando por `if`/`for`) é uma
/// entrada `chave: valor`.
fn elementos_com_entrada(elementos: &[CollectionElement]) -> bool {
    fn tem(el: &CollectionElement) -> bool {
        match el {
            CollectionElement::MapEntry { .. } => true,
            CollectionElement::If { then, else_, .. } => tem(then) || else_.as_deref().is_some_and(tem),
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => tem(body),
            _ => false,
        }
    }
    elementos.iter().any(tem)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn codigos(fonte: &str) -> Vec<(&'static str, usize, usize)> {
        let mut nomes = dartforge_intern::Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        verificar(u).into_iter().map(|d| (d.code.map_or("", |c| c.info().nome), d.span.start, d.span.end)).collect()
    }

    #[test]
    fn super_em_metodo_de_instancia_vale() {
        assert!(codigos("class A { void m() { super.toString(); } }").is_empty());
    }

    #[test]
    fn super_em_contexto_estatico() {
        // 0         1         2         3
        // 0123456789012345678901234567890123456789
        // class A { static void m() { super.x; } }
        let r = codigos("class A { static void m() { super.x; } }");
        assert_eq!(r, vec![("super_in_invalid_context", 28, 33)]);
    }

    #[test]
    fn super_em_campo_nao_late_e_em_inicializador() {
        let r = codigos("class A { var a = super.hashCode; late var b = super.hashCode; A() : a = super.hashCode; }");
        let nomes: Vec<&str> = r.iter().map(|x| x.0).collect();
        assert_eq!(nomes, vec!["super_in_invalid_context", "super_in_invalid_context"]);
    }

    #[test]
    fn super_em_extensao_e_em_tipo_de_extensao() {
        let r = codigos("extension E on int { void m() { super.toString(); } }");
        assert_eq!(r[0].0, "super_in_extension");
        let r = codigos("extension type T(int i) { void m() { super.toString(); } }");
        assert_eq!(r[0].0, "super_in_extension_type");
    }

    #[test]
    fn await_for_fora_de_async() {
        // void f(s) { await for (var x in s) {} }
        let r = codigos("void f(s) { await for (var x in s) {} }");
        assert_eq!(r, vec![("async_for_in_wrong_context", 12, 17)]);
        assert!(codigos("void f(s) async { await for (var x in s) {} }").is_empty());
    }

    #[test]
    fn await_em_local_late() {
        // void f(g) async { late var x = await g; }
        let r = codigos("void f(g) async { late var x = await g; }");
        assert_eq!(r, vec![("await_in_late_local_variable_initializer", 31, 36)]);
        // A função literal no meio não conta.
        assert!(codigos("void f(g) async { late var x = () async => await g; }").is_empty());
    }

    #[test]
    fn argumentos_de_tipo_de_literais() {
        // var a = <int, int>[];
        let r = codigos("var a = <int, int>[];");
        assert_eq!(r, vec![("expected_one_list_type_arguments", 8, 18)]);
        let r = codigos("var a = <int, int, int>{};");
        assert_eq!(r[0].0, "expected_two_map_type_arguments");
        let r = codigos("var a = <int, int, int>{1};");
        assert_eq!(r[0].0, "expected_one_set_type_arguments");
    }
}
