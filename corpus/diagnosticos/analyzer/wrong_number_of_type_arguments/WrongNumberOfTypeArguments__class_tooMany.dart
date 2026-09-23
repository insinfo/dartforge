class A<E> {}
A<A, A>? a;
// [diag.wrongNumberOfTypeArguments][column 1][length 8] The type 'A' is declared with 1 type parameters, but 2 type arguments were given.
