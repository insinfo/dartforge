/// O serviço `build.*` do `dfexec/1` do lado do executor
/// (docs/BUILD-PROTOCOLO.md §3): instancia as fábricas do bootstrap e executa
/// `Builder.build` pelo `runBuilder` do próprio `package:build`, com um
/// [AssetReader] e um [AssetWriter] que perguntam tudo ao hospedeiro. O
/// hospedeiro registra cada pergunta como consulta da ação — é assim que o
/// motor sabe o que a ação leu e quando reexecutá-la.
///
/// O `BuildStep.resolver` é o `AnalyzerResolvers` do `build_resolvers`, o
/// mesmo que o `build_runner` usa: ele lê as bibliotecas pelo `BuildStep`,
/// então também essas leituras passam pelo hospedeiro. Quando o banco
/// semântico do DartForge servir o resolver (BUILD-RUST.md, Fase 3), o
/// hospedeiro responderá `build.resolver.*` e este arquivo trocará de
/// implementação sem mudar o protocolo.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/sdk/build_sdk_summary.dart';
import 'package:analyzer/file_system/physical_file_system.dart';
import 'package:build/build.dart';
import 'package:build_resolvers/build_resolvers.dart';
import 'package:glob/glob.dart';
import 'package:logging/logging.dart';
import 'package:package_config/package_config.dart';

import 'canal.dart';

/// Nome e versão do protocolo (o handshake recusa outros).
const protocolo = 'dfexec/1';

/// Com `DARTFORGE_BUILD_EXECUTOR_LOG=1`, o executor narra no stderr as
/// mensagens do protocolo e os passos de cada ação (depuração de um
/// executor que para: o stderr é herdado do hospedeiro).
final bool _narrar = Platform.environment['DARTFORGE_BUILD_EXECUTOR_LOG'] == '1';

void _log(String Function() texto) {
  if (_narrar) stderr.writeln('[executor] ${texto()}');
}

/// Versão do executor de builders; muda quando o comportamento muda.
const versaoDoExecutor = 'dartforge-build-executor/0.2.0';

/// Uma fábrica de builder como o `build.yaml` a declara:
/// `Builder Function(BuilderOptions)` (ou `PostProcessBuilder`).
typedef FabricaDeBuilder = Object Function(BuilderOptions);

/// Atende o hospedeiro até `fim`. [fabricas] é o mapa do bootstrap:
/// chave da aplicação (`pacote:builder`) → nome da fábrica → fábrica.
/// [packageConfig] é o `package_config.json` do projeto: dá as versões de
/// linguagem ao analyzer, como o `build_runner` faz com o do processo dele.
Future<void> servir(
  Map<String, Map<String, FabricaDeBuilder>> fabricas, {
  required String packageConfig,
  Canal? canal,
}) async {
  final c = canal ?? CanalStdio();
  final servico = _Servico(fabricas, c, packageConfig);
  // `print` de um builder fora de uma ação não pode sujar o stdout, que é o
  // canal; dentro da ação o `runBuilder` já o transforma em aviso.
  await runZoned(
    () async {
      await for (final m in c.entrada) {
        if (!await servico.receber(m)) break;
      }
    },
    zoneSpecification: ZoneSpecification(
      print: (_, __, ___, linha) => stderr.writeln(linha),
    ),
  );
  await c.fechar();
}

final class _Servico {
  final Map<String, Map<String, FabricaDeBuilder>> fabricas;
  final Canal canal;
  final String caminhoDoPackageConfig;

  /// Builders já instanciados, por aplicação, fábrica e opções (o
  /// `build_runner` instancia um por fase e o reusa em todas as entradas).
  final _builders = <String, Builder>{};

  /// Pós-processadores já instanciados, pela mesma chave.
  final _posProcessadores = <String, PostProcessBuilder>{};
  final _pendentes = <int, Completer<Map<String, Object?>>>{};
  var _proximoPedido = 1;

  PackageConfig? _packageConfig;
  Resolvers? _resolvers;
  ResourceManager _recursos = ResourceManager();

  /// A ação em curso (o hospedeiro executa uma por vez).
  Future<void>? _emCurso;

  _Servico(this.fabricas, this.canal, this.caminhoDoPackageConfig);

  void _erro(Object? id, String mensagem) =>
      canal.enviar({'t': 'erro', 'id': id, 'mensagem': mensagem});

