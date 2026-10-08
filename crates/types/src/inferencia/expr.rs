//! Inferência de expressões: contexto (esquema) para baixo, tipo estático
//! para cima (`inference.md`, "Expression inference").
//!
//! Cadeias com `?.` fazem *null-shorting*: os nós internos da cadeia recebem
//! o tipo não anulável e só o nó que termina a cadeia recebe o `?`
//! ([`inferir_no`] devolve se a cadeia está em curto).

use super::atalhos;
use super::chamadas;
use super::colecoes;
use super::corpo::{Base, Corpo, Local, Nome};
use super::fluxo::Fluxo;
use super::funcoes;
use super::membros::{Busca, Membro};
use super::padroes;
use super::BodyInferrer;
use crate::codes::*;
use crate::resolved::{LocalId, MemberRef, Resolved};
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionElementId, FunctionKind};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, ExprId, ExprKind, UnaryOp};
use dartforge_intern::SymbolId;

/// Infere `e` no contexto `ctx` (o desconhecido `_` = sem contexto).
pub(crate) fn inferir(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId) -> TypeId {
    // `TypeAnalyzer.analyzeExpression` (`type_analyzer.dart:557-561`): o
    // esquema `dynamic` vira `_` (o `InvalidType` não).
    let ctx = if matches!(inf.table.get(ctx), Type::Dynamic) && !inf.table.e_invalido(ctx) { inf.core.unknown } else { ctx };
    let marca = cx.cadeias.len();
    let (t, curto) = inferir_no(inf, cx, e, ctx, false);
    fechar_cadeia(inf, cx, marca);
    let t = if curto {
        let t = inf.anulavel(t);
        registrar(inf, cx, e, t);
        t
    } else {
        t
    };
    registrar_chamada_implicita(inf, cx, e, t, ctx);
    t
}

/// `_insertImplicitCallReference` (`resolver.dart:4085-4142`) nas espécies
/// de expressão cujo `visit…` o chama: com o `getImplicitCallMethod`
/// (`error_detection_helpers.dart:312-334`), a expressão vira `.call`.
fn registrar_chamada_implicita(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, t: TypeId, ctx: TypeId) {
    let especie_que_insere = matches!(
        &ast(inf, cx).expr(e).kind,
        ExprKind::As { .. }
            | ExprKind::Assign { .. }
            | ExprKind::Await(_)
            | ExprKind::Binary { .. }
            | ExprKind::Cascade { .. }
            | ExprKind::Call { .. }
            | ExprKind::InstanceCreation { .. }
            | ExprKind::Index { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Identifier(_)
            | ExprKind::Property { .. }
            | ExprKind::This
    );
    if !especie_que_insere || inf.e_desconhecido(ctx) {
        return;
    }
    // `acceptsFunctionType(context)`: tipo de função, `Function`, ou
    // `FutureOr` de um deles.
    let mut contexto = ctx;
    for _ in 0..8 {
        match inf.table.get(contexto).clone() {
            Type::FutureOr { arg, .. } => contexto = arg,
            _ => break,
        }
    }
    let aceita = matches!(inf.table.get(contexto), Type::Function { .. }) || contexto == inf.core.function;
    if !aceita {
        return;
    }
    // Parâmetro de tipo não anulável: o limite.
    let mut tipo = t;
    let mut vistos = std::collections::HashSet::new();
    loop {
        match inf.table.get(tipo).clone() {
            Type::TypeParameter { param, nullable } => {
                if nullable || !vistos.insert(param) {
                    return;
                }
                tipo = inf.table.param(param).bound;
            }
            Type::Intersection { bound, .. } => tipo = bound,
            _ => break,
        }
    }
    if !matches!(inf.table.get(tipo), Type::Interface { nullable: false, .. }) || tipo == inf.core.function {
        return;
    }
    let Some(call) = inf.sym.call else { return };
    if let Some(m) = inf.membro_de_interface(tipo, call, false).filter(|m| m.metodo) {
        inf.body_types.units[cx.unit.0 as usize].chamadas_implicitas.insert(e);
        // O `call` genérico num contexto de função: a instanciação pelo
        // contexto, com os `COULD_NOT_INFER` na expressão.
        let _ = instanciar_em_contexto_com(inf, cx, e, m.tipo, contexto, false);
    }
}

/// Fim de uma cadeia com `?.`: a promoção do receptor valeu só dentro dela;
/// depois, o fluxo é a junção do "era nulo" (o de antes do primeiro `?.`) com
/// o de ter percorrido a cadeia.
fn fechar_cadeia(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, marca: usize) {
    if cx.cadeias.len() > marca {
        let antes = cx.cadeias[marca].clone();
        cx.cadeias.truncate(marca);
        let depois = cx.fluxo.clone();
        cx.fluxo = inf.juntar(&antes, &depois);
    }
}

/// Infere sem contexto.
pub(crate) fn inferir_livre(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId) -> TypeId {
    let u = inf.core.unknown;
    inferir(inf, cx, e, u)
}

pub(crate) fn registrar(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, t: TypeId) {
    inf.body_types.units[cx.unit.0 as usize].set_type(e, t);
}

pub(crate) fn resolver(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, r: Resolved) {
    let tabela = &mut inf.body_types.units[cx.unit.0 as usize];
    if inf.registrar_locais
        && let Resolved::Local(id) = &r
    {
        tabela.declaracoes_de_locais.insert(e, cx.local(*id).offset);
    }
    tabela.set_resolved(e, r);
}

/// Captura o escopo léxico quando `nome` é o identificador sondado pelo LSP.
pub(crate) fn sondar_escopo(inf: &mut BodyInferrer<'_>, cx: &Corpo, nome: ast::Name) {
    if inf.sonda_escopo == Some((cx.unit, nome.span.start)) && inf.escopo_sondado.is_none() {
        inf.escopo_sondado = Some(cx.escopo_visivel());
    }
}

fn ast<'p>(inf: &BodyInferrer<'p>, cx: &Corpo) -> &'p ast::Ast {
    &inf.program.unit(cx.unit).ast
}

/// O que um nome simples denota no ponto atual.
#[derive(Debug, Clone, Copy)]
pub(crate) enum RefNome {
    Local(LocalId),
    TipoParam(crate::table::TypeParamId),
    /// Pseudotipo embutido (`dynamic`, `Never`), sem elemento no namespace
    /// da biblioteca: como valor, é um literal de tipo (`Type`).
    TipoEmbutido,
    Elemento(Element),
    /// Membro declarado no corpo da classe/extensão envolvente.
    MembroLexico(dartforge_elements::model::FunctionElementId, bool),
    ConstanteEnum(dartforge_elements::model::VariableId),
    Prefixo,
    /// Não achado lexicamente, mas há `this`: `this.nome`.
    ThisImplicito,
    /// Local do bloco declarado depois deste uso (o nome na declaração).
    Adiante(dartforge_diagnostics::Span),
    Nenhum,
}

/// Busca léxica de um nome (`setter` = contexto de escrita).
pub(crate) fn resolver_nome(inf: &mut BodyInferrer<'_>, cx: &Corpo, nome: SymbolId, setter: bool) -> RefNome {
    match cx.buscar(nome) {
        Some(Nome::Local(id)) => return RefNome::Local(id),
        Some(Nome::TipoParam(p)) => return RefNome::TipoParam(p),
        Some(Nome::Adiante(s)) => return RefNome::Adiante(s),
        None => {}
    }
    // Membros declarados no corpo da classe/extensão.
    if cx.classe.is_some() || cx.extensao.is_some() {
        // O escopo do contêiner guarda getter e setter pelo nome base: a
        // leitura que só acha o setter para ali (`LexicalLookup.resolveGetter`).
        let chave_setter = inf.chave_setter(nome);
        let ordem: Vec<Option<SymbolId>> = if setter { vec![chave_setter, Some(nome)] } else { vec![Some(nome), chave_setter] };
        for chave in ordem.into_iter().flatten() {
            if let Some((f, estatico)) = inf.membro_declarado_lexico(cx.classe, cx.extensao, chave, None) {
                return RefNome::MembroLexico(f, estatico);
            }
        }
        if let Some(c) = cx.classe {
            if let Some(&v) = inf.program.class(c).enum_constants.iter().find(|&&v| inf.program.variable(v).name == nome) {
                return RefNome::ConstanteEnum(v);
            }
        }
        if let Some(x) = cx.extensao {
            if let Some(&v) = inf.program.extension(x).fields.iter().find(|&&v| inf.program.variable(v).name == nome) {
                return RefNome::Elemento(Element::Variable(v));
            }
        }
    }
    if let Some(b) = inf.program.lookup_na_unidade(cx.unit, nome) {
        let el = if setter { b.setter.or(b.getter) } else { b.getter.or(b.setter) };
        if let Some(el) = el {
            if let Element::Prefix(..) = el {
                return RefNome::Prefixo;
            }
            return RefNome::Elemento(el);
        }
    }
    if inf.program.prefixos_na_unidade(cx.unit).contains_key(&nome) {
        return RefNome::Prefixo;
    }
    if matches!(inf.interner.resolve(nome), "dynamic" | "Never") {
        return RefNome::TipoEmbutido;
    }
    if cx.tipo_this.is_some() && !cx.estatico {
        return RefNome::ThisImplicito;
    }
    RefNome::Nenhum
}

/// O `thisType` do `ResolverVisitor` (`_setupThisType`,
/// `an611:src/generated/resolver.dart:4180-4193`): o tipo `this` da classe
/// ou o tipo estendido da extensão envolvente, em **qualquer** membro —
/// estático, fábrica, inicializador de campo ou de construtor. A busca por
/// `this` implícito de um nome que não resolve lexicamente usa esse tipo
/// mesmo onde `this` não vale (o erro passa a ser o de acesso a membro de
/// instância, não o de nome indefinido).
fn tipo_this_do_analyzer(inf: &mut BodyInferrer<'_>, cx: &Corpo) -> Option<TypeId> {
    if cx.tipo_this.is_some() {
        return cx.tipo_this;
    }
    if let Some(c) = cx.classe {
        return Some(inf.tipo_this_classe(c));
    }
    cx.extensao.map(|x| inf.outline.extensions[x.0 as usize].on)
}

/// O que a busca léxica do analyzer acha para a **leitura** de um nome
/// (`LexicalLookup.resolveGetter`, `an611:src/dart/resolver/lexical_lookup.dart:17-33`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lexico {
    /// Getter, método, variável, tipo, prefixo…: a leitura resolve.
    Getter,
    /// Só um setter de instância declarado no contêiner: a busca segue
    /// pelo `this` implícito.
    SetterDeInstancia,
    /// Só um setter que não é de instância (de topo, estático, importado):
    /// recuperação — a leitura não tem elemento, e a busca para aqui.
    SetterSolto,
    /// Nada no escopo léxico.
    Nada,
}

fn busca_lexica_de_leitura(inf: &mut BodyInferrer<'_>, cx: &Corpo, nome: SymbolId) -> Lexico {
    match resolver_nome(inf, cx, nome, false) {
        RefNome::Nenhum | RefNome::ThisImplicito => Lexico::Nada,
        RefNome::MembroLexico(f, estatico) => {
            if inf.program.function(f).kind == FunctionKind::Setter {
                // `noGetterIsPossible` (3.6.2 `method_invocation_resolver.dart:616-628`):
                // o setter estático ou declarado numa extensão (também o de
                // instância) é a propriedade acessada; não se procura getter.
                if estatico || inf.program.function(f).extension.is_some() {
                    Lexico::SetterSolto
                } else {
                    Lexico::SetterDeInstancia
                }
            } else {
                Lexico::Getter
            }
        }
        RefNome::Elemento(Element::Function(f)) if inf.program.function(f).kind == FunctionKind::Setter => {
            // Setter de topo (declarado ou importado) sem getter de mesmo nome.
            match inf.program.lookup_na_unidade(cx.unit, nome) {
                Some(b) if b.getter.is_none() => Lexico::SetterSolto,
                _ => Lexico::Getter,
            }
        }
        _ => Lexico::Getter,
    }
}

/// O que a busca por `this` implícito acha (`ThisLookup`,
/// `an611:src/dart/resolver/this_lookup.dart:20-83`, que chama
/// `TypePropertyResolver.resolve` com a recuperação estática de
/// `_lookupInterfaceType`, `an611:src/dart/resolver/type_property_resolver.dart:251-281`).
#[derive(Debug, Clone, Copy)]
enum PeloThis {
    /// Membro de instância (da interface ou de extensão aplicável).
    Instancia,
    /// Membro estático da classe ou de uma superclasse/mixin (recuperação):
    /// `(classe que o declara, é método)`.
    Estatico(ClassId, bool),
    Ausente,
    /// Receptor anulável ou que não é de interface: o analyzer segue por
    /// outros caminhos (erros de nulo); não relatamos nada.
    Incerto,
}

fn buscar_pelo_this(inf: &mut BodyInferrer<'_>, cx: &Corpo, this: TypeId, nome: SymbolId, setter: bool) -> PeloThis {
    let classe = match inf.table.get(this) {
        Type::Interface { class, nullable: false, .. } => *class,
        Type::ExtensionType { decl, nullable: false, .. } => *decl,
        _ => return PeloThis::Incerto,
    };
    match inf.buscar_membro(cx.lib, this, nome, setter) {
        Busca::Achado(_) => PeloThis::Instancia,
        Busca::Dinamico | Busca::Nunca => PeloThis::Incerto,
        Busca::Ausente => match inf.recuperacao_estatica(cx.lib, classe, nome, setter) {
            Some((dono, metodo)) => PeloThis::Estatico(dono, metodo),
            None => PeloThis::Ausente,
        },
    }
}

/// O erro de acessar um membro de instância sem `this`
/// (`_checkForInvalidInstanceMemberAccess`,
/// `an611:src/generated/error_verifier.dart:3976-4038`): método estático
/// (e o que ele aninha), construtor de fábrica, inicializador de construtor
/// (os de campo ficam de fora: veja o braço `Raiz::Nada`). `None` onde
/// `this` vale.
fn erro_de_instancia_sem_this(inf: &BodyInferrer<'_>, cx: &Corpo) -> Option<dartforge_diagnostics::Codigo> {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    if cx.tipo_this.is_some() && !cx.estatico {
        return None;
    }
    if cx.classe.is_none() && cx.extensao.is_none() {
        return None;
    }
    match cx.raiz {
        super::corpo::Raiz::Funcao(_) if cx.membro_estatico => Some(c::INSTANCE_MEMBER_ACCESS_FROM_STATIC),
        super::corpo::Raiz::Funcao(_) => None,
        super::corpo::Raiz::Construtor(m) => match &inf.program.unit(cx.unit).ast.member(m).kind {
            ast::MemberKind::Constructor(k) if k.factory => Some(c::INSTANCE_MEMBER_ACCESS_FROM_FACTORY),
            ast::MemberKind::Constructor(_) => Some(c::IMPLICIT_THIS_REFERENCE_IN_INITIALIZER),
            _ => None,
        },
        // Inicializador de campo de instância não `late` ou de variável
        // estática (3.6.2 `error_verifier.dart:3976-4038`); os argumentos de
        // constante de enum e as anotações usam o mesmo corpo sem raiz e
        // ficam de fora.
        super::corpo::Raiz::Nada if cx.inicializador_de_variavel => Some(c::IMPLICIT_THIS_REFERENCE_IN_INITIALIZER),
        super::corpo::Raiz::Nada => None,
    }
}

/// Relata o acesso a membro de instância onde `this` não vale (ver
/// [`erro_de_instancia_sem_this`]).
fn avisar_instancia_sem_this(inf: &mut BodyInferrer<'_>, cx: &Corpo, n: ast::Name) {
    if let Some(codigo) = erro_de_instancia_sem_this(inf, cx) {
        let nome = inf.interner.resolve(n.sym).to_string();
        if codigo.info().nome == "implicit_this_reference_in_initializer" {
            inf.aviso_com_codigo(codigo, n.span, &[&nome]);
        } else {
            inf.aviso_com_codigo(codigo, n.span, &[]);
        }
    }
}

/// Referência sem qualificação a um membro estático herdado
/// (`_checkForUnqualifiedReferenceToNonLocalStaticMember`,
/// `an611:src/generated/error_verifier.dart:5660-5700`, e
/// `_reportInstanceAccessToStaticMember`,
/// `an611:src/dart/resolver/method_invocation_resolver.dart:208-227`).
fn avisar_estatico_nao_qualificado(inf: &mut BodyInferrer<'_>, cx: &Corpo, dono: ClassId, n: ast::Name) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let codigo = if cx.extensao.is_some() {
        c::UNQUALIFIED_REFERENCE_TO_STATIC_MEMBER_OF_EXTENDED_TYPE
    } else {
        c::UNQUALIFIED_REFERENCE_TO_NON_LOCAL_STATIC_MEMBER
    };
    let nome = inf.interner.resolve(inf.program.class(dono).name).to_string();
    inf.aviso_com_codigo(codigo, n.span, &[&nome]);
}

/// `p.loadLibrary` de um prefixo de import `deferred` (`PrefixScope.lookup`,
/// `an611:src/dart/element/scope.dart:576-583`): a função sintética
/// `Future<dynamic> Function()` da biblioteca adiada.
pub(crate) fn load_library(inf: &mut BodyInferrer<'_>, cx: &Corpo, prefixo: SymbolId, nome: SymbolId) -> Option<TypeId> {
    if inf.interner.resolve(nome) != "loadLibrary" {
        return None;
    }
    let lib = inf.program.library(cx.lib);
    let adiado = lib
        .imports
        .iter()
        .any(|im| im.prefix == Some(prefixo) && im.deferred && !inf.program.library(im.library).units.is_empty());
    if !adiado {
        return None;
    }
    let ret = match inf.core.future_class {
        Some(f) => {
            let d = inf.core.dynamic_;
            inf.table.intern(Type::Interface { class: f, args: Box::new([d]), nullable: false })
        }
        None => inf.core.dynamic_,
    };
    Some(inf.table.intern(Type::Function {
        type_params: Box::new([]),
        ret,
        positional: Box::new([]),
        optional: Box::new([]),
        named: Box::new([]),
        nullable: false,
    }))
}

/// `p.nome` que o prefixo não tem (`_resolveTargetPrefixElement`,
/// `an611:src/dart/resolver/property_element_resolver.dart:744-786`):
/// `UNDEFINED_PREFIXED_NAME`, salvo o nome ignorado de um import que não
/// existe.
pub(crate) fn avisar_nome_prefixado_indefinido(inf: &mut BodyInferrer<'_>, cx: &Corpo, prefixo: SymbolId, name: ast::Name) {
    if crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, cx.unit, Some(prefixo), name.sym) {
        return;
    }
    let nome = inf.interner.resolve(name.sym).to_string();
    let p = inf.interner.resolve(prefixo).to_string();
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_PREFIXED_NAME, name.span, &[&nome, &p]);
}

/// A leitura de um nome simples cuja busca léxica não achou getter
/// (`SimpleIdentifierResolver._resolve1`,
/// `an611:src/dart/resolver/simple_identifier_resolver.dart:209-224`, com a
/// busca de `PropertyElementResolver.resolveSimpleIdentifier`,
/// `an611:src/dart/resolver/property_element_resolver.dart:243-272`): o
/// `this` implícito ainda pode achar o membro (o erro é então o de acesso
/// sem `this` ou o de estático não qualificado); senão `await` num corpo de
/// função, o nome ignorado de um import que não existe, ou nome indefinido.
fn nome_lido_indefinido(inf: &mut BodyInferrer<'_>, cx: &Corpo, n: ast::Name, lexico: Lexico) {
    if lexico != Lexico::SetterSolto
        && let Some(this) = tipo_this_do_analyzer(inf, cx)
    {
        match buscar_pelo_this(inf, cx, this, n.sym, false) {
            PeloThis::Instancia => {
                avisar_instancia_sem_this(inf, cx, n);
                return;
            }
            PeloThis::Estatico(dono, metodo) => {
                // Tear-off de método estático: o `ErrorVerifier` não relata.
                if !metodo {
                    avisar_estatico_nao_qualificado(inf, cx, dono, n);
                }
                return;
            }
            PeloThis::Incerto => return,
            PeloThis::Ausente => {}
        }
    }
    nome_indefinido_sem_this(inf, cx, n);
}

/// `PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD` (`ScopeResolverVisitor.visitSimpleIdentifier`,
/// `an611:src/generated/resolver.dart:5186-5193`): o identificador em
/// contexto de escrita (`=`, `op=`, `++`/`--`) é uma variável do padrão
/// guardado cuja cláusula `when` está em análise (também dentro de closures
/// e de `if-case` da guarda).
/// `_checkForAssignmentToPrimaryConstructorParameter` (3.13,
/// `error_verifier.dart:3302-3327` do checkout main): escrita num parâmetro
/// do construtor primário dentro dos inicializadores dele ou de um campo de
/// instância (também numa função dentro deles), no nome escrito.
pub(crate) fn escrita_em_primario(inf: &mut BodyInferrer<'_>, cx: &Corpo, id: LocalId, span: dartforge_diagnostics::Span) {
    if cx.parametros_primarios.contains(&id) {
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::ASSIGNMENT_TO_PRIMARY_CONSTRUCTOR_PARAMETER, span, &[]);
    }
}

pub(crate) fn escrita_em_guarda(inf: &mut BodyInferrer<'_>, cx: &Corpo, id: LocalId, span: dartforge_diagnostics::Span) {
    if cx.variaveis_em_guarda.contains(&id) {
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD, span, &[]);
    }
}

/// A referência (leitura ou escrita) a uma variável de junção inconsistente
/// de casos que dividem o corpo (`ResolverVisitor.finishJoinedPatternVariable`,
/// `an611:src/generated/resolver.dart:878-915`), com o nome.
pub(crate) fn referencia_de_juncao(inf: &mut BodyInferrer<'_>, cx: &Corpo, id: LocalId, n: ast::Name) {
    if let Some(&codigo) = cx.juncoes_inconsistentes.get(&id) {
        let nome = inf.interner.resolve(n.sym).to_string();
        inf.aviso_com_codigo(codigo, n.span, &[&nome]);
    }
}

/// O fim de [`nome_lido_indefinido`]: `await` num corpo de função, nome
/// ignorado ou `UNDEFINED_IDENTIFIER`.
pub(crate) fn nome_indefinido_sem_this(inf: &mut BodyInferrer<'_>, cx: &Corpo, n: ast::Name) {
    let texto = inf.interner.resolve(n.sym).to_string();
    let em_funcao = !matches!(cx.raiz, super::corpo::Raiz::Nada) || !cx.funcoes.is_empty();
    if texto == "await" && em_funcao {
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_IDENTIFIER_AWAIT, n.span, &[]);
        return;
    }
    if crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, cx.unit, None, n.sym) {
        return;
    }
    // O prefixo de um import cujo alvo não existe (`import 'dart:foo' as foo;`):
    // o `PrefixElement` existe mesmo assim, e `foo.bar` é um nome prefixado
    // que não resolve, sem relato aqui.
    let lib = inf.program.unit(cx.unit).library;
    let e_prefixo = inf.program.library(lib).units.iter().any(|&u| {
        inf.program.unit(u).unit.directives.iter().any(|d| matches!(&d.kind, ast::DirectiveKind::Import { prefix: Some(p), .. } if p.sym == n.sym))
    });
    if e_prefixo {
        return;
    }
    let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, texto);
    inf.aviso(msg, n.span);
}

/// A invocação `nome(args)` sem alvo (`MethodInvocationResolver._resolveReceiverNull`,
/// `an611:src/dart/resolver/method_invocation_resolver.dart:559-657`) quando
/// a busca léxica não acha getter: sem `this`, `UNDEFINED_FUNCTION`; com
/// `this`, a busca nele (membro de instância achado segue o caminho comum;
/// estático herdado, estático não qualificado; nada, `UNDEFINED_METHOD` com
/// o nome da classe). Devolve se tratou a invocação (e relatou o que havia a
/// relatar): o chamador infere os argumentos sem contexto e dá `dynamic`.
pub(crate) fn invocacao_sem_alvo_indefinida(inf: &mut BodyInferrer<'_>, cx: &Corpo, n: ast::Name) -> bool {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    if cx.curinga == Some(n.sym) {
        return false;
    }
    let lexico = busca_lexica_de_leitura(inf, cx, n.sym);
    if lexico == Lexico::Getter {
        return false;
    }
    let Some(this) = tipo_this_do_analyzer(inf, cx) else {
        if !crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, cx.unit, None, n.sym) {
            let nome = inf.interner.resolve(n.sym).to_string();
            inf.aviso_com_codigo(c::UNDEFINED_FUNCTION, n.span, &[&nome]);
        }
        return true;
    };
    let nome_do_tipo = |inf: &BodyInferrer<'_>| -> String {
        match inf.table.get(this) {
            Type::Interface { class, .. } => inf.interner.resolve(inf.program.class(*class).name).to_string(),
            Type::ExtensionType { decl, .. } => inf.interner.resolve(inf.program.class(*decl).name).to_string(),
            Type::Function { .. } => "Function".to_string(),
            _ => "<unknown>".to_string(),
        }
    };
    if lexico == Lexico::SetterSolto {
        // Setter de topo, de extensão ou estático: não há getter possível.
        let nome = inf.interner.resolve(n.sym).to_string();
        let tipo = nome_do_tipo(inf);
        inf.aviso_com_codigo(c::UNDEFINED_METHOD, n.span, &[&nome, &tipo]);
        return true;
    }
    // `this` potencialmente anulável onde vale (extensão sobre tipo
    // anulável): o caminho comum resolve pelo `Object`/extensões ou relata
    // o uso sem checagem (`ThisLookup` → `TypePropertyResolver`).
    // (O tipo de extensão é anulável só pelo `?`, `type_property_resolver.dart:91-96`.)
    if let Some(t) = cx.tipo_this
        && !cx.estatico
        && !inf.e_dynamic(t)
        && !inf.e_nao_anulavel(t)
        && !matches!(inf.table.get(t), Type::ExtensionType { nullable: false, .. })
    {
        return false;
    }
    inf.ambiguidade_de_extensao = None;
    match buscar_pelo_this(inf, cx, this, n.sym, false) {
        // Extensões ambíguas (`_resolveReceiverNull` → `TypePropertyResolver`):
        // só a ambiguidade, no nome.
        PeloThis::Incerto if inf.ambiguidade_de_extensao.is_some() => {
            inf.relatar_ambiguidade_de_extensao(n.span);
            true
        }
        // Membro de instância: onde `this` vale, o caminho comum o resolve.
        PeloThis::Instancia if cx.tipo_this.is_some() && !cx.estatico => false,
        PeloThis::Instancia => {
            avisar_instancia_sem_this(inf, cx, n);
            true
        }
        PeloThis::Estatico(dono, _) => {
            avisar_estatico_nao_qualificado(inf, cx, dono, n);
            true
        }
        PeloThis::Incerto => true,
        PeloThis::Ausente => {
            let nome = inf.interner.resolve(n.sym).to_string();
            let tipo = nome_do_tipo(inf);
            inf.aviso_com_codigo(c::UNDEFINED_METHOD, n.span, &[&nome, &tipo]);
            true
        }
    }
}

/// Uma referência a tipo usada como receptor (`C.m`, `p.C.m`, `C<T>.m`).
#[derive(Debug, Clone)]
pub(crate) enum RefTipo {
    Classe(ClassId, Option<Vec<ast::TypeId>>),
    /// Alias de tipo que nomeia uma classe (`typedef A = B<int>`).
    /// Typedef para classe: argumentos da classe (`None`: o typedef só
    /// renomeia os parâmetros, e os argumentos são os explícitos ou
    /// inferidos como os da classe).
    Alias(ClassId, Option<Vec<TypeId>>, dartforge_elements::model::TypedefId),
    Extensao(ExtensionId),
}

/// Se `e` nomeia uma classe/extensão/typedef (não um valor), qual.
pub(crate) fn referencia_a_tipo(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> Option<RefTipo> {
    let a = ast(inf, cx);
    let el = match &a.expr(e).kind {
        ExprKind::Identifier(n) => match resolver_nome(inf, cx, n.sym, false) {
            RefNome::Elemento(el) => el,
            _ => return None,
        },
        ExprKind::Property { target, name, null_aware: false } => {
            let ExprKind::Identifier(p) = &a.expr(*target).kind else { return None };
            if !matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
                return None;
            }
            inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym)?.getter?
        }
        ExprKind::TypeArguments { target, type_args } => {
            let targs = type_args.to_vec();
            return match referencia_a_tipo(inf, cx, *target)? {
                RefTipo::Classe(c, None) => Some(RefTipo::Classe(c, Some(targs))),
                RefTipo::Alias(_, _, td) => {
                    let ex: Vec<TypeId> = targs.iter().map(|&t| inf.tipo_de_argumento_de_tipo(cx, t)).collect();
                    alias_de(inf, td, Some(ex))
                }
                _ => None,
            };
        }
        _ => return None,
    };
    match el {
        Element::Class(c) => Some(RefTipo::Classe(c, None)),
        Element::Extension(x) => Some(RefTipo::Extensao(x)),
        Element::Typedef(td) => alias_de(inf, td, None),
        _ => None,
    }
}

/// `e` nomeia (sem ou com prefixo) um alias de tipo genérico cujo alvo não
/// é classe nem tipo de extensão.
pub(crate) fn alias_sem_classe(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    let a = ast(inf, cx);
    let el = match &a.expr(e).kind {
        ExprKind::Identifier(n) => match resolver_nome(inf, cx, n.sym, false) {
            RefNome::Elemento(el) => el,
            _ => return false,
        },
        ExprKind::Property { target, name, null_aware: false } => {
            let ExprKind::Identifier(p) = &a.expr(*target).kind else { return false };
            if !matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
                return false;
            }
            match inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter) {
                Some(el) => el,
                None => return false,
            }
        }
        _ => return false,
    };
    let Element::Typedef(td) = el else { return false };
    let d = &inf.outline.typedefs[td.0 as usize];
    !d.type_params.is_empty() && !matches!(inf.table.get(d.target_type), Type::Interface { .. } | Type::ExtensionType { .. })
}

/// `e` (identificador simples) nomeia um alias de tipo cujo alvo não é classe
/// nem tipo de extensão (genérico ou não): o `nome(…)` dele é
/// `MethodInvocation` de um elemento que não é função
/// (`_resolveReceiverNull`, `method_invocation_resolver.dart:599`).
pub(crate) fn alias_de_tipo_nao_classe(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    let ExprKind::Identifier(n) = &ast(inf, cx).expr(e).kind else { return false };
    let RefNome::Elemento(Element::Typedef(td)) = resolver_nome(inf, cx, n.sym, false) else { return false };
    let d = &inf.outline.typedefs[td.0 as usize];
    // `Null` e `FutureOr` são `InterfaceType` no analyzer.
    !matches!(inf.table.get(d.target_type), Type::Interface { .. } | Type::ExtensionType { .. } | Type::Null | Type::FutureOr { .. })
}

/// `e` nomeia um alias de tipo de função.
pub(crate) fn alias_de_tipo_de_funcao(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    alvo_do_alias_nomeado(inf, cx, e).is_some_and(|t| matches!(inf.table.get(t), Type::Function { .. }))
}

/// `e` nomeia um alias cujo alvo é um parâmetro de tipo (`typedef T<X> = X`).
pub(crate) fn alias_de_parametro_de_tipo(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    alvo_do_alias_nomeado(inf, cx, e).is_some_and(|t| matches!(inf.table.get(t), Type::TypeParameter { .. }))
}

