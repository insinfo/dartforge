# Driver AOT LLVM → executável nativo

## Contrato

dartforge-native expõe NativeOptions (clang, rustc, optimize) e build_executable(ir, output, options). Defaults usam DARTFORGE_CLANG e DARTFORGE_RUSTC quando definidas; caso contrário, clang e rustc no PATH.

A entrada é LLVM IR do subconjunto com void @dartforge_entry(), void @dartforge_print_i64(i64), void @dartforge_print_bool(i8), void @dartforge_print_null() e void @dartforge_null_assert_fail(). Esta última função não retorna: escreve o diagnóstico de asserção nula em stderr e encerra com código 101, sem exceções Dart capturáveis. Não há strings, classes, GC ou seleção de destino cross. IR com target triple ou target datalayout explícitos é rejeitado; Clang e rustc precisam pertencer ao toolchain nativo compatível do host.

1. Diretório temporário exclusivo é reservado junto da saída.
2. Clang recebe argumentos separados: -x ir -c -O0 ou -O2 e produz objeto.
3. rustc compila o harness Rust 2024 embarcado em dartforge-runtime, com -C link-arg apontando para o objeto e opt-level correspondente.
4. Só após sucesso, o destino é reservado com create_new e recebe o executável. Um destino preexistente nunca é substituído, inclusive se surgir durante o build.
5. Erros de ferramentas preservam etapa, status, stdout e stderr. Falhas de build não publicam saída; erro durante cópia remove a saída parcial. Drop remove apenas o diretório reservado pelo driver.

Nenhum comando passa por shell. O driver não executa o programa gerado. O runtime usa std Rust para impressão e inicialização; não inicia Dart VM nem chama dart compile.

## Fronteira unsafe

As crates compiladas pelo workspace mantêm unsafe_code=forbid. runtime/src/runtime_main.rs é um recurso include_str compilado separadamente pelo rustc. Nesse harness, unsafe fica restrito aos nomes ABI reservados, à declaração extern e à chamada de dartforge_entry. Print recebe somente i64 ou u8, nunca ponteiros; u8 evita estados inválidos de bool Rust. A segurança exige IR emitido pelo compilador com as assinaturas acordadas. O driver não é verificador de segurança para IR arbitrário.

## Medição por fase

build_executable_with_report(ir, output, options) retorna BuildReport. build_executable mantém a API anterior e descarta o relatório. Os campos de tempo são Duration medidos por Instant, sem dependência do relógio civil:

| Campo Rust | Campo JSON da CLI | Intervalo medido |
| --- | --- | --- |
| write_ir_runtime | prepare_ns | Gravação dos dois arquivos de entrada |
| clang | clang_ns | Comando Clang completo, incluindo inicialização, espera e captura das saídas |
| rustc_link | rustc_link_ns | Preparação do argumento de ligação e processo rustc completo |
| publish | publish_ns | Abertura do binário, reserva create_new, cópia, permissões e sync_all |
| total | driver_total_ns | Toda a chamada, incluindo validações, preparação de caminhos e limpeza do staging |
| executable_bytes | executable_bytes | Quantidade de bytes efetivamente copiados para a saída |

A CLI converte Duration com as_nanos(); os intervalos não se sobrepõem e total é pelo menos sua soma. frontend_ns mede separadamente carregamento, análise e emissão de IR; total_ns pertence à chamada completa da CLI. O relatório só existe após sucesso. Cada chamada recompila o harness e religa o programa: estas medições não representam compilação incremental nem cache. Tempos de processos incluem escalonamento do sistema e custos de inicialização; não são tempos exclusivos de CPU. Limpeza continua best-effort, como no driver anterior.

## Referências oficiais consultadas

- [Clang command guide](https://clang.llvm.org/docs/CommandGuide/clang.html): -x seleciona linguagem de entrada; -c produz objeto sem ligação; níveis -O selecionam otimização.
- Referência local references/dartino-llvm/compile.sh, revisão 07f12fa22c5a3c2652d258dde3770848e4542819: encadeia exportação de bytecode, llvm-codegen, llvm-dis, llc, as, objcopy e ligação g++. Isso orienta separar geração de objeto e ligação nas medições, sem incorporar o runtime Dartino nem seu bootstrap. O checkout LLVM local não inclui a documentação Clang; o guia oficial acima foi consultado novamente para os limites dessas etapas.
- [rustc codegen options](https://doc.rust-lang.org/rustc/codegen-options/index.html#link-arg): link-arg passa argumento ao linker usado pelo toolchain Rust.
- [Rust 2024 unsafe attributes](https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-attributes.html): no_mangle exige marcação unsafe.
- [Rust 2024 extern blocks](https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-extern.html): a declaração externa exige explicitar o contrato inseguro.

Fontes consultadas em 2026-09-20. Os testes unitários não exigem LLVM instalado: cobrem ferramenta ausente, falha com diagnóstico, limpeza e preservação da saída. A integração com LLVM e executáveis reais é validada separadamente pelo pipeline principal.

## Verificação inicial

Em 2026-09-20, o teste real_executable_prints_ints_and_bools foi executado explicitamente com D:/LLVM/22.1.8/bin/clang.exe e rustc 1.98.1 (host x86_64-pc-windows-msvc). Gerou e executou dois binários, O0 e O2, ambos com saída -42, true e false. O teste é marcado ignore para não impor LLVM às verificações unitárias comuns; executar com --ignored quando o toolchain nativo estiver configurado.

Os quatro testes de falha/preservação/limpeza, o doctest público e Clippy all-targets com -D warnings também passaram. Unix execute bits são copiados do binário temporário para a saída final; a execução em Ubuntu pertence à validação de CI.
