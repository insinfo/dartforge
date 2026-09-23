// R-NUL-03: encurtamento nulo: a cadeia inteira depois de `?.` fica
// anulável; `?..` e `?[`.
class A {
  B b = B();
  B? bn;
}

class B {
  int c = 0;
  int m() => 1;
}

void f(A? a, List<int>? l) {
  print([/*@*/a?.b.c, /*@*/a?.b.m(), /*@*/a?.b, /*@*/a?.bn?.c, /*@*/a?.b.c.isEven]);
  print([/*@*/l?..add(1), /*@*/l?[0], /*@*/l?.length]);
}

void main() => f(A(), [1]);
