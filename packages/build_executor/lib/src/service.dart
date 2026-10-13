/// The `build.*` service of `dfexec/1` on the executor side
/// (docs/BUILD-PROTOCOLO.md §3): instantiates the bootstrap's factories and
/// runs `Builder.build` through `package:build`'s own `runBuilder`, with an
/// [AssetReader] and an [AssetWriter] that ask the host for everything. The
/// host records each question as a query of the action — that is how the
/// engine knows what the action read and when to rerun it.
///
/// `BuildStep.resolver` is `build_resolvers`' `AnalyzerResolvers`, the same
/// one `build_runner` uses: it reads libraries through the `BuildStep`, so
/// those reads go through the host too. When DartForge's semantic database
/// serves the resolver (BUILD-RUST.md, Phase 3), the host will answer
/// `build.resolver.*` and this file will switch implementation without
/// changing the protocol.
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

import 'channel.dart';

/// Protocol name and version (the handshake rejects any other).
const protocol = 'dfexec/1';

/// With `DARTFORGE_BUILD_EXECUTOR_LOG=1`, the executor narrates the protocol
/// messages and the steps of each action on stderr (debugging an executor
/// that stalls: stderr is inherited from the host).
final bool _narrate = Platform.environment['DARTFORGE_BUILD_EXECUTOR_LOG'] == '1';

void _log(String Function() text) {
  if (_narrate) stderr.writeln('[executor] ${text()}');
}

/// Version of the builder executor; changes when the behavior changes.
const executorVersion = 'dartforge-build-executor/0.2.0';

/// A builder factory as `build.yaml` declares it:
/// `Builder Function(BuilderOptions)` (or `PostProcessBuilder`).
typedef BuilderFactory = Object Function(BuilderOptions);

/// Serves the host until `end`. [factories] is the bootstrap's map:
/// application key (`package:builder`) → factory name → factory.
/// [packageConfig] is the project's `package_config.json`: it gives the
/// language versions to the analyzer, as `build_runner` does with its own.
Future<void> serve(
  Map<String, Map<String, BuilderFactory>> factories, {
  required String packageConfig,
  Channel? channel,
}) async {
  final c = channel ?? StdioChannel();
  final service = _Service(factories, c, packageConfig);
  // A builder's `print` outside an action must not dirty stdout, which is the
  // channel; inside an action `runBuilder` already turns it into a warning.
  await runZoned(
    () async {
      await for (final m in c.input) {
        if (!await service.receive(m)) break;
      }
    },
    zoneSpecification: ZoneSpecification(
      print: (_, __, ___, line) => stderr.writeln(line),
    ),
  );
  await c.close();
}

final class _Service {
  final Map<String, Map<String, BuilderFactory>> factories;
  final Channel channel;
  final String packageConfigPath;

  /// Builders already instantiated, by application, factory and options
  /// (`build_runner` instantiates one per phase and reuses it for all inputs).
  final _builders = <String, Builder>{};

  /// Post-processors already instantiated, by the same key.
  final _postProcessors = <String, PostProcessBuilder>{};
  final _pending = <int, Completer<Map<String, Object?>>>{};
  var _nextRequest = 1;

  PackageConfig? _packageConfig;
  Resolvers? _resolvers;
  ResourceManager _resources = ResourceManager();

  /// The action in progress (the host runs one at a time).
  Future<void>? _inProgress;

  _Service(this.factories, this.channel, this.packageConfigPath);

  void _error(Object? id, String message) =>
      channel.send({'t': 'error', 'id': id, 'message': message});

