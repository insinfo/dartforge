//! Inferência de expressões: contexto (esquema) para baixo, tipo estático
//! para cima (`inference.md`, "Expression inference").
//!
//! Cadeias com `?.` fazem *null-shorting*: os nós internos da cadeia recebem
//! o tipo não anulável e só o nó que termina a cadeia recebe o `?`
//! ([`inferir_no`] devolve se a cadeia está em curto).

use super::shorthands;
use super::calls;
use super::collections;
use super::body::{Base, Body, Local, Nome};
use super::flow::Fluxo;
use super::functions;
use super::members::{Busca, Membro};
use super::patterns;
use super::BodyInferrer;
use crate::codes::*;
use crate::resolved::{LocalId, MemberRef, Resolved};
use crate::table::{Type, TypeId};
use dartforge_elements::model::{ClassId, Element, ExtensionId, FunctionElementId, FunctionKind};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, ExprId, ExprKind, UnaryOp};
use dartforge_intern::SymbolId;

/// Infere `e` no contexto `ctx` (o desconhecido `_` = sem contexto).
pub(crate) fn inferir(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId) -> TypeId {
    // `TypeAnalyzer.analyzeExpression` (`type_analyzer.dart:557-561`): o
    // esquema `dynamic` vira `_` (o `InvalidType` não).
    let ctx = if matches!(inf.table.get(ctx), Type::Dynamic) && !inf.table.e_invalido(ctx) { inf.core.unknown } else { ctx };
    let mark = cx.cadeias.len();
    let (t, short_circuit) = infer_at(inf, cx, e, ctx, false);
    close_chain(inf, cx, mark);
    let t = if short_circuit {
        let t = inf.anulavel(t);
        registrar(inf, cx, e, t);
        t
    } else {
        t
    };
    record_implicit_call(inf, cx, e, t, ctx);
    t
}

/// `_insertImplicitCallReference` (`resolver.dart:4085-4142`) nas espécies
/// de expressão cujo `visit…` o chama: com o `getImplicitCallMethod`
/// (`error_detection_helpers.dart:312-334`), a expressão vira `.call`.
fn record_implicit_call(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId, ctx: TypeId) {
    let inserting_kind = matches!(
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
    if !inserting_kind || inf.the_is_unknown(ctx) {
        return;
    }
    // `acceptsFunctionType(context)`: tipo de função, `Function`, ou
    // `FutureOr` de um deles.
    let mut context = ctx;
    for _ in 0..8 {
        match inf.table.get(context).clone() {
            Type::FutureOr { arg, .. } => context = arg,
            _ => break,
        }
    }
    let accepts = matches!(inf.table.get(context), Type::Function { .. }) || context == inf.core.function;
    if !accepts {
        return;
    }
    // Parâmetro de tipo não anulável: o limite.
    let mut typ = t;
    let mut seen = std::collections::HashSet::new();
    loop {
        match inf.table.get(typ).clone() {
            Type::TypeParameter { param, nullable } => {
                if nullable || !seen.insert(param) {
                    return;
                }
                typ = inf.table.param(param).bound;
            }
            Type::Intersection { bound, .. } => typ = bound,
            _ => break,
        }
    }
    if !matches!(inf.table.get(typ), Type::Interface { nullable: false, .. }) || typ == inf.core.function {
        return;
    }
    let Some(call) = inf.sym.call else { return };
    if let Some(m) = inf.interface_member(typ, call, false).filter(|m| m.metodo) {
        inf.body_types.units[cx.unit.0 as usize].chamadas_implicitas.insert(e);
        // O `call` genérico num contexto de função: a instanciação pelo
        // contexto, com os `COULD_NOT_INFER` na expressão.
        let _ = instantiate_in_context_with(inf, cx, e, m.tipo, context, false);
    }
}

/// Fim de uma cadeia com `?.`: a promoção do receptor valeu só dentro dela;
/// depois, o fluxo é a junção do "era nulo" (o de antes do primeiro `?.`) com
/// o de ter percorrido a cadeia.
fn close_chain(inf: &mut BodyInferrer<'_>, cx: &mut Body, mark: usize) {
    if cx.cadeias.len() > mark {
        let before = cx.cadeias[mark].clone();
        cx.cadeias.truncate(mark);
        let after = cx.flow.clone();
        cx.flow = inf.juntar(&before, &after);
    }
}

/// Infere sem contexto.
pub(crate) fn infer_free(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId) -> TypeId {
    let u = inf.core.unknown;
    inferir(inf, cx, e, u)
}

pub(crate) fn registrar(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId) {
    inf.body_types.units[cx.unit.0 as usize].set_type(e, t);
}

pub(crate) fn resolver(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, r: Resolved) {
    let table = &mut inf.body_types.units[cx.unit.0 as usize];
    if inf.registrar_locais
        && let Resolved::Local(id) = &r
    {
        table.declaracoes_de_locais.insert(e, cx.local(*id).offset);
    }
    table.set_resolved(e, r);
}

/// Captura o escopo léxico quando `nome` é o identificador sondado pelo LSP.
pub(crate) fn probe_scope_at(inf: &mut BodyInferrer<'_>, cx: &Body, the_name: ast::Name) {
    if inf.sonda_escopo == Some((cx.unit, the_name.span.start)) && inf.probed_scope.is_none() {
        inf.probed_scope = Some(cx.visible_scope());
    }
}

fn ast<'p>(inf: &BodyInferrer<'p>, cx: &Body) -> &'p ast::Ast {
    &inf.program.unit(cx.unit).ast
}

/// O que um nome simples denota no ponto atual.
#[derive(Debug, Clone, Copy)]
pub(crate) enum NameRef {
    Local(LocalId),
    TypeParam(crate::table::TypeParamId),
    /// Pseudotipo embutido (`dynamic`, `Never`), sem elemento no namespace
    /// da biblioteca: como valor, é um literal de tipo (`Type`).
    BuiltinType,
    Elemento(Element),
    /// Membro declarado no corpo da classe/extensão envolvente.
    LexicalMember(dartforge_elements::model::FunctionElementId, bool),
    ConstantEnum(dartforge_elements::model::VariableId),
    Prefixo,
    /// Não achado lexicamente, mas há `this`: `this.nome`.
    ImplicitThis,
    /// Local do bloco declarado depois deste uso (o nome na declaração).
    Forward(dartforge_diagnostics::Span),
    Nenhum,
}

/// Busca léxica de um nome (`setter` = contexto de escrita).
pub(crate) fn resolve_name(inf: &mut BodyInferrer<'_>, cx: &Body, the_name: SymbolId, setter: bool) -> NameRef {
    match cx.buscar(the_name) {
        Some(Nome::Local(id)) => return NameRef::Local(id),
        Some(Nome::TypeParam(p)) => return NameRef::TypeParam(p),
        Some(Nome::Forward(s)) => return NameRef::Forward(s),
        None => {}
    }
    // Membros declarados no corpo da classe/extensão.
    if cx.classe.is_some() || cx.extensao.is_some() {
        // O escopo do contêiner guarda getter e setter pelo nome base: a
        // leitura que só acha o setter para ali (`LexicalLookup.resolveGetter`).
        let setter_key = inf.setter_key(the_name);
        let order: Vec<Option<SymbolId>> = if setter { vec![setter_key, Some(the_name)] } else { vec![Some(the_name), setter_key] };
        for the_key in order.into_iter().flatten() {
            if let Some((f, static_)) = inf.lexical_declared_member(cx.classe, cx.extensao, the_key, None) {
                return NameRef::LexicalMember(f, static_);
            }
        }
        if let Some(c) = cx.classe {
            if let Some(&v) = inf.program.class(c).enum_constants.iter().find(|&&v| inf.program.variable(v).name == the_name) {
                return NameRef::ConstantEnum(v);
            }
        }
        if let Some(x) = cx.extensao {
            if let Some(&v) = inf.program.extension(x).fields.iter().find(|&&v| inf.program.variable(v).name == the_name) {
                return NameRef::Elemento(Element::Variable(v));
            }
        }
    }
    if let Some(b) = inf.program.lookup_na_unidade(cx.unit, the_name) {
        let el = if setter { b.setter.or(b.getter) } else { b.getter.or(b.setter) };
        if let Some(el) = el {
            if let Element::Prefix(..) = el {
                return NameRef::Prefixo;
            }
            return NameRef::Elemento(el);
        }
    }
    if inf.program.prefixos_na_unidade(cx.unit).contains_key(&the_name) {
        return NameRef::Prefixo;
    }
    if matches!(inf.interner.resolve(the_name), "dynamic" | "Never") {
        return NameRef::BuiltinType;
    }
    if cx.tipo_this.is_some() && !cx.estatico {
        return NameRef::ImplicitThis;
    }
    NameRef::Nenhum
}

/// O `thisType` do `ResolverVisitor` (`_setupThisType`,
/// `an611:src/generated/resolver.dart:4180-4193`): o tipo `this` da classe
/// ou o tipo estendido da extensão envolvente, em **qualquer** membro —
/// estático, fábrica, inicializador de campo ou de construtor. A busca por
/// `this` implícito de um nome que não resolve lexicamente usa esse tipo
/// mesmo onde `this` não vale (o erro passa a ser o de acesso a membro de
/// instância, não o de nome indefinido).
fn analyzer_this_type(inf: &mut BodyInferrer<'_>, cx: &Body) -> Option<TypeId> {
    if cx.tipo_this.is_some() {
        return cx.tipo_this;
    }
    if let Some(c) = cx.classe {
        return Some(inf.class_this_type(c));
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
    InstanceSetter,
    /// Só um setter que não é de instância (de topo, estático, importado):
    /// recuperação — a leitura não tem elemento, e a busca para aqui.
    StandaloneSetter,
    /// Nada no escopo léxico.
    Nada,
}

fn lexical_read_lookup(inf: &mut BodyInferrer<'_>, cx: &Body, the_name: SymbolId) -> Lexico {
    match resolve_name(inf, cx, the_name, false) {
        NameRef::Nenhum | NameRef::ImplicitThis => Lexico::Nada,
        NameRef::LexicalMember(f, static_) => {
            if inf.program.function(f).kind == FunctionKind::Setter {
                // `noGetterIsPossible` (3.6.2 `method_invocation_resolver.dart:616-628`):
                // o setter estático ou declarado numa extensão (também o de
                // instância) é a propriedade acessada; não se procura getter.
                if static_ || inf.program.function(f).extension.is_some() {
                    Lexico::StandaloneSetter
                } else {
                    Lexico::InstanceSetter
                }
            } else {
                Lexico::Getter
            }
        }
        NameRef::Elemento(Element::Function(f)) if inf.program.function(f).kind == FunctionKind::Setter => {
            // Setter de topo (declarado ou importado) sem getter de mesmo nome.
            match inf.program.lookup_na_unidade(cx.unit, the_name) {
                Some(b) if b.getter.is_none() => Lexico::StandaloneSetter,
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
enum ThroughThis {
    /// Membro de instância (da interface ou de extensão aplicável).
    Instance,
    /// Membro estático da classe ou de uma superclasse/mixin (recuperação):
    /// `(classe que o declara, é método)`.
    Estatico(ClassId, bool),
    Missing,
    /// Receptor anulável ou que não é de interface: o analyzer segue por
    /// outros caminhos (erros de nulo); não relatamos nada.
    Uncertain,
}

fn lookup_through_this(inf: &mut BodyInferrer<'_>, cx: &Body, this: TypeId, the_name: SymbolId, setter: bool) -> ThroughThis {
    let cls = match inf.table.get(this) {
        Type::Interface { class, nullable: false, .. } => *class,
        Type::ExtensionType { decl, nullable: false, .. } => *decl,
        _ => return ThroughThis::Uncertain,
    };
    match inf.lookup_member(cx.lib, this, the_name, setter) {
        Busca::Achado(_) => ThroughThis::Instance,
        Busca::Dinamico | Busca::NeverType => ThroughThis::Uncertain,
        Busca::Missing => match inf.static_recovery(cx.lib, cls, the_name, setter) {
            Some((owner, method)) => ThroughThis::Estatico(owner, method),
            None => ThroughThis::Missing,
        },
    }
}

/// O erro de acessar um membro de instância sem `this`
/// (`_checkForInvalidInstanceMemberAccess`,
/// `an611:src/generated/error_verifier.dart:3976-4038`): método estático
/// (e o que ele aninha), construtor de fábrica, inicializador de construtor
/// (os de campo ficam de fora: veja o braço `Raiz::Nada`). `None` onde
/// `this` vale.
fn instance_without_this_error(inf: &BodyInferrer<'_>, cx: &Body) -> Option<dartforge_diagnostics::Codigo> {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    if cx.tipo_this.is_some() && !cx.estatico {
        return None;
    }
    if cx.classe.is_none() && cx.extensao.is_none() {
        return None;
    }
    match cx.raiz {
        super::body::Raiz::Funcao(_) if cx.static_member => Some(c::INSTANCE_MEMBER_ACCESS_FROM_STATIC),
        super::body::Raiz::Funcao(_) => None,
        super::body::Raiz::Construtor(m) => match &inf.program.unit(cx.unit).ast.member(m).kind {
            ast::MemberKind::Constructor(k) if k.factory => Some(c::INSTANCE_MEMBER_ACCESS_FROM_FACTORY),
            ast::MemberKind::Constructor(_) => Some(c::IMPLICIT_THIS_REFERENCE_IN_INITIALIZER),
            _ => None,
        },
        // Inicializador de campo de instância não `late` ou de variável
        // estática (3.6.2 `error_verifier.dart:3976-4038`); os argumentos de
        // constante de enum e as anotações usam o mesmo corpo sem raiz e
        // ficam de fora.
        super::body::Raiz::Nada if cx.variable_initializer => Some(c::IMPLICIT_THIS_REFERENCE_IN_INITIALIZER),
        super::body::Raiz::Nada => None,
    }
}

/// Relata o acesso a membro de instância onde `this` não vale (ver
/// [`erro_de_instancia_sem_this`]).
fn warn_instance_without_this(inf: &mut BodyInferrer<'_>, cx: &Body, n: ast::Name) {
    if let Some(code) = instance_without_this_error(inf, cx) {
        let the_name = inf.interner.resolve(n.sym).to_string();
        if code.info().nome == "implicit_this_reference_in_initializer" {
            inf.warning_with_code(code, n.span, &[&the_name]);
        } else {
            inf.warning_with_code(code, n.span, &[]);
        }
    }
}

/// Referência sem qualificação a um membro estático herdado
/// (`_checkForUnqualifiedReferenceToNonLocalStaticMember`,
/// `an611:src/generated/error_verifier.dart:5660-5700`, e
/// `_reportInstanceAccessToStaticMember`,
/// `an611:src/dart/resolver/method_invocation_resolver.dart:208-227`).
fn warn_unqualified_static(inf: &mut BodyInferrer<'_>, cx: &Body, owner: ClassId, n: ast::Name) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let code = if cx.extensao.is_some() {
        c::UNQUALIFIED_REFERENCE_TO_STATIC_MEMBER_OF_EXTENDED_TYPE
    } else {
        c::UNQUALIFIED_REFERENCE_TO_NON_LOCAL_STATIC_MEMBER
    };
    let the_name = inf.interner.resolve(inf.program.class(owner).name).to_string();
    inf.warning_with_code(code, n.span, &[&the_name]);
}

/// `p.loadLibrary` de um prefixo de import `deferred` (`PrefixScope.lookup`,
/// `an611:src/dart/element/scope.dart:576-583`): a função sintética
/// `Future<dynamic> Function()` da biblioteca adiada.
pub(crate) fn load_library(inf: &mut BodyInferrer<'_>, cx: &Body, prefix: SymbolId, the_name: SymbolId) -> Option<TypeId> {
    if inf.interner.resolve(the_name) != "loadLibrary" {
        return None;
    }
    let lib = inf.program.library(cx.lib);
    let deferred = lib
        .imports
        .iter()
        .any(|im| im.prefix == Some(prefix) && im.deferred && !inf.program.library(im.library).units.is_empty());
    if !deferred {
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
pub(crate) fn warn_undefined_prefixed_name(inf: &mut BodyInferrer<'_>, cx: &Body, prefix: SymbolId, name: ast::Name) {
    if crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, Some(prefix), name.sym) {
        return;
    }
    let the_name = inf.interner.resolve(name.sym).to_string();
    let p = inf.interner.resolve(prefix).to_string();
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_PREFIXED_NAME, name.span, &[&the_name, &p]);
}

/// A leitura de um nome simples cuja busca léxica não achou getter
/// (`SimpleIdentifierResolver._resolve1`,
/// `an611:src/dart/resolver/simple_identifier_resolver.dart:209-224`, com a
/// busca de `PropertyElementResolver.resolveSimpleIdentifier`,
/// `an611:src/dart/resolver/property_element_resolver.dart:243-272`): o
/// `this` implícito ainda pode achar o membro (o erro é então o de acesso
/// sem `this` ou o de estático não qualificado); senão `await` num corpo de
/// função, o nome ignorado de um import que não existe, ou nome indefinido.
fn undefined_read_name(inf: &mut BodyInferrer<'_>, cx: &Body, n: ast::Name, lexical: Lexico) {
    if lexical != Lexico::StandaloneSetter
        && let Some(this) = analyzer_this_type(inf, cx)
    {
        match lookup_through_this(inf, cx, this, n.sym, false) {
            ThroughThis::Instance => {
                warn_instance_without_this(inf, cx, n);
                return;
            }
            ThroughThis::Estatico(owner, method) => {
                // Tear-off de método estático: o `ErrorVerifier` não relata.
                if !method {
                    warn_unqualified_static(inf, cx, owner, n);
                }
                return;
            }
            ThroughThis::Uncertain => return,
            ThroughThis::Missing => {}
        }
    }
    undefined_name_without_this(inf, cx, n);
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
pub(crate) fn write_in_primary(inf: &mut BodyInferrer<'_>, cx: &Body, id: LocalId, span: dartforge_diagnostics::Span) {
    if cx.primary_parameters.contains(&id) {
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::ASSIGNMENT_TO_PRIMARY_CONSTRUCTOR_PARAMETER, span, &[]);
    }
}

pub(crate) fn write_in_guard(inf: &mut BodyInferrer<'_>, cx: &Body, id: LocalId, span: dartforge_diagnostics::Span) {
    if cx.variables_in_guard.contains(&id) {
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD, span, &[]);
    }
}

/// A referência (leitura ou escrita) a uma variável de junção inconsistente
/// de casos que dividem o corpo (`ResolverVisitor.finishJoinedPatternVariable`,
/// `an611:src/generated/resolver.dart:878-915`), com o nome.
pub(crate) fn join_reference(inf: &mut BodyInferrer<'_>, cx: &Body, id: LocalId, n: ast::Name) {
    if let Some(&code) = cx.inconsistent_joins.get(&id) {
        let the_name = inf.interner.resolve(n.sym).to_string();
        inf.warning_with_code(code, n.span, &[&the_name]);
    }
}

/// O fim de [`nome_lido_indefinido`]: `await` num corpo de função, nome
/// ignorado ou `UNDEFINED_IDENTIFIER`.
pub(crate) fn undefined_name_without_this(inf: &mut BodyInferrer<'_>, cx: &Body, n: ast::Name) {
    let text = inf.interner.resolve(n.sym).to_string();
    let in_function = !matches!(cx.raiz, super::body::Raiz::Nada) || !cx.funcoes.is_empty();
    if text == "await" && in_function {
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_IDENTIFIER_AWAIT, n.span, &[]);
        return;
    }
    if crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, None, n.sym) {
        return;
    }
    // O prefixo de um import cujo alvo não existe (`import 'dart:foo' as foo;`):
    // o `PrefixElement` existe mesmo assim, e `foo.bar` é um nome prefixado
    // que não resolve, sem relato aqui.
    let lib = inf.program.unit(cx.unit).library;
    let is_prefix = inf.program.library(lib).units.iter().any(|&u| {
        inf.program.unit(u).unit.directives.iter().any(|d| matches!(&d.kind, ast::DirectiveKind::Import { prefix: Some(p), .. } if p.sym == n.sym))
    });
    if is_prefix {
        return;
    }
    let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, text);
    inf.aviso(msg, n.span);
}

/// A invocação `nome(args)` sem alvo (`MethodInvocationResolver._resolveReceiverNull`,
/// `an611:src/dart/resolver/method_invocation_resolver.dart:559-657`) quando
/// a busca léxica não acha getter: sem `this`, `UNDEFINED_FUNCTION`; com
/// `this`, a busca nele (membro de instância achado segue o caminho comum;
/// estático herdado, estático não qualificado; nada, `UNDEFINED_METHOD` com
/// o nome da classe). Devolve se tratou a invocação (e relatou o que havia a
/// relatar): o chamador infere os argumentos sem contexto e dá `dynamic`.
pub(crate) fn undefined_targetless_invocation(inf: &mut BodyInferrer<'_>, cx: &Body, n: ast::Name) -> bool {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    if cx.curinga == Some(n.sym) {
        return false;
    }
    let lexical = lexical_read_lookup(inf, cx, n.sym);
    if lexical == Lexico::Getter {
        return false;
    }
    let Some(this) = analyzer_this_type(inf, cx) else {
        if !crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, None, n.sym) {
            let the_name = inf.interner.resolve(n.sym).to_string();
            inf.warning_with_code(c::UNDEFINED_FUNCTION, n.span, &[&the_name]);
        }
        return true;
    };
    let type_name = |inf: &BodyInferrer<'_>| -> String {
        match inf.table.get(this) {
            Type::Interface { class, .. } => inf.interner.resolve(inf.program.class(*class).name).to_string(),
            Type::ExtensionType { decl, .. } => inf.interner.resolve(inf.program.class(*decl).name).to_string(),
            Type::Function { .. } => "Function".to_string(),
            _ => "<unknown>".to_string(),
        }
    };
    if lexical == Lexico::StandaloneSetter {
        // Setter de topo, de extensão ou estático: não há getter possível.
        let the_name = inf.interner.resolve(n.sym).to_string();
        let typ = type_name(inf);
        inf.warning_with_code(c::UNDEFINED_METHOD, n.span, &[&the_name, &typ]);
        return true;
    }
    // `this` potencialmente anulável onde vale (extensão sobre tipo
    // anulável): o caminho comum resolve pelo `Object`/extensões ou relata
    // o uso sem checagem (`ThisLookup` → `TypePropertyResolver`).
    // (O tipo de extensão é anulável só pelo `?`, `type_property_resolver.dart:91-96`.)
    if let Some(t) = cx.tipo_this
        && !cx.estatico
        && !inf.e_dynamic(t)
        && !inf.is_non_nullable(t)
        && !matches!(inf.table.get(t), Type::ExtensionType { nullable: false, .. })
    {
        return false;
    }
    inf.extension_ambiguity = None;
    match lookup_through_this(inf, cx, this, n.sym, false) {
        // Extensões ambíguas (`_resolveReceiverNull` → `TypePropertyResolver`):
        // só a ambiguidade, no nome.
        ThroughThis::Uncertain if inf.extension_ambiguity.is_some() => {
            inf.report_extension_ambiguity(n.span);
            true
        }
        // Membro de instância: onde `this` vale, o caminho comum o resolve.
        ThroughThis::Instance if cx.tipo_this.is_some() && !cx.estatico => false,
        ThroughThis::Instance => {
            warn_instance_without_this(inf, cx, n);
            true
        }
        ThroughThis::Estatico(owner, _) => {
            warn_unqualified_static(inf, cx, owner, n);
            true
        }
        ThroughThis::Uncertain => true,
        ThroughThis::Missing => {
            let the_name = inf.interner.resolve(n.sym).to_string();
            let typ = type_name(inf);
            inf.warning_with_code(c::UNDEFINED_METHOD, n.span, &[&the_name, &typ]);
            true
        }
    }
}

/// Uma referência a tipo usada como receptor (`C.m`, `p.C.m`, `C<T>.m`).
#[derive(Debug, Clone)]
pub(crate) enum TypeRef {
    Classe(ClassId, Option<Vec<ast::TypeId>>),
    /// Alias de tipo que nomeia uma classe (`typedef A = B<int>`).
    /// Typedef para classe: argumentos da classe (`None`: o typedef só
    /// renomeia os parâmetros, e os argumentos são os explícitos ou
    /// inferidos como os da classe).
    Alias(ClassId, Option<Vec<TypeId>>, dartforge_elements::model::TypedefId),
    Extensao(ExtensionId),
}

/// Se `e` nomeia uma classe/extensão/typedef (não um valor), qual.
pub(crate) fn type_reference(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<TypeRef> {
    let a = ast(inf, cx);
    let el = match &a.expr(e).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Elemento(el) => el,
            _ => return None,
        },
        ExprKind::Property { target, name, null_aware: false } => {
            let ExprKind::Identifier(p) = &a.expr(*target).kind else { return None };
            if !matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefixo) {
                return None;
            }
            inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym)?.getter?
        }
        ExprKind::TypeArguments { target, type_args } => {
            let targs = type_args.to_vec();
            return match type_reference(inf, cx, *target)? {
                TypeRef::Classe(c, None) => Some(TypeRef::Classe(c, Some(targs))),
                TypeRef::Alias(_, _, td) => {
                    let ex: Vec<TypeId> = targs.iter().map(|&t| inf.type_argument_type(cx, t)).collect();
                    alias_of(inf, td, Some(ex))
                }
                _ => None,
            };
        }
        _ => return None,
    };
    match el {
        Element::Class(c) => Some(TypeRef::Classe(c, None)),
        Element::Extension(x) => Some(TypeRef::Extensao(x)),
        Element::Typedef(td) => alias_of(inf, td, None),
        _ => None,
    }
}

