// Patch do `dart:isolate` para o backend nativo do DartForge (sobreposição
// `sdk_nativo/`): o `_internal/vm/lib/isolate_patch.dart` do SDK 3.6.2 com
// as portas e as capacidades sobre o runtime (`crates/runtime/src/
// portas.rs`) no lugar dos objetos nativos da VM. O resto é o da VM.

// Copyright (c) 2012, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

import "dart:_internal" show ClassID, VMLibraryHooks, patch;

import "dart:async"
    show Completer, Future, Stream, StreamController, StreamSubscription, Timer;

import "dart:collection" show HashMap;
import "dart:typed_data" show ByteBuffer, TypedData, Uint8List;

// O `timer_impl.dart` da VM (timers por porta e EventHandler) não entra:
// os timers do nativo são os do laço de eventos do runtime
// (`sdk_nativo/async/timer_patch.dart`, `crates/runtime/src/eventos.rs`).

@patch
class ReceivePort {
  @patch
  factory ReceivePort([String debugName = '']) =>
      new _ReceivePortImpl(debugName);

  @patch
  factory ReceivePort.fromRawReceivePort(RawReceivePort rawPort) {
    return new _ReceivePortImpl.fromRawReceivePort(rawPort);
  }
}

@patch
@pragma("vm:entry-point")
class Capability {
  @patch
  factory Capability() => new _Capability();
}

/// Uma capacidade: um id único no processo. A igualdade é pelo id, que a
/// cópia de uma mensagem preserva (entre isolados também).
@pragma("vm:entry-point")
class _Capability implements Capability {
  final int _id;

  _Capability() : _id = _nova();

  /// A capacidade de id conhecido (a de um isolado, que o runtime guarda).
  _Capability._comId(this._id);

  @pragma("vm:external-name", "DartForge_capacidade_nova")
  external static int _nova();

  bool operator ==(Object other) {
    return (other is _Capability) && _id == other._id;
  }

  int get hashCode => _id;
}

@patch
class RawReceivePort {
  /**
   * Opens a long-lived port for receiving messages.
   *
   * A [RawReceivePort] is low level and does not work with [Zone]s. It
   * can not be paused. The data-handler must be set before the first
   * event is received.
   */
  @patch
  factory RawReceivePort([Function? handler, String debugName = '']) {
    _RawReceivePort result = new _RawReceivePort(debugName);
    result.handler = handler;
    return result;
  }
}

final class _ReceivePortImpl extends Stream implements ReceivePort {
  _ReceivePortImpl([String debugName = ''])
      : this.fromRawReceivePort(new RawReceivePort(null, debugName));

  _ReceivePortImpl.fromRawReceivePort(this._rawPort)
      : _controller = new StreamController(sync: true) {
    _naoEnviavel(this);
    _controller.onCancel = close;
    _rawPort.handler = _controller.add;
  }

  SendPort get sendPort {
    return _rawPort.sendPort;
  }

  StreamSubscription listen(void onData(var message)?,
      {Function? onError, void onDone()?, bool? cancelOnError}) {
    return _controller.stream.listen(onData,
        onError: onError, onDone: onDone, cancelOnError: cancelOnError);
  }

  close() {
    _rawPort.close();
    _controller.close();
  }

  final RawReceivePort _rawPort;
  final StreamController _controller;
}

/// Marca a classe de `objeto` como não enviável numa mensagem (a
/// `@pragma('vm:isolate-unsendable')` da VM).
@pragma("vm:external-name", "DartForge_classe_nao_enviavel")
external void _naoEnviavel(Object objeto);

/// A porta de recepção: o id da porta no runtime (`crates/runtime/src/
/// portas.rs`), o manipulador e a `SendPort`. O runtime entrega cada
/// mensagem chamando `_despachar`, que faz o `_handleMessage` da VM.
@pragma("vm:entry-point")
final class _RawReceivePort implements RawReceivePort {
  int _id = 0;
  Function? _handler;
  late final SendPort sendPort = new _SendPort._(_id);

  factory _RawReceivePort(String debugName) {
    if (!_despachanteRegistrado) {
      _despachanteRegistrado = true;
      _registrarDespachante(_despachar);
    }
    final port = _RawReceivePort._();
    port._id = _abrir(port);
    _portMap[port._id] = port;
    return port;
  }

