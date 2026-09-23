void f(A x) {
  if (x case R _) {}
}

final class A {}
final class A2 extends R implements A {}
class R {}
