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

/// Todas as anotações escritas na unidade (o que o `visitAnnotation` de um
/// visitante recursivo veria): das diretivas, das declarações e dos
/// membros, das constantes de enum, da representação de tipo de extensão,
/// dos parâmetros de tipo, dos parâmetros (também os de tipos `Function` e
/// os aninhados), das declarações locais e das variáveis de `for-in`.
pub fn todas_as_anotacoes<'a>(a: &'a Ast, unit: &'a CompilationUnit) -> Vec<&'a Annotation> {
    let mut v: Vec<&'a Annotation> = Vec::new();
    fn parameters_<'a>(ps: &'a [Parameter], v: &mut Vec<&'a Annotation>) {
        for p in ps {
            v.extend(p.metadata.iter());
            for t in p.function_type_params.iter() {
                v.extend(t.metadata.iter());
            }
            if let Some(fs) = &p.function_parameters {
                parameters_(fs, v);
            }
        }
    }
    fn types_<'a>(ts: &'a [TypeParameter], v: &mut Vec<&'a Annotation>) {
        for t in ts {
            v.extend(t.metadata.iter());
        }
    }
    for d in unit.directives.iter() {
        v.extend(d.metadata.iter());
    }
    for d in a.decls.iter() {
        v.extend(d.metadata.iter());
        match &d.kind {
            DeclKind::Class(x) => types_(&x.type_params, &mut v),
            DeclKind::Mixin(x) => types_(&x.type_params, &mut v),
            DeclKind::Enum(x) => {
                types_(&x.type_params, &mut v);
                for c in x.constants.iter() {
                    v.extend(c.metadata.iter());
                }
            }
            DeclKind::Extension(x) => types_(&x.type_params, &mut v),
            DeclKind::ExtensionType(x) => {
                types_(&x.type_params, &mut v);
                v.extend(x.representation_metadata.iter());
            }
            DeclKind::Typedef(t) => {
                types_(&t.type_params, &mut v);
                if let TypedefKind::Legacy { parameters, .. } = &t.kind {
                    parameters_(parameters, &mut v);
                }
            }
            DeclKind::Function(_) | DeclKind::Variables(_) => {}
        }
    }
    for m in a.members.iter() {
        v.extend(m.metadata.iter());
        if let MemberKind::Constructor(k) = &m.kind {
            parameters_(&k.parameters, &mut v);
        }
    }
    for f in a.functions.iter() {
        types_(&f.type_params, &mut v);
        if let Some(ps) = &f.parameters {
            parameters_(ps, &mut v);
        }
    }
    for t in a.types.iter() {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
            types_(type_params, &mut v);
            parameters_(parameters, &mut v);
        }
    }
    for (_, ms) in a.metadados_locais.iter() {
        v.extend(ms.iter());
    }
    for s in a.stmts.iter() {
        if let StmtKind::ForIn { target: ForInTarget::Declared { metadata, .. }, .. } = &s.kind {
            v.extend(metadata.iter());
        }
    }
    for e in a.exprs.iter() {
        if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
            fn from_element<'a>(el: &'a CollectionElement, v: &mut Vec<&'a Annotation>) {
                match el {
                    CollectionElement::ForIn { target, body, .. } => {
                        if let ForInTarget::Declared { metadata, .. } = target {
                            v.extend(metadata.iter());
                        }
                        from_element(body, v);
                    }
                    CollectionElement::For { body, .. } => from_element(body, v),
                    CollectionElement::If { then, else_, .. } => {
                        from_element(then, v);
                        if let Some(x) = else_ {
                            from_element(x, v);
                        }
                    }
                    _ => {}
                }
            }
            for el in elements.iter() {
                from_element(el, &mut v);
            }
        }
    }
    v
}

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
    Variavel { constant: bool },
    /// Expressão de um padrão constante (`const` escrito ou não) ou do
    /// `case` antigo (constante).
    PadraoConstante { constant: bool },
}

/// Os pais das expressões de uma unidade, por `ExprId`.
pub struct Pais {
    pais: Vec<Pai>,
}

impl Pais {
    /// `before_3`: a biblioteca é de antes da 3.0 (o `case e:` do
    /// `switch` é o `SwitchCase` antigo, contexto constante).
    pub fn novo(a: &Ast, unit: &CompilationUnit, fonte: &str, before_3: bool) -> Pais {
        let mut p = Constructor { a, fonte, pais: vec![Pai::Fronteira; a.exprs.len()] };
        p.everything(unit, before_3);
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
                Pai::Variavel { constant } | Pai::PadraoConstante { constant } => return constant,
                Pai::Expr(p) => {
                    let constant = match &a.expr(p).kind {
                        ExprKind::InstanceCreation { keyword, .. } => *keyword == Some(CreationKeyword::Const),
                        ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } | ExprKind::Record { const_, .. } => *const_,
                        _ => false,
                    };
                    if constant {
                        return true;
                    }
                    atual = self.pai(p);
                }
            }
        }
        false
    }
}

struct Constructor<'a> {
    a: &'a Ast,
    fonte: &'a str,
    pais: Vec<Pai>,
}

