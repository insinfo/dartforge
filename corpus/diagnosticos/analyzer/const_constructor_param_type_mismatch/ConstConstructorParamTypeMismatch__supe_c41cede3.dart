class A<T> {
  const A({int a = 0});
}

class B extends A<int> {
  const B({super.a});
}

const b = const B();
