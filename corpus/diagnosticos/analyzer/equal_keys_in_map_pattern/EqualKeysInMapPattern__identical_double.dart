void f(x) {
  if (x case {3.14: 1, 3.14: 2}) {}
//            ^^^^
// [context 1] The first key with this value.
//                     ^^^^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