  /// Handles a message from the host; `false` ends the session.
  Future<bool> receive(Map<String, Object?> m) async {
    final id = m['id'];
    _log(() => '<- ${m['t']} ${m['id'] ?? ''}');
    switch (m['t']) {
      case 'hello':
        if (m['protocol'] != protocol) {
          _error(null, 'protocol ${m['protocol']} not supported (this executor speaks $protocol)');
          return false;
        }
        channel.send({
          't': 'hello',
          'protocol': protocol,
          'services': ['build'],
          'executor': executorVersion,
          'abi': '',
        });
      case 'build.load':
        _load(id, m['script']);
      case 'build.round':
        // A new update from the engine: it is the interval between two
        // `build_runner` builds (`build_impl.dart:94-96`).
        await _inProgress;
        _resolvers?.reset();
        await _resources.disposeAll();
        _resources = ResourceManager();
      case 'build.extensions':
        // The runtime extensions are those of the `Builder` object
        // (`expectedOutputs`), not those of `build.yaml`.
        try {
          final options = ((m['options'] as Map?) ?? const {}).cast<String, Object?>();
          final b = _builder(m['key'] as String, m['factory'] as String, options, m['isRoot'] == true);
          channel.send({
            't': 'build.extensions',
            'id': id,
            'extensions': [
              for (final MapEntry(:key, :value) in b.buildExtensions.entries) [key, value],
            ],
          });
        } catch (e) {
          _error(id, '$e');
        }
      case 'build.post_inputs':
        // The `inputExtensions` of the `PostProcessBuilder` object: it is what
        // `build_runner_core`'s `_actionMatches` uses for anchors.
        try {
          final options = ((m['options'] as Map?) ?? const {}).cast<String, Object?>();
          final b = _postProcessor(m['key'] as String, m['factory'] as String, options, m['isRoot'] == true);
          channel.send({'t': 'build.post_inputs', 'id': id, 'inputs': b.inputExtensions.toList()});
        } catch (e) {
          _error(id, '$e');
        }
      case 'build.run':
        final action = _run(id, m);
        _inProgress = action;
        unawaited(action);
      case 'build.post_process':
        final action = _postProcess(id, m);
        _inProgress = action;
        unawaited(action);
      case 'build.response':
        final request = _pending.remove(id);
        if (request == null) {
          stderr.writeln('dartforge_build_executor: response without a request: $m');
        } else {
          request.complete(m);
        }
      case 'end':
        await _inProgress;
        await _resources.disposeAll();
        await _resources.beforeExit();
        channel.send({'t': 'end'});
        return false;
      default:
        _error(id, 'unknown message: ${m['t']}');
    }
    return true;
  }

  void _load(Object? id, Object? script) {
    final applications = (script as Map?)?['applications'] as List? ?? const [];
    final missing = <String>[];
    for (final a in applications.cast<Map<String, Object?>>()) {
      final key = a['key'] as String;
      for (final f in (a['factories'] as List).cast<String>()) {
        if (factories[key]?[f] == null) missing.add('$key#$f');
      }
    }
    if (missing.isNotEmpty) {
      _error(id, 'the bootstrap lacks the factories ${missing.join(', ')}');
    } else {
      channel.send({'t': 'build.loaded', 'id': id});
    }
  }

  /// A request from the executor to the host (the `BuildStep` queries).
  Future<Map<String, Object?>> request(String kind, Map<String, Object?> fields) {
    final id = _nextRequest++;
    final c = Completer<Map<String, Object?>>();
    _pending[id] = c;
    _log(() => '-> $kind $id ${fields['asset'] ?? fields['pattern'] ?? ''}');
    channel.send({'t': kind, 'id': id, ...fields});
    return c.future;
  }

  Future<PackageConfig> _config() async =>
      _packageConfig ??= await loadPackageConfig(File(packageConfigPath));

  Builder _builder(String key, String factory, Map<String, Object?> options, bool isRoot) {
    final k = jsonEncode([key, factory, options, isRoot]);
    return _builders.putIfAbsent(k, () {
      final f = factories[key]?[factory];
      if (f == null) throw StateError('unknown factory: $key#$factory');
      final b = f(BuilderOptions(options, isRoot: isRoot));
      if (b is! Builder) {
        throw StateError('$key#$factory is not a Builder (${b.runtimeType}); '
            'post-processors do not produce an engine action');
      }
      return b;
    });
  }

  PostProcessBuilder _postProcessor(String key, String factory, Map<String, Object?> options, bool isRoot) {
    final k = jsonEncode([key, factory, options, isRoot]);
    return _postProcessors.putIfAbsent(k, () {
      final f = factories[key]?[factory];
      if (f == null) throw StateError('unknown factory: $key#$factory');
      final b = f(BuilderOptions(options, isRoot: isRoot));
      if (b is! PostProcessBuilder) {
        throw StateError('$key#$factory is not a PostProcessBuilder (${b.runtimeType})');
      }
      return b;
    });
  }

