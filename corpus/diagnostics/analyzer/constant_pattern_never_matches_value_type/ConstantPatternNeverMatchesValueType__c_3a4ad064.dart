void f(A<num> x) {
  if (x case const A<int>()) {}
}

class A<T> {
  const A();
}
