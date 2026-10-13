class A {
  final int a;
  A({this.a = 0});
}

class B extends A {
  B({required super.a});
}

class C extends B {
  C({super.a});
//         ^
// [diag.missingDefaultValueForParameter] The parameter 'a' can't have a value of 'null' because of its type, but the implicit default value is 'null'.
}