/// `e` nomeia (sem ou com prefixo) um alias de tipo genérico cujo alvo não
/// é classe nem tipo de extensão.
pub(crate) fn alias_without_class(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let a = ast(inf, cx);
    let el = match &a.expr(e).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Elemento(el) => el,
            _ => return false,
        },
        ExprKind::Property { target, name, null_aware: false } => {
            let ExprKind::Identifier(p) = &a.expr(*target).kind else { return false };
            if !matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefixo) {
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
pub(crate) fn non_class_type_alias(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let ExprKind::Identifier(n) = &ast(inf, cx).expr(e).kind else { return false };
    let NameRef::Elemento(Element::Typedef(td)) = resolve_name(inf, cx, n.sym, false) else { return false };
    let d = &inf.outline.typedefs[td.0 as usize];
    // `Null` e `FutureOr` são `InterfaceType` no analyzer.
    !matches!(inf.table.get(d.target_type), Type::Interface { .. } | Type::ExtensionType { .. } | Type::Null | Type::FutureOr { .. })
}

/// `e` nomeia um alias de tipo de função.
pub(crate) fn function_type_alias(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    named_alias_target(inf, cx, e).is_some_and(|t| matches!(inf.table.get(t), Type::Function { .. }))
}

/// O acesso `alvo.nome` com `alvo` o literal de tipo instanciado de um alias
/// de tipo de função (`Fn<int>.foo`, `Fn<int>.foo = 1`): infere o alvo e
/// relata `UNDEFINED_GETTER_ON_FUNCTION_TYPE` (leitura) ou
/// `UNDEFINED_SETTER_ON_FUNCTION_TYPE` (escrita) no nome, com o nome do
/// tipo como escrito. Diz se relatou.
pub(crate) fn function_type_literal(inf: &mut BodyInferrer<'_>, cx: &mut Body, tgt: ExprId, the_name: ast::Name, read: bool) -> bool {
    let ExprKind::TypeArguments { target: base, .. } = &ast(inf, cx).expr(tgt).kind else { return false };
    let base = *base;
    if !function_type_alias(inf, cx, base) {
        return false;
    }
    infer_free(inf, cx, tgt);
    let sp_base = inf.span_expr(cx.unit, base);
    let qualified = inf.program.unit(cx.unit).source[sp_base.start..sp_base.end].to_string();
    let text = inf.interner.resolve(the_name.sym).to_string();
    let code = if read {
        dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_GETTER_ON_FUNCTION_TYPE
    } else {
        dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_SETTER_ON_FUNCTION_TYPE
    };
    inf.warning_with_code(code, the_name.span, &[&text, &qualified]);
    true
}

/// `e` nomeia um alias cujo alvo é um parâmetro de tipo (`typedef T<X> = X`).
pub(crate) fn type_parameter_alias(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    named_alias_target(inf, cx, e).is_some_and(|t| matches!(inf.table.get(t), Type::TypeParameter { .. }))
}

/// O alvo do alias que `e` (`T` ou `p.T`) nomeia.
fn named_alias_target(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<TypeId> {
    let el = match &ast(inf, cx).expr(e).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Elemento(el) => el,
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
pub(crate) fn alias_of_null_or_futureor(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let el = match &ast(inf, cx).expr(e).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Elemento(el) => el,
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
pub(crate) fn alias_of(inf: &mut BodyInferrer<'_>, td: dartforge_elements::model::TypedefId, explicit: Option<Vec<TypeId>>) -> Option<TypeRef> {
    let tgt = inf.outline.typedefs[td.0 as usize].target_type;
    let params = inf.outline.typedefs[td.0 as usize].type_params.clone();
    let (class, args) = match inf.table.get(tgt).clone() {
        Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => (class, args),
        // `Null` e `FutureOr<T>` são `InterfaceType` no analyzer.
        Type::Null => (inf.core.null_class?, Box::new([]) as Box<[TypeId]>),
        Type::FutureOr { arg, .. } => {
            let lib = inf.core.async_library?;
            let the_name = inf.interner.lookup("FutureOr")?;
            let Some(Element::Class(c)) = inf.program.library(lib).declared.get(&the_name).and_then(|b| b.getter) else { return None };
            (c, Box::new([arg]) as Box<[TypeId]>)
        }
        _ => return None,
    };
    if params.is_empty() {
        return Some(TypeRef::Alias(class, Some(args.to_vec()), td));
    }
    let inst = match explicit {
        Some(ex) if ex.len() == params.len() => ex,
        _ => {
            // Typedef que só renomeia (`typedef M<K, V> = _M<K, V>`): os
            // argumentos se inferem como os da classe.
            let renames = args.len() == params.len()
                && args.iter().zip(params.iter()).all(|(&a, &p)| matches!(inf.table.get(a), Type::TypeParameter { param, nullable: false } if *param == p));
            if renames {
                return Some(TypeRef::Alias(class, None, td));
            }
            inf.instantiate_to_bounds(&params)
        }
    };
    let map = inf.mapa(&params, &inst);
    let args = args.iter().map(|a| inf.subst(*a, &map)).collect();
    Some(TypeRef::Alias(class, Some(args), td))
}

/// Registra os nós de uma referência a tipo (tipo `Type`, resolução do elemento).
fn record_type_ref(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) {
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
            record_type_ref(inf, cx, t);
            registrar(inf, cx, e, tt);
        }
        _ => {}
    }
}

/// Tipo de uma leitura de variável local (com checagem de atribuição definitiva).
fn ler_local(inf: &mut BodyInferrer<'_>, cx: &mut Body, id: LocalId, span: dartforge_diagnostics::Span) -> TypeId {
    let l = cx.local(id).clone();
    // `checkReadOfNotAssignedLocalVariable`
    // (`an611:src/generated/resolver.dart:643-690`). Também no código
    // inalcançável, que herda o estado dos caminhos que levam a ele
    // (`return; use(x);` relata).
    if !l.funcao_local {
        let the_name = inf.interner.resolve(l.nome).to_string();
        if l.late {
            if cx.flow.unassigned(id) {
                inf.warning_with_code(
                    dartforge_diagnostics::codigos::compile_time_error::DEFINITELY_UNASSIGNED_LATE_LOCAL_VARIABLE,
                    span,
                    &[&the_name],
                );
            }
        } else if !cx.flow.assigned(id) {
            if l.final_ {
                inf.warning_with_code(
                    dartforge_diagnostics::codigos::compile_time_error::READ_POTENTIALLY_UNASSIGNED_FINAL,
                    span,
                    &[&the_name],
                );
            } else if !inf.the_is_nullable(l.tipo) && !inf.e_dynamic(l.tipo) {
                // `isPotentiallyNonNullable`: `Null` não é subtipo do tipo
                // (o parâmetro de tipo de limite `Object?` e o tipo de
                // extensão sem `Object` também).
                let msg = format!("{}: '{}'", DEFINITELY_UNASSIGNED_VARIABLE.template, the_name);
                inf.aviso(msg, span);
            }
        }
    }
    cx.flow.current_type(id, l.tipo)
}

/// `EXTENSION_AS_EXPRESSION` (`simple_identifier_resolver.dart:315-323`,
/// `prefixed_identifier_resolver.dart:160-184`): o nome (ou `p.E`) de uma
/// extensão usado como valor — fora de alvo de método ou de acesso a
/// propriedade. Com o relato o nó fica `dynamic`.
fn extension_as_expression(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, sp: dartforge_diagnostics::Span) -> bool {
    if let dartforge_frontend::pais::Pai::Expr(p) = inf.parent_of(cx.unit, e) {
        match &ast(inf, cx).expr(p).kind {
            ExprKind::Property { target, .. } | ExprKind::Call { target, .. } if *target == e => return false,
            _ => {}
        }
    }
    let text = inf.program.unit(cx.unit).source[sp.start..sp.end].to_string();
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::EXTENSION_AS_EXPRESSION, sp, &[&text]);
    true
}

/// O nome com que `_resolve` relata a contagem de argumentos de tipo de um
/// tear-off: o de uma função, método ou operador declarado (também a função
/// local); `None` para variável, getter ou outra expressão.
fn referenced_function_name(inf: &BodyInferrer<'_>, cx: &Body, tgt: ExprId) -> Option<String> {
    use crate::resolved::{MemberRef, Resolved};
    let function = |f: dartforge_elements::model::FunctionElementId| {
        let fe = inf.program.function(f);
        matches!(fe.kind, FunctionKind::Function | FunctionKind::Operator).then(|| inf.interner.resolve(fe.name).to_string())
    };
    match inf.body_types.units[cx.unit.0 as usize].get_resolved(tgt)? {
        Resolved::Element(Element::Function(f)) => function(*f),
        Resolved::Member { member: MemberRef::Function(f), .. } => function(*f),
        Resolved::ExtensionMember { member, .. } => function(*member),
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
        Element::Variable(v) => inf.variable_type(v),
        Element::Function(f) => {
            let fe = inf.program.function(f);
            match (fe.kind, fe.variable) {
                (FunctionKind::ImplicitAccessor, Some(v)) => inf.variable_type(v),
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

pub(crate) fn lexical_member_resolved(inf: &BodyInferrer<'_>, cx: &Body, f: dartforge_elements::model::FunctionElementId, static_: bool) -> Resolved {
    let fe = inf.program.function(f);
    if let Some(x) = fe.extension.or(cx.extensao) {
        if fe.class.is_none() {
            return Resolved::ExtensionMember { extension: x, member: f };
        }
    }
    let class = fe.class.or(cx.classe).unwrap_or(ClassId(0));
    let member = match (fe.kind, fe.variable) {
        (FunctionKind::ImplicitAccessor, Some(v)) if static_ => MemberRef::Variable(v),
        _ => MemberRef::Function(f),
    };
    Resolved::Member { class, member, via_super: false }
}

/// `referencedBeforeDeclaration` (`diagnostic_factory.dart:342`): o contexto
/// é a declaração, no nome dela.
fn warning_before_declaration(inf: &mut BodyInferrer<'_>, n: ast::Name, msg: String, decl: dartforge_diagnostics::Span) {
    let text = format!("The declaration of '{}' is here.", inf.interner.resolve(n.sym));
    inf.diagnostics.push(dartforge_diagnostics::Diagnostic::new(msg, n.span).com_contexto(decl, text));
    inf.warning_units.push(inf.current_unit);
}

/// Identificador como valor.
fn identificador(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, n: ast::Name) -> TypeId {
    probe_scope_at(inf, cx, n);
    if let Some(&(_, decl)) = cx.pattern_hidden.iter().find(|(s, _)| *s == n.sym) {
        let msg = format!("{}: '{}'", REFERENCED_BEFORE_DECLARATION.template, inf.interner.resolve(n.sym));
        warning_before_declaration(inf, n, msg, decl);
        return inf.core.dynamic_;
    }
    match resolve_name(inf, cx, n.sym, false) {
        NameRef::Local(id) => {
            resolver(inf, cx, e, Resolved::Local(id));
            join_reference(inf, cx, id, n);
            if inf.invalid_locals.contains(&(cx.unit, cx.local(id).offset)) {
                inf.body_types.units[cx.unit.0 as usize].invalid_types.insert(e);
            }
            if n.span.start < cx.local(id).offset && !cx.local(id).funcao_local {
                let msg = format!("{}: '{}'", REFERENCED_BEFORE_DECLARATION.template, inf.interner.resolve(n.sym));
                let decl = dartforge_diagnostics::Span { start: cx.local(id).offset, end: cx.local(id).offset + (n.span.end - n.span.start) };
                warning_before_declaration(inf, n, msg, decl);
            }
            let t = ler_local(inf, cx, id, n.span);
            // `whyNotPromoted` desta leitura (o histórico de não promoção da
            // variável neste ponto).
            inf.record_local_non_promotion(cx, e, id);
            t
        }
        NameRef::TypeParam(p) => {
            resolver(inf, cx, e, Resolved::TypeParameter(p));
            // `_checkForTypeParameterReferencedByStatic` do
            // `visitSimpleIdentifier` (3.6.2, `error_verifier.dart:5419-5434`).
            if cx.static_member && crate::resolve::class_param(inf.table, p) {
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::TYPE_PARAMETER_REFERENCED_BY_STATIC, n.span, &[]);
            }
            inf.core.type_
        }
        NameRef::BuiltinType => inf.core.type_,
        NameRef::Elemento(_) if ambiguous_import(inf, cx, None, n) => inf.table.invalid(inf.core.dynamic_),
        NameRef::Elemento(el) => {
            resolver(inf, cx, e, Resolved::Element(el));
            // Só um setter de topo com esse nome: a leitura não tem elemento.
            if lexical_read_lookup(inf, cx, n.sym) == Lexico::StandaloneSetter {
                undefined_read_name(inf, cx, n, Lexico::StandaloneSetter);
                return inf.table.invalid(inf.core.dynamic_);
            }
            if let Element::Extension(_) = el
                && extension_as_expression(inf, cx, e, n.span)
            {
                return inf.core.dynamic_;
            }
            ler_elemento(inf, el)
        }
        NameRef::LexicalMember(f, static_) => {
            let r = lexical_member_resolved(inf, cx, f, static_);
            resolver(inf, cx, e, r);
            let only_setter = inf.program.function(f).kind == FunctionKind::Setter;
            // `isInstanceMember` (3.6.2 `extensions.dart:109-119`) só vale para
            // membro de `InterfaceElement`: o de extensão achado no escopo
            // léxico é o elemento lido (o getter) ou a recuperação (o setter,
            // sem busca pelo `this` implícito: `undefined_identifier`, não a
            // ambiguidade com o getter de outra extensão;
            // `static_extension_internal_basename_shadowing_error_test.dart:276`).
            let of_extension = inf.program.function(f).extension.is_some();
            if !static_ && !of_extension {
                // Membro de instância declarado aqui: pelo tipo `this` (a
                // substituição é a identidade).
                if let Some(this) = cx.tipo_this {
                    if let Busca::Achado(m) = inf.lookup_member(cx.lib, this, n.sym, false) {
                        // Achado só o setter aqui, a leitura é `this.nome`: o
                        // elemento lido é o getter da interface (herdado), não
                        // o setter (os backends invocam o `Resolved` do nó).
                        if only_setter {
                            resolver(inf, cx, e, m.resolved.clone());
                        }
                        return field_read(inf, cx, e, Base::This, m.tipo);
                    }
                }
            }
            if only_setter {
                // Leitura de um nome que só tem setter no contêiner: a de
                // instância ainda busca pelo `this` implícito; a estática não.
                let lexical = if static_ || of_extension { Lexico::StandaloneSetter } else { Lexico::InstanceSetter };
                undefined_read_name(inf, cx, n, lexical);
                return inf.table.invalid(inf.core.dynamic_);
            }
            if !static_ {
                warn_instance_without_this(inf, cx, n);
            }
            inf.declared_member_type(f, false).0
        }
        NameRef::ConstantEnum(v) => {
            let c = inf.program.variable(v).class;
            if let Some(c) = c {
                resolver(inf, cx, e, Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
            inf.variable_type(v)
        }
        NameRef::Prefixo => {
            // `SimpleIdentifierResolver._resolve1` (3.6.2,
            // `simple_identifier_resolver.dart:199-205`): o prefixo fora de
            // `p.x` e de `p.f()` (argumento, `p?.x`, `p[0]`, cascata, `p()`).
            resolver(inf, cx, e, Resolved::Prefix(cx.lib));
            prefix_without_dot(inf, n);
            // `_resolve2` (`simple_identifier_resolver.dart:269-276`): fora de
            // `p.x`/`p.f()` o tipo do prefixo é `InvalidType`.
            inf.table.invalid(inf.core.dynamic_)
        }
        NameRef::ImplicitThis => {
            let this = cx.tipo_this.unwrap();
            // `ThisLookup.lookupGetter` com `this` potencialmente anulável
            // (extensão sobre tipo anulável): o `TypePropertyResolver` relata
            // o uso sem checagem, com o código pelo pai do nome, e recupera
            // pela interface do tipo sem `?`.
            if inf.requires_null_check(cx.lib, this, n.sym, false) {
                let code = nullable_this_code(inf, cx, e);
                let the_name = inf.interner.resolve(n.sym).to_string();
                let since = inf.diagnostics.len();
                inf.null_warning(this, code, n.span, &[&the_name]);
                inf.attach_non_promotion(since, cx, None, n.span);
                let nn = inf.non_null(this);
                return match inf.lookup_member(cx.lib, nn, n.sym, false) {
                    Busca::Achado(m) => {
                        resolver(inf, cx, e, m.resolved.clone());
                        m.tipo
                    }
                    _ => inf.core.dynamic_,
                };
            }
            match inf.lookup_member(cx.lib, this, n.sym, false) {
                Busca::Achado(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    field_read(inf, cx, e, Base::This, m.tipo)
                }
                // Extensões ambíguas (`ThisLookup.lookupGetter` →
                // `TypePropertyResolver`): a ambiguidade no nome, e sem
                // getter o `UNDEFINED_IDENTIFIER` também
                // (`simple_identifier_resolver.dart:208-221`).
                Busca::Dinamico if inf.extension_ambiguity.is_some() => {
                    inf.report_extension_ambiguity(n.span);
                    undefined_name_without_this(inf, cx, n);
                    inf.table.invalid(inf.core.dynamic_)
                }
                Busca::Dinamico => inf.core.dynamic_,
                Busca::NeverType => inf.core.never,
                Busca::Missing => {
                    undefined_read_name(inf, cx, n, Lexico::Nada);
                    inf.table.invalid(inf.core.dynamic_)
                }
            }
        }
        NameRef::Forward(decl) => {
            let msg = format!("{}: '{}'", REFERENCED_BEFORE_DECLARATION.template, inf.interner.resolve(n.sym));
            warning_before_declaration(inf, n, msg, decl);
            // `checkReadOfNotAssignedLocalVariable` (3.6.2 `resolver.dart:643-679`):
            // o local ainda não atribuído, se `final` (não `late`; o `const`
            // não), é `READ_POTENTIALLY_UNASSIGNED_FINAL` no nome.
            let forward_final = inf.program.unit(cx.unit).ast.stmts.iter().any(|st| match &st.kind {
                ast::StmtKind::Variables(vl) => vl.final_ && !vl.const_ && !vl.late && vl.variables.iter().any(|v| v.name.span == decl),
                _ => false,
            });
            if forward_final {
                let the_name = inf.interner.resolve(n.sym).to_string();
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::READ_POTENTIALLY_UNASSIGNED_FINAL, n.span, &[&the_name]);
            }
            // O analyzer resolve o nome para o local declarado adiante (o
            // escopo do bloco já o tem): o `ReferenceFinder` vê a dependência
            // (`const x = [x];` é ciclo).
            if inf.registrar_locais {
                inf.body_types.units[cx.unit.0 as usize].declaracoes_de_locais.insert(e, decl.start);
            }
            // O tipo do elemento neste ponto: o `ResolutionVisitor` já pôs o
            // tipo escrito, ou `dynamic` sem tipo (`resolution_visitor.dart:1397`);
            // a inferência do implícito só vem na declaração. `Function f = () { x = f; };`
            // dá `invalid_assignment` de `Function` (`ref_before_declaration_test.dart:71`).
            // Os avisos da anotação são os da declaração, relatados lá.
            let written = inf.program.unit(cx.unit).ast.stmts.iter().find_map(|st| match &st.kind {
                ast::StmtKind::Variables(vl) if vl.variables.iter().any(|v| v.name.span == decl) => Some(vl.ty),
                _ => None,
            });
            match written.flatten() {
                Some(t) => {
                    let (nd, nu) = (inf.diagnostics.len(), inf.warning_units.len());
                    let r = inf.type_from_annotation(cx, t);
                    inf.diagnostics.truncate(nd);
                    inf.warning_units.truncate(nu);
                    r
                }
                None => inf.core.dynamic_,
            }
        }
        // Só curingas declaram `_` aqui (3.7): usar `_` é erro
        // (`Undefined name '_'` no CFE).
        NameRef::Nenhum if cx.curinga == Some(n.sym) => {
            inf.erro_de_linguagem(cx.unit, n.span, WILDCARD_DOES_NOT_BIND.template.to_string());
            inf.core.dynamic_
        }
        NameRef::Nenhum => {
            undefined_read_name(inf, cx, n, Lexico::Nada);
            inf.table.invalid(inf.core.dynamic_)
        }
    }
}

/// O código do uso sem checagem pelo `this` implícito
/// (`type_property_resolver.dart:107-140`): pelo pai do nome — a invocação
/// de método, o operador binário (o pai de cascata vale pela primeira
/// seção), senão o acesso a propriedade.
fn nullable_this_code(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> dartforge_diagnostics::Codigo {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let a = ast(inf, cx);
    let parent = match inf.parent_of(cx.unit, e) {
        dartforge_frontend::pais::Pai::Expr(p) => p,
        _ => return c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
    };
    // Alvo de cascata: vale a primeira seção (`..m()` é invocação).
    if let ExprKind::Cascade { sections, .. } = &a.expr(parent).kind {
        return match sections.first().map(|&s| &a.expr(s).kind) {
            Some(ExprKind::Call { .. }) => c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
            _ => c::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
        };
    }
    match &a.expr(parent).kind {
        ExprKind::Binary { .. } => c::UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE,
        // A escrita pelo `this` implícito (`nome += 1`, `nome ??= 1`,
        // `nome++`): a variante de método, como a do `lookupSetter`
        // (conferido no oráculo 3.6.2).
        ExprKind::Assign { target, .. } if *target == e => c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
        ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } if *operand == e => {
            c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE
        }
        // `nome()`: o `MethodInvocation` do nome.
        ExprKind::Call { target, .. } if *target == e => c::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
        // `nome.m()`: o nome é o alvo do `MethodInvocation`.
        ExprKind::Property { target, .. } if *target == e => match inf.parent_of(cx.unit, parent) {
            dartforge_frontend::pais::Pai::Expr(q) if matches!(&a.expr(q).kind, ExprKind::Call { target, .. } if *target == parent) => {
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
pub(crate) fn promotion_target(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId) -> Option<LocalId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            promotion_target(inf, cx, i)
        }
        ExprKind::Identifier(n) => match cx.buscar(n.sym) {
            Some(Nome::Local(id)) if !cx.local(id).funcao_local => Some(id),
            Some(_) => None,
            // `_x` implícito: `this._x`.
            None => property_target(inf, cx, e, Base::This),
        },
        ExprKind::Property { target, null_aware: false, .. } => {
            let t = *target;
            let base = property_base(inf, cx, t)?;
            let before = force_cascade_version(inf, cx, t);
            let r = property_target(inf, cx, e, base);
            cx.forced_version = before;
            r
        }
        ExprKind::This => this_local(inf, cx),
        _ => None,
    }
}

/// A base de uma propriedade lida de `t` (o `PropertyTarget` com nó SSA):
/// `this`, `super`, uma local (não `late`, não função local) ou outra
/// propriedade promovível (estável).
fn property_base(inf: &mut BodyInferrer<'_>, cx: &mut Body, t: ExprId) -> Option<Base> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(t).kind {
        ExprKind::Parenthesized(i) => {
            let i = *i;
            property_base(inf, cx, i)
        }
        ExprKind::This => Some(Base::This),
        ExprKind::Super => Some(Base::Super),
        // A seção de cascata lê do alvo da cascata, cuja referência é a da
        // expressão alvo (`cascadeExpression_afterTarget` guarda o
        // `_getExpressionReference(target)`): `c?.._field` vê a promoção de
        // `c._field`.
        ExprKind::CascadeTarget => cx.cascade_bases.last().copied().flatten().map(|(b, _)| b),
        // O valor da cascata é o do alvo (a mesma referência).
        ExprKind::Cascade { target, .. } => {
            let tgt = *target;
            property_base(inf, cx, tgt)
        }
        ExprKind::Identifier(n) => match cx.buscar(n.sym) {
            Some(Nome::Local(id)) if !cx.local(id).late && !cx.local(id).funcao_local => Some(Base::Local(id)),
            Some(_) => None,
            None => stable_property(inf, cx, t, Base::This).map(Base::Local),
        },
        ExprKind::Property { target, null_aware: false, .. } => {
            let t2 = *target;
            let b = property_base(inf, cx, t2)?;
            let before = force_cascade_version(inf, cx, t2);
            let r = stable_property(inf, cx, t, b).map(Base::Local);
            cx.forced_version = before;
            r
        }
        _ => None,
    }
}

/// Com `t` o alvo de cascata, força a versão da base dele à do início da
/// cascata; devolve a forçada anterior, para restaurar.
fn force_cascade_version(inf: &BodyInferrer<'_>, cx: &mut Body, t: ExprId) -> Option<(Base, u32)> {
    let before = cx.forced_version;
    if matches!(ast(inf, cx).expr(t).kind, ExprKind::CascadeTarget)
        && let Some(Some(bv)) = cx.cascade_bases.last().copied()
    {
        cx.forced_version = Some(bv);
    }
    before
}

/// O membro de instância (getter, campo ou método, de classe ou de
/// extensão) que a leitura `e` alcança, com o nome escrito: o
/// `propertyMember` do `propertyGet`.
fn read_member(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> Option<(FunctionElementId, SymbolId)> {
    let the_name = match &ast(inf, cx).expr(e).kind {
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
    Some((f, the_name))
}

/// O sintético estável de uma propriedade promovível lida em `e` (`None`
/// se não é promovível).
fn stable_property(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, base: Base) -> Option<LocalId> {
    let (f, the_name) = read_member(inf, cx, e)?;
    if !inf.promotable_property(cx.lib, f) {
        return None;
    }
    let the_key = (base, cx.base_version(base), the_name);
    if let Some(&id) = cx.campos.get(&the_key) {
        cx.ensure_model(id);
        return Some(id);
    }
    // O tipo declarado visto pelo receptor: o da leitura antes de qualquer
    // promoção (a primeira leitura, que ainda não tinha o sintético).
    let t = inf.body_types.units[cx.unit.0 as usize].get_type(e).unwrap_or(inf.core.dynamic_);
    let id = cx.declare_synthetic(Local { nome: the_name, tipo: t, final_: true, late: false, const_: false, offset: 0, funcao_local: false });
    cx.campos.insert(the_key, id);
    Some(id)
}

/// A propriedade lida em `e` como alvo de promoção: a promovível pelo
/// sintético estável; a não promovível ganha uma geração nova (o nó SSA
/// fresco de cada acesso), que só o why-not-promoted das leituras seguintes
/// consulta.
fn property_target(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, base: Base) -> Option<LocalId> {
    if let Some(id) = stable_property(inf, cx, e, base) {
        return Some(id);
    }
    let (_, the_name) = read_member(inf, cx, e)?;
    if let Some(&id) = cx.read_generation.get(&e) {
        cx.ensure_model(id);
        return Some(id);
    }
    let the_key = (base, cx.base_version(base), the_name);
    let t = inf.body_types.units[cx.unit.0 as usize].get_type(e).unwrap_or(inf.core.dynamic_);
    let id = cx.declare_synthetic(Local { nome: the_name, tipo: t, final_: true, late: false, const_: false, offset: 0, funcao_local: false });
    cx.generations.entry(the_key).or_default().push(id);
    cx.read_generation.insert(e, id);
    Some(id)
}

/// O sintético de `this` (criado na primeira vez).
pub(crate) fn this_local(inf: &mut BodyInferrer<'_>, cx: &mut Body) -> Option<LocalId> {
    let typ = cx.tipo_this?;
    if let Some(id) = cx.local_this {
        cx.ensure_model(id);
        return Some(id);
    }
    let the_name = inf.sym.this_?;
    let id = cx.declare_synthetic(Local { nome: the_name, tipo: typ, final_: true, late: false, const_: false, offset: 0, funcao_local: false });
    cx.local_this = Some(id);
    Some(id)
}

/// O tipo lido da propriedade `e` (`alvo.nome`, já resolvida) de tipo
/// declarado `t`, com a promoção da propriedade quando o alvo é uma base
/// promovível: a função de `this._f()` (`FunctionExpressionInvocation` do
/// `PropertyAccess`).
pub(crate) fn property_read_type(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, t: TypeId) -> TypeId {
    let ExprKind::Property { target, null_aware: false, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind else { return t };
    let tgt = *target;
    match property_base(inf, cx, tgt) {
        Some(base) => field_read(inf, cx, e, base, t),
        None => t,
    }
}

/// Tipo lido de uma propriedade (`propertyGet` + `_handleProperty`): o
/// promovido, quando a propriedade é promovível e o tipo promovido é
/// subtipo do não promovido; a leitura da não promovível registra o
/// why-not-promoted dela.
pub(crate) fn field_read(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, base: Base, t: TypeId) -> TypeId {
    let Some((f, the_name)) = read_member(inf, cx, e) else { return t };
    // A seção de cascata lê do temporário do alvo, com o nó SSA do início
    // da cascata: uma escrita na local durante a cascata não tira a
    // promoção das seções seguintes (`c?.._f.g([c = C()]).._f`).
    let version = match &ast(inf, cx).expr(e).kind {
        ExprKind::Property { target, .. } if matches!(ast(inf, cx).expr(*target).kind, ExprKind::CascadeTarget) => match cx.cascade_bases.last().copied().flatten() {
            Some((b, v)) if b == base => v,
            _ => cx.base_version(base),
        },
        _ => cx.base_version(base),
    };
    let the_key = (base, version, the_name);
    if inf.promotable_property(cx.lib, f) {
        return match cx.campos.get(&the_key) {
            Some(&id) => {
                let p = cx.flow.current_type(id, t);
                if p != t && !inf.sub(p, t) {
                    t
                } else {
                    p
                }
            }
            None => t,
        };
    }
    inf.record_property_non_promotion(cx, e, the_key, f);
    t
}

/// Coerção de tear-off genérico para um contexto de função não genérico
/// (instanciação implícita).
fn instantiate_in_context(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId, ctx: TypeId) -> TypeId {
    instantiate_in_context_with(inf, cx, e, t, ctx, true)
}

/// [`instanciar_em_contexto`]; `registrar` grava a instanciação do tear-off
/// (a referência implícita a `call` não é tear-off do nó).
fn instantiate_in_context_with(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId, ctx: TypeId, registrar: bool) -> TypeId {
    let Type::Function { type_params, ret, positional, optional, named, nullable } = inf.table.get(t).clone() else { return t };
    if type_params.is_empty() || inf.the_is_unknown(ctx) {
        return t;
    }
    let k = inf.greatest_closure_of(ctx);
    let k = inf.non_null(k);
    let Type::Function { type_params: tp_ctx, .. } = inf.table.get(k).clone() else { return t };
    if !tp_ctx.is_empty() {
        return t;
    }
    let without = inf.table.intern(Type::Function {
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
    gi.with_origin(crate::constraints::Origem::Funcao { declarado: t, contexto: k });
    gi.generic_metadata = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::GenericMetadata);
    let (interner, program) = (inf.interner, inf.program);
    let mut env = inf.env();
    gi.constrain_return(without, k, &mut env);
    let args = gi.choose_final(&mut env);
    let failures = gi.failures(&args, &mut env, interner, program);
    let r = crate::constraints::instantiate_function(t, &args, &mut env);
    drop(env);
    let sp = inf.span_expr(cx.unit, e);
    for (the_name, suffix) in failures {
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::COULD_NOT_INFER, sp, &[&the_name, &suffix]);
    }
    if registrar {
        inf.body_types.units[cx.unit.0 as usize].tearoff_instantiations.insert(e, args.into_boxed_slice());
    }
    r
}

/// Infere um nó; devolve `(tipo, curto)` onde `curto` diz que há `?.` na
/// cadeia abaixo (o chamador que termina a cadeia torna o tipo anulável).
pub(crate) fn infer_at(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, ctx: TypeId, _chain: bool) -> (TypeId, bool) {
    let a = ast(inf, cx);
    let expr = a.expr(e);
    let span = expr.span;
    // `checkUnreachableNode` (`NullSafetyDeadCodeVerifier.visitNode`): a
    // primeira expressão visitada num fluxo inalcançável é o primeiro nó
    // morto; o trecho vai até o fim do bloco básico em curso (ou da última
    // instrução do bloco).
    if !cx.flow.alcancavel && cx.dead_code.is_none() {
        let end = cx.flow_ends.last().copied().or(cx.block_ends.last().copied()).unwrap_or(span.end).max(span.end);
        inf.aviso(DEAD_CODE.template.to_string(), dartforge_diagnostics::Span { start: span.start, end: end });
        cx.dead_code = Some(cx.flow_ends.len());
    }
    let mut short_circuit = false;
    shorthands::record_chain(inf, cx, e, ctx);
    let t = match &expr.kind {
        ExprKind::DotShorthand { name, .. } => shorthands::valor(inf, cx, e, name.sym, ctx),
        ExprKind::Int(_) => {
            let s = inf.greatest_closure_of(ctx);
            let (int, double) = (inf.core.int, inf.core.double);
            let typ = if !inf.the_is_unknown(ctx) && inf.sub(double, s) && !inf.sub(int, s) { double } else { int };
            let the_negated = cx.negated_literal == Some(e);
            if the_negated {
                cx.negated_literal = None;
            }
            super::integers::literal_out_of_range(inf, cx, e, typ, the_negated);
            typ
        }
        ExprKind::Double(_) => inf.core.double,
        ExprKind::Bool(_) => inf.core.bool_,
        ExprKind::Null => inf.core.null,
        ExprKind::String(lit) => {
            for p in lit.parts.iter() {
                if let ast::StringPart::Interpolation(i) = p {
                    let t = infer_free(inf, cx, *i);
                    void_use(inf, cx, *i, t);
                }
            }
            inf.core.string
        }
        ExprKind::Symbol(_) => inf.core.symbol,
        // `$this` numa interpolação chega como identificador.
        ExprKind::Identifier(n) if Some(n.sym) == inf.sym.this_ => cx.tipo_this.unwrap_or(inf.core.dynamic_),
        ExprKind::Identifier(n) => {
            let t = identificador(inf, cx, e, *n);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        // O `_thisType` do `ResolverVisitor` vale no corpo inteiro da
        // declaração, também nos membros estáticos, nos inicializadores de
        // campo e nas `factory` (o `this` ali é `INVALID_REFERENCE_TO_THIS`,
        // mas tem o tipo dela).
        ExprKind::This => match cx.tipo_this {
            Some(t) => t,
            None if !cx.untyped_this => analyzer_this_type(inf, cx).unwrap_or(inf.core.dynamic_),
            None => inf.core.dynamic_,
        },
        // `visitSuperExpression` (3.6.2 `static_type_analyzer.dart:240-252`):
        // sem `this` ou dentro de `extension`, o tipo é inválido.
        ExprKind::Super => match cx.tipo_this {
            Some(t) if cx.extensao.is_none() => t,
            _ => inf.table.invalid(inf.core.dynamic_),
        },
        ExprKind::Parenthesized(i) => {
            let mark = cx.cadeias.len();
            let (t, c) = infer_at(inf, cx, *i, ctx, false);
            close_chain(inf, cx, mark);
            let t = if c { inf.anulavel(t) } else { t };
            registrar(inf, cx, *i, t);
            t
        }
        ExprKind::List { .. } | ExprKind::SetOrMap { .. } => collections::literal(inf, cx, e, ctx),
        ExprKind::Record { positional, named, .. } => {
            let k = inf.greatest_closure_if_known(ctx);
            let k = inf.non_null(k);
            let (ctx_pos, ctx_named) = match inf.table.get(k).clone() {
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
                let tp = dynamic_cast_in_field(inf, tp, c);
                if matches!(inf.table.get(tp), Type::Void) {
                    let sp = inf.span_expr(cx.unit, *p);
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
                }
                pos.push(tp);
            }
            let mut nm = Vec::new();
            for (n, x) in named.iter() {
                let c = ctx_named.as_ref().and_then(|v| v.iter().find(|(s, _)| *s == n.sym).map(|(_, t)| *t)).unwrap_or(u);
                let tx = inferir(inf, cx, *x, c);
                let tx = dynamic_cast_in_field(inf, tx, c);
                if matches!(inf.table.get(tx), Type::Void) {
                    let end = inf.span_expr(cx.unit, *x).end;
                    let sp = dartforge_diagnostics::Span { start: n.span.start, end: end };
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
                }
                nm.push((n.sym, tx));
            }
            inf.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
        }
        ExprKind::InstanceCreation { .. } => {
            let t = calls::instanciacao(inf, cx, e, ctx);
            null_free_api(inf, cx, e, t);
            t
        }
        // `insertGenericFunctionInstantiation` (3.6.2 `resolver.dart:1171`)
        // depois de `visitFunctionExpression` (`:2833`), das invocações
        // (`:2855`, `:3244`), do índice (`:3035`), dos operadores
        // (`:2094`, `:3414`, `:3455`), de `as` (`:1867`), da atribuição
        // (`:1933`) e de `await` (`:2081`), além dos identificadores e
        // propriedades.
        ExprKind::FunctionExpression(f) => {
            let t = functions::expressao_de_funcao(inf, cx, *f, ctx, e);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Property { target, name, null_aware } => {
            let (t, c) = propriedade(inf, cx, e, *target, *name, *null_aware);
            short_circuit = c;
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Index { target, index, null_aware } => {
            let (t, c) = read_index(inf, cx, e, *target, *index, *null_aware, ctx);
            short_circuit = c;
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Call { .. } => {
            if let Some(t) = shorthands::construction(inf, cx, e, ctx) {
                t
            } else {
                let (t, c) = calls::chamada(inf, cx, e, ctx);
                short_circuit = c;
                null_free_api(inf, cx, e, t);
                instantiate_in_context(inf, cx, e, t, ctx)
            }
        }
        ExprKind::TypeArguments { target, type_args } => {
            if type_reference(inf, cx, e).is_some() {
                check_literal_arguments(inf, cx, e, false);
                // O `NamedType.type` de cada argumento (o verificador de
                // constantes lê o do `const (List<T>)`); os relatos já saíram
                // na conferência acima.
                let before = inf.diagnostics.len();
                for &x in type_args.iter() {
                    inf.type_argument_type(cx, x);
                }
                inf.diagnostics.truncate(before);
                inf.warning_units.truncate(before);
                record_type_ref(inf, cx, e);
                inf.core.type_
            } else if alias_without_class(inf, cx, *target) {
                // `Cb<String>` com `Cb` alias de um tipo que não é classe
                // (`typedef Cb<T> = void Function()`): também `TypeLiteral`.
                check_literal_arguments(inf, cx, e, false);
                for &x in type_args.iter() {
                    inf.type_argument_type(cx, x);
                }
                record_type_ref(inf, cx, e);
                inf.core.type_
            } else {
                // `C.nome<T>` (tear-off de construtor com argumentos):
                // `WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR`, e a instanciação
                // segue sobre o tipo do tear-off.
                if let ExprKind::Property { target: r, name, .. } = &ast(inf, cx).expr(*target).kind
                    && let Some(rt) = type_reference(inf, cx, *r)
                    && let TypeRef::Classe(c, _) | TypeRef::Alias(c, _, _) = rt
                {
                    let the_key = if Some(name.sym) == inf.sym.new_ { inf.sym.vazio } else { Some(name.sym) };
                    if the_key.and_then(|k| inf.constructor_or_primary(c, k)).is_some() {
                        let written = ast(inf, cx).expr(*r).span;
                        super::calls::types_in_constructor(inf, cx.unit, written, *name, type_args);
                        // O `FunctionReference` de um `ConstructorReference`
                        // fica `InvalidType` (`function_reference_resolver.dart:250-252`).
                        infer_free(inf, cx, *target);
                        for &x in type_args.iter() {
                            inf.type_argument_type(cx, x);
                        }
                        return (inf.table.invalid(inf.core.dynamic_), false);
                    }
                }
                // `p.f<T>` com `p` que não resolve no escopo
                // (`_resolvePrefixedIdentifierFunction`, 3.6.2
                // `function_reference_resolver.dart:413-426`): no prefixo, com o
                // nome prefixado inteiro (`unresolved.Class<int>.named()`).
                if let ExprKind::Property { target: r, name, null_aware: false } = &ast(inf, cx).expr(*target).kind
                    && let ExprKind::Identifier(p) = &ast(inf, cx).expr(*r).kind
                    && matches!(resolve_name(inf, cx, p.sym, false), NameRef::Nenhum | NameRef::ImplicitThis)
                    && !crate::scope::should_ignore_undefined(inf.program, inf.interner, cx.unit, None, p.sym)
                {
                    let (p, name) = (*p, *name);
                    let text = format!("{}.{}", inf.interner.resolve(p.sym), inf.interner.resolve(name.sym));
                    inf.aviso(format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, text), p.span);
                    for &x in type_args.iter() {
                        inf.type_argument_type(cx, x);
                    }
                    return (inf.table.invalid(inf.core.dynamic_), false);
                }
                // `m<T>` com `m` estático herdado (ou do tipo estendido) achado
                // pelo `this` implícito (`_resolveSimpleIdentifierFunction`,
                // 3.6.2 `function_reference_resolver.dart:657-694`): o
                // `FunctionReference` relata o acesso não qualificado, que o
                // tear-off sem argumentos de tipo não relata.
                if let ExprKind::Identifier(n) = &ast(inf, cx).expr(*target).kind
                    && matches!(resolve_name(inf, cx, n.sym, false), NameRef::Nenhum | NameRef::ImplicitThis)
                    && let Some(this) = analyzer_this_type(inf, cx)
                    && let ThroughThis::Estatico(owner, true) = lookup_through_this(inf, cx, this, n.sym, false)
                {
                    let n = *n;
                    warn_unqualified_static(inf, cx, owner, n);
                }
                let t = infer_free(inf, cx, *target);
                let targs: Vec<TypeId> = type_args.iter().map(|&x| inf.type_argument_type(cx, x)).collect();
                // `_resolve`: o parâmetro de tipo vale pelo limite.
                let raw = match inf.table.get(t).clone() {
                    Type::TypeParameter { param, .. } => inf.table.param(param).bound,
                    _ => t,
                };
                match inf.table.get(raw).clone() {
                    Type::Function { type_params, .. } if type_params.len() == targs.len() => {
                        // `checkFunctionReference` do `ErrorVerifier`.
                        let nodes = type_args.to_vec();
                        let sp = inf.span_expr(cx.unit, e);
                        let visible: Vec<crate::table::TypeParamId> = cx.visible_type_parameters().into_values().collect();
                        calls::check_explicit_bounds(inf, cx.unit, &type_params, &targs, &nodes, sp, Some(&visible));
                        let mut env = inf.env();
                        crate::constraints::instantiate_function(raw, &targs, &mut env)
                    }
                    Type::Function { type_params, .. } => {
                        // `_checkTypeArguments` (`function_reference_resolver.dart:115-150`):
                        // com o nome de uma função ou método declarado,
                        // `WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION`; de uma
                        // variável, getter ou expressão, a variante anônima.
                        // Os argumentos viram `dynamic`.
                        if let Some(sp) = super::calls::type_list_range(inf, cx.unit, type_args) {
                            let (n, d) = (type_params.len().to_string(), targs.len().to_string());
                            match referenced_function_name(inf, cx, *target) {
                                Some(the_name) => inf.warning_with_code(
                                    dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION,
                                    sp,
                                    &[&the_name, &n, &d],
                                ),
                                // O tipo da função vai depois: só o texto do
                                // 3.13.4 o usa (`The type of this function is …`).
                                None => {
                                    let typ = inf.table.format(raw, inf.interner, inf.program);
                                    inf.warning_with_code(
                                        dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION,
                                        sp,
                                        &[&n, &d, &typ],
                                    )
                                }
                            }
                        }
                        let dynamic = vec![inf.core.dynamic_; type_params.len()];
                        let mut env = inf.env();
                        crate::constraints::instantiate_function(raw, &dynamic, &mut env)
                    }
                    _ if inf.table.e_invalido(raw) => raw,
                    // `p.nome<…>` com `nome` que não resolve no prefixo (o
                    // import ausente, `shouldIgnoreUndefined`): o resolvedor do
                    // nome prefixado para sem chegar ao `_resolve` da
                    // referência, e o tipo é inválido, sem relato.
                    _ if matches!(&ast(inf, cx).expr(*target).kind, ExprKind::Property { target: p, .. }
                        if matches!(inf.body_types.units[cx.unit.0 as usize].get_resolved(*p), Some(Resolved::Prefix(_))))
                        && inf.body_types.units[cx.unit.0 as usize].get_resolved(*target).is_none() =>
                    {
                        inf.table.invalid(inf.core.dynamic_)
                    }
                    // `node<…>` de um objeto com o método `call`: `node.call<…>`
                    // (`_resolveAsImplicitCallReference`, 3.6.2
                    // `function_reference_resolver.dart:287-306`): a contagem
                    // confere com os parâmetros de tipo do `call`, com o nome
                    // `call`, e o tipo é o do método instanciado.
                    Type::Interface { .. }
                        if inf.sym.call.is_some_and(|call| matches!(inf.lookup_member(cx.lib, raw, call, false), super::members::Busca::Achado(m) if m.metodo)) =>
                    {
                        let call = inf.sym.call.expect("call");
                        let call_type = match inf.lookup_member(cx.lib, raw, call, false) {
                            super::members::Busca::Achado(m) => m.tipo,
                            _ => unreachable!(),
                        };
                        match inf.table.get(call_type).clone() {
                            Type::Function { type_params, .. } => {
                                let used = if type_params.len() == targs.len() {
                                    targs.clone()
                                } else {
                                    if let Some(sp) = super::calls::type_list_range(inf, cx.unit, type_args) {
                                        let (n, d) = (type_params.len().to_string(), targs.len().to_string());
                                        inf.warning_with_code(
                                            dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION,
                                            sp,
                                            &["call", &n, &d],
                                        );
                                    }
                                    vec![inf.core.dynamic_; type_params.len()]
                                };
                                let mut env = inf.env();
                                crate::constraints::instantiate_function(call_type, &used, &mut env)
                            }
                            _ => inf.core.dynamic_,
                        }
                    }
                    _ => {
                        // `DISALLOWED_TYPE_INSTANTIATION_EXPRESSION`
                        // (`function_reference_resolver.dart:270-278`), com
                        // os tear-offs de construtor (2.15).
                        let version = inf.program.library(cx.lib).features.versao();
                        if version >= dartforge_frontend::features::LanguageVersion::new(2, 15) {
                            let sp = inf.span_expr(cx.unit, *target);
                            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::DISALLOWED_TYPE_INSTANTIATION_EXPRESSION, sp, &[]);
                            inf.table.invalid(inf.core.dynamic_)
                        } else {
                            inf.core.dynamic_
                        }
                    }
                }
            }
        }
        ExprKind::Unary { op, operand } => {
            let t = unary(inf, cx, e, *op, *operand, ctx, &mut short_circuit);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Binary { op, left, right } => {
            let t = binary(inf, cx, e, *op, *left, *right, ctx);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Conditional { condition, then, else_ } => {
            let (then, else_) = (*then, *else_);
            let (vf, ff) = checked_condition(inf, cx, *condition);
            let before = cx.flow.clone();
            cx.flow = vf;
            let t1 = flow_operand(inf, cx, then, None, |inf, cx| inferir(inf, cx, then, ctx));
            let after1 = std::mem::replace(&mut cx.flow, ff);
            let t2 = flow_operand(inf, cx, else_, None, |inf, cx| inferir(inf, cx, else_, ctx));
            let after2 = std::mem::replace(&mut cx.flow, before);
            cx.flow = inf.juntar(&after1, &after2);
            upper_bound_in_context(inf, t1, t2, ctx)
        }
        ExprKind::Is { value, ty, negated } => {
            let (vf, ff) = type_test(inf, cx, *value, *ty, *negated, span);
            cx.flow = inf.juntar(&vf, &ff);
            inf.core.bool_
        }
        ExprKind::As { value, ty } => {
            let v = infer_free(inf, cx, *value);
            let t = inf.type_in_context(cx, *ty, crate::resolve::TypeContext::As);
            let equal = v == t || crate::ops::without_display(v, inf.table) == crate::ops::without_display(t, inf.table);
            if equal && !inf.e_dynamic(v) && !inf.table.e_invalido(v) && !inf.table.e_invalido(t) {
                inf.aviso(UNNECESSARY_CAST.template.to_string(), span);
            }
            // `CAST_FROM_NULL_ALWAYS_FAILS` (`visitAsExpression`,
            // `best_practices_verifier.dart:135-150`).
            let non_nullable = inf.is_non_nullable(t);
            if non_nullable && e_null_do_core(inf, v) {
                inf.warning_with_code(dartforge_diagnostics::codigos::warning::CAST_FROM_NULL_ALWAYS_FAILS, span, &[]);
            }
            // `CAST_FROM_NULLABLE_ALWAYS_FAILS` (`ResolverVisitor.visitAsExpression`,
            // `resolver.dart:1871-1887`): o operando é o identificador de uma
            // local ou parâmetro de tipo declarado anulável, definitivamente
            // não atribuído.
            if non_nullable && !e_null_do_core(inf, v)
                && let ExprKind::Identifier(n) = &ast(inf, cx).expr(*value).kind
                && let Some(Resolved::Local(id)) = inf.body_types.units[cx.unit.0 as usize].get_resolved(*value).cloned()
            {
                let n = *n;
                let declared = cx.local(id).tipo;
                if inf.the_is_nullable(declared) && cx.flow.unassigned(id) {
                    let the_name = inf.interner.resolve(n.sym).to_string();
                    inf.warning_with_code(dartforge_diagnostics::codigos::warning::CAST_FROM_NULLABLE_ALWAYS_FAILS, n.span, &[&the_name]);
                }
            }
            if let Some(id) = promotion_target(inf, cx, *value) {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                inf.promover(&mut f, id, decl, t);
                cx.flow = f;
            }
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Assign { op, target, value } => {
            let t = atribuicao(inf, cx, e, *op, *target, *value, &mut short_circuit);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::PatternAssign { pattern, value } => patterns::pattern_assignment(inf, cx, *pattern, *value),
        ExprKind::Cascade { target, sections, null_aware } => {
            let t = inferir(inf, cx, *target, ctx);
            void_use(inf, cx, *target, t);
            // `EXTENSION_OVERRIDE_WITH_CASCADE`
            // (`method_invocation_resolver.dart:431-437`,
            // `property_element_resolver.dart:606-612`): no nome da extensão
            // do `E(x)` alvo da cascata (um relato por cascata).
            if cx.overlays.contains_key(target)
                && let ExprKind::Call { target: f, .. } = &ast(inf, cx).expr(*target).kind
            {
                let the_name = match &ast(inf, cx).expr(*f).kind {
                    ExprKind::Identifier(n) => Some(n.span),
                    ExprKind::Property { name, .. } => Some(name.span),
                    ExprKind::TypeArguments { target: g, .. } => match &ast(inf, cx).expr(*g).kind {
                        ExprKind::Identifier(n) => Some(n.span),
                        ExprKind::Property { name, .. } => Some(name.span),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(sp) = the_name {
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::EXTENSION_OVERRIDE_WITH_CASCADE, sp, &[]);
                }
            }
            // `c?..x` com `c` não anulável: relatado pela primeira seção
            // (`visitPropertyAccess`/`visitMethodInvocation`/`visitIndexExpression`
            // do `ErrorVerifier`).
            if *null_aware {
                if let Some(&first) = sections.first() {
                    if first_section_without_rewrite(inf, cx, first, t) {
                        unnecessary_null_operator(inf, cx, *target, t);
                    }
                }
            }
            // `promoteToNonNull`, como no `?.`: o parâmetro de tipo de limite
            // anulável vira `T & NonNull(limite)` (`nn?..toRadixString(16)`).
            let r = if *null_aware {
                match inf.table.get(t) {
                    Type::TypeParameter { nullable: false, .. } => inf.non_null_promotion(t),
                    _ => inf.non_null(t),
                }
            } else {
                t
            };
            cx.cascades.push(r);
            cx.cascade_targets.push(*target);
            // Alvo que não é referência (`getC()?..`): o temporário da
            // cascata é um nó SSA novo, cujas propriedades promovem entre as
            // seções.
            // A local capturada por escrita não promove, mas o temporário
            // sim: ganha base própria.
            let captured = |cx: &Body, b: Base| matches!(b, Base::Local(id) if cx.flow.modelo(id).is_some_and(|m| m.captured));
            let bv = match property_base(inf, cx, *target).filter(|&b| !captured(cx, b)) {
                Some(b) => Some((b, cx.base_version(b))),
                None => inf.sym.vazio.map(|the_name| {
                    let id = cx.declare_synthetic(Local { nome: the_name, tipo: r, final_: true, late: false, const_: false, offset: 0, funcao_local: false });
                    (Base::Local(id), cx.base_version(Base::Local(id)))
                }),
            };
            cx.cascade_bases.push(bv);
            // `nullAwareAccess_rightBegin` sobre o temporário do alvo
            // (`tryMarkNonNullable`): com alvo `Null`, o não nulo é `Never` e
            // as seções ficam inalcançáveis (a primeira é o nó morto); no fim,
            // a junção com o caminho do atalho nulo volta a alcançar.
            let shorthand = if *null_aware && matches!(inf.table.get(r), Type::Never) {
                let s = cx.flow.clone();
                cx.flow = s.unreachable();
                Some(s)
            } else {
                None
            };
            let secs = sections.to_vec();
            for s in secs {
                // A seção (`MethodInvocation`/`PropertyAccess`/`IndexExpression`
                // da cascata) começa no operador `..`/`?..`: é dali o nó morto.
                if !cx.flow.alcancavel && cx.dead_code.is_none() {
                    let sp = inf.span_expr(cx.unit, s);
                    let source = inf.program.unit(cx.unit).source.as_str();
                    let before = source[..sp.start].trim_end();
                    let start = if before.ends_with("?..") {
                        before.len() - 3
                    } else if before.ends_with("..") {
                        before.len() - 2
                    } else {
                        sp.start
                    };
                    let end = cx.flow_ends.last().copied().or(cx.block_ends.last().copied()).unwrap_or(sp.end).max(sp.end);
                    inf.aviso(DEAD_CODE.template.to_string(), dartforge_diagnostics::Span { start: start, end: end });
                    cx.dead_code = Some(cx.flow_ends.len());
                }
                infer_free(inf, cx, s);
            }
            if let Some(s) = shorthand {
                let after = cx.flow.clone();
                cx.flow = inf.juntar(&s, &after);
            }
            cx.cascade_bases.pop();
            cx.cascade_targets.pop();
            cx.cascades.pop();
            t
        }
        ExprKind::CascadeTarget => cx.cascades.last().copied().unwrap_or(inf.core.dynamic_),
        ExprKind::Await(i) => {
            let k1 = await_context(inf, ctx);
            let t1 = inferir(inf, cx, *i, k1);
            void_use(inf, cx, *i, t1);
            // `AWAIT_OF_INCOMPATIBLE_TYPE` no `await`
            // (an611:src/generated/error_verifier.dart:2214-2223).
            if incompatible_with_await(inf, t1, 0) {
                let begin = inf.span_expr(cx.unit, e).start;
                let sp = dartforge_diagnostics::Span { start: begin, end: begin + 5 };
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::AWAIT_OF_INCOMPATIBLE_TYPE, sp, &[]);
            }
            let t = inf.flatten(t1);
            instantiate_in_context(inf, cx, e, t, ctx)
        }
        ExprKind::Throw(i) => {
            let t = infer_free(inf, cx, *i);
            void_use(inf, cx, *i, t);
            // `THROW_OF_INVALID_TYPE` (an611:src/generated/error_verifier.dart:5336-5347):
            // não atribuível a `Object`; `void` relata também.
            let object = inf.core.object;
            if !inf.atribuivel(t, object) {
                let tt = inf.table.format(t, inf.interner, inf.program);
                let sp = inf.span_expr(cx.unit, *i);
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::THROW_OF_INVALID_TYPE, sp, &[&tt]);
            }
            cx.flow.alcancavel = false;
            inf.core.never
        }
        ExprKind::Rethrow => {
            cx.flow.alcancavel = false;
            inf.core.never
        }
        ExprKind::Switch { value, cases } => patterns::switch_expression(inf, cx, *value, cases, ctx),
    };
    registrar(inf, cx, e, t);
    if matches!(inf.table.get(t), Type::Never) {
        cx.flow.alcancavel = false;
    }
    (t, short_circuit)
}

impl<'a> BodyInferrer<'a> {
    /// Fecho maior se o contexto não for `_` (senão o próprio `_`).
    pub(crate) fn greatest_closure_if_known(&mut self, ctx: TypeId) -> TypeId {
        if self.the_is_unknown(ctx) {
            ctx
        } else {
            self.greatest_closure_of(ctx)
        }
    }
}

/// `K1` de `await e` em contexto `K` (`inference.md`, "Await expressions").
fn await_context(inf: &mut BodyInferrer<'_>, k: TypeId) -> TypeId {
    if inf.the_is_unknown(k) {
        return inf.future_or(k);
    }
    match inf.table.get(k) {
        Type::FutureOr { .. } => k,
        Type::Dynamic => {
            let u = inf.core.unknown;
            inf.future_or(u)
        }
        _ => inf.future_or(k),
    }
}

/// UP dos ramos com a regra de `inference-update-3`: se UP não cabe no
/// contexto mas os dois ramos cabem, o tipo é o contexto.
pub(crate) fn upper_bound_in_context(inf: &mut BodyInferrer<'_>, t1: TypeId, t2: TypeId, ctx: TypeId) -> TypeId {
    let t = inf.up(t1, t2);
    if inf.the_is_unknown(ctx) {
        return t;
    }
    let s = inf.greatest_closure_of(ctx);
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
pub(crate) fn void_use(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId) -> bool {
    if !matches!(inf.table.get(t), Type::Void) {
        return false;
    }
    let a = ast(inf, cx);
    let types = &inf.body_types.units[cx.unit.0 as usize];
    let method = |x: ExprId| -> bool {
        match types.get_resolved(x) {
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
            ExprKind::Property { name, .. } if method(*target) => name.span,
            ExprKind::Identifier(n) if method(*target) => n.span,
            _ => a.expr(e).span,
        },
        _ => a.expr(e).span,
    };
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
    true
}

/// `isIncompatibleWithAwait` (an611:src/dart/element/type_system.dart:971-1001):
/// `S?` com `S` incompatível; extension type que não é subtipo de
/// `Future<Object?>`; parâmetro de tipo (promovido ou não) pelo limite.
fn incompatible_with_await(inf: &mut BodyInferrer<'_>, t: TypeId, prof: u32) -> bool {
    if prof > 16 {
        return false;
    }
    match inf.table.get(t).clone() {
        Type::ExtensionType { nullable, .. } => {
            if nullable {
                let nn = inf.non_null(t);
                return incompatible_with_await(inf, nn, prof + 1);
            }
            let oq = inf.core.object_nullable;
            let fut = inf.futuro(oq);
            !inf.sub(t, fut)
        }
        Type::Intersection { bound, .. } => incompatible_with_await(inf, bound, prof + 1),
        Type::TypeParameter { param, .. } => {
            let b = inf.table.param(param).bound;
            if b == t {
                return false;
            }
            incompatible_with_await(inf, b, prof + 1)
        }
        _ => false,
    }
}

/// `getErrorNode` de `checkForAssignableExpressionAtType`
/// (an611:src/generated/error_detection_helpers.dart:83-91): desce
/// parênteses e o alvo de cascata.
pub(crate) fn sem_parenteses(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> ExprId {
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
pub(crate) fn verify_assignable_expr(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, of: TypeId, for_: TypeId, template: &str) {
    verify_assignable_expr_at(inf, cx, e, of, for_, template, true);
}

/// Como [`verificar_atribuivel_expr`]; `desembrulhar` aplica o
/// `getErrorNode` (falso no lado direito de atribuição, que o analyzer
/// relata como escrito: `assignment_expression_resolver.dart:148-154`).
pub(crate) fn verify_assignable_expr_at(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, of: TypeId, for_: TypeId, template: &str, unwrap: bool) {
    if !matches!(inf.table.get(for_), Type::Void) && void_use(inf, cx, e, of) {
        return;
    }
    if inf.atribuivel(of, for_) {
        return;
    }
    // Record de um campo posicional com `(x)` sem vírgula
    // (an611:src/generated/error_detection_helpers.dart:93-108): no parêntese.
    if let Type::Record { positional, named, .. } = inf.table.get(for_).clone()
        && positional.len() == 1
        && named.is_empty()
        && !matches!(inf.table.get(of), Type::Record { .. })
        && matches!(ast(inf, cx).expr(e).kind, ExprKind::Parenthesized(_))
        && inf.atribuivel(positional[0], of)
    {
        let sp = inf.span_expr(cx.unit, e);
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA, sp, &[]);
        return;
    }
    let no = if unwrap { sem_parenteses(inf, cx, e) } else { e };
    let sp = inf.span_expr(cx.unit, no);
    // `_insertImplicitCallReference` (`resolver.dart:4085-4142`): a
    // expressão virou a referência implícita ao `call`, cujo tipo é o do
    // método instanciado pelo contexto; é ele que a mensagem mostra.
    let the_implicit = {
        let table = &inf.body_types.units[cx.unit.0 as usize].chamadas_implicitas;
        table.contains(&e) || table.contains(&no)
    };
    let of = match the_implicit.then(|| inf.implicit_call_type(of, for_)).flatten() {
        Some(f) => inf.instantiate_function_from_context(f, for_),
        None => of,
    };
    // `checkForAssignableExpressionAtType`: o erro leva o why-not-promoted
    // da expressão (`computeWhyNotPromotedMessages(expression, …)`).
    let since = inf.diagnostics.len();
    let entity = inf.span_expr(cx.unit, e);
    if template == ARGUMENT_TYPE_NOT_ASSIGNABLE.template {
        // `{2}`: só entre records (`error_detection_helpers.dart:109-139`).
        let mut info: Vec<String> = Vec::new();
        if let (Type::Record { positional: pa, named: at, .. }, Type::Record { positional: pe, named: ne, .. }) =
            (inf.table.get(of).clone(), inf.table.get(for_).clone())
        {
            if !pe.is_empty() && pa.len() != pe.len() {
                info.push(format!("Expected {} positional arguments, but got {} instead.", pe.len(), pa.len()));
            }
            if !ne.is_empty() && at.len() != ne.len() {
                info.push(format!("Expected {} named arguments, but got {} instead.", ne.len(), at.len()));
            }
            if !ne.is_empty() {
                let mut named_args: Vec<(String, TypeId)> = at.iter().map(|(n, t)| (inf.interner.resolve(*n).to_string(), *t)).collect();
                named_args.sort_by(|a, b| a.0.cmp(&b.0));
                for (the_name, t) in named_args {
                    if !ne.iter().any(|(n2, t2)| inf.interner.resolve(*n2) == the_name && *t2 == t) {
                        let tt = inf.table.format_sem_alias(t, inf.interner, inf.program);
                        info.push(format!("Unexpected named argument `{the_name}` with type `{tt}`."));
                    }
                }
            }
        }
        let a3 = info.join(" ");
        inf.warning_with_args(
            dartforge_diagnostics::codigos::compile_time_error::ARGUMENT_TYPE_NOT_ASSIGNABLE,
            sp,
            &[crate::exibicao::Arg::Tipo(of), crate::exibicao::Arg::Tipo(for_), crate::exibicao::Arg::Texto(a3.into())],
        );
        inf.attach_non_promotion(since, cx, Some(e), entity);
        return;
    }
    inf.verify_assignable(of, for_, sp, template);
    inf.attach_non_promotion(since, cx, Some(e), entity);
}

/// Um operando que fecha um bloco básico (o `flowEnd` do
/// `NullSafetyDeadCodeVerifier`, `an611:src/error/dead_code_verifier.dart:213-330`):
/// os ramos de `?:` e o lado direito de `&&`/`||`. Se ele começa
/// inalcançável e não há trecho morto aberto, ele é o primeiro nó morto e o
/// trecho vai dele (ou do operador, `inicio`, quando o pai é binário) ao fim
/// dele; os nós de dentro fazem parte do mesmo trecho.
fn flow_operand<R>(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    start: Option<usize>,
    f: impl FnOnce(&mut BodyInferrer<'_>, &mut Body) -> R,
) -> R {
    let sp = inf.span_expr(cx.unit, e);
    let open = !cx.flow.alcancavel && cx.dead_code.is_none();
    // `f()` com `f` local de tipo função: o analyzer troca o nó
    // (`MethodInvocation` → `FunctionExpressionInvocation`), e o `flowEnd`
    // deste operando não o encontra mais (`_containsFirstDeadNode`): o
    // trecho só fecha no fim do bloco básico de fora.
    // `p.y` com `p` local de tipo record: o `PrefixedIdentifierResolver`
    // troca o nó por um `PropertyAccess`, com o mesmo efeito.
    let rewritten = open && (local_call(inf, cx, e) || local_record_property(inf, cx, e)) && !cx.flow_ends.is_empty();
    if open {
        // Com pai binário, o `flowEnd` que fecha o trecho troca o nó pelo
        // `rightOperand` (o novo): o fim é o do operando de qualquer forma.
        let end = if rewritten && start.is_none() { cx.flow_ends.last().copied().unwrap_or(sp.end).max(sp.end) } else { sp.end };
        let span = dartforge_diagnostics::Span { start: start.unwrap_or(sp.start), end: end };
        inf.aviso(DEAD_CODE.template.to_string(), span);
        if rewritten {
            cx.dead_code = Some(cx.flow_ends.len());
        }
    }
    super::statements::enter_flow(cx, sp.end);
    if open && !rewritten {
        cx.dead_code = Some(cx.flow_ends.len());
    }
    let r = f(inf, cx);
    super::statements::exit_flow(cx);
    r
}

/// A primeira seção de uma cascata `c?..s` é um acesso que o analyzer não
/// reescreve: propriedade, índice, ou invocação de **método**. `..g()` com
/// `g` getter (ou campo) vira invocação de expressão, e o `?..` sai sem
/// relato.
fn first_section_without_rewrite(inf: &mut BodyInferrer<'_>, cx: &Body, section: ExprId, t: TypeId) -> bool {
    let a = ast(inf, cx);
    // Desce até o acesso aplicado ao alvo da cascata, lembrando o pai.
    let mut parent: Option<ExprId> = None;
    let mut x = section;
    loop {
        let tgt = match &a.expr(x).kind {
            ExprKind::Call { target, .. } | ExprKind::Property { target, .. } | ExprKind::Index { target, .. } => *target,
            ExprKind::Assign { target, .. } => *target,
            _ => return true,
        };
        if matches!(a.expr(tgt).kind, ExprKind::CascadeTarget) {
            break;
        }
        parent = Some(x);
        x = tgt;
    }
    let ExprKind::Property { name, .. } = &a.expr(x).kind else { return true };
    let invoked = parent.is_some_and(|p| matches!(&a.expr(p).kind, ExprKind::Call { target, .. } if *target == x));
    if !invoked {
        return true;
    }
    let the_name = name.sym;
    let nn = inf.non_null(t);
    !matches!(inf.lookup_member(cx.lib, nn, the_name, false), Busca::Achado(m) if !m.metodo)
}

/// `e` é `p.y` (um `PrefixedIdentifier`) com `p` local cujo tipo, pelo
/// limite, é um record (`prefixed_identifier_resolver.dart:31-46`).
fn local_record_property(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let a = ast(inf, cx);
    let ExprKind::Property { target, null_aware: false, .. } = &a.expr(e).kind else { return false };
    let ExprKind::Identifier(n) = &a.expr(*target).kind else { return false };
    let Some(super::body::Nome::Local(id)) = cx.buscar(n.sym) else { return false };
    let t = cx.flow.current_type(id, cx.local(id).tipo);
    let t = inf.resolver_ao_limite(t);
    matches!(inf.table.get(t), Type::Record { .. })
}

/// `e` é `f(...)` com `f` uma variável local (não função local).
fn local_call(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let a = ast(inf, cx);
    let ExprKind::Call { target, .. } = &a.expr(e).kind else { return false };
    let ExprKind::Identifier(n) = &a.expr(*target).kind else { return false };
    match cx.buscar(n.sym) {
        Some(super::body::Nome::Local(id)) => !cx.funcoes_locais.contains(&id),
        _ => false,
    }
}

/// `RECEIVER_OF_TYPE_NEVER` (`binary_expression_resolver.dart:422-428`,
/// `method_invocation_resolver.dart:543`, `property_element_resolver.dart:92`,
/// `function_expression_invocation_resolver.dart:66`): o receptor de um
/// operador, índice, método ou invocação tem tipo `Never` (não `Never?`).
/// Relata no receptor e diz se relatou.
pub(crate) fn never_receiver(inf: &mut BodyInferrer<'_>, cx: &Body, r: ExprId, t: TypeId) -> bool {
    if !matches!(inf.table.get(t), Type::Never) || matches!(ast(inf, cx).expr(r).kind, ExprKind::Super) {
        return false;
    }
    // Numa seção de cascata, o receptor é o alvo da cascata (`realTarget`),
    // com o tipo estático dele: o `?..` não promove aqui (`null?..m()` tem
    // receptor `Null`, não `Never`; o não nulo fica para o
    // `TypePropertyResolver`).
    let r = if matches!(ast(inf, cx).expr(r).kind, ExprKind::CascadeTarget) {
        let tgt = cx.cascade_targets.last().copied().unwrap_or(r);
        let raw = inf.body_types.units[cx.unit.0 as usize].get_type(tgt);
        if !raw.is_some_and(|b| matches!(inf.table.get(b), Type::Never)) {
            return false;
        }
        tgt
    } else {
        r
    };
    let sp = inf.span_expr(cx.unit, r);
    inf.warning_with_code(dartforge_diagnostics::codigos::warning::RECEIVER_OF_TYPE_NEVER, sp, &[]);
    true
}

/// `RecordLiteralResolver._resolveField` (3.6.2 `record_literal_resolver.dart:135-148`):
/// o campo `dynamic` com contexto conhecido vale o fecho maior do contexto,
/// quando `dynamic` não é subtipo dele (o cast implícito).
fn dynamic_cast_in_field(inf: &mut BodyInferrer<'_>, t: TypeId, context: TypeId) -> TypeId {
    if context == inf.core.unknown || !matches!(inf.table.get(t), Type::Dynamic) {
        return t;
    }
    let closure = inf.greatest_closure_of(context);
    if inf.sub(t, closure) { t } else { closure }
}

/// `DEAD_NULL_AWARE_EXPRESSION` (`_checkForDeadNullCoalesce`,
/// `an611:src/generated/error_verifier.dart:3045-3052`): em `a ?? b` e
/// `a ??= b`, com `a` estritamente não anulável, o lado direito.
fn warn_dead_null(inf: &mut BodyInferrer<'_>, cx: &Body, the_left: TypeId, the_right: ExprId) {
    if strictly_non_nullable(inf, the_left) {
        let sp = inf.span_expr(cx.unit, the_right);
        inf.warning_with_code(dartforge_diagnostics::codigos::static_warning::DEAD_NULL_AWARE_EXPRESSION, sp, &[]);
    }
}

/// `t` é estritamente não anulável (`isStrictlyNonNullable`): nem `dynamic`,
/// `void`, `Null` ou anulável; parâmetro de tipo pelo limite; tipo de
/// extensão só com `implements` (aqui, nunca: pelo lado seguro).
pub(crate) fn strictly_non_nullable(inf: &mut BodyInferrer<'_>, t: TypeId) -> bool {
    // `TypeSystemImpl.isStrictlyNonNullable` (3.6.2 `type_system.dart:1290-1310`).
    let mut t = t;
    for _ in 0..64 {
        if inf.table.e_invalido(t) || inf.the_is_unknown(t) {
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
fn unnecessary_null_operator(inf: &mut BodyInferrer<'_>, cx: &Body, r: ExprId, t: TypeId) {
    if matches!(ast(inf, cx).expr(r).kind, ExprKind::Super) || !strictly_non_nullable(inf, t) {
        return;
    }
    report_null_operator(inf, cx, r, true);
}

/// `C?.x`, `E?.m()`, `Alias?.x`: o receptor é um literal de tipo (sem tipo
/// estático no analyzer, `targetElement` de classe, extensão ou alias,
/// `error_verifier.dart:5613-5627`): o operador é desnecessário.
pub(crate) fn null_operator_on_type(inf: &mut BodyInferrer<'_>, cx: &Body, r: ExprId) {
    let simple = match &ast(inf, cx).expr(r).kind {
        ExprKind::Identifier(_) => true,
        ExprKind::Property { target, .. } => matches!(ast(inf, cx).expr(*target).kind, ExprKind::Identifier(_)),
        _ => false,
    };
    if simple {
        report_null_operator(inf, cx, r, false);
    }
}

/// O `INVALID_NULL_AWARE_OPERATOR` no operador que segue `r` (`?.`, `?..`
/// ou `?[`); `cadeia`: com um `?.` anterior na mesma cadeia, é o
/// `…_AFTER_SHORT_CIRCUIT`.
fn report_null_operator(inf: &mut BodyInferrer<'_>, cx: &Body, r: ExprId, chain: bool) {
    use dartforge_diagnostics::codigos::static_warning as w;
    let end = inf.span_expr(cx.unit, r).end;
    let source = &inf.program.unit(cx.unit).source;
    let rest = source.get(end..).unwrap_or("");
    let pos = end + skip_whitespace_and_comments(rest);
    let after = source.get(pos..).unwrap_or("");
    let (size, args): (usize, [&str; 2]) = if after.starts_with("?..") {
        (3, ["?..", ".."])
    } else if after.starts_with("?.") {
        (2, ["?.", "."])
    } else if after.starts_with('?') {
        // `?[`: do `?` ao `[` (com o que houver entre eles).
        let Some(i) = after.find('[') else { return };
        (i + 1, ["?[", "["])
    } else {
        return;
    };
    let sp = dartforge_diagnostics::Span { start: pos, end: pos + size };
    // `invalidNullAwareAfterShortCircuit`: o contexto é o operador que
    // encurta.
    match (chain && args[0] != "?..").then(|| previous_short_circuit_operator(inf, cx, r)).flatten() {
        Some((previous, lexeme)) => inf.warning_with_context(
            w::INVALID_NULL_AWARE_OPERATOR_AFTER_SHORT_CIRCUIT,
            sp,
            &args,
            vec![(None, previous, format!("The operator '{lexeme}' is causing the short circuiting."))],
        ),
        None => inf.warning_with_code(w::INVALID_NULL_AWARE_OPERATOR, sp, &args),
    }
}

/// `...?e` com `e` estritamente não anulável: no `...?`.
pub(crate) fn unnecessary_null_spread(inf: &mut BodyInferrer<'_>, cx: &Body, the_value: ExprId, t: TypeId) {
    use dartforge_diagnostics::codigos::static_warning as w;
    if !strictly_non_nullable(inf, t) {
        return;
    }
    let begin = inf.span_expr(cx.unit, the_value).start;
    let source = &inf.program.unit(cx.unit).source;
    let before = source.get(..begin).unwrap_or("").trim_end();
    if let Some(pos) = before.strip_suffix("...?").map(str::len) {
        let sp = dartforge_diagnostics::Span { start: pos, end: pos + 4 };
        inf.warning_with_code(w::INVALID_NULL_AWARE_OPERATOR, sp, &["...?", "..."]);
    }
}

/// `previousShortCircuitingOperator` (`error_verifier.dart:5583-5605`): o
/// operador `?.`/`?` (do `?[`) de `r` que encurta a cadeia, o mais fundo
/// primeiro; só pelos acessos que são eles mesmos `?.`/`?[` (um `.` no meio
/// encerra a busca). O intervalo e o lexema do token.
fn previous_short_circuit_operator(inf: &BodyInferrer<'_>, cx: &Body, r: ExprId) -> Option<(dartforge_diagnostics::Span, &'static str)> {
    let source = &inf.program.unit(cx.unit).source;
    let token_after = |tgt: ExprId, lexeme: &'static str| -> Option<(dartforge_diagnostics::Span, &'static str)> {
        let end = inf.span_expr(cx.unit, tgt).end;
        let pos = end + skip_whitespace_and_comments(source.get(end..).unwrap_or(""));
        source.get(pos..)?.starts_with(lexeme).then_some((dartforge_diagnostics::Span { start: pos, end: pos + lexeme.len() }, lexeme))
    };
    match &ast(inf, cx).expr(r).kind {
        ExprKind::Property { target, null_aware: true, .. } => previous_short_circuit_operator(inf, cx, *target).or_else(|| token_after(*target, "?.")),
        ExprKind::Index { target, null_aware: true, .. } => previous_short_circuit_operator(inf, cx, *target).or_else(|| token_after(*target, "?")),
        ExprKind::Call { target, .. } => match &ast(inf, cx).expr(*target).kind {
            ExprKind::Property { target: t2, null_aware: true, .. } => previous_short_circuit_operator(inf, cx, *t2).or_else(|| token_after(*t2, "?.")),
            _ => None,
        },
        _ => None,
    }
}

/// Infere o receptor de um acesso (`r.x`, `r[i]`, `r.m()`), tratando `?.` e
/// o *null-shorting*. Devolve o tipo do receptor para a busca e se há curto.
pub(crate) fn receptor(inf: &mut BodyInferrer<'_>, cx: &mut Body, r: ExprId, null_aware: bool) -> (TypeId, bool) {
    let u = inf.core.unknown;
    let (t, c) = infer_at(inf, cx, r, u, true);
    if null_aware {
        unnecessary_null_operator(inf, cx, r, t);
        // Promove o receptor dentro da cadeia; `fechar_cadeia` desfaz no fim.
        cx.cadeias.push(cx.flow.clone());
        if let Some(id) = promotion_target(inf, cx, r) {
            let decl = cx.local(id).tipo;
            let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
            inf.promote_non_null(&mut f, id, decl);
            cx.flow = f;
        }
        // `promoteToNonNull` do alvo (o parâmetro de tipo de limite anulável
        // vira `T & NonNull(limite)`: `nn?.toRadixString(16)` com
        // `NN extends int?`).
        let nn = match inf.table.get(t) {
            Type::TypeParameter { nullable: false, .. } => inf.non_null_promotion(t),
            _ => inf.non_null(t),
        };
        (nn, true)
    } else {
        (t, c)
    }
}

/// Busca de membro no receptor `target`: sobreposição explícita de
/// extensão (`E(x).m`) consulta só a extensão; senão a busca normal.
pub(crate) fn lookup_target_member(inf: &mut BodyInferrer<'_>, cx: &Body, target: ExprId, recv: TypeId, the_name: SymbolId, setter: bool) -> Busca {
    if let Some((x, args)) = cx.overlays.get(&target).cloned() {
        return match inf.explicit_extension_member(x, &args, the_name, setter) {
            Some(m) => Busca::Achado(m),
            None => Busca::Missing,
        };
    }
    inf.lookup_member(cx.lib, recv, the_name, setter)
}

/// `target.name` (leitura).
/// `startNullAwarePropertyAccess`/`visitMethodInvocation` (3.6.2,
/// `resolver.dart:1777-1794`, `:3215-3226`): o `?.` sobre um literal de tipo
/// só deixa de encurtar quando o alvo é um `SimpleIdentifier` que nomeia um
/// `InterfaceElement` (`C?.x` é `C.x`); `p.C?.x`, `C<int>?.x`, um alias ou
/// uma extensão encurtam (o resultado fica anulável).
pub(crate) fn shortening_waived(inf: &BodyInferrer<'_>, cx: &Body, target: ExprId, rt: &TypeRef) -> bool {
    matches!(ast(inf, cx).expr(target).kind, ExprKind::Identifier(_)) && matches!(rt, TypeRef::Classe(..))
}

/// `PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT` no nome do prefixo.
pub(crate) fn prefix_without_dot(inf: &mut BodyInferrer<'_>, n: ast::Name) {
    let the_name = inf.interner.resolve(n.sym).to_string();
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT, n.span, &[&the_name]);
}

fn propriedade(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, target: ExprId, name: ast::Name, null_aware: bool) -> (TypeId, bool) {
    let a = ast(inf, cx);
    // `p.nome` com prefixo de import.
    if let ExprKind::Identifier(p) = &a.expr(target).kind {
        if matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefixo) {
            // `p?.x` é `PropertyAccess`, não `PrefixedIdentifier`: o prefixo
            // não vale ali (`_isValidAsPrefix`).
            if null_aware {
                prefix_without_dot(inf, *p);
            }
            resolver(inf, cx, target, Resolved::Prefix(cx.lib));
            if ambiguous_import(inf, cx, Some(p.sym), name) {
                return (inf.table.invalid(inf.core.dynamic_), false);
            }
            match inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter.or(b.setter)) {
                Some(el) => {
                    resolver(inf, cx, e, Resolved::Element(el));
                    if let Element::Extension(_) = el {
                        let sp = inf.span_expr(cx.unit, e);
                        if extension_as_expression(inf, cx, e, sp) {
                            return (inf.core.dynamic_, false);
                        }
                    }
                    return (ler_elemento(inf, el), false);
                }
                None => {
                    if let Some(t) = load_library(inf, cx, p.sym, name.sym) {
                        return (t, false);
                    }
                    warn_undefined_prefixed_name(inf, cx, p.sym, name);
                    return (inf.core.dynamic_, false);
                }
            }
        }
    }
    // `Fn<int>.foo` com `Fn` alias de tipo de função (3.6.2
    // `property_element_resolver.dart:459-477`): nenhuma resolução possível,
    // `UNDEFINED_GETTER_ON_FUNCTION_TYPE` no nome.
    if function_type_literal(inf, cx, target, name, true) {
        return (inf.table.invalid(inf.core.dynamic_), false);
    }
    // `C.x`: estático, constante de enum ou tear-off de construtor.
    if let Some(rt) = type_reference(inf, cx, target) {
        record_type_ref(inf, cx, target);
        if null_aware {
            null_operator_on_type(inf, cx, target);
        }
        let short_circuit = null_aware && !shortening_waived(inf, cx, target, &rt);
        return (acesso_estatico(inf, cx, e, rt, name), short_circuit);
    }
    // `super.x`. Com `super?.x` (`INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER`
    // no parser), o acesso continua null-aware: o tipo sai anulável no fim
    // da cadeia (`int y = super?.x` é `INVALID_ASSIGNMENT` de `int?`).
    if matches!(a.expr(target).kind, ExprKind::Super) {
        let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
        registrar(inf, cx, target, this);
        return (super_member(inf, cx, e, name, SuperUse::Leitura), null_aware);
    }
    let (recv, short_circuit) = receptor(inf, cx, target, null_aware);
    // `x.new` com `x` valor: o `TypePropertyResolver` nunca acha `new`
    // (`type_property_resolver.dart:75-79`), nem em `dynamic` nem no anulável.
    if !cx.overlays.contains_key(&target) && inf.interner.resolve(name.sym) == "new" {
        let typ = inf.table.format(recv, inf.interner, inf.program);
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_GETTER, name.span, &["new", &typ]);
        return (inf.table.invalid(inf.core.dynamic_), short_circuit);
    }
    if let Some((x, args)) = cx.overlays.get(&target).cloned() {
        if inf.explicit_extension_member(x, &args, name.sym, false).is_none()
            && inf.extension_static_member(x, name.sym, false).is_some()
        {
            inf.aviso(EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(), name.span);
            return (inf.core.dynamic_, short_circuit);
        }
    }
    let base = if null_aware { None } else { property_base(inf, cx, target) };
    // `void`: o valor não pode ser usado (analyzer: `use_of_void_result`, no nome).
    // O alvo `E(x)` não: o membro vem da extensão, e o `void` do argumento
    // já saiu no argumento (`extension_member_resolver.dart:227-232`).
    if !cx.overlays.contains_key(&target) && matches!(inf.table.get(recv), Type::Void) {
        // Em `x..p`, o relato é no alvo da cascata (feito lá).
        if !matches!(a.expr(target).kind, ExprKind::CascadeTarget) {
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, name.span, &[]);
        }
        return (inf.core.dynamic_, short_circuit);
    }
    // Receptor potencialmente anulável sem o membro em `Object` nem numa
    // extensão: `unchecked_use_of_nullable_value` no nome, e a leitura segue
    // pelo tipo não anulável só para recuperar o tipo (sem `undefined_getter`).
    let check_null = !cx.overlays.contains_key(&target) && inf.requires_null_check(cx.lib, recv, name.sym, false);
    if check_null {
        let the_name = inf.interner.resolve(name.sym).to_string();
        let since = inf.diagnostics.len();
        inf.null_warning(
            recv,
            dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
            name.span,
            &[&the_name],
        );
        // `computeWhyNotPromotedMessages(nameErrorEntity, whyNotPromoted(receiver))`.
        inf.attach_non_promotion(since, cx, Some(target), name.span);
    }
    // `f.call` lido com `f` do tipo `Function` de `dart:core`: o
    // `functionTypeCallType` é o próprio tipo (`property_element_resolver.dart:
    // 440-445`); só a invocação `f.call()` sai dinâmica.
    if !check_null
        && Some(name.sym) == inf.sym.call
        && matches!(inf.table.get(recv), Type::Interface { class, nullable: false, .. } if Some(*class) == inf.core.function_class)
    {
        return (recv, short_circuit);
    }
    let search = lookup_target_member(inf, cx, target, recv, name.sym, false);
    inf.report_extension_ambiguity(name.span);
    let t = match search {
        Busca::Achado(m) => {
            resolver(inf, cx, e, m.resolved.clone());
            match base {
                Some(b) => field_read(inf, cx, e, b, m.tipo),
                None => m.tipo,
            }
        }
        Busca::Missing if check_null => match inf.instance_access_to_static(cx.lib, recv, name.sym, false, name.span) {
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
            match inf.interface_member(o, name.sym, false) {
                Some(m) => m.tipo,
                None => inf.core.dynamic_,
            }
        }
        // Receptor `Never` (analyzer 3.6.2): na forma `id.x`
        // (`PrefixedIdentifier`) o resultado é `Never`; nas outras
        // (`PropertyAccess`, `falha().x`) valem só os membros de `Object`, e
        // um nome que não é de `Object` fica com o tipo de recuperação
        // (`InvalidType`, que se comporta como `dynamic`).
        Busca::NeverType => {
            let prefixed = !null_aware && matches!(a.expr(target).kind, ExprKind::Identifier(_));
            let o = inf.core.object;
            match inf.interface_member(o, name.sym, false) {
                _ if prefixed => inf.core.never,
                Some(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    m.tipo
                }
                None => {
                    inf.body_types.units[cx.unit.0 as usize].invalid_types.insert(e);
                    inf.table.invalid(inf.core.dynamic_)
                }
            }
        }
        Busca::Missing => {
            if let Some((x, _)) = cx.overlays.get(&target).cloned() {
                let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_GETTER.template, inf.interner.resolve(name.sym), extension);
                inf.aviso(msg, name.span);
                return (inf.core.dynamic_, short_circuit);
            }
            if let Some(m) = inf.instance_access_to_static(cx.lib, recv, name.sym, false, name.span) {
                resolver(inf, cx, e, m.resolved.clone());
                return (m.tipo, short_circuit);
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
            inf.table.invalid(inf.core.dynamic_)
        }
    };
    (t, short_circuit)
}

/// `WRONG_NUMBER_OF_TYPE_ARGUMENTS` de uma classe ou alias instanciado
/// como expressão (`e` é o `TypeArguments`). Como literal de tipo
/// (`_resolveDirectTypeLiteral`/`_resolveTypeAlias`,
/// `function_reference_resolver.dart:115-150, 316-327, 821-835`), na lista
/// de argumentos; com `no_tipo_nomeado` (o `NamedType` de `C<T>.x` e da
/// criação implícita, `NamedTypeResolver._buildTypeArguments`), do nome ao
/// `>`. O nome é o escrito (o do alias, se for um).
pub(crate) fn check_literal_arguments(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, in_named_type: bool) {
    let a = ast(inf, cx);
    let ExprKind::TypeArguments { target, type_args } = &a.expr(e).kind else { return };
    let (target, n_args) = (*target, type_args.len());
    let last_end = type_args.last().map(|&t| a.ty(t).span.end);
    let (el, the_name) = match &a.expr(target).kind {
        ExprKind::Identifier(n) => match resolve_name(inf, cx, n.sym, false) {
            NameRef::Elemento(el) => (el, *n),
            _ => return,
        },
        ExprKind::Property { target: p, name, null_aware: false } => {
            let ExprKind::Identifier(p) = &a.expr(*p).kind else { return };
            if !matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefixo) {
                return;
            }
            let Some(el) = inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter) else { return };
            (el, *name)
        }
        _ => return,
    };
    let parameters = match el {
        Element::Class(c) => inf.outline.classes[c.0 as usize].type_params.len(),
        Element::Typedef(td) => inf.outline.typedefs[td.0 as usize].type_params.len(),
        _ => return,
    };
    if parameters == n_args {
        return;
    }
    let source = &inf.program.unit(cx.unit).source;
    let tgt = a.expr(target).span;
    let after_name = tgt.end;
    let open = source.get(after_name..).and_then(|r| r.find('<')).map(|i| after_name + i).unwrap_or(after_name);
    let args_end = last_end.unwrap_or(open + 1);
    let closes = source.get(args_end..).and_then(|r| r.find('>')).map(|i| args_end + i + 1).unwrap_or(args_end);
    let span = if in_named_type {
        dartforge_diagnostics::Span { start: tgt.start, end: closes }
    } else {
        dartforge_diagnostics::Span { start: open, end: closes }
    };
    let (text, p, n) = (inf.interner.resolve(the_name.sym).to_string(), parameters.to_string(), n_args.to_string());
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS, span, &[&text, &p, &n]);
}

/// Acesso estático `C.x` / `E.x` / `C.new`.
fn acesso_estatico(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, rt: TypeRef, name: ast::Name) -> TypeId {
    let explicit_instance = explicit_instantiation_receiver(inf, cx, e);
    if explicit_instance && let ExprKind::Property { target, .. } = &ast(inf, cx).expr(e).kind {
        check_literal_arguments(inf, cx, *target, true);
    }
    match rt {
        TypeRef::Extensao(x) => match inf.extension_static_member(x, name.sym, false) {
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
                let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_GETTER.template, inf.interner.resolve(name.sym), extension);
                inf.aviso(msg, name.span);
                inf.core.dynamic_
            }
        },
        TypeRef::Classe(c, targs) => {
            // O nome privado de outra biblioteca não é achado (`Enum2._A` fora
            // da biblioteca do enum: `undefined_enum_constant`).
            let the_visible = !inf.interner.resolve(name.sym).starts_with('_') || inf.program.class(c).library == cx.lib;
            if let Some(m) = inf.static_member(c, name.sym, false).filter(|_| the_visible) {
                if explicit_instance {
                    warn_static_instantiation(inf, cx, e, name);
                    return inf.core.dynamic_;
                }
                resolver(inf, cx, e, m.resolved.clone());
                return m.tipo;
            }
            let args = targs.map(|v| v.iter().map(|&t| inf.type_argument_type(cx, t)).collect::<Vec<_>>());
            tearoff_de_construtor(inf, cx, e, c, args, name, true, false)
        }
        TypeRef::Alias(c, args, td) => {
            let the_visible = !inf.interner.resolve(name.sym).starts_with('_') || inf.program.class(c).library == cx.lib;
            if let Some(m) = inf.static_member(c, name.sym, false).filter(|_| the_visible) {
                if explicit_instance {
                    warn_static_instantiation(inf, cx, e, name);
                    return inf.core.dynamic_;
                }
                resolver(inf, cx, e, m.resolved.clone());
                return m.tipo;
            }
            if !explicit_instance && args.is_some() && !inf.outline.typedefs[td.0 as usize].type_params.is_empty() {
                return generic_alias_tearoff(inf, cx, e, c, td, name);
            }
            // `X<T>.m` instanciado explicitamente: o mesmo
            // `CLASS_INSTANTIATION_ACCESS_TO_MEMBER` da classe.
            tearoff_de_construtor(inf, cx, e, c, args, name, explicit_instance, true)
        }
    }
}

fn explicit_instantiation_receiver(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    let ExprKind::Property { target, .. } = &ast(inf, cx).expr(e).kind else { return false };
    matches!(&ast(inf, cx).expr(*target).kind, ExprKind::TypeArguments { .. })
}

/// Somente membros declarados na própria classe; a busca herdada requer
/// resolver substituições e precedência antes de diagnosticar.
fn visible_direct_instance_member(inf: &BodyInferrer<'_>, cx: &Body, cls: ClassId, name: ast::Name, write: bool) -> bool {
    if inf.interner.resolve(name.sym).starts_with('_') && inf.program.class(cls).library != cx.lib {
        return false;
    }
    let members = &inf.program.class(cls).instance_members;
    members.contains_key(&name.sym)
        || (write && inf.setter_key(name.sym).is_some_and(|the_key| members.contains_key(&the_key)))
}

fn warn_static_access_to_instance(inf: &mut BodyInferrer<'_>, cx: &Body, cls: ClassId, name: ast::Name, write: bool) -> bool {
    let found = visible_direct_instance_member(inf, cx, cls, name, write);
    if found {
        let msg = format!("{}: '{}'", STATIC_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
        inf.aviso(msg, name.span);
    }
    found
}

fn warn_class_instantiation(inf: &mut BodyInferrer<'_>, cx: &Body, expr: ExprId, cls: ClassId, name: ast::Name, write: bool) -> bool {
    if !visible_direct_instance_member(inf, cx, cls, name, write) { return false; }
    let msg = format!("{}: '{}'", CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
    let span = ast(inf, cx).expr(expr).span;
    inf.aviso(msg, span);
    true
}

fn warn_static_instantiation(inf: &mut BodyInferrer<'_>, cx: &Body, expr: ExprId, name: ast::Name) {
    let msg = format!("{}: '{}'", CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER.template, inf.interner.resolve(name.sym));
    let span = ast(inf, cx).expr(expr).span;
    inf.aviso(msg, span);
}

fn warn_unknown_instantiation(inf: &mut BodyInferrer<'_>, cx: &Body, expr: ExprId, cls: ClassId, name: ast::Name) {
    // O identificador sintético da recuperação (`A<int>.;`) não é resolvido.
    if name.span.start == name.span.end {
        return;
    }
    let cls = inf.interner.resolve(inf.program.class(cls).name);
    let mem = inf.interner.resolve(name.sym);
    let msg = format!("{}: '{cls}', '{mem}'", CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER.template);
    let span = ast(inf, cx).expr(expr).span;
    inf.aviso(msg, span);
}

/// `C.nome` / `C.new` como valor: tipo de função do construtor (genérico
/// sobre os parâmetros da classe se não instanciado).
/// `de_alias`: o receptor é um alias de tipo; instanciado (`X<T>.new`), o
/// `AstRewriter` não o reescreve em referência de construtor, e o `new`
/// ausente é membro desconhecido da instanciação como qualquer outro nome.
#[allow(clippy::too_many_arguments)]
fn tearoff_de_construtor(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, c: ClassId, args: Option<Vec<TypeId>>, name: ast::Name, diagnose_absence: bool, of_alias: bool) -> TypeId {
    let explicit_instance = explicit_instantiation_receiver(inf, cx, e);
    let the_key = if Some(name.sym) == inf.sym.new_ { inf.sym.vazio } else { Some(name.sym) };
    let Some(the_key) = the_key else { return inf.core.dynamic_ };
    // O primário de um tipo de extensão (`E.new`, `E.nome`) não é elemento:
    // `Some(None)`, com a assinatura `(Representação) -> E`.
    let primary = inf.constructor_or_primary(c, the_key) == Some(None);
    let constructor = inf.construtor_de(c, the_key);
    // Construtores geradores de enum só criam as constantes do próprio enum.
    // O sem nome implícito não aparece na tabela, mas `E.new` também é sua
    // referência. Factories declaradas no enum podem ser referenciadas.
    if inf.program.class(c).kind == dartforge_elements::model::ClassKind::Enum
        && (constructor.is_some_and(|f| !inf.program.function(f).factory)
            || (constructor.is_none() && Some(name.sym) == inf.sym.new_))
    {
        inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), ast(inf, cx).expr(e).span);
        return inf.core.dynamic_;
    }
    // `ConstructorReferenceResolver.resolve`
    // (`an611:src/dart/resolver/constructor_reference_resolver.dart:31-41`):
    // gerador (também o sintético sem nome, que o modelo não cria) de classe
    // abstrata, no `ConstructorReference` inteiro.
    let abstract_ = {
        let ce = inf.program.class(c);
        matches!(ce.kind, dartforge_elements::model::ClassKind::Class | dartforge_elements::model::ClassKind::MixinApplication) && ce.modifiers.abstract_
    };
    let synthetic = constructor.is_none() && !primary && Some(the_key) == inf.sym.vazio && super::functions::without_implicit_name(inf, c);
    if abstract_ && (synthetic || constructor.is_some_and(|f| !inf.program.function(f).factory)) {
        let sp = ast(inf, cx).expr(e).span;
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::TEAROFF_OF_GENERATIVE_CONSTRUCTOR_OF_ABSTRACT_CLASS, sp, &[]);
    }
    if synthetic {
        let ret = inf.class_this_type(c);
        let sig = inf.table.intern(Type::Function {
            type_params: Box::new([]),
            ret,
            positional: Box::new([]),
            optional: Box::new([]),
            named: Box::new([]),
            nullable: false,
        });
        return tearoff_with_arguments(inf, c, sig, args);
    }
    let Some(f) = constructor else {
        if primary {
            let sig = inf.primary_signature(c);
            return tearoff_with_arguments(inf, c, sig, args);
        }
        // `C<T>.new` ainda é uma referência ao construtor sem nome. A
        // ausência dele tem diagnóstico no token `new`, mesmo quando o
        // receptor foi instanciado explicitamente.
        if diagnose_absence && Some(name.sym) == inf.sym.new_ && !(of_alias && explicit_instance) {
            let cls = inf.interner.resolve(inf.program.class(c).name);
            let msg = if explicit_instance {
                format!("{}: '{cls}', 'new'", NEW_WITH_UNDEFINED_CONSTRUCTOR.template)
            } else {
                format!("{}: '{cls}'", NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT.template)
            };
            inf.aviso(msg, name.span);
            return inf.core.dynamic_;
        }
        if explicit_instance && warn_class_instantiation(inf, cx, e, c, name, false) {
            return inf.core.dynamic_;
        }
        if explicit_instance && diagnose_absence {
            warn_unknown_instantiation(inf, cx, e, c, name);
            return inf.core.dynamic_;
        }
        if !explicit_instance && warn_static_access_to_instance(inf, cx, c, name, false) {
            return inf.core.dynamic_;
        }
        // `PropertyElementResolver` (`property_element_resolver.dart:696-705`):
        // num enum o código é `UNDEFINED_ENUM_CONSTANT`.
        if inf.program.class(c).kind == dartforge_elements::model::ClassKind::Enum {
            let (n, en) = (inf.interner.resolve(name.sym).to_string(), inf.interner.resolve(inf.program.class(c).name).to_string());
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_ENUM_CONSTANT, name.span, &[&n, &en]);
            return inf.core.dynamic_;
        }
        // `[propertyName.name, typeReference.name]`
        // (`property_element_resolver.dart:697-705`): o nome da classe, também
        // pelo alias.
        let (n, cls) = (inf.interner.resolve(name.sym).to_string(), inf.interner.resolve(inf.program.class(c).name).to_string());
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_GETTER, name.span, &[&n, &cls]);
        return inf.core.dynamic_;
    };
    if inf.program.function(f).class == Some(c) {
        resolver(inf, cx, e, Resolved::Constructor(f));
    }
    let sig = inf.constructor_signature(c, f);
    tearoff_with_arguments(inf, c, sig, args)
}

/// `F.new`/`F.nome` por um alias genérico que não só repassa os parâmetros
/// (`typedef F<T> = C<int, T>`), sem argumentos de tipo: a função genérica
/// nos parâmetros do alias, `C<int, T> Function<T>(…)` (como o analyzer e o
/// CFE), que a instanciação implícita do contexto fecha depois.
fn generic_alias_tearoff(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    e: ExprId,
    c: ClassId,
    td: dartforge_elements::model::TypedefId,
    name: ast::Name,
) -> TypeId {
    let tgt = inf.outline.typedefs[td.0 as usize].target_type;
    let params = inf.outline.typedefs[td.0 as usize].type_params.clone();
    let target_args = match inf.table.get(tgt).clone() {
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args,
        _ => return inf.core.dynamic_,
    };
    let new_: Vec<crate::table::TypeParamId> = params
        .iter()
        .map(|&p| {
            let d = inf.table.param(p).clone();
            inf.table.alloc_type_param(d.name, crate::table::TypeParamOwner::GenericFunctionType, d.bound, d.variance)
        })
        .collect();
    let types: Vec<TypeId> = new_.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
    let map = inf.mapa(&params, &types);
    for (&p, &o) in new_.iter().zip(params.iter()) {
        let b = inf.table.param(p).bound;
        let b = inf.subst(b, &map);
        inf.table.set_type_param_bound(p, b);
        inf.table.param_mut(p).explicito = inf.table.param(o).explicito;
    }
    let args: Vec<TypeId> = target_args.iter().map(|a| inf.subst(*a, &map)).collect();
    let t = tearoff_de_construtor(inf, cx, e, c, Some(args), name, false, true);
    match inf.table.get(t).clone() {
        Type::Function { type_params, ret, positional, optional, named, nullable } if type_params.is_empty() => {
            inf.table.intern(Type::Function { type_params: new_.into_boxed_slice(), ret, positional, optional, named, nullable })
        }
        _ => t,
    }
}

/// O tipo do tearoff de um construtor de `c` com assinatura `sig`: instanciado
/// com `args`, ou genérico sobre parâmetros novos no lugar dos da classe.
fn tearoff_with_arguments(inf: &mut BodyInferrer<'_>, c: ClassId, sig: TypeId, args: Option<Vec<TypeId>>) -> TypeId {
    let params = inf.outline.classes[c.0 as usize].type_params.clone();
    match args {
        Some(args) if args.len() == params.len() => {
            let map = inf.mapa(&params, &args);
            inf.subst(sig, &map)
        }
        _ if params.is_empty() => sig,
        _ => {
            // Genérico: `C<T> Function<T>(...)` com os parâmetros da **própria
            // classe** como formais (`ConstructorElementToInfer.asType`,
            // `invocation_inference_helper.dart:45-54`: `typeElement.typeParameters`,
            // sem cópia). No contexto que menciona os mesmos parâmetros
            // (`[A<T> Function() fn = A.new]` dentro de `A<T>`) o tipo do
            // tear-off e o do contexto são iguais, o `trySubtypeMatch` fecha
            // sem restrição (`P == Q`) e `T` vai a `dynamic`: `A<dynamic> Function()`
            // não é atribuível (oráculo `ConstWithTypeParametersConstructorTearo_9751514d`).
            match inf.table.get(sig).clone() {
                Type::Function { ret, positional, optional, named, nullable, .. } => inf.table.intern(Type::Function {
                    type_params: params.clone(),
                    ret,
                    positional,
                    optional,
                    named,
                    nullable,
                }),
                _ => sig,
            }
        }
    }
}

/// Como `super.nome` é usado: o código do nome que não existe muda.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuperUse {
    Leitura,
    Escrita,
    Invocacao,
}

/// As classes da cadeia de `super` da classe corrente, na ordem da busca:
/// os mixins dela (o último primeiro), a superclasse, os mixins dela… — sem
/// as interfaces (o `getInheritedConcreteMap` do analyzer). Num mixin, as
/// restrições `on` e os supertipos delas.
fn super_chain(inf: &BodyInferrer<'_>, c: ClassId) -> Vec<ClassId> {
    let mut v = Vec::new();
    let cls = inf.program.class(c);
    if cls.kind == dartforge_elements::model::ClassKind::Mixin {
        for &o in &cls.on_classes {
            for (sup, _) in crate::scope::ordered_supertypes(inf.program, &inf.outline.hierarchy, o) {
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
    for &m in cls.mixin_classes.iter().rev() {
        v.push(m);
    }
    let mut current = cls.supertype_class;
    while let Some(k) = current {
        if v.contains(&k) {
            break;
        }
        v.push(k);
        for &m in inf.program.class(k).mixin_classes.iter().rev() {
            if !v.contains(&m) {
                v.push(m);
            }
        }
        current = inf.program.class(k).supertype_class;
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
pub(crate) fn member_reached_through_super(
    inf: &mut BodyInferrer<'_>,
    c: ClassId,
    the_key: SymbolId,
) -> Result<(ClassId, dartforge_elements::model::FunctionElementId), Option<(ClassId, dartforge_elements::model::FunctionElementId)>> {
    let owner = |inf: &BodyInferrer<'_>, m: &crate::heranca::Membro| inf.program.function(m.funcao).class.unwrap_or(m.classe);
    if let Some(m) = inf.inheritance_member(c, the_key, false, true) {
        return Ok((owner(inf, &m), m.funcao));
    }
    match inf.inherited_from_inheritance(c, the_key) {
        Some(m) => Err(Some((owner(inf, &m), m.funcao))),
        None => Err(None),
    }
}

/// `super.nome` (leitura, escrita ou invocação): o membro concreto da cadeia
/// do super; só abstrato → `ABSTRACT_SUPER_MEMBER_REFERENCE` (e o tipo dele);
/// nenhum → `UNDEFINED_SUPER_{GETTER,SETTER,METHOD}`
/// (`an611:src/dart/resolver/property_element_resolver.dart:790-880`,
/// `method_invocation_resolver.dart:744-788`). Membros de `Object` contam
/// (toda cadeia termina nele).
pub(crate) fn super_member(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, name: ast::Name, use_: SuperUse) -> TypeId {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let setter = use_ == SuperUse::Escrita;
    let Some(cls) = cx.classe else { return inf.core.dynamic_ };
    let Some(this) = cx.tipo_this else { return inf.core.dynamic_ };
    // `super` num extension type não é válido (`SuperContext`): outro erro,
    // sem resolução.
    if inf.program.class(cls).kind == dartforge_elements::model::ClassKind::ExtensionType {
        return inf.core.dynamic_;
    }
    // Sem a chave do setter no interner, nenhuma declaração tem esse
    // setter: a busca não acha nada e segue para o relato.
    let the_key = if setter { inf.setter_key(name.sym) } else { Some(name.sym) };
    let finding = match the_key.map(|k| member_reached_through_super(inf, cls, k)).unwrap_or(Err(None)) {
        Ok(x) => Some(x),
        Err(Some((sup, f))) => {
            let fe = &inf.program.functions[f.0 as usize];
            let typ = match fe.kind {
                dartforge_elements::model::FunctionKind::Getter => "getter",
                dartforge_elements::model::FunctionKind::Setter => "setter",
                dartforge_elements::model::FunctionKind::ImplicitAccessor if setter => "setter",
                dartforge_elements::model::FunctionKind::ImplicitAccessor => "getter",
                _ => "method",
            };
            let the_name = inf.interner.resolve(name.sym).to_string();
            inf.warning_with_code(c::ABSTRACT_SUPER_MEMBER_REFERENCE, name.span, &[typ, &the_name]);
            Some((sup, f))
        }
        Err(None) => None,
    };
    if let Some((sup, f)) = finding {
        let (t, _) = inf.declared_member_type(f, setter);
        let t = inf.substitute_from_owner(this, cls, sup, t);
        resolver(inf, cx, e, Resolved::Member { class: sup, member: MemberRef::Function(f), via_super: true });
        // `super._x`: a propriedade do `_superSsaNode`.
        if use_ == SuperUse::Leitura {
            return field_read(inf, cx, e, Base::Super, t);
        }
        return t;
    }
    // `late final x;` sem inicializador tem setter implícito (o modelo de
    // elementos não o cria): `super.x = v` o alcança.
    if setter {
        for k in super_chain(inf, cls) {
            let field = inf.program.class(k).fields.iter().copied().find(|&v| {
                let ve = inf.program.variable(v);
                ve.name == name.sym && ve.late && ve.final_ && !ve.static_
            });
            if let Some(v) = field {
                return inf.variable_type(v);
            }
        }
    }
    // `Object` (a cadeia de uma classe sempre termina nele).
    let o = inf.core.object;
    if let Some(m) = inf.interface_member(o, name.sym, setter) {
        return m.tipo;
    }
    let the_name = inf.interner.resolve(name.sym).to_string();
    match use_ {
        SuperUse::Invocacao => {
            let owner = inf.interner.resolve(inf.program.class(cls).name).to_string();
            inf.warning_with_code(c::UNDEFINED_SUPER_METHOD, name.span, &[&the_name, &owner]);
        }
        SuperUse::Leitura | SuperUse::Escrita => {
            let typ = inf.table.format(this, inf.interner, inf.program);
            let code = if setter { c::UNDEFINED_SUPER_SETTER } else { c::UNDEFINED_SUPER_GETTER };
            inf.warning_with_code(code, name.span, &[&the_name, &typ]);
        }
    }
    inf.core.dynamic_
}

/// O operador `op` visto por `super` na classe corrente: o primeiro
/// declarado nos supertipos, na ordem de [`membro_super`], instanciado como
/// a classe corrente o vê; senão o de `Object`.
pub(crate) fn lookup_super_operator(inf: &mut BodyInferrer<'_>, cx: &Body, op: SymbolId) -> Busca {
    let (Some(c), Some(this)) = (cx.classe, cx.tipo_this) else { return Busca::Dinamico };
    if inf.program.class(c).kind == dartforge_elements::model::ClassKind::ExtensionType {
        return Busca::Dinamico;
    }
    // Só o concreto (`getMember(forSuper: true)` do `TypePropertyResolver`).
    if let Ok((sup, f)) = member_reached_through_super(inf, c, op) {
        let (t, _) = inf.declared_member_type(f, false);
        let typ = inf.substitute_from_owner(this, c, sup, t);
        return Busca::Achado(Membro {
            resolved: Resolved::Member { class: sup, member: MemberRef::Function(f), via_super: true },
            tipo: typ,
            metodo: true,
            funcao: Some(f),
            of_extension: false,
        });
    }
    let o = inf.core.object;
    match inf.interface_member(o, op, false) {
        Some(m) => Busca::Achado(m),
        None => Busca::Missing,
    }
}

/// Símbolo do operador binário.
fn operator_text(op: BinaryOp) -> Option<&'static str> {
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

fn operator_symbol(inf: &BodyInferrer<'_>, op: BinaryOp) -> Option<SymbolId> {
    inf.interner.lookup(operator_text(op)?)
}

fn skip_whitespace_and_comments(snippet: &str) -> usize {
    let bytes = snippet.as_bytes();
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

fn index_span(inf: &BodyInferrer<'_>, cx: &Body, tgt: ExprId, target: ExprId) -> dartforge_diagnostics::Span {
    let todo = inf.span_expr(cx.unit, tgt);
    let start = inf.span_expr(cx.unit, target).end;
    let between = inf.program.unit(cx.unit).source.get(start..todo.end).unwrap_or("");
    let pos = skip_whitespace_and_comments(between);
    let bracket = start + pos + between.get(pos..).unwrap_or("").find('[').unwrap_or(0);
    dartforge_diagnostics::Span { start: bracket, end: todo.end }
}

fn warn_extension_operator(inf: &mut BodyInferrer<'_>, x: ExtensionId, the_name: &str, span: dartforge_diagnostics::Span) {
    let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
    let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, the_name, extension);
    inf.aviso(msg, span);
}

fn read_index(
    inf: &mut BodyInferrer<'_>, cx: &mut Body, tgt: ExprId, target: ExprId,
    index: ExprId, null_aware: bool, ctx: TypeId,
) -> (TypeId, bool) {
    let (recv, short_circuit) = receptor(inf, cx, target, null_aware);
    // `resolveIndexExpression` (3.6.2 `property_element_resolver.dart:79-97`):
    // o alvo pelo limite (`resolveToBound`) e o `Never` antes do `?[`.
    let raw = inf.body_types.units[cx.unit.0 as usize].get_type(target).unwrap_or(recv);
    let bnd = inf.resolver_ao_limite(raw);
    if !cx.overlays.contains_key(&target) && never_receiver(inf, cx, target, bnd) {
        infer_free(inf, cx, index);
        return (inf.core.never, short_circuit);
    }
    let op = inf.sym.indice;
    if let Some((x, args)) = cx.overlays.get(&target).cloned() {
        if let Some((s, m)) = op.and_then(|s| inf.explicit_extension_member(x, &args, s, false).map(|m| (s, m))) {
            return (binary_operator_with_member(inf, cx, recv, s, index, ctx, Some(tgt), m).0, short_circuit);
        }
        infer_free(inf, cx, index);
        let span = index_span(inf, cx, tgt, target);
        warn_extension_operator(inf, x, "[]", span);
        return (inf.core.dynamic_, short_circuit);
    }
    let undefined = index_span(inf, cx, tgt, target);
    // Alvo `void` (pelo limite): `USE_OF_VOID_RESULT` no `[…]`
    // (`property_element_resolver.dart:81-90`, `_reportUnresolvedIndex`).
    let bnd = {
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
    if matches!(inf.table.get(bnd), Type::Void) {
        infer_free(inf, cx, index);
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, undefined, &[]);
        return (inf.core.dynamic_, short_circuit);
    }
    let token = dartforge_diagnostics::Span { start: undefined.start, end: undefined.start + 1 };
    let super_ = matches!(ast(inf, cx).expr(target).kind, ExprKind::Super);
    let positions = OperatorPositions { token, undefined, compound: false, super_, numeric_target: None };
    (operador_binario(inf, cx, recv, op, index, ctx, positions, Some(tgt)).0, short_circuit)
}

/// Onde o analyzer relata os erros de um operador: `token` para o uso sem
/// checagem de nulo (o operador; em `a[i]`, o `[`), `indefinido` para o
/// `undefined_operator` (o operador; em `a[i]`, o `[i]`).
#[derive(Clone, Copy)]
pub(crate) struct OperatorPositions {
    pub token: dartforge_diagnostics::Span,
    pub undefined: dartforge_diagnostics::Span,
    /// Atribuição composta (`x += 1`): o analyzer fala em "method '+'", e
    /// o operador que falta não é relatado aqui quando o alvo é um método
    /// (`assignment_to_method`) ou `void` (`use_of_void_result`).
    pub compound: bool,
    /// O receptor é `super` (`undefined_super_operator`, outra regra).
    pub super_: bool,
    /// O alvo do refinamento numérico do contexto do operando, quando não é
    /// o receptor: na atribuição composta, o tipo de escrita
    /// (`_computeRhsContext`, assignment_expression_resolver.dart:184-201).
    pub numeric_target: Option<TypeId>,
}

/// Operadores de Dart, do mais longo para o mais curto.
const OPERATORS: &[&str] = &[
    ">>>=", "~/=", ">>=", "<<=", "??=", ">>>", "~/", ">>", "<<", "<=", ">=", "==", "!=", "&&", "||", "++", "--", "+=", "-=",
    "*=", "/=", "%=", "&=", "|=", "^=", "??", "+", "-", "*", "/", "%", "<", ">", "&", "|", "^", "~",
];

/// O token de operador logo depois de `depois_de` (pulando espaços e
/// comentários).
fn operator_token(inf: &BodyInferrer<'_>, cx: &Body, after_: usize) -> dartforge_diagnostics::Span {
    let source = &inf.program.unit(cx.unit).source;
    let rest = source.get(after_..).unwrap_or("");
    let pos = after_ + skip_whitespace_and_comments(rest);
    let text = source.get(pos..).unwrap_or("");
    let n = OPERATORS.iter().find(|o| text.starts_with(**o)).map_or(0, |o| o.len());
    dartforge_diagnostics::Span { start: pos, end: pos + n }
}

/// Invocação de um operador de um argumento (`a + b`, `a[i]`): busca o
/// membro no receptor, infere o argumento com o contexto do parâmetro
/// (com o refinamento numérico) e devolve `(tipo, membro)`.
pub(crate) fn operador_binario(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Body,
    recv: TypeId,
    op: Option<SymbolId>,
    arg: ExprId,
    ctx: TypeId,
    positions: OperatorPositions,
    no: Option<ExprId>,
) -> (TypeId, Option<Membro>) {
    let Some(op) = op else {
        infer_free(inf, cx, arg);
        return (inf.core.dynamic_, None);
    };
    // Receptor potencialmente anulável: o analyzer relata a invocação do
    // operador (`[]`/`[]=` como invocação de método) sem checagem de nulo.
    let check_null = inf.requires_null_check(cx.lib, recv, op, false);
    if check_null {
        let text = inf.interner.resolve(op).to_string();
        let code = if text == "[]" || text == "[]=" || positions.compound {
            dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE
        } else {
            dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE
        };
        let since = inf.diagnostics.len();
        inf.null_warning(recv, code, positions.token, &[&text]);
        // O receptor do operador (`leftOperand`, o alvo do índice, o lado
        // esquerdo da composta).
        let receiver = no.and_then(|n| match &ast(inf, cx).expr(n).kind {
            ExprKind::Binary { left, .. } => Some(*left),
            ExprKind::Index { target, .. } => Some(*target),
            ExprKind::Assign { target, .. } => Some(*target),
            _ => None,
        });
        if let Some(r) = receiver {
            inf.attach_non_promotion(since, cx, Some(r), positions.token);
        }
    }
    // `super[i]`, `super + x`: o operador é o da superclasse (como o
    // `super.m` de [`membro_super`]), não o que a própria classe sobrescreve.
    let search = if positions.super_ { lookup_super_operator(inf, cx, op) } else { inf.lookup_member(cx.lib, recv, op, false) };
    // `AMBIGUOUS_EXTENSION_MEMBER_ACCESS` do operador: no binário inteiro;
    // no índice, no alvo (`extension_member_resolver.dart:115-128`).
    {
        let sp = match no.map(|n| &ast(inf, cx).expr(n).kind) {
            Some(ExprKind::Index { target, .. }) => inf.span_expr(cx.unit, *target),
            // A composta (`a += 0`): no operador.
            Some(ExprKind::Assign { .. }) => positions.token,
            Some(_) => inf.span_expr(cx.unit, no.unwrap()),
            None => positions.token,
        };
        inf.report_extension_ambiguity(sp);
    }
    match search {
        Busca::Achado(m) => binary_operator_with_target_member(inf, cx, recv, positions.numeric_target.unwrap_or(recv), op, arg, ctx, no, m),
        Busca::Missing if check_null => {
            infer_free(inf, cx, arg);
            (inf.core.dynamic_, None)
        }
        Busca::Dinamico => {
            infer_free(inf, cx, arg);
            // Operador sobre `InvalidType`: o resultado é inválido
            // (`binary_expression_resolver.dart:332-335`).
            (if inf.table.e_invalido(recv) { recv } else { inf.core.dynamic_ }, None)
        }
        Busca::NeverType => {
            infer_free(inf, cx, arg);
            (inf.core.never, None)
        }
        Busca::Missing => {
            infer_free(inf, cx, arg);
            if positions.super_ {
                // `UNDEFINED_SUPER_OPERATOR` com o tipo de `super` (o de `this`).
                let text = inf.interner.resolve(op).to_string();
                let typ = inf.table.format(recv, inf.interner, inf.program);
                inf.warning_with_code(
                    dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_SUPER_OPERATOR,
                    positions.undefined,
                    &[&text, &typ],
                );
                return (inf.core.dynamic_, None);
            }
            if matches!(inf.table.get(recv), Type::Void | Type::Function { .. }) {
                return (inf.core.dynamic_, None);
            }
            let text = inf.interner.resolve(op).to_string();
            let typ = inf.table.format(recv, inf.interner, inf.program);
            inf.warning_with_code(
                dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR,
                positions.undefined,
                &[&text, &typ],
            );
            // Operador que o tipo não tem: o resultado é `InvalidType`
            // (`binary_expression_resolver.dart:308`).
            (inf.table.invalid(inf.core.dynamic_), None)
        }
    }
}

fn binary_operator_with_member(
    inf: &mut BodyInferrer<'_>, cx: &mut Body, recv: TypeId, op: SymbolId,
    arg: ExprId, ctx: TypeId, no: Option<ExprId>, m: Membro,
) -> (TypeId, Option<Membro>) {
    binary_operator_with_target_member(inf, cx, recv, recv, op, arg, ctx, no, m)
}

/// Como [`operador_binario_com_membro`], com o alvo do refinamento numérico
/// do contexto do operando à parte do receptor.
#[allow(clippy::too_many_arguments)]
fn binary_operator_with_target_member(
    inf: &mut BodyInferrer<'_>, cx: &mut Body, recv: TypeId, numeric_target: TypeId, op: SymbolId,
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
        Some(p) => numeric_context(inf, numeric_target, &m, op, ctx, p),
        None => inf.core.unknown,
    };
    let ta = inferir(inf, cx, arg, ctx_arg);
    if let Some(p) = param {
        verify_assignable_expr(inf, cx, arg, ta, p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
    }
    let t = refine_numeric(inf, recv, &m, op, &[ta], ret);
    (t, Some(m))
}

fn is_refinable_numeric(inf: &BodyInferrer<'_>, m: &Membro, op: SymbolId) -> bool {
    if m.of_extension {
        return false;
    }
    let s = inf.interner.resolve(op);
    matches!(s, "+" | "-" | "*" | "%" | "remainder")
}

/// Contexto do argumento de `e1 op e2` numérico (`_refineNumericInvocationContext`).
pub(crate) fn numeric_context(inf: &mut BodyInferrer<'_>, t: TypeId, m: &Membro, op: SymbolId, ctx: TypeId, current: TypeId) -> TypeId {
    if !is_refinable_numeric(inf, m, op) && Some(op) != inf.sym.clamp {
        return current;
    }
    let num_q = inf.anulavel(inf.core.num);
    if !inf.sub(t, num_q) {
        return current;
    }
    let c = inf.greatest_closure_of(ctx);
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
pub(crate) fn refine_numeric(inf: &mut BodyInferrer<'_>, t: TypeId, m: &Membro, op: SymbolId, args: &[TypeId], current: TypeId) -> TypeId {
    let (int, double, num) = (inf.core.int, inf.core.double, inf.core.num);
    let num_q = inf.anulavel(num);
    let int_q = inf.anulavel(int);
    let double_q = inf.anulavel(double);
    if is_refinable_numeric(inf, m, op) && args.len() == 1 && inf.sub(t, num_q) {
        let s = args[0];
        if inf.sub(t, double_q) {
            return double;
        }
        let bottom = inf.e_fundo(s);
        if !bottom && inf.sub(s, double_q) {
            return double;
        }
        if !bottom && inf.sub(t, int_q) && inf.sub(s, int_q) {
            return int;
        }
        return num;
    }
    if Some(op) == inf.sym.clamp && !m.of_extension && args.len() == 2 && inf.sub(t, num_q) {
        let (t2, t3) = (args[0], args[1]);
        if inf.e_fundo(t2) || inf.e_fundo(t3) {
            return current;
        }
        if inf.sub(t, int_q) && inf.sub(t2, int_q) && inf.sub(t3, int_q) {
            return int;
        }
        if inf.sub(t, double_q) && inf.sub(t2, double_q) && inf.sub(t3, double_q) {
            return double;
        }
        return num;
    }
    current
}

/// Operadores binários.
fn binary(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, op: BinaryOp, left: ExprId, right: ExprId, ctx: TypeId) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    match op {
        // `BinaryExpressionResolver.resolve` (`binary_expression_resolver.dart:69-77`):
        // o operador que não é binário, no token; `_resolveUnsupportedOperator`
        // visita os dois lados sem contexto e o tipo é `InvalidType`.
        BinaryOp::NaoBinario => {
            let left_end = inf.span_expr(cx.unit, left).end;
            let token = operator_token(inf, cx, left_end);
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::NOT_BINARY_OPERATOR, token, &["~"]);
            infer_free(inf, cx, left);
            infer_free(inf, cx, right);
            inf.table.invalid(inf.core.dynamic_)
        }
        BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq => {
            let (vf, ff) = binary_condition(inf, cx, e, op, left, right);
            cx.flow = inf.juntar(&vf, &ff);
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
            let k_q = if inf.the_is_unknown(ctx) { ctx } else { inf.anulavel(ctx) };
            let t1 = inferir(inf, cx, left, k_q);
            void_use(inf, cx, left, t1);
            dead_in_right_operand(inf, cx, left, right);
            warn_dead_null(inf, cx, t1, right);
            let j = if inf.the_is_unknown(ctx) || inf.e_dynamic(ctx) { t1 } else { ctx };
            // Ramo em que `e1` não é nulo: `e1` promove a não nulo; no outro
            // nada se promove. A junção dos dois vale depois, então
            // `y ?? (throw 0)` deixa `y` promovido.
            let mut before = cx.flow.clone();
            if let Some(id) = promotion_target(inf, cx, left) {
                let decl = cx.local(id).tipo;
                inf.promote_non_null(&mut before, id, decl);
            }
            let t2 = inferir(inf, cx, right, j);
            let after = cx.flow.clone();
            cx.flow = inf.juntar(&before, &after);
            let nn = inf.non_null(t1);
            upper_bound_in_context(inf, nn, t2, ctx)
        }
        _ => {
            let u = inf.core.unknown;
            let l = inferir(inf, cx, left, u);
            void_use(inf, cx, left, l);
            dead_in_right_operand(inf, cx, left, right);
            if never_receiver(inf, cx, left, l) {
                infer_free(inf, cx, right);
                return inf.core.never;
            }
            if matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, left, this);
            }
            let sym = operator_symbol(inf, op);
            if let Some((x, args)) = cx.overlays.get(&left).cloned()
                && let Some(text) = operator_text(op)
            {
                if let Some((s, m)) = sym.and_then(|s| inf.explicit_extension_member(x, &args, s, false).map(|m| (s, m))) {
                    return binary_operator_with_member(inf, cx, l, s, right, ctx, Some(e), m).0;
                }
                let start = inf.span_expr(cx.unit, left).end;
                let end = inf.span_expr(cx.unit, right).start;
                let snippet = inf.program.unit(cx.unit).source.get(start..end).unwrap_or("");
                let pos = skip_whitespace_and_comments(snippet);
                let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, text, extension);
                if snippet.get(pos..).is_some_and(|rest| rest.starts_with(text)) {
                    let offset = start + pos;
                    inf.aviso(msg, dartforge_diagnostics::Span { start: offset, end: offset + text.len() });
                }
                infer_free(inf, cx, right);
                return inf.core.dynamic_;
            }
            let token = operator_token(inf, cx, inf.span_expr(cx.unit, left).end);
            let _ = span;
            let super_ = matches!(ast(inf, cx).expr(left).kind, ExprKind::Super);
            let positions = OperatorPositions { token, undefined: token, compound: false, super_, numeric_target: None };
            let (t, _) = operador_binario(inf, cx, l, sym, right, ctx, positions, Some(e));
            t
        }
    }
}

/// O operando direito de um binário que começa inalcançável: o primeiro nó
/// morto tem pai `BinaryExpression`, e o trecho vai do operador ao fim do
/// operando direito (`dead_code_verifier.dart:316-318`).
fn dead_in_right_operand(inf: &mut BodyInferrer<'_>, cx: &mut Body, left: ExprId, right: ExprId) {
    if cx.flow.alcancavel || cx.dead_code.is_some() {
        return;
    }
    let token = operator_token(inf, cx, inf.span_expr(cx.unit, left).end);
    let end = inf.span_expr(cx.unit, right).end;
    inf.aviso(DEAD_CODE.template.to_string(), dartforge_diagnostics::Span { start: token.start, end: end });
    cx.dead_code = Some(cx.flow_ends.len());
}

/// Unários (`!`, `-`, `~`, `++`, `--`, `!` pós-fixo).
fn unary(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, op: UnaryOp, operand: ExprId, ctx: TypeId, short_circuit: &mut bool) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    match op {
        UnaryOp::Not => {
            let (vf, ff) = condicao(inf, cx, operand);
            // Fora de condição, `!e` também exige `bool` (e não `void`).
            verify_bool(inf, cx, operand, BoolUse::Negation);
            cx.flow = inf.juntar(&vf, &ff);
            inf.core.bool_
        }
        UnaryOp::NullAssert => {
            // O operando de `e!` recebe o contexto anulável (`K?`): em
            // `_name = _becomeParentOf(name)!`, `T` sai `IdentifierImpl?`.
            let k = if inf.the_is_unknown(ctx) { ctx } else { inf.anulavel(ctx) };
            let (t, c) = infer_at(inf, cx, operand, k, true);
            *short_circuit = c;
            // `NULL_CHECK_ALWAYS_FAILS` (`visitPostfixExpression`,
            // `best_practices_verifier.dart:686-697`): `e!` com `e` de tipo `Null`.
            if e_null_do_core(inf, t) {
                inf.warning_with_code(dartforge_diagnostics::codigos::warning::NULL_CHECK_ALWAYS_FAILS, span, &[]);
            }
            // `unnecessary_non_null_assertion` (no `!`): só com o operando
            // certamente não anulável (`T extends Object?` não é).
            if !c && strictly_non_nullable(inf, t) {
                let sp = dartforge_diagnostics::Span { start: span.end.saturating_sub(1), end: span.end };
                inf.warning_with_code(dartforge_diagnostics::codigos::static_warning::UNNECESSARY_NON_NULL_ASSERTION, sp, &[]);
            }
            if let Some(id) = promotion_target(inf, cx, operand) {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                inf.promote_non_null(&mut f, id, decl);
                cx.flow = f;
            }
            // `visitPostfixExpression` com `!`: `checkForUseOfVoidResult(node)`
            // (3.6.2 `error_verifier.dart:1310-1316`), no `e!` inteiro.
            let r = inf.non_null_promotion(t);
            if matches!(inf.table.get(r), Type::Void) {
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, span, &[]);
            }
            r
        }
        UnaryOp::Neg | UnaryOp::BitNot => {
            // `-1` com literal: o literal recebe o contexto (`double x = -1`).
            let literal = op == UnaryOp::Neg && matches!(inf.program.unit(cx.unit).ast.expr(operand).kind, ExprKind::Int(_));
            let c = if literal { ctx } else { inf.core.unknown };
            if literal && op == UnaryOp::Neg {
                cx.negated_literal = Some(operand);
            }
            let t = inferir(inf, cx, operand, c);
            void_use(inf, cx, operand, t);
            let sym = if op == UnaryOp::Neg { inf.sym.unary_minus } else { inf.sym.til };
            if let Some((x, args)) = cx.overlays.get(&operand).cloned() {
                if let Some(m) = sym.and_then(|s| inf.explicit_extension_member(x, &args, s, false)) {
                    resolver(inf, cx, e, m.resolved.clone());
                    return match inf.table.get(m.tipo) {
                        Type::Function { ret, .. } => *ret,
                        _ => inf.core.dynamic_,
                    };
                }
                let operator = if op == UnaryOp::Neg { "unary-" } else { "~" };
                let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, operator, extension);
                inf.aviso(msg, dartforge_diagnostics::Span { start: span.start, end: span.start + 1 });
                return inf.core.dynamic_;
            }
            let Some(sym) = sym else { return inf.core.dynamic_ };
            let the_name = if op == UnaryOp::Neg { "unary-" } else { "~" };
            let sp_op = dartforge_diagnostics::Span { start: span.start, end: span.start + 1 };
            let null = null_in_unary(inf, cx, t, sym, the_name, sp_op, operand);
            let search = inf.lookup_member(cx.lib, t, sym, false);
            // A ambiguidade de extensão do prefixo, no operando.
            let sp_operand = inf.span_expr(cx.unit, operand);
            inf.report_extension_ambiguity(sp_operand);
            match search {
                Busca::Achado(m) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    match inf.table.get(m.tipo) {
                        Type::Function { ret, .. } => *ret,
                        _ => inf.core.dynamic_,
                    }
                }
                Busca::NeverType => inf.core.never,
                Busca::Missing if !null => {
                    undefined_unary_operator(inf, t, the_name, sp_op);
                    inf.table.invalid(inf.core.dynamic_)
                }
                _ => inf.core.dynamic_,
            }
        }
        UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
            let prefix = matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec);
            let bop = if matches!(op, UnaryOp::PrefixInc | UnaryOp::PostfixInc) { BinaryOp::Add } else { BinaryOp::Sub };
            let (read, write, local) = read_for_write(inf, cx, operand, short_circuit);
            // `_resolve1` do `PostfixExpressionResolver` e o do
            // `PrefixExpressionResolver` (3.6.2 `:127-132`, `:177-183`): a
            // leitura `Never` é `RECEIVER_OF_TYPE_NEVER` no operando, sem
            // operador, e o tipo é `Never`.
            if matches!(inf.table.get(read), Type::Never) {
                never_receiver(inf, cx, operand, read);
                return inf.core.never;
            }
            let sym = operator_symbol(inf, bop);
            let int = inf.core.int;
            if let Some(s) = sym {
                // O token `++`/`--`, antes ou depois do operando.
                let sp_op = if prefix {
                    dartforge_diagnostics::Span { start: span.start, end: span.start + 2 }
                } else {
                    dartforge_diagnostics::Span { start: span.end.saturating_sub(2), end: span.end }
                };
                let the_name = if bop == BinaryOp::Add { "+" } else { "-" };
                let null = null_in_unary(inf, cx, read, s, the_name, sp_op, operand);
                // `C++` com `C` tipo: só o `ASSIGNMENT_TO_TYPE` da escrita.
                let of_type = type_reference(inf, cx, operand).is_some()
                    || matches!(&ast(inf, cx).expr(operand).kind, ExprKind::Identifier(n)
                        if matches!(resolve_name(inf, cx, n.sym, false), NameRef::BuiltinType | NameRef::TypeParam(_)));
                if !null && !of_type && matches!(inf.lookup_member(cx.lib, read, s, false), Busca::Missing) {
                    undefined_unary_operator(inf, read, the_name, sp_op);
                }
            }
            let res = match sym.map(|s| inf.lookup_member(cx.lib, read, s, false)) {
                Some(Busca::Achado(m)) => {
                    resolver(inf, cx, e, m.resolved.clone());
                    let (ret, param) = match inf.table.get(m.tipo) {
                        Type::Function { ret, positional, .. } => (*ret, positional.first().copied()),
                        _ => (inf.core.dynamic_, None),
                    };
                    let r = refine_numeric(inf, read, &m, sym.unwrap(), &[int], ret);
                    // `++a`/`a++`: o `1` implícito (`int`) contra o parâmetro do
                    // operador, relatado no operando
                    // (`_checkForIntNotAssignable`, an611:src/generated/error_verifier.dart:3899-3906);
                    // o resultado contra o tipo de escrita, no nó inteiro
                    // (`_checkForInvalidAssignmentIncDec`,
                    // an611:src/dart/resolver/prefix_expression_resolver.dart:96-110).
                    if !inf.e_dynamic(read) && !matches!(inf.table.get(read), Type::Void) {
                        if let Some(p) = param {
                            let sp = inf.span_expr(cx.unit, sem_parenteses(inf, cx, operand));
                            inf.verify_assignable(int, p, sp, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                        }
                        // Getter e setter de tipos diferentes (`int get x`, `set
                        // x(String)`): o oráculo 3.6.2 não relata (conferido no
                        // corpus, `InvalidAssignment__postfixExpression_in_*`).
                        // O retorno do operador (`operatorReturnType`), sem o
                        // `?` do encurtamento nulo (`x?[0]++`).
                        if !inf.e_dynamic(write) && (local.is_some() || read == write) {
                            inf.verify_assignable(r, write, span, INVALID_ASSIGNMENT.template);
                        }
                    }
                    r
                }
                Some(Busca::NeverType) => inf.core.never,
                _ => inf.core.dynamic_,
            };
            if let Some(id) = local {
                // A regra de escrita vê o estado de antes da escrita.
                let target_span = inf.span_expr(cx.unit, operand);
                check_final_local(inf, cx, id, target_span);
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                let reason = super::flow::NonPromotionReason::Escrita { nome: cx.local(id).nome, span };
                inf.assign_flow(&mut f, id, decl, res, Some(reason));
                cx.flow = f;
            }
            let _ = write;
            if prefix {
                res
            } else {
                read
            }
        }
    }
}

