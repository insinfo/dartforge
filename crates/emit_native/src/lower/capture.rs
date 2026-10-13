//! Análise de captura (P1): quais variáveis locais uma closure ou função
//! local enxerga de fora, e quais delas moram numa célula.
//!
//! A especificação (§17.11 "Function Expressions", §10 "Variables") diz que a
//! closure captura a **variável**, não o valor: uma gravação feita depois da
//! criação é vista por ela, e uma gravação dela é vista de fora. O `dartdevc`
//! e o `dart2js` rebaixam isso em duas classes, como aqui:
//!
//! * capturada e **nunca atribuída** depois da declaração (o caso comum:
//!   parâmetro, `final`, `var` só inicializada): o valor é copiado para o
//!   ambiente na criação da closure — não há o que observar;
//! * capturada e atribuída em qualquer ponto (fora ou dentro de alguma
//!   closure, antes ou depois da captura): a variável mora numa `Cell` do heap
//!   desde a declaração, e tanto a função quanto as closures passam por ela.
//!   Uma função local conta como atribuída (o nome é ligado depois de a
//!   closure existir — é assim que ela chama a si mesma).
//!
//! A análise é por **função baixada** (a de topo e cada closure, na hora de
//! baixá-la): ela olha o corpo inteiro, com as closures aninhadas, e decide as
//! células das variáveis declaradas **naquela** função. As variáveis livres de
//! uma closure são calculadas na criação, contra os locais visíveis.
//!
//! Uma referência é local quando a resolução diz `Local`/`Parameter` ou não
//! diz nada (os corpos de closure ainda não são inferidos: tudo neles é
//! `dynamic` e sem resolução); um identificador que resolve para membro ou
//! elemento de topo nunca é captura. Os escopos seguem os da especificação:
//! bloco, `for` (a variável do laço), `catch`, `case`, parâmetros.

use crate::context::Context;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{
    self, CollectionElement, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody, FunctionId,
    ListPatternElement, PatternId, PatternKind, StmtId, StmtKind, UnaryOp,
};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::Resolved;
use std::collections::{HashMap, HashSet};

/// Uma declaração vista pelo percurso: o offset do nome e a profundidade de
/// função (0 = a função analisada).
#[derive(Clone, Copy)]
struct Decl {
    offset: usize,
    prof: u32,
}

/// Percurso único do corpo de uma função, com a pilha de escopos.
struct Walk<'x, 'a> {
    ctx: &'x Context<'a>,
    unit: UnitId,
    ast: &'x ast::Ast,
    scopes: Vec<HashMap<SymbolId, Decl>>,
    prof: u32,
    /// Declarações da profundidade 0 lidas ou gravadas de dentro de uma
    /// função aninhada.
    captured: HashSet<usize>,
    /// Declarações da profundidade 0 gravadas em algum ponto (ou funções
    /// locais).
    assigned: HashSet<usize>,
    /// Nomes lidos ou gravados sem declaração no percurso (as variáveis
    /// livres, na ordem da primeira referência).
    free_set: Vec<SymbolId>,
    /// Nomes sem declaração no percurso que ele grava.
    stored_free: HashSet<SymbolId>,
    uses_this: bool,
    // --- funções locais diretas (`direct_functions.rs`) ---
    /// As funções abertas, da mais externa à corrente: a função local
    /// nomeada (`Some`) ou a expressão de função (`None`). O tamanho é `prof`.
    stack: Vec<Option<FunctionId>>,
    /// Nome (offset) de cada função local declarada no percurso → a função.
    local_functions: HashMap<usize, FunctionId>,
    /// Cada referência a uma função local: a função, se ela é o alvo de uma
    /// chamada `f(…)`, e as funções atravessadas entre a declaração e a
    /// referência.
    refs: Vec<(FunctionId, bool, Vec<Option<FunctionId>>)>,
    /// Funções locais que não podem ser diretas pela forma (genérica,
    /// `async`/gerador, parâmetro opcional ou nomeado, lê um `late` de fora).
    discarded_set: HashSet<FunctionId>,
    /// Offsets das declarações `late`.
    lates: HashSet<usize>,
    /// Nomes de fora da raiz que são `late`.
    outside_lates: HashSet<SymbolId>,
    /// O identificador que é o alvo da chamada sendo percorrida.
    call_target: Option<ExprId>,
    /// As funções locais diretas (segunda passada): chamá-las não captura
    /// nada, o nome delas não é variável, e o que elas leem de fora vai por
    /// parâmetro — a variável só precisa de célula se alguma função que
    /// escapa também a captura.
    directs: Option<HashSet<FunctionId>>,
}

