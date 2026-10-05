//! O estado do casamento de padrões no fluxo (docs/ANALYZER-ESPECIFICACAO.md
//! §G, T6): o porte de `_pushScrutinee`/`_pushPattern`/`_popPattern`,
//! `promoteForPattern`, `_nullCheckPattern`, `_handleEqualityCheckPattern` e
//! dos `logicalOrPattern_*` do `_FlowAnalysisImpl`
//! (`_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart`), com os
//! chamadores do `TypeAnalyzer` (`type_inference/type_analyzer.dart`).
//!
//! O valor casado é uma **referência temporária** com chave de promoção
//! própria: aqui, um local sintético do corpo, cujo modelo vive no
//! [`Fluxo`] corrente como o de qualquer variável. O tipo casado de um
//! padrão é o tipo promovido dessa chave na entrada do padrão; promover a
//! chave promove também o escrutínio, quando ele é uma referência que a
//! construção deixa promover. O estado "não casou" é acumulado à parte
//! ([`Casamento::nao_casou`]) e começa inalcançável: ele só fica alcançável
//! quando algum padrão pode falhar.
//!
//! Só o analisador usa este caminho (ver [`ligado`]); os compiladores ficam
//! com a tipagem antiga de `super::tipar`.

use super::super::corpo::{Corpo, Local, Nome};
use super::super::expr::{self, declarar_local, inferir};
use super::super::fluxo::Fluxo;
use super::super::membros::Busca;
use super::super::BodyInferrer;
use super::{
    anotacao_invalida, campo_nomeado_implicito, constante_nunca_casa, nome_implicito, nunca_casa, registrar_tipo_de_padrao,
    tipo_do_padrao_objeto,
};
use crate::resolved::LocalId;
use crate::table::{Type, TypeId};
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, ExprId, ListPatternElement, PatternId, PatternKind};

/// A referência do valor casado de um padrão (ou subpadrão).
#[derive(Debug, Clone)]
pub(crate) struct RefCasada {
    /// O local sintético que carrega as promoções do valor casado.
    pub chave: LocalId,
    /// O tipo com que a referência foi criada.
    pub tipo: TypeId,
    /// O escrutínio, quando é uma referência promovível.
    pub alvo: Option<LocalId>,
    /// O escrutínio é uma propriedade promovível (promove em qualquer
    /// construção); senão é variável e só promove enquanto não for reescrita.
    pub propriedade: bool,
    /// A versão de escrita da variável escrutinada no começo do casamento.
    pub versao: Option<u32>,
}

/// O casamento em curso de um padrão de topo.
#[derive(Debug, Clone)]
pub(crate) struct Casamento {
    /// `_unmatched`.
    pub nao_casou: Fluxo,
    /// A referência do padrão de topo e as dos subpadrões abertos.
    pub pilha: Vec<RefCasada>,
}

/// O fluxo de padrões do analyzer vale só no modo do analisador (o mesmo
/// que preserva a exibição dos tipos): ele muda o tipo promovido visto por
/// todo corpo com `switch` ou `if-case`, e os emissores JS e nativo
/// consomem as promoções (T6.7).
pub(crate) fn ligado(inf: &BodyInferrer<'_>) -> bool {
    inf.table.preservar_exibicao
}

fn padroes_da_linguagem(inf: &BodyInferrer<'_>, cx: &Corpo) -> bool {
    inf.program.library(cx.lib).features.versao() >= dartforge_frontend::features::LanguageVersion::new(3, 0)
}

/// Um local sintético para a chave de promoção de um valor casado.
pub(crate) fn nova_chave(inf: &BodyInferrer<'_>, cx: &mut Corpo, tipo: TypeId) -> LocalId {
    let nome = match inf.sym.vazio.or(inf.sym.new_) {
        Some(s) => s,
        None => inf.program.classes[0].name,
    };
    cx.declarar_sintetico(Local { nome, tipo, final_: true, late: false, const_: false, offset: 0, funcao_local: false })
}