/// O operador de prefixo ou sufixo (`-x`, `~x`, `++x`, `x--`) num receptor
/// potencialmente anulável: `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE`
/// no token do operador, pelo `TypePropertyResolver` (o `Null` puro é
/// `INVALID_USE_OF_NULL_VALUE`), com as mensagens de não promoção do operando.
fn null_in_unary(inf: &mut BodyInferrer<'_>, cx: &Body, recv: TypeId, sym: SymbolId, the_name: &str, sp: dartforge_diagnostics::Span, operand: ExprId) -> bool {
    // `void` é potencialmente anulável e não é limitado por `dynamic`: o
    // `TypePropertyResolver` (`type_property_resolver.dart:80-150`) relata o
    // operador que nem `Object` nem uma extensão tem (além do
    // `USE_OF_VOID_RESULT` no operando).
    let requires = if matches!(inf.table.get(recv), Type::Void) {
        let o = inf.core.object;
        inf.interface_member(o, sym, false).is_none() && inf.extension_member(cx.lib, recv, sym, false).is_none()
    } else {
        inf.requires_null_check(cx.lib, recv, sym, false)
    };
    if !requires {
        return false;
    }
    let since = inf.diagnostics.len();
    inf.null_warning(recv, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE, sp, &[the_name]);
    inf.attach_non_promotion(since, cx, Some(operand), sp);
    true
}

