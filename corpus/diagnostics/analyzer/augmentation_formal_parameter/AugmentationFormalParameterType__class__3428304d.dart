class A {
  A([int? p1]);
}
class B extends A {
  B([int? p1]);
  augment B([super.p1]);
}