/// Casa com a referência `r` o que `f` tipa e devolve o estado "não casou".
pub(crate) fn casar(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    r: RefCasada,
    f: impl FnOnce(&mut BodyInferrer<'_>, &mut Corpo),
) -> Fluxo {
    let nao_casou = cx.fluxo.inalcancavel();
    let salvo = cx.casamento.replace(Casamento { nao_casou, pilha: vec![r] });
    f(inf, cx);
    let fim = std::mem::replace(&mut cx.casamento, salvo);
    match fim {
        Some(c) => c.nao_casou,
        None => cx.fluxo.inalcancavel(),
    }
}

fn topo(cx: &Corpo) -> Option<RefCasada> {
    cx.casamento.as_ref().and_then(|c| c.pilha.last().cloned())
}

/// `_getMatchedValueType`: o tipo promovido da chave do topo.
fn casado(cx: &Corpo, r: &RefCasada) -> TypeId {
    cx.fluxo.tipo_atual(r.chave, r.tipo)
}

/// `_unmatched = _join(_unmatched, extra)`.
fn juntar_nao_casou(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, extra: &Fluxo) {
    let Some(velho) = cx.casamento.as_ref().map(|c| c.nao_casou.clone()) else { return };
    let novo = inf.juntar(&velho, extra);
    if let Some(c) = cx.casamento.as_mut() {
        c.nao_casou = novo;
    }
}

/// O escrutínio a promover junto com a chave: a propriedade, sempre; a
/// variável, só se a construção deixa e ela não foi reescrita desde o
/// começo do casamento (`promoteForPattern`, `:5227-5235`).
fn alvo_valido(cx: &Corpo, r: &RefCasada) -> Option<LocalId> {
    let id = r.alvo?;
    if r.propriedade || cx.fluxo.versao(id) == r.versao {
        Some(id)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Classe {
    NaoNulo,
    Nulo,
    Talvez,
}

/// `classifyType` (`flow_analysis_visitor.dart:446-456`).
fn classificar(inf: &mut BodyInferrer<'_>, t: TypeId) -> Classe {
    let (objeto, nulo) = (inf.core.object, inf.core.null);
    if inf.sub(t, objeto) {
        Classe::NaoNulo
    } else if inf.sub(t, nulo) {
        Classe::Nulo
    } else {
        Classe::Talvez
    }
}

/// O ramo verdadeiro de `tryPromoteForTypeCheck`. Promover nunca torna o
/// fluxo inalcançável, nem para `Never`.
fn promover_sim(inf: &mut BodyInferrer<'_>, f: &mut Fluxo, id: LocalId, declarado: TypeId, k: TypeId) {
    let alcancavel = f.alcancavel;
    inf.promover_testado(f, id, declarado, k, k);
    f.alcancavel = alcancavel;
}

/// O ramo falso: promove para `factor(atual, K)`, salvo quando o fator é
/// `Never` ou o próprio tipo atual (aí só o tipo de interesse fica).
fn promover_nao(inf: &mut BodyInferrer<'_>, f: &mut Fluxo, id: LocalId, declarado: TypeId, k: TypeId) {
    let alcancavel = f.alcancavel;
    let s = f.tipo_atual(id, declarado);
    let fat = expr::fator(inf, s, k);
    let alvo = if fat == s || matches!(inf.table.get(fat), Type::Never) { s } else { fat };
    inf.promover_testado(f, id, declarado, alvo, k);
    f.alcancavel = alcancavel;
}

/// `promoteForPattern` (`:5191-5258`): devolve se `K` cobre o tipo casado.
fn promover_para_padrao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, k: TypeId, falha: bool, pode: bool) -> bool {
    let Some(r) = topo(cx) else { return true };
    let t = casado(cx, &r);
    let cobre = inf.sub(t, k);
    let mut sim = cx.fluxo.clone();
    let mut nao = cx.fluxo.clone();
    promover_sim(inf, &mut sim, r.chave, r.tipo, k);
    promover_nao(inf, &mut nao, r.chave, r.tipo, k);
    if let Some(id) = alvo_valido(cx, &r) {
        let d = cx.local(id).tipo;
        promover_sim(inf, &mut sim, id, d, k);
        promover_nao(inf, &mut nao, id, d, k);
    }
    cx.fluxo = sim;
    if pode || (falha && !cobre) {
        let extra = if cobre { cx.fluxo.clone() } else { nao };
        juntar_nao_casou(inf, cx, &extra);
    }
    cobre
}

/// `_nullCheckPattern` (`:6019-6032`): o estado "não é nulo", ou `None`
/// quando o tipo casado já é não anulável (nada a checar).
fn checar_nulo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo) -> Option<Fluxo> {
    let r = topo(cx)?;
    let t = casado(cx, &r);
    let classe = classificar(inf, t);
    if classe == Classe::NaoNulo {
        return None;
    }
    let nn = inf.nao_nulo_promocao(t);
    let mut sim = cx.fluxo.clone();
    promover_sim(inf, &mut sim, r.chave, r.tipo, nn);
    if let Some(id) = alvo_valido(cx, &r) {
        let d = cx.local(id).tipo;
        let atual = sim.tipo_atual(id, d);
        let nn_alvo = inf.nao_nulo_promocao(atual);
        promover_sim(inf, &mut sim, id, d, nn_alvo);
    }
    if classe == Classe::Nulo {
        sim.alcancavel = false;
    }
    Some(sim)
}

/// `_handleEqualityCheckPattern` (`:5832-5903`): padrão constante (`==`) e
/// relacional `==`/`!=` contra `operando`, de tipo `tipo_operando`.
fn igualdade(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, operando: ExprId, tipo_operando: TypeId, diferente: bool) {
    let Some(r) = topo(cx) else { return };
    let t = casado(cx, &r);
    let a = &inf.program.unit(cx.unit).ast;
    let mut x = operando;
    while let ast::ExprKind::Parenthesized(i) = &a.expr(x).kind {
        x = *i;
    }
    let literal_nulo = matches!(a.expr(x).kind, ast::ExprKind::Null);
    let (ct, co) = (classificar(inf, t), classificar(inf, tipo_operando));
    let atual = cx.fluxo.clone();
    if ct == Classe::Nulo && co == Classe::Nulo {
        // Iguais com certeza: `==` sempre casa, `!=` nunca.
        if diferente {
            juntar_nao_casou(inf, cx, &atual);
            cx.fluxo.alcancavel = false;
        }
    } else if (ct == Classe::Nulo && co == Classe::NaoNulo) || (co == Classe::Nulo && ct == Classe::NaoNulo) {
        // Sem informação (no modo fraco poderiam ser iguais).
        juntar_nao_casou(inf, cx, &atual);
    } else if literal_nulo {
        // A comparação com `null` é uma checagem de nulo do valor casado.
        let nao_nulo = checar_nulo(inf, cx).unwrap_or_else(|| atual.clone());
        if diferente {
            juntar_nao_casou(inf, cx, &atual);
            cx.fluxo = nao_nulo;
        } else {
            juntar_nao_casou(inf, cx, &nao_nulo);
        }
    } else {
        juntar_nao_casou(inf, cx, &atual);
    }
}

/// `pushSubpattern(tipo)` … `popSubpattern`: `f` tipa o subpadrão contra uma
/// referência nova de tipo `tipo`; devolve o tipo demonstrado (o promovido
/// da referência depois do subpadrão).
fn com_subpadrao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, tipo: TypeId, f: impl FnOnce(&mut BodyInferrer<'_>, &mut Corpo)) -> TypeId {
    let chave = nova_chave(inf, cx, tipo);
    if let Some(c) = cx.casamento.as_mut() {
        c.pilha.push(RefCasada { chave, tipo, alvo: None, propriedade: false, versao: None });
    }
    f(inf, cx);
    let demonstrado = cx.fluxo.tipo_atual(chave, tipo);
    if let Some(c) = cx.casamento.as_mut() {
        c.pilha.pop();
    }
    demonstrado
}

/// A palavra `as` de um padrão de cast, entre o subpadrão e o tipo.
fn palavra_as(inf: &BodyInferrer<'_>, cx: &Corpo, interno: PatternId, ty: ast::TypeId) -> Option<Span> {
    let a = &inf.program.unit(cx.unit).ast;
    let (ini, fim) = (a.pattern(interno).span.end, a.ty(ty).span.start);
    let fonte = inf.program.unit(cx.unit).source.as_bytes();
    let trecho = fonte.get(ini..fim)?;
    let mut i = 0;
    while i + 2 <= trecho.len() {
        let antes_ok = i == 0 || !(trecho[i - 1].is_ascii_alphanumeric() || trecho[i - 1] == b'_');
        let depois_ok = trecho.get(i + 2).is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
        if &trecho[i..i + 2] == b"as" && antes_ok && depois_ok {
            return Some(Span { start: ini + i, end: ini + i + 2 });
        }
        i += 1;
    }
    None
}

/// Tipa o padrão `p` contra o valor casado do topo da pilha, declarando as
/// variáveis. `de_e`: `p` é operando direto de `&&` (o único lugar em que um
/// curinga que sempre casa é relatado como desnecessário).
pub(crate) fn tipar(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, p: PatternId, final_: bool, atribuicao: bool, de_e: bool) {
    use dartforge_diagnostics::codigos::{static_warning as sw, warning as w};
    let a = &inf.program.unit(cx.unit).ast;
    let u = inf.core.unknown;
    let Some(r) = topo(cx) else { return };
    // O tipo casado é o da ENTRADA do padrão, antes de ele mesmo promover.
    let t = casado(cx, &r);
    inf.body_types.units[cx.unit.0 as usize].tipos_casados.insert(p, t);
    let span_do_padrao = a.pattern(p).span;
    match &a.pattern(p).kind {
        PatternKind::Wildcard { ty: Some(x) } => {
            let x = *x;
            let k = inf.tipo_de_anotacao(cx, x);
            let inv = anotacao_invalida(inf, cx, x, k);
            registrar_tipo_de_padrao(inf, cx, p, k, inv);
            let sp = inf.program.unit(cx.unit).ast.ty(x).span;
            nunca_casa(inf, cx, t, k, sp);
            let cobre = promover_para_padrao(inf, cx, k, true, false);
            if cobre && de_e {
                inf.aviso_com_codigo(w::UNNECESSARY_WILDCARD_PATTERN, span_do_padrao, &[]);
            }
        }
        PatternKind::Wildcard { ty: None } => {
            if de_e {
                inf.aviso_com_codigo(w::UNNECESSARY_WILDCARD_PATTERN, span_do_padrao, &[]);
            }
        }
        PatternKind::Variable { final_: f2, var_, ty, name } => {
            let (f2, ty, name) = (*f2, *ty, *name);
            if cx.padrao_refutavel && !*var_ && !f2 && ty.is_none() {
                // `case limite:` é o padrão constante de uma `const`: pode
                // casar ou não, sem informação.
                let atual = cx.fluxo.clone();
                juntar_nao_casou(inf, cx, &atual);
                return;
            }
            if atribuicao {
                if let Some(Nome::Local(id)) = cx.buscar(name.sym) {
                    let decl = cx.local(id).tipo;
                    promover_para_padrao(inf, cx, decl, true, false);
                    let escrito = casado(cx, &r);
                    let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                    inf.atribuir_fluxo(&mut f, id, decl, escrito);
                    cx.fluxo = f;
                }
                return;
            }
            let k = match ty {
                Some(x) => {
                    let k = inf.tipo_de_anotacao(cx, x);
                    nunca_casa(inf, cx, t, k, inf.program.unit(cx.unit).ast.ty(x).span);
                    k
                }
                // `variableTypeFromInitializerType`: um valor `Null`
                // (inclusive `Never?`) declara a variável como `dynamic`.
                None => {
                    if classificar(inf, t) == Classe::Nulo {
                        inf.core.dynamic_
                    } else {
                        t
                    }
                }
            };
            promover_para_padrao(inf, cx, k, true, false);
            let inv = ty.is_some_and(|x| anotacao_invalida(inf, cx, x, k));
            registrar_tipo_de_padrao(inf, cx, p, k, inv);
            declarar_local(
                inf,
                cx,
                Local { nome: name.sym, tipo: k, final_: final_ || f2, late: false, const_: false, offset: name.span.start, funcao_local: false },
                true,
            );
        }
        PatternKind::Constant(e) => {
            let e = *e;
            let c = inferir(inf, cx, e, t);
            constante_nunca_casa(inf, cx, p, e, c, t);
            if padroes_da_linguagem(inf, cx) {
                igualdade(inf, cx, e, c, false);
            } else {
                // Sem padrões (< 3.0): todo `case` é alcançável e nada é
                // promovido (`constantPattern_end`, `:4489-4505`).
                let atual = cx.fluxo.clone();
                juntar_nao_casou(inf, cx, &atual);
            }
        }
        PatternKind::Relational { op, value } => {
            let (op, value) = (*op, *value);
            let nome = match op {
                ast::BinaryOp::Eq | ast::BinaryOp::NotEq => "==",
                ast::BinaryOp::Lt => "<",
                ast::BinaryOp::Gt => ">",
                ast::BinaryOp::LtEq => "<=",
                ast::BinaryOp::GtEq => ">=",
                _ => "",
            };
            let ctx = match inf.interner.lookup(nome).map(|s| inf.buscar_membro(cx.lib, t, s, false)) {
                Some(Busca::Achado(m)) => match inf.table.get(m.tipo) {
                    Type::Function { positional, .. } => positional.first().copied().unwrap_or(u),
                    _ => u,
                },
                _ => u,
            };
            let tv = inferir(inf, cx, value, ctx);
            match op {
                ast::BinaryOp::Eq => igualdade(inf, cx, value, tv, false),
                ast::BinaryOp::NotEq => igualdade(inf, cx, value, tv, true),
                _ => {
                    let atual = cx.fluxo.clone();
                    juntar_nao_casou(inf, cx, &atual);
                }
            }
        }
        PatternKind::Or(x, y) => {
            let (x, y) = (*x, *y);
            // `logicalOrPattern_begin`: o operando esquerdo tem o seu
            // próprio "não casou", que é a entrada do direito.
            let vazio = cx.fluxo.inalcancavel();
            let previo = cx.casamento.as_mut().map(|c| std::mem::replace(&mut c.nao_casou, vazio));
            tipar(inf, cx, x, final_, atribuicao, false);
            let casou_esquerdo = cx.fluxo.clone();
            let falhou = match (cx.casamento.as_mut(), previo) {
                (Some(c), Some(previo)) => std::mem::replace(&mut c.nao_casou, previo),
                _ => cx.fluxo.clone(),
            };
            cx.fluxo = falhou;
            // O operando direito de um `||` cujo esquerdo sempre casa é
            // código morto (Apêndice B do §B).
            // O trecho começa no operador `||` e vai ao fim do operando
            // (`dead_code_verifier.dart:321-324`).
            let (fim_x, sp_y) = (inf.program.unit(cx.unit).ast.pattern(x).span.end, inf.program.unit(cx.unit).ast.pattern(y).span);
            super::super::instrucoes::entrar_fluxo(cx, sp_y.end);
            if !cx.fluxo.alcancavel && cx.trecho_morto.is_none() {
                let fonte = inf.program.unit(cx.unit).source.as_bytes();
                let operador = fonte
                    .get(fim_x..sp_y.start)
                    .and_then(|trecho| trecho.windows(2).position(|w| w == b"||"))
                    .map_or(sp_y.start, |i| fim_x + i);
                inf.aviso(crate::codes::DEAD_CODE.template.to_string(), Span { start: operador, end: sp_y.end });
                cx.trecho_morto = Some(cx.fins_de_fluxo.len());
            }
            tipar(inf, cx, y, final_, atribuicao, false);
            super::super::instrucoes::sair_fluxo(cx);
            let casou_direito = cx.fluxo.clone();
            cx.fluxo = inf.juntar(&casou_esquerdo, &casou_direito);
        }
        PatternKind::And(x, y) => {
            let (x, y) = (*x, *y);
            tipar(inf, cx, x, final_, atribuicao, true);
            tipar(inf, cx, y, final_, atribuicao, true);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) => {
            let x = *x;
            let afirmacao = matches!(a.pattern(p).kind, PatternKind::NullAssert(_));
            match checar_nulo(inf, cx) {
                Some(nao_nulo) => {
                    if !afirmacao {
                        let atual = cx.fluxo.clone();
                        juntar_nao_casou(inf, cx, &atual);
                    }
                    cx.fluxo = nao_nulo;
                }
                None => {
                    // O valor casado não pode ser nulo: o `?`/`!` não faz
                    // nada. O `?` num contexto irrefutável já é outro erro.
                    if afirmacao || cx.padrao_refutavel {
                        let token = Span { start: span_do_padrao.end.saturating_sub(1), end: span_do_padrao.end };
                        let codigo = if afirmacao { sw::UNNECESSARY_NULL_ASSERT_PATTERN } else { sw::UNNECESSARY_NULL_CHECK_PATTERN };
                        inf.aviso_com_codigo(codigo, token, &[]);
                    }
                }
            }
            tipar(inf, cx, x, final_, atribuicao, false);
        }
        PatternKind::Parenthesized(x) => {
            let x = *x;
            tipar(inf, cx, x, final_, atribuicao, false);
        }
        PatternKind::Cast { pattern, ty } => {
            let (pattern, ty) = (*pattern, *ty);
            let c = inf.tipo_de_anotacao(cx, ty);
            let inv = anotacao_invalida(inf, cx, ty, c);
            registrar_tipo_de_padrao(inf, cx, p, c, inv);
            let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
            nunca_casa(inf, cx, t, c, sp);
            if !inv && !inf.table.e_invalido(c) && inf.sub(t, c) {
                if let Some(palavra) = palavra_as(inf, cx, pattern, ty) {
                    inf.aviso_com_codigo(w::UNNECESSARY_CAST_PATTERN, palavra, &[]);
                }
            }
            if inf.e_nao_anulavel(c) && classificar(inf, t) == Classe::Nulo {
                inf.aviso_com_codigo(w::CAST_FROM_NULL_ALWAYS_FAILS, span_do_padrao, &[]);
            }
            // O cast não falha por tipo errado (lança): nada vai ao "não casou".
            promover_para_padrao(inf, cx, c, false, false);
            com_subpadrao(inf, cx, c, |inf, cx| tipar(inf, cx, pattern, final_, atribuicao, false));
        }
        PatternKind::List { type_args, elements } => {
            let el = if let Some(x) = type_args.first() {
                inf.tipo_de_argumento_de_tipo(cx, *x)
            } else if inf.e_dynamic(t) {
                // `dynamic` e o inválido dão elementos do mesmo tipo.
                t
            } else {
                inf.como_instancia_de(t, inf.core.list_class).map(|a| a[0]).unwrap_or(inf.core.object_nullable)
            };
            let requerido = inf.lista(el);
            {
                let inv = type_args.first().is_some_and(|&x| anotacao_invalida(inf, cx, x, el));
                registrar_tipo_de_padrao(inf, cx, p, requerido, inv);
                nunca_casa(inf, cx, t, requerido, span_do_padrao);
            }
            // Só `[...]` (um elemento, e ele é o resto) casa com toda lista.
            let so_resto = elements.len() == 1 && matches!(elements[0], ListPatternElement::Rest(_));
            promover_para_padrao(inf, cx, requerido, true, !so_resto);
            for e in elements.iter() {
                match e {
                    ListPatternElement::Pattern(x) => {
                        let x = *x;
                        com_subpadrao(inf, cx, el, |inf, cx| tipar(inf, cx, x, final_, atribuicao, false));
                    }
                    ListPatternElement::Rest(Some(x)) => {
                        let x = *x;
                        com_subpadrao(inf, cx, requerido, |inf, cx| tipar(inf, cx, x, final_, atribuicao, false));
                    }
                    ListPatternElement::Rest(None) => {}
                }
            }
        }
        PatternKind::Map { type_args, entries, .. } => {
            let (k, v) = if type_args.len() == 2 {
                (inf.tipo_de_argumento_de_tipo(cx, type_args[0]), inf.tipo_de_argumento_de_tipo(cx, type_args[1]))
            } else if inf.e_dynamic(t) {
                (t, t)
            } else {
                match inf.como_instancia_de(t, inf.core.map_class) {
                    Some(a) => (a[0], a[1]),
                    None => (inf.core.object_nullable, inf.core.object_nullable),
                }
            };
            match inf.core.map_class {
                Some(mc) => {
                    let requerido = inf.table.intern(Type::Interface { class: mc, args: vec![k, v].into_boxed_slice(), nullable: false });
                    let inv = type_args.len() == 2 && (anotacao_invalida(inf, cx, type_args[0], k) || anotacao_invalida(inf, cx, type_args[1], v));
                    registrar_tipo_de_padrao(inf, cx, p, requerido, inv);
                    // Um mapa pode não ter a chave: falha mesmo com o tipo certo.
                    promover_para_padrao(inf, cx, requerido, true, true);
                }
                None => {
                    let atual = cx.fluxo.clone();
                    juntar_nao_casou(inf, cx, &atual);
                }
            }
            for en in entries.iter() {
                let (key, val) = (en.key, en.value);
                inferir(inf, cx, key, k);
                com_subpadrao(inf, cx, v, |inf, cx| tipar(inf, cx, val, final_, atribuicao, false));
            }
        }
        PatternKind::Record { fields } => {
            let campos: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            // A forma do registro: `(Object?, …, {Object? n})`.
            let nomes: Vec<Option<dartforge_intern::SymbolId>> = campos
                .iter()
                .map(|&(n, x)| n.map(|n| n.sym).or_else(|| if campo_nomeado_implicito(inf, cx, p, x) { nome_implicito(inf, cx, x) } else { None }))
                .collect();
            let topo_dos_campos = inf.core.object_nullable;
            let montar = |inf: &mut BodyInferrer<'_>, tipos: &[TypeId]| -> TypeId {
                let mut pos = Vec::new();
                let mut nm = Vec::new();
                for (nome, &tipo) in nomes.iter().zip(tipos.iter()) {
                    match nome {
                        Some(s) => nm.push((*s, tipo)),
                        None => pos.push(tipo),
                    }
                }
                inf.table.intern(Type::Record { positional: pos.into_boxed_slice(), named: nm.into_boxed_slice(), nullable: false })
            };
            let requerido = montar(inf, &vec![topo_dos_campos; campos.len()]);
            promover_para_padrao(inf, cx, requerido, true, false);
            // Os tipos dos campos vêm do tipo casado já promovido.
            let promovido = casado(cx, &r);
            let rec = match inf.table.get(promovido).clone() {
                Type::Record { positional, named, .. } => Some((positional, named)),
                _ => None,
            };
            let mut ipos = 0;
            let mut demonstrados: Vec<TypeId> = Vec::with_capacity(campos.len());
            for (i, &(_, x)) in campos.iter().enumerate() {
                let ft = match (&rec, nomes[i]) {
                    (Some((_, named)), Some(nm)) => named.iter().find(|(s, _)| *s == nm).map(|(_, t)| *t),
                    (Some((pos, _)), None) => {
                        let achado = pos.get(ipos).copied();
                        ipos += 1;
                        achado
                    }
                    _ => None,
                };
                let ft = ft.unwrap_or(if inf.e_dynamic(t) { t } else { topo_dos_campos });
                let d = com_subpadrao(inf, cx, ft, |inf, cx| tipar(inf, cx, x, final_, atribuicao, false));
                demonstrados.push(d);
            }
            // A segunda promoção, para o registro dos tipos demonstrados
            // pelos subpadrões; não falha por tipo.
            let demonstrado = montar(inf, &demonstrados);
            promover_para_padrao(inf, cx, demonstrado, false, false);
        }
        PatternKind::Object { ty, fields } => {
            let ty = *ty;
            let campos: Vec<(Option<ast::Name>, PatternId)> = fields.iter().map(|f| (f.name, f.pattern)).collect();
            let obj = tipo_do_padrao_objeto(inf, cx, ty, t);
            let inv = anotacao_invalida(inf, cx, ty, obj);
            registrar_tipo_de_padrao(inf, cx, p, obj, inv);
            {
                let sp = inf.program.unit(cx.unit).ast.ty(ty).span;
                nunca_casa(inf, cx, t, obj, sp);
            }
            promover_para_padrao(inf, cx, obj, true, false);
            for (n, x) in campos {
                let nome = n.map(|n| n.sym).or_else(|| nome_implicito(inf, cx, x));
                let ft = match nome {
                    Some(nm) => match inf.buscar_membro(cx.lib, obj, nm, false) {
                        Busca::Achado(m) => {
                            let de_tipo_de_extensao = matches!(
                                &m.resolved,
                                crate::resolved::Resolved::Member { class, .. }
                                    if inf.program.class(*class).kind == dartforge_elements::model::ClassKind::ExtensionType
                            );
                            if m.de_extensao || de_tipo_de_extensao {
                                inf.body_types.units[cx.unit.0 as usize].campos_de_extensao.insert(x, m.tipo);
                            }
                            m.tipo
                        }
                        Busca::Nunca => inf.core.never,
                        _ => inf.core.dynamic_,
                    },
                    None => inf.core.dynamic_,
                };
                // Getter `Never`: a leitura não volta (`flow.handleExit()`).
                if matches!(inf.table.get(ft), Type::Never) {
                    cx.fluxo.alcancavel = false;
                }
                com_subpadrao(inf, cx, ft, |inf, cx| tipar(inf, cx, x, final_, atribuicao, false));
            }
        }
    }
}