/// O alvo do alias que `e` (`T` ou `p.T`) nomeia.
fn alvo_do_alias_nomeado(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> Option<TypeId> {
    let el = match &ast(inf, cx).expr(e).kind {
        ExprKind::Identifier(n) => match resolver_nome(inf, cx, n.sym, false) {
            RefNome::Elemento(el) => el,
            _ => return None,
        },
        ExprKind::Property { target, name, .. } => match &ast(inf, cx).expr(*target).kind {
            ExprKind::Identifier(p) => inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter)?,
            _ => return None,
        },
        _ => return None,
    };
    let Element::Typedef(td) = el else { return None };
    Some(inf.outline.typedefs[td.0 as usize].target_type)
}

/// `e` nomeia um alias cujo alvo é `Null` ou `FutureOr<…>` (classes para o
/// analyzer, que reescreve `T<X>.nome()` como criação).
pub(crate) fn alias_de_null_ou_futureor(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    let el = match &ast(inf, cx).expr(e).kind {
        ExprKind::Identifier(n) => match resolver_nome(inf, cx, n.sym, false) {
            RefNome::Elemento(el) => el,
            _ => return false,
        },
        ExprKind::Property { target, name, .. } => match &ast(inf, cx).expr(*target).kind {
            ExprKind::Identifier(p) => match inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter) {
                Some(el) => el,
                None => return false,
            },
            _ => return false,
        },
        _ => return false,
    };
    let Element::Typedef(td) = el else { return false };
    matches!(inf.table.get(inf.outline.typedefs[td.0 as usize].target_type), Type::Null | Type::FutureOr { .. })
}

/// Classe e argumentos de um typedef usado como classe, com os argumentos
/// explícitos do typedef (ou sem eles).
fn alias_de(inf: &mut BodyInferrer<'_>, td: dartforge_elements::model::TypedefId, explicitos: Option<Vec<TypeId>>) -> Option<RefTipo> {
    let alvo = inf.outline.typedefs[td.0 as usize].target_type;
    let params = inf.outline.typedefs[td.0 as usize].type_params.clone();
    let (class, args) = match inf.table.get(alvo).clone() {
        Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => (class, args),
        // `Null` e `FutureOr<T>` são `InterfaceType` no analyzer.
        Type::Null => (inf.core.null_class?, Box::new([]) as Box<[TypeId]>),
        Type::FutureOr { arg, .. } => {
            let lib = inf.core.async_library?;
            let nome = inf.interner.lookup("FutureOr")?;
            let Some(Element::Class(c)) = inf.program.library(lib).declared.get(&nome).and_then(|b| b.getter) else { return None };
            (c, Box::new([arg]) as Box<[TypeId]>)
        }
        _ => return None,
    };
    if params.is_empty() {
        return Some(RefTipo::Alias(class, Some(args.to_vec()), td));
    }
    let inst = match explicitos {
        Some(ex) if ex.len() == params.len() => ex,
        _ => {
            // Typedef que só renomeia (`typedef M<K, V> = _M<K, V>`): os
            // argumentos se inferem como os da classe.
            let renomeia = args.len() == params.len()
                && args.iter().zip(params.iter()).all(|(&a, &p)| matches!(inf.table.get(a), Type::TypeParameter { param, nullable: false } if *param == p));
            if renomeia {
                return Some(RefTipo::Alias(class, None, td));
            }
            inf.instanciar_para_limites(&params)
        }
    };
    let mapa = inf.mapa(&params, &inst);
    let args = args.iter().map(|a| inf.subst(*a, &mapa)).collect();
    Some(RefTipo::Alias(class, Some(args), td))
}

/// Registra os nós de uma referência a tipo (tipo `Type`, resolução do elemento).
fn registrar_ref_tipo(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) {
    let a = ast(inf, cx);
    let tt = inf.core.type_;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => {
            if let Some(b) = inf.program.lookup_na_unidade(cx.unit, n.sym) {
                if let Some(el) = b.getter {
                    resolver(inf, cx, e, Resolved::Element(el));
                }
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::Property { target, name, .. } => {
            if let ExprKind::Identifier(p) = &a.expr(*target).kind {
                resolver(inf, cx, *target, Resolved::Prefix(cx.lib));
                if let Some(el) = inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter) {
                    resolver(inf, cx, e, Resolved::Element(el));
                }
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::TypeArguments { target, .. } => {
            let t = *target;
            registrar_ref_tipo(inf, cx, t);
            registrar(inf, cx, e, tt);
        }
        _ => {}
    }
}

/// Tipo de uma leitura de variável local (com checagem de atribuição definitiva).
fn ler_local(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, id: LocalId, span: dartforge_diagnostics::Span) -> TypeId {
    let l = cx.local(id).clone();
    // `checkReadOfNotAssignedLocalVariable`
    // (`an611:src/generated/resolver.dart:643-690`). Também no código
    // inalcançável, que herda o estado dos caminhos que levam a ele
    // (`return; use(x);` relata).
    if !l.funcao_local {
        let nome = inf.interner.resolve(l.nome).to_string();
        if l.late {
            if cx.fluxo.nao_atribuida(id) {
                inf.aviso_com_codigo(
                    dartforge_diagnostics::codigos::compile_time_error::DEFINITELY_UNASSIGNED_LATE_LOCAL_VARIABLE,
                    span,
                    &[&nome],
                );
            }
        } else if !cx.fluxo.atribuida(id) {
            if l.final_ {
                inf.aviso_com_codigo(
                    dartforge_diagnostics::codigos::compile_time_error::READ_POTENTIALLY_UNASSIGNED_FINAL,
                    span,
                    &[&nome],
                );
            } else if !inf.e_anulavel(l.tipo) && !inf.e_dynamic(l.tipo) {
                // `isPotentiallyNonNullable`: `Null` não é subtipo do tipo
                // (o parâmetro de tipo de limite `Object?` e o tipo de
                // extensão sem `Object` também).
                let msg = format!("{}: '{}'", DEFINITELY_UNASSIGNED_VARIABLE.template, nome);
                inf.aviso(msg, span);
            }
        }
    }
    cx.fluxo.tipo_atual(id, l.tipo)
}

/// `EXTENSION_AS_EXPRESSION` (`simple_identifier_resolver.dart:315-323`,
/// `prefixed_identifier_resolver.dart:160-184`): o nome (ou `p.E`) de uma
/// extensão usado como valor — fora de alvo de método ou de acesso a
/// propriedade. Com o relato o nó fica `dynamic`.
fn extensao_como_expressao(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, sp: dartforge_diagnostics::Span) -> bool {
    if let dartforge_frontend::pais::Pai::Expr(p) = inf.pai_de(cx.unit, e) {
        match &ast(inf, cx).expr(p).kind {
            ExprKind::Property { target, .. } | ExprKind::Call { target, .. } if *target == e => return false,
            _ => {}
        }
    }
    let texto = inf.program.unit(cx.unit).source[sp.start..sp.end].to_string();
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::EXTENSION_AS_EXPRESSION, sp, &[&texto]);
    true
}

/// O nome com que `_resolve` relata a contagem de argumentos de tipo de um
/// tear-off: o de uma função, método ou operador declarado (também a função
/// local); `None` para variável, getter ou outra expressão.
fn nome_de_funcao_referida(inf: &BodyInferrer<'_>, cx: &Corpo, alvo: ExprId) -> Option<String> {
    use crate::resolved::{MemberRef, Resolved};
    let funcao = |f: dartforge_elements::model::FunctionElementId| {
        let fe = inf.program.function(f);
        matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator).then(|| inf.interner.resolve(fe.name).to_string())
    };
    match inf.body_types.units[cx.unit.0 as usize].get_resolved(alvo)? {
        Resolved::Element(Element::Function(f)) => funcao(*f),
        Resolved::Member { member: MemberRef::Function(f), .. } => funcao(*f),
        Resolved::ExtensionMember { member, .. } => funcao(*member),
        // A função local (declarada antes: a marca `funcao_local` some depois
        // da inferência dela, e fica o conjunto `funcoes_locais`).
        Resolved::Local(id) if cx.local(*id).funcao_local || cx.funcoes_locais.contains(id) => {
            Some(inf.interner.resolve(cx.local(*id).nome).to_string())
        }
        _ => None,
    }
}

/// Tipo de uma leitura de elemento de topo.
fn ler_elemento(inf: &mut BodyInferrer<'_>, el: Element) -> TypeId {
    match el {
        Element::Class(_) | Element::Typedef(_) | Element::Extension(_) => inf.core.type_,
        Element::Variable(v) => inf.tipo_variavel(v),
        Element::Function(f) => {
            let fe = inf.program.function(f);
            match (fe.kind, fe.variable) {
                (FunctionKind::ImplicitAccessor, Some(v)) => inf.tipo_variavel(v),
                (FunctionKind::Getter, _) => inf.outline.functions[f.0 as usize].return_type,
                (FunctionKind::Setter, _) => {
                    inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_)
                }
                _ => inf.outline.functions[f.0 as usize].signature,
            }
        }
        Element::Prefix(..) => inf.core.dynamic_,
    }
}

fn resolved_de_membro_lexico(inf: &BodyInferrer<'_>, cx: &Corpo, f: dartforge_elements::model::FunctionElementId, estatico: bool) -> Resolved {
    let fe = inf.program.function(f);
    if let Some(x) = fe.extension.or(cx.extensao) {
        if fe.class.is_none() {
            return Resolved::ExtensionMember { extension: x, member: f };
        }
    }
    let class = fe.class.or(cx.classe).unwrap_or(ClassId(0));
    let member = match (fe.kind, fe.variable) {
        (FunctionKind::ImplicitAccessor, Some(v)) if estatico => MemberRef::Variable(v),
        _ => MemberRef::Function(f),
    };
    Resolved::Member { class, member, via_super: false }
}

/// `referencedBeforeDeclaration` (`diagnostic_factory.dart:342`): o contexto
/// é a declaração, no nome dela.
fn aviso_antes_da_declaracao(inf: &mut BodyInferrer<'_>, n: ast::Name, msg: String, decl: dartforge_diagnostics::Span) {
    let texto = format!("The declaration of '{}' is here.", inf.interner.resolve(n.sym));
    inf.diagnostics.push(dartforge_diagnostics::Diagnostic::new(msg, n.span).com_contexto(decl, texto));
    inf.unidades_dos_avisos.push(inf.unidade_corrente);
}

/// Identificador como valor.
fn identificador(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, n: ast::Name) -> TypeId {
    sondar_escopo(inf, cx, n);
    if let Some(&(_, decl)) = cx.ocultas_do_padrao.iter().find(|(s, _)| *s == n.sym) {
        let msg = format!("{}: '{}'", REFERENCED_BEFORE_DECLARATION.template, inf.interner.resolve(n.sym));
        aviso_antes_da_declaracao(inf, n, msg, decl);
        return inf.core.dynamic_;
    }
    match resolver_nome(inf, cx, n.sym, false) {
        RefNome::Local(id) => {
            resolver(inf, cx, e, Resolved::Local(id));
            referencia_de_juncao(inf, cx, id, n);
            if inf.locais_invalidos.contains(&(cx.unit, cx.local(id).offset)) {
                inf.body_types.units[cx.unit.0 as usize].tipos_invalidos.insert(e);
            }
            if n.span.start < cx.local(id).offset && !cx.local(id).funcao_local {
                let msg = format!("{}: '{}'", REFERENCED_BEFORE_DECLARATION.template, inf.interner.resolve(n.sym));
                let decl = dartforge_diagnostics::Span { start: cx.local(id).offset, end: cx.local(id).offset + (n.span.end - n.span.start) };
                aviso_antes_da_declaracao(inf, n, msg, decl);
            }
            let t = ler_local(inf, cx, id, n.span);
            // `whyNotPromoted` desta leitura (o histórico de não promoção da
            // variável neste ponto).
            inf.registrar_nao_promocao_local(cx, e, id);
            t
        }
        RefNome::TipoParam(p) => {
            resolver(inf, cx, e, Resolved::TypeParameter(p));
            // `_checkForTypeParameterReferencedByStatic` do
            // `visitSimpleIdentifier` (3.6.2, `error_verifier.dart:5419-5434`).
            if cx.membro_estatico && crate::resolve::param_da_classe(inf.table, p) {
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC, n.span, &[]);
            }
            inf.core.type_
        }
        RefNome::TipoEmbutido => inf.core.type_,
        RefNome::Elemento(_) if importacao_ambigua(inf, cx, None, n) => inf.table.invalido(inf.core.dynamic_),
        RefNome::Elemento(el) => {
            resolver(inf, cx, e, Resolved::Element(el));
            // Só um setter de topo com esse nome: a leitura não tem elemento.
            if busca_lexica_de_leitura(inf, cx, n.sym) == Lexico::SetterSolto {
                nome_lido_indefinido(inf, cx, n, Lexico::SetterSolto);
                return inf.table.invalido(inf.core.dynamic_);
            }
            if let Element::Extension(_) = el
                && extensao_como_expressao(inf, cx, e, n.span)
            {
                return inf.core.dynamic_;
            }
            ler_elemento(inf, el)
        }
        RefNome::MembroLexico(f, estatico) => {
            let r = resolved_de_membro_lexico(inf, cx, f, estatico);
            resolver(inf, cx, e, r);
            let so_setter = inf.program.function(f).kind == FunctionKind::Setter;
            if !estatico {
                // Membro de instância declarado aqui: pelo tipo `this` (a
                // substituição é a identidade).
                if let Some(this) = cx.tipo_this {
                    if let Busca::Achado(m) = inf.buscar_membro(cx.lib, this, n.sym, false) {
                        // Achado só o setter aqui, a leitura é `this.nome`: o
                        // elemento lido é o getter da interface (herdado), não
                        // o setter (os backends invocam o `Resolved` do nó).
                        if so_setter {
                            resolver(inf, cx, e, m.resolved.clone());
                        }
                        return leitura_de_campo(inf, cx, e, Base::This, m.tipo);
                    }
                }
            }
            if so_setter {
                // Leitura de um nome que só tem setter no contêiner: a de
                // instância ainda busca pelo `this` implícito; a estática não.
                let lexico = if estatico { Lexico::SetterSolto } else { Lexico::SetterDeInstancia };
                nome_lido_indefinido(inf, cx, n, lexico);
                return inf.table.invalido(inf.core.dynamic_);
            }
            if !estatico {
                avisar_instancia_sem_this(inf, cx, n);
            }
            inf.tipo_do_membro_declarado(f, false).0
        }
        RefNome::ConstanteEnum(v) => {
            let c = inf.program.variable(v).class;
            if let Some(c) = c {
                resolver(inf, cx, e, Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
            inf.tipo_variavel(v)
        }
        RefNome::Prefixo => {
            // `SimpleIdentifierResolver._resolve1` (3.6.2,
            // `simple_identifier_resolver.dart:199-205`): o prefixo fora de
            // `p.x` e de `p.f()` (argumento, `p?.x`, `p[0]`, cascata, `p()`).
            resolver(inf, cx, e, Resolved::Prefix(cx.lib));
            prefixo_sem_ponto(inf, n);
            inf.core.dynamic_
        }
        RefNome::ThisImplicito => {
            let this = cx.tipo_this.unwrap();
            // `ThisLookup.lookupGetter` com `this` potencialmente anulável
            // (extensão sobre tipo anulável): o `TypePropertyResolver` relata
            // o uso sem checagem, com o código pelo pai do nome, e recupera
            // pela interface do tipo sem `?`.
            if inf.exige_checagem_de_nulo(cx.lib, this, n.sym, false) {
                let codigo = codigo_do_this_anulavel(inf, cx, e);
                let nome = inf.interner.resolve(n.sym).to_string();
                let desde = inf.diagnostics.len();
                inf.aviso_de_nulo(this, codigo, n.span, &[&nome]);
                inf.anexar_nao_promocao(desde, cx, None, n.span);
                let nn = inf.nao_nulo(this);
                return match inf.buscar_membro(cx.lib, nn, n.sym, false) {
                    Busca::Achado(m) => {
                        resolver(inf, cx, e, m.resolved.clone());
                        m.tipo
                    }
                    _ => inf.core.dynamic_,
                };
            }
            match inf.buscar_membro(cx.lib, this, n.sym, false) {
                Busca::Achado(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    leitura_de_campo(inf, cx, e, Base::This, m.tipo)
                }
                // Extensões ambíguas (`ThisLookup.lookupGetter` →
                // `TypePropertyResolver`): a ambiguidade no nome, e sem
                // getter o `UNDEFINED_IDENTIFIER` também
                // (`simple_identifier_resolver.dart:208-221`).
                Busca::Dinamico if inf.ambiguidade_de_extensao.is_some() => {
                    inf.relatar_ambiguidade_de_extensao(n.span);
                    nome_indefinido_sem_this(inf, cx, n);
                    inf.table.invalido(inf.core.dynamic_)
                }
                Busca::Dinamico => inf.core.dynamic_,
                Busca::Nunca => inf.core.never,
                Busca::Ausente => {
                    nome_lido_indefinido(inf, cx, n, Lexico::Nada);
                    inf.table.invalido(inf.core.dynamic_)
                }
            }
        }
        RefNome::Adiante(decl) => {
            let msg = format!("{}: '{}'", REFERENCED_BEFORE_DECLARATION.template, inf.interner.resolve(n.sym));
            aviso_antes_da_declaracao(inf, n, msg, decl);
            // `checkReadOfNotAssignedLocalVariable` (3.6.2 `resolver.dart:643-679`):
            // o local ainda não atribuído, se `final` (não `late`; o `const`
            // não), é `READ_POTENTIALLY_UNASSIGNED_FINAL` no nome.
            let final_adiante = inf.program.unit(cx.unit).ast.stmts.iter().any(|st| match &st.kind {
                ast::StmtKind::Variables(vl) => vl.final_ && !vl.const_ && !vl.late && vl.variables.iter().any(|v| v.name.span == decl),
                _ => false,
            });
            if final_adiante {
                let nome = inf.interner.resolve(n.sym).to_string();
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::READ_POTENTIALLY_UNASSIGNED_FINAL, n.span, &[&nome]);
            }
            // O analyzer resolve o nome para o local declarado adiante (o
            // escopo do bloco já o tem): o `ReferenceFinder` vê a dependência
            // (`const x = [x];` é ciclo).
            if inf.registrar_locais {
                inf.body_types.units[cx.unit.0 as usize].declaracoes_de_locais.insert(e, decl.start);
            }
            inf.core.dynamic_
        }
        // Só curingas declaram `_` aqui (3.7): usar `_` é erro
        // (`Undefined name '_'` no CFE).
        RefNome::Nenhum if cx.curinga == Some(n.sym) => {
            inf.erro_de_linguagem(cx.unit, n.span, WILDCARD_NAO_LIGA.template.to_string());
            inf.core.dynamic_
        }
        RefNome::Nenhum => {
            nome_lido_indefinido(inf, cx, n, Lexico::Nada);
            inf.table.invalido(inf.core.dynamic_)
        }
    }
}

/// O código do uso sem checagem pelo `this` implícito
/// (`type_property_resolver.dart:107-140`): pelo pai do nome — a invocação
/// de método, o operador binário (o pai de cascata vale pela primeira
/// seção), senão o acesso a propriedade.
fn codigo_do_this_anulavel(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> dartforge_diagnostics::Codigo {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let a = ast(inf, cx);
    let pai = match inf.pai_de(cx.unit, e) {
        dartforge_frontend::pais::Pai::Expr(p) => p,
        _ => return c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
    };
    // Alvo de cascata: vale a primeira seção (`..m()` é invocação).
    if let ExprKind::Cascade { sections, .. } = &a.expr(pai).kind {
        return match sections.first().map(|&s| &a.expr(s).kind) {
            Some(ExprKind::Call { .. }) => c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
            _ => c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
        };
    }
    match &a.expr(pai).kind {
        ExprKind::Binary { .. } => c::UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE,
        // `nome()`: o `MethodInvocation` do nome.
        ExprKind::Call { target, .. } if *target == e => c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
        // `nome.m()`: o nome é o alvo do `MethodInvocation`.
        ExprKind::Property { target, .. } if *target == e => match inf.pai_de(cx.unit, pai) {
            dartforge_frontend::pais::Pai::Expr(q) if matches!(&a.expr(q).kind, ExprKind::Call { target, .. } if *target == pai) => {
                c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE
            }
            _ => c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
        },
        _ => c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
    }
}

/// O alvo de promoção que `e` denota, sem parênteses: a variável local, a
/// propriedade (`_PropertyReference`: o sintético estável da promovível, ou
/// a geração desta leitura da não promovível) ou `this`.
pub(crate) fn alvo_de_promocao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId) -> Option<LocalId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            alvo_de_promocao(inf, cx, i)
        }
        ExprKind::Identifier(n) => match cx.buscar(n.sym) {
            Some(Nome::Local(id)) if !cx.local(id).funcao_local => Some(id),
            Some(_) => None,
            // `_x` implícito: `this._x`.
            None => alvo_de_propriedade(inf, cx, e, Base::This),
        },
        ExprKind::Property { target, null_aware: false, .. } => {
            let t = *target;
            let base = base_de_propriedade(inf, cx, t)?;
            let antes = forcar_versao_de_cascata(inf, cx, t);
            let r = alvo_de_propriedade(inf, cx, e, base);
            cx.versao_forcada = antes;
            r
        }
        ExprKind::This => local_de_this(inf, cx),
        _ => None,
    }
}

/// A base de uma propriedade lida de `t` (o `PropertyTarget` com nó SSA):
/// `this`, `super`, uma local (não `late`, não função local) ou outra
/// propriedade promovível (estável).
fn base_de_propriedade(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, t: ExprId) -> Option<Base> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(t).kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            base_de_propriedade(inf, cx, i)
        }
        ExprKind::This => Some(Base::This),
        ExprKind::Super => Some(Base::Super),
        // A seção de cascata lê do alvo da cascata, cuja referência é a da
        // expressão alvo (`cascadeExpression_afterTarget` guarda o
        // `_getExpressionReference(target)`): `c?.._field` vê a promoção de
        // `c._field`.
        ExprKind::CascadeTarget => cx.bases_de_cascata.last().copied().flatten().map(|(b, _)| b),
        // O valor da cascata é o do alvo (a mesma referência).
        ExprKind::Cascade { target, .. } => {
            let alvo = *target;
            base_de_propriedade(inf, cx, alvo)
        }
        ExprKind::Identifier(n) => match cx.buscar(n.sym) {
            Some(Nome::Local(id)) if !cx.local(id).late && !cx.local(id).funcao_local => Some(Base::Local(id)),
            Some(_) => None,
            None => propriedade_estavel(inf, cx, t, Base::This).map(Base::Local),
        },
        ExprKind::Property { target, null_aware: false, .. } => {
            let t2 = *target;
            let b = base_de_propriedade(inf, cx, t2)?;
            let antes = forcar_versao_de_cascata(inf, cx, t2);
            let r = propriedade_estavel(inf, cx, t, b).map(Base::Local);
            cx.versao_forcada = antes;
            r
        }
        _ => None,
    }
}

/// Com `t` o alvo de cascata, força a versão da base dele à do início da
/// cascata; devolve a forçada anterior, para restaurar.
fn forcar_versao_de_cascata(inf: &BodyInferrer<'_>, cx: &mut Corpo, t: ExprId) -> Option<(Base, u32)> {
    let antes = cx.versao_forcada;
    if matches!(ast(inf, cx).expr(t).kind, ExprKind::CascadeTarget)
        && let Some(Some(bv)) = cx.bases_de_cascata.last().copied()
    {
        cx.versao_forcada = Some(bv);
    }
    antes
}

/// O membro de instância (getter, campo ou método, de classe ou de
/// extensão) que a leitura `e` alcança, com o nome escrito: o
/// `propertyMember` do `propertyGet`.
fn membro_lido(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> Option<(FunctionElementId, SymbolId)> {
    let nome = match &ast(inf, cx).expr(e).kind {
        ExprKind::Identifier(n) => n.sym,
        ExprKind::Property { name, .. } => name.sym,
        _ => return None,
    };
    let f = match inf.body_types.units[cx.unit.0 as usize].get_resolved(e)? {
        Resolved::Member { member: MemberRef::Function(f), .. } => *f,
        Resolved::ExtensionMember { member, .. } => *member,
        _ => return None,
    };
    if inf.program.function(f).static_ {
        return None;
    }
    Some((f, nome))
}

/// O sintético estável de uma propriedade promovível lida em `e` (`None`
/// se não é promovível).
fn propriedade_estavel(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, base: Base) -> Option<LocalId> {
    let (f, nome) = membro_lido(inf, cx, e)?;
    if !inf.propriedade_promovivel(cx.lib, f) {
        return None;
    }
    let chave = (base, cx.versao_da_base(base), nome);
    if let Some(&id) = cx.campos.get(&chave) {
        cx.garantir_modelo(id);
        return Some(id);
    }
    // O tipo declarado visto pelo receptor: o da leitura antes de qualquer
    // promoção (a primeira leitura, que ainda não tinha o sintético).
    let t = inf.body_types.units[cx.unit.0 as usize].get_type(e).unwrap_or(inf.core.dynamic_);
    let id = cx.declarar_sintetico(Local { nome, tipo: t, final_: true, late: false, const_: false, offset: 0, funcao_local: false });
    cx.campos.insert(chave, id);
    Some(id)
}

/// A propriedade lida em `e` como alvo de promoção: a promovível pelo
/// sintético estável; a não promovível ganha uma geração nova (o nó SSA
/// fresco de cada acesso), que só o why-not-promoted das leituras seguintes
/// consulta.
fn alvo_de_propriedade(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, base: Base) -> Option<LocalId> {
    if let Some(id) = propriedade_estavel(inf, cx, e, base) {
        return Some(id);
    }
    let (_, nome) = membro_lido(inf, cx, e)?;
    if let Some(&id) = cx.geracao_da_leitura.get(&e) {
        cx.garantir_modelo(id);
        return Some(id);
    }
    let chave = (base, cx.versao_da_base(base), nome);
    let t = inf.body_types.units[cx.unit.0 as usize].get_type(e).unwrap_or(inf.core.dynamic_);
    let id = cx.declarar_sintetico(Local { nome, tipo: t, final_: true, late: false, const_: false, offset: 0, funcao_local: false });
    cx.geracoes.entry(chave).or_default().push(id);
    cx.geracao_da_leitura.insert(e, id);
    Some(id)
}

/// O sintético de `this` (criado na primeira vez).
pub(crate) fn local_de_this(inf: &mut BodyInferrer<'_>, cx: &mut Corpo) -> Option<LocalId> {
    let tipo = cx.tipo_this?;
    if let Some(id) = cx.local_this {
        cx.garantir_modelo(id);
        return Some(id);
    }
    let nome = inf.sym.this_?;
    let id = cx.declarar_sintetico(Local { nome, tipo, final_: true, late: false, const_: false, offset: 0, funcao_local: false });
    cx.local_this = Some(id);
    Some(id)
}

/// O tipo lido da propriedade `e` (`alvo.nome`, já resolvida) de tipo
/// declarado `t`, com a promoção da propriedade quando o alvo é uma base
/// promovível: a função de `this._f()` (`FunctionExpressionInvocation` do
/// `PropertyAccess`).
pub(crate) fn tipo_lido_de_propriedade(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, t: TypeId) -> TypeId {
    let ExprKind::Property { target, null_aware: false, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind else { return t };
    let alvo = *target;
    match base_de_propriedade(inf, cx, alvo) {
        Some(base) => leitura_de_campo(inf, cx, e, base, t),
        None => t,
    }
}

/// Tipo lido de uma propriedade (`propertyGet` + `_handleProperty`): o
/// promovido, quando a propriedade é promovível e o tipo promovido é
/// subtipo do não promovido; a leitura da não promovível registra o
/// why-not-promoted dela.
pub(crate) fn leitura_de_campo(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, base: Base, t: TypeId) -> TypeId {
    let Some((f, nome)) = membro_lido(inf, cx, e) else { return t };
    // A seção de cascata lê do temporário do alvo, com o nó SSA do início
    // da cascata: uma escrita na local durante a cascata não tira a
    // promoção das seções seguintes (`c?.._f.g([c = C()]).._f`).
    let versao = match &ast(inf, cx).expr(e).kind {
        ExprKind::Property { target, .. } if matches!(ast(inf, cx).expr(*target).kind, ExprKind::CascadeTarget) => match cx.bases_de_cascata.last().copied().flatten() {
            Some((b, v)) if b == base => v,
            _ => cx.versao_da_base(base),
        },
        _ => cx.versao_da_base(base),
    };
    let chave = (base, versao, nome);
    if inf.propriedade_promovivel(cx.lib, f) {
        return match cx.campos.get(&chave) {
            Some(&id) => {
                let p = cx.fluxo.tipo_atual(id, t);
                if p != t && !inf.sub(p, t) {
                    t
                } else {
                    p
                }
            }
            None => t,
        };
    }
    inf.registrar_nao_promocao_de_propriedade(cx, e, chave, f);
    t
}

/// Coerção de tear-off genérico para um contexto de função não genérico
/// (instanciação implícita).
fn instanciar_em_contexto(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, t: TypeId, ctx: TypeId) -> TypeId {
    instanciar_em_contexto_com(inf, cx, e, t, ctx, true)
}

/// [`instanciar_em_contexto`]; `registrar` grava a instanciação do tear-off
/// (a referência implícita a `call` não é tear-off do nó).
fn instanciar_em_contexto_com(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, t: TypeId, ctx: TypeId, registrar: bool) -> TypeId {
    let Type::Function { type_params, ret, positional, optional, named, nullable } = inf.table.get(t).clone() else { return t };
    if type_params.is_empty() || inf.e_desconhecido(ctx) {
        return t;
    }
    let k = inf.fecho_maior(ctx);
    let k = inf.nao_nulo(k);
    let Type::Function { type_params: tp_ctx, .. } = inf.table.get(k).clone() else { return t };
    if !tp_ctx.is_empty() {
        return t;
    }
    let sem = inf.table.intern(Type::Function {
        type_params: Box::new([]),
        ret,
        positional: positional.clone(),
        optional: optional.clone(),
        named: named.clone(),
        nullable,
    });
    // `inferFunctionTypeInstantiation` (3.6.2 `type_system.dart`): o
    // `GenericInferrer` com o relator e a expressão como entidade
    // (`constrainGenericFunctionInContext`, origem "Function type declared
    // as … used where … is required."); o `chooseFinalTypes` relata
    // `COULD_NOT_INFER`.
    let mut gi = crate::constraints::GenericInferrer::new(&type_params);
    gi.com_origem(crate::constraints::Origem::Funcao { declarado: t, contexto: k });
    gi.metadados_genericos = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::GenericMetadata);
    let (interner, program) = (inf.interner, inf.program);
    let mut env = inf.env();
    gi.constrain_return(sem, k, &mut env);
    let args = gi.choose_final(&mut env);
    let falhas = gi.falhas(&args, &mut env, interner, program);
    let r = crate::constraints::instanciar_funcao(t, &args, &mut env);
    drop(env);
    let sp = inf.span_expr(cx.unit, e);
    for (nome, sufixo) in falhas {
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::COULD_NOT_INFER, sp, &[&nome, &sufixo]);
    }
    if registrar {
        inf.body_types.units[cx.unit.0 as usize].instanciacoes_de_tearoff.insert(e, args.into_boxed_slice());
    }
    r
}

