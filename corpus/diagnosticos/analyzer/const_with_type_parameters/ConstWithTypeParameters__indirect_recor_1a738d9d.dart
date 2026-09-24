class A<T> {
  const A();
  void m() {
    const A<(T, int)>();
//           ^
// [diag.constWithTypeParameters] A constant creation can't use a type parameter as a type argument.
  }
}
