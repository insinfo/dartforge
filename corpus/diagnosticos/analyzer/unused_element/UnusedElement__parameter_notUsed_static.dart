class A {
  static void _m([int? a]) {}
//                     ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => A._m();