impl<'x, 'a> Walk<'x, 'a> {
    fn new(ctx: &'x Context<'a>, unit: UnitId, ast: &'x ast::Ast) -> Self {
        Self {
            ctx,
            unit,
            ast,
            scopes: vec![HashMap::new()],
            prof: 0,
            captured: HashSet::new(),
            assigned: HashSet::new(),
            free_set: Vec::new(),
            stored_free: HashSet::new(),
            uses_this: false,
            stack: Vec::new(),
            local_functions: HashMap::new(),
            refs: Vec::new(),
            discarded_set: HashSet::new(),
            lates: HashSet::new(),
            outside_lates: HashSet::new(),
            call_target: None,
            directs: None,
        }
    }

    fn open(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn close(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: ast::Name) {
        let d = Decl {
            offset: name.span.start as usize,
            prof: self.prof,
        };
        self.scopes
            .last_mut()
            .expect("escopo")
            .insert(name.sym, d);
    }

    fn lookup(&self, sym: SymbolId) -> Option<Decl> {
        self.scopes.iter().rev().find_map(|e| e.get(&sym).copied())
    }

    /// Referência a um nome (leitura ou gravação) na expressão `e`.
    fn reference(&mut self, e: ExprId, sym: SymbolId, writes: bool) {
        match self.ctx.get_resolved(self.unit, e) {
            None | Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) => {}
            Some(_) => return,
        }
        match self.lookup(sym) {
            Some(d) => {
                let crossed = self.stack[(d.prof as usize).min(self.stack.len())..].to_vec();
                if let Some(&f) = self.local_functions.get(&d.offset) {
                    let alvo = !writes && self.call_target == Some(e);
                    self.refs.push((f, alvo, crossed.clone()));
                    // O nome de uma função direta não é variável.
                    if self.directs.as_ref().is_some_and(|s| s.contains(&f)) {
                        return;
                    }
                }
                if self.lates.contains(&d.offset) {
                    self.discarded_set.extend(crossed.iter().flatten().copied());
                }
                self.use_declaration(d, writes, &crossed);
            }
            None => {
                if self.outside_lates.contains(&sym) {
                    self.discarded_set.extend(self.stack.iter().flatten().copied());
                }
                if !self.free_set.contains(&sym) {
                    self.free_set.push(sym);
                }
                if writes {
                    self.stored_free.insert(sym);
                }
            }
        }
    }

    /// Leitura ou gravação da declaração `d`, vista de dentro das funções
    /// `crossed`: da profundidade 0, é captura (pelo ambiente) se
    /// alguma delas escapa, e captura direta (por parâmetro) se todas são
    /// funções locais diretas.
    fn use_declaration(&mut self, d: Decl, writes: bool, crossed: &[Option<FunctionId>]) {
        if d.prof != 0 {
            return;
        }
        if self.prof > 0 {
            let only_directs = self
                .directs
                .as_ref()
                .is_some_and(|s| crossed.iter().all(|f| f.is_some_and(|f| s.contains(&f))));
            // Só por funções diretas: a variável fica no quadro de quem a
            // declara e vai pelo endereço (`direct_functions.rs`).
            if !only_directs {
                self.captured.insert(d.offset);
            }
        }
        if writes {
            self.assigned.insert(d.offset);
        }
    }

    /// Uma função (closure ou local) aninhada: parâmetros e corpo numa
    /// profundidade a mais. `local`: é a declaração de função local.
    fn function(&mut self, fid: FunctionId) {
        self.function_of(fid, false);
    }

    fn function_of(&mut self, fid: FunctionId, local: bool) {
        let f = self.ast.function(fid);
        self.stack.push(local.then_some(fid));
        self.prof += 1;
        self.open();
        if let Some(params) = &f.parameters {
            self.parameters(params);
        }
        self.body(&f.body);
        self.close();
        self.prof -= 1;
        self.stack.pop();
    }

    fn parameters(&mut self, params: &[ast::Parameter]) {
        for p in params {
            if let Some(d) = p.default_value {
                self.expr(d);
            }
            if let Some(n) = p.name {
                self.declare(n);
            }
        }
    }

