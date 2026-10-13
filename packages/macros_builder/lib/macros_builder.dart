/// Opt-in steps of the macro builder: discovery by resolved element and
/// execution of the declarations phase with factories registered by the
/// project. It does not produce an augmentation yet.
library;

import 'dart:convert';

import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:build/build.dart';
import 'package:macros/macros.dart' show Macro;
import 'package:macros/src/executor/assembly.dart';
import 'package:macros/src/executor/execute.dart';
import 'package:macros/src/executor/model.dart';
import 'package:macros/src/executor/result.dart';

import 'src/analyzer_assembly_resolver.dart';
import 'src/analyzer_model.dart';
import 'src/definition_queries.dart';
import 'src/generated_members.dart';
import 'src/identifier_resolver.dart';
import 'src/identifier_table.dart';

Builder macroDiscoveryBuilder(BuilderOptions _) => _MacroDiscoveryBuilder();

/// Runs the declarations phase in the build_runner's own isolate. The
/// project explicitly registers the factories available in the builder's
/// bootstrap; Dart offers no dynamic imports to load them at run time.
Builder macroDeclarationsBuilder(Map<String, Macro Function(ElementAnnotation)> factories) =>
    _MacroDeclarationsBuilder(factories);

final class _AnalyzerHost implements Host {
  final DefinitionQueries queries;
  _AnalyzerHost(IdentifierResolver resolver, [Map<int, Map<String, Object?>>? membersById])
      : queries = DefinitionQueries(resolver, membersById);

  @override
  Future<Object?> query(String kind, Map<String, Object?> args) => queries.query(kind, args);
}

final class _MacroDeclarationsBuilder implements Builder {
  final Map<String, Macro Function(ElementAnnotation)> factories;
  _MacroDeclarationsBuilder(this.factories);

  @override
  Map<String, List<String>> get buildExtensions => const {
        '.dart': [
          '.macro_declarations.json',
          '.macro_declarations.txt',
          '.macro_definitions_model.json',
          '.macro_definitions.json',
          '.macro_complete.txt',
          '.macro.dart',
        ]
      };

