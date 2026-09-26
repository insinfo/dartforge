// Substitui `_internal/vm/bin/secure_socket_patch.dart` (sobreposição
// `sdk_nativo/`): as mesmas classes, com o estado nativo no runtime do
// DartForge (`crates/runtime/src/tls.rs`, sobre o rustls) em vez do
// BoringSSL da VM. O `secure_socket.dart` do SDK fica intacto:
//
// * os quatro anéis do filtro são `Uint8List` do Dart (na VM, memória C
//   externa) e o pedido `sslProcessFilter` do IOService é atendido na
//   thread do isolado (`io_service_patch.dart`, `_processarPedido`), num
//   evento seguinte do laço, como a resposta da porta do IOService: o filtro
//   só faz criptografia em memória;
// * o fim do handshake e o certificado a avaliar voltam como códigos do
//   `_handshake` (a VM chama o `handshakeCompleteHandler` e posta na porta
//   do avaliador de dentro do native);
// * os natives com parâmetros nomeados da VM viram posicionais e devolvem o
//   texto do erro, que o Dart lança como a `TlsException` da VM.

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

/// O filtro de um `RawSecureSocket`: a conexão TLS nativa e os quatro anéis
/// que o `secure_socket.dart` enche e esvazia.
@pragma("vm:entry-point")
base class _SecureFilterImpl extends NativeFieldWrapperClass1
    implements _SecureFilter {
  // Os tamanhos da VM: um anel cheio de texto cabe no anel cifrado.
  @pragma("vm:entry-point")
  static final int SIZE = 8 * 1024;
  @pragma("vm:entry-point")
  static final int ENCRYPTED_SIZE = 10 * 1024;

  /// Os filtros vivos pelo identificador que `_pointer()` entrega ao
  /// pedido `sslProcessFilter`.
  static final Map<int, _SecureFilterImpl> _filtros = <int, _SecureFilterImpl>{};
  static int _proximoId = 1;

  int _id = 0;
  bool _servidor = false;
  Function? _aoCompletarHandshake;

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
    _novo();
    _id = _proximoId++;
    _filtros[_id] = this;
  }

  void connect(
      String hostName,
      SecurityContext context,
      bool isServer,
      bool requestClientCertificate,
      bool requireClientCertificate,
      Uint8List protocols) {
    _servidor = isServer;
    final erro = _conectar(hostName, context, isServer,
        requestClientCertificate, requireClientCertificate, protocols);
    if (erro != null) {
      throw new TlsException("Failure in connect", new OSError(erro));
    }
  }

  void destroy() {
    buffers = null;
    _filtros.remove(_id);
    _destruir();
  }

  Future<bool> handshake() {
    final estado = _handshake();
    if (estado < 0) {
      throw new HandshakeException(
          "Handshake error in ${_servidor ? 'server' : 'client'}",
          new OSError(_erro() ?? ""));
    }
    if (estado == 1) {
      _aoCompletarHandshake!();
      return new Future<bool>.value(false);
    }
    const int kSslErrorWantCertificateVerify = 16; // ssl.h:558
    if (estado != kSslErrorWantCertificateVerify) {
      return new Future<bool>.value(false);
    }
    // O avaliador da VM responde por uma porta; a decisão chega num evento
    // seguinte, e o `onBadCertificate` roda fora do handshake.
    final avaliacao = new Completer<bool>();
    Timer.run(() {
      final certificado =
          new _X509CertificateImpl._adotar(_certificadoPendente());
      bool confiavel = false;
      final callback = badCertificateCallback;
      if (callback != null) {
        try {
          confiavel = callback(certificado);
        } catch (e, st) {
          avaliacao.completeError(e, st);
          return;
        }
      }
      _decidir(confiavel);
      avaliacao.complete(true);
    });
    return avaliacao.future;
  }

  void rehandshake() => throw new UnimplementedError();

  int processBuffer(int bufferIndex) => throw new UnimplementedError();

  /// O pedido `sslProcessFilter` (`_pushAllFilterStages`):
  /// `[filtro, emHandshake, início0, fim0, …, início3, fim3]` → as posições
  /// novas, ou `[código, mensagem]`.
  ///
  /// Como a resposta do IOService da VM, que chega por uma porta, a resposta
  /// vem num evento seguinte do laço (`Future` de um `Timer`), nunca numa
  /// microtarefa: o laço do `_tryFilter` cede a vez a cada passo, e os
  /// eventos de E/S e os timers dos outros soquetes (o fim do handshake do
  /// servidor no mesmo isolado, por exemplo) andam.
  static Future<Object?> _processarPedido(List dados) =>
      new Future<Object?>(() => _processarAgora(dados));

  static Object? _processarAgora(List dados) {
    final filtro = _filtros[dados[0] as int];
    final buffers = filtro?.buffers;
    if (filtro == null || buffers == null) {
      // Destruído com o pedido em voo: nada muda.
      return <Object?>[for (int i = 2; i < dados.length; i++) dados[i]];
    }
    final posicoes = new Int64List(8);
    for (int i = 0; i < 8; i++) {
      posicoes[i] = dados[i + 2] as int;
    }
    final codigo = filtro._processar(
        dados[1] as bool,
        buffers[0].data as Uint8List,
        buffers[1].data as Uint8List,
        buffers[2].data as Uint8List,
        buffers[3].data as Uint8List,
        posicoes);
    filtro._despejarChaves();
    if (codigo != 0) {
      return <Object?>[codigo, filtro._erro()];
    }
    return <Object?>[...posicoes];
  }

  String? selectedProtocol() => _protocolo();

  X509Certificate? get peerCertificate {
    final p = _certificadoDoPar();
    return p == 0 ? null : new _X509CertificateImpl._adotar(p);
  }

  bool Function(X509Certificate)? badCertificateCallback;

  void registerBadCertificateCallback(bool Function(X509Certificate) callback) {
    badCertificateCallback = callback;
  }

  void registerHandshakeCompleteCallback(Function handshakeCompleteHandler) {
    _aoCompletarHandshake = handshakeCompleteHandler;
  }

  SendPort? _portaDeChaves;

  void registerKeyLogPort(SendPort port) {
    _portaDeChaves = port;
    _registrarChaves();
  }

  /// Leva as linhas do `keyLog` registradas pelo rustls à porta.
  void _despejarChaves() {
    final porta = _portaDeChaves;
    if (porta == null) return;
    final linhas = _chavesPendentes();
    if (linhas == null) return;
    for (final linha in linhas.split("\n")) {
      porta.send(linha);
    }
  }

  int _pointer() => _id;

  @pragma("vm:entry-point", "get")
  List<_ExternalBuffer>? buffers;

  @pragma("vm:external-name", "DartForge_tls_filtro_novo")
  external void _novo();
  @pragma("vm:external-name", "DartForge_tls_filtro_conectar")
  external String? _conectar(String host, SecurityContext contexto,
      bool servidor, bool pedir, bool exigir, Uint8List protocolos);
  @pragma("vm:external-name", "DartForge_tls_filtro_destruir")
  external void _destruir();
  @pragma("vm:external-name", "DartForge_tls_filtro_handshake")
  external int _handshake();
  @pragma("vm:external-name", "DartForge_tls_filtro_erro")
  external String? _erro();
  @pragma("vm:external-name", "DartForge_tls_filtro_certificado_pendente")
  external int _certificadoPendente();
  @pragma("vm:external-name", "DartForge_tls_filtro_decidir")
  external void _decidir(bool confiavel);
  @pragma("vm:external-name", "DartForge_tls_filtro_protocolo")
  external String? _protocolo();
  @pragma("vm:external-name", "DartForge_tls_filtro_certificado_do_par")
  external int _certificadoDoPar();
  @pragma("vm:external-name", "DartForge_tls_filtro_registrar_chaves")
  external void _registrarChaves();
  @pragma("vm:external-name", "DartForge_tls_filtro_chaves_pendentes")
  external String? _chavesPendentes();
  @pragma("vm:external-name", "DartForge_tls_filtro_processar")
  external int _processar(bool emHandshake, Uint8List textoLido,
      Uint8List textoAEscrever, Uint8List cifradoLido,
      Uint8List cifradoAEscrever, Int64List posicoes);
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
    // O rustls não renegocia; o valor só é guardado.
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

  static void _verificar(String operacao, String? erro) {
    if (erro != null) {
      throw new TlsException("Failure in $operacao", new OSError(erro));
    }
  }

  void usePrivateKey(String file, {String? password}) {
    List<int> bytes = (new File(file)).readAsBytesSync();
    usePrivateKeyBytes(bytes, password: password);
  }

  void usePrivateKeyBytes(List<int> keyBytes, {String? password}) {
    _verificar(
        "usePrivateKeyBytes", _usarChave(_bytes(keyBytes), password ?? ""));
  }

  void setTrustedCertificates(String file, {String? password}) {
    List<int> bytes = (new File(file)).readAsBytesSync();
    setTrustedCertificatesBytes(bytes, password: password);
  }

  void setTrustedCertificatesBytes(List<int> certBytes, {String? password}) {
    _verificar("setTrustedCertificatesBytes",
        _confiar(_bytes(certBytes), password ?? ""));
  }

  void useCertificateChain(String file, {String? password}) {
    List<int> bytes = (new File(file)).readAsBytesSync();
    useCertificateChainBytes(bytes, password: password);
  }

  void useCertificateChainBytes(List<int> chainBytes, {String? password}) {
    _verificar("useCertificateChainBytes",
        _usarCadeia(_bytes(chainBytes), password ?? ""));
  }

  void setClientAuthorities(String file, {String? password}) {
    List<int> bytes = (new File(file)).readAsBytesSync();
    setClientAuthoritiesBytes(bytes, password: password);
  }

  void setClientAuthoritiesBytes(List<int> authCertBytes, {String? password}) {
    _verificar("setClientAuthoritiesBytes",
        _usarAutoridades(_bytes(authCertBytes), password ?? ""));
  }

  void setAlpnProtocols(List<String> protocols, bool isServer) {
    Uint8List encodedProtocols =
        SecurityContext._protocolsToLengthEncoding(protocols);
    _setAlpnProtocols(encodedProtocols, isServer);
  }

  @pragma("vm:external-name", "DartForge_tls_contexto_novo")
  external void _createNativeContext();
  @pragma("vm:external-name", "DartForge_tls_contexto_chave")
  external String? _usarChave(Uint8List bytes, String senha);
  @pragma("vm:external-name", "DartForge_tls_contexto_confiaveis")
  external String? _confiar(Uint8List bytes, String senha);
  @pragma("vm:external-name", "DartForge_tls_contexto_cadeia")
  external String? _usarCadeia(Uint8List bytes, String senha);
  @pragma("vm:external-name", "DartForge_tls_contexto_autoridades")
  external String? _usarAutoridades(Uint8List bytes, String senha);
  @pragma("vm:external-name", "DartForge_tls_contexto_alpn")
  external void _setAlpnProtocols(Uint8List protocols, bool isServer);
  @pragma("vm:external-name", "DartForge_tls_contexto_raizes_embutidas")
  external void _trustBuiltinRoots();
  @pragma("vm:external-name", "DartForge_tls_contexto_definir_versao_minima")
  external void _setMinimumProtocolVersion(int version);
  @pragma("vm:external-name", "DartForge_tls_contexto_versao_minima")
  external int _getMinimumProtocolVersion();
}

