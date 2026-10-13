class A {
  const A({int a = 0});
}

class B extends A {
  static const f = B();

  const B({super.a = 2});
}
