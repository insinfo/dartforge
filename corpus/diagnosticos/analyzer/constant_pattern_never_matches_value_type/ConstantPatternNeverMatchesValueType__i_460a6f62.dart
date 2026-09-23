void f((int, int) x) {
  if (x case 0) {}
//           ^
// [diag.constantPatternNeverMatchesValueType] The matched value type '(int, int)' can never be equal to this constant of type 'int'.
}

class A {}
