//! Variáveis e funções locais nunca lidas: a parte local do
//! `UnusedLocalElementsVerifier` do analyzer 6.11.0
//! (`src/error/unused_local_elements_verifier.dart`).
//!
//! O escopo de um local é léxico, então a resolução é sintática: cada
//! identificador simples é ligado ao local de mesmo nome mais interno em
//! escopo (parâmetros entram no escopo e escondem os de fora, mas nunca são
//! relatados aqui). Uma leitura é o que o `_isReadIdentifier` do analyzer
//! chama de leitura:
//! * atribuição simples (`x = e`) não lê `x`;
//! * `x++;`, `++x;` e `x += e;` **como comando** não leem (`x ??= e;` lê);
//!   dentro de uma expressão, leem;
//! * todo o resto é leitura.
//!
//! Relatados (`_visitLocalVariableElement`):
//! * `UNUSED_LOCAL_VARIABLE`: variável local declarada num comando, na parte
//!   de declaração de um `for`, num `for-in` ou num padrão (de uma
//!   declaração por padrão só se **nenhuma** das variáveis dela for lida);
//! * `UNUSED_ELEMENT`: função local nunca referenciada.
//!
//! Nomes só de `_` (`_`, `__`) não são relatados; com `wildcard-variables`
//! (3.7+), `_` nem declara.

use crate::Unidade;
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_frontend::ast::{
    self, AssignOp, Ast, BinaryOp, CollectionElement, DeclKind, ExprId, ExprKind, ForInTarget, ForInit,
    FunctionBody, FunctionId, Initializer, ListPatternElement, MemberKind, Parameter, PatternId, PatternKind,
    StmtId, StmtKind, StringPart, UnaryOp,
};
use dartforge_intern::{Interner, SymbolId};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Especie {
    /// Variável local relatável.
    Variavel,
    /// Função local.
    Funcao,
    /// Parâmetro, variável de `catch` etc.: esconde nomes, não se relata.
    Oculta,
}

struct Local {
    nome: SymbolId,
    span: Span,
    especie: Especie,
    lido: bool,
    /// Declaração por padrão a que pertence (todas ou nenhuma).
    grupo: Option<usize>,
}

struct Visita<'a> {
    ast: &'a Ast,
    interner: &'a Interner,
    curinga: bool,
    locais: Vec<Local>,
    escopos: Vec<Vec<usize>>,
    grupos: usize,
}

impl<'a> Visita<'a> {
    fn entrar(&mut self) {
        self.escopos.push(Vec::new());
    }

    fn sair(&mut self) {
        self.escopos.pop();
    }

    fn declarar(&mut self, n: ast::Name, especie: Especie, grupo: Option<usize>) {
        if self.curinga && self.interner.resolve(n.sym) == "_" {
            return;
        }
        let i = self.locais.len();
        self.locais.push(Local { nome: n.sym, span: n.span, especie, lido: false, grupo });
        if let Some(e) = self.escopos.last_mut() {
            e.push(i);
        }
    }

    fn achar(&self, nome: SymbolId) -> Option<usize> {
        self.escopos.iter().rev().flat_map(|e| e.iter().rev()).copied().find(|&i| self.locais[i].nome == nome)
    }

    /// Referência ao identificador `n`: `leitura` diz se lê o valor. Função
    /// local conta como usada por qualquer referência.
    fn referir(&mut self, n: ast::Name, leitura: bool) {
        if let Some(i) = self.achar(n.sym) {
            let l = &mut self.locais[i];
            if leitura || l.especie == Especie::Funcao {
                l.lido = true;
            }
        }
    }

    // -- Expressões ----------------------------------------------------------

    fn expr(&mut self, e: ExprId) {
        self.expr_ctx(e, false);
    }

