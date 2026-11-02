// The DartForge test host, compiled with DartForge AOT against the package's
// own test_core.
//
// It is `dart test` with one platform replaced. It reads the command line and
// the dart_test.yaml files with package:test's `Configuration`, finds the
// suites like its `Runner` and `Loader`, and runs them through package:test's
// own `Engine`, reporters (`expanded`, `compact`, `json`, `github`,
// `failures-only`, `silent`, file reporters) and `--fail-fast`. Each suite is
// compiled by DartForge into a native executable around test_core's own
// native bootstrap (`internalBootstrapNativeTest`, the `exe` compiler of
// `dart test`) and talks to this host over a socket with test_core's
// `deserializeSuite` protocol, exactly as `VMPlatform` does for `-c exe`.
//
// The only parts DartForge supplies are the library annotations of each suite
// (read by DartForge instead of the analyzer that `parseMetadata` uses) and
// the native compilation itself.
import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:async/async.dart';
import 'package:boolean_selector/boolean_selector.dart';
import 'package:package_config/package_config.dart';
import 'package:path/path.dart' as p;
import 'package:pool/pool.dart';
import 'package:source_span/source_span.dart';
import 'package:stack_trace/stack_trace.dart';
import 'package:stream_channel/stream_channel.dart';
import 'package:test_api/scaffolding.dart' show Timeout;
import 'package:test_api/src/backend/compiler.dart';
import 'package:test_api/src/backend/group.dart';
import 'package:test_api/src/backend/group_entry.dart';
import 'package:test_api/src/backend/invoker.dart';
import 'package:test_api/src/backend/metadata.dart';
import 'package:test_api/src/backend/operating_system.dart';
import 'package:test_api/src/backend/platform_selector.dart';
import 'package:test_api/src/backend/runtime.dart';
import 'package:test_api/src/backend/suite.dart';
import 'package:test_api/src/backend/suite_platform.dart';
import 'package:test_api/src/backend/test.dart';
import 'package:test_api/src/backend/util/identifier_regex.dart';
import 'package:test_api/src/backend/util/pretty_print.dart' show toSentence;
import 'package:test_core/src/runner/application_exception.dart';
import 'package:test_core/src/runner/configuration.dart';
import 'package:test_core/src/runner/configuration/reporters.dart'
    show allReporters;
import 'package:test_core/src/runner/engine.dart';
import 'package:test_core/src/runner/environment.dart';
import 'package:test_core/src/runner/load_exception.dart';
import 'package:test_core/src/runner/load_suite.dart';
import 'package:test_core/src/runner/no_tests_found_exception.dart';
import 'package:test_core/src/runner/plugin/platform_helpers.dart'
    show deserializeSuite;
import 'package:test_core/src/runner/plugin/shared_platform_helpers.dart'
    show jsonSocketStreamChannel;
import 'package:test_core/src/runner/reporter.dart';
import 'package:test_core/src/runner/reporter/multiplex.dart';
import 'package:test_core/src/runner/runner_suite.dart';
import 'package:test_core/src/runner/suite.dart';
import 'package:test_core/src/runner/version.dart';
import 'package:test_core/src/util/errors.dart' show getErrorMessage;
import 'package:test_core/src/util/io.dart'
    show currentOS, currentPlatform, inGoogle, warn, wordWrap;

/// The runtimes `dart test` registers in package:test 1.25 and 1.26, in
/// registration order; they are the platform variables of suite metadata.
const _platformVariables = [
  'vm',
  'node',
  'chrome',
  'edge',
  'firefox',
  'safari'
];

/// DartForge runs suites as native executables, the `exe` compiler of the VM.
const _compiler = Compiler.exe;

/// Whether stack traces of this executable, compiled like the suites, carry
/// frames. Without them the suites do not collect declaration traces: the
/// reporters then see no trace at all (the JSON reporter's `line`, `column`
/// and `url` are null) instead of an empty one, which they cannot read.
final bool _tracesHaveFrames = Trace.current().frames.isNotEmpty;

/// What the DartForge CLI passes in the first argument (a JSON file).
late final Map _settings;

/// `executable.dart`'s global configuration path.
String get _globalConfigPath {
  if (Platform.environment.containsKey('DART_TEST_CONFIG')) {
    return Platform.environment['DART_TEST_CONFIG']!;
  } else if (Platform.operatingSystem == 'windows') {
    return p.join(Platform.environment['LOCALAPPDATA']!, 'DartTest.yaml');
  } else {
    return '${Platform.environment['HOME']}/.dart_test.yaml';
  }
}