    fn body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(e) => self.expr(*e),
            FunctionBody::Empty | FunctionBody::Native(_) => {}
        }
    }

    fn variable_list(&mut self, list: &ast::VariableList) {
        for v in list.variables.iter() {
            if let Some(i) = v.initializer {
                self.expr(i);
            }
            self.declare(v.name);
            if list.late {
                self.lates.insert(v.name.span.start as usize);
            }
            // Mesmo sem escrita explícita, a marca de inicialização de um
            // `late` sem initializer deve ser compartilhada com a closure.
            if list.late && v.initializer.is_none() {
                self.assigned.insert(v.name.span.start as usize);
            }
        }
    }

    fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        match &ast.stmt(s).kind {
            StmtKind::Block(ss) => {
                self.open();
                for &x in ss.iter() {
                    self.stmt(x);
                }
                self.close();
            }
            StmtKind::Variables(list) => self.variable_list(list),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(*value);
                self.pattern(*pattern, true);
            }
            StmtKind::Function(fid) => {
                // O nome existe antes do corpo (recursão) e é ligado depois
                // de a closure existir: conta como atribuído — salvo a
                // função direta, cujo nome não é variável.
                let f = ast.function(*fid);
                if let Some(n) = f.name {
                    self.declare(n);
                    self.local_functions.insert(n.span.start as usize, *fid);
                    let direct = self.directs.as_ref().is_some_and(|s| s.contains(fid));
                    if self.prof == 0 && !direct {
                        self.assigned.insert(n.span.start as usize);
                    }
                }
                let params = f.parameters.as_deref().unwrap_or(&[]);
                if !f.type_params.is_empty()
                    || f.modifier != ast::AsyncModifier::None
                    || params.iter().any(|p| p.kind != ast::ParameterKind::Required || p.default_value.is_some())
                    || !matches!(f.body, FunctionBody::Block(_) | FunctionBody::Expression(_))
                {
                    self.discarded_set.insert(*fid);
                }
                self.function_of(*fid, true);
            }
            StmtKind::Expression(e) => self.expr(*e),
            StmtKind::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                self.expr(*condition);
                self.open();
                if let Some(p) = case_pattern {
                    self.pattern(*p, true);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.stmt(*then);
                self.close();
                if let Some(e) = else_ {
                    self.stmt(*e);
                }
            }
            StmtKind::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.open();
                match init {
                    Some(ForInit::Variables(l)) => self.variable_list(l),
                    Some(ForInit::Expression(e)) => self.expr(*e),
                    Some(ForInit::Pattern { pattern, value, .. }) => {
                        self.expr(*value);
                        self.pattern(*pattern, true);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &u in updates.iter() {
                    self.expr(u);
                }
                self.stmt(*body);
                self.close();
            }
            StmtKind::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.expr(*iterable);
                self.open();
                self.for_in_target(target);
                self.stmt(*body);
                self.close();
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
                for c in cases.iter() {
                    self.open();
                    if let Some(p) = c.pattern {
                        self.pattern(p, true);
                    }
                    if let Some(g) = c.guard {
                        self.expr(g);
                    }
                    for &x in c.body.iter() {
                        self.stmt(x);
                    }
                    self.close();
                }
            }
            StmtKind::Return(e) => {
                if let Some(e) = e {
                    self.expr(*e);
                }
            }
            StmtKind::Yield { value, .. } => self.expr(*value),
            StmtKind::Try {
                body,
                catches,
                finally_,
            } => {
                self.stmt(*body);
                for c in catches.iter() {
                    self.open();
                    if let Some(n) = c.exception {
                        self.declare(n);
                    }
                    if let Some(n) = c.stack_trace {
                        self.declare(n);
                    }
                    self.stmt(c.body);
                    self.close();
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

    fn for_in_target(&mut self, alvo: &ForInTarget) {
        match alvo {
            ForInTarget::Declared { name, .. } => self.declare(*name),
            ForInTarget::Pattern { pattern, .. } => self.pattern(*pattern, true),
            ForInTarget::Expression(e) => self.assignment_target(*e),
        }
    }

    /// O alvo de uma atribuição: um identificador é gravação; o resto,
    /// leitura das subexpressões.
    fn assignment_target(&mut self, e: ExprId) {
        if let ExprKind::Identifier(n) = &self.ast.expr(e).kind {
            self.reference(e, n.sym, true);
        } else {
            self.expr(e);
        }
    }

    /// Um padrão: `declares` para os de declaração (`var (a, b) = …`, `case`),
    /// senão os nomes são gravações de variáveis existentes (`(a, b) = …`).
    fn pattern(&mut self, p: PatternId, declares: bool) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Wildcard { .. } => {}
            PatternKind::Variable { name, .. } => {
                if declares {
                    self.declare(*name);
                } else {
                    match self.lookup(name.sym) {
                        Some(d) => {
                            let crossed = self.stack[(d.prof as usize).min(self.stack.len())..].to_vec();
                            if self.lates.contains(&d.offset) {
                                self.discarded_set.extend(crossed.iter().flatten().copied());
                            }
                            self.use_declaration(d, true, &crossed);
                        }
                        None => {
                            if !self.free_set.contains(&name.sym) {
                                self.free_set.push(name.sym);
                            }
                        }
                    }
                }
            }
            PatternKind::Constant(e) => self.expr(*e),
            PatternKind::Relational { value, .. } => self.expr(*value),
            PatternKind::Or(a, b) | PatternKind::And(a, b) => {
                self.pattern(*a, declares);
                self.pattern(*b, declares);
            }
            PatternKind::NullCheck(x)
            | PatternKind::NullAssert(x)
            | PatternKind::Parenthesized(x)
            | PatternKind::Cast { pattern: x, .. } => self.pattern(*x, declares),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => {
                            self.pattern(*x, declares)
                        }
                        ListPatternElement::Rest(None) => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    self.expr(en.key);
                    self.pattern(en.value, declares);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.pattern(f.pattern, declares);
                }
            }
        }
    }

    fn element(&mut self, el: &CollectionElement) {
        match el {
            CollectionElement::Expression(e) | CollectionElement::NullAwareExpression(e) => {
                self.expr(*e)
            }
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key);
                self.expr(*value);
            }
            CollectionElement::Spread { value, .. } => self.expr(*value),
            CollectionElement::If {
                condition,
                case_pattern,
                guard,
                then,
                else_,
            } => {
                self.expr(*condition);
                self.open();
                if let Some(p) = case_pattern {
                    self.pattern(*p, true);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.element(then);
                self.close();
                if let Some(e) = else_ {
                    self.element(e);
                }
            }
            CollectionElement::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.open();
                match init {
                    Some(ForInit::Variables(l)) => self.variable_list(l),
                    Some(ForInit::Expression(e)) => self.expr(*e),
                    Some(ForInit::Pattern { pattern, value, .. }) => {
                        self.expr(*value);
                        self.pattern(*pattern, true);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &u in updates.iter() {
                    self.expr(u);
                }
                self.element(body);
                self.close();
            }
            CollectionElement::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.expr(*iterable);
                self.open();
                self.for_in_target(target);
                self.element(body);
                self.close();
            }
        }
    }

    fn arguments(&mut self, a: &ast::Arguments) {
        for x in a.args.iter() {
            self.expr(x.value);
        }
    }

    fn expr(&mut self, e: ExprId) {
        let ast = self.ast;
        match &ast.expr(e).kind {
            ExprKind::Int(_)
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Symbol(_)
            | ExprKind::CascadeTarget
            // `.id` (3.10) nomeia um membro estático da declaração do
            // contexto: não lê local.
            | ExprKind::DotShorthand { .. }
            | ExprKind::Rethrow => {}
            ExprKind::String(s) => {
                for p in s.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x);
                    }
                }
            }
            ExprKind::Identifier(n) => self.reference(e, n.sym, false),
            ExprKind::This | ExprKind::Super => self.uses_this = true,
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.element(el);
                }
            }
            ExprKind::Record {
                positional, named, ..
            } => {
                for &x in positional.iter() {
                    self.expr(x);
                }
                for (_, x) in named.iter() {
                    self.expr(*x);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => self.arguments(arguments),
            ExprKind::FunctionExpression(fid) => self.function(*fid),
            ExprKind::Property { target, .. } => self.expr(*target),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target);
                self.expr(*index);
            }
            ExprKind::Call { target, arguments } => {
                let saved = self.call_target.replace(*target);
                self.expr(*target);
                self.call_target = saved;
                self.arguments(arguments);
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target),
            ExprKind::Unary { op, operand } => match op {
                UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
                    self.assignment_target(*operand)
                }
                _ => self.expr(*operand),
            },
            ExprKind::Binary { left, right, .. } => {
                self.expr(*left);
                self.expr(*right);
            }
            ExprKind::Conditional {
                condition,
                then,
                else_,
            } => {
                self.expr(*condition);
                self.expr(*then);
                self.expr(*else_);
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.expr(*value),
            ExprKind::Assign { target, value, .. } => {
                self.assignment_target(*target);
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.expr(*value);
                self.pattern(*pattern, false);
            }
            ExprKind::Cascade {
                target, sections, ..
            } => {
                self.expr(*target);
                for &s in sections.iter() {
                    self.expr(s);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value);
                for c in cases.iter() {
                    self.open();
                    self.pattern(c.pattern, true);
                    if let Some(g) = c.guard {
                        self.expr(g);
                    }
                    self.expr(c.body);
                    self.close();
                }
            }
        }
    }
}