/// O operador de prefixo/sufixo que o tipo do operando não tem
/// (`TypePropertyResolver` sem membro): `UNDEFINED_OPERATOR` no token do
/// operador (`prefix_expression_resolver.dart`, `postfix_expression_resolver.dart`).
fn undefined_unary_operator(inf: &mut BodyInferrer<'_>, recv: TypeId, the_name: &str, sp: dartforge_diagnostics::Span) {
    if matches!(inf.table.get(recv), Type::Void | Type::Function { .. }) || inf.table.e_invalido(recv) {
        return;
    }
    let typ = inf.table.format(recv, inf.interner, inf.program);
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR, sp, &[the_name, &typ]);
}

fn check_final_local(inf: &mut BodyInferrer<'_>, cx: &Body, id: LocalId, span: dartforge_diagnostics::Span) {
    let l = cx.local(id);
    if cx.funcoes_locais.contains(&id) {
        inf.aviso(ASSIGNMENT_TO_FUNCTION.template.to_string(), span);
    } else if l.const_ {
        inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), span);
    } else if l.final_ && l.late {
        if cx.flow.assigned(id) {
            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::LATE_FINAL_LOCAL_ALREADY_ASSIGNED, span, &[]);
        }
    } else if l.final_ && !cx.flow.unassigned(id) {
        let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
        inf.aviso(msg, span);
    }
}

