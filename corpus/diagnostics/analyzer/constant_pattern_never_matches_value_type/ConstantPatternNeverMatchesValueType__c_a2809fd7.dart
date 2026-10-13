void f<T>(A<T> x) {
  if (x case const A<int>()) {}
}

class A<T> {
  const A();
}
