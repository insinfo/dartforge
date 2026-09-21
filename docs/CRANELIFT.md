# Cranelift JIT — experimento medido, não adoção

Este documento registra um **experimento**: implementar o perfil de
desenvolvimento (JIT) do DartForge com **Cranelift**, em Rust puro, e medir o
resultado contra o caminho LLVM que já existe no repositório. A decisão de qual
backend adotar é por medição, e os números estão aqui. A recomendação está no
fim, e ela não é "adotar".

O crate é `crates/cranelift-jit` (`dartforge-cranelift-jit`). Ele **não traduz
LLVM IR textual**: parte de `dartforge_hir::Module`, a mesma estrutura que
`dartforge-llvm` consome. Uma tradução a mais só acrescentaria custo e perderia
informação que já existe antes.

## A fatia coberta

Funções de topo com parâmetros e retorno escalares e o corpo de `main`:

| Recurso | Observação |
| --- | --- |
| `int` | i64 com estouro modular, igual ao backend LLVM e à VM do Dart |
| `bool` | um byte com 0 ou 1, que é o que `icmp` do Cranelift produz |
| `void` | só como retorno de função |
| variáveis locais | escopo léxico com sombreamento, via variáveis SSA do `cranelift-frontend` |
| `+`, `-`, `*`, `-` unário | `iadd`/`isub`/`imul`/`ineg`, modulares em complemento de dois |
| `==`, `!=`, `<`, `<=`, `>`, `>=` | `icmp` com sinal |
| `&&`, `\|\|`, `!` | curto-circuito por parâmetro de bloco (o `phi` do Cranelift) |
| `if` / `else` | bloco de junção criado só quando é alcançável |
| `while`, `do`/`while`, `for` | `continue` salta para a atualização, como no backend LLVM |
| `break`, `continue`, `return` | `break` sai do laço mais próximo |
| chamadas entre funções do programa | inclusive recursão simples, de árvore e mútua |
| corpo arrow (`=>`) | tratado como `return` pelo frontend |
| `print` de `int` e de `bool` | pelos símbolos do runtime, registrados por endereço |

A ordem de avaliação é a do Dart: operando esquerdo antes do direito, argumentos
na ordem escrita, e o direito de `&&`/`||` só quando necessário.

### O runtime

O JIT não liga nada. Ele registra no `JITBuilder` os nomes
`dartforge_print_i64` e `dartforge_print_bool` apontando para funções Rust do
próprio crate (`src/runtime.rs`), que reproduzem exatamente o formato do harness
AOT de `dartforge-runtime` (`println!("{valor}")` e `true`/`false`). É por isso
que a comparação byte a byte com o executável AOT fecha.

`crates/runtime` não foi tocado: o harness `runtime_main.rs` é uma **string de
código-fonte** compilada pelo driver nativo, não um módulo do crate, então não
há como referenciar aquelas funções por endereço. Duplicar duas funções de
impressão de três linhas foi preferível a pedir mudança num crate com trabalho
concorrente.

## Os limites, com a mensagem exata

Nada fora da fatia é aceito em silêncio, nem em código morto: o corpo inteiro é
percorrido, inclusive depois de um `return`. As mensagens têm o prefixo
`Cranelift JIT ainda não suporta `, espelhando o `LLVM AOT ainda não suporta `
do backend nativo. O prefixo difere **de propósito**: um mesmo programa pode ser
aceito por um backend e recusado pelo outro, e a mensagem precisa dizer qual dos
dois recusou.

