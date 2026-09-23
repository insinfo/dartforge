enum E({required var int foo}) {
//                       ^^^
// [diag.nonFinalFieldInEnum] Enums can only declare final fields.
  v(foo: 0);
}
