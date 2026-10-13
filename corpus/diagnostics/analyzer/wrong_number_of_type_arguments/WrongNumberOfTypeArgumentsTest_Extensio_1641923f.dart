extension type A<T>(int it) {}

void f(A<int, String> a) {}
//     ^^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'A' is declared with 1 type parameters, but 2 type arguments were given.