    /// `comando`: a expressão é o comando inteiro (`ExpressionStatement`).
    fn expr_ctx(&mut self, e: ExprId, comando: bool) {
        let ast = self.ast;
        match &ast.expr(e).kind {
            ExprKind::Identifier(n) => self.referir(*n, true),
            ExprKind::String(s) => self.string(s),
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.elemento(el);
                }
            }
            ExprKind::Record { positional, named, .. } => {
                for x in positional.iter() {
                    self.expr(*x);
                }
                for (_, x) in named.iter() {
                    self.expr(*x);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => self.argumentos(arguments),
            ExprKind::FunctionExpression(f) => self.funcao(*f, false),
            ExprKind::Property { target, .. } => self.expr(*target),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target);
                self.expr(*index);
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target);
                self.argumentos(arguments);
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target),
            ExprKind::Unary { op, operand } => {
                let incremento = matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec);
                if incremento {
                    if let ExprKind::Identifier(n) = &ast.expr(*operand).kind {
                        self.referir(*n, !comando);
                        return;
                    }
                }
                self.expr(*operand);
            }
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
            ExprKind::Assign { op, target, value } => {
                match &ast.expr(*target).kind {
                    ExprKind::Identifier(n) => {
                        let leitura = match op {
                            AssignOp::Assign => false,
                            AssignOp::Compound(BinaryOp::IfNull) => true,
                            AssignOp::Compound(_) => !comando,
                        };
                        self.referir(*n, leitura);
                    }
                    _ => self.expr(*target),
                }
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.expr(*value);
                self.padrao_de_atribuicao(*pattern);
            }
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(*target);
                for s in sections.iter() {
                    self.expr(*s);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value);
                for c in cases.iter() {
                    self.entrar();
                    self.padrao_declarado(c.pattern, None);
                    if let Some(g) = c.guard {
                        self.expr(g);
                    }
                    self.expr(c.body);
                    self.sair();
                }
            }
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Symbol(_)
            | ExprKind::This
            | ExprKind::Super
            | ExprKind::CascadeTarget
            | ExprKind::Rethrow
            | ExprKind::DotShorthand { .. } => {}
        }
    }

    fn string(&mut self, s: &ast::StringLit) {
        for p in s.parts.iter() {
            if let StringPart::Interpolation(x) = p {
                self.expr(*x);
            }
        }
    }

    fn argumentos(&mut self, a: &ast::Arguments) {
        for x in a.args.iter() {
            self.expr(x.value);
        }
    }

    fn elemento(&mut self, el: &CollectionElement) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => self.expr(*x),
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key);
                self.expr(*value);
            }
            CollectionElement::Spread { value, .. } => self.expr(*value),
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                self.entrar();
                if let Some(p) = case_pattern {
                    self.padrao_declarado(*p, None);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.elemento(then);
                self.sair();
                if let Some(e) = else_ {
                    self.elemento(e);
                }
            }
            CollectionElement::For { init, condition, updates, body, .. } => {
                self.entrar();
                self.for_init(init.as_ref());
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for u in updates.iter() {
                    self.expr_ctx(*u, true);
                }
                self.elemento(body);
                self.sair();
            }
            CollectionElement::ForIn { target, iterable, body, .. } => {
                self.expr(*iterable);
                self.entrar();
                self.for_in_alvo(target);
                self.elemento(body);
                self.sair();
            }
        }
    }

    // -- Padrões -------------------------------------------------------------

    /// Padrão que declara variáveis (`var (a, b) = e`, `case`, `if-case`).
    fn padrao_declarado(&mut self, p: PatternId, grupo: Option<usize>) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, .. } => self.declarar(*name, Especie::Variavel, grupo),
            PatternKind::Constant(x) | PatternKind::Relational { value: x, .. } => self.expr(*x),
            PatternKind::Or(a, b) | PatternKind::And(a, b) => {
                self.padrao_declarado(*a, grupo);
                self.padrao_declarado(*b, grupo);
            }
            PatternKind::NullCheck(a) | PatternKind::NullAssert(a) | PatternKind::Parenthesized(a) => {
                self.padrao_declarado(*a, grupo)
            }
            PatternKind::Cast { pattern, .. } => self.padrao_declarado(*pattern, grupo),
            PatternKind::List { elements, .. } => {
                for e in elements.iter() {
                    if let ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) = e {
                        self.padrao_declarado(*x, grupo);
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for e in entries.iter() {
                    self.expr(e.key);
                    self.padrao_declarado(e.value, grupo);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.padrao_declarado(f.pattern, grupo);
                }
            }
            PatternKind::Wildcard { .. } => {}
        }
    }

    /// Padrão de atribuição: as variáveis são escritas, não declaradas.
    fn padrao_de_atribuicao(&mut self, p: PatternId) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, .. } => self.referir(*name, false),
            PatternKind::Constant(x) | PatternKind::Relational { value: x, .. } => self.expr(*x),
            PatternKind::Or(a, b) | PatternKind::And(a, b) => {
                self.padrao_de_atribuicao(*a);
                self.padrao_de_atribuicao(*b);
            }
            PatternKind::NullCheck(a) | PatternKind::NullAssert(a) | PatternKind::Parenthesized(a) => {
                self.padrao_de_atribuicao(*a)
            }
            PatternKind::Cast { pattern, .. } => self.padrao_de_atribuicao(*pattern),
            PatternKind::List { elements, .. } => {
                for e in elements.iter() {
                    if let ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) = e {
                        self.padrao_de_atribuicao(*x);
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for e in entries.iter() {
                    self.expr(e.key);
                    self.padrao_de_atribuicao(e.value);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.padrao_de_atribuicao(f.pattern);
                }
            }
            PatternKind::Wildcard { .. } => {}
        }
    }

    // -- Comandos ------------------------------------------------------------

    fn for_init(&mut self, init: Option<&ForInit>) {
        match init {
            Some(ForInit::Variables(vl)) => self.variaveis(vl),
            Some(ForInit::Expression(x)) => self.expr_ctx(*x, true),
            None => {}
        }
    }

    fn for_in_alvo(&mut self, t: &ForInTarget) {
        match t {
            ForInTarget::Declared { name, .. } => self.declarar(*name, Especie::Variavel, None),
            ForInTarget::Pattern { pattern, .. } => {
                let g = self.novo_grupo();
                self.padrao_declarado(*pattern, Some(g));
            }
            ForInTarget::Expression(x) => match &self.ast.expr(*x).kind {
                ExprKind::Identifier(n) => self.referir(*n, false),
                _ => self.expr(*x),
            },
        }
    }

    fn novo_grupo(&mut self) -> usize {
        self.grupos += 1;
        self.grupos
    }

    fn variaveis(&mut self, vl: &ast::VariableList) {
        for v in vl.variables.iter() {
            if let Some(i) = v.initializer {
                self.expr(i);
            }
            self.declarar(v.name, Especie::Variavel, None);
        }
    }

    fn bloco(&mut self, lista: &[StmtId]) {
        self.entrar();
        for &s in lista {
            self.stmt(s);
        }
        self.sair();
    }

    fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        match &ast.stmt(s).kind {
            StmtKind::Block(l) => self.bloco(l),
            StmtKind::Variables(vl) => self.variaveis(vl),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(*value);
                let g = self.novo_grupo();
                self.padrao_declarado(*pattern, Some(g));
            }
            StmtKind::Function(f) => {
                if let Some(n) = ast.function(*f).name {
                    self.declarar(n, Especie::Funcao, None);
                }
                self.funcao(*f, false);
            }
            StmtKind::Expression(x) => self.expr_ctx(*x, true),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                self.entrar();
                if let Some(p) = case_pattern {
                    self.padrao_declarado(*p, None);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.stmt(*then);
                self.sair();
                if let Some(e) = else_ {
                    self.stmt(*e);
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                self.entrar();
                self.for_init(init.as_ref());
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for u in updates.iter() {
                    self.expr_ctx(*u, true);
                }
                self.stmt(*body);
                self.sair();
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                self.expr(*iterable);
                self.entrar();
                self.for_in_alvo(target);
                self.stmt(*body);
                self.sair();
            }
            StmtKind::While { condition, body } | StmtKind::DoWhile { body, condition } => {
                self.expr(*condition);
                self.stmt(*body);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value);
                for c in cases.iter() {
                    self.entrar();
                    if let Some(p) = c.pattern {
                        self.padrao_declarado(p, None);
                    }
                    if let Some(g) = c.guard {
                        self.expr(g);
                    }
                    for &b in c.body.iter() {
                        self.stmt(b);
                    }
                    self.sair();
                }
            }
            StmtKind::Return(x) => {
                if let Some(x) = x {
                    self.expr(*x);
                }
            }
            StmtKind::Yield { value, .. } => self.expr(*value),
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(*body);
                for c in catches.iter() {
                    self.entrar();
                    for n in [c.exception, c.stack_trace].into_iter().flatten() {
                        self.declarar(n, Especie::Oculta, None);
                    }
                    self.stmt(c.body);
                    self.sair();
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
            StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Empty => {}
        }
    }

    // -- Funções -------------------------------------------------------------

    fn parametros(&mut self, ps: &[Parameter]) {
        for p in ps {
            if let Some(d) = p.default_value {
                self.expr(d);
            }
        }
        for p in ps {
            if let Some(n) = p.name {
                self.declarar(n, Especie::Oculta, None);
            }
        }
    }

    fn corpo(&mut self, b: &FunctionBody) {
        match b {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(x) => self.expr(*x),
            FunctionBody::Empty | FunctionBody::Native(_) => {}
        }
    }

    /// Uma função (de topo, membro, local ou expressão): parâmetros num
    /// escopo próprio, depois o corpo.
    fn funcao(&mut self, f: FunctionId, _raiz: bool) {
        let ast = self.ast;
        let func = ast.function(f);
        self.entrar();
        if let Some(ps) = &func.parameters {
            self.parametros(ps);
        }
        self.corpo(&func.body);
        self.sair();
    }

    fn inicializadores(&mut self, inits: &[Initializer]) {
        for i in inits {
            match i {
                Initializer::Field { value, .. } => self.expr(*value),
                Initializer::Super { arguments, .. } | Initializer::Redirect { arguments, .. } => {
                    self.argumentos(arguments)
                }
                Initializer::Assert { condition, message, .. } => {
                    self.expr(*condition);
                    if let Some(m) = message {
                        self.expr(*m);
                    }
                }
            }
        }
    }

    fn membro(&mut self, m: ast::MemberId) {
        let ast = self.ast;
        match &ast.member(m).kind {
            MemberKind::Field(vl) => {
                for v in vl.variables.iter() {
                    if let Some(i) = v.initializer {
                        self.expr(i);
                    }
                }
            }
            MemberKind::Method(f) => self.funcao(*f, true),
            MemberKind::Constructor(k) => {
                self.entrar();
                self.parametros(&k.parameters);
                self.inicializadores(&k.initializers);
                self.corpo(&k.body);
                self.sair();
            }
        }
    }
}

