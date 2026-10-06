//! Invocações: funções, métodos, construtores, `call`, e a inferência de
//! argumentos de tipo com inferência "horizontal" (`inference-update-1`):
//! literais de função cujos parâmetros dependem de variáveis ainda não
//! resolvidas são inferidos depois dos demais argumentos.

use super::corpo::Corpo;
use super::expr::{self, inferir, inferir_livre, receptor, referencia_a_tipo, registrar, resolver, resolver_nome, RefNome, RefTipo};
use super::membros::Busca;
use super::BodyInferrer;
use crate::codes::*;
use crate::constraints::{instanciar_funcao, GenericInferrer};
use crate::resolved::Resolved;
use crate::table::{Type, TypeId, TypeParamId};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use std::collections::HashMap;

/// Parâmetro formal correspondente a cada argumento.
fn parametros_dos_argumentos(
    inf: &BodyInferrer<'_>,
    positional: &[TypeId],
    optional: &[TypeId],
    named: &[(dartforge_intern::SymbolId, TypeId, bool)],
    args: &ast::Arguments,
) -> Vec<Option<TypeId>> {
    let _ = inf;
    let mut i = 0usize;
    args.args
        .iter()
        .map(|a| match &a.name {
            None => {
                let t = if i < positional.len() {
                    Some(positional[i])
                } else {
                    optional.get(i - positional.len()).copied()
                };
                i += 1;
                t
            }
            Some(n) => named.iter().find(|(s, _, _)| *s == n.sym).map(|(_, t, _)| *t),
        })
        .collect()
}

/// O que a verificação de aridade precisa saber da chamada (o `nameNode` e
/// o `errorEntity` do analyzer), definido por quem conhece a forma dela.
#[derive(Debug, Clone, Default)]
pub(crate) struct AlvoDaAridade {
    /// Nome citado em `NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_*`
    /// (`_reportNotEnoughPositionalArguments`,
    /// an611:src/generated/resolver.dart:4370-4420); `None` usa as
    /// variantes sem nome.
    pub nome: Option<String>,
    /// Intervalo do `MISSING_REQUIRED_ARGUMENT`
    /// (an611:src/error/required_parameters_verifier.dart:21-110): o nome do
    /// método, o nome do construtor, a chamada inteira ou a lista de
    /// argumentos, conforme a forma.
    pub entidade: Option<Span>,
    /// Parâmetros `super.x` posicionais do construtor corrente (argumentos
    /// implícitos de um `super(...)`, `verifySuperFormalParameters`,
    /// an611:src/error/super_formal_parameters_verifier.dart:11-37).
    pub super_posicionais: usize,
    /// Nomes dos parâmetros `super.x` nomeados do construtor corrente.
    pub super_nomeados: Vec<dartforge_intern::SymbolId>,
}

/// Constante de enum sem argumentos (`v;`) com construtor sem nome que
/// exige argumentos: `NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_*` no nome da
/// constante (an611:src/generated/resolver.dart:2531-2544) e
/// `MISSING_REQUIRED_ARGUMENT` também nele
/// (an611:src/error/required_parameters_verifier.dart:37-43).
pub(crate) fn aridade_sem_argumentos(inf: &mut BodyInferrer<'_>, f: FunctionElementId, nome_enum: String, span: Span) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let sig = inf.outline.functions[f.0 as usize].signature;
    let Type::Function { positional, named, .. } = inf.table.get(sig).clone() else { return };
    let n = positional.len();
    if n == 1 {
        inf.aviso_com_codigo(ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_SINGULAR, span, &[&nome_enum]);
    } else if n > 1 {
        inf.aviso_com_codigo(ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_PLURAL, span, &[&n.to_string(), "0", &nome_enum]);
    }
    for (s, _, req) in named.iter() {
        if *req {
            let nome = inf.interner.resolve(*s).to_string();
            inf.aviso_com_codigo(ce::MISSING_REQUIRED_ARGUMENT, span, &[&nome]);
        }
    }
}

/// Define o alvo da próxima verificação de aridade.
pub(crate) fn definir_alvo(inf: &mut BodyInferrer<'_>, nome: Option<String>, entidade: Span) {
    inf.alvo_da_aridade = Some(AlvoDaAridade { nome, entidade: Some(entidade), ..Default::default() });
}

