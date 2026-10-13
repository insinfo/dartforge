void f(({A f1,}) x) {
  if (x case ({R f1,}) _) {}
//           ^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type '({A f1})' can never match the required type '({R f1})'.
}

final class A {}
class R {}
