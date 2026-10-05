//! Os conflitos da interface no `InheritanceOverrideVerifier.verify()`
//! (`analyzer/lib/src/error/inheritance_override.dart:186-217`, `:847-875`)
//! e o `overrideNoCombinedSuperSignature` da inferência de sobrescrita
//! (`analyzer/lib/src/task/strong_mode.dart:403-457`), relatado em
//! `_reportNoCombinedSuperSignature` (`inheritance_override.dart:953-971`):
//!
//! * `inconsistent_inheritance_getter_and_method` e `inconsistent_inheritance`,
//!   um por conflito, no nome da declaração;
//! * `no_combined_super_signature`, no nome do método de instância com tipo
//!   omitido cujos sobrescritos (todos métodos) não se combinam.
//!
//! Só nas classes em que `verify()` passa das verificações que o encerram
//! cedo (`clausulas::verificador_de_heranca_prossegue`).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::heranca::{combinar, Conflito, Especie, Heranca, Membro, Nome, ProvedorDoOutline};
use crate::resolve::OutlineTypes;
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{ClassId, FunctionElementId, FunctionRef, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, MemberKind};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

/// O `Name.name` do analyzer: o setter é `x=` (a chave do modelo é `x_=`).
fn nome_do_membro(interner: &Interner, n: &Nome) -> String {
    let texto = interner.resolve(n.chave);
    texto.strip_suffix("_=").map_or_else(|| texto.to_string(), |s| format!("{s}="))
}

/// `'Classe.nome (tipo)'` de cada candidato, com `, `.
fn candidatos_exibidos(program: &Program, interner: &Interner, table: &TypeTable, nome: &str, candidatos: &[Membro]) -> String {
    candidatos
        .iter()
        .map(|m| {
            let classe = interner.resolve(program.class(m.classe).name);
            let tipo = table.format_sem_alias(m.tipo, interner, program);
            format!("{classe}.{nome} ({tipo})")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// O nome e os membros da declaração de uma classe, mixin, enum ou alias.
fn declaracao(program: &Program, c: ClassId) -> Option<(UnitId, ast::Name, &[ast::MemberId])> {
    let d = program.class(c).decl?;
    let a = &program.unit(d.unit).ast;
    let decl = a.decl(d.decl);
    if decl.augment {
        return None;
    }
    match &decl.kind {
        DeclKind::Class(x) => Some((d.unit, x.name, &x.members)),
        DeclKind::Mixin(x) => Some((d.unit, x.name, &x.members)),
        DeclKind::Enum(x) => Some((d.unit, x.name, &x.members)),
        _ => None,
    }
}

/// Os métodos (e operadores) de instância com tipo omitido cujos
/// sobrescritos não se combinam: `(classe, método, nome, explicação)`.
pub fn metodos_sem_assinatura_combinada(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(ClassId, FunctionElementId, ast::Name, String)> {
    let mut por_funcao: HashMap<(UnitId, u32), FunctionElementId> = HashMap::new();
    for (i, f) in program.functions.iter().enumerate() {
        if f.class.is_some()
            && !f.static_
            && let FunctionRef::Function { unit, function } = f.node
        {
            por_funcao.insert((unit, function.0), FunctionElementId(i as u32));
        }
    }
    let mut heranca = Heranca::default();
    let mut saida = Vec::new();
    for &cid in classes {
        let Some((u, _, membros)) = declaracao(program, cid) else { continue };
        let a = &program.unit(u).ast;
        let lib = program.class(cid).library;
        for &m in membros {
            let MemberKind::Method(fid) = &a.member(m).kind else { continue };
            let af = a.function(*fid);
            if af.static_ || !matches!(af.kind, ast::FunctionKind::Function | ast::FunctionKind::Operator) {
                continue;
            }
            let (Some(&f), Some(n)) = (por_funcao.get(&(u, fid.0)), af.name) else { continue };
            let implicito = af.return_type.is_none()
                || af.parameters.as_deref().is_some_and(|ps| ps.iter().any(|p| p.ty.is_none() && p.function_parameters.is_none()));
            if !implicito {
                continue;
            }
            let nome = Nome::novo(interner, lib, program.function(f).name);
            let mut p = ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
            let Some(sobrescritos) = heranca.sobrescritos(&mut p, cid, nome) else { continue };
            if sobrescritos.is_empty() || sobrescritos.iter().any(|s| s.especie != Especie::Metodo) {
                continue;
            }
            let mut conflitos: Vec<Conflito> = Vec::new();
            if combinar(&mut p, cid, &sobrescritos, true, nome, Some(&mut conflitos)).is_some() {
                continue;
            }
            let texto_nome = nome_do_membro(interner, &nome);
            let explicacao = match conflitos.as_slice() {
                [Conflito::Candidatos { candidatos, .. }] => candidatos_exibidos(program, interner, table, &texto_nome, candidatos),
                _ => "<unknown>".to_string(),
            };
            saida.push((cid, f, n, explicacao));
        }
    }
    saida
}

pub fn inconsistencias(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    classes: &[ClassId],
) -> Vec<(UnitId, Diagnostic)> {
    let mut heranca = Heranca::default();
    let mut saida = Vec::new();
    for &cid in classes {
        let Some((u, nome_classe, _)) = declaracao(program, cid) else { continue };
        let interface = {
            let mut p = ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
            heranca.interface(&mut p, cid)
        };
        // O mesmo conflito pode vir das restrições `on` e das interfaces de
        // um mixin: um relato por texto.
        let mut vistos: HashSet<(bool, String, String)> = HashSet::new();
        for conflito in interface.conflitos.iter() {
            match conflito {
                Conflito::GetterMetodo { nome, getter, metodo } => {
                    let n = nome_do_membro(interner, nome);
                    let g = interner.resolve(program.class(getter.classe).name).to_string();
                    let m = interner.resolve(program.class(metodo.classe).name).to_string();
                    if vistos.insert((true, n.clone(), format!("{g}\u{0}{m}"))) {
                        saida.push((u, Diagnostic::com_codigo(c::INCONSISTENT_INHERITANCE_GETTER_AND_METHOD, nome_classe.span, [n.as_str(), g.as_str(), m.as_str()])));
                    }
                }
                Conflito::Candidatos { nome, candidatos } => {
                    let n = nome_do_membro(interner, nome);
                    let lista = candidatos_exibidos(program, interner, table, &n, candidatos);
                    if vistos.insert((false, n.clone(), lista.clone())) {
                        saida.push((u, Diagnostic::com_codigo(c::INCONSISTENT_INHERITANCE, nome_classe.span, [n.as_str(), lista.as_str()])));
                    }
                }
                Conflito::ExtensaoENaoExtensao { .. } | Conflito::ExtensaoNaoUnica { .. } => {}
            }
        }
    }
    // `_reportNoCombinedSuperSignature`, no laço dos membros (depois dos
    // conflitos de todas as classes: offsets diferentes).
    for (cid, _, n, explicacao) in metodos_sem_assinatura_combinada(program, interner, table, core, outline, classes) {
        let Some((u, _, _)) = declaracao(program, cid) else { continue };
        let classe = interner.resolve(program.class(cid).name).to_string();
        saida.push((u, Diagnostic::com_codigo(c::NO_COMBINED_SUPER_SIGNATURE, n.span, [classe.as_str(), explicacao.as_str()])));
    }
    saida
}