/// O próximo token da fonte a partir de `pos` (pula espaços e comentários):
/// um identificador inteiro ou um caractere.
fn proximo_token(fonte: &str, mut pos: usize) -> Span {
    let b = fonte.as_bytes();
    loop {
        while pos < b.len() && (b[pos] as char).is_ascii_whitespace() {
            pos += 1;
        }
        if b.get(pos) == Some(&b'/') && b.get(pos + 1) == Some(&b'/') {
            while pos < b.len() && b[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if b.get(pos) == Some(&b'/') && b.get(pos + 1) == Some(&b'*') {
            match fonte[pos + 2..].find("*/") {
                Some(i) => pos = pos + 2 + i + 2,
                None => pos = b.len(),
            }
            continue;
        }
        break;
    }
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut fim = pos;
    if b.get(pos).is_some_and(|&c| ident(c)) {
        while fim < b.len() && ident(b[fim]) {
            fim += 1;
        }
    } else if pos < b.len() {
        fim = pos + fonte[pos..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    Span { start: pos, end: fim }
}

/// Diagnósticos de aridade e nomes (`resolveArgumentsToParameters`,
/// an611:src/generated/resolver.dart:4229-4355, e
/// `RequiredParametersVerifier`): nomeado indefinido e repetido no nome do
/// argumento; poucos posicionais no token depois do último posicional (ou
/// depois do `(`); posicionais demais no primeiro excedente, com a variante
/// `_COULD_BE_NAMED` quando sobram nomeados não usados; nomeado obrigatório
/// ausente na entidade da chamada.
fn verificar_aridade(
    inf: &mut BodyInferrer<'_>,
    cx: &Corpo,
    positional: &[TypeId],
    optional: &[TypeId],
    named: &[(dartforge_intern::SymbolId, TypeId, bool)],
    args: &ast::Arguments,
    alvo: Option<AlvoDaAridade>,
) {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let alvo = alvo.unwrap_or_default();
    let a = &inf.program.unit(cx.unit).ast;
    let fonte = &inf.program.unit(cx.unit).source;
    let sem_nome = positional.len() + optional.len();
    let mut npos = 0usize;
    let mut sem_branco = true;
    let mut primeiro_excedente: Option<Span> = None;
    let mut ultimo_posicional: Option<Span> = None;
    for x in args.args.iter().filter(|x| x.name.is_none()) {
        let sp = a.expr(x.value).span;
        if sp.start >= sp.end {
            sem_branco = false;
        }
        if npos >= sem_nome && primeiro_excedente.is_none() {
            primeiro_excedente = Some(sp);
        }
        npos += 1;
        ultimo_posicional = Some(sp);
    }
    // Token do "poucos posicionais": depois do último posicional, ou o
    // primeiro depois do `(`.
    let token_poucos = match ultimo_posicional {
        Some(sp) => proximo_token(fonte, sp.end),
        None => {
            let ini = args.type_args.last().map(|&t| a.ty(t).span.end).unwrap_or(args.span.start);
            let abre = fonte.get(ini..).and_then(|r| r.find('(')).map(|i| ini + i + 1).unwrap_or(args.span.start);
            proximo_token(fonte, abre)
        }
    };
    // O `)` sintético do scanner (comprimento zero no fim dos argumentos):
    // quando o token seguinte é ele, o erro sai ali com comprimento zero.
    let fecho_sintetico = args.span.end > args.span.start && fonte.as_bytes().get(args.span.end - 1) != Some(&b')');
    let token_poucos = if fecho_sintetico && token_poucos.start >= args.span.end {
        Span { start: args.span.end, end: args.span.end }
    } else {
        token_poucos
    };
    let total_pos = npos + alvo.super_posicionais;
    let mut usados: Vec<dartforge_intern::SymbolId> = alvo.super_nomeados.clone();
    let mut avisos: Vec<(dartforge_diagnostics::Codigo, Span, Vec<String>)> = Vec::new();
    for x in args.args.iter() {
        if let Some(n) = &x.name {
            let texto = inf.interner.resolve(n.sym).to_string();
            if !named.iter().any(|(s, _, _)| *s == n.sym) {
                avisos.push((ce::UNDEFINED_NAMED_PARAMETER, n.span, vec![texto.clone()]));
            }
            if usados.contains(&n.sym) {
                avisos.push((ce::DUPLICATE_NAMED_ARGUMENT, n.span, vec![texto]));
            } else {
                usados.push(n.sym);
            }
        }
    }
    if total_pos < positional.len() && sem_branco {
        let requeridos = positional.len();
        let plural = requeridos > 1;
        let mut argumentos: Vec<String> = Vec::new();
        if plural {
            argumentos.push(requeridos.to_string());
            argumentos.push(total_pos.to_string());
        }
        let codigo = match (&alvo.nome, plural) {
            (None, true) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_PLURAL,
            (None, false) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_SINGULAR,
            (Some(_), true) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_PLURAL,
            (Some(_), false) => ce::NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_SINGULAR,
        };
        if let Some(n) = &alvo.nome {
            argumentos.push(n.clone());
        }
        avisos.push((codigo, token_poucos, argumentos));
    } else if total_pos > sem_nome && sem_branco {
        let codigo = if named.len() > usados.len() {
            ce::EXTRA_POSITIONAL_ARGUMENTS_COULD_BE_NAMED
        } else {
            ce::EXTRA_POSITIONAL_ARGUMENTS
        };
        if let Some(sp) = primeiro_excedente {
            avisos.push((codigo, sp, vec![sem_nome.to_string(), total_pos.to_string()]));
        }
    }
    let entidade = alvo.entidade.unwrap_or(args.span);
    for (s, _, req) in named.iter() {
        if *req && !args.args.iter().any(|x| x.name.map(|n| n.sym) == Some(*s)) && !alvo.super_nomeados.contains(s) {
            avisos.push((ce::MISSING_REQUIRED_ARGUMENT, entidade, vec![inf.interner.resolve(*s).to_string()]));
        }
    }
    for (codigo, sp, argumentos) in avisos {
        let refs: Vec<&str> = argumentos.iter().map(|s| s.as_str()).collect();
        inf.aviso_com_codigo(codigo, sp, &refs);
    }
}

/// O argumento é um literal de função com algum parâmetro sem tipo (candidato
/// a ser adiado na inferência horizontal).
fn literal_de_funcao_adiavel(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> bool {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::FunctionExpression(f) => {
            let f = a.function(*f);
            f.parameters.as_ref().is_some_and(|ps| ps.iter().any(|p| p.ty.is_none() && p.function_parameters.is_none()))
        }
        ExprKind::Parenthesized(i) => literal_de_funcao_adiavel(inf, cx, *i),
        _ => false,
    }
}

fn menciona(inf: &BodyInferrer<'_>, t: TypeId, params: &[TypeParamId]) -> bool {
    match inf.table.get(t) {
        Type::TypeParameter { param, .. } => params.contains(param),
        Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.iter().any(|a| menciona(inf, *a, params)),
        Type::FutureOr { arg, .. } => menciona(inf, *arg, params),
        Type::Function { ret, positional, optional, named, .. } => {
            menciona(inf, *ret, params)
                || positional.iter().chain(optional.iter()).any(|a| menciona(inf, *a, params))
                || named.iter().any(|(_, a, _)| menciona(inf, *a, params))
        }
        Type::Record { positional, named, .. } => {
            positional.iter().any(|a| menciona(inf, *a, params)) || named.iter().any(|(_, a)| menciona(inf, *a, params))
        }
        _ => false,
    }
}

/// Ordem da inferência horizontal (`inference-update-1`): os argumentos não
/// adiados formam o estágio 0; os literais de função adiados seguem as
/// dependências entre si — um literal cujos tipos de parâmetro mencionam uma
/// variável que o retorno de outro literal adiado ainda fornece espera por
/// ele. Os que só dependem de variáveis já fornecidas formam o estágio
/// seguinte (na ordem do texto); num ciclo, os restantes vão juntos.
/// Devolve `(estágio, índice do argumento)` na ordem de inferência.
fn estagios_horizontais(
    inf: &BodyInferrer<'_>,
    type_params: &[TypeParamId],
    params: &[Option<TypeId>],
    adiados: &[bool],
) -> Vec<(usize, usize)> {
    let mut ordem: Vec<(usize, usize)> = (0..adiados.len()).filter(|&i| !adiados[i]).map(|i| (0, i)).collect();
    // Para cada adiado: variáveis de que depende (nos tipos dos parâmetros
    // do formal) e as que fornece (no retorno do formal).
    let mut restantes: Vec<(usize, Vec<TypeParamId>, Vec<TypeParamId>)> = Vec::new();
    for i in (0..adiados.len()).filter(|&i| adiados[i]) {
        let Some(p) = params[i] else { continue };
        let (entradas, saida): (Vec<TypeId>, Option<TypeId>) = match inf.table.get(p) {
            Type::Function { ret, positional, optional, named, .. } => (
                positional.iter().chain(optional.iter()).copied().chain(named.iter().map(|(_, t, _)| *t)).collect(),
                Some(*ret),
            ),
            _ => (vec![p], None),
        };
        let depende: Vec<TypeParamId> =
            type_params.iter().copied().filter(|&v| entradas.iter().any(|&t| menciona(inf, t, &[v]))).collect();
        let fornece: Vec<TypeParamId> =
            type_params.iter().copied().filter(|&v| saida.is_some_and(|t| menciona(inf, t, &[v]))).collect();
        restantes.push((i, depende, fornece));
    }
    let mut estagio = 1;
    while !restantes.is_empty() {
        let prontos: Vec<usize> = (0..restantes.len())
            .filter(|&k| {
                restantes[k].1.iter().all(|v| {
                    !restantes.iter().enumerate().any(|(j, (_, _, fornece))| j != k && fornece.contains(v))
                })
            })
            .collect();
        let prontos = if prontos.is_empty() { (0..restantes.len()).collect() } else { prontos };
        for &k in &prontos {
            ordem.push((estagio, restantes[k].0));
        }
        let mut k = 0;
        restantes.retain(|_| {
            let fica = !prontos.contains(&k);
            k += 1;
            fica
        });
        estagio += 1;
    }
    ordem
}

/// Invoca um tipo de função com os argumentos; devolve `(retorno, função instanciada)`.
pub(crate) fn invocar(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    f: TypeId,
    args: &ast::Arguments,
    ctx: TypeId,
    explicitos: Option<Vec<TypeId>>,
) -> (TypeId, TypeId) {
    let alvo = inf.alvo_da_aridade.take();
    let Type::Function { type_params, ret, positional, optional, named, .. } = inf.table.get(f).clone() else {
        inf.entidade_da_inferencia = None;
        inf.nomes_posicionais = None;
        for a in args.args.iter() {
            inferir_livre(inf, cx, a.value);
        }
        return (inf.core.dynamic_, f);
    };
    let u = inf.core.unknown;
    // Chamada genérica cujos parâmetros de tipo estão em escopo (a função
    // chamando a si mesma, `_mergeSort(elements, keyOf, …)` dentro de
    // `_mergeSort<E, K>`): os argumentos mencionam os mesmos parâmetros que
    // a inferência resolve; renomeia para parâmetros novos, como a
    // instanciação da especificação (R-GEN-04: variáveis frescas).
    if !type_params.is_empty()
        && type_params.iter().any(|&p| {
            let n = inf.table.param(p).name;
            matches!(cx.buscar(n), Some(super::corpo::Nome::TipoParam(q)) if q == p)
        })
    {
        let novos = inf.parametros_novos(&type_params);
        let tipos: Vec<TypeId> = novos.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
        let mapa = inf.mapa(&type_params, &tipos);
        let ret = inf.subst(ret, &mapa);
        let positional: Box<[TypeId]> = positional.iter().map(|&t| inf.subst(t, &mapa)).collect();
        let optional: Box<[TypeId]> = optional.iter().map(|&t| inf.subst(t, &mapa)).collect();
        let named: Box<[_]> = named.iter().map(|&(n, t, r)| (n, inf.subst(t, &mapa), r)).collect();
        let f2 = inf.table.intern(Type::Function { type_params: novos.into_boxed_slice(), ret, positional, optional, named, nullable: false });
        inf.alvo_da_aridade = alvo;
        return invocar(inf, cx, f2, args, ctx, explicitos);
    }
    // O `errorEntity` vale só para esta invocação (os argumentos têm as suas).
    let entidade = inf.entidade_da_inferencia.take();
    let nomes_posicionais = inf.nomes_posicionais.take();
    // `FullInvocationInferrer.resolveInvocation`
    // (`invocation_inferrer.dart:175-184` do analyzer 3.6.2): a lista de
    // argumentos de tipo com contagem diferente da dos parâmetros de tipo
    // do tipo invocado é `WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD`, na lista
    // `<…>`, com o tipo de função cru como primeiro argumento; a inferência
    // segue sem os explícitos. Sem a espécie e o nome do alvo (que só quem
    // chama conhece), a variante 3.13 do código sai na forma "tipo desta
    // função" (docs/ANALYZER-ESPECIFICACAO.md, T2 §8, caso c27).
    if let Some(ex) = &explicitos
        && ex.len() != type_params.len()
        && let (Some(&primeiro), Some(&ultimo)) = (args.type_args.first(), args.type_args.last())
    {
        let programa = inf.program;
        let unidade = programa.unit(cx.unit);
        let (de, ate) = (unidade.ast.ty(primeiro).span.start, unidade.ast.ty(ultimo).span.end);
        let fonte = unidade.source.as_str();
        let inicio = fonte.get(..de).and_then(|t| t.rfind('<')).unwrap_or(de);
        let fim = fonte.get(ate..).and_then(|t| t.find('>')).map_or(ate, |i| ate + i + 1);
        let tipo = inf.table.format(f, inf.interner, inf.program);
        inf.aviso_com_codigo(
            dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD,
            Span { start: inicio, end: fim },
            &[&tipo, &type_params.len().to_string(), &ex.len().to_string()],
        );
    }
    // Argumentos de tipo explícitos: instancia e segue como não genérica.
    if !type_params.is_empty() {
        if let Some(ex) = explicitos {
            if ex.len() == type_params.len() {
                let mut env = inf.env();
                let inst = instanciar_funcao(f, &ex, &mut env);
                drop(env);
                inf.body_types.units[cx.unit.0 as usize].set_instanciacao(args.span.start, ex.into_boxed_slice());
                inf.alvo_da_aridade = alvo;
                return invocar(inf, cx, inst, args, ctx, None);
            }
        }
    }
    verificar_aridade(inf, cx, &positional, &optional, &named, args, alvo);
    let params = parametros_dos_argumentos(inf, &positional, &optional, &named, args);
    let contexto_numerico = inf.contexto_numerico_pendente.take();
    if type_params.is_empty() {
        for (a, p) in args.args.iter().zip(params.iter()) {
            // `x.clamp(a, b)` / `x.remainder(a)`: o contexto refinado do analyzer.
            let c = match (contexto_numerico, a.name) {
                (Some(c), None) => c,
                _ => p.unwrap_or(u),
            };
            let t = inferir(inf, cx, a.value, c);
            if let Some(p) = p {
                inf.body_types.units[cx.unit.0 as usize].tipos_de_parametros.insert(a.value, *p);
                expr::verificar_atribuivel_expr(inf, cx, a.value, t, *p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                closure_com_conjunto(inf, cx, a.value, *p);
            }
        }
        return (ret, f);
    }
    // Genérica: para baixo (retorno × contexto), depois argumentos em estágios.
    let mut gi = GenericInferrer::new(&type_params);
    if !inf.e_desconhecido(ctx) {
        gi.com_origem(crate::constraints::Origem::Retorno { declarado: ret, contexto: ctx });
        let mut env = inf.env();
        gi.constrain_return(ret, ctx, &mut env);
    }
    let mut env = inf.env();
    let mut prelim = gi.choose_preliminary(&mut env);
    drop(env);
    let adiados: Vec<bool> = args
        .args
        .iter()
        .zip(params.iter())
        .map(|(a, p)| p.is_some_and(|p| menciona(inf, p, &type_params)) && literal_de_funcao_adiavel(inf, cx, a.value))
        .collect();
    let mut tipos: Vec<TypeId> = vec![inf.core.dynamic_; args.args.len()];
    for (estagio, i) in estagios_horizontais(inf, &type_params, &params, &adiados) {
        let a = &args.args[i];
        {
            if estagio > 0 {
                let mut env = inf.env();
                prelim = gi.choose_preliminary(&mut env);
            }
            let contexto = match params[i] {
                Some(p) => {
                    let mapa: HashMap<TypeParamId, TypeId> = type_params.iter().copied().zip(prelim.iter().copied()).collect();
                    inf.subst(p, &mapa)
                }
                None => u,
            };
            let t = inferir(inf, cx, a.value, contexto);
            tipos[i] = t;
            if let Some(p) = params[i] {
                let t = inf.tipo_do_call_implicito(t, p).unwrap_or(t);
                let parametro = match a.name {
                    Some(n) => inf.interner.resolve(n.sym).to_string(),
                    None => {
                        let pos = args.args[..i].iter().filter(|x| x.name.is_none()).count();
                        nomes_posicionais.as_ref().and_then(|v| v.get(pos).cloned()).unwrap_or_default()
                    }
                };
                gi.com_origem(crate::constraints::Origem::Argumento { parametro, declarado: p, argumento: t, prefixo: None });
                let mut env = inf.env();
                gi.constrain_argument(t, p, &mut env);
            }
        }
    }
    let usar_limites = inf.program.library(cx.lib).features.tem(dartforge_frontend::Feature::InferenceUsingBounds);
    let (interner, program) = (inf.interner, inf.program);
    let mut env = inf.env();
    if usar_limites {
        gi.restringir_pelos_limites(&mut env);
    }
    let finais = gi.choose_final(&mut env);
    let falhas = match entidade {
        Some(_) => gi.falhas(&finais, &mut env, interner, program),
        None => Vec::new(),
    };
    let inst = instanciar_funcao(f, &finais, &mut env);
    drop(env);
    if let Some(sp) = entidade {
        for (nome, sufixo) in falhas {
            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::COULD_NOT_INFER, sp, &[&nome, &sufixo]);
        }
    }
    inf.body_types.units[cx.unit.0 as usize].set_instanciacao(args.span.start, finais.clone().into_boxed_slice());
    // Checagem com os parâmetros instanciados.
    if let Type::Function { positional: ip, optional: io, named: inm, ret: iret, .. } = inf.table.get(inst).clone() {
        let ips = parametros_dos_argumentos(inf, &ip, &io, &inm, args);
        for (i, a) in args.args.iter().enumerate() {
            if let Some(p) = ips[i] {
                inf.body_types.units[cx.unit.0 as usize].tipos_de_parametros.insert(a.value, p);
                expr::verificar_atribuivel_expr(inf, cx, a.value, tipos[i], p, ARGUMENT_TYPE_NOT_ASSIGNABLE.template);
                closure_com_conjunto(inf, cx, a.value, p);
            }
        }
        return (iret, inst);
    }
    (inf.core.dynamic_, inst)
}

/// O argumento é uma closure (o próprio `FunctionExpression`, com o
/// `staticParameterElement`) e o parâmetro é um tipo de função: o
/// `UNNECESSARY_SET_LITERAL` pelo retorno dele.
fn closure_com_conjunto(inf: &mut BodyInferrer<'_>, cx: &Corpo, argumento: ExprId, parametro: TypeId) {
    let ExprKind::FunctionExpression(fid) = inf.program.unit(cx.unit).ast.expr(argumento).kind else { return };
    if let Type::Function { ret, .. } = inf.table.get(parametro).clone() {
        super::funcoes::conjunto_desnecessario(inf, cx.unit, fid, ret);
    }
}

/// Invoca um valor de tipo `t` (função, objeto com `call`, `dynamic`).
fn invocar_valor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, t: TypeId, args: &ast::Arguments, ctx: TypeId, explicitos: Option<Vec<TypeId>>, span: Span) -> (TypeId, TypeId) {
    // O alvo da aridade só vale se a invocação de fato acontecer.
    let alvo = inf.alvo_da_aridade.take();
    if let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
        let alvo = *target;
        if !matches!(inf.program.unit(cx.unit).ast.expr(alvo).kind, ExprKind::Property { .. }) && expr::receptor_nunca(inf, cx, alvo, t) {
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
            return (inf.core.never, t);
        }
    }
    let t_nn = inf.nao_nulo(t);
    // `x(3)` com `x` de tipo `void`: `use_of_void_result` na função.
    if matches!(inf.table.get(t_nn), Type::Void) {
        if let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
            let alvo = *target;
            expr::uso_de_void(inf, cx, alvo, t_nn);
        }
    }
    match inf.table.get(t_nn).clone() {
        Type::Function { .. } => {
            // `FunctionExpressionInvocationResolver.resolve`
            // (`function_expression_invocation_resolver.dart:52-58`): a função
            // de tipo de função anulável, no nó da função.
            if let ExprKind::Call { target, .. } = &inf.program.unit(cx.unit).ast.expr(e).kind {
                let funcao = *target;
                expr::desreferencia_anulavel(inf, cx, funcao, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_INVOCATION_OF_NULLABLE_VALUE);
            }
            inf.alvo_da_aridade = alvo;
            invocar(inf, cx, t_nn, args, ctx, explicitos)
        }
        Type::Dynamic => {
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
            (inf.core.dynamic_, t)
        }
        Type::Never => {
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
            (inf.core.never, t)
        }
        Type::Interface { class, .. } if Some(class) == inf.core.function_class => {
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
            (inf.core.dynamic_, t)
        }
        _ => {
            if let Some(call) = inf.sym.call {
                if let Some(m) = inf.membro_de_interface(t_nn, call, false) {
                    inf.alvo_da_aridade = alvo;
                    return invocar(inf, cx, m.tipo, args, ctx, explicitos);
                }
                let busca = inf.buscar_membro(cx.lib, t_nn, call, false);
                if inf.ambiguidade_de_extensao.is_some() {
                    // `call` ambíguo: o analyzer relata a ambiguidade no alvo
                    // (`function_expression_invocation_resolver.dart`) e não
                    // resolve.
                    let sp = match &inf.program.unit(cx.unit).ast.expr(e).kind {
                        ExprKind::Call { target, .. } => inf.span_expr(cx.unit, *target),
                        _ => span,
                    };
                    inf.relatar_ambiguidade_de_extensao(sp);
                    for a in args.args.iter() {
                        inferir_livre(inf, cx, a.value);
                    }
                    return (inf.core.dynamic_, t);
                }
                if let Busca::Achado(m) = busca {
                    // `valor(args)` com o `call` de uma extensão
                    // (`calloc<Int32>(4)`, o `AllocatorAlloc.call`): a
                    // chamada registra o membro, que o lowering invoca com
                    // o valor como receptor.
                    if m.de_extensao {
                        super::expr::resolver(inf, cx, e, m.resolved.clone());
                    }
                    inf.alvo_da_aridade = alvo;
                    return invocar(inf, cx, m.tipo, args, ctx, explicitos);
                }
            }
            for a in args.args.iter() {
                inferir_livre(inf, cx, a.value);
            }
            // `o()`, `x.campo()`, `3(5)`: o analyzer relata
            // `invocation_of_non_function_expression` no alvo da chamada. Um
            // literal de tipo (`T<Null>()` com `T` alias de `dynamic`) é outro
            // código (`invocation_of_non_function`), fora daqui.
            // Ficam de fora (outros códigos ou nada): receptor anulável
            // (`unchecked_…`), `void` (`use_of_void_result`), parâmetro de
            // tipo (a chamada vai pelo limite) e nome solto que não é local
            // (`foo()` com só `set foo`: `undefined_method`).
            let _ = span;
            let a = &inf.program.unit(cx.unit).ast;
            let alvo = match &a.expr(e).kind {
                ExprKind::Call { target, .. } => Some(*target),
                _ => None,
            };
            let nome_nao_local = alvo.is_some_and(|x| match &a.expr(x).kind {
                ExprKind::Identifier(n) => !matches!(cx.buscar(n.sym), Some(super::corpo::Nome::Local(_))),
                _ => false,
            });
            let fora = t_nn == inf.core.type_
                || t != t_nn
                || nome_nao_local
                || matches!(inf.table.get(t_nn), Type::Void | Type::TypeParameter { .. } | Type::Intersection { .. });
            if let (false, Some(alvo)) = (fora, alvo) {
                let sp = inf.span_expr(cx.unit, alvo);
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::INVOCATION_OF_NON_FUNCTION_EXPRESSION, sp, &[]);
            }
            (inf.core.dynamic_, t)
        }
    }
}

