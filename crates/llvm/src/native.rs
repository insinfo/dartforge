//! Adaptadores escalares de `@Native`, inspirados no contrato público SDK 3.6.2
//! sdk/lib/ffi/ffi.dart. A resolução é estática pelo linker do host: não implementa
//! assetId, DynamicLibrary, ponteiros, callbacks ou transições do GC do SDK.
use dartforge_diagnostics::Diagnostic;
use dartforge_hir::Module;
use dartforge_syntax::{NativeBinding, NativeType, Type};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Tipos C permanecem separados do int i64 utilizado pelo código Dart.
fn ir(ty: NativeType) -> &'static str {
    match ty {
        NativeType::Void => "void",
        NativeType::Int32 => "i32",
        NativeType::Int64 => "i64",
    }
}

/// Recusa injeção de IR e símbolos controlados pelo compilador/runtime/linker.
fn allowed_symbol(symbol: &str) -> bool {
    let mut bytes = symbol.bytes();
    let valid = bytes
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_');
    valid
        && !["dartforge_", "df_", "__", "_Unwind", "rust_"]
            .iter()
            .any(|prefix| symbol.starts_with(prefix))
        && !matches!(
            symbol,
            "main"
                | "WinMain"
                | "wmain"
                | "DllMain"
                | "malloc"
                | "calloc"
                | "realloc"
                | "free"
                | "memcpy"
                | "memmove"
                | "memset"
                | "memcmp"
                | "strlen"
                | "printf"
                | "fprintf"
                | "sprintf"
                | "snprintf"
                | "puts"
                | "putchar"
                | "exit"
                | "abort"
                | "atexit"
                | "_exit"
                | "_start"
                | "bcmp"
                | "bcopy"
                | "bzero"
        )
}

/// Valida também HIR construída diretamente, antes de produzir qualquer declaração.
pub(super) fn validate(module: &Module<'_>) -> Result<(), Diagnostic> {
    let mut symbols = BTreeMap::new();
    for function in &module.functions {
        let Some(binding) = &function.native_binding else {
            continue;
        };
        let fail = |message| Diagnostic::new(message, binding.span);
        if !allowed_symbol(&binding.symbol) {
            return Err(fail(
                "símbolo Native inválido ou reservado pelo backend/runtime",
            ));
        }
        if function.is_getter || !function.body.is_empty() || !function.type_parameters.is_empty() {
            return Err(fail(
                "Native requer função escalar externa sem corpo ou parâmetros genéricos",
            ));
        }
        let expected_return = if binding.result == NativeType::Void {
            Type::Void
        } else {
            Type::Int
        };
        if function.return_type != expected_return
            || function.parameters.len() != binding.parameters.len()
            || function
                .parameters
                .iter()
                .zip(&binding.parameters)
                .any(|(dart, native)| dart.ty != Type::Int || *native == NativeType::Void)
        {
            return Err(fail(
                "assinatura Native incompatível: somente int e void escalares",
            ));
        }
        let signature = (binding.result, &binding.parameters);
        if let Some(previous) = symbols.insert(&binding.symbol, signature)
            && previous != signature
        {
            return Err(fail(
                "símbolo Native reutilizado com assinaturas C incompatíveis",
            ));
        }
    }
    for method in module
        .classes
        .iter()
        .flat_map(|class| class.methods.iter().chain(&class.abstract_methods))
        .chain(
            module
                .extensions
                .iter()
                .flat_map(|extension| &extension.methods),
        )
    {
        if let Some(binding) = &method.native_binding {
            return Err(Diagnostic::new(
                "Native suporta apenas funções top-level neste backend",
                binding.span,
            ));
        }
    }
    Ok(())
}

