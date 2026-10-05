class A {
  A.foo();
  static void bar() {}
}
typedef TA = A;
A.foo v1 = throw 0;
A.bar v2 = throw 0;
A.zzz v3 = throw 0;
TA.foo v4 = throw 0;
void f(Object o) {
  o is A.foo;
  o as A.foo;
  <A.foo>[];
  try {} on A.foo catch (e) {}
  new A.foo.x();
  const A.foo.x();
  new A.foo();
  A<int>.foo();
  new A.foo<int>();
}
class B extends A.foo {}
