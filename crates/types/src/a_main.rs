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
use dartforge_diagnostics::{Diagnostic, Span};
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
    let mut output = Vec::new();
    let Some(sym_main) = interner.lookup("main") else { return output };
    for (i, f) in program.functions.iter().enumerate() {
        // Só a função de topo: um método `main` de classe não conta, nem um
        // getter (que não tem lista de parâmetros).
        if f.library != lib || f.name != sym_main || f.class.is_some() || f.extension.is_some() || f.kind != FunctionKind::Function {
            continue;
        }
        let FunctionRef::Function { unit, function } = f.node else { continue };
        let a = &program.unit(unit).ast;
        let func = a.function(function);
        let (Some(ps), Some(name)) = (&func.parameters, func.name) else { continue };
        let empty: Vec<&str> = Vec::new();
        let required = ps.iter().filter(|p| p.kind == ParameterKind::Required).count();
        if required > 2 {
            output.push((unit, Diagnostic::com_codigo(c::MAIN_HAS_TOO_MANY_REQUIRED_POSITIONAL_PARAMETERS, name.span, empty.iter().copied())));
        }
        if ps.iter().any(|p| p.kind == ParameterKind::Named && p.required) {
            output.push((unit, Diagnostic::com_codigo(c::MAIN_HAS_REQUIRED_NAMED_PARAMETERS, name.span, empty.iter().copied())));
        }
        // O primeiro posicional (obrigatório ou opcional), pelo tipo escrito:
        // sem tipo ele é `dynamic` e a condição não dispara.
        let Some(k) = ps.iter().position(|p| p.kind != ParameterKind::Named) else { continue };
        let p = &ps[k];
        // `typeOrSelf` do `notDefault`: o tipo escrito de um parâmetro simples;
        // o parâmetro inteiro (sem o valor padrão) na forma de função
        // (`void args()`), em `this.x` e em `super.x`.
        let span = if p.function_parameters.is_none() && !p.this_ && !p.super_ {
            let Some(written) = p.ty else { continue };
            a.ty(written).span
        } else {
            let start = p.metadata.first().map_or(p.span.start, |m| m.span.start.min(p.span.start));
            let end = match p.default_value {
                Some(d) => {
                    let source = program.unit(unit).source.as_bytes();
                    let mut i = a.expr(d).span.start;
                    while i > start && source[i - 1].is_ascii_whitespace() {
                        i -= 1;
                    }
                    if i > start && matches!(source[i - 1], b'=' | b':') {
                        i -= 1;
                    }
                    while i > start && source[i - 1].is_ascii_whitespace() {
                        i -= 1;
                    }
                    i
                }
                None => p.span.end,
            };
            Span { start: start, end: end }
        };
        let Some(data) = outline.functions.get(i) else { continue };
        let Some(ty) = data.parameters.get(k).map(|p| p.ty) else { continue };
        let Some(list) = core.list_class else { continue };
        let string_list = table.intern(Type::Interface { class: list, args: vec![core.string].into_boxed_slice(), nullable: false });
        let fits = {
            let mut env = SubtypeEnv::new(table, &outline.hierarchy, core);
            is_subtype(string_list, ty, &mut env)
        };
        if !fits {
            output.push((unit, Diagnostic::com_codigo(c::MAIN_FIRST_POSITIONAL_PARAMETER_TYPE, span, empty.iter().copied())));
        }
    }
    output
}
