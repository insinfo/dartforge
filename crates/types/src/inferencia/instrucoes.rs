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
fn escritas_em(inf: &BodyInferrer<'_>, cx: &Corpo, partes: &[Parte]) -> (Vec<LocalId>, Vec<LocalId>) {
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
            P::Variable { name, .. } => {
                if declara {
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
