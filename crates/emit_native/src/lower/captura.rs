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
struct Percurso<'x, 'a> {
    ctx: &'x Context<'a>,
    unit: UnitId,
    ast: &'x ast::Ast,
    escopos: Vec<HashMap<SymbolId, Decl>>,
    prof: u32,
    /// Declarações da profundidade 0 lidas ou gravadas de dentro de uma
    /// função aninhada.
    capturadas: HashSet<usize>,
    /// Declarações da profundidade 0 gravadas em algum ponto (ou funções
    /// locais).
    atribuidas: HashSet<usize>,
    /// Nomes lidos ou gravados sem declaração no percurso (as variáveis
    /// livres, na ordem da primeira referência).
    livres: Vec<SymbolId>,
    /// Nomes sem declaração no percurso que ele grava.
    livres_gravados: HashSet<SymbolId>,
    usa_this: bool,
    // --- funções locais diretas (`funcoes_diretas.rs`) ---
    /// As funções abertas, da mais externa à corrente: a função local
    /// nomeada (`Some`) ou a expressão de função (`None`). O tamanho é `prof`.
    pilha: Vec<Option<FunctionId>>,
    /// Nome (offset) de cada função local declarada no percurso → a função.
    funcoes_locais: HashMap<usize, FunctionId>,
    /// Cada referência a uma função local: a função, se ela é o alvo de uma
    /// chamada `f(…)`, e as funções atravessadas entre a declaração e a
    /// referência.
    refs: Vec<(FunctionId, bool, Vec<Option<FunctionId>>)>,
    /// Funções locais que não podem ser diretas pela forma (genérica,
    /// `async`/gerador, parâmetro opcional ou nomeado, lê um `late` de fora).
    descartadas: HashSet<FunctionId>,
    /// Offsets das declarações `late`.
    lates: HashSet<usize>,
    /// Nomes de fora da raiz que são `late`.
    lates_de_fora: HashSet<SymbolId>,
    /// O identificador que é o alvo da chamada sendo percorrida.
    alvo_de_chamada: Option<ExprId>,
    /// As funções locais diretas (segunda passada): chamá-las não captura
    /// nada, o nome delas não é variável, e o que elas leem de fora vai por
    /// parâmetro — a variável só precisa de célula se alguma função que
    /// escapa também a captura.
    diretas: Option<HashSet<FunctionId>>,
}

impl<'x, 'a> Percurso<'x, 'a> {
    fn novo(ctx: &'x Context<'a>, unit: UnitId, ast: &'x ast::Ast) -> Self {
        Self {
            ctx,
            unit,
            ast,
            escopos: vec![HashMap::new()],
            prof: 0,
            capturadas: HashSet::new(),
            atribuidas: HashSet::new(),
            livres: Vec::new(),
            livres_gravados: HashSet::new(),
            usa_this: false,
            pilha: Vec::new(),
            funcoes_locais: HashMap::new(),
            refs: Vec::new(),
            descartadas: HashSet::new(),
            lates: HashSet::new(),
            lates_de_fora: HashSet::new(),
            alvo_de_chamada: None,
            diretas: None,
        }
    }

    fn abrir(&mut self) {
        self.escopos.push(HashMap::new());
    }

    fn fechar(&mut self) {
        self.escopos.pop();
    }

    fn declarar(&mut self, nome: ast::Name) {
        let d = Decl {
            offset: nome.span.start as usize,
            prof: self.prof,
        };
        self.escopos
            .last_mut()
            .expect("escopo")
            .insert(nome.sym, d);
    }

    fn buscar(&self, sym: SymbolId) -> Option<Decl> {
        self.escopos.iter().rev().find_map(|e| e.get(&sym).copied())
    }