/// Um certificado X.509 (o DER no campo nativo).
base class _X509CertificateImpl extends NativeFieldWrapperClass1
    implements X509Certificate {
  _X509CertificateImpl._();

  /// O certificado do ponteiro que um native entregou (com uma referência
  /// para este objeto).
  factory _X509CertificateImpl._adotar(int ponteiro) {
    final c = new _X509CertificateImpl._();
    c._fixar(ponteiro);
    return c;
  }

  late final Uint8List der = _der();
  late final String pem = _pem();
  late final Uint8List sha1 = _sha1();

  String get subject => _sujeito();
  String get issuer => _emissor();

  DateTime get startValidity =>
      new DateTime.fromMillisecondsSinceEpoch(_inicio(), isUtc: true);

  DateTime get endValidity =>
      new DateTime.fromMillisecondsSinceEpoch(_fim(), isUtc: true);

  @pragma("vm:external-name", "DartForge_tls_x509_adotar")
  external void _fixar(int ponteiro);
  @pragma("vm:external-name", "DartForge_tls_x509_der")
  external Uint8List _der();
  @pragma("vm:external-name", "DartForge_tls_x509_pem")
  external String _pem();
  @pragma("vm:external-name", "DartForge_tls_x509_sha1")
  external Uint8List _sha1();
  @pragma("vm:external-name", "DartForge_tls_x509_sujeito")
  external String _sujeito();
  @pragma("vm:external-name", "DartForge_tls_x509_emissor")
  external String _emissor();
  @pragma("vm:external-name", "DartForge_tls_x509_inicio")
  external int _inicio();
  @pragma("vm:external-name", "DartForge_tls_x509_fim")
  external int _fim();
}
