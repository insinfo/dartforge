//! Corpos de funções declaradas, construtores (inicializadores), expressões
//! de função (com inferência do retorno, `inference.md` "Function literal
//! return type inference"), funções locais, variáveis de topo sem tipo e
//! metadados.

use super::corpo::{Corpo, CtxFuncao, Local};
use super::expr::{self, declarar_local, inferir, inferir_livre};
use super::instrucoes;
use super::BodyInferrer;
use crate::codes::*;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, FunctionBody};
use std::collections::HashMap;

impl<'a> BodyInferrer<'a> {
    /// Contexto das expressões de `return` para um retorno declarado `r`.
    pub(crate) fn contexto_de_retorno_declarado(&mut self, r: TypeId, m: AsyncModifier) -> TypeId {
        let u = self.core.unknown;
        match m {
            AsyncModifier::None => r,
            AsyncModifier::Async => {
                let fv = self.tipo_valor_futuro_esquema(r);
                self.futuro_ou(fv)
            }
            AsyncModifier::SyncStar => self.como_instancia_de(r, self.core.iterable_class).map(|a| a[0]).unwrap_or(u),
            AsyncModifier::AsyncStar => self.como_instancia_de(r, self.core.stream_class).map(|a| a[0]).unwrap_or(u),
        }
    }
}

/// Declara os parâmetros de uma lista escrita, com os tipos dados; infere
/// os valores padrão. `so_inicializadores` marca `this.x`/`super.x`.
fn declarar_parametros(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, params: &[ast::Parameter], tipos: &[TypeId], pular_inicializadores: bool) {
    for (i, p) in params.iter().enumerate() {
        let t = tipos.get(i).copied().unwrap_or(inf.core.dynamic_);
        if let Some(d) = p.default_value {
            let td = inferir(inf, cx, d, t);
            if !matches!(inf.table.get(t), Type::Void) {
                expr::uso_de_void(inf, cx, d, td);
            }
        }
        if pular_inicializadores && (p.this_ || p.super_) {
            continue;
        }
        if let Some(n) = &p.name {
            // Tipo escrito que não resolve: `dynamic` sem ser `dynamic`.
            if matches!(inf.table.get(t), Type::Dynamic) {
                if let Some(x) = p.ty {
                    if let ast::TypeKind::Named { name, .. } = &inf.program.unit(cx.unit).ast.ty(x).kind {
                        if !name.last().is_some_and(|k| inf.interner.resolve(k.sym) == "dynamic") {
                            inf.locais_invalidos.insert((cx.unit, n.span.start));
                        }
                    }
                }
            }
            declarar_local(
                inf,
                cx,
                Local { nome: n.sym, tipo: t, final_: p.final_, late: false, const_: false, offset: n.span.start, funcao_local: false },
                true,
            );
        }
    }
}

/// Infere o corpo de uma função/método/construtor declarado.
pub(crate) fn inferir_funcao_declarada(inf: &mut BodyInferrer<'_>, f: FunctionElementId) {
    let fe = inf.program.function(f);
    match fe.node {
        FunctionRef::Function { unit, function } => {
            let af = inf.program.unit(unit).ast.function(function);
            let dados = inf.outline.functions[f.0 as usize].clone();
            let mut cx = Corpo::para_funcao(inf, f, unit);
            cx.raiz = super::corpo::Raiz::Funcao(function);
            for &p in dados.type_params.iter() {
                let nome = inf.table.param(p).name;
                cx.declarar_tipo_param(nome, p);
            }
            cx.empurrar_escopo();
            if let Some(ps) = &af.parameters {
                let tipos: Vec<TypeId> = dados.parameters.iter().map(|p| p.ty).collect();
                declarar_parametros(inf, &mut cx, ps, &tipos, false);
            }
            let ret = dados.return_type;
            let ctx_ret = inf.contexto_de_retorno_declarado(ret, af.modifier);
            let executavel = inf.executavel_declarado(f);
            let retorno_legal = tipo_de_retorno_legal(inf, unit, af.return_type, af.modifier, ret);
            cx.funcoes.push(CtxFuncao { modificador: af.modifier, retorno: Some(ret), contexto_retorno: ctx_ret, retornados: Vec::new(), retorno_vazio: false, expressoes_retornadas: Vec::new(), executavel, retorno_legal });
            corpo_de_funcao(inf, &mut cx, &af.body, af.modifier, ret, None);
            // `checkForBodyMayCompleteNormally` no nome da função/método.
            if matches!(af.body, FunctionBody::Block(_))
                && cx.fluxo.alcancavel
                && let Some(n) = af.name
            {
                corpo_completa_normalmente(inf, Some(ret), Some(ctx_ret), af.modifier, n.span);
            }
            cx.funcoes.pop();
            // `FunctionDeclaration` (de topo, não método) com retorno escrito.
            let fe = inf.program.function(f);
            if fe.class.is_none() && fe.extension.is_none() && af.return_type.is_some() {
                conjunto_desnecessario(inf, unit, function, ret);
            }
        }
        FunctionRef::Constructor { unit, member } => {
            let ast::MemberKind::Constructor(ctor) = &inf.program.unit(unit).ast.member(member).kind else { return };
            let dados = inf.outline.functions[f.0 as usize].clone();
            let mut cx = Corpo::para_funcao(inf, f, unit);
            cx.raiz = super::corpo::Raiz::Construtor(member);
            if !fe.factory {
                cx.estatico = false;
                if let Some(c) = fe.class {
                    cx.tipo_this = Some(inf.tipo_this_classe(c));
                }
            }
            let tipos: Vec<TypeId> = dados.parameters.iter().map(|p| p.ty).collect();
            // Escopo dos inicializadores: todos os parâmetros.
            cx.empurrar_escopo();
            declarar_parametros(inf, &mut cx, &ctor.parameters, &tipos, false);
            // Inicializadores não veem `this` (salvo o acesso ao campo inicializado).
            let this_salvo = cx.tipo_this.take();
            let estatico_salvo = cx.estatico;
            cx.estatico = true;
            for init in ctor.initializers.iter() {
                inicializador(inf, &mut cx, fe.class, init, &ctor.parameters, fe.const_, f);
            }
            // `_checkForRecursiveConstructorRedirect` (`error_verifier.dart:5047-5066`):
            // no primeiro `this(...)` de um construtor gerador em ciclo.
            if !fe.factory
                && let Some(sp) = ctor.initializers.iter().find_map(|i| match i {
                    ast::Initializer::Redirect { span, .. } => Some(*span),
                    _ => None,
                })
                && redireciona_em_ciclo(inf, f)
            {
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::RECURSIVE_CONSTRUCTOR_REDIRECT, sp, &[]);
            }
            if let Some(red) = &ctor.redirect {
                alvo_de_factory_redirecionadora(inf, &mut cx, red, f, ctor);
            }
            if let Some(c) = fe.class {
                super_implicito(inf, c, ctor);
                parametros_super(inf, c, ctor, &tipos);
            }
            // `_checkForValidField` (`error_verifier.dart:5702-5760`): o tipo
            // escrito de `this.x` contra o do campo (subtipo, não atribuível).
            if let Some(c) = fe.class {
                for (i, p) in ctor.parameters.iter().enumerate() {
                    if !p.this_ || p.super_ || p.ty.is_none() {
                        continue;
                    }
                    let Some(n) = p.name else { continue };
                    let campo = inf.program.class(c).fields.iter().copied().find(|&v| inf.program.variable(v).name == n.sym);
                    let Some(v) = campo else { continue };
                    if inf.program.variable(v).static_ {
                        continue;
                    }
                    let (Some(&pt), ft) = (tipos.get(i), inf.tipo_variavel(v)) else { continue };
                    if !inf.sub(pt, ft) {
                        let inicio = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
                        let sp = dartforge_diagnostics::Span { start: inicio, end: n.span.end };
                        inf.aviso_com_args(
                            dartforge_diagnostics::codigos::compile_time_error::FIELD_INITIALIZING_FORMAL_NOT_ASSIGNABLE,
                            sp,
                            &[crate::exibicao::Arg::Tipo(pt), crate::exibicao::Arg::Tipo(ft)],
                        );
                    }
                }
            }
            cx.tipo_this = this_salvo;
            cx.estatico = estatico_salvo;
            cx.tirar_escopo();
            // Escopo do corpo: parâmetros sem `this.`/`super.`.
            cx.empurrar_escopo();
            let mut cx2 = cx;
            for (i, p) in ctor.parameters.iter().enumerate() {
                if p.this_ || p.super_ {
                    continue;
                }
                if let Some(n) = &p.name {
                    let t = tipos.get(i).copied().unwrap_or(inf.core.dynamic_);
                    let id = cx2.declarar(Local { nome: n.sym, tipo: t, final_: p.final_, late: false, const_: false, offset: n.span.start, funcao_local: false });
                    cx2.fluxo.inicializar(id);
                }
            }
            let v = inf.core.void_;
            let ret = if fe.factory { dados.return_type } else { v };
            let ctx_ret = ret;
            // Construtor gerador: `return e;` é `return_in_generative_constructor`.
            let executavel = if fe.factory { inf.executavel_declarado(f) } else { None };
            cx2.funcoes.push(CtxFuncao { modificador: AsyncModifier::None, retorno: Some(ret), contexto_retorno: ctx_ret, retornados: Vec::new(), retorno_vazio: false, expressoes_retornadas: Vec::new(), executavel, retorno_legal: true });
            // `flowEnd(ConstructorDeclaration)`: o analyzer não apara o
            // construtor na última instrução; o trecho vai até o fim dele.
            let fim = inf.program.unit(unit).ast.member(member).span.end;
            corpo_de_funcao(inf, &mut cx2, &ctor.body, AsyncModifier::None, ret, Some(fim));
            // Factory: `atConstructorDeclaration`, do tipo de retorno ao fim do
            // nome.
            if fe.factory && matches!(ctor.body, FunctionBody::Block(_)) && cx2.fluxo.alcancavel {
                let onde = dartforge_diagnostics::Span { start: ctor.class_name.span.start, end: ctor.name.map_or(ctor.class_name.span.end, |n| n.span.end) };
                corpo_completa_normalmente(inf, Some(ret), Some(ret), AsyncModifier::None, onde);
            }
        }
        FunctionRef::None => {}
    }
}

/// Um inicializador de construtor.
fn inicializador(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, classe: Option<ClassId>, init: &ast::Initializer, params: &[ast::Parameter], construtor_const: bool, atual: FunctionElementId) {
    let u = inf.core.unknown;
    match init {
        ast::Initializer::Field { name, value, .. } => {
            let campo = classe.and_then(|c| inf.program.class(c).fields.iter().copied().find(|&v| inf.program.variable(v).name == name.sym));
            let t = campo.map(|v| inf.tipo_variavel(v)).unwrap_or(u);
            let tv = inferir(inf, cx, *value, t);
            // `checkForFieldInitializerNotAssignable`
            // (`error_detection_helpers.dart:168-205`): na expressão; num
            // construtor `const`, a variante `CONST_`.
            if campo.is_some() {
                if inf.atribuivel(tv, t) {
                    if !matches!(inf.table.get(t), Type::Void) {
                        expr::uso_de_void(inf, cx, *value, tv);
                    }
                } else {
                    use dartforge_diagnostics::codigos::compile_time_error as c;
                    let codigo = if construtor_const { c::CONST_FIELD_INITIALIZER_NOT_ASSIGNABLE } else { c::FIELD_INITIALIZER_NOT_ASSIGNABLE };
                    let sp = inf.span_expr(cx.unit, *value);
                    let desde = inf.diagnostics.len();
                    inf.aviso_com_args(codigo, sp, &[crate::exibicao::Arg::Tipo(tv), crate::exibicao::Arg::Tipo(t)]);
                    inf.anexar_nao_promocao(desde, cx, Some(*value), sp);
                }
            }
        }
        ast::Initializer::Super { span, constructor, arguments } => {
            inicializador_super(inf, cx, classe, *span, *constructor, arguments, params);
        }
        ast::Initializer::Redirect { span, constructor, arguments } => {
            inicializador_redirecionador(inf, classe, *span, *constructor, atual);
            let alvo = classe.map(|c| {
                let ps = inf.outline.classes[c.0 as usize].type_params.clone();
                let args: Vec<TypeId> = ps.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
                (c, args.into_boxed_slice())
            });
            chamar_construtor_de(inf, cx, alvo, *constructor, arguments, &[], *span);
        }
        ast::Initializer::Assert { condition, message, .. } => {
            expr::condicao_de_assert(inf, cx, *condition);
            if let Some(m) = message {
                inferir_livre(inf, cx, *m);
            }
        }
    }
}

/// O que o nome do tipo de um alvo de redirecionamento (`= X`, `= p.X`,
/// `= X.n`, `= X<T>.n`) designa, como o `NamedTypeResolver` o resolve com
/// `redirectedConstructor_namedType`
/// (`an611:src/dart/resolver/named_type_resolver.dart:84-133`, `:230-312`).
enum TipoDoAlvo {
    /// Classe, enum, tipo de extensão ou alias que expande a um deles.
    Classe(ClassId),
    /// Um tipo sem construtores (parâmetro de tipo, `Never`, alias de tipo de
    /// função): o `staticElement` fica nulo e o tipo é o do `{1}` de
    /// `redirect_to_missing_constructor`.
    SemConstrutores(TypeId),
    /// `dynamic` (ou alias dele): nada a relatar.
    Dinamico,
    /// Alias que expande a um parâmetro de tipo (`_verifyTypeAliasForContext`).
    AliasDeParametro,
    /// `reportNullOrNonTypeElement`: `redirect_to_non_class` com o nome.
    NaoClasse(dartforge_intern::SymbolId),
    /// `p.X` com `p` que não é prefixo nem tipo.
    PrefixoSombreado,
    /// Indefinido mas calado (`shouldIgnoreUndefinedNamedType`) ou sintaxe
    /// que não chega a ser alvo.
    Nada,
}

/// O alvo escrito de uma factory redirecionadora, já resolvido.
struct AlvoEscrito {
    tipo: TipoDoAlvo,
    /// O nome do construtor (`None` = sem nome; `new` já vira sem nome).
    construtor: Option<dartforge_intern::SymbolId>,
    /// `NamedType.qualifiedName` com o `.nome` do construtor, como escrito
    /// (o `{0}` de `redirect_to_missing_constructor`).
    nome_citado: String,
    /// `_getErrorRange`: do prefixo ao nome do tipo, sem argumentos de tipo.
    faixa_do_tipo: dartforge_diagnostics::Span,
    /// O token do prefixo (para `prefix_shadowed_by_local_declaration`).
    prefixo: Option<ast::Name>,
}

/// O nome `nome` no escopo do construtor de `classe` (os parâmetros dele, os
/// parâmetros de tipo da classe, os membros dela e o escopo da unidade), como
/// o `nameScope` do `ResolutionVisitor.visitConstructorDeclaration`
/// (`an611:src/dart/resolver/resolution_visitor.dart:356-382`: os
/// parâmetros são definidos antes de `_resolveRedirectedConstructor`).
enum NoEscopo {
    Elemento(Element),
    TipoParam(TypeParamId),
    Prefixo,
    Embutido,
    /// Parâmetro, membro ou constante de enum: não é tipo.
    Outro,
    Nenhum,
}