fn argumentos_de_tipo(inf: &mut BodyInferrer<'_>, cx: &Corpo, args: &ast::Arguments) -> Option<Vec<TypeId>> {
    if args.type_args.is_empty() {
        return None;
    }
    Some(args.type_args.iter().map(|&t| inf.tipo_de_argumento_de_tipo(cx, t)).collect())
}

/// Prepara o `errorEntity` do `COULD_NOT_INFER` da próxima invocação
/// genérica e os nomes dos parâmetros posicionais do alvo `f`.
pub(crate) fn preparar_entidade(inf: &mut BodyInferrer<'_>, sp: Span, f: Option<FunctionElementId>) {
    inf.entidade_da_inferencia = Some(sp);
    inf.nomes_posicionais = f.map(|f| {
        inf.outline.functions[f.0 as usize]
            .parameters
            .iter()
            .filter(|p| p.kind != ast::ParameterKind::Named)
            .map(|p| p.name.map(|n| inf.interner.resolve(n).to_string()).unwrap_or_default())
            .collect()
    });
}

/// A função (de topo, membro, de extensão) a que `e` resolveu.
fn funcao_resolvida(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> Option<FunctionElementId> {
    match inf.body_types.units[cx.unit.0 as usize].get_resolved(e)? {
        Resolved::Element(Element::Function(f)) => Some(*f),
        Resolved::Member { member: crate::resolved::MemberRef::Function(f), .. } => Some(*f),
        Resolved::ExtensionMember { member, .. } => Some(*member),
        _ => None,
    }
}

/// `f(args)`, `r.m(args)`, `C(args)`, `C.nome(args)`...
/// `ExtensionMemberResolver.resolveOverride`
/// (`an611:src/dart/resolver/extension_member_resolver.dart:177-187`): o
/// `E(x)` fora de um acesso — operando esquerdo de binária, função de uma
/// invocação, alvo de índice, de método ou de propriedade, operando de
/// prefixo — e fora de alvo de cascata: `EXTENSION_OVERRIDE_WITHOUT_ACCESS`
/// no override inteiro.
fn override_sem_acesso(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) {
    let valido = match inf.pai_de(cx.unit, e) {
        dartforge_frontend::pais::Pai::Expr(p) => match &inf.program.unit(cx.unit).ast.expr(p).kind {
            ExprKind::Binary { left, .. } => *left == e,
            ExprKind::Call { target, .. } | ExprKind::Index { target, .. } | ExprKind::Property { target, .. } | ExprKind::Cascade { target, .. } => *target == e,
            ExprKind::Unary { op, operand } => {
                *operand == e && matches!(op, ast::UnaryOp::Neg | ast::UnaryOp::Not | ast::UnaryOp::BitNot | ast::UnaryOp::PrefixInc | ast::UnaryOp::PrefixDec)
            }
            _ => false,
        },
        _ => false,
    };
    if !valido {
        let sp = inf.span_expr(cx.unit, e);
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::EXTENSION_OVERRIDE_WITHOUT_ACCESS, sp, &[]);
    }
}

/// A lista `<…>` escrita de argumentos de tipo: do `<` antes do primeiro
/// ao `>` depois do último.
pub(crate) fn faixa_da_lista_de_tipos(inf: &BodyInferrer<'_>, unit: dartforge_elements::model::UnitId, tipos: &[ast::TypeId]) -> Option<Span> {
    let (primeiro, ultimo) = (tipos.first()?, tipos.last()?);
    let unidade = inf.program.unit(unit);
    let (de, ate) = (unidade.ast.ty(*primeiro).span.start, unidade.ast.ty(*ultimo).span.end);
    let fonte = &unidade.source;
    let inicio = fonte.get(..de).and_then(|t| t.rfind('<')).unwrap_or(de);
    let fim = fonte.get(ate..).and_then(|t| t.find('>')).map_or(ate, |i| ate + i + 1);
    Some(Span { start: inicio, end: fim })
}

/// `WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR` (`ast_rewrite.dart:438-445`,
/// `:604-611`; `named_type_resolver.dart:337-343`;
/// `function_reference_resolver.dart:51-62`): argumentos de tipo depois do
/// nome de um construtor, na lista `<…>`; `{0}` o tipo como escrito (com o
/// prefixo), `{1}` o construtor.
pub(crate) fn tipos_no_construtor(inf: &mut BodyInferrer<'_>, unit: dartforge_elements::model::UnitId, classe_escrita: Span, construtor: ast::Name, tipos: &[ast::TypeId]) {
    let Some(sp) = faixa_da_lista_de_tipos(inf, unit, tipos) else { return };
    let fonte = &inf.program.unit(unit).source;
    let classe = fonte[classe_escrita.start..classe_escrita.end].split_whitespace().collect::<String>();
    let nome = inf.interner.resolve(construtor.sym).to_string();
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR, sp, &[&classe, &nome]);
}

