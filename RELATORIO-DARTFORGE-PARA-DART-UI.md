# DartForge para o dart_ui: pacote e anotações sem quebrar o ecossistema

Escrito em 2026-10-12. Proposta de como o DartForge pode resolver ou aliviar os
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

- O DartForge define `dartforge.compiler=true` em toda compilação.
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
  - `vm:prefer-inline` (11 ocorrências) e `vm:never-inline`;
  - `vm:entry-point`;
  - `vm:awaiter-link`.
- Hoje as duas primeiras são ignoradas. Honrá-las é ganho sem mudar código.

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
  - o backend AppKit direto passa a ser possível no DartForge, sem host, sem
    sinal e sem SkyLight;
  - os três backends atuais ficam como caminho da VM oficial.
- **Fallback na VM:** os backends de hoje, sem mudança.

### P2. Loop do isolado × loop nativo — runtime

- **No dart_ui hoje:**
  - polling com `pumpEvents(timeout)` e backoff de 16–250 ms;
  - na POC-19, o timer de 60 Hz roda efetivamente a 16 Hz, e a latência entre
    isolados no p95 é de 63 ms (com a API ideal, 0,54 ms).
- **No DartForge:** o laço tem a fila de timers e de mensagens na mão
  (`eventos.rs:136-200`). Expor o próximo prazo e um objeto de acordar é trivial:

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
  - recusar com erro claro um `await` que dependa do quadro externo.
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
  - **Com o ARC (`--memoria arc`):** objetos acíclicos morrem quando o último
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
- honrar `vm:prefer-inline`/`vm:never-inline`;
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
  - o executável do DartForge é nativo comum, com DWARF (`--depuracao`) e
    símbolos (`DARTFORGE_KEEP_SYMBOLS=1`);
  - o LLDB e o `sample` veem nomes Dart;
  - o rastro simbólico (`--rastro=simbolico`) dá a pilha Dart nas exceções.
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
  - `--subsistema=gui` no PE (hoje fixo em `/subsystem:console`, em
    `ligador_windows.rs:865`), sem invalidar assinatura;
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
| 1 | constante `dartforge.compiler`; honrar `vm:prefer-inline`; `--subsistema=gui` | dias | base para o resto; ganho de inline |
| 2 | bomba nativa no laço (macOS CFRunLoop e fila principal, Windows mensagens, Linux fds); `nextDeadline` e `runOneTurn` | 1–2 semanas | **elimina os três backends macOS e o polling** (P1, P2, P3) |
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
