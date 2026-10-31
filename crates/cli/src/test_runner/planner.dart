// The DartForge test planner, compiled with DartForge AOT against the
// package's own test_core.
//
// It reads the command line and the dart_test.yaml files with package:test's
// `Configuration` exactly as `dart test` does, finds the suites like its
// `Runner` and `Loader`, and turns each suite's configuration plus the library
// annotations read by DartForge into the settings of one native worker. The
// planner talks to DartForge through protocol lines on stdout and one reply
// line on stdin; everything else it prints is ordinary runner output.
import 'dart:convert';
import 'dart:io';

import 'package:boolean_selector/boolean_selector.dart';
import 'package:path/path.dart' as p;
import 'package:source_span/source_span.dart';
import 'package:test_api/scaffolding.dart' show Timeout;
import 'package:test_api/src/backend/compiler.dart';
import 'package:test_api/src/backend/group.dart';
import 'package:test_api/src/backend/invoker.dart';
import 'package:test_api/src/backend/metadata.dart';
import 'package:test_api/src/backend/operating_system.dart';
import 'package:test_api/src/backend/platform_selector.dart';
import 'package:test_api/src/backend/runtime.dart';
import 'package:test_api/src/backend/suite_platform.dart';
import 'package:test_api/src/backend/util/identifier_regex.dart';
import 'package:test_api/src/backend/util/pretty_print.dart' show toSentence;
import 'package:test_core/src/runner/configuration.dart';
import 'package:test_core/src/runner/configuration/reporters.dart'
    show defaultReporter;
import 'package:test_core/src/runner/environment.dart';
import 'package:test_core/src/runner/load_exception.dart';
import 'package:test_core/src/runner/runner_suite.dart';
import 'package:test_core/src/runner/suite.dart';
import 'package:test_core/src/util/io.dart'
    show currentOS, currentPlatform, inGoogle, warn, wordWrap;

import 'selection.dart';

const _protocol = '__DARTFORGE_PROTOCOL__';

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

void _send(Map<String, Object?> message) =>
    stdout.writeln('$_protocol${jsonEncode(message)}');

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

void _usageError(String message, int code) {
  stderr.writeln(wordWrap(message));
  stderr.writeln('\nRun "dartforge test --help" for usage.');
  exitCode = code;
}

Future<void> main(List<String> args) async {
  Configuration configuration;
  try {
    configuration = Configuration.parse(args);
  } on FormatException catch (error) {
    _usageError(error.message, 64);
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
    final names = undefinedPresets.map((preset) => '"$preset"').join(', ');
    _usageError(
        'Undefined preset${undefinedPresets.length == 1 ? '' : 's'} $names.',
        64);
    return;
  }

  if (!configuration.explicitPaths &&
      !Directory(configuration.testSelections.keys.single).existsSync()) {
    _usageError(
        'No test files were passed and the default "test/" '
        "directory doesn't exist.",
        65);
    return;
  }

  final refusal = _unsupported(configuration);
  if (refusal != null) {
    _send({'type': 'refusal', 'message': refusal});
    return;
  }

  _warnForUnsupportedPlatforms(configuration);

  // `Runner._loadSuites` and `Loader.loadDir`.
  final loads = <Map<String, Object?>>[];
  configuration.testSelections.forEach((testPath, _) {
    if (Directory(testPath).existsSync()) {
      for (final entry in Directory(testPath).listSync(recursive: true)) {
        if (entry is! File ||
            !configuration.filename.matches(p.basename(entry.path))) {
          continue;
        }
        loads.add({'path': entry.path, 'selection': testPath});
      }
    } else if (File(testPath).existsSync()) {
      loads.add({'path': testPath, 'selection': testPath});
    } else {
      loads.add({'path': testPath, 'selection': testPath, 'missing': true});
    }
  });
  _send({
    'type': 'plan',
    'loads': loads,
    'concurrency': configuration.concurrency,
    'reporter': configuration.reporter,
    'defaultReporter': defaultReporter,
    'noTestsMessage': _noTestsMessage(configuration),
    // The reporters' `printPath` rule.
    'printPath': configuration.testSelections.length > 1 ||
        Directory(configuration.testSelections.keys.single).existsSync(),
    'color': configuration.color,
  });

  final reply = jsonDecode(stdin.readLineSync(encoding: utf8)!) as Map;
  final libraries = reply['libraries'] as Map;
  final suites = <Map<String, Object?>>[];
  for (final load in loads) {
    suites.add(_resolve(configuration, load, libraries[load['path']] as Map?));
  }
  _send({'type': 'suites', 'suites': suites});
}

