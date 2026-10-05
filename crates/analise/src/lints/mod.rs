//! As tabelas das regras de lint do SDK 3.6.2
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): o registro das 240 regras
//! ([`tabela_g`]) e os códigos que elas relatam ([`codigos_g`]), geradas por
//! `scripts/gerar-tabelas-analise.py`. As regras implementadas estão em
//! [`regras`] a [`regras10`] (oitenta, as que só olham a árvore e o texto; [`andar`] é o passeio com ancestrais e [`cordas`] relê os literais de string e os comentários); as tabelas servem também à
//! validação de `linter: rules:` do `analysis_options.yaml`
//! (`crate::naodart::opcoes`).
//!
//! Escrito sem compilar nem executar (2026-10-05).

pub mod andar;
pub mod codigos_g;
pub mod cordas;
pub mod regras;
pub mod regras2;
pub mod regras3;
pub mod regras4;
pub mod regras5;
pub mod regras6;
pub mod regras7;
pub mod regras8;
pub mod regras10;
pub mod regras11;
pub mod regras12;
pub mod regras13;
pub mod regras9;
pub mod tabela_g;

/// O `state:` de uma regra (`linter/lib/src/analyzer.dart`, `State`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDaRegra {
    Estavel,
    Experimental,
    /// Só para o próprio SDK.
    Interna,
    Depreciada,
    Removida,
}

/// O conjunto do `package:lints` que liga a regra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conjunto {
    Nenhum,
    /// `package:lints/core.yaml` (e portanto também o `recommended`).
    Core,
    /// Só `package:lints/recommended.yaml`.
    Recommended,
}

/// Uma regra registrada.
#[derive(Debug, PartialEq, Eq)]
pub struct InfoRegra {
    pub nome: &'static str,
    pub estado: EstadoDaRegra,
    /// A versão do `since:` do estado, quando há.
    pub desde: Option<(u32, u32, u32)>,
    /// `incompatibleRules`.
    pub incompativeis: &'static [&'static str],
    pub conjunto: Conjunto,
    /// O arquivo da regra em `linter/lib/src/`.
    pub arquivo: &'static str,
}

/// Um código de lint (`LinterLintCode`).
#[derive(Debug, PartialEq, Eq)]
pub struct CodigoLint {
    /// O nome relatado (o da regra).
    pub nome: &'static str,
    /// `uniqueName`.
    pub unico: &'static str,
    pub mensagem: &'static str,
    pub correcao: Option<&'static str>,
    pub documentado: bool,
}

/// Roda todas as regras implementadas que estão ligadas (`ligada(nome)`)
/// sobre uma unidade; os relatos saem em ordem de posição.
pub fn executar(
    u: crate::Unidade<'_>,
    interner: &dartforge_intern::Interner,
    ligada: &dyn Fn(&str) -> bool,
) -> Vec<regras::RelatoDeLint> {
    executar_com(u, interner, ligada, None)
}

/// A semântica de uma unidade cuja árvore é a do programa resolvido: o que
/// as regras que o original decide pelo elemento ou pelo tipo (e não pelo
/// nome) leem. Sem ela, essas regras não relatam.
#[derive(Clone, Copy)]
pub struct Semantica<'a> {
    pub program: &'a dartforge_elements::model::Program,
    pub unidade: dartforge_elements::model::UnitId,
    pub corpo: &'a dartforge_types::resolved::UnitBodyTypes,
    /// Os corpos de todas as unidades (as regras que olham a biblioteca
    /// inteira).
    pub corpos: &'a dartforge_types::resolved::BodyTypes,
    pub table: &'a dartforge_types::table::TypeTable,
    pub core: &'a dartforge_types::table::CoreTypes,
    pub outline: &'a dartforge_types::resolve::OutlineTypes,
}

