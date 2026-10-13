// Type oracle: the official `package:analyzer` records, per expression, the
// static type and the resolved element. It is a test oracle for the
// inference of `crates/types`, never a product (docs/FRONTEND-NEW-SALI.md,
// "Oráculo").
//
// Usage (the analyzer comes from the analyzed project's own package_config):
//
//   dart --packages=<project>/.dart_tool/package_config.json \
//       tools/type_oracle/oracle.dart <project root> <files.txt> <output.tsv>
//
// `files.txt` is the list of units our dump covered
// (`cargo run -p dartforge-types --example dump_types`), one path per line;
// the output has the same format as our dump:
//
//   path \t offset \t length \t node \t type \t element
import 'dart:io';

import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/visitor.dart';
import 'package:analyzer/dart/element/element.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/analysis/analysis_context_collection.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/analysis/byte_store.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/analysis/driver_based_analysis_context.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/analysis/file_byte_store.dart';
import 'package:path/path.dart' as p;

Future<void> main(List<String> args) async {
  if (args.length != 3) {
    stderr.writeln('usage: oracle.dart <root> <files.txt> <output.tsv>');
    exit(64);
  }
  final root = _abs(args[0]);
  final files = File(args[1])
      .readAsLinesSync()
      .map((l) => l.trim())
      .where((l) => l.isNotEmpty)
      .toList();
  final output = File(args[2]).openWrite();
  // Summaries on disk (the analyzer cache next to the output), with only
  // 32 MB in memory: the peak stays at what one library needs, not the project.
  final cache = p.join(p.dirname(_abs(args[2])), 'analyzer-cache');
  Directory(cache).createSync(recursive: true);
  final collection = AnalysisContextCollectionImpl(
    includedPaths: [root],
    // Compiled with `dart compile exe`, the executable does not know where
    // the SDK is: DART_SDK, or the machine's 3.6.2.
    sdkPath: _abs(Platform.environment['DART_SDK'] ?? 'C:/tools/dartsdk-3.6.2'),
    byteStore: MemoryCachingByteStore(EvictingFileByteStore(cache, 1 << 30), 32 << 20),
  );
  final ctx = collection.contextFor(root);
  var ok = 0, failures = 0;
  for (final file in files) {
    // Generated files: first through the source path (the analyzer's
    // `package:build` workspace finds them there), then through their own path.
    var r = await ctx.currentSession.getResolvedUnit(_withoutGenerated(file));
    if (r is! ResolvedUnitResult) {
      r = await ctx.currentSession.getResolvedUnit(_abs(file));
    }
    if (r is! ResolvedUnitResult) {
      output.writeln('#failure\t$file\t${r.runtimeType}');
      failures++;
      continue;
    }
    // Keyed by the path our dump used (the generated one, if that is the
    // case). The lines come out sorted by (offset, length), in the order of
    // the list (our dump's): the comparator merges them as a stream.
    final v = _Visitor(_norm(file));
    r.unit.accept(v);
    v.lines.sort((a, b) => a.$1 != b.$1 ? a.$1.compareTo(b.$1) : a.$2.compareTo(b.$2));
    for (final l in v.lines) {
      output.writeln(l.$3);
    }
    ok++;
    // Memory: the analyzer keeps the element model of everything it has
    // resolved; clearing it from time to time keeps the peak low (what was
    // already linked comes back from the cached summaries).
    if (ok % 150 == 0) {
      (ctx as DriverBasedAnalysisContext).driver.clearLibraryContext();
    }
  }
  await output.close();
  stderr.writeln('oracle: $ok units resolved, $failures failures');
}

/// `.dart_tool/build/generated/<package>/<rest>` → `<package root>/<rest>`:
/// the analyzer's `package:build` workspace finds the generated file through
/// the source path.
String _withoutGenerated(String q) {
  final n = q.replaceAll('\\', '/');
  final i = n.indexOf('/.dart_tool/build/generated/');
  if (i < 0) return _abs(q);
  final root = n.substring(0, i);
  final rest = n.substring(i + '/.dart_tool/build/generated/'.length);
  final slash = rest.indexOf('/');
  return _abs('$root/${rest.substring(slash + 1)}');
}

String _norm(String x) => x.replaceAll('\\', '/');

String _abs(String x) => p.normalize(p.absolute(x));

class _Visitor extends GeneralizingAstVisitor<void> {
  final String file;
  final lines = <(int, int, String)>[];
  _Visitor(this.file);

  // Directive URIs are not expressions for us.
  @override
  void visitDirective(Directive node) {}

  @override
  void visitExpression(Expression node) {
    final type = node.staticType?.getDisplayString() ?? '-';
    final el = _element(node);
    lines.add((node.offset, node.length,
        '$file\t${node.offset}\t${node.length}\t${node.runtimeType.toString().replaceAll('Impl', '')}\t$type\t$el'));
    super.visitExpression(node);
  }

  String _element(Expression node) {
    Element? e;
    if (node is SimpleIdentifier) {
      e = node.staticElement;
    } else if (node is PrefixedIdentifier) {
      e = node.staticElement;
    } else if (node is MethodInvocation) {
      e = node.methodName.staticElement;
    } else if (node is PropertyAccess) {
      e = node.propertyName.staticElement;
    } else if (node is InstanceCreationExpression) {
      e = node.constructorName.staticElement;
    } else if (node is AssignmentExpression) {
      e = node.writeElement;
    } else if (node is BinaryExpression) {
      e = node.staticElement;
    } else if (node is IndexExpression) {
      e = node.staticElement;
    }
    if (e == null) return '-';
    final owner = e.enclosingElement3;
    final ownerName = owner is CompilationUnitElement ? '' : '${owner?.name}.';
    return '${e.kind.name}:$ownerName${e.name}';
  }
}
