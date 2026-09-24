void f(void Function() x) {
  if (x case int _) {}
//           ^^^
// [diag.patternNeverMatchesValueType] The matched value type 'void Function()' can never match the required type 'int'.
}
