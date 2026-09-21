//! Pipeline compartilhado de compilação Dart 3.6.2 para JavaScript e LLVM IR.
use dartforge_diagnostics::Diagnostic;
mod session;
pub use dartforge_linker::LinkStats;
pub use dartforge_macros::{MacroCacheStats, MacroSession};
pub use dartforge_packages::{CompilationEnvironment, CompilationTarget};
pub use session::{Compilation, CompilerSession, SessionStats};
/// Compila o subconjunto suportado de Dart em um módulo JavaScript ESM.
///
/// Executa tokenização, parsing, validação semântica, lowering estrutural e emissão.
/// O módulo exporta e executa `main`; use um arquivo `.mjs` ou carregamento de módulo.
///
/// # Erros
///
/// Retorna o primeiro diagnóstico léxico, sintático ou semântico. Recursos ainda não
/// suportados são rejeitados; não existe fallback silencioso para outro compilador.
///
/// # Exemplos
///
/// ```
/// let fonte = "int dobro(int n) { return n * 2; } void main() { print(dobro(21)); }";
/// let javascript = dartforge_compiler::compile(fonte)?;
/// assert!(javascript.contains("export function main"));
/// # Ok::<(), dartforge_diagnostics::Diagnostic>(())
/// ```
pub fn compile(source: &str) -> Result<String, Diagnostic> {
    compile_with_optimization(source, Optimization::None)
}

/// Seleciona passes opcionais após a validação semântica completa do subconjunto.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Optimization {
    /// Emissão direta, voltada à baixa latência de desenvolvimento.
    #[default]
    None,
    /// Avaliação de constantes puras com limites de tamanho e faixa numérica.
    Constants,
}

/// Opções independentes de transformação; desativadas por padrão.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompileOptions {
    pub optimization: Optimization,
    pub merge_identical_functions: bool,
    /// Remove declarações inalcançáveis no backend JavaScript; desativado para menor latência.
    pub tree_shaking: bool,
}
impl From<Optimization> for CompileOptions {
    /// Preserva o comportamento das APIs anteriores.
    fn from(optimization: Optimization) -> Self {
        Self {
            optimization,
            merge_identical_functions: false,
            tree_shaking: false,
        }
    }
}
/// Compila uma unidade e aplica somente a política de otimização solicitada.
///
/// A análise semântica precede qualquer otimização, inclusive em ramos constantes.
/// Esta opção aplica apenas constantes; use CompileOptions para passes adicionais.
///
/// # Erros
/// Retorna os mesmos diagnósticos de [`compile`]; otimização não oculta código inválido.
///
/// # Exemplos
/// ```
/// use dartforge_compiler::{compile_with_optimization, Optimization};
/// let js = compile_with_optimization("void main() { print(2 + 3); }", Optimization::Constants)?;
/// assert!(js.contains("console.log(5)"));
/// # Ok::<(), dartforge_diagnostics::Diagnostic>(())
/// ```
pub fn compile_with_optimization(
    source: &str,
    optimization: Optimization,
) -> Result<String, Diagnostic> {
    compile_with_options(source, optimization.into())
}
/// Compila JavaScript com opções independentes após análise completa.
/// # Erros
/// Retorna diagnósticos de sintaxe e semântica antes de transformar o programa.
pub fn compile_with_options(source: &str, options: CompileOptions) -> Result<String, Diagnostic> {
    compile_with_macro_session(source, options, &mut MacroSession::with_limits(0, 0))
}

