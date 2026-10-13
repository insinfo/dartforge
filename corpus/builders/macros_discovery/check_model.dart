import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:dartforge_macros_builder/src/analyzer_model.dart';
import 'package:dartforge_macros_builder/src/definition_queries.dart';
import 'package:dartforge_macros_builder/src/identifier_resolver.dart';
import 'package:dartforge_macros_builder/src/identifier_table.dart';
import 'package:json/json.dart';
import 'package:macros/src/executor/execute.dart';
import 'package:macros/src/executor/model.dart';
import 'package:macros/src/executor/result.dart';

final class AnalyzerHost implements Host {
  final IdentifierResolver resolver;
  AnalyzerHost(this.resolver);

  @override
  Future<Object?> query(String kind, Map<String, Object?> args) {
    if (kind != 'resolveIdentifier') throw UnsupportedError('query $kind not implemented yet');
    return resolver.resolve(args['uri'] as String, args['name'] as String);
  }
}

Object? replaceUri(Object? value) {
  if (value == 'package:caso_json/modelos.dart') {
    return 'package:corpus_macros_discovery/modelos.dart';
  }
  if (value is List) return [for (final item in value) replaceUri(item)];
  if (value is Map) {
    return {for (final entry in value.entries) entry.key: replaceUri(entry.value)};
  }
  return value;
}

Object? withoutIds(Object? value) {
  if (value is List) return [for (final item in value) withoutIds(item)];
  if (value is Map) {
    return {
      for (final entry in value.entries)
        if (entry.key != 'id' && entry.key != 'key' && entry.key != 'i') entry.key: withoutIds(entry.value),
    };
  }
  return value;
}

Object? withoutTargetId(Object? result) {
  final map = Map<String, Object?>.from(result as Map);
  map['types'] = [
    for (final pair in map['types'] as List) [0, (pair as List)[1]],
  ];
  return withoutIds(map);
}

bool equal(Object? a, Object? b) {
  if (a is List && b is List) {
    return a.length == b.length && List.generate(a.length, (i) => i).every((i) => equal(a[i], b[i]));
  }
  if (a is Map && b is Map) {
    return a.length == b.length && a.keys.every((key) => b.containsKey(key) && equal(a[key], b[key]));
  }
  return a == b;
}

