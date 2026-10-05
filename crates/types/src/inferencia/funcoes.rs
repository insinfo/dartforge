//! Corpos de funções declaradas, construtores (inicializadores), expressões
//! de função (com inferência do retorno, `inference.md` "Function literal
//! return type inference"), funções locais, variáveis de topo sem tipo e
//! metadados.

use super::corpo::{Corpo, CtxFuncao, Local};
use super::expr::{self, declarar_local, inferir, inferir_livre, RefNome};
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
            cx.funcoes.push(CtxFuncao { modificador: af.modifier, retorno: Some(ret), contexto_retorno: ctx_ret, retornados: Vec::new(), retorno_vazio: false, expressoes_retornadas: Vec::new(), executavel });
            corpo_de_funcao(inf, &mut cx, &af.body, af.modifier, ret, None);
            cx.funcoes.pop();
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
                inicializador(inf, &mut cx, fe.class, init, &ctor.parameters);
            }
            cx.tipo_this = this_salvo;
            cx.estatico = estatico_salvo;
            if let Some(red) = &ctor.redirect {
                alvo_de_factory_redirecionadora(inf, &mut cx, red, fe.class);
            }
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
            cx2.funcoes.push(CtxFuncao { modificador: AsyncModifier::None, retorno: Some(ret), contexto_retorno: ctx_ret, retornados: Vec::new(), retorno_vazio: false, expressoes_retornadas: Vec::new(), executavel });
            // `flowEnd(ConstructorDeclaration)`: o analyzer não apara o
            // construtor na última instrução; o trecho vai até o fim dele.
            let fim = inf.program.unit(unit).ast.member(member).span.end;
            corpo_de_funcao(inf, &mut cx2, &ctor.body, AsyncModifier::None, ret, Some(fim));
        }
        FunctionRef::None => {}
    }
}

