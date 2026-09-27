# Natives pendentes: o que realmente falta ao backend nativo

Levantamento de 2026-09-27 sobre `crates/emit_native/src/nativos.rs` e o SDK
3.6.2 (`/opt/tc/dart-3.6.2/lib`). A tabela `NATIVOS` tinha **66** entradas
`pendente(...)`; depois do multicast, do `RawSynchronousSocket` e das
mensagens de controle (§1.2–1.4, agora implementados) e da observação de
arquivos (§1.1, implementada no Linux) restam **38**. Esse número **não** mede o que falta: a maior parte das
entradas é de natives que continuam declarados no SDK com a sobreposição
`sdk_nativo/`, mas que nenhum caminho de um programa Dart válido alcança,
porque o recurso público é servido pela sobreposição ou pelo próprio
lowering. Este documento separa as duas coisas.

## Como um native pendente se comporta

* `nativos.rs` lista **todo** native das `BIBLIOTECAS_DA_FONTE`
  (`crates/emit_native/src/sdk_modulo.rs:25`) já com a sobreposição; o teste
  `inventario::toda_native_da_fonte_tem_entrada` (`nativos.rs`, fim do
  arquivo) recusa native sem entrada **e** entrada que não é mais native.
  Por isso uma declaração `external` que a sobreposição mantém mas nunca
  chama continua na tabela como `pendente`.
* A chamada a um native pendente vira, no código do SDK compilado da fonte,
  `UnsupportedError("não suportado no backend nativo: native pendente `X`
  (Classe.membro)")` em tempo de execução
  (`crates/emit_native/src/lower/sdk_fonte.rs:561-563` →
  `FnBuilder::nao_suportado`, `crates/emit_native/src/lower/fn_builder.rs:316-343`;
  prefixo em `crates/emit_native/src/lib.rs:72`). O programa que não passa
  por ali compila e roda; `DARTFORGE_FONTE_NAO_SUPORTADO=1` lista, na
  compilação, cada construto que virou `UnsupportedError`.
* O SDK da fonte é o padrão (`sdk_modulo::sdk_da_fonte_pedido`,
  `sdk_modulo.rs:121-127`); `DARTFORGE_SDK_DA_FONTE=0` volta ao runtime por
  nome, que não tem `dart:io` nem FFI e não é considerado aqui.
* `libraries.json` da sobreposição troca **arquivos inteiros**; `file_patch.dart`,
  `socket_patch.dart`, `sync_socket_patch.dart`, `internal_patch.dart`,
  `type_patch.dart`, `errors_patch.dart`, `object_patch.dart`,
  `lib_prefix.dart`, `invocation_mirror_patch.dart` e `developer.dart` **não**
  são substituídos (`sdk_nativo/libraries.json`, chave `substitui`;
  `sdk_nativo/io/common_patch.dart:49-59` só reúne os `part` originais).

Classificação usada abaixo:

* **ausente** — API pública alcançável que lança `UnsupportedError` no
  backend nativo e funciona na VM.
* **servido por sobreposição** — a sobreposição reimplementa a API sem
  passar pelo native.
* **servido pelo lowering** — o compilador trata a construção de linguagem
  ou a chamada antes de chegar ao native.
* **inalcançável** — o native só é chamado pelo compilador/runtime da VM
  (entradas `vm:entry-point`), ou mora em biblioteca privada do SDK sem
  chamador; não há API pública que leve a ele.

## 1. APIs ausentes

### 1.1 `dart:io` — observação de sistema de arquivos

Natives: `FileSystemWatcher_CloseWatcher`, `_GetSocketId`, `_InitWatcher`,
`_IsSupported`, `_ReadEvents`, `_UnwatchPath`, `_WatchPath`
(`nativos.rs:346-352`).

* Sítios: `_internal/vm/bin/file_patch.dart:380-395` (`_FileSystemWatcher`).
* APIs públicas: `FileSystemEntity.watch()` (e portanto `File.watch`,
  `Directory.watch`, `Link.watch`) em `io/file_system_entity.dart:450-459`,
  e `FileSystemEntity.isWatchSupported` em `io/file_system_entity.dart:638-644`.
* Estado: **implementado no Linux** (inotify, como a VM:
  `crates/runtime/src/io_observador.rs`; o descritor vai ao laço de eventos
  como soquete interno pelo `_NativeSocket.watch` do patch);
  `corpus/nativo/34_observar_arquivos.dart`. No macOS (FSEvents) e no
  Windows (`ReadDirectoryChangesW`) falta: `isWatchSupported` é falso e
  `watch()` lança a `FileSystemException` "not supported" do patch.

