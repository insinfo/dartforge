class A {}

class B extends A {
  @override
  static set foo(int _) {}
//           ^^^
// [diag.overrideOnNonOverridingSetter] The setter doesn't override an inherited setter.
}