/// `case p when g` com o fluxo de padrões: `(casou, não casou)`.
pub(crate) fn caso(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    p: PatternId,
    t: TypeId,
    guarda: Option<ExprId>,
    escrutinio: Option<ExprId>,
) -> (Fluxo, Fluxo) {
    // O escrutínio é resolvido no escopo de fora, antes de o padrão
    // declarar as suas variáveis.
    let alvo = escrutinio.and_then(|e| expr::alvo_de_promocao(inf, cx, e));
    let propriedade = alvo.is_some_and(|id| cx.campos.values().any(|&v| v == id));
    // Num `switch`, todos os casos casam a MESMA referência: as promoções
    // do "não casou" de um caso chegam ao seguinte.
    let reuso = match (cx.escrutinio_de_switch, escrutinio) {
        (Some(Some((e0, chave, versao))), Some(e)) if e0 == e && cx.fluxo.modelo(chave).is_some() => Some((chave, versao)),
        _ => None,
    };
    let (chave, versao) = match reuso {
        Some(x) => x,
        None => {
            let chave = nova_chave(inf, cx, t);
            let versao = alvo.and_then(|id| cx.fluxo.versao(id));
            if cx.escrutinio_de_switch.is_some() {
                if let Some(e) = escrutinio {
                    cx.escrutinio_de_switch = Some(Some((e, chave, versao)));
                }
            }
            (chave, versao)
        }
    };
    let r = RefCasada { chave, tipo: t, alvo, propriedade, versao };
    let refutavel_antes = std::mem::replace(&mut cx.padrao_refutavel, true);
    let mut nao = casar(inf, cx, r, |inf, cx| tipar(inf, cx, p, false, false, false));
    cx.padrao_refutavel = refutavel_antes;
    if let Some(g) = guarda {
        let (gv, gf) = expr::condicao_verificada(inf, cx, g);
        cx.fluxo = gv;
        nao = inf.juntar(&nao, &gf);
    }
    (cx.fluxo.clone(), nao)
}

/// Declaração, atribuição e `for-in` de padrão: o mesmo casamento, sem
/// promover variável escrutinada (`allowScrutineePromotion: false`) e com o
/// "não casou" descartado. `valor`: o inicializador, quando há — uma
/// propriedade promovível é promovida em qualquer construção.
pub(crate) fn irrefutavel(
    inf: &mut BodyInferrer<'_>,
    cx: &mut Corpo,
    p: PatternId,
    t: TypeId,
    final_: bool,
    atribuicao: bool,
    valor: Option<ExprId>,
) {
    let alvo = valor
        .and_then(|e| expr::alvo_de_promocao(inf, cx, e))
        .filter(|id| cx.campos.values().any(|v| v == id));
    let chave = nova_chave(inf, cx, t);
    let r = RefCasada { chave, tipo: t, alvo, propriedade: alvo.is_some(), versao: None };
    let _ = casar(inf, cx, r, |inf, cx| tipar(inf, cx, p, final_, atribuicao, false));
}
