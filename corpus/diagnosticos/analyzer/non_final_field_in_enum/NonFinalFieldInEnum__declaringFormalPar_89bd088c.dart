enum E([var int foo = 0]) {
//              ^^^
// [diag.nonFinalFieldInEnum] Enums can only declare final fields.
  v(0);
}
