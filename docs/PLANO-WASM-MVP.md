# Plano de implementação: DartForge → LLVM → WebAssembly MVP

Data: 20/10/2026. **Estado (21/10/2026): W0 concluído; W1 em diante não
iniciados.** O W0 prova o perfil binário e a ABI de host com um módulo escalar
(LLVM IR + fragmento Rust com `std`), não Dart→Wasm: este documento ainda não
registra execução Dart→Wasm bem-sucedida.

## 1. Objetivo e contrato de entrega

Adicionar ao DartForge um compilador AOT de Dart para WebAssembly que compila **qualquer programa Dart
que o `dart compile wasm` oficial compila**, com o mesmo comportamento, gerando
WebAssembly MVP em memória linear, como fazem Rust e C++: coletor de lixo,
alocador, exceções e RTI próprios dentro da memória linear, sem WasmGC. O mesmo
arquivo `.wasm` executa no navegador e em um executor independente que implemente
a ABI de host do DartForge. Referência semântica: Dart 3.6.2.

“MVP”, nos nomes destes documentos, quer dizer **WebAssembly de memória linear
sem WasmGC**, do jeito que Rust e C++ compilam, e não um produto incompleto nem o
conjunto de 2017. O perfil (`wasm32-linear`, §3.1) usa as extensões que o LLVM e o
Chrome suportam. **Não existe subconjunto de Dart catalogado**: a linguagem inteira e o mesmo
conjunto de bibliotecas do dart2wasm oficial (`libraries.json`, alvo `wasm`):

| biblioteca | contrato |
|---|---|
| `dart:core` (inclusive `BigInt`, `RegExp`, `Uri`, `DateTime`, `StackTrace`, `Expando`, `WeakReference`, `Finalizer`, records, `Symbol`) | completa, como na VM |
| `dart:async` (`Future`, `Stream`, `Timer`, zonas, microtasks), `dart:collection`, `dart:convert`, `dart:math`, `dart:typed_data` (inclusive `Float32x4`/`Int32x4`/`Float64x2`, sobre SIMD128) | completas |
| `dart:developer` | o que o dart2wasm oferece |
| `dart:js_interop`, `dart:js_interop_unsafe`, `dart:js_util` | completas no host JS, com valores JS em tabela de handles do adaptador (sem `externref`) |
| `dart:ffi` | `Pointer` sobre a memória linear e `@Native` ligado a funções Wasm/imports, como no dart2wasm |
| `dart:isolate` | mesmo comportamento do dart2wasm (`UnsupportedError` onde ele lança) |
| `dart:io` | não suportada, como no dart2wasm (`dart.library.io` falso) |
| imports `deferred` | suportados |

Constantes `dart.library.*`, imports condicionais e `const bool.fromEnvironment`
seguem o alvo `wasm` do SDK, então pacotes que já funcionam no dart2wasm compilam
sem mudança. Onde o dart2wasm difere da VM por ser web (números, `identical`,
`Platform`), vale o comportamento do dart2wasm; no resto, o da VM.

**Sem código provisório.** Cada fase entrega código definitivo: o compilador é o
pipeline atual (`emit_native`) com um descritor de destino, e o runtime é o
`crates/runtime` atual compilado para wasm32 com a camada de plataforma trocada,
não um runtime reduzido que depois precisaria ser reescrito. Nada de stub que
finge sucesso, de exceção que vira trap, de recurso “documentado como
limitação”: o que o dart2wasm faz, este destino faz.

Não usar o compilador oficial Dart→Wasm para produzir os artefatos próprios; ele
é oráculo e comparador (exige WasmGC, presente no Node 24 e no Chrome atuais).

### Critério de entrega

- Corpus nativo inteiro (`corpus/native`) e corpus JS inteiro (`corpus/js`) com a
  saída igual à da VM (ou à do dart2wasm onde a semântica web manda), em modo
  normal e com GC forçado; os testes web do `new_sali/core` iguais ao `dart run`.
- Programas que só existem no dart2wasm (`js_interop`, `deferred`) iguais ao
  dart2wasm no navegador.
- Mesmo SHA-256 do `.wasm` no navegador e no executor independente (programas
  sem `js_interop`; com ele, o host é o JS, como no oficial).
- Nenhum import de libc, POSIX, Win32, AppKit ou WASI; nenhuma extensão fora do
  perfil da §3.1 (WasmGC entre elas), no módulo final e em todas as dependências.
- Tamanho e velocidade medidos contra o `dart compile wasm -O2/-O4`; a meta é ser
  menor e não mais lento.
- Destinos nativos e JS sem regressão.

## 2. Base existente e lacunas confirmadas

