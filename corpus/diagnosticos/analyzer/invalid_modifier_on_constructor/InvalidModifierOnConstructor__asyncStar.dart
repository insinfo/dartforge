class A {
  A() async* {}
//    ^^^^^
// [diag.invalidModifierOnConstructor] The modifier 'async' can't be applied to the body of a constructor.
}
