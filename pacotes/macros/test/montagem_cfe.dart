import 'dart:convert';
import 'dart:io';

import 'package:macros/src/executor/montagem.dart';

/// Reproduz os resultados reais de `package:json` 0.20.4 registrados em
/// `sessao.dfexec`, incluindo as consultas de identificação ao hospedeiro.
/// O oráculo é a augmentation extraída do CFE 3.6.2, não código escrito aqui.
final class _TabelaDaSessao implements ResolvedorMontagem {
  final ids = <int, IdentificadorResolvido>{};
  final tipos = <int, TipoAumentado>{};
  final consultas = <int, Map<String, Object?>>{};

  void registrar(Object? valor) {
    if (valor is List) {
      for (final item in valor) registrar(item);
      return;
    }
    if (valor is! Map) return;
    final m = Map<String, Object?>.from(valor);
    final ident = m['ident'];
    if (ident is Map && ident['id'] is int && ident['nome'] is String) {
      final id = ident['id'] as int;
      final nome = ident['nome'] as String;
      final lib = m['lib'];
      final uri = lib is Map ? lib['uri'] as String? : null;
      final dono = m['dono'];
      final escopo = dono is Map ? dono['nome'] as String? : null;
      switch (m['k']) {
        case 'classe' || 'enum' || 'mixin':
          ids[id] =
              IdentificadorResolvido(nome, TipoDeIdentificador.topo, uri: uri);
          tipos[id] = TipoAumentado(
            m['k'] == 'classe' ? 'class' : m['k'] as String,
            [
              for (final modificador in [
                'abstract',
                'base',
                'final',
                'interface',
                'mixin',
                'sealed'
              ])
                if (m[modificador] == true) modificador,
            ],
            nome,
          );
        case 'campo' || 'metodo':
          ids[id] = m['static'] == true
              ? IdentificadorResolvido(nome, TipoDeIdentificador.estatico,
                  uri: uri, escopo: escopo)
              : IdentificadorResolvido(nome, TipoDeIdentificador.instancia);
        case 'construtor':
          ids[id] = IdentificadorResolvido(nome, TipoDeIdentificador.estatico,
              uri: uri, escopo: escopo);
        case 'parametro':
          ids[id] = IdentificadorResolvido(nome, TipoDeIdentificador.local);
      }
    }
    for (final filho in m.values) registrar(filho);
  }

  void mensagem(Map<String, Object?> m) {
    if (m['t'] == 'macro.consulta' && m['tipo'] == 'resolverIdentificador') {
      consultas[m['id'] as int] = Map<String, Object?>.from(m['args'] as Map);
    } else if (m['t'] == 'macro.resposta' && consultas.containsKey(m['id'])) {
      final resposta = m['valor'] as Map;
      final args = consultas.remove(m['id'])!;
      ids[resposta['id'] as int] = IdentificadorResolvido(
        resposta['nome'] as String,
        TipoDeIdentificador.topo,
        uri: args['uri'] as String,
      );
    }
    registrar(m);
  }

  @override
  IdentificadorResolvido identificador(int id) =>
      ids[id] ?? (throw StateError('identificador $id ausente'));

  @override
  TipoAumentado tipoAumentado(int id) =>
      tipos[id] ?? (throw StateError('tipo $id ausente'));

  @override
  Map<String, Object?>? tipoInferido(int chave) => null;
}

void main() {
  final raiz = Directory('../../corpus/macros/410_json_codable/esperado');
  final tabela = _TabelaDaSessao();
  final resultados = <Map<String, Object?>>[];
  for (final linha in File('${raiz.path}/sessao.dfexec').readAsLinesSync()) {
    final mensagem =
        Map<String, Object?>.from(jsonDecode(linha.substring(2)) as Map);
    tabela.mensagem(mensagem);
    if (mensagem['t'] == 'macro.resultado') {
      resultados.add(Map<String, Object?>.from(mensagem['resultado'] as Map));
    }
  }
  final obtido = montarAugmentation(
    resultados,
    tabela,
    cabecalho: 'augment library',
    uri: 'package:caso_json/modelos.dart',
  );
  final esperado =
      File('${raiz.path}/modelos.augmentation.dart').readAsStringSync();
  if (obtido != esperado) {
    var posicao = 0;
    while (posicao < obtido.length &&
        posicao < esperado.length &&
        obtido[posicao] == esperado[posicao]) {
      posicao++;
    }
    throw StateError('augmentation diferente do CFE no caractere $posicao '
        '(obtido ${obtido.length}, esperado ${esperado.length})');
  }
  print(
      'montagem Dart: ${resultados.length} resultados de macro idênticos ao CFE 3.6.2 (${obtido.length} caracteres)');
}