/// Locais não usados de uma unidade.
pub fn nao_usados(u: Unidade<'_>, interner: &Interner, curinga: bool) -> Vec<Diagnostic> {
    let ast = u.ast;
    let mut v = Visita { ast, interner, curinga, locais: Vec::new(), escopos: vec![Vec::new()], grupos: 0 };
    for &d in &u.unit.declarations {
        match &ast.decl(d).kind {
            DeclKind::Function(f) => v.funcao(*f, true),
            DeclKind::Variables(vl) => {
                for var in vl.variables.iter() {
                    if let Some(i) = var.initializer {
                        v.expr(i);
                    }
                }
            }
            DeclKind::Class(x) => x.members.iter().for_each(|m| v.membro(*m)),
            DeclKind::Mixin(x) => x.members.iter().for_each(|m| v.membro(*m)),
            DeclKind::Extension(x) => x.members.iter().for_each(|m| v.membro(*m)),
            DeclKind::ExtensionType(x) => x.members.iter().for_each(|m| v.membro(*m)),
            DeclKind::Enum(x) => {
                for k in &x.constants {
                    if let Some(a) = &k.arguments {
                        v.argumentos(a);
                    }
                }
                x.members.iter().for_each(|m| v.membro(*m));
            }
            DeclKind::Typedef(_) => {}
        }
    }
    let mut grupos_lidos = std::collections::HashSet::new();
    for l in &v.locais {
        if let (Some(g), true) = (l.grupo, l.lido) {
            grupos_lidos.insert(g);
        }
    }
    let mut out = Vec::new();
    for l in &v.locais {
        let nome = interner.resolve(l.nome);
        // `_isNamedWildcard`: só `_`s (com o recurso, só `_`, que nem declara).
        let so_sublinhados = nome.bytes().all(|b| b == b'_') && !(curinga && nome.len() > 1);
        if l.lido || so_sublinhados || l.grupo.is_some_and(|g| grupos_lidos.contains(&g)) {
            continue;
        }
        match l.especie {
            Especie::Variavel => out.push(Diagnostic::com_codigo(w::UNUSED_LOCAL_VARIABLE, l.span, [nome])),
            Especie::Funcao => out.push(Diagnostic::com_codigo(w::UNUSED_ELEMENT, l.span, [nome])),
            Especie::Oculta => {}
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    fn rodar(fonte: &str) -> Vec<(String, String)> {
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut interner);
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        let u = Unidade { ast: &p.ast, unit: &p.unit };
        let mut v: Vec<_> = nao_usados(u, &interner, false)
            .into_iter()
            .map(|d| (d.code.unwrap().info().nome.to_string(), fonte[d.span.start..d.span.end].to_string()))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn leituras_e_escritas() {
        let f = "void f(int p) {\n  var a = 1;\n  var b = 2;\n  b = 3;\n  var c = 0;\n  c++;\n  var d = 0;\n  print(d++);\n  var e = 0;\n  e += 1;\n  int? g;\n  g ??= 1;\n  var (h, i) = (1, 2);\n  print(h);\n  var (j, k) = (1, 2);\n  for (var x in [1]) {}\n  void l() {}\n  var _ = 1;\n  var p2 = () => a;\n  print(p2);\n}\n";
        let r = rodar(f);
        let nomes: Vec<&str> = r.iter().map(|x| x.1.as_str()).collect();
        assert_eq!(nomes, vec!["l", "b", "c", "e", "j", "k", "x"], "{r:?}");
        assert_eq!(r[0].0, "unused_element");
    }

    #[test]
    fn sombra_e_closure() {
        let f = "void f() {\n  var a = 1;\n  void g(int a) { print(a); }\n  g(0);\n}\n";
        let r = rodar(f);
        assert_eq!(r, vec![("unused_local_variable".to_string(), "a".to_string())]);
    }
}
