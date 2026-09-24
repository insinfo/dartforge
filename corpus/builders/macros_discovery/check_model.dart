import 'dart:convert';
import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:dartforge_macros_builder/src/resolver_identificadores.dart';
import 'package:dartforge_macros_builder/src/consultas_definicoes.dart';
import 'package:dartforge_macros_builder/src/modelo_analyzer.dart';
import 'package:dartforge_macros_builder/src/tabela_identificadores.dart';
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

Object? semIds(Object? valor) {
  if (valor is List) return [for (final item in valor) semIds(item)];
  if (valor is Map) {
    return {
      for (final entrada in valor.entries)
        if (entrada.key != 'id' && entrada.key != 'chave' && entrada.key != 'i')
          entrada.key: semIds(entrada.value),
    };
  }
  return valor;
}

Object? semIdDoAlvo(Object? resultado) {
  final mapa = Map<String, Object?>.from(resultado as Map);
  mapa['tipos'] = [
    for (final par in mapa['tipos'] as List) [0, (par as List)[1]],
  ];
  return semIds(mapa);
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
    final declaracoes = jsonDecode(
        File('lib/modelos.macro_declarations.json').readAsStringSync()) as Map;
    final geradas = declaracoes['resultados'] as List;
    if (declaracoes['versao'] != 1 || geradas.length != 4)
      throw StateError('builder não executou as quatro aplicações registradas');
    final geradosPorAlvo = {
      for (final item in geradas) (item as Map)['alvo']: item,
    };
    final idsDosAlvos = <int>{};
    final idsDoCore = <String, int>{};
    void conferirIds(Object? valor) {
      if (valor is List) {
        for (final item in valor) conferirIds(item);
      } else if (valor is Map) {
        if (valor['i'] is int && valor['n'] is String) {
          final nome = valor['n'] as String;
          if (const {'Map', 'String', 'Object'}.contains(nome)) {
            final anterior =
                idsDoCore.putIfAbsent(nome, () => valor['i'] as int);
            if (anterior != valor['i'])
              throw StateError('id de dart:core mudou entre aplicações: $nome');
          }
        }
        for (final item in valor.values) conferirIds(item);
      }
    }

    for (final alvo in ['Endereco', 'Usuario', 'SoSaida', 'SoEntrada']) {
      final item = geradosPorAlvo[alvo];
      if (item == null) throw StateError('resultado ausente para $alvo');
      final resultadoDaMacro = item['resultado'] as Map;
      if (resultadoDaMacro['excecao'] != null ||
          (resultadoDaMacro['diagnosticos'] as List).isNotEmpty ||
          (resultadoDaMacro['tipos'] as List).isEmpty) {
        throw StateError('macro de $alvo não gerou declarações válidas');
      }
      final tipos = resultadoDaMacro['tipos'] as List;
      final id = (tipos.single as List).first as int;
      if (!idsDosAlvos.add(id))
        throw StateError('duas classes compartilham o identificador $id');
      conferirIds(resultadoDaMacro);
    }
    if (idsDoCore.length != 3 ||
        (geradosPorAlvo['Endereco'] as Map)['resultado']['tipos'][0][0] != 1 ||
        (geradosPorAlvo['Usuario'] as Map)['resultado']['tipos'][0][0] != 8) {
      throw StateError(
          'tabela de ids divergiu das duas primeiras aplicações do CFE');
    }
    final parcial = File('lib/modelos.macro_declarations.txt')
        .readAsStringSync()
        .replaceAll('\r\n', '\n');
    final augmentationCfe =
        File('../../macros/410_json_codable/esperado/modelos.augmentation.dart')
            .readAsStringSync()
            .replaceAll('\r\n', '\n');
    String declaracoesDe(String texto, String nome) {
      final inicio = texto.indexOf('augment class $nome {\n');
      if (inicio < 0) throw StateError('classe $nome ausente da augmentation');
      final proximaDefinicao = texto.indexOf('\n  augment ', inicio);
      final fimDaClasse = texto.indexOf('\n}', inicio);
      if (fimDaClasse < 0) throw StateError('classe $nome sem fechamento');
      final fim = proximaDefinicao >= 0 && proximaDefinicao < fimDaClasse
          ? proximaDefinicao
          : fimDaClasse;
      return texto.substring(inicio, fim);
    }

    for (final alvo in ['Endereco', 'Usuario', 'SoSaida', 'SoEntrada']) {
      if (declaracoesDe(parcial, alvo) !=
          declaracoesDe(augmentationCfe, alvo)) {
        throw StateError('declarações montadas de $alvo diferem do CFE');
      }
    }
    final modelosDeDefinicao = jsonDecode(
            File('lib/modelos.macro_definitions_model.json').readAsStringSync())
        as Map;
    final pedidosDeDefinicao = modelosDeDefinicao['aplicacoes'] as List;
    if (modelosDeDefinicao['versao'] != 1 || pedidosDeDefinicao.length != 4) {
      throw StateError('modelos de definições incompletos');
    }
    final pedidoCfeDef = sessao
        .map((linha) => jsonDecode(linha.substring(2)) as Map)
        .singleWhere((m) => m['t'] == 'macro.executar' && m['id'] == 8);
    final membrosCfe =
        trocarUri((pedidoCfeDef['modelo'] as Map)['membros']['1']);
    final geradoEndereco =
        pedidosDeDefinicao.singleWhere((a) => a['alvo'] == 'Endereco') as Map;
    final membrosBuilder =
        ((geradoEndereco['execucao'] as Map)['modelo'] as Map)['membros']['1'];
    if (!igual(semIds(membrosBuilder), semIds(membrosCfe))) {
      throw StateError('modelo pós-declarações de Endereco difere do CFE');
    }
    final tabelaDef = TabelaIdentificadores();
    final inicialDef = modeloDaClasse(classe, tabelaDef);
    final resolvedorDef =
        ResolvedorIdentificadores(classe, inicialDef, tabelaDef);
    for (final nome in ['Map', 'String', 'Object']) {
      await resolvedorDef.resolver('dart:core', nome);
    }
    final consultasDef = ConsultasDefinicoes(resolvedorDef);
    final pendentesDef = <int, Map<String, Object?>>{};
    var respostasDef = 0;
    for (final linha in sessao) {
      final m = jsonDecode(linha.substring(2)) as Map;
      if (m['t'] == 'macro.consulta' && m['execucao'] == 8) {
        pendentesDef[m['id'] as int] = Map<String, Object?>.from(m);
      } else if (m['t'] == 'macro.resposta' &&
          pendentesDef.containsKey(m['id'])) {
        final pedido = pendentesDef.remove(m['id'])!;
        final tipo = pedido['tipo'] as String;
        final valor = await consultasDef.consultar(
            tipo, Map<String, Object?>.from(pedido['args'] as Map));
        if (!igual(semIds(valor), semIds(m['valor']))) {
          throw StateError('consulta de definição $tipo divergiu do CFE');
        }
        respostasDef++;
      }
    }
    if (respostasDef != 14) {
      throw StateError('$respostasDef consultas de definições; esperadas 14');
    }
    final definicoes = jsonDecode(
        File('lib/modelos.macro_definitions.json').readAsStringSync()) as Map;
    final resultadosDef = definicoes['resultados'] as List;
    if (definicoes['versao'] != 1 || resultadosDef.length != 4) {
      throw StateError('fase de definições não executou as quatro aplicações');
    }
    for (final item in resultadosDef) {
      final resultado = (item as Map)['resultado'] as Map;
      if (resultado['excecao'] != null ||
          (resultado['diagnosticos'] as List).isNotEmpty ||
          (resultado['tipos'] as List).isEmpty) {
        throw StateError('fase de definições de ${item['alvo']} incompleta');
      }
    }
    for (final caso in <(String, int)>[
      ('Endereco', 8),
      ('SoSaida', 10),
      ('SoEntrada', 11),
    ]) {
      final (alvo, idCfe) = caso;
      final resultadoDef =
          resultadosDef.singleWhere((r) => r['alvo'] == alvo) as Map;
      final resultadoCfeDef = sessao
          .map((linha) => jsonDecode(linha.substring(2)) as Map)
          .singleWhere((m) => m['t'] == 'macro.resultado' && m['id'] == idCfe);
      if (!igual(semIdDoAlvo(resultadoDef['resultado']),
          semIdDoAlvo(resultadoCfeDef['resultado']))) {
        throw StateError('fase de definições de $alvo divergiu do CFE');
      }
    }
    final peloBuilder =
        geradas.singleWhere((r) => r['alvo'] == 'Endereco') as Map;
    if (peloBuilder['macro'] != 'package:json/json.dart#JsonCodable' ||
        !igual(peloBuilder['resultado'], esperadoResultado)) {
      throw StateError('resultado do build_runner divergiu do CFE');
    }
    const uriReexportada = 'package:corpus_macros_discovery/reexportado.dart';
    final bibliotecaReexportada =
        await classe.library.session.getLibraryByUri(uriReexportada);
    if (bibliotecaReexportada is! LibraryElementResult ||
        bibliotecaReexportada.element.exportNamespace.get('Random') == null) {
      throw StateError('fixture de reexportação não expõe Random');
    }
    var rejeitouReexportacao = false;
    try {
      await resolvedor.resolver(uriReexportada, 'Random');
    } on StateError {
      rejeitouReexportacao = true;
    }
    if (!rejeitouReexportacao)
      throw StateError('reexportação aceita como declaração local');
    print(
        'modelo, $resolvidas consultas e fase de declarações iguais ao CFE; build_runner executou e montou 4 aplicações; modelo, $respostasDef consultas e 3 resultados de definições iguais ao CFE; reexportação rejeitada');
  } finally {
    await contextos.dispose();
  }
}