| Área | Base reaproveitável | Trabalho necessário |
|---|---|---|
| Frontend/HIR | Parser, elementos, tipos, lowering e otimizações atuais | Seleção de bibliotecas/capacidades e auditoria de pressupostos nativos |
| ABI | `Target::Wasm32`, ponteiro de 32 bits, `i64` escalar | Contrato completo de runtime, imports/exports, layouts e chamadas indiretas |
| Teste ABI | LLVM escalar emitido como objeto Wasm, cabeçalho conferido | Ligação e execução de Dart real; validar instruções e semântica |
| LLVM instalado | Clang lista `wasm32`/`wasm64`; `lld -flavor wasm` disponível | Registrar WebAssembly no LLVM embutido e controlar features em ambos os geradores |
| Driver | Seleção imutável de destino e caches por alvo | Separar arquitetura, ambiente de execução, formato e capacidades de plataforma |
| Runtime | Objetos, RTI, tracing, exceção pendente, raízes explícitas | Heap linear, contexto por instância, plataforma portátil e runtime Wasm compilável |
| SDK | Fontes e overlays revisados, efeitos/ownership catalogados | Alvo `wasm` do `libraries.json` (constantes, imports condicionais, patches) e fronteiras de host |

Pontos de entrada para leitura antes das alterações:

- [ABI e teste](../crates/abi/src/lib.rs),
  [native_call](../crates/abi/tests/native_call.rs).
- [Destinos](../crates/emit_native/src/target.rs),
  [gerador](../crates/emit_native/src/generator.rs),
  [integração LLVM](../crates/llvm/src/lib.rs).
- [Emissão LLVM](../crates/emit_native/src/llvm/mod.rs),
  [cache](../crates/emit_native/src/cache.rs),
  [SDK](../crates/emit_native/src/sdk_module.rs).
- [Runtime](../crates/runtime/src/lib.rs),
  [fragmentos](../crates/runtime/build.rs),
  [espaço de objetos](../crates/runtime/src/space.rs),
  [layout](../crates/runtime/src/layout.rs),
  [raízes](../crates/runtime/src/gc_roots.rs).

O contrato inicial em [ABI-FFI-WASM](ABI-FFI-WASM.md) não representa um runtime
Wasm pronto. As referências antigas ao driver host-only são históricas; os
destinos nativos atuais estão em [CROSS-COMPILACAO](CROSS-COMPILACAO.md).

## 3. Arquitetura proposta

```text
Dart + package_config + SDK/overlays Wasm
                   ↓
          frontend → HIR → LLVM IR
                   ↓
       LLVM WebAssembly + runtime Wasm
                   ↓
              LLD Wasm
                   ↓
       app.wasm + manifesto da ABI
             /              \
    adaptador JS          host de executor
    navegador             independente
```

O compilador continua executando nativamente em macOS, Linux ou Windows.
Compilar o próprio DartForge para executar dentro de Wasm fica fora desta entrega.
Não será necessário iniciar as VMs para cada compilação Wasm; elas serão usadas
para validar a distribuição e os executores nos sistemas hóspedes.

Mapa inicial de responsabilidades, sem criar arquivos de implementação agora:

| Componente | Alteração planejada |
|---|---|
| `crates/cli` | Comando/opções, erros, manifesto e publicação segura de saídas |
| `crates/emit_native/src/target.rs` e `cross.rs` | Descritor Wasm, capacidades, layout e isolamento de bundles |
| `crates/llvm` e `emit_native/src/generator.rs` | Registro do backend, CPU/features explícitas, objetos/bitcode |
| Ligação em `crates/emit_native` | Caminho próprio LLD Wasm; imports e exports permitidos |
| `emit_native/src/llvm` e HIR | Remover pressupostos sp/rsp/TLS nativos e aplicar ABI/layout do destino |
| `runtime` e seleção de fragmentos | Núcleo portátil, allocator/GC/contexto e implementações de plataforma |
| `native_sdk` e carregador de SDK | Overlays/capacidades Wasm, efeitos e fingerprint independente |
| Integração/distribuição | Adapters JS/executor, validator, corpus e ferramentas empacotadas |

Extrair novos módulos/crates somente quando a separação do núcleo/host estiver
definida; não começar por duplicar frontend ou runtime. Usar nomes/APIs/código
em inglês e preservar a seleção de fontes revisadas usada pelas provas atuais.

### 3.1 Perfil binário fixado: `wasm32-linear`

Alvo LLVM `wasm32-unknown-unknown`, memória linear de 32 bits, uma memória e uma
tabela de funções. Regra de admissão de uma extensão: **o LLVM a gera e o Chrome a
executa**, e ela não é WasmGC. Fica de fora o que contraria o modelo de memória
linear ou o determinismo, cada item por um motivo:

