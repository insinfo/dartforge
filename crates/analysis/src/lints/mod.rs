//! As tabelas das regras de lint do SDK 3.6.2
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): o registro das 240 regras
//! ([`tabela_g`]) e os códigos que elas relatam ([`codigos_g`]), geradas por
//! `scripts/gerar-tabelas-analise.py`. As regras implementadas estão em
//! [`rules`] a [`rules10`] (oitenta, as que só olham a árvore e o texto; [`walk`] é o passeio com ancestrais e [`strings`] relê os literais de string e os comentários); as tabelas servem também à
//! validação de `linter: rules:` do `analysis_options.yaml`
//! (`crate::naodart::options`).
//!
//! Escrito sem compilar nem executar (2026-10-05).

pub mod walk;
pub mod codigos_g;
pub mod strings;
pub mod scope;
pub mod rules;
pub mod rules2;
pub mod rules3;
pub mod rules4;
pub mod rules5;
pub mod rules6;
pub mod rules7;
pub mod rules8;
pub mod rules10;
pub mod rules11;
pub mod rules12;
pub mod rules13;
pub mod rules14;
pub mod rules15;
pub mod rules16;
pub mod rules17;
pub mod rules18;
pub mod rules19;
pub mod rules20;
pub mod rules21;
pub mod rules22;
pub mod rules23;
pub mod rules24;
pub mod rules25;
pub mod rules26;
pub mod rules27;
pub mod rules28;
pub mod rules29;
pub mod rules30;
pub mod rules31;
pub mod rules32;
pub mod rules33;
pub mod obvious;
pub mod flutter;
pub mod pubspec;
pub mod rules9;
pub mod regexp_unicode_g;
pub mod regexp_vm;
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
) -> Vec<rules::LintReport> {
    run_with(u, interner, ligada, None)
}

/// A semântica de uma unidade cuja árvore é a do programa resolvido: o que
/// as regras que o original decide pelo elemento ou pelo tipo (e não pelo
/// nome) leem. Sem ela, essas regras não relatam.
#[derive(Clone, Copy)]
pub struct Semantics<'a> {
    pub program: &'a dartforge_elements::model::Program,
    pub unit: dartforge_elements::model::UnitId,
    pub body: &'a dartforge_types::resolved::UnitBodyTypes,
    /// Os corpos de todas as unidades (as regras que olham a biblioteca
    /// inteira).
    pub bodies: &'a dartforge_types::resolved::BodyTypes,
    pub table: &'a dartforge_types::table::TypeTable,
    pub core: &'a dartforge_types::table::CoreTypes,
    pub outline: &'a dartforge_types::resolve::OutlineTypes,
    /// O `canBeConst` das criações, literais e construtores da unidade
    /// (`dartforge_types::lints_tipados6`), quando calculado.
    pub can_be_const: Option<&'a dartforge_types::lints_tipados6::PodemSerConst>,
}

/// `FunctionBody.isPotentiallyMutatedInScope` do local declarado em
/// `declaracao` (o deslocamento do nome): o conjunto do `ScopeResolverVisitor`
/// é um só por unidade, então vale qualquer escrita ao elemento na unidade —
/// o lado esquerdo de uma atribuição (de qualquer operador), o operando de
/// `++`/`--`, a variável de `for (x in …)` e a variável de um padrão de
/// atribuição.
pub fn is_mutated(sem: &Semantics<'_>, a: &dartforge_frontend::ast::Ast, declaracao: usize) -> bool {
    use dartforge_frontend::ast::{CollectionElement, ExprKind, ForInTarget, StmtKind, UnaryOp};
    let e_ele = |x: dartforge_frontend::ast::ExprId| sem.body.declaracao_local(x) == Some(declaracao);
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
    sem.body.declaracoes_de_padroes.values().any(|d| *d == declaracao)
}

/// O `canonicalElement` de um elemento resolvido: o local pela declaração;
/// o acessor (implícito, ou o par getter/setter explícito) pela variável ou
/// pela propriedade sintética que o analyzer cria para ele; o resto pelo
/// próprio elemento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Canonical {
    Local(usize),
    Parameter(dartforge_intern::SymbolId),
    Variable(dartforge_elements::model::VariableId),
    /// O par `get x`/`set x` explícito: o dono (classe, extensão ou
    /// biblioteca) e o nome sem `=`.
    Property(PropertyOwner, dartforge_intern::SymbolId),
    Function(dartforge_elements::model::FunctionElementId),
    Element(dartforge_elements::model::Element),
    TypeParameter(dartforge_types::table::TypeParamId),
    Prefix(dartforge_elements::model::LibraryId),
}