/// `executable.dart`'s `_printUsage`.
void _printUsage([String? error]) {
  IOSink output = stdout;
  var message = 'Runs tests in this package.';
  if (error != null) {
    message = error;
    output = stderr;
  }
  output.write('''${wordWrap(message)}

Usage: dart test [files or directories...]

${Configuration.usage}
''');
}

Future<void> main(List<String> arguments) async {
  _settings = jsonDecode(File(arguments.first).readAsStringSync()) as Map;
  await _execute(arguments.sublist(1));
}

/// `executable.dart`'s `_execute`, with the runner below.
Future<void> _execute(List<String> args) async {
  Configuration configuration;
  try {
    configuration = Configuration.parse(args);
  } on FormatException catch (error) {
    _printUsage(error.message);
    exitCode = 64;
    return;
  }

  if (configuration.help) {
    _printUsage();
    return;
  }

  if (configuration.version) {
    final version = testVersion;
    if (version == null) {
      stderr.writeln("Couldn't find version number.");
      exitCode = 65;
    } else {
      print(version);
    }
    return;
  }

  try {
    var fileConfiguration = Configuration.empty;
    if (File(_globalConfigPath).existsSync()) {
      fileConfiguration = fileConfiguration
          .merge(Configuration.load(_globalConfigPath, global: true));
    }
    if (File(configuration.configurationPath).existsSync()) {
      fileConfiguration = fileConfiguration
          .merge(Configuration.load(configuration.configurationPath));
    }
    configuration = fileConfiguration.merge(configuration);
  } on SourceSpanFormatException catch (error) {
    stderr.writeln(error.toString(color: configuration.color));
    exitCode = 65;
    return;
  } on FormatException catch (error) {
    stderr.writeln(error.message);
    exitCode = 65;
    return;
  } on IOException catch (error) {
    stderr.writeln(error.toString());
    exitCode = 66;
    return;
  }

  final undefinedPresets = configuration.chosenPresets
      .where((preset) => !configuration.knownPresets.contains(preset))
      .toList();
  if (undefinedPresets.isNotEmpty) {
    _printUsage("Undefined preset${undefinedPresets.length == 1 ? '' : 's'} "
        "${toSentence(undefinedPresets.map((preset) => '"$preset"'))}.");
    exitCode = 64;
    return;
  }

  if (!configuration.explicitPaths &&
      !Directory(configuration.testSelections.keys.single).existsSync()) {
    _printUsage('No test files were passed and the default "test/" '
        "directory doesn't exist.");
    exitCode = 65;
    return;
  }

  final refusal = _unsupported(configuration);
  if (refusal != null) {
    stderr.writeln('error: test: $refusal');
    exitCode = 64;
    return;
  }

  _Runner? runner;
  StreamSubscription? signals;
  try {
    signals = StreamGroup.merge(
            [ProcessSignal.sigterm.watch(), ProcessSignal.sigint.watch()])
        .listen((signal) async {
      await signals?.cancel();
      await runner?.close();
    });
  } on Object {
    // Without signal support the default handlers stay in place.
  }

  try {
    runner = _Runner(configuration);
    exitCode = (await runner.run()) ? 0 : 1;
  } on ApplicationException catch (error) {
    stderr.writeln(error.message);
    exitCode = 65;
  } on SourceSpanFormatException catch (error) {
    stderr.writeln(error.toString(color: configuration.color));
    exitCode = 65;
  } on FormatException catch (error) {
    stderr.writeln(error.message);
    exitCode = 65;
  } on NoTestsFoundException catch (error) {
    stderr.writeln(error.message);
    exitCode = 79;
  } catch (error, stackTrace) {
    stderr.writeln(getErrorMessage(error));
    stderr.writeln(Trace.from(stackTrace).terse);
    stderr.writeln('This is an unexpected error. Please file an issue at '
        'http://github.com/dart-lang/test\n'
        'with the stack trace and instructions for reproducing the error.');
    exitCode = 70;
  } finally {
    await runner?.close();
    await signals?.cancel();
  }
}

