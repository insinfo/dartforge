import 'dart:convert';
import 'dart:io';

import 'package:json/json.dart' as json;
import 'package:macros/macros.dart';
import 'package:macros/src/executor/executar.dart';
import 'package:macros/src/executor/modelo.dart';
import 'package:macros/src/executor/resultado.dart';

final class _Consulta {
  final String tipo;
  final Map<String, Object?> args;
  final Object? valor;
  final Map<String, Object?>? erro;
  _Consulta(this.tipo, this.args, this.valor, this.erro);
}

bool _igual(Object? a, Object? b) {
  if (a is Map && b is Map) {
    return a.length == b.length &&
        a.keys.every(
            (chave) => b.containsKey(chave) && _igual(a[chave], b[chave]));
  }
  if (a is List && b is List) {
    return a.length == b.length &&
        List.generate(a.length, (i) => i).every((i) => _igual(a[i], b[i]));
  }
  return a == b;
}

final class _HospedeiroGravado implements Hospedeiro {
  final List<_Consulta> pendentes;
  _HospedeiroGravado(this.pendentes);

  @override
  Future<Object?> consultar(String tipo, Map<String, Object?> args) async {
    final indice =
        pendentes.indexWhere((p) => p.tipo == tipo && _igual(p.args, args));
    if (indice < 0)
      throw StateError('consulta ausente no oráculo: $tipo $args');
    final resposta = pendentes.removeAt(indice);
    final erro = resposta.erro;
    if (erro != null) {
      throw ErroDoHospedeiro(
          erro['tipo'] as String, erro['mensagem'] as String);
    }
    return resposta.valor;
  }
}

Macro _macro(String nome) => switch (nome) {
      'Endereco' || 'Usuario' => const json.JsonCodable(),
      'SoSaida' => const json.JsonEncodable(),
      'SoEntrada' => const json.JsonDecodable(),
      _ => throw StateError('macro desconhecida no corpus: $nome'),
    };

Future<void> main() async {
  final linhas =
      File('../../corpus/macros/410_json_codable/esperado/sessao.dfexec')
          .readAsLinesSync();
  final execucoes = <int, Map<String, Object?>>{};
  final resultados = <int, Map<String, Object?>>{};
  final consultas = <int, Map<String, Object?>>{};
  final porExecucao = <int, List<_Consulta>>{};
  for (final linha in linhas) {
    final m = Map<String, Object?>.from(jsonDecode(linha.substring(2)) as Map);
    final id = m['id'];
    switch (m['t']) {
      case 'macro.executar':
        execucoes[id as int] = m;
      case 'macro.resultado':
        resultados[id as int] =
            Map<String, Object?>.from(m['resultado'] as Map);
      case 'macro.consulta':
        consultas[id as int] = m;
      case 'macro.resposta':
        final pedido = consultas.remove(id);
        if (pedido == null) throw StateError('resposta $id sem consulta');
        (porExecucao[pedido['execucao'] as int] ??= []).add(_Consulta(
          pedido['tipo'] as String,
          Map<String, Object?>.from(pedido['args'] as Map),
          m['valor'],
          m['erro'] is Map ? Map<String, Object?>.from(m['erro'] as Map) : null,
        ));
    }
  }

  for (final entrada in execucoes.entries) {
    final id = entrada.key;
    final execucao = entrada.value;
    final hospedeiro = _HospedeiroGravado(porExecucao[id] ?? []);
    final modelo = Modelo(hospedeiro)
      ..receber(Map<String, Object?>.from(execucao['modelo'] as Map));
    final alvoJson = Map<String, Object?>.from(execucao['alvo'] as Map);
    final alvo = modelo.declaracao(alvoJson);
    final nome = (alvoJson['ident'] as Map)['nome'] as String;
    final fase = Fase.values.byName(execucao['fase'] as String);
    final obtido =
        (await executarFase(_macro(nome), fase, alvo, Introspector(modelo)))
            .paraJson();
    final esperado = resultados[id];
    if (!_igual(obtido, esperado)) {
      throw StateError('resultado $id ($nome, $fase) divergiu do CFE:\n'
          'obtido: ${jsonEncode(obtido)}\nesperado: ${jsonEncode(esperado)}');
    }
    if (hospedeiro.pendentes.isNotEmpty) {
      throw StateError(
          'execução $id deixou ${hospedeiro.pendentes.length} consultas sem uso');
    }
  }
  print(
      'executor no mesmo isolate: ${execucoes.length} resultados iguais à sessão CFE');
}
