class A {
  var a = undef1;
  static var b = undef2;
  final c;
  A() : c = undef3, super();
  A.n(int p) : c = p, assert(undef4);
  void m([x = undef5]) {}
}
var t = undef6;
void f([x = undef7]) {}
enum E { v(undef8); const E(Object o); }
