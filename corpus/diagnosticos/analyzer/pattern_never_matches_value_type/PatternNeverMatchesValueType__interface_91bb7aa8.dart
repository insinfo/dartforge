void f(A x) {
  if (x case R _) {}
}

final class A extends Object with R {}
mixin class R {}