/// Um inicializador de construtor.
fn inicializador(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, classe: Option<ClassId>, init: &ast::Initializer, params: &[ast::Parameter]) {
    let u = inf.core.unknown;
    match init {
        ast::Initializer::Field { name, value, .. } => {
            let campo = classe.and_then(|c| inf.program.class(c).fields.iter().copied().find(|&v| inf.program.variable(v).name == name.sym));
            let t = campo.map(|v| inf.tipo_variavel(v)).unwrap_or(u);
            let tv = inferir(inf, cx, *value, t);
            if campo.is_some() {
                expr::verificar_atribuivel_expr(inf, cx, *value, tv, t, INVALID_ASSIGNMENT.template);
            }
        }
        ast::Initializer::Super { span, constructor, arguments } => {
            let alvo = classe.and_then(|c| {
                let sup = inf.outline.classes[c.0 as usize].supertype?;
                let Type::Interface { class: sc, args, .. } = inf.table.get(sup).clone() else { return None };
                Some((sc, args))
            });
            chamar_construtor_de(inf, cx, alvo, *constructor, arguments, params, *span);
        }
        ast::Initializer::Redirect { span, constructor, arguments } => {
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

/// `factory E() = E.named;`: o alvo não pode ser um construtor gerador
/// de enum — só factories (ou valores) podem ser referenciados assim.
/// `: this(...)` é outro caminho ([`inicializador`]) e continua legal, e
/// nome indefinido (`= E.inexistente`) é de outro código
/// (`const_with_undefined_constructor`) e segue mudo.
///
/// O ponto do alvo vem dentro do tipo (`E.named` é um nome de duas
/// partes; `constructor` só aparece com argumentos de tipo, em
/// `= E<T>.nome`), então a resolução é sintática — sem inferir nada, sem
/// recursão.
fn alvo_de_factory_redirecionadora(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, red: &ast::RedirectTarget, dona: Option<ClassId>) {
    let (partes, escrito): (Vec<dartforge_intern::SymbolId>, Option<ast::Name>) = {
        let nodo = inf.program.unit(cx.unit).ast.ty(red.ty);
        let ast::TypeKind::Named { name, .. } = &nodo.kind else { return };
        (name.iter().map(|n| n.sym).collect(), red.constructor)
    };
    // (classe alvo, nome do construtor; `None` = sem nome)
    let Some((alvo, ctor)): Option<(ClassId, Option<dartforge_intern::SymbolId>)> = (|| {
        match partes.as_slice() {
            [a] => Some((resolver_classe_alvo(inf, cx, *a)?, escrito.map(|n| n.sym))),
            [a, b] if inf.program.prefixos_na_unidade(cx.unit).contains_key(a) => {
                let el = inf.program.lookup_prefixed_na_unidade(cx.unit, *a, *b)?.getter?;
                Some((classe_de_elemento(inf, el)?, escrito.map(|n| n.sym)))
            }
            [a, b] => {
                if escrito.is_some() {
                    return None;
                }
                Some((resolver_classe_alvo(inf, cx, *a)?, Some(*b)))
            }
            _ => None,
        }
    })() else { return };
    let vazio = inf.sym.vazio;
    let chave = match ctor {
        // `= E.new` é o sem nome escrito por extenso, como no tearoff.
        Some(s) if Some(s) == inf.sym.new_ => vazio,
        Some(s) => Some(s),
        None => vazio,
    };
    let Some(chave) = chave else { return };
    if inf.program.class(alvo).kind != ClassKind::Enum {
        // `_checkForAllRedirectConstructorErrorCodes` (EV:2025-2075):
        // `redirect_to_missing_constructor` sem o construtor no alvo e
        // `redirect_to_invalid_return_type` com o tipo do alvo fora do da
        // classe da factory (só entre classes sem parâmetros de tipo: com
        // eles o tipo do alvo é inferido contra o da classe, o que não se faz
        // aqui). O alvo pode ser o homônimo que o nome designa (T1, `c10`).
        match inf.construtor_de(alvo, chave) {
            // O construtor padrão implícito de uma classe sem construtor
            // declarado (a abstrata não ganha o sintético no outline; o
            // `= Abstrata` é outro código, `redirect_to_abstract_class_constructor`).
            None if Some(chave) == vazio && inf.program.class(alvo).constructors.is_empty() => {}
            None => {
                // `{0}`: o nome do tipo como escrito, com o do construtor.
                let tipo_escrito: Vec<&str> = match (partes.as_slice(), escrito) {
                    ([a, _b], None) => vec![inf.interner.resolve(*a)],
                    _ => partes.iter().map(|s| inf.interner.resolve(*s)).collect(),
                };
                let mut nome = tipo_escrito.join(".");
                if let Some(c) = ctor {
                    nome.push('.');
                    nome.push_str(inf.interner.resolve(c));
                }
                let tipo = inf.tipo_this_classe(alvo);
                let exibido = inf.table.format(tipo, inf.interner, inf.program);
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::REDIRECT_TO_MISSING_CONSTRUCTOR, red.span, &[&nome, &exibido]);
            }
            Some(_) => {
                if let Some(classe) = dona
                    && inf.program.class(alvo).type_params.is_empty()
                    && inf.program.class(classe).type_params.is_empty()
                {
                    let de = inf.tipo_this_classe(alvo);
                    let para = inf.tipo_this_classe(classe);
                    if !inf.sub(de, para) {
                        let (a, b) = (inf.table.format(de, inf.interner, inf.program), inf.table.format(para, inf.interner, inf.program));
                        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::REDIRECT_TO_INVALID_RETURN_TYPE, red.span, &[&a, &b]);
                    }
                }
            }
        }
        return;
    }
    match inf.construtor_de(alvo, chave) {
        Some(f) if inf.program.function(f).factory => {}
        Some(_) => {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), red.span);
        }
        // Sem nome implícito = gerador implícito (como em `E()`); nome
        // explícito sem alvo é outro diagnóstico.
        None if ctor.is_none() => {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), red.span);
        }
        None => {}
    }
}

/// Classe nomeada por um segmento de alvo de factory redirecionadora
/// (`E` em `= E` ou `= E.nomeado`).
fn resolver_classe_alvo(inf: &mut BodyInferrer<'_>, cx: &Corpo, nome: dartforge_intern::SymbolId) -> Option<ClassId> {
    let RefNome::Elemento(el) = expr::resolver_nome(inf, cx, nome, false) else { return None };
    classe_de_elemento(inf, el)
}