  /// A post-processing anchor (`build_runner_core`'s
  /// `_runPostProcessBuilderForAnchor`): `package:build`'s
  /// `runPostProcessBuilder` with the host's reader and writer. `addAsset`
  /// rejects the same asset twice; the host rejects what the graph already
  /// has. `deletePrimaryInput` comes back in `deleted`.
  Future<void> _postProcess(Object? id, Map<String, Object?> m) async {
    final logs = <Map<String, Object?>>[];
    final deleted = <String>[];
    var failed = false;
    try {
      final key = m['key'] as String;
      final input = AssetId.parse(m['input'] as String);
      final options = ((m['options'] as Map?) ?? const {}).cast<String, Object?>();
      final builder = _postProcessor(key, m['factory'] as String, options, m['isRoot'] == true);
      final logger = Logger.detached('$builder on $input')..level = Level.ALL;
      final subscription = logger.onRecord.listen((r) {
        if (r.level >= Level.SEVERE) failed = true;
        logs.add({'level': _level(r.level), 'message': _text(r)});
      });
      final added = <AssetId>{};
      try {
        await runPostProcessBuilder(builder, input, _Reader(this), _Writer(this), logger,
            addAsset: (a) {
          if (!added.add(a)) throw InvalidOutputException(a, 'Asset already exists');
        }, deleteAsset: (a) {
          if (a != input) throw InvalidOutputException(a, 'Can only delete primary input');
          if (!deleted.contains('$a')) deleted.add('$a');
        });
      } catch (e, s) {
        // `scopeLogAsync` already logged the error as severe.
        if (!failed) logs.add({'level': 'severe', 'message': '$e\n$s'});
        failed = true;
      } finally {
        await subscription.cancel();
      }
    } catch (e, s) {
      logs.add({'level': 'severe', 'message': '$e\n$s'});
      failed = true;
    }
    channel.send({
      't': 'build.result',
      'id': id,
      'outputs': const [],
      'logs': logs,
      'failed': failed,
      'deleted': deleted,
    });
  }

  Future<void> _run(Object? id, Map<String, Object?> m) async {
    final logs = <Map<String, Object?>>[];
    var failed = false;
    try {
      final key = m['key'] as String;
      final input = AssetId.parse(m['input'] as String);
      final options = ((m['options'] as Map?) ?? const {}).cast<String, Object?>();
      final builder = _builder(key, m['factory'] as String, options, m['isRoot'] == true);
      _log(() => 'action ${m['key']} on ${m['input']}: package config');
      final config = await _config();
      _log(() => 'action: runBuilder');
      final resolvers = _resolvers ??= AnalyzerResolvers.custom(
          packageConfig: config, sdkSummaryGenerator: _sdkSummaryGenerator());
      // The same logger name as `build_runner` (`_actionLoggerName`).
      final logger = Logger.detached('$key on $input')..level = Level.ALL;
      final subscription = logger.onRecord.listen((r) {
        if (r.level >= Level.SEVERE) failed = true;
        logs.add({'level': _level(r.level), 'message': _text(r)});
      });
      final reader = _Reader(this);
      try {
        await runBuilder(builder, [input], reader, _Writer(this), resolvers,
            logger: logger, resourceManager: _resources, packageConfig: config);
      } catch (e, s) {
        // `runBuilder` already logged the error as severe.
        if (!failed) logs.add({'level': 'severe', 'message': '$e\n$s'});
        failed = true;
      } finally {
        await subscription.cancel();
      }
    } catch (e, s) {
      logs.add({'level': 'severe', 'message': '$e\n$s'});
      failed = true;
    }
    _log(() => 'action $id: ${failed ? 'failed' : 'ok'}'
        '${logs.where((l) => l['level'] == 'severe').map((l) => '\n  ${l['message']}').join()}');
    channel.send({'t': 'build.result', 'id': id, 'outputs': const [], 'logs': logs, 'failed': failed});
  }
}