  _RawReceivePort._();

  static bool _despachanteRegistrado = false;

  @pragma("vm:external-name", "DartForge_porta_abrir")
  external static int _abrir(_RawReceivePort porta);

  @pragma("vm:external-name", "DartForge_porta_fechar")
  external static void _fechar(int id);

  @pragma("vm:external-name", "DartForge_porta_manter_vivo")
  external static void _manterVivo(int id, bool manter);

  @pragma("vm:external-name", "DartForge_porta_mantem_vivo")
  external static bool _mantemVivo(int id);

  @pragma("vm:external-name", "DartForge_portas_despachante")
  external static void _registrarDespachante(void Function() despachar);

  @pragma("vm:external-name", "DartForge_porta_atual")
  external static int _portaAtual();

  @pragma("vm:external-name", "DartForge_mensagem_atual")
  external static Object? _mensagemAtual();

  close() {
    // Fecha a porta e a tira do mapa dos manipuladores.
    _fechar(_id);
    _portMap.remove(_id);
  }

  bool operator ==(var other) {
    return (other is _RawReceivePort) && (this._id == other._id);
  }

  int get hashCode {
    return sendPort.hashCode;
  }

  /// A mensagem corrente do laço de eventos do runtime.
  static void _despachar() {
    _handleMessage(_portaAtual(), _mensagemAtual());
  }

  static _handleMessage(int id, var message) {
    final Function? handler = _portMap[id]?._handler;
    if (handler == null) {
      return null;
    }
    handler(message);
    return handler;
  }

  void set handler(Function? value) {
    _handler = value;
  }

  void set keepIsolateAlive(bool value) {
    _manterVivo(_id, value);
  }

  bool get keepIsolateAlive => _mantemVivo(_id);

  static final _portMap = <int, _RawReceivePort>{};
}

/// A `SendPort` de uma porta aberta pelo runtime (as portas nativas, como a
/// do IOService: o `Dart_NewSendPort` da VM).
@pragma("vm:entry-point")
SendPort _dartforgeSendPort(int id) => new _SendPort._(id);

// As funções que o runtime chama num isolado (`crates/runtime/src/
// isolados.rs`): o que a VM faz em C++ ao criar e encerrar um isolado.

/// `[SendPort, Capability, Capability]` do isolado: a porta de controle e
/// as capacidades de pausa e de término
/// (`Isolate_getPortAndCapabilitiesOfCurrentIsolate`).
@pragma("vm:entry-point")
List _dartforgePortaECapacidades(int controle, int pausa, int termino) =>
    new List<Object>.unmodifiable(<Object>[
      new _SendPort._(controle),
      new _Capability._comId(pausa),
      new _Capability._comId(termino)
    ]);

/// A mensagem de pronto que o isolado novo manda ao que o criou:
/// `[porta de controle, [pausa, término]]` (a de `IsolateSpawnState`).
@pragma("vm:entry-point")
List _dartforgeMensagemDePronto(int controle, int pausa, int termino) => [
      new _SendPort._(controle),
      [new _Capability._comId(pausa), new _Capability._comId(termino)]
    ];

/// O começo do isolado: a entrada roda como a primeira mensagem, como no
/// `_delayEntrypointInvocation` da VM para `Isolate.spawn` (que sempre
/// chama `entryPoint(message)`).
@pragma("vm:entry-point")
void _dartforgeIniciarIsolado(Function entrada, Object? mensagem) {
  final port = RawReceivePort();
  port.handler = (_) {
    port.close();
    entrada(mensagem);
  };
  port.sendPort.send(null);
}

/// O erro não tratado como os ouvintes de erro o recebem:
/// `[erro.toString(), rastro.toString()]`.
@pragma("vm:entry-point")
List<String> _dartforgeDescreverErro(Object? erro, Object? rastro) =>
    <String>[erro.toString(), rastro?.toString() ?? ""];

/// O lado de envio: só o id da porta, que a cópia de uma mensagem preserva
/// — uma `SendPort` vale em qualquer isolado.
@pragma("vm:entry-point")
final class _SendPort implements SendPort {
  final int _id;

  _SendPort._(this._id);

