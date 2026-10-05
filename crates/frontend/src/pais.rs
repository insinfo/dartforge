//! O pai de cada expressão de uma unidade, no papel que as consultas de
//! ancestral do analyzer distinguem, e o `inConstantContext` da 3.6.2
//! (`ExpressionImpl.constantContext`, `analyzer/lib/src/dart/ast/ast.dart:6208`).
//!
//! O `constantContext` sobe a partir do pai da expressão: atravessa
//! expressões, listas de argumentos, elementos `if`/`for` de coleção (pelo
//! corpo e pela condição do `if`), entradas de mapa, `...` e declarações de
//! variável; para com verdadeiro na anotação, nos argumentos de constante de
//! enum, na criação/coleção/record com `const` escrito, na lista de
//! variáveis `const`, no padrão constante com `const` e no `case` antigo
//! (`SwitchCase`, biblioteca antes da 3.0); para com falso na lista de
//! variáveis sem `const`, no padrão constante sem `const` e em qualquer
//! outro pai (instrução, corpo de função, interpolação, valor padrão,
//! inicializador de construtor, partes do `for`, guarda `when`, corpo de
//! caso de `switch`, elemento `?e`).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::ast::{
    Annotation, Arguments, Ast, CollectionElement, CompilationUnit, CreationKeyword, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, MemberKind,
    Parameter, PatternId, PatternKind, StmtKind, TypeKind, TypeParameter, TypedefKind, VariableList,
};

/// O papel do pai de uma expressão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pai {
    /// Um pai que o `constantContext` não atravessa (e o que não foi visto).
    Fronteira,
    /// Uma expressão que o contém por um papel transparente (operando,
    /// alvo, argumento, elemento de coleção, condição de `if` de coleção,
    /// campo de record, seção de cascata…).
    Expr(ExprId),
    /// Argumento de anotação.
    Anotacao,
    /// Argumento de constante de enum (`EnumConstantArguments`).
    ConstanteDeEnum,
    /// Inicializador de variável de uma lista (`const` escrito ou não).
    Variavel { constante: bool },
    /// Expressão de um padrão constante (`const` escrito ou não) ou do
    /// `case` antigo (constante).
    PadraoConstante { constante: bool },
}

/// Os pais das expressões de uma unidade, por `ExprId`.
pub struct Pais {
    pais: Vec<Pai>,
}

impl Pais {
    /// `antes_de_3`: a biblioteca é de antes da 3.0 (o `case e:` do
    /// `switch` é o `SwitchCase` antigo, contexto constante).
    pub fn novo(a: &Ast, unidade: &CompilationUnit, fonte: &str, antes_de_3: bool) -> Pais {
        let mut p = Construtor { a, fonte, pais: vec![Pai::Fronteira; a.exprs.len()] };
        p.tudo(unidade, antes_de_3);
        Pais { pais: p.pais }
    }

    pub fn pai(&self, e: ExprId) -> Pai {
        self.pais.get(e.0 as usize).copied().unwrap_or(Pai::Fronteira)
    }

    /// `Expression.inConstantContext`.
    pub fn em_contexto_constante(&self, a: &Ast, e: ExprId) -> bool {
        let mut atual = self.pai(e);
        // A árvore é finita; o limite só protege de um ciclo por erro.
        for _ in 0..a.exprs.len() + 1 {
            match atual {
                Pai::Fronteira => return false,
                Pai::Anotacao | Pai::ConstanteDeEnum => return true,
                Pai::Variavel { constante } | Pai::PadraoConstante { constante } => return constante,
                Pai::Expr(p) => {
                    let constante = match &a.expr(p).kind {
                        ExprKind::InstanceCreation { keyword, .. } => *keyword == Some(CreationKeyword::Const),
                        ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } | ExprKind::Record { const_, .. } => *const_,
                        _ => false,
                    };
                    if constante {
                        return true;
                    }
                    atual = self.pai(p);
                }
            }
        }
        false
    }
}

struct Construtor<'a> {
    a: &'a Ast,
    fonte: &'a str,
    pais: Vec<Pai>,
}