/// O dono de uma propriedade sintética.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyOwner {
    Class(dartforge_elements::model::ClassId),
    Extension(dartforge_elements::model::ExtensionId),
    Library(dartforge_elements::model::LibraryId),
}

/// O canônico da função `f` (o acessor vale pela variável/propriedade).
fn function_canonical(sem: &Semantics<'_>, interner: &dartforge_intern::Interner, f: dartforge_elements::model::FunctionElementId) -> Canonical {
    use dartforge_elements::model::FunctionKind;
    let g = sem.program.function(f);
    if let Some(v) = g.variable {
        return Canonical::Variable(v);
    }
    if matches!(g.kind, FunctionKind::Getter | FunctionKind::Setter) {
        let texto = interner.resolve(g.name);
        let base = texto.strip_suffix("_=").or_else(|| texto.strip_suffix('=')).unwrap_or(texto);
        let nome = interner.lookup(base).unwrap_or(g.name);
        let dono = match (g.class, g.extension) {
            (Some(c), _) => PropertyOwner::Class(c),
            (None, Some(x)) => PropertyOwner::Extension(x),
            (None, None) => PropertyOwner::Library(g.library),
        };
        return Canonical::Property(dono, nome);
    }
    Canonical::Function(f)
}

/// `node.staticElement?.canonicalElement` (ou o de escrita, no lado
/// esquerdo de uma atribuição: `getWriteOrReadElement`) da expressão `e`.
pub fn canonical(sem: &Semantics<'_>, interner: &dartforge_intern::Interner, e: dartforge_frontend::ast::ExprId) -> Option<Canonical> {
    use dartforge_elements::model::Element;
    use dartforge_types::resolved::{MemberRef, Resolved};
    Some(match sem.body.get_resolved(e)? {
        Resolved::Local(_) => Canonical::Local(sem.body.declaracao_local(e)?),
        Resolved::Parameter { name, .. } => Canonical::Parameter(*name),
        Resolved::TypeParameter(p) => Canonical::TypeParameter(*p),
        Resolved::Element(Element::Variable(v)) | Resolved::Member { member: MemberRef::Variable(v), .. } => Canonical::Variable(*v),
        Resolved::Element(Element::Function(f))
        | Resolved::Member { member: MemberRef::Function(f), .. }
        | Resolved::ExtensionMember { member: f, .. }
        | Resolved::Constructor(f) => function_canonical(sem, interner, *f),
        Resolved::Element(x) => Canonical::Element(*x),
        Resolved::Prefix(lib) => Canonical::Prefix(*lib),
        Resolved::Dynamic => return None,
    })
}

/// `canonicalElementsFromIdentifiersAreEqual`: os dois são identificadores
/// simples, prefixados ou acessos de propriedade (sem os parênteses) com os
/// mesmos elementos canônicos (e os mesmos prefixos/alvos). Dois elementos
/// desconhecidos são iguais (`null == null`).
pub fn same_elements(
    sem: &Semantics<'_>,
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
        (ExprKind::Identifier(_), ExprKind::Identifier(_)) => canonical(sem, interner, x) == canonical(sem, interner, y),
        (ExprKind::Identifier(_), _) | (_, ExprKind::Identifier(_)) => false,
        (ExprKind::Property { target: t1, .. }, ExprKind::Property { target: t2, .. }) => match (prefixado(x), prefixado(y)) {
            (Some(p1), Some(p2)) => canonical(sem, interner, p1) == canonical(sem, interner, p2) && canonical(sem, interner, x) == canonical(sem, interner, y),
            (Some(_), None) | (None, Some(_)) => false,
            // `PropertyAccess`: os alvos pela mesma regra, e a propriedade.
            (None, None) => same_elements(sem, interner, a, Some(*t1), Some(*t2)) && canonical(sem, interner, x) == canonical(sem, interner, y),
        },
        _ => false,
    }
}