  /*--- public interface ---*/
  @pragma("vm:entry-point", "call")
  void send(var message) {
    if (_enviar(_id, message)) {
      throw new ArgumentError(
          "Illegal argument in isolate message: ${_recusa()}");
    }
  }

  bool operator ==(var other) {
    return (other is _SendPort) && (this._id == other._id);
  }

  int get hashCode => _id;

  String toString() => "SendPort";

  @pragma("vm:external-name", "DartForge_porta_enviar")
  external static bool _enviar(int id, Object? message);

  @pragma("vm:external-name", "DartForge_porta_recusa")
  external static String _recusa();
}

typedef _UnaryFunction(Never args);
typedef _BinaryFunction(Never args, Never message);

/**
 * Takes the real entry point as argument and schedules it to run in the message
 * queue.
 */
@pragma("vm:entry-point", "call")
void _startMainIsolate(Function entryPoint, List<String>? args) {
  _delayEntrypointInvocation(entryPoint, args, null, true);
}

/**
 * Returns the _startMainIsolate function. This closurization allows embedders
 * to setup trampolines to the main function. This workaround can be removed
 * once support for @pragma("vm:entry_point", "get") as documented in
 * https://github.com/dart-lang/sdk/issues/35720 lands.
 */
@pragma("vm:entry-point", "call")
Function _getStartMainIsolateFunction() {
  return _startMainIsolate;
}

/**
 * Takes the real entry point as argument and schedules it to run in the message
 * queue.
 */
@pragma("vm:entry-point", "call")
void _startIsolate(
    Function entryPoint, List<String>? args, Object? message, bool isSpawnUri) {
  _delayEntrypointInvocation(entryPoint, args, message, isSpawnUri);
}

void _delayEntrypointInvocation(Function entryPoint, List<String>? args,
    Object? message, bool allowZeroOneOrTwoArgs) {
  final port = RawReceivePort();
  port.handler = (_) {
    port.close();
    if (allowZeroOneOrTwoArgs) {
      if (entryPoint is _BinaryFunction) {
        (entryPoint as Function)(args, message);
      } else if (entryPoint is _UnaryFunction) {
        (entryPoint as Function)(args);
      } else {
        entryPoint();
      }
    } else {
      entryPoint(message);
    }
  };
  port.sendPort.send(null);
}

@patch
final class Isolate {
  static final _currentIsolate = _getCurrentIsolate();
  static final _rootUri = _getCurrentRootUri();

  @patch
  static Isolate get current => _currentIsolate;

  @patch
  String? get debugName => _getDebugName(controlPort);

  @patch
  static Future<Uri?> get packageConfig {
    return Future.value(packageConfigSync);
  }

  @patch
  static Uri? get packageConfigSync {
    var hook = VMLibraryHooks.packageConfigUriSync;
    if (hook == null) {
      throw new UnsupportedError("Isolate.packageConfig");
    }
    return hook();
  }

  @patch
  static Future<Uri?> resolvePackageUri(Uri packageUri) {
    return Future.value(resolvePackageUriSync(packageUri));
  }

  @patch
  static Uri? resolvePackageUriSync(Uri packageUri) {
    var hook = VMLibraryHooks.resolvePackageUriSync;
    if (hook == null) {
      throw new UnsupportedError("Isolate.resolvePackageUriSync");
    }
    return hook(packageUri);
  }

  static bool _packageSupported() =>
      (VMLibraryHooks.packageConfigUriSync != null) &&
      (VMLibraryHooks.resolvePackageUriSync != null);

