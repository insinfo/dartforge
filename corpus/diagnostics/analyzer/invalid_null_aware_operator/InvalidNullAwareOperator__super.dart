class A {
  void foo() {}
}

class B extends A {
  void bar() {
    super?.foo();
//       ^^
// [diag.invalidOperatorQuestionmarkPeriodForSuper] The operator '?.' cannot be used with 'super' because 'super' cannot be null.
  }
}