pub(crate) fn chamada(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId) -> (TypeId, bool) {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::Call { target, arguments } = &a.expr(e).kind else { unreachable!() };
    let (target, args): (ExprId, &ast::Arguments) = (*target, arguments);
    let span = a.expr(e).span;
    let explicitos = argumentos_de_tipo(inf, cx, args);
    // `C<T>.nome(…)` é criação só quando o fasta a lê como criação implícita
    // (`criacoes_implicitas`); com argumentos de tipo depois do nome
    // (`C<T>.nome<U>()`) é método do literal de tipo, que o `AstRewriter` não
    // reescreve.
    let implicita = a.criacoes_implicitas.contains(&e);
    let sobre_instanciacao = matches!(&a.expr(target).kind, ExprKind::Property { target: r, .. } if matches!(a.expr(*r).kind, ExprKind::TypeArguments { .. }));
    let construtor = if sobre_instanciacao && !implicita { None } else { alvo_construtor(inf, cx, target) };
    if implicita && let ExprKind::Property { target: recv, .. } = &a.expr(target).kind {
        expr::conferir_argumentos_do_literal(inf, cx, *recv, true);
    }
    if construtor.is_none()
        && implicita
        && let ExprKind::Property { target: recv, name, .. } = &a.expr(target).kind
        && let Some(rt) = referencia_a_tipo(inf, cx, *recv)
        && !matches!(rt, RefTipo::Extensao(_))
    {
        // A criação implícita sem o construtor: `InstanceCreationExpression`
        // de construtor não resolvido (`NEW_WITH_UNDEFINED_CONSTRUCTOR`, no
        // nome, com o tipo como escrito); o tipo é o do tipo nomeado e os
        // argumentos ficam sem contexto.
        let (recv, name) = (*recv, *name);
        let (c, targs) = match &rt {
            RefTipo::Classe(c, Some(ts)) => (*c, ts.iter().map(|&t| inf.tipo_de_argumento_de_tipo(cx, t)).collect::<Vec<_>>()),
            RefTipo::Classe(c, None) => (*c, Vec::new()),
            RefTipo::Alias(c, args, _) => (*c, args.clone().unwrap_or_default()),
            RefTipo::Extensao(_) => unreachable!(),
        };
        registrar_referencia(inf, cx, recv);
        if !matches!(inf.program.class(c).kind, ClassKind::Enum | ClassKind::Mixin) {
            let base = match &a.expr(recv).kind {
                ExprKind::TypeArguments { target: b, .. } => *b,
                _ => recv,
            };
            let qualificado = match &a.expr(base).kind {
                ExprKind::Identifier(n) => inf.interner.resolve(n.sym).to_string(),
                ExprKind::Property { target: p, name: n, .. } => match &a.expr(*p).kind {
                    ExprKind::Identifier(p) => format!("{}.{}", inf.interner.resolve(p.sym), inf.interner.resolve(n.sym)),
                    _ => inf.interner.resolve(n.sym).to_string(),
                },
                _ => String::new(),
            };
            let nome = inf.interner.resolve(name.sym).to_string();
            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::NEW_WITH_UNDEFINED_CONSTRUCTOR, name.span, &[&qualificado, &nome]);
        }
        for x in args.args.iter() {
            inferir_livre(inf, cx, x.value);
        }
        return (inf.tipo_de_classe_com_args(c, targs), false);
    }
    // Construtor sem `new`.
    if let Some((c, f, targs)) = construtor {
        if inf.program.class(c).kind == ClassKind::Enum
            && f.is_some_and(|f| !inf.program.function(f).factory)
        {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), a.expr(target).span);
            for arg in args.args.iter() {
                inferir_livre(inf, cx, arg.value);
            }
            return (inf.core.dynamic_, false);
        }
        if let Some(f) = f {
            // O tipo nomeado da criação implícita: `A`, `A<int>`, `p.A`
            // (sem o nome do construtor).
            let sp = match &a.expr(target).kind {
                ExprKind::Property { target: t, .. } if expr::referencia_a_tipo(inf, cx, *t).is_some() => tipo_nomeado_da_criacao(inf, cx, *t, &[]),
                _ => tipo_nomeado_da_criacao(inf, cx, target, &args.type_args),
            };
            avisar_classe_abstrata(inf, c, f, sp);
        }
        // `C(...)`/`p.C(...)` viram `InstanceCreationExpression` no analyzer:
        // o nome é `C.new` (ou o do construtor) e a entidade, o
        // `constructorName` (tipo com prefixo e argumentos, mais `.nome`).
        let (nome, ent) = match &a.expr(target).kind {
            ExprKind::Property { target: t, name, .. } if expr::referencia_a_tipo(inf, cx, *t).is_some() => {
                (inf.interner.resolve(name.sym).to_string(), a.expr(target).span)
            }
            _ => {
                let classe = ultimo_identificador(inf, cx, target).map(|s| inf.interner.resolve(s).to_string()).unwrap_or_default();
                (format!("{classe}.new"), tipo_nomeado_da_criacao(inf, cx, target, &args.type_args))
            }
        };
        // `C.nome<T>()`: a lista é do construtor (erro) e não da classe.
        let explicitos = match &a.expr(target).kind {
            ExprKind::Property { target: t, name, .. } if !args.type_args.is_empty() && expr::referencia_a_tipo(inf, cx, *t).is_some() => {
                let escrito = a.expr(*t).span;
                tipos_no_construtor(inf, cx.unit, escrito, *name, &args.type_args);
                None
            }
            _ => explicitos,
        };
        definir_alvo(inf, Some(nome), ent);
        let t = construir(inf, cx, Some(e), c, f, targs.or(explicitos), args, ctx);
        inf.alvo_da_aridade = None;
        return (t, false);
    }
    // `E()`: o sem nome implícito do enum não está na tabela de
    // construtores (então `alvo_construtor` não o achou), mas existe e é
    // gerador. Com nome explícito, o braço acima já resolveu (factory é
    // legal, gerador acusa); aqui só falta o implícito.
    if let Some(rt) = referencia_a_tipo(inf, cx, target)
        && !matches!(rt, RefTipo::Extensao(_))
    {
        let c = match rt {
            RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) => c,
            RefTipo::Extensao(_) => unreachable!(),
        };
        // `A()` de classe abstrata sem construtor declarado: o padrão
        // implícito é gerador (`INSTANTIATE_ABSTRACT_CLASS`).
        {
            let k = inf.program.class(c);
            if matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication)
                && (k.modifiers.abstract_ || k.modifiers.sealed)
                && k.constructors.is_empty()
            {
                let sp = tipo_nomeado_da_criacao(inf, cx, target, &args.type_args);
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, sp, &[]);
            }
        }
        if inf.program.class(c).kind == ClassKind::Enum
            && inf.sym.vazio.is_some_and(|v| inf.construtor_de(c, v).is_none())
        {
            inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), a.expr(target).span);
            for arg in args.args.iter() {
                inferir_livre(inf, cx, arg.value);
            }
            return (inf.core.dynamic_, false);
        }
    }
    let u = inf.core.unknown;
    // `E(a, b)` / `E()`: `INVALID_EXTENSION_ARGUMENT_COUNT` na lista de
    // argumentos (an611:src/dart/resolver/extension_member_resolver.dart:189-197).
    if let Some(RefTipo::Extensao(_)) = referencia_a_tipo(inf, cx, target) {
        override_sem_acesso(inf, cx, e);
    }
    if let Some(RefTipo::Extensao(x)) = referencia_a_tipo(inf, cx, target)
        && !(args.args.len() == 1 && args.args[0].name.is_none())
        && args.args.iter().all(|a| a.name.is_none())
        && args.args.len() != 1
    {
        let fonte = &inf.program.unit(cx.unit).source;
        let ini = args.type_args.last().map(|&t| a.ty(t).span.end).unwrap_or(args.span.start);
        let abre = fonte.get(ini..).and_then(|r| r.find('(')).map(|i| ini + i).unwrap_or(args.span.start);
        let sp = Span { start: abre, end: args.span.end.max(abre) };
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::INVALID_EXTENSION_ARGUMENT_COUNT, sp, &[]);
        for arg in args.args.iter() {
            inferir_livre(inf, cx, arg.value);
        }
        let dados = inf.outline.extensions[x.0 as usize].clone();
        let ext_args = inf.instanciar_para_limites(&dados.type_params);
        cx.sobreposicoes.insert(e, (x, ext_args));
        return (inf.core.dynamic_, false);
    }
    // `E(x)` / `E<T>(x)`: sobreposição explícita de extensão (R-EXT-02).
    if let Some(RefTipo::Extensao(x)) = referencia_a_tipo(inf, cx, target)
        && args.args.len() == 1
        && args.args[0].name.is_none()
    {
        let dados = inf.outline.extensions[x.0 as usize].clone();
        // `E<A, B>(x)` com número errado de argumentos de tipo
        // (an611:src/dart/resolver/extension_member_resolver.dart:333-345).
        if let Some(ex) = &explicitos
            && ex.len() != dados.type_params.len()
            && let (Some(&p), Some(&u2)) = (args.type_args.first(), args.type_args.last())
        {
            let fonte = &inf.program.unit(cx.unit).source;
            let ini_t = a.ty(p).span.start;
            let fim_t = a.ty(u2).span.end;
            let abre = fonte.get(..ini_t).and_then(|r| r.rfind('<')).unwrap_or(ini_t);
            let fecha = fonte.get(fim_t..).and_then(|r| r.find('>')).map(|i| fim_t + i + 1).unwrap_or(fim_t);
            let nome = inf.program.extension(x).name.map(|n| inf.interner.resolve(n).to_string()).unwrap_or_default();
            let (np, na) = (dados.type_params.len().to_string(), ex.len().to_string());
            inf.aviso_com_codigo(
                dartforge_diagnostics::codigos::compile_time_error::WRONG_NUMBER_OF_TYPE_ARGUMENTS_EXTENSION,
                Span { start: abre, end: fecha },
                &[&nome, &np, &na],
            );
        }
        let ext_args = match &explicitos {
            Some(ex) if ex.len() == dados.type_params.len() => Some(ex.clone()),
            _ => None,
        };
        let ctx_arg = match &ext_args {
            Some(ex) => {
                let mapa = inf.mapa(&dados.type_params, ex);
                inf.subst(dados.on, &mapa)
            }
            None => u,
        };
        let t = inferir(inf, cx, args.args[0].value, ctx_arg);
        let explicitos_ok = ext_args.is_some();
        let ext_args = match ext_args {
            Some(ex) => ex,
            None => inf.extensao_aplicavel(x, t).unwrap_or_else(|| inf.instanciar_para_limites(&dados.type_params)),
        };
        // `EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE` no argumento
        // (an611:src/dart/resolver/extension_member_resolver.dart:227-243);
        // `void` é `USE_OF_VOID_RESULT`. Só sem parâmetros de tipo ou com
        // argumentos explícitos (a inferência falha tem outro relato).
        if dados.type_params.is_empty() || explicitos_ok {
            let arg = args.args[0].value;
            if !expr::uso_de_void(inf, cx, arg, t) {
                let mapa = inf.mapa(&dados.type_params, &ext_args);
                let on = inf.subst(dados.on, &mapa);
                if !inf.atribuivel(t, on) {
                    let sp = inf.span_expr(cx.unit, arg);
                    let desde = inf.diagnostics.len();
                    inf.aviso_com_args(
                        dartforge_diagnostics::codigos::compile_time_error::EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE,
                        sp,
                        &[crate::exibicao::Arg::Tipo(t), crate::exibicao::Arg::Tipo(on)],
                    );
                    inf.anexar_nao_promocao(desde, cx, Some(arg), sp);
                }
            }
        }
        cx.sobreposicoes.insert(e, (x, ext_args));
        return (t, false);
    }
    match &a.expr(target).kind {
        ExprKind::Property { target: recv, name, null_aware } => {
            let (recv, name, null_aware) = (*recv, *name, *null_aware);
            if null_aware && expr::referencia_a_tipo(inf, cx, recv).is_some() {
                expr::operador_nulo_em_tipo(inf, cx, recv);
            }
            // `p.f(args)`
            if let ExprKind::Identifier(p) = &a.expr(recv).kind {
                if matches!(resolver_nome(inf, cx, p.sym, false), RefNome::Prefixo) {
                    // `p.f(args)` sem getter `f` no prefixo
                    // (`_resolveReceiverPrefix`,
                    // `an611:src/dart/resolver/method_invocation_resolver.dart:666-722`):
                    // `UNDEFINED_FUNCTION`, salvo `loadLibrary` de import
                    // adiado e o nome ignorado de import que não existe.
                    let p = *p;
                    if inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter).is_none()
                        && expr::load_library(inf, cx, p.sym, name.sym).is_none()
                    {
                        expr::resolver(inf, cx, recv, Resolved::Prefix(cx.lib));
                        if !crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, cx.unit, Some(p.sym), name.sym) {
                            let nome = inf.interner.resolve(name.sym).to_string();
                            inf.aviso_com_codigo(
                                dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_FUNCTION,
                                name.span,
                                &[&nome],
                            );
                        }
                        let d = inf.core.dynamic_;
                        registrar(inf, cx, target, d);
                        for x in args.args.iter() {
                            inferir_livre(inf, cx, x.value);
                        }
                        return (d, false);
                    }
                    let t = inferir(inf, cx, target, u);
                    let f = funcao_resolvida(inf, cx, target);
                    preparar_entidade(inf, name.span, f);
                    alvo_de_metodo(inf, f, name, span);
                    let (r, _) = invocar_valor(inf, cx, e, t, args, ctx, explicitos, span);
                    inf.entidade_da_inferencia = None;
                    inf.alvo_da_aridade = None;
                    return (r, false);
                }
            }
            // `C.m(args)` estático / `E.m(args)`. Só um receptor identificador
            // (`receiver is IdentifierImpl`, method_invocation_resolver.dart:129-143):
            // o literal de tipo instanciado (`C<T>.m<U>()`) é expressão de
            // tipo `Type`, e o método é procurado nele.
            let instanciado = matches!(inf.program.unit(cx.unit).ast.expr(recv).kind, ExprKind::TypeArguments { .. });
            if !instanciado && let Some(rt) = referencia_a_tipo(inf, cx, recv) {
                if let RefTipo::Extensao(x) = rt {
                    if inf.membro_estatico_de_extensao(x, name.sym, false).is_none() {
                        if inf.program.extension(x).instance_members.contains_key(&name.sym) {
                            let msg = format!("{}: '{}'", STATIC_ACCESS_TO_INSTANCE_MEMBER.template, inf.interner.resolve(name.sym));
                            inf.aviso(msg, name.span);
                            for arg in args.args.iter() {
                                inferir_livre(inf, cx, arg.value);
                            }
                            return (inf.core.dynamic_, false);
                        }
                        let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                        let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_METHOD.template, inf.interner.resolve(name.sym), extensao);
                        inf.aviso(msg, name.span);
                        for arg in args.args.iter() {
                            inferir_livre(inf, cx, arg.value);
                        }
                        return (inf.core.dynamic_, false);
                    }
                }
                // `C.m(args)` sem membro estático `m` nem de instância (este
                // é `STATIC_ACCESS_TO_INSTANCE_MEMBER`, pelo acesso): o
                // analyzer relata `UNDEFINED_METHOD` com o **nome** do
                // elemento (`_resolveReceiverTypeLiteral`,
                // an611:src/dart/resolver/method_invocation_resolver.dart:890-939;
                // alias de interface: o elemento aliasado). `C.new()` e
                // `C<T>.m()` seguem o caminho comum.
                let classe = match &rt {
                    RefTipo::Classe(c, _) | RefTipo::Alias(c, _, _) => Some(*c),
                    _ => None,
                };
                if let Some(c) = classe
                    && !matches!(a.expr(recv).kind, ExprKind::TypeArguments { .. })
                    && Some(name.sym) != inf.sym.new_
                    && inf.membro_estatico(c, name.sym, false).is_none()
                    && !inf.program.class(c).instance_members.contains_key(&name.sym)
                {
                    let nome = inf.interner.resolve(name.sym).to_string();
                    let tipo = inf.interner.resolve(inf.program.class(c).name).to_string();
                    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_METHOD, name.span, &[&nome, &tipo]);
                    // `_setInvalidTypeResolution`: a invocação que não
                    // resolve tem o tipo de recuperação.
                    let d = inf.table.invalido(inf.core.dynamic_);
                    registrar(inf, cx, target, d);
                    for arg in args.args.iter() {
                        inferir_livre(inf, cx, arg.value);
                    }
                    return (d, false);
                }
                let t = inferir(inf, cx, target, u);
                let f = funcao_resolvida(inf, cx, target);
                preparar_entidade(inf, name.span, f);
                alvo_de_metodo(inf, f, name, span);
                let (r, _) = invocar_valor(inf, cx, e, t, args, ctx, explicitos, span);
                inf.entidade_da_inferencia = None;
                inf.alvo_da_aridade = None;
                return (r, false);
            }
            // `super.m(args)`.
            if matches!(a.expr(recv).kind, ExprKind::Super) {
                let this = cx.tipo_this.unwrap_or(inf.core.dynamic_);
                registrar(inf, cx, recv, this);
                let t = expr::membro_super(inf, cx, target, name, expr::UsoDoSuper::Invocacao);
                registrar(inf, cx, target, t);
                let f = funcao_resolvida(inf, cx, target);
                alvo_de_metodo(inf, f, name, span);
                let (r, _) = invocar_valor(inf, cx, e, t, args, ctx, explicitos, span);
                inf.alvo_da_aridade = None;
                return (r, false);
            }
            let (r_ty, curto) = receptor(inf, cx, recv, null_aware);
            if !null_aware && !cx.sobreposicoes.contains_key(&recv) && expr::receptor_nunca(inf, cx, recv, r_ty) {
                for arg in args.args.iter() {
                    inferir_livre(inf, cx, arg.value);
                }
                return (inf.core.never, curto);
            }
            // `void` e receptor anulável: as mesmas regras do acesso a
            // propriedade (`expr::propriedade`), na variante de invocação.
            if matches!(inf.table.get(r_ty), crate::table::Type::Void) {
                // Na invocação, o analyzer relata no receptor (`this` em
                // `this.m()`); no acesso a propriedade, no nome.
                // Numa seção de cascata (`x..m()`), o relato é no alvo da
                // cascata, feito lá.
                if !matches!(inf.program.unit(cx.unit).ast.expr(recv).kind, ExprKind::CascadeTarget) {
                    let sp = inf.span_expr(cx.unit, recv);
                    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::USE_OF_VOID_RESULT, sp, &[]);
                }
                let d = inf.core.dynamic_;
                registrar(inf, cx, target, d);
                for x in args.args.iter() {
                    inferir_livre(inf, cx, x.value);
                }
                return (d, curto);
            }
            let checar_nulo =
                !cx.sobreposicoes.contains_key(&recv) && inf.exige_checagem_de_nulo(cx.lib, r_ty, name.sym, false);
            if checar_nulo {
                let nome = inf.interner.resolve(name.sym).to_string();
                let desde = inf.diagnostics.len();
                inf.aviso_de_nulo(
                    r_ty,
                    dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE,
                    name.span,
                    &[&nome],
                );
                inf.anexar_nao_promocao(desde, cx, Some(recv), name.span);
            }
            let mut busca = expr::buscar_membro_do_alvo(inf, cx, recv, r_ty, name.sym, false);
            inf.relatar_ambiguidade_de_extensao(name.span);
            if matches!(busca, Busca::Ausente) {
                if let Some((x, _)) = cx.sobreposicoes.get(&recv).cloned() {
                    if let Some(m) = inf.membro_estatico_de_extensao(x, name.sym, false) {
                        // O analyzer ainda resolve a assinatura do método,
                        // mas rejeita seu acesso via `E(valor).metodo()`.
                        inf.aviso(
                            EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(),
                            name.span,
                        );
                        busca = Busca::Achado(m);
                    }
                } else if let Some(m) = inf.acesso_de_instancia_a_estatico(cx.lib, r_ty, name.sym, false, name.span) {
                    // O estático recuperado é o alvo da invocação
                    // (`method_invocation_resolver.dart:841-857`): um getter
                    // vira `FunctionExpressionInvocation` do tipo de retorno.
                    busca = Busca::Achado(m);
                }
            }
            match busca {
                Busca::Achado(m) => {
                    resolver(inf, cx, target, m.resolved.clone());
                    registrar(inf, cx, target, m.tipo);
                    // O nome do método guarda o tipo não instanciado (como o
                    // `methodName.staticType` do analyzer).
                    if m.metodo {
                        if let Some(sym) = [inf.sym.remainder, inf.sym.clamp].into_iter().flatten().find(|s| *s == name.sym) {
                            let param = match inf.table.get(m.tipo) {
                                Type::Function { positional, .. } => positional.first().copied(),
                                _ => None,
                            };
                            if let Some(pt) = param {
                                inf.contexto_numerico_pendente = Some(expr::contexto_numerico(inf, r_ty, &m, sym, ctx, pt));
                            }
                        }
                    }
                    let (mut r, _) = if m.metodo {
                        let t = inf.nao_nulo(m.tipo);
                        preparar_entidade(inf, name.span, m.funcao);
                        // `f.call(...)` com `f` de tipo função: a entidade do
                        // nomeado obrigatório é a lista de argumentos.
                        let r_nn = inf.nao_nulo(r_ty);
                        let ent = if Some(name.sym) == inf.sym.call && matches!(inf.table.get(r_nn), Type::Function { .. }) {
                            args.span
                        } else {
                            name.span
                        };
                        { let t = inf.interner.resolve(name.sym).to_string(); definir_alvo(inf, Some(t), ent); }
                        let r = invocar(inf, cx, t, args, ctx, explicitos);
                        inf.entidade_da_inferencia = None;
                        r
                    } else {
                        definir_alvo(inf, None, span);
                        let r = invocar_valor(inf, cx, e, m.tipo, args, ctx, explicitos, span);
                        inf.alvo_da_aridade = None;
                        r
                    };
                    if m.metodo {
                        // Refinamento numérico de `remainder`/`clamp`.
                        if let Some(sym) = [inf.sym.remainder, inf.sym.clamp].into_iter().flatten().find(|s| *s == name.sym) {
                            let tipos: Vec<TypeId> = args
                                .args
                                .iter()
                                .map(|x| inf.body_types.units[cx.unit.0 as usize].get_type(x.value).unwrap_or(inf.core.dynamic_))
                                .collect();
                            r = expr::refinar_numerico(inf, r_ty, &m, sym, &tipos, r);
                        }
                    }
                    (r, curto)
                }
                // Invocação sobre `InvalidType`: tudo inválido, e os
                // argumentos são inferidos sem contexto
                // (`method_invocation_resolver.dart:459-463`).
                Busca::Dinamico if inf.table.e_invalido(r_ty) => {
                    resolver(inf, cx, target, Resolved::Dynamic);
                    registrar(inf, cx, target, r_ty);
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    (r_ty, curto)
                }
                Busca::Dinamico => {
                    resolver(inf, cx, target, Resolved::Dynamic);
                    // Métodos de `Object` mantêm o tipo mesmo em `dynamic`.
                    let o = inf.core.object;
                    if let Some(m) = inf.membro_de_interface(o, name.sym, false) {
                        if m.metodo {
                            registrar(inf, cx, target, m.tipo);
                            { let t = inf.interner.resolve(name.sym).to_string(); definir_alvo(inf, Some(t), name.span); }
                            let (r, _) = invocar(inf, cx, m.tipo, args, ctx, explicitos);
                            return (r, curto);
                        }
                    }
                    let d = inf.core.dynamic_;
                    registrar(inf, cx, target, d);
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    (inf.core.dynamic_, curto)
                }
                Busca::Nunca => {
                    let n = inf.core.never;
                    registrar(inf, cx, target, n);
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    (inf.core.never, curto)
                }
                Busca::Ausente if checar_nulo => {
                    let d = inf.core.dynamic_;
                    registrar(inf, cx, target, d);
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    (d, curto)
                }
                Busca::Ausente => {
                    if let Some((x, _)) = cx.sobreposicoes.get(&recv).cloned() {
                        let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                        let msg = format!("{}: '{}' em '{}'", UNDEFINED_EXTENSION_METHOD.template, inf.interner.resolve(name.sym), extensao);
                        inf.aviso(msg, name.span);
                        let d = inf.core.dynamic_;
                        registrar(inf, cx, target, d);
                        for x in args.args.iter() {
                            inferir_livre(inf, cx, x.value);
                        }
                        return (d, curto);
                    }
                    // `{1}` é o nome do elemento, não o tipo
                    // (`_resolveReceiverType`,
                    // an611:src/dart/resolver/method_invocation_resolver.dart:862-873).
                    let nome = inf.interner.resolve(name.sym).to_string();
                    let tipo = nome_do_receptor(inf, r_ty);
                    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::UNDEFINED_METHOD, name.span, &[&nome, &tipo]);
                    // `_setInvalidTypeResolution`
                    // (`method_invocation_resolver.dart:1062-1068`).
                    let d = inf.table.invalido(inf.core.dynamic_);
                    registrar(inf, cx, target, d);
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    (d, curto)
                }
            }
        }
        _ => {
            // `assert(…)` como expressão: `FunctionExpressionInvocation` do
            // identificador `assert`, lido como nome (`UNDEFINED_IDENTIFIER`)
            // e invocado como valor.
            let de_assert = a.invocacoes_de_assert.contains(&e);
            // `nome(args)` sem getter no escopo léxico: o erro é o da
            // invocação (`UNDEFINED_FUNCTION`/`UNDEFINED_METHOD`…), não o
            // de nome indefinido (ver `expr::invocacao_sem_alvo_indefinida`).
            if !de_assert
                && let ExprKind::Identifier(n) = &a.expr(target).kind
                && expr::invocacao_sem_alvo_indefinida(inf, cx, *n)
            {
                let d = inf.table.invalido(inf.core.dynamic_);
                registrar(inf, cx, target, d);
                for x in args.args.iter() {
                    inferir_livre(inf, cx, x.value);
                }
                return (d, false);
            }
            // `T()` com `T` parâmetro de tipo (`_reportInvocationOfNonFunction`,
            // `method_invocation_resolver.dart:257-270`, de
            // `_resolveReceiverNull`): o elemento achado no escopo não é
            // executável nem variável. No nome; o resultado é inválido.
            if !de_assert
                && let ExprKind::Identifier(n) = &a.expr(target).kind
                && matches!(cx.buscar(n.sym), Some(super::corpo::Nome::TipoParam(_)))
            {
                let nome = inf.interner.resolve(n.sym).to_string();
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::INVOCATION_OF_NON_FUNCTION, n.span, &[&nome]);
                let d = inf.table.invalido(inf.core.dynamic_);
                registrar(inf, cx, target, d);
                for x in args.args.iter() {
                    inferir_livre(inf, cx, x.value);
                }
                return (d, false);
            }
            let t = inferir(inf, cx, target, u);
            if let Some((x, ext_args)) = cx.sobreposicoes.get(&target).cloned() {
                if let Some(call) = inf.sym.call {
                    if let Some(m) = inf.membro_de_extensao_explicita(x, &ext_args, call, false) {
                        definir_alvo(inf, None, span);
                        let (r, _) = invocar(inf, cx, m.tipo, args, ctx, explicitos);
                        return (r, false);
                    }
                    if let Some(m) = inf.membro_estatico_de_extensao(x, call, false) {
                        // `E(valor)()` usa a lista de argumentos como localização
                        // do erro, conforme o analyzer oficial.
                        inf.aviso(
                            EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template.to_string(),
                            args.span,
                        );
                        definir_alvo(inf, None, span);
                        let (r, _) = invocar(inf, cx, m.tipo, args, ctx, explicitos);
                        return (r, false);
                    }
                }
                let extensao = inf.program.extension(x).name.map(|n| inf.interner.resolve(n)).unwrap_or("");
                let msg = format!("{}: '{}'", INVOCATION_OF_EXTENSION_WITHOUT_CALL.template, extensao);
                inf.aviso(msg, a.expr(target).span);
                for arg in args.args.iter() {
                    inferir_livre(inf, cx, arg.value);
                }
                return (inf.core.dynamic_, false);
            }
            if !de_assert && let ExprKind::Identifier(n) = &a.expr(target).kind {
                let f = funcao_resolvida(inf, cx, target);
                preparar_entidade(inf, a.expr(target).span, f);
                // Função (de topo, método, local) é `MethodInvocation` (a
                // entidade é o nome); variável ou getter vira
                // `FunctionExpressionInvocation` (a chamada inteira). O nome
                // é o identificador nos dois casos.
                let funcao = match f {
                    Some(f) => matches!(inf.program.function(f).kind, FunctionKind::Function),
                    None => match cx.buscar(n.sym) {
                        Some(super::corpo::Nome::Local(id)) => local_e_funcao(inf, cx, cx.locais[id.0 as usize].offset),
                        _ => false,
                    },
                };
                let ent = if funcao { n.span } else { span };
                { let t = inf.interner.resolve(n.sym).to_string(); definir_alvo(inf, Some(t), ent); }
            } else {
                definir_alvo(inf, None, span);
            }
            let (r, _) = invocar_valor(inf, cx, e, t, args, ctx, explicitos, span);
            inf.entidade_da_inferencia = None;
            inf.alvo_da_aridade = None;
            (r, false)
        }
    }
}