  /// Trata uma mensagem do hospedeiro; `false` encerra.
  Future<bool> receber(Map<String, Object?> m) async {
    final id = m['id'];
    _log(() => '<- ${m['t']} ${m['id'] ?? ''}');
    switch (m['t']) {
      case 'ola':
        if (m['protocolo'] != protocolo) {
          _erro(null, 'protocolo ${m['protocolo']} não suportado (este executor fala $protocolo)');
          return false;
        }
        canal.enviar({
          't': 'ola',
          'protocolo': protocolo,
          'servicos': ['build'],
          'executor': versaoDoExecutor,
          'abi': '',
        });
      case 'build.carregar':
        _carregar(id, m['script']);
      case 'build.rodada':
        // Uma atualização nova do motor: é o intervalo entre dois builds do
        // `build_runner` (`build_impl.dart:94-96`).
        await _emCurso;
        _resolvers?.reset();
        await _recursos.disposeAll();
        _recursos = ResourceManager();
      case 'build.extensoes':
        // As extensões de execução são as do objeto `Builder`
        // (`expectedOutputs`), não as do `build.yaml`.
        try {
          final opcoes = ((m['opcoes'] as Map?) ?? const {}).cast<String, Object?>();
          final b = _builder(m['chave'] as String, m['fabrica'] as String, opcoes, m['isRoot'] == true);
          canal.enviar({
            't': 'build.extensoes',
            'id': id,
            'extensoes': [
              for (final MapEntry(:key, :value) in b.buildExtensions.entries) [key, value],
            ],
          });
        } catch (e) {
          _erro(id, '$e');
        }
      case 'build.entradas_pos':
        // As `inputExtensions` do objeto `PostProcessBuilder`: é o que o
        // `_actionMatches` do `build_runner_core` usa para as âncoras.
        try {
          final opcoes = ((m['opcoes'] as Map?) ?? const {}).cast<String, Object?>();
          final b = _posProcessador(m['chave'] as String, m['fabrica'] as String, opcoes, m['isRoot'] == true);
          canal.enviar({'t': 'build.entradas_pos', 'id': id, 'entradas': b.inputExtensions.toList()});
        } catch (e) {
          _erro(id, '$e');
        }
      case 'build.executar':
        final acao = _executar(id, m);
        _emCurso = acao;
        unawaited(acao);
      case 'build.posprocessar':
        final acao = _posprocessar(id, m);
        _emCurso = acao;
        unawaited(acao);
      case 'build.resposta':
        final pedido = _pendentes.remove(id);
        if (pedido == null) {
          stderr.writeln('dartforge_build_executor: resposta sem pedido: $m');
        } else {
          pedido.complete(m);
        }
      case 'fim':
        await _emCurso;
        await _recursos.disposeAll();
        await _recursos.beforeExit();
        canal.enviar({'t': 'fim'});
        return false;
      default:
        _erro(id, 'mensagem desconhecida: ${m['t']}');
    }
    return true;
  }

  void _carregar(Object? id, Object? script) {
    final aplicacoes = (script as Map?)?['aplicacoes'] as List? ?? const [];
    final faltam = <String>[];
    for (final a in aplicacoes.cast<Map<String, Object?>>()) {
      final chave = a['chave'] as String;
      for (final f in (a['fabricas'] as List).cast<String>()) {
        if (fabricas[chave]?[f] == null) faltam.add('$chave#$f');
      }
    }
    if (faltam.isNotEmpty) {
      _erro(id, 'o bootstrap não tem as fábricas ${faltam.join(', ')}');
    } else {
      canal.enviar({'t': 'build.carregado', 'id': id});
    }
  }

  /// Um pedido do executor ao hospedeiro (as consultas do `BuildStep`).
  Future<Map<String, Object?>> pedir(String tipo, Map<String, Object?> campos) {
    final id = _proximoPedido++;
    final c = Completer<Map<String, Object?>>();
    _pendentes[id] = c;
    _log(() => '-> $tipo $id ${campos['asset'] ?? campos['padrao'] ?? ''}');
    canal.enviar({'t': tipo, 'id': id, ...campos});
    return c.future;
  }

  Future<PackageConfig> _config() async =>
      _packageConfig ??= await loadPackageConfig(File(caminhoDoPackageConfig));

  Builder _builder(String chave, String fabrica, Map<String, Object?> opcoes, bool raiz) {
    final k = jsonEncode([chave, fabrica, opcoes, raiz]);
    return _builders.putIfAbsent(k, () {
      final f = fabricas[chave]?[fabrica];
      if (f == null) throw StateError('fábrica desconhecida: $chave#$fabrica');
      final b = f(BuilderOptions(opcoes, isRoot: raiz));
      if (b is! Builder) {
        throw StateError('$chave#$fabrica não é um Builder (${b.runtimeType}); '
            'pós-processadores não geram ação no motor');
      }
      return b;
    });
  }

  PostProcessBuilder _posProcessador(String chave, String fabrica, Map<String, Object?> opcoes, bool raiz) {
    final k = jsonEncode([chave, fabrica, opcoes, raiz]);
    return _posProcessadores.putIfAbsent(k, () {
      final f = fabricas[chave]?[fabrica];
      if (f == null) throw StateError('fábrica desconhecida: $chave#$fabrica');
      final b = f(BuilderOptions(opcoes, isRoot: raiz));
      if (b is! PostProcessBuilder) {
        throw StateError('$chave#$fabrica não é um PostProcessBuilder (${b.runtimeType})');
      }
      return b;
    });
  }

