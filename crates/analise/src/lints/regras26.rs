//! O vigésimo sexto lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), pela
//! semântica da unidade:
//!
//! * `unnecessary_statements`: a expressão sem efeito claro numa instrução
//!   de expressão, na inicialização e nas atualizações de um `for` de
//!   expressões, e a seção de cascata que é um acesso de tipo função. O
//!   getter escrito (não sintético) não conta; `??`, `||` e `&&` olham só o
//!   lado direito; o condicional, os dois ramos.
//! * `unnecessary_parenthesis`: os parênteses em volta de uma expressão,
//!   pelo pai que o analyzer daria a eles (o `MethodInvocation` quando são o
//!   alvo de uma chamada de método, a `NamedExpression` de um argumento
//!   nomeado ou de um campo de registro, o `ArgumentList` de anotação, de
//!   constante de enum e de `super(…)`/`this(…)`), com todas as exceções do
//!   emissor na ordem dele.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::andar::No;
use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::regras16::{anulavel, e_invocacao_de_metodo, pais_da_unidade, tipo_de_variavel, Pais};
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind, FunctionRef, VariableRef};
use dartforge_frontend::ast::{self, Ast, BinaryOp, CollectionElement, DeclKind, ExprId, ExprKind, ForInit, FunctionBody, Initializer, MemberKind, PatternKind, StmtKind, UnaryOp};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::Type;
use std::collections::HashMap;

/// O elemento resolvido é um `GetterElement` escrito (não sintético).
fn getter_escrito(s: &super::Semantica<'_>, e: ExprId) -> bool {
    let f = match s.corpo.get_resolved(e) {
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::Element(Element::Function(f))) => *f,
        Some(Resolved::ExtensionMember { member, .. }) => *member,
        _ => return false,
    };
    let g = s.program.function(f);
    g.kind == FunctionKind::Getter && !matches!(g.node, FunctionRef::None)
}

/// `_ReportNoClearEffectVisitor`: relata `e` (ou as partes dele) quando não
/// tem efeito claro.
fn sem_efeito(s: &super::Semantica<'_>, a: &Ast, e: ExprId, out: &mut Vec<Span>) {
    match &a.expr(e).kind {
        ExprKind::As { .. }
        | ExprKind::Assign { .. }
        | ExprKind::Await(_)
        | ExprKind::Cascade { .. }
        | ExprKind::Call { .. }
        | ExprKind::InstanceCreation { .. }
        | ExprKind::PatternAssign { .. }
        | ExprKind::Rethrow
        | ExprKind::Throw(_) => {}
        ExprKind::Unary { op: UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert | UnaryOp::PrefixInc | UnaryOp::PrefixDec, .. } => {}
        ExprKind::Binary { op: BinaryOp::IfNull | BinaryOp::Or | BinaryOp::And, right, .. } => sem_efeito(s, a, *right, out),
        ExprKind::Conditional { then, else_, .. } => {
            sem_efeito(s, a, *then, out);
            sem_efeito(s, a, *else_, out);
        }
        ExprKind::Identifier(_) | ExprKind::Property { .. } if getter_escrito(s, e) => {}
        _ => out.push(a.expr(e).span),
    }
}

/// O pai de um nó de parênteses como o analyzer o vê.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pai {
    Parenteses,
    Interpolacao,
    /// Argumento posicional de um `ArgumentList` com `n` argumentos.
    Argumento { n: usize },
    /// A `NamedExpression` de um argumento nomeado.
    Nomeado,
    /// A `NamedExpression` de um campo de registro.
    CampoNomeado,
    CondicaoDoIf,
    CondicaoDoIfElemento,
    CondicaoDoWhile,
    CondicaoDoDo,
    SwitchInstrucao,
    SwitchExpressao,
    PadraoConstante,
    Espalhamento,
    /// `VariableDeclaration`, com o tipo do elemento.
    Variavel(Option<dartforge_types::table::TypeId>),
    /// `PropertyAccess` cujo alvo são os parênteses.
    AcessoDePropriedade(ExprId),
    /// `MethodInvocation` (a chamada e o acesso ao nome).
    InvocacaoDeMetodo { chamada: ExprId, nome: ExprId },
    Indice,
    PosfixoExclamacao,
    Posfixo,
    Prefixo,
    Binario,
    Condicional { entao: bool },
    ChaveDeMapa,
    Cascata,
    InvocacaoDeFuncao { args_de_tipo: bool },
    Como,
    E,
    Atribuicao,
    Retorno,
    Yield,
    InicializadorDeCampo,
    CorpoDeExpressao,
    InstrucaoDeExpressao,
    OutraExpressao,
    Outro,
}

