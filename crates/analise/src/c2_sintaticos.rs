//! Verificações da família C que só dependem da árvore
//! (docs/ANALYZER-ESPECIFICACAO.md §C):
//!
//! * `rethrow_outside_catch` (`error_verifier.dart:5199-5207`);
//! * `return_in_generator` (o `messageGeneratorReturnsValue` do parser);
//! * `label_undefined`, `label_in_outer_scope`, `continue_label_invalid`
//!   (`ResolverVisitor._lookupBreakOrContinueTarget`, `resolver.dart:5314-5366`);
//! * `invalid_modifier_on_setter` (o `messageSetterNotSync` do parser);
//! * `uri_with_interpolation` (`library_analyzer.dart:719`, `:947`, `:995`);
//! * `main_is_not_function` (`error_verifier.dart:4125-4141`);
//! * família E: `break_outside_of_loop`, `continue_outside_of_loop`,
//!   `continue_without_label_in_case` (o `loopState` do parser do fasta);
//! * família B: `break_label_on_switch_member`, `duplicate_variable_pattern`,
//!   `duplicate_pattern_assignment_variable`, `duplicate_pattern_field`,
//!   `rest_element_in_map_pattern`, `empty_map_pattern`,
//!   `expected_one_list_pattern_type_arguments`,
//!   `expected_two_map_pattern_type_arguments`;
//! * família F: `unused_label` (`dead_code_verifier.dart:159-174`);
//! * `yield_in_non_generator` no comando (`YieldStatementResolver`,
//!   `yield_statement_resolver.dart:184-196`; o do parser fica no `yield`);
//! * família B: `unnecessary_final` (`best_practices_verifier.dart:828-835`) e
//!   `late_final_field_with_const_constructor` (`error_verifier.dart:4068-4091`).
//!
//! Escrito sem compilar nem executar (2026-10-04).

use crate::Unidade;
use dartforge_diagnostics::codigos::{compile_time_error as c, parser as pe, warning as w};
use dartforge_diagnostics::{Codigo, Diagnostic, Span};
use dartforge_frontend::ast::{
    self, AsyncModifier, CollectionElement, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, FunctionKind, MemberId,
    MemberKind, PatternId, PatternKind, StmtId, StmtKind,
};
use dartforge_intern::{Interner, SymbolId};

/// Um rótulo visível: o nome e se o alvo aceita `continue` (laço ou membro
/// de `switch`).
#[derive(Clone, Copy)]
struct Rotulo {
    nome: SymbolId,
    span: Span,
    aceita_continue: bool,
    /// O rótulo está num `case`/`default`.
    membro_de_switch: bool,
    usado: bool,
}

/// O `loopState` do parser do fasta: onde `break` e `continue` valem.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Laco {
    Fora,
    DentroDeSwitch,
    DentroDeLaco,
}

/// O contexto de um padrão de topo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Contexto {
    /// `var (a, b) = …`: todo identificador declara variável.
    Declaracao,
    /// `case`: só declara o que tem `var`, `final` ou tipo.
    Refutavel,
    /// `(a, b) = …`: os identificadores são variáveis existentes.
    Atribuicao,
}

struct Visita<'a> {
    a: &'a ast::Ast,
    fonte: &'a str,
    nomes: &'a Interner,
    saida: Vec<Diagnostic>,
    /// Os rótulos em escopo; `fronteira` marca onde começa a função
    /// corrente (o que está antes é de um método de fora).
    rotulos: Vec<Rotulo>,
    fronteira: usize,
    /// Quantos `catch` da função corrente envolvem o ponto.
    nivel_de_catch: u32,
    modificador: AsyncModifier,
    laco: Laco,
}