impl Constructor<'_> {
    fn por(&mut self, child: ExprId, pai: Pai) {
        if let Some(x) = self.pais.get_mut(child.0 as usize) {
            *x = pai;
        }
    }

    fn arguments_(&mut self, args: &Arguments, pai: Pai) {
        for x in args.args.iter() {
            self.por(x.value, pai);
        }
    }

    fn annotations(&mut self, ms: &[Annotation]) {
        for m in ms {
            if let Some(args) = &m.arguments {
                self.arguments_(args, Pai::Anotacao);
            }
        }
    }

    fn type_parameters_(&mut self, ps: &[TypeParameter]) {
        for p in ps {
            self.annotations(&p.metadata);
        }
    }

    fn parameters_(&mut self, ps: &[Parameter]) {
        for p in ps {
            self.annotations(&p.metadata);
            self.type_parameters_(&p.function_type_params);
            if let Some(fs) = &p.function_parameters {
                self.parameters_(fs);
            }
        }
    }

    fn list(&mut self, l: &VariableList) {
        for v in l.variables.iter() {
            if let Some(i) = v.initializer {
                self.por(i, Pai::Variavel { constant: l.const_ });
            }
        }
    }

    /// Os elementos de coleção do literal `owner`.
    fn elements_(&mut self, owner: ExprId, es: &[CollectionElement]) {
        for e in es {
            self.element(owner, e);
        }
    }

    fn element(&mut self, owner: ExprId, e: &CollectionElement) {
        let pai = Pai::Expr(owner);
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
                self.element(owner, then);
                if let Some(x) = else_ {
                    self.element(owner, x);
                }
            }
            CollectionElement::For { init, body, .. } => {
                // As partes do laço são filhas do `ForParts…`: fronteira; a
                // lista de variáveis delas, pela própria regra.
                if let Some(ForInit::Variables(l)) = init {
                    self.list(l);
                }
                self.element(owner, body);
            }
            CollectionElement::ForIn { target, body, .. } => {
                if let ForInTarget::Declared { metadata, .. } = target {
                    self.annotations(metadata);
                }
                self.element(owner, body);
            }
        }
    }

    fn constant_pattern(&mut self, p: PatternId, old_case: bool) {
        let pt = self.a.pattern(p);
        if let PatternKind::Constant(e) = &pt.kind {
            let written = crate::fonte::palavra_em(self.fonte, pt.span.start, "const");
            self.por(*e, Pai::PadraoConstante { constant: written || old_case });
        }
    }

    fn everything(&mut self, unit: &CompilationUnit, before_3: bool) {
        let a = self.a;
        for d in unit.directives.iter() {
            self.annotations(&d.metadata);
        }
        // Os filhos de cada expressão.
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let pai = Pai::Expr(id);
            match &e.kind {
                ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.por(*x, pai),
                ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => self.elements_(id, elements),
                ExprKind::Record { positional, named, .. } => {
                    for &x in positional.iter() {
                        self.por(x, pai);
                    }
                    for (_, x) in named.iter() {
                        self.por(*x, pai);
                    }
                }
                ExprKind::InstanceCreation { arguments, .. } => self.arguments_(arguments, pai),
                ExprKind::Property { target, .. } | ExprKind::TypeArguments { target, .. } => self.por(*target, pai),
                ExprKind::Index { target, index, .. } => {
                    self.por(*target, pai);
                    self.por(*index, pai);
                }
                ExprKind::Call { target, arguments } => {
                    self.por(*target, pai);
                    self.arguments_(arguments, pai);
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
            self.constant_pattern(PatternId(k as u32), false);
        }
        // O `case e:` antigo de `switch` (antes da 3.0): `SwitchCase`.
        if before_3 {
            for s in a.stmts.iter() {
                if let StmtKind::Switch { cases, .. } = &s.kind {
                    for c in cases.iter() {
                        if let Some(p) = c.pattern {
                            self.constant_pattern(p, true);
                        }
                    }
                }
            }
        }
        // As declarações.
        for d in a.decls.iter() {
            self.annotations(&d.metadata);
            match &d.kind {
                DeclKind::Class(x) => self.type_parameters_(&x.type_params),
                DeclKind::Mixin(x) => self.type_parameters_(&x.type_params),
                DeclKind::Enum(x) => {
                    self.type_parameters_(&x.type_params);
                    for c in x.constants.iter() {
                        self.annotations(&c.metadata);
                        if let Some(args) = &c.arguments {
                            self.arguments_(args, Pai::ConstanteDeEnum);
                        }
                    }
                }
                DeclKind::Extension(x) => self.type_parameters_(&x.type_params),
                DeclKind::ExtensionType(x) => {
                    self.type_parameters_(&x.type_params);
                    self.annotations(&x.representation_metadata);
                }
                DeclKind::Typedef(t) => {
                    self.type_parameters_(&t.type_params);
                    if let TypedefKind::Legacy { parameters, .. } = &t.kind {
                        self.parameters_(parameters);
                    }
                }
                DeclKind::Function(_) => {}
                DeclKind::Variables(l) => self.list(l),
            }
        }
        for m in a.members.iter() {
            self.annotations(&m.metadata);
            match &m.kind {
                MemberKind::Field(l) => self.list(l),
                MemberKind::Constructor(k) => self.parameters_(&k.parameters),
                MemberKind::Method(_) => {}
            }
        }
        for f in a.functions.iter() {
            self.type_parameters_(&f.type_params);
            if let Some(ps) = &f.parameters {
                self.parameters_(ps);
            }
        }
        for t in a.types.iter() {
            if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
                self.type_parameters_(type_params);
                self.parameters_(parameters);
            }
        }
        for (_, ms) in a.metadados_locais.iter() {
            self.annotations(ms);
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) => self.list(l),
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => self.list(l),
                StmtKind::ForIn { target: ForInTarget::Declared { metadata, .. }, .. } => self.annotations(metadata),
                _ => {}
            }
        }
    }
}