impl Pai {
    /// O pai é uma `Expression` no analyzer.
    fn e_expressao(self) -> bool {
        matches!(
            self,
            Pai::Parenteses
                | Pai::Nomeado
                | Pai::CampoNomeado
                | Pai::SwitchExpressao
                | Pai::AcessoDePropriedade(_)
                | Pai::InvocacaoDeMetodo { .. }
                | Pai::Indice
                | Pai::PosfixoExclamacao
                | Pai::Posfixo
                | Pai::Prefixo
                | Pai::Binario
                | Pai::Condicional { .. }
                | Pai::Cascata
                | Pai::InvocacaoDeFuncao { .. }
                | Pai::Como
                | Pai::E
                | Pai::Atribuicao
                | Pai::OutraExpressao
        )
    }
}

/// O papel de `eu` num elemento de coleção; `direto`: o elemento é filho
/// do literal (o pai é a `Expression` do literal) e não de um
/// `IfElement`/`ForElement`.
fn papel_na_colecao(s: &super::Semantica<'_>, x: &CollectionElement, eu: ExprId, direto: bool) -> Option<Pai> {
    match x {
        CollectionElement::Expression(e) if *e == eu => Some(if direto { Pai::OutraExpressao } else { Pai::Outro }),
        CollectionElement::NullAwareExpression(e) if *e == eu => Some(Pai::Outro),
        CollectionElement::MapEntry { key, .. } if *key == eu => Some(Pai::ChaveDeMapa),
        CollectionElement::MapEntry { value, .. } if *value == eu => Some(Pai::Outro),
        CollectionElement::Spread { value, .. } if *value == eu => Some(Pai::Espalhamento),
        CollectionElement::If { condition, guard, then, else_, .. } => {
            if *condition == eu {
                return Some(Pai::CondicaoDoIfElemento);
            }
            if *guard == Some(eu) {
                return Some(Pai::Outro);
            }
            papel_na_colecao(s, then, eu, false).or_else(|| else_.as_deref().and_then(|x| papel_na_colecao(s, x, eu, false)))
        }
        CollectionElement::For { init, condition, updates, body, .. } => {
            match init {
                Some(ForInit::Variables(l)) => {
                    if let Some(v) = l.variables.iter().find(|v| v.initializer == Some(eu)) {
                        return Some(Pai::Variavel(s.corpo.tipo_local(v.name.span.start)));
                    }
                }
                Some(ForInit::Expression(e)) if *e == eu => return Some(Pai::Outro),
                Some(ForInit::Pattern { value, .. }) if *value == eu => return Some(Pai::Outro),
                _ => {}
            }
            if *condition == Some(eu) || updates.contains(&eu) {
                return Some(Pai::Outro);
            }
            papel_na_colecao(s, body, eu, false)
        }
        CollectionElement::ForIn { iterable, body, .. } => {
            if *iterable == eu {
                return Some(Pai::Outro);
            }
            papel_na_colecao(s, body, eu, false)
        }
        _ => None,
    }
}

/// O papel de `eu` numa lista de argumentos.
fn papel_nos_argumentos(args: &ast::Arguments, eu: ExprId) -> Pai {
    match args.args.iter().find(|x| x.value == eu) {
        Some(x) if x.name.is_some() => Pai::Nomeado,
        Some(_) => Pai::Argumento { n: args.args.len() },
        None => Pai::Outro,
    }
}