/// Compila com cache limitado de planos de macro, separado da saída JavaScript.
/// # Erros
/// Retorna os mesmos diagnósticos de sintaxe e semântica, inclusive em acertos de plano.
pub fn compile_with_macro_session(
    source: &str,
    options: CompileOptions,
    macros: &mut MacroSession,
) -> Result<String, Diagnostic> {
    dartforge_packages::validate_language_version(source)?;
    let tokens = dartforge_lexer::lex(source)?;
    let mut ast = dartforge_parser::parse(&tokens, source.len())?;
    reject_unresolved_native(&ast)?;
    let expansion = macros.expand(&mut ast, source.len())?;
    dartforge_hir::expand_mixins(&mut ast).map_err(|e| expansion.remap(e))?;
    let resolution = dartforge_semantic::analyze(&ast).map_err(|e| expansion.remap(e))?;
    if options.optimization == Optimization::Constants {
        dartforge_optimizer::fold_constants(&mut ast);
    }
    if options.merge_identical_functions {
        dartforge_optimizer::merge_identical_functions(&mut ast, &resolution);
    }
    if options.tree_shaking {
        dartforge_optimizer::tree_shake(&mut ast, &resolution);
    }
    Ok(dartforge_codegen::emit(&dartforge_hir::lower_resolved(
        ast, resolution,
    )))
}
/// Compila uma entrada e suas bibliotecas relativas ou package: em um módulo JavaScript.
///
/// Cada biblioteca mantém namespace e privacidade próprios; apenas imports diretos
/// tornam seus namespaces exportados visíveis. O modo de otimização é aplicado depois
/// da resolução e da análise semântica do programa combinado.
///
/// # Erros
/// Retorna caminho e intervalo local para falhas de leitura, diretivas não suportadas,
/// nomes ambíguos, sintaxe inválida ou incompatibilidade semântica.
///
/// # Exemplos
/// ```no_run
/// use dartforge_compiler::{compile_path, Optimization};
/// let javascript = compile_path(std::path::Path::new("lib/main.dart"), Optimization::None)?;
/// assert!(javascript.contains("export function main"));
/// # Ok::<(), dartforge_packages::GraphError>(())
/// ```
pub fn compile_path(
    path: &std::path::Path,
    optimization: Optimization,
) -> Result<String, dartforge_packages::GraphError> {
    compile_path_with_options(path, optimization.into())
}
/// Compila arquivos e imports com todas as opções explícitas.
/// # Erros
/// Retorna falhas de carga, resolução e análise.
pub fn compile_path_with_options(
    path: &std::path::Path,
    options: CompileOptions,
) -> Result<String, dartforge_packages::GraphError> {
    compile_path_with_environment(path, options, &CompilationEnvironment::javascript())
}
/// Compila JavaScript selecionando imports/exports no ambiente explícito.
///
/// # Erros
/// Rejeita perfis nativo/Wasm e retorna diagnósticos das bibliotecas selecionadas.
pub fn compile_path_with_environment(
    path: &std::path::Path,
    options: CompileOptions,
    environment: &CompilationEnvironment,
) -> Result<String, dartforge_packages::GraphError> {
    validate_environment(path, environment, CompilationTarget::JavaScript)?;
    let graph = dartforge_packages::load_with_environment(path, environment)?;
    compile_loaded_graph(&graph, options)
}
/// Impede que seleção de bibliotecas de um alvo seja emitida por outro backend.
pub(crate) fn validate_environment(
    path: &std::path::Path,
    environment: &CompilationEnvironment,
    expected: CompilationTarget,
) -> Result<(), dartforge_packages::GraphError> {
    if environment.target() != expected {
        return Err(dartforge_packages::GraphError {
            path: path.to_owned(),
            span: None,
            message: if environment.target() == CompilationTarget::Wasm {
                "Wasm emission is not implemented; use graph --target wasm for graph inspection"
                    .into()
            } else {
                format!(
                    "Compilation environment {:?} must match backend {expected:?}",
                    environment.target()
                )
            },
        });
    }
    Ok(())
}
/// Relatório de custo de uma solicitação, separando descoberta de front-end.
///
/// `load_ns` cobre descoberta do grafo, leitura de arquivos e resolução de
/// pacotes; `link` cobre o pipeline de compilação e a emissão. Nenhuma fase é
/// obtida por subtração: cada uma é cronometrada no próprio trecho.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompileReport {
    /// Descoberta do grafo, leitura das fontes e resolução de pacotes.
    pub load_ns: u128,
    /// Fases do front-end e da emissão; zero quando a saída foi reutilizada.
    pub link: LinkStats,
    /// Tempo total da solicitação, medido de ponta a ponta.
    pub total_ns: u128,
    /// A saída anterior foi reutilizada integralmente.
    pub cache_hit: bool,
}

