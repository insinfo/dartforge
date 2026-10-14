# DartForge para o dart_ui: pacote e anotações sem quebrar o ecossistema

Escrito em 2026-10-12; atualizado em 2026-10-13. Proposta de como o DartForge pode resolver ou aliviar os
problemas que o **dart_ui** (`/Volumes/ssd_dock/MyProjectsDart/dart_ui/`, um
framework de UI multiplataforma em Dart puro sobre FFI) documenta como limites
da VM e do toolchain oficial. A condição é manter compatibilidade: o mesmo
código continua rodando na VM oficial, no `dart compile exe` e no `dart analyze`.

O inventário dos problemas vem dos documentos do próprio dart_ui:
- `README.md`, "Limitações conhecidas";
- `doc/ROTEIRO_FRAMEWORK_MULTIPLATAFORMA_100_PURO_DART.md` §68 e registro de
  riscos R01–R25;
- `doc/propostas/01`, `04` e `08`, os pedidos ao SDK;
- `doc/TECNICA_MAIN_THREAD_DART_FFI.md`, `doc/SPIKE_MACOS_MAIN_THREAD.md` e os
  logs em `doc/logs/`.

Abaixo, a numeração P1–P17 é a desse inventário.

## 1. Resumo

O DartForge tem uma vantagem estrutural para o dart_ui: **o runtime e o laço de
eventos são nossos** (Rust, em `crates/runtime`), e o executável é um binário
nativo comum.

**Três dos problemas mais caros do dart_ui já não existem no DartForge AOT, por
construção:**

1. **`main()` roda na thread 0 do processo (P1).**
   - O `main` C do executável chama `dartforge_iniciar`
     (`crates/runtime/src/nucleo.rs:36`), que roda o isolado principal na própria
     thread que entrou (`crates/emit_native/src/llvm/mod.rs:2822`).
   - Não há o `Dart_RunLoop` que estaciona a thread 0. Com isso, `pthread_main_np()`
     vale 1 e o AppKit aceita a thread.
2. **Cada isolado é uma thread do SO dedicada (P6).** `isolados.rs:656` usa
   `std::thread::Builder`, e o isolado não migra de thread entre `await`s. Isso
   mantém estáveis o TLS de drivers, o COM STA e o CFRunLoop da thread.
3. **O executável já é nativo, pequeno e de partida rápida (P14).**
   - O "olá" tem 3,1 MB e parte em ~25 ms (`RELATORIO-COMPARATIVO-NATIVO.md`).
   - O próprio dart_ui mede ~5 s até poder dar play com `dart run`.

**O resto se divide em dois grupos:**
- recursos que o runtime pode oferecer com pouco trabalho, porque o laço é nosso
  (P2, P3, P4, P5, P8 e P9);
- recursos de compilador já parcialmente presentes (P10, P11, P12, P16 e P17).

## 2. Como manter a compatibilidade

Há quatro mecanismos, combináveis. Nenhum quebra a VM oficial.

### 2.1 Constante de detecção

```dart
const bool isDartForge = bool.fromEnvironment('dartforge.compiler');
```

- **Implementado no lowering nativo:** `dartforge.compiler=true` no AOT.
  O analisador mantém o valor desconhecido; a detecção nos demais backends
  não foi validada nesta rodada.
- A VM e o `dart compile exe` veem `false`.
- Por ser `const`, o ramo não usado sai na compilação, nos dois mundos.
- **Custo:** quase nenhum. É só definir a constante no ambiente de compilação
  (`-D` implícito).

### 2.2 `@pragma('dartforge:...')`

- A especificação do Dart manda ignorar pragmas desconhecidas; a VM, o
  `dart2js` e o analisador já fazem isso.
- As pragmas servem para dicas que **não mudam a semântica**: inline, laço sem
  checagem de limites provada, região sem coleta, afinidade e prioridade.
- Se o DartForge não puder honrar uma pragma, compila normalmente e avisa.

### 2.3 Pacote `package:dartforge` (Dart puro, publicado no pub)

- Toda API tem duas implementações:
  - no DartForge, intrínsecos que o compilador troca por chamadas ao runtime
    (como hoje acontece com `dart:ffi`);
  - na VM, uma implementação Dart de fallback, ou `isSupported == false`.
- O dart_ui escolhe o caminho por `if (Laco.integracaoNativa.isSupported)`, sem
  `import` condicional.
- O pacote é analisável e testável na VM: os testes do dart_ui continuam rodando
  como hoje.

### 2.4 `@pragma('vm:...')` já existentes

- O DartForge deve honrar as pragmas da VM que o dart_ui já usa:
  - `vm:prefer-inline` (15 ocorrências em `lib/` na revisão de 2026-10-13) e `vm:never-inline`;
  - `vm:entry-point`;
  - `vm:awaiter-link`.
- **As duas primeiras foram implementadas no nativo em 2026-10-13:**
  preferência limitada no inliner HIR e `inlinehint` no LLVM; proibição nos
  dois níveis com `noinline`. Isso ainda não demonstra ganho de tempo.
  `vm:entry-point` e `vm:awaiter-link` continuam fora desta entrega.

**Regra:** um recurso que muda semântica observável (por exemplo, memória
compartilhada entre isolados) só entra pelo pacote, com `isSupported`. Nunca só
por pragma.

## 3. Problema a problema

Legenda do estado no DartForge:
- **já resolve:** funciona hoje;
- **runtime:** pequeno a médio, o laço e o runtime são nossos;
- **compilador:** exige trabalho no emissor;
- **pesquisa:** design em aberto.

### P1. Thread principal do processo (macOS/AppKit) — já resolve, falta a fila principal

- **No dart_ui hoje:**
  - três backends para contornar a VM: SkyLight (API privada), sequestro da
    thread 0 por `SIGUSR2` e um host Objective-C de 2.397 linhas em processo
    separado;
  - um popup leva 66 ms no M4 e um drag-and-drop responde "uma atualização
    atrás".
- **No DartForge:**
  - o isolado principal já está na thread 0;
  - falta drenar a **main dispatch queue** e o `CFRunLoop` entre eventos Dart. A
    SkyLight do macOS 27 depende disso, e o `dispatch_async(&_dispatch_main_q)`
    nunca é atendido sem o run loop.
- **Proposta (runtime):** o laço de eventos (`eventos.rs`, `dartforge_laco_de_eventos`)
  ganha uma **bomba nativa** opcional.
  - No macOS, em vez de dormir até o próximo prazo, chama
    `CFRunLoopRunInMode(kCFRunLoopDefaultMode, prazo, true)`.
  - Uma fonte do run loop acorda o laço quando chega mensagem Dart.
  - O mesmo vale para `MsgWaitForMultipleObjectsEx` no Windows e para `poll`
    nos fds do X11/Wayland no Linux.

```dart
// package:dartforge
abstract final class EventLoop {
  /// On DartForge AOT: the Dart loop waits on the OS pump
  /// (macOS: CFRunLoop and main queue; Windows: messages; Linux: fds).
  static void integrateNativePump({List<int> descriptors = const []});
  static bool get isNativeIntegrationSupported; // false on the Dart VM
}
```

- **Efeito no dart_ui:**
  - é uma base para um backend AppKit direto no DartForge, sem host ou
    sinal; ainda é necessário distribuir os `NSEvent` com o AppKit.
    `CFRunLoop` sozinho drena fontes/fila principal, mas não substitui o
    caminho `nextEventMatchingMask:`/`sendEvent:` do backend atual;
  - os três backends atuais ficam como caminho da VM oficial.
- **Fallback na VM:** os backends de hoje, sem mudança.

