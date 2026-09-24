/// Etapas opt-in do builder de macros: descoberta por elemento resolvido e
/// execução da fase de declarações com fábricas registradas pelo projeto.
/// Ainda não produz uma augmentation.
library;

import 'dart:convert';

import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:build/build.dart';
import 'package:macros/macros.dart' show Macro;
import 'package:macros/src/executor/executar.dart';
import 'package:macros/src/executor/modelo.dart';
import 'package:macros/src/executor/resultado.dart';

import 'src/modelo_analyzer.dart';
import 'src/resolver_identificadores.dart';
import 'src/tabela_identificadores.dart';

Builder macroDiscoveryBuilder(BuilderOptions _) => _MacroDiscoveryBuilder();

/// Executa a fase de declarações no mesmo isolate do build_runner. O projeto
/// registra explicitamente as fábricas disponíveis no bootstrap do builder;
/// Dart não oferece imports dinâmicos para carregá-las em tempo de execução.
Builder macroDeclarationsBuilder(
        Map<String, Macro Function(ElementAnnotation)> fabricas) =>
    _MacroDeclarationsBuilder(fabricas);

final class _HospedeiroAnalyzer implements Hospedeiro {
  final ResolvedorIdentificadores resolvedor;
  _HospedeiroAnalyzer(this.resolvedor);

  @override
  Future<Object?> consultar(String tipo, Map<String, Object?> args) {
    if (tipo != 'resolverIdentificador') {
      throw UnsupportedError(
          'consulta $tipo ainda não implementada pelo builder');
    }
    return resolvedor.resolver(args['uri'] as String, args['nome'] as String);
  }
}

final class _MacroDeclarationsBuilder implements Builder {
  final Map<String, Macro Function(ElementAnnotation)> fabricas;
  _MacroDeclarationsBuilder(this.fabricas);

  @override
  Map<String, List<String>> get buildExtensions => const {
        '.dart': ['.macro_declarations.json']
      };

  @override
  Future<void> build(BuildStep step) async {
    final library = await step.resolver.libraryFor(step.inputId);
    final tabela = TabelaIdentificadores();
    final resultados = <Map<String, Object?>>[];
    for (final alvo in library.topLevelElements.whereType<ClassElement>()) {
      for (final anotacao in alvo.metadata) {
        final construtor = anotacao.element;
        if (construtor is! ConstructorElement) continue;
        final classe = construtor.enclosingElement3;
        if (classe is! ClassElement || !_eMacro(classe)) continue;
        final nomeConstrutor = construtor.name;
        final chave = '${classe.librarySource.uri}#${classe.name}'
            '${nomeConstrutor.isEmpty ? '' : '.$nomeConstrutor'}';
        final fabrica = fabricas[chave];
        if (fabrica == null) continue;
        final execucao = modeloDaClasse(alvo, tabela);
        final resolvedor = ResolvedorIdentificadores(alvo, execucao, tabela);
        final modelo = Modelo(_HospedeiroAnalyzer(resolvedor))
          ..receber(Map<String, Object?>.from(execucao['modelo'] as Map));
        final declaracao = modelo
            .declaracao(Map<String, Object?>.from(execucao['alvo'] as Map));
        final resultado = await executarFase(fabrica(anotacao),
            Fase.declaracoes, declaracao, Introspector(modelo));
        resultados.add({
          'alvo': alvo.name,
          'macro': chave,
          'resultado': resultado.paraJson(),
        });
      }
    }
    if (resultados.isEmpty) return;
    await step.writeAsString(
      step.inputId.changeExtension('.macro_declarations.json'),
      '${jsonEncode({'versao': 1, 'resultados': resultados})}\n',
    );
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
    final aplicacoes = <Map<String, Object?>>[];
    for (final alvo in library.topLevelElements) {
      final nome = alvo.name;
      if (nome == null) continue;
      for (final anotacao in alvo.metadata) {
        final elemento = anotacao.element;
        if (elemento is! ConstructorElement) continue;
        final classe = elemento.enclosingElement3;
        if (classe is! ClassElement || !_eMacro(classe)) continue;
        final aplicacao = <String, Object?>{
          'alvo': nome,
          'biblioteca': classe.librarySource.uri.toString(),
          'classe': classe.name,
          'construtor': elemento.name,
        };
        if (alvo is ClassElement) {
          try {
            aplicacao['execucao'] = modeloDaClasse(alvo);
          } on UnsupportedError catch (e) {
            aplicacao['modelo_indisponivel'] =
                e.message ?? 'declaração ainda não coberta';
          }
        }
        aplicacoes.add(aplicacao);
      }
    }
    if (aplicacoes.isEmpty) return;
    final saida = step.inputId.changeExtension('.macro_uses.json');
    await step.writeAsString(
        saida, '${jsonEncode({'versao': 1, 'aplicacoes': aplicacoes})}\n');
  }
}

bool _eMacro(ClassElement classe) {
  // `ClassElement` do analyzer 7.3 não expõe `isMacro`; o AST expõe o token
  // mesmo quando há modificadores antes dele (`abstract macro class`).
  final fonte = classe.source.contents.data;
  final unidade = parseString(
    content: fonte,
    featureSet: FeatureSet.latestLanguageVersion(flags: ['macros']),
    throwIfDiagnostics: false,
  ).unit;
  return unidade.declarations.whereType<ClassDeclaration>().any(
        // ignore: deprecated_member_use
        (d) => d.name.lexeme == classe.name && d.macroKeyword != null,
      );
}