| Forma | Mensagem completa | Span |
| --- | --- | --- |
| `double x = 1.5;` | `Cranelift JIT ainda não suporta double e num` | a declaração inteira |
| `String s = 'a';` | `Cranelift JIT ainda não suporta strings` | a declaração inteira |
| `'a' + 'b'` | `Cranelift JIT ainda não suporta strings` | o literal |
| `int? x = 1;` | `Cranelift JIT ainda não suporta tipos anuláveis` | a declaração inteira |
| `7 ~/ 2`, `7 / 2` | ``Cranelift JIT ainda não suporta divisão double e truncada (`/`, `~/`)`` | a expressão binária |
| `7 % 2` | `Cranelift JIT ainda não suporta módulo euclidiano` | a expressão binária |
| `a & b`, `a \| b`, `a ^ b`, `a << b`, `a >> b`, `a >>> b` | `Cranelift JIT ainda não suporta operadores bit a bit e deslocamentos` | a expressão binária |
| `~a` | `Cranelift JIT ainda não suporta operadores bit a bit` | a expressão unária |
| `a ?? b` | ``Cranelift JIT ainda não suporta o operador `??` `` | a expressão binária |
| `a!` | ``Cranelift JIT ainda não suporta o operador `!` de não nulo`` | a expressão unária |
| `c ? a : b` | `Cranelift JIT ainda não suporta o operador condicional` | a expressão inteira |
| `[1, 2]`, mapas, indexação | `Cranelift JIT ainda não suporta coleções` | o literal |
| closures, `f()(x)` | `Cranelift JIT ainda não suporta closures` | a expressão |
| `switch` (instrução e expressão) | `Cranelift JIT ainda não suporta switch` | a instrução/expressão inteira |
| `try`/`catch`/`finally`, `rethrow` | `Cranelift JIT ainda não suporta try, catch, finally e rethrow` | a instrução inteira |
| `assert(...)` | `Cranelift JIT ainda não suporta assert` | a instrução inteira |
| `for (final x in ...)` | `Cranelift JIT ainda não suporta for-in` | a instrução inteira |
| rótulos de laço | `Cranelift JIT ainda não suporta rótulos de laço` | a instrução inteira |
| `class`, `enum`, membros estáticos | `Cranelift JIT ainda não suporta classes, enums e membros estáticos` | a declaração inteira |
| variável de topo | `Cranelift JIT ainda não suporta variáveis de topo` | a declaração inteira |
| `[int a = 1]`, `{int a = 1}` | `Cranelift JIT ainda não suporta parâmetros opcionais ou nomeados` | o parâmetro |
| `async`/`await` | `Cranelift JIT ainda não suporta async/await` | a função |
| `T f<T>(T x)` | `Cranelift JIT ainda não suporta funções genéricas` | a função |
| `@Native` | `Cranelift JIT ainda não suporta @Native e ligação estática a símbolos C` | a anotação |
| records, `throw`, cascatas, `is`/`as`, interpolação, atalhos `.nome` | mensagem própria de cada forma | a expressão |

Cada linha desta tabela é conferida por
`crates/cranelift-jit/tests/execucao.rs::cada_forma_fora_da_fatia_tem_mensagem_e_span_exatos`,
que compara a mensagem literalmente e confere o span **recortando a fonte** com
ele: o teste só passa se o diagnóstico apontar para o texto certo.

A fatia do Cranelift é **estritamente menor** que a do LLVM AOT. Isso é uma
escolha do experimento, não um descuido: o objetivo era medir, e medir uma fatia
honesta e completamente testada vale mais do que cobrir mais formas pela metade.

## O contrato central: acordo com o backend LLVM

Trocar o backend do perfil de desenvolvimento só faz sentido se os dois
caminhos derem o mesmo resultado. É isso que os testes fixam:

1. `a_saida_do_jit_bate_com_a_do_executavel_aot` (`#[ignore]`, exige Clang e
   rustc) compila cada programa do corpus pelos **dois** caminhos — JIT
   Cranelift em memória e LLVM IR → Clang → objeto → executável ligado ao
   runtime Rust — e compara a saída **byte a byte**, além de conferir os dois
   contra a VM do Dart 3.6.2.
2. `o_jit_e_o_backend_llvm_aceitam_o_mesmo_corpus` roda sempre e garante que
   nenhum programa do corpus foi aceito só por um dos dois.
3. `jit_e_llvm_recusam_o_mesmo_ponto_do_programa` confere que, nas formas que os
   dois recusam, o **span é idêntico** e cada mensagem traz o prefixo do seu
   backend.

O corpus vem da VM do Dart 3.6.2 como oráculo, inclusive o estouro modular:
`2^63` imprime `-9223372036854775808`, `2^63 - 1` imprime
`9223372036854775807` e `2^63 * 2^63` imprime `0`, nos três caminhos.

## Metodologia das medições

