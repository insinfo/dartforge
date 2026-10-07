//! Os mixins (`MixinElement`) da cláusula `with` no
//! `ErrorVerifier._checkForAllMixinErrorCodes`
//! (`analyzer/lib/src/generated/error_verifier.dart:1974-2017`), com tipos:
//!
//! * `_checkForMixinSuperclassConstraints` (`:4393-4425`): cada restrição
//!   `on` do mixin, substituída pelos argumentos dele, tem de ser
//!   supertipo do supertipo da classe ou de um mixin anterior —
//!   `mixin_application_not_implemented_interface` no nome do mixin;
//! * `_checkForMixinSuperInvokedMembers` (`:4428-4489`): cada nome invocado
//!   por `super` no mixin (`MixinSuperInvokedNamesCollector`,
//!   `analyzer/lib/src/dart/ast/mixin_super_invoked_names.dart`) precisa de
//!   uma implementação concreta antes do mixin
//!   (`mixin_application_no_concrete_super_invoked_member`/`_setter`) que
//!   seja sobrescrita correta do membro do mixin
//!   (`mixin_application_concrete_super_invoked_member_type`), no tipo do
//!   mixin inteiro.
//!
//! O resultado vai para a porta de `analise::clausulas`
//! (`definir_decisoes_de_mixins`): um mixin decidido aqui deixa de deixá-la
//! incerta. Mixin genérico escrito sem argumentos e com restrições `on`
//! depende da inferência de mixins do link (não portada) e fica sem decisão.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::heranca::{Heranca, Nome, Param, ProvedorDoOutline};
use crate::ops;
use crate::resolve::OutlineTypes;
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{ClassId, ClassKind, LibraryId, Program};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, DeclKind, ExprKind, MemberKind, UnaryOp};
use dartforge_frontend::pais::Pai;
use dartforge_intern::Interner;
use std::collections::HashMap;

/// O lexema de um operador binário.
fn lexema(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::IfNull => "??",
        BinaryOp::NaoBinario => "~",
    }
}

/// `(leitura, escrita)` de um alvo de atribuição/incremento: o contexto de
/// `inGetterContext`/`inSetterContext`.
fn contexto(a: &ast::Ast, pais: &dartforge_frontend::pais::Pais, e: ast::ExprId) -> (bool, bool) {
    match pais.pai(e) {
        Pai::Expr(p) => match &a.expr(p).kind {
            ExprKind::Assign { op, target, .. } if *target == e => match op {
                AssignOp::Assign => (false, true),
                AssignOp::Compound(_) => (true, true),
            },
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } if *operand == e => (true, true),
            _ => (true, false),
        },
        _ => (true, false),
    }
}

/// `superInvokedNames`: os nomes invocados por `super` nos corpos dos
/// métodos e acessores do mixin, na ordem da visita (pré-ordem: pelo início
/// e, no mesmo início, o nó maior antes).
fn nomes_invocados_por_super(program: &Program, m: ClassId) -> Vec<String> {
    let Some(d) = program.class(m).decl else { return Vec::new() };
    let unidade = program.unit(d.unit);
    let a = &unidade.ast;
    let DeclKind::Mixin(x) = &a.decl(d.decl).kind else { return Vec::new() };
    let corpos: Vec<dartforge_diagnostics::Span> = x
        .members
        .iter()
        .filter_map(|&mid| match &a.member(mid).kind {
            MemberKind::Method(f) => Some(a.function(*f).span),
            _ => None,
        })
        .collect();
    if corpos.is_empty() {
        return Vec::new();
    }
    let pais = crate::lints_tipados::pais_da_unidade(program, d.unit);
    let e_super = |e: ast::ExprId| matches!(a.expr(e).kind, ExprKind::Super);
    let mut achados: Vec<(usize, usize, Vec<String>)> = Vec::new();
    for (i, ex) in a.exprs.iter().enumerate() {
        let id = ast::ExprId(i as u32);
        let sp = ex.span;
        if !corpos.iter().any(|c| c.start <= sp.start && sp.end <= c.end) {
            continue;
        }
        let nomes: Vec<String> = match &ex.kind {
            // `super.m(...)`: a `MethodInvocation`.
            ExprKind::Call { target, .. } => match &a.expr(*target).kind {
                ExprKind::Property { target: t, name, .. } if e_super(*t) => vec![unidade.source[name.span.start..name.span.end].to_string()],
                _ => Vec::new(),
            },
            ExprKind::Property { target, name, .. } if e_super(*target) => {
                // O alvo de uma chamada é o `methodName`, já contado.
                if let Pai::Expr(p) = pais.pai(id)
                    && matches!(&a.expr(p).kind, ExprKind::Call { target: t, .. } if *t == id)
                {
                    Vec::new()
                } else {
                    let nome = unidade.source[name.span.start..name.span.end].to_string();
                    let (le, escreve) = contexto(a, &pais, id);
                    let mut v = Vec::new();
                    if le {
                        v.push(nome.clone());
                    }
                    if escreve {
                        v.push(format!("{nome}="));
                    }
                    v
                }
            }
            ExprKind::Index { target, .. } if e_super(*target) => {
                let (le, escreve) = contexto(a, &pais, id);
                let mut v = Vec::new();
                if le {
                    v.push("[]".to_string());
                }
                if escreve {
                    v.push("[]=".to_string());
                }
                v
            }
            ExprKind::Binary { op, left, .. } if e_super(*left) => vec![lexema(*op).to_string()],
            ExprKind::Unary { op: UnaryOp::Neg, operand } if e_super(*operand) => vec!["unary-".to_string()],
            ExprKind::Unary { op: UnaryOp::BitNot, operand } if e_super(*operand) => vec!["~".to_string()],
            _ => Vec::new(),
        };
        if !nomes.is_empty() {
            achados.push((sp.start, usize::MAX - (sp.end - sp.start), nomes));
        }
    }
    achados.sort_by_key(|(ini, tam, _)| (*ini, *tam));
    let mut saida: Vec<String> = Vec::new();
    for (_, _, ns) in achados {
        for n in ns {
            if !saida.contains(&n) {
                saida.push(n);
            }
        }
    }
    saida
}