/// Compila um caminho devolvendo o relatório de custo junto do JavaScript.
///
/// Esta rota não usa cache: mede sempre uma compilação fria, que é a linha de
/// base contra a qual qualquer reuso precisa ser comparado.
///
/// # Erros
/// Retorna os mesmos diagnósticos de [`compile_path_with_options`].
pub fn compile_path_with_report(
    path: &std::path::Path,
    options: CompileOptions,
) -> Result<(String, CompileReport), dartforge_packages::GraphError> {
    let total = std::time::Instant::now();
    let load = std::time::Instant::now();
    let environment = CompilationEnvironment::javascript();
    validate_environment(path, &environment, CompilationTarget::JavaScript)?;
    let graph = dartforge_packages::load_with_environment(path, &environment)?;
    let load_ns = load.elapsed().as_nanos();
    let mut link = LinkStats::default();
    let javascript = compile_loaded_graph_instrumented(
        &graph,
        options,
        &mut MacroSession::with_limits(0, 0),
        &mut link,
    )?;
    Ok((
        javascript,
        CompileReport {
            load_ns,
            link,
            total_ns: total.elapsed().as_nanos(),
            cache_hit: false,
        },
    ))
}

/// Compila um grafo recarregado pela rota compartilhada com a sessão.
pub(crate) fn compile_loaded_graph(
    graph: &dartforge_packages::SourceGraph,
    options: CompileOptions,
) -> Result<String, dartforge_packages::GraphError> {
    compile_loaded_graph_with_macros(graph, options, &mut MacroSession::with_limits(0, 0))
}

/// Reutiliza planos de macro da sessão sem reutilizar ASTs de bibliotecas anteriores.
pub(crate) fn compile_loaded_graph_with_macros(
    graph: &dartforge_packages::SourceGraph,
    options: CompileOptions,
    macros: &mut MacroSession,
) -> Result<String, dartforge_packages::GraphError> {
    compile_loaded_graph_instrumented(graph, options, macros, &mut LinkStats::default())
}

/// Compila o grafo carregado preenchendo o tempo por fase em `stats`.
pub(crate) fn compile_loaded_graph_instrumented(
    graph: &dartforge_packages::SourceGraph,
    options: CompileOptions,
    macros: &mut MacroSession,
    stats: &mut LinkStats,
) -> Result<String, dartforge_packages::GraphError> {
    validate_environment(
        &graph.units[graph.entry].path,
        &graph.environment,
        CompilationTarget::JavaScript,
    )?;
    // A unidade isolada preserva o caminho direto e evita resolver um namespace
    // sem imports. `directives_end == 0` é indispensável: um arquivo que declara
    // apenas `library x;` não tem aresta alguma, mas tem prefixo de diretivas, e
    // entregá-lo inteiro ao parser faria a diretiva virar erro de sintaxe.
    if graph.units.len() == 1
        && graph.units[0].imports.is_empty()
        && graph.units[0].exports.is_empty()
        && graph.units[0].parts.is_empty()
        && graph.units[0].directives_end == 0
    {
        return compile_unit_instrumented(&graph.units[0].source, options, macros, stats).map_err(
            |error| dartforge_packages::GraphError {
                path: graph.units[0].path.clone(),
                span: Some(error.span),
                message: error.message,
            },
        );
    }
    dartforge_linker::compile_graph_instrumented(
        graph,
        options.optimization == Optimization::Constants,
        options.merge_identical_functions,
        options.tree_shaking,
        macros,
        stats,
        |module| {
            dartforge_codegen::validate_javascript(module)?;
            Ok(dartforge_codegen::emit(module))
        },
    )
}

