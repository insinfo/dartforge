class A<T> {
  const A();
  void m() {
    const A<void Function(T)>();
//                        ^
// [diag.constWithTypeParameters] A constant creation can't use a type parameter as a type argument.
  }
}
