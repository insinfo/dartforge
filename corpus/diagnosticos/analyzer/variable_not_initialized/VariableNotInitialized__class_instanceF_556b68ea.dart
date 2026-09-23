class A() {
  int v1;
  int v2;
  this : v1 = 0, v1 = 0, v2 = 0, v2 = 0;
//               ^^
// [diag.fieldInitializedByMultipleInitializers] The field 'v1' can't be initialized twice in the same constructor.
//                               ^^
// [diag.fieldInitializedByMultipleInitializers] The field 'v2' can't be initialized twice in the same constructor.
}
