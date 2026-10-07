//! Instruções: blocos, declarações, controle de fluxo (com os modelos de
//! `flow-analysis.md` para `if`, laços, `switch`, `try`, saltos), `return` e
//! `yield` (coletados para a inferência do retorno de closures).

use super::corpo::{AlvoSalto, Corpo, Local, Nome};
use super::expr::{self, declarar_local, inferir, inferir_livre};
use super::fluxo::Fluxo;
use super::funcoes;
use super::padroes;
use super::BodyInferrer;
use crate::codes::*;
use crate::resolved::{LocalId, MemberRef, Resolved};
use crate::table::{Type, TypeId};
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, ExprKind, StmtId, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;

/// Infere uma instrução.
pub(crate) fn inferir_instrucao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, s: StmtId) {
    validar_anotacoes_locais(inf, cx, s);
    let a = &inf.program.unit(cx.unit).ast;
    let st = a.stmt(s);
    let span = st.span;
    match &st.kind {
        StmtKind::Block(stmts) => {
            cx.empurrar_escopo();
            // O escopo do bloco contém todas as suas declarações locais.
            declarar_adiantes(inf, cx, stmts);
            // Sem bloco básico em curso, o trecho vai até a última instrução
            // deste bloco.
            let fim_do_bloco = stmts.last().map(|&x| inf.program.unit(cx.unit).ast.stmt(x).span.end).unwrap_or(span.end);
            cx.fins_de_bloco.push(fim_do_bloco);
            for &x in stmts.iter() {
                if !cx.fluxo.alcancavel && cx.trecho_morto.is_none() {
                    let sx = inf.program.unit(cx.unit).ast.stmt(x).span;
                    let fim = cx.fins_de_fluxo.last().copied().unwrap_or(fim_do_bloco);
                    inf.aviso(DEAD_CODE.template.to_string(), Span { start: sx.start, end: fim.max(sx.end) });
                    cx.trecho_morto = Some(cx.fins_de_fluxo.len());
                    cx.origem_do_morto = Some(s);
                }
                inferir_instrucao(inf, cx, x);
            }
            cx.fins_de_bloco.pop();
            cx.tirar_escopo();
        }
        StmtKind::Variables(vl) => declaracao_de_variaveis(inf, cx, vl),
        StmtKind::PatternVariables { final_, pattern, value } => padroes::declaracao(inf, cx, *final_, *pattern, *value),
        StmtKind::Function(f) => funcoes::funcao_local(inf, cx, *f),
        StmtKind::Expression(e) => {
            inferir_livre(inf, cx, *e);
        }
        StmtKind::If { condition, case_pattern, guard, then, else_ } => {
            let antes = cx.fluxo.clone();
            let (vf, ff) = match case_pattern {
                Some(p) => {
                    let t = inferir_livre(inf, cx, *condition);
                    cx.empurrar_escopo();
                    padroes::caso(inf, cx, *p, t, *guard, Some(*condition))
                }
                None => expr::condicao_verificada(inf, cx, *condition),
            };
            cx.fluxo = vf;
            ramo_de_fluxo(inf, cx, *then);
            if case_pattern.is_some() {
                cx.tirar_escopo();
            }
            let depois_then = std::mem::replace(&mut cx.fluxo, ff);
            if let Some(e) = else_ {
                ramo_de_fluxo(inf, cx, *e);
            }
            let depois_else = std::mem::replace(&mut cx.fluxo, antes);
            cx.fluxo = inf.juntar(&depois_then, &depois_else);
        }
        StmtKind::While { condition, body } => {
            let rotulos = rotulos_pendentes(cx);
            let (escritas, capturadas) = escritas_em(inf, cx, &[Parte::Expr(*condition), Parte::Stmt(*body)]);
            cx.fluxo.juncao_conservadora(&escritas, &capturadas);
            let (vf, ff) = expr::condicao_verificada(inf, cx, *condition);
            cx.saltos.push(AlvoSalto { rotulos, laco: true, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
            cx.fluxo = vf;
            ramo_de_fluxo(inf, cx, *body);
            let alvo = cx.saltos.pop().unwrap();
            let mut saidas = vec![ff];
            saidas.extend(alvo.breaks);
            let base = cx.fluxo.clone();
            cx.fluxo = inf.juntar_todos(&base, &saidas);
        }
        StmtKind::DoWhile { body, condition } => {
            let rotulos = rotulos_pendentes(cx);
            let (escritas, capturadas) = escritas_em(inf, cx, &[Parte::Expr(*condition), Parte::Stmt(*body)]);
            cx.fluxo.juncao_conservadora(&escritas, &capturadas);
            cx.saltos.push(AlvoSalto { rotulos, laco: true, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
            ramo(inf, cx, *body);
            let alvo = cx.saltos.pop().unwrap();
            let mut conts = vec![cx.fluxo.clone()];
            conts.extend(alvo.continues);
            let base = cx.fluxo.clone();
            cx.fluxo = inf.juntar_todos(&base, &conts);
            let (_vf, ff) = expr::condicao_verificada(inf, cx, *condition);
            let mut saidas = vec![ff];
            saidas.extend(alvo.breaks);
            cx.fluxo = inf.juntar_todos(&base, &saidas);
        }
        StmtKind::For { init, condition, updates, body, .. } => {
            let rotulos = rotulos_pendentes(cx);
            cx.empurrar_escopo();
            if let Some(i) = init {
                inicializacao_de_for(inf, cx, i);
            }
            // Inicialização que não completa (`for (var i = throw 0; c; u)`):
            // o primeiro nó morto é a condição, e o `flowEnd` das partes vai
            // dela ao fim da última atualização (`node = parent.updaters.last`,
            // `dead_code_verifier.dart:309-310`); sem atualizações o
            // `updaters.last` lança e nada sai. O resto do laço fica no mesmo
            // trecho.
            let morto_no_inicio = !cx.fluxo.alcancavel && cx.trecho_morto.is_none();
            if morto_no_inicio {
                let a = &inf.program.unit(cx.unit).ast;
                if let (Some(c), Some(u)) = (condition, updates.last()) {
                    let sp = Span { start: a.expr(*c).span.start, end: a.expr(*u).span.end };
                    inf.aviso(DEAD_CODE.template.to_string(), sp);
                }
                cx.trecho_morto = Some(usize::MAX);
            }
            let mut partes = vec![Parte::Stmt(*body)];
            if let Some(c) = condition {
                partes.push(Parte::Expr(*c));
            }
            for u in updates.iter() {
                partes.push(Parte::Expr(*u));
            }
            let (escritas, capturadas) = escritas_em(inf, cx, &partes);
            cx.fluxo.juncao_conservadora(&escritas, &capturadas);
            let (vf, ff) = match condition {
                Some(c) => expr::condicao_verificada(inf, cx, *c),
                None => {
                    let f = cx.fluxo.clone();
                    let i = f.inalcancavel();
                    (f, i)
                }
            };
            cx.saltos.push(AlvoSalto { rotulos, laco: true, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
            cx.fluxo = vf;
            let morto_antes = cx.trecho_morto.is_some();
            cx.origem_do_morto = None;
            ramo_de_fluxo(inf, cx, *body);
            // `_reportForUpdaters`: o trecho morto começou no corpo (ou numa
            // instrução direta dele) e terminou nele — as atualizações também
            // são código morto.
            if !morto_antes && cx.trecho_morto.is_none() && cx.origem_do_morto == Some(*body) {
                let a = &inf.program.unit(cx.unit).ast;
                if let (Some(p), Some(u)) = (updates.first(), updates.last()) {
                    let sp = Span { start: a.expr(*p).span.start, end: a.expr(*u).span.end };
                    inf.aviso(DEAD_CODE.template.to_string(), sp);
                }
            }
            let alvo = cx.saltos.pop().unwrap();
            let mut conts = vec![cx.fluxo.clone()];
            conts.extend(alvo.continues);
            let base = cx.fluxo.clone();
            cx.fluxo = inf.juntar_todos(&base, &conts);
            // Atualizações inalcançáveis: o trecho delas é o do
            // `_reportForUpdaters` (ou nenhum); nada de dentro abre outro.
            let fechar = !cx.fluxo.alcancavel && cx.trecho_morto.is_none();
            if fechar {
                cx.trecho_morto = Some(usize::MAX);
            }
            for u in updates.iter() {
                inferir_livre(inf, cx, *u);
            }
            if fechar {
                cx.trecho_morto = None;
            }
            let mut saidas = vec![ff];
            saidas.extend(alvo.breaks);
            cx.fluxo = inf.juntar_todos(&base, &saidas);
            // Sem atualizações, o `updaters.last` do analyzer lança e o
            // verificador de código morto não segue no corpo: o trecho fica
            // aberto até o fim dele.
            if morto_no_inicio {
                // Profundidade 0: nenhum `sair_fluxo` o fecha.
                cx.trecho_morto = if condition.is_some() && updates.is_empty() { Some(0) } else { None };
            }
            cx.tirar_escopo();
        }
        StmtKind::ForIn { await_, target, iterable, body } => {
            let rotulos = rotulos_pendentes(cx);
            cx.empurrar_escopo();
            let antes_do_corpo = {
                let alcancavel_antes = cx.fluxo.alcancavel;
                cabecalho_for_in(inf, cx, target, *iterable, *await_);
                // Iterável que não completa (`Never`): o primeiro nó morto é a
                // variável do laço (o `DeclaredIdentifier`, depois do
                // iterável na ordem do fluxo), e o trecho vai dela ao fim do
                // bloco básico.
                if alcancavel_antes && !cx.fluxo.alcancavel && cx.trecho_morto.is_none() {
                    let fonte = inf.program.unit(cx.unit).source.as_bytes();
                    let mut i = span.start;
                    while i < span.end && fonte[i] != b'(' {
                        i += 1;
                    }
                    i += 1;
                    while i < span.end && fonte[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    let fim = cx.fins_de_fluxo.last().copied().or(cx.fins_de_bloco.last().copied()).unwrap_or(span.end).max(span.end);
                    inf.aviso(DEAD_CODE.template.to_string(), Span { start: i, end: fim });
                    cx.trecho_morto = Some(cx.fins_de_fluxo.len());
                }
                let (escritas, capturadas) = escritas_em(inf, cx, &[Parte::Stmt(*body)]);
                cx.fluxo.juncao_conservadora(&escritas, &capturadas);
                cx.fluxo.clone()
            };
            cx.saltos.push(AlvoSalto { rotulos, laco: true, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
            ramo_de_fluxo(inf, cx, *body);
            let alvo = cx.saltos.pop().unwrap();
            let depois = cx.fluxo.clone();
            let mut saidas = vec![antes_do_corpo, depois];
            saidas.extend(alvo.breaks);
            let base = cx.fluxo.clone();
            cx.fluxo = inf.juntar_todos(&base, &saidas);
            cx.tirar_escopo();
        }
        StmtKind::Switch { value, cases } => {
            let rotulos = rotulos_pendentes(cx);
            let t = inferir_livre(inf, cx, *value);
            expr::uso_de_void(inf, cx, *value, t);
            let depois_valor = cx.fluxo.clone();
            let mut nao_casou = depois_valor.clone();
            // Todos os casos casam a mesma referência do valor (T6).
            let escrutinio_de_fora = cx.escrutinio_de_switch.replace(None);
            cx.saltos.push(AlvoSalto { rotulos, laco: false, e_switch: true, breaks: Vec::new(), continues: Vec::new() });
            let mut tem_default = false;
            let mut i = 0;
            // O índice do grupo de casos (os que dividem o corpo contam um).
            let mut grupo = 0usize;
            let n = cases.len();
            let mut saidas: Vec<Fluxo> = Vec::new();
            while i < n {
                // Casos agrupados: os vazios seguem para o próximo corpo.
                let mut j = i;
                let mut entradas: Vec<Fluxo> = Vec::new();
                cx.empurrar_escopo();
                let primeiro_local = cx.locais.len();
                // As variáveis de cada membro do grupo (o `addAll` do
                // `_SharedCaseScope`), e se há rótulo ou `default`.
                let mut membros: Vec<Vec<(SymbolId, LocalId)>> = Vec::new();
                let mut com_rotulo = false;
                let mut com_default = false;
                loop {
                    let c = &cases[j];
                    if !c.labels.is_empty() {
                        com_rotulo = true;
                    }
                    cx.fluxo = nao_casou.clone();
                    // A cabeça de cada `case`/`default` é um bloco básico
                    // (`handleSwitchBeforeAlternative` e o `flowEnd` de
                    // `resolver.dart:1083`/`:1098`): se nenhum valor chega a
                    // ela, o trecho morto é só a palavra-chave, e o padrão e
                    // a guarda não abrem outro (Apêndice B do §B).
                    let fluxo_de_padroes = padroes::fluxo_de_padroes_ligado(inf);
                    if fluxo_de_padroes {
                        entrar_fluxo(cx, c.span.end);
                        if !cx.fluxo.alcancavel && cx.trecho_morto.is_none() {
                            let palavra = palavra_do_caso(inf, cx, c);
                            inf.aviso(DEAD_CODE.template.to_string(), palavra);
                            cx.trecho_morto = Some(cx.fins_de_fluxo.len());
                        }
                    }
                    match c.pattern {
                        Some(p) => {
                            let (vf, ff) = padroes::caso(inf, cx, p, t, c.guard, Some(*value));
                            entradas.push(vf);
                            nao_casou = ff;
                            let faixa = cx.locais_do_ultimo_padrao.clone();
                            membros.push(
                                faixa
                                    .filter(|&k| cx.locais[k].offset != 0)
                                    .map(|k| (cx.locais[k].nome, LocalId(k as u32)))
                                    .collect(),
                            );
                        }
                        None => {
                            tem_default = true;
                            com_default = true;
                            membros.push(Vec::new());
                            entradas.push(nao_casou.clone());
                            nao_casou = nao_casou.inalcancavel();
                        }
                    }
                    if fluxo_de_padroes {
                        sair_fluxo(cx);
                    }
                    if !c.body.is_empty() || j + 1 >= n {
                        break;
                    }
                    j += 1;
                }
                // A variável de junção (a que o corpo enxerga, a cópia do
                // último caso) tem, em cada alternativa, o estado da cópia
                // daquela alternativa: a junção dos fluxos guarda a promoção
                // comum a todas (`switchStatement_endAlternatives` junta as
                // variáveis de junção, `case int? x?: case int? x?:` → `int`).
                if j > i && membros.len() == entradas.len() {
                    if let Some(primeiro) = membros.first() {
                        for &(n, _) in primeiro {
                            let ids: Option<Vec<LocalId>> = membros.iter().map(|m| m.iter().find(|(x, _)| *x == n).map(|(_, id)| *id)).collect();
                            let Some(ids) = ids else { continue };
                            let Some(&rep) = ids.last() else { continue };
                            for (k, &idk) in ids.iter().enumerate() {
                                if idk == rep {
                                    continue;
                                }
                                if let Some(m) = entradas[k].modelo(idk).cloned() {
                                    let r = rep.0 as usize;
                                    if entradas[k].vars.len() <= r {
                                        entradas[k].vars.resize(r + 1, None);
                                    }
                                    entradas[k].vars[r] = Some(m);
                                }
                            }
                        }
                    }
                }
                let base = depois_valor.clone();
                cx.fluxo = inf.juntar_todos(&base, &entradas);
                // Casos que dividem o corpo: as variáveis dos padrões são
                // variáveis de junção, atribuídas em qualquer caminho que
                // chegue ao corpo (cada `case` declarou a sua cópia; a
                // junção dos fluxos, sozinha, as via como não atribuídas).
                if j > i {
                    for id in primeiro_local..cx.locais.len() {
                        let id = LocalId(id as u32);
                        if cx.fluxo.modelo(id).is_none() {
                            cx.fluxo.declarar(id);
                        }
                        cx.fluxo.inicializar(id);
                    }
                }
                if com_rotulo {
                    membros.push(Vec::new());
                }
                juncoes_do_grupo(inf, cx, &membros, com_rotulo || com_default);
                let corpo: Vec<StmtId> = cases[j].body.to_vec();
                // O corpo do grupo é um bloco básico (`resolver.dart:1126`):
                // o trecho morto vai da primeira instrução inalcançável ao
                // fim da última instrução do corpo.
                let fluxo_de_padroes = padroes::fluxo_de_padroes_ligado(inf);
                let fim_do_corpo = corpo.last().map(|&x| inf.program.unit(cx.unit).ast.stmt(x).span.end);
                if fluxo_de_padroes {
                    if let Some(fim) = fim_do_corpo {
                        entrar_fluxo(cx, fim);
                    }
                }
                // As instruções do membro formam um escopo local com as
                // declarações delas (o `LocalScope` das `statements` do
                // `SwitchMember`): usar antes de declarar é
                // `REFERENCED_BEFORE_DECLARATION`.
                declarar_adiantes(inf, cx, &corpo);
                for s in corpo {
                    if fluxo_de_padroes && !cx.fluxo.alcancavel && cx.trecho_morto.is_none() {
                        if let Some(fim) = fim_do_corpo {
                            let inicio = inf.program.unit(cx.unit).ast.stmt(s).span.start;
                            inf.aviso(DEAD_CODE.template.to_string(), Span { start: inicio, end: fim });
                            cx.trecho_morto = Some(cx.fins_de_fluxo.len());
                        }
                    }
                    inferir_instrucao(inf, cx, s);
                }
                if fluxo_de_padroes && fim_do_corpo.is_some() {
                    sair_fluxo(cx);
                }
                // `switchCaseCompletesNormally` (`shared_type_analyzer.dart:232-239`,
                // `analyzeSwitchStatement`): sem padrões (< 3.0), o corpo de um
                // grupo que não é o último não pode chegar ao fim; na palavra
                // do primeiro membro do grupo.
                // O analisador do 3.6.2 recebe o índice do GRUPO e o usa na lista
                // de membros (`node.members[caseIndex].keyword`,
                // `shared_type_analyzer.dart:232-239`): com casos que dividem o
                // corpo antes, o relato cai num membro anterior.
                let versao = inf.program.library(cx.lib).features.versao();
                if cx.fluxo.alcancavel && j + 1 < n && versao < dartforge_frontend::features::LanguageVersion::new(3, 0) {
                    let palavra = palavra_do_caso(inf, cx, &cases[grupo]);
                    inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::SWITCH_CASE_COMPLETES_NORMALLY, palavra, &[]);
                }
                if cx.fluxo.alcancavel {
                    saidas.push(cx.fluxo.clone());
                }
                cx.tirar_escopo();
                i = j + 1;
                grupo += 1;
            }
            let alvo = cx.saltos.pop().unwrap();
            cx.escrutinio_de_switch = escrutinio_de_fora;
            saidas.extend(alvo.breaks);
            if !tem_default && !switch_exaustivo(inf, cx, t, cases) {
                saidas.push(nao_casou);
            }
            cx.fluxo = inf.juntar_todos(&depois_valor, &saidas);
        }
        StmtKind::Break(rotulo) => {
            let f = cx.fluxo.clone();
            if let Some(alvo) = alvo_de_salto(cx, rotulo.map(|n| n.sym), false) {
                alvo.breaks.push(f);
            }
            cx.fluxo.alcancavel = false;
        }
        StmtKind::Continue(rotulo) => {
            let f = cx.fluxo.clone();
            if let Some(alvo) = alvo_de_salto(cx, rotulo.map(|n| n.sym), true) {
                alvo.continues.push(f);
            }
            cx.fluxo.alcancavel = false;
        }
        StmtKind::Return(e) => {
            retorno(inf, cx, *e, span);
            cx.fluxo.alcancavel = false;
        }
        StmtKind::Yield { star, value } => {
            let (m, k) = match cx.funcoes.last() {
                Some(f) => (f.modificador, f.contexto_retorno),
                None => (AsyncModifier::None, inf.core.unknown),
            };
            // O tipo de retorno declarado do gerador (`imposedType`).
            let declarado = cx.funcoes.last().and_then(|f| f.retorno.filter(|_| f.executavel.is_some()));
            if *star {
                let ctx = if m == AsyncModifier::AsyncStar { inf.fluxo_de(k) } else { inf.iteravel(k) };
                let t = inferir(inf, cx, *value, ctx);
                expr::uso_de_void(inf, cx, *value, t);
                // `YieldStatementResolver._resolve_generator` (`:171-175`).
                if matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
                    expr::desreferencia_anulavel(inf, cx, *value, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_USE_OF_NULLABLE_VALUE_IN_YIELD_EACH);
                }
                yield_invalido(inf, cx, *value, t, declarado, true, m);
                let classe = if m == AsyncModifier::AsyncStar { inf.core.stream_class } else { inf.core.iterable_class };
                let el = if inf.e_dynamic(t) { inf.core.dynamic_ } else { inf.como_instancia_de(t, classe).map(|a| a[0]).unwrap_or(inf.core.dynamic_) };
                if let Some(f) = cx.funcoes.last_mut() {
                    f.retornados.push(el);
                }
            } else {
                let t = inferir(inf, cx, *value, k);
                expr::uso_de_void(inf, cx, *value, t);
                yield_invalido(inf, cx, *value, t, declarado, false, m);
                if let Some(f) = cx.funcoes.last_mut() {
                    f.retornados.push(t);
                }
            }
        }
        StmtKind::Try { body, catches, finally_ } => {
            let antes = cx.fluxo.clone();
            let (escritas, capturadas) = escritas_em(inf, cx, &[Parte::Stmt(*body)]);
            ramo_de_fluxo(inf, cx, *body);
            let depois_try = cx.fluxo.clone();
            let mut saidas = vec![depois_try];
            // `_CatchClausesVerifier` (`dead_code_verifier.dart:495-545`): os
            // tipos `on` já vistos, e se algum `catch` já foi dado por morto.
            let mut tipos_de_catch: Vec<TypeId> = Vec::new();
            let mut catch_morto = false;
            for (indice_do_catch, c) in catches.iter().enumerate() {
                let mut f = antes.clone();
                f.juncao_conservadora(&escritas, &capturadas);
                cx.fluxo = f;
                cx.empurrar_escopo();
                let tipo_ex = match c.on_type {
                    Some(t) => inf.tipo_no_contexto(cx, t, crate::resolve::ContextoDeTipo::Catch),
                    None => inf.core.object,
                };
                {
                    use dartforge_diagnostics::codigos::warning as w;
                    // `on T` com `T` potencialmente anulável (e não inválido):
                    // não se lança um valor anulável.
                    if let Some(t) = c.on_type {
                        if !inf.table.e_invalido(tipo_ex) && !inf.e_nao_anulavel(tipo_ex) {
                            let sp = inf.program.unit(cx.unit).ast.ty(t).span;
                            inf.aviso_com_codigo(w::NULLABLE_TYPE_IN_CATCH_CLAUSE, sp, &[]);
                        }
                    }
                    if !catch_morto {
                        let ultimo = catches.len() - 1;
                        let fim = catches[ultimo].span.end;
                        if c.on_type.is_none() || tipo_ex == inf.core.object {
                            // Um `catch` que pega tudo: os seguintes são mortos.
                            if indice_do_catch != ultimo {
                                let inicio = catches[indice_do_catch + 1].span.start;
                                inf.aviso_com_codigo(w::DEAD_CODE_CATCH_FOLLOWING_CATCH, Span { start: inicio, end: fim }, &[]);
                                catch_morto = true;
                            }
                        } else {
                            let anterior = tipos_de_catch.iter().copied().find(|&s| inf.sub(tipo_ex, s));
                            match anterior {
                                Some(s) => {
                                    inf.aviso_com_args(
                                        w::DEAD_CODE_ON_CATCH_SUBTYPE,
                                        Span { start: c.span.start, end: fim },
                                        &[crate::exibicao::Arg::Tipo(tipo_ex), crate::exibicao::Arg::Tipo(s)],
                                    );
                                    catch_morto = true;
                                }
                                None => tipos_de_catch.push(tipo_ex),
                            }
                        }
                    }
                }
                if let Some(n) = &c.exception {
                    declarar_local(inf, cx, Local { nome: n.sym, tipo: tipo_ex, final_: true, late: false, const_: false, offset: n.span.start, funcao_local: false }, true);
                }
                if let Some(n) = &c.stack_trace {
                    let st = stack_trace(inf);
                    declarar_local(inf, cx, Local { nome: n.sym, tipo: st, final_: true, late: false, const_: false, offset: n.span.start, funcao_local: false }, true);
                }
                let fim = fim_de_fluxo(inf, cx, c.body);
                entrar_fluxo(cx, fim);
                inferir_instrucao(inf, cx, c.body);
                sair_fluxo(cx);
                cx.tirar_escopo();
                saidas.push(cx.fluxo.clone());
            }
            let apos_catches = inf.juntar_todos(&antes, &saidas);
            if let Some(fin) = finally_ {
                let mut f = inf.juntar(&apos_catches, &antes);
                f.juncao_conservadora(&escritas, &capturadas);
                let inicio_fin = f.clone();
                cx.fluxo = f;
                ramo(inf, cx, *fin);
                if cx.fluxo.alcancavel {
                    // O `finally` completa (`attachFinally`): vale o estado
                    // depois do `try`/`catch`, com as promoções do `finally`
                    // reaplicadas (`o as int;` no `finally` promove depois),
                    // e o estado do `finally` para o que ele escreveu.
                    let fim = cx.fluxo.clone();
                    let alcanca = apos_catches.alcancavel;
                    let mut r = inf.reaplicar(&apos_catches, &fim);
                    for (i, m) in fim.vars.iter().enumerate() {
                        let v0 = inicio_fin.vars.get(i).and_then(|m| m.as_ref()).map(|m| m.versao);
                        if m.is_some() && m.as_ref().map(|m| m.versao) != v0 {
                            if r.vars.len() <= i {
                                r.vars.resize(i + 1, None);
                            }
                            r.vars[i] = m.clone();
                        }
                    }
                    r.alcancavel = alcanca;
                    cx.fluxo = r;
                }
            } else {
                cx.fluxo = apos_catches;
            }
        }
        StmtKind::Labeled { labels, body } => {
            let ls: Vec<SymbolId> = labels.iter().map(|l| l.sym).collect();
            let corpo_e_laco = matches!(
                inf.program.unit(cx.unit).ast.stmt(*body).kind,
                StmtKind::For { .. } | StmtKind::ForIn { .. } | StmtKind::While { .. } | StmtKind::DoWhile { .. } | StmtKind::Switch { .. }
            );
            if corpo_e_laco {
                cx.rotulos_pendentes.extend(ls);
                inferir_instrucao(inf, cx, *body);
            } else {
                cx.saltos.push(AlvoSalto { rotulos: ls, laco: false, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
                inferir_instrucao(inf, cx, *body);
                let alvo = cx.saltos.pop().unwrap();
                let mut saidas = vec![cx.fluxo.clone()];
                saidas.extend(alvo.breaks);
                let base = cx.fluxo.clone();
                cx.fluxo = inf.juntar_todos(&base, &saidas);
            }
        }
        StmtKind::Assert { condition, message } => {
            let antes = cx.fluxo.clone();
            expr::condicao_de_assert(inf, cx, *condition);
            if let Some(m) = message {
                inferir_livre(inf, cx, *m);
            }
            cx.fluxo = antes;
        }
        StmtKind::Empty => {}
    }
}

/// Pré-declara (como "adiante") os locais de uma lista de instruções:
/// variáveis, funções locais e as variáveis das declarações de padrão.
fn declarar_adiantes(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, stmts: &[StmtId]) {
    for &x in stmts.iter() {
        match &inf.program.unit(cx.unit).ast.stmt(x).kind {
            StmtKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    cx.declarar_adiante(v.name.sym, v.name.span);
                }
            }
            StmtKind::Function(f) => {
                if let Some(n) = inf.program.unit(cx.unit).ast.function(*f).name {
                    cx.declarar_adiante(n.sym, n.span);
                }
            }
            StmtKind::PatternVariables { pattern, .. } => {
                let refutavel = std::mem::replace(&mut cx.padrao_refutavel, false);
                let mut nomes = Vec::new();
                padroes::variaveis_declaradas(inf, cx, *pattern, &mut nomes);
                cx.padrao_refutavel = refutavel;
                for n in nomes {
                    cx.declarar_adiante(n.sym, n.span);
                }
            }
            _ => {}
        }
    }
}

/// Um ramo (`then`, `else`, corpo de laço) em escopo próprio.
fn ramo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, s: StmtId) {
    cx.empurrar_escopo();
    inferir_instrucao(inf, cx, s);
    cx.tirar_escopo();
}

/// Um ramo que fecha um bloco básico (`flowEnd` do analyzer: `then`/`else`,
/// corpo de `while`/`for`/`for-in`, corpo do `try`): se ele mesmo é
/// inalcançável, o trecho morto é a instrução inteira (`if (false) { … }`
/// relata do `{` ao `}`).
fn ramo_de_fluxo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, s: StmtId) {
    let span = inf.program.unit(cx.unit).ast.stmt(s).span;
    if !cx.fluxo.alcancavel && cx.trecho_morto.is_none() {
        inf.aviso(DEAD_CODE.template.to_string(), span);
        cx.trecho_morto = Some(cx.fins_de_fluxo.len() + 1);
        cx.origem_do_morto = Some(s);
    }
    let fim = fim_de_fluxo(inf, cx, s);
    entrar_fluxo(cx, fim);
    ramo(inf, cx, s);
    sair_fluxo(cx);
}

/// O fim de um bloco básico que termina em `s`, aparado como o analyzer:
/// num bloco com instruções, o fim da última.
pub(crate) fn fim_de_fluxo(inf: &BodyInferrer<'_>, cx: &Corpo, s: StmtId) -> usize {
    let a = &inf.program.unit(cx.unit).ast;
    match &a.stmt(s).kind {
        StmtKind::Block(stmts) if !stmts.is_empty() => a.stmt(*stmts.last().unwrap()).span.end,
        _ => a.stmt(s).span.end,
    }
}

/// Entra num bloco básico que termina em `fim`.
pub(crate) fn entrar_fluxo(cx: &mut Corpo, fim: usize) {
    cx.fins_de_fluxo.push(fim);
}

/// Sai do bloco básico: um trecho morto começado nele termina aqui.
pub(crate) fn sair_fluxo(cx: &mut Corpo) {
    let profundidade = cx.fins_de_fluxo.len();
    cx.fins_de_fluxo.pop();
    if cx.trecho_morto.is_some_and(|p| p >= profundidade) {
        cx.trecho_morto = None;
    }
}

/// A palavra `case` ou `default` de um membro de `switch`, depois dos
/// rótulos (`rotulo: case 1:`).
/// `switchStatementSharedCaseScopeFinish` (`variable_bindings.dart:177-202`) e
/// `finishJoinedPatternVariable` (`resolver.dart:878-915`): as variáveis de
/// um grupo de casos que divide o corpo. Uma variável que não está em todos
/// os membros (um `default` ou um rótulo conta como membro vazio) é
/// `…_HAS_LABEL` com rótulo ou `default`, senão `…_NOT_ALL_CASES`; presente em
/// todos, mas com tipos ou `final` diferentes, `…_DIFFERENT_FINALITY_OR_TYPE`.
/// O código vale para cada referência no corpo.
fn juncoes_do_grupo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, membros: &[Vec<(SymbolId, LocalId)>], com_rotulo: bool) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    let mut nomes: Vec<SymbolId> = Vec::new();
    let mut componentes: std::collections::HashMap<SymbolId, (Vec<LocalId>, bool)> = std::collections::HashMap::new();
    for (k, m) in membros.iter().enumerate() {
        if k == 0 {
            for &(n, id) in m {
                if !componentes.contains_key(&n) {
                    nomes.push(n);
                    componentes.insert(n, (vec![id], true));
                }
            }
            continue;
        }
        for n in nomes.clone() {
            let entrada = componentes.get_mut(&n).unwrap();
            match m.iter().find(|(x, _)| *x == n) {
                Some(&(_, id)) => entrada.0.push(id),
                None => entrada.1 = false,
            }
        }
        for &(n, id) in m {
            if !componentes.contains_key(&n) {
                nomes.push(n);
                componentes.insert(n, (vec![id], false));
            }
        }
    }
    for n in nomes {
        let (ids, todos) = componentes.remove(&n).unwrap();
        if todos && ids.len() == 1 {
            continue;
        }
        let codigo = if !todos {
            Some(if com_rotulo { c::PATTERN_VARIABLE_SHARED_CASE_SCOPE_HAS_LABEL } else { c::PATTERN_VARIABLE_SHARED_CASE_SCOPE_NOT_ALL_CASES })
        } else {
            let primeiro = cx.local(ids[0]).clone();
            let tipo = inf.table.canonico(primeiro.tipo);
            let diferente = ids[1..].iter().any(|&id| {
                let l = cx.local(id);
                inf.table.canonico(l.tipo) != tipo || l.final_ != primeiro.final_
            });
            diferente.then_some(c::PATTERN_VARIABLE_SHARED_CASE_SCOPE_DIFFERENT_FINALITY_OR_TYPE)
        };
        if let Some(codigo) = codigo {
            for id in ids {
                cx.juncoes_inconsistentes.insert(id, codigo);
            }
        }
    }
}

fn palavra_do_caso(inf: &BodyInferrer<'_>, cx: &Corpo, c: &ast::SwitchCase) -> Span {
    let palavra: &[u8] = if c.pattern.is_some() { b"case" } else { b"default" };
    let fonte = inf.program.unit(cx.unit).source.as_bytes();
    let de = c.labels.last().map_or(c.span.start, |l| l.span.end);
    let ate = c.span.end.min(fonte.len());
    let mut i = de;
    while i + palavra.len() <= ate {
        let antes_ok = i == 0 || !(fonte[i - 1].is_ascii_alphanumeric() || fonte[i - 1] == b'_');
        if antes_ok && &fonte[i..i + palavra.len()] == palavra {
            return Span { start: i, end: i + palavra.len() };
        }
        i += 1;
    }
    Span { start: c.span.start, end: (c.span.start + palavra.len()).min(c.span.end) }
}

fn rotulos_pendentes(cx: &mut Corpo) -> Vec<SymbolId> {
    std::mem::take(&mut cx.rotulos_pendentes)
}

fn alvo_de_salto(cx: &mut Corpo, rotulo: Option<SymbolId>, continuar: bool) -> Option<&mut AlvoSalto> {
    for alvo in cx.saltos.iter_mut().rev() {
        match rotulo {
            Some(r) => {
                if alvo.rotulos.contains(&r) {
                    return Some(alvo);
                }
            }
            None => {
                if alvo.laco || (!continuar && alvo.e_switch) {
                    return Some(alvo);
                }
            }
        }
    }
    None
}

fn stack_trace(inf: &mut BodyInferrer<'_>) -> TypeId {
    let core = inf.core.core_library;
    let sym = inf.interner.lookup("StackTrace");
    match (core, sym) {
        (Some(l), Some(s)) => match inf.program.lookup(l, s).and_then(|b| b.getter) {
            Some(dartforge_elements::model::Element::Class(c)) => inf.tipo_de_classe_com_args(c, Vec::new()),
            _ => inf.core.dynamic_,
        },
        _ => inf.core.dynamic_,
    }
}

/// O `isExhaustive` do `analyzeSwitchStatement`
/// (`_fe_analyzer_shared/lib/src/type_inference/type_analyzer.dart:2017-2027`)
/// de um `switch` sem `default`: com padrões (linguagem 3.0 ou mais), o
/// `isAlwaysExhaustiveType` do tipo do escrutínio; antes, o
/// `SwitchExhaustiveness` legado (`resolver.dart:5469-5557`): um enum cujas
/// constantes aparecem todas em `case` sem guarda (constante, entre
/// parênteses ou não), com o `null` coberto pelo tipo não anulável ou por um
/// `case null`.
fn switch_exaustivo(inf: &mut BodyInferrer<'_>, cx: &Corpo, t: TypeId, casos: &[ast::SwitchCase]) -> bool {
    if inf.program.library(cx.lib).features.versao().major >= 3 {
        return sempre_exaustivo(inf, t, 0);
    }
    let Type::Interface { class, nullable, .. } = inf.table.get(t).clone() else { return false };
    let k = inf.program.class(class);
    if k.kind != dartforge_elements::model::ClassKind::Enum {
        return false;
    }
    let mut constantes: Vec<dartforge_elements::model::VariableId> = k.enum_constants.clone();
    let mut nulo_coberto = !nullable;
    let a = &inf.program.unit(cx.unit).ast;
    let corpo = &inf.body_types.units[cx.unit.0 as usize];
    let mut exaustivo = false;
    for c in casos {
        let Some(p) = c.pattern else { continue };
        if c.guard.is_some() {
            continue;
        }
        let ast::PatternKind::Constant(mut e) = a.pattern(p).kind else { continue };
        while let ast::ExprKind::Parenthesized(i) = a.expr(e).kind {
            e = i;
        }
        // `_referencedElement`: o getter da constante (a variável dele).
        let variavel = match corpo.get_resolved(e) {
            Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => Some(*v),
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) => inf.program.function(*f).variable,
            Some(Resolved::Element(dartforge_elements::model::Element::Variable(v))) => Some(*v),
            _ => None,
        };
        if let Some(v) = variavel {
            constantes.retain(|x| *x != v);
        }
        if matches!(a.expr(e).kind, ast::ExprKind::Null) {
            nulo_coberto = true;
        }
        if constantes.is_empty() && nulo_coberto {
            exaustivo = true;
        }
    }
    exaustivo
}

/// `TypeSystemImpl.isAlwaysExhaustive` (analyzer 7.7.1,
/// `type_system.dart:819`): o `?` não conta (o `S?` de uma classe `sealed`
/// é sempre exaustivo — `case var y?` e `case null` o cobrem); `Null`,
/// `bool`, enum e classe `sealed`; tipo de extensão pela *erasure*;
/// `FutureOr<T>` pelo `T`; parâmetro de tipo pelo limite promovido ou pelo
/// declarado; record quando todos os campos são.
fn sempre_exaustivo(inf: &mut BodyInferrer<'_>, t: TypeId, prof: u32) -> bool {
    if prof > 32 {
        return false;
    }
    match inf.table.get(t).clone() {
        Type::Null => true,
        Type::Interface { class, .. } => {
            let c = inf.program.class(class);
            c.kind == dartforge_elements::model::ClassKind::Enum
                || c.modifiers.sealed
                || Some(class) == inf.core.bool_class
        }
        Type::ExtensionType { decl, .. } => {
            let Some(rep) = inf.program.class(decl).representation else {
                return false;
            };
            let Some(d) = inf.outline.variables[rep.0 as usize].declared_type else {
                return false;
            };
            let apagado = inf.substituir_do_dono(t, decl, decl, d);
            sempre_exaustivo(inf, apagado, prof + 1)
        }
        Type::FutureOr { arg, .. } => sempre_exaustivo(inf, arg, prof + 1),
        Type::Intersection { param, bound } => {
            sempre_exaustivo(inf, bound, prof + 1) || {
                let b = inf.table.param(param).bound;
                sempre_exaustivo(inf, b, prof + 1)
            }
        }
        Type::TypeParameter { param, .. } => {
            let b = inf.table.param(param).bound;
            sempre_exaustivo(inf, b, prof + 1)
        }
        Type::Record { positional, named, .. } => positional
            .iter()
            .copied()
            .chain(named.iter().map(|(_, c)| *c))
            .collect::<Vec<_>>()
            .into_iter()
            .all(|c| sempre_exaustivo(inf, c, prof + 1)),
        _ => false,
    }
}

/// `_checkForYieldOfInvalidType` (`an611:src/dart/resolver/yield_statement_resolver.dart:
/// 72-140`): o valor de `yield` não cabe no elemento do retorno declarado
/// (`YIELD_OF_INVALID_TYPE`), ou o de `yield*` não cabe no retorno
/// (`YIELD_EACH_OF_INVALID_TYPE`), na expressão.
fn yield_invalido(inf: &mut BodyInferrer<'_>, cx: &Corpo, e: ExprId, t: TypeId, r: Option<TypeId>, estrela: bool, m: AsyncModifier) {
    use dartforge_diagnostics::codigos::compile_time_error as c;
    if !matches!(m, AsyncModifier::SyncStar | AsyncModifier::AsyncStar) {
        return;
    }
    let sp = inf.span_expr(cx.unit, e);
    let classe = if m == AsyncModifier::AsyncStar { inf.core.stream_class } else { inf.core.iterable_class };
    if estrela {
        if let Some(r) = r.filter(|&r| !inf.atribuivel(t, r)) {
            inf.aviso_com_args(c::YIELD_EACH_OF_INVALID_TYPE, sp, &[crate::exibicao::Arg::Tipo(t), crate::exibicao::Arg::Tipo(r)]);
            return;
        }
        let d = inf.core.dynamic_;
        let req = if m == AsyncModifier::AsyncStar { inf.fluxo_de(d) } else { inf.iteravel(d) };
        if !inf.atribuivel(t, req) {
            inf.aviso_com_args(c::YIELD_EACH_OF_INVALID_TYPE, sp, &[crate::exibicao::Arg::Tipo(t), crate::exibicao::Arg::Tipo(req)]);
        }
    } else if let Some(args) = r.and_then(|r| inf.como_instancia_de(r, classe)) {
        let v = args[0];
        if !inf.atribuivel(t, v) {
            inf.aviso_com_args(c::YIELD_OF_INVALID_TYPE, sp, &[crate::exibicao::Arg::Tipo(t), crate::exibicao::Arg::Tipo(v)]);
        }
    }
}

/// O inicializador de uma local `late`: o fluxo o trata como um literal de
/// função (`lateInitializer_begin`/`_end` são os de `functionExpression`):
/// lá dentro, as variáveis escritas em qualquer lugar do membro não estão
/// definitivamente não atribuídas; depois dele, o fluxo é o de antes, e o
/// que ele escreve conta como escrita capturada.
fn inicializador_late(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, init: ExprId, ctx: TypeId) -> TypeId {
    let antes = cx.fluxo.clone();
    let mut dentro = antes.clone();
    if cx.escritos_no_corpo.is_none() {
        let a = &inf.program.unit(cx.unit).ast;
        cx.escritos_no_corpo = Some(match cx.raiz {
            super::corpo::Raiz::Funcao(f) => nomes_escritos_separados(inf, cx.unit, &a.function(f).body),
            super::corpo::Raiz::Construtor(m) => match &a.member(m).kind {
                ast::MemberKind::Constructor(c) => nomes_escritos_separados(inf, cx.unit, &c.body),
                _ => Default::default(),
            },
            super::corpo::Raiz::Nada => Default::default(),
        });
    }
    let (fora, capturadas) = cx.escritos_no_corpo.clone().unwrap_or_default();
    for e in fora.iter() {
        if let Some(id) = local_da_escrita(cx, *e) {
            dentro.juncao_conservadora(&[id], &[]);
        }
    }
    for e in capturadas.iter() {
        if let Some(id) = local_da_escrita(cx, *e) {
            dentro.juncao_conservadora(&[], &[id]);
        }
    }
    cx.fluxo = dentro;
    let t = inferir(inf, cx, init, ctx);
    // O que o inicializador escreve, sintaticamente.
    let mut v = Varredura::nova(&inf.program.unit(cx.unit).ast);
    v.expr(init, false);
    let escritas: Vec<Escrita> = v.fora.into_iter().chain(v.dentro).collect();
    cx.fluxo = antes;
    for e in escritas {
        if let Some(id) = local_da_escrita(cx, e) {
            cx.fluxo.capturar(id);
        }
    }
    t
}

/// Declaração de variáveis locais (`var`, `final`, tipadas, `late`, `const`).
pub(crate) fn declaracao_de_variaveis(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, vl: &ast::VariableList) {
    let declarado = vl.ty.map(|t| inf.tipo_de_anotacao(cx, t));
    let u = inf.core.unknown;
    for v in vl.variables.iter() {
        let mut tipo = declarado;
        let mut escrito = None;
        let mut condicao_guardada = None;
        if let Some(init) = v.initializer {
            // Inicializador que é condição (`x != null && …`): os modelos
            // verdadeiro/falso ficam guardados na variável (§7.10); `late`
            // nunca guarda.
            let t = if vl.late {
                inicializador_late(inf, cx, init, declarado.unwrap_or(u))
            } else if expr::e_forma_de_condicao(inf, cx, init) {
                let (sim, nao) = expr::condicao(inf, cx, init);
                cx.fluxo = inf.juntar(&sim, &nao);
                condicao_guardada = Some((sim, nao));
                inf.body_types.units[cx.unit.0 as usize].get_type(init).unwrap_or(inf.core.bool_)
            } else {
                inferir(inf, cx, init, declarado.unwrap_or(u))
            };
            escrito = Some(t);
            if let Some(d) = declarado {
                expr::verificar_atribuivel_expr(inf, cx, init, t, d, INVALID_ASSIGNMENT.template);
            } else {
                tipo = Some(if matches!(inf.table.get(t), Type::Null) { inf.core.dynamic_ } else { t });
            }
            if vl.const_ {
                colecoes_const(inf, cx, init);
            }
            // `DeadCodeVerifier.visitVariableDeclaration`
            // (`dead_code_verifier.dart:114-128`): `late` local curinga (3.7)
            // com inicializador, que nunca é avaliado.
            if vl.late && cx.curinga == Some(v.name.sym) {
                let sp = inf.span_expr(cx.unit, init);
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::warning::DEAD_CODE_LATE_WILDCARD_VARIABLE_INITIALIZER, sp, &[]);
            }
        }
        let tipo = tipo.unwrap_or(inf.core.dynamic_);
        let id = declarar_local(
            inf,
            cx,
            Local { nome: v.name.sym, tipo, final_: vl.final_ || vl.const_, late: vl.late, const_: vl.const_, offset: v.name.span.start, funcao_local: false },
            false,
        );
        match escrito {
            Some(t) if !vl.late => {
                let toi = declarado.is_some() && !vl.final_;
                let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                inf.escrever_fluxo(&mut f, id, tipo, t, toi, None);
                cx.fluxo = f;
            }
            Some(_) => cx.fluxo.inicializar(id),
            None => {}
        }
        if let (Some((sim, nao)), Some(versao)) = (condicao_guardada, cx.fluxo.versao(id)) {
            cx.condicoes.insert(id, (sim, nao, versao));
        }
    }
}

fn colecoes_const(inf: &mut BodyInferrer<'_>, cx: &Corpo, init: ExprId) {
    let unit = cx.unit;
    super::colecoes::validar_colecao_const(inf, cx, init);
    let mut av = crate::constant::ConstantEvaluator::new(inf.program, inf.interner, inf.table, inf.core);
    let r = av.evaluate_expr(unit, init);
    let erro = av.error_thrown.clone();
    drop(av);
    if r.is_none() {
        let sp = inf.span_expr(unit, init);
        match erro {
            Some(e) => inf.aviso(format!("{}: {}", CONST_EVAL_THROWS_EXCEPTION.template, e), sp),
            None if !inf.e_constante(cx, init) => inf.aviso(CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE.template.to_string(), sp),
            None => {}
        }
    }
}

/// Inicialização de um `for` clássico.
pub(crate) fn inicializacao_de_for(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, init: &ast::ForInit) {
    match init {
        ast::ForInit::Variables(vl) => declaracao_de_variaveis(inf, cx, vl),
        ast::ForInit::Expression(e) => {
            inferir_livre(inf, cx, *e);
        }
        ast::ForInit::Pattern { final_, pattern, value } => padroes::declaracao(inf, cx, *final_, *pattern, *value),
    }
}

/// Cabeçalho de `for-in` (instrução ou elemento de coleção): infere o
/// iterável e declara/atribui a variável do laço.
pub(crate) fn cabecalho_for_in(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, target: &ast::ForInTarget, iterable: ExprId, await_: bool) {
    let classe = if await_ { inf.core.stream_class } else { inf.core.iterable_class };
    // Contexto do iterável: `Iterable<T>` com o tipo escrito da variável.
    let escrito = match target {
        ast::ForInTarget::Declared { ty: Some(t), .. } => Some(inf.tipo_de_anotacao(cx, *t)),
        _ => None,
    };
    let u = inf.core.unknown;
    let ctx = match classe {
        Some(c) => {
            let el = escrito.unwrap_or(u);
            inf.iface(Some(c), vec![el])
        }
        None => u,
    };
    let t = inferir(inf, cx, iterable, ctx);
    expr::uso_de_void(inf, cx, iterable, t);
    // `ForResolver._forEachParts` (`for_resolver.dart:164-169`).
    expr::desreferencia_anulavel(inf, cx, iterable, t, dartforge_diagnostics::codigos::compile_time_error::UNCHECKED_USE_OF_NULLABLE_VALUE_AS_ITERATOR);
    let partes_validas = for_in_tipo_invalido(inf, cx, target, iterable, t, escrito, await_);
    // `visitForEachPartsWithDeclaration` (`error_verifier.dart:912-921`): com
    // o `_checkForEachParts` sem erro, o `const` da variável.
    if partes_validas
        && let ast::ForInTarget::Declared { const_: Some(palavra), .. } = target
    {
        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::FOR_IN_WITH_CONST_VARIABLE, *palavra, &[]);
    }
    // `_computeForEachElementType` (`for_resolver.dart:92-116`): iterável
    // `dynamic` dá `dynamic`; o que não é `Iterable`/`Stream` (inclusive o
    // próprio `InvalidType`) dá `InvalidType`, e os usos do elemento não
    // relatam mais nada.
    let el = if inf.table.e_invalido(t) {
        t
    } else if inf.e_dynamic(t) {
        inf.core.dynamic_
    } else {
        match inf.como_instancia_de(t, classe) {
            Some(a) => a[0],
            None => inf.table.invalido(inf.core.dynamic_),
        }
    };
    match target {
        ast::ForInTarget::Declared { final_, name, .. } => {
            let tipo = escrito.unwrap_or(el);
            declarar_local(inf, cx, Local { nome: name.sym, tipo, final_: *final_, late: false, const_: false, offset: name.span.start, funcao_local: false }, true);
        }
        ast::ForInTarget::Pattern { final_, pattern } => {
            padroes::declarar_por_tipo(inf, cx, *final_, *pattern, el);
        }
        ast::ForInTarget::Expression(e) => {
            // `for (x in e)`: atribuição a uma variável existente.
            let a = &inf.program.unit(cx.unit).ast;
            if let ExprKind::Identifier(n) = &a.expr(*e).kind {
                if let Some(Nome::Local(id)) = cx.buscar(n.sym) {
                    expr::resolver(inf, cx, *e, crate::resolved::Resolved::Local(id));
                    // `checkFinalAlreadyAssigned(identifier, isForEachIdentifier:
                    // true)` (3.6.2 `for_resolver.dart:131-136`,
                    // `assignment_expression_resolver.dart:354-388`): o local
                    // `final` relata sempre, sem olhar a atribuição definida.
                    let l = cx.local(id);
                    if l.final_ && !l.const_ && !cx.funcoes_locais.contains(&id) {
                        if l.late {
                            inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::LATE_FINAL_LOCAL_ALREADY_ASSIGNED, n.span, &[]);
                        } else {
                            let msg = format!("{}: '{}'", crate::codes::ASSIGNMENT_TO_FINAL_LOCAL.template, inf.interner.resolve(l.nome));
                            inf.aviso(msg, n.span);
                        }
                    }
                    let decl = cx.local(id).tipo;
                    expr::registrar(inf, cx, *e, decl);
                    let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                    // `DemoteViaExplicitWrite` no identificador
                    // (`ForEachPartsWithIdentifier.identifier`).
                    let motivo = super::fluxo::MotivoDeNaoPromocao::Escrita { nome: n.sym, span: n.span };
                    inf.atribuir_fluxo(&mut f, id, decl, el, Some(motivo));
                    cx.fluxo = f;
                    return;
                }
            }
            inferir_livre(inf, cx, *e);
        }
    }
}