/// O que a análise de uma função entrega ao lowering.
pub struct Captures {
    /// Offsets das declarações desta função que moram numa célula.
    pub cells: HashSet<usize>,
    /// Offsets das declarações desta função gravadas em algum ponto.
    pub assigned: HashSet<usize>,
    /// As funções locais (em qualquer profundidade) chamadas só
    /// diretamente, de dentro de funções também diretas
    /// (`direct_functions.rs`).
    pub directs: HashSet<FunctionId>,
}

/// Corpo de uma função a analisar: parâmetros e as partes executáveis.
pub struct Root<'x> {
    pub parameters: &'x [ast::Parameter],
    pub body: Option<&'x FunctionBody>,
    /// Lista de inicialização de um construtor.
    pub initializers: &'x [ast::Initializer],
}

/// As células das variáveis declaradas na função `root` (parâmetros e
/// locais do corpo, fora das closures aninhadas).
pub fn analyze(ctx: &Context, unit: UnitId, ast: &ast::Ast, root: Root) -> Captures {
    analyze_with(ctx, unit, ast, root, false, &HashSet::new())
}

/// [`analyze`]; com `with_direct`, as funções locais que não escapam viram
/// diretas (`direct_functions.rs`): primeiro as referências a cada função
/// local, e a análise de escape — só é direta a função cujo nome aparece
/// apenas como alvo de chamada `f(…)`, de dentro da função que a declara
/// ou de outras funções diretas (uma closure que escapa poderia chamá-la
/// depois de a função de fora retornar, com os endereços dos locais
/// dela) —; depois as células com esse conjunto.
///
/// `outside_lates`: os nomes de fora da raiz (capturas já ligadas) que são
/// `late`; uma função que os lê também não pode ser direta.
pub fn analyze_with(
    ctx: &Context,
    unit: UnitId,
    ast: &ast::Ast,
    root: Root,
    with_direct: bool,
    outside_lates: &HashSet<SymbolId>,
) -> Captures {
    let directs = if with_direct {
        let mut p = Walk::new(ctx, unit, ast);
        p.outside_lates = outside_lates.clone();
        p.walk(&root);
        let mut directs: HashSet<FunctionId> =
            p.local_functions.values().copied().filter(|f| !p.discarded_set.contains(f)).collect();
        loop {
            let before = directs.len();
            for (f, alvo, crossed) in &p.refs {
                if directs.contains(f)
                    && !(*alvo && crossed.iter().all(|g| g.is_some_and(|g| directs.contains(&g))))
                {
                    directs.remove(f);
                }
            }
            if directs.len() == before {
                break;
            }
        }
        directs
    } else {
        HashSet::new()
    };
    let mut p = Walk::new(ctx, unit, ast);
    p.directs = Some(directs.clone());
    p.walk(&root);
    let cells: HashSet<usize> = p.captured.intersection(&p.assigned).copied().collect();
    Captures { cells, assigned: p.assigned, directs }
}

