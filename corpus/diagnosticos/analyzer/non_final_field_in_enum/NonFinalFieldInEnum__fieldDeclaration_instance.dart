enum E {
  v;
  int foo = 0;
//    ^^^
// [diag.nonFinalFieldInEnum] Enums can only declare final fields.
}