    /// Referência a um nome (leitura ou gravação) na expressão `e`.
    fn referencia(&mut self, e: ExprId, sym: SymbolId, grava: bool) {
        match self.ctx.get_resolved(self.unit, e) {
            None | Some(Resolved::Local(_)) | Some(Resolved::Parameter { .. }) => {}
            Some(_) => return,
        }
        match self.buscar(sym) {
            Some(d) => {
                let atravessadas = self.pilha[(d.prof as usize).min(self.pilha.len())..].to_vec();
                if let Some(&f) = self.funcoes_locais.get(&d.offset) {
                    let alvo = !grava && self.alvo_de_chamada == Some(e);
                    self.refs.push((f, alvo, atravessadas.clone()));
                    // O nome de uma função direta não é variável.
                    if self.diretas.as_ref().is_some_and(|s| s.contains(&f)) {
                        return;
                    }
                }
                if self.lates.contains(&d.offset) {
                    self.descartadas.extend(atravessadas.iter().flatten().copied());
                }
                self.usar_declaracao(d, grava, &atravessadas);
            }
            None => {
                if self.lates_de_fora.contains(&sym) {
                    self.descartadas.extend(self.pilha.iter().flatten().copied());
                }
                if !self.livres.contains(&sym) {
                    self.livres.push(sym);
                }
                if grava {
                    self.livres_gravados.insert(sym);
                }
            }
        }
    }

    /// Leitura ou gravação da declaração `d`, vista de dentro das funções
    /// `atravessadas`: da profundidade 0, é captura (pelo ambiente) se
    /// alguma delas escapa, e captura direta (por parâmetro) se todas são
    /// funções locais diretas.
    fn usar_declaracao(&mut self, d: Decl, grava: bool, atravessadas: &[Option<FunctionId>]) {
        if d.prof != 0 {
            return;
        }
        if self.prof > 0 {
            let so_diretas = self
                .diretas
                .as_ref()
                .is_some_and(|s| atravessadas.iter().all(|f| f.is_some_and(|f| s.contains(&f))));
            // Só por funções diretas: a variável fica no quadro de quem a
            // declara e vai pelo endereço (`funcoes_diretas.rs`).
            if !so_diretas {
                self.capturadas.insert(d.offset);
            }
        }
        if grava {
            self.atribuidas.insert(d.offset);
        }
    }

    /// Uma função (closure ou local) aninhada: parâmetros e corpo numa
    /// profundidade a mais. `local`: é a declaração de função local.
    fn funcao(&mut self, fid: FunctionId) {
        self.funcao_de(fid, false);
    }

    fn funcao_de(&mut self, fid: FunctionId, local: bool) {
        let f = self.ast.function(fid);
        self.pilha.push(local.then_some(fid));
        self.prof += 1;
        self.abrir();
        if let Some(params) = &f.parameters {
            self.parametros(params);
        }
        self.corpo(&f.body);
        self.fechar();
        self.prof -= 1;
        self.pilha.pop();
    }

    fn parametros(&mut self, params: &[ast::Parameter]) {
        for p in params {
            if let Some(d) = p.default_value {
                self.expr(d);
            }
            if let Some(n) = p.name {
                self.declarar(n);
            }
        }
    }

    fn corpo(&mut self, corpo: &FunctionBody) {
        match corpo {
            FunctionBody::Block(s) => self.stmt(*s),
            FunctionBody::Expression(e) => self.expr(*e),
            FunctionBody::Empty | FunctionBody::Native(_) => {}
        }
    }

    fn lista_de_variaveis(&mut self, lista: &ast::VariableList) {
        for v in lista.variables.iter() {
            if let Some(i) = v.initializer {
                self.expr(i);
            }
            self.declarar(v.name);
            if lista.late {
                self.lates.insert(v.name.span.start as usize);
            }
            // Mesmo sem escrita explícita, a marca de inicialização de um
            // `late` sem initializer deve ser compartilhada com a closure.
            if lista.late && v.initializer.is_none() {
                self.atribuidas.insert(v.name.span.start as usize);
            }
        }
    }