### 1.2 `dart:io` — multicast de UDP

Natives: `Socket_JoinMulticast`, `Socket_LeaveMulticast` (`nativos.rs:612-613`).

* Sítios: `_internal/vm/bin/socket_patch.dart:1797-1802`
  (`_NativeSocket.nativeJoinMulticast`/`nativeLeaveMulticast`), chamados por
  `_NativeSocket.joinMulticast`/`leaveMulticast` (`socket_patch.dart:1713-1728`).
* APIs públicas: `RawDatagramSocket.joinMulticast` e
  `RawDatagramSocket.leaveMulticast`. O resto do UDP
  (`RawDatagramSocket.bind`, `send`, `receive`, `Socket_SendTo`/`RecvFrom`)
  é `runtime(...)`.
* Estado: **implementado** (`Socket_JoinMulticast`/`LeaveMulticast` em
  `crates/runtime/src/io_soquetes.rs`: `MCAST_JOIN_GROUP`/`MCAST_LEAVE_GROUP`
  no Linux, `IP_ADD_MEMBERSHIP`/`IPV6_JOIN_GROUP` nos outros);
  `corpus/nativo/31_udp_multicast.dart`.

### 1.3 `dart:io` — mensagens de controle e `ResourceHandle` (Unix domain sockets)

Natives: `Socket_ReceiveMessage`, `Socket_SendMessage` (`nativos.rs:615`,
`617`); `SocketControlMessage_fromHandles`,
`SocketControlMessageImpl_extractHandles` (`nativos.rs:592-593`);
`ResourceHandleImpl_toFile`, `_toRawDatagramSocket`, `_toRawSocket`,
`_toSocket` (`nativos.rs:580-583`).

* Sítios: `socket_patch.dart:1744-1757` (`nativeReceiveMessage`/
  `nativeSendMessage`, usados por `readMessage` em `socket_patch.dart:1155-1165`
  e pelo envio em `socket_patch.dart:1295-1316`), `socket_patch.dart:2799-2806`
  (`_ResourceHandleImpl`), `socket_patch.dart:2811-2833`
  (`SocketControlMessage.fromHandles`, `_SocketControlMessageImpl.extractHandles`).
* APIs públicas: `RawSocket.readMessage`, `RawSocket.sendMessage`,
  `SocketControlMessage.fromHandles`, `SocketControlMessage.extractHandles`,
  `ResourceHandle.toFile`/`toSocket`/`toRawSocket`/`toRawDatagramSocket`
  (e `toReadPipe`/`toWritePipe`, que passam por `toFile`).
* As factories `ResourceHandle.fromFile`/`fromSocket`/`fromRawSocket`/
  `fromRawDatagramSocket`/`fromStdin` são Dart puro
  (`socket_patch.dart:2681-2719`) e funcionam; o que falha é converter o
  handle de volta e trafegá-lo pelo socket.
* Estado: **implementado** no Unix (`sendmsg`/`recvmsg` com `SCM_RIGHTS`,
  `crates/runtime/src/io_soquetes_unix.rs`; os objetos Dart pelos ajudantes
  `_dartforge*` de `sdk_nativo/io/common_patch.dart`);
  `corpus/nativo/33_mensagens_de_controle.dart`. Como na VM,
  `ResourceHandle.toSocket`/`toRawDatagramSocket` não são suportados (o
  native da VM devolve um `UnsupportedError`, que o retorno tipado vira
  `TypeError`; aqui a `UnsupportedError` é lançada) e o Windows não passa
  descritores.

### 1.4 `dart:io` — `RawSynchronousSocket`

Natives: os 11 `SynchronousSocket_*` (`nativos.rs:649-659`).

* Sítios: `_internal/vm/bin/sync_socket_patch.dart:303-323`
  (`_NativeSynchronousSocket`).
* API pública: `RawSynchronousSocket.connectSync` e todos os métodos da
  instância (`io/sync_socket.dart`). Nenhuma outra biblioteca do SDK usa
  `RawSynchronousSocket` (só os patches de JS/Wasm, que lançam).
* Estado: **implementado** (`SynchronousSocket_*` em
  `crates/runtime/src/io_soquetes.rs`, sobre `std::net::TcpStream`);
  `corpus/nativo/32_soquete_sincrono.dart`.

