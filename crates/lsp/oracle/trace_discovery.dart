// Trace OwnedFiles through initial analysis and discoverAvailableFiles.
// Run with --packages selecting analyzer from the Dart 3.6.2 source tree:
// dart --packages=<config> trace_discovery.dart <project> <priority-file> <sdk> <output.json>
// This is an auxiliary source-level trace, not a replacement for the LSP oracle.

import 'dart:convert';
import 'dart:io';
import 'dart:isolate';

import 'package:analyzer/src/dart/analysis/analysis_context_collection.dart';

String normalized(String value) =>
    Uri.file(File(value).absolute.path).normalizePath().toFilePath();

Future<void> main(List<String> args) async {
  if (args.length != 4) {
    stderr.writeln(
      'Usage: trace_discovery.dart <project> <priority-file> <sdk> <output.json>',
    );
    exitCode = 64;
    return;
  }
  final root = normalized(args[0]);
  final anchor = normalized(args[1]);
  final sdk = normalized(args[2]);
  final packageConfig = await Isolate.packageConfig;
  Uri? analyzerRoot;
  if (packageConfig != null) {
    final config = jsonDecode(await File.fromUri(packageConfig).readAsString());
    for (final package in config['packages'] as List) {
      if (package['name'] == 'analyzer') {
        analyzerRoot = packageConfig.resolve(package['rootUri'] as String);
      }
    }
  }
  final collection = AnalysisContextCollectionImpl(
    includedPaths: [root],
    sdkPath: sdk,
  );
  try {
    for (final context in collection.contexts) {
      for (final file in context.contextRoot.analyzedFiles()) {
        if (file.endsWith('.dart')) context.driver.addFile(file);
      }
      context.driver.priorityFiles = [anchor];
    }
    for (final context in collection.contexts) {
      await context.applyPendingFileChanges();
    }
    await collection.contextFor(anchor).driver.getResolvedUnit(anchor);
    await collection.scheduler.waitForIdle();
    List<Map<String, Object?>> known() => [
          for (final entry in collection.ownedFiles.knownFiles.entries)
            {
              'uri': entry.key.toString(),
              'path': entry.value.fsState.getExistingFromUri(entry.key)?.path,
              'context': entry.value.analysisContext?.contextRoot.root.path,
            },
        ];
    final before = known();
    await Future.wait(
      collection.contexts.map((c) => c.driver.discoverAvailableFiles()),
    );
    final after = known();
    await File(args[3]).writeAsString(jsonEncode({
      'root': root,
      'anchor': anchor,
      'sdk': sdk,
      'hostDartVersion': Platform.version,
      'analyzerPackageRoot': analyzerRoot?.toString(),
      'contexts': [
        for (final c in collection.contexts) c.contextRoot.root.path
      ],
      'added': collection.ownedFiles.addedFiles.keys.map((u) => '$u').toList(),
      'beforeDiscovery': before,
      'afterDiscovery': after,
    }));
    stdout.writeln('added=${collection.ownedFiles.addedFiles.length}, '
        'knownBefore=${before.length}, knownAfter=${after.length}');
  } finally {
    await collection.dispose();
  }
}