/// `TypeSystem.isNullable`: `dynamic`, `void`, `Null`, o tipo inválido ou
/// desconhecido, o tipo com `?`, o `FutureOr` de anulável e o parâmetro de
/// tipo promovido a anulável.
pub fn anulavel(sem: &Semantics<'_>, t: dartforge_types::table::TypeId) -> bool {
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
pub fn variable_type(sem: &Semantics<'_>, v: dartforge_elements::model::VariableId) -> Option<dartforge_types::table::TypeId> {
    let d = sem.outline.variables.get(v.0 as usize)?;
    d.declared_type.or(d.inferred)
}

/// O elemento da função `f` da árvore da unidade (de topo, método,
/// acessor).
pub fn function_element(sem: &Semantics<'_>, f: dartforge_frontend::ast::FunctionId) -> Option<dartforge_elements::model::FunctionElementId> {
    let i = sem.program.functions.iter().position(|e| {
        matches!(e.node, dartforge_elements::model::FunctionRef::Function { unit, function } if unit == sem.unit && function == f)
    })?;
    Some(dartforge_elements::model::FunctionElementId(i as u32))
}

/// A cadeia de implementação de `c` (`_implementationsOfGetter`): a classe,
/// os mixins do último ao primeiro, e assim pela cadeia de superclasses.
/// Com `setter_herdado`, há um setter `nome=` concreto, de instância e
/// acessível numa classe da cadeia que não `c`
/// (`lookUpInheritedConcreteSetter`); sem, há um getter `nome` acessível em
/// alguma da cadeia (`lookUpGetter`, também estático ou abstrato).
pub fn lookup_in_chain(
    sem: &Semantics<'_>,
    interner: &dartforge_intern::Interner,
    c: dartforge_elements::model::ClassId,
    nome: dartforge_intern::SymbolId,
    setter_herdado: bool,
) -> bool {
    use dartforge_elements::model::FunctionKind;
    let p = sem.program;
    let biblioteca = p.class(c).library;
    let privado = interner.resolve(nome).starts_with('_');
    let do_setter = interner.lookup(&format!("{}_=", interner.resolve(nome)));
    let mut cadeia: Vec<dartforge_elements::model::ClassId> = Vec::new();
    let mut vistos = std::collections::HashSet::new();
    let mut atual = Some(c);
    while let Some(k) = atual {
        if !vistos.insert(k) {
            break;
        }
        cadeia.push(k);
        cadeia.extend(p.class(k).mixin_classes.iter().rev().copied());
        atual = p.class(k).supertype_class;
    }
    cadeia.into_iter().any(|k| {
        let e = p.class(k);
        if privado && e.library != biblioteca {
            return false;
        }
        if setter_herdado {
            k != c && do_setter.and_then(|x| e.instance_members.get(&x)).is_some_and(|f| !p.function(*f).abstract_ && !p.function(*f).static_)
        } else {
            e.instance_members.get(&nome).or_else(|| e.static_members.get(&nome)).is_some_and(|f| {
                matches!(p.function(*f).kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor)
            })
        }
    })
}

/// A classe (classe, enum ou tipo de extensão) que declara o membro `m` da
/// unidade.
pub fn member_class(sem: &Semantics<'_>, m: dartforge_frontend::ast::MemberId) -> Option<dartforge_elements::model::ClassId> {
    use dartforge_frontend::ast::DeclKind;
    let p = sem.program;
    let a = &p.unit(sem.unit).ast;
    let (di, _) = a.decls.iter().enumerate().find(|(_, d)| match &d.kind {
        DeclKind::Class(x) => x.members.contains(&m),
        DeclKind::Enum(x) => x.members.contains(&m),
        DeclKind::ExtensionType(x) => x.members.contains(&m),
        DeclKind::Mixin(x) => x.members.contains(&m),
        _ => false,
    })?;
    let i = p.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == sem.unit && r.decl.0 as usize == di))?;
    Some(dartforge_elements::model::ClassId(i as u32))
}

/// `FieldFormalParameterElement.field`: o campo de nome `nome` declarado na
/// classe do construtor `m`.
pub fn class_field(sem: &Semantics<'_>, m: dartforge_frontend::ast::MemberId, nome: dartforge_intern::SymbolId) -> Option<dartforge_elements::model::VariableId> {
    let c = member_class(sem, m)?;
    sem.program.class(c).fields.iter().copied().find(|v| sem.program.variable(*v).name == nome)
}