/// `FunctionBody.isPotentiallyMutatedInScope` do local declarado em
/// `declaracao` (o deslocamento do nome): o conjunto do `ScopeResolverVisitor`
/// é um só por unidade, então vale qualquer escrita ao elemento na unidade —
/// o lado esquerdo de uma atribuição (de qualquer operador), o operando de
/// `++`/`--`, a variável de `for (x in …)` e a variável de um padrão de
/// atribuição.
pub fn mutado(sem: &Semantica<'_>, a: &dartforge_frontend::ast::Ast, declaracao: usize) -> bool {
    use dartforge_frontend::ast::{CollectionElement, ExprKind, ForInTarget, StmtKind, UnaryOp};
    let e_ele = |x: dartforge_frontend::ast::ExprId| sem.corpo.declaracao_local(x) == Some(declaracao);
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Assign { target, .. } if e_ele(*target) => return true,
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } if e_ele(*operand) => {
                return true;
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                fn de_colecao(el: &CollectionElement, e_ele: &dyn Fn(dartforge_frontend::ast::ExprId) -> bool) -> bool {
                    match el {
                        CollectionElement::ForIn { target, body, .. } => {
                            matches!(target, ForInTarget::Expression(x) if e_ele(*x)) || de_colecao(body, e_ele)
                        }
                        CollectionElement::For { body, .. } => de_colecao(body, e_ele),
                        CollectionElement::If { then, else_, .. } => de_colecao(then, e_ele) || else_.as_ref().is_some_and(|x| de_colecao(x, e_ele)),
                        _ => false,
                    }
                }
                if elements.iter().any(|el| de_colecao(el, &e_ele)) {
                    return true;
                }
            }
            _ => {}
        }
    }
    if a.stmts.iter().any(|s| matches!(&s.kind, StmtKind::ForIn { target: ForInTarget::Expression(x), .. } if e_ele(*x))) {
        return true;
    }
    sem.corpo.declaracoes_de_padroes.values().any(|d| *d == declaracao)
}

/// O `canonicalElement` de um elemento resolvido: o local pela declaração;
/// o acessor (implícito, ou o par getter/setter explícito) pela variável ou
/// pela propriedade sintética que o analyzer cria para ele; o resto pelo
/// próprio elemento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Canonico {
    Local(usize),
    Parametro(dartforge_intern::SymbolId),
    Variavel(dartforge_elements::model::VariableId),
    /// O par `get x`/`set x` explícito: o dono (classe, extensão ou
    /// biblioteca) e o nome sem `=`.
    Propriedade(Dono, dartforge_intern::SymbolId),
    Funcao(dartforge_elements::model::FunctionElementId),
    Elemento(dartforge_elements::model::Element),
    ParametroDeTipo(dartforge_types::table::TypeParamId),
    Prefixo(dartforge_elements::model::LibraryId),
}

/// O dono de uma propriedade sintética.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dono {
    Classe(dartforge_elements::model::ClassId),
    Extensao(dartforge_elements::model::ExtensionId),
    Biblioteca(dartforge_elements::model::LibraryId),
}

/// O canônico da função `f` (o acessor vale pela variável/propriedade).
fn canonico_da_funcao(sem: &Semantica<'_>, interner: &dartforge_intern::Interner, f: dartforge_elements::model::FunctionElementId) -> Canonico {
    use dartforge_elements::model::FunctionKind;
    let g = sem.program.function(f);
    if let Some(v) = g.variable {
        return Canonico::Variavel(v);
    }
    if matches!(g.kind, FunctionKind::Getter | FunctionKind::Setter) {
        let texto = interner.resolve(g.name);
        let base = texto.strip_suffix("_=").or_else(|| texto.strip_suffix('=')).unwrap_or(texto);
        let nome = interner.lookup(base).unwrap_or(g.name);
        let dono = match (g.class, g.extension) {
            (Some(c), _) => Dono::Classe(c),
            (None, Some(x)) => Dono::Extensao(x),
            (None, None) => Dono::Biblioteca(g.library),
        };
        return Canonico::Propriedade(dono, nome);
    }
    Canonico::Funcao(f)
}

