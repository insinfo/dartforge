void f(x) {
  if (x case {int: 0, E: 0}) {}
//            ^^^
// [context 1] The first key with this value.
//                    ^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
extension type E(int it) {}