### 1.5 `dart:developer` — `NativeRuntime.writeHeapSnapshotToFile`

Native: `Developer_NativeRuntime_writeHeapSnapshotToFile` (`nativos.rs:287`).

* Sítio: `_internal/vm/lib/developer.dart:193-195`; API pública em
  `developer/developer.dart:148-174`.
* O resto de `NativeRuntime`/`dart:developer` alcançável tem native no
  runtime (`Developer_NativeRuntime_buildId`, `Developer_log`,
  `Developer_postEvent`, `Developer_registerExtension`…, `nativos.rs:286-300`).
* Estado: **ausente**. Não há heap snapshot no formato da VM para escrever.

## 2. Servido por sobreposição

### 2.1 `Finalizer` e `NativeFinalizer` — `FinalizerEntry_allocate`

* Sítio: `_internal/vm/lib/internal_patch.dart:394-399`
  (`FinalizerEntry.allocate`), chamado na VM só por
  `_internal/vm/lib/finalizer_patch.dart:49` e
  `_internal/vm/lib/ffi_native_finalizer_patch.dart:56`.
* Os dois arquivos são substituídos (`sdk_nativo/libraries.json`) por
  `sdk_nativo/core/finalizer_patch.dart` e
  `sdk_nativo/ffi/ffi_native_finalizer_patch.dart`, que guardam o anexo no
  coletor do runtime (`crates/runtime/src/finalizadores.rs`); nenhum deles
  chama `FinalizerEntry.allocate` (só o citam no comentário de cabeçalho).
  `internal_patch.dart` não é substituído, por isso a declaração continua na
  tabela.
* Teste: `corpus/nativo/14_finalizadores.dart`.

### 2.2 `dart:ffi` — callbacks nativos

Natives: `Ffi_nativeCallbackFunction`, `Ffi_nativeAsyncCallbackFunction`,
`Ffi_createNativeCallableListener`, `Ffi_createNativeCallableIsolateLocal`,
`Ffi_nativeIsolateLocalCallbackFunction` (`nativos.rs:335-336`, `343-345`).

* Sítios na sobreposição: `sdk_nativo/ffi/ffi_patch.dart:188-204` e `282-285`
  (declarações mantidas, sem chamador).
* APIs públicas: `Pointer.fromFunction`, `NativeCallable.isolateLocal`,
  `NativeCallable.listener`. Na VM o front-end reescreve essas chamadas
  estáticas para os natives; aqui o lowering troca a chamada pelos ajudantes
  `_dartforgeFromFunction`, `_dartforgeCallableLocal`,
  `_dartforgeCallableListener` da sobreposição
  (`crates/emit_native/src/lower/ffi.rs:390-409`;
  `sdk_nativo/ffi/ffi_patch.dart:214-280`), que usam os natives
  `DartForge_ffi_callback_*` do runtime (`crates/runtime/src/ffi_callbacks.rs`).
  Chamada dinâmica ou por tear-off de `Pointer.fromFunction` lança
  `UnsupportedError` também na VM (`sdk_nativo/ffi/ffi_patch.dart:298-309`,
  `ffi/ffi.dart:329-333`, `401-404`).
* Teste: `corpus/nativo/13_ffi_callbacks.dart`.
* Estado: **servido por sobreposição + lowering** (tabela: sobreposição).

## 3. Servido pelo lowering

### 3.1 `bool/int/String.fromEnvironment`, `bool.hasEnvironment`

Natives: `Bool_fromEnvironment`, `Bool_hasEnvironment`,
`Integer_fromEnvironment`, `String_fromEnvironment` (`nativos.rs:183-184`,
`493`, `644`).

* Sítios: `_internal/vm_shared/lib/bool_patch.dart:13-19`,
  `_internal/vm_shared/lib/integers_patch.dart:15-16`,
  `sdk_nativo/core/string_patch.dart:50`.
* O lowering intercepta toda criação pelas factories `const external` de
  ambiente e nunca chama o native
  (`crates/emit_native/src/lower/membros.rs:1376-1407`): `hasEnvironment` é
  `false`, `fromEnvironment` é o `defaultValue` (ou `false`/`0`/`""`). Vale
  também para o código do SDK (`const bool.fromEnvironment("dart.vm.product")`
  em `socket_patch.dart:1296`).