fn nome_no_escopo_do_construtor(inf: &BodyInferrer<'_>, unit: UnitId, classe: Option<ClassId>, params: &[ast::Parameter], nome: dartforge_intern::SymbolId) -> NoEscopo {
    if params.iter().any(|p| p.name.is_some_and(|n| n.sym == nome)) {
        return NoEscopo::Outro;
    }
    if let Some(c) = classe {
        if let Some(&p) = inf.outline.classes[c.0 as usize].type_params.iter().find(|&&p| inf.table.param(p).name == nome) {
            return NoEscopo::TipoParam(p);
        }
        let chave_setter = inf.chave_setter(nome);
        if inf.membro_declarado_lexico(Some(c), None, nome, chave_setter).is_some() {
            return NoEscopo::Outro;
        }
        let ce = inf.program.class(c);
        if ce.enum_constants.iter().chain(ce.fields.iter()).any(|&v| inf.program.variable(v).name == nome) {
            return NoEscopo::Outro;
        }
    }
    if let Some(b) = inf.program.lookup_na_unidade(unit, nome)
        && let Some(el) = b.getter.or(b.setter)
    {
        if let Element::Prefix(..) = el {
            return NoEscopo::Prefixo;
        }
        return NoEscopo::Elemento(el);
    }
    if inf.program.prefixos_na_unidade(unit).contains_key(&nome) {
        return NoEscopo::Prefixo;
    }
    if matches!(inf.interner.resolve(nome), "dynamic" | "Never") {
        return NoEscopo::Embutido;
    }
    NoEscopo::Nenhum
}

/// O tipo que um elemento de tipo designa (`_resolveToElement`).
fn tipo_do_elemento(inf: &mut BodyInferrer<'_>, el: Element, nome: dartforge_intern::SymbolId) -> TipoDoAlvo {
    match el {
        Element::Class(c) => TipoDoAlvo::Classe(c),
        Element::Typedef(td) => {
            let alvo = inf.outline.typedefs[td.0 as usize].target_type;
            match inf.table.get(alvo).clone() {
                Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => TipoDoAlvo::Classe(class),
                Type::TypeParameter { .. } => TipoDoAlvo::AliasDeParametro,
                _ if inf.e_dynamic(alvo) => TipoDoAlvo::Dinamico,
                _ if inf.table.e_invalido(alvo) => TipoDoAlvo::Nada,
                _ => TipoDoAlvo::SemConstrutores(alvo),
            }
        }
        _ => TipoDoAlvo::NaoClasse(nome),
    }
}

/// Resolve o alvo escrito de `= …` no escopo do construtor.
fn resolver_alvo_escrito(inf: &mut BodyInferrer<'_>, unit: UnitId, classe: Option<ClassId>, params: &[ast::Parameter], red: &ast::RedirectTarget) -> Option<AlvoEscrito> {
    let (nomes, escrito): (Vec<ast::Name>, Option<ast::Name>) = {
        let nodo = inf.program.unit(unit).ast.ty(red.ty);
        let ast::TypeKind::Named { name, .. } = &nodo.kind else { return None };
        (name.to_vec(), red.constructor)
    };
    let sem_new = |inf: &BodyInferrer<'_>, s: Option<dartforge_intern::SymbolId>| match s {
        Some(x) if Some(x) == inf.sym.new_ => None,
        x => x,
    };
    // (prefixo, nome do tipo, nome do construtor)
    let (prefixo, tipo, ctor): (Option<ast::Name>, ast::Name, Option<ast::Name>) = match (nomes.as_slice(), escrito) {
        ([a], c) => (None, *a, c),
        ([a, b], None) => {
            // `A.b`: o parser não distingue `prefixo.Tipo` de
            // `Tipo.construtor`; o resolvedor reescreve o segundo quando o
            // primeiro designa uma classe ou alias.
            match nome_no_escopo_do_construtor(inf, unit, classe, params, a.sym) {
                NoEscopo::Elemento(Element::Class(_) | Element::Typedef(_)) => (None, *a, Some(*b)),
                _ => (Some(*a), *b, None),
            }
        }
        ([a, b], Some(c)) => (Some(*a), *b, Some(c)),
        ([a, b, c], None) => (Some(*a), *b, Some(*c)),
        _ => return None,
    };
    let construtor = sem_new(inf, ctor.map(|n| n.sym));
    let mut nome_citado: String = match prefixo {
        Some(p) => format!("{}.{}", inf.interner.resolve(p.sym), inf.interner.resolve(tipo.sym)),
        None => inf.interner.resolve(tipo.sym).to_string(),
    };
    if let Some(c) = ctor {
        nome_citado.push('.');
        nome_citado.push_str(inf.interner.resolve(c.sym));
    }
    let faixa_do_tipo = dartforge_diagnostics::Span { start: prefixo.map_or(tipo.span.start, |p| p.span.start), end: tipo.span.end };
    let tipo_do_alvo = match prefixo {
        None => match nome_no_escopo_do_construtor(inf, unit, classe, params, tipo.sym) {
            NoEscopo::Elemento(el) => tipo_do_elemento(inf, el, tipo.sym),
            NoEscopo::TipoParam(p) => TipoDoAlvo::SemConstrutores(inf.table.intern(Type::TypeParameter { param: p, nullable: false })),
            NoEscopo::Embutido if inf.interner.resolve(tipo.sym) == "dynamic" => TipoDoAlvo::Dinamico,
            NoEscopo::Embutido => TipoDoAlvo::SemConstrutores(inf.core.never),
            NoEscopo::Prefixo | NoEscopo::Outro => TipoDoAlvo::NaoClasse(tipo.sym),
            NoEscopo::Nenhum => {
                if crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, unit, None, tipo.sym) {
                    TipoDoAlvo::Nada
                } else {
                    TipoDoAlvo::NaoClasse(tipo.sym)
                }
            }
        },
        Some(p) => match nome_no_escopo_do_construtor(inf, unit, classe, params, p.sym) {
            NoEscopo::Prefixo => match inf.program.lookup_prefixed_na_unidade(unit, p.sym, tipo.sym).and_then(|b| b.getter) {
                Some(el) => tipo_do_elemento(inf, el, tipo.sym),
                None if crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, unit, Some(p.sym), tipo.sym) => TipoDoAlvo::Nada,
                None => TipoDoAlvo::NaoClasse(tipo.sym),
            },
            NoEscopo::Nenhum => {
                if crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, unit, Some(p.sym), tipo.sym) {
                    TipoDoAlvo::Nada
                } else {
                    TipoDoAlvo::NaoClasse(tipo.sym)
                }
            }
            // `p.X.n` com `p` classe: sintaxe que não chega a alvo.
            NoEscopo::Elemento(Element::Class(_) | Element::Typedef(_)) => TipoDoAlvo::Nada,
            _ => TipoDoAlvo::PrefixoSombreado,
        },
    };
    Some(AlvoEscrito { tipo: tipo_do_alvo, construtor, nome_citado, faixa_do_tipo, prefixo })
}

/// O construtor que um alvo designa (`lookUpConstructor`: com o teste de
/// acessibilidade do privado).
#[derive(Clone, Copy)]
enum Construtor {
    Declarado(FunctionElementId),
    /// O sintético sem nome de uma classe abstrata ou enum sem construtor
    /// declarado (o modelo só cria o das classes concretas), ou o primário de
    /// um tipo de extensão.
    Implicito,
}

fn achar_construtor(inf: &BodyInferrer<'_>, alvo: ClassId, nome: Option<dartforge_intern::SymbolId>, lib: dartforge_elements::model::LibraryId) -> Option<Construtor> {
    let chave = nome.or(inf.sym.vazio)?;
    if inf.interner.resolve(chave).starts_with('_') && inf.program.class(alvo).library != lib {
        return None;
    }
    match inf.construtor_ou_primario(alvo, chave) {
        Some(Some(f)) => Some(Construtor::Declarado(f)),
        Some(None) => Some(Construtor::Implicito),
        None => {
            if nome.is_some() {
                return None;
            }
            // O sintético que o modelo não cria, também repassado por uma
            // aplicação de mixin.
            let mut c = alvo;
            loop {
                let ce = inf.program.class(c);
                match ce.kind {
                    ClassKind::MixinApplication => c = ce.supertype_class?,
                    ClassKind::Class | ClassKind::Enum if ce.constructors.is_empty() => return Some(Construtor::Implicito),
                    _ => return None,
                }
            }
        }
    }
}

/// O sintético sem nome que o modelo não cria (classe abstrata sem
/// construtor declarado, também através de aplicações de mixin).
pub(crate) fn sem_nome_implicito(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let mut atual = c;
    for _ in 0..16 {
        let ce = inf.program.class(atual);
        match ce.kind {
            ClassKind::MixinApplication => match ce.supertype_class {
                Some(s) => atual = s,
                None => return false,
            },
            ClassKind::Class | ClassKind::Enum => return ce.constructors.is_empty(),
            _ => return false,
        }
    }
    false
}

/// `isFactory`/`isConst` de um construtor achado.
fn fabrica_e_const(inf: &BodyInferrer<'_>, alvo: ClassId, c: Construtor) -> (bool, bool) {
    match c {
        Construtor::Declarado(f) => {
            let fe = inf.program.function(f);
            (fe.factory, fe.const_)
        }
        // O sintético de enum é `const`; o de classe não. O primário de um
        // tipo de extensão é `const` com `extension type const`.
        Construtor::Implicito => {
            let ce = inf.program.class(alvo);
            match ce.kind {
                ClassKind::Enum => (false, true),
                ClassKind::ExtensionType => {
                    let const_ = ce.decl.is_some_and(|d| matches!(&inf.program.unit(d.unit).ast.decl(d.decl).kind, ast::DeclKind::ExtensionType(et) if et.const_));
                    (false, const_)
                }
                _ => (false, false),
            }
        }
    }
}

/// O tipo de função de um construtor, com o retorno trocado pelo tipo
/// `this` da classe (o do construtor repassado de uma aplicação de mixin é
/// o da aplicação).
fn tipo_do_construtor(inf: &mut BodyInferrer<'_>, alvo: ClassId, c: Construtor) -> TypeId {
    let this = inf.tipo_this_classe(alvo);
    let sig = match c {
        Construtor::Declarado(f) if !matches!(inf.program.function(f).node, FunctionRef::None) => inf.outline.functions[f.0 as usize].signature,
        Construtor::Implicito if inf.program.class(alvo).kind == ClassKind::ExtensionType => inf.assinatura_primario(alvo),
        _ => inf.table.intern(Type::Function {
            type_params: Box::new([]),
            ret: this,
            positional: Box::new([]),
            optional: Box::new([]),
            named: Box::new([]),
            nullable: false,
        }),
    };
    // Os parâmetros de tipo da classe ficam livres (o `ConstructorElement.type`
    // não tem parâmetros de tipo próprios).
    match inf.table.get(sig).clone() {
        Type::Function { positional, optional, named, nullable, .. } => {
            inf.table.intern(Type::Function { type_params: Box::new([]), ret: this, positional, optional, named, nullable })
        }
        _ => sig,
    }
}

/// `redirectedConstructor?.declaration` de um construtor declarado: o alvo
/// de `= …` ou do `this(...)` (o primeiro), quando é um construtor
/// declarado.
pub(crate) fn construtor_redirecionado(inf: &mut BodyInferrer<'_>, f: FunctionElementId) -> Option<FunctionElementId> {
    let fe = inf.program.function(f);
    let FunctionRef::Constructor { unit, member } = fe.node else { return None };
    let classe = fe.class;
    let lib = fe.library;
    let ast::MemberKind::Constructor(ctor) = &inf.program.unit(unit).ast.member(member).kind else { return None };
    if let Some(red) = &ctor.redirect {
        let alvo = resolver_alvo_escrito(inf, unit, classe, &ctor.parameters, red)?;
        let TipoDoAlvo::Classe(c) = alvo.tipo else { return None };
        return match achar_construtor(inf, c, alvo.construtor, lib)? {
            Construtor::Declarado(g) => Some(g),
            Construtor::Implicito => None,
        };
    }
    if ctor.factory {
        return None;
    }
    let nome = ctor.initializers.iter().find_map(|i| match i {
        ast::Initializer::Redirect { constructor, .. } => Some(*constructor),
        _ => None,
    })?;
    let c = classe?;
    let chave = match nome.map(|n| n.sym) {
        Some(s) if Some(s) == inf.sym.new_ => inf.sym.vazio,
        Some(s) => Some(s),
        None => inf.sym.vazio,
    }?;
    inf.construtor_de(c, chave)
}

/// `_hasRedirectingFactoryConstructorCycle`
/// (`an611:src/generated/error_verifier.dart:6327-6338`): `f` volta a si
/// mesmo pela cadeia de redirecionamentos (um ciclo adiante dele não conta).
pub(crate) fn redireciona_em_ciclo(inf: &mut BodyInferrer<'_>, f: FunctionElementId) -> bool {
    let mut vistos = std::collections::HashSet::new();
    let mut atual = Some(f);
    while let Some(g) = atual {
        if !vistos.insert(g) {
            return g == f;
        }
        atual = construtor_redirecionado(inf, g);
    }
    false
}