| extensão | perfil | motivo |
|---|---|---|
| sign-extension, conversões sem trap, bulk memory, multivalue, reference types, globais mutáveis | dentro | padrão do LLVM 22 (`-mcpu=generic`) e do Rust |
| SIMD128 | dentro | LLVM, Chrome 91, Firefox 89, Safari 16.4 |
| tail calls | dentro | LLVM, Chrome 112, Firefox 121, Safari 18.2 |
| extended constant expressions | dentro | LLVM, Chrome 114, Firefox 112, Safari 17.4 |
| exception handling com `exnref` | dentro | LLVM 22, Chrome 137, Firefox 131, Safari 18.4, Node 24.15; candidato para as exceções Dart (§3.3) |
| WasmGC e typed function references | fora | o coletor é o do runtime, na memória linear |
| EH legado (`try`/`catch` antigos) | fora | substituído por `exnref` |
| relaxed SIMD | fora | resultados diferentes entre motores |
| threads e memória compartilhada | fora | exige isolamento entre origens (COOP/COEP) na página; o dart2wasm não tem isolates com threads |
| memory64 | fora | ponteiros de 32 bits são mais rápidos; Safari ainda não tem |
| multi-memory, wide arithmetic, custom page sizes, stack switching | fora | sem uso no LLVM ou sem suporte estável no Chrome |

Todos os insumos usam as mesmas features: programa, SDK Dart, runtime Rust
(`std` do alvo `wasm32-unknown-unknown`), builtins e bibliotecas auxiliares. A seção
`target_features` não serve de prova: o portão (`crates/wasm_gate`) valida o módulo
final com `wasmparser` restrito ao perfil e recusa o resto. O executor independente
(`crates/wasm_host`, Wasmtime) liga as mesmas features e nenhuma outra.

### 3.2 Valores, ponteiros, heap e chamadas

- Preservar inicialmente a representação Dart interna de 64 bits onde isso
  mantiver o contrato semântico; `i64` é admitido no MVP.
- Separar rigorosamente `Ref`/handle, índice da tabela de funções e offset de
  memória. Nenhum deles é um ponteiro nativo do host.
- Fazer layout do runtime depender do destino, não de `usize`/`size_of` do
  compilador host. Descrever e testar cabeçalhos, alinhamentos, offsets de
  campos, RTI, closures, listas, typed data, frames e estruturas FFI admitidas.
- Auditar `ptrtoint`/`inttoptr`, tagging, sinais e truncamentos. Conversões para
  offsets Wasm32 precisam preservar faixa e identidade; não truncar silenciosamente
  um handle arbitrário de 64 bits para “fazê-lo caber”.
- Centralizar o allocator da instância. A heap Rust de suporte e a heap Dart
  devem usar regiões disjuntas ou um allocator coordenado; duas cópias independentes
  de `memory.grow`/allocator não podem reservar a mesma região.
- Tratar falha de `memory.grow`, limite configurado e overflow de tamanho antes
  de publicar objetos. Definir fronteiras entre dados estáticos, stack LLVM,
  stacks/frames gerenciados e heap; reservas não podem se sobrepor.
- Liberar objetos para reutilização interna; não depender de devolver páginas
  lineares ao sistema operacional após cada coleta.
- Rebaixar ponteiros de função para índices/convenções LLVM Wasm revisadas.
  Preservar assinatura das chamadas indiretas; trap de assinatura não é um
  mecanismo de despacho dinâmico Dart.

### 3.3 Runtime portátil, GC e exceções

Separar semântica comum das implementações de plataforma. Não copiar todo o
runtime para uma segunda implementação que possa divergir silenciosamente.
O perfil Wasm deverá ter um núcleo sem dependências de SO, preferencialmente
`no_std` + `alloc`, com seleção explícita dos fragmentos/bibliotecas necessários.
Isso não exige converter o compilador host nem todo o runtime desktop para
`no_std`; exige isolar as dependências para o artefato Wasm.

Coletor: o mesmo do nativo (preciso, geracional, shadow stack explícita),
contexto por instância, uma thread. Não escrever um coletor “simples” para depois
trocar: identidade, weak references, `Expando` e `Finalizer` funcionam desde o
primeiro programa. ARC e mapas de pilha nativos não entram no destino Wasm.

Não localizar raízes na stack opaca do engine nem ler `sp`/`rsp` ou usar
registradores TLS nativos. Referências vivas em chamadas, alocações, closures,
globais, async e retornos precisam ser visíveis ao coletor. Auditoria específica
das raízes em otimizações/LTO e nas fronteiras com o host.

Exceções Dart: as duas implementações do nativo servem aqui. O modelo de exceção
pendente (A0) não depende do motor; o modelo por tabelas (A1, `invoke`/`landingpad`)
vira EH Wasm com `exnref`, que o LLVM 22 já gera (W0). Medir as duas no W3 e ficar
com a mais rápida, sem `setjmp/longjmp`. `throw`, `catch`, `rethrow` e `finally`
preservam ordem de efeitos e limpeza.
Panic do runtime, trap Wasm, erro retornado pelo host e exceção Dart têm caminhos
distintos. Não declarar um trap irrecuperável como exceção Dart capturável.