* **Limitação real:** a CLI nativa não aceita `-D`, então nenhuma chave está
  definida (`membros.rs:1376-1379`); `dart.vm.product` é `false`, como na VM
  JIT (`dart run`), e diferente do AOT de produto da VM.

### 3.2 `@Native` — `Ffi_GetFfiNativeResolver`

* Sítio: `sdk_nativo/ffi/ffi_patch.dart:1623-1637` (`Native._get_ffi_native_resolver`,
  usado só pelo `_ffi_resolver` `vm:entry-point`).
* O `external` com `@Native` é baixado por
  `FnBuilder::chamar_native_anotado` (`crates/emit_native/src/lower/ffi.rs:1283-1284`),
  que resolve o símbolo pelo runtime (`crates/runtime/src/ffi.rs:746-748`).
* Teste: `corpus/nativo/19_ffi_native_variaveis.dart`,
  `corpus/nativo/15_ffi_api_nativa.dart`.

### 3.3 `assert`

Natives: `AssertionError_throwNew`, `AssertionError_throwNewSource`
(`nativos.rs:181-182`).

* Sítios: `_internal/vm/lib/errors_patch.dart:38-50`, entradas
  `vm:entry-point` que o compilador da VM chama.
* `assert(c, m)` é baixado por `FnBuilder::lower_assert`
  (`crates/emit_native/src/lower/comandos.rs:9-43`), que cria o erro por
  `dartforge_assertion_error_new` (`crates/runtime/src/excecoes.rs:429-437`)
  com o ajudante `_dartforgeErroDeAssercao` da sobreposição
  (`sdk_nativo/core/identical_patch.dart:25`).
* Diferença conhecida: o texto é o de `AssertionError(m)`, sem a URL, a
  linha e o trecho da asserção que `_throwNew` extrai do script na VM.

### 3.4 Objetos `Type` — `AbstractType_*`, `Type_equality`

Natives: `AbstractType_equality`, `AbstractType_getHashCode`,
`AbstractType_toString`, `Type_equality` (`nativos.rs:178-180`, `667`).

* Sítios: `_internal/vm/lib/type_patch.dart:9-34`. As classes `_Type`,
  `_FunctionType`, `_RecordType`… só têm factory `_uninstantiable` (lança
  `"Unreachable"`); quem as instancia é a VM.
* No nativo, `runtimeType` e literais de tipo produzem o objeto canônico do
  runtime (`dartforge_rti_objeto_tipo`, `crates/runtime/src/tipos.rs:1086-1110`),
  de uma classe `_Type` própria com `==` e `toString` gerados pelo compilador
  (`crates/emit_native/src/lower/rti.rs:1052-1090`); `hashCode` é o de
  `Object` (o objeto é canônico por tipo).

### 3.5 Carga deferida — `LibraryPrefix_*`

Natives: `LibraryPrefix_isLoaded`, `_issueLoad`, `_loadingUnit`, `_setLoaded`
(`nativos.rs:531-534`).

* Sítios: `_internal/vm/lib/lib_prefix.dart:9-90` (`_LibraryPrefix`,
  `_loadLibrary`, `_checkLoaded`), que o front-end da VM insere para
  `import … deferred as p`.
* O lowering troca `p.loadLibrary()` por `_dartforgeCarregarBiblioteca`, um
  `Future` já completo (`crates/emit_native/src/lower/chamadas.rs:225-234`;
  `sdk_nativo/async/async_patch.dart:97`): o programa compilado já contém a
  biblioteca. Não há `_checkLoaded` antes de `loadLibrary`.

### 3.6 Testes de tipo e erros de tipo da VM

Natives: `Object_instanceOf`, `Object_simpleInstanceOf` (`nativos.rs:549`,
`551`), `TypeError_throwNew` (`nativos.rs:666`).

* Sítios: `_internal/vm/lib/object_patch.dart:53-62` e
  `_internal/vm/lib/errors_patch.dart:97-101`, todos `vm:entry-point` do
  código não otimizado da VM.
* `is`/`as` são baixados pela RTI do nativo (`crates/emit_native/src/lower/rti.rs`),
  e a falha cria o `_TypeError` da fonte por
  `dartforge_type_error_com_mensagem` (`crates/runtime/src/tipos.rs:1121-1130`;
  ajudante `_dartforgeErroDeTipo`, `sdk_nativo/core/identical_patch.dart:21`).

## 4. Inalcançáveis

