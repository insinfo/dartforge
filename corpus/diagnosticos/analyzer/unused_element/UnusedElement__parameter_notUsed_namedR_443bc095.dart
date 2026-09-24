class A {
  A() : this._named(b: 0);
  A._named({int a = 0, int b = 0});
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