/// The `NoTestsFoundException` message of `Runner.run`.
String _noTestsMessage(Configuration config) {
  if (config.globalPatterns.isNotEmpty) {
    final patterns = toSentence(config.globalPatterns.map((pattern) =>
        pattern is RegExp
            ? 'regular expression "${pattern.pattern}"'
            : '"$pattern"'));
    return 'No tests match $patterns.';
  } else if (config.includeTags != BooleanSelector.all ||
      config.excludeTags != BooleanSelector.none) {
    return 'No tests match the requested tag selectors:\n'
        '  include: "${config.includeTags}"\n'
        '  exclude: "${config.excludeTags}"';
  }
  return 'No tests were found.';
}

/// Options the native runner cannot honor are refused instead of ignored.
String? _unsupported(Configuration configuration) {
  if (configuration.help || configuration.version) {
    return '--version is not supported; dartforge test does not report the package:test version';
  }
  if (configuration.coverage != null) {
    return 'coverage collection is not supported by the native runner';
  }
  if (configuration.pauseAfterLoad || configuration.debug) {
    return 'debugging (--pause-after-load, --debug) is not supported by the '
        'native runner';
  }
  if (configuration.stopOnFirstFailure) {
    return '--fail-fast is not supported by the native runner';
  }
  if (configuration.fileReporters.isNotEmpty) {
    return 'file reporters are not supported by the native runner';
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
    // ordinary errors; they still fail the load.
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

Map<String, Object?> _selection(Configuration config, String selectionPath) {
  return {
    'globalPatterns': config.globalPatterns.map(encodePattern).toList(),
    'includeTags': encodeSelector(config.includeTags),
    'excludeTags': encodeSelector(config.excludeTags),
    'selections': [
      for (final selection in config.testSelections[selectionPath]!)
        selection.testPatterns.map(encodePattern).toList()
    ],
    'totalShards': config.totalShards,
    'shardIndex': config.shardIndex,
  };
}

/// `Loader.loadFile` for one suite on the vm runtime.
Map<String, Object?> _resolve(Configuration config, Map load, Map? library) {
  final path = load['path'] as String;
  if (load['missing'] == true) {
    return {
      'path': path,
      'action': 'error',
      'message': LoadException(path, 'Does not exist.').toString(),
    };
  }
  var suiteConfig = config.suiteDefaults
      .selectTests(config.testSelections[load['selection']]!);
  try {
    if (library != null) {
      suiteConfig = suiteConfig.merge(SuiteConfiguration.fromMetadata(
          _libraryMetadata(path, File(path).readAsStringSync(), library)));
    }
  } catch (error) {
    // Format errors are load errors; parseMetadata's other failures escape
    // the official loader and still fail the run.
    return {
      'path': path,
      'action': 'error',
      'message': LoadException(path, error).toString(),
    };
  }

  if (config.excludeTags.evaluate(suiteConfig.metadata.tags.contains)) {
    return {'path': path, 'action': 'none'};
  }

  final platform = currentPlatform(Runtime.vm, _compiler);
  if (!suiteConfig.metadata.testOn.evaluate(platform)) {
    return {'path': path, 'action': 'none'};
  }
  final platformConfig = suiteConfig.forPlatform(platform);
  final selection = _selection(config, load['selection'] as String);
  final color = config.color;

  if (platformConfig.metadata.skip && !platformConfig.runSkipped) {
    // The skipped suite is never loaded: it is one skipped `(suite)` test,
    // still subject to the runner's filters, sharding and tag warning.
    final metadata = platformConfig.metadata;
    var suite = RunnerSuite(
        const PluginEnvironment(),
        platformConfig,
        Group.root([LocalTest('(suite)', metadata, () {})], metadata: metadata),
        platform,
        path: path);
    final warning = unknownTagsWarning(suite, config.knownTags, color);
    suite = Selection.fromJson(selection).apply(suite);
    return {
      'path': path,
      'action': suite.group.testCount == 0 ? 'none' : 'skip',
      'reason': metadata.skipReason,
      'warning': warning,
    };
  }

  return {
    'path': path,
    'action': 'run',
    'worker': {
      'path': path,
      'platform': platform.serialize(),
      'metadata': platformConfig.metadata.serialize(),
      'platformVariables': _platformVariables,
      'collectTraces':
          config.reporter == 'json' || config.fileReporters.containsKey('json'),
      'noRetry': config.noRetry,
      'foldTraceExcept': config.foldTraceExcept.toList(),
      'foldTraceOnly': config.foldTraceOnly.toList(),
      'allowDuplicateTestNames': platformConfig.allowDuplicateTestNames,
      'allowTestRandomization': platformConfig.allowTestRandomization,
      'ignoreTimeouts': platformConfig.ignoreTimeouts,
      'runSkipped': platformConfig.runSkipped,
      'seed': config.testRandomizeOrderingSeed,
      'selection': selection,
      'knownTags': config.knownTags.toList(),
      'color': color,
    },
  };
}
