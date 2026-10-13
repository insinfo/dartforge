void f(x) {
  if (x case {(a: 0): 1, (a: 0): 2}) {}
//            ^^^^^^
// [context 1] The first key with this value.
//                       ^^^^^^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