`StackOverflowError` precisa ser capturável como na VM: verificação de
profundidade na entrada das funções contra um orçamento menor que o limite da
pilha LLVM (`__stack_pointer`) e que a pilha do motor, medido nos motores
suportados, para que a exceção Dart sempre ocorra antes da trap. Stack traces usam
metadados próprios de funções, sem unwinder nativo.

### 3.4 ABI comum de host v1

Namespace proposto de imports: `dartforge_host_v1`. Nomes/assinaturas finais
serão fixados antes de escrever os dois adaptadores. A ABI usará `i32`, `f64`
e buffers em memória linear na fronteira JS portátil; inteiros de 64 bits serão
transferidos por buffer ou duas metades, sem exigir a integração JS BigInt.
Callbacks assíncronos usam IDs, nunca objetos JS como referências Dart brutas.

| Operação | Contrato a definir |
|---|---|
| Saída | UTF-8 `(offset, length)`, canal e resultado explícito; consumo síncrono/cópia |
| Relógios | Monotônico e civil separados, unidade e origem documentadas |
| Entropia | Preencher buffer, erro explícito quando indisponível; sem fallback inseguro |
| Agendamento | Solicitação/cancelamento por ID; entrega somente em ponto permitido |
| Recursos opcionais | Capacidade identificada e retorno “não suportado” explícito |
| Inicialização/execução | Inicializar runtime e executar entrada uma vez, com estado verificável |
| Conclusão/erro | Status numérico e buffer diagnóstico, sem encerrar o processo do host |

Só importar o necessário ao perfil do artefato. Nenhum export assinado como
“ponteiro” concede acesso fora da memória linear. Validar faixas com aritmética
que detecte overflow, alinhar estruturas e especificar ownership/lifetime.
Copiar dados que precisem sobreviver ao retorno. Handles externos precisam de
retenção/liberação e proteção contra IDs obsoletos.

O host não pode chamar Dart de novo durante GC, inicialização ou import síncrono
não reentrante. A primeira ABI enfileira conclusões e drena eventos em pontos
controlados. Uma exceção JS lançada por um adaptador não pode atravessar a
fronteira como se o unwinding Dart tivesse ocorrido; adaptadores capturam e
traduzem falhas nos termos da ABI.

### 3.5 Biblioteca padrão e ambiente

O conjunto de bibliotecas é o da §1, com as constantes `dart.library.*` do alvo
`wasm` do SDK, incluindo dependências transitivas e fontes geradas. Não detectar
“Linux” pelo sistema onde o compilador está rodando.

Para o **mesmo binário nos dois ambientes**, as decisões de compilação e a ABI são
as mesmas; o que depende do host (relógio, entropia, timers, JS) passa pela ABI.
`dart:js_interop` exige o host JS, como no dart2wasm: o executor independente
recusa na instanciação um módulo que importa o namespace JS, com mensagem clara.
`dart:io` não existe neste alvo, como no oficial; `dart:ffi` opera sobre a memória
linear e funções ligadas ao módulo, nunca sobre bibliotecas dinâmicas do host.

## 4. Fases, dependências e portas de aceite

A ordem das fases é ordem de trabalho, não de escopo: cada fase entrega código
definitivo, e só a soma delas cumpre o critério da §1. O trabalho será dividido em
commits revisáveis em `main`, sem branches/worktrees adicionais por padrão.

### W0 — Inventário e prova técnica limitada

1. Inventariar dependências de plataforma/representação do runtime, SDK e LLVM.
2. Fixar versões do toolchain, validator, engines e configuração MVP estrita.
3. Conferir ABI de LLVM↔Rust: escalares, `i64`, offsets, alinhamento e chamada
   indireta, sem GC ou bibliotecas completas.
4. Provar objeto→ligação→execução em browser e executor com imports mínimos.
5. Confirmar que runtime auxiliar/builtins podem ser produzidos no mesmo perfil;
   verificar controle de panic, allocator e bibliotecas Rust reconstruídas.

**Aceite:** módulo escalar válido no perfil estrito, mesmo hash nos dois hosts,
resultado esperado e nenhuma dependência inesperada. **Não conta como Dart→Wasm.**
Se Rust/LLVM ou builtins exigirem recursos extras, corrigir essa fronteira antes
de portar o runtime; não renomear o perfil moderno como MVP.

**Resultado (21/10/2026): aceito.** Reproduzir com `scripts/wasm-w0.sh`;
evidências em `bench/results/2026-10-21-wasm-w0/`.