/// `node.staticElement?.canonicalElement` (ou o de escrita, no lado
/// esquerdo de uma atribuição: `getWriteOrReadElement`) da expressão `e`.
pub fn canonico(sem: &Semantica<'_>, interner: &dartforge_intern::Interner, e: dartforge_frontend::ast::ExprId) -> Option<Canonico> {
    use dartforge_elements::model::Element;
    use dartforge_types::resolved::{MemberRef, Resolved};
    Some(match sem.corpo.get_resolved(e)? {
        Resolved::Local(_) => Canonico::Local(sem.corpo.declaracao_local(e)?),
        Resolved::Parameter { name, .. } => Canonico::Parametro(*name),
        Resolved::TypeParameter(p) => Canonico::ParametroDeTipo(*p),
        Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => Canonico::Variavel(*v),
        Resolved::Element(Element::Function(f))
        | Resolved::Member { member: MemberRef::Function(f), .. }
        | Resolved::ExtensionMember { member: f, .. }
        | Resolved::Constructor(f) => canonico_da_funcao(sem, interner, *f),
        Resolved::Element(x) => Canonico::Elemento(*x),
        Resolved::Prefix(lib) => Canonico::Prefixo(*lib),
        Resolved::Dynamic => return None,
    })
}

/// `canonicalElementsFromIdentifiersAreEqual`: os dois são identificadores
/// simples, prefixados ou acessos de propriedade (sem os parênteses) com os
/// mesmos elementos canônicos (e os mesmos prefixos/alvos). Dois elementos
/// desconhecidos são iguais (`null == null`).
pub fn mesmos_elementos(
    sem: &Semantica<'_>,
    interner: &dartforge_intern::Interner,
    a: &dartforge_frontend::ast::Ast,
    x: Option<dartforge_frontend::ast::ExprId>,
    y: Option<dartforge_frontend::ast::ExprId>,
) -> bool {
    use dartforge_frontend::ast::ExprKind;
    let (Some(x), Some(y)) = (x, y) else { return false };
    let sem_parenteses = |mut e: dartforge_frontend::ast::ExprId| {
        while let ExprKind::Parenthesized(i) = &a.expr(e).kind {
            e = *i;
        }
        e
    };
    let (x, y) = (sem_parenteses(x), sem_parenteses(y));
    // O `PrefixedIdentifier`: `p.x` com `p` identificador simples, sem `?.`.
    let prefixado = |e: dartforge_frontend::ast::ExprId| match &a.expr(e).kind {
        ExprKind::Property { target, null_aware: false, .. } => matches!(a.expr(*target).kind, ExprKind::Identifier(_)).then_some(*target),
        _ => None,
    };
    match (&a.expr(x).kind, &a.expr(y).kind) {
        (ExprKind::Identifier(_), ExprKind::Identifier(_)) => canonico(sem, interner, x) == canonico(sem, interner, y),
        (ExprKind::Identifier(_), _) | (_, ExprKind::Identifier(_)) => false,
        (ExprKind::Property { target: t1, .. }, ExprKind::Property { target: t2, .. }) => match (prefixado(x), prefixado(y)) {
            (Some(p1), Some(p2)) => canonico(sem, interner, p1) == canonico(sem, interner, p2) && canonico(sem, interner, x) == canonico(sem, interner, y),
            (Some(_), None) | (None, Some(_)) => false,
            // `PropertyAccess`: os alvos pela mesma regra, e a propriedade.
            (None, None) => mesmos_elementos(sem, interner, a, Some(*t1), Some(*t2)) && canonico(sem, interner, x) == canonico(sem, interner, y),
        },
        _ => false,
    }
}

/// `TypeSystem.isNullable`: `dynamic`, `void`, `Null`, o tipo inválido ou
/// desconhecido, o tipo com `?`, o `FutureOr` de anulável e o parâmetro de
/// tipo promovido a anulável.
pub fn anulavel(sem: &Semantica<'_>, t: dartforge_types::table::TypeId) -> bool {
    use dartforge_types::table::Type;
    if sem.core.is_unknown(sem.table, t) {
        return true;
    }
    match sem.table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => anulavel(sem, *bound),
        Type::FutureOr { arg, nullable } => *nullable || anulavel(sem, *arg),
        outro => outro.is_declared_nullable(),
    }
}