/// Classe por trás de um elemento de alvo (classe ou typedef de classe).
fn classe_de_elemento(inf: &mut BodyInferrer<'_>, el: Element) -> Option<ClassId> {
    match el {
        Element::Class(c) => Some(c),
        Element::Typedef(td) => match inf.table.get(inf.outline.typedefs[td.0 as usize].target_type).clone() {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(class),
            _ => None,
        },
        _ => None,
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
pub(crate) fn expressao_de_funcao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, fid: ast::FunctionId, ctx: TypeId) -> TypeId {
    // `wasFunctionTypeSupplied` (`function_expression_resolver.dart:35`): o
    // contexto, como chegou, é um tipo de função.
    if matches!(inf.table.get(ctx), Type::Function { .. }) {
        inf.body_types.units[cx.unit.0 as usize].com_tipo_de_funcao.insert(fid);
    }
    let (t, _) = funcao_literal(inf, cx, fid, ctx, None);
    t
}

/// Infere uma função literal ou local. `local` é o id da função local
/// (declarada antes, para chamadas recursivas).
fn funcao_literal(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, fid: ast::FunctionId, ctx: TypeId, local: Option<crate::resolved::LocalId>) -> (TypeId, ()) {
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
    cx.funcoes.push(CtxFuncao { modificador: m, retorno: declarado, contexto_retorno: ctx_ret, retornados: Vec::new(), retorno_vazio: false, expressoes_retornadas: Vec::new(), executavel });
    let saltos_salvos = std::mem::take(&mut cx.saltos);
    let cascatas_salvas = std::mem::take(&mut cx.cascatas);
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
            (None, cx.fluxo.alcancavel)
        }
        _ => (None, false),
    };
    cx.saltos = saltos_salvos;
    cx.cascatas = cascatas_salvas;
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
    let (t, _) = funcao_literal(inf, cx, fid, u, Some(id));
    cx.locais[id.0 as usize].tipo = t;
    cx.locais[id.0 as usize].funcao_local = false;
    inf.body_types.units[cx.unit.0 as usize].set_tipo_local(nome.span.start, t);
}

/// Metadados (anotações) e argumentos de constantes de enum de uma unidade.
pub(crate) fn inferir_metadados_da_unidade(inf: &mut BodyInferrer<'_>, unit: UnitId) {
    let a = &inf.program.unit(unit).ast;
    for d in a.decls.iter() {
        let (classe, extensao) = classe_da_decl(inf, unit, d);
        for m in d.metadata.iter() {
            anotacao(inf, unit, classe, extensao, m);
        }
        match &d.kind {
            ast::DeclKind::Enum(en) => {
                let Some(c) = classe else { continue };
                for k in en.constants.iter() {
                    for m in k.metadata.iter() {
                        anotacao(inf, unit, Some(c), None, m);
                    }
                    if let Some(args) = &k.arguments {
                        let mut cx = Corpo::novo(inf, unit, Some(c), None, true);
                        let chave = k.constructor.map(|n| n.sym).or(inf.sym.vazio);
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
                        } else if k.constructor.is_none() && inf.program.class(c).constructors.is_empty() {
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
    let declaradas: std::collections::HashSet<u32> = inf
        .program
        .functions
        .iter()
        .filter_map(|fe| match fe.node {
            dartforge_elements::model::FunctionRef::Function { unit: u, function } if u == unit => Some(function.0),
            _ => None,
        })
        .collect();
    for (i, f) in a.functions.iter().enumerate() {
        let validar = declaradas.contains(&(i as u32));
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
    if let Some(Element::Class(c)) = c {
        let chave = ctor.or(inf.sym.vazio);
        if let Some(f) = chave.and_then(|k| inf.program.class(c).constructors.get(&k).copied()) {
            let explicitos = if m.type_args.is_empty() {
                None
            } else {
                Some(m.type_args.iter().map(|&t| inf.tipo_de_argumento_de_tipo(&cx, t)).collect())
            };
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