| item | resultado |
|---|---|
| toolchain | LLVM/Clang/LLD 22.1.8 (`-mcpu=generic -msimd128 -mtail-call -mextended-const -mexception-handling`, EH `exnref`); rustc 1.98.1 com `wasm32-unknown-unknown` e `std` |
| portão | `crates/wasm_gate`: `wasmparser` 0.262.0 com o perfil da §3.1, imports só de funções de `dartforge_host_v1`, nenhuma global exportada, uma memória/tabela; controles positivos (sign-ext, saturantes, bulk, SIMD, tail call, `exnref`) e negativos (WasmGC, memória compartilhada, EH legado, relaxed SIMD, memory64, multi-memory, global exportada, WASI, memória importada, export ausente) |
| controle negativo real | módulo LLVM com `atomicrmw` e `--shared-memory`: o portão recusa |
| ABI LLVM↔Rust | `i64` por valor e por buffer, `i128` (`__multi3`), divisão/resto com sinal, deslocamentos, `fptosi`, chamada indireta por tabela, exceção LLVM (`invoke`/`catchpad` → `try_table`) lançada e capturada, `std` do Rust (dlmalloc com `memory.grow` a partir de 2 páginas, `Vec`, `format!`, `thread_local!`, hook de panic pela ABI) |
| módulo | 27.968 bytes, 96 funções, um import (`dartforge_host_v1.write`) |
| hosts | mesmo SHA-256 e mesma saída, status 0, no Wasmtime 50.0.0 (`crates/wasm_host`, Cranelift, só o perfil), no Node 24.21.0 (V8) e no Chrome 155 headless; saída comparada com valores calculados em Python |
| ABI de host | primeira parte da v1 fixada em [WASM-HOST-ABI-V1.md](WASM-HOST-ABI-V1.md) |

Limites: Node e Chrome usam o mesmo motor (V8); Firefox e Safari entram no W5.

**Inventário do runtime (`crates/runtime`, 64 fragmentos, ~59 mil linhas).** O que
precisa de outra implementação ou ficar fora do perfil Wasm:

| dependência | onde | destino no W2 |
|---|---|---|
| páginas por `mmap`/`VirtualAlloc` | `space.rs` | reservas na memória linear (`memory.grow`) |
| `thread_local!` (contexto da thread, caches) | 23 fragmentos, inclusive `core`, `heap`, `gc_roots`, `selectors` | contexto por instância (uma thread por módulo) |
| `std::process::exit/abort` | `core`, `heap`, `gc_roots`, `selectors` | status de `dartforge_run` e trap controlada |
| `std::thread` (isolados, coletor auxiliar, testes) | 20 fragmentos | uma thread; `Isolate` como no dart2wasm |
| `std::fs`, `std::net`, `std::process`, `std::env` | fragmentos `io_*`, `natives_system` | não entram: `dart:io` não existe no alvo `wasm` |
| `std::time` | 13 fragmentos | relógios da ABI v1 |
| rustls/ring, zlib (`libz-sys` em C), cifras de `tls_formats` | `tls`, `tls_formats`, `zlib` | não entram (são de `dart:io`) |
| `extern "C"` do SO e `cfg(unix/windows/target_os)` | ~40 fragmentos | seleção de fragmentos por perfil (W1/W2) |

O seletor de fragmentos de `crates/runtime/build.rs` é o ponto de partida: o perfil
Wasm escolhe os fragmentos das bibliotecas da §1 com a camada de plataforma Wasm;
nenhum fragmento é reescrito em versão reduzida.

### W1 — Modelo de destino, ABI e caches

1. Acrescentar o destino ao modelo sem fingir que Wasm é Linux/Unix.
2. Definir descritor compartilhado de layout e ABI, versões e feature set.
3. Registrar o backend WebAssembly no LLVM embutido; configurar Clang e LLD.
4. Separar caches host/wasm, MVP/outros perfis e revisões da ABI/runtime/SDK.
5. Validar opções incompatíveis: JIT nativo, CPU host, shared SDK nativo,
   table exceptions, stack maps e bundles/sysroots desktop.
6. Fixar allowlist de imports/exports e resolver todos os demais símbolos na
   ligação. Não usar liberação irrestrita de símbolos indefinidos para esconder
   fragmentos do runtime ainda não portados. Preservar os entry points exigidos
   pela ABI mesmo sob dead stripping/LTO.

**Aceite:** consultas de alvo não inicializam plataforma errada, metadados
corretos, recusa de combinações inválidas e testes dos destinos antigos passam.
Nenhum cache/bundle incompatível pode ser aceito por alteração manual do manifesto.

**Resultado (21/10/2026): aceito.**