/// Infere um nó; devolve `(tipo, curto)` onde `curto` diz que há `?.` na
/// cadeia abaixo (o chamador que termina a cadeia torna o tipo anulável).
pub(crate) fn inferir_no(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId, _cadeia: bool) -> (TypeId, bool) {
    let a = ast(inf, cx);
    let expr = a.expr(e);
    let span = expr.span;
    // `checkUnreachableNode` (`NullSafetyDeadCodeVerifier.visitNode`): a
    // primeira expressão visitada num fluxo inalcançável é o primeiro nó
    // morto; o trecho vai até o fim do bloco básico em curso (ou da última
    // instrução do bloco).
    if !cx.fluxo.alcancavel && cx.trecho_morto.is_none() {
        let fim = cx.fins_de_fluxo.last().copied().or(cx.fins_de_bloco.last().copied()).unwrap_or(span.end).max(span.end);
        inf.aviso(DEAD_CODE.template.to_string(), dartforge_diagnostics::Span { start: span.start, end: fim });
        cx.trecho_morto = Some(cx.fins_de_fluxo.len());
    }
    let mut curto = false;
    atalhos::registrar_cadeia(inf, cx, e, ctx);
    let t = match &expr.kind {
        ExprKind::DotShorthand { name, .. } => atalhos::valor(inf, cx, e, name.sym, ctx),
        ExprKind::Int(_) => {
            let s = inf.fecho_maior(ctx);
            let (int, double) = (inf.core.int, inf.core.double);
            let tipo = if !inf.e_desconhecido(ctx) && inf.sub(double, s) && !inf.sub(int, s) { double } else { int };
            let negado = cx.literal_negado == Some(e);
            if negado {
                cx.literal_negado = None;
            }
            super::inteiros::literal_fora_do_alcance(inf, cx, e, tipo, negado);
            tipo
        }
        ExprKind::Double(_) => inf.core.double,
        ExprKind::Bool(_) => inf.core.bool_,
        ExprKind::Null => inf.core.null,
        ExprKind::String(lit) => {
            for p in lit.parts.iter() {
                if let ast::StringPart::Interpolation(i) = p {
                    let t = inferir_livre(inf, cx, *i);
                    uso_de_void(inf, cx, *i, t);
                }
            }
            inf.core.string
        }
        ExprKind::Symbol(_) => inf.core.symbol,
        // `$this` numa interpolação chega como identificador.
        ExprKind::Identifier(n) if Some(n.sym) == inf.sym.this_ => cx.tipo_this.unwrap_or(inf.core.dynamic_),
        ExprKind::Identifier(n) => {
            let t = identificador(inf, cx, e, *n);
            instanciar_em_contexto(inf, cx, e, t, ctx)
        }
        // O `_thisType` do `ResolverVisitor` vale no corpo inteiro da
        // declaração, também nos membros estáticos, nos inicializadores de
        // campo e nas `factory` (o `this` ali é `INVALID_REFERENCE_TO_THIS`,
        // mas tem o tipo dela).
        ExprKind::This => match cx.tipo_this {
            Some(t) => t,
            None if !cx.this_sem_tipo => tipo_this_do_analyzer(inf, cx).unwrap_or(inf.core.dynamic_),
            None => inf.core.dynamic_,
        },
        // `visitSuperExpression` (3.6.2 `static_type_analyzer.dart:240-252`):
        // sem `this` ou dentro de `extension`, o tipo é inválido.
        ExprKind::Super => match cx.tipo_this {
            Some(t) if cx.extensao.is_none() => t,
            _ => inf.table.invalido(inf.core.dynamic_),
        },
        ExprKind::Parenthesized(i) => {
            let marca = cx.cadeias.len();
            let (t, c) = inferir_no(inf, cx, *i, ctx, false);
            fechar_cadeia(inf, cx, marca);
            let t = if c { inf.anulavel(t) } else { t };
            registrar(inf, cx, *i, t);
            t
        }
        ExprKind::List { .. } | ExprKind::SetOrMap { .. } => colecoes::literal(inf, cx, e, ctx),
        ExprKind::Record { positional, named, .. } => {
            let k = inf.fecho_maior_se_conhecido(ctx);
            let k = inf.nao_nulo(k);
            let (ctx_pos, ctx_nom) = match inf.table.get(k).clone() {
                Type::Record { positional: p, named: n, .. } if p.len() == positional.len() && n.len() == named.len() => (Some(p), Some(n)),
                _ => (None, None),
            };
            let u = inf.core.unknown;
            let positional = positional.to_vec();
            let named: Vec<(ast::Name, ExprId)> = named.to_vec();
            // `_resolveField` (3.6.2 `record_literal_resolver.dart:134-158`):
            // o campo `void` relata no campo (o nomeado, do nome ao valor).
            let mut pos = Vec::new();
            for (i, p) in positional.iter().enumerate() {
                let c = ctx_pos.as_ref().map(|v| v[i]).unwrap_or(u);
                let tp = inferir(inf, cx, *p, c);
                let tp = cast_de_dynamic_no_campo(inf, tp, c);
                if matches!(inf.table.get(tp), Type::Void) {
                    let sp = inf.span_expr(cx.unit, *p);
                    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
                }
                pos.push(tp);
            }
            let mut nm = Vec::new();
            for (n, x) in named.iter() {
                let c = ctx_nom.as_ref().and_then(|v| v.iter().find(|(s, _)| *s == n.sym).map(|(_, t)| *t)).unwrap_or(u);
                let tx = inferir(inf, cx, *x, c);
                let tx = cast_de_dynamic_no_campo(inf, tx, c);
                if matches!(inf.table.get(tx), Type::Void) {
                    let fim = inf.span_expr(cx.unit, *x).end;
                    let sp = dartforge_diagnostics::Span { start: n.span.start, end: fim };
                    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
                }
                nm.push((n.sym, tx));
            }
            inf.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
        }
        ExprKind::InstanceCreation { .. } => {
            let t = chamadas::instanciacao(inf, cx, e, ctx);
            api_sem_nulo(inf, cx, e, t);
            t
        }
        // `insertGenericFunctionInstantiation` (3.6.2 `resolver.dart:1171`)
        // depois de `visitFunctionExpression` (`:2833`), das invocações
        // (`:2855`, `:3244`), do índice (`:3035`), dos operadores
        // (`:2094`, `:3414`, `:3455`), de `as` (`:1867`), da atribuição
        // (`:1933`) e de `await` (`:2081`), além dos identificadores e
        // propriedades.
        ExprKind::FunctionExpression(f) => {
            let t = funcoes::expressao_de_funcao(inf, cx, *f, ctx, e);
            instanciar_em_contexto(inf, cx, e, t, ctx)
        }
        ExprKind::Property { target, name, null_aware } => {
            let (t, c) = propriedade(inf, cx, e, *target, *name, *null_aware);
            curto = c;
            instanciar_em_contexto(inf, cx, e, t, ctx)
        }
        ExprKind::Index { target, index, null_aware } => {
            let (t, c) = ler_indice(inf, cx, e, *target, *index, *null_aware, ctx);
            curto = c;
            instanciar_em_contexto(inf, cx, e, t, ctx)
        }
        ExprKind::Call { .. } => {
            if let Some(t) = atalhos::construcao(inf, cx, e, ctx) {
                t
            } else {
                let (t, c) = chamadas::chamada(inf, cx, e, ctx);
                curto = c;
                api_sem_nulo(inf, cx, e, t);
                instanciar_em_contexto(inf, cx, e, t, ctx)
            }
        }
        ExprKind::TypeArguments { target, type_args } => {
            if referencia_a_tipo(inf, cx, e).is_some() {
                conferir_argumentos_do_literal(inf, cx, e, false);
                // O `NamedType.type` de cada argumento (o verificador de
                // constantes lê o do `const (List<T>)`); os relatos já saíram
                // na conferência acima.
                let antes = inf.diagnostics.len();
                for &x in type_args.iter() {
                    inf.tipo_de_argumento_de_tipo(cx, x);
                }
                inf.diagnostics.truncate(antes);
                inf.unidades_dos_avisos.truncate(antes);
                registrar_ref_tipo(inf, cx, e);
                inf.core.type_
            } else if alias_sem_classe(inf, cx, *target) {
                // `Cb<String>` com `Cb` alias de um tipo que não é classe
                // (`typedef Cb<T> = void Function()`): também `TypeLiteral`.
                conferir_argumentos_do_literal(inf, cx, e, false);
                for &x in type_args.iter() {
                    inf.tipo_de_argumento_de_tipo(cx, x);
                }
                registrar_ref_tipo(inf, cx, e);
                inf.core.type_
            } else {
                // `C.nome<T>` (tear-off de construtor com argumentos):
                // `WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR`, e a instanciação
                // segue sobre o tipo do tear-off.
                if let ExprKind::Property { target: r, name, .. } = &ast(inf, cx).expr(*target).kind
                    && let Some(rt) = referencia_a_tipo(inf, cx, *r)
                    && let RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) = rt
                {
                    let chave = if Some(name.sym) == inf.sym.new_ { inf.sym.vazio } else { Some(name.sym) };
                    if chave.and_then(|k| inf.construtor_ou_primario(c, k)).is_some() {
                        let escrito = ast(inf, cx).expr(*r).span;
                        super::chamadas::tipos_no_construtor(inf, cx.unit, escrito, *name, type_args);
                        // O `FunctionReference` de um `ConstructorReference`
                        // fica `InvalidType` (`function_reference_resolver.dart:250-252`).
                        inferir_livre(inf, cx, *target);
                        for &x in type_args.iter() {
                            inf.tipo_de_argumento_de_tipo(cx, x);
                        }
                        return (inf.table.invalido(inf.core.dynamic_), false);
                    }
                }
                let t = inferir_livre(inf, cx, *target);
                let targs: Vec<TypeId> = type_args.iter().map(|&x| inf.tipo_de_argumento_de_tipo(cx, x)).collect();
                // `_resolve`: o parâmetro de tipo vale pelo limite.
                let bruto = match inf.table.get(t).clone() {
                    Type::TypeParameter { param, .. } => inf.table.param(param).bound,
                    _ => t,
                };
                match inf.table.get(bruto).clone() {
                    Type::Function { type_params, .. } if type_params.len() == targs.len() => {
                        // `checkFunctionReference` do `ErrorVerifier`.
                        let nos = type_args.to_vec();
                        let sp = inf.span_expr(cx.unit, e);
                        let visiveis: Vec<crate::table::TypeParamId> = cx.parametros_de_tipo_visiveis().into_values().collect();
                        chamadas::conferir_limites_explicitos(inf, cx.unit, &type_params, &targs, &nos, sp, Some(&visiveis));
                        let mut env = inf.env();
                        crate::constraints::instanciar_funcao(bruto, &targs, &mut env)
                    }
                    Type::Function { type_params, .. } => {
                        // `_checkTypeArguments` (`function_reference_resolver.dart:115-150`):
                        // com o nome de uma função ou método declarado,
                        // `WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION`; de uma
                        // variável, getter ou expressão, a variante anônima.
                        // Os argumentos viram `dynamic`.
                        if let Some(sp) = super::chamadas::faixa_da_lista_de_tipos(inf, cx.unit, type_args) {
                            let (n, d) = (type_params.len().to_string(), targs.len().to_string());
                            match nome_de_funcao_referida(inf, cx, *target) {
                                Some(nome) => inf.aviso_com_codigo(
                                    dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION,
                                    sp,
                                    &[&nome, &n, &d],
                                ),
                                // O tipo da função vai depois: só o texto do
                                // 3.13.4 o usa (`The type of this function is …`).
                                None => {
                                    let tipo = inf.table.format(bruto, inf.interner, inf.program);
                                    inf.aviso_com_codigo(
                                        dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION,
                                        sp,
                                        &[&n, &d, &tipo],
                                    )
                                }
                            }
                        }
                        let dinamicos = vec![inf.core.dynamic_; type_params.len()];
                        let mut env = inf.env();
                        crate::constraints::instanciar_funcao(bruto, &dinamicos, &mut env)
                    }
                    _ if inf.table.e_invalido(bruto) => bruto,
                    // `p.nome<…>` com `nome` que não resolve no prefixo (o
                    // import ausente, `shouldIgnoreUndefined`): o resolvedor do
                    // nome prefixado para sem chegar ao `_resolve` da
                    // referência, e o tipo é inválido, sem relato.
                    _ if matches!(&ast(inf, cx).expr(*target).kind, ExprKind::Property { target: p, .. }
                        if matches!(inf.body_types.units[cx.unit.0 as usize].get_resolved(*p), Some(Resolved::Prefix(_))))
                        && inf.body_types.units[cx.unit.0 as usize].get_resolved(*target).is_none() =>
                    {
                        inf.table.invalido(inf.core.dynamic_)
                    }
                    // `node<…>` de um objeto com o método `call`: `node.call<…>`
                    // (`_resolveAsImplicitCallReference`, 3.6.2
                    // `function_reference_resolver.dart:287-306`): a contagem
                    // confere com os parâmetros de tipo do `call`, com o nome
                    // `call`, e o tipo é o do método instanciado.
                    Type::Interface { .. }
                        if inf.sym.call.is_some_and(|call| matches!(inf.buscar_membro(cx.lib, bruto, call, false), super::membros::Busca::Achado(m) if m.metodo)) =>
                    {
                        let call = inf.sym.call.expect("call");
                        let tipo_call = match inf.buscar_membro(cx.lib, bruto, call, false) {
                            super::membros::Busca::Achado(m) => m.tipo,
                            _ => unreachable!(),
                        };
                        match inf.table.get(tipo_call).clone() {
                            Type::Function { type_params, .. } => {
                                let usados = if type_params.len() == targs.len() {
                                    targs.clone()
                                } else {
                                    if let Some(sp) = super::chamadas::faixa_da_lista_de_tipos(inf, cx.unit, type_args) {
                                        let (n, d) = (type_params.len().to_string(), targs.len().to_string());
                                        inf.aviso_com_codigo(
                                            dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION,
                                            sp,
                                            &["call", &n, &d],
                                        );
                                    }
                                    vec![inf.core.dynamic_; type_params.len()]
                                };
                                let mut env = inf.env();
                                crate::constraints::instanciar_funcao(tipo_call, &usados, &mut env)
                            }
                            _ => inf.core.dynamic_,
                        }
                    }
                    _ => {
                        // `DISALLOWED_TYPE_INSTANTIATION_EXPRESSION`
                        // (`function_reference_resolver.dart:270-278`), com
                        // os tear-offs de construtor (2.15).
                        let versao = inf.program.library(cx.lib).features.versao();
                        if versao >= dartforge_frontend::features::LanguageVersion::new(2, 15) {
                            let sp = inf.span_expr(cx.unit, *target);
                            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::DISALLOWED_TYPE_INSTANTIATION_EXPRESSION, sp, &[]);
                            inf.table.invalido(inf.core.dynamic_)
                        } else {
                            inf.core.dynamic_
                        }
                    }
                }
            }
        }
        ExprKind::Unary { op, operand } => {
            let t = unario(inf, cx, e, *op, *operand, ctx, &mut curto);
            instanciar_em_contexto(inf, cx, e, t, ctx)
        }
        ExprKind::Binary { op, left, right } => {
            let t = binario(inf, cx, e, *op, *left, *right, ctx);
            instanciar_em_contexto(inf, cx, e, t, ctx)
        }
        ExprKind::Conditional { condition, then, else_ } => {
            let (then, else_) = (*then, *else_);
            let (vf, ff) = condicao_verificada(inf, cx, *condition);
            let antes = cx.fluxo.clone();
            cx.fluxo = vf;
            let t1 = operando_de_fluxo(inf, cx, then, None, |inf, cx| inferir(inf, cx, then, ctx));
            let depois1 = std::mem::replace(&mut cx.fluxo, ff);
            let t2 = operando_de_fluxo(inf, cx, else_, None, |inf, cx| inferir(inf, cx, else_, ctx));
            let depois2 = std::mem::replace(&mut cx.fluxo, antes);
            cx.fluxo = inf.juntar(&depois1, &depois2);
            limite_superior_em_contexto(inf, t1, t2, ctx)
        }
        ExprKind::Is { value, ty, negated } => {
            let (vf, ff) = teste_de_tipo(inf, cx, *value, *ty, *negated, span);
            cx.fluxo = inf.juntar(&vf, &ff);
            inf.core.bool_
        }
        ExprKind::As { value, ty } => {
            let v = inferir_livre(inf, cx, *value);
            let t = inf.tipo_no_contexto(cx, *ty, crate::resolve::ContextoDeTipo::As);
            let iguais = v == t || crate::ops::sem_exibicao(v, inf.table) == crate::ops::sem_exibicao(t, inf.table);
            if iguais && !inf.e_dynamic(v) && !inf.table.e_invalido(v) && !inf.table.e_invalido(t) {
                inf.aviso(UNNECESSARY_CAST.template.to_string(), span);
            }
            // `CAST_FROM_NULL_ALWAYS_FAILS` (`visitAsExpression`,
            // `best_practices_verifier.dart:135-150`).
            let nao_anulavel = inf.e_nao_anulavel(t);
            if nao_anulavel && e_null_do_core(inf, v) {
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::warning::CAST_FROM_NULL_ALWAYS_FAILS, span, &[]);
            }
            // `CAST_FROM_NULLABLE_ALWAYS_FAILS` (`ResolverVisitor.visitAsExpression`,
            // `resolver.dart:1871-1887`): o operando é o identificador de uma
            // local ou parâmetro de tipo declarado anulável, definitivamente
            // não atribuído.
            if nao_anulavel && !e_null_do_core(inf, v)
                && let ExprKind::Identifier(n) = &ast(inf, cx).expr(*value).kind
                && let Some(Resolved::Local(id)) = inf.body_types.units[cx.unit.0 as usize].get_resolved(*value).cloned()
            {
                let n = *n;
                let declarado = cx.local(id).tipo;
                if inf.e_anulavel(declarado) && cx.fluxo.nao_atribuida(id) {
                    let nome = inf.interner.resolve(n.sym).to_string();
                    inf.aviso_com_codigo(dartforge_diagnostics::codigos::warning::CAST_FROM_NULLABLE_ALWAYS_FAILS, n.span, &[&nome]);
                }
            }
            if let Some(id) = alvo_de_promocao(inf, cx, *value) {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                inf.promover(&mut f, id, decl, t);
                cx.fluxo = f;
            }
            instanciar_em_contexto(inf, cx, e, t, ctx)
        }
        ExprKind::Assign { op, target, value } => {
            let t = atribuicao(inf, cx, e, *op, *target, *value, &mut curto);
            instanciar_em_contexto(inf, cx, e, t, ctx)
        }
        ExprKind::PatternAssign { pattern, value } => padroes::atribuicao_de_padrao(inf, cx, *pattern, *value),
        ExprKind::Cascade { target, sections, null_aware } => {
            let t = inferir(inf, cx, *target, ctx);
            uso_de_void(inf, cx, *target, t);
            // `EXTENSION_OVERRIDE_WITH_CASCADE`
            // (`method_invocation_resolver.dart:431-437`,
            // `property_element_resolver.dart:606-612`): no nome da extensão
            // do `E(x)` alvo da cascata (um relato por cascata).
            if cx.sobreposicoes.contains_key(target)
                && let ExprKind::Call { target: f, .. } = &ast(inf, cx).expr(*target).kind
            {
                let nome = match &ast(inf, cx).expr(*f).kind {
                    ExprKind::Identifier(n) => Some(n.span),
                    ExprKind::Property { name, .. } => Some(name.span),
                    ExprKind::TypeArguments { target: g, .. } => match &ast(inf, cx).expr(*g).kind {
                        ExprKind::Identifier(n) => Some(n.span),
                        ExprKind::Property { name, .. } => Some(name.span),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(sp) = nome {
                    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::EXTENSION_OVERRIDE_WITH_CASCADE, sp, &[]);
                }
            }
            // `c?..x` com `c` não anulável: relatado pela primeira seção
            // (`visitPropertyAccess`/`visitMethodInvocation`/`visitIndexExpression`
            // do `ErrorVerifier`).
            if *null_aware {
                if let Some(&primeira) = sections.first() {
                    if primeira_secao_sem_reescrita(inf, cx, primeira, t) {
                        operador_nulo_desnecessario(inf, cx, *target, t);
                    }
                }
            }
            // `promoteToNonNull`, como no `?.`: o parâmetro de tipo de limite
            // anulável vira `T & NonNull(limite)` (`nn?..toRadixString(16)`).
            let r = if *null_aware {
                match inf.table.get(t) {
                    Type::TypeParameter { nullable: false, .. } => inf.nao_nulo_promocao(t),
                    _ => inf.nao_nulo(t),
                }
            } else {
                t
            };
            cx.cascatas.push(r);
            cx.alvos_de_cascata.push(*target);
            // Alvo que não é referência (`getC()?..`): o temporário da
            // cascata é um nó SSA novo, cujas propriedades promovem entre as
            // seções.
            // A local capturada por escrita não promove, mas o temporário
            // sim: ganha base própria.
            let capturada = |cx: &Corpo, b: Base| matches!(b, Base::Local(id) if cx.fluxo.modelo(id).is_some_and(|m| m.capturada));
            let bv = match base_de_propriedade(inf, cx, *target).filter(|&b| !capturada(cx, b)) {
                Some(b) => Some((b, cx.versao_da_base(b))),
                None => inf.sym.vazio.map(|nome| {
                    let id = cx.declarar_sintetico(Local { nome, tipo: r, final_: true, late: false, const_: false, offset: 0, funcao_local: false });
                    (Base::Local(id), cx.versao_da_base(Base::Local(id)))
                }),
            };
            cx.bases_de_cascata.push(bv);
            let secs = sections.to_vec();
            for s in secs {
                inferir_livre(inf, cx, s);
            }
            cx.bases_de_cascata.pop();
            cx.alvos_de_cascata.pop();
            cx.cascatas.pop();
            t
        }
        ExprKind::CascadeTarget => cx.cascatas.last().copied().unwrap_or(inf.core.dynamic_),
        ExprKind::Await(i) => {
            let k1 = contexto_de_await(inf, ctx);
            let t1 = inferir(inf, cx, *i, k1);
            uso_de_void(inf, cx, *i, t1);
            // `AWAIT_OF_INCOMPATIBLE_TYPE` no `await`
            // (an611:src/generated/error_verifier.dart:2214-2223).
            if incompativel_com_await(inf, t1, 0) {
                let ini = inf.span_expr(cx.unit, e).start;
                let sp = dartforge_diagnostics::Span { start: ini, end: ini + 5 };
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::AWAIT_OF_INCOMPATIBLE_TYPE, sp, &[]);
            }
            let t = inf.flatten(t1);
            instanciar_em_contexto(inf, cx, e, t, ctx)
        }
        ExprKind::Throw(i) => {
            let t = inferir_livre(inf, cx, *i);
            uso_de_void(inf, cx, *i, t);
            // `THROW_OF_INVALID_TYPE` (an611:src/generated/error_verifier.dart:5336-5347):
            // não atribuível a `Object`; `void` relata também.
            let objeto = inf.core.object;
            if !inf.atribuivel(t, objeto) {
                let tt = inf.table.format(t, inf.interner, inf.program);
                let sp = inf.span_expr(cx.unit, *i);
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::THROW_OF_INVALID_TYPE, sp, &[&tt]);
            }
            cx.fluxo.alcancavel = false;
            inf.core.never
        }
        ExprKind::Rethrow => {
            cx.fluxo.alcancavel = false;
            inf.core.never
        }
        ExprKind::Switch { value, cases } => padroes::expressao_switch(inf, cx, *value, cases, ctx),
    };
    registrar(inf, cx, e, t);
    if matches!(inf.table.get(t), Type::Never) {
        cx.fluxo.alcancavel = false;
    }
    (t, curto)
}

impl<'a> BodyInferrer<'a> {
    /// Fecho maior se o contexto não for `_` (senão o próprio `_`).
    pub(crate) fn fecho_maior_se_conhecido(&mut self, ctx: TypeId) -> TypeId {
        if self.e_desconhecido(ctx) {
            ctx
        } else {
            self.fecho_maior(ctx)
        }
    }
}

/// `K1` de `await e` em contexto `K` (`inference.md`, "Await expressions").
fn contexto_de_await(inf: &mut BodyInferrer<'_>, k: TypeId) -> TypeId {
    if inf.e_desconhecido(k) {
        return inf.futuro_ou(k);
    }
    match inf.table.get(k) {
        Type::FutureOr { .. } => k,
        Type::Dynamic => {
            let u = inf.core.unknown;
            inf.futuro_ou(u)
        }
        _ => inf.futuro_ou(k),
    }
}

/// UP dos ramos com a regra de `inference-update-3`: se UP não cabe no
/// contexto mas os dois ramos cabem, o tipo é o contexto.
pub(crate) fn limite_superior_em_contexto(inf: &mut BodyInferrer<'_>, t1: TypeId, t2: TypeId, ctx: TypeId) -> TypeId {
    let t = inf.up(t1, t2);
    if inf.e_desconhecido(ctx) {
        return t;
    }
    let s = inf.fecho_maior(ctx);
    if inf.sub(t, s) {
        t
    } else if inf.sub(t1, s) && inf.sub(t2, s) {
        s
    } else {
        t
    }
}

/// `checkForUseOfVoidResult`: uma expressão de tipo `void` usada onde se
/// espera um valor. O intervalo é o nome do método numa invocação de método
/// (`f()` com `f` função ou método, `a.m()`); senão, a expressão inteira.
/// Devolve se relatou.
pub(crate) fn uso_de_void(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, t: TypeId) -> bool {
    if !matches!(inf.table.get(t), Type::Void) {
        return false;
    }
    let a = ast(inf, cx);
    let tipos = &inf.body_types.units[cx.unit.0 as usize];
    let metodo = |x: ExprId| -> bool {
        match tipos.get_resolved(x) {
            Some(Resolved::Element(dartforge_elements::model::Element::Function(_))) => true,
            Some(Resolved::Member { member: crate::resolved::MemberRef::Function(f), .. })
            | Some(Resolved::ExtensionMember { member: f, .. }) => {
                !matches!(inf.program.function(*f).kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor)
            }
            Some(Resolved::Local(id)) => cx.funcoes_locais.contains(id),
            _ => false,
        }
    };
    let sp = match &a.expr(e).kind {
        ExprKind::Call { target, .. } => match &a.expr(*target).kind {
            ExprKind::Property { name, .. } if metodo(*target) => name.span,
            ExprKind::Identifier(n) if metodo(*target) => n.span,
            _ => a.expr(e).span,
        },
        _ => a.expr(e).span,
    };
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
    true
}

/// `isIncompatibleWithAwait` (an611:src/dart/element/type_system.dart:971-1001):
/// `S?` com `S` incompatível; extension type que não é subtipo de
/// `Future<Object?>`; parâmetro de tipo (promovido ou não) pelo limite.
fn incompativel_com_await(inf: &mut BodyInferrer<'_>, t: TypeId, prof: u32) -> bool {
    if prof > 16 {
        return false;
    }
    match inf.table.get(t).clone() {
        Type::ExtensionType { nullable, .. } => {
            if nullable {
                let nn = inf.nao_nulo(t);
                return incompativel_com_await(inf, nn, prof + 1);
            }
            let oq = inf.core.object_nullable;
            let fut = inf.futuro(oq);
            !inf.sub(t, fut)
        }
        Type::Intersection { bound, .. } => incompativel_com_await(inf, bound, prof + 1),
        Type::TypeParameter { param, .. } => {
            let b = inf.table.param(param).bound;
            if b == t {
                return false;
            }
            incompativel_com_await(inf, b, prof + 1)
        }
        _ => false,
    }
}

/// `getErrorNode` de `checkForAssignableExpressionAtType`
/// (an611:src/generated/error_detection_helpers.dart:83-91): desce
/// parênteses e o alvo de cascata.
pub(crate) fn sem_parenteses(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> ExprId {
    let a = ast(inf, cx);
    let mut x = e;
    loop {
        match &a.expr(x).kind {
            ExprKind::Parenthesized(i) => x = *i,
            ExprKind::Cascade { target, .. } => x = *target,
            _ => return x,
        }
    }
}

/// `checkForAssignableExpressionAtType`: com o alvo não `void`, valor `void`
/// é `use_of_void_result`; senão, a atribuibilidade.
pub(crate) fn verificar_atribuivel_expr(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, de: TypeId, para: TypeId, template: &str) {
    verificar_atribuivel_expr_em(inf, cx, e, de, para, template, true);
}

/// Como [`verificar_atribuivel_expr`]; `desembrulhar` aplica o
/// `getErrorNode` (falso no lado direito de atribuição, que o analyzer
/// relata como escrito: `assignment_expression_resolver.dart:148-154`).
pub(crate) fn verificar_atribuivel_expr_em(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, de: TypeId, para: TypeId, template: &str, desembrulhar: bool) {
    if !matches!(inf.table.get(para), Type::Void) && uso_de_void(inf, cx, e, de) {
        return;
    }
    if inf.atribuivel(de, para) {
        return;
    }
    // Record de um campo posicional com `(x)` sem vírgula
    // (an611:src/generated/error_detection_helpers.dart:93-108): no parêntese.
    if let Type::Record { positional, named, .. } = inf.table.get(para).clone()
        && positional.len() == 1
        && named.is_empty()
        && !matches!(inf.table.get(de), Type::Record { .. })
        && matches!(ast(inf, cx).expr(e).kind, ExprKind::Parenthesized(_))
        && inf.atribuivel(positional[0], de)
    {
        let sp = inf.span_expr(cx.unit, e);
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA, sp, &[]);
        return;
    }
    let no = if desembrulhar { sem_parenteses(inf, cx, e) } else { e };
    let sp = inf.span_expr(cx.unit, no);
    // `_insertImplicitCallReference` (`resolver.dart:4085-4142`): a
    // expressão virou a referência implícita ao `call`, cujo tipo é o do
    // método instanciado pelo contexto; é ele que a mensagem mostra.
    let implicita = {
        let tabela = &inf.body_types.units[cx.unit.0 as usize].chamadas_implicitas;
        tabela.contains(&e) || tabela.contains(&no)
    };
    let de = match implicita.then(|| inf.tipo_do_call_implicito(de, para)).flatten() {
        Some(f) => inf.instanciar_funcao_pelo_contexto(f, para),
        None => de,
    };
    // `checkForAssignableExpressionAtType`: o erro leva o why-not-promoted
    // da expressão (`computeWhyNotPromotedMessages(expression, …)`).
    let desde = inf.diagnostics.len();
    let entidade = inf.span_expr(cx.unit, e);
    if template == ARGUMENT_TYPE_NOT_ASSIGNABLE.template {
        // `{2}`: só entre records (`error_detection_helpers.dart:109-139`).
        let mut info: Vec<String> = Vec::new();
        if let (Type::Record { positional: pa, named: na, .. }, Type::Record { positional: pe, named: ne, .. }) =
            (inf.table.get(de).clone(), inf.table.get(para).clone())
        {
            if !pe.is_empty() && pa.len() != pe.len() {
                info.push(format!("Expected {} positional arguments, but got {} instead.", pe.len(), pa.len()));
            }
            if !ne.is_empty() && na.len() != ne.len() {
                info.push(format!("Expected {} named arguments, but got {} instead.", ne.len(), na.len()));
            }
            if !ne.is_empty() {
                let mut nomeados: Vec<(String, TypeId)> = na.iter().map(|(n, t)| (inf.interner.resolve(*n).to_string(), *t)).collect();
                nomeados.sort_by(|a, b| a.0.cmp(&b.0));
                for (nome, t) in nomeados {
                    if !ne.iter().any(|(n2, t2)| inf.interner.resolve(*n2) == nome && *t2 == t) {
                        let tt = inf.table.format_sem_alias(t, inf.interner, inf.program);
                        info.push(format!("Unexpected named argument `{nome}` with type `{tt}`."));
                    }
                }
            }
        }
        let a3 = info.join(" ");
        inf.aviso_com_args(
            dartforge_diagnostics::codigos::compile_time_error::ARGUMENT_TYPE_NOT_ASSIGNABLE,
            sp,
            &[crate::exibicao::Arg::Tipo(de), crate::exibicao::Arg::Tipo(para), crate::exibicao::Arg::Texto(a3.into())],
        );
        inf.anexar_nao_promocao(desde, cx, Some(e), entidade);
        return;
    }
    inf.verificar_atribuivel(de, para, sp, template);
    inf.anexar_nao_promocao(desde, cx, Some(e), entidade);
}