/// Options the native runner cannot honor are refused instead of ignored.
String? _unsupported(Configuration configuration) {
  if (configuration.coverage != null) {
    return 'coverage collection is not supported by the native runner';
  }
  if (configuration.pauseAfterLoad || configuration.debug) {
    return 'debugging (--pause-after-load, --debug) is not supported by the '
        'native runner';
  }
  if (configuration.defineRuntimes.isNotEmpty ||
      configuration.overrideRuntimes.isNotEmpty) {
    return 'define_platforms and override_platforms are not supported: '
        'the native runner only runs the vm platform';
  }
  final suiteDefaults = configuration.suiteDefaults;
  final others = suiteDefaults.runtimes.where((r) => r != 'vm').toSet();
  if (others.isNotEmpty) {
    return 'the native runner only runs the vm platform; '
        '${others.join(', ')} ${others.length == 1 ? 'is' : 'are'} not '
        'supported';
  }
  final compilers = suiteDefaults.compilerSelections;
  if (compilers != null &&
      compilers.any((selection) => selection.compiler != _compiler)) {
    return 'the native runner compiles suites as native executables; only '
        'the exe compiler can be selected';
  }
  for (final selections in configuration.testSelections.values) {
    for (final selection in selections) {
      if (selection.line != null || selection.col != null) {
        return 'line and column test selection is not supported by the '
            'native runner';
      }
    }
  }
  return null;
}

/// test_core's `Runner`, with [_NativeLoader] in place of its `Loader`.
class _Runner {
  final Configuration _config;
  final Engine _engine;
  final Reporter _reporter;
  final List<IOSink> _sinks;
  final _NativeLoader _loader;
  StreamSubscription? _suiteSubscription;
  final _tagWarningSuites = <String>{};
  final _closeMemo = AsyncMemoizer<void>();
  bool get _closed => _closeMemo.hasRun;

  _Runner._(this._config, this._engine, this._reporter, this._sinks)
      : _loader = _NativeLoader(_config);

  factory _Runner(Configuration config) => config.asCurrent(() {
        final engine = Engine(
          concurrency: config.concurrency,
          coverage: config.coverage,
          testRandomizeOrderingSeed: config.testRandomizeOrderingSeed,
          stopOnFirstFailure: config.stopOnFirstFailure,
        );
        final sinks = <IOSink>[];
        Reporter createFileReporter(String reporterName, String filepath) {
          final sink =
              (File(filepath)..createSync(recursive: true)).openWrite();
          sinks.add(sink);
          return allReporters[reporterName]!.factory(config, engine, sink);
        }

        return _Runner._(
          config,
          engine,
          MultiplexReporter([
            allReporters[config.reporter]!.factory(config, engine, stdout),
            for (final reporter in config.fileReporters.keys)
              createFileReporter(reporter, config.fileReporters[reporter]!),
          ]),
          sinks,
        );
      });

  Future<bool> run() => _config.asCurrent(() async {
        if (_closed) {
          throw StateError('run() may not be called on a closed Runner.');
        }
        _warnForUnsupportedPlatforms(_config);
        final suites = _loadSuites();
        final subscription =
            _suiteSubscription = suites.listen(_engine.suiteSink.add);
        final results = await Future.wait(<Future>[
          subscription.asFuture<void>().then((_) => _engine.suiteSink.close()),
          _engine.run()
        ], eagerError: true);
        final success = results.last as bool?;
        if (_closed) return false;

        if (_engine.passed.isEmpty &&
            _engine.failed.isEmpty &&
            _engine.skipped.isEmpty) {
          if (_config.globalPatterns.isNotEmpty) {
            final patterns = toSentence(_config.globalPatterns.map((pattern) =>
                pattern is RegExp
                    ? 'regular expression "${pattern.pattern}"'
                    : '"$pattern"'));
            throw NoTestsFoundException('No tests match $patterns.');
          } else if (_config.includeTags != BooleanSelector.all ||
              _config.excludeTags != BooleanSelector.none) {
            throw NoTestsFoundException(
                'No tests match the requested tag selectors:\n'
                '  include: "${_config.includeTags}"\n'
                '  exclude: "${_config.excludeTags}"');
          } else {
            throw NoTestsFoundException('No tests were found.');
          }
        }
        return (success ?? false) &&
            (_engine.passed.isNotEmpty || _engine.skipped.isNotEmpty);
      });

