void f(x) {
  switch (x) {
    case (:var foo, foo: 1):
//        ^
// [context 1] The first field.
//             ^^^
// [diag.unusedLocalVariable] The value of the local variable 'foo' isn't used.
//                  ^^^
// [diag.duplicatePatternField][context 1] The field 'foo' is already matched in this pattern.
      break;
  }
}
