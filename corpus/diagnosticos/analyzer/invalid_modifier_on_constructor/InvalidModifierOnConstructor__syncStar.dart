class A {
  A() sync* {}
//    ^^^^
// [diag.invalidModifierOnConstructor] The modifier 'sync' can't be applied to the body of a constructor.
}
