enum E(var void foo()?) {
//              ^^^
// [diag.nonFinalFieldInEnum] Enums can only declare final fields.
  v(null);
}