/// As regras de `visitConstructorDeclaration` para a factory
/// redirecionadora `= alvo` (`an611:src/generated/error_verifier.dart:589-605`),
/// na ordem do analyzer:
///
/// 1. `_checkForRedirectingConstructorErrorCodes` (`:5094-5137`):
///    `redirect_to_non_const_constructor`,
///    `redirect_to_abstract_class_constructor` e
///    `invalid_reference_to_generative_enum_constructor`;
/// 2. `_checkForRecursiveFactoryRedirect` (`:5073-5089`) ou, sem ciclo,
///    `_checkForAllRedirectConstructorErrorCodes` (`:2025-2075`):
///    `redirect_to_missing_constructor`, `redirect_to_invalid_return_type` e
///    `redirect_to_invalid_function_type`.
///
/// Antes, os da resolução do tipo do alvo (`redirect_to_non_class`,
/// `redirect_to_type_alias_expands_to_type_parameter`,
/// `prefix_shadowed_by_local_declaration`).
fn alvo_de_factory_redirecionadora(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, red: &ast::RedirectTarget, f_atual: FunctionElementId, ctor: &ast::Constructor) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let fe = inf.program.function(f_atual);
    let (dona, lib, const_atual) = (fe.class, fe.library, fe.const_);
    let Some(alvo) = resolver_alvo_escrito(inf, cx.unit, dona, &ctor.parameters, red) else { return };
    let classe = match alvo.tipo {
        TipoDoAlvo::Classe(c) => c,
        TipoDoAlvo::NaoClasse(nome) => {
            let texto = inf.interner.resolve(nome).to_string();
            inf.aviso_com_codigo(ce::REDIRECT_TO_NON_CLASS, alvo.faixa_do_tipo, &[&texto]);
            return;
        }
        TipoDoAlvo::AliasDeParametro => {
            inf.aviso_com_codigo(ce::REDIRECT_TO_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER, alvo.faixa_do_tipo, &[]);
            return;
        }
        TipoDoAlvo::PrefixoSombreado => {
            if let Some(p) = alvo.prefixo {
                let texto = inf.interner.resolve(p.sym).to_string();
                inf.aviso_com_codigo(ce::PREFIX_SHADOWED_BY_LOCAL_DECLARATION, p.span, &[&texto]);
            }
            return;
        }
        TipoDoAlvo::SemConstrutores(t) => {
            if !redireciona_em_ciclo(inf, f_atual) {
                let exibido = inf.table.format(t, inf.interner, inf.program);
                inf.aviso_com_codigo(ce::REDIRECT_TO_MISSING_CONSTRUCTOR, red.span, &[&alvo.nome_citado, &exibido]);
            }
            return;
        }
        TipoDoAlvo::Dinamico | TipoDoAlvo::Nada => return,
    };
    // Os argumentos de tipo escritos do alvo (`= B<T>.nome`), no escopo do
    // construtor: o `NamedType.type` que o `TypeArgumentsVerifier` lê.
    let escritos: Vec<ast::TypeId> = match &inf.program.unit(cx.unit).ast.ty(red.ty).kind {
        ast::TypeKind::Named { args, .. } => args.to_vec(),
        _ => Vec::new(),
    };
    for &x in escritos.iter() {
        inf.tipo_de_argumento_de_tipo(cx, x);
    }
    let n_params = inf.outline.classes[classe.0 as usize].type_params.len();
    if !escritos.is_empty() && escritos.len() != n_params {
        let texto = inf.interner.resolve(inf.program.class(classe).name).to_string();
        let sp = inf.program.unit(cx.unit).ast.ty(red.ty).span;
        inf.relatar_argumentos_de_tipo(&texto, n_params, escritos.len(), sp);
    }
    let achado = achar_construtor(inf, classe, alvo.construtor, lib);
    // 1. `_checkForRedirectingConstructorErrorCodes`.
    if let Some(c) = achado {
        let (fabrica, const_alvo) = fabrica_e_const(inf, classe, c);
        if const_atual && !const_alvo {
            inf.aviso_com_codigo(ce::REDIRECT_TO_NON_CONST_CONSTRUCTOR, red.span, &[]);
        }
        let ce_alvo = inf.program.class(classe);
        if matches!(ce_alvo.kind, ClassKind::Class | ClassKind::MixinApplication) && ce_alvo.modifiers.abstract_ && !fabrica {
            let mut nome = dona.map(|d| inf.interner.resolve(inf.program.class(d).name).to_string()).unwrap_or_default();
            if let Some(n) = ctor.name {
                nome.push('.');
                nome.push_str(inf.interner.resolve(n.sym));
            }
            let classe_alvo = inf.interner.resolve(ce_alvo.name).to_string();
            inf.aviso_com_codigo(ce::REDIRECT_TO_ABSTRACT_CLASS_CONSTRUCTOR, red.span, &[&nome, &classe_alvo]);
        }
        // `_checkForInvalidGenerativeConstructorReference`.
        if inf.program.class(classe).kind == ClassKind::Enum && !fabrica {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), red.span);
        }
    }
    // 2. Ciclo, ou as regras do alvo.
    if achado.is_some_and(|c| matches!(c, Construtor::Declarado(_))) && redireciona_em_ciclo(inf, f_atual) {
        inf.aviso_com_codigo(ce::RECURSIVE_FACTORY_REDIRECT, red.span, &[]);
        return;
    }
    let Some(c) = achado else {
        // `{1}`: o tipo do alvo como `DartType` (o da classe instanciada com
        // os próprios parâmetros, que é o que a inferência dá sem
        // argumentos escritos).
        let tipo = inf.tipo_this_classe(classe);
        let exibido = inf.table.format(tipo, inf.interner, inf.program);
        inf.aviso_com_codigo(ce::REDIRECT_TO_MISSING_CONSTRUCTOR, red.span, &[&alvo.nome_citado, &exibido]);
        return;
    };
    // Os testes de tipo só sem inferência de argumentos de tipo
    // (`_inferRedirectedConstructor` não é portado): alvo sem parâmetros de
    // tipo.
    let Some(dona) = dona else { return };
    if !inf.program.class(classe).type_params.is_empty() {
        return;
    }
    let de = tipo_do_construtor(inf, classe, c);
    let para = tipo_do_construtor(inf, dona, Construtor::Declarado(f_atual));
    let (Type::Function { ret: ret_de, .. }, Type::Function { ret: ret_para, .. }) = (inf.table.get(de).clone(), inf.table.get(para).clone()) else { return };
    if !inf.atribuivel(ret_de, ret_para) {
        let (a, b) = (inf.table.format(ret_de, inf.interner, inf.program), inf.table.format(ret_para, inf.interner, inf.program));
        inf.aviso_com_codigo(ce::REDIRECT_TO_INVALID_RETURN_TYPE, red.span, &[&a, &b]);
    } else if !inf.sub(de, para) {
        let (a, b) = (inf.table.format(de, inf.interner, inf.program), inf.table.format(para, inf.interner, inf.program));
        inf.aviso_com_codigo(ce::REDIRECT_TO_INVALID_FUNCTION_TYPE, red.span, &[&a, &b]);
    }
}

/// `enclosingClass.supertype` do analyzer para uma classe
/// (`an611:src/summary2/types_builder.dart:117-127` e
/// `library_builder.dart:732-743`): o `extends` escrito quando é uma classe
/// (não enum, tipo de extensão, mixin, `Function`, `Null` nem anulável);
/// senão `Object`. Nada para o próprio `Object`, para enums (o supertipo é
/// `Enum`, cujo sintético sem nome nunca falta) e para o que não é classe.
/// Com mixins o supertipo continua o do `extends` (o analyzer não
/// desaçucara a aplicação).
fn supertipo_do_analyzer(inf: &mut BodyInferrer<'_>, c: ClassId) -> Option<(ClassId, TypeId)> {
    if inf.program.class(c).kind != ClassKind::Class {
        return None;
    }
    let objeto = inf.core.object_class?;
    if c == objeto {
        return None;
    }
    if let Some(t) = inf.outline.classes[c.0 as usize].supertype
        && let Type::Interface { class: s, nullable: false, .. } = inf.table.get(t).clone()
        && matches!(inf.program.class(s).kind, ClassKind::Class | ClassKind::MixinApplication)
        && Some(s) != inf.core.function_class
        && Some(s) != inf.core.null_class
    {
        return Some((s, t));
    }
    Some((objeto, inf.core.object))
}

/// `shouldIgnoreUndefinedNamedType` do `extends` escrito de `c`.
fn extends_ignorado(inf: &BodyInferrer<'_>, c: ClassId) -> bool {
    let Some((u, t)) = inf.program.class(c).supertype else { return false };
    let ast::TypeKind::Named { name, .. } = &inf.program.unit(u).ast.ty(t).kind else { return false };
    let (prefixo, nome) = match name.as_ref() {
        [a] => (None, a.sym),
        [p, a] => (Some(p.sym), a.sym),
        _ => return false,
    };
    crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, u, prefixo, nome)
}

/// `constructors.every((c) => c.isFactory)` da classe `s` (o sintético é
/// gerador; os de uma aplicação de mixin são os geradores repassados da
/// base, e a lista vazia passa).
fn so_fabricas(inf: &BodyInferrer<'_>, s: ClassId) -> bool {
    let ce = inf.program.class(s);
    if ce.kind == ClassKind::MixinApplication {
        let mut base = ce.supertype_class;
        while let Some(b) = base {
            let be = inf.program.class(b);
            if be.kind != ClassKind::MixinApplication {
                return !(be.constructors.is_empty() || be.constructors.values().any(|&f| !inf.program.function(f).factory));
            }
            base = be.supertype_class;
        }
        return true;
    }
    !ce.constructors.is_empty() && ce.constructors.values().all(|&f| inf.program.function(f).factory)
}

/// Os parâmetros de um construtor achado: o nó do AST (com a unidade) e os
/// tipos do outline, na mesma ordem.
fn parametros_do_construtor<'a>(inf: &BodyInferrer<'a>, k: Construtor) -> Option<(UnitId, &'a ast::Constructor, Vec<crate::resolve::ParameterTypeData>)> {
    let Construtor::Declarado(f) = k else { return None };
    let programa: &'a dartforge_elements::model::Program = inf.program;
    let FunctionRef::Constructor { unit, member } = programa.function(f).node else { return None };
    let ast::MemberKind::Constructor(ctor) = &programa.unit(unit).ast.member(member).kind else { return None };
    Some((unit, ctor, inf.outline.functions[f.0 as usize].parameters.to_vec()))
}

/// Os posicionais obrigatórios e os nomes dos nomeados `required`.
fn obrigatorios_do_construtor(inf: &BodyInferrer<'_>, k: Construtor) -> (usize, Vec<dartforge_intern::SymbolId>) {
    let Some((_, ctor, _)) = parametros_do_construtor(inf, k) else { return (0, Vec::new()) };
    let posicionais = ctor.parameters.iter().filter(|p| p.kind == ast::ParameterKind::Required).count();
    let nomeados = ctor.parameters.iter().filter(|p| p.kind == ast::ParameterKind::Named && p.required).filter_map(|p| p.nome_externo().map(|n| n.sym)).collect();
    (posicionais, nomeados)
}

/// `ConstructorElement.getDisplayString()`
/// (`an611:src/dart/element/display_string_builder.dart:78-93`, `:405-461`,
/// `:531-553`): `Tipo Classe.nome(int a, [int b = 0], {required int c})`,
/// com o tipo e os parâmetros do construtor visto pelo tipo `tipo` (o
/// `ConstructorMember` substituído).
fn exibir_construtor(inf: &mut BodyInferrer<'_>, alvo: ClassId, k: Construtor, tipo: TypeId) -> String {
    let mut s = inf.table.format(tipo, inf.interner, inf.program);
    s.push(' ');
    s.push_str(inf.interner.resolve(inf.program.class(alvo).name));
    if let Construtor::Declarado(f) = k {
        let nome = inf.program.function(f).name;
        if Some(nome) != inf.sym.vazio {
            s.push('.');
            s.push_str(inf.interner.resolve(nome));
        }
    }
    s.push('(');
    let mapa = match inf.table.get(tipo).clone() {
        Type::Interface { args, .. } => {
            let ps = inf.outline.classes[alvo.0 as usize].type_params.clone();
            (ps.len() == args.len()).then(|| inf.mapa(&ps, &args))
        }
        _ => None,
    };
    if let Some((unit, ctor, tipos)) = parametros_do_construtor(inf, k) {
        let programa: &dartforge_elements::model::Program = inf.program;
        let fonte = &programa.unit(unit).source;
        let mut pedacos: Vec<(ast::ParameterKind, String)> = Vec::new();
        for (i, p) in ctor.parameters.iter().enumerate() {
            let mut t = tipos.get(i).map_or(inf.core.dynamic_, |d| d.ty);
            if let Some(m) = &mapa {
                t = inf.subst(t, m);
            }
            let mut texto = String::new();
            if p.kind == ast::ParameterKind::Named && p.required {
                texto.push_str("required ");
            }
            texto.push_str(&inf.table.format(t, inf.interner, inf.program));
            texto.push(' ');
            if let Some(n) = p.nome_externo() {
                texto.push_str(inf.interner.resolve(n.sym));
            }
            if let Some(d) = p.default_value {
                let sp = programa.unit(unit).ast.expr(d).span;
                texto.push_str(" = ");
                texto.push_str(&fonte[sp.start..sp.end]);
            }
            pedacos.push((p.kind, texto));
        }
        let mut ultimo: Option<ast::ParameterKind> = None;
        let mut fecho = "";
        for (i, (tipo_p, texto)) in pedacos.iter().enumerate() {
            if i != 0 {
                s.push_str(", ");
            }
            if ultimo != Some(*tipo_p) {
                s.push_str(fecho);
                let (abre, fecha) = match tipo_p {
                    ast::ParameterKind::Required => ("", ""),
                    ast::ParameterKind::Optional => ("[", "]"),
                    ast::ParameterKind::Named => ("{", "}"),
                };
                s.push_str(abre);
                fecho = fecha;
                ultimo = Some(*tipo_p);
            }
            s.push_str(texto);
        }
        s.push_str(fecho);
    }
    s.push(')');
    s
}

/// `super(...)`/`super.n(...)`: `ElementResolver.visitSuperConstructorInvocation`
/// (`an611:src/generated/element_resolver.dart:338-404`):
/// `undefined_constructor_in_initializer(_default)` sem o construtor (ou
/// privado de outra biblioteca) e `non_generative_constructor` com uma
/// factory numa superclasse que tem algum gerador; depois os argumentos
/// contra o construtor (salvo com o `extends` calado por
/// `shouldIgnoreUndefinedNamedType`).
fn inicializador_super(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, classe: Option<ClassId>, span: dartforge_diagnostics::Span, constructor: Option<ast::Name>, arguments: &ast::Arguments, params: &[ast::Parameter]) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let sup = classe.and_then(|c| supertipo_do_analyzer(inf, c).map(|s| (c, s)));
    let Some((c, (s, st))) = sup else {
        for a in arguments.args.iter() {
            inferir_livre(inf, cx, a.value);
        }
        return;
    };
    let chave = match constructor.map(|n| n.sym) {
        Some(x) if Some(x) == inf.sym.new_ => None,
        x => x,
    };
    let lib = inf.program.class(c).library;
    let Some(k) = achar_construtor(inf, s, chave, lib) else {
        let exibido = inf.table.format(st, inf.interner, inf.program);
        match constructor {
            Some(n) => {
                let nome = inf.interner.resolve(n.sym).to_string();
                inf.aviso_com_codigo(ce::UNDEFINED_CONSTRUCTOR_IN_INITIALIZER, span, &[&exibido, &nome]);
            }
            None => inf.aviso_com_codigo(ce::UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT, span, &[&exibido]),
        }
        for a in arguments.args.iter() {
            inferir_livre(inf, cx, a.value);
        }
        return;
    };
    if fabrica_e_const(inf, s, k).0 && !so_fabricas(inf, s) {
        let exibido = exibir_construtor(inf, s, k, st);
        inf.aviso_com_codigo(ce::NON_GENERATIVE_CONSTRUCTOR, span, &[&exibido]);
    }
    if extends_ignorado(inf, c) {
        for a in arguments.args.iter() {
            inferir_livre(inf, cx, a.value);
        }
        return;
    }
    let alvo = match inf.table.get(st).clone() {
        Type::Interface { class, args, .. } => Some((class, args)),
        _ => None,
    };
    chamar_construtor_de(inf, cx, alvo, constructor, arguments, params, span);
}

/// `this(...)`/`this.n(...)` (`_checkForConflictingInitializerErrorCodes`,
/// `an611:src/generated/error_verifier.dart:2705-2733`): fora de `factory`,
/// `redirect_generative_to_missing_constructor` sem o alvo (procurado só na
/// própria classe) e `redirect_generative_to_non_generative_constructor`
/// com uma factory; e `redirect_to_non_const_constructor` no nome (ou no
/// `this`) quando o atual é `const` e o alvo não.
fn inicializador_redirecionador(inf: &mut BodyInferrer<'_>, classe: Option<ClassId>, span: dartforge_diagnostics::Span, constructor: Option<ast::Name>, atual: FunctionElementId) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let Some(c) = classe else { return };
    let chave = match constructor.map(|n| n.sym) {
        Some(x) if Some(x) == inf.sym.new_ => inf.sym.vazio,
        Some(x) => Some(x),
        None => inf.sym.vazio,
    };
    let achado = chave.and_then(|k| inf.construtor_ou_primario(c, k)).map(|f| match f {
        Some(f) => Construtor::Declarado(f),
        None => Construtor::Implicito,
    });
    let fe = inf.program.function(atual);
    let (fabrica_atual, const_atual) = (fe.factory, fe.const_);
    if !fabrica_atual {
        match achado {
            None => {
                let classe_nome = inf.interner.resolve(inf.program.class(c).name).to_string();
                let mut nome = classe_nome.clone();
                if let Some(n) = constructor {
                    nome.push('.');
                    nome.push_str(inf.interner.resolve(n.sym));
                }
                inf.aviso_com_codigo(ce::REDIRECT_GENERATIVE_TO_MISSING_CONSTRUCTOR, span, &[&nome, &classe_nome]);
            }
            Some(k) if fabrica_e_const(inf, c, k).0 => {
                inf.aviso_com_codigo(ce::REDIRECT_GENERATIVE_TO_NON_GENERATIVE_CONSTRUCTOR, span, &[]);
            }
            Some(_) => {}
        }
    }
    if let Some(k) = achado
        && const_atual
        && !fabrica_e_const(inf, c, k).1
    {
        let sp = constructor.map_or(dartforge_diagnostics::Span { start: span.start, end: span.start + 4 }, |n| n.span);
        inf.aviso_com_codigo(ce::REDIRECT_TO_NON_CONST_CONSTRUCTOR, sp, &[]);
    }
}