fn warn_write_to_method(inf: &mut BodyInferrer<'_>, the_name: ast::Name) {
    inf.aviso(ASSIGNMENT_TO_METHOD.template.to_string(), the_name.span);
}

/// Diagnóstico quando a recuperação de uma escrita encontra apenas o getter.
/// O getter explícito é uma propriedade sem setter; o getter implícito é um
/// campo `final` ou `const`. `late final` sem inicializador ainda aceita escrita.
fn warn_member_without_setter(inf: &mut BodyInferrer<'_>, the_name: ast::Name, f: dartforge_elements::model::FunctionElementId) -> bool {
    let fe = inf.program.function(f);
    match fe.kind {
        FunctionKind::Getter => {
            let owner = fe.class.map(|c| inf.program.class(c).name)
                .or_else(|| fe.extension.and_then(|e| inf.program.extension(e).name));
            let Some(owner) = owner else { return false };
            let msg = format!(
                "{}: '{}' na classe '{}'",
                ASSIGNMENT_TO_FINAL_NO_SETTER.template,
                inf.interner.resolve(the_name.sym),
                inf.interner.resolve(owner)
            );
            inf.aviso(msg, the_name.span);
            true
        }
        FunctionKind::ImplicitAccessor => {
            let Some(v) = fe.variable else { return false };
            let ve = inf.program.variable(v);
            if ve.const_ {
                inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), the_name.span);
            } else if ve.final_ && !(ve.late && inf.inicializador(v).is_none()) {
                let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL.template, inf.interner.resolve(the_name.sym));
                inf.aviso(msg, the_name.span);
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
fn read_for_write(inf: &mut BodyInferrer<'_>, cx: &mut Body, tgt: ExprId, short_circuit_out: &mut bool) -> (TypeId, TypeId, Option<LocalId>) {
    let a = ast(inf, cx);
    match &a.expr(tgt).kind {
        ExprKind::Identifier(n) => {
            let n = *n;
            match resolve_name(inf, cx, n.sym, true) {
                NameRef::Local(id) => {
                    resolver(inf, cx, tgt, Resolved::Local(id));
                    join_reference(inf, cx, id, n);
                    write_in_guard(inf, cx, id, n.span);
                    write_in_primary(inf, cx, id, n.span);
                    let t = ler_local(inf, cx, id, n.span);
                    let decl = cx.local(id).tipo;
                    (t, decl, Some(id))
                }
                _ => {
                    let read = identificador(inf, cx, tgt, n);
                    let write = name_write_type(inf, cx, tgt, n);
                    (read, write, None)
                }
            }
        }
        ExprKind::Property { target, name, null_aware } => {
            let (target, name, null_aware) = (*target, *name, *null_aware);
            let (read, short_circuit_read) = propriedade(inf, cx, tgt, target, name, null_aware);
            let recv_read = inf.body_types.units[cx.unit.0 as usize].get_type(target).map(|t| {
                (if null_aware { inf.non_null(t) } else { t }, short_circuit_read)
            });
            let mut short_circuit = false;
            let write = property_write(inf, cx, tgt, target, name, null_aware, &mut short_circuit, recv_read);
            *short_circuit_out = short_circuit_read || short_circuit;
            registrar(inf, cx, tgt, read);
            (read, write, None)
        }
        ExprKind::Index { target, index, null_aware } => {
            let (target, index, null_aware) = (*target, *index, *null_aware);
            let u = inf.core.unknown;
            let (t, c) = read_index(inf, cx, tgt, target, index, null_aware, u);
            *short_circuit_out = c;
            // O tipo de escrita (`setWriteElement` com `IndexExpression`, 3.6.2
            // `resolver.dart:1722-1728`): o 2º parâmetro do `[]=`; sem ele,
            // `InvalidType` (`dynamic` em alvo `dynamic`).
            let second = |inf: &BodyInferrer<'_>, f: TypeId| match inf.table.get(f) {
                Type::Function { positional, .. } if positional.len() == 2 => Some(positional[1]),
                _ => None,
            };
            let mut write = inf.table.invalid(inf.core.dynamic_);
            if let Some((x, args)) = cx.overlays.get(&target).cloned() {
                match inf.sym.set_index.and_then(|s| inf.explicit_extension_member(x, &args, s, false)) {
                    Some(m) => {
                        if let Some(p1) = second(inf, m.tipo) {
                            write = p1;
                        }
                    }
                    None => {
                        let span = index_span(inf, cx, tgt, target);
                        warn_extension_operator(inf, x, "[]=", span);
                    }
                }
            } else if let Some(op) = inf.sym.set_index
                && matches!(ast(inf, cx).expr(target).kind, ExprKind::Super)
            {
                // `super[i]++`: o `[]=` também pela cadeia de `super`.
                match lookup_super_operator(inf, cx, op) {
                    Busca::Missing => {
                        let span = index_span(inf, cx, tgt, target);
                        let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                        let typ = inf.table.format(this, inf.interner, inf.program);
                        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_SUPER_OPERATOR, span, &["[]=", &typ]);
                    }
                    Busca::Achado(m) => {
                        if let Some(p1) = second(inf, m.tipo) {
                            write = p1;
                        }
                    }
                    _ => {}
                }
            } else if let Some(op) = inf.sym.set_index
                && let Some(recv) = inf.body_types.units[cx.unit.0 as usize].get_type(target)
                && !matches!(ast(inf, cx).expr(target).kind, ExprKind::Super)
            {
                // `x[i]++`: o `[]=` também é procurado (o `writeElement` do
                // `PropertyElementResolver`); ausente, `UNDEFINED_OPERATOR`
                // no `[…]` (`_reportUnresolvedIndex`).
                let r = if null_aware { inf.non_null(recv) } else { recv };
                let invalid = inf.table.e_invalido(r) || matches!(inf.table.get(r), Type::Dynamic | Type::Never | Type::Void);
                if matches!(inf.table.get(r), Type::Dynamic) && !inf.table.e_invalido(r) {
                    write = inf.core.dynamic_;
                }
                if !invalid {
                    if let Busca::Achado(m) = inf.lookup_member(cx.lib, r, op, false)
                        && let Some(p1) = second(inf, m.tipo)
                    {
                        write = p1;
                    }
                }
                if !invalid && !inf.requires_null_check(cx.lib, r, op, false) {
                    match inf.lookup_member(cx.lib, r, op, false) {
                        Busca::Missing => {
                            let span = index_span(inf, cx, tgt, target);
                            let typ = inf.table.format(r, inf.interner, inf.program);
                            inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR, span, &["[]=", &typ]);
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
                                inf.verify_assignable(ti, p0, sp, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                            }
                        }
                        _ => {}
                    }
                }
            }
            registrar(inf, cx, tgt, t);
            (t, write, None)
        }
        _ => {
            let t = infer_free(inf, cx, tgt);
            (t, t, None)
        }
    }
}

