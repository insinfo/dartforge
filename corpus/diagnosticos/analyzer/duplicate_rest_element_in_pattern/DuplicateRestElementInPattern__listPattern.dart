void f(List<int> x) {
  if (x case [..., ...]) {}
//            ^^^
// [context 1] The first rest element.
//                 ^^^
// [diag.duplicateRestElementInPattern][context 1] At most one rest element is allowed in a list or map pattern.
}
