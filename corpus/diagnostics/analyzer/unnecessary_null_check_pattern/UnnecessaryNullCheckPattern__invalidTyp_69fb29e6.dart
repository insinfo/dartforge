UnknownType getValue() => UnknownType();
// [diag.undefinedClass][column 1][length 11] Undefined class 'UnknownType'.
//                        ^^^^^^^^^^^
// [diag.undefinedFunction] The function 'UnknownType' isn't defined.
void f() {
  if (getValue() case final valueX?) {
    print(valueX);
  }
}
