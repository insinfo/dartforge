class A<E> {
  late E element;
}
f(A<int, int> a) {
//^^^^^^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'A' is declared with 1 type parameters, but 2 type arguments were given.
  a.element.anyGetterExistsInDynamic;
}