/// Um operando que fecha um bloco básico (o `flowEnd` do
/// `NullSafetyDeadCodeVerifier`, `an611:src/error/dead_code_verifier.dart:213-330`):
/// os ramos de `?:` e o lado direito de `&&`/`||`. Se ele começa
/// inalcançável e não há trecho morto aberto, ele é o primeiro nó morto e o
/// trecho vai dele (ou do operador, `inicio`, quando o pai é binário) ao fim
/// dele; os nós de dentro fazem parte do mesmo trecho.
fn operando_de_fluxo<R>(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    e: ExprId,
    inicio: Option<usize>,
    f: impl FnOnce(&mut BodyInferrer<'_>, &mut Corpo) -> R,
) -> R {
    let sp = inf.span_expr(cx.unit, e);
    let abre = !cx.fluxo.alcancavel && cx.trecho_morto.is_none();
    // `f()` com `f` local de tipo função: o analyzer troca o nó
    // (`MethodInvocation` → `FunctionExpressionInvocation`), e o `flowEnd`
    // deste operando não o encontra mais (`_containsFirstDeadNode`): o
    // trecho só fecha no fim do bloco básico de fora.
    let reescrito = abre && chamada_de_local(inf, cx, e) && !cx.fins_de_fluxo.is_empty();
    if abre {
        let fim = if reescrito { cx.fins_de_fluxo.last().copied().unwrap_or(sp.end).max(sp.end) } else { sp.end };
        let span = dartforge_diagnostics::Span { start: inicio.unwrap_or(sp.start), end: fim };
        inf.aviso(DEAD_CODE.template.to_string(), span);
        if reescrito {
            cx.trecho_morto = Some(cx.fins_de_fluxo.len());
        }
    }
    super::instrucoes::entrar_fluxo(cx, sp.end);
    if abre && !reescrito {
        cx.trecho_morto = Some(cx.fins_de_fluxo.len());
    }
    let r = f(inf, cx);
    super::instrucoes::sair_fluxo(cx);
    r
}

/// A primeira seção de uma cascata `c?..s` é um acesso que o analyzer não
/// reescreve: propriedade, índice, ou invocação de **método**. `..g()` com
/// `g` getter (ou campo) vira invocação de expressão, e o `?..` sai sem
/// relato.
fn primeira_secao_sem_reescrita(inf: &mut BodyInferrer<'_>, cx: &Corpo, secao: ExprId, t: TypeId) -> bool {
    let a = ast(inf, cx);
    // Desce até o acesso aplicado ao alvo da cascata, lembrando o pai.
    let mut pai: Option<ExprId> = None;
    let mut x = secao;
    loop {
        let alvo = match &a.expr(x).kind {
            ExprKind::Call { target, .. } | ExprKind::Property { target, .. } | ExprKind::Index { target, .. } => *target,
            ExprKind::Assign { target, .. } => *target,
            _ => return true,
        };
        if matches!(a.expr(alvo).kind, ExprKind::CascadeTarget) {
            break;
        }
        pai = Some(x);
        x = alvo;
    }
    let ExprKind::Property { name, .. } = &a.expr(x).kind else { return true };
    let invocada = pai.is_some_and(|p| matches!(&a.expr(p).kind, ExprKind::Call { target, .. } if *target == x));
    if !invocada {
        return true;
    }
    let nome = name.sym;
    let nn = inf.nao_nulo(t);
    !matches!(inf.buscar_membro(cx.lib, nn, nome, false), Busca::Achado(m) if !m.metodo)
}

/// `e` é `f(...)` com `f` uma variável local (não função local).
fn chamada_de_local(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    let a = ast(inf, cx);
    let ExprKind::Call { target, .. } = &a.expr(e).kind else { return false };
    let ExprKind::Identifier(n) = &a.expr(*target).kind else { return false };
    match cx.buscar(n.sym) {
        Some(super::corpo::Nome::Local(id)) => !cx.funcoes_locais.contains(&id),
        _ => false,
    }
}

/// `RECEIVER_OF_TYPE_NEVER` (`binary_expression_resolver.dart:422-428`,
/// `method_invocation_resolver.dart:543`, `property_element_resolver.dart:92`,
/// `function_expression_invocation_resolver.dart:66`): o receptor de um
/// operador, índice, método ou invocação tem tipo `Never` (não `Never?`).
/// Relata no receptor e diz se relatou.
pub(crate) fn receptor_nunca(inf: &mut BodyInferrer<'_>, cx: &Corpo, r: ExprId, t: TypeId) -> bool {
    if !matches!(inf.table.get(t), Type::Never) || matches!(ast(inf, cx).expr(r).kind, ExprKind::Super) {
        return false;
    }
    // Numa seção de cascata, o receptor é o alvo da cascata (`realTarget`).
    let r = if matches!(ast(inf, cx).expr(r).kind, ExprKind::CascadeTarget) { cx.alvos_de_cascata.last().copied().unwrap_or(r) } else { r };
    let sp = inf.span_expr(cx.unit, r);
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::warning::RECEIVER_OF_TYPE_NEVER, sp, &[]);
    true
}

/// `RecordLiteralResolver._resolveField` (3.6.2 `record_literal_resolver.dart:135-148`):
/// o campo `dynamic` com contexto conhecido vale o fecho maior do contexto,
/// quando `dynamic` não é subtipo dele (o cast implícito).
fn cast_de_dynamic_no_campo(inf: &mut BodyInferrer<'_>, t: TypeId, contexto: TypeId) -> TypeId {
    if contexto == inf.core.unknown || !matches!(inf.table.get(t), Type::Dynamic) {
        return t;
    }
    let fecho = inf.fecho_maior(contexto);
    if inf.sub(t, fecho) { t } else { fecho }
}

/// `DEAD_NULL_AWARE_EXPRESSION` (`_checkForDeadNullCoalesce`,
/// `an611:src/generated/error_verifier.dart:3045-3052`): em `a ?? b` e
/// `a ??= b`, com `a` estritamente não anulável, o lado direito.
fn avisar_nulo_morto(inf: &mut BodyInferrer<'_>, cx: &Corpo, esquerdo: TypeId, direito: ExprId) {
    if estritamente_nao_anulavel(inf, esquerdo) {
        let sp = inf.span_expr(cx.unit, direito);
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::static_warning::DEAD_NULL_AWARE_EXPRESSION, sp, &[]);
    }
}

/// `t` é estritamente não anulável (`isStrictlyNonNullable`): nem `dynamic`,
/// `void`, `Null` ou anulável; parâmetro de tipo pelo limite; tipo de
/// extensão só com `implements` (aqui, nunca: pelo lado seguro).
pub(crate) fn estritamente_nao_anulavel(inf: &mut BodyInferrer<'_>, t: TypeId) -> bool {
    // `TypeSystemImpl.isStrictlyNonNullable` (3.6.2 `type_system.dart:1290-1310`).
    let mut t = t;
    for _ in 0..64 {
        if inf.table.e_invalido(t) || inf.e_desconhecido(t) {
            return false;
        }
        let ty = inf.table.get(t).clone();
        if matches!(ty, Type::Dynamic | Type::Void | Type::Null) || ty.is_declared_nullable() {
            return false;
        }
        match ty {
            Type::FutureOr { arg, .. } => t = arg,
            // O tipo de extensão só com `implements` (`type.interfaces`).
            Type::ExtensionType { decl, .. } => return !inf.outline.classes[decl.0 as usize].interfaces.is_empty(),
            // `TypeParameterType.bound`: o promovido, senão o declarado.
            Type::TypeParameter { param, .. } => {
                let b = inf.table.param(param).bound;
                if b == t {
                    return true;
                }
                t = b;
            }
            Type::Intersection { bound, .. } => t = bound,
            _ => return true,
        }
    }
    true
}

/// `_checkForUnnecessaryNullAware` para `?.` e `?[`: receptor estritamente
/// não anulável. O intervalo é o operador (`?.`, ou `?[` inteiro); com um
/// `?.`/`?[` anterior na mesma cadeia, é o `…_AFTER_SHORT_CIRCUIT`.
fn operador_nulo_desnecessario(inf: &mut BodyInferrer<'_>, cx: &Corpo, r: ExprId, t: TypeId) {
    if matches!(ast(inf, cx).expr(r).kind, ExprKind::Super) || !estritamente_nao_anulavel(inf, t) {
        return;
    }
    relatar_operador_nulo(inf, cx, r, true);
}

/// `C?.x`, `E?.m()`, `Alias?.x`: o receptor é um literal de tipo (sem tipo
/// estático no analyzer, `targetElement` de classe, extensão ou alias,
/// `error_verifier.dart:5613-5627`): o operador é desnecessário.
pub(crate) fn operador_nulo_em_tipo(inf: &mut BodyInferrer<'_>, cx: &Corpo, r: ExprId) {
    let simples = match &ast(inf, cx).expr(r).kind {
        ExprKind::Identifier(_) => true,
        ExprKind::Property { target, .. } => matches!(ast(inf, cx).expr(*target).kind, ExprKind::Identifier(_)),
        _ => false,
    };
    if simples {
        relatar_operador_nulo(inf, cx, r, false);
    }
}

/// O `INVALID_NULL_AWARE_OPERATOR` no operador que segue `r` (`?.`, `?..`
/// ou `?[`); `cadeia`: com um `?.` anterior na mesma cadeia, é o
/// `…_AFTER_SHORT_CIRCUIT`.
fn relatar_operador_nulo(inf: &mut BodyInferrer<'_>, cx: &Corpo, r: ExprId, cadeia: bool) {
    use dartforge_diagnostics::codigos::static_warning as w;
    let fim = inf.span_expr(cx.unit, r).end;
    let fonte = &inf.program.unit(cx.unit).source;
    let resto = fonte.get(fim..).unwrap_or("");
    let pos = fim + pular_espacos_e_comentarios(resto);
    let depois = fonte.get(pos..).unwrap_or("");
    let (tamanho, args): (usize, [&str; 2]) = if depois.starts_with("?..") {
        (3, ["?..", ".."])
    } else if depois.starts_with("?.") {
        (2, ["?.", "."])
    } else if depois.starts_with('?') {
        // `?[`: do `?` ao `[` (com o que houver entre eles).
        let Some(i) = depois.find('[') else { return };
        (i + 1, ["?[", "["])
    } else {
        return;
    };
    let sp = dartforge_diagnostics::Span { start: pos, end: pos + tamanho };
    // `invalidNullAwareAfterShortCircuit`: o contexto é o operador que
    // encurta.
    match (cadeia && args[0] != "?..").then(|| operador_curto_anterior(inf, cx, r)).flatten() {
        Some((anterior, lexema)) => inf.aviso_com_contexto(
            w::INVALID_NULL_AWARE_OPERATOR_AFTER_SHORT_CIRCUIT,
            sp,
            &args,
            vec![(None, anterior, format!("The operator '{lexema}' is causing the short circuiting."))],
        ),
        None => inf.aviso_com_codigo(w::INVALID_NULL_AWARE_OPERATOR, sp, &args),
    }
}

/// `...?e` com `e` estritamente não anulável: no `...?`.
pub(crate) fn espalhamento_nulo_desnecessario(inf: &mut BodyInferrer<'_>, cx: &Corpo, valor: ExprId, t: TypeId) {
    use dartforge_diagnostics::codigos::static_warning as w;
    if !estritamente_nao_anulavel(inf, t) {
        return;
    }
    let ini = inf.span_expr(cx.unit, valor).start;
    let fonte = &inf.program.unit(cx.unit).source;
    let antes = fonte.get(..ini).unwrap_or("").trim_end();
    if let Some(pos) = antes.strip_suffix("...?").map(str::len) {
        let sp = dartforge_diagnostics::Span { start: pos, end: pos + 4 };
        inf.aviso_com_codigo(w::INVALID_NULL_AWARE_OPERATOR, sp, &["...?", "..."]);
    }
}

/// `previousShortCircuitingOperator` (`error_verifier.dart:5583-5605`): o
/// operador `?.`/`?` (do `?[`) de `r` que encurta a cadeia, o mais fundo
/// primeiro; só pelos acessos que são eles mesmos `?.`/`?[` (um `.` no meio
/// encerra a busca). O intervalo e o lexema do token.
fn operador_curto_anterior(inf: &BodyInferrer<'_>, cx: &Corpo, r: ExprId) -> Option<(dartforge_diagnostics::Span, &'static str)> {
    let fonte = &inf.program.unit(cx.unit).source;
    let token_depois = |alvo: ExprId, lexema: &'static str| -> Option<(dartforge_diagnostics::Span, &'static str)> {
        let fim = inf.span_expr(cx.unit, alvo).end;
        let pos = fim + pular_espacos_e_comentarios(fonte.get(fim..).unwrap_or(""));
        fonte.get(pos..)?.starts_with(lexema).then_some((dartforge_diagnostics::Span { start: pos, end: pos + lexema.len() }, lexema))
    };
    match &ast(inf, cx).expr(r).kind {
        ExprKind::Property { target, null_aware: true, .. } => operador_curto_anterior(inf, cx, *target).or_else(|| token_depois(*target, "?.")),
        ExprKind::Index { target, null_aware: true, .. } => operador_curto_anterior(inf, cx, *target).or_else(|| token_depois(*target, "?")),
        ExprKind::Call { target, .. } => match &ast(inf, cx).expr(*target).kind {
            ExprKind::Property { target: t2, null_aware: true, .. } => operador_curto_anterior(inf, cx, *t2).or_else(|| token_depois(*t2, "?.")),
            _ => None,
        },
        _ => None,
    }
}

/// Infere o receptor de um acesso (`r.x`, `r[i]`, `r.m()`), tratando `?.` e
/// o *null-shorting*. Devolve o tipo do receptor para a busca e se há curto.
pub(crate) fn receptor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, r: ExprId, null_aware: bool) -> (TypeId, bool) {
    let u = inf.core.unknown;
    let (t, c) = inferir_no(inf, cx, r, u, true);
    if null_aware {
        operador_nulo_desnecessario(inf, cx, r, t);
        // Promove o receptor dentro da cadeia; `fechar_cadeia` desfaz no fim.
        cx.cadeias.push(cx.fluxo.clone());
        if let Some(id) = alvo_de_promocao(inf, cx, r) {
            let decl = cx.local(id).tipo;
            let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
            inf.promover_nao_nulo(&mut f, id, decl);
            cx.fluxo = f;
        }
        // `promoteToNonNull` do alvo (o parâmetro de tipo de limite anulável
        // vira `T & NonNull(limite)`: `nn?.toRadixString(16)` com
        // `NN extends int?`).
        let nn = match inf.table.get(t) {
            Type::TypeParameter { nullable: false, .. } => inf.nao_nulo_promocao(t),
            _ => inf.nao_nulo(t),
        };
        (nn, true)
    } else {
        (t, c)
    }
}

/// Busca de membro no receptor `target`: sobreposição explícita de
/// extensão (`E(x).m`) consulta só a extensão; senão a busca normal.
pub(crate) fn buscar_membro_do_alvo(inf: &mut BodyInferrer<'_>, cx: &Corpo, target: ExprId, recv: TypeId, nome: SymbolId, setter: bool) -> Busca {
    if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned() {
        return match inf.membro_de_extensao_explicita(x, &args, nome, setter) {
            Some(m) => Busca::Achado(m),
            None => Busca::Ausente,
        };
    }
    inf.buscar_membro(cx.lib, recv, nome, setter)
}

/// `target.name` (leitura).
/// `startNullAwarePropertyAccess`/`visitMethodInvocation` (3.6.2,
/// `resolver.dart:1777-1794`, `:3215-3226`): o `?.` sobre um literal de tipo
/// só deixa de encurtar quando o alvo é um `SimpleIdentifier` que nomeia um
/// `InterfaceElement` (`C?.x` é `C.x`); `p.C?.x`, `C<int>?.x`, um alias ou
/// uma extensão encurtam (o resultado fica anulável).
pub(crate) fn encurtamento_dispensado(inf: &BodyInferrer<'_>, cx: &Corpo, target: ExprId, rt: &RefTipo) -> bool {
    matches!(ast(inf, cx).expr(target).kind, ExprKind::Identifier(_)) && matches!(rt, RefTipo::Classe(..))
}

/// `PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT` no nome do prefixo.
pub(crate) fn prefixo_sem_ponto(inf: &mut BodyInferrer<'_>, n: ast::Name) {
    let nome = inf.interner.resolve(n.sym).to_string();
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT, n.span, &[&nome]);
}

fn propriedade(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, target: ExprId, name: ast::Name, null_aware: bool) -> (TypeId, bool) {
    let a = ast(inf, cx);
    // `p.nome` com prefixo de import.
    if let ExprKind::Identifier(p) = &a.expr(target).kind {
        if matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
            // `p?.x` é `PropertyAccess`, não `PrefixedIdentifier`: o prefixo
            // não vale ali (`_isValidAsPrefix`).
            if null_aware {
                prefixo_sem_ponto(inf, *p);
            }
            resolver(inf, cx, target, Resolved::Prefix(cx.lib));
            if importacao_ambigua(inf, cx, Some(p.sym), name) {
                return (inf.table.invalido(inf.core.dynamic_), false);
            }
            match inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter.or(b.setter)) {
                Some(el) => {
                    resolver(inf, cx, e, Resolved::Element(el));
                    if let Element::Extension(_) = el {
                        let sp = inf.span_expr(cx.unit, e);
                        if extensao_como_expressao(inf, cx, e, sp) {
                            return (inf.core.dynamic_, false);
                        }
                    }
                    return (ler_elemento(inf, el), false);
                }
                None => {
                    if let Some(t) = load_library(inf, cx, p.sym, name.sym) {
                        return (t, false);
                    }
                    avisar_nome_prefixado_indefinido(inf, cx, p.sym, name);
                    return (inf.core.dynamic_, false);
                }
            }
        }
    }
    // `C.x`: estático, constante de enum ou tear-off de construtor.
    if let Some(rt) = referencia_a_tipo(inf, cx, target) {
        registrar_ref_tipo(inf, cx, target);
        if null_aware {
            operador_nulo_em_tipo(inf, cx, target);
        }
        let curto = null_aware && !encurtamento_dispensado(inf, cx, target, &rt);
        return (acesso_estatico(inf, cx, e, rt, name), curto);
    }
    // `super.x`. Com `super?.x` (`INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER`
    // no parser), o acesso continua null-aware: o tipo sai anulável no fim
    // da cadeia (`int y = super?.x` é `INVALID_ASSIGNMENT` de `int?`).
    if matches!(a.expr(target).kind, ExprKind::Super) {
        let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
        registrar(inf, cx, target, this);
        return (membro_super(inf, cx, e, name, UsoDoSuper::Leitura), null_aware);
    }
    let (recv, curto) = receptor(inf, cx, target, null_aware);
    // `x.new` com `x` valor: o `TypePropertyResolver` nunca acha `new`
    // (`type_property_resolver.dart:75-79`), nem em `dynamic` nem no anulável.
    if !cx.sobreposicoes.contains_key(&target) && inf.interner.resolve(name.sym) == "new" {
        let tipo = inf.table.format(recv, inf.interner, inf.program);
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_GETTER, name.span, &["new", &tipo]);
        return (inf.table.invalido(inf.core.dynamic_), curto);
    }
    if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned() {
        if inf.membro_de_extensao_explicita(x, &args, name.sym, false).is_none()
            && inf.membro_estatico_de_extensao(x, name.sym, false).is_some()
        {
            inf.aviso(EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(), name.span);
            return (inf.core.dynamic_, curto);
        }
    }
    let base = if null_aware { None } else { base_de_propriedade(inf, cx, target) };
    // `void`: o valor não pode ser usado (analyzer: `use_of_void_result`, no nome).
    if matches!(inf.table.get(recv), Type::Void) {
        // Em `x..p`, o relato é no alvo da cascata (feito lá).
        if !matches!(a.expr(target).kind, ExprKind::CascadeTarget) {
            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, name.span, &[]);
        }
        return (inf.core.dynamic_, curto);
    }
    // Receptor potencialmente anulável sem o membro em `Object` nem numa
    // extensão: `unchecked_use_of_nullable_value` no nome, e a leitura segue
    // pelo tipo não anulável só para recuperar o tipo (sem `undefined_getter`).
    let checar_nulo = !cx.sobreposicoes.contains_key(&target) && inf.exige_checagem_de_nulo(cx.lib, recv, name.sym, false);
    if checar_nulo {
        let nome = inf.interner.resolve(name.sym).to_string();
        let desde = inf.diagnostics.len();
        inf.aviso_de_nulo(
            recv,
            dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
            name.span,
            &[&nome],
        );
        // `computeWhyNotPromotedMessages(nameErrorEntity, whyNotPromoted(receiver))`.
        inf.anexar_nao_promocao(desde, cx, Some(target), name.span);
    }
    let busca = buscar_membro_do_alvo(inf, cx, target, recv, name.sym, false);
    inf.relatar_ambiguidade_de_extensao(name.span);
    let t = match busca {
        Busca::Achado(m) => {
            resolver(inf, cx, e, m.resolved.clone());
            match base {
                Some(b) => leitura_de_campo(inf, cx, e, b, m.tipo),
                None => m.tipo,
            }
        }
        Busca::Ausente if checar_nulo => match inf.acesso_de_instancia_a_estatico(cx.lib, recv, name.sym, false, name.span) {
            Some(m) => {
                resolver(inf, cx, e, m.resolved.clone());
                m.tipo
            }
            None => inf.core.dynamic_,
        },
        // Receptor `InvalidType`: nenhum membro é procurado, e o resultado
        // é o próprio inválido (`property_element_resolver`, T4.1 item 2).
        Busca::Dinamico if inf.table.e_invalido(recv) => {
            resolver(inf, cx, e, Resolved::Dynamic);
            recv
        }
        Busca::Dinamico => {
            resolver(inf, cx, e, Resolved::Dynamic);
            // `hashCode`, `runtimeType`, `toString`… de `Object` valem também em `dynamic`.
            let o = inf.core.object;
            match inf.membro_de_interface(o, name.sym, false) {
                Some(m) => m.tipo,
                None => inf.core.dynamic_,
            }
        }
        // Receptor `Never` (analyzer 3.6.2): na forma `id.x`
        // (`PrefixedIdentifier`) o resultado é `Never`; nas outras
        // (`PropertyAccess`, `falha().x`) valem só os membros de `Object`, e
        // um nome que não é de `Object` fica com o tipo de recuperação
        // (`InvalidType`, que se comporta como `dynamic`).
        Busca::Nunca => {
            let prefixado = !null_aware && matches!(a.expr(target).kind, ExprKind::Identifier(_));
            let o = inf.core.object;
            match inf.membro_de_interface(o, name.sym, false) {
                _ if prefixado => inf.core.never,
                Some(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    m.tipo
                }
                None => {
                    inf.body_types.units[cx.unit.0 as usize].tipos_invalidos.insert(e);
                    inf.table.invalido(inf.core.dynamic_)
                }
            }
        }
        Busca::Ausente => {
            if let Some((x, _)) = cx.sobreposicoes.get(&target).cloned() {
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_GETTER.template, inf.interner.resolve(name.sym), extensao);
                inf.aviso(msg, name.span);
                return (inf.core.dynamic_, curto);
            }
            if let Some(m) = inf.acesso_de_instancia_a_estatico(cx.lib, recv, name.sym, false, name.span) {
                resolver(inf, cx, e, m.resolved.clone());
                return (m.tipo, curto);
            }
            let msg = format!(
                "{}: getter '{}' não definido para o tipo '{}'",
                UNDEFINED_GETTER.template,
                inf.interner.resolve(name.sym),
                inf.table.format(recv, inf.interner, inf.program)
            );
            inf.aviso(msg, name.span);
            // Leitura sem elemento num alvo que não é dinâmico: o tipo de
            // recuperação (`generated/resolver.dart:1668`).
            inf.table.invalido(inf.core.dynamic_)
        }
    };
    (t, curto)
}

/// `WRONG_NUMBER_OF_TYPE_ARGUMENTS` de uma classe ou alias instanciado
/// como expressão (`e` é o `TypeArguments`). Como literal de tipo
/// (`_resolveDirectTypeLiteral`/`_resolveTypeAlias`,
/// `function_reference_resolver.dart:115-150, 316-327, 821-835`), na lista
/// de argumentos; com `no_tipo_nomeado` (o `NamedType` de `C<T>.x` e da
/// criação implícita, `NamedTypeResolver._buildTypeArguments`), do nome ao
/// `>`. O nome é o escrito (o do alias, se for um).
pub(crate) fn conferir_argumentos_do_literal(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, no_tipo_nomeado: bool) {
    let a = ast(inf, cx);
    let ExprKind::TypeArguments { target, type_args } = &a.expr(e).kind else { return };
    let (target, n_args) = (*target, type_args.len());
    let ultimo_fim = type_args.last().map(|&t| a.ty(t).span.end);
    let (el, nome) = match &a.expr(target).kind {
        ExprKind::Identifier(n) => match resolver_nome(inf, cx, n.sym, false) {
            RefNome::Elemento(el) => (el, *n),
            _ => return,
        },
        ExprKind::Property { target: p, name, null_aware: false } => {
            let ExprKind::Identifier(p) = &a.expr(*p).kind else { return };
            if !matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
                return;
            }
            let Some(el) = inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter) else { return };
            (el, *name)
        }
        _ => return,
    };
    let parametros = match el {
        Element::Class(c) => inf.outline.classes[c.0 as usize].type_params.len(),
        Element::Typedef(td) => inf.outline.typedefs[td.0 as usize].type_params.len(),
        _ => return,
    };
    if parametros == n_args {
        return;
    }
    let fonte = &inf.program.unit(cx.unit).source;
    let alvo = a.expr(target).span;
    let depois_do_nome = alvo.end;
    let abre = fonte.get(depois_do_nome..).and_then(|r| r.find('<')).map(|i| depois_do_nome + i).unwrap_or(depois_do_nome);
    let fim_args = ultimo_fim.unwrap_or(abre + 1);
    let fecha = fonte.get(fim_args..).and_then(|r| r.find('>')).map(|i| fim_args + i + 1).unwrap_or(fim_args);
    let span = if no_tipo_nomeado {
        dartforge_diagnostics::Span { start: alvo.start, end: fecha }
    } else {
        dartforge_diagnostics::Span { start: abre, end: fecha }
    };
    let (texto, p, n) = (inf.interner.resolve(nome.sym).to_string(), parametros.to_string(), n_args.to_string());
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS, span, &[&texto, &p, &n]);
}

/// Acesso estático `C.x` / `E.x` / `C.new`.
fn acesso_estatico(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, rt: RefTipo, name: ast::Name) -> TypeId {
    let instancia_explicita = receptor_de_instanciacao_explicita(inf, cx, e);
    if instancia_explicita && let ExprKind::Property { target, .. } = &ast(inf, cx).expr(e).kind {
        conferir_argumentos_do_literal(inf, cx, *target, true);
    }
    match rt {
        RefTipo::Extensao(x) => match inf.membro_estatico_de_extensao(x, name.sym, false) {
            Some(m) => {
                resolver(inf, cx, e, m.resolved.clone());
                m.tipo
            }
            None => {
                if inf.program.extension(x).instance_members.contains_key(&name.sym) {
                    let msg = format!("{}: '{}'", STATIC_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
                    inf.aviso(msg, name.span);
                    return inf.core.dynamic_;
                }
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_GETTER.template, inf.interner.resolve(name.sym), extensao);
                inf.aviso(msg, name.span);
                inf.core.dynamic_
            }
        },
        RefTipo::Classe(c, targs) => {
            if let Some(m) = inf.membro_estatico(c, name.sym, false) {
                if instancia_explicita {
                    avisar_instanciacao_estatica(inf, cx, e, name);
                    return inf.core.dynamic_;
                }
                resolver(inf, cx, e, m.resolved.clone());
                return m.tipo;
            }
            let args = targs.map(|v| v.iter().map(|&t| inf.tipo_de_argumento_de_tipo(cx, t)).collect::<Vec<_>>());
            tearoff_de_construtor(inf, cx, e, c, args, name, true, false)
        }
        RefTipo::Alias(c, args, td) => {
            if let Some(m) = inf.membro_estatico(c, name.sym, false) {
                if instancia_explicita {
                    avisar_instanciacao_estatica(inf, cx, e, name);
                    return inf.core.dynamic_;
                }
                resolver(inf, cx, e, m.resolved.clone());
                return m.tipo;
            }
            if !instancia_explicita && args.is_some() && !inf.outline.typedefs[td.0 as usize].type_params.is_empty() {
                return tearoff_generico_de_alias(inf, cx, e, c, td, name);
            }
            // `X<T>.m` instanciado explicitamente: o mesmo
            // `CLASS_INSTANTIATION_ACCESS_TO_MEMBER` da classe.
            tearoff_de_construtor(inf, cx, e, c, args, name, instancia_explicita, true)
        }
    }
}

fn receptor_de_instanciacao_explicita(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    let ExprKind::Property { target, .. } = &ast(inf, cx).expr(e).kind else { return false };
    matches!(&ast(inf, cx).expr(*target).kind, ExprKind::TypeArguments { .. })
}

/// Somente membros declarados na própria classe; a busca herdada requer
/// resolver substituições e precedência antes de diagnosticar.
fn membro_instancia_direto_visivel(inf: &BodyInferrer<'_>, cx: &Corpo, classe: ClassId, name: ast::Name, escrita: bool) -> bool {
    if inf.interner.resolve(name.sym).starts_with('_') && inf.program.class(classe).library != cx.lib {
        return false;
    }
    let membros = &inf.program.class(classe).instance_members;
    membros.contains_key(&name.sym)
        || (escrita && inf.chave_setter(name.sym).is_some_and(|chave| membros.contains_key(&chave)))
}

