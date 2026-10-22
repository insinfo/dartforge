// Identidade e igualdade de tear-offs como na VM: o tear-off de função
// estática é constante (`const F(main)` canônico); a instanciação implícita de
// uma função genérica estática é canônica por argumentos de tipo; a de um
// método de instância tem o tipo instanciado, passa os argumentos de tipo ao
// corpo e é igual a outra do mesmo receptor e dos mesmos tipos; strings
// constantes concatenadas são o literal canônico em contexto constante.
class F {
  final f;
  const F(this.f);
}

int top(int x) => x;

class S {
  static int st(int x) => x;
}

X? genericTop<X>() => null;

class A {
  X? g<X>() {
    print(X);
    return null;
  }
}

const yz = "y" + "z";

void main() {
  print(identical(const F(main), const F(main)));
  print(identical(const F(top), const F(S.st)));
  print(identical(top, top));
  const g = top;
  print(identical(g, top));

  int? Function() t1 = genericTop;
  int? Function() t2 = genericTop;
  String? Function() t3 = genericTop;
  print(identical(t1, t2));
  print(identical(t1, t3));

  var a = A();
  int? Function() i1 = a.g;
  int? Function() i2 = a.g;
  String? Function() s1 = a.g;
  print(i1.runtimeType);
  i1();
  s1();
  print(i1 == i2);
  print(i1 == s1);

  const ab = identical("ab", "a" + "b");
  print(ab);
  const xyz = identical("xyz", "x" + yz);
  print(xyz);
  print(identical("abcd", "ab" + "cd"));
}
