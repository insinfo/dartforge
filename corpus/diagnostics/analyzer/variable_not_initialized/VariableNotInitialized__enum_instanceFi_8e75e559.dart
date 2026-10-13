enum A {
  e;
  int v = 0;
//    ^
// [diag.nonFinalFieldInEnum] Enums can only declare final fields.
  const A() : v = 0;
}