/// Alvo da aridade de `x.m(...)`/`p.f(...)`/`super.m(...)`: um método (ou
/// função) é `MethodInvocation` (nome e entidade no `m`); um getter ou
/// variável vira `FunctionExpressionInvocation` sobre o acesso (sem nome,
/// a chamada inteira).
fn alvo_de_metodo(inf: &mut BodyInferrer<'_>, f: Option<FunctionElementId>, name: ast::Name, span: Span) {
    if f.is_some_and(|f| matches!(inf.program.function(f).kind, FunctionKind::Function)) {
        { let t = inf.interner.resolve(name.sym).to_string(); definir_alvo(inf, Some(t), name.span); }
    } else {
        definir_alvo(inf, None, span);
    }
}

/// O `{1}` do `UNDEFINED_METHOD` de instância: o nome do elemento do tipo
/// do receptor (sem argumentos de tipo), `'Function'` para tipos de função
/// e `'<unknown>'` para o resto (records, parâmetros de tipo).
pub(crate) fn nome_do_receptor(inf: &BodyInferrer<'_>, t: TypeId) -> String {
    match inf.table.get(t) {
        Type::Interface { class, .. } => inf.interner.resolve(inf.program.class(*class).name).to_string(),
        Type::ExtensionType { decl, .. } => inf.interner.resolve(inf.program.class(*decl).name).to_string(),
        Type::FutureOr { .. } => "FutureOr".to_string(),
        Type::Null => "Null".to_string(),
        Type::Function { .. } => "Function".to_string(),
        _ => "<unknown>".to_string(),
    }
}

