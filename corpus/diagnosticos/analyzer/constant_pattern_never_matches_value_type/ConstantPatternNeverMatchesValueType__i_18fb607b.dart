void f(void Function() x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'void Function()' can never be equal to this constant of type 'int'.
}

class A {}
