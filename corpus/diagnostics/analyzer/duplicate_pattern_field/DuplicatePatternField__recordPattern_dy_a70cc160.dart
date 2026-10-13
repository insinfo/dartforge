void f(x) {
  switch (x) {
    case (foo: 0, :var foo):
//        ^^^
// [context 1] The first field.
//                ^
// [diag.duplicatePatternField][context 1] The field 'foo' is already matched in this pattern.
//                     ^^^
// [diag.unusedLocalVariable] The value of the local variable 'foo' isn't used.
      break;
  }
}