    fn stmt(&mut self, s: StmtId) {
        let ast = self.ast;
        match &ast.stmt(s).kind {
            StmtKind::Block(ss) => {
                self.abrir();
                for &x in ss.iter() {
                    self.stmt(x);
                }
                self.fechar();
            }
            StmtKind::Variables(lista) => self.lista_de_variaveis(lista),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(*value);
                self.padrao(*pattern, true);
            }
            StmtKind::Function(fid) => {
                // O nome existe antes do corpo (recursão) e é ligado depois
                // de a closure existir: conta como atribuído — salvo a
                // função direta, cujo nome não é variável.
                let f = ast.function(*fid);
                if let Some(n) = f.name {
                    self.declarar(n);
                    self.funcoes_locais.insert(n.span.start as usize, *fid);
                    let direta = self.diretas.as_ref().is_some_and(|s| s.contains(fid));
                    if self.prof == 0 && !direta {
                        self.atribuidas.insert(n.span.start as usize);
                    }
                }
                let params = f.parameters.as_deref().unwrap_or(&[]);
                if !f.type_params.is_empty()
                    || f.modifier != ast::AsyncModifier::None
                    || params.iter().any(|p| p.kind != ast::ParameterKind::Required || p.default_value.is_some())
                    || !matches!(f.body, FunctionBody::Block(_) | FunctionBody::Expression(_))
                {
                    self.descartadas.insert(*fid);
                }
                self.funcao_de(*fid, true);
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
                self.abrir();
                if let Some(p) = case_pattern {
                    self.padrao(*p, true);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.stmt(*then);
                self.fechar();
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
                self.abrir();
                match init {
                    Some(ForInit::Variables(l)) => self.lista_de_variaveis(l),
                    Some(ForInit::Expression(e)) => self.expr(*e),
                    None => {}
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &u in updates.iter() {
                    self.expr(u);
                }
                self.stmt(*body);
                self.fechar();
            }
            StmtKind::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.expr(*iterable);
                self.abrir();
                self.alvo_for_in(target);
                self.stmt(*body);
                self.fechar();
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
                    self.abrir();
                    if let Some(p) = c.pattern {
                        self.padrao(p, true);
                    }
                    if let Some(g) = c.guard {
                        self.expr(g);
                    }
                    for &x in c.body.iter() {
                        self.stmt(x);
                    }
                    self.fechar();
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
                    self.abrir();
                    if let Some(n) = c.exception {
                        self.declarar(n);
                    }
                    if let Some(n) = c.stack_trace {
                        self.declarar(n);
                    }
                    self.stmt(c.body);
                    self.fechar();
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

    fn alvo_for_in(&mut self, alvo: &ForInTarget) {
        match alvo {
            ForInTarget::Declared { name, .. } => self.declarar(*name),
            ForInTarget::Pattern { pattern, .. } => self.padrao(*pattern, true),
            ForInTarget::Expression(e) => self.alvo_de_atribuicao(*e),
        }
    }

    /// O alvo de uma atribuição: um identificador é gravação; o resto,
    /// leitura das subexpressões.
    fn alvo_de_atribuicao(&mut self, e: ExprId) {
        if let ExprKind::Identifier(n) = &self.ast.expr(e).kind {
            self.referencia(e, n.sym, true);
        } else {
            self.expr(e);
        }
    }

    /// Um padrão: `declara` para os de declaração (`var (a, b) = …`, `case`),
    /// senão os nomes são gravações de variáveis existentes (`(a, b) = …`).
    fn padrao(&mut self, p: PatternId, declara: bool) {
        let ast = self.ast;
        match &ast.pattern(p).kind {
            PatternKind::Wildcard { .. } => {}
            PatternKind::Variable { name, .. } => {
                if declara {
                    self.declarar(*name);
                } else {
                    match self.buscar(name.sym) {
                        Some(d) => {
                            let atravessadas = self.pilha[(d.prof as usize).min(self.pilha.len())..].to_vec();
                            if self.lates.contains(&d.offset) {
                                self.descartadas.extend(atravessadas.iter().flatten().copied());
                            }
                            self.usar_declaracao(d, true, &atravessadas);
                        }
                        None => {
                            if !self.livres.contains(&name.sym) {
                                self.livres.push(name.sym);
                            }
                        }
                    }
                }
            }
            PatternKind::Constant(e) => self.expr(*e),
            PatternKind::Relational { value, .. } => self.expr(*value),
            PatternKind::Or(a, b) | PatternKind::And(a, b) => {
                self.padrao(*a, declara);
                self.padrao(*b, declara);
            }
            PatternKind::NullCheck(x)
            | PatternKind::NullAssert(x)
            | PatternKind::Parenthesized(x)
            | PatternKind::Cast { pattern: x, .. } => self.padrao(*x, declara),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => {
                            self.padrao(*x, declara)
                        }
                        ListPatternElement::Rest(None) => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    self.expr(en.key);
                    self.padrao(en.value, declara);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.padrao(f.pattern, declara);
                }
            }
        }
    }

    fn elemento(&mut self, el: &CollectionElement) {
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
                self.abrir();
                if let Some(p) = case_pattern {
                    self.padrao(*p, true);
                }
                if let Some(g) = guard {
                    self.expr(*g);
                }
                self.elemento(then);
                self.fechar();
                if let Some(e) = else_ {
                    self.elemento(e);
                }
            }
            CollectionElement::For {
                init,
                condition,
                updates,
                body,
                ..
            } => {
                self.abrir();
                match init {
                    Some(ForInit::Variables(l)) => self.lista_de_variaveis(l),
                    Some(ForInit::Expression(e)) => self.expr(*e),
                    None => {}
                }
                if let Some(c) = condition {
                    self.expr(*c);
                }
                for &u in updates.iter() {
                    self.expr(u);
                }
                self.elemento(body);
                self.fechar();
            }
            CollectionElement::ForIn {
                target,
                iterable,
                body,
                ..
            } => {
                self.expr(*iterable);
                self.abrir();
                self.alvo_for_in(target);
                self.elemento(body);
                self.fechar();
            }
        }
    }

    fn argumentos(&mut self, a: &ast::Arguments) {
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
            ExprKind::Identifier(n) => self.referencia(e, n.sym, false),
            ExprKind::This | ExprKind::Super => self.usa_this = true,
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.elemento(el);
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
            ExprKind::InstanceCreation { arguments, .. } => self.argumentos(arguments),
            ExprKind::FunctionExpression(fid) => self.funcao(*fid),
            ExprKind::Property { target, .. } => self.expr(*target),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target);
                self.expr(*index);
            }
            ExprKind::Call { target, arguments } => {
                let salvo = self.alvo_de_chamada.replace(*target);
                self.expr(*target);
                self.alvo_de_chamada = salvo;
                self.argumentos(arguments);
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target),
            ExprKind::Unary { op, operand } => match op {
                UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
                    self.alvo_de_atribuicao(*operand)
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
                self.alvo_de_atribuicao(*target);
                self.expr(*value);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.expr(*value);
                self.padrao(*pattern, false);
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
                    self.abrir();
                    self.padrao(c.pattern, true);
                    if let Some(g) = c.guard {
                        self.expr(g);
                    }
                    self.expr(c.body);
                    self.fechar();
                }
            }
        }
    }
}