fn avisar_acesso_estatico_a_instancia(inf: &mut BodyInferrer<'_>, cx: &Corpo, classe: ClassId, name: ast::Name, escrita: bool) -> bool {
    let encontrado = membro_instancia_direto_visivel(inf, cx, classe, name, escrita);
    if encontrado {
        let msg = format!("{}: '{}'", STATIC_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
        inf.aviso(msg, name.span);
    }
    encontrado
}

fn avisar_instanciacao_de_classe(inf: &mut BodyInferrer<'_>, cx: &Corpo, expr: ExprId, classe: ClassId, name: ast::Name, escrita: bool) -> bool {
    if !membro_instancia_direto_visivel(inf, cx, classe, name, escrita) { return false; }
    let msg = format!("{}: '{}'", CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
    let span = ast(inf, cx).expr(expr).span;
    inf.aviso(msg, span);
    true
}

fn avisar_instanciacao_estatica(inf: &mut BodyInferrer<'_>, cx: &Corpo, expr: ExprId, name: ast::Name) {
    let msg = format!("{}: '{}'", CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER.template, inf.interner.resolve(name.sym));
    let span = ast(inf, cx).expr(expr).span;
    inf.aviso(msg, span);
}

fn avisar_instanciacao_desconhecida(inf: &mut BodyInferrer<'_>, cx: &Corpo, expr: ExprId, classe: ClassId, name: ast::Name) {
    // O identificador sintético da recuperação (`A<int>.;`) não é resolvido.
    if name.span.start == name.span.end {
        return;
    }
    let classe = inf.interner.resolve(inf.program.class(classe).name);
    let membro = inf.interner.resolve(name.sym);
    let msg = format!("{}: '{classe}', '{membro}'", CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER.template);
    let span = ast(inf, cx).expr(expr).span;
    inf.aviso(msg, span);
}

/// `C.nome` / `C.new` como valor: tipo de função do construtor (genérico
/// sobre os parâmetros da classe se não instanciado).
/// `de_alias`: o receptor é um alias de tipo; instanciado (`X<T>.new`), o
/// `AstRewriter` não o reescreve em referência de construtor, e o `new`
/// ausente é membro desconhecido da instanciação como qualquer outro nome.
#[allow(clippy::too_many_arguments)]
fn tearoff_de_construtor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, c: ClassId, args: Option<Vec<TypeId>>, name: ast::Name, diagnosticar_ausencia: bool, de_alias: bool) -> TypeId {
    let instancia_explicita = receptor_de_instanciacao_explicita(inf, cx, e);
    let chave = if Some(name.sym) == inf.sym.new_ { inf.sym.vazio } else { Some(name.sym) };
    let Some(chave) = chave else { return inf.core.dynamic_ };
    // O primário de um tipo de extensão (`E.new`, `E.nome`) não é elemento:
    // `Some(None)`, com a assinatura `(Representação) -> E`.
    let primario = inf.construtor_ou_primario(c, chave) == Some(None);
    let construtor = inf.construtor_de(c, chave);
    // Construtores geradores de enum só criam as constantes do próprio enum.
    // O sem nome implícito não aparece na tabela, mas `E.new` também é sua
    // referência. Factories declaradas no enum podem ser referenciadas.
    if inf.program.class(c).kind == dartforge_elements::model::ClassKind::Enum
        && (construtor.is_some_and(|f| !inf.program.function(f).factory)
            || (construtor.is_none() && Some(name.sym) == inf.sym.new_))
    {
        inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), ast(inf, cx).expr(e).span);
        return inf.core.dynamic_;
    }
    // `ConstructorReferenceResolver.resolve`
    // (`an611:src/dart/resolver/constructor_reference_resolver.dart:31-41`):
    // gerador (também o sintético sem nome, que o modelo não cria) de classe
    // abstrata, no `ConstructorReference` inteiro.
    let abstrata = {
        let ce = inf.program.class(c);
        matches!(ce.kind, dartforge_elements::model::ClassKind::Class | dartforge_elements::model::ClassKind::MixinApplication) && ce.modifiers.abstract_
    };
    let sintetico = construtor.is_none() && !primario && Some(chave) == inf.sym.vazio && super::funcoes::sem_nome_implicito(inf, c);
    if abstrata && (sintetico || construtor.is_some_and(|f| !inf.program.function(f).factory)) {
        let sp = ast(inf, cx).expr(e).span;
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::TEAROFF_OF_GENERATIVE_CONSTRUCTOR_OF_ABSTRACT_CLASS, sp, &[]);
    }
    if sintetico {
        let ret = inf.tipo_this_classe(c);
        let sig = inf.table.intern(Type::Function {
            type_params: Box::new([]),
            ret,
            positional: Box::new([]),
            optional: Box::new([]),
            named: Box::new([]),
            nullable: false,
        });
        return tearoff_com_argumentos(inf, c, sig, args);
    }
    let Some(f) = construtor else {
        if primario {
            let sig = inf.assinatura_primario(c);
            return tearoff_com_argumentos(inf, c, sig, args);
        }
        // `C<T>.new` ainda é uma referência ao construtor sem nome. A
        // ausência dele tem diagnóstico no token `new`, mesmo quando o
        // receptor foi instanciado explicitamente.
        if diagnosticar_ausencia && Some(name.sym) == inf.sym.new_ && !(de_alias && instancia_explicita) {
            let classe = inf.interner.resolve(inf.program.class(c).name);
            let msg = if instancia_explicita {
                format!("{}: '{classe}', 'new'", NEW_WITH_UNDEFINED_CONSTRUCTOR.template)
            } else {
                format!("{}: '{classe}'", NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT.template)
            };
            inf.aviso(msg, name.span);
            return inf.core.dynamic_;
        }
        if instancia_explicita && avisar_instanciacao_de_classe(inf, cx, e, c, name, false) {
            return inf.core.dynamic_;
        }
        if instancia_explicita && diagnosticar_ausencia {
            avisar_instanciacao_desconhecida(inf, cx, e, c, name);
            return inf.core.dynamic_;
        }
        if !instancia_explicita && avisar_acesso_estatico_a_instancia(inf, cx, c, name, false) {
            return inf.core.dynamic_;
        }
        // `PropertyElementResolver` (`property_element_resolver.dart:696-705`):
        // num enum o código é `UNDEFINED_ENUM_CONSTANT`.
        if inf.program.class(c).kind == dartforge_elements::model::ClassKind::Enum {
            let (n, en) = (inf.interner.resolve(name.sym).to_string(), inf.interner.resolve(inf.program.class(c).name).to_string());
            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_ENUM_CONSTANT, name.span, &[&n, &en]);
            return inf.core.dynamic_;
        }
        // `[propertyName.name, typeReference.name]`
        // (`property_element_resolver.dart:697-705`): o nome da classe, também
        // pelo alias.
        let (n, classe) = (inf.interner.resolve(name.sym).to_string(), inf.interner.resolve(inf.program.class(c).name).to_string());
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_GETTER, name.span, &[&n, &classe]);
        return inf.core.dynamic_;
    };
    if inf.program.function(f).class == Some(c) {
        resolver(inf, cx, e, Resolved::Constructor(f));
    }
    let sig = inf.assinatura_construtor(c, f);
    tearoff_com_argumentos(inf, c, sig, args)
}

/// `F.new`/`F.nome` por um alias genérico que não só repassa os parâmetros
/// (`typedef F<T> = C<int, T>`), sem argumentos de tipo: a função genérica
/// nos parâmetros do alias, `C<int, T> Function<T>(…)` (como o analyzer e o
/// CFE), que a instanciação implícita do contexto fecha depois.
fn tearoff_generico_de_alias(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    e: ExprId,
    c: ClassId,
    td: dartforge_elements::model::TypedefId,
    name: ast::Name,
) -> TypeId {
    let alvo = inf.outline.typedefs[td.0 as usize].target_type;
    let params = inf.outline.typedefs[td.0 as usize].type_params.clone();
    let alvo_args = match inf.table.get(alvo).clone() {
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args,
        _ => return inf.core.dynamic_,
    };
    let novos: Vec<crate::table::TypeParamId> = params
        .iter()
        .map(|&p| {
            let d = inf.table.param(p).clone();
            inf.table.alloc_type_param(d.name, crate::table::TypeParamOwner::GenericFunctionType, d.bound, d.variance)
        })
        .collect();
    let tipos: Vec<TypeId> = novos.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
    let mapa = inf.mapa(&params, &tipos);
    for (&p, &o) in novos.iter().zip(params.iter()) {
        let b = inf.table.param(p).bound;
        let b = inf.subst(b, &mapa);
        inf.table.set_type_param_bound(p, b);
        inf.table.param_mut(p).explicito = inf.table.param(o).explicito;
    }
    let args: Vec<TypeId> = alvo_args.iter().map(|a| inf.subst(*a, &mapa)).collect();
    let t = tearoff_de_construtor(inf, cx, e, c, Some(args), name, false, true);
    match inf.table.get(t).clone() {
        Type::Function { type_params, ret, positional, optional, named, nullable } if type_params.is_empty() => {
            inf.table.intern(Type::Function { type_params: novos.into_boxed_slice(), ret, positional, optional, named, nullable })
        }
        _ => t,
    }
}

/// O tipo do tearoff de um construtor de `c` com assinatura `sig`: instanciado
/// com `args`, ou genérico sobre parâmetros novos no lugar dos da classe.
fn tearoff_com_argumentos(inf: &mut BodyInferrer<'_>, c: ClassId, sig: TypeId, args: Option<Vec<TypeId>>) -> TypeId {
    let params = inf.outline.classes[c.0 as usize].type_params.clone();
    match args {
        Some(args) if args.len() == params.len() => {
            let mapa = inf.mapa(&params, &args);
            inf.subst(sig, &mapa)
        }
        _ if params.is_empty() => sig,
        _ => {
            // Genérico: `C<T> Function<T>(...)` com parâmetros novos.
            let novos: Vec<crate::table::TypeParamId> = params
                .iter()
                .map(|&p| {
                    let d = inf.table.param(p).clone();
                    inf.table.alloc_type_param(d.name, crate::table::TypeParamOwner::GenericFunctionType, d.bound, d.variance)
                })
                .collect();
            let tipos: Vec<TypeId> = novos.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
            let mapa = inf.mapa(&params, &tipos);
            let s = inf.subst(sig, &mapa);
            for (&p, &o) in novos.iter().zip(params.iter()) {
                let b = inf.table.param(p).bound;
                let b = inf.subst(b, &mapa);
                inf.table.set_type_param_bound(p, b);
                inf.table.param_mut(p).explicito = inf.table.param(o).explicito;
            }
            match inf.table.get(s).clone() {
                Type::Function { ret, positional, optional, named, nullable, .. } => inf.table.intern(Type::Function {
                    type_params: novos.into_boxed_slice(),
                    ret,
                    positional,
                    optional,
                    named,
                    nullable,
                }),
                _ => s,
            }
        }
    }
}

/// Como `super.nome` é usado: o código do nome que não existe muda.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsoDoSuper {
    Leitura,
    Escrita,
    Invocacao,
}

/// As classes da cadeia de `super` da classe corrente, na ordem da busca:
/// os mixins dela (o último primeiro), a superclasse, os mixins dela… — sem
/// as interfaces (o `getInheritedConcreteMap` do analyzer). Num mixin, as
/// restrições `on` e os supertipos delas.
fn cadeia_do_super(inf: &BodyInferrer<'_>, c: ClassId) -> Vec<ClassId> {
    let mut v = Vec::new();
    let classe = inf.program.class(c);
    if classe.kind == dartforge_elements::model::ClassKind::Mixin {
        for &o in &classe.on_classes {
            for (sup, _) in crate::scope::supertipos_ordenados(inf.program, &inf.outline.hierarchy, o) {
                if !v.contains(&sup) {
                    v.push(sup);
                }
            }
            if !v.contains(&o) {
                v.insert(0, o);
            }
        }
        // Sem `on`, a restrição é `Object`.
        if let Some(o) = inf.core.object_class {
            if !v.contains(&o) {
                v.push(o);
            }
        }
        return v;
    }
    for &m in classe.mixin_classes.iter().rev() {
        v.push(m);
    }
    let mut atual = classe.supertype_class;
    while let Some(k) = atual {
        if v.contains(&k) {
            break;
        }
        v.push(k);
        for &m in inf.program.class(k).mixin_classes.iter().rev() {
            if !v.contains(&m) {
                v.push(m);
            }
        }
        atual = inf.program.class(k).supertype_class;
    }
    // Toda cadeia termina em `Object` (o modelo pode não ligar a superclasse
    // de um enum).
    if let Some(o) = inf.core.object_class {
        if !v.contains(&o) && c != o {
            v.push(o);
        }
    }
    v
}

/// O membro `chave` que `super` alcança: o `getMember2(forSuper: true)` da
/// classe corrente (o `superImplemented.last` do `InheritanceManager3`).
/// `Err(Some(..))`: só o herdado pela interface (`getInherited2`), que o
/// analyzer usa para a recuperação com `ABSTRACT_SUPER_MEMBER_REFERENCE`;
/// `Err(None)`: nenhum.
#[allow(clippy::type_complexity)]
pub(crate) fn membro_alcancado_pelo_super(
    inf: &mut BodyInferrer<'_>,
    c: ClassId,
    chave: SymbolId,
) -> Result<(ClassId, dartforge_elements::model::FunctionElementId), Option<(ClassId, dartforge_elements::model::FunctionElementId)>> {
    let dono = |inf: &BodyInferrer<'_>, m: &crate::heranca::Membro| inf.program.function(m.funcao).class.unwrap_or(m.classe);
    if let Some(m) = inf.membro_da_heranca(c, chave, false, true) {
        return Ok((dono(inf, &m), m.funcao));
    }
    match inf.herdado_da_heranca(c, chave) {
        Some(m) => Err(Some((dono(inf, &m), m.funcao))),
        None => Err(None),
    }
}

/// `super.nome` (leitura, escrita ou invocação): o membro concreto da cadeia
/// do super; só abstrato → `ABSTRACT_SUPER_MEMBER_REFERENCE` (e o tipo dele);
/// nenhum → `UNDEFINED_SUPER_{GETTER,SETTER,METHOD}`
/// (`an611:src/dart/resolver/property_element_resolver.dart:790-880`,
/// `method_invocation_resolver.dart:744-788`). Membros de `Object` contam
/// (toda cadeia termina nele).
pub(crate) fn membro_super(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, name: ast::Name, uso: UsoDoSuper) -> TypeId {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let setter = uso == UsoDoSuper::Escrita;
    let Some(classe) = cx.classe else { return inf.core.dynamic_ };
    let Some(this) = cx.tipo_this else { return inf.core.dynamic_ };
    // `super` num extension type não é válido (`SuperContext`): outro erro,
    // sem resolução.
    if inf.program.class(classe).kind == dartforge_elements::model::ClassKind::ExtensionType {
        return inf.core.dynamic_;
    }
    // Sem a chave do setter no interner, nenhuma declaração tem esse
    // setter: a busca não acha nada e segue para o relato.
    let chave = if setter { inf.chave_setter(name.sym) } else { Some(name.sym) };
    let achado = match chave.map(|k| membro_alcancado_pelo_super(inf, classe, k)).unwrap_or(Err(None)) {
        Ok(x) => Some(x),
        Err(Some((sup, f))) => {
            let fe = &inf.program.functions[f.0 as usize];
            let tipo = match fe.kind {
                dartforge_elements::model::FunctionKind::Getter => "getter",
                dartforge_elements::model::FunctionKind::Setter => "setter",
                dartforge_elements::model::FunctionKind::ImplicitAccessor if setter => "setter",
                dartforge_elements::model::FunctionKind::ImplicitAccessor => "getter",
                _ => "method",
            };
            let nome = inf.interner.resolve(name.sym).to_string();
            inf.aviso_com_codigo(c::ABSTRACT_SUPER_MEMBER_REFERENCE, name.span, &[tipo, &nome]);
            Some((sup, f))
        }
        Err(None) => None,
    };
    if let Some((sup, f)) = achado {
        let (t, _) = inf.tipo_do_membro_declarado(f, setter);
        let t = inf.substituir_do_dono(this, classe, sup, t);
        resolver(inf, cx, e, Resolved::Member { class: sup, member: MemberRef::Function(f), via_super: true });
        // `super._x`: a propriedade do `_superSsaNode`.
        if uso == UsoDoSuper::Leitura {
            return leitura_de_campo(inf, cx, e, Base::Super, t);
        }
        return t;
    }
    // `late final x;` sem inicializador tem setter implícito (o modelo de
    // elementos não o cria): `super.x = v` o alcança.
    if setter {
        for k in cadeia_do_super(inf, classe) {
            let campo = inf.program.class(k).fields.iter().copied().find(|&v| {
                let ve = inf.program.variable(v);
                ve.name == name.sym && ve.late && ve.final_ && !ve.static_
            });
            if let Some(v) = campo {
                return inf.tipo_variavel(v);
            }
        }
    }
    // `Object` (a cadeia de uma classe sempre termina nele).
    let o = inf.core.object;
    if let Some(m) = inf.membro_de_interface(o, name.sym, setter) {
        return m.tipo;
    }
    let nome = inf.interner.resolve(name.sym).to_string();
    match uso {
        UsoDoSuper::Invocacao => {
            let dono = inf.interner.resolve(inf.program.class(classe).name).to_string();
            inf.aviso_com_codigo(c::UNDEFINED_SUPER_METHOD, name.span, &[&nome, &dono]);
        }
        UsoDoSuper::Leitura | UsoDoSuper::Escrita => {
            let tipo = inf.table.format(this, inf.interner, inf.program);
            let codigo = if setter { c::UNDEFINED_SUPER_SETTER } else { c::UNDEFINED_SUPER_GETTER };
            inf.aviso_com_codigo(codigo, name.span, &[&nome, &tipo]);
        }
    }
    inf.core.dynamic_
}

/// O operador `op` visto por `super` na classe corrente: o primeiro
/// declarado nos supertipos, na ordem de [`membro_super`], instanciado como
/// a classe corrente o vê; senão o de `Object`.
pub(crate) fn buscar_operador_super(inf: &mut BodyInferrer<'_>, cx: &Corpo, op: SymbolId) -> Busca {
    let (Some(c), Some(this)) = (cx.classe, cx.tipo_this) else { return Busca::Dinamico };
    if inf.program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType {
        return Busca::Dinamico;
    }
    // Só o concreto (`getMember(forSuper: true)` do `TypePropertyResolver`).
    if let Ok((sup, f)) = membro_alcancado_pelo_super(inf, c, op) {
        let (t, _) = inf.tipo_do_membro_declarado(f, false);
        let tipo = inf.substituir_do_dono(this, c, sup, t);
        return Busca::Achado(Membro {
            resolved: Resolved::Member { class: sup, member: MemberRef::Function(f), via_super: true },
            tipo,
            metodo: true,
            funcao: Some(f),
            de_extensao: false,
        });
    }
    let o = inf.core.object;
    match inf.membro_de_interface(o, op, false) {
        Some(m) => Busca::Achado(m),
        None => Busca::Ausente,
    }
}

/// Símbolo do operador binário.
fn texto_operador(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::Eq | BinaryOp::NotEq => "==",
        _ => return None,
    })
}

fn simbolo_operador(inf: &BodyInferrer<'_>, op: BinaryOp) -> Option<SymbolId> {
    inf.interner.lookup(texto_operador(op)?)
}

fn pular_espacos_e_comentarios(trecho: &str) -> usize {
    let bytes = trecho.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
        } else if bytes.get(pos..pos + 2) == Some(b"/*") {
            pos += 2;
            while pos + 1 < bytes.len() && &bytes[pos..pos + 2] != b"*/" { pos += 1; }
            pos = (pos + 2).min(bytes.len());
        } else if bytes.get(pos..pos + 2) == Some(b"//") {
            while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
        } else {
            break;
        }
    }
    pos
}

fn span_indice(inf: &BodyInferrer<'_>, cx: &Corpo, alvo: ExprId, target: ExprId) -> dartforge_diagnostics::Span {
    let todo = inf.span_expr(cx.unit, alvo);
    let inicio = inf.span_expr(cx.unit, target).end;
    let entre = inf.program.unit(cx.unit).source.get(inicio..todo.end).unwrap_or("");
    let pos = pular_espacos_e_comentarios(entre);
    let colchete = inicio + pos + entre.get(pos..).unwrap_or("").find('[').unwrap_or(0);
    dartforge_diagnostics::Span { start: colchete, end: todo.end }
}

fn avisar_operador_de_extensao(inf: &mut BodyInferrer<'_>, x: ExtensionId, nome: &str, span: dartforge_diagnostics::Span) {
    let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
    let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, nome, extensao);
    inf.aviso(msg, span);
}

fn ler_indice(
    inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, target: ExprId,
    index: ExprId, null_aware: bool, ctx: TypeId,
) -> (TypeId, bool) {
    let (recv, curto) = receptor(inf, cx, target, null_aware);
    // `resolveIndexExpression` (3.6.2 `property_element_resolver.dart:79-97`):
    // o alvo pelo limite (`resolveToBound`) e o `Never` antes do `?[`.
    let bruto = inf.body_types.units[cx.unit.0 as usize].get_type(target).unwrap_or(recv);
    let limite = inf.resolver_ao_limite(bruto);
    if !cx.sobreposicoes.contains_key(&target) && receptor_nunca(inf, cx, target, limite) {
        inferir_livre(inf, cx, index);
        return (inf.core.never, curto);
    }
    let op = inf.sym.indice;
    if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned() {
        if let Some((s, m)) = op.and_then(|s| inf.membro_de_extensao_explicita(x, &args, s, false).map(|m| (s, m))) {
            return (operador_binario_com_membro(inf, cx, recv, s, index, ctx, Some(alvo), m).0, curto);
        }
        inferir_livre(inf, cx, index);
        let span = span_indice(inf, cx, alvo, target);
        avisar_operador_de_extensao(inf, x, "[]", span);
        return (inf.core.dynamic_, curto);
    }
    let indefinido = span_indice(inf, cx, alvo, target);
    // Alvo `void` (pelo limite): `USE_OF_VOID_RESULT` no `[…]`
    // (`property_element_resolver.dart:81-90`, `_reportUnresolvedIndex`).
    let limite = {
        let mut b = recv;
        for _ in 0..16 {
            match inf.table.get(b) {
                Type::TypeParameter { param, nullable: false } => {
                    let l = inf.table.param(*param).bound;
                    if l == b {
                        break;
                    }
                    b = l;
                }
                Type::Intersection { bound, .. } => b = *bound,
                _ => break,
            }
        }
        b
    };
    if matches!(inf.table.get(limite), Type::Void) {
        inferir_livre(inf, cx, index);
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, indefinido, &[]);
        return (inf.core.dynamic_, curto);
    }
    let token = dartforge_diagnostics::Span { start: indefinido.start, end: indefinido.start + 1 };
    let super_ = matches!(ast(inf, cx).expr(target).kind, ExprKind::Super);
    let posicoes = PosicoesDeOperador { token, indefinido, composta: false, super_, alvo_numerico: None };
    (operador_binario(inf, cx, recv, op, index, ctx, posicoes, Some(alvo)).0, curto)
}

/// Onde o analyzer relata os erros de um operador: `token` para o uso sem
/// checagem de nulo (o operador; em `a[i]`, o `[`), `indefinido` para o
/// `undefined_operator` (o operador; em `a[i]`, o `[i]`).
#[derive(Clone, Copy)]
pub(crate) struct PosicoesDeOperador {
    pub token: dartforge_diagnostics::Span,
    pub indefinido: dartforge_diagnostics::Span,
    /// Atribuição composta (`x += 1`): o analyzer fala em "method '+'", e
    /// o operador que falta não é relatado aqui quando o alvo é um método
    /// (`assignment_to_method`) ou `void` (`use_of_void_result`).
    pub composta: bool,
    /// O receptor é `super` (`undefined_super_operator`, outra regra).
    pub super_: bool,
    /// O alvo do refinamento numérico do contexto do operando, quando não é
    /// o receptor: na atribuição composta, o tipo de escrita
    /// (`_computeRhsContext`, assignment_expression_resolver.dart:184-201).
    pub alvo_numerico: Option<TypeId>,
}

/// Operadores de Dart, do mais longo para o mais curto.
const OPERADORES: &[&str] = &[
    ">>>=", "~/=", ">>=", "<<=", "??=", ">>>", "~/", ">>", "<<", "<=", ">=", "==", "!=", "&&", "||", "++", "--", "+=", "-=",
    "*=", "/=", "%=", "&=", "|=", "^=", "??", "+", "-", "*", "/", "%", "<", ">", "&", "|", "^", "~",
];

/// O token de operador logo depois de `depois_de` (pulando espaços e
/// comentários).
fn token_de_operador(inf: &BodyInferrer<'_>, cx: &Corpo, depois_de: usize) -> dartforge_diagnostics::Span {
    let fonte = &inf.program.unit(cx.unit).source;
    let resto = fonte.get(depois_de..).unwrap_or("");
    let pos = depois_de + pular_espacos_e_comentarios(resto);
    let texto = fonte.get(pos..).unwrap_or("");
    let n = OPERADORES.iter().find(|o| texto.starts_with(**o)).map_or(0, |o| o.len());
    dartforge_diagnostics::Span { start: pos, end: pos + n }
}

/// Invocação de um operador de um argumento (`a + b`, `a[i]`): busca o
/// membro no receptor, infere o argumento com o contexto do parâmetro
/// (com o refinamento numérico) e devolve `(tipo, membro)`.
pub(crate) fn operador_binario(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    recv: TypeId,
    op: Option<SymbolId>,
    arg: ExprId,
    ctx: TypeId,
    posicoes: PosicoesDeOperador,
    no: Option<ExprId>,
) -> (TypeId, Option<Membro>) {
    let Some(op) = op else {
        inferir_livre(inf, cx, arg);
        return (inf.core.dynamic_, None);
    };
    // Receptor potencialmente anulável: o analyzer relata a invocação do
    // operador (`[]`/`[]=` como invocação de método) sem checagem de nulo.
    let checar_nulo = inf.exige_checagem_de_nulo(cx.lib, recv, op, false);
    if checar_nulo {
        let texto = inf.interner.resolve(op).to_string();
        let codigo = if texto == "[]" || texto == "[]=" || posicoes.composta {
            dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE
        } else {
            dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE
        };
        let desde = inf.diagnostics.len();
        inf.aviso_de_nulo(recv, codigo, posicoes.token, &[&texto]);
        // O receptor do operador (`leftOperand`, o alvo do índice, o lado
        // esquerdo da composta).
        let receptor = no.and_then(|n| match &ast(inf, cx).expr(n).kind {
            ExprKind::Binary { left, .. } => Some(*left),
            ExprKind::Index { target, .. } => Some(*target),
            ExprKind::Assign { target, .. } => Some(*target),
            _ => None,
        });
        if let Some(r) = receptor {
            inf.anexar_nao_promocao(desde, cx, Some(r), posicoes.token);
        }
    }
    // `super[i]`, `super + x`: o operador é o da superclasse (como o
    // `super.m` de [`membro_super`]), não o que a própria classe sobrescreve.
    let busca = if posicoes.super_ { buscar_operador_super(inf, cx, op) } else { inf.buscar_membro(cx.lib, recv, op, false) };
    // `AMBIGUOUS_EXTENSION_MEMBER_ACCESS` do operador: no binário inteiro;
    // no índice, no alvo (`extension_member_resolver.dart:115-128`).
    {
        let sp = match no.map(|n| &ast(inf, cx).expr(n).kind) {
            Some(ExprKind::Index { target, .. }) => inf.span_expr(cx.unit, *target),
            // A composta (`a += 0`): no operador.
            Some(ExprKind::Assign { .. }) => posicoes.token,
            Some(_) => inf.span_expr(cx.unit, no.unwrap()),
            None => posicoes.token,
        };
        inf.relatar_ambiguidade_de_extensao(sp);
    }
    match busca {
        Busca::Achado(m) => operador_binario_com_membro_alvo(inf, cx, recv, posicoes.alvo_numerico.unwrap_or(recv), op, arg, ctx, no, m),
        Busca::Ausente if checar_nulo => {
            inferir_livre(inf, cx, arg);
            (inf.core.dynamic_, None)
        }
        Busca::Dinamico => {
            inferir_livre(inf, cx, arg);
            // Operador sobre `InvalidType`: o resultado é inválido
            // (`binary_expression_resolver.dart:332-335`).
            (if inf.table.e_invalido(recv) { recv } else { inf.core.dynamic_ }, None)
        }
        Busca::Nunca => {
            inferir_livre(inf, cx, arg);
            (inf.core.never, None)
        }
        Busca::Ausente => {
            inferir_livre(inf, cx, arg);
            if posicoes.super_ {
                // `UNDEFINED_SUPER_OPERATOR` com o tipo de `super` (o de `this`).
                let texto = inf.interner.resolve(op).to_string();
                let tipo = inf.table.format(recv, inf.interner, inf.program);
                inf.aviso_com_codigo(
                    dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_SUPER_OPERATOR,
                    posicoes.indefinido,
                    &[&texto, &tipo],
                );
                return (inf.core.dynamic_, None);
            }
            if matches!(inf.table.get(recv), Type::Void | Type::Function { .. }) {
                return (inf.core.dynamic_, None);
            }
            let texto = inf.interner.resolve(op).to_string();
            let tipo = inf.table.format(recv, inf.interner, inf.program);
            inf.aviso_com_codigo(
                dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR,
                posicoes.indefinido,
                &[&texto, &tipo],
            );
            // Operador que o tipo não tem: o resultado é `InvalidType`
            // (`binary_expression_resolver.dart:308`).
            (inf.table.invalido(inf.core.dynamic_), None)
        }
    }
}

fn operador_binario_com_membro(
    inf: &mut BodyInferrer<'_>, cx: &mut Corpo, recv: TypeId, op: SymbolId,
    arg: ExprId, ctx: TypeId, no: Option<ExprId>, m: Membro,
) -> (TypeId, Option<Membro>) {
    operador_binario_com_membro_alvo(inf, cx, recv, recv, op, arg, ctx, no, m)
}

