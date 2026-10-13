void f(A x) {
  if (x case E _) {}
}

final class A {}
enum E implements A {
  v
}
