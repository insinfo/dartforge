// Builders locais do caso: código trivial, para o oráculo exercitar só a
// configuração. Toda saída começa por um cabeçalho com o nome do builder, a
// entrada, `options.isRoot` e `options.config` em JSON com as chaves
// ordenadas — o oráculo registra a precedência das opções (defaults <
// dev/release dos defaults < alvo < dev/release do alvo < global_options <
// --define) e, pelos arquivos que existem, quais passos rodaram.
//
// O mesmo arquivo serve ao caso `corpus/builders/perfil_2_4` (build 2.4) e ao
// `corpus/builders_novos/perfil_2_16` (build 4): só usa a API comum.
import 'dart:convert';

import 'package:build/build.dart';

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

/// Escreve o cabecalho e o tamanho da entrada em cada saída permitida.
class Registro implements Builder {
  final String nome;
  final BuilderOptions opcoes;
  @override
  final Map<String, List<String>> buildExtensions;

  Registro(this.nome, this.opcoes, this.buildExtensions);

  @override
  Future<void> build(BuildStep passo) async {
    final texto = await passo.readAsString(passo.inputId);
    for (final saida in passo.allowedOutputs) {
      await passo.writeAsString(
          saida, '${cabecalho(nome, passo.inputId, opcoes)}${texto.length}\n');
    }
  }
}

/// `run_only_if_triggered: true` nos defaults; trigger `annotation Gerar`.
Builder anotacao(BuilderOptions o) => Registro('anotacao', o, const {
      '.dart': ['.anotacao.txt'],
    });

/// `run_only_if_triggered: true` nos defaults; trigger por import.
Builder importacao(BuilderOptions o) => Registro('importacao', o, const {
      '.dart': ['.importacao.txt'],
    });

/// `run_only_if_triggered: true` nos defaults, desligado no alvo (`false`):
/// roda em toda entrada.
Builder sempre(BuilderOptions o) => Registro('sempre', o, const {
      '.dart': ['.sempre.txt'],
    });

/// `run_only_if_triggered: true` e nenhum trigger: nunca roda.
Builder semGatilho(BuilderOptions o) => Registro('sem_gatilho', o, const {
      '.dart': ['.sem_gatilho.txt'],
    });

/// `enabled: false` no alvo: nunca roda.
Builder desligado(BuilderOptions o) => Registro('desligado', o, const {
      '.dart': ['.desligado.txt'],
    });

/// Gera uma parte (`build_to: cache`) que traz a anotação: o trigger de
/// anotação lê as partes, inclusive as geradas em fase anterior.
Builder semente(BuilderOptions o) => _Semente();

class _Semente implements Builder {
  @override
  final buildExtensions = const {
    '.semente': ['.parte.dart'],
  };

  @override
  Future<void> build(BuildStep passo) async {
    final nome = (await passo.readAsString(passo.inputId)).trim();
    await passo.writeAsString(
        passo.allowedOutputs.single,
        "part of 'semeado.dart';\n\n"
        '@Gerar()\n'
        'class $nome {}\n');
  }
}

/// `^lib/resumo.lista` -> `lib/resumo.txt`: lê cada caminho listado (saídas
/// de outras fases). Com `--build-filter` só nele, o oficial constrói sob
/// demanda o que ele lê.
Builder resumo(BuilderOptions o) => _Resumo(o);

class _Resumo implements Builder {
  final BuilderOptions opcoes;
  _Resumo(this.opcoes);

  @override
  final buildExtensions = const {
    '^lib/resumo.lista': ['lib/resumo.txt'],
  };

  @override
  Future<void> build(BuildStep passo) async {
    final linhas = <String>[];
    for (final linha in const LineSplitter()
        .convert(await passo.readAsString(passo.inputId))
        .map((l) => l.trim())
        .where((l) => l.isNotEmpty)) {
      final id = AssetId(passo.inputId.package, linha);
      if (await passo.canRead(id)) {
        final texto = await passo.readAsString(id);
        linhas.add('$linha: ${const LineSplitter().convert(texto).first}');
      } else {
        linhas.add('$linha: ilegível');
      }
    }
    await passo.writeAsString(passo.allowedOutputs.single,
        '${cabecalho('resumo', passo.inputId, opcoes)}${linhas.join('\n')}\n');
  }
}
