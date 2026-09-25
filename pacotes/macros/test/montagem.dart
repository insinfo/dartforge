import 'package:macros/src/executor/montagem.dart';

Map<String, Object?> codigo(List<Object?> partes) => {'k': 'raw', 'p': partes};

final class Tabela implements ResolvedorMontagem {
  @override
  IdentificadorResolvido identificador(int id) => switch (id) {
        1 => const IdentificadorResolvido(
            'Comparable', TipoDeIdentificador.topo,
            uri: 'dart:core'),
        2 => const IdentificadorResolvido('Ponto', TipoDeIdentificador.topo,
            uri: 'package:a/a.dart'),
        _ => throw StateError('identificador $id'),
      };

  @override
  TipoAumentado tipoAumentado(int id) => switch (id) {
        2 => TipoAumentado(
            'class',
            const ['abstract', 'base'],
            'Ponto',
            [
              codigo(['T'])
            ]),
        _ => throw StateError('tipo $id'),
      };

  @override
  Map<String, Object?>? tipoInferido(int chave) => null;
}

void conferir(String obtido, String esperado) {
  if (obtido != esperado) {
    throw StateError(
        'augmentation diferente:\n--- obtido ---\n$obtido--- esperado ---\n$esperado');
  }
}

void main() {
  // Mesmo contrato de ordem, cabeçalho e imports de `montagem.rs`, cuja
  // saída foi conferida com o CFE 3.6.2.
  final resultado = [
    {
      'biblioteca': [
        codigo(['class PontoGemeo {}'])
      ],
      'interfaces': [
        [
          2,
          [
            codigo([
              {'i': 1},
              '<',
              {'i': 2},
              '>'
            ])
          ],
        ],
      ],
    },
    {
      'tipos': [
        [
          2,
          [
            codigo(['  int get x => 1;'])
          ],
        ],
      ],
    },
  ];
  conferir(
    montarAugmentation(resultado, Tabela(),
        cabecalho: 'part of', uri: 'a.dart'),
    "part of 'a.dart';\n\n"
    "import 'dart:core' as prefix0;\n"
    "import 'package:a/a.dart' as prefix1;\n\n"
    'class PontoGemeo {}\n'
    'augment abstract base class Ponto<T> implements prefix0.Comparable<prefix1.Ponto> {\n'
    '  int get x => 1;\n'
    '}\n',
  );
  conferir(
    montarAugmentation([
      {
        'biblioteca': [
          codigo([
            'prefix prefix0 ',
            {'i': 1}
          ])
        ],
      },
    ], Tabela(), cabecalho: 'augment library', uri: 'a.dart'),
    "augment library 'a.dart';\n\nimport 'dart:core' as prefix1_0;\n\nprefix prefix0 prefix1_0.Comparable\n",
  );
  print('montagem Dart: 2 casos iguais ao contrato Rust/CFE');
}
