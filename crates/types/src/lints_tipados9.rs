//! `avoid_redundant_argument_values` (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8),
//! escrita direto do emissor da 3.6.2, com o motor de constantes.
//!
//! As listas de argumentos de `MethodInvocation`, de
//! `FunctionExpressionInvocation` cujo alvo é o tear-off de uma função
//! declarada (os parâmetros do tipo dela são os do elemento; o tipo de
//! função escrito tem parâmetros sintéticos, sem valor), de criação de
//! instância (a factory redirecionadora segue a cadeia até o último
//! construtor e casa os argumentos pelos parâmetros dele) e de constante de
//! enum. Do último argumento para o primeiro: o parâmetro opcional, não
//! `required` nem `@required`, com valor padrão conhecido (o escrito; sem
//! ele, `null`, ou o do parâmetro do construtor da superclasse num
//! `super.x` cujo tipo cabe no do parâmetro) igual ao valor constante do
//! argumento (pelo `ConstantVisitor`, no contexto constante do nó); o
//! primeiro posicional opcional encerra a varredura.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::constantes::avaliador::{Constante, Ctx, Motor};
use crate::constantes::valor::Valor;
use crate::lints_tipados::Achado;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, TypeTable};
use dartforge_elements::model::{Element, FunctionElementId, FunctionRef, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind, Initializer, MemberKind, ParameterKind, StmtKind};
use dartforge_intern::Interner;
use std::collections::HashSet;

/// Os parâmetros escritos de uma função do modelo, com a unidade.
fn parametros_de(program: &Program, f: FunctionElementId) -> Option<(UnitId, &[ast::Parameter])> {
    match program.function(f).node {
        FunctionRef::Function { unit, function } => Some((unit, program.unit(unit).ast.function(function).parameters.as_deref().unwrap_or(&[]))),
        FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
            MemberKind::Constructor(k) => Some((unit, &k.parameters[..])),
            _ => None,
        },
        FunctionRef::None => None,
    }
}

/// O construtor para o qual a factory `f` redireciona (`= C.nome`).
fn redirecionado(program: &Program, interner: &Interner, f: FunctionElementId) -> Option<FunctionElementId> {
    let FunctionRef::Constructor { unit, member } = program.function(f).node else { return None };
    let a = &program.unit(unit).ast;
    let MemberKind::Constructor(k) = &a.member(member).kind else { return None };
    if !k.factory {
        return None;
    }
    let r = k.redirect.as_ref()?;
    let (alvo, construtor) = crate::redirecionamento::classe_e_construtor(program, unit, r)?;
    let chave = match construtor {
        Some(n) => n.sym,
        None => interner.lookup("")?,
    };
    program.class(alvo).constructors.get(&chave).copied()
}

/// Um parâmetro escrito com a função dona (para o `super.x`).
#[derive(Clone, Copy)]
struct Param<'p> {
    dono: Option<FunctionElementId>,
    unidade: UnitId,
    p: &'p ast::Parameter,
    /// O índice na lista escrita.
    indice: usize,
}

struct Cx<'m, 'p> {
    program: &'p Program,
    interner: &'p Interner,
    core: &'p CoreTypes,
    outline: &'p OutlineTypes,
    motor: Motor<'m>,
}