/// As anotações de uma declaração local (`Ast::metadados_locais`), pelo
/// `AnnotationResolver` no escopo local: o nome que resolve para uma local
/// (`_localVariable`, 3.6.2 `annotation_resolver.dart:170-184`) é
/// `INVALID_ANNOTATION` se ela não é `const` ou se há argumentos; a função
/// local também (não é variável nem acessor); o resto pelo escopo da
/// biblioteca, como as anotações de declaração.
fn validar_anotacoes_locais(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, s: StmtId) {
    let a = &inf.program.unit(cx.unit).ast;
    let Some((_, ms)) = a.metadados_locais.iter().find(|(x, _)| *x == s) else { return };
    for m in ms.iter() {
        validar_anotacao_local(inf, cx, m);
    }
}

/// Uma anotação no escopo local (ver [`validar_anotacoes_locais`]).
pub(crate) fn validar_anotacao_local(inf: &mut BodyInferrer<'_>, cx: &Corpo, m: &ast::Annotation) {
    let Some(n1) = m.name.first() else { return };
    match cx.buscar(n1.sym) {
        Some(Nome::Local(id)) => {
            let l = cx.local(id);
            if cx.funcoes_locais.contains(&id) || !l.const_ || m.arguments.is_some() || m.name.len() > 1 {
                inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::INVALID_ANNOTATION, m.span, &[]);
            }
        }
        Some(_) => {}
        None => super::funcoes::validar_anotacao(inf, cx.unit, cx.classe, m),
    }
}

