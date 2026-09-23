class A<T> {
  const A();
  void m() {
    const A<void Function<U>(void Function<V>(U, V))>();
  }
}