| item | resultado |
|---|---|
| destino | `NativeTarget::Wasm32` e `Os::Wasm` (`target.rs`): triple `wasm32-unknown-unknown`, ponteiro de 4 bytes, valores Dart em células de 8 bytes. Não é Unix nem Windows (`is_unix()` falso); cada `match` por sistema ganhou decisão explícita (21 pontos) |
| LLVM | o LLVM embutido registra o backend WebAssembly (componente `webassembly`); CPU `generic` mais `+simd128,+tail-call,+extended-const`, sem PIC; o caminho por Clang usa as mesmas flags |
| caches | diretório por triple; a identidade do gerador inclui perfil, CPU e features, então objeto nativo e objeto Wasm nunca dividem chave |
| ligação | `linker_wasm.rs`: `lld -flavor wasm`, `--no-entry`, exports da ABI v1, pilha de 8 MiB, memória máxima de 4 GiB, `--strip-all` sem `--debug`, nunca `--allow-undefined`; o módulo ligado passa pelo portão, e um símbolo não resolvido derruba o link em vez de virar import (teste) |
| geração | a verificação de pilha lê `__stack_pointer` (`llvm.stacksave`) em vez de `sp`/`rsp`; imagem estática na seção `dfimg` com marcadores `extern_weak` (no `wasm-ld`, seção sem objetos vira o intervalo 0..0); ABI C do Clang para wasm32 (struct de uma folha vai direto, o resto `byval`/`sret`); ponteiro C de 4 bytes; inteiros de `Abi` do `dart:ffi` pelo mapeamento `androidArm`, como o dart2wasm; `Float32x4.clamp` com a semântica do dart2wasm (NaN preservado); `Platform.operatingSystem` não é dobrado; `nounwind` como no Unix |
| recusas | `compile-wasm` recusa `--cpu`, `--exceptions`, `--memory`, `--roots`, `--stack-trace`, `--subsystem` e `--target`; `aot`/`compile-native --target wasm32` mandam usar `compile-wasm`; o JIT não aceita destino; runtime, SDK compartilhado e bundles nativos nunca entram num módulo (erro explícito até o W2/W3) |
| CLI | `dartforge compile-wasm <app.dart> -o <app.wasm> [--optimize] [--enable-asserts] [--debug] [--emit-ir]` |
| sondagem | o IR de produção de um programa real emitido para macOS, com o cabeçalho trocado e só os três pontos de plataforma já resolvidos acima (TLS por `asm`, `read_register`, seção Mach-O) substituídos, compila para um objeto wasm32: o resto do IR o backend WebAssembly já aceita |
| regressão | testes do `emit_native` (1 027), da CLI e corpus nativo contra a VM nos cinco modos |

### W2 — Núcleo do runtime e heap linear

1. Isolar seleção de fragmentos e dependências Rust do perfil Wasm.
2. Implementar reservas/allocator na memória linear com limites configuráveis.
3. Implementar contexto por instância, estados de inicialização e destruição.
4. Validar layouts compartilhados entre Rust e LLVM, strings e objetos básicos.
5. Testar uma e duas instâncias independentes, crescimento e falta de memória.

**Aceite:** runtime liga sem imports ocultos; allocations não se sobrepõem;
memória e contexto não vazam entre instâncias. Falhas não corrompem o estado
ou viram sucesso silencioso. OOM irrecuperável deve ser distinguido de erro Dart.

### W3 — Pipeline Dart completo com o runtime real

1. Compilar pelo pipeline DartForge completo, com o alvo `wasm` do SDK, usando o
   runtime real (não um runtime de bootstrap): objetos, strings, RTI, closures,
   coleções, exceções pendentes e o coletor geracional.
2. Shadow stack, barreiras e raízes em campos/globais, closures, laços, chamadas
   indiretas e retornos, com o verificador de raízes do nativo.
3. `throw`/`catch`/`finally`/`rethrow`, stack traces e `StackOverflowError`
   capturável (§3.3).
4. Rodar o corpus nativo inteiro, normal e com `--gc-stress`, e registrar cada
   divergência como defeito a corrigir, não como exclusão.

**Aceite:** a ligação usa o runtime de produção; GC sob estresse e verificador de
raízes passam nos programas que já executam. Desabilitar GC não aprova nada.

### W4 — Paridade com a VM e o dart2wasm

1. Corpus nativo e JS inteiros iguais à VM, normal e com GC forçado.
2. Bibliotecas da §1 completas: `core` (inclusive `BigInt`, `RegExp`, `Uri`,
   `DateTime`, weak references, finalizers), `collection`, `convert`, `math`,
   `typed_data` (inclusive as classes SIMD sobre SIMD128), `developer`, `ffi` sobre
   a memória linear.
3. Inteiros, shifts, divisão/resto, overflow, NaN/−0 e double→int; onde a VM e o dart2wasm divergem, seguir o dart2wasm e
   registrar o caso no corpus.
4. Testes web do `new_sali/core` iguais ao `dart run`.

**Aceite:** zero divergências no corpus, com GC forçado; diferenças em relação à
VM só onde o dart2wasm também difere, cada uma com teste.

### W5 — Dois hosts, mesmo módulo

1. Implementar adaptador JS pequeno, independente do compilador, para browsers.
2. Executor independente (`crates/wasm_host`, Wasmtime fixado e restrito ao
   perfil), sem exigir a CLI WASI.
3. Definir API de entrada/conclusão, argumentos, códigos de saída e diagnóstico.
4. Tratar atualização de views JS após `memory.grow`, strings e buffers copiados.
5. Validar callbacks, encerramento, capacidade ausente, host que retorna erro,
   import faltante, assinatura errada e manifesto ABI incompatível.