/// Os diagnósticos sintáticos de uma unidade.
pub fn verificar(u: Unidade<'_>, nomes: &Interner) -> Vec<Diagnostic> {
    let mut v = Visita {
        a: u.ast,
        fonte: u.fonte,
        nomes,
        saida: Vec::new(),
        rotulos: Vec::new(),
        fronteira: 0,
        nivel_de_catch: 0,
        modificador: AsyncModifier::None,
        laco: Laco::Fora,
    };
    // As URIs das diretivas não podem ter interpolação.
    for d in u.unit.directives.iter() {
        let uri = match &d.kind {
            DirectiveKind::Import { uri, .. } | DirectiveKind::Export { uri, .. } | DirectiveKind::Part { uri } => Some(uri),
            _ => None,
        };
        if let Some(uri) = uri {
            if uri.parts.iter().any(|p| matches!(p, ast::StringPart::Interpolation(_))) {
                v.relatar(c::URI_WITH_INTERPOLATION, uri.span, &[]);
            }
        }
    }
    let principal = nomes.lookup("main");
    for &d in u.unit.declarations.iter() {
        v.declaracao(d, principal);
    }
    v.saida
}

impl<'a> Visita<'a> {
    fn relatar(&mut self, codigo: Codigo, span: Span, args: &[&str]) {
        self.saida.push(Diagnostic::com_codigo(codigo, span, args.iter().copied()));
    }

