class A {
  A._([int? a]);
//          ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => A._();
