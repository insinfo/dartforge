# Isolates na Web: o que o Dart 1 fazia e o que o DartForge deve fazer

Documento de **pesquisa e desenho**. Não descreve recurso implementado. Onde o texto
propõe API ou comportamento, o verbo está no futuro ou em forma condicional; onde
descreve o Dart 1 ou a plataforma Web, há referência a arquivo e símbolo.

O objetivo é restaurar a abstração de paralelismo que a Web perdeu no Dart 2, sem
violar a **regra de equivalência semântica com o Dart oficial** registrada no fim de
[PLANO.md](../PLANO.md): o DartForge pode ser mais rápido e gerar menos bytes, mas não
pode fazer o mesmo código-fonte significar outra coisa.

## Sumário da decisão

1. **Três camadas separadas**: `package:forge_isolate` (API portátil, Dart comum),
   `@WorkerEntrypoint()` (metadata opcional, sem efeito semântico) e o reconhecimento
   da API como intrínseco no DartForge (otimização de tamanho, sem efeito semântico).
2. **A anotação nunca muda onde o código executa.** Quem decide isolamento é a chamada
   de API, idêntica nos quatro alvos. A anotação só habilita diagnóstico e emissão de
   chunk.
3. **O entrypoint é um handle `const`**, não um `Function` qualquer. Isso troca a tabela
   global nome→símbolo do Dart 1 — a peça que quebrava minificação e tree shaking — por
   uma tabela local de poucas entradas que o tree shaker vê como uso normal.
4. **`TransferableBuffer` é a única categoria com transferência de posse**, mapeada para
   `postMessage(msg, [buffer])` na Web e para `TransferableTypedData` na VM. As duas
   têm o mesmo contrato observável: recurso de uso único, inutilizável após o envio.