/// The SDK summary for the analyzer when the executor is not the VM (the
/// native executor, B01): `build_resolvers`' `defaultSdkSummaryGenerator`
/// finds the SDK through the running executable
/// (`Platform.resolvedExecutable`, `dart`), and here the executable is the
/// executor itself. The host says where the SDK is (`DARTFORGE_DART_SDK`, the
/// root with `lib/` and `version`); the summary lives in
/// `.dart_tool/dartforge/build/sdk.sum`, rebuilt when the SDK changes.
/// Without the variable (the VM), `build_resolvers`' default applies.
Future<String> Function()? _sdkSummaryGenerator() {
  final root = Platform.environment['DARTFORGE_DART_SDK'];
  if (root == null || root.isEmpty) return null;
  return () async {
    _log(() => 'SDK summary from $root');
    final dir = Directory('.dart_tool/dartforge/build');
    await dir.create(recursive: true);
    final summary = File('${dir.path}/sdk.sum');
    final deps = File('${dir.path}/sdk.sum.deps');
    final version = File('$root/version');
    // Like `defaultSdkSummaryGenerator` (SDK version and analyzer path), plus
    // the executable that generates it: a new executor (another compiler,
    // another analyzer) rebuilds the summary.
    final executable = File(Platform.resolvedExecutable);
    final current = jsonEncode({
      'sdk': root,
      'version': await version.exists() ? (await version.readAsString()).trim() : '',
      'executor': executable.path,
      'data': (await executable.lastModified()).microsecondsSinceEpoch,
    });
    if (!await summary.exists() || !await deps.exists() || await deps.readAsString() != current) {
      _log(() => 'SDK summary: generating');
      final temporary = File('${summary.path}.tmp');
      await temporary.writeAsBytes(await buildSdkSummary(
          sdkPath: root, resourceProvider: PhysicalResourceProvider.INSTANCE));
      await temporary.rename(summary.path);
      await deps.writeAsString(current);
    }
    _log(() => 'SDK summary ready');
    return summary.absolute.path;
  };
}

String _level(Level l) => l >= Level.SEVERE
    ? 'severe'
    : l >= Level.WARNING
        ? 'warning'
        : l >= Level.INFO
            ? 'info'
            : 'fine';

String _text(LogRecord r) {
  final b = StringBuffer(r.message);
  if (r.error != null) b.write(b.isEmpty ? '${r.error}' : '\n${r.error}');
  if (r.stackTrace != null && r.level >= Level.SEVERE) b.write('\n${r.stackTrace}');
  return b.toString();
}

/// Reads through the host. The answers of an action do not change during it
/// (the engine publishes nothing in the middle of an action), so each asset
/// is requested once per action.
final class _Reader extends AssetReader implements MultiPackageAssetReader {
  final _Service service;
  final _bytes = <AssetId, Future<List<int>?>>{};
  final _exists = <AssetId, Future<bool>>{};
  _Reader(this.service);

  Future<List<int>?> _read(AssetId id) => _bytes.putIfAbsent(id, () async {
        final r = await service.request('build.read', {'asset': '$id'});
        final b = r['bytes_base64'];
        return b is String ? base64Decode(b) : null;
      });

  @override
  Future<List<int>> readAsBytes(AssetId id) async =>
      await _read(id) ?? (throw AssetNotFoundException(id));

  @override
  Future<String> readAsString(AssetId id, {Encoding encoding = utf8}) async =>
      encoding.decode(await readAsBytes(id));

  @override
  Future<bool> canRead(AssetId id) => _exists.putIfAbsent(id, () async {
        final r = await service.request('build.exists', {'asset': '$id'});
        return r['exists'] == true;
      });

  @override
  Stream<AssetId> findAssets(Glob glob, {String? package}) async* {
    // The host limits the search to the primary input's package, like
    // `build_runner_core`'s `SingleStepReader`.
    final r = await service.request('build.glob', {'pattern': glob.pattern});
    for (final a in (r['assets'] as List? ?? const []).cast<String>()) {
      yield AssetId.parse(a);
    }
  }
}

final class _Writer implements AssetWriter {
  final _Service service;
  _Writer(this.service);

  @override
  Future<void> writeAsBytes(AssetId id, List<int> bytes) async {
    final r = await service.request('build.write', {'asset': '$id', 'bytes_base64': base64Encode(bytes)});
    final error = r['error'];
    if (error != null) throw InvalidOutputException(id, '$error');
  }

  @override
  Future<void> writeAsString(AssetId id, String contents, {Encoding encoding = utf8}) =>
      writeAsBytes(id, encoding.encode(contents));
}
