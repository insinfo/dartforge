// Replaces `_internal/vm/bin/secure_socket_patch.dart` (overlay
// `native_sdk/`): the same classes, with the native state in the DartForge
// runtime (`crates/runtime/src/tls.rs`, over rustls) instead of the VM's
// BoringSSL. The SDK's `secure_socket.dart` stays intact:
//
// * the filter's four rings are Dart `Uint8List`s (on the VM, external C
//   memory) and the IOService's `sslProcessFilter` request is served on the
//   isolate's thread (`io_service_patch.dart`, `_processRequest`), in a later
//   event of the loop, like the answer from the IOService port: the filter
//   only does in-memory cryptography;
// * the end of the handshake and the certificate to evaluate come back as
//   codes of `_handshake` (the VM calls the `handshakeCompleteHandler` and
//   posts to the evaluator's port from inside the native);
// * the VM's natives with named parameters become positional and return the
//   error text, which Dart throws as the VM's `TlsException`.

// Copyright (c) 2012, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

part of "common_patch.dart";

@patch
class SecureSocket {
  @patch
  factory SecureSocket._(RawSecureSocket rawSocket) =>
      new _SecureSocket(rawSocket);
}

@patch
class _SecureFilter {
  @patch
  factory _SecureFilter._() => new _SecureFilterImpl._();
}

@patch
@pragma("vm:entry-point")
class X509Certificate {
  @patch
  @pragma("vm:entry-point")
  factory X509Certificate._() => new _X509CertificateImpl._();
}

class _SecureSocket extends _Socket implements SecureSocket {
  _RawSecureSocket? get _raw => super._raw as _RawSecureSocket?;

  _SecureSocket(RawSecureSocket raw) : super(raw);

  void renegotiate(
      {bool useSessionCache = true,
      bool requestClientCertificate = false,
      bool requireClientCertificate = false}) {}

  X509Certificate? get peerCertificate {
    if (_raw == null) {
      throw new StateError("peerCertificate called on destroyed SecureSocket");
    }
    return _raw!.peerCertificate;
  }

  String? get selectedProtocol {
    if (_raw == null) {
      throw new StateError("selectedProtocol called on destroyed SecureSocket");
    }
    return _raw!.selectedProtocol;
  }
}