/// Executa o caminho de unidade isolada cronometrando cada fase.
///
/// Mantém o atalho que evita resolver namespaces quando não há imports, sem
/// abrir mão da medição: o caminho rápido também precisa ser mensurável.
fn compile_unit_instrumented(
    source: &str,
    options: CompileOptions,
    macros: &mut MacroSession,
    stats: &mut LinkStats,
) -> Result<String, Diagnostic> {
    dartforge_packages::validate_language_version(source)?;
    stats.units = 1;
    stats.source_bytes = source.len();
    let lex_start = std::time::Instant::now();
    let tokens = dartforge_lexer::lex(source)?;
    stats.lex_ns = lex_start.elapsed().as_nanos();
    stats.tokens = tokens.len();
    let parse_start = std::time::Instant::now();
    let mut ast = dartforge_parser::parse(&tokens, source.len())?;
    stats.parse_ns = parse_start.elapsed().as_nanos();
    reject_unresolved_native(&ast)?;
    let macros_start = std::time::Instant::now();
    let expansion = macros.expand(&mut ast, source.len())?;
    stats.macros_ns = macros_start.elapsed().as_nanos();
    let analyze_start = std::time::Instant::now();
    dartforge_hir::expand_mixins(&mut ast).map_err(|e| expansion.remap(e))?;
    let resolution = dartforge_semantic::analyze(&ast).map_err(|e| expansion.remap(e))?;
    stats.analyze_ns = analyze_start.elapsed().as_nanos();
    stats.classes = ast.classes.len();
    stats.functions = ast.functions.len();
    let optimize_start = std::time::Instant::now();
    if options.optimization == Optimization::Constants {
        dartforge_optimizer::fold_constants(&mut ast);
    }
    if options.merge_identical_functions {
        dartforge_optimizer::merge_identical_functions(&mut ast, &resolution);
    }
    if options.tree_shaking {
        dartforge_optimizer::tree_shake(&mut ast, &resolution);
    }
    stats.optimize_ns = optimize_start.elapsed().as_nanos();
    let emit_start = std::time::Instant::now();
    let javascript = dartforge_codegen::emit(&dartforge_hir::lower_resolved(ast, resolution));
    stats.emit_ns = emit_start.elapsed().as_nanos();
    stats.output_bytes = javascript.len();
    Ok(javascript)
}
/// Compila uma unidade validada para LLVM IR do subconjunto nativo.
///
/// Inteiros usam i64 com overflow modular, conforme o alvo nativo Dart. A otimização
/// de máquina é responsabilidade do driver LLVM, sem reaproveitar hipóteses do JS.
///
/// # Erros
/// Rejeita recursos ainda sem representação nativa, inclusive em código morto.
pub fn compile_llvm(source: &str) -> Result<String, Diagnostic> {
    compile_llvm_with_options(source, CompileOptions::default())
}
/// Compila LLVM com fusão opcional; constantes JS não são aplicadas ao alvo nativo.
/// # Erros
/// Retorna falhas semânticas ou recursos nativos não suportados.
pub fn compile_llvm_with_options(
    source: &str,
    options: CompileOptions,
) -> Result<String, Diagnostic> {
    if options.tree_shaking {
        return Err(Diagnostic::new(
            "tree shaking ainda exige o backend JavaScript",
            dartforge_diagnostics::Span { start: 0, end: 0 },
        ));
    }
    dartforge_packages::validate_language_version(source)?;
    let tokens = dartforge_lexer::lex(source)?;
    let mut ast = dartforge_parser::parse(&tokens, source.len())?;
    reject_unresolved_native(&ast)?;
    let expansion = dartforge_macros::expand(&mut ast, source.len())?;
    dartforge_hir::expand_mixins(&mut ast).map_err(|e| expansion.remap(e))?;
    let resolution = dartforge_semantic::analyze(&ast).map_err(|e| expansion.remap(e))?;
    if options.merge_identical_functions {
        dartforge_optimizer::merge_identical_functions(&mut ast, &resolution);
    }
    dartforge_llvm::emit(&dartforge_hir::lower_resolved(ast, resolution))
        .map_err(|e| expansion.remap(e))
}

/// Inspeciona a expansão experimental sem emitir código nem executar a aplicação.
/// # Erros
/// Retorna falhas de parsing, alvo de macro ou colisões entre declarações.
pub fn macro_expansion_report(
    source: &str,
) -> Result<dartforge_macros::ExpansionReport, Diagnostic> {
    dartforge_packages::validate_language_version(source)?;
    let tokens = dartforge_lexer::lex(source)?;
    let mut ast = dartforge_parser::parse(&tokens, source.len())?;
    dartforge_macros::expand(&mut ast, source.len())
}

/// Impede bindings sem resolução de biblioteca nas APIs de fonte isolada.
fn reject_unresolved_native(ast: &dartforge_syntax::Program<'_>) -> Result<(), Diagnostic> {
    if let Some(binding) = ast
        .functions
        .iter()
        .find_map(|function| function.native_binding.as_ref())
    {
        return Err(Diagnostic::new(
            "@Native exige import dart:ffi; use a API de compilação por arquivo",
            binding.span,
        ));
    }
    Ok(())
}