Future<void> main() async {
  final manifest = jsonDecode(File('lib/modelos.macro_uses.json').readAsStringSync()) as Map;
  final applications = manifest['applications'] as List;
  final endereco = applications.singleWhere((a) => a['target'] == 'Endereco') as Map;
  final session = File('../../macros/410_json_codable/esperado/session.dfexec').readAsLinesSync();
  final request = session
      .map((line) => jsonDecode(line.substring(2)) as Map)
      .singleWhere((m) => m['t'] == 'macro.execute' && m['id'] == 4);
  final expected = replaceUri({'target': request['target'], 'model': request['model']});
  if (!equal(endereco['execution'], expected)) {
    throw StateError('the Endereco model differs from the CFE macro.execute request');
  }
  final path =
      File('${Directory.current.path}${Platform.pathSeparator}lib${Platform.pathSeparator}modelos.dart').absolute.path;
  final contexts = AnalysisContextCollection(includedPaths: [Directory.current.absolute.path]);
  try {
    final library = await contexts
        .contextFor(path)
        .currentSession
        .getLibraryByUri('package:corpus_macros_discovery/modelos.dart');
    if (library is! LibraryElementResult) throw StateError('fixture library not resolved');
    final classElement =
        library.element.topLevelElements.whereType<ClassElement>().singleWhere((c) => c.name == 'Endereco');
    final resolver = IdentifierResolver(classElement, Map<String, Object?>.from(endereco['execution'] as Map));
    final queries = <int, Map<String, Object?>>{};
    var resolved = 0;
    for (final line in session) {
      final m = jsonDecode(line.substring(2)) as Map;
      if (m['t'] == 'macro.query' && m['execution'] == 4) {
        queries[m['id'] as int] = Map<String, Object?>.from(m['args'] as Map);
      } else if (m['t'] == 'macro.response' && queries.containsKey(m['id'])) {
        final args = queries.remove(m['id'])!;
        final actual = await resolver.resolve(args['uri'] as String, args['name'] as String);
        if (!equal(actual, m['value'])) throw StateError('resolveIdentifier diverged from the CFE: $args');
        resolved++;
      }
    }
    if (resolved != 3) throw StateError('$resolved queries resolved; expected 3');
    final execution = Map<String, Object?>.from(endereco['execution'] as Map);
    final model = Model(AnalyzerHost(resolver))..receive(Map<String, Object?>.from(execution['model'] as Map));
    final target = model.declaration(Map<String, Object?>.from(execution['target'] as Map));
    final result =
        (await executePhase(const JsonCodable(), Phase.declarations, target, Introspector(model))).toJson();
    final expectedResult = session
        .map((line) => jsonDecode(line.substring(2)) as Map)
        .singleWhere((m) => m['t'] == 'macro.result' && m['id'] == 4)['result'];
    if (!equal(result, expectedResult)) throw StateError('declarations phase diverged from the CFE');
    final declarations = jsonDecode(File('lib/modelos.macro_declarations.json').readAsStringSync()) as Map;
    final generated = declarations['results'] as List;
    if (declarations['version'] != 1 || generated.length != 4) {
      throw StateError('the builder did not run the four registered applications');
    }
    final generatedByTarget = {
      for (final item in generated) (item as Map)['target']: item,
    };
    final targetIds = <int>{};
    final coreIds = <String, int>{};
    void checkIds(Object? value) {
      if (value is List) {
        for (final item in value) checkIds(item);
      } else if (value is Map) {
        if (value['i'] is int && value['n'] is String) {
          final name = value['n'] as String;
          if (const {'Map', 'String', 'Object'}.contains(name)) {
            final previous = coreIds.putIfAbsent(name, () => value['i'] as int);
            if (previous != value['i']) throw StateError('dart:core id changed between applications: $name');
          }
        }
        for (final item in value.values) checkIds(item);
      }
    }

    for (final target in ['Endereco', 'Usuario', 'SoSaida', 'SoEntrada']) {
      final item = generatedByTarget[target];
      if (item == null) throw StateError('result missing for $target');
      final macroResult = item['result'] as Map;
      if (macroResult['exception'] != null ||
          (macroResult['diagnostics'] as List).isNotEmpty ||
          (macroResult['types'] as List).isEmpty) {
        throw StateError('the macro of $target did not generate valid declarations');
      }
      final types = macroResult['types'] as List;
      final id = (types.single as List).first as int;
      if (!targetIds.add(id)) throw StateError('two classes share the identifier $id');
      checkIds(macroResult);
    }
    if (coreIds.length != 3 ||
        (generatedByTarget['Endereco'] as Map)['result']['types'][0][0] != 1 ||
        (generatedByTarget['Usuario'] as Map)['result']['types'][0][0] != 8) {
      throw StateError('the id table diverged from the first two CFE applications');
    }
    final partial = File('lib/modelos.macro_declarations.txt').readAsStringSync().replaceAll('\r\n', '\n');
    final cfeAugmentation = File('../../macros/410_json_codable/esperado/modelos.augmentation.dart')
        .readAsStringSync()
        .replaceAll('\r\n', '\n');
    String declarationsOf(String text, String name) {
      final start = text.indexOf('augment class $name {\n');
      if (start < 0) throw StateError('class $name missing from the augmentation');
      final nextDefinition = text.indexOf('\n  augment ', start);
      final classEnd = text.indexOf('\n}', start);
      if (classEnd < 0) throw StateError('class $name is not closed');
      final end = nextDefinition >= 0 && nextDefinition < classEnd ? nextDefinition : classEnd;
      return text.substring(start, end);
    }

    for (final target in ['Endereco', 'Usuario', 'SoSaida', 'SoEntrada']) {
      if (declarationsOf(partial, target) != declarationsOf(cfeAugmentation, target)) {
        throw StateError('the assembled declarations of $target differ from the CFE');
      }
    }
    final complete = File('lib/modelos.macro_complete.txt').readAsStringSync().replaceAll('\r\n', '\n');
    String completeBlock(String text, String name) {
      final start = text.indexOf('augment class $name {\n');
      if (start < 0) throw StateError('class $name missing from the assembly');
      final end = text.indexOf('\n}', start);
      if (end < 0) throw StateError('class $name is not closed in the assembly');
      return text.substring(start, end + 2);
    }

    for (final target in ['Endereco', 'SoSaida', 'SoEntrada']) {
      if (completeBlock(complete, target) != completeBlock(cfeAugmentation, target)) {
        throw StateError('the complete assembly of $target differs from the CFE');
      }
    }
    if (complete.replaceAll('package:corpus_macros_discovery/modelos.dart', 'package:caso_json/modelos.dart') !=
        cfeAugmentation) {
      throw StateError('the complete augmentation differs byte for byte from the CFE');
    }
    final materialized = File('lib/modelos.macro.dart').readAsStringSync().replaceAll('\r\n', '\n');
    if (materialized !=
        complete.replaceFirst("augment library 'package:corpus_macros_discovery/modelos.dart';",
            "augment library 'modelos.dart';")) {
      throw StateError('the .macro.dart file diverged from the assembly');
    }
    final parsed = parseString(
      content: materialized,
      featureSet: FeatureSet.latestLanguageVersion(flags: ['macros']),
      throwIfDiagnostics: false,
    );
    if (parsed.errors.isNotEmpty) {
      throw StateError('invalid .macro.dart file: ${parsed.errors}');
    }
    final definitionModels = jsonDecode(File('lib/modelos.macro_definitions_model.json').readAsStringSync()) as Map;
    final definitionRequests = definitionModels['applications'] as List;
    if (definitionModels['version'] != 1 || definitionRequests.length != 4) {
      throw StateError('incomplete definition models');
    }
    final cfeDefinitionRequest = session
        .map((line) => jsonDecode(line.substring(2)) as Map)
        .singleWhere((m) => m['t'] == 'macro.execute' && m['id'] == 8);
    final cfeMembers = replaceUri((cfeDefinitionRequest['model'] as Map)['members']['1']);
    final generatedEndereco = definitionRequests.singleWhere((a) => a['target'] == 'Endereco') as Map;
    final builderMembers = ((generatedEndereco['execution'] as Map)['model'] as Map)['members']['1'];
    if (!equal(withoutIds(builderMembers), withoutIds(cfeMembers))) {
      throw StateError('the post-declarations model of Endereco differs from the CFE');
    }
    final definitionTable = IdentifierTable();
    final definitionInitial = classModel(classElement, definitionTable);
    final definitionResolver = IdentifierResolver(classElement, definitionInitial, definitionTable);
    for (final name in ['Map', 'String', 'Object']) {
      await definitionResolver.resolve('dart:core', name);
    }
    final definitionQueries = DefinitionQueries(definitionResolver);
    final pendingDefinitions = <int, Map<String, Object?>>{};
    var definitionAnswers = 0;
    for (final line in session) {
      final m = jsonDecode(line.substring(2)) as Map;
      if (m['t'] == 'macro.query' && m['execution'] == 8) {
        pendingDefinitions[m['id'] as int] = Map<String, Object?>.from(m);
      } else if (m['t'] == 'macro.response' && pendingDefinitions.containsKey(m['id'])) {
        final request = pendingDefinitions.remove(m['id'])!;
        final kind = request['kind'] as String;
        final value = await definitionQueries.query(kind, Map<String, Object?>.from(request['args'] as Map));
        if (!equal(withoutIds(value), withoutIds(m['value']))) {
          throw StateError('definition query $kind diverged from the CFE');
        }
        definitionAnswers++;
      }
    }
    if (definitionAnswers != 14) {
      throw StateError('$definitionAnswers definition queries; expected 14');
    }
    final definitions = jsonDecode(File('lib/modelos.macro_definitions.json').readAsStringSync()) as Map;
    final definitionResults = definitions['results'] as List;
    if (definitions['version'] != 1 || definitionResults.length != 4) {
      throw StateError('the definitions phase did not run the four applications');
    }
    for (final item in definitionResults) {
      final result = (item as Map)['result'] as Map;
      if (result['exception'] != null ||
          (result['diagnostics'] as List).isNotEmpty ||
          (result['types'] as List).isEmpty) {
        throw StateError('definitions phase of ${item['target']} incomplete');
      }
    }
    for (final entry in <(String, int)>[
      ('Endereco', 8),
      ('Usuario', 9),
      ('SoSaida', 10),
      ('SoEntrada', 11),
    ]) {
      final (target, cfeId) = entry;
      final definitionResult = definitionResults.singleWhere((r) => r['target'] == target) as Map;
      final cfeDefinitionResult = session
          .map((line) => jsonDecode(line.substring(2)) as Map)
          .singleWhere((m) => m['t'] == 'macro.result' && m['id'] == cfeId);
      if (!equal(withoutTargetId(definitionResult['result']), withoutTargetId(cfeDefinitionResult['result']))) {
        throw StateError('definitions phase of $target diverged from the CFE');
      }
    }
    final byBuilder = generated.singleWhere((r) => r['target'] == 'Endereco') as Map;
    if (byBuilder['macro'] != 'package:json/json.dart#JsonCodable' || !equal(byBuilder['result'], expectedResult)) {
      throw StateError('the build_runner result diverged from the CFE');
    }
    const reexportedUri = 'package:corpus_macros_discovery/reexportado.dart';
    final reexportedLibrary = await classElement.library.session.getLibraryByUri(reexportedUri);
    if (reexportedLibrary is! LibraryElementResult ||
        reexportedLibrary.element.exportNamespace.get('Random') == null) {
      throw StateError('the re-export fixture does not expose Random');
    }
    var rejectedReexport = false;
    try {
      await resolver.resolve(reexportedUri, 'Random');
    } on StateError {
      rejectedReexport = true;
    }
    if (!rejectedReexport) throw StateError('re-export accepted as a local declaration');
    print('model, $resolved queries and declarations phase equal to the CFE; build_runner ran and assembled '
        '4 applications; whole augmentation byte for byte and 4 definition results equal to the CFE; '
        '$definitionAnswers queries of Endereco; re-export rejected');
  } finally {
    await contexts.dispose();
  }
}