/// `SuperConstructorResolver._constructor`
/// (`an611:src/summary2/super_constructor_resolver.dart:28-62`): o
/// `superConstructor` é o construtor do supertipo com o nome do `super(...)`
/// (ou o sem nome), comparado ao pé da letra (`super.new()` não acha), sem
/// teste de acessibilidade e factory incluída; só para classes (enum fica
/// sem).
fn super_construtor(inf: &mut BodyInferrer<'_>, c: ClassId, ctor: &ast::Constructor) -> Option<(ClassId, TypeId, Construtor)> {
    let nome = ctor
        .initializers
        .iter()
        .find_map(|i| match i {
            ast::Initializer::Super { constructor, .. } => Some(constructor.map(|n| n.sym)),
            _ => None,
        })
        .flatten();
    let (s, st) = supertipo_do_analyzer(inf, c)?;
    let chave = nome.or(inf.sym.vazio)?;
    if Some(chave) == inf.sym.new_ {
        return None;
    }
    let mut atual = s;
    let mut repassado = false;
    for _ in 0..16 {
        let ce = inf.program.class(atual);
        if ce.kind == ClassKind::MixinApplication {
            atual = ce.supertype_class?;
            repassado = true;
            continue;
        }
        if let Some(&f) = ce.constructors.get(&chave) {
            if repassado && inf.program.function(f).factory {
                return None;
            }
            if matches!(inf.program.function(f).node, FunctionRef::None) {
                return Some((s, st, Construtor::Implicito));
            }
            return Some((s, st, Construtor::Declarado(f)));
        }
        return (Some(chave) == inf.sym.vazio && ce.constructors.is_empty() && ce.kind == ClassKind::Class).then_some((s, st, Construtor::Implicito));
    }
    None
}

/// `ErrorVerifier.visitSuperFormalParameter`
/// (`an611:src/generated/error_verifier.dart:1457-1503`) nos `super.x` de um
/// construtor gerador não redirecionador e não `external` (os outros lugares
/// são `invalid_super_formal_parameter_location`, em `analise`; num tipo de
/// extensão, `extension_type_constructor_with_super_formal_parameter`): sem
/// o parâmetro associado (o posicional pelo índice entre os `super.`, o
/// nomeado pelo nome) `super_formal_parameter_without_associated_*`; com
/// ele e o tipo escrito fora do dele,
/// `super_formal_parameter_type_is_not_subtype_of_associated`. No nome.
fn parametros_super(inf: &mut BodyInferrer<'_>, c: ClassId, ctor: &ast::Constructor, tipos: &[TypeId]) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    if ctor.factory || ctor.external || ctor.initializers.iter().any(|i| matches!(i, ast::Initializer::Redirect { .. })) {
        return;
    }
    if inf.program.class(c).kind == ClassKind::ExtensionType || !ctor.parameters.iter().any(|p| p.super_) {
        return;
    }
    let associado = super_construtor(inf, c, ctor);
    // Os parâmetros do construtor associado, vistos pelo supertipo.
    let (posicionais, nomeados): (Vec<TypeId>, Vec<(dartforge_intern::SymbolId, TypeId)>) = match associado {
        Some((s, st, k)) => {
            let mapa = match inf.table.get(st).clone() {
                Type::Interface { args, .. } => {
                    let ps = inf.outline.classes[s.0 as usize].type_params.clone();
                    (ps.len() == args.len()).then(|| inf.mapa(&ps, &args))
                }
                _ => None,
            };
            let mut pos = Vec::new();
            let mut nom = Vec::new();
            if let Some((_, k_ast, dados)) = parametros_do_construtor(inf, k) {
                for (i, p) in k_ast.parameters.iter().enumerate() {
                    let mut t = dados.get(i).map_or(inf.core.dynamic_, |d| d.ty);
                    if let Some(m) = &mapa {
                        t = inf.subst(t, m);
                    }
                    match p.kind {
                        ast::ParameterKind::Named => {
                            if let Some(n) = p.nome_externo() {
                                nom.push((n.sym, t));
                            }
                        }
                        _ => pos.push(t),
                    }
                }
            }
            (pos, nom)
        }
        None => (Vec::new(), Vec::new()),
    };
    let mut indice = 0usize;
    for (i, p) in ctor.parameters.iter().enumerate() {
        if !p.super_ {
            continue;
        }
        let Some(n) = p.name else {
            indice += 1;
            continue;
        };
        let alvo = if p.kind == ast::ParameterKind::Named {
            let externo = p.nome_externo().map_or(n.sym, |x| x.sym);
            nomeados.iter().find(|(m, _)| *m == externo).map(|(_, t)| *t)
        } else {
            posicionais.get(indice).copied()
        };
        indice += 1;
        let Some(tipo_associado) = alvo else {
            let codigo = if p.kind == ast::ParameterKind::Named { ce::SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_NAMED } else { ce::SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_POSITIONAL };
            inf.aviso_com_codigo(codigo, n.span, &[]);
            continue;
        };
        // Sem tipo escrito o tipo é o herdado: nunca dispara.
        if p.ty.is_none() && p.function_parameters.is_none() {
            continue;
        }
        let Some(&proprio) = tipos.get(i) else { continue };
        if !inf.sub(proprio, tipo_associado) {
            inf.aviso_com_args(
                ce::SUPER_FORMAL_PARAMETER_TYPE_IS_NOT_SUBTYPE_OF_ASSOCIATED,
                n.span,
                &[crate::exibicao::Arg::Tipo(proprio), crate::exibicao::Arg::Tipo(tipo_associado)],
            );
        }
    }
}

/// `_checkForUndefinedConstructorInInitializerImplicit`
/// (`an611:src/generated/error_verifier.dart:5444-5551`): construtor
/// gerador, não `external`, sem `super(...)` nem `this(...)`, numa classe
/// cuja superclasse tem algum gerador — o sem nome dela falta
/// (`undefined_constructor_in_initializer_default`), é factory
/// (`non_generative_constructor`) ou exige mais argumentos do que os
/// `super.x` dão (`implicit_super_initializer_missing_arguments`; sem
/// `super-parameters`, `no_default_super_constructor`). No nome da classe
/// (até o nome do construtor nos dois últimos).
fn super_implicito(inf: &mut BodyInferrer<'_>, c: ClassId, ctor: &ast::Constructor) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    if ctor.factory || ctor.external {
        return;
    }
    if ctor.initializers.iter().any(|i| matches!(i, ast::Initializer::Super { .. } | ast::Initializer::Redirect { .. })) {
        return;
    }
    let Some((s, st)) = supertipo_do_analyzer(inf, c) else { return };
    if so_fabricas(inf, s) {
        return;
    }
    let tipo = ctor.class_name.span;
    let lib = inf.program.class(c).library;
    // `superElement.unnamedConstructor`: sem o teste de acessibilidade.
    let Some(k) = achar_construtor(inf, s, None, inf.program.class(s).library) else {
        let nome = inf.interner.resolve(inf.program.class(s).name).to_string();
        inf.aviso_com_codigo(ce::UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT, tipo, &[&nome]);
        return;
    };
    if fabrica_e_const(inf, s, k).0 {
        // `{0}` = o elemento: `superElement.unnamedConstructor`, sem
        // substituição (o tipo é o da classe com os próprios parâmetros).
        let proprio = inf.tipo_this_classe(s);
        let exibido = exibir_construtor(inf, s, k, proprio);
        inf.aviso_com_codigo(ce::NON_GENERATIVE_CONSTRUCTOR, tipo, &[&exibido]);
        return;
    }
    let (posicionais, mut nomeados) = obrigatorios_do_construtor(inf, k);
    let faixa = dartforge_diagnostics::Span { start: tipo.start, end: ctor.name.map_or(tipo.end, |n| n.span.end) };
    let exibido = inf.table.format(st, inf.interner, inf.program);
    let super_parametros = inf.program.library(lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(2, 17);
    if !super_parametros {
        if posicionais != 0 || !nomeados.is_empty() {
            inf.aviso_com_codigo(ce::NO_DEFAULT_SUPER_CONSTRUCTOR_EXPLICIT, faixa, &[&exibido]);
        }
        return;
    }
    let super_posicionais = ctor.parameters.iter().filter(|p| p.super_ && p.kind != ast::ParameterKind::Named).count();
    let super_nomeados: Vec<dartforge_intern::SymbolId> = ctor.parameters.iter().filter(|p| p.super_ && p.kind == ast::ParameterKind::Named).filter_map(|p| p.nome_externo().map(|n| n.sym)).collect();
    nomeados.retain(|n| !super_nomeados.contains(n));
    if posicionais > super_posicionais || !nomeados.is_empty() {
        inf.aviso_com_codigo(ce::IMPLICIT_SUPER_INITIALIZER_MISSING_ARGUMENTS, faixa, &[&exibido]);
    }
}

/// `super(...)`/`this(...)`: invoca o construtor da classe instanciada.
/// Os parâmetros `super.x` do construtor corrente são argumentos
/// implícitos (posicionais depois dos explícitos; nomeados pelo nome).
fn chamar_construtor_de(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, alvo: Option<(ClassId, Box<[TypeId]>)>, nome: Option<ast::Name>, args: &ast::Arguments, params: &[ast::Parameter], span: dartforge_diagnostics::Span) {
    let u = inf.core.unknown;
    let chave = nome.map(|n| n.sym).or(inf.sym.vazio);
    let f = alvo.as_ref().and_then(|(c, _)| chave.and_then(|k| inf.program.class(*c).constructors.get(&k).copied()));
    match (alvo, f) {
        (Some((c, targs)), Some(f)) => {
            let sig = inf.outline.functions[f.0 as usize].signature;
            let ps = inf.outline.classes[c.0 as usize].type_params.clone();
            let sig = if ps.len() == targs.len() {
                let mapa = inf.mapa(&ps, &targs);
                inf.subst(sig, &mapa)
            } else {
                sig
            };
            // Aridade: os `super.x` contam como argumentos implícitos
            // (`verifySuperFormalParameters`); o nome citado é o do
            // construtor, ou `<tipo de retorno>.new`; a entidade do nomeado
            // obrigatório é a invocação inteira.
            let nome_citado = match nome {
                Some(n) => inf.interner.resolve(n.sym).to_string(),
                None => {
                    let ret = match inf.table.get(sig) {
                        Type::Function { ret, .. } => *ret,
                        _ => sig,
                    };
                    format!("{}.new", inf.table.format(ret, inf.interner, inf.program))
                }
            };
            let super_posicionais = params.iter().filter(|p| p.super_ && p.kind != ast::ParameterKind::Named).count();
            // `POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT`
            // (`verifySuperFormalParameters`,
            // an611:src/error/super_formal_parameters_verifier.dart:22-33): no
            // nome de cada `super.x` posicional, se o `super(...)` tem
            // argumento posicional.
            if args.args.iter().any(|a| a.name.is_none()) {
                for p in params.iter().filter(|p| p.super_ && p.kind != ast::ParameterKind::Named) {
                    if let Some(n) = p.name {
                        inf.aviso_com_codigo(
                            dartforge_diagnostics::codigos::compile_time_error::POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT,
                            n.span,
                            &[],
                        );
                    }
                }
            }
            let super_nomeados: Vec<_> =
                params.iter().filter(|p| p.super_ && p.kind == ast::ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
            inf.alvo_da_aridade = Some(super::chamadas::AlvoDaAridade {
                nome: Some(nome_citado),
                entidade: Some(span),
                super_posicionais,
                super_nomeados,
            });
            super::chamadas::invocar(inf, cx, sig, args, u, None);
            inf.alvo_da_aridade = None;
        }
        _ => {
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
        }
    }
}

/// Infere um corpo (bloco ou expressão) com o retorno já no contexto.
/// `fim`: onde termina o trecho morto do corpo (o padrão é a última
/// instrução do bloco).
fn corpo_de_funcao(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    body: &FunctionBody,
    m: AsyncModifier,
    ret: TypeId,
    fim: Option<usize>,
) {
    let _ = (m, ret);
    match body {
        FunctionBody::Expression(e) => {
            let ctx = cx.funcoes.last().map(|f| f.contexto_retorno).unwrap_or(inf.core.unknown);
            let t = inferir(inf, cx, *e, ctx);
            if let Some(fc) = cx.funcoes.last().cloned() {
                verificar_retorno_de_expressao(inf, cx, &fc, *e, t);
            }
        }
        FunctionBody::Block(s) => {
            let fim = fim.unwrap_or_else(|| instrucoes::fim_de_fluxo(inf, cx, *s));
            instrucoes::entrar_fluxo(cx, fim);
            instrucoes::inferir_instrucao(inf, cx, *s);
            instrucoes::sair_fluxo(cx);
        }
        _ => {}
    }
}

/// `ReturnTypeVerifier.verifyReturnType` (`an611:src/error/return_type_verifier.dart:79-130`,
/// `_isLegalReturnType` `:297-322`): com o retorno escrito, `async` exige
/// `Future<Never>` subtipo dele, `async*` `Stream<Never>`, `sync*`
/// `Iterable<Never>`, e um gerador não pode ser `void`; o relato vai no tipo
/// escrito. Devolve `hasLegalReturnType`.
fn tipo_de_retorno_legal(inf: &mut BodyInferrer<'_>, unit: UnitId, escrito: Option<ast::TypeId>, m: AsyncModifier, ret: TypeId) -> bool {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let Some(escrito) = escrito else { return true };
    let (classe, codigo) = match m {
        AsyncModifier::None => return true,
        AsyncModifier::Async => (inf.core.future_class, ce::ILLEGAL_ASYNC_RETURN_TYPE),
        AsyncModifier::AsyncStar => (inf.core.stream_class, ce::ILLEGAL_ASYNC_GENERATOR_RETURN_TYPE),
        AsyncModifier::SyncStar => (inf.core.iterable_class, ce::ILLEGAL_SYNC_GENERATOR_RETURN_TYPE),
    };
    let gerador = matches!(m, AsyncModifier::AsyncStar | AsyncModifier::SyncStar);
    let ilegal = if gerador && matches!(inf.table.get(ret), Type::Void) {
        true
    } else {
        let n = inf.core.never;
        let piso = inf.iface(classe, vec![n]);
        !inf.sub(piso, ret)
    };
    if ilegal {
        let sp = inf.program.unit(unit).ast.ty(escrito).span;
        inf.aviso_com_codigo(codigo, sp, &[]);
    }
    !ilegal
}

/// `ResolverVisitor.checkForBodyMayCompleteNormally`
/// (`an611:src/generated/resolver.dart:522-603`), com o fim do corpo de bloco
/// alcançável: sem contexto de retorno, nada (o `catchError` é outra regra);
/// gerador, nada; `async` com o imposto fora de `Future<Never>`, nada; o
/// contexto potencialmente não anulável dá `BODY_MIGHT_COMPLETE_NORMALLY`; o
/// anulável cuja base (`futureOrBase`) não é `dynamic`, `_`, `void` nem
/// `Null`, `BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE`.
fn corpo_completa_normalmente(inf: &mut BodyInferrer<'_>, imposto: Option<TypeId>, contexto: Option<TypeId>, m: AsyncModifier, onde: dartforge_diagnostics::Span) {
    use dartforge_diagnostics::codigos::{compile_time_error as ce, warning as w};
    let Some(rt) = contexto else { return };
    if matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) || inf.e_desconhecido(rt) {
        return;
    }
    if m == AsyncModifier::Async
        && let Some(i) = imposto
    {
        let n = inf.core.never;
        let piso = inf.futuro(n);
        if !inf.sub(piso, i) {
            return;
        }
    }
    // `isPotentiallyNonNullable` é `!isNullable` (type_system.dart:1242-1284):
    // `dynamic`, inválido, o desconhecido `_`, `void` e `Null` são anuláveis,
    // e `FutureOr<T>` é anulável se `T` for (`FutureOr<_>`, o contexto de uma
    // closure passada a `Future.delayed`, é anulável).
    fn anulavel(inf: &mut BodyInferrer<'_>, t: TypeId, fundo: u32) -> bool {
        if fundo > 32 {
            return false;
        }
        if inf.e_dynamic(t) || inf.table.e_invalido(t) || inf.e_desconhecido(t) || matches!(inf.table.get(t), Type::Void | Type::Null) {
            return true;
        }
        match inf.table.get(t).clone() {
            Type::FutureOr { arg, nullable } => nullable || anulavel(inf, arg, fundo + 1),
            Type::Intersection { bound, .. } => anulavel(inf, bound, fundo + 1),
            _ => inf.e_anulavel(t),
        }
    }
    if !anulavel(inf, rt, 0) {
        inf.aviso_com_args(ce::BODY_MIGHT_COMPLETE_NORMALLY, onde, &[crate::exibicao::Arg::Tipo(rt)]);
        return;
    }
    let mut base = rt;
    while let Type::FutureOr { arg, .. } = inf.table.get(base) {
        base = *arg;
    }
    if inf.e_dynamic(base) || inf.table.e_invalido(base) || inf.e_desconhecido(base) || matches!(inf.table.get(base), Type::Void | Type::Null) {
        return;
    }
    inf.aviso_com_args(w::BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE, onde, &[crate::exibicao::Arg::Tipo(rt)]);
}

