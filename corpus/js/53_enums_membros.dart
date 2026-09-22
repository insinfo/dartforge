// Enhanced enums: campos finais, construtor const, métodos, getters, implements, generics, static, mixin.
abstract class Descritivel {
  String descreve();
}

mixin Rotulo on Enum {
  String get rotulo => name.toUpperCase();
}

enum Planeta implements Descritivel {
  mercurio(ordem: 1, luas: 0),
  terra(ordem: 3, luas: 1),
  marte(ordem: 4, luas: 2),
  jupiter(ordem: 5, luas: 95);

  const Planeta({required this.ordem, required this.luas});

  final int ordem;
  final int luas;

  bool get temLuas => luas > 0;

  bool get interno => ordem <= 4;

  @override
  String descreve() => '$name: ordem $ordem, $luas luas';

  static Planeta comMaisLuas() {
    var melhor = values.first;
    for (final p in values) {
      if (p.luas > melhor.luas) melhor = p;
    }
    return melhor;
  }

  static const Planeta padrao = terra;
}

enum Operacao {
  soma('+', 1),
  subtracao('-', 1),
  multiplicacao('*', 2);

  const Operacao(this.simbolo, this.precedencia);
  final String simbolo;
  final int precedencia;

  int aplica(int a, int b) {
    switch (this) {
      case Operacao.soma:
        return a + b;
      case Operacao.subtracao:
        return a - b;
      case Operacao.multiplicacao:
        return a * b;
    }
  }

  @override
  String toString() => 'Op($simbolo)';
}

enum Valor<T> {
  inteiro<int>(7),
  texto<String>('sete'),
  lista<List<int>>([7, 7]);

  const Valor(this.padrao);
  final T padrao;

  T get() => padrao;
}

enum Nivel with Rotulo {
  baixo(1),
  medio(5),
  alto(9);

  const Nivel(this.peso);
  final int peso;

  Nivel get seguinte => values[(index + 1) % values.length];

  static Nivel deP(int p) => values.lastWhere((n) => n.peso <= p);
}

void main() {
  print(Planeta.terra);
  print(Planeta.terra.ordem);
  print(Planeta.marte.luas);
  print(Planeta.mercurio.temLuas);
  print(Planeta.jupiter.interno);
  print(Planeta.terra.descreve());
  print(Planeta.comMaisLuas());
  print(Planeta.padrao);
  print(Planeta.values.where((p) => p.interno).map((p) => p.name).toList());
  final Descritivel d = Planeta.marte;
  print(d.descreve());
  print(d is Enum);
  print(Planeta.terra is Descritivel);

  for (final op in Operacao.values) {
    print('${op.simbolo} ${op.precedencia} ${op.aplica(6, 3)} $op');
  }
  print(Operacao.values.map((o) => o.name).join(' '));
  print(Operacao.multiplicacao.index);
  print(Operacao.soma.name);

  print(Valor.inteiro.get());
  print(Valor.texto.get());
  print(Valor.lista.get());
  print(Valor.inteiro.padrao + 1);
  print(Valor.texto.padrao.length);
  print(Valor.values.map((v) => v.padrao.toString()).toList());

  print(Nivel.baixo.rotulo);
  print(Nivel.alto.seguinte);
  print(Nivel.medio.seguinte.rotulo);
  print(Nivel.deP(6));
  print(Nivel.deP(9));
  print(Nivel.deP(1));
  print(Nivel.values.map((n) => n.peso).reduce((a, b) => a + b));
  print(Nivel.medio is Rotulo);
  print(Nivel.medio == Nivel.deP(5));
}
