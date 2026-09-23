class A {
  final int? a;
  A({this.a});
}

class B extends A {
  B({required super.a});
}

class C extends B {
  C({super.a});
}
