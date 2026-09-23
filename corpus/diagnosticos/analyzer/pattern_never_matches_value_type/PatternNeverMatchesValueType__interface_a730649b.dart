void f(A x) {
  if (x case R _) {}
}

sealed class A {}
final class A2 extends A {}
class A3 implements A {}
class R {}