/// O local declarado em `offset` é uma função local (o nome segue de `(`
/// ou `<` na declaração).
fn local_e_funcao(inf: &BodyInferrer<'_>, cx: &Corpo, offset: usize) -> bool {
    let fonte = &inf.program.unit(cx.unit).source;
    let resto = fonte.get(offset..).unwrap_or("");
    let depois = resto.trim_start_matches(|c: char| c.is_alphanumeric() || c == '_' || c == '$').trim_start();
    depois.starts_with('(') || depois.starts_with('<')
}

/// O último identificador de uma referência a tipo (`C`, `p.C`, `C<int>`).
fn ultimo_identificador(inf: &BodyInferrer<'_>, cx: &Corpo, e: ExprId) -> Option<dartforge_intern::SymbolId> {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => Some(n.sym),
        ExprKind::Property { name, .. } => Some(name.sym),
        ExprKind::TypeArguments { target, .. } => ultimo_identificador(inf, cx, *target),
        _ => None,
    }
}

/// Se `target` (alvo de uma chamada) nomeia um construtor: `(classe, construtor, args explícitos)`.
fn alvo_construtor(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, target: ExprId) -> Option<(ClassId, Option<FunctionElementId>, Option<Vec<TypeId>>)> {
    let vazio = inf.sym.vazio?;
    let tipo_args = |inf: &mut BodyInferrer<'_>, cx: &Corpo, rt: &RefTipo| -> (ClassId, Option<Vec<TypeId>>) {
        match rt {
            RefTipo::Classe(c, Some(ts)) => (*c, Some(ts.iter().map(|&t| inf.tipo_de_argumento_de_tipo(cx, t)).collect())),
            RefTipo::Classe(c, None) => (*c, None),
            RefTipo::Alias(c, args, _) => (*c, args.clone()),
            RefTipo::Extensao(_) => unreachable!(),
        }
    };
    if let Some(rt) = referencia_a_tipo(inf, cx, target) {
        if matches!(rt, RefTipo::Extensao(_)) {
            return None;
        }
        let (c, args) = tipo_args(inf, cx, &rt);
        let f = inf.construtor_ou_primario(c, vazio)?;
        registrar_referencia(inf, cx, target);
        return Some((c, f, args));
    }
    let a = &inf.program.unit(cx.unit).ast;
    if let ExprKind::Property { target: recv, name, null_aware: false } = &a.expr(target).kind {
        let (recv, name) = (*recv, *name);
        let rt = referencia_a_tipo(inf, cx, recv)?;
        if matches!(rt, RefTipo::Extensao(_)) {
            return None;
        }
        let (c, args) = tipo_args(inf, cx, &rt);
        let chave = if Some(name.sym) == inf.sym.new_ { vazio } else { name.sym };
        let f = inf.construtor_ou_primario(c, chave)?;
        registrar_referencia(inf, cx, recv);
        if let Some(f) = f
            && inf.program.function(f).class == Some(c)
        {
            resolver(inf, cx, target, Resolved::Constructor(f));
        }
        return Some((c, f, args));
    }
    None
}

