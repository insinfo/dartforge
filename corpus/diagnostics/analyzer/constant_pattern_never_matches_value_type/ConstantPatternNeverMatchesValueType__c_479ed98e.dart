void f<T>(A<void Function(T)> x) {
  if (x case const A<void Function(int)>()) {}
}

class A<T> {
  const A();
}