  Future close() => _closeMemo.runOnce(() async {
        Timer? timer;
        if (!_engine.isIdle) {
          timer = Timer(const Duration(seconds: 1), () {
            _reporter.pause();
            print('Waiting for current test(s) to finish.');
            print('Press Control-C again to terminate immediately.');
            _reporter.resume();
          });
        }
        await _suiteSubscription?.cancel();
        _suiteSubscription = null;
        await _engine.close();
        timer?.cancel();
        await _loader.close();
        await Future.wait(_sinks.map((s) => s.flush().then((_) => s.close())));
        _sinks.clear();
      });

  /// `Runner._loadSuites`.
  Stream<LoadSuite> _loadSuites() {
    return StreamGroup.merge(_config.testSelections.entries.map((pathEntry) {
      final testPath = pathEntry.key;
      final testSelections = pathEntry.value;
      final suiteConfig = _config.suiteDefaults.selectTests(testSelections);
      if (Directory(testPath).existsSync()) {
        return _loader.loadDir(testPath, suiteConfig);
      } else if (File(testPath).existsSync()) {
        return _loader.loadFile(testPath, suiteConfig);
      } else {
        return Stream.fromIterable([
          LoadSuite.forLoadException(
            LoadException(testPath, 'Does not exist.'),
            suiteConfig,
          ),
        ]);
      }
    })).map((loadSuite) {
      return loadSuite.changeSuite((suite) {
        _warnForUnknownTags(suite);

        return _shardSuite(suite.filter((test) {
          if (!_config.globalPatterns
              .every((pattern) => test.name.contains(pattern))) {
            return false;
          }
          if (!_config.includeTags.evaluate(test.metadata.tags.contains)) {
            return false;
          }
          if (_config.excludeTags.evaluate(test.metadata.tags.contains)) {
            return false;
          }
          final testSelections = suite.config.testSelections;
          assert(testSelections.isNotEmpty, 'Tests should have been selected');
          return testSelections.any((selection) => selection.testPatterns
              .every((pattern) => test.name.contains(pattern)));
        }));
      });
    });
  }

  void _warnForUnknownTags(Suite suite) {
    if (_tagWarningSuites.contains(suite.path)) return;
    _tagWarningSuites.add(suite.path!);

    final unknownTags = _collectUnknownTags(suite);
    if (unknownTags.isEmpty) return;

    final yellow = _config.color ? '\u001b[33m' : '';
    final bold = _config.color ? '\u001b[1m' : '';
    final noColor = _config.color ? '\u001b[0m' : '';

    final buffer = StringBuffer()
      ..write('${yellow}Warning:$noColor ')
      ..write(unknownTags.length == 1 ? 'A tag was ' : 'Tags were ')
      ..write('used that ')
      ..write(unknownTags.length == 1 ? "wasn't " : "weren't ")
      ..writeln('specified in dart_test.yaml.');

    unknownTags.forEach((tag, entries) {
      buffer.write('  $bold$tag$noColor was used in');
      if (entries.length == 1) {
        buffer.writeln(' ${_entryDescription(entries.single)}');
        return;
      }
      buffer.write(':');
      for (final entry in entries) {
        buffer.write('\n    ${_entryDescription(entry)}');
      }
      buffer.writeln();
    });

    print(buffer.toString());
  }

  Map<String, List<GroupEntry>> _collectUnknownTags(Suite suite) {
    final unknownTags = <String, List<GroupEntry>>{};
    final currentTags = <String>{};

    void collect(GroupEntry entry) {
      final newTags = <String>{};
      for (final unknownTag
          in entry.metadata.tags.difference(_config.knownTags)) {
        if (currentTags.contains(unknownTag)) continue;
        unknownTags.putIfAbsent(unknownTag, () => []).add(entry);
        newTags.add(unknownTag);
      }
      if (entry is! Group) return;
      currentTags.addAll(newTags);
      for (final child in entry.entries) {
        collect(child);
      }
      currentTags.removeAll(newTags);
    }

    collect(suite.group);
    return unknownTags;
  }

  String _entryDescription(GroupEntry entry) {
    if (entry is Test) return 'the test "${entry.name}"';
    if (entry.name.isNotEmpty) return 'the group "${entry.name}"';
    return 'the suite itself';
  }

  RunnerSuite _shardSuite(RunnerSuite suite) {
    if (_config.totalShards == null) return suite;
    final shardSize = suite.group.testCount / _config.totalShards!;
    final shardIndex = _config.shardIndex!;
    final shardStart = (shardSize * shardIndex).round();
    final shardEnd = (shardSize * (shardIndex + 1)).round();
    var count = -1;
    return suite.filter((test) {
      count++;
      return count >= shardStart && count < shardEnd;
    });
  }
}