/// O que a análise de uma função entrega ao lowering.
pub struct Capturas {
    /// Offsets das declarações desta função que moram numa célula.
    pub celulas: HashSet<usize>,
    /// Offsets das declarações desta função gravadas em algum ponto.
    pub atribuidas: HashSet<usize>,
    /// As funções locais (em qualquer profundidade) chamadas só
    /// diretamente, de dentro de funções também diretas
    /// (`funcoes_diretas.rs`).
    pub diretas: HashSet<FunctionId>,
}

/// Corpo de uma função a analisar: parâmetros e as partes executáveis.
pub struct Raiz<'x> {
    pub parametros: &'x [ast::Parameter],
    pub corpo: Option<&'x FunctionBody>,
    /// Lista de inicialização de um construtor.
    pub inicializadores: &'x [ast::Initializer],
}

/// As células das variáveis declaradas na função `raiz` (parâmetros e
/// locais do corpo, fora das closures aninhadas).
pub fn analisar(ctx: &Context, unit: UnitId, ast: &ast::Ast, raiz: Raiz) -> Capturas {
    analisar_com(ctx, unit, ast, raiz, false, &HashSet::new())
}

/// [`analisar`]; com `com_diretas`, as funções locais que não escapam viram
/// diretas (`funcoes_diretas.rs`): primeiro as referências a cada função
/// local, e a análise de escape — só é direta a função cujo nome aparece
/// apenas como alvo de chamada `f(…)`, de dentro da função que a declara
/// ou de outras funções diretas (uma closure que escapa poderia chamá-la
/// depois de a função de fora retornar, com os endereços dos locais
/// dela) —; depois as células com esse conjunto.
///
/// `lates_de_fora`: os nomes de fora da raiz (capturas já ligadas) que são
/// `late`; uma função que os lê também não pode ser direta.
pub fn analisar_com(
    ctx: &Context,
    unit: UnitId,
    ast: &ast::Ast,
    raiz: Raiz,
    com_diretas: bool,
    lates_de_fora: &HashSet<SymbolId>,
) -> Capturas {
    let diretas = if com_diretas {
        let mut p = Percurso::novo(ctx, unit, ast);
        p.lates_de_fora = lates_de_fora.clone();
        p.percorrer(&raiz);
        let mut diretas: HashSet<FunctionId> =
            p.funcoes_locais.values().copied().filter(|f| !p.descartadas.contains(f)).collect();
        loop {
            let antes = diretas.len();
            for (f, alvo, atravessadas) in &p.refs {
                if diretas.contains(f)
                    && !(*alvo && atravessadas.iter().all(|g| g.is_some_and(|g| diretas.contains(&g))))
                {
                    diretas.remove(f);
                }
            }
            if diretas.len() == antes {
                break;
            }
        }
        diretas
    } else {
        HashSet::new()
    };
    let mut p = Percurso::novo(ctx, unit, ast);
    p.diretas = Some(diretas.clone());
    p.percorrer(&raiz);
    let celulas: HashSet<usize> = p.capturadas.intersection(&p.atribuidas).copied().collect();
    Capturas { celulas, atribuidas: p.atribuidas, diretas }
}

