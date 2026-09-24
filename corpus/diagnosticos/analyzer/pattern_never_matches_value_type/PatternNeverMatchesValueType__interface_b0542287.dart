void f(A x) {
  if (x case R _) {}
}

final class A {}
final class A2 extends A {}
final class A3 extends A2 implements R {}
class R {}