    fn declaracao(&mut self, d: ast::DeclId, principal: Option<SymbolId>) {
        let a = self.a;
        // `main` de topo que não é função.
        let nao_funcao = |n: ast::Name| -> Option<Span> { (Some(n.sym) == principal).then_some(n.span) };
        match &a.decl(d).kind {
            DeclKind::Variables(l) => {
                for var in l.variables.iter() {
                    if let Some(sp) = nao_funcao(var.name) {
                        self.relatar(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                    }
                    if let Some(init) = var.initializer {
                        self.expr(init);
                    }
                }
            }
            DeclKind::Function(f) => {
                let func = a.function(*f);
                if matches!(func.kind, FunctionKind::Getter | FunctionKind::Setter) {
                    if let Some(sp) = func.name.and_then(nao_funcao) {
                        self.relatar(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                    }
                }
                self.funcao(*f);
            }
            DeclKind::Class(k) => {
                if let Some(sp) = nao_funcao(k.name) {
                    self.relatar(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                }
                let com_const = k.members.iter().any(|m| matches!(&a.member(*m).kind, MemberKind::Constructor(x) if x.const_ && !x.factory));
                self.membros(&k.members, com_const);
            }
            DeclKind::Mixin(k) => {
                if let Some(sp) = nao_funcao(k.name) {
                    self.relatar(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                }
                self.membros(&k.members, false);
            }
            DeclKind::Enum(k) => {
                if let Some(sp) = nao_funcao(k.name) {
                    self.relatar(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                }
                for cst in k.constants.iter() {
                    if let Some(args) = &cst.arguments {
                        for x in args.args.iter() {
                            self.expr(x.value);
                        }
                    }
                }
                // Os construtores de um enum são sempre `const`.
                self.membros(&k.members, true);
            }
            DeclKind::Extension(k) => self.membros(&k.members, false),
            DeclKind::ExtensionType(k) => self.membros(&k.members, false),
            DeclKind::Typedef(t) => {
                if let Some(sp) = nao_funcao(t.name) {
                    self.relatar(c::MAIN_IS_NOT_FUNCTION, sp, &[]);
                }
            }
        }
    }

    /// `com_const_gerador`: o tipo tem algum construtor gerador `const`.
    fn membros(&mut self, membros: &'a [MemberId], com_const_gerador: bool) {
        let a = self.a;
        for &mid in membros {
            match &a.member(mid).kind {
                MemberKind::Field(l) => {
                    // Campo de instância `late final` num tipo com construtor
                    // gerador `const`: na palavra `late`.
                    if com_const_gerador && l.late && l.final_ && !l.static_ {
                        let sp = a.member(mid).span;
                        if let Some(i) = self.fonte.get(sp.start..sp.end.min(self.fonte.len())).and_then(|t| primeira_palavra(t, "late")) {
                            self.relatar(c::LATE_FINAL_FIELD_WITH_CONST_CONSTRUCTOR, Span { start: sp.start + i, end: sp.start + i + 4 }, &[]);
                        }
                    }
                    for var in l.variables.iter() {
                        if let Some(init) = var.initializer {
                            self.expr(init);
                        }
                    }
                }
                MemberKind::Method(f) => self.funcao(*f),
                MemberKind::Constructor(k) => {
                    self.parametros(&k.parameters);
                    for init in k.initializers.iter() {
                        match init {
                            ast::Initializer::Field { value, .. } => self.expr(*value),
                            ast::Initializer::Assert { condition, message, .. } => {
                                self.expr(*condition);
                                if let Some(m) = message {
                                    self.expr(*m);
                                }
                            }
                            ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => {
                                for x in arguments.args.iter() {
                                    self.expr(x.value);
                                }
                            }
                        }
                    }
                    self.em_funcao(AsyncModifier::None, |v| v.corpo(&k.body, None));
                }
            }
        }
    }

    fn parametros(&mut self, ps: &'a [ast::Parameter]) {
        for p in ps {
            // `final this.x` / `final super.x`: o parâmetro já é final.
            if p.final_ && (p.this_ || p.super_) {
                let fim = p.name.map_or(p.span.end, |n| n.span.start);
                if let Some(i) = self.fonte.get(p.span.start..fim).and_then(|t| primeira_palavra(t, "final")) {
                    self.relatar(w::UNNECESSARY_FINAL, Span { start: p.span.start + i, end: p.span.start + i + 5 }, &[]);
                }
            }
            if let Some(fp) = &p.function_parameters {
                self.parametros(fp);
            }
            if let Some(d) = p.default_value {
                self.expr(d);
            }
        }
    }

    /// O corpo de uma função: rótulos e `catch` de fora não valem dentro.
    fn em_funcao(&mut self, modificador: AsyncModifier, f: impl FnOnce(&mut Self)) {
        let fronteira = std::mem::replace(&mut self.fronteira, self.rotulos.len());
        let nivel = std::mem::replace(&mut self.nivel_de_catch, 0);
        let antes = std::mem::replace(&mut self.modificador, modificador);
        let laco = std::mem::replace(&mut self.laco, Laco::Fora);
        f(self);
        self.laco = laco;
        self.modificador = antes;
        self.nivel_de_catch = nivel;
        // Cada comando rotulado já fechou os seus; o que sobrar sai aqui.
        self.rotulos.truncate(self.fronteira);
        self.fronteira = fronteira;
    }

    fn funcao(&mut self, f: ast::FunctionId) {
        let a = self.a;
        let func = a.function(f);
        if let Some(ps) = &func.parameters {
            self.parametros(ps);
        }
        // Setter com `async`, `async*` ou `sync*`: no primeiro token do
        // modificador (o `*` não entra).
        if func.kind == FunctionKind::Setter && func.modifier != AsyncModifier::None {
            let palavra = if func.modifier == AsyncModifier::SyncStar { "sync" } else { "async" };
            let fim = match &func.body {
                FunctionBody::Block(s) => a.stmt(*s).span.start,
                FunctionBody::Expression(e) => a.expr(*e).span.start,
                _ => func.span.end,
            };
            let inicio = func.name.map_or(func.span.start, |n| n.span.end);
            if let Some(i) = self.fonte.get(inicio..fim).and_then(|t| ultima_palavra(t, palavra)) {
                let ini = inicio + i;
                self.relatar(c::INVALID_MODIFIER_ON_SETTER, Span { start: ini, end: ini + palavra.len() }, &[]);
            }
        }
        let depois_dos_parametros = func.name.map_or(func.span.start, |n| n.span.end);
        self.em_funcao(func.modifier, |v| v.corpo(&func.body, Some(depois_dos_parametros)));
    }

    /// `a_partir`: de onde procurar o `=>` de um corpo de expressão.
    fn corpo(&mut self, b: &FunctionBody, a_partir: Option<usize>) {
        match b {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(e) => {
                // `f() sync* => 0;`: um gerador não devolve valor; na seta.
                if matches!(self.modificador, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                    let fim = self.a.expr(*e).span.start;
                    let inicio = a_partir.unwrap_or(0).min(fim);
                    if let Some(i) = self.fonte.get(inicio..fim).and_then(|t| t.rfind("=>")) {
                        let ini = inicio + i;
                        self.relatar(c::RETURN_IN_GENERATOR, Span { start: ini, end: ini + 2 }, &[]);
                    }
                }
                self.expr(*e);
            }
            _ => {}
        }
    }

    // -- Rótulos -------------------------------------------------------------------

    /// `break L` / `continue L`: o rótulo na função corrente, senão num
    /// método de fora, senão indefinido.
    fn salto(&mut self, rotulo: ast::Name, continuar: bool, comando: Span) {
        let texto = self.nomes.resolve(rotulo.sym).to_string();
        // O uso conta para o rótulo mais interno com o nome, mesmo de um
        // método de fora (`_LabelTracker.recordUsage`).
        if let Some(r) = self.rotulos.iter_mut().rev().find(|r| r.nome == rotulo.sym) {
            r.usado = true;
        }
        let local = self.rotulos[self.fronteira..].iter().rev().find(|r| r.nome == rotulo.sym).copied();
        match local {
            Some(r) => {
                if continuar && !r.aceita_continue {
                    self.relatar(c::CONTINUE_LABEL_INVALID, comando, &[]);
                }
                if !continuar && r.membro_de_switch {
                    self.relatar(c::BREAK_LABEL_ON_SWITCH_MEMBER, rotulo.span, &[]);
                }
            }
            None => {
                if self.rotulos[..self.fronteira].iter().any(|r| r.nome == rotulo.sym) {
                    self.relatar(c::LABEL_IN_OUTER_SCOPE, rotulo.span, &[&texto]);
                } else {
                    self.relatar(c::LABEL_UNDEFINED, rotulo.span, &[&texto]);
                }
            }
        }
    }

    /// Tira do escopo os rótulos a partir de `de`, relatando os não usados.
    fn fechar_rotulos(&mut self, de: usize) {
        let fechados: Vec<Rotulo> = self.rotulos.drain(de..).collect();
        for r in fechados {
            if !r.usado {
                let texto = self.nomes.resolve(r.nome).to_string();
                self.relatar(w::UNUSED_LABEL, r.span, &[&texto]);
            }
        }
    }

    /// O corpo de um laço: `break` e `continue` valem.
    fn corpo_de_laco(&mut self, s: StmtId) {
        let antes = std::mem::replace(&mut self.laco, Laco::DentroDeLaco);
        self.stmt(s);
        self.laco = antes;
    }

    // -- Comandos ------------------------------------------------------------------

    fn lista(&mut self, l: &'a ast::VariableList) {
        for var in l.variables.iter() {
            if let Some(init) = var.initializer {
                self.expr(init);
            }
        }
    }

    fn inicio_de_for(&mut self, init: &'a Option<ast::ForInit>) {
        match init {
            Some(ast::ForInit::Variables(l)) => self.lista(l),
            Some(ast::ForInit::Expression(e)) => self.expr(*e),
            Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                self.padrao_de_topo(*pattern, Contexto::Declaracao);
                self.expr(*value);
            }
            None => {}
        }
    }

    fn alvo_de_for_in(&mut self, alvo: &'a ast::ForInTarget) {
        match alvo {
            ast::ForInTarget::Declared { .. } => {}
            ast::ForInTarget::Pattern { pattern, .. } => self.padrao_de_topo(*pattern, Contexto::Declaracao),
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
            StmtKind::Variables(l) => self.lista(l),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.padrao_de_topo(*pattern, Contexto::Declaracao);
                self.expr(*value);
            }
            StmtKind::Function(f) => self.funcao(*f),
            StmtKind::Expression(e) => self.expr(*e),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition);
                if let Some(p) = case_pattern {
                    self.padrao_de_topo(*p, Contexto::Refutavel);
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
                self.corpo_de_laco(*body);
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                self.alvo_de_for_in(target);
                self.expr(*iterable);
                self.corpo_de_laco(*body);
            }
            StmtKind::While { condition, body } => {
                self.expr(*condition);
                self.corpo_de_laco(*body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.corpo_de_laco(*body);
                self.expr(*condition);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value);
                // Os rótulos dos membros valem no `switch` inteiro.
                let antes = self.rotulos.len();
                for caso in cases.iter() {
                    for l in caso.labels.iter() {
                        self.rotulos.push(Rotulo { nome: l.sym, span: l.span, aceita_continue: true, membro_de_switch: true, usado: false });
                    }
                }
                // Dentro de um laço o `switch` não muda o estado: o
                // `continue` sem rótulo é do laço.
                let laco = self.laco;
                if laco == Laco::Fora {
                    self.laco = Laco::DentroDeSwitch;
                }
                for caso in cases.iter() {
                    if let Some(p) = caso.pattern {
                        self.padrao_de_topo(p, Contexto::Refutavel);
                    }
                    if let Some(g) = caso.guard {
                        self.expr(g);
                    }
                    for x in caso.body.iter() {
                        self.stmt(*x);
                    }
                }
                self.laco = laco;
                self.fechar_rotulos(antes);
            }
            StmtKind::Break(rotulo) => {
                match rotulo {
                    Some(r) => self.salto(*r, false, no.span),
                    None => {
                        if self.laco == Laco::Fora {
                            self.relatar(pe::BREAK_OUTSIDE_OF_LOOP, Span { start: no.span.start, end: no.span.start + 5 }, &[]);
                        }
                    }
                }
            }
            StmtKind::Continue(rotulo) => {
                let palavra = Span { start: no.span.start, end: no.span.start + 8 };
                if self.laco == Laco::Fora {
                    self.relatar(pe::CONTINUE_OUTSIDE_OF_LOOP, palavra, &[]);
                } else if rotulo.is_none() && self.laco == Laco::DentroDeSwitch {
                    self.relatar(pe::CONTINUE_WITHOUT_LABEL_IN_CASE, palavra, &[]);
                }
                if let Some(r) = rotulo {
                    self.salto(*r, true, no.span);
                }
            }
            StmtKind::Return(Some(e)) => {
                // `return valor;` num gerador: no token `return`.
                if matches!(self.modificador, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                    self.relatar(c::RETURN_IN_GENERATOR, Span { start: no.span.start, end: no.span.start + 6 }, &[]);
                }
                self.expr(*e);
            }
            StmtKind::Yield { star, value } => {
                // `YieldStatementResolver._resolve_notGenerator`: no comando
                // inteiro, `YIELD_EACH_IN_NON_GENERATOR` com `*`.
                if !matches!(self.modificador, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                    let codigo = if *star { c::YIELD_EACH_IN_NON_GENERATOR } else { c::YIELD_IN_NON_GENERATOR };
                    self.relatar(codigo, no.span, &[]);
                }
                self.expr(*value)
            }
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(*body);
                for k in catches.iter() {
                    self.nivel_de_catch += 1;
                    self.stmt(k.body);
                    self.nivel_de_catch -= 1;
                }
                if let Some(f) = finally_ {
                    self.stmt(*f);
                }
            }
            StmtKind::Labeled { labels, body } => {
                let laco = matches!(
                    a.stmt(*body).kind,
                    StmtKind::For { .. } | StmtKind::ForIn { .. } | StmtKind::While { .. } | StmtKind::DoWhile { .. }
                );
                let antes = self.rotulos.len();
                for l in labels.iter() {
                    self.rotulos.push(Rotulo { nome: l.sym, span: l.span, aceita_continue: laco, membro_de_switch: false, usado: false });
                }
                self.stmt(*body);
                self.fechar_rotulos(antes);
            }
            StmtKind::Assert { condition, message } => {
                self.expr(*condition);
                if let Some(m) = message {
                    self.expr(*m);
                }
            }
            _ => {}
        }
    }

    // -- Padrões e expressões --------------------------------------------------------

    /// As variáveis que o padrão declara (ou atribui), com o nome de cada
    /// ocorrência; os dois lados de um `||` declaram as mesmas e contam uma
    /// vez. Uma repetição fora disso é relatada na segunda ocorrência.
    fn variaveis_do_padrao(&mut self, p: PatternId, ctx: Contexto, vistas: &mut Vec<SymbolId>) {
        let a = self.a;
        match &a.pattern(p).kind {
            PatternKind::Variable { final_, var_, ty, name } => {
                let declara = match ctx {
                    Contexto::Declaracao | Contexto::Atribuicao => true,
                    Contexto::Refutavel => *final_ || *var_ || ty.is_some(),
                };
                if !declara {
                    return;
                }
                if vistas.contains(&name.sym) {
                    let texto = self.nomes.resolve(name.sym).to_string();
                    let codigo = if ctx == Contexto::Atribuicao { c::DUPLICATE_PATTERN_ASSIGNMENT_VARIABLE } else { c::DUPLICATE_VARIABLE_PATTERN };
                    self.relatar(codigo, name.span, &[&texto]);
                } else {
                    vistas.push(name.sym);
                }
            }
            PatternKind::Or(x, y) => {
                let mut esquerda = vistas.clone();
                let mut direita = vistas.clone();
                self.variaveis_do_padrao(*x, ctx, &mut esquerda);
                self.variaveis_do_padrao(*y, ctx, &mut direita);
                for n in esquerda.into_iter().chain(direita) {
                    if !vistas.contains(&n) {
                        vistas.push(n);
                    }
                }
            }
            PatternKind::And(x, y) => {
                self.variaveis_do_padrao(*x, ctx, vistas);
                self.variaveis_do_padrao(*y, ctx, vistas);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => self.variaveis_do_padrao(*x, ctx, vistas),
            PatternKind::Cast { pattern, .. } => self.variaveis_do_padrao(*pattern, ctx, vistas),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => self.variaveis_do_padrao(*x, ctx, vistas),
                        _ => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    self.variaveis_do_padrao(en.value, ctx, vistas);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.variaveis_do_padrao(f.pattern, ctx, vistas);
                }
            }
            _ => {}
        }
    }