/// `Runner._warnForUnsupportedPlatforms` for the only runtime used here.
void _warnForUnsupportedPlatforms(Configuration configuration) {
  final testOn = configuration.suiteDefaults.metadata.testOn;
  if (testOn == PlatformSelector.all) return;
  if (testOn.evaluate(currentPlatform(Runtime.vm, null))) return;
  final supportsAnyOS = OperatingSystem.all.any((os) => testOn.evaluate(
      SuitePlatform(Runtime.vm, compiler: null, os: os, inGoogle: inGoogle)));
  warn("this package doesn't support running tests on "
      '${supportsAnyOS ? currentOS.name : 'the Dart VM'}.');
}

/// test_core's `Loader` for the vm runtime and the exe compiler, with the
/// library annotations read by DartForge and [_NativePlatform] as the
/// platform plugin.
class _NativeLoader {
  final Configuration _config;
  final _platform = _NativePlatform();

  /// DartForge's reading of each suite's annotations, by path.
  final _libraries = <String, Map?>{};
  final _suites = <RunnerSuite>{};
  final _closeMemo = AsyncMemoizer<void>();

  _NativeLoader(this._config);

  /// Reads the annotations of every suite of [paths] in one DartForge call.
  void _readLibraries(Iterable<String> paths) {
    final missing = paths.where((path) => !_libraries.containsKey(path));
    if (missing.isEmpty) return;
    final command = (_settings['metadataCommand'] as List).cast<String>();
    final result = Process.runSync(command.first,
        [...command.skip(1), ...missing], stdoutEncoding: utf8);
    if (result.exitCode != 0) {
      throw StateError('DartForge could not read the suite annotations:\n'
          '${result.stderr}');
    }
    final libraries = jsonDecode(result.stdout as String) as Map;
    for (final path in missing) {
      _libraries[path] = libraries[path] as Map?;
    }
  }

  Stream<LoadSuite> loadDir(String dir, SuiteConfiguration suiteConfig) {
    final files = [
      for (final entry in Directory(dir).listSync(recursive: true))
        if (entry is File && _config.filename.matches(p.basename(entry.path)))
          entry.path
    ];
    _readLibraries(files);
    return StreamGroup.merge(
        Directory(dir).listSync(recursive: true).map((entry) {
      if (entry is! File || !_config.filename.matches(p.basename(entry.path))) {
        return const Stream.empty();
      }
      return loadFile(entry.path, suiteConfig);
    }));
  }

  Stream<LoadSuite> loadFile(
      String path, SuiteConfiguration suiteConfig) async* {
    _readLibraries([path]);
    try {
      final library = _libraries[path];
      if (library != null) {
        suiteConfig = suiteConfig.merge(SuiteConfiguration.fromMetadata(
            _libraryMetadata(path, File(path).readAsStringSync(), library)));
      }
    } on FormatException catch (error, stackTrace) {
      yield LoadSuite.forLoadException(LoadException(path, error), suiteConfig,
          stackTrace: stackTrace);
      return;
    }

    if (_config.excludeTags.evaluate(suiteConfig.metadata.tags.contains)) {
      return;
    }

    final platform = currentPlatform(Runtime.vm, _compiler);
    if (!suiteConfig.metadata.testOn.evaluate(platform)) return;

    final platformConfig = suiteConfig.forPlatform(platform);

    // Don't load a skipped suite.
    if (platformConfig.metadata.skip && !platformConfig.runSkipped) {
      yield LoadSuite.forSuite(RunnerSuite(
          const PluginEnvironment(),
          platformConfig,
          Group.root([LocalTest('(suite)', platformConfig.metadata, () {})],
              metadata: platformConfig.metadata),
          platform,
          path: path));
      return;
    }

    yield LoadSuite('loading $path', platformConfig, platform, () async {
      var retriesLeft = suiteConfig.metadata.retry;
      while (true) {
        try {
          final suite = await _platform.load(path, platform, platformConfig, {
            'platformVariables': _platformVariables,
            // Declaration traces would reach the reporters without frames.
            if (!_tracesHaveFrames) 'collectTraces': false,
          });
          if (suite != null) _suites.add(suite);
          return suite;
        } on Object catch (error, stackTrace) {
          if (retriesLeft > 0) {
            retriesLeft--;
            print('Retrying load of $path in 1s ($retriesLeft remaining)');
            await Future<void>.delayed(const Duration(seconds: 1));
            continue;
          }
          if (error is LoadException) {
            rethrow;
          }
          await Future<void>.error(LoadException(path, error), stackTrace);
          return null;
        }
      }
    }, path: path);
  }

