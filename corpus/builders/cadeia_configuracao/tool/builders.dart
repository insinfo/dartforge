// Builders locais do caso: código trivial (texto derivado da entrada), para o
// oráculo exercitar só a configuração. Toda saída começa por um cabeçalho com
// o nome do builder, a entrada, `options.isRoot` e `options.config` em JSON
// com as chaves ordenadas — assim o oráculo registra a precedência de opções
// (defaults < dev/release dos defaults < alvo < dev/release do alvo <
// global_options).
import 'dart:convert';

import 'package:build/build.dart';
import 'package:glob/glob.dart';

Object? _ordenado(Object? valor) {
  if (valor is Map) {
    final chaves = valor.keys.map((k) => '$k').toList()..sort();
    return {for (final k in chaves) k: _ordenado(valor[k])};
  }
  if (valor is List) return [for (final v in valor) _ordenado(v)];
  return valor;
}

String cabecalho(String nome, AssetId entrada, BuilderOptions opcoes) =>
    'builder: $nome\n'
    'entrada: $entrada\n'
    'isRoot: ${opcoes.isRoot}\n'
    'opcoes: ${jsonEncode(_ordenado(opcoes.config))}\n'
    '---\n';

typedef Transformacao = Future<String> Function(BuildStep passo, String texto);

class Texto implements Builder {
  final String nome;
  final BuilderOptions opcoes;
  @override
  final Map<String, List<String>> buildExtensions;
  final Transformacao transformar;

  Texto(this.nome, this.opcoes, this.buildExtensions, this.transformar);

  @override
  Future<void> build(BuildStep passo) async {
    final texto = await passo.readAsString(passo.inputId);
    final corpo = await transformar(passo, texto);
    for (final saida in passo.allowedOutputs) {
      await passo.writeAsString(saida, cabecalho(nome, passo.inputId, opcoes) + corpo);
    }
  }
}

/// auto_apply: root_package, build_to: cache. `.txt` -> `.maiusculas.txt`.
Builder maiusculas(BuilderOptions o) => Texto('maiusculas', o, const {
      '.txt': ['.maiusculas.txt'],
    }, (_, t) async => t.toUpperCase());

/// Duas fábricas num builder só (`contagem`); cada uma declara a sua metade
/// das extensões do build.yaml. auto_apply: all_packages.
Builder contarLinhas(BuilderOptions o) => Texto('contagem/linhas', o, const {
      '.maiusculas.txt': ['.linhas.txt'],
    }, (_, t) async => '${const LineSplitter().convert(t).length}\n');

Builder contarPalavras(BuilderOptions o) => Texto('contagem/palavras', o, const {
      '.maiusculas.txt': ['.palavras.txt'],
    }, (_, t) async {
      final palavras = t.split(RegExp(r'\s+')).where((p) => p.isNotEmpty).toList();
      return '${palavras.length}\n';
    });

/// Captura `{{}}`: lib/entrada/<x>.txt -> lib/espelho/<x>.espelho.txt
/// (build_to: source, auto_apply: none, ligado no alvo).
Builder espelho(BuilderOptions o) => Texto('espelho', o, const {
      'lib/entrada/{{}}.txt': ['lib/espelho/{{}}.espelho.txt'],
    }, (_, t) async => '${String.fromCharCodes(t.trimRight().runes.toList().reversed)}\n');

/// Extensão com `^`: um caminho exato de entrada para um caminho exato de
/// saída. Lista o que já existe em lib/ na fase em que roda — o conteúdo
/// depende da ordem das fases (required_inputs, runs_before global).
Builder indice(BuilderOptions o) => Texto('indice', o, const {
      '^lib/entrada/indice.lista': ['lib/indice.g.txt'],
    }, (passo, t) async {
      final padroes = const LineSplitter().convert(t).where((l) => l.trim().isNotEmpty);
      final linhas = <String>[];
      for (final padrao in padroes) {
        final achados = await passo.findAssets(Glob(padrao.trim())).map((a) => a.path).toList();
        achados.sort();
        linhas.add('$padrao -> ${achados.join(', ')}');
      }
      return '${linhas.join('\n')}\n';
    });

/// auto_apply: root_package, mas `enabled: false` no alvo: nunca roda.
Builder desligado(BuilderOptions o) => Texto('desligado', o, const {
      '.dart': ['.desligado.txt'],
    }, (_, t) async => t);

/// auto_apply: dependents num builder do próprio pacote raiz: ninguém depende
/// dele, então não roda.
Builder ignorado(BuilderOptions o) => Texto('ignorado', o, const {
      '.dart': ['.ignorado.txt'],
    }, (_, t) async => t);

/// Post-process: resume cada `.rascunho` num `.rascunho.resumo` (cache) e só
/// apaga a entrada quando `apagar` é true (release_options dos defaults).
PostProcessBuilder limpeza(BuilderOptions o) => _Limpeza(o);

class _Limpeza implements PostProcessBuilder {
  final BuilderOptions opcoes;
  _Limpeza(this.opcoes);

  @override
  Iterable<String> get inputExtensions => const ['.rascunho'];

  @override
  Future<void> build(PostProcessBuildStep passo) async {
    final texto = await passo.readInputAsString();
    final primeira = const LineSplitter().convert(texto).first;
    await passo.writeAsString(passo.inputId.addExtension('.resumo'),
        '${cabecalho('limpeza', passo.inputId, opcoes)}$primeira\n');
    if (opcoes.config['apagar'] == true) passo.deletePrimaryInput();
  }
}
