//! O `resolveNameInScope` do linter (`util/scope.dart`) sobre a árvore do
//! DartForge: o escopo do primeiro ancestral com escopo do
//! `ScopeResolverVisitor` e a cadeia dele, por regiões da fonte.
//!
//! Cada declaração local entra com a região onde o nome vale, do mais
//! interno ao mais de fora: os parâmetros de tipo de uma função valem nela
//! inteira; os parâmetros, no corpo; os locais de um bloco (variáveis,
//! declarações por padrão e funções locais), no bloco inteiro (o
//! `LocalScope` guardado no bloco recebe todos); os de um `case`, no
//! `case`; os de um `for`, no comando; os de um `catch`, no corpo; os
//! membros declarados de uma classe (o `InstanceScope`: acessores e
//! métodos, de instância e estáticos), nos métodos e nos corpos de
//! construtor; os parâmetros de tipo de uma classe, na declaração. Em
//! regiões iguais, a ordem desempata (o `TypeParameterScope` de um método
//! fica dentro do `InstanceScope` da classe). O escopo é o primeiro com o
//! nome, como getter ou como setter (o `EnclosedScope.lookup`); um local é
//! só getter.
//!
//! Desvio conhecido: as variáveis de `for` e de `if case` de elementos de
//! coleção e as de casos de `switch` em expressão não entram nas regiões.
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, Ast, DeclId, DeclKind, ForInTarget, ForInit, FunctionBody, MemberKind, PatternId, PatternKind, StmtId, StmtKind};
use dartforge_intern::SymbolId;

/// O que o nome é numa região.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Especie {
    /// Variável, parâmetro ou função local (só getter).
    Local,
    /// Parâmetro de tipo (de função ou de classe).
    ParametroDeTipo,
    /// Membro declarado da classe `classe` (o getter ou o setter).
    Membro { classe: DeclId, setter: bool },
}

/// Uma declaração com a região onde o nome vale.
#[derive(Clone, Copy, Debug)]
pub struct Entrada {
    pub nome: SymbolId,
    pub regiao: Span,
    /// Desempate entre regiões iguais (menor é mais interno).
    pub ordem: u8,
    pub especie: Especie,
    /// O início do nome declarado.
    pub declaracao: usize,
}

/// As regiões da unidade.
pub struct Escopos {
    entradas: Vec<Entrada>,
}

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

fn regiao_do_corpo(a: &Ast, b: &FunctionBody) -> Option<Span> {
    match b {
        FunctionBody::Block(s) => Some(a.stmt(*s).span),
        FunctionBody::Expression(e) => Some(a.expr(*e).span),
        _ => None,
    }
}

/// As variáveis declaradas por um padrão.
pub fn nomes_do_padrao(a: &Ast, p: PatternId) -> Vec<ast::Name> {
    let mut ps = Vec::new();
    super::regras9::variaveis_do_padrao(a, p, &mut ps);
    ps.into_iter()
        .filter_map(|x| match &a.pattern(x).kind {
            PatternKind::Variable { name, .. } => Some(*name),
            _ => None,
        })
        .collect()
}

/// Os locais declarados direto numa lista de comandos.
fn locais_dos_comandos(a: &Ast, comandos: &[StmtId]) -> Vec<ast::Name> {
    let mut v = Vec::new();
    for &s in comandos {
        let mut s = s;
        while let StmtKind::Labeled { body, .. } = &a.stmt(s).kind {
            s = *body;
        }
        match &a.stmt(s).kind {
            StmtKind::Variables(l) => v.extend(l.variables.iter().map(|x| x.name)),
            StmtKind::PatternVariables { pattern, .. } => v.extend(nomes_do_padrao(a, *pattern)),
            StmtKind::Function(f) => v.extend(a.function(*f).name),
            _ => {}
        }
    }
    v
}