/// `_checkForFutureCatchErrorOnError` (`an611:src/generated/resolver.dart:3971-4006`):
/// a expressão de função sem contexto de retorno, argumento posicional direto
/// de `x.catchError(...)` com `x` um `Future<T>`: o `{` relata
/// `BODY_MIGHT_COMPLETE_NORMALLY_CATCH_ERROR` com a base de `FutureOr<T>`.
fn catch_error_sem_retorno(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, onde: dartforge_diagnostics::Span) {
    let dartforge_frontend::pais::Pai::Expr(chamada) = inf.pai_de(cx.unit, e) else { return };
    let a = &inf.program.unit(cx.unit).ast;
    let ast::ExprKind::Call { target, arguments } = &a.expr(chamada).kind else { return };
    if !arguments.args.iter().any(|x| x.value == e && x.name.is_none()) {
        return;
    }
    let ast::ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { return };
    if inf.interner.resolve(name.sym) != "catchError" {
        return;
    }
    let Some(recv) = inf.body_types.units[cx.unit.0 as usize].static_types.get(alvo.0 as usize).copied() else { return };
    if !matches!(inf.table.get(recv), Type::Interface { .. }) {
        return;
    }
    let Some(args) = inf.como_instancia_de(recv, inf.core.future_class) else { return };
    let Some(&t) = args.first() else { return };
    let mut base = t;
    while let Type::FutureOr { arg, .. } = inf.table.get(base) {
        base = *arg;
    }
    if inf.e_dynamic(base) || inf.e_desconhecido(base) || matches!(inf.table.get(base), Type::Void | Type::Null) {
        return;
    }
    inf.aviso_com_args(dartforge_diagnostics::codigos::warning::BODY_MIGHT_COMPLETE_NORMALLY_CATCH_ERROR, onde, &[crate::exibicao::Arg::Tipo(base)]);
}

/// Tipo de uma variável de topo/campo sem tipo: sobreposição (campo que
/// sobrepõe um getter) ou inicializador (`Null` vira `dynamic`).
pub(crate) fn inferir_tipo_de_variavel_sem_tipo(inf: &mut BodyInferrer<'_>, vid: VariableId) -> TypeId {
    let v = inf.program.variable(vid);
    // Sobreposição de membro herdado (campos de instância).
    if !v.static_ {
        if let Some(c) = v.class {
            if let Some(t) = tipo_sobreposto(inf, c, vid) {
                if let Some((unit, init)) = inf.inicializador(vid) {
                    let mut cx = Corpo::para_variavel(inf, vid, unit);
                    inferir(inf, &mut cx, init, t);
                }
                return t;
            }
        }
    }
    match inf.inicializador(vid) {
        Some((unit, init)) => {
            let mut cx = Corpo::para_variavel(inf, vid, unit);
            let u = inf.core.unknown;
            let t = inferir(inf, &mut cx, init, u);
            if matches!(inf.table.get(t), Type::Null) {
                inf.core.dynamic_
            } else {
                t
            }
        }
        None => match v.node {
            // O tipo da constante vem do outline (a instanciação escrita ou a
            // dos limites); o tipo `this` da classe (`E<T>`) vazaria o `T`.
            VariableRef::EnumConstant { .. } => {
                let d = &inf.outline.variables[vid.0 as usize];
                match d.declared_type.or(d.inferred) {
                    Some(t) => t,
                    None => v.class.map(|c| inf.tipo_this_classe(c)).unwrap_or(inf.core.dynamic_),
                }
            }
            _ => inf.core.dynamic_,
        },
    }
}

/// O tipo de um campo de instância sem tipo pela inferência de sobrescrita
/// (`_inferAccessorOrField` com `field`): os getters e os setters que ele
/// sobrescreve nas superinterfaces diretas (`getOverridden2`), pela
/// assinatura combinada. Só getters: o retorno do getter combinado; só
/// setters: o parâmetro do setter combinado; os dois: o do getter num campo
/// `final`, e num não `final` o tipo só quando os dois coincidem. Sem nada
/// (ou sem coincidência), o inicializador decide.
fn tipo_sobreposto(inf: &mut BodyInferrer<'_>, c: ClassId, vid: VariableId) -> Option<TypeId> {
    let antes = inf.declarados_sem_tipar.replace(c);
    let r = tipo_sobreposto_em(inf, c, vid);
    inf.declarados_sem_tipar = antes;
    if inf.heranca_provisoria && antes.is_none() {
        inf.heranca = crate::heranca::Heranca::default();
        inf.heranca_provisoria = false;
    }
    r
}

fn tipo_sobreposto_em(inf: &mut BodyInferrer<'_>, c: ClassId, vid: VariableId) -> Option<TypeId> {
    let v = inf.program.variable(vid);
    let final_ = v.final_ || v.const_;
    let (getters, setters, n_getter, n_setter) = inf.getters_e_setters_sobrescritos(c, v.library, v.name);
    match (getters.is_empty(), setters.is_empty(), n_setter) {
        (false, true, _) => Some(inf.tipo_de_getter_combinado(c, &getters, n_getter)),
        (true, false, Some(ns)) => Some(inf.tipo_de_setter_combinado(c, &setters, ns)),
        (false, false, Some(ns)) => {
            let g = inf.tipo_de_getter_combinado(c, &getters, n_getter);
            if final_ {
                return Some(g);
            }
            let s = inf.tipo_de_setter_combinado(c, &setters, ns);
            (inf.table.canonico(g) == inf.table.canonico(s)).then_some(g)
        }
        _ => None,
    }
}

/// Extrai de um contexto o tipo de função (através de `?` e `FutureOr`).
fn funcao_do_contexto(inf: &mut BodyInferrer<'_>, ctx: TypeId) -> Option<TypeId> {
    if inf.e_desconhecido(ctx) {
        return None;
    }
    let t = inf.nao_nulo(ctx);
    match inf.table.get(t).clone() {
        Type::Function { .. } => Some(t),
        Type::FutureOr { arg, .. } => funcao_do_contexto(inf, arg),
        _ => None,
    }
}

/// Expressão de função (closure) no contexto `ctx`.
pub(crate) fn expressao_de_funcao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, fid: ast::FunctionId, ctx: TypeId, e: ExprId) -> TypeId {
    // `wasFunctionTypeSupplied` (`function_expression_resolver.dart:35`): o
    // contexto, como chegou, é um tipo de função.
    if matches!(inf.table.get(ctx), Type::Function { .. }) {
        inf.body_types.units[cx.unit.0 as usize].com_tipo_de_funcao.insert(fid);
    }
    let (t, _) = funcao_literal(inf, cx, fid, ctx, None, Some(e));
    t
}