/// O tipo do elemento da variável de topo ou do campo `v`: o escrito, ou o
/// inferido (que a inferência dos corpos completa).
pub fn tipo_da_variavel(sem: &Semantica<'_>, v: dartforge_elements::model::VariableId) -> Option<dartforge_types::table::TypeId> {
    let d = sem.outline.variables.get(v.0 as usize)?;
    d.declared_type.or(d.inferred)
}

/// A classe (classe, enum ou tipo de extensão) que declara o membro `m` da
/// unidade.
pub fn classe_do_membro(sem: &Semantica<'_>, m: dartforge_frontend::ast::MemberId) -> Option<dartforge_elements::model::ClassId> {
    use dartforge_frontend::ast::DeclKind;
    let p = sem.program;
    let a = &p.unit(sem.unidade).ast;
    let (di, _) = a.decls.iter().enumerate().find(|(_, d)| match &d.kind {
        DeclKind::Class(x) => x.members.contains(&m),
        DeclKind::Enum(x) => x.members.contains(&m),
        DeclKind::ExtensionType(x) => x.members.contains(&m),
        DeclKind::Mixin(x) => x.members.contains(&m),
        _ => false,
    })?;
    let i = p.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == sem.unidade && r.decl.0 as usize == di))?;
    Some(dartforge_elements::model::ClassId(i as u32))
}

/// `FieldFormalParameterElement.field`: o campo de nome `nome` declarado na
/// classe do construtor `m`.
pub fn campo_da_classe(sem: &Semantica<'_>, m: dartforge_frontend::ast::MemberId, nome: dartforge_intern::SymbolId) -> Option<dartforge_elements::model::VariableId> {
    let c = classe_do_membro(sem, m)?;
    sem.program.class(c).fields.iter().copied().find(|v| sem.program.variable(*v).name == nome)
}

/// O `superConstructorParameter` do parâmetro `super.x` de índice `i` do
/// construtor `m` da unidade: o construtor da superclasse chamado (pelo
/// `super(…)`/`super.nome(…)` dos inicializadores, ou o sem nome) e o
/// parâmetro dele (o posicional pela ordem entre os `super.` posicionais, o
/// nomeado pelo nome).
pub fn parametro_do_super(
    sem: &Semantica<'_>,
    interner: &dartforge_intern::Interner,
    m: dartforge_frontend::ast::MemberId,
    i: usize,
) -> Option<(dartforge_elements::model::FunctionElementId, usize)> {
    use dartforge_frontend::ast::{Initializer, MemberKind, ParameterKind};
    let p = sem.program;
    let a = &p.unit(sem.unidade).ast;
    let MemberKind::Constructor(k) = &a.member(m).kind else { return None };
    let sup = p.class(classe_do_membro(sem, m)?).supertype_class?;
    let nome = k
        .initializers
        .iter()
        .find_map(|x| match x {
            Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
            _ => None,
        })
        .unwrap_or(None)
        .or_else(|| interner.lookup(""))?;
    let ctor = *p.class(sup).constructors.get(&nome)?;
    let dados = sem.outline.functions.get(ctor.0 as usize)?;
    let alvo = k.parameters.get(i)?;
    if !alvo.super_ {
        return None;
    }
    match alvo.kind {
        ParameterKind::Named => {
            let n = alvo.name?.sym;
            let j = dados.parameters.iter().position(|x| x.kind == ParameterKind::Named && x.externo.or(x.name) == Some(n))?;
            Some((ctor, j))
        }
        _ => {
            let ordem = k.parameters[..i].iter().filter(|x| x.super_ && x.kind != ParameterKind::Named).count();
            let j = dados
                .parameters
                .iter()
                .enumerate()
                .filter(|(_, x)| x.kind != ParameterKind::Named)
                .nth(ordem)
                .map(|(j, _)| j)?;
            Some((ctor, j))
        }
    }
}

