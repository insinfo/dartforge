enum E(var int foo) {
//             ^^^
// [diag.nonFinalFieldInEnum] Enums can only declare final fields.
  v(0);
}
