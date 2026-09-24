class A() {
  int v;
  this : v = 0, v = 0, v = 0;
//              ^
// [diag.fieldInitializedByMultipleInitializers] The field 'v' can't be initialized twice in the same constructor.
//                     ^
// [diag.fieldInitializedByMultipleInitializers] The field 'v' can't be initialized twice in the same constructor.
}