/// A chave do modelo de elementos para um nome do analyzer (o setter `x=` é
/// `x_=`; os operadores ficam como estão).
fn chave_do_modelo(interner: &Interner, nome: &str) -> Option<dartforge_intern::SymbolId> {
    if let Some(base) = nome.strip_suffix('=')
        && !base.is_empty()
        && base.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
    {
        return interner.lookup(&format!("{base}_="));
    }
    interner.lookup(nome)
}

/// `_computeThisTypeForSubtype`: os parâmetros covariantes valem `Object?`.
fn tipo_para_subtipo(table: &mut TypeTable, core: &CoreTypes, t: TypeId, covariantes: &[Param]) -> TypeId {
    if covariantes.is_empty() {
        return t;
    }
    let Type::Function { type_params, ret, positional, optional, named, nullable } = table.get(t).clone() else { return t };
    let mut pos = positional.to_vec();
    let mut opc = optional.to_vec();
    let mut nom = named.to_vec();
    for p in covariantes {
        match *p {
            Param::Indice(i) => {
                if i < pos.len() {
                    pos[i] = core.object_nullable;
                } else if i - pos.len() < opc.len() {
                    let k = i - pos.len();
                    opc[k] = core.object_nullable;
                }
            }
            Param::Nome(n) => {
                if let Some(x) = nom.iter_mut().find(|(m, _, _)| *m == n) {
                    x.1 = core.object_nullable;
                }
            }
        }
    }
    table.intern(Type::Function { type_params, ret, positional: pos.into_boxed_slice(), optional: opc.into_boxed_slice(), named: nom.into_boxed_slice(), nullable })
}