/// Registra tipo/resolução dos nós de uma referência a classe.
fn registrar_referencia(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId) {
    let a = &inf.program.unit(cx.unit).ast;
    let tt = inf.core.type_;
    match &a.expr(e).kind {
        ExprKind::Identifier(n) => {
            if let Some(el) = inf.program.lookup_na_unidade(cx.unit, n.sym).and_then(|b| b.getter) {
                resolver(inf, cx, e, Resolved::Element(el));
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::Property { target, name, .. } => {
            let (target, name) = (*target, *name);
            if let ExprKind::Identifier(p) = &a.expr(target).kind {
                resolver(inf, cx, target, Resolved::Prefix(cx.lib));
                if let Some(el) = inf.program.lookup_prefixed_na_unidade(cx.unit, p.sym, name.sym).and_then(|b| b.getter) {
                    resolver(inf, cx, e, Resolved::Element(el));
                }
            }
            registrar(inf, cx, e, tt);
        }
        ExprKind::TypeArguments { target, .. } => {
            let t = *target;
            registrar_referencia(inf, cx, t);
            registrar(inf, cx, e, tt);
        }
        _ => {}
    }
}

impl<'a> BodyInferrer<'a> {
    /// Parâmetros novos (por classe) para inferir os argumentos de tipo de
    /// construtores: os da classe podem estar em escopo no ponto da chamada
    /// (`C(x)` dentro de `C<T>`), e não podem ser confundidos.
    /// Construtor `nome` de `c`. Aplicação de mixin (`class A = B with M;`)
    /// tem os construtores generativos da superclasse encaminhados
    /// (especificação, "Mixin Application").
    pub(crate) fn construtor_de(&self, c: ClassId, nome: dartforge_intern::SymbolId) -> Option<FunctionElementId> {
        let cl = self.program.class(c);
        if let Some(&f) = cl.constructors.get(&nome) {
            return Some(f);
        }
        if cl.kind != ClassKind::MixinApplication {
            return None;
        }
        let f = self.construtor_de(cl.supertype_class?, nome)?;
        (!self.program.function(f).factory).then_some(f)
    }

    /// Construtor `nome` de `c`, ou `Some(None)` para o construtor primário
    /// de um tipo de extensão (`extension type Id(int v)`, R-EXT-04), que o
    /// modelo de elementos não cria.
    pub(crate) fn construtor_ou_primario(&self, c: ClassId, nome: dartforge_intern::SymbolId) -> Option<Option<FunctionElementId>> {
        if let Some(f) = self.construtor_de(c, nome) {
            return Some(Some(f));
        }
        let cl = self.program.class(c);
        if cl.kind != ClassKind::ExtensionType {
            return None;
        }
        let d = cl.decl?;
        match &self.program.unit(d.unit).ast.decl(d.decl).kind {
            ast::DeclKind::ExtensionType(et) if et.constructor.map(|n| n.sym).or(self.sym.vazio) == Some(nome) => Some(None),
            _ => None,
        }
    }

    /// `(Representação) -> E<parâmetros>`: o construtor primário.
    pub(crate) fn assinatura_primario(&mut self, c: ClassId) -> TypeId {
        let rep = self.program.class(c).representation;
        let t = rep
            .and_then(|v| self.outline.variables[v.0 as usize].declared_type)
            .unwrap_or(self.core.dynamic_);
        let this = self.tipo_this_classe(c);
        self.table.intern(Type::Function {
            type_params: Box::new([]),
            ret: this,
            positional: Box::new([t]),
            optional: Box::new([]),
            named: Box::new([]),
            nullable: false,
        })
    }

    /// Assinatura do construtor `f` vista de `c` (encaminhado quando `f` é
    /// da superclasse de uma aplicação de mixin): tipos pelos argumentos do
    /// supertipo e retorno `c<parâmetros>`.
    pub(crate) fn assinatura_construtor(&mut self, c: ClassId, f: FunctionElementId) -> TypeId {
        let sig = self.outline.functions[f.0 as usize].signature;
        if self.program.function(f).class == Some(c) {
            return sig;
        }
        let Some(sup) = self.outline.classes[c.0 as usize].supertype else { return sig };
        let Type::Interface { class: sc, args, .. } = self.table.get(sup).clone() else { return sig };
        let s = self.assinatura_construtor(sc, f);
        let params = self.outline.classes[sc.0 as usize].type_params.clone();
        let s = if params.len() == args.len() {
            let mapa = self.mapa(&params, &args);
            self.subst(s, &mapa)
        } else {
            s
        };
        let this = self.tipo_this_classe(c);
        match self.table.get(s).clone() {
            Type::Function { type_params, positional, optional, named, nullable, .. } => {
                self.table.intern(Type::Function { type_params, ret: this, positional, optional, named, nullable })
            }
            _ => s,
        }
    }

    fn parametros_de_construtor(&mut self, c: ClassId) -> (Vec<TypeParamId>, Vec<TypeParamId>) {
        let originais = self.outline.classes[c.0 as usize].type_params.to_vec();
        let novos = self.parametros_novos(&originais);
        (originais, novos)
    }
}

/// Invocação de construtor: infere os argumentos de tipo da classe quando
/// omitidos (como uma função genérica sobre os parâmetros da classe).
#[allow(clippy::too_many_arguments)]
pub(crate) fn construir(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    e: Option<ExprId>,
    c: ClassId,
    f: Option<FunctionElementId>,
    explicitos: Option<Vec<TypeId>>,
    args: &ast::Arguments,
    ctx: TypeId,
) -> TypeId {
    // Construtor encaminhado de aplicação de mixin: sem resolução (o
    // elemento é o da superclasse). `None`: construtor primário de tipo de
    // extensão (R-EXT-04), sem elemento.
    let sig = match f {
        Some(f) => {
            if let Some(e) = e
                && inf.program.function(f).class == Some(c)
            {
                resolver(inf, cx, e, Resolved::Constructor(f));
            }
            inf.assinatura_construtor(c, f)
        }
        None => inf.assinatura_primario(c),
    };
    let (originais, novos) = inf.parametros_de_construtor(c);
    if originais.is_empty() {
        let (r, _) = invocar(inf, cx, sig, args, ctx, None);
        return r;
    }
    if let Some(ex) = explicitos.filter(|x| x.len() == originais.len()) {
        inf.body_types.units[cx.unit.0 as usize].set_instanciacao(args.span.start, ex.clone().into_boxed_slice());
        let mapa = inf.mapa(&originais, &ex);
        let s = inf.subst(sig, &mapa);
        let (r, _) = invocar(inf, cx, s, args, ctx, None);
        return r;
    }
    // Função genérica `<T'..>(params) -> C<T'..>`.
    let tipos: Vec<TypeId> = novos.iter().map(|&p| inf.table.intern(Type::TypeParameter { param: p, nullable: false })).collect();
    let mapa = inf.mapa(&originais, &tipos);
    let s = inf.subst(sig, &mapa);
    let Type::Function { ret, positional, optional, named, .. } = inf.table.get(s).clone() else { return inf.core.dynamic_ };
    let generica = inf.table.intern(Type::Function {
        type_params: novos.into_boxed_slice(),
        ret,
        positional,
        optional,
        named,
        nullable: false,
    });
    // Contexto `C<...>?` ou supertipo: a restrição do retorno cuida.
    if let Some(e) = e {
        let a = &inf.program.unit(cx.unit).ast;
        let sp = match &a.expr(e).kind {
            ExprKind::Call { target, .. } => a.expr(*target).span,
            ExprKind::InstanceCreation { ty, constructor, .. } => {
                let t = a.ty(*ty).span;
                Span { start: t.start, end: constructor.map(|n| n.span.end).unwrap_or(t.end) }
            }
            _ => a.expr(e).span,
        };
        preparar_entidade(inf, sp, f);
    }
    let (r, _) = invocar(inf, cx, generica, args, ctx, None);
    inf.entidade_da_inferencia = None;
    r
}

/// `CREATION_WITH_NON_TYPE` (`new X()`, `const X()`, `X<T>.nome()`): o nome
/// escrito como tipo da criação não é classe — indefinido, local, parâmetro
/// de tipo, função. O intervalo é o nome escrito; com `p.X` e `p` prefixo de
/// import, só o `X`. O argumento é a última parte do nome (`new A.foo()` com
/// `A` indefinido: `'foo'`).
///
/// Ficam de fora (o analyzer relata outro código ou nada):
/// * `p.X` com `p` declarado mas não prefixo (`void p() {}`):
///   `prefix_shadowed_by_local_declaration`;
/// * `p.X` com `p` prefixo de um import que não resolveu: nada.
fn criacao_sem_classe(inf: &mut BodyInferrer<'_>, cx: &Corpo, name: &[ast::Name]) {
    let (Some(primeiro), Some(ultimo)) = (name.first(), name.last()) else { return };
    let mut prefixo = false;
    if name.len() == 2 {
        if inf.program.prefixos_na_unidade(cx.unit).contains_key(&primeiro.sym) {
            // `shouldIgnoreUndefinedNamedType`
            // (`an611:src/dart/resolver/named_type_resolver.dart:310`): o
            // prefixo de um import que não existe.
            if crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, cx.unit, Some(primeiro.sym), ultimo.sym) {
                return;
            }
            prefixo = true;
        } else if inf.program.lookup_na_unidade(cx.unit, primeiro.sym).is_some_and(|b| {
            !matches!(b.getter, Some(Element::Class(_)))
        }) || prefixo_de_import_nao_resolvido(inf, cx, primeiro.sym)
        {
            return;
        }
    }
    if name.len() == 1
        && inf.program.lookup_na_unidade(cx.unit, primeiro.sym).is_none()
        && crate::scope::deve_ignorar_indefinido(inf.program, inf.interner, cx.unit, None, primeiro.sym)
    {
        return;
    }
    let inicio = if prefixo { ultimo.span.start } else { primeiro.span.start };
    let texto = inf.interner.resolve(ultimo.sym).to_string();
    inf.aviso_com_codigo(
        dartforge_diagnostics::codigos::compile_time_error::NEW_WITH_NON_TYPE,
        Span { start: inicio, end: ultimo.span.end },
        &[&texto],
    );
}

/// `p` é o prefixo de alguma diretiva `import … as p` da biblioteca, mas
/// não chegou ao escopo (o alvo do import não existe).
fn prefixo_de_import_nao_resolvido(inf: &BodyInferrer<'_>, cx: &Corpo, p: dartforge_intern::SymbolId) -> bool {
    let lib = inf.program.library(cx.lib);
    !inf.program.prefixos_na_unidade(cx.unit).contains_key(&p)
        && lib.units.iter().any(|u| {
            inf.program.unit(*u).unit.directives.iter().any(|d| {
                matches!(&d.kind, ast::DirectiveKind::Import { prefix: Some(n), .. } if n.sym == p)
            })
        })
}

/// `_checkForConstWithUndefinedConstructor` (`error_verifier.dart:3020-3045`):
/// `const C.nome()` sem o construtor é `CONST_WITH_UNDEFINED_CONSTRUCTOR` no
/// nome (com o nome qualificado escrito e o do construtor); `const C()` sem
/// o sem nome, `CONST_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT` no tipo nomeado
/// inteiro.
fn construtor_constante_indefinido(
    inf: &mut BodyInferrer<'_>,
    a: &ast::Ast,
    ty: ast::TypeId,
    name: &[ast::Name],
    constructor: Option<ast::Name>,
    ctor_do_nome: bool,
) {
    construtor_indefinido(inf, a, ty, name, constructor, ctor_do_nome, true);
}

/// `_checkForConstWithUndefinedConstructor` e
/// `_checkForNewWithUndefinedConstructor` (`error_verifier.dart:4595-4625`):
/// no nome do construtor (`[namedType.qualifiedName, nome]`), ou, sem nome,
/// no `constructorName` inteiro, que é o tipo.
fn construtor_indefinido(
    inf: &mut BodyInferrer<'_>,
    a: &ast::Ast,
    ty: ast::TypeId,
    name: &[ast::Name],
    constructor: Option<ast::Name>,
    ctor_do_nome: bool,
    constante: bool,
) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let partes = if ctor_do_nome { &name[..1] } else { name };
    let qualificado = partes.iter().map(|n| inf.interner.resolve(n.sym)).collect::<Vec<_>>().join(".");
    match constructor {
        Some(n) => {
            let nome = inf.interner.resolve(n.sym).to_string();
            let codigo = if constante { c::CONST_WITH_UNDEFINED_CONSTRUCTOR } else { c::NEW_WITH_UNDEFINED_CONSTRUCTOR };
            inf.aviso_com_codigo(codigo, n.span, &[&qualificado, &nome]);
        }
        None => {
            let sp = a.ty(ty).span;
            let codigo = if constante { c::CONST_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT } else { c::NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT };
            inf.aviso_com_codigo(codigo, sp, &[&qualificado]);
        }
    }
}