  @patch
  static Future<Isolate> spawn<T>(void entryPoint(T message), T message,
      {bool paused = false,
      bool errorsAreFatal = true,
      SendPort? onExit,
      SendPort? onError,
      String? debugName}) async {
    // The VM will invoke [_startIsolate] with [entryPoint] as argument.

    // We do not inherit the package config settings from the parent isolate,
    // instead we use the values that were set on the command line.
    var packageConfig = VMLibraryHooks.packageConfigString;
    // O script do executável, com ou sem `dart:io` no programa (o embedder
    // da VM sempre o define; o isolado novo roda no mesmo executável).
    var script = VMLibraryHooks.platformScript ?? _rootUri;
    if (script == null) {
      // We do not have enough information to support spawning the new
      // isolate.
      throw new UnsupportedError("Isolate.spawn");
    }
    if (script.isScheme("package")) {
      if (Isolate._packageSupported()) {
        // resolving script uri is not really necessary, but can be useful
        // for better failed-to-lookup-function-in-a-script spawn errors.
        script = Isolate.resolvePackageUriSync(script);
      }
    }

    final RawReceivePort readyPort =
        new RawReceivePort(null, 'Isolate.spawn ready');
    try {
      _spawnFunction(readyPort.sendPort, script.toString(), entryPoint, message,
          paused, errorsAreFatal, onExit, onError, packageConfig, debugName);
      return await _spawnCommon(readyPort);
    } catch (e, st) {
      readyPort.close();
      return await new Future<Isolate>.error(e, st);
    }
  }

  @pragma("vm:external-name", "Isolate_spawnFunction")
  external static void _spawnFunction(
      SendPort readyPort,
      String uri,
      Function topLevelFunction,
      var message,
      bool paused,
      bool errorsAreFatal,
      SendPort? onExit,
      SendPort? onError,
      String? packageConfig,
      String? debugName);

  @patch
  static Future<Isolate> spawnUri(Uri uri, List<String> args, var message,
      {bool paused = false,
      SendPort? onExit,
      SendPort? onError,
      bool errorsAreFatal = true,
      bool? checked,
      Map<String, String>? environment,
      Uri? packageRoot,
      Uri? packageConfig,
      bool automaticPackageResolution = false,
      String? debugName}) async {
    if (environment != null) {
      throw new UnimplementedError("environment");
    }

    // Verify that no mutually exclusive arguments have been passed.
    if (automaticPackageResolution) {
      if (packageRoot != null) {
        throw new ArgumentError("Cannot simultaneously request "
            "automaticPackageResolution and specify a "
            "packageRoot.");
      }
      if (packageConfig != null) {
        throw new ArgumentError("Cannot simultaneously request "
            "automaticPackageResolution and specify a "
            "packageConfig.");
      }
    } else {
      if ((packageRoot != null) && (packageConfig != null)) {
        throw new ArgumentError("Cannot simultaneously specify a "
            "packageRoot and a packageConfig.");
      }
    }
    // Resolve the uri against the current isolate's root Uri first.
    final Uri spawnedUri = _rootUri!.resolveUri(uri);

    // Inherit this isolate's package resolution setup if not overridden.
    if (!automaticPackageResolution && packageConfig == null) {
      if (Isolate._packageSupported()) {
        packageConfig = Isolate.packageConfigSync;
      }
    }

    // Ensure to resolve package: URIs being handed in as parameters.
    if (packageConfig != null) {
      // Avoid calling resolvePackageUri if not strictly necessary in case
      // the API is not supported.
      if (packageConfig.isScheme("package")) {
        packageConfig = Isolate.resolvePackageUriSync(packageConfig);
      }
    }

    // The VM will invoke [_startIsolate] and not `main`.
    final packageConfigString = packageConfig?.toString();

    final RawReceivePort readyPort =
        new RawReceivePort(null, 'Isolate.spawnUri ready');
    final enableAsserts = checked;
    try {
      _spawnUri(
          readyPort.sendPort,
          spawnedUri.toString(),
          args,
          message,
          paused,
          onExit,
          onError,
          errorsAreFatal,
          enableAsserts,
          null,
          /* environment */
          packageConfigString,
          debugName);
      return await _spawnCommon(readyPort);
    } catch (e) {
      readyPort.close();
      rethrow;
    }
  }

  static Future<Isolate> _spawnCommon(RawReceivePort readyPort) {
    final completer = new Completer<Isolate>.sync();
    readyPort.handler = (readyMessage) {
      readyPort.close();
      if (readyMessage is List && readyMessage.length == 2) {
        SendPort controlPort = readyMessage[0];
        List capabilities = readyMessage[1];
        completer.complete(new Isolate(controlPort,
            pauseCapability: capabilities[0],
            terminateCapability: capabilities[1]));
      } else if (readyMessage is String) {
        // We encountered an error while starting the new isolate.
        completer.completeError(new IsolateSpawnException(
            'Unable to spawn isolate: ${readyMessage}'));
      } else {
        // This shouldn't happen.
        completer.completeError(new IsolateSpawnException(
            "Internal error: unexpected format for ready message: "
            "'${readyMessage}'"));
      }
    };
    return completer.future;
  }

