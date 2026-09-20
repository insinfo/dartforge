# Driver AOT LLVM → executável nativo

## Contrato

dartforge-native expõe NativeOptions (clang, rustc, optimize) e build_executable(ir, output, options). Defaults usam DARTFORGE_CLANG e DARTFORGE_RUSTC quando definidas; caso contrário, clang e rustc no PATH.

A entrada é LLVM IR do subconjunto com void @dartforge_entry(), void @dartforge_print_i64(i64) e void @dartforge_print_bool(i8). Não há strings, null, classes, GC ou seleção de destino cross. IR com target triple ou target datalayout explícitos é rejeitado; Clang e rustc precisam pertencer ao toolchain nativo compatível do host.

1. Diretório temporário exclusivo é reservado junto da saída.
2. Clang recebe argumentos separados: -x ir -c -O0 ou -O2 e produz objeto.
3. rustc compila o harness Rust 2024 embarcado em dartforge-runtime, com -C link-arg apontando para o objeto e opt-level correspondente.
4. Só após sucesso, o destino é reservado com create_new e recebe o executável. Um destino preexistente nunca é substituído, inclusive se surgir durante o build.
5. Erros de ferramentas preservam etapa, status, stdout e stderr. Falhas de build não publicam saída; erro durante cópia remove a saída parcial. Drop remove apenas o diretório reservado pelo driver.

Nenhum comando passa por shell. O driver não executa o programa gerado. O runtime usa std Rust para impressão e inicialização; não inicia Dart VM nem chama dart compile.

## Fronteira unsafe

As crates compiladas pelo workspace mantêm unsafe_code=forbid. runtime/src/runtime_main.rs é um recurso include_str compilado separadamente pelo rustc. Nesse harness, unsafe fica restrito aos nomes ABI reservados, à declaração extern e à chamada de dartforge_entry. Print recebe somente i64 ou u8, nunca ponteiros; u8 evita estados inválidos de bool Rust. A segurança exige IR emitido pelo compilador com as assinaturas acordadas. O driver não é verificador de segurança para IR arbitrário.

## Referências oficiais consultadas

- [Clang command guide](https://clang.llvm.org/docs/CommandGuide/clang.html): -x seleciona linguagem de entrada; -c produz objeto sem ligação; níveis -O selecionam otimização.
- [rustc codegen options](https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg): link-arg passa argumento ao linker usado pelo toolchain Rust.
- [Rust 2024 unsafe attributes](https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-attributes.html): no_mangle exige marcação unsafe.
- [Rust 2024 extern blocks](https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-extern.html): a declaração externa exige explicitar o contrato inseguro.

Fontes consultadas em 2026-09-20. Os testes unitários não exigem LLVM instalado: cobrem ferramenta ausente, falha com diagnóstico, limpeza e preservação da saída. A integração com LLVM e executáveis reais é validada separadamente pelo pipeline principal.

## Verificação inicial

Em 2026-09-20, o teste real_executable_prints_ints_and_bools foi executado explicitamente com D:/LLVM/22.1.8/bin/clang.exe e rustc 1.98.1 (host x86_64-pc-windows-msvc). Gerou e executou dois binários, O0 e O2, ambos com saída -42, true e false. O teste é marcado ignore para não impor LLVM às verificações unitárias comuns; executar com --ignored quando o toolchain nativo estiver configurado.

Os quatro testes de falha/preservação/limpeza, o doctest público e Clippy all-targets com -D warnings também passaram. Unix execute bits são copiados do binário temporário para a saída final; a execução em Ubuntu pertence à validação de CI.