/// The filter of a `RawSecureSocket`: the native TLS connection and the four
/// rings that `secure_socket.dart` fills and drains.
@pragma("vm:entry-point")
base class _SecureFilterImpl extends NativeFieldWrapperClass1
    implements _SecureFilter {
  // The VM's sizes: a ring full of plaintext fits in the encrypted ring.
  @pragma("vm:entry-point")
  static final int SIZE = 8 * 1024;
  @pragma("vm:entry-point")
  static final int ENCRYPTED_SIZE = 10 * 1024;

  /// The live filters by the identifier that `_pointer()` hands to the
  /// `sslProcessFilter` request.
  static final Map<int, _SecureFilterImpl> _filters = <int, _SecureFilterImpl>{};
  static int _nextId = 1;

  int _id = 0;
  bool _server = false;
  Function? _onHandshakeComplete;

  _SecureFilterImpl._() {
    buffers = <_ExternalBuffer>[
      for (int i = 0; i < _RawSecureSocket.bufferCount; ++i)
        new _ExternalBuffer(
            _RawSecureSocket._isBufferEncrypted(i) ? ENCRYPTED_SIZE : SIZE)
          ..data = new Uint8List(
              _RawSecureSocket._isBufferEncrypted(i) ? ENCRYPTED_SIZE : SIZE),
    ];
  }

  void init() {
    _create();
    _id = _nextId++;
    _filters[_id] = this;
  }

  void connect(
      String hostName,
      SecurityContext context,
      bool isServer,
      bool requestClientCertificate,
      bool requireClientCertificate,
      Uint8List protocols) {
    _server = isServer;
    final error = _connect(hostName, context, isServer,
        requestClientCertificate, requireClientCertificate, protocols);
    if (error != null) {
      throw new TlsException("Failure in connect", new OSError(error));
    }
  }

  void destroy() {
    buffers = null;
    _filters.remove(_id);
    _destroy();
  }

  Future<bool> handshake() {
    final state = _handshake();
    if (state < 0) {
      throw new HandshakeException(
          "Handshake error in ${_server ? 'server' : 'client'}",
          new OSError(_errorNative() ?? ""));
    }
    if (state == 1) {
      _onHandshakeComplete!();
      return new Future<bool>.value(false);
    }
    const int kSslErrorWantCertificateVerify = 16; // ssl.h:558
    if (state != kSslErrorWantCertificateVerify) {
      return new Future<bool>.value(false);
    }
    // The VM's evaluator answers through a port; the decision arrives in a later
    // event, and `onBadCertificate` runs outside the handshake.
    final evaluation = new Completer<bool>();
    Timer.run(() {
      final certificate =
          new _X509CertificateImpl._adopt(_pendingCertificate());
      bool trusted = false;
      final callback = badCertificateCallback;
      if (callback != null) {
        try {
          trusted = callback(certificate);
        } catch (e, st) {
          evaluation.completeError(e, st);
          return;
        }
      }
      _decide(trusted);
      evaluation.complete(true);
    });
    return evaluation.future;
  }

  void rehandshake() => throw new UnimplementedError();

  int processBuffer(int bufferIndex) => throw new UnimplementedError();

  /// The `sslProcessFilter` request (`_pushAllFilterStages`):
  /// `[filter, inHandshake, start0, end0, …, start3, end3]` → the new positions,
  /// or `[code, message]`.
  ///
  /// Like the answer of the VM's IOService, which arrives through a port, the
  /// answer comes in a later event of the loop (a `Timer`'s `Future`), never in
  /// a microtask: the `_tryFilter` loop yields at each step, and the I/O events
  /// and timers of the other sockets (the end of the server handshake in the
  /// same isolate, for example) move on.
  static Future<Object?> _processRequest(List dataArg) =>
      new Future<Object?>(() => _processNow(dataArg));

  static Object? _processNow(List dataArg) {
    final filter = _filters[dataArg[0] as int];
    final buffers = filter?.buffers;
    if (filter == null || buffers == null) {
      // Destroyed with the request in flight: nothing changes.
      return <Object?>[for (int i = 2; i < dataArg.length; i++) dataArg[i]];
    }
    final positions = new Int64List(8);
    for (int i = 0; i < 8; i++) {
      positions[i] = dataArg[i + 2] as int;
    }
    final code = filter._process(
        dataArg[1] as bool,
        buffers[0].data as Uint8List,
        buffers[1].data as Uint8List,
        buffers[2].data as Uint8List,
        buffers[3].data as Uint8List,
        positions);
    filter._dumpKeys();
    if (code != 0) {
      return <Object?>[code, filter._errorNative()];
    }
    return <Object?>[...positions];
  }

  String? selectedProtocol() => _protocolNative();

  X509Certificate? get peerCertificate {
    final p = _peerCertificate();
    return p == 0 ? null : new _X509CertificateImpl._adopt(p);
  }

  bool Function(X509Certificate)? badCertificateCallback;

  void registerBadCertificateCallback(bool Function(X509Certificate) callback) {
    badCertificateCallback = callback;
  }

  void registerHandshakeCompleteCallback(Function handshakeCompleteHandler) {
    _onHandshakeComplete = handshakeCompleteHandler;
  }

  SendPort? _keyPort;

  void registerKeyLogPort(SendPort port) {
    _keyPort = port;
    _registerKeys();
  }

  /// Sends the `keyLog` lines registered by rustls to the port.
  void _dumpKeys() {
    final portArg = _keyPort;
    if (portArg == null) return;
    final lines = _pendingKeys();
    if (lines == null) return;
    for (final line in lines.split("\n")) {
      portArg.send(line);
    }
  }

  int _pointer() => _id;

  @pragma("vm:entry-point", "get")
  List<_ExternalBuffer>? buffers;

  @pragma("vm:external-name", "DartForge_tls_filter_new")
  external void _create();
  @pragma("vm:external-name", "DartForge_tls_filter_connect")
  external String? _connect(String host, SecurityContext ctx,
      bool asServer, bool request, bool require, Uint8List protocolList);
  @pragma("vm:external-name", "DartForge_tls_filter_destroy")
  external void _destroy();
  @pragma("vm:external-name", "DartForge_tls_filter_handshake")
  external int _handshake();
  @pragma("vm:external-name", "DartForge_tls_filter_error")
  external String? _errorNative();
  @pragma("vm:external-name", "DartForge_tls_filter_pending_certificate")
  external int _pendingCertificate();
  @pragma("vm:external-name", "DartForge_tls_filter_decide")
  external void _decide(bool trusted);
  @pragma("vm:external-name", "DartForge_tls_filter_protocol")
  external String? _protocolNative();
  @pragma("vm:external-name", "DartForge_tls_filter_peer_certificate")
  external int _peerCertificate();
  @pragma("vm:external-name", "DartForge_tls_filter_register_keys")
  external void _registerKeys();
  @pragma("vm:external-name", "DartForge_tls_filter_pending_keys")
  external String? _pendingKeys();
  @pragma("vm:external-name", "DartForge_tls_filter_process")
  external int _process(bool inHandshake, Uint8List textRead,
      Uint8List textToWrite, Uint8List encryptedRead,
      Uint8List encryptedToWrite, Int64List positions);
}

