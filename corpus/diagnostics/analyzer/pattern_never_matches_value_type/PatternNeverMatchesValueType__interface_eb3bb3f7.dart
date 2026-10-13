void f(A x) {
  if (x case M _) {}
}

sealed class A {}
mixin M implements A {}
