# JIT ORCv2 — execução em memória do subconjunto nativo

O DartForge passa a ter dois perfis de execução nativa sobre **o mesmo LLVM IR**:

| | Desenvolvimento (`crates/jit`) | Produção (`crates/native`) |
| --- | --- | --- |
| Comando | `dartforge run <entrada.dart>` | `dartforge aot <entrada.dart> <saida.exe>` |
| Geração de código | ORCv2 (`LLJIT`), em memória | Clang, em processo separado |
| Runtime nativo | endereços das funções Rust publicados como símbolos absolutos | `rustc` compila `RUNTIME_MAIN` e o linker resolve os símbolos |
| Artefato | nenhum | executável no disco |
| Isolamento do programa | **nenhum**: executa no processo do compilador | processo próprio |

O contrato entre os dois é observável e testado: `crates/jit/tests/execucao.rs`
compila um programa Dart uma única vez, executa o IR resultante pelos dois
caminhos e exige saída idêntica. Divergir em tempo de compilação é esperado;
divergir em resultado é defeito.

## O que executa

Exatamente o subconjunto que `dartforge_compiler::compile_llvm_with_options`
aceita — nem mais, nem menos. O JIT não tem emissor próprio: ele recebe o IR
textual pronto. Na prática isso cobre `int`/`bool`/`String`, classes com campos,
métodos e despacho virtual, enums, controle de fluxo e `print`, com o coletor
preciso de `crates/runtime` participando por frames de raízes explícitas.

O que o emissor nativo recusa continua recusado, com a mesma mensagem: `double` e
`num`, coleções, genéricos reificados, funções como valores, records, `Duration`
e `Timer`. Isso é limite de `crates/llvm`, não deste crate.

## O que não executa

