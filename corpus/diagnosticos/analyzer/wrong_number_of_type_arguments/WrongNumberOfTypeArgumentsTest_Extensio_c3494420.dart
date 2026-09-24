extension type A(int it) {}

void f(A<int> a) {}
//     ^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'A' is declared with 0 type parameters, but 1 type arguments were given.