impl Escopos {
    pub fn de(a: &Ast) -> Escopos {
        let mut v: Vec<Entrada> = Vec::new();
        let por = |nomes: &mut dyn Iterator<Item = ast::Name>, regiao: Span, ordem: u8, especie: Especie, v: &mut Vec<Entrada>| {
            for n in nomes {
                v.push(Entrada { nome: n.sym, regiao, ordem, especie, declaracao: n.span.start });
            }
        };
        for f in a.functions.iter() {
            por(&mut f.type_params.iter().map(|t| t.name), f.span, 0, Especie::ParametroDeTipo, &mut v);
            if let (Some(ps), Some(corpo)) = (&f.parameters, regiao_do_corpo(a, &f.body)) {
                por(&mut ps.iter().filter_map(|p| p.name), corpo, 0, Especie::Local, &mut v);
            }
        }
        for (k, d) in a.decls.iter().enumerate() {
            let did = DeclId(k as u32);
            let (tps, membros): (&[ast::TypeParameter], &[ast::MemberId]) = match &d.kind {
                DeclKind::Class(x) => (&x.type_params, &x.members),
                DeclKind::Mixin(x) => (&x.type_params, &x.members),
                DeclKind::Enum(x) => (&x.type_params, &x.members),
                DeclKind::Extension(x) => (&x.type_params, &x.members),
                DeclKind::ExtensionType(x) => (&x.type_params, &x.members),
                _ => continue,
            };
            por(&mut tps.iter().map(|t| t.name), d.span, 2, Especie::ParametroDeTipo, &mut v);
            // O `InstanceScope`: getters (campos, getters, métodos, constantes
            // de enum, a representação) e setters (campos que não são
            // `final`/`const`, setters).
            let mut getters: Vec<ast::Name> = Vec::new();
            let mut setters: Vec<ast::Name> = Vec::new();
            for &m in membros {
                match &a.member(m).kind {
                    MemberKind::Field(l) => {
                        getters.extend(l.variables.iter().map(|x| x.name));
                        if !l.final_ && !l.const_ {
                            setters.extend(l.variables.iter().map(|x| x.name));
                        }
                    }
                    MemberKind::Method(f) => {
                        let f = a.function(*f);
                        if let Some(n) = f.name {
                            if f.kind == ast::FunctionKind::Setter {
                                setters.push(n);
                            } else {
                                getters.push(n);
                            }
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
            if let DeclKind::Enum(x) = &d.kind {
                getters.extend(x.constants.iter().map(|k| k.name));
            }
            if let DeclKind::ExtensionType(x) = &d.kind {
                getters.push(x.representation_name);
            }
            for &m in membros {
                let regiao = match &a.member(m).kind {
                    MemberKind::Method(f) => Some(a.function(*f).span),
                    MemberKind::Constructor(k) => regiao_do_corpo(a, &k.body),
                    MemberKind::Field(_) => None,
                };
                if let Some(r) = regiao {
                    por(&mut getters.iter().copied(), r, 1, Especie::Membro { classe: did, setter: false }, &mut v);
                    por(&mut setters.iter().copied(), r, 1, Especie::Membro { classe: did, setter: true }, &mut v);
                }
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind
                && let Some(corpo) = regiao_do_corpo(a, &k.body)
            {
                por(&mut k.parameters.iter().filter_map(|p| p.name), corpo, 0, Especie::Local, &mut v);
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Block(comandos) => por(&mut locais_dos_comandos(a, comandos).into_iter(), s.span, 0, Especie::Local, &mut v),
                StmtKind::Switch { cases, .. } => {
                    for caso in cases.iter() {
                        let mut nomes = locais_dos_comandos(a, &caso.body);
                        if let Some(p) = caso.pattern {
                            nomes.extend(nomes_do_padrao(a, p));
                        }
                        por(&mut nomes.into_iter(), caso.span, 0, Especie::Local, &mut v);
                    }
                }
                StmtKind::Try { catches, .. } => {
                    for k in catches.iter() {
                        por(&mut k.exception.into_iter().chain(k.stack_trace), a.stmt(k.body).span, 0, Especie::Local, &mut v);
                    }
                }
                StmtKind::For { init, .. } => {
                    let nomes: Vec<ast::Name> = match init {
                        Some(ForInit::Variables(l)) => l.variables.iter().map(|x| x.name).collect(),
                        Some(ForInit::Pattern { pattern, .. }) => nomes_do_padrao(a, *pattern),
                        _ => Vec::new(),
                    };
                    por(&mut nomes.into_iter(), s.span, 0, Especie::Local, &mut v);
                }
                StmtKind::ForIn { target, .. } => {
                    let nomes: Vec<ast::Name> = match target {
                        ForInTarget::Declared { name, .. } => vec![*name],
                        ForInTarget::Pattern { pattern, .. } => nomes_do_padrao(a, *pattern),
                        ForInTarget::Expression(_) => Vec::new(),
                    };
                    por(&mut nomes.into_iter(), s.span, 0, Especie::Local, &mut v);
                }
                StmtKind::If { case_pattern: Some(p), then, .. } => {
                    let regiao = Span { start: a.pattern(*p).span.start, end: a.stmt(*then).span.end };
                    por(&mut nomes_do_padrao(a, *p).into_iter(), regiao, 0, Especie::Local, &mut v);
                }
                _ => {}
            }
        }
        Escopos { entradas: v }
    }

    /// As entradas do escopo mais interno em `pos` que tem `nome` (como
    /// getter ou setter); vazio quando nenhum escopo local ou de classe tem.
    pub fn procurar(&self, nome: SymbolId, pos: usize) -> Vec<&Entrada> {
        let ponto = Span { start: pos, end: pos };
        let candidatas: Vec<&Entrada> = self.entradas.iter().filter(|r| r.nome == nome && dentro(ponto, r.regiao)).collect();
        let Some(melhor) = candidatas.iter().map(|r| (r.regiao.end - r.regiao.start, r.ordem)).min() else { return Vec::new() };
        candidatas.into_iter().filter(|r| (r.regiao.end - r.regiao.start, r.ordem) == melhor).collect()
    }
}
