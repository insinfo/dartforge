class A<T> {
  const A();
  void m() {
    const A<List<T>>();
//               ^
// [diag.constWithTypeParameters] A constant creation can't use a type parameter as a type argument.
  }
}