### P2. Loop do isolado × loop nativo — runtime

- **No dart_ui hoje:**
  - polling com `pumpEvents(timeout)` e backoff de 16–250 ms;
  - na POC-19, o timer de 60 Hz roda efetivamente a 16 Hz, e a latência entre
    isolados no p95 é de 63 ms (com a API ideal, 0,54 ms).
- **No DartForge:** o laço tem a fila de timers e de mensagens na mão
  (`crates/runtime/src/events.rs`). O prazo existe internamente; expor um
objeto de acordar exige integrar os caminhos de notificação e espera de
`ports.rs`, preservando mensagens que chegam entre destravar e esperar:

```dart
abstract final class EventLoop {
  static Duration? get nextDeadline;   // null = nothing scheduled
  static int get wakeHandle;    // fd (POSIX) or HANDLE (Windows)
  static void runOneTurn();           // microtasks + one event
}
```

- Com a bomba nativa de P1, nem isso é preciso: o próprio laço espera nos
  dois mundos.
- **Fallback na VM:** o polling atual.
- É exatamente a proposta 04 do dart_ui, que no SDK oficial depende do
  `Isolate.onEvent`, até hoje não implementado.

### P3. Laço modal aninhado (arrasto, diálogos, menus, OLE) — runtime

- **No dart_ui hoje:** o Dart fica inacessível enquanto `DispatchMessageW` não
  retorna. O vídeo congela durante o arrasto da janela.
- **No DartForge:** `EventLoop.runOneTurn()` chamado de dentro do callback
  nativo, isto é, um turno reentrante com orçamento de tempo. A reentrância
  precisa de duas regras:
  - não drenar de novo um evento que já está na pilha;
  - empilhar/restaurar a mensagem e suas raízes: `dispatch_next` usa hoje
    um único `CURRENT` e `MESSAGE_ROOT`, inadequados à chamada aninhada;
  - definir uma política de dependências/reentrância antes de oferecer a API.
    Detectar todo `await` que dependa do quadro externo não é uma garantia
    demonstrada pelo runtime atual.
- **Fallback na VM:** os contornos atuais (`SetTimer` e relógio virtual).

### P4. Callback síncrono vindo de outra thread — runtime

