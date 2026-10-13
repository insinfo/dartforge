extension on String {
  void m([int? a]) {}
//             ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => "hello".m();
