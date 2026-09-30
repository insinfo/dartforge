// `is C` e `as C` de classes sem argumentos de tipo pelo mapa de bits do
// alvo (`df.subclasse`, `MapasDeSubtipo` no runtime): hierarquias com
// herança, interfaces, mixins (inclusive aplicação nomeada e `on`),
// classes abstratas, `sealed` com `switch` por tipo, classes do SDK
// (erros, exceções, `StringSink`, `Comparable`) e do usuário que as
// implementam, e um objeto de cada classe testado contra todos os alvos
// (a matriz inteira de uma vez, nas duas ordens). A saída tem de ser a da
// VM.

abstract class Forma {
  double area();
}

abstract interface class Nomeado {
  String get nome;
}

mixin Contador {
  int contagem = 0;
  void contar() => contagem++;
}

mixin Registra on Forma {
  String registro() => 'área ${area().toStringAsFixed(1)}';
}

class Circulo extends Forma with Contador implements Nomeado {
  final double r;
  Circulo(this.r);
  @override
  double area() => 3 * r * r;
  @override
  String get nome => 'círculo';
}

class Quadrado extends Forma with Registra implements Nomeado, Comparable<Quadrado> {
  final double l;
  Quadrado(this.l);
  @override
  double area() => l * l;
  @override
  String get nome => 'quadrado';
  @override
  int compareTo(Quadrado o) => l.compareTo(o.l);
}

class QuadradoGrande extends Quadrado with Contador {
  QuadradoGrande() : super(10);
}

class Base {}

class Meio = Base with Contador;

class Folha extends Meio implements Nomeado {
  @override
  String get nome => 'folha';
}

class Solto {}

class MeuErro extends StateError {
  MeuErro() : super('meu');
}

class MinhaExcecao implements Exception {
  @override
  String toString() => 'MinhaExcecao';
}

class Pia implements StringSink {
  final b = StringBuffer();
  @override
  void write(Object? o) => b.write(o);
  @override
  void writeAll(Iterable<Object?> os, [String sep = '']) => b.writeAll(os, sep);
  @override
  void writeCharCode(int c) => b.writeCharCode(c);
  @override
  void writeln([Object? o = '']) => b.writeln(o);
}

sealed class Expr {}

class Num extends Expr {
  final int v;
  Num(this.v);
}

class Soma extends Expr {
  final Expr a, b;
  Soma(this.a, this.b);
}

class Mul extends Expr {
  final Expr a, b;
  Mul(this.a, this.b);
}

int avaliar(Expr e) => switch (e) {
      Num(v: final v) => v,
      Soma(a: final a, b: final b) => avaliar(a) + avaliar(b),
      Mul(a: final a, b: final b) => avaliar(a) * avaliar(b),
    };

final testes = <String, bool Function(Object?)>{
  'Forma': (x) => x is Forma,
  'Nomeado': (x) => x is Nomeado,
  'Contador': (x) => x is Contador,
  'Registra': (x) => x is Registra,
  'Circulo': (x) => x is Circulo,
  'Quadrado': (x) => x is Quadrado,
  'QuadradoGrande': (x) => x is QuadradoGrande,
  'Base': (x) => x is Base,
  'Meio': (x) => x is Meio,
  'Folha': (x) => x is Folha,
  'Solto': (x) => x is Solto,
  'Comparable': (x) => x is Comparable,
  'Error': (x) => x is Error,
  'StateError': (x) => x is StateError,
  'Exception': (x) => x is Exception,
  'FormatException': (x) => x is FormatException,
  'StringSink': (x) => x is StringSink,
  'Expr': (x) => x is Expr,
  'Soma': (x) => x is Soma,
  'Forma?': (x) => x is Forma?,
};

void main() {
  final objetos = <String, Object?>{
    'Circulo': Circulo(1),
    'Quadrado': Quadrado(2),
    'QuadradoGrande': QuadradoGrande(),
    'Base': Base(),
    'Meio': Meio(),
    'Folha': Folha(),
    'Solto': Solto(),
    'MeuErro': MeuErro(),
    'StateError': StateError('x'),
    'ArgumentError': ArgumentError('y'),
    'RangeError': RangeError('z'),
    'MinhaExcecao': MinhaExcecao(),
    'FormatException': const FormatException('f'),
    'StringBuffer': StringBuffer(),
    'Pia': Pia(),
    'Num': Num(1),
    'Soma': Soma(Num(1), Num(2)),
    'texto': 'abc',
    'int': 7,
    'null': null,
  };
  // Alvo por alvo (cada mapa montado de uma vez) …
  for (final t in testes.entries) {
    final sim = [
      for (final o in objetos.entries)
        if (t.value(o.value)) o.key
    ];
    print('${t.key}: ${sim.join(' ')}');
  }
  // … e objeto por objeto (os alvos alternando).
  for (final o in objetos.entries) {
    final sim = [
      for (final t in testes.entries)
        if (t.value(o.value)) t.key
    ];
    print('${o.key} é ${sim.join(' ')}');
  }

  // `as` que passa e que falha.
  for (final o in objetos.values) {
    try {
      print((o as Nomeado).nome);
    } on TypeError {
      print('não é Nomeado');
    }
  }

  // Mixins com estado e `on`.
  final c = Circulo(2)..contar()..contar();
  final g = QuadradoGrande()..contar();
  print('${c.contagem} ${g.contagem} ${g.registro()} ${Quadrado(3).registro()}');
  final qs = [Quadrado(3), Quadrado(1), QuadradoGrande(), Quadrado(2)]..sort();
  print(qs.map((q) => q.l).join(' '));

  // `sealed` com `switch` por tipo.
  final e = Soma(Num(2), Mul(Num(3), Soma(Num(1), Num(4))));
  print(avaliar(e));

  // Erros e exceções pelo `on`.
  for (final f in <void Function()>[
    () => throw MeuErro(),
    () => throw MinhaExcecao(),
    () => throw const FormatException('ruim'),
    () => [1][3],
    () => int.parse('x'),
  ]) {
    try {
      f();
    } on StateError catch (e) {
      print('StateError ${e.message}');
    } on FormatException catch (e) {
      print('FormatException ${e.message}');
    } on Exception catch (e) {
      print('Exception $e');
    } on Error catch (e) {
      print('Error ${e.runtimeType}');
    }
  }

  // `StringSink` do usuário e do SDK pelo mesmo ponto de chamada.
  for (final s in <StringSink>[StringBuffer(), Pia()]) {
    s.write('a');
    s.writeAll([1, 2], '-');
    s.writeCharCode(0x41);
    print('${s is StringBuffer} ${s is Pia} ${s is StringSink}');
  }

  // Muitos testes num laço: o caminho em linha, depois de montado o mapa.
  var n = 0;
  final lista = objetos.values.toList();
  for (var k = 0; k < 5000; k++) {
    for (final o in lista) {
      if (o is Forma) n++;
      if (o is Nomeado) n += 2;
      if (o is Contador) n += 4;
      if (o is Error) n += 8;
      if (o is Exception) n += 16;
    }
  }
  print('contagem: $n');
}
