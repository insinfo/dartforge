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
    StmtId, StmtKind, StringPart,
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
    /// A exceção de `on T catch (e)` sem pilha (`UNUSED_CATCH_CLAUSE`).
    CatchExcecao,
    /// A pilha de `catch (e, s)` (`UNUSED_CATCH_STACK`).
    CatchPilha,
}

struct Local {
    nome: SymbolId,
    span: Span,
    especie: Especie,
    lido: bool,
    /// Declaração por padrão a que pertence (todas ou nenhuma).
    grupo: Option<usize>,
    /// Mesma variável que outra (padrões compartilhados).
    alias: Option<usize>,
    /// Lida pela guarda do próprio caso (antes da junção): só ela, não a
    /// junção (`JoinPatternVariableElement.transitiveVariables` só no corpo).
    lido_proprio: bool,
}

struct Visita<'a> {
    ast: &'a Ast,
    interner: &'a Interner,
    curinga: bool,
    locais: Vec<Local>,
    escopos: Vec<Vec<usize>>,
    fonte: &'a str,
    grupos: usize,
    /// Dentro de um padrão refutável.
    refutavel: bool,
    /// Início (índice em `locais`) de uma região de padrões compartilhados.
    regiao_compartilhada: Option<usize>,
    /// As variáveis do padrão do caso cuja guarda está sendo visitada.
    da_guarda: Vec<usize>,
    /// A função local cujo corpo está sendo visitado (o `_enclosingExec`
    /// do `GatherUsedLocalElementsVisitor`, que só muda numa
    /// `FunctionDeclaration`): a referência a ela de dentro dela não a usa.
    executavel: Option<usize>,
    /// Início (índice em `locais`) das variáveis do padrão sendo declarado.
    inicio_padrao: Option<usize>,
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
        self.locais.push(Local { nome: n.sym, span: n.span, especie, lido: false, grupo, alias: None, lido_proprio: false });
        if let Some(e) = self.escopos.last_mut() {
            // `for (int i = 0, i = 0; …)`, `var a; var a;`: a redeclaração
            // (`duplicate_definition`) não entra no escopo — os usos são da
            // primeira, e a segunda, nunca lida, é relatada.
            if especie != Especie::Oculta && e.iter().any(|&j| self.locais[j].nome == n.sym) {
                return;
            }
            e.push(i);
        }
    }

    fn achar(&self, nome: SymbolId) -> Option<usize> {
        self.escopos.iter().rev().flat_map(|e| e.iter().rev()).copied().find(|&i| self.locais[i].nome == nome)
    }

    /// Referência ao identificador `n`: `leitura` diz se lê o valor. Função
    /// local conta como usada por qualquer referência.
    fn raiz(&self, mut i: usize) -> usize {
        while let Some(a) = self.locais[i].alias {
            i = a;
        }
        i
    }

    fn referir(&mut self, n: ast::Name, leitura: bool) {
        // Na guarda, o nome é a variável do próprio caso.
        if let Some(&i) = self.da_guarda.iter().rev().find(|&&i| self.locais[i].nome == n.sym) {
            if leitura {
                self.locais[i].lido_proprio = true;
            }
            return;
        }
        if let Some(i) = self.achar(n.sym).map(|i| self.raiz(i)) {
            // `identical(element, _enclosingExec)`
            // (`unused_local_elements_verifier.dart:383-385`).
            if Some(i) == self.executavel {
                return;
            }
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
                // `_isReadIdentifier`: o identificador de **qualquer**
                // `PrefixExpression`/`PostfixExpression` (`-x`, `~x`, `!x`,
                // `x!`, `++x`, `x--`) que é a instrução inteira não é leitura.
                let _ = op;
                if let ExprKind::Identifier(n) = &ast.expr(*operand).kind {
                    self.referir(*n, !comando);
                    return;
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
                    self.padrao_de_caso(c.pattern);
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
                    self.padrao_de_caso(*p);
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
                    self.expr(*u);
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

    /// Padrão de `case`, `if-case` ou caso de `switch` expressão: refutável.
    fn padrao_de_caso(&mut self, p: PatternId) {
        let antes = self.refutavel;
        self.refutavel = true;
        self.padrao_declarado(p, None);
        self.refutavel = antes;
    }

    /// Padrão que declara variáveis (`var (a, b) = e`, `case`, `if-case`):
    /// primeiro as variáveis, depois as expressões constantes — `== b && var b`
    /// já se refere ao `b` do próprio padrão (`referenced_before_declaration`).
    fn padrao_declarado(&mut self, p: PatternId, grupo: Option<usize>) {
        let antes = self.inicio_padrao.replace(self.locais.len());
        self.declarar_do_padrao(p, grupo);
        self.inicio_padrao = antes;
        self.ler_do_padrao(p);
    }

    /// Declara uma variável de padrão; numa região compartilhada (os dois
    /// lados de `||`, os `case` que dividem um corpo), o mesmo nome já
    /// declarado nela é a **mesma** variável (`JoinPatternVariableElement`).
    fn declarar_de_padrao(&mut self, n: ast::Name, grupo: Option<usize>) {
        if let Some(regiao) = self.regiao_compartilhada {
            if let Some(i) = self.achar(n.sym).filter(|&i| i >= regiao) {
                let raiz = self.raiz(i);
                if self.curinga && self.interner.resolve(n.sym) == "_" {
                    return;
                }
                self.locais.push(Local { nome: n.sym, span: n.span, especie: Especie::Variavel, lido: false, grupo, alias: Some(raiz), lido_proprio: false });
                return;
            }
        }
        // `var a && var a` (`duplicate_variable_pattern`): a segunda não
        // entra no escopo; os usos de `a` são da primeira.
        if let Some(inicio) = self.inicio_padrao {
            if self.achar(n.sym).is_some_and(|i| i >= inicio) {
                return;
            }
        }
        self.declarar(n, Especie::Variavel, grupo);
    }

    fn declarar_do_padrao(&mut self, p: PatternId, grupo: Option<usize>) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, final_, var_, ty } => {
                // Num padrão refutável, o identificador solto é constante.
                if !(self.refutavel && !*final_ && !*var_ && ty.is_none()) {
                    self.declarar_de_padrao(*name, grupo);
                }
            }
            PatternKind::Or(a, b) => {
                let antes = self.regiao_compartilhada;
                self.regiao_compartilhada = Some(antes.unwrap_or(self.locais.len()));
                self.declarar_do_padrao(*a, grupo);
                self.declarar_do_padrao(*b, grupo);
                self.regiao_compartilhada = antes;
            }
            PatternKind::And(a, b) => {
                self.declarar_do_padrao(*a, grupo);
                self.declarar_do_padrao(*b, grupo);
            }
            PatternKind::NullCheck(a) | PatternKind::NullAssert(a) | PatternKind::Parenthesized(a) => {
                self.declarar_do_padrao(*a, grupo)
            }
            PatternKind::Cast { pattern, .. } => self.declarar_do_padrao(*pattern, grupo),
            PatternKind::List { elements, .. } => {
                for e in elements.iter() {
                    if let ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) = e {
                        self.declarar_do_padrao(*x, grupo);
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for e in entries.iter() {
                    self.declarar_do_padrao(e.value, grupo);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.declarar_do_padrao(f.pattern, grupo);
                }
            }
            PatternKind::Wildcard { .. } | PatternKind::Constant(_) | PatternKind::Relational { .. } => {}
        }
    }

    fn ler_do_padrao(&mut self, p: PatternId) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, final_, var_, ty } => {
                if self.refutavel && !*final_ && !*var_ && ty.is_none() {
                    self.referir(*name, true)
                }
            }
            PatternKind::Constant(x) | PatternKind::Relational { value: x, .. } => self.expr(*x),
            PatternKind::Or(a, b) | PatternKind::And(a, b) => {
                self.ler_do_padrao(*a);
                self.ler_do_padrao(*b);
            }
            PatternKind::NullCheck(a) | PatternKind::NullAssert(a) | PatternKind::Parenthesized(a) => {
                self.ler_do_padrao(*a)
            }
            PatternKind::Cast { pattern, .. } => self.ler_do_padrao(*pattern),
            PatternKind::List { elements, .. } => {
                for e in elements.iter() {
                    if let ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) = e {
                        self.ler_do_padrao(*x);
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for e in entries.iter() {
                    self.expr(e.key);
                    self.ler_do_padrao(e.value);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.ler_do_padrao(f.pattern);
                }
            }
            PatternKind::Wildcard { .. } => {}
        }
    }
    /// Padrão de atribuição: as variáveis são escritas, não declaradas; com
    /// `var`, `final` ou tipo, o `DeclaredVariablePattern` declara o local no
    /// escopo do bloco (`ResolutionVisitor.visitDeclaredVariablePattern`).
    fn padrao_de_atribuicao(&mut self, p: PatternId) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, var_, final_, ty } if *var_ || *final_ || ty.is_some() => self.declarar_de_padrao(*name, None),
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
            Some(ForInit::Expression(x)) => self.expr(*x),
            // Como a declaração por padrão: o grupo só é relatado se
            // nenhuma variável dele for lida.
            Some(ForInit::Pattern { pattern, value, .. }) => {
                self.expr(*value);
                let g = self.novo_grupo();
                self.padrao_declarado(*pattern, Some(g));
            }
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

    /// `@nome` no texto `[de, ate)`: leitura do local `nome`, se houver.
    fn anotacoes_no_texto(&mut self, de: usize, ate: usize) {
        let Some(trecho) = self.fonte.get(de..ate) else { return };
        let b = trecho.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'@' {
                let ini = i + 1;
                let mut j = ini;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'$') {
                    j += 1;
                }
                if j > ini {
                    if let Some(sym) = self.interner.lookup(&trecho[ini..j]) {
                        let span = Span { start: de + ini, end: de + j };
                        self.referir(ast::Name { sym, span }, true);
                    }
                }
                i = j.max(i + 1);
            } else {
                i += 1;
            }
        }
    }

    fn bloco(&mut self, lista: &[StmtId], inicio: usize) {
        self.entrar();
        self.comandos(lista, inicio);
        self.sair();
    }

    /// Uma lista de comandos no escopo corrente. O escopo de uma declaração
    /// local é o bloco inteiro (uma referência antes dela já é a ela, e é o
    /// erro `referenced_before_declaration`): as declarações são içadas.
    fn comandos(&mut self, lista: &[StmtId], inicio_da_lista: usize) {
        let ast = self.ast;
        for &s in lista {
            match &ast.stmt(s).kind {
                StmtKind::Variables(vl) => {
                    for v in vl.variables.iter() {
                        self.declarar(v.name, Especie::Variavel, None);
                    }
                }
                StmtKind::Function(f) => {
                    if let Some(n) = ast.function(*f).name {
                        self.declarar(n, Especie::Funcao, None);
                    }
                }
                StmtKind::PatternVariables { pattern, .. } => {
                    let g = self.novo_grupo();
                    self.padrao_declarado(*pattern, Some(g));
                }
                _ => {}
            }
        }
        let mut fim_anterior = Some(inicio_da_lista);
        for &s in lista {
            // Anotações de declaração local (`@a var b;`) leem o que nomeiam, e
            // o parser não as guarda: procuradas no texto entre o comando
            // anterior e este.
            // Até o nome declarado: a anotação pode estar dentro do intervalo do comando.
            let nome = match &ast.stmt(s).kind {
                StmtKind::Variables(vl) => vl.variables.first().map(|v| v.name.span.start),
                StmtKind::Function(f) => ast.function(*f).name.map(|n| n.span.start),
                _ => None,
            };
            if let (Some(de), Some(ate)) = (fim_anterior, nome) {
                self.anotacoes_no_texto(de, ate);
            }
            fim_anterior = Some(ast.stmt(s).span.end);
            match &ast.stmt(s).kind {
                StmtKind::Variables(vl) => {
                    for v in vl.variables.iter() {
                        if let Some(i) = v.initializer {
                            self.expr(i);
                        }
                    }
                }
                StmtKind::Function(f) => self.funcao_local(*f),
                StmtKind::PatternVariables { value, .. } => self.expr(*value),
                _ => self.stmt(s),
            }
        }
    }

    /// O corpo de uma função local, com ela como `_enclosingExec`.
    fn funcao_local(&mut self, f: ast::FunctionId) {
        let i = self.ast.function(f).name.and_then(|n| self.achar(n.sym));
        let salvo = std::mem::replace(&mut self.executavel, i);
        self.funcao(f, false);
        self.executavel = salvo;
    }

    fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        match &ast.stmt(s).kind {
            StmtKind::Block(l) => self.bloco(l, ast.stmt(s).span.start),
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
                self.funcao_local(*f);
            }
            StmtKind::Expression(x) => self.expr_ctx(*x, true),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                self.entrar();
                if let Some(p) = case_pattern {
                    self.padrao_de_caso(*p);
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
                    self.expr(*u);
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
                // `case A: case B: corpo` — os casos sem corpo dividem o do
                // seguinte, e as variáveis de mesmo nome são uma só.
                let mut i = 0;
                while i < cases.len() {
                    let mut j = i;
                    while j + 1 < cases.len() && cases[j].body.is_empty() {
                        j += 1;
                    }
                    self.entrar();
                    let antes = self.regiao_compartilhada;
                    self.regiao_compartilhada = Some(self.locais.len());
                    for c in &cases[i..=j] {
                        let primeiro = self.locais.len();
                        if let Some(p) = c.pattern {
                            self.padrao_de_caso(p);
                        }
                        if let Some(g) = c.guard {
                            let guarda = std::mem::replace(&mut self.da_guarda, (primeiro..self.locais.len()).collect());
                            self.expr(g);
                            self.da_guarda = guarda;
                        }
                    }
                    self.regiao_compartilhada = antes;
                    self.comandos(&cases[j].body, cases[j].span.start);
                    self.sair();
                    i = j + 1;
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
                    // `visitCatchClause`: a exceção conta como usada quando há
                    // pilha ou não há `on`.
                    if let Some(n) = c.exception {
                        let usada = c.stack_trace.is_some() || c.on_type.is_none();
                        self.declarar(n, if usada { Especie::Oculta } else { Especie::CatchExcecao }, None);
                    }
                    if let Some(n) = c.stack_trace {
                        self.declarar(n, Especie::CatchPilha, None);
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
            // `g(@a x)`: a anotação lê o local `a` (antes dos parâmetros
            // entrarem no escopo).
            for a in p.metadata.iter() {
                if let Some(n) = a.name.first() {
                    self.referir(*n, true);
                }
                if let Some(args) = &a.arguments {
                    self.argumentos(args);
                }
            }
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

/// Intervalos das declarações executáveis da unidade: função ou variável
/// de topo, e cada membro de classe, mixin, extensão, tipo de extensão e
/// enum.
fn executaveis(u: Unidade<'_>) -> Vec<Span> {
    let ast = u.ast;
    let mut out = Vec::new();
    for &d in &u.unit.declarations {
        let decl = ast.decl(d);
        let membros: &[ast::MemberId] = match &decl.kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            DeclKind::Extension(x) => &x.members,
            DeclKind::ExtensionType(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Function(_) | DeclKind::Variables(_) => {
                out.push(decl.span);
                continue;
            }
            DeclKind::Typedef(_) => continue,
        };
        out.extend(membros.iter().map(|m| ast.member(*m).span));
    }
    out
}

/// Locais não usados de uma unidade.
///
/// `erros_sintaticos`: os diagnósticos do parser na unidade. Numa
/// declaração executável com erro de sintaxe, a recuperação do fasta guarda
/// trechos que a nossa descarta (e o contrário); uma leitura que o analyzer
/// vê e nós não viraria falso positivo. Um local declarado antes de um erro
/// de sintaxe da mesma declaração executável não se relata — pelo lado
/// seguro, como os imports.
/// Como [`nao_usados`], com a regra do T5 no lugar da porta por erro de
/// sintaxe: um local só fica de fora quando o nome dele é citado num trecho
/// que o parser pulou dentro da mesma declaração executável (um uso que o
/// fasta pode ter mantido).
pub fn nao_usados_com_pulados(u: Unidade<'_>, interner: &Interner, curinga: bool, pulados: crate::Pulados<'_>) -> Vec<Diagnostic> {
    let mut out = nao_usados_sem_filtro(u, interner, curinga);
    if !pulados.vazio() {
        let execs = executaveis(u);
        out.retain(|d| {
            let nome = u.fonte.get(d.span.start..d.span.end).unwrap_or("");
            !execs.iter().any(|e| d.span.start >= e.start && d.span.end <= e.end && pulados.cita_em(nome, *e))
        });
    }
    out
}

/// Os locais não usados da unidade, sem a regra dos trechos pulados (uma
/// unidade que o parser leu inteira).
pub fn nao_usados(u: Unidade<'_>, interner: &Interner, curinga: bool) -> Vec<Diagnostic> {
    nao_usados_sem_filtro(u, interner, curinga)
}

fn nao_usados_sem_filtro(u: Unidade<'_>, interner: &Interner, curinga: bool) -> Vec<Diagnostic> {
    let ast = u.ast;
    let mut v = Visita { ast, interner, curinga, fonte: u.fonte, locais: Vec::new(), escopos: vec![Vec::new()], grupos: 0, refutavel: false, regiao_compartilhada: None, inicio_padrao: None, da_guarda: Vec::new(), executavel: None };
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
    // Lida é a variável cuja raiz (a declaração que ela compartilha) foi lida.
    let lidos: Vec<bool> = (0..v.locais.len()).map(|i| v.locais[v.raiz(i)].lido || v.locais[i].lido_proprio).collect();
    let mut grupos_lidos = std::collections::HashSet::new();
    for (i, l) in v.locais.iter().enumerate() {
        if let (Some(g), true) = (l.grupo, lidos[i]) {
            grupos_lidos.insert(g);
        }
    }
    let mut out = Vec::new();
    for (i, l) in v.locais.iter().enumerate() {
        let nome = interner.resolve(l.nome);
        // `_isNamedWildcard`: só `_`s (com o recurso, só `_`, que nem declara).
        // A função local só fica de fora com o recurso
        // (`_visitFunctionElement`, `unused_local_elements_verifier.dart:1033-1040`).
        let so_sublinhados = match l.especie {
            Especie::Funcao => curinga && nome == "_",
            _ => nome.bytes().all(|b| b == b'_') && !(curinga && nome.len() > 1),
        };
        if lidos[i] || so_sublinhados || l.grupo.is_some_and(|g| grupos_lidos.contains(&g)) {
            continue;
        }
        match l.especie {
            Especie::Variavel => out.push(Diagnostic::com_codigo(w::UNUSED_LOCAL_VARIABLE, l.span, [nome])),
            Especie::Funcao => out.push(Diagnostic::com_codigo(w::UNUSED_ELEMENT, l.span, [nome])),
            Especie::CatchExcecao => out.push(Diagnostic::com_codigo(w::UNUSED_CATCH_CLAUSE, l.span, [nome])),
            Especie::CatchPilha => out.push(Diagnostic::com_codigo(w::UNUSED_CATCH_STACK, l.span, [nome])),
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
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
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
    fn padroes_compartilhados_e_atualizacao_de_for() {
        let f = "void f(Object? x) {\n  if (x case [var a || var a]) { print(a); }\n  switch (x) {\n    case (0, int y):\n    case (1, final int y):\n      print(y);\n  }\n  if (x case == b && var b) {}\n  for (int i = 0; i < 3; i++) {}\n}\n";
        let r = rodar(f);
        assert_eq!(r, vec![], "{r:?}");
    }

    #[test]
    fn caso_refutavel_referencia_e_ancora_e_icamento() {
        // `case a` lê a constante `a`; `@a` anota e lê; `v;` antes de
        // `var v` já é a declaração interna.
        let f = "void f(x) {\n  var a = 0;\n  if (x case a) {}\n  const c = 0;\n  @c\n  var b = 1;\n  print(b);\n  var v = 1;\n  {\n    v;\n    var v = 2;\n  }\n}\n";
        let r = rodar(f);
        assert_eq!(r, vec![("unused_local_variable".to_string(), "v".to_string())], "{r:?}");
    }

    #[test]
    fn sombra_e_closure() {
        let f = "void f() {\n  var a = 1;\n  void g(int a) { print(a); }\n  g(0);\n}\n";
        let r = rodar(f);
        assert_eq!(r, vec![("unused_local_variable".to_string(), "a".to_string())]);
    }

    #[test]
    fn redeclaracao_no_mesmo_escopo() {
        // O `i` lido na condição é o primeiro; o segundo é relatado.
        let f = "void f() {\n  for (int i = 0, i = 0; i < 5;) {}\n}\n";
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(f, &mut interner);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: f };
        let r: Vec<usize> = nao_usados(u, &interner, false).iter().map(|d| d.span.start).collect();
        assert_eq!(r, vec![f.find(", i").unwrap() + 2]);
    }

    #[test]
    fn trecho_pulado_que_cita_o_nome_nao_relata() {
        let f = "void f() {\n  var a = 1;\n  // a b c\n}\nvoid g() {\n  var b = 1;\n}\n";
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(f, &mut interner);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: f };
        // Um trecho pulado no corpo de `f` que cita `a`: `a` não se relata
        // (o fasta teria lido a leitura); `b`, de `g`, sim.
        let trecho = Span { start: f.find("a b c").unwrap(), end: f.find("a b c").unwrap() + 5 };
        let pulados = crate::Pulados { fonte: f, trechos: &[trecho] };
        let r: Vec<usize> = nao_usados_com_pulados(u, &interner, false, pulados).iter().map(|d| d.span.start).collect();
        assert_eq!(r, vec![f.find("b = 1;\n}\n").unwrap()]);
        // O trecho só vale dentro da declaração executável que ele toca.
        let fora = Span { start: f.find("void g").unwrap(), end: f.find("void g").unwrap() + 4 };
        let pulados = crate::Pulados { fonte: f, trechos: &[fora] };
        assert_eq!(nao_usados_com_pulados(u, &interner, false, pulados).len(), 2);
    }

    #[test]
    fn padrao_duplicado_e_anotacao_de_parametro() {
        // `var a && var a`: o uso é da primeira `a` (a segunda nem entra no
        // escopo); `g(@a x)` lê `a`.
        let f = "void f(int x) {\n  if (x case var a && var a) {\n    a;\n  }\n  const b = 0;\n  g(@b y) {}\n  g(0);\n}\n";
        let r = rodar(f);
        assert_eq!(r, vec![], "{r:?}");
    }

    /// `UnusedLocalElementsVerifier.visitCatchClause`: a exceção só é
    /// relatada em `on T catch (e)` sem pilha; a pilha, sempre que não lida.
    #[test]
    fn variaveis_de_catch() {
        let f = "void f() {\n  try {} on Exception catch (e) {}\n  try {} catch (e) {}\n  try {} on Exception catch (e, s) {}\n  try {} catch (e, t) { print(t); }\n  try {} on Error catch (_) {}\n}\n";
        let r = rodar(f);
        assert_eq!(
            r,
            vec![("unused_catch_clause".to_string(), "e".to_string()), ("unused_catch_stack".to_string(), "s".to_string())]
        );
    }
}