impl<'p> Cx<'_, 'p> {
    fn lib(&self, u: UnitId) -> LibraryId {
        self.program.unit(u).library
    }

    /// `computeConstantValue` do parâmetro opcional.
    fn padrao(&mut self, q: Param<'p>, prof: u32) -> Option<Valor> {
        if prof > 32 {
            return None;
        }
        if let Some(d) = q.p.default_value {
            let lib = self.lib(q.unidade);
            return match self.motor.resultado_padrao(q.unidade, lib, d) {
                Constante::Valor(v) => Some(v),
                Constante::Invalida(_) => None,
            };
        }
        if !q.p.super_ {
            return Some(Valor::nulo(self.core));
        }
        // `_superConstructorParameterDefaultValue`.
        let f = q.dono?;
        let sup = self.parametro_do_super(f, q)?;
        let v = self.padrao(sup, prof + 1)?;
        let proprio = self.outline.functions.get(f.0 as usize)?.parameters.get(q.indice)?.ty;
        let mut env = SubtypeEnv::new(self.motor.table, &self.outline.hierarchy, self.core);
        is_subtype(v.tipo, proprio, &mut env).then_some(v)
    }

    /// `superConstructorParameter`: o parâmetro de mesmo nome (nomeado) ou
    /// de mesma posição entre os `super.x` posicionais.
    fn parametro_do_super(&self, f: FunctionElementId, q: Param<'p>) -> Option<Param<'p>> {
        let program = self.program;
        let FunctionRef::Constructor { unit, member } = program.function(f).node else { return None };
        let MemberKind::Constructor(k) = &program.unit(unit).ast.member(member).kind else { return None };
        let classe = program.function(f).class?;
        let sup = program.class(classe).supertype_class?;
        let nome = k.initializers.iter().find_map(|i| match i {
            Initializer::Super { constructor, .. } => Some(*constructor),
            _ => None,
        });
        let chave = match nome.flatten() {
            Some(n) => n.sym,
            None => self.interner.lookup("")?,
        };
        let g = *program.class(sup).constructors.get(&chave)?;
        let (gu, gps) = parametros_de(program, g)?;
        let alvo = if q.p.kind == ParameterKind::Named {
            let n = q.p.nome_externo()?.sym;
            gps.iter().position(|x| x.kind == ParameterKind::Named && x.nome_externo().is_some_and(|m| m.sym == n))?
        } else {
            let i = k.parameters.iter().filter(|x| x.super_ && x.kind != ParameterKind::Named).position(|x| std::ptr::eq(x, q.p))?;
            let posicionais: Vec<usize> = gps.iter().enumerate().filter(|(_, x)| x.kind != ParameterKind::Named).map(|(j, _)| j).collect();
            *posicionais.get(i)?
        };
        Some(Param { dono: Some(g), unidade: gu, p: &gps[alvo], indice: alvo })
    }

    /// `checkArgument`.
    fn checar(&mut self, u: UnitId, pais: &dartforge_frontend::pais::Pais, valor: ExprId, param: Option<Param<'p>>, out: &mut Vec<Achado>) {
        let Some(q) = param else { return };
        if q.p.kind != ParameterKind::Optional && q.p.kind != ParameterKind::Named {
            return;
        }
        if q.p.required || q.p.metadata.iter().any(|m| crate::fase_resultado::anotacao_do_meta(self.program, self.interner, q.unidade, m, "required")) {
            return;
        }
        let Some(padrao) = self.padrao(q, 0) else { return };
        if padrao.estado.desconhecido() {
            return;
        }
        let a = &self.program.unit(u).ast;
        let cx = Ctx::simples(u, self.lib(u));
        let em_const = pais.em_contexto_constante(a, valor);
        let Constante::Valor(v) = self.motor.avaliar(&cx, valor, em_const) else { return };
        if v.estado.desconhecido() {
            return;
        }
        if self.motor.iguais(&v, &padrao) {
            out.push((a.expr(valor).span, "avoid_redundant_argument_values", Vec::new()));
        }
    }

    /// `check`: o `staticParameterElement` de cada argumento.
    fn lista(&mut self, u: UnitId, pais: &dartforge_frontend::pais::Pais, args: &ast::Arguments, alvo: Option<(Option<FunctionElementId>, UnitId, &'p [ast::Parameter])>, out: &mut Vec<Achado>) {
        for i in (0..args.args.len()).rev() {
            let x = args.args[i];
            let param = alvo.and_then(|(dono, pu, ps)| {
                let indice = match x.name {
                    Some(n) => ps.iter().position(|p| p.kind == ParameterKind::Named && p.nome_externo().is_some_and(|m| m.sym == n.sym)),
                    None => {
                        let k = args.args[..i].iter().filter(|y| y.name.is_none()).count();
                        ps.iter().enumerate().filter(|(_, p)| p.kind != ParameterKind::Named).nth(k).map(|(j, _)| j)
                    }
                };
                indice.map(|j| Param { dono, unidade: pu, p: &ps[j], indice: j })
            });
            self.checar(u, pais, x.value, param, out);
            if param.is_some_and(|q| q.p.kind == ParameterKind::Optional) {
                break;
            }
        }
    }

    /// `visitInstanceCreationExpression`.
    fn criacao(&mut self, u: UnitId, pais: &dartforge_frontend::pais::Pais, args: &ast::Arguments, construtor: Option<FunctionElementId>, out: &mut Vec<Achado>) {
        let Some(k) = construtor else {
            self.lista(u, pais, args, None, out);
            return;
        };
        let alvo_de = |f: FunctionElementId| parametros_de(self.program, f).map(|(pu, ps)| (Some(f), pu, ps));
        let fabrica = matches!(self.program.function(k).node, FunctionRef::Constructor { unit, member } if matches!(&self.program.unit(unit).ast.member(member).kind, MemberKind::Constructor(c) if c.factory));
        let mut redirecionado_para = if fabrica { redirecionado(self.program, self.interner, k) } else { None };
        if !fabrica || redirecionado_para.is_none() {
            let alvo = alvo_de(k);
            self.lista(u, pais, args, alvo, out);
            return;
        }
        let mut vistos: HashSet<FunctionElementId> = HashSet::from([k]);
        let mut atual = k;
        while let Some(r) = redirecionado_para {
            if !vistos.insert(r) {
                // Ciclo: erro de compilação.
                return;
            }
            atual = r;
            redirecionado_para = redirecionado(self.program, self.interner, r);
        }
        let Some((dono, pu, ps)) = alvo_de(atual) else { return };
        if args.args.is_empty() {
            return;
        }
        for i in (0..args.args.len()).rev() {
            let x = args.args[i];
            let indice = match x.name {
                Some(n) => ps.iter().position(|p| p.kind == ParameterKind::Named && p.name.is_some_and(|m| m.sym == n.sym)),
                None => {
                    let contagem = args.args[..=i].iter().filter(|y| y.name.is_none()).count();
                    let j = contagem - 1;
                    (j < ps.len() && ps[j].kind != ParameterKind::Named).then_some(j)
                }
            };
            let param = indice.map(|j| Param { dono, unidade: pu, p: &ps[j], indice: j });
            self.checar(u, pais, x.value, param, out);
            if param.is_some_and(|q| q.p.kind == ParameterKind::Optional) {
                break;
            }
        }
    }
}

/// Os achados na unidade `u`.
#[allow(clippy::too_many_arguments)]
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    corpos: &BodyTypes,
    inferidas: &HashSet<LibraryId>,
    u: UnitId,
) -> Vec<Achado> {
    let mut out = Vec::new();
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let corpo = &corpos.units[u.0 as usize];
    let antes_de_3 = program.library(unidade.library).features.versao().major < 3;
    let pais = dartforge_frontend::pais::Pais::novo(a, &unidade.unit, unidade.source.as_str(), antes_de_3);
    let motor = Motor::novo(program, interner, table, core, outline, corpos, inferidas);
    let mut cx = Cx { program, interner, core, outline, motor };
    // A função local declarada em `d` (o offset do nome).
    let local = |d: usize| -> Option<&[ast::Parameter]> {
        a.stmts.iter().find_map(|s| match &s.kind {
            StmtKind::Function(g) if a.function(*g).name.is_some_and(|n| n.span.start == d) => Some(a.function(*g).parameters.as_deref().unwrap_or(&[])),
            _ => None,
        })
    };
    // Os parâmetros da função invocada por nome (`MethodInvocation`) ou pelo
    // tear-off (`FunctionExpressionInvocation`).
    let da_funcao = |e: ExprId| -> Option<(Option<FunctionElementId>, UnitId, &[ast::Parameter])> {
        let f = match corpo.get_resolved(e)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => *f,
            Resolved::ExtensionMember { member, .. } => *member,
            Resolved::Local(_) => return local(corpo.declaracao_local(e)?).map(|ps| (None, u, ps)),
            _ => return None,
        };
        if program.function(f).kind == dartforge_elements::model::FunctionKind::Getter {
            return None;
        }
        parametros_de(program, f).map(|(pu, ps)| (Some(f), pu, ps))
    };
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        match &e.kind {
            ExprKind::InstanceCreation { arguments, .. } => {
                let construtor = match corpo.get_resolved(id) {
                    Some(Resolved::Constructor(f)) => Some(*f),
                    _ => None,
                };
                cx.criacao(u, &pais, arguments, construtor, &mut out);
            }
            ExprKind::Call { target, arguments } => {
                if let Some(Resolved::Constructor(f)) = corpo.get_resolved(id) {
                    cx.criacao(u, &pais, arguments, Some(*f), &mut out);
                    continue;
                }
                if arguments.args.is_empty() {
                    continue;
                }
                if crate::lints_tipados4::e_invocacao_de_metodo(program, a, corpo, *target) {
                    let alvo = da_funcao(*target);
                    cx.lista(u, &pais, arguments, alvo, &mut out);
                    continue;
                }
                // `FunctionExpressionInvocation`: o tear-off de uma função
                // declarada.
                let mut f = *target;
                while let ExprKind::Parenthesized(x) = &a.expr(f).kind {
                    f = *x;
                }
                let alvo = if matches!(a.expr(f).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) { da_funcao(f) } else { None };
                cx.lista(u, &pais, arguments, alvo, &mut out);
            }
            _ => {}
        }
    }
    // `visitEnumConstantArguments`.
    for (di, d) in a.decls.iter().enumerate() {
        let DeclKind::Enum(x) = &d.kind else { continue };
        let Some(classe) = (0..program.classes.len())
            .map(|i| dartforge_elements::model::ClassId(i as u32))
            .find(|c| program.class(*c).decl.is_some_and(|r| r.unit == u && r.decl == ast::DeclId(di as u32)))
        else {
            continue;
        };
        for k in x.constants.iter() {
            let Some(args) = &k.arguments else { continue };
            let chave = match k.constructor {
                Some(n) => Some(n.sym),
                None => interner.lookup(""),
            };
            let alvo = chave
                .and_then(|c| program.class(classe).constructors.get(&c).copied())
                .and_then(|f| parametros_de(program, f).map(|(pu, ps)| (Some(f), pu, ps)));
            cx.lista(u, &pais, args, alvo, &mut out);
        }
    }
    out
}
