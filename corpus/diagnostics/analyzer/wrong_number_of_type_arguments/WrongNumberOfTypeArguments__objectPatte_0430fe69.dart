abstract class A {
  int get foo;
}

void f(x) {
  switch (x) {
    case A<int>(foo: 0):
//       ^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'A' is declared with 0 type parameters, but 1 type arguments were given.
      break;
  }
}
