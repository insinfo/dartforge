typedef A<X> = X Function(X);

enum E<T extends A<T>, U> {
//   ^
// [diag.enumInstantiatedToBoundsIsNotWellBounded] The result of instantiating the enum to bounds is not well-bounded.
  v<Never, int>()
}
