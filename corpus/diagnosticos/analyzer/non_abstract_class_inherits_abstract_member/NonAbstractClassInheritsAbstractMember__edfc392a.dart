class A {
  set s(v) {}
}
abstract class B extends A {
  set s(v);
}
class C extends B {}
