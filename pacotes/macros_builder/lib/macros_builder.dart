/// Primeira etapa do builder de macros: identifica aplicações pelo elemento
/// resolvido da anotação e grava um manifesto determinístico. Ainda não
/// executa a macro nem afirma produzir uma augmentation.
library;

import 'dart:convert';

import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:build/build.dart';

Builder macroDiscoveryBuilder(BuilderOptions _) => _MacroDiscoveryBuilder();

final class _MacroDiscoveryBuilder implements Builder {
  @override
  Map<String, List<String>> get buildExtensions => const {'.dart': ['.macro_uses.json']};

  @override
  Future<void> build(BuildStep step) async {
    final library = await step.resolver.libraryFor(step.inputId);
    final aplicacoes = <Map<String, String>>[];
    for (final alvo in library.topLevelElements) {
      for (final anotacao in alvo.metadata) {
        final elemento = anotacao.element;
        if (elemento is! ConstructorElement) continue;
        final classe = elemento.enclosingElement3;
        if (classe is! ClassElement || !_eMacro(classe)) continue;
        aplicacoes.add({
          'alvo': alvo.name,
          'biblioteca': classe.librarySource.uri.toString(),
          'classe': classe.name,
          'construtor': elemento.name,
        });
      }
    }
    if (aplicacoes.isEmpty) return;
    final saida = step.inputId.changeExtension('.macro_uses.json');
    await step.writeAsString(saida, '${jsonEncode({'versao': 1, 'aplicacoes': aplicacoes})}\n');
  }
}

bool _eMacro(ClassElement classe) {
  // `ClassElement` do analyzer 7.3 não expõe `isMacro`. O token inicial da
  // declaração resolvida preserva `macro` mesmo que haja comentários antes.
  final fonte = classe.source.contents.data;
  final unidade = parseString(
    content: fonte,
    featureSet: FeatureSet.latestLanguageVersion(flags: ['macros']),
    throwIfDiagnostics: false,
  ).unit;
  return unidade.declarations.whereType<ClassDeclaration>().any(
        (d) => d.name.lexeme == classe.name && d.beginToken.lexeme == 'macro',
      );
}
