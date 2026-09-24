import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:dartforge_macros_builder/src/resolver_identificadores.dart';
import 'package:json/json.dart';
import 'package:macros/src/executor/executar.dart';
import 'package:macros/src/executor/modelo.dart';
import 'package:macros/src/executor/resultado.dart';

final class HospedeiroAnalyzer implements Hospedeiro {
  final ResolvedorIdentificadores resolvedor;
  HospedeiroAnalyzer(this.resolvedor);

  @override
  Future<Object?> consultar(String tipo, Map<String, Object?> args) {
    if (tipo != 'resolverIdentificador')
      throw UnsupportedError('consulta $tipo ainda não implementada');
    return resolvedor.resolver(args['uri'] as String, args['nome'] as String);
  }
}

Object? trocarUri(Object? valor) {
  if (valor == 'package:caso_json/modelos.dart') {
    return 'package:corpus_macros_discovery/modelos.dart';
  }
  if (valor is List) return [for (final item in valor) trocarUri(item)];
  if (valor is Map) {
    return {
      for (final entrada in valor.entries) entrada.key: trocarUri(entrada.value)
    };
  }
  return valor;
}

bool igual(Object? a, Object? b) {
  if (a is List && b is List) {
    return a.length == b.length &&
        List.generate(a.length, (i) => i).every((i) => igual(a[i], b[i]));
  }
  if (a is Map && b is Map) {
    return a.length == b.length &&
        a.keys.every(
            (chave) => b.containsKey(chave) && igual(a[chave], b[chave]));
  }
  return a == b;
}

Future<void> main() async {
  final manifesto =
      jsonDecode(File('lib/modelos.macro_uses.json').readAsStringSync()) as Map;
  final aplicacoes = manifesto['aplicacoes'] as List;
  final endereco =
      aplicacoes.singleWhere((a) => a['alvo'] == 'Endereco') as Map;
  final sessao = File('../../macros/410_json_codable/esperado/sessao.dfexec')
      .readAsLinesSync();
  final pedido = sessao
      .map((linha) => jsonDecode(linha.substring(2)) as Map)
      .singleWhere((m) => m['t'] == 'macro.executar' && m['id'] == 4);
  final esperado =
      trocarUri({'alvo': pedido['alvo'], 'modelo': pedido['modelo']});
  if (!igual(endereco['execucao'], esperado)) {
    throw StateError(
        'modelo do Endereco difere do pedido macro.executar do CFE');
  }
  final caminho = File(
          '${Directory.current.path}${Platform.pathSeparator}lib${Platform.pathSeparator}modelos.dart')
      .absolute
      .path;
  final contextos = AnalysisContextCollection(
      includedPaths: [Directory.current.absolute.path]);
  try {
    final biblioteca = await contextos
        .contextFor(caminho)
        .currentSession
        .getLibraryByUri('package:corpus_macros_discovery/modelos.dart');
    if (biblioteca is! LibraryElementResult)
      throw StateError('biblioteca do fixture não resolvida');
    final classe = biblioteca.element.topLevelElements
        .whereType<ClassElement>()
        .singleWhere((c) => c.name == 'Endereco');
    final resolvedor = ResolvedorIdentificadores(
        classe, Map<String, Object?>.from(endereco['execucao'] as Map));
    final consultas = <int, Map<String, Object?>>{};
    var resolvidas = 0;
    for (final linha in sessao) {
      final m = jsonDecode(linha.substring(2)) as Map;
      if (m['t'] == 'macro.consulta' && m['execucao'] == 4) {
        consultas[m['id'] as int] = Map<String, Object?>.from(m['args'] as Map);
      } else if (m['t'] == 'macro.resposta' && consultas.containsKey(m['id'])) {
        final args = consultas.remove(m['id'])!;
        final obtido = await resolvedor.resolver(
            args['uri'] as String, args['nome'] as String);
        if (!igual(obtido, m['valor']))
          throw StateError('resolverIdentificador divergiu do CFE: $args');
        resolvidas++;
      }
    }
    if (resolvidas != 3)
      throw StateError('$resolvidas consultas resolvidas; esperadas 3');
    final execucao = Map<String, Object?>.from(endereco['execucao'] as Map);
    final modelo = Modelo(HospedeiroAnalyzer(resolvedor))
      ..receber(Map<String, Object?>.from(execucao['modelo'] as Map));
    final alvo =
        modelo.declaracao(Map<String, Object?>.from(execucao['alvo'] as Map));
    final resultado = (await executarFase(
            const JsonCodable(), Fase.declaracoes, alvo, Introspector(modelo)))
        .paraJson();
    final esperadoResultado = sessao
        .map((linha) => jsonDecode(linha.substring(2)) as Map)
        .singleWhere(
            (m) => m['t'] == 'macro.resultado' && m['id'] == 4)['resultado'];
    if (!igual(resultado, esperadoResultado))
      throw StateError('fase de declarações divergiu do CFE');
    print(
        'modelo, $resolvidas consultas e fase de declarações iguais ao CFE, via analyzer');
  } finally {
    await contextos.dispose();
  }
}