/// Como [`operador_binario_com_membro`], com o alvo do refinamento numérico
/// do contexto do operando à parte do receptor.
#[allow(clippy::too_many_arguments)]
fn operador_binario_com_membro_alvo(
    inf: &mut BodyInferrer<'_>, cx: &mut Corpo, recv: TypeId, alvo_numerico: TypeId, op: SymbolId,
    arg: ExprId, ctx: TypeId, no: Option<ExprId>, m: Membro,
) -> (TypeId, Option<Membro>) {
    if let Some(n) = no {
        resolver(inf, cx, n, m.resolved.clone());
    }
    let (param, ret) = match inf.table.get(m.tipo).clone() {
        Type::Function { positional, optional, ret, .. } => (positional.first().or(optional.first()).copied(), ret),
        _ => (None, inf.core.dynamic_),
    };
    let ctx_arg = match param {
        Some(p) => contexto_numerico(inf, alvo_numerico, &m, op, ctx, p),
        None => inf.core.unknown,
    };
    let ta = inferir(inf, cx, arg, ctx_arg);
    if let Some(p) = param {
        verificar_atribuivel_expr(inf, cx, arg, ta, p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
    }
    let t = refinar_numerico(inf, recv, &m, op, &[ta], ret);
    (t, Some(m))
}

fn e_numerico_refinavel(inf: &BodyInferrer<'_>, m: &Membro, op: SymbolId) -> bool {
    if m.de_extensao {
        return false;
    }
    let s = inf.interner.resolve(op);
    matches!(s, "+" | "-" | "*" | "%" | "remainder")
}

/// Contexto do argumento de `e1 op e2` numérico (`_refineNumericInvocationContext`).
pub(crate) fn contexto_numerico(inf: &mut BodyInferrer<'_>, t: TypeId, m: &Membro, op: SymbolId, ctx: TypeId, atual: TypeId) -> TypeId {
    if !e_numerico_refinavel(inf, m, op) && Some(op) != inf.sym.clamp {
        return atual;
    }
    let num_q = inf.anulavel(inf.core.num);
    if !inf.sub(t, num_q) {
        return atual;
    }
    let c = inf.fecho_maior(ctx);
    let (int, double, num) = (inf.core.int, inf.core.double, inf.core.num);
    let int_q = inf.anulavel(int);
    let double_q = inf.anulavel(double);
    if inf.sub(int, c) && !inf.sub(num, c) && inf.sub(t, int_q) {
        return int;
    }
    let clamp = Some(op) == inf.sym.clamp;
    if inf.sub(double, c) && !inf.sub(num, c) && (if clamp { inf.sub(t, double_q) } else { !inf.sub(t, double_q) }) {
        return double;
    }
    num
}

/// Tipo de `e1 op e2` numérico (`_refineNumericInvocationTypeNullSafe`).
pub(crate) fn refinar_numerico(inf: &mut BodyInferrer<'_>, t: TypeId, m: &Membro, op: SymbolId, args: &[TypeId], atual: TypeId) -> TypeId {
    let (int, double, num) = (inf.core.int, inf.core.double, inf.core.num);
    let num_q = inf.anulavel(num);
    let int_q = inf.anulavel(int);
    let double_q = inf.anulavel(double);
    if e_numerico_refinavel(inf, m, op) && args.len() == 1 && inf.sub(t, num_q) {
        let s = args[0];
        if inf.sub(t, double_q) {
            return double;
        }
        let fundo = inf.e_fundo(s);
        if !fundo && inf.sub(s, double_q) {
            return double;
        }
        if !fundo && inf.sub(t, int_q) && inf.sub(s, int_q) {
            return int;
        }
        return num;
    }
    if Some(op) == inf.sym.clamp && !m.de_extensao && args.len() == 2 && inf.sub(t, num_q) {
        let (t2, t3) = (args[0], args[1]);
        if inf.e_fundo(t2) || inf.e_fundo(t3) {
            return atual;
        }
        if inf.sub(t, int_q) && inf.sub(t2, int_q) && inf.sub(t3, int_q) {
            return int;
        }
        if inf.sub(t, double_q) && inf.sub(t2, double_q) && inf.sub(t3, double_q) {
            return double;
        }
        return num;
    }
    atual
}

/// Operadores binários.
fn binario(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, op: BinaryOp, left: ExprId, right: ExprId, ctx: TypeId) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    match op {
        // `BinaryExpressionResolver.resolve` (`binary_expression_resolver.dart:69-77`):
        // o operador que não é binário, no token; `_resolveUnsupportedOperator`
        // visita os dois lados sem contexto e o tipo é `InvalidType`.
        BinaryOp::NaoBinario => {
            let fim_esquerdo = inf.span_expr(cx.unit, left).end;
            let token = token_de_operador(inf, cx, fim_esquerdo);
            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::NOT_BINARY_OPERATOR, token, &["~"]);
            inferir_livre(inf, cx, left);
            inferir_livre(inf, cx, right);
            inf.table.invalido(inf.core.dynamic_)
        }
        BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq => {
            let (vf, ff) = condicao_binaria(inf, cx, e, op, left, right);
            cx.fluxo = inf.juntar(&vf, &ff);
            // `x == y` com `x: Never`: o receptor nunca existe, e a invocação
            // de `==` tem o tipo `Never` (como qualquer membro de `Never`).
            if matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                && inf.body_types.units[cx.unit.0 as usize].get_type(left).is_some_and(|t| matches!(inf.table.get(t), Type::Never))
            {
                return inf.core.never;
            }
            inf.core.bool_
        }
        BinaryOp::IfNull => {
            let k_q = if inf.e_desconhecido(ctx) { ctx } else { inf.anulavel(ctx) };
            let t1 = inferir(inf, cx, left, k_q);
            uso_de_void(inf, cx, left, t1);
            morto_no_operando_direito(inf, cx, left, right);
            avisar_nulo_morto(inf, cx, t1, right);
            let j = if inf.e_desconhecido(ctx) || inf.e_dynamic(ctx) { t1 } else { ctx };
            // Ramo em que `e1` não é nulo: `e1` promove a não nulo; no outro
            // nada se promove. A junção dos dois vale depois, então
            // `y ?? (throw 0)` deixa `y` promovido.
            let mut antes = cx.fluxo.clone();
            if let Some(id) = alvo_de_promocao(inf, cx, left) {
                let decl = cx.local(id).tipo;
                inf.promover_nao_nulo(&mut antes, id, decl);
            }
            let t2 = inferir(inf, cx, right, j);
            let depois = cx.fluxo.clone();
            cx.fluxo = inf.juntar(&antes, &depois);
            let nn = inf.nao_nulo(t1);
            limite_superior_em_contexto(inf, nn, t2, ctx)
        }
        _ => {
            let u = inf.core.unknown;
            let l = inferir(inf, cx, left, u);
            uso_de_void(inf, cx, left, l);
            morto_no_operando_direito(inf, cx, left, right);
            if receptor_nunca(inf, cx, left, l) {
                inferir_livre(inf, cx, right);
                return inf.core.never;
            }
            if matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, left, this);
            }
            let sym = simbolo_operador(inf, op);
            if let Some((x, args)) = cx.sobreposicoes.get(&left).cloned()
                && let Some(texto) = texto_operador(op)
            {
                if let Some((s, m)) = sym.and_then(|s| inf.membro_de_extensao_explicita(x, &args, s, false).map(|m| (s, m))) {
                    return operador_binario_com_membro(inf, cx, l, s, right, ctx, Some(e), m).0;
                }
                let inicio = inf.span_expr(cx.unit, left).end;
                let fim = inf.span_expr(cx.unit, right).start;
                let trecho = inf.program.unit(cx.unit).source.get(inicio..fim).unwrap_or("");
                let pos = pular_espacos_e_comentarios(trecho);
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, texto, extensao);
                if trecho.get(pos..).is_some_and(|resto| resto.starts_with(texto)) {
                    let offset = inicio + pos;
                    inf.aviso(msg, dartforge_diagnostics::Span { start: offset, end: offset + texto.len() });
                }
                inferir_livre(inf, cx, right);
                return inf.core.dynamic_;
            }
            let token = token_de_operador(inf, cx, inf.span_expr(cx.unit, left).end);
            let _ = span;
            let super_ = matches!(ast(inf, cx).expr(left).kind, ExprKind::Super);
            let posicoes = PosicoesDeOperador { token, indefinido: token, composta: false, super_, alvo_numerico: None };
            let (t, _) = operador_binario(inf, cx, l, sym, right, ctx, posicoes, Some(e));
            t
        }
    }
}

/// O operando direito de um binário que começa inalcançável: o primeiro nó
/// morto tem pai `BinaryExpression`, e o trecho vai do operador ao fim do
/// operando direito (`dead_code_verifier.dart:316-318`).
fn morto_no_operando_direito(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, left: ExprId, right: ExprId) {
    if cx.fluxo.alcancavel || cx.trecho_morto.is_some() {
        return;
    }
    let token = token_de_operador(inf, cx, inf.span_expr(cx.unit, left).end);
    let fim = inf.span_expr(cx.unit, right).end;
    inf.aviso(DEAD_CODE.template.to_string(), dartforge_diagnostics::Span { start: token.start, end: fim });
    cx.trecho_morto = Some(cx.fins_de_fluxo.len());
}

/// Unários (`!`, `-`, `~`, `++`, `--`, `!` pós-fixo).
fn unario(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, op: UnaryOp, operand: ExprId, ctx: TypeId, curto: &mut bool) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    match op {
        UnaryOp::Not => {
            let (vf, ff) = condicao(inf, cx, operand);
            // Fora de condição, `!e` também exige `bool` (e não `void`).
            verificar_bool(inf, cx, operand, UsoBool::Negacao);
            cx.fluxo = inf.juntar(&vf, &ff);
            inf.core.bool_
        }
        UnaryOp::NullAssert => {
            // O operando de `e!` recebe o contexto anulável (`K?`): em
            // `_name = _becomeParentOf(name)!`, `T` sai `IdentifierImpl?`.
            let k = if inf.e_desconhecido(ctx) { ctx } else { inf.anulavel(ctx) };
            let (t, c) = inferir_no(inf, cx, operand, k, true);
            *curto = c;
            // `NULL_CHECK_ALWAYS_FAILS` (`visitPostfixExpression`,
            // `best_practices_verifier.dart:686-697`): `e!` com `e` de tipo `Null`.
            if e_null_do_core(inf, t) {
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::warning::NULL_CHECK_ALWAYS_FAILS, span, &[]);
            }
            // `unnecessary_non_null_assertion` (no `!`): só com o operando
            // certamente não anulável (`T extends Object?` não é).
            if !c && estritamente_nao_anulavel(inf, t) {
                let sp = dartforge_diagnostics::Span { start: span.end.saturating_sub(1), end: span.end };
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::static_warning::UNNECESSARY_NON_NULL_ASSERTION, sp, &[]);
            }
            if let Some(id) = alvo_de_promocao(inf, cx, operand) {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                inf.promover_nao_nulo(&mut f, id, decl);
                cx.fluxo = f;
            }
            // `visitPostfixExpression` com `!`: `checkForUseOfVoidResult(node)`
            // (3.6.2 `error_verifier.dart:1310-1316`), no `e!` inteiro.
            let r = inf.nao_nulo_promocao(t);
            if matches!(inf.table.get(r), Type::Void) {
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, span, &[]);
            }
            r
        }
        UnaryOp::Neg | UnaryOp::BitNot => {
            // `-1` com literal: o literal recebe o contexto (`double x = -1`).
            let literal = op == UnaryOp::Neg && matches!(inf.program.unit(cx.unit).ast.expr(operand).kind, ExprKind::Int(_));
            let c = if literal { ctx } else { inf.core.unknown };
            if literal && op == UnaryOp::Neg {
                cx.literal_negado = Some(operand);
            }
            let t = inferir(inf, cx, operand, c);
            uso_de_void(inf, cx, operand, t);
            let sym = if op == UnaryOp::Neg { inf.sym.menos_unario } else { inf.sym.til };
            if let Some((x, args)) = cx.sobreposicoes.get(&operand).cloned() {
                if let Some(m) = sym.and_then(|s| inf.membro_de_extensao_explicita(x, &args, s, false)) {
                    resolver(inf, cx, e, m.resolved.clone());
                    return match inf.table.get(m.tipo) {
                        Type::Function { ret, .. } => *ret,
                        _ => inf.core.dynamic_,
                    };
                }
                let operador = if op == UnaryOp::Neg { "unary-" } else { "~" };
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, operador, extensao);
                inf.aviso(msg, dartforge_diagnostics::Span { start: span.start, end: span.start + 1 });
                return inf.core.dynamic_;
            }
            let Some(sym) = sym else { return inf.core.dynamic_ };
            let nome = if op == UnaryOp::Neg { "unary-" } else { "~" };
            let sp_op = dartforge_diagnostics::Span { start: span.start, end: span.start + 1 };
            let nulo = nulo_em_unario(inf, cx, t, sym, nome, sp_op, operand);
            let busca = inf.buscar_membro(cx.lib, t, sym, false);
            // A ambiguidade de extensão do prefixo, no operando.
            let sp_operando = inf.span_expr(cx.unit, operand);
            inf.relatar_ambiguidade_de_extensao(sp_operando);
            match busca {
                Busca::Achado(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    match inf.table.get(m.tipo) {
                        Type::Function { ret, .. } => *ret,
                        _ => inf.core.dynamic_,
                    }
                }
                Busca::Nunca => inf.core.never,
                Busca::Ausente if !nulo => {
                    operador_unario_indefinido(inf, t, nome, sp_op);
                    inf.table.invalido(inf.core.dynamic_)
                }
                _ => inf.core.dynamic_,
            }
        }
        UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
            let prefixo = matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec);
            let bop = if matches!(op, UnaryOp::PrefixInc | UnaryOp::PostfixInc) { BinaryOp::Add } else { BinaryOp::Sub };
            let (leitura, escrita, local) = ler_para_escrita(inf, cx, operand, curto);
            // `_resolve1` do `PostfixExpressionResolver` e o do
            // `PrefixExpressionResolver` (3.6.2 `:127-132`, `:177-183`): a
            // leitura `Never` é `RECEIVER_OF_TYPE_NEVER` no operando, sem
            // operador, e o tipo é `Never`.
            if matches!(inf.table.get(leitura), Type::Never) {
                receptor_nunca(inf, cx, operand, leitura);
                return inf.core.never;
            }
            let sym = simbolo_operador(inf, bop);
            let int = inf.core.int;
            if let Some(s) = sym {
                // O token `++`/`--`, antes ou depois do operando.
                let sp_op = if prefixo {
                    dartforge_diagnostics::Span { start: span.start, end: span.start + 2 }
                } else {
                    dartforge_diagnostics::Span { start: span.end.saturating_sub(2), end: span.end }
                };
                let nome = if bop == BinaryOp::Add { "+" } else { "-" };
                let nulo = nulo_em_unario(inf, cx, leitura, s, nome, sp_op, operand);
                // `C++` com `C` tipo: só o `ASSIGNMENT_TO_TYPE` da escrita.
                let de_tipo = referencia_a_tipo(inf, cx, operand).is_some()
                    || matches!(&ast(inf, cx).expr(operand).kind, ExprKind::Identifier(n)
                        if matches!(resolver_nome(inf, cx, n.sym, false), RefNome::TipoEmbutido | RefNome::TipoParam(_)));
                if !nulo && !de_tipo && matches!(inf.buscar_membro(cx.lib, leitura, s, false), Busca::Ausente) {
                    operador_unario_indefinido(inf, leitura, nome, sp_op);
                }
            }
            let res = match sym.map(|s| inf.buscar_membro(cx.lib, leitura, s, false)) {
                Some(Busca::Achado(m)) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    let (ret, param) = match inf.table.get(m.tipo) {
                        Type::Function { ret, positional, .. } => (*ret, positional.first().copied()),
                        _ => (inf.core.dynamic_, None),
                    };
                    let r = refinar_numerico(inf, leitura, &m, sym.unwrap(), &[int], ret);
                    // `++a`/`a++`: o `1` implícito (`int`) contra o parâmetro do
                    // operador, relatado no operando
                    // (`_checkForIntNotAssignable`, an611:src/generated/error_verifier.dart:3899-3906);
                    // o resultado contra o tipo de escrita, no nó inteiro
                    // (`_checkForInvalidAssignmentIncDec`,
                    // an611:src/dart/resolver/prefix_expression_resolver.dart:96-110).
                    if !inf.e_dynamic(leitura) && !matches!(inf.table.get(leitura), Type::Void) {
                        if let Some(p) = param {
                            let sp = inf.span_expr(cx.unit, sem_parenteses(inf, cx, operand));
                            inf.verificar_atribuivel(int, p, sp, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                        }
                        // Getter e setter de tipos diferentes (`int get x`, `set
                        // x(String)`): o oráculo 3.6.2 não relata (conferido no
                        // corpus, `InvalidAssignment__postfixExpression_in_*`).
                        // O retorno do operador (`operatorReturnType`), sem o
                        // `?` do encurtamento nulo (`x?[0]++`).
                        if !inf.e_dynamic(escrita) && (local.is_some() || leitura == escrita) {
                            inf.verificar_atribuivel(r, escrita, span, INVALID_ASSIGNMENT.template);
                        }
                    }
                    r
                }
                Some(Busca::Nunca) => inf.core.never,
                _ => inf.core.dynamic_,
            };
            if let Some(id) = local {
                // A regra de escrita vê o estado de antes da escrita.
                let alvo_span = inf.span_expr(cx.unit, operand);
                check_final_local(inf, cx, id, alvo_span);
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                let motivo = super::fluxo::MotivoDeNaoPromocao::Escrita { nome: cx.local(id).nome, span };
                inf.atribuir_fluxo(&mut f, id, decl, res, Some(motivo));
                cx.fluxo = f;
            }
            let _ = escrita;
            if prefixo {
                res
            } else {
                leitura
            }
        }
    }
}

/// O operador de prefixo ou sufixo (`-x`, `~x`, `++x`, `x--`) num receptor
/// potencialmente anulável: `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE`
/// no token do operador, pelo `TypePropertyResolver` (o `Null` puro é
/// `INVALID_USE_OF_NULL_VALUE`), com as mensagens de não promoção do operando.
fn nulo_em_unario(inf: &mut BodyInferrer<'_>, cx: &Corpo, recv: TypeId, sym: SymbolId, nome: &str, sp: dartforge_diagnostics::Span, operand: ExprId) -> bool {
    // `void` é potencialmente anulável e não é limitado por `dynamic`: o
    // `TypePropertyResolver` (`type_property_resolver.dart:80-150`) relata o
    // operador que nem `Object` nem uma extensão tem (além do
    // `USE_OF_VOID_RESULT` no operando).
    let exige = if matches!(inf.table.get(recv), Type::Void) {
        let o = inf.core.object;
        inf.membro_de_interface(o, sym, false).is_none() && inf.membro_de_extensao(cx.lib, recv, sym, false).is_none()
    } else {
        inf.exige_checagem_de_nulo(cx.lib, recv, sym, false)
    };
    if !exige {
        return false;
    }
    let desde = inf.diagnostics.len();
    inf.aviso_de_nulo(recv, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE, sp, &[nome]);
    inf.anexar_nao_promocao(desde, cx, Some(operand), sp);
    true
}

/// O operador de prefixo/sufixo que o tipo do operando não tem
/// (`TypePropertyResolver` sem membro): `UNDEFINED_OPERATOR` no token do
/// operador (`prefix_expression_resolver.dart`, `postfix_expression_resolver.dart`).
fn operador_unario_indefinido(inf: &mut BodyInferrer<'_>, recv: TypeId, nome: &str, sp: dartforge_diagnostics::Span) {
    if matches!(inf.table.get(recv), Type::Void | Type::Function { .. }) || inf.table.e_invalido(recv) {
        return;
    }
    let tipo = inf.table.format(recv, inf.interner, inf.program);
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR, sp, &[nome, &tipo]);
}

fn check_final_local(inf: &mut BodyInferrer<'_>, cx: &Corpo, id: LocalId, span: dartforge_diagnostics::Span) {
    let l = cx.local(id);
    if cx.funcoes_locais.contains(&id) {
        inf.aviso(ASSIGNMENT_TO_FUNCTION.template.to_string(), span);
    } else if l.const_ {
        inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), span);
    } else if l.final_ && l.late {
        if cx.fluxo.atribuida(id) {
            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::LATE_FINAL_LOCAL_ALREADY_ASSIGNED, span, &[]);
        }
    } else if l.final_ && !cx.fluxo.nao_atribuida(id) {
        let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
        inf.aviso(msg, span);
    }
}

fn avisar_escrita_em_metodo(inf: &mut BodyInferrer<'_>, nome: ast::Name) {
    inf.aviso(ASSIGNMENT_TO_METHOD.template.to_string(), nome.span);
}

/// Diagnóstico quando a recuperação de uma escrita encontra apenas o getter.
/// O getter explícito é uma propriedade sem setter; o getter implícito é um
/// campo `final` ou `const`. `late final` sem inicializador ainda aceita escrita.
fn avisar_membro_sem_setter(inf: &mut BodyInferrer<'_>, nome: ast::Name, f: dartforge_elements::model::FunctionElementId) -> bool {
    let fe = inf.program.function(f);
    match fe.kind {
        FunctionKind::Getter => {
            let dono = fe.class.map(|c| inf.program.class(c).name)
                .or_else(|| fe.extension.and_then(|e| inf.program.extension(e).name));
            let Some(dono) = dono else { return false };
            let msg = format!(
                "{}: '{}' na classe '{}'",
                ASSIGNMENT_TO_FINAL_NO_SETTER.template,
                inf.interner.resolve(nome.sym),
                inf.interner.resolve(dono)
            );
            inf.aviso(msg, nome.span);
            true
        }
        FunctionKind::ImplicitAccessor => {
            let Some(v) = fe.variable else { return false };
            let ve = inf.program.variable(v);
            if ve.const_ {
                inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), nome.span);
            } else if ve.final_ && !(ve.late && inf.inicializador(v).is_none()) {
                let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL.template, inf.interner.resolve(nome.sym));
                inf.aviso(msg, nome.span);
            } else {
                return false;
            }
            true
        }
        _ => false,
    }
}

/// Para `x op= e` / `x++`: lê o alvo, devolvendo `(tipo lido, tipo de escrita, local)`.
/// `curto` fica verdadeiro quando o alvo está numa cadeia `?.`/`?[` (o
/// resultado da expressão é então anulável: `a?.x += 1` vale null com `a`
/// null).
fn ler_para_escrita(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, curto_saida: &mut bool) -> (TypeId, TypeId, Option<LocalId>) {
    let a = ast(inf, cx);
    match &a.expr(alvo).kind {
        ExprKind::Identifier(n) => {
            let n = *n;
            match resolver_nome(inf, cx, n.sym, true) {
                RefNome::Local(id) => {
                    resolver(inf, cx, alvo, Resolved::Local(id));
                    referencia_de_juncao(inf, cx, id, n);
                    escrita_em_guarda(inf, cx, id, n.span);
                    escrita_em_primario(inf, cx, id, n.span);
                    let t = ler_local(inf, cx, id, n.span);
                    let decl = cx.local(id).tipo;
                    (t, decl, Some(id))
                }
                _ => {
                    let leitura = identificador(inf, cx, alvo, n);
                    let escrita = tipo_de_escrita_nome(inf, cx, alvo, n);
                    (leitura, escrita, None)
                }
            }
        }
        ExprKind::Property { target, name, null_aware } => {
            let (target, name, null_aware) = (*target, *name, *null_aware);
            let (leitura, curto_lido) = propriedade(inf, cx, alvo, target, name, null_aware);
            let recv_lido = inf.body_types.units[cx.unit.0 as usize].get_type(target).map(|t| {
                (if null_aware { inf.nao_nulo(t) } else { t }, curto_lido)
            });
            let mut curto = false;
            let escrita = escrita_propriedade(inf, cx, alvo, target, name, null_aware, &mut curto, recv_lido);
            *curto_saida = curto_lido || curto;
            registrar(inf, cx, alvo, leitura);
            (leitura, escrita, None)
        }
        ExprKind::Index { target, index, null_aware } => {
            let (target, index, null_aware) = (*target, *index, *null_aware);
            let u = inf.core.unknown;
            let (t, c) = ler_indice(inf, cx, alvo, target, index, null_aware, u);
            *curto_saida = c;
            // O tipo de escrita (`setWriteElement` com `IndexExpression`, 3.6.2
            // `resolver.dart:1722-1728`): o 2º parâmetro do `[]=`; sem ele,
            // `InvalidType` (`dynamic` em alvo `dynamic`).
            let segundo = |inf: &BodyInferrer<'_>, f: TypeId| match inf.table.get(f) {
                Type::Function { positional, .. } if positional.len() == 2 => Some(positional[1]),
                _ => None,
            };
            let mut escrita = inf.table.invalido(inf.core.dynamic_);
            if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned() {
                match inf.sym.indice_set.and_then(|s| inf.membro_de_extensao_explicita(x, &args, s, false)) {
                    Some(m) => {
                        if let Some(p1) = segundo(inf, m.tipo) {
                            escrita = p1;
                        }
                    }
                    None => {
                        let span = span_indice(inf, cx, alvo, target);
                        avisar_operador_de_extensao(inf, x, "[]=", span);
                    }
                }
            } else if let Some(op) = inf.sym.indice_set
                && matches!(ast(inf, cx).expr(target).kind, ExprKind::Super)
            {
                // `super[i]++`: o `[]=` também pela cadeia de `super`.
                match buscar_operador_super(inf, cx, op) {
                    Busca::Ausente => {
                        let span = span_indice(inf, cx, alvo, target);
                        let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                        let tipo = inf.table.format(this, inf.interner, inf.program);
                        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_SUPER_OPERATOR, span, &["[]=", &tipo]);
                    }
                    Busca::Achado(m) => {
                        if let Some(p1) = segundo(inf, m.tipo) {
                            escrita = p1;
                        }
                    }
                    _ => {}
                }
            } else if let Some(op) = inf.sym.indice_set
                && let Some(recv) = inf.body_types.units[cx.unit.0 as usize].get_type(target)
                && !matches!(ast(inf, cx).expr(target).kind, ExprKind::Super)
            {
                // `x[i]++`: o `[]=` também é procurado (o `writeElement` do
                // `PropertyElementResolver`); ausente, `UNDEFINED_OPERATOR`
                // no `[…]` (`_reportUnresolvedIndex`).
                let r = if null_aware { inf.nao_nulo(recv) } else { recv };
                let invalido = inf.table.e_invalido(r) || matches!(inf.table.get(r), Type::Dynamic | Type::Never | Type::Void);
                if matches!(inf.table.get(r), Type::Dynamic) && !inf.table.e_invalido(r) {
                    escrita = inf.core.dynamic_;
                }
                if !invalido {
                    if let Busca::Achado(m) = inf.buscar_membro(cx.lib, r, op, false)
                        && let Some(p1) = segundo(inf, m.tipo)
                    {
                        escrita = p1;
                    }
                }
                if !invalido && !inf.exige_checagem_de_nulo(cx.lib, r, op, false) {
                    match inf.buscar_membro(cx.lib, r, op, false) {
                        Busca::Ausente => {
                            let span = span_indice(inf, cx, alvo, target);
                            let tipo = inf.table.format(r, inf.interner, inf.program);
                            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR, span, &["[]=", &tipo]);
                        }
                        // `checkIndexExpressionIndex` com o `writeElement`
                        // (3.6.2 `error_detection_helpers.dart:257-287`,
                        // `resolver.dart:1443`): o índice também contra o
                        // primeiro parâmetro do `[]=`.
                        Busca::Achado(m) if m.metodo => {
                            if let Type::Function { positional, .. } = inf.table.get(m.tipo).clone()
                                && let Some(&p0) = positional.first()
                                && let Some(ti) = inf.body_types.units[cx.unit.0 as usize].get_type(index)
                            {
                                let sp = inf.span_expr(cx.unit, index);
                                inf.verificar_atribuivel(ti, p0, sp, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                            }
                        }
                        _ => {}
                    }
                }
            }
            registrar(inf, cx, alvo, t);
            (t, escrita, None)
        }
        _ => {
            let t = inferir_livre(inf, cx, alvo);
            (t, t, None)
        }
    }
}

/// `_checkForAmbiguousImport` (3.6.2 `error_verifier.dart:2116-2130`): o
/// nome (com o prefixo) que dois imports trazem com elementos diferentes é
/// `AMBIGUOUS_IMPORT` no nome; o elemento é o `MultiplyDefinedElement` e o tipo
/// é inválido. Diz se relatou.
pub(crate) fn importacao_ambigua(inf: &mut BodyInferrer<'_>, cx: &Corpo, prefixo: Option<SymbolId>, nome: ast::Name) -> bool {
    let b = match prefixo {
        Some(p) => inf.program.lookup_prefixed_na_unidade(cx.unit, p, nome.sym),
        None => inf.program.lookup_na_unidade(cx.unit, nome.sym),
    };
    if !b.is_some_and(|b| b.ambiguous) {
        return false;
    }
    let lista = crate::scope::bibliotecas_ambiguas(inf.program, cx.unit, prefixo, nome.sym);
    let texto = inf.interner.resolve(nome.sym).to_string();
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_IMPORT, nome.span, &[&texto, &lista]);
    true
}

/// Tipo de escrita (setter) de um nome não local.
fn tipo_de_escrita_nome(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, n: ast::Name) -> TypeId {
    match resolver_nome(inf, cx, n.sym, true) {
        RefNome::Local(id) => cx.local(id).tipo,
        RefNome::Elemento(_) if importacao_ambigua(inf, cx, None, n) => inf.table.invalido(inf.core.dynamic_),
        RefNome::Elemento(el) => {
            resolver(inf, cx, alvo, Resolved::Element(el));
            match el {
                // `setWriteElement` (3.6.2 `resolver.dart:1709-1747`): o tipo
                // de escrita só vem de um setter (o sintético, pela variável)
                // ou de um `VariableElement`; a variável de topo ou estática
                // `final`/`const` não tem setter, a escrita cai no getter de
                // recuperação e o tipo é `InvalidType` (sem
                // `INVALID_ASSIGNMENT`).
                Element::Variable(v) => {
                    let ve = inf.program.variable(v);
                    if ve.const_ {
                        inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), n.span);
                        inf.table.invalido(inf.core.dynamic_)
                    } else if ve.final_ && !(ve.late && inf.inicializador(v).is_none()) {
                        let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL.template, inf.interner.resolve(n.sym));
                        inf.aviso(msg, n.span);
                        inf.table.invalido(inf.core.dynamic_)
                    } else {
                        inf.tipo_variavel(v)
                    }
                }
                Element::Function(f) => {
                    let fe = inf.program.function(f);
                    match (fe.kind, fe.variable) {
                        (FunctionKind::ImplicitAccessor, Some(v)) => inf.tipo_variavel(v),
                        (FunctionKind::Setter, _) => inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_),
                        (FunctionKind::Getter, _) => {
                            let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL.template, inf.interner.resolve(n.sym));
                            inf.aviso(msg, n.span);
                            inf.table.invalido(inf.core.dynamic_)
                        }
                        (FunctionKind::Function, _) => {
                            inf.aviso(ASSIGNMENT_TO_FUNCTION.template.to_string(), n.span);
                            inf.core.dynamic_
                        }
                        _ => inf.core.dynamic_,
                    }
                }
                Element::Class(_) | Element::Typedef(_) => {
                    inf.aviso(ASSIGNMENT_TO_TYPE.template.to_string(), n.span);
                    inf.core.dynamic_
                }
                _ => inf.core.dynamic_,
            }
        }
        RefNome::TipoParam(_) | RefNome::TipoEmbutido => {
            inf.aviso(ASSIGNMENT_TO_TYPE.template.to_string(), n.span);
            inf.core.dynamic_
        }
        // `AssignmentVerifier.verify` (`assignment_verifier.dart:66-71`): a
        // recuperação da escrita é o prefixo.
        RefNome::Prefixo => {
            prefixo_sem_ponto(inf, n);
            inf.core.dynamic_
        }
        RefNome::MembroLexico(f, estatico) => {
            let r = resolved_de_membro_lexico(inf, cx, f, estatico);
            resolver(inf, cx, alvo, r);
            let fe = inf.program.function(f);
            // O setter de instância escrito onde `this` não vale (método
            // estático, fábrica, inicializador): o elemento de escrita é ele
            // (`setWriteElement`) e o erro é o do acesso sem `this`
            // (`_checkForInvalidInstanceMemberAccess`).
            if !estatico && erro_de_instancia_sem_this(inf, cx).is_some() {
                let tipo_de_escrita = match (fe.kind, fe.variable) {
                    (FunctionKind::Setter, _) => Some(inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_)),
                    (FunctionKind::ImplicitAccessor, Some(v)) if !inf.program.variable(v).final_ && !inf.program.variable(v).const_ => {
                        Some(inf.tipo_variavel(v))
                    }
                    _ => None,
                };
                if let Some(t) = tipo_de_escrita {
                    avisar_instancia_sem_this(inf, cx, n);
                    return t;
                }
            }
            if fe.kind == FunctionKind::Function && fe.class.is_some() {
                avisar_escrita_em_metodo(inf, n);
                // `setWriteElement`: o tipo de escrita só vem de setter ou
                // variável; de um método é `InvalidType`.
                return inf.table.invalido(inf.core.dynamic_);
            }
            if let (FunctionKind::ImplicitAccessor, Some(v)) = (fe.kind, fe.variable) {
                let ve = inf.program.variable(v);
                if (ve.final_ || ve.const_) && ve.setter.is_none() {
                    // Campo final sem setter: pode haver setter herdado.
                    if let Some(this) = cx.tipo_this {
                        if let Busca::Achado(m) = inf.buscar_membro(cx.lib, this, n.sym, true) {
                            return m.tipo;
                        }
                    }
                }
            }
            if !estatico {
                if let Some(this) = cx.tipo_this {
                    if let Busca::Achado(m) = inf.buscar_membro(cx.lib, this, n.sym, true) {
                        return m.tipo;
                    }
                }
            }
            avisar_membro_sem_setter(inf, n, f);
            // Sem setter, o elemento de escrita é o de recuperação (o getter):
            // `InvalidType` (`setWriteElement`).
            inf.table.invalido(inf.core.dynamic_)
        }
        RefNome::ThisImplicito => {
            let this = cx.tipo_this.unwrap();
            // `ThisLookup.lookupSetter` com `this` potencialmente anulável:
            // o pai é a atribuição, então o código é o do acesso.
            if inf.exige_checagem_de_nulo(cx.lib, this, n.sym, true) {
                let nome = inf.interner.resolve(n.sym).to_string();
                let desde = inf.diagnostics.len();
                inf.aviso_de_nulo(this, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE, n.span, &[&nome]);
                inf.anexar_nao_promocao(desde, cx, None, n.span);
                let nn = inf.nao_nulo(this);
                return match inf.buscar_membro(cx.lib, nn, n.sym, true) {
                    Busca::Achado(m) => {
                        resolver(inf, cx, alvo, m.resolved.clone());
                        m.tipo
                    }
                    _ => inf.core.dynamic_,
                };
            }
            match inf.buscar_membro(cx.lib, this, n.sym, true) {
                Busca::Achado(m) => {
                    resolver(inf, cx, alvo, m.resolved.clone());
                    m.tipo
                }
                Busca::Ausente => {
                    if let Busca::Achado(getter) = inf.buscar_membro(cx.lib, this, n.sym, false) {
                        if let Some(f) = getter.funcao {
                            if avisar_membro_sem_setter(inf, n, f) {
                                resolver(inf, cx, alvo, getter.resolved);
                                return getter.tipo;
                            }
                        }
                    }
                    nome_escrito_indefinido(inf, cx, n);
                    inf.core.dynamic_
                }
                // Extensões ambíguas (`ThisLookup.lookupSetter`): a
                // ambiguidade e, sem setter, o nome indefinido.
                Busca::Dinamico if inf.ambiguidade_de_extensao.is_some() => {
                    inf.relatar_ambiguidade_de_extensao(n.span);
                    nome_indefinido_sem_this(inf, cx, n);
                    inf.core.dynamic_
                }
                _ => inf.core.dynamic_,
            }
        }
        RefNome::ConstanteEnum(v) => inf.tipo_variavel(v),
        // Só curingas declaram `_` aqui (3.7): usar `_` é erro
        // (`Undefined name '_'` no CFE).
        RefNome::Nenhum if cx.curinga == Some(n.sym) => {
            inf.erro_de_linguagem(cx.unit, n.span, WILDCARD_NAO_LIGA.template.to_string());
            inf.core.dynamic_
        }
        RefNome::Nenhum => {
            nome_escrito_indefinido(inf, cx, n);
            inf.core.dynamic_
        }
        _ => inf.core.dynamic_,
    }
}

