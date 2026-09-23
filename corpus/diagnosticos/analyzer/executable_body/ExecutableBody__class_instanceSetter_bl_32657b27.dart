class A {
  set foo(int _) async* {}
//               ^^^^^
// [diag.invalidModifierOnSetter] Setters can't use 'async', 'async*', or 'sync*'.
}