  // TODO(iposva): Cleanup to have only one definition.
  // These values need to be kept in sync with the class IsolateMessageHandler
  // in vm/isolate.cc.
  static const _PAUSE = 1;
  static const _RESUME = 2;
  static const _PING = 3;
  static const _KILL = 4;
  static const _ADD_EXIT = 5;
  static const _DEL_EXIT = 6;
  static const _ADD_ERROR = 7;
  static const _DEL_ERROR = 8;
  static const _ERROR_FATAL = 9;

  // For 'spawnFunction' see internal_patch.dart.

  @pragma("vm:external-name", "Isolate_spawnUri")
  external static void _spawnUri(
      SendPort readyPort,
      String uri,
      List<String> args,
      var message,
      bool paused,
      SendPort? onExit,
      SendPort? onError,
      bool errorsAreFatal,
      bool? enableAsserts,
      List? environment,
      String? packageConfig,
      String? debugName);

  @pragma("vm:external-name", "Isolate_sendOOB")
  external static void _sendOOB(port, msg);

  @pragma("vm:external-name", "Isolate_getDebugName")
  external static String? _getDebugName(SendPort controlPort);

  @patch
  void _pause(Capability resumeCapability) {
    // _sendOOB expects a fixed length array and hence we create a fixed
    // length array and assign values to it instead of using [ ... ].
    var msg = new List<Object?>.filled(4, null)
      ..[0] = 0 // Make room for OOB message type.
      ..[1] = _PAUSE
      ..[2] = pauseCapability
      ..[3] = resumeCapability;
    _sendOOB(controlPort, msg);
  }

  @patch
  void resume(Capability resumeCapability) {
    var msg = new List<Object?>.filled(4, null)
      ..[0] = 0 // Make room for OOB message type.
      ..[1] = _RESUME
      ..[2] = pauseCapability
      ..[3] = resumeCapability;
    _sendOOB(controlPort, msg);
  }

  @patch
  void addOnExitListener(SendPort responsePort, {Object? response}) {
    var msg = new List<Object?>.filled(4, null)
      ..[0] = 0 // Make room for OOB message type.
      ..[1] = _ADD_EXIT
      ..[2] = responsePort
      ..[3] = response;
    _sendOOB(controlPort, msg);
  }

  @patch
  void removeOnExitListener(SendPort responsePort) {
    var msg = new List<Object?>.filled(3, null)
      ..[0] = 0 // Make room for OOB message type.
      ..[1] = _DEL_EXIT
      ..[2] = responsePort;
    _sendOOB(controlPort, msg);
  }

  @patch
  void setErrorsFatal(bool errorsAreFatal) {
    var msg = new List<Object?>.filled(4, null)
      ..[0] = 0 // Make room for OOB message type.
      ..[1] = _ERROR_FATAL
      ..[2] = terminateCapability
      ..[3] = errorsAreFatal;
    _sendOOB(controlPort, msg);
  }

  @patch
  void kill({int priority = beforeNextEvent}) {
    var msg = new List<Object?>.filled(4, null)
      ..[0] = 0 // Make room for OOB message type.
      ..[1] = _KILL
      ..[2] = terminateCapability
      ..[3] = priority;
    _sendOOB(controlPort, msg);
  }

  @patch
  void ping(SendPort responsePort,
      {Object? response, int priority = immediate}) {
    var msg = new List<Object?>.filled(5, null)
      ..[0] = 0 // Make room for OOM message type.
      ..[1] = _PING
      ..[2] = responsePort
      ..[3] = priority
      ..[4] = response;
    _sendOOB(controlPort, msg);
  }

  @patch
  void addErrorListener(SendPort port) {
    var msg = new List<Object?>.filled(3, null)
      ..[0] = 0 // Make room for OOB message type.
      ..[1] = _ADD_ERROR
      ..[2] = port;
    _sendOOB(controlPort, msg);
  }

