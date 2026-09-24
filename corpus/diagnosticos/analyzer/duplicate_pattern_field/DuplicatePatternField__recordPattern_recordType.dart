void f(({int foo}) x) {
  switch (x) {
    case (foo: 0, foo: 1):
//        ^^^
// [context 1] The first field.
//                ^^^
// [diag.duplicatePatternField][context 1] The field 'foo' is already matched in this pattern.
      break;
  }
}