  Future close() => _closeMemo.runOnce(() async {
        await Future.wait([
          _platform.close(),
          Future.wait(_suites.map((suite) => suite.close())),
        ]);
        _suites.clear();
      });
}

/// `VMPlatform` for the exe compiler, compiling with DartForge instead of
/// `dart compile exe`.
class _NativePlatform {
  final _closeMemo = AsyncMemoizer<void>();
  final _compilePool = Pool((_settings['compileJobs'] as int?) ?? 1);
  final _processes = <Process>{};
  var _nextBootstrap = 0;

  Directory get _workDirectory =>
      Directory(_settings['workDirectory'] as String);

  Future<RunnerSuite?> load(String path, SuitePlatform platform,
      SuiteConfiguration suiteConfig, Map<String, Object?> message) async {
    final serverSocket = await ServerSocket.bind('localhost', 0);
    Process process;
    try {
      final executable = await _compileToNative(path, suiteConfig.metadata);
      process = await Process.start(
          executable, [serverSocket.address.host, serverSocket.port.toString()]);
    } catch (error) {
      unawaited(serverSocket.close());
      rethrow;
    }
    _processes.add(process);
    process.stdout.listen(stdout.add);
    process.stderr.listen(stderr.add);
    final timeout = _settings['processTimeout'] as int?;
    final timer = timeout == null
        ? null
        : Timer(Duration(seconds: timeout), () {
            stderr.writeln('Test process timed out: $path');
            process.kill();
          });
    unawaited(process.exitCode.then((_) {
      timer?.cancel();
      _processes.remove(process);
    }));

    // A native executable that dies before connecting would otherwise leave
    // the load waiting for the socket.
    final connected = Completer<Socket>();
    final accepted = serverSocket.first.then(connected.complete,
        onError: connected.completeError);
    unawaited(accepted);
    unawaited(process.exitCode.then((code) {
      if (!connected.isCompleted) {
        connected.completeError(LoadException(
            path, 'The test executable exited with code $code before '
            'connecting to the test runner.'));
      }
    }));
    Socket socket;
    try {
      socket = await connected.future;
    } catch (_) {
      unawaited(serverSocket.close());
      rethrow;
    }
    final outerChannel = MultiChannel<Object?>(jsonSocketStreamChannel(socket));
    final cleanupCallbacks = <void Function()>[
      serverSocket.close,
      process.kill,
      outerChannel.sink.close,
    ];

    final outerQueue = StreamQueue(outerChannel.stream);
    final channelId = (await outerQueue.next) as int;
    final channel = outerChannel.virtualChannel(channelId).transformStream(
        StreamTransformer.fromHandlers(handleDone: (sink) async {
      for (final fn in cleanupCallbacks) {
        fn();
      }
      sink.close();
    }));

    final controller = deserializeSuite(path, platform, suiteConfig,
        const PluginEnvironment(), channel.cast(), message);
    return await controller.suite;
  }

  /// `VMPlatform._compileToNative` with `dartforge compile-native`.
  Future<String> _compileToNative(String path, Metadata suiteMetadata) =>
      _compilePool.withResource(() async {
        final directory = Directory(
            p.join(_workDirectory.path, '${_nextBootstrap++}'))
          ..createSync(recursive: true);
        final bootstrap = File(p.join(directory.path, 'bootstrap.dart'));
        bootstrap.writeAsStringSync(_bootstrapNativeTestContents(
            _relativeImport(directory.path, p.absolute(path)),
            suiteMetadata.languageVersionComment ??
                await _rootPackageLanguageVersionComment));
        final output = File(p.join(directory.path,
            Platform.isWindows ? 'bootstrap.exe' : 'bootstrap'));
        final processResult = await Process.run(
            _settings['compiler'] as String, [
          ...(_settings['compileCommand'] as List).cast<String>(),
          bootstrap.path,
          '-o',
          output.path,
          ...(_settings['compileArguments'] as List).cast<String>(),
        ]);
        if (processResult.exitCode != 0 || !(await output.exists())) {
          throw LoadException(path, '''
exitCode: ${processResult.exitCode}
stdout: ${processResult.stdout}
stderr: ${processResult.stderr}''');
        }
        return output.path;
      });

