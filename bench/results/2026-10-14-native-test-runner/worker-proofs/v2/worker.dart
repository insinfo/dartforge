// The CLI compiles this adapter and the ordinary suite with DartForge AOT.
// All declarations, matchers, async guards and scaffolds belong to package:test.
import 'dart:async';
import 'dart:convert';
import 'dart:io';
import '../cases/semantics_test.dart' as suite_source;
import 'package:test_api/src/backend/compiler.dart';
import 'package:test_api/src/backend/declarer.dart';
import 'package:test_api/src/backend/invoker.dart';
import 'package:test_api/src/backend/operating_system.dart';
import 'package:test_api/src/backend/runtime.dart';
import 'package:test_api/src/backend/stack_trace_formatter.dart';
import 'package:test_api/src/backend/suite_platform.dart';
import 'package:test_core/src/runner/engine.dart';
import 'package:test_core/src/runner/environment.dart';
import 'package:test_core/src/runner/runner_suite.dart';
import 'package:test_core/src/runner/suite.dart';

void emit(Map<String, Object?> event) {
  stdout.writeln(jsonEncode(event));
}

Future<void> main(List<String> args) async {
  // These are worker arguments, never arguments supplied to the suite's main.
  final report = File(args[0]);
  final token = args[1];
  final filter = args[2];
  final plain = args[3] == 'plain';
  try {
    await StackTraceFormatter().asCurrent(() async {
      final declarer = Declarer();
      await declarer.declare<FutureOr<void>>(suite_source.main);
      var suite = RunnerSuite(
        const PluginEnvironment(),
        SuiteConfiguration.empty,
        declarer.build(),
        SuitePlatform(Runtime.vm,
            compiler: Compiler.exe,
            os: OperatingSystem.find(Platform.operatingSystem)),
        path: '/Volumes/ssd_dock/MyRustProjects/dartforge/target/test-runner-development/cases/semantics_test.dart',
      );
      if (filter.isNotEmpty) {
        final pattern = plain ? null : RegExp(filter);
        suite = suite.filter((test) =>
            plain ? test.name.contains(filter) : pattern!.hasMatch(test.name));
      }
      final count = suite.group.testCount;
      final engine = Engine.withSuites([suite]);
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
          emit({'type': 'message', 'name': live.test.name,
            'message': message.text});
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
        for (final live in engine.liveTests) {
          emit({'type': 'testDone', 'name': live.test.name,
            'result': live.state.result.toString()});
        }
        final summary = <String, Object?>{
          'schemaVersion': 1,
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
        exitCode = count == 0 ? 79 : success == true ? 0 : 1;
      } finally {
        await engine.close();
      }
    });
  } catch (error, stack) {
    emit({'type': 'loadError', 'error': error.toString(),
      'stackTrace': stack.toString()});
    exitCode = 1;
  }
}