  /// Uma âncora de pós-processamento (`_runPostProcessBuilderForAnchor` do
  /// `build_runner_core`): o `runPostProcessBuilder` do `package:build` com o
  /// leitor e o escritor do hospedeiro. O `addAsset` recusa o mesmo asset
  /// duas vezes; o hospedeiro recusa o que o grafo já tem. O
  /// `deletePrimaryInput` volta em `apagados`.
  Future<void> _posprocessar(Object? id, Map<String, Object?> m) async {
    final logs = <Map<String, Object?>>[];
    final apagados = <String>[];
    var falhou = false;
    try {
      final chave = m['chave'] as String;
      final entrada = AssetId.parse(m['entrada'] as String);
      final opcoes = ((m['opcoes'] as Map?) ?? const {}).cast<String, Object?>();
      final builder = _posProcessador(chave, m['fabrica'] as String, opcoes, m['isRoot'] == true);
      final logger = Logger.detached('$builder on $entrada')..level = Level.ALL;
      final assinatura = logger.onRecord.listen((r) {
        if (r.level >= Level.SEVERE) falhou = true;
        logs.add({'nivel': _nivel(r.level), 'mensagem': _texto(r)});
      });
      final adicionados = <AssetId>{};
      try {
        await runPostProcessBuilder(builder, entrada, _Leitor(this), _Escritor(this), logger,
            addAsset: (a) {
          if (!adicionados.add(a)) throw InvalidOutputException(a, 'Asset already exists');
        }, deleteAsset: (a) {
          if (a != entrada) throw InvalidOutputException(a, 'Can only delete primary input');
          if (!apagados.contains('$a')) apagados.add('$a');
        });
      } catch (e, s) {
        // O `scopeLogAsync` já registrou o erro como severo no logger.
        if (!falhou) logs.add({'nivel': 'severo', 'mensagem': '$e\n$s'});
        falhou = true;
      } finally {
        await assinatura.cancel();
      }
    } catch (e, s) {
      logs.add({'nivel': 'severo', 'mensagem': '$e\n$s'});
      falhou = true;
    }
    canal.enviar({
      't': 'build.resultado',
      'id': id,
      'saidas': const [],
      'logs': logs,
      'falhou': falhou,
      'apagados': apagados,
    });
  }

  Future<void> _executar(Object? id, Map<String, Object?> m) async {
    final logs = <Map<String, Object?>>[];
    var falhou = false;
    try {
      final chave = m['chave'] as String;
      final entrada = AssetId.parse(m['entrada'] as String);
      final opcoes = ((m['opcoes'] as Map?) ?? const {}).cast<String, Object?>();
      final builder = _builder(chave, m['fabrica'] as String, opcoes, m['isRoot'] == true);
      _log(() => 'ação ${m['chave']} em ${m['entrada']}: package config');
      final config = await _config();
      _log(() => 'ação: runBuilder');
      final resolvers = _resolvers ??= AnalyzerResolvers.custom(
          packageConfig: config, sdkSummaryGenerator: _geradorDoResumoDoSdk());
      // O mesmo nome de logger do `build_runner` (`_actionLoggerName`).
      final logger = Logger.detached('$chave on $entrada')..level = Level.ALL;
      final assinatura = logger.onRecord.listen((r) {
        if (r.level >= Level.SEVERE) falhou = true;
        logs.add({'nivel': _nivel(r.level), 'mensagem': _texto(r)});
      });
      final leitor = _Leitor(this);
      try {
        await runBuilder(builder, [entrada], leitor, _Escritor(this), resolvers,
            logger: logger, resourceManager: _recursos, packageConfig: config);
      } catch (e, s) {
        // O `runBuilder` já registrou o erro como severo no logger.
        if (!falhou) logs.add({'nivel': 'severo', 'mensagem': '$e\n$s'});
        falhou = true;
      } finally {
        await assinatura.cancel();
      }
    } catch (e, s) {
      logs.add({'nivel': 'severo', 'mensagem': '$e\n$s'});
      falhou = true;
    }
    _log(() => 'ação $id: ${falhou ? 'falhou' : 'ok'}'
        '${logs.where((l) => l['nivel'] == 'severo').map((l) => '\n  ${l['mensagem']}').join()}');
    canal.enviar({'t': 'build.resultado', 'id': id, 'saidas': const [], 'logs': logs, 'falhou': falhou});
  }
}