/// A escrita num nome simples sem setter no escopo léxico
/// (`AssignmentVerifier.verify`, `an611:src/error/assignment_verifier.dart:28-113`,
/// com `ThisLookup.lookupSetter`): o `this` implícito ainda pode achar o
/// setter (o erro é então o de acesso sem `this` ou o de estático não
/// qualificado) ou um getter de recuperação (o erro é outro: escrita em
/// final, em método…); senão, nome indefinido — sem a exceção dos imports
/// que não existem, que a escrita não consulta.
fn nome_escrito_indefinido(inf: &mut BodyInferrer<'_>, cx: &Corpo, n: ast::Name) {
    if let Some(this) = tipo_this_do_analyzer(inf, cx) {
        match buscar_pelo_this(inf, cx, this, n.sym, true) {
            PeloThis::Instancia => {
                avisar_instancia_sem_this(inf, cx, n);
                return;
            }
            PeloThis::Estatico(dono, _) => {
                avisar_estatico_nao_qualificado(inf, cx, dono, n);
                return;
            }
            PeloThis::Incerto => return,
            PeloThis::Ausente => {}
        }
        if !matches!(buscar_pelo_this(inf, cx, this, n.sym, false), PeloThis::Ausente) {
            return;
        }
    }
    let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, inf.interner.resolve(n.sym));
    inf.aviso(msg, n.span);
}

/// Atribuições: `=`, compostas e `??=`.
fn atribuicao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, op: AssignOp, alvo: ExprId, valor: ExprId, curto: &mut bool) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    let a = ast(inf, cx);
    let alvo_kind = &a.expr(alvo).kind;
    match op {
        AssignOp::Assign => {
            // Tipo de escrita e contexto.
            let (escrita, contexto, local) = match alvo_kind {
                ExprKind::Identifier(n) => {
                    let n = *n;
                    match resolver_nome(inf, cx, n.sym, true) {
                        RefNome::Local(id) => {
                            resolver(inf, cx, alvo, Resolved::Local(id));
                            referencia_de_juncao(inf, cx, id, n);
                            escrita_em_guarda(inf, cx, id, n.span);
                            escrita_em_primario(inf, cx, id, n.span);
                            let l = cx.local(id).clone();
                            if cx.funcoes_locais.contains(&id) {
                                inf.aviso(ASSIGNMENT_TO_FUNCTION.template.to_string(), n.span);
                                (inf.core.dynamic_, inf.core.dynamic_, None)
                            } else {
                                // `assignment_expression_resolver.dart:360-385`:
                                // `late final` já atribuída e `final` talvez atribuída.
                                if l.const_ {
                                    if !cx.fluxo.nao_atribuida(id) {
                                        inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), n.span);
                                    }
                                } else if l.final_ && l.late {
                                    if cx.fluxo.atribuida(id) {
                                        inf.aviso_com_codigo(
                                            dartforge_diagnostics::codigos::compile_time_error::LATE_FINAL_LOCAL_ALREADY_ASSIGNED,
                                            n.span,
                                            &[],
                                        );
                                    }
                                } else if l.final_ && !cx.fluxo.nao_atribuida(id) {
                                    let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
                                    inf.aviso(msg, n.span);
                                }
                                let atual = cx.fluxo.tipo_atual(id, l.tipo);
                                (l.tipo, atual, Some(id))
                            }
                        }
                        _ => {
                            let t = tipo_de_escrita_nome(inf, cx, alvo, n);
                            (t, t, None)
                        }
                    }
                }
                ExprKind::Property { target, name, null_aware } => {
                    let (target, name, null_aware) = (*target, *name, *null_aware);
                    let t = escrita_propriedade(inf, cx, alvo, target, name, null_aware, curto, None);
                    (t, t, None)
                }
                ExprKind::Index { target, index, null_aware } => {
                    let (target, index, null_aware) = (*target, *index, *null_aware);
                    let (recv, c) = receptor(inf, cx, target, null_aware);
                    *curto = c;
                    let bruto = inf.body_types.units[cx.unit.0 as usize].get_type(target).unwrap_or(recv);
                    let limite = inf.resolver_ao_limite(bruto);
                    if receptor_nunca(inf, cx, target, limite) {
                        inferir_livre(inf, cx, index);
                        let d = inf.core.dynamic_;
                        (d, d, None)
                    } else {
                        let t = escrita_indice(inf, cx, alvo, recv, index, span);
                        (t, t, None)
                    }
                }
                _ => {
                    let t = inferir_livre(inf, cx, alvo);
                    (t, t, None)
                }
            };
            inf.body_types.units[cx.unit.0 as usize].tipos_de_escrita.insert(e, escrita);
            // `flow.write(…, rhs)` (`_write`, `flow_analysis.dart:6133-6149`):
            // a escrita de uma condição numa local guarda os modelos
            // verdadeiro/falso dela no nó SSA, como o inicializador (§7.10).
            let (tv, condicao_guardada) = if local.is_some() && e_forma_de_condicao(inf, cx, valor) {
                let (sim, nao) = condicao(inf, cx, valor);
                cx.fluxo = inf.juntar(&sim, &nao);
                let t = inf.body_types.units[cx.unit.0 as usize].get_type(valor).unwrap_or(inf.core.bool_);
                (t, Some((sim, nao)))
            } else {
                (inferir(inf, cx, valor, contexto), None)
            };
            if let Some(id) = local {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                let motivo = super::fluxo::MotivoDeNaoPromocao::Escrita { nome: cx.local(id).nome, span: inf.span_expr(cx.unit, e) };
                inf.atribuir_fluxo(&mut f, id, decl, tv, Some(motivo));
                cx.fluxo = f;
                cx.esquecer_campos_de(id);
                if let (Some((sim, nao)), Some(versao)) = (condicao_guardada, cx.fluxo.versao(id)) {
                    cx.condicoes.insert(id, (sim, nao, versao));
                }
            }
            if !inf.e_dynamic(escrita) {
                verificar_atribuivel_expr_em(inf, cx, valor, tv, escrita, INVALID_ASSIGNMENT.template, false);
            } else {
                uso_de_void(inf, cx, valor, tv);
            }
            tv
        }
        AssignOp::Compound(bop) => {
            let (leitura, escrita, local) = ler_para_escrita(inf, cx, alvo, curto);
            inf.body_types.units[cx.unit.0 as usize].tipos_de_escrita.insert(e, escrita);
            // `x += 1` / `x ??= 3` com `x` de tipo `void`: no operador.
            if matches!(inf.table.get(leitura), Type::Void) {
                let token = token_de_operador(inf, cx, inf.span_expr(cx.unit, alvo).end);
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, token, &[]);
            }
            if bop == BinaryOp::IfNull {
                // `checkFinalAlreadyAssigned(left)` antes do lado direito
                // (3.6.2 `assignment_expression_resolver.dart:79`), com o
                // estado de antes da escrita.
                if let Some(id) = local {
                    let alvo_span = inf.span_expr(cx.unit, alvo);
                    check_final_local(inf, cx, id, alvo_span);
                }
                // O lado direito só roda se o alvo for nulo: o que ele promove
                // ou atribui não vale depois (`origin ??= element!.library;`
                // não promove `element`).
                let antes = cx.fluxo.clone();
                // O índice sem `[]` resolvido (receptor `Never`) tem o
                // `readType` inválido (3.6.2 `resolver.dart:1665-1672`): o
                // `isStrictlyNonNullable` não vale.
                // O mesmo para o nome que não é getter nem variável (classe,
                // alias, função, método, parâmetro de tipo: `C ??= null`).
                let sem_leitura = match &ast(inf, cx).expr(alvo).kind {
                    ExprKind::Index { target, null_aware: false, .. } => inf.body_types.units[cx.unit.0 as usize]
                        .get_type(*target)
                        .is_some_and(|r| matches!(inf.table.get(inf.resolver_ao_limite(r)), Type::Never)),
                    ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                        let e_metodo = |inf: &BodyInferrer<'_>, f: dartforge_elements::model::FunctionElementId| {
                            matches!(inf.program.function(f).kind, FunctionKind::Function)
                        };
                        match inf.body_types.units[cx.unit.0 as usize].get_resolved(alvo).cloned() {
                            Some(Resolved::Element(Element::Function(f))) => e_metodo(inf, f),
                            Some(Resolved::Element(Element::Variable(_))) => false,
                            Some(Resolved::Element(_)) => true,
                            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => e_metodo(inf, f),
                            Some(Resolved::ExtensionMember { member: f, .. }) => e_metodo(inf, f),
                            Some(Resolved::TypeParameter(_) | Resolved::Constructor(_) | Resolved::Prefix(_)) => true,
                            _ => false,
                        }
                    }
                    _ => false,
                };
                if !sem_leitura {
                    avisar_nulo_morto(inf, cx, leitura, valor);
                }
                let tv = inferir(inf, cx, valor, escrita);
                if !matches!(inf.table.get(escrita), Type::Void) {
                    uso_de_void(inf, cx, valor, tv);
                }
                // `x ??= e`: o tipo de `e` contra o tipo de escrita, no lado
                // direito (`_checkForInvalidAssignment`,
                // an611:src/dart/resolver/assignment_expression_resolver.dart:117-155, 263-270).
                if !inf.e_dynamic(escrita) && !matches!(inf.table.get(tv), Type::Void) {
                    let sp = inf.span_expr(cx.unit, valor);
                    inf.verificar_atribuivel(tv, escrita, sp, INVALID_ASSIGNMENT.template);
                }
                let nn = inf.nao_nulo(leitura);
                let t = inf.up(nn, tv);
                // A escrita acontece só no ramo do nulo (`ifNullExpression_
                // rightBegin` … `write` … `ifNullExpression_end`): depois da
                // junção o local fica só potencialmente atribuído se não
                // estava antes.
                if let Some(id) = local {
                    let decl = cx.local(id).tipo;
                    let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                    let motivo = super::fluxo::MotivoDeNaoPromocao::Escrita { nome: cx.local(id).nome, span: inf.span_expr(cx.unit, e) };
                    inf.atribuir_fluxo(&mut f, id, decl, tv, Some(motivo));
                    cx.fluxo = f;
                }
                let depois = cx.fluxo.clone();
                // No ramo sem o lado direito o alvo não é nulo.
                let mut antes = antes;
                if let Some(id) = local {
                    let decl = cx.local(id).tipo;
                    inf.promover_nao_nulo(&mut antes, id, decl);
                }
                cx.fluxo = inf.juntar(&antes, &depois);
                return t;
            }
            let sym = simbolo_operador(inf, bop);
            let token = token_de_operador(inf, cx, inf.span_expr(cx.unit, alvo).end);
            // O contexto do lado direito: `refineNumericInvocationContext`
            // com o tipo de escrita como alvo e como contexto.
            let posicoes = PosicoesDeOperador { token, indefinido: token, composta: true, super_: false, alvo_numerico: Some(escrita) };
            let (t, _) = operador_binario(inf, cx, leitura, sym, valor, escrita, posicoes, Some(e));
            // `x op= e`: o retorno do operador contra o tipo de escrita, no
            // lado direito (`_resolveTypes` + `_checkForInvalidAssignment`,
            // an611:src/dart/resolver/assignment_expression_resolver.dart:117-155, 272-290).
            if !inf.e_dynamic(escrita) && !inf.e_dynamic(leitura) && !matches!(inf.table.get(t), Type::Void) && (local.is_some() || leitura == escrita) {
                let sp = inf.span_expr(cx.unit, valor);
                inf.verificar_atribuivel(t, escrita, sp, INVALID_ASSIGNMENT.template);
            }
            if let Some(id) = local {
                // A regra de escrita vê o estado de antes da escrita.
                let alvo_span = inf.span_expr(cx.unit, alvo);
                check_final_local(inf, cx, id, alvo_span);
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                let motivo = super::fluxo::MotivoDeNaoPromocao::Escrita { nome: cx.local(id).nome, span: inf.span_expr(cx.unit, e) };
                inf.atribuir_fluxo(&mut f, id, decl, t, Some(motivo));
                cx.fluxo = f;
            }
            t
        }
    }
}

/// `r.x = …`: tipo do setter.
fn escrita_propriedade(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, target: ExprId, name: ast::Name, null_aware: bool, curto: &mut bool, recv_lido: Option<(TypeId, bool)>) -> TypeId {
    let a = ast(inf, cx);
    if let ExprKind::Identifier(p) = &a.expr(target).kind {
        if matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
            if null_aware {
                prefixo_sem_ponto(inf, *p);
            }
            resolver(inf, cx, target, Resolved::Prefix(cx.lib));
            if importacao_ambigua(inf, cx, Some(p.sym), name) {
                return inf.table.invalido(inf.core.dynamic_);
            }
            if let Some(el) = inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.setter.or(b.getter)) {
                resolver(inf, cx, alvo, Resolved::Element(el));
                return match el {
                    Element::Variable(v) => inf.tipo_variavel(v),
                    Element::Function(f) => {
                        let fe = inf.program.function(f);
                        match (fe.kind, fe.variable) {
                            (FunctionKind::ImplicitAccessor, Some(v)) => inf.tipo_variavel(v),
                            _ => inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_),
                        }
                    }
                    _ => inf.core.dynamic_,
                };
            }
            return inf.core.dynamic_;
        }
    }
    if null_aware && referencia_a_tipo(inf, cx, target).is_some() {
        operador_nulo_em_tipo(inf, cx, target);
    }
    if let Some(rt) = referencia_a_tipo(inf, cx, target) {
        registrar_ref_tipo(inf, cx, target);
        if null_aware && !encurtamento_dispensado(inf, cx, target, &rt) {
            *curto = true;
        }
        let instancia_explicita = matches!(&ast(inf, cx).expr(target).kind, ExprKind::TypeArguments { .. });
        if instancia_explicita {
            if let RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) = &rt {
                if inf.membro_estatico(*c, name.sym, true).is_some()
                    || inf.membro_estatico(*c, name.sym, false).is_some()
                {
                    avisar_instanciacao_estatica(inf, cx, alvo, name);
                    return inf.core.dynamic_;
                }
                if avisar_instanciacao_de_classe(inf, cx, alvo, *c, name, true) {
                    return inf.core.dynamic_;
                }
            }
        }
        let m = match &rt {
            RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) => inf.membro_estatico(*c, name.sym, true),
            RefTipo::Extensao(x) => inf.membro_estatico_de_extensao(*x, name.sym, true),
        };
        return match m {
            Some(m) => {
                resolver(inf, cx, alvo, m.resolved.clone());
                m.tipo
            }
            None => {
                if let RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) = &rt {
                    if inf.membro_estatico(*c, name.sym, false).is_none() {
                        // Um método de instância não declara setter. Em `C.m = v`,
                        // o analyzer procura o setter estático e relata sua
                        // ausência, enquanto `C.m` como leitura é acesso
                        // estático indevido ao método de instância.
                        let classe = inf.program.class(*c);
                        let setter_de_instancia = inf.chave_setter(name.sym)
                            .is_some_and(|chave| classe.instance_members.contains_key(&chave));
                        let metodo_de_instancia = classe.instance_members.get(&name.sym)
                            .is_some_and(|&f| inf.program.function(f).kind == FunctionKind::Function);
                        if !instancia_explicita && !setter_de_instancia && metodo_de_instancia
                            && (!inf.interner.resolve(name.sym).starts_with('_') || classe.library == cx.lib)
                        {
                            let msg = format!(
                                "{}: setter '{}' não definido para o tipo '{}'",
                                UNDEFINED_SETTER.template,
                                inf.interner.resolve(name.sym),
                                inf.interner.resolve(classe.name),
                            );
                            inf.aviso(msg, name.span);
                            return inf.core.dynamic_;
                        }
                        if !instancia_explicita && avisar_acesso_estatico_a_instancia(inf, cx, *c, name, true) {
                            return inf.core.dynamic_;
                        }
                    }
                }
                if let RefTipo::Extensao(x) = rt {
                    if inf.membro_de_extensao_explicita(x, &[], name.sym, true).is_some() {
                        let msg = format!("{}: '{}'", STATIC_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
                        inf.aviso(msg, name.span);
                        return inf.core.dynamic_;
                    }
                }
                let getter = match rt {
                    RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) => inf.membro_estatico(c, name.sym, false),
                    RefTipo::Extensao(x) => inf.membro_estatico_de_extensao(x, name.sym, false),
                };
                let getter_ausente = getter.is_none();
                if let Some(getter) = getter {
                    if let Some(f) = getter.funcao {
                        if avisar_membro_sem_setter(inf, name, f) {
                            resolver(inf, cx, alvo, getter.resolved);
                            return getter.tipo;
                        }
                    }
                }
                if getter_ausente && let RefTipo::Extensao(x) = rt {
                    let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                    let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_SETTER.template, inf.interner.resolve(name.sym), extensao);
                    inf.aviso(msg, name.span);
                }
                // `_resolveTargetInterfaceElement` sem setter nem getter
                // (`augmented.getGetter`, que também vê os de instância): o
                // `AssignmentVerifier` relata `UNDEFINED_SETTER` com o
                // `thisType` da classe (3.6.2 `property_element_resolver.dart:708-731`,
                // `assignment_verifier.dart:96-106`); os estáticos da
                // superclasse não contam (`C.s = 1`), e `C.new = 1` também.
                if getter_ausente
                    && !instancia_explicita
                    && let RefTipo::Classe(c, _) = rt
                {
                    let classe = inf.program.class(c);
                    let de_instancia = classe.instance_members.contains_key(&name.sym)
                        || inf.chave_setter(name.sym).is_some_and(|ch| classe.instance_members.contains_key(&ch));
                    if !de_instancia && name.span.start != name.span.end {
                        let this = inf.tipo_this_classe(c);
                        let msg = format!(
                            "{}: setter '{}' não definido para o tipo '{}'",
                            UNDEFINED_SETTER.template,
                            inf.interner.resolve(name.sym),
                            inf.table.format(this, inf.interner, inf.program),
                        );
                        inf.aviso(msg, name.span);
                    }
                }
                inf.core.dynamic_
            }
        };
    }
    if matches!(a.expr(target).kind, ExprKind::Super) {
        let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
        registrar(inf, cx, target, this);
        return membro_super(inf, cx, alvo, name, UsoDoSuper::Escrita);
    }
    // Na escrita composta (`x.p += 1`) o receptor já foi lido, e a checagem
    // de nulo saiu na leitura (uma só resolução no analyzer).
    let composta = recv_lido.is_some();
    let (recv, c) = recv_lido.unwrap_or_else(|| receptor(inf, cx, target, null_aware));
    *curto = c;
    // Receptor `void` (`property_element_resolver.dart:449-455`): no nome.
    if !cx.sobreposicoes.contains_key(&target) && matches!(inf.table.get(recv), Type::Void) {
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, name.span, &[]);
        return inf.core.dynamic_;
    }
    // `x.new = v` (`type_property_resolver.dart:75-79`): sem setter `new`.
    if !cx.sobreposicoes.contains_key(&target) && inf.interner.resolve(name.sym) == "new" {
        let msg = format!(
            "{}: setter '{}' não definido para o tipo '{}'",
            UNDEFINED_SETTER.template,
            "new",
            inf.table.format(recv, inf.interner, inf.program),
        );
        inf.aviso(msg, name.span);
        return inf.core.dynamic_;
    }
    // `E(valor).m` força a extensão nomeada: na falta de setter ela emite
    // `undefined_extension_setter`, mesmo que a extensão tenha um getter `m`.
    if let Some((x, args)) = cx.sobreposicoes.get(&target).cloned() {
        if let Some(m) = inf.membro_de_extensao_explicita(x, &args, name.sym, true) {
            resolver(inf, cx, alvo, m.resolved);
            return m.tipo;
        }
        if inf.membro_estatico_de_extensao(x, name.sym, true).is_some() {
            // `+=` resolve leitura e escrita do mesmo nome. O analyzer relata
            // o acesso estático uma vez, mesmo quando ambos os lados resolvem.
            let ja_reportado = inf.diagnostics.iter().zip(&inf.unidades_dos_avisos).any(|(d, unidade)| {
                *unidade == inf.unidade_corrente
                    && d.span == name.span
                    && d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template
            });
            if !ja_reportado {
                inf.aviso(EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(), name.span);
            }
            return inf.core.dynamic_;
        }
        let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
        let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_SETTER.template, inf.interner.resolve(name.sym), extensao);
        inf.aviso(msg, name.span);
        return inf.core.dynamic_;
    }
    // Receptor anulável sem o getter nem o setter em `Object` ou numa
    // extensão do tipo anulável: `UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE`
    // (o `_hasGetterOrSetter` do `TypePropertyResolver.resolve`,
    // an362:src/dart/resolver/type_property_resolver.dart:99-180), e então
    // nada de `undefined_setter`; a resolução segue pelo tipo não anulável.
    let checar_nulo = !composta
        && inf.exige_checagem_de_nulo(cx.lib, recv, name.sym, false)
        && inf.exige_checagem_de_nulo(cx.lib, recv, name.sym, true);
    if checar_nulo {
        let nome = inf.interner.resolve(name.sym).to_string();
        let desde = inf.diagnostics.len();
        inf.aviso_de_nulo(
            recv,
            dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
            name.span,
            &[&nome],
        );
        inf.anexar_nao_promocao(desde, cx, Some(target), name.span);
    }
    // Um método da interface da classe prevalece sobre setter de extensão homônimo.
    if let Some(m) = inf.membro_de_interface(recv, name.sym, false) {
        if m.metodo && m.funcao.is_some_and(|f| inf.program.function(f).kind == FunctionKind::Function) {
            avisar_escrita_em_metodo(inf, name);
            resolver(inf, cx, alvo, m.resolved);
            // `setWriteElement`: escrita num método tem tipo `InvalidType`.
            return inf.table.invalido(inf.core.dynamic_);
        }
    }
    let busca = inf.buscar_membro(cx.lib, recv, name.sym, true);
    inf.relatar_ambiguidade_de_extensao(name.span);
    match busca {
        Busca::Achado(m) => {
            resolver(inf, cx, alvo, m.resolved.clone());
            m.tipo
        }
        Busca::Ausente => {
            // O setter estático recuperado (`lookupStaticSetter`).
            if let Some(m) = inf.acesso_de_instancia_a_estatico(cx.lib, recv, name.sym, true, name.span) {
                resolver(inf, cx, alvo, m.resolved);
                return m.tipo;
            }
            if checar_nulo {
                return inf.core.dynamic_;
            }
            // O analyzer recupera o getter quando a escrita não encontra um
            // setter (`AssignmentVerifier.verify` com o `result.getter`, que
            // pode ser o estático recuperado). Um getter declarado numa classe
            // tem diagnóstico próprio; sem getter, continua sendo um setter
            // indefinido.
            let getter = match inf.buscar_membro(cx.lib, recv, name.sym, false) {
                Busca::Achado(getter) => Some(getter),
                Busca::Ausente => match inf.recuperacao_estatica_do_receptor(cx.lib, recv, name.sym, false) {
                    Some((dono, true)) => {
                        let m = inf.membro_estatico(dono, name.sym, false);
                        avisar_escrita_em_metodo(inf, name);
                        if let Some(m) = m {
                            resolver(inf, cx, alvo, m.resolved);
                            return inf.table.invalido(inf.core.dynamic_);
                        }
                        return inf.core.dynamic_;
                    }
                    Some((dono, false)) => inf.membro_estatico(dono, name.sym, false),
                    None => None,
                },
                _ => None,
            };
            if let Some(getter) = getter {
                if let Some(f) = getter.funcao {
                    if avisar_membro_sem_setter(inf, name, f) {
                        resolver(inf, cx, alvo, getter.resolved);
                        return getter.tipo;
                    }
                } else if let Resolved::Member { member: MemberRef::Variable(v), .. } = getter.resolved {
                    // A constante de enum (`static const`): `ASSIGNMENT_TO_CONST`.
                    if inf.program.variable(v).const_ {
                        inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), name.span);
                        resolver(inf, cx, alvo, getter.resolved);
                        return getter.tipo;
                    }
                }
            }
            let msg = format!(
                "{}: setter '{}' não definido para o tipo '{}'",
                UNDEFINED_SETTER.template,
                inf.interner.resolve(name.sym),
                inf.table.format(recv, inf.interner, inf.program)
            );
            inf.aviso(msg, name.span);
            inf.core.dynamic_
        }
        _ => {
            resolver(inf, cx, alvo, Resolved::Dynamic);
            inf.core.dynamic_
        }
    }
}

/// `r[i] = …`: tipo do valor em `[]=`.
fn escrita_indice(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: ExprId, recv: TypeId, index: ExprId, span: dartforge_diagnostics::Span) -> TypeId {
    let target = match &ast(inf, cx).expr(alvo).kind {
        ExprKind::Index { target, .. } => Some(*target),
        _ => None,
    };
    let busca = if let Some(target) = target
        && let Some((x, args)) = cx.sobreposicoes.get(&target).cloned()
    {
        match inf.sym.indice_set.and_then(|op| inf.membro_de_extensao_explicita(x, &args, op, false)) {
            Some(m) => Busca::Achado(m),
            None => {
                inferir_livre(inf, cx, index);
                let sp = span_indice(inf, cx, alvo, target);
                avisar_operador_de_extensao(inf, x, "[]=", sp);
                return inf.core.dynamic_;
            }
        }
    } else {
        let Some(op) = inf.sym.indice_set else {
            inferir_livre(inf, cx, index);
            return inf.core.dynamic_;
        };
        // `resolveIndexExpression` (`property_element_resolver.dart:80-115`):
        // o `TypePropertyResolver` com o nome `[]` (que procura `[]` e
        // `[]=`); receptor potencialmente anulável é
        // `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE` `['[]']` no `[`
        // (o `IndexExpression` é `MethodReferenceExpression`).
        if let Some(t) = target
            && let Some(ler) = inf.interner.lookup("[]")
            && inf.exige_checagem_de_nulo(cx.lib, recv, ler, false)
            && inf.exige_checagem_de_nulo(cx.lib, recv, op, false)
        {
            let inicio = span_indice(inf, cx, alvo, t).start;
            let sp = dartforge_diagnostics::Span { start: inicio, end: inicio + 1 };
            let desde = inf.diagnostics.len();
            inf.aviso_de_nulo(recv, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE, sp, &["[]"]);
            inf.anexar_nao_promocao(desde, cx, Some(t), sp);
        }
        // `super[i] = v`: o `[]=` da cadeia de `super`.
        if target.is_some_and(|t| matches!(ast(inf, cx).expr(t).kind, ExprKind::Super)) {
            buscar_operador_super(inf, cx, op)
        } else {
            inf.buscar_membro(cx.lib, recv, op, false)
        }
    };
    match busca {
        Busca::Achado(m) => {
            resolver(inf, cx, alvo, m.resolved.clone());
            let (pi, pv) = match inf.table.get(m.tipo).clone() {
                Type::Function { positional, .. } => (positional.first().copied(), positional.get(1).copied()),
                _ => (None, None),
            };
            let u = inf.core.unknown;
            let ti = inferir(inf, cx, index, pi.unwrap_or(u));
            if let Some(p) = pi {
                verificar_atribuivel_expr(inf, cx, index, ti, p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
            }
            pv.unwrap_or(inf.core.dynamic_)
        }
        Busca::Ausente => {
            inferir_livre(inf, cx, index);
            // `Never?` em `x?[i] = v`: promovido a `Never`, nada a relatar.
            let nn = inf.nao_nulo(recv);
            if matches!(inf.table.get(nn), Type::Never) {
                return inf.core.dynamic_;
            }
            // `[]=` ausente: `UNDEFINED_OPERATOR` (ou `…_SUPER_OPERATOR`) do
            // `[` ao `]` (`_reportUnresolvedIndex`,
            // an611:src/dart/resolver/property_element_resolver.dart:109-133, 365-381).
            let tipo = inf.table.format(recv, inf.interner, inf.program);
            let (sp, codigo) = match target {
                Some(t) => {
                    let sup = matches!(ast(inf, cx).expr(t).kind, ExprKind::Super);
                    let c = if sup {
                        dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_SUPER_OPERATOR
                    } else {
                        dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR
                    };
                    (span_indice(inf, cx, alvo, t), c)
                }
                None => (span, dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR),
            };
            // Composta sem `[]` nem `[]=`: o oráculo só relata o `[]=` (o
            // `[]` da leitura, no mesmo intervalo, sai).
            let mut i = 0;
            while i < inf.diagnostics.len() {
                let d = &inf.diagnostics[i];
                if d.span == sp && d.code == Some(codigo) && d.args.first().is_some_and(|a| &**a == "[]") {
                    inf.diagnostics.remove(i);
                    inf.unidades_dos_avisos.remove(i);
                } else {
                    i += 1;
                }
            }
            inf.aviso_com_codigo(codigo, sp, &["[]=", &tipo]);
            inf.core.dynamic_
        }
        _ => {
            inferir_livre(inf, cx, index);
            inf.core.dynamic_
        }
    }
}

// -------------------------------------------------------------------
// Condições: modelos de fluxo verdadeiro/falso
// -------------------------------------------------------------------

/// Infere uma condição (contexto `bool`) e devolve `(true(E), false(E))`.
/// Deixa `cx.fluxo` no estado de antes (o chamador escolhe o ramo).
pub(crate) fn condicao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId) -> (Fluxo, Fluxo) {
    let a = ast(inf, cx);
    let kind = &a.expr(e).kind;
    let span = a.expr(e).span;
    match kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            let r = condicao(inf, cx, i);
            let t = inf.body_types.units[cx.unit.0 as usize].get_type(i).unwrap_or(inf.core.bool_);
            registrar(inf, cx, e, t);
            r
        }
        ExprKind::Bool(b) => {
            let b = *b;
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            let f = cx.fluxo.clone();
            if b {
                (f.clone(), f.inalcancavel())
            } else {
                (f.inalcancavel(), f)
            }
        }
        ExprKind::Unary { op: UnaryOp::Not, operand } => {
            let o = *operand;
            let (v, f) = condicao(inf, cx, o);
            verificar_bool(inf, cx, o, UsoBool::Negacao);
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            (f, v)
        }
        ExprKind::Binary { op, left, right } if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq) => {
            let (op, left, right) = (*op, *left, *right);
            let r = condicao_binaria(inf, cx, e, op, left, right);
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            r
        }
        ExprKind::Is { value, ty, negated } => {
            let (value, ty, negated) = (*value, *ty, *negated);
            let r = teste_de_tipo(inf, cx, value, ty, negated, span);
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            r
        }
        _ => {
            let b = inf.core.bool_;
            inferir(inf, cx, e, b);
            let f = cx.fluxo.clone();
            // Leitura de variável de condição não reescrita (§7.10).
            if let ExprKind::Identifier(n) = &ast(inf, cx).expr(e).kind
                && let Some(Nome::Local(id)) = cx.buscar(n.sym)
                && let Some((sim, nao, versao)) = cx.condicoes.get(&id).cloned()
                && f.versao(id) == Some(versao)
                && !f.modelo(id).is_some_and(|m| m.capturada)
            {
                let v = inf.reaplicar(&f, &sim);
                let fa = inf.reaplicar(&f, &nao);
                return (v, fa);
            }
            (f.clone(), f)
        }
    }
}

