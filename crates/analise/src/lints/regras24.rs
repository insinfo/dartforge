//! O vigésimo quarto lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), pela
//! semântica da unidade:
//!
//! * `specify_nonobvious_local_variable_types`: as variáveis locais (também
//!   as de `for` e `for-in`, de declaração por padrão e de `case`) sem tipo
//!   cujo inicializador não tem tipo óbvio (`super::obvio`).
//! * `use_string_buffers`: o `s += …` (de variável que não é local do corpo)
//!   e o `s = s + …` de `String` direto no corpo de um laço. Como no
//!   analyzer, o identificador do lado esquerdo de uma atribuição não tem
//!   elemento, então o segundo caso só casa um identificador não resolvido.
//! * `use_enums`: a classe que é um enum escrito à mão.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::ClassId;
use dartforge_frontend::ast::{
    AssignOp, Ast, BinaryOp, DeclId, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, MemberKind, PatternId, PatternKind, StmtId, StmtKind, StringPart,
};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::Type;
use std::collections::HashSet;

fn classe_da_decl(s: &super::Semantica<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == d))
}

/// Os padrões de variável declarada sem tipo (ou `dynamic`/`Null`) dentro de
/// `p`.
fn padroes_sem_tipo(s: &super::Semantica<'_>, a: &Ast, p: PatternId, saida: &mut Vec<Span>) {
    let mut vs = Vec::new();
    super::regras9::variaveis_do_padrao(a, p, &mut vs);
    for x in vs {
        let PatternKind::Variable { ty, .. } = &a.pattern(x).kind else { continue };
        let sem_tipo = match ty {
            None => true,
            Some(t) => s.corpo.tipos_de_anotacoes.get(t).is_none_or(|t| matches!(s.table.get(*t), Type::Dynamic | Type::Null)),
        };
        if sem_tipo {
            saida.push(a.pattern(x).span);
        }
    }
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let program = s.program;
    let table = s.table;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `specify_nonobvious_local_variable_types`.
    if ligada("specify_nonobvious_local_variable_types") {
        let mut achados: Vec<Span> = Vec::new();
        let obvio = |e: ExprId| super::obvio::tem_tipo_obvio(s, interner, a, e);
        let lista = |l: &dartforge_frontend::ast::VariableList, inicio: usize, achados: &mut Vec<Span>| {
            if let Some(t) = l.ty
                && s.corpo.tipos_de_anotacoes.get(&t).is_some_and(|x| !matches!(table.get(*x), Type::Null))
            {
                return;
            }
            let precisam: Vec<&dartforge_frontend::ast::Variable> = l.variables.iter().filter(|v| v.initializer.is_none_or(|i| !obvio(i))).collect();
            if precisam.is_empty() {
                return;
            }
            if l.variables.len() == 1 {
                let v = &l.variables[0];
                let fim = v.initializer.map_or(v.name.span.end, |i| a.expr(i).span.end);
                achados.push(Span { start: inicio, end: fim });
            } else {
                for v in precisam {
                    let fim = v.initializer.map_or(v.name.span.end, |i| a.expr(i).span.end);
                    achados.push(Span { start: v.name.span.start, end: fim });
                }
            }
        };
        let metadados = |k: usize| a.metadados_locais.iter().find(|(x, _)| x.0 as usize == k).map_or(&[][..], |(_, m)| &m[..]);
        let inicio_da_lista = |st_inicio: usize, meta: &[dartforge_frontend::ast::Annotation]| match meta.last() {
            Some(m) => dartforge_frontend::fonte::pular_brancos(u.fonte.as_bytes(), m.span.end),
            None => st_inicio,
        };
        for (k, st) in a.stmts.iter().enumerate() {
            match &st.kind {
                StmtKind::Variables(l) => lista(l, inicio_da_lista(st.span.start, metadados(k)), &mut achados),
                StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                    // A lista começa depois do `(`.
                    let ini = u.fonte[st.span.start..].find('(').map_or(st.span.start, |i| dartforge_frontend::fonte::pular_brancos(u.fonte.as_bytes(), st.span.start + i + 1));
                    lista(l, ini, &mut achados);
                }
                StmtKind::ForIn { target: ForInTarget::Declared { ty, name, .. }, iterable, .. } => {
                    let tipado = ty.and_then(|t| s.corpo.tipos_de_anotacoes.get(&t)).is_some_and(|x| !matches!(table.get(*x), Type::Dynamic));
                    if tipado || obvio(*iterable) {
                        continue;
                    }
                    let ini = u.fonte[st.span.start..].find('(').map_or(name.span.start, |i| dartforge_frontend::fonte::pular_brancos(u.fonte.as_bytes(), st.span.start + i + 1));
                    achados.push(Span { start: ini, end: name.span.end });
                }
                StmtKind::PatternVariables { pattern, value, .. } => {
                    if !obvio(*value) {
                        padroes_sem_tipo(s, a, *pattern, &mut achados);
                    }
                }
                StmtKind::Switch { value, cases } => {
                    if obvio(*value) {
                        continue;
                    }
                    for caso in cases.iter() {
                        let Some(p) = caso.pattern else { continue };
                        // Os padrões de declaração do caso inteiro (o padrão e
                        // as declarações por padrão do corpo).
                        padroes_sem_tipo(s, a, p, &mut achados);
                        for (k2, st2) in a.stmts.iter().enumerate() {
                            let _ = k2;
                            if st2.span.start >= caso.span.start
                                && st2.span.end <= caso.span.end
                                && let StmtKind::PatternVariables { pattern, .. } = &st2.kind
                            {
                                padroes_sem_tipo(s, a, *pattern, &mut achados);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        achados.sort_by_key(|x| (x.start, x.end));
        achados.dedup();
        for sp in achados {
            relatar(&c::SPECIFY_NONOBVIOUS_LOCAL_VARIABLE_TYPES, sp, &[]);
        }
    }

    // `use_string_buffers`.
    if ligada("use_string_buffers") {
        let e_string = |t: dartforge_types::table::TypeId| matches!(table.get(t), Type::Interface { class, .. } if Some(*class) == s.core.string_class);
        // Os corpos de laço (`do`, `for`, `for-in`, `while`).
        let corpos: Vec<StmtId> = a
            .stmts
            .iter()
            .filter_map(|st| match &st.kind {
                StmtKind::DoWhile { body, .. } | StmtKind::For { body, .. } | StmtKind::ForIn { body, .. } | StmtKind::While { body, .. } => Some(*body),
                _ => None,
            })
            .collect();
        let mut achados: Vec<Span> = Vec::new();
        for corpo in corpos {
            // As instruções visitadas: as do bloco (em ordem) ou a única.
            let instrucoes: Vec<StmtId> = match &a.stmt(corpo).kind {
                StmtKind::Block(cmds) => cmds.to_vec(),
                _ => vec![corpo],
            };
            let mut locais: HashSet<usize> = HashSet::new();
            for st in instrucoes {
                match &a.stmt(st).kind {
                    StmtKind::Variables(l) => {
                        for v in l.variables.iter() {
                            locais.insert(v.name.span.start);
                        }
                    }
                    StmtKind::Expression(e) => {
                        let mut x = *e;
                        while let ExprKind::Parenthesized(y) = &a.expr(x).kind {
                            x = *y;
                        }
                        let ExprKind::Assign { op, target, value } = &a.expr(x).kind else { continue };
                        if !matches!(op, AssignOp::Assign | AssignOp::Compound(BinaryOp::Add)) {
                            continue;
                        }
                        if !matches!(a.expr(*target).kind, ExprKind::Identifier(_)) {
                            continue;
                        }
                        let Some(&escrita) = s.corpo.tipos_de_escrita.get(&x) else { continue };
                        if !e_string(escrita) {
                            continue;
                        }
                        if matches!(op, AssignOp::Compound(_)) {
                            // O elemento escrito é uma local do corpo?
                            let local = match s.corpo.get_resolved(*target) {
                                Some(Resolved::Local(_)) => s.corpo.declaracao_local(*target).is_some_and(|d| locais.contains(&d)),
                                _ => false,
                            };
                            if !local {
                                achados.push(a.expr(x).span);
                            }
                        } else {
                            // `_IdentifierIsPrefixVisitor`: o elemento do lado
                            // esquerdo é nulo.
                            let mut y = *value;
                            loop {
                                match &a.expr(y).kind {
                                    ExprKind::Binary { op: BinaryOp::Add, left, .. } => y = *left,
                                    ExprKind::Parenthesized(z) => y = *z,
                                    ExprKind::String(l) if l.parts.len() >= 2 => match (&l.parts[0], &l.parts[1]) {
                                        (StringPart::Text(t), StringPart::Interpolation(z)) if t.code_units().next().is_none() => y = *z,
                                        _ => break,
                                    },
                                    ExprKind::Identifier(_) => {
                                        if s.corpo.get_resolved(y).is_none() {
                                            achados.push(a.expr(*target).span);
                                        }
                                        break;
                                    }
                                    _ => break,
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        achados.sort_by_key(|x| x.start);
        for sp in achados {
            relatar(&c::USE_STRING_BUFFERS, sp, &[]);
        }
    }

    // `use_enums` (com `enhanced-enums`, 2.17).
    if ligada("use_enums") && program.library(program.unit(s.unidade).library).features.versao() >= dartforge_frontend::features::LanguageVersion::new(2, 17) {
        for (k, d) in a.decls.iter().enumerate() {
            let DeclKind::Class(x) = &d.kind else { continue };
            if d.augment || x.modifiers.abstract_ || x.mixin_application {
                continue;
            }
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            if program.class(classe).supertype_class != s.core.object_class {
                continue;
            }
            let publica = !interner.resolve(x.name.sym).starts_with('_');
            let mut candidatas: Vec<ExprId> = Vec::new();
            let mut invalida = false;
            for &mid in &x.members {
                let m = a.member(mid);
                let nome_e = |n: &str| match &m.kind {
                    MemberKind::Method(f) => a.function(*f).name.is_some_and(|q| interner.resolve(q.sym) == n),
                    MemberKind::Field(l) => l.variables.iter().any(|v| interner.resolve(v.name.sym) == n),
                    MemberKind::Constructor(_) => false,
                };
                if nome_e("hashCode") || nome_e("index") || nome_e("==") || nome_e("values") {
                    invalida = true;
                    break;
                }
                match &m.kind {
                    MemberKind::Field(l) if l.static_ => {
                        for v in l.variables.iter() {
                            if !l.const_ {
                                continue;
                            }
                            let Some(i) = v.initializer else { continue };
                            let Some(Resolved::Constructor(f)) = s.corpo.get_resolved(i) else { continue };
                            if !matches!(a.expr(i).kind, ExprKind::InstanceCreation { .. } | ExprKind::Call { .. }) {
                                continue;
                            }
                            let g = program.function(*f);
                            if g.factory || g.class != Some(classe) {
                                continue;
                            }
                            candidatas.push(i);
                        }
                    }
                    MemberKind::Constructor(kc) => {
                        if !kc.factory && !kc.const_ {
                            invalida = true;
                            break;
                        }
                        if publica && kc.name.is_none_or(|n| !interner.resolve(n.sym).starts_with('_')) {
                            invalida = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if invalida || candidatas.len() < 2 {
                continue;
            }
            // Nenhuma outra criação geradora da classe (na unidade, fora as
            // das constantes), nem subclasse que a estenda, implemente ou
            // misture.
            let geradora = |e: ExprId| match s.corpo.get_resolved(e) {
                Some(Resolved::Constructor(f)) => {
                    let g = program.function(*f);
                    !g.factory && g.class == Some(classe)
                }
                _ => false,
            };
            let mut outras = false;
            for (i, _) in a.exprs.iter().enumerate() {
                let e = ExprId(i as u32);
                if geradora(e) && !candidatas.contains(&e) {
                    outras = true;
                    break;
                }
            }
            if outras {
                continue;
            }
            let usada = (0..program.classes.len()).map(|i| ClassId(i as u32)).any(|cl| {
                let kx = program.class(cl);
                cl != classe
                    && kx.decl.is_some_and(|r| r.unit == s.unidade)
                    && (kx.supertype_class == Some(classe) || kx.interface_classes.contains(&classe) || kx.mixin_classes.contains(&classe))
            });
            if usada {
                continue;
            }
            relatar(&c::USE_ENUMS, x.name.span, &[]);
        }
    }

    out
}