@patch
class SecurityContext {
  @patch
  factory SecurityContext({bool withTrustedRoots = false}) {
    return new _SecurityContext(withTrustedRoots);
  }

  @patch
  static SecurityContext get defaultContext {
    return _SecurityContext.defaultContext;
  }

  @patch
  static bool get alpnSupported => true;
}

base class _SecurityContext extends NativeFieldWrapperClass1
    implements SecurityContext {
  bool _allowLegacyUnsafeRenegotiation = false;

  _SecurityContext(bool withTrustedRoots) {
    _createNativeContext();
    if (withTrustedRoots) {
      _trustBuiltinRoots();
    }
  }

  set allowLegacyUnsafeRenegotiation(bool allow) {
    // rustls does not renegotiate; the value is only kept. A renegotiation request
    // from the server is refused with or without the option (without it, like the
    // VM); `tls_renegociacao.dart` in crates/cli/tests.
    _allowLegacyUnsafeRenegotiation = allow;
  }

  bool get allowLegacyUnsafeRenegotiation => _allowLegacyUnsafeRenegotiation;

  set minimumTlsProtocolVersion(TlsProtocolVersion version) {
    _setMinimumProtocolVersion(version._version);
  }

  TlsProtocolVersion get minimumTlsProtocolVersion =>
      TlsProtocolVersion._fromProtocolVersionConstant(
          _getMinimumProtocolVersion());

  static final SecurityContext defaultContext = new _SecurityContext(true);

  static Uint8List _bytes(List<int> b) =>
      b is Uint8List ? b : new Uint8List.fromList(b);

  static void _verify(String operation, String? error) {
    if (error != null) {
      throw new TlsException("Failure in $operation", new OSError(error));
    }
  }

  void usePrivateKey(String file, {String? password}) {
    List<int> bytes = (new File(file)).readAsBytesSync();
    usePrivateKeyBytes(bytes, password: password);
  }

  void usePrivateKeyBytes(List<int> keyBytes, {String? password}) {
    // Like the VM: no key in the bytes (PEM, encrypted PEM, PKCS#12, wrong
    // password), `ArgumentError`, not `TlsException`.
    if (!_useKey(_bytes(keyBytes), password ?? "")) {
      throw new ArgumentError("Expected private key, but none was found");
    }
  }

  void setTrustedCertificates(String file, {String? password}) {
    List<int> bytes = (new File(file)).readAsBytesSync();
    setTrustedCertificatesBytes(bytes, password: password);
  }

  void setTrustedCertificatesBytes(List<int> certBytes, {String? password}) {
    // The message is the VM's (`security_context.cc` uses the text of the
    // built-in roots here too).
    final error = _trust(_bytes(certBytes), password ?? "");
    if (error != null) {
      throw new TlsException("Failure trusting builtin roots", new OSError(error));
    }
  }

  void useCertificateChain(String file, {String? password}) {
    List<int> bytes = (new File(file)).readAsBytesSync();
    useCertificateChainBytes(bytes, password: password);
  }

  void useCertificateChainBytes(List<int> chainBytes, {String? password}) {
    _verify("useCertificateChainBytes",
        _useChain(_bytes(chainBytes), password ?? ""));
  }

  void setClientAuthorities(String file, {String? password}) {
    List<int> bytes = (new File(file)).readAsBytesSync();
    setClientAuthoritiesBytes(bytes, password: password);
  }

  void setClientAuthoritiesBytes(List<int> authCertBytes, {String? password}) {
    _verify("setClientAuthoritiesBytes",
        _useAuthorities(_bytes(authCertBytes), password ?? ""));
  }

  void setAlpnProtocols(List<String> protocols, bool isServer) {
    Uint8List encodedProtocols =
        SecurityContext._protocolsToLengthEncoding(protocols);
    _setAlpnProtocols(encodedProtocols, isServer);
  }

  @pragma("vm:external-name", "DartForge_tls_context_new")
  external void _createNativeContext();
  @pragma("vm:external-name", "DartForge_tls_context_key")
  external bool _useKey(Uint8List bytes, String pass);
  @pragma("vm:external-name", "DartForge_tls_context_trusted")
  external String? _trust(Uint8List bytes, String pass);
  @pragma("vm:external-name", "DartForge_tls_context_chain")
  external String? _useChain(Uint8List bytes, String pass);
  @pragma("vm:external-name", "DartForge_tls_context_authorities")
  external String? _useAuthorities(Uint8List bytes, String pass);
  @pragma("vm:external-name", "DartForge_tls_context_alpn")
  external void _setAlpnProtocols(Uint8List protocols, bool isServer);
  @pragma("vm:external-name", "DartForge_tls_context_builtin_roots")
  external void _trustBuiltinRoots();
  @pragma("vm:external-name", "DartForge_tls_context_set_min_version")
  external void _setMinimumProtocolVersion(int version);
  @pragma("vm:external-name", "DartForge_tls_context_min_version")
  external int _getMinimumProtocolVersion();
}