**Aceite:** o **mesmo hash** executa ao menos em Chromium, Firefox e executor
independente; Safari/WebKit é a próxima porta antes da declaração de suporte
à web nessa plataforma. Não compilar separadamente para “provar” mesmo módulo.
Nenhum host fornece privilégios de filesystem/rede por padrão.

### W6 — Async, JS interop e deferred

1. Event loop completo: microtasks, `Future`/`Stream`, zonas, `Timer` e
   cancelamento, erros assíncronos e encerramento, com raízes vivas nas máquinas
   de estado; timers pela ABI (no JS, `setTimeout`; no executor, laço próprio).
2. `dart:js_interop`, `js_interop_unsafe` e `js_util` com tabela de handles JS no
   adaptador, liberação por finalizer e callbacks Dart chamados pelo JS fora do GC.
3. Imports `deferred` (módulos carregados sob demanda ou ligados no mesmo módulo,
   com o mesmo comportamento observável do dart2wasm).
4. Testar conclusões síncronas/atrasadas, cancelamento tardio e IDs obsoletos.

**Aceite:** corpus `async` e de interop iguais à VM/dart2wasm, sem reentrada
durante GC, sem busy loop quando ocioso e sem bloquear o event loop do browser.

### W7 — CLI, distribuição e homologação

Comando fixado no W1 (o equivalente do `dart compile wasm`):

```sh
dartforge compile-wasm app.dart --sdk /path/to/dart-sdk/lib \
  --packages .dart_tool/package_config.json -o build/app.wasm --optimize
```

Ele usa os componentes comuns do backend, sem os conceitos de executável nativo,
sysroot e biblioteca dinâmica, e recusa as opções que só valem para o nativo.

Entregar `.wasm`, manifesto de ABI/features/capacidades e loader/adaptador
selecionado, com documentação de implantação. O manifesto registra hashes do
módulo, SDK/overlays, runtime, compilador, toolchain e opções. Definir nomes sem
ambiguidade para fonte de debug/mapas e separar artefatos de release/debug.

Validar construção/distribuição do compilador nos três hosts existentes,
reutilizando as VMs Parallels **uma por vez**, sem build/JIT pesado concorrente
nos hóspedes de 4 GB. Executar o mesmo corpus Wasm em cada executor suportado.
Não exigir igualdade binária entre hosts antes de controlar timestamps, caminhos
e versões das ferramentas; a porta de portabilidade do mesmo módulo usa cópia
e hashes do artefato efetivamente distribuído.

**Aceite:** instalação limpa compila e executa exemplos documentados; nenhum
caminho de desenvolvimento ou cache antigo é requisito escondido. Artefatos
inválidos/falhos não substituem a última saída válida nem retornam exit 0.
Antes de declarar suporte a Safari/WebKit, incluir sua execução real no mesmo
corpus e registrar a versão; até lá, identificar nominalmente os browsers validados.

## 5. Plano de testes e evidências

| Camada | Verificação obrigatória |
|---|---|
| Binário | Portão do perfil no módulo final, debug/release/LTO; imports/exports permitidos |
| ABI/layout | Rust↔LLVM↔host; limites, alinhamentos, i64 por buffer, função indireta, overflow |
| Runtime | Heap/allocator, GC forçado, erros, callbacks, duas instâncias e encerramento |
| Linguagem | Corpus nativo e JS inteiros contra a VM, normal e com GC forçado; dart2wasm onde a semântica web manda |
| Bibliotecas | Todas as bibliotecas da §1; testes web do `new_sali/core`; programas `js_interop`/`deferred` contra o dart2wasm |
| Integração | Browser + executor com o mesmo hash; erros de import/capacidade/versão |
| Regressão nativa | Testes/contratos existentes nos cinco destinos, AOT/JIT desktop nos três SOs |
| Distribuição | Toolchain/cache limpos, relocação, ausência de dependências de máquina |

Adicionar controles negativos que insiram deliberadamente módulos com extensão
proibida, import desconhecido, assinatura incompatível, layout errado e manifesto
obsoleto. O gate precisa falhar nesses casos, não só aceitar artefatos válidos.
Conservar também resultados inesperados e contagem de casos não executados.

Fixar lista de corpus, hashes de fontes/configuração/dependências e versões dos
oráculos. Comparar saídas normalizadas somente nos campos não determinísticos
declarados, sem apagar mensagens/exceções relevantes. Timeouts são falhas ou
limitações identificadas, nunca testes aprovados por ausência de resposta.

Os checks de CONTRIBUTING continuam obrigatórios para implementação: fmt,
Clippy estrito, workspace com ignorados, CLI native/JIT separada e documentação.
A falha já existente de poda de backends estrangeiros não vira aprovação por
este plano; registrar o estado anterior e qualquer nova regressão separadamente.

## 6. Medições depois da correção

Separar cinco custos: compilação Dart→Wasm, validação/compilação pelo engine,
instanciação, primeira execução e execução aquecida. Não comparar startup de
um engine com throughput aquecido de outro.