5. **`dart:mirrors` não será implementado.** O substituto é reflexão estática gerada em
   compilação, dirigida por anotação, com alvo de API em `package:reflectable` — que é
   também a estratégia recomendada pelo time do Dart. O desenho dela está em
   [historico/MACROS-ARQUITETURA.md](historico/MACROS-ARQUITETURA.md#reflexão-estática-o-que-herdar-da-proposta-de-macros);
   aqui ficam só a evidência de código da Web e a interseção com a serialização de
   isolates. Ver [Recomendação sobre mirrors](#recomendação-sobre-mirrors).
6. **Não há protótipo neste incremento.** A razão é concreta e está em
   [Protótipo: por que não há](#protótipo-por-que-não-há) — as três peças necessárias
   ficam em crates fora do conjunto de arquivos permitido.

## Fontes e método

Referência histórica: **Dart SDK 1.24.3**, última release da série Dart 1 e última em
que `dart:isolate` e `dart:mirrors` funcionavam na Web.

| Campo | Valor |
|---|---|
| Repositório | `https://github.com/dart-lang/sdk` |
| Tag | `1.24.3` |
| Commit | `0b0b41ef25358dd77d63b4d02718287c26f8e408` (2017-12-13) |
| Licença | BSD-3-Clause, `Copyright 2012, the Dart project authors`, arquivo `LICENSE` |
| Clone local | `references/dart-sdk-1.24.3` |

O clone está registrado em [references-manifest.json](references-manifest.json). A
árvore de trabalho não tem `sdk/` nem `pkg/` materializados; as leituras usaram
`git -C references/dart-sdk-1.24.3 show HEAD:<caminho>`, sem alterar o checkout.
**Nenhuma linha do SDK foi copiada para os fontes MIT do projeto.**

Para o estado atual, as consultas usaram o clone `references/dart-sdk` na tag `3.6.2`
(commit `b0cc5495e0f5e8ae150825a5352e708cb49e65ff`), mesmo método de leitura.

Comportamento da plataforma Web: especificação HTML, seção de dados estruturados
(`StructuredSerializeInternal`, atributos IDL `[Serializable]` e `[Transferable]`).

---

## Parte 1 — O que o Dart 1 fazia

Todos os caminhos desta seção são relativos à raiz de `references/dart-sdk-1.24.3`.

### 1.1 O mapa: Isolate → Web Worker

O arquivo central é `sdk/lib/_internal/js_runtime/lib/isolate_helper.dart` (1483
linhas). A arquitetura tem três níveis de identidade:

- **`_Manager`** (L176) — um por contexto JavaScript, isto é, um por página e um por
  Worker. Guarda `isolates` (mapa id→contexto) e, só no manager principal, `managers`
  (mapa id→objeto `Worker` nativo). O campo `currentManagerId` (L181) é o `workerId`
  usado no endereçamento de portas.
- **`_IsolateContext`** (L294) — um por isolate lógico. Vários isolates podem morar no
  mesmo `_Manager`; é isso que permite `useWorkers == false`.
- **`_EventLoop`** (L610) — fila de eventos própria, em Dart, por manager.

`_Manager.useWorkers` (L211) é um getter que devolve `supportsWorkers`, com o comentário
explícito *"Whether to use web workers when implementing isolates. Set to false for
debugging/testing"*. Ou seja: **o próprio Dart 1 já tratava "rodar no mesmo thread" como
modo de depuração, não como fallback silencioso.** Esse detalhe sustenta uma decisão de
desenho adiante ([§3.9](#39-depuração-no-mesmo-thread-é-opt-in-nunca-fallback)).

`_Manager._nativeDetectEnvironment` (L243) decide o papel do contexto por feature
detection: `isWorker = !isWindowDefined && globalPostMessageDefined`, e
`supportsWorkers = isWorker || (isWorkerDefined && IsolateNatives.thisScript != null)`.

### 1.2 O nascimento de um Worker

`IsolateNatives` (L755) concentra o spawn. O caminho completo:

```
Isolate.spawn(f, msg)                         isolate_patch.dart:41
  → IsolateNatives.spawnFunction(f, msg, p)   isolate_helper.dart:950
      name = _getJSFunctionName(f)            isolate_helper.dart:939
      if (name == null) throw UnsupportedError("only top-level functions can be spawned.")
  → IsolateNatives.spawn(name, null, …)       isolate_helper.dart:974
      if (useWorkers && !isLight) _startWorker(…)   L994
  → IsolateNatives._startWorker(…)            isolate_helper.dart:1004
  → IsolateNatives._spawnWorker(…)            isolate_helper.dart:1106
      if (uri == null) uri = thisScript;                     L1115
      final worker = JS('var', 'new Worker(#)', uri);        L1116
      worker.postMessage(_serializeMessage({'command':'start', 'functionName': name, …}))
```

Três fatos com consequência direta no desenho:

**Fato 1 — o Worker nasce de uma URL, e a URL era o próprio programa.**
`_spawnWorker` (L1115–L1116) faz `uri ??= thisScript` e `new Worker(uri)`. O
`thisScript` vem de `IsolateNatives.computeThisScript()` (L783), que lê o global
embutido `CURRENT_SCRIPT` — o `document.currentScript` capturado pelo emissor — e, num
Worker, recai em `computeThisScriptFromTrace()` (L795), que extrai a URL de um
`new Error().stack` com duas expressões regulares (V8/IE e Firefox). O Worker portanto
carrega **o mesmo `main.dart.js` inteiro**, com o runtime completo e todas as
bibliotecas do programa, para executar uma única função. Esse é o problema de tamanho
que motivou a remoção, e está em duas linhas de código.

**Fato 2 — a função tinha de ser identificável pelo nome.**
`spawnFunction` (L950) chama `_getJSFunctionName` (L939), que lê a propriedade
`STATIC_FUNCTION_NAME_PROPERTY_NAME` — a string literal `$static_name`, definida em
`sdk/lib/_internal/js_runtime/lib/shared/embedded_names.dart:48` — do objeto closure. Só
tear-offs de função estática ou top-level têm essa propriedade, gravada em
`sdk/lib/_internal/js_runtime/lib/js_helper.dart:2635`. Para qualquer outra closure o
nome é `null` e o resultado é
`UnsupportedError("only top-level functions can be spawned.")` (L955).

**Fato 3 — o `main` de todo programa com isolates era reescrito.**
`pkg/compiler/lib/src/js_emitter/main_call_stub_generator.dart:38` testa
`_backendUsage.isIsolateInUse` e, quando verdadeiro, gera
`function(args) { $.startRootIsolate(X.main$closure(), args); }` em vez de chamar `main`
diretamente. `startRootIsolate` (isolate_helper.dart:102) constrói o `_Manager`, e na
L111 faz `if (_globalState.isWorker) return;` — é assim que o Worker evita rodar o
`main` do aplicativo. **Importar `dart:isolate` mudava o caminho de inicialização de
todo o programa**, não só do trecho que usava isolates.

### 1.3 O ciclo de vida e o roteamento de mensagens

`IsolateNatives._processWorkerMessage` (L841) é o `switch` que implementa o protocolo:
`start`, `spawn-worker`, `message`, `close`, `log`, `print`, `error`.

Dois detalhes valem registro:

- **Worker não podia criar Worker diretamente.** `_startWorker` (L1017) verifica
  `if (_globalState.isWorker)` e, em vez de chamar `new Worker`, envia
  `{'command': 'spawn-worker', …}` ao manager principal, que atende em
  `handleSpawnWorkerRequest` (L901). Analogamente, `_WorkerSendPort.send` (L1256) roteia
  mensagem de Worker para Worker **através do thread principal** (L1261–L1263), com o
  comentário *"Communication from one worker to another go through the main worker"*.
  Cada mensagem entre dois workers custava duas serializações e um salto pelo thread
  que a biblioteca existia para desocupar.
- **`print` era encaminhado por mensagem.** `_Manager._nativeInitWorkerMessageHandler`
  (L255) instala um `self.dartPrint` que usa `console.log` quando existe e, senão,
  `self.postMessage(serialize(object))`; o manager principal atende em
  `_processWorkerMessage` caso `'print'` (L888).
- **O Worker se fechava sozinho.** `_Manager.maybeCloseWorker` (L286) envia
  `{'command': 'close'}` quando não há isolates nem operações JS assíncronas pendentes,
  contadas à mão por `enterJsAsync`/`leaveJsAsync` (L75/L82). O manager principal
  responde com `worker.terminate()` (L882).

### 1.4 As portas

`_BaseSendPort` (L1197) tem duas implementações:

- **`_NativeJsSendPort`** (L1217) — entrega em memória, no mesmo contexto JS. `send`
  clona a mensagem com `_clone` (L1229) e enfileira no `_EventLoop`. Note que **mesmo no
  mesmo thread a mensagem é clonada**: o isolamento de memória é preservado
  independentemente de haver Worker. Isso é exatamente o que a regra de equivalência
  semântica exige, e o Dart 1 já fazia.
- **`_WorkerSendPort`** (L1249) — identidade é a tripla
  `(_workerId, _isolateId, _receivePortId)`, com `operator ==` (L1274) e `hashCode`
  (L1281) definidos sobre ela. Entrega via `postMessage`.

`RawReceivePortImpl` (L1286) numera portas com um contador global por manager
(`_nextFreeId`, L1287) e se registra no `_IsolateContext`.

### 1.5 A serialização

`sdk/lib/_internal/js_runtime/lib/isolate_serialization.dart` (387 linhas), `part of
_isolate_helper`.

`_Serializer.serialize` (L35) constrói um **envelope em arrays JS** com tags de string, e
`_Deserializer.deserialize` (L192) reconstrói. As tags são `ref`, `buffer`, `typed`,
`fixed`, `extendable`, `mutable`, `const`, `map`, `sendport`, `raw sendport`,
`js-object`, `function`, `capability`, `dart`.

Ciclos e compartilhamento são preservados por `serializedObjectIds`, um
`Map.identity()`, e pela tag `ref` (L38–L41, `makeRef` em L74).

Quatro rejeições explícitas, que definem o contrato real:

| Rejeição | Linha | Mensagem |
|---|---|---|
| `Interceptor` residual | L52 | `Can't transmit:` |
| `RawReceivePort` | L54–L55 | `RawReceivePorts can't be transmitted:` |
| Closure sem `$static_name` | L162–L165 | `Closures can't be transmitted:` |
| Objeto JS com construtor ≠ `Object` | L127–L134 | `Only plain JS Objects are supported:` |

A última é importante e é a origem do problema "objetos nativos não são
transmissíveis": o navegador **sempre** soube clonar `Blob`, `File` e `ImageData`, mas o
serializador do Dart 1 os recusava antes de chegarem ao `postMessage`, porque
`x.constructor !== Object`. Era limitação da implementação, não da plataforma.

O ponto mais caro é `serializeDartObject` (L167) e `deserializeDartObject` (L369):

```dart
// serializeDartObject, L170
var classExtractor = JS_EMBEDDED_GLOBAL('', CLASS_ID_EXTRACTOR);
var fieldsExtractor = JS_EMBEDDED_GLOBAL('', CLASS_FIELDS_EXTRACTOR);
String classId = JS('String', '#(#)', classExtractor, x);
List fields = JS('JSArray', '#(#)', fieldsExtractor, x);
return ['dart', classId, serializeArrayInPlace(fields)];
```

Existe porque o algoritmo de clone estruturado **não preserva o protótipo**. Uma
instância de classe Dart, na Web, é um objeto JS com protótipo; atravessar `postMessage`
a devolveria como objeto simples. O Dart 1 contornou isso reconstruindo a instância do
outro lado a partir de um identificador de classe e da lista de campos.

Note a composição de custos: a extração produz um array novo por instância (**cópia 1**),
e em seguida `postMessage` aplica clone estruturado sobre esse array (**cópia 2**).

### 1.6 O apoio que o compilador tinha de emitir

`pkg/compiler/lib/src/js_emitter/startup_emitter/fragment_emitter.dart`, em
`emitEmbeddedGlobals`, sob a guarda `if (program.hasIsolateSupport)` (L1399), emite seis
globais. Os três decisivos:

```js
// CLASS_ID_EXTRACTOR — fragment_emitter.dart:1415
function(o) { return o.constructor.name; }

// CLASS_FIELDS_EXTRACTOR — fragment_emitter.dart:1418
function(o) {
  var constructor = o.constructor;
  var fieldNames = constructor.$cachedClassFieldNames;
  if (!fieldNames) {
    var empty = new constructor();
    fieldNames = constructor.$cachedClassFieldNames = Object.keys(empty);
  }
  …
}

// INSTANCE_FROM_CLASS_ID — fragment_emitter.dart:1437
function(name) { var constructor = getGlobalFromName(name); return new constructor(); }
```

E `getGlobalFromName` (L294), a peça central:

```js
// Returns the global with the given [name].
function getGlobalFromName(name) {
  for (var i = 0; i < holders.length; i++) {
    if (holders[i] == #constantHolderReference) continue;
    if (holders[i][name]) return holders[i][name];
  }
}
```

Duas propriedades hostis, e elas são a razão técnica da frase oficial sobre "overhead
substancial":

- **Resolução de símbolo por string, em tempo de execução.** Qualquer classe ou função
  estática pode ser ressuscitada por nome, a partir de uma string que veio de outro
  contexto. O compilador não pode provar que um nome não será pedido, então não pode
  remover o símbolo nem renomeá-lo com liberdade.
- **Identidade de classe = nome do construtor JS.** `o.constructor.name` amarra o
  protocolo de serialização ao nome que o minificador gostaria de encurtar, e amarra
  main e Worker ao **mesmo** esquema de nomes.

`STATIC_FUNCTION_NAME_TO_CLOSURE` (L1404) tem a mesma forma: recebe um nome, chama
`getGlobalFromName`, e executa o getter de tear-off.

Há ainda um sinal de que o time lutava contra o tamanho **dentro** da implementação.
`IsolateNatives.enableSpawnWorker` (L763):

```dart
// We set [enableSpawnWorker] to true (not null) when calling isolate
// primitives that require support for spawning workers. The field starts out
// by being null, and dart2js' type inference will track if it can have a
// non-null value. So by testing if this value is not null, we generate code
// that dart2js knows is dead when worker support isn't needed.
static var enableSpawnWorker;
```

É um campo `dynamic` sem tipo, deixado `null`, cuja única função é enganar a inferência
de tipos do próprio compilador para que ela apague o suporte a Worker quando ninguém o
usa. Não é um mecanismo; é uma gambiarra documentada como tal.

### 1.7 `dart:mirrors` na Web

`sdk/lib/_internal/js_runtime/lib/mirrors_patch.dart` (51 linhas) é um patch fino que
delega tudo para `sdk/lib/_internal/js_runtime/lib/js_mirrors.dart` (3090 linhas).

A evidência não precisa de interpretação. Linhas 71–73 de `js_mirrors.dart`:

```dart
/// No-op method that is called to inform the compiler that tree-shaking needs
/// to be disabled.
disableTreeShaking() => preserveNames();
```

E irmãs em L77, L81, L85: `preserveMetadata()`, `preserveUris()`,
`preserveLibraryNames()`, todas corpos vazios.

Chamadas de `disableTreeShaking()` em `js_mirrors.dart`: L141
(`computeLibrariesByName`), L630 (`reflectClassByName`), L822
(`reflectMixinApplication`), L1063 (`_getCachedInvocation`), L2238. Ou seja: enumerar
bibliotecas, refletir uma classe por nome, resolver aplicação de mixin e invocar um
membro por símbolo — as quatro operações centrais de reflexão — **cada uma desliga o
tree shaking do programa inteiro**.

Do lado do compilador, `pkg/compiler/lib/src/js_backend/mirrors_data.dart` L216–L224
observa essas chamadas e liga os campos `mustDisableTreeShaking`, `mustPreserveNames`,
`mustPreserveMetadata`, `mustPreserveUris`, `mustPreserveLibraryNames`. O compilador
literalmente desliga as próprias otimizações quando vê o marcador.

Os globais embutidos que só existem para reflexão estão anotados como tal em
`shared/embedded_names.dart`: `ALL_CLASSES` (*"only used by reflection"*),
`TYPE_INFORMATION` (idem), `LIBRARIES`, `STATICS`, `LAZIES`.

A válvula de escape era `@MirrorsUsed`, declarada em `sdk/lib/mirrors/mirrors.dart:1242`.
A própria documentação da classe admite o problema e marca o recurso como
**EXPERIMENTAL**:

> *Annotation describing how "dart:mirrors" is used (EXPERIMENTAL). … In some
> scenarios, for example, when minifying Dart code, or when generating JavaScript code
> from a Dart program, the size and performance of the output can suffer from use of
> reflection. In those cases, telling the compiler what is used, can have a significant
> impact.*

Isto é: a solução oficial para reflexão na Web já era, em 2017, **declarar
antecipadamente o que seria refletido** — ou seja, fechar o mundo à mão. É precisamente
a ideia da reflexão estática recomendada adiante, só que sem geração de código e com o
ônus no programador.

---

## Parte 2 — Por que foi removido

Esta seção separa deliberadamente três níveis de confiança.

### 2.1 Fato documentado — o estado atual do SDK

Verificável em `references/dart-sdk`, tag `3.6.2`:

- `sdk/lib/libraries.json`: na seção `dart2js`, a entrada `isolate` existe mas tem
  `"supported": false`; idem na seção `dartdevc`. A chave `mirrors` **não existe** nessas
  seções — só na seção `vm`.
- `sdk/lib/_internal/js_runtime/lib/isolate_patch.dart` foi reduzido a um patch que
  lança `UnsupportedError` em absolutamente tudo: `Isolate.current`, `Isolate.spawn`,
  `Isolate.spawnUri`, `ReceivePort.listen`, `RawReceivePort`, `Capability`,
  `TransferableTypedData.fromList`.
- `isolate_helper.dart`, `isolate_serialization.dart`, `js_mirrors.dart` e
  `mirrors_patch.dart` **não existem mais** em `sdk/lib/_internal/js_runtime/lib/`.
- Consequência para imports condicionais, já medida por este projeto em
  [historico/ENVIRONMENT-REFERENCIAS.md](historico/ENVIRONMENT-REFERENCIAS.md): `dart.library.isolate` é
  **ausente** em dart2js, `true` em Native AOT e em dart2wasm; `dart.library.mirrors` é
  ausente nos três.

### 2.2 Fato documentado — a razão declarada pelo time

O anúncio de quebra de compatibilidade do Dart 2 (*"Dart2 Breaking Change: Removing web
support for dart:mirrors and dart:isolate"*, lista `dart-announce`) dá duas razões para
isolates e uma para mirrors:

- Sobre `dart:isolate`: *"most users found the isolate API limiting compared to the Web
  Workers API"*, e *"the infrastructure for supporting isolates also adds substantial
  overhead when compiling to JavaScript"*.
- Sobre `dart:mirrors`: *"to provide a full fidelity experience, Dart JavaScript
  compilers included a substantial amount of type information while also losing the
  ability to do precise tree-shaking. This leads to dramatically increased output size
  and execution time."* A recomendação oficial é geração de código —
  `package:build`, `package:source_gen`.

O metabug `dart-lang/sdk#30538` acrescenta: *"Support for those has been limited and
problematic in many ways for years, so the simplest solution that gives us the most room
for future improvement is to not support them at all."*

O `dart-lang/sdk#32684` registra a intenção de oferecer *"a highly functional
alternative with web workers"*. Essa alternativa não chegou ao SDK; é o vácuo que este
desenho pretende preencher.

### 2.3 Inferência deste documento

O que segue **não** está itemizado em documento oficial; é a leitura do código feita
acima, e está marcada como inferência para que ninguém a cite como fato do time do Dart:

- A frase *"substantial overhead"* se materializa em três coisas específicas:
  (a) `new Worker(thisScript)` faz cada Worker baixar, parsear e inicializar o programa
  inteiro (§1.2, Fato 1); (b) `getGlobalFromName` exige que nomes de símbolo permaneçam
  resolvíveis por string em tempo de execução, o que impede remoção e limita minificação
  (§1.6); (c) `isIsolateInUse` reescreve o `main` de qualquer programa que importe a
  biblioteca (§1.2, Fato 3).
- A incompatibilidade com tree shaking em `dart:mirrors` é de natureza diferente da de
  isolates: em isolates ela vem do **protocolo de serialização** (identidade de classe
  por nome), não da reflexão. Corrigir o protocolo remove o problema em isolates sem
  resolver nada em mirrors. É por isso que este desenho aceita um e recusa o outro.
- A existência de `enableSpawnWorker` como truque de inferência (§1.6) sugere que o
  custo era percebido internamente muito antes da remoção. É leitura de intenção a
  partir de um comentário, não declaração.

### 2.4 Os quatro problemas de serialização, revisitados

A lista de quatro problemas é a premissa da investigação. O veredito abaixo é meu, com a
evidência de cada caso.

| Problema do Dart 1 | Situação em 2026 | Evidência |
|---|---|---|
| **Clone 10–20% mais lento** | **Persiste como natureza, muda de grau.** Clone estruturado é cópia por definição; não existe passagem por referência entre agentes. O que era evitável é a **cópia dupla**: o Dart 1 montava um envelope em arrays (cópia 1) e deixava o `postMessage` cloná-lo (cópia 2). Um protocolo que mantenha o dado em formas já nativas de clone estruturado reduz o excedente a *um array curto por instância de classe*, proporcional à contagem de objetos, não ao volume de bytes. | `serializeDartObject` (isolate_serialization.dart:170) + `serializeArrayInPlace` (L111) |
| **Sem transferência de posse** | **Resolvido.** `postMessage(msg, transferList)` transfere `ArrayBuffer`, `MessagePort`, `ImageBitmap`, `OffscreenCanvas`, streams e mais, com posse e zero cópia; o objeto de origem fica com `[[Detached]]` verdadeiro e inutilizável. O Dart 1 simplesmente nunca usou o segundo argumento: `serializeByteBuffer` (L79) devolve `["buffer", buffer]`, isto é, manda o buffer *pelo clone*. Não há nenhuma lista de transferência em todo o arquivo. | HTML, `[Transferable]` e `[[Detached]]`; isolate_serialization.dart:79 |
| **`Blob`/`File`/`ImageData` não transmissíveis** | **Resolvido, e nunca foi limitação da plataforma.** Esses três são serializáveis por clone estruturado. O que os barrava era `serializeJSObject`, que lança `Only plain JS Objects are supported:` quando `x.constructor !== Object`. Deixar o valor passar intacto basta. | isolate_serialization.dart:127–134; HTML `[Serializable]` |
| **Inconsistência dart2js × Dartium** | **Obsoleto na forma, vivo na substância.** Dartium não existe. A divergência hoje é Web × VM: a VM aceita enviar closures (`Isolate.run` envia uma `FutureOr<R> Function()`, `sdk/lib/isolate/isolate.dart:253` em 3.6.2) e objetos de qualquer classe; a Web não aceita função nenhuma (`DataCloneError` para qualquer `IsCallable`) e perde protótipo. A resposta de desenho é fazer o contrato da API ser a **interseção**, imposta igualmente nas quatro plataformas — um programa que funciona em uma funciona em todas. | isolate.dart:253 (3.6.2); HTML, *"if IsCallable(value) is true"* |

### 2.5 O que o clone estruturado cobre hoje

Resumo normativo, para fixar o contrato da camada de serialização.

**Cobre** — primitivos (exceto `Symbol`), `String`, `Boolean`, `Number`, `BigInt`,
`Date`, `RegExp` (sem `lastIndex`), `Array`, `Map`, `Set`, objetos simples,
`ArrayBuffer`, `DataView`, todos os `TypedArray`, os `Error` intrínsecos, `Blob`,
`File`, `FileList`, `ImageData`, `ImageBitmap`, `DOMException`, `CryptoKey`,
`FileSystemHandle`, `DOMPoint`/`DOMRect`/`DOMMatrix`/`DOMQuad`, `AudioData`,
`VideoFrame`, `EncodedAudioChunk`, `EncodedVideoChunk`, `RTCCertificate`. Preserva
ciclos e compartilhamento interno do grafo.

**Não cobre** — lança `DataCloneError`: funções (*"if IsCallable(value) is true"*),
`Symbol`, nós do DOM, `Proxy`, objetos com slots internos restritos (`Promise`,
`WeakMap`), e qualquer objeto de plataforma sem `[Serializable]`.

**Perde, silenciosamente** — e esta é a lista que mais importa para Dart:

- **A cadeia de protótipos.** Instância de classe chega como objeto simples.
- **Descritores de propriedade, getters e setters.** Um objeto `readonly` volta
  gravável.
- **Campos privados de classe** (`#x` do JS).
- **Identidade de subclasse de `Error`.** Uma subclasse vira `Error` genérico.
- `RegExp.lastIndex`.

A perda de protótipo é o único item que obriga a existir uma camada de serialização
própria em Dart. Todo o resto pode passar direto.

---

## Parte 3 — O desenho proposto para o DartForge

### 3.1 O diagrama das três camadas

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ CAMADA 1 — API portátil:  package:forge_isolate                             │
│ Dart comum. Compila em DartForge, dart2js, DDC e Dart VM.                    │
│ Define a SEMÂNTICA. É a única camada que qualquer programa precisa.          │
│                                                                             │
│   WorkerIsolate.guard / run / spawn / terminate                             │
│   WorkerTask<T,R>   SendPort<T>   ReceivePort<T>   TransferableBuffer        │
│                                                                             │
│   forge_isolate.dart                                                        │
│     export 'src/stub.dart'                                                  │
│         if (dart.library.io)          'src/vm.dart'                         │
│         if (dart.library.js_interop)  'src/web.dart';                       │
│                                                                             │
│   ├── src/vm.dart   → dart:isolate            (Isolate.spawn,               │
│   │                                            TransferableTypedData)       │
│   └── src/web.dart  → dart:js_interop + package:web                         │
│                       (new Worker(url,{type:'module'}), postMessage,        │
│                        MessageChannel, transfer list)                        │
└─────────────────────────────────────────────────────────────────────────────┘
                │                                        ▲
                │ o programa só depende daqui            │ nenhuma camada
                ▼                                        │ acima muda isto
┌─────────────────────────────────────────────────────────────────────────────┐
│ CAMADA 2 — Metadata opcional:  @WorkerEntrypoint()                          │
│ Anotação Dart comum. dart2js, DDC, VM e o analyzer aceitam e IGNORAM.       │
│ ZERO efeito semântico. Omiti-la não muda o comportamento do programa.       │
│                                                                             │
│   @WorkerEntrypoint()                                                       │
│   Future<PdfResult> renderPdf(PdfJob job) async { … }                       │
│                                                                             │
│   Serve para: (a) diagnóstico no ponto da DECLARAÇÃO, não da chamada;       │
│               (b) dar ao DartForge a raiz de um chunk sem ter de provar     │
│                   o conjunto de handles;                                    │
│               (c) documentar intenção para leitor e IDE.                    │
└─────────────────────────────────────────────────────────────────────────────┘
                │
                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ CAMADA 3 — Otimização do DartForge (só tamanho e tempo, nunca semântica)    │
│                                                                             │
│  ┌──────────────┐   reconhece     ┌───────────────────────────────────┐    │
│  │  semantic    │ ─ intrínseco ─► │ tabela de tarefas de worker        │    │
│  │              │   guard/run/    │ (nome declarado → função raiz)     │    │
│  └──────────────┘   spawn         └───────────────┬───────────────────┘    │
│                                                    │                        │
│  ┌──────────────┐   fecho transitivo por raiz      ▼                        │
│  │  optimizer   │ ◄──────────────────  raízes = {main} ∪ {entrypoints}     │
│  │  tree_shake  │                                                           │
│  └──────┬───────┘                                                           │
│         ▼                                                                   │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  codegen — N unidades de saída, cada uma com SEU runtime mínimo       │  │
│  │                                                                       │  │
│  │   app.mjs          = fecho(main)   \ {corpos de entrypoint não usados │  │
│  │                                       no thread principal}            │  │
│  │   app.w.pdf.mjs    = fecho(renderPdf) + serializador dos tipos da     │  │
│  │                      assinatura + protocolo de porta                  │  │
│  │                                                                       │  │
│  │   (dart2js emite 1 arquivo; o Worker carrega o programa inteiro.      │  │
│  │    Mesmo comportamento observável, mais bytes.)                        │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

A propriedade que torna o desenho defensável: **removendo a camada 3 inteira, o programa
continua correto**; removendo a camada 2, também. Só a camada 1 é obrigatória, e ela é
Dart comum.

### 3.2 A API portátil, em concreto

```dart
// package:forge_isolate

/// Identificador estável de uma função que pode rodar em outro isolate.
///
/// O nome é declarado pelo programador, não derivado de símbolo do compilador.
/// É ele que atravessa a fronteira; a função é referenciada diretamente.
final class WorkerTask<T, R> {
  const WorkerTask(this.name, this.function);
  final String name;
  final FutureOr<R> Function(T message) function;
}

abstract final class WorkerIsolate {
  /// Executa [task] noutro isolate, entrega [message] e completa com o resultado.
  /// Equivale a `Isolate.run` restrito a entrypoints declarados.
  static Future<R> run<T, R>(WorkerTask<T, R> task, T message,
      {String? debugName});

  /// Cria um isolate de vida longa. O protocolo de mensagens é do programa.
  static Future<WorkerIsolate> spawn<T>(WorkerTask<T, void> task, T message,
      {SendPort<Object?>? onExit, SendPort<Object?>? onError,
       bool errorsAreFatal = true, String? debugName});

  /// Ponto de entrada único do programa.
  ///
  /// Quando o contexto atual é o thread principal, executa [body].
  /// Quando é um isolate secundário, atende ao protocolo de spawn e NUNCA
  /// executa [body].
  static Future<void> guard(
      List<WorkerTask<Object?, Object?>> tasks, FutureOr<void> Function() body);

  void terminate();
}
```

Uso:

```dart
@WorkerEntrypoint()
Future<PdfResult> renderPdf(PdfJob job) async { … }

const renderPdfTask = WorkerTask<PdfJob, PdfResult>('pdf.render', renderPdf);

Future<void> main() => WorkerIsolate.guard(const [renderPdfTask], () async {
  final resultado = await WorkerIsolate.run(renderPdfTask, PdfJob(bytes));
  print(resultado.paginas);
});
```

`guard` existe por uma razão que não é escolha estética. Em dart2js e DDC, o isolate
secundário carrega **o programa inteiro**, e o programa inteiro começa em `main`. Sem um
ponto de decisão explícito, o `main` do aplicativo rodaria dentro do Worker. O Dart 1
resolvia isso reescrevendo o `main` no emissor (`main_call_stub_generator.dart:38`, §1.2
Fato 3) — o que muda o programa de quem só importa a biblioteca. `guard` move essa
decisão para o código-fonte, em uma linha, visível, e idêntica nos quatro alvos.

Consequência de desenho, e é ela que fecha a equivalência: **o DartForge pode
especializar `guard` em cada chunk.** No chunk de worker, `guard` é conhecido como "sou
secundário", a condição dobra em constante, o corpo `body` fica inalcançável e o tree
shaker o remove — junto com o aplicativo todo. No chunk principal, `guard` é "sou
principal" e o despacho de tarefas some. Nos dois casos o *comportamento observável* é o
mesmo que em dart2js, onde os dois caminhos existem no arquivo e só um roda. A otimização
é legítima porque especializa um ramo que já era morto naquele contexto.

Tudo que está antes de `guard` em `main` roda nos dois casos, em todos os compiladores —
por isso `guard` tomar `body` como closure, em vez de ser um `if (…) return;`, elimina a
categoria de bug "código antes da guarda". Não há nada antes da guarda.

### 3.3 `Isolate.spawn(funcao)` — por que é difícil e como resolver

O problema, textualmente: **um Worker nasce de uma URL, não de uma função.** O que
atravessa a fronteira é bytes, e uma função Dart não é serializável em lugar nenhum
(`DataCloneError` para qualquer `IsCallable`, e o Dart 1 recusava com
`Closures can't be transmitted:`). Então algo do lado de lá tem de já conter o código, e
alguma chave tem de dizer *qual* código rodar.

O Dart 1 escolheu como chave o **nome do símbolo do compilador** (`$static_name`, §1.2
Fato 2) e pagou com `getGlobalFromName` (§1.6): uma tabela global consultável por string,
que impede remoção e limita minificação.

O DartForge escolhe como chave um **nome declarado pelo programador**, guardado num
`WorkerTask` `const`, junto com a referência direta à função:

```dart
const renderPdfTask = WorkerTask<PdfJob, PdfResult>('pdf.render', renderPdf);
```

Quatro consequências, e é nelas que o desenho se paga:

1. **A referência `renderPdf` é um uso comum.** O tree shaker que já existe em
   `crates/optimizer/src/tree_shake.rs` alcança tear-offs por nome via `Scan::name`
   (L126). Não há caso especial: a função é alcançável porque alguém a menciona.
2. **Nenhum nome de símbolo vai para o protocolo.** A string `'pdf.render'` é dado do
   programa. Minificação, renomeação e fusão de funções idênticas
   (`crates/optimizer/src/merge.rs`) continuam livres.
3. **A resolução no lado receptor é uma varredura de poucas entradas** na lista `const`
   passada a `guard` — não uma varredura de todos os *holders* do programa.
4. **O conjunto de entrypoints é finito e conhecido estaticamente**, o que é exatamente
   a pré-condição para emitir um chunk por entrypoint.

**A limitação resultante, dita sem rodeio.** O entrypoint tem de ser uma função top-level
ou estática, declarada num `WorkerTask` que apareça na lista de `guard`. Não são
aceitos:

- closure que captura estado;
- tear-off de método de instância;
- valor `Function` calculado em tempo de execução;
- função vinda de uma biblioteca carregada depois.

Isso é **estritamente mais restrito que `Isolate.run` da VM**, que aceita qualquer
closure. E a restrição vale **também na VM**, porque a API é a mesma nos quatro alvos.
Essa é a escolha central do desenho e o preço de não divergir: preferimos uma API
portátil mais estreita a uma API larga que funciona em um alvo e falha em outro. É o
mesmo raciocínio de §2.4, linha 4 — o contrato é a interseção.

O que **não** fazemos, e isso é importante: não aceitamos `Function` na assinatura para
depois lançar em tempo de execução na Web. Uma assinatura que aceita o que uma plataforma
não suporta transforma erro de compilação em erro de produção.

### 3.4 Serialização: tabela estática dirigida por tipo

O único motivo para existir uma camada de serialização própria é a perda de protótipo
(§2.5). Então a camada deve tratar **apenas** esse caso e deixar todo o resto passar
intacto — o oposto do Dart 1, que reserializava tudo.

Regra de despacho:

1. Valor já nativo de clone estruturado (`num`, `bool`, `String`, `null`, `List` de
   nativos, `Map`, `Set`, `Uint8List` e demais `TypedData`, `ByteBuffer`, `DateTime`
   como `Date`, `Blob`, `File`, `ImageData`) → **passa direto**. Zero trabalho em Dart,
   um único clone dentro do navegador.
2. Instância de classe Dart → envelope `[tag, campo…]`, onde `tag` é um **inteiro
   pequeno atribuído em tempo de compilação**, não `o.constructor.name`.
3. `TransferableBuffer` → vai para a lista de transferência (§3.5).
4. `SendPort` → identificador de canal (§3.6).
5. Qualquer outra coisa → **erro de compilação** quando o tipo é estaticamente conhecido,
   e `ArgumentError` em tempo de execução quando não é.

Como o conjunto de tipos é fechado: pelos argumentos de tipo. `WorkerTask<PdfJob,
PdfResult>` diz que `PdfJob` entra e `PdfResult` sai; `SendPort<Progresso>` diz o que
trafega naquele canal. O DartForge calcula o fecho dos tipos alcançáveis a partir
daqueles e gera um par de funções por tipo:

```
_forgeWrite$PdfJob(obj)   → [17, obj.a, obj.b, …]
_forgeRead$PdfJob(record) → PdfJob._raw(record[1], record[2], …)
```

Diferenças deliberadas em relação ao Dart 1, cada uma corrigindo um problema de §1.6:

| Dart 1 | DartForge proposto | O que se ganha |
|---|---|---|
| `o.constructor.name` | inteiro de tabela | minificação livre; nome não vaza no protocolo |
| `getGlobalFromName` varrendo holders | índice direto na tabela do chunk | símbolo não precisa ser resolvível por string |
| tabela emitida para **todas** as classes | tabela só para os tipos que atravessam porta | menos bytes; tree shaking intacto |
| todo objeto reserializado | só instâncias de classe | menos uma cópia sobre os bytes grandes |
| `Object.keys(new constructor())` em tempo de execução | ordem de campos fixada em compilação | sem sonda reflexiva no runtime |

Três obrigações que o desenho assume e que não devem ser esquecidas na implementação:

- **Ciclos e compartilhamento têm de ser preservados**, porque tanto clone estruturado
  quanto a VM os preservam, e tanto o Dart 1 preservava (`serializedObjectIds` + tag
  `ref`). Perdê-los seria divergência semântica, não otimização.
- **`dynamic` e genéricos não resolvidos** no tipo da mensagem tornam o mundo aberto.
  A escolha proposta é diagnóstico em compilação; a alternativa — recair numa tabela
  global — é justamente a armadilha do Dart 1.
- **A tabela de tags é por compilação.** Um chunk de worker antigo em cache do navegador
  com um `app.mjs` novo leria tags trocadas. Mitigação obrigatória: um hash da build no
  *handshake* de `start`, com recusa explícita e mensagem clara em caso de divergência.
  O `o.constructor.name` do Dart 1 era acidentalmente robusto a isso — ao preço de
  §1.6. É uma troca consciente, não um esquecimento.

Sobre a promessa de custo, dita com honestidade: **clone estruturado é cópia e continuará
sendo.** O que o desenho reduz é o excedente sobre ela. Passando pelo caminho 1 os
`TypedData`, a cópia extra em Dart desaparece para o volume; sobra um array curto por
instância de classe, isto é, custo proporcional à **contagem de objetos**, não ao número
de bytes. Nenhuma medição foi feita; a afirmação é estrutural e precisa de benchmark
antes de virar número.

### 3.5 `TransferableBuffer` e as três categorias

As três categorias pedidas, com a regra e o motivo:

| Categoria | Exemplos | Regra | Por que |
|---|---|---|---|
| **Objeto comum** | instância de classe, `List`, `Map`, `Set`, `DateTime`, `Uint8List` passado como dado | **clone** | Isolamento de memória é a semântica de isolate. A VM clona, o clone estruturado clona, e o Dart 1 clonava até no mesmo thread (`_NativeJsSendPort.send`, isolate_helper.dart:1229). |
| **Primitivo imutável** | `null`, `bool`, `int`, `double`, `String`, constantes canônicas | **clone** | Sendo imutáveis, cópia e compartilhamento são indistinguíveis por observação. Identidade entre isolates não é preservada em plataforma nenhuma, então nada se perde. |
| **Buffer transferível** | `TransferableBuffer` | **transferência de posse, zero cópia** | É a única forma de mover grande volume sem duplicar. O preço é explícito no tipo: a origem morre. |

```dart
final class TransferableBuffer {
  /// Toma posse dos bytes de [data]. Após esta chamada o dado de origem
  /// não deve ser usado.
  factory TransferableBuffer.fromTypedData(TypedData data);

  /// Devolve os bytes. Recurso de USO ÚNICO: a segunda chamada lança
  /// [StateError], em qualquer plataforma.
  ByteBuffer materialize();
}
```

Mapeamento:

- **Web** — o buffer entra na lista de transferência de
  `postMessage(mensagem, [arrayBuffer])`. A especificação HTML exige que o objeto tenha
  `[Transferable]` e marca `[[Detached]]` como verdadeiro após a transferência,
  impedindo reuso. Um `ArrayBuffer` destacado lança ao ser lido.
- **VM** — `TransferableTypedData.fromList` / `materialize()`. A documentação em
  `sdk/lib/isolate/isolate.dart:1052–1057` (3.6.2) diz textualmente: *"The
  [TransferableTypedData] is a cross-isolate single-use resource. This method must not be
  called more than once on the same underlying transferable bytes, even if the calls
  occur in different isolates."*

**As duas semânticas coincidem**: recurso de uso único, consumido por materialização,
inutilizável na origem. É por isso que `TransferableBuffer` pode ser um invólucro fino
com contrato observável idêntico nos quatro alvos, incluindo a **mesma classe de exceção**
no uso após transferência — condição para que o comportamento não dependa do compilador.

O caso de uso que justifica o tipo, com números: um PDF de 200 MB enviado ao worker.
Por clone, o pico é **400 MB** — original e cópia coexistem durante a serialização. Por
transferência, é **200 MB**, e o custo de tempo é O(1) em vez de O(n). A conta não depende
de medição; é a definição das duas operações.

Limite honesto: a transferência funciona sobre `ArrayBuffer`, não sobre um grafo de
objetos. Um `PdfDocument` com 4000 objetos e um `Uint8List` grande transfere o buffer e
clona os 4000 objetos. Quem quer zero cópia projeta o dado como bytes.

Fora de escopo, e por escolha: `SharedArrayBuffer` e atomics. Não são semântica de
isolate — são memória compartilhada, o oposto — a VM não tem equivalente em
`dart:isolate`, e exigem cabeçalhos COOP/COEP no servidor. Expor isso em
`forge_isolate` criaria uma construção sem significado no backend nativo.

### 3.6 Portas e canais

`SendPort<T>` / `ReceivePort<T>` acompanham a forma de `dart:isolate`, com dois desvios:

- **São tipados.** O argumento de tipo é o que fecha o mundo da serialização (§3.4). A
  `SendPort` da VM é sem tipo; o invólucro estreita, e estreita igual nos quatro alvos.
- **Worker↔Worker é direto.** O Dart 1 roteava toda mensagem entre dois workers pelo
  thread principal (`_WorkerSendPort.send`, isolate_helper.dart:1256–1263), pagando duas
  serializações e um salto pelo thread que a biblioteca existia para desocupar. Hoje
  `MessageChannel` produz um par de `MessagePort`, e `MessagePort` é transferível: o
  canal vai por transferência para os dois workers e a conversa não toca o thread
  principal. Na VM, `SendPort` já é direto. **Mesma semântica, sem o salto** — melhoria
  de desempenho, não de comportamento.

A identidade de porta segue a ideia do Dart 1 — tripla `(isolate, porta, canal)` com
`operator ==` e `hashCode` sobre ela (`_WorkerSendPort`, L1274/L1281) — porque
`postMessage` não preserva identidade de objeto e uma porta tem de sobreviver a ida e
volta comparando igual.

### 3.7 Como o worker acha o código: a URL

Na Web, `guard` precisa de uma URL para `new Worker(...)`. Três casos:

- **dart2js** — a URL é o próprio bundle, como no Dart 1. Obtida do
  `document.currentScript.src` capturado **durante** a avaliação do script, que é quando
  `main` roda; o Dart 1 fazia o equivalente pelo global embutido `CURRENT_SCRIPT`
  (isolate_helper.dart:784) e tinha o fallback por pilha de exceção
  (`computeThisScriptFromTrace`, L795) que este desenho não pretende repetir. A API
  oferece sobreposição explícita para quem usa bundler.
- **DDC** — ver §3.9; é o ponto fraco.
- **DartForge** — a URL é o chunk daquele entrypoint, resolvido por uma tabela gerada na
  compilação.

O gancho para a camada 3 injetar essa tabela deve ser Dart comum que degrade para o caso
dart2js quando o compilador não o reconhece — por exemplo uma função privada que devolve
`null` por padrão e cujo corpo o DartForge substitui pela consulta à tabela gerada. Em
dart2js e DDC ela devolve `null` e o código recai na URL do bundle. **A URL não é
semântica observável**: os dois arquivos implementam o mesmo entrypoint.

Nota sobre o mecanismo: `@pragma` seria o veículo natural para esse gancho, e é aceita e
ignorada por dart2js, DDC e VM quando o nome é desconhecido. Hoje o parser do DartForge
**rejeita** `@pragma` explicitamente (`crates/parser/src/lib.rs:2518–2535`, mensagem
`"@pragma directs the compiler and cannot be ignored; it is not implemented"`). Ou o
gancho usa outro veículo, ou essa rejeição precisa ser revista. Ver §5.

### 3.8 Chunk de worker: por que a arquitetura atual do DartForge ajuda

A promessa da camada 3 é "runtime mínimo e só as dependências do entrypoint". Vale
registrar que o emissor atual já é construído do jeito que essa promessa exige, e isso
não é coincidência arquitetural pequena.

`crates/codegen/src/lib.rs` mantem em `struct Output` (L46–L107) cerca de trinta flags de
"runtime usado" — `strings_used`, `runtime_types_used`, `async_used`, `late_used`,
`records`, `shift_used`, `spread_used`, `const_instance_used`, entre outras — e injeta
cada bloco de runtime no início do buffer **só quando a flag correspondente foi ligada**
(L336–L346 para `records.js`, `async.js`, `types.js`; L216 para `core.js`). Há até um
caso de granularidade fina: `strings.rs:18` extrai de `core.js` apenas a fatia entre os
marcadores `// >>> $dartforgeString` e `// <<< $dartforgeString`, para não arrastar o
runtime de coleções inteiro.

Isso significa que **"runtime mínimo por chunk" é uma consequência automática de emitir
cada chunk com seu próprio `Output`**. É a diferença estrutural com o Dart 1, cujo
emissor produzia um programa e o Worker carregava esse programa.

O que falta é outra coisa, e é estrutural: `emit` tem assinatura
`pub fn emit(module: &Module<'_>) -> String` (L146) — **uma** unidade de saída — e esse
contrato atravessa `crates/linker` (`emit: impl FnOnce(&Module) -> Result<String, _>`,
L211) e o CLI (`write_new`, `crates/cli/src/main.rs:520`). Ver §5.

### 3.9 Depuração no mesmo thread é opt-in, nunca fallback

DDC é o caso difícil. Sua saída é um conjunto de módulos carregados por um bootstrap, e
carregar esse bootstrap dentro de um escopo de Worker não é garantido pelo ambiente de
desenvolvimento.

A tentação é clara e tem de ser recusada: se o Worker não puder ser criado, rodar o
entrypoint no thread principal. **Isso é exatamente a divergência que a regra de projeto
proíbe.** Um `contador++` global chegaria a 1 no fallback e a 0 no Worker, e o defeito
aparece ao trocar de compilador — o exemplo textual do PLANO.md.

Portanto:

- Falha em criar o isolate → **lança**. `UnsupportedError`, com mensagem dizendo por quê.
- Existe um modo de mesmo thread, mas ele é **explícito, nomeado e documentado como não
  equivalente**: `WorkerIsolate.debugRunInline()`. É o `_Manager.useWorkers` do Dart 1
  (isolate_helper.dart:207–211, *"Set to false for debugging/testing"*), com o nome
  dizendo a verdade.
- Ainda assim, esse modo **clona a mensagem**, como fazia `_NativeJsSendPort.send`
  (L1229). Sem compartilhamento de estado *através das mensagens*; o que se perde é só o
  isolamento do estado global.

---

## Parte 4 — Tabela de comportamento por compilador

A coluna que importa é a última linha: **a diferença é de desempenho e de bytes, nunca de
semântica.**

| Aspecto | DartForge (JS) | dart2js | DDC | Dart VM / AOT |
|---|---|---|---|---|
| `WorkerIsolate.run` executa em | Web Worker dedicado | Web Worker dedicado | Web Worker dedicado | isolate da VM |
| Memória do entrypoint | **isolada** | **isolada** | **isolada** | **isolada** |
| Estado global visto pelo entrypoint | novo, zerado | novo, zerado | novo, zerado | novo, zerado |
| Entrypoints aceitos | top-level/estático em `WorkerTask` | idem | idem | idem |
| Código carregado pelo isolate | chunk = fecho transitivo do entrypoint + runtime mínimo | **bundle inteiro** (`main.dart.js`) | bootstrap + módulos do app | mesma imagem do processo |
| Quem decide onde executa | a chamada de API | a chamada de API | a chamada de API | a chamada de API |
| Efeito de `@WorkerEntrypoint()` | diagnóstico e raiz de chunk | nenhum (ignorada) | nenhum (ignorada) | nenhum (ignorada) |
| Serialização de instância de classe | tabela estática por tipo, tag inteira | envelope genérico sobre clone estruturado | idem dart2js | regras de `dart:isolate` |
| Cópias sobre o volume de bytes | 1 (clone do navegador) | 1 | 1 | 1 |
| Cópias extras em Dart | O(nº de instâncias) | O(nº de instâncias) | O(nº de instâncias) | 0 |
| `TransferableBuffer` | `postMessage(msg,[ab])` | `postMessage(msg,[ab])` | `postMessage(msg,[ab])` | `TransferableTypedData` |
| Posse após transferir | origem inutilizável | origem inutilizável | origem inutilizável | origem inutilizável |
| Exceção no uso após transferir | `StateError` | `StateError` | `StateError` | `StateError` |
| Ciclos e compartilhamento na mensagem | preservados | preservados | preservados | preservados |
| Worker ↔ Worker | direto (`MessagePort`) | direto (`MessagePort`) | direto (`MessagePort`) | direto (`SendPort`) |
| Tree shaking | preservado; corpos de entrypoint saem do chunk principal quando não usados nele | preservado | inexistente (DDC não faz) | preservado no AOT |
| `main` do programa é reescrito? | não | não | não | não |
| Falha em criar o isolate | `UnsupportedError` | `UnsupportedError` | `UnsupportedError` | `UnsupportedError` |
| **Diferença observável por um programa** | **nenhuma** | **nenhuma** | **nenhuma** | **nenhuma** |

Para comparação, a mesma tabela aplicada ao **Dart 1** dá: `main` reescrito (sim),
entrypoint identificado por símbolo do compilador, Worker carrega o bundle inteiro,
worker↔worker pelo thread principal, duas cópias em Dart, minificação limitada por
`getGlobalFromName`. Cinco das linhas mudam — e nenhuma delas é semântica.

---

## Parte 5 — Limites: o que não será implementado, e por quê

### 5.1 `dart:mirrors` — não

Ver [Recomendação sobre mirrors](#recomendação-sobre-mirrors).

### 5.2 `Isolate.spawnUri` — não

Significa "carregue e execute um programa Dart arbitrário desta URL". Exigiria compilador
em tempo de execução ou um artefato pré-compilado por **outra** compilação — e, nesse
caso, a tabela de tags de tipo (§3.4) e o protocolo de portas não têm como concordar. O
Dart 1 só sustentava isso por convenção de nome de arquivo (`spawn`, L979:
`if (uri.endsWith(".dart")) uri += ".js"`) e recusava sem workers
(`_startNonWorker`, L1045: `"Currently spawnUri is not supported without web workers."`).
Se algum dia houver necessidade, que seja uma API explicitamente distinta, não um
`spawnUri`.

### 5.3 Enviar closure, tear-off de instância ou `Function` dinâmico — não

Nenhuma plataforma reconstrói ambiente de captura a partir de bytes. `DataCloneError` na
Web para qualquer `IsCallable`; `Closures can't be transmitted:` no Dart 1
(isolate_serialization.dart:162–165). A restrição vale também na VM, para que a API seja
uma só (§3.3).

### 5.4 `pause` / `resume` / `startPaused` / prioridades de `kill` — não

Suspender um isolate exige que o laço de eventos dele esteja sob nosso controle; era para
isso que o Dart 1 mantinha um `_EventLoop` próprio, em Dart, por manager
(isolate_helper.dart:610), com fila `delayedEvents` e o campo `isPaused` no
`_IsolateContext`. Reimplementar o laço de eventos em Dart significa não usar o do
navegador, e é uma fatia grande de runtime em cada chunk — parte direta do "overhead
substancial" citado em §2.2.

Oferecemos `terminate()`, mapeado para `Worker.terminate()` e para `Isolate.kill()`.
As prioridades `beforeNextEvent` e `immediate` **não** são expostas, porque
`Worker.terminate()` não as distingue: expor uma diferença que só um alvo respeita seria
divergência.

### 5.5 `SharedArrayBuffer` e atomics — não

Não é semântica de isolate, é o contrário dela. Sem equivalente em `dart:isolate`, e
dependente de COOP/COEP no servidor. Ficaria sem significado no backend nativo.

### 5.6 Reescrever `main` — não

O Dart 1 envolvia `main` em `startRootIsolate` sempre que a biblioteca estivesse em uso
(`main_call_stub_generator.dart:38`). Mudar o caminho de inicialização de todo programa
que importa uma biblioteca é custo cobrado de quem não usa o recurso. `guard` põe a
decisão no código-fonte (§3.2).

### 5.7 Valores tipados de `dart:html` na API portátil — não, e sim num anexo web-only

`Blob`, `File` e `ImageData` atravessam clone estruturado (§2.5) e devem passar intactos.
Mas tipá-los na API portátil tornaria o pacote não compilável para VM. A solução é um
ponto de extensão opaco na camada portátil e o tipo concreto numa parte declaradamente
web-only, selecionada por import condicional — o mesmo padrão que `dart:html` já impõe.
Quem usa, escolheu não ser portátil, e escolheu de forma visível.

### 5.8 Fallback silencioso para o thread principal — nunca

§3.9. É a violação direta da regra de projeto.

### 5.9 O que falta no DartForge antes de qualquer implementação

Levantamento do código atual, com caminho e linha, porque o desenho só vale se o caminho
de implementação for real:

| Peça necessária | Estado hoje | Onde |
|---|---|---|
| Saída em múltiplas unidades | **não existe.** `emit` devolve `String`; o contrato atravessa linker e CLI | `crates/codegen/src/lib.rs:146`; `crates/linker/src/lib.rs:211`; `crates/cli/src/main.rs:520` |
| Runtime mínimo por unidade | **existe**, por ~30 flags em `Output` + injeção condicional | `crates/codegen/src/lib.rs:46–107`, L216, L336–L346; `crates/codegen/src/strings.rs:18` |
| Fecho transitivo com raízes arbitrárias | **quase.** BFS por worklist pronta; raízes fixas em `main` + extensions | `crates/optimizer/src/tree_shake.rs:18`, raízes em L39–L45 |
| Anotação nova (`@WorkerEntrypoint`) | **não existe.** `AnnotationKind` tem 4 variantes; exige braço no parser e nome no linker | `crates/syntax/src/lib.rs:738–747`; `crates/parser/src/lib.rs:2498`; `crates/linker/src/lib.rs:1043` |
| `@pragma` como veículo de gancho | **rejeitada explicitamente** | `crates/parser/src/lib.rs:2518–2535` |
| Reconhecer `WorkerIsolate.*` como intrínseco | **não existe.** Intrínsecos exigem parser + semantic + `Symbol::intrinsic` no linker + braço no `match` do codegen | `crates/semantic/src/asynchronous.rs:136`; `crates/linker/src/lib.rs:349–367`; `crates/codegen/src/lib.rs:1940–1971` |
| `dart:isolate` / `dart:js_interop` reconhecidas | **não.** Allowlist tem só `dart:core`, `dart:async`, `dart:ffi` | `crates/packages/src/lib.rs:253–271` |
| Imports condicionais | **existe** | [historico/IMPLEMENTACAO-15.md](historico/IMPLEMENTACAO-15.md) |
| Resolução `package:` | **existe** | [historico/PACKAGES.md](historico/PACKAGES.md) |
| Carregamento diferido (`deferred`) | **rejeitado.** Compartilha maquinaria com chunking | `crates/packages/src/lib.rs:1037–1044` |

---

## Recomendação sobre mirrors

**Não implementar `dart:mirrors`.** O substituto é reflexão estática gerada em
compilação, dirigida por anotação — e o desenho dela **não pertence a este documento**.
Ele está em
[historico/MACROS-ARQUITETURA.md, "Reflexão estática: o que herdar da proposta de macros"](historico/MACROS-ARQUITETURA.md#reflexão-estática-o-que-herdar-da-proposta-de-macros),
que fixa o contrato de fases, a decisão de gerar em processo em Rust em vez de hospedar
macro de usuário por RPC, e a forma da anotação. Esta seção cobre só o que é específico
da Web e o que a interseção com isolates acrescenta.

### O alvo de API é `package:reflectable`

Não há API inventada aqui. A referência é
[`package:reflectable`](https://pub.dev/packages/reflectable), repositório
`google/reflectable.dart`, cujo desenvolvedor principal é Erik Ernst (`eernstg`).
Pontos confirmados na pesquisa, porque o desenho depende deles:

- Gera descritores estáticos via `build_runner` — o builder é o pacote irmão
  `reflectable_builder` — e o programa **importa o código gerado**, num arquivo cujo nome
  é o da biblioteca principal com `.reflectable` acrescentado.
- Exige **capacidades declaradas na anotação**, com classes e constantes nomeadas:
  `InstanceInvokeCapability`, `StaticInvokeCapability`, `TopLevelInvokeCapability`,
  `NewInstanceCapability`, `DeclarationsCapability`, `MetadataCapability`,
  `TypeCapability`, `TypeRelationsCapability`, `LibraryCapability`, `UriCapability`,
  `SuperclassQuantifyCapability`, `TypeAnnotationQuantifyCapability`, e as constantes
  `declarationsCapability`, `metadataCapability`, `typeCapability`,
  `instanceInvokeCapability`, `invokingCapability`, `typingCapability`,
  `reflectedTypeCapability`, entre outras.
- A razão declarada para as capacidades é exatamente tamanho: *"the desired level of
  support for reflection is specified explicitly and statically, and any usage of
  reflection at run time must stay within the boundaries thus specified; in return for
  statically confining the required reflection support, programs can be smaller"*. E a
  descrição do repositório diz que o uso de reflexão dinâmica *"is constrained in order to
  ensure that the specialized code can be generated and will have a reasonable size"*.
- A superfície imita `dart:mirrors` de propósito — *"using this package and using
  `dart:mirrors` is very similar"* — com limitações declaradas, entre elas não refletir
  sobre funções e closures nem sobre declarações privadas.

Há ainda um relato dos próprios autores sobre o problema, útil como citação:
*"Reflection in Dart: A Cautionary Experience"*, workshop META 2016, SPLASH 2016.

Mirar essa API é o que faz a anotação atender à regra de equivalência semântica: ela vem
de pacote pub com builder `build_runner`, então dart2js e DDC geram os descritores pela
etapa de build e o DartForge gera a mesma API nativamente, em processo. Mesmo programa,
mesma semântica; um dos dois só constrói mais rápido. O raciocínio completo está em
historico/MACROS-ARQUITETURA.md; ele é o mesmo que §3.2 aplica a `guard`.

### O que é específico da Web

1. **A incompatibilidade é estrutural, e a implementação do Dart 1 dizia isso no código.**
   `disableTreeShaking() => preserveNames()` em `js_mirrors.dart:73`, com o comentário
   *"No-op method that is called to inform the compiler that tree-shaking needs to be
   disabled"*, chamada por `computeLibrariesByName` (L141), `reflectClassByName` (L630),
   `reflectMixinApplication` (L822) e `_getCachedInvocation` (L1063). Do outro lado,
   `mirrors_data.dart:216–224` liga `mustDisableTreeShaking` e `mustPreserveNames`.
   Enumerar bibliotecas, refletir classe por nome, resolver mixin e invocar membro por
   símbolo — **cada uma desliga a otimização do programa inteiro.** Isto não é
   argumentação: é o mecanismo, com nome de função.
2. **A razão oficial da remoção é a mesma tese, dita pelo time do Dart:** *"to provide a
   full fidelity experience, Dart JavaScript compilers included a substantial amount of
   type information while also losing the ability to do precise tree-shaking. This leads
   to dramatically increased output size and execution time."* A recomendação oficial que
   acompanha é geração de código — `package:build`, `package:source_gen`.
3. **Recusar mirrors na Web é divergência zero.** `dart:mirrors` não está nas seções
   `dart2js` nem `dartdevc` de `libraries.json` em 3.6.2 — um programa que a importa
   **não compila** para a Web hoje. No perfil nativo, `dart.library.mirrors` é ausente em
   AOT ([historico/ENVIRONMENT-REFERENCIAS.md](historico/ENVIRONMENT-REFERENCIAS.md)) e o backend nativo do
   DartForge reproduz AOT. Em nenhum dos dois casos recusar mirrors muda o significado de
   um programa que hoje compila. É a diferença entre este item e §5.2: recusar `spawnUri`
   estreita uma API que a plataforma teria como sustentar; recusar mirrors não estreita
   nada que a Web ofereça.
4. **A válvula de escape do Dart 1 na Web já era declarativa, e falhou por ser manual.**
   `@MirrorsUsed` (`sdk/lib/mirrors/mirrors.dart:1242`, marcada **EXPERIMENTAL**) pedia
   ao programador listar símbolos e alvos antecipadamente, e a própria documentação da
   classe admitia que *"the size and performance of the output can suffer from use of
   reflection"*. As capacidades de `package:reflectable` são a mesma ideia — mundo fechado
   por declaração — com a lista sustentada por ferramenta em vez de mantida à mão. O
   precedente histórico e o alvo moderno são a mesma estratégia, vinte anos de
   ferramental à parte.

### A interseção com isolates

A serialização de §3.4 **é** um caso de reflexão estática: `_forgeWrite$T` /
`_forgeRead$T` são descritores gerados para um conjunto declarado de tipos. Devem sair do
mesmo gerador e da mesma tabela descritos em historico/MACROS-ARQUITETURA.md, não de um segundo
gerador paralelo.

Duas restrições que só existem por causa da Web e que o gerador geral precisa acomodar:

- **A tabela é por unidade de saída, não por programa.** Cada chunk carrega descritores
  só dos tipos que atravessam as portas daquele entrypoint (§3.4). Uma tabela global —
  `ALL_CLASSES`, `TYPE_INFORMATION`, anotadas em `shared/embedded_names.dart` como *"only
  used by reflection"* — é justamente o que se está evitando.
- **A identidade de tipo no protocolo é inteiro, não nome.** `o.constructor.name` (§1.6)
  amarraria o protocolo ao esquema de nomes e mataria a minificação. Reflexão estática
  pode usar nomes internamente; a serialização entre agentes não pode.

E a assimetria que historico/MACROS-ARQUITETURA.md nomeia aparece aqui na forma mais concreta
possível: `@JsonCodable` gera código **para a classe anotada**, e o gerador sabe sobre o
que emitir. Reflexão recebe perguntas **por nome, em execução** — `invoke('foo')` com
`'foo'` calculado — e o gerador não sabe quais nomes serão pedidos. Na serialização de
isolates o problema aparece como mensagem de tipo `dynamic`: o conjunto de tipos que
podem atravessar a porta deixa de ser fechado, e a única saída sem tabela global é
diagnóstico em compilação. **O conjunto é declarado, nunca inferido** — em reflexão pelas
capacidades, em isolates pelos argumentos de tipo de `WorkerTask` e `SendPort`.

### O que se perde, sem atenuar

- **Enumerar o programa.** `MirrorSystem.findLibrary`, `MirrorSystem.libraries`,
  `ClassMirror.declarations` sobre tudo. Não é questão de esforço: um sistema dirigido por
  anotação é fechado por construção, e perguntar "quais bibliotecas existem" é a pergunta
  que nenhum mundo fechado responde. Um navegador de objetos genérico não é construível.
- **Refletir objeto arbitrário não anotado.** Mesma razão. Consequência prática: um pacote
  que reflete tipos definidos pelo *usuário* do pacote passa a exigir que o usuário anote
  e declare capacidades.
- **Invocar membro com nome calculado em execução** fora do conjunto declarado.
- **`noSuchMethod` que imprime nomes reais**, que depende de `preserveNames` — exatamente
  o que se quer poder minificar.
- **Reflexão sobre funções, closures e declarações privadas**, que `package:reflectable`
  também não oferece.
- **Compatibilidade de fonte com código que importa `dart:mirrors`.** Não compila. Na Web
  não é regressão (item 3); em cenário de VM JIT, é.
- **Custo de ferramenta.** Descritores exigem geração, cache e invalidação — complexidade
  que mirrors não tinha e que historico/MACROS-ARQUITETURA.md trata nas três regras de
  incrementalidade.

### O diagnóstico

`import 'dart:mirrors'` não deve receber a recusa genérica de biblioteca desconhecida do
allowlist em `crates/packages/src/lib.rs:253–271`. Precisa de diagnóstico próprio que
**nomeie o substituto**: dizer que reflexão em tempo de execução é incompatível com tree
shaking por construção, que a Web oficial também não a oferece, e apontar
`package:reflectable` com capacidades declaradas. Uma recusa que só recusa transfere ao
programador o trabalho de descobrir que existe caminho.

---

## Protótipo: por que não há

O enunciado admitia um protótipo mínimo — reconhecer `WorkerIsolate.run` no compilador e
emitir um chunk separado — com edições pontuais restritas a `crates/codegen/**` e
`crates/semantic/**`. **Não foi feito, e a razão é que ele não é implementável nesse
conjunto de arquivos.** As três peças mínimas caem, cada uma, fora dele:

1. **Aceitar a anotação e a API no fonte.** `@WorkerEntrypoint()` não está na allowlist de
   metadata ignorável (`crates/parser/src/lib.rs:5545`) e `AnnotationKind` tem quatro
   variantes fixas (`crates/syntax/src/lib.rs:738–747`); adicionar exige um braço em
   `parser::metadata` (`crates/parser/src/lib.rs:2498`). `@pragma`, o veículo alternativo,
   é rejeitada em `crates/parser/src/lib.rs:2518–2535`. `crates/parser` está fora do
   conjunto permitido.
2. **Calcular o fecho transitivo a partir de outra raiz.** O tree shaker fixa as raízes
   em `main` + extensions (`crates/optimizer/src/tree_shake.rs:39–45`). Generalizar é uma
   mudança pequena — e `crates/optimizer` não está no conjunto permitido. Sem isso, "o
   chunk do entrypoint" seria o programa inteiro, ou seja, o modelo do Dart 1: emitir isso
   e chamar de protótipo de otimização seria enganoso.
3. **Emitir duas unidades de saída.** `emit` devolve `String`
   (`crates/codegen/src/lib.rs:146`), e o contrato atravessa
   `crates/linker/src/lib.rs:211` e `crates/cli/src/main.rs:520`. `crates/linker` está
   fora do conjunto permitido.

Um "protótipo" que contornasse os três — reconhecendo um nome mágico no codegen e
cuspindo um segundo arquivo com o programa duplicado — demonstraria o oposto da tese
deste documento, que é justamente que o chunk contém **só** o fecho do entrypoint.
Preferi não escrever código que argumenta contra o desenho.

Nenhum arquivo de código foi tocado neste incremento. As alterações são
`docs/ISOLATES-WEB.md` (novo) e a entrada `dart-sdk-1.24.3` em
`docs/references-manifest.json`.

### Ordem sugerida para a implementação real

Cada etapa é verificável sozinha, e nenhuma depende de acertar a seguinte:

1. `package:forge_isolate` como **Dart puro**, com as implementações de VM e de Web, sem
   nenhuma mudança no compilador. Validar com `dart compile js`, `dart run` e DDC. Se
   esta etapa não passar, o desenho está errado e nada do resto importa.
2. Generalizar as raízes de `tree_shake` para um conjunto (`crates/optimizer`).
3. Tornar `emit` capaz de devolver N unidades, propagando por linker e CLI.
4. Reconhecer `guard` / `run` / `spawn` como intrínsecos e gerar a tabela de chunks.
5. Serialização estática dirigida por tipo, com o hash de build no handshake.
6. Anotação `@WorkerEntrypoint()` — **por último**, porque é a única peça sem efeito
   semântico e portanto a menos urgente.

A ordem tem uma propriedade útil: depois da etapa 1 o recurso já **funciona** nos quatro
alvos; as etapas 2 a 5 só reduzem bytes.

---

## Referências

Clone `references/dart-sdk-1.24.3`, tag `1.24.3`, commit
`0b0b41ef25358dd77d63b4d02718287c26f8e408`, BSD-3-Clause. Leitura por
`git show HEAD:<caminho>`.

- `sdk/lib/_internal/js_runtime/lib/isolate_helper.dart` — `_Manager` (176),
  `useWorkers` (211), `_nativeInitWorkerMessageHandler` (255),
  `maybeCloseWorker` (286), `_IsolateContext` (294), `_EventLoop` (610),
  `_MainManagerStub` (718), `IsolateNatives` (755), `enableSpawnWorker` (763),
  `computeThisScript` (783), `computeThisScriptFromTrace` (795),
  `_processWorkerMessage` (841), `handleSpawnWorkerRequest` (901),
  `_getJSFunctionName` (939), `spawnFunction` (950), erro
  `only top-level functions can be spawned.` (955), `spawnUri` (962), `spawn` (974),
  `_startWorker` (1004), `_startNonWorker` (1034), `_startIsolate` (1067),
  `_spawnWorker` (1106), `new Worker(#)` (1116), `_BaseSendPort` (1197),
  `_NativeJsSendPort` (1217), `_WorkerSendPort` (1249), `RawReceivePortImpl` (1286)
- `sdk/lib/_internal/js_runtime/lib/isolate_serialization.dart` — `_clone` (21),
  `_Serializer` (27), `serialize` (35), `serializeByteBuffer` (79),
  `serializeJSObject` (127), `serializeClosure` (162), `serializeDartObject` (170),
  `_Deserializer` (180), `deserializeDartObject` (374)
- `sdk/lib/_internal/js_runtime/lib/isolate_patch.dart` — `Isolate.spawn` (39),
  `Isolate.spawnUri` (84)
- `sdk/lib/_internal/js_runtime/lib/js_mirrors.dart` — `disableTreeShaking` (73),
  `preserveMetadata` (77), `preserveUris` (81), `preserveLibraryNames` (85),
  `computeLibrariesByName` (140), `reflectClassByName` (626),
  `reflectMixinApplication` (821), `_getCachedInvocation` (1057)
- `sdk/lib/_internal/js_runtime/lib/mirrors_patch.dart` — patch integral
- `sdk/lib/_internal/js_runtime/lib/js_helper.dart` —
  `createDartClosureFromNameOfStaticFunction` (207), gravação de `$static_name` (2635)
- `sdk/lib/_internal/js_runtime/lib/shared/embedded_names.dart` —
  `STATIC_FUNCTION_NAME_PROPERTY_NAME` (48), `GLOBAL_FUNCTIONS` (250),
  `STATIC_FUNCTION_NAME_TO_CLOSURE` (263), `ALL_CLASSES` (270),
  `TYPE_INFORMATION` (275)
- `sdk/lib/mirrors/mirrors.dart` — `MirrorsUsed` (1242)
- `pkg/compiler/lib/src/js_emitter/startup_emitter/fragment_emitter.dart` —
  `getGlobalFromName` (294), `hasIsolateSupport` (1399),
  `STATIC_FUNCTION_NAME_TO_CLOSURE` (1404), `CLASS_ID_EXTRACTOR` (1415),
  `CLASS_FIELDS_EXTRACTOR` (1418), `INSTANCE_FROM_CLASS_ID` (1437)
- `pkg/compiler/lib/src/js_emitter/main_call_stub_generator.dart` —
  `isIsolateInUse` (38)
- `pkg/compiler/lib/src/js_backend/mirrors_data.dart` — marcadores (216–224)
- `pkg/compiler/lib/src/js_backend/backend_usage.dart` — `isIsolateInUse` (38, 274)

Clone `references/dart-sdk`, tag `3.6.2`, commit
`b0cc5495e0f5e8ae150825a5352e708cb49e65ff`:

- `sdk/lib/libraries.json` — `isolate` com `"supported": false` em `dart2js` e
  `dartdevc`; `mirrors` só em `vm`
- `sdk/lib/_internal/js_runtime/lib/isolate_patch.dart` — `UnsupportedError` em tudo
- `sdk/lib/isolate/isolate.dart` — `Isolate.run` (253), `TransferableTypedData` (1044),
  contrato de uso único (1052–1057), `_RemoteRunner` (1064)

Plataforma Web: especificação HTML, dados estruturados — `StructuredSerializeInternal`,
`[Serializable]`, `[Transferable]`, `[[Detached]]`.

Anúncio da remoção: *"Dart2 Breaking Change: Removing web support for dart:mirrors and
dart:isolate"*, lista `dart-announce`. Metabug `dart-lang/sdk#30538`; deprecação
`dart-lang/sdk#32684`.

Reflexão estática: `package:reflectable` (`https://pub.dev/packages/reflectable`,
repositório `google/reflectable.dart`, desenvolvedor principal Erik Ernst), pacote irmão
`reflectable_builder`, biblioteca `reflectable.capability`. Relato dos autores:
*"Reflection in Dart: A Cautionary Experience"*, META 2016, SPLASH 2016.

Documentos internos relacionados: [PLANO.md](../PLANO.md) (regra de equivalência
semântica),
[historico/MACROS-ARQUITETURA.md](historico/MACROS-ARQUITETURA.md#reflexão-estática-o-que-herdar-da-proposta-de-macros)
(desenho da reflexão estática, contrato de fases e forma da anotação),
[historico/IMPLEMENTACAO-15.md](historico/IMPLEMENTACAO-15.md) (imports condicionais),
[historico/ENVIRONMENT-REFERENCIAS.md](historico/ENVIRONMENT-REFERENCIAS.md) (matriz de `dart.library.*`),
[historico/IMPLEMENTACAO-23.md](historico/IMPLEMENTACAO-23.md) (tree shaking),
[historico/PACKAGES.md](historico/PACKAGES.md), [historico/MODULES.md](historico/MODULES.md),
[DART2JS-REFERENCIA.md](DART2JS-REFERENCIA.md).
