void f(A<int> x) {
  if (x case const B()) {}
}

class A<T> {
  const A();
}

class B extends A<int> {
  const B();
}