/// Os argumentos das anotações e das constantes de enum pelo intervalo
/// (os nós `No::Anotacao`/`No::ArgumentosDeEnum` do passeio).
fn argumentos_por_intervalo(a: &Ast) -> HashMap<(usize, usize), &ast::Arguments> {
    let mut m = HashMap::new();
    fn anotacoes<'a>(xs: &'a [ast::Annotation], m: &mut HashMap<(usize, usize), &'a ast::Arguments>) {
        for x in xs {
            if let Some(args) = &x.arguments {
                m.insert((x.span.start, x.span.end), args);
            }
        }
    }
    fn parametros<'a>(ps: &'a [ast::Parameter], m: &mut HashMap<(usize, usize), &'a ast::Arguments>) {
        for p in ps {
            anotacoes(&p.metadata, m);
            if let Some(internos) = &p.function_parameters {
                parametros(internos, m);
            }
        }
    }
    for d in a.decls.iter() {
        anotacoes(&d.metadata, &mut m);
        match &d.kind {
            DeclKind::Enum(x) => {
                for k in x.constants.iter() {
                    anotacoes(&k.metadata, &mut m);
                    if let Some(args) = &k.arguments {
                        m.insert((k.span.start, k.span.end), args);
                    }
                }
            }
            DeclKind::ExtensionType(x) => anotacoes(&x.representation_metadata, &mut m),
            _ => {}
        }
    }
    for x in a.members.iter() {
        anotacoes(&x.metadata, &mut m);
        if let MemberKind::Constructor(k) = &x.kind {
            parametros(&k.parameters, &mut m);
        }
    }
    for f in a.functions.iter() {
        parametros(f.parameters.as_deref().unwrap_or(&[]), &mut m);
    }
    for t in a.types.iter() {
        if let ast::TypeKind::Function { parameters, .. } = &t.kind {
            parametros(parameters, &mut m);
        }
    }
    m
}