/// `_checkForEachParts` (an611:src/generated/error_verifier.dart:3103-3233):
/// `FOR_IN_OF_INVALID_TYPE` quando o iterável (resolvido ao limite) não é
/// `Iterable`/`Stream`, e `FOR_IN_OF_INVALID_ELEMENT_TYPE` quando o tipo dos
/// elementos não é atribuível à variável do laço. `void` (relatado antes),
/// `dynamic` e tipos anuláveis (o erro é o de nulo) não relatam.
/// Devolve o `_checkForEachParts` (`error_verifier.dart:3104-3210`): sem
/// erro, com o iterável não `void` nem anulável e a variável declarada.
fn for_in_tipo_invalido(
    inf: &mut BodyInferrer<'_>,
    cx: &Corpo,
    target: &ast::ForInTarget,
    iterable: ExprId,
    t: TypeId,
    escrito: Option<TypeId>,
    await_: bool,
) -> bool {
    use dartforge_diagnostics::codigos::compile_time_error as ce;
    let Some(classe) = (if await_ { inf.core.stream_class } else { inf.core.iterable_class }) else { return false };
    // `for (var (p) in e)`: o `_checkForEachParts` não roda (só as partes com
    // declaração e com identificador); o `analyzePatternForIn` compartilhado
    // (3.6.2 `type_analyzer.dart:1445-1463`) relata se `asInstanceOf` do
    // tipo (sem olhar o `?`, parâmetro de tipo pelo limite) não acha
    // `Iterable`/`Stream`, fora `dynamic` e o tipo inválido; os argumentos
    // são o tipo e sempre `'Iterable'` (`shared_type_analyzer.dart:161-171`).
    if matches!(target, ast::ForInTarget::Pattern { .. }) {
        if inf.e_dynamic(t) || inf.table.e_invalido(t) || inf.e_desconhecido(t) {
            return false;
        }
        let mut b = t;
        for _ in 0..16 {
            match inf.table.get(b) {
                Type::TypeParameter { param, .. } => {
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
        let acha = match inf.table.get(b) {
            Type::Interface { .. } | Type::ExtensionType { .. } => {
                let nn = inf.nao_nulo(b);
                inf.como_instancia_de(nn, Some(classe)).is_some()
            }
            _ => false,
        };
        if !acha {
            let sp = inf.span_expr(cx.unit, iterable);
            let tt = inf.table.format(t, inf.interner, inf.program);
            inf.aviso_com_codigo(ce::FOR_IN_OF_INVALID_TYPE, sp, &[&tt, "Iterable"]);
        }
        return false;
    }
    if matches!(inf.table.get(t), Type::Void | Type::Null) || inf.table.get(t).is_declared_nullable() {
        return false;
    }
    if inf.e_dynamic(t) || inf.e_desconhecido(t) {
        return !matches!(target, ast::ForInTarget::Pattern { .. });
    }
    // `resolveToBound`.
    let mut base = t;
    for _ in 0..16 {
        match inf.table.get(base) {
            Type::TypeParameter { param, nullable: false } => {
                let b = inf.table.param(*param).bound;
                if b == base {
                    break;
                }
                base = b;
            }
            Type::Intersection { bound, .. } => base = *bound,
            _ => break,
        }
    }
    let d = inf.core.dynamic_;
    let requerido = inf.iface(Some(classe), vec![d]);
    let nome = if await_ { "Stream" } else { "Iterable" };
    if inf.e_dynamic(base) || matches!(inf.table.get(base), Type::Void) || base == inf.core.object_nullable {
        return !matches!(target, ast::ForInTarget::Pattern { .. });
    }
    let sp = inf.span_expr(cx.unit, iterable);
    if !inf.atribuivel(base, requerido) {
        let tt = inf.table.format(base, inf.interner, inf.program);
        inf.aviso_com_codigo(ce::FOR_IN_OF_INVALID_TYPE, sp, &[&tt, nome]);
        return false;
    }
    // Tipo da variável: o escrito, ou o do local existente em `for (x in …)`.
    let var = match target {
        ast::ForInTarget::Declared { .. } => escrito,
        ast::ForInTarget::Expression(e) => match &inf.program.unit(cx.unit).ast.expr(*e).kind {
            ExprKind::Identifier(n) => match cx.buscar(n.sym) {
                Some(Nome::Local(id)) => Some(cx.local(id).tipo),
                _ => None,
            },
            _ => None,
        },
        ast::ForInTarget::Pattern { .. } => return false,
    };
    // A variável declarada sem tipo escrito tem o tipo do elemento: cabe.
    let Some(var) = var else { return matches!(target, ast::ForInTarget::Declared { .. }) };
    let Some(el) = inf.como_instancia_de(base, Some(classe)).map(|a| a[0]) else { return true };
    let tearoff = matches!(inf.table.get(var), Type::Function { .. } | Type::FutureOr { .. })
        && matches!(inf.table.get(el), Type::Interface { .. } | Type::ExtensionType { .. } | Type::TypeParameter { .. } | Type::Intersection { .. });
    if !tearoff && !inf.atribuivel(el, var) {
        inf.aviso_com_args(ce::FOR_IN_OF_INVALID_ELEMENT_TYPE, sp, &[crate::exibicao::Arg::Tipo(base), crate::exibicao::Arg::from(nome), crate::exibicao::Arg::Tipo(var)]);
        return false;
    }
    true
}

/// `return e;` / `return;`.
fn retorno(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, e: Option<ExprId>, span: dartforge_diagnostics::Span) {
    let Some(fc) = cx.funcoes.last().cloned() else {
        if let Some(e) = e {
            inferir_livre(inf, cx, e);
        }
        return;
    };
    match e {
        Some(e) => {
            let t = inferir(inf, cx, e, fc.contexto_retorno);
            match fc.retorno {
                Some(_) => {
                    let _ = span;
                    super::funcoes::verificar_retorno(inf, cx, &fc, e, t);
                }
                None => {
                    if let Some(f) = cx.funcoes.last_mut() {
                        f.expressoes_retornadas.push((e, t));
                    }
                    let t = if fc.modificador == AsyncModifier::Async { inf.flatten(t) } else { t };
                    if let Some(f) = cx.funcoes.last_mut() {
                        f.retornados.push(t);
                    }
                }
            }
        }
        None => {
            // `RETURN_WITHOUT_VALUE` (`_checkReturnWithoutValue`,
            // `an611:src/error/return_type_verifier.dart:277-295`): num
            // executável declarado não gerador, com retorno (o valor futuro,
            // em `async`) que não é `void`, `dynamic` nem `Null`.
            if let (Some(t), Some(_)) = (fc.retorno, fc.executavel.as_ref()) {
                let gerador = matches!(fc.modificador, AsyncModifier::SyncStar | AsyncModifier::AsyncStar);
                if !gerador {
                    let tv = if fc.modificador == AsyncModifier::Async { inf.tipo_valor_futuro(t) } else { t };
                    if !matches!(inf.table.get(tv), Type::Void | Type::Dynamic | Type::Null) {
                        let sp = dartforge_diagnostics::Span { start: span.start, end: span.start + "return".len() };
                        inf.aviso_com_codigo(dartforge_diagnostics::codigos::compile_time_error::RETURN_WITHOUT_VALUE, sp, &[]);
                    }
                }
            }
            if let Some(f) = cx.funcoes.last_mut() {
                f.retorno_vazio = true;
            }
        }
    }
}

// -------------------------------------------------------------------
// Varredura sintática: variáveis escritas (assignedIn/capturedIn)
// -------------------------------------------------------------------

pub(crate) enum Parte {
    Expr(ExprId),
    Stmt(StmtId),
}

/// Uma escrita: o nome e o offset do nome na declaração a que ela se refere,
/// quando essa declaração está dentro da região varrida (a mais interna com
/// esse nome, pelo escopo léxico); `None` quando é de fora (parâmetro do
/// membro, variável externa). Sem isso, `x = …` numa closure `test(…)`
/// contaminava um `x` homônimo de outra closure do mesmo `main`.
pub(crate) type Escrita = (SymbolId, Option<usize>);

/// O local de `cx` a que a escrita se refere, visto do ponto corrente.
pub(crate) fn local_da_escrita(cx: &Corpo, e: Escrita) -> Option<LocalId> {
    let Some(Nome::Local(id)) = cx.buscar(e.0) else { return None };
    match e.1 {
        Some(o) if cx.local(id).offset != o => None,
        _ => Some(id),
    }
}

/// Locais escritos nas partes (e os escritos dentro de closures nelas).
pub(crate) fn escritas_em(inf: &BodyInferrer<'_>, cx: &Corpo, partes: &[Parte]) -> (Vec<LocalId>, Vec<LocalId>) {
    let mut v = Varredura::nova(&inf.program.unit(cx.unit).ast);
    for p in partes {
        match p {
            Parte::Expr(e) => v.expr(*e, false),
            Parte::Stmt(s) => v.stmt(*s, false),
        }
    }
    let ids = |es: &[Escrita]| -> Vec<LocalId> {
        let mut out = Vec::new();
        for e in es {
            if let Some(id) = local_da_escrita(cx, *e) {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    };
    (ids(&v.fora), ids(&v.dentro))
}

/// Escritas dentro de uma função (para a captura de escrita): os parâmetros
/// dela sombreiam os de fora no corpo inteiro (resolvem para eles mesmos).
pub(crate) fn nomes_escritos_em_funcao(inf: &BodyInferrer<'_>, unit: UnitId, f: &ast::Function) -> Vec<Escrita> {
    let mut v = Varredura::nova(&inf.program.unit(unit).ast);
    v.escopos.push(Vec::new());
    v.declarar_parametros(f);
    v.corpo(&f.body, false);
    let mut out = v.fora;
    for e in v.dentro {
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

/// Escritas num corpo, separadas: `(fora de literais de função, dentro de
/// literais)` — `assignedVariables.anywhere.written` e `.captured` do
/// analyzer.
pub(crate) fn nomes_escritos_separados(inf: &BodyInferrer<'_>, unit: UnitId, body: &ast::FunctionBody) -> (Vec<Escrita>, Vec<Escrita>) {
    let mut v = Varredura::nova(&inf.program.unit(unit).ast);
    v.corpo(body, false);
    (v.fora, v.dentro)
}

/// Varredura sintática das escritas, com os escopos léxicos declarados
/// dentro da região (blocos, `for`, `catch`, `case`, closures e funções
/// locais).
struct Varredura<'a> {
    a: &'a ast::Ast,
    escopos: Vec<Vec<(SymbolId, usize)>>,
    fora: Vec<Escrita>,
    dentro: Vec<Escrita>,
}

impl<'a> Varredura<'a> {
    fn nova(a: &'a ast::Ast) -> Self {
        Varredura { a, escopos: vec![Vec::new()], fora: Vec::new(), dentro: Vec::new() }
    }

    fn declarar(&mut self, n: ast::Name) {
        if let Some(e) = self.escopos.last_mut() {
            e.push((n.sym, n.span.start));
        }
    }

    fn declarar_parametros(&mut self, f: &ast::Function) {
        for p in f.parameters.iter().flat_map(|ps| ps.iter()) {
            if let Some(n) = p.name {
                self.declarar(n);
            }
        }
    }

    fn escrever(&mut self, n: ast::Name, dentro: bool) {
        let decl = self
            .escopos
            .iter()
            .rev()
            .find_map(|e| e.iter().rev().find(|(s, _)| *s == n.sym).map(|(_, o)| *o));
        let e = (n.sym, decl);
        let v = if dentro { &mut self.dentro } else { &mut self.fora };
        if !v.contains(&e) {
            v.push(e);
        }
    }

    fn com_escopo(&mut self, f: impl FnOnce(&mut Self)) {
        self.escopos.push(Vec::new());
        f(self);
        self.escopos.pop();
    }

    fn corpo(&mut self, body: &ast::FunctionBody, dentro: bool) {
        match body {
            ast::FunctionBody::Block(b) => self.stmt(*b, dentro),
            ast::FunctionBody::Expression(x) => self.expr(*x, dentro),
            _ => {}
        }
    }

    /// Closure ou função local: o que ela escreve conta como escrito em
    /// closure; os parâmetros dela são declarações próprias.
    fn funcao(&mut self, f: &ast::Function) {
        self.com_escopo(|v| {
            v.declarar_parametros(f);
            v.corpo(&f.body, true);
        });
    }

    fn escrever_alvo(&mut self, alvo: ExprId, dentro: bool) {
        let mut x = alvo;
        loop {
            match &self.a.expr(x).kind {
                ExprKind::Parenthesized(i) => x = *i,
                ExprKind::Identifier(n) => {
                    let n = *n;
                    self.escrever(n, dentro);
                    return;
                }
                _ => return,
            }
        }
    }

    /// Variáveis de um padrão: escritas (`PatternAssign`) ou declarações.
    fn padrao(&mut self, p: ast::PatternId, dentro: bool, declara: bool) {
        use ast::PatternKind as P;
        let a = self.a;
        match &a.pattern(p).kind {
            P::Variable { name, var_, final_, ty } => {
                // `var`/`final`/tipo numa atribuição também declara
                // (`DeclaredVariablePattern`).
                if declara || *var_ || *final_ || ty.is_some() {
                    self.declarar(*name);
                } else {
                    self.escrever(*name, dentro);
                }
            }
            P::Or(x, y) | P::And(x, y) => {
                self.padrao(*x, dentro, declara);
                self.padrao(*y, dentro, declara);
            }
            P::NullCheck(x) | P::NullAssert(x) | P::Parenthesized(x) => self.padrao(*x, dentro, declara),
            P::Cast { pattern, .. } => self.padrao(*pattern, dentro, declara),
            P::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => {
                            self.padrao(*x, dentro, declara)
                        }
                        _ => {}
                    }
                }
            }
            P::Map { entries, .. } => {
                for en in entries.iter() {
                    self.padrao(en.value, dentro, declara);
                }
            }
            P::Record { fields } | P::Object { fields, .. } => {
                for f in fields.iter() {
                    self.padrao(f.pattern, dentro, declara);
                }
            }
            _ => {}
        }
    }

    fn expr(&mut self, e: ExprId, dentro: bool) {
        let a = self.a;
        match &a.expr(e).kind {
            ExprKind::Assign { target, value, .. } => {
                self.escrever_alvo(*target, dentro);
                self.expr(*target, dentro);
                self.expr(*value, dentro);
            }
            ExprKind::Unary { op, operand } => {
                if matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec) {
                    self.escrever_alvo(*operand, dentro);
                }
                self.expr(*operand, dentro);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.padrao(*pattern, dentro, false);
                self.expr(*value, dentro);
            }
            ExprKind::FunctionExpression(f) => self.funcao(a.function(*f)),
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x, dentro),
            ExprKind::Property { target, .. } => self.expr(*target, dentro),
            ExprKind::Index { target, index, .. } => {
                self.expr(*target, dentro);
                self.expr(*index, dentro);
            }
            ExprKind::Call { target, arguments } => {
                self.expr(*target, dentro);
                for x in arguments.args.iter() {
                    self.expr(x.value, dentro);
                }
            }
            ExprKind::InstanceCreation { arguments, .. } => {
                for x in arguments.args.iter() {
                    self.expr(x.value, dentro);
                }
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target, dentro),
            ExprKind::Binary { left, right, .. } => {
                self.expr(*left, dentro);
                self.expr(*right, dentro);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(*condition, dentro);
                self.expr(*then, dentro);
                self.expr(*else_, dentro);
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.expr(*value, dentro),
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(*target, dentro);
                for s in sections.iter() {
                    self.expr(*s, dentro);
                }
            }
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x, dentro);
                    }
                }
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    self.elemento(el, dentro);
                }
            }
            ExprKind::Record { positional, named, .. } => {
                for x in positional.iter() {
                    self.expr(*x, dentro);
                }
                for (_, x) in named.iter() {
                    self.expr(*x, dentro);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value, dentro);
                for c in cases.iter() {
                    self.com_escopo(|v| {
                        v.padrao(c.pattern, dentro, true);
                        if let Some(g) = c.guard {
                            v.expr(g, dentro);
                        }
                        v.expr(c.body, dentro);
                    });
                }
            }
            _ => {}
        }
    }

    fn elemento(&mut self, el: &ast::CollectionElement, dentro: bool) {
        use ast::CollectionElement as C;
        match el {
            C::Expression(x) | C::NullAwareExpression(x) => self.expr(*x, dentro),
            C::MapEntry { key, value, .. } => {
                self.expr(*key, dentro);
                self.expr(*value, dentro);
            }
            C::Spread { value, .. } => self.expr(*value, dentro),
            C::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition, dentro);
                self.com_escopo(|v| {
                    if let Some(p) = case_pattern {
                        v.padrao(*p, dentro, true);
                    }
                    if let Some(g) = guard {
                        v.expr(*g, dentro);
                    }
                    v.elemento(then, dentro);
                });
                if let Some(e) = else_ {
                    self.elemento(e, dentro);
                }
            }
            C::For { init, condition, updates, body, .. } => self.com_escopo(|v| {
                match init {
                    Some(ast::ForInit::Expression(x)) => v.expr(*x, dentro),
                    Some(ast::ForInit::Variables(vl)) => v.variaveis(vl, dentro),
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        v.expr(*value, dentro);
                        v.padrao(*pattern, dentro, true);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    v.expr(*c, dentro);
                }
                for u in updates.iter() {
                    v.expr(*u, dentro);
                }
                v.elemento(body, dentro);
            }),
            C::ForIn { target, iterable, body, .. } => {
                self.expr(*iterable, dentro);
                self.com_escopo(|v| {
                    v.alvo_for_in(target, dentro);
                    v.elemento(body, dentro);
                });
            }
        }
    }

    fn variaveis(&mut self, vl: &ast::VariableList, dentro: bool) {
        for x in vl.variables.iter() {
            if let Some(i) = x.initializer {
                self.expr(i, dentro);
            }
            self.declarar(x.name);
        }
    }

    fn alvo_for_in(&mut self, target: &ast::ForInTarget, dentro: bool) {
        match target {
            ast::ForInTarget::Declared { name, .. } => self.declarar(*name),
            ast::ForInTarget::Expression(x) => self.escrever_alvo(*x, dentro),
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }

    fn stmt(&mut self, s: StmtId, dentro: bool) {
        let a = self.a;
        match &a.stmt(s).kind {
            StmtKind::Block(ss) => self.com_escopo(|v| {
                for x in ss.iter() {
                    v.stmt(*x, dentro);
                }
            }),
            StmtKind::Variables(vl) => self.variaveis(vl, dentro),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.expr(*value, dentro);
                self.padrao(*pattern, dentro, true);
            }
            StmtKind::Function(f) => {
                let f = a.function(*f);
                if let Some(n) = f.name {
                    self.declarar(n);
                }
                self.funcao(f);
            }
            StmtKind::Expression(e) => self.expr(*e, dentro),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition, dentro);
                self.com_escopo(|v| {
                    if let Some(p) = case_pattern {
                        v.padrao(*p, dentro, true);
                    }
                    if let Some(g) = guard {
                        v.expr(*g, dentro);
                    }
                    v.stmt(*then, dentro);
                });
                if let Some(e) = else_ {
                    self.stmt(*e, dentro);
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => self.com_escopo(|v| {
                match init {
                    Some(ast::ForInit::Expression(x)) => v.expr(*x, dentro),
                    Some(ast::ForInit::Variables(vl)) => v.variaveis(vl, dentro),
                    Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                        v.expr(*value, dentro);
                        v.padrao(*pattern, dentro, true);
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    v.expr(*c, dentro);
                }
                for u in updates.iter() {
                    v.expr(*u, dentro);
                }
                v.stmt(*body, dentro);
            }),
            StmtKind::ForIn { target, iterable, body, .. } => {
                self.expr(*iterable, dentro);
                self.com_escopo(|v| {
                    v.alvo_for_in(target, dentro);
                    v.stmt(*body, dentro);
                });
            }
            StmtKind::While { condition, body } | StmtKind::DoWhile { body, condition } => {
                self.expr(*condition, dentro);
                self.stmt(*body, dentro);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value, dentro);
                for c in cases.iter() {
                    self.com_escopo(|v| {
                        if let Some(g) = c.guard {
                            v.expr(g, dentro);
                        }
                        for x in c.body.iter() {
                            v.stmt(*x, dentro);
                        }
                    });
                }
            }
            StmtKind::Return(Some(e)) => self.expr(*e, dentro),
            StmtKind::Yield { value, .. } => self.expr(*value, dentro),
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(*body, dentro);
                for c in catches.iter() {
                    self.com_escopo(|v| {
                        if let Some(n) = c.exception {
                            v.declarar(n);
                        }
                        if let Some(n) = c.stack_trace {
                            v.declarar(n);
                        }
                        v.stmt(c.body, dentro);
                    });
                }
                if let Some(f) = finally_ {
                    self.stmt(*f, dentro);
                }
            }
            StmtKind::Labeled { body, .. } => self.stmt(*body, dentro),
            StmtKind::Assert { condition, message } => {
                self.expr(*condition, dentro);
                if let Some(m) = message {
                    self.expr(*m, dentro);
                }
            }
            _ => {}
        }
    }
}