Seguindo [DESEMPENHO.md](DESEMPENHO.md): cada intervalo é cronometrado **no
próprio trecho**, nunca por subtração; há aquecimento antes das amostras;
reporta-se mediana e p95; e há sempre um contador de trabalho que **não depende
da carga da máquina**, porque a mediana sozinha esconde tanto ruído quanto
regressão.

Os contadores independentes de carga deste experimento são dois:

* **instruções CLIF emitidas** — quanto trabalho a tradução produziu;
* **bytes de código de máquina** — quanto trabalho a geração produziu.

Os dois são determinísticos para um mesmo programa e uma mesma versão do
Cranelift, e são o que se deve olhar primeiro quando o tempo parecer estranho.

> **Esta máquina estava ruidosa.** As medições foram feitas com outros agentes
> compilando o mesmo workspace em paralelo (inclusive `cargo build` do LLVM e
> do próprio Cranelift). Os tempos de parede abaixo, sobretudo os de build, são
> **limites superiores**, não números limpos. É por isso que os contadores de
> trabalho estão em toda tabela: eles não se movem com a carga.

Reprodução:

```
cargo run --release --example experimento -p dartforge-cranelift-jit
```

com `DARTFORGE_CLANG` apontando para um `clang` (o eixo 2 precisa dele para
construir o lado AOT). Perfil `release` padrão do workspace.

<!-- EIXO12 -->

## Eixo 3 — custo de construir o próprio compilador

Este é o eixo em que os dois caminhos mais divergem, e o único em que a
diferença é de ordem de grandeza.

### O que o Cranelift custa

| Medida | Valor |
| --- | --- |
| dependências externas novas no grafo | **44 crates** (15 do próprio Cranelift, o resto transitivo: `regalloc2`, `gimli`, `wasmtime-internal-core`, `memmap2`, `region`, `target-lexicon`, …) |
| download externo além do `crates.io` | **nenhum** |
| build limpo do subgrafo, perfil `dev`, `-j8` | **3 min 06 s** (188 s) |
| build limpo do subgrafo, perfil `release`, `-j8` | **5 min 21 s** (323 s) |
| `target/debug` do subgrafo isolado | **440 MB** |
| `target/release` do subgrafo isolado | **243 MB** |
| rlibs do Cranelift em `dev` | **231 MB**, dos quais **171 MB** só de `libcranelift_codegen` |
| binário `release` que usa o JIT | **4,11 MB** |

O build limpo foi medido num crate isolado com só as quatro dependências do
Cranelift (`cranelift-codegen`, `-frontend`, `-module`, `-jit`) e um `main`
trivial, num `CARGO_TARGET_DIR` próprio, para separar o custo do Cranelift do
resto do workspace. Medir `cargo build --workspace` inteiro agora daria ruído:
há outros agentes alterando `crates/semantic`, `crates/llvm` e `crates/codegen`
ao mesmo tempo, e o workspace esteve várias vezes sem compilar durante esta
sessão. O subgrafo isolado é reprodutível; o total do workspace, hoje, não é.

`cranelift-codegen` é uma única unidade de compilação enorme, com geração de
código por `build.rs` (ISLE). Ela fica no caminho crítico de qualquer build
limpo e não paraleliza: os 3 minutos não encolhem com mais núcleos.

### O que o ORCv2 custa

| Medida | Valor |
| --- | --- |
| download externo | **`llvm-22.1.8-msvc.tar.xz`, 862.053.924 bytes (822 MiB)** |
| distribuição extraída em disco | **3.947 MB** (`bin` 3.282 MB, `lib` 588 MB, `include` 77 MB) |
| dependências Rust novas | `llvm-sys` 221.1.0 (poucos crates; o peso está fora do Cargo) |
| configuração exigida | `LLVM_SYS_221_PREFIX` com caminho **absoluto**, fixado em `.cargo/config.toml` |
| dependência de execução | `LLVM-C.dll` alcançável pelo carregador (`scripts/env.ps1` ajusta o `PATH`) |

Três consequências medidas, não hipotéticas:

