//! Um passeio pela árvore de uma unidade com a pilha de ancestrais, para as
//! regras de lint que perguntam "dentro de quê" (o `thisOrAncestorMatching`
//! do analyzer): a árvore do DartForge fica em arenas e os nós não conhecem
//! o pai.
//!
//! A visita é em pré-ordem. Entram as declarações, os membros, as funções
//! (declaradas, locais e literais), os comandos, as expressões e os
//! padrões; também os argumentos das anotações e os valores padrão dos
//! parâmetros. Os tipos não são nós do passeio. As anotações e os
//! argumentos das constantes de enum entram como nós só para marcar o
//! contexto constante dos argumentos.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, CollectionElement, DeclId, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody, FunctionId, Initializer,
    ListPatternElement, MemberId, MemberKind, PatternId, PatternKind, StmtId, StmtKind, StringPart,
};

/// Um nó do passeio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum No {
    Decl(DeclId),
    Membro(MemberId),
    Funcao(FunctionId),
    Stmt(StmtId),
    Expr(ExprId),
    Padrao(PatternId),
    /// Uma anotação com argumentos (o intervalo dela): os argumentos são
    /// os filhos. Contexto constante.
    Anotacao(Span),
    /// Os argumentos de uma constante de enum (o intervalo da constante).
    /// Contexto constante.
    ArgumentosDeEnum(Span),
}

struct Andador<'a, 'f> {
    a: &'a Ast,
    pilha: Vec<No>,
    f: &'f mut dyn FnMut(No, &[No]),
}

/// Chama `f(nó, ancestrais)` para cada nó da unidade; `ancestrais` vai da
/// declaração de topo até o pai do nó.
pub fn andar(u: Unidade<'_>, f: &mut dyn FnMut(No, &[No])) {
    let mut x = Andador { a: u.ast, pilha: Vec::new(), f };
    for &d in u.unit.declarations.iter() {
        x.decl(d);
    }
}