  @patch
  void removeErrorListener(SendPort port) {
    var msg = new List<Object?>.filled(3, null)
      ..[0] = 0 // Make room for OOB message type.
      ..[1] = _DEL_ERROR
      ..[2] = port;
    _sendOOB(controlPort, msg);
  }

  static Isolate _getCurrentIsolate() {
    List portAndCapabilities = _getPortAndCapabilitiesOfCurrentIsolate();
    return new Isolate(portAndCapabilities[0],
        pauseCapability: portAndCapabilities[1],
        terminateCapability: portAndCapabilities[2]);
  }

  @pragma("vm:external-name", "Isolate_getPortAndCapabilitiesOfCurrentIsolate")
  external static List _getPortAndCapabilitiesOfCurrentIsolate();

  static Uri? _getCurrentRootUri() {
    try {
      return Uri.parse(_getCurrentRootUriStr());
    } catch (e) {
      return null;
    }
  }

  @pragma("vm:external-name", "Isolate_getCurrentRootUriStr")
  external static String _getCurrentRootUriStr();

  @pragma("vm:external-name", "Isolate_exit_")
  external static Never _exit(SendPort? finalMessagePort, Object? message);

  @pragma("vm:entry-point")
  static bool _mayExit = true;

  @patch
  static Never exit([SendPort? finalMessagePort, Object? message]) {
    if (!_mayExit) {
      throw UnsupportedError("Isolate.exit");
    }
    _exit(finalMessagePort, message);
  }

  /**
   * Creates an Uri representing the script which was compiled into kernel
   * binary in [kernelBlob].
   * The resulting Uri can be used for the subsequent spawnUri calls.
   * Such spawnUri will start an isolate which would run the given
   * compiled script in [kernelBlob].
   */
  /*static*/ Uri createUriForKernelBlob(Uint8List kernelBlob) {
    return Uri.parse(_registerKernelBlob(kernelBlob));
  }

  /**
   * Unregisters kernel blob previously registered with
   * [createUriForKernelBlob] and frees underlying resources.
   */
  /*static*/ void unregisterKernelBlobUri(Uri kernelBlobUri) {
    _unregisterKernelBlob(kernelBlobUri.toString());
  }

  @pragma("vm:external-name", "Isolate_registerKernelBlob")
  external static String _registerKernelBlob(Uint8List kernelBlob);

  @pragma("vm:external-name", "Isolate_unregisterKernelBlob")
  external static void _unregisterKernelBlob(String kernelBlobUri);
}

@patch
@pragma("vm:entry-point")
abstract final class TransferableTypedData {
  @patch
  factory TransferableTypedData.fromList(List<TypedData> chunks) {
    if (chunks == null) {
      throw ArgumentError(chunks);
    }
    final int cid = ClassID.getID(chunks);
    if (cid != ClassID.cidArray &&
        cid != ClassID.cidGrowableObjectArray &&
        cid != ClassID.cidImmutableArray) {
      chunks = List.unmodifiable(chunks);
    }
    return _TransferableTypedDataImpl(chunks);
  }
}

/// Os bytes de um `TransferableTypedData`: concatenados uma vez na
/// criação; `materialize` os entrega uma vez só, e enviar numa mensagem os
/// move (o objeto de origem fica vazio), como o peer externo da VM.
@pragma("vm:entry-point")
final class _TransferableTypedDataImpl implements TransferableTypedData {
  Uint8List? _dados;

  _TransferableTypedDataImpl(List<TypedData> list) {
    var total = 0;
    for (final c in list) {
      total += c.lengthInBytes;
    }
    final dados = new Uint8List(total);
    var i = 0;
    for (final c in list) {
      dados.setRange(
          i, i + c.lengthInBytes, c.buffer.asUint8List(c.offsetInBytes, c.lengthInBytes));
      i += c.lengthInBytes;
    }
    _dados = dados;
    _transferivel(this);
  }

  ByteBuffer materialize() {
    final dados = _dados;
    if (dados == null) {
      throw new ArgumentError(
          "Attempt to materialize object that was transferred already.");
    }
    _dados = null;
    return dados.buffer;
  }

  @pragma("vm:external-name", "DartForge_classe_transferivel")
  external static void _transferivel(Object objeto);
}
