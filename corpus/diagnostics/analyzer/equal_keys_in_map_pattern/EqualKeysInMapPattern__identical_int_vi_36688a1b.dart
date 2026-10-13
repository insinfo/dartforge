const a = 0;
const b = 0;

void f(x) {
  if (x case {a: 1, b: 2}) {}
//            ^
// [context 1] The first key with this value.
//                  ^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
