extension type A<S, T>(int it) {}

void f(A<int> a) {}
//     ^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'A' is declared with 2 type parameters, but 1 type arguments were given.
