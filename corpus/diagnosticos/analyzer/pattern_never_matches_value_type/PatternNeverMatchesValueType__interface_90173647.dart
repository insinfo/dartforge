void f(A x) {
  if (x case E _) {}
}

sealed class A {}
enum E implements A {
  v
}