impl Walk<'_, '_> {
    /// Percorre a função `root` inteira: parâmetros, lista de
    /// inicialização e corpo.
    fn walk(&mut self, root: &Root) {
        self.parameters(root.parameters);
        for i in root.initializers {
            match i {
                ast::Initializer::Field { value, .. } => self.expr(*value),
                ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => {
                    self.arguments(arguments)
                }
                ast::Initializer::Assert {
                    condition, message, ..
                } => {
                    self.expr(*condition);
                    if let Some(m) = message {
                        self.expr(*m);
                    }
                }
            }
        }
        if let Some(c) = root.body {
            self.body(c);
        }
    }
}

/// As variáveis livres da função aninhada `fid` (na ordem da primeira
/// referência) e se ela usa `this` explicitamente.
pub fn free_set(ctx: &Context, unit: UnitId, ast: &ast::Ast, fid: FunctionId) -> (Vec<SymbolId>, bool) {
    let mut p = Walk::new(ctx, unit, ast);
    let f = ast.function(fid);
    if let Some(params) = &f.parameters {
        p.parameters(params);
    }
    p.body(&f.body);
    (p.free_set, p.uses_this)
}

/// Os nomes declarados fora de `body` e de `exprs` que eles gravam
/// (atribuição, `++`/`--`, atribuição composta, padrão; também de dentro de
/// closures aninhadas): o que decide se um laço `for` é contado
/// (`statements.rs`).
pub fn stored_from_outside(ctx: &Context, unit: UnitId, ast: &ast::Ast, body: StmtId, exprs: &[ExprId]) -> HashSet<SymbolId> {
    let mut p = Walk::new(ctx, unit, ast);
    p.stmt(body);
    for &e in exprs {
        p.expr(e);
    }
    p.stored_free
}