/// O `superConstructorParameter` do parâmetro `super.x` de índice `i` do
/// construtor `m` da unidade: o construtor da superclasse chamado (pelo
/// `super(…)`/`super.nome(…)` dos inicializadores, ou o sem nome) e o
/// parâmetro dele (o posicional pela ordem entre os `super.` posicionais, o
/// nomeado pelo nome).
pub fn super_parameter(
    sem: &Semantics<'_>,
    interner: &dartforge_intern::Interner,
    m: dartforge_frontend::ast::MemberId,
    i: usize,
) -> Option<(dartforge_elements::model::FunctionElementId, usize)> {
    use dartforge_frontend::ast::{Initializer, MemberKind, ParameterKind};
    let p = sem.program;
    let a = &p.unit(sem.unit).ast;
    let MemberKind::Constructor(k) = &a.member(m).kind else { return None };
    let sup = p.class(member_class(sem, m)?).supertype_class?;
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
pub fn written_type(sem: &Semantics<'_>, t: dartforge_frontend::ast::TypeId) -> Option<dartforge_types::table::TypeId> {
    sem.outline.tipos_escritos.get(&(sem.unit, t)).copied().or_else(|| sem.body.tipos_de_anotacoes.get(&t).copied())
}

/// O tipo de retorno do elemento da função `f` da unidade
/// (`ExecutableElement.returnType`): o inferido da função literal ou local,
/// ou o do outline (escrito, ou herdado da sobrescrita).
pub fn function_return(sem: &Semantics<'_>, f: dartforge_frontend::ast::FunctionId) -> Option<dartforge_types::table::TypeId> {
    if let Some(t) = sem.body.tipo_de_execucao_de_funcao(f)
        && let dartforge_types::table::Type::Function { ret, .. } = sem.table.get(t)
    {
        return Some(*ret);
    }
    let i = sem.program.functions.iter().position(|e| {
        matches!(e.node, dartforge_elements::model::FunctionRef::Function { unit, function } if unit == sem.unit && function == f)
    })?;
    sem.outline.functions.get(i).map(|d| d.return_type)
}

/// A biblioteca da unidade tem a versão de linguagem `maior.menor` ou mais.
pub fn version_at_least(sem: &Semantics<'_>, maior: u16, menor: u16) -> bool {
    let p = sem.program;
    p.library(p.unit(sem.unit).library).features.versao() >= dartforge_frontend::features::LanguageVersion::new(maior, menor)
}

/// `LinterContext.isInTestDirectory`: a unidade que define a biblioteca da
/// unidade está no `test/` do pacote pub dela (a pasta do `pubspec.yaml`
/// mais próximo acima do arquivo; sem um, não há pacote).
pub fn in_package_test(sem: &Semantics<'_>) -> bool {
    let p = sem.program;
    let lib = p.library(p.unit(sem.unit).library);
    let Some(caminho) = lib.units.first().and_then(|d| p.unit(*d).path.as_ref()) else { return false };
    let Some(raiz) = caminho.ancestors().skip(1).find(|d| d.join("pubspec.yaml").is_file()) else { return false };
    caminho.starts_with(raiz.join("test"))
}

/// Como [`executar`], com a semântica da unidade (`u` tem de ser a árvore
/// do programa de `sem`, com o `interner` dele).
pub fn run_with(
    u: crate::Unidade<'_>,
    interner: &dartforge_intern::Interner,
    ligada: &dyn Fn(&str) -> bool,
    sem: Option<&Semantics<'_>>,
) -> Vec<rules::LintReport> {
    let mut out = rules::executar(u, interner, ligada, sem);
    out.extend(rules2::executar(u, interner, ligada, sem));
    out.extend(rules3::executar(u, interner, ligada, sem));
    out.extend(rules4::executar(u, interner, ligada, sem));
    out.extend(rules5::executar(u, interner, ligada, sem));
    out.extend(rules6::executar(u, interner, ligada, sem));
    out.extend(rules7::executar(u, interner, ligada, sem));
    out.extend(rules8::executar(u, interner, ligada, sem));
    out.extend(rules9::executar(u, interner, ligada, sem));
    out.extend(rules10::executar(u, interner, ligada, sem));
    out.extend(rules11::executar(u, interner, ligada, sem));
    out.extend(rules12::executar(u, interner, ligada, sem));
    out.extend(rules13::executar(u, interner, ligada, sem));
    out.extend(rules14::executar(u, interner, ligada, sem));
    out.extend(rules15::executar(u, interner, ligada, sem));
    out.extend(rules16::executar(u, interner, ligada, sem));
    out.extend(rules17::executar(u, interner, ligada, sem));
    out.extend(rules18::executar(u, interner, ligada, sem));
    out.extend(rules19::executar(u, interner, ligada, sem));
    out.extend(rules20::executar(u, interner, ligada, sem));
    out.extend(rules21::executar(u, interner, ligada, sem));
    out.extend(rules22::executar(u, interner, ligada, sem));
    out.extend(rules23::executar(u, interner, ligada, sem));
    out.extend(rules24::executar(u, interner, ligada, sem));
    out.extend(rules25::executar(u, interner, ligada, sem));
    out.extend(rules26::executar(u, interner, ligada, sem));
    out.extend(rules27::executar(u, interner, ligada, sem));
    out.extend(rules28::executar(u, interner, ligada, sem));
    out.extend(rules29::executar(u, interner, ligada, sem));
    out.extend(rules30::executar(u, interner, ligada, sem));
    out.extend(rules31::executar(u, interner, ligada, sem));
    out.extend(rules32::executar(u, interner, ligada, sem));
    out.extend(rules33::executar(u, interner, ligada, sem));
    out.sort_by_key(|r| (r.span.start, r.span.end));
    out
}

/// A regra registrada de nome `nome`.
pub fn regra(nome: &str) -> Option<&'static InfoRegra> {
    tabela_g::REGRAS.iter().find(|r| r.nome == nome)
}