/// `_checkForAmbiguousImport` (3.6.2 `error_verifier.dart:2116-2130`): o
/// nome (com o prefixo) que dois imports trazem com elementos diferentes é
/// `AMBIGUOUS_IMPORT` no nome; o elemento é o `MultiplyDefinedElement` e o tipo
/// é inválido. Diz se relatou.
pub(crate) fn ambiguous_import(inf: &mut BodyInferrer<'_>, cx: &Body, prefix: Option<SymbolId>, the_name: ast::Name) -> bool {
    let b = match prefix {
        Some(p) => inf.program.lookup_prefixed_na_unidade(cx.unit, p, the_name.sym),
        None => inf.program.lookup_na_unidade(cx.unit, the_name.sym),
    };
    if !b.is_some_and(|b| b.ambiguous) {
        return false;
    }
    let list = crate::scope::ambiguous_libraries(inf.program, cx.unit, prefix, the_name.sym);
    let text = inf.interner.resolve(the_name.sym).to_string();
    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::AMBIGUOUS_IMPORT, the_name.span, &[&text, &list]);
    true
}

/// Tipo de escrita (setter) de um nome não local.
fn name_write_type(inf: &mut BodyInferrer<'_>, cx: &mut Body, tgt: ExprId, n: ast::Name) -> TypeId {
    match resolve_name(inf, cx, n.sym, true) {
        NameRef::Local(id) => cx.local(id).tipo,
        NameRef::Elemento(_) if ambiguous_import(inf, cx, None, n) => inf.table.invalid(inf.core.dynamic_),
        NameRef::Elemento(el) => {
            resolver(inf, cx, tgt, Resolved::Element(el));
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
                        inf.table.invalid(inf.core.dynamic_)
                    } else if ve.final_ && !(ve.late && inf.inicializador(v).is_none()) {
                        let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL.template, inf.interner.resolve(n.sym));
                        inf.aviso(msg, n.span);
                        inf.table.invalid(inf.core.dynamic_)
                    } else {
                        inf.variable_type(v)
                    }
                }
                Element::Function(f) => {
                    let fe = inf.program.function(f);
                    match (fe.kind, fe.variable) {
                        (FunctionKind::ImplicitAccessor, Some(v)) => inf.variable_type(v),
                        (FunctionKind::Setter, _) => inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_),
                        (FunctionKind::Getter, _) => {
                            let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL.template, inf.interner.resolve(n.sym));
                            inf.aviso(msg, n.span);
                            inf.table.invalid(inf.core.dynamic_)
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
        NameRef::TypeParam(_) | NameRef::BuiltinType => {
            inf.aviso(ASSIGNMENT_TO_TYPE.template.to_string(), n.span);
            inf.core.dynamic_
        }
        // `AssignmentVerifier.verify` (`assignment_verifier.dart:66-71`): a
        // recuperação da escrita é o prefixo.
        NameRef::Prefixo => {
            prefix_without_dot(inf, n);
            inf.core.dynamic_
        }
        NameRef::LexicalMember(f, static_) => {
            let r = lexical_member_resolved(inf, cx, f, static_);
            resolver(inf, cx, tgt, r);
            let fe = inf.program.function(f);
            // O setter de instância escrito onde `this` não vale (método
            // estático, fábrica, inicializador): o elemento de escrita é ele
            // (`setWriteElement`) e o erro é o do acesso sem `this`
            // (`_checkForInvalidInstanceMemberAccess`).
            if !static_ && instance_without_this_error(inf, cx).is_some() {
                let write_type = match (fe.kind, fe.variable) {
                    (FunctionKind::Setter, _) => Some(inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_)),
                    (FunctionKind::ImplicitAccessor, Some(v)) if !inf.program.variable(v).final_ && !inf.program.variable(v).const_ => {
                        Some(inf.variable_type(v))
                    }
                    _ => None,
                };
                if let Some(t) = write_type {
                    warn_instance_without_this(inf, cx, n);
                    return t;
                }
            }
            if fe.kind == FunctionKind::Function && fe.class.is_some() {
                warn_write_to_method(inf, n);
                // `setWriteElement`: o tipo de escrita só vem de setter ou
                // variável; de um método é `InvalidType`.
                return inf.table.invalid(inf.core.dynamic_);
            }
            // `LexicalLookup.resolveSetter` (3.6.2 `lexical_lookup.dart`): o
            // getter achado sem setter só segue para o `this` implícito
            // (`ThisLookup.lookupSetter`, onde um setter herdado ou de extensão
            // pode aparecer) quando é `isInstanceMember` — membro de instância
            // de `InterfaceElement` (`extensions.dart:109-119`). O estático e o
            // de extensão são a recuperação: `static final int x` numa classe
            // cuja superclasse tem um setter `x` de extensão dá
            // `ASSIGNMENT_TO_FINAL` (`static_extension_internal_basename_shadowing_error_test.dart:402`).
            let follows_through_this = !static_ && fe.extension.is_none();
            if let (FunctionKind::ImplicitAccessor, Some(v)) = (fe.kind, fe.variable) {
                let ve = inf.program.variable(v);
                if (ve.final_ || ve.const_) && ve.setter.is_none() && follows_through_this {
                    // Campo final sem setter: pode haver setter herdado.
                    if let Some(this) = cx.tipo_this {
                        if let Busca::Achado(m) = inf.lookup_member(cx.lib, this, n.sym, true) {
                            return m.tipo;
                        }
                    }
                }
            }
            if follows_through_this {
                if let Some(this) = cx.tipo_this {
                    if let Busca::Achado(m) = inf.lookup_member(cx.lib, this, n.sym, true) {
                        return m.tipo;
                    }
                }
            }
            warn_member_without_setter(inf, n, f);
            // Sem setter, o elemento de escrita é o de recuperação (o getter):
            // `InvalidType` (`setWriteElement`).
            inf.table.invalid(inf.core.dynamic_)
        }
        NameRef::ImplicitThis => {
            let this = cx.tipo_this.unwrap();
            // `ThisLookup.lookupSetter` com `this` potencialmente anulável: o
            // 3.6.2 relata a variante de método (`The method 'foo' can't be
            // unconditionally invoked`) em toda escrita pelo `this`
            // implícito — `=`, composta, `??=`, `++` — conferido no oráculo.
            if inf.requires_null_check(cx.lib, this, n.sym, true) {
                let the_name = inf.interner.resolve(n.sym).to_string();
                let since = inf.diagnostics.len();
                inf.null_warning(this, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE, n.span, &[&the_name]);
                inf.attach_non_promotion(since, cx, None, n.span);
                let nn = inf.non_null(this);
                return match inf.lookup_member(cx.lib, nn, n.sym, true) {
                    Busca::Achado(m) => {
                        resolver(inf, cx, tgt, m.resolved.clone());
                        m.tipo
                    }
                    _ => inf.core.dynamic_,
                };
            }
            match inf.lookup_member(cx.lib, this, n.sym, true) {
                Busca::Achado(m) => {
                    resolver(inf, cx, tgt, m.resolved.clone());
                    m.tipo
                }
                Busca::Missing => {
                    if let Busca::Achado(getter) = inf.lookup_member(cx.lib, this, n.sym, false) {
                        if let Some(f) = getter.funcao {
                            if warn_member_without_setter(inf, n, f) {
                                resolver(inf, cx, tgt, getter.resolved);
                                return getter.tipo;
                            }
                        }
                    }
                    undefined_written_name(inf, cx, n);
                    inf.core.dynamic_
                }
                // Extensões ambíguas (`ThisLookup.lookupSetter`): a
                // ambiguidade e, sem setter, o nome indefinido.
                Busca::Dinamico if inf.extension_ambiguity.is_some() => {
                    inf.report_extension_ambiguity(n.span);
                    undefined_name_without_this(inf, cx, n);
                    inf.core.dynamic_
                }
                _ => inf.core.dynamic_,
            }
        }
        NameRef::ConstantEnum(v) => inf.variable_type(v),
        // Só curingas declaram `_` aqui (3.7): usar `_` é erro
        // (`Undefined name '_'` no CFE).
        NameRef::Nenhum if cx.curinga == Some(n.sym) => {
            inf.erro_de_linguagem(cx.unit, n.span, WILDCARD_DOES_NOT_BIND.template.to_string());
            inf.core.dynamic_
        }
        NameRef::Nenhum => {
            undefined_written_name(inf, cx, n);
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
fn undefined_written_name(inf: &mut BodyInferrer<'_>, cx: &Body, n: ast::Name) {
    if let Some(this) = analyzer_this_type(inf, cx) {
        match lookup_through_this(inf, cx, this, n.sym, true) {
            ThroughThis::Instance => {
                warn_instance_without_this(inf, cx, n);
                return;
            }
            ThroughThis::Estatico(owner, _) => {
                warn_unqualified_static(inf, cx, owner, n);
                return;
            }
            ThroughThis::Uncertain => return,
            ThroughThis::Missing => {}
        }
        if !matches!(lookup_through_this(inf, cx, this, n.sym, false), ThroughThis::Missing) {
            return;
        }
    }
    let msg = format!("{}: '{}'", UNDEFINED_IDENTIFIER.template, inf.interner.resolve(n.sym));
    inf.aviso(msg, n.span);
}

/// Atribuições: `=`, compostas e `??=`.
fn atribuicao(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, op: AssignOp, tgt: ExprId, the_value: ExprId, short_circuit: &mut bool) -> TypeId {
    let span = inf.span_expr(cx.unit, e);
    let a = ast(inf, cx);
    let target_kind = &a.expr(tgt).kind;
    match op {
        AssignOp::Assign => {
            // Tipo de escrita e contexto.
            let (write, context, local) = match target_kind {
                ExprKind::Identifier(n) => {
                    let n = *n;
                    match resolve_name(inf, cx, n.sym, true) {
                        NameRef::Local(id) => {
                            resolver(inf, cx, tgt, Resolved::Local(id));
                            join_reference(inf, cx, id, n);
                            write_in_guard(inf, cx, id, n.span);
                            write_in_primary(inf, cx, id, n.span);
                            let l = cx.local(id).clone();
                            if cx.funcoes_locais.contains(&id) {
                                inf.aviso(ASSIGNMENT_TO_FUNCTION.template.to_string(), n.span);
                                (inf.core.dynamic_, inf.core.dynamic_, None)
                            } else {
                                // `assignment_expression_resolver.dart:360-385`:
                                // `late final` já atribuída e `final` talvez atribuída.
                                if l.const_ {
                                    if !cx.flow.unassigned(id) {
                                        inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), n.span);
                                    }
                                } else if l.final_ && l.late {
                                    if cx.flow.assigned(id) {
                                        inf.warning_with_code(
                                            dartforge_diagnostics::codigos::compile_time_error::LATE_FINAL_LOCAL_ALREADY_ASSIGNED,
                                            n.span,
                                            &[],
                                        );
                                    }
                                } else if l.final_ && !cx.flow.unassigned(id) {
                                    let msg = format!("{}: '{}'", ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
                                    inf.aviso(msg, n.span);
                                }
                                let current = cx.flow.current_type(id, l.tipo);
                                (l.tipo, current, Some(id))
                            }
                        }
                        _ => {
                            let t = name_write_type(inf, cx, tgt, n);
                            (t, t, None)
                        }
                    }
                }
                ExprKind::Property { target, name, null_aware } => {
                    let (target, name, null_aware) = (*target, *name, *null_aware);
                    let t = property_write(inf, cx, tgt, target, name, null_aware, short_circuit, None);
                    (t, t, None)
                }
                ExprKind::Index { target, index, null_aware } => {
                    let (target, index, null_aware) = (*target, *index, *null_aware);
                    let (recv, c) = receptor(inf, cx, target, null_aware);
                    *short_circuit = c;
                    let raw = inf.body_types.units[cx.unit.0 as usize].get_type(target).unwrap_or(recv);
                    let bnd = inf.resolver_ao_limite(raw);
                    if never_receiver(inf, cx, target, bnd) {
                        infer_free(inf, cx, index);
                        let d = inf.core.dynamic_;
                        (d, d, None)
                    } else {
                        let t = index_write(inf, cx, tgt, recv, index, span);
                        (t, t, None)
                    }
                }
                _ => {
                    let t = infer_free(inf, cx, tgt);
                    (t, t, None)
                }
            };
            inf.body_types.units[cx.unit.0 as usize].tipos_de_escrita.insert(e, write);
            // `flow.write(…, rhs)` (`_write`, `flow_analysis.dart:6133-6149`):
            // a escrita de uma condição numa local guarda os modelos
            // verdadeiro/falso dela no nó SSA, como o inicializador (§7.10).
            let (tv, saved_condition) = if local.is_some() && is_condition_form(inf, cx, the_value) {
                let (yes, not_) = condicao(inf, cx, the_value);
                cx.flow = inf.juntar(&yes, &not_);
                let t = inf.body_types.units[cx.unit.0 as usize].get_type(the_value).unwrap_or(inf.core.bool_);
                (t, Some((yes, not_)))
            } else {
                (inferir(inf, cx, the_value, context), None)
            };
            if let Some(id) = local {
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                let reason = super::flow::NonPromotionReason::Escrita { nome: cx.local(id).nome, span: inf.span_expr(cx.unit, e) };
                inf.assign_flow(&mut f, id, decl, tv, Some(reason));
                cx.flow = f;
                cx.forget_fields_of(id);
                if let (Some((yes, not_)), Some(version)) = (saved_condition, cx.flow.versao(id)) {
                    cx.conditions.insert(id, (yes, not_, version));
                }
            }
            if !inf.e_dynamic(write) {
                verify_assignable_expr_at(inf, cx, the_value, tv, write, INVALID_ASSIGNMENT.template, false);
            } else {
                void_use(inf, cx, the_value, tv);
            }
            tv
        }
        AssignOp::Compound(bop) => {
            let (read, write, local) = read_for_write(inf, cx, tgt, short_circuit);
            inf.body_types.units[cx.unit.0 as usize].tipos_de_escrita.insert(e, write);
            // `x += 1` / `x ??= 3` com `x` de tipo `void`: no operador.
            if matches!(inf.table.get(read), Type::Void) {
                let token = operator_token(inf, cx, inf.span_expr(cx.unit, tgt).end);
                inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, token, &[]);
            }
            if bop == BinaryOp::IfNull {
                // `checkFinalAlreadyAssigned(left)` antes do lado direito
                // (3.6.2 `assignment_expression_resolver.dart:79`), com o
                // estado de antes da escrita.
                if let Some(id) = local {
                    let target_span = inf.span_expr(cx.unit, tgt);
                    check_final_local(inf, cx, id, target_span);
                }
                // O lado direito só roda se o alvo for nulo: o que ele promove
                // ou atribui não vale depois (`origin ??= element!.library;`
                // não promove `element`).
                let before = cx.flow.clone();
                // O índice sem `[]` resolvido (receptor `Never`) tem o
                // `readType` inválido (3.6.2 `resolver.dart:1665-1672`): o
                // `isStrictlyNonNullable` não vale.
                // O mesmo para o nome que não é getter nem variável (classe,
                // alias, função, método, parâmetro de tipo: `C ??= null`).
                let without_read = match &ast(inf, cx).expr(tgt).kind {
                    ExprKind::Index { target, null_aware: false, .. } => inf.body_types.units[cx.unit.0 as usize]
                        .get_type(*target)
                        .is_some_and(|r| matches!(inf.table.get(inf.resolver_ao_limite(r)), Type::Never)),
                    ExprKind::Identifier(_) | ExprKind::Property { .. } => {
                        let is_method = |inf: &BodyInferrer<'_>, f: dartforge_elements::model::FunctionElementId| {
                            matches!(inf.program.function(f).kind, FunctionKind::Function)
                        };
                        match inf.body_types.units[cx.unit.0 as usize].get_resolved(tgt).cloned() {
                            Some(Resolved::Element(Element::Function(f))) => is_method(inf, f),
                            Some(Resolved::Element(Element::Variable(_))) => false,
                            Some(Resolved::Element(_)) => true,
                            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => is_method(inf, f),
                            Some(Resolved::ExtensionMember { member: f, .. }) => is_method(inf, f),
                            Some(Resolved::TypeParameter(_) | Resolved::Constructor(_) | Resolved::Prefix(_)) => true,
                            _ => false,
                        }
                    }
                    _ => false,
                };
                if !without_read {
                    warn_dead_null(inf, cx, read, the_value);
                }
                let tv = inferir(inf, cx, the_value, write);
                if !matches!(inf.table.get(write), Type::Void) {
                    void_use(inf, cx, the_value, tv);
                }
                // `x ??= e`: o tipo de `e` contra o tipo de escrita, no lado
                // direito (`_checkForInvalidAssignment`,
                // an611:src/dart/resolver/assignment_expression_resolver.dart:117-155, 263-270).
                if !inf.e_dynamic(write) && !matches!(inf.table.get(tv), Type::Void) {
                    let sp = inf.span_expr(cx.unit, the_value);
                    inf.verify_assignable(tv, write, sp, INVALID_ASSIGNMENT.template);
                }
                let nn = inf.non_null(read);
                let t = inf.up(nn, tv);
                // A escrita acontece só no ramo do nulo (`ifNullExpression_
                // rightBegin` … `write` … `ifNullExpression_end`): depois da
                // junção o local fica só potencialmente atribuído se não
                // estava antes.
                if let Some(id) = local {
                    let decl = cx.local(id).tipo;
                    let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                    let reason = super::flow::NonPromotionReason::Escrita { nome: cx.local(id).nome, span: inf.span_expr(cx.unit, e) };
                    inf.assign_flow(&mut f, id, decl, tv, Some(reason));
                    cx.flow = f;
                }
                let after = cx.flow.clone();
                // No ramo sem o lado direito o alvo não é nulo.
                let mut before = before;
                if let Some(id) = local {
                    let decl = cx.local(id).tipo;
                    inf.promote_non_null(&mut before, id, decl);
                }
                cx.flow = inf.juntar(&before, &after);
                return t;
            }
            let sym = operator_symbol(inf, bop);
            let token = operator_token(inf, cx, inf.span_expr(cx.unit, tgt).end);
            // O contexto do lado direito: `refineNumericInvocationContext`
            // com o tipo de escrita como alvo e como contexto.
            let positions = OperatorPositions { token, undefined: token, compound: true, super_: false, numeric_target: Some(write) };
            let (t, _) = operador_binario(inf, cx, read, sym, the_value, write, positions, Some(e));
            // `x op= e`: o retorno do operador contra o tipo de escrita, no
            // lado direito (`_resolveTypes` + `_checkForInvalidAssignment`,
            // an611:src/dart/resolver/assignment_expression_resolver.dart:117-155, 272-290).
            if !inf.e_dynamic(write) && !inf.e_dynamic(read) && !matches!(inf.table.get(t), Type::Void) && (local.is_some() || read == write) {
                let sp = inf.span_expr(cx.unit, the_value);
                inf.verify_assignable(t, write, sp, INVALID_ASSIGNMENT.template);
            }
            if let Some(id) = local {
                // A regra de escrita vê o estado de antes da escrita.
                let target_span = inf.span_expr(cx.unit, tgt);
                check_final_local(inf, cx, id, target_span);
                let decl = cx.local(id).tipo;
                let mut f = std::mem::replace(&mut cx.flow, Fluxo::alcancavel());
                let reason = super::flow::NonPromotionReason::Escrita { nome: cx.local(id).nome, span: inf.span_expr(cx.unit, e) };
                inf.assign_flow(&mut f, id, decl, t, Some(reason));
                cx.flow = f;
            }
            t
        }
    }
}