    /// Um padrão de topo: as variáveis repetidas e, depois, a forma de cada
    /// subpadrão.
    fn padrao_de_topo(&mut self, p: PatternId, ctx: Contexto) {
        let mut vistas = Vec::new();
        self.variaveis_do_padrao(p, ctx, &mut vistas);
        self.padrao(p);
    }

    /// A lista `<…>` de argumentos de tipo de um padrão.
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

    /// Campos repetidos de um padrão registro ou objeto: no nome do campo
    /// repetido (no campo inteiro, quando o nome é implícito, `:var x`).
    fn campos_repetidos(&mut self, campos: &'a [ast::PatternField]) {
        let a = self.a;
        let mut vistos: Vec<SymbolId> = Vec::new();
        for f in campos.iter() {
            let (nome, span) = match f.name {
                Some(n) => (Some(n.sym), n.span),
                None => {
                    // `:var x` (o campo começa em `:`): o nome é o da variável.
                    let implicito = self.fonte.as_bytes().get(f.span.start) == Some(&b':');
                    let mut q = f.pattern;
                    let nome = loop {
                        match &a.pattern(q).kind {
                            PatternKind::Variable { name, .. } => break Some(name.sym),
                            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => q = *x,
                            PatternKind::Cast { pattern, .. } => q = *pattern,
                            _ => break None,
                        }
                    };
                    (if implicito { nome } else { None }, f.span)
                }
            };
            let Some(nome) = nome else { continue };
            if vistos.contains(&nome) {
                let texto = self.nomes.resolve(nome).to_string();
                self.relatar(c::DUPLICATE_PATTERN_FIELD, span, &[&texto]);
            } else {
                vistos.push(nome);
            }
        }
    }

