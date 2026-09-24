class A {
  set foo(int _) sync* {}
//               ^^^^
// [diag.invalidModifierOnSetter] Setters can't use 'async', 'async*', or 'sync*'.
}
