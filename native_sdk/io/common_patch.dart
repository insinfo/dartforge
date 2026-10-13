// Replaces `_internal/vm/bin/common_patch.dart` (overlay `native_sdk/`): the
// VM's file, unchanged, plus the `_dartforge*` functions the DartForge runtime
// calls to build `dart:io` objects (the VM builds them through the C API,
// `DartUtils::NewDartOSError`, `SyncDirectoryListing`…). The library's
// registration hands the addresses to the runtime
// (`dartforge_register_helper`); the natives are in
// `crates/runtime/src/io_*.rs`.

// Copyright (c) 2012, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

/// Note: the VM concatenates all patch files into a single patch file. This
/// file is the first patch in "dart:io" which contains all the imports used by
/// patches of that library. We plan to change this when we have a shared front
/// end and simply use parts.

import "dart:_internal" show VMLibraryHooks, patch, checkNotNullable, ClassID;

import "dart:async"
    show
        Completer,
        Future,
        Stream,
        StreamConsumer,
        StreamController,
        StreamSubscription,
        Timer,
        Zone,
        scheduleMicrotask;

import "dart:collection" show HashMap, Queue;

import "dart:convert" show Encoding, utf8;

import "dart:developer" show registerExtension;

import "dart:isolate" show RawReceivePort, ReceivePort, SendPort;

import "dart:math" show min;

import "dart:nativewrappers" show NativeFieldWrapperClass1;

import "dart:typed_data" show ByteData, BytesBuilder, Endian, Int64List, Uint8List;

/// These are the additional parts of this patch library:
part "directory_patch.dart";
part "eventhandler_patch.dart";
part "file_patch.dart";
part "file_system_entity_patch.dart";
part "filter_patch.dart";
part "io_service_patch.dart";
part "namespace_patch.dart";
part "platform_patch.dart";
part "process_patch.dart";
part "socket_patch.dart";
part "stdio_patch.dart";
part "secure_socket_patch.dart";
part "sync_socket_patch.dart";

@patch
class _IOCrypto {
  @patch
  @pragma("vm:external-name", "Crypto_GetRandomBytes")
  external static Uint8List getRandomBytes(int count);
}

@pragma("vm:entry-point", "call")
_setupHooks() {
  VMLibraryHooks.eventHandlerSendData = _EventHandler._sendData;
  VMLibraryHooks.timerMillisecondClock = _EventHandler._timerMillisecondClock;
}

// ---------------------------------------------------------------------------
// DartForge: constructors the runtime calls.

/// The `OSError` of a native (`DartUtils::NewDartOSError`).
@pragma("vm:entry-point")
OSError _dartforgeOSError(String message, int errorCode) =>
    new OSError(message, errorCode);

/// An entry of the synchronous listing (`SyncDirectoryListing::HandleFile`,
/// `HandleDirectory`, `HandleLink`): the kind is the VM's `ListType`
/// (0 file, 1 directory, 2 link) and the path comes as bytes, undecoded, as in
/// the VM's `fromRawPath`.
@pragma("vm:entry-point")
void _dartforgeListingEntry(
    List<FileSystemEntity> list, int kind, Uint8List path) {
  switch (kind) {
    case 0:
      list.add(new File.fromRawPath(path));
    case 1:
      list.add(new Directory.fromRawPath(path));
    default:
      list.add(new Link.fromRawPath(path));
  }
}

/// The error of the synchronous listing (`SyncDirectoryListing::HandleError`).
@pragma("vm:entry-point")
Object _dartforgeListingError(OSError error, String path) =>
    new FileSystemException._fromOSError(
        error, "Directory listing failed", path);

/// The `dart:io` setup the VM's embedder does before `main`
/// (`DartUtils::SetupIOLibrary`): the program's script. Returns the
/// `Uri.base` hook (`_getUriBaseClosure`), which the runtime installs in `dart:core`.
@pragma("vm:entry-point")
Object _dartforgeStartIo(String script) {
  _Platform._nativeScript = script;
  return _getUriBaseClosure();
}

/// The start error of a process in the `_ProcessStartStatus` (the
/// `Dart_SetField` of `Process_Start`).
@pragma("vm:entry-point")
void _dartforgeStartFailure(
    _ProcessStartStatus status, int code, String controlMessage) {
  status._errorCode = code;
  status._errorMessage = controlMessage;
}

/// The `Datagram` that `Socket_RecvFrom` returns (the VM calls `_makeDatagram`).
@pragma("vm:entry-point")
Datagram _dartforgeDatagram(
        Uint8List payload, String address, Uint8List raw, int port, int kind) =>
    _makeDatagram(payload, address, raw, port, kind);

// ---------------------------------------------------------------------------
// DartForge: the control messages and the resources passed through a socket
// (`SocketControlMessage`, `ResourceHandle`, N04). The runtime does the
// `sendmsg`/`recvmsg`; these build the Dart objects (the VM builds them with
// the embedding API). `cmsgLevel`/`kind` are the system's `SOL_SOCKET`/`SCM_RIGHTS`.

/// `SocketControlMessage.fromHandles`: the descriptors as system `int32`s, in
/// order.
@pragma("vm:entry-point")
Object _dartforgeResourcesMessage(List<ResourceHandle> resources, int cmsgLevel, int kind) {
  final payload = Uint8List(resources.length * 4);
  final bd = ByteData.sublistView(payload);
  for (var i = 0; i < resources.length; i++) {
    bd.setInt32(i * 4, (resources[i] as _ResourceHandleImpl)._handle, Endian.host);
  }
  return _SocketControlMessageImpl(cmsgLevel, kind, payload);
}

/// `_SocketControlMessageImpl.extractHandles`: the descriptors of an
/// `SCM_RIGHTS` message; another message has no resources.
@pragma("vm:entry-point")
List<ResourceHandle> _dartforgeMessageResources(Object m, int cmsgLevel, int kind) {
  final msg = m as _SocketControlMessageImpl;
  if (msg.level != cmsgLevel || msg.type != kind) return <ResourceHandle>[];
  final bd = ByteData.sublistView(msg.data);
  return <ResourceHandle>[
    for (var i = 0; i + 4 <= msg.data.length; i += 4) _ResourceHandleImpl(bd.getInt32(i, Endian.host)),
  ];
}

/// The descriptor of a `ResourceHandle`.
@pragma("vm:entry-point")
int _dartforgeResourceDescriptor(Object resource) => (resource as _ResourceHandleImpl)._handle;

/// `ResourceHandleImpl_toFile`: the file of the pointer the runtime opened over
/// the descriptor.
@pragma("vm:entry-point")
RandomAccessFile _dartforgeResourceFile(int pointer) => _RandomAccessFile(pointer, "");
