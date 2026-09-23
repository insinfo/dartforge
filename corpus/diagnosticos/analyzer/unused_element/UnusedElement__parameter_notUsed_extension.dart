extension E on String {
  void _m([int? a]) {}
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => "hello"._m();