- **No dart_ui hoje:** `NativeCallable.isolateLocal` chamado de outra thread
  **aborta o processo**. O DartForge faz o mesmo hoje
  (`ffi_callbacks.rs:441`: "Um callback local chamado fora da thread do isolado
  dono é fatal").
- **Proposta:** um modo novo de callback, "síncrono pelo dono":
  1. a thread estranha posta o pedido na fila do isolado dono e espera numa
     variável de condição;
  2. o dono atende no próximo turno, inclusive num turno reentrante (P3);
  3. o resultado volta para quem chamou.
  - Serve para os delegates do AppKit que devolvem `BOOL` (`windowShouldClose:`),
    para o UI Automation sem STA e para o COM.
  - **Risco de deadlock** se o dono estiver bloqueado esperando a própria thread
    estranha: o runtime detecta o ciclo e devolve o `exceptionalReturn`.

```dart
final cb = SyncNativeCallable<Bool Function(Pointer)>(handler, exceptionalReturn: false);
```

- **Fallback na VM:** `isSupported == false`; o dart_ui continua com `.listener`
  e o host.

### P5. `NativeCallable` com callback tardio depois do `close()` — runtime

- **Hoje:** é comportamento indefinido. O dart_ui mantém um registro geracional
  próprio (risco R01, "alto/crítico").
- **No DartForge:** o trampolim e o contexto são nossos (`ffi_callbacks.rs`).
  Depois do `close()`, o trampolim pode continuar mapeado (a memória executável
  não é devolvida) e responder com o `exceptionalReturn`, contando a chamada
  tardia num contador de diagnóstico.
- **Compatibilidade:** é o mesmo código Dart; só o comportamento indefinido vira
  definido.

### P6. Afinidade isolado↔thread — já resolve

- Cada isolado do DartForge é uma thread dedicada e não migra.
- Falta só o encerramento limpo dos destrutores TLS de bibliotecas nativas, para
  repetir no DartForge o caso do D3D12 (POC-22).
- **Pacote:** expor `Isolate.current.threadId` para o dart_ui conferir em
  tempo de execução. Na VM, `null`.

### P7. Concorrência real e memória compartilhada — runtime e pesquisa

- **Hoje:** anéis e mutex em memória nativa, ligados à mão.
- **Curto prazo (pacote):** primitivas prontas e corretas por plataforma:
  - `Mutex`, `ConditionVariable` e `Atomic<Int64>` sobre `Pointer`;
  - views com limite conferido (`NativeMemory.view(ptr, bytes)`).
  - No DartForge viram intrínsecos com operações atômicas do LLVM; na VM, o FFI
    atual.
- **Longo prazo (pesquisa):** objetos imutáveis compartilhados entre isolados do
  mesmo grupo. O DartForge já cogita isso com ARC.

### P8. Pausas do GC durante o quadro — runtime (e o ARC)

- **Hoje:** o GC custava 12,8–14,3 ms por quadro no player 1080p, até que o
  framework tirasse o framebuffer do heap.
- **No DartForge:**
  - **Agora, sem mudar o coletor:**
    - `GarbageCollector.deferUntil(prazo)` impede a coleta durante o quadro;
    - `GarbageCollector.collectWhenIdle()` coleta no ocioso.
    - O coletor do DartForge só roda em pontos de alocação e em gatilhos que o
      runtime controla (`heap.rs`, `coletar_automatico`).
  - **Com o ARC (`--memory arc`):** objetos acíclicos morrem quando o último
    dono solta, sem varredura. O lixo de quadro vira custo distribuído, sem
    pausa. O ARC ainda está 2,7× (legado) e 5,2× (preparado) mais lento que o
    tracing no agregado (`ESTADO.md`); é trabalho em curso.
- **Pragma:**

```dart
@pragma('dartforge:no-gc')
void desenharQuadro(...)
```

  Dica para o coletor não iniciar uma coleta dentro da função. A VM ignora.

### P9. Liberação determinística de recursos nativos — runtime e ARC

- **Hoje:** `VideoSample.release()` manual em oito rotas. Pools de autorelease
  à mão.
- **Com o ARC:** um `NativeFinalizer` dispara quando o RC chega a zero, não numa
  coleta maior distante.
- **Sem ARC (pacote):** `using(resource, (r) { ... })` com liberação garantida e
  `@pragma('dartforge:scoped')` para avisar em compilação quando o recurso escapa
  do escopo. Na VM, só a função (`try/finally`).

### P10. Laços numéricos, SIMD e inline — compilador

O DartForge gera código pelo LLVM:
- laços sobre listas tipadas versionados sem checagem de limites, que o LLVM
  autovetoriza;
- `Float32x4`, `Int32x4` e `Float64x2` em vetores nativos (`hir::Type::V4I32`
  etc.);
- leitura e escrita em `Pointer<T>` como carga e gravação diretas
  (`CargaNativa`), sem o custo de 1,88× que o dart_ui mediu na VM.

A fazer:
- **feito no nativo:** `vm:prefer-inline`/`vm:never-inline`; preferência não
  força inline nem remove limites estruturais/de depuração;
- `@pragma('dartforge:vectorize')` para pedir e conferir a vetorização (relatório
  de otimização);
- `gather` em `Float32x4List` como intrínseco;
- `fillRange`/`setRange` sobre memória nativa como `memset`/`memcpy`.

**Medida a fazer:** o rasterizador de strips e o YUV→RGB do dart_ui compilados
pelo DartForge, contra a VM.

### P11. ABI: struct por valor, `_stret`, variádicas — já resolve em boa parte

- O DartForge tem `ChamadaNativaComposta`: struct ou union por valor e
  variádicas, com a ABI C do alvo em `llvm/abi_c.rs`.
- **Pacote:** um `objc_msgSend` tipado em que o compilador escolhe a variante
  certa por arquitetura (`_stret`/`_fpret`).
- **Pragma (pesquisa):** `@pragma('dartforge:check-layout', 'header.h')`
  para conferir o layout de `Struct` contra o header C em compilação.

### P12. Diagnóstico de falhas FFI em AOT — já resolve em parte

- **O que já existe:**
  - o executável do DartForge é nativo comum, com DWARF (`--debug`) e
    símbolos (`DARTFORGE_KEEP_SYMBOLS=1`);
  - o LLDB e o `sample` veem nomes Dart;
  - o rastro simbólico (`--stack-trace=symbolic`) dá a pilha Dart nas exceções.
- **A fazer (runtime):**
  - registrar a última chamada FFI ativa por thread e imprimi-la no `SIGSEGV`;
  - timeline de quadros em AOT, que hoje o dart_ui não tem.

### P13. Exceções e `await` dentro de callbacks FFI — compilador

- O DartForge pode **recusar em compilação** um callback `async`, ou avisar sobre
  o `await` que vira código morto.
- Quando uma exceção é engolida pelo `exceptionalReturn`, pode publicá-la no
  laço como erro não capturado, com o rastro.

### P14. Partida, empacotamento e toolchain — já resolve em parte

- **Já resolve:**
  - partida de ~25 ms;
  - executável pequeno;
  - compilação cruzada pelo LLVM.
- **A fazer:**
  - **implementado:** `--subsystem=gui` no PE, escolhido durante a ligação
    (`crates/emit_native/src/linker_windows.rs`), com CRT GUI próprio.
    O padrão continua `console`; execução em Windows ainda precisa ser validada;
  - montagem do `.app` no macOS, com `Info.plist` e assinatura.

### P15. Sinais e encerramento — deixa de ser necessário

- Com P1 resolvido, não há sequestro por `SIGUSR2`.
- O encerramento com isolados criados e não fechados pode ser definido no
  runtime: espera com prazo, depois aviso.

### P16. Carregamento de bibliotecas — pacote

- `NativeLibrary.openFirst(['libc.so.6', 'libpthread.so.0', ...])` e
  `hasSymbol`.
- No DartForge, os símbolos de `@Native` resolvidos podem ser ligados
  estaticamente quando a biblioteca é do sistema (sem `dlsym` por chamada).

### P17. Memória nativa sem cópia — runtime

- `MappedMemory.open(file)` com `mmap` ou `MapViewOfFile`, devolvendo uma
  `Uint8List` externa com ciclo de vida.
- No DartForge, as listas tipadas externas já existem no runtime (visões
  externas). Na VM, o pacote faz o mesmo por FFI.

## 4. O pacote `package:dartforge`

Esboço da superfície mínima. Cada classe tem `isSupported` e o fallback descrito:

| API | P | DartForge | VM oficial |
|---|---|---|---|
| `const isDartForge` | — | `true` | `false` |
| `EventLoop.integrateNativePump()` | P1, P2 | bomba do SO no laço do runtime | `UnsupportedError`; o dart_ui segue com o polling |
| `EventLoop.nextDeadline` / `wakeHandle` / `runOneTurn()` | P2, P3 | do laço do runtime | `null` / `-1` / `UnsupportedError` |
| `SyncNativeCallable<T>` | P4 | entrega ao dono e espera | `isSupported == false` |
| `GarbageCollector.deferUntil`, `GarbageCollector.collectWhenIdle`, `@pragma('dartforge:no-gc')` | P8 | gatilho do coletor | sem efeito |
| `using(resource, body)` | P9 | liberação no fim do escopo; com ARC, imediata | `try/finally` |
| `Mutex`, `ConditionVariable`, `Atomic<Int64>`, `NativeMemory.view` | P7 | intrínsecos atômicos do LLVM | FFI para pthread/SRWLOCK |
| `MappedMemory.open` | P17 | `mmap` no runtime | `mmap` por FFI |
| `NativeLibrary.openFirst` / `hasSymbol` | P16 | `dlopen`/`dlsym` do runtime | `DynamicLibrary` |
| `Isolate.current.threadId` (extensão) | P6 | id da thread | `null` |

**Implementação no DartForge:**
- **Lowering:** o pacote declara funções `external` com
  `@pragma('dartforge:intrinsic', 'name')`, e o lowering do DartForge as troca
  por chamadas ao runtime, como já faz com os natives do SDK.
- **Fallbacks:** os corpos de fallback ficam num `part` que só a VM usa,
  escolhido por `isDartForge`.
- **Validação:** os testes do pacote rodam nos dois mundos, na VM pelo
  `dart test` e no DartForge pelo corpus nativo.

## 5. Ordem sugerida e esforço

| fase | itens | esforço | efeito no dart_ui |
|---|---|---|---|
| 1 | detecção AOT; pragmas de inline; `--subsystem=gui` | implementados; validação abaixo | base para o resto; ganho de desempenho ainda não medido |
| 2 | bomba nativa no laço (macOS CFRunLoop e fila principal, Windows mensagens, Linux fds); `nextDeadline` e `runOneTurn` | 1–2 semanas | base para eliminar polling; AppKit e reentrância ainda exigem trabalho (P1, P2, P3) |
| 3 | `SyncNativeCallable` e `close()` seguro | 1–2 semanas | delegates AppKit com retorno, UIA e COM (P4, P5) |
| 4 | `GarbageCollector.deferUntil`, `sem-coleta` e diagnóstico FFI no `SIGSEGV` | 1 semana | quadros sem pausa e crash com nome (P8, P12) |
| 5 | primitivas de concorrência, `mmap`, bibliotecas | 1–2 semanas | P7, P16, P17 |
| 6 | ARC competitivo; vetorização pedida | em curso | P8, P9 e P10 no limite |

**Medidas para cada fase:** o dart_ui já tem as sondas.
- Para P2: a POC-19 dá timer efetivo, latência p95 e wakeups.
- Para P1: as medidas de popup e de input ponta a ponta do macOS.
- Para P8: o tempo de quadro do player 1080p.
- Para P10: o rasterizador de strips e o YUV→RGB.

Rodar as mesmas sondas compiladas pelo DartForge dá o antes e depois de cada
fase.

## 6. Riscos e limites

- **Duas semânticas para o mesmo código.** O dart_ui precisa de testes nos dois
  mundos. Os pacotes e pragmas só podem ser dicas ou APIs com `isSupported`;
  nunca mudar silenciosamente o significado de código Dart comum.
- **Reentrância (P3) e callback síncrono (P4)** abrem portas para deadlock e para
  estados intermediários visíveis. Cada um precisa de recusa explícita (erro
  claro) quando a condição não puder ser garantida.
- **O DartForge ainda não compila o dart_ui inteiro.** São 907 arquivos e ~394
  mil linhas, com muito `dart:ffi`. O primeiro passo prático é compilar as POCs
  e sondas do dart_ui (POC-19, `nested_callback_probe`, os testes de FFI) com o
  DartForge e corrigir o que faltar. Esse trabalho já paga o resto.
- **ARC:** a liberação determinística de P9 e o fim das pausas de P8 dependem de
  o ARC ficar competitivo. Hoje ele está 2,7–5,2× atrás do tracing. Até lá, o
  caminho é o tracing com `GarbageCollector.deferUntil`.

## 7. Atualização de estado e prioridades — 2026-10-13

Esta seção atualiza e, quando houver divergência, substitui as recomendações
de estado, desenho de APIs e ordem de implementação das seções anteriores.
O texto de 12/10 permanece como registro da proposta inicial.

A revisão foi feita por leitura do código atual dos dois projetos e dos
contratos públicos do Dart. **Não foram executados novos benchmarks nem uma
compilação completa do dart_ui nesta revisão.** Presença de código e testes
no repositório não equivale a validação ponta a ponta do framework.

### 7.1 O que já existe e saiu da lista de implementação

| item da proposta inicial | estado encontrado no código atual | evidência no DartForge |
|---|---|---|
| constante `dartforge.compiler` | reconhecida como definição reservada no lowering nativo; consulta booleana resulta em `true` | `crates/emit_native/src/lower/environment.rs` |
| `vm:prefer-inline` e `vm:never-inline` | reconhecidas no lowering e traduzidas para `inlinehint` e `noinline`; a preferência tem restrições nos caminhos de depuração | `crates/emit_native/src/lower/mod.rs`, `crates/emit_native/src/llvm/mod.rs` e testes em `llvm/tests.rs` |
| subsistema GUI no Windows | opção `--subsystem=gui` implementada no CLI e no ligador, com testes de validação de argumentos | `crates/cli/src/native.rs`, `crates/emit_native/src/linker_windows.rs`, `crates/cli/tests/subsystem_cli.rs` |

Esses itens precisam ser verificados nas cargas do dart_ui, mas não devem ser
tratados como funcionalidades ainda ausentes. Os nomes de arquivos, símbolos
e opções em português citados anteriormente também devem ser conferidos contra
a migração do código para inglês.

O dart_ui ganhou um **quarto backend macOS, `appkitEmbedded`**, em 13/10/2026.
Ele já implementa janelas e delegates AppKit diretamente no mesmo processo,
usando um runtime Dart AOT compilado do SDK com patch. Referências:

- `doc/MACOS_TRES_BACKENDS.md`, seção "Backend 4";
- `lib/src/backends/macos/embedded/embedded_backend.dart`;
- `lib/src/backends/macos/embedded/embedded_runtime.dart`;
- `tool/mobile/dart_sdk_patches/3.13.5-macos.patch`.

Esse runtime modificado não é a VM upstream padrão. O código detecta os
recursos e mantém os outros caminhos para o runtime oficial. Para o DartForge,
a oportunidade é **adaptar e aproveitar esse backend**, em vez de iniciar
outra implementação AppKit. Não há autorização técnica para remover os
backends usados pelo upstream somente porque o caminho DartForge funciona.

### 7.2 Nova ordem de prioridades

| prioridade | implementação | problemas relacionados | resultado esperado e condição de aceite |
|---|---|---|---|
| 1 | corpus diferencial com recortes reais do dart_ui | transversal | descobrir e corrigir bloqueios de compilação e divergências; comparar saídas, pixels, ordem de eventos e descarte com o Dart oficial |
| 2 | integração do laço do runtime com AppKit, Win32 e Linux, começando pelo macOS | P1, P2, P6 | aproveitar `appkitEmbedded`; atender timers, mensagens Dart e input nativo com wakeup correto, sem polling periódico no caminho integrado |
| 3 | otimização dos kernels existentes de listas tipadas, SIMD e FFI | P10, P17 | melhorar conversão de vídeo e rasterização mantendo o mesmo código Dart e resultados compatíveis |
| 4 | diagnóstico de FFI e callbacks | P5, P12, P13 | identificar chamada, thread e ciclo de vida envolvidos nas falhas; manter os contratos upstream de callbacks existentes |
| 5 | turnos aninhados e callback síncrono entre threads por API específica | P3, P4 | permitir progresso durante resize e menus modais e atender delegates com retorno; contratos de reentrância, espera e encerramento testados |
| 6 | adiamento limitado de GC durante quadros | P8, P9 | reduzir picos de latência com orçamento explícito de tempo e memória, sem prometer ausência absoluta de pausas |

Primitivas de concorrência, mapeamento de arquivos, helpers de bibliotecas e
empacotamento continuam úteis, mas entram quando uma carga concreta exigir.
O dart_ui já possui bindings de sincronização nativa; validar e reaproveitar
esse código evita duplicar APIs antes de demonstrar a necessidade.

As estimativas de semanas da seção 5 não foram revalidadas. Reentrância e
callbacks entre threads têm riscos que impedem tratar o trabalho como simples
exposição de funções já existentes.

### 7.3 Primeiro passo: compatibilidade executável

Criar sondas pequenas, derivadas do código atual da `main` do dart_ui, para:

- memória FFI, alinhamento, structs e assinaturas nativas;
- callbacks locais e listeners, exceções e fechamento;
- listas tipadas internas e externas, views e cópias sobrepostas;
- timers, microtasks, mensagens e encerramento de isolates;
- conversão de vídeo e rasterização de strips.

Comparar VM JIT, AOT oficial e DartForge com o mesmo código-fonte e entradas.
Separar validação de correção de medição de desempenho. Fixar e registrar
SDK, arquitetura, flags e versões usadas; o alvo inicial do DartForge é Dart
3.6.2, enquanto o backend AppKit embutido cita um patch para 3.13.5.

Para renderização, conferir bytes/pixels segundo o contrato do algoritmo;
para assincronismo, ordem dos eventos; para recursos, descarte e encerramento.
Tracing e ARC devem passar separadamente pelos mesmos contratos. Não trazer
`poc/` ou `mvp/` do branch de exploração para a `main`: usar as sondas e os
testes atuais, ou extrair novos recortes mínimos.

### 7.4 Laço nativo: a thread principal é necessária, mas não suficiente

No AOT, `dartforge_init` chama a entrada na thread que entrou no runtime, e
os isolates criados usam threads dedicadas. Isso favorece a integração nativa,
mas não demonstra, sozinho, que uma aplicação AppKit completa funciona.

O laço atual em `crates/runtime/src/events.rs` espera mensagens e deadlines
pelo mecanismo interno de `ports.rs`. Para integrar o backend existente:

1. Criar um adaptador de runtime para o backend detectar o DartForge e pedir
   o início do loop da aplicação, sem apresentar funções próprias como se
   fossem APIs oficiais de embedding do Dart.
2. Acordar a espera nativa quando chegam mensagens Dart, com protocolo que
   não perca notificações na transição entre verificar a fila e dormir.
3. Atender timers e mensagens Dart enquanto AppKit administra a aplicação,
   inclusive nos modos de menus e resize. O patch existente registra uma
   fonte em `kCFRunLoopCommonModes` e integra `[NSApp run]`.
4. Limitar eventos nativos por passagem para que uma enxurrada de input ou
   resize não impeça o progresso do trabalho Dart.
5. Definir o que mantém o processo vivo enquanto há uma aplicação nativa
   ativa e como a parada libera fontes, wakeups e recursos.

Uma chamada isolada a `CFRunLoopRunInMode(kCFRunLoopDefaultMode, ...)` não deve
ser apresentada como solução completa para o ciclo de vida AppKit ou loops
modais. A política de microtasks e eventos precisa conservar os contratos
Dart; reentrância adicional entra explicitamente pelo recurso da prioridade 5.

### 7.5 Ganhos do compilador sem código exclusivo

Medir primeiro os kernels atuais do dart_ui:

- `lib/src/graphics/video/video_color_conversion.dart`;
- `lib/src/rendering/gpu/vector/native_strip_rasterizer.dart`.

Priorizar eliminação de boxing, especialização de listas tipadas, remoção
de checagens de limites quando provada e vetorização. Produzir relatório de
otimização com os laços vetorizados e os motivos de recusa.

Preservar arredondamento, NaN, overflow, ordem de operações e efeitos
observáveis. Não habilitar transformações de ponto flutuante que alterem o
contrato silenciosamente. Em cópias, preservar a semântica de sobreposição;
`memcpy` não é substituto universal para `setRange`.

Os números antigos de GC e partida não são uma previsão de ganho do DartForge.
O próprio roteiro do dart_ui registra redução de alocações e de custo de GC
após reaproveitamento de buffers. Medir a versão atual e comparar partida em
condições equivalentes: `dart run` inclui custos que um executável AOT não tem.

### 7.6 Correção do desenho de `package:dartforge`

**Um `if` constante não seleciona arquivos `part` e não fornece uma
implementação para uma função `external`.** Portanto, o esquema da seção 4
de funções `external` com corpos em um `part` "que só a VM usa" precisa ser
substituído por um mecanismo concreto compatível com o toolchain oficial.

Preferir métodos com corpo Dart válido: fallback funcional quando possível,
ou `UnsupportedError` quando o recurso estiver indisponível. O DartForge pode
reconhecer essas declarações e substituir a implementação por intrínsecos.
O reconhecimento deve considerar a identidade da biblioteca, a declaração
e a versão do contrato, não apenas uma pragma arbitrária com o mesmo nome.

Expor capacidades específicas, como `supportsNativePump`,
`supportsNestedTurns` e `supportsSyncCallbacks`. `isDartForge` identifica o
compilador; não prova a disponibilidade de todos os recursos no runtime,
na plataforma ou no isolate atual.

Usar `@pragma('dartforge:...')` como dica para ferramentas, sem transformar
uma otimização em requisito oculto de correção. A documentação oficial orienta
que ferramentas ignorem prefixos de outras ferramentas:
[contrato de pragma](https://api.dart.dev/dart-core/pragma-class.html).

### 7.7 Callbacks: preservar o contrato e definir as extensões

Manter o comportamento upstream de `NativeCallable.isolateLocal`: uma chamada
de outra thread aborta o processo. O callback síncrono entre threads deve ser
uma API separada, com capacidade explicitamente consultável:
[contrato oficial](https://api.dart.dev/dart-ffi/NativeCallable/NativeCallable.isolateLocal.html).

O runtime não conhece todas as esperas e locks de bibliotecas externas, então
não pode prometer detecção geral de deadlocks. Antes da implementação, definir:

- condições em que o isolate dono pode atender o pedido;
- política de espera, cancelamento e retorno excepcional;
- encerramento do isolate com pedidos pendentes;
- validade dos ponteiros e argumentos durante toda a execução;
- comportamento quando a espera termina mas o handler já começou.

Um timeout não autoriza liberar argumentos ainda usados pelo handler.
Turnos aninhados também precisam de contrato explícito sobre trabalho elegível,
profundidade de reentrância, raízes do coletor, exceções e estado intermediário.
Não prometer detectar genericamente um `await` dependente do quadro externo.

Para chamadas após `close()`, conservar apenas a memória executável do
trampolim é insuficiente: contexto, identidade e sincronização também precisam
continuar seguros. Uma política de retenção/quarentena deve explicar seu custo
de memória e evitar reutilização que encaminhe uma chamada tardia para outro
callback. A proteção adicional do DartForge não muda a regra portátil:
chamadas após `close()` são comportamento indefinido no upstream.
[Contrato oficial de close](https://api.dart.dev/dart-ffi/NativeCallable/close.html).

### 7.8 GC e recursos nativos

Adiamento de GC deve ser uma solicitação limitada por tempo e bytes alocados,
com restauração da política em exceções, tratamento de regiões aninhadas e
possibilidade de coleta emergencial. É uma ferramenta para reduzir picos de
latência, não uma garantia de "quadros sem pausa".

ARC não substitui `dispose`/`release` para recursos de UI: ciclos, ordem de
descarte e exigências de thread permanecem. O `NativeFinalizer` upstream pode
executar em uma thread arbitrária e não permite reentrar normalmente na VM.
Não liberar objetos AppKit ou COM apenas supondo que o finalizador roda no
dono. Usar descarte explícito e, quando necessário, uma fila de liberação na
thread correta. [Contrato oficial de NativeFinalizer](https://api.dart.dev/dart-ffi/NativeFinalizer/NativeFinalizer.html).

Helpers de escopo com `try/finally` são úteis nos dois runtimes. Se o helper
aceitar um corpo assíncrono, precisa aguardar sua conclusão antes de liberar
o recurso; uma versão síncrona não deve aparentar oferecer essa garantia.

### 7.9 Primeiro marco de integração

**Uma janela usando o backend AppKit já existente, compilada pelo DartForge,
com timer, input, mensagens entre isolates e fechamento correto; o mesmo
programa continua pelo fallback no AOT oficial.**

O aceite inclui ausência de wakeups perdidos e progresso das mensagens e
timers em operação normal. Progresso durante menus e resize deve ser validado
como parte do contrato de loops aninhados, sem ser presumido pela primeira
janela aberta. Registrar startup, latência de input, atraso de timers,
wakeups ociosos e descarte, distinguindo medidas de correção.

Depois desse marco, medir conversão de vídeo e rasterização com as mesmas
entradas nos dois compiladores. A compatibilidade desejada é de código-fonte
e contratos públicos Dart; não implica que binários, snapshots, objetos do
heap ou bibliotecas internas dos dois runtimes sejam intercambiáveis.

## 8. Android e iOS: compilação, runtime embutido e empacotamento — 2026-10-13

Esta seção amplia as prioridades para mobile, com base no estado documentado
e no código dos dois projetos. **Os comandos abaixo são propostas, não
funcionalidades disponíveis.** Não houve compilação ou teste mobile do
DartForge nesta revisão.

### 8.1 Objetivo: gerar o aplicativo completo

O DartForge poderia oferecer a experiência de um "compile para iOS/Android",
produzindo código AOT, runtime, recursos e configuração da plataforma em um
único fluxo. A interface sugerida distingue a construção de um aplicativo
da emissão de um executável avulso:

```sh
# iOS: aplicativo para simulador
dartforge build ios example/hello_world.dart --simulator

# iOS: aplicativo para aparelho, com assinatura configurada
dartforge build ios example/hello_world.dart --device

# Android: APK para desenvolvimento
dartforge build android example/hello_world.dart --format apk

# Android: AAB para distribuição
dartforge build android example/hello_world.dart --format aab
```

No dart_ui, a mesma escolha poderia ser exposta pelo comando de build:

```sh
dart_ui build example/hello_world.dart --ios --compiler dartforge
dart_ui build example/hello_world.dart --android --compiler dartforge
```

Essas interfaces ainda precisam de desenho e implementação. A configuração
do projeto deve informar identificador do aplicativo, versão mínima do SO,
arquiteturas, assets, permissões e assinatura. O compilador cuida da geração
de código e do runtime; o empacotador e os adaptadores do dart_ui cuidam da
montagem do aplicativo e da integração com cada plataforma.

### 8.2 Correção de estado: LLVM não significa cross-compilation pronta

A afirmação da seção P14 de que a compilação cruzada já está resolvida pelo
LLVM deve ser corrigida: `crates/emit_native/src/target.rs` declara que o alvo
é o próprio host e enumera Windows, Linux e macOS. Há decisões baseadas em
`cfg!` do compilador e caminhos de ligação específicos do host.

O LLVM oferece a infraestrutura de geração de código, mas falta implementar
a seleção explícita de alvo e propagá-la por ABI, layout de dados, ligador,
SDK/sysroot, runtime e caches. Android não pode ser tratado como Linux glibc,
e iOS não pode ser tratado como macOS. Aparelho e simulador iOS também são
alvos distintos, mesmo quando ambos usam arm64.

Primeira entrega estrutural: uma configuração de alvo que separe host de
destino e inclua arquitetura, SO, ABI, versão mínima e SDK. As chaves de cache
precisam incorporar essas escolhas e a versão do runtime, evitando reutilizar
objetos compilados para outro destino. Os artefatos Rust do runtime também
precisam ser construídos para o destino.

### 8.3 iOS: aproveitar UIKit e substituir o runtime upstream modificado

Hoje o dart_ui usa um `dartaotruntime` compilado para iOS com patch, mais um
snapshot Mach-O em `Frameworks/App.framework/App`. O backend Dart registra
delegates e views e fala com UIKit por FFI. As referências no dart_ui são:

- `doc/ANDROID_IOS.md`;
- `doc/RELATORIO_DART_PURO_ANDROID_IOS.md`;
- `doc/adr/0010-dart-aot-embutido-android-ios.md`;
- `lib/src/backends/ios/ios_backend.dart`;
- `tool/ios/build_app.dart` e `tool/mobile/dart_sdk_patches/`.

O DartForge poderia gerar o AOT e ligar seu runtime diretamente ao aplicativo,
aproveitando esse backend UIKit. Para builds DartForge, isso substituiria o
fluxo kernel → `gen_snapshot` → runtime upstream modificado. O aplicativo
continuaria tendo um runtime, agora o do DartForge.

Entregas necessárias:

1. Emitir e ligar código para iOS arm64 e iOS Simulator arm64, com a plataforma
   e a versão mínima corretas no Mach-O desde a geração.
2. Portar o runtime e validar FFI, exceções, GC/ARC, TLS, timers, isolates,
   `dart:io` e identificação da plataforma (`Platform.isIOS`).
3. Integrar o isolate de UI à thread principal e ao `CFRunLoop`, incluindo a
   entrada em UIKit e a entrega de mensagens nos modos relevantes.
4. Agendar o atendimento de microtasks após callbacks nativos em pontos
   seguros, sem drenar arbitrariamente a fila dentro de uma pilha FFI ativa.
   O backend atual usa uma mensagem vazia para acordar esse trabalho.
5. Garantir que os callbacks FFI usados em produção funcionem em AOT sem
   depender de geração de código executável em runtime; auditar os trampolins
   atuais e prever stubs pré-compilados ou outra solução compatível.
6. Gerar `.app`, `Info.plist`, assets e símbolos de depuração, integrando
   assinatura, provisioning e exportação para distribuição.

Xcode, SDKs Apple e requisitos de assinatura permanecem no fluxo. Um `.app`
que roda no simulador não demonstra instalação em aparelho nem distribuição
pela loja. Referência oficial:
[distribuição para dispositivos registrados](https://developer.apple.com/documentation/xcode/distributing-your-app-to-registered-devices).

### 8.4 Android: biblioteca dentro do processo da Activity

O caminho documentado hoje usa uma Activity Java que inicia o runtime Dart
em processo separado; entrada por socket e quadros em buffers compartilhados.
O DartForge poderia gerar uma biblioteca com o código AOT e seu runtime:

```text
APK
 ├─ Activity e integração Android
 ├─ lib/arm64-v8a/libdartforge_app.so
 │    └─ código Dart AOT + runtime DartForge
 └─ assets
```

O primeiro passo recomendado é manter a Activity existente e substituir a
comunicação entre processos pela integração com essa biblioteca. Isso permite
reaproveitar teclado e lifecycle já implementados. NativeActivity ou
GameActivity podem ser avaliadas depois, conforme as necessidades do backend.

Dentro do processo, o adaptador pode entregar uma `ANativeWindow` ao backend,
abrindo caminho para Vulkan ou OpenGL ES diretamente na superfície do app.
**Gerar a biblioteca não implementa a apresentação GPU:** o backend ainda
precisa cuidar da superfície, sincronização e recursos gráficos. Começar com
desenho CPU permite validar o runtime sem depender dessa etapa.

Entregas necessárias:

1. Suportar Android/bionic, NDK, API mínima e ABIs escolhidas, começando por
   arm64-v8a; incluir x86_64 quando necessário para emuladores.
2. Emitir biblioteca compartilhada com pontos de entrada de embedding e
   contrato de inicialização/parada, em vez de depender de um `main` de
   executável desktop. Conferir `Platform.isAndroid` e os contratos de FFI.
3. Integrar filas Dart e wakeups ao mecanismo de eventos Android. Escolher
   explicitamente a thread dona do isolate e a ponte com a UI thread Java;
   entrar pela JNI não autoriza bloquear o loop da Activity.
4. Definir a propriedade de referências JNI e de recursos nativos. `JNIEnv`
   pertence à thread; callbacks e trabalho em outras threads exigem tratamento
   correto de attachment e referências.
5. Tratar criação/destruição de superfície, pausa, retomada e recriação da
   Activity sem reusar ponteiros de uma instância anterior.
6. Validar páginas de memória de 16 KB no ligador, empacotamento, alocador,
   memória mapeada e em todas as bibliotecas nativas distribuídas.
7. Produzir APK/AAB com manifesto, assets, assinatura e símbolos de depuração.

As APIs Java/JNI continuam úteis para teclado, acessibilidade e serviços do
Android. Referências oficiais:
[conceitos do NDK](https://developer.android.com/ndk/guides/concepts),
[cuidados com JNI](https://developer.android.com/ndk/guides/jni-tips) e
[suporte a páginas de 16 KB](https://developer.android.com/guide/practices/page-sizes).

### 8.5 Prioridades e critérios de aceite mobile

Esta sequência detalha a frente mobile e compartilha com a seção 7 o corpus
diferencial, a integração de event loops e os contratos de callbacks. Não
pressupõe que todo o trabalho desktop precise terminar antes do primeiro
experimento mobile.

| etapa | entrega | critério de aceite |
|---|---|---|
| 1 | separar host e alvo no compilador e nos caches | IR, objetos, bibliotecas e runtime usam o destino correto; rejeição clara de combinações não suportadas |
| 2 | portar o runtime para iOS e Android | sondas de FFI, listas tipadas, exceções, GC, timers e isolates executam no destino; ARC validado separadamente se oferecido |
| 3 | primeira janela no iOS Simulator com o backend UIKit atual | inicialização na thread principal, quadro visível, timer e mensagens Dart progredindo, sem geração de código em runtime |
| 4 | Android dentro do processo da Activity, inicialmente CPU | biblioteca carregada, quadro e input funcionando sem processo Dart filho; lifecycle básico correto |
| 5 | testes de interação e lifecycle em aparelhos | toque, teclado, pausa/retomada, recriação de superfície e descarte; condições de memória e threads verificadas |
| 6 | automatizar empacotamento e distribuição | `.app`, APK/AAB e assinatura reproduzíveis, símbolos preservados à parte e instalação validada nos destinos previstos |
| 7 | GPU direta e otimização | apresentação correta e medidas comparáveis de partida, memória, tempo de quadro e consumo em ociosidade |

O empacotamento mínimo é necessário já nas etapas 3 e 4; a etapa 6 o transforma
em ferramenta de uso regular e distribuição. Sucesso no simulador ou emulador
não substitui a etapa de aparelhos. Publicação em lojas é um marco separado
de compilar, instalar e executar.

### 8.6 Compatibilidade upstream e distribuição dos artefatos

Manter a compatibilidade de código-fonte e dos contratos públicos Dart. Os
adaptadores de plataforma escolhem os recursos disponíveis, preservando os
caminhos atuais com o toolchain Dart. Isso não significa que o SDK upstream
padrão já forneça todo o fluxo mobile: hoje o dart_ui ainda precisa de builds
e patches próprios nesse caminho.

O DartForge deve fornecer seus próprios artefatos mobile versionados e
verificados, para que o desenvolvedor de aplicativos não precise compilar o
runtime a cada instalação. Código gerado, bibliotecas do SDK e runtime precisam
ter ABI compatível entre si; distribuir artefatos prontos desloca essa gestão
para o toolchain, mas não elimina a necessidade de versionamento.

O benefício central é um build mobile reproduzível, com o runtime integrado
ao SO e o mesmo aplicativo Dart. Ganhos de tamanho, partida, memória e quadros
devem ser medidos contra o caminho AOT atual do dart_ui, em condições
equivalentes, sem usar o custo de `dart run` como referência de execução AOT.

## 9. Entrega da base nativa — 2026-10-13

Três subagentes implementaram em paralelo as frentes da fase 1, com revisão e
validação integradas na `main`. Detecção AOT, pragmas de inline e subsistema
Windows GUI estão no código; o pacote DartForge e as APIs do laço continuam
propostas. O relatório agora distingue a bomba CFRunLoop da distribuição de
`NSEvent` e da reentrância modal: a primeira não elimina sozinha os backends.

**Limites da detecção:** não há `-D` geral no CLI. Os nomes literais,
concatenações, aliases const locais/importados e formas comuns de interpolação
e condição booleana são reconhecidos. Nomes const cujo cálculo o lowering
não conhece dão diagnóstico (`constant compilation environment name`),
inclusive em consultas aninhadas; não retornam silenciosamente false.
A detecção só foi validada no AOT. O fallback na VM usa o próprio SDK.

**Validação:** 487 testes do emissor, três da CLI e um teste opcional de
ligação cruzada passaram. O último verifica subsistemas PE 2/3, ponto de
entrada válido e ligação com os CRTs GUI/console; não executa Windows neste
Mac. Há 30 execuções AOT iguais à VM para detecção, inline, JSON inválido,
buffers Socket e o código real de composição de pixels do dart_ui, em
tracing/ARC/ARC preparado e com/sem GC stress e efeitos/RC conferidos.
As regressões persistentes são `140_compiler_environment.dart` (com biblioteca
importada) e `141_inline_pragmas.dart`.

Do dart_ui na `main`, `tool/isolate_frame_handoff_probe.dart` compilou e
executou os quatro caminhos e confirmou escrita compartilhada em 200 rodadas.
O probe portátil `repro/dart_sdk_isolate_event_loop/bin/nested_callback_probe.dart`
compilou e executou: zero ticks durante ~1.505 ms em `qsort`, assim como na VM.
Após retornar foram 34 ticks no DartForge e quatro na VM; investigar o
catch-up periódico em P2. A variante em `tool/` é Windows-only; as POCs
arquivadas não foram restauradas. Tempos de frames desta execução durante
builds não são medidas comparativas. Ganho de inline ainda não foi medido.

O próximo passo é integrar wake+CFRunLoop de forma opt-in no macOS, com
sondas de thread principal, fila principal, corrida de wake, precisão de
timer e CPU ociosa. A fonte persistente precisa sobreviver aos emissores de
mensagens e ser retirada com sincronização; nenhuma espera ou callback nativo
pode ocorrer segurando locks da fila ou empréstimos do heap/eventos.
O estado/raízes de mensagem precisam de uma pilha antes de P3. Um limite de
tempo não interrompe um callback já em execução nem o dreno de microtasks.
Detecção completa de ciclos envolvendo Futures e joins/mutexes estrangeiros
não existe hoje e exige um protocolo adicional.

Dados em `bench/results/2026-10-13-dart-ui-phase1/validation.json`;
logs/fontes locais em `target/dart-ui-evidence/phase1/`, no SSD.

## 10. Corpus diferencial e correção de timers — 2026-10-13

Seguindo a prioridade 1 da revisão §7, duas sondas importam os kernels atuais
da `main` do dart_ui: `bench/dart_ui/video_probe.dart` e `strip_probe.dart`.
O runner `scripts/verify-dart-ui.py` compara VM JIT, AOT oficial e AOT
DartForge tracing/ARC/ARC preparado, com/sem GC stress; preserva a configuração
do framework e registra revisão, hashes das fontes e flags. São recortes
pequenos com resultados verificáveis, não uma compilação completa do framework.

Vídeo verifica layouts NV12/I420/YUY2, espaços/faixas, offsets/strides, crop,
reutilização e formatos empacotados. Strips verifica 11 cenas, incluindo
curvas, clipping, winding, sobreposição e transformação, emitindo todos os
pixels e registros/alphas ativos. As saídas de 9.067 e 33.201 bytes foram
idênticas nas 12 execuções nativas e nos dois oráculos. O modo de benchmark
de strips é separado e inclui checksum de toda a saída ativa; seu smoke
confirmou os mesmos 382 quads/checksum 948835220 e zero crescimento. Ainda
não há medida controlada de ganho de desempenho.

O corpus também levou à correção do timer periódico: atrasos maiores que
um período pulam os períodos vencidos antes do callback e avançam `Timer.tick`,
seguindo a regra do SDK 3.6.2; o reagendamento preserva a fase, e período zero
não pula ticks. As regressões nativas `142`/`143` deram 12 execuções iguais
à VM/AOT oficial, cobrindo callback nativo bloqueado, callback lento,
microtasks, cancelamento e período zero. Quatro testes Rust cobrem deadlines
e o limite estrito atraso > período sem depender do relógio de parede.

No probe real `qsort`, a entrega após o bloqueio passou de 34 callbacks para
quatro nesta execução; dentro do callback continuaram zero. Não se trata de
P3 resolvido. A espera CFRunLoop e a adaptação a `appkitEmbedded` ainda não
foram implementadas. A arquitetura de alvo e os builds mobile da seção 8
continuam propostas, sem código emitido para iOS/Android.

Gates: 494 testes do emissor e 349 do runtime passaram, incluindo os ignorados
por padrão, sem falhas. HTTP teve 1.100 respostas JSON corretas sob GC stress
em um smoke funcional. Evidências em
`bench/results/2026-10-13-dart-ui-corpus/validation.json`; instruções do runner
em `bench/dart_ui/README.md`. Nenhuma mudança foi feita no repositório dart_ui.

## 11. Pacote único e espera nativa macOS — 2026-10-13

`packages/dartforge` reúne SIMD, as quatro anotações do desenho ARC e a
primeira capacidade de espera nativa. `packages/dartforge_simd` foi removido
por pedido do proprietário; consumidores locais migraram para
`package:dartforge/simd.dart`. A extensão mantém o mesmo código e o LLVM
continua emitindo vetores nativos.

`PreferRegion`, `PreferAcyclic`, `NoEscape` e `Realtime` são metadata const
portátil com esquema 1. Raiz e `annotations.dart` exportam as mesmas declarações;
a especificação identifica a biblioteca definidora `src/annotations.dart`.
Alvos expressáveis recebem feedback de `meta`; variável local não é um desses
alvos. Ainda não há reconhecimento/prova dos contratos no compilador:
a API não certifica região, ausência de escape ou RT.

No macOS, `EventLoop.supportsNativePump` permite escolha por capacidade.
`enableNativePump()` integra espera CFRunLoop/fila principal ao laço;
`disableNativePump()` libera o keep-alive. Operações idempotentes, na thread
principal, com fonte persistente e wake de mensagens/timers/microtasks.
O runtime solta locks/empréstimos antes da espera; sincroniza a retirada com
notificadores e adia a última liberação até sair do callback CF.
VM/outros sistemas retornam capacidade falsa. `supportsNestedTurns` e
`supportsSyncCallbacks` permanecem falsos. NSEvents continuam sob a aplicação;
adaptação ao `appkitEmbedded`, CPU ociosa e latência ainda não foram verificadas.
Não há limite para callback longo, dreno de microtasks ou lote CF.

Para áudio em thread estrangeira falta a entrada AOT preparada do §36.4:
kernel e wrapper verificados, contexto/buffers preparados, domínio explícito
e quiescência antes do descarte. Listener assíncrono e espera pelo isolado
não satisfazem `dsp-v1`; `Realtime` sozinho não cria a ponte. P3/P4/P5 e
certificação de áudio continuam trabalhos separados.

Novo recorte FFI importa alocador, arena, mutex e layouts reais do dart_ui,
incluindo views sobrepostas, strings, callbacks com erro e ciclo de vida.
FFI/vídeo/strips deram 18 execuções nativas iguais à VM/AOT oficial; a bomba
teve 12 execuções válidas de wake, afinidade e encerramento. Metadata passou
nos seis modos e SIMD preservou saída/IR vetorial. Corrigido contrato de
ownership ausente de `Timer.tick` no SDK preparado; contratos escalares da
bomba também são explícitos, com efeitos conservadores e verificador ativo.
496 testes do emissor e 353 do runtime passaram, incluindo os ignorados.
Dados em `bench/results/2026-10-13-dartforge-package/`; sem nova afirmação de
ganho de desempenho nem alteração no repositório dart_ui.

## 12. Dispatcher, AppKit real e contratos A0 — 2026-10-13

O pacote agora registra um dispatcher de eventos nativos, separado da bomba:
callback limitado no turno externo Dart após CF/microtasks/controle, protegido
por root persistente e frame ativo. Pode remover/substituir-se ou desligar a
bomba. Não permite processamento reentrante de eventos Dart.

O dart_ui recebeu hook opcional de lançamento no backend embedded e cleanup
de inicialização parcial, preservando o padrão do SDK modificado. O probe
real inicia NSApplication via `finishLaunching`, confirma delegate, evento
sintético, timer/mensagem e shutdown nos seis modos. Falha injetada também
limpa delegate/sink e preserva o erro antes de inicializar backend novo.
Ainda não é prova de janela, rendering, input, menu/resize ou progresso modal.

Anotações agora são normalizadas por identidade/constantes/esquema/alvos,
incluindo aliases locais e rejeição de autoridade por homônimos/ambiguidade.
CLI AOT/IR tem `--dartforge-contracts=verify|ignore`: padrão recusa obrigações
NoEscape/Realtime não provadas; checkers completos de emissão ainda ausentes.
Preferências são `not_applied`; ignore produz manifestos explicitamente sem
certificação, com dependências/política/artefato e nomes sem colisão.
Nenhuma prova de região, aciclicidade, escape ou áudio é inferida de A0.

`Isolate.onEvent`/`handleEvent` não foram implementados no DartForge. São
relevantes para o controle externo de P2, incluindo timers e um evento por
chamada; a bomba administrada pelo runtime tem outro contrato. A patch SDK
3.13.3 lança UnsupportedError e a superfície experimental saiu do main
([#64285](https://github.com/dart-lang/sdk/issues/64285)); o DartForge usa
SDK 3.6.2 com overlay. P3 e a ponte preparada de áudio seguem pendentes.

Evidências em `bench/results/2026-10-13-dartforge-package/`, incluindo provas
de execução do dispatcher e de erro não capturado fora do wrapper FFI.
CPU ociosa foi apenas observada em intervalos curtos, com resolução do `ps`;
não é demonstração de custo zero ou ganho pareado de desempenho.

## 13. APIs portáveis de recursos e arquivos — 2026-10-13

P9 agora tem `ScopedResource`, `ResourceHandle`, `using` síncrono e
`usingAsync` aguardado, com liberação em finally. Erro de cleanup prevalece,
como no Dart comum; corpo não pode reter o recurso nem usar async no helper
síncrono. Não há `@scoped` verificado ou mudança do ARC.

P16 tem `NativeLibrary.openFirst`, `hasSymbol`, lookup e close idempotente;
preserva os erros de todos os candidatos quando falham. Usa o DynamicLibrary
existente nos dois runtimes; ligação estática de `@Native` continua pendente.

P17 tem `MappedMemory.open` via FFI para hosts de 64 bits macOS/Linux/Windows.
Arquivos existentes, offsets/intervalos conferidos, views sem cópia, readonly,
flush e liberação explícita. `MappedBytes` é uma fachada revogável por acesso;
`withBytes` oferece a Uint8List externa exigida pelos consumidores existentes,
com contrato de empréstimo síncrono sem escape. O callback não pode reter view,
buffer ou derivados; views brutas escapadas não podem ser revogadas.
Truncamento externo e escritores concorrentes requerem controle do aplicativo.

As três sondas deram 18 execuções nativas iguais à VM/AOT oficial no macOS.
Windows/Linux ainda não tiveram execução. Registro separado em
`resources-and-mapping-validation.json`; runner
`scripts/verify-package-resources.py`. Isso é evidência funcional, sem ganho
medido de desempenho ou certificação de escape/realtime.

## 14. P5 e análise HIR — continuação de 2026-10-13

AOT agora mantém contexto/trampolim de callback fechado como tombstone.
Chamadas tardias não decodificam argumentos nem entram no heap/isolado:
retornam exceptionalReturn escalar, descartam void/listener ou zeram structs.
O root Dart é solto; a memória nativa publicada permanece até o fim do processo,
com crescimento por callback criado. Teardown publica o tombstone antes de
onExit; finalização principal preserva callbacks durante finalizers/toString.

18 casos nativos de ciclo de vida e seis de retorno composto passaram em
macOS arm64, tracing/ARC/preparado, normal e GC stress. Os testes C incluem
thread estrangeira e atexit, com símbolos removidos por padrão. O caminho da
VM/AOT oficial não invoca callbacks fechados. Ver os registros
`late-callback-validation.json` e `late-composite-callback-validation.json`.
Isso cobre P5 em código AOT residente; reload/unloading JIT segue sem prova.
Callbacks locais vivos em outra thread continuam fatais. P4/áudio não decorrem
dessa extensão.

A análise NoEscape HIR v2 valida tipos, dominância, definições/uso, multiplicidade
de arestas Phi e ABI das chamadas estáticas no subconjunto admitido. Ainda
exige mapping fonte/ABI, proveniência de aliases, helpers/cleanup e rechecagem
após transformações; a gate pública continua recusando contratos não provados.
Predicados nullable preparados agora podem eliminar dispatch morto por
normalização exata de null/boolean, mantendo efeitos eager. Não há ganho de
velocidade medido neste recorte.

A sonda adicional de NSWindow real passou criação/show/resize/close e limpeza
pelo backend embedded em seis modos, registrada com sua CLI histórica em
`embedded-window-validation.json`. Não valida rendering/input/menus/modal nem
implementa Isolate.onEvent/handleEvent. A bomba do runtime continua com contrato
diferente de um laço externo que recebe uma notificação por evento.
