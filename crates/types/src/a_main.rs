//! A função `main` de topo (`ErrorVerifier._checkForMainFunction2`,
//! `analyzer/lib/src/generated/error_verifier.dart:4143-4187`;
//! docs/ANALYZER-ESPECIFICACAO.md §A, grupo 4): mais de dois posicionais
//! obrigatórios, nomeado `required`, e primeiro posicional cujo tipo não é
//! supertipo de `List<String>`. Os três relatos são independentes.
//!
//! Escrito sem compilar nem executar (2026-10-04).

use crate::resolve::OutlineTypes;
use crate::subtyping::{is_subtype, SubtypeEnv};
use crate::table::{CoreTypes, Type, TypeTable};
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{FunctionKind, FunctionRef, LibraryId, Program, UnitId};
use dartforge_frontend::ast::ParameterKind;
use dartforge_intern::Interner;

/// Os diagnósticos da função `main` de topo da biblioteca `lib`.
pub fn funcao_main(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let mut saida = Vec::new();
    let Some(sym_main) = interner.lookup("main") else { return saida };
    for (i, f) in program.functions.iter().enumerate() {
        // Só a função de topo: um método `main` de classe não conta, nem um
        // getter (que não tem lista de parâmetros).
        if f.library != lib || f.name != sym_main || f.class.is_some() || f.extension.is_some() || f.kind != FunctionKind::Function {
            continue;
        }
        let FunctionRef::Function { unit, function } = f.node else { continue };
        let a = &program.unit(unit).ast;
        let func = a.function(function);
        let (Some(ps), Some(nome)) = (&func.parameters, func.name) else { continue };
        let vazio: Vec<&str> = Vec::new();
        let obrigatorios = ps.iter().filter(|p| p.kind == ParameterKind::Required).count();
        if obrigatorios > 2 {
            saida.push((unit, Diagnostic::com_codigo(c::MAIN_HAS_TOO_MANY_REQUIRED_POSITIONAL_PARAMETERS, nome.span, vazio.iter().copied())));
        }
        if ps.iter().any(|p| p.kind == ParameterKind::Named && p.required) {
            saida.push((unit, Diagnostic::com_codigo(c::MAIN_HAS_REQUIRED_NAMED_PARAMETERS, nome.span, vazio.iter().copied())));
        }
        // O primeiro posicional (obrigatório ou opcional), pelo tipo escrito:
        // sem tipo ele é `dynamic` e a condição não dispara.
        let Some(k) = ps.iter().position(|p| p.kind != ParameterKind::Named) else { continue };
        let Some(escrito) = ps[k].ty else { continue };
        let Some(dados) = outline.functions.get(i) else { continue };
        let Some(tipo) = dados.parameters.get(k).map(|p| p.ty) else { continue };
        let Some(lista) = core.list_class else { continue };
        let lista_de_string = table.intern(Type::Interface { class: lista, args: vec![core.string].into_boxed_slice(), nullable: false });
        let cabe = {
            let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
            is_subtype(lista_de_string, tipo, &mut env)
        };
        if !cabe {
            let span = a.ty(escrito).span;
            saida.push((unit, Diagnostic::com_codigo(c::MAIN_FIRST_POSITIONAL_PARAMETER_TYPE, span, vazio.iter().copied())));
        }
    }
    saida
}