/// Declara uma vez cada símbolo, em ordem determinística independente do HashMap.
pub(super) fn declarations(module: &Module<'_>) -> String {
    let symbols: BTreeMap<_, _> = module
        .functions
        .iter()
        .filter_map(|function| function.native_binding.as_ref())
        .map(|binding| (&binding.symbol, binding))
        .collect();
    let mut output = String::new();
    for (symbol, binding) in symbols {
        let parameters = binding
            .parameters
            .iter()
            .map(|&ty| ir(ty))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            output,
            "declare {} @{symbol}({parameters})",
            ir(binding.result)
        )
        .unwrap();
    }
    output
}

/// Faz conversões de largura com sinal explícitas, sem transformar handles em pointers.
pub(super) fn wrapper(binding: &NativeBinding<'_>, symbol: &str) -> String {
    let result = if binding.result == NativeType::Void {
        "void"
    } else {
        "i64"
    };
    let parameters = (0..binding.parameters.len())
        .map(|index| format!("i64 %a{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut output = format!("define {result} @{symbol}({parameters}) {{\nentry:\n");
    let mut arguments = vec![];
    for (index, &ty) in binding.parameters.iter().enumerate() {
        if ty == NativeType::Int32 {
            writeln!(output, "  %c{index} = trunc i64 %a{index} to i32").unwrap();
            arguments.push(format!("i32 %c{index}"));
        } else {
            arguments.push(format!("i64 %a{index}"));
        }
    }
    let assignment = if binding.result == NativeType::Void {
        ""
    } else {
        "%native_result = "
    };
    writeln!(
        output,
        "  {assignment}call {} @{}({})",
        ir(binding.result),
        binding.symbol,
        arguments.join(", ")
    )
    .unwrap();
    match binding.result {
        NativeType::Void => output.push_str("  ret void\n"),
        NativeType::Int64 => output.push_str("  ret i64 %native_result\n"),
        NativeType::Int32 => {
            output.push_str("  %extended = sext i32 %native_result to i64\n  ret i64 %extended\n")
        }
    }
    output.push_str("}\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercita diretamente a validação defensiva do backend sobre uma AST parseada.
    fn emit(source: &str) -> Result<String, Diagnostic> {
        let tokens = dartforge_lexer::lex(source)?;
        let program = dartforge_parser::parse(&tokens, source.len())?;
        crate::emit(&dartforge_hir::lower(program))
    }

    /// Int32 exige truncamento e extensão de sinal; duas declarações compatíveis compartilham símbolo.
    #[test]
    fn scalar_wrappers_preserve_widths_and_deduplicate_declarations() {
        let ir = emit("@Native<Int32 Function(Int32, Int64)>(symbol:'user_mix') external int mix(int a,int b); @Native<Int32 Function(Int32, Int64)>(symbol:'user_mix') external int alias(int a,int b); @Native<Void Function(Int64)>(symbol:'user_sink') external void sink(int a); void main(){}").unwrap();
        assert_eq!(ir.matches("declare i32 @user_mix(i32, i64)").count(), 1);
        assert!(ir.contains("trunc i64 %a0 to i32"));
        assert!(ir.contains("sext i32 %native_result to i64"));
        assert!(ir.contains("call void @user_sink(i64 %a0)"));
    }

    /// Mesmo código morto não pode introduzir conflitos ABI ou símbolos reservados.
    #[test]
    fn rejects_conflicting_signatures_and_internal_symbols() {
        let source = "@Native<Int32 Function(Int32)>(symbol:'user_same') external int a(int x); @Native<Int64 Function(Int64)>(symbol:'user_same') external int b(int x); void main(){}";
        assert!(emit(source).unwrap_err().message.contains("incompatíveis"));
        for symbol in [
            "dartforge_print_i64",
            "df_fn_0",
            "main",
            "malloc",
            "memcpy",
            "__chkstk",
            "bad\"symbol",
        ] {
            assert!(!allowed_symbol(symbol));
        }
        assert!(allowed_symbol("project_sum_32"));
        let error =
            emit("@Native<Int64 Function()>(symbol:'malloc') external int x(); void main(){}")
                .unwrap_err();
        assert!(error.message.contains("reservado"));
        assert!(error.span.end > error.span.start);
    }
}
