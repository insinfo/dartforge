// The CLI compiles this adapter and the ordinary suite with DartForge AOT.
// All declarations, matchers, async guards and scaffolds belong to package:test;
// the suite settings come from the DartForge test planner, which derives them
// from package:test's own configuration classes.
import 'dart:async';
import 'dart:convert';
import 'dart:io';
import '__DARTFORGE_SUITE_URI__' as suite_source;
import 'package:test_api/src/backend/declarer.dart';
import 'package:test_api/src/backend/invoker.dart';
import 'package:test_api/src/backend/metadata.dart';
import 'package:test_api/src/backend/state.dart';
import 'package:test_api/src/backend/stack_trace_formatter.dart';
import 'package:test_api/src/backend/suite_platform.dart';
import 'package:test_core/src/runner/engine.dart';
import 'package:test_core/src/runner/environment.dart';
import 'package:test_core/src/runner/runner_suite.dart';
import 'package:test_core/src/runner/suite.dart';

import 'selection.dart';

void emit(Map<String, Object?> event) {
  stdout.writeln(jsonEncode(event));
}

Set<String>? _set(Object? list) =>
    list == null || (list as List).isEmpty ? null : Set.from(list);

Future<void> main(List<String> args) async {
  // These are worker arguments, never arguments supplied to the suite's main.
  final report = File(args[0]);
  final token = args[1];
  final spec = jsonDecode(File(args[2]).readAsStringSync()) as Map;
  try {
    await StackTraceFormatter().asCurrent(() async {
      StackTraceFormatter.current!.configure(
          except: _set(spec['foldTraceExcept']),
          only: _set(spec['foldTraceOnly']));
      final declarer = Declarer(
        metadata: Metadata.deserialize(spec['metadata'] as Map),
        platformVariables: Set.from(spec['platformVariables'] as List),
        collectTraces: spec['collectTraces'] as bool,
        noRetry: spec['noRetry'] as bool,
        allowDuplicateTestNames: spec['allowDuplicateTestNames'] as bool,
      );
      await declarer.declare<FutureOr<void>>(suite_source.main);
      final config = SuiteConfiguration.empty.change(
        runSkipped: spec['runSkipped'] as bool,
        allowTestRandomization: spec['allowTestRandomization'] as bool,
        ignoreTimeouts: spec['ignoreTimeouts'] as bool,
      );
      var suite = RunnerSuite(
        const PluginEnvironment(),
        config,
        declarer.build(),
        SuitePlatform.deserialize(spec['platform'] as Object),
        path: spec['path'] as String,
      );
      final warning = unknownTagsWarning(
          suite, Set.from(spec['knownTags'] as List), spec['color'] as bool);
      if (warning != null) emit({'type': 'print', 'message': warning});
      suite = Selection.fromJson(spec['selection'] as Map).apply(suite);
      final count = suite.group.testCount;
      final engine = Engine(testRandomizeOrderingSeed: spec['seed'] as int?);
      engine.suiteSink.add(suite);
      engine.suiteSink.close();
      engine.onTestStarted.listen((live) {
        emit({'type': 'testStart', 'name': live.test.name});
        live.onError.listen((error) {
          emit({
            'type': 'error',
            'name': live.test.name,
            'error': error.error.toString(),
            'stackTrace': error.stackTrace.toString(),
          });
        });
        live.onMessage.listen((message) {
          emit({
            'type': 'message',
            'name': live.test.name,
            'message': message.text
          });
        });
        live.onComplete.then((_) {
          emit({
            'type': 'testDone',
            'name': live.test.name,
            'result': live.state.result.toString(),
            'skipped': live.state.result == Result.skipped,
            'hidden':
                !live.test.metadata.skip && live.state.result == Result.skipped,
          });
        });
      });
      bool? success;
      try {
        success = await runZoned(
          () => Invoker.guard<Future<bool?>>(() => engine.run()),
          zoneValues: {#test.declarer: declarer},
          zoneSpecification: ZoneSpecification(
            print: (self, parent, zone, line) {
              emit({'type': 'print', 'message': line});
            },
          ),
        );
        final summary = <String, Object?>{
          'schemaVersion': 2,
          'token': token,
          'total': count,
          'passed': engine.passed.length,
          'failed': engine.failed.length,
          'skipped': engine.skipped.length,
          'success': success == true,
        };
        // Closing remains part of the run: no success report if cleanup fails.
        await engine.close();
        await report.writeAsString(jsonEncode(summary));
        emit({'type': 'suiteDone', ...summary});
        exitCode = count == 0
            ? 79
            : success == true
                ? 0
                : 1;
      } finally {
        await engine.close();
      }
    });
  } catch (error, stack) {
    emit({
      'type': 'loadError',
      'error': error.toString(),
      'stackTrace': stack.toString()
    });
    exitCode = 1;
  }
}