- Compilação: processo novo e residente, quando existir; caches frios/aquecidos
  discriminados; mesmo corpus/dependências; medições alternadas e várias amostras.
- Tamanho: `.wasm` bruto/gzip/Brotli, loader/runtime JS, manifesto e total
  necessário ao primeiro carregamento; debug e release separados.
- Memória: páginas lineares/livres/vivas, pico e crescimento, memória do host
  e limite configurado. Memória linear reservada não equivale a objetos vivos.
- Execução: CPU-bound, strings/Unicode, JSON, coleções/alocação, RTI/dispatch,
  exceções e async. Conferir checksums antes de aceitar tempos.
- Comparadores: JS dev/produção próprios, DDC/dart2js com contratos identificados,
  AOT nativo e, opcionalmente, Dart→Wasm oficial quando compatível com o corpus.
  Esse último pode depender de features fora do MVP e não constitui comparação
  de requisitos de execução equivalentes.

Não prometer ganho de velocidade ou tamanho antes dessas medições. Otimizações
de layout/GC, LTO, inlining, tabelas e RTI só entram após W3–W5, com ablação,
correção sob GC e inspeção de features da saída final.

## 7. Riscos e decisões que bloqueiam os marcos

| Risco | Tratamento/porta |
|---|---|
| Rust/libs introduzem features modernas | Reconstruir dependências no perfil; validator final obrigatório em W0 |
| Runtime demasiado acoplado a std/SO | Inventário de fragmentos e núcleo portátil em W0/W2, sem stubs silenciosos |
| Layout host64 vaza para Wasm32 | Descritor por destino e fixtures independentes antes de objetos reais |
| Raiz perdida ou callback reentrante | Shadow stack, contratos de efeitos, GC forçado e host controlado |
| ABI LLVM/Rust para agregados diverge | Assinaturas escalares/buffers e testes bidirecionais; sem assumir C nativo |
| Heap Rust/Dart/stack colidem | Mapa de memória e allocator coordenado, limites e testes de crescimento |
| Stack overflow/panic vira trap irrecuperável | Contratos de erro separados e limitação documentada até prova |
| Bibliotecas arrastam FFI/I/O do SDK desktop | Overlays Wasm, capacidades e seleção de fontes no cache |
| “Mesmo módulo” depende de WASI ou DOM | ABI comum explícita, adapters separados e mesma cópia/hash nos testes |
| LTO reintroduz instrução proibida | Verificar o artefato ligado, não só IR/objetos intermediários |

Cada decisão de ABI/layout/features é registrada antes dos consumidores e
versionada no manifesto/cache. Não estimar prazo fechado antes de W0: a maior
incerteza é o isolamento do runtime, não a existência do backend Wasm do LLVM.
Depois de W0, estimar pacotes de trabalho por dependência e tamanho, com revisão
do escopo quando houver evidência nova.

## 8. Ordem recomendada e fora do primeiro escopo

Caminho crítico: **W0 → W1 → W2 → W3 → W4/W5 → W6 → W7**.
A especificação dos hosts começa em W1; implementações de W5 só se estabilizam
após o ABI/GC de W3. Packaging e CI podem ser preparados progressivamente,
mas não antecipam a homologação final.

Fora do destino (o dart2wasm também não tem): `dart:io`, `dart:mirrors`,
`dart:html`, isolates com threads, LLVM JIT dentro do browser e hot reload. Fora do
perfil pelos motivos da §3.1: WasmGC, threads com memória compartilhada, relaxed
SIMD e memory64. Um backend `dart_ui` para canvas/WebGL/DOM requer plano próprio.

## 9. Referências e natureza das decisões

Fontes baixados, ordem de leitura e relação com W0–W7:
[referências locais Wasm MVP](REFERENCIAS-WASM-MVP.md), em
`/Volumes/ssd_dock/references`. O guia distingue clones novos e snapshots
anteriores, com revisões e hashes em manifesto próprio.

Os contratos da implementação atual vêm dos arquivos locais vinculados na seção
2. As fases, nomes de comandos, ABI v1 e escolhas iniciais de runtime deste
documento são **propostas de engenharia**, não recursos existentes.

- [WebAssembly: portabilidade e imports](https://webassembly.org/docs/portability/):
  o módulo depende das APIs fornecidas pelo host, não de syscalls universais.
- [Rust: wasm32-unknown-unknown](https://doc.rust-lang.org/stable/rustc/platform-support/wasm32-unknown-unknown.html):
  limitações de std, evolução das features padrão e cuidados para MVP estrito.
- [Histórico de mudanças da especificação](https://webassembly.github.io/spec/core/appendix/changes.html):
  distinguir o conjunto original das extensões posteriores.

Referências externas consultadas em 20/10/2026. As versões efetivamente usadas
na implementação/testes deverão ser fixadas e registradas em W0, sem depender
de documentação mutável como definição única do perfil.
