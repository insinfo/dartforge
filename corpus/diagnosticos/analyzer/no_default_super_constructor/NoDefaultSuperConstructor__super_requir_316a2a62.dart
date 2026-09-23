class A {
  A({required int? a});
}
class B({super.a = 0}) extends A {
  this;
}
