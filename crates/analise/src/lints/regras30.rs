//! O trigésimo lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `use_late_for_private_fields_and_variables`: na biblioteca inteira, as
//!   variáveis de topo privadas e os campos privados (ou de extensão
//!   privada), não `late`, de tipo anulável, que nenhum acesso anulável
//!   alcança. Fora das classes com construtor `const` e dos `enum` (o
//!   visitante não desce neles), todo identificador, `PrefixedIdentifier` e
//!   `PropertyAccess` com elemento conta como acesso anulável, menos o
//!   identificador simples logo antes de um `!` (o nome de propriedade
//!   dentro de `a.b` tem o `a.b` como pai e conta sempre); a atribuição
//!   conta pelo elemento de escrita, menos o `=` com lado direito não
//!   anulável; o lado esquerdo e o operando de `++`/`--` não têm elemento.
//!
//!   Divergência registrada: as referências de comentário de documentação
//!   (`[nome]`) não entram.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::regras16::{nao_anulavel, tipo_de_variavel};
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{AssignOp, Ast, DeclId, DeclKind, ExprId, ExprKind, MemberId, MemberKind, UnaryOp};
use dartforge_intern::Interner;
use dartforge_types::resolved::{MemberRef, Resolved, UnitBodyTypes};
use std::collections::HashSet;

/// A variável do modelo que o elemento resolvido canoniza (o acessor vira
/// a variável).
fn variavel(s: &super::Semantica<'_>, corpo: &UnitBodyTypes, e: ExprId) -> Option<VariableId> {
    match corpo.get_resolved(e)? {
        Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => Some(*v),
        Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => s.program.function(*f).variable,
        Resolved::ExtensionMember { member, .. } => s.program.function(*member).variable,
        _ => None,
    }
}

/// As regiões que o visitante não visita: as classes com construtor
/// `const` e os `enum`.
fn excluidas(a: &Ast) -> Vec<Span> {
    let mut v = Vec::new();
    for d in a.decls.iter() {
        match &d.kind {
            DeclKind::Class(x) if x.members.iter().any(|m| matches!(&a.member(*m).kind, MemberKind::Constructor(k) if k.const_)) => v.push(d.span),
            DeclKind::Enum(_) => v.push(d.span),
            _ => {}
        }
    }
    v
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    if !ligada("use_late_for_private_fields_and_variables") {
        return out;
    }
    let program = s.program;
    let table = s.table;
    let privado = |n: dartforge_intern::SymbolId| interner.resolve(n).starts_with('_');
    let biblioteca = program.unit(s.unidade).library;
    let unidades: Vec<UnitId> = program.library(biblioteca).units.clone();
    // Os acessos anuláveis da biblioteca.
    let mut anulaveis: HashSet<VariableId> = HashSet::new();
    for &x in unidades.iter() {
        let a = &program.unit(x).ast;
        let corpo = &s.corpos.units[x.0 as usize];
        let fora = excluidas(a);
        let visitado = |sp: Span| !fora.iter().any(|f| f.start <= sp.start && sp.end <= f.end);
        let mut escritos: HashSet<ExprId> = HashSet::new();
        // O operando imediato de um `!` (sem parênteses).
        let mut checados: HashSet<ExprId> = HashSet::new();
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::Assign { target, .. } => {
                    escritos.insert(*target);
                }
                ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                    escritos.insert(*operand);
                }
                ExprKind::Unary { op: UnaryOp::NullAssert, operand } => {
                    checados.insert(*operand);
                }
                _ => {}
            }
        }
        for (k, e) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            if !visitado(e.span) {
                continue;
            }
            match &e.kind {
                ExprKind::Identifier(_) if !escritos.contains(&id) => {
                    if let Some(v) = variavel(s, corpo, id)
                        && !checados.contains(&id)
                    {
                        anulaveis.insert(v);
                    }
                }
                ExprKind::Property { .. } if !escritos.contains(&id) => {
                    // O nome dentro de `a.b` tem o próprio `a.b` como pai.
                    if let Some(v) = variavel(s, corpo, id) {
                        anulaveis.insert(v);
                    }
                }
                ExprKind::Assign { op, target, value } => {
                    if let Some(v) = variavel(s, corpo, *target) {
                        let nao_nulo = *op == AssignOp::Assign && s.corpos.units[x.0 as usize].get_type(*value).is_some_and(|t| nao_anulavel(table, t));
                        if !nao_nulo {
                            anulaveis.insert(v);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // As candidatas da unidade.
    let a = u.ast;
    let fora = excluidas(a);
    let visitado = |sp: Span| !fora.iter().any(|f| f.start <= sp.start && sp.end <= f.end);
    let mut achados: Vec<Span> = Vec::new();
    let candidata = |r: VariableRef, l: &dartforge_frontend::ast::VariableList, i: usize, achados: &mut Vec<Span>| {
        if l.late {
            return;
        }
        let Some(v) = program.variables.iter().position(|x| x.node == r).map(|i| VariableId(i as u32)) else { return };
        let Some(t) = tipo_de_variavel(s, r) else { return };
        if nao_anulavel(table, t) {
            return;
        }
        if !anulaveis.contains(&v) {
            let x = &l.variables[i];
            let fim = x.initializer.map_or(x.name.span.end, |e| a.expr(e).span.end);
            achados.push(Span { start: x.name.span.start, end: fim });
        }
    };
    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        if !visitado(d.span) {
            continue;
        }
        match &d.kind {
            DeclKind::Variables(l) => {
                for (i, x) in l.variables.iter().enumerate() {
                    if privado(x.name.sym) {
                        candidata(VariableRef::TopLevel { unit: s.unidade, decl: id, index: i }, l, i, &mut achados);
                    }
                }
            }
            _ => {
                let (membros, extensao_privada, tipo_de_extensao): (&[MemberId], bool, bool) = match &d.kind {
                    DeclKind::Class(x) => (&x.members, false, false),
                    DeclKind::Mixin(x) => (&x.members, false, false),
                    DeclKind::Extension(x) => (&x.members, x.name.is_none_or(|n| privado(n.sym)), false),
                    DeclKind::ExtensionType(x) => (&x.members, false, true),
                    _ => continue,
                };
                for &m in membros {
                    let MemberKind::Field(l) = &a.member(m).kind else { continue };
                    if tipo_de_extensao && !l.static_ {
                        continue;
                    }
                    for (i, x) in l.variables.iter().enumerate() {
                        if extensao_privada || privado(x.name.sym) {
                            candidata(VariableRef::Field { unit: s.unidade, member: m, index: i }, l, i, &mut achados);
                        }
                    }
                }
            }
        }
    }
    for sp in achados {
        out.push(RelatoDeLint { codigo: &c::USE_LATE_FOR_PRIVATE_FIELDS_AND_VARIABLES, span: sp, args: Vec::new() });
    }
    out
}