/// O resumo do SDK para o analyzer quando o executor não é a VM (o executor
/// nativo, B01): o `defaultSdkSummaryGenerator` do `build_resolvers` acha o
/// SDK pelo executável que roda (`Platform.resolvedExecutable`, o `dart`),
/// e aqui o executável é o próprio executor. O hospedeiro diz onde está o
/// SDK (`DARTFORGE_DART_SDK`, a raiz com `lib/` e `version`); o resumo fica
/// em `.dart_tool/dartforge/build/sdk.sum`, refeito quando o SDK muda.
/// Sem a variável (a VM), vale o padrão do `build_resolvers`.
Future<String> Function()? _geradorDoResumoDoSdk() {
  final raiz = Platform.environment['DARTFORGE_DART_SDK'];
  if (raiz == null || raiz.isEmpty) return null;
  return () async {
    _log(() => 'resumo do SDK de $raiz');
    final dir = Directory('.dart_tool/dartforge/build');
    await dir.create(recursive: true);
    final resumo = File('${dir.path}/sdk.sum');
    final deps = File('${dir.path}/sdk.sum.deps');
    final versao = File('$raiz/version');
    // Como o `defaultSdkSummaryGenerator` (versão do SDK e caminho do
    // analyzer), mais o executável que gera: um executor novo (outro
    // compilador, outro analyzer) refaz o resumo.
    final executavel = File(Platform.resolvedExecutable);
    final atual = jsonEncode({
      'sdk': raiz,
      'versao': await versao.exists() ? (await versao.readAsString()).trim() : '',
      'executor': executavel.path,
      'data': (await executavel.lastModified()).microsecondsSinceEpoch,
    });
    if (!await resumo.exists() || !await deps.exists() || await deps.readAsString() != atual) {
      _log(() => 'resumo do SDK: gerando');
      final provisorio = File('${resumo.path}.tmp');
      await provisorio.writeAsBytes(await buildSdkSummary(
          sdkPath: raiz, resourceProvider: PhysicalResourceProvider.INSTANCE));
      await provisorio.rename(resumo.path);
      await deps.writeAsString(atual);
    }
    _log(() => 'resumo do SDK pronto');
    return resumo.absolute.path;
  };
}

String _nivel(Level l) => l >= Level.SEVERE
    ? 'severo'
    : l >= Level.WARNING
        ? 'aviso'
        : l >= Level.INFO
            ? 'info'
            : 'fino';

String _texto(LogRecord r) {
  final b = StringBuffer(r.message);
  if (r.error != null) b.write(b.isEmpty ? '${r.error}' : '\n${r.error}');
  if (r.stackTrace != null && r.level >= Level.SEVERE) b.write('\n${r.stackTrace}');
  return b.toString();
}

/// Leituras pelo hospedeiro. As respostas de uma ação não mudam durante ela
/// (o motor não publica nada no meio de uma ação), então cada asset é pedido
/// uma vez por ação.
final class _Leitor extends AssetReader implements MultiPackageAssetReader {
  final _Servico servico;
  final _bytes = <AssetId, Future<List<int>?>>{};
  final _existe = <AssetId, Future<bool>>{};
  _Leitor(this.servico);

  Future<List<int>?> _ler(AssetId id) => _bytes.putIfAbsent(id, () async {
        final r = await servico.pedir('build.ler', {'asset': '$id'});
        final b = r['bytes_base64'];
        return b is String ? base64Decode(b) : null;
      });

  @override
  Future<List<int>> readAsBytes(AssetId id) async =>
      await _ler(id) ?? (throw AssetNotFoundException(id));

  @override
  Future<String> readAsString(AssetId id, {Encoding encoding = utf8}) async =>
      encoding.decode(await readAsBytes(id));

  @override
  Future<bool> canRead(AssetId id) => _existe.putIfAbsent(id, () async {
        final r = await servico.pedir('build.existe', {'asset': '$id'});
        return r['sim'] == true;
      });

  @override
  Stream<AssetId> findAssets(Glob glob, {String? package}) async* {
    // O hospedeiro limita a busca ao pacote da entrada primária, como o
    // `SingleStepReader` do `build_runner_core`.
    final r = await servico.pedir('build.glob', {'padrao': glob.pattern});
    for (final a in (r['assets'] as List? ?? const []).cast<String>()) {
      yield AssetId.parse(a);
    }
  }
}

final class _Escritor implements AssetWriter {
  final _Servico servico;
  _Escritor(this.servico);

  @override
  Future<void> writeAsBytes(AssetId id, List<int> bytes) async {
    final r = await servico.pedir('build.escrever', {'asset': '$id', 'bytes_base64': base64Encode(bytes)});
    final erro = r['erro'];
    if (erro != null) throw InvalidOutputException(id, '$erro');
  }

  @override
  Future<void> writeAsString(AssetId id, String contents, {Encoding encoding = utf8}) =>
      writeAsBytes(id, encoding.encode(contents));
}
