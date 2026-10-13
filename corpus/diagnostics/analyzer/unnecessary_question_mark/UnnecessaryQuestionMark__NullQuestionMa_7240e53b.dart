void f(Object a) {
  switch (a) {
    case var _ as Null?:
//                ^^^^^
// [diag.patternNeverMatchesValueType] The matched value type 'Object' can never match the required type 'Null'.
//                    ^
// [diag.unnecessaryQuestionMark] The '?' is unnecessary because 'Null' is nullable without it.
  }
}