* **`@Native` de `dart:ffi`.** O AOT resolve esses símbolos ligando objetos
  nativos passados em `--link-object`. A sessão JIT define apenas os nomes de
  [runtime](#como-o-runtime-é-ligado); um `@Native` não resolvido vira erro na
  etapa `lookup`, não uma chamada para endereço errado.
* **Dois programas completos na mesma sessão.** Todo programa nativo define
  `@dartforge_entry`, e cada nome existe uma única vez na `JITDylib`. O segundo
  `add_ir_module` devolve erro na etapa `add-module`. Um programa por sessão, ou
  módulos com símbolos distintos — como no teste `two_modules_share_one_session`.
* **Execução em outra thread.** O heap gerenciado é `thread_local`, como no
  harness AOT. `JitSession` contém ponteiros crus e por isso não é `Send` nem
  `Sync`: o compilador impede o engano em vez de deixá-lo virar heap vazio em
  tempo de execução.
* **Hot reload.** Esta etapa entrega apenas a execução. Recarregar código sem
  reiniciar exige stubs indiretos
  (`LLVMOrcCreateLocalIndirectStubsManager`/`LazyCallThroughManager`) e um
  protocolo de gerações; nada disso está aqui.

### Falhas que derrubam o processo hospedeiro

É a consequência direta de não haver isolamento, e vale registrar explicitamente:

* `dartforge_null_assert_fail` escreve `Null check operator used on a null value`
  em stderr e encerra o processo com código 101 — idêntico ao AOT, exceto que no
  JIT o processo encerrado é o do próprio compilador.
* Um erro **interno** do runtime (handle inválido, tipo errado num campo) chega
  como `panic` dentro de uma função `extern "C"`, o que aborta o processo. A
  alternativa seria desenrolar por quadros de pilha gerados pelo LLVM, que não
  sabem desenrolar. Abortar é o final definido; `crates/runtime/CONTRACT.md` já
  trata esses casos como erro de compilador, não como exceção Dart.

Nenhuma dessas é erro *esperado*. Todo erro esperado — IR inválido, símbolo
ausente, símbolo duplicado, falha ao descarregar módulo — vira `Result` com
`JitError { stage, message }` em português, com o diagnóstico original do LLVM
anexado. Não há `panic` em caminho de erro esperado.

## Como o runtime é ligado

Esta é a diferença estrutural entre os dois perfis, e a decisão menos óbvia do
crate.

`crates/runtime` **não** exporta os símbolos da ABI nativa como biblioteca. Ele
publica `RUNTIME_MAIN`, um programa Rust completo em forma de texto
(`include_str!` de `heap.rs` + `runtime_main.rs`), que o driver AOT grava no
staging e manda o `rustc` compilar. Os `#[unsafe(no_mangle)] extern "C" fn` só
existem *nesse* binário. O processo que hospeda o JIT não os tem.

Logo, registrar o gerador de símbolos do processo
(`LLVMOrcCreateDynamicLibrarySearchGeneratorForProcess`) **não resolveria nada**:
`dartforge_print_i64` não está entre os símbolos do processo. Além disso, esse
gerador exporia ao código gerado todo símbolo do executável hospedeiro, uma
superfície muito maior que o contrato de `crates/llvm`, e resolveria por
acidente um nome homônimo em vez de falhar alto.

A sessão faz o equivalente exato da ligação AOT, pela via explícita:
`crates/jit/src/runtime.rs` reimplementa a mesma ABI sobre o mesmo
`dartforge_runtime::heap::Heap`, e `crates/jit/src/ffi.rs` publica os endereços
dessas funções como símbolos absolutos (`LLVMOrcAbsoluteSymbols` +
`LLVMOrcJITDylibDefine`) na `JITDylib` principal, com os nomes decorados por
`LLVMOrcLLJITMangleAndIntern` para casar com a decoração que o `lookup` aplica.

São 18 nomes, os mesmos que `crates/llvm` declara no IR e que o harness AOT
define. A lista está em `dartforge_jit::RUNTIME_SYMBOLS` e é verificada por
teste. **Toda mudança em `crates/runtime/src/runtime_main.rs` tem contrapartida
em `crates/jit/src/runtime.rs`**; o teste diferencial é o que impede a divergência
de passar despercebida.

### Captura de saída

O runtime do JIT imprime no stdout do processo, como o AOT. Para o teste
diferencial isso não serve — seria preciso redirecionar o stdout do processo de
teste, o que não é confiável sob execução paralela. Por isso as funções de
impressão escrevem num destino `thread_local` que `run_entry_capturing` e
`run_ir_capturing` desviam para um buffer, com quebras `\n` em qualquer sistema.
É a única diferença deliberada de comportamento entre os dois runtimes, e ela
não afeta o texto produzido.

## Ciclo de vida e liberação

Cada módulo adicionado recebe um `LLVMOrcResourceTracker` próprio
(`LLVMOrcJITDylibCreateResourceTracker`), guardado junto do nome do módulo.
`JitSession::remove_module(i)` chama `LLVMOrcResourceTrackerRemove`, que
descarrega o código daquele módulo.

> **Remover só é seguro quando nenhuma função daquele módulo está executando**,
> em nenhuma thread e em nenhum quadro de pilha abaixo do chamador. O LLVM não
> verifica isso: o que era código vira memória liberada, e chamar um endereço
> resolvido antes da remoção é uso de memória liberada.

A assinatura carrega a parte da garantia que Rust consegue dar: `remove_module`
exige `&mut self` e `run_entry` toma `&self`, então uma execução em curso e uma
remoção não coexistem *nesta* sessão. O que o compilador não impede é a remoção
feita de dentro de uma chamada ao código gerado — por exemplo, a partir de uma
função de runtime invocada pelo programa. É justamente essa garantia manual que
o hot reload eliminará, usando stubs indiretos.

Os handles do heap **não** são liberados pela remoção: o heap pertence à thread,
não ao módulo, e sobrevive ao descarregamento.

A ordem de destruição também é contrato: `JitSession` declara o vetor de módulos
**antes** do campo da `LLJIT`, porque os campos são destruídos na ordem de
declaração e liberar um `ResourceTracker` depois de destruir a `LLJIT` que o
criou seria uso de memória liberada.

## Medição por fase

No mesmo espírito de `LinkStats` e de [DESEMPENHO.md](DESEMPENHO.md): cada
intervalo é cronometrado no próprio trecho, nunca por subtração, e `total` é pelo
menos a soma das fases.

| Campo Rust | Campo JSON de `dartforge run --timings` | Intervalo medido |
| --- | --- | --- |
| — | `frontend_ns` | Carga do grafo, análise e emissão do IR |
| `JitReport::session` | `session_ns` | `LLVMOrcCreateLLJIT` e publicação dos 18 símbolos de runtime |
| `ModuleReport::parse_ir` | `parse_ir_ns` | `LLVMParseIRInContext2` e construção do `ThreadSafeModule` |
| `ModuleReport::add_module` | `add_module_ns` | `LLVMOrcLLJITAddLLVMIRModuleWithRT` |
| `EntryReport::lookup` | `lookup_ns` | `LLVMOrcLLJITLookup`, **incluindo a geração de código sob demanda** |
| `EntryReport::execute` | `execute_ns` | Execução de `@dartforge_entry`, do `call` ao retorno |
| `JitReport::total` | `jit_total_ns` | Sessão inteira, incluindo sua destruição |
| `ModuleReport::ir_bytes` | `ir_bytes` | Bytes de IR textual analisados |

A linha que mais engana é `lookup_ns`. No ORCv2 um módulo só é compilado quando
algum de seus símbolos é procurado: o primeiro `lookup` de uma sessão carrega
toda a geração de código, e `execute_ns` mede apenas a execução do que já está
materializado. Comparar `execute_ns` do JIT com o tempo de processo do AOT seria
comparar coisas diferentes — o número comparável com o AOT é a soma
`session + parse_ir + add_module + lookup + execute`.

Tempos incluem escalonamento do sistema; não são tempos exclusivos de CPU.

## Por que ORCv2 antes de Cranelift

A pergunta não é qual gera código melhor, e sim qual chega antes a um perfil de
desenvolvimento **correto**.

1. **Zero divergência de backend.** O JIT consome o IR já emitido por
   `crates/llvm`, byte a byte o mesmo que o AOT. Um backend Cranelift exigiria um
   segundo emissor, a partir da HIR, e cada recurso da linguagem precisaria ser
   implementado e conferido duas vezes. O teste diferencial deste crate só é
   barato porque não há segundo emissor: ele compara execuções, não compiladores.
2. **O runtime já existe para esta ABI.** As 18 funções, o layout de handles e o
   protocolo de raízes foram desenhados contra as declarações de `crates/llvm`.
   Reaproveitá-los custou um módulo de adaptação; refazê-los para outra ABI não.
3. **Hot reload tem caminho pronto.** A próxima etapa precisa de stubs indiretos
   e redirecionamento de chamadas. O ORCv2 traz isso na API C
   (`LLVMOrcCreateLocalIndirectStubsManager`,
   `LLVMOrcCreateLocalLazyCallThroughManager`, `LLVMOrcLazyReexports`) e os
   `ResourceTracker` já dão o descarregamento por módulo.
4. **O custo da dependência já estava pago.** O projeto já exige LLVM para o
   perfil de produção.

Cranelift continua interessante pelo tempo de compilação e por dispensar uma
instalação do LLVM, que são os argumentos reais a favor dele; `crates/cranelift-jit`
explora esse caminho em paralelo, sobre uma fatia escalar da HIR. As duas coisas
não competem nesta etapa: trocar de backend só faz sentido depois que existir uma
linha de base medida e um diferencial verde para comparar contra o AOT — e é
exatamente isso que este crate entrega.

## Requisito de build e de execução

`crates/jit` usa `llvm-sys` 221.1.0 para as assinaturas da API C e para os
invólucros de `LLVMInitializeNative*`. Ele localiza o LLVM pelo `llvm-config` da
**distribuição completa** — a que traz `bin/llvm-config.exe`, `include/llvm-c/**`
e as bibliotecas. O instalador reduzido de Windows (`LLVM-*-win64.exe`), que só
tem `LLVM-C.dll`/`LLVM-C.lib` e os cabeçalhos `Remarks.h`/`lto.h`, **não serve**.

O repositório já traz `.cargo/config.toml` com

```toml
[env]
LLVM_SYS_221_PREFIX = "D:/DartSDKs/llvm/clang+llvm-22.1.8-x86_64-pc-windows-msvc"
```

de modo que qualquer `cargo` executado dentro dele enxerga a variável. Uma
definição no ambiente do shell tem precedência, para testar outro prefixo.
`DARTFORGE_LLVM_DIR`, o nome preferido do projeto para apontar um prefixo LLVM,
também é aceito pelo `build.rs` deste crate — mas quem procura o `llvm-config` é
o `build.rs` do `llvm-sys`, que só conhece `LLVM_SYS_221_PREFIX`; mantenha as
duas com o mesmo valor.

Sem um prefixo válido a build do workspace inteiro falha, porque `crates/jit` é
membro de `crates/*`. Essa é a consequência de o perfil de desenvolvimento ser
parte do produto, não um extra.

### Por que a ligação é dinâmica, e o que isso custa

A feature `no-llvm-linking` do `llvm-sys` está ligada: as diretivas de ligação
saem do `build.rs` deste crate, que liga `LLVM-C` — a **biblioteca
compartilhada** da API C — em vez das bibliotecas estáticas.

O motivo é concreto, não preferência. As bibliotecas estáticas do pacote oficial
`clang+llvm-22.1.8-x86_64-pc-windows-msvc` são compiladas com a CRT **estática**
(`libcmt`); o Rust usa a CRT dinâmica (`msvcrt`). Ligar as duas produz

```
LINK : warning LNK4098: defaultlib 'libcmt.lib' conflita com uso de outras
bibliotecas; use /NODEFAULTLIB:library
```

e um binário com **dois heaps**. O efeito observado, reproduzível: um
`LLVMParseIRInContext2` que falha devolve a mensagem de erro, ela é lida
corretamente, e o `LLVMDisposeMessage` seguinte derruba o processo com
`STATUS_ACCESS_VIOLATION` — alocada por uma CRT, liberada pela outra. O mesmo
pacote também traz `llvm-config --system-libs` pedindo um `xml2s.lib` que não
está no pacote, e não aceita `llvm-config --link-shared`, que procura um
`LLVM-22.dll` inexistente. Com `LLVM-C.dll`, alocação e liberação acontecem as
duas dentro da DLL, com a CRT dela, e nenhum dos dois problemas aparece.

O preço é uma dependência de execução: **`LLVM-C.dll` precisa estar alcançável
pelo carregador** — no `PATH`, em Windows — tanto para `dartforge run` quanto
para os testes. `scripts/env.ps1` acrescenta `<prefixo>/bin` ao `PATH` quando
encontra a DLL lá. Sem ela o executável nem chega a iniciar; a falha é do
carregador do sistema operacional, não do programa:

```
dartforge.exe: error while loading shared libraries: LLVM-C.dll:
cannot open shared object file: No such file or directory
```

Por isso os testes que abrem uma `LLJIT` são marcados

```rust
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
```

seguindo o padrão do repositório para testes que dependem de um toolchain
externo, como os de `crates/native`. Os testes unitários da biblioteca — formato
das mensagens de erro, tabela de símbolos, captura de saída do runtime — não
tocam no LLVM e continuam rodando sempre. A verificação canônica do projeto
(`cargo test --locked --workspace -- --include-ignored`, em CONTRIBUTING.md) já
executa os ignorados, então nada fica de fora do que o projeto considera verde.

`jit_and_aot_agree_on_the_same_ir` acumula os dois requisitos: precisa da DLL e
de Clang/`rustc` para construir o lado AOT da comparação.

### Integração contínua

O workflow atual instala apenas o Clang do ambiente e **não** provisiona a
distribuição completa do LLVM nem define `LLVM_SYS_221_PREFIX`. Enquanto isso não
for ajustado, `cargo build --workspace` falhará na CI por falta de `llvm-config`.

## Referências oficiais consultadas

- [LLVM ORCv2 design and implementation](https://llvm.org/docs/ORCv2.html):
  `JITDylib` como unidade de nomes, `ResourceTracker` para descarregamento e o
  papel das unidades de materialização.
- [LLVM C API — Orc](https://llvm.org/doxygen/group__LLVMCExecutionEngineORC.html)
  e LLJIT: propriedade dos argumentos em `LLVMOrcCreateNewThreadSafeModule`,
  `LLVMOrcLLJITAddLLVMIRModuleWithRT` e `LLVMOrcAbsoluteSymbols`; consumo do
  `LLVMErrorRef` por `LLVMGetErrorMessage`.
- [`llvm-sys` 221.1.0](https://docs.rs/llvm-sys/221.1.0/): assinaturas usadas e a
  convenção `LLVM_SYS_<versão>_PREFIX`.
- `crates/runtime/CONTRACT.md` e [AOT-DRIVER.md](AOT-DRIVER.md): a ABI que este
  crate precisa reproduzir e a fronteira `unsafe` do perfil de produção.