/// Infere uma função literal ou local. `local` é o id da função local
/// (declarada antes, para chamadas recursivas).
fn funcao_literal(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, fid: ast::FunctionId, ctx: TypeId, local: Option<crate::resolved::LocalId>, expressao: Option<ExprId>) -> (TypeId, ()) {
    let af = inf.program.unit(cx.unit).ast.function(fid);
    let u = inf.core.unknown;
    let ctx_fn = funcao_do_contexto(inf, ctx);
    // Captura de escrita: locais externos escritos dentro da closure perdem
    // as promoções (aqui e depois dela).
    let escritos = instrucoes::nomes_escritos_em_funcao(inf, cx.unit, af);
    let mut capturados: Vec<crate::resolved::LocalId> = Vec::new();
    for n in &escritos {
        if let Some(id) = instrucoes::local_da_escrita(cx, *n) {
            capturados.push(id);
        }
    }
    let fluxo_antes = cx.fluxo.clone();
    let mut fluxo_dentro = cx.fluxo.clone();
    for &id in &capturados {
        fluxo_dentro.capturar(id);
    }
    for e in cx.escritos_em_closure.clone().iter() {
        if let Some(id) = instrucoes::local_da_escrita(cx, *e) {
            fluxo_dentro.capturar(id);
        }
    }
    if cx.escritos_no_corpo.is_none() {
        let a = &inf.program.unit(cx.unit).ast;
        cx.escritos_no_corpo = Some(match cx.raiz {
            // O membro inteiro, inclusive escritas nos próprios parâmetros
            // (R-FLU-07 regra 2; `base ??= const {}` e depois a closure).
            super::corpo::Raiz::Funcao(f) => instrucoes::nomes_escritos_separados(inf, cx.unit, &a.function(f).body),
            super::corpo::Raiz::Construtor(m) => match &a.member(m).kind {
                ast::MemberKind::Constructor(c) => instrucoes::nomes_escritos_separados(inf, cx.unit, &c.body),
                _ => Default::default(),
            },
            super::corpo::Raiz::Nada => Default::default(),
        });
    }
    // `functionExpression_begin`: `conservativeJoin(anywhere.written,
    // anywhere.captured)` — escrita em qualquer lugar do membro perde as
    // promoções dentro do literal mas continua promovível; escrita dentro de
    // algum literal do membro (antes ou depois deste) é capturada e não
    // promove (R-FLU-07 regra 2).
    let (fora, dentro) = cx.escritos_no_corpo.clone().unwrap_or_default();
    for e in fora.iter() {
        if let Some(id) = instrucoes::local_da_escrita(cx, *e) {
            fluxo_dentro.juncao_conservadora(&[id], &[]);
        }
    }
    for e in dentro.iter() {
        if let Some(id) = instrucoes::local_da_escrita(cx, *e) {
            fluxo_dentro.juncao_conservadora(&[], &[id]);
        }
    }
    cx.escritos_em_closure.extend(escritos.iter().copied());
    cx.fluxo = fluxo_dentro;
    cx.empurrar_escopo();
    // Parâmetros de tipo da função literal.
    let mut tps: Vec<TypeParamId> = Vec::new();
    for tp in af.type_params.iter() {
        let p = inf.table.alloc_type_param(tp.name.sym, crate::table::TypeParamOwner::GenericFunctionType, inf.core.object_nullable, crate::table::Variance::Unspecified);
        cx.declarar_tipo_param(tp.name.sym, p);
        tps.push(p);
    }
    for (tp, &p) in af.type_params.iter().zip(tps.iter()) {
        if let Some(b) = tp.bound {
            let bt = inf.tipo_de_anotacao(cx, b);
            inf.table.set_type_param_bound(p, bt);
        }
    }
    // Contexto de função com os parâmetros de tipo renomeados para os nossos.
    let (cpos, copt, cnamed, cret) = match ctx_fn.map(|t| inf.table.get(t).clone()) {
        Some(Type::Function { type_params, ret, positional, optional, named, .. }) => {
            let mapa: HashMap<TypeParamId, TypeId> = if type_params.len() == tps.len() {
                type_params.iter().copied().zip(tps.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false }))).collect()
            } else {
                HashMap::new()
            };
            let s = |inf: &mut BodyInferrer<'_>, t: TypeId| inf.subst(t, &mapa);
            let pos: Vec<TypeId> = positional.iter().map(|&t| s(inf, t)).collect();
            let opt: Vec<TypeId> = optional.iter().map(|&t| s(inf, t)).collect();
            let named: Vec<(dartforge_intern::SymbolId, TypeId, bool)> = named.iter().map(|(n, t, r)| (*n, s(inf, *t), *r)).collect();
            let r = s(inf, ret);
            (pos, opt, named, Some(r))
        }
        _ => (Vec::new(), Vec::new(), Vec::new(), None),
    };
    let todos_ctx: Vec<TypeId> = cpos.iter().chain(copt.iter()).copied().collect();
    let mut pos = Vec::new();
    let mut opt = Vec::new();
    let mut named = Vec::new();
    let mut ipos = 0usize;
    if let Some(ps) = &af.parameters {
        for p in ps.iter() {
            let escopo = cx.parametros_de_tipo_visiveis();
            let escrito = inf.tipo_de_parametro_escrito(cx.unit, cx.lib, p, &escopo);
            let do_ctx = match p.kind {
                ast::ParameterKind::Named => p.name.and_then(|n| cnamed.iter().find(|(s, _, _)| *s == n.sym).map(|(_, t, _)| *t)),
                _ => {
                    let t = todos_ctx.get(ipos).copied();
                    ipos += 1;
                    t
                }
            };
            let t = match escrito {
                Some(t) => t,
                None => match do_ctx {
                    Some(k) => {
                        let s = inf.fecho_maior(k);
                        let n = inf.core.null;
                        if inf.sub(s, n) {
                            inf.core.object_nullable
                        } else {
                            s
                        }
                    }
                    None => inf.core.dynamic_,
                },
            };
            if let Some(d) = p.default_value {
                let td = inferir(inf, cx, d, t);
                if !matches!(inf.table.get(t), Type::Void) {
                    expr::uso_de_void(inf, cx, d, td);
                }
            }
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    if let Some(n) = &p.name {
                        named.push((n.sym, t, p.required));
                    }
                }
            }
            if let Some(n) = &p.name {
                declarar_local(
                    inf,
                    cx,
                    Local { nome: n.sym, tipo: t, final_: p.final_, late: false, const_: false, offset: n.span.start, funcao_local: false },
                    true,
                );
            }
        }
    }
    // Retorno: escrito, ou inferido com o esquema imposto pelo contexto.
    let escrito_ret = af.return_type.map(|r| inf.tipo_de_anotacao(cx, r));
    let m = af.modifier;
    // O contexto do retorno inteiro (o tipo de execução de um gerador o usa).
    let contexto_completo = cret;
    let (ctx_ret, declarado) = match escrito_ret {
        Some(r) => (inf.contexto_de_retorno_declarado(r, m), Some(r)),
        None => {
            let s = cret.unwrap_or(u);
            let k = match m {
                AsyncModifier::None => s,
                AsyncModifier::AsyncStar => inf.como_instancia_de_esquema(s, inf.core.stream_class).unwrap_or(u),
                AsyncModifier::SyncStar => inf.como_instancia_de_esquema(s, inf.core.iterable_class).unwrap_or(u),
                AsyncModifier::Async => {
                    let fv = inf.tipo_valor_futuro_esquema(s);
                    inf.futuro_ou(fv)
                }
            };
            (k, None)
        }
    };
    // Função local com retorno escrito: tipo disponível para recursão.
    if let (Some(id), Some(r)) = (local, declarado) {
        let ft = inf.table.intern(Type::Function {
            type_params: tps.clone().into_boxed_slice(),
            ret: r,
            positional: pos.clone().into_boxed_slice(),
            optional: opt.clone().into_boxed_slice(),
            named: named.clone().into_boxed_slice(),
            nullable: false,
        });
        cx.locais[id.0 as usize].tipo = ft;
        cx.locais[id.0 as usize].funcao_local = false;
    }
    // Função local com nome: "function 'f'"; closure: outra regra (fora).
    let executavel = match (local, af.name) {
        (Some(_), Some(n)) => Some(super::corpo::Executavel {
            especie: super::corpo::EspecieExecutavel::Funcao,
            nome: inf.interner.resolve(n.sym).to_string(),
        }),
        _ => None,
    };
    // `verifyReturnType` da função local nomeada (`visitFunctionDeclaration`).
    let retorno_legal = match (local, declarado) {
        (Some(_), Some(r)) => tipo_de_retorno_legal(inf, cx.unit, af.return_type, m, r),
        _ => true,
    };
    cx.funcoes.push(CtxFuncao { modificador: m, retorno: declarado, contexto_retorno: ctx_ret, retornados: Vec::new(), retorno_vazio: false, expressoes_retornadas: Vec::new(), executavel, retorno_legal });
    let saltos_salvos = std::mem::take(&mut cx.saltos);
    let cascatas_salvas = std::mem::take(&mut cx.cascatas);
    let alvos_salvos = std::mem::take(&mut cx.alvos_de_cascata);
    let (corpo_t, completa) = match &af.body {
        FunctionBody::Expression(e) => {
            let t = inferir(inf, cx, *e, ctx_ret);
            if let Some(fc) = cx.funcoes.last().cloned() {
                verificar_retorno_de_expressao(inf, cx, &fc, *e, t);
            }
            if let Some(f) = cx.funcoes.last_mut() {
                f.expressoes_retornadas.push((*e, t));
            }
            (Some(t), false)
        }
        FunctionBody::Block(s) => {
            let fim = instrucoes::fim_de_fluxo(inf, cx, *s);
            instrucoes::entrar_fluxo(cx, fim);
            instrucoes::inferir_instrucao(inf, cx, *s);
            instrucoes::sair_fluxo(cx);
            let alcancavel = cx.fluxo.alcancavel;
            if alcancavel {
                // `checkForBodyMayCompleteNormally`: a função local no nome,
                // a expressão de função no `{`; o imposto é o retorno escrito
                // ou o do contexto.
                let imposto = declarado.or(cret);
                let onde = match (local, af.name) {
                    (Some(_), Some(n)) => n.span,
                    _ => {
                        let ini = inf.program.unit(cx.unit).ast.stmt(*s).span.start;
                        dartforge_diagnostics::Span { start: ini, end: ini + 1 }
                    }
                };
                corpo_completa_normalmente(inf, imposto, imposto.map(|_| ctx_ret), m, onde);
                if imposto.is_none()
                    && local.is_none()
                    && let Some(ex) = expressao
                {
                    catch_error_sem_retorno(inf, cx, ex, onde);
                }
            }
            (None, alcancavel)
        }
        _ => (None, false),
    };
    cx.saltos = saltos_salvos;
    cx.cascatas = cascatas_salvas;
    cx.alvos_de_cascata = alvos_salvos;
    let fc = cx.funcoes.pop().unwrap();
    cx.tirar_escopo();
    cx.fluxo = fluxo_antes;
    for &id in &capturados {
        cx.fluxo.capturar(id);
    }
    let mut execucao_do_gerador = None;
    let ret = match declarado {
        Some(r) => r,
        None => {
            // Tipo efetivamente retornado.
            let mut t = match corpo_t {
                Some(t) => t,
                None => {
                    if completa && !matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                        inf.core.null
                    } else {
                        inf.core.never
                    }
                }
            };
            let gerador = matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar);
            if corpo_t.is_none() {
                for &r in &fc.retornados {
                    t = inf.up(r, t);
                }
                // `return;` contribui `Null`, também em gerador: o analyzer
                // 3.6.2 (oráculo) junta os `return` aos `yield` do gerador
                // (a regra que ignora `return;` em gerador é posterior).
                if fc.retorno_vazio {
                    let n = inf.core.null;
                    t = inf.up(n, t);
                }
                // Gerador sem `yield` nem `return`: elemento `dynamic`
                // (analyzer 3.6.2), não `Never`.
                if gerador && fc.retornados.is_empty() && !fc.retorno_vazio {
                    t = inf.core.dynamic_;
                }
            } else if m == AsyncModifier::Async {
                t = inf.flatten(t);
            }
            let r = inf.fecho_maior(ctx_ret);
            let embrulhar = |inf: &mut BodyInferrer<'_>, t: TypeId| {
                let s = if matches!(inf.table.get(r), Type::Void)
                    || (m == AsyncModifier::Async && matches!(inf.table.get(r), Type::FutureOr { arg, .. } if matches!(inf.table.get(*arg), Type::Void)))
                {
                    inf.core.void_
                } else if inf.sub(t, r) {
                    t
                } else {
                    r
                };
                match m {
                    AsyncModifier::Async => {
                        let f = inf.flatten(s);
                        inf.futuro(f)
                    }
                    AsyncModifier::AsyncStar => inf.fluxo_de(s),
                    AsyncModifier::SyncStar => inf.iteravel(s),
                    AsyncModifier::None => s,
                }
            };
            let estatico = embrulhar(inf, t);
            // O retorno inferido não cabe no do contexto e foi trocado por
            // ele: cada `return` confere contra esse tipo
            // (`RETURN_OF_INVALID_TYPE_FROM_CLOSURE`).
            // Closure `async` em contexto de retorno `void`: o retorno dela é
            // `Future<void>` e todo `return e;` com `flatten(e)` fora de
            // `void`/`dynamic`/`Null` é erro (`return_type_verifier.dart:245-256`).
            let async_void = m == AsyncModifier::Async && matches!(inf.table.get(r), Type::Void);
            if fc.executavel.is_none() && !gerador && ((!inf.sub(t, r) && !matches!(inf.table.get(r), Type::Void | Type::Dynamic) && !inf.e_desconhecido(r)) || async_void) {
                retornos_da_closure(inf, cx, &fc, r, estatico);
            }
            // O tipo de execução do gerador (a regra do CFE, conferida contra
            // a VM 3.6.2 e a 3.13.4): o elemento é o limite superior dos
            // `yield` (o `return;` não conta; `Null` sem nenhum), embrulhado
            // (`Iterable`/`Stream`); o CFE compara esse tipo embrulhado com o
            // ELEMENTO do contexto e, se não for subtipo dele, o retorno é o
            // tipo de retorno do contexto, inteiro (`Iterable<num>? Function()`
            // dá `Iterable<num>?`).
            if gerador && corpo_t.is_none() {
                let mut e = if fc.retornados.is_empty() { inf.core.null } else { inf.core.never };
                for &y in &fc.retornados {
                    e = inf.up(y, e);
                }
                let embrulhado = match m {
                    AsyncModifier::AsyncStar => inf.fluxo_de(e),
                    _ => inf.iteravel(e),
                };
                let execucao = if inf.sub(embrulhado, r) {
                    embrulhado
                } else {
                    let contexto = contexto_completo.unwrap_or(inf.core.unknown);
                    inf.fecho_maior(contexto)
                };
                if execucao != estatico {
                    execucao_do_gerador = Some(execucao);
                }
            }
            estatico
        }
    };
    if let Some(r) = execucao_do_gerador {
        let fe = inf.table.intern(Type::Function {
            type_params: tps.clone().into_boxed_slice(),
            ret: r,
            positional: pos.clone().into_boxed_slice(),
            optional: opt.clone().into_boxed_slice(),
            named: named.clone().into_boxed_slice(),
            nullable: false,
        });
        inf.body_types.units[cx.unit.0 as usize].tipos_de_execucao_de_funcoes.insert(fid, fe);
    }
    let ft = inf.table.intern(Type::Function {
        type_params: tps.into_boxed_slice(),
        ret,
        positional: pos.into_boxed_slice(),
        optional: opt.into_boxed_slice(),
        named: named.into_boxed_slice(),
        nullable: false,
    });
    (ft, ())
}

impl<'a> BodyInferrer<'a> {
    /// `S` é `C<S1>` (sem subtipos): `S1`.
    pub(crate) fn como_instancia_de_esquema(&mut self, s: TypeId, c: Option<ClassId>) -> Option<TypeId> {
        let c = c?;
        match self.table.get(s) {
            Type::Interface { class, args, .. } if *class == c && args.len() == 1 => Some(args[0]),
            _ => None,
        }
    }
}

/// Declaração de função local.
pub(crate) fn funcao_local(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, fid: ast::FunctionId) {
    let af = inf.program.unit(cx.unit).ast.function(fid);
    let Some(nome) = af.name else { return };
    let d = inf.core.dynamic_;
    let id = cx.declarar(Local { nome: nome.sym, tipo: d, final_: true, late: false, const_: false, offset: nome.span.start, funcao_local: true });
    cx.funcoes_locais.insert(id);
    cx.fluxo.inicializar(id);
    let u = inf.core.unknown;
    let (t, _) = funcao_literal(inf, cx, fid, u, Some(id), None);
    cx.locais[id.0 as usize].tipo = t;
    cx.locais[id.0 as usize].funcao_local = false;
    inf.body_types.units[cx.unit.0 as usize].set_tipo_local(nome.span.start, t);
    // A função local com retorno escrito (`parent.returnType?.type`).
    if let Some(rt) = inf.program.unit(cx.unit).ast.function(fid).return_type
        && let Some(&retorno) = inf.body_types.units[cx.unit.0 as usize].tipos_de_anotacoes.get(&rt)
    {
        conjunto_desnecessario(inf, cx.unit, fid, retorno);
    }
}

/// `retorno` é `void`, `Future<void>` ou `FutureOr<void>`.
fn retorno_void(inf: &BodyInferrer<'_>, t: TypeId) -> bool {
    match inf.table.get(t) {
        Type::Void => true,
        Type::Interface { class, args, .. } if Some(*class) == inf.core.future_class => args.len() == 1 && matches!(inf.table.get(args[0]), Type::Void),
        Type::FutureOr { arg, .. } => matches!(inf.table.get(*arg), Type::Void),
        _ => false,
    }
}

/// `UNNECESSARY_SET_LITERAL` (`_checkForUnnecessarySetLiteral`,
/// `an611:src/error/best_practices_verifier.dart:1283-1318`): o corpo `=> {…}`
/// é um literal de conjunto e o retorno (o do parâmetro que recebe a closure,
/// ou o escrito da declaração) é `void`, `Future<void>` ou `FutureOr<void>`;
/// no literal.
pub(crate) fn conjunto_desnecessario(inf: &mut BodyInferrer<'_>, unit: UnitId, fid: ast::FunctionId, retorno: TypeId) {
    let a = &inf.program.unit(unit).ast;
    let ast::FunctionBody::Expression(e) = a.function(fid).body else { return };
    if !matches!(a.expr(e).kind, ast::ExprKind::SetOrMap { .. }) || !retorno_void(inf, retorno) {
        return;
    }
    let Some(t) = inf.body_types.units[unit.0 as usize].get_type(e) else { return };
    if matches!(inf.table.get(t), Type::Interface { class, .. } if Some(*class) == inf.core.set_class) {
        let sp = inf.span_expr(unit, e);
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::warning::UNNECESSARY_SET_LITERAL, sp, &[]);
    }
}

