void f(Object? x) {
  switch (x) {
    case int(sign: 0, sign: 1):
//           ^^^^
// [context 1] The first field.
//                    ^^^^
// [diag.duplicatePatternField][context 1] The field 'sign' is already matched in this pattern.
      break;
  }
}
