void f(A x) {
  if (x case R _) {}
}

class A {}
sealed class R {}
final class R1 extends R {}
class R2 extends R {}
