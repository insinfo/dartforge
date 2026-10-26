// Variáveis `late` como o CFE as reduz: com tipo anulável, `null` é um valor
// (um campo atribuído a `null` não reexecuta o inicializador e um `late final`
// atribuído a `null` não aceita outra atribuição); um `late final` com
// inicializador que é atribuído durante a própria inicialização lança; um
// campo `late final` com inicializador herda o setter do supertipo; o
// construtor marca o campo como atribuído; um acesso dinâmico encontra o
// campo pelo nome; e um campo `late` que sobrescreve um `covariant` verifica
// o valor. A expressão de um padrão constante é uma constante canônica.

int calls = 0;
int? nothing() {
  calls++;
  return null;
}

late int? top = nothing();
late final int? topFinal;

class A {
  static late int? s = nothing();
  late int? x = nothing();
  late final int? y;
  late int? fromCtor;
  late final int reentrant = initReentrant();

  A() : fromCtor = null;

  int initReentrant() {
    if (calls++ == 0) {
      print(reentrant);
    }
    return 2;
  }
}

class Base {
  late final int field;
}

class Sub extends Base {
  @override
  late final int field = 7;
  int get superField => super.field;
}

class Cov {
  covariant late num n;
}

class CovSub extends Cov {
  @override
  late int n;
}

void throws(void Function() f) {
  try {
    f();
    print('did not throw');
  } catch (e) {
    print('threw');
  }
}

void main() {
  print([top, top, calls]);
  topFinal = null;
  throws(() => topFinal = 1);
  print(topFinal);

  calls = 0;
  print([A.s, A.s, calls]);
  A.s = null;
  print([A.s, calls]);

  final a = A();
  calls = 0;
  print([a.x, a.x, calls]);
  a.y = null;
  throws(() => a.y = 1);
  print([a.y, a.fromCtor]);

  calls = 0;
  throws(() => print(a.reentrant));

  Base b = Sub();
  b.field = 3;
  print([b.field, (b as Sub).superField]);
  throws(() => b.field = 4);

  dynamic d = A();
  d.x = 5;
  print([d.x, d.fromCtor]);

  Cov c = CovSub();
  c.n = 1;
  throws(() => c.n = 1.5);
  print(c.n);

  const list = <int>[42];
  switch (list) {
    case const (<int>[42]):
      print('canonical');
    default:
      print('not canonical');
  }
  if (list case const <int>[42]) print('canonical again');
}
