//! Condições booleanas constantes no programa inteiro
//! (`docs/JS-PRODUCAO-TAMANHO.md` §3.3).
//!
//! Com os `assert` desligados (o padrão do perfil de produção, como no
//! `dart2js`), o `isDevMode` do ngdart —
//! `bool get _assertionsEnabled { var enabled = false; assert(enabled = true); return enabled; }` —
//! vale `false`, e com ele `isDevToolsEnabled` (`isDevMode && _isDevToolsEnabled`)
//! e `debugThrowIfChanged`. O dart2js prova isso pela inferência
//! (`inferrer/engine.dart:850-884`) e apaga o desvio morto. Aqui a prova é
//! sintática e conservadora: literais, `!`, `&&`/`||` (com curto-circuito),
//! `==`/`!=` entre literais, `const` e `final` de topo com inicializador
//! avaliável, e *getters* de topo ou estáticos sem parâmetros cujo corpo é uma
//! expressão avaliável ou um bloco só de declarações locais com literal,
//! `assert` (ignorado sem asserts) e um `return` final. Qualquer outra coisa
//! é desconhecida, e o código fica.
//!
//! O mundo (o desvio morto não é percorrido) e o emissor (o desvio morto não é
//! emitido) usam a mesma resposta, então o verificador do texto concorda.

use crate::Entrada;
use dartforge_elements::model::{Element, FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, FunctionBody, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::Resolved;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Valor {
    Bool(bool),
    Null,
}

/// O avaliador, com memo por *getter* e por variável.
pub struct Constantes<'a> {
    e: Entrada<'a>,
    sem_asserts: bool,
    memo_fn: RefCell<HashMap<FunctionElementId, Option<Valor>>>,
    memo_var: RefCell<HashMap<VariableId, Option<Valor>>>,
}

impl<'a> Constantes<'a> {
    pub fn nova(e: Entrada<'a>, sem_asserts: bool) -> Constantes<'a> {
        Constantes { e, sem_asserts, memo_fn: RefCell::new(HashMap::new()), memo_var: RefCell::new(HashMap::new()) }
    }

    /// Os `assert` estão desligados (o emissor não os escreve).
    pub fn sem_asserts(&self) -> bool {
        self.sem_asserts
    }

    /// O valor booleano constante de `x` (na unidade `u`), se provado.
    pub fn bool_de(&self, u: UnitId, x: ExprId) -> Option<bool> {
        match self.expr(u, x, &HashMap::new(), 0)? {
            Valor::Bool(b) => Some(b),
            Valor::Null => None,
        }
    }

    fn resolvido(&self, u: UnitId, x: ExprId) -> Option<&'a Resolved> {
        self.e.bodies.units.get(u.0 as usize)?.get_resolved(x)
    }

    fn expr(&self, u: UnitId, x: ExprId, locais: &HashMap<SymbolId, Valor>, prof: u32) -> Option<Valor> {
        if prof > 24 {
            return None;
        }
        let ast = &self.e.program.unit(u).ast;
        match &ast.expr(x).kind {
            ExprKind::Bool(b) => Some(Valor::Bool(*b)),
            ExprKind::Null => Some(Valor::Null),
            ExprKind::Parenthesized(i) => self.expr(u, *i, locais, prof + 1),
            ExprKind::Unary { op: UnaryOp::Not, operand } => match self.expr(u, *operand, locais, prof + 1)? {
                Valor::Bool(b) => Some(Valor::Bool(!b)),
                Valor::Null => None,
            },
            ExprKind::Binary { op: BinaryOp::And, left, right } => match self.expr(u, *left, locais, prof + 1)? {
                Valor::Bool(false) => Some(Valor::Bool(false)),
                Valor::Bool(true) => match self.expr(u, *right, locais, prof + 1)? {
                    Valor::Bool(b) => Some(Valor::Bool(b)),
                    Valor::Null => None,
                },
                Valor::Null => None,
            },
            ExprKind::Binary { op: BinaryOp::Or, left, right } => match self.expr(u, *left, locais, prof + 1)? {
                Valor::Bool(true) => Some(Valor::Bool(true)),
                Valor::Bool(false) => match self.expr(u, *right, locais, prof + 1)? {
                    Valor::Bool(b) => Some(Valor::Bool(b)),
                    Valor::Null => None,
                },
                Valor::Null => None,
            },
            ExprKind::Binary { op: op @ (BinaryOp::Eq | BinaryOp::NotEq), left, right } => {
                let a = self.expr(u, *left, locais, prof + 1)?;
                let b = self.expr(u, *right, locais, prof + 1)?;
                Some(Valor::Bool((a == b) == (*op == BinaryOp::Eq)))
            }
            ExprKind::Identifier(n) => match self.resolvido(u, x)? {
                Resolved::Local(_) => locais.get(&n.sym).copied(),
                Resolved::Element(el) => self.elemento(*el, prof),
                _ => None,
            },
            ExprKind::Property { .. } => match self.resolvido(u, x)? {
                Resolved::Element(el) => self.elemento(*el, prof),
                _ => None,
            },
            _ => None,
        }
    }