  @override
  Future<void> build(BuildStep step) async {
    final library = await step.resolver.libraryFor(step.inputId);
    final table = IdentifierTable();
    final results = <Map<String, Object?>>[];
    final executions = <(ClassElement, Map<String, Object?>, Macro)>[];
    for (final target in library.topLevelElements.whereType<ClassElement>()) {
      for (final annotation in target.metadata) {
        final constructor = annotation.element;
        if (constructor is! ConstructorElement) continue;
        final macroClass = constructor.enclosingElement3;
        if (macroClass is! ClassElement || !_isMacro(macroClass)) continue;
        final constructorName = constructor.name;
        final key = '${macroClass.librarySource.uri}#${macroClass.name}'
            '${constructorName.isEmpty ? '' : '.$constructorName'}';
        final factory = factories[key];
        if (factory == null) continue;
        final macro = factory(annotation);
        final execution = classModel(target, table);
        executions.add((target, execution, macro));
        final resolver = IdentifierResolver(target, execution, table);
        final model = Model(_AnalyzerHost(resolver))..receive(Map<String, Object?>.from(execution['model'] as Map));
        final declaration = model.declaration(Map<String, Object?>.from(execution['target'] as Map));
        final result = await executePhase(macro, Phase.declarations, declaration, Introspector(model));
        results.add({
          'target': target.name,
          'macro': key,
          'result': result.toJson(),
        });
      }
    }
    if (results.isEmpty) return;
    await step.writeAsString(
      step.inputId.changeExtension('.macro_declarations.json'),
      '${jsonEncode({'version': 1, 'results': results})}\n',
    );
    if (!step.inputId.path.startsWith('lib/')) {
      throw UnsupportedError('augmentation of a library outside lib/');
    }
    final uri = 'package:${step.inputId.package}/${step.inputId.path.substring(4)}';
    final partial = assembleAugmentation(
      [for (final item in results) item['result'] as Map<String, Object?>],
      AnalyzerAssemblyResolver(table),
      header: 'augment library',
      uri: uri,
    );
    await step.writeAsString(step.inputId.changeExtension('.macro_declarations.txt'), partial);
    final definitionModels = <Map<String, Object?>>[];
    final definitionResults = <Map<String, Object?>>[];
    final availableMembers = <int, Map<String, Object?>>{};
    for (final (classElement, execution, macro) in executions) {
      final generated = await generatedMembers(partial, classElement, execution, table);
      final target = Map<String, Object?>.from(execution['target'] as Map);
      final id = (target['ident'] as Map)['id'] as int;
      final model = Map<String, Object?>.from(execution['model'] as Map);
      final members = Map<String, Object?>.from(model['members'] as Map);
      final ofClass = Map<String, Object?>.from(members['$id'] as Map);
      ofClass['methods'] = [
        ...ofClass['methods'] as List,
        ...generated['methods'] as List,
      ];
      ofClass['constructors'] = [
        ...ofClass['constructors'] as List,
        ...generated['constructors'] as List,
      ];
      members['$id'] = ofClass;
      model['members'] = members;
      availableMembers[id] = ofClass;
      definitionModels.add({
        'target': classElement.name,
        'execution': {'target': target, 'model': model},
      });
      final host = _AnalyzerHost(IdentifierResolver(classElement, execution, table), availableMembers);
      final definitionModel = Model(host)..receive(model);
      final definitionTarget = definitionModel.declaration(target);
      final definitionResult =
          await executePhase(macro, Phase.definitions, definitionTarget, Introspector(definitionModel));
      definitionResults.add({
        'target': classElement.name,
        'result': definitionResult.toJson(),
      });
    }
    await step.writeAsString(
      step.inputId.changeExtension('.macro_definitions_model.json'),
      '${jsonEncode({'version': 1, 'applications': definitionModels})}\n',
    );
    await step.writeAsString(
      step.inputId.changeExtension('.macro_definitions.json'),
      '${jsonEncode({'version': 1, 'results': definitionResults})}\n',
    );
    for (final item in [...results, ...definitionResults]) {
      final result = item['result'] as Map<String, Object?>;
      final errors = (result['diagnostics'] as List).where((d) => (d as Map)['severity'] == 'error').toList();
      if (result['exception'] != null || errors.isNotEmpty) {
        throw StateError('the macro of ${item['target']} failed; '
            'augmentation not materialized: ${result['exception'] ?? errors}');
      }
    }
    final complete = assembleAugmentation(
      [
        for (final item in results) item['result'] as Map<String, Object?>,
        for (final item in definitionResults) item['result'] as Map<String, Object?>,
      ],
      AnalyzerAssemblyResolver(table),
      header: 'augment library',
      uri: uri,
    );
    await step.writeAsString(step.inputId.changeExtension('.macro_complete.txt'), complete);
    final libraryName = step.inputId.path.split('/').last;
    final materialized = complete.replaceFirst("augment library '$uri';", "augment library '$libraryName';");
    await step.writeAsString(step.inputId.changeExtension('.macro.dart'), materialized);
  }
}

final class _MacroDiscoveryBuilder implements Builder {
  @override
  Map<String, List<String>> get buildExtensions => const {
        '.dart': ['.macro_uses.json']
      };

  @override
  Future<void> build(BuildStep step) async {
    final library = await step.resolver.libraryFor(step.inputId);
    final applications = <Map<String, Object?>>[];
    for (final target in library.topLevelElements) {
      final name = target.name;
      if (name == null) continue;
      for (final annotation in target.metadata) {
        final element = annotation.element;
        if (element is! ConstructorElement) continue;
        final macroClass = element.enclosingElement3;
        if (macroClass is! ClassElement || !_isMacro(macroClass)) continue;
        final application = <String, Object?>{
          'target': name,
          'library': macroClass.librarySource.uri.toString(),
          'class': macroClass.name,
          'constructor': element.name,
        };
        if (target is ClassElement) {
          try {
            application['execution'] = classModel(target);
          } on UnsupportedError catch (e) {
            application['model_unavailable'] = e.message ?? 'declaration not covered yet';
          }
        }
        applications.add(application);
      }
    }
    if (applications.isEmpty) return;
    final output = step.inputId.changeExtension('.macro_uses.json');
    await step.writeAsString(output, '${jsonEncode({'version': 1, 'applications': applications})}\n');
  }
}

bool _isMacro(ClassElement macroClass) {
  // analyzer 7.3's `ClassElement` does not expose `isMacro`; the AST exposes
  // the token even when modifiers come before it (`abstract macro class`).
  final source = macroClass.source.contents.data;
  final unit = parseString(
    content: source,
    featureSet: FeatureSet.latestLanguageVersion(flags: ['macros']),
    throwIfDiagnostics: false,
  ).unit;
  return unit.declarations.whereType<ClassDeclaration>().any(
        // ignore: deprecated_member_use
        (d) => d.name.lexeme == macroClass.name && d.macroKeyword != null,
      );
}
