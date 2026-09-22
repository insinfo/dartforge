// Enums básicos: values, index, name, toString, comparação, switch exaustivo, byName, em Map/Set.
enum Cor { vermelho, verde, azul }

enum Dia { seg, ter, qua, qui, sex, sab, dom }

String descreveCor(Cor c) {
  switch (c) {
    case Cor.vermelho:
      return 'quente';
    case Cor.verde:
      return 'natural';
    case Cor.azul:
      return 'frio';
  }
}

bool fimDeSemana(Dia d) => switch (d) {
      Dia.sab || Dia.dom => true,
      _ => false,
    };

Dia proximo(Dia d) => Dia.values[(d.index + 1) % Dia.values.length];

void main() {
  print(Cor.values);
  print(Cor.values.length);
  print(Cor.verde);
  print(Cor.verde.name);
  print(Cor.verde.index);
  print(Cor.azul.toString());
  print('${Cor.vermelho}');
  print(Cor.vermelho == Cor.vermelho);
  print(Cor.vermelho == Cor.azul);
  print(identical(Cor.verde, Cor.values[1]));

  for (final c in Cor.values) {
    print('${c.name}: ${descreveCor(c)}');
  }

  print(Cor.values.byName('azul'));
  print(Cor.values.byName('azul').index);
  try {
    Cor.values.byName('roxo');
  } catch (e) {
    print('byName falhou: ${e is ArgumentError}');
  }
  print(Cor.values.asNameMap().keys.toList());
  print(Cor.values.asNameMap()['verde']);

  print(Dia.values.map((d) => d.name).join(','));
  print(Dia.values.where(fimDeSemana).map((d) => d.name).toList());
  print(proximo(Dia.dom));
  print(proximo(Dia.qua));
  print(Dia.sex.index > Dia.seg.index);
  final ordenados = [Dia.sex, Dia.seg, Dia.qua]..sort((a, b) => a.index - b.index);
  print(ordenados);

  final precos = <Cor, int>{Cor.vermelho: 10, Cor.verde: 20};
  precos[Cor.azul] = 30;
  print(precos);
  print(precos[Cor.verde]);
  print(precos.containsKey(Cor.azul));
  final vistas = <Cor>{Cor.azul, Cor.azul, Cor.vermelho};
  print(vistas.length);
  print(vistas.contains(Cor.verde));
  print(Cor.values.where((c) => !vistas.contains(c)).toList());

  final Object o = Dia.ter;
  print(o is Dia);
  print(o is Cor);
  print(o.runtimeType == Dia);
  print(Cor.values.indexOf(Cor.azul));
  print(Cor.values.first.name);
  print(Cor.values.last.name);
  print(Cor.values.reversed.map((c) => c.index).toList());
}
