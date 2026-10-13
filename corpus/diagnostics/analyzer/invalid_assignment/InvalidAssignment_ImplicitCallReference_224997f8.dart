mixin M {
  void call(int a) {}
}
class C with M {}

Function f = C();