Sem API pública e sem chamador no SDK com a sobreposição (ou só chamados
pelo compilador/runtime da VM):

| Native | Sítio | Por quê |
|---|---|---|
| `Internal_allocateObjectInstructionsStart`/`End`, `Internal_collectAllGarbage`, `Internal_deoptimizeFunctionsOnStack` | `_internal/vm/lib/internal_patch.dart:167-186` | `VMInternalsForTesting`, em `dart:_internal` |
| `Internal_extractTypeArguments` | `internal_patch.dart:37-39` | `extractTypeArguments` de `dart:_internal`; nenhum chamador no SDK fora da documentação de `internal/internal.dart:659-695` |
| `StringBase_intern` | `internal_patch.dart:443-444` | `intern` de `dart:_internal`, sem chamador no SDK |
| `Internal_loadDynamicModule` | `internal_patch.dart:446-455` | `loadDynamicModule` de `dart:_internal` (módulos dinâmicos da VM) |
| `Internal_nativeEffect` | `internal_patch.dart:160-164` | inserido pelo transformador de FFI da VM; sem chamador no SDK |
| `Internal_prependTypeArguments`, `Internal_boundsCheckForPartialInstantiation` | `internal_patch.dart:135-148` | `vm:entry-point` das closures genéricas da VM; o nativo tem as suas (`lower/closures.rs`) |
| `InvocationMirror_unpackTypeArguments` | `_internal/vm/lib/invocation_mirror_patch.dart:106-109` | só via `_InvocationMirror`/`NoSuchMethodError._withType`, criados pela VM; o nativo monta `Invocation` pela factory pública (`lower/nsm.rs:1-22`) e tem `NoSuchMethodError` próprio (id 1012, `lower/mod.rs:205`) |
| `Isolate_registerKernelBlob`, `Isolate_unregisterKernelBlob` | `sdk_nativo/isolate/isolate_patch.dart:701-717` | `createUriForKernelBlob`/`unregisterKernelBlobUri` são membros injetados pelo patch, ausentes de `isolate/isolate.dart`; recebem kernel, que o nativo não executa |

Ressalva: `dart:_internal` é privada da plataforma (a VM recusa
`import 'dart:_internal'` em código de usuário). O DartForge ainda **não
emite** `IMPORT_INTERNAL_LIBRARY` (o código existe só na tabela
`crates/diagnostics/src/codigos_g.rs:240`, sem emissor), então um programa
inválido que importe `dart:_internal` compila e, ao chamar um desses membros,
lança `UnsupportedError`. Para programas válidos, são inalcançáveis.

## 5. Resumo

