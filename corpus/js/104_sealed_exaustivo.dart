// sealed class com switch expression exaustivo sem default, padrões de objeto, árvore recursiva de expressões.
sealed class Forma {}

class Circulo extends Forma {
  final double raio;
  Circulo(this.raio);
}

class Retangulo extends Forma {
  final double w;
  final double h;
  Retangulo(this.w, this.h);
}

class Triangulo extends Forma {
  final double base;
  final double altura;
  Triangulo(this.base, this.altura);
}

double area(Forma f) => switch (f) {
      Circulo(raio: final r) => 3 * r * r,
      Retangulo(w: final w, h: final h) => w * h,
      Triangulo(:final base, :final altura) => base * altura / 2,
    };

String nome(Forma f) => switch (f) {
      Circulo() => 'circulo',
      Retangulo(w: final w, h: final h) when w == h => 'quadrado',
      Retangulo() => 'retangulo',
      Triangulo() => 'triangulo',
    };

sealed class Expr {}

class Num extends Expr {
  final int valor;
  Num(this.valor);
}

class Soma extends Expr {
  final Expr a;
  final Expr b;
  Soma(this.a, this.b);
}

class Mult extends Expr {
  final Expr a;
  final Expr b;
  Mult(this.a, this.b);
}

class Neg extends Expr {
  final Expr e;
  Neg(this.e);
}

int avalia(Expr e) => switch (e) {
      Num(:final valor) => valor,
      Soma(:final a, :final b) => avalia(a) + avalia(b),
      Mult(:final a, :final b) => avalia(a) * avalia(b),
      Neg(e: final inner) => -avalia(inner),
    };

String mostra(Expr e) => switch (e) {
      Num(:final valor) => '$valor',
      Soma(:final a, :final b) => '(${mostra(a)} + ${mostra(b)})',
      Mult(:final a, :final b) => '${mostra(a)} * ${mostra(b)}',
      Neg(:final e) => '-${mostra(e)}',
    };

Expr simplifica(Expr e) => switch (e) {
      Soma(a: Num(valor: 0), :final b) => simplifica(b),
      Soma(:final a, b: Num(valor: 0)) => simplifica(a),
      Mult(a: Num(valor: 1), :final b) => simplifica(b),
      Mult(:final a, b: Num(valor: 1)) => simplifica(a),
      Mult(a: Num(valor: 0), b: _) || Mult(a: _, b: Num(valor: 0)) => Num(0),
      Neg(e: Neg(e: final inner)) => simplifica(inner),
      Soma(:final a, :final b) => Soma(simplifica(a), simplifica(b)),
      Mult(:final a, :final b) => Mult(simplifica(a), simplifica(b)),
      Neg(:final e) => Neg(simplifica(e)),
      Num() => e,
    };

int profundidade(Expr e) {
  switch (e) {
    case Num():
      return 1;
    case Soma(:final a, :final b):
    case Mult(:final a, :final b):
      final pa = profundidade(a);
      final pb = profundidade(b);
      return 1 + (pa > pb ? pa : pb);
    case Neg(:final e):
      return 1 + profundidade(e);
  }
}

sealed class Resultado<T> {}

class Ok<T> extends Resultado<T> {
  final T valor;
  Ok(this.valor);
}

class Erro<T> extends Resultado<T> {
  final String msg;
  Erro(this.msg);
}

Resultado<int> divide(int a, int b) => b == 0 ? Erro('divisao por zero') : Ok(a ~/ b);

String mostraResultado(Resultado<int> r) => switch (r) {
      Ok(:final valor) => 'ok=$valor',
      Erro(:final msg) => 'erro=$msg',
    };

void main() {
  final formas = <Forma>[Circulo(1.5), Retangulo(2, 2.5), Retangulo(3, 3), Triangulo(4, 2.5)];
  for (final f in formas) {
    print('${nome(f)}: ${area(f).toStringAsFixed(2)}');
  }

  final e = Soma(Mult(Num(2), Num(3)), Neg(Soma(Num(1), Num(4))));
  print(mostra(e));
  print(avalia(e));
  print(profundidade(e));
  final e2 = Soma(Num(0), Mult(Num(1), Neg(Neg(Num(7)))));
  print(mostra(e2));
  print(mostra(simplifica(e2)));
  print(avalia(e2) == avalia(simplifica(e2)));
  final e3 = Mult(Soma(Num(2), Num(0)), Num(0));
  print(mostra(simplifica(e3)));
  print(avalia(e3));
  print(profundidade(Num(1)));
  print(profundidade(Neg(Neg(Neg(Num(1))))));

  print(mostraResultado(divide(10, 3)));
  print(mostraResultado(divide(1, 0)));
  final resultados = [divide(9, 3), divide(5, 0), divide(8, 2)];
  final oks = resultados.whereType<Ok<int>>().map((o) => o.valor).toList();
  print(oks);
  print(resultados.map(mostraResultado).join(', '));

  final Forma f = Circulo(2);
  final descricao = switch (f) {
    Circulo(raio: > 1) => 'circulo grande',
    Circulo() => 'circulo pequeno',
    Retangulo() || Triangulo() => 'poligono',
  };
  print(descricao);
}