1. **O instalador oficial reduzido não serve.** Conferi a instalação
   `D:/LLVM/22.1.8` (2.898 MB extraídos): ela traz `LLVM-C.dll` (70,9 MB) e
   `LLVM-C.lib`, mas o diretório `include/llvm-c/` tem só `Remarks.h` e
   `lto.h` — **nenhum cabeçalho de ORC** — e não há `llvm-config.exe`. Por isso
   foi preciso baixar a distribuição completa `clang+llvm-...-windows-msvc`, de
   822 MiB comprimidos.
2. **Sem o prefixo, o workspace inteiro para de compilar**, porque `crates/jit`
   é membro de `crates/*`. Um colaborador sem LLVM não constrói nem o backend
   JavaScript. O Cranelift não tem esse efeito: `cargo build` basta.
3. **O caminho absoluto de uma máquina foi versionado** em `.cargo/config.toml`.
   É a solução prática, mas é dívida: a CI atual não provisiona a distribuição
   e, segundo [JIT.md](JIT.md), `cargo build --workspace` falha lá hoje.

### O confronto

| | Cranelift | LLVM ORCv2 |
| --- | --- | --- |
| bytes baixados fora do `crates.io` | 0 | 822 MiB |
| bytes em disco para poder compilar | 440 MB de `target` | 3.947 MB de distribuição + `target` |
| passos para um clone novo compilar | `cargo build` | baixar 822 MiB, extrair 3,9 GB, apontar `LLVM_SYS_221_PREFIX`, pôr `LLVM-C.dll` no `PATH` |
| custo no caminho crítico do build | ~3 min de `cranelift-codegen`, uma vez | ~0 (o LLVM já está compilado) |
| tamanho acrescentado ao binário | 4,11 MB estáticos | ~0 estático, mas uma DLL de 70,9 MB em execução |

O Cranelift troca **peso de instalação** por **tempo de build**. O ORCv2 troca o
contrário. Qual dos dois dói mais depende de quantas vezes se faz um build limpo
e de quantas máquinas precisam do compilador — não há resposta universal, e é
por isso que os dois números estão aqui em vez de uma opinião.



## Eixo 4 — o que cada backend oferece para hot reload

O levantamento abaixo foi feito lendo o **código-fonte dos crates baixados**
(em `~/.cargo/registry/src`), bissectando versões do `cranelift-jit`, e
conferindo contra a documentação e o histórico oficiais. Cada afirmação tem ou
uma citação ou um caminho de arquivo.

### Cranelift: existiu, foi removido, e nunca liberou memória

`cranelift-jit` **teve** redefinição de função. A bissecção por versão, feita
baixando cada `.crate` e procurando os símbolos em `src/backend.rs`:

| versão | `hotswap` / `prepare_for_function_redefine` |
| --- | --- |
| até **0.118.0** (Wasmtime 31.0.0) | **presentes** |
| **0.119.0** (Wasmtime 32.0.0) em diante, inclusive 0.135.2 e 0.136.0-rc.1 | **ausentes** |

A API era esta, com toda a documentação que ela tinha:

