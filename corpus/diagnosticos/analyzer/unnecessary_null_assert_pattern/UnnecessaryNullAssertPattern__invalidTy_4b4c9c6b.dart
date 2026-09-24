UnknownType? getValue() => null;
// [diag.undefinedClass][column 1][length 11] Undefined class 'UnknownType'.
void f() {
  if (getValue() case final valueX!) {
    print(valueX);
  }
}
