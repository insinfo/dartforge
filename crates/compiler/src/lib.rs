//! Shared synchronous pipeline for CLI and future editor/build tools.
use dartforge_diagnostics::Diagnostic;
pub fn compile(source: &str) -> Result<String, Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let ast = dartforge_parser::parse(&tokens, source.len())?;
    dartforge_semantic::validate(&ast)?;
    Ok(dartforge_codegen::emit(&dartforge_hir::lower(ast)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_unicode_and_quotes() {
        let js = compile("void main() { print('Olá \"Rust\"'); print('😀'); }").unwrap();
        assert!(js.contains("console.log(\"Olá \\\"Rust\\\"\");"));
        assert!(js.contains("console.log(\"😀\");"));
    }
    #[test]
    fn rejects_unsupported_instead_of_silently_miscompiling() {
        for source in [
            "class App {}",
            "void main() { print(1.5); }",
            "void main() { print('$name'); }",
            "void main() { print('a\\nb'); }",
            "void main() {} trailing",
            "void main() {",
            "void main() { print('unterminated); }",
        ] {
            assert!(compile(source).is_err(), "unexpected success: {source}");
        }
    }
    #[test]
    fn comments_and_empty_main() {
        assert!(compile("// test\nvoid main() { // ok\n}").is_ok());
    }
    #[test]
    fn diagnostic_span_is_a_valid_utf8_boundary() {
        let source = "void main() { 😀 }";
        let e = compile(source).unwrap_err();
        assert_eq!(&source[e.span.start..e.span.end], "😀");
    }
}

#[cfg(test)]
mod subset_tests {
    use super::compile;

    #[test]
    fn accepts_arithmetic_variables_and_scopes() {
        let js = compile("void main() { int x = 2; final y = x + 3 * 4; { bool ok = y > 0; print(ok); } x = y; print(x); }").unwrap();
        assert!(js.starts_with("//"));
        assert!(js.contains("console.log"));
    }

    #[test]
    fn rejects_name_type_and_mutability_errors() {
        for source in [
            "void main() { print(missing); }",
            "void main() { var x = x; }",
            "void main() { var x = 1; var x = 2; }",
            "void main() { final x = 1; x = 2; }",
            "void main() { int x = true; }",
            "void main() { var x = 1; x = 'wrong'; }",
            "void main() { print(1 + true); }",
            "void main() { print(!1); }",
            "void main() { print(true && 1); }",
            "void main() { { var hidden = 1; } print(hidden); }",
            "void main() { var print = 1; print('wrong builtin'); }",
        ] {
            let error = compile(source).expect_err(source);
            assert!(error.span.start <= error.span.end, "{source}");
            assert!(error.span.end <= source.len(), "{source}");
        }
    }
}

#[cfg(test)]
mod function_tests {
    use super::compile;
    #[test]
    fn compiles_forward_recursion_and_void_returns() {
        let source = "void main() { print(factorial(5)); log(); } int factorial(int n) { if (n <= 1) { return 1; } else { return n * factorial(n - 1); } } void log() { return print('done'); }";
        assert!(compile(source).is_ok());
    }
    #[test]
    fn rejects_invalid_function_contracts() {
        for source in [
            "void main() { print(unknown(1)); }",
            "int f(int x) { return x; } void main() { print(f()); }",
            "int f(int x) { return x; } void main() { print(f(true)); }",
            "int f() { return true; } void main() {}",
            "int f() {} void main() {}",
            "int f() { if (true) { return 1; } } void main() {}",
            "int f() { return; } void main() {}",
            "void f() { return 1; } void main() {}",
            "void f() {} void main() { var x = f(); }",
            "void f() {} void main() { print(f()); }",
            "void main() { if (1) {} }",
            "int f(int x, int x) { return x; } void main() {}",
            "int f() { return 1; } int f() { return 2; } void main() {}",
            "int f() { return 1; } void main() { var f = 1; print(f()); }",
            "void main() { if (true) { var local = 1; } print(local); }",
            "void main() { print(print(1)); }",
        ] {
            assert!(compile(source).is_err(), "{source}");
        }
    }
}
