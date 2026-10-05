class A<X> {
  const A();
  A.nc();
}
class K<T> {
  m() {
    var a = const A<T>.nao_existe();
    var b = const A<T>.nc();
    return [a, b];
  }
}