/// O pai (do analyzer) dos parênteses `eu`.
fn pai_de(s: &super::Semantica<'_>, a: &Ast, pais: &Pais, por_intervalo: &HashMap<(usize, usize), &ast::Arguments>, eu: ExprId) -> Pai {
    let Some(no) = pais.expr.get(&eu).copied() else { return Pai::Outro };
    match no {
        No::Expr(p) => match &a.expr(p).kind {
            ExprKind::Parenthesized(_) => Pai::Parenteses,
            ExprKind::String(_) => Pai::Interpolacao,
            ExprKind::Call { target, arguments } => {
                if *target == eu {
                    Pai::InvocacaoDeFuncao { args_de_tipo: !arguments.type_args.is_empty() }
                } else {
                    papel_nos_argumentos(arguments, eu)
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => papel_nos_argumentos(arguments, eu),
            ExprKind::Property { .. } => {
                // O alvo de `(x).m(…)` é o do `MethodInvocation`.
                if let Some(No::Expr(ch)) = pais.expr.get(&p).copied()
                    && matches!(&a.expr(ch).kind, ExprKind::Call { target, .. } if *target == p)
                    && e_invocacao_de_metodo(s, a, p)
                {
                    Pai::InvocacaoDeMetodo { chamada: ch, nome: p }
                } else {
                    Pai::AcessoDePropriedade(p)
                }
            }
            ExprKind::Index { .. } => Pai::Indice,
            ExprKind::Unary { op: UnaryOp::NullAssert, .. } => Pai::PosfixoExclamacao,
            ExprKind::Unary { op: UnaryOp::PostfixInc | UnaryOp::PostfixDec, .. } => Pai::Posfixo,
            ExprKind::Unary { .. } => Pai::Prefixo,
            ExprKind::Binary { .. } => Pai::Binario,
            ExprKind::Conditional { then, .. } => Pai::Condicional { entao: *then == eu },
            ExprKind::Is { .. } => Pai::E,
            ExprKind::As { .. } => Pai::Como,
            ExprKind::Assign { .. } => Pai::Atribuicao,
            ExprKind::Cascade { .. } => Pai::Cascata,
            ExprKind::Switch { value, .. } => {
                if *value == eu {
                    Pai::SwitchExpressao
                } else {
                    // O corpo e a guarda de um caso.
                    Pai::Outro
                }
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => elements.iter().find_map(|x| papel_na_colecao(s, x, eu, true)).unwrap_or(Pai::Outro),
            ExprKind::Record { named, .. } => {
                if named.iter().any(|(_, x)| *x == eu) {
                    Pai::CampoNomeado
                } else {
                    Pai::OutraExpressao
                }
            }
            ExprKind::TypeArguments { .. } => match pais.expr.get(&p).copied() {
                Some(No::Expr(ch)) if matches!(&a.expr(ch).kind, ExprKind::Call { target, .. } if *target == p) => Pai::InvocacaoDeFuncao { args_de_tipo: true },
                _ => Pai::OutraExpressao,
            },
            _ => Pai::OutraExpressao,
        },
        No::Stmt(st) => match &a.stmt(st).kind {
            StmtKind::Expression(_) => Pai::InstrucaoDeExpressao,
            StmtKind::Return(_) => Pai::Retorno,
            StmtKind::Yield { .. } => Pai::Yield,
            StmtKind::If { condition, .. } if *condition == eu => Pai::CondicaoDoIf,
            StmtKind::While { condition, .. } if *condition == eu => Pai::CondicaoDoWhile,
            StmtKind::DoWhile { condition, .. } if *condition == eu => Pai::CondicaoDoDo,
            StmtKind::Switch { value, .. } if *value == eu => Pai::SwitchInstrucao,
            StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => match l.variables.iter().find(|v| v.initializer == Some(eu)) {
                Some(v) => Pai::Variavel(s.corpo.tipo_local(v.name.span.start)),
                None => Pai::Outro,
            },
            _ => Pai::Outro,
        },
        No::Funcao(f) => match a.function(f).body {
            FunctionBody::Expression(x) if x == eu => Pai::CorpoDeExpressao,
            _ => Pai::Outro,
        },
        No::Membro(m) => match &a.member(m).kind {
            MemberKind::Constructor(k) => {
                for i in k.initializers.iter() {
                    match i {
                        Initializer::Field { value, .. } if *value == eu => return Pai::InicializadorDeCampo,
                        Initializer::Super { arguments, .. } | Initializer::Redirect { arguments, .. } if arguments.args.iter().any(|x| x.value == eu) => {
                            return papel_nos_argumentos(arguments, eu);
                        }
                        _ => {}
                    }
                }
                Pai::Outro
            }
            MemberKind::Field(l) => match l.variables.iter().position(|v| v.initializer == Some(eu)) {
                Some(index) => Pai::Variavel(tipo_de_variavel(s, VariableRef::Field { unit: s.unidade, member: m, index })),
                None => Pai::Outro,
            },
            _ => Pai::Outro,
        },
        No::Decl(d) => match &a.decl(d).kind {
            DeclKind::Variables(l) => match l.variables.iter().position(|v| v.initializer == Some(eu)) {
                Some(index) => Pai::Variavel(tipo_de_variavel(s, VariableRef::TopLevel { unit: s.unidade, decl: d, index })),
                None => Pai::Outro,
            },
            _ => Pai::Outro,
        },
        No::Padrao(p) => match &a.pattern(p).kind {
            PatternKind::Constant(_) => Pai::PadraoConstante,
            _ => Pai::Outro,
        },
        No::Anotacao(sp) | No::ArgumentosDeEnum(sp) => match por_intervalo.get(&(sp.start, sp.end)) {
            Some(args) => papel_nos_argumentos(args, eu),
            None => Pai::Outro,
        },
    }
}

/// `containsNullAwareInvocationInChain`.
fn cadeia_null_aware(s: &super::Semantica<'_>, a: &Ast, e: ExprId) -> bool {
    match &a.expr(e).kind {
        // `PropertyAccess` (o `a.b` simples é `PrefixedIdentifier`).
        ExprKind::Property { target, null_aware, .. } => {
            if *null_aware {
                return true;
            }
            !matches!(a.expr(*target).kind, ExprKind::Identifier(_)) && cadeia_null_aware(s, a, *target)
        }
        ExprKind::Call { target, .. } if e_invocacao_de_metodo(s, a, *target) => match &a.expr(*target).kind {
            ExprKind::Property { target: t, null_aware, .. } => *null_aware || cadeia_null_aware(s, a, *t),
            _ => false,
        },
        ExprKind::Index { target, null_aware, .. } => *null_aware || cadeia_null_aware(s, a, *target),
        _ => false,
    }
}

/// `directlyContainsWhitespace`.
fn tem_espaco(s: &super::Semantica<'_>, a: &Ast, e: ExprId) -> bool {
    match &a.expr(e).kind {
        ExprKind::As { .. } | ExprKind::Assign { .. } | ExprKind::Await(_) | ExprKind::Binary { .. } | ExprKind::Is { .. } => true,
        ExprKind::InstanceCreation { keyword, .. } => keyword.is_some(),
        ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } => *const_,
        ExprKind::Call { target, .. } if e_invocacao_de_metodo(s, a, *target) => match &a.expr(*target).kind {
            ExprKind::Property { target: t, .. } => tem_espaco(s, a, *t),
            _ => false,
        },
        ExprKind::Property { target, .. } => tem_espaco(s, a, *target),
        _ => false,
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let table = s.table;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `unnecessary_statements`.
    if ligada("unnecessary_statements") {
        let mut achados: Vec<Span> = Vec::new();
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Expression(e) => sem_efeito(s, a, *e, &mut achados),
                // `ForPartsWithExpression`: a inicialização e as atualizações.
                StmtKind::For { init, updates, .. } if matches!(init, None | Some(ForInit::Expression(_))) => {
                    if let Some(ForInit::Expression(e)) = init {
                        sem_efeito(s, a, *e, &mut achados);
                    }
                    for x in updates.iter() {
                        sem_efeito(s, a, *x, &mut achados);
                    }
                }
                _ => {}
            }
        }
        // A seção de cascata `..x` que é um `PropertyAccess` de tipo função
        // (o nó começa no `..`/`?..`).
        for e in a.exprs.iter() {
            let ExprKind::Cascade { sections, null_aware, .. } = &e.kind else { continue };
            for (i, sec) in sections.iter().enumerate() {
                if !matches!(a.expr(*sec).kind, ExprKind::Property { .. }) {
                    continue;
                }
                if !s.corpo.get_type(*sec).is_some_and(|t| matches!(table.get(t), Type::Function { .. })) {
                    continue;
                }
                let sp = a.expr(*sec).span;
                let pontos = if i == 0 && *null_aware { 3 } else { 2 };
                achados.push(Span { start: sp.start.saturating_sub(pontos), end: sp.end });
            }
        }
        for sp in achados {
            relatar(&c::UNNECESSARY_STATEMENTS, sp, &[]);
        }
    }

    // `unnecessary_parenthesis`.
    if ligada("unnecessary_parenthesis") {
        let e_parenteses = |e: ExprId| matches!(a.expr(e).kind, ExprKind::Parenthesized(_));
        let pais = pais_da_unidade(u, &e_parenteses);
        let por_intervalo = argumentos_por_intervalo(a);
        let e_registro = |t: Option<dartforge_types::table::TypeId>| t.is_some_and(|t| matches!(table.get(t), Type::Record { .. }));
        for (k, e) in a.exprs.iter().enumerate() {
            let no = ExprId(k as u32);
            let ExprKind::Parenthesized(x) = &e.kind else { continue };
            let x = *x;
            let pai = pai_de(s, a, &pais, &por_intervalo, no);
            if matches!(pai, Pai::PadraoConstante | Pai::Espalhamento) {
                continue;
            }
            let literal_de_registro = matches!(a.expr(x).kind, ExprKind::Record { .. });
            if let Pai::Variavel(t) = pai
                && e_registro(t)
                && !literal_de_registro
            {
                continue;
            }
            // O `staticParameterElement` do argumento (posicional ou nomeado).
            if matches!(pai, Pai::Argumento { .. } | Pai::Nomeado) && e_registro(s.corpo.tipos_de_parametros.get(&no).copied()) && !literal_de_registro {
                continue;
            }
            // Já entre delimitadores: sempre relata.
            if matches!(
                pai,
                Pai::Parenteses
                    | Pai::Interpolacao
                    | Pai::Argumento { n: 1 }
                    | Pai::CondicaoDoIf
                    | Pai::CondicaoDoIfElemento
                    | Pai::CondicaoDoWhile
                    | Pai::CondicaoDoDo
                    | Pai::SwitchInstrucao
                    | Pai::SwitchExpressao
            ) {
                relatar(&c::UNNECESSARY_PARENTHESIS, e.span, &[]);
                continue;
            }
            let interno = &a.expr(x).kind;
            if matches!(interno, ExprKind::Conditional { .. }) {
                continue;
            }
            // O `TypeLiteral` (`List<int>`).
            if let ExprKind::TypeArguments { target, .. } = interno
                && matches!(s.corpo.get_resolved(*target), Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))))
            {
                continue;
            }
            if matches!(interno, ExprKind::Identifier(_)) || cadeia_null_aware(s, a, x) {
                let extensao_anulavel = |nome: ExprId| match s.corpo.get_resolved(nome) {
                    Some(Resolved::ExtensionMember { extension, .. }) => s.outline.extensions.get(extension.0 as usize).is_some_and(|d| anulavel(table, d.on)),
                    _ => false,
                };
                match pai {
                    Pai::AcessoDePropriedade(p) => {
                        let ExprKind::Property { name, .. } = &a.expr(p).kind else { continue };
                        let n = interner.resolve(name.sym);
                        if n == "hashCode" || n == "runtimeType" || extensao_anulavel(p) {
                            continue;
                        }
                    }
                    Pai::InvocacaoDeMetodo { nome, .. } => {
                        let ExprKind::Property { name, .. } = &a.expr(nome).kind else { continue };
                        let n = interner.resolve(name.sym);
                        if n == "noSuchMethod" || n == "toString" || extensao_anulavel(nome) {
                            continue;
                        }
                    }
                    Pai::PosfixoExclamacao => continue,
                    _ => {
                        if matches!(interno, ExprKind::Index { null_aware: true, .. }) && matches!(pai, Pai::Condicional { entao: true } | Pai::ChaveDeMapa) {
                            continue;
                        }
                    }
                }
                relatar(&c::UNNECESSARY_PARENTHESIS, e.span, &[]);
                continue;
            }
            if matches!(interno, ExprKind::FunctionExpression(_)) && matches!(pai, Pai::InvocacaoDeMetodo { .. } | Pai::AcessoDePropriedade(_) | Pai::Binario | Pai::Indice) {
                continue;
            }
            // `ConstructorReference`: o tear-off de construtor.
            let referencia_de_construtor = !matches!(interno, ExprKind::Call { .. } | ExprKind::InstanceCreation { .. }) && matches!(s.corpo.get_resolved(x), Some(Resolved::Constructor(_)));
            if referencia_de_construtor && !matches!(pai, Pai::InvocacaoDeFuncao { args_de_tipo: true }) {
                relatar(&c::UNNECESSARY_PARENTHESIS, e.span, &[]);
                continue;
            }
            let ancestrais: &[No] = pais.ancestrais.get(&no).map(|v| &v[..]).unwrap_or(&[]);
            // `a..b = (c..d)`: a cascata, ou a cascata antes da instrução.
            if matches!(interno, ExprKind::Cascade { .. }) {
                continue;
            }
            let mut em_cascata = false;
            for n in ancestrais.iter().rev() {
                match n {
                    No::Stmt(_) => break,
                    No::Expr(p) if matches!(a.expr(*p).kind, ExprKind::Cascade { .. }) => {
                        em_cascata = true;
                        break;
                    }
                    _ => {}
                }
            }
            if em_cascata {
                continue;
            }
            // `isBareInConstructorFieldInitializer` e uma closure dentro.
            let mut nu = false;
            for n in ancestrais.iter().rev() {
                match n {
                    No::Funcao(_) => break,
                    No::Expr(p) => {
                        if let ExprKind::Call { target, .. } = &a.expr(*p).kind
                            && e_invocacao_de_metodo(s, a, *target)
                        {
                            break;
                        }
                    }
                    No::Membro(m) => {
                        if let MemberKind::Constructor(k) = &a.member(*m).kind {
                            nu = k.initializers.iter().any(|i| matches!(i, Initializer::Field { value, .. } if { let sp = a.expr(*value).span; sp.start <= e.span.start && e.span.end <= sp.end }));
                        }
                        break;
                    }
                    _ => {}
                }
            }
            if nu && a.exprs.iter().any(|y| matches!(y.kind, ExprKind::FunctionExpression(_)) && y.span.start >= e.span.start && y.span.end <= e.span.end) {
                continue;
            }
            if matches!(interno, ExprKind::Binary { op: BinaryOp::Eq | BinaryOp::NotEq, .. })
                && matches!(pai, Pai::Atribuicao | Pai::Variavel(_) | Pai::Retorno | Pai::Yield | Pai::InicializadorDeCampo)
            {
                continue;
            }
            if pai == Pai::InstrucaoDeExpressao && matches!(interno, ExprKind::Switch { .. }) {
                continue;
            }
            let e_argumento = matches!(pai, Pai::Argumento { .. } | Pai::Nomeado);
            if tem_espaco(s, a, x)
                && !matches!(pai, Pai::Atribuicao | Pai::InicializadorDeCampo | Pai::Variavel(_) | Pai::CorpoDeExpressao | Pai::Retorno | Pai::Yield)
                && !e_argumento
            {
                continue;
            }
            if pai.e_expressao() {
                if matches!(pai, Pai::Binario | Pai::Condicional { .. } | Pai::Cascata | Pai::Como | Pai::E) {
                    continue;
                }
                // `PrefixedIdentifier`: `a.b` de identificador simples.
                let identificador_prefixado = matches!(interno, ExprKind::Property { target, null_aware: false, .. } if matches!(a.expr(*target).kind, ExprKind::Identifier(_)));
                if matches!(pai, Pai::InvocacaoDeFuncao { .. }) && !identificador_prefixado {
                    continue;
                }
                let no_do_pai = match pai {
                    Pai::InvocacaoDeMetodo { chamada, .. } => Some(chamada),
                    Pai::AcessoDePropriedade(p) => Some(p),
                    _ => None,
                };
                if let Some(np) = no_do_pai {
                    let avo_prefixo = matches!(pais.expr.get(&np).copied(), Some(No::Expr(g)) if matches!(a.expr(g).kind, ExprKind::Unary { op: UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot | UnaryOp::PrefixInc | UnaryOp::PrefixDec, .. }));
                    if avo_prefixo && tem_espaco(s, a, x) {
                        continue;
                    }
                    if matches!(interno, ExprKind::Unary { .. }) {
                        continue;
                    }
                }
                // `({1, 2}).forEach(print);`: sem os parênteses vira bloco.
                if matches!(interno, ExprKind::SetOrMap { .. }) {
                    let instrucao = ancestrais.iter().rev().find_map(|n| match n {
                        No::Stmt(st) if matches!(a.stmt(*st).kind, StmtKind::Expression(_)) => Some(*st),
                        _ => None,
                    });
                    if instrucao.is_some_and(|st| a.stmt(st).span.start == e.span.start) {
                        continue;
                    }
                }
            }
            relatar(&c::UNNECESSARY_PARENTHESIS, e.span, &[]);
        }
    }

    out
}