/// `r.x = …`: tipo do setter.
fn property_write(inf: &mut BodyInferrer<'_>, cx: &mut Body, tgt: ExprId, target: ExprId, name: ast::Name, null_aware: bool, short_circuit: &mut bool, recv_read: Option<(TypeId, bool)>) -> TypeId {
    // `Fn<int>.foo = 1`: o setter sem resolução possível; na composta só o
    // getter relata (`hasRead`), já na leitura.
    if let ExprKind::TypeArguments { target: base, .. } = &ast(inf, cx).expr(target).kind
        && function_type_alias(inf, cx, *base)
    {
        if recv_read.is_none() {
            function_type_literal(inf, cx, target, name, false);
        }
        return inf.table.invalid(inf.core.dynamic_);
    }
    let a = ast(inf, cx);
    if let ExprKind::Identifier(p) = &a.expr(target).kind {
        if matches!(resolve_name(inf, cx, p.sym, false), NameRef::Prefixo) {
            if null_aware {
                prefix_without_dot(inf, *p);
            }
            resolver(inf, cx, target, Resolved::Prefix(cx.lib));
            if ambiguous_import(inf, cx, Some(p.sym), name) {
                return inf.table.invalid(inf.core.dynamic_);
            }
            if let Some(el) = inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.setter.or(b.getter)) {
                resolver(inf, cx, tgt, Resolved::Element(el));
                return match el {
                    Element::Variable(v) => inf.variable_type(v),
                    Element::Function(f) => {
                        let fe = inf.program.function(f);
                        match (fe.kind, fe.variable) {
                            (FunctionKind::ImplicitAccessor, Some(v)) => inf.variable_type(v),
                            _ => inf.outline.functions[f.0 as usize].parameters.first().map(|p| p.ty).unwrap_or(inf.core.dynamic_),
                        }
                    }
                    _ => inf.core.dynamic_,
                };
            }
            return inf.core.dynamic_;
        }
    }
    if null_aware && type_reference(inf, cx, target).is_some() {
        null_operator_on_type(inf, cx, target);
    }
    if let Some(rt) = type_reference(inf, cx, target) {
        record_type_ref(inf, cx, target);
        if null_aware && !shortening_waived(inf, cx, target, &rt) {
            *short_circuit = true;
        }
        let explicit_instance = matches!(&ast(inf, cx).expr(target).kind, ExprKind::TypeArguments { .. });
        if explicit_instance {
            if let TypeRef::Classe(c, _) | TypeRef::Alias(c, _, _) = &rt {
                if inf.static_member(*c, name.sym, true).is_some()
                    || inf.static_member(*c, name.sym, false).is_some()
                {
                    warn_static_instantiation(inf, cx, tgt, name);
                    return inf.core.dynamic_;
                }
                if warn_class_instantiation(inf, cx, tgt, *c, name, true) {
                    return inf.core.dynamic_;
                }
            }
        }
        let m = match &rt {
            TypeRef::Classe(c, _) | TypeRef::Alias(c, _, _) => inf.static_member(*c, name.sym, true),
            TypeRef::Extensao(x) => inf.extension_static_member(*x, name.sym, true),
        };
        return match m {
            Some(m) => {
                // O setter estático privado de outra biblioteca: achado, mas
                // inacessível (`PRIVATE_SETTER`, 3.6.2
                // `property_element_resolver.dart:710-719`).
                if let TypeRef::Classe(c, _) | TypeRef::Alias(c, _, _) = &rt
                    && inf.interner.resolve(name.sym).starts_with('_')
                    && inf.program.class(*c).library != cx.lib
                {
                    let text = inf.interner.resolve(name.sym).to_string();
                    inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::PRIVATE_SETTER, name.span, &[&text]);
                }
                resolver(inf, cx, tgt, m.resolved.clone());
                m.tipo
            }
            None => {
                if let TypeRef::Classe(c, _) | TypeRef::Alias(c, _, _) = &rt {
                    if inf.static_member(*c, name.sym, false).is_none() {
                        // Um método de instância não declara setter. Em `C.m = v`,
                        // o analyzer procura o setter estático e relata sua
                        // ausência, enquanto `C.m` como leitura é acesso
                        // estático indevido ao método de instância.
                        let cls = inf.program.class(*c);
                        let instance_setter = inf.setter_key(name.sym)
                            .is_some_and(|the_key| cls.instance_members.contains_key(&the_key));
                        let instance_method = cls.instance_members.get(&name.sym)
                            .is_some_and(|&f| inf.program.function(f).kind == FunctionKind::Function);
                        if !explicit_instance && !instance_setter && instance_method
                            && (!inf.interner.resolve(name.sym).starts_with('_') || cls.library == cx.lib)
                        {
                            let msg = format!(
                                "{}: setter '{}' não definido para o tipo '{}'",
                                UNDEFINED_SETTER.template,
                                inf.interner.resolve(name.sym),
                                inf.interner.resolve(cls.name),
                            );
                            inf.aviso(msg, name.span);
                            return inf.core.dynamic_;
                        }
                        if !explicit_instance && warn_static_access_to_instance(inf, cx, *c, name, true) {
                            return inf.core.dynamic_;
                        }
                    }
                }
                if let TypeRef::Extensao(x) = rt {
                    if inf.explicit_extension_member(x, &[], name.sym, true).is_some() {
                        let msg = format!("{}: '{}'", STATIC_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
                        inf.aviso(msg, name.span);
                        return inf.core.dynamic_;
                    }
                }
                let getter = match rt {
                    TypeRef::Classe(c, _) | TypeRef::Alias(c, _, _) => inf.static_member(c, name.sym, false),
                    TypeRef::Extensao(x) => inf.extension_static_member(x, name.sym, false),
                };
                let missing_getter = getter.is_none();
                if let Some(getter) = getter {
                    if let Some(f) = getter.funcao {
                        if warn_member_without_setter(inf, name, f) {
                            resolver(inf, cx, tgt, getter.resolved);
                            return getter.tipo;
                        }
                    }
                }
                if missing_getter && let TypeRef::Extensao(x) = rt {
                    let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                    let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_SETTER.template, inf.interner.resolve(name.sym), extension);
                    inf.aviso(msg, name.span);
                }
                // `_resolveTargetInterfaceElement` sem setter nem getter
                // (`augmented.getGetter`, que também vê os de instância): o
                // `AssignmentVerifier` relata `UNDEFINED_SETTER` com o
                // `thisType` da classe (3.6.2 `property_element_resolver.dart:708-731`,
                // `assignment_verifier.dart:96-106`); os estáticos da
                // superclasse não contam (`C.s = 1`), e `C.new = 1` também.
                if missing_getter
                    && !explicit_instance
                    && let TypeRef::Classe(c, _) = rt
                {
                    let cls = inf.program.class(c);
                    let of_instance = cls.instance_members.contains_key(&name.sym)
                        || inf.setter_key(name.sym).is_some_and(|ch| cls.instance_members.contains_key(&ch));
                    if !of_instance && name.span.start != name.span.end {
                        let this = inf.class_this_type(c);
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
        return super_member(inf, cx, tgt, name, SuperUse::Escrita);
    }
    // Na escrita composta (`x.p += 1`) o receptor já foi lido, e a checagem
    // de nulo saiu na leitura (uma só resolução no analyzer).
    let compound = recv_read.is_some();
    let (recv, c) = recv_read.unwrap_or_else(|| receptor(inf, cx, target, null_aware));
    *short_circuit = c;
    // Receptor `void` (`property_element_resolver.dart:449-455`): no nome.
    if !cx.overlays.contains_key(&target) && matches!(inf.table.get(recv), Type::Void) {
        inf.warning_with_code(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, name.span, &[]);
        return inf.core.dynamic_;
    }
    // `x.new = v` (`type_property_resolver.dart:75-79`): sem setter `new`.
    if !cx.overlays.contains_key(&target) && inf.interner.resolve(name.sym) == "new" {
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
    if let Some((x, args)) = cx.overlays.get(&target).cloned() {
        if let Some(m) = inf.explicit_extension_member(x, &args, name.sym, true) {
            resolver(inf, cx, tgt, m.resolved);
            return m.tipo;
        }
        if inf.extension_static_member(x, name.sym, true).is_some() {
            // `+=` resolve leitura e escrita do mesmo nome. O analyzer relata
            // o acesso estático uma vez, mesmo quando ambos os lados resolvem.
            let already_reported = inf.diagnostics.iter().zip(&inf.warning_units).any(|(d, unit)| {
                *unit == inf.current_unit
                    && d.span == name.span
                    && d.message == EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template
            });
            if !already_reported {
                inf.aviso(EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(), name.span);
            }
            return inf.core.dynamic_;
        }
        let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
        let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_SETTER.template, inf.interner.resolve(name.sym), extension);
        inf.aviso(msg, name.span);
        return inf.core.dynamic_;
    }
    // Receptor anulável sem o getter nem o setter em `Object` ou numa
    // extensão do tipo anulável: `UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE`
    // (o `_hasGetterOrSetter` do `TypePropertyResolver.resolve`,
    // an362:src/dart/resolver/type_property_resolver.dart:99-180), e então
    // nada de `undefined_setter`; a resolução segue pelo tipo não anulável.
    let check_null = !compound
        && inf.requires_null_check(cx.lib, recv, name.sym, false)
        && inf.requires_null_check(cx.lib, recv, name.sym, true);
    if check_null {
        let the_name = inf.interner.resolve(name.sym).to_string();
        let since = inf.diagnostics.len();
        inf.null_warning(
            recv,
            dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE,
            name.span,
            &[&the_name],
        );
        inf.attach_non_promotion(since, cx, Some(target), name.span);
    }
    // Um método da interface da classe prevalece sobre setter de extensão homônimo.
    if let Some(m) = inf.interface_member(recv, name.sym, false) {
        if m.metodo && m.funcao.is_some_and(|f| inf.program.function(f).kind == FunctionKind::Function) {
            warn_write_to_method(inf, name);
            resolver(inf, cx, tgt, m.resolved);
            // `setWriteElement`: escrita num método tem tipo `InvalidType`.
            return inf.table.invalid(inf.core.dynamic_);
        }
    }
    let search = inf.lookup_member(cx.lib, recv, name.sym, true);
    inf.report_extension_ambiguity(name.span);
    match search {
        Busca::Achado(m) => {
            resolver(inf, cx, tgt, m.resolved.clone());
            m.tipo
        }
        Busca::Missing => {
            // O setter estático recuperado (`lookupStaticSetter`).
            if let Some(m) = inf.instance_access_to_static(cx.lib, recv, name.sym, true, name.span) {
                resolver(inf, cx, tgt, m.resolved);
                return m.tipo;
            }
            if check_null {
                return inf.core.dynamic_;
            }
            // O analyzer recupera o getter quando a escrita não encontra um
            // setter (`AssignmentVerifier.verify` com o `result.getter`, que
            // pode ser o estático recuperado). Um getter declarado numa classe
            // tem diagnóstico próprio; sem getter, continua sendo um setter
            // indefinido.
            let getter = match inf.lookup_member(cx.lib, recv, name.sym, false) {
                Busca::Achado(getter) => Some(getter),
                Busca::Missing => match inf.receiver_static_recovery(cx.lib, recv, name.sym, false) {
                    Some((owner, true)) => {
                        let m = inf.static_member(owner, name.sym, false);
                        warn_write_to_method(inf, name);
                        if let Some(m) = m {
                            resolver(inf, cx, tgt, m.resolved);
                            return inf.table.invalid(inf.core.dynamic_);
                        }
                        return inf.core.dynamic_;
                    }
                    Some((owner, false)) => inf.static_member(owner, name.sym, false),
                    None => None,
                },
                _ => None,
            };
            if let Some(getter) = getter {
                if let Some(f) = getter.funcao {
                    if warn_member_without_setter(inf, name, f) {
                        resolver(inf, cx, tgt, getter.resolved);
                        return getter.tipo;
                    }
                } else if let Resolved::Member { member: MemberRef::Variable(v), .. } = getter.resolved {
                    // A constante de enum (`static const`): `ASSIGNMENT_TO_CONST`.
                    if inf.program.variable(v).const_ {
                        inf.aviso(ASSIGNMENT_TO_CONST.template.to_string(), name.span);
                        resolver(inf, cx, tgt, getter.resolved);
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
            resolver(inf, cx, tgt, Resolved::Dynamic);
            inf.core.dynamic_
        }
    }
}

/// `r[i] = …`: tipo do valor em `[]=`.
fn index_write(inf: &mut BodyInferrer<'_>, cx: &mut Body, tgt: ExprId, recv: TypeId, index: ExprId, span: dartforge_diagnostics::Span) -> TypeId {
    let target = match &ast(inf, cx).expr(tgt).kind {
        ExprKind::Index { target, .. } => Some(*target),
        _ => None,
    };
    let search = if let Some(target) = target
        && let Some((x, args)) = cx.overlays.get(&target).cloned()
    {
        match inf.sym.set_index.and_then(|op| inf.explicit_extension_member(x, &args, op, false)) {
            Some(m) => Busca::Achado(m),
            None => {
                infer_free(inf, cx, index);
                let sp = index_span(inf, cx, tgt, target);
                warn_extension_operator(inf, x, "[]=", sp);
                return inf.core.dynamic_;
            }
        }
    } else {
        let Some(op) = inf.sym.set_index else {
            infer_free(inf, cx, index);
            return inf.core.dynamic_;
        };
        // `resolveIndexExpression` (`property_element_resolver.dart:80-115`):
        // o `TypePropertyResolver` com o nome `[]` (que procura `[]` e
        // `[]=`); receptor potencialmente anulável é
        // `UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE` `['[]']` no `[`
        // (o `IndexExpression` é `MethodReferenceExpression`).
        if let Some(t) = target
            && let Some(the_read) = inf.interner.lookup("[]")
            && inf.requires_null_check(cx.lib, recv, the_read, false)
            && inf.requires_null_check(cx.lib, recv, op, false)
        {
            let start = index_span(inf, cx, tgt, t).start;
            let sp = dartforge_diagnostics::Span { start: start, end: start + 1 };
            let since = inf.diagnostics.len();
            inf.null_warning(recv, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE, sp, &["[]"]);
            inf.attach_non_promotion(since, cx, Some(t), sp);
        }
        // `super[i] = v`: o `[]=` da cadeia de `super`.
        if target.is_some_and(|t| matches!(ast(inf, cx).expr(t).kind, ExprKind::Super)) {
            lookup_super_operator(inf, cx, op)
        } else {
            inf.lookup_member(cx.lib, recv, op, false)
        }
    };
    match search {
        Busca::Achado(m) => {
            resolver(inf, cx, tgt, m.resolved.clone());
            let (pi, pv) = match inf.table.get(m.tipo).clone() {
                Type::Function { positional, .. } => (positional.first().copied(), positional.get(1).copied()),
                _ => (None, None),
            };
            let u = inf.core.unknown;
            let ti = inferir(inf, cx, index, pi.unwrap_or(u));
            if let Some(p) = pi {
                verify_assignable_expr(inf, cx, index, ti, p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
            }
            pv.unwrap_or(inf.core.dynamic_)
        }
        Busca::Missing => {
            infer_free(inf, cx, index);
            // `Never?` em `x?[i] = v`: promovido a `Never`, nada a relatar.
            let nn = inf.non_null(recv);
            if matches!(inf.table.get(nn), Type::Never) {
                return inf.core.dynamic_;
            }
            // `[]=` ausente: `UNDEFINED_OPERATOR` (ou `…_SUPER_OPERATOR`) do
            // `[` ao `]` (`_reportUnresolvedIndex`,
            // an611:src/dart/resolver/property_element_resolver.dart:109-133, 365-381).
            let typ = inf.table.format(recv, inf.interner, inf.program);
            let (sp, code) = match target {
                Some(t) => {
                    let sup = matches!(ast(inf, cx).expr(t).kind, ExprKind::Super);
                    let c = if sup {
                        dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_SUPER_OPERATOR
                    } else {
                        dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR
                    };
                    (index_span(inf, cx, tgt, t), c)
                }
                None => (span, dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_OPERATOR),
            };
            // Composta sem `[]` nem `[]=`: o oráculo só relata o `[]=` (o
            // `[]` da leitura, no mesmo intervalo, sai).
            let mut i = 0;
            while i < inf.diagnostics.len() {
                let d = &inf.diagnostics[i];
                if d.span == sp && d.code == Some(code) && d.args.first().is_some_and(|a| &**a == "[]") {
                    inf.diagnostics.remove(i);
                    inf.warning_units.remove(i);
                } else {
                    i += 1;
                }
            }
            inf.warning_with_code(code, sp, &["[]=", &typ]);
            inf.core.dynamic_
        }
        _ => {
            infer_free(inf, cx, index);
            inf.core.dynamic_
        }
    }
}

// -------------------------------------------------------------------
// Condições: modelos de fluxo verdadeiro/falso
// -------------------------------------------------------------------

/// Infere uma condição (contexto `bool`) e devolve `(true(E), false(E))`.
/// Deixa `cx.fluxo` no estado de antes (o chamador escolhe o ramo).
pub(crate) fn condicao(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId) -> (Fluxo, Fluxo) {
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
            let f = cx.flow.clone();
            if b {
                (f.clone(), f.unreachable())
            } else {
                (f.unreachable(), f)
            }
        }
        ExprKind::Unary { op: UnaryOp::Not, operand } => {
            let o = *operand;
            let (v, f) = condicao(inf, cx, o);
            verify_bool(inf, cx, o, BoolUse::Negation);
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            (f, v)
        }
        ExprKind::Binary { op, left, right } if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::NotEq) => {
            let (op, left, right) = (*op, *left, *right);
            let r = binary_condition(inf, cx, e, op, left, right);
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            r
        }
        ExprKind::Is { value, ty, negated } => {
            let (value, ty, negated) = (*value, *ty, *negated);
            let r = type_test(inf, cx, value, ty, negated, span);
            let t = inf.core.bool_;
            registrar(inf, cx, e, t);
            r
        }
        _ => {
            let b = inf.core.bool_;
            inferir(inf, cx, e, b);
            let f = cx.flow.clone();
            // Leitura de variável de condição não reescrita (§7.10).
            if let ExprKind::Identifier(n) = &ast(inf, cx).expr(e).kind
                && let Some(Nome::Local(id)) = cx.buscar(n.sym)
                && let Some((yes, not_, version)) = cx.conditions.get(&id).cloned()
                && f.versao(id) == Some(version)
                && !f.modelo(id).is_some_and(|m| m.captured)
            {
                let v = inf.reapply(&f, &yes);
                let fa = inf.reapply(&f, &not_);
                return (v, fa);
            }
            (f.clone(), f)
        }
    }
}

/// Expressão cuja informação de condição não é trivial (`==`, `!=`, `&&`,
/// `||`, `!`, `is`), para guardar numa variável de condição.
pub(crate) fn is_condition_form(inf: &BodyInferrer<'_>, cx: &Body, e: ExprId) -> bool {
    match &ast(inf, cx).expr(e).kind {
        ExprKind::Parenthesized(i) => is_condition_form(inf, cx, *i),
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
pub(crate) fn checked_condition(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId) -> (Fluxo, Fluxo) {
    let r = condicao(inf, cx, e);
    verify_bool(inf, cx, e, BoolUse::Condition);
    r
}

/// Condição de `assert` (comando ou inicializador).
pub(crate) fn assert_condition(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId) -> (Fluxo, Fluxo) {
    let r = condicao(inf, cx, e);
    verify_bool(inf, cx, e, BoolUse::Assert);
    r
}

/// Onde uma expressão precisa ser `bool` (o `BoolExpressionVerifier` do
/// analyzer escolhe o código por aí).
#[derive(Clone, Copy)]
enum BoolUse {
    /// `if`, `while`, `do`, `for`, `?:`, `when`: `NON_BOOL_CONDITION`.
    Condition,
    /// `assert`: `NON_BOOL_EXPRESSION`.
    Assert,
    /// Operando de `&&`/`||`: `NON_BOOL_OPERAND`.
    Operand(&'static str),
    /// Operando de `!`: `NON_BOOL_NEGATION_EXPRESSION`.
    Negation,
}

/// `checkForNonBoolExpression`: tipo não atribuível a `bool`. Com o próprio
/// `bool` anulável (`bool?`), o erro é de uso de valor anulável como
/// condição, em qualquer dos usos. Com `void`, o analyzer relata o uso do
/// resultado `void` no lugar (fora daqui); nada sai.
fn verify_bool(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, use_: BoolUse) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let t = inf.body_types.units[cx.unit.0 as usize].get_type(e).unwrap_or(inf.core.bool_);
    let b = inf.core.bool_;
    if void_use(inf, cx, e, t) || inf.atribuivel(t, b) {
        return;
    }
    let sp = inf.span_expr(cx.unit, e);
    let e_bool = match (inf.table.get(t), inf.table.get(b)) {
        (Type::Interface { class: c1, .. }, Type::Interface { class: c2, .. }) => c1 == c2,
        _ => false,
    };
    if e_bool {
        let since = inf.diagnostics.len();
        inf.warning_with_code(c::UNCHECKED_USE_OF_NULLABLE_VALUE_AS_CONDITION, sp, &[]);
        inf.attach_non_promotion(since, cx, Some(e), sp);
        return;
    }
    match use_ {
        BoolUse::Condition => inf.aviso(NON_BOOL_CONDITION.template.to_string(), sp),
        BoolUse::Negation => inf.aviso(NON_BOOL_NEGATION_EXPRESSION.template.to_string(), sp),
        BoolUse::Assert => inf.warning_with_code(c::NON_BOOL_EXPRESSION, sp, &[]),
        BoolUse::Operand(op) => inf.warning_with_code(c::NON_BOOL_OPERAND, sp, &[op]),
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
pub(crate) fn nullable_dereference(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, t: TypeId, code: dartforge_diagnostics::Codigo) {
    if inf.e_dynamic(t) || inf.table.e_invalido(t) || inf.is_non_nullable(t) {
        return;
    }
    let sp = inf.span_expr(cx.unit, e);
    let since = inf.diagnostics.len();
    inf.null_warning(t, code, sp, &[]);
    // `_check`: `computeWhyNotPromotedMessages(errorNode, whyNotPromoted(errorNode))`.
    inf.attach_non_promotion(since, cx, Some(e), sp);
}

/// `isDoubleNan` (`an611:src/error/best_practices_verifier.dart:2093-2100`):
/// o `PrefixedIdentifier` `double.nan`, só pela forma escrita.
pub(crate) fn e_double_nan(inf: &BodyInferrer<'_>, cx: &Body, x: ExprId) -> bool {
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
fn comparison_with_nan(inf: &mut BodyInferrer<'_>, cx: &Body, op: BinaryOp, left: ExprId, right: ExprId) {
    use dartforge_diagnostics::codigos::warning as w;
    let code = match op {
        BinaryOp::NotEq => w::UNNECESSARY_NAN_COMPARISON_TRUE,
        BinaryOp::Eq => w::UNNECESSARY_NAN_COMPARISON_FALSE,
        _ => return,
    };
    let (sl, sr) = (inf.span_expr(cx.unit, left), inf.span_expr(cx.unit, right));
    let operator = operator_token(inf, cx, sl.end);
    if e_double_nan(inf, cx, left) {
        inf.warning_with_code(code, dartforge_diagnostics::Span { start: sl.start, end: operator.end }, &[]);
    } else if e_double_nan(inf, cx, right) {
        inf.warning_with_code(code, dartforge_diagnostics::Span { start: operator.start, end: sr.end }, &[]);
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
fn null_free_api(inf: &mut BodyInferrer<'_>, cx: &Body, e: ExprId, result: TypeId) {
    let a = ast(inf, cx);
    let argument_of = |inf: &BodyInferrer<'_>, t: TypeId, cls: Option<ClassId>| -> Option<TypeId> {
        match inf.table.get(t) {
            Type::Interface { class, args, .. } if Some(*class) == cls && args.len() == 1 => Some(args[0]),
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
    let future = inf.core.future_class;
    let (the_name, typ, args): (&str, TypeId, &ast::Arguments) = match &a.expr(e).kind {
        ExprKind::InstanceCreation { constructor: Some(n), arguments, .. } if inf.interner.resolve(n.sym) == "value" => {
            let Some(x) = argument_of(inf, result, future) else { return };
            ("Future.value", x, arguments)
        }
        ExprKind::Call { target, arguments } => {
            let ExprKind::Property { target: receiver, name, .. } = &a.expr(*target).kind else { return };
            let bt = &inf.body_types.units[cx.unit.0 as usize];
            match bt.get_resolved(*target) {
                Some(Resolved::Constructor(f)) => {
                    if inf.interner.resolve(name.sym) != "value" || inf.program.function(*f).class != future {
                        return;
                    }
                    let Some(x) = argument_of(inf, result, future) else { return };
                    ("Future.value", x, arguments)
                }
                _ if inf.interner.resolve(name.sym) == "complete" => {
                    let Some(rt) = bt.get_type(*receiver) else { return };
                    let Some(x) = argument_of(inf, rt, completer) else { return };
                    ("Completer.complete", x, arguments)
                }
                _ => return,
            }
        }
        _ => return,
    };
    if args.args.len() > 1 || !inf.is_non_nullable(typ) {
        return;
    }
    let argument = args.args.first().map(|x| x.value);
    let argument_type = argument.map(|x| inf.body_types.units[cx.unit.0 as usize].get_type(x));
    let null = match argument_type {
        None => true,
        Some(None) => return,
        Some(Some(t)) => matches!(inf.table.get(t), Type::Null),
    };
    if null {
        let mut sp = inf.span_expr(cx.unit, argument.unwrap_or(e));
        // A seção de cascata (`c..complete()`): o `MethodInvocation` sem alvo
        // começa no operador `..`/`?..`.
        if argument.is_none()
            && let ExprKind::Call { target, .. } = &a.expr(e).kind
            && let ExprKind::Property { target: receiver, .. } = &a.expr(*target).kind
            && matches!(a.expr(*receiver).kind, ExprKind::CascadeTarget)
        {
            let source = inf.program.unit(cx.unit).source.as_bytes();
            let mut i = sp.start;
            while i > 0 && source[i - 1].is_ascii_whitespace() {
                i -= 1;
            }
            if i >= 2 && &source[i - 2..i] == b".." {
                i -= 2;
                if i >= 1 && source[i - 1] == b'?' {
                    i -= 1;
                }
                sp.start = i;
            }
        }
        let displayed = inf.table.format_sem_alias(typ, inf.interner, inf.program);
        inf.warning_with_code(dartforge_diagnostics::codigos::warning::NULL_ARGUMENT_TO_NON_NULL_TYPE, sp, &[the_name, &displayed]);
    }
}

fn comparison_with_null(inf: &mut BodyInferrer<'_>, cx: &Body, op: BinaryOp, left: ExprId, right: ExprId, tl: TypeId, tr: TypeId) {
    use dartforge_diagnostics::codigos::warning as w;
    let a = ast(inf, cx);
    let null = |x: ExprId| matches!(a.expr(x).kind, ExprKind::Null);
    let (sl, sr) = (inf.span_expr(cx.unit, left), inf.span_expr(cx.unit, right));
    let operator = operator_token(inf, cx, sl.end);
    // `===`/`!==` (`UNSUPPORTED_OPERATOR`): o nó não é uma comparação de
    // igualdade para o `_checkForUnnecessaryNullComparison`.
    if inf.program.unit(cx.unit).source.get(operator.start..).is_some_and(|s| s.starts_with("===") || s.starts_with("!==")) {
        return;
    }
    let different = op == BinaryOp::NotEq;
    let unassigned_local = |inf: &mut BodyInferrer<'_>, x: ExprId| match &ast(inf, cx).expr(x).kind {
        ExprKind::Identifier(_) => match inf.body_types.units[cx.unit.0 as usize].get_resolved(x) {
            Some(Resolved::Local(id)) => cx.flow.unassigned(*id),
            _ => false,
        },
        _ => false,
    };
    let always_null = if different { w::UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_FALSE } else { w::UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_TRUE };
    if null(right) && unassigned_local(inf, left) {
        inf.warning_with_code(always_null, dartforge_diagnostics::Span { start: sl.start, end: operator.end }, &[]);
    } else if null(left) && unassigned_local(inf, right) {
        inf.warning_with_code(always_null, dartforge_diagnostics::Span { start: operator.start, end: sr.end }, &[]);
    }
    let never_null = if different { w::UNNECESSARY_NULL_COMPARISON_NEVER_NULL_TRUE } else { w::UNNECESSARY_NULL_COMPARISON_NEVER_NULL_FALSE };
    if null(left) && strictly_non_nullable(inf, tr) {
        inf.warning_with_code(never_null, dartforge_diagnostics::Span { start: sl.start, end: operator.end }, &[]);
    }
    if null(right) && strictly_non_nullable(inf, tl) {
        inf.warning_with_code(never_null, dartforge_diagnostics::Span { start: operator.start, end: sr.end }, &[]);
    }
}

fn binary_condition(inf: &mut BodyInferrer<'_>, cx: &mut Body, e: ExprId, op: BinaryOp, left: ExprId, right: ExprId) -> (Fluxo, Fluxo) {
    match op {
        BinaryOp::And | BinaryOp::Or => {
            let before = cx.flow.clone();
            let symbol = if op == BinaryOp::And { "&&" } else { "||" };
            let (lv, lf) = condicao(inf, cx, left);
            verify_bool(inf, cx, left, BoolUse::Operand(symbol));
            cx.flow = if op == BinaryOp::And { lv.clone() } else { lf.clone() };
            // O código morto do lado direito começa no operador.
            let start = {
                let the_left_end = inf.span_expr(cx.unit, left).end;
                let right_start = inf.span_expr(cx.unit, right).start;
                let snippet = inf.program.unit(cx.unit).source.get(the_left_end..right_start).unwrap_or("");
                the_left_end + skip_whitespace_and_comments(snippet)
            };
            let (rv, rf) = flow_operand(inf, cx, right, Some(start), |inf, cx| condicao(inf, cx, right));
            verify_bool(inf, cx, right, BoolUse::Operand(symbol));
            cx.flow = before;
            if op == BinaryOp::And {
                let false_ = inf.juntar(&lf, &rf);
                (rv, false_)
            } else {
                let true_ = inf.juntar(&lv, &rv);
                (true_, rf)
            }
        }
        _ => {
            // `==` / `!=`
            let u = inf.core.unknown;
            let tl = inferir(inf, cx, left, u);
            void_use(inf, cx, left, tl);
            // `e == .x`: o atalho à direita usa o tipo de `e` (3.10).
            shorthands::record_equality(inf, cx, right, tl);
            dead_in_right_operand(inf, cx, left, right);
            let tr = inferir(inf, cx, right, u);
            // O lado direito é o argumento de `operator ==(Object)`.
            void_use(inf, cx, right, tr);
            never_receiver(inf, cx, left, tl);
            comparison_with_null(inf, cx, op, left, right, tl, tr);
            comparison_with_nan(inf, cx, op, left, right);
            if matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, left, this);
            }
            // `E(x) == y`: o `==` só na extensão (`_resolveUserDefinableElement`
            // com `ExtensionOverride`); ausente, `UNDEFINED_EXTENSION_OPERATOR`
            // no operador, com o nome `==` também para `!=`.
            if let Some((x, args)) = cx.overlays.get(&left).cloned()
                && let Some(eq) = inf.sym.igual
                && inf.explicit_extension_member(x, &args, eq, false).is_none()
            {
                let token = operator_token(inf, cx, inf.span_expr(cx.unit, left).end);
                let extension = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_OPERATOR.template, "==", extension);
                inf.aviso(msg, token);
            }
            // `super == x`: o `==` da cadeia de `super`, com o parâmetro
            // tornado anulável (`_resolveEqual`): `super == 'a'` contra
            // `==(covariant num other)` é `ARGUMENT_TYPE_NOT_ASSIGNABLE` de `num?`.
            if let Some(eq) = inf.sym.igual
                && matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super)
            {
                if let Busca::Achado(m) = lookup_super_operator(inf, cx, eq) {
                    let param = match inf.table.get(m.tipo) {
                        Type::Function { positional, .. } => positional.first().copied(),
                        _ => None,
                    };
                    if let Some(p) = param
                        && !matches!(inf.table.get(tr), Type::Void)
                    {
                        let p = inf.anulavel(p);
                        verify_assignable_expr(inf, cx, right, tr, p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                    }
                    resolver(inf, cx, e, m.resolved);
                }
            } else if let Some(eq) = inf.sym.igual {
                if let Busca::Achado(m) = inf.lookup_member(cx.lib, tl, eq, false) {
                    // O lado direito contra o parâmetro de `operator ==`
                    // tornado anulável (`binary_expression_resolver.dart:117-124`,
                    // `promoteParameterToNullable`), procurado no tipo
                    // esquerdo promovido a não nulo.
                    let tl_nn = inf.non_null(tl);
                    let mem = if tl_nn != tl {
                        match inf.lookup_member(cx.lib, tl_nn, eq, false) {
                            Busca::Achado(m2) => Some(m2),
                            _ => None,
                        }
                    } else {
                        Some(m.clone())
                    };
                    let param = mem.and_then(|m| match inf.table.get(m.tipo) {
                        Type::Function { positional, .. } => positional.first().copied(),
                        _ => None,
                    });
                    if let Some(p) = param
                        && !matches!(inf.table.get(tr), Type::Void)
                        && !cx.overlays.contains_key(&left)
                        && !matches!(inf.program.unit(cx.unit).ast.expr(left).kind, ExprKind::Super)
                    {
                        let p = inf.anulavel(p);
                        verify_assignable_expr(inf, cx, right, tr, p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                    }
                    resolver(inf, cx, e, m.resolved);
                }
            }
            let after = cx.flow.clone();
            let a = ast(inf, cx);
            let is_null = |x: ExprId| {
                let mut x = x;
                loop {
                    match &a.expr(x).kind {
                        ExprKind::Parenthesized(i) => x = *i,
                        ExprKind::Null => return true,
                        _ => return false,
                    }
                }
            };
            let tgt = if is_null(right) {
                promotion_target(inf, cx, left)
            } else if is_null(left) {
                promotion_target(inf, cx, right)
            } else {
                None
            };
            let _ = (tl, tr);
            // O alvo (um campo) pode ter sido criado agora: o fluxo corrente o tem.
            let after = if tgt.is_some() { cx.flow.clone() } else { after };
            let (mut the_equal, mut different) = (after.clone(), after.clone());
            if let Some(id) = tgt {
                let decl = cx.local(id).tipo;
                inf.promote_non_null(&mut different, id, decl);
                let _ = &mut the_equal;
            }
            // Fluxo sólido (3.9, `sound-flow-analysis`): comparar com `null`
            // uma expressão de tipo não anulável tem resultado conhecido — o
            // ramo "igual" é inalcançável (`flow-analysis.md`, `equalityOp`:
            // `equivalentToNull(T1)` e `T2` não anulável, ou o inverso). Antes
            // da 3.9 os dois ramos continuam alcançáveis (mixed mode).
            let solido = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::SoundFlowAnalysis);
            if solido {
                let null = |inf: &BodyInferrer<'_>, t: TypeId| matches!(inf.table.get(t), Type::Null);
                if (null(inf, tr) && inf.is_non_nullable(tl)) || (null(inf, tl) && inf.is_non_nullable(tr)) {
                    the_equal = the_equal.unreachable();
                }
            }
            if op == BinaryOp::Eq {
                (the_equal, different)
            } else {
                (different, the_equal)
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
fn unnecessary_type_test(
    inf: &mut BodyInferrer<'_>,
    cx: &Body,
    value: ExprId,
    v: TypeId,
    ty: ast::TypeId,
    t: TypeId,
    the_negated: bool,
    invalid: bool,
    span: dartforge_diagnostics::Span,
) {
    use dartforge_diagnostics::codigos::warning as w;
    // Operando ou tipo `InvalidType`: nenhum aviso de teste desnecessário
    // (`best_practices_verifier.dart:790`, `:801`).
    if invalid || inf.table.e_invalido(v) || inf.table.e_invalido(t) {
        return;
    }
    let written_name = match &ast(inf, cx).ty(ty).kind {
        ast::TypeKind::Named { name, .. } if name.len() == 1 && !ast(inf, cx).ty(ty).nullable => {
            Some(inf.interner.resolve(name[0].sym).to_string())
        }
        _ => None,
    };
    let code = if the_negated { w::UNNECESSARY_TYPE_CHECK_FALSE } else { w::UNNECESSARY_TYPE_CHECK_TRUE };
    if inf.e_dynamic(t) {
        // O `dynamic` de verdade: o escrito, ou o alias que chega a si mesmo
        // (`hasSelfReference` instancia como `DynamicType`).
        let self_referencing_alias = match &ast(inf, cx).ty(ty).kind {
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
        if written_name.as_deref() == Some("dynamic") || self_referencing_alias {
            inf.warning_with_code(code, span, &[]);
        }
        return;
    }
    // `is Never?` também é `isDartCoreNull` (o `Never?` escrito vira `Null`).
    let nullable_never = matches!(&ast(inf, cx).ty(ty).kind, ast::TypeKind::Named { name, .. }
        if ast(inf, cx).ty(ty).nullable && name.len() == 1 && inf.interner.resolve(name[0].sym) == "Never")
        && inf.table.canonico(t) == inf.core.null;
    if (t == inf.core.null && written_name.as_deref() == Some("Null")) || nullable_never {
        if matches!(ast(inf, cx).expr(value).kind, ExprKind::Null) {
            inf.warning_with_code(code, span, &[]);
        } else {
            let c = if the_negated { w::TYPE_CHECK_IS_NOT_NULL } else { w::TYPE_CHECK_IS_NULL };
            inf.warning_with_code(c, span, &[]);
        }
        return;
    }
    if inf.sub(v, t) {
        inf.warning_with_code(code, span, &[]);
    }
}

fn type_test(inf: &mut BodyInferrer<'_>, cx: &mut Body, value: ExprId, ty: ast::TypeId, the_negated: bool, span: dartforge_diagnostics::Span) -> (Fluxo, Fluxo) {
    let v = infer_free(inf, cx, value);
    void_use(inf, cx, value, v);
    let warnings_before = inf.diagnostics.len();
    let t = inf.type_in_context(cx, ty, crate::resolve::TypeContext::Is);
    // Tipo que não resolveu (`InvalidType` no analyzer): nada a dizer.
    let invalid = inf.diagnostics[warnings_before..].iter().any(|d| d.code.is_some());
    unnecessary_type_test(inf, cx, value, v, ty, t, the_negated, invalid, span);
    // `x is Never` não promove `x` (o ramo "sim" fica com o tipo de antes).
    let tgt = promotion_target(inf, cx, value).filter(|_| !matches!(inf.table.get(t), Type::Never));
    let after = cx.flow.clone();
    let (mut yes, mut not_) = (after.clone(), after.clone());
    if let Some(id) = tgt {
        let decl = cx.local(id).tipo;
        inf.promover(&mut yes, id, decl, t);
        let factored = factor(inf, v, t);
        // `tryPromoteForTypeCheck` (flow_analysis.dart:2544-2557): o fator
        // `Never` não promove (marcaria o ramo como inalcançável, e a
        // unsoundness de modo misto pode alcançá-lo), nem o fator igual ao
        // tipo anterior; o tipo testado é registrado de todo jeito.
        let promotes = !matches!(inf.table.get(factored), Type::Never) && factored != v;
        inf.promote_factored(&mut not_, id, promotes.then_some(factored), t);
    }
    // `e is Never` nunca é verdadeiro: o ramo "sim" é inalcançável (sem
    // promover o alvo).
    if matches!(inf.table.get(t), Type::Never) {
        yes = yes.unreachable();
    }
    if the_negated {
        (not_, yes)
    } else {
        (yes, not_)
    }
}

/// `factor(T, S)` (`flow-analysis.md`).
pub(crate) fn factor(inf: &mut BodyInferrer<'_>, t: TypeId, s: TypeId) -> TypeId {
    if inf.sub(t, s) {
        return inf.core.never;
    }
    let n = inf.core.null;
    match inf.table.get(t).clone() {
        ty if ty.is_declared_nullable() && !matches!(ty, Type::FutureOr { .. }) => {
            let r = inf.non_null(t);
            let f = factor(inf, r, s);
            if inf.sub(n, s) {
                f
            } else {
                inf.anulavel(f)
            }
        }
        Type::FutureOr { arg, nullable: false } => {
            let fut = inf.futuro(arg);
            if inf.sub(fut, s) {
                factor(inf, arg, s)
            } else if inf.sub(arg, s) {
                factor(inf, fut, s)
            } else {
                t
            }
        }
        _ => t,
    }
}

/// Declara um local (com tabela lateral de tipo por offset).
pub(crate) fn declarar_local(inf: &mut BodyInferrer<'_>, cx: &mut Body, local: Local, initialized: bool) -> LocalId {
    let (offset, typ) = (local.offset, local.tipo);
    let id = cx.declarar(local);
    if initialized {
        cx.flow.inicializar(id);
    }
    inf.body_types.units[cx.unit.0 as usize].set_local_type(offset, typ);
    id
}

impl<'a> BodyInferrer<'a> {
    /// Expressão constante (especificação, "Constants"), sobre a resolução
    /// já feita: literais, constantes referidas, construtores e coleções
    /// `const`, operadores sobre constantes, `?:`, interpolação, tipos.
    /// `alvo` (de uma chamada) é o construtor primário de um tipo de extensão
    /// declarado `const` (`extension type const E._(int v)`: `E._(1)`), que
    /// não é elemento e por isso não tem `Resolved::Constructor`.
    fn const_primary(&self, cx: &Body, tgt: ExprId) -> bool {
        let a = ast(self, cx);
        let bt = &self.body_types.units[cx.unit.0 as usize];
        let (cls, the_name) = match &a.expr(tgt).kind {
            ExprKind::Identifier(_) => (tgt, self.sym.vazio),
            ExprKind::Property { target, name, .. } => (*target, Some(name.sym)),
            _ => return false,
        };
        let Some(Resolved::Element(Element::Class(c))) = bt.get_resolved(cls) else { return false };
        let Some(the_name) = the_name else { return false };
        if self.constructor_or_primary(*c, the_name) != Some(None) {
            return false;
        }
        let Some(d) = self.program.class(*c).decl else { return false };
        matches!(&self.program.unit(d.unit).ast.decl(d.decl).kind, ast::DeclKind::ExtensionType(et) if et.const_)
    }

    pub(crate) fn is_constant(&self, cx: &Body, e: ExprId) -> bool {
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
                ast::StringPart::Interpolation(x) => self.is_constant(cx, *x),
                _ => true,
            }),
            ExprKind::Parenthesized(x) => self.is_constant(cx, *x),
            ExprKind::Identifier(_) => ref_const(bt.get_resolved(e)),
            ExprKind::Property { target, name, .. } => {
                if ref_const(bt.get_resolved(e)) {
                    return true;
                }
                // `s.length` de string constante.
                self.interner.resolve(name.sym) == "length" && self.is_constant(cx, *target)
            }
            ExprKind::TypeArguments { .. } => true,
            ExprKind::List { const_, elements, .. } | ExprKind::SetOrMap { const_, elements, .. } => {
                *const_ || elements.iter().all(|el| match el {
                    ast::CollectionElement::Expression(x) => self.is_constant(cx, *x),
                    ast::CollectionElement::MapEntry { key, value, .. } => self.is_constant(cx, *key) && self.is_constant(cx, *value),
                    _ => false,
                })
            }
            ExprKind::Record { positional, named, .. } => {
                positional.iter().all(|x| self.is_constant(cx, *x)) && named.iter().all(|(_, x)| self.is_constant(cx, *x))
            }
            ExprKind::InstanceCreation { keyword, arguments, .. } => {
                matches!(keyword, Some(ast::CreationKeyword::Const)) || arguments.args.iter().all(|x| self.is_constant(cx, x.value)) && self.const_constructor(bt.get_resolved(e))
            }
            ExprKind::Call { target, arguments } => {
                // Construtor `const` sem `new` em contexto constante, ou `identical`.
                let ok_args = arguments.args.iter().all(|x| self.is_constant(cx, x.value));
                if !ok_args {
                    return false;
                }
                if self.const_constructor(bt.get_resolved(e)) || self.const_primary(cx, *target) {
                    return true;
                }
                matches!(&a.expr(*target).kind, ExprKind::Identifier(n) if self.interner.resolve(n.sym) == "identical")
            }
            ExprKind::Unary { op, operand } => {
                matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot) && self.is_constant(cx, *operand)
            }
            ExprKind::Binary { left, right, .. } => self.is_constant(cx, *left) && self.is_constant(cx, *right),
            ExprKind::Conditional { condition, then, else_ } => {
                self.is_constant(cx, *condition) && self.is_constant(cx, *then) && self.is_constant(cx, *else_)
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.is_constant(cx, *value),
            _ => false,
        }
    }

    fn const_constructor(&self, r: Option<&Resolved>) -> bool {
        match r {
            Some(Resolved::Constructor(f)) => self.program.function(self.program.publico(*f)).const_,
            _ => false,
        }
    }
}