impl Percurso<'_, '_> {
    /// Percorre a função `raiz` inteira: parâmetros, lista de
    /// inicialização e corpo.
    fn percorrer(&mut self, raiz: &Raiz) {
        self.parametros(raiz.parametros);
        for i in raiz.inicializadores {
            match i {
                ast::Initializer::Field { value, .. } => self.expr(*value),
                ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => {
                    self.argumentos(arguments)
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
        if let Some(c) = raiz.corpo {
            self.corpo(c);
        }
    }
}

/// As variáveis livres da função aninhada `fid` (na ordem da primeira
/// referência) e se ela usa `this` explicitamente.
pub fn livres(ctx: &Context, unit: UnitId, ast: &ast::Ast, fid: FunctionId) -> (Vec<SymbolId>, bool) {
    let mut p = Percurso::novo(ctx, unit, ast);
    let f = ast.function(fid);
    if let Some(params) = &f.parameters {
        p.parametros(params);
    }
    p.corpo(&f.body);
    (p.livres, p.usa_this)
}

/// Os nomes declarados fora de `corpo` e de `exprs` que eles gravam
/// (atribuição, `++`/`--`, atribuição composta, padrão; também de dentro de
/// closures aninhadas): o que decide se um laço `for` é contado
/// (`comandos.rs`).
pub fn gravados_de_fora(ctx: &Context, unit: UnitId, ast: &ast::Ast, corpo: StmtId, exprs: &[ExprId]) -> HashSet<SymbolId> {
    let mut p = Percurso::novo(ctx, unit, ast);
    p.stmt(corpo);
    for &e in exprs {
        p.expr(e);
    }
    p.livres_gravados
}