/// Metadados (anotações) e argumentos de constantes de enum de uma unidade.
pub(crate) fn inferir_metadados_da_unidade(inf: &mut BodyInferrer<'_>, unit: UnitId) {
    let a = &inf.program.unit(unit).ast;
    for d in a.decls.iter() {
        let (classe, _) = classe_da_decl(inf, unit, d);
        // A anotação da declaração é resolvida no escopo de fora dela (o da
        // biblioteca): os membros do contêiner não estão nele.
        for m in d.metadata.iter() {
            anotacao(inf, unit, None, None, m);
        }
        // As anotações dos parâmetros de tipo: no escopo dos parâmetros de
        // tipo, cujo pai é o da biblioteca (os membros do contêiner não).
        let tps: &[ast::TypeParameter] = match &d.kind {
            ast::DeclKind::Class(x) => &x.type_params,
            ast::DeclKind::Mixin(x) => &x.type_params,
            ast::DeclKind::Enum(x) => &x.type_params,
            ast::DeclKind::Extension(x) => &x.type_params,
            ast::DeclKind::ExtensionType(x) => &x.type_params,
            ast::DeclKind::Typedef(x) => &x.type_params,
            ast::DeclKind::Function(_) | ast::DeclKind::Variables(_) => &[],
        };
        for tp in tps.iter() {
            for m in tp.metadata.iter() {
                anotacao(inf, unit, None, None, m);
            }
        }
        match &d.kind {
            ast::DeclKind::Enum(en) => {
                let Some(c) = classe else { continue };
                for k in en.constants.iter() {
                    for m in k.metadata.iter() {
                        anotacao(inf, unit, Some(c), None, m);
                    }
                    // `ResolverVisitor.visitEnumConstantDeclaration`
                    // (`an611:src/generated/resolver.dart:2483-2520`), antes
                    // dos argumentos: o construtor da criação sintética é
                    // factory, ou não existe (o sintético `const` só sem
                    // construtor declarado).
                    let chave = match k.constructor.map(|n| n.sym) {
                        Some(x) if Some(x) == inf.sym.new_ => inf.sym.vazio,
                        Some(x) => Some(x),
                        None => inf.sym.vazio,
                    };
                    {
                        use dartforge_diagnostics::codigos::compile_time_error as ce;
                        let ce_enum = inf.program.class(c);
                        let declarado = chave.and_then(|ch| ce_enum.constructors.get(&ch).copied());
                        let sintetico = chave == inf.sym.vazio && ce_enum.constructors.is_empty();
                        match declarado {
                            Some(f) if inf.program.function(f).factory => {
                                let sp = k.constructor.map_or(k.name.span, |n| n.span);
                                inf.aviso_com_codigo(ce::ENUM_CONSTANT_INVOKES_FACTORY_CONSTRUCTOR, sp, &[]);
                            }
                            Some(_) => {}
                            None if sintetico => {}
                            None => match k.constructor {
                                Some(n) => {
                                    let nome = inf.interner.resolve(n.sym).to_string();
                                    inf.aviso_com_codigo(ce::UNDEFINED_ENUM_CONSTRUCTOR_NAMED, n.span, &[&nome]);
                                }
                                None => inf.aviso_com_codigo(ce::UNDEFINED_ENUM_CONSTRUCTOR_UNNAMED, k.name.span, &[]),
                            },
                        }
                        // `TypeArgumentsVerifier.checkEnumConstantDeclaration`
                        // (`type_arguments_verifier.dart:93-112`): com o
                        // construtor resolvido, a lista `<…>` escrita com
                        // contagem diferente da do enum.
                        let resolvido = declarado.is_some() || sintetico;
                        let esperados = inf.program.class(c).type_params.len();
                        if resolvido
                            && let (Some(&primeiro), Some(&ultimo)) = (k.type_args.first(), k.type_args.last())
                            && k.type_args.len() != esperados
                        {
                            let fonte = &inf.program.unit(unit).source;
                            let ini_arg = a.ty(primeiro).span.start;
                            let fim_arg = a.ty(ultimo).span.end;
                            let inicio = fonte[..ini_arg].rfind('<').unwrap_or(ini_arg);
                            let fim = fonte[fim_arg..].find('>').map_or(fim_arg, |i| fim_arg + i + 1);
                            let (e, d) = (esperados.to_string(), k.type_args.len().to_string());
                            inf.aviso_com_codigo(ce::WRONG_NUMBER_OF_TYPE_ARGUMENTS_ENUM, dartforge_diagnostics::Span { start: inicio, end: fim }, &[&e, &d]);
                        }
                    }
                    if let Some(args) = &k.arguments {
                        let mut cx = Corpo::novo(inf, unit, Some(c), None, true);
                        let f = chave.and_then(|ch| inf.program.class(c).constructors.get(&ch).copied());
                        if let Some(f) = f {
                            let explicitos = if k.type_args.is_empty() {
                                None
                            } else {
                                Some(k.type_args.iter().map(|&t| inf.tipo_de_argumento_de_tipo(&cx, t)).collect())
                            };
                            let u = inf.core.unknown;
                            let nome_enum = inf.interner.resolve(en.name.sym).to_string();
                            super::chamadas::definir_alvo(inf, Some(nome_enum), k.name.span);
                            super::chamadas::construir(inf, &mut cx, None, c, Some(f), explicitos, args, u);
                            inf.alvo_da_aridade = None;
                            // `checkEnumConstantDeclaration` do `TypeArgumentsVerifier`:
                            // os argumentos de tipo do construtor (escritos ou
                            // inferidos), no argumento escrito ou no nome.
                            let params = inf.outline.classes[c.0 as usize].type_params.to_vec();
                            if let Some(tipos) = inf.body_types.units[unit.0 as usize].instanciacao(args.span.start).map(|x| x.to_vec())
                                && tipos.len() == params.len()
                            {
                                let nos = k.type_args.to_vec();
                                super::chamadas::conferir_limites_explicitos(inf, unit, &params, &tipos, &nos, k.name.span, None);
                            }
                        } else if chave == inf.sym.vazio && inf.program.class(c).constructors.is_empty() {
                            // Construtor sem nome implícito `const E()`: os
                            // argumentos passam pela aridade dele (sem
                            // parâmetros).
                            let ret = inf.tipo_this_classe(c);
                            let sig = inf.table.intern(Type::Function {
                                type_params: Box::new([]),
                                ret,
                                positional: Box::new([]),
                                optional: Box::new([]),
                                named: Box::new([]),
                                nullable: false,
                            });
                            let nome_enum = inf.interner.resolve(en.name.sym).to_string();
                            super::chamadas::definir_alvo(inf, Some(nome_enum), k.name.span);
                            let u = inf.core.unknown;
                            super::chamadas::invocar(inf, &mut cx, sig, args, u, None);
                            inf.alvo_da_aridade = None;
                            // `checkEnumConstantDeclaration`: os argumentos
                            // escritos do construtor implícito.
                            let params = inf.outline.classes[c.0 as usize].type_params.to_vec();
                            if !k.type_args.is_empty() && k.type_args.len() == params.len() {
                                let nos = k.type_args.to_vec();
                                let tipos: Vec<TypeId> = nos.iter().map(|&t| inf.tipo_de_argumento_de_tipo(&cx, t)).collect();
                                super::chamadas::conferir_limites_explicitos(inf, unit, &params, &tipos, &nos, k.name.span, None);
                            }
                        } else {
                            for x in args.args.iter() {
                                inferir_livre(inf, &mut cx, x.value);
                            }
                        }
                    } else if let Some(f) = inf.sym.vazio.and_then(|v| inf.program.class(c).constructors.get(&v).copied()) {
                        let nome_enum = inf.interner.resolve(en.name.sym).to_string();
                        super::chamadas::aridade_sem_argumentos(inf, f, nome_enum, k.name.span);
                    }
                }
            }
            _ => {}
        }
    }
    for mb in a.members.iter() {
        for m in mb.metadata.iter() {
            // Classe dona desconhecida aqui: resolve no escopo da biblioteca.
            anotacao(inf, unit, None, None, m);
        }
        if let ast::MemberKind::Constructor(ctor) = &mb.kind {
            for p in ctor.parameters.iter() {
                for m in p.metadata.iter() {
                    anotacao(inf, unit, None, None, m);
                }
            }
        }
    }
    // Funções de topo e métodos (as locais e os literais veem locais que a
    // anotação pode citar: ficam sem a validação de nomes).
    let declaradas: std::collections::HashMap<u32, (Option<ClassId>, Option<dartforge_elements::model::ExtensionId>)> = inf
        .program
        .functions
        .iter()
        .filter_map(|fe| match fe.node {
            dartforge_elements::model::FunctionRef::Function { unit: u, function } if u == unit => Some((function.0, (fe.class, fe.extension))),
            _ => None,
        })
        .collect();
    for (i, f) in a.functions.iter().enumerate() {
        let validar = declaradas.contains_key(&(i as u32));
        // Os parâmetros de tipo de um método veem o escopo do contêiner.
        let (dona, ext_dona) = declaradas.get(&(i as u32)).copied().unwrap_or((None, None));
        for tp in f.type_params.iter() {
            for m in tp.metadata.iter() {
                if validar {
                    anotacao(inf, unit, dona, ext_dona, m);
                } else {
                    anotacao_sem_validar(inf, unit, None, m);
                }
            }
        }
        if let Some(ps) = &f.parameters {
            for p in ps.iter() {
                for m in p.metadata.iter() {
                    if validar {
                        anotacao(inf, unit, None, None, m);
                    } else {
                        anotacao_sem_validar(inf, unit, None, m);
                    }
                }
            }
        }
    }
}

fn classe_da_decl(inf: &BodyInferrer<'_>, unit: UnitId, d: &ast::Decl) -> (Option<ClassId>, Option<dartforge_elements::model::ExtensionId>) {
    let lib = inf.program.unit(unit).library;
    let nome = match &d.kind {
        ast::DeclKind::Class(c) => Some(c.name.sym),
        ast::DeclKind::Mixin(m) => Some(m.name.sym),
        ast::DeclKind::Enum(e) => Some(e.name.sym),
        ast::DeclKind::ExtensionType(e) => Some(e.name.sym),
        _ => None,
    };
    match nome.and_then(|n| inf.program.library(lib).declared.get(&n)).and_then(|b| b.getter) {
        Some(Element::Class(c)) => (Some(c), None),
        _ => (None, None),
    }
}

/// O que um nome da anotação é, para o `AnnotationResolver`.
enum NaAnotacao {
    Nada,
    Prefixo,
    Classe(ClassId),
    Extensao(dartforge_elements::model::ExtensionId),
    Alias,
    /// Getter: o implícito de uma variável (`Some(const)`) ou um escrito (`None`).
    Getter(Option<bool>),
    Outro,
}

fn classificar_na_anotacao(inf: &BodyInferrer<'_>, b: Option<dartforge_elements::model::Binding>) -> NaAnotacao {
    use dartforge_elements::model::FunctionKind as K;
    match b.and_then(|b| b.getter) {
        None => NaAnotacao::Nada,
        Some(Element::Prefix(..)) => NaAnotacao::Prefixo,
        Some(Element::Class(c)) => NaAnotacao::Classe(c),
        Some(Element::Extension(x)) => NaAnotacao::Extensao(x),
        Some(Element::Typedef(_)) => NaAnotacao::Alias,
        Some(Element::Variable(v)) => NaAnotacao::Getter(Some(inf.program.variable(v).const_)),
        Some(Element::Function(f)) => {
            let fe = inf.program.function(f);
            match (fe.kind, fe.variable) {
                (K::ImplicitAccessor, Some(v)) => NaAnotacao::Getter(Some(inf.program.variable(v).const_)),
                (K::Getter, _) => NaAnotacao::Getter(None),
                _ => NaAnotacao::Outro,
            }
        }
    }
}

/// O getter estático `nome` de uma classe ou extensão (`getGetter`):
/// `Some(Some(const))` para o implícito de um campo (ou constante de enum,
/// ou `values`), `Some(None)` para um getter escrito.
fn getter_estatico(
    inf: &BodyInferrer<'_>,
    classe: Option<ClassId>,
    ext: Option<dartforge_elements::model::ExtensionId>,
    nome: dartforge_intern::SymbolId,
) -> Option<Option<bool>> {
    use dartforge_elements::model::FunctionKind as K;
    let (estaticos, campos) = match (classe, ext) {
        (Some(c), _) => {
            let k = inf.program.class(c);
            if k.enum_constants.iter().any(|&v| inf.program.variable(v).name == nome) {
                return Some(Some(true));
            }
            if k.kind == dartforge_elements::model::ClassKind::Enum && inf.interner.resolve(nome) == "values" {
                return Some(Some(true));
            }
            (&k.static_members, k.fields.clone())
        }
        (None, Some(x)) => {
            let e = inf.program.extension(x);
            (&e.static_members, e.fields.clone())
        }
        _ => return None,
    };
    if let Some(&f) = estaticos.get(&nome) {
        let fe = inf.program.function(f);
        return match (fe.kind, fe.variable) {
            (K::ImplicitAccessor, Some(v)) => Some(Some(inf.program.variable(v).const_)),
            (K::Getter, _) => Some(None),
            _ => None,
        };
    }
    campos
        .iter()
        .find(|&&v| inf.program.variable(v).name == nome && inf.program.variable(v).static_)
        .map(|&v| Some(inf.program.variable(v).const_))
}

/// O `AnnotationResolver` (`an611:src/dart/resolver/annotation_resolver.dart:
/// 30-420`): `UNDEFINED_ANNOTATION` no nome que não resolve,
/// `INVALID_ANNOTATION` na anotação que não é referência a constante nem
/// invocação de construtor constante. Os nomes são resolvidos no escopo da
/// biblioteca (o da classe, só quando ela é conhecida).
fn validar_anotacao(inf: &mut BodyInferrer<'_>, unit: UnitId, classe: Option<ClassId>, m: &ast::Annotation) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let span = m.span;
    let Some(&n1) = m.name.first() else { return };
    let args = m.arguments.is_some();
    let n2 = m.name.get(1).copied();
    // Constantes estáticas da classe em que a anotação está.
    if m.name.len() == 1 {
        if let Some(g) = getter_estatico(inf, classe, None, n1.sym) {
            if g != Some(true) || args {
                inf.aviso_com_codigo(c::INVALID_ANNOTATION, span, &[]);
            }
            return;
        }
    }
    let b1 = inf.program.lookup_na_unidade(unit, n1.sym);
    let mut e1 = classificar_na_anotacao(inf, b1);
    if matches!(e1, NaAnotacao::Nada) && inf.program.prefixos_na_unidade(unit).contains_key(&n1.sym) {
        e1 = NaAnotacao::Prefixo;
    }
    // Para o prefixo: o segundo nome resolvido nele, e o terceiro é o membro.
    let (alvo, membro, nome_alvo) = match e1 {
        NaAnotacao::Prefixo => {
            let Some(n2) = n2 else {
                inf.aviso_com_codigo(c::INVALID_ANNOTATION, span, &[]);
                return;
            };
            let b2 = inf.program.lookup_prefixed_na_unidade(unit, n1.sym, n2.sym);
            match classificar_na_anotacao(inf, b2) {
                NaAnotacao::Prefixo | NaAnotacao::Outro => {
                    inf.aviso_com_codigo(c::INVALID_ANNOTATION, span, &[]);
                    return;
                }
                x => (x, m.name.get(2).copied(), n2),
            }
        }
        x => (x, n2, n1),
    };
    let invalida = match alvo {
        NaAnotacao::Nada => {
            let t = inf.interner.resolve(nome_alvo.sym).to_string();
            inf.aviso_com_codigo(c::UNDEFINED_ANNOTATION, span, &[&t]);
            return;
        }
        NaAnotacao::Classe(k) => {
            let kind = inf.program.class(k).kind;
            let e_classe = matches!(kind, dartforge_elements::model::ClassKind::Class | dartforge_elements::model::ClassKind::MixinApplication);
            // `C.new` é o construtor sem nome.
            let ctor = |inf: &BodyInferrer<'_>, n: Option<ast::Name>| {
                let n = n.filter(|n| inf.interner.resolve(n.sym) != "new");
                let chave = n.map(|n| n.sym).or(inf.sym.vazio);
                chave.and_then(|ch| inf.program.class(k).constructors.get(&ch).copied()).is_some()
            };
            if kind == dartforge_elements::model::ClassKind::ExtensionType {
                // O construtor (primário) de um extension type: o modelo não
                // o põe na tabela pelo nome; fica sem relato.
                false
            } else if e_classe && args {
                let achou = ctor(inf, membro);
                // O construtor existe e não é `const`
                // (`constant_verifier.dart:103-115`): na anotação inteira.
                if achou {
                    let n = membro.filter(|n| inf.interner.resolve(n.sym) != "new");
                    let chave = n.map(|n| n.sym).or(inf.sym.vazio);
                    let e_const = chave
                        .and_then(|ch| inf.program.class(k).constructors.get(&ch).copied())
                        .is_some_and(|f| inf.program.function(inf.program.publico(f)).const_);
                    if !e_const {
                        inf.aviso_com_codigo(c::NON_CONSTANT_ANNOTATION_CONSTRUCTOR, span, &[]);
                    }
                }
                !achou
            } else {
                // Sem lista de argumentos, a anotação que resolve a um
                // construtor `const` é `NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS`
                // (`constant_verifier.dart:103-128`), na anotação inteira; o
                // construtor que não é `const` tem o código dele.
                let construtor_const = |inf: &BodyInferrer<'_>, n: Option<ast::Name>| -> bool {
                    let n = n.filter(|n| inf.interner.resolve(n.sym) != "new");
                    let chave = n.map(|n| n.sym).or(inf.sym.vazio);
                    chave
                        .and_then(|ch| inf.program.class(k).constructors.get(&ch).copied())
                        .is_some_and(|f| inf.program.function(inf.program.publico(f)).const_)
                };
                let do_membro = match membro {
                    Some(n) => match getter_estatico(inf, Some(k), None, n.sym) {
                        Some(g) => Some(g != Some(true) || args),
                        None => None,
                    },
                    None => None,
                };
                match do_membro {
                    Some(invalida) => invalida,
                    None => {
                        let achou = ctor(inf, membro);
                        if achou && !args && e_classe && construtor_const(inf, membro) {
                            inf.aviso_com_codigo(c::NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS, span, &[]);
                        }
                        !achou
                    }
                }
            }
        }
        NaAnotacao::Extensao(x) => match membro.and_then(|n| getter_estatico(inf, None, Some(x), n.sym)) {
            Some(g) => g != Some(true) || args,
            None => true,
        },
        NaAnotacao::Getter(g) => g != Some(true) || args,
        // Alias: o tipo apelidado decide; fica sem relato.
        NaAnotacao::Alias => false,
        NaAnotacao::Prefixo | NaAnotacao::Outro => true,
    };
    if invalida {
        inf.aviso_com_codigo(c::INVALID_ANNOTATION, span, &[]);
    }
}