    fn elemento(&self, el: Element, prof: u32) -> Option<Valor> {
        match el {
            Element::Function(f) => self.getter(f, prof),
            Element::Variable(v) => self.variavel(v, prof),
            _ => None,
        }
    }

    fn variavel(&self, v: VariableId, prof: u32) -> Option<Valor> {
        if let Some(r) = self.memo_var.borrow().get(&v) {
            return *r;
        }
        self.memo_var.borrow_mut().insert(v, None);
        let p = self.e.program;
        let var = p.variable(v);
        let r = (|| {
            if var.external || var.extension.is_some() {
                return None;
            }
            let (u, lista) = match var.node {
                VariableRef::TopLevel { unit, decl, index } => match &p.unit(unit).ast.decl(decl).kind {
                    ast::DeclKind::Variables(l) => (unit, (l, index)),
                    _ => return None,
                },
                VariableRef::Field { unit, member, index } => match &p.unit(unit).ast.member(member).kind {
                    ast::MemberKind::Field(l) => (unit, (l, index)),
                    _ => return None,
                },
                _ => return None,
            };
            let (l, index) = lista;
            // `const`, ou `final` não `late` de topo/estático: nunca reescrita.
            if !(l.const_ || (l.final_ && !l.late)) || (var.class.is_some() && !var.static_) {
                return None;
            }
            let init = l.variables.get(index)?.initializer?;
            self.expr(u, init, &HashMap::new(), prof + 1)
        })();
        self.memo_var.borrow_mut().insert(v, r);
        r
    }

    fn getter(&self, f: FunctionElementId, prof: u32) -> Option<Valor> {
        if let Some(r) = self.memo_fn.borrow().get(&f) {
            return *r;
        }
        // Recursão: desconhecido até terminar.
        self.memo_fn.borrow_mut().insert(f, None);
        let p = self.e.program;
        let func = p.function(f);
        let r = (|| {
            if let Some(v) = func.variable {
                if func.kind == FunctionKind::ImplicitAccessor {
                    return self.variavel(v, prof);
                }
            }
            if func.kind != FunctionKind::Getter || func.external || (func.class.is_some() && !func.static_) || func.extension.is_some() {
                return None;
            }
            let FunctionRef::Function { unit, function } = func.node else { return None };
            let ast = &p.unit(unit).ast;
            let fun = ast.function(function);
            if fun.modifier != ast::AsyncModifier::None {
                return None;
            }
            match fun.body {
                FunctionBody::Expression(x) => self.expr(unit, x, &HashMap::new(), prof + 1),
                FunctionBody::Block(b) => self.bloco(unit, b, prof),
                _ => None,
            }
        })();
        self.memo_fn.borrow_mut().insert(f, r);
        r
    }

    /// `{ var a = lit; …; assert(…); return e; }` — só com asserts desligados
    /// o `assert` some; com eles ligados, qualquer `assert` torna o corpo
    /// desconhecido (ele pode escrever num local).
    fn bloco(&self, u: UnitId, b: ast::StmtId, prof: u32) -> Option<Valor> {
        let ast = &self.e.program.unit(u).ast;
        let StmtKind::Block(ss) = &ast.stmt(b).kind else { return None };
        let mut locais: HashMap<SymbolId, Valor> = HashMap::new();
        for (i, s) in ss.iter().enumerate() {
            match &ast.stmt(*s).kind {
                StmtKind::Variables(l) if !l.late => {
                    for v in l.variables.iter() {
                        let x = v.initializer?;
                        let val = self.expr(u, x, &locais, prof + 1)?;
                        locais.insert(v.name.sym, val);
                    }
                }
                StmtKind::Assert { .. } if self.sem_asserts => {}
                StmtKind::Return(Some(x)) if i + 1 == ss.len() => return self.expr(u, *x, &locais, prof + 1),
                _ => return None,
            }
        }
        None
    }
}
