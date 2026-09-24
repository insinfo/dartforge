import 'dart:convert';
import 'dart:io';

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

void main() {
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
  print('modelo do Endereco igual ao pedido macro.executar do CFE');
}