/// Uma anotação: `@x`, `@C(args)`, `@C.nome(args)`, `@p.C(args)`.
fn anotacao(inf: &mut BodyInferrer<'_>, unit: UnitId, classe: Option<ClassId>, _ext: Option<dartforge_elements::model::ExtensionId>, m: &ast::Annotation) {
    validar_anotacao(inf, unit, classe, m);
    anotacao_sem_validar(inf, unit, classe, m);
}

/// Os argumentos da anotação (inferência), sem a validação dos nomes.
fn anotacao_sem_validar(inf: &mut BodyInferrer<'_>, unit: UnitId, classe: Option<ClassId>, m: &ast::Annotation) {
    let Some(args) = &m.arguments else { return };
    let mut cx = Corpo::novo(inf, unit, classe, None, true);
    let nomes: Vec<dartforge_intern::SymbolId> = m.name.iter().map(|n| n.sym).collect();
    // Classe e construtor.
    let (c, ctor) = match nomes.as_slice() {
        [c] => (inf.program.lookup_na_unidade(unit, *c).and_then(|b| b.getter), None),
        [a, b] => match inf.program.lookup_na_unidade(unit, *a).and_then(|x| x.getter) {
            Some(el @ Element::Class(_)) => (Some(el), Some(*b)),
            _ => (inf.program.lookup_prefixed_na_unidade(unit, *a, *b).and_then(|x| x.getter), None),
        },
        [p, c, n] => (inf.program.lookup_prefixed_na_unidade(unit, *p, *c).and_then(|x| x.getter), Some(*n)),
        _ => (None, None),
    };
    let u = inf.core.unknown;
    // `@B<T>()` com `B` alias de classe: os argumentos contra os parâmetros
    // de tipo do alias (o `AnnotationInferrer` com o construtor do alias).
    let c_original = c;
    let (c, params_do_alias) = match c {
        Some(Element::Typedef(td)) => {
            let alvo = inf.outline.typedefs[td.0 as usize].target_type;
            match inf.table.get(alvo).clone() {
                Type::Interface { class, .. } => (Some(Element::Class(class)), Some(inf.outline.typedefs[td.0 as usize].type_params.to_vec())),
                _ => (c, None),
            }
        }
        _ => (c, None),
    };
    if let Some(Element::Class(c)) = c {
        let chave = ctor.or(inf.sym.vazio);
        if let Some(f) = chave.and_then(|k| inf.program.class(c).constructors.get(&k).copied()) {
            let explicitos: Option<Vec<TypeId>> = if m.type_args.is_empty() {
                None
            } else {
                Some(m.type_args.iter().map(|&t| inf.tipo_de_argumento_de_tipo(&cx, t)).collect())
            };
            // `_needsTypeArgumentBoundsCheck` do `AnnotationInferrer`.
            if let Some(ex) = &explicitos {
                let params = params_do_alias.clone().unwrap_or_else(|| inf.outline.classes[c.0 as usize].type_params.to_vec());
                // `_reportWrongNumberOfTypeArguments` (`invocation_inferrer.dart:316-327`)
                // com o código do `AnnotationInferrer`: `{0}` é o tipo de
                // função cru do construtor, na lista `<…>`.
                if ex.len() != params.len()
                    && let Some(sp) = super::chamadas::faixa_da_lista_de_tipos(inf, unit, &m.type_args)
                {
                    // O tipo do construtor como função genérica nos parâmetros
                    // de tipo da classe (ou do alias, com o alvo dele).
                    let mut cru = inf.assinatura_construtor(c, f);
                    if params_do_alias.is_some()
                        && let Some(Element::Typedef(td)) = c_original
                        && let Type::Interface { args: alvo_args, .. } = inf.table.get(inf.outline.typedefs[td.0 as usize].target_type).clone()
                    {
                        let formais = inf.outline.classes[c.0 as usize].type_params.to_vec();
                        let mapa = inf.mapa(&formais, &alvo_args);
                        cru = inf.subst(cru, &mapa);
                    }
                    if let Type::Function { ret, positional, optional, named, .. } = inf.table.get(cru).clone() {
                        cru = inf.table.intern(Type::Function { type_params: params.clone().into_boxed_slice(), ret, positional, optional, named, nullable: false });
                    }
                    let texto = inf.table.format(cru, inf.interner, inf.program);
                    let (n, d) = (params.len().to_string(), ex.len().to_string());
                    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS, sp, &[&texto, &n, &d]);
                }
                if ex.len() == params.len() {
                    let (ex, nos) = (ex.clone(), m.type_args.to_vec());
                    super::chamadas::conferir_limites_explicitos(inf, unit, &params, &ex, &nos, m.span, None);
                }
            }
            if params_do_alias.is_some() {
                for x in args.args.iter() {
                    inferir_livre(inf, &mut cx, x.value);
                }
                return;
            }
            // `_reportNotEnoughPositionalArguments` com `Annotation`: o
            // `identifier` do nome prefixado (`@A.nome` → `nome`, `@p.A` →
            // `A`), ou `<nome>.new`; o `MISSING_REQUIRED_ARGUMENT` vai no
            // identificador do construtor, ou no da classe.
            let prefixo0 = m.name.first().is_some_and(|n| inf.program.prefixos_na_unidade(unit).contains_key(&n.sym));
            let nome_citado = match m.name.as_slice() {
                [a] => format!("{}.new", inf.interner.resolve(a.sym)),
                [a, b] if !m.type_args.is_empty() && !prefixo0 => {
                    let _ = b;
                    format!("{}.new", inf.interner.resolve(a.sym))
                }
                [_, b, ..] => inf.interner.resolve(b.sym).to_string(),
                [] => String::new(),
            };
            let entidade = match m.name.as_slice() {
                [a] => a.span,
                [_, b] => b.span,
                [_, _, c] => c.span,
                _ => m.span,
            };
            super::chamadas::definir_alvo(inf, Some(nome_citado), entidade);
            super::chamadas::construir(inf, &mut cx, None, c, Some(f), explicitos, args, u);
            inf.alvo_da_aridade = None;
            return;
        }
    }
    for x in args.args.iter() {
        inferir_livre(inf, &mut cx, x.value);
    }
}

/// `_checkReturnExpression` de uma closure cujo retorno é o do contexto `r`
/// (embrulhado: `exibido`): `RETURN_OF_INVALID_TYPE_FROM_CLOSURE`
/// (`an611:src/error/return_type_verifier.dart:150-170`), com o tipo
/// retornado e o da closure.
fn retornos_da_closure(inf: &mut BodyInferrer<'_>, cx: &Corpo, fc: &CtxFuncao, r: TypeId, exibido: TypeId) {
    let e_void_dyn = |inf: &BodyInferrer<'_>, x: TypeId| matches!(inf.table.get(x), Type::Void | Type::Dynamic);
    for &(e, s) in &fc.expressoes_retornadas {
        let erro = match fc.modificador {
            AsyncModifier::None => {
                if matches!(inf.table.get(s), Type::Void) {
                    !e_void_dyn(inf, r)
                } else {
                    !inf.atribuivel(s, r)
                }
            }
            AsyncModifier::Async => {
                let tv = inf.flatten(r);
                let fs = inf.flatten(s);
                if matches!(inf.table.get(tv), Type::Void) {
                    !matches!(inf.table.get(fs), Type::Void | Type::Dynamic | Type::Null)
                } else if matches!(inf.table.get(fs), Type::Void) {
                    !e_void_dyn(inf, tv)
                } else {
                    !inf.atribuivel(s, tv) && !inf.sub(fs, tv)
                }
            }
            _ => false,
        };
        if erro {
            let sp = inf.span_expr(cx.unit, e);
            let de = inf.table.format(s, inf.interner, inf.program);
            let para = inf.table.format(exibido, inf.interner, inf.program);
            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::RETURN_OF_INVALID_TYPE_FROM_CLOSURE, sp, &[&de, &para]);
        }
    }
}

/// `=> e`: o `verifyExpressionFunctionBody` do analyzer — retorno (depois do
/// `flatten` em `async`) `void` aceita qualquer expressão; o resto é a regra
/// de `return e;` ([`verificar_retorno`]).
fn verificar_retorno_de_expressao(inf: &mut BodyInferrer<'_>, cx: &Corpo, fc: &CtxFuncao, e: ExprId, t: TypeId) {
    let Some(r) = fc.retorno else { return };
    let achatado = if fc.modificador == AsyncModifier::None { r } else { inf.flatten(r) };
    if matches!(inf.table.get(achatado), Type::Void) {
        return;
    }
    verificar_retorno(inf, cx, fc, e, t);
}

/// `_checkReturnExpression` do `ReturnTypeVerifier`: `return e;` (ou `=> e`)
/// com `e` de tipo `S` num executável de retorno declarado `T`. Em geradores,
/// closures e construtores geradores nada sai daqui.
pub(crate) fn verificar_retorno(inf: &mut BodyInferrer<'_>, cx: &Corpo, fc: &CtxFuncao, e: ExprId, s: TypeId) {
    // `_checkReturnExpression`: com o retorno declarado ilegal, nada.
    if !fc.retorno_legal {
        return;
    }
    let (Some(t), Some(exe)) = (fc.retorno, fc.executavel.as_ref()) else { return };
    let e_void_dyn = |inf: &BodyInferrer<'_>, x: TypeId| matches!(inf.table.get(x), Type::Void | Type::Dynamic);
    let e_void_dyn_null = |inf: &BodyInferrer<'_>, x: TypeId| matches!(inf.table.get(x), Type::Void | Type::Dynamic | Type::Null);
    let erro = match fc.modificador {
        AsyncModifier::None => {
            if matches!(inf.table.get(t), Type::Void) && !e_void_dyn_null(inf, s) {
                true
            } else if matches!(inf.table.get(s), Type::Void) {
                !e_void_dyn(inf, t)
            } else if let Some(campo) = campo_de_registro_de_um(inf, cx, t, s, e)
                && inf.atribuivel(campo, s)
            {
                // `(int,) f() => (1);`: o parêntese queria ser um registro.
                let sp = inf.span_expr(cx.unit, e);
                inf.aviso_com_codigo(
                    dartforge_diagnostics::codigos::compile_time_error::RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA,
                    sp,
                    &[],
                );
                return;
            } else {
                !inf.atribuivel(s, t)
            }
        }
        AsyncModifier::Async => {
            let tv = inf.tipo_valor_futuro(t);
            let fs = inf.flatten(s);
            if matches!(inf.table.get(tv), Type::Void) && !e_void_dyn_null(inf, fs) {
                true
            } else if matches!(inf.table.get(fs), Type::Void) {
                !e_void_dyn(inf, tv)
            } else {
                !inf.atribuivel(s, tv) && !inf.sub(fs, tv)
            }
        }
        AsyncModifier::SyncStar | AsyncModifier::AsyncStar => false,
    };
    if !erro {
        return;
    }
    use super::corpo::EspecieExecutavel as E;
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let codigo = match exe.especie {
        E::Funcao => c::RETURN_OF_INVALID_TYPE_FROM_FUNCTION,
        E::Metodo => c::RETURN_OF_INVALID_TYPE_FROM_METHOD,
        E::Construtor => c::RETURN_OF_INVALID_TYPE_FROM_CONSTRUCTOR,
    };
    let sp = inf.span_expr(cx.unit, e);
    let de = inf.table.format(s, inf.interner, inf.program);
    let para = inf.table.format(t, inf.interner, inf.program);
    let nome = exe.nome.clone();
    inf.aviso_com_codigo(codigo, sp, &[&de, &para, &nome]);
}

/// `T` é um registro de um só campo posicional, `S` não é registro e `e` é
/// uma expressão entre parênteses: o tipo do campo.
fn campo_de_registro_de_um(inf: &BodyInferrer<'_>, cx: &Corpo, t: TypeId, s: TypeId, e: ExprId) -> Option<TypeId> {
    let Type::Record { positional, named, .. } = inf.table.get(t) else { return None };
    if positional.len() != 1 || !named.is_empty() || matches!(inf.table.get(s), Type::Record { .. }) {
        return None;
    }
    let campo = positional[0];
    matches!(inf.program.unit(cx.unit).ast.expr(e).kind, ast::ExprKind::Parenthesized(_)).then_some(campo)
}

impl<'a> BodyInferrer<'a> {
    /// **futureValueType**(`T`) de um retorno declarado: `Future<S>`,
    /// `FutureOr<S>` (anuláveis ou não) dão `S`; `void` e `dynamic` ficam;
    /// o resto é `Object?`.
    pub(crate) fn tipo_valor_futuro(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Void | Type::Dynamic => t,
            Type::FutureOr { arg, .. } => arg,
            Type::Interface { class, args, .. } if Some(class) == self.core.future_class && args.len() == 1 => args[0],
            _ => self.core.object_nullable,
        }
    }

    /// Espécie e nome de exibição de um executável declarado.
    pub(crate) fn executavel_declarado(&self, f: FunctionElementId) -> Option<super::corpo::Executavel> {
        use super::corpo::{EspecieExecutavel as E, Executavel};
        let fe = self.program.function(f);
        let nome = self.interner.resolve(fe.name);
        if matches!(fe.node, FunctionRef::Constructor { .. }) {
            let classe = self.interner.resolve(self.program.class(fe.class?).name).to_string();
            let nome = if nome.is_empty() || nome == "new" { classe } else { format!("{classe}.{nome}") };
            return Some(Executavel { especie: E::Construtor, nome });
        }
        let especie = match fe.kind {
            FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => E::Funcao,
            _ if fe.class.is_some() || fe.extension.is_some() => E::Metodo,
            _ => E::Funcao,
        };
        // O setter se exibe sem o `=` da chave; `operator ==` fica como está.
        let nome = if fe.kind == FunctionKind::Setter { nome.strip_suffix('=').unwrap_or(nome) } else { nome };
        Some(Executavel { especie, nome: nome.to_string() })
    }
}
