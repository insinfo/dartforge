class A {
  int __a = 0;
  int get _a => __a;
//        ^^
// [diag.unusedElement] The declaration '_a' isn't referenced.
  void set _a(int val) {
    __a = val;
  }
  int b() => _a = 7;
}
class B extends A {
  @override
  int get _a => 3;
//        ^^
// [diag.unusedElement] The declaration '_a' isn't referenced.
}
