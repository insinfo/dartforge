import 'dart:convert';
import 'dart:io';

import 'package:json/json.dart';
import 'package:macros/src/executor/executar.dart';
import 'package:macros/src/executor/modelo.dart';
import 'package:macros/src/executor/resultado.dart';

bool igual(Object? a, Object? b) {
  if (a is Map && b is Map) {
    return a.length == b.length &&
        a.keys.every(
            (chave) => b.containsKey(chave) && igual(a[chave], b[chave]));
  }
  if (a is List && b is List) {
    return a.length == b.length &&
        List.generate(a.length, (i) => i).every((i) => igual(a[i], b[i]));
  }
  return a == b;
}

final class _HospedeiroDaSessao implements Hospedeiro {
  final List<(Map<String, Object?>, Object?)> pendentes;
  _HospedeiroDaSessao(this.pendentes);

  @override
  Future<Object?> consultar(String tipo, Map<String, Object?> args) async {
    if (tipo != 'resolverIdentificador')
      throw StateError('consulta não coberta: $tipo');
    final indice = pendentes.indexWhere((par) => igual(par.$1, args));
    if (indice < 0) throw StateError('resposta CFE ausente: $args');
    return pendentes.removeAt(indice).$2;
  }
}

Future<void> main() async {
  final manifesto = jsonDecode(
      File('../../corpus/builders/macros_discovery/lib/modelos.macro_uses.json')
          .readAsStringSync()) as Map;
  final aplicacao = (manifesto['aplicacoes'] as List)
      .singleWhere((a) => a['alvo'] == 'Endereco') as Map;
  final execucao = Map<String, Object?>.from(aplicacao['execucao'] as Map);
  final linhas =
      File('../../corpus/macros/410_json_codable/esperado/sessao.dfexec')
          .readAsLinesSync();
  final consultas = <int, Map<String, Object?>>{};
  final respostas = <(Map<String, Object?>, Object?)>[];
  Map<String, Object?>? esperado;
  for (final linha in linhas) {
    final m = Map<String, Object?>.from(jsonDecode(linha.substring(2)) as Map);
    if (m['t'] == 'macro.consulta' && m['execucao'] == 4) {
      consultas[m['id'] as int] = Map<String, Object?>.from(m['args'] as Map);
    } else if (m['t'] == 'macro.resposta' && consultas.containsKey(m['id'])) {
      respostas.add((consultas.remove(m['id'])!, m['valor']));
    } else if (m['t'] == 'macro.resultado' && m['id'] == 4) {
      esperado = Map<String, Object?>.from(m['resultado'] as Map);
    }
  }
  final hospedeiro = _HospedeiroDaSessao(respostas);
  final modelo = Modelo(hospedeiro)
    ..receber(Map<String, Object?>.from(execucao['modelo'] as Map));
  final alvo =
      modelo.declaracao(Map<String, Object?>.from(execucao['alvo'] as Map));
  final obtido = (await executarFase(
          const JsonCodable(), Fase.declaracoes, alvo, Introspector(modelo)))
      .paraJson();
  if (!igual(obtido, esperado) || hospedeiro.pendentes.isNotEmpty) {
    throw StateError(
        'JsonCodable com modelo do analyzer difere da fase 2 do CFE: '
        '${jsonEncode(obtido)}');
  }
  print(
      'fase de declarações JsonCodable igual ao CFE com modelo do analyzer (3 consultas)');
}
