class Target<T> {}

class SubTarget<T> extends Target<T> {}

extension E1 on SubTarget<Object> {
  int get foo => 0;
}

extension E2<T> on Target<T> {
  int get foo => 0;
}

f(SubTarget<num> t) {
  // The instantiated on type of `E1(t)` is `SubTarget<Object>`.
  // The instantiated on type of `E2(t)` is `Target<num>`.
  // Neither is a subtype of the other, so the resolution is ambiguous.
  t.foo;
//  ^^^
// [diag.ambiguousExtensionMemberAccessTwo] A member named 'foo' is defined in 'extension E1 on SubTarget<Object>' and 'extension E2<T> on Target<T>', and neither is more specific.
}