/// `TypeAnnotation.type` da anotação `t` da unidade: a do outline
/// (assinaturas) ou a dos corpos.
pub fn tipo_escrito(sem: &Semantica<'_>, t: dartforge_frontend::ast::TypeId) -> Option<dartforge_types::table::TypeId> {
    sem.outline.tipos_escritos.get(&(sem.unidade, t)).copied().or_else(|| sem.corpo.tipos_de_anotacoes.get(&t).copied())
}

/// O tipo de retorno do elemento da função `f` da unidade
/// (`ExecutableElement.returnType`): o inferido da função literal ou local,
/// ou o do outline (escrito, ou herdado da sobrescrita).
pub fn retorno_da_funcao(sem: &Semantica<'_>, f: dartforge_frontend::ast::FunctionId) -> Option<dartforge_types::table::TypeId> {
    if let Some(t) = sem.corpo.tipo_de_execucao_de_funcao(f)
        && let dartforge_types::table::Type::Function { ret, .. } = sem.table.get(t)
    {
        return Some(*ret);
    }
    let i = sem.program.functions.iter().position(|e| {
        matches!(e.node, dartforge_elements::model::FunctionRef::Function { unit, function } if unit == sem.unidade && function == f)
    })?;
    sem.outline.functions.get(i).map(|d| d.return_type)
}

/// A biblioteca da unidade tem a versão de linguagem `maior.menor` ou mais.
pub fn versao_ao_menos(sem: &Semantica<'_>, maior: u32, menor: u32) -> bool {
    let p = sem.program;
    p.library(p.unit(sem.unidade).library).features.versao() >= dartforge_frontend::features::LanguageVersion::new(maior, menor)
}

/// `LinterContext.isInTestDirectory`: a unidade que define a biblioteca da
/// unidade está no `test/` do pacote pub dela (a pasta do `pubspec.yaml`
/// mais próximo acima do arquivo; sem um, não há pacote).
pub fn em_teste_do_pacote(sem: &Semantica<'_>) -> bool {
    let p = sem.program;
    let lib = p.library(p.unit(sem.unidade).library);
    let Some(caminho) = lib.units.first().and_then(|d| p.unit(*d).path.as_ref()) else { return false };
    let Some(raiz) = caminho.ancestors().skip(1).find(|d| d.join("pubspec.yaml").is_file()) else { return false };
    caminho.starts_with(raiz.join("test"))
}

/// Como [`executar`], com a semântica da unidade (`u` tem de ser a árvore
/// do programa de `sem`, com o `interner` dele).
pub fn executar_com(
    u: crate::Unidade<'_>,
    interner: &dartforge_intern::Interner,
    ligada: &dyn Fn(&str) -> bool,
    sem: Option<&Semantica<'_>>,
) -> Vec<regras::RelatoDeLint> {
    let mut out = regras::executar(u, interner, ligada, sem);
    out.extend(regras2::executar(u, interner, ligada, sem));
    out.extend(regras3::executar(u, interner, ligada, sem));
    out.extend(regras4::executar(u, interner, ligada, sem));
    out.extend(regras5::executar(u, interner, ligada, sem));
    out.extend(regras6::executar(u, interner, ligada, sem));
    out.extend(regras7::executar(u, interner, ligada, sem));
    out.extend(regras8::executar(u, interner, ligada, sem));
    out.extend(regras9::executar(u, interner, ligada, sem));
    out.extend(regras10::executar(u, interner, ligada, sem));
    out.extend(regras11::executar(u, interner, ligada, sem));
    out.extend(regras12::executar(u, interner, ligada, sem));
    out.extend(regras13::executar(u, interner, ligada, sem));
    out.sort_by_key(|r| (r.span.start, r.span.end));
    out
}

/// A regra registrada de nome `nome`.
pub fn regra(nome: &str) -> Option<&'static InfoRegra> {
    tabela_g::REGRAS.iter().find(|r| r.nome == nome)
}
