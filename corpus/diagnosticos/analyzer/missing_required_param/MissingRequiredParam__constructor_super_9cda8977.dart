class A {
  A({required int a});
}

class B extends A {
  B({required super.a}) : super();
}