    fn padrao(&mut self, p: PatternId) {
        let a = self.a;
        let span_do_padrao = a.pattern(p).span;
        match &a.pattern(p).kind {
            PatternKind::Constant(e) => self.expr(*e),
            PatternKind::Relational { value, .. } => self.expr(*value),
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.padrao(*x);
                self.padrao(*y);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => self.padrao(*x),
            PatternKind::Cast { pattern, .. } => self.padrao(*pattern),
            PatternKind::List { type_args, elements } => {
                if type_args.len() > 1 {
                    if let Some(sp) = self.lista_de_tipos(type_args) {
                        let n = type_args.len().to_string();
                        self.relatar(c::EXPECTED_ONE_LIST_PATTERN_TYPE_ARGUMENTS, sp, &[&n]);
                    }
                }
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => self.padrao(*x),
                        _ => {}
                    }
                }
            }
            PatternKind::Map { type_args, entries, rest } => {
                if !type_args.is_empty() && type_args.len() != 2 {
                    if let Some(sp) = self.lista_de_tipos(type_args) {
                        let n = type_args.len().to_string();
                        self.relatar(c::EXPECTED_TWO_MAP_PATTERN_TYPE_ARGUMENTS, sp, &[&n]);
                    }
                }
                // Um padrão de mapa não tem resto: no `...`.
                if *rest {
                    let de = entries.last().map_or(span_do_padrao.start, |en| a.pattern(en.value).span.end);
                    if let Some(i) = self.fonte.get(de..span_do_padrao.end).and_then(|t| t.find("...")) {
                        self.relatar(c::REST_ELEMENT_IN_MAP_PATTERN, Span { start: de + i, end: de + i + 3 }, &[]);
                    }
                }
                for en in entries.iter() {
                    self.expr(en.key);
                    self.padrao(en.value);
                }
                if entries.is_empty() {
                    self.relatar(c::EMPTY_MAP_PATTERN, span_do_padrao, &[]);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                self.campos_repetidos(fields);
                for f in fields.iter() {
                    self.padrao(f.pattern);
                }
            }
            _ => {}
        }
    }

    fn elemento(&mut self, el: &'a CollectionElement) {
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
                    self.padrao_de_topo(*p, Contexto::Refutavel);
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
                self.inicio_de_for(init);
                if let Some(x) = condition {
                    self.expr(*x);
                }
                for x in updates.iter() {
                    self.expr(*x);
                }
                self.elemento(body);
            }
            CollectionElement::ForIn { target, iterable, body, .. } => {
                self.alvo_de_for_in(target);
                self.expr(*iterable);
                self.elemento(body);
            }
        }
    }

    fn expr(&mut self, e: ExprId) {
        let a = self.a;
        let no = a.expr(e);
        match &no.kind {
            ExprKind::Rethrow => {
                if self.nivel_de_catch == 0 {
                    self.relatar(c::RETHROW_OUTSIDE_CATCH, no.span, &[]);
                }
            }
            ExprKind::FunctionExpression(f) => self.funcao(*f),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.elemento(el);
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
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x),
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
                self.padrao_de_topo(*pattern, Contexto::Atribuicao);
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
                    self.padrao_de_topo(caso.pattern, Contexto::Refutavel);
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

/// A primeira ocorrência de `palavra` em `texto` como palavra inteira.
fn primeira_palavra(texto: &str, palavra: &str) -> Option<usize> {
    let b = texto.as_bytes();
    let de_palavra = |x: u8| x.is_ascii_alphanumeric() || x == b'_';
    let mut de = 0;
    while let Some(i) = texto.get(de..).and_then(|t| t.find(palavra)) {
        let ini = de + i;
        let depois = ini + palavra.len();
        if (ini == 0 || !de_palavra(b[ini - 1])) && (depois >= b.len() || !de_palavra(b[depois])) {
            return Some(ini);
        }
        de = depois;
    }
    None
}

/// A última ocorrência de `palavra` em `texto` como palavra inteira.
fn ultima_palavra(texto: &str, palavra: &str) -> Option<usize> {
    let b = texto.as_bytes();
    let de_palavra = |x: u8| x.is_ascii_alphanumeric() || x == b'_';
    let mut fim = texto.len();
    while let Some(i) = texto[..fim].rfind(palavra) {
        let depois = i + palavra.len();
        if (i == 0 || !de_palavra(b[i - 1])) && (depois >= b.len() || !de_palavra(b[depois])) {
            return Some(i);
        }
        if i == 0 {
            break;
        }
        fim = i;
        while fim > 0 && !texto.is_char_boundary(fim) {
            fim -= 1;
        }
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;

    fn codigos(fonte: &str) -> Vec<(&'static str, usize, usize)> {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte };
        verificar(u, &nomes).into_iter().map(|d| (d.code.map_or("", |c| c.info().nome), d.span.start, d.span.end)).collect()
    }

    #[test]
    fn rethrow_fora_e_dentro_de_catch() {
        // void f() { rethrow; }
        assert_eq!(codigos("void f() { rethrow; }"), vec![("rethrow_outside_catch", 11, 18)]);
        assert!(codigos("void f() { try {} catch (e) { rethrow; } }").is_empty());
        // A função literal dentro do `catch` recomeça a contagem.
        let r = codigos("void f() { try {} catch (e) { () { rethrow; }; } }");
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn retorno_com_valor_em_gerador() {
        // f() async* { return 0; }
        assert_eq!(codigos("f() async* { return 0; }"), vec![("return_in_generator", 13, 19)]);
        // f() sync* => 0;
        assert_eq!(codigos("f() sync* => 0;"), vec![("return_in_generator", 10, 12)]);
        assert!(codigos("f() async* { return; }").is_empty());
    }

    #[test]
    fn rotulos() {
        // void f() { break L; }
        assert_eq!(codigos("void f() { break L; }"), vec![("label_undefined", 17, 18)]);
        let r = codigos("void f() { L: while (true) { () { break L; }; } }");
        assert_eq!(r[0].0, "label_in_outer_scope");
        // Fora de laço saem os dois: o do parser e o da resolução.
        let r = codigos("void f() { L: { continue L; } }");
        let nomes: Vec<&str> = r.iter().map(|x| x.0).collect();
        assert_eq!(nomes, vec!["continue_outside_of_loop", "continue_label_invalid"]);
        assert!(codigos("void f() { L: while (true) { continue L; } }").is_empty());
    }

    #[test]
    fn saltos_fora_de_laco() {
        // void f() { break; }
        assert_eq!(codigos("void f() { break; }"), vec![("break_outside_of_loop", 11, 16)]);
        assert_eq!(codigos("void f() { continue; }"), vec![("continue_outside_of_loop", 11, 19)]);
        let r = codigos("void f(x) { switch (x) { case 1: continue; } }");
        assert_eq!(r[0].0, "continue_without_label_in_case");
        assert!(codigos("void f(x) { while (x) { switch (x) { case 1: continue; } } }").is_empty());
        assert!(codigos("void f(x) { switch (x) { case 1: break; } }").is_empty());
    }

    #[test]
    fn rotulo_nao_usado_e_de_membro() {
        // void f() { L: while (true) {} }
        assert_eq!(codigos("void f() { L: while (true) {} }"), vec![("unused_label", 11, 12)]);
        let r = codigos("void f(x) { switch (x) { L: case 1: break L; } }");
        assert_eq!(r[0].0, "break_label_on_switch_member");
    }

    #[test]
    fn forma_de_padroes() {
        let r = codigos("void f(x) { var (a, a) = x; }");
        assert_eq!(r[0].0, "duplicate_variable_pattern");
        let r = codigos("void f(x) { var a, b; (a, a) = x; }");
        assert_eq!(r[0].0, "duplicate_pattern_assignment_variable");
        let r = codigos("void f(x) { if (x case {}) {} }");
        assert_eq!(r[0].0, "empty_map_pattern");
        // Os dois lados de `||` declaram a mesma variável: não é repetição.
        assert!(codigos("void f(x) { if (x case int a || int a) {} }").is_empty());
    }

    #[test]
    fn final_desnecessario_e_late_final() {
        // class A { int x; A(final this.x); }
        let r = codigos("class A { int x; A(final this.x); }");
        assert_eq!(r, vec![("unnecessary_final", 19, 24)]);
        // class A { late final int x; const A(); }
        let r = codigos("class A { late final int x; const A(); }");
        assert_eq!(r, vec![("late_final_field_with_const_constructor", 10, 14)]);
        assert!(codigos("class A { late final int x; A(); }").is_empty());
    }

    #[test]
    fn main_que_nao_e_funcao() {
        // var main = 1;
        assert_eq!(codigos("var main = 1;"), vec![("main_is_not_function", 4, 8)]);
        assert!(codigos("void main() {}").is_empty());
    }
}
