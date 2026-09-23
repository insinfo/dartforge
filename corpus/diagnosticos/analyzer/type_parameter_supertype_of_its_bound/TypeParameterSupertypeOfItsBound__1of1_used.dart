class A<T extends T> {
//      ^
// [diag.typeParameterSupertypeOfItsBound] 'T' can't be a supertype of its upper bound.
  void foo(x) {
    x is T;
  }
}
