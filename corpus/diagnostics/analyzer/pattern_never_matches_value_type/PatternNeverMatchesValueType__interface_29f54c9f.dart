void f(A x) {
  if (x case R _) {}
}

class A {}
sealed class R {}
final class R1 extends R {}
final class R2 extends R implements A {}