impl Construtor<'_> {
    fn por(&mut self, filho: ExprId, pai: Pai) {
        if let Some(x) = self.pais.get_mut(filho.0 as usize) {
            *x = pai;
        }
    }

    fn argumentos(&mut self, args: &Arguments, pai: Pai) {
        for x in args.args.iter() {
            self.por(x.value, pai);
        }
    }

    fn anotacoes(&mut self, ms: &[Annotation]) {
        for m in ms {
            if let Some(args) = &m.arguments {
                self.argumentos(args, Pai::Anotacao);
            }
        }
    }

    fn parametros_de_tipo(&mut self, ps: &[TypeParameter]) {
        for p in ps {
            self.anotacoes(&p.metadata);
        }
    }

    fn parametros(&mut self, ps: &[Parameter]) {
        for p in ps {
            self.anotacoes(&p.metadata);
            self.parametros_de_tipo(&p.function_type_params);
            if let Some(fs) = &p.function_parameters {
                self.parametros(fs);
            }
        }
    }

    fn lista(&mut self, l: &VariableList) {
        for v in l.variables.iter() {
            if let Some(i) = v.initializer {
                self.por(i, Pai::Variavel { constante: l.const_ });
            }
        }
    }

    /// Os elementos de coleção do literal `dono`.
    fn elementos(&mut self, dono: ExprId, es: &[CollectionElement]) {
        for e in es {
            self.elemento(dono, e);
        }
    }

    fn elemento(&mut self, dono: ExprId, e: &CollectionElement) {
        let pai = Pai::Expr(dono);
        match e {
            CollectionElement::Expression(x) => self.por(*x, pai),
            // `NullAwareElement` não é atravessado.
            CollectionElement::NullAwareExpression(_) => {}
            CollectionElement::MapEntry { key, value, .. } => {
                self.por(*key, pai);
                self.por(*value, pai);
            }
            CollectionElement::Spread { value, .. } => self.por(*value, pai),
            CollectionElement::If { condition, then, else_, .. } => {
                // A condição é filha do `IfElement`; o padrão e a guarda,
                // não.
                self.por(*condition, pai);
                self.elemento(dono, then);
                if let Some(x) = else_ {
                    self.elemento(dono, x);
                }
            }
            CollectionElement::For { init, body, .. } => {
                // As partes do laço são filhas do `ForParts…`: fronteira; a
                // lista de variáveis delas, pela própria regra.
                if let Some(ForInit::Variables(l)) = init {
                    self.lista(l);
                }
                self.elemento(dono, body);
            }
            CollectionElement::ForIn { target, body, .. } => {
                if let ForInTarget::Declared { metadata, .. } = target {
                    self.anotacoes(metadata);
                }
                self.elemento(dono, body);
            }
        }
    }

    fn padrao_constante(&mut self, p: PatternId, case_antigo: bool) {
        let pt = self.a.pattern(p);
        if let PatternKind::Constant(e) = &pt.kind {
            let escrito = crate::fonte::palavra_em(self.fonte, pt.span.start, "const");
            self.por(*e, Pai::PadraoConstante { constante: escrito || case_antigo });
        }
    }

    fn tudo(&mut self, unidade: &CompilationUnit, antes_de_3: bool) {
        let a = self.a;
        for d in unidade.directives.iter() {
            self.anotacoes(&d.metadata);
        }
        // Os filhos de cada expressão.
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let pai = Pai::Expr(id);
            match &e.kind {
                ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.por(*x, pai),
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => self.elementos(id, elements),
                ExprKind::Record { positional, named, .. } => {
                    for &x in positional.iter() {
                        self.por(x, pai);
                    }
                    for (_, x) in named.iter() {
                        self.por(*x, pai);
                    }
                }
                ExprKind::InstanceCreation { arguments, .. } => self.argumentos(arguments, pai),
                ExprKind::Property { target, .. } | ExprKind::TypeArguments { target, .. } => self.por(*target, pai),
                ExprKind::Index { target, index, .. } => {
                    self.por(*target, pai);
                    self.por(*index, pai);
                }
                ExprKind::Call { target, arguments } => {
                    self.por(*target, pai);
                    self.argumentos(arguments, pai);
                }
                ExprKind::Unary { operand, .. } => self.por(*operand, pai),
                ExprKind::Binary { left, right, .. } => {
                    self.por(*left, pai);
                    self.por(*right, pai);
                }
                ExprKind::Conditional { condition, then, else_ } => {
                    self.por(*condition, pai);
                    self.por(*then, pai);
                    self.por(*else_, pai);
                }
                ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.por(*value, pai),
                ExprKind::Assign { target, value, .. } => {
                    self.por(*target, pai);
                    self.por(*value, pai);
                }
                ExprKind::PatternAssign { value, .. } => self.por(*value, pai),
                ExprKind::Cascade { target, sections, .. } => {
                    self.por(*target, pai);
                    for &s in sections.iter() {
                        self.por(s, pai);
                    }
                }
                // O valor do `switch` é filho da expressão; a guarda e o
                // corpo de cada caso, não.
                ExprKind::Switch { value, .. } => self.por(*value, pai),
                // As interpolações (`InterpolationExpression`) e o corpo de
                // uma expressão de função: fronteira.
                _ => {}
            }
        }
        // Os padrões constantes (em qualquer lugar: `case`, `if-case`,
        // declaração, atribuição, subpadrão).
        for k in 0..a.patterns.len() {
            self.padrao_constante(PatternId(k as u32), false);
        }
        // O `case e:` antigo de `switch` (antes da 3.0): `SwitchCase`.
        if antes_de_3 {
            for s in a.stmts.iter() {
                if let StmtKind::Switch { cases, .. } = &s.kind {
                    for c in cases.iter() {
                        if let Some(p) = c.pattern {
                            self.padrao_constante(p, true);
                        }
                    }
                }
            }
        }
        // As declarações.
        for d in a.decls.iter() {
            self.anotacoes(&d.metadata);
            match &d.kind {
                DeclKind::Class(x) => self.parametros_de_tipo(&x.type_params),
                DeclKind::Mixin(x) => self.parametros_de_tipo(&x.type_params),
                DeclKind::Enum(x) => {
                    self.parametros_de_tipo(&x.type_params);
                    for c in x.constants.iter() {
                        self.anotacoes(&c.metadata);
                        if let Some(args) = &c.arguments {
                            self.argumentos(args, Pai::ConstanteDeEnum);
                        }
                    }
                }
                DeclKind::Extension(x) => self.parametros_de_tipo(&x.type_params),
                DeclKind::ExtensionType(x) => {
                    self.parametros_de_tipo(&x.type_params);
                    self.anotacoes(&x.representation_metadata);
                }
                DeclKind::Typedef(t) => {
                    self.parametros_de_tipo(&t.type_params);
                    if let TypedefKind::Legacy { parameters, .. } = &t.kind {
                        self.parametros(parameters);
                    }
                }
                DeclKind::Function(_) => {}
                DeclKind::Variables(l) => self.lista(l),
            }
        }
        for m in a.members.iter() {
            self.anotacoes(&m.metadata);
            match &m.kind {
                MemberKind::Field(l) => self.lista(l),
                MemberKind::Constructor(k) => self.parametros(&k.parameters),
                MemberKind::Method(_) => {}
            }
        }
        for f in a.functions.iter() {
            self.parametros_de_tipo(&f.type_params);
            if let Some(ps) = &f.parameters {
                self.parametros(ps);
            }
        }
        for t in a.types.iter() {
            if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
                self.parametros_de_tipo(type_params);
                self.parametros(parameters);
            }
        }
        for (_, ms) in a.metadados_locais.iter() {
            self.anotacoes(ms);
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) => self.lista(l),
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => self.lista(l),
                StmtKind::ForIn { target: ForInTarget::Declared { metadata, .. }, .. } => self.anotacoes(metadata),
                _ => {}
            }
        }
    }
}