  Future close() => _closeMemo.runOnce(() async {
        for (final process in _processes.toList()) {
          process.kill();
        }
      });
}

/// The language version of the root package, as `rootPackageLanguageVersionComment`.
final Future<String> _rootPackageLanguageVersionComment = () async {
  final config =
      await loadPackageConfig(File(_settings['packageConfig'] as String));
  final rootPackage = config.packageOf(Uri.file(p.absolute('foo.dart')));
  if (rootPackage == null) return '';
  return '// @dart=${rootPackage.languageVersion}';
}();

/// Relative source imports avoid depending on native loader file-URI support.
String _relativeImport(String from, String to) =>
    p.url.joinAll(p.split(p.relative(to, from: from)));

/// test_core's `_bootstrapNativeTestContents`, with a relative import.
String _bootstrapNativeTestContents(
        String testUri, String languageVersionComment) =>
    '''
    $languageVersionComment
    import "dart:isolate";
    import "package:test_core/src/bootstrap/vm.dart";
    import ${jsonEncode(testUri).replaceAll(r'$', r'\$')} as test;
    void main(List<String> args) {
      internalBootstrapNativeTest(() => test.main, args);
    }
  ''';

/// Builds the metadata `parseMetadata` would produce from DartForge's reading
/// of the suite's first directive.
Metadata _libraryMetadata(String path, String contents, Map library) {
  final file = SourceFile.fromString(contents, url: p.toUri(path));
  for (final check in library['checks'] as List) {
    final text = check['text'] as String;
    final span = file.span(check['start'] as int, check['end'] as int);
    if (check['kind'] == 'tag') {
      if (!text.contains(anchoredHyphenatedIdentifier)) {
        throw SourceSpanFormatException(
            'Invalid tag name. Tags must be (optionally hyphenated) Dart '
            'identifiers.',
            span);
      }
      continue;
    }
    try {
      PlatformSelector.parse(text).validate(_platformVariables.toSet());
    } on SourceSpanFormatException catch (error) {
      final offsets = check['offsets'] as List?;
      final inner = error.span;
      if (offsets == null ||
          inner == null ||
          inner.end.offset >= offsets.length) {
        rethrow;
      }
      throw SourceSpanFormatException(
          error.message,
          file.span(offsets[inner.start.offset] as int,
              offsets[inner.end.offset] as int));
    }
  }
  final error = library['error'] as Map?;
  if (error != null) {
    final message = error['message'] as String;
    final span = file.span(error['start'] as int, error['end'] as int);
    // `parseMetadata` lets null checks and empty argument lists escape as
    // ordinary errors; they escape the loader here too.
    if (error['fatal'] == true) throw StateError(message);
    throw SourceSpanFormatException(message, span);
  }
  final m = library['metadata'] as Map;
  PlatformSelector selector(Object? text) =>
      PlatformSelector.parse(text as String);
  final skip = m['skip'];
  return Metadata(
    testOn: m['testOn'] == null ? null : selector(m['testOn']),
    timeout: _timeout(m['timeout']),
    skip: skip == null ? null : true,
    skipReason: skip is String ? skip : null,
    onPlatform: m['onPlatform'] == null
        ? null
        : {
            for (final entry in m['onPlatform'] as List)
              selector(entry[0]): Metadata.parse(
                timeout: _timeout(entry[1]['timeout']),
                skip: entry[1]['skip'],
              ),
          },
    tags: m['tags'] == null ? null : {...(m['tags'] as List).cast<String>()},
    retry: m['retry'] as int?,
    languageVersionComment: m['languageVersionComment'] as String?,
  );
}

Timeout? _timeout(Object? value) {
  if (value == null) return null;
  final map = value as Map;
  if (map['none'] == true) return Timeout.none;
  if (map.containsKey('factor')) return Timeout.factor(map['factor'] as num);
  final d = map['duration'] as Map;
  return Timeout(Duration(
      days: d['days'] as int,
      hours: d['hours'] as int,
      minutes: d['minutes'] as int,
      seconds: d['seconds'] as int,
      milliseconds: d['milliseconds'] as int,
      microseconds: d['microseconds'] as int));
}