/// Compila uma entrada e seu grafo de bibliotecas para LLVM IR.
///
/// Reutiliza resolução de pacotes, privacidade e reexports do backend JavaScript.
/// Não aplica o passe de constantes JS; O0/O2 são selecionados no driver nativo.
///
/// # Erros
/// Retorna diagnósticos localizados para carga, análise e restrições do backend.
pub fn compile_path_llvm(path: &std::path::Path) -> Result<String, dartforge_packages::GraphError> {
    compile_path_llvm_with_options(path, CompileOptions::default())
}
/// Compila grafo LLVM com fusão estrutural opcional.
/// # Erros
/// Retorna diagnósticos localizados do grafo e backend.
pub fn compile_path_llvm_with_options(
    path: &std::path::Path,
    options: CompileOptions,
) -> Result<String, dartforge_packages::GraphError> {
    compile_path_llvm_with_environment(path, options, &CompilationEnvironment::native())
}
/// Compila LLVM após selecionar diretivas pelo perfil nativo AOT do Dart 3.6.2.
///
/// # Erros
/// Rejeita perfis JavaScript/Wasm e APIs de biblioteca sem implementação nativa.
pub fn compile_path_llvm_with_environment(
    path: &std::path::Path,
    options: CompileOptions,
    environment: &CompilationEnvironment,
) -> Result<String, dartforge_packages::GraphError> {
    if options.tree_shaking {
        return Err(dartforge_packages::GraphError {
            path: path.to_owned(),
            span: None,
            message: "tree shaking ainda exige o backend JavaScript".into(),
        });
    }
    validate_environment(path, environment, CompilationTarget::Native)?;
    let graph = dartforge_packages::load_with_environment(path, environment)?;
    if graph.units.len() == 1
        && graph.units[0].imports.is_empty()
        && graph.units[0].exports.is_empty()
        && graph.units[0].parts.is_empty()
        && graph.units[0].directives_end == 0
    {
        return compile_llvm_with_options(&graph.units[0].source, options).map_err(|error| {
            dartforge_packages::GraphError {
                path: graph.units[0].path.clone(),
                span: Some(error.span),
                message: error.message,
            }
        });
    }
    dartforge_linker::compile_graph_with_options(
        &graph,
        false,
        options.merge_identical_functions,
        dartforge_llvm::emit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    /// As APIs em memória e por arquivo devem respeitar o mesmo alvo de linguagem.
    #[test]
    fn language_version_is_checked_before_optimization() {
        for mode in [Optimization::None, Optimization::Constants] {
            assert!(compile_with_optimization("// @dart = 2.9\nvoid main() {}", mode).is_err());
            assert!(compile_with_optimization("// @dart = 3.6\nvoid main() {}", mode).is_ok());
            assert!(
                compile_with_optimization("/*\n// @dart = 2.9\n*/\nvoid main() {}", mode).is_ok()
            );
        }
    }
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
            "void main() { print('\\uD800'); }",
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

#[cfg(test)]
mod loop_tests {
    use super::compile;
    #[test]
    fn rejects_invalid_loop_control_scope_and_types() {
        for source in [
            "void main() { break; }",
            "void main() { continue; }",
            "void main() { while (1) {} }",
            "void main() { do {} while (1); }",
            "void main() { for (; 1; ) {} }",
            "void main() { for (var i = 0; i < 2; i++) {} print(i); }",
            "void main() { final n = 0; while (n < 2) { n++; } }",
            "void main() { for (final i = 0; i < 2; i++) {} }",
            "void main() { for (var i = i; i < 2; i++) {} }",
            "void stop() { break; } void main() { while (true) { stop(); } }",
            "void main() { var text = 'a'; text *= 2; }",
        ] {
            assert!(compile(source).is_err(), "{source}");
        }
    }
}

#[cfg(test)]
mod string_tests {
    use super::compile;
    #[test]
    fn rejects_malformed_unicode_without_invalid_byte_spans() {
        for source in [
            r#"void main() { print('\xG0'); }"#,
            r#"void main() { print('\u{}'); }"#,
            r#"void main() { print('\u{110000}'); }"#,
            r#"void main() { print('\u{1234567}'); }"#,
            r#"void main() { print('\uD800'); }"#,
            r#"void main() { print('\uDC00'); }"#,
            r#"void main() { print('ação\u12'); }"#,
        ] {
            let error = compile(source).expect_err(source);
            assert!(error.span.start <= error.span.end);
            assert!(source.is_char_boundary(error.span.start));
            assert!(source.is_char_boundary(error.span.end));
        }
    }
}
