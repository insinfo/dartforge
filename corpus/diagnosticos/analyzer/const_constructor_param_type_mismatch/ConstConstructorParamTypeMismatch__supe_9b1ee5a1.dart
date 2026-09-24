class A {
  const A({int a = 0});
}

class B extends A {
  const B({super.a});
}

const b = const B();