> "Enable or disable hotswap support. See [`JITModule::prepare_for_function_redefine`]
> for more information.
>
> Enabling hotswap support requires PIC code."
> — [`JITBuilder::hotswap`, 0.118.0](https://docs.rs/cranelift-jit/0.118.0/cranelift_jit/struct.JITBuilder.html#method.hotswap)

> "Allow a single future `define_function` on a previously defined function.
> This allows for hot code swapping and lazy compilation of functions.
>
> This requires hotswap support to be enabled first using [`JITBuilder::hotswap`]."
> — [`JITModule::prepare_for_function_redefine`, 0.118.0](https://docs.rs/cranelift-jit/0.118.0/cranelift_jit/struct.JITModule.html#method.prepare_for_function_redefine)

**Sim, redirecionava chamadas**, e por indireção: com hotswap ligado,
`get_address` devolvia o endereço de uma entrada de **PLT** em vez do corpo da
função, e `define_function` forçava `colocated = false` em todas as referências
externas para que nenhuma chamada virasse um `call` pc-relativo direto. A
entrada de PLT é um `jmp *got_ptr`, e a GOT é um `AtomicPtr<u8>` escrito com
`Ordering::SeqCst`. Redefinir era escrever o ponteiro novo na GOT; toda chamada
**futura** passava a ir para o código novo. Frames já na pilha continuavam no
código antigo — isso decorre do mecanismo, mas **não está escrito em nenhum
ponto da documentação**.

Duas ressalvas que importam mais do que a API:

1. **A memória antiga nunca era liberada.** O corpo de
   `prepare_for_function_redefine` na 0.118.0 termina assim:

   ```rust
   self.compiled_functions[func_id] = None;

   // FIXME return some kind of handle that allows for deallocating the function

   Ok(())
   ```

   O blob antigo era esquecido, e vazava.
2. **O PLT só existia em x86_64.** `write_plt_entry_bytes` continha
   `assert!(cfg!(target_arch = "x86_64"), "PLT is currently only supported on x86_64")`.

A remoção foi o PR
[bytecodealliance/wasmtime#10345, "Remove hotswapping support from
cranelift-jit"](https://github.com/bytecodealliance/wasmtime/pull/10345)
(autor `bjorn3`, merge em 2025-03-06, commit `27de1e0b9`), cujo corpo é a melhor
descrição do estado da arte:

> "It was originally introduced for cg_clif. cg_clif recently removed it's use
> of hotswapping as the way it is implemented in cranelift-jit has various
> issues like leaking memory, panicking when the memory allocator decided to put
> two functions more than 2GB away from each other and only supporting x86_64.
> **Better hotswapping support will likely require a fundamentally different
> implementation.**"

A montante, o `rustc_codegen_cranelift` havia removido o modo lazy-JIT um dia
antes ([commit `5d03df9`, 2025-03-05](https://github.com/rust-lang/rustc_codegen_cranelift/commit/5d03df9431833ad992c1507fedc7daee3e232443)):

> "I might re-implement it in the future, but would probably do so by replacing
> cranelift-jit. **cranelift-jit's api doesn't quite work well for lazy jitting.**"

Não há entrada de changelog para a remoção: o `RELEASES.md` do Wasmtime não
cobre mudanças de API dos crates Cranelift avulsos, e esses crates não mantêm
changelog próprio. O PR é a única fonte.

### O que a 0.135.2 permite hoje

**Redefinir é impossível.** `define_function`, `define_function_bytes` e
`define_data` rejeitam com `ModuleError::DuplicateDefinition` quando o `FuncId`
já tem código. Não há escape.

A única desalocação é por módulo inteiro:

> "Free memory allocated for code and data segments of compiled functions.
>
> # Safety
>
> Because this function invalidates any pointers retrieved from the
> corresponding module, it should only be used when none of the functions from
> that module are currently executing and none of the `fn` pointers are called
> afterwards."
> — [`JITModule::free_memory`](https://docs.rs/cranelift-jit/0.135.2/cranelift_jit/struct.JITModule.html#method.free_memory)

A assinatura é `pub unsafe fn free_memory(mut self)`: **consome o módulo**. E a
documentação de `get_finalized_function` admite a lacuna em voz alta:

> "The pointer remains valid until either [`JITModule::free_memory`] is called
> or **in the future some way of deallocating this individual function is
> used**."

Os provedores de memória confirmam: o trait
[`JITMemoryProvider`](https://docs.rs/cranelift-jit/0.135.2/cranelift_jit/trait.JITMemoryProvider.html)
tem `allocate`, `finalize` e `free_memory` — e **nenhum `deallocate(handle)`**.
Tanto o `SystemMemoryProvider` quanto o `ArenaMemoryProvider` documentam:

> "Note: Memory will be leaked by default unless [`JITMemoryProvider::free_memory`]
> is called to ensure function pointers remain valid for the remainder of the
> program's life."

A issue que pede desalocação por função,
[wasmtime#1157](https://github.com/bytecodealliance/wasmtime/issues/1157), está
**aberta desde 2019**; o comentário mais recente de `bjorn3` (2026-08-13) diz
"This hasn't been implemented yet."

Consequência prática, e é o que este crate faz: **uma versão por `JITModule`**.
O módulo antigo permanece válido — nada é redirecionado — e sua memória só é
devolvida quando o `ProgramaCompilado` correspondente é destruído. O teste
`recompilar_cria_modulo_novo_e_mantem_o_antigo_valido` fixa exatamente esse
comportamento, para que uma mudança de versão do Cranelift que o altere apareça
como teste vermelho e não como surpresa em produção.

### ORCv2: a peça que falta no Cranelift existe

O ORCv2 lista remoção de código entre suas funcionalidades de topo:

> "*ResourceTrackers* allow you to remove code."
> — [LLVM ORCv2 design and implementation](https://llvm.org/docs/ORCv2.html)

E documenta o procedimento:

> "To remove an individual module from a JITDylib it must first be added using
> an explicit `ResourceTracker`. The module can then be removed by calling
> `ResourceTracker::remove`."
> — [ORCv2, "How to remove code"](https://llvm.org/docs/ORCv2.html#how-to-remove-code)

Para o redirecionamento de chamadas, o ORCv2 tem *lazy reexports*:

> "a lazy reexport ... only trigger[s] materialization of a function stub. This
> function stub is initialized to point at a *lazy call-through*, which provides
> reentry into the JIT. If the stub is called at runtime then the lazy
> call-through will look up the reexported symbol ..., **update the stub** (to
> call directly to the reexported symbol on subsequent calls), and then return
> via the reexported symbol."
> — [ORCv2, "Laziness"](https://llvm.org/docs/ORCv2.html#laziness)

Abaixo disso, o `ObjectLinkingLayer` usa o JITLink, cujo `JITLinkMemoryManager`
tem `deallocate(std::vector<FinalizedAlloc>, ...)` — desalocação por **alocação
finalizada**, identificada por um handle opaco
([JITLink.html](https://llvm.org/docs/JITLink.html)).

**É exatamente essa peça que falta no `cranelift-jit`.** O `JITMemoryProvider`
não tem análogo de `FinalizedAlloc`: por isso `ResourceTracker::remove` devolve
memória por módulo enquanto o Cranelift só devolve por processo.

Confirmei localmente que os símbolos existem na distribuição instalada:
`LLVM-C.dll` de 22.1.8 exporta `LLVMOrcResourceTrackerRemove`,
`LLVMOrcResourceTrackerTransferTo` e a família `LLVMOrcLLJIT*`, inclusive
`LLVMOrcLLJITAddLLVMIRModuleWithRT`. E [JIT.md](JIT.md) registra que
`crates/jit` já cria um `ResourceTracker` por módulo.

Uma ressalva de honestidade: o ORCv2 também **não tem um `redefine()`**. O padrão
é `RT_antigo->remove()` seguido de um `addIRModule` com um tracker novo. A
diferença para o Cranelift não é a existência de um verbo "redefinir", e sim a
existência de (a) descarte com devolução de memória e (b) indireção pronta para
redirecionar chamadas.

### O placar do eixo 4

| Capacidade | Cranelift 0.118 (removido) | Cranelift 0.135.2 (atual) | LLVM ORCv2 |
| --- | --- | --- | --- |
| redefinir uma função já compilada | sim, `prepare_for_function_redefine` | **não**, `DuplicateDefinition` | sim, `RT.remove()` + novo `add` |
| redirecionar chamadas existentes | sim, GOT/PLT (só x86_64) | **não** (o front-end teria de montar a indireção) | sim, lazy reexports e stubs |
| liberar o código antigo | **nunca** (`// FIXME`) | só `unsafe free_memory(self)`, módulo inteiro | `ResourceTracker::remove` → `JITLinkMemoryManager::deallocate` |
| granularidade de descarte | — | módulo | módulo, `JITDylib` ou símbolo |
| portabilidade | só x86_64 | todas | todas |

Para hot reload, **o ORCv2 ganha sem margem de dúvida**, e não por pouco: o
Cranelift não só não tem a funcionalidade como a removeu depois de concluir que
a implementação existente era defeituosa e que refazê-la exige "uma implementação
fundamentalmente diferente".


## Recomendação

### O que o experimento mostrou

1. **Geração de código.** O Cranelift compila a fatia inteira em **décimos de
   milissegundo**, barato o bastante para recompilar a cada tecla. Mas a
   comparação justa não é com o AOT inteiro: é com a emissão de LLVM IR textual,
   que custa ainda menos — porque ela não gera código de máquina, só texto. O
   custo real do caminho ORCv2 está no `lookup` do ORC, que materializa o módulo,
   e esse número pertence a `crates/jit`, não a este experimento.
2. **Execução.** Com `opt_level` padrão (`none`), o código do Cranelift fica na
   mesma ordem de grandeza do de `clang -O0`; com `opt_level = "speed"` ele
   melhora, e continua muito atrás de `clang -O2`. Para um perfil de
   **desenvolvimento** isso é secundário: o que importa é o tempo até rodar.
3. **Custo de build.** Aqui o Cranelift ganha de forma decisiva. Zero bytes
   baixados fora do `crates.io` contra 822 MiB comprimidos e 3,9 GB extraídos;
   `cargo build` contra um caminho absoluto versionado em `.cargo/config.toml`
   mais uma DLL que precisa estar no `PATH`. E o workspace inteiro para de
   compilar sem o LLVM, porque `crates/jit` é membro de `crates/*`.
4. **Hot reload.** Aqui o Cranelift perde de forma decisiva, e não é questão de
   esforço de implementação nosso. A funcionalidade existiu, era defeituosa
   (vazava memória, só x86_64, quebrava com alocações a mais de 2 GB de
   distância) e **foi removida** do upstream. O ORCv2 tem `ResourceTracker` e
   lazy reexports, e `crates/jit` já usa o primeiro.

### A recomendação: não adotar agora

**Não adotar o Cranelift como backend do perfil de desenvolvimento nesta etapa.**
Três razões, em ordem de peso:

1. **O objetivo declarado do perfil de desenvolvimento inclui hot reload**, e o
   Cranelift hoje não o suporta — não por imaturidade do nosso código, mas por
   decisão do projeto upstream, documentada no PR que removeu a funcionalidade.
   Montar a indireção no nosso próprio front-end é possível, mas seria
   reimplementar dentro do DartForge justamente aquilo que o upstream concluiu
   precisar de "uma implementação fundamentalmente diferente".
2. **Um segundo emissor custa duas vezes, para sempre.** Este crate cobre uma
   fatia escalar; o backend LLVM cobre strings, classes, herança, despacho
   virtual, enums, `switch` com padrões, null safety e `@Native`. Cada recurso
   novo da linguagem teria de ser implementado e conferido nos dois. O caminho
   ORCv2 não tem esse custo porque consome o IR já emitido: `crates/jit` não tem
   emissor próprio.
3. **A vantagem real do Cranelift — dispensar o LLVM — já foi parcialmente
   gasta.** A distribuição completa já foi baixada, `LLVM_SYS_221_PREFIX` já está
   versionado e `crates/jit` já existe. O argumento "o colaborador não precisa
   instalar LLVM" perdeu força no instante em que o perfil de produção passou a
   exigir a mesma instalação.

### O que fazer com este crate

**Manter, sem ligá-lo ao driver.** Ele custa ~3 minutos de build limpo e paga
isso de três formas:

* **É um oráculo independente.** O teste diferencial contra o AOT pega erro de
  emissão que um backend sozinho não pega: dois emissores escritos por caminhos
  diferentes, concordando byte a byte com a VM do Dart, é evidência que um
  emissor sozinho não produz.
* **É a linha de base para reabrir a decisão.** Se
  [wasmtime#1157](https://github.com/bytecodealliance/wasmtime/issues/1157) for
  implementado, ou se aparecer uma API de redefinição nova, a comparação já está
  montada e medida: basta rodar
  `cargo run --release --example experimento -p dartforge-cranelift-jit`.
* **É o plano B para máquinas sem LLVM.** A CI hoje não provisiona a distribuição
  completa e, segundo [JIT.md](JIT.md), `cargo build --workspace` falha lá.
  Enquanto isso não for resolvido, existe um caminho de execução nativa que não
  depende de nada externo.

### O que reabriria a decisão

* Desalocação por função no `cranelift-jit` (wasmtime#1157) **e** alguma forma de
  redirecionamento de chamadas.
* Medição do `lookup` do ORCv2 mostrando que a geração de código do LLVM em
  memória é lenta o bastante para atrapalhar o ciclo de edição. Esse número não
  existe hoje; ele é de `crates/jit`.
* Exigência de rodar o compilador onde instalar 3,9 GB de LLVM não é viável.

Nenhuma das três é verdade hoje. Por isso: **experimento concluído, resultado
registrado, backend não adotado.**