/// As decisões dos mixins de `with` das declarações de `libs`:
/// `(classe, posição no with)` → `None` (nada relatado) ou o relato.
pub fn decisoes(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    libs: &[LibraryId],
) -> HashMap<(ClassId, usize), Option<Diagnostic>> {
    let mut saida = HashMap::new();
    let mut heranca = Heranca::default();
    let mut nomes_por_mixin: HashMap<ClassId, Vec<String>> = HashMap::new();
    let enum_ = program
        .classes
        .iter()
        .position(|ce| ce.decl.is_some() && program.library(ce.library).uri == "dart:core" && interner.resolve(ce.name) == "Enum")
        .map(|i| ClassId(i as u32));
    for (ci, ce) in program.classes.iter().enumerate() {
        if !libs.contains(&ce.library) {
            continue;
        }
        let id = ClassId(ci as u32);
        let Some(d) = ce.decl else { continue };
        let a = &program.unit(d.unit).ast;
        let with: &[ast::TypeId] = match &a.decl(d.decl).kind {
            DeclKind::Class(x) => &x.with,
            DeclKind::Enum(x) => &x.with,
            _ => continue,
        };
        if with.is_empty() {
            continue;
        }
        let dados = outline.classes[ci].clone();
        if dados.mixins.len() != with.len() {
            continue;
        }
        // `_enclosingClass.supertype`, sem `?`.
        let supertipo = match ce.kind {
            ClassKind::Enum => match enum_ {
                Some(e) => table.intern(Type::Interface { class: e, args: Box::new([]), nullable: false }),
                None => continue,
            },
            _ => match dados.supertype {
                Some(t) if matches!(table.get(t), Type::Interface { .. }) => {
                    let Type::Interface { class, args, .. } = table.get(t).clone() else { continue };
                    table.intern(Type::Interface { class, args, nullable: false })
                }
                _ => core.object,
            },
        };
        // `mixinTypeIndex`: só os mixins que são tipos de interface.
        let mut indice_de_tipo: isize = -1;
        for (i, (&escrito, &mt)) in with.iter().zip(dados.mixins.iter()).enumerate() {
            let Type::Interface { class: m, args, .. } = table.get(mt).clone() else { continue };
            indice_de_tipo += 1;
            let me = program.class(m);
            if me.kind != ClassKind::Mixin {
                continue;
            }
            // O mixin escrito sem argumentos já vem inferido do outline
            // (`resolve::inferir_mixin`), ou cru quando a inferência falha.
            let params: Vec<TypeParamId> = outline.classes[m.0 as usize].type_params.to_vec();
            let mapa: HashMap<TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            // `_checkForMixinSuperclassConstraints`.
            let mut decisao: Option<Diagnostic> = None;
            let restricoes = outline.classes[m.0 as usize].on.clone();
            for &r in restricoes.iter() {
                let restricao = if mapa.is_empty() { r } else { ops::substitute(r, &mapa, table) };
                let mut satisfeita = {
                    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                    is_subtype(supertipo, restricao, &mut env)
                };
                for &anterior in dados.mixins[..i].iter() {
                    if satisfeita {
                        break;
                    }
                    let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                    satisfeita = is_subtype(anterior, restricao, &mut env);
                }
                if !satisfeita {
                    let nome = match &a.ty(escrito).kind {
                        ast::TypeKind::Named { name, .. } => name.last().map(|n| n.span),
                        _ => None,
                    };
                    let Some(sp) = nome else { break };
                    let (a0, a1, a2) = (table.format(mt, interner, program), table.format(supertipo, interner, program), table.format(restricao, interner, program));
                    decisao = Some(Diagnostic::com_codigo(c::MIXIN_APPLICATION_NOT_IMPLEMENTED_INTERFACE, sp, [a0.as_str(), a1.as_str(), a2.as_str()]));
                    break;
                }
            }
            // `_checkForMixinSuperInvokedMembers`.
            if decisao.is_none() {
                let nomes = nomes_por_mixin.entry(m).or_insert_with(|| nomes_invocados_por_super(program, m)).clone();
                if !nomes.is_empty() {
                    let interface = {
                        let mut p = ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
                        heranca.interface(&mut p, id)
                    };
                    let interface_do_mixin = {
                        let mut p = ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
                        heranca.interface(&mut p, m)
                    };
                    let sp = a.ty(escrito).span;
                    for nome in nomes {
                        // `isSetter = name.endsWith('=')` (também `==`, `<=`,
                        // `[]=`: o nome perde o último `=`).
                        let (codigo_ausente, texto_ausente) = match nome.strip_suffix('=') {
                            Some(s) => (c::MIXIN_APPLICATION_NO_CONCRETE_SUPER_INVOKED_SETTER, s.to_string()),
                            None => (c::MIXIN_APPLICATION_NO_CONCRETE_SUPER_INVOKED_MEMBER, nome.clone()),
                        };
                        let Some(chave) = chave_do_modelo(interner, &nome) else {
                            decisao = Some(Diagnostic::com_codigo(codigo_ausente, sp, [texto_ausente.as_str()]));
                            break;
                        };
                        let n = Nome::novo(interner, me.library, chave);
                        let super_membro = usize::try_from(indice_de_tipo).ok().and_then(|k| interface.super_implemented.get(k)).and_then(|mp| mp.get(&n)).cloned();
                        let Some(super_membro) = super_membro else {
                            decisao = Some(Diagnostic::com_codigo(codigo_ausente, sp, [texto_ausente.as_str()]));
                            break;
                        };
                        // `getMember(mixinType, name, forSuper: true)`: o
                        // `super` do próprio mixin (as restrições), visto
                        // pelos argumentos do mixin.
                        let Some(do_mixin) = interface_do_mixin.super_implemented.last().and_then(|mp| mp.get(&n)).cloned() else { continue };
                        let tipo_mixin = if mapa.is_empty() { do_mixin.tipo } else { ops::substitute(do_mixin.tipo, &mapa, table) };
                        let este = tipo_para_subtipo(table, core, super_membro.tipo, &super_membro.covariantes);
                        let correto = {
                            let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
                            is_subtype(este, tipo_mixin, &mut env)
                        };
                        if !correto {
                            let (t1, t2) = (table.format(tipo_mixin, interner, program), table.format(super_membro.tipo, interner, program));
                            decisao = Some(Diagnostic::com_codigo(c::MIXIN_APPLICATION_CONCRETE_SUPER_INVOKED_MEMBER_TYPE, sp, [nome.as_str(), t1.as_str(), t2.as_str()]));
                            break;
                        }
                    }
                }
            }
            saida.insert((id, i), decisao);
        }
    }
    saida
}
