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
use crate::resolved::LocalId;
use crate::table::{Type, TypeId};
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, ExprKind, StmtId, StmtKind, UnaryOp};
use dartforge_intern::SymbolId;

/// Infere uma instrução.
pub(crate) fn inferir_instrucao(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, s: StmtId) {
    let a = &inf.program.unit(cx.unit).ast;
    let st = a.stmt(s);
    let span = st.span;
    match &st.kind {
        StmtKind::Block(stmts) => {
            cx.empurrar_escopo();
            // O escopo do bloco contém todas as suas declarações locais.
            for &x in stmts.iter() {
                match &inf.program.unit(cx.unit).ast.stmt(x).kind {
                    StmtKind::Variables(vl) => {
                        for v in vl.variables.iter() {
                            cx.declarar_adiante(v.name.sym);
                        }
                    }
                    StmtKind::Function(f) => {
                        if let Some(n) = inf.program.unit(cx.unit).ast.function(*f).name {
                            cx.declarar_adiante(n.sym);
                        }
                    }
                    _ => {}
                }
            }
            let mut avisou = false;
            for &x in stmts.iter() {
                if !cx.fluxo.alcancavel && !avisou {
                    let sx = inf.program.unit(cx.unit).ast.stmt(x).span;
                    inf.aviso(DEAD_CODE.template.to_string(), sx);
                    avisou = true;
                }
                inferir_instrucao(inf, cx, x);
            }
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
            ramo(inf, cx, *then);
            if case_pattern.is_some() {
                cx.tirar_escopo();
            }
            let depois_then = std::mem::replace(&mut cx.fluxo, ff);
            if let Some(e) = else_ {
                ramo(inf, cx, *e);
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
            ramo(inf, cx, *body);
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
            ramo(inf, cx, *body);
            let alvo = cx.saltos.pop().unwrap();
            let mut conts = vec![cx.fluxo.clone()];
            conts.extend(alvo.continues);
            let base = cx.fluxo.clone();
            cx.fluxo = inf.juntar_todos(&base, &conts);
            for u in updates.iter() {
                inferir_livre(inf, cx, *u);
            }
            let mut saidas = vec![ff];
            saidas.extend(alvo.breaks);
            cx.fluxo = inf.juntar_todos(&base, &saidas);
            cx.tirar_escopo();
        }
        StmtKind::ForIn { await_, target, iterable, body } => {
            let rotulos = rotulos_pendentes(cx);
            cx.empurrar_escopo();
            let antes_do_corpo = {
                cabecalho_for_in(inf, cx, target, *iterable, *await_);
                let (escritas, capturadas) = escritas_em(inf, cx, &[Parte::Stmt(*body)]);
                cx.fluxo.juncao_conservadora(&escritas, &capturadas);
                cx.fluxo.clone()
            };
            cx.saltos.push(AlvoSalto { rotulos, laco: true, e_switch: false, breaks: Vec::new(), continues: Vec::new() });
            ramo(inf, cx, *body);
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
            let depois_valor = cx.fluxo.clone();
            let mut nao_casou = depois_valor.clone();
            cx.saltos.push(AlvoSalto { rotulos, laco: false, e_switch: true, breaks: Vec::new(), continues: Vec::new() });
            let mut tem_default = false;
            let mut i = 0;
            let n = cases.len();
            let mut saidas: Vec<Fluxo> = Vec::new();
            while i < n {
                // Casos agrupados: os vazios seguem para o próximo corpo.
                let mut j = i;
                let mut entradas: Vec<Fluxo> = Vec::new();
                cx.empurrar_escopo();
                loop {
                    let c = &cases[j];
                    cx.fluxo = nao_casou.clone();
                    match c.pattern {
                        Some(p) => {
                            let (vf, ff) = padroes::caso(inf, cx, p, t, c.guard, Some(*value));
                            entradas.push(vf);
                            nao_casou = ff;
                        }
                        None => {
                            tem_default = true;
                            entradas.push(nao_casou.clone());
                            nao_casou = nao_casou.inalcancavel();
                        }
                    }
                    if !c.body.is_empty() || j + 1 >= n {
                        break;
                    }
                    j += 1;
                }
                let base = depois_valor.clone();
                cx.fluxo = inf.juntar_todos(&base, &entradas);
                let corpo: Vec<StmtId> = cases[j].body.to_vec();
                let mut avisou = false;
                for s in corpo {
                    if !cx.fluxo.alcancavel && !avisou {
                        avisou = true;
                    }
                    inferir_instrucao(inf, cx, s);
                }
                if cx.fluxo.alcancavel {
                    saidas.push(cx.fluxo.clone());
                }
                cx.tirar_escopo();
                i = j + 1;
            }
            let alvo = cx.saltos.pop().unwrap();
            saidas.extend(alvo.breaks);
            if !tem_default && !switch_exaustivo(inf, t) {
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
            if *star {
                let ctx = if m == AsyncModifier::AsyncStar { inf.fluxo_de(k) } else { inf.iteravel(k) };
                let t = inferir(inf, cx, *value, ctx);
                let classe = if m == AsyncModifier::AsyncStar { inf.core.stream_class } else { inf.core.iterable_class };
                let el = if inf.e_dynamic(t) { inf.core.dynamic_ } else { inf.como_instancia_de(t, classe).map(|a| a[0]).unwrap_or(inf.core.dynamic_) };
                if let Some(f) = cx.funcoes.last_mut() {
                    f.retornados.push(el);
                }
            } else {
                let t = inferir(inf, cx, *value, k);
                if let Some(f) = cx.funcoes.last_mut() {
                    f.retornados.push(t);
                }
            }
        }
        StmtKind::Try { body, catches, finally_ } => {
            let antes = cx.fluxo.clone();
            let (escritas, capturadas) = escritas_em(inf, cx, &[Parte::Stmt(*body)]);
            ramo(inf, cx, *body);
            let depois_try = cx.fluxo.clone();
            let mut saidas = vec![depois_try];
            for c in catches.iter() {
                let mut f = antes.clone();
                f.juncao_conservadora(&escritas, &capturadas);
                cx.fluxo = f;
                cx.empurrar_escopo();
                let tipo_ex = match c.on_type {
                    Some(t) => inf.tipo_de_anotacao(cx, t),
                    None => inf.core.object,
                };
                if let Some(n) = &c.exception {
                    declarar_local(inf, cx, Local { nome: n.sym, tipo: tipo_ex, final_: true, late: false, const_: false, offset: n.span.start, funcao_local: false }, true);
                }
                if let Some(n) = &c.stack_trace {
                    let st = stack_trace(inf);
                    declarar_local(inf, cx, Local { nome: n.sym, tipo: st, final_: true, late: false, const_: false, offset: n.span.start, funcao_local: false }, true);
                }
                inferir_instrucao(inf, cx, c.body);
                cx.tirar_escopo();
                saidas.push(cx.fluxo.clone());
            }
            let apos_catches = inf.juntar_todos(&antes, &saidas);
            if let Some(fin) = finally_ {
                let mut f = inf.juntar(&apos_catches, &antes);
                f.juncao_conservadora(&escritas, &capturadas);
                cx.fluxo = f;
                ramo(inf, cx, *fin);
                if cx.fluxo.alcancavel {
                    // O `finally` completa: vale o estado depois do `try`.
                    let alcanca = apos_catches.alcancavel;
                    let mut r = apos_catches;
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
            expr::condicao_verificada(inf, cx, *condition);
            if let Some(m) = message {
                inferir_livre(inf, cx, *m);
            }
            cx.fluxo = antes;
        }
        StmtKind::Empty => {}
    }
}

/// Um ramo (`then`, `else`, corpo de laço) em escopo próprio.
fn ramo(inf: &mut BodyInferrer<'_>, cx: &mut Corpo, s: StmtId) {
    cx.empurrar_escopo();
    inferir_instrucao(inf, cx, s);
    cx.tirar_escopo();
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

/// `switch` sem `default` exaustivo pelo tipo (enum, `bool`, selado): a
/// saída "nenhum caso casou" é inalcançável. Aproximação: tipos cuja
/// exaustividade o analyzer pode provar são tratados como exaustivos.
fn switch_exaustivo(inf: &mut BodyInferrer<'_>, t: TypeId) -> bool {
    match inf.table.get(t) {
        Type::Interface { class, nullable: false, .. } => {
            let c = inf.program.class(*class);
            c.kind == dartforge_elements::model::ClassKind::Enum || c.modifiers.sealed || Some(*class) == inf.core.bool_class
        }
        _ => false,
    }
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
            let t = if !vl.late && expr::e_forma_de_condicao(inf, cx, init) {
                let (sim, nao) = expr::condicao(inf, cx, init);
                cx.fluxo = inf.juntar(&sim, &nao);
                condicao_guardada = Some((sim, nao));
                inf.body_types.units[cx.unit.0 as usize].get_type(init).unwrap_or(inf.core.bool_)
            } else {
                inferir(inf, cx, init, declarado.unwrap_or(u))
            };
            escrito = Some(t);
            if let Some(d) = declarado {
                let sp = inf.span_expr(cx.unit, init);
                inf.verificar_atribuivel(t, d, sp, INVALID_ASSIGNMENT.template);
            } else {
                tipo = Some(if matches!(inf.table.get(t), Type::Null) { inf.core.dynamic_ } else { t });
            }
            if vl.const_ {
                colecoes_const(inf, cx, init);
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
                inf.escrever_fluxo(&mut f, id, tipo, t, toi);
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
    let el = if inf.e_dynamic(t) {
        inf.core.dynamic_
    } else {
        inf.como_instancia_de(t, classe).map(|a| a[0]).unwrap_or(inf.core.dynamic_)
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
                    let decl = cx.local(id).tipo;
                    expr::registrar(inf, cx, *e, decl);
                    let mut f = std::mem::replace(&mut cx.fluxo, Fluxo::alcancavel());
                    inf.atribuir_fluxo(&mut f, id, decl, el);
                    cx.fluxo = f;
                    return;
                }
            }
            inferir_livre(inf, cx, *e);
        }
    }
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
                Some(r) => {
                    let (valor, esperado) = match fc.modificador {
                        AsyncModifier::Async => {
                            let fv = inf.tipo_valor_futuro_esquema(r);
                            let ft = inf.flatten(t);
                            (ft, fv)
                        }
                        _ => (t, r),
                    };
                    if !matches!(inf.table.get(esperado), Type::Void | Type::Dynamic) && !inf.atribuivel(valor, esperado) {
                        // `return v` em função `void`/`dynamic` não se checa; `Future<void>` também não.
                        let msg = format!(
                            "{}: retorno '{}' incompatível com '{}'",
                            RETURN_OF_INVALID_TYPE.template,
                            inf.table.format(valor, inf.interner, inf.program),
                            inf.table.format(esperado, inf.interner, inf.program)
                        );
                        inf.aviso(msg, span);
                    }
                }
                None => {
                    let t = if fc.modificador == AsyncModifier::Async { inf.flatten(t) } else { t };
                    if let Some(f) = cx.funcoes.last_mut() {
                        f.retornados.push(t);
                    }
                }
            }
        }
        None => {
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

/// Locais escritos nas partes (e os escritos dentro de closures nelas).
fn escritas_em(inf: &BodyInferrer<'_>, cx: &Corpo, partes: &[Parte]) -> (Vec<LocalId>, Vec<LocalId>) {
    let a = &inf.program.unit(cx.unit).ast;
    let mut nomes = Vec::new();
    let mut em_closure = Vec::new();
    for p in partes {
        match p {
            Parte::Expr(e) => varrer_expr(a, *e, &mut nomes, &mut em_closure, false),
            Parte::Stmt(s) => varrer_stmt(a, *s, &mut nomes, &mut em_closure, false),
        }
    }
    let ids = |ns: &[SymbolId]| -> Vec<LocalId> {
        let mut v = Vec::new();
        for n in ns {
            if let Some(Nome::Local(id)) = cx.buscar(*n) {
                if !v.contains(&id) {
                    v.push(id);
                }
            }
        }
        v
    };
    (ids(&nomes), ids(&em_closure))
}

/// Nomes escritos dentro de uma função (para a captura de escrita); os
/// parâmetros dela sombreiam os de fora no corpo inteiro.
pub(crate) fn nomes_escritos_em_funcao(inf: &BodyInferrer<'_>, unit: UnitId, f: &ast::Function) -> Vec<SymbolId> {
    let mut v = nomes_escritos_em_corpo(inf, unit, &f.body);
    let ps = parametros_de(f);
    v.retain(|n| !ps.contains(n));
    v
}

fn parametros_de(f: &ast::Function) -> Vec<SymbolId> {
    f.parameters.iter().flat_map(|ps| ps.iter()).filter_map(|p| p.name.map(|n| n.sym)).collect()
}

/// Closure ou função local: o que ela escreve, menos os próprios
/// parâmetros, conta como escrito em closure.
fn varrer_funcao_aninhada(a: &ast::Ast, f: &ast::Function, em_closure: &mut Vec<SymbolId>) {
    let (mut n, mut em) = (Vec::new(), Vec::new());
    match &f.body {
        ast::FunctionBody::Block(b) => varrer_stmt(a, *b, &mut n, &mut em, true),
        ast::FunctionBody::Expression(x) => varrer_expr(a, *x, &mut n, &mut em, true),
        _ => {}
    }
    let ps = parametros_de(f);
    for x in n.into_iter().chain(em) {
        if !ps.contains(&x) && !em_closure.contains(&x) {
            em_closure.push(x);
        }
    }
}

/// Nomes escritos num corpo, separados: `(fora de literais de função,
/// dentro de literais)` — `assignedVariables.anywhere.written` e
/// `.captured` do analyzer.
pub(crate) fn nomes_escritos_separados(inf: &BodyInferrer<'_>, unit: UnitId, body: &ast::FunctionBody) -> (Vec<SymbolId>, Vec<SymbolId>) {
    let a = &inf.program.unit(unit).ast;
    let mut nomes = Vec::new();
    let mut em_closure = Vec::new();
    match body {
        ast::FunctionBody::Block(s) => varrer_stmt(a, *s, &mut nomes, &mut em_closure, false),
        ast::FunctionBody::Expression(e) => varrer_expr(a, *e, &mut nomes, &mut em_closure, false),
        _ => {}
    }
    (nomes, em_closure)
}

/// Nomes escritos num corpo (inclusive dentro de closures dele).
pub(crate) fn nomes_escritos_em_corpo(inf: &BodyInferrer<'_>, unit: UnitId, body: &ast::FunctionBody) -> Vec<SymbolId> {
    let a = &inf.program.unit(unit).ast;
    let mut nomes = Vec::new();
    let mut em_closure = Vec::new();
    match body {
        ast::FunctionBody::Block(s) => varrer_stmt(a, *s, &mut nomes, &mut em_closure, true),
        ast::FunctionBody::Expression(e) => varrer_expr(a, *e, &mut nomes, &mut em_closure, true),
        _ => {}
    }
    nomes.extend(em_closure);
    nomes
}

fn escrever_nome(a: &ast::Ast, alvo: ExprId, nomes: &mut Vec<SymbolId>, em_closure: &mut Vec<SymbolId>, dentro: bool) {
    let mut x = alvo;
    loop {
        match &a.expr(x).kind {
            ExprKind::Parenthesized(i) => x = *i,
            ExprKind::Identifier(n) => {
                let v = if dentro { em_closure } else { nomes };
                if !v.contains(&n.sym) {
                    v.push(n.sym);
                }
                return;
            }
            _ => return,
        }
    }
}

fn varrer_padrao(a: &ast::Ast, p: ast::PatternId, nomes: &mut Vec<SymbolId>, em_closure: &mut Vec<SymbolId>, dentro: bool) {
    use ast::PatternKind as P;
    match &a.pattern(p).kind {
        P::Variable { name, .. } => {
            let v = if dentro { &mut *em_closure } else { &mut *nomes };
            if !v.contains(&name.sym) {
                v.push(name.sym);
            }
        }
        P::Or(x, y) | P::And(x, y) => {
            varrer_padrao(a, *x, nomes, em_closure, dentro);
            varrer_padrao(a, *y, nomes, em_closure, dentro);
        }
        P::NullCheck(x) | P::NullAssert(x) | P::Parenthesized(x) => varrer_padrao(a, *x, nomes, em_closure, dentro),
        P::Cast { pattern, .. } => varrer_padrao(a, *pattern, nomes, em_closure, dentro),
        P::List { elements, .. } => {
            for el in elements.iter() {
                match el {
                    ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => varrer_padrao(a, *x, nomes, em_closure, dentro),
                    _ => {}
                }
            }
        }
        P::Map { entries, .. } => {
            for en in entries.iter() {
                varrer_padrao(a, en.value, nomes, em_closure, dentro);
            }
        }
        P::Record { fields } | P::Object { fields, .. } => {
            for f in fields.iter() {
                varrer_padrao(a, f.pattern, nomes, em_closure, dentro);
            }
        }
        _ => {}
    }
}

fn varrer_expr(a: &ast::Ast, e: ExprId, nomes: &mut Vec<SymbolId>, em_closure: &mut Vec<SymbolId>, dentro: bool) {
    let rec = |x: ExprId, nomes: &mut Vec<SymbolId>, em: &mut Vec<SymbolId>| varrer_expr(a, x, nomes, em, dentro);
    match &a.expr(e).kind {
        ExprKind::Assign { target, value, .. } => {
            escrever_nome(a, *target, nomes, em_closure, dentro);
            rec(*target, nomes, em_closure);
            rec(*value, nomes, em_closure);
        }
        ExprKind::Unary { op, operand } => {
            if matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec) {
                escrever_nome(a, *operand, nomes, em_closure, dentro);
            }
            rec(*operand, nomes, em_closure);
        }
        ExprKind::PatternAssign { pattern, value } => {
            varrer_padrao(a, *pattern, nomes, em_closure, dentro);
            rec(*value, nomes, em_closure);
        }
        ExprKind::FunctionExpression(f) => varrer_funcao_aninhada(a, a.function(*f), em_closure),
        ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => rec(*x, nomes, em_closure),
        ExprKind::Property { target, .. } => rec(*target, nomes, em_closure),
        ExprKind::Index { target, index, .. } => {
            rec(*target, nomes, em_closure);
            rec(*index, nomes, em_closure);
        }
        ExprKind::Call { target, arguments } => {
            rec(*target, nomes, em_closure);
            for x in arguments.args.iter() {
                rec(x.value, nomes, em_closure);
            }
        }
        ExprKind::InstanceCreation { arguments, .. } => {
            for x in arguments.args.iter() {
                rec(x.value, nomes, em_closure);
            }
        }
        ExprKind::TypeArguments { target, .. } => rec(*target, nomes, em_closure),
        ExprKind::Binary { left, right, .. } => {
            rec(*left, nomes, em_closure);
            rec(*right, nomes, em_closure);
        }
        ExprKind::Conditional { condition, then, else_ } => {
            rec(*condition, nomes, em_closure);
            rec(*then, nomes, em_closure);
            rec(*else_, nomes, em_closure);
        }
        ExprKind::Is { value, .. } | ExprKind::As { value, .. } => rec(*value, nomes, em_closure),
        ExprKind::Cascade { target, sections, .. } => {
            rec(*target, nomes, em_closure);
            for s in sections.iter() {
                rec(*s, nomes, em_closure);
            }
        }
        ExprKind::String(lit) => {
            for p in lit.parts.iter() {
                if let ast::StringPart::Interpolation(x) = p {
                    rec(*x, nomes, em_closure);
                }
            }
        }
        ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
            for el in elements.iter() {
                varrer_elemento(a, el, nomes, em_closure, dentro);
            }
        }
        ExprKind::Record { positional, named, .. } => {
            for x in positional.iter() {
                rec(*x, nomes, em_closure);
            }
            for (_, x) in named.iter() {
                rec(*x, nomes, em_closure);
            }
        }
        ExprKind::Switch { value, cases } => {
            rec(*value, nomes, em_closure);
            for c in cases.iter() {
                if let Some(g) = c.guard {
                    rec(g, nomes, em_closure);
                }
                rec(c.body, nomes, em_closure);
            }
        }
        _ => {}
    }
}

fn varrer_elemento(a: &ast::Ast, el: &ast::CollectionElement, nomes: &mut Vec<SymbolId>, em_closure: &mut Vec<SymbolId>, dentro: bool) {
    use ast::CollectionElement as C;
    match el {
        C::Expression(x) | C::NullAwareExpression(x) => varrer_expr(a, *x, nomes, em_closure, dentro),
        C::MapEntry { key, value, .. } => {
            varrer_expr(a, *key, nomes, em_closure, dentro);
            varrer_expr(a, *value, nomes, em_closure, dentro);
        }
        C::Spread { value, .. } => varrer_expr(a, *value, nomes, em_closure, dentro),
        C::If { condition, guard, then, else_, .. } => {
            varrer_expr(a, *condition, nomes, em_closure, dentro);
            if let Some(g) = guard {
                varrer_expr(a, *g, nomes, em_closure, dentro);
            }
            varrer_elemento(a, then, nomes, em_closure, dentro);
            if let Some(e) = else_ {
                varrer_elemento(a, e, nomes, em_closure, dentro);
            }
        }
        C::For { init, condition, updates, body, .. } => {
            if let Some(ast::ForInit::Expression(x)) = init {
                varrer_expr(a, *x, nomes, em_closure, dentro);
            }
            if let Some(c) = condition {
                varrer_expr(a, *c, nomes, em_closure, dentro);
            }
            for u in updates.iter() {
                varrer_expr(a, *u, nomes, em_closure, dentro);
            }
            varrer_elemento(a, body, nomes, em_closure, dentro);
        }
        C::ForIn { target, iterable, body, .. } => {
            if let ast::ForInTarget::Expression(x) = target {
                escrever_nome(a, *x, nomes, em_closure, dentro);
            }
            varrer_expr(a, *iterable, nomes, em_closure, dentro);
            varrer_elemento(a, body, nomes, em_closure, dentro);
        }
    }
}

fn varrer_stmt(a: &ast::Ast, s: StmtId, nomes: &mut Vec<SymbolId>, em_closure: &mut Vec<SymbolId>, dentro: bool) {
    let re = |x: ExprId, nomes: &mut Vec<SymbolId>, em: &mut Vec<SymbolId>| varrer_expr(a, x, nomes, em, dentro);
    let rs = |x: StmtId, nomes: &mut Vec<SymbolId>, em: &mut Vec<SymbolId>| varrer_stmt(a, x, nomes, em, dentro);
    match &a.stmt(s).kind {
        StmtKind::Block(ss) => {
            for x in ss.iter() {
                rs(*x, nomes, em_closure);
            }
        }
        StmtKind::Variables(vl) => {
            for v in vl.variables.iter() {
                if let Some(i) = v.initializer {
                    re(i, nomes, em_closure);
                }
            }
        }
        StmtKind::PatternVariables { value, .. } => re(*value, nomes, em_closure),
        StmtKind::Function(f) => varrer_funcao_aninhada(a, a.function(*f), em_closure),
        StmtKind::Expression(e) => re(*e, nomes, em_closure),
        StmtKind::If { condition, guard, then, else_, .. } => {
            re(*condition, nomes, em_closure);
            if let Some(g) = guard {
                re(*g, nomes, em_closure);
            }
            rs(*then, nomes, em_closure);
            if let Some(e) = else_ {
                rs(*e, nomes, em_closure);
            }
        }
        StmtKind::For { init, condition, updates, body, .. } => {
            if let Some(i) = init {
                match i {
                    ast::ForInit::Expression(x) => re(*x, nomes, em_closure),
                    ast::ForInit::Variables(vl) => {
                        for v in vl.variables.iter() {
                            if let Some(i) = v.initializer {
                                re(i, nomes, em_closure);
                            }
                        }
                    }
                }
            }
            if let Some(c) = condition {
                re(*c, nomes, em_closure);
            }
            for u in updates.iter() {
                re(*u, nomes, em_closure);
            }
            rs(*body, nomes, em_closure);
        }
        StmtKind::ForIn { target, iterable, body, .. } => {
            if let ast::ForInTarget::Expression(x) = target {
                escrever_nome(a, *x, nomes, em_closure, dentro);
            }
            re(*iterable, nomes, em_closure);
            rs(*body, nomes, em_closure);
        }
        StmtKind::While { condition, body } | StmtKind::DoWhile { body, condition } => {
            re(*condition, nomes, em_closure);
            rs(*body, nomes, em_closure);
        }
        StmtKind::Switch { value, cases } => {
            re(*value, nomes, em_closure);
            for c in cases.iter() {
                if let Some(g) = c.guard {
                    re(g, nomes, em_closure);
                }
                for x in c.body.iter() {
                    rs(*x, nomes, em_closure);
                }
            }
        }
        StmtKind::Return(Some(e)) => re(*e, nomes, em_closure),
        StmtKind::Yield { value, .. } => re(*value, nomes, em_closure),
        StmtKind::Try { body, catches, finally_ } => {
            rs(*body, nomes, em_closure);
            for c in catches.iter() {
                rs(c.body, nomes, em_closure);
            }
            if let Some(f) = finally_ {
                rs(*f, nomes, em_closure);
            }
        }
        StmtKind::Labeled { body, .. } => rs(*body, nomes, em_closure),
        StmtKind::Assert { condition, message } => {
            re(*condition, nomes, em_closure);
            if let Some(m) = message {
                re(*m, nomes, em_closure);
            }
        }
        _ => {}
    }
}