| API pública | Natives pendentes | Estado | Evidência |
|---|---|---|---|
| `FileSystemEntity.watch`, `isWatchSupported` (`dart:io`) | 7 `FileSystemWatcher_*` | implementado no Linux; ausente no macOS e no Windows | `file_patch.dart:163-168`, `380-395`; `io/file_system_entity.dart:450-459`, `638-644` |
| `RawDatagramSocket.joinMulticast`/`leaveMulticast` | `Socket_JoinMulticast`, `Socket_LeaveMulticast` | implementado | `socket_patch.dart:1713-1728`, `1797-1802` |
| `RawSocket.readMessage`/`sendMessage`, `SocketControlMessage.fromHandles`/`extractHandles`, `ResourceHandle.to*` | `Socket_ReceiveMessage`, `Socket_SendMessage`, 2 `SocketControlMessage*`, 4 `ResourceHandleImpl_*` | implementado (Unix) | `socket_patch.dart:1155-1165`, `1295-1316`, `1744-1757`, `2740-2833` |
| `RawSynchronousSocket` | 11 `SynchronousSocket_*` | implementado | `sync_socket_patch.dart:303-323` |
| `NativeRuntime.writeHeapSnapshotToFile` (`dart:developer`) | `Developer_NativeRuntime_writeHeapSnapshotToFile` | ausente | `_internal/vm/lib/developer.dart:193-195` |
| `Finalizer`, `NativeFinalizer` | `FinalizerEntry_allocate` | servido por sobreposição | `sdk_nativo/core/finalizer_patch.dart`, `sdk_nativo/ffi/ffi_native_finalizer_patch.dart`; `corpus/nativo/14_finalizadores.dart` |
| `Pointer.fromFunction`, `NativeCallable.isolateLocal`/`listener` | 5 `Ffi_*Callback*`/`Ffi_createNativeCallable*` | servido por sobreposição (com redirecionamento no lowering) | `lower/ffi.rs:390-409`; `sdk_nativo/ffi/ffi_patch.dart:214-280`; `corpus/nativo/13_ffi_callbacks.dart` |
| `bool/int/String.fromEnvironment`, `bool.hasEnvironment` | `Bool_fromEnvironment`, `Bool_hasEnvironment`, `Integer_fromEnvironment`, `String_fromEnvironment` | servido pelo lowering (sem `-D`) | `lower/membros.rs:1376-1407` |
| `external` com `@Native` | `Ffi_GetFfiNativeResolver` | servido pelo lowering | `lower/ffi.rs:1283-1284`; `runtime/src/ffi.rs:746-748`; `corpus/nativo/19_ffi_native_variaveis.dart` |
| `assert` | `AssertionError_throwNew`, `AssertionError_throwNewSource` | servido pelo lowering (texto sem URL/linha) | `lower/comandos.rs:9-43`; `runtime/src/excecoes.rs:429-437` |
| `Type` (`runtimeType`, literais de tipo, `==`, `toString`) | 3 `AbstractType_*`, `Type_equality` | servido pelo lowering | `lower/rti.rs:1052-1090`; `runtime/src/tipos.rs:1086-1110` |
| `import … deferred as p` / `p.loadLibrary()` | 4 `LibraryPrefix_*` | servido pelo lowering | `lower/chamadas.rs:225-234`; `sdk_nativo/async/async_patch.dart:97` |
| `is`/`as` e `TypeError` | `Object_instanceOf`, `Object_simpleInstanceOf`, `TypeError_throwNew` | servido pelo lowering | `lower/rti.rs`; `runtime/src/tipos.rs:1121-1130` |
| `VMInternalsForTesting`, `extractTypeArguments`, `intern`, `loadDynamicModule`, `_nativeEffect` (`dart:_internal`) | 7 `Internal_*`, `StringBase_intern` | inalcançável | `_internal/vm/lib/internal_patch.dart` (§4) |
| closures genéricas da VM, `_InvocationMirror` | `Internal_prependTypeArguments`, `Internal_boundsCheckForPartialInstantiation`, `InvocationMirror_unpackTypeArguments` | inalcançável | §4 |
| `Isolate.createUriForKernelBlob` (membro injetado) | `Isolate_registerKernelBlob`, `Isolate_unregisterKernelBlob` | inalcançável | `sdk_nativo/isolate/isolate_patch.dart:701-717` |

Contagem dos 66 pendentes do levantamento:

* **implementados depois — 21:** 2 de multicast + 8 de mensagens de
  controle e `ResourceHandle` + 11 `SynchronousSocket_*` (saíram da tabela
  de pendentes);
* **implementados depois (só Linux) — 7:** `FileSystemWatcher_*` (inotify; no
  macOS e no Windows `isWatchSupported` é falso);
* **ausente — 1:** `writeHeapSnapshotToFile`;
* **servido por sobreposição — 6:** `FinalizerEntry_allocate` + 5 de
  callbacks FFI;
* **servido pelo lowering — 18:** 4 de ambiente + `Ffi_GetFfiNativeResolver`
  + 2 de `assert` + 4 de `Type` + 4 `LibraryPrefix_*` + 3 de `is`/`as`/
  `TypeError`;
* **inalcançável — 13:** 9 `Internal_*` + `StringBase_intern` +
  `InvocationMirror_unpackTypeArguments` + 2 de kernel blob.

Ou seja: a lacuna real do backend nativo nesta tabela são dois grupos de
`dart:io`/`dart:developer` (observação de arquivos e heap snapshot); os
outros 37 pendentes não correspondem a recurso público faltando.

## 6. Conferência

Para refazer a lista de pendentes e os sítios:

```sh
grep -o 'pendente("[A-Za-z_]*")' crates/emit_native/src/nativos.rs | wc -l
for n in $(grep -o 'pendente("[A-Za-z_]*")' crates/emit_native/src/nativos.rs \
           | sed 's/pendente("\(.*\)")/\1/'); do
  echo "== $n"; grep -rn --include='*.dart' "['\"]$n['\"]" "$SDK/lib" sdk_nativo
done
```

(`$SDK` = `/opt/tc/dart-3.6.2`.) Um native que sair de `pendente` para
`runtime` deve sair também daqui; um native novo que o teste de inventário
acusar deve entrar numa das seções acima.