impl<'a> Andador<'a, '_> {
    fn entra(&mut self, no: No) {
        (self.f)(no, &self.pilha);
        self.pilha.push(no);
    }

    fn sai(&mut self) {
        self.pilha.pop();
    }

    fn anotacoes(&mut self, anotacoes: &[ast::Annotation]) {
        for m in anotacoes {
            if let Some(args) = &m.arguments {
                self.entra(No::Anotacao(m.span));
                self.argumentos(args);
                self.sai();
            }
        }
    }

    fn argumentos(&mut self, args: &ast::Arguments) {
        for x in args.args.iter() {
            self.expr(x.value);
        }
    }

    fn parametros(&mut self, ps: &[ast::Parameter]) {
        for p in ps {
            self.anotacoes(&p.metadata);
            if let Some(d) = p.default_value {
                self.expr(d);
            }
            if let Some(internos) = &p.function_parameters {
                self.parametros(internos);
            }
        }
    }

    fn lista(&mut self, l: &ast::VariableList) {
        for v in l.variables.iter() {
            if let Some(i) = v.initializer {
                self.expr(i);
            }
        }
    }

    fn corpo(&mut self, b: &FunctionBody) {
        match b {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(e) => self.expr(*e),
            FunctionBody::Empty | FunctionBody::Native(_) => {}
        }
    }

    fn membros(&mut self, primario: Option<MemberId>, membros: &[MemberId]) {
        if let Some(p) = primario
            && !membros.contains(&p)
        {
            self.membro(p);
        }
        for &m in membros {
            self.membro(m);
        }
    }

    fn decl(&mut self, d: DeclId) {
        let a: &'a Ast = self.a;
        let n = a.decl(d);
        self.entra(No::Decl(d));
        self.anotacoes(&n.metadata);
        match &n.kind {
            DeclKind::Class(x) => self.membros(x.primary_constructor, &x.members),
            DeclKind::Mixin(x) => self.membros(None, &x.members),
            DeclKind::Enum(x) => {
                for k in x.constants.iter() {
                    self.anotacoes(&k.metadata);
                    if let Some(args) = &k.arguments {
                        self.entra(No::ArgumentosDeEnum(k.span));
                        self.argumentos(args);
                        self.sai();
                    }
                }
                self.membros(x.primary_constructor, &x.members);
            }
            DeclKind::Extension(x) => self.membros(None, &x.members),
            DeclKind::ExtensionType(x) => {
                self.anotacoes(&x.representation_metadata);
                self.membros(None, &x.members);
            }
            DeclKind::Typedef(_) => {}
            DeclKind::Function(f) => self.funcao(*f),
            DeclKind::Variables(l) => self.lista(l),
        }
        self.sai();
    }

    fn membro(&mut self, m: MemberId) {
        let a: &'a Ast = self.a;
        let n = a.member(m);
        self.entra(No::Membro(m));
        self.anotacoes(&n.metadata);
        match &n.kind {
            MemberKind::Field(l) => self.lista(l),
            MemberKind::Method(f) => self.funcao(*f),
            MemberKind::Constructor(k) => {
                self.parametros(&k.parameters);
                for i in k.initializers.iter() {
                    match i {
                        Initializer::Field { value, .. } => self.expr(*value),
                        Initializer::Super { arguments, .. } | Initializer::Redirect { arguments, .. } => self.argumentos(arguments),
                        Initializer::Assert { condition, message, .. } => {
                            self.expr(*condition);
                            if let Some(m) = message {
                                self.expr(*m);
                            }
                        }
                    }
                }
                self.corpo(&k.body);
            }
        }
        self.sai();
    }

    fn funcao(&mut self, f: FunctionId) {
        let a: &'a Ast = self.a;
        let n = a.function(f);
        self.entra(No::Funcao(f));
        if let Some(ps) = &n.parameters {
            self.parametros(ps);
        }
        self.corpo(&n.body);
        self.sai();
    }

    fn inicio_do_for(&mut self, i: &ForInit) {
        match i {
            ForInit::Variables(l) => self.lista(l),
            ForInit::Expression(e) => self.expr(*e),
            ForInit::Pattern { pattern, value, .. } => {
                self.padrao(*pattern);
                self.expr(*value);
            }
        }
    }

    fn alvo_do_for(&mut self, t: &ForInTarget) {
        match t {
            ForInTarget::Declared { .. } => {}
            ForInTarget::Pattern { pattern, .. } => self.padrao(*pattern),
            ForInTarget::Expression(e) => self.expr(*e),
        }
    }

    fn stmt(&mut self, s: StmtId) {
        let a: &'a Ast = self.a;
        let n = a.stmt(s);
        self.entra(No::Stmt(s));
        match &n.kind {
            StmtKind::Block(ss) => {
                for &x in ss.iter() {
                    self.stmt(x);
                }
            }
            StmtKind::Variables(l) => self.lista(l),
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
                if let Some(i) = init {
                    self.inicio_do_for(i);
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &x in updates.iter() {
                    self.expr(x);
                }
                self.stmt(*body);
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                self.alvo_do_for(target);
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
                for k in cases.iter() {
                    if let Some(p) = k.pattern {
                        self.padrao(p);
                    }
                    if let Some(g) = k.guard {
                        self.expr(g);
                    }
                    for &x in k.body.iter() {
                        self.stmt(x);
                    }
                }
            }
            StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Empty => {}
            StmtKind::Return(e) => {
                if let Some(e) = e {
                    self.expr(*e);
                }
            }
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
        }
        self.sai();
    }

    fn elemento(&mut self, e: &CollectionElement) {
        match e {
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
                self.elemento(then);
                if let Some(x) = else_ {
                    self.elemento(x);
                }
            }
            CollectionElement::For { init, condition, updates, body, .. } => {
                if let Some(i) = init {
                    self.inicio_do_for(i);
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &x in updates.iter() {
                    self.expr(x);
                }
                self.elemento(body);
            }
            CollectionElement::ForIn { target, iterable, body, .. } => {
                self.alvo_do_for(target);
                self.expr(*iterable);
                self.elemento(body);
            }
        }
    }

    fn expr(&mut self, e: ExprId) {
        let a: &'a Ast = self.a;
        let n = a.expr(e);
        self.entra(No::Expr(e));
        match &n.kind {
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Symbol(_)
            | ExprKind::Identifier(_)
            | ExprKind::This
            | ExprKind::Super
            | ExprKind::CascadeTarget
            | ExprKind::Rethrow
            | ExprKind::DotShorthand { .. } => {}
            ExprKind::String(lit) => {
                for parte in lit.parts.iter() {
                    if let StringPart::Interpolation(x) = parte {
                        self.expr(*x);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.elemento(el);
                }
            }
            ExprKind::Record { positional, named, .. } => {
                for &x in positional.iter() {
                    self.expr(x);
                }
                for (_, x) in named.iter() {
                    self.expr(*x);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => self.argumentos(arguments),
            ExprKind::FunctionExpression(f) => self.funcao(*f),
            ExprKind::Property { target, .. } | ExprKind::TypeArguments { target, .. } => self.expr(*target),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target);
                self.expr(*index);
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target);
                self.argumentos(arguments);
            }
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
                for &x in sections.iter() {
                    self.expr(x);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value);
                for k in cases.iter() {
                    self.padrao(k.pattern);
                    if let Some(g) = k.guard {
                        self.expr(g);
                    }
                    self.expr(k.body);
                }
            }
        }
        self.sai();
    }

    fn padrao(&mut self, p: PatternId) {
        let a: &'a Ast = self.a;
        let n = a.pattern(p);
        self.entra(No::Padrao(p));
        match &n.kind {
            PatternKind::Wildcard { .. } | PatternKind::Variable { .. } => {}
            PatternKind::Constant(e) | PatternKind::Relational { value: e, .. } => self.expr(*e),
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.padrao(*x);
                self.padrao(*y);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => {
                self.padrao(*x)
            }
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => self.padrao(*x),
                        ListPatternElement::Rest(None) => {}
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
                for c in fields.iter() {
                    self.padrao(c.pattern);
                }
            }
        }
        self.sai();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_intern::Interner;

    #[test]
    fn pilha_de_ancestrais() {
        let fonte = "void f() {\n  try {\n  } finally {\n    g(() => 1);\n  }\n}\n";
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        let mut inteiros = Vec::new();
        andar(u, &mut |no, pilha| {
            if let No::Expr(e) = no
                && matches!(p.ast.expr(e).kind, ExprKind::Int(_))
            {
                inteiros.push(pilha.iter().filter(|n| matches!(n, No::Funcao(_))).count());
                assert!(matches!(pilha[0], No::Decl(_)));
            }
        });
        // O `1` está dentro de `f` e da função literal.
        assert_eq!(inteiros, vec![2]);
    }
}
