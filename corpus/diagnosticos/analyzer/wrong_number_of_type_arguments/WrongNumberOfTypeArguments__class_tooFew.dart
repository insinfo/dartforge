class A<E, F> {}
A<A>? a;
// [diag.wrongNumberOfTypeArguments][column 1][length 5] The type 'A' is declared with 2 type parameters, but 1 type arguments were given.