/// An X.509 certificate (the DER in the native field).
base class _X509CertificateImpl extends NativeFieldWrapperClass1
    implements X509Certificate {
  _X509CertificateImpl._();

  /// The certificate of the pointer a native handed over (with a reference to
  /// this object).
  factory _X509CertificateImpl._adopt(int pointer) {
    final c = new _X509CertificateImpl._();
    c._pin(pointer);
    return c;
  }

  late final Uint8List der = _der();
  late final String pem = _pem();
  late final Uint8List sha1 = _sha1();

  String get subject => _subject();
  String get issuer => _issuer();

  DateTime get startValidity =>
      new DateTime.fromMillisecondsSinceEpoch(_start(), isUtc: true);

  DateTime get endValidity =>
      new DateTime.fromMillisecondsSinceEpoch(_end(), isUtc: true);

  @pragma("vm:external-name", "DartForge_tls_x509_adopt")
  external void _pin(int pointer);
  @pragma("vm:external-name", "DartForge_tls_x509_der")
  external Uint8List _der();
  @pragma("vm:external-name", "DartForge_tls_x509_pem")
  external String _pem();
  @pragma("vm:external-name", "DartForge_tls_x509_sha1")
  external Uint8List _sha1();
  @pragma("vm:external-name", "DartForge_tls_x509_subject")
  external String _subject();
  @pragma("vm:external-name", "DartForge_tls_x509_issuer")
  external String _issuer();
  @pragma("vm:external-name", "DartForge_tls_x509_start")
  external int _start();
  @pragma("vm:external-name", "DartForge_tls_x509_end")
  external int _end();
}