/// `new C<T>.nome(args)` / `const C(args)`.
pub(crate) fn instanciacao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: ExprId, ctx: TypeId) -> TypeId {
    let a = &inf.program.unit(cx.unit).ast;
    let ExprKind::InstanceCreation { ty, constructor, arguments, keyword } = &a.expr(e).kind else { unreachable!() };
    let (ty, constructor, args): (ast::TypeId, Option<ast::Name>, &ast::Arguments) = (*ty, *constructor, arguments);
    let constante = matches!(keyword, Some(ast::CreationKeyword::Const));
    let node = a.ty(ty);
    let ast::TypeKind::Named { name, args: targs } = &node.kind else {
        for x in args.args.iter() {
            inferir_livre(inf, cx, x.value);
        }
        return inf.core.dynamic_;
    };
    let binding = if name.len() == 2 {
        inf.program.lookup_prefixed_na_unidade(cx.unit, name[0].sym, name[1].sym)
    } else {
        inf.program.lookup_na_unidade(cx.unit, name[0].sym)
    };
    // `new C.nome()` chega como tipo de duas partes quando `C` não é prefixo.
    let (binding, constructor, ctor_do_nome) = match (binding, name.len()) {
        (None, 2) => match inf.program.lookup_na_unidade(cx.unit, name[0].sym) {
            Some(b) if matches!(b.getter, Some(Element::Class(_))) && constructor.is_none() => (Some(b), Some(name[1]), true),
            _ => (binding, constructor, false),
        },
        _ => (binding, constructor, false),
    };
    // `_verifyTypeAliasForContext` (`named_type_resolver.dart:420-450`): o
    // alias cujo tipo é um parâmetro de tipo (`typedef A<T> = T`, também por
    // outro alias) não se instancia; no nome, sem os argumentos.
    {
        let alias_de_parametro = |inf: &BodyInferrer<'_>, el: Option<Element>| match el {
            Some(Element::Typedef(td)) => matches!(inf.table.get(inf.outline.typedefs[td.0 as usize].target_type), Type::TypeParameter { .. }),
            _ => false,
        };
        let faixa = if alias_de_parametro(inf, binding.and_then(|b| b.getter)) {
            name.first().zip(name.last()).map(|(p, n)| Span { start: p.span.start, end: n.span.end })
        } else if binding.is_none() && name.len() == 2 && alias_de_parametro(inf, inf.program.lookup_na_unidade(cx.unit, name[0].sym).and_then(|b| b.getter)) {
            Some(name[0].span)
        } else {
            None
        };
        if let Some(sp) = faixa {
            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER, sp, &[]);
            for x in args.args.iter() {
                inferir_livre(inf, cx, x.value);
            }
            return inf.core.dynamic_;
        }
    }
    let (c, explicitos) = match binding.and_then(|b| b.getter) {
        // `new C.nome<T>()`: lido como `prefixo.Tipo<T>`; a lista é do
        // construtor (`_rewriteToConstructorName`).
        Some(Element::Class(c)) if ctor_do_nome && !targs.is_empty() => {
            if let Some(n) = constructor {
                tipos_no_construtor(inf, cx.unit, name[0].span, n, targs);
            }
            (c, None)
        }
        Some(Element::Class(c)) => {
            let ex = if targs.is_empty() { None } else { Some(targs.iter().map(|&t| inf.tipo_de_argumento_de_tipo(cx, t)).collect::<Vec<_>>()) };
            (c, ex)
        }
        Some(Element::Typedef(_)) => {
            let t = inf.tipo_de_anotacao(cx, ty);
            match inf.table.get(t).clone() {
                Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => (class, Some(args.to_vec())),
                ref outro => {
                    // `typedef F = void Function()`: o alias não nomeia
                    // classe. Outros alvos (`typedef T = dynamic`, `typedef
                    // A<X> = X`, que é `instantiate_type_alias_expands_…`)
                    // ficam mudos.
                    if matches!(outro, Type::Function { .. }) {
                        criacao_sem_classe(inf, cx, name);
                    }
                    for x in args.args.iter() {
                        inferir_livre(inf, cx, x.value);
                    }
                    return inf.core.dynamic_;
                }
            }
        }
        _ => {
            if !binding.is_some_and(|b| b.ambiguous) {
                criacao_sem_classe(inf, cx, name);
            }
            for x in args.args.iter() {
                inferir_livre(inf, cx, x.value);
            }
            return inf.core.dynamic_;
        }
    };
    let chave = match constructor {
        Some(n) if Some(n.sym) != inf.sym.new_ => Some(n.sym),
        _ => inf.sym.vazio,
    };
    let f = chave.and_then(|k| inf.construtor_ou_primario(c, k));
    // `new E()` / `const E()`: o construtor gerador do enum (ou o sem nome
    // implícito) não instancia fora da criação de constantes. Nome
    // explícito sem alvo (`const E.foo()`) é outro diagnóstico
    // (`const_with_undefined_constructor`, fora do escopo) e segue mudo.
    // Factories seguem o caminho normal (`construir`). O intervalo é o nome
    // do construtor, ou o da classe quando ele é implícito.
    // `new M()` / `const M()` com `M` mixin (`error_verifier.dart:2976-2984`):
    // no tipo nomeado; o construtor que o mixin não tem fica mudo.
    if inf.program.class(c).kind == ClassKind::Mixin {
        let sp = if constructor.is_some() && ctor_do_nome { name[0].span } else { a.ty(ty).span };
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::MIXIN_INSTANTIATE, sp, &[]);
        if constante {
            construtor_constante_indefinido(inf, a, ty, name, constructor, ctor_do_nome);
        }
        for x in args.args.iter() {
            inferir_livre(inf, cx, x.value);
        }
        return inf.tipo_de_classe_com_args(c, explicitos.unwrap_or_default());
    }
    let gerador = f.is_some_and(|f| f.is_some_and(|f| !inf.program.function(f).factory));
    if inf.program.class(c).kind == ClassKind::Enum && (gerador || (constructor.is_none() && f.is_none()))
    {
        let span = constructor.map(|n| n.span).unwrap_or_else(|| name.last().map(|n| n.span).unwrap_or(a.expr(e).span));
        inf.aviso(INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template.to_string(), span);
        for x in args.args.iter() {
            inferir_livre(inf, cx, x.value);
        }
        return inf.core.dynamic_;
    }
    let Some(f) = f else {
        // Sem construtor declarado, o sem nome implícito existe (o elemento
        // sintético): nada a relatar por ele.
        let implicito = constructor.is_none() && inf.program.class(c).constructors.is_empty();
        if !implicito {
            construtor_indefinido(inf, a, ty, name, constructor, ctor_do_nome, constante);
        }
        // Sem construtor declarado: o padrão implícito (gerador).
        if constructor.is_none() && inf.program.class(c).constructors.is_empty() {
            let k = inf.program.class(c);
            if matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) && (k.modifiers.abstract_ || k.modifiers.sealed) {
                let sp = a.ty(ty).span;
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, sp, &[]);
            }
        }
        for x in args.args.iter() {
            inferir_livre(inf, cx, x.value);
        }
        return inf.tipo_de_classe_com_args(c, explicitos.unwrap_or_default());
    };
    if let Some(fid) = f {
        // O tipo nomeado: sem o nome do construtor que veio como 2ª parte.
        let sp = if constructor.is_some() && ctor_do_nome {
            name[0].span
        } else {
            a.ty(ty).span
        };
        avisar_classe_abstrata(inf, c, fid, sp);
    }
    // `constructorName`: o tipo (com prefixo e argumentos) e o `.nome`; o
    // nome citado é o do construtor, ou `C.new` (sem o prefixo).
    {
        let classe = if ctor_do_nome { name[0] } else { *name.last().unwrap_or(&name[0]) };
        let nome = match constructor {
            Some(n) => inf.interner.resolve(n.sym).to_string(),
            None => format!("{}.new", inf.interner.resolve(classe.sym)),
        };
        let t = a.ty(ty).span;
        let ent = Span { start: t.start, end: constructor.map(|n| n.span.end).unwrap_or(t.end).max(t.end) };
        definir_alvo(inf, Some(nome), ent);
    }
    let r = construir(inf, cx, Some(e), c, f, explicitos, args, ctx);
    inf.alvo_da_aridade = None;
    r
}

/// O intervalo do tipo nomeado de uma criação implícita: `A`, `p.A` ou
/// `A<int>` (até o `>`).
fn tipo_nomeado_da_criacao(inf: &BodyInferrer<'_>, cx: &Corpo, alvo: ExprId, targs: &[ast::TypeId]) -> Span {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.expr(alvo).kind {
        ExprKind::TypeArguments { target, type_args } => {
            let ini = a.expr(*target).span.start;
            let fim = type_args.last().map(|&t| a.ty(t).span.end).unwrap_or(a.expr(*target).span.end);
            let fonte = &inf.program.unit(cx.unit).source;
            let resto = fonte.get(fim..).unwrap_or("");
            let fecha = resto.find('>').map(|i| fim + i + 1).unwrap_or(fim);
            Span { start: ini, end: fecha }
        }
        _ => {
            let sp = a.expr(alvo).span;
            // `A<int>()`: os argumentos de tipo vêm na lista da chamada.
            match targs.last() {
                Some(&t) => {
                    let fim = a.ty(t).span.end;
                    let resto = inf.program.unit(cx.unit).source.get(fim..).unwrap_or("");
                    let fecha = resto.find('>').map(|i| fim + i + 1).unwrap_or(fim);
                    Span { start: sp.start, end: fecha }
                }
                None => sp,
            }
        }
    }
}

/// `INSTANTIATE_ABSTRACT_CLASS` (`_checkForConstOrNewWithAbstractClass`,
/// `an611:src/generated/error_verifier.dart:2950-2973`): criação de uma
/// classe abstrata (ou `sealed`) por um construtor que não é `factory`, no
/// tipo nomeado.
fn avisar_classe_abstrata(inf: &mut BodyInferrer<'_>, c: ClassId, f: FunctionElementId, span: Span) {
    let k = inf.program.class(c);
    if !matches!(k.kind, ClassKind::Class | ClassKind::MixinApplication) || !(k.modifiers.abstract_ || k.modifiers.sealed) {
        return;
    }
    if inf.program.function(f).factory {
        return;
    }
    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::INSTANTIATE_ABSTRACT_CLASS, span, &[]);
}