/// Expressão cuja informação de condição não é trivial (`==`, `!=`, `&&`,
/// `||`, `!`, `is`), para guardar numa variável de condição.
pub(crate) fn e_forma_de_condicao(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    match &ast(inf, cx).expr(e).kind {
        ExprKind::Parenthesized(i) => e_forma_de_condicao(inf, cx, *i),
        ExprKind::Unary { op: UnaryOp::Not, .. } | ExprKind::Is { .. } => true,
        ExprKind::Binary { op, .. } => matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq),
        // `booleanLiteral`: o ramo oposto é inalcançável (informação não
        // trivial, guardada na variável de condição: `bool c = true; c ? a
        // : b` tem o `b` morto).
        ExprKind::Bool(_) => true,
        _ => false,
    }
}

/// Condição de `if`/`while`/`?:`/`for`/`when`, com o diagnóstico de não-`bool`.
pub(crate) fn condicao_verificada(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId) -> (Fluxo, Fluxo) {
    let r = condicao(inf, cx, e);
    verificar_bool(inf, cx, e, UsoBool::Condicao);
    r
}

/// Condição de `assert` (comando ou inicializador).
pub(crate) fn condicao_de_assert(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId) -> (Fluxo, Fluxo) {
    let r = condicao(inf, cx, e);
    verificar_bool(inf, cx, e, UsoBool::Assert);
    r
}

/// Onde uma expressão precisa ser `bool` (o `BoolExpressionVerifier` do
/// analyzer escolhe o código por aí).
#[derive(Clone, Copy)]
enum UsoBool {
    /// `if`, `while`, `do`, `for`, `?:`, `when`: `NON_BOOL_CONDITION`.
    Condicao,
    /// `assert`: `NON_BOOL_EXPRESSION`.
    Assert,
    /// Operando de `&&`/`||`: `NON_BOOL_OPERAND`.
    Operando(&'static str),
    /// Operando de `!`: `NON_BOOL_NEGATION_EXPRESSION`.
    Negacao,
}

/// `checkForNonBoolExpression`: tipo não atribuível a `bool`. Com o próprio
/// `bool` anulável (`bool?`), o erro é de uso de valor anulável como
/// condição, em qualquer dos usos. Com `void`, o analyzer relata o uso do
/// resultado `void` no lugar (fora daqui); nada sai.
fn verificar_bool(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, uso: UsoBool) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let t = inf.body_types.units[cx.unit.0 as usize].get_type(e).unwrap_or(inf.core.bool_);
    let b = inf.core.bool_;
    if uso_de_void(inf, cx, e, t) || inf.atribuivel(t, b) {
        return;
    }
    let sp = inf.span_expr(cx.unit, e);
    let e_bool = match (inf.table.get(t), inf.table.get(b)) {
        (Type::Interface { class: c1, .. }, Type::Interface { class: c2, .. }) => c1 == c2,
        _ => false,
    };
    if e_bool {
        let desde = inf.diagnostics.len();
        inf.aviso_com_codigo(c::UNCHECKED_USE_OF_NULLABLE_VALUE_AS_CONDITION, sp, &[]);
        inf.anexar_nao_promocao(desde, cx, Some(e), sp);
        return;
    }
    match uso {
        UsoBool::Condicao => inf.aviso(NON_BOOL_CONDITION.template.to_string(), sp),
        UsoBool::Negacao => inf.aviso(NON_BOOL_NEGATION_EXPRESSION.template.to_string(), sp),
        UsoBool::Assert => inf.aviso_com_codigo(c::NON_BOOL_EXPRESSION, sp, &[]),
        UsoBool::Operando(op) => inf.aviso_com_codigo(c::NON_BOOL_OPERAND, sp, &[op]),
    }
}

/// `UNNECESSARY_NULL_COMPARISON` (`_checkForInvariantNullComparison`,
/// `an611:src/error/best_practices_verifier.dart:1060-1092`): `null` literal
/// de um lado e o outro estritamente não anulável, do `null` ao operador ou
/// do operador ao `null`; e, no resolvedor (`binary_expression_resolver.dart:
/// 128-160`), uma local definitivamente não atribuída comparada com `null`
/// (`ALWAYS_NULL`).
/// `NullableDereferenceVerifier.expression` (`an611:src/error/nullable_dereference_verifier.dart:33-90`):
/// na expressão inteira, quando o tipo não é `dynamic`, inválido nem não
/// anulável; com o tipo `Null`, o `INVALID_USE_OF_NULL_VALUE`.
pub(crate) fn desreferencia_anulavel(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, t: TypeId, codigo: dartforge_diagnostics::Codigo) {
    if inf.e_dynamic(t) || inf.table.e_invalido(t) || inf.e_nao_anulavel(t) {
        return;
    }
    let sp = inf.span_expr(cx.unit, e);
    let desde = inf.diagnostics.len();
    inf.aviso_de_nulo(t, codigo, sp, &[]);
    // `_check`: `computeWhyNotPromotedMessages(errorNode, whyNotPromoted(errorNode))`.
    inf.anexar_nao_promocao(desde, cx, Some(e), sp);
}

/// `isDoubleNan` (`an611:src/error/best_practices_verifier.dart:2093-2100`):
/// o `PrefixedIdentifier` `double.nan`, só pela forma escrita.
pub(crate) fn e_double_nan(inf: &BodyInferrer<'_>, cx: &Corpo, x: ExprId) -> bool {
    let a = ast(inf, cx);
    match &a.expr(x).kind {
        ExprKind::Property { target, name, null_aware: false } => {
            matches!(&a.expr(*target).kind, ExprKind::Identifier(n) if inf.interner.resolve(n.sym) == "double")
                && inf.interner.resolve(name.sym) == "nan"
        }
        _ => false,
    }
}

/// `UNNECESSARY_NAN_COMPARISON` (`_checkForInvariantNanComparison`,
/// `an611:src/error/best_practices_verifier.dart:1031-1058`): `double.nan`
/// num lado de `==`/`!=`, do `double.nan` ao operador ou do operador ao fim.
fn comparacao_com_nan(inf: &mut BodyInferrer<'_>, cx: &Corpo, op: BinaryOp, left: ExprId, right: ExprId) {
    use dartforge_diagnostics::codigos::warning as w;
    let codigo = match op {
        BinaryOp::NotEq => w::UNNECESSARY_NAN_COMPARISON_TRUE,
        BinaryOp::Eq => w::UNNECESSARY_NAN_COMPARISON_FALSE,
        _ => return,
    };
    let (sl, sr) = (inf.span_expr(cx.unit, left), inf.span_expr(cx.unit, right));
    let operador = token_de_operador(inf, cx, sl.end);
    if e_double_nan(inf, cx, left) {
        inf.aviso_com_codigo(codigo, dartforge_diagnostics::Span { start: sl.start, end: operador.end }, &[]);
    } else if e_double_nan(inf, cx, right) {
        inf.aviso_com_codigo(codigo, dartforge_diagnostics::Span { start: operador.start, end: sr.end }, &[]);
    }
}

/// O `Null` do `dart:core` (`isDartCoreNull`): não o `Never?`.
pub(crate) fn e_null_do_core(inf: &BodyInferrer<'_>, t: TypeId) -> bool {
    matches!(inf.table.get(t), Type::Null) && !matches!(inf.table.exibicao(t), Some(crate::table::Exibicao::NeverAnulavel))
}

/// O `NullSafeApiVerifier` (`an611:src/error/null_safe_api_verifier.dart:29-80`):
/// `Future<T>.value(…)` e `Completer<T>.complete(…)` com `T` não anulável, no
/// máximo um argumento, e o argumento ausente (na chamada inteira) ou de
/// tipo `Null` (no argumento): `NULL_ARGUMENT_TO_NON_NULL_TYPE`.
fn api_sem_nulo(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, resultado: TypeId) {
    let a = ast(inf, cx);
    let argumento_de = |inf: &BodyInferrer<'_>, t: TypeId, classe: Option<ClassId>| -> Option<TypeId> {
        match inf.table.get(t) {
            Type::Interface { class, args, .. } if Some(*class) == classe && args.len() == 1 => Some(args[0]),
            _ => None,
        }
    };
    let completer = inf.core.async_library.and_then(|l| {
        let s = inf.interner.lookup("Completer")?;
        match inf.program.library(l).declared.get(&s)?.getter {
            Some(Element::Class(c)) => Some(c),
            _ => None,
        }
    });
    let futuro = inf.core.future_class;
    let (nome, tipo, args): (&str, TypeId, &ast::Arguments) = match &a.expr(e).kind {
        ExprKind::InstanceCreation { constructor: Some(n), arguments, .. } if inf.interner.resolve(n.sym) == "value" => {
            let Some(x) = argumento_de(inf, resultado, futuro) else { return };
            ("Future.value", x, arguments)
        }
        ExprKind::Call { target, arguments } => {
            let ExprKind::Property { target: receptor, name, .. } = &a.expr(*target).kind else { return };
            let bt = &inf.body_types.units[cx.unit.0 as usize];
            match bt.get_resolved(*target) {
                Some(Resolved::Constructor(f)) => {
                    if inf.interner.resolve(name.sym) != "value" || inf.program.function(*f).class != futuro {
                        return;
                    }
                    let Some(x) = argumento_de(inf, resultado, futuro) else { return };
                    ("Future.value", x, arguments)
                }
                _ if inf.interner.resolve(name.sym) == "complete" => {
                    let Some(rt) = bt.get_type(*receptor) else { return };
                    let Some(x) = argumento_de(inf, rt, completer) else { return };
                    ("Completer.complete", x, arguments)
                }
                _ => return,
            }
        }
        _ => return,
    };
    if args.args.len() > 1 || !inf.e_nao_anulavel(tipo) {
        return;
    }
    let argumento = args.args.first().map(|x| x.value);
    let tipo_do_argumento = argumento.map(|x| inf.body_types.units[cx.unit.0 as usize].get_type(x));
    let nulo = match tipo_do_argumento {
        None => true,
        Some(None) => return,
        Some(Some(t)) => matches!(inf.table.get(t), Type::Null),
    };
    if nulo {
        let sp = inf.span_expr(cx.unit, argumento.unwrap_or(e));
        let exibido = inf.table.format_sem_alias(tipo, inf.interner, inf.program);
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::warning::NULL_ARGUMENT_TO_NON_NULL_TYPE, sp, &[nome, &exibido]);
    }
}

fn comparacao_com_nulo(inf: &mut BodyInferrer<'_>, cx: &Corpo, op: BinaryOp, left: ExprId, right: ExprId, tl: TypeId, tr: TypeId) {
    use dartforge_diagnostics::codigos::warning as w;
    let a = ast(inf, cx);
    let nulo = |x: ExprId| matches!(a.expr(x).kind, ExprKind::Null);
    let (sl, sr) = (inf.span_expr(cx.unit, left), inf.span_expr(cx.unit, right));
    let operador = token_de_operador(inf, cx, sl.end);
    // `===`/`!==` (`UNSUPPORTED_OPERATOR`): o nó não é uma comparação de
    // igualdade para o `_checkForUnnecessaryNullComparison`.
    if inf.program.unit(cx.unit).source.get(operador.start..).is_some_and(|s| s.starts_with("===") || s.starts_with("!==")) {
        return;
    }
    let diferente = op == BinaryOp::NotEq;
    let local_nao_atribuida = |inf: &mut BodyInferrer<'_>, x: ExprId| match &ast(inf, cx).expr(x).kind {
        ExprKind::Identifier(_) => match inf.body_types.units[cx.unit.0 as usize].get_resolved(x) {
            Some(Resolved::Local(id)) => cx.fluxo.nao_atribuida(*id),
            _ => false,
        },
        _ => false,
    };
    let sempre_nulo = if diferente { w::UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_FALSE } else { w::UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_TRUE };
    if nulo(right) && local_nao_atribuida(inf, left) {
        inf.aviso_com_codigo(sempre_nulo, dartforge_diagnostics::Span { start: sl.start, end: operador.end }, &[]);
    } else if nulo(left) && local_nao_atribuida(inf, right) {
        inf.aviso_com_codigo(sempre_nulo, dartforge_diagnostics::Span { start: operador.start, end: sr.end }, &[]);
    }
    let nunca_nulo = if diferente { w::UNNECESSARY_NULL_COMPARISON_NEVER_NULL_TRUE } else { w::UNNECESSARY_NULL_COMPARISON_NEVER_NULL_FALSE };
    if nulo(left) && estritamente_nao_anulavel(inf, tr) {
        inf.aviso_com_codigo(nunca_nulo, dartforge_diagnostics::Span { start: sl.start, end: operador.end }, &[]);
    }
    if nulo(right) && estritamente_nao_anulavel(inf, tl) {
        inf.aviso_com_codigo(nunca_nulo, dartforge_diagnostics::Span { start: operador.start, end: sr.end }, &[]);
    }
}

fn condicao_binaria(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, op: BinaryOp, left: ExprId, right: ExprId) -> (Fluxo, Fluxo) {
    match op {
        BinaryOp::And | BinaryOp::Or => {
            let antes = cx.fluxo.clone();
            let simbolo = if op == BinaryOp::And { "&&" } else { "||" };
            let (lv, lf) = condicao(inf, cx, left);
            verificar_bool(inf, cx, left, UsoBool::Operando(simbolo));
            cx.fluxo = if op == BinaryOp::And { lv.clone() } else { lf.clone() };
            // O código morto do lado direito começa no operador.
            let inicio = {
                let fim_esq = inf.span_expr(cx.unit, left).end;
                let ini_dir = inf.span_expr(cx.unit, right).start;
                let trecho = inf.program.unit(cx.unit).source.get(fim_esq..ini_dir).unwrap_or("");
                fim_esq + pular_espacos_e_comentarios(trecho)
            };
            let (rv, rf) = operando_de_fluxo(inf, cx, right, Some(inicio), |inf, cx| condicao(inf, cx, right));
            verificar_bool(inf, cx, right, UsoBool::Operando(simbolo));
            cx.fluxo = antes;
            if op == BinaryOp::And {
                let falso = inf.juntar(&lf, &rf);
                (rv, falso)
            } else {
                let verdadeiro = inf.juntar(&lv, &rv);
                (verdadeiro, rf)
            }
        }
        _ => {
            // `==` / `!=`
            let u = inf.core.unknown;
            let tl = inferir(inf, cx, left, u);
            uso_de_void(inf, cx, left, tl);
            // `e == .x`: o atalho à direita usa o tipo de `e` (3.10).
            atalhos::registrar_igualdade(inf, cx, right, tl);
            morto_no_operando_direito(inf, cx, left, right);
            let tr = inferir(inf, cx, right, u);
            // O lado direito é o argumento de `operator ==(Object)`.
            uso_de_void(inf, cx, right, tr);
            receptor_nunca(inf, cx, left, tl);
            comparacao_com_nulo(inf, cx, op, left, right, tl, tr);
            comparacao_com_nan(inf, cx, op, left, right);
            if matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, left, this);
            }
            // `E(x) == y`: o `==` só na extensão (`_resolveUserDefinableElement`
            // com `ExtensionOverride`); ausente, `UNDEFINED_EXTENSION_OPERATOR`
            // no operador, com o nome `==` também para `!=`.
            if let Some((x, args)) = cx.sobreposicoes.get(&left).cloned()
                && let Some(eq) = inf.sym.igual
                && inf.membro_de_extensao_explicita(x, &args, eq, false).is_none()
            {
                let token = token_de_operador(inf, cx, inf.span_expr(cx.unit, left).end);
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, "==", extensao);
                inf.aviso(msg, token);
            }
            // `super == x`: o `==` da cadeia de `super`, com o parâmetro
            // tornado anulável (`_resolveEqual`): `super == 'a'` contra
            // `==(covariant num other)` é `ARGUMENT_TYPE_NOT_ASSIGNABLE` de `num?`.
            if let Some(eq) = inf.sym.igual
                && matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super)
            {
                if let Busca::Achado(m) = buscar_operador_super(inf, cx, eq) {
                    let param = match inf.table.get(m.tipo) {
                        Type::Function { positional, .. } => positional.first().copied(),
                        _ => None,
                    };
                    if let Some(p) = param
                        && !matches!(inf.table.get(tr), Type::Void)
                    {
                        let p = inf.anulavel(p);
                        verificar_atribuivel_expr(inf, cx, right, tr, p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                    }
                    resolver(inf, cx, e, m.resolved);
                }
            } else if let Some(eq) = inf.sym.igual {
                if let Busca::Achado(m) = inf.buscar_membro(cx.lib, tl, eq, false) {
                    // O lado direito contra o parâmetro de `operator ==`
                    // tornado anulável (`binary_expression_resolver.dart:117-124`,
                    // `promoteParameterToNullable`), procurado no tipo
                    // esquerdo promovido a não nulo.
                    let tl_nn = inf.nao_nulo(tl);
                    let membro = if tl_nn != tl {
                        match inf.buscar_membro(cx.lib, tl_nn, eq, false) {
                            Busca::Achado(m2) => Some(m2),
                            _ => None,
                        }
                    } else {
                        Some(m.clone())
                    };
                    let param = membro.and_then(|m| match inf.table.get(m.tipo) {
                        Type::Function { positional, .. } => positional.first().copied(),
                        _ => None,
                    });
                    if let Some(p) = param
                        && !matches!(inf.table.get(tr), Type::Void)
                        && !cx.sobreposicoes.contains_key(&left)
                        && !matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super)
                    {
                        let p = inf.anulavel(p);
                        verificar_atribuivel_expr(inf, cx, right, tr, p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                    }
                    resolver(inf, cx, e, m.resolved);
                }
            }
            let depois = cx.fluxo.clone();
            let a = ast(inf, cx);
            let e_nulo = |x: ExprId| {
                let mut x = x;
                loop {
                    match &a.expr(x).kind {
                        ExprKind::Parenthesized(i) => x = *i,
                        ExprKind::Null => return true,
                        _ => return false,
                    }
                }
            };
            let alvo = if e_nulo(right) {
                alvo_de_promocao(inf, cx, left)
            } else if e_nulo(left) {
                alvo_de_promocao(inf, cx, right)
            } else {
                None
            };
            let _ = (tl, tr);
            // O alvo (um campo) pode ter sido criado agora: o fluxo corrente o tem.
            let depois = if alvo.is_some() { cx.fluxo.clone() } else { depois };
            let (mut igual, mut diferente) = (depois.clone(), depois.clone());
            if let Some(id) = alvo {
                let decl = cx.local(id).tipo;
                inf.promover_nao_nulo(&mut diferente, id, decl);
                let _ = &mut igual;
            }
            // Fluxo sólido (3.9, `sound-flow-analysis`): comparar com `null`
            // uma expressão de tipo não anulável tem resultado conhecido — o
            // ramo "igual" é inalcançável (`flow-analysis.md`, `equalityOp`:
            // `equivalentToNull(T1)` e `T2` não anulável, ou o inverso). Antes
            // da 3.9 os dois ramos continuam alcançáveis (mixed mode).
            let solido = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::SoundFlowAnalysis);
            if solido {
                let nulo = |inf: &BodyInferrer<'_>, t: TypeId| matches!(inf.table.get(t), Type::Null);
                if (nulo(inf, tr) && inf.e_nao_anulavel(tl)) || (nulo(inf, tl) && inf.e_nao_anulavel(tr)) {
                    igual = igual.inalcancavel();
                }
            }
            if op == BinaryOp::Eq {
                (igual, diferente)
            } else {
                (diferente, igual)
            }
        }
    }
}

/// `e is T` / `e is! T`: `(true, false)` com promoção.
/// `_checkAllTypeChecks` do `BestPracticesVerifier`
/// (`an611:src/error/best_practices_verifier.dart:773-823`): `e is dynamic`,
/// `null is Null`, e `e is T` com o tipo estático de `e` subtipo de `T` são
/// sempre verdadeiros (`UNNECESSARY_TYPE_CHECK_TRUE`, ou `_FALSE` com `is!`);
/// `e is Null` com `e` não literal é `TYPE_CHECK_IS_NULL` (`_IS_NOT_NULL`).
#[allow(clippy::too_many_arguments)]
fn teste_de_tipo_desnecessario(
    inf: &mut BodyInferrer<'_>,
    cx: &Corpo,
    value: ExprId,
    v: TypeId,
    ty: ast::TypeId,
    t: TypeId,
    negado: bool,
    invalido: bool,
    span: dartforge_diagnostics::Span,
) {
    use dartforge_diagnostics::codigos::warning as w;
    // Operando ou tipo `InvalidType`: nenhum aviso de teste desnecessário
    // (`best_practices_verifier.dart:790`, `:801`).
    if invalido || inf.table.e_invalido(v) || inf.table.e_invalido(t) {
        return;
    }
    let nome_escrito = match &ast(inf, cx).ty(ty).kind {
        ast::TypeKind::Named { name, .. } if name.len() == 1 && !ast(inf, cx).ty(ty).nullable => {
            Some(inf.interner.resolve(name[0].sym).to_string())
        }
        _ => None,
    };
    let codigo = if negado { w::UNNECESSARY_TYPE_CHECK_FALSE } else { w::UNNECESSARY_TYPE_CHECK_TRUE };
    if inf.e_dynamic(t) {
        // O `dynamic` de verdade: o escrito, ou o alias que chega a si mesmo
        // (`hasSelfReference` instancia como `DynamicType`).
        let alias_auto_referente = match &ast(inf, cx).ty(ty).kind {
            ast::TypeKind::Named { name, .. } => {
                let b = match &name[..] {
                    [n] => inf.program.lookup_na_unidade(cx.unit, n.sym),
                    [p, n] => inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, n.sym),
                    _ => None,
                };
                matches!(b.and_then(|b| b.getter), Some(Element::Typedef(tid))
                    if crate::auto_referencia::typedef_auto_referente(inf.program, inf.program.typedef(tid).decl))
            }
            _ => false,
        };
        if nome_escrito.as_deref() == Some("dynamic") || alias_auto_referente {
            inf.aviso_com_codigo(codigo, span, &[]);
        }
        return;
    }
    if t == inf.core.null && nome_escrito.as_deref() == Some("Null") {
        if matches!(ast(inf, cx).expr(value).kind, ExprKind::Null) {
            inf.aviso_com_codigo(codigo, span, &[]);
        } else {
            let c = if negado { w::TYPE_CHECK_IS_NOT_NULL } else { w::TYPE_CHECK_IS_NULL };
            inf.aviso_com_codigo(c, span, &[]);
        }
        return;
    }
    if inf.sub(v, t) {
        inf.aviso_com_codigo(codigo, span, &[]);
    }
}

fn teste_de_tipo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, value: ExprId, ty: ast::TypeId, negado: bool, span: dartforge_diagnostics::Span) -> (Fluxo, Fluxo) {
    let v = inferir_livre(inf, cx, value);
    uso_de_void(inf, cx, value, v);
    let avisos_antes = inf.diagnostics.len();
    let t = inf.tipo_no_contexto(cx, ty, crate::resolve::ContextoDeTipo::Is);
    // Tipo que não resolveu (`InvalidType` no analyzer): nada a dizer.
    let invalido = inf.diagnostics[avisos_antes..].iter().any(|d| d.code.is_some());
    teste_de_tipo_desnecessario(inf, cx, value, v, ty, t, negado, invalido, span);
    // `x is Never` não promove `x` (o ramo "sim" fica com o tipo de antes).
    let alvo = alvo_de_promocao(inf, cx, value).filter(|_| !matches!(inf.table.get(t), Type::Never));
    let depois = cx.fluxo.clone();
    let (mut sim, mut nao) = (depois.clone(), depois.clone());
    if let Some(id) = alvo {
        let decl = cx.local(id).tipo;
        inf.promover(&mut sim, id, decl, t);
        let fatorado = fator(inf, v, t);
        // `tryPromoteForTypeCheck` (flow_analysis.dart:2544-2557): o fator
        // `Never` não promove (marcaria o ramo como inalcançável, e a
        // unsoundness de modo misto pode alcançá-lo), nem o fator igual ao
        // tipo anterior; o tipo testado é registrado de todo jeito.
        let promove = !matches!(inf.table.get(fatorado), Type::Never) && fatorado != v;
        inf.promover_fatorado(&mut nao, id, promove.then_some(fatorado), t);
    }
    // `e is Never` nunca é verdadeiro: o ramo "sim" é inalcançável (sem
    // promover o alvo).
    if matches!(inf.table.get(t), Type::Never) {
        sim = sim.inalcancavel();
    }
    if negado {
        (nao, sim)
    } else {
        (sim, nao)
    }
}

/// `factor(T, S)` (`flow-analysis.md`).
pub(crate) fn fator(inf: &mut BodyInferrer<'_>, t: TypeId, s: TypeId) -> TypeId {
    if inf.sub(t, s) {
        return inf.core.never;
    }
    let n = inf.core.null;
    match inf.table.get(t).clone() {
        ty if ty.is_declared_nullable() && !matches!(ty, Type::FutureOr { .. }) => {
            let r = inf.nao_nulo(t);
            let f = fator(inf, r, s);
            if inf.sub(n, s) {
                f
            } else {
                inf.anulavel(f)
            }
        }
        Type::FutureOr { arg, nullable: false } => {
            let fut = inf.futuro(arg);
            if inf.sub(fut, s) {
                fator(inf, arg, s)
            } else if inf.sub(arg, s) {
                fator(inf, fut, s)
            } else {
                t
            }
        }
        _ => t,
    }
}

/// Declara um local (com tabela lateral de tipo por offset).
pub(crate) fn declarar_local(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, local: Local, inicializado: bool) -> LocalId {
    let (offset, tipo) = (local.offset, local.tipo);
    let id = cx.declarar(local);
    if inicializado {
        cx.fluxo.inicializar(id);
    }
    inf.body_types.units[cx.unit.0 as usize].set_tipo_local(offset, tipo);
    id
}

impl<'a> BodyInferrer<'a> {
    /// Expressão constante (especificação, "Constants"), sobre a resolução
    /// já feita: literais, constantes referidas, construtores e coleções
    /// `const`, operadores sobre constantes, `?:`, interpolação, tipos.
    /// `alvo` (de uma chamada) é o construtor primário de um tipo de extensão
    /// declarado `const` (`extension type const E._(int v)`: `E._(1)`), que
    /// não é elemento e por isso não tem `Resolved::Constructor`.
    fn primario_const(&self, cx: &Corpo, alvo: ExprId) -> bool {
        let a = ast(self, cx);
        let bt = &self.body_types.units[cx.unit.0 as usize];
        let (classe, nome) = match &a.expr(alvo).kind {
            ExprKind::Identifier(_) => (alvo, self.sym.vazio),
            ExprKind::Property { target, name, .. } => (*target, Some(name.sym)),
            _ => return false,
        };
        let Some(Resolved::Element(Element::Class(c))) = bt.get_resolved(classe) else { return false };
        let Some(nome) = nome else { return false };
        if self.construtor_ou_primario(*c, nome) != Some(None) {
            return false;
        }
        let Some(d) = self.program.class(*c).decl else { return false };
        matches!(&self.program.unit(d.unit).ast.decl(d.decl).kind, ast::DeclKind::ExtensionType(et) if et.const_)
    }

    pub(crate) fn e_constante(&self, cx: &Corpo, e: ExprId) -> bool {
        let a = &self.program.unit(cx.unit).ast;
        let bt = &self.body_types.units[cx.unit.0 as usize];
        let var_const = |v: dartforge_elements::model::VariableId| self.program.variable(v).const_;
        let fun_const = |f: dartforge_elements::model::FunctionElementId| {
            let fe = self.program.function(f);
            match (fe.kind, fe.variable) {
                (FunctionKind::ImplicitAccessor, Some(v)) => var_const(v),
                (FunctionKind::Getter | FunctionKind::Setter, _) => false,
                // Tear-off de função de topo ou estática é constante.
                _ => fe.static_ || fe.class.is_none(),
            }
        };
        let ref_const = |r: Option<&Resolved>| match r {
            Some(Resolved::Local(id)) => cx.locais.get(id.0 as usize).is_some_and(|l| l.const_),
            Some(Resolved::Element(Element::Variable(v))) => var_const(*v),
            Some(Resolved::Element(Element::Function(f))) => fun_const(*f),
            Some(Resolved::Element(Element::Class(_) | Element::Typedef(_))) => true,
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => var_const(*v),
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => fun_const(*f),
            Some(Resolved::Constructor(_)) => true,
            _ => false,
        };
        match &a.expr(e).kind {
            ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) | ExprKind::Null | ExprKind::Symbol(_) => true,
            ExprKind::String(lit) => lit.parts.iter().all(|p| match p {
                ast::StringPart::Interpolation(x) => self.e_constante(cx, *x),
                _ => true,
            }),
            ExprKind::Parenthesized(x) => self.e_constante(cx, *x),
            ExprKind::Identifier(_) => ref_const(bt.get_resolved(e)),
            ExprKind::Property { target, name, .. } => {
                if ref_const(bt.get_resolved(e)) {
                    return true;
                }
                // `s.length` de string constante.
                self.interner.resolve(name.sym) == "length" && self.e_constante(cx, *target)
            }
            ExprKind::TypeArguments { .. } => true,
            ExprKind::List { const_, elements, .. } | ExprKind::SetOrMap { const_, elements, .. } => {
                *const_ || elements.iter().all(|el| match el {
                    ast::CollectionElement::Expression(x) => self.e_constante(cx, *x),
                    ast::CollectionElement::MapEntry { key, value, .. } => self.e_constante(cx, *key) && self.e_constante(cx, *value),
                    _ => false,
                })
            }
            ExprKind::Record { positional, named, .. } => {
                positional.iter().all(|x| self.e_constante(cx, *x)) && named.iter().all(|(_, x)| self.e_constante(cx, *x))
            }
            ExprKind::InstanceCreation { keyword, arguments, .. } => {
                matches!(keyword, Some(ast::CreationKeyword::Const)) || arguments.args.iter().all(|x| self.e_constante(cx, x.value)) && self.construtor_const(bt.get_resolved(e))
            }
            ExprKind::Call { target, arguments } => {
                // Construtor `const` sem `new` em contexto constante, ou `identical`.
                let ok_args = arguments.args.iter().all(|x| self.e_constante(cx, x.value));
                if !ok_args {
                    return false;
                }
                if self.construtor_const(bt.get_resolved(e)) || self.primario_const(cx, *target) {
                    return true;
                }
                matches!(&a.expr(*target).kind, ExprKind::Identifier(n) if self.interner.resolve(n.sym) == "identical")
            }
            ExprKind::Unary { op, operand } => {
                matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot) && self.e_constante(cx, *operand)
            }
            ExprKind::Binary { left, right, .. } => self.e_constante(cx, *left) && self.e_constante(cx, *right),
            ExprKind::Conditional { condition, then, else_ } => {
                self.e_constante(cx, *condition) && self.e_constante(cx, *then) && self.e_constante(cx, *else_)
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.e_constante(cx, *value),
            _ => false,
        }
    }

    fn construtor_const(&self, r: Option<&Resolved>) -> bool {
        match r {
            Some(Resolved::Constructor(f)) => self.program.function(self.program.publico(*f)).const_,
            _ => false,
        }
    }
}
